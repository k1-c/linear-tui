//! Linear's answers: what comes back for each [`Request`] a use case made.
//!
//! The UI queues a [`Request`] (the use cases' output port), the main loop
//! hands it to `crate::infra::dispatch` on the tokio runtime, and the
//! resulting [`Message`] comes back over an mpsc channel. The UI thread never
//! awaits a network call, so input and animation stay responsive.

use crate::core::entity::*;
use crate::core::usecase::Request;

/// Every response that fills a list names what it was fetched for — the team,
/// view, project, or cycle — so a reply that lands after the user has moved
/// on can be told apart from one for the list on screen and dropped.
#[derive(Debug)]
pub enum Message {
    Teams(Vec<Team>),
    Viewer {
        id: UserId,
        organization: Option<Organization>,
    },
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
    /// The labels a team's issues can carry.
    Labels {
        team_id: TeamId,
        labels: Vec<Label>,
    },
    /// A comment or reply is posted; Linear gives back its id and URL.
    CommentPosted {
        issue_id: IssueId,
        comment: Box<Comment>,
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
    use crate::core::usecase::{agent, cycle, favorite, issue, notes, project, team, user, view};
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
            issue::Request::Update { .. } => "Failed to update issue",
            issue::Request::Comment { .. } => "Failed to post comment",
            issue::Request::EditComment { .. } => "Failed to edit comment",
            issue::Request::DeleteComment { .. } => "Failed to delete comment",
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
        Request::Team(team::Request::Labels { .. }) => "Failed to load labels",
        Request::View(view::Request::Views) => "Failed to load views",
        Request::Favorite(favorite::Request::Favorites) => "Failed to load favorites",
        Request::Favorite(favorite::Request::OpenInBrowser(_)) => "Failed to open browser",
        Request::User(user::Request::Viewer) => "Failed to identify current user",
        Request::Notes(notes::Request::Deliver(_))
        | Request::Agent(agent::Request::Focus { .. }) => "Failed to reach herdr",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::entity::{Handoff, TeamId};
    use crate::core::usecase::{agent, cycle, favorite, issue, notes, project, team, user, view};

    /// A failed load says what could not be loaded, a failed change what
    /// could not be changed, and anything handed to herdr that herdr could
    /// not be reached.
    #[test]
    fn a_failure_says_what_could_not_be_done() {
        let team = || TeamId::from("t");
        let cases: Vec<(Request, &str)> = vec![
            (
                issue::Request::TeamIssues {
                    team_id: team(),
                    preset: Default::default(),
                    after: None,
                }
                .into(),
                "Failed to load issues",
            ),
            (
                issue::Request::MyIssues {
                    user_id: "u".into(),
                    after: None,
                }
                .into(),
                "Failed to load my issues",
            ),
            (
                issue::Request::ViewIssues {
                    view_id: "v".into(),
                    after: None,
                }
                .into(),
                "Failed to load view issues",
            ),
            (
                issue::Request::ProjectIssues {
                    project_id: "p".into(),
                    after: None,
                }
                .into(),
                "Failed to load project issues",
            ),
            (
                issue::Request::CycleIssues {
                    cycle_id: "c".into(),
                    after: None,
                }
                .into(),
                "Failed to load cycle issues",
            ),
            (
                issue::Request::Detail {
                    issue_id: "i".into(),
                }
                .into(),
                "Failed to load detail",
            ),
            (
                issue::Request::Search {
                    term: "x".into(),
                    team_id: None,
                }
                .into(),
                "Search failed",
            ),
            (
                issue::Request::QuickSearch {
                    term: "x".into(),
                    seq: 1,
                }
                .into(),
                "Search failed",
            ),
            (
                issue::Request::SetStatus {
                    issue_id: "i".into(),
                    state_id: "s".into(),
                }
                .into(),
                "Failed to update status",
            ),
            (
                issue::Request::SetPriority {
                    issue_id: "i".into(),
                    priority: Default::default(),
                }
                .into(),
                "Failed to update priority",
            ),
            (
                issue::Request::SetAssignee {
                    issue_id: "i".into(),
                    assignee_id: None,
                }
                .into(),
                "Failed to update assignee",
            ),
            (
                issue::Request::Update {
                    issue_id: "i".into(),
                    changes: Default::default(),
                }
                .into(),
                "Failed to update issue",
            ),
            (
                issue::Request::Comment {
                    issue_id: "i".into(),
                    body: "b".into(),
                    parent_id: None,
                }
                .into(),
                "Failed to post comment",
            ),
            (
                issue::Request::EditComment {
                    issue_id: "i".into(),
                    comment_id: "c".into(),
                    body: "b".into(),
                }
                .into(),
                "Failed to edit comment",
            ),
            (
                issue::Request::DeleteComment {
                    issue_id: "i".into(),
                    comment_id: "c".into(),
                }
                .into(),
                "Failed to delete comment",
            ),
            (
                issue::Request::Create {
                    team_id: team(),
                    draft: Default::default(),
                }
                .into(),
                "Failed to create issue",
            ),
            (
                issue::Request::OpenInBrowser("u".into()).into(),
                "Failed to open browser",
            ),
            (
                project::Request::TeamProjects {
                    team_id: team(),
                    after: None,
                }
                .into(),
                "Failed to load projects",
            ),
            (
                project::Request::ViewProjects {
                    view_id: "v".into(),
                    after: None,
                }
                .into(),
                "Failed to load view projects",
            ),
            (
                project::Request::OpenInBrowser("u".into()).into(),
                "Failed to open browser",
            ),
            (
                cycle::Request::TeamCycles {
                    team_id: team(),
                    after: None,
                }
                .into(),
                "Failed to load cycles",
            ),
            (team::Request::Teams.into(), "Failed to load teams"),
            (
                team::Request::Context { team_id: team() }.into(),
                "Failed to load team context",
            ),
            (
                team::Request::Labels { team_id: team() }.into(),
                "Failed to load labels",
            ),
            (view::Request::Views.into(), "Failed to load views"),
            (
                favorite::Request::Favorites.into(),
                "Failed to load favorites",
            ),
            (
                favorite::Request::OpenInBrowser("u".into()).into(),
                "Failed to open browser",
            ),
            (
                user::Request::Viewer.into(),
                "Failed to identify current user",
            ),
            (
                notes::Request::Deliver(Handoff::Focus { pane: "p".into() }).into(),
                "Failed to reach herdr",
            ),
            (
                agent::Request::Focus { pane: "p".into() }.into(),
                "Failed to reach herdr",
            ),
        ];
        for (request, wording) in cases {
            assert_eq!(failure(&request), wording, "{request:?}");
        }
    }
}
