//! Linear's standing slices of a team's issues: Active, Backlog, All.
//!
//! Kept apart from `grouping` (which draws with ratatui colours) because the
//! use case layer names a preset in the requests it builds.

use super::issue::{Issue, StateType};

/// Linear's three standing slices of a team's issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Preset {
    #[default]
    Active,
    Backlog,
    All,
}

impl Preset {
    pub fn all() -> &'static [Preset] {
        &[Preset::Active, Preset::Backlog, Preset::All]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Backlog => "Backlog",
            Self::All => "All issues",
        }
    }

    /// The `state` clause of an `IssueFilter` that selects this slice on the
    /// server, or `None` for no restriction.
    ///
    /// Active is written as "none of the inactive categories" rather than "one
    /// of started/unstarted", so an issue in a category Linear adds later still
    /// comes back — the same fallback [`Self::admits`] makes on the client.
    pub fn state_filter(&self) -> Option<serde_json::Value> {
        match self {
            Self::All => None,
            Self::Active => Some(serde_json::json!({
                "type": { "nin": ["backlog", "triage", "completed", "canceled", "duplicate"] }
            })),
            Self::Backlog => Some(serde_json::json!({
                "type": { "in": ["backlog", "triage"] }
            })),
        }
    }

    /// Whether an issue belongs in this slice.
    ///
    /// An issue whose state category this client does not recognise counts as
    /// active: a new Linear category should surface somewhere obvious rather
    /// than vanish from the default view.
    pub fn admits(&self, issue: &Issue) -> bool {
        let state_type = issue.state.as_ref().and_then(|s| s.state_type);
        match self {
            Self::All => true,
            Self::Active => match state_type {
                Some(t) => t.is_active() || t == StateType::Unknown,
                None => true,
            },
            Self::Backlog => state_type.is_some_and(|t| t.is_backlog()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_state(kind: &str) -> Issue {
        serde_json::from_str(&format!(
            r#"{{"id":"1","identifier":"X-1","title":"t","priority":0,
                "state":{{"id":"s","name":"{kind}","type":"{kind}","position":1.0}},
                "assignee":null,"description":null,"comments":null,"project":null,"cycle":null}}"#
        ))
        .unwrap()
    }

    #[test]
    fn active_admits_started_and_unstarted_only() {
        assert!(Preset::Active.admits(&in_state("started")));
        assert!(Preset::Active.admits(&in_state("unstarted")));
        assert!(!Preset::Active.admits(&in_state("completed")));
        assert!(!Preset::Active.admits(&in_state("backlog")));
    }

    #[test]
    fn backlog_admits_backlog_and_triage() {
        assert!(Preset::Backlog.admits(&in_state("backlog")));
        assert!(Preset::Backlog.admits(&in_state("triage")));
        assert!(!Preset::Backlog.admits(&in_state("started")));
    }

    /// A category added to Linear after this client shipped must not make
    /// issues disappear from the default view.
    #[test]
    fn an_unknown_category_stays_visible_under_active() {
        let odd = in_state("inventedIn2027");
        assert!(Preset::Active.admits(&odd));
        assert!(Preset::All.admits(&odd));
    }

    #[test]
    fn only_all_is_unfiltered_on_the_server() {
        assert!(Preset::All.state_filter().is_none());
        assert!(Preset::Active.state_filter().is_some());
        assert!(Preset::Backlog.state_filter().is_some());
    }

    #[test]
    fn an_issue_without_a_state_stays_visible_under_active() {
        let issue: Issue = serde_json::from_str(
            r#"{"id":"1","identifier":"X-1","title":"t","priority":0,"state":null,
                "assignee":null,"description":null,"comments":null,"project":null,"cycle":null}"#,
        )
        .unwrap();
        assert!(Preset::Active.admits(&issue));
    }
}
