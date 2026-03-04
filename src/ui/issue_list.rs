use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::api::types::Issue;
use crate::app::{App, InputMode, Tab};
use crate::config::Theme;

fn state_color(state_type: Option<&crate::api::types::StateType>) -> Color {
    match state_type {
        Some(st) => st.color(),
        None => Color::White,
    }
}

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Min(0),    // table
        Constraint::Length(1), // footer
    ])
    .split(area);

    draw_header(f, app, chunks[0]);
    draw_issue_table(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let loading = if app.loading {
        format!(" {} Loading...", app.spinner_symbol())
    } else {
        String::new()
    };

    let (label, count) = match app.tab {
        Tab::MyIssues => ("My Issues".to_string(), app.my_issues.len()),
        _ => {
            let team_name = app
                .current_team()
                .map(|t| format!("Team: {} [{}]", t.name, t.key))
                .unwrap_or_else(|| "No team selected".to_string());
            (team_name, app.visible_issues().len())
        }
    };

    let issue_count = format!(" ({} issues)", count);
    let filter_info = if app.tab == Tab::Issues && app.filters.is_active() {
        format!("  [{}]", app.filters.summary())
    } else {
        String::new()
    };

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {label}"),
            Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(issue_count, Style::default().fg(th.muted)),
        Span::styled(filter_info, Style::default().fg(th.secondary)),
        Span::styled(&loading, Style::default().fg(th.warning)),
    ]));
    f.render_widget(header, area);
}

pub fn issue_row(issue: &Issue, theme: &Theme) -> Row<'static> {
    let state_name = issue
        .state
        .as_ref()
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "-".to_string());
    let state_type = issue.state.as_ref().and_then(|s| s.state_type.as_ref());
    let pri_label = issue
        .priority_label
        .clone()
        .unwrap_or_else(|| issue.priority.label().to_string());
    let assignee = issue
        .assignee
        .as_ref()
        .and_then(|a| a.display_name.clone().or_else(|| Some(a.name.clone())))
        .unwrap_or_else(|| "-".to_string());

    Row::new(vec![
        Cell::from(issue.identifier.clone()),
        Cell::from(issue.title.clone()),
        Cell::from(state_name).style(Style::default().fg(state_color(state_type))),
        Cell::from(pri_label).style(Style::default().fg(issue.priority.color(theme))),
        Cell::from(assignee).style(Style::default().fg(theme.text_dim)),
    ])
}

fn draw_issue_table(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let (issues_list, selected_index, title) = match app.tab {
        Tab::MyIssues => {
            let issues: Vec<&Issue> = app.my_issues.iter().collect();
            (issues, app.selected_my_issue_index, " My Issues ")
        }
        _ => {
            let issues = app.visible_issues();
            (issues, app.selected_issue_index, " Issues ")
        }
    };

    let rows: Vec<Row> = issues_list
        .iter()
        .map(|issue| issue_row(issue, th))
        .collect();

    let header = Row::new(vec!["ID", "Title", "Status", "Priority", "Assignee"])
        .style(Style::default().fg(th.accent).add_modifier(Modifier::BOLD))
        .bottom_margin(0);

    let widths = [
        Constraint::Length(10),
        Constraint::Min(20),
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(16),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .fg(th.highlight_fg),
        )
        .highlight_symbol(" > ");

    let mut state = TableState::default();
    state.select(Some(selected_index));
    f.render_stateful_widget(table, area, &mut state);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let content = match app.input_mode {
        InputMode::Search => Line::from(vec![
            Span::styled(" /", Style::default().fg(th.warning)),
            Span::raw(&app.search_query),
            Span::styled("_", Style::default().fg(th.muted)),
        ]),
        InputMode::Comment | InputMode::Normal => {
            if let Some(msg) = &app.status_message {
                Line::from(Span::styled(
                    format!(" {msg}"),
                    Style::default().fg(th.warning),
                ))
            } else {
                Line::from(vec![
                    Span::styled(" j/k", Style::default().fg(th.accent)),
                    Span::raw(":move "),
                    Span::styled("Enter", Style::default().fg(th.accent)),
                    Span::raw(":detail "),
                    Span::styled("/", Style::default().fg(th.accent)),
                    Span::raw(":search "),
                    Span::styled("t", Style::default().fg(th.accent)),
                    Span::raw(":team "),
                    Span::styled("f", Style::default().fg(th.accent)),
                    Span::raw(":filter "),
                    Span::styled("r", Style::default().fg(th.accent)),
                    Span::raw(":reload "),
                    Span::styled("?", Style::default().fg(th.accent)),
                    Span::raw(":help "),
                    Span::styled("q", Style::default().fg(th.accent)),
                    Span::raw(":quit"),
                ])
            }
        }
    };
    f.render_widget(Paragraph::new(content), area);
}
