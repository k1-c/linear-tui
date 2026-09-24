use std::collections::{HashSet, VecDeque};

use ratatui::layout::Rect;
use ratatui::widgets::TableState;

use crate::api::types::*;
use crate::config::{Config, Theme};
use crate::grouping::{GroupBy, Preset, Section, group};
use crate::message::{Message, Page, Request};

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

/// Which of the app's issue lists the current screen is showing.
///
/// Five lists behave identically once you know which one is on screen —
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
    fn slot(self) -> usize {
        self as usize
    }
}

/// One line of the navigation sidebar.
#[derive(Debug, Clone)]
pub enum SidebarRow {
    /// A section caption. Not selectable.
    Header(String),
    /// Blank spacing between sections. Not selectable.
    Gap,
    Item(SidebarItem),
}

#[derive(Debug, Clone)]
pub struct SidebarItem {
    pub label: String,
    pub icon: &'static str,
    /// Colour Linear gives this team, view, or project, when it has one.
    pub color: Option<ratatui::style::Color>,
    pub depth: u8,
    /// What Enter (or a click) does.
    pub action: SidebarAction,
    /// Open/closed, for a row that heads a foldable group.
    pub expanded: Option<bool>,
    /// Text shown right-aligned, muted.
    pub trailing: Option<String>,
    /// How loudly the row is drawn. Favorites are what people navigate by, so
    /// they read strongest; secondary pages like Views recede.
    pub tone: Tone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Subtle,
    Normal,
    Strong,
}

impl SidebarItem {
    /// The destination this row goes to, if it is one.
    pub fn nav(&self) -> Option<Nav> {
        match self.action {
            SidebarAction::Go(nav) => Some(nav),
            _ => None,
        }
    }
}

fn contains(area: Rect, x: u16, y: u16) -> bool {
    area.width > 0
        && x >= area.x
        && x < area.x + area.width
        && y >= area.y
        && y < area.y + area.height
}

/// One rendered line of a grouped issue list.
#[derive(Debug, Clone)]
pub enum ListRow {
    Group {
        key: String,
        label: String,
        color: ratatui::style::Color,
        glyph: &'static str,
        count: usize,
        collapsed: bool,
    },
    /// `ordinal` indexes into [`App::visible_issues`].
    Issue { ordinal: usize, depth: u8 },
}

/// What activating a sidebar row does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction {
    Go(Nav),
    /// Fold or unfold a Favorites folder, by its position in `favorites`.
    Fold(usize),
    /// Open the team picker — the team row is a switcher, not a tree.
    SwitchTeam,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
    Comment,
    NewIssue,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Popup {
    None,
    TeamSelect,
    Filter,
    StatusChange,
    PriorityChange,
    AssigneeChange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterKind {
    Status,
    Priority,
}

#[derive(Debug, Clone, Default)]
pub struct Filters {
    pub status: Option<String>,
    pub priority: Option<Priority>,
}

impl Filters {
    pub fn is_active(&self) -> bool {
        self.status.is_some() || self.priority.is_some()
    }

    pub fn clear(&mut self) {
        self.status = None;
        self.priority = None;
    }

    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(s) = &self.status {
            parts.push(format!("Status:{s}"));
        }
        if let Some(p) = self.priority {
            parts.push(format!("Priority:{}", p.label()));
        }
        parts.join(" | ")
    }
}

/// A single-line text field with a cursor, used for search and comment input.
#[derive(Debug, Clone, Default)]
pub struct Input {
    pub value: String,
    /// Cursor position as a byte offset into `value`.
    pub cursor: usize,
}

