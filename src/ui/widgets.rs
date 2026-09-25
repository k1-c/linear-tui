//! Small building blocks shared by every screen: status and priority glyphs,
//! label chips, people's names, and width-aware truncation.

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::api::types::{Label, Priority, StateType, User, WorkflowState, hex_color};
use crate::config::Theme;

/// The mark for a herdr agent's state, in the colour it deserves: amber when
/// it waits for you, the accent while it works.
pub fn agent_glyph(status: crate::herdr::AgentStatus, theme: &Theme) -> Span<'static> {
    use crate::herdr::AgentStatus;
    let (glyph, color) = match status {
        AgentStatus::Blocked => ("\u{25b2}", theme.warning),
        AgentStatus::Working => ("\u{25cf}", theme.accent),
        AgentStatus::Done => ("\u{2713}", theme.success),
        AgentStatus::Idle => ("\u{25cb}", theme.success),
        AgentStatus::Unknown => ("\u{00b7}", theme.muted),
    };
    Span::styled(glyph, Style::default().fg(color))
}

/// The name Linear shows for a person: their display name, else full name.
pub fn user_name(user: &User) -> &str {
    user.display_name.as_deref().unwrap_or(&user.name)
}

/// Colour a workflow state the way the workspace configured it.
pub fn state_color(state: Option<&WorkflowState>, theme: &Theme) -> Color {
    let Some(state) = state else {
        return theme.muted;
    };
    state
        .color
        .as_deref()
        .and_then(hex_color)
        .or_else(|| state.state_type.map(|t| t.color()))
        .unwrap_or(theme.text_dim)
}

/// The progress-circle glyph for a state, coloured.
pub fn state_glyph(state: Option<&WorkflowState>, theme: &Theme) -> Span<'static> {
    let glyph = state
        .and_then(|s| s.state_type)
        .unwrap_or(StateType::Unknown)
        .glyph();
    Span::styled(glyph, Style::default().fg(state_color(state, theme)))
}

/// The bar-chart glyph for a priority, coloured.
pub fn priority_glyph(priority: Priority, theme: &Theme) -> Span<'static> {
    let style = Style::default().fg(priority.color(theme));
    let style = if priority == Priority::Urgent {
        style.add_modifier(Modifier::BOLD)
    } else {
        style
    };
    Span::styled(priority.glyph(), style)
}

/// A label rendered as Linear draws it: a coloured dot and the name on a chip.
pub fn label_chip(label: &Label, theme: &Theme) -> Vec<Span<'static>> {
    let dot = label
        .color
        .as_deref()
        .and_then(hex_color)
        .unwrap_or(theme.muted);
    vec![
        Span::styled(" \u{25cf}", Style::default().fg(dot).bg(theme.chip_bg)),
        Span::styled(
            format!(" {} ", label.name),
            Style::default().fg(theme.text_dim).bg(theme.chip_bg),
        ),
        Span::raw(" "),
    ]
}

/// Initials for a person, the terminal's stand-in for an avatar: always two
/// cells wide, so the columns after an avatar line up whatever the name.
///
/// Two letters of Latin script fill the badge, but one CJK character already
/// does, so a name written in one gets a single character.
pub fn initials(name: &str) -> String {
    let mut parts = name
        .split(|c: char| c.is_whitespace() || c == '.' || c == '_' || c == '-')
        .filter(|p| !p.is_empty());
    let first = parts.next().and_then(|p| p.chars().next());
    let second = parts.next().and_then(|p| p.chars().next());
    let letters: String = match (first, second) {
        (Some(a), Some(b)) => [a, b].iter().collect(),
        (Some(_), None) => name.chars().take(2).collect(),
        _ => "?".to_string(),
    };
    let mut out = String::new();
    let mut width = 0;
    for c in letters.to_uppercase().chars() {
        let w = c.width().unwrap_or(0);
        if width + w > 2 {
            break;
        }
        out.push(c);
        width += w;
    }
    out.push_str(&" ".repeat(2 - width));
    out
}

/// A person's initials on their colour, as every list and card draws them.
pub fn avatar(name: &str) -> Span<'static> {
    Span::styled(
        initials(name),
        Style::default()
            .fg(Color::Black)
            .bg(person_color(name))
            .add_modifier(Modifier::BOLD),
    )
}

/// An estimate as Linear writes it: `3`, not `3.0`, but `0.5` stays.
pub fn estimate(points: f64) -> String {
    if points.fract() == 0.0 {
        format!("{points:.0}")
    } else {
        format!("{points}")
    }
}

/// The row a third of the way down `area`, where an empty list says why it
/// is empty. Zero-height when `area` is, so nothing is drawn outside it.
pub fn message_row(area: Rect) -> Rect {
    Rect {
        y: area.y + area.height / 3,
        height: area.height.min(1),
        ..area
    }
}

/// A `width` × `height` rectangle centred in `area`.
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);
    horizontal[0]
}

/// A stable colour for a person, so the same name is always the same hue —
/// the property that makes Linear's avatars recognisable at a glance.
pub fn person_color(name: &str) -> Color {
    const PALETTE: [Color; 8] = [
        Color::Rgb(240, 130, 110),
        Color::Rgb(240, 180, 90),
        Color::Rgb(150, 200, 110),
        Color::Rgb(90, 190, 170),
        Color::Rgb(100, 170, 240),
        Color::Rgb(150, 140, 240),
        Color::Rgb(210, 130, 220),
        Color::Rgb(230, 120, 160),
    ];
    let hash = name
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    PALETTE[(hash % PALETTE.len() as u32) as usize]
}

