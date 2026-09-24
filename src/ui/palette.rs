//! The command palette: a query line over the commands it matches.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::widgets::centered_rect;
use crate::app::App;
use crate::config::Theme;
use crate::palette::{self, Entry};

const WIDTH: u16 = 76;
/// Rows of entries shown at once.
const ROWS: u16 = 14;

pub fn draw(f: &mut Frame, app: &mut App) {
    let th = app.theme;
    let entries = palette::entries(app);
    let screen = f.area();
    let width = WIDTH.min(screen.width.saturating_sub(4));
    let rows = (entries.len().max(1) as u16).min(ROWS);
    // Border, query line, and the rule under it.
    let height = (rows + 4).min(screen.height);
    let area = centered_rect(width, height, screen);

    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.border))
        .title(Span::styled(
            " Command palette ",
            Style::default().fg(th.text).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Line::from(Span::styled(
            " \u{2191}\u{2193} move \u{00b7} Enter run \u{00b7} Esc close ",
            Style::default().fg(th.muted),
        )));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height < 3 {
        return;
    }

    let query = &app.view.palette.query;
    let mut line = vec![Span::styled("\u{203a} ", Style::default().fg(th.accent))];
    if query.is_empty() {
        line.extend(super::input_spans(query, &th));
        line.push(Span::styled(
            "Type a command\u{2026}",
            Style::default().fg(th.muted),
        ));
    } else {
        line.extend(super::input_spans(query, &th));
    }
    f.render_widget(
        Paragraph::new(Line::from(line)),
        Rect { height: 1, ..inner },
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            "\u{2500}".repeat(inner.width as usize),
            Style::default().fg(th.border),
        )),
        Rect {
            y: inner.y + 1,
            height: 1,
            ..inner
        },
    );

    let list = Rect {
        y: inner.y + 2,
        height: inner.height - 2,
        ..inner
    };
    let selected = app
        .view
        .palette
        .selected
        .min(entries.len().saturating_sub(1));
    app.view.palette.selected = selected;

    // Keep the cursor on screen.
    let height = list.height as usize;
    let mut offset = app.frame.popup_offset;
    if selected < offset {
        offset = selected;
    } else if height > 0 && selected >= offset + height {
        offset = selected + 1 - height;
    }
    offset = offset.min(entries.len().saturating_sub(height));
    app.frame.popup_offset = offset;
    app.frame.popup_area = list;

    if entries.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled(
                "  No matching commands",
                Style::default().fg(th.muted),
            )),
            list,
        );
        return;
    }
    let lines: Vec<Line> = entries
        .iter()
        .enumerate()
        .skip(offset)
        .take(height)
        .map(|(i, entry)| row(entry, i == selected, list.width as usize, &th))
        .collect();
    f.render_widget(Paragraph::new(lines), list);
}

/// One entry: the title with the matched letters picked out, then its
/// section and keys right-aligned.
fn row(entry: &Entry, selected: bool, width: usize, th: &Theme) -> Line<'static> {
    let base = if selected {
        Style::default().bg(th.selection_bg)
    } else {
        Style::default()
    };
    let trailing = if entry.keys.is_empty() {
        format!("{}  ", entry.section)
    } else {
        format!("{}  {}  ", entry.section, entry.keys)
    };
    let marker = if selected { "\u{258c} " } else { "  " };
    let room = width.saturating_sub(marker.width() + trailing.width() + 1);

    let mut spans = vec![Span::styled(marker, base.fg(th.accent))];
    let mut used = 0;
    for (i, c) in entry.title.chars().enumerate() {
        let w = c.width().unwrap_or(0);
        if used + w > room {
            spans.push(Span::styled("\u{2026}", base.fg(th.text_dim)));
            used += 1;
            break;
        }
        let style = if entry.positions.contains(&i) {
            base.fg(th.accent).add_modifier(Modifier::BOLD)
        } else {
            base.fg(th.text)
        };
        spans.push(Span::styled(c.to_string(), style));
        used += w;
    }
    let pad = width.saturating_sub(marker.width() + used + trailing.width());
    spans.push(Span::styled(" ".repeat(pad), base));
    spans.push(Span::styled(trailing, base.fg(th.muted)));
    Line::from(spans)
}
