//! The command palette (Controller): Linear's Cmd/Ctrl+K menu.
//!
//! It lists the [`keys::BINDINGS`] rows that carry a [`keys::Command`] for
//! the context it was opened from, narrowed by a fuzzy query, and runs the
//! chosen row's action exactly as its key would. Its state — query, cursor,
//! the issue it was opened on — lives in `app.view.palette`.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::fuzzy;
use crate::keys::{self, Binding};

/// A keyword hit ranks below a hit on the title itself.
const KEYWORD_PENALTY: i32 = 20;

/// One row of the palette.
pub struct Entry {
    pub binding: &'static Binding,
    pub title: &'static str,
    pub section: &'static str,
    /// The keys that do the same, as the palette shows them.
    pub keys: String,
    /// Char indices of `title` the query matched, for highlighting.
    pub positions: Vec<usize>,
}

impl Entry {
    fn of(binding: &'static Binding, positions: Vec<usize>) -> Option<Self> {
        let command = binding.command?;
        Some(Self {
            binding,
            title: command.title,
            section: command.section.title(),
            keys: binding.keys_label(),
            positions,
        })
    }
}

/// What the palette lists for its query, best match first.
///
/// With no query, recently run commands come first and the rest follow by
/// section, so the palette doubles as a map of what can be done here.
pub fn entries(app: &App) -> Vec<Entry> {
    // The palette does not change the input mode, screen, or sidebar focus,
    // so this is the context it was opened from.
    let commands = keys::commands(keys::context(app));
    let query = app.view.palette.query.value.trim();

    if query.is_empty() {
        let recent = &app.view.palette.recent;
        let rank = |b: &&Binding| {
            let title = b.command.map(|c| c.title);
            recent
                .iter()
                .position(|t| Some(*t) == title)
                .unwrap_or(usize::MAX)
        };
        let mut commands = commands;
        // Stable: commands never run keep their section order.
        commands.sort_by_key(rank);
        return commands
            .into_iter()
            .filter_map(|b| Entry::of(b, Vec::new()))
            .collect();
    }

    let mut scored: Vec<(i32, Entry)> = commands
        .into_iter()
        .filter_map(|binding| {
            let command = binding.command?;
            let (score, positions) = match fuzzy::score(query, command.title) {
                Some(m) => (m.score, m.positions),
                None => {
                    let best = command
                        .keywords
                        .iter()
                        .filter_map(|k| fuzzy::score(query, k))
                        .map(|m| m.score)
                        .max()?;
                    (best - KEYWORD_PENALTY, Vec::new())
                }
            };
            Some((score, Entry::of(binding, positions)?))
        })
        .collect();
    scored.sort_by_key(|(score, _)| -score);
    scored.into_iter().map(|(_, entry)| entry).collect()
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => app.close_popup(),
        // The key that opened it closes it, as in Linear.
        KeyCode::Char('k') if ctrl => app.close_popup(),
        KeyCode::Enter => run_selected(app),
        KeyCode::Down | KeyCode::Tab => move_by(app, 1),
        KeyCode::Up | KeyCode::BackTab => move_by(app, -1),
        KeyCode::Char('n') if ctrl => move_by(app, 1),
        KeyCode::Char('p') if ctrl => move_by(app, -1),
        KeyCode::PageDown => move_by(app, 10),
        KeyCode::PageUp => move_by(app, -10),
        _ => edit(app, key.code, ctrl),
    }
}

/// Readline-style editing of the query, as in every other text field.
fn edit(app: &mut App, code: KeyCode, ctrl: bool) {
    let query = &mut app.view.palette.query;
    let before = query.value.clone();
    match code {
        KeyCode::Char('w') if ctrl => query.kill_word(),
        KeyCode::Char('u') if ctrl => query.kill_to_start(),
        KeyCode::Char('a') if ctrl => query.home(),
        KeyCode::Char('e') if ctrl => query.end(),
        KeyCode::Char(c) if !ctrl => query.insert(c),
        KeyCode::Backspace => query.backspace(),
        KeyCode::Delete => query.delete(),
        KeyCode::Left => query.left(),
        KeyCode::Right => query.right(),
        KeyCode::Home => query.home(),
        KeyCode::End => query.end(),
        _ => {}
    }
    if app.view.palette.query.value != before {
        app.view.palette.selected = 0;
    }
}

fn move_by(app: &mut App, delta: isize) {
    let len = entries(app).len();
    App::nav_by(len, &mut app.view.palette.selected, delta);
}

fn run_selected(app: &mut App) {
    let binding = entries(app)
        .get(app.view.palette.selected)
        .map(|e| e.binding);
    match binding {
        Some(binding) => run(app, binding),
        None => app.close_popup(),
    }
}

/// Close the palette and run a command the way its key would.
fn run(app: &mut App, binding: &'static Binding) {
    let issue = app.view.palette.issue.clone();
    app.close_popup();
    if issue.is_some() && app.focused_issue_id() != issue {
        app.set_status("The issue under the cursor changed \u{2014} nothing was done");
        return;
    }
    if let Some(command) = binding.command {
        app.remember_command(command.title);
    }
    (binding.action)(app);
}

/// A click while the palette is open: run the row under it, or close the
/// palette when the click lands elsewhere.
pub fn click(app: &mut App, x: u16, y: u16) {
    let area = app.frame.popup_area;
    let inside = area.width > 0
        && (area.x..area.x + area.width).contains(&x)
        && (area.y..area.y + area.height).contains(&y);
    if !inside {
        app.close_popup();
        return;
    }
    let index = app.frame.popup_offset + (y - area.y) as usize;
    if let Some(binding) = entries(app).get(index).map(|e| e.binding) {
        run(app, binding);
    }
}

/// The mouse wheel over the palette moves its cursor.
pub fn wheel(app: &mut App, delta: i16) {
    move_by(app, delta as isize);
}

#[cfg(test)]
mod tests;
