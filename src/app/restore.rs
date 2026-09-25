//! Reopening a [`ViewSnapshot`] on launch.
//!
//! A snapshot names its page by ID, and the IDs only turn into places once
//! Linear has answered: a team needs the teams, a saved view the views, a
//! project page the team's projects. So a restore is a walk down the path the
//! user took — destination, then the project or cycle page, then the issue —
//! each step taken as soon as what it needs has arrived. Anything that is gone
//! lands on its parent: a deleted view on the Views page, a deleted project on
//! the project list, a deleted issue on the list it was opened from.

use super::*;
use crate::snapshot::{self as snap, ViewSnapshot};

/// What is left of a restore.
#[derive(Debug)]
pub struct Restore {
    pub(super) team: Option<TeamId>,
    /// The destination to open, until it has been.
    pub(super) dest: Option<snap::Destination>,
    search: Option<String>,
    project: Option<ProjectId>,
    cycle: Option<CycleId>,
    issue: Option<snap::IssueRef>,
    /// The restored issue page, until Linear confirms the issue still exists.
    opened_issue: Option<IssueId>,
    /// The row to put the cursor back on in a project, cycle, or view list.
    row: Option<String>,
    /// The issue to put each list's cursor back on once its rows arrive.
    selection: Vec<(IssueSource, IssueId)>,
}

impl Restore {
    fn is_done(&self) -> bool {
        self.dest.is_none()
            && self.project.is_none()
            && self.cycle.is_none()
            && self.issue.is_none()
            && self.opened_issue.is_none()
            && self.row.is_none()
            && self.selection.is_empty()
    }
}

impl App {
    /// Reopen `snapshot` as Linear's answers come in. The list settings take
    /// effect at once: a team list's preset decides what is fetched.
    pub fn restore(&mut self, snapshot: ViewSnapshot) {
        self.view.group_by = snapshot.group_by.into();
        let mut selection = Vec::new();
        for settings in &snapshot.lists {
            let source = IssueSource::from(settings.source);
            let list = &mut self.view.lists[source];
            list.preset = settings.preset.into();
            list.filters.status.clone_from(&settings.status);
            list.filters.priority = settings
                .priority
                .as_deref()
                .and_then(|label| Priority::ALL.into_iter().find(|p| p.label() == label));
            if let Some(issue) = &settings.selected {
                selection.push((source, issue.id.clone()));
            }
        }
        let row = match snapshot.screen {
            snap::Screen::ProjectList | snap::Screen::CycleList | snap::Screen::ViewList => {
                snapshot.selected().map(|row| row.id.clone())
            }
            _ => None,
        };
        self.restore = Some(Restore {
            team: snapshot.team.map(|t| t.id),
            dest: Some(snapshot.destination),
            search: snapshot.search,
            project: snapshot.project.map(|p| p.id),
            cycle: snapshot.cycle.map(|c| c.id),
            issue: snapshot.issue,
            opened_issue: None,
            row,
            selection,
        });
    }

    /// Whether a restore is still finding its way to the destination — in
    /// which case the default first page is not fetched.
    pub(super) fn restoring_destination(&self) -> bool {
        self.restore.as_ref().is_some_and(|r| r.dest.is_some())
    }

    /// The team a restore reopens, by its position in `teams`.
    pub(super) fn restored_team(&self, teams: &[Team]) -> Option<usize> {
        let id = self.restore.as_ref()?.team.as_ref()?;
        teams.iter().position(|t| &t.id == id)
    }

    /// Take the next steps of the restore that what has arrived allows.
    pub(super) fn advance_restore(&mut self) {
        let Some(mut restore) = self.restore.take() else {
            return;
        };
        self.step_restore(&mut restore);
        if !restore.is_done() {
            self.restore = Some(restore);
        }
    }

