//! The pick-one popups: team switcher, filters, grouping, and the change
//! popups. Each narrows as you type, like the command palette, and one opened
//! from the palette steps back to it on Esc.

use super::*;
use crate::fuzzy;

impl App {
    /// Show `popup` with its cursor on row `index` and an empty query.
    fn show_popup(&mut self, popup: Popup, index: usize) {
        self.view.popup = popup;
        self.view.popup_index = index;
        self.view.popup_query.clear();
    }

    pub fn open_team_select(&mut self) {
        self.show_popup(Popup::TeamSelect, self.nav.team);
    }

    /// Pick a team from the switcher and go to it, keeping the page (Issues,
    /// Cycles, Projects) when already on one of the team's pages.
    pub fn select_team(&mut self) {
        let Some(index) = self.popup_choice() else {
            return;
        };
        self.close_popup();
        let section = match self.nav.dest {
            Nav::Team(_, section) => section,
            _ => TeamSection::Issues,
        };
        self.activate(Nav::Team(index, section));
    }

    pub fn open_filter(&mut self) {
        for team_id in self.list_team_ids() {
            self.ensure_team_context(team_id);
        }
        self.show_popup(Popup::Filter(FilterKind::Status), 0);
    }

    /// Pick a grouping outright rather than cycling to it.
    pub fn open_group_by(&mut self) {
        let index = GroupBy::ALL
            .iter()
            .position(|g| *g == self.view.group_by)
            .unwrap_or(0);
        self.show_popup(Popup::GroupBy, index);
    }

    pub fn open_status_change(&mut self) {
        if let Some(issue) = self.focused_issue() {
            self.open_change_popup(Popup::StatusChange(issue.id.clone()));
        }
    }

    pub fn open_priority_change(&mut self) {
        if let Some(issue) = self.focused_issue() {
            let index = issue.priority.as_index();
            self.show_popup(Popup::PriorityChange(issue.id.clone()), index);
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
        self.show_popup(popup, 0);
        if let Some(team_id) = self.popup_team_id().cloned() {
            self.ensure_team_context(team_id);
        }
        self.view.popup_index = self.popup_initial_index();
    }

    /// Where a change popup starts: on the issue's current value, so Enter is
    /// a no-op rather than a surprise. Once a query narrows the list, the top
    /// match.
    pub(super) fn popup_initial_index(&self) -> usize {
        let Some(issue) = self.popup_issue() else {
            return 0;
        };
        if !self.view.popup_query.is_empty() {
            return 0;
        }
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
            Popup::GroupBy => self.apply_group_by_selection(),
            // The palette's entries come from the binding table; the
            // `palette` module runs them.
            Popup::Palette | Popup::None => {}
        }
    }

    pub fn apply_status_selection(&mut self) {
        // Nothing to pick while the issue's team loads; the popup stays open.
        let Some(state) = self
            .popup_choice()
            .and_then(|i| self.popup_states().get(i))
            .cloned()
        else {
            return;
        };
        if let Popup::StatusChange(issue_id) = self.take_popup() {
            let request = usecase::issue::set_status(&mut self.store, &issue_id, state);
            self.request(request);
        }
    }

    pub fn apply_priority_selection(&mut self) {
        let Some(index) = self.popup_choice() else {
            return;
        };
        if let Popup::PriorityChange(issue_id) = self.take_popup() {
            let priority = Priority::from_index(index);
            let request = usecase::issue::set_priority(&mut self.store, &issue_id, priority);
            self.request(request);
        }
    }

