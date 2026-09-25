mod adapter;
mod config;
mod entity;
mod interface;
mod logging;
mod store;
mod usecase;

use adapter::{api, auth, cli, dispatch, herdr, snapshot};
use interface::{app, event, message, ui};

use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
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
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;

use base64::Engine;

use api::client::LinearClient;
use app::App;
use auth::token::TokenStore;
use config::Config;
use entity::OrganizationId;
use message::Message;

/// Spinner advance interval.
const TICK: Duration = Duration::from_millis(80);

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = logging::init();

    let args: Vec<String> = std::env::args().collect();

    // `open <ID>` runs the TUI; every other subcommand runs without it.
    let open = match args.get(1).map(String::as_str) {
        Some("open") => {
            let [_, _, key] = args.as_slice() else {
                anyhow::bail!("Usage: linear-tui open <ID>");
            };
            Some(cli::issue_key(key)?)
        }
        Some(_) => return cli::handle_subcommand(&args[1..]).await,
        None => None,
    };

    // Load config and authenticate
    let mut config = Config::load()?;
    let token_store = TokenStore::new()?;
    let Some(auth) = authenticate(&token_store, &config).await? else {
        return Ok(());
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
    let recorder = shelf.as_ref().map(|shelf| {
        shelf.prune();
        snapshot::Recorder::new(shelf, origin.pid)
    });

    // Run TUI
    run_tui(
        config,
        token_store,
        auth,
        Session {
            origin,
            restored,
            open,
            recorder,
            shelf,
            pinned: pinned.is_some(),
        },
    )
    .await
}

