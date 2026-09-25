mod api;
mod app;
mod auth;
mod cli;
mod config;
mod dispatch;
mod entity;
mod event;
mod fuzzy;
mod grouping;
mod herdr;
mod keys;
mod logging;
mod look;
mod message;
mod palette;
mod private_file;
mod snapshot;
mod store;
mod ui;
mod usecase;

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
use entity::InstanceId;
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
    let client = LinearClient::new(auth.into_credentials(token_store));

    // An entry for this directory in config.toml wins over the view
    // remembered for it.
    let origin = snapshot::Origin::current(std::env::current_dir()?);
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
            let me = InstanceId {
                workspace: origin.workspace.clone(),
                pid: origin.pid,
            };
            let instances = s.instances();
            usecase::instance::to_reopen(&instances, &me).map(|i| i.view.clone())
        }),
    };
    let recorder = shelf.map(|shelf| {
        shelf.prune();
        snapshot::Recorder::new(&shelf, origin.pid)
    });

    // Run TUI
    run_tui(
        client,
        config,
        Session {
            origin,
            restored,
            open,
            recorder,
        },
    )
    .await
}

/// Where this instance runs, what it reopens, and where it records its view.
struct Session {
    origin: snapshot::Origin,
    restored: Option<entity::snapshot::ViewSnapshot>,
    /// The issue `linear-tui open` asked for.
    open: Option<String>,
    recorder: Option<snapshot::Recorder>,
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

async fn run_tui(client: LinearClient, config: Config, session: Session) -> Result<()> {
    let Session {
        origin,
        restored,
        open,
        mut recorder,
    } = session;
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let client = Arc::new(client);
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // `App::new` seeds the initial Teams/Viewer requests.
    let mut app = App::new(&config);
    // Only the herdr plugin writes the agents file.
    let mut agents = app.herdr.then(herdr::AgentWatch::new).flatten();
    if let Some(snapshot) = restored {
        tracing::info!(updated_at = %snapshot.updated_at, "restoring the last view");
        app.restore(snapshot);
    }
    if let Some(identifier) = &open {
        app.open_on_launch(identifier);
    }
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
            let client = Arc::clone(&client);
            let tx = tx.clone();
            let per_page = app.items_per_page;
            tokio::spawn(async move {
                let msg = dispatch::execute_request(&client, req, per_page).await;
                let _ = tx.send(msg);
            });
            dirty = true;
        }

        if dirty {
            terminal.draw(|f| ui::draw(f, &mut app, &mut cache))?;
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

        if event::poll_and_handle(&mut app)? {
            moved = true;
        }

        // Record where the user is once the view has rested. This is the
        // only place a snapshot is written — never from rendering.
        let now = Instant::now();
        if let Some(recorder) = &mut recorder {
            if moved {
                recorder.touch(now);
            }
            if recorder.is_due(now)
                && let Some(snapshot) = app.snapshot(&origin)
            {
                recorder.record(snapshot);
            }
        }
        dirty |= moved;

        // What the herdr plugin says its agents are working on.
        if let Some(watch) = &mut agents
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
            break;
        }
    }

    if let Some(recorder) = &mut recorder
        && let Some(snapshot) = app.snapshot(&origin)
    {
        recorder.close(snapshot);
    }
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
