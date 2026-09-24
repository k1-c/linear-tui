//! Folding answers from Linear into the store, and deciding which of
//! them still belong on screen.

use super::*;

/// Identifies one of the paginated sub-lists, for index clamping.
#[derive(Debug, Clone, Copy)]
enum Field {
    Projects,
    Cycles,
}

impl App {
    pub fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::Teams(teams) => {
                if let Some(default_team) = &self.default_team
                    && let Some(idx) = teams
                        .iter()
                        .position(|t| t.name == *default_team || t.key == *default_team)
                {
                    self.nav.team = idx;
                }
                self.store.teams = teams;
                self.nav.dest = Nav::Team(self.nav.team, TeamSection::Issues);
                if let Some(team_id) = self.team_id() {
                    self.ensure_team_context(team_id.clone());
                    self.request(Request::Issues {
                        team_id,
                        after: None,
                        preset: self.view.lists[IssueSource::Team].preset,
                    });
                }
            }
            Message::Viewer(id) => {
                self.store.viewer_id = Some(id);
                if self.nav.dest == Nav::MyIssues {
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
                self.outbox.team_contexts.remove(&team_id);
                let waiting = self.popup_team_id() == Some(&team_id);
                self.store
                    .team_contexts
                    .insert(team_id, TeamContext { states, members });
                // A popup opened while this was loading starts on the
                // issue's current value, as it would have if it were cached.
                if waiting {
                    self.view.popup_index = self.popup_initial_index();
                }
            }
            Message::Issues {
                team_id,
                preset,
                page,
            } => {
                // A page for a team or preset the user has since left would
                // file, say, another team's backlog under this team's Active.
                if !self.is_current_team(&team_id)
                    || preset != self.view.lists[IssueSource::Team].preset
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
            Message::PaletteResults { seq, issues } => self.accept_palette_results(seq, issues),
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
                self.nav.dest = Nav::Team(self.nav.team, TeamSection::Issues);
                self.nav.screen = Screen::IssueList;
                self.store.issues[IssueSource::Team].items = issues;
                self.store.issues[IssueSource::Team].page_info = PageInfo::default();
                self.view.lists[IssueSource::Team].filters.clear();
                // Search answers "where is it", done or not; the Active slice
                // would quietly hide half the matches. Set without refetching,
                // which would replace the results with the team's list.
                self.view.lists[IssueSource::Team].preset = Preset::All;
                self.view.lists[IssueSource::Team].search.clear();
                self.view.lists[IssueSource::Team].selected = 0;
                self.nav.global_search = Some(term);
            }
            Message::ViewProjects { view_id, page } => {
                if !self.accepts_view_page(&view_id, page.append, ViewKind::Projects) {
                    return;
                }
                let append = page.append;
                self.store.view_projects.accept(page);
                if !append {
                    self.view.selected_view_project_index = 0;
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
                let open = match self.nav.dest {
                    Nav::View(i) => self.store.custom_views.get(i).map(|v| v.id.clone()),
                    _ => None,
                };
                self.store.set_custom_views(views);
                if let Some(id) = open {
                    match self.store.custom_views.iter().position(|v| v.id == id) {
                        Some(i) => self.nav.dest = Nav::View(i),
                        None => self.activate(Nav::Views),
                    }
                }
                self.view.selected_view_index = self
                    .view
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
                if self.nav.current_project.as_ref().map(|p| &p.id) != Some(&project_id) {
                    return;
                }
                self.accept_issue_page(IssueSource::Project, page);
            }
            Message::CycleIssues { cycle_id, page } => {
                if self.nav.current_cycle.as_ref().map(|c| &c.id) != Some(&cycle_id) {
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
                if self.issue_source() == IssueSource::Team && self.nav.screen == Screen::IssueList
                {
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
                    self.outbox.prefetched.remove(cursor);
                }
                // A palette search that failed is not worth an error over
                // the palette; the local matches are still there.
                if let Request::PaletteSearch { seq, .. } = request.as_ref() {
                    if *seq == self.view.palette.seq {
                        self.view.palette.searching = false;
                    }
                    self.set_status(format!("Search failed: {error}"));
                    return;
                }
                // Reopening the popup asks again.
                if let Request::TeamContext { team_id } = request.as_ref() {
                    self.outbox.team_contexts.remove(team_id);
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
        matches!(self.nav.dest, Nav::View(i) if self.store.custom_views.get(i).is_some_and(|v| &v.id == view_id))
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
                &mut self.view.selected_project_index,
            ),
            Field::Cycles => (
                self.store.cycles.items.len(),
                &mut self.view.selected_cycle_index,
            ),
        };
        *index = (*index).min(len.saturating_sub(1));
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
}
