//! Mouse input, hit-tested against what the last frame drew.

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
        if self.error_popup.is_some() {
            self.dismiss_error();
            return;
        }
        if self.show_help {
            self.show_help = false;
            return;
        }
        if self.popup != Popup::None {
            if contains(self.popup_area, x, y) {
                let index = self.popup_offset + (y - self.popup_area.y) as usize;
                if index < self.popup_list_len() {
                    self.popup_index = index;
                    self.apply_popup();
                }
            } else {
                self.close_popup();
            }
            return;
        }
        if self.input_mode != InputMode::Normal {
            return;
        }

        if contains(self.sidebar_area, x, y) {
            let index = self.sidebar_offset + (y - self.sidebar_area.y) as usize;
            let Some(SidebarRow::Item(item)) = self.sidebar_rows.get(index).cloned() else {
                return;
            };
            self.sidebar_index = index;
            self.run_sidebar_action(item.action);
            return;
        }

        if let Some((_, chip)) = self.chip_areas.iter().find(|(r, _)| contains(*r, x, y)) {
            let chip = *chip;
            self.sidebar_focus = false;
            match chip {
                Chip::Preset(preset) => self.set_preset(preset),
                Chip::ViewKind(kind) => self.set_view_kind(kind),
            }
            return;
        }

        if !contains(self.list_area, x, y) {
            return;
        }
        self.sidebar_focus = false;
        let row = (y - self.list_area.y) as usize;
        match self.screen {
            Screen::IssueList | Screen::ProjectDetail | Screen::CycleDetail => {
                match self.list_rows.get(row).cloned() {
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
                let Some(Some(target)) = self.row_targets.get(row).copied() else {
                    return;
                };
                let current = match self.screen {
                    Screen::ProjectList => self.project_cursor_mut(),
                    Screen::CycleList => &mut self.selected_cycle_index,
                    _ => &mut self.selected_view_index,
                };
                if *current != target {
                    *current = target;
                    return;
                }
                match self.screen {
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
        if self.popup != Popup::None {
            if delta > 0 {
                self.popup_next();
            } else {
                self.popup_prev();
            }
        } else if contains(self.sidebar_area, x, y) {
            let max = self
                .sidebar_rows
                .len()
                .saturating_sub(self.sidebar_area.height as usize);
            self.sidebar_offset =
                (self.sidebar_offset as isize + delta as isize).clamp(0, max as isize) as usize;
        } else if self.show_help {
            self.help_scroll = (self.help_scroll as i32 + delta as i32).max(0) as u16;
        } else {
            self.scroll_list_or_detail(delta);
        }
    }
}
