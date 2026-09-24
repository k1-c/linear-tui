//! The command palette's state. What it lists and how a command runs is in
//! the top-level `palette` module, which reads the binding table.

use super::*;

/// How many recently run commands the palette remembers.
const RECENT: usize = 5;

impl App {
    /// Linear's Cmd/Ctrl+K.
    pub fn open_palette(&mut self) {
        self.view.popup = Popup::Palette;
        let issue = self.focused_issue_id();
        let palette = &mut self.view.palette;
        palette.query.clear();
        palette.selected = 0;
        palette.issue = issue;
    }

    /// Note a command as just run, for the empty palette's first rows.
    pub fn remember_command(&mut self, title: &'static str) {
        let recent = &mut self.view.palette.recent;
        recent.retain(|t| *t != title);
        recent.insert(0, title);
        recent.truncate(RECENT);
    }
}
