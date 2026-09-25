//! Saved views: the list of them, and which a Views page shows.
//!
//! [`ensure_views`] and [`reload_views`] fetch them and [`take_views`] keeps
//! them in Linear's order; [`listed`] says which a Views page lists. A view's
//! contents are an issue list (`issue::open_view_issues`) or a project list
//! (`project::open_view_projects`).

use crate::core::entity::{CustomView, TeamId};
use crate::core::store::Store;

/// What the saved view use cases ask of Linear.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Every saved view the user can open.
    Views,
}

/// **Know the saved views.** Fetched when not yet known.
pub fn ensure_views(store: &Store) -> Option<Request> {
    (!store.views_loaded).then_some(Request::Views)
}

/// **Fetch the saved views again**, for a refresh.
pub fn reload_views(store: &mut Store) -> Request {
    store.views_loaded = false;
    Request::Views
}

/// **The saved views arrive**, personal ones first and then by name, as
/// Linear's Views page lists them.
pub fn take_views(store: &mut Store, views: Vec<CustomView>) {
    store.set_custom_views(views);
}

/// **Which views a Views page lists**, as positions in the saved views.
///
/// As in Linear, the workspace's page (`team` = `None`) holds the views that
/// belong to no team, and each team's page holds the views scoped to it. The
/// page's tab shows either the issue views or the project views.
pub fn listed(views: &[CustomView], team: Option<&TeamId>, issue_views: bool) -> Vec<usize> {
    views
        .iter()
        .enumerate()
        .filter(|(_, v)| v.team.as_ref().map(|t| &t.id) == team)
        .filter(|(_, v)| v.lists_issues() == issue_views)
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(id: &str, name: &str, shared: bool, team: Option<&str>, model: &str) -> CustomView {
        let team = team.map_or("null".to_string(), |t| {
            format!(r#"{{"id":"{t}","name":"{t}","key":"{t}"}}"#)
        });
        serde_json::from_str(&format!(
            r#"{{"id":"{id}","name":"{name}","shared":{shared},"team":{team},"modelName":"{model}"}}"#
        ))
        .unwrap()
    }

    /// The views are fetched once, and again on a refresh.
    #[test]
    fn views_are_fetched_until_known_and_on_refresh() {
        let mut store = Store::default();
        assert_eq!(ensure_views(&store), Some(Request::Views));
        take_views(&mut store, Vec::new());
        assert_eq!(ensure_views(&store), None);
        assert_eq!(reload_views(&mut store), Request::Views);
        assert_eq!(ensure_views(&store), Some(Request::Views));
    }

    /// Personal views come before shared ones, each group by name.
    #[test]
    fn personal_views_come_first_then_by_name() {
        let mut store = Store::default();
        take_views(
            &mut store,
            vec![
                view("1", "b shared", true, None, "Issue"),
                view("2", "Zeta", false, None, "Issue"),
                view("3", "alpha", false, None, "Issue"),
            ],
        );
        let names: Vec<&str> = store.custom_views.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["alpha", "Zeta", "b shared"]);
    }

    /// The workspace page lists views of no team; a team's page, its own;
    /// the tab picks issue or project views.
    #[test]
    fn a_views_page_lists_its_scope_and_kind() {
        let views = vec![
            view("1", "mine", false, None, "Issue"),
            view("2", "team", false, Some("t"), "Issue"),
            view("3", "roadmap", false, None, "Project"),
        ];
        assert_eq!(listed(&views, None, true), [0]);
        assert_eq!(listed(&views, Some(&TeamId::from("t")), true), [1]);
        assert_eq!(listed(&views, None, false), [2]);
    }
}
