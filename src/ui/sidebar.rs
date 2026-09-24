//! The navigation sidebar: My Issues and Views, the user's Favorites, and the
//! current team — shown as a switcher — with its pages.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use super::widgets::truncate;
use crate::app::{App, Nav, SidebarAction, SidebarRow, TeamSection, Tone};

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let th = app.theme;
    app.frame.sidebar_rows = app.sidebar_layout();
    app.frame.sidebar_area = area;

    // The pane is separated from content by a single rule on its right edge.
    let inner = Rect {
        width: area.width.saturating_sub(1),
        ..area
    };
    for y in area.y..area.y + area.height {
        f.buffer_mut()[(area.x + area.width.saturating_sub(1), y)]
            .set_symbol("\u{2502}")
            .set_style(Style::default().fg(th.border));
    }

    // Title row.
    let title = Line::from(vec![
        Span::styled(" \u{25c6} ", Style::default().fg(th.accent)),
        Span::styled(
            "Linear",
            Style::default().fg(th.text).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(title), Rect { height: 1, ..inner });
    let list = Rect {
        y: inner.y + 2,
        height: inner.height.saturating_sub(2),
        ..inner
    };
    app.frame.sidebar_area = list;

    // Keep the cursor on screen.
    let height = list.height as usize;
    if app.sidebar_index < app.frame.sidebar_offset {
        app.frame.sidebar_offset = app.sidebar_index;
    } else if height > 0 && app.sidebar_index >= app.frame.sidebar_offset + height {
        app.frame.sidebar_offset = app.sidebar_index + 1 - height;
    }
    app.frame.sidebar_offset = app
        .frame
        .sidebar_offset
        .min(app.frame.sidebar_rows.len().saturating_sub(height));

    let width = list.width as usize;
    // A view that is not a favorite has no row of its own; light up Views.
    let exact = app
        .frame
        .sidebar_rows
        .iter()
        .any(|r| matches!(r, SidebarRow::Item(i) if i.nav() == Some(app.nav)));
    // A team's view lights its team's Views row; any other, the workspace's.
    let fallback = match app.nav {
        Nav::View(i) if !exact => Some(
            match app.store.custom_views.get(i).and_then(|v| v.team.as_ref()) {
                Some(team) if app.current_team().is_some_and(|t| t.id == team.id) => {
                    Nav::Team(app.selected_team_index, TeamSection::Views)
                }
                _ => Nav::Views,
            },
        ),
        _ => None,
    };
    let lines: Vec<Line> = app
        .frame
        .sidebar_rows
        .iter()
        .enumerate()
        .skip(app.frame.sidebar_offset)
        .take(height)
        .map(|(index, row)| match row {
            SidebarRow::Gap => Line::from(""),
            SidebarRow::Header(text) => Line::from(Span::styled(
                format!(" {text}"),
                Style::default().fg(th.muted).add_modifier(Modifier::BOLD),
            )),
            SidebarRow::Item(item) => {
                let active =
                    item.nav() == Some(app.nav) || (item.nav().is_some() && item.nav() == fallback);
                let cursor = app.sidebar_focus && index == app.sidebar_index;
                let bg = if cursor {
                    Some(th.selection_bg)
                } else if active {
                    Some(th.surface)
                } else {
                    None
                };

                let indent = "  ".repeat(item.depth as usize);
                let chevron = match item.expanded {
                    Some(true) => "\u{25be} ",
                    Some(false) => "\u{25b8} ",
                    None => "  ",
                };
                let (text, icon_default) = match item.tone {
                    Tone::Subtle => (th.muted, th.muted),
                    Tone::Normal => (th.text_dim, th.text_dim),
                    Tone::Strong => (th.text, th.text_dim),
                };
                let icon_style = Style::default().fg(item.color.unwrap_or(icon_default));
                let mut label_style =
                    Style::default().fg(if active || cursor { th.text } else { text });
                if active || cursor || item.action == SidebarAction::SwitchTeam {
                    label_style = label_style.add_modifier(Modifier::BOLD);
                }
                let trailing = item
                    .trailing
                    .as_ref()
                    .map(|t| format!("{t} "))
                    .unwrap_or_default();

                let lead = format!(" {indent}{chevron}");
                // The icon and the space after it; some icons take two cells.
                let icon_width = item.icon.width() + 1;
                let fixed = lead.width() + icon_width + trailing.width();
                let label = truncate(&item.label, width.saturating_sub(fixed));
                let pad = width.saturating_sub(fixed + label.width());

                let mut spans = vec![
                    Span::styled(lead, Style::default().fg(th.muted)),
                    Span::styled(format!("{} ", item.icon), icon_style),
                    Span::styled(label, label_style),
                    Span::raw(" ".repeat(pad)),
                    Span::styled(trailing, Style::default().fg(th.muted)),
                ];
                if let Some(bg) = bg {
                    for span in &mut spans {
                        span.style = span.style.bg(bg);
                    }
                }
                Line::from(spans)
            }
        })
        .collect();

    f.render_widget(Paragraph::new(lines), list);
}
