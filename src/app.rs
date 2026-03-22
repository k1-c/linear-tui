use crate::api::types::*;
use crate::config::Theme;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

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

/// Pending action from a popup selection, to be executed by main loop.
#[derive(Debug, Clone)]
pub enum PendingAction {
    UpdateStatus {
        issue_id: String,
        state_id: String,
    },
    UpdatePriority {
        issue_id: String,
        priority: Priority,
    },
    UpdateAssignee {
        issue_id: String,
        assignee_id: Option<String>,
    },
    CreateComment {
        issue_id: String,
        body: String,
    },
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

pub struct App {
    pub screen: Screen,
    pub input_mode: InputMode,
    pub should_quit: bool,
    pub needs_reload: bool,
    pub needs_load_more: bool,

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
    pub loading: bool,

    // Filters
    pub filters: Filters,
    pub filter_kind: FilterKind,
    pub workflow_states: Vec<WorkflowState>,

    // Issue detail
    pub current_issue: Option<Issue>,
    pub detail_scroll: u16,

    // Search
    pub search_query: String,

    // Comment input
    pub comment_input: String,

    // Pending actions for main loop to execute
    pub pending_action: Option<PendingAction>,

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

    // Cycles
    pub cycles: Vec<Cycle>,
    pub selected_cycle_index: usize,
    pub current_cycle: Option<Cycle>,
    pub cycle_issues: Vec<Issue>,
    pub selected_cycle_issue_index: usize,
    pub cycles_loaded: bool,

    // Status
    pub status_message: Option<String>,

    // Spinner
    pub spinner_frame: usize,

    // Error popup
    pub error_popup: Option<String>,

    // Help
    pub show_help: bool,

    // Theme
    pub theme: Theme,
}

impl App {
    pub fn new(theme: Theme) -> Self {
        Self {
            screen: Screen::IssueList,
            input_mode: InputMode::Normal,
            should_quit: false,
            needs_reload: true,
            needs_load_more: false,
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
            loading: false,
            filters: Filters::default(),
            filter_kind: FilterKind::Status,
            workflow_states: Vec::new(),
            current_issue: None,
            detail_scroll: 0,
            search_query: String::new(),
            comment_input: String::new(),
            pending_action: None,
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
            cycles: Vec::new(),
            selected_cycle_index: 0,
            current_cycle: None,
            cycle_issues: Vec::new(),
            selected_cycle_issue_index: 0,
            cycles_loaded: false,
            status_message: None,
            spinner_frame: 0,
            error_popup: None,
            show_help: false,
            theme,
        }
    }

