//! The command palette (Controller): Linear's Cmd/Ctrl+K menu.
//!
//! It lists the [`keys::BINDINGS`] rows that carry a [`keys::Command`] for
//! the context it was opened from, and — once something is typed — the
//! places the query names: teams, views, favorites, projects, cycles, and
//! issues, both those already loaded and what Linear's search returns. A
//! command runs exactly as its key would; a place is opened the way the
//! sidebar or a list would open it. Its state — query, cursor, the issue it
//! was opened on, search results — lives in `app.view.palette`.

use std::collections::HashSet;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::entity::{Cycle, Issue, Project};
use crate::interface::app::{App, Nav, Popup, SidebarAction, TeamSection};
use crate::interface::fuzzy;
use crate::interface::keys::{self, Binding};

/// A keyword hit ranks below a hit on the title itself.
const KEYWORD_PENALTY: i32 = 20;
/// An exact issue ID outranks every fuzzy hit.
const EXACT_ID: i32 = 1000;
/// Rows of places and of issues listed at most, each, so a short query over
/// a long list stays a menu.
const MAX_PLACES: usize = 20;
const MAX_ISSUES: usize = 30;

/// What choosing a row does.
pub enum Target<'a> {
    Command(&'static Binding),
    Go(Nav),
    Favorite(usize),
    Issue(&'a Issue),
    Project(&'a Project),
    Cycle(&'a Cycle),
}

/// One row of the palette.
pub struct Entry<'a> {
    pub target: Target<'a>,
    pub title: String,
    /// What kind of row it is, drawn muted on the right.
    pub section: &'static str,
    /// The keys that do the same, or a detail such as the issue's state.
    pub detail: String,
    /// Char indices of `title` the query matched, for highlighting.
    pub positions: Vec<usize>,
}

/// What the palette lists for its query, best match first.
///
/// With no query it lists the commands for where you are — recently run ones
/// first, the rest by section — so it doubles as a map of what can be done
/// here. A query also finds places and issues, listed after the commands;
/// starting it with `>` asks for commands only.
pub fn entries(app: &App) -> Vec<Entry<'_>> {
    let raw = app.view.palette.query.value.trim();
    let (query, commands_only) = match raw.strip_prefix('>') {
        Some(rest) => (rest.trim(), true),
        None => (raw, false),
    };
    let mut entries = commands(app, query);
    if !query.is_empty() && !commands_only {
        entries.extend(places(app, query));
        entries.extend(issues(app, query));
    }
    entries
}

fn command_entry(binding: &'static Binding, positions: Vec<usize>) -> Option<Entry<'static>> {
    let command = binding.command?;
    Some(Entry {
        target: Target::Command(binding),
        title: command.title.to_string(),
        section: command.section.title(),
        detail: binding.keys_label(),
        positions,
    })
}

fn commands(app: &App, query: &str) -> Vec<Entry<'static>> {
    // The palette does not change the input mode, screen, or sidebar focus,
    // so this is the context it was opened from.
    let mut commands = keys::commands(keys::context(app), app.herdr);

    if query.is_empty() {
        let recent = &app.view.palette.recent;
        // Stable: commands never run keep their section order.
        commands.sort_by_key(|b| {
            let title = b.command.map(|c| c.title);
            recent
                .iter()
                .position(|t| Some(*t) == title)
                .unwrap_or(usize::MAX)
        });
        return commands
            .into_iter()
            .filter_map(|b| command_entry(b, Vec::new()))
            .collect();
    }

    ranked(commands.into_iter().filter_map(|binding| {
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
        Some((score, command_entry(binding, positions)?))
    }))
}

/// Sort scored entries best first, keeping the input order among equals.
fn ranked<'a>(scored: impl Iterator<Item = (i32, Entry<'a>)>) -> Vec<Entry<'a>> {
    let mut scored: Vec<(i32, Entry)> = scored.collect();
    scored.sort_by_key(|(score, _)| -score);
    scored.into_iter().map(|(_, entry)| entry).collect()
}

/// An entry for `title` if the query matches it.
fn matched<'a>(
    query: &str,
    title: String,
    section: &'static str,
    detail: String,
    target: Target<'a>,
) -> Option<(i32, Entry<'a>)> {
    let m = fuzzy::score(query, &title)?;
    Some((
        m.score,
        Entry {
            target,
            title,
            section,
            detail,
            positions: m.positions,
        },
    ))
}

