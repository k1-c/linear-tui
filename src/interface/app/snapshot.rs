//! Capturing the view as a [`ViewSnapshot`] — everything is recorded by
//! Linear ID, so the record still means the same page after the sidebar
//! reorders.

use super::*;
use crate::entity::snapshot::{self as snap, ViewSnapshot};

impl App {
    /// The view as a snapshot, or `None` while there is no view worth
    /// recording yet: before the teams arrive, or while a restore is still
    /// finding its way back — writing then would replace the snapshot being
    /// restored with the half-built page in front of it.
    /// `now` is when it is taken, as the record states it.
    pub fn snapshot(&self, origin: &Origin, now: String) -> Option<ViewSnapshot> {
        if self.store.teams.is_empty() || self.restore.as_ref().is_some_and(|r| r.dest.is_some()) {
            return None;
        }
        let screen = self.nav.screen;
        let behind = if screen == Screen::IssueDetail {
            self.nav.detail_return
        } else {
            screen
        };
        let (rows, rows_total, selected_row) = self.snapshot_rows();
        Some(ViewSnapshot {
            version: snap::VERSION,
            workspace: origin.workspace.clone(),
            cwd: origin.cwd.clone(),
            pid: origin.pid,
            updated_at: now,
            closed_at: None,
            herdr_pane: origin.herdr_pane.clone(),
            organization: self
                .store
                .organization
                .as_ref()
                .map(|org| snap::OrganizationRef {
                    id: org.id.clone(),
                    name: org.name.clone(),
                    url_key: org.url_key.clone(),
                }),
            team: self.current_team().map(team_ref),
            destination: self.snapshot_destination(),
            screen: screen.into(),
            project: (behind == Screen::ProjectDetail)
                .then_some(self.nav.current_project.as_ref())
                .flatten()
                .map(|p| snap::NamedRef {
                    id: p.id.clone(),
                    name: p.name.clone(),
                }),
            cycle: (behind == Screen::CycleDetail)
                .then_some(self.nav.current_cycle.as_ref())
                .flatten()
                .map(|c| snap::NamedRef {
                    id: c.id.clone(),
                    name: c.label(),
                }),
            issue: (screen == Screen::IssueDetail)
                .then_some(self.store.current_issue.as_ref())
                .flatten()
                .map(issue_ref),
            search: self.nav.global_search.clone(),
            group_by: self.view.group_by.into(),
            lists: self.snapshot_lists(),
            rows,
            rows_total,
            selected_row,
        })
    }

    fn snapshot_destination(&self) -> snap::Destination {
        let team = |index: usize| self.store.teams.get(index).map(team_ref);
        match self.nav.dest {
            Nav::MyIssues => snap::Destination::MyIssues,
            Nav::Views => snap::Destination::Views,
            Nav::View(index) => match self.store.custom_views.get(index) {
                Some(view) => snap::Destination::View {
                    id: view.id.clone(),
                    name: view.name.clone(),
                },
                None => snap::Destination::Views,
            },
            Nav::Team(index, section) => match team(index) {
                Some(team) => snap::Destination::Team {
                    team,
                    section: section.into(),
                },
                None => snap::Destination::MyIssues,
            },
            Nav::Favorite(index) => match self.store.favorites.get(index) {
                Some(favorite) => snap::Destination::Favorite {
                    id: favorite.id.clone(),
                    title: favorite.label(),
                },
                None => snap::Destination::MyIssues,
            },
        }
    }

    /// Every issue list that is loaded or shaped differently from how it
    /// opens, with the issue under its cursor.
    fn snapshot_lists(&self) -> Vec<snap::ListSettings> {
        IssueSource::ALL
            .into_iter()
            .filter_map(|source| {
                let list = &self.view.lists[source];
                let selected = self.store.issues[source]
                    .loaded
                    .then(|| {
                        visible_of(self.sections_of(source))
                            .get(list.selected)
                            .copied()
                    })
                    .flatten()
                    .map(issue_ref);
                let default = IssueList::new(source);
                if selected.is_none() && list.preset == default.preset && !list.filters.is_active()
                {
                    return None;
                }
                Some(snap::ListSettings {
                    source: source.into(),
                    preset: list.preset.into(),
                    status: list.filters.status.clone(),
                    priority: list.filters.priority.map(|p| p.label().to_string()),
                    selected,
                })
            })
            .collect()
    }

    /// The list on screen as rows, with how many it has and which is selected.
    fn snapshot_rows(&self) -> (Vec<snap::Row>, usize, Option<usize>) {
        let (rows, cursor): (Vec<snap::Row>, usize) = match self.nav.screen {
            Screen::ProjectList => (
                self.project_rows().iter().map(project_row).collect(),
                self.project_cursor(),
            ),
            Screen::CycleList => (
                self.store.cycles.items.iter().map(cycle_row).collect(),
                self.view.selected_cycle_index,
            ),
            Screen::ViewList => (
                self.listed_views()
                    .into_iter()
                    .filter_map(|i| self.store.custom_views.get(i))
                    .map(view_row)
                    .collect(),
                self.view.selected_view_index,
            ),
            Screen::IssueList
            | Screen::IssueDetail
            | Screen::ProjectDetail
            | Screen::CycleDetail => {
                let issues = self.visible_issues();
                let cursor = match (&self.store.current_issue, self.nav.screen) {
                    // An issue page may have been stepped away from the cursor.
                    (Some(open), Screen::IssueDetail) => issues
                        .iter()
                        .position(|i| i.id == open.id)
                        .unwrap_or(self.selected_index()),
                    _ => self.selected_index(),
                };
                (issues.into_iter().map(issue_row).collect(), cursor)
            }
        };
        let total = rows.len();
        let selected = (cursor < total.min(snap::MAX_ROWS)).then_some(cursor);
        let mut rows = rows;
        rows.truncate(snap::MAX_ROWS);
        (rows, total, selected)
    }
}

