use ratatui::layout::Rect;

use super::*;
use crate::core::store::ListOf;

fn issue(id: &str, identifier: &str, title: &str) -> Issue {
    serde_json::from_str(&format!(
        r#"{{"id":"{id}","identifier":"{identifier}","title":"{title}",
            "priority":0,"state":null,"assignee":null,"description":null,
            "comments":null,"project":null,"cycle":null}}"#
    ))
    .unwrap()
}

fn app_with(issues: Vec<Issue>) -> App {
    let mut app = App::new(&Config::default());
    // Every list response names the team it was fetched for.
    app.store.teams = vec![team("t", "Core")];
    // The team's Active issues, as opened and loaded.
    let rows = &mut app.store.issues[IssueSource::Team];
    rows.of = Some(ListOf::TeamIssues {
        team_id: TeamId::from("t"),
        preset: Preset::Active,
    });
    rows.items = issues;
    rows.loaded = true;
    app.outbox.requests.clear();
    app
}

// ------------------------------------------------------------------ Input

#[test]
fn input_edits_at_the_cursor() {
    let mut input = Input::default();
    for c in "hello world".chars() {
        input.insert(c);
    }
    input.left();
    input.left();
    input.insert('X');
    assert_eq!(input.value, "hello worXld");
    input.backspace();
    assert_eq!(input.value, "hello world");
}

#[test]
fn input_handles_multibyte_characters() {
    let mut input = Input::default();
    for c in "日本語".chars() {
        input.insert(c);
    }
    assert_eq!(input.cursor, 9);
    input.backspace();
    assert_eq!(input.value, "日本");
    input.home();
    input.delete();
    assert_eq!(input.value, "本");
}

#[test]
fn input_kill_word_stops_at_the_previous_space() {
    let mut input = Input::default();
    for c in "add search feature".chars() {
        input.insert(c);
    }
    input.kill_word();
    assert_eq!(input.value, "add search ");
    input.kill_word();
    assert_eq!(input.value, "add ");
}

#[test]
fn input_kill_to_start_and_end_split_at_the_cursor() {
    let mut input = Input::default();
    for c in "abcdef".chars() {
        input.insert(c);
    }
    input.left();
    input.left();
    input.kill_to_end();
    assert_eq!(input.value, "abcd");
    input.kill_to_start();
    assert_eq!(input.value, "");
}

// ------------------------------------------------------------ navigation

#[test]
fn nav_by_clamps_to_the_list() {
    let mut i = 0;
    App::nav_by(5, &mut i, 3);
    assert_eq!(i, 3);
    App::nav_by(5, &mut i, 10);
    assert_eq!(i, 4);
    App::nav_by(5, &mut i, -100);
    assert_eq!(i, 0);
}

/// `select_first`/`select_last` pass extreme deltas; they must not overflow.
#[test]
fn nav_by_survives_sentinel_deltas() {
    let mut i = 2;
    App::nav_by(5, &mut i, isize::MAX / 2);
    assert_eq!(i, 4);
    App::nav_by(5, &mut i, isize::MIN / 2);
    assert_eq!(i, 0);
}

#[test]
fn nav_by_on_an_empty_list_stays_at_zero() {
    let mut i = 7;
    App::nav_by(0, &mut i, 1);
    assert_eq!(i, 0);
}

// ---------------------------------------------------------------- filters

#[test]
fn search_filters_by_title_and_identifier() {
    let mut app = app_with(vec![
        issue("1", "ENG-1", "Fix login"),
        issue("2", "ENG-2", "Add search"),
        issue("3", "OPS-9", "Rotate keys"),
    ]);
    app.list_mut().search.value = "search".into();
    app.apply_search();
    assert_eq!(app.visible_issues().len(), 1);

    app.list_mut().search.value = "eng-".into();
    app.apply_search();
    assert_eq!(app.visible_issues().len(), 2);
}

#[test]
fn clearing_the_search_restores_every_issue() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a"), issue("2", "ENG-2", "b")]);
    app.list_mut().search.value = "nothing-matches".into();
    app.apply_search();
    assert_eq!(app.visible_issues().len(), 0);

    app.clear_search();
    assert_eq!(app.visible_issues().len(), 2);
}

#[test]
fn priority_filter_narrows_the_list() {
    let mut a = issue("1", "ENG-1", "a");
    a.priority = Priority::Urgent;
    let app_issues = vec![a, issue("2", "ENG-2", "b")];
    let mut app = app_with(app_issues);
    app.list_mut().filters.priority = Some(Priority::Urgent);
    assert_eq!(app.visible_issues().len(), 1);
    assert_eq!(app.visible_issues()[0].identifier, "ENG-1");
}

// -------------------------------------------------------------- mutations

/// Regression: changing a field from the detail view used to leave the
/// detail showing the old value until the issue was reopened.
#[test]
fn a_priority_change_updates_every_copy_of_the_issue() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.store.issues[IssueSource::My].items = vec![issue("1", "ENG-1", "a")];
    app.store.issues[IssueSource::View].items = vec![issue("1", "ENG-1", "a")];
    app.store.current_issue = Some(issue("1", "ENG-1", "a"));
    app.nav.screen = Screen::IssueDetail;
    app.open_priority_change();
    app.view.popup_index = Priority::High.as_index();

    app.apply_priority_selection();

    assert_eq!(
        app.store.issues[IssueSource::Team].items[0].priority,
        Priority::High
    );
    assert_eq!(
        app.store.issues[IssueSource::My].items[0].priority,
        Priority::High
    );
    assert_eq!(
        app.store.issues[IssueSource::View].items[0].priority,
        Priority::High
    );
    assert_eq!(
        app.store.current_issue.as_ref().unwrap().priority,
        Priority::High
    );
    assert_eq!(app.view.popup, Popup::None);
}

#[test]
fn a_mutation_queues_exactly_one_request() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.open_priority_change();
    app.view.popup_index = Priority::Low.as_index();
    app.apply_priority_selection();
    assert_eq!(app.outbox.requests.len(), 1);
    assert!(matches!(
        app.outbox.requests.front(),
        Some(Request::Issue(
            crate::core::usecase::issue::Request::SetPriority {
                priority: Priority::Low,
                ..
            }
        ))
    ));
}

#[test]
fn identical_requests_are_not_queued_twice() {
    let mut app = app_with(vec![]);
    let req = Request::Issue(crate::core::usecase::issue::Request::TeamIssues {
        team_id: "t".into(),
        after: None,
        preset: Preset::Active,
    });
    app.request(req.clone());
    app.request(req);
    assert_eq!(app.outbox.requests.len(), 1);
}

// ------------------------------------------------------------ pagination

#[test]
fn appending_a_page_extends_the_list() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.handle_message(Message::Issues {
        team_id: "t".into(),
        preset: Preset::Active,
        page: Page::new(vec![issue("2", "ENG-2", "b")], PageInfo::default(), true),
    });
    assert_eq!(app.store.issues[IssueSource::Team].items.len(), 2);
}

