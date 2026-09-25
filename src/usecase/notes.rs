//! Notes for a coding agent: remarks on issues, or on the whole view,
//! collected while reading and sent together as one prompt.

use super::Refusal;
use crate::entity::{Handoff, Note, Notes, Subject};

/// What the notes use cases ask of herdr.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Hand the notes' prompt to the agent beside linear-tui.
    Deliver(Handoff),
}

/// Every prompt ends with this, so an agent that has never heard of
/// linear-tui can still look closer.
pub const HINT: &str = "`linear-tui context` shows this view with the row under my cursor; \
`linear-tui issue show|comment|status <ID>` and `linear-tui issue create --team <key> --title <text>` \
act on Linear.";

/// Where sent notes went.
#[derive(Debug, Clone, PartialEq)]
pub enum Delivery {
    /// Inside herdr: handed to the agent beside linear-tui by this request.
    Herdr(Request),
    /// Anywhere else: this text, for the clipboard.
    Clipboard(String),
}

/// **Write a note** on an issue (`n`) or on the whole view (`N`).
///
/// Surrounding blank space is trimmed, and a note with nothing left is
/// dropped. Returns how many notes now wait to be sent, or `None` when the
/// note was dropped.
pub fn add(notes: &mut Notes, about: Option<Subject>, body: &str) -> Option<usize> {
    let body = body.trim();
    if body.is_empty() {
        return None;
    }
    notes.push(Note {
        about,
        body: body.to_string(),
    });
    Some(notes.len())
}

/// **The prompt the notes make**, whole and in parts.
///
/// The prompt opens with where the user is (`view`, e.g. `Engineering ›
/// Issues`), lists the notes in the order they were written — each naming
/// its issue by identifier and title, or "This view" — and ends with
/// [`HINT`]. A note of several lines stays inside its list item.
pub fn prompt(notes: &Notes, view: String) -> Handoff {
    let lines: Vec<String> = notes
        .iter()
        .map(|note| {
            let about = match &note.about {
                Some(subject) => format!("**{}** {}", subject.identifier, subject.title),
                None => "**This view**".to_string(),
            };
            let body = note.body.replace('\n', "\n  ");
            format!("- {about}: {body}")
        })
        .collect();
    let notes = lines.join("\n");
    let text =
        format!("My notes on what I am looking at in linear-tui ({view}):\n\n{notes}\n\n({HINT})");
    Handoff::Prompt {
        text,
        notes,
        view,
        hint: HINT.to_string(),
    }
}

/// **Send the notes to my agent** (`Ctrl+S`), as one prompt.
///
/// Inside herdr the prompt is handed to the agent beside linear-tui; anywhere
/// else it goes to the clipboard, to paste into whichever agent the user
/// runs. Either way the notes are gone once sent. With no notes there is
/// nothing to send, and the user is told how to write one.
pub fn send(notes: &mut Notes, view: String, herdr: bool) -> Result<Delivery, Refusal> {
    if notes.is_empty() {
        return Err(Refusal::NoNotes);
    }
    let handoff = prompt(notes, view);
    notes.clear();
    Ok(if herdr {
        Delivery::Herdr(Request::Deliver(handoff))
    } else {
        Delivery::Clipboard(salvage(&handoff).unwrap_or_default())
    })
}

/// **Discard the notes** unsent. Returns how many there were.
pub fn discard(notes: &mut Notes) -> usize {
    let count = notes.len();
    notes.clear();
    count
}

/// **Keep a prompt herdr could not take.** When handing notes to herdr
/// fails, their prompt is what goes to the clipboard instead, so nothing
/// written is lost. Other hand-offs carry no text worth keeping.
pub fn salvage(handoff: &Handoff) -> Option<String> {
    match handoff {
        Handoff::Prompt { text, .. } => Some(text.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eng_42() -> Option<Subject> {
        Some(Subject {
            identifier: "ENG-42".into(),
            title: "Checkout fails".into(),
        })
    }

    /// A note on ENG-42 and a note on the view.
    fn two_notes() -> Notes {
        let mut notes = Notes::default();
        add(&mut notes, eng_42(), "reproduce first\nthen fix");
        add(&mut notes, None, "the top three are one bug");
        notes
    }

    // ------------------------------------------------------------- add

    /// Blank space around a note is trimmed, and the count is returned.
    #[test]
    fn a_note_is_trimmed_and_counted() {
        let mut notes = Notes::default();
        assert_eq!(add(&mut notes, None, "  look here \n"), Some(1));
        assert_eq!(notes.iter().next().unwrap().body, "look here");
        assert_eq!(add(&mut notes, eng_42(), "and here"), Some(2));
    }

    /// A note of nothing but blank space is dropped.
    #[test]
    fn a_blank_note_is_dropped() {
        let mut notes = Notes::default();
        assert_eq!(add(&mut notes, None, "  \n "), None);
        assert!(notes.is_empty());
    }

    // ---------------------------------------------------------- prompt

    /// The prompt says where the user is, then lists the notes in order,
    /// each naming its issue or the view.
    #[test]
    fn the_prompt_names_the_view_and_each_notes_issue() {
        let Handoff::Prompt {
            text, notes, view, ..
        } = prompt(&two_notes(), "Engineering › Issues".into())
        else {
            panic!("notes make a prompt");
        };
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
    }

    /// Every prompt ends with the hint on linear-tui's own commands.
    #[test]
    fn the_prompt_ends_with_the_hint() {
        let Handoff::Prompt { text, hint, .. } = prompt(&two_notes(), "Views".into()) else {
            panic!("notes make a prompt");
        };
        assert!(text.ends_with(&format!("({HINT})")));
        assert_eq!(hint, HINT);
    }

    // ------------------------------------------------------------ send

    /// Outside herdr the prompt goes to the clipboard, and the notes are gone.
    #[test]
    fn outside_herdr_the_notes_go_to_the_clipboard() {
        let mut notes = two_notes();
        let Ok(Delivery::Clipboard(text)) = send(&mut notes, "Views".into(), false) else {
            panic!("expected the clipboard");
        };
        assert!(text.contains("**ENG-42**"));
        assert!(notes.is_empty());
    }

    /// Inside herdr the prompt is handed to the plugin, and the notes are gone.
    #[test]
    fn inside_herdr_the_notes_are_handed_to_the_agent() {
        let mut notes = two_notes();
        let delivery = send(&mut notes, "Views".into(), true);
        assert!(matches!(
            delivery,
            Ok(Delivery::Herdr(Request::Deliver(Handoff::Prompt { .. })))
        ));
        assert!(notes.is_empty());
    }

    /// With no notes, nothing is sent and the user is told how to write one.
    #[test]
    fn with_no_notes_there_is_nothing_to_send() {
        assert_eq!(
            send(&mut Notes::default(), "Views".into(), false),
            Err(Refusal::NoNotes)
        );
    }

    // ------------------------------------------------- discard, salvage

    /// Discarding drops every note and says how many there were.
    #[test]
    fn discarding_drops_every_note() {
        let mut notes = two_notes();
        assert_eq!(discard(&mut notes), 2);
        assert!(notes.is_empty());
        assert_eq!(discard(&mut notes), 0);
    }

    /// A prompt herdr could not take is kept for the clipboard; a pane
    /// focus has nothing to keep.
    #[test]
    fn a_prompt_herdr_refused_is_kept_for_the_clipboard() {
        let handoff = prompt(&two_notes(), "Views".into());
        assert!(salvage(&handoff).is_some_and(|t| t.contains("ENG-42")));
        assert_eq!(salvage(&Handoff::Focus { pane: "p".into() }), None);
    }
}
