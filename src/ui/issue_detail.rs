use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{App, InputMode};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let Some(issue) = &app.current_issue else {
        return;
    };
    let th = &app.theme;

    let chunks = Layout::vertical([
        Constraint::Length(4), // metadata
        Constraint::Min(0),    // body + comments
        Constraint::Length(1), // footer
    ])
    .split(area);

    // Metadata
    let state_name = issue.state.as_ref().map(|s| s.name.as_str()).unwrap_or("-");
    let pri = issue
        .priority_label
        .as_deref()
        .unwrap_or_else(|| issue.priority.label());
    let assignee = issue
        .assignee
        .as_ref()
        .and_then(|a| a.display_name.as_deref().or(Some(a.name.as_str())))
        .unwrap_or("-");
    let labels = issue
        .labels
        .as_ref()
        .map(|c| {
            c.nodes
                .iter()
                .map(|l| format!("[{}]", l.name))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let project = issue
        .project
        .as_ref()
        .map(|p| p.name.as_str())
        .unwrap_or("-");
    let cycle_display = issue.cycle.as_ref().map(|c| {
        c.name
            .clone()
            .unwrap_or_else(|| format!("#{}", c.number.unwrap_or(0.0)))
    });
    let cycle = cycle_display.as_deref().unwrap_or("-");

    let meta = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" Status: ", Style::default().fg(th.text_dim)),
            Span::styled(state_name, Style::default().fg(th.warning)),
            Span::raw("    "),
            Span::styled("Priority: ", Style::default().fg(th.text_dim)),
            Span::styled(pri, Style::default().fg(issue.priority.color(th))),
        ]),
        Line::from(vec![
            Span::styled(" Assignee: ", Style::default().fg(th.text_dim)),
            Span::styled(assignee, Style::default().fg(th.accent)),
            Span::raw("    "),
            Span::styled("Labels: ", Style::default().fg(th.text_dim)),
            Span::raw(labels),
        ]),
        Line::from(vec![
            Span::styled(" Project: ", Style::default().fg(th.text_dim)),
            Span::raw(project),
            Span::raw("    "),
            Span::styled("Cycle: ", Style::default().fg(th.text_dim)),
            Span::raw(cycle),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} : {} ", issue.identifier, issue.title))
            .title_style(Style::default().fg(th.accent).add_modifier(Modifier::BOLD)),
    );
    f.render_widget(meta, chunks[0]);

    // Description + Comments
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::styled(
        "Description",
        Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
    )));
    if let Some(desc) = &issue.description {
        for line in desc.lines() {
            lines.push(Line::from(line.to_string()));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "(no description)",
            Style::default().fg(th.muted),
        )));
    }

    lines.push(Line::from(""));

    if let Some(comments) = &issue.comments {
        lines.push(Line::from(Span::styled(
            format!("Comments ({})", comments.nodes.len()),
            Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        for comment in &comments.nodes {
            let author = comment
                .user
                .as_ref()
                .and_then(|u| u.display_name.as_deref().or(Some(u.name.as_str())))
                .unwrap_or("Unknown");
            let time = comment.created_at.as_deref().unwrap_or("");

            lines.push(Line::from(vec![
                Span::styled(
                    format!("@{author}"),
                    Style::default().fg(th.success).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" ({time})"), Style::default().fg(th.muted)),
            ]));
            for line in comment.body.lines() {
                lines.push(Line::from(format!("  {line}")));
            }
            lines.push(Line::from(""));
        }
    }

    let body = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));
    f.render_widget(body, chunks[1]);

    // Footer
    let footer_content = if app.input_mode == InputMode::Comment {
        Line::from(vec![
            Span::styled(" Comment: ", Style::default().fg(th.warning)),
            Span::raw(&app.comment_input),
            Span::styled("_", Style::default().fg(th.muted)),
            Span::styled(
                "  (Ctrl+Enter to send, Esc to cancel)",
                Style::default().fg(th.text_dim),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(" Esc/q", Style::default().fg(th.accent)),
            Span::raw(":back "),
            Span::styled("j/k", Style::default().fg(th.accent)),
            Span::raw(":scroll "),
            Span::styled("s", Style::default().fg(th.accent)),
            Span::raw(":status "),
            Span::styled("p", Style::default().fg(th.accent)),
            Span::raw(":priority "),
            Span::styled("a", Style::default().fg(th.accent)),
            Span::raw(":assign "),
            Span::styled("c", Style::default().fg(th.accent)),
            Span::raw(":comment"),
        ])
    };
    f.render_widget(Paragraph::new(footer_content), chunks[2]);
}