/// Regression: scrolling near the bottom while the next page was still in
/// flight asked for it again on every step, and each copy was appended.
#[test]
fn a_page_is_requested_once_while_scrolling() {
    let mut app = app_with(
        (0..10)
            .map(|i| issue(&i.to_string(), &format!("ENG-{i}"), "t"))
            .collect(),
    );
    app.store.teams =
        vec![serde_json::from_str(r#"{"id":"t","name":"Core","key":"ENG"}"#).unwrap()];
    app.store.issues[IssueSource::Team].page_info = PageInfo {
        has_next_page: true,
        end_cursor: Some("c1".into()),
    };
    app.select_last();
    // The main loop takes the request off the queue: it is now in flight.
    app.outbox.requests.clear();
    app.move_selection(-1);
    app.move_selection(1);
    assert!(
        app.outbox.requests.is_empty(),
        "the in-flight page is not asked for again"
    );
}

#[test]
fn an_appended_page_never_duplicates_an_issue() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a"), issue("2", "ENG-2", "b")]);
    app.handle_message(Message::Issues {
        team_id: "t".into(),
        preset: Preset::Active,
        page: Page::new(
            vec![issue("2", "ENG-2", "b"), issue("3", "ENG-3", "c")],
            PageInfo::default(),
            true,
        ),
    });
    let ids: Vec<_> = app.store.issues[IssueSource::Team]
        .items
        .iter()
        .map(|i| i.id.as_str())
        .collect();
    assert_eq!(ids, ["1", "2", "3"]);
}

#[test]
fn a_fresh_page_replaces_the_list_and_keeps_the_selection() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a"), issue("2", "ENG-2", "b")]);
    app.view.lists[IssueSource::Team].selected = 1;
    // ENG-2 comes back first this time; the cursor must follow the issue,
    // not stay on row 1.
    app.handle_message(Message::Issues {
        team_id: "t".into(),
        preset: Preset::Active,
        page: Page::new(
            vec![issue("2", "ENG-2", "b"), issue("9", "ENG-9", "c")],
            PageInfo::default(),
            false,
        ),
    });
    assert_eq!(app.store.issues[IssueSource::Team].items.len(), 2);
    assert_eq!(app.view.lists[IssueSource::Team].selected, 0);
}

#[test]
fn a_shrinking_list_clamps_the_selection() {
    let mut app = app_with(vec![]);
    app.go_to_team_section(TeamSection::Projects);
    app.view.selected_project_index = 4;
    app.handle_message(Message::Projects {
        team_id: "t".into(),
        page: Page::new(Vec::new(), PageInfo::default(), false),
    });
    assert_eq!(app.view.selected_project_index, 0);
}

// ---------------------------------------------------------------- scroll

#[test]
fn detail_scroll_clamps_to_the_content() {
    let mut app = app_with(vec![]);
    app.frame.detail_lines = 30;
    app.frame.detail_viewport = 10;
    app.scroll_to_bottom();
    assert_eq!(app.view.detail_scroll, 20);
    app.scroll_down();
    assert_eq!(app.view.detail_scroll, 20, "must not scroll past the end");
    app.scroll_by(-100);
    assert_eq!(app.view.detail_scroll, 0);
}

#[test]
fn a_short_body_cannot_scroll_at_all() {
    let mut app = app_with(vec![]);
    app.frame.detail_lines = 4;
    app.frame.detail_viewport = 20;
    app.scroll_down();
    assert_eq!(app.view.detail_scroll, 0);
}

// ------------------------------------------------------------------ tabs

#[test]
fn switching_to_a_cached_tab_does_not_refetch() {
    let mut app = app_with(vec![]);
    app.store.projects.of = Some(ListOf::TeamProjects(TeamId::from("t")));
    app.store.projects.loaded = true;
    app.go_to_team_section(TeamSection::Projects);
    assert_eq!(app.nav.screen, Screen::ProjectList);
    assert!(app.outbox.requests.is_empty());
}

