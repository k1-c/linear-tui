//! A Views page, laid out like Linear's: Issues / Projects tabs, then personal
//! views above shared ones, each with its description and owner. The
//! workspace page lists views that belong to no team; a team's page, its own.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use super::widgets::{truncate, user_name};
use crate::interface::tui::app::{App, Chip, Nav, TeamSection, ViewKind};
use crate::interface::tui::look::hex_color;

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let th = app.theme;
    draw_tabs(f, app, Rect { height: 1, ..area });
    let area = Rect {
        y: area.y + 2,
        height: area.height.saturating_sub(2),
        ..area
    };
    app.frame.list_area = area;
    app.frame.list_viewport = area.height;

    let listed = app.listed_views();
    if listed.is_empty() {
        app.frame.row_targets.clear();
        let message = if !app.store.views_loaded {
            "Loading views\u{2026}".to_string()
        } else {
            let (this, other) = match app.view.view_kind {
                ViewKind::Issues => ("issue", "project"),
                ViewKind::Projects => ("project", "issue"),
            };
            format!("No {this} views here \u{2014} Shift+Tab shows {other} views")
        };
        f.render_widget(
            Paragraph::new(Span::styled(message, Style::default().fg(th.muted)))
                .alignment(ratatui::layout::Alignment::Center),
            super::widgets::message_row(area),
        );
        return;
    }

    let shared_caption = match app.nav.dest {
        Nav::Team(index, TeamSection::Views) => app
            .store
            .teams
            .get(index)
            .map(|t| (t.name.clone(), "Shared with the team"))
            .unwrap_or_else(|| ("Team".into(), "Shared with the team")),
        _ => ("Workspace".into(), "Shared with everyone"),
    };
    let icon = match app.view.view_kind {
        ViewKind::Issues => "\u{2261}",
        ViewKind::Projects => "\u{25a3}",
    };

    let width = area.width as usize;
    // (line, position in `listed`) pairs, with a caption above each scope.
    let mut rows: Vec<(Line<'static>, Option<usize>)> = Vec::new();
    let mut last_scope = None;
    for (position, &index) in listed.iter().enumerate() {
        let view = &app.store.custom_views[index];
        if last_scope != Some(view.shared) {
            last_scope = Some(view.shared);
            let (title, note) = if view.shared {
                (shared_caption.0.clone(), shared_caption.1)
            } else {
                ("Personal views".to_string(), "Only visible to you")
            };
            let mut caption = vec![
                Span::styled(
                    format!(" {title}"),
                    Style::default().fg(th.text).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" \u{00b7} {note}"), Style::default().fg(th.muted)),
            ];
            let used: usize = caption.iter().map(|s| s.width()).sum();
            caption.push(Span::raw(" ".repeat(width.saturating_sub(used))));
            for span in &mut caption {
                span.style = span.style.bg(th.surface);
            }
            if !rows.is_empty() {
                rows.push((Line::from(""), None));
            }
            rows.push((Line::from(caption), None));
        }

        let selected = position == app.view.selected_view_index;
        let color = view
            .color
            .as_deref()
            .and_then(hex_color)
            .unwrap_or(th.accent);
        let owner = view
            .owner
            .as_ref()
            .map(|u| user_name(u).to_string())
            .unwrap_or_default();
        let right = format!("{} ", truncate(&owner, 18));
        let name = truncate(&view.name, width.saturating_sub(right.width() + 6));
        let desc_room = width.saturating_sub(6 + name.width() + right.width() + 3);
        let desc = view
            .description
            .as_deref()
            .filter(|d| !d.is_empty())
            .map(|d| format!("  {}", truncate(d, desc_room)))
            .unwrap_or_default();
        let used = 4 + name.width() + desc.width() + right.width();

        let mut spans = vec![
            if selected {
                Span::styled("\u{258c}", Style::default().fg(th.accent))
            } else {
                Span::raw(" ")
            },
            Span::styled(format!(" {icon} "), Style::default().fg(color)),
            Span::styled(
                name,
                Style::default().fg(th.text).add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Span::styled(desc, Style::default().fg(th.muted)),
            Span::raw(" ".repeat(width.saturating_sub(used))),
            Span::styled(right, Style::default().fg(th.text_dim)),
        ];
        if selected {
            for span in &mut spans {
                span.style = span.style.bg(th.selection_bg);
            }
        }
        rows.push((Line::from(spans), Some(position)));
    }

    // Scroll to keep the cursor in view.
    let height = area.height as usize;
    let sel_row = rows
        .iter()
        .position(|(_, t)| *t == Some(app.view.selected_view_index))
        .unwrap_or(0);
    let mut offset = app.frame.offsets.views;
    if sel_row < offset {
        offset = sel_row.saturating_sub(1);
    } else if height > 0 && sel_row >= offset + height {
        offset = sel_row + 1 - height;
    }
    offset = offset.min(rows.len().saturating_sub(height));
    app.frame.offsets.views = offset;

    let visible: Vec<_> = rows.into_iter().skip(offset).take(height).collect();
    app.frame.row_targets = visible.iter().map(|(_, t)| *t).collect();
    f.render_widget(
        Paragraph::new(visible.into_iter().map(|(l, _)| l).collect::<Vec<_>>()),
        area,
    );
}

/// The Issues / Projects tabs, recorded as clickable chips.
fn draw_tabs(f: &mut Frame, app: &mut App, area: Rect) {
    let th = app.theme;
    let mut spans = vec![Span::raw(" ")];
    let mut x = area.x + 1;
    for kind in [ViewKind::Issues, ViewKind::Projects] {
        let label = format!(" {} ", kind.label());
        let width = label.width() as u16;
        let style = if kind == app.view.view_kind {
            Style::default()
                .fg(th.text)
                .bg(th.selection_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(th.muted)
        };
        app.frame.chip_areas.push((
            Rect {
                x,
                y: area.y,
                width,
                height: 1,
            },
            Chip::ViewKind(kind),
        ));
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
        x += width + 1;
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
