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
    self, agent_glyph, avatar, label_chip, person, priority_glyph, short_date, state_color,
    state_glyph, truncate, user_name,
};
use crate::config::Theme;
use crate::core::entity::AgentLink;
use crate::core::entity::{Comment, Issue, StateType};
use crate::interface::tui::app::{App, CommentTarget, InputMode};
use crate::interface::tui::look::hex_color;

/// Width of the properties panel when there is room for one.
const PANEL_WIDTH: u16 = 34;
/// Narrowest content pane that still gets a separate properties panel.
const PANEL_MIN_TOTAL: u16 = 96;

pub fn draw(f: &mut Frame, app: &mut App, memo: &mut Memo, area: Rect) {
    let th = app.theme;
    // Borrowed, not cloned: a long thread is a lot of strings to copy on
    // every spinner tick.
    let Some(issue) = app.store.current_issue.as_ref() else {
        return;
    };
    let comment_mode = app.view.input_mode == InputMode::Comment;

    let comment_height = if comment_mode {
        let entered = app.view.comment.value.split('\n').count() as u16;
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
    memo.begin();
    let body = body_lines(issue, body_area.width, !with_panel, &th, memo);
    memo.finish();

    app.frame.detail_lines = u16::try_from(body.len).unwrap_or(u16::MAX);
    app.frame.detail_viewport = body_area.height;
    app.view.detail_scroll = app.view.detail_scroll.min(
        app.frame
            .detail_lines
            .saturating_sub(app.frame.detail_viewport),
    );

    // Only the lines in view go to the widget, already scrolled: the body is
    // pre-wrapped, so this draws exactly what `Paragraph::scroll` would.
    let in_view = body.window(
        memo,
        app.view.detail_scroll as usize,
        body_area.height as usize,
    );
    f.render_widget(Paragraph::new(in_view), body_area);

    if app.frame.detail_lines > app.frame.detail_viewport {
        let mut state = ScrollbarState::new(
            app.frame
                .detail_lines
                .saturating_sub(app.frame.detail_viewport) as usize,
        )
        .position(app.view.detail_scroll as usize);
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
        draw_panel(f, issue, app.agent_for(&issue.identifier), cols[1], &th);
    }

    if comment_mode {
        draw_comment_editor(f, app, rows[1]);
    }
}

fn draw_comment_editor(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let title = match &app.view.comment_target {
        CommentTarget::New => " Leave a comment\u{2026} ".to_string(),
        CommentTarget::Reply(id) => format!(" Reply to {}\u{2026} ", author_of(app, id)),
        CommentTarget::Edit(_) => " Edit your comment ".to_string(),
    };
    let editor = Paragraph::new(super::input_lines(&app.view.comment, th)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(th.accent))
            .title(Span::styled(
                title,
                Style::default().fg(th.text).add_modifier(Modifier::BOLD),
            ))
            .title_bottom(Line::from(Span::styled(
                " Ctrl+Enter send \u{00b7} Enter newline \u{00b7} Esc cancel ",
                Style::default().fg(th.muted),
            ))),
    );
    f.render_widget(editor, area);
}