#[test]
fn switching_to_an_uncached_tab_fetches_it() {
    let mut app = app_with(vec![]);
    app.store.teams =
        vec![serde_json::from_str(r#"{"id":"t","name":"Core","key":"ENG"}"#).unwrap()];
    app.go_to_team_section(TeamSection::Cycles);
    assert!(matches!(
        app.outbox.requests.front(),
        Some(Request::Cycle(
            crate::core::usecase::cycle::Request::TeamCycles { .. }
        ))
    ));
}

#[test]
fn changing_team_invalidates_the_other_tabs() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.store.teams = vec![
        serde_json::from_str(r#"{"id":"t1","name":"Core","key":"ENG"}"#).unwrap(),
        serde_json::from_str(r#"{"id":"t2","name":"Ops","key":"OPS"}"#).unwrap(),
    ];
    app.store.projects.loaded = true;
    app.store.issues[IssueSource::My].loaded = true;
    app.open_team_select();
    app.view.popup_index = 1;

    app.select_team();

    assert_eq!(app.nav.team, 1);
    assert!(!app.store.projects.loaded);
    assert!(!app.store.issues[IssueSource::My].loaded);
    assert!(app.store.issues[IssueSource::Team].items.is_empty());
}

// ------------------------------------------------------------ new issue

#[test]
fn creating_an_issue_requires_a_title() {
    let mut app = app_with(vec![]);
    app.store.teams =
        vec![serde_json::from_str(r#"{"id":"t","name":"Core","key":"ENG"}"#).unwrap()];
    app.start_new_issue();
    app.submit_new_issue();
    assert!(app.outbox.requests.is_empty());
    assert!(app.view.new_issue.is_some(), "the form stays open");
}

#[test]
fn a_completed_form_queues_a_create_request() {
    let mut app = app_with(vec![]);
    app.store.teams =
        vec![serde_json::from_str(r#"{"id":"t","name":"Core","key":"ENG"}"#).unwrap()];
    app.start_new_issue();
    if let Some(form) = &mut app.view.new_issue {
        form.title.value = "Ship it".into();
        form.priority = Priority::Urgent;
    }
    app.submit_new_issue();

    assert!(app.view.new_issue.is_none());
    assert!(matches!(
        app.outbox.requests.front(),
        Some(Request::Issue(
            crate::core::usecase::issue::Request::Create { draft, .. }
        )) if draft.priority == Priority::Urgent
    ));
}

// ------------------------------------------------ editing an issue, its thread

use crate::core::usecase::issue::Request as IssueRequest;

/// An app on the detail view of ENG-1, whose thread has `c1` by me and `c2`
/// by Ada. I am `me`.
fn app_on_thread() -> App {
    let mut app = app_with(vec![issue("1", "ENG-1", "Old title")]);
    app.store.viewer_id = Some(UserId::from("me"));
    app.open_selected();
    let current = app.store.current_issue.as_mut().unwrap();
    current.description = Some("Before".into());
    current.comments = Some(
        serde_json::from_str(
            r#"{"nodes":[
                {"id":"c1","body":"mine","createdAt":"2026-01-01T00:00:00.000Z","user":{"id":"me","name":"Me"}},
                {"id":"c2","body":"hers","createdAt":"2026-01-02T00:00:00.000Z","user":{"id":"u2","name":"Ada"}}
            ]}"#,
        )
        .unwrap(),
    );
    app.outbox.requests.clear();
    app
}

fn last_issue_request(app: &App) -> Option<&IssueRequest> {
    match app.outbox.requests.back() {
        Some(Request::Issue(request)) => Some(request),
        _ => None,
    }
}

#[test]
fn renaming_sends_the_new_title_and_an_unchanged_one_nothing() {
    let mut app = app_on_thread();
    app.start_rename();
    assert_eq!(app.view.input_mode, InputMode::Title);
    assert_eq!(app.view.title.value, "Old title");
    app.submit_rename();
    assert!(app.outbox.requests.is_empty());

    app.start_rename();
    app.view.title = Input::with("New title");
    app.submit_rename();
    assert!(matches!(
        last_issue_request(&app),
        Some(IssueRequest::Update { changes, .. }) if changes.title.as_deref() == Some("New title")
    ));
    assert_eq!(app.store.current_issue.as_ref().unwrap().title, "New title");
}

#[test]
fn without_a_terminal_the_description_is_written_in_a_field() {
    let mut app = app_on_thread();
    app.start_description();
    assert_eq!(app.view.input_mode, InputMode::Description);
    assert_eq!(app.view.description.value, "Before");
    app.view.description = Input::with("After");
    app.submit_description();
    assert!(matches!(
        last_issue_request(&app),
        Some(IssueRequest::Update { changes, .. }) if changes.description.as_deref() == Some("After")
    ));
}

#[test]
fn with_a_terminal_the_description_goes_to_the_editor() {
    let mut app = app_on_thread();
    app.external_editor = true;
    app.start_description();
    let job = app.outbox.editor.take().expect("an editor job");
    assert_eq!(job.text, "Before");
    assert_eq!(app.view.input_mode, InputMode::Normal);

    app.description_edited(&job.issue_id, None);
    app.description_edited(&job.issue_id, Some("Before\n".into()));
    assert!(
        app.outbox.requests.is_empty(),
        "quit or unchanged: nothing sent"
    );
    app.description_edited(&job.issue_id, Some("After".into()));
    assert!(matches!(
        last_issue_request(&app),
        Some(IssueRequest::Update { .. })
    ));
}

#[test]
fn only_my_comments_are_offered_to_edit_but_all_to_reply_to() {
    let mut app = app_on_thread();
    assert_eq!(app.pickable_comments(CommentAction::Edit).len(), 1);
    assert_eq!(app.pickable_comments(CommentAction::Reply).len(), 2);
    app.open_edit_comment_pick();
    assert_eq!(app.view.popup, Popup::CommentPick(CommentAction::Edit));
}

#[test]
fn an_edited_comment_is_sent_as_an_edit() {
    let mut app = app_on_thread();
    app.open_edit_comment_pick();
    app.apply_popup();
    assert_eq!(app.view.input_mode, InputMode::Comment);
    assert_eq!(app.view.comment.value, "mine");
    app.view.comment = Input::with("mine, fixed");
    app.submit_comment();
    assert_eq!(
        last_issue_request(&app),
        Some(&IssueRequest::EditComment {
            issue_id: "1".into(),
            comment_id: "c1".into(),
            body: "mine, fixed".into(),
        })
    );
    assert_eq!(app.view.comment_target, CommentTarget::New);
}

#[test]
fn a_reply_is_posted_in_the_thread_picked() {
    let mut app = app_on_thread();
    app.open_reply_pick();
    app.popup_pick(1);
    app.view.comment = Input::with("thanks");
    app.submit_comment();
    assert!(matches!(
        last_issue_request(&app),
        Some(IssueRequest::Comment { parent_id: Some(p), .. }) if p == "c2"
    ));
}

#[test]
fn a_comment_picked_to_delete_is_deleted() {
    let mut app = app_on_thread();
    app.open_delete_comment_pick();
    app.apply_popup();
    assert!(matches!(
        last_issue_request(&app),
        Some(IssueRequest::DeleteComment { comment_id, .. }) if comment_id == "c1"
    ));
}

#[test]
fn with_no_comments_of_mine_the_picker_says_so() {
    let mut app = app_on_thread();
    app.store
        .current_issue
        .as_mut()
        .unwrap()
        .comments
        .as_mut()
        .unwrap()
        .nodes
        .retain(|c| c.id == "c2");
    app.open_delete_comment_pick();
    assert_eq!(app.view.popup, Popup::None);
    assert_eq!(
        app.view.status_message.as_deref(),
        Some("No comments of yours here")
    );
}

#[test]
fn search_results_are_not_narrowed_to_active() {
    let mut app = app_with(vec![]);
    app.handle_message(Message::SearchResults {
        term: "x".into(),
        team_id: Some("t".into()),
        issues: vec![stated("1", "s", "Done", "completed")],
    });
    assert_eq!(app.visible_issues().len(), 1);
    app.clear_search();
    assert_eq!(app.preset(), Preset::Active);
}

#[test]
fn a_created_issue_appears_at_the_top() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.handle_message(Message::IssueCreated {
        team_id: "t".into(),
        issue: Box::new(issue("2", "ENG-2", "new")),
    });
    assert_eq!(
        app.store.issues[IssueSource::Team].items[0].identifier,
        "ENG-2"
    );
    assert_eq!(app.view.lists[IssueSource::Team].selected, 0);
}

// -------------------------------------------------------------- clipboard

#[test]
fn copying_reports_when_there_is_nothing_to_copy() {
    let mut app = app_with(vec![]);
    app.copy_url();
    assert!(app.outbox.clipboard.is_none());
    assert!(app.view.status_message.is_some());
}

#[test]
fn copying_an_identifier_queues_the_clipboard_write() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.copy_identifier();
    assert_eq!(app.outbox.clipboard.as_deref(), Some("ENG-1"));
}

// -------------------------------------------------- Linear-aligned keys

#[test]
fn assign_to_me_sets_the_viewer_as_assignee() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.store.viewer_id = Some("u1".into());
    app.store.team_contexts.insert(
        "t".into(),
        TeamContext {
            states: Vec::new(),
            members: vec![member("u1", "Me")],
        },
    );

    app.assign_to_me();

    assert_eq!(
        app.store.issues[IssueSource::Team].items[0]
            .assignee
            .as_ref()
            .unwrap()
            .id,
        "u1"
    );
    assert!(matches!(
        app.outbox.requests.front(),
        Some(Request::Issue(
            crate::core::usecase::issue::Request::SetAssignee { .. }
        ))
    ));
}

#[test]
fn assign_to_me_waits_for_the_viewer() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.store.viewer_id = None;
    app.assign_to_me();
    assert!(app.outbox.requests.is_empty());
    assert!(app.view.status_message.is_some());
}

/// Linear's Shift+1…Shift+4 set a priority without opening the menu.
#[test]
fn set_priority_applies_without_a_popup() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.set_priority(Priority::Urgent);
    assert_eq!(
        app.store.issues[IssueSource::Team].items[0].priority,
        Priority::Urgent
    );
    assert_eq!(app.view.popup, Popup::None);
    assert!(matches!(
        app.outbox.requests.front(),
        Some(Request::Issue(
            crate::core::usecase::issue::Request::SetPriority {
                priority: Priority::Urgent,
                ..
            }
        ))
    ));
}

