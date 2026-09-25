//! What the renderer records about the frame it drew.
//!
//! Hit-testing runs against the last frame, so a click lands on what the
//! user actually saw, and page-sized moves use the heights it measured.
//! Only renderers write here; the rest of the app reads.

use ratatui::layout::Rect;

use super::{Chip, ListRow, PerSource, SidebarRow};

#[derive(Debug, Default)]
pub struct FrameState {
    /// Display rows the content list drew — only the visible slice, top to
    /// bottom.
    pub list_rows: Vec<ListRow>,
    /// For the non-issue lists (projects, cycles, views): which item each
    /// visible row of `list_area` selects, top to bottom.
    pub row_targets: Vec<Option<usize>>,
    /// Screen area those rows occupy, so a click can be mapped back to a row.
    pub list_area: Rect,
    /// Height of the visible list body, for half-page jumps.
    pub list_viewport: u16,
    /// Where the sidebar was drawn.
    pub sidebar_area: Rect,
    /// Rows the sidebar drew, for keyboard and mouse hit-testing.
    pub sidebar_rows: Vec<SidebarRow>,
    /// First sidebar row on screen, when the tree is taller than the pane.
    pub sidebar_offset: usize,
    /// Screen area of each toolbar chip, left to right.
    pub chip_areas: Vec<(Rect, Chip)>,
    /// Where the open popup drew its entries, so they are clickable too.
    pub popup_area: Rect,
    /// First popup entry on screen, when the list scrolls.
    pub popup_offset: usize,
    /// Rendered height of the detail body, so scrolling can clamp.
    pub detail_lines: u16,
    pub detail_viewport: u16,
    pub offsets: Offsets,
}

/// Scroll offsets of the lists, kept across frames so the viewport does not
/// jump when the rows under it change.
#[derive(Debug, Default)]
pub struct Offsets {
    pub issues: PerSource<usize>,
    pub views: usize,
    pub view_projects: usize,
    pub projects: usize,
    pub cycles: usize,
}
