//! Key notation: how an agent — or a script, or a test — writes the keys it
//! presses (`linear-tui tui press`).
//!
//! Whitespace separates tokens. A token is either a key in angle brackets —
//! `<Enter>`, `<Esc>`, `<Tab>`, `<S-Tab>`, `<C-k>`, `<A-Enter>`, `<F5>` — or
//! plain characters, each pressed in turn: `gm` is `g` then `m`, the same as
//! `g m`. `<Space>` and `<lt>` stand for a space and `<`. Text with spaces in
//! it is typed rather than pressed (`linear-tui tui type`).

use anyhow::{Result, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Parse a key sequence.
pub fn parse(keys: &str) -> Result<Vec<KeyEvent>> {
    let mut events = Vec::new();
    for token in keys.split_whitespace() {
        if let Some(name) = token.strip_prefix('<').and_then(|t| t.strip_suffix('>')) {
            events.push(named(name)?);
        } else {
            events.extend(token.chars().map(char_key));
        }
    }
    Ok(events)
}

/// A typed character, reported the way a terminal does: capitals carry Shift.
pub fn char_key(c: char) -> KeyEvent {
    let mods = if c.is_ascii_uppercase() {
        KeyModifiers::SHIFT
    } else {
        KeyModifiers::NONE
    };
    KeyEvent::new(KeyCode::Char(c), mods)
}

fn named(name: &str) -> Result<KeyEvent> {
    let mut mods = KeyModifiers::NONE;
    let mut rest = name;
    while let Some((prefix, tail)) = rest.split_once('-').filter(|(_, tail)| !tail.is_empty()) {
        match prefix {
            "C" => mods |= KeyModifiers::CONTROL,
            "A" | "M" => mods |= KeyModifiers::ALT,
            "S" => mods |= KeyModifiers::SHIFT,
            _ => break,
        }
        rest = tail;
    }
    let code = match rest.to_ascii_lowercase().as_str() {
        "enter" | "cr" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "tab" if mods.contains(KeyModifiers::SHIFT) => {
            mods.remove(KeyModifiers::SHIFT);
            KeyCode::BackTab
        }
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "space" => KeyCode::Char(' '),
        "lt" => KeyCode::Char('<'),
        "bs" | "backspace" => KeyCode::Backspace,
        "del" | "delete" => KeyCode::Delete,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" => KeyCode::PageDown,
        f if f.starts_with('f') && f.len() > 1 => match f[1..].parse() {
            Ok(n) => KeyCode::F(n),
            Err(_) => bail!("unknown key <{name}>"),
        },
        _ => {
            let mut chars = rest.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => KeyCode::Char(c),
                _ => bail!("unknown key <{name}>"),
            }
        }
    };
    Ok(KeyEvent::new(code, mods))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn plain_characters_are_pressed_one_by_one() {
        assert_eq!(
            parse("gm J").unwrap(),
            [
                key(KeyCode::Char('g'), KeyModifiers::NONE),
                key(KeyCode::Char('m'), KeyModifiers::NONE),
                key(KeyCode::Char('J'), KeyModifiers::SHIFT),
            ]
        );
    }

    #[test]
    fn named_keys_take_modifiers() {
        assert_eq!(
            parse("<C-k> <S-Tab> <A-Enter> <F5> <lt> <Space>").unwrap(),
            [
                key(KeyCode::Char('k'), KeyModifiers::CONTROL),
                key(KeyCode::BackTab, KeyModifiers::NONE),
                key(KeyCode::Enter, KeyModifiers::ALT),
                key(KeyCode::F(5), KeyModifiers::NONE),
                key(KeyCode::Char('<'), KeyModifiers::NONE),
                key(KeyCode::Char(' '), KeyModifiers::NONE),
            ]
        );
    }

    #[test]
    fn a_ctrl_dash_is_the_dash() {
        assert_eq!(
            parse("<C-->").unwrap(),
            [key(KeyCode::Char('-'), KeyModifiers::CONTROL)]
        );
    }

    #[test]
    fn an_unknown_name_is_an_error() {
        assert!(parse("<Nope>").is_err());
        assert!(parse("<Fx>").is_err());
    }
}
