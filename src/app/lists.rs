//! The issue lists and how the one on screen is filtered, grouped, and paged.

use super::*;

/// How one issue list is shaped on screen. Its rows are in
/// [`Store::issues`], under the same [`IssueSource`].
#[derive(Debug, Default)]
pub struct IssueList {
    /// Cursor position, as an index into [`App::visible_issues`].
    pub selected: usize,
    /// The preset chip selected on this list. A team's list is sliced by it
    /// on the server, so it also decides what is fetched.
    pub preset: Preset,
    /// The live search box. Each list keeps its own, as it keeps its own
    /// filters: narrowing one list must not quietly narrow another.
    pub search: Input,
    pub filters: Filters,
}

impl IssueList {
    /// Whether an issue survives this list's preset, search box, and filters.
    /// `query` is the search text, lowercased once by the caller.
    fn admits(&self, issue: &Issue, query: &str) -> bool {
        self.preset.admits(issue)
            && (query.is_empty()
                || issue.title.to_lowercase().contains(query)
                || issue.identifier.to_lowercase().contains(query))
            && self.filters.status.as_ref().is_none_or(|status| {
                issue
                    .state
                    .as_ref()
                    .is_some_and(|state| &state.name == status)
            })
            && self.filters.priority.is_none_or(|p| issue.priority == p)
    }
}

/// The five issue lists.
pub type IssueLists = PerSource<IssueList>;

impl IssueList {
    /// An empty list for `source`.
    ///
    /// A team's issues open on Active, as in Linear; every other list opens on
    /// All, because its contents were already chosen — by a saved view's
    /// filter, by a project, by being yours — and hiding the done half of a
    /// view someone deliberately built would be a surprise.
    pub fn new(source: IssueSource) -> Self {
        Self {
            preset: match source {
                IssueSource::Team => Preset::Active,
                _ => Preset::All,
            },
            ..Self::default()
        }
    }
}

/// One rendered line of a grouped issue list.
#[derive(Debug, Clone)]
pub enum ListRow {
    Group {
        key: String,
        label: String,
        color: ratatui::style::Color,
        glyph: &'static str,
        count: usize,
        collapsed: bool,
    },
    /// `ordinal` indexes into [`App::visible_issues`].
    Issue { ordinal: usize, depth: u8 },
}

/// The active list as one frame sees it: see [`App::list_view`].
pub struct ListView<'a> {
    /// Every issue the cursor can reach, in the order it is drawn.
    pub issues: Vec<&'a Issue>,
    /// Group headers interleaved with the issues under them, each issue row
    /// addressed by its position in `issues`.
    pub rows: Vec<ListRow>,
}

/// The issues of the open sections, in the order they are drawn.
fn visible_of(sections: Vec<Section<'_>>) -> Vec<&Issue> {
    sections
        .into_iter()
        .filter(|s| !s.collapsed)
        .flat_map(|s| s.issues.into_iter().map(|(issue, _)| issue))
        .collect()
}

