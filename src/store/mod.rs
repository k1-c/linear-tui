//! Everything Linear has told us, and the rules for folding new answers in.
//!
//! The store knows nothing about screens, cursors, or popups: it is the
//! client-side copy of the workspace. `app` decides which answers belong on
//! screen; the store only keeps them consistent — one issue patched
//! everywhere it is held, pages merged without duplicates.

use std::collections::{HashMap, HashSet};

use crate::api::ids::{CustomViewId, IssueId, TeamId, UserId};
use crate::api::types::{
    CustomView, Cycle, Favorite, Issue, Organization, PageInfo, Project, Team, User, WorkflowState,
};
use crate::message::Page;

#[cfg(test)]
mod tests;

/// Which of the issue lists a value belongs to.
///
/// Five lists behave identically once you know which one is meant —
/// selection, prefetch, grouping, opening a row — so they are addressed
/// through this rather than duplicated five times over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSource {
    Team,
    My,
    View,
    Project,
    Cycle,
}

impl IssueSource {
    pub const ALL: [IssueSource; 5] =
        [Self::Team, Self::My, Self::View, Self::Project, Self::Cycle];
}

/// One `T` per [`IssueSource`].
#[derive(Debug, Default)]
pub struct PerSource<T>([T; 5]);

impl<T> PerSource<T> {
    pub fn from_fn(f: impl FnMut(IssueSource) -> T) -> Self {
        Self(IssueSource::ALL.map(f))
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut()
    }
}

impl<T> std::ops::Index<IssueSource> for PerSource<T> {
    type Output = T;
    fn index(&self, source: IssueSource) -> &T {
        &self.0[source as usize]
    }
}

impl<T> std::ops::IndexMut<IssueSource> for PerSource<T> {
    fn index_mut(&mut self, source: IssueSource) -> &mut T {
        &mut self.0[source as usize]
    }
}

/// A paginated list as fetched so far.
#[derive(Debug)]
pub struct Rows<T> {
    /// In the order Linear returned them.
    pub items: Vec<T>,
    pub page_info: PageInfo,
    /// Whether a first page has arrived for what the list currently belongs to.
    pub loaded: bool,
}

impl<T> Default for Rows<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            page_info: PageInfo::default(),
            loaded: false,
        }
    }
}

impl<T> Rows<T> {
    /// Fold one page in, either replacing the rows or extending them.
    pub fn accept(&mut self, page: Page<T>) {
        let Page {
            mut items,
            page_info,
            append,
        } = page;
        if append {
            self.items.append(&mut items);
        } else {
            self.items = items;
        }
        self.page_info = page_info;
        self.loaded = true;
    }

    /// Forget the rows, for a list that is about to belong to something else.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

impl Rows<Issue> {
    /// [`Self::accept`], dropping any issue already held.
    ///
    /// Pages are ordered by `updatedAt`, so an issue touched between two page
    /// fetches moves and can come back on both; without this it would be
    /// listed twice.
    pub fn accept_issues(&mut self, page: Page<Issue>) {
        let append = page.append;
        self.accept(page);
        if append {
            let mut seen = HashSet::new();
            self.items.retain(|issue| seen.insert(issue.id.clone()));
        }
    }
}

/// One team's workflow states and members — what its issues can be moved to
/// and assigned to.
#[derive(Debug, Clone, Default)]
pub struct TeamContext {
    pub states: Vec<WorkflowState>,
    pub members: Vec<User>,
}

#[derive(Debug, Default)]
pub struct Store {
    pub teams: Vec<Team>,
    /// States and members of every team loaded so far. A status or assignee
    /// must come from the issue's own team, and My Issues, views, projects
    /// and cycles can hold issues of teams other than the selected one.
    pub team_contexts: HashMap<TeamId, TeamContext>,
    pub viewer_id: Option<UserId>,
    /// The workspace the credentials act in, as the viewer query named it.
    pub organization: Option<Organization>,
    /// Saved views, personal ones first.
    pub custom_views: Vec<CustomView>,
    pub views_loaded: bool,
    /// Favorites, in the order Linear's sidebar shows them.
    pub favorites: Vec<Favorite>,
    pub favorites_loaded: bool,
    pub issues: PerSource<Rows<Issue>>,
    /// Which view `issues[View]` belongs to, so switching views refetches.
    pub loaded_view_id: Option<CustomViewId>,
    /// The selected team's projects.
    pub projects: Rows<Project>,
    /// The selected team's cycles.
    pub cycles: Rows<Cycle>,
    /// A saved project view's projects.
    pub view_projects: Rows<Project>,
    pub loaded_view_projects_id: Option<CustomViewId>,
    /// The issue open in the detail view, with its comment thread.
    pub current_issue: Option<Issue>,
}

impl Store {
    /// Apply `f` to every copy of the issue we hold, so the UI updates
    /// without a refetch.
    pub fn patch_issue(&mut self, issue_id: &IssueId, f: impl Fn(&mut Issue)) {
        for rows in self.issues.iter_mut() {
            for issue in rows.items.iter_mut().filter(|i| &i.id == issue_id) {
                f(issue);
            }
        }
        if let Some(current) = &mut self.current_issue
            && &current.id == issue_id
        {
            f(current);
        }
    }

    /// Take a freshly fetched issue as the truth for every copy we hold.
    ///
    /// List copies keep their own `comments`: whether a copy has its thread
    /// loaded is what decides that opening it fetches the detail.
    pub fn refresh_issue(&mut self, fresh: Issue) {
        let id = fresh.id.clone();
        self.patch_issue(&id, |issue| {
            let comments = issue.comments.take();
            *issue = fresh.clone();
            issue.comments = comments;
        });
        if let Some(current) = &mut self.current_issue
            && current.id == id
        {
            *current = fresh;
        }
    }

    /// Any copy of an issue we hold.
    pub fn issue(&self, id: &IssueId) -> Option<&Issue> {
        self.current_issue
            .iter()
            .chain(self.issues.iter().flat_map(|rows| &rows.items))
            .find(|issue| &issue.id == id)
    }

    /// Replace the saved views, personal ones first as Linear's Views page
    /// lists them.
    pub fn set_custom_views(&mut self, mut views: Vec<CustomView>) {
        views.sort_by(|a, b| {
            a.shared
                .cmp(&b.shared)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        self.custom_views = views;
        self.views_loaded = true;
    }

    pub fn set_favorites(&mut self, mut favorites: Vec<Favorite>) {
        favorites.sort_by(|a, b| a.sort_order.total_cmp(&b.sort_order));
        self.favorites = favorites;
        self.favorites_loaded = true;
    }

    /// The team `issue` belongs to; `fallback` for an issue that does not say.
    pub fn issue_team_id<'a>(
        &'a self,
        issue: &'a Issue,
        fallback: Option<&'a TeamId>,
    ) -> Option<&'a TeamId> {
        issue.team.as_ref().map(|team| &team.id).or(fallback)
    }

    /// The current user, as a member of any team loaded so far. The viewer is
    /// the same user in every team they belong to.
    pub fn viewer(&self) -> Option<&User> {
        let id = self.viewer_id.as_ref()?;
        self.team_contexts
            .values()
            .flat_map(|c| &c.members)
            .find(|u| &u.id == id)
    }
}
