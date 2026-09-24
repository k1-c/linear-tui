use crossterm::event::{KeyEventKind, KeyEventState};

use super::*;
use crate::api::types::{Issue, Priority};
use crate::app::{IssueSource, Popup, Screen};
use crate::config::Config;
use crate::message::Request;

fn key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    keys::handle_key(
        app,
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        },
    );
}

fn press(app: &mut App, code: KeyCode) {
    key(app, code, KeyModifiers::NONE);
}

fn ctrl(app: &mut App, c: char) {
    key(app, KeyCode::Char(c), KeyModifiers::CONTROL);
}

fn typed(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}

fn issue(id: &str) -> Issue {
    serde_json::from_str(&format!(
        r#"{{"id":"{id}","identifier":"ENG-{id}","title":"t{id}","priority":0}}"#
    ))
    .unwrap()
}

/// A team list with three issues, the cursor on the first.
fn app() -> App {
    let mut app = App::new(&Config::default());
    app.store.teams =
        vec![serde_json::from_str(r#"{"id":"t","name":"Core","key":"ENG"}"#).unwrap()];
    app.store.issues[IssueSource::Team].items = vec![issue("1"), issue("2"), issue("3")];
    app.outbox.requests.clear();
    app
}

fn titles(app: &App) -> Vec<&'static str> {
    entries(app).iter().map(|e| e.title).collect()
}

#[test]
fn ctrl_k_opens_the_palette_everywhere_and_esc_closes_it() {
    for screen in [
        Screen::IssueList,
        Screen::IssueDetail,
        Screen::ProjectList,
        Screen::CycleList,
        Screen::ViewList,
    ] {
        let mut app = app();
        app.nav.screen = screen;
        ctrl(&mut app, 'k');
        assert_eq!(app.view.popup, Popup::Palette, "{screen:?}");
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.view.popup, Popup::None);
        assert_eq!(app.nav.screen, screen, "Esc has no side effect");
        assert!(app.outbox.requests.is_empty());
    }

    let mut app = app();
    app.view.sidebar.focus = true;
    ctrl(&mut app, 'k');
    assert_eq!(app.view.popup, Popup::Palette, "from the sidebar too");
}

#[test]
fn ctrl_k_in_a_text_field_still_kills_to_the_end() {
    let mut app = app();
    press(&mut app, KeyCode::Char('/'));
    typed(&mut app, "abc");
    press(&mut app, KeyCode::Home);
    ctrl(&mut app, 'k');
    assert_eq!(app.view.popup, Popup::None);
    assert_eq!(app.list().search.value, "");
}

#[test]
fn only_commands_that_apply_here_are_listed() {
    let mut app = app();
    ctrl(&mut app, 'k');
    assert!(titles(&app).contains(&"Change status\u{2026}"));

    let mut app = self::app();
    app.nav.screen = Screen::ProjectList;
    ctrl(&mut app, 'k');
    let listed = titles(&app);
    assert!(!listed.contains(&"Change status\u{2026}"), "no issue here");
    assert!(
        listed.contains(&"Go to my issues"),
        "chord targets are offered"
    );
}

#[test]
fn typing_narrows_and_ranks_the_commands() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, "stat");
    assert_eq!(titles(&app)[0], "Change status\u{2026}");

    // j and k are letters here, not movement.
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    typed(&mut app, "jk");
    assert_eq!(app.view.palette.query.value, "jk");
}

#[test]
fn a_keyword_finds_a_command_whose_title_does_not_match() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, "clipboard");
    let listed = titles(&app);
    assert!(listed.contains(&"Copy issue ID"));
    assert!(listed.contains(&"Copy issue URL"));
}

#[test]
fn enter_runs_the_command_as_its_key_would() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, "urgent");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.view.popup, Popup::None);
    assert!(matches!(
        app.outbox.requests.back(),
        Some(Request::UpdatePriority {
            priority: Priority::Urgent,
            ..
        })
    ));
    assert_eq!(app.focused_issue().unwrap().priority, Priority::Urgent);
}

#[test]
fn a_command_opening_a_popup_leaves_that_popup_open() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, "change priority");
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.view.popup, Popup::PriorityChange(_)));
}

#[test]
fn arrows_move_the_cursor_and_enter_runs_the_highlighted_row() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, "priority to");
    let second = titles(&app)[1];
    press(&mut app, KeyCode::Down);
    assert_eq!(app.view.palette.selected, 1);
    ctrl(&mut app, 'p');
    ctrl(&mut app, 'n');
    press(&mut app, KeyCode::Enter);
    let expected = match second {
        "Set priority to High" => Priority::High,
        "Set priority to Urgent" => Priority::Urgent,
        "Set priority to Medium" => Priority::Medium,
        "Set priority to Low" => Priority::Low,
        other => panic!("unexpected second row {other}"),
    };
    assert_eq!(app.focused_issue().unwrap().priority, expected);
}

#[test]
fn a_command_does_not_run_on_another_issue_than_the_one_it_was_opened_on() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, "urgent");
    // A page landing under the palette moved the cursor.
    app.set_selected_index(2);
    press(&mut app, KeyCode::Enter);
    assert!(app.outbox.requests.is_empty());
    assert!(
        app.store.issues[IssueSource::Team]
            .items
            .iter()
            .all(|i| i.priority == Priority::None)
    );
    assert!(app.view.status_message.is_some());
}

#[test]
fn recently_run_commands_come_first() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, "copy issue url");
    press(&mut app, KeyCode::Enter);
    ctrl(&mut app, 'k');
    assert_eq!(titles(&app)[0], "Copy issue URL");
}

#[test]
fn a_click_on_a_row_runs_it_and_a_click_outside_closes() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, "urgent");
    app.frame.popup_area = ratatui::layout::Rect::new(10, 5, 40, 10);
    click(&mut app, 12, 5);
    assert_eq!(app.focused_issue().unwrap().priority, Priority::Urgent);

    ctrl(&mut app, 'k');
    click(&mut app, 0, 0);
    assert_eq!(app.view.popup, Popup::None);
}
