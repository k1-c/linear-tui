//! Notes for an agent: remarks on the issue under the cursor or on the whole
//! view, collected while reading and sent together as one prompt.
//!
//! Inside herdr the prompt goes to the agent next to linear-tui through the
//! plugin; anywhere else it is copied to the clipboard, to paste into
//! whichever agent you use.

use super::*;
use crate::herdr::Handoff;

/// One note, and what it is about.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    /// The issue it is about — `(identifier, title)` — or `None` for the
    /// view as a whole.
    pub about: Option<(String, String)>,
    pub body: String,
}

/// Every prompt ends with this, so an agent that has never heard of
/// linear-tui can still look closer.
pub const HINT: &str = "`linear-tui context` shows this view with the row under my cursor; \
`linear-tui issue show|comment|status <ID>` and `linear-tui issue create --team <key> --title <text>` \
act on Linear.";

impl App {
    /// `n`: a note on the issue under the cursor — or on the view, where
    /// there is none.
    pub fn start_note_on_issue(&mut self) {
        let about = self
            .focused_issue()
            .map(|i| (i.identifier.clone(), i.title.clone()));
        self.start_note(about);
    }

    /// `N`: a note on the whole view.
    pub fn start_note_on_view(&mut self) {
        self.start_note(None);
    }

    fn start_note(&mut self, about: Option<(String, String)>) {
        self.view.note_about = about;
        self.view.note.clear();
        self.view.input_mode = InputMode::Note;
    }

    pub fn submit_note(&mut self) {
        let body = std::mem::take(&mut self.view.note.value).trim().to_string();
        self.view.input_mode = InputMode::Normal;
        self.view.note.clear();
        if body.is_empty() {
            return;
        }
        self.view.notes.push(Note {
            about: self.view.note_about.take(),
            body,
        });
        let count = self.view.notes.len();
        let plural = if count == 1 { "" } else { "s" };
        self.set_status(format!(
            "{count} note{plural} — Ctrl+S sends them to your agent"
        ));
    }

    pub fn cancel_note(&mut self) {
        self.view.input_mode = InputMode::Normal;
        self.view.note.clear();
        self.view.note_about = None;
    }

    /// Send the notes as one prompt: to herdr's agent inside herdr, to the
    /// clipboard anywhere else.
    pub fn send_notes(&mut self) {
        if self.view.notes.is_empty() {
            self.set_status("No notes yet — n notes the issue under the cursor, N the whole view");
            return;
        }
        let handoff = self.notes_prompt();
        self.view.notes.clear();
        if self.herdr {
            self.request(Request::Herdr(handoff));
        } else {
            let Handoff::Prompt { text, .. } = handoff;
            self.outbox.clipboard = Some(text);
            self.set_status("Notes copied — paste them into your agent");
        }
    }

    pub fn discard_notes(&mut self) {
        let count = self.view.notes.len();
        self.view.notes.clear();
        self.set_status(match count {
            0 => "No notes to discard".to_string(),
            1 => "Discarded 1 note".to_string(),
            n => format!("Discarded {n} notes"),
        });
    }

    /// The hand-off to herdr failed: keep the prompt by copying it.
    pub(super) fn notes_not_delivered(&mut self, handoff: &Handoff, error: &str) {
        let Handoff::Prompt { text, .. } = handoff;
        self.outbox.clipboard = Some(text.clone());
        self.set_error(format!(
            "Could not hand the notes to herdr: {error}\n\nThey are on the clipboard instead."
        ));
    }

    /// The notes as a prompt, whole and in parts.
    pub fn notes_prompt(&self) -> Handoff {
        let notes: Vec<String> = self
            .view
            .notes
            .iter()
            .map(|note| {
                let about = match &note.about {
                    Some((identifier, title)) => format!("**{identifier}** {title}"),
                    None => "**This view**".to_string(),
                };
                // A multi-line note stays inside its list item.
                let body = note.body.replace('\n', "\n  ");
                format!("- {about}: {body}")
            })
            .collect();
        let notes = notes.join("\n");
        let view = self.view_label();
        let text = format!(
            "My notes on what I am looking at in linear-tui ({view}):\n\n{notes}\n\n({HINT})"
        );
        Handoff::Prompt {
            text,
            notes,
            view,
            hint: HINT.to_string(),
        }
    }

