//! The issue detail view, laid out like Linear's: the title, description,
//! sub-issues, and comment threads in a scrolling main column, and the issue's
//! properties — status, priority, assignee, creator, estimate, due date, cycle,
//! labels, project — in a panel down the right.
//!
//! On a narrow terminal the panel folds into a compact block under the title,
//! so nothing is lost, only rearranged.

use std::collections::HashMap;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use unicode_width::UnicodeWidthStr;

use super::markdown::{self, wrap};
use super::widgets::{
    initials, label_chip, person, person_color, priority_glyph, short_date, state_color,
    state_glyph, truncate, user_name,
};
use crate::api::types::{Comment, Issue, StateType, hex_color};
use crate::app::{App, InputMode};
use crate::config::Theme;

/// Width of the properties panel when there is room for one.
const PANEL_WIDTH: u16 = 34;
/// Narrowest content pane that still gets a separate properties panel.
const PANEL_MIN_TOTAL: u16 = 96;

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let th = app.theme;
    let Some(issue) = app.current_issue.clone() else {
        return;
    };
    let comment_mode = app.input_mode == InputMode::Comment;

    let comment_height = if comment_mode {
        let entered = app.comment.value.split('\n').count() as u16;
        entered.saturating_add(2).clamp(4, 12)
    } else {
        0
    };
    let rows =
        Layout::vertical([Constraint::Min(0), Constraint::Length(comment_height)]).split(area);

    let with_panel = area.width >= PANEL_MIN_TOTAL;
    let cols = if with_panel {
        Layout::horizontal([Constraint::Min(0), Constraint::Length(PANEL_WIDTH)]).split(rows[0])
    } else {
        Layout::horizontal([Constraint::Min(0), Constraint::Length(0)]).split(rows[0])
    };

    // Main column, padded like a document rather than flush to the border.
    let main = cols[0];
    let body_area = Rect {
        x: main.x + 2,
        width: main.width.saturating_sub(5),
        ..main
    };
    let lines = body_lines(&issue, body_area.width, !with_panel, &th);

    app.detail_lines = lines.len() as u16;
    app.detail_viewport = body_area.height;
    app.detail_scroll = app
        .detail_scroll
        .min(app.detail_lines.saturating_sub(app.detail_viewport));

    f.render_widget(
        Paragraph::new(lines).scroll((app.detail_scroll, 0)),
        body_area,
    );

    if app.detail_lines > app.detail_viewport {
        let mut state =
            ScrollbarState::new(app.detail_lines.saturating_sub(app.detail_viewport) as usize)
                .position(app.detail_scroll as usize);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some(" "))
                .thumb_style(Style::default().fg(th.border)),
            Rect {
                x: main.x + main.width.saturating_sub(2),
                width: 1,
                ..main
            },
            &mut state,
        );
    }

    if with_panel {
        draw_panel(f, &issue, cols[1], &th);
    }

    if comment_mode {
        draw_comment_editor(f, app, rows[1]);
    }
}

fn draw_comment_editor(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let editor = Paragraph::new(super::input_lines(&app.comment, th)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(th.accent))
            .title(Span::styled(
                " Leave a comment\u{2026} ",
                Style::default().fg(th.text).add_modifier(Modifier::BOLD),
            ))
            .title_bottom(Line::from(Span::styled(
                " Ctrl+Enter send \u{00b7} Enter newline \u{00b7} Esc cancel ",
                Style::default().fg(th.muted),
            ))),
    );
    f.render_widget(editor, area);
}

// --------------------------------------------------------------- main column