/// Where this instance runs, what it reopens, and where it records its view.
struct Session {
    origin: entity::Origin,
    restored: Option<entity::snapshot::ViewSnapshot>,
    /// The issue `linear-tui open` asked for.
    open: Option<String>,
    recorder: Option<snapshot::Recorder>,
    /// Where a view to reopen is looked for after switching workspace.
    shelf: Option<snapshot::Shelf>,
    /// Whether config.toml pins this directory, which reopens no view.
    pinned: bool,
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

type Term = Terminal<CrosstermBackend<io::Stdout>>;

/// Run the TUI until it quits. Switching workspace ends one session and
/// starts another on the same terminal: a new client, and a new `App`, since
/// nothing loaded from one workspace means anything in the next.
async fn run_tui(
    config: Config,
    token_store: TokenStore,
    auth: auth::AuthMethod,
    session: Session,
) -> Result<()> {
    let Session {
        origin,
        restored,
        open,
        mut recorder,
        shelf,
        pinned,
    } = session;
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    // Only the herdr plugin writes the agents file.
    let mut agents = herdr::available().then(herdr::AgentWatch::new).flatten();
    // The view each workspace was left on this run, to come back to.
    let mut left: HashMap<OrganizationId, entity::snapshot::ViewSnapshot> = HashMap::new();
    let mut organization = auth.organization().map(|org| org.id.clone());
    let mut client = Arc::new(LinearClient::new(
        auth.into_credentials(token_store.clone()),
    ));
    let mut app = new_app(&config, &token_store, restored, open.as_deref());

    loop {
        run_session(
            &mut terminal,
            &mut app,
            &client,
            &origin,
            &mut recorder,
            &mut agents,
        )?;
        let Some(target) = app.switch_to.take() else {
            break;
        };
        // The one await on the UI's path: a token past its expiry is renewed
        // before the new session can send anything. The status line says so.
        match auth::switch_to(&token_store, config.auth.api_key.as_deref(), &target).await {
            Ok(auth) => {
                tracing::info!(organization = %target, "switched workspace");
                if let (Some(org), Some(view)) = (
                    organization.take(),
                    app.snapshot(&origin, snapshot::timestamp_now()),
                ) {
                    left.insert(org, view);
                }
                organization = Some(target.clone());
                client = Arc::new(LinearClient::new(
                    auth.into_credentials(token_store.clone()),
                ));
                let restored = if pinned {
                    None
                } else {
                    left.remove(&target).or_else(|| {
                        shelf
                            .as_ref()
                            .and_then(|s| reopen(s, &origin, Some(&target)))
                    })
                };
                app = new_app(&config, &token_store, restored, None);
            }
            Err(e) => {
                tracing::warn!(error = %e, "could not switch workspace");
                app.set_error(format!("Could not switch workspace: {e:#}"));
            }
        }
    }

    if let Some(recorder) = &mut recorder
        && let Some(snapshot) = app.snapshot(&origin, snapshot::timestamp_now())
    {
        recorder.close(snapshot);
    }
    Ok(())
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

/// The signed-in workspaces the switcher offers. One not yet identified has
/// nothing to show, so it is left out until the next launch names it.
fn workspace_entries(token_store: &TokenStore) -> Vec<app::WorkspaceEntry> {
    let Ok(accounts) = token_store.load() else {
        return Vec::new();
    };
    accounts
        .accounts
        .iter()
        .filter_map(|account| {
            let org = account.organization.as_ref()?;
            Some(app::WorkspaceEntry {
                id: org.id.clone(),
                name: org.name.clone(),
                url_key: org.url_key.clone(),
                current: accounts.is_current(account),
            })
        })
        .collect()
}

/// Drive one session until the user quits or picks another workspace.
fn run_session(
    terminal: &mut Term,
    app: &mut App,
    client: &Arc<LinearClient>,
    origin: &entity::Origin,
    recorder: &mut Option<snapshot::Recorder>,
    agents: &mut Option<herdr::AgentWatch>,
) -> Result<()> {
    // A channel per session: answers still on their way from the last one
    // have nowhere to land.
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let mut cache = ui::Cache::default();
    let mut last_tick = Instant::now();
    let mut dirty = true;

    loop {
        // A palette query that has rested long enough is searched now.
        if app.flush_palette_search(Instant::now()) {
            dirty = true;
        }

        // Spawn everything the UI has queued since the last pass. Each request
        // runs on the tokio runtime, so the UI never blocks on the network.
        while let Some(req) = app.outbox.requests.pop_front() {
            app.outbox.inflight += 1;
            let client = Arc::clone(client);
            let tx = tx.clone();
            let per_page = app.items_per_page;
            tokio::spawn(async move {
                let msg = dispatch::execute_request(&client, req, per_page).await;
                let _ = tx.send(msg);
            });
            dirty = true;
        }

        if dirty {
            terminal.draw(|f| ui::draw(f, app, &mut cache))?;
            dirty = false;
        }

        // Drain completed requests without blocking.
        let mut moved = false;
        while let Ok(msg) = rx.try_recv() {
            app.outbox.inflight = app.outbox.inflight.saturating_sub(1);
            app.handle_message(msg);
            moved = true;
        }

        if let Some(text) = app.outbox.clipboard.take() {
            copy_to_clipboard(&text)?;
        }

        if event::poll_and_handle(app)? {
            moved = true;
        }

        // Record where the user is once the view has rested. This is the
        // only place a snapshot is written — never from rendering.
        let now = Instant::now();
        if let Some(recorder) = recorder {
            if moved {
                recorder.touch(now);
            }
            if recorder.is_due(now)
                && let Some(snapshot) = app.snapshot(origin, snapshot::timestamp_now())
            {
                recorder.record(snapshot);
            }
        }
        dirty |= moved;

        // What the herdr plugin says its agents are working on.
        if let Some(watch) = agents
            && let Some(list) = watch.poll(now)
        {
            app.set_agents(list);
            dirty = true;
        }

        if app.loading() && last_tick.elapsed() >= TICK {
            app.tick_spinner();
            last_tick = Instant::now();
            dirty = true;
        }

        if app.should_quit {
            return Ok(());
        }
        if app.switch_to.is_some() {
            // Show "Switching to …" while the next session is prepared.
            terminal.draw(|f| ui::draw(f, app, &mut cache))?;
            return Ok(());
        }
    }
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
