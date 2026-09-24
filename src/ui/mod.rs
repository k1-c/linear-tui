pub mod cycle_detail;
pub mod cycle_list;
pub mod issue_detail;
pub mod issue_list;
pub mod markdown;
pub mod new_issue;
pub mod popup;
pub mod project_detail;
pub mod project_list;
pub mod sidebar;
pub mod view_list;
pub mod widgets;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Input, InputMode, Nav, Popup, Screen, TeamSection};
use crate::config::Theme;
use crate::keys;

/// Narrowest terminal that still gets the sidebar; below it the content pane
/// needs every column, and `Tab` has nothing to focus.
const SIDEBAR_MIN_WIDTH: u16 = 100;

/// What the renderer keeps from one frame to the next for its own sake —
/// never read by `app`. The main loop owns it, next to the `App`, so the
/// model does not depend on the view.
#[derive(Debug, Default)]
pub struct Cache {
    /// The detail view's rendered Markdown.
    pub detail_markdown: issue_detail::Memo,
}

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

/// A multi-line text field: one line per `\n`, with the block cursor on the
/// line that holds it.
pub fn input_lines(input: &Input, theme: &Theme) -> Vec<Line<'static>> {
    let cursor_style = Style::default()
        .fg(theme.highlight_fg)
        .add_modifier(Modifier::REVERSED);
    let mut out = Vec::new();
    let mut offset = 0;
    for part in input.value.split('\n') {
        let start = offset;
        let end = offset + part.len();
        if (start..=end).contains(&input.cursor) {
            let (before, after) = part.split_at(input.cursor - start);
            let mut chars = after.chars();
            let under = chars.next().map(String::from).unwrap_or_else(|| " ".into());
            out.push(Line::from(vec![
                Span::raw(before.to_string()),
                Span::styled(under, cursor_style),
                Span::raw(chars.collect::<String>()),
            ]));
        } else {
            out.push(Line::from(part.to_string()));
        }
        offset = end + 1;
    }
    out
}

/// Below this the layout has no room for its own chrome; the frame says so
/// instead of drawing panes that cannot fit.
const MIN_WIDTH: u16 = 20;
const MIN_HEIGHT: u16 = 5;

pub fn draw(f: &mut Frame, app: &mut App, cache: &mut Cache) {
    let area = f.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(f, app);
        return;
    }
    let show_sidebar = app.sidebar_visible && area.width >= SIDEBAR_MIN_WIDTH;
    if !show_sidebar {
        app.sidebar_focus = false;
        app.frame.sidebar_area = Rect::ZERO;
    }

    let cols = if show_sidebar {
        Layout::horizontal([Constraint::Length(app.sidebar_width), Constraint::Min(0)]).split(area)
    } else {
        Layout::horizontal([Constraint::Length(0), Constraint::Min(0)]).split(area)
    };
    if show_sidebar {
        sidebar::draw(f, app, cols[0]);
    }

    let rows = Layout::vertical([
        Constraint::Length(1), // breadcrumb
        Constraint::Length(1), // rule
        Constraint::Min(0),    // content
        Constraint::Length(1), // status bar
    ])
    .split(cols[1]);

    draw_breadcrumb(f, app, rows[0]);
    let th = app.theme;
    f.render_widget(
        Paragraph::new(Span::styled(
            "\u{2500}".repeat(rows[1].width as usize),
            Style::default().fg(th.border),
        )),
        rows[1],
    );

    // Each screen resets these as it draws; clear them so a screen without
    // clickable rows does not leave stale targets from the last one behind.
    app.frame.list_rows.clear();
    app.frame.row_targets.clear();
    app.frame.chip_areas.clear();
    app.frame.list_area = Rect::ZERO;

    let content = rows[2];
    match app.screen {
        Screen::IssueList => issue_list::draw(f, app, content),
        Screen::ProjectList => project_list::draw(f, app, content),
        Screen::CycleList => cycle_list::draw(f, app, content),
        Screen::ViewList => view_list::draw(f, app, content),
        Screen::IssueDetail => issue_detail::draw(f, app, &mut cache.detail_markdown, content),
        Screen::ProjectDetail => project_detail::draw(f, app, content),
        Screen::CycleDetail => cycle_detail::draw(f, app, content),
    }

    draw_status_bar(f, app, rows[3]);

    app.frame.popup_area = Rect::ZERO;
    if app.popup != Popup::None {
        popup::draw(f, app);
    } else {
        app.frame.popup_offset = 0;
    }
    if app.input_mode == InputMode::NewIssue {
        new_issue::draw(f, app);
    }
    if app.show_help {
        draw_help(f, app);
    }
    if let Some(err) = app.error_popup.clone() {
        draw_error_popup(f, &err, app);
    }
}

