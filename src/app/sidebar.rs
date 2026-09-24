//! The navigation sidebar: its rows, Favorites, and moving through it.

use super::*;

/// One line of the navigation sidebar.
#[derive(Debug, Clone)]
pub enum SidebarRow {
    /// A section caption. Not selectable.
    Header(String),
    /// Blank spacing between sections. Not selectable.
    Gap,
    Item(SidebarItem),
}

#[derive(Debug, Clone)]
pub struct SidebarItem {
    pub label: String,
    pub icon: &'static str,
    /// Colour Linear gives this team, view, or project, when it has one.
    pub color: Option<ratatui::style::Color>,
    pub depth: u8,
    /// What Enter (or a click) does.
    pub action: SidebarAction,
    /// Open/closed, for a row that heads a foldable group.
    pub expanded: Option<bool>,
    /// Text shown right-aligned, muted.
    pub trailing: Option<String>,
    /// How loudly the row is drawn. Favorites are what people navigate by, so
    /// they read strongest; secondary pages like Views recede.
    pub tone: Tone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Subtle,
    Normal,
    Strong,
}

impl SidebarItem {
    /// The destination this row goes to, if it is one.
    pub fn nav(&self) -> Option<Nav> {
        match self.action {
            SidebarAction::Go(nav) => Some(nav),
            _ => None,
        }
    }
}

/// What activating a sidebar row does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction {
    Go(Nav),
    /// Fold or unfold a Favorites folder, by its position in `favorites`.
    Fold(usize),
    /// Open the team picker — the team row is a switcher, not a tree.
    SwitchTeam,
}

impl App {
    // ----------------------------------------------------------- sidebar

    /// Build the sidebar's rows from the current state.
    ///
    /// Recomputed rather than cached because every input to it — teams, views,
    /// favorites, folded folders — can change under a message arriving from
    /// the network.
    pub fn sidebar_layout(&self) -> Vec<SidebarRow> {
        let item =
            |label: &str, icon: &'static str, depth: u8, action: SidebarAction, tone: Tone| {
                SidebarRow::Item(SidebarItem {
                    label: label.to_string(),
                    icon,
                    color: None,
                    depth,
                    action,
                    expanded: None,
                    trailing: None,
                    tone,
                })
            };

        let mut rows = vec![
            item(
                "My Issues",
                "\u{25c9}",
                0,
                SidebarAction::Go(Nav::MyIssues),
                Tone::Normal,
            ),
            item(
                "Views",
                "\u{2261}",
                0,
                SidebarAction::Go(Nav::Views),
                Tone::Subtle,
            ),
        ];

