//! The pick-one popups: team switcher, filters, and the change popups.

use super::*;

impl App {
    pub fn open_team_select(&mut self) {
        self.view.popup = Popup::TeamSelect;
        self.view.popup_index = self.nav.team;
    }

    /// Pick a team from the switcher and go to it, keeping the page (Issues,
    /// Cycles, Projects) when already on one of the team's pages.
    pub fn select_team(&mut self) {
        self.view.popup = Popup::None;
        if self.view.popup_index >= self.store.teams.len() {
            return;
        }
        let section = match self.nav.dest {
            Nav::Team(_, section) => section,
            _ => TeamSection::Issues,
        };
        self.activate(Nav::Team(self.view.popup_index, section));
    }

    pub fn open_filter(&mut self) {
        for team_id in self.list_team_ids() {
            self.ensure_team_context(team_id);
        }
        self.view.popup = Popup::Filter(FilterKind::Status);
        self.view.popup_index = 0;
    }

    pub fn open_status_change(&mut self) {
        if let Some(issue) = self.focused_issue() {
            self.open_change_popup(Popup::StatusChange(issue.id.clone()));
        }
    }

    pub fn open_priority_change(&mut self) {
        if let Some(issue) = self.focused_issue() {
            let index = issue.priority.as_index();
            self.view.popup = Popup::PriorityChange(issue.id.clone());
            self.view.popup_index = index;
        }
    }

    pub fn open_assignee_change(&mut self) {
        if let Some(issue) = self.focused_issue() {
            self.open_change_popup(Popup::AssigneeChange(issue.id.clone()));
        }
    }

    /// Open a status or assignee popup, fetching the issue's team first if
    /// its states and members have not been loaded yet.
    fn open_change_popup(&mut self, popup: Popup) {
        self.view.popup = popup;
        if let Some(team_id) = self.popup_team_id().cloned() {
            self.ensure_team_context(team_id);
        }
        self.view.popup_index = self.popup_initial_index();
    }

    /// Where a change popup starts: on the issue's current value, so Enter is
    /// a no-op rather than a surprise.
    pub(super) fn popup_initial_index(&self) -> usize {
        let Some(issue) = self.popup_issue() else {
            return 0;
        };
        match self.view.popup {
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

    /// The team `issue` belongs to. An issue that does not say is taken to be
    /// the selected team's.
    pub fn issue_team_id<'a>(&'a self, issue: &'a Issue) -> Option<&'a TeamId> {
        self.store
            .issue_team_id(issue, self.current_team().map(|t| &t.id))
    }

    /// Ask for a team's states and members, unless they are loaded or on the way.
    pub(super) fn ensure_team_context(&mut self, team_id: TeamId) {
        if self.store.team_contexts.contains_key(&team_id)
            || !self.outbox.team_contexts.insert(team_id.clone())
        {
            return;
        }
        self.request(Request::TeamContext { team_id });
    }

    /// The team whose states or members the open change popup offers.
    pub(super) fn popup_team_id(&self) -> Option<&TeamId> {
        match self.view.popup {
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
        match self.view.popup {
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

    /// Apply the highlighted popup entry.
    pub fn apply_popup(&mut self) {
        match self.view.popup {
            Popup::TeamSelect => self.select_team(),
            Popup::Filter(_) => self.apply_filter_selection(),
            Popup::StatusChange(_) => self.apply_status_selection(),
            Popup::PriorityChange(_) => self.apply_priority_selection(),
            Popup::AssigneeChange(_) => self.apply_assignee_selection(),
            Popup::None => {}
        }
    }

    pub fn apply_status_selection(&mut self) {
        // Nothing to pick while the issue's team loads; the popup stays open.
        let Some(state) = self.popup_states().get(self.view.popup_index).cloned() else {
            return;
        };
        if let Popup::StatusChange(issue_id) = self.take_popup() {
            let request = usecase::issue::set_status(&mut self.store, &issue_id, state);
            self.request(request);
        }
    }

    pub fn apply_priority_selection(&mut self) {
        if let Popup::PriorityChange(issue_id) = self.take_popup() {
            let priority = Priority::from_index(self.view.popup_index);
            let request = usecase::issue::set_priority(&mut self.store, &issue_id, priority);
            self.request(request);
        }
    }

    pub fn apply_assignee_selection(&mut self) {
        let assignee = if self.view.popup_index == 0 {
            None // Unassign
        } else {
            // Only Unassign can be picked while the issue's team loads.
            let Some(member) = self.popup_members().get(self.view.popup_index - 1).cloned() else {
                return;
            };
            Some(member)
        };
        if let Popup::AssigneeChange(issue_id) = self.take_popup() {
            let request = usecase::issue::set_assignee(&mut self.store, &issue_id, assignee);
            self.request(request);
        }
    }

    pub fn apply_filter_selection(&mut self) {
        let Popup::Filter(kind) = self.view.popup else {
            return;
        };
        match kind {
            FilterKind::Status => {
                if self.view.popup_index == 0 {
                    self.list_mut().filters.status = None;
                } else if let Some(state) = self.filter_states().get(self.view.popup_index - 1) {
                    let name = state.name.clone();
                    self.list_mut().filters.status = Some(name);
                }
                self.view.popup = Popup::Filter(FilterKind::Priority);
                self.view.popup_index = 0;
            }
            FilterKind::Priority => {
                let priority = match self.view.popup_index {
                    0 => None,
                    n => Some(Priority::from_index(n)),
                };
                self.list_mut().filters.priority = priority;
                self.view.popup = Popup::None;
                *self.selected_index_mut() = 0;
            }
        }
    }

    pub fn clear_filters(&mut self) {
        self.list_mut().filters.clear();
        *self.selected_index_mut() = 0;
    }

    pub fn close_popup(&mut self) {
        self.view.popup = Popup::None;
    }

    /// The issue the open change popup acts on.
    pub fn popup_issue(&self) -> Option<&Issue> {
        let id = match &self.view.popup {
            Popup::StatusChange(id) | Popup::PriorityChange(id) | Popup::AssigneeChange(id) => id,
            _ => return None,
        };
        self.store.issue(id)
    }

    /// Close the popup, handing back what it was open for.
    fn take_popup(&mut self) -> Popup {
        std::mem::replace(&mut self.view.popup, Popup::None)
    }

    pub fn popup_next(&mut self) {
        let max = self.popup_list_len();
        if max > 0 && self.view.popup_index < max - 1 {
            self.view.popup_index += 1;
        }
    }

    pub fn popup_prev(&mut self) {
        if self.view.popup_index > 0 {
            self.view.popup_index -= 1;
        }
    }

    pub fn popup_first(&mut self) {
        self.view.popup_index = 0;
    }

    pub fn popup_last(&mut self) {
        self.view.popup_index = self.popup_list_len().saturating_sub(1);
    }

    /// Pick popup entry `index` outright, as a number key or a click does.
    pub fn popup_pick(&mut self, index: usize) {
        if index < self.popup_list_len() {
            self.view.popup_index = index;
            self.apply_popup();
        }
    }

    pub fn popup_list_len(&self) -> usize {
        match self.view.popup {
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
}