/// Where the content pane is: "Platform › Issues › PF-157", as in Linear's
/// header, with the list position on the right in the detail view.
fn draw_breadcrumb(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let sep = || Span::styled(" \u{203a} ", Style::default().fg(th.muted));
    let dim = |t: String| Span::styled(t, Style::default().fg(th.text_dim));
    let strong =
        |t: String| Span::styled(t, Style::default().fg(th.text).add_modifier(Modifier::BOLD));

    let team = app
        .current_team()
        .map(|t| t.name.clone())
        .unwrap_or_else(|| "No team".into());
    let mut crumbs: Vec<Span> = vec![Span::raw(" ")];
    match app.nav {
        Nav::MyIssues => crumbs.push(strong("My Issues".into())),
        Nav::Views => crumbs.push(strong("Views".into())),
        // The page itself (project, cycle, issue) is added below.
        Nav::Favorite(_) => crumbs.push(dim("Favorites".into())),
        Nav::View(i) => {
            let view = app.custom_views.get(i);
            // A team's view sits under that team, as in Linear.
            if let Some(team) = view.and_then(|v| v.team.as_ref()) {
                if let Some(color) = team.color.as_deref().and_then(crate::api::types::hex_color) {
                    crumbs.push(Span::styled("\u{25cf} ", Style::default().fg(color)));
                }
                crumbs.push(dim(team.name.clone()));
                crumbs.push(sep());
            }
            crumbs.push(dim("Views".into()));
            crumbs.push(sep());
            crumbs.push(strong(view.map(|v| v.name.clone()).unwrap_or_default()));
        }
        Nav::Team(_, section) => {
            if let Some(color) = app
                .current_team()
                .and_then(|t| t.color.as_deref())
                .and_then(crate::api::types::hex_color)
            {
                crumbs.insert(1, Span::styled("\u{25cf} ", Style::default().fg(color)));
            }
            crumbs.push(dim(team));
            crumbs.push(sep());
            let label = match section {
                TeamSection::Issues => "Issues",
                TeamSection::Cycles => "Cycles",
                TeamSection::Projects => "Projects",
                TeamSection::Views => "Views",
            };
            crumbs.push(strong(label.into()));
            if let Some(term) = &app.global_search {
                crumbs.push(sep());
                crumbs.push(Span::styled(
                    format!("Search \u{201c}{term}\u{201d}"),
                    Style::default().fg(th.warning),
                ));
            }
        }
    }
    match app.screen {
        Screen::ProjectDetail => {
            if let Some(p) = &app.current_project {
                crumbs.push(sep());
                crumbs.push(strong(p.name.clone()));
            }
        }
        Screen::CycleDetail => {
            if let Some(c) = &app.current_cycle {
                crumbs.push(sep());
                crumbs.push(strong(cycle_list::cycle_name(c)));
            }
        }
        Screen::IssueDetail => {
            if let Some(issue) = &app.current_issue {
                match app.detail_return {
                    Screen::ProjectDetail => {
                        if let Some(p) = &app.current_project {
                            crumbs.push(sep());
                            crumbs.push(dim(p.name.clone()));
                        }
                    }
                    Screen::CycleDetail => {
                        if let Some(c) = &app.current_cycle {
                            crumbs.push(sep());
                            crumbs.push(dim(cycle_list::cycle_name(c)));
                        }
                    }
                    _ => {}
                }
                crumbs.push(sep());
                crumbs.push(Span::styled(
                    format!("{} ", issue.identifier),
                    Style::default().fg(th.text_dim),
                ));
                crumbs.push(strong(issue.title.clone()));
            }
        }
        _ => {}
    }

    // Right side: spinner, and "6 / 203" in the detail view.
    let mut right: Vec<Span> = Vec::new();
    if app.loading() {
        right.push(Span::styled(
            format!("{} ", app.spinner_symbol()),
            Style::default().fg(th.accent),
        ));
    }
    if app.screen == Screen::IssueDetail
        && let Some((index, total)) = app.detail_position()
    {
        right.push(Span::styled(
            format!("{} / {}", index + 1, total),
            Style::default().fg(th.text_dim),
        ));
        right.push(Span::styled(
            "  J\u{2193} K\u{2191} ",
            Style::default().fg(th.muted),
        ));
    }
    let right_w: usize = right.iter().map(|s| s.width()).sum();

    // Truncate the crumbs from the right so the position readout survives.
    let room = (area.width as usize).saturating_sub(right_w + 1);
    let mut used = 0;
    let mut line: Vec<Span> = Vec::new();
    for span in crumbs {
        let w = span.width();
        if used + w > room {
            let cut = widgets::truncate(&span.content, room.saturating_sub(used));
            used += cut.width();
            line.push(Span::styled(cut, span.style));
            break;
        }
        used += w;
        line.push(span);
    }
    line.push(Span::raw(
        " ".repeat((area.width as usize).saturating_sub(used + right_w)),
    ));
    line.extend(right);
    f.render_widget(Paragraph::new(Line::from(line)), area);
}