    fn step_restore(&mut self, r: &mut Restore) {
        if self.store.teams.is_empty() {
            return;
        }
        if let Some(dest) = r.dest.take() {
            match self.resolve_destination(&dest) {
                None => {
                    r.dest = Some(dest);
                    return;
                }
                Some(Ok(nav)) => self.enter(nav),
                Some(Err(parent)) => {
                    self.enter(parent);
                    r.project = None;
                    r.cycle = None;
                    r.issue = None;
                    r.row = None;
                }
            }
            if let Some(term) = r.search.take() {
                let team_id = self.team_id();
                self.request(Request::Search { term, team_id });
            }
            // A favorite opens its own project, cycle, or issue page.
            if let Nav::Favorite(_) = self.nav.dest {
                r.project = None;
                r.cycle = None;
            }
        }

        if let Some(id) = &r.project {
            let rows = if self.in_project_view() {
                &self.store.view_projects
            } else {
                &self.store.projects
            };
            if !rows.loaded {
                return;
            }
            // A project that is gone leaves the list it was on open.
            if let Some(project) = rows.items.iter().find(|p| &p.id == id).cloned() {
                self.open_project(project);
            }
            r.project = None;
        }
        if let Some(id) = &r.cycle {
            if !self.store.cycles.loaded {
                return;
            }
            if let Some(cycle) = self
                .store
                .cycles
                .items
                .iter()
                .find(|c| &c.id == id)
                .cloned()
            {
                self.open_cycle(cycle);
            }
            r.cycle = None;
        }

        if let Some(issue) = r.issue.take()
            && self.store.current_issue.as_ref().map(|i| &i.id) != Some(&issue.id)
        {
            // Just enough to draw the header; the detail fetch fills the rest.
            let stub = Issue {
                id: issue.id.clone(),
                identifier: issue.identifier,
                title: issue.title,
                ..Issue::default()
            };
            self.open_issue_from_list(&stub);
            r.opened_issue = Some(issue.id);
        }

        if let Some(id) = r.row.clone() {
            match self.nav.screen {
                Screen::ProjectList => {
                    let rows = if self.in_project_view() {
                        &self.store.view_projects
                    } else {
                        &self.store.projects
                    };
                    if !rows.loaded {
                        return;
                    }
                    if let Some(index) = rows.items.iter().position(|p| p.id == id.as_str()) {
                        *self.project_cursor_mut() = index;
                    }
                }
                Screen::CycleList => {
                    if !self.store.cycles.loaded {
                        return;
                    }
                    let cycles = &self.store.cycles.items;
                    if let Some(index) = cycles.iter().position(|c| c.id == id.as_str()) {
                        self.view.selected_cycle_index = index;
                    }
                }
                Screen::ViewList => {
                    let views = &self.store.custom_views;
                    if let Some(index) = self
                        .listed_views()
                        .iter()
                        .position(|&i| views[i].id == id.as_str())
                    {
                        self.view.selected_view_index = index;
                    }
                }
                _ => {}
            }
            r.row = None;
        }
    }

    /// The place a snapshot's destination names now: `Ok` when it is still
    /// there, `Err` with its parent when it is gone, `None` while what it
    /// needs has not arrived.
    fn resolve_destination(&self, dest: &snap::Destination) -> Option<Result<Nav, Nav>> {
        let team_issues = Nav::Team(self.nav.team, TeamSection::Issues);
        Some(match dest {
            snap::Destination::MyIssues => Ok(Nav::MyIssues),
            snap::Destination::Views => Ok(Nav::Views),
            snap::Destination::Team { team, section } => self
                .store
                .teams
                .iter()
                .position(|t| t.id == team.id)
                .map(|index| Nav::Team(index, (*section).into()))
                .ok_or(team_issues),
            snap::Destination::View { id, .. } => {
                if !self.store.views_loaded {
                    return None;
                }
                self.store
                    .custom_views
                    .iter()
                    .position(|v| &v.id == id)
                    .map(Nav::View)
                    .ok_or(Nav::Views)
            }
            snap::Destination::Favorite { id, .. } => {
                if !self.store.favorites_loaded {
                    return None;
                }
                self.store
                    .favorites
                    .iter()
                    .position(|f| &f.id == id)
                    .map(Nav::Favorite)
                    .ok_or(team_issues)
            }
        })
    }

    /// Go to `nav` as the first page of the session. The default page is
    /// not fetched while a restore is pending, so arriving somewhere the app
    /// already points at must still fetch it.
    fn enter(&mut self, nav: Nav) {
        let before = (self.nav.dest, self.nav.screen);
        self.activate(nav);
        if (self.nav.dest, self.nav.screen) == before {
            self.reload_current_tab();
        }
    }

    /// The issue to put `source`'s cursor back on, taken once its rows land.
    pub(super) fn take_restored_selection(&mut self, source: IssueSource) -> Option<IssueId> {
        let restore = self.restore.as_mut()?;
        let at = restore.selection.iter().position(|(s, _)| *s == source)?;
        let (_, id) = restore.selection.remove(at);
        if restore.is_done() {
            self.restore = None;
        }
        Some(id)
    }

