use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, InputMode, Popup, Screen, Tab};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    // Error popup dismisses on any key
    if app.error_popup.is_some() {
        app.dismiss_error();
        return;
    }

    // Help overlay
    if app.show_help {
        app.show_help = false;
        return;
    }

    // Popup takes priority
    if app.popup != Popup::None {
        handle_popup_keys(app, key);
        return;
    }

    match app.input_mode {
        InputMode::Normal => handle_normal_mode(app, key),
        InputMode::Search => handle_search_mode(app, key),
        InputMode::Comment => handle_comment_mode(app, key),
    }
}

fn handle_popup_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.close_popup(),
        KeyCode::Char('j') | KeyCode::Down => app.popup_next(),
        KeyCode::Char('k') | KeyCode::Up => app.popup_prev(),
        KeyCode::Enter => match app.popup {
            Popup::TeamSelect => app.select_team(),
            Popup::Filter => app.apply_filter_selection(),
            Popup::StatusChange => app.apply_status_selection(),
            Popup::PriorityChange => app.apply_priority_selection(),
            Popup::AssigneeChange => app.apply_assignee_selection(),
            Popup::None => {}
        },
        KeyCode::Char(c @ '1'..='9') => {
            let idx = (c as usize) - ('1' as usize);
            if idx < app.popup_list_len() {
                app.popup_index = idx;
                match app.popup {
                    Popup::TeamSelect => app.select_team(),
                    Popup::Filter => app.apply_filter_selection(),
                    Popup::StatusChange => app.apply_status_selection(),
                    Popup::PriorityChange => app.apply_priority_selection(),
                    Popup::AssigneeChange => app.apply_assignee_selection(),
                    Popup::None => {}
                }
            }
        }
        _ => {}
    }
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) {
    // Help
    if key.code == KeyCode::Char('?') {
        app.show_help = true;
        return;
    }

    // Tab switching with 1-4 (only on list screens)
    if matches!(
        app.screen,
        Screen::IssueList | Screen::ProjectList | Screen::CycleList
    ) {
        match key.code {
            KeyCode::Char('1') => {
                app.switch_tab(Tab::Issues);
                return;
            }
            KeyCode::Char('2') => {
                app.switch_tab(Tab::MyIssues);
                return;
            }
            KeyCode::Char('3') => {
                app.switch_tab(Tab::Projects);
                return;
            }
            KeyCode::Char('4') => {
                app.switch_tab(Tab::Cycles);
                return;
            }
            _ => {}
        }
    }

    // Shared mutation keys (work in issue list/detail screens)
    if matches!(
        app.screen,
        Screen::IssueList | Screen::IssueDetail | Screen::ProjectDetail | Screen::CycleDetail
    ) {
        match key.code {
            KeyCode::Char('s') => {
                app.open_status_change();
                return;
            }
            KeyCode::Char('p') if app.screen != Screen::IssueList => {
                app.open_priority_change();
                return;
            }
            KeyCode::Char('a') => {
                app.open_assignee_change();
                return;
            }
            KeyCode::Char('c') => {
                app.start_comment();
                return;
            }
            _ => {}
        }
    }

    match app.screen {
        Screen::IssueList => match app.tab {
            Tab::Issues => handle_issue_list_keys(app, key),
            Tab::MyIssues => handle_my_issues_keys(app, key),
            _ => {}
        },
        Screen::IssueDetail => handle_issue_detail_keys(app, key),
        Screen::ProjectList => handle_project_list_keys(app, key),
        Screen::ProjectDetail => handle_project_detail_keys(app, key),
        Screen::CycleList => handle_cycle_list_keys(app, key),
        Screen::CycleDetail => handle_cycle_detail_keys(app, key),
    }
}

fn handle_issue_list_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => app.next_issue(),
        KeyCode::Char('k') | KeyCode::Up => app.previous_issue(),
        KeyCode::Char('g') => app.first_issue(),
        KeyCode::Char('G') => app.last_issue(),
        KeyCode::Enter => app.open_issue_detail(),
        KeyCode::Char('r') => app.request_reload(),
        KeyCode::Char('t') => app.open_team_select(),
        KeyCode::Char('f') => app.open_filter(),
        KeyCode::Char('F') => app.clear_filters(),
        KeyCode::Char('p') => app.open_priority_change(),
        KeyCode::Char('/') => {
            app.input_mode = InputMode::Search;
            app.search_query.clear();
        }
        KeyCode::Esc => {
            if !app.search_query.is_empty() {
                app.search_query.clear();
                app.filtered_issues.clear();
                app.selected_issue_index = 0;
            }
        }
        _ => {}
    }
}