/// Teams' pages, saved views, favorites, and the team's projects and cycles.
fn places<'a>(app: &'a App, query: &str) -> Vec<Entry<'a>> {
    let store = &app.store;
    let mut found = Vec::new();

    for (index, team) in store.teams.iter().enumerate() {
        let mut pages = vec![
            ("Issues", TeamSection::Issues),
            ("Projects", TeamSection::Projects),
            ("Views", TeamSection::Views),
        ];
        if team.cycles_enabled {
            pages.insert(1, ("Cycles", TeamSection::Cycles));
        }
        for (page, section) in pages {
            found.extend(matched(
                query,
                format!("{} \u{203a} {page}", team.name),
                "Team",
                team.key.clone(),
                Target::Go(Nav::Team(index, section)),
            ));
        }
    }
    for (index, view) in store.custom_views.iter().enumerate() {
        found.extend(matched(
            query,
            view.name.clone(),
            "View",
            String::new(),
            Target::Go(Nav::View(index)),
        ));
    }
    for (index, fav) in store.favorites.iter().enumerate() {
        if fav.is_folder() {
            continue;
        }
        found.extend(matched(
            query,
            fav.label(),
            "Favorite",
            String::new(),
            Target::Favorite(index),
        ));
    }
    for project in &store.projects.items {
        found.extend(matched(
            query,
            project.name.clone(),
            "Project",
            project.state.clone().unwrap_or_default(),
            Target::Project(project),
        ));
    }
    for cycle in &store.cycles.items {
        let name = cycle
            .name
            .clone()
            .unwrap_or_else(|| format!("Cycle {}", cycle.number.unwrap_or(0.0)));
        found.extend(matched(
            query,
            name,
            "Cycle",
            String::new(),
            Target::Cycle(cycle),
        ));
    }

    let mut places = ranked(found.into_iter());
    places.truncate(MAX_PLACES);
    places
}

/// Issues matching by ID or title: every list already loaded, the open one,
/// and what Linear's search returned — each issue once.
fn issues<'a>(app: &'a App, query: &str) -> Vec<Entry<'a>> {
    let store = &app.store;
    let mut seen = HashSet::new();
    let candidates = store
        .current_issue
        .iter()
        .chain(store.issues.iter().flat_map(|rows| &rows.items))
        .chain(&app.view.palette.results)
        .filter(|issue| seen.insert(&issue.id));

    let found = candidates.filter_map(|issue| {
        let (score, entry) = matched(
            query,
            format!("{} {}", issue.identifier, issue.title),
            "Issue",
            issue
                .state
                .as_ref()
                .map(|s| s.name.clone())
                .unwrap_or_default(),
            Target::Issue(issue),
        )?;
        let exact = issue.identifier.eq_ignore_ascii_case(query);
        Some((score + if exact { EXACT_ID } else { 0 }, entry))
    });
    let mut issues = ranked(found);
    issues.truncate(MAX_ISSUES);
    issues
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
        app.palette_query_changed(Instant::now());
    }
}

fn move_by(app: &mut App, delta: isize) {
    let len = entries(app).len();
    App::nav_by(len, &mut app.view.palette.selected, delta);
}

/// A chosen row, detached from the app so the app can be changed.
enum Chosen {
    Command(&'static Binding),
    Go(Nav),
    Favorite(usize),
    Issue(Box<Issue>),
    Project(Box<Project>),
    Cycle(Box<Cycle>),
}

impl Target<'_> {
    fn chosen(&self) -> Chosen {
        match self {
            Target::Command(binding) => Chosen::Command(binding),
            Target::Go(nav) => Chosen::Go(*nav),
            Target::Favorite(index) => Chosen::Favorite(*index),
            Target::Issue(issue) => Chosen::Issue(Box::new((*issue).clone())),
            Target::Project(project) => Chosen::Project(Box::new((*project).clone())),
            Target::Cycle(cycle) => Chosen::Cycle(Box::new((*cycle).clone())),
        }
    }
}

fn chosen_at(app: &App, index: usize) -> Option<Chosen> {
    entries(app).get(index).map(|e| e.target.chosen())
}

fn run_selected(app: &mut App) {
    match chosen_at(app, app.view.palette.selected) {
        Some(chosen) => run(app, chosen),
        None => app.close_popup(),
    }
}

/// Close the palette and do what the row stands for.
fn run(app: &mut App, chosen: Chosen) {
    let issue = app.view.palette.issue.clone();
    app.close_popup();
    match chosen {
        Chosen::Command(binding) => {
            if issue.is_some() && app.focused_issue_id() != issue {
                app.set_status("The issue under the cursor changed \u{2014} nothing was done");
                return;
            }
            if let Some(command) = binding.command {
                app.remember_command(command.title);
            }
            (binding.action)(app);
            // A command that asks a follow-up question — which status, which
            // person — leads there as a page of the palette: Esc steps back.
            if app.view.popup != Popup::None {
                app.view.popup_from_palette = true;
            }
        }
        Chosen::Go(nav) => app.run_sidebar_action(SidebarAction::Go(nav)),
        Chosen::Favorite(index) => app.open_favorite_entry(index),
        Chosen::Issue(issue) => app.open_issue_from_list(&issue),
        Chosen::Project(project) => app.go_to_project(*project),
        Chosen::Cycle(cycle) => app.go_to_cycle(*cycle),
    }
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
    if let Some(chosen) = chosen_at(app, index) {
        run(app, chosen);
    }
}

/// The mouse wheel over the palette moves its cursor.
pub fn wheel(app: &mut App, delta: i16) {
    move_by(app, delta as isize);
}

#[cfg(test)]
mod tests;
