use ratatui::style::Color;
use serde::Deserialize;

use crate::config::Theme;

/// Priority levels from the Linear API (0=None, 1=Urgent, 2=High, 3=Medium, 4=Low).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Priority {
    #[default]
    None,
    Urgent,
    High,
    Medium,
    Low,
}

impl Priority {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Urgent => "Urgent",
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
        }
    }

    pub fn color(&self, theme: &Theme) -> Color {
        match self {
            Self::Urgent => theme.pri_urgent,
            Self::High => theme.pri_high,
            Self::Medium => theme.pri_medium,
            Self::Low => theme.pri_low,
            Self::None => theme.muted,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Urgent => 1,
            Self::High => 2,
            Self::Medium => 3,
            Self::Low => 4,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Urgent,
            2 => Self::High,
            3 => Self::Medium,
            4 => Self::Low,
            _ => Self::None,
        }
    }
}

impl From<f64> for Priority {
    fn from(v: f64) -> Self {
        match v as u8 {
            1 => Self::Urgent,
            2 => Self::High,
            3 => Self::Medium,
            4 => Self::Low,
            _ => Self::None,
        }
    }
}

impl<'de> Deserialize<'de> for Priority {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = f64::deserialize(deserializer)?;
        Ok(Self::from(v))
    }
}

/// Workflow state type categories from the Linear API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StateType {
    Triage,
    Backlog,
    Unstarted,
    Started,
    Completed,
    #[serde(alias = "canceled")]
    Cancelled,
}

