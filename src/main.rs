mod commands;
mod config;
mod core;
mod infra;
mod interface;
mod logging;
mod private_file;
mod runtime;

use crate::core::{entity, usecase};
use infra::disk::snapshot;
use infra::herdr;
use infra::linear::auth;
use interface::tui::{app, event};
use interface::{cli, control};

use std::collections::HashMap;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        supports_keyboard_enhancement,
    },
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend, TestBackend},
};

use base64::Engine;

use app::App;
use auth::token::TokenStore;
use config::Config;
use entity::OrganizationId;
use infra::linear::client::LinearClient;
use runtime::Runtime;

/// The size of the screen a headless instance draws, unless told otherwise.
const HEADLESS_SIZE: (u16, u16) = (120, 40);

/// How a launch runs the TUI.
#[derive(Clone, Copy)]
enum Mode {
    /// In this terminal.
    Terminal,
    /// With no terminal, for agents alone: they read and drive it through
    /// the control channel (`linear-tui tui …`).
    Headless { size: (u16, u16) },
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = logging::init();

    let args: Vec<String> = std::env::args().collect();

    // `open <ID>` and `--headless` run the TUI; every other subcommand runs
    // without it.
    let (mode, open) = match args.get(1).map(String::as_str) {
        Some("open") => {
            let [_, _, key] = args.as_slice() else {
                anyhow::bail!("Usage: linear-tui open <ID>");
            };
            (Mode::Terminal, Some(cli::issue_key(key)?))
        }
        Some("--headless") => (headless_mode(&args[2..])?, None),
        Some(_) => return commands::run(&args[1..]).await,
        None => (Mode::Terminal, None),
    };

    // Load config and authenticate
    let mut config = Config::load()?;
    let token_store = TokenStore::new()?;
    let auth = match mode {
        Mode::Terminal => match authenticate(&token_store, &config).await? {
            Some(auth) => auth,
            None => return Ok(()),
        },
        // Nobody is there to answer the first-run questions.
        Mode::Headless { .. } => {
            if !auth::has_credentials(&token_store, &config)? {
                anyhow::bail!(
                    "Not signed in. Run `linear-tui auth login` (or `linear-tui auth token <key>`) first."
                );
            }
            auth::resolve_auth(&token_store, config.auth.api_key.as_deref()).await?
        }
    };
    tracing::info!(method = auth.label(), "authenticated successfully");
    let organization = auth.organization().map(|org| org.id.clone());

    // An entry for this directory in config.toml wins over the view
    // remembered for it.
    let origin = snapshot::this_process(std::env::current_dir()?);
    let pinned = config.workspace(&origin.workspace, &origin.cwd).cloned();
    let shelf = snapshot::state_dir()
        .map(|dir| snapshot::Shelf::new(&dir, &origin.workspace))
        .inspect_err(|e| tracing::warn!("snapshots disabled: {e:#}"))
        .ok();
    let restored = match &pinned {
        Some(entry) => {
            if entry.team.is_some() {
                config.ui.default_team.clone_from(&entry.team);
            }
            None
        }
        // Asked for an issue, the remembered view is not reopened.
        None if open.is_some() => None,
        None => shelf
            .as_ref()
            .and_then(|s| reopen(s, &origin, organization.as_ref())),
    };
    let control_file = shelf
        .as_ref()
        .filter(|_| config.agent.control)
        .map(|s| control::endpoint_path(&s.path_for(origin.pid)));
    let recorder = shelf.as_ref().map(|shelf| {
        shelf.prune();
        snapshot::Recorder::new(shelf, origin.pid)
    });

    let client = LinearClient::new(auth.into_credentials(token_store.clone()));
    let app = new_app(&config, &token_store, restored, open.as_deref());
    let workspace = origin.workspace.clone();
    let mut runtime = Runtime::new(app, client, origin.clone(), recorder);

    // Agents drive this instance through the control channel.
    let endpoint = match control_file {
        Some(path) => match control::listen(path).await {
            Ok((commands, file)) => {
                runtime.accept_control(commands);
                Some(file)
            }
            Err(e) => {
                tracing::warn!("control channel disabled: {e:#}");
                None
            }
        },
        None => None,
    };

    let mut switcher = Switcher {
        config,
        token_store,
        origin,
        shelf,
        pinned: pinned.is_some(),
        organization,
        left: HashMap::new(),
    };
    let result = match mode {
        Mode::Terminal => run_in_terminal(&mut runtime, &mut switcher).await,
        Mode::Headless { size } => {
            if endpoint.is_none() {
                anyhow::bail!(
                    "A headless linear-tui needs its control channel: set `[agent] control = true`."
                );
            }
            println!(
                "linear-tui is running headless (pid {}) for {}; drive it with `linear-tui tui …` there.",
                std::process::id(),
                workspace.display()
            );
            run_headless(&mut runtime, size, &mut switcher).await
        }
    };
    // The control file goes once the instance stops.
    drop(endpoint);
    result
}

