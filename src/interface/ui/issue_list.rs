//! Issue lists, drawn the way Linear draws them: a row of view presets, then
//! issues stacked under collapsible status (or assignee, priority, project)
//! headers, each row carrying priority, identifier, status, title, labels,
//! project, estimate, assignee, and date.
//!
//! The same renderer serves every issue list — a team's issues, My Issues, a
//! saved view, and the issues inside a project or cycle.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use super::widgets::{
    agent_glyph, avatar, estimate, fit, label_chip, message_row, priority_glyph, short_date,
    state_glyph, truncate, user_name,
};
use crate::config::Theme;
use crate::entity::AgentStatus;
use crate::entity::Issue;
use crate::interface::app::FilterSummary;
use crate::interface::app::{App, Chip, IssueSource, ListRow, ListView};
use crate::interface::grouping::{GroupBy, Preset};
use crate::interface::look::hex_color;

/// The issue-list screen: preset chips over the grouped list.
pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // presets + display summary
        Constraint::Length(1), // breathing room
        Constraint::Min(0),    // list
    ])
    .split(area);
    let count = draw_list(f, app, chunks[2]);
    draw_toolbar(f, app, chunks[0], count);
}

/// Preset chips on the left, grouping and `count` — the issues [`draw_list`]
/// drew — on the right.
pub fn draw_toolbar(f: &mut Frame, app: &mut App, area: Rect, count: usize) {
    let th = app.theme;
    let mut spans = vec![Span::raw(" ")];
    let mut x = area.x + 1;
    app.frame.chip_areas.clear();
    for preset in Preset::all() {
        let label = format!(" {} ", preset.label());
        let width = label.width() as u16;
        let style = if *preset == app.preset() {
            Style::default()
                .fg(th.text)
                .bg(th.selection_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(th.muted)
        };
        app.frame.chip_areas.push((
            Rect {
                x,
                y: area.y,
                width,
                height: 1,
            },
            Chip::Preset(*preset),
        ));
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
        x += width + 1;
    }

    let mut right = vec![];
    if app.list().filters.is_active() {
        right.push(Span::styled(
            format!("\u{25bc} {}  ", app.list().filters.summary()),
            Style::default().fg(th.secondary),
        ));
    }
    if !app.list().search.is_empty() {
        right.push(Span::styled(
            format!("/{}  ", app.list().search.value),
            Style::default().fg(th.warning),
        ));
    }
    right.push(Span::styled(
        format!("{} ", app.view.group_by.label()),
        Style::default().fg(th.text_dim),
    ));
    right.push(Span::styled("\u{00b7} ", Style::default().fg(th.muted)));
    right.push(Span::styled(
        format!("{count} issues "),
        Style::default().fg(th.text_dim),
    ));

    let left_w: usize = spans.iter().map(|s| s.width()).sum();
    let right_w: usize = right.iter().map(|s| s.width()).sum();
    let pad = (area.width as usize).saturating_sub(left_w + right_w);
    spans.push(Span::raw(" ".repeat(pad)));
    spans.extend(right);
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// The grouped list itself. Records its rows and area for mouse hit-testing,
/// and returns how many issues the cursor can reach.
///
/// The list is grouped once here, and everything the frame needs from it is
/// read off that one pass.
pub fn draw_list(f: &mut Frame, app: &mut App, area: Rect) -> usize {
    let th = app.theme;
    app.frame.list_viewport = area.height;
    app.frame.list_area = area;

    let ListView { issues, rows } = app.list_view();
    let count = issues.len();
    let selected = app.selected_index();
    let selected_row = rows
        .iter()
        .position(|r| matches!(r, ListRow::Issue { ordinal, .. } if *ordinal == selected));

    if rows.is_empty() {
        app.frame.list_rows.clear();
        let message = if app.loading() {
            format!("{} Loading issues\u{2026}", app.spinner_symbol())
        } else if !app.list().search.is_empty() || app.list().filters.is_active() {
            "No issues match the current filter".to_string()
        } else if app.preset() != Preset::All {
            format!(
                "No {} issues \u{2014} Shift+Tab or g e shows all issues",
                app.preset().label().to_lowercase()
            )
        } else {
            "No issues".to_string()
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                message,
                Style::default().fg(th.muted),
            )))
            .alignment(ratatui::layout::Alignment::Center),
            message_row(area),
        );
        return count;
    }

    // Scroll so the cursor stays visible — and when the cursor is the first
    // issue of a group, keep that group's header in view with it.
    let height = area.height as usize;
    let mut offset = app.frame.offsets.issues[app.issue_source()];
    if let Some(sel) = selected_row {
        let want_top = if sel > 0 && matches!(rows[sel - 1], ListRow::Group { .. }) {
            sel - 1
        } else {
            sel
        };
        if want_top < offset {
            offset = want_top;
        } else if height > 0 && sel >= offset + height {
            offset = sel + 1 - height;
        }
    }
    offset = offset.min(rows.len().saturating_sub(height));

    let id_width = issues
        .iter()
        .map(|i| i.identifier.width())
        .max()
        .unwrap_or(6)
        .min(10);
    let width = area.width as usize;
    // A column that only repeats what the page or the group header already
    // says is left out: the project inside a project, the assignee when
    // grouped by assignee.
    let columns = Columns {
        project: app.issue_source() != IssueSource::Project
            && app.view.group_by != GroupBy::Project,
        assignee: app.view.group_by != GroupBy::Assignee,
    };

    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .skip(offset)
        .take(height)
        .map(|(index, row)| match row {
            ListRow::Group {
                label,
                color,
                glyph,
                count,
                collapsed,
                ..
            } => group_line(label, *color, glyph, *count, *collapsed, width, &th),
            ListRow::Issue { ordinal, depth } => match issues.get(*ordinal) {
                Some(issue) => issue_line(
                    issue,
                    RowState {
                        depth: *depth,
                        selected: Some(index) == selected_row,
                        agent: app.agent_for(&issue.identifier).map(|a| a.status),
                    },
                    id_width,
                    width,
                    columns,
                    &th,
                ),
                None => Line::from(""),
            },
        })
        .collect();

    *app.list_offset() = offset;
    app.frame.list_rows = rows.into_iter().skip(offset).take(height).collect();
    f.render_widget(Paragraph::new(lines), area);
    count
}

