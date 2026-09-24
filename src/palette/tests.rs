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

fn titles(app: &App) -> Vec<String> {
    entries(app).into_iter().map(|e| e.title).collect()
}

fn lists(app: &App, title: &str) -> bool {
    titles(app).iter().any(|t| t == title)
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
    assert!(lists(&app, "Change status\u{2026}"));

    let mut app = self::app();
    app.nav.screen = Screen::ProjectList;
    ctrl(&mut app, 'k');
    let listed = titles(&app);
    assert!(
        !listed.iter().any(|t| t == "Change status\u{2026}"),
        "no issue here"
    );
    assert!(
        listed.iter().any(|t| t == "Go to my issues"),
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
    assert!(listed.iter().any(|t| t == "Copy issue ID"));
    assert!(listed.iter().any(|t| t == "Copy issue URL"));
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
    let second = titles(&app)[1].clone();
    press(&mut app, KeyCode::Down);
    assert_eq!(app.view.palette.selected, 1);
    ctrl(&mut app, 'p');
    ctrl(&mut app, 'n');
    press(&mut app, KeyCode::Enter);
    let expected = match second.as_str() {
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

// ------------------------------------------------------ pages (pickers)

use crate::api::types::{User, WorkflowState};
use crate::grouping::GroupBy;
use crate::message::Message;

fn state(id: &str, name: &str, kind: &str) -> WorkflowState {
    serde_json::from_str(&format!(
        r#"{{"id":"{id}","name":"{name}","type":"{kind}","position":1}}"#
    ))
    .unwrap()
}

fn member(id: &str, name: &str) -> User {
    serde_json::from_str(&format!(r#"{{"id":"{id}","name":"{name}"}}"#)).unwrap()
}

fn context() -> Message {
    Message::TeamContext {
        team_id: "t".into(),
        states: vec![
            state("s-todo", "Todo", "unstarted"),
            state("s-prog", "In Progress", "started"),
            state("s-done", "Done", "completed"),
        ],
        members: vec![member("u1", "Ada Lovelace"), member("u2", "Grace Hopper")],
    }
}

fn app_with_team() -> App {
    let mut app = app();
    app.handle_message(context());
    app.outbox.requests.clear();
    app
}

#[test]
fn a_picker_narrows_as_you_type_and_enter_takes_the_top_match() {
    let mut app = app_with_team();
    press(&mut app, KeyCode::Char('s'));
    typed(&mut app, "Prog");
    assert_eq!(app.popup_list_len(), 1);
    press(&mut app, KeyCode::Enter);
    assert!(matches!(
        app.outbox.requests.back(),
        Some(Request::UpdateStatus { state_id, .. }) if state_id.as_str() == "s-prog"
    ));
}

#[test]
fn until_something_is_typed_a_picker_keeps_its_single_key_moves() {
    let mut app = app_with_team();
    press(&mut app, KeyCode::Char('s'));
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.view.popup_index, 1, "j moves");
    assert!(app.view.popup_query.is_empty());

    // Upper case starts a query: matching ignores case.
    typed(&mut app, "Do");
    assert_eq!(app.view.popup_query.value, "Do");
    typed(&mut app, "jk");
    assert_eq!(
        app.view.popup_query.value, "Dojk",
        "then j and k are letters"
    );
}

#[test]
fn digits_pick_only_while_nothing_is_typed() {
    let mut app = app_with_team();
    press(&mut app, KeyCode::Char('a'));
    typed(&mut app, "Grace");
    typed(&mut app, "1");
    assert_eq!(app.view.popup_query.value, "Grace1");
    assert!(app.outbox.requests.is_empty());
}

#[test]
fn an_assignee_is_found_by_name() {
    let mut app = app_with_team();
    press(&mut app, KeyCode::Char('a'));
    typed(&mut app, "Hop");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.focused_issue().unwrap().assignee.as_ref().unwrap().name,
        "Grace Hopper"
    );
}

#[test]
fn a_picker_reached_from_the_palette_steps_back_to_it() {
    let mut app = app_with_team();
    ctrl(&mut app, 'k');
    typed(&mut app, "change status");
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.view.popup, Popup::StatusChange(_)));

    press(&mut app, KeyCode::Esc);
    assert_eq!(app.view.popup, Popup::Palette, "Esc goes back a page");
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.view.popup, Popup::None);
}

#[test]
fn backspace_on_an_empty_query_steps_back_only_from_the_palette() {
    let mut app = app_with_team();
    ctrl(&mut app, 'k');
    typed(&mut app, "assign to");
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.view.popup, Popup::AssigneeChange(_)));
    typed(&mut app, "A");
    press(&mut app, KeyCode::Backspace);
    assert!(
        matches!(app.view.popup, Popup::AssigneeChange(_)),
        "erases first"
    );
    press(&mut app, KeyCode::Backspace);
    assert_eq!(app.view.popup, Popup::Palette);

    let mut app = app_with_team();
    press(&mut app, KeyCode::Char('a'));
    press(&mut app, KeyCode::Backspace);
    assert!(
        matches!(app.view.popup, Popup::AssigneeChange(_)),
        "opened by its key, it stays"
    );
}

#[test]
fn group_by_is_picked_directly_from_the_palette() {
    let mut app = app_with_team();
    ctrl(&mut app, 'k');
    typed(&mut app, "group by");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.view.popup, Popup::GroupBy);
    typed(&mut app, "Assi");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.view.group_by, GroupBy::Assignee);
    assert_eq!(app.view.popup, Popup::None);
}

#[test]
fn a_filter_page_narrows_its_states() {
    let mut app = app_with_team();
    press(&mut app, KeyCode::Char('f'));
    typed(&mut app, "Done");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.list().filters.status.as_deref(), Some("Done"));
    assert!(
        app.view.popup_query.is_empty(),
        "the priority question starts afresh"
    );
}

