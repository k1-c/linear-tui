//! Everything Linear has told us, and the rules for folding new answers in.
//!
//! The store knows nothing about screens, cursors, or popups: it is the
//! client-side copy of the workspace. `app` decides which answers belong on
//! screen; the store only keeps them consistent — one issue patched
//! everywhere it is held, pages merged without duplicates.

use std::collections::{HashMap, HashSet};

use crate::entity::Page;
use crate::entity::{
    CustomView, Cycle, Favorite, Issue, PageInfo, Project, Team, User, WorkflowState,
};
use crate::entity::{CustomViewId, CycleId, IssueId, Preset, ProjectId, TeamId, UserId};

mod lists;

pub use lists::{Opening, PageAsk};
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

/// What a paginated list is a list of. A page fetched for anything else is
/// not folded into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListOf {
    /// A team's issues, in one of Linear's slices.
    TeamIssues {
        team_id: TeamId,
        preset: Preset,
    },
    /// The issues assigned to a user.
    MyIssues(UserId),
    /// A saved issue view's issues.
    ViewIssues(CustomViewId),
    ProjectIssues(ProjectId),
    CycleIssues(CycleId),
    TeamProjects(TeamId),
    /// A saved project view's projects.
    ViewProjects(CustomViewId),
    TeamCycles(TeamId),
}

impl ListOf {
    /// Whether `other` lists the same thing, if perhaps another slice of it:
    /// a team's Backlog after its Active issues.
    pub fn same_owner(&self, other: &ListOf) -> bool {
        match (self, other) {
            (Self::TeamIssues { team_id: a, .. }, Self::TeamIssues { team_id: b, .. }) => a == b,
            _ => self == other,
        }
    }
}

/// One of the paginated lists the store holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum List {
    Issues(IssueSource),
    /// The selected team's projects.
    Projects,
    /// A saved project view's projects.
    ViewProjects,
    /// The selected team's cycles.
    Cycles,
}

/// A paginated list as fetched so far.
#[derive(Debug)]
pub struct Rows<T> {
    /// In the order Linear returned them.
    pub items: Vec<T>,
    pub page_info: PageInfo,
    /// Whether a first page has arrived for what the list belongs to.
    pub loaded: bool,
    /// What the list belongs to, once its first page has been asked for.
    pub of: Option<ListOf>,
}

impl<T> Default for Rows<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            page_info: PageInfo::default(),
            loaded: false,
            of: None,
        }
    }
}

/// What every paginated list has, whatever it lists.
pub trait Listing {
    fn of(&self) -> Option<&ListOf>;
    fn set_of(&mut self, of: ListOf);
    fn loaded(&self) -> bool;
    fn set_loaded(&mut self, loaded: bool);
    fn page_info(&self) -> &PageInfo;
    /// Forget the rows and what they belonged to.
    fn reset(&mut self);
}

impl<T> Listing for Rows<T> {
    fn of(&self) -> Option<&ListOf> {
        self.of.as_ref()
    }
    fn set_of(&mut self, of: ListOf) {
        self.of = Some(of);
    }
    fn loaded(&self) -> bool {
        self.loaded
    }
    fn set_loaded(&mut self, loaded: bool) {
        self.loaded = loaded;
    }
    fn page_info(&self) -> &PageInfo {
        &self.page_info
    }
    fn reset(&mut self) {
        Rows::reset(self);
    }
}

/// What has been asked of Linear and not answered yet, so it is asked once.
///
/// Not something Linear told us, but kept beside what it told us: the use
/// cases that ask decide by it, and a failure clears it so the ask can be
/// made again.
#[derive(Debug, Default)]
pub struct Requested {
    /// Page cursors already asked for. A next-page request stays in flight
    /// while the user keeps scrolling; each step near the bottom would
    /// otherwise ask for the same page again, appending it once per step.
    pub cursors: HashSet<String>,
    /// Teams whose states and members are on their way.
    pub team_contexts: HashSet<TeamId>,
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
    /// Saved views, personal ones first.
    pub custom_views: Vec<CustomView>,
    pub views_loaded: bool,
    /// Favorites, in the order Linear's sidebar shows them.
    pub favorites: Vec<Favorite>,
    pub favorites_loaded: bool,
    pub issues: PerSource<Rows<Issue>>,
    /// The selected team's projects.
    pub projects: Rows<Project>,
    /// The selected team's cycles.
    pub cycles: Rows<Cycle>,
    /// A saved project view's projects.
    pub view_projects: Rows<Project>,
    /// The issue open in the detail view, with its comment thread.
    pub current_issue: Option<Issue>,
    pub requested: Requested,
}

impl Store {
    /// One of the paginated lists, whatever it lists.
    pub fn listing(&self, list: List) -> &dyn Listing {
        match list {
            List::Issues(source) => &self.issues[source],
            List::Projects => &self.projects,
            List::ViewProjects => &self.view_projects,
            List::Cycles => &self.cycles,
        }
    }

    pub fn listing_mut(&mut self, list: List) -> &mut dyn Listing {
        match list {
            List::Issues(source) => &mut self.issues[source],
            List::Projects => &mut self.projects,
            List::ViewProjects => &mut self.view_projects,
            List::Cycles => &mut self.cycles,
        }
    }

    /// Whether a page fetched for `of` belongs in `list`: only while the list
    /// still belongs to exactly that. A page that lands after the user moved
    /// on — to another team, view, project, cycle, or preset — is dropped.
    pub fn wants_page(&self, list: List, of: &ListOf) -> bool {
        self.listing(list).of() == Some(of)
    }

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
