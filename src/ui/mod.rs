pub mod cycle_detail;
pub mod cycle_list;
pub mod issue_detail;
pub mod issue_list;
pub mod new_issue;
pub mod popup;
pub mod project_detail;
pub mod project_list;

use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{App, Input, InputMode, Popup, Screen, Tab};
use crate::config::Theme;

/// Days since the Unix epoch for a civil (proleptic Gregorian) date.
/// Howard Hinnant's `days_from_civil`.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Parse an ISO-8601 UTC timestamp (`2026-03-04T18:34:15.000Z`) into Unix seconds.
fn parse_iso(ts: &str) -> Option<i64> {
    let bytes = ts.as_bytes();
    if bytes.len() < 19 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let num = |range: std::ops::Range<usize>| ts.get(range)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + sec)
}

/// Render a timestamp as an age relative to now ("3d ago"), falling back to the
/// raw date when it can't be parsed.
pub fn relative_time(ts: Option<&str>) -> String {
    let Some(ts) = ts else {
        return "-".to_string();
    };
    let Some(then) = parse_iso(ts) else {
        return format_date(Some(ts));
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(then);
    let secs = now - then;
    if secs < 0 {
        return format_date(Some(ts));
    }
    match secs {
        s if s < 60 => "just now".to_string(),
        s if s < 3600 => format!("{}m ago", s / 60),
        s if s < 86_400 => format!("{}h ago", s / 3600),
        s if s < 2_592_000 => format!("{}d ago", s / 86_400),
        s if s < 31_536_000 => format!("{}mo ago", s / 2_592_000),
        s => format!("{}y ago", s / 31_536_000),
    }
}

/// Format an ISO date string to just the date portion (YYYY-MM-DD).
pub fn format_date(date_str: Option<&str>) -> String {
    date_str
        .and_then(|s| s.get(..10))
        .unwrap_or("-")
        .to_string()
}

/// Render a text field with a visible block cursor at the insertion point.
pub fn input_spans(input: &Input, theme: &Theme) -> Vec<Span<'static>> {
    let (before, after) = input.value.split_at(input.cursor);
    let mut cursor_chars = after.chars();
    let under_cursor = cursor_chars.next();
    let rest: String = cursor_chars.collect();

    vec![
        Span::raw(before.to_string()),
        Span::styled(
            under_cursor.map(String::from).unwrap_or_else(|| " ".into()),
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::REVERSED),
        ),
        Span::raw(rest),
    ]
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let show_tabs = matches!(
        app.screen,
        Screen::IssueList | Screen::ProjectList | Screen::CycleList
    );

    if show_tabs {
        let chunks = Layout::vertical([
            Constraint::Length(1), // tab bar
            Constraint::Min(0),    // content
        ])
        .split(f.area());

        draw_tab_bar(f, app, chunks[0]);

        match app.screen {
            Screen::IssueList => issue_list::draw(f, app, chunks[1]),
            Screen::ProjectList => project_list::draw(f, app, chunks[1]),
            Screen::CycleList => cycle_list::draw(f, app, chunks[1]),
            _ => {}
        }
    } else {
        let area = f.area();
        match app.screen {
            Screen::IssueDetail => issue_detail::draw(f, app, area),
            Screen::ProjectDetail => project_detail::draw(f, app, area),
            Screen::CycleDetail => cycle_detail::draw(f, app, area),
            _ => {}
        }
    }

    // Draw popup overlay
    if app.popup != Popup::None {
        popup::draw(f, app);
    }

    // Draw error popup (highest priority overlay)
    if let Some(err) = app.error_popup.clone() {
        draw_error_popup(f, &err, app);
    }

    // Draw the issue-creation form
    if app.input_mode == InputMode::NewIssue {
        new_issue::draw(f, app);
    }

    // Draw help overlay
    if app.show_help {
        draw_help(f, app);
    }
}