    pub fn current_team(&self) -> Option<&Team> {
        self.teams.get(self.selected_team_index)
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
                Tab::MyIssues => self.my_issues.get(self.selected_my_issue_index),
                _ => None,
            },
            Screen::ProjectDetail => self.project_issues.get(self.selected_project_issue_index),
            Screen::CycleDetail => self.cycle_issues.get(self.selected_cycle_issue_index),
            Screen::ProjectList | Screen::CycleList => None,
        }
    }

    pub fn visible_issues(&self) -> Vec<&Issue> {
        let base: Vec<&Issue> = if !self.filtered_issues.is_empty() || !self.search_query.is_empty()
        {
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

    pub fn next_issue(&mut self) {
        let len = self.visible_issues().len();
        if len > 0 && self.selected_issue_index < len - 1 {
            self.selected_issue_index += 1;
            if self.selected_issue_index >= len.saturating_sub(5) && self.page_info.has_next_page {
                self.needs_load_more = true;
            }
        }
    }

    pub fn previous_issue(&mut self) {
        Self::nav_prev(&mut self.selected_issue_index);
    }

    pub fn first_issue(&mut self) {
        Self::nav_first(&mut self.selected_issue_index);
    }

    pub fn last_issue(&mut self) {
        let len = self.visible_issues().len();
        Self::nav_last(len, &mut self.selected_issue_index);
    }

    pub fn open_issue_detail(&mut self) {
        let issues = self.visible_issues();
        if let Some(issue) = issues.get(self.selected_issue_index) {
            let issue = (*issue).clone();
            self.open_issue_from_list(&issue);
        }
    }

    pub fn request_reload(&mut self) {
        self.needs_reload = true;
    }

    pub fn open_team_select(&mut self) {
        self.popup = Popup::TeamSelect;
        self.popup_index = self.selected_team_index;
    }

    pub fn select_team(&mut self) {
        if self.popup_index < self.teams.len() {
            self.selected_team_index = self.popup_index;
            self.issues.clear();
            self.filtered_issues.clear();
            self.selected_issue_index = 0;
            self.filters.clear();
            self.invalidate_tab_caches();
            self.needs_reload = true;
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
            self.popup = Popup::StatusChange;
            self.popup_index = 0;
        }
    }

    pub fn open_priority_change(&mut self) {
        if self.focused_issue().is_some() {
            self.popup = Popup::PriorityChange;
            self.popup_index = 0;
        }
    }

    pub fn open_assignee_change(&mut self) {
        if self.focused_issue().is_some() {
            self.popup = Popup::AssigneeChange;
            self.popup_index = 0;
        }
    }

    pub fn start_comment(&mut self) {
        if self.focused_issue().is_some() {
            self.input_mode = InputMode::Comment;
            self.comment_input.clear();
        }
    }

    pub fn submit_comment(&mut self) {
        if let Some(issue) = self.focused_issue()
            && !self.comment_input.is_empty()
        {
            self.pending_action = Some(PendingAction::CreateComment {
                issue_id: issue.id.clone(),
                body: self.comment_input.clone(),
            });
        }
        self.input_mode = InputMode::Normal;
        self.comment_input.clear();
    }

    pub fn apply_status_selection(&mut self) {
        if let Some(issue) = self.focused_issue()
            && let Some(state) = self.workflow_states.get(self.popup_index)
        {
            self.pending_action = Some(PendingAction::UpdateStatus {
                issue_id: issue.id.clone(),
                state_id: state.id.clone(),
            });
        }
        self.popup = Popup::None;
    }

    pub fn apply_priority_selection(&mut self) {
        if let Some(issue) = self.focused_issue() {
            let priority = Priority::from_index(self.popup_index);
            self.pending_action = Some(PendingAction::UpdatePriority {
                issue_id: issue.id.clone(),
                priority,
            });
        }
        self.popup = Popup::None;
    }

    pub fn apply_assignee_selection(&mut self) {
        if let Some(issue) = self.focused_issue() {
            let assignee_id = if self.popup_index == 0 {
                None // Unassign
            } else {
                self.team_members
                    .get(self.popup_index - 1)
                    .map(|u| u.id.clone())
            };
            self.pending_action = Some(PendingAction::UpdateAssignee {
                issue_id: issue.id.clone(),
                assignee_id,
            });
        }
        self.popup = Popup::None;
    }

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

    pub fn scroll_down(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_add(1);
    }

    pub fn scroll_up(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_sub(1);
    }

    pub fn apply_search(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_issues.clear();
        } else {
            let query = self.search_query.to_lowercase();
            self.filtered_issues = self
                .issues
                .iter()
                .enumerate()
                .filter(|(_, issue)| {
                    issue.title.to_lowercase().contains(&query)
                        || issue.identifier.to_lowercase().contains(&query)
                })
                .map(|(i, _)| i)
                .collect();
        }
        self.selected_issue_index = 0;
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    // Generic list navigation helpers
    pub fn nav_next(len: usize, index: &mut usize) {
        if len > 0 && *index < len - 1 {
            *index += 1;
        }
    }

    pub fn nav_prev(index: &mut usize) {
        if *index > 0 {
            *index -= 1;
        }
    }

    pub fn nav_first(index: &mut usize) {
        *index = 0;
    }

    pub fn nav_last(len: usize, index: &mut usize) {
        if len > 0 {
            *index = len - 1;
        }
    }

    /// Open issue detail from any sub-list (project issues, cycle issues, my issues).
    pub fn open_issue_from_list(&mut self, issue: &Issue) {
        self.current_issue = Some(issue.clone());
        self.detail_scroll = 0;
        self.screen = Screen::IssueDetail;
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

    pub fn switch_tab(&mut self, tab: Tab) {
        if self.tab == tab {
            return;
        }
        self.tab = tab;
        match tab {
            Tab::Issues => {
                self.screen = Screen::IssueList;
                self.needs_reload = true;
            }
            Tab::MyIssues => {
                self.screen = Screen::IssueList;
                if !self.my_issues_loaded {
                    self.needs_reload = true;
                }
            }
            Tab::Projects => {
                self.screen = Screen::ProjectList;
                if !self.projects_loaded {
                    self.needs_reload = true;
                }
            }
            Tab::Cycles => {
                self.screen = Screen::CycleList;
                if !self.cycles_loaded {
                    self.needs_reload = true;
                }
            }
        }
    }

    pub fn invalidate_tab_caches(&mut self) {
        self.my_issues_loaded = false;
        self.projects_loaded = false;
        self.cycles_loaded = false;
    }

    // Project navigation
    pub fn next_project(&mut self) {
        Self::nav_next(self.projects.len(), &mut self.selected_project_index);
    }

    pub fn previous_project(&mut self) {
        Self::nav_prev(&mut self.selected_project_index);
    }

    pub fn open_project_detail(&mut self) {
        if let Some(project) = self.projects.get(self.selected_project_index) {
            self.current_project = Some(project.clone());
            self.project_issues.clear();
            self.selected_project_issue_index = 0;
            self.screen = Screen::ProjectDetail;
        }
    }

    // Cycle navigation
    pub fn next_cycle(&mut self) {
        Self::nav_next(self.cycles.len(), &mut self.selected_cycle_index);
    }

    pub fn previous_cycle(&mut self) {
        Self::nav_prev(&mut self.selected_cycle_index);
    }

    pub fn open_cycle_detail(&mut self) {
        if let Some(cycle) = self.cycles.get(self.selected_cycle_index) {
            self.current_cycle = Some(cycle.clone());
            self.cycle_issues.clear();
            self.selected_cycle_issue_index = 0;
            self.screen = Screen::CycleDetail;
        }
    }
}

#[cfg(test)]
struct IssueBuilder {
    issue: Issue,
}

#[cfg(test)]
impl IssueBuilder {
    fn new(id: &str, identifier: &str, title: &str) -> Self {
        Self {
            issue: Issue {
                id: id.to_string(),
                identifier: identifier.to_string(),
                title: title.to_string(),
                priority: Priority::default(),
                priority_label: None,
                state: None,
                assignee: None,
                labels: None,
                description: None,
                created_at: None,
                updated_at: None,
                comments: None,
                project: None,
                cycle: None,
            },
        }
    }

    fn state(mut self, name: &str) -> Self {
        self.issue.state = Some(WorkflowState {
            id: name.to_string(),
            name: name.to_string(),
            color: None,
            state_type: None,
        });
        self
    }

    fn priority(mut self, priority: Priority) -> Self {
        self.issue.priority = priority;
        self
    }

    fn build(self) -> Issue {
        self.issue
    }
}

#[cfg(test)]
mod tests {
    use crate::config::ThemeName;

    use super::*;

    fn make_issue(id: &str, identifier: &str, title: &str) -> Issue {
        Issue {
            id: id.to_string(),
            identifier: identifier.to_string(),
            title: title.to_string(),
            priority: Priority::default(),
            priority_label: None,
            state: None,
            assignee: None,
            labels: None,
            description: None,
            created_at: None,
            updated_at: None,
            comments: None,
            project: None,
            cycle: None,
        }
    }

    #[test]
    fn apply_search_filters_by_title() {
        let mut sut = App::new(Theme::from_name(ThemeName::Default));
        sut.issues = vec![
            make_issue("1", "ENG-1", "Fix login bug"),
            make_issue("2", "ENG-2", "Add dashboard"),
            make_issue("3", "ENG-3", "Login page redesign"),
        ];

        sut.search_query = "login".to_string();
        sut.apply_search();

        assert_eq!(sut.filtered_issues, vec![0, 2]);
    }

    #[test]
    fn apply_search_filters_by_identifier() {
        let mut sut = App::new(Theme::from_name(ThemeName::Default));
        sut.issues = vec![
            make_issue("1", "ENG-1", "Fix login bug"),
            make_issue("2", "ENG-2", "Add dashboard"),
            make_issue("3", "ENG-3", "Login page redesign"),
        ];

        sut.search_query = "ENG-2".to_string();
        sut.apply_search();

        assert_eq!(sut.filtered_issues, vec![1]);
    }

    #[test]
    fn apply_search_empty_query_clears() {
        let mut sut = App::new(Theme::from_name(ThemeName::Default));
        sut.issues = vec![
            make_issue("1", "ENG-1", "Fix login bug"),
            make_issue("2", "ENG-2", "Add dashboard"),
            make_issue("3", "ENG-3", "Login page redesign"),
        ];

        sut.search_query = "".to_string();
        sut.apply_search();

        assert!(sut.filtered_issues.is_empty());
    }

    #[test]
    fn apply_search_resets_index() {
        let mut sut = App::new(Theme::from_name(ThemeName::Default));
        sut.issues = vec![
            make_issue("1", "ENG-1", "Fix login bug"),
            make_issue("2", "ENG-2", "Add dashboard"),
            make_issue("3", "ENG-3", "Login page redesign"),
        ];

        sut.selected_issue_index = 1;
        sut.search_query = "login".to_string();
        sut.apply_search();

        assert_eq!(sut.selected_issue_index, 0);
    }

    #[test]
    fn visible_issues_status_filter() {
        let mut sut = App::new(Theme::from_name(ThemeName::Default));
        sut.issues = vec![
            IssueBuilder::new("1", "ENG-1", "Fix login bug")
                .state("In Progress")
                .build(),
            IssueBuilder::new("2", "ENG-2", "Add dashboard")
                .state("Done")
                .build(),
            IssueBuilder::new("3", "ENG-3", "Login page redesign")
                .state("In Progress")
                .build(),
        ];

        sut.filters.status = Some("In Progress".to_string());
        let actual = sut.visible_issues();

        assert_eq!(actual.len(), 2);
        assert_eq!(actual[0].identifier, "ENG-1");
        assert_eq!(actual[1].identifier, "ENG-3");
    }

    #[test]
    fn visible_issues_priority_filter() {
        let mut sut = App::new(Theme::from_name(ThemeName::Default));

        sut.issues = vec![
            IssueBuilder::new("1", "ENG-1", "Fix login bug")
                .priority(Priority::Low)
                .build(),
            IssueBuilder::new("2", "ENG-2", "Add dashboard")
                .priority(Priority::High)
                .build(),
            IssueBuilder::new("3", "ENG-3", "Login page redesign")
                .priority(Priority::High)
                .build(),
        ];

        sut.filters.priority = Some(Priority::High);
        let actual = sut.visible_issues();

        assert_eq!(actual.len(), 2);
        assert_eq!(actual[0].identifier, "ENG-2");
        assert_eq!(actual[1].identifier, "ENG-3");
    }

    #[test]
    fn visible_issues_search_and_status_combined() {
        let mut sut = App::new(Theme::from_name(ThemeName::Default));

        sut.issues = vec![
            IssueBuilder::new("1", "ENG-1", "Fix login bug")
                .state("In Progress")
                .build(),
            IssueBuilder::new("2", "ENG-2", "Add login feature")
                .state("Todo")
                .build(),
            IssueBuilder::new("3", "ENG-3", "Fix payment bug")
                .state("In Progress")
                .build(),
            IssueBuilder::new("4", "ENG-4", "Update docs")
                .state("In Progress")
                .build(),
        ];

        sut.filters.status = Some("In Progress".to_string());
        sut.search_query = "login".to_string();
        sut.apply_search();

        let actual = sut.visible_issues();

        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0].identifier, "ENG-1");
    }
}