/// Key hint: a highlighted key and what it does.
fn hint(key: &str, what: &str, th: &Theme) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!(" {key}"),
            Style::default()
                .fg(th.text_dim)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {what} "), Style::default().fg(th.muted)),
    ]
}

/// The bottom line: the search prompt while searching, the latest status
/// message if there is one, otherwise the keys that matter on this screen.
fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    if app.input_mode == InputMode::Search {
        let mut spans = vec![Span::styled(
            " / ",
            Style::default().fg(th.warning).add_modifier(Modifier::BOLD),
        )];
        spans.extend(input_spans(&app.list().search, th));
        let mut tail = format!("   {} matches", app.visible_issues().len());
        for h in keys::hints(keys::Ctx::Search) {
            tail.push_str(&format!(" \u{00b7} {} {}", h.keys, h.what));
        }
        spans.push(Span::styled(tail, Style::default().fg(th.muted)));
        f.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }
    if let Some(msg) = &app.status_message {
        f.render_widget(
            Paragraph::new(Span::styled(
                format!(" {msg}"),
                Style::default().fg(th.warning),
            )),
            area,
        );
        return;
    }
    if let Some(chord) = app.pending_chord {
        let mut spans = vec![Span::styled(
            format!(" {chord} \u{2026} "),
            Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
        )];
        for h in keys::hints(keys::Ctx::GoTo) {
            spans.extend(hint(h.keys, h.what, th));
        }
        f.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }

    let spans: Vec<Span> = keys::hints(keys::context(app))
        .into_iter()
        .flat_map(|h| hint(h.keys, h.what, th))
        .collect();
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_too_small(f: &mut Frame, app: &mut App) {
    // Nothing on screen is clickable, so nothing from a larger frame may be.
    app.frame.list_rows.clear();
    app.frame.row_targets.clear();
    app.frame.chip_areas.clear();
    app.frame.list_area = Rect::ZERO;
    app.frame.sidebar_area = Rect::ZERO;
    app.frame.popup_area = Rect::ZERO;
    let area = f.area();
    f.render_widget(
        Paragraph::new("Terminal too small")
            .style(Style::default().fg(app.theme.muted))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_error_popup(f: &mut Frame, message: &str, app: &App) {
    let th = &app.theme;
    let lines: Vec<Line> = message.lines().map(|l| Line::from(l.to_string())).collect();
    let height = (lines.len() as u16 + 4).min(15);
    let width = 50.min(f.area().width.saturating_sub(4));
    let area = widgets::centered_rect(width, height, f.area());

    f.render_widget(Clear, area);
    let popup = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.error))
                .title(" Error ")
                .title_style(Style::default().fg(th.error).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(popup, area);

    // Hint at bottom
    let hint_area = Rect {
        x: area.x + 1,
        y: (area.y + area.height).saturating_sub(1),
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

fn draw_help(f: &mut Frame, app: &mut App) {
    let th = &app.theme;
    let section = |text: &str| -> Line<'static> {
        Line::from(vec![Span::styled(
            text.to_string(),
            Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
        )])
    };
    let key_line = |key: &str, desc: &str| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("  {key:<9}  "), Style::default().fg(th.accent)),
            Span::raw(desc.to_string()),
        ])
    };

    let mut help_text = Vec::new();
    for heading in keys::Section::ALL {
        let mut rows = keys::help_rows(heading).peekable();
        if rows.peek().is_none() {
            continue;
        }
        if !help_text.is_empty() {
            help_text.push(Line::from(""));
        }
        help_text.push(section(heading.title()));
        help_text.extend(rows.map(|row| key_line(row.keys, row.text)));
    }

    let total = help_text.len() as u16;
    let height = (total + 2).min(f.area().height.saturating_sub(4));
    let width = 52.min(f.area().width.saturating_sub(4));
    let area = widgets::centered_rect(width, height, f.area());
    let scroll = app
        .help_scroll
        .min(total.saturating_sub(height.saturating_sub(2)));
    // Written back, like the detail view's measurements, so the offset never
    // runs past what the overlay can show.
    app.help_scroll = scroll;

    f.render_widget(Clear, area);
    let help = Paragraph::new(help_text).scroll((scroll, 0)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(th.border))
            .title(" Help (j/k to scroll, any other key closes) ")
            .title_style(Style::default().fg(th.accent).add_modifier(Modifier::BOLD)),
    );
    f.render_widget(help, area);
}

#[cfg(test)]
mod render_tests;

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
