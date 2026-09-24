//! How the screen is shaped: cursors, filters, what is open over the content.

use std::collections::HashSet;

use super::{Input, InputMode, IssueList, IssueLists, NewIssueForm, PerSource, Popup, ViewKind};
use crate::api::ids::FavoriteId;
use crate::config::Config;
use crate::grouping::GroupBy;

#[derive(Debug)]
pub struct ViewState {
    pub input_mode: InputMode,
    pub popup: Popup,
    /// The highlighted row of the open popup.
    pub popup_index: usize,
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
