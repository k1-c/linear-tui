//! Favorites: Linear's own sidebar, and where each entry leads.
//!
//! [`load`] asks for them and [`take_favorites`] keeps them in Linear's
//! order; [`target`] says what opening one opens.

use crate::core::entity::{
    CustomView, CustomViewId, Cycle, Favorite, IssueRef, Project, Team, TeamId,
};
use crate::core::store::Store;

/// What the favorite use cases ask of Linear.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// The user's favorites.
    Favorites,
    /// Show a favorite linear-tui has no page for on linear.app.
    OpenInBrowser(String),
}

/// A page of a team a favorite can point at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamPage {
    Issues,
    Cycles,
    Projects,
}

/// What opening a favorite opens. Two targets are the same when they lead
/// to the same place: a project, cycle, or issue is compared by its id.
#[derive(Debug, Clone)]
pub enum Target {
    /// A folder of favorites: it folds and unfolds.
    Folder,
    /// A saved view the user can open here.
    View(CustomViewId),
    /// Linear's "My issues".
    MyIssues,
    /// A page of one of the user's teams.
    TeamPage(TeamId, TeamPage),
    Project(Project),
    Cycle(Cycle),
    Issue(IssueRef),
    /// Something linear-tui has no page for — a document, a label — on
    /// linear.app.
    Browser(String),
    /// Something with neither a page here nor a URL.
    Nowhere,
}

impl PartialEq for Target {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Folder, Self::Folder)
            | (Self::MyIssues, Self::MyIssues)
            | (Self::Nowhere, Self::Nowhere) => true,
            (Self::View(a), Self::View(b)) => a == b,
            (Self::TeamPage(a, p), Self::TeamPage(b, q)) => a == b && p == q,
            (Self::Project(a), Self::Project(b)) => a.id == b.id,
            (Self::Cycle(a), Self::Cycle(b)) => a.id == b.id,
            (Self::Issue(a), Self::Issue(b)) => a.id == b.id,
            (Self::Browser(a), Self::Browser(b)) => a == b,
            _ => false,
        }
    }
}

/// **Know my favorites.** Asked for as linear-tui starts: the sidebar shows
/// them whatever page is open.
pub fn load() -> Request {
    Request::Favorites
}

/// **The favorites arrive**, in the order Linear's sidebar shows them.
pub fn take_favorites(store: &mut Store, favorites: Vec<Favorite>) {
    store.set_favorites(favorites);
}

