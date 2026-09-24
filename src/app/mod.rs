use std::collections::{HashSet, VecDeque};

use crate::api::ids::*;
use crate::api::types::*;
use crate::config::{Config, Theme};
use crate::grouping::{GroupBy, Preset, Section, group};
use crate::message::{Message, Page, Request};
pub use crate::store::{IssueSource, PerSource, Store, TeamContext};

mod frame;
mod input;
mod lists;
mod mouse;
mod sidebar;
#[cfg(test)]
mod tests;

pub use frame::*;
pub use input::*;
pub use lists::*;
pub use sidebar::*;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// How close to the bottom of the list the cursor must get before the next page
/// is requested.
const PREFETCH_MARGIN: usize = 5;

/// A destination in the sidebar — everything the content pane can be showing.
///
/// Linear's navigation is a tree of places, not a strip of four tabs, and the
/// team a place belongs to is part of the place: `Platform > Cycles` and
/// `Development > Cycles` are different destinations. Carrying the team index
/// here is what lets the sidebar jump between teams without a modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    MyIssues,
    /// The index of saved views.
    Views,
    /// One saved view, by its position in `custom_views`.
    View(usize),
    Team(usize, TeamSection),
    /// A favorite opened in place — a project, cycle, or issue — by its
    /// position in `favorites`. Favorites that point at a view or a team page
    /// use that destination instead, so the sidebar highlights one row.
    Favorite(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamSection {
    Issues,
    Cycles,
    Projects,
    /// Saved views scoped to the team.
    Views,
}

/// What a saved view lists — Linear's Issues / Projects tabs on a Views page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewKind {
    #[default]
    Issues,
    Projects,
}

impl ViewKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Issues => "Issues",
            Self::Projects => "Projects",
        }
    }

    fn of(view: &CustomView) -> Self {
        if view.lists_issues() {
            Self::Issues
        } else {
            Self::Projects
        }
    }
}

/// A clickable chip in a toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chip {
    Preset(Preset),
    ViewKind(ViewKind),
}

