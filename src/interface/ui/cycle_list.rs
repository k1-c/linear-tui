//! A team's cycles, current one marked, each with its progress and dates.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::widgets::{progress_bar, row, short_date, truncate};
use crate::config::Theme;
use crate::entity::Cycle;
use crate::interface::app::App;

pub fn cycle_name(cycle: &Cycle) -> String {
    cycle.label()
}

/// Whether `now` (an ISO timestamp) falls inside the cycle.
fn is_current(cycle: &Cycle, now: &str) -> bool {
    match (cycle.starts_at.as_deref(), cycle.ends_at.as_deref()) {
        (Some(start), Some(end)) => start <= now && now < end,
        _ => false,
    }
}

pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil-from-days (Howard Hinnant), inverse of ui::days_from_civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let th = app.theme;
    let width = area.width as usize;
    let list = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    app.frame.list_area = list;
    app.frame.list_viewport = list.height;

    if app.store.cycles.items.is_empty() {
        app.frame.row_targets.clear();
        let message = if app.loading() {
            format!("{} Loading cycles\u{2026}", app.spinner_symbol())
        } else {
            "This team has no cycles".to_string()
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
    let sel = app.view.selected_cycle_index;
    let mut offset = app.frame.offsets.cycles;
    if sel < offset {
        offset = sel;
    } else if height > 0 && sel >= offset + height {
        offset = sel + 1 - height;
    }
    app.frame.offsets.cycles = offset;

    let now = now_iso();
    let lines: Vec<Line> = app
        .store
        .cycles
        .items
        .iter()
        .enumerate()
        .skip(offset)
        .take(height)
        .map(|(index, cycle)| cycle_row(cycle, &now, width, index == sel, &th))
        .collect();
    app.frame.row_targets = (offset..offset + lines.len()).map(Some).collect();
    f.render_widget(Paragraph::new(lines), list);
}

fn cycle_row(cycle: &Cycle, now: &str, width: usize, selected: bool, th: &Theme) -> Line<'static> {
    let current = is_current(cycle, now);
    let past = cycle.ends_at.as_deref().is_some_and(|end| end <= now);
    let (glyph, color) = if current {
        ("\u{25d4}", th.accent)
    } else if past {
        ("\u{25cf}", th.muted)
    } else {
        ("\u{25cb}", th.text_dim)
    };
    let progress = cycle.progress.unwrap_or(0.0);

    let mut left = vec![
        Span::styled(format!("{glyph} "), Style::default().fg(color)),
        Span::styled(
            truncate(&cycle_name(cycle), width.saturating_sub(50)),
            Style::default()
                .fg(if past { th.text_dim } else { th.text })
                .add_modifier(if selected || current {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ),
    ];
    if current {
        left.push(Span::styled("  Current", Style::default().fg(th.accent)));
    }

    let mut right = progress_bar(progress, 12, color, th);
    right.push(Span::styled(
        format!(" {:>3.0}%   ", progress * 100.0),
        Style::default().fg(th.text_dim),
    ));
    right.push(Span::styled(
        format!(
            "{:>6} \u{2192} {:<6} ",
            short_date(cycle.starts_at.as_deref()),
            short_date(cycle.ends_at.as_deref())
        ),
        Style::default().fg(th.muted),
    ));
    row(left, right, width, selected, th)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_is_a_well_formed_timestamp() {
        let now = now_iso();
        assert_eq!(now.len(), 24);
        assert!(super::super::parse_iso(&now).is_some());
    }
}
