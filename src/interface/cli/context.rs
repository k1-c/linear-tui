//! `linear-tui context`: what linear-tui is showing in this repository, for
//! an agent to resolve "this issue" or "the top three in this list".

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Result, bail};
use serde_json::json;

use super::args::Args;
use super::issue::is_identifier;
use crate::core::entity::snapshot::{self as snap, Destination, ViewSnapshot};
use crate::infra::disk::snapshot::Shelf;
use crate::infra::linear::auth::token::{Account, TokenStore};

pub fn run(args: &[String]) -> Result<()> {
    let args = Args::parse(args, &["json"], &["workspace"])?;
    args.positionals::<0>("linear-tui context [--json] [--workspace <path>]")?;
    let cwd = match args.value("workspace") {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir()?,
    };
    let workspace = crate::infra::disk::snapshot::workspace_of(&cwd);
    let instances = Shelf::new(&crate::infra::disk::snapshot::state_dir()?, &workspace).instances();
    let found = crate::core::usecase::instance::to_report(&instances)
        .map(|(instance, open)| (instance.view.clone(), open));
    let worktree_issue = worktree_issue(&cwd);
    if found.is_none() && worktree_issue.is_none() {
        bail!(
            "No linear-tui view recorded for {} — open linear-tui there first.",
            workspace.display()
        );
    }
    let now = crate::infra::disk::snapshot::unix_now();
    let out = if args.flag("json") {
        let value = json!({
            "workspace": workspace,
            "running": found.as_ref().map(|(_, running)| *running),
            "worktree_issue": worktree_issue,
            "snapshot": found.as_ref().map(|(s, _)| s),
        });
        serde_json::to_string_pretty(&value)?
    } else {
        let mut out = render(found.as_ref(), worktree_issue.as_deref(), now);
        if let Some(warning) = found
            .as_ref()
            .and_then(|(s, _)| other_workspace(s, current_account().as_ref()))
        {
            out.push_str(&format!("\n{warning}\n"));
        }
        out
    };
    println!("{}", out.trim_end());
    Ok(())
}

/// The account `linear-tui issue …` acts in. Read without the network, and
/// `None` when it cannot be told — an API key, or nothing stored.
fn current_account() -> Option<Account> {
    TokenStore::new()
        .ok()?
        .load()
        .ok()?
        .current_account()
        .cloned()
}

/// A warning when the view on screen is of another Linear workspace than the
/// one the issue commands act in: its IDs would not be found there.
fn other_workspace(s: &ViewSnapshot, current: Option<&Account>) -> Option<String> {
    let shown = s.organization.as_ref()?;
    let acting = current?.organization.as_ref()?;
    (shown.id != acting.id).then(|| {
        format!(
            "**This view is of {} ({}), but `linear-tui issue …` acts in {} ({}).** \
             `linear-tui auth switch {}` first to act on what is shown.",
            shown.name, shown.url_key, acting.name, acting.url_key, shown.url_key
        )
    })
}

/// The issue a linked worktree is for, read from its branch name — Linear
/// suggests branches like `me/eng-42-checkout-fails`. The main checkout is
/// not tied to one issue, whatever its branch is called.
pub fn worktree_issue(cwd: &Path) -> Option<String> {
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let dirs = git(&[
        "rev-parse",
        "--path-format=absolute",
        "--git-dir",
        "--git-common-dir",
    ])?;
    let mut dirs = dirs.lines();
    if dirs.next()? == dirs.next()? {
        return None;
    }
    identifier_in_branch(&git(&["branch", "--show-current"])?)
}

/// `ENG-42` from `me/eng-42-checkout-fails`, `eng-42`, or `ENG-42/fix`.
pub fn identifier_in_branch(branch: &str) -> Option<String> {
    branch.split('/').find_map(|segment| {
        let mut parts = segment.split('-');
        let team = parts.next()?;
        let number = parts.next()?;
        let candidate = format!("{team}-{number}");
        is_identifier(&candidate).then(|| candidate.to_ascii_uppercase())
    })
}

fn destination_label(dest: &Destination) -> String {
    match dest {
        Destination::MyIssues => "My Issues".into(),
        Destination::Views => "Views".into(),
        Destination::View { name, .. } => format!("View “{name}”"),
        Destination::Team { team, section } => {
            let section = match section {
                snap::Section::Issues => "Issues",
                snap::Section::Cycles => "Cycles",
                snap::Section::Projects => "Projects",
                snap::Section::Views => "Views",
            };
            format!("{} › {section}", team.name)
        }
        Destination::Favorite { title, .. } => format!("Favorite “{title}”"),
    }
}

