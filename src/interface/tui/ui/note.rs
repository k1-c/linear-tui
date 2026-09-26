use ratatui::{
    Frame,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use super::widgets::{centered_rect, truncate};
use crate::interface::tui::app::{App, InputMode};

/// Draw the box a note for the agent is typed into.
pub fn draw(f: &mut Frame, app: &App) {
    let th = &app.theme;
    let area = f.area();
    let width = area.width.saturating_sub(4).min(76);
    let lines = app.view.note.value.split('\n').count() as u16;
    let height = lines.saturating_add(2).clamp(5, 14);
    let area = centered_rect(width, height, area);
    f.render_widget(Clear, area);

    let about = match &app.view.note_about {
        Some(subject) => format!(" Note on {} {} ", subject.identifier, subject.title),
        None => " Note on this view ".to_string(),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.accent))
        .title(Span::styled(
            truncate(&about, width.saturating_sub(4) as usize),
            Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Line::from(Span::styled(
            " Ctrl+Enter add \u{00b7} Enter newline \u{00b7} Esc cancel ",
            Style::default().fg(th.muted),
        )));
    f.render_widget(
        Paragraph::new(super::input_lines(&app.view.note, th))
            .wrap(Wrap { trim: false })
            .block(block),
        area,
    );
}

/// Draw the field an issue is renamed in, or — where there is no `$EDITOR`
/// — its description rewritten in.
pub fn draw_edit(f: &mut Frame, app: &App) {
    let th = &app.theme;
    let describing = app.view.input_mode == InputMode::Description;
    let input = if describing {
        &app.view.description
    } else {
        &app.view.title
    };
    let area = f.area();
    let width = area
        .width
        .saturating_sub(4)
        .min(if describing { 96 } else { 76 });
    let height = if describing {
        let lines = input.value.split('\n').count() as u16;
        let most = area.height.saturating_sub(4).max(8);
        lines.saturating_add(2).clamp(8, most)
    } else {
        3
    };
    let area = centered_rect(width, height, area);
    f.render_widget(Clear, area);

    let identifier = app
        .view
        .editing
        .as_ref()
        .and_then(|id| app.store.issue(id))
        .map_or(String::new(), |i| format!(" {}", i.identifier));
    let (title, keys) = if describing {
        (
            format!(" Description of{identifier} "),
            " Ctrl+Enter save \u{00b7} Enter newline \u{00b7} Esc cancel ",
        )
    } else {
        (
            format!(" Rename{identifier} "),
            " Enter save \u{00b7} Esc cancel ",
        )
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.accent))
        .title(Span::styled(
            truncate(&title, width.saturating_sub(4) as usize),
            Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Line::from(Span::styled(
            keys,
            Style::default().fg(th.muted),
        )));
    f.render_widget(
        Paragraph::new(super::input_lines(input, th))
            .wrap(Wrap { trim: false })
            .block(block),
        area,
    );
}
