//! Mouse input, hit-tested against what the last frame drew.

use ratatui::layout::Rect;

use super::*;

fn contains(area: Rect, x: u16, y: u16) -> bool {
    area.width > 0
        && x >= area.x
        && x < area.x + area.width
        && y >= area.y
        && y < area.y + area.height
}

impl App {
    /// A left click at terminal cell `(x, y)`.
    ///
    /// Hit-testing runs against the rows and areas the last frame recorded, so
    /// a click always lands on what the user actually saw. Clicking a row
    /// selects it; clicking the selected row again opens it — the terminal's
    /// stand-in for a double-click, which crossterm cannot report.
    pub fn click(&mut self, x: u16, y: u16) {
        if self.view.error_popup.is_some() {
            self.dismiss_error();
            return;
        }
        if self.view.show_help {
            self.close_help();
            return;
        }
        if self.view.popup != Popup::None {
            if contains(self.frame.popup_area, x, y) {
                self.popup_pick(self.frame.popup_offset + (y - self.frame.popup_area.y) as usize);
            } else {
                self.close_popup();
            }
            return;
        }
        if self.view.input_mode != InputMode::Normal {
            return;
        }

        if contains(self.frame.sidebar_area, x, y) {
            let index = self.frame.sidebar_offset + (y - self.frame.sidebar_area.y) as usize;
            let Some(SidebarRow::Item(item)) = self.frame.sidebar_rows.get(index).cloned() else {
                return;
            };
            self.view.sidebar.index = index;
            self.run_sidebar_action(item.action);
            return;
        }

        if let Some((_, chip)) = self
            .frame
            .chip_areas
            .iter()
            .find(|(r, _)| contains(*r, x, y))
        {
            let chip = *chip;
            self.view.sidebar.focus = false;
            match chip {
                Chip::Preset(preset) => self.set_preset(preset),
                Chip::ViewKind(kind) => self.set_view_kind(kind),
            }
            return;
        }

        if !contains(self.frame.list_area, x, y) {
            return;
        }
        self.view.sidebar.focus = false;
        let row = (y - self.frame.list_area.y) as usize;
        match self.nav.screen {
            Screen::IssueList | Screen::ProjectDetail | Screen::CycleDetail => {
                match self.frame.list_rows.get(row).cloned() {
                    Some(ListRow::Group { key, .. }) => self.toggle_group(&key),
                    Some(ListRow::Issue { ordinal, .. }) => {
                        if ordinal == self.selected_index() {
                            self.open_issue_detail();
                        } else {
                            self.set_selected_index(ordinal);
                            self.maybe_prefetch();
                        }
                    }
                    None => {}
                }
            }
            Screen::ProjectList | Screen::CycleList | Screen::ViewList => {
                let Some(Some(target)) = self.frame.row_targets.get(row).copied() else {
                    return;
                };
                let current = match self.nav.screen {
                    Screen::ProjectList => self.project_cursor_mut(),
                    Screen::CycleList => &mut self.view.selected_cycle_index,
                    _ => &mut self.view.selected_view_index,
                };
                if *current != target {
                    *current = target;
                    return;
                }
                match self.nav.screen {
                    Screen::ProjectList => self.open_project_detail(),
                    Screen::CycleList => self.open_cycle_detail(),
                    _ => self.open_selected_view(),
                }
            }
            Screen::IssueDetail => {}
        }
    }

    /// The mouse wheel at `(x, y)`: scroll whatever is under the pointer.
    pub fn wheel(&mut self, x: u16, y: u16, delta: i16) {
        if self.view.popup != Popup::None {
            if delta > 0 {
                self.popup_next();
            } else {
                self.popup_prev();
            }
        } else if contains(self.frame.sidebar_area, x, y) {
            let max = self
                .frame
                .sidebar_rows
                .len()
                .saturating_sub(self.frame.sidebar_area.height as usize);
            self.frame.sidebar_offset = (self.frame.sidebar_offset as isize + delta as isize)
                .clamp(0, max as isize) as usize;
        } else if self.view.show_help {
            self.scroll_help(delta);
        } else {
            self.scroll_list_or_detail(delta);
        }
    }
}
