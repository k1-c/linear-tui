//! Keeping the paginated lists consistent: what each belongs to, which page
//! to ask for next, and which pages to take.
//!
//! This is mechanism, not a use case. The use cases that open a list — a
//! team's issues, a project's, a team's cycles — decide *what* to open and
//! turn a [`PageAsk`] into a request; these rules are the same for all of
//! them.

use super::{Issue, List, ListOf, Page, Rows, Store};

/// A page to ask Linear for: of which list, after which cursor (`None` for
/// the first page).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageAsk {
    pub of: ListOf,
    pub after: Option<String>,
}

impl PageAsk {
    fn first(of: ListOf) -> Self {
        Self { of, after: None }
    }
}

/// What opening a list takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opening {
    /// It already holds a first page of what was asked for.
    Cached,
    /// It holds the same thing in another slice (a team's Backlog after its
    /// Active issues). The rows stay until the new first page lands, so the
    /// screen does not flash empty.
    Fetch(PageAsk),
    /// It held something else. The rows are gone, so nothing is shown under
    /// the wrong name.
    Replace(PageAsk),
}

impl Store {
    /// Point `list` at `of`: see [`Opening`] for the three outcomes.
    pub fn open_list(&mut self, list: List, of: ListOf) -> Opening {
        let rows = self.listing_mut(list);
        let current = rows.of().cloned();
        if current.as_ref() == Some(&of) && rows.loaded() {
            return Opening::Cached;
        }
        if current.as_ref().is_some_and(|c| c.same_owner(&of)) {
            rows.set_loaded(false);
            rows.set_of(of.clone());
            Opening::Fetch(PageAsk::first(of))
        } else {
            rows.reset();
            rows.set_of(of.clone());
            Opening::Replace(PageAsk::first(of))
        }
    }

    /// Ask for `list`'s first page again, keeping its rows until it lands.
    /// A list that belongs to nothing yet has nothing to ask for.
    pub fn reload_list(&mut self, list: List) -> Option<PageAsk> {
        let rows = self.listing_mut(list);
        let of = rows.of()?.clone();
        rows.set_loaded(false);
        Some(PageAsk::first(of))
    }

    /// `list`'s next page, when there is one and it has not been asked for.
    /// A page still on its way is not asked for again, however often the
    /// cursor passes near the bottom meanwhile.
    pub fn next_page(&mut self, list: List) -> Option<PageAsk> {
        let rows = self.listing(list);
        let info = rows.page_info();
        if !info.has_next_page {
            return None;
        }
        let cursor = info.end_cursor.clone()?;
        let of = rows.of()?.clone();
        self.requested
            .cursors
            .insert(cursor.clone())
            .then_some(PageAsk {
                of,
                after: Some(cursor),
            })
    }

    /// A page did not arrive; it may be asked for again.
    pub fn page_failed(&mut self, cursor: &str) {
        self.requested.cursors.remove(cursor);
    }

    /// Stop remembering which pages were asked for, so each can be asked for
    /// again — for a refresh.
    pub fn forget_pages(&mut self) {
        self.requested.cursors.clear();
    }
}

impl<T> Rows<T> {
    /// Fold in a page fetched for `of`, when the rows still belong to it: a
    /// page that lands after the list moved on is dropped. Returns whether
    /// it was taken.
    pub fn accept_for(&mut self, of: &ListOf, page: Page<T>) -> bool {
        if self.of.as_ref() != Some(of) {
            return false;
        }
        self.accept(page);
        true
    }
}