fn row_line(index: usize, row: &snap::Row, selected: bool) -> String {
    let mut line = format!("{}. ", index + 1);
    if let Some(identifier) = &row.identifier {
        line.push_str(identifier);
        line.push(' ');
    }
    line.push_str(&row.title);
    let details: Vec<&str> = [&row.state, &row.assignee, &row.priority]
        .into_iter()
        .filter_map(|d| d.as_deref())
        .collect();
    if !details.is_empty() {
        line.push_str(&format!(" — {}", details.join(" · ")));
    }
    if selected {
        line.push_str("  ← cursor");
    }
    line
}

pub fn render(
    found: Option<&(ViewSnapshot, bool)>,
    worktree_issue: Option<&str>,
    now: u64,
) -> String {
    let mut out = String::from("# linear-tui context\n\n");
    if let Some(issue) = worktree_issue {
        out.push_str(&format!(
            "This worktree is for **{issue}** — `linear-tui issue show {issue}` for the issue itself.\n\n"
        ));
    }
    let Some((s, running)) = found else {
        out.push_str("No linear-tui view is recorded for this repository.\n");
        return out;
    };

    let mut path = vec![destination_label(&s.destination)];
    // A favorite project or cycle is the page it opens.
    let favorite = match &s.destination {
        Destination::Favorite { title, .. } => Some(title.as_str()),
        _ => None,
    };
    if let Some(project) = s
        .project
        .as_ref()
        .filter(|p| Some(p.name.as_str()) != favorite)
    {
        path.push(format!("project “{}”", project.name));
    }
    if let Some(cycle) = s
        .cycle
        .as_ref()
        .filter(|c| Some(c.name.as_str()) != favorite)
    {
        path.push(cycle.name.clone());
    }
    if let Some(issue) = &s.issue {
        path.push(issue.identifier.clone());
    }
    out.push_str(&format!("- Showing: {}\n", path.join(" › ")));
    if let Some(org) = &s.organization {
        out.push_str(&format!(
            "- Linear workspace: {} ({})\n",
            org.name, org.url_key
        ));
    }
    if let Some(term) = &s.search {
        out.push_str(&format!("- Search results for: {term}\n"));
    }
    let ago = crate::infra::disk::snapshot::ago(&s.updated_at, now)
        .unwrap_or_else(|| s.updated_at.clone());
    let state = if *running {
        format!("open (pid {})", s.pid)
    } else {
        "**closed — this is the last view before it quit, and may be stale**".into()
    };
    out.push_str(&format!(
        "- Updated: {ago} ({}); linear-tui is {state}\n",
        s.updated_at
    ));

    if let Some(issue) = &s.issue {
        out.push_str(&format!(
            "\n## Open issue\n\n{} {} — `linear-tui issue show {}` for its description and comments.\n",
            issue.identifier, issue.title, issue.identifier
        ));
    }

    if !s.rows.is_empty() {
        let heading = if s.issue.is_some() {
            "The list it was opened from"
        } else {
            "List"
        };
        let shown = if s.rows_total > s.rows.len() {
            format!("first {} of {}", s.rows.len(), s.rows_total)
        } else {
            format!("{}", s.rows_total)
        };
        out.push_str(&format!("\n## {heading} ({shown})\n\n"));
        if let Some(list) = list_settings(s) {
            out.push_str(&format!("{list}\n\n"));
        }
        for (i, row) in s.rows.iter().enumerate() {
            out.push_str(&row_line(i, row, s.selected_row == Some(i)));
            out.push('\n');
        }
    }

    out.push_str(
        "\nAct on these with `linear-tui issue show|comment|status <ID>` and \
         `linear-tui issue create --team <key> --title <text>`.\n",
    );
    out
}

