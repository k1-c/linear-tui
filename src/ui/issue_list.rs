use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
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

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
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

fn draw_header(f: &mut Frame, app: &mut App, area: Rect) {
    let th = &app.theme;
    let loading = if app.loading() {
        format!(" {} Loading...", app.spinner_symbol())
    } else {
        String::new()
    };

    let (label, count) = match app.tab {
        Tab::MyIssues => ("My Issues".to_string(), app.visible_my_issues().len()),
        _ => match &app.global_search {
            Some(term) => (format!("Search: \"{term}\""), app.visible_issues().len()),
            None => {
                let team_name = app
                    .current_team()
                    .map(|t| format!("Team: {} [{}]", t.name, t.key))
                    .unwrap_or_else(|| "No team selected".to_string());
                (team_name, app.visible_issues().len())
            }
        },
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

fn draw_issue_table(f: &mut Frame, app: &mut App, area: Rect) {
    let th = app.theme;
    let my_issues = app.tab == Tab::MyIssues;

    // Build owned rows first so the immutable borrow of `app` ends before we
    // reach for the mutable table state below.
    let (rows, selected_index, title): (Vec<Row>, usize, &str) = if my_issues {
        let rows = app
            .visible_my_issues()
            .into_iter()
            .map(|i| issue_row(i, &th))
            .collect();
        (rows, app.selected_my_issue_index, " My Issues ")
    } else {
        let rows = app
            .visible_issues()
            .into_iter()
            .map(|i| issue_row(i, &th))
            .collect();
        (rows, app.selected_issue_index, " Issues ")
    };

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

    app.list_viewport = area.height.saturating_sub(3); // borders + header row
    let state = if my_issues {
        &mut app.tables.my_issues
    } else {
        &mut app.tables.issues
    };
    state.select(Some(selected_index));
    f.render_stateful_widget(table, area, state);
}

fn draw_footer(f: &mut Frame, app: &mut App, area: Rect) {
    let th = &app.theme;
    let content = match app.input_mode {
        InputMode::Search => {
            let mut spans = vec![Span::styled(" /", Style::default().fg(th.warning))];
            spans.extend(super::input_spans(&app.search, th));
            let matches = if app.tab == Tab::MyIssues {
                app.visible_my_issues().len()
            } else {
                app.visible_issues().len()
            };
            spans.push(Span::styled(
                format!("   ({matches} matches · Ctrl+G searches all of Linear)"),
                Style::default().fg(th.muted),
            ));
            Line::from(spans)
        }
        InputMode::Comment | InputMode::NewIssue | InputMode::Normal => {
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
                    Span::styled("c", Style::default().fg(th.accent)),
                    Span::raw(":new "),
                    Span::styled("o", Style::default().fg(th.accent)),
                    Span::raw(":open "),
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
