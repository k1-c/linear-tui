use ratatui::style::Color;
use serde::Deserialize;

use super::ids::*;

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

    /// Linear draws priority as a small bar chart that grows with urgency, and
    /// urgent as an exclamation. These block glyphs are the terminal stand-in.
    pub fn glyph(&self) -> &'static str {
        match self {
            Self::Urgent => "!",
            Self::High => "\u{2587}",
            Self::Medium => "\u{2585}",
            Self::Low => "\u{2583}",
            Self::None => "\u{00b7}",
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

    /// The status glyph Linear draws beside an issue — a progress circle that
    /// fills as the issue moves through the workflow.
    pub fn glyph(&self) -> &'static str {
        match self {
            Self::Triage => "\u{25c8}",
            Self::Backlog => "\u{25cc}",
            Self::Unstarted => "\u{25cb}",
            Self::Started => "\u{25d0}",
            Self::Completed => "\u{25cf}",
            Self::Cancelled | Self::Duplicate => "\u{2298}",
            Self::Unknown => "\u{25cb}",
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

    pub fn color(&self) -> Color {
        match self {
            Self::Started => Color::Yellow,
            Self::Completed => Color::Green,
            Self::Cancelled | Self::Duplicate => Color::DarkGray,
            Self::Backlog => Color::DarkGray,
            Self::Unstarted => Color::White,
            Self::Triage => Color::Magenta,
            Self::Unknown => Color::White,
        }
    }
}

/// Parse a Linear `#rrggbb` colour into a terminal colour.
///
/// Linear hands back a hex string for every state, label, project, and team, so
/// honouring it is what makes a list look like the workspace the user knows
/// rather than a generic eight-colour palette.
pub fn hex_color(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() != 6 {
        return None;
    }
    let byte = |r: std::ops::Range<usize>| u8::from_str_radix(hex.get(r)?, 16).ok();
    Some(Color::Rgb(byte(0..2)?, byte(2..4)?, byte(4..6)?))
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

/// Just the id of a related record, for parent/child links.
/// A reference to another entity by id alone, typed by what it points at.
#[derive(Debug, Clone, Deserialize)]
pub struct Ref<I> {
    pub id: I,
}

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

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Cycle {
    pub id: CycleId,
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

/// One entry in the user's Favorites — Linear's own sidebar, and the way most
/// people actually get around a workspace.
///
/// A favorite can point at nearly anything (about twenty kinds); `type` says
/// which, and exactly one of the object fields is set to match. `title`,
/// `color`, and `url` are resolved by Linear, so every kind can be shown — and
/// opened in the browser — even when this client has no screen for it.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Favorite {
    pub id: FavoriteId,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default, rename = "sortOrder")]
    pub sort_order: f64,
    /// The folder this favorite sits in, when it is in one.
    #[serde(default)]
    pub parent: Option<Ref<FavoriteId>>,
    #[serde(default, rename = "folderName")]
    pub folder_name: Option<String>,
    /// For a built-in page ("issues", "projects", "cycles", …).
    #[serde(default, rename = "predefinedViewType")]
    pub predefined_view_type: Option<String>,
    #[serde(default, rename = "predefinedViewTeam")]
    pub predefined_view_team: Option<Ref<TeamId>>,
    #[serde(default, rename = "customView")]
    pub custom_view: Option<Ref<CustomViewId>>,
    #[serde(default)]
    pub issue: Option<IssueRef>,
    #[serde(default)]
    pub project: Option<Project>,
    #[serde(default)]
    pub cycle: Option<Cycle>,
}

impl Favorite {
    pub fn is_folder(&self) -> bool {
        self.kind == "folder"
    }

    /// What the sidebar calls it.
    pub fn label(&self) -> String {
        self.title
            .clone()
            .or_else(|| self.folder_name.clone())
            .or_else(|| self.project.as_ref().map(|p| p.name.clone()))
            .or_else(|| self.issue.as_ref().map(|i| i.title.clone()))
            .unwrap_or_else(|| self.kind.clone())
    }
}

