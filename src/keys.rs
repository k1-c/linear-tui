use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::api::types::Priority;
use crate::app::{App, FormField, Input, InputMode, Nav, Popup, Screen, TeamSection};
use crate::grouping::Preset;

pub fn handle_key(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    if ctrl && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    // Error popup dismisses on any key
    if app.error_popup.is_some() {
        app.dismiss_error();
        return;
    }

    // Help overlay: j/k scrolls, anything else closes
    if app.show_help {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                app.help_scroll = app.help_scroll.saturating_add(1)
            }
            KeyCode::Char('k') | KeyCode::Up => app.help_scroll = app.help_scroll.saturating_sub(1),
            _ => app.show_help = false,
        }
        return;
    }

    // Esc abandons a half-typed chord
    if key.code == KeyCode::Esc && app.pending_chord.take().is_some() {
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
        InputMode::NewIssue => handle_new_issue_mode(app, key),
    }
}

fn handle_popup_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.close_popup(),
        KeyCode::Char('j') | KeyCode::Down => app.popup_next(),
        KeyCode::Char('k') | KeyCode::Up => app.popup_prev(),
        KeyCode::Char('g') | KeyCode::Home => app.popup_index = 0,
        KeyCode::Char('G') | KeyCode::End => {
            app.popup_index = app.popup_list_len().saturating_sub(1);
        }
        KeyCode::Enter => app.apply_popup(),
        KeyCode::Char(c @ '1'..='9') => {
            let idx = (c as usize) - ('1' as usize);
            if idx < app.popup_list_len() {
                app.popup_index = idx;
                app.apply_popup();
            }
        }
        _ => {}
    }
}

/// Linear binds several actions to `Ctrl`+punctuation. A terminal only reports
/// those distinctly under the kitty keyboard protocol, so each one also has a
/// plain-key alias that works everywhere.
fn handle_global_actions(app: &mut App, key: KeyEvent) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match key.code {
        // Copy issue ID — Linear: Ctrl+.
        KeyCode::Char('.') if ctrl && !shift => app.copy_identifier(),
        KeyCode::Char('y') if !ctrl => app.copy_identifier(),
        // Copy issue URL — Linear: Ctrl+Shift+,
        KeyCode::Char(',' | '<') if ctrl && shift => app.copy_url(),
        KeyCode::Char('Y') if !ctrl => app.copy_url(),
        // Copy git branch name — Linear: Ctrl+Shift+.
        KeyCode::Char('.' | '>') if ctrl && shift => app.copy_branch_name(),
        KeyCode::Char('b') if !ctrl => app.copy_branch_name(),
        // Add comment — Linear: Ctrl+M. A legacy terminal reports Ctrl+M as
        // Enter, so plain `m` is accepted too (Linear leaves `m` for relations,
        // which this client does not support).
        KeyCode::Char('m') if !ctrl => app.start_comment(),
        // Direct priority — Linear: Shift+1..4 / Shift+0
        KeyCode::Char('!') => app.set_priority(Priority::Urgent),
        KeyCode::Char('@') => app.set_priority(Priority::High),
        KeyCode::Char('#') => app.set_priority(Priority::Medium),
        KeyCode::Char('$') => app.set_priority(Priority::Low),
        KeyCode::Char(')') => app.set_priority(Priority::None),
        _ => return false,
    }
    true
}

