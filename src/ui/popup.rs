use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState},
};

use super::widgets::{avatar, centered_rect, priority_glyph, state_glyph, user_name};
use crate::api::types::Priority;
use crate::app::{App, FilterKind, Popup};
use crate::config::Theme;

pub fn draw(f: &mut Frame, app: &mut App) {
    match app.popup {
        Popup::TeamSelect => draw_team_select(f, app),
        Popup::Filter(kind) => draw_filter(f, app, kind),
        Popup::StatusChange(_) => draw_status_change(f, app),
        Popup::PriorityChange(_) => draw_priority_change(f, app),
        Popup::AssigneeChange(_) => draw_assignee_change(f, app),
        Popup::None => {}
    }
}

fn render_popup_list(f: &mut Frame, app: &mut App, title: &str, items: Vec<ListItem>, width: u16) {
    let th = app.theme;
    let height = (items.len() as u16 + 2).min(22).min(f.area().height);
    let width = width.min(f.area().width.saturating_sub(2));
    let area = centered_rect(width, height, f.area());

    f.render_widget(Clear, area);
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.border))
                .title(Span::styled(
                    format!(" {title} "),
                    Style::default().fg(th.text).add_modifier(Modifier::BOLD),
                ))
                .title_bottom(Line::from(Span::styled(
                    " 1-9 pick \u{00b7} Enter apply \u{00b7} Esc close ",
                    Style::default().fg(th.muted),
                ))),
        )
        .highlight_style(
            Style::default()
                .bg(th.selection_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("\u{258c}");

    let mut state = ListState::default().with_offset(app.popup_offset);
    state.select(Some(app.popup_index));
    f.render_stateful_widget(list, area, &mut state);
    app.popup_offset = state.offset();
    app.popup_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
}

fn numbered_item(
    index: usize,
    lead: Vec<Span<'static>>,
    text: &str,
    is_current: bool,
    th: &Theme,
) -> ListItem<'static> {
    let mut spans = vec![Span::styled(
        if index < 9 {
            format!("{} ", index + 1)
        } else {
            "  ".to_string()
        },
        Style::default().fg(th.muted),
    )];
    spans.extend(lead);
    spans.push(Span::styled(text.to_string(), Style::default().fg(th.text)));
    if is_current {
        spans.push(Span::styled("  \u{2713}", Style::default().fg(th.success)));
    }
    ListItem::new(Line::from(spans))
}

fn draw_team_select(f: &mut Frame, app: &mut App) {
    let th = app.theme;
    let items: Vec<ListItem> = app
        .teams
        .iter()
        .enumerate()
        .map(|(i, team)| {
            let color = team
                .color
                .as_deref()
                .and_then(crate::api::types::hex_color)
                .unwrap_or(th.accent);
            numbered_item(
                i,
                vec![Span::styled("\u{25cf} ", Style::default().fg(color))],
                &format!("{}  {}", team.name, team.key),
                i == app.selected_team_index,
                &th,
            )
        })
        .collect();
    render_popup_list(f, app, "Switch team", items, 44);
}

fn draw_filter(f: &mut Frame, app: &mut App, kind: FilterKind) {
    let th = app.theme;
    let (title, items) = match kind {
        FilterKind::Status => {
            let mut items = vec![numbered_item(
                0,
                vec![],
                "Any status",
                app.list().filters.status.is_none(),
                &th,
            )];
            for (i, state) in app.filter_states().into_iter().enumerate() {
                let is_current = app
                    .list()
                    .filters
                    .status
                    .as_ref()
                    .is_some_and(|s| s == &state.name);
                items.push(numbered_item(
                    i + 1,
                    vec![state_glyph(Some(state), &th), Span::raw(" ")],
                    &state.name,
                    is_current,
                    &th,
                ));
            }
            if app.popup_loading() {
                items.push(loading_item(app, "Loading statuses"));
            }
            ("Filter \u{2014} status", items)
        }
        FilterKind::Priority => {
            let mut items = vec![numbered_item(
                0,
                vec![],
                "Any priority",
                app.list().filters.priority.is_none(),
                &th,
            )];
            for i in 1..=5 {
                // Index 5 is the "no priority" row: Priority::from_index maps it to None.
                let p = Priority::from_index(i);
                items.push(numbered_item(
                    i,
                    vec![priority_glyph(p, &th), Span::raw(" ")],
                    p.label(),
                    app.list().filters.priority == Some(p),
                    &th,
                ));
            }
            ("Filter \u{2014} priority", items)
        }
    };
    render_popup_list(f, app, title, items, 38);
}

fn draw_status_change(f: &mut Frame, app: &mut App) {
    let th = app.theme;
    let current_state_id = app
        .popup_issue()
        .and_then(|i| i.state.as_ref())
        .map(|s| s.id.clone());

    let mut items: Vec<ListItem> = app
        .popup_states()
        .iter()
        .enumerate()
        .map(|(i, state)| {
            numbered_item(
                i,
                vec![state_glyph(Some(state), &th), Span::raw(" ")],
                &state.name,
                current_state_id.as_ref() == Some(&state.id),
                &th,
            )
        })
        .collect();
    if app.popup_loading() {
        items.push(loading_item(app, "Loading statuses"));
    }
    render_popup_list(f, app, "Change status", items, 38);
}

fn draw_priority_change(f: &mut Frame, app: &mut App) {
    let th = app.theme;
    let current_pri = app.popup_issue().map(|i| i.priority);
    let items: Vec<ListItem> = Priority::ALL
        .iter()
        .enumerate()
        .map(|(i, pri)| {
            numbered_item(
                i,
                vec![priority_glyph(*pri, &th), Span::raw(" ")],
                pri.label(),
                current_pri == Some(*pri),
                &th,
            )
        })
        .collect();
    render_popup_list(f, app, "Change priority", items, 34);
}

fn draw_assignee_change(f: &mut Frame, app: &mut App) {
    let th = app.theme;
    let current_assignee_id = app
        .popup_issue()
        .and_then(|i| i.assignee.as_ref())
        .map(|a| a.id.clone());

    let mut items = vec![numbered_item(
        0,
        vec![Span::styled("\u{25cc}  ", Style::default().fg(th.muted))],
        "No assignee",
        current_assignee_id.is_none(),
        &th,
    )];
    for (i, member) in app.popup_members().iter().enumerate() {
        let name = user_name(member).to_string();
        items.push(numbered_item(
            i + 1,
            vec![avatar(&name), Span::raw(" ")],
            &name,
            current_assignee_id.as_ref() == Some(&member.id),
            &th,
        ));
    }
    if app.popup_loading() {
        items.push(loading_item(app, "Loading members"));
    }
    render_popup_list(f, app, "Assign to", items, 44);
}

/// A trailing row shown while the issue's team is still being fetched. It
/// sits past `App::popup_list_len`, so it can be neither picked nor clicked.
fn loading_item(app: &App, text: &str) -> ListItem<'static> {
    ListItem::new(Line::from(Span::styled(
        format!("  {} {text}\u{2026}", app.spinner_symbol()),
        Style::default().fg(app.theme.muted),
    )))
}
