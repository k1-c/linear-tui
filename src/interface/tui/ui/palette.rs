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
use crate::config::Theme;
use crate::interface::tui::app::App;
use crate::interface::tui::palette::{self, Entry};

const WIDTH: u16 = 76;
/// Rows of entries shown at once.
const ROWS: u16 = 14;

pub fn draw(f: &mut Frame, app: &mut App) {
    let th = app.theme;
    let entries = palette::entries(app);
    let searching = app.view.palette.searching;
    let screen = f.area();
    let width = WIDTH.min(screen.width.saturating_sub(4));
    // A trailing line says a workspace search is still on its way.
    let shown = entries.len() + usize::from(searching);
    let rows = (shown.max(1) as u16).min(ROWS);
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

    // Keep the cursor on screen.
    let height = list.height as usize;
    let mut offset = app.frame.popup_offset;
    if selected < offset {
        offset = selected;
    } else if height > 0 && selected >= offset + height {
        offset = selected + 1 - height;
    }
    offset = offset.min(shown.saturating_sub(height));

    let muted = |text: String| Line::from(Span::styled(text, Style::default().fg(th.muted)));
    let mut lines: Vec<Line> = entries
        .iter()
        .enumerate()
        .skip(offset)
        .take(height)
        .map(|(i, entry)| row(entry, i == selected, list.width as usize, &th))
        .collect();
    let count = entries.len();
    drop(entries);
    if searching && lines.len() < height {
        lines.push(muted(format!(
            "  {} Searching Linear\u{2026}",
            app.spinner_symbol()
        )));
    } else if count == 0 {
        lines.push(muted("  No matches".to_string()));
    }
    f.render_widget(Paragraph::new(lines), list);

    app.view.palette.selected = selected;
    app.frame.popup_offset = offset;
    app.frame.popup_area = list;
}

/// One entry: the title with the matched letters picked out, then its
/// section and keys right-aligned.
fn row(entry: &Entry, selected: bool, width: usize, th: &Theme) -> Line<'static> {
    let base = if selected {
        Style::default().bg(th.selection_bg)
    } else {
        Style::default()
    };
    let trailing = if entry.detail.is_empty() {
        format!("{}  ", entry.section)
    } else {
        format!("{}  {}  ", entry.section, entry.detail)
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