/// **Where a favorite leads.**
///
/// A folder folds. A saved view opens as the view, when it is one the user
/// can see here. Linear's predefined pages open as the page: "My issues",
/// and a team's issues (whichever slice the favorite names), cycles, or
/// projects — when that team is one of the user's. A project, cycle, or
/// issue opens in place. Anything else opens on linear.app, and a favorite
/// without even a URL leads nowhere.
pub fn target(favorite: &Favorite, views: &[CustomView], teams: &[Team]) -> Target {
    if favorite.is_folder() {
        return Target::Folder;
    }
    if let Some(view) = &favorite.custom_view
        && views.iter().any(|v| v.id == view.id)
    {
        return Target::View(view.id.clone());
    }
    if favorite.kind == "predefinedView" {
        if favorite.predefined_view_type.as_deref() == Some("myIssues") {
            return Target::MyIssues;
        }
        let team = favorite
            .predefined_view_team
            .as_ref()
            .filter(|t| teams.iter().any(|x| x.id == t.id));
        let page = match favorite.predefined_view_type.as_deref() {
            Some("issues" | "allIssues" | "activeIssues" | "backlog") => Some(TeamPage::Issues),
            Some("cycles") => Some(TeamPage::Cycles),
            Some("projects") => Some(TeamPage::Projects),
            _ => None,
        };
        if let (Some(team), Some(page)) = (team, page) {
            return Target::TeamPage(team.id.clone(), page);
        }
    }
    if let Some(project) = &favorite.project {
        return Target::Project(project.clone());
    }
    if let Some(cycle) = &favorite.cycle {
        return Target::Cycle(cycle.clone());
    }
    if let Some(issue) = &favorite.issue {
        return Target::Issue(issue.clone());
    }
    match &favorite.url {
        Some(url) => Target::Browser(url.clone()),
        None => Target::Nowhere,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fav(json: &str) -> Favorite {
        serde_json::from_str(json).unwrap()
    }

    /// The favorites are asked of Linear.
    #[test]
    fn the_favorites_are_asked_for() {
        assert_eq!(load(), Request::Favorites);
    }

    fn views() -> Vec<CustomView> {
        vec![serde_json::from_str(r#"{"id":"v1","name":"Mine"}"#).unwrap()]
    }

    fn teams() -> Vec<Team> {
        vec![serde_json::from_str(r#"{"id":"t1","name":"Core","key":"COR"}"#).unwrap()]
    }

    fn target_of(json: &str) -> Target {
        target(&fav(json), &views(), &teams())
    }

    /// The favorites are kept in the order of Linear's sidebar.
    #[test]
    fn favorites_keep_linears_order() {
        let mut store = Store::default();
        take_favorites(
            &mut store,
            vec![
                fav(r#"{"id":"b","type":"issue","sortOrder":2}"#),
                fav(r#"{"id":"a","type":"issue","sortOrder":1}"#),
            ],
        );
        let ids: Vec<&str> = store.favorites.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
    }

    /// A folder folds.
    #[test]
    fn a_folder_folds() {
        assert_eq!(
            target_of(r#"{"id":"f","type":"folder","folderName":"Ops"}"#),
            Target::Folder
        );
    }

    /// A saved view the user can see opens as the view; one they cannot
    /// falls through to its URL.
    #[test]
    fn a_view_favorite_opens_the_view() {
        assert_eq!(
            target_of(r#"{"id":"f","type":"customView","customView":{"id":"v1"}}"#),
            Target::View("v1".into())
        );
        assert_eq!(
            target_of(
                r#"{"id":"f","type":"customView","customView":{"id":"v9"},"url":"https://x"}"#
            ),
            Target::Browser("https://x".into())
        );
    }

    /// Linear's predefined pages open as the page, for the user's teams.
    #[test]
    fn predefined_pages_open_as_the_page() {
        assert_eq!(
            target_of(r#"{"id":"f","type":"predefinedView","predefinedViewType":"myIssues"}"#),
            Target::MyIssues
        );
        assert_eq!(
            target_of(
                r#"{"id":"f","type":"predefinedView","predefinedViewType":"backlog",
                    "predefinedViewTeam":{"id":"t1"}}"#
            ),
            Target::TeamPage("t1".into(), TeamPage::Issues)
        );
        assert_eq!(
            target_of(
                r#"{"id":"f","type":"predefinedView","predefinedViewType":"cycles",
                    "predefinedViewTeam":{"id":"t9"}}"#
            ),
            Target::Nowhere
        );
    }

    /// A project, cycle, or issue opens in place.
    #[test]
    fn a_project_cycle_or_issue_opens_in_place() {
        assert!(matches!(
            target_of(r#"{"id":"f","type":"project","project":{"id":"p","name":"P"}}"#),
            Target::Project(p) if p.name == "P"
        ));
        assert!(matches!(
            target_of(r#"{"id":"f","type":"cycle","cycle":{"id":"c"}}"#),
            Target::Cycle(_)
        ));
        assert!(matches!(
            target_of(
                r#"{"id":"f","type":"issue","issue":{"id":"i","identifier":"COR-1","title":"t"}}"#
            ),
            Target::Issue(i) if i.identifier == "COR-1"
        ));
    }

    /// Anything else opens on linear.app, or nowhere without a URL.
    #[test]
    fn anything_else_opens_in_the_browser() {
        assert_eq!(
            target_of(r#"{"id":"f","type":"document","url":"https://linear.app/doc"}"#),
            Target::Browser("https://linear.app/doc".into())
        );
        assert_eq!(
            target_of(r#"{"id":"f","type":"document"}"#),
            Target::Nowhere
        );
    }
}
