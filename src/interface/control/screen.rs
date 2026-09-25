//! The screen as an agent reads it (`linear-tui tui screen`): the frame as
//! text, and what the frame alone does not say — where focus is, what is
//! open over it, what the keys here do, and which commands the palette runs.
//!
//! An agent works the TUI the way a person does: it reads this, presses a
//! key or runs a command, and reads it again. The Markdown form is the
//! contract in `docs/cli.md`; the JSON form only ever gains fields.

use serde::{Deserialize, Serialize};

use crate::interface::tui::app::{App, InputMode, Popup};
use crate::interface::tui::keys;

/// What is on the screen of a running linear-tui.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenReport {
    /// Where the user is, in words: `Engineering › Issues › ENG-42`.
    pub showing: String,
    /// The frame, one line per terminal row, trailing blanks trimmed.
    pub lines: Vec<String>,
    pub width: u16,
    pub height: u16,
    /// Where keys go: a screen (`issue_list`, `issue_detail`, …), the
    /// `sidebar`, a text field, or a `g` chord waiting for its second key.
    pub focus: String,
    /// What is open over the screen: a picker, the palette, the help, an
    /// error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<String>,
    /// The issue under the cursor, or open: `ENG-42 Checkout fails`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The status line's message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The error popup's text; any key dismisses it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Whether Linear has not answered everything asked yet.
    pub loading: bool,
    /// Notes written for the agent and not yet sent.
    pub notes: usize,
    /// The keys the status bar offers here.
    pub keys: Vec<KeyHint>,
    /// The palette commands that apply here, by title (`linear-tui tui run`).
    pub commands: Vec<CommandEntry>,
    /// What linear-tui did not do because nobody is at the desktop: pages
    /// it would have opened in a browser, hand-offs to herdr.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub held: Vec<String>,
}

/// A key the status bar offers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyHint {
    pub keys: String,
    pub does: String,
}

/// A palette command that applies here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEntry {
    pub title: String,
    /// The keys that run it too, when it has some.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<String>,
}

/// Report the screen `app` is on. `lines` is the frame last drawn, `held`
/// what was held back from the desktop.
pub fn report(app: &App, lines: Vec<String>, size: (u16, u16), held: Vec<String>) -> ScreenReport {
    let ctx = keys::context(app);
    ScreenReport {
        showing: app.view_label(),
        lines,
        width: size.0,
        height: size.1,
        focus: focus(app, ctx),
        overlay: overlay(app),
        issue: app
            .focused_issue()
            .map(|i| format!("{} {}", i.identifier, i.title)),
        status: app.view.status_message.clone(),
        error: app.view.error_popup.clone(),
        loading: app.loading(),
        notes: app.notes.len(),
        keys: keys::hints(ctx, app.herdr)
            .into_iter()
            .map(|h| KeyHint {
                keys: h.keys.to_string(),
                does: h.what.to_string(),
            })
            .collect(),
        commands: keys::commands(ctx, app.herdr)
            .into_iter()
            .filter_map(|b| {
                let command = b.command?;
                let keys = b.keys_label();
                Some(CommandEntry {
                    title: command.title.to_string(),
                    keys: (!keys.is_empty()).then_some(keys),
                })
            })
            .collect(),
        held,
    }
}

fn focus(app: &App, ctx: keys::Ctx) -> String {
    let name = match app.view.input_mode {
        InputMode::Search => "search field",
        InputMode::Comment => "comment field",
        InputMode::Note => "note field",
        InputMode::NewIssue => match ctx {
            keys::Ctx::IssueDescription => "new issue: description",
            keys::Ctx::IssuePriority => "new issue: priority",
            _ => "new issue: title",
        },
        InputMode::Normal => return snake(&format!("{ctx:?}")),
    };
    name.to_string()
}

fn overlay(app: &App) -> Option<String> {
    if app.view.error_popup.is_some() {
        return Some("error (any key dismisses it)".into());
    }
    if app.view.show_help {
        return Some("help (j/k scroll, any other key closes)".into());
    }
    let query = &app.view.popup_query.value;
    let picker = |what: &str| {
        if query.is_empty() {
            format!("{what} picker (type to filter, Enter picks, Esc closes)")
        } else {
            format!("{what} picker, filtered by \"{query}\"")
        }
    };
    Some(match &app.view.popup {
        Popup::None => return None,
        Popup::Palette => format!(
            "command palette, query \"{}\" (Enter runs, Esc closes)",
            app.view.palette.query.value
        ),
        Popup::GroupBy => picker("group by"),
        Popup::TeamSelect => picker("team"),
        Popup::WorkspaceSelect => picker("workspace"),
        Popup::Filter(kind) => picker(&format!("{kind:?} filter").to_lowercase()),
        Popup::StatusChange(_) => picker("status"),
        Popup::PriorityChange(_) => picker("priority"),
        Popup::AssigneeChange(_) => picker("assignee"),
    })
}

