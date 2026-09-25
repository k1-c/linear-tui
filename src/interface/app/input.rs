//! Text fields and the new-issue form.

use super::*;

/// A single-line text field with a cursor, used for search and comment input.
#[derive(Debug, Clone, Default)]
pub struct Input {
    pub value: String,
    /// Cursor position as a byte offset into `value`.
    pub cursor: usize,
}

impl Input {
    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn insert(&mut self, c: char) {
        self.value.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn backspace(&mut self) {
        if let Some(prev) = self.prev_boundary() {
            self.value.remove(prev);
            self.cursor = prev;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.value.len() {
            self.value.remove(self.cursor);
        }
    }

    pub fn left(&mut self) {
        if let Some(prev) = self.prev_boundary() {
            self.cursor = prev;
        }
    }

    pub fn right(&mut self) {
        if let Some(next) = self.next_boundary() {
            self.cursor = next;
        }
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.value.len();
    }

    /// Ctrl+U — delete everything before the cursor.
    pub fn kill_to_start(&mut self) {
        self.value.drain(..self.cursor);
        self.cursor = 0;
    }

    /// Ctrl+K — delete everything from the cursor onward.
    pub fn kill_to_end(&mut self) {
        self.value.truncate(self.cursor);
    }

    /// Ctrl+W — delete the whitespace-delimited word before the cursor.
    pub fn kill_word(&mut self) {
        let head = &self.value[..self.cursor];
        let trimmed = head.trim_end();
        let start = trimmed
            .rfind(char::is_whitespace)
            .map(|i| i + trimmed[i..].chars().next().map_or(1, char::len_utf8))
            .unwrap_or(0);
        self.value.drain(start..self.cursor);
        self.cursor = start;
    }

    fn prev_boundary(&self) -> Option<usize> {
        if self.cursor == 0 {
            return None;
        }
        self.value[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
    }

    fn next_boundary(&self) -> Option<usize> {
        self.value[self.cursor..]
            .chars()
            .next()
            .map(|c| self.cursor + c.len_utf8())
    }
}

/// Which field of the new-issue form has focus.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FormField {
    #[default]
    Title,
    Description,
    Priority,
}

impl FormField {
    pub(super) fn next(self) -> Self {
        match self {
            Self::Title => Self::Description,
            Self::Description => Self::Priority,
            Self::Priority => Self::Title,
        }
    }

    pub(super) fn prev(self) -> Self {
        match self {
            Self::Title => Self::Priority,
            Self::Description => Self::Title,
            Self::Priority => Self::Description,
        }
    }
}

/// Draft state for the issue-creation form.
#[derive(Debug, Default)]
pub struct NewIssueForm {
    pub title: Input,
    pub description: Input,
    pub priority: Priority,
    pub field: FormField,
}