/// Group headers interleaved with the issues under them. An ungrouped list
/// has no headers.
fn layout_of(sections: &[Section<'_>], by: GroupBy) -> Vec<ListRow> {
    let mut rows = Vec::new();
    let mut ordinal = 0;
    for section in sections {
        if by != GroupBy::None {
            rows.push(ListRow::Group {
                key: section.key.clone(),
                label: section.label.clone(),
                color: section.color,
                glyph: section.glyph,
                count: section.issues.len(),
                collapsed: section.collapsed,
            });
        }
        if section.collapsed {
            continue;
        }
        for (_, depth) in &section.issues {
            rows.push(ListRow::Issue {
                ordinal,
                depth: *depth,
            });
            ordinal += 1;
        }
    }
    rows
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

impl App {
    // ------------------------------------------------- the active issue list

    /// Forget a list's rows, for a list that is about to belong to something
    /// else.
    pub fn reset_list(&mut self, source: IssueSource) {
        self.store.issues[source].reset();
        self.view.lists[source].selected = 0;
    }

    /// Which of the five issue lists the current screen is showing.
    pub fn issue_source(&self) -> IssueSource {
        match self.nav.screen {
            Screen::ProjectDetail => IssueSource::Project,
            Screen::CycleDetail => IssueSource::Cycle,
            _ => match self.nav.dest {
                Nav::MyIssues => IssueSource::My,
                Nav::View(_) => IssueSource::View,
                _ => IssueSource::Team,
            },
        }
    }

    /// The issue list on screen.
    pub fn list(&self) -> &IssueList {
        &self.view.lists[self.issue_source()]
    }

    pub fn list_mut(&mut self) -> &mut IssueList {
        let source = self.issue_source();
        &mut self.view.lists[source]
    }

    /// Cursor position within the active list, as an index into
    /// [`Self::visible_issues`].
    pub fn selected_index(&self) -> usize {
        self.view.lists[self.issue_source()].selected
    }

    pub(super) fn selected_index_mut(&mut self) -> &mut usize {
        self.selected_index_of(self.issue_source())
    }

    pub(super) fn selected_index_of(&mut self, source: IssueSource) -> &mut usize {
        &mut self.view.lists[source].selected
    }

    pub fn set_selected_index(&mut self, index: usize) {
        let len = self.visible_issues().len();
        *self.selected_index_mut() = index.min(len.saturating_sub(1));
    }

    /// The scroll offset of the active list.
    pub fn list_offset(&mut self) -> &mut usize {
        let source = self.issue_source();
        &mut self.frame.offsets.issues[source]
    }

    /// The id of [`Self::focused_issue`], for handing to a use case.
    pub fn focused_issue_id(&self) -> Option<IssueId> {
        self.focused_issue().map(|i| i.id.clone())
    }

    /// Get the issue currently focused (selected in list, or being viewed in detail).
    pub fn focused_issue(&self) -> Option<&Issue> {
        match self.nav.screen {
            Screen::IssueDetail => self.store.current_issue.as_ref(),
            Screen::ViewList | Screen::ProjectList | Screen::CycleList => None,
            _ => self.visible_issues().get(self.selected_index()).copied(),
        }
    }

    /// The active list, filtered and stacked into its groups.
    ///
    /// Sections are the single source of truth for both what is on screen and
    /// where the cursor can land, so a collapsed group cannot leave the cursor
    /// pointing at an issue nobody can see.
    pub fn sections(&self) -> Vec<Section<'_>> {
        let list = self.list();
        let query = list.search.value.to_lowercase();
        group(
            self.store.issues[self.issue_source()]
                .items
                .iter()
                .filter(|i| list.admits(i, &query)),
            self.view.group_by,
            &self.view.collapsed_groups,
            &self.theme,
        )
    }

    /// Every issue the cursor can currently reach, in the order it is drawn.
    pub fn visible_issues(&self) -> Vec<&Issue> {
        visible_of(self.sections())
    }

    /// The visible issues and the display rows of the active list — group
    /// headers interleaved with the issues under them — from one grouping
    /// pass. A renderer takes this once per frame rather than grouping the
    /// whole list for each thing it needs from it.
    pub fn list_view(&self) -> ListView<'_> {
        let sections = self.sections();
        let rows = layout_of(&sections, self.view.group_by);
        ListView {
            issues: visible_of(sections),
            rows,
        }
    }

    /// The group the cursor currently sits in, if the list is grouped.
    fn selected_group_key(&self) -> Option<String> {
        let target = self.selected_index();
        let mut ordinal = 0;
        for section in self.sections() {
            if section.collapsed {
                continue;
            }
            if target < ordinal + section.issues.len() {
                return Some(section.key);
            }
            ordinal += section.issues.len();
        }
        None
    }

    /// Fold or unfold the group the cursor is in.
    pub fn toggle_selected_group(&mut self) {
        let Some(key) = self.selected_group_key() else {
            return;
        };
        self.toggle_group(&key);
    }

    pub fn toggle_group(&mut self, key: &str) {
        if !self.view.collapsed_groups.remove(key) {
            self.view.collapsed_groups.insert(key.to_string());
        }
        // Folding a group can put the cursor past the end of what is left.
        let len = self.visible_issues().len();
        *self.selected_index_mut() = self.selected_index().min(len.saturating_sub(1));
    }

    /// Fold every group, or unfold them all if none is folded.
    pub fn toggle_all_groups(&mut self) {
        let keys: Vec<String> = self.sections().into_iter().map(|s| s.key).collect();
        let any_open = keys.iter().any(|k| !self.view.collapsed_groups.contains(k));
        if any_open {
            self.view.collapsed_groups.extend(keys);
        } else {
            for key in keys {
                self.view.collapsed_groups.remove(&key);
            }
        }
        let len = self.visible_issues().len();
        *self.selected_index_mut() = self.selected_index().min(len.saturating_sub(1));
    }

    pub fn cycle_group_by(&mut self) {
        self.set_group_by(self.view.group_by.next());
    }

    pub fn set_group_by(&mut self, group_by: GroupBy) {
        self.view.group_by = group_by;
        self.view.collapsed_groups.clear();
        *self.selected_index_mut() = 0;
        self.set_status(format!("Grouped by {}", group_by.label()));
    }

    /// The preset chip selected on the list currently on screen.
    pub fn preset(&self) -> Preset {
        self.view.lists[self.issue_source()].preset
    }

    pub fn set_preset(&mut self, preset: Preset) {
        if self.preset() == preset {
            return;
        }
        let source = self.issue_source();
        self.view.lists[source].preset = preset;
        *self.selected_index_mut() = 0;
        // A team's list is sliced on the server; the other lists already hold
        // everything they can show and only filter locally. Workspace search
        // results are not refetched either — that would throw them away.
        if source == IssueSource::Team
            && self.nav.global_search.is_none()
            && let Some(team_id) = self.team_id()
        {
            self.request(Request::Issues {
                team_id,
                after: None,
                preset,
            });
        }
    }

    /// Step to the next preset chip, as clicking along the row would.
    pub fn cycle_preset(&mut self) {
        let presets = Preset::all();
        let next = presets
            .iter()
            .position(|p| *p == self.preset())
            .map(|i| (i + 1) % presets.len())
            .unwrap_or(0);
        self.set_preset(presets[next]);
    }

    /// Ask for the next page once the cursor nears the end of the active list.
    pub(super) fn maybe_prefetch(&mut self) {
        // The cursor moves through what is visible, so that is what it can
        // reach the end of — a search or a folded group can leave most of
        // the loaded list off screen.
        self.maybe_prefetch_within(self.visible_issues().len());
    }

    /// [`Self::maybe_prefetch`] for a caller that already knows how many
    /// issues are visible, so the list is not grouped again to count them.
    pub(super) fn maybe_prefetch_within(&mut self, visible: usize) {
        let source = self.issue_source();
        if self.selected_index() + PREFETCH_MARGIN < visible {
            return;
        }
        let info = &self.store.issues[source].page_info;
        if !info.has_next_page {
            return;
        }
        let Some(cursor) = info.end_cursor.clone() else {
            return;
        };
        let after = Some(cursor.clone());
        let request = match source {
            IssueSource::Team => self.team_id().map(|team_id| Request::Issues {
                team_id,
                after,
                preset: self.view.lists[IssueSource::Team].preset,
            }),
            IssueSource::My => self
                .store
                .viewer_id
                .clone()
                .map(|user_id| Request::MyIssues { user_id, after }),
            IssueSource::View => self
                .store
                .loaded_view_id
                .clone()
                .map(|view_id| Request::ViewIssues { view_id, after }),
            IssueSource::Project => {
                self.nav
                    .current_project
                    .as_ref()
                    .map(|p| Request::ProjectIssues {
                        project_id: p.id.clone(),
                        after,
                    })
            }
            IssueSource::Cycle => self
                .nav
                .current_cycle
                .as_ref()
                .map(|c| Request::CycleIssues {
                    cycle_id: c.id.clone(),
                    after,
                }),
        };
        if let Some(request) = request
            && self.outbox.prefetched.insert(cursor)
        {
            self.request(request);
        }
    }

    pub(super) fn maybe_prefetch_projects(&mut self) {
        let in_view = self.in_project_view();
        let info = if in_view {
            &self.store.view_projects.page_info
        } else {
            &self.store.projects.page_info
        };
        if self.project_cursor() + PREFETCH_MARGIN < self.project_rows().len()
            || !info.has_next_page
        {
            return;
        }
        let Some(cursor) = info.end_cursor.clone() else {
            return;
        };
        let after = Some(cursor.clone());
        let request = if in_view {
            self.store
                .loaded_view_projects_id
                .clone()
                .map(|view_id| Request::ViewProjects { view_id, after })
        } else {
            self.team_id()
                .map(|team_id| Request::Projects { team_id, after })
        };
        if let Some(request) = request
            && self.outbox.prefetched.insert(cursor)
        {
            self.request(request);
        }
    }
}