impl StateType {
    pub fn color(&self) -> Color {
        match self {
            Self::Started => Color::Yellow,
            Self::Completed => Color::Green,
            Self::Cancelled => Color::DarkGray,
            Self::Backlog => Color::DarkGray,
            Self::Unstarted => Color::White,
            Self::Triage => Color::Magenta,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Connection<T> {
    pub nodes: Vec<T>,
    #[serde(default, rename = "pageInfo")]
    pub page_info: PageInfo,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PageInfo {
    #[serde(default, rename = "hasNextPage")]
    pub has_next_page: bool,
    #[serde(default, rename = "endCursor")]
    pub end_cursor: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub key: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Issue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default, rename = "priorityLabel")]
    pub priority_label: Option<String>,
    pub state: Option<WorkflowState>,
    pub assignee: Option<User>,
    #[serde(default)]
    pub labels: Option<Connection<Label>>,
    pub description: Option<String>,
    #[serde(default, rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(default, rename = "updatedAt")]
    pub updated_at: Option<String>,
    pub comments: Option<Connection<Comment>>,
    pub project: Option<Project>,
    pub cycle: Option<Cycle>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowState {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default, rename = "type")]
    pub state_type: Option<StateType>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Label {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Comment {
    pub id: String,
    pub body: String,
    #[serde(default, rename = "createdAt")]
    pub created_at: Option<String>,
    pub user: Option<User>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub progress: Option<f64>,
    #[serde(default, rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(default, rename = "targetDate")]
    pub target_date: Option<String>,
    pub lead: Option<User>,
    pub issues: Option<Connection<Issue>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Cycle {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub number: Option<f64>,
    #[serde(default, rename = "startsAt")]
    pub starts_at: Option<String>,
    #[serde(default, rename = "endsAt")]
    pub ends_at: Option<String>,
    #[serde(default)]
    pub progress: Option<f64>,
    pub issues: Option<Connection<Issue>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Viewer {
    pub id: String,
    pub name: String,
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct MutationSuccess {
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read fixture {path}: {e}"))
    }

    /// Helper: deserialize a fixture and extract the `data` wrapper that
    /// the actual GraphQL client strips. Fixtures store the inner `data` object
    /// directly (no `{"data": ...}` envelope) to match client.rs response types.

    #[test]
    fn deserialize_teams() {
        #[derive(Deserialize)]
        struct Resp {
            teams: Connection<Team>,
        }
        let resp: Resp = serde_json::from_str(&fixture("teams.json")).unwrap();
        assert_eq!(resp.teams.nodes.len(), 2);
        assert_eq!(resp.teams.nodes[0].key, "ENG");
    }

    #[test]
    fn deserialize_issues_with_pagination() {
        #[derive(Deserialize)]
        struct Resp {
            issues: Connection<Issue>,
        }
        let resp: Resp = serde_json::from_str(&fixture("issues.json")).unwrap();
        assert_eq!(resp.issues.nodes.len(), 2);
        assert!(resp.issues.page_info.has_next_page);
        assert_eq!(
            resp.issues.page_info.end_cursor.as_deref(),
            Some("cursor-abc123")
        );
    }

    #[test]
    fn deserialize_issue_with_null_fields() {
        #[derive(Deserialize)]
        struct Resp {
            issues: Connection<Issue>,
        }
        let resp: Resp = serde_json::from_str(&fixture("issues.json")).unwrap();
        let issue = &resp.issues.nodes[1];
        assert!(issue.assignee.is_none());
        assert!(issue.description.is_none());
    }

    #[test]
    fn deserialize_canceled_state_type() {
        #[derive(Deserialize)]
        struct Resp {
            issues: Connection<Issue>,
        }
        let resp: Resp = serde_json::from_str(&fixture("issues.json")).unwrap();
        let state = resp.issues.nodes[1].state.as_ref().unwrap();
        assert_eq!(state.state_type, Some(StateType::Cancelled));
    }

    #[test]
    fn deserialize_issue_detail_with_comments() {
        #[derive(Deserialize)]
        struct Resp {
            issue: Issue,
        }
        let resp: Resp = serde_json::from_str(&fixture("issue_detail.json")).unwrap();
        let comments = resp.issue.comments.as_ref().unwrap();
        assert_eq!(comments.nodes.len(), 1);
        assert_eq!(comments.nodes[0].body, "This is a comment");
        assert!(resp.issue.project.is_some());
        assert!(resp.issue.cycle.is_some());
    }

    #[test]
    fn deserialize_workflow_states_all_types() {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "workflowStates")]
            workflow_states: Connection<WorkflowState>,
        }
        let resp: Resp = serde_json::from_str(&fixture("workflow_states.json")).unwrap();
        let types: Vec<_> = resp
            .workflow_states
            .nodes
            .iter()
            .filter_map(|s| s.state_type)
            .collect();
        assert!(types.contains(&StateType::Started));
        assert!(types.contains(&StateType::Backlog));
        assert!(types.contains(&StateType::Cancelled));
        assert!(types.contains(&StateType::Unstarted));
        assert!(types.contains(&StateType::Completed));
        assert!(types.contains(&StateType::Triage));
    }

    #[test]
    fn deserialize_team_members() {
        #[derive(Deserialize)]
        struct TeamResp {
            team: TeamWithMembers,
        }
        #[derive(Deserialize)]
        struct TeamWithMembers {
            members: Connection<User>,
        }
        let resp: TeamResp = serde_json::from_str(&fixture("team_members.json")).unwrap();
        assert_eq!(resp.team.members.nodes.len(), 2);
    }

    #[test]
    fn deserialize_viewer() {
        #[derive(Deserialize)]
        struct Resp {
            viewer: Viewer,
        }
        let resp: Resp = serde_json::from_str(&fixture("viewer.json")).unwrap();
        assert_eq!(resp.viewer.name, "Test User");
    }

    #[test]
    fn deserialize_my_issues_with_float_priority() {
        #[derive(Deserialize)]
        struct Resp {
            issues: Connection<Issue>,
        }
        let resp: Resp = serde_json::from_str(&fixture("my_issues.json")).unwrap();
        // priority: 3.0 should deserialize to Medium
        assert_eq!(resp.issues.nodes[0].priority, Priority::Medium);
        // priority: 0.0 should deserialize to None
        assert_eq!(resp.issues.nodes[1].priority, Priority::None);
        // "canceled" state type
        let state = resp.issues.nodes[0].state.as_ref().unwrap();
        assert_eq!(state.state_type, Some(StateType::Cancelled));
        // Unicode description
        assert!(
            resp.issues.nodes[1]
                .description
                .as_ref()
                .unwrap()
                .contains("日本語")
        );
    }

    #[test]
    fn deserialize_projects() {
        #[derive(Deserialize)]
        struct TeamResp {
            team: TeamWithProjects,
        }
        #[derive(Deserialize)]
        struct TeamWithProjects {
            projects: Connection<Project>,
        }
        let resp: TeamResp = serde_json::from_str(&fixture("projects.json")).unwrap();
        assert_eq!(resp.team.projects.nodes.len(), 2);
        assert!(resp.team.projects.nodes[0].lead.is_some());
        assert!(resp.team.projects.nodes[1].lead.is_none());
        assert!(resp.team.projects.nodes[1].start_date.is_none());
    }

    #[test]
    fn deserialize_cycles() {
        #[derive(Deserialize)]
        struct TeamResp {
            team: TeamWithCycles,
        }
        #[derive(Deserialize)]
        struct TeamWithCycles {
            cycles: Connection<Cycle>,
        }
        let resp: TeamResp = serde_json::from_str(&fixture("cycles.json")).unwrap();
        assert_eq!(resp.team.cycles.nodes.len(), 2);
        assert_eq!(resp.team.cycles.nodes[0].name.as_deref(), Some("Sprint 1"));
        assert!(resp.team.cycles.nodes[1].name.is_none());
    }

    #[test]
    fn deserialize_priority_edge_cases() {
        // Integer values (Linear sometimes returns int instead of float)
        assert_eq!(
            serde_json::from_str::<Priority>("0").unwrap(),
            Priority::None
        );
        assert_eq!(
            serde_json::from_str::<Priority>("1").unwrap(),
            Priority::Urgent
        );
        assert_eq!(
            serde_json::from_str::<Priority>("4").unwrap(),
            Priority::Low
        );
        // Float values
        assert_eq!(
            serde_json::from_str::<Priority>("2.0").unwrap(),
            Priority::High
        );
        // Out of range
        assert_eq!(
            serde_json::from_str::<Priority>("99").unwrap(),
            Priority::None
        );
    }

    #[test]
    fn deserialize_state_type_both_spellings() {
        assert_eq!(
            serde_json::from_str::<StateType>("\"canceled\"").unwrap(),
            StateType::Cancelled
        );
        assert_eq!(
            serde_json::from_str::<StateType>("\"cancelled\"").unwrap(),
            StateType::Cancelled
        );
    }
}