        if !self.store.favorites.is_empty() {
            rows.push(SidebarRow::Gap);
            rows.push(SidebarRow::Header("Favorites".into()));
            // Top-level entries in order, each folder followed by its contents.
            for (index, fav) in self.store.favorites.iter().enumerate() {
                if fav.parent.is_some() {
                    continue;
                }
                rows.push(self.favorite_row(index, 0));
                if fav.is_folder() && !self.collapsed_folders.contains(&fav.id) {
                    for (child, _) in self
                        .store
                        .favorites
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| f.parent.as_ref().is_some_and(|p| p.id == fav.id))
                    {
                        rows.push(self.favorite_row(child, 1));
                    }
                }
            }
        }

        // The team is a switcher: one row naming the current team, and only
        // that team's pages beneath it.
        if let Some(team) = self.current_team() {
            let index = self.selected_team_index;
            rows.push(SidebarRow::Gap);
            rows.push(SidebarRow::Header("Team".into()));
            rows.push(SidebarRow::Item(SidebarItem {
                label: team.name.clone(),
                icon: "\u{25cf}",
                color: team.color.as_deref().and_then(hex_color),
                depth: 0,
                action: SidebarAction::SwitchTeam,
                expanded: None,
                trailing: (self.store.teams.len() > 1).then(|| "t \u{21c5}".to_string()),
                tone: Tone::Strong,
            }));
            rows.push(item(
                "Issues",
                "\u{2263}",
                1,
                SidebarAction::Go(Nav::Team(index, TeamSection::Issues)),
                Tone::Normal,
            ));
            // Teams that do not run cycles have no Cycles page in Linear
            // either, so showing an empty one would be a dead end.
            if team.cycles_enabled {
                rows.push(item(
                    "Cycles",
                    "\u{25d4}",
                    1,
                    SidebarAction::Go(Nav::Team(index, TeamSection::Cycles)),
                    Tone::Normal,
                ));
            }
            rows.push(item(
                "Projects",
                "\u{25a3}",
                1,
                SidebarAction::Go(Nav::Team(index, TeamSection::Projects)),
                Tone::Normal,
            ));
            rows.push(item(
                "Views",
                "\u{2261}",
                1,
                SidebarAction::Go(Nav::Team(index, TeamSection::Views)),
                Tone::Normal,
            ));
        }

        rows
    }

    /// The sidebar row for one favorite.
    fn favorite_row(&self, index: usize, depth: u8) -> SidebarRow {
        let fav = &self.store.favorites[index];
        let color = fav
            .color
            .as_deref()
            .or_else(|| fav.project.as_ref().and_then(|p| p.color.as_deref()))
            .or_else(|| {
                fav.issue
                    .as_ref()
                    .and_then(|i| i.state.as_ref()?.color.as_deref())
            })
            .and_then(hex_color);
        let icon = match fav.kind.as_str() {
            "folder" | "document" => "\u{25a4}",
            "project" => "\u{25a3}",
            "customView" | "predefinedView" => "\u{2261}",
            "issue" => fav
                .issue
                .as_ref()
                .and_then(|i| i.state.as_ref()?.state_type)
                .unwrap_or(StateType::Unknown)
                .glyph(),
            "cycle" => "\u{25d4}",
            "label" => "\u{25cf}",
            _ => "\u{2022}",
        };
        let label = match (&fav.issue, fav.kind.as_str()) {
            (Some(issue), "issue") => format!("{} {}", issue.identifier, issue.title),
            _ => fav.label(),
        };
        SidebarRow::Item(SidebarItem {
            label,
            icon,
            color,
            depth,
            action: self.favorite_action(index),
            expanded: fav
                .is_folder()
                .then(|| !self.collapsed_folders.contains(&fav.id)),
            trailing: None,
            tone: Tone::Strong,
        })
    }

    /// Where a favorite leads. Views and team pages resolve to their own
    /// destination, so opening one lights the same row as getting there any
    /// other way.
    pub(super) fn favorite_action(&self, index: usize) -> SidebarAction {
        let fav = &self.store.favorites[index];
        if fav.is_folder() {
            return SidebarAction::Fold(index);
        }
        if let Some(view) = &fav.custom_view
            && let Some(i) = self.store.custom_views.iter().position(|v| v.id == view.id)
        {
            return SidebarAction::Go(Nav::View(i));
        }
        if fav.kind == "predefinedView" {
            if fav.predefined_view_type.as_deref() == Some("myIssues") {
                return SidebarAction::Go(Nav::MyIssues);
            }
            let team = fav
                .predefined_view_team
                .as_ref()
                .and_then(|t| self.store.teams.iter().position(|x| x.id == t.id));
            let section = match fav.predefined_view_type.as_deref() {
                Some("issues" | "allIssues" | "activeIssues" | "backlog") => {
                    Some(TeamSection::Issues)
                }
                Some("cycles") => Some(TeamSection::Cycles),
                Some("projects") => Some(TeamSection::Projects),
                _ => None,
            };
            if let (Some(team), Some(section)) = (team, section) {
                return SidebarAction::Go(Nav::Team(team, section));
            }
        }
        SidebarAction::Go(Nav::Favorite(index))
    }

    /// Open a favorite that has no destination of its own: a project, cycle,
    /// or issue in place, and anything this client has no page for — a
    /// document, a label, a workspace-wide page — on linear.app.
    pub(super) fn open_favorite(&mut self, index: usize) {
        let Some(fav) = self.store.favorites.get(index).cloned() else {
            return;
        };
        self.sidebar_focus = false;
        if let Some(project) = fav.project {
            self.nav = Nav::Favorite(index);
            self.open_project(project);
        } else if let Some(cycle) = fav.cycle {
            self.nav = Nav::Favorite(index);
            self.open_cycle(cycle);
        } else if let Some(issue) = fav.issue {
            // Just enough to draw the header; the detail fetch fills the rest.
            let stub = Issue {
                id: issue.id,
                identifier: issue.identifier,
                title: issue.title,
                state: issue.state,
                ..Issue::default()
            };
            self.open_issue_from_list(&stub);
            self.nav = Nav::Favorite(index);
        } else if let Some(url) = fav.url {
            self.request(Request::OpenUrl(url));
        } else {
            self.set_status("This favorite cannot be opened here");
        }
    }

    /// Indices of the sidebar rows the cursor may land on.
    fn sidebar_stops(&self) -> Vec<usize> {
        self.frame
            .sidebar_rows
            .iter()
            .enumerate()
            .filter(|(_, row)| matches!(row, SidebarRow::Item(_)))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn sidebar_move(&mut self, delta: isize) {
        let stops = self.sidebar_stops();
        if stops.is_empty() {
            return;
        }
        let current = stops
            .iter()
            .position(|i| *i == self.sidebar_index)
            .unwrap_or(0);
        let mut next = current;
        Self::nav_by(stops.len(), &mut next, delta);
        self.sidebar_index = stops[next];
    }

    fn sidebar_item(&self, index: usize) -> Option<&SidebarItem> {
        match self.frame.sidebar_rows.get(index) {
            Some(SidebarRow::Item(item)) => Some(item),
            _ => None,
        }
    }

    /// Enter on the sidebar.
    pub fn sidebar_activate(&mut self) {
        if let Some(action) = self.sidebar_item(self.sidebar_index).map(|i| i.action) {
            self.run_sidebar_action(action);
        }
    }

    pub(super) fn run_sidebar_action(&mut self, action: SidebarAction) {
        match action {
            SidebarAction::Go(nav) => self.activate(nav),
            SidebarAction::Fold(index) => self.toggle_folder(index),
            SidebarAction::SwitchTeam => self.open_team_select(),
        }
    }

    /// `h`/`l` on the sidebar: fold or unfold the folder under the cursor.
    pub fn sidebar_toggle(&mut self) {
        if let Some(SidebarAction::Fold(index)) =
            self.sidebar_item(self.sidebar_index).map(|i| i.action)
        {
            self.toggle_folder(index);
        }
    }

    pub fn toggle_folder(&mut self, index: usize) {
        let Some(id) = self.store.favorites.get(index).map(|f| f.id.clone()) else {
            return;
        };
        if !self.collapsed_folders.remove(&id) {
            self.collapsed_folders.insert(id);
        }
        self.frame.sidebar_rows = self.sidebar_layout();
    }

    /// Move focus between the sidebar and the content pane.
    pub fn focus_sidebar(&mut self, focused: bool) {
        if focused && !self.sidebar_visible {
            return;
        }
        self.sidebar_focus = focused;
        if focused {
            // Start on the row matching where the content pane already is, so
            // the sidebar opens pointing at you rather than at the top.
            if let Some(index) = self.frame.sidebar_rows.iter().position(
                |row| matches!(row, SidebarRow::Item(item) if item.nav() == Some(self.nav)),
            ) {
                self.sidebar_index = index;
            }
        }
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
        if !self.sidebar_visible {
            self.sidebar_focus = false;
        }
    }
}
