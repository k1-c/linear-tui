use std::collections::VecDeque;

use ratatui::widgets::TableState;

use crate::api::types::*;
use crate::config::{Config, Theme};
use crate::message::{Message, Page, Request};

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// How close to the bottom of the list the cursor must get before the next page
/// is requested.
const PREFETCH_MARGIN: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Issues,
    MyIssues,
    Projects,
    Cycles,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[Tab::Issues, Tab::MyIssues, Tab::Projects, Tab::Cycles]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Tab::Issues => "Issues",
            Tab::MyIssues => "My Issues",
            Tab::Projects => "Projects",
            Tab::Cycles => "Cycles",
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

    // Tab
    pub tab: Tab,

    // Popup
    pub popup: Popup,
    pub popup_index: usize,

    // Teams
    pub teams: Vec<Team>,
    pub selected_team_index: usize,
    pub team_members: Vec<User>,

    // Issues
    pub issues: Vec<Issue>,
    pub filtered_issues: Vec<usize>,
    pub selected_issue_index: usize,
    pub page_info: PageInfo,

    // Filters
    pub filters: Filters,
    pub filter_kind: FilterKind,
    pub workflow_states: Vec<WorkflowState>,

    // Issue detail
    pub current_issue: Option<Issue>,
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
            tab: Tab::Issues,
            popup: Popup::None,
            popup_index: 0,
            teams: Vec::new(),
            selected_team_index: 0,
            team_members: Vec::new(),
            issues: Vec::new(),
            filtered_issues: Vec::new(),
            selected_issue_index: 0,
            page_info: PageInfo::default(),
            filters: Filters::default(),
            filter_kind: FilterKind::Status,
            workflow_states: Vec::new(),
            current_issue: None,
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

    /// Queue the fetch that populates the currently visible tab.
    pub fn reload_current_tab(&mut self) {
        match self.tab {
            Tab::Issues => {
                if let Some(team_id) = self.team_id() {
                    self.request(Request::Issues {
                        team_id,
                        after: None,
                    });
                }
            }
            Tab::MyIssues => {
                if let Some(user_id) = self.viewer_id.clone() {
                    self.request(Request::MyIssues {
                        user_id,
                        after: None,
                    });
                }
            }
            Tab::Projects => {
                if let Some(team_id) = self.team_id() {
                    self.request(Request::Projects {
                        team_id,
                        after: None,
                    });
                }
            }
            Tab::Cycles => {
                if let Some(team_id) = self.team_id() {
                    self.request(Request::Cycles {
                        team_id,
                        after: None,
                    });
                }
            }
        }
    }

    /// Force a refetch of the current tab, discarding its cached flag.
    pub fn force_reload(&mut self) {
        self.global_search = None;
        match self.tab {
            Tab::MyIssues => self.my_issues_loaded = false,
            Tab::Projects => self.projects_loaded = false,
            Tab::Cycles => self.cycles_loaded = false,
            Tab::Issues => {}
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
            Message::Issues(p) | Message::MyIssues(p) => p.append,
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
                if let Some(team_id) = self.team_id() {
                    self.request(Request::TeamContext {
                        team_id: team_id.clone(),
                    });
                    self.request(Request::Issues {
                        team_id,
                        after: None,
                    });
                }
            }
            Message::Viewer(id) => {
                self.viewer_id = Some(id);
                if self.tab == Tab::MyIssues {
                    self.reload_current_tab();
                }
            }
            Message::TeamContext { states, members } => {
                self.workflow_states = states;
                self.team_members = members;
            }
            Message::Issues(page) => {
                let keep = self.selected_issue_id();
                Self::merge(&mut self.issues, page, &mut self.page_info);
                if !page_appended {
                    self.filtered_issues.clear();
                    if !self.search.is_empty() {
                        self.apply_search();
                    }
                    self.restore_issue_selection(keep.as_deref());
                }
                self.clear_status();
            }
            Message::MyIssues(page) => {
                Self::merge(&mut self.my_issues, page, &mut self.my_issues_page_info);
                if !page_appended {
                    self.selected_my_issue_index = 0;
                }
                self.my_issues_loaded = true;
                self.clear_status();
            }
            Message::SearchResults { term, issues } => {
                self.tab = Tab::Issues;
                self.screen = Screen::IssueList;
                self.issues = issues;
                self.page_info = PageInfo::default();
                self.filtered_issues.clear();
                self.filters.clear();
                self.search.clear();
                self.selected_issue_index = 0;
                self.global_search = Some(term);
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
                Self::merge(
                    &mut self.project_issues,
                    page,
                    &mut self.project_issues_page_info,
                );
                self.clamp(Field::ProjectIssues);
                self.project_issues_loaded = true;
            }
            Message::CycleIssues(page) => {
                Self::merge(
                    &mut self.cycle_issues,
                    page,
                    &mut self.cycle_issues_page_info,
                );
                self.clamp(Field::CycleIssues);
                self.cycle_issues_loaded = true;
            }
            Message::IssueCreated(issue) => {
                self.set_status(format!("Created {}", issue.identifier));
                // Show it immediately at the top rather than waiting for a refetch.
                self.issues.insert(0, *issue);
                if self.tab == Tab::Issues && self.screen == Screen::IssueList {
                    self.selected_issue_index = 0;
                }
            }
            Message::Mutated(what) => {
                self.set_status(what);
                // A posted comment clears the cached thread; pull it back in.
                self.queue_detail_fetches();
            }
            Message::Error(err) => self.set_error(err),
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
            .get(self.selected_issue_index)
            .map(|i| i.id.clone())
    }

    fn restore_issue_selection(&mut self, id: Option<&str>) {
        self.selected_issue_index = id
            .and_then(|id| self.visible_issues().iter().position(|i| i.id == id))
            .unwrap_or(0);
    }

    /// Get the issue currently focused (selected in list, or being viewed in detail).
    pub fn focused_issue(&self) -> Option<&Issue> {
        match self.screen {
            Screen::IssueDetail => self.current_issue.as_ref(),
            Screen::IssueList => match self.tab {
                Tab::Issues => {
                    let issues = self.visible_issues();
                    issues.get(self.selected_issue_index).copied()
                }
                Tab::MyIssues => self
                    .visible_my_issues()
                    .get(self.selected_my_issue_index)
                    .copied(),
                _ => None,
            },
            Screen::ProjectDetail => self.project_issues.get(self.selected_project_issue_index),
            Screen::CycleDetail => self.cycle_issues.get(self.selected_cycle_issue_index),
            Screen::ProjectList | Screen::CycleList => None,
        }
    }

    /// My Issues filtered by the live search query.
    pub fn visible_my_issues(&self) -> Vec<&Issue> {
        if self.search.is_empty() {
            return self.my_issues.iter().collect();
        }
        let query = self.search.value.to_lowercase();
        self.my_issues
            .iter()
            .filter(|i| Self::matches(i, &query))
            .collect()
    }

    fn matches(issue: &Issue, lowercase_query: &str) -> bool {
        issue.title.to_lowercase().contains(lowercase_query)
            || issue.identifier.to_lowercase().contains(lowercase_query)
    }

    pub fn visible_issues(&self) -> Vec<&Issue> {
        let base: Vec<&Issue> = if !self.filtered_issues.is_empty() || !self.search.is_empty() {
            self.filtered_issues
                .iter()
                .filter_map(|&i| self.issues.get(i))
                .collect()
        } else {
            self.issues.iter().collect()
        };

        base.into_iter()
            .filter(|issue| {
                if let Some(status) = &self.filters.status {
                    if let Some(state) = &issue.state {
                        if &state.name != status {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                if let Some(pri) = self.filters.priority
                    && issue.priority != pri
                {
                    return false;
                }
                true
            })
            .collect()
    }

    /// Ask for the next page once the cursor nears the end of what we have.
    fn maybe_prefetch(&mut self, len: usize) {
        if self.selected_issue_index + PREFETCH_MARGIN < len || !self.page_info.has_next_page {
            return;
        }
        if let (Some(team_id), Some(cursor)) = (self.team_id(), self.page_info.end_cursor.clone()) {
            self.request(Request::Issues {
                team_id,
                after: Some(cursor),
            });
        }
    }

    fn maybe_prefetch_my_issues(&mut self) {
        if self.selected_my_issue_index + PREFETCH_MARGIN < self.my_issues.len()
            || !self.my_issues_page_info.has_next_page
        {
            return;
        }
        if let (Some(user_id), Some(cursor)) = (
            self.viewer_id.clone(),
            self.my_issues_page_info.end_cursor.clone(),
        ) {
            self.request(Request::MyIssues {
                user_id,
                after: Some(cursor),
            });
        }
    }

    fn maybe_prefetch_projects(&mut self) {
        if self.selected_project_index + PREFETCH_MARGIN < self.projects.len()
            || !self.projects_page_info.has_next_page
        {
            return;
        }
        if let (Some(team_id), Some(cursor)) =
            (self.team_id(), self.projects_page_info.end_cursor.clone())
        {
            self.request(Request::Projects {
                team_id,
                after: Some(cursor),
            });
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
        {
            self.request(Request::Cycles {
                team_id,
                after: Some(cursor),
            });
        }
    }

    fn maybe_prefetch_project_issues(&mut self) {
        if self.selected_project_issue_index + PREFETCH_MARGIN < self.project_issues.len()
            || !self.project_issues_page_info.has_next_page
        {
            return;
        }
        if let (Some(project), Some(cursor)) = (
            self.current_project.as_ref().map(|p| p.id.clone()),
            self.project_issues_page_info.end_cursor.clone(),
        ) {
            self.request(Request::ProjectIssues {
                project_id: project,
                after: Some(cursor),
            });
        }
    }

    fn maybe_prefetch_cycle_issues(&mut self) {
        if self.selected_cycle_issue_index + PREFETCH_MARGIN < self.cycle_issues.len()
            || !self.cycle_issues_page_info.has_next_page
        {
            return;
        }
        if let (Some(cycle), Some(cursor)) = (
            self.current_cycle.as_ref().map(|c| c.id.clone()),
            self.cycle_issues_page_info.end_cursor.clone(),
        ) {
            self.request(Request::CycleIssues {
                cycle_id: cycle,
                after: Some(cursor),
            });
        }
    }

    pub fn open_issue_detail(&mut self) {
        let issues = self.visible_issues();
        if let Some(issue) = issues.get(self.selected_issue_index) {
            let issue = (*issue).clone();
            self.open_issue_from_list(&issue);
        }
    }

    pub fn open_team_select(&mut self) {
        self.popup = Popup::TeamSelect;
        self.popup_index = self.selected_team_index;
    }

    pub fn select_team(&mut self) {
        if self.popup_index < self.teams.len() && self.popup_index != self.selected_team_index {
            self.selected_team_index = self.popup_index;
            self.issues.clear();
            self.filtered_issues.clear();
            self.selected_issue_index = 0;
            self.filters.clear();
            self.invalidate_tab_caches();
            if let Some(team_id) = self.team_id() {
                self.request(Request::TeamContext { team_id });
            }
            self.reload_current_tab();
        }
        self.popup = Popup::None;
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
            Screen::IssueList => match self.tab {
                Tab::Issues => {
                    let len = self.visible_issues().len();
                    Self::nav_by(len, &mut self.selected_issue_index, delta);
                    self.maybe_prefetch(len);
                }
                Tab::MyIssues => {
                    let len = self.visible_my_issues().len();
                    Self::nav_by(len, &mut self.selected_my_issue_index, delta);
                    self.maybe_prefetch_my_issues();
                }
                _ => {}
            },
            Screen::ProjectList => {
                Self::nav_by(self.projects.len(), &mut self.selected_project_index, delta);
                self.maybe_prefetch_projects();
            }
            Screen::CycleList => {
                Self::nav_by(self.cycles.len(), &mut self.selected_cycle_index, delta);
                self.maybe_prefetch_cycles();
            }
            Screen::ProjectDetail => {
                Self::nav_by(
                    self.project_issues.len(),
                    &mut self.selected_project_issue_index,
                    delta,
                );
                self.maybe_prefetch_project_issues();
            }
            Screen::CycleDetail => {
                Self::nav_by(
                    self.cycle_issues.len(),
                    &mut self.selected_cycle_issue_index,
                    delta,
                );
                self.maybe_prefetch_cycle_issues();
            }
            Screen::IssueDetail => {}
        }
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

    pub fn apply_search(&mut self) {
        self.selected_my_issue_index = 0;
        if self.search.is_empty() {
            self.filtered_issues.clear();
        } else {
            let query = self.search.value.to_lowercase();
            self.filtered_issues = self
                .issues
                .iter()
                .enumerate()
                .filter(|(_, issue)| Self::matches(issue, &query))
                .map(|(i, _)| i)
                .collect();
        }
        self.selected_issue_index = 0;
    }

    pub fn clear_search(&mut self) {
        self.search.clear();
        self.filtered_issues.clear();
        self.selected_issue_index = 0;
        self.selected_my_issue_index = 0;
        // Leaving a workspace search returns the list to the team's own issues.
        if self.global_search.take().is_some() {
            self.reload_current_tab();
        }
    }

    // ------------------------------------------------- open / copy / create

    /// Open the focused issue (or the selected project) on linear.app.
    pub fn open_in_browser(&mut self) {
        let url = match self.screen {
            Screen::ProjectList => self
                .projects
                .get(self.selected_project_index)
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
        self.current_issue = Some(issue.clone());
        self.detail_scroll = 0;
        self.screen = Screen::IssueDetail;
        self.queue_detail_fetches();
    }

    pub fn switch_tab(&mut self, tab: Tab) {
        if self.tab == tab {
            return;
        }
        self.tab = tab;
        let cached = match tab {
            Tab::Issues => {
                self.screen = Screen::IssueList;
                !self.issues.is_empty()
            }
            Tab::MyIssues => {
                self.screen = Screen::IssueList;
                self.my_issues_loaded
            }
            Tab::Projects => {
                self.screen = Screen::ProjectList;
                self.projects_loaded
            }
            Tab::Cycles => {
                self.screen = Screen::CycleList;
                self.cycles_loaded
            }
        };
        if !cached {
            self.reload_current_tab();
        }
    }

    pub fn invalidate_tab_caches(&mut self) {
        self.my_issues_loaded = false;
        self.projects_loaded = false;
        self.cycles_loaded = false;
        self.project_issues_loaded = false;
        self.cycle_issues_loaded = false;
    }

    // Project navigation
    pub fn open_project_detail(&mut self) {
        if let Some(project) = self.projects.get(self.selected_project_index) {
            self.current_project = Some(project.clone());
            self.project_issues.clear();
            self.project_issues_loaded = false;
            self.selected_project_issue_index = 0;
            self.screen = Screen::ProjectDetail;
            self.queue_detail_fetches();
        }
    }

    // Cycle navigation
    pub fn open_cycle_detail(&mut self) {
        if let Some(cycle) = self.cycles.get(self.selected_cycle_index) {
            self.current_cycle = Some(cycle.clone());
            self.cycle_issues.clear();
            self.cycle_issues_loaded = false;
            self.selected_cycle_issue_index = 0;
            self.screen = Screen::CycleDetail;
            self.queue_detail_fetches();
        }
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
        };
        app.request(req.clone());
        app.request(req);
        assert_eq!(app.requests.len(), 1);
    }

    // ------------------------------------------------------------ pagination

    #[test]
    fn appending_a_page_extends_the_list() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a")]);
        app.handle_message(Message::Issues(Page::new(
            vec![issue("2", "ENG-2", "b")],
            PageInfo::default(),
            true,
        )));
        assert_eq!(app.issues.len(), 2);
    }

    #[test]
    fn a_fresh_page_replaces_the_list_and_keeps_the_selection() {
        let mut app = app_with(vec![issue("1", "ENG-1", "a"), issue("2", "ENG-2", "b")]);
        app.selected_issue_index = 1;
        // ENG-2 comes back first this time; the cursor must follow the issue,
        // not stay on row 1.
        app.handle_message(Message::Issues(Page::new(
            vec![issue("2", "ENG-2", "b"), issue("9", "ENG-9", "c")],
            PageInfo::default(),
            false,
        )));
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
        app.switch_tab(Tab::Projects);
        assert_eq!(app.screen, Screen::ProjectList);
        assert!(app.requests.is_empty());
    }

    #[test]
    fn switching_to_an_uncached_tab_fetches_it() {
        let mut app = app_with(vec![]);
        app.teams = vec![serde_json::from_str(r#"{"id":"t","name":"Core","key":"ENG"}"#).unwrap()];
        app.switch_tab(Tab::Cycles);
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
}
