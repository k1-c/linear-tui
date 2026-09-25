//! The view snapshot: a small record of what linear-tui is showing, written
//! per working directory.
//!
//! One record serves three readers. The next launch in the same repository
//! reopens it; `linear-tui context` renders it for an agent, so "this issue"
//! or "the top three in this list" resolves without spelling out IDs; and a
//! restored herdr session reopens its linear-tui panes from it.
//!
//! Everything a snapshot points at is held by Linear ID, never by position:
//! the sidebar holds teams and views by index, and a reordered sidebar must
//! not reopen the wrong page. The format is documented in
//! `docs/view-snapshot.md`; it is a contract with other programs, so a field
//! is only ever added, and a breaking change bumps [`VERSION`].
//!
//! Like `store/`, this module knows nothing about screens as the TUI draws
//! them: `app` captures and restores a snapshot, this module only defines it
//! and keeps it on disk.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::api::ids::{CustomViewId, CycleId, FavoriteId, IssueId, ProjectId, TeamId};

mod files;
#[cfg(test)]
mod tests;
mod time;

pub use files::*;
pub use time::*;

/// The format version. Readers refuse a snapshot from a newer version rather
/// than guess at it.
pub const VERSION: u32 = 1;

/// At most this many rows of the list on screen are recorded. An agent is
/// pointed at the top of a list, and a restore does not need the rows at all.
pub const MAX_ROWS: usize = 100;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewSnapshot {
    pub version: u32,
    /// The repository root the snapshot is filed under, shared by all of its
    /// worktrees — or the directory itself outside git.
    pub workspace: PathBuf,
    /// Where the instance that wrote it was started.
    pub cwd: PathBuf,
    pub pid: u32,
    /// RFC 3339, UTC, to the second.
    pub updated_at: String,
    /// Set when the instance quit normally. A snapshot without it whose
    /// process is gone was left by a crash or a closed terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<String>,
    /// The herdr pane the instance ran in, when it ran inside herdr.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub herdr_pane: Option<String>,

    /// The selected team.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<TeamRef>,
    /// The sidebar destination the content pane belongs to.
    pub destination: Destination,
    pub screen: Screen,
    /// The project whose page is open, or is behind the open issue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<NamedRef<ProjectId>>,
    /// The cycle whose page is open, or is behind the open issue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle: Option<NamedRef<CycleId>>,
    /// The issue open in the detail view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<IssueRef>,
    /// Set while the team's list shows workspace search results for this term.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,

    pub group_by: GroupBy,
    /// How each issue list is shaped. A list left as it opens is omitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lists: Vec<ListSettings>,

    /// The first [`MAX_ROWS`] rows of the list on screen, in the order they
    /// are drawn — for an issue page, the list it was opened from.
    #[serde(default)]
    pub rows: Vec<Row>,
    /// How many rows the list has in all, loaded so far.
    #[serde(default)]
    pub rows_total: usize,
    /// The row under the cursor, as an index into `rows`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_row: Option<usize>,
}

impl ViewSnapshot {
    /// Whether two snapshots record the same view, whenever and by whichever
    /// instance they were written.
    pub fn same_view(&self, other: &Self) -> bool {
        let mut other = other.clone();
        other.updated_at.clone_from(&self.updated_at);
        other.closed_at.clone_from(&self.closed_at);
        *self == other
    }

    /// The row under the cursor.
    pub fn selected(&self) -> Option<&Row> {
        self.rows.get(self.selected_row?)
    }
}

/// Who is writing a snapshot, and from where.
#[derive(Debug, Clone)]
pub struct Origin {
    pub workspace: PathBuf,
    pub cwd: PathBuf,
    pub pid: u32,
    pub herdr_pane: Option<String>,
}

impl Origin {
    /// This process, started in `cwd`.
    pub fn current(cwd: PathBuf) -> Self {
        Self {
            workspace: workspace_of(&cwd),
            cwd,
            pid: std::process::id(),
            herdr_pane: std::env::var("HERDR_PANE_ID")
                .ok()
                .filter(|p| !p.is_empty()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamRef {
    pub id: TeamId,
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedRef<I> {
    pub id: I,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueRef {
    pub id: IssueId,
    /// `ENG-123`.
    pub identifier: String,
    pub title: String,
}

/// A sidebar destination, by ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Destination {
    MyIssues,
    /// The workspace's index of saved views. A team's is a [`Section`].
    Views,
    /// One saved view.
    View {
        id: CustomViewId,
        name: String,
    },
    Team {
        team: TeamRef,
        section: Section,
    },
    /// A favorite opened in place: a project, a cycle, or an issue.
    Favorite {
        id: FavoriteId,
        title: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    Issues,
    Cycles,
    Projects,
    Views,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    IssueList,
    IssueDetail,
    ProjectList,
    ProjectDetail,
    CycleList,
    CycleDetail,
    ViewList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    Status,
    Assignee,
    Priority,
    Project,
    None,
}

/// Which of the five issue lists a [`ListSettings`] shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Team,
    My,
    View,
    Project,
    Cycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    Active,
    Backlog,
    All,
}

/// How one issue list is shaped: its preset chip, its filters, and the issue
/// under its cursor. The search box is not kept — it is text being typed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListSettings {
    pub source: Source,
    pub preset: Preset,
    /// A workflow state name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// A priority label: `Urgent`, `High`, `Medium`, `Low`, or `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<IssueRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowKind {
    Issue,
    Project,
    Cycle,
    View,
}

/// One row of the list on screen, with what an agent needs to tell it apart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub kind: RowKind,
    /// The Linear ID of the issue, project, cycle, or view.
    pub id: String,
    /// `ENG-123`, for an issue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    pub title: String,
    /// An issue's workflow state, a project's state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}
