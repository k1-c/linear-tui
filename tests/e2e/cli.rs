//! The headless commands as an agent runs them, with no TUI open: every
//! list a person reads in the TUI, and projects and their milestones made,
//! changed, and deleted. What Linear ends up holding is read back through
//! the same commands.

use serde_json::Value;

use super::a;
use super::linear::titles::*;
use super::tui::{Tui, eventually};

fn json(text: String) -> Value {
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

/// What a command prints, or the scenario stops with its error.
fn run(tui: &Tui, args: &[&str]) -> String {
    tui.cli(args)
        .unwrap_or_else(|e| panic!("linear-tui {}: {e}", args.join(" ")))
}

/// The identifiers a list printed as JSON holds.
fn identifiers(list: &Value) -> Vec<String> {
    list["issues"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| i["identifier"].as_str().map(String::from))
        .collect()
}

/// Every list the TUI shows reads from the command line too: a team's
/// slices, mine, a project's, a cycle's, a saved view's, a search, and the
/// teams, cycles, views, and favorites themselves.
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn every_list_reads_from_the_command_line() {
    let (account, seeded) = a();
    let tui = Tui::start(&account, "");
    let team = seeded.team_key.as_str();
    let crash = seeded.issue(CRASH);
    let docs = seeded.issue(DOCS);

    let active = json(run(&tui, &["issue", "list", "--team", team, "--json"]));
    assert!(identifiers(&active).contains(&crash.to_string()));
    assert!(
        !identifiers(&active).contains(&docs.to_string()),
        "Active hides done work"
    );
    let all = run(&tui, &["issue", "list", "--team", team, "--preset", "all"]);
    assert!(all.contains(docs), "{all}");
    let urgent = json(run(
        &tui,
        &[
            "issue",
            "list",
            "--team",
            team,
            "--priority",
            "urgent",
            "--json",
        ],
    ));
    assert_eq!(identifiers(&urgent), [crash]);

    let mine = json(run(&tui, &["issue", "list", "--mine", "--json"]));
    assert!(identifiers(&mine).contains(&crash.to_string()));
    let project = run(&tui, &["issue", "list", "--project", PROJECT]);
    assert!(
        project.contains(crash) && project.contains(docs),
        "{project}"
    );
    // The seeded cycle may not have started yet, so it is named, not
    // `current`.
    let in_cycle = tui.issue(crash)["cycle"]["name"]
        .as_str()
        .unwrap()
        .to_string();
    let cycle = run(
        &tui,
        &["issue", "list", "--team", team, "--cycle", &in_cycle],
    );
    assert!(cycle.contains(crash), "{cycle}");
    let view = run(&tui, &["issue", "list", "--view", ISSUE_VIEW]);
    assert!(view.contains(seeded.issue(WIND)), "{view}");

    // Linear indexes a new issue for search a while after it is filed.
    eventually("issue search finds the seeded issue", || {
        let found = json(run(&tui, &["issue", "search", CRASH, "--json"]));
        identifiers(&found)
            .contains(&crash.to_string())
            .then_some(())
    });

    let teams = run(&tui, &["team", "list"]);
    assert!(teams.contains(team), "{teams}");
    let shown = run(&tui, &["team", "show", team]);
    for part in ["## States", "In Progress", "## Members", "## Labels", "Bug"] {
        assert!(shown.contains(part), "{part} in {shown}");
    }
    let cycles = run(&tui, &["cycle", "list", "--team", team]);
    assert!(cycles.contains(&format!("- {in_cycle} — ")), "{cycles}");
    let views = run(&tui, &["view", "list"]);
    assert!(
        views.contains(ISSUE_VIEW) && views.contains(PROJECT_VIEW),
        "{views}"
    );
    let projects = run(&tui, &["project", "list", "--team", team]);
    assert!(projects.contains(PROJECT), "{projects}");
    let favorites = run(&tui, &["favorite", "list"]);
    assert!(
        favorites.contains(&format!("linear-tui project show {PROJECT:?}")),
        "{favorites}"
    );
}

/// Covers: project::find, project::open, project::load_statuses,
/// project::create, project::update, project::delete, project::add_milestone,
/// project::update_milestone, project::delete_milestone
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn a_project_and_its_milestones_are_made_changed_and_deleted() {
    let (account, seeded) = a();
    let tui = Tui::start(&account, "");
    let team = seeded.team_key.as_str();
    let name = format!("E2E project {}", std::process::id());

    let made = run(
        &tui,
        &[
            "project",
            "create",
            "--team",
            team,
            "--name",
            &name,
            "--status",
            "Planned",
            "--target",
            "2027-03-31",
            "--lead",
            "me",
            "--content",
            "## Why\n\nThe first body.",
        ],
    );
    assert!(
        made.starts_with(&format!("Created project {name}")),
        "{made}"
    );
    let shown = json(run(&tui, &["project", "show", &name, "--json"]));
    assert_eq!(shown["status"]["name"], "Planned");
    assert_eq!(shown["target_date"], "2027-03-31");
    assert_eq!(shown["lead"], seeded.viewer_name.as_str());
    let content = shown["content"].as_str().unwrap_or_default();
    assert!(content.contains("The first body."), "{shown}");

    let changed = run(
        &tui,
        &[
            "project",
            "update",
            &name,
            "--status",
            "In Progress",
            "--target",
            "none",
            "--content",
            "The second body.",
        ],
    );
    assert!(
        changed.contains(&format!("{name}: status Planned → In Progress")),
        "{changed}"
    );
    assert!(
        changed.contains(&format!("{name}: content rewritten")),
        "{changed}"
    );
    let shown = run(&tui, &["project", "show", &name]);
    assert!(
        shown.contains("## Content\n\nThe second body.\n") && !shown.contains("first body"),
        "{shown}"
    );
    let backwards = tui
        .cli(&[
            "project",
            "update",
            &name,
            "--start",
            "2027-05-01",
            "--target",
            "2027-04-01",
        ])
        .unwrap_err();
    assert!(backwards.contains("before the start date"), "{backwards}");

    let beta = run(
        &tui,
        &[
            "milestone",
            "create",
            &name,
            "--name",
            "Beta",
            "--target",
            "2027-02-01",
        ],
    );
    assert!(
        beta.starts_with("Created milestone Beta — target 2027-02-01"),
        "{beta}"
    );
    run(
        &tui,
        &["milestone", "update", &name, "Beta", "--name", "Beta 1"],
    );
    let milestones = run(&tui, &["milestone", "list", &name]);
    assert!(
        milestones.contains("- Beta 1 — target 2027-02-01"),
        "{milestones}"
    );

    // An issue filed under the milestone, and moved out of it.
    let filed = json(run(
        &tui,
        &[
            "issue",
            "create",
            "--team",
            team,
            "--title",
            "E2E milestone issue",
            "--project",
            &name,
            "--milestone",
            "Beta 1",
            "--json",
        ],
    ));
    let id = filed["identifier"]
        .as_str()
        .expect("the new issue")
        .to_string();
    assert_eq!(tui.issue(&id)["milestone"]["name"], "Beta 1");
    run(&tui, &["issue", "update", &id, "--milestone", "none"]);
    assert!(tui.issue(&id)["milestone"].is_null());

    run(&tui, &["milestone", "delete", &name, "Beta 1"]);
    let milestones = run(&tui, &["milestone", "list", &name]);
    assert!(milestones.contains("(none)"), "{milestones}");
    let deleted = run(&tui, &["project", "delete", &name]);
    assert!(deleted.contains("in Linear's trash"), "{deleted}");
    let gone = tui.cli(&["project", "show", &name]).unwrap_err();
    assert!(gone.contains("no project"), "{gone}");
}