fn group_line(
    label: &str,
    color: ratatui::style::Color,
    glyph: &str,
    count: usize,
    collapsed: bool,
    width: usize,
    th: &Theme,
) -> Line<'static> {
    let chevron = if collapsed { "\u{25b8}" } else { "\u{25be}" };
    let spans = vec![
        Span::styled(format!(" {chevron} "), Style::default().fg(th.muted)),
        Span::styled(format!("{glyph} "), Style::default().fg(color)),
        Span::styled(
            label.to_string(),
            Style::default().fg(th.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  {count}"), Style::default().fg(th.muted)),
    ];
    let used: usize = spans.iter().map(|s| s.width()).sum();
    let mut spans = spans;
    spans.push(Span::raw(" ".repeat(width.saturating_sub(used))));
    for span in &mut spans {
        span.style = span.style.bg(th.surface);
    }
    Line::from(spans)
}

/// Optional columns an issue row may carry, when there is room.
#[derive(Debug, Clone, Copy)]
pub struct Columns {
    pub project: bool,
    pub assignee: bool,
}

/// What sets one row apart from the others.
#[derive(Debug, Clone, Copy)]
pub struct RowState {
    /// How deep a sub-issue nests under its parent.
    pub depth: u8,
    pub selected: bool,
    /// The state of a herdr agent working on the issue.
    pub agent: Option<AgentStatus>,
}

/// One issue row. Columns drop out from the right as the pane narrows, in
/// roughly the order Linear hides them: labels first, then project, estimate,
/// date — the title and assignee go last.
pub fn issue_line(
    issue: &Issue,
    row: RowState,
    id_width: usize,
    width: usize,
    columns: Columns,
    th: &Theme,
) -> Line<'static> {
    let RowState {
        depth,
        selected,
        agent,
    } = row;
    let show_labels = width >= 110;
    let show_project = width >= 90 && columns.project;
    let show_estimate = width >= 80;
    let show_date = width >= 64;
    let show_assignee = width >= 44 && columns.assignee;

    // Right-hand cluster, built first so the title gets whatever is left.
    let mut right: Vec<Span<'static>> = Vec::new();
    // A herdr agent on the issue leads the cluster: it is what changes.
    if let Some(status) = agent {
        right.push(agent_glyph(status, th));
        right.push(Span::raw(" "));
    }
    if show_labels && let Some(labels) = &issue.labels {
        for label in labels.nodes.iter().take(2) {
            let mut chip = label_chip(label, th);
            if let Some(name) = chip.get_mut(1) {
                name.content = format!(" {} ", truncate(&label.name, 14)).into();
            }
            right.extend(chip);
        }
        if labels.nodes.len() > 2 {
            right.push(Span::styled(
                format!("+{} ", labels.nodes.len() - 2),
                Style::default().fg(th.muted),
            ));
        }
    }
    if show_project && let Some(project) = &issue.project {
        let dot = project
            .color
            .as_deref()
            .and_then(hex_color)
            .unwrap_or(th.secondary);
        right.push(Span::styled("\u{25a3} ", Style::default().fg(dot)));
        right.push(Span::styled(
            format!("{} ", truncate(&project.name, 18)),
            Style::default().fg(th.text_dim),
        ));
    }
    if show_estimate {
        let est = issue.estimate.map(estimate).unwrap_or_default();
        right.push(Span::styled(
            format!("{est:>2} "),
            Style::default().fg(th.muted),
        ));
    }
    if show_assignee {
        match &issue.assignee {
            Some(user) => {
                let name = user_name(user);
                right.push(avatar(name));
            }
            None => right.push(Span::styled("\u{25cc} ", Style::default().fg(th.muted))),
        }
        right.push(Span::raw(" "));
    }
    if show_date {
        right.push(Span::styled(
            format!("{:>6} ", short_date(issue.created_at.as_deref())),
            Style::default().fg(th.muted),
        ));
    }
    let right_w: usize = right.iter().map(|s| s.width()).sum();

    let marker = if selected {
        Span::styled("\u{258c}", Style::default().fg(th.accent))
    } else {
        Span::raw(" ")
    };
    // A sub-issue shifts its whole row right under a hook, as Linear nests it.
    let nest = if depth > 0 {
        Span::styled(
            format!("{}\u{2570} ", "  ".repeat(depth as usize - 1)),
            Style::default().fg(th.border),
        )
    } else {
        Span::raw("")
    };
    let mut spans = vec![
        marker,
        nest,
        priority_glyph(issue.priority, th),
        Span::raw("  "),
        Span::styled(
            fit(&issue.identifier, id_width),
            Style::default().fg(th.muted),
        ),
        Span::raw("  "),
        state_glyph(issue.state.as_ref(), th),
        Span::raw(" "),
    ];
    let left_w: usize = spans.iter().map(|s| s.width()).sum();

    // Parent reference on a sub-issue whose parent is not beside it.
    let parent_hint = match (&issue.parent, depth) {
        (Some(parent), 0) => format!(" \u{2190} {}", parent.identifier),
        _ => String::new(),
    };
    let title_room = width.saturating_sub(left_w + right_w + 1);
    let title = truncate(&issue.title, title_room.saturating_sub(parent_hint.width()));
    let title_style = if selected {
        Style::default().fg(th.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(th.text)
    };
    let title_w = title.width();
    spans.push(Span::styled(title, title_style));
    let hint = truncate(&parent_hint, title_room.saturating_sub(title_w));
    let hint_w = hint.width();
    spans.push(Span::styled(hint, Style::default().fg(th.muted)));
    spans.push(Span::raw(
        " ".repeat(width.saturating_sub(left_w + title_w + hint_w + right_w)),
    ));
    spans.extend(right);

    if selected {
        for span in &mut spans {
            span.style = span.style.bg(th.selection_bg);
        }
    }
    Line::from(spans)
}
