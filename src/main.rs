mod adapter;
mod config;
mod entity;
mod interface;
mod logging;
mod store;
mod usecase;

use adapter::{api, auth, cli, control, herdr, snapshot};
use interface::{app, event};

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
    backend::{CrosstermBackend, TestBackend},
};

use base64::Engine;

use adapter::runtime::Runtime;
use api::client::LinearClient;
use app::App;
use auth::token::TokenStore;
use config::Config;

/// The size of the screen a headless instance draws, unless told otherwise.
const HEADLESS_SIZE: (u16, u16) = (120, 40);

/// How a launch runs the TUI.
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
        Some(_) => return cli::handle_subcommand(&args[1..]).await,
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
    let client = LinearClient::new(auth.into_credentials(token_store));

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
        None => shelf.as_ref().and_then(|s| {
            let instances = s.instances();
            usecase::instance::to_reopen(&instances, &origin.id()).map(|i| i.view.clone())
        }),
    };
    let control_file = shelf
        .as_ref()
        .filter(|_| config.agent.control)
        .map(|s| control::endpoint_path(&s.path_for(origin.pid)));
    let recorder = shelf.map(|shelf| {
        shelf.prune();
        snapshot::Recorder::new(&shelf, origin.pid)
    });

    // `App::new` seeds the initial Teams/Viewer requests.
    let mut app = App::new(&config);
    // Inside herdr, the actions that hand work to its plugin are offered.
    app.herdr = herdr::available();
    if let Some(snapshot) = restored {
        tracing::info!(updated_at = %snapshot.updated_at, "restoring the last view");
        app.restore(snapshot);
    }
    if let Some(identifier) = &open {
        app.open_on_launch(identifier);
    }
    let workspace = origin.workspace.clone();
    let mut runtime = Runtime::new(app, client, origin, recorder);

    // Agents drive this instance through the control channel.
    let _endpoint = match control_file {
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

    match mode {
        Mode::Terminal => run_in_terminal(&mut runtime),
        Mode::Headless { size } => {
            if _endpoint.is_none() {
                anyhow::bail!(
                    "A headless linear-tui needs its control channel: set `[agent] control = true`."
                );
            }
            println!(
                "linear-tui is running headless (pid {}) for {}; drive it with `linear-tui tui …` there.",
                std::process::id(),
                workspace.display()
            );
            run_headless(&mut runtime, size).await
        }
    }
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
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.enhanced_keys {
            let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
        }
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

/// The TUI in this terminal, until the user quits.
fn run_in_terminal(runtime: &mut Runtime) -> Result<()> {
    let _guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.hide_cursor()?;

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
        runtime.settle(moved, Instant::now());

        if runtime.app.should_quit {
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
async fn run_headless(runtime: &mut Runtime, size: (u16, u16)) -> Result<()> {
    runtime.hold_external();
    let mut terminal = Terminal::new(TestBackend::new(size.0, size.1))?;
    let interrupted = tokio::signal::ctrl_c();
    tokio::pin!(interrupted);

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

        if runtime.app.should_quit {
            break;
        }
        tokio::select! {
            _ = tokio::time::sleep(HEADLESS_IDLE) => {}
            _ = &mut interrupted => break,
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
