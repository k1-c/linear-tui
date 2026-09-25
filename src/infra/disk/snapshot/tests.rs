use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use super::*;
use crate::core::entity::snapshot::*;

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("linear-tui-snapshot-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn snapshot(pid: u32, updated_at: &str, closed_at: Option<&str>) -> ViewSnapshot {
    ViewSnapshot {
        version: VERSION,
        workspace: PathBuf::from("/repo"),
        cwd: PathBuf::from("/repo"),
        pid,
        updated_at: updated_at.into(),
        closed_at: closed_at.map(Into::into),
        herdr_pane: None,
        organization: None,
        team: None,
        destination: Destination::MyIssues,
        screen: Screen::IssueList,
        project: None,
        cycle: None,
        issue: None,
        search: None,
        group_by: GroupBy::Status,
        lists: Vec::new(),
        rows: Vec::new(),
        rows_total: 0,
        selected_row: None,
    }
}

fn put(shelf: &Shelf, snapshot: &ViewSnapshot) {
    fs::create_dir_all(shelf.path_for(0).parent().unwrap()).unwrap();
    fs::write(
        shelf.path_for(snapshot.pid),
        serde_json::to_vec(snapshot).unwrap(),
    )
    .unwrap();
}

#[test]
fn timestamps_are_rfc3339_utc_to_the_second() {
    assert_eq!(format_timestamp(0), "1970-01-01T00:00:00Z");
    assert_eq!(format_timestamp(951_782_400), "2000-02-29T00:00:00Z");
    assert_eq!(format_timestamp(1_790_316_062), "2026-09-25T06:01:02Z");
    assert_eq!(format_timestamp(4_102_444_799), "2099-12-31T23:59:59Z");
}

#[test]
fn the_documented_example_parses() {
    let text = fs::read_to_string("tests/fixtures/view_snapshot.json").unwrap();
    let snapshot: ViewSnapshot = serde_json::from_str(&text).unwrap();
    assert_eq!(snapshot.screen, Screen::IssueDetail);
    assert_eq!(
        snapshot.selected().and_then(|r| r.identifier.as_deref()),
        Some("ENG-42")
    );
    // It survives a round trip unchanged.
    let again: ViewSnapshot =
        serde_json::from_str(&serde_json::to_string(&snapshot).unwrap()).unwrap();
    assert_eq!(again, snapshot);
}

#[test]
fn a_minimal_snapshot_parses_with_every_optional_field_absent() {
    let snapshot: ViewSnapshot = serde_json::from_str(
        r#"{"version":1,"workspace":"/r","cwd":"/r","pid":1,"updated_at":"2026-01-01T00:00:00Z",
            "destination":{"kind":"my_issues"},"screen":"issue_list","group_by":"none"}"#,
    )
    .unwrap();
    assert!(snapshot.rows.is_empty() && snapshot.lists.is_empty());
    assert!(snapshot.selected().is_none());
}

#[test]
fn each_record_is_read_as_an_instance() {
    let shelf = Shelf::new(&scratch("instances"), Path::new("/repo"));
    put(&shelf, &snapshot(1, "2026-09-25T08:00:00Z", None));
    put(
        &shelf,
        &snapshot(2, "2026-09-25T09:00:00Z", Some("2026-09-25T09:30:00Z")),
    );
    let mut pids: Vec<(u32, bool)> = shelf
        .instances()
        .iter()
        .map(|i| (i.id.pid, i.closed()))
        .collect();
    pids.sort();
    assert_eq!(pids, [(1, false), (2, true)]);
}

#[test]
fn unreadable_and_newer_snapshots_are_skipped() {
    let shelf = Shelf::new(&scratch("skip"), Path::new("/repo"));
    let mut newer = snapshot(1, "2026-09-25T10:00:00Z", Some("2026-09-25T10:00:00Z"));
    newer.version = VERSION + 1;
    put(&shelf, &newer);
    fs::write(shelf.path_for(2), "{ not json").unwrap();
    put(
        &shelf,
        &snapshot(3, "2026-09-25T08:00:00Z", Some("2026-09-25T08:00:00Z")),
    );
    let pids: Vec<u32> = shelf.instances().iter().map(|i| i.id.pid).collect();
    assert_eq!(pids, [3]);
}

#[test]
fn each_workspace_has_its_own_shelf() {
    let state = scratch("shelves");
    let a = Shelf::new(&state, Path::new("/dev/app"));
    let b = Shelf::new(&state, Path::new("/work/app"));
    assert_ne!(a.path_for(1), b.path_for(1), "same name, different repos");
    put(
        &a,
        &snapshot(1, "2026-09-25T08:00:00Z", Some("2026-09-25T08:00:00Z")),
    );
    assert!(b.instances().is_empty());
}

#[test]
fn pruning_keeps_the_newest_few_of_the_instances_that_are_gone() {
    let shelf = Shelf::new(&scratch("prune"), Path::new("/repo"));
    for pid in 1..=7 {
        let at = format!("2026-09-25T0{pid}:00:00Z");
        put(&shelf, &snapshot(1000 + pid, &at, Some(&at)));
    }
    // This process is running and has not closed: never pruned.
    put(
        &shelf,
        &snapshot(std::process::id(), "2026-09-24T00:00:00Z", None),
    );
    shelf.prune();
    let mut left: Vec<u32> = shelf.read_all().into_iter().map(|(_, s)| s.pid).collect();
    left.sort();
    assert_eq!(left, [1004, 1005, 1006, 1007, std::process::id()]);
}

#[test]
fn the_recorder_waits_for_the_view_to_rest_and_skips_what_it_wrote() {
    let shelf = Shelf::new(&scratch("recorder"), Path::new("/repo"));
    let mut recorder = Recorder::new(&shelf, 5);
    let start = Instant::now();
    recorder.touch(start);
    recorder.touch(start + Duration::from_millis(300));
    assert!(!recorder.is_due(start + Duration::from_millis(400)));
    assert!(
        recorder.is_due(start + Duration::from_millis(500)),
        "the first touch sets the deadline"
    );

    recorder.record(snapshot(5, "2026-09-25T08:00:00Z", None));
    let written = fs::metadata(shelf.path_for(5)).unwrap().modified().unwrap();
    fs::remove_file(shelf.path_for(5)).unwrap();
    // The same view, a second later, is not written again.
    recorder.record(snapshot(5, "2026-09-25T08:00:01Z", None));
    assert!(!shelf.path_for(5).exists());

    let mut moved = snapshot(5, "2026-09-25T08:00:02Z", None);
    moved.destination = Destination::Views;
    recorder.record(moved);
    assert!(fs::metadata(shelf.path_for(5)).unwrap().modified().unwrap() >= written);

    recorder.close(snapshot(5, "2026-09-25T08:00:03Z", None));
    assert!(shelf.instances()[0].closed());
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

#[test]
fn a_worktree_belongs_to_its_repository() {
    let root = scratch("worktree");
    let repo = root.join("repo");
    fs::create_dir_all(repo.join("src")).unwrap();
    git(&repo, &["init", "-q"]);
    git(
        &repo,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "init",
        ],
    );
    git(&repo, &["worktree", "add", "-q", "../feature"]);

    let repo = fs::canonicalize(&repo).unwrap();
    assert_eq!(workspace_of(&repo), repo);
    assert_eq!(workspace_of(&repo.join("src")), repo, "a subdirectory too");
    assert_eq!(workspace_of(&root.join("feature")), repo);

    let outside = scratch("no-git");
    assert_eq!(workspace_of(&outside), fs::canonicalize(&outside).unwrap());
}
