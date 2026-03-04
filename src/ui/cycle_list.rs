use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};

use super::format_date;
use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Min(0),    // table
        Constraint::Length(1), // footer
    ])
    .split(area);

    draw_header(f, app, chunks[0]);
    draw_cycle_table(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let team_name = app
        .current_team()
        .map(|t| format!("Team: {} [{}]", t.name, t.key))
        .unwrap_or_else(|| "No team selected".to_string());

    let loading = if app.loading {
        format!(" {} Loading...", app.spinner_symbol())
    } else {
        String::new()
    };
    let count = format!(" ({} cycles)", app.cycles.len());

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {team_name}"),
            Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(count, Style::default().fg(th.muted)),
        Span::styled(&loading, Style::default().fg(th.warning)),
    ]));
    f.render_widget(header, area);
}

fn draw_cycle_table(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let rows: Vec<Row> = app
        .cycles
        .iter()
        .map(|cycle| {
            let name = cycle
                .name
                .clone()
                .unwrap_or_else(|| format!("Cycle #{}", cycle.number.unwrap_or(0.0)));
            let progress = cycle
                .progress
                .map(|p| format!("{:.0}%", p * 100.0))
                .unwrap_or_else(|| "-".to_string());
            let start = format_date(cycle.starts_at.as_deref());
            let end = format_date(cycle.ends_at.as_deref());

            Row::new(vec![
                Cell::from(name),
                Cell::from(progress),
                Cell::from(start).style(Style::default().fg(th.text_dim)),
                Cell::from(end).style(Style::default().fg(th.text_dim)),
            ])
        })
        .collect();

    let header = Row::new(vec!["Name", "Progress", "Start", "End"])
        .style(Style::default().fg(th.accent).add_modifier(Modifier::BOLD))
        .bottom_margin(0);

    let widths = [
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" Cycles "))
        .row_highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .fg(th.highlight_fg),
        )
        .highlight_symbol(" > ");

    let mut state = TableState::default();
    state.select(Some(app.selected_cycle_index));
    f.render_stateful_widget(table, area, &mut state);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let content = Line::from(vec![
        Span::styled(" j/k", Style::default().fg(th.accent)),
        Span::raw(":move "),
        Span::styled("Enter", Style::default().fg(th.accent)),
        Span::raw(":issues "),
        Span::styled("1-4", Style::default().fg(th.accent)),
        Span::raw(":tab "),
        Span::styled("t", Style::default().fg(th.accent)),
        Span::raw(":team "),
        Span::styled("q", Style::default().fg(th.accent)),
        Span::raw(":quit"),
    ]);
    f.render_widget(Paragraph::new(content), area);
}