    /// Where the user is, in words: `Engineering › Issues › ENG-42`.
    pub fn view_label(&self) -> String {
        let team = |index: usize| {
            self.store
                .teams
                .get(index)
                .map_or_else(|| "Team".to_string(), |t| t.name.clone())
        };
        let mut parts = vec![match self.nav.dest {
            Nav::MyIssues => "My Issues".to_string(),
            Nav::Views => "Views".to_string(),
            Nav::View(i) => self
                .store
                .custom_views
                .get(i)
                .map_or_else(|| "View".to_string(), |v| format!("view “{}”", v.name)),
            Nav::Team(i, section) => format!(
                "{} › {}",
                team(i),
                match section {
                    TeamSection::Issues => "Issues",
                    TeamSection::Cycles => "Cycles",
                    TeamSection::Projects => "Projects",
                    TeamSection::Views => "Views",
                }
            ),
            Nav::Favorite(_) => "Favorites".to_string(),
        }];
        let behind = match self.nav.screen {
            Screen::IssueDetail => self.nav.detail_return,
            screen => screen,
        };
        if behind == Screen::ProjectDetail
            && let Some(project) = &self.nav.current_project
        {
            parts.push(project.name.clone());
        }
        if behind == Screen::CycleDetail
            && let Some(cycle) = &self.nav.current_cycle
        {
            parts.push(cycle.label());
        }
        if self.nav.screen == Screen::IssueDetail
            && let Some(issue) = &self.store.current_issue
        {
            parts.push(issue.identifier.clone());
        }
        parts.join(" › ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new(&Config::default());
        app.store.teams =
            vec![serde_json::from_str(r#"{"id":"t","name":"Engineering","key":"ENG"}"#).unwrap()];
        app.store.issues[IssueSource::Team].items = vec![
            serde_json::from_str(
                r#"{"id":"i1","identifier":"ENG-42","title":"Checkout fails","priority":0}"#,
            )
            .unwrap(),
        ];
        app.store.issues[IssueSource::Team].loaded = true;
        app.outbox.requests.clear();
        app
    }

    fn type_note(app: &mut App, text: &str) {
        for c in text.chars() {
            app.view.note.insert(c);
        }
        app.submit_note();
    }

    #[test]
    fn notes_name_their_issue_or_the_view_and_end_with_the_hint() {
        let mut app = app();
        app.start_note_on_issue();
        type_note(&mut app, "reproduce first\nthen fix");
        app.start_note_on_view();
        type_note(&mut app, "the top three are one bug");
        let Handoff::Prompt {
            text, notes, view, ..
        } = app.notes_prompt();
        assert_eq!(view, "Engineering › Issues");
        assert_eq!(
            notes,
            "- **ENG-42** Checkout fails: reproduce first\n  then fix\n\
             - **This view**: the top three are one bug"
        );
        assert!(
            text.starts_with(
                "My notes on what I am looking at in linear-tui (Engineering › Issues):"
            )
        );
        assert!(text.ends_with(&format!("({HINT})")));
    }

    #[test]
    fn an_empty_note_is_dropped_and_escape_keeps_nothing() {
        let mut app = app();
        app.start_note_on_view();
        type_note(&mut app, "   ");
        app.start_note_on_view();
        app.view.note.insert('x');
        app.cancel_note();
        assert!(app.view.notes.is_empty());
        assert_eq!(app.view.input_mode, InputMode::Normal);
    }

    #[test]
    fn outside_herdr_the_notes_go_to_the_clipboard() {
        let mut app = app();
        app.herdr = false;
        app.send_notes();
        assert!(app.outbox.clipboard.is_none(), "nothing to send yet");
        app.start_note_on_issue();
        type_note(&mut app, "look");
        app.send_notes();
        assert!(
            app.outbox
                .clipboard
                .as_deref()
                .is_some_and(|t| t.contains("ENG-42"))
        );
        assert!(app.view.notes.is_empty());
        assert!(app.outbox.requests.is_empty());
    }

    #[test]
    fn inside_herdr_the_notes_are_handed_to_the_plugin() {
        let mut app = app();
        app.herdr = true;
        app.start_note_on_view();
        type_note(&mut app, "look");
        app.send_notes();
        assert!(matches!(
            app.outbox.requests.front(),
            Some(Request::Herdr(Handoff::Prompt { .. }))
        ));
        assert!(app.outbox.clipboard.is_none());
    }
}