/// Second key of a `g …` chord.
///
/// Linear reaches its view presets with `G` then `A`/`B`/`E` (Active, Backlog,
/// All issues) and its pages with the other letters; both are mirrored here,
/// plus vim's `gg`.
fn handle_goto_chord(app: &mut App, key: KeyEvent) {
    app.pending_chord = None;
    match key.code {
        // vim: gg
        KeyCode::Char('g') => {
            if app.screen == Screen::IssueDetail {
                app.scroll_to_top();
            } else {
                app.select_first();
            }
        }
        // Linear's view presets.
        KeyCode::Char('a') => {
            app.go_to_team_issues();
            app.set_preset(Preset::Active);
        }
        KeyCode::Char('b') => {
            app.go_to_team_issues();
            app.set_preset(Preset::Backlog);
        }
        KeyCode::Char('e') => {
            app.go_to_team_issues();
            app.set_preset(Preset::All);
        }
        KeyCode::Char('m') => app.activate(Nav::MyIssues),
        KeyCode::Char('v') => app.activate(Nav::Views),
        KeyCode::Char('p') => app.go_to_team_section(TeamSection::Projects),
        KeyCode::Char('c') => app.go_to_team_section(TeamSection::Cycles),
        _ => {}
    }
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) {
    // A pending `g` swallows the next key.
    if app.pending_chord == Some('g') {
        handle_goto_chord(app, key);
        return;
    }

    // Help
    if key.code == KeyCode::Char('?') {
        app.show_help = true;
        app.help_scroll = 0;
        return;
    }

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    // Sidebar: show/hide, and move focus in and out of it.
    if ctrl && key.code == KeyCode::Char('b') {
        return app.toggle_sidebar();
    }
    if app.sidebar_focus {
        return handle_sidebar_keys(app, key);
    }
    if matches!(key.code, KeyCode::Tab) {
        return app.focus_sidebar(true);
    }

    // Destination jumps by number, a TUI shorthand for the sidebar. Linear has
    // no equivalent — it reaches pages with `g …`, which works here too.
    if matches!(
        app.screen,
        Screen::IssueList | Screen::ProjectList | Screen::CycleList | Screen::ViewList
    ) {
        match key.code {
            KeyCode::Char('1') => return app.go_to_team_issues(),
            KeyCode::Char('2') => return app.activate(Nav::MyIssues),
            KeyCode::Char('3') => return app.go_to_team_section(TeamSection::Projects),
            KeyCode::Char('4') => return app.go_to_team_section(TeamSection::Cycles),
            KeyCode::Char('5') => return app.activate(Nav::Views),
            _ => {}
        }
    }

    // List shaping. Neither has a Linear keybinding — the web app puts both
    // behind a display-options menu — so they take keys Linear leaves free.
    if app.screen == Screen::IssueList
        || matches!(app.screen, Screen::ProjectDetail | Screen::CycleDetail)
    {
        match key.code {
            KeyCode::Char('D') => return app.cycle_group_by(),
            KeyCode::Char('z') => return app.toggle_selected_group(),
            KeyCode::Char('Z') => return app.toggle_all_groups(),
            KeyCode::BackTab => return app.cycle_preset(),
            _ => {}
        }
    }

    // Issue actions, matching Linear's single-key bindings
    if matches!(
        app.screen,
        Screen::IssueList | Screen::IssueDetail | Screen::ProjectDetail | Screen::CycleDetail
    ) {
        // Step to the neighbouring issue without leaving the detail view.
        if app.screen == Screen::IssueDetail {
            match key.code {
                KeyCode::Char('J') => return app.step_issue(1),
                KeyCode::Char('K') => return app.step_issue(-1),
                _ => {}
            }
        }
        match key.code {
            KeyCode::Char('s') => return app.open_status_change(),
            KeyCode::Char('p') => return app.open_priority_change(),
            KeyCode::Char('a') => return app.open_assignee_change(),
            KeyCode::Char('i') => return app.assign_to_me(),
            _ => {}
        }
        if handle_global_actions(app, key) {
            return;
        }
    }

    // `c` creates an issue from anywhere, as in Linear
    if key.code == KeyCode::Char('c') {
        return app.start_new_issue();
    }

    // `o` opens the issue on linear.app — a TUI-only action, since Linear is
    // already in the browser.
    if key.code == KeyCode::Char('o') {
        return app.open_in_browser();
    }

    // Refresh. Linear syncs live and binds plain `r` to Rename, so this client
    // uses the terminal convention instead and leaves `r` free.
    let refresh = matches!(key.code, KeyCode::F(5))
        || (key.code == KeyCode::Char('r') && key.modifiers.contains(KeyModifiers::CONTROL));
    if refresh {
        if app.screen == Screen::IssueDetail {
            app.refresh_detail();
        } else {
            app.force_reload();
        }
        return;
    }

    match app.screen {
        Screen::IssueList => handle_issue_list_keys(app, key),
        Screen::ViewList => handle_view_list_keys(app, key),
        Screen::IssueDetail => handle_issue_detail_keys(app, key),
        Screen::ProjectList => handle_project_list_keys(app, key),
        Screen::ProjectDetail => handle_project_detail_keys(app, key),
        Screen::CycleList => handle_cycle_list_keys(app, key),
        Screen::CycleDetail => handle_cycle_detail_keys(app, key),
    }
}

