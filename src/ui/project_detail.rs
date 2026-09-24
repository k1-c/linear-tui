//! A project's page: its summary card over its issues, grouped like any list.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::issue_list::{draw_list, draw_toolbar};
use super::project_list::{health, project_state_color};
use super::widgets::{person, progress_bar, short_date, truncate};
use crate::api::types::hex_color;
use crate::app::App;

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(project) = app.current_project.clone() else {
        return;
    };
    let th = app.theme;
    let chunks = Layout::vertical([
        Constraint::Length(4), // summary
        Constraint::Length(1), // toolbar
        Constraint::Length(1), // gap
        Constraint::Min(0),    // issues
    ])
    .split(area);

    let color = project
        .color
        .as_deref()
        .and_then(hex_color)
        .unwrap_or_else(|| project_state_color(project.state.as_deref(), &th));
    let width = chunks[0].width as usize;
    let progress = project.progress.unwrap_or(0.0);

    let mut meta = vec![
        Span::styled(
            format!(" {} ", project.state.as_deref().unwrap_or("-")),
            Style::default().fg(project_state_color(project.state.as_deref(), &th)),
        ),
        Span::styled("\u{00b7} ", Style::default().fg(th.muted)),
    ];
    meta.extend(progress_bar(progress, 16, color, &th));
    meta.push(Span::styled(
        format!(" {:.0}%  \u{00b7}  ", progress * 100.0),
        Style::default().fg(th.text_dim),
    ));
    let h = health(&project, &th);
    if !h.content.is_empty() {
        meta.push(h);
        meta.push(Span::styled("  \u{00b7}  ", Style::default().fg(th.muted)));
    }
    meta.push(Span::styled("Lead ", Style::default().fg(th.muted)));
    meta.extend(person(project.lead.as_ref(), &th));
    meta.push(Span::styled(
        format!(
            "  \u{00b7}  {} \u{2192} {}",
            short_date(project.start_date.as_deref()),
            short_date(project.target_date.as_deref())
        ),
        Style::default().fg(th.muted),
    ));

    let lines = vec![
        Line::from(vec![
            Span::styled(" \u{25a3} ", Style::default().fg(color)),
            Span::styled(
                project.name.clone(),
                Style::default().fg(th.text).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(meta),
        Line::from(Span::styled(
            format!(
                " {}",
                truncate(
                    project.description.as_deref().unwrap_or(""),
                    width.saturating_sub(2)
                )
            ),
            Style::default().fg(th.text_dim),
        )),
    ];
    f.render_widget(Paragraph::new(lines), chunks[0]);

    draw_toolbar(f, app, chunks[1]);
    draw_list(f, app, chunks[3]);
}
