//! A saved view.

use serde::Deserialize;

use super::ids::*;
use super::team::{Team, User};

/// A saved view. Linear users live in these — "my issues due today", "everything
/// my team touched this week" — and the filter lives on the server, so opening
/// one is a query against `customView.issues` rather than a filter reimplemented
/// here.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct CustomView {
    pub id: CustomViewId,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    /// False for a personal view, true for one shared with the workspace.
    #[serde(default)]
    pub shared: bool,
    /// What the view lists: `"Issue"` or `"Project"`. Absent on older
    /// payloads, which predate project views and so were all issue views.
    #[serde(default, rename = "modelName")]
    pub model_name: Option<String>,
    #[serde(default)]
    pub team: Option<Team>,
    #[serde(default)]
    pub owner: Option<User>,
}

impl CustomView {
    /// Whether this view lists issues. Project views filter projects, not
    /// issues, so opening one as an issue list would only ever show nothing.
    pub fn lists_issues(&self) -> bool {
        self.model_name.as_deref().is_none_or(|m| m == "Issue")
    }
}