/// `--headless [--size <W>x<H>]`.
fn headless_mode(args: &[String]) -> Result<Mode> {
    let usage = "Usage: linear-tui --headless [--size <width>x<height>]";
    let size = match args {
        [] => HEADLESS_SIZE,
        [flag, size] if flag == "--size" => {
            let (w, h) = size.split_once('x').context(usage)?;
            (w.parse().context(usage)?, h.parse().context(usage)?)
        }
        _ => anyhow::bail!("{usage}"),
    };
    Ok(Mode::Headless { size })
}

/// Resolve stored credentials, running first-run setup when there are none.
/// `None` means the user backed out of setup, which ends the run quietly.
async fn authenticate(
    token_store: &TokenStore,
    config: &Config,
) -> Result<Option<auth::AuthMethod>> {
    if auth::has_credentials(token_store, config)? {
        return auth::resolve_auth(token_store, config.auth.api_key.as_deref())
            .await
            .map(Some);
    }

    if !auth::setup::run(token_store).await? {
        return Ok(None);
    }

    // Setup may have written an API key, so the file on disk is newer than ours.
    let config = Config::load()?;
    auth::resolve_auth(token_store, config.auth.api_key.as_deref())
        .await
        .map(Some)
}

/// The view a launch in `origin`'s repository reopens, signed in to
/// `organization`: see `usecase::instance::to_reopen`.
fn reopen(
    shelf: &snapshot::Shelf,
    origin: &entity::Origin,
    organization: Option<&OrganizationId>,
) -> Option<entity::snapshot::ViewSnapshot> {
    let instances = shelf.instances();
    usecase::instance::to_reopen(&instances, &origin.id(), organization).map(|i| i.view.clone())
}

/// A fresh session's state, reopening `restored` or the issue `open` names.
fn new_app(
    config: &Config,
    token_store: &TokenStore,
    restored: Option<entity::snapshot::ViewSnapshot>,
    open: Option<&str>,
) -> App {
    // `App::new` seeds the initial Teams/Viewer requests.
    let mut app = App::new(config);
    // Inside herdr, the actions that hand work to its plugin are offered.
    app.herdr = herdr::available();
    app.workspaces = workspace_entries(token_store);
    if let Some(snapshot) = restored {
        tracing::info!(updated_at = %snapshot.updated_at, "restoring the last view");
        app.restore(snapshot);
    }
    if let Some(identifier) = open {
        app.open_on_launch(identifier);
    }
    app
}

/// The signed-in workspaces the switcher offers: see
/// `usecase::workspace::signed_in`.
fn workspace_entries(token_store: &TokenStore) -> Vec<entity::Workspace> {
    let Ok(accounts) = token_store.load() else {
        return Vec::new();
    };
    usecase::workspace::signed_in(
        accounts
            .accounts
            .iter()
            .map(|account| (account.organization.clone(), accounts.is_current(account))),
    )
}

/// Moving between workspaces: switching ends one session and starts another
/// on the same screen, with that workspace's credentials and a fresh app.
struct Switcher {
    config: Config,
    token_store: TokenStore,
    origin: entity::Origin,
    /// Where a view to reopen is looked for after switching workspace.
    shelf: Option<snapshot::Shelf>,
    /// Whether config.toml pins this directory, which reopens no view.
    pinned: bool,
    /// The workspace the session in progress acts in.
    organization: Option<OrganizationId>,
    /// The view each workspace was left on this run, to come back to.
    left: usecase::workspace::LeftViews,
}

impl Switcher {
    /// Start the session the user asked for, if they picked a workspace.
    /// Returns whether linear-tui keeps running: false once they quit.
    async fn switch(&mut self, runtime: &mut Runtime) -> bool {
        let Some(target) = runtime.app.switch_to.take() else {
            return false;
        };
        // The one await on the UI's path: a token past its expiry is renewed
        // before the new session can send anything. The status line says so.
        let api_key = self.config.auth.api_key.as_deref();
        match auth::switch_to(&self.token_store, api_key, &target).await {
            Ok(auth) => {
                tracing::info!(organization = %target, "switched workspace");
                if let Some(org) = self.organization.take() {
                    let view = runtime
                        .app
                        .snapshot(&self.origin, snapshot::timestamp_now());
                    usecase::workspace::leave(&mut self.left, org, view);
                }
                self.organization = Some(target.clone());
                let client = LinearClient::new(auth.into_credentials(self.token_store.clone()));
                let (shelf, origin) = (&self.shelf, &self.origin);
                let restored = usecase::workspace::view_on_return(
                    &mut self.left,
                    &target,
                    self.pinned,
                    || {
                        shelf
                            .as_ref()
                            .and_then(|s| reopen(s, origin, Some(&target)))
                    },
                );
                let app = new_app(&self.config, &self.token_store, restored, None);
                runtime.restart(app, client);
            }
            Err(e) => {
                tracing::warn!(error = %e, "could not switch workspace");
                runtime
                    .app
                    .set_error(format!("Could not switch workspace: {e:#}"));
            }
        }
        true
    }
}

/// RAII guard so the terminal is restored even if the loop returns an error.
struct TerminalGuard {
    /// Whether the kitty keyboard protocol was successfully enabled.
    enhanced_keys: bool,
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;

