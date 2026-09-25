//! Whole-frame render tests against ratatui's `TestBackend`.
//!
//! They check what reaches the screen and that the areas the renderer records
//! for hit-testing match what it drew — the contract `App::click` relies on.

use ratatui::{Terminal, backend::TestBackend};

use crate::config::Config;
use crate::core::entity::Issue;
use crate::interface::tui::app::{App, Chip, IssueSource, ListRow, Nav, Popup, Screen};
use crate::interface::tui::grouping::Preset;

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
    app.outbox.requests.clear();
    app.store.teams =
        vec![serde_json::from_str(r#"{"id":"t","name":"Core","key":"ENG"}"#).unwrap()];
    app.store.issues[IssueSource::Team].items = vec![
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
    terminal
        .draw(|f| super::draw(f, app, &mut super::Cache::default()))
        .unwrap();
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
        .frame
        .list_rows
        .iter()
        .filter(|r| matches!(r, ListRow::Issue { .. }))
        .count();
    let groups = app.frame.list_rows.len() - issues;
    assert_eq!((groups, issues), (2, 3));

    // Each recorded issue row is the screen line that shows that issue.
    let visible: Vec<String> = app
        .visible_issues()
        .iter()
        .map(|i| i.identifier.clone())
        .collect();
    for (offset, row) in app.frame.list_rows.iter().enumerate() {
        if let ListRow::Issue { ordinal, .. } = row {
            let line = &lines[app.frame.list_area.y as usize + offset];
            assert!(line.contains(&visible[*ordinal]), "{line:?}");
        }
    }
}

#[test]
fn a_click_on_a_drawn_chip_selects_its_preset() {
    let mut app = app();
    render(&mut app, 100, 20);
    let (area, chip) = app.frame.chip_areas[2];
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
    let area = app.frame.popup_area;
    assert!(area.width > 0 && area.height > 0);
    // The second entry is Urgent.
    app.click(area.x + 1, area.y + 1);
    assert_eq!(app.view.popup, Popup::None);
    assert_eq!(
        app.store.issues[IssueSource::Team].items[1].priority,
        crate::core::entity::Priority::Urgent
    );
}

#[test]
fn the_palette_draws_its_matches_where_it_records_them() {
    let mut app = app();
    app.open_palette();
    for c in "status".chars() {
        app.view.palette.query.insert(c);
    }
    let lines = render(&mut app, 100, 30);
    let text = screen_text(&lines);
    assert!(text.contains("Command palette"));
    let area = app.frame.popup_area;
    assert!(area.width > 0 && area.height > 0);
    // The first recorded row is the best match, with its key beside it.
    let first = &lines[area.y as usize];
    assert!(first.contains("Change status"), "{first}");
    assert!(first.contains("Issue actions  s "), "{first}");

    crate::interface::tui::palette::click(&mut app, area.x + 2, area.y);
    assert!(matches!(app.view.popup, Popup::StatusChange(_)));
}

#[test]
fn a_narrowed_picker_draws_its_query_and_a_click_picks_the_filtered_row() {
    let mut app = app();
    app.open_priority_change();
    app.popup_type('H');
    app.popup_type('i');
    let lines = render(&mut app, 100, 30);
    assert!(
        lines.iter().any(|l| l.contains("\u{203a} Hi")),
        "query line"
    );
    let area = app.frame.popup_area;
    assert!(lines[area.y as usize].contains("High"));
    app.click(area.x + 1, area.y);
    assert_eq!(
        app.store.issues[IssueSource::Team].items[1].priority,
        crate::core::entity::Priority::High
    );
}

#[test]
fn the_palette_finds_an_issue_by_a_wide_character_title() {
    let mut app = app();
    app.open_palette();
    for c in "課題".chars() {
        app.view.palette.query.insert(c);
    }
    let lines = render(&mut app, 80, 30);
    let area = app.frame.popup_area;
    let first = &lines[area.y as usize];
    assert!(first.contains("ENG-3"), "{first}");
    assert!(first.contains("Issue"), "{first}");
    // Cut to fit, wide characters counted as two cells: the section label
    // still sits against the box's right edge.
    assert!(first.contains("Issue  Todo  \u{2502}"), "{first}");
}

#[test]
fn a_palette_with_no_match_says_so() {
    let mut app = app();
    app.open_palette();
    for c in "課題zzz".chars() {
        app.view.palette.query.insert(c);
    }
    let lines = render(&mut app, 100, 30);
    assert!(screen_text(&lines).contains("No matches"));
    // A wide character takes two cells; the second is blank in the buffer.
    let query = lines.iter().find(|l| l.contains("zzz")).unwrap();
    assert!(query.contains("課 題 zzz"), "{query}");
}

#[test]
fn the_detail_view_renders_markdown_and_measures_itself() {
    let mut app = app();
    app.open_issue_detail();
    let text = screen_text(&render(&mut app, 100, 30));
    assert_eq!(app.nav.screen, Screen::IssueDetail);
    assert!(text.contains("Fix the build"), "{text}");
    assert!(
        text.contains("bold") && !text.contains("**bold**"),
        "{text}"
    );
    assert!(app.frame.detail_lines > 0);
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
    let setups: [fn(&mut App); 16] = [
        |_| {},
        |app| app.open_issue_detail(),
        |app| {
            app.open_issue_detail();
            app.start_comment();
        },
        |app| app.activate(Nav::MyIssues),
        |app| app.activate(Nav::Views),
        |app| app.go_to_team_section(crate::interface::tui::app::TeamSection::Projects),
        |app| app.go_to_team_section(crate::interface::tui::app::TeamSection::Cycles),
        |app| app.focus_sidebar(true),
        |app| app.open_help(),
        |app| app.open_filter(),
        |app| app.open_team_select(),
        |app| {
            app.workspaces = ["Acme", "Globex"]
                .iter()
                .enumerate()
                .map(|(i, name)| crate::interface::tui::app::WorkspaceEntry {
                    id: (*name).into(),
                    name: (*name).into(),
                    url_key: name.to_lowercase(),
                    current: i == 0,
                })
                .collect();
            app.open_workspace_select();
        },
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
    assert!(app.frame.list_rows.is_empty() && app.frame.chip_areas.is_empty());
}

/// The help overlay is drawn from the binding table, so every documented
/// binding shows up in it.
#[test]
fn every_documented_binding_appears_in_the_help_overlay() {
    let mut app = app();
    app.open_help();
    let lines = render(&mut app, 100, 120);
    // A herdr-only row is listed only inside herdr.
    for binding in crate::interface::tui::keys::BINDINGS
        .iter()
        .filter(|b| b.shown(app.herdr))
    {
        let Some(help) = binding.help else { continue };
        let row = format!("  {:<9}  {}", help.keys, help.text);
        assert!(
            lines.iter().any(|l| l.contains(&row)),
            "{row:?} missing from the help overlay"
        );
    }
}

/// The detail view keeps rendered Markdown between frames; whatever changes
/// in between must still reach the screen.
#[test]
fn the_detail_view_redraws_what_changed_since_the_last_frame() {
    let mut app = app();
    app.open_issue_detail();
    let text = screen_text(&render(&mut app, 100, 40));
    assert!(text.contains("Loading comments"), "{text}");

    let issue = app.store.current_issue.as_mut().unwrap();
    issue.comments =
        Some(serde_json::from_str(r#"{"nodes":[{"id":"c1","body":"first **draft**"}]}"#).unwrap());
    let text = screen_text(&render(&mut app, 100, 40));
    assert!(text.contains("first draft"), "{text}");

    let issue = app.store.current_issue.as_mut().unwrap();
    issue.comments.as_mut().unwrap().nodes[0].body = "second take".into();
    issue.description = Some("A new description".into());
    let text = screen_text(&render(&mut app, 100, 40));
    assert!(
        text.contains("second take") && !text.contains("first draft"),
        "{text}"
    );
    assert!(text.contains("A new description"), "{text}");

    // A narrower pane rewraps rather than reusing the wider lines.
    let narrow = render(&mut app, 60, 40);
    assert!(narrow.iter().all(|line| line.chars().count() == 60));
    assert!(screen_text(&narrow).contains("second take"));
    assert_eq!(
        narrow,
        render(&mut app, 60, 40),
        "a repeat frame is identical"
    );
}

/// Frame and key timings on a long list and a long comment thread.
///
/// Ignored by default, since a debug build's timings mean nothing. Run with
/// `cargo test --release timings -- --ignored --nocapture`.
mod timings {
    use std::time::{Duration, Instant};

    use super::*;

    const STATES: [(&str, &str); 5] = [
        ("Backlog", "backlog"),
        ("Todo", "unstarted"),
        ("In Progress", "started"),
        ("In Review", "started"),
        ("Done", "completed"),
    ];

    fn long_list(app: &mut App, n: usize) {
        app.store.issues[IssueSource::Team].items = (0..n)
            .map(|i| {
                let (name, kind) = STATES[i % STATES.len()];
                stated(&i.to_string(), &format!("Issue number {i}"), name, kind)
            })
            .collect();
        app.view.lists[IssueSource::Team].preset = Preset::All;
    }

    fn long_thread(n: usize) -> Issue {
        let comments: Vec<String> = (0..n)
            .map(|i| {
                // Every third comment replies to the one before it.
                let parent = if i % 3 == 2 {
                    format!(r#","parent":{{"id":"c{}"}}"#, i - 1)
                } else {
                    String::new()
                };
                let body = format!(
                    "Comment {i} with **bold**, `code` and a [link](https://example.com).\n\n\
                     - a first point long enough to wrap across the card at most widths\n\
                     - a second point\n\n> quoted text from an earlier message"
                );
                format!(
                    r#"{{"id":"c{i}","body":{body:?},"createdAt":"2026-01-01T00:00:00.000Z",
                        "user":{{"id":"u1","name":"Ada Lovelace"}}{parent}}}"#
                )
            })
            .collect();
        let description = "## Summary\n\nA description with **bold** and `code`, long \
                           enough to wrap a few times across the column.\n\n"
            .repeat(20);
        serde_json::from_str(&format!(
            r#"{{"id":"big","identifier":"ENG-9999","title":"A long thread","priority":2,
                "state":{{"id":"s","name":"Todo","type":"unstarted","position":1}},
                "assignee":{{"id":"u1","name":"Ada Lovelace"}},
                "description":{description:?},
                "comments":{{"nodes":[{}]}},"project":null,"cycle":null}}"#,
            comments.join(",")
        ))
        .unwrap()
    }

    fn time(label: &str, runs: u32, mut f: impl FnMut()) -> Duration {
        f(); // warm up
        let start = Instant::now();
        for _ in 0..runs {
            f();
        }
        let each = start.elapsed() / runs;
        println!("{label:<40} {each:>10.1?}");
        each
    }

    #[test]
    #[ignore = "timing run: cargo test --release timings -- --ignored --nocapture"]
    fn timings() {
        let mut terminal = Terminal::new(TestBackend::new(160, 50)).unwrap();
        let mut cache = crate::interface::tui::ui::Cache::default();

        let mut list = app();
        long_list(&mut list, 1000);
        time("list frame, 1000 issues", 500, || {
            terminal
                .draw(|f| crate::interface::tui::ui::draw(f, &mut list, &mut cache))
                .unwrap();
        });
        time("j then k, 1000 issues", 2000, || {
            list.move_selection(1);
            list.move_selection(-1);
        });
        list.open_issue_detail();
        time("detail frame over 1000 issues", 500, || {
            terminal
                .draw(|f| crate::interface::tui::ui::draw(f, &mut list, &mut cache))
                .unwrap();
        });

        let mut thread = app();
        thread.open_issue_detail();
        thread.store.current_issue = Some(long_thread(0));
        time("detail frame, no comments", 500, || {
            terminal
                .draw(|f| crate::interface::tui::ui::draw(f, &mut thread, &mut cache))
                .unwrap();
        });
        thread.store.current_issue = Some(long_thread(100));
        time("detail frame, 100 comments", 500, || {
            terminal
                .draw(|f| crate::interface::tui::ui::draw(f, &mut thread, &mut cache))
                .unwrap();
        });
    }
}

#[test]
fn an_issue_a_herdr_agent_works_on_carries_its_state() {
    let mut app = app();
    app.set_agents(
        serde_json::from_str(
            r#"[{"pane":"w1:p1","workspace_label":"docs","agent":"claude","status":"blocked","issue":"ENG-1"}]"#,
        )
        .unwrap(),
    );
    let lines = render(&mut app, 120, 20);
    let row = |id: &str| lines.iter().find(|l| l.contains(id)).unwrap().clone();
    assert!(row("ENG-1").contains('\u{25b2}'), "{}", row("ENG-1"));
    assert!(!row("ENG-2").contains('\u{25b2}'), "{}", row("ENG-2"));

    // The issue page names the agent and how to reach it.
    let issue = app.store.issues[IssueSource::Team].items[0].clone();
    app.open_issue_from_list(&issue);
    let text = screen_text(&render(&mut app, 140, 30));
    assert!(text.contains("claude waiting for you"), "{text}");
    assert!(text.contains("docs · g w to go"), "{text}");
}