/// `IssueList` as `issue_list`.
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

impl ScreenReport {
    /// The report as Markdown, for an agent's context.
    pub fn markdown(&self) -> String {
        let mut out = String::from("# linear-tui screen\n\n");
        out.push_str(&format!("- Showing: {}\n", self.showing));
        out.push_str(&format!("- Focus: {}\n", self.focus));
        if let Some(overlay) = &self.overlay {
            out.push_str(&format!("- Open: {overlay}\n"));
        }
        if let Some(issue) = &self.issue {
            out.push_str(&format!("- Issue: {issue}\n"));
        }
        if let Some(status) = &self.status {
            out.push_str(&format!("- Status: {status}\n"));
        }
        if let Some(error) = &self.error {
            out.push_str(&format!("- Error: {}\n", error.replace('\n', " ")));
        }
        if self.loading {
            out.push_str("- Loading: Linear has not answered everything yet\n");
        }
        if self.notes > 0 {
            out.push_str(&format!("- Notes: {} waiting to be sent\n", self.notes));
        }
        for held in &self.held {
            out.push_str(&format!("- Held: {held}\n"));
        }
        out.push_str(&format!("\n```text\n{}\n```\n", self.lines.join("\n")));
        if !self.keys.is_empty() {
            let keys: Vec<String> = self
                .keys
                .iter()
                .map(|k| format!("`{}` {}", k.keys, k.does))
                .collect();
            out.push_str(&format!("\nKeys here: {}\n", keys.join(" · ")));
        }
        if !self.commands.is_empty() {
            out.push_str("\n## Commands here\n\n");
            for command in &self.commands {
                match &command.keys {
                    Some(keys) => out.push_str(&format!("- {} (`{keys}`)\n", command.title)),
                    None => out.push_str(&format!("- {}\n", command.title)),
                }
            }
        }
        out.push_str(
            "\nPress keys with `linear-tui tui press <keys>`, type with `linear-tui tui type <text>`, \
             run a command with `linear-tui tui run <title>`.\n",
        );
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::core::store::IssueSource;

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
        app.outbox.requests.clear();
        app
    }

    #[test]
    fn the_report_says_where_focus_is_and_what_the_keys_do() {
        let app = app();
        let report = report(&app, vec!["frame".into()], (80, 24), Vec::new());
        assert_eq!(report.showing, "Engineering › Issues");
        assert_eq!(report.focus, "issue_list");
        assert_eq!(report.issue.as_deref(), Some("ENG-42 Checkout fails"));
        assert!(report.keys.iter().any(|k| k.does == "open"));
        assert!(
            report
                .commands
                .iter()
                .any(|c| c.title == "Change status\u{2026}")
        );
        assert!(report.overlay.is_none());
    }

    #[test]
    fn an_open_picker_and_a_text_field_are_named() {
        let mut app = app();
        app.open_status_change();
        let report = report(&app, Vec::new(), (80, 24), Vec::new());
        assert!(report.overlay.unwrap().starts_with("status picker"));

        let mut app = self::app();
        app.start_comment();
        assert_eq!(
            super::report(&app, Vec::new(), (80, 24), Vec::new()).focus,
            "comment field"
        );
    }

    #[test]
    fn the_markdown_carries_the_frame_and_the_commands() {
        let app = app();
        let text = report(
            &app,
            vec!["│ ENG-42 │".into()],
            (80, 24),
            vec!["opened https://x".into()],
        )
        .markdown();
        assert!(text.starts_with("# linear-tui screen\n\n- Showing: Engineering › Issues\n"));
        assert!(text.contains("```text\n│ ENG-42 │\n```"));
        assert!(text.contains("- Held: opened https://x"));
        assert!(text.contains("## Commands here"));
        assert!(text.contains("- Change status\u{2026} (`s`)"));
    }
}