// ------------------------------------------------------ grouping / nav

fn stated(id: &str, state_id: &str, name: &str, kind: &str) -> Issue {
    serde_json::from_str(&format!(
        r#"{{"id":"{id}","identifier":"ENG-{id}","title":"t{id}","priority":0,
            "state":{{"id":"{state_id}","name":"{name}","type":"{kind}","position":1}},
            "assignee":null,"description":null,"comments":null,"project":null,"cycle":null}}"#
    ))
    .unwrap()
}

fn team(id: &str, name: &str) -> Team {
    serde_json::from_str(&format!(
        r#"{{"id":"{id}","name":"{name}","key":"{name}"}}"#
    ))
    .unwrap()
}

fn view(id: &str, name: &str, shared: bool) -> CustomView {
    serde_json::from_str(&format!(
        r#"{{"id":"{id}","name":"{name}","shared":{shared}}}"#
    ))
    .unwrap()
}

#[test]
fn a_team_list_opens_on_active_and_hides_done_work() {
    let mut app = app_with(vec![
        stated("1", "s-todo", "Todo", "unstarted"),
        stated("2", "s-done", "Done", "completed"),
    ]);
    assert_eq!(app.preset(), Preset::Active);
    assert_eq!(app.visible_issues().len(), 1);
    app.set_preset(Preset::All);
    assert_eq!(app.visible_issues().len(), 2);
}

/// A saved view already chose its issues; the client must not narrow them.
#[test]
fn a_saved_view_opens_on_all() {
    let mut app = app_with(vec![]);
    app.store.custom_views = vec![view("v", "Mine", false)];
    app.store.issues[IssueSource::View].items = vec![stated("2", "s-done", "Done", "completed")];
    app.store.issues[IssueSource::View].of = Some(ListOf::ViewIssues("v".into()));
    app.store.issues[IssueSource::View].loaded = true;
    app.activate(Nav::View(0));
    assert_eq!(app.preset(), Preset::All);
    assert_eq!(app.visible_issues().len(), 1);
}

#[test]
fn issues_are_listed_in_group_order() {
    let mut app = app_with(vec![
        stated("1", "s-todo", "Todo", "unstarted"),
        stated("2", "s-prog", "In Progress", "started"),
        stated("3", "s-todo", "Todo", "unstarted"),
    ]);
    let ids: Vec<_> = app.visible_issues().iter().map(|i| i.id.clone()).collect();
    assert_eq!(ids, ["2", "1", "3"]);
    // Header rows interleave with the issues.
    let layout = app.list_view().rows;
    assert!(matches!(layout[0], ListRow::Group { count: 1, .. }));
    assert!(matches!(layout[2], ListRow::Group { count: 2, .. }));
    app.view.group_by = GroupBy::None;
    assert!(
        app.list_view()
            .rows
            .iter()
            .all(|r| matches!(r, ListRow::Issue { .. }))
    );
}

#[test]
fn folding_a_group_keeps_the_cursor_on_a_visible_issue() {
    let mut app = app_with(vec![
        stated("1", "s-prog", "In Progress", "started"),
        stated("2", "s-todo", "Todo", "unstarted"),
        stated("3", "s-todo", "Todo", "unstarted"),
    ]);
    app.view.lists[IssueSource::Team].selected = 2;
    app.toggle_selected_group();
    assert_eq!(app.visible_issues().len(), 1);
    assert_eq!(app.view.lists[IssueSource::Team].selected, 0);
    app.toggle_group("status:s-todo");
    assert_eq!(app.visible_issues().len(), 3);
}

#[test]
fn folding_all_groups_then_again_unfolds_them() {
    let mut app = app_with(vec![
        stated("1", "a", "In Progress", "started"),
        stated("2", "b", "Todo", "unstarted"),
    ]);
    app.toggle_all_groups();
    assert!(app.visible_issues().is_empty());
    app.toggle_all_groups();
    assert_eq!(app.visible_issues().len(), 2);
}

#[test]
fn switching_a_team_preset_refetches_that_slice() {
    let mut app = app_with(vec![]);
    app.store.teams = vec![team("t1", "Core")];
    app.set_preset(Preset::Backlog);
    assert!(matches!(
        app.outbox.requests.back(),
        Some(Request::Issue(
            crate::core::usecase::issue::Request::TeamIssues {
                preset: Preset::Backlog,
                after: None,
                ..
            }
        ))
    ));
}

#[test]
fn a_page_for_a_preset_already_left_is_dropped() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.handle_message(Message::Issues {
        team_id: "t".into(),
        preset: Preset::All,
        page: Page::new(
            vec![issue("9", "ENG-9", "stale")],
            PageInfo::default(),
            false,
        ),
    });
    assert_eq!(app.store.issues[IssueSource::Team].items[0].id, "1");
}

#[test]
fn a_sidebar_team_entry_switches_team() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.store.teams = vec![team("t1", "Core"), team("t2", "Ops")];
    app.activate(Nav::Team(1, TeamSection::Cycles));
    assert_eq!(app.nav.team, 1);
    assert_eq!(app.nav.screen, Screen::CycleList);
    assert!(
        app.store.issues[IssueSource::Team].items.is_empty(),
        "the old team's issues are dropped"
    );
    assert!(
        app.outbox
            .requests
            .iter()
            .any(|r| matches!(r, Request::Cycle(crate::core::usecase::cycle::Request::TeamCycles { team_id, .. }) if team_id == "t2"))
    );
}

fn fav(json: &str) -> Favorite {
    serde_json::from_str(json).unwrap()
}

fn sidebar_navs(app: &App) -> Vec<Option<Nav>> {
    app.frame
        .sidebar_rows
        .iter()
        .filter_map(|r| match r {
            SidebarRow::Item(i) => Some(i.nav()),
            _ => None,
        })
        .collect()
}

/// Only the current team's pages are listed; other teams are reached
/// through the switcher, not a tree of every team.
#[test]
fn the_sidebar_shows_only_the_current_team() {
    let mut app = app_with(vec![]);
    app.store.teams = vec![team("t1", "Core"), team("t2", "Ops")];
    app.store.custom_views = vec![view("v1", "Today", false)];
    app.frame.sidebar_rows = app.sidebar_layout();

    let navs = sidebar_navs(&app);
    assert!(navs.contains(&Some(Nav::Team(0, TeamSection::Projects))));
    assert!(!navs.contains(&Some(Nav::Team(1, TeamSection::Projects))));
    assert!(
        !navs.contains(&Some(Nav::View(0))),
        "views are not expanded"
    );
    assert!(app.frame.sidebar_rows.iter().any(|r| matches!(
        r,
        SidebarRow::Item(i) if i.action == SidebarAction::SwitchTeam
    )));

    app.view.sidebar.index = 0;
    for _ in 0..app.frame.sidebar_rows.len() {
        app.sidebar_move(1);
        assert!(matches!(
            app.frame.sidebar_rows[app.view.sidebar.index],
            SidebarRow::Item(_)
        ));
    }
}

