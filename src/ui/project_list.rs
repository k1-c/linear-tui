use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Min(0),    // table
        Constraint::Length(1), // footer
    ])
    .split(area);

    draw_header(f, app, chunks[0]);
    draw_project_table(f, app, chunks[1]);
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
    let count = format!(" ({} projects)", app.projects.len());

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

fn project_state_color(state: Option<&str>) -> Color {
    match state {
        Some("started") => Color::Yellow,
        Some("planned") => Color::Blue,
        Some("completed") => Color::Green,
        Some("cancelled") | Some("canceled") => Color::DarkGray,
        Some("paused") => Color::Magenta,
        Some("backlog") => Color::DarkGray,
        _ => Color::White,
    }
}

fn draw_project_table(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let rows: Vec<Row> = app
        .projects
        .iter()
        .map(|project| {
            let state = project.state.as_deref().unwrap_or("-");
            let progress = project
                .progress
                .map(|p| format!("{:.0}%", p * 100.0))
                .unwrap_or_else(|| "-".to_string());
            let lead = project
                .lead
                .as_ref()
                .and_then(|u| u.display_name.as_deref().or(Some(u.name.as_str())))
                .unwrap_or("-");
            let target = project.target_date.as_deref().unwrap_or("-");

            Row::new(vec![
                Cell::from(project.name.clone()),
                Cell::from(state.to_string())
                    .style(Style::default().fg(project_state_color(Some(state)))),
                Cell::from(progress),
                Cell::from(lead.to_string()).style(Style::default().fg(th.text_dim)),
                Cell::from(target.to_string()).style(Style::default().fg(th.muted)),
            ])
        })
        .collect();

    let header = Row::new(vec!["Name", "Status", "Progress", "Lead", "Target"])
        .style(Style::default().fg(th.accent).add_modifier(Modifier::BOLD))
        .bottom_margin(0);

    let widths = [
        Constraint::Min(20),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(16),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" Projects "))
        .row_highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .fg(th.highlight_fg),
        )
        .highlight_symbol(" > ");

    let mut state = TableState::default();
    state.select(Some(app.selected_project_index));
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
