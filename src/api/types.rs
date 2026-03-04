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
