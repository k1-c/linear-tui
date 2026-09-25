//! Notes the user writes for a coding agent while reading. Not Linear data:
//! they live in linear-tui until sent.

/// The issue a note is about.
#[derive(Debug, Clone, PartialEq)]
pub struct Subject {
    pub identifier: String,
    pub title: String,
}

/// One note, and what it is about.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    /// The issue it is about, or `None` for the view as a whole.
    pub about: Option<Subject>,
    pub body: String,
}

/// The notes written and not yet sent, oldest first.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Notes(Vec<Note>);

impl Notes {
    /// How many notes wait to be sent.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The notes, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &Note> {
        self.0.iter()
    }

    pub fn push(&mut self, note: Note) {
        self.0.push(note);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}