fn section_rule(title: &str, trailing: &str, width: u16, th: &Theme) -> Line<'static> {
    let head = format!("{title} ");
    let tail = if trailing.is_empty() {
        String::new()
    } else {
        format!(" {trailing}")
    };
    let rule = (width as usize).saturating_sub(head.width() + tail.width());
    Line::from(vec![
        Span::styled(
            head,
            Style::default().fg(th.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled(tail, Style::default().fg(th.muted)),
        Span::styled(" ", Style::default()),
        Span::styled(
            "\u{2500}".repeat(rule.saturating_sub(1)),
            Style::default().fg(th.border),
        ),
    ])
}

fn body_lines(issue: &Issue, width: u16, inline_props: bool, th: &Theme) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = vec![Line::from("")];
    let w = width as usize;

    // Title, large in Linear; bold and wrapped here.
    let title_style = Style::default().fg(th.text).add_modifier(Modifier::BOLD);
    for line in wrap(vec![(issue.title.clone(), title_style)], w, vec![], vec![]) {
        out.push(Line::from(line));
    }

    // "Sub-issue of PF-151 <parent title>"
    if let Some(parent) = &issue.parent {
        let mut spans = vec![
            Span::styled("Sub-issue of ", Style::default().fg(th.muted)),
            state_glyph(parent.state.as_ref(), th),
            Span::styled(
                format!(" {} ", parent.identifier),
                Style::default().fg(th.muted),
            ),
        ];
        let used: usize = spans.iter().map(|s| s.width()).sum();
        spans.push(Span::styled(
            truncate(&parent.title, w.saturating_sub(used)),
            Style::default().fg(th.text_dim),
        ));
        out.push(Line::from(spans));
    }
    out.push(Line::from(""));

    if inline_props {
        out.extend(compact_properties(issue, width, th));
        out.push(Line::from(""));
    }

    // Description.
    match issue
        .description
        .as_deref()
        .filter(|d| !d.trim().is_empty())
    {
        Some(desc) => out.extend(markdown::render(desc, width, &[], th)),
        None => out.push(Line::from(Span::styled(
            "Add description\u{2026}",
            Style::default().fg(th.muted).add_modifier(Modifier::ITALIC),
        ))),
    }
    out.push(Line::from(""));

    // Sub-issues.
    if let Some(children) = issue.children.as_ref().filter(|c| !c.nodes.is_empty()) {
        let done = children
            .nodes
            .iter()
            .filter(|c| {
                c.state
                    .as_ref()
                    .and_then(|s| s.state_type)
                    .is_some_and(|t| t == StateType::Completed)
            })
            .count();
        out.push(Line::from(""));
        out.push(section_rule(
            "Sub-issues",
            &format!("{done}/{}", children.nodes.len()),
            width,
            th,
        ));
        for child in &children.nodes {
            let mut spans = vec![
                Span::raw(" "),
                state_glyph(child.state.as_ref(), th),
                Span::styled(
                    format!(" {:<9} ", child.identifier),
                    Style::default().fg(th.muted),
                ),
            ];
            let used: usize = spans.iter().map(|s| s.width()).sum();
            spans.push(Span::styled(
                truncate(&child.title, w.saturating_sub(used)),
                Style::default().fg(th.text),
            ));
            out.push(Line::from(spans));
        }
        out.push(Line::from(""));
    }

    // Comments.
    out.push(Line::from(""));
    match &issue.comments {
        None => {
            out.push(section_rule("Activity", "", width, th));
            out.push(Line::from(Span::styled(
                "Loading comments\u{2026}",
                Style::default().fg(th.muted),
            )));
        }
        Some(comments) => {
            out.push(section_rule(
                "Activity",
                &format!("{} comments", comments.nodes.len()),
                width,
                th,
            ));
            out.push(Line::from(""));
            out.extend(comment_threads(&comments.nodes, width, th));
            out.push(Line::from(Span::styled(
                "  m  Leave a comment\u{2026}",
                Style::default().fg(th.muted),
            )));
        }
    }
    out.push(Line::from(""));
    out
}

/// Comments as threads: each top-level comment as a card, its replies nested
/// inside it. Linear returns replies as siblings with a `parent`, oldest last,
/// so the tree is rebuilt here and shown chronologically.
fn comment_threads(comments: &[Comment], width: u16, th: &Theme) -> Vec<Line<'static>> {
    let mut replies: HashMap<&str, Vec<&Comment>> = HashMap::new();
    let mut roots: Vec<&Comment> = Vec::new();
    let ids: Vec<&str> = comments.iter().map(|c| c.id.as_str()).collect();
    for comment in comments {
        match comment.parent.as_ref().map(|p| p.id.as_str()) {
            Some(parent) if ids.contains(&parent) => {
                replies.entry(parent).or_default().push(comment)
            }
            _ => roots.push(comment),
        }
    }
    let by_time = |a: &&Comment, b: &&Comment| a.created_at.cmp(&b.created_at);
    roots.sort_by(by_time);
    for list in replies.values_mut() {
        list.sort_by(by_time);
    }

    let mut out = Vec::new();
    for root in roots {
        out.extend(comment_card(root, &replies, width, th));
        out.push(Line::from(""));
    }
    out
}

