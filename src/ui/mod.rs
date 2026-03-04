pub mod cycle_detail;
pub mod cycle_list;
pub mod issue_detail;
pub mod issue_list;
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

use crate::app::{App, Popup, Screen, Tab};

/// Format an ISO date string to just the date portion (YYYY-MM-DD).
pub fn format_date(date_str: Option<&str>) -> String {
    date_str
        .and_then(|s| s.get(..10))
        .unwrap_or("-")
        .to_string()
}

pub fn draw(f: &mut Frame, app: &App) {
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
    if let Some(err) = &app.error_popup {
        draw_error_popup(f, err, app);
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
            Span::styled(format!("  {key:<7}  "), Style::default().fg(th.warning)),
            Span::raw(desc.to_string()),
        ])
    };

    let help_text = vec![
        section("Navigation"),
        key_line("j/k", "Move cursor up/down"),
        key_line("g/G", "Go to first/last item"),
        key_line("Enter", "Open detail / drill into"),
        key_line("Esc/q", "Back / quit"),
        Line::from(""),
        section("Tabs"),
        key_line("1", "Issues"),
        key_line("2", "My Issues"),
        key_line("3", "Projects"),
        key_line("4", "Cycles"),
        Line::from(""),
        section("Actions"),
        key_line("s", "Change status"),
        key_line("p", "Change priority"),
        key_line("a", "Change assignee"),
        key_line("c", "Add comment"),
        Line::from(""),
        section("Other"),
        key_line("t", "Switch team"),
        key_line("f/F", "Filter / clear filters"),
        key_line("/", "Search issues"),
        key_line("r", "Reload data"),
        key_line("?", "Toggle this help"),
    ];

    let height = (help_text.len() as u16 + 2).min(f.area().height.saturating_sub(4));
    let width = 40.min(f.area().width.saturating_sub(4));
    let area = centered_rect(width, height, f.area());

    f.render_widget(Clear, area);
    let help = Paragraph::new(help_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help (press any key to close) ")
            .title_style(Style::default().fg(th.accent).add_modifier(Modifier::BOLD)),
    );
    f.render_widget(help, area);
}
