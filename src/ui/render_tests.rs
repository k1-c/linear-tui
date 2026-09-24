//! Whole-frame render tests against ratatui's `TestBackend`.
//!
//! They check what reaches the screen and that the areas the renderer records
//! for hit-testing match what it drew — the contract `App::click` relies on.

use ratatui::{Terminal, backend::TestBackend};

use crate::api::types::Issue;
use crate::app::{App, Chip, IssueSource, ListRow, Nav, Popup, Screen};
use crate::config::Config;
use crate::grouping::Preset;

fn stated(id: &str, title: &str, state: &str, kind: &str) -> Issue {
    serde_json::from_str(&format!(
        r#"{{"id":"{id}","identifier":"ENG-{id}","title":"{title}","priority":0,
            "state":{{"id":"s-{kind}","name":"{state}","type":"{kind}","position":1}},
            "assignee":{{"id":"u1","name":"Ada Lovelace"}},
            "description":"Some **bold** text","comments":null,"project":null,"cycle":null}}"#
    ))
    .unwrap()
}

fn app() -> App {
    let mut app = App::new(&Config::default());
    app.requests.clear();
    app.teams = vec![serde_json::from_str(r#"{"id":"t","name":"Core","key":"ENG"}"#).unwrap()];
    app.lists[IssueSource::Team].issues = vec![
        stated("1", "Write the docs", "Todo", "unstarted"),
        stated("2", "Fix the build", "In Progress", "started"),
        stated(
            "3",
            "日本語のタイトルがとても長い課題の例です",
            "Todo",
            "unstarted",
        ),
    ];
    app
}

fn render(app: &mut App, width: u16, height: u16) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| super::draw(f, app)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

fn screen_text(lines: &[String]) -> String {
    lines.join("\n")
}

#[test]
fn the_issue_list_shows_groups_rows_and_a_count() {
    let mut app = app();
    let text = screen_text(&render(&mut app, 100, 20));
    for expected in [
        "In Progress",
        "Todo",
        "ENG-1",
        "ENG-2",
        "Fix the build",
        "3 issues",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in\n{text}");
    }
}

#[test]
fn recorded_rows_match_the_drawn_list() {
    let mut app = app();
    let lines = render(&mut app, 100, 20);
    let issues = app
        .list_rows
        .iter()
        .filter(|r| matches!(r, ListRow::Issue { .. }))
        .count();
    let groups = app.list_rows.len() - issues;
    assert_eq!((groups, issues), (2, 3));

    // Each recorded issue row is the screen line that shows that issue.
    let visible: Vec<String> = app
        .visible_issues()
        .iter()
        .map(|i| i.identifier.clone())
        .collect();
    for (offset, row) in app.list_rows.iter().enumerate() {
        if let ListRow::Issue { ordinal, .. } = row {
            let line = &lines[app.list_area.y as usize + offset];
            assert!(line.contains(&visible[*ordinal]), "{line:?}");
        }
    }
}

#[test]
fn a_click_on_a_drawn_chip_selects_its_preset() {
    let mut app = app();
    render(&mut app, 100, 20);
    let (area, chip) = app.chip_areas[2];
    assert_eq!(chip, Chip::Preset(Preset::all()[2]));
    app.click(area.x, area.y);
    assert_eq!(app.preset(), Preset::all()[2]);
}

#[test]
fn a_wide_title_is_cut_to_the_row_and_the_columns_stay_aligned() {
    let mut app = app();
    let lines = render(&mut app, 60, 20);
    let row_of = |id: &str| lines.iter().find(|l| l.contains(id)).unwrap().clone();
    let (short, wide) = (row_of("ENG-1"), row_of("ENG-3"));
    // Both rows end with the same avatar column, whatever the title's width.
    assert_eq!(
        short.trim_end().chars().last(),
        wide.trim_end().chars().last(),
        "\n{short}\n{wide}"
    );
}

#[test]
fn a_change_popup_draws_where_it_records_and_a_click_applies_it() {
    let mut app = app();
    app.open_priority_change();
    render(&mut app, 100, 30);
    let area = app.popup_area;
    assert!(area.width > 0 && area.height > 0);
    // The second entry is Urgent.
    app.click(area.x + 1, area.y + 1);
    assert_eq!(app.popup, Popup::None);
    assert_eq!(
        app.lists[IssueSource::Team].issues[1].priority,
        crate::api::types::Priority::Urgent
    );
}

#[test]
fn the_detail_view_renders_markdown_and_measures_itself() {
    let mut app = app();
    app.open_issue_detail();
    let text = screen_text(&render(&mut app, 100, 30));
    assert_eq!(app.screen, Screen::IssueDetail);
    assert!(text.contains("Fix the build"), "{text}");
    assert!(
        text.contains("bold") && !text.contains("**bold**"),
        "{text}"
    );
    assert!(app.detail_lines > 0);
}

/// No screen, overlay, or terminal size may panic the renderer.
#[test]
fn every_screen_renders_at_every_size() {
    let mut sizes = vec![(0, 0), (1, 1), (3, 2), (19, 30), (200, 4)];
    for width in [20, 21, 25, 33, 40, 60, 80, 99, 100, 101, 140] {
        for height in [5, 6, 7, 9, 12, 24] {
            sizes.push((width, height));
        }
    }
    let setups: [fn(&mut App); 15] = [
        |_| {},
        |app| app.open_issue_detail(),
        |app| {
            app.open_issue_detail();
            app.start_comment();
        },
        |app| app.activate(Nav::MyIssues),
        |app| app.activate(Nav::Views),
        |app| app.go_to_team_section(crate::app::TeamSection::Projects),
        |app| app.go_to_team_section(crate::app::TeamSection::Cycles),
        |app| app.focus_sidebar(true),
        |app| app.open_help(),
        |app| app.open_filter(),
        |app| app.open_team_select(),
        |app| app.open_status_change(),
        |app| app.open_assignee_change(),
        |app| app.start_new_issue(),
        |app| app.set_error("Something went wrong\nwith a second line"),
    ];
    for (i, setup) in setups.iter().enumerate() {
        for &(width, height) in &sizes {
            let mut app = app();
            setup(&mut app);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                render(&mut app, width, height);
            }));
            assert!(result.is_ok(), "setup {i} panicked at {width}x{height}");
        }
    }
}

#[test]
fn a_tiny_terminal_says_so_and_leaves_nothing_clickable() {
    let mut app = app();
    render(&mut app, 100, 20);
    let text = screen_text(&render(&mut app, 12, 3));
    assert!(text.contains("Terminal"), "{text}");
    assert!(app.list_rows.is_empty() && app.chip_areas.is_empty());
}

/// The help overlay is drawn from the binding table, so every documented
/// binding shows up in it.
#[test]
fn every_documented_binding_appears_in_the_help_overlay() {
    let mut app = app();
    app.open_help();
    let lines = render(&mut app, 100, 120);
    for binding in crate::keys::BINDINGS {
        let Some(help) = binding.help else { continue };
        let row = format!("  {:<9}  {}", help.keys, help.text);
        assert!(
            lines.iter().any(|l| l.contains(&row)),
            "{row:?} missing from the help overlay"
        );
    }
}
