//! Folding answers from Linear into the store, and deciding which of
//! them still belong on screen.

use super::*;
use crate::core::store::{List, ListOf};
use crate::core::usecase::project::Projects;

/// Identifies one of the paginated sub-lists, for index clamping.
#[derive(Debug, Clone, Copy)]
enum Field {
    Projects,
    Cycles,
}

impl App {
    pub fn handle_message(&mut self, msg: Message) {
        self.fold_message(msg);
        // Each answer may be what a restore was waiting for.
        self.advance_restore();
    }

    fn fold_message(&mut self, msg: Message) {
        match msg {
            Message::Teams(teams) => {
                let remembered = self.restored_team_id().cloned();
                let preferred = usecase::team::pick_initial(
                    &teams,
                    remembered.as_ref(),
                    self.default_team.as_deref(),
                );
                if let Some(idx) = preferred {
                    self.nav.team = idx;
                }
                self.store.teams = teams;
                let Some(team_id) = self.team_id() else {
                    return;
                };
                let request = usecase::team::ensure_context(&mut self.store, team_id.clone());
                self.send(request);
                // A restore opens its own first page.
                if self.restoring_destination() {
                    return;
                }
                self.nav.dest = Nav::Team(self.nav.team, TeamSection::Issues);
                let preset = self.view.lists[IssueSource::Team].preset;
                let open = usecase::issue::open_team_issues(&mut self.store, team_id, preset);
                self.open_issues(IssueSource::Team, open);
            }
            Message::Viewer { id, organization } => {
                usecase::user::take_viewer(&mut self.store, id, organization);
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
                let waiting = self.popup_team_id() == Some(&team_id);
                usecase::team::context_arrived(&mut self.store, team_id, states, members);
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
                let of = ListOf::TeamIssues { team_id, preset };
                if self.accept_issue_page(IssueSource::Team, &of, page) {
                    self.clear_status();
                }
            }
            Message::MyIssues(page) => {
                let Some(of) = self.store.issues[IssueSource::My].of.clone() else {
                    return;
                };
                if self.accept_issue_page(IssueSource::My, &of, page) {
                    self.clear_status();
                }
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
                let Some(team) = self.team_id() else {
                    return;
                };
                usecase::issue::take_search_results(&mut self.store, team, issues);
                self.nav.dest = Nav::Team(self.nav.team, TeamSection::Issues);
                self.nav.screen = Screen::IssueList;
                let list = &mut self.view.lists[IssueSource::Team];
                list.filters.clear();
                // The results are the team's All slice; set without refetching,
                // which would replace them with the team's list.
                list.preset = Preset::All;
                list.search.clear();
                list.selected = 0;
                self.nav.global_search = Some(term);
            }
            Message::ViewProjects { view_id, page } => {
                let append = page.append;
                let of = ListOf::ViewProjects(view_id);
                if !usecase::project::take_page(&mut self.store, Projects::View, &of, page) {
                    return;
                }
                if !append {
                    self.view.selected_view_project_index = 0;
                }
                self.clear_status();
            }
            Message::Projects { team_id, page } => {
                let of = ListOf::TeamProjects(team_id);
                if !usecase::project::take_page(&mut self.store, Projects::Team, &of, page) {
                    return;
                }
                self.clamp(Field::Projects);
                self.clear_status();
            }
            Message::Cycles { team_id, page } => {
                let of = ListOf::TeamCycles(team_id);
                if !usecase::cycle::take_page(&mut self.store, &of, page) {
                    return;
                }
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
                usecase::view::take_views(&mut self.store, views);
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
            Message::Favorites(favorites) => {
                usecase::favorite::take_favorites(&mut self.store, favorites)
            }
            Message::ViewIssues { view_id, page } => {
                let of = ListOf::ViewIssues(view_id);
                if self.accept_issue_page(IssueSource::View, &of, page) {
                    self.clear_status();
                }
            }
            Message::IssueDetail(issue) => {
                let id = issue.id.clone();
                let adopted = usecase::issue::take_detail(&mut self.store, *issue);
                self.restored_issue_loaded(&id, adopted);
            }
            Message::ProjectIssues { project_id, page } => {
                let of = ListOf::ProjectIssues(project_id);
                self.accept_issue_page(IssueSource::Project, &of, page);
            }
            Message::CycleIssues { cycle_id, page } => {
                let of = ListOf::CycleIssues(cycle_id);
                self.accept_issue_page(IssueSource::Cycle, &of, page);
            }
            Message::IssueCreated { team_id, issue } => {
                self.set_status(format!("Created {}", issue.identifier));
                // Show it with the cursor on it — found by id, since grouping
                // decides where in the list it lands.
                let id = issue.id.clone();
                if usecase::issue::created(&mut self.store, &team_id, *issue)
                    && self.issue_source() == IssueSource::Team
                    && self.nav.screen == Screen::IssueList
                {
                    self.restore_issue_selection(Some(&id));
                }
            }
            // Kept for nothing yet: the labels are asked for by the
            // headless commands.
            Message::Labels { .. } => {}
            Message::CommentPosted { issue_id, .. } => {
                self.set_status("Comment posted");
                // Its thread was dropped when it was sent; read it back if it
                // is still the one on screen.
                if self.store.current_issue.as_ref().map(|i| &i.id) == Some(&issue_id) {
                    self.queue_detail_fetches();
                }
            }
            Message::Mutated(what) => {
                self.set_status(what);
                // A posted comment clears the cached thread; pull it back in.
                self.queue_detail_fetches();
            }
            Message::Failed { request, error } => self.request_failed(&request, &error),
        }
    }

    /// Linear, the browser, or herdr could not do what was asked.
    fn request_failed(&mut self, request: &Request, error: &str) {
        if let Request::Issue(usecase::issue::Request::Detail { issue_id }) = request
            && self.restored_issue_failed(issue_id)
        {
            return;
        }
        self.restore_lost(request);
        match request {
            Request::Notes(usecase::notes::Request::Deliver(handoff)) => {
                return self.handoff_failed(handoff, error);
            }
            Request::Agent(_) => {
                return self.set_error(format!("Could not reach herdr: {error}"));
            }
            // A palette search that failed is not worth an error over the
            // palette; the local matches are still there.
            Request::Issue(usecase::issue::Request::QuickSearch { seq, .. }) => {
                if *seq == self.view.palette.seq {
                    self.view.palette.searching = false;
                }
                return self.set_status(format!("Search failed: {error}"));
            }
            // Reopening the popup asks again.
            Request::Team(usecase::team::Request::Context { team_id }) => {
                usecase::team::context_failed(&mut self.store, team_id);
            }
            Request::Issue(change) => {
                if let Some(issue_id) = change.changed_issue() {
                    self.request(usecase::issue::change_refused(issue_id));
                }
            }
            _ => {}
        }
        // A failed page must be retryable.
        if let Some(cursor) = request.cursor() {
            self.store.page_failed(cursor);
        }
        self.set_error(format!(
            "{}: {error}",
            crate::core::message::failure(request)
        ));
    }

    /// Fold a page into one of the issue lists, keeping the cursor on the
    /// issue it was on.
    ///
    /// Grouping decides where each row lands, so even an appended page can
    /// slot issues in above the cursor; restoring by index would quietly move
    /// the selection — and whatever popup is open — to another issue.
    /// Returns whether the page was taken.
    fn accept_issue_page(&mut self, source: IssueSource, of: &ListOf, page: Page<Issue>) -> bool {
        if !self.store.wants_page(List::Issues(source), of) {
            return false;
        }
        let on_screen = source == self.issue_source();
        let keep = if on_screen {
            let restored = (!page.append)
                .then(|| self.take_restored_selection(source))
                .flatten();
            restored.or_else(|| self.selected_issue_id())
        } else {
            None
        };
        let append = page.append;
        usecase::issue::take_page(&mut self.store, source, of, page);
        if on_screen {
            self.restore_issue_selection(keep.as_ref());
        } else if !append {
            *self.selected_index_of(source) = 0;
        }
        true
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