/// Keys while the cursor is in the navigation sidebar.
fn handle_sidebar_keys(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => app.sidebar_move(1),
        KeyCode::Char('k') | KeyCode::Up => app.sidebar_move(-1),
        KeyCode::Char('d') if ctrl => app.sidebar_move(app.half_page()),
        KeyCode::Char('u') if ctrl => app.sidebar_move(-app.half_page()),
        KeyCode::Char('g') | KeyCode::Home => app.sidebar_move(isize::MIN / 2),
        KeyCode::Char('G') | KeyCode::End => app.sidebar_move(isize::MAX / 2),
        // A tree folds with h/l, as in every file explorer.
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Char('l') | KeyCode::Right => {
            app.sidebar_toggle()
        }
        KeyCode::Enter | KeyCode::Char(' ') => app.sidebar_activate(),
        KeyCode::Tab | KeyCode::Esc => app.focus_sidebar(false),
        KeyCode::Char('t') => app.open_team_select(),
        KeyCode::Char('q') => app.should_quit = true,
        _ => {}
    }
}

/// Keys on the saved-view index.
fn handle_view_list_keys(app: &mut App, key: KeyEvent) {
    if handle_list_nav(app, key) {
        return;
    }
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Enter | KeyCode::Char(' ') => app.open_selected_view(),
        // Linear's Issues / Projects tabs on a Views page.
        KeyCode::BackTab => app.cycle_view_kind(),
        _ => {}
    }
}

/// Vertical movement shared by every list screen. Returns the row delta, or
/// `None` if the key wasn't a movement key.
fn list_delta(app: &App, key: KeyEvent) -> Option<isize> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(1),
        KeyCode::Char('k') | KeyCode::Up => Some(-1),
        KeyCode::Char('d') if ctrl => Some(app.half_page()),
        KeyCode::Char('u') if ctrl => Some(-app.half_page()),
        KeyCode::PageDown => Some(app.list_viewport.max(1) as isize),
        KeyCode::PageUp => Some(-(app.list_viewport.max(1) as isize)),
        _ => None,
    }
}

/// Cursor movement every list screen shares. Returns true if the key was consumed.
fn handle_list_nav(app: &mut App, key: KeyEvent) -> bool {
    if let Some(delta) = list_delta(app, key) {
        app.move_selection(delta);
        return true;
    }
    match key.code {
        // `g` opens a chord: `gg` jumps to the top, `gm`/`gp`/… switch views.
        KeyCode::Char('g') => app.pending_chord = Some('g'),
        KeyCode::Home => app.select_first(),
        KeyCode::Char('G') | KeyCode::End => app.select_last(),
        _ => return false,
    }
    true
}

fn handle_issue_list_keys(app: &mut App, key: KeyEvent) {
    if handle_list_nav(app, key) {
        return;
    }
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        // Space is Linear's peek; with no split pane it simply opens the issue.
        KeyCode::Enter | KeyCode::Char(' ') => app.open_issue_detail(),
        KeyCode::Char('t') => app.open_team_select(),
        KeyCode::Char('f') => app.open_filter(),
        KeyCode::Char('F') => app.clear_filters(),
        KeyCode::Char('/') => {
            app.input_mode = InputMode::Search;
            app.search.clear();
        }
        KeyCode::Esc => app.clear_search(),
        _ => {}
    }
}

