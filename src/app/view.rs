//! How the screen is shaped: cursors, filters, what is open over the content.

use std::collections::HashSet;

use super::{Input, InputMode, IssueList, IssueLists, NewIssueForm, PerSource, Popup, ViewKind};
use crate::api::ids::{FavoriteId, IssueId};
use crate::config::Config;
use crate::grouping::GroupBy;

#[derive(Debug)]
pub struct ViewState {
    pub input_mode: InputMode,
    pub popup: Popup,
    /// The highlighted row of the open popup, counted in what its query
    /// leaves — see `App::popup_rows`.
    pub popup_index: usize,
    /// What has been typed into the open popup to narrow it.
    pub popup_query: Input,
    /// Whether the open popup was reached from the command palette, so Esc
    /// steps back there instead of closing.
    pub popup_from_palette: bool,
    /// How each of the five issue lists is shaped.
    pub lists: IssueLists,
    pub group_by: GroupBy,
    /// Group keys the user has folded away.
    pub collapsed_groups: HashSet<String>,
    /// Which tab the Views pages show.
    pub view_kind: ViewKind,
    pub selected_view_index: usize,
    pub selected_view_project_index: usize,
    pub selected_project_index: usize,
    pub selected_cycle_index: usize,
    pub sidebar: Sidebar,
    pub palette: Palette,
    pub detail_scroll: u16,
    pub comment: Input,
    /// Draft issue, present only while the create form is open.
    pub new_issue: Option<NewIssueForm>,
    /// First key of a pending multi-key chord (Linear's `g …` sequences).
    pub pending_chord: Option<char>,
    pub show_help: bool,
    pub help_scroll: u16,
    pub status_message: Option<String>,
    pub error_popup: Option<String>,
    pub spinner_frame: usize,
}

#[derive(Debug, Default)]
pub struct Palette {
    pub query: Input,
    /// The highlighted entry, as an index into what the query matches.
    pub selected: usize,
    /// The issue under the cursor when the palette opened. A command runs on
    /// that issue or not at all — a page landing under the palette can move
    /// the cursor to another one.
    pub issue: Option<IssueId>,
    /// Commands run from the palette, most recent first, by title.
    pub recent: Vec<&'static str>,
}

#[derive(Debug)]
pub struct Sidebar {
    pub visible: bool,
    pub width: u16,
    /// True while the cursor is in the sidebar rather than the content pane.
    pub focus: bool,
    pub index: usize,
    /// Favorites folders the user has folded, by favorite id.
    pub collapsed_folders: HashSet<FavoriteId>,
}

impl ViewState {
    pub fn new(config: &Config) -> Self {
        Self {
            input_mode: InputMode::Normal,
            popup: Popup::None,
            popup_index: 0,
            popup_query: Input::default(),
            popup_from_palette: false,
            lists: PerSource::from_fn(IssueList::new),
            group_by: GroupBy::from_config(config.ui.group_by),
            collapsed_groups: HashSet::new(),
            view_kind: ViewKind::default(),
            selected_view_index: 0,
            selected_view_project_index: 0,
            selected_project_index: 0,
            selected_cycle_index: 0,
            sidebar: Sidebar {
                visible: config.ui.sidebar,
                width: config.ui.sidebar_width,
                focus: false,
                index: 0,
                collapsed_folders: HashSet::new(),
            },
            palette: Palette::default(),
            detail_scroll: 0,
            comment: Input::default(),
            new_issue: None,
            pending_chord: None,
            show_help: false,
            help_scroll: 0,
            status_message: None,
            error_popup: None,
            spinner_frame: 0,
        }
    }
}
