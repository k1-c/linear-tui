//! Where the user is: the screen, the destination behind it, and the way back.

use super::*;

#[derive(Debug)]
pub struct Navigation {
    pub screen: Screen,
    /// The sidebar destination the content pane belongs to.
    pub dest: Nav,
    /// The selected team, by its position in `Store::teams`.
    pub team: usize,
    /// Screen to return to when the detail view is closed.
    pub detail_return: Screen,
    /// Destination to return to with it — an issue opened from Favorites
    /// points the sidebar at the favorite while it is open.
    pub detail_return_nav: Nav,
    /// The project whose page is open, or was last.
    pub current_project: Option<Project>,
    /// The cycle whose page is open, or was last.
    pub current_cycle: Option<Cycle>,
    /// Set while the team's list shows workspace-wide search results for
    /// this term instead of the team's issues.
    pub global_search: Option<String>,
}

impl Default for Navigation {
    fn default() -> Self {
        Self {
            screen: Screen::IssueList,
            dest: Nav::Team(0, TeamSection::Issues),
            team: 0,
            detail_return: Screen::IssueList,
            detail_return_nav: Nav::Team(0, TeamSection::Issues),
            current_project: None,
            current_cycle: None,
            global_search: None,
        }
    }
}

impl App {
    /// Open whatever the cursor is on: a project, cycle, view, or issue.
    pub fn open_selected(&mut self) {
        match self.nav.screen {
            Screen::ProjectList => self.open_project_detail(),
            Screen::CycleList => self.open_cycle_detail(),
            Screen::ViewList => self.open_selected_view(),
            // The cursor counts rows in display order, which grouping
            // reorders, so the issue is looked up in that order too.
            Screen::IssueList | Screen::ProjectDetail | Screen::CycleDetail => {
                self.open_issue_detail()
            }
            Screen::IssueDetail => {}
        }
    }

