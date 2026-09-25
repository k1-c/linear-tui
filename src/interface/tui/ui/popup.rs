use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use super::widgets::{avatar, centered_rect, priority_glyph, state_glyph, user_name};
use crate::config::Theme;
use crate::core::entity::Priority;
use crate::interface::tui::app::{App, FilterKind, Popup};
use crate::interface::tui::grouping::GroupBy;

pub fn draw(f: &mut Frame, app: &mut App) {
    match app.view.popup {
        Popup::TeamSelect => draw_team_select(f, app),
        Popup::WorkspaceSelect => draw_workspace_select(f, app),
        Popup::Filter(kind) => draw_filter(f, app, kind),
        Popup::StatusChange(_) => draw_status_change(f, app),
        Popup::PriorityChange(_) => draw_priority_change(f, app),
        Popup::AssigneeChange(_) => draw_assignee_change(f, app),
        Popup::GroupBy => draw_group_by(f, app),
        Popup::Palette => super::palette::draw(f, app),
        Popup::None => {}
    }
}

/// Draw a pick-one popup: `items` is its full list, one per row of
/// `App::popup_rows`'s index space; only the rows the query leaves are shown,
/// best match first. `trailing` is a row past them that cannot be picked,
/// such as a loading line.
fn render_popup_list(
    f: &mut Frame,
    app: &mut App,
    title: &str,
    items: Vec<ListItem<'static>>,
    trailing: Option<ListItem<'static>>,
    width: u16,
) {
    let th = app.theme;
    let query = app.view.popup_query.clone();
    let mut shown: Vec<ListItem> = app
        .popup_rows()
        .into_iter()
        .filter_map(|i| items.get(i).cloned())
        .collect();
    if shown.is_empty() && trailing.is_none() {
        shown.push(ListItem::new(Span::styled(
            "  No matches",
            Style::default().fg(th.muted),
        )));
    }
    shown.extend(trailing);

    // A query line above the rows once something is typed.
    let query_rows = u16::from(!query.is_empty());
    let height = (shown.len() as u16 + 2 + query_rows)
        .min(22)
        .min(f.area().height);
    let width = width.min(f.area().width.saturating_sub(2));
    let area = centered_rect(width, height, f.area());

    f.render_widget(Clear, area);
    let hint = if query.is_empty() {
        " 1-9 pick \u{00b7} type to filter \u{00b7} Esc "
    } else {
        " Enter apply \u{00b7} Esc close "
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.border))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(th.text).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Line::from(Span::styled(
            hint,
            Style::default().fg(th.muted),
        )));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if query_rows > 0 && inner.height > 0 {
        let mut line = vec![Span::styled("\u{203a} ", Style::default().fg(th.accent))];
        line.extend(super::input_spans(&query, &th));
        f.render_widget(
            Paragraph::new(Line::from(line)),
            Rect { height: 1, ..inner },
        );
    }
    let list_area = Rect {
        y: inner.y + query_rows,
        height: inner.height.saturating_sub(query_rows),
        ..inner
    };

    let list = List::new(shown)
        .highlight_style(
            Style::default()
                .bg(th.selection_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("\u{258c}");
    let mut state = ListState::default().with_offset(app.frame.popup_offset);
    state.select(Some(app.view.popup_index));
    f.render_stateful_widget(list, list_area, &mut state);
    app.frame.popup_offset = state.offset();
    app.frame.popup_area = list_area;
}

/// A popup row led by its digit shortcut. `index` is `None` once a query
/// narrows the list, since the digits are then typed as text.
fn numbered_item(
    index: Option<usize>,
    lead: Vec<Span<'static>>,
    text: &str,
    is_current: bool,
    th: &Theme,
) -> ListItem<'static> {
    let mut spans = vec![Span::styled(
        match index {
            Some(i) if i < 9 => format!("{} ", i + 1),
            _ => "  ".to_string(),
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
    let num = numbering(app);
    let items: Vec<ListItem> = app
        .store
        .teams
        .iter()
        .enumerate()
        .map(|(i, team)| {
            let color = team
                .color
                .as_deref()
                .and_then(crate::interface::tui::look::hex_color)
                .unwrap_or(th.accent);
            numbered_item(
                num(i),
                vec![Span::styled("\u{25cf} ", Style::default().fg(color))],
                &format!("{}  {}", team.name, team.key),
                i == app.nav.team,
                &th,
            )
        })
        .collect();
    render_popup_list(f, app, "Switch team", items, None, 44);
}

fn draw_workspace_select(f: &mut Frame, app: &mut App) {
    let th = app.theme;
    let num = numbering(app);
    let items: Vec<ListItem> = app
        .workspaces
        .iter()
        .enumerate()
        .map(|(i, w)| {
            numbered_item(
                num(i),
                vec![],
                &format!("{}  {}", w.name, w.url_key),
                w.current,
                &th,
            )
        })
        .collect();
    render_popup_list(f, app, "Switch workspace", items, None, 44);
}

fn draw_filter(f: &mut Frame, app: &mut App, kind: FilterKind) {
    let th = app.theme;
    let num = numbering(app);
    let loading = app
        .popup_loading()
        .then(|| loading_item(app, "Loading statuses"));
    let (title, items) = match kind {
        FilterKind::Status => {
            let mut items = vec![numbered_item(
                num(0),
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
                    num(i + 1),
                    vec![state_glyph(Some(state), &th), Span::raw(" ")],
                    &state.name,
                    is_current,
                    &th,
                ));
            }
            ("Filter \u{2014} status", items)
        }
        FilterKind::Priority => {
            let mut items = vec![numbered_item(
                num(0),
                vec![],
                "Any priority",
                app.list().filters.priority.is_none(),
                &th,
            )];
            for i in 1..=5 {
                // Index 5 is the "no priority" row: Priority::from_index maps it to None.
                let p = Priority::from_index(i);
                items.push(numbered_item(
                    num(i),
                    vec![priority_glyph(p, &th), Span::raw(" ")],
                    p.label(),
                    app.list().filters.priority == Some(p),
                    &th,
                ));
            }
            ("Filter \u{2014} priority", items)
        }
    };
    render_popup_list(f, app, title, items, loading, 38);
}

fn draw_status_change(f: &mut Frame, app: &mut App) {
    let th = app.theme;
    let num = numbering(app);
    let current_state_id = app
        .popup_issue()
        .and_then(|i| i.state.as_ref())
        .map(|s| s.id.clone());

    let items: Vec<ListItem> = app
        .popup_states()
        .iter()
        .enumerate()
        .map(|(i, state)| {
            numbered_item(
                num(i),
                vec![state_glyph(Some(state), &th), Span::raw(" ")],
                &state.name,
                current_state_id.as_ref() == Some(&state.id),
                &th,
            )
        })
        .collect();
    let loading = app
        .popup_loading()
        .then(|| loading_item(app, "Loading statuses"));
    render_popup_list(f, app, "Change status", items, loading, 38);
}

fn draw_priority_change(f: &mut Frame, app: &mut App) {
    let th = app.theme;
    let num = numbering(app);
    let current_pri = app.popup_issue().map(|i| i.priority);
    let items: Vec<ListItem> = Priority::ALL
        .iter()
        .enumerate()
        .map(|(i, pri)| {
            numbered_item(
                num(i),
                vec![priority_glyph(*pri, &th), Span::raw(" ")],
                pri.label(),
                current_pri == Some(*pri),
                &th,
            )
        })
        .collect();
    render_popup_list(f, app, "Change priority", items, None, 34);
}

fn draw_assignee_change(f: &mut Frame, app: &mut App) {
    let th = app.theme;
    let num = numbering(app);
    let current_assignee_id = app
        .popup_issue()
        .and_then(|i| i.assignee.as_ref())
        .map(|a| a.id.clone());

    let mut items = vec![numbered_item(
        num(0),
        vec![Span::styled("\u{25cc}  ", Style::default().fg(th.muted))],
        "No assignee",
        current_assignee_id.is_none(),
        &th,
    )];
    for (i, member) in app.popup_members().iter().enumerate() {
        let name = user_name(member).to_string();
        items.push(numbered_item(
            num(i + 1),
            vec![avatar(&name), Span::raw(" ")],
            &name,
            current_assignee_id.as_ref() == Some(&member.id),
            &th,
        ));
    }
    let loading = app
        .popup_loading()
        .then(|| loading_item(app, "Loading members"));
    render_popup_list(f, app, "Assign to", items, loading, 44);
}

/// Digit shortcuts work only while nothing is typed; this shows them then.
fn numbering(app: &App) -> impl Fn(usize) -> Option<usize> + use<> {
    let typing = !app.view.popup_query.is_empty();
    move |i| (!typing).then_some(i)
}

fn draw_group_by(f: &mut Frame, app: &mut App) {
    let th = app.theme;
    let num = numbering(app);
    let items: Vec<ListItem> = GroupBy::ALL
        .iter()
        .enumerate()
        .map(|(i, g)| numbered_item(num(i), vec![], g.label(), *g == app.view.group_by, &th))
        .collect();
    render_popup_list(f, app, "Group by", items, None, 30);
}

/// A trailing row shown while the issue's team is still being fetched. It
/// sits past `App::popup_list_len`, so it can be neither picked nor clicked.
fn loading_item(app: &App, text: &str) -> ListItem<'static> {
    ListItem::new(Line::from(Span::styled(
        format!("  {} {text}\u{2026}", app.spinner_symbol()),
        Style::default().fg(app.theme.muted),
    )))
}
