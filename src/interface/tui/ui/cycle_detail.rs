//! A cycle's page: its dates and progress over its issues, grouped.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::cycle_list::cycle_name;
use super::issue_list::{draw_list, draw_toolbar};
use super::widgets::{progress_bar, short_date};
use crate::interface::tui::app::App;

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(cycle) = app.nav.current_cycle.clone() else {
        return;
    };
    let th = app.theme;
    let chunks = Layout::vertical([
        Constraint::Length(3), // summary
        Constraint::Length(1), // toolbar
        Constraint::Length(1), // gap
        Constraint::Min(0),    // issues
    ])
    .split(area);

    let progress = cycle.progress.unwrap_or(0.0);
    let mut meta = vec![Span::raw(" ")];
    meta.extend(progress_bar(progress, 20, th.accent, &th));
    meta.push(Span::styled(
        format!(
            " {:.0}%   {} \u{2192} {}",
            progress * 100.0,
            short_date(cycle.starts_at.as_deref()),
            short_date(cycle.ends_at.as_deref())
        ),
        Style::default().fg(th.text_dim),
    ));
    let lines = vec![
        Line::from(vec![
            Span::styled(" \u{25d4} ", Style::default().fg(th.accent)),
            Span::styled(
                cycle_name(&cycle),
                Style::default().fg(th.text).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(meta),
    ];
    f.render_widget(Paragraph::new(lines), chunks[0]);

    let count = draw_list(f, app, chunks[3]);
    draw_toolbar(f, app, chunks[1], count);
}