fn comment_card(
    root: &Comment,
    replies: &HashMap<&str, Vec<&Comment>>,
    width: u16,
    th: &Theme,
) -> Vec<Line<'static>> {
    let border = Style::default().fg(th.border);
    let w = width as usize;
    let mut out = Vec::new();

    out.push(Line::from(vec![
        Span::styled("\u{256d}", border),
        Span::styled("\u{2500}".repeat(w.saturating_sub(2)), border),
        Span::styled("\u{256e}", border),
    ]));

    let gutter = vec![Span::styled("\u{2502} ", border)];
    let mut entries: Vec<(&Comment, bool)> = vec![(root, false)];
    if let Some(list) = replies.get(root.id.as_str()) {
        entries.extend(list.iter().map(|c| (*c, true)));
    }

    for (index, (comment, is_reply)) in entries.iter().enumerate() {
        if index > 0 {
            // A thin divider between the comment and each reply.
            out.push(Line::from(vec![
                Span::styled("\u{2502} ", border),
                Span::styled(
                    "\u{2508}".repeat(w.saturating_sub(4)),
                    Style::default().fg(th.border),
                ),
            ]));
        }
        let indent = if *is_reply { "  " } else { "" };
        let mut header = gutter.clone();
        header.push(Span::raw(indent));
        match &comment.user {
            Some(user) => {
                let name = user_name(user).to_string();
                header.push(Span::styled(
                    initials(&name),
                    Style::default()
                        .fg(ratatui::style::Color::Black)
                        .bg(person_color(&name))
                        .add_modifier(Modifier::BOLD),
                ));
                header.push(Span::styled(
                    format!(" {name}"),
                    Style::default().fg(th.text).add_modifier(Modifier::BOLD),
                ));
            }
            None => header.push(Span::styled(
                "Unknown",
                Style::default()
                    .fg(th.text_dim)
                    .add_modifier(Modifier::BOLD),
            )),
        }
        header.push(Span::styled(
            format!("  {}", super::relative_time(comment.created_at.as_deref())),
            Style::default().fg(th.muted),
        ));
        if comment.edited_at.is_some() {
            header.push(Span::styled(" (edited)", Style::default().fg(th.muted)));
        }
        out.push(Line::from(header));

        let mut body_gutter = gutter.clone();
        body_gutter.push(Span::raw(indent));
        out.extend(markdown::render(
            &comment.body,
            width.saturating_sub(2),
            &body_gutter,
            th,
        ));
        // `render` fills `width - 2` including the gutter, which leaves one
        // cell of padding before the right edge.
    }

    // Close every inner line with the card's right edge.
    for line in out.iter_mut().skip(1) {
        let used = line.width();
        line.spans
            .push(Span::raw(" ".repeat(w.saturating_sub(used + 1))));
        line.spans.push(Span::styled("\u{2502}", border));
    }

    out.push(Line::from(vec![
        Span::styled("\u{2570}", border),
        Span::styled("\u{2500}".repeat(w.saturating_sub(2)), border),
        Span::styled("\u{256f}", border),
    ]));
    out
}

// ---------------------------------------------------------------- properties

/// A property row: dim label then value spans.
fn prop(label: &str, value: Vec<Span<'static>>, th: &Theme) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!(" {label:<9}"),
        Style::default().fg(th.muted),
    )];
    spans.extend(value);
    Line::from(spans)
}

fn heading(text: &str, th: &Theme) -> Line<'static> {
    Line::from(Span::styled(
        format!(" {text}"),
        Style::default().fg(th.muted).add_modifier(Modifier::BOLD),
    ))
}