#[test]
fn the_team_switcher_goes_to_the_picked_team() {
    let mut app = app_with(vec![]);
    app.store.teams = vec![team("t1", "Core"), team("t2", "Ops")];
    app.nav.dest = Nav::MyIssues;
    app.run_sidebar_action(SidebarAction::SwitchTeam);
    assert_eq!(app.view.popup, Popup::TeamSelect);
    app.view.popup_index = 1;
    app.select_team();
    assert_eq!(app.nav.dest, Nav::Team(1, TeamSection::Issues));
    assert_eq!(app.nav.team, 1);
}

fn workspace(id: &str, name: &str, current: bool) -> Workspace {
    Workspace {
        organization: Organization {
            id: id.into(),
            name: name.into(),
            url_key: name.to_lowercase(),
        },
        current,
    }
}

#[test]
fn the_workspace_switcher_asks_the_main_loop_to_switch() {
    let mut app = app_with(vec![]);
    app.workspaces = vec![
        workspace("o1", "Acme", false),
        workspace("o2", "Globex", true),
    ];
    app.open_workspace_select();
    assert_eq!(app.view.popup, Popup::WorkspaceSelect);
    assert_eq!(app.view.popup_index, 1, "starts on the current workspace");

    // Picking the current one changes nothing.
    app.select_workspace();
    assert_eq!(app.switch_to, None);

    app.open_workspace_select();
    app.popup_type('a');
    app.popup_type('c');
    app.apply_popup();
    assert_eq!(app.view.popup, Popup::None);
    assert_eq!(app.switch_to, Some("o1".into()));
}

#[test]
fn with_one_workspace_the_switcher_says_how_to_add_another() {
    let mut app = app_with(vec![]);
    app.workspaces = vec![workspace("o1", "Acme", true)];
    app.open_workspace_select();
    assert_eq!(app.view.popup, Popup::None);
    assert!(
        app.view
            .status_message
            .as_deref()
            .is_some_and(|m| m.contains("linear-tui auth login"))
    );
}

#[test]
fn switching_team_keeps_the_page() {
    let mut app = app_with(vec![]);
    app.store.teams = vec![team("t1", "Core"), team("t2", "Ops")];
    app.go_to_team_section(TeamSection::Cycles);
    app.open_team_select();
    app.view.popup_index = 1;
    app.select_team();
    assert_eq!(app.nav.dest, Nav::Team(1, TeamSection::Cycles));
}