impl Nav {
    /// The screen this destination opens.
    pub fn screen(&self) -> Screen {
        match self {
            // A project view opens a project list instead; see `App::screen_for`.
            Self::MyIssues | Self::View(_) | Self::Team(_, TeamSection::Issues) => {
                Screen::IssueList
            }
            Self::Views | Self::Team(_, TeamSection::Views) => Screen::ViewList,
            Self::Team(_, TeamSection::Cycles) => Screen::CycleList,
            Self::Team(_, TeamSection::Projects) => Screen::ProjectList,
            // Depends on what the favorite is; `App::activate` opens it.
            Self::Favorite(_) => Screen::ProjectDetail,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Screen {
    IssueList,
    IssueDetail,
    ProjectList,
    ProjectDetail,
    CycleList,
    CycleDetail,
    /// The index of saved views.
    ViewList,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
    Comment,
    NewIssue,
}

/// The open popup, and what it acts on.
///
/// The change popups hold the issue they were opened on by id: the cursor
/// can move under an open popup when a page lands, and Enter must change the
/// issue the user picked, not whichever one is under the cursor by then.
#[derive(Debug, Clone, PartialEq)]
pub enum Popup {
    None,
    TeamSelect,
    /// The filter popup asks for a status, then a priority.
    Filter(FilterKind),
    StatusChange(IssueId),
    PriorityChange(IssueId),
    AssigneeChange(IssueId),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterKind {
    Status,
    Priority,
}

/// Identifies one of the paginated sub-lists, for index clamping.
#[derive(Debug, Clone, Copy)]
enum Field {
    Projects,
    Cycles,
}

pub struct App {
    pub screen: Screen,
    pub input_mode: InputMode,
    pub should_quit: bool,

    /// Requests queued by the UI, drained and spawned by the main loop.
    pub requests: VecDeque<Request>,
    /// Number of requests currently in flight.
    pub inflight: usize,
    /// Page cursors already requested. A next-page request stays in flight
    /// while the user keeps scrolling, and each step near the bottom would
    /// otherwise ask for the same page again — appending it once per step.
    prefetched: HashSet<String>,

    /// Where the content pane currently is.
    pub nav: Nav,

    /// What the last frame drew, for hit-testing and page-sized moves.
    pub frame: FrameState,

    /// Everything Linear has told us.
    pub store: Store,
    /// How each of the five issue lists is shaped on screen.
    pub lists: IssueLists,

    // Popup
    pub popup: Popup,
    pub popup_index: usize,

    // Teams
    pub selected_team_index: usize,
    /// Teams whose context is in flight, so each one is asked for once.
    team_contexts_pending: HashSet<TeamId>,

    // Saved views
    pub selected_view_index: usize,
    /// Which tab the Views pages show.
    pub view_kind: ViewKind,
    pub selected_view_project_index: usize,

    // Sidebar
    pub sidebar_visible: bool,
    pub sidebar_width: u16,
    /// True while the cursor is in the sidebar rather than the content pane.
    pub sidebar_focus: bool,
    pub sidebar_index: usize,
    /// Favorites folders the user has folded, by favorite id.
    pub collapsed_folders: HashSet<FavoriteId>,

    // List shaping
    pub group_by: GroupBy,
    /// Group keys the user has folded away.
    pub collapsed_groups: HashSet<String>,

    // Issue detail
    /// Screen to return to when the detail view is closed.
    pub detail_return: Screen,
    /// Destination to return to with it — an issue opened from Favorites
    /// points the sidebar at the favorite while it is open.
    pub detail_return_nav: Nav,
    pub detail_scroll: u16,

    // Search
    /// Set while the list shows workspace-wide search results instead of the team's issues.
    pub global_search: Option<String>,

    // Comment input
    pub comment: Input,

    /// Draft issue, present only while the create form is open.
    pub new_issue: Option<NewIssueForm>,

    /// Text the main loop should push to the system clipboard via OSC 52.
    pub pending_clipboard: Option<String>,

    // Projects
    pub selected_project_index: usize,
    pub current_project: Option<Project>,

    // Cycles
    pub selected_cycle_index: usize,
    pub current_cycle: Option<Cycle>,

    // Status
    pub status_message: Option<String>,

    // Spinner
    pub spinner_frame: usize,

    // Error popup
    pub error_popup: Option<String>,

    /// First key of a pending multi-key chord (Linear's `g …` sequences).
    pub pending_chord: Option<char>,

    // Help
    pub show_help: bool,
    pub help_scroll: u16,

    // Settings
    pub theme: Theme,
    pub items_per_page: u32,
    default_team: Option<String>,
}

impl App {
    pub fn new(config: &Config) -> Self {
        let mut app = Self {
            screen: Screen::IssueList,
            input_mode: InputMode::Normal,
            should_quit: false,
            requests: VecDeque::new(),
            inflight: 0,
            prefetched: HashSet::new(),
            nav: Nav::Team(0, TeamSection::Issues),
            frame: FrameState::default(),
            store: Store::default(),
            lists: PerSource::from_fn(IssueList::new),
            popup: Popup::None,
            popup_index: 0,
            selected_team_index: 0,
            team_contexts_pending: HashSet::new(),
            selected_view_index: 0,
            view_kind: ViewKind::default(),
            selected_view_project_index: 0,
            sidebar_visible: config.ui.sidebar,
            sidebar_width: config.ui.sidebar_width,
            sidebar_focus: false,
            sidebar_index: 0,
            collapsed_folders: HashSet::new(),
            group_by: GroupBy::from_config(config.ui.group_by),
            collapsed_groups: HashSet::new(),
            detail_return: Screen::IssueList,
            detail_return_nav: Nav::Team(0, TeamSection::Issues),
            detail_scroll: 0,
            global_search: None,
            comment: Input::default(),
            new_issue: None,
            pending_clipboard: None,
            selected_project_index: 0,
            current_project: None,
            selected_cycle_index: 0,
            current_cycle: None,
            status_message: None,
            spinner_frame: 0,
            error_popup: None,
            pending_chord: None,
            show_help: false,
            help_scroll: 0,
            theme: Theme::from_name(config.ui.theme),
            items_per_page: config.ui.items_per_page,
            default_team: config.ui.default_team.clone(),
        };
        // The config loaded, but not as written. The popup goes on any key,
        // and unlike the status line it outlasts the first page arriving.
        if !config.warnings.is_empty() {
            app.set_error(format!("config.toml:\n{}", config.warnings.join("\n")));
        }
        app.request(Request::Teams);
        app.request(Request::Viewer);
        app.request(Request::CustomViews);
        app.request(Request::Favorites);
        app
    }

    // ---------------------------------------------------------------- requests

    pub fn request(&mut self, req: Request) {
        // Collapse duplicates so a held-down key can't pile up identical fetches.
        if self.requests.contains(&req) {
            return;
        }
        self.requests.push_back(req);
    }

    pub fn loading(&self) -> bool {
        self.inflight > 0
    }

    pub fn team_id(&self) -> Option<TeamId> {
        self.current_team().map(|t| t.id.clone())
    }

    /// Queue the fetch that populates the current destination.
    pub fn reload_current_tab(&mut self) {
        match self.nav {
            Nav::MyIssues => {
                if let Some(user_id) = self.store.viewer_id.clone() {
                    self.request(Request::MyIssues {
                        user_id,
                        after: None,
                    });
                }
            }
            Nav::Views | Nav::Team(_, TeamSection::Views) => {
                if !self.store.views_loaded {
                    self.request(Request::CustomViews);
                }
            }
            Nav::View(index) => {
                if let Some(view) = self.store.custom_views.get(index) {
                    let view_id = view.id.clone();
                    self.request(match ViewKind::of(view) {
                        ViewKind::Issues => Request::ViewIssues {
                            view_id,
                            after: None,
                        },
                        ViewKind::Projects => Request::ViewProjects {
                            view_id,
                            after: None,
                        },
                    });
                }
            }
            // A favorite's page reloads through `queue_detail_fetches`.
            Nav::Favorite(_) => {}
            Nav::Team(_, section) => {
                let Some(team_id) = self.team_id() else {
                    return;
                };
                let request = match section {
                    TeamSection::Issues => Request::Issues {
                        team_id,
                        after: None,
                        preset: self.lists[IssueSource::Team].preset,
                    },
                    TeamSection::Projects => Request::Projects {
                        team_id,
                        after: None,
                    },
                    TeamSection::Cycles => Request::Cycles {
                        team_id,
                        after: None,
                    },
                    TeamSection::Views => return,
                };
                self.request(request);
            }
        }
    }

    /// Force a refetch of the current destination, discarding its cached flag.
    pub fn force_reload(&mut self) {
        self.global_search = None;
        self.prefetched.clear();
        match self.nav {
            Nav::MyIssues => self.store.issues[IssueSource::My].loaded = false,
            Nav::Views | Nav::Team(_, TeamSection::Views) => self.store.views_loaded = false,
            Nav::View(_) => {
                self.store.loaded_view_id = None;
                self.store.loaded_view_projects_id = None;
            }
            Nav::Team(_, TeamSection::Projects) => self.store.projects.loaded = false,
            Nav::Team(_, TeamSection::Cycles) => self.store.cycles.loaded = false,
            Nav::Team(_, TeamSection::Issues) | Nav::Favorite(_) => {}
        }
        if self.screen == Screen::ProjectDetail {
            self.store.issues[IssueSource::Project].loaded = false;
        }
        if self.screen == Screen::CycleDetail {
            self.store.issues[IssueSource::Cycle].loaded = false;
        }
        self.reload_current_tab();
        self.queue_detail_fetches();
    }

    /// Queue whatever the current screen still needs but has not fetched yet.
    pub fn queue_detail_fetches(&mut self) {
        match self.screen {
            Screen::IssueDetail => {
                if let Some(issue) = &self.store.current_issue
                    && issue.comments.is_none()
                {
                    let issue_id = issue.id.clone();
                    self.request(Request::IssueDetail { issue_id });
                }
            }
            Screen::ProjectDetail => {
                if !self.store.issues[IssueSource::Project].loaded
                    && let Some(project) = &self.current_project
                {
                    let project_id = project.id.clone();
                    self.request(Request::ProjectIssues {
                        project_id,
                        after: None,
                    });
                }
            }
            Screen::CycleDetail => {
                if !self.store.issues[IssueSource::Cycle].loaded
                    && let Some(cycle) = &self.current_cycle
                {
                    let cycle_id = cycle.id.clone();
                    self.request(Request::CycleIssues {
                        cycle_id,
                        after: None,
                    });
                }
            }
            _ => {}
        }
    }

    // ---------------------------------------------------------------- messages

    pub fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::Teams(teams) => {
                if let Some(default_team) = &self.default_team
                    && let Some(idx) = teams
                        .iter()
                        .position(|t| t.name == *default_team || t.key == *default_team)
                {
                    self.selected_team_index = idx;
                }
                self.store.teams = teams;
                self.nav = Nav::Team(self.selected_team_index, TeamSection::Issues);
                if let Some(team_id) = self.team_id() {
                    self.ensure_team_context(team_id.clone());
                    self.request(Request::Issues {
                        team_id,
                        after: None,
                        preset: self.lists[IssueSource::Team].preset,
                    });
                }
            }
            Message::Viewer(id) => {
                self.store.viewer_id = Some(id);
                if self.nav == Nav::MyIssues {
                    self.reload_current_tab();
                }
            }
            Message::TeamContext {
                team_id,
                states,
                members,
            } => {
                // Kept whichever team is selected: it is filed under its own
                // team, so it can only ever be offered for that team's issues.
                self.team_contexts_pending.remove(&team_id);
                let waiting = self.popup_team_id() == Some(&team_id);
                self.store
                    .team_contexts
                    .insert(team_id, TeamContext { states, members });
                // A popup opened while this was loading starts on the
                // issue's current value, as it would have if it were cached.
                if waiting {
                    self.popup_index = self.popup_initial_index();
                }
            }
            Message::Issues {
                team_id,
                preset,
                page,
            } => {
                // A page for a team or preset the user has since left would
                // file, say, another team's backlog under this team's Active.
                if !self.is_current_team(&team_id) || preset != self.lists[IssueSource::Team].preset
                {
                    return;
                }
                self.accept_issue_page(IssueSource::Team, page);
                self.clear_status();
            }
            Message::MyIssues(page) => {
                self.accept_issue_page(IssueSource::My, page);
                self.clear_status();
            }
            Message::SearchResults {
                term,
                team_id,
                issues,
            } => {
                // Results scoped to a team the user has left would be shown
                // under the new team's name.
                if team_id.is_some() && team_id != self.team_id() {
                    return;
                }
                self.nav = Nav::Team(self.selected_team_index, TeamSection::Issues);
                self.screen = Screen::IssueList;
                self.store.issues[IssueSource::Team].items = issues;
                self.store.issues[IssueSource::Team].page_info = PageInfo::default();
                self.lists[IssueSource::Team].filters.clear();
                // Search answers "where is it", done or not; the Active slice
                // would quietly hide half the matches. Set without refetching,
                // which would replace the results with the team's list.
                self.lists[IssueSource::Team].preset = Preset::All;
                self.lists[IssueSource::Team].search.clear();
                self.lists[IssueSource::Team].selected = 0;
                self.global_search = Some(term);
            }
            Message::ViewProjects { view_id, page } => {
                if !self.accepts_view_page(&view_id, page.append, ViewKind::Projects) {
                    return;
                }
                let append = page.append;
                self.store.view_projects.accept(page);
                if !append {
                    self.selected_view_project_index = 0;
                }
                self.store.loaded_view_projects_id = Some(view_id);
                self.clear_status();
            }
            Message::Projects { team_id, page } => {
                if !self.is_current_team(&team_id) {
                    return;
                }
                self.store.projects.accept(page);
                self.clamp(Field::Projects);
                self.clear_status();
            }
            Message::Cycles { team_id, page } => {
                if !self.is_current_team(&team_id) {
                    return;
                }
                self.store.cycles.accept(page);
                self.clamp(Field::Cycles);
                self.clear_status();
            }
            Message::CustomViews(views) => {
                // Keep the open view pointing at the same view across a refetch;
                // the index alone would silently swap which one is on screen.
                let open = match self.nav {
                    Nav::View(i) => self.store.custom_views.get(i).map(|v| v.id.clone()),
                    _ => None,
                };
                self.store.set_custom_views(views);
                if let Some(id) = open {
                    match self.store.custom_views.iter().position(|v| v.id == id) {
                        Some(i) => self.nav = Nav::View(i),
                        None => self.activate(Nav::Views),
                    }
                }
                self.selected_view_index = self
                    .selected_view_index
                    .min(self.listed_views().len().saturating_sub(1));
            }
            Message::Favorites(favorites) => self.store.set_favorites(favorites),
            Message::ViewIssues { view_id, page } => {
                if !self.accepts_view_page(&view_id, page.append, ViewKind::Issues) {
                    return;
                }
                self.store.loaded_view_id = Some(view_id);
                self.accept_issue_page(IssueSource::View, page);
                self.clear_status();
            }
            Message::IssueDetail(issue) => self.store.refresh_issue(*issue),
            Message::ProjectIssues { project_id, page } => {
                if self.current_project.as_ref().map(|p| &p.id) != Some(&project_id) {
                    return;
                }
                self.accept_issue_page(IssueSource::Project, page);
            }
            Message::CycleIssues { cycle_id, page } => {
                if self.current_cycle.as_ref().map(|c| &c.id) != Some(&cycle_id) {
                    return;
                }
                self.accept_issue_page(IssueSource::Cycle, page);
            }
            Message::IssueCreated { team_id, issue } => {
                self.set_status(format!("Created {}", issue.identifier));
                // Another team's list is not on screen; its next fetch will
                // include the issue anyway.
                if !self.is_current_team(&team_id) {
                    return;
                }
                // Show it immediately rather than waiting for a refetch, with
                // the cursor on it — found by id, since grouping decides where
                // in the list it lands.
                let id = issue.id.clone();
                self.store.issues[IssueSource::Team].items.insert(0, *issue);
                if self.issue_source() == IssueSource::Team && self.screen == Screen::IssueList {
                    self.restore_issue_selection(Some(&id));
                }
            }
            Message::Mutated(what) => {
                self.set_status(what);
                // A posted comment clears the cached thread; pull it back in.
                self.queue_detail_fetches();
            }
            Message::Failed { request, error } => {
                // A failed page must be retryable.
                if let Some(cursor) = request.cursor() {
                    self.prefetched.remove(cursor);
                }
                // Reopening the popup asks again.
                if let Request::TeamContext { team_id } = request.as_ref() {
                    self.team_contexts_pending.remove(team_id);
                }
                // The change is already on screen; ask Linear what the issue
                // really looks like now rather than guess what to undo.
                if let Some(issue_id) = request.patched_issue() {
                    self.request(Request::IssueDetail {
                        issue_id: issue_id.clone(),
                    });
                }
                self.set_error(format!("{}: {error}", request.failure()));
            }
        }
    }

    fn is_current_team(&self, team_id: &TeamId) -> bool {
        self.current_team().is_some_and(|t| &t.id == team_id)
    }

    /// Whether a page of saved view `view_id` belongs on screen.
    ///
    /// A first page is taken only for the view that is open; a next page only
    /// when the list it would extend is that view's.
    fn accepts_view_page(&self, view_id: &CustomViewId, append: bool, kind: ViewKind) -> bool {
        if append {
            let loaded = match kind {
                ViewKind::Issues => &self.store.loaded_view_id,
                ViewKind::Projects => &self.store.loaded_view_projects_id,
            };
            return loaded.as_ref() == Some(view_id);
        }
        matches!(self.nav, Nav::View(i) if self.store.custom_views.get(i).is_some_and(|v| &v.id == view_id))
    }

    /// Fold a page into one of the issue lists, keeping the cursor on the
    /// issue it was on.
    ///
    /// Grouping decides where each row lands, so even an appended page can
    /// slot issues in above the cursor; restoring by index would quietly move
    /// the selection — and whatever popup is open — to another issue.
    fn accept_issue_page(&mut self, source: IssueSource, page: Page<Issue>) {
        let on_screen = source == self.issue_source();
        let keep = if on_screen {
            self.selected_issue_id()
        } else {
            None
        };
        let append = page.append;
        self.store.issues[source].accept_issues(page);
        if on_screen {
            self.restore_issue_selection(keep.as_ref());
        } else if !append {
            *self.selected_index_of(source) = 0;
        }
    }

    /// Keep a selection index inside its (possibly shrunken) list.
    fn clamp(&mut self, field: Field) {
        let (len, index) = match field {
            Field::Projects => (
                self.store.projects.items.len(),
                &mut self.selected_project_index,
            ),
            Field::Cycles => (
                self.store.cycles.items.len(),
                &mut self.selected_cycle_index,
            ),
        };
        *index = (*index).min(len.saturating_sub(1));
    }

    // -------------------------------------------------------------- selections

    pub fn current_team(&self) -> Option<&Team> {
        self.store.teams.get(self.selected_team_index)
    }

    fn selected_issue_id(&self) -> Option<IssueId> {
        self.visible_issues()
            .get(self.selected_index())
            .map(|i| i.id.clone())
    }

    /// Put the cursor back on the issue it was on, by identity.
    ///
    /// A refetch reorders the list — `updatedAt` moves the moment anyone
    /// touches an issue — so restoring by row index would quietly select a
    /// different issue than the one the user was looking at.
    fn restore_issue_selection(&mut self, id: Option<&IssueId>) {
        let index = id
            .and_then(|id| self.visible_issues().iter().position(|i| &i.id == id))
            .unwrap_or(0);
        *self.selected_index_mut() = index;
    }

    // ------------------------------------------------------------ project lists

    /// Whether the project list on screen is a saved project view's rather
    /// than the team's.
    fn in_project_view(&self) -> bool {
        matches!(self.nav, Nav::View(i) if self.store.custom_views.get(i).is_some_and(|v| ViewKind::of(v) == ViewKind::Projects))
    }

    /// The projects the project list is showing.
    pub fn project_rows(&self) -> &[Project] {
        if self.in_project_view() {
            &self.store.view_projects.items
        } else {
            &self.store.projects.items
        }
    }

    pub fn project_cursor(&self) -> usize {
        if self.in_project_view() {
            self.selected_view_project_index
        } else {
            self.selected_project_index
        }
    }

    fn project_cursor_mut(&mut self) -> &mut usize {
        if self.in_project_view() {
            &mut self.selected_view_project_index
        } else {
            &mut self.selected_project_index
        }
    }

    /// Scroll offset of the project list on screen.
    pub fn project_offset(&mut self) -> &mut usize {
        if self.in_project_view() {
            &mut self.frame.offsets.view_projects
        } else {
            &mut self.frame.offsets.projects
        }
    }

    // ----------------------------------------------------------- views pages

    /// The team whose views a Views page shows, or `None` for the workspace
    /// page.
    fn views_scope(&self) -> Option<&TeamId> {
        match self.nav {
            Nav::Team(index, TeamSection::Views) => self.store.teams.get(index).map(|t| &t.id),
            _ => None,
        }
    }

    /// The views the current Views page lists, as indices into `custom_views`.
    ///
    /// As in Linear, the workspace page holds views that belong to no team,
    /// and each team's page holds the views scoped to it; the tab picks issue
    /// or project views.
    pub fn listed_views(&self) -> Vec<usize> {
        let scope = self.views_scope();
        self.store
            .custom_views
            .iter()
            .enumerate()
            .filter(|(_, v)| v.team.as_ref().map(|t| &t.id) == scope)
            .filter(|(_, v)| ViewKind::of(v) == self.view_kind)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn set_view_kind(&mut self, kind: ViewKind) {
        if self.view_kind != kind {
            self.view_kind = kind;
            self.selected_view_index = 0;
        }
    }

    pub fn cycle_view_kind(&mut self) {
        self.set_view_kind(match self.view_kind {
            ViewKind::Issues => ViewKind::Projects,
            ViewKind::Projects => ViewKind::Issues,
        });
    }

    /// The screen a destination opens: a project view is a project list.
    fn screen_for(&self, nav: Nav) -> Screen {
        match nav {
            Nav::View(i)
                if self
                    .store
                    .custom_views
                    .get(i)
                    .is_some_and(|v| ViewKind::of(v) == ViewKind::Projects) =>
            {
                Screen::ProjectList
            }
            _ => nav.screen(),
        }
    }

    fn maybe_prefetch_cycles(&mut self) {
        if self.selected_cycle_index + PREFETCH_MARGIN < self.store.cycles.items.len()
            || !self.store.cycles.page_info.has_next_page
        {
            return;
        }
        if let (Some(team_id), Some(cursor)) = (
            self.team_id(),
            self.store.cycles.page_info.end_cursor.clone(),
        ) && self.prefetched.insert(cursor.clone())
        {
            self.request(Request::Cycles {
                team_id,
                after: Some(cursor),
            });
        }
    }

    pub fn open_issue_detail(&mut self) {
        if let Some(issue) = self.visible_issues().get(self.selected_index()).copied() {
            let issue = issue.clone();
            self.open_issue_from_list(&issue);
        }
    }

    pub fn open_team_select(&mut self) {
        self.popup = Popup::TeamSelect;
        self.popup_index = self.selected_team_index;
    }

    /// Pick a team from the switcher and go to it, keeping the page (Issues,
    /// Cycles, Projects) when already on one of the team's pages.
    pub fn select_team(&mut self) {
        self.popup = Popup::None;
        if self.popup_index >= self.store.teams.len() {
            return;
        }
        let section = match self.nav {
            Nav::Team(_, section) => section,
            _ => TeamSection::Issues,
        };
        self.activate(Nav::Team(self.popup_index, section));
    }

    pub fn open_filter(&mut self) {
        for team_id in self.list_team_ids() {
            self.ensure_team_context(team_id);
        }
        self.popup = Popup::Filter(FilterKind::Status);
        self.popup_index = 0;
    }

    pub fn open_status_change(&mut self) {
        if let Some(issue) = self.focused_issue() {
            self.open_change_popup(Popup::StatusChange(issue.id.clone()));
        }
    }

    /// Open a status or assignee popup, fetching the issue's team first if
    /// its states and members have not been loaded yet.
    fn open_change_popup(&mut self, popup: Popup) {
        self.popup = popup;
        if let Some(team_id) = self.popup_team_id().cloned() {
            self.ensure_team_context(team_id);
        }
        self.popup_index = self.popup_initial_index();
    }

    /// Where a change popup starts: on the issue's current value, so Enter is
    /// a no-op rather than a surprise.
    fn popup_initial_index(&self) -> usize {
        let Some(issue) = self.popup_issue() else {
            return 0;
        };
        match self.popup {
            Popup::StatusChange(_) => issue
                .state
                .as_ref()
                .and_then(|state| self.popup_states().iter().position(|s| s.id == state.id))
                .unwrap_or(0),
            // Row 0 is Unassign, so a member's row is one past their index.
            Popup::AssigneeChange(_) => issue
                .assignee
                .as_ref()
                .and_then(|user| self.popup_members().iter().position(|u| u.id == user.id))
                .map_or(0, |i| i + 1),
            _ => 0,
        }
    }

    // ----------------------------------------------------------- team context

    /// The team `issue` belongs to. An issue that does not say is taken to be
    /// the selected team's.
    pub fn issue_team_id<'a>(&'a self, issue: &'a Issue) -> Option<&'a TeamId> {
        self.store
            .issue_team_id(issue, self.current_team().map(|t| &t.id))
    }

    /// Ask for a team's states and members, unless they are loaded or on the way.
    fn ensure_team_context(&mut self, team_id: TeamId) {
        if self.store.team_contexts.contains_key(&team_id)
            || !self.team_contexts_pending.insert(team_id.clone())
        {
            return;
        }
        self.request(Request::TeamContext { team_id });
    }

    /// The team whose states or members the open change popup offers.
    fn popup_team_id(&self) -> Option<&TeamId> {
        match self.popup {
            Popup::StatusChange(_) | Popup::AssigneeChange(_) => {
                self.issue_team_id(self.popup_issue()?)
            }
            _ => None,
        }
    }

    fn popup_context(&self) -> Option<&TeamContext> {
        self.store.team_contexts.get(self.popup_team_id()?)
    }

    /// True while a change popup waits for its issue's team to load.
    pub fn popup_loading(&self) -> bool {
        match self.popup {
            Popup::StatusChange(_) | Popup::AssigneeChange(_) => self.popup_context().is_none(),
            Popup::Filter(FilterKind::Status) => self
                .list_team_ids()
                .iter()
                .any(|id| !self.store.team_contexts.contains_key(id)),
            _ => false,
        }
    }

    /// The states the status popup offers: those of the issue's own team.
    pub fn popup_states(&self) -> &[WorkflowState] {
        self.popup_context().map_or(&[], |c| &c.states)
    }

    /// The members the assignee popup offers: those of the issue's own team.
    pub fn popup_members(&self) -> &[User] {
        self.popup_context().map_or(&[], |c| &c.members)
    }

    /// Teams of the issues in the list on screen, in order of first
    /// appearance; the selected team when the list is empty.
    fn list_team_ids(&self) -> Vec<TeamId> {
        let mut ids: Vec<TeamId> = Vec::new();
        for issue in &self.store.issues[self.issue_source()].items {
            if let Some(id) = self.issue_team_id(issue)
                && !ids.contains(id)
            {
                ids.push(id.clone());
            }
        }
        if ids.is_empty() {
            ids.extend(self.team_id());
        }
        ids
    }

    /// The states the status filter offers. The filter matches by name, so a
    /// list spanning teams offers each name once, whichever team it is from.
    pub fn filter_states(&self) -> Vec<&WorkflowState> {
        let mut states: Vec<&WorkflowState> = Vec::new();
        for id in self.list_team_ids() {
            let Some(context) = self.store.team_contexts.get(&id) else {
                continue;
            };
            for state in &context.states {
                if !states.iter().any(|s| s.name == state.name) {
                    states.push(state);
                }
            }
        }
        states
    }

    pub fn open_priority_change(&mut self) {
        if let Some(issue) = self.focused_issue() {
            let index = issue.priority.as_index();
            self.popup = Popup::PriorityChange(issue.id.clone());
            self.popup_index = index;
        }
    }

    pub fn open_assignee_change(&mut self) {
        if let Some(issue) = self.focused_issue() {
            self.open_change_popup(Popup::AssigneeChange(issue.id.clone()));
        }
    }

    /// Linear's `I` — assign the focused issue to the current user.
    pub fn assign_to_me(&mut self) {
        let Some(viewer_id) = self.store.viewer_id.clone() else {
            self.set_status("Current user is not loaded yet");
            return;
        };
        let Some(issue_id) = self.focused_issue().map(|i| i.id.clone()) else {
            return;
        };
        let me = self.store.viewer().cloned();
        self.store
            .patch_issue(&issue_id, |i| i.assignee = me.clone());
        self.request(Request::UpdateAssignee {
            issue_id,
            assignee_id: Some(viewer_id),
        });
    }

    /// Linear's `Shift+1`…`Shift+4` / `Shift+0` — set a priority without the menu.
    pub fn set_priority(&mut self, priority: Priority) {
        let Some(issue_id) = self.focused_issue().map(|i| i.id.clone()) else {
            return;
        };
        self.store.patch_issue(&issue_id, |i| {
            i.priority = priority;
            i.priority_label = Some(priority.label().to_string());
        });
        self.request(Request::UpdatePriority { issue_id, priority });
    }

    pub fn start_comment(&mut self) {
        if self.focused_issue().is_some() {
            self.input_mode = InputMode::Comment;
            self.comment.clear();
        }
    }

    pub fn submit_comment(&mut self) {
        if let Some(issue) = self.focused_issue()
            && !self.comment.is_empty()
        {
            let issue_id = issue.id.clone();
            let body = self.comment.value.clone();
            // Drop the cached comments so the detail view refetches them once
            // the mutation lands.
            if let Some(current) = &mut self.store.current_issue
                && current.id == issue_id
            {
                current.comments = None;
            }
            self.request(Request::CreateComment { issue_id, body });
        }
        self.input_mode = InputMode::Normal;
        self.comment.clear();
    }

    // -------------------------------------------------------------- mutations

    pub fn apply_status_selection(&mut self) {
        // Nothing to pick while the issue's team loads; the popup stays open.
        let Some(state) = self.popup_states().get(self.popup_index).cloned() else {
            return;
        };
        if let Popup::StatusChange(issue_id) = self.take_popup() {
            let state_id = state.id.clone();
            self.store
                .patch_issue(&issue_id, |i| i.state = Some(state.clone()));
            self.request(Request::UpdateStatus { issue_id, state_id });
        }
    }

    pub fn apply_priority_selection(&mut self) {
        if let Popup::PriorityChange(issue_id) = self.take_popup() {
            let priority = Priority::from_index(self.popup_index);
            self.store.patch_issue(&issue_id, |i| {
                i.priority = priority;
                i.priority_label = Some(priority.label().to_string());
            });
            self.request(Request::UpdatePriority { issue_id, priority });
        }
    }

    pub fn apply_assignee_selection(&mut self) {
        let assignee = if self.popup_index == 0 {
            None // Unassign
        } else {
            // Only Unassign can be picked while the issue's team loads.
            let Some(member) = self.popup_members().get(self.popup_index - 1).cloned() else {
                return;
            };
            Some(member)
        };
        if let Popup::AssigneeChange(issue_id) = self.take_popup() {
            let assignee_id = assignee.as_ref().map(|u| u.id.clone());
            self.store
                .patch_issue(&issue_id, |i| i.assignee = assignee.clone());
            self.request(Request::UpdateAssignee {
                issue_id,
                assignee_id,
            });
        }
    }

    // ------------------------------------------------------------------ popups

    pub fn close_popup(&mut self) {
        self.popup = Popup::None;
    }

    /// The issue the open change popup acts on.
    pub fn popup_issue(&self) -> Option<&Issue> {
        let id = match &self.popup {
            Popup::StatusChange(id) | Popup::PriorityChange(id) | Popup::AssigneeChange(id) => id,
            _ => return None,
        };
        self.store.issue(id)
    }

    /// Close the popup, handing back what it was open for.
    fn take_popup(&mut self) -> Popup {
        std::mem::replace(&mut self.popup, Popup::None)
    }

    pub fn popup_next(&mut self) {
        let max = self.popup_list_len();
        if max > 0 && self.popup_index < max - 1 {
            self.popup_index += 1;
        }
    }

    pub fn popup_first(&mut self) {
        self.popup_index = 0;
    }

    pub fn popup_last(&mut self) {
        self.popup_index = self.popup_list_len().saturating_sub(1);
    }

    /// Pick popup entry `index` outright, as a number key or a click does.
    pub fn popup_pick(&mut self, index: usize) {
        if index < self.popup_list_len() {
            self.popup_index = index;
            self.apply_popup();
        }
    }

    pub fn popup_prev(&mut self) {
        if self.popup_index > 0 {
            self.popup_index -= 1;
        }
    }

    pub fn popup_list_len(&self) -> usize {
        match self.popup {
            Popup::TeamSelect => self.store.teams.len(),
            // Each filter list starts with an "any" row.
            Popup::Filter(FilterKind::Status) => self.filter_states().len() + 1,
            Popup::Filter(FilterKind::Priority) => Priority::ALL.len() + 1,
            Popup::StatusChange(_) => self.popup_states().len(),
            Popup::PriorityChange(_) => Priority::ALL.len(),
            // +1 for Unassign
            Popup::AssigneeChange(_) => self.popup_members().len() + 1,
            Popup::None => 0,
        }
    }

    pub fn apply_filter_selection(&mut self) {
        let Popup::Filter(kind) = self.popup else {
            return;
        };
        match kind {
            FilterKind::Status => {
                if self.popup_index == 0 {
                    self.list_mut().filters.status = None;
                } else if let Some(state) = self.filter_states().get(self.popup_index - 1) {
                    let name = state.name.clone();
                    self.list_mut().filters.status = Some(name);
                }
                self.popup = Popup::Filter(FilterKind::Priority);
                self.popup_index = 0;
            }
            FilterKind::Priority => {
                let priority = match self.popup_index {
                    0 => None,
                    n => Some(Priority::from_index(n)),
                };
                self.list_mut().filters.priority = priority;
                self.popup = Popup::None;
                *self.selected_index_mut() = 0;
            }
        }
    }

    pub fn clear_filters(&mut self) {
        self.list_mut().filters.clear();
        *self.selected_index_mut() = 0;
    }

    // ------------------------------------------------------------------ scroll

    /// Largest scroll offset that still shows content.
    fn max_detail_scroll(&self) -> u16 {
        self.frame
            .detail_lines
            .saturating_sub(self.frame.detail_viewport)
    }

    pub fn scroll_down(&mut self) {
        self.detail_scroll = self
            .detail_scroll
            .saturating_add(1)
            .min(self.max_detail_scroll());
    }

    pub fn scroll_up(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_sub(1);
    }

    pub fn scroll_by(&mut self, delta: i16) {
        let next = self.detail_scroll as i32 + delta as i32;
        self.detail_scroll = next.clamp(0, self.max_detail_scroll() as i32) as u16;
    }

    /// Number of rows a half-page jump should cover.
    pub fn half_page(&self) -> isize {
        (self.frame.list_viewport / 2).max(1) as isize
    }

    /// Move the list cursor on the current screen by `delta` rows, requesting
    /// the next page if the cursor nears the end of a paginated list.
    pub fn move_selection(&mut self, delta: isize) {
        match self.screen {
            Screen::ProjectList => {
                let len = self.project_rows().len();
                let mut index = self.project_cursor();
                Self::nav_by(len, &mut index, delta);
                *self.project_cursor_mut() = index;
                self.maybe_prefetch_projects();
            }
            Screen::CycleList => {
                Self::nav_by(
                    self.store.cycles.items.len(),
                    &mut self.selected_cycle_index,
                    delta,
                );
                self.maybe_prefetch_cycles();
            }
            Screen::ViewList => {
                let len = self.listed_views().len();
                Self::nav_by(len, &mut self.selected_view_index, delta);
            }
            Screen::IssueDetail => {}
            // Every issue list scrolls the same way, whichever one it is.
            Screen::IssueList | Screen::ProjectDetail | Screen::CycleDetail => {
                let len = self.visible_issues().len();
                let mut index = self.selected_index();
                Self::nav_by(len, &mut index, delta);
                *self.selected_index_mut() = index;
                self.maybe_prefetch_within(len);
            }
        }
    }

    /// Step to the neighbouring issue without leaving the detail view — the
    /// `↓`/`↑` pair Linear puts next to the "6 / 203" counter.
    pub fn step_issue(&mut self, delta: isize) {
        if self.screen != Screen::IssueDetail {
            return;
        }
        let Some(current) = &self.store.current_issue else {
            return;
        };
        // One grouping pass serves the position, the neighbour, and the count.
        let issues = self.visible_issues();
        let Some(position) = issues.iter().position(|i| i.id == current.id) else {
            return;
        };
        let total = issues.len();
        let next = (position as isize + delta).clamp(0, total as isize - 1) as usize;
        if next == position {
            return;
        }
        let issue = issues[next].clone();
        *self.selected_index_mut() = next;
        self.maybe_prefetch_within(total);
        let ret = self.detail_return;
        self.open_issue_from_list(&issue);
        self.detail_return = ret;
    }

    /// Where the open issue sits in the list it came from: `(index, total)`.
    pub fn detail_position(&self) -> Option<(usize, usize)> {
        let current = self.store.current_issue.as_ref()?;
        let issues = self.visible_issues();
        let index = issues.iter().position(|i| i.id == current.id)?;
        Some((index, issues.len()))
    }

    /// Drop the cached issue detail and fetch it again.
    pub fn refresh_detail(&mut self) {
        if let Some(issue) = &mut self.store.current_issue {
            issue.comments = None;
        }
        self.queue_detail_fetches();
    }

    /// Jump to the first row of the current list.
    pub fn select_first(&mut self) {
        self.move_selection(isize::MIN / 2);
    }

    /// Jump to the last row of the current list, pulling in another page if there is one.
    pub fn select_last(&mut self) {
        self.move_selection(isize::MAX / 2);
    }

    /// Mouse wheel: move the list cursor, or scroll the detail body.
    pub fn scroll_list_or_detail(&mut self, delta: i16) {
        if self.screen == Screen::IssueDetail {
            self.scroll_by(delta);
        } else {
            self.move_selection(delta as isize);
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.detail_scroll = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.detail_scroll = self.max_detail_scroll();
    }

    // ------------------------------------------------------------------ search

    /// Open the search box on the list on screen.
    pub fn start_search(&mut self) {
        self.input_mode = InputMode::Search;
        self.list_mut().search.clear();
    }

    /// Hand the current query to Linear's workspace-wide search.
    pub fn search_workspace(&mut self) {
        if self.list().search.is_empty() {
            return;
        }
        let term = self.list().search.value.clone();
        let team_id = self.team_id();
        self.set_status(format!("Searching Linear for \"{term}\"…"));
        self.request(Request::Search { term, team_id });
        self.input_mode = InputMode::Normal;
    }

    /// Re-run the live filter. Matching happens inside [`Self::sections`], so
    /// this only has to put the cursor back at the top of what is left.
    pub fn apply_search(&mut self) {
        *self.selected_index_mut() = 0;
    }

    pub fn clear_search(&mut self) {
        let list = self.list_mut();
        list.search.clear();
        list.selected = 0;
        // Leaving a workspace search returns the list to the team's own issues.
        if self.issue_source() == IssueSource::Team && self.global_search.take().is_some() {
            self.lists[IssueSource::Team].preset = Preset::Active;
            self.reload_current_tab();
        }
    }

    // ------------------------------------------------- open / copy / create

    /// Open the focused issue (or the selected project) on linear.app.
    pub fn open_in_browser(&mut self) {
        let url = match self.screen {
            Screen::ProjectList => self
                .project_rows()
                .get(self.project_cursor())
                .and_then(|p| p.url.clone()),
            Screen::ProjectDetail if self.focused_issue().is_none() => {
                self.current_project.as_ref().and_then(|p| p.url.clone())
            }
            _ => self.focused_issue().and_then(|i| i.url.clone()),
        };
        match url {
            Some(url) => self.request(Request::OpenUrl(url)),
            None => self.set_status("Nothing to open here"),
        }
    }

    /// Queue `text` for the clipboard; the main loop emits the OSC 52 sequence.
    fn copy(&mut self, what: &str, text: Option<String>) {
        match text {
            Some(text) => {
                self.set_status(format!("Copied {what}: {text}"));
                self.pending_clipboard = Some(text);
            }
            None => self.set_status(format!("No {what} to copy")),
        }
    }

    pub fn copy_identifier(&mut self) {
        let id = self.focused_issue().map(|i| i.identifier.clone());
        self.copy("identifier", id);
    }

    pub fn copy_url(&mut self) {
        let url = self.focused_issue().and_then(|i| i.url.clone());
        self.copy("URL", url);
    }

    pub fn copy_branch_name(&mut self) {
        let branch = self.focused_issue().and_then(|i| i.branch_name.clone());
        self.copy("branch name", branch);
    }

    /// Open the issue-creation form for the current team.
    pub fn start_new_issue(&mut self) {
        if self.team_id().is_none() {
            self.set_status("Select a team first");
            return;
        }
        self.new_issue = Some(NewIssueForm::default());
        self.input_mode = InputMode::NewIssue;
    }

    pub fn cancel_new_issue(&mut self) {
        self.new_issue = None;
        self.input_mode = InputMode::Normal;
    }

    pub fn new_issue_cycle_field(&mut self, forward: bool) {
        if let Some(form) = &mut self.new_issue {
            form.field = if forward {
                form.field.next()
            } else {
                form.field.prev()
            };
        }
    }

    pub fn new_issue_cycle_priority(&mut self, delta: isize) {
        if let Some(form) = &mut self.new_issue {
            let next = (form.priority.as_index() as isize + delta).rem_euclid(5);
            form.priority = Priority::from_index(next as usize);
        }
    }

    pub fn submit_new_issue(&mut self) {
        let Some(form) = &self.new_issue else {
            return;
        };
        if form.title.is_empty() {
            self.set_status("A title is required");
            return;
        }
        let Some(team_id) = self.team_id() else {
            return;
        };
        let title = form.title.value.clone();
        let description = Some(form.description.value.clone()).filter(|d| !d.is_empty());
        let priority = form.priority;
        self.request(Request::CreateIssue {
            team_id,
            title,
            description,
            priority,
        });
        self.set_status("Creating issue…");
        self.cancel_new_issue();
    }

    // ------------------------------------------------------------------- mouse

    /// Apply the highlighted popup entry.
    pub fn apply_popup(&mut self) {
        match self.popup {
            Popup::TeamSelect => self.select_team(),
            Popup::Filter(_) => self.apply_filter_selection(),
            Popup::StatusChange(_) => self.apply_status_selection(),
            Popup::PriorityChange(_) => self.apply_priority_selection(),
            Popup::AssigneeChange(_) => self.apply_assignee_selection(),
            Popup::None => {}
        }
    }

    // ------------------------------------------------------------------ status

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn open_help(&mut self) {
        self.show_help = true;
        self.help_scroll = 0;
    }

    pub fn close_help(&mut self) {
        self.show_help = false;
    }

    /// Scroll the help overlay. The renderer clamps the offset to what the
    /// overlay can show, so scrolling past the end does not bank rows that
    /// then have to be scrolled back through.
    pub fn scroll_help(&mut self, delta: i16) {
        self.help_scroll = self.help_scroll.saturating_add_signed(delta);
    }

    /// Begin Linear's `g …` chord; the next key completes it.
    pub fn start_goto_chord(&mut self) {
        self.pending_chord = Some('g');
    }

    pub fn end_chord(&mut self) {
        self.pending_chord = None;
    }

    /// Close the search box, keeping its query as the list's filter.
    pub fn finish_search(&mut self) {
        self.input_mode = InputMode::Normal;
        self.apply_search();
    }

    /// Close the search box and drop its query.
    pub fn cancel_search(&mut self) {
        self.input_mode = InputMode::Normal;
        self.clear_search();
    }

    pub fn cancel_comment(&mut self) {
        self.input_mode = InputMode::Normal;
        self.comment.clear();
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error_popup = Some(msg.into());
    }

    pub fn dismiss_error(&mut self) {
        self.error_popup = None;
    }

    pub fn tick_spinner(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
    }

    pub fn spinner_symbol(&self) -> &'static str {
        SPINNER_FRAMES[self.spinner_frame]
    }

    // ------------------------------------------------------- list nav helpers

    pub fn nav_by(len: usize, index: &mut usize, delta: isize) {
        if len == 0 {
            *index = 0;
            return;
        }
        let next = (*index as isize)
            .saturating_add(delta)
            .clamp(0, len as isize - 1);
        *index = next as usize;
    }

    // -------------------------------------------------------------- navigation

    /// Open issue detail from any sub-list (project issues, cycle issues, my issues).
    pub fn open_issue_from_list(&mut self, issue: &Issue) {
        if self.screen != Screen::IssueDetail {
            self.detail_return = self.screen;
            self.detail_return_nav = self.nav;
        }
        self.store.current_issue = Some(issue.clone());
        self.detail_scroll = 0;
        self.screen = Screen::IssueDetail;
        self.queue_detail_fetches();
    }

    /// Leave the detail view for wherever it was opened from.
    pub fn close_detail(&mut self) {
        self.screen = self.detail_return;
        self.nav = self.detail_return_nav;
    }

    /// Esc on a project or cycle page: back to the team's list it came from,
    /// or — opened from Favorites, where there is no list behind it — to the
    /// sidebar.
    pub fn leave_container(&mut self) {
        match self.nav {
            Nav::Team(_, TeamSection::Projects) => self.screen = Screen::ProjectList,
            Nav::Team(_, TeamSection::Cycles) => self.screen = Screen::CycleList,
            // A project opened from a project view goes back to the view.
            Nav::View(_) => self.screen = Screen::ProjectList,
            _ => self.focus_sidebar(true),
        }
    }

    /// Go to a sidebar destination, fetching whatever it needs.
    pub fn activate(&mut self, nav: Nav) {
        if let Nav::Favorite(index) = nav {
            return self.open_favorite(index);
        }
        // A team destination also selects that team, which is how the sidebar
        // crosses team boundaries without the team-switch popup.
        if let Nav::Team(index, _) = nav
            && index != self.selected_team_index
            && index < self.store.teams.len()
        {
            self.select_team_index(index);
        }
        let screen = self.screen_for(nav);
        if self.nav == nav && self.screen == screen {
            return;
        }
        if screen == Screen::ViewList && self.nav != nav {
            // A different Views page lists different views.
            self.selected_view_index = 0;
        }
        self.nav = nav;
        self.screen = screen;
        self.sidebar_focus = false;

        let cached = match nav {
            Nav::MyIssues => self.store.issues[IssueSource::My].loaded,
            Nav::Views | Nav::Team(_, TeamSection::Views) => self.store.views_loaded,
            Nav::View(index) => self.store.custom_views.get(index).is_some_and(|view| {
                let loaded = match ViewKind::of(view) {
                    ViewKind::Issues => &self.store.loaded_view_id,
                    ViewKind::Projects => &self.store.loaded_view_projects_id,
                };
                loaded.as_ref() == Some(&view.id)
            }),
            Nav::Team(_, TeamSection::Issues) => self.store.issues[IssueSource::Team].loaded,
            Nav::Team(_, TeamSection::Projects) => self.store.projects.loaded,
            Nav::Team(_, TeamSection::Cycles) => self.store.cycles.loaded,
            Nav::Favorite(_) => true,
        };
        if let Nav::View(_) = nav
            && !cached
        {
            // A different view's rows are still in the list; clear them so
            // the old results are not briefly attributed to the new view.
            self.reset_list(IssueSource::View);
            self.store.view_projects.items.clear();
            self.store.view_projects.page_info = PageInfo::default();
            self.selected_view_project_index = 0;
        }
        if !cached {
            self.reload_current_tab();
        }
    }

    /// Go to the current team's issue list.
    pub fn go_to_team_issues(&mut self) {
        self.go_to_team_section(TeamSection::Issues);
    }

    /// Go to a page of the team that is already selected.
    ///
    /// The `g …` chords and the number keys are shorthand for "this section, of
    /// wherever I am" — they should not drag the user to another team.
    pub fn go_to_team_section(&mut self, section: TeamSection) {
        self.activate(Nav::Team(self.selected_team_index, section));
    }

    /// Open the view under the cursor on the saved-view index.
    pub fn open_selected_view(&mut self) {
        if let Some(index) = self.listed_views().get(self.selected_view_index).copied() {
            self.activate(Nav::View(index));
        }
    }

    /// Switch the current team, discarding everything scoped to the old one.
    fn select_team_index(&mut self, index: usize) {
        self.selected_team_index = index;
        // The old team's cursors mean nothing to the new team's lists.
        self.reset_list(IssueSource::Team);
        self.store.projects.page_info = PageInfo::default();
        self.store.cycles.page_info = PageInfo::default();
        // The old team's projects and cycles would otherwise sit on screen,
        // under the new team's name, until the refetch lands.
        self.store.projects.items.clear();
        self.store.cycles.items.clear();
        self.selected_project_index = 0;
        self.selected_cycle_index = 0;
        self.lists[IssueSource::Team].filters.clear();
        self.invalidate_tab_caches();
        if let Some(team_id) = self.team_id() {
            self.ensure_team_context(team_id);
        }
    }

    pub fn invalidate_tab_caches(&mut self) {
        self.store.issues[IssueSource::My].loaded = false;
        self.store.loaded_view_id = None;
        self.store.projects.loaded = false;
        self.store.cycles.loaded = false;
        self.store.issues[IssueSource::Project].loaded = false;
        self.store.issues[IssueSource::Cycle].loaded = false;
    }

    // Project navigation
    pub fn open_project_detail(&mut self) {
        if let Some(project) = self.project_rows().get(self.project_cursor()).cloned() {
            self.open_project(project);
        }
    }

    fn open_project(&mut self, project: Project) {
        self.current_project = Some(project);
        self.reset_list(IssueSource::Project);
        self.screen = Screen::ProjectDetail;
        self.queue_detail_fetches();
    }

    // Cycle navigation
    pub fn open_cycle_detail(&mut self) {
        if let Some(cycle) = self
            .store
            .cycles
            .items
            .get(self.selected_cycle_index)
            .cloned()
        {
            self.open_cycle(cycle);
        }
    }

    fn open_cycle(&mut self, cycle: Cycle) {
        self.current_cycle = Some(cycle);
        self.reset_list(IssueSource::Cycle);
        self.screen = Screen::CycleDetail;
        self.queue_detail_fetches();
    }
}
