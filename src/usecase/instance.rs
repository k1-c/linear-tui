//! linear-tui instances and the views they record: which one a launch
//! reopens, and which one an agent reads.
//!
//! [`to_reopen`] picks the view a new instance reopens ("relaunch where I
//! left off"); [`to_report`] picks the view `linear-tui context` shows an
//! agent ("what am I looking at"). Reading the records from disk and asking
//! the system which processes run is the adapter's (`crate::adapter::snapshot`).

use crate::entity::{Instance, InstanceId};

/// The instances, the one that recorded its view last first.
fn newest_first(instances: &[Instance]) -> Vec<&Instance> {
    let mut sorted: Vec<&Instance> = instances.iter().collect();
    sorted.sort_by(|a, b| b.view.updated_at.cmp(&a.view.updated_at));
    sorted
}

/// **Relaunch where I left off**: the instance whose view a launch in the
/// same repository reopens.
///
/// The instance that quit most recently comes first: quitting is how the
/// user says "this is where I was". When none quit normally — the terminal
/// was closed, or the machine went down — the one that recorded its view
/// last is reopened. A record left under the launching process's own id is
/// from an earlier process that happened to have it, and is reopened only
/// if that process quit normally.
pub fn to_reopen<'a>(instances: &'a [Instance], me: &InstanceId) -> Option<&'a Instance> {
    let candidates: Vec<&Instance> = newest_first(instances)
        .into_iter()
        .filter(|i| i.id.pid != me.pid || i.closed())
        .collect();
    let quit_last = candidates
        .iter()
        .filter(|i| i.closed())
        .max_by(|a, b| a.view.closed_at.cmp(&b.view.closed_at))
        .copied();
    quit_last.or(candidates.first().copied())
}

/// **An agent reads what I am looking at**: the instance whose view
/// `linear-tui context` reports, and whether it is still open.
///
/// An instance still open comes first — the one that moved last, when
/// several are. With none open, the last view recorded is reported, marked
/// as closed so the agent knows it may be stale.
pub fn to_report(instances: &[Instance]) -> Option<(&Instance, bool)> {
    let sorted = newest_first(instances);
    match sorted.iter().find(|i| i.running && !i.closed()) {
        Some(open) => Some((open, true)),
        None => sorted.first().map(|i| (*i, false)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::snapshot::ViewSnapshot;

    /// An instance of process `pid` that recorded its view at hour `updated`
    /// and, if `closed` is set, quit at that hour.
    fn instance(pid: u32, updated: u32, closed: Option<u32>, running: bool) -> Instance {
        let at = |hour: u32| format!("2026-09-25T{hour:02}:00:00Z");
        let mut view: ViewSnapshot = serde_json::from_str(&format!(
            r#"{{"version":1,"workspace":"/repo","cwd":"/repo","pid":{pid},
                "updated_at":"{}","destination":{{"kind":"my_issues"}},"screen":"issue_list",
                "group_by":"status"}}"#,
            at(updated)
        ))
        .unwrap();
        view.closed_at = closed.map(at);
        Instance {
            id: InstanceId {
                workspace: "/repo".into(),
                pid,
            },
            view,
            running,
        }
    }

    fn me(pid: u32) -> InstanceId {
        InstanceId {
            workspace: "/repo".into(),
            pid,
        }
    }

    /// Of the instances that quit, the one that quit last is reopened, even
    /// over one that moved later without quitting.
    #[test]
    fn the_instance_that_quit_last_is_reopened() {
        let instances = [
            instance(1, 9, Some(9), false),
            instance(2, 10, Some(10), false),
            instance(3, 11, None, false),
        ];
        assert_eq!(to_reopen(&instances, &me(99)).unwrap().id.pid, 2);
    }

    /// When none quit normally, the view recorded last is reopened.
    #[test]
    fn without_a_clean_quit_the_last_view_recorded_is_reopened() {
        let instances = [instance(1, 9, None, false), instance(2, 11, None, false)];
        assert_eq!(to_reopen(&instances, &me(99)).unwrap().id.pid, 2);
    }

    /// A record under the launching process's own id is an earlier
    /// process's, and is passed over unless that process quit normally.
    #[test]
    fn a_record_under_my_own_pid_is_passed_over_unless_it_quit() {
        let stale = [instance(7, 11, None, true), instance(1, 9, None, false)];
        assert_eq!(to_reopen(&stale, &me(7)).unwrap().id.pid, 1);
        let quit = [instance(7, 11, Some(11), false)];
        assert_eq!(to_reopen(&quit, &me(7)).unwrap().id.pid, 7);
    }

    /// With no record at all, there is nothing to reopen.
    #[test]
    fn with_no_record_nothing_is_reopened() {
        assert!(to_reopen(&[], &me(1)).is_none());
    }

    /// An agent is shown the open instance that moved last.
    #[test]
    fn an_agent_reads_the_open_instance_that_moved_last() {
        let instances = [
            instance(1, 9, None, true),
            instance(2, 10, None, true),
            instance(3, 11, Some(11), false),
        ];
        let (shown, open) = to_report(&instances).unwrap();
        assert_eq!((shown.id.pid, open), (2, true));
    }

    /// With none open, the last view recorded is shown, marked as closed.
    #[test]
    fn with_none_open_the_last_view_is_reported_as_closed() {
        let instances = [instance(1, 9, Some(9), false), instance(2, 10, None, false)];
        let (shown, open) = to_report(&instances).unwrap();
        assert_eq!((shown.id.pid, open), (2, false));
    }
}