#[test]
fn favorites_are_listed_in_linear_order_with_folders() {
    let mut app = app_with(vec![]);
    app.handle_message(Message::Favorites(vec![
        fav(r#"{"id":"b","type":"document","title":"Second","sortOrder":2}"#),
        fav(r#"{"id":"f","type":"folder","folderName":"Box","sortOrder":3}"#),
        fav(r#"{"id":"a","type":"document","title":"First","sortOrder":1}"#),
        fav(r#"{"id":"c","type":"document","title":"Inside","sortOrder":1,"parent":{"id":"f"}}"#),
    ]));
    app.frame.sidebar_rows = app.sidebar_layout();
    let labels: Vec<_> = app
        .frame
        .sidebar_rows
        .iter()
        .filter_map(|r| match r {
            SidebarRow::Item(i)
                if i.tone == Tone::Strong && i.action != SidebarAction::SwitchTeam =>
            {
                Some((i.label.clone(), i.depth))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        labels,
        [
            ("First".into(), 0),
            ("Second".into(), 0),
            ("Box".into(), 0),
            ("Inside".into(), 1)
        ]
    );

    let folder = app
        .store
        .favorites
        .iter()
        .position(|f| f.id == "f")
        .unwrap();
    app.toggle_folder(folder);
    assert!(
        !app.frame
            .sidebar_rows
            .iter()
            .any(|r| matches!(r, SidebarRow::Item(i) if i.label == "Inside"))
    );
}

#[test]
fn a_view_favorite_opens_the_view_itself() {
    let mut app = app_with(vec![]);
    app.store.custom_views = vec![view("v1", "Today", false)];
    app.store.favorites = vec![fav(
        r#"{"id":"x","type":"customView","customView":{"id":"v1"},"sortOrder":1}"#,
    )];
    assert_eq!(app.favorite_action(0), SidebarAction::Go(Nav::View(0)));
}

#[test]
fn a_project_favorite_opens_the_project_and_esc_returns_to_the_sidebar() {
    let mut app = app_with(vec![]);
    app.store.favorites = vec![fav(
        r#"{"id":"x","type":"project","sortOrder":1,"project":{"id":"p1","name":"Launch","lead":null}}"#,
    )];
    app.activate(Nav::Favorite(0));
    assert_eq!(app.nav.screen, Screen::ProjectDetail);
    assert_eq!(app.nav.current_project.as_ref().unwrap().id, "p1");
    assert!(matches!(
        app.outbox.requests.back(),
        Some(Request::Issue(
            crate::core::usecase::issue::Request::ProjectIssues { .. }
        ))
    ));
    app.leave_container();
    assert!(app.view.sidebar.focus);
    assert_eq!(
        app.nav.screen,
        Screen::ProjectDetail,
        "nothing behind it to go back to"
    );
}

#[test]
fn an_issue_favorite_opens_the_detail_and_esc_restores_where_you_were() {
    let mut app = app_with(vec![]);
    app.store.favorites = vec![fav(r#"{"id":"x","type":"issue","sortOrder":1,
            "issue":{"id":"i1","identifier":"ENG-1","title":"Bug"}}"#)];
    app.nav.dest = Nav::MyIssues;
    app.activate(Nav::Favorite(0));
    assert_eq!(app.nav.screen, Screen::IssueDetail);
    assert_eq!(app.nav.dest, Nav::Favorite(0));
    assert!(matches!(
        app.outbox.requests.back(),
        Some(Request::Issue(
            crate::core::usecase::issue::Request::Detail { .. }
        ))
    ));
    app.close_detail();
    assert_eq!(app.nav.dest, Nav::MyIssues);
    assert_eq!(app.nav.screen, Screen::IssueList);
}

#[test]
fn a_favorite_with_no_page_here_opens_in_the_browser() {
    let mut app = app_with(vec![]);
    app.store.favorites = vec![fav(
        r#"{"id":"x","type":"document","sortOrder":1,"url":"https://linear.app/d"}"#,
    )];
    let before = app.nav.dest;
    app.activate(Nav::Favorite(0));
    assert_eq!(app.nav.dest, before);
    assert!(
        matches!(app.outbox.requests.back(), Some(Request::Favorite(crate::core::usecase::favorite::Request::OpenInBrowser(u))) if u == "https://linear.app/d")
    );
}

#[test]
fn a_views_refetch_keeps_the_open_view_by_identity() {
    let mut app = app_with(vec![]);
    app.store.custom_views = vec![view("a", "A", false), view("b", "B", false)];
    app.nav.dest = Nav::View(1);
    app.handle_message(Message::CustomViews(vec![
        view("b", "B", false),
        view("z", "Z", true),
    ]));
    assert_eq!(app.nav.dest, Nav::View(0));
    assert_eq!(app.store.custom_views[0].id, "b");
}

fn scoped_view(id: &str, name: &str, team: Option<&str>, model: &str) -> CustomView {
    let mut v = view(id, name, true);
    v.team = team.map(|t| {
        serde_json::from_str(&format!(r#"{{"id":"{t}","name":"{t}","key":"{t}"}}"#)).unwrap()
    });
    v.model_name = Some(model.into());
    v
}

/// Like Linear: the workspace Views page lists views with no team, each
/// team's page lists its own, and the tab splits issue from project views.
#[test]
fn views_pages_list_by_scope_and_kind() {
    let mut app = app_with(vec![]);
    app.store.teams = vec![team("t1", "Core")];
    app.handle_message(Message::CustomViews(vec![
        scoped_view("a", "Workspace bugs", None, "Issue"),
        scoped_view("b", "Core board", Some("t1"), "Issue"),
        scoped_view("c", "Roadmap", None, "Project"),
        scoped_view("d", "Core roadmap", Some("t1"), "Project"),
    ]));
    let names = |app: &App| -> Vec<String> {
        app.listed_views()
            .into_iter()
            .map(|i| app.store.custom_views[i].name.clone())
            .collect()
    };

    app.activate(Nav::Views);
    assert_eq!(names(&app), ["Workspace bugs"]);
    app.cycle_view_kind();
    assert_eq!(names(&app), ["Roadmap"]);

    app.activate(Nav::Team(0, TeamSection::Views));
    assert_eq!(app.nav.screen, Screen::ViewList);
    assert_eq!(names(&app), ["Core roadmap"]);
    app.set_view_kind(ViewKind::Issues);
    assert_eq!(names(&app), ["Core board"]);
}

#[test]
fn a_project_view_opens_as_a_project_list() {
    let mut app = app_with(vec![]);
    app.store.custom_views = vec![scoped_view("p", "Roadmap", None, "Project")];
    app.activate(Nav::View(0));
    assert_eq!(app.nav.screen, Screen::ProjectList);
    assert!(matches!(
        app.outbox.requests.back(),
        Some(Request::Project(
            crate::core::usecase::project::Request::ViewProjects { after: None, .. }
        ))
    ));

    let project: Project =
        serde_json::from_str(r#"{"id":"p1","name":"Launch","lead":null}"#).unwrap();
    app.handle_message(Message::ViewProjects {
        view_id: "p".into(),
        page: Page::new(vec![project], PageInfo::default(), false),
    });
    assert_eq!(
        app.project_rows().len(),
        1,
        "rows come from the view, not the team"
    );
    app.open_project_detail();
    assert_eq!(app.nav.current_project.as_ref().unwrap().id, "p1");
    app.leave_container();
    assert_eq!(app.nav.screen, Screen::ProjectList, "back to the view");
}

#[test]
fn opening_the_same_project_view_again_does_not_refetch() {
    let mut app = app_with(vec![]);
    app.store.custom_views = vec![scoped_view("p", "Roadmap", None, "Project")];
    app.store.view_projects.of = Some(ListOf::ViewProjects("p".into()));
    app.store.view_projects.loaded = true;
    app.activate(Nav::View(0));
    assert!(app.outbox.requests.is_empty());
}

#[test]
fn personal_views_sort_above_shared_ones() {
    let mut app = app_with(vec![]);
    app.handle_message(Message::CustomViews(vec![
        view("1", "Team board", true),
        view("2", "zzz mine", false),
    ]));
    assert!(!app.store.custom_views[0].shared);
}

#[test]
fn stepping_through_issues_from_the_detail_view() {
    let mut app = app_with(vec![
        stated("1", "s", "Todo", "unstarted"),
        stated("2", "s", "Todo", "unstarted"),
    ]);
    app.open_issue_detail();
    assert_eq!(app.detail_position(), Some((0, 2)));
    app.step_issue(1);
    assert_eq!(app.store.current_issue.as_ref().unwrap().id, "2");
    assert_eq!(app.view.lists[IssueSource::Team].selected, 1);
    app.step_issue(1);
    assert_eq!(
        app.store.current_issue.as_ref().unwrap().id,
        "2",
        "clamped at the end"
    );
    app.close_detail();
    assert_eq!(app.nav.screen, Screen::IssueList);
}

#[test]
fn the_detail_view_returns_to_the_project_it_came_from() {
    let mut app = app_with(vec![]);
    app.store.issues[IssueSource::Project].items = vec![stated("1", "s", "Todo", "unstarted")];
    app.nav.screen = Screen::ProjectDetail;
    app.open_issue_detail();
    assert_eq!(app.nav.screen, Screen::IssueDetail);
    app.close_detail();
    assert_eq!(app.nav.screen, Screen::ProjectDetail);
}

// ------------------------------------------------------------------ mouse

fn clickable(app: &mut App) {
    app.frame.list_area = Rect::new(30, 5, 80, 20);
    app.frame.list_rows = app.list_view().rows;
}

#[test]
fn clicking_a_row_selects_it_and_clicking_again_opens_it() {
    let mut app = app_with(vec![
        stated("1", "s", "Todo", "unstarted"),
        stated("2", "s", "Todo", "unstarted"),
    ]);
    clickable(&mut app);
    // Row 0 is the "Todo" header, rows 1 and 2 the issues.
    app.click(40, 7);
    assert_eq!(app.view.lists[IssueSource::Team].selected, 1);
    assert_eq!(app.nav.screen, Screen::IssueList);
    app.click(40, 7);
    assert_eq!(app.nav.screen, Screen::IssueDetail);
    assert_eq!(app.store.current_issue.as_ref().unwrap().id, "2");
}

#[test]
fn clicking_a_group_header_folds_it() {
    let mut app = app_with(vec![stated("1", "s", "Todo", "unstarted")]);
    clickable(&mut app);
    app.click(40, 5);
    assert!(app.view.collapsed_groups.contains("status:s"));
}

#[test]
fn clicking_a_preset_chip_switches_preset() {
    let mut app = app_with(vec![]);
    app.frame.chip_areas = vec![(Rect::new(30, 3, 8, 1), Chip::Preset(Preset::Backlog))];
    app.click(32, 3);
    assert_eq!(app.preset(), Preset::Backlog);
}

#[test]
fn clicking_a_sidebar_entry_navigates() {
    let mut app = app_with(vec![]);
    app.store.viewer_id = Some("u".into());
    app.frame.sidebar_rows = app.sidebar_layout();
    app.frame.sidebar_area = Rect::new(0, 2, 25, 30);
    // Row 0 is My Issues.
    app.click(10, 2);
    assert_eq!(app.nav.dest, Nav::MyIssues);
    assert!(matches!(
        app.outbox.requests.back(),
        Some(Request::Issue(
            crate::core::usecase::issue::Request::MyIssues { .. }
        ))
    ));
}

#[test]
fn clicking_a_popup_entry_applies_it() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.open_priority_change();
    app.frame.popup_area = Rect::new(10, 10, 30, 5);
    app.click(15, 11); // second entry: Urgent
    assert_eq!(app.view.popup, Popup::None);
    assert_eq!(
        app.store.issues[IssueSource::Team].items[0].priority,
        Priority::Urgent
    );
}

#[test]
fn clicking_outside_a_popup_closes_it() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.open_priority_change();
    app.frame.popup_area = Rect::new(10, 10, 30, 5);
    app.click(0, 0);
    assert_eq!(app.view.popup, Popup::None);
    assert!(app.outbox.requests.is_empty());
}

// ------------------------------------------------------- stale responses

fn page(issues: Vec<Issue>, append: bool) -> Page<Issue> {
    Page::new(issues, PageInfo::default(), append)
}

#[test]
fn a_page_for_a_team_already_left_is_dropped() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.handle_message(Message::Issues {
        team_id: "other".into(),
        preset: Preset::Active,
        page: page(vec![issue("9", "OPS-9", "stale")], false),
    });
    assert_eq!(app.store.issues[IssueSource::Team].items[0].id, "1");
}

#[test]
fn switching_team_forgets_the_old_teams_cursors() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.store.teams.push(team("t2", "Ops"));
    app.store.issues[IssueSource::Team].page_info = PageInfo {
        has_next_page: true,
        end_cursor: Some("c1".into()),
    };
    app.activate(Nav::Team(1, TeamSection::Issues));
    assert!(!app.store.issues[IssueSource::Team].page_info.has_next_page);
    assert!(app.outbox.requests.contains(&Request::Team(
        crate::core::usecase::team::Request::Context {
            team_id: "t2".into()
        }
    )));
}

// ------------------------------------------------------- per-team context

fn member(id: &str, name: &str) -> User {
    serde_json::from_str(&format!(
        r#"{{"id":"{id}","name":"{name}","displayName":"{name}"}}"#
    ))
    .unwrap()
}

/// An issue of team `team_id`, in state `state_id`.
fn of_team(id: &str, team_id: &str, state_id: &str) -> Issue {
    let mut issue = stated(id, state_id, "State", "started");
    issue.team = Some(serde_json::from_str(&format!(r#"{{"id":"{team_id}"}}"#)).unwrap());
    issue
}

fn state(id: &str, name: &str) -> WorkflowState {
    stated("0", id, name, "started").state.unwrap()
}

fn context_message(team_id: &str, states: &[(&str, &str)], members: Vec<User>) -> Message {
    Message::TeamContext {
        team_id: team_id.into(),
        states: states.iter().map(|(id, name)| state(id, name)).collect(),
        members,
    }
}

/// Team A ("t") is selected; My Issues holds an issue of team B.
fn my_issues_with_team_b_issue() -> App {
    let mut app = app_with(vec![]);
    app.store.teams.push(team("b", "Ops"));
    app.handle_message(context_message(
        "t",
        &[("a-todo", "Todo"), ("a-done", "Done")],
        vec![member("ua", "Ann")],
    ));
    app.nav.dest = Nav::MyIssues;
    app.store.issues[IssueSource::My].items = vec![of_team("1", "b", "b-doing")];
    app.outbox.requests.clear();
    app
}

#[test]
fn status_popup_on_another_teams_issue_lists_that_teams_states() {
    let mut app = my_issues_with_team_b_issue();

    app.open_status_change();

    // Team B is not loaded yet: fetch it, and offer nothing to pick meanwhile.
    assert_eq!(app.view.popup, Popup::StatusChange("1".into()));
    assert!(app.outbox.requests.contains(&Request::Team(
        crate::core::usecase::team::Request::Context {
            team_id: "b".into()
        }
    )));
    assert!(app.popup_loading());
    assert_eq!(app.popup_list_len(), 0);
    app.apply_popup();
    assert_eq!(app.view.popup, Popup::StatusChange("1".into()));

    app.handle_message(context_message(
        "b",
        &[("b-todo", "Backlog"), ("b-doing", "Doing")],
        vec![member("ub", "Bob")],
    ));

    assert!(!app.popup_loading());
    let names: Vec<&str> = app.popup_states().iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Backlog", "Doing"]);
    // Starts on the issue's current state.
    assert_eq!(app.view.popup_index, 1);

    app.view.popup_index = 0;
    app.apply_popup();
    assert!(app.outbox.requests.contains(&Request::Issue(
        crate::core::usecase::issue::Request::SetStatus {
            issue_id: "1".into(),
            state_id: "b-todo".into(),
        }
    )));
    // Team A is still the selected team.
    assert_eq!(app.team_id(), Some("t".into()));
}

#[test]
fn a_loaded_team_is_not_fetched_again() {
    let mut app = my_issues_with_team_b_issue();
    app.handle_message(context_message("b", &[("b-doing", "Doing")], Vec::new()));
    app.outbox.requests.clear();

    app.open_status_change();

    assert!(app.outbox.requests.is_empty());
    assert_eq!(app.popup_list_len(), 1);
}

#[test]
fn a_team_in_flight_is_asked_for_once() {
    let mut app = my_issues_with_team_b_issue();
    app.open_status_change();
    app.close_popup();
    app.outbox.requests.clear();

    app.open_status_change();

    assert!(app.outbox.requests.is_empty());
}

#[test]
fn a_failed_team_context_is_asked_for_again() {
    let mut app = my_issues_with_team_b_issue();
    app.open_status_change();
    let request = app.outbox.requests.pop_front().unwrap();
    app.handle_message(Message::Failed {
        request: Box::new(request),
        error: "boom".into(),
    });
    app.view.error_popup = None;
    app.close_popup();

    app.open_status_change();

    assert!(app.outbox.requests.contains(&Request::Team(
        crate::core::usecase::team::Request::Context {
            team_id: "b".into()
        }
    )));
}

#[test]
fn assignee_popup_on_another_teams_issue_lists_that_teams_members() {
    let mut app = my_issues_with_team_b_issue();

    app.open_assignee_change();
    // Unassign needs no team, so it stays pickable while members load.
    assert_eq!(app.popup_list_len(), 1);
    app.view.popup_index = 1;
    app.apply_popup();
    assert_eq!(app.view.popup, Popup::AssigneeChange("1".into()));

    app.handle_message(context_message("b", &[], vec![member("ub", "Bob")]));

    let names: Vec<&str> = app
        .popup_members()
        .iter()
        .map(|u| u.name.as_str())
        .collect();
    assert_eq!(names, ["Bob"]);
}

#[test]
fn status_filter_offers_the_states_of_the_teams_in_the_list() {
    let mut app = my_issues_with_team_b_issue();
    app.store.issues[IssueSource::My]
        .items
        .push(of_team("2", "t", "a-todo"));
    app.handle_message(context_message(
        "b",
        &[("b-todo", "Todo"), ("b-doing", "Doing")],
        Vec::new(),
    ));

    app.open_filter();

    let names: Vec<&str> = app
        .filter_states()
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    // Team B's first, as its issue comes first; "Todo" once for both teams.
    assert_eq!(names, ["Todo", "Doing", "Done"]);
}

#[test]
fn issues_for_a_project_already_left_are_dropped() {
    let mut app = app_with(vec![]);
    app.nav.current_project =
        Some(serde_json::from_str(r#"{"id":"p2","name":"B","lead":null}"#).unwrap());
    app.nav.screen = Screen::ProjectDetail;
    app.handle_message(Message::ProjectIssues {
        project_id: "p1".into(),
        page: page(vec![issue("9", "ENG-9", "stale")], false),
    });
    assert!(app.store.issues[IssueSource::Project].items.is_empty());
    assert!(!app.store.issues[IssueSource::Project].loaded);
}

#[test]
fn a_first_page_for_a_view_no_longer_open_is_dropped() {
    let mut app = app_with(vec![]);
    app.store.custom_views = vec![view("v1", "Mine", false), view("v2", "Theirs", false)];
    app.nav.dest = Nav::View(1);
    app.handle_message(Message::ViewIssues {
        view_id: "v1".into(),
        page: page(vec![issue("9", "ENG-9", "stale")], false),
    });
    assert!(app.store.issues[IssueSource::View].items.is_empty());
    assert!(app.store.issues[IssueSource::View].of.is_none());
}

#[test]
fn search_results_for_a_team_already_left_are_dropped() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.handle_message(Message::SearchResults {
        term: "x".into(),
        team_id: Some("other".into()),
        issues: vec![issue("9", "OPS-9", "x")],
    });
    assert_eq!(app.store.issues[IssueSource::Team].items[0].id, "1");
    assert_eq!(app.nav.global_search, None);
}

// ------------------------------------------------------------- failures

#[test]
fn a_failed_mutation_rereads_the_issue_from_the_server() {
    let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
    app.set_priority(Priority::Urgent);
    let request = app.outbox.requests.pop_front().unwrap();
    app.handle_message(Message::Failed {
        request: Box::new(request),
        error: "nope".into(),
    });
    assert!(matches!(
        app.outbox.requests.front(),
        Some(Request::Issue(crate::core::usecase::issue::Request::Detail { issue_id })) if issue_id == "1"
    ));
    assert!(
        app.view
            .error_popup
            .as_deref()
            .unwrap()
            .contains("priority")
    );

    // Linear's answer puts every copy back.
    app.handle_message(Message::IssueDetail(Box::new(issue("1", "ENG-1", "a"))));
    assert_eq!(
        app.store.issues[IssueSource::Team].items[0].priority,
        Priority::None
    );
}

#[test]
fn a_failed_page_can_be_requested_again() {
    let mut app = app_with(
        (0..10)
            .map(|i| issue(&i.to_string(), &format!("ENG-{i}"), "t"))
            .collect(),
    );
    app.store.issues[IssueSource::Team].page_info = PageInfo {
        has_next_page: true,
        end_cursor: Some("c1".into()),
    };
    app.select_last();
    let request = app.outbox.requests.pop_front().unwrap();
    app.handle_message(Message::Failed {
        request: Box::new(request),
        error: "timeout".into(),
    });
    app.move_selection(-1);
    app.move_selection(1);
    assert!(matches!(
        app.outbox.requests.front(),
        Some(Request::Issue(crate::core::usecase::issue::Request::TeamIssues { after: Some(c), .. })) if c == "c1"
    ));
}

// ------------------------------------------------------------- selection

/// Regression: a page that grouped rows in above the cursor moved the
/// selection — and the open popup's target — to another issue.
#[test]
fn an_appended_page_keeps_the_cursor_and_the_popup_on_their_issue() {
    let mut app = app_with(vec![stated("a", "s-todo", "Todo", "unstarted")]);
    app.open_priority_change();
    app.handle_message(Message::Issues {
        team_id: "t".into(),
        preset: Preset::Active,
        page: page(vec![stated("b", "s-doing", "Doing", "started")], true),
    });
    // In Progress groups above Todo, so the new issue lands on row 0.
    assert_eq!(app.visible_issues()[0].id, "b");
    assert_eq!(app.focused_issue().unwrap().id, "a");

    app.view.popup_index = Priority::High.as_index();
    app.apply_priority_selection();
    assert!(matches!(
        app.outbox.requests.front(),
        Some(Request::Issue(crate::core::usecase::issue::Request::SetPriority { issue_id, .. })) if issue_id == "a"
    ));
}

/// Regression: the prefetch compared the cursor with the unfiltered list,
/// so a narrowed list could reach its last row without loading more.
#[test]
fn a_filtered_list_still_prefetches_at_its_last_visible_row() {
    let mut app = app_with(
        (0..20)
            .map(|i| issue(&i.to_string(), &format!("ENG-{i}"), &format!("t{i}")))
            .collect(),
    );
    app.store.issues[IssueSource::Team].page_info = PageInfo {
        has_next_page: true,
        end_cursor: Some("c1".into()),
    };
    for c in "t19".chars() {
        app.list_mut().search.insert(c);
    }
    app.move_selection(1);
    assert!(matches!(
        app.outbox.requests.front(),
        Some(Request::Issue(
            crate::core::usecase::issue::Request::TeamIssues { after: Some(_), .. }
        ))
    ));
}

#[test]
fn filtering_resets_the_cursor_of_the_list_on_screen() {
    let mut app = app_with(vec![]);
    app.nav.dest = Nav::MyIssues;
    app.store.issues[IssueSource::My].items =
        vec![issue("1", "ENG-1", "a"), issue("2", "ENG-2", "b")];
    app.view.lists[IssueSource::My].selected = 1;
    app.clear_filters();
    assert_eq!(app.view.lists[IssueSource::My].selected, 0);
}

/// Regression: the search box and filters were one set shared by every
/// list, so narrowing a project's issues also narrowed the team's.
#[test]
fn each_list_keeps_its_own_search_and_filters() {
    let mut app = app_with(vec![issue("1", "ENG-1", "alpha")]);
    app.nav.current_project =
        Some(serde_json::from_str(r#"{"id":"p1","name":"P","lead":null}"#).unwrap());
    app.nav.screen = Screen::ProjectDetail;
    app.store.issues[IssueSource::Project].items = vec![issue("2", "ENG-2", "beta")];
    app.start_search();
    for c in "zzz".chars() {
        app.list_mut().search.insert(c);
    }
    app.list_mut().filters.priority = Some(Priority::Urgent);
    assert!(app.visible_issues().is_empty());

    app.leave_container();
    app.nav.screen = Screen::IssueList;
    assert_eq!(app.visible_issues().len(), 1, "the team list is untouched");
}

#[test]
fn config_warnings_are_shown_at_startup() {
    let config = Config::parse("[ui]\ntheem = 1\n").unwrap();
    let app = App::new(&config);
    assert!(
        app.view
            .error_popup
            .as_deref()
            .unwrap()
            .contains("ui.theem")
    );
    assert!(App::new(&Config::default()).view.error_popup.is_none());
}