fn estimate_text(estimate: Option<f64>) -> Option<String> {
    estimate.map(|e| {
        if e.fract() == 0.0 {
            format!("{e:.0} pts")
        } else {
            format!("{e} pts")
        }
    })
}

fn cycle_name(issue: &Issue) -> Option<String> {
    issue.cycle.as_ref().map(|c| {
        c.name
            .clone()
            .unwrap_or_else(|| format!("Cycle {}", c.number.unwrap_or(0.0)))
    })
}

fn panel_lines(issue: &Issue, width: u16, th: &Theme) -> Vec<Line<'static>> {
    let w = width as usize;
    let mut out = vec![Line::from(""), heading("Properties", th)];

    let state_name = issue
        .state
        .as_ref()
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "No status".into());
    out.push(Line::from(vec![
        Span::raw(" "),
        state_glyph(issue.state.as_ref(), th),
        Span::styled(
            format!(" {state_name}"),
            Style::default().fg(state_color(issue.state.as_ref(), th)),
        ),
    ]));
    let priority = issue
        .priority_label
        .clone()
        .unwrap_or_else(|| issue.priority.label().to_string());
    out.push(Line::from(vec![
        Span::raw(" "),
        priority_glyph(issue.priority, th),
        Span::styled(format!(" {priority}"), Style::default().fg(th.text)),
    ]));
    let mut assignee = vec![Span::raw(" ")];
    assignee.extend(person(issue.assignee.as_ref(), th));
    out.push(Line::from(assignee));
    if let Some(cycle) = cycle_name(issue) {
        out.push(Line::from(vec![
            Span::styled(" \u{25d4} ", Style::default().fg(th.accent)),
            Span::styled(cycle, Style::default().fg(th.text)),
        ]));
    }
    if let Some(est) = estimate_text(issue.estimate) {
        out.push(Line::from(vec![
            Span::styled(" \u{25c7} ", Style::default().fg(th.muted)),
            Span::styled(est, Style::default().fg(th.text)),
        ]));
    }
    if let Some(due) = &issue.due_date {
        out.push(Line::from(vec![
            Span::styled(" \u{2691} ", Style::default().fg(th.warning)),
            Span::styled(
                format!("Due {}", short_date(Some(due))),
                Style::default().fg(th.text),
            ),
        ]));
    }

    out.push(Line::from(""));
    out.push(heading("Labels", th));
    match issue.labels.as_ref().filter(|l| !l.nodes.is_empty()) {
        Some(labels) => {
            // Chips flow onto as many rows as the panel needs.
            let mut row: Vec<Span<'static>> = vec![Span::raw(" ")];
            let mut used = 1;
            for label in &labels.nodes {
                let chip = label_chip(label, th);
                let chip_w: usize = chip.iter().map(|s| s.width()).sum();
                if used + chip_w > w && used > 1 {
                    out.push(Line::from(std::mem::replace(
                        &mut row,
                        vec![Span::raw(" ")],
                    )));
                    used = 1;
                }
                used += chip_w;
                row.extend(chip);
            }
            out.push(Line::from(row));
        }
        None => out.push(Line::from(Span::styled(
            " No labels",
            Style::default().fg(th.muted),
        ))),
    }

    out.push(Line::from(""));
    out.push(heading("Project", th));
    match &issue.project {
        Some(project) => {
            let color = project
                .color
                .as_deref()
                .and_then(hex_color)
                .unwrap_or(th.secondary);
            out.push(Line::from(vec![
                Span::styled(" \u{25a3} ", Style::default().fg(color)),
                Span::styled(
                    truncate(&project.name, w.saturating_sub(4)),
                    Style::default().fg(th.text),
                ),
            ]));
            if let Some(milestone) = &issue.project_milestone {
                out.push(Line::from(vec![
                    Span::styled("   \u{25c6} ", Style::default().fg(th.muted)),
                    Span::styled(
                        truncate(&milestone.name, w.saturating_sub(6)),
                        Style::default().fg(th.text_dim),
                    ),
                ]));
            }
        }
        None => out.push(Line::from(Span::styled(
            " No project",
            Style::default().fg(th.muted),
        ))),
    }

    out.push(Line::from(""));
    out.push(heading("Created by", th));
    let mut creator = vec![Span::raw(" ")];
    match &issue.creator {
        Some(user) => creator.extend(person(Some(user), th)),
        None => creator.push(Span::styled("Unknown", Style::default().fg(th.muted))),
    }
    out.push(Line::from(creator));
    out.push(Line::from(Span::styled(
        format!(
            " {} \u{00b7} {}",
            super::format_date(issue.created_at.as_deref()),
            super::relative_time(issue.created_at.as_deref())
        ),
        Style::default().fg(th.muted),
    )));
    out.push(Line::from(""));
    out.push(prop(
        "Updated",
        vec![Span::styled(
            super::relative_time(issue.updated_at.as_deref()),
            Style::default().fg(th.text_dim),
        )],
        th,
    ));
    out.push(prop(
        "ID",
        vec![Span::styled(
            issue.identifier.clone(),
            Style::default().fg(th.text_dim),
        )],
        th,
    ));
    if let Some(branch) = &issue.branch_name {
        out.push(Line::from(""));
        out.push(heading("Branch", th));
        out.push(Line::from(Span::styled(
            format!(" {}", truncate(branch, w.saturating_sub(2))),
            Style::default().fg(th.secondary),
        )));
    }
    out
}

