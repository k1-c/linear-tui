mod api;
mod app;
mod auth;
mod config;
mod event;
mod grouping;
mod keys;
mod logging;
mod message;
mod private_file;
mod ui;

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
use message::{Message, Page, Request};

/// Spinner advance interval.
const TICK: Duration = Duration::from_millis(80);

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = logging::init();

    let args: Vec<String> = std::env::args().collect();

    // Handle CLI subcommands
    if args.len() > 1 {
        return handle_subcommand(&args[1..]).await;
    }

    // Load config and authenticate
    let config = Config::load()?;
    let token_store = TokenStore::new()?;
    let Some(auth) = authenticate(&token_store, &config).await? else {
        return Ok(());
    };
    tracing::info!(method = auth.label(), "authenticated successfully");
    let client = LinearClient::with_header(auth.authorization_header());

    // Run TUI
    run_tui(client, config).await
}

const USAGE: &str = "\
linear-tui — a terminal UI for Linear

Usage:
  linear-tui                     Open the TUI (sets up credentials on first run)
  linear-tui auth login          Sign in through the browser
  linear-tui auth status         Show which credentials are in use
  linear-tui auth token <key>    Sign in with a personal API key
  linear-tui auth logout [--all] Forget the stored token (--all: the API key too)
  linear-tui auth set-oauth <client-id> [client-secret]
                                 Authorize against your own Linear application
";

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

async fn handle_subcommand(args: &[String]) -> Result<()> {
    match args[0].as_str() {
        "auth" => handle_auth(&args[1..]).await,
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            Ok(())
        }
        "--version" | "-V" => {
            println!("linear-tui {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        other => {
            eprint!("{USAGE}");
            anyhow::bail!("unknown command: {other}")
        }
    }
}

async fn handle_auth(args: &[String]) -> Result<()> {
    let token_store = TokenStore::new()?;

    match args.first().map(String::as_str) {
        Some("login") => {
            auth::oauth::login(&token_store).await?;
            let method = auth::resolve_auth(&token_store, None).await?;
            println!("Signed in as {}.", auth::identify(&method).await?.name);
            Ok(())
        }

        Some("status") => auth_status(&token_store).await,

        Some("token") => {
            let Some(key) = args.get(1) else {
                anyhow::bail!("Usage: linear-tui auth token <api-key>");
            };
            // Verified before it is written, so a typo fails here and not at
            // the next launch.
            let name = auth::setup::save_api_key(key).await?;
            println!("API key saved. Signed in as {name}.");
            Ok(())
        }

        Some("set-oauth") => {
            let Some(client_id) = args.get(1) else {
                anyhow::bail!("Usage: linear-tui auth set-oauth <client-id> [client-secret]");
            };
            let mut config = Config::load()?;
            config.auth.oauth_client_id = Some(client_id.clone());
            config.auth.oauth_client_secret = args.get(2).cloned();
            config.save()?;

            println!("Logins will now use your application ({client_id}).");
            println!("Register these redirect URIs on it:");
            for port in auth::oauth::callback_ports() {
                println!("  http://localhost:{port}/callback");
            }
            Ok(())
        }

        Some("logout") => {
            let clear_key = args.iter().any(|arg| arg == "--all");
            token_store.clear()?;

            let mut config = Config::load()?;
            if clear_key {
                config.auth.api_key = None;
                config.save()?;
                println!("Logged out. Stored token and API key removed.");
            } else if config.auth.api_key.is_some() {
                println!(
                    "Signed out of OAuth. An API key is still set in config.toml — \
                     `linear-tui auth logout --all` removes that too."
                );
            } else {
                println!("Logged out.");
            }
            Ok(())
        }

        _ => {
            print!("{USAGE}");
            Ok(())
        }
    }
}

async fn auth_status(token_store: &TokenStore) -> Result<()> {
    let config = Config::load()?;

    println!("Application  {}", auth::oauth::application_summary()?);
    match token_store.load()? {
        Some(tokens) => {
            let remaining = tokens.seconds_until_expiry();
            if remaining > 0 {
                println!("OAuth token  valid for {}", format_duration(remaining));
            } else {
                println!(
                    "OAuth token  expired {} ago, refreshes on next use",
                    format_duration(-remaining)
                );
            }
            println!("Stored in    {}", token_store.path().display());
        }
        None => println!("OAuth token  none"),
    }
    println!(
        "API key      {}",
        match config.auth.api_key {
            Some(_) => "set in config.toml",
            None => "none",
        }
    );

    println!();
    match auth::resolve_auth(token_store, config.auth.api_key.as_deref()).await {
        Ok(method) => match auth::identify(&method).await {
            Ok(viewer) => println!("Signed in as {} via {}.", viewer.name, method.label()),
            Err(e) => println!("Stored credentials were rejected by the API: {e}"),
        },
        Err(e) => println!("{e}"),
    }
    Ok(())
}