impl CustomView {
    /// Whether this view lists issues. Project views filter projects, not
    /// issues, so opening one as an issue list would only ever show nothing.
    pub fn lists_issues(&self) -> bool {
        self.model_name.as_deref().is_none_or(|m| m == "Issue")
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Viewer {
    pub id: UserId,
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
    fn deserialize_issue_url_and_branch_name() {
        #[derive(Deserialize)]
        struct Resp {
            issues: Connection<Issue>,
        }
        let resp: Resp = serde_json::from_str(&fixture("issues.json")).unwrap();
        let issue = &resp.issues.nodes[0];
        assert_eq!(
            issue.url.as_deref(),
            Some("https://linear.app/acme/issue/ENG-100")
        );
        assert_eq!(
            issue.branch_name.as_deref(),
            Some("test-user/eng-100-fix-login-bug")
        );
    }

    /// `url`/`branchName` are absent from older cached payloads; they must not
    /// break deserialization.
    #[test]
    fn deserialize_issue_without_url_fields() {
        let issue: Issue = serde_json::from_str(
            r#"{"id":"i1","identifier":"ENG-1","title":"t","state":null,"assignee":null,
                "description":null,"comments":null,"project":null,"cycle":null}"#,
        )
        .unwrap();
        assert!(issue.url.is_none());
        assert!(issue.branch_name.is_none());
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
        assert!(types.contains(&StateType::Duplicate));
        // A category this client does not know still parses, and a state
        // without one at all stays None.
        assert!(types.contains(&StateType::Unknown));
        assert_eq!(resp.workflow_states.nodes.len(), 9);
        assert!(
            resp.workflow_states
                .nodes
                .iter()
                .any(|s| s.state_type.is_none())
        );
    }

    #[test]
    fn unknown_state_type_does_not_fail_the_query() {
        // Linear types `WorkflowState.type` as a String and has added values
        // over time. Rejecting an unfamiliar one would fail the whole response
        // and empty every list, so it degrades to Unknown instead.
        assert_eq!(
            serde_json::from_str::<StateType>("\"duplicate\"").unwrap(),
            StateType::Duplicate
        );
        assert_eq!(
            serde_json::from_str::<StateType>("\"somethingNewIn2027\"").unwrap(),
            StateType::Unknown
        );
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
        assert_eq!(
            serde_json::from_str::<Priority>("2.5").unwrap(),
            Priority::None,
            "a fraction is not rounded into a level"
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

    #[test]
    fn deserialize_issue_detail_metadata() {
        #[derive(Deserialize)]
        struct Resp {
            issue: Issue,
        }
        let resp: Resp = serde_json::from_str(&fixture("issue_detail_full.json")).unwrap();
        let issue = resp.issue;
        assert_eq!(issue.creator.as_ref().unwrap().name, "Other Person");
        assert_eq!(issue.estimate, Some(3.0));
        assert_eq!(issue.due_date.as_deref(), Some("2026-10-01"));
        assert_eq!(issue.parent.as_ref().unwrap().identifier, "ENG-151");
        assert_eq!(issue.children.as_ref().unwrap().nodes.len(), 1);
        assert_eq!(issue.project_milestone.as_ref().unwrap().name, "Beta");
        assert_eq!(issue.state.as_ref().unwrap().position, Some(2.0));
        let comments = &issue.comments.as_ref().unwrap().nodes;
        assert!(comments[0].parent.is_none());
        assert_eq!(comments[1].parent.as_ref().unwrap().id, "comment-001");
        assert!(comments[1].edited_at.is_some());
    }

    /// Every field added for the richer detail view is optional: older
    /// fixtures and trimmed queries must still parse.
    #[test]
    fn deserialize_issue_without_the_new_metadata() {
        #[derive(Deserialize)]
        struct Resp {
            issue: Issue,
        }
        let resp: Resp = serde_json::from_str(&fixture("issue_detail.json")).unwrap();
        let issue = resp.issue;
        assert!(issue.creator.is_none());
        assert!(issue.estimate.is_none());
        assert!(issue.due_date.is_none());
        assert!(issue.parent.is_none());
        assert!(issue.children.is_none());
        assert!(issue.project_milestone.is_none());
        assert!(issue.comments.unwrap().nodes[0].parent.is_none());
    }

    #[test]
    fn deserialize_custom_views() {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "customViews")]
            custom_views: Connection<CustomView>,
        }
        let resp: Resp = serde_json::from_str(&fixture("custom_views.json")).unwrap();
        let views = resp.custom_views.nodes;
        assert_eq!(views.len(), 3);
        assert!(!views[0].shared);
        assert!(views[1].shared);
        assert!(views[0].team.is_none());
        assert!(!views[1].team.as_ref().unwrap().cycles_enabled);
        assert!(views[1].description.is_none());
        assert!(views[0].lists_issues());
        assert!(!views[1].lists_issues(), "a project view");
        assert!(views[2].lists_issues(), "no modelName: an old issue view");
    }