fn handle_issue_detail_keys(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let page = app.detail_viewport as i16;
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.close_detail(),
        KeyCode::Char('j') | KeyCode::Down => app.scroll_down(),
        KeyCode::Char('k') | KeyCode::Up => app.scroll_up(),
        KeyCode::Char('d') if ctrl => app.scroll_by(page / 2),
        KeyCode::Char('u') if ctrl => app.scroll_by(-page / 2),
        KeyCode::PageDown => app.scroll_by(page),
        KeyCode::PageUp => app.scroll_by(-page),
        KeyCode::Char('g') => app.pending_chord = Some('g'),
        KeyCode::Home => app.scroll_to_top(),
        KeyCode::Char('G') | KeyCode::End => app.scroll_to_bottom(),
        _ => {}
    }
}

fn handle_project_list_keys(app: &mut App, key: KeyEvent) {
    if handle_list_nav(app, key) {
        return;
    }
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Enter | KeyCode::Char(' ') => app.open_project_detail(),
        KeyCode::Char('t') => app.open_team_select(),
        _ => {}
    }
}

fn handle_project_detail_keys(app: &mut App, key: KeyEvent) {
    if handle_list_nav(app, key) {
        return;
    }
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.leave_container(),
        // The cursor counts rows in display order, which grouping reorders,
        // so the issue has to be looked up in that order too.
        KeyCode::Enter | KeyCode::Char(' ') => app.open_issue_detail(),
        _ => {}
    }
}

fn handle_cycle_list_keys(app: &mut App, key: KeyEvent) {
    if handle_list_nav(app, key) {
        return;
    }
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Enter | KeyCode::Char(' ') => app.open_cycle_detail(),
        KeyCode::Char('t') => app.open_team_select(),
        _ => {}
    }
}

fn handle_cycle_detail_keys(app: &mut App, key: KeyEvent) {
    if handle_list_nav(app, key) {
        return;
    }
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.leave_container(),
        // The cursor counts rows in display order, which grouping reorders,
        // so the issue has to be looked up in that order too.
        KeyCode::Enter | KeyCode::Char(' ') => app.open_issue_detail(),
        _ => {}
    }
}

/// Readline-style editing shared by the search and comment fields. Returns true
/// if the key was consumed.
fn edit_input(input: &mut Input, key: KeyEvent) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('w') if ctrl => input.kill_word(),
        KeyCode::Char('u') if ctrl => input.kill_to_start(),
        KeyCode::Char('k') if ctrl => input.kill_to_end(),
        KeyCode::Char('a') if ctrl => input.home(),
        KeyCode::Char('e') if ctrl => input.end(),
        KeyCode::Left => input.left(),
        KeyCode::Right => input.right(),
        KeyCode::Home => input.home(),
        KeyCode::End => input.end(),
        KeyCode::Backspace => input.backspace(),
        KeyCode::Delete => input.delete(),
        KeyCode::Char(c) if !ctrl => input.insert(c),
        _ => return false,
    }
    true
}

fn handle_search_mode(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('g') {
        app.search_workspace();
        return;
    }
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.clear_search();
            return;
        }
        KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
            app.apply_search();
            return;
        }
        _ => {}
    }
    if edit_input(&mut app.search, key) {
        // Filter as you type so the list stays in sync with the query.
        app.apply_search();
    }
}

fn handle_comment_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.comment.clear();
            return;
        }
        KeyCode::Enter => {
            // Ctrl/Alt+Enter submits; a bare Enter inserts a newline.
            if key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            {
                app.submit_comment();
            } else {
                app.comment.insert('\n');
            }
            return;
        }
        _ => {}
    }
    edit_input(&mut app.comment, key);
}