#[test]
fn a_team_context_landing_while_typing_keeps_the_query() {
    let mut app = app();
    press(&mut app, KeyCode::Char('a'));
    assert!(app.popup_loading());
    typed(&mut app, "Grace");
    app.handle_message(context());
    assert_eq!(app.view.popup_query.value, "Grace");
    assert_eq!(
        app.view.popup_index, 0,
        "the top match, not the current value"
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.focused_issue().unwrap().assignee.as_ref().unwrap().name,
        "Grace Hopper"
    );
}

// ------------------------------------------------------ places and issues

use std::time::Duration;

use crate::app::{Nav, TeamSection};

fn later() -> Instant {
    Instant::now() + Duration::from_secs(1)
}

fn searches(app: &App) -> Vec<(String, u64)> {
    app.outbox
        .requests
        .iter()
        .filter_map(|r| match r {
            Request::PaletteSearch { term, seq } => Some((term.clone(), *seq)),
            _ => None,
        })
        .collect()
}

#[test]
fn an_issue_found_only_by_linear_s_search_opens() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, "ENG-99");
    assert!(searches(&app).is_empty(), "not before the query rests");
    assert!(app.flush_palette_search(later()));
    let [(term, seq)] = searches(&app).try_into().unwrap();
    assert_eq!(term, "ENG-99");

    app.handle_message(Message::PaletteResults {
        seq,
        issues: vec![issue("99")],
    });
    assert_eq!(titles(&app)[0], "ENG-99 t99");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.nav.screen, Screen::IssueDetail);
    assert_eq!(app.store.current_issue.as_ref().unwrap().id.as_str(), "99");
}

#[test]
fn a_burst_of_typing_sends_one_search() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, "deploy");
    assert!(
        !app.flush_palette_search(Instant::now()),
        "still inside the delay"
    );
    assert!(app.flush_palette_search(later()));
    assert!(!app.flush_palette_search(later()), "sent once");
    assert_eq!(searches(&app).len(), 1);
    assert_eq!(searches(&app)[0].0, "deploy");
}

#[test]
fn a_stale_answer_is_dropped() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, "first");
    app.flush_palette_search(later());
    let old = searches(&app)[0].1;
    typed(&mut app, " second");
    app.flush_palette_search(later());

    app.handle_message(Message::PaletteResults {
        seq: old,
        issues: vec![issue("50")],
    });
    assert!(app.view.palette.results.is_empty());
    assert!(app.view.palette.searching, "still waiting for the latest");
}

#[test]
fn short_queries_and_commands_only_queries_are_not_searched() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, "ab");
    assert!(!app.flush_palette_search(later()));
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    typed(&mut app, ">refresh");
    assert!(!app.flush_palette_search(later()));

    // An ID is worth searching however short.
    let mut app = self::app();
    ctrl(&mut app, 'k');
    typed(&mut app, "e-1");
    assert!(app.flush_palette_search(later()));
}

#[test]
fn a_failed_search_leaves_the_palette_open_without_an_error() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, "deploy");
    app.flush_palette_search(later());
    let request = app.outbox.requests.pop_back().unwrap();
    app.handle_message(Message::Failed {
        request: Box::new(request),
        error: "offline".into(),
    });
    assert_eq!(app.view.popup, Popup::Palette);
    assert!(app.view.error_popup.is_none());
    assert!(!app.view.palette.searching);
}

#[test]
fn a_loaded_issue_is_found_by_its_id() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, "eng-3");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.store.current_issue.as_ref().unwrap().id.as_str(), "3");
}

#[test]
fn a_team_page_is_reached_by_name_with_the_sidebar_in_sync() {
    let mut app = app();
    app.store
        .teams
        .push(serde_json::from_str(r#"{"id":"o","name":"Ops","key":"OPS"}"#).unwrap());
    ctrl(&mut app, 'k');
    typed(&mut app, "ops proj");
    let top = entries(&app).into_iter().next().unwrap();
    assert!(matches!(
        top.target,
        Target::Go(Nav::Team(1, TeamSection::Projects))
    ));
    drop(top);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.nav.dest, Nav::Team(1, TeamSection::Projects));
    assert_eq!(app.nav.team, 1);
    assert_eq!(app.nav.screen, Screen::ProjectList);
}

#[test]
fn a_saved_view_and_a_project_are_places_too() {
    let mut app = app();
    app.store.set_custom_views(vec![
        serde_json::from_str(r#"{"id":"v","name":"Release blockers","shared":false}"#).unwrap(),
    ]);
    app.store.projects.items =
        vec![serde_json::from_str(r#"{"id":"p","name":"Mobile launch"}"#).unwrap()];

    ctrl(&mut app, 'k');
    typed(&mut app, "release blockers");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.nav.dest, Nav::View(0));

    ctrl(&mut app, 'k');
    typed(&mut app, "mobile launch");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.nav.screen, Screen::ProjectDetail);
    assert_eq!(
        app.nav.current_project.as_ref().unwrap().name,
        "Mobile launch"
    );
    assert_eq!(app.nav.dest, Nav::Team(0, TeamSection::Projects));
}

#[test]
fn a_leading_angle_bracket_asks_for_commands_only() {
    let mut app = app();
    ctrl(&mut app, 'k');
    typed(&mut app, ">core");
    assert!(
        entries(&app)
            .iter()
            .all(|e| matches!(e.target, Target::Command(_)))
    );
}

#[test]
fn places_are_offered_only_once_something_is_typed() {
    let mut app = app();
    ctrl(&mut app, 'k');
    assert!(
        entries(&app)
            .iter()
            .all(|e| matches!(e.target, Target::Command(_)))
    );
}