fn handle_my_issues_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => {
            App::nav_next(app.my_issues.len(), &mut app.selected_my_issue_index);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            App::nav_prev(&mut app.selected_my_issue_index);
        }
        KeyCode::Char('g') => App::nav_first(&mut app.selected_my_issue_index),
        KeyCode::Char('G') => {
            App::nav_last(app.my_issues.len(), &mut app.selected_my_issue_index);
        }
        KeyCode::Enter => {
            if let Some(issue) = app.my_issues.get(app.selected_my_issue_index).cloned() {
                app.open_issue_from_list(&issue);
            }
        }
        KeyCode::Char('r') => {
            app.my_issues_loaded = false;
            app.needs_reload = true;
        }
        KeyCode::Char('t') => app.open_team_select(),
        _ => {}
    }
}

fn handle_issue_detail_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => match app.tab {
            Tab::Issues | Tab::MyIssues => app.screen = Screen::IssueList,
            Tab::Projects => app.screen = Screen::ProjectDetail,
            Tab::Cycles => app.screen = Screen::CycleDetail,
        },
        KeyCode::Char('j') | KeyCode::Down => app.scroll_down(),
        KeyCode::Char('k') | KeyCode::Up => app.scroll_up(),
        KeyCode::Char('g') => app.detail_scroll = 0,
        _ => {}
    }
}

fn handle_project_list_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => app.next_project(),
        KeyCode::Char('k') | KeyCode::Up => app.previous_project(),
        KeyCode::Char('g') => App::nav_first(&mut app.selected_project_index),
        KeyCode::Char('G') => {
            App::nav_last(app.projects.len(), &mut app.selected_project_index);
        }
        KeyCode::Enter => app.open_project_detail(),
        KeyCode::Char('r') => {
            app.projects_loaded = false;
            app.needs_reload = true;
        }
        KeyCode::Char('t') => app.open_team_select(),
        _ => {}
    }
}

fn handle_project_detail_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.screen = Screen::ProjectList,
        KeyCode::Char('j') | KeyCode::Down => {
            App::nav_next(
                app.project_issues.len(),
                &mut app.selected_project_issue_index,
            );
        }
        KeyCode::Char('k') | KeyCode::Up => {
            App::nav_prev(&mut app.selected_project_issue_index);
        }
        KeyCode::Enter => {
            if let Some(issue) = app
                .project_issues
                .get(app.selected_project_issue_index)
                .cloned()
            {
                app.open_issue_from_list(&issue);
            }
        }
        _ => {}
    }
}

fn handle_cycle_list_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => app.next_cycle(),
        KeyCode::Char('k') | KeyCode::Up => app.previous_cycle(),
        KeyCode::Char('g') => App::nav_first(&mut app.selected_cycle_index),
        KeyCode::Char('G') => {
            App::nav_last(app.cycles.len(), &mut app.selected_cycle_index);
        }
        KeyCode::Enter => app.open_cycle_detail(),
        KeyCode::Char('r') => {
            app.cycles_loaded = false;
            app.needs_reload = true;
        }
        KeyCode::Char('t') => app.open_team_select(),
        _ => {}
    }
}

fn handle_cycle_detail_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.screen = Screen::CycleList,
        KeyCode::Char('j') | KeyCode::Down => {
            App::nav_next(app.cycle_issues.len(), &mut app.selected_cycle_issue_index);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            App::nav_prev(&mut app.selected_cycle_issue_index);
        }
        KeyCode::Enter => {
            if let Some(issue) = app
                .cycle_issues
                .get(app.selected_cycle_issue_index)
                .cloned()
            {
                app.open_issue_from_list(&issue);
            }
        }
        _ => {}
    }
}

fn handle_search_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.search_query.clear();
            app.filtered_issues.clear();
            app.selected_issue_index = 0;
        }
        KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
            app.apply_search();
        }
        KeyCode::Backspace => {
            app.search_query.pop();
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
        }
        _ => {}
    }
}

fn handle_comment_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.comment_input.clear();
        }
        KeyCode::Enter => {
            // Ctrl+Enter or just Enter to submit
            if key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::ALT)
            {
                app.submit_comment();
            } else {
                app.comment_input.push('\n');
            }
        }
        KeyCode::Backspace => {
            app.comment_input.pop();
        }
        KeyCode::Char(c) => {
            app.comment_input.push(c);
        }
        _ => {}
    }
}