impl Input {
    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn insert(&mut self, c: char) {
        self.value.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn backspace(&mut self) {
        if let Some(prev) = self.prev_boundary() {
            self.value.remove(prev);
            self.cursor = prev;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.value.len() {
            self.value.remove(self.cursor);
        }
    }

    pub fn left(&mut self) {
        if let Some(prev) = self.prev_boundary() {
            self.cursor = prev;
        }
    }

    pub fn right(&mut self) {
        if let Some(next) = self.next_boundary() {
            self.cursor = next;
        }
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.value.len();
    }

    /// Ctrl+U — delete everything before the cursor.
    pub fn kill_to_start(&mut self) {
        self.value.drain(..self.cursor);
        self.cursor = 0;
    }

    /// Ctrl+K — delete everything from the cursor onward.
    pub fn kill_to_end(&mut self) {
        self.value.truncate(self.cursor);
    }

    /// Ctrl+W — delete the whitespace-delimited word before the cursor.
    pub fn kill_word(&mut self) {
        let head = &self.value[..self.cursor];
        let trimmed = head.trim_end();
        let start = trimmed
            .rfind(char::is_whitespace)
            .map(|i| i + trimmed[i..].chars().next().map_or(1, char::len_utf8))
            .unwrap_or(0);
        self.value.drain(start..self.cursor);
        self.cursor = start;
    }

    fn prev_boundary(&self) -> Option<usize> {
        if self.cursor == 0 {
            return None;
        }
        self.value[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
    }

    fn next_boundary(&self) -> Option<usize> {
        self.value[self.cursor..]
            .chars()
            .next()
            .map(|c| self.cursor + c.len_utf8())
    }
}

/// Persistent [`TableState`]s, one per scrollable list.
#[derive(Debug, Default)]
pub struct TableStates {
    pub issues: TableState,
    pub my_issues: TableState,
    pub view_issues: TableState,
    pub views: TableState,
    pub view_projects: TableState,
    pub projects: TableState,
    pub cycles: TableState,
    pub project_issues: TableState,
    pub cycle_issues: TableState,
}

/// Identifies one of the paginated sub-lists, for index clamping.
#[derive(Debug, Clone, Copy)]
enum Field {
    Projects,
    Cycles,
    ProjectIssues,
    CycleIssues,
}

/// Which field of the new-issue form has focus.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FormField {
    #[default]
    Title,
    Description,
    Priority,
}

impl FormField {
    fn next(self) -> Self {
        match self {
            Self::Title => Self::Description,
            Self::Description => Self::Priority,
            Self::Priority => Self::Title,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Title => Self::Priority,
            Self::Description => Self::Title,
            Self::Priority => Self::Description,
        }
    }
}

/// Draft state for the issue-creation form.
#[derive(Debug, Default)]
pub struct NewIssueForm {
    pub title: Input,
    pub description: Input,
    pub priority: Priority,
    pub field: FormField,
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

    // Popup
    pub popup: Popup,
    pub popup_index: usize,

    // Teams
    pub teams: Vec<Team>,
    pub selected_team_index: usize,
    pub team_members: Vec<User>,

    // Issues
    pub issues: Vec<Issue>,
    pub selected_issue_index: usize,
    pub page_info: PageInfo,

    // Filters
    pub filters: Filters,
    pub filter_kind: FilterKind,
    pub workflow_states: Vec<WorkflowState>,

    // Saved views
    pub custom_views: Vec<CustomView>,
    pub views_loaded: bool,
    pub selected_view_index: usize,
    pub view_issues: Vec<Issue>,
    pub view_issues_page_info: PageInfo,
    pub selected_view_issue_index: usize,
    /// Which view `view_issues` belongs to, so switching views refetches.
    pub loaded_view_id: Option<String>,
    /// Which tab the Views pages show.
    pub view_kind: ViewKind,
    // Saved project views
    pub view_projects: Vec<Project>,
    pub view_projects_page_info: PageInfo,
    pub selected_view_project_index: usize,
    pub loaded_view_projects_id: Option<String>,

    // Sidebar
    pub sidebar_visible: bool,
    pub sidebar_width: u16,
    /// True while the cursor is in the sidebar rather than the content pane.
    pub sidebar_focus: bool,
    pub sidebar_index: usize,
    /// First sidebar row on screen, when the tree is taller than the pane.
    pub sidebar_offset: usize,
    /// Rows the sidebar drew last frame, kept for keyboard and mouse hit-testing.
    pub sidebar_rows: Vec<SidebarRow>,
    /// Favorites, in the order Linear's sidebar shows them.
    pub favorites: Vec<Favorite>,
    /// Favorites folders the user has folded, by favorite id.
    pub collapsed_folders: HashSet<String>,

    // List shaping
    pub group_by: GroupBy,
    /// The preset chip selected on each list, indexed by [`IssueSource`].
    ///
    /// A team's issues open on Active, as in Linear; every other list opens on
    /// All, because its contents were already chosen — by a saved view's
    /// filter, by a project, by being yours — and hiding the done half of a
    /// view someone deliberately built would be a surprise.
    presets: [Preset; 5],
    /// Group keys the user has folded away.
    pub collapsed_groups: HashSet<String>,
    /// Display rows the content list drew last frame — only the visible
    /// slice, top to bottom — for mouse hit-testing.
    pub list_rows: Vec<ListRow>,
    /// For the non-issue lists (projects, cycles, views): which item each
    /// visible row of `list_area` selects, top to bottom.
    pub row_targets: Vec<Option<usize>>,
    /// Screen area those rows occupy, so a click can be mapped back to a row.
    pub list_area: Rect,
    /// Where the sidebar was drawn.
    pub sidebar_area: Rect,
    /// Screen area of each preset chip, left to right.
    pub chip_areas: Vec<(Rect, Chip)>,
    /// Where the open popup drew its entries, so they are clickable too.
    pub popup_area: Rect,
    /// First popup entry on screen, when the list scrolls.
    pub popup_offset: usize,

    // Issue detail
    pub current_issue: Option<Issue>,
    /// Screen to return to when the detail view is closed.
    pub detail_return: Screen,
    /// Destination to return to with it — an issue opened from Favorites
    /// points the sidebar at the favorite while it is open.
    pub detail_return_nav: Nav,
    pub detail_scroll: u16,
    /// Rendered height of the detail body, updated each frame so scrolling can clamp.
    pub detail_lines: u16,
    pub detail_viewport: u16,

    // Search
    pub search: Input,
    /// Set while the list shows workspace-wide search results instead of the team's issues.
    pub global_search: Option<String>,

    // Comment input
    pub comment: Input,

    /// Draft issue, present only while the create form is open.
    pub new_issue: Option<NewIssueForm>,

    /// Text the main loop should push to the system clipboard via OSC 52.
    pub pending_clipboard: Option<String>,

    // Viewer (current user)
    pub viewer_id: Option<String>,

    // My Issues
    pub my_issues: Vec<Issue>,
    pub selected_my_issue_index: usize,
    pub my_issues_page_info: PageInfo,
    pub my_issues_loaded: bool,

    // Projects
    pub projects: Vec<Project>,
    pub selected_project_index: usize,
    pub current_project: Option<Project>,
    pub project_issues: Vec<Issue>,
    pub selected_project_issue_index: usize,
    pub projects_loaded: bool,
    pub project_issues_loaded: bool,
    pub projects_page_info: PageInfo,
    pub project_issues_page_info: PageInfo,

    // Cycles
    pub cycles: Vec<Cycle>,
    pub selected_cycle_index: usize,
    pub current_cycle: Option<Cycle>,
    pub cycle_issues: Vec<Issue>,
    pub selected_cycle_issue_index: usize,
    pub cycles_loaded: bool,
    pub cycle_issues_loaded: bool,
    pub cycles_page_info: PageInfo,
    pub cycle_issues_page_info: PageInfo,

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

    /// Height of the visible list body, updated each frame for half-page jumps.
    pub list_viewport: u16,

    /// Scroll offsets for each table, kept across frames so the viewport
    /// doesn't jump when the underlying list changes.
    pub tables: TableStates,

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
            popup: Popup::None,
            popup_index: 0,
            teams: Vec::new(),
            selected_team_index: 0,
            team_members: Vec::new(),
            issues: Vec::new(),
            selected_issue_index: 0,
            page_info: PageInfo::default(),
            filters: Filters::default(),
            filter_kind: FilterKind::Status,
            workflow_states: Vec::new(),
            custom_views: Vec::new(),
            views_loaded: false,
            selected_view_index: 0,
            view_issues: Vec::new(),
            view_issues_page_info: PageInfo::default(),
            selected_view_issue_index: 0,
            loaded_view_id: None,
            view_kind: ViewKind::default(),
            view_projects: Vec::new(),
            view_projects_page_info: PageInfo::default(),
            selected_view_project_index: 0,
            loaded_view_projects_id: None,
            sidebar_visible: config.ui.sidebar,
            sidebar_width: config.ui.sidebar_width.clamp(18, 48),
            sidebar_focus: false,
            sidebar_index: 0,
            sidebar_offset: 0,
            sidebar_rows: Vec::new(),
            favorites: Vec::new(),
            collapsed_folders: HashSet::new(),
            group_by: GroupBy::from_config(config.ui.group_by),
            presets: [
                Preset::Active,
                Preset::All,
                Preset::All,
                Preset::All,
                Preset::All,
            ],
            collapsed_groups: HashSet::new(),
            list_rows: Vec::new(),
            row_targets: Vec::new(),
            list_area: Rect::ZERO,
            sidebar_area: Rect::ZERO,
            chip_areas: Vec::new(),
            popup_area: Rect::ZERO,
            popup_offset: 0,
            current_issue: None,
            detail_return: Screen::IssueList,
            detail_return_nav: Nav::Team(0, TeamSection::Issues),
            detail_scroll: 0,
            detail_lines: 0,
            detail_viewport: 0,
            search: Input::default(),
            global_search: None,
            comment: Input::default(),
            new_issue: None,
            pending_clipboard: None,
            viewer_id: None,
            my_issues: Vec::new(),
            selected_my_issue_index: 0,
            my_issues_page_info: PageInfo::default(),
            my_issues_loaded: false,
            projects: Vec::new(),
            selected_project_index: 0,
            current_project: None,
            project_issues: Vec::new(),
            selected_project_issue_index: 0,
            projects_loaded: false,
            project_issues_loaded: false,
            projects_page_info: PageInfo::default(),
            project_issues_page_info: PageInfo::default(),
            cycles: Vec::new(),
            selected_cycle_index: 0,
            current_cycle: None,
            cycle_issues: Vec::new(),
            selected_cycle_issue_index: 0,
            cycles_loaded: false,
            cycle_issues_loaded: false,
            cycles_page_info: PageInfo::default(),
            cycle_issues_page_info: PageInfo::default(),
            status_message: None,
            spinner_frame: 0,
            error_popup: None,
            pending_chord: None,
            show_help: false,
            help_scroll: 0,
            list_viewport: 0,
            tables: TableStates::default(),
            theme: Theme::from_name(config.ui.theme),
            items_per_page: config.ui.items_per_page,
            default_team: config.ui.default_team.clone(),
        };
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

    pub fn team_id(&self) -> Option<String> {
        self.current_team().map(|t| t.id.clone())
    }

    /// Queue the fetch that populates the current destination.
    pub fn reload_current_tab(&mut self) {
        match self.nav {
            Nav::MyIssues => {
                if let Some(user_id) = self.viewer_id.clone() {
                    self.request(Request::MyIssues {
                        user_id,
                        after: None,
                    });
                }
            }
            Nav::Views | Nav::Team(_, TeamSection::Views) => {
                if !self.views_loaded {
                    self.request(Request::CustomViews);
                }
            }
            Nav::View(index) => {
                if let Some(view) = self.custom_views.get(index) {
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
                        preset: self.presets[IssueSource::Team.slot()],
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
            Nav::MyIssues => self.my_issues_loaded = false,
            Nav::Views | Nav::Team(_, TeamSection::Views) => self.views_loaded = false,
            Nav::View(_) => {
                self.loaded_view_id = None;
                self.loaded_view_projects_id = None;
            }
            Nav::Team(_, TeamSection::Projects) => self.projects_loaded = false,
            Nav::Team(_, TeamSection::Cycles) => self.cycles_loaded = false,
            Nav::Team(_, TeamSection::Issues) | Nav::Favorite(_) => {}
        }
        if self.screen == Screen::ProjectDetail {
            self.project_issues_loaded = false;
        }
        if self.screen == Screen::CycleDetail {
            self.cycle_issues_loaded = false;
        }
        self.reload_current_tab();
        self.queue_detail_fetches();
    }

    /// Queue whatever the current screen still needs but has not fetched yet.
    pub fn queue_detail_fetches(&mut self) {
        match self.screen {
            Screen::IssueDetail => {
                if let Some(issue) = &self.current_issue
                    && issue.comments.is_none()
                {
                    let issue_id = issue.id.clone();
                    self.request(Request::IssueDetail { issue_id });
                }
            }
            Screen::ProjectDetail => {
                if !self.project_issues_loaded
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
                if !self.cycle_issues_loaded
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
        // Captured before the match moves `msg`; several arms need it.
        let page_appended = match &msg {
            Message::Issues { page: p, .. } | Message::MyIssues(p) => p.append,
            _ => false,
        };
        match msg {
            Message::Teams(teams) => {
                if let Some(default_team) = &self.default_team
                    && let Some(idx) = teams
                        .iter()
                        .position(|t| t.name == *default_team || t.key == *default_team)
                {
                    self.selected_team_index = idx;
                }
                self.teams = teams;
                self.nav = Nav::Team(self.selected_team_index, TeamSection::Issues);
                if let Some(team_id) = self.team_id() {
                    self.request(Request::TeamContext {
                        team_id: team_id.clone(),
                    });
                    self.request(Request::Issues {
                        team_id,
                        after: None,
                        preset: self.presets[IssueSource::Team.slot()],
                    });
                }
            }
            Message::Viewer(id) => {
                self.viewer_id = Some(id);
                if self.nav == Nav::MyIssues {
                    self.reload_current_tab();
                }
            }
            Message::TeamContext { states, members } => {
                self.workflow_states = states;
                self.team_members = members;
            }
            Message::Issues { preset, page } => {
                // A page for a preset the user has since left would mix, say,
                // backlog issues into the Active list.
                if preset != self.presets[IssueSource::Team.slot()] {
                    return;
                }
                let keep = self.selected_issue_id();
                Self::merge_issues(&mut self.issues, page, &mut self.page_info);
                if !page_appended {
                    self.restore_issue_selection(keep.as_deref());
                }
                self.clear_status();
            }
            Message::MyIssues(page) => {
                Self::merge_issues(&mut self.my_issues, page, &mut self.my_issues_page_info);
                if !page_appended {
                    self.selected_my_issue_index = 0;
                }
                self.my_issues_loaded = true;
                self.clear_status();
            }
            Message::SearchResults { term, issues } => {
                self.nav = Nav::Team(self.selected_team_index, TeamSection::Issues);
                self.screen = Screen::IssueList;
                self.issues = issues;
                self.page_info = PageInfo::default();
                self.filters.clear();
                // Search answers "where is it", done or not; the Active slice
                // would quietly hide half the matches. Set without refetching,
                // which would replace the results with the team's list.
                self.presets[IssueSource::Team.slot()] = Preset::All;
                self.search.clear();
                self.selected_issue_index = 0;
                self.global_search = Some(term);
            }
            Message::ViewProjects { view_id, page } => {
                let append = page.append;
                Self::merge(
                    &mut self.view_projects,
                    page,
                    &mut self.view_projects_page_info,
                );
                if !append {
                    self.selected_view_project_index = 0;
                }
                self.loaded_view_projects_id = Some(view_id);
                self.clear_status();
            }
            Message::Projects(page) => {
                Self::merge(&mut self.projects, page, &mut self.projects_page_info);
                self.clamp(Field::Projects);
                self.projects_loaded = true;
                self.clear_status();
            }
            Message::Cycles(page) => {
                Self::merge(&mut self.cycles, page, &mut self.cycles_page_info);
                self.clamp(Field::Cycles);
                self.cycles_loaded = true;
                self.clear_status();
            }
            Message::CustomViews(views) => {
                // Keep the open view pointing at the same view across a refetch;
                // the index alone would silently swap which one is on screen.
                let open = match self.nav {
                    Nav::View(i) => self.custom_views.get(i).map(|v| v.id.clone()),
                    _ => None,
                };
                let mut views = views;
                // Linear's Views page lists personal views above workspace ones.
                views.sort_by(|a, b| {
                    a.shared
                        .cmp(&b.shared)
                        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
                self.custom_views = views;
                self.views_loaded = true;
                if let Some(id) = open {
                    match self.custom_views.iter().position(|v| v.id == id) {
                        Some(i) => self.nav = Nav::View(i),
                        None => self.activate(Nav::Views),
                    }
                }
                self.selected_view_index = self
                    .selected_view_index
                    .min(self.listed_views().len().saturating_sub(1));
            }
            Message::Favorites(mut favorites) => {
                favorites.sort_by(|a, b| a.sort_order.total_cmp(&b.sort_order));
                self.favorites = favorites;
            }
            Message::ViewIssues { view_id, page } => {
                let append = page.append;
                Self::merge_issues(&mut self.view_issues, page, &mut self.view_issues_page_info);
                if !append {
                    self.selected_view_issue_index = 0;
                }
                self.loaded_view_id = Some(view_id);
                self.clear_status();
            }
            Message::IssueDetail(issue) => {
                // Ignore a detail response for an issue the user already navigated away from.
                if self
                    .current_issue
                    .as_ref()
                    .is_some_and(|c| c.id == issue.id)
                {
                    self.current_issue = Some(*issue);
                }
            }
            Message::ProjectIssues(page) => {
                Self::merge_issues(
                    &mut self.project_issues,
                    page,
                    &mut self.project_issues_page_info,
                );
                self.clamp(Field::ProjectIssues);
                self.project_issues_loaded = true;
            }
            Message::CycleIssues(page) => {
                Self::merge_issues(
                    &mut self.cycle_issues,
                    page,
                    &mut self.cycle_issues_page_info,
                );
                self.clamp(Field::CycleIssues);
                self.cycle_issues_loaded = true;
            }
            Message::IssueCreated(issue) => {
                self.set_status(format!("Created {}", issue.identifier));
                // Show it immediately rather than waiting for a refetch, with
                // the cursor on it — found by id, since grouping decides where
                // in the list it lands.
                let id = issue.id.clone();
                self.issues.insert(0, *issue);
                if self.issue_source() == IssueSource::Team && self.screen == Screen::IssueList {
                    self.restore_issue_selection(Some(&id));
                }
            }
            Message::Mutated(what) => {
                self.set_status(what);
                // A posted comment clears the cached thread; pull it back in.
                self.queue_detail_fetches();
            }
            Message::Error(err) => {
                // A failed page must be retryable.
                self.prefetched.clear();
                self.set_error(err);
            }
        }
    }

    /// [`Self::merge`] for issue lists, dropping any issue already held.
    ///
    /// Pages are ordered by `updatedAt`, so an issue touched between two page
    /// fetches moves and can come back on both; without this it would be
    /// listed twice.
    fn merge_issues(dst: &mut Vec<Issue>, page: Page<Issue>, info: &mut PageInfo) {
        let append = page.append;
        Self::merge(dst, page, info);
        if append {
            let mut seen = HashSet::new();
            dst.retain(|issue| seen.insert(issue.id.clone()));
        }
    }

    /// Fold one page into a list, either replacing it or extending it.
    fn merge<T>(dst: &mut Vec<T>, page: Page<T>, info: &mut PageInfo) {
        let Page {
            mut items,
            page_info,
            append,
        } = page;
        if append {
            dst.append(&mut items);
        } else {
            *dst = items;
        }
        *info = page_info;
    }

    /// Keep a selection index inside its (possibly shrunken) list.
    fn clamp(&mut self, field: Field) {
        let (len, index) = match field {
            Field::Projects => (self.projects.len(), &mut self.selected_project_index),
            Field::Cycles => (self.cycles.len(), &mut self.selected_cycle_index),
            Field::ProjectIssues => (
                self.project_issues.len(),
                &mut self.selected_project_issue_index,
            ),
            Field::CycleIssues => (
                self.cycle_issues.len(),
                &mut self.selected_cycle_issue_index,
            ),
        };
        *index = (*index).min(len.saturating_sub(1));
    }

    // -------------------------------------------------------------- selections

    pub fn current_team(&self) -> Option<&Team> {
        self.teams.get(self.selected_team_index)
    }

    fn selected_issue_id(&self) -> Option<String> {
        self.visible_issues()
            .get(self.selected_index())
            .map(|i| i.id.clone())
    }

    /// Put the cursor back on the issue it was on, by identity.
    ///
    /// A refetch reorders the list — `updatedAt` moves the moment anyone
    /// touches an issue — so restoring by row index would quietly select a
    /// different issue than the one the user was looking at.
    fn restore_issue_selection(&mut self, id: Option<&str>) {
        let index = id
            .and_then(|id| self.visible_issues().iter().position(|i| i.id == id))
            .unwrap_or(0);
        *self.selected_index_mut() = index;
    }

    // ------------------------------------------------- the active issue list

    /// Which of the five issue lists the current screen is showing.
    pub fn issue_source(&self) -> IssueSource {
        match self.screen {
            Screen::ProjectDetail => IssueSource::Project,
            Screen::CycleDetail => IssueSource::Cycle,
            _ => match self.nav {
                Nav::MyIssues => IssueSource::My,
                Nav::View(_) => IssueSource::View,
                _ => IssueSource::Team,
            },
        }
    }

    /// The unfiltered contents of the active list.
    fn source_issues(&self) -> &[Issue] {
        match self.issue_source() {
            IssueSource::Team => &self.issues,
            IssueSource::My => &self.my_issues,
            IssueSource::View => &self.view_issues,
            IssueSource::Project => &self.project_issues,
            IssueSource::Cycle => &self.cycle_issues,
        }
    }

    /// Cursor position within the active list, as an index into
    /// [`Self::visible_issues`].
    pub fn selected_index(&self) -> usize {
        match self.issue_source() {
            IssueSource::Team => self.selected_issue_index,
            IssueSource::My => self.selected_my_issue_index,
            IssueSource::View => self.selected_view_issue_index,
            IssueSource::Project => self.selected_project_issue_index,
            IssueSource::Cycle => self.selected_cycle_issue_index,
        }
    }

    fn selected_index_mut(&mut self) -> &mut usize {
        match self.issue_source() {
            IssueSource::Team => &mut self.selected_issue_index,
            IssueSource::My => &mut self.selected_my_issue_index,
            IssueSource::View => &mut self.selected_view_issue_index,
            IssueSource::Project => &mut self.selected_project_issue_index,
            IssueSource::Cycle => &mut self.selected_cycle_issue_index,
        }
    }

    pub fn set_selected_index(&mut self, index: usize) {
        let len = self.visible_issues().len();
        *self.selected_index_mut() = index.min(len.saturating_sub(1));
    }

    /// The [`TableState`] whose scroll offset belongs to the active list.
    pub fn active_table_state(&mut self) -> &mut TableState {
        match self.issue_source() {
            IssueSource::Team => &mut self.tables.issues,
            IssueSource::My => &mut self.tables.my_issues,
            IssueSource::View => &mut self.tables.view_issues,
            IssueSource::Project => &mut self.tables.project_issues,
            IssueSource::Cycle => &mut self.tables.cycle_issues,
        }
    }

    /// Get the issue currently focused (selected in list, or being viewed in detail).
    pub fn focused_issue(&self) -> Option<&Issue> {
        match self.screen {
            Screen::IssueDetail => self.current_issue.as_ref(),
            Screen::ViewList | Screen::ProjectList | Screen::CycleList => None,
            _ => self.visible_issues().get(self.selected_index()).copied(),
        }
    }

    fn matches(issue: &Issue, lowercase_query: &str) -> bool {
        issue.title.to_lowercase().contains(lowercase_query)
            || issue.identifier.to_lowercase().contains(lowercase_query)
    }

    /// Whether an issue survives the search box and the status/priority filters.
    fn admitted(&self, issue: &Issue) -> bool {
        if !self.preset().admits(issue) {
            return false;
        }
        if !self.search.is_empty() {
            let query = self.search.value.to_lowercase();
            if !Self::matches(issue, &query) {
                return false;
            }
        }
        if let Some(status) = &self.filters.status {
            match &issue.state {
                Some(state) if &state.name == status => {}
                _ => return false,
            }
        }
        if let Some(pri) = self.filters.priority
            && issue.priority != pri
        {
            return false;
        }
        true
    }

    /// The active list, filtered and stacked into its groups.
    ///
    /// Sections are the single source of truth for both what is on screen and
    /// where the cursor can land, so a collapsed group cannot leave the cursor
    /// pointing at an issue nobody can see.
    pub fn sections(&self) -> Vec<Section<'_>> {
        group(
            self.source_issues().iter().filter(|i| self.admitted(i)),
            self.group_by,
            &self.collapsed_groups,
            &self.theme,
        )
    }

    /// Every issue the cursor can currently reach, in the order it is drawn.
    pub fn visible_issues(&self) -> Vec<&Issue> {
        self.sections()
            .into_iter()
            .filter(|s| !s.collapsed)
            .flat_map(|s| s.issues.into_iter().map(|(issue, _)| issue))
            .collect()
    }

    /// The display rows of the active list: group headers interleaved with the
    /// issues under them, addressed by their position in [`Self::visible_issues`].
    pub fn list_layout(&self) -> Vec<ListRow> {
        let mut rows = Vec::new();
        let mut ordinal = 0;
        for section in self.sections() {
            if self.group_by != GroupBy::None {
                rows.push(ListRow::Group {
                    key: section.key.clone(),
                    label: section.label.clone(),
                    color: section.color,
                    glyph: section.glyph,
                    count: section.issues.len(),
                    collapsed: section.collapsed,
                });
            }
            if section.collapsed {
                continue;
            }
            for (_, depth) in &section.issues {
                rows.push(ListRow::Issue {
                    ordinal,
                    depth: *depth,
                });
                ordinal += 1;
            }
        }
        rows
    }

    /// The group the cursor currently sits in, if the list is grouped.
    fn selected_group_key(&self) -> Option<String> {
        let target = self.selected_index();
        let mut ordinal = 0;
        for section in self.sections() {
            if section.collapsed {
                continue;
            }
            if target < ordinal + section.issues.len() {
                return Some(section.key);
            }
            ordinal += section.issues.len();
        }
        None
    }

    /// Fold or unfold the group the cursor is in.
    pub fn toggle_selected_group(&mut self) {
        let Some(key) = self.selected_group_key() else {
            return;
        };
        self.toggle_group(&key);
    }

    pub fn toggle_group(&mut self, key: &str) {
        if !self.collapsed_groups.remove(key) {
            self.collapsed_groups.insert(key.to_string());
        }
        // Folding a group can put the cursor past the end of what is left.
        let len = self.visible_issues().len();
        *self.selected_index_mut() = self.selected_index().min(len.saturating_sub(1));
    }

    /// Fold every group, or unfold them all if none is folded.
    pub fn toggle_all_groups(&mut self) {
        let keys: Vec<String> = self.sections().into_iter().map(|s| s.key).collect();
        let any_open = keys.iter().any(|k| !self.collapsed_groups.contains(k));
        if any_open {
            self.collapsed_groups.extend(keys);
        } else {
            for key in keys {
                self.collapsed_groups.remove(&key);
            }
        }
        let len = self.visible_issues().len();
        *self.selected_index_mut() = self.selected_index().min(len.saturating_sub(1));
    }

    pub fn cycle_group_by(&mut self) {
        self.group_by = self.group_by.next();
        self.collapsed_groups.clear();
        *self.selected_index_mut() = 0;
        self.set_status(format!("Grouped by {}", self.group_by.label()));
    }

    /// The preset chip selected on the list currently on screen.
    pub fn preset(&self) -> Preset {
        self.presets[self.issue_source().slot()]
    }

    pub fn set_preset(&mut self, preset: Preset) {
        if self.preset() == preset {
            return;
        }
        let source = self.issue_source();
        self.presets[source.slot()] = preset;
        *self.selected_index_mut() = 0;
        // A team's list is sliced on the server; the other lists already hold
        // everything they can show and only filter locally. Workspace search
        // results are not refetched either — that would throw them away.
        if source == IssueSource::Team
            && self.global_search.is_none()
            && let Some(team_id) = self.team_id()
        {
            self.request(Request::Issues {
                team_id,
                after: None,
                preset,
            });
        }
    }

    /// Step to the next preset chip, as clicking along the row would.
    pub fn cycle_preset(&mut self) {
        let presets = Preset::all();
        let next = presets
            .iter()
            .position(|p| *p == self.preset())
            .map(|i| (i + 1) % presets.len())
            .unwrap_or(0);
        self.set_preset(presets[next]);
    }

    /// Ask for the next page once the cursor nears the end of the active list.
    fn maybe_prefetch(&mut self) {
        let source = self.issue_source();
        if self.selected_index() + PREFETCH_MARGIN < self.source_issues().len() {
            return;
        }
        let info = match source {
            IssueSource::Team => &self.page_info,
            IssueSource::My => &self.my_issues_page_info,
            IssueSource::View => &self.view_issues_page_info,
            IssueSource::Project => &self.project_issues_page_info,
            IssueSource::Cycle => &self.cycle_issues_page_info,
        };
        if !info.has_next_page {
            return;
        }
        let Some(cursor) = info.end_cursor.clone() else {
            return;
        };
        let after = Some(cursor.clone());
        let request = match source {
            IssueSource::Team => self.team_id().map(|team_id| Request::Issues {
                team_id,
                after,
                preset: self.presets[IssueSource::Team.slot()],
            }),
            IssueSource::My => self
                .viewer_id
                .clone()
                .map(|user_id| Request::MyIssues { user_id, after }),
            IssueSource::View => self
                .loaded_view_id
                .clone()
                .map(|view_id| Request::ViewIssues { view_id, after }),
            IssueSource::Project => self
                .current_project
                .as_ref()
                .map(|p| Request::ProjectIssues {
                    project_id: p.id.clone(),
                    after,
                }),
            IssueSource::Cycle => self.current_cycle.as_ref().map(|c| Request::CycleIssues {
                cycle_id: c.id.clone(),
                after,
            }),
        };
        if let Some(request) = request
            && self.prefetched.insert(cursor)
        {
            self.request(request);
        }
    }

    fn maybe_prefetch_projects(&mut self) {
        let in_view = self.in_project_view();
        let info = if in_view {
            &self.view_projects_page_info
        } else {
            &self.projects_page_info
        };
        if self.project_cursor() + PREFETCH_MARGIN < self.project_rows().len()
            || !info.has_next_page
        {
            return;
        }
        let Some(cursor) = info.end_cursor.clone() else {
            return;
        };
        let after = Some(cursor.clone());
        let request = if in_view {
            self.loaded_view_projects_id
                .clone()
                .map(|view_id| Request::ViewProjects { view_id, after })
        } else {
            self.team_id()
                .map(|team_id| Request::Projects { team_id, after })
        };
        if let Some(request) = request
            && self.prefetched.insert(cursor)
        {
            self.request(request);
        }
    }

    // ------------------------------------------------------------ project lists

    /// Whether the project list on screen is a saved project view's rather
    /// than the team's.
    fn in_project_view(&self) -> bool {
        matches!(self.nav, Nav::View(i) if self.custom_views.get(i).is_some_and(|v| ViewKind::of(v) == ViewKind::Projects))
    }

    /// The projects the project list is showing.
    pub fn project_rows(&self) -> &[Project] {
        if self.in_project_view() {
            &self.view_projects
        } else {
            &self.projects
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

    /// Scroll state of the project list on screen.
    pub fn project_table(&mut self) -> &mut TableState {
        if self.in_project_view() {
            &mut self.tables.view_projects
        } else {
            &mut self.tables.projects
        }
    }

    // ----------------------------------------------------------- views pages

    /// The team whose views a Views page shows, or `None` for the workspace
    /// page.
    fn views_scope(&self) -> Option<&str> {
        match self.nav {
            Nav::Team(index, TeamSection::Views) => self.teams.get(index).map(|t| t.id.as_str()),
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
        self.custom_views
            .iter()
            .enumerate()
            .filter(|(_, v)| v.team.as_ref().map(|t| t.id.as_str()) == scope)
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
        if self.selected_cycle_index + PREFETCH_MARGIN < self.cycles.len()
            || !self.cycles_page_info.has_next_page
        {
            return;
        }
        if let (Some(team_id), Some(cursor)) =
            (self.team_id(), self.cycles_page_info.end_cursor.clone())
            && self.prefetched.insert(cursor.clone())
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
        if self.popup_index >= self.teams.len() {
            return;
        }
        let section = match self.nav {
            Nav::Team(_, section) => section,
            _ => TeamSection::Issues,
        };
        self.activate(Nav::Team(self.popup_index, section));
    }

    pub fn open_filter(&mut self) {
        self.popup = Popup::Filter;
        self.popup_index = 0;
        self.filter_kind = FilterKind::Status;
    }

    pub fn open_status_change(&mut self) {
        if self.focused_issue().is_some() {
            // Start on the issue's current state so Enter is a no-op, not a surprise.
            let current = self
                .focused_issue()
                .and_then(|i| i.state.as_ref())
                .map(|s| s.id.clone());
            self.popup_index = current
                .and_then(|id| self.workflow_states.iter().position(|s| s.id == id))
                .unwrap_or(0);
            self.popup = Popup::StatusChange;
        }
    }

    pub fn open_priority_change(&mut self) {
        if let Some(issue) = self.focused_issue() {
            self.popup_index = issue.priority.as_index();
            self.popup = Popup::PriorityChange;
        }
    }

    pub fn open_assignee_change(&mut self) {
        if self.focused_issue().is_some() {
            let current = self
                .focused_issue()
                .and_then(|i| i.assignee.as_ref())
                .map(|u| u.id.clone());
            self.popup_index = current
                .and_then(|id| self.team_members.iter().position(|u| u.id == id))
                .map(|i| i + 1)
                .unwrap_or(0);
            self.popup = Popup::AssigneeChange;
        }
    }

    /// Linear's `I` — assign the focused issue to the current user.
    pub fn assign_to_me(&mut self) {
        let Some(viewer_id) = self.viewer_id.clone() else {
            self.set_status("Current user is not loaded yet");
            return;
        };
        let Some(issue_id) = self.focused_issue().map(|i| i.id.clone()) else {
            return;
        };
        let me = self
            .team_members
            .iter()
            .find(|u| u.id == viewer_id)
            .cloned();
        self.patch_issue(&issue_id, |i| i.assignee = me.clone());
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
        self.patch_issue(&issue_id, |i| {
            i.priority = priority;
            i.priority_label = Some(priority.label().to_string());
        });
        self.request(Request::UpdatePriority {
            issue_id,
            priority: priority.as_u8(),
        });
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
            if let Some(current) = &mut self.current_issue
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

    /// Apply `f` to every copy of the issue we hold, so the UI updates without a refetch.
    fn patch_issue(&mut self, issue_id: &str, f: impl Fn(&mut Issue)) {
        let lists = [
            &mut self.issues,
            &mut self.my_issues,
            &mut self.project_issues,
            &mut self.cycle_issues,
        ];
        for list in lists {
            for issue in list.iter_mut().filter(|i| i.id == issue_id) {
                f(issue);
            }
        }
        if let Some(current) = &mut self.current_issue
            && current.id == issue_id
        {
            f(current);
        }
    }

    pub fn apply_status_selection(&mut self) {
        if let Some(issue) = self.focused_issue()
            && let Some(state) = self.workflow_states.get(self.popup_index)
        {
            let issue_id = issue.id.clone();
            let state = state.clone();
            let state_id = state.id.clone();
            self.patch_issue(&issue_id, |i| i.state = Some(state.clone()));
            self.request(Request::UpdateStatus { issue_id, state_id });
        }
        self.popup = Popup::None;
    }

    pub fn apply_priority_selection(&mut self) {
        if let Some(issue) = self.focused_issue() {
            let issue_id = issue.id.clone();
            let priority = Priority::from_index(self.popup_index);
            self.patch_issue(&issue_id, |i| {
                i.priority = priority;
                i.priority_label = Some(priority.label().to_string());
            });
            self.request(Request::UpdatePriority {
                issue_id,
                priority: priority.as_u8(),
            });
        }
        self.popup = Popup::None;
    }

    pub fn apply_assignee_selection(&mut self) {
        if let Some(issue) = self.focused_issue() {
            let issue_id = issue.id.clone();
            let assignee = if self.popup_index == 0 {
                None // Unassign
            } else {
                self.team_members.get(self.popup_index - 1).cloned()
            };
            let assignee_id = assignee.as_ref().map(|u| u.id.clone());
            self.patch_issue(&issue_id, |i| i.assignee = assignee.clone());
            self.request(Request::UpdateAssignee {
                issue_id,
                assignee_id,
            });
        }
        self.popup = Popup::None;
    }

    // ------------------------------------------------------------------ popups

    pub fn close_popup(&mut self) {
        self.popup = Popup::None;
    }

    pub fn popup_next(&mut self) {
        let max = self.popup_list_len();
        if max > 0 && self.popup_index < max - 1 {
            self.popup_index += 1;
        }
    }

    pub fn popup_prev(&mut self) {
        if self.popup_index > 0 {
            self.popup_index -= 1;
        }
    }

    pub fn popup_list_len(&self) -> usize {
        match self.popup {
            Popup::TeamSelect => self.teams.len(),
            Popup::Filter => match self.filter_kind {
                FilterKind::Status => self.workflow_states.len() + 1,
                FilterKind::Priority => 6,
            },
            Popup::StatusChange => self.workflow_states.len(),
            Popup::PriorityChange => 5, // None, Urgent, High, Medium, Low
            Popup::AssigneeChange => self.team_members.len() + 1, // +1 for Unassign
            Popup::None => 0,
        }
    }

    pub fn apply_filter_selection(&mut self) {
        match self.filter_kind {
            FilterKind::Status => {
                if self.popup_index == 0 {
                    self.filters.status = None;
                } else if let Some(state) = self.workflow_states.get(self.popup_index - 1) {
                    self.filters.status = Some(state.name.clone());
                }
                self.filter_kind = FilterKind::Priority;
                self.popup_index = 0;
            }
            FilterKind::Priority => {
                self.filters.priority = match self.popup_index {
                    0 => None,
                    n => Some(Priority::from_index(n)),
                };
                self.popup = Popup::None;
                self.selected_issue_index = 0;
            }
        }
    }

    pub fn clear_filters(&mut self) {
        self.filters.clear();
        self.selected_issue_index = 0;
    }

    // ------------------------------------------------------------------ scroll

    /// Largest scroll offset that still shows content.
    fn max_detail_scroll(&self) -> u16 {
        self.detail_lines.saturating_sub(self.detail_viewport)
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
        (self.list_viewport / 2).max(1) as isize
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
                Self::nav_by(self.cycles.len(), &mut self.selected_cycle_index, delta);
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
                self.maybe_prefetch();
            }
        }
    }

    /// Step to the neighbouring issue without leaving the detail view — the
    /// `↓`/`↑` pair Linear puts next to the "6 / 203" counter.
    pub fn step_issue(&mut self, delta: isize) {
        if self.screen != Screen::IssueDetail {
            return;
        }
        let Some(position) = self.detail_position() else {
            return;
        };
        let next = (position.0 as isize + delta).clamp(0, position.1 as isize - 1) as usize;
        if next == position.0 {
            return;
        }
        let Some(issue) = self.visible_issues().get(next).copied().cloned() else {
            return;
        };
        *self.selected_index_mut() = next;
        self.maybe_prefetch();
        let ret = self.detail_return;
        self.open_issue_from_list(&issue);
        self.detail_return = ret;
    }

    /// Where the open issue sits in the list it came from: `(index, total)`.
    pub fn detail_position(&self) -> Option<(usize, usize)> {
        let current = self.current_issue.as_ref()?;
        let issues = self.visible_issues();
        let index = issues.iter().position(|i| i.id == current.id)?;
        Some((index, issues.len()))
    }

    /// Drop the cached issue detail and fetch it again.
    pub fn refresh_detail(&mut self) {
        if let Some(issue) = &mut self.current_issue {
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

    /// Hand the current query to Linear's workspace-wide search.
    pub fn search_workspace(&mut self) {
        if self.search.is_empty() {
            return;
        }
        let term = self.search.value.clone();
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
        self.search.clear();
        self.selected_issue_index = 0;
        self.selected_my_issue_index = 0;
        self.selected_view_issue_index = 0;
        // Leaving a workspace search returns the list to the team's own issues.
        if self.global_search.take().is_some() {
            self.presets[IssueSource::Team.slot()] = Preset::Active;
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
        let priority = form.priority.as_u8();
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
            Popup::Filter => self.apply_filter_selection(),
            Popup::StatusChange => self.apply_status_selection(),
            Popup::PriorityChange => self.apply_priority_selection(),
            Popup::AssigneeChange => self.apply_assignee_selection(),
            Popup::None => {}
        }
    }

    /// A left click at terminal cell `(x, y)`.
    ///
    /// Hit-testing runs against the rows and areas the last frame recorded, so
    /// a click always lands on what the user actually saw. Clicking a row
    /// selects it; clicking the selected row again opens it — the terminal's
    /// stand-in for a double-click, which crossterm cannot report.
    pub fn click(&mut self, x: u16, y: u16) {
        if self.error_popup.is_some() {
            self.dismiss_error();
            return;
        }
        if self.show_help {
            self.show_help = false;
            return;
        }
        if self.popup != Popup::None {
            if contains(self.popup_area, x, y) {
                let index = self.popup_offset + (y - self.popup_area.y) as usize;
                if index < self.popup_list_len() {
                    self.popup_index = index;
                    self.apply_popup();
                }
            } else {
                self.close_popup();
            }
            return;
        }
        if self.input_mode != InputMode::Normal {
            return;
        }

        if contains(self.sidebar_area, x, y) {
            let index = self.sidebar_offset + (y - self.sidebar_area.y) as usize;
            let Some(SidebarRow::Item(item)) = self.sidebar_rows.get(index).cloned() else {
                return;
            };
            self.sidebar_index = index;
            self.run_sidebar_action(item.action);
            return;
        }

        if let Some((_, chip)) = self.chip_areas.iter().find(|(r, _)| contains(*r, x, y)) {
            let chip = *chip;
            self.sidebar_focus = false;
            match chip {
                Chip::Preset(preset) => self.set_preset(preset),
                Chip::ViewKind(kind) => self.set_view_kind(kind),
            }
            return;
        }

        if !contains(self.list_area, x, y) {
            return;
        }
        self.sidebar_focus = false;
        let row = (y - self.list_area.y) as usize;
        match self.screen {
            Screen::IssueList | Screen::ProjectDetail | Screen::CycleDetail => {
                match self.list_rows.get(row).cloned() {
                    Some(ListRow::Group { key, .. }) => self.toggle_group(&key),
                    Some(ListRow::Issue { ordinal, .. }) => {
                        if ordinal == self.selected_index() {
                            self.open_issue_detail();
                        } else {
                            self.set_selected_index(ordinal);
                            self.maybe_prefetch();
                        }
                    }
                    None => {}
                }
            }
            Screen::ProjectList | Screen::CycleList | Screen::ViewList => {
                let Some(Some(target)) = self.row_targets.get(row).copied() else {
                    return;
                };
                let current = match self.screen {
                    Screen::ProjectList => self.project_cursor_mut(),
                    Screen::CycleList => &mut self.selected_cycle_index,
                    _ => &mut self.selected_view_index,
                };
                if *current != target {
                    *current = target;
                    return;
                }
                match self.screen {
                    Screen::ProjectList => self.open_project_detail(),
                    Screen::CycleList => self.open_cycle_detail(),
                    _ => self.open_selected_view(),
                }
            }
            Screen::IssueDetail => {}
        }
    }

    /// The mouse wheel at `(x, y)`: scroll whatever is under the pointer.
    pub fn wheel(&mut self, x: u16, y: u16, delta: i16) {
        if self.popup != Popup::None {
            if delta > 0 {
                self.popup_next();
            } else {
                self.popup_prev();
            }
        } else if contains(self.sidebar_area, x, y) {
            let max = self
                .sidebar_rows
                .len()
                .saturating_sub(self.sidebar_area.height as usize);
            self.sidebar_offset =
                (self.sidebar_offset as isize + delta as isize).clamp(0, max as isize) as usize;
        } else if self.show_help {
            self.help_scroll = (self.help_scroll as i32 + delta as i32).max(0) as u16;
        } else {
            self.scroll_list_or_detail(delta);
        }
    }

    // ------------------------------------------------------------------ status

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
        self.current_issue = Some(issue.clone());
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
            && index < self.teams.len()
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
            Nav::MyIssues => self.my_issues_loaded,
            Nav::Views | Nav::Team(_, TeamSection::Views) => self.views_loaded,
            Nav::View(index) => self.custom_views.get(index).is_some_and(|view| {
                let loaded = match ViewKind::of(view) {
                    ViewKind::Issues => &self.loaded_view_id,
                    ViewKind::Projects => &self.loaded_view_projects_id,
                };
                loaded.as_deref() == Some(view.id.as_str())
            }),
            Nav::Team(_, TeamSection::Issues) => !self.issues.is_empty(),
            Nav::Team(_, TeamSection::Projects) => self.projects_loaded,
            Nav::Team(_, TeamSection::Cycles) => self.cycles_loaded,
            Nav::Favorite(_) => true,
        };
        if let Nav::View(_) = nav
            && !cached
        {
            // A different view's rows are still in the list; clear them so
            // the old results are not briefly attributed to the new view.
            self.view_issues.clear();
            self.view_issues_page_info = PageInfo::default();
            self.selected_view_issue_index = 0;
            self.view_projects.clear();
            self.view_projects_page_info = PageInfo::default();
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
        self.issues.clear();
        self.selected_issue_index = 0;
        // The old team's projects and cycles would otherwise sit on screen,
        // under the new team's name, until the refetch lands.
        self.projects.clear();
        self.cycles.clear();
        self.selected_project_index = 0;
        self.selected_cycle_index = 0;
        self.filters.clear();
        self.invalidate_tab_caches();
        if let Some(team_id) = self.team_id() {
            self.request(Request::TeamContext { team_id });
        }
    }

    // ----------------------------------------------------------- sidebar

    /// Build the sidebar's rows from the current state.
    ///
    /// Recomputed rather than cached because every input to it — teams, views,
    /// favorites, folded folders — can change under a message arriving from
    /// the network.
    pub fn sidebar_layout(&self) -> Vec<SidebarRow> {
        let item =
            |label: &str, icon: &'static str, depth: u8, action: SidebarAction, tone: Tone| {
                SidebarRow::Item(SidebarItem {
                    label: label.to_string(),
                    icon,
                    color: None,
                    depth,
                    action,
                    expanded: None,
                    trailing: None,
                    tone,
                })
            };

        let mut rows = vec![
            item(
                "My Issues",
                "\u{25c9}",
                0,
                SidebarAction::Go(Nav::MyIssues),
                Tone::Normal,
            ),
            item(
                "Views",
                "\u{2261}",
                0,
                SidebarAction::Go(Nav::Views),
                Tone::Subtle,
            ),
        ];

        if !self.favorites.is_empty() {
            rows.push(SidebarRow::Gap);
            rows.push(SidebarRow::Header("Favorites".into()));
            // Top-level entries in order, each folder followed by its contents.
            for (index, fav) in self.favorites.iter().enumerate() {
                if fav.parent.is_some() {
                    continue;
                }
                rows.push(self.favorite_row(index, 0));
                if fav.is_folder() && !self.collapsed_folders.contains(&fav.id) {
                    for (child, _) in self
                        .favorites
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| f.parent.as_ref().is_some_and(|p| p.id == fav.id))
                    {
                        rows.push(self.favorite_row(child, 1));
                    }
                }
            }
        }

        // The team is a switcher: one row naming the current team, and only
        // that team's pages beneath it.
        if let Some(team) = self.current_team() {
            let index = self.selected_team_index;
            rows.push(SidebarRow::Gap);
            rows.push(SidebarRow::Header("Team".into()));
            rows.push(SidebarRow::Item(SidebarItem {
                label: team.name.clone(),
                icon: "\u{25cf}",
                color: team.color.as_deref().and_then(hex_color),
                depth: 0,
                action: SidebarAction::SwitchTeam,
                expanded: None,
                trailing: (self.teams.len() > 1).then(|| "t \u{21c5}".to_string()),
                tone: Tone::Strong,
            }));
            rows.push(item(
                "Issues",
                "\u{2263}",
                1,
                SidebarAction::Go(Nav::Team(index, TeamSection::Issues)),
                Tone::Normal,
            ));
            // Teams that do not run cycles have no Cycles page in Linear
            // either, so showing an empty one would be a dead end.
            if team.cycles_enabled {
                rows.push(item(
                    "Cycles",
                    "\u{25d4}",
                    1,
                    SidebarAction::Go(Nav::Team(index, TeamSection::Cycles)),
                    Tone::Normal,
                ));
            }
            rows.push(item(
                "Projects",
                "\u{25a3}",
                1,
                SidebarAction::Go(Nav::Team(index, TeamSection::Projects)),
                Tone::Normal,
            ));
            rows.push(item(
                "Views",
                "\u{2261}",
                1,
                SidebarAction::Go(Nav::Team(index, TeamSection::Views)),
                Tone::Normal,
            ));
        }

        rows
    }

    /// The sidebar row for one favorite.
    fn favorite_row(&self, index: usize, depth: u8) -> SidebarRow {
        let fav = &self.favorites[index];
        let color = fav
            .color
            .as_deref()
            .or_else(|| fav.project.as_ref().and_then(|p| p.color.as_deref()))
            .or_else(|| {
                fav.issue
                    .as_ref()
                    .and_then(|i| i.state.as_ref()?.color.as_deref())
            })
            .and_then(hex_color);
        let icon = match fav.kind.as_str() {
            "folder" | "document" => "\u{25a4}",
            "project" => "\u{25a3}",
            "customView" | "predefinedView" => "\u{2261}",
            "issue" => fav
                .issue
                .as_ref()
                .and_then(|i| i.state.as_ref()?.state_type)
                .unwrap_or(StateType::Unknown)
                .glyph(),
            "cycle" => "\u{25d4}",
            "label" => "\u{25cf}",
            _ => "\u{2022}",
        };
        let label = match (&fav.issue, fav.kind.as_str()) {
            (Some(issue), "issue") => format!("{} {}", issue.identifier, issue.title),
            _ => fav.label(),
        };
        SidebarRow::Item(SidebarItem {
            label,
            icon,
            color,
            depth,
            action: self.favorite_action(index),
            expanded: fav
                .is_folder()
                .then(|| !self.collapsed_folders.contains(&fav.id)),
            trailing: None,
            tone: Tone::Strong,
        })
    }

    /// Where a favorite leads. Views and team pages resolve to their own
    /// destination, so opening one lights the same row as getting there any
    /// other way.
    fn favorite_action(&self, index: usize) -> SidebarAction {
        let fav = &self.favorites[index];
        if fav.is_folder() {
            return SidebarAction::Fold(index);
        }
        if let Some(view) = &fav.custom_view
            && let Some(i) = self.custom_views.iter().position(|v| v.id == view.id)
        {
            return SidebarAction::Go(Nav::View(i));
        }
        if fav.kind == "predefinedView" {
            if fav.predefined_view_type.as_deref() == Some("myIssues") {
                return SidebarAction::Go(Nav::MyIssues);
            }
            let team = fav
                .predefined_view_team
                .as_ref()
                .and_then(|t| self.teams.iter().position(|x| x.id == t.id));
            let section = match fav.predefined_view_type.as_deref() {
                Some("issues" | "allIssues" | "activeIssues" | "backlog") => {
                    Some(TeamSection::Issues)
                }
                Some("cycles") => Some(TeamSection::Cycles),
                Some("projects") => Some(TeamSection::Projects),
                _ => None,
            };
            if let (Some(team), Some(section)) = (team, section) {
                return SidebarAction::Go(Nav::Team(team, section));
            }
        }
        SidebarAction::Go(Nav::Favorite(index))
    }

    /// Open a favorite that has no destination of its own: a project, cycle,
    /// or issue in place, and anything this client has no page for — a
    /// document, a label, a workspace-wide page — on linear.app.
    fn open_favorite(&mut self, index: usize) {
        let Some(fav) = self.favorites.get(index).cloned() else {
            return;
        };
        self.sidebar_focus = false;
        if let Some(project) = fav.project {
            self.nav = Nav::Favorite(index);
            self.open_project(project);
        } else if let Some(cycle) = fav.cycle {
            self.nav = Nav::Favorite(index);
            self.open_cycle(cycle);
        } else if let Some(issue) = fav.issue {
            // Just enough to draw the header; the detail fetch fills the rest.
            let stub = Issue {
                id: issue.id,
                identifier: issue.identifier,
                title: issue.title,
                state: issue.state,
                ..Issue::default()
            };
            self.open_issue_from_list(&stub);
            self.nav = Nav::Favorite(index);
        } else if let Some(url) = fav.url {
            self.request(Request::OpenUrl(url));
        } else {
            self.set_status("This favorite cannot be opened here");
        }
    }

    /// Indices of the sidebar rows the cursor may land on.
    fn sidebar_stops(&self) -> Vec<usize> {
        self.sidebar_rows
            .iter()
            .enumerate()
            .filter(|(_, row)| matches!(row, SidebarRow::Item(_)))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn sidebar_move(&mut self, delta: isize) {
        let stops = self.sidebar_stops();
        if stops.is_empty() {
            return;
        }
        let current = stops
            .iter()
            .position(|i| *i == self.sidebar_index)
            .unwrap_or(0);
        let mut next = current;
        Self::nav_by(stops.len(), &mut next, delta);
        self.sidebar_index = stops[next];
    }

    fn sidebar_item(&self, index: usize) -> Option<&SidebarItem> {
        match self.sidebar_rows.get(index) {
            Some(SidebarRow::Item(item)) => Some(item),
            _ => None,
        }
    }

    /// Enter on the sidebar.
    pub fn sidebar_activate(&mut self) {
        if let Some(action) = self.sidebar_item(self.sidebar_index).map(|i| i.action) {
            self.run_sidebar_action(action);
        }
    }

    fn run_sidebar_action(&mut self, action: SidebarAction) {
        match action {
            SidebarAction::Go(nav) => self.activate(nav),
            SidebarAction::Fold(index) => self.toggle_folder(index),
            SidebarAction::SwitchTeam => self.open_team_select(),
        }
    }

    /// `h`/`l` on the sidebar: fold or unfold the folder under the cursor.
    pub fn sidebar_toggle(&mut self) {
        if let Some(SidebarAction::Fold(index)) =
            self.sidebar_item(self.sidebar_index).map(|i| i.action)
        {
            self.toggle_folder(index);
        }
    }

    pub fn toggle_folder(&mut self, index: usize) {
        let Some(id) = self.favorites.get(index).map(|f| f.id.clone()) else {
            return;
        };
        if !self.collapsed_folders.remove(&id) {
            self.collapsed_folders.insert(id);
        }
        self.sidebar_rows = self.sidebar_layout();
    }

    /// Move focus between the sidebar and the content pane.
    pub fn focus_sidebar(&mut self, focused: bool) {
        if focused && !self.sidebar_visible {
            return;
        }
        self.sidebar_focus = focused;
        if focused {
            // Start on the row matching where the content pane already is, so
            // the sidebar opens pointing at you rather than at the top.
            if let Some(index) = self.sidebar_rows.iter().position(
                |row| matches!(row, SidebarRow::Item(item) if item.nav() == Some(self.nav)),
            ) {
                self.sidebar_index = index;
            }
        }
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
        if !self.sidebar_visible {
            self.sidebar_focus = false;
        }
    }

    pub fn invalidate_tab_caches(&mut self) {
        self.my_issues_loaded = false;
        self.loaded_view_id = None;
        self.projects_loaded = false;
        self.cycles_loaded = false;
        self.project_issues_loaded = false;
        self.cycle_issues_loaded = false;
    }

    // Project navigation
    pub fn open_project_detail(&mut self) {
        if let Some(project) = self.project_rows().get(self.project_cursor()).cloned() {
            self.open_project(project);
        }
    }

    fn open_project(&mut self, project: Project) {
        self.current_project = Some(project);
        self.project_issues.clear();
        self.project_issues_loaded = false;
        self.selected_project_issue_index = 0;
        self.screen = Screen::ProjectDetail;
        self.queue_detail_fetches();
    }

    // Cycle navigation
    pub fn open_cycle_detail(&mut self) {
        if let Some(cycle) = self.cycles.get(self.selected_cycle_index).cloned() {
            self.open_cycle(cycle);
        }
    }

    fn open_cycle(&mut self, cycle: Cycle) {
        self.current_cycle = Some(cycle);
        self.cycle_issues.clear();
        self.cycle_issues_loaded = false;
        self.selected_cycle_issue_index = 0;
        self.screen = Screen::CycleDetail;
        self.queue_detail_fetches();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Page;

    fn issue(id: &str, identifier: &str, title: &str) -> Issue {
        serde_json::from_str(&format!(
            r#"{{"id":"{id}","identifier":"{identifier}","title":"{title}",
                "priority":0,"state":null,"assignee":null,"description":null,
                "comments":null,"project":null,"cycle":null}}"#
        ))
        .unwrap()
    }

    fn app_with(issues: Vec<Issue>) -> App {
        let mut app = App::new(&Config::default());
        app.issues = issues;
        app.requests.clear();
        app
    }

    // ------------------------------------------------------------------ Input

    #[test]
    fn input_edits_at_the_cursor() {
        let mut input = Input::default();
        for c in "hello world".chars() {
            input.insert(c);
        }
        input.left();
        input.left();
        input.insert('X');
        assert_eq!(input.value, "hello worXld");
        input.backspace();
        assert_eq!(input.value, "hello world");
    }

    #[test]
    fn input_handles_multibyte_characters() {
        let mut input = Input::default();
        for c in "日本語".chars() {
            input.insert(c);
        }
        assert_eq!(input.cursor, 9);
        input.backspace();
        assert_eq!(input.value, "日本");
        input.home();
        input.delete();
        assert_eq!(input.value, "本");
    }

    #[test]
    fn input_kill_word_stops_at_the_previous_space() {
        let mut input = Input::default();
        for c in "add search feature".chars() {
            input.insert(c);
        }
        input.kill_word();
        assert_eq!(input.value, "add search ");
        input.kill_word();
        assert_eq!(input.value, "add ");
    }

    #[test]
    fn input_kill_to_start_and_end_split_at_the_cursor() {
        let mut input = Input::default();
        for c in "abcdef".chars() {
            input.insert(c);
        }
        input.left();
        input.left();
        input.kill_to_end();
        assert_eq!(input.value, "abcd");
        input.kill_to_start();
        assert_eq!(input.value, "");
    }

    // ------------------------------------------------------------ navigation

    #[test]
    fn nav_by_clamps_to_the_list() {
        let mut i = 0;
        App::nav_by(5, &mut i, 3);
        assert_eq!(i, 3);
        App::nav_by(5, &mut i, 10);
        assert_eq!(i, 4);
        App::nav_by(5, &mut i, -100);
        assert_eq!(i, 0);
    }

    /// `select_first`/`select_last` pass extreme deltas; they must not overflow.
    #[test]
    fn nav_by_survives_sentinel_deltas() {
        let mut i = 2;
        App::nav_by(5, &mut i, isize::MAX / 2);
        assert_eq!(i, 4);
        App::nav_by(5, &mut i, isize::MIN / 2);
        assert_eq!(i, 0);
    }

    #[test]
    fn nav_by_on_an_empty_list_stays_at_zero() {
        let mut i = 7;
        App::nav_by(0, &mut i, 1);
        assert_eq!(i, 0);
    }

    // ---------------------------------------------------------------- filters

    #[test]
    fn search_filters_by_title_and_identifier() {
        let mut app = app_with(vec![
            issue("1", "ENG-1", "Fix login"),
            issue("2", "ENG-2", "Add search"),
            issue("3", "OPS-9", "Rotate keys"),
        ]);
        app.search.value = "search".into();
        app.apply_search();
        assert_eq!(app.visible_issues().len(), 1);

        app.search.value = "eng-".into();
        app.apply_search();
        assert_eq!(app.visible_issues().len(), 2);
    }

    #[test]
    fn clearing_the_search_restores_every_issue() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a"), issue("2", "ENG-2", "b")]);
        app.search.value = "nothing-matches".into();
        app.apply_search();
        assert_eq!(app.visible_issues().len(), 0);

        app.clear_search();
        assert_eq!(app.visible_issues().len(), 2);
    }

    #[test]
    fn priority_filter_narrows_the_list() {
        let mut a = issue("1", "ENG-1", "a");
        a.priority = Priority::Urgent;
        let app_issues = vec![a, issue("2", "ENG-2", "b")];
        let mut app = app_with(app_issues);
        app.filters.priority = Some(Priority::Urgent);
        assert_eq!(app.visible_issues().len(), 1);
        assert_eq!(app.visible_issues()[0].identifier, "ENG-1");
    }

    // -------------------------------------------------------------- mutations

    /// Regression: changing a field from the detail view used to leave the
    /// detail showing the old value until the issue was reopened.
    #[test]
    fn a_priority_change_updates_every_copy_of_the_issue() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
        app.my_issues = vec![issue("1", "ENG-1", "a")];
        app.current_issue = Some(issue("1", "ENG-1", "a"));
        app.screen = Screen::IssueDetail;
        app.popup_index = Priority::High.as_index();

        app.apply_priority_selection();

        assert_eq!(app.issues[0].priority, Priority::High);
        assert_eq!(app.my_issues[0].priority, Priority::High);
        assert_eq!(app.current_issue.as_ref().unwrap().priority, Priority::High);
        assert_eq!(app.popup, Popup::None);
    }

    #[test]
    fn a_mutation_queues_exactly_one_request() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
        app.popup_index = Priority::Low.as_index();
        app.apply_priority_selection();
        assert_eq!(app.requests.len(), 1);
        assert!(matches!(
            app.requests.front(),
            Some(Request::UpdatePriority { priority: 4, .. })
        ));
    }

    #[test]
    fn identical_requests_are_not_queued_twice() {
        let mut app = app_with(vec![]);
        let req = Request::Issues {
            team_id: "t".into(),
            after: None,
            preset: Preset::Active,
        };
        app.request(req.clone());
        app.request(req);
        assert_eq!(app.requests.len(), 1);
    }

    // ------------------------------------------------------------ pagination

    #[test]
    fn appending_a_page_extends_the_list() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
        app.handle_message(Message::Issues {
            preset: Preset::Active,
            page: Page::new(vec![issue("2", "ENG-2", "b")], PageInfo::default(), true),
        });
        assert_eq!(app.issues.len(), 2);
    }

    /// Regression: scrolling near the bottom while the next page was still in
    /// flight asked for it again on every step, and each copy was appended.
    #[test]
    fn a_page_is_requested_once_while_scrolling() {
        let mut app = app_with(
            (0..10)
                .map(|i| issue(&i.to_string(), &format!("ENG-{i}"), "t"))
                .collect(),
        );
        app.teams = vec![serde_json::from_str(r#"{"id":"t","name":"Core","key":"ENG"}"#).unwrap()];
        app.page_info = PageInfo {
            has_next_page: true,
            end_cursor: Some("c1".into()),
        };
        app.select_last();
        // The main loop takes the request off the queue: it is now in flight.
        app.requests.clear();
        app.move_selection(-1);
        app.move_selection(1);
        assert!(
            app.requests.is_empty(),
            "the in-flight page is not asked for again"
        );
    }

    #[test]
    fn an_appended_page_never_duplicates_an_issue() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a"), issue("2", "ENG-2", "b")]);
        app.handle_message(Message::Issues {
            preset: Preset::Active,
            page: Page::new(
                vec![issue("2", "ENG-2", "b"), issue("3", "ENG-3", "c")],
                PageInfo::default(),
                true,
            ),
        });
        let ids: Vec<_> = app.issues.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["1", "2", "3"]);
    }

    #[test]
    fn a_fresh_page_replaces_the_list_and_keeps_the_selection() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a"), issue("2", "ENG-2", "b")]);
        app.selected_issue_index = 1;
        // ENG-2 comes back first this time; the cursor must follow the issue,
        // not stay on row 1.
        app.handle_message(Message::Issues {
            preset: Preset::Active,
            page: Page::new(
                vec![issue("2", "ENG-2", "b"), issue("9", "ENG-9", "c")],
                PageInfo::default(),
                false,
            ),
        });
        assert_eq!(app.issues.len(), 2);
        assert_eq!(app.selected_issue_index, 0);
    }

    #[test]
    fn a_shrinking_list_clamps_the_selection() {
        let mut app = app_with(vec![]);
        app.selected_project_index = 4;
        app.handle_message(Message::Projects(Page::new(
            Vec::new(),
            PageInfo::default(),
            false,
        )));
        assert_eq!(app.selected_project_index, 0);
    }

    // ---------------------------------------------------------------- scroll

    #[test]
    fn detail_scroll_clamps_to_the_content() {
        let mut app = app_with(vec![]);
        app.detail_lines = 30;
        app.detail_viewport = 10;
        app.scroll_to_bottom();
        assert_eq!(app.detail_scroll, 20);
        app.scroll_down();
        assert_eq!(app.detail_scroll, 20, "must not scroll past the end");
        app.scroll_by(-100);
        assert_eq!(app.detail_scroll, 0);
    }

    #[test]
    fn a_short_body_cannot_scroll_at_all() {
        let mut app = app_with(vec![]);
        app.detail_lines = 4;
        app.detail_viewport = 20;
        app.scroll_down();
        assert_eq!(app.detail_scroll, 0);
    }

    // ------------------------------------------------------------------ tabs

    #[test]
    fn switching_to_a_cached_tab_does_not_refetch() {
        let mut app = app_with(vec![]);
        app.projects_loaded = true;
        app.go_to_team_section(TeamSection::Projects);
        assert_eq!(app.screen, Screen::ProjectList);
        assert!(app.requests.is_empty());
    }

    #[test]
    fn switching_to_an_uncached_tab_fetches_it() {
        let mut app = app_with(vec![]);
        app.teams = vec![serde_json::from_str(r#"{"id":"t","name":"Core","key":"ENG"}"#).unwrap()];
        app.go_to_team_section(TeamSection::Cycles);
        assert!(matches!(app.requests.front(), Some(Request::Cycles { .. })));
    }

    #[test]
    fn changing_team_invalidates_the_other_tabs() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
        app.teams = vec![
            serde_json::from_str(r#"{"id":"t1","name":"Core","key":"ENG"}"#).unwrap(),
            serde_json::from_str(r#"{"id":"t2","name":"Ops","key":"OPS"}"#).unwrap(),
        ];
        app.projects_loaded = true;
        app.my_issues_loaded = true;
        app.popup_index = 1;

        app.select_team();

        assert_eq!(app.selected_team_index, 1);
        assert!(!app.projects_loaded);
        assert!(!app.my_issues_loaded);
        assert!(app.issues.is_empty());
    }

    // ------------------------------------------------------------ new issue

    #[test]
    fn creating_an_issue_requires_a_title() {
        let mut app = app_with(vec![]);
        app.teams = vec![serde_json::from_str(r#"{"id":"t","name":"Core","key":"ENG"}"#).unwrap()];
        app.start_new_issue();
        app.submit_new_issue();
        assert!(app.requests.is_empty());
        assert!(app.new_issue.is_some(), "the form stays open");
    }

    #[test]
    fn a_completed_form_queues_a_create_request() {
        let mut app = app_with(vec![]);
        app.teams = vec![serde_json::from_str(r#"{"id":"t","name":"Core","key":"ENG"}"#).unwrap()];
        app.start_new_issue();
        if let Some(form) = &mut app.new_issue {
            form.title.value = "Ship it".into();
            form.priority = Priority::Urgent;
        }
        app.submit_new_issue();

        assert!(app.new_issue.is_none());
        assert!(matches!(
            app.requests.front(),
            Some(Request::CreateIssue { priority: 1, .. })
        ));
    }

    #[test]
    fn search_results_are_not_narrowed_to_active() {
        let mut app = app_with(vec![]);
        app.handle_message(Message::SearchResults {
            term: "x".into(),
            issues: vec![stated("1", "s", "Done", "completed")],
        });
        assert_eq!(app.visible_issues().len(), 1);
        app.clear_search();
        assert_eq!(app.preset(), Preset::Active);
    }

    #[test]
    fn a_created_issue_appears_at_the_top() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
        app.handle_message(Message::IssueCreated(Box::new(issue("2", "ENG-2", "new"))));
        assert_eq!(app.issues[0].identifier, "ENG-2");
        assert_eq!(app.selected_issue_index, 0);
    }

    // -------------------------------------------------------------- clipboard

    #[test]
    fn copying_reports_when_there_is_nothing_to_copy() {
        let mut app = app_with(vec![]);
        app.copy_url();
        assert!(app.pending_clipboard.is_none());
        assert!(app.status_message.is_some());
    }

    #[test]
    fn copying_an_identifier_queues_the_clipboard_write() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
        app.copy_identifier();
        assert_eq!(app.pending_clipboard.as_deref(), Some("ENG-1"));
    }

    // -------------------------------------------------- Linear-aligned keys

    #[test]
    fn assign_to_me_sets_the_viewer_as_assignee() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
        app.viewer_id = Some("u1".into());
        app.team_members =
            vec![serde_json::from_str(r#"{"id":"u1","name":"Me","displayName":"me"}"#).unwrap()];

        app.assign_to_me();

        assert_eq!(app.issues[0].assignee.as_ref().unwrap().id, "u1");
        assert!(matches!(
            app.requests.front(),
            Some(Request::UpdateAssignee { .. })
        ));
    }

    #[test]
    fn assign_to_me_waits_for_the_viewer() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
        app.viewer_id = None;
        app.assign_to_me();
        assert!(app.requests.is_empty());
        assert!(app.status_message.is_some());
    }

    /// Linear's Shift+1…Shift+4 set a priority without opening the menu.
    #[test]
    fn set_priority_applies_without_a_popup() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
        app.set_priority(Priority::Urgent);
        assert_eq!(app.issues[0].priority, Priority::Urgent);
        assert_eq!(app.popup, Popup::None);
        assert!(matches!(
            app.requests.front(),
            Some(Request::UpdatePriority { priority: 1, .. })
        ));
    }

    // ------------------------------------------------------ grouping / nav

    fn stated(id: &str, state_id: &str, name: &str, kind: &str) -> Issue {
        serde_json::from_str(&format!(
            r#"{{"id":"{id}","identifier":"ENG-{id}","title":"t{id}","priority":0,
                "state":{{"id":"{state_id}","name":"{name}","type":"{kind}","position":1}},
                "assignee":null,"description":null,"comments":null,"project":null,"cycle":null}}"#
        ))
        .unwrap()
    }

    fn team(id: &str, name: &str) -> Team {
        serde_json::from_str(&format!(
            r#"{{"id":"{id}","name":"{name}","key":"{name}"}}"#
        ))
        .unwrap()
    }

    fn view(id: &str, name: &str, shared: bool) -> CustomView {
        serde_json::from_str(&format!(
            r#"{{"id":"{id}","name":"{name}","shared":{shared}}}"#
        ))
        .unwrap()
    }

    #[test]
    fn a_team_list_opens_on_active_and_hides_done_work() {
        let mut app = app_with(vec![
            stated("1", "s-todo", "Todo", "unstarted"),
            stated("2", "s-done", "Done", "completed"),
        ]);
        assert_eq!(app.preset(), Preset::Active);
        assert_eq!(app.visible_issues().len(), 1);
        app.set_preset(Preset::All);
        assert_eq!(app.visible_issues().len(), 2);
    }

    /// A saved view already chose its issues; the client must not narrow them.
    #[test]
    fn a_saved_view_opens_on_all() {
        let mut app = app_with(vec![]);
        app.custom_views = vec![view("v", "Mine", false)];
        app.view_issues = vec![stated("2", "s-done", "Done", "completed")];
        app.loaded_view_id = Some("v".into());
        app.activate(Nav::View(0));
        assert_eq!(app.preset(), Preset::All);
        assert_eq!(app.visible_issues().len(), 1);
    }

    #[test]
    fn issues_are_listed_in_group_order() {
        let mut app = app_with(vec![
            stated("1", "s-todo", "Todo", "unstarted"),
            stated("2", "s-prog", "In Progress", "started"),
            stated("3", "s-todo", "Todo", "unstarted"),
        ]);
        let ids: Vec<_> = app.visible_issues().iter().map(|i| i.id.clone()).collect();
        assert_eq!(ids, ["2", "1", "3"]);
        // Header rows interleave with the issues.
        let layout = app.list_layout();
        assert!(matches!(layout[0], ListRow::Group { count: 1, .. }));
        assert!(matches!(layout[2], ListRow::Group { count: 2, .. }));
        app.group_by = GroupBy::None;
        assert!(
            app.list_layout()
                .iter()
                .all(|r| matches!(r, ListRow::Issue { .. }))
        );
    }

    #[test]
    fn folding_a_group_keeps_the_cursor_on_a_visible_issue() {
        let mut app = app_with(vec![
            stated("1", "s-prog", "In Progress", "started"),
            stated("2", "s-todo", "Todo", "unstarted"),
            stated("3", "s-todo", "Todo", "unstarted"),
        ]);
        app.selected_issue_index = 2;
        app.toggle_selected_group();
        assert_eq!(app.visible_issues().len(), 1);
        assert_eq!(app.selected_issue_index, 0);
        app.toggle_group("status:s-todo");
        assert_eq!(app.visible_issues().len(), 3);
    }

    #[test]
    fn folding_all_groups_then_again_unfolds_them() {
        let mut app = app_with(vec![
            stated("1", "a", "In Progress", "started"),
            stated("2", "b", "Todo", "unstarted"),
        ]);
        app.toggle_all_groups();
        assert!(app.visible_issues().is_empty());
        app.toggle_all_groups();
        assert_eq!(app.visible_issues().len(), 2);
    }

    #[test]
    fn switching_a_team_preset_refetches_that_slice() {
        let mut app = app_with(vec![]);
        app.teams = vec![team("t1", "Core")];
        app.set_preset(Preset::Backlog);
        assert!(matches!(
            app.requests.back(),
            Some(Request::Issues {
                preset: Preset::Backlog,
                after: None,
                ..
            })
        ));
    }

    #[test]
    fn a_page_for_a_preset_already_left_is_dropped() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
        app.handle_message(Message::Issues {
            preset: Preset::All,
            page: Page::new(
                vec![issue("9", "ENG-9", "stale")],
                PageInfo::default(),
                false,
            ),
        });
        assert_eq!(app.issues[0].id, "1");
    }

    #[test]
    fn a_sidebar_team_entry_switches_team() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
        app.teams = vec![team("t1", "Core"), team("t2", "Ops")];
        app.activate(Nav::Team(1, TeamSection::Cycles));
        assert_eq!(app.selected_team_index, 1);
        assert_eq!(app.screen, Screen::CycleList);
        assert!(app.issues.is_empty(), "the old team's issues are dropped");
        assert!(
            app.requests
                .iter()
                .any(|r| matches!(r, Request::Cycles { team_id, .. } if team_id == "t2"))
        );
    }

    fn fav(json: &str) -> Favorite {
        serde_json::from_str(json).unwrap()
    }

    fn sidebar_navs(app: &App) -> Vec<Option<Nav>> {
        app.sidebar_rows
            .iter()
            .filter_map(|r| match r {
                SidebarRow::Item(i) => Some(i.nav()),
                _ => None,
            })
            .collect()
    }

    /// Only the current team's pages are listed; other teams are reached
    /// through the switcher, not a tree of every team.
    #[test]
    fn the_sidebar_shows_only_the_current_team() {
        let mut app = app_with(vec![]);
        app.teams = vec![team("t1", "Core"), team("t2", "Ops")];
        app.custom_views = vec![view("v1", "Today", false)];
        app.sidebar_rows = app.sidebar_layout();

        let navs = sidebar_navs(&app);
        assert!(navs.contains(&Some(Nav::Team(0, TeamSection::Projects))));
        assert!(!navs.contains(&Some(Nav::Team(1, TeamSection::Projects))));
        assert!(
            !navs.contains(&Some(Nav::View(0))),
            "views are not expanded"
        );
        assert!(app.sidebar_rows.iter().any(|r| matches!(
            r,
            SidebarRow::Item(i) if i.action == SidebarAction::SwitchTeam
        )));

        app.sidebar_index = 0;
        for _ in 0..app.sidebar_rows.len() {
            app.sidebar_move(1);
            assert!(matches!(
                app.sidebar_rows[app.sidebar_index],
                SidebarRow::Item(_)
            ));
        }
    }

    #[test]
    fn the_team_switcher_goes_to_the_picked_team() {
        let mut app = app_with(vec![]);
        app.teams = vec![team("t1", "Core"), team("t2", "Ops")];
        app.nav = Nav::MyIssues;
        app.run_sidebar_action(SidebarAction::SwitchTeam);
        assert_eq!(app.popup, Popup::TeamSelect);
        app.popup_index = 1;
        app.select_team();
        assert_eq!(app.nav, Nav::Team(1, TeamSection::Issues));
        assert_eq!(app.selected_team_index, 1);
    }

    #[test]
    fn switching_team_keeps_the_page() {
        let mut app = app_with(vec![]);
        app.teams = vec![team("t1", "Core"), team("t2", "Ops")];
        app.go_to_team_section(TeamSection::Cycles);
        app.open_team_select();
        app.popup_index = 1;
        app.select_team();
        assert_eq!(app.nav, Nav::Team(1, TeamSection::Cycles));
    }

    #[test]
    fn favorites_are_listed_in_linear_order_with_folders() {
        let mut app = app_with(vec![]);
        app.handle_message(Message::Favorites(vec![
            fav(r#"{"id":"b","type":"document","title":"Second","sortOrder":2}"#),
            fav(r#"{"id":"f","type":"folder","folderName":"Box","sortOrder":3}"#),
            fav(r#"{"id":"a","type":"document","title":"First","sortOrder":1}"#),
            fav(r#"{"id":"c","type":"document","title":"Inside","sortOrder":1,"parent":{"id":"f"}}"#),
        ]));
        app.sidebar_rows = app.sidebar_layout();
        let labels: Vec<_> = app
            .sidebar_rows
            .iter()
            .filter_map(|r| match r {
                SidebarRow::Item(i)
                    if i.tone == Tone::Strong && i.action != SidebarAction::SwitchTeam =>
                {
                    Some((i.label.clone(), i.depth))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            labels,
            [
                ("First".into(), 0),
                ("Second".into(), 0),
                ("Box".into(), 0),
                ("Inside".into(), 1)
            ]
        );

        let folder = app.favorites.iter().position(|f| f.id == "f").unwrap();
        app.toggle_folder(folder);
        assert!(
            !app.sidebar_rows
                .iter()
                .any(|r| matches!(r, SidebarRow::Item(i) if i.label == "Inside"))
        );
    }

    #[test]
    fn a_view_favorite_opens_the_view_itself() {
        let mut app = app_with(vec![]);
        app.custom_views = vec![view("v1", "Today", false)];
        app.favorites = vec![fav(
            r#"{"id":"x","type":"customView","customView":{"id":"v1"},"sortOrder":1}"#,
        )];
        assert_eq!(app.favorite_action(0), SidebarAction::Go(Nav::View(0)));
    }

    #[test]
    fn a_project_favorite_opens_the_project_and_esc_returns_to_the_sidebar() {
        let mut app = app_with(vec![]);
        app.favorites = vec![fav(
            r#"{"id":"x","type":"project","sortOrder":1,"project":{"id":"p1","name":"Launch","lead":null}}"#,
        )];
        app.activate(Nav::Favorite(0));
        assert_eq!(app.screen, Screen::ProjectDetail);
        assert_eq!(app.current_project.as_ref().unwrap().id, "p1");
        assert!(matches!(
            app.requests.back(),
            Some(Request::ProjectIssues { .. })
        ));
        app.leave_container();
        assert!(app.sidebar_focus);
        assert_eq!(
            app.screen,
            Screen::ProjectDetail,
            "nothing behind it to go back to"
        );
    }

    #[test]
    fn an_issue_favorite_opens_the_detail_and_esc_restores_where_you_were() {
        let mut app = app_with(vec![]);
        app.favorites = vec![fav(r#"{"id":"x","type":"issue","sortOrder":1,
                "issue":{"id":"i1","identifier":"ENG-1","title":"Bug"}}"#)];
        app.nav = Nav::MyIssues;
        app.activate(Nav::Favorite(0));
        assert_eq!(app.screen, Screen::IssueDetail);
        assert_eq!(app.nav, Nav::Favorite(0));
        assert!(matches!(
            app.requests.back(),
            Some(Request::IssueDetail { .. })
        ));
        app.close_detail();
        assert_eq!(app.nav, Nav::MyIssues);
        assert_eq!(app.screen, Screen::IssueList);
    }

    #[test]
    fn a_favorite_with_no_page_here_opens_in_the_browser() {
        let mut app = app_with(vec![]);
        app.favorites = vec![fav(
            r#"{"id":"x","type":"document","sortOrder":1,"url":"https://linear.app/d"}"#,
        )];
        let before = app.nav;
        app.activate(Nav::Favorite(0));
        assert_eq!(app.nav, before);
        assert!(
            matches!(app.requests.back(), Some(Request::OpenUrl(u)) if u == "https://linear.app/d")
        );
    }

    #[test]
    fn a_views_refetch_keeps_the_open_view_by_identity() {
        let mut app = app_with(vec![]);
        app.custom_views = vec![view("a", "A", false), view("b", "B", false)];
        app.nav = Nav::View(1);
        app.handle_message(Message::CustomViews(vec![
            view("b", "B", false),
            view("z", "Z", true),
        ]));
        assert_eq!(app.nav, Nav::View(0));
        assert_eq!(app.custom_views[0].id, "b");
    }

    fn scoped_view(id: &str, name: &str, team: Option<&str>, model: &str) -> CustomView {
        let mut v = view(id, name, true);
        v.team = team.map(|t| {
            serde_json::from_str(&format!(r#"{{"id":"{t}","name":"{t}","key":"{t}"}}"#)).unwrap()
        });
        v.model_name = Some(model.into());
        v
    }

    /// Like Linear: the workspace Views page lists views with no team, each
    /// team's page lists its own, and the tab splits issue from project views.
    #[test]
    fn views_pages_list_by_scope_and_kind() {
        let mut app = app_with(vec![]);
        app.teams = vec![team("t1", "Core")];
        app.handle_message(Message::CustomViews(vec![
            scoped_view("a", "Workspace bugs", None, "Issue"),
            scoped_view("b", "Core board", Some("t1"), "Issue"),
            scoped_view("c", "Roadmap", None, "Project"),
            scoped_view("d", "Core roadmap", Some("t1"), "Project"),
        ]));
        let names = |app: &App| -> Vec<String> {
            app.listed_views()
                .into_iter()
                .map(|i| app.custom_views[i].name.clone())
                .collect()
        };

        app.activate(Nav::Views);
        assert_eq!(names(&app), ["Workspace bugs"]);
        app.cycle_view_kind();
        assert_eq!(names(&app), ["Roadmap"]);

        app.activate(Nav::Team(0, TeamSection::Views));
        assert_eq!(app.screen, Screen::ViewList);
        assert_eq!(names(&app), ["Core roadmap"]);
        app.set_view_kind(ViewKind::Issues);
        assert_eq!(names(&app), ["Core board"]);
    }

    #[test]
    fn a_project_view_opens_as_a_project_list() {
        let mut app = app_with(vec![]);
        app.custom_views = vec![scoped_view("p", "Roadmap", None, "Project")];
        app.activate(Nav::View(0));
        assert_eq!(app.screen, Screen::ProjectList);
        assert!(matches!(
            app.requests.back(),
            Some(Request::ViewProjects { after: None, .. })
        ));

        let project: Project =
            serde_json::from_str(r#"{"id":"p1","name":"Launch","lead":null}"#).unwrap();
        app.handle_message(Message::ViewProjects {
            view_id: "p".into(),
            page: Page::new(vec![project], PageInfo::default(), false),
        });
        assert_eq!(
            app.project_rows().len(),
            1,
            "rows come from the view, not the team"
        );
        app.open_project_detail();
        assert_eq!(app.current_project.as_ref().unwrap().id, "p1");
        app.leave_container();
        assert_eq!(app.screen, Screen::ProjectList, "back to the view");
    }

    #[test]
    fn opening_the_same_project_view_again_does_not_refetch() {
        let mut app = app_with(vec![]);
        app.custom_views = vec![scoped_view("p", "Roadmap", None, "Project")];
        app.loaded_view_projects_id = Some("p".into());
        app.activate(Nav::View(0));
        assert!(app.requests.is_empty());
    }

    #[test]
    fn personal_views_sort_above_shared_ones() {
        let mut app = app_with(vec![]);
        app.handle_message(Message::CustomViews(vec![
            view("1", "Team board", true),
            view("2", "zzz mine", false),
        ]));
        assert!(!app.custom_views[0].shared);
    }

    #[test]
    fn stepping_through_issues_from_the_detail_view() {
        let mut app = app_with(vec![
            stated("1", "s", "Todo", "unstarted"),
            stated("2", "s", "Todo", "unstarted"),
        ]);
        app.open_issue_detail();
        assert_eq!(app.detail_position(), Some((0, 2)));
        app.step_issue(1);
        assert_eq!(app.current_issue.as_ref().unwrap().id, "2");
        assert_eq!(app.selected_issue_index, 1);
        app.step_issue(1);
        assert_eq!(
            app.current_issue.as_ref().unwrap().id,
            "2",
            "clamped at the end"
        );
        app.close_detail();
        assert_eq!(app.screen, Screen::IssueList);
    }

    #[test]
    fn the_detail_view_returns_to_the_project_it_came_from() {
        let mut app = app_with(vec![]);
        app.project_issues = vec![stated("1", "s", "Todo", "unstarted")];
        app.screen = Screen::ProjectDetail;
        app.open_issue_detail();
        assert_eq!(app.screen, Screen::IssueDetail);
        app.close_detail();
        assert_eq!(app.screen, Screen::ProjectDetail);
    }

    // ------------------------------------------------------------------ mouse

    fn clickable(app: &mut App) {
        app.list_area = Rect::new(30, 5, 80, 20);
        app.list_rows = app.list_layout();
    }

    #[test]
    fn clicking_a_row_selects_it_and_clicking_again_opens_it() {
        let mut app = app_with(vec![
            stated("1", "s", "Todo", "unstarted"),
            stated("2", "s", "Todo", "unstarted"),
        ]);
        clickable(&mut app);
        // Row 0 is the "Todo" header, rows 1 and 2 the issues.
        app.click(40, 7);
        assert_eq!(app.selected_issue_index, 1);
        assert_eq!(app.screen, Screen::IssueList);
        app.click(40, 7);
        assert_eq!(app.screen, Screen::IssueDetail);
        assert_eq!(app.current_issue.as_ref().unwrap().id, "2");
    }

    #[test]
    fn clicking_a_group_header_folds_it() {
        let mut app = app_with(vec![stated("1", "s", "Todo", "unstarted")]);
        clickable(&mut app);
        app.click(40, 5);
        assert!(app.collapsed_groups.contains("status:s"));
    }

    #[test]
    fn clicking_a_preset_chip_switches_preset() {
        let mut app = app_with(vec![]);
        app.chip_areas = vec![(Rect::new(30, 3, 8, 1), Chip::Preset(Preset::Backlog))];
        app.click(32, 3);
        assert_eq!(app.preset(), Preset::Backlog);
    }

    #[test]
    fn clicking_a_sidebar_entry_navigates() {
        let mut app = app_with(vec![]);
        app.viewer_id = Some("u".into());
        app.sidebar_rows = app.sidebar_layout();
        app.sidebar_area = Rect::new(0, 2, 25, 30);
        // Row 0 is My Issues.
        app.click(10, 2);
        assert_eq!(app.nav, Nav::MyIssues);
        assert!(matches!(
            app.requests.back(),
            Some(Request::MyIssues { .. })
        ));
    }

    #[test]
    fn clicking_a_popup_entry_applies_it() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
        app.open_priority_change();
        app.popup_area = Rect::new(10, 10, 30, 5);
        app.click(15, 11); // second entry: Urgent
        assert_eq!(app.popup, Popup::None);
        assert_eq!(app.issues[0].priority, Priority::Urgent);
    }

    #[test]
    fn clicking_outside_a_popup_closes_it() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
        app.open_priority_change();
        app.popup_area = Rect::new(10, 10, 30, 5);
        app.click(0, 0);
        assert_eq!(app.popup, Popup::None);
        assert!(app.requests.is_empty());
    }
}
