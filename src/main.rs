mod api;
mod app;
mod auth;
mod config;
mod event;
mod keys;
mod logging;
mod message;
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
    let auth = auth::resolve_auth(&token_store, config.auth.api_key.as_deref()).await?;
    tracing::info!("authenticated successfully");
    let client = LinearClient::new(auth.authorization_header());

    // Run TUI
    run_tui(client, config).await
}

async fn handle_subcommand(args: &[String]) -> Result<()> {
    match args[0].as_str() {
        "auth" => {
            if args.len() < 2 {
                println!("Usage: linear-tui auth <login|set-oauth|token|logout>");
                return Ok(());
            }
            let token_store = TokenStore::new()?;
            match args[1].as_str() {
                "login" => auth::oauth::login(&token_store).await?,
                "set-oauth" => {
                    if args.len() < 4 {
                        println!("Usage: linear-tui auth set-oauth <client-id> <client-secret>");
                        return Ok(());
                    }
                    let mut config = Config::load()?;
                    config.auth.oauth_client_id = Some(args[2].clone());
                    config.auth.oauth_client_secret = Some(args[3].clone());
                    config.save()?;
                    println!("OAuth credentials saved.");
                }
                "token" => {
                    if args.len() < 3 {
                        println!("Usage: linear-tui auth token <api-key>");
                        return Ok(());
                    }
                    let mut config = Config::load()?;
                    config.auth.api_key = Some(args[2].clone());
                    config.save()?;
                    println!("API key saved.");
                }
                "logout" => {
                    token_store.clear()?;
                    println!("Logged out.");
                }
                _ => println!("Unknown auth command: {}", args[1]),
            }
            Ok(())
        }
        _ => {
            println!("Unknown command: {}", args[0]);
            println!("Usage: linear-tui [auth login|auth set-oauth|auth token <key>|auth logout]");
            Ok(())
        }
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
async fn execute_request(client: &LinearClient, req: Request, per_page: u32) -> Message {
    match req {
        Request::Teams => match client.teams().await {
            Ok(teams) => Message::Teams(teams),
            Err(e) => Message::Error(format!("Failed to load teams: {e}")),
        },
        Request::Viewer => match client.viewer().await {
            Ok(viewer) => Message::Viewer(viewer.id),
            Err(e) => Message::Error(format!("Failed to identify current user: {e}")),
        },
        Request::TeamContext { team_id } => {
            // Independent queries — fetch them concurrently.
            let (states, members) = tokio::join!(
                client.workflow_states(&team_id),
                client.team_members(&team_id)
            );
            match (states, members) {
                (Ok(states), Ok(members)) => Message::TeamContext { states, members },
                (Err(e), _) | (_, Err(e)) => {
                    Message::Error(format!("Failed to load team context: {e}"))
                }
            }
        }
        Request::Issues { team_id, after } => {
            let append = after.is_some();
            match client.issues(&team_id, after.as_deref(), per_page).await {
                Ok((issues, info)) => Message::Issues(Page::new(issues, info, append)),
                Err(e) => Message::Error(format!("Failed to load issues: {e}")),
            }
        }
        Request::MyIssues { user_id, after } => {
            let append = after.is_some();
            match client.my_issues(&user_id, after.as_deref(), per_page).await {
                Ok((issues, info)) => Message::MyIssues(Page::new(issues, info, append)),
                Err(e) => Message::Error(format!("Failed to load my issues: {e}")),
            }
        }
        Request::Search { term, team_id } => {
            match client
                .search_issues(&term, team_id.as_deref(), per_page)
                .await
            {
                Ok((issues, _)) => Message::SearchResults { term, issues },
                Err(e) => Message::Error(format!("Search failed: {e}")),
            }
        }
        Request::Projects { team_id, after } => {
            let append = after.is_some();
            match client.projects(&team_id, after.as_deref()).await {
                Ok((projects, info)) => Message::Projects(Page::new(projects, info, append)),
                Err(e) => Message::Error(format!("Failed to load projects: {e}")),
            }
        }
        Request::Cycles { team_id, after } => {
            let append = after.is_some();
            match client.cycles(&team_id, after.as_deref()).await {
                Ok((cycles, info)) => Message::Cycles(Page::new(cycles, info, append)),
                Err(e) => Message::Error(format!("Failed to load cycles: {e}")),
            }
        }
        Request::IssueDetail { issue_id } => match client.issue_detail(&issue_id).await {
            Ok(issue) => Message::IssueDetail(Box::new(issue)),
            Err(e) => Message::Error(format!("Failed to load detail: {e}")),
        },
        Request::ProjectIssues { project_id, after } => {
            let append = after.is_some();
            match client.project_issues(&project_id, after.as_deref()).await {
                Ok((issues, info)) => Message::ProjectIssues(Page::new(issues, info, append)),
                Err(e) => Message::Error(format!("Failed to load project issues: {e}")),
            }
        }
        Request::CycleIssues { cycle_id, after } => {
            let append = after.is_some();
            match client.cycle_issues(&cycle_id, after.as_deref()).await {
                Ok((issues, info)) => Message::CycleIssues(Page::new(issues, info, append)),
                Err(e) => Message::Error(format!("Failed to load cycle issues: {e}")),
            }
        }
        Request::UpdateStatus { issue_id, state_id } => {
            match client.update_issue_state(&issue_id, &state_id).await {
                Ok(()) => Message::Mutated("Status updated"),
                Err(e) => Message::Error(format!("Failed to update status: {e}")),
            }
        }
        Request::UpdatePriority { issue_id, priority } => {
            match client.update_issue_priority(&issue_id, priority).await {
                Ok(()) => Message::Mutated("Priority updated"),
                Err(e) => Message::Error(format!("Failed to update priority: {e}")),
            }
        }
        Request::UpdateAssignee {
            issue_id,
            assignee_id,
        } => {
            match client
                .update_issue_assignee(&issue_id, assignee_id.as_deref())
                .await
            {
                Ok(()) => Message::Mutated("Assignee updated"),
                Err(e) => Message::Error(format!("Failed to update assignee: {e}")),
            }
        }
        Request::CreateComment { issue_id, body } => {
            match client.create_comment(&issue_id, &body).await {
                Ok(()) => Message::Mutated("Comment posted"),
                Err(e) => Message::Error(format!("Failed to post comment: {e}")),
            }
        }
        Request::CreateIssue {
            team_id,
            title,
            description,
            priority,
        } => {
            match client
                .create_issue(&team_id, &title, description.as_deref(), priority)
                .await
            {
                Ok(issue) => Message::IssueCreated(Box::new(issue)),
                Err(e) => Message::Error(format!("Failed to create issue: {e}")),
            }
        }
        Request::OpenUrl(url) => match open::that_detached(&url) {
            Ok(()) => Message::Mutated("Opened in browser"),
            Err(e) => Message::Error(format!("Failed to open browser: {e}")),
        },
    }
}
