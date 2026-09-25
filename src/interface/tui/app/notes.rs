//! Notes for an agent, as the user writes them: which issue a note is
//! about, and where the prompt goes. The rules are `usecase::notes`.

use super::*;
use crate::core::entity::Handoff;
use crate::core::entity::Subject;
use crate::core::usecase::notes::{self, Delivery};

impl App {
    /// `n`: a note on the issue under the cursor — or on the view, where
    /// there is none.
    pub fn start_note_on_issue(&mut self) {
        let about = self.focused_issue().map(|i| Subject {
            identifier: i.identifier.clone(),
            title: i.title.clone(),
        });
        self.start_note(about);
    }

    /// `N`: a note on the whole view.
    pub fn start_note_on_view(&mut self) {
        self.start_note(None);
    }

    fn start_note(&mut self, about: Option<Subject>) {
        self.view.note_about = about;
        self.view.note.clear();
        self.view.input_mode = InputMode::Note;
    }

    pub fn submit_note(&mut self) {
        let body = std::mem::take(&mut self.view.note.value);
        self.view.input_mode = InputMode::Normal;
        self.view.note.clear();
        let about = self.view.note_about.take();
        if let Some(count) = notes::add(&mut self.notes, about, &body) {
            let plural = if count == 1 { "" } else { "s" };
            self.set_status(format!(
                "{count} note{plural} — Ctrl+S sends them to your agent"
            ));
        }
    }

    pub fn cancel_note(&mut self) {
        self.view.input_mode = InputMode::Normal;
        self.view.note.clear();
        self.view.note_about = None;
    }

    /// `Ctrl+S`: send the notes as one prompt.
    pub fn send_notes(&mut self) {
        let view = self.view_label();
        match notes::send(&mut self.notes, view, self.herdr) {
            Ok(Delivery::Herdr(request)) => self.request(request),
            Ok(Delivery::Clipboard(text)) => {
                self.outbox.clipboard = Some(text);
                self.set_status("Notes copied — paste them into your agent");
            }
            // Say how to write one, in the keys of this terminal.
            Err(refusal) => self.set_status(format!(
                "{refusal} — n notes the issue under the cursor, N the whole view"
            )),
        }
    }

    pub fn discard_notes(&mut self) {
        let discarded = notes::discard(&mut self.notes);
        self.set_status(match discarded {
            0 => "No notes to discard".to_string(),
            1 => "Discarded 1 note".to_string(),
            n => format!("Discarded {n} notes"),
        });
    }

    /// A hand-off to herdr failed. A prompt is kept by copying it.
    pub(super) fn handoff_failed(&mut self, handoff: &Handoff, error: &str) {
        let Some(text) = notes::salvage(handoff) else {
            self.set_error(format!("Could not reach herdr: {error}"));
            return;
        };
        self.outbox.clipboard = Some(text);
        self.set_error(format!(
            "Could not hand the notes to herdr: {error}\n\nThey are on the clipboard instead."
        ));
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
    fn n_notes_the_issue_under_the_cursor_and_ctrl_s_copies_the_prompt() {
        let mut app = app();
        app.herdr = false;
        app.start_note_on_issue();
        type_note(&mut app, "look");
        assert_eq!(app.notes.len(), 1);
        app.send_notes();
        let text = app.outbox.clipboard.as_deref().unwrap();
        assert!(text.contains("**ENG-42** Checkout fails: look"), "{text}");
        assert!(text.contains("(Engineering › Issues)"), "{text}");
        assert!(app.notes.is_empty());
    }

    #[test]
    fn escape_keeps_nothing() {
        let mut app = app();
        app.start_note_on_view();
        app.view.note.insert('x');
        app.cancel_note();
        assert!(app.notes.is_empty());
        assert_eq!(app.view.input_mode, InputMode::Normal);
    }

    #[test]
    fn inside_herdr_ctrl_s_hands_the_notes_to_the_plugin() {
        let mut app = app();
        app.herdr = true;
        app.start_note_on_view();
        type_note(&mut app, "look");
        app.send_notes();
        assert!(matches!(
            app.outbox.requests.front(),
            Some(Request::Notes(
                crate::core::usecase::notes::Request::Deliver(Handoff::Prompt { .. })
            ))
        ));
        assert!(app.outbox.clipboard.is_none());
    }
}