    /// A list the restore was waiting on could not be loaded: stop waiting
    /// and go to the destination's parent.
    pub(super) fn restore_lost(&mut self, request: &Request) {
        let Some(restore) = &mut self.restore else {
            return;
        };
        match (request, &restore.dest) {
            (Request::CustomViews, Some(snap::Destination::View { .. })) => {
                restore.dest = Some(snap::Destination::Views);
            }
            (Request::Favorites, Some(snap::Destination::Favorite { .. })) => {
                restore.dest = Some(snap::Destination::MyIssues);
                restore.project = None;
                restore.cycle = None;
                restore.issue = None;
            }
            _ => return,
        }
        self.advance_restore();
    }

    /// The restored issue page could not be loaded — the issue was deleted,
    /// or moved out of reach. Go back to the list it was opened from rather
    /// than show a page for nothing. True when that is what happened.
    pub(super) fn restored_issue_failed(&mut self, issue_id: &IssueId) -> bool {
        let Some(restore) = &mut self.restore else {
            return false;
        };
        if restore.opened_issue.as_ref() != Some(issue_id) {
            return false;
        }
        restore.opened_issue = None;
        if restore.is_done() {
            self.restore = None;
        }
        if self.nav.screen == Screen::IssueDetail
            && self.store.current_issue.as_ref().map(|i| &i.id) == Some(issue_id)
        {
            let identifier = self.store.current_issue.take().map(|i| i.identifier);
            self.close_detail();
            if let Some(identifier) = identifier {
                self.set_status(format!("{identifier} is no longer available"));
            }
        }
        true
    }

    /// The user took over: whatever the restore has not reached yet stays
    /// where it is, and a destination it was still waiting for gives way to
    /// the team's issues.
    pub fn cancel_restore(&mut self) {
        let Some(restore) = self.restore.take() else {
            return;
        };
        if restore.dest.is_some() && !self.store.teams.is_empty() {
            self.enter(Nav::Team(self.nav.team, TeamSection::Issues));
        }
    }

