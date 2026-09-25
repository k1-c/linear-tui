//! More than one workspace: instances signed in to different workspaces
//! stay apart, and signing in to another workspace shows it.

use super::linear::titles::*;
use super::tui::Tui;
use super::{a, b};

/// Two instances, each in its own repository and signed in to its own
/// workspace, show and answer for their own workspace only.
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn instances_in_two_workspaces_stay_apart() {
    let (account_a, seeded_a) = a();
    let (account_b, seeded_b) = b();
    let tui_a = Tui::start(&account_a, "");
    let tui_b = Tui::start(&account_b, "");

    tui_a.screen().expect(CRASH).expect_not(IN_WORKSPACE_B);
    tui_b.screen().expect(IN_WORKSPACE_B).expect_not(CRASH);

    // Each repository's commands reach its own instance.
    tui_b.open(seeded_b.issue(IN_WORKSPACE_B));
    let context_a = tui_a.cli(&["context"]).unwrap();
    let context_b = tui_b.cli(&["context"]).unwrap();
    assert!(context_a.contains(&seeded_a.team_name), "{context_a}");
    assert!(
        context_b.contains(seeded_b.issue(IN_WORKSPACE_B)),
        "{context_b}"
    );
    assert!(
        !context_a.contains(seeded_b.issue(IN_WORKSPACE_B)),
        "{context_a}"
    );
}

/// Signing in with another workspace's key shows that workspace on the
/// next launch.
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn signing_in_to_another_workspace_shows_it() {
    let (account_a, _) = a();
    let (account_b, _) = b();
    let mut tui = Tui::start(&account_a, "");
    tui.screen().expect(CRASH);
    tui.cli(&["auth", "token", &account_b.api_key]).unwrap();
    tui.relaunch();
    tui.screen().expect(IN_WORKSPACE_B).expect_not(CRASH);
}
