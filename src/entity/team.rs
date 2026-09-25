//! A team, and the people in the workspace.

use serde::Deserialize;

use super::ids::*;

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub id: UserId,
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Team {
    pub id: TeamId,
    pub name: String,
    pub key: String,
    #[serde(default)]
    pub color: Option<String>,
    /// Whether the team runs cycles at all — teams with them switched off get
    /// no Cycles entry in the sidebar, exactly as in Linear.
    #[serde(default = "default_true", rename = "cyclesEnabled")]
    pub cycles_enabled: bool,
}

fn default_true() -> bool {
    true
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Viewer {
    pub id: UserId,
    pub name: String,
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
}
