mod api;
mod app;
mod auth;
mod config;
mod event;
mod keys;
mod ui;

use std::io;

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use api::client::LinearClient;
use app::{App, PendingAction, Screen, Tab};
use auth::token::TokenStore;
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Handle CLI subcommands
    if args.len() > 1 {
        return handle_subcommand(&args[1..]).await;
    }

    // Load config and authenticate
    let config = Config::load()?;
    let token_store = TokenStore::new()?;
    let auth = auth::resolve_auth(&token_store, config.auth.api_key.as_deref()).await?;
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

async fn run_tui(client: LinearClient, config: Config) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let theme = config::Theme::from_name(config.ui.theme);
    let mut app = App::new(theme);
    let items_per_page = config.ui.items_per_page;

    // Load teams
    app.loading = true;
    match client.teams().await {
        Ok(teams) => {
            // Select default team if configured
            if let Some(default_team) = &config.ui.default_team
                && let Some(idx) = teams
                    .iter()
                    .position(|t| t.name == *default_team || t.key == *default_team)
            {
                app.selected_team_index = idx;
            }
            app.teams = teams;
        }
        Err(e) => app.set_error(format!("Failed to load teams: {e}")),
    }
    app.loading = false;

    // Fetch viewer (current user) for My Issues
    if let Ok(viewer) = client.viewer().await {
        app.viewer_id = Some(viewer.id);
    }

    // Load workflow states and team members for default team
    if let Some(team) = app.current_team() {
        let team_id = team.id.clone();
        if let Ok(states) = client.workflow_states(&team_id).await {
            app.workflow_states = states;
        }
        if let Ok(members) = client.team_members(&team_id).await {
            app.team_members = members;
        }
    }

    // Main loop
    loop {
        // Load data based on current tab
        if app.needs_reload {
            app.needs_reload = false;
            app.loading = true;
            terminal.draw(|f| ui::draw(f, &app))?;

            match app.tab {
                Tab::Issues => {
                    if let Some(team) = app.current_team() {
                        let team_id = team.id.clone();

                        // Load workflow states and members for new team
                        if let Ok(states) = client.workflow_states(&team_id).await {
                            app.workflow_states = states;
                        }
                        if let Ok(members) = client.team_members(&team_id).await {
                            app.team_members = members;
                        }

                        match client.issues(&team_id, None, items_per_page).await {
                            Ok((issues, page_info)) => {
                                app.issues = issues;
                                app.page_info = page_info;
                                app.filtered_issues.clear();
                                app.selected_issue_index = 0;
                                app.clear_status();
                            }
                            Err(e) => app.set_error(format!("Failed to load issues: {e}")),
                        }
                    }
                }
                Tab::MyIssues => {
                    if let Some(user_id) = &app.viewer_id {
                        let user_id = user_id.clone();
                        match client.my_issues(&user_id, None, items_per_page).await {
                            Ok((issues, page_info)) => {
                                app.my_issues = issues;
                                app.my_issues_page_info = page_info;
                                app.selected_my_issue_index = 0;
                                app.my_issues_loaded = true;
                                app.clear_status();
                            }
                            Err(e) => app.set_error(format!("Failed to load my issues: {e}")),
                        }
                    } else {
                        app.set_error("Viewer not loaded — cannot fetch my issues");
                    }
                }
                Tab::Projects => {
                    if let Some(team) = app.current_team() {
                        let team_id = team.id.clone();
                        match client.projects(&team_id).await {
                            Ok(projects) => {
                                app.projects = projects;
                                app.selected_project_index = 0;
                                app.projects_loaded = true;
                                app.clear_status();
                            }
                            Err(e) => app.set_error(format!("Failed to load projects: {e}")),
                        }
                    }
                }
                Tab::Cycles => {
                    if let Some(team) = app.current_team() {
                        let team_id = team.id.clone();
                        match client.cycles(&team_id).await {
                            Ok(cycles) => {
                                app.cycles = cycles;
                                app.selected_cycle_index = 0;
                                app.cycles_loaded = true;
                                app.clear_status();
                            }
                            Err(e) => app.set_error(format!("Failed to load cycles: {e}")),
                        }
                    }
                }
            }
            app.loading = false;
        }

        // Load more issues (pagination) — only for Issues tab
        if app.needs_load_more && !app.loading {
            app.needs_load_more = false;
            if let Some(team) = app.current_team()
                && let Some(cursor) = app.page_info.end_cursor.clone()
            {
                let team_id = team.id.clone();
                app.loading = true;
                terminal.draw(|f| ui::draw(f, &app))?;

                match client.issues(&team_id, Some(&cursor), items_per_page).await {
                    Ok((mut new_issues, page_info)) => {
                        app.issues.append(&mut new_issues);
                        app.page_info = page_info;
                    }
                    Err(e) => app.set_error(format!("Failed to load more: {e}")),
                }
                app.loading = false;
            }
        }

        // Load issue detail if needed
        if app.screen == Screen::IssueDetail
            && let Some(issue) = &app.current_issue
            && issue.comments.is_none()
        {
            let issue_id = issue.id.clone();
            app.loading = true;
            terminal.draw(|f| ui::draw(f, &app))?;

            match client.issue_detail(&issue_id).await {
                Ok(detail) => app.current_issue = Some(detail),
                Err(e) => app.set_error(format!("Failed to load detail: {e}")),
            }
            app.loading = false;
        }

        // Load project issues if needed
        if app.screen == Screen::ProjectDetail
            && app.project_issues.is_empty()
            && let Some(project) = &app.current_project
        {
            let project_id = project.id.clone();
            app.loading = true;
            terminal.draw(|f| ui::draw(f, &app))?;

            match client.project_issues(&project_id).await {
                Ok(issues) => app.project_issues = issues,
                Err(e) => app.set_error(format!("Failed to load project issues: {e}")),
            }
            app.loading = false;
        }

        // Load cycle issues if needed
        if app.screen == Screen::CycleDetail
            && app.cycle_issues.is_empty()
            && let Some(cycle) = &app.current_cycle
        {
            let cycle_id = cycle.id.clone();
            app.loading = true;
            terminal.draw(|f| ui::draw(f, &app))?;

            match client.cycle_issues(&cycle_id).await {
                Ok(issues) => app.cycle_issues = issues,
                Err(e) => app.set_error(format!("Failed to load cycle issues: {e}")),
            }
            app.loading = false;
        }

        // Execute pending mutations
        if let Some(action) = app.pending_action.take() {
            app.loading = true;
            terminal.draw(|f| ui::draw(f, &app))?;

            match &action {
                PendingAction::UpdateStatus { issue_id, state_id } => {
                    match client.update_issue_state(issue_id, state_id).await {
                        Ok(()) => {
                            app.set_status("Status updated");
                            app.needs_reload = true;
                        }
                        Err(e) => app.set_error(format!("Failed to update status: {e}")),
                    }
                }
                PendingAction::UpdatePriority { issue_id, priority } => {
                    match client
                        .update_issue_priority(issue_id, priority.as_u8())
                        .await
                    {
                        Ok(()) => {
                            app.set_status("Priority updated");
                            app.needs_reload = true;
                        }
                        Err(e) => app.set_error(format!("Failed to update priority: {e}")),
                    }
                }
                PendingAction::UpdateAssignee {
                    issue_id,
                    assignee_id,
                } => {
                    match client
                        .update_issue_assignee(issue_id, assignee_id.as_deref())
                        .await
                    {
                        Ok(()) => {
                            app.set_status("Assignee updated");
                            app.needs_reload = true;
                        }
                        Err(e) => app.set_error(format!("Failed to update assignee: {e}")),
                    }
                }
                PendingAction::CreateComment { issue_id, body } => {
                    match client.create_comment(issue_id, body).await {
                        Ok(()) => {
                            app.set_status("Comment posted");
                            // Reload detail to show new comment
                            if app.screen == Screen::IssueDetail
                                && let Some(issue) = &mut app.current_issue
                            {
                                issue.comments = None;
                            }
                        }
                        Err(e) => app.set_error(format!("Failed to post comment: {e}")),
                    }
                }
            }
            app.loading = false;
        }

        terminal.draw(|f| ui::draw(f, &app))?;

        event::poll_and_handle(&mut app)?;

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