    pub fn apply_assignee_selection(&mut self) {
        let assignee = match self.popup_choice() {
            None => return,
            Some(0) => None, // Unassign
            // Only Unassign can be picked while the issue's team loads.
            Some(i) => match self.popup_members().get(i - 1) {
                Some(member) => Some(member.clone()),
                None => return,
            },
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
        let Some(choice) = self.popup_choice() else {
            return;
        };
        match kind {
            FilterKind::Status => {
                if choice == 0 {
                    self.list_mut().filters.status = None;
                } else if let Some(state) = self.filter_states().get(choice - 1) {
                    let name = state.name.clone();
                    self.list_mut().filters.status = Some(name);
                }
                // The second question keeps the way back to the palette.
                self.view.popup = Popup::Filter(FilterKind::Priority);
                self.view.popup_index = 0;
                self.view.popup_query.clear();
            }
            FilterKind::Priority => {
                let priority = match choice {
                    0 => None,
                    n => Some(Priority::from_index(n)),
                };
                self.list_mut().filters.priority = priority;
                self.close_popup();
                *self.selected_index_mut() = 0;
            }
        }
    }

    pub fn apply_group_by_selection(&mut self) {
        let Some(group_by) = self.popup_choice().and_then(|i| GroupBy::ALL.get(i)) else {
            return;
        };
        let group_by = *group_by;
        self.close_popup();
        self.set_group_by(group_by);
    }

    pub fn clear_filters(&mut self) {
        self.list_mut().filters.clear();
        *self.selected_index_mut() = 0;
    }

    pub fn close_popup(&mut self) {
        self.view.popup = Popup::None;
        self.view.popup_from_palette = false;
    }

    /// Esc on a popup: back to the palette it was opened from, or closed.
    pub fn popup_escape(&mut self) {
        if self.view.popup_from_palette {
            self.open_palette();
        } else {
            self.close_popup();
        }
    }

    /// Type into the popup's query; the list narrows to what matches.
    pub fn popup_type(&mut self, c: char) {
        self.edit_popup_query(|q| q.insert(c));
    }

    /// Backspace: erase from the query, or — with nothing left to erase —
    /// step back to the palette the popup was opened from.
    pub fn popup_erase(&mut self) {
        if self.view.popup_query.is_empty() {
            if self.view.popup_from_palette {
                self.open_palette();
            }
            return;
        }
        self.edit_popup_query(Input::backspace);
    }

    pub fn edit_popup_query(&mut self, change: impl FnOnce(&mut Input)) {
        change(&mut self.view.popup_query);
        self.view.popup_index = 0;
    }

    /// Every row of the open popup as text, for matching against the query.
    fn popup_labels(&self) -> Vec<String> {
        let priorities = |any: bool| {
            let any = any.then(|| "Any priority".to_string());
            let levels = (1..=5).map(|i| Priority::from_index(i).label().to_string());
            any.into_iter().chain(levels)
        };
        match &self.view.popup {
            Popup::TeamSelect => self
                .store
                .teams
                .iter()
                .map(|t| format!("{} {}", t.name, t.key))
                .collect(),
            Popup::Filter(FilterKind::Status) => std::iter::once("Any status".to_string())
                .chain(self.filter_states().iter().map(|s| s.name.clone()))
                .collect(),
            Popup::Filter(FilterKind::Priority) => priorities(true).collect(),
            Popup::StatusChange(_) => self.popup_states().iter().map(|s| s.name.clone()).collect(),
            Popup::PriorityChange(_) => Priority::ALL
                .iter()
                .map(|p| p.label().to_string())
                .collect(),
            Popup::AssigneeChange(_) => std::iter::once("No assignee unassign".to_string())
                .chain(self.popup_members().iter().map(|u| {
                    let display = u.display_name.as_deref().unwrap_or_default();
                    format!("{} {display}", u.name)
                }))
                .collect(),
            Popup::GroupBy => GroupBy::ALL.iter().map(|g| g.label().to_string()).collect(),
            Popup::Palette | Popup::None => Vec::new(),
        }
    }

    /// The rows the query leaves, as indices into the popup's full list —
    /// best match first, or every row in order when nothing is typed.
    pub fn popup_rows(&self) -> Vec<usize> {
        let labels = self.popup_labels();
        let query = self.view.popup_query.value.trim();
        if query.is_empty() {
            return (0..labels.len()).collect();
        }
        let mut scored: Vec<(i32, usize)> = labels
            .iter()
            .enumerate()
            .filter_map(|(i, label)| fuzzy::score(query, label).map(|m| (m.score, i)))
            .collect();
        scored.sort_by_key(|(score, _)| -score);
        scored.into_iter().map(|(_, i)| i).collect()
    }

    /// The highlighted row, as an index into the popup's full list.
    fn popup_choice(&self) -> Option<usize> {
        self.popup_rows().get(self.view.popup_index).copied()
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
        self.view.popup_from_palette = false;
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

    /// How many rows the popup offers under its query.
    pub fn popup_list_len(&self) -> usize {
        self.popup_rows().len()
    }
}
