//! Projects: a team's, and a saved project view's.
//!
//! [`open_team_projects`] and [`open_view_projects`] list them; [`next_page`],
//! [`reload`], and [`take_page`] follow the list as pages land. A project's
//! issues are an issue list: `issue::open_project_issues`.

use super::{Open, Refusal};
use crate::entity::{CustomViewId, Page, Project, TeamId};
use crate::store::{List, ListOf, Store};

/// What the project use cases ask of Linear, and of the desktop.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// A page of a team's projects.
    TeamProjects {
        team_id: TeamId,
        after: Option<String>,
    },
    /// A page of a saved project view's projects. Linear evaluates the view's
    /// filter.
    ViewProjects {
        view_id: CustomViewId,
        after: Option<String>,
    },
    /// Show the project's page on linear.app in the desktop's browser.
    OpenInBrowser(String),
}

impl Request {
    /// The page cursor this request continues from, if it asks for a next
    /// page.
    pub fn cursor(&self) -> Option<&str> {
        match self {
            Self::TeamProjects { after, .. } | Self::ViewProjects { after, .. } => after.as_deref(),
            Self::OpenInBrowser(_) => None,
        }
    }
}

/// Which list of projects: the team's, or a saved view's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Projects {
    Team,
    View,
}

impl Projects {
    fn list(self) -> List {
        match self {
            Self::Team => List::Projects,
            Self::View => List::ViewProjects,
        }
    }
}

/// **Browse a team's projects.** A list already holding them is not fetched
/// again; another team's projects are dropped first.
pub fn open_team_projects(store: &mut Store, team_id: TeamId) -> Open {
    store
        .open_list(List::Projects, ListOf::TeamProjects(team_id))
        .into()
}

/// **Open a saved project view.** Linear evaluates its filter; another
/// view's projects are dropped first.
pub fn open_view_projects(store: &mut Store, view_id: CustomViewId) -> Open {
    store
        .open_list(List::ViewProjects, ListOf::ViewProjects(view_id))
        .into()
}

/// **Scroll on to the next page** of projects, asked for once.
pub fn next_page(store: &mut Store, projects: Projects) -> Option<super::Request> {
    store.next_page(projects.list()).map(super::Request::page)
}

/// **Refresh a list of projects**: its first page again.
pub fn reload(store: &mut Store, projects: Projects) -> Option<super::Request> {
    store.reload_list(projects.list()).map(super::Request::page)
}

/// **Open a project on linear.app** (`o`). Without a URL there is nothing
/// to open.
pub fn open_url(url: Option<String>) -> Result<Request, Refusal> {
    url.map(Request::OpenInBrowser)
        .ok_or(Refusal::NothingToOpen)
}

/// **A page of projects lands**, and is taken only while the list still
/// belongs to what it was fetched for. Returns whether it was taken.
pub fn take_page(store: &mut Store, projects: Projects, of: &ListOf, page: Page<Project>) -> bool {
    match projects {
        Projects::Team => store.projects.accept_for(of, page),
        Projects::View => store.view_projects.accept_for(of, page),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(id: &str) -> Project {
        serde_json::from_str(&format!(r#"{{"id":"{id}","name":"{id}"}}"#)).unwrap()
    }

    /// A team's projects are asked for once, then shown from what is held.
    #[test]
    fn a_teams_projects_are_fetched_once() {
        let mut store = Store::default();
        assert_eq!(
            open_team_projects(&mut store, TeamId::from("t")).request(),
            Some(crate::usecase::Request::Project(Request::TeamProjects {
                team_id: "t".into(),
                after: None
            }))
        );
        let of = ListOf::TeamProjects(TeamId::from("t"));
        take_page(
            &mut store,
            Projects::Team,
            &of,
            Page::new(vec![project("p")], Default::default(), false),
        );
        assert_eq!(
            open_team_projects(&mut store, TeamId::from("t")),
            Open::Cached
        );
    }

    /// Projects fetched for a team the user has left are dropped.
    #[test]
    fn projects_for_a_team_left_behind_are_dropped() {
        let mut store = Store::default();
        open_team_projects(&mut store, TeamId::from("u"));
        let of = ListOf::TeamProjects(TeamId::from("t"));
        let page = Page::new(vec![project("p")], Default::default(), false);
        assert!(!take_page(&mut store, Projects::Team, &of, page));
    }

    /// A project view asks for its own projects, apart from the team's.
    #[test]
    fn a_project_view_lists_its_own_projects() {
        let mut store = Store::default();
        assert_eq!(
            open_view_projects(&mut store, CustomViewId::from("v")).request(),
            Some(crate::usecase::Request::Project(Request::ViewProjects {
                view_id: "v".into(),
                after: None
            }))
        );
        assert!(store.projects.of.is_none());
        assert_eq!(
            reload(&mut store, Projects::View),
            Some(crate::usecase::Request::Project(Request::ViewProjects {
                view_id: "v".into(),
                after: None
            }))
        );
        assert_eq!(next_page(&mut store, Projects::View), None);
    }
}
