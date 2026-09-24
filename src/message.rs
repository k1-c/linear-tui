//! Message-passing types that decouple the UI loop from network I/O.
//!
//! The UI queues a [`Request`], the main loop spawns it onto the tokio runtime,
//! and the resulting [`Message`] is delivered back over an mpsc channel. The UI
//! thread never awaits a network call, so input and animation stay responsive.

use crate::api::ids::*;
use crate::api::types::*;
use crate::grouping::Preset;

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Teams,
    Viewer,
    TeamContext {
        team_id: TeamId,
    },
    Issues {
        team_id: TeamId,
        after: Option<String>,
        /// Which slice to fetch. Linear filters by workflow category on the
        /// server, so Active means every active issue, not the active ones
        /// among the latest page.
        preset: Preset,
    },
    MyIssues {
        user_id: UserId,
        after: Option<String>,
    },
    /// Workspace-wide full-text search, scoped to the current team.
    Search {
        term: String,
        team_id: Option<TeamId>,
    },
    /// Issues across the workspace matching what is typed into the command
    /// palette. `seq` tells a stale answer from the one for the latest query.
    PaletteSearch {
        term: String,
        seq: u64,
    },
    /// The saved views the user can open. Fetched once at startup, because the
    /// sidebar shows them whatever destination is on screen.
    CustomViews,
    /// The user's Favorites, for the sidebar.
    Favorites,
    /// One saved view's issues. Linear evaluates the view's filter, so this is
    /// a plain page request rather than a filter rebuilt on the client.
    ViewIssues {
        view_id: CustomViewId,
        after: Option<String>,
    },
    /// One saved project view's projects.
    ViewProjects {
        view_id: CustomViewId,
        after: Option<String>,
    },
    Projects {
        team_id: TeamId,
        after: Option<String>,
    },
    Cycles {
        team_id: TeamId,
        after: Option<String>,
    },
    IssueDetail {
        issue_id: IssueId,
    },
    ProjectIssues {
        project_id: ProjectId,
        after: Option<String>,
    },
    CycleIssues {
        cycle_id: CycleId,
        after: Option<String>,
    },
    UpdateStatus {
        issue_id: IssueId,
        state_id: WorkflowStateId,
    },
    UpdatePriority {
        issue_id: IssueId,
        priority: Priority,
    },
    UpdateAssignee {
        issue_id: IssueId,
        assignee_id: Option<UserId>,
    },
    CreateComment {
        issue_id: IssueId,
        body: String,
    },
    CreateIssue {
        team_id: TeamId,
        title: String,
        description: Option<String>,
        priority: Priority,
    },
    /// Hand a URL to the desktop's default browser.
    OpenUrl(String),
}

/// One page of a cursor-paginated list.
#[derive(Debug)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page_info: PageInfo,
    /// True when this page extends the existing list rather than replacing it.
    pub append: bool,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, page_info: PageInfo, append: bool) -> Self {
        Self {
            items,
            page_info,
            append,
        }
    }
}

impl Request {
    /// The page cursor this request continues from, if it is a next-page fetch.
    pub fn cursor(&self) -> Option<&str> {
        match self {
            Self::Issues { after, .. }
            | Self::MyIssues { after, .. }
            | Self::ViewIssues { after, .. }
            | Self::ViewProjects { after, .. }
            | Self::Projects { after, .. }
            | Self::Cycles { after, .. }
            | Self::ProjectIssues { after, .. }
            | Self::CycleIssues { after, .. } => after.as_deref(),
            _ => None,
        }
    }

    /// The issue an optimistic mutation already patched locally, whose copy
    /// must be re-read from the server if the mutation fails.
    pub fn patched_issue(&self) -> Option<&IssueId> {
        match self {
            Self::UpdateStatus { issue_id, .. }
            | Self::UpdatePriority { issue_id, .. }
            | Self::UpdateAssignee { issue_id, .. } => Some(issue_id),
            _ => None,
        }
    }

    /// What failed, as the error popup phrases it.
    pub fn failure(&self) -> &'static str {
        match self {
            Self::Teams => "Failed to load teams",
            Self::Viewer => "Failed to identify current user",
            Self::TeamContext { .. } => "Failed to load team context",
            Self::Issues { .. } => "Failed to load issues",
            Self::MyIssues { .. } => "Failed to load my issues",
            Self::Search { .. } | Self::PaletteSearch { .. } => "Search failed",
            Self::CustomViews => "Failed to load views",
            Self::Favorites => "Failed to load favorites",
            Self::ViewIssues { .. } => "Failed to load view issues",
            Self::ViewProjects { .. } => "Failed to load view projects",
            Self::Projects { .. } => "Failed to load projects",
            Self::Cycles { .. } => "Failed to load cycles",
            Self::IssueDetail { .. } => "Failed to load detail",
            Self::ProjectIssues { .. } => "Failed to load project issues",
            Self::CycleIssues { .. } => "Failed to load cycle issues",
            Self::UpdateStatus { .. } => "Failed to update status",
            Self::UpdatePriority { .. } => "Failed to update priority",
            Self::UpdateAssignee { .. } => "Failed to update assignee",
            Self::CreateComment { .. } => "Failed to post comment",
            Self::CreateIssue { .. } => "Failed to create issue",
            Self::OpenUrl(_) => "Failed to open browser",
        }
    }
}

/// Every response that fills a list names what it was fetched for — the team,
/// view, project, or cycle — so a reply that lands after the user has moved
/// on can be told apart from one for the list on screen and dropped.
#[derive(Debug)]
pub enum Message {
    Teams(Vec<Team>),
    Viewer(UserId),
    TeamContext {
        team_id: TeamId,
        states: Vec<WorkflowState>,
        members: Vec<User>,
    },
    Issues {
        team_id: TeamId,
        /// Echoed back so a page fetched for another preset can be dropped.
        preset: Preset,
        page: Page<Issue>,
    },
    MyIssues(Page<Issue>),
    PaletteResults {
        seq: u64,
        issues: Vec<Issue>,
    },
    SearchResults {
        term: String,
        team_id: Option<TeamId>,
        issues: Vec<Issue>,
    },
    CustomViews(Vec<CustomView>),
    Favorites(Vec<Favorite>),
    ViewIssues {
        /// Echoed back so a late page cannot be filed under the wrong view.
        view_id: CustomViewId,
        page: Page<Issue>,
    },
    ViewProjects {
        view_id: CustomViewId,
        page: Page<Project>,
    },
    Projects {
        team_id: TeamId,
        page: Page<Project>,
    },
    Cycles {
        team_id: TeamId,
        page: Page<Cycle>,
    },
    IssueDetail(Box<Issue>),
    ProjectIssues {
        project_id: ProjectId,
        page: Page<Issue>,
    },
    CycleIssues {
        cycle_id: CycleId,
        page: Page<Issue>,
    },
    IssueCreated {
        team_id: TeamId,
        issue: Box<Issue>,
    },
    /// A mutation succeeded; carries the status line to show.
    Mutated(&'static str),
    /// A request failed. Carries the request itself, so the failure can be
    /// undone precisely: a page cursor made retryable, an optimistic patch
    /// re-read from the server.
    Failed {
        request: Box<Request>,
        error: String,
    },
}
