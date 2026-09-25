//! An issue, and what it is made of: its workflow state, priority,
//! labels, comments, and the issues it links to.

use serde::Deserialize;

use super::cycle::Cycle;
use super::ids::*;
use super::page::{Connection, Ref};
use super::project::Project;
use super::team::User;

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
    /// Every level, in the order the priority menu lists them. Position in
    /// this array is what [`Self::as_index`] and [`Self::from_index`] map.
    pub const ALL: [Priority; 5] = [
        Self::None,
        Self::Urgent,
        Self::High,
        Self::Medium,
        Self::Low,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Urgent => "Urgent",
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
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

    /// Index of this priority in the priority popup list (inverse of `from_index`).
    pub fn as_index(self) -> usize {
        self.as_u8() as usize
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

impl Priority {
    /// Read Linear's numeric priority. The schema types it as a float, so
    /// `2` and `2.0` both arrive; anything that is not one of the five levels
    /// is treated as no priority rather than rounded into one.
    fn from_api(value: f64) -> Self {
        match value {
            1.0 => Self::Urgent,
            2.0 => Self::High,
            3.0 => Self::Medium,
            4.0 => Self::Low,
            v => {
                if v != 0.0 {
                    tracing::debug!(priority = v, "unrecognised priority");
                }
                Self::None
            }
        }
    }
}

impl<'de> Deserialize<'de> for Priority {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = f64::deserialize(deserializer)?;
        Ok(Self::from_api(v))
    }
}

/// Workflow state type categories from the Linear API.
///
/// `WorkflowState.type` is a plain `String` in Linear's schema, so the set of
/// values is not something the schema pins down and Linear can add to it at any
/// time. An unrecognised value therefore has to fall back rather than fail:
/// rejecting one would fail the whole query, and a single state a workspace
/// happens to use would empty every issue list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateType {
    Triage,
    Backlog,
    Unstarted,
    Started,
    Completed,
    Cancelled,
    Duplicate,
    Unknown,
}

impl<'de> Deserialize<'de> for StateType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Ok(match raw.as_str() {
            "triage" => Self::Triage,
            "backlog" => Self::Backlog,
            "unstarted" => Self::Unstarted,
            "started" => Self::Started,
            "completed" => Self::Completed,
            "canceled" | "cancelled" => Self::Cancelled,
            "duplicate" => Self::Duplicate,
            other => {
                tracing::debug!(state_type = %other, "unrecognised workflow state type");
                Self::Unknown
            }
        })
    }
}

impl StateType {
    /// The name Linear gives the category.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Triage => "triage",
            Self::Backlog => "backlog",
            Self::Unstarted => "unstarted",
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Cancelled => "canceled",
            Self::Duplicate => "duplicate",
            Self::Unknown => "unknown",
        }
    }

    /// Order Linear groups states in: triage first, then the workflow from
    /// backlog to done. Drives the order of grouped sections in a list.
    pub fn rank(&self) -> u8 {
        match self {
            Self::Triage => 0,
            Self::Started => 1,
            Self::Unstarted => 2,
            Self::Backlog => 3,
            Self::Completed => 4,
            Self::Cancelled | Self::Duplicate => 5,
            Self::Unknown => 6,
        }
    }

    /// Whether the state counts as "active" work in Linear's Active/Backlog
    /// view split: started and unstarted are active, everything else is not.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Started | Self::Unstarted)
    }

    /// Whether the state belongs to Linear's Backlog view (backlog + triage).
    pub fn is_backlog(&self) -> bool {
        matches!(self, Self::Backlog | Self::Triage)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Issue {
    pub id: IssueId,
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
    #[serde(default, rename = "projectMilestone")]
    pub project_milestone: Option<Milestone>,
    pub cycle: Option<Cycle>,
    /// Who filed the issue. Linear shows it on every issue; a tracker that
    /// hides it makes triage guesswork.
    #[serde(default)]
    pub creator: Option<User>,
    #[serde(default)]
    pub estimate: Option<f64>,
    #[serde(default, rename = "dueDate")]
    pub due_date: Option<String>,
    /// Parent issue, so sub-issues can nest under it in a list.
    #[serde(default)]
    pub parent: Option<IssueRef>,
    /// Sub-issues, fetched only for the detail view.
    #[serde(default)]
    pub children: Option<Connection<IssueRef>>,
    /// Permalink to the issue on linear.app.
    #[serde(default)]
    pub url: Option<String>,
    /// Branch name Linear suggests for this issue.
    #[serde(default, rename = "branchName")]
    pub branch_name: Option<String>,
    /// The team the issue belongs to. Lists like My Issues span teams, and a
    /// status or assignee can only come from the issue's own team.
    #[serde(default)]
    pub team: Option<Ref<TeamId>>,
}

/// A shallow reference to another issue — enough to name it and colour its
/// status, without pulling a second full issue down for every row.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct IssueRef {
    pub id: IssueId,
    pub identifier: String,
    pub title: String,
    #[serde(default)]
    pub state: Option<WorkflowState>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Milestone {
    pub id: MilestoneId,
    pub name: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowState {
    pub id: WorkflowStateId,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default, rename = "type")]
    pub state_type: Option<StateType>,
    /// Where the state sits in the team's workflow. Groups are ordered by
    /// category first and this second, so a list reads top-to-bottom the way
    /// the board does.
    #[serde(default)]
    pub position: Option<f64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Label {
    pub id: LabelId,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Comment {
    pub id: CommentId,
    pub body: String,
    #[serde(default, rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(default, rename = "editedAt")]
    pub edited_at: Option<String>,
    pub user: Option<User>,
    /// Set on a reply; the detail view nests it under the comment it answers.
    #[serde(default)]
    pub parent: Option<Ref<CommentId>>,
}

/// What an issue list is narrowed to, besides its preset and search text.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IssueFilter {
    /// A workflow state, by name: a list can hold issues of several teams,
    /// each with its own "Todo".
    pub status: Option<String>,
    pub priority: Option<Priority>,
}

impl IssueFilter {
    /// Whether it narrows anything.
    pub fn is_active(&self) -> bool {
        self.status.is_some() || self.priority.is_some()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}
