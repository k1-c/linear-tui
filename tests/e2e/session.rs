//! Sessions: quitting and relaunching where the user left off.

use super::a;
use super::linear::titles::*;
use super::tui::Tui;

/// Covers: instance::to_reopen
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn a_relaunch_reopens_the_page_left() {
    let (account, _) = a();
    let mut tui = Tui::start(&account, "");
    tui.press("g p");
    tui.press("<Enter>")
        .expect("Hourly forecasts, rebuilt on the new API");
    tui.relaunch();
    let screen = tui.screen();
    assert!(screen.showing().ends_with(PROJECT), "{}", screen.showing());
    screen.expect(CRASH);
}
