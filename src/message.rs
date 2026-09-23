//! Message-passing types that decouple the UI loop from network I/O.
//!
//! The UI queues a [`Request`], the main loop spawns it onto the tokio runtime,
//! and the resulting [`Message`] is delivered back over an mpsc channel. The UI
//! thread never awaits a network call, so input and animation stay responsive.

use crate::api::types::*;
use crate::grouping::Preset;

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Teams,
    Viewer,
    TeamContext {
        team_id: String,
    },
    Issues {
        team_id: String,
        after: Option<String>,
        /// Which slice to fetch. Linear filters by workflow category on the
        /// server, so Active means every active issue, not the active ones
        /// among the latest page.
        preset: Preset,
    },
    MyIssues {
        user_id: String,
        after: Option<String>,
    },
    /// Workspace-wide full-text search, scoped to the current team.
    Search {
        term: String,
        team_id: Option<String>,
    },
    /// The saved views the user can open. Fetched once at startup, because the
    /// sidebar shows them whatever destination is on screen.
    CustomViews,
    /// The user's Favorites, for the sidebar.
    Favorites,
    /// One saved view's issues. Linear evaluates the view's filter, so this is
    /// a plain page request rather than a filter rebuilt on the client.
    ViewIssues {
        view_id: String,
        after: Option<String>,
    },
    Projects {
        team_id: String,
        after: Option<String>,
    },
    Cycles {
        team_id: String,
        after: Option<String>,
    },
    IssueDetail {
        issue_id: String,
    },
    ProjectIssues {
        project_id: String,
        after: Option<String>,
    },
    CycleIssues {
        cycle_id: String,
        after: Option<String>,
    },
    UpdateStatus {
        issue_id: String,
        state_id: String,
    },
    UpdatePriority {
        issue_id: String,
        priority: u8,
    },
    UpdateAssignee {
        issue_id: String,
        assignee_id: Option<String>,
    },
    CreateComment {
        issue_id: String,
        body: String,
    },
    CreateIssue {
        team_id: String,
        title: String,
        description: Option<String>,
        priority: u8,
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

#[derive(Debug)]
pub enum Message {
    Teams(Vec<Team>),
    Viewer(String),
    TeamContext {
        states: Vec<WorkflowState>,
        members: Vec<User>,
    },
    Issues {
        /// Echoed back so a page fetched for another preset can be dropped.
        preset: Preset,
        page: Page<Issue>,
    },
    MyIssues(Page<Issue>),
    SearchResults {
        term: String,
        issues: Vec<Issue>,
    },
    CustomViews(Vec<CustomView>),
    Favorites(Vec<Favorite>),
    ViewIssues {
        /// Echoed back so a late page cannot be filed under the wrong view.
        view_id: String,
        page: Page<Issue>,
    },
    Projects(Page<Project>),
    Cycles(Page<Cycle>),
    IssueDetail(Box<Issue>),
    ProjectIssues(Page<Issue>),
    CycleIssues(Page<Issue>),
    IssueCreated(Box<Issue>),
    /// A mutation succeeded; carries the status line to show.
    Mutated(&'static str),
    Error(String),
}