fn team_ref(team: &Team) -> snap::TeamRef {
    snap::TeamRef {
        id: team.id.clone(),
        key: team.key.clone(),
        name: team.name.clone(),
    }
}

fn issue_ref(issue: &Issue) -> snap::IssueRef {
    snap::IssueRef {
        id: issue.id.clone(),
        identifier: issue.identifier.clone(),
        title: issue.title.clone(),
    }
}

fn user_name(user: &User) -> String {
    user.display_name
        .clone()
        .unwrap_or_else(|| user.name.clone())
}

fn issue_row(issue: &Issue) -> snap::Row {
    snap::Row {
        kind: snap::RowKind::Issue,
        id: issue.id.to_string(),
        identifier: Some(issue.identifier.clone()),
        title: issue.title.clone(),
        state: issue.state.as_ref().map(|s| s.name.clone()),
        assignee: issue.assignee.as_ref().map(user_name),
        priority: (issue.priority != Priority::None).then(|| issue.priority.label().to_string()),
        url: issue.url.clone(),
    }
}

fn project_row(project: &Project) -> snap::Row {
    snap::Row {
        kind: snap::RowKind::Project,
        id: project.id.to_string(),
        identifier: None,
        title: project.name.clone(),
        state: project.state.clone(),
        assignee: project.lead.as_ref().map(user_name),
        priority: None,
        url: project.url.clone(),
    }
}

fn cycle_row(cycle: &Cycle) -> snap::Row {
    snap::Row {
        kind: snap::RowKind::Cycle,
        id: cycle.id.to_string(),
        identifier: None,
        title: cycle.label(),
        state: None,
        assignee: None,
        priority: None,
        url: None,
    }
}

fn view_row(view: &CustomView) -> snap::Row {
    snap::Row {
        kind: snap::RowKind::View,
        id: view.id.to_string(),
        identifier: None,
        title: view.name.clone(),
        state: None,
        assignee: None,
        priority: None,
        url: None,
    }
}

// The TUI's enums and the snapshot's are kept apart: the snapshot's are a
// file format, and must not change shape when a screen is renamed.

impl From<Screen> for snap::Screen {
    fn from(screen: Screen) -> Self {
        match screen {
            Screen::IssueList => Self::IssueList,
            Screen::IssueDetail => Self::IssueDetail,
            Screen::ProjectList => Self::ProjectList,
            Screen::ProjectDetail => Self::ProjectDetail,
            Screen::CycleList => Self::CycleList,
            Screen::CycleDetail => Self::CycleDetail,
            Screen::ViewList => Self::ViewList,
        }
    }
}

impl From<TeamSection> for snap::Section {
    fn from(section: TeamSection) -> Self {
        match section {
            TeamSection::Issues => Self::Issues,
            TeamSection::Cycles => Self::Cycles,
            TeamSection::Projects => Self::Projects,
            TeamSection::Views => Self::Views,
        }
    }
}

impl From<snap::Section> for TeamSection {
    fn from(section: snap::Section) -> Self {
        match section {
            snap::Section::Issues => Self::Issues,
            snap::Section::Cycles => Self::Cycles,
            snap::Section::Projects => Self::Projects,
            snap::Section::Views => Self::Views,
        }
    }
}

impl From<GroupBy> for snap::GroupBy {
    fn from(group_by: GroupBy) -> Self {
        match group_by {
            GroupBy::Status => Self::Status,
            GroupBy::Assignee => Self::Assignee,
            GroupBy::Priority => Self::Priority,
            GroupBy::Project => Self::Project,
            GroupBy::None => Self::None,
        }
    }
}

impl From<snap::GroupBy> for GroupBy {
    fn from(group_by: snap::GroupBy) -> Self {
        match group_by {
            snap::GroupBy::Status => Self::Status,
            snap::GroupBy::Assignee => Self::Assignee,
            snap::GroupBy::Priority => Self::Priority,
            snap::GroupBy::Project => Self::Project,
            snap::GroupBy::None => Self::None,
        }
    }
}

impl From<IssueSource> for snap::Source {
    fn from(source: IssueSource) -> Self {
        match source {
            IssueSource::Team => Self::Team,
            IssueSource::My => Self::My,
            IssueSource::View => Self::View,
            IssueSource::Project => Self::Project,
            IssueSource::Cycle => Self::Cycle,
        }
    }
}

impl From<snap::Source> for IssueSource {
    fn from(source: snap::Source) -> Self {
        match source {
            snap::Source::Team => Self::Team,
            snap::Source::My => Self::My,
            snap::Source::View => Self::View,
            snap::Source::Project => Self::Project,
            snap::Source::Cycle => Self::Cycle,
        }
    }
}

impl From<Preset> for snap::Preset {
    fn from(preset: Preset) -> Self {
        match preset {
            Preset::Active => Self::Active,
            Preset::Backlog => Self::Backlog,
            Preset::All => Self::All,
        }
    }
}

impl From<snap::Preset> for Preset {
    fn from(preset: snap::Preset) -> Self {
        match preset {
            snap::Preset::Active => Self::Active,
            snap::Preset::Backlog => Self::Backlog,
            snap::Preset::All => Self::All,
        }
    }
}