impl Rows<Issue> {
    /// [`Rows::accept_for`], dropping any issue already held (see
    /// [`Rows::accept_issues`]).
    pub fn accept_issues_for(&mut self, of: &ListOf, page: Page<Issue>) -> bool {
        if self.of.as_ref() != Some(of) {
            return false;
        }
        self.accept_issues(page);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::entity::{PageInfo, Preset, Project, TeamId};
    use crate::core::store::IssueSource;

    fn team_projects(team: &str) -> ListOf {
        ListOf::TeamProjects(TeamId::from(team))
    }

    fn project(id: &str) -> Project {
        serde_json::from_str(&format!(r#"{{"id":"{id}","name":"{id}"}}"#)).unwrap()
    }

    fn page(more: Option<&str>) -> Page<Project> {
        let info = PageInfo {
            has_next_page: more.is_some(),
            end_cursor: more.map(Into::into),
        };
        Page::new(vec![project("p")], info, false)
    }

    /// A list opened for the first time asks for its first page.
    #[test]
    fn a_new_list_asks_for_its_first_page() {
        let mut store = Store::default();
        assert_eq!(
            store.open_list(List::Projects, team_projects("t")),
            Opening::Replace(PageAsk::first(team_projects("t")))
        );
    }

    /// A list that holds a first page of what is asked for asks for nothing.
    #[test]
    fn a_loaded_list_asks_for_nothing() {
        let mut store = Store::default();
        store.open_list(List::Projects, team_projects("t"));
        store.projects.accept_for(&team_projects("t"), page(None));
        assert_eq!(
            store.open_list(List::Projects, team_projects("t")),
            Opening::Cached
        );
    }

    /// Opening a list on something else drops the old rows.
    #[test]
    fn a_list_of_something_else_drops_the_old_rows() {
        let mut store = Store::default();
        store.open_list(List::Projects, team_projects("t"));
        store.projects.accept_for(&team_projects("t"), page(None));
        assert!(matches!(
            store.open_list(List::Projects, team_projects("u")),
            Opening::Replace(_)
        ));
        assert!(store.projects.items.is_empty());
    }

    /// Another slice of the same team's issues keeps the rows until its page
    /// lands.
    #[test]
    fn another_slice_keeps_the_rows_until_its_page_lands() {
        let mut store = Store::default();
        let slice = |preset| ListOf::TeamIssues {
            team_id: TeamId::from("t"),
            preset,
        };
        let list = List::Issues(IssueSource::Team);
        store.open_list(list, slice(Preset::Active));
        store.issues[IssueSource::Team].items =
            vec![serde_json::from_str(r#"{"id":"i","identifier":"T-1","title":"t"}"#).unwrap()];
        assert!(matches!(
            store.open_list(list, slice(Preset::Backlog)),
            Opening::Fetch(_)
        ));
        assert_eq!(store.issues[IssueSource::Team].items.len(), 1);
    }

    /// The next page is asked for once while it is on its way.
    #[test]
    fn the_next_page_is_asked_for_once() {
        let mut store = Store::default();
        store.open_list(List::Projects, team_projects("t"));
        store
            .projects
            .accept_for(&team_projects("t"), page(Some("c1")));
        assert_eq!(
            store.next_page(List::Projects),
            Some(PageAsk {
                of: team_projects("t"),
                after: Some("c1".into())
            })
        );
        assert_eq!(store.next_page(List::Projects), None);
    }

    /// A page that failed can be asked for again.
    #[test]
    fn a_failed_page_can_be_asked_for_again() {
        let mut store = Store::default();
        store.open_list(List::Projects, team_projects("t"));
        store
            .projects
            .accept_for(&team_projects("t"), page(Some("c1")));
        store.next_page(List::Projects);
        store.page_failed("c1");
        assert!(store.next_page(List::Projects).is_some());
    }

    /// The last page has no next page.
    #[test]
    fn the_last_page_has_no_next() {
        let mut store = Store::default();
        store.open_list(List::Projects, team_projects("t"));
        store.projects.accept_for(&team_projects("t"), page(None));
        assert_eq!(store.next_page(List::Projects), None);
    }

    /// A page for what the list no longer belongs to is dropped.
    #[test]
    fn a_page_for_a_list_left_behind_is_dropped() {
        let mut store = Store::default();
        store.open_list(List::Projects, team_projects("u"));
        assert!(!store.projects.accept_for(&team_projects("t"), page(None)));
        assert!(store.projects.items.is_empty());
    }

    /// Reloading asks for the first page again and keeps the rows meanwhile.
    #[test]
    fn reloading_asks_for_the_first_page_again() {
        let mut store = Store::default();
        store.open_list(List::Projects, team_projects("t"));
        store.projects.accept_for(&team_projects("t"), page(None));
        assert_eq!(
            store.reload_list(List::Projects),
            Some(PageAsk::first(team_projects("t")))
        );
        assert!(!store.projects.loaded);
        assert_eq!(store.projects.items.len(), 1);
    }
}
