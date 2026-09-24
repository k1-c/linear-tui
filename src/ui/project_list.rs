//! A team's projects: status, health, progress, lead, and target date.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::widgets::{fit, progress_bar, row, short_date, truncate, user_name};
use crate::api::types::{Project, hex_color};
use crate::app::App;
use crate::config::Theme;

/// A project state's colour, from the theme so it reads on light
/// backgrounds too. `state` is Linear's string, and workspaces can add their
/// own, so anything unknown takes the plain text colour.
pub fn project_state_color(state: Option<&str>, th: &Theme) -> Color {
    match state {
        Some("started") => th.warning,
        Some("planned") => th.accent,
        Some("completed") => th.success,
        Some("paused") => th.secondary,
        Some("cancelled" | "canceled" | "backlog") => th.muted,
        _ => th.text,
    }
}

/// Linear's project health, as a coloured word.
pub fn health(project: &Project, th: &Theme) -> Span<'static> {
    match project.health.as_deref() {
        Some("onTrack") => Span::styled("On track", Style::default().fg(th.success)),
        Some("atRisk") => Span::styled("At risk", Style::default().fg(th.warning)),
        Some("offTrack") => Span::styled("Off track", Style::default().fg(th.error)),
        _ => Span::styled("", Style::default()),
    }
}

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let th = app.theme;
    let width = area.width as usize;

    let header = Line::from(Span::styled(
        format!(
            " {}{}",
            fit("Name", width.saturating_sub(60)),
            "  Health       Progress         Lead         Target"
        ),
        Style::default().fg(th.muted),
    ));
    f.render_widget(Paragraph::new(header), Rect { height: 1, ..area });

    let list = Rect {
        y: area.y + 2,
        height: area.height.saturating_sub(2),
        ..area
    };
    app.list_area = list;
    app.list_viewport = list.height;

    if app.project_rows().is_empty() {
        app.row_targets.clear();
        let message = if app.loading() {
            format!("{} Loading projects\u{2026}", app.spinner_symbol())
        } else if matches!(app.nav, crate::app::Nav::View(_)) {
            "No projects in this view".to_string()
        } else {
            "No projects in this team".to_string()
        };
        f.render_widget(
            Paragraph::new(Span::styled(message, Style::default().fg(th.muted)))
                .alignment(ratatui::layout::Alignment::Center),
            Rect {
                y: list.y + list.height / 3,
                height: 1,
                ..list
            },
        );
        return;
    }

    let height = list.height as usize;
    let mut offset = app.project_table().offset();
    let sel = app.project_cursor();
    if sel < offset {
        offset = sel;
    } else if height > 0 && sel >= offset + height {
        offset = sel + 1 - height;
    }
    *app.project_table().offset_mut() = offset;

    let lines: Vec<Line> = app
        .project_rows()
        .iter()
        .enumerate()
        .skip(offset)
        .take(height)
        .map(|(index, project)| project_row(project, width, index == sel, &th))
        .collect();
    app.row_targets = (offset..offset + lines.len()).map(Some).collect();
    f.render_widget(Paragraph::new(lines), list);
}

fn project_row(project: &Project, width: usize, selected: bool, th: &Theme) -> Line<'static> {
    let color = project
        .color
        .as_deref()
        .and_then(hex_color)
        .unwrap_or_else(|| project_state_color(project.state.as_deref(), th));
    let progress = project.progress.unwrap_or(0.0);
    let lead = project
        .lead
        .as_ref()
        .map(|u| user_name(u).to_string())
        .unwrap_or_else(|| "-".into());

    let mut right = vec![Span::raw("  ")];
    let mut h = health(project, th);
    h.content = fit(&h.content, 11).into();
    right.push(h);
    right.push(Span::raw("  "));
    right.extend(progress_bar(progress, 10, color, th));
    right.push(Span::styled(
        format!(" {:>3.0}%  ", progress * 100.0),
        Style::default().fg(th.text_dim),
    ));
    right.push(Span::styled(
        fit(&lead, 12),
        Style::default().fg(th.text_dim),
    ));
    right.push(Span::styled(
        format!(" {:>7} ", short_date(project.target_date.as_deref())),
        Style::default().fg(th.muted),
    ));

    let state = project.state.as_deref().unwrap_or("");
    let name_room = width.saturating_sub(62 + state.len());
    let left = vec![
        Span::styled("\u{25a3} ", Style::default().fg(color)),
        Span::styled(
            truncate(&project.name, name_room),
            Style::default().fg(th.text).add_modifier(if selected {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
        ),
        Span::styled(
            format!("  {state}"),
            Style::default().fg(project_state_color(Some(state), th)),
        ),
    ];
    row(left, right, width, selected, th)
}