fn format_duration(seconds: i64) -> String {
    match seconds {
        s if s >= 3600 => format!("{}h {}m", s / 3600, (s % 3600) / 60),
        s if s >= 60 => format!("{}m", s / 60),
        s => format!("{s}s"),
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

async fn run_tui(client: LinearClient, config: Config) -> Result<()> {
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let client = Arc::new(client);
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // `App::new` seeds the initial Teams/Viewer requests.
    let mut app = App::new(&config);
    let mut last_tick = Instant::now();
    let mut dirty = true;

    loop {
        // Spawn everything the UI has queued since the last pass. Each request
        // runs on the tokio runtime, so the UI never blocks on the network.
        while let Some(req) = app.requests.pop_front() {
            app.inflight += 1;
            let client = Arc::clone(&client);
            let tx = tx.clone();
            let per_page = app.items_per_page;
            tokio::spawn(async move {
                let msg = execute_request(&client, req, per_page).await;
                let _ = tx.send(msg);
            });
            dirty = true;
        }

        if dirty {
            terminal.draw(|f| ui::draw(f, &mut app))?;
            dirty = false;
        }

        // Drain completed requests without blocking.
        while let Ok(msg) = rx.try_recv() {
            app.inflight = app.inflight.saturating_sub(1);
            app.handle_message(msg);
            dirty = true;
        }

        if let Some(text) = app.pending_clipboard.take() {
            copy_to_clipboard(&text)?;
        }

        if event::poll_and_handle(&mut app)? {
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

/// Run one request against the API and turn the outcome into a [`Message`].
///
/// A failure comes back as [`Message::Failed`] carrying the request, so the app
/// can undo exactly what that request stood for.
async fn execute_request(client: &LinearClient, req: Request, per_page: u32) -> Message {
    match run_request(client, &req, per_page).await {
        Ok(msg) => msg,
        Err(e) => Message::Failed {
            request: Box::new(req),
            error: e.to_string(),
        },
    }
}

async fn run_request(client: &LinearClient, req: &Request, per_page: u32) -> Result<Message> {
    let append = req.cursor().is_some();
    Ok(match req {
        Request::Teams => Message::Teams(client.teams().await?),
        Request::Viewer => Message::Viewer(client.viewer().await?.id),
        Request::TeamContext { team_id } => {
            // Independent queries — fetch them concurrently.
            let (states, members) = tokio::join!(
                client.workflow_states(team_id),
                client.team_members(team_id)
            );
            Message::TeamContext {
                team_id: team_id.clone(),
                states: states?,
                members: members?,
            }
        }
        Request::Issues {
            team_id,
            after,
            preset,
        } => {
            let (issues, info) = client
                .issues(team_id, preset.state_filter(), after.as_deref(), per_page)
                .await?;
            Message::Issues {
                team_id: team_id.clone(),
                preset: *preset,
                page: Page::new(issues, info, append),
            }
        }
        Request::MyIssues { user_id, after } => {
            let (issues, info) = client
                .my_issues(user_id, after.as_deref(), per_page)
                .await?;
            Message::MyIssues(Page::new(issues, info, append))
        }
        Request::Search { term, team_id } => {
            let (issues, _) = client
                .search_issues(term, team_id.as_ref(), per_page)
                .await?;
            Message::SearchResults {
                term: term.clone(),
                team_id: team_id.clone(),
                issues,
            }
        }
        Request::CustomViews => Message::CustomViews(client.custom_views().await?),
        Request::Favorites => Message::Favorites(client.favorites().await?),
        Request::ViewIssues { view_id, after } => {
            let (issues, info) = client
                .custom_view_issues(view_id, after.as_deref(), per_page)
                .await?;
            Message::ViewIssues {
                view_id: view_id.clone(),
                page: Page::new(issues, info, append),
            }
        }
        Request::ViewProjects { view_id, after } => {
            let (projects, info) = client
                .custom_view_projects(view_id, after.as_deref())
                .await?;
            Message::ViewProjects {
                view_id: view_id.clone(),
                page: Page::new(projects, info, append),
            }
        }
        Request::Projects { team_id, after } => {
            let (projects, info) = client.projects(team_id, after.as_deref()).await?;
            Message::Projects {
                team_id: team_id.clone(),
                page: Page::new(projects, info, append),
            }
        }
        Request::Cycles { team_id, after } => {
            let (cycles, info) = client.cycles(team_id, after.as_deref()).await?;
            Message::Cycles {
                team_id: team_id.clone(),
                page: Page::new(cycles, info, append),
            }
        }
        Request::IssueDetail { issue_id } => {
            Message::IssueDetail(Box::new(client.issue_detail(issue_id).await?))
        }
        Request::ProjectIssues { project_id, after } => {
            let (issues, info) = client.project_issues(project_id, after.as_deref()).await?;
            Message::ProjectIssues {
                project_id: project_id.clone(),
                page: Page::new(issues, info, append),
            }
        }
        Request::CycleIssues { cycle_id, after } => {
            let (issues, info) = client.cycle_issues(cycle_id, after.as_deref()).await?;
            Message::CycleIssues {
                cycle_id: cycle_id.clone(),
                page: Page::new(issues, info, append),
            }
        }
        Request::UpdateStatus { issue_id, state_id } => {
            client.update_issue_state(issue_id, state_id).await?;
            Message::Mutated("Status updated")
        }
        Request::UpdatePriority { issue_id, priority } => {
            client.update_issue_priority(issue_id, *priority).await?;
            Message::Mutated("Priority updated")
        }
        Request::UpdateAssignee {
            issue_id,
            assignee_id,
        } => {
            client
                .update_issue_assignee(issue_id, assignee_id.as_ref())
                .await?;
            Message::Mutated("Assignee updated")
        }
        Request::CreateComment { issue_id, body } => {
            client.create_comment(issue_id, body).await?;
            Message::Mutated("Comment posted")
        }
        Request::CreateIssue {
            team_id,
            title,
            description,
            priority,
        } => {
            let issue = client
                .create_issue(team_id, title, description.as_deref(), *priority)
                .await?;
            Message::IssueCreated {
                team_id: team_id.clone(),
                issue: Box::new(issue),
            }
        }
        Request::OpenUrl(url) => {
            open::that_detached(url)?;
            Message::Mutated("Opened in browser")
        }
    })
}
