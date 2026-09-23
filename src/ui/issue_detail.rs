use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{App, InputMode};
use crate::config::Theme;

/// Height a paragraph occupies once wrapped to `width`.
fn wrapped_height(lines: &[Line], width: u16) -> u16 {
    if width == 0 {
        return lines.len() as u16;
    }
    lines
        .iter()
        .map(|line| {
            let w = line.width() as u16;
            w.div_ceil(width).max(1)
        })
        .fold(0u16, |acc, h| acc.saturating_add(h))
}

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let th = app.theme;
    let comment_mode = app.input_mode == InputMode::Comment;

    // Build every owned piece up front so the borrow of `app.current_issue`
    // ends before we write back the measured scroll extents.
    let Some((title, meta_lines, body_lines)) = build(app, &th) else {
        return;
    };

    let comment_height = if comment_mode {
        let entered = app.comment.value.lines().count().max(1) as u16;
        entered.saturating_add(2).clamp(3, 10)
    } else {
        0
    };

    let chunks = Layout::vertical([
        Constraint::Length(5),              // metadata
        Constraint::Min(0),                 // body + comments
        Constraint::Length(comment_height), // comment editor
        Constraint::Length(1),              // footer
    ])
    .split(area);

    let meta = Paragraph::new(meta_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_style(Style::default().fg(th.accent).add_modifier(Modifier::BOLD)),
    );
    f.render_widget(meta, chunks[0]);

    // Measure the wrapped body so scrolling can clamp to real content.
    let body_area = chunks[1];
    let inner_width = body_area.width.saturating_sub(2);
    app.detail_lines = wrapped_height(&body_lines, inner_width);
    app.detail_viewport = body_area.height.saturating_sub(2);
    app.detail_scroll = app
        .detail_scroll
        .min(app.detail_lines.saturating_sub(app.detail_viewport));

    let scrollbar = scroll_hint(app.detail_scroll, app.detail_lines, app.detail_viewport);
    let body = Paragraph::new(body_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title_bottom(Line::from(Span::styled(
                    scrollbar,
                    Style::default().fg(th.muted),
                ))),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));
    f.render_widget(body, body_area);

    if comment_mode {
        let mut spans = vec![];
        spans.extend(super::input_spans(&app.comment, &th));
        let editor = Paragraph::new(Line::from(spans))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(th.warning))
                    .title(" New comment — Ctrl+Enter to send, Esc to cancel "),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(editor, chunks[2]);
    }

    let footer = Line::from(vec![
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
        Span::styled("i", Style::default().fg(th.accent)),
        Span::raw(":assign-me "),
        Span::styled("m", Style::default().fg(th.accent)),
        Span::raw(":comment "),
        Span::styled("o", Style::default().fg(th.accent)),
        Span::raw(":open "),
        Span::styled("?", Style::default().fg(th.accent)),
        Span::raw(":help"),
    ]);
    f.render_widget(Paragraph::new(footer), chunks[3]);
}

/// A compact "42% · 12/280" position readout for the body pane.
fn scroll_hint(scroll: u16, total: u16, viewport: u16) -> String {
    if total <= viewport {
        return String::new();
    }
    let max = total.saturating_sub(viewport);
    let pct = if max == 0 {
        100
    } else {
        (scroll as u32 * 100 / max as u32) as u16
    };
    format!(" {pct}% · {}/{total} ", scroll + viewport.min(total))
}

type DetailParts = (String, Vec<Line<'static>>, Vec<Line<'static>>);

fn build(app: &App, th: &Theme) -> Option<DetailParts> {
    let issue = app.current_issue.as_ref()?;

    let state_name = issue
        .state
        .as_ref()
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "-".into());
    let pri = issue
        .priority_label
        .clone()
        .unwrap_or_else(|| issue.priority.label().to_string());
    let assignee = issue
        .assignee
        .as_ref()
        .and_then(|a| a.display_name.clone().or_else(|| Some(a.name.clone())))
        .unwrap_or_else(|| "-".into());
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
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "-".into());
    let cycle = issue
        .cycle
        .as_ref()
        .map(|c| {
            c.name
                .clone()
                .unwrap_or_else(|| format!("#{}", c.number.unwrap_or(0.0)))
        })
        .unwrap_or_else(|| "-".into());
    let updated = super::relative_time(issue.updated_at.as_deref());
    let created = super::relative_time(issue.created_at.as_deref());

    let meta_lines = vec![
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
        Line::from(vec![
            Span::styled(" Created: ", Style::default().fg(th.text_dim)),
            Span::styled(created, Style::default().fg(th.muted)),
            Span::raw("    "),
            Span::styled("Updated: ", Style::default().fg(th.text_dim)),
            Span::styled(updated, Style::default().fg(th.muted)),
        ]),
    ];

    let mut body = Vec::new();
    body.push(Line::from(Span::styled(
        "Description",
        Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
    )));
    match issue
        .description
        .as_deref()
        .filter(|d| !d.trim().is_empty())
    {
        Some(desc) => body.extend(desc.lines().map(|l| markdown_line(l, th))),
        None => body.push(Line::from(Span::styled(
            "(no description)",
            Style::default().fg(th.muted),
        ))),
    }
    body.push(Line::from(""));

    match &issue.comments {
        Some(comments) => {
            body.push(Line::from(Span::styled(
                format!("Comments ({})", comments.nodes.len()),
                Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
            )));
            body.push(Line::from(""));

            for comment in &comments.nodes {
                let author = comment
                    .user
                    .as_ref()
                    .and_then(|u| u.display_name.clone().or_else(|| Some(u.name.clone())))
                    .unwrap_or_else(|| "Unknown".into());
                body.push(Line::from(vec![
                    Span::styled(
                        format!("@{author}"),
                        Style::default().fg(th.success).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" · {}", super::relative_time(comment.created_at.as_deref())),
                        Style::default().fg(th.muted),
                    ),
                ]));
                for line in comment.body.lines() {
                    body.push(Line::from(format!("  {line}")));
                }
                body.push(Line::from(""));
            }
        }
        None => body.push(Line::from(Span::styled(
            "Loading comments…",
            Style::default().fg(th.muted),
        ))),
    }

    let title = format!(" {} : {} ", issue.identifier, issue.title);
    Some((title, meta_lines, body))
}

/// Light-touch Markdown styling — headings, bullets, and fenced code markers.
fn markdown_line(line: &str, th: &Theme) -> Line<'static> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
        ))
    } else if trimmed.starts_with("```") {
        Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(th.muted),
        ))
    } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        let indent = line.len() - trimmed.len();
        Line::from(vec![
            Span::raw(" ".repeat(indent)),
            Span::styled("• ", Style::default().fg(th.secondary)),
            Span::raw(trimmed[2..].to_string()),
        ])
    } else if trimmed.starts_with("> ") {
        Line::from(Span::styled(
            line.to_string(),
            Style::default()
                .fg(th.text_dim)
                .add_modifier(Modifier::ITALIC),
        ))
    } else {
        Line::from(line.to_string())
    }
}
