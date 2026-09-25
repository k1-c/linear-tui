//! Intents: what a key or a click asks for, resolved against what is on
//! screen and handed to a use case.

use super::*;
use crate::usecase::issue::CopyField;

impl App {
    /// Linear's `I` — assign the focused issue to the current user.
    pub fn assign_to_me(&mut self) {
        let Some(issue_id) = self.focused_issue_id() else {
            return;
        };
        let outcome = usecase::issue::assign_to_me(&mut self.store, &issue_id);
        self.run(outcome);
    }

    /// Linear's `Shift+1`…`Shift+4` / `Shift+0` — set a priority without the menu.
    pub fn set_priority(&mut self, priority: Priority) {
        if let Some(issue_id) = self.focused_issue_id() {
            let request = usecase::issue::set_priority(&mut self.store, &issue_id, priority);
            self.request(request);
        }
    }

    pub fn start_comment(&mut self) {
        if self.focused_issue().is_some() {
            self.view.input_mode = InputMode::Comment;
            self.view.comment.clear();
        }
    }

    pub fn submit_comment(&mut self) {
        if let Some(issue_id) = self.focused_issue_id() {
            let body = std::mem::take(&mut self.view.comment.value);
            if let Some(request) = usecase::issue::comment(&mut self.store, &issue_id, body) {
                self.request(request);
            }
        }
        self.view.input_mode = InputMode::Normal;
        self.view.comment.clear();
    }

    pub fn cancel_comment(&mut self) {
        self.view.input_mode = InputMode::Normal;
        self.view.comment.clear();
    }

    /// Open the search box on the list on screen.
    pub fn start_search(&mut self) {
        self.view.input_mode = InputMode::Search;
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
        let request = usecase::issue::search(&term, team_id);
        self.send(request);
        self.view.input_mode = InputMode::Normal;
    }

    /// Re-run the live filter. Matching happens inside [`Self::sections`], so
    /// this only has to put the cursor back at the top of what is left.
    pub fn apply_search(&mut self) {
        *self.selected_index_mut() = 0;
    }

    /// Close the search box, keeping its query as the list's filter.
    pub fn finish_search(&mut self) {
        self.view.input_mode = InputMode::Normal;
        self.apply_search();
    }

    /// Close the search box and drop its query.
    pub fn cancel_search(&mut self) {
        self.view.input_mode = InputMode::Normal;
        self.clear_search();
    }

    pub fn clear_search(&mut self) {
        let list = self.list_mut();
        list.search.clear();
        list.selected = 0;
        // Leaving a workspace search returns the list to the team's own issues.
        if self.issue_source() == IssueSource::Team && self.nav.global_search.take().is_some() {
            self.view.lists[IssueSource::Team].preset = Preset::Active;
            self.reload_current_tab();
        }
    }

    /// Open the focused issue (or the selected project) on linear.app.
    pub fn open_in_browser(&mut self) {
        match self.nav.screen {
            Screen::ProjectList => {
                let url = self
                    .project_rows()
                    .get(self.project_cursor())
                    .and_then(|p| p.url.clone());
                self.run(usecase::project::open_url(url));
            }
            Screen::ProjectDetail if self.focused_issue().is_none() => {
                let url = self
                    .nav
                    .current_project
                    .as_ref()
                    .and_then(|p| p.url.clone());
                self.run(usecase::project::open_url(url));
            }
            _ => {
                let url = self.focused_issue().and_then(|i| i.url.clone());
                self.run(usecase::issue::open_url(url));
            }
        }
    }

    /// Copy a field of the focused issue. The main loop emits the OSC 52
    /// sequence for the clipboard.
    fn copy(&mut self, field: CopyField) {
        match usecase::issue::copy(self.focused_issue(), field) {
            Ok(text) => {
                self.set_status(format!("Copied {}: {text}", field.label()));
                self.outbox.clipboard = Some(text);
            }
            Err(refusal) => self.set_status(refusal.to_string()),
        }
    }

    pub fn copy_identifier(&mut self) {
        self.copy(CopyField::Identifier);
    }

    pub fn copy_url(&mut self) {
        self.copy(CopyField::Url);
    }

    pub fn copy_branch_name(&mut self) {
        self.copy(CopyField::BranchName);
    }

    /// Open the issue-creation form for the current team.
    pub fn start_new_issue(&mut self) {
        if self.team_id().is_none() {
            self.set_status("Select a team first");
            return;
        }
        self.view.new_issue = Some(NewIssueForm::default());
        self.view.input_mode = InputMode::NewIssue;
    }

    pub fn cancel_new_issue(&mut self) {
        self.view.new_issue = None;
        self.view.input_mode = InputMode::Normal;
    }

    pub fn new_issue_cycle_field(&mut self, forward: bool) {
        if let Some(form) = &mut self.view.new_issue {
            form.field = if forward {
                form.field.next()
            } else {
                form.field.prev()
            };
        }
    }

    pub fn new_issue_cycle_priority(&mut self, delta: isize) {
        if let Some(form) = &mut self.view.new_issue {
            let next = (form.priority.as_index() as isize + delta).rem_euclid(5);
            form.priority = Priority::from_index(next as usize);
        }
    }

    pub fn submit_new_issue(&mut self) {
        let (Some(form), Some(team_id)) = (&self.view.new_issue, self.team_id()) else {
            return;
        };
        let draft = usecase::issue::Draft {
            title: form.title.value.clone(),
            description: form.description.value.clone(),
            priority: form.priority,
        };
        if self.run(usecase::issue::create(team_id, draft)) {
            self.set_status("Creating issue…");
            self.cancel_new_issue();
        }
    }
}