    /// Queue the fetch that populates the current destination.
    pub fn reload_current_tab(&mut self) {
        match self.nav.dest {
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
                        preset: self.view.lists[IssueSource::Team].preset,
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
        self.nav.global_search = None;
        self.outbox.prefetched.clear();
        match self.nav.dest {
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
        if self.nav.screen == Screen::ProjectDetail {
            self.store.issues[IssueSource::Project].loaded = false;
        }
        if self.nav.screen == Screen::CycleDetail {
            self.store.issues[IssueSource::Cycle].loaded = false;
        }
        self.reload_current_tab();
        self.queue_detail_fetches();
    }

    /// Queue whatever the current screen still needs but has not fetched yet.
    pub fn queue_detail_fetches(&mut self) {
        match self.nav.screen {
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
                    && let Some(project) = &self.nav.current_project
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
                    && let Some(cycle) = &self.nav.current_cycle
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

    /// Go to a sidebar destination, fetching whatever it needs.
    pub fn activate(&mut self, nav: Nav) {
        if let Nav::Favorite(index) = nav {
            return self.open_favorite(index);
        }
        // A team destination also selects that team, which is how the sidebar
        // crosses team boundaries without the team-switch popup.
        if let Nav::Team(index, _) = nav
            && index != self.nav.team
            && index < self.store.teams.len()
        {
            self.select_team_index(index);
        }
        let screen = self.screen_for(nav);
        if self.nav.dest == nav && self.nav.screen == screen {
            return;
        }
        if screen == Screen::ViewList && self.nav.dest != nav {
            // A different Views page lists different views.
            self.view.selected_view_index = 0;
        }
        self.nav.dest = nav;
        self.nav.screen = screen;
        self.view.sidebar.focus = false;

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
            self.view.selected_view_project_index = 0;
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
        self.activate(Nav::Team(self.nav.team, section));
    }

    /// Open the view under the cursor on the saved-view index.
    pub fn open_selected_view(&mut self) {
        if let Some(index) = self
            .listed_views()
            .get(self.view.selected_view_index)
            .copied()
        {
            self.activate(Nav::View(index));
        }
    }

    /// Switch the current team, discarding everything scoped to the old one.
    fn select_team_index(&mut self, index: usize) {
        self.nav.team = index;
        // The old team's cursors mean nothing to the new team's lists.
        self.reset_list(IssueSource::Team);
        self.store.projects.page_info = PageInfo::default();
        self.store.cycles.page_info = PageInfo::default();
        // The old team's projects and cycles would otherwise sit on screen,
        // under the new team's name, until the refetch lands.
        self.store.projects.items.clear();
        self.store.cycles.items.clear();
        self.view.selected_project_index = 0;
        self.view.selected_cycle_index = 0;
        self.view.lists[IssueSource::Team].filters.clear();
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

    pub fn open_issue_detail(&mut self) {
        if let Some(issue) = self.visible_issues().get(self.selected_index()).copied() {
            let issue = issue.clone();
            self.open_issue_from_list(&issue);
        }
    }

    /// Open issue detail from any sub-list (project issues, cycle issues, my issues).
    pub fn open_issue_from_list(&mut self, issue: &Issue) {
        if self.nav.screen != Screen::IssueDetail {
            self.nav.detail_return = self.nav.screen;
            self.nav.detail_return_nav = self.nav.dest;
        }
        self.store.current_issue = Some(issue.clone());
        self.view.detail_scroll = 0;
        self.nav.screen = Screen::IssueDetail;
        self.queue_detail_fetches();
    }

    /// Leave the detail view for wherever it was opened from.
    pub fn close_detail(&mut self) {
        self.nav.screen = self.nav.detail_return;
        self.nav.dest = self.nav.detail_return_nav;
    }

    /// Esc on a project or cycle page: back to the team's list it came from,
    /// or — opened from Favorites, where there is no list behind it — to the
    /// sidebar.
    pub fn leave_container(&mut self) {
        match self.nav.dest {
            Nav::Team(_, TeamSection::Projects) => self.nav.screen = Screen::ProjectList,
            Nav::Team(_, TeamSection::Cycles) => self.nav.screen = Screen::CycleList,
            // A project opened from a project view goes back to the view.
            Nav::View(_) => self.nav.screen = Screen::ProjectList,
            _ => self.focus_sidebar(true),
        }
    }

    /// Step to the neighbouring issue without leaving the detail view — the
    /// `↓`/`↑` pair Linear puts next to the "6 / 203" counter.
    pub fn step_issue(&mut self, delta: isize) {
        if self.nav.screen != Screen::IssueDetail {
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
        let ret = self.nav.detail_return;
        self.open_issue_from_list(&issue);
        self.nav.detail_return = ret;
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

    /// Open a project's page from anywhere, as if picked from the team's
    /// project list — which is where Esc then leads.
    pub fn go_to_project(&mut self, project: Project) {
        self.activate(Nav::Team(self.nav.team, TeamSection::Projects));
        self.open_project(project);
    }

    /// Open a cycle's page from anywhere, over the team's cycle list.
    pub fn go_to_cycle(&mut self, cycle: Cycle) {
        self.activate(Nav::Team(self.nav.team, TeamSection::Cycles));
        self.open_cycle(cycle);
    }

    pub fn open_project_detail(&mut self) {
        if let Some(project) = self.project_rows().get(self.project_cursor()).cloned() {
            self.open_project(project);
        }
    }

    pub(super) fn open_project(&mut self, project: Project) {
        self.nav.current_project = Some(project);
        self.reset_list(IssueSource::Project);
        self.nav.screen = Screen::ProjectDetail;
        self.queue_detail_fetches();
    }

    pub fn open_cycle_detail(&mut self) {
        if let Some(cycle) = self
            .store
            .cycles
            .items
            .get(self.view.selected_cycle_index)
            .cloned()
        {
            self.open_cycle(cycle);
        }
    }

    pub(super) fn open_cycle(&mut self, cycle: Cycle) {
        self.nav.current_cycle = Some(cycle);
        self.reset_list(IssueSource::Cycle);
        self.nav.screen = Screen::CycleDetail;
        self.queue_detail_fetches();
    }

    /// Whether the project list on screen is a saved project view's rather
    /// than the team's.
    pub(super) fn in_project_view(&self) -> bool {
        matches!(self.nav.dest, Nav::View(i) if self.store.custom_views.get(i).is_some_and(|v| ViewKind::of(v) == ViewKind::Projects))
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
            self.view.selected_view_project_index
        } else {
            self.view.selected_project_index
        }
    }

    pub(super) fn project_cursor_mut(&mut self) -> &mut usize {
        if self.in_project_view() {
            &mut self.view.selected_view_project_index
        } else {
            &mut self.view.selected_project_index
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

    /// The team whose views a Views page shows, or `None` for the workspace
    /// page.
    fn views_scope(&self) -> Option<&TeamId> {
        match self.nav.dest {
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
            .filter(|(_, v)| ViewKind::of(v) == self.view.view_kind)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn set_view_kind(&mut self, kind: ViewKind) {
        if self.view.view_kind != kind {
            self.view.view_kind = kind;
            self.view.selected_view_index = 0;
        }
    }

    pub fn cycle_view_kind(&mut self) {
        self.set_view_kind(match self.view.view_kind {
            ViewKind::Issues => ViewKind::Projects,
            ViewKind::Projects => ViewKind::Issues,
        });
    }
}
