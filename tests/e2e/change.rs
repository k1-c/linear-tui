//! Changing issues from the TUI, and checking Linear holds the change:
//! status, priority, assignee, comments, new issues, copying and opening.

use super::a;
use super::linear::titles::*;
use super::tui::Tui;

/// Covers: issue::set_status
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn a_status_change_reaches_linear() {
    let (account, seeded) = a();
    let id = seeded.issue(FOR_STATUS);
    let tui = Tui::start(&account, "");
    tui.open(id);
    let picker = tui.run("Change status");
    assert!(
        picker
            .overlay()
            .is_some_and(|o| o.starts_with("status picker"))
    );
    tui.type_text("In Progress");
    tui.press("<Enter>").expect("In Progress");
    assert_eq!(tui.issue(id)["state"]["name"], "In Progress");
}

/// Covers: issue::set_priority
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn a_priority_change_reaches_linear_by_key_and_by_picker() {
    let (account, seeded) = a();
    let id = seeded.issue(FOR_PRIORITY);
    let tui = Tui::start(&account, "");
    tui.open(id);
    tui.press("@");
    assert_eq!(tui.issue(id)["priority"], "High");
    tui.press("p");
    tui.type_text("low");
    tui.press("<Enter>");
    assert_eq!(tui.issue(id)["priority"], "Low");
}

/// Covers: issue::assign_to_me, issue::set_assignee
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn assigning_to_me_and_unassigning_reach_linear() {
    let (account, seeded) = a();
    let id = seeded.issue(FOR_ASSIGNEE);
    let tui = Tui::start(&account, "");
    tui.open(id);
    tui.press("i");
    assert_eq!(tui.issue(id)["assignee"], seeded.viewer_name.as_str());
    tui.press("a");
    tui.type_text("unassign");
    tui.press("<Enter>");
    assert!(tui.issue(id)["assignee"].is_null());
}

/// Covers: issue::comment
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn a_comment_is_posted_and_shows_in_the_thread() {
    let (account, seeded) = a();
    let id = seeded.issue(FOR_COMMENT);
    let body = format!("Posted by the end-to-end run {}", std::process::id());
    let tui = Tui::start(&account, "");
    tui.open(id);
    let field = tui.press("m");
    assert_eq!(field.focus(), "comment field");
    tui.type_text(&body);
    tui.press("<C-Enter>").expect(&body);
    let comments = tui.issue(id)["comments"].clone();
    assert!(
        comments
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["body"] == body.as_str()),
        "{comments}"
    );
}

/// Covers: issue::create, issue::created
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn a_new_issue_is_filed_and_listed() {
    let (account, _) = a();
    let title = format!("Filed from the TUI {}", std::process::id());
    let tui = Tui::start(&account, "");
    let form = tui.press("c");
    assert_eq!(form.focus(), "new issue: title");
    tui.type_text(&title);
    tui.press("<Enter>");
    tui.type_text("Written by the end-to-end run.");
    let screen = tui.press("<C-Enter>");
    screen.expect(&title);
    let status = screen.status().unwrap_or_default().to_string();
    let id = status
        .strip_prefix("Created ")
        .unwrap_or_else(|| panic!("no \"Created …\" status: {status:?}"));
    let issue = tui.issue(id);
    assert_eq!(issue["title"], title.as_str());
    assert_eq!(issue["description"], "Written by the end-to-end run.");
}

/// Covers: issue::copy, issue::open_url, project::open_url
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn an_issue_is_copied_and_opened_and_so_is_a_project() {
    let (account, seeded) = a();
    let id = seeded.issue(CHART);
    let tui = Tui::start(&account, "");
    tui.open(id);
    tui.press("y");
    let held = tui.press("o").held();
    assert!(
        held.iter().any(|h| h.contains(&format!("{id:?}"))),
        "{held:?}"
    );
    assert!(
        held.iter()
            .any(|h| h.starts_with("would open https://linear.app/") && h.contains(id)),
        "{held:?}"
    );
    let held = tui.press("g p o").held();
    assert!(
        held.iter().any(|h| h.contains("/project/")),
        "the project's page: {held:?}"
    );
}