fn draw_tab_bar(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let mut spans = vec![Span::raw(" ")];
    for (i, tab) in Tab::all().iter().enumerate() {
        let label = format!(" {}:{} ", i + 1, tab.label());
        if *tab == app.tab {
            spans.push(Span::styled(
                label,
                Style::default()
                    .fg(th.accent)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED),
            ));
        } else {
            spans.push(Span::styled(label, Style::default().fg(th.muted)));
        }
        spans.push(Span::raw(" "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);
    horizontal[0]
}

fn draw_error_popup(f: &mut Frame, message: &str, app: &App) {
    let th = &app.theme;
    let lines: Vec<Line> = message.lines().map(|l| Line::from(l.to_string())).collect();
    let height = (lines.len() as u16 + 4).min(15);
    let width = 50.min(f.area().width.saturating_sub(4));
    let area = centered_rect(width, height, f.area());

    f.render_widget(Clear, area);
    let popup = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(th.error))
                .title(" Error ")
                .title_style(Style::default().fg(th.error).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(popup, area);

    // Hint at bottom
    let hint_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 1,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Press any key to dismiss",
            Style::default().fg(th.muted),
        ))),
        hint_area,
    );
}

fn draw_help(f: &mut Frame, app: &App) {
    let th = &app.theme;
    let section = |text: &str| -> Line<'static> {
        Line::from(vec![Span::styled(
            text.to_string(),
            Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
        )])
    };
    let key_line = |key: &str, desc: &str| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("  {key:<9}  "), Style::default().fg(th.warning)),
            Span::raw(desc.to_string()),
        ])
    };

    let help_text = vec![
        section("Navigation"),
        key_line("j/k", "Move cursor up/down"),
        key_line("gg/G", "Go to first/last item"),
        key_line("Enter", "Open issue"),
        key_line("Space", "Open issue (Linear: peek)"),
        key_line("Esc", "Back / close"),
        Line::from(""),
        section("Go to view"),
        key_line("g e", "All issues"),
        key_line("g m", "My issues"),
        key_line("g p", "Projects"),
        key_line("g c", "Cycles"),
        key_line("1-4", "Same, by tab number"),
        Line::from(""),
        section("Issue actions"),
        key_line("c", "Create issue"),
        key_line("s", "Change status"),
        key_line("p", "Change priority"),
        key_line("!@#$)", "Urgent/High/Medium/Low/None"),
        key_line("a", "Assign to someone"),
        key_line("i", "Assign to me"),
        key_line("m", "Add comment (Ctrl+M)"),
        Line::from(""),
        section("Copy & open"),
        key_line("y", "Copy issue ID (Ctrl+.)"),
        key_line("Y", "Copy issue URL (Ctrl+Shift+,)"),
        key_line("b", "Copy branch name (Ctrl+Shift+.)"),
        key_line("o", "Open on linear.app"),
        Line::from(""),
        section("Search & filter"),
        key_line("/", "Filter as you type"),
        key_line("C-g", "Search all of Linear"),
        key_line("f/F", "Filter / clear filters"),
        Line::from(""),
        section("Other"),
        key_line("t", "Switch team"),
        key_line("C-r", "Refresh"),
        key_line("?", "Toggle this help"),
        key_line("q", "Quit"),
        Line::from(""),
        section("Scrolling"),
        key_line("C-d/C-u", "Half page down/up"),
        key_line("PgDn/PgUp", "Full page down/up"),
        key_line("wheel", "Mouse scrolling"),
        Line::from(""),
        section("Editing"),
        key_line("C-w", "Delete previous word"),
        key_line("C-u/C-k", "Delete to start/end"),
        key_line("C-a/C-e", "Jump to start/end"),
        key_line("C-Enter", "Submit"),
    ];

    let total = help_text.len() as u16;
    let height = (total + 2).min(f.area().height.saturating_sub(4));
    let width = 48.min(f.area().width.saturating_sub(4));
    let area = centered_rect(width, height, f.area());
    let scroll = app
        .help_scroll
        .min(total.saturating_sub(height.saturating_sub(2)));

    f.render_widget(Clear, area);
    let help = Paragraph::new(help_text).scroll((scroll, 0)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help (j/k to scroll, any other key closes) ")
            .title_style(Style::default().fg(th.accent).add_modifier(Modifier::BOLD)),
    );
    f.render_widget(help, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iso_timestamps() {
        assert_eq!(parse_iso("1970-01-01T00:00:00.000Z"), Some(0));
        assert_eq!(parse_iso("2026-03-04T18:34:15.000Z"), Some(1772649255));
        assert_eq!(parse_iso("not-a-date"), None);
    }

    #[test]
    fn relative_time_falls_back_to_the_date() {
        assert_eq!(relative_time(None), "-");
        assert_eq!(relative_time(Some("garbage-value-here")), "garbage-va");
    }

    #[test]
    fn relative_time_reports_an_age() {
        assert!(relative_time(Some("2020-01-01T00:00:00.000Z")).contains("ago"));
    }
}
