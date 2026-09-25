//! What the user can do, independent of how they asked — one module per
//! aggregate, each holding every use case on it: finding, reading, and
//! changing alike.
//!
//! | Module | Aggregate | Use cases |
//! | --- | --- | --- |
//! | [`issue`] | an issue, with its comments | list, page, narrow, search, read, change, comment, create, copy, open |
//! | [`project`] | a project | list a team's or a view's, open |
//! | [`cycle`] | a cycle | list a team's |
//! | [`team`] | a team, its states and members | pick at startup, switch, know its states and members |
//! | [`view`] | a saved view | list, pick those a Views page shows |
//! | [`favorite`] | a favorite | list, where one leads |
//! | [`user`] | the signed-in user | know who that is |
//! | [`notes`] | notes for an agent | write, compose, send, discard |
//! | [`agent`] | a coding agent beside linear-tui | which one works on an issue, jump to it |
//! | [`instance`] | a running linear-tui and the view it records | which to reopen, which an agent reads |
//!
//! A use case takes the [`Store`](crate::store::Store) and explicit
//! arguments — which issue, which value — applies the optimistic change, and
//! returns the [`Request`] that makes it real. It never looks at a cursor or
//! a popup: resolving "the issue under the cursor" is the caller's job, so a
//! key, a click, the command palette, and a headless subcommand all end up
//! here. It never performs I/O either: an adapter outside this layer
//! (`dispatch`) carries the request out.
//!
//! This layer is the specification. Each use case is a function whose doc
//! comment says, in plain words, what the user can do and the rules it
//! follows; its tests state those rules one by one. How to write them is in
//! `docs/development.md` ("The use case layer").

use crate::store::{ListOf, Opening, PageAsk};

pub mod agent;
pub mod cycle;
pub mod favorite;
pub mod instance;
pub mod issue;
pub mod notes;
pub mod project;
pub mod team;
pub mod user;
pub mod view;

/// What a use case asks of the world outside linear-tui, by the aggregate it
/// is about. The use cases' output port: `dispatch` carries each out.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Issue(issue::Request),
    Project(project::Request),
    Cycle(cycle::Request),
    Team(team::Request),
    View(view::Request),
    Favorite(favorite::Request),
    User(user::Request),
    Notes(notes::Request),
    Agent(agent::Request),
}

macro_rules! from_aggregate {
    ($($variant:ident($module:ident)),* $(,)?) => {$(
        impl From<$module::Request> for Request {
            fn from(request: $module::Request) -> Self {
                Self::$variant(request)
            }
        }
    )*};
}

from_aggregate!(
    Issue(issue),
    Project(project),
    Cycle(cycle),
    Team(team),
    View(view),
    Favorite(favorite),
    User(user),
    Notes(notes),
    Agent(agent),
);

impl Request {
    /// The request for a page the store asks for, from the aggregate the
    /// list is of.
    pub fn page(ask: PageAsk) -> Self {
        let PageAsk { of, after } = ask;
        match of {
            ListOf::TeamIssues { team_id, preset } => issue::Request::TeamIssues {
                team_id,
                preset,
                after,
            }
            .into(),
            ListOf::MyIssues(user_id) => issue::Request::MyIssues { user_id, after }.into(),
            ListOf::ViewIssues(view_id) => issue::Request::ViewIssues { view_id, after }.into(),
            ListOf::ProjectIssues(project_id) => {
                issue::Request::ProjectIssues { project_id, after }.into()
            }
            ListOf::CycleIssues(cycle_id) => issue::Request::CycleIssues { cycle_id, after }.into(),
            ListOf::TeamProjects(team_id) => {
                project::Request::TeamProjects { team_id, after }.into()
            }
            ListOf::ViewProjects(view_id) => {
                project::Request::ViewProjects { view_id, after }.into()
            }
            ListOf::TeamCycles(team_id) => cycle::Request::TeamCycles { team_id, after }.into(),
        }
    }

    /// The page cursor this request continues from, if it asks for a next
    /// page.
    pub fn cursor(&self) -> Option<&str> {
        match self {
            Self::Issue(request) => request.cursor(),
            Self::Project(request) => request.cursor(),
            Self::Cycle(request) => request.cursor(),
            _ => None,
        }
    }
}

/// What opening a list takes.
#[derive(Debug, Clone, PartialEq)]
pub enum Open {
    /// It already holds a first page of what was asked for: nothing to fetch.
    Cached,
    /// It holds the same thing in another slice; its rows stay on screen
    /// until the new first page lands.
    Fetch(Request),
    /// It held something else, and its rows are gone: the cursor belongs at
    /// the top.
    Replace(Request),
}

impl Open {
    /// The request to send, if any.
    pub fn request(self) -> Option<Request> {
        match self {
            Self::Cached => None,
            Self::Fetch(request) | Self::Replace(request) => Some(request),
        }
    }

    /// Whether the list's old rows are gone.
    pub fn replaced(&self) -> bool {
        matches!(self, Self::Replace(_))
    }
}

impl From<Opening> for Open {
    fn from(opening: Opening) -> Self {
        match opening {
            Opening::Cached => Self::Cached,
            Opening::Fetch(ask) => Self::Fetch(Request::page(ask)),
            Opening::Replace(ask) => Self::Replace(Request::page(ask)),
        }
    }
}

/// Why a use case declined to run. The text is shown to the user as is.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    /// "Myself" is not known until Linear answers the startup query.
    #[error("Current user is not loaded yet")]
    ViewerNotLoaded,
    /// An issue needs a title.
    #[error("A title is required")]
    TitleRequired,
    /// The issue has no value for what was to be copied, or there is no issue.
    #[error("No {0} to copy")]
    NothingToCopy(&'static str),
    /// Nothing here has a page on linear.app.
    #[error("Nothing to open here")]
    NothingToOpen,
    /// The action hands work to herdr, which is not running.
    #[error("Jumping to an agent needs herdr")]
    NeedsHerdr,
    /// No herdr agent works on the issue named.
    #[error("No herdr agent is working on {0}")]
    NoAgentOn(String),
    /// There are no notes to send.
    #[error("No notes yet")]
    NoNotes,
}