/// A person as an avatar-ish badge followed by their name.
pub fn person(user: Option<&User>, theme: &Theme) -> Vec<Span<'static>> {
    match user {
        Some(user) => {
            let name = user_name(user).to_string();
            vec![
                avatar(&name),
                Span::raw(" "),
                Span::styled(name, Style::default().fg(theme.text)),
            ]
        }
        None => vec![
            Span::styled("\u{25cc} ", Style::default().fg(theme.muted)),
            Span::styled("Unassigned", Style::default().fg(theme.muted)),
        ],
    }
}

/// Cut `text` to at most `width` terminal cells, marking the cut with an
/// ellipsis. Counts cells, not bytes or chars, so Japanese titles — two cells a
/// character — line up with ASCII ones.
pub fn truncate(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w + 1 > width {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('\u{2026}');
    out
}

/// Right-pad `text` with spaces to exactly `width` cells (truncating if longer).
pub fn fit(text: &str, width: usize) -> String {
    let cut = truncate(text, width);
    let pad = width.saturating_sub(cut.width());
    format!("{cut}{}", " ".repeat(pad))
}

/// A short "Sep 9" style date for list rows, falling back to the ISO date.
pub fn short_date(ts: Option<&str>) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let Some(ts) = ts else {
        return String::new();
    };
    let month = ts.get(5..7).and_then(|m| m.parse::<usize>().ok());
    let day = ts.get(8..10).and_then(|d| d.parse::<u32>().ok());
    match (month, day) {
        (Some(m @ 1..=12), Some(d)) => format!("{} {}", MONTHS[m - 1], d),
        _ => ts.get(..10).unwrap_or(ts).to_string(),
    }
}

/// A horizontal progress bar `width` cells wide, eighths-resolution.
pub fn progress_bar(progress: f64, width: usize, fill: Color, theme: &Theme) -> Vec<Span<'static>> {
    const EIGHTHS: [&str; 8] = [
        "", "\u{258f}", "\u{258e}", "\u{258d}", "\u{258c}", "\u{258b}", "\u{258a}", "\u{2589}",
    ];
    let progress = progress.clamp(0.0, 1.0);
    let total = (progress * width as f64 * 8.0).round() as usize;
    let full = total / 8;
    let part = EIGHTHS[total % 8];
    let used = full + usize::from(!part.is_empty());
    vec![
        Span::styled(
            "\u{2588}".repeat(full),
            Style::default().fg(fill).bg(theme.chip_bg),
        ),
        Span::styled(part, Style::default().fg(fill).bg(theme.chip_bg)),
        Span::styled(
            " ".repeat(width.saturating_sub(used)),
            Style::default().bg(theme.chip_bg),
        ),
    ]
}

/// Lay out a row as left spans, flexible padding, and right spans, and paint it
/// with the selection band when it is the cursor row.
pub fn row(
    mut left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    width: usize,
    selected: bool,
    theme: &Theme,
) -> ratatui::text::Line<'static> {
    let used: usize = left.iter().chain(right.iter()).map(|s| s.width()).sum();
    left.insert(
        0,
        if selected {
            Span::styled("\u{258c}", Style::default().fg(theme.accent))
        } else {
            Span::raw(" ")
        },
    );
    left.push(Span::raw(" ".repeat(width.saturating_sub(used + 1))));
    left.extend(right);
    if selected {
        for span in &mut left {
            span.style = span.style.bg(theme.selection_bg);
        }
    }
    ratatui::text::Line::from(left)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_counts_terminal_cells() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 6), "hello\u{2026}");
        // Each of these characters is two cells wide.
        assert_eq!(truncate("日本語テキスト", 7), "日本語\u{2026}");
        assert_eq!(truncate("anything", 0), "");
    }

    #[test]
    fn fit_pads_to_the_exact_width() {
        assert_eq!(fit("ab", 4), "ab  ");
        assert_eq!(fit("日本", 5).width(), 5);
    }

    #[test]
    fn initials_take_one_letter_per_name_part() {
        assert_eq!(initials("Shun Kimura"), "SK");
        assert_eq!(initials("masakazu.ishida"), "MI");
        assert_eq!(initials("k1c"), "K1");
        assert_eq!(initials(""), "? ");
    }

    #[test]
    fn initials_are_always_two_cells_wide() {
        for name in ["Shun Kimura", "木村 駿", "木村", "x", ""] {
            assert_eq!(initials(name).width(), 2, "{name}");
        }
        assert_eq!(initials("木村 駿"), "木");
    }

    #[test]
    fn a_person_always_gets_the_same_colour() {
        assert_eq!(person_color("alice"), person_color("alice"));
    }

    #[test]
    fn a_progress_bar_is_exactly_as_wide_as_asked() {
        let th = Theme::from_name(crate::config::ThemeName::Default);
        for p in [0.0, 0.13, 0.5, 0.99, 1.0, 7.0] {
            let bar = progress_bar(p, 10, Color::Green, &th);
            let w: usize = bar.iter().map(|s| s.width()).sum();
            assert_eq!(w, 10, "progress {p}");
        }
    }

    #[test]
    fn short_dates_read_like_linear() {
        assert_eq!(short_date(Some("2026-09-10T12:00:00.000Z")), "Sep 10");
        assert_eq!(short_date(Some("2026-01-02")), "Jan 2");
        assert_eq!(short_date(None), "");
    }
}
