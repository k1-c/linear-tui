//! Working with agents: notes, `linear-tui context`, and the headless
//! issue commands against the same workspace the TUI shows.

use super::a;
use super::linear::titles::*;
use super::tui::Tui;

/// Covers: notes::add, notes::prompt, notes::send, notes::discard
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn notes_are_sent_as_one_prompt_or_discarded() {
    let (account, seeded) = a();
    let tui = Tui::start(&account, "");
    tui.open(seeded.issue(CRASH));
    tui.press("n");
    tui.type_text("reproduce with an empty forecast first");
    let screen = tui.press("<C-Enter>");
    assert_eq!(screen.0["notes"], 1);
    let held = tui.press("<C-s>").held();
    let prompt = held
        .iter()
        .find(|h| h.starts_with("would copy"))
        .unwrap_or_else(|| panic!("the prompt goes to the clipboard: {held:?}"));
    assert!(
        prompt.contains(&format!("**{}**", seeded.issue(CRASH))),
        "{prompt}"
    );
    assert!(
        prompt.contains("reproduce with an empty forecast first"),
        "{prompt}"
    );

    tui.press("N");
    tui.type_text("the view note");
    tui.press("<C-Enter>");
    let screen = tui.run("Discard notes");
    assert_eq!(screen.status(), Some("Discarded 1 note"));
}

/// Covers: instance::to_report
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn an_agent_reads_the_view_on_screen() {
    let (account, seeded) = a();
    let tui = Tui::start(&account, "");
    let id = seeded.issue(CRASH);
    tui.open(id);
    let context = tui.cli(&["context"]).unwrap();
    assert!(
        context.contains(&format!("- Showing: {} › Issues › {id}", seeded.team_name)),
        "{context}"
    );
    assert!(context.contains("linear-tui is open"), "{context}");
    assert!(context.contains(&format!("{id} {CRASH}")), "{context}");
}

/// Covers: issue::comment, issue::set_status, issue::create, team::load
#[test]
#[ignore = "touches real Linear workspaces; see tests/e2e/main.rs"]
fn the_headless_issue_commands_act_on_the_same_workspace() {
    let (account, seeded) = a();
    let tui = Tui::start(&account, "");
    let id = seeded.issue(FOR_AGENT);

    let shown = tui.cli(&["issue", "show", id]).unwrap();
    assert!(shown.starts_with(&format!("# {id} {FOR_AGENT}")), "{shown}");

    let body = format!("From an agent, run {}", std::process::id());
    assert_eq!(
        tui.cli(&["issue", "comment", id, &body]).unwrap().trim(),
        format!("Commented on {id}")
    );
    let moved = tui.cli(&["issue", "status", id, "todo"]).unwrap();
    assert_eq!(moved.trim(), format!("{id}: Backlog → Todo"));

    // The TUI shows what the agent did.
    tui.open(id).expect(&body).expect("Todo");
    let error = tui.cli(&["issue", "status", id, "Nowhere"]).unwrap_err();
    assert!(
        error.contains("In Progress"),
        "the team's states are listed: {error}"
    );

    let title = format!("Filed by an agent {}", std::process::id());
    let created = tui
        .cli(&[
            "issue",
            "create",
            "--team",
            &seeded.team_key,
            "--title",
            &title,
        ])
        .unwrap();
    assert!(created.starts_with("Created "), "{created}");
    tui.press("g e <F5>").expect(&title);
}
