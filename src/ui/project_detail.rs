use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table, TableState},
};

use super::issue_list::issue_row;
use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let Some(project) = &app.current_project else {
        return;
    };
    let th = &app.theme;

    let chunks = Layout::vertical([
        Constraint::Length(3), // project info
        Constraint::Min(0),    // issues table
        Constraint::Length(1), // footer
    ])
    .split(area);

    // Project info
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

    let meta = Paragraph::new(vec![Line::from(vec![
        Span::styled(" Status: ", Style::default().fg(th.text_dim)),
        Span::styled(state, Style::default().fg(th.warning)),
        Span::raw("    "),
        Span::styled("Progress: ", Style::default().fg(th.text_dim)),
        Span::styled(progress, Style::default().fg(th.success)),
        Span::raw("    "),
        Span::styled("Lead: ", Style::default().fg(th.text_dim)),
        Span::styled(lead, Style::default().fg(th.accent)),
    ])])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", project.name))
            .title_style(Style::default().fg(th.accent).add_modifier(Modifier::BOLD)),
    );
    f.render_widget(meta, chunks[0]);

    // Issues table
    let loading = if app.loading {
        format!(" ({} Loading...)", app.spinner_symbol())
    } else {
        String::new()
    };
    let rows: Vec<Row> = app
        .project_issues
        .iter()
        .map(|issue| issue_row(issue, th))
        .collect();

    let header = Row::new(vec!["ID", "Title", "Status", "Priority", "Assignee"])
        .style(Style::default().fg(th.accent).add_modifier(Modifier::BOLD));

    let widths = [
        Constraint::Length(10),
        Constraint::Min(20),
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(16),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Issues ({}){}",
            app.project_issues.len(),
            loading
        )))
        .row_highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .fg(th.highlight_fg),
        )
        .highlight_symbol(" > ");

    let mut table_state = TableState::default();
    table_state.select(Some(app.selected_project_issue_index));
    f.render_stateful_widget(table, chunks[1], &mut table_state);

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" Esc/q", Style::default().fg(th.accent)),
        Span::raw(":back "),
        Span::styled("j/k", Style::default().fg(th.accent)),
        Span::raw(":move "),
        Span::styled("Enter", Style::default().fg(th.accent)),
        Span::raw(":detail "),
    ]));
    f.render_widget(footer, chunks[2]);
}
