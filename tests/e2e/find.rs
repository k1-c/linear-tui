//! Finding issues: the list filter, workspace search, the filter picker, and
//! the command palette.

use super::a;
use super::linear::titles::*;
use super::tui::{Tui, eventually};

/// Covers: issue::matches
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn typing_a_filter_narrows_the_list() {
    let (account, _) = a();
    let tui = Tui::start(&account, "");
    let field = tui.press("/");
    assert_eq!(field.focus(), "search field");
    tui.type_text("chart");
    tui.press("<Enter>").expect(CHART).expect_not(CRASH);
}

/// Covers: issue::search, issue::take_search_results
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn searching_all_of_linear_finds_issues_the_preset_hides() {
    let (account, seeded) = a();
    let tui = Tui::start(&account, "");
    // By this run's identifier: Linear keeps a search's answer for a while,
    // and a term searched before this run was seeded would still find the
    // issues it deleted.
    let id = seeded.issue(RADAR);
    eventually("search finds the radar issue", || {
        tui.press("/");
        tui.type_text(id);
        let screen = tui.press("<C-g>");
        screen.text().contains(RADAR).then_some(())
    });
}

/// Covers: issue::matches
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn the_filter_picker_narrows_by_status_and_priority() {
    let (account, _) = a();
    let tui = Tui::start(&account, "");
    tui.press("f");
    tui.type_text("todo");
    tui.press("<Enter>");
    tui.type_text("high");
    tui.press("<Enter>")
        .expect(CHART)
        .expect_not(FOR_STATUS)
        .expect_not(CRASH);
    tui.press("F").expect(CRASH);
}

/// Covers: issue::quick_search
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn the_palette_finds_an_issue_anywhere_in_the_workspace() {
    let (account, seeded) = a();
    let tui = Tui::start(&account, "");
    eventually("the palette finds the wind issue", || {
        let palette = tui.press("<C-k>");
        assert!(
            palette
                .overlay()
                .is_some_and(|o| o.starts_with("command palette"))
        );
        // By this run's identifier, as above.
        let found = tui.type_text(seeded.issue(WIND)).text().contains(WIND);
        if !found {
            tui.press("<Esc>");
        }
        found.then_some(())
    });
    let screen = tui.press("<Enter>");
    assert_eq!(screen.focus(), "issue_detail");
    screen.expect(WIND);
}

/// A title that names no single command fails, and changes nothing.
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn an_ambiguous_command_is_refused_with_the_choices() {
    let (account, _) = a();
    let tui = Tui::start(&account, "");
    let error = tui.run_fails("priority");
    assert!(error.contains("Set priority to Urgent"), "{error}");
    assert!(tui.screen().overlay().is_none());
}
