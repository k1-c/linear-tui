//! How Linear's values look in a terminal: the glyphs and colours the list,
//! the sidebar, and the detail view draw them with.
//!
//! Kept out of `entity`, which knows nothing of drawing.

use ratatui::style::Color;

use crate::config::Theme;
use crate::entity::{Priority, StateType};

/// Parse a Linear `#rrggbb` colour into a terminal colour.
///
/// Linear hands back a hex string for every state, label, project, and team, so
/// honouring it is what makes a list look like the workspace the user knows
/// rather than a generic eight-colour palette.
pub fn hex_color(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() != 6 {
        return None;
    }
    let byte = |r: std::ops::Range<usize>| u8::from_str_radix(hex.get(r)?, 16).ok();
    Some(Color::Rgb(byte(0..2)?, byte(2..4)?, byte(4..6)?))
}

/// How a priority is drawn.
pub trait PriorityLook {
    /// Linear draws priority as a small bar chart that grows with urgency,
    /// and urgent as an exclamation. These block glyphs are the terminal
    /// stand-in.
    fn glyph(&self) -> &'static str;
    fn color(&self, theme: &Theme) -> Color;
}

impl PriorityLook for Priority {
    fn glyph(&self) -> &'static str {
        match self {
            Self::Urgent => "!",
            Self::High => "\u{2587}",
            Self::Medium => "\u{2585}",
            Self::Low => "\u{2583}",
            Self::None => "\u{00b7}",
        }
    }

    fn color(&self, theme: &Theme) -> Color {
        match self {
            Self::Urgent => theme.pri_urgent,
            Self::High => theme.pri_high,
            Self::Medium => theme.pri_medium,
            Self::Low => theme.pri_low,
            Self::None => theme.muted,
        }
    }
}

/// How a workflow state's category is drawn.
pub trait StateLook {
    /// The status glyph Linear draws beside an issue — a progress circle that
    /// fills as the issue moves through the workflow.
    fn glyph(&self) -> &'static str;
    /// The colour for a state Linear gave none.
    fn color(&self) -> Color;
}

impl StateLook for StateType {
    fn glyph(&self) -> &'static str {
        match self {
            Self::Triage => "\u{25c8}",
            Self::Backlog => "\u{25cc}",
            Self::Unstarted => "\u{25cb}",
            Self::Started => "\u{25d0}",
            Self::Completed => "\u{25cf}",
            Self::Cancelled | Self::Duplicate => "\u{2298}",
            Self::Unknown => "\u{25cb}",
        }
    }

    fn color(&self) -> Color {
        match self {
            Self::Started => Color::Yellow,
            Self::Completed => Color::Green,
            Self::Cancelled | Self::Duplicate => Color::DarkGray,
            Self::Backlog => Color::DarkGray,
            Self::Unstarted => Color::White,
            Self::Triage => Color::Magenta,
            Self::Unknown => Color::White,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hex_colour_becomes_an_rgb_colour() {
        assert_eq!(hex_color("#5e6ad2"), Some(Color::Rgb(0x5e, 0x6a, 0xd2)));
        assert_eq!(hex_color("f2c94c"), Some(Color::Rgb(0xf2, 0xc9, 0x4c)));
        assert_eq!(hex_color("#fff"), None);
    }
}
