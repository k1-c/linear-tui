//! Moving the cursor through lists and scrolling the detail view.

use super::*;

impl App {
    /// Move the list cursor on the current screen by `delta` rows, requesting
    /// the next page if the cursor nears the end of a paginated list.
    pub fn move_selection(&mut self, delta: isize) {
        match self.nav.screen {
            Screen::ProjectList => {
                let len = self.project_rows().len();
                let mut index = self.project_cursor();
                Self::nav_by(len, &mut index, delta);
                *self.project_cursor_mut() = index;
                self.maybe_prefetch_projects();
            }
            Screen::CycleList => {
                Self::nav_by(
                    self.store.cycles.items.len(),
                    &mut self.view.selected_cycle_index,
                    delta,
                );
                self.maybe_prefetch_cycles();
            }
            Screen::ViewList => {
                let len = self.listed_views().len();
                Self::nav_by(len, &mut self.view.selected_view_index, delta);
            }
            Screen::IssueDetail => {}
            // Every issue list scrolls the same way, whichever one it is.
            Screen::IssueList | Screen::ProjectDetail | Screen::CycleDetail => {
                let len = self.visible_issues().len();
                let mut index = self.selected_index();
                Self::nav_by(len, &mut index, delta);
                *self.selected_index_mut() = index;
                self.maybe_prefetch_within(len);
            }
        }
    }

    /// Number of rows a half-page jump should cover.
    pub fn half_page(&self) -> isize {
        (self.frame.list_viewport / 2).max(1) as isize
    }

    /// Jump to the first row of the current list.
    pub fn select_first(&mut self) {
        self.move_selection(isize::MIN / 2);
    }

    /// Jump to the last row of the current list, pulling in another page if there is one.
    pub fn select_last(&mut self) {
        self.move_selection(isize::MAX / 2);
    }

    fn maybe_prefetch_cycles(&mut self) {
        if self.view.selected_cycle_index + PREFETCH_MARGIN < self.store.cycles.items.len()
            || !self.store.cycles.page_info.has_next_page
        {
            return;
        }
        if let (Some(team_id), Some(cursor)) = (
            self.team_id(),
            self.store.cycles.page_info.end_cursor.clone(),
        ) && self.outbox.prefetched.insert(cursor.clone())
        {
            self.request(Request::Cycles {
                team_id,
                after: Some(cursor),
            });
        }
    }

    /// Largest scroll offset that still shows content.
    fn max_detail_scroll(&self) -> u16 {
        self.frame
            .detail_lines
            .saturating_sub(self.frame.detail_viewport)
    }

    pub fn scroll_down(&mut self) {
        self.view.detail_scroll = self
            .view
            .detail_scroll
            .saturating_add(1)
            .min(self.max_detail_scroll());
    }

    pub fn scroll_up(&mut self) {
        self.view.detail_scroll = self.view.detail_scroll.saturating_sub(1);
    }

    pub fn scroll_by(&mut self, delta: i16) {
        let next = self.view.detail_scroll as i32 + delta as i32;
        self.view.detail_scroll = next.clamp(0, self.max_detail_scroll() as i32) as u16;
    }

    /// Mouse wheel: move the list cursor, or scroll the detail body.
    pub fn scroll_list_or_detail(&mut self, delta: i16) {
        if self.nav.screen == Screen::IssueDetail {
            self.scroll_by(delta);
        } else {
            self.move_selection(delta as isize);
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.view.detail_scroll = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.view.detail_scroll = self.max_detail_scroll();
    }

    pub fn nav_by(len: usize, index: &mut usize, delta: isize) {
        if len == 0 {
            *index = 0;
            return;
        }
        let next = (*index as isize)
            .saturating_add(delta)
            .clamp(0, len as isize - 1);
        *index = next as usize;
    }
}