    #[test]
    fn deserialize_view_issues_with_pagination() {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "customView")]
            custom_view: ViewIssues,
        }
        #[derive(Deserialize)]
        struct ViewIssues {
            issues: Connection<Issue>,
        }
        let resp: Resp = serde_json::from_str(&fixture("view_issues.json")).unwrap();
        assert_eq!(resp.custom_view.issues.nodes.len(), 1);
        assert_eq!(
            resp.custom_view.issues.page_info.end_cursor.as_deref(),
            Some("view-cursor-1")
        );
    }

    /// A team payload without `color`/`cyclesEnabled` still parses, and
    /// assumes cycles are on so the sidebar does not hide a real page.
    #[test]
    fn a_team_without_the_new_fields_defaults_sensibly() {
        #[derive(Deserialize)]
        struct Resp {
            teams: Connection<Team>,
        }
        let resp: Resp = serde_json::from_str(&fixture("teams.json")).unwrap();
        assert!(resp.teams.nodes[0].color.is_none());
        assert!(resp.teams.nodes[0].cycles_enabled);
    }

    #[test]
    fn deserialize_favorites() {
        #[derive(Deserialize)]
        struct Resp {
            favorites: Connection<Favorite>,
        }
        let resp: Resp = serde_json::from_str(&fixture("favorites.json")).unwrap();
        let favs = resp.favorites.nodes;
        assert_eq!(favs.len(), 6);
        assert_eq!(favs[0].kind, "project");
        assert_eq!(favs[0].project.as_ref().unwrap().name, "Q1 Release");
        assert_eq!(favs[1].custom_view.as_ref().unwrap().id, "view-001");
        assert!(favs[2].is_folder());
        assert_eq!(favs[2].label(), "Ops");
        assert_eq!(favs[3].parent.as_ref().unwrap().id, "fav-3");
        assert_eq!(favs[3].issue.as_ref().unwrap().identifier, "ENG-100");
        assert_eq!(favs[4].predefined_view_type.as_deref(), Some("projects"));
        // A kind this client knows nothing about still parses and has a name.
        assert_eq!(favs[5].label(), "Onboarding doc");
        assert!(favs[5].title.is_some());
    }

    #[test]
    fn hex_colours_parse() {
        assert_eq!(hex_color("#5e6ad2"), Some(Color::Rgb(0x5e, 0x6a, 0xd2)));
        assert_eq!(hex_color("f2c94c"), Some(Color::Rgb(0xf2, 0xc9, 0x4c)));
        assert_eq!(hex_color("#fff"), None);
        assert_eq!(hex_color("#zzzzzz"), None);
    }
}
