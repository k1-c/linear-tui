//! Browsing: the team's list and its slices, an issue, my issues, projects,
//! cycles, saved views, favorites, the next page, a refresh, another team.

use super::a;
use super::linear::titles::*;
use super::tui::Tui;

/// Covers: team::load, team::pick_initial, team::ensure_context,
/// team::context_arrived, user::load, user::take_viewer, view::ensure_views,
/// view::take_views, favorite::load, favorite::take_favorites,
/// issue::open_team_issues, issue::take_page
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn the_team_list_opens_on_its_active_issues() {
    let (account, seeded) = a();
    let tui = Tui::start(&account, "");
    let screen = tui.screen();
    assert_eq!(screen.showing(), format!("{} › Issues", seeded.team_name));
    assert_eq!(screen.focus(), "issue_list");
    screen.expect(CRASH).expect(CHART).expect(RETRY);
    // Active leaves backlog and closed work out.
    screen.expect_not(RADAR).expect_not(DOCS);
    // The sidebar lists the favorites.
    screen.expect(PROJECT).expect(ISSUE_VIEW);
}

/// Covers: issue::open_team_issues, issue::matches
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn the_backlog_and_all_issues_are_a_key_away() {
    let (account, _) = a();
    let tui = Tui::start(&account, "");
    tui.press("<S-Tab>")
        .expect(RADAR)
        .expect(WIND)
        .expect_not(CRASH);
    tui.press("g e").expect(DOCS).expect(CRASH).expect(RADAR);
}

/// Covers: issue::open, issue::take_detail, issue::ensure_thread,
/// issue::refresh
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn an_issue_shows_its_description_sub_issues_and_thread() {
    let (account, seeded) = a();
    let tui = Tui::start(&account, "");
    let screen = tui.open(seeded.issue(CRASH));
    assert_eq!(screen.focus(), "issue_detail");
    assert_eq!(
        screen.issue(),
        Some(format!("{} {CRASH}", seeded.issue(CRASH)).as_str())
    );
    screen
        .expect(CRASH)
        .expect("Pick a city with no data")
        .expect(RETRY)
        .expect(PROJECT);
    tui.open(seeded.issue(FOR_COMMENT)).expect(SEEDED_COMMENT);
    tui.press("<F5>").expect(SEEDED_COMMENT);
}

/// Covers: issue::open_my_issues
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn my_issues_lists_what_is_assigned_to_me() {
    let (account, _) = a();
    let tui = Tui::start(&account, "");
    let screen = tui.run("Go to my issues");
    assert_eq!(screen.showing(), "My Issues");
    screen.expect(CRASH).expect_not(CHART);
}

/// Covers: project::open_team_projects, project::take_page,
/// issue::open_project_issues
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn a_project_lists_its_issues() {
    let (account, _) = a();
    let tui = Tui::start(&account, "");
    tui.press("g p").expect(PROJECT);
    tui.press("<Enter>")
        .expect("Hourly forecasts, rebuilt on the new API")
        .expect(CRASH)
        .expect(RETRY)
        .expect(DOCS)
        .expect_not(CHART);
}

/// Covers: cycle::open_team_cycles, cycle::take_page, issue::open_cycle_issues
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn a_cycle_lists_its_issues() {
    let (account, _) = a();
    let tui = Tui::start(&account, "");
    let cycles = tui.press("g c");
    assert_eq!(cycles.focus(), "cycle_list");
    tui.press("<Enter>")
        .expect(CRASH)
        .expect(CHART)
        .expect_not(RADAR);
}

/// Covers: view::listed, issue::open_view_issues, project::open_view_projects
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn saved_views_open_as_issue_and_project_lists() {
    let (account, _) = a();
    let tui = Tui::start(&account, "");
    tui.press("g v").expect(ISSUE_VIEW);
    tui.press("<Enter>")
        .expect(WIND)
        .expect(CHART)
        .expect(CRASH)
        .expect_not(RADAR);
    tui.press("g v <S-Tab>").expect(PROJECT_VIEW);
    tui.press("<Enter>").expect(PROJECT);
}

/// Covers: favorite::target
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn a_favorite_opens_its_page() {
    let (account, _) = a();
    let tui = Tui::start(&account, "");
    // The sidebar's first rows: My Issues, Views, then the favorites.
    let screen = tui.press("<Tab> g j j <Enter>");
    screen
        .expect("Hourly forecasts, rebuilt on the new API")
        .expect(CRASH);
}

/// Covers: issue::next_page
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn scrolling_to_the_bottom_loads_the_next_page() {
    let (account, seeded) = a();
    let tui = Tui::start(&account, "[ui]\nitems_per_page = 2");
    let first = tui.screen();
    assert!(
        !first.text().contains(seeded.issue(FOR_ASSIGNEE)),
        "two to a page, the oldest active issue is not loaded yet:\n{}",
        first.text()
    );
    let mut screen = first;
    for _ in 0..10 {
        if screen.text().contains(FOR_ASSIGNEE) {
            break;
        }
        screen = tui.press("G");
    }
    screen.expect(FOR_ASSIGNEE).expect(CRASH);
}

/// Covers: issue::reload, project::reload, cycle::reload, view::reload_views
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn a_refresh_shows_what_changed_in_linear() {
    let (account, seeded) = a();
    let tui = Tui::start(&account, "");
    let filed = format!("Filed behind linear-tui's back {}", std::process::id());
    tui.cli(&[
        "issue",
        "create",
        "--team",
        &seeded.team_key,
        "--title",
        &filed,
    ])
    .unwrap();
    tui.screen().expect_not(&filed);
    tui.press("<F5>").expect(&filed);
    // Every list refreshes the same way.
    tui.press("g p <F5>").expect(PROJECT);
    tui.press("g c <F5>");
    tui.press("g v <F5>").expect(ISSUE_VIEW);
}

/// Covers: team::switch
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn switching_team_shows_that_teams_issues() {
    let (account, seeded) = a();
    let tui = Tui::start(&account, "");
    let other = seeded
        .other_team
        .as_deref()
        .expect("the seed gives workspace A a second team");
    let picker = tui.press("t");
    assert!(
        picker
            .overlay()
            .is_some_and(|o| o.starts_with("team picker"))
    );
    tui.type_text(other);
    let screen = tui.press("<Enter>");
    assert_eq!(screen.showing(), format!("{other} › Issues"));
    screen.expect_not(CRASH);
}
