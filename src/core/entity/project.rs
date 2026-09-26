//! A project, its status, and its milestones.

use serde::Deserialize;

use super::ids::*;
use super::issue::{Issue, Milestone};
use super::page::Connection;
use super::team::{Team, User};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub health: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub progress: Option<f64>,
    #[serde(default, rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(default, rename = "targetDate")]
    pub target_date: Option<String>,
    pub lead: Option<User>,
    pub issues: Option<Connection<Issue>>,
    #[serde(default)]
    pub url: Option<String>,
    /// Where the project stands: one of the workspace's project statuses.
    #[serde(default)]
    pub status: Option<ProjectStatus>,
    #[serde(default, rename = "priorityLabel")]
    pub priority_label: Option<String>,
    /// The teams it belongs to, fetched only when reading one project.
    #[serde(default)]
    pub teams: Option<Connection<Team>>,
    /// Its milestones, fetched only when reading one project.
    #[serde(default, rename = "projectMilestones")]
    pub milestones: Option<Connection<Milestone>>,
}

/// One of the workspace's project statuses: Backlog, Planned, In Progress,
/// and the rest, each of a Linear category (`type`).
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectStatus {
    pub id: ProjectStatusId,
    pub name: String,
    /// Linear's category: `backlog`, `planned`, `started`, `paused`,
    /// `completed`, or `canceled`. A string, since Linear may add more.
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}
