use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{App, FormField};
use crate::config::Theme;

/// Draw the modal issue-creation form.
pub fn draw(f: &mut Frame, app: &App) {
    let Some(form) = &app.new_issue else {
        return;
    };
    let th = &app.theme;

    let area = centered(70, 16, f.area());
    f.render_widget(Clear, area);

    let team = app
        .current_team()
        .map(|t| format!(" New issue in {} ", t.key))
        .unwrap_or_else(|| " New issue ".to_string());
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(th.accent))
        .title(team)
        .title_style(Style::default().fg(th.accent).add_modifier(Modifier::BOLD))
        .title_bottom(Line::from(Span::styled(
            " Tab: next field · Ctrl+Enter: create · Esc: cancel ",
            Style::default().fg(th.muted),
        )));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let rows = Layout::vertical([
        Constraint::Length(3), // title
        Constraint::Min(3),    // description
        Constraint::Length(1), // priority
    ])
    .split(inner);

    let title_spans = super::input_spans(&form.title, th);
    f.render_widget(
        Paragraph::new(Line::from(title_spans)).block(field_block(
            "Title",
            FormField::Title,
            form.field,
            th,
        )),
        rows[0],
    );

    f.render_widget(
        Paragraph::new(super::input_lines(&form.description, th))
            .wrap(Wrap { trim: false })
            .block(field_block(
                "Description",
                FormField::Description,
                form.field,
                th,
            )),
        rows[1],
    );

    let selected = form.field == FormField::Priority;
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " Priority: ",
                Style::default().fg(if selected { th.accent } else { th.text_dim }),
            ),
            Span::styled(
                form.priority.label(),
                Style::default().fg(form.priority.color(th)),
            ),
            Span::styled(
                if selected { "   (j/k to change)" } else { "" },
                Style::default().fg(th.muted),
            ),
        ])),
        rows[2],
    );
}

fn field_block(label: &str, field: FormField, focused: FormField, th: &Theme) -> Block<'static> {
    let active = field == focused;
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if active { th.accent } else { th.muted }))
        .title(format!(" {label} "))
        .title_style(Style::default().fg(if active { th.accent } else { th.text_dim }))
}

fn centered(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    let v = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(v[0])[0]
}