/// How the list on screen is sliced, if it is an issue list.
fn list_settings(s: &ViewSnapshot) -> Option<String> {
    let source = match (&s.destination, s.screen, &s.project, &s.cycle) {
        (_, snap::Screen::ProjectDetail, _, _) => snap::Source::Project,
        (_, snap::Screen::CycleDetail, _, _) => snap::Source::Cycle,
        (_, snap::Screen::IssueDetail, Some(_), _) => snap::Source::Project,
        (_, snap::Screen::IssueDetail, _, Some(_)) => snap::Source::Cycle,
        (Destination::MyIssues, ..) => snap::Source::My,
        (Destination::View { .. }, ..) => snap::Source::View,
        (
            Destination::Team {
                section: snap::Section::Issues,
                ..
            },
            ..,
        ) => snap::Source::Team,
        _ => return None,
    };
    let settings = s.lists.iter().find(|l| l.source == source);
    let preset = match settings.map(|l| l.preset) {
        Some(snap::Preset::Backlog) => "Backlog",
        Some(snap::Preset::All) => "All issues",
        Some(snap::Preset::Active) => "Active",
        // A team list opens on Active, every other list on All.
        None if source == snap::Source::Team => "Active",
        None => "All issues",
    };
    let mut parts = vec![format!("Preset: {preset}")];
    if let Some(status) = settings.and_then(|l| l.status.as_deref()) {
        parts.push(format!("Status: {status}"));
    }
    if let Some(priority) = settings.and_then(|l| l.priority.as_deref()) {
        parts.push(format!("Priority: {priority}"));
    }
    Some(parts.join(" · "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> ViewSnapshot {
        let text = std::fs::read_to_string("tests/fixtures/view_snapshot.json").unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn a_view_of_another_workspace_than_the_issue_commands_is_flagged() {
        let snapshot = fixture();
        let account = |id: &str| Account {
            organization: Some(crate::core::entity::Organization {
                id: id.into(),
                name: "Other".into(),
                url_key: "other".into(),
            }),
            user: None,
            tokens: crate::infra::linear::auth::token::OAuthTokens {
                access_token: String::new(),
                refresh_token: String::new(),
                expires_at: 0,
            },
        };
        let warning = other_workspace(&snapshot, Some(&account("elsewhere"))).unwrap();
        assert!(
            warning.contains("`linear-tui auth switch shop`"),
            "{warning}"
        );
        assert!(other_workspace(&snapshot, Some(&account("b41d…org"))).is_none());
        assert!(other_workspace(&snapshot, None).is_none());
    }

    #[test]
    fn a_branch_names_its_issue() {
        assert_eq!(
            identifier_in_branch("me/eng-42-checkout-fails").as_deref(),
            Some("ENG-42")
        );
        assert_eq!(
            identifier_in_branch("ENG-42/fix").as_deref(),
            Some("ENG-42")
        );
        assert_eq!(identifier_in_branch("feat/view-snapshot"), None);
        assert_eq!(identifier_in_branch("main"), None);
    }

    #[test]
    fn the_markdown_says_what_is_open_and_marks_the_cursor() {
        let snapshot = fixture();
        // Two minutes after the snapshot was written.
        let now =
            crate::infra::disk::snapshot::parse_timestamp(&snapshot.updated_at).unwrap() + 120;
        let text = render(Some(&(snapshot, true)), Some("ENG-42"), now);
        assert!(text.contains("This worktree is for **ENG-42**"), "{text}");
        assert!(
            text.contains("- Showing: Engineering › Issues › ENG-42\n"),
            "{text}"
        );
        assert!(text.contains("2 minutes ago"), "{text}");
        assert!(text.contains("- Linear workspace: Shop (shop)\n"), "{text}");
        assert!(text.contains("linear-tui is open (pid 48213)"), "{text}");
        assert!(
            text.contains("## The list it was opened from (2)"),
            "{text}"
        );
        assert!(text.contains("Preset: Active · Priority: High"), "{text}");
        assert!(
            text.contains("1. ENG-40 Retry payment webhooks — In Progress · me · High\n"),
            "{text}"
        );
        assert!(
            text.contains("2. ENG-42 Checkout fails on empty cart — Todo · High  ← cursor\n"),
            "{text}"
        );
    }

    #[test]
    fn a_closed_instance_is_flagged_as_stale() {
        let snapshot = fixture();
        let now =
            crate::infra::disk::snapshot::parse_timestamp(&snapshot.updated_at).unwrap() + 7200;
        let text = render(Some(&(snapshot, false)), None, now);
        assert!(text.contains("may be stale"), "{text}");
        assert!(text.contains("2 hours ago"), "{text}");
        assert!(!text.contains("This worktree"), "{text}");
    }

    #[test]
    fn a_worktree_issue_alone_is_still_context() {
        let text = render(None, Some("ENG-7"), 0);
        assert!(text.contains("**ENG-7**"), "{text}");
        assert!(text.contains("No linear-tui view is recorded"), "{text}");
    }
}
