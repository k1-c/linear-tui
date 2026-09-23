//! The saved-view index, laid out like Linear's Views page: personal views,
//! then workspace views, each with its description and owner.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use super::widgets::{truncate, user_name};
use crate::api::types::hex_color;
use crate::app::App;

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let th = app.theme;
    let area = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    app.list_area = area;
    app.list_viewport = area.height;

    if app.custom_views.is_empty() {
        app.row_targets.clear();
        let message = if app.views_loaded {
            "No saved views yet \u{2014} create one on linear.app and press Ctrl+R"
        } else {
            "Loading views\u{2026}"
        };
        f.render_widget(
            Paragraph::new(Span::styled(message, Style::default().fg(th.muted)))
                .alignment(ratatui::layout::Alignment::Center),
            Rect {
                y: area.y + area.height / 3,
                height: 1,
                ..area
            },
        );
        return;
    }

    let width = area.width as usize;
    // Build (line, target) pairs, with a caption above each scope.
    let mut rows: Vec<(Line<'static>, Option<usize>)> = Vec::new();
    let mut last_scope = None;
    for (index, view) in app.custom_views.iter().enumerate() {
        if last_scope != Some(view.shared) {
            last_scope = Some(view.shared);
            let (title, note) = if view.shared {
                ("Workspace", "Shared with everyone")
            } else {
                ("Personal views", "Only visible to you")
            };
            let caption = vec![
                Span::styled(
                    format!(" {title}"),
                    Style::default().fg(th.text).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" \u{00b7} {note}"), Style::default().fg(th.muted)),
            ];
            let used: usize = caption.iter().map(|s| s.width()).sum();
            let mut caption = caption;
            caption.push(Span::raw(" ".repeat(width.saturating_sub(used))));
            for span in &mut caption {
                span.style = span.style.bg(th.surface);
            }
            if !rows.is_empty() {
                rows.push((Line::from(""), None));
            }
            rows.push((Line::from(caption), None));
        }

        let selected = index == app.selected_view_index;
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
        let team = view
            .team
            .as_ref()
            .map(|t| format!("{}  ", t.key))
            .unwrap_or_default();
        let right = format!("{team}{} ", truncate(&owner, 18));
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
            Span::styled(" \u{2261} ", Style::default().fg(color)),
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
        rows.push((Line::from(spans), Some(index)));
    }

    // Scroll to keep the cursor in view.
    let height = area.height as usize;
    let sel_row = rows
        .iter()
        .position(|(_, t)| *t == Some(app.selected_view_index))
        .unwrap_or(0);
    let mut offset = app.tables.views.offset();
    if sel_row < offset {
        offset = sel_row.saturating_sub(1);
    } else if height > 0 && sel_row >= offset + height {
        offset = sel_row + 1 - height;
    }
    offset = offset.min(rows.len().saturating_sub(height));
    *app.tables.views.offset_mut() = offset;

    let visible: Vec<_> = rows.into_iter().skip(offset).take(height).collect();
    app.row_targets = visible.iter().map(|(_, t)| *t).collect();
    f.render_widget(
        Paragraph::new(visible.into_iter().map(|(l, _)| l).collect::<Vec<_>>()),
        area,
    );
}