fn draw_panel(f: &mut Frame, issue: &Issue, area: Rect, th: &Theme) {
    // A rule separates the panel from the document, as in Linear.
    for y in area.y..area.y + area.height {
        f.buffer_mut()[(area.x, y)]
            .set_symbol("\u{2502}")
            .set_style(Style::default().fg(th.border));
    }
    let inner = Rect {
        x: area.x + 2,
        width: area.width.saturating_sub(3),
        ..area
    };
    f.render_widget(Paragraph::new(panel_lines(issue, inner.width, th)), inner);
}

/// The panel's essentials as a few dense lines, for terminals too narrow to
/// give the panel its own column.
fn compact_properties(issue: &Issue, width: u16, th: &Theme) -> Vec<Line<'static>> {
    let sep = || Span::styled("  \u{00b7}  ", Style::default().fg(th.muted));
    let state_name = issue
        .state
        .as_ref()
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "No status".into());

    let mut first = vec![
        state_glyph(issue.state.as_ref(), th),
        Span::styled(
            format!(" {state_name}"),
            Style::default().fg(state_color(issue.state.as_ref(), th)),
        ),
        sep(),
        priority_glyph(issue.priority, th),
        Span::styled(
            format!(" {}", issue.priority.label()),
            Style::default().fg(th.text),
        ),
        sep(),
    ];
    first.extend(person(issue.assignee.as_ref(), th));

    let mut second: Vec<(String, Style)> = Vec::new();
    let dim = Style::default().fg(th.text_dim);
    if let Some(project) = &issue.project {
        second.push((format!("\u{25a3} {}   ", project.name), dim));
    }
    if let Some(cycle) = cycle_name(issue) {
        second.push((format!("\u{25d4} {cycle}   "), dim));
    }
    if let Some(est) = estimate_text(issue.estimate) {
        second.push((format!("\u{25c7} {est}   "), dim));
    }
    if let Some(due) = &issue.due_date {
        second.push((format!("\u{2691} Due {}   ", short_date(Some(due))), dim));
    }
    let creator = issue
        .creator
        .as_ref()
        .map(|u| user_name(u).to_string())
        .unwrap_or_else(|| "unknown".into());
    second.push((
        format!(
            "Created by {creator} {}",
            super::relative_time(issue.created_at.as_deref())
        ),
        Style::default().fg(th.muted),
    ));

    let mut out: Vec<Line<'static>> = vec![Line::from(first)];
    for line in wrap(second, width as usize, vec![], vec![]) {
        out.push(Line::from(line));
    }
    if let Some(labels) = issue.labels.as_ref().filter(|l| !l.nodes.is_empty()) {
        let chips: Vec<Span<'static>> = labels
            .nodes
            .iter()
            .flat_map(|l| label_chip(l, th))
            .collect();
        out.push(Line::from(chips));
    }
    out
}