/// Who wrote a comment of the open issue, for the reply field's title.
fn author_of(app: &App, id: &crate::core::entity::CommentId) -> String {
    app.store
        .current_issue
        .as_ref()
        .and_then(|i| i.comments.as_ref())
        .and_then(|c| c.nodes.iter().find(|c| &c.id == id))
        .and_then(|c| c.user.as_ref())
        .map_or_else(|| "the comment".to_string(), |u| user_name(u).to_string())
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

/// Rendered Markdown kept from one frame to the next.
///
/// The detail view renders the same sources in the same order on every frame
/// — the description, then each comment — and re-parsing and re-wrapping them
/// on each spinner tick is most of what a long thread costs to draw. Each call
/// is matched against the call made at the same position on the last frame,
/// and its lines are reused only when everything they were rendered from is
/// equal. There is nothing to invalidate: any change to what would be drawn
/// is simply a miss, and re-renders that one entry.
#[derive(Debug, Default)]
pub struct Memo {
    entries: Vec<MemoEntry>,
    next: usize,
}

#[derive(Debug)]
struct MemoEntry {
    source: String,
    width: u16,
    gutter: Vec<Span<'static>>,
    /// Whether the lines are closed with a comment card's right edge.
    card: bool,
    theme: Theme,
    lines: Vec<Line<'static>>,
}

impl Memo {
    /// Start a frame: calls are matched in order from the first entry again.
    pub fn begin(&mut self) {
        self.next = 0;
    }

    /// End a frame, dropping whatever the frame did not render.
    pub fn finish(&mut self) {
        self.entries.truncate(self.next);
    }

    /// Render `source` as [`markdown::render`] would — closed with the right
    /// edge of a comment card `width` wide when `card` is set — reusing the
    /// last frame's lines for an identical call. Returns the entry holding
    /// them, for [`Self::lines`].
    fn render(
        &mut self,
        source: &str,
        width: u16,
        gutter: &[Span<'static>],
        card: bool,
        th: &Theme,
    ) -> usize {
        let index = self.next;
        self.next += 1;
        let hit = self.entries.get(index).is_some_and(|e| {
            e.width == width
                && e.card == card
                && e.source == source
                && e.gutter == gutter
                && e.theme == *th
        });
        if !hit {
            let lines = if card {
                // `render` fills `width - 2` including the gutter, which
                // leaves one cell of padding before the right edge.
                let mut lines = markdown::render(source, width.saturating_sub(2), gutter, th);
                for line in &mut lines {
                    close_card_line(line, width as usize, th);
                }
                lines
            } else {
                markdown::render(source, width, gutter, th)
            };
            let entry = MemoEntry {
                source: source.to_owned(),
                width,
                gutter: gutter.to_vec(),
                card,
                theme: *th,
                lines,
            };
            match self.entries.get_mut(index) {
                Some(slot) => *slot = entry,
                None => self.entries.push(entry),
            }
        }
        index
    }

    fn lines(&self, entry: usize) -> &[Line<'static>] {
        &self.entries[entry].lines
    }
}

/// The detail body as one frame lays it out: lines built for this frame,
/// interleaved with runs of Markdown held in the [`Memo`]. Only the lines in
/// view are ever copied out, so a long thread is not duplicated on each draw.
struct Body {
    pieces: Vec<Piece>,
    /// Total height in lines.
    len: usize,
}

enum Piece {
    Line(Line<'static>),
    Memo(usize),
}

impl Body {
    fn new() -> Self {
        Self {
            pieces: Vec::new(),
            len: 0,
        }
    }

    fn push(&mut self, line: Line<'static>) {
        self.pieces.push(Piece::Line(line));
        self.len += 1;
    }

    fn extend(&mut self, lines: impl IntoIterator<Item = Line<'static>>) {
        for line in lines {
            self.push(line);
        }
    }

    fn memo(&mut self, memo: &Memo, entry: usize) {
        self.pieces.push(Piece::Memo(entry));
        self.len += memo.lines(entry).len();
    }

    /// The `take` lines starting at line `skip`.
    fn window(&self, memo: &Memo, skip: usize, take: usize) -> Vec<Line<'static>> {
        self.pieces
            .iter()
            .flat_map(|piece| match piece {
                Piece::Line(line) => std::slice::from_ref(line),
                Piece::Memo(entry) => memo.lines(*entry),
            })
            .skip(skip)
            .take(take)
            .cloned()
            .collect()
    }
}

fn body_lines(issue: &Issue, width: u16, inline_props: bool, th: &Theme, memo: &mut Memo) -> Body {
    let mut out = Body::new();
    out.push(Line::from(""));
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
        Some(desc) => {
            let entry = memo.render(desc, width, &[], false, th);
            out.memo(memo, entry);
        }
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
            comment_threads(&mut out, &comments.nodes, width, th, memo);
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
fn comment_threads(out: &mut Body, comments: &[Comment], width: u16, th: &Theme, memo: &mut Memo) {
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

    for root in roots {
        comment_card(out, root, &replies, width, th, memo);
        out.push(Line::from(""));
    }
}

/// Close a line inside a comment card with the card's right edge.
fn close_card_line(line: &mut Line<'static>, width: usize, th: &Theme) {
    let used = line.width();
    line.spans
        .push(Span::raw(" ".repeat(width.saturating_sub(used + 1))));
    line.spans
        .push(Span::styled("\u{2502}", Style::default().fg(th.border)));
}

fn comment_card(
    out: &mut Body,
    root: &Comment,
    replies: &HashMap<&str, Vec<&Comment>>,
    width: u16,
    th: &Theme,
    memo: &mut Memo,
) {
    let border = Style::default().fg(th.border);
    let w = width as usize;
    // Every line between the top and bottom edges is closed on the right.
    let closed = |mut line: Line<'static>| {
        close_card_line(&mut line, w, th);
        line
    };

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
            out.push(closed(Line::from(vec![
                Span::styled("\u{2502} ", border),
                Span::styled(
                    "\u{2508}".repeat(w.saturating_sub(4)),
                    Style::default().fg(th.border),
                ),
            ])));
        }
        let indent = if *is_reply { "  " } else { "" };
        let mut header = gutter.clone();
        header.push(Span::raw(indent));
        match &comment.user {
            Some(user) => {
                let name = user_name(user).to_string();
                header.push(avatar(&name));
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
        out.push(closed(Line::from(header)));

        let mut body_gutter = gutter.clone();
        body_gutter.push(Span::raw(indent));
        let entry = memo.render(&comment.body, width, &body_gutter, true, th);
        out.memo(memo, entry);
    }

    out.push(Line::from(vec![
        Span::styled("\u{2570}", border),
        Span::styled("\u{2500}".repeat(w.saturating_sub(2)), border),
        Span::styled("\u{256f}", border),
    ]));
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
    estimate.map(|e| format!("{} pts", widgets::estimate(e)))
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

fn draw_panel(f: &mut Frame, issue: &Issue, agent: Option<&AgentLink>, area: Rect, th: &Theme) {
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
    let mut lines = panel_lines(issue, inner.width, th);
    if let Some(agent) = agent {
        lines.extend(agent_lines(agent, inner.width, th));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

/// The herdr agent working on the issue, and where it is.
fn agent_lines(agent: &AgentLink, width: u16, th: &Theme) -> Vec<Line<'static>> {
    let w = width as usize;
    let place = agent.workspace_label.as_deref().unwrap_or(&agent.pane);
    vec![
        Line::from(""),
        heading("Agent", th),
        Line::from(vec![
            Span::raw(" "),
            agent_glyph(agent.status, th),
            Span::styled(
                format!(" {} {}", agent.agent, agent.status.label()),
                Style::default().fg(th.text),
            ),
        ]),
        Line::from(Span::styled(
            format!(
                " {}",
                truncate(&format!("{place} \u{00b7} g w to go"), w.saturating_sub(2))
            ),
            Style::default().fg(th.muted),
        )),
    ]
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