    /// The restored issue page loaded; nothing is left to undo.
    pub(super) fn restored_issue_loaded(&mut self, issue_id: &IssueId) {
        if let Some(restore) = &mut self.restore
            && restore.opened_issue.as_ref() == Some(issue_id)
        {
            restore.opened_issue = None;
            if restore.is_done() {
                self.restore = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::snapshot::Origin;

    fn issue(id: &str) -> Issue {
        serde_json::from_str(&format!(
            r#"{{"id":"{id}","identifier":"ENG-{id}","title":"Issue {id}","priority":0}}"#
        ))
        .unwrap()
    }

    fn team(id: &str, key: &str) -> Team {
        serde_json::from_str(&format!(
            r#"{{"id":"{id}","name":"{key} team","key":"{key}"}}"#
        ))
        .unwrap()
    }

    fn view(id: &str) -> CustomView {
        serde_json::from_str(&format!(r#"{{"id":"{id}","name":"View {id}"}}"#)).unwrap()
    }

    fn project(id: &str) -> Project {
        serde_json::from_str(&format!(r#"{{"id":"{id}","name":"Project {id}"}}"#)).unwrap()
    }

    fn origin() -> Origin {
        Origin {
            workspace: PathBuf::from("/repo"),
            cwd: PathBuf::from("/repo"),
            pid: 42,
            herdr_pane: None,
        }
    }

    fn page<T>(items: Vec<T>) -> Page<T> {
        Page::new(items, PageInfo::default(), false)
    }

    fn team_ref(id: &str, key: &str) -> snap::TeamRef {
        snap::TeamRef {
            id: TeamId::from(id),
            key: key.into(),
            name: format!("{key} team"),
        }
    }

    fn snapshot(destination: snap::Destination, screen: snap::Screen) -> ViewSnapshot {
        ViewSnapshot {
            version: snap::VERSION,
            workspace: PathBuf::from("/repo"),
            cwd: PathBuf::from("/repo"),
            pid: 7,
            updated_at: "2026-09-25T00:00:00Z".into(),
            closed_at: None,
            herdr_pane: None,
            team: Some(team_ref("t1", "ENG")),
            destination,
            screen,
            project: None,
            cycle: None,
            issue: None,
            search: None,
            group_by: snap::GroupBy::Status,
            lists: Vec::new(),
            rows: Vec::new(),
            rows_total: 0,
            selected_row: None,
        }
    }

    fn fresh_app() -> App {
        let mut app = App::new(&Config::default());
        app.outbox.requests.clear();
        app
    }

    fn requested(app: &App, want: impl Fn(&Request) -> bool) -> bool {
        app.outbox.requests.iter().any(want)
    }

    #[test]
    fn a_snapshot_round_trips_through_a_restore() {
        let mut before = fresh_app();
        before.handle_message(Message::Teams(vec![team("t1", "ENG"), team("t2", "WEB")]));
        before.activate(Nav::Team(1, TeamSection::Issues));
        before.handle_message(Message::Issues {
            team_id: TeamId::from("t2"),
            preset: Preset::Active,
            page: page(vec![issue("1"), issue("2"), issue("3")]),
        });
        before.set_group_by(GroupBy::None);
        before.set_selected_index(2);
        let saved = before.snapshot(&origin()).unwrap();
        assert_eq!(
            saved.destination,
            snap::Destination::Team {
                team: team_ref("t2", "WEB"),
                section: snap::Section::Issues
            }
        );
        assert_eq!(
            saved.selected().and_then(|r| r.identifier.as_deref()),
            Some("ENG-3")
        );

        // The sidebar has since been reordered: WEB comes first now.
        let mut after = fresh_app();
        after.restore(saved.clone());
        after.handle_message(Message::Teams(vec![team("t2", "WEB"), team("t1", "ENG")]));
        assert_eq!(after.nav.dest, Nav::Team(0, TeamSection::Issues));
        assert!(requested(
            &after,
            |r| matches!(r, Request::Issues { team_id, .. } if team_id == "t2")
        ));
        after.handle_message(Message::Issues {
            team_id: TeamId::from("t2"),
            preset: Preset::Active,
            page: page(vec![issue("1"), issue("2"), issue("3")]),
        });
        assert_eq!(after.selected_index(), 2);
        assert_eq!(after.view.group_by, GroupBy::None);
        assert!(after.snapshot(&origin()).unwrap().same_view(&saved));
    }

    #[test]
    fn a_view_is_found_by_id_after_the_views_reorder() {
        let mut app = fresh_app();
        app.restore(snapshot(
            snap::Destination::View {
                id: CustomViewId::from("v2"),
                name: "View v2".into(),
            },
            snap::Screen::IssueList,
        ));
        app.handle_message(Message::Teams(vec![team("t1", "ENG")]));
        // Nothing to open yet, and the team's list is not fetched meanwhile.
        assert!(!requested(&app, |r| matches!(r, Request::Issues { .. })));
        assert!(
            app.snapshot(&origin()).is_none(),
            "nothing is recorded mid-restore"
        );

        app.handle_message(Message::CustomViews(vec![
            view("v1"),
            view("v2"),
            view("v3"),
        ]));
        let Nav::View(index) = app.nav.dest else {
            panic!("expected a view, got {:?}", app.nav.dest);
        };
        assert_eq!(app.store.custom_views[index].id, "v2");
        assert!(requested(
            &app,
            |r| matches!(r, Request::ViewIssues { view_id, .. } if view_id == "v2")
        ));
    }

    #[test]
    fn a_deleted_view_falls_back_to_the_views_page() {
        let mut app = fresh_app();
        app.restore(snapshot(
            snap::Destination::View {
                id: CustomViewId::from("gone"),
                name: "Gone".into(),
            },
            snap::Screen::IssueList,
        ));
        app.handle_message(Message::Teams(vec![team("t1", "ENG")]));
        app.handle_message(Message::CustomViews(vec![view("v1")]));
        assert_eq!(app.nav.dest, Nav::Views);
        assert_eq!(app.nav.screen, Screen::ViewList);
    }

    #[test]
    fn a_deleted_team_falls_back_to_the_first_team() {
        let mut app = fresh_app();
        app.restore(snapshot(
            snap::Destination::Team {
                team: team_ref("gone", "OLD"),
                section: snap::Section::Cycles,
            },
            snap::Screen::CycleList,
        ));
        app.handle_message(Message::Teams(vec![team("t1", "ENG")]));
        assert_eq!(app.nav.dest, Nav::Team(0, TeamSection::Issues));
        assert!(requested(
            &app,
            |r| matches!(r, Request::Issues { team_id, .. } if team_id == "t1")
        ));
    }

    #[test]
    fn a_project_page_is_reopened_once_the_projects_arrive() {
        let mut saved = snapshot(
            snap::Destination::Team {
                team: team_ref("t1", "ENG"),
                section: snap::Section::Projects,
            },
            snap::Screen::IssueDetail,
        );
        saved.project = Some(snap::NamedRef {
            id: ProjectId::from("p2"),
            name: "Project p2".into(),
        });
        saved.issue = Some(snap::IssueRef {
            id: IssueId::from("9"),
            identifier: "ENG-9".into(),
            title: "Issue 9".into(),
        });
        let mut app = fresh_app();
        app.restore(saved);
        app.handle_message(Message::Teams(vec![team("t1", "ENG")]));
        assert_eq!(app.nav.screen, Screen::ProjectList);

        app.handle_message(Message::Projects {
            team_id: TeamId::from("t1"),
            page: page(vec![project("p1"), project("p2")]),
        });
        assert_eq!(app.nav.screen, Screen::IssueDetail);
        assert_eq!(app.nav.detail_return, Screen::ProjectDetail);
        assert_eq!(app.nav.current_project.as_ref().unwrap().id, "p2");
        assert_eq!(
            app.store.current_issue.as_ref().unwrap().identifier,
            "ENG-9"
        );

        // Back navigation retraces the path.
        app.close_detail();
        assert_eq!(app.nav.screen, Screen::ProjectDetail);
        app.leave_container();
        assert_eq!(app.nav.screen, Screen::ProjectList);
    }

    #[test]
    fn a_deleted_project_leaves_its_list_open() {
        let mut saved = snapshot(
            snap::Destination::Team {
                team: team_ref("t1", "ENG"),
                section: snap::Section::Projects,
            },
            snap::Screen::ProjectDetail,
        );
        saved.project = Some(snap::NamedRef {
            id: ProjectId::from("gone"),
            name: "Gone".into(),
        });
        let mut app = fresh_app();
        app.restore(saved);
        app.handle_message(Message::Teams(vec![team("t1", "ENG")]));
        app.handle_message(Message::Projects {
            team_id: TeamId::from("t1"),
            page: page(vec![project("p1")]),
        });
        assert_eq!(app.nav.screen, Screen::ProjectList);
        assert!(app.restore.is_none());
    }

    #[test]
    fn a_deleted_issue_returns_to_the_list_it_was_opened_from() {
        let mut saved = snapshot(
            snap::Destination::Team {
                team: team_ref("t1", "ENG"),
                section: snap::Section::Issues,
            },
            snap::Screen::IssueDetail,
        );
        saved.issue = Some(snap::IssueRef {
            id: IssueId::from("9"),
            identifier: "ENG-9".into(),
            title: "Issue 9".into(),
        });
        let mut app = fresh_app();
        app.restore(saved);
        app.handle_message(Message::Teams(vec![team("t1", "ENG")]));
        assert_eq!(app.nav.screen, Screen::IssueDetail);

        app.handle_message(Message::Failed {
            request: Box::new(Request::IssueDetail {
                issue_id: IssueId::from("9"),
            }),
            error: "Entity not found".into(),
        });
        assert_eq!(app.nav.screen, Screen::IssueList);
        assert!(
            app.view.error_popup.is_none(),
            "a vanished issue is not an error"
        );
        assert_eq!(
            app.view.status_message.as_deref(),
            Some("ENG-9 is no longer available")
        );
    }

    #[test]
    fn a_key_press_before_the_page_is_found_opens_the_team_list() {
        let mut app = fresh_app();
        app.restore(snapshot(
            snap::Destination::View {
                id: CustomViewId::from("v1"),
                name: "View v1".into(),
            },
            snap::Screen::IssueList,
        ));
        app.handle_message(Message::Teams(vec![team("t1", "ENG")]));
        app.cancel_restore();
        assert!(requested(&app, |r| matches!(r, Request::Issues { .. })));
        // The views arriving later no longer yank the user away.
        app.handle_message(Message::CustomViews(vec![view("v1")]));
        assert_eq!(app.nav.dest, Nav::Team(0, TeamSection::Issues));
    }

    #[test]
    fn list_settings_take_effect_before_the_first_fetch() {
        let mut saved = snapshot(
            snap::Destination::Team {
                team: team_ref("t1", "ENG"),
                section: snap::Section::Issues,
            },
            snap::Screen::IssueList,
        );
        saved.lists.push(snap::ListSettings {
            source: snap::Source::Team,
            preset: snap::Preset::Backlog,
            status: None,
            priority: Some("High".into()),
            selected: None,
        });
        let mut app = fresh_app();
        app.restore(saved);
        app.handle_message(Message::Teams(vec![team("t1", "ENG")]));
        assert!(requested(&app, |r| matches!(
            r,
            Request::Issues {
                preset: Preset::Backlog,
                ..
            }
        )));
        assert_eq!(app.list().filters.priority, Some(Priority::High));
    }
}
