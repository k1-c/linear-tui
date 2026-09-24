//! The command palette's state. What it lists and how a command runs is in
//! the top-level `palette` module, which reads the binding table.

use std::time::{Duration, Instant};

use super::*;

/// How many recently run commands the palette remembers.
const RECENT: usize = 5;
/// How long the query must rest before Linear is searched for it, so a burst
/// of typing sends one request rather than one per key.
const SEARCH_DELAY: Duration = Duration::from_millis(250);
/// A shorter query than this is searched only when it looks like an ID.
const SEARCH_MIN_CHARS: usize = 3;

/// `ENG-123`, or the start of one — `eng-1`.
fn looks_like_identifier(query: &str) -> bool {
    let Some((team, number)) = query.split_once('-') else {
        return false;
    };
    !team.is_empty()
        && team.chars().all(|c| c.is_ascii_alphabetic())
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
}

impl App {
    /// Linear's Cmd/Ctrl+K.
    pub fn open_palette(&mut self) {
        self.view.popup = Popup::Palette;
        let issue = self.focused_issue_id();
        let palette = &mut self.view.palette;
        palette.query.clear();
        palette.selected = 0;
        palette.issue = issue;
        palette.results.clear();
        palette.searching = false;
        palette.search_due = None;
    }

    /// The query changed: schedule a search of the workspace for it, unless
    /// it asks for commands only (`>`) or is too short to be worth one.
    pub fn palette_query_changed(&mut self, now: Instant) {
        let palette = &mut self.view.palette;
        let query = palette.query.value.trim();
        let worth_searching = !query.starts_with('>')
            && (query.chars().count() >= SEARCH_MIN_CHARS || looks_like_identifier(query));
        palette.search_due = worth_searching.then(|| now + SEARCH_DELAY);
        if !worth_searching {
            palette.searching = false;
            palette.results.clear();
        }
    }

    /// Called from the main loop: send the pending search once the query has
    /// rested. Returns true when it did.
    pub fn flush_palette_search(&mut self, now: Instant) -> bool {
        if self.view.popup != Popup::Palette {
            self.view.palette.search_due = None;
            return false;
        }
        let palette = &mut self.view.palette;
        match palette.search_due {
            Some(due) if now >= due => {
                palette.search_due = None;
                palette.seq += 1;
                palette.searching = true;
                let request = Request::PaletteSearch {
                    term: palette.query.value.trim().to_string(),
                    seq: palette.seq,
                };
                self.request(request);
                true
            }
            _ => false,
        }
    }

    /// Linear answered a palette search; keep it if it is for the latest query.
    pub(super) fn accept_palette_results(&mut self, seq: u64, issues: Vec<Issue>) {
        let palette = &mut self.view.palette;
        if seq != palette.seq {
            return;
        }
        palette.searching = false;
        palette.results = issues;
    }

    /// Note a command as just run, for the empty palette's first rows.
    pub fn remember_command(&mut self, title: &'static str) {
        let recent = &mut self.view.palette.recent;
        recent.retain(|t| *t != title);
        recent.insert(0, title);
        recent.truncate(RECENT);
    }
}
