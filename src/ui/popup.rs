use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
};

use crate::app::{App, FilterKind, Popup};
use crate::config::Theme;

pub fn draw(f: &mut Frame, app: &App) {
    match app.popup {
        Popup::TeamSelect => draw_team_select(f, app),
        Popup::Filter => draw_filter(f, app),
        Popup::StatusChange => draw_status_change(f, app),
        Popup::PriorityChange => draw_priority_change(f, app),
        Popup::AssigneeChange => draw_assignee_change(f, app),
        Popup::None => {}
    }
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

fn render_popup_list(
    f: &mut Frame,
    title: &str,
    items: Vec<ListItem>,
    popup_index: usize,
    width: u16,
    th: &Theme,
) {
    let height = (items.len() as u16 + 2).min(20);
    let area = centered_rect(width, height, f.area());

    f.render_widget(Clear, area);
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title.to_string())
                .title_style(Style::default().fg(th.accent)),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .fg(th.accent),
        )
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(popup_index));
    f.render_stateful_widget(list, area, &mut state);
}

fn numbered_item(index: usize, text: &str, is_current: bool, th: &Theme) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::styled(format!("{}. ", index + 1), Style::default().fg(th.muted)),
        Span::raw(text.to_string()),
        if is_current {
            Span::styled(" *", Style::default().fg(th.success))
        } else {
            Span::raw("")
        },
    ]))
}

fn draw_team_select(f: &mut Frame, app: &App) {
    let th = &app.theme;
    let items: Vec<ListItem> = app
        .teams
        .iter()
        .enumerate()
        .map(|(i, team)| {
            numbered_item(
                i,
                &format!("{} [{}]", team.name, team.key),
                i == app.selected_team_index,
                th,
            )
        })
        .collect();
    render_popup_list(f, " Select Team ", items, app.popup_index, 40, th);
}

fn draw_filter(f: &mut Frame, app: &App) {
    let th = &app.theme;
    let (title, items) = match app.filter_kind {
        FilterKind::Status => {
            let mut items = vec![numbered_item(0, "All", app.filters.status.is_none(), th)];
            for (i, state) in app.workflow_states.iter().enumerate() {
                let is_current = app
                    .filters
                    .status
                    .as_ref()
                    .is_some_and(|s| s == &state.name);
                items.push(numbered_item(i + 1, &state.name, is_current, th));
            }
            (" Filter: Status ", items)
        }
        FilterKind::Priority => {
            let labels = ["All", "Urgent", "High", "Medium", "Low", "None"];
            let items: Vec<ListItem> = labels
                .iter()
                .enumerate()
                .map(|(i, label)| {
                    let is_current = if i == 0 {
                        app.filters.priority.is_none()
                    } else {
                        app.filters.priority == Some(crate::api::types::Priority::from_index(i))
                    };
                    numbered_item(i, label, is_current, th)
                })
                .collect();
            (" Filter: Priority ", items)
        }
    };
    render_popup_list(f, title, items, app.popup_index, 35, th);
}

fn draw_status_change(f: &mut Frame, app: &App) {
    let th = &app.theme;
    let current_state_id = app
        .focused_issue()
        .and_then(|i| i.state.as_ref())
        .map(|s| s.id.as_str());

    let items: Vec<ListItem> = app
        .workflow_states
        .iter()
        .enumerate()
        .map(|(i, state)| {
            numbered_item(
                i,
                &state.name,
                current_state_id == Some(state.id.as_str()),
                th,
            )
        })
        .collect();
    render_popup_list(f, " Change Status ", items, app.popup_index, 35, th);
}

fn draw_priority_change(f: &mut Frame, app: &App) {
    use crate::api::types::Priority;
    let th = &app.theme;
    let current_pri = app.focused_issue().map(|i| i.priority);
    let priorities = [
        Priority::None,
        Priority::Urgent,
        Priority::High,
        Priority::Medium,
        Priority::Low,
    ];
    let items: Vec<ListItem> = priorities
        .iter()
        .enumerate()
        .map(|(i, pri)| numbered_item(i, pri.label(), current_pri == Some(*pri), th))
        .collect();
    render_popup_list(f, " Change Priority ", items, app.popup_index, 30, th);
}

fn draw_assignee_change(f: &mut Frame, app: &App) {
    let th = &app.theme;
    let current_assignee_id = app
        .focused_issue()
        .and_then(|i| i.assignee.as_ref())
        .map(|a| a.id.as_str());

    let mut items = vec![numbered_item(
        0,
        "Unassign",
        current_assignee_id.is_none(),
        th,
    )];
    for (i, member) in app.team_members.iter().enumerate() {
        let display = member
            .display_name
            .as_deref()
            .unwrap_or(member.name.as_str());
        items.push(numbered_item(
            i + 1,
            display,
            current_assignee_id == Some(member.id.as_str()),
            th,
        ));
    }
    render_popup_list(f, " Change Assignee ", items, app.popup_index, 40, th);
}
