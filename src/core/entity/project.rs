//! A project.

use serde::Deserialize;

use super::ids::*;
use super::issue::Issue;
use super::page::Connection;
use super::team::User;

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
}