fn handle_new_issue_mode(app: &mut App, key: KeyEvent) {
    let ctrl_or_alt = key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);

    match key.code {
        KeyCode::Esc => return app.cancel_new_issue(),
        KeyCode::Tab => return app.new_issue_cycle_field(true),
        KeyCode::BackTab => return app.new_issue_cycle_field(false),
        KeyCode::Enter if ctrl_or_alt => return app.submit_new_issue(),
        _ => {}
    }

    let Some(form) = &mut app.new_issue else {
        return;
    };
    match form.field {
        FormField::Title => {
            // A bare Enter on the title advances rather than inserting a newline.
            if key.code == KeyCode::Enter {
                app.new_issue_cycle_field(true);
            } else {
                edit_input(&mut form.title, key);
            }
        }
        FormField::Description => {
            if key.code == KeyCode::Enter {
                form.description.insert('\n');
            } else {
                edit_input(&mut form.description, key);
            }
        }
        FormField::Priority => match key.code {
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Right => app.new_issue_cycle_priority(1),
            KeyCode::Char('k') | KeyCode::Up | KeyCode::Left => app.new_issue_cycle_priority(-1),
            _ => {}
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::Issue;
    use crate::app::Screen;
    use crate::config::Config;
    use crossterm::event::{KeyEventKind, KeyEventState};

    fn press(app: &mut App, code: KeyCode) {
        handle_key(
            app,
            KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
        );
    }

    fn stated(id: &str, state_id: &str, name: &str, kind: &str) -> Issue {
        serde_json::from_str(&format!(
            r#"{{"id":"{id}","identifier":"ENG-{id}","title":"t{id}","priority":0,
                "state":{{"id":"{state_id}","name":"{name}","type":"{kind}","position":1}},
                "assignee":null,"description":null,"comments":null,"project":null,"cycle":null}}"#
        ))
        .unwrap()
    }

    /// The API returns these as Todo, In Progress, Todo; grouping shows In
    /// Progress first, so the second row on screen is raw index 0.
    fn reordered() -> Vec<Issue> {
        vec![
            stated("1", "s-todo", "Todo", "unstarted"),
            stated("2", "s-prog", "In Progress", "started"),
            stated("3", "s-todo", "Todo", "unstarted"),
        ]
    }

    fn app() -> App {
        let mut app = App::new(&Config::default());
        app.requests.clear();
        app
    }

    /// Regression: Enter on a project's issue opened whichever issue sat at the
    /// cursor's position in the raw API order, not the one highlighted.
    #[test]
    fn enter_on_a_project_issue_opens_the_highlighted_one() {
        let mut app = app();
        app.project_issues = reordered();
        app.screen = Screen::ProjectDetail;
        press(&mut app, KeyCode::Char('j'));
        let highlighted = app.focused_issue().unwrap().id.clone();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.screen, Screen::IssueDetail);
        assert_eq!(app.current_issue.as_ref().unwrap().id, highlighted);
        assert_eq!(highlighted, "1");
    }

    #[test]
    fn enter_on_a_cycle_issue_opens_the_highlighted_one() {
        let mut app = app();
        app.cycle_issues = reordered();
        app.screen = Screen::CycleDetail;
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('j'));
        let highlighted = app.focused_issue().unwrap().id.clone();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.current_issue.as_ref().unwrap().id, highlighted);
        assert_eq!(highlighted, "3");
    }

    /// Every list screen: keyboard Enter and the highlighted row agree.
    #[test]
    fn enter_on_every_issue_list_opens_the_highlighted_issue() {
        use crate::app::Nav;
        for (screen, nav) in [
            (Screen::IssueList, None),
            (Screen::IssueList, Some(Nav::MyIssues)),
            (Screen::ProjectDetail, None),
            (Screen::CycleDetail, None),
        ] {
            let mut app = app();
            app.issues = reordered();
            app.my_issues = reordered();
            app.project_issues = reordered();
            app.cycle_issues = reordered();
            app.set_preset(crate::grouping::Preset::All);
            app.requests.clear();
            if let Some(nav) = nav {
                app.nav = nav;
            }
            app.screen = screen;
            for downs in 0..3 {
                app.screen = screen;
                app.set_selected_index(0);
                for _ in 0..downs {
                    press(&mut app, KeyCode::Char('j'));
                }
                let highlighted = app.focused_issue().unwrap().id.clone();
                press(&mut app, KeyCode::Enter);
                assert_eq!(
                    app.current_issue.as_ref().unwrap().id,
                    highlighted,
                    "{screen:?} {nav:?} row {downs}"
                );
                app.close_detail();
            }
        }
    }
}
