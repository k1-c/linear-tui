//! Message-passing types that decouple the UI loop from network I/O.
//!
//! The UI queues a [`Request`] (the use cases' output port), the main loop
//! spawns it onto the tokio runtime,
//! and the resulting [`Message`] is delivered back over an mpsc channel. The UI
//! thread never awaits a network call, so input and animation stay responsive.

use crate::entity::*;
use crate::usecase::Request;

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

/// What failed, as the error popup and the headless commands phrase it.
pub fn failure(request: &Request) -> &'static str {
    use crate::usecase::{agent, cycle, favorite, issue, notes, project, team, user, view};
    match request {
        Request::Issue(request) => match request {
            issue::Request::TeamIssues { .. } => "Failed to load issues",
            issue::Request::MyIssues { .. } => "Failed to load my issues",
            issue::Request::ViewIssues { .. } => "Failed to load view issues",
            issue::Request::ProjectIssues { .. } => "Failed to load project issues",
            issue::Request::CycleIssues { .. } => "Failed to load cycle issues",
            issue::Request::Detail { .. } => "Failed to load detail",
            issue::Request::Search { .. } | issue::Request::QuickSearch { .. } => "Search failed",
            issue::Request::SetStatus { .. } => "Failed to update status",
            issue::Request::SetPriority { .. } => "Failed to update priority",
            issue::Request::SetAssignee { .. } => "Failed to update assignee",
            issue::Request::Comment { .. } => "Failed to post comment",
            issue::Request::Create { .. } => "Failed to create issue",
            issue::Request::OpenInBrowser(_) => "Failed to open browser",
        },
        Request::Project(request) => match request {
            project::Request::TeamProjects { .. } => "Failed to load projects",
            project::Request::ViewProjects { .. } => "Failed to load view projects",
            project::Request::OpenInBrowser(_) => "Failed to open browser",
        },
        Request::Cycle(cycle::Request::TeamCycles { .. }) => "Failed to load cycles",
        Request::Team(team::Request::Teams) => "Failed to load teams",
        Request::Team(team::Request::Context { .. }) => "Failed to load team context",
        Request::View(view::Request::Views) => "Failed to load views",
        Request::Favorite(favorite::Request::Favorites) => "Failed to load favorites",
        Request::Favorite(favorite::Request::OpenInBrowser(_)) => "Failed to open browser",
        Request::User(user::Request::Viewer) => "Failed to identify current user",
        Request::Notes(notes::Request::Deliver(_))
        | Request::Agent(agent::Request::Focus { .. }) => "Failed to reach herdr",
    }
}