        // Linear binds actions to Ctrl+punctuation (copy ID, copy branch name)
        // and to Ctrl+M, none of which a legacy terminal can distinguish. The
        // kitty keyboard protocol reports them as distinct events; terminals
        // without it fall back to the plain-key aliases in `keys.rs`.
        let enhanced_keys = supports_keyboard_enhancement().unwrap_or(false);
        if enhanced_keys {
            execute!(
                io::stdout(),
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )?;
        }

        Ok(Self { enhanced_keys })
    }

    /// Hand the terminal back as it was, for a program of the user's.
    fn suspend(&self) {
        if self.enhanced_keys {
            let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
        }
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }

    /// Take the terminal again after [`Self::suspend`].
    fn resume(&self) -> Result<()> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        if self.enhanced_keys {
            execute!(
                io::stdout(),
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )?;
        }
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.suspend();
    }
}

/// Write a description in `$EDITOR`, lending it the terminal, and hand what
/// was saved to the app.
fn edit_in_editor<B>(
    runtime: &mut Runtime,
    guard: &TerminalGuard,
    terminal: &mut Terminal<B>,
    job: app::EditorJob,
) -> Result<()>
where
    B: Backend,
    B::Error: Send + Sync + 'static,
{
    guard.suspend();
    let edited = infra::editor::edit(&job.text);
    guard.resume()?;
    terminal.clear()?;
    match edited {
        Ok(text) => runtime.app.description_edited(&job.issue_id, text),
        Err(e) => runtime
            .app
            .set_error(format!("Could not open your editor: {e:#}")),
    }
    runtime.touch();
    Ok(())
}

/// Whether a session is over: the user quit, or picked another workspace.
/// A switch is drawn first, so "Switching to …" shows while the next session
/// is prepared.
fn session_over<B>(runtime: &mut Runtime, terminal: &mut Terminal<B>) -> Result<bool>
where
    B: Backend,
    B::Error: Send + Sync + 'static,
{
    if runtime.app.switch_to.is_some() {
        runtime.touch();
        runtime.draw(terminal)?;
        return Ok(true);
    }
    Ok(runtime.app.should_quit)
}

/// The TUI in this terminal, until the user quits.
async fn run_in_terminal(runtime: &mut Runtime, switcher: &mut Switcher) -> Result<()> {
    let guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.hide_cursor()?;

    loop {
        // There is a terminal to lend $EDITOR, in every session.
        runtime.app.external_editor = true;
        loop {
            runtime.send_queued(Instant::now());
            runtime.draw(&mut terminal)?;
            runtime.answer_waiting(Instant::now());

            // Drain completed requests without blocking.
            let mut moved = runtime.receive();

            if let Some(text) = runtime.app.outbox.clipboard.take() {
                copy_to_clipboard(&text)?;
            }

            moved |= event::poll_and_handle(&mut runtime.app)?;
            moved |= runtime.serve_control();
            if let Some(job) = runtime.app.outbox.editor.take() {
                edit_in_editor(runtime, &guard, &mut terminal, job)?;
                moved = true;
            }
            runtime.settle(moved, Instant::now());

            if session_over(runtime, &mut terminal)? {
                break;
            }
        }
        if !switcher.switch(runtime).await {
            break;
        }
    }

    runtime.close();
    Ok(())
}

/// How often a headless instance looks for work when it has none.
const HEADLESS_IDLE: Duration = Duration::from_millis(20);

/// The TUI with no terminal, until an agent quits it or the process is
/// interrupted.
async fn run_headless(
    runtime: &mut Runtime,
    size: (u16, u16),
    switcher: &mut Switcher,
) -> Result<()> {
    runtime.hold_external();
    let mut terminal = Terminal::new(TestBackend::new(size.0, size.1))?;
    let interrupted = tokio::signal::ctrl_c();
    tokio::pin!(interrupted);

    'run: loop {
        loop {
            runtime.send_queued(Instant::now());
            runtime.draw(&mut terminal)?;
            runtime.answer_waiting(Instant::now());

            let mut moved = runtime.receive();
            moved |= runtime.serve_control();
            // Noted in the same pass as the key that copied, so the answer to
            // that key already says so.
            if let Some(text) = runtime.app.outbox.clipboard.take() {
                runtime.note_held(format!("would copy {text:?} to the clipboard"));
            }
            runtime.settle(moved, Instant::now());

            if session_over(runtime, &mut terminal)? {
                break;
            }
            tokio::select! {
                _ = tokio::time::sleep(HEADLESS_IDLE) => {}
                _ = &mut interrupted => break 'run,
            }
        }
        if !switcher.switch(runtime).await {
            break;
        }
    }

    runtime.close();
    Ok(())
}

/// Push `text` to the system clipboard with an OSC 52 escape sequence.
///
/// This goes through the terminal rather than a platform clipboard API, so it
/// also works over SSH. Terminals that disable OSC 52 will simply ignore it,
/// and tmux needs `set -g set-clipboard on`.
fn copy_to_clipboard(text: &str) -> Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let mut stdout = io::stdout();
    write!(stdout, "\x1b]52;c;{encoded}\x07")?;
    stdout.flush()?;
    Ok(())
}
