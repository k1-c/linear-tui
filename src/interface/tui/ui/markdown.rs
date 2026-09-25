//! Markdown to terminal lines, for issue descriptions and comments.
//!
//! Linear stores both as Markdown and renders them richly: bold, inline code,
//! nested lists, headings, quotes, fenced code. Printing the raw source leaves
//! `**` and backticks all over a Japanese paragraph, which is most of why a
//! description used to be hard to read here.
//!
//! The renderer wraps text itself rather than leaving it to
//! [`ratatui::widgets::Wrap`], because a wrapped line has to keep its
//! indentation — the continuation of a bullet, or of a comment inside its
//! gutter, must stay under its first line. It also means the caller knows the
//! exact rendered height, so scrolling clamps to the real content.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::config::Theme;

/// Render `source` into lines no wider than `width` cells.
///
/// `gutter` is prepended to every output line — the comment thread uses it to
/// draw its left rule — and counts against `width`.
pub fn render(
    source: &str,
    width: u16,
    gutter: &[Span<'static>],
    th: &Theme,
) -> Vec<Line<'static>> {
    let gutter_width: usize = gutter.iter().map(|s| s.width()).sum();
    let width = (width as usize).saturating_sub(gutter_width).max(8);
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut in_code = false;
    let mut blank_run = 0;

    let push = |out: &mut Vec<Line<'static>>, mut spans: Vec<Span<'static>>| {
        let mut line = gutter.to_vec();
        line.append(&mut spans);
        out.push(Line::from(line));
    };

    for raw in source.lines() {
        let trimmed = raw.trim_start();
        // Measured in cells, not bytes: an ideographic space is three bytes
        // but only two cells.
        let indent = raw[..raw.len() - trimmed.len()].width();

        // Fenced code: shown verbatim on a tinted band, never inline-parsed.
        if trimmed.starts_with("```") {
            in_code = !in_code;
            let lang = trimmed.trim_start_matches('`').trim();
            if in_code && !lang.is_empty() {
                push(
                    &mut out,
                    vec![Span::styled(
                        format!(" {lang} "),
                        Style::default().fg(th.muted).bg(th.code_bg),
                    )],
                );
            }
            continue;
        }
        if in_code {
            let style = Style::default().fg(th.text_dim).bg(th.code_bg);
            for chunk in hard_wrap(raw, width.saturating_sub(2)) {
                let pad = width.saturating_sub(chunk.width() + 1);
                push(
                    &mut out,
                    vec![
                        Span::styled(" ", style),
                        Span::styled(chunk, style),
                        Span::styled(" ".repeat(pad), style),
                    ],
                );
            }
            continue;
        }

        if trimmed.is_empty() {
            // Collapse runs of blank lines: the editor leaves them lying
            // around, and in a narrow pane each one costs a whole row.
            blank_run += 1;
            if blank_run == 1 && !out.is_empty() {
                push(&mut out, vec![]);
            }
            continue;
        }
        blank_run = 0;

        // Thematic break.
        if matches!(trimmed, "---" | "***" | "___") {
            push(
                &mut out,
                vec![Span::styled(
                    "\u{2500}".repeat(width),
                    Style::default().fg(th.border),
                )],
            );
            continue;
        }

        // Headings.
        if let Some(level) = heading_level(trimmed) {
            let text = trimmed[level..].trim();
            let style = match level {
                1 | 2 => Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
                _ => Style::default().fg(th.text).add_modifier(Modifier::BOLD),
            };
            let segments = inline(text, style, th);
            for line in wrap(segments, width, vec![], vec![]) {
                push(&mut out, line);
            }
            continue;
        }

        // Block quote.
        if let Some(rest) = trimmed.strip_prefix('>') {
            let bar = Span::styled("\u{258e} ", Style::default().fg(th.muted));
            let style = Style::default()
                .fg(th.text_dim)
                .add_modifier(Modifier::ITALIC);
            let segments = inline(rest.trim_start(), style, th);
            for line in wrap(segments, width, vec![bar.clone()], vec![bar.clone()]) {
                push(&mut out, line);
            }
            continue;
        }

        // List items, bulleted, numbered, or task.
        if let Some((marker, rest)) = list_item(trimmed, th) {
            let pad = " ".repeat(indent.min(12));
            let marker_width = marker.iter().map(|s| s.width()).sum::<usize>();
            let mut first = vec![Span::raw(pad.clone())];
            first.extend(marker);
            let cont = vec![Span::raw(format!("{pad}{}", " ".repeat(marker_width)))];
            let segments = inline(rest, Style::default().fg(th.text), th);
            for line in wrap(segments, width, first, cont) {
                push(&mut out, line);
            }
            continue;
        }

        // Plain paragraph line.
        let segments = inline(trimmed, Style::default().fg(th.text), th);
        for line in wrap(segments, width, vec![], vec![]) {
            push(&mut out, line);
        }
    }

    // A trailing blank from the source adds nothing but a wasted row.
    while out.last().is_some_and(|l| l.spans.len() == gutter.len()) {
        out.pop();
    }
    out
}

fn heading_level(line: &str) -> Option<usize> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    (1..=6)
        .contains(&hashes)
        .then_some(hashes)
        .filter(|&n| line[n..].starts_with(' '))
}

/// Recognise a list marker, returning the styled marker and the item text.
fn list_item<'a>(line: &'a str, th: &Theme) -> Option<(Vec<Span<'static>>, &'a str)> {
    let bullet = Style::default().fg(th.secondary);
    for task in ["- [ ] ", "* [ ] "] {
        if let Some(rest) = line.strip_prefix(task) {
            return Some((vec![Span::styled("\u{2610} ", bullet)], rest));
        }
    }
    for task in ["- [x] ", "* [x] ", "- [X] ", "* [X] "] {
        if let Some(rest) = line.strip_prefix(task) {
            return Some((
                vec![Span::styled("\u{2611} ", Style::default().fg(th.success))],
                rest,
            ));
        }
    }
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            return Some((vec![Span::styled("\u{2022} ", bullet)], rest));
        }
    }
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits <= 3 {
        let after = &line[digits..];
        if let Some(rest) = after
            .strip_prefix(". ")
            .or_else(|| after.strip_prefix(") "))
        {
            return Some((
                vec![Span::styled(format!("{}. ", &line[..digits]), bullet)],
                rest,
            ));
        }
    }
    None
}

/// Split one line of Markdown into styled runs: bold, italic, strikethrough,
/// inline code, links, and images.
pub fn inline(text: &str, base: Style, th: &Theme) -> Vec<(String, Style)> {
    let mut out: Vec<(String, Style)> = Vec::new();
    let mut buf = String::new();
    let (mut bold, mut italic, mut strike) = (false, false, false);
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    let style_of = |bold: bool, italic: bool, strike: bool| {
        let mut s = base;
        if bold {
            s = s.add_modifier(Modifier::BOLD);
        }
        if italic {
            s = s.add_modifier(Modifier::ITALIC);
        }
        if strike {
            s = s.add_modifier(Modifier::CROSSED_OUT);
        }
        s
    };
    let flush = |buf: &mut String, out: &mut Vec<(String, Style)>, style: Style| {
        if !buf.is_empty() {
            out.push((std::mem::take(buf), style));
        }
    };

    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let prev = if i == 0 { None } else { Some(chars[i - 1]) };

        // Inline code is atomic: nothing inside it is Markdown.
        if c == '`'
            && let Some(end) = chars[i + 1..].iter().position(|&x| x == '`')
        {
            flush(&mut buf, &mut out, style_of(bold, italic, strike));
            let code: String = chars[i + 1..i + 1 + end].iter().collect();
            out.push((code, Style::default().fg(th.secondary).bg(th.code_bg)));
            i += end + 2;
            continue;
        }

        // Images and links: keep the text, drop the URL.
        if (c == '[' || (c == '!' && next == Some('[')))
            && let Some((label, consumed)) = link_at(&chars[i..])
        {
            flush(&mut buf, &mut out, style_of(bold, italic, strike));
            if c == '!' {
                out.push((
                    format!(
                        "[image{}]",
                        if label.is_empty() {
                            String::new()
                        } else {
                            format!(": {label}")
                        }
                    ),
                    Style::default().fg(th.muted),
                ));
            } else {
                out.push((
                    label,
                    style_of(bold, italic, strike)
                        .fg(th.accent)
                        .add_modifier(Modifier::UNDERLINED),
                ));
            }
            i += consumed;
            continue;
        }

        // Bold (** or __) and strikethrough (~~).
        if (c == '*' && next == Some('*')) || (c == '_' && next == Some('_') && !is_word(prev)) {
            flush(&mut buf, &mut out, style_of(bold, italic, strike));
            bold = !bold;
            i += 2;
            continue;
        }
        if c == '~' && next == Some('~') {
            flush(&mut buf, &mut out, style_of(bold, italic, strike));
            strike = !strike;
            i += 2;
            continue;
        }

        // Italic. `_` only toggles at a word boundary, so snake_case survives;
        // a `*` needs a matching close ahead, so "5 * 3" stays arithmetic.
        let italic_star = c == '*'
            && (italic
                || (chars[i + 1..].contains(&'*') && next.is_some_and(|n| !n.is_whitespace())));
        let italic_underscore = c == '_'
            && (if italic {
                !is_word(next)
            } else {
                !is_word(prev) && next.is_some_and(|n| !n.is_whitespace())
            });
        if italic_star || italic_underscore {
            flush(&mut buf, &mut out, style_of(bold, italic, strike));
            italic = !italic;
            i += 1;
            continue;
        }

        buf.push(c);
        i += 1;
    }
    flush(&mut buf, &mut out, style_of(bold, italic, strike));
    out
}

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric())
}

/// Parse `[label](url)` (or `![label](url)`) at the start of `chars`, returning
/// the label and how many chars the whole construct spans.
fn link_at(chars: &[char]) -> Option<(String, usize)> {
    let start = if chars.first() == Some(&'!') { 1 } else { 0 };
    if chars.get(start) != Some(&'[') {
        return None;
    }
    let close = chars[start + 1..].iter().position(|&c| c == ']')? + start + 1;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let end = chars[close + 2..].iter().position(|&c| c == ')')? + close + 2;
    let label: String = chars[start + 1..close].iter().collect();
    Some((label, end + 1))
}

/// Break a string into pieces of at most `width` cells, by character.
fn hard_wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = vec![String::new()];
    let mut used = 0;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w > width {
            out.push(String::new());
            used = 0;
        }
        out.last_mut().unwrap().push(c);
        used += w;
    }
    out
}

/// Characters a line may not begin with under Japanese line-breaking rules.
fn no_line_start(token: &str) -> bool {
    matches!(
        token,
        "\u{3002}"
            | "\u{3001}"
            | "\u{ff0c}"
            | "\u{ff0e}"
            | "\u{ff09}"
            | "\u{300d}"
            | "\u{300f}"
            | "\u{3011}"
            | "\u{3015}"
            | "\u{ff01}"
            | "\u{ff1f}"
            | "\u{30fc}"
            | "\u{3063}"
            | "\u{30c3}"
            | "\u{ff1a}"
            | "\u{ff1b}"
    )
}

/// Characters a line may not end with: opening brackets and quotes.
fn no_line_end(token: &str) -> bool {
    matches!(
        token,
        "\u{300c}" | "\u{300e}" | "\u{ff08}" | "\u{3010}" | "\u{3014}" | "(" | "["
    )
}

/// Greedy word wrap over styled runs.
///
/// A run of Latin letters is kept whole where it fits; each wide (CJK)
/// character is its own break opportunity, since Japanese has no spaces to
/// break at. A token wider than a whole line is split by character.
pub fn wrap(
    segments: Vec<(String, Style)>,
    width: usize,
    first_prefix: Vec<Span<'static>>,
    cont_prefix: Vec<Span<'static>>,
) -> Vec<Vec<Span<'static>>> {
    let first_w: usize = first_prefix.iter().map(|s| s.width()).sum();
    let cont_w: usize = cont_prefix.iter().map(|s| s.width()).sum();

    // Tokenise.
    let mut tokens: Vec<(String, Style)> = Vec::new();
    for (text, style) in segments {
        let mut word = String::new();
        for c in text.chars() {
            let wide = c.width().unwrap_or(0) > 1;
            if c.is_whitespace() || wide {
                if !word.is_empty() {
                    tokens.push((std::mem::take(&mut word), style));
                }
                tokens.push((
                    if c.is_whitespace() {
                        " ".into()
                    } else {
                        c.to_string()
                    },
                    style,
                ));
            } else {
                word.push(c);
            }
        }
        if !word.is_empty() {
            tokens.push((word, style));
        }
    }

    let mut lines: Vec<Vec<Span<'static>>> = Vec::new();
    // Spans at the head of `line` that belong to its prefix, not its text.
    let mut prefix_spans = first_prefix.len();
    let mut line: Vec<Span<'static>> = first_prefix;
    let mut avail = width.saturating_sub(first_w).max(1);
    let mut used = 0;
    let cont_avail = width.saturating_sub(cont_w).max(1);

    for (token, style) in tokens {
        let w = token.width();
        if token == " " && used == 0 && !lines.is_empty() {
            // No leading space on a continuation line.
            continue;
        }
        if used + w > avail && used > 0 {
            // Kinsoku: Japanese never starts a line with closing punctuation.
            // Carry the character before it down too, so "。" never sits
            // alone at the start of a line.
            // Nor does a line end with an opening bracket; carry those down
            // with whatever they open. Only ever carry while something else
            // stays behind, so a line cannot empty out.
            let mut carried = Vec::new();
            if no_line_start(&token) && line.len() > prefix_spans + 1 {
                carried.extend(line.pop());
            }
            while line.len() > prefix_spans + 1
                && line.last().is_some_and(|s| no_line_end(&s.content))
            {
                carried.extend(line.pop());
            }
            carried.reverse();
            lines.push(std::mem::replace(&mut line, cont_prefix.clone()));
            prefix_spans = cont_prefix.len();
            avail = cont_avail;
            used = 0;
            for span in carried {
                used += span.width();
                line.push(span);
            }
            if token == " " {
                continue;
            }
        }
        if w > avail {
            for piece in hard_wrap(&token, avail) {
                let pw = piece.width();
                if used + pw > avail && used > 0 {
                    lines.push(std::mem::replace(&mut line, cont_prefix.clone()));
                    prefix_spans = cont_prefix.len();
                    avail = cont_avail;
                    used = 0;
                }
                line.push(Span::styled(piece, style));
                used += pw;
            }
            continue;
        }
        line.push(Span::styled(token, style));
        used += w;
    }
    lines.push(line);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Theme, ThemeName};

    fn th() -> Theme {
        Theme::from_name(ThemeName::Default)
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn plain(lines: &[Line]) -> Vec<String> {
        lines.iter().map(text).collect()
    }

    #[test]
    fn emphasis_markers_are_removed() {
        let lines = render("a **bold** and `code` and *it*", 80, &[], &th());
        assert_eq!(plain(&lines), ["a bold and code and it"]);
        let bold = lines[0].spans.iter().find(|s| s.content == "bold").unwrap();
        assert!(bold.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn snake_case_is_not_italicised() {
        let lines = render("call my_func_name now", 80, &[], &th());
        assert_eq!(plain(&lines), ["call my_func_name now"]);
    }

    #[test]
    fn arithmetic_stars_survive() {
        let lines = render("5 * 3 = 15", 80, &[], &th());
        assert_eq!(plain(&lines), ["5 * 3 = 15"]);
    }

    #[test]
    fn links_keep_their_text_only() {
        let lines = render("see [the docs](https://x.y/z) now", 80, &[], &th());
        assert_eq!(plain(&lines), ["see the docs now"]);
    }

    #[test]
    fn images_become_a_placeholder() {
        let lines = render("![shot](https://x/y.png)", 80, &[], &th());
        assert_eq!(plain(&lines), ["[image: shot]"]);
    }

    #[test]
    fn a_nested_bullet_is_indented_by_display_width() {
        // One ideographic space: three bytes, two cells.
        let lines = render("\u{3000}- nested", 80, &[], &th());
        assert!(
            plain(&lines)[0].starts_with("  \u{2022} "),
            "{:?}",
            plain(&lines)
        );
    }

    #[test]
    fn bullets_wrap_under_their_text() {
        let lines = render("- one two three four five", 12, &[], &th());
        let out = plain(&lines);
        assert!(out.len() > 1);
        assert!(out[0].starts_with("\u{2022} "));
        assert!(
            out[1].starts_with("  "),
            "continuation is indented: {out:?}"
        );
    }

    #[test]
    fn japanese_wraps_by_display_width() {
        let lines = render("日本語のテキストを折り返す", 10, &[], &th());
        for line in &lines {
            assert!(text(line).width() <= 10, "{:?}", text(line));
        }
        assert_eq!(plain(&lines).concat(), "日本語のテキストを折り返す");
    }

    #[test]
    fn a_line_never_starts_with_a_full_stop() {
        // Ten cells hold exactly five characters, so "。" lands at a break.
        let lines = render("あいうえお。かき", 10, &[], &th());
        let out = plain(&lines);
        assert_eq!(out, ["あいうえ", "お。かき"]);
    }

    #[test]
    fn a_line_never_ends_with_an_opening_bracket() {
        let lines = render("あいうえ「かき」", 10, &[], &th());
        let out = plain(&lines);
        assert!(out.iter().all(|l| !l.ends_with('\u{300c}')), "{out:?}");
        assert_eq!(out.concat(), "あいうえ「かき」");
    }

    #[test]
    fn the_gutter_prefixes_every_wrapped_line() {
        let gutter = vec![Span::raw("| ")];
        let lines = render("aaaa bbbb cccc dddd", 11, &gutter, &th());
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| text(l).starts_with("| ")));
    }

    #[test]
    fn code_blocks_are_verbatim() {
        let lines = render("```rust\nlet **x** = 1;\n```", 40, &[], &th());
        let out = plain(&lines);
        assert!(out.iter().any(|l| l.contains("let **x** = 1;")));
        assert!(out[0].contains("rust"));
    }

    #[test]
    fn headings_drop_their_hashes() {
        let lines = render("## Heading", 40, &[], &th());
        assert_eq!(plain(&lines), ["Heading"]);
    }

    #[test]
    fn runs_of_blank_lines_collapse() {
        let lines = render("a\n\n\n\nb", 40, &[], &th());
        assert_eq!(plain(&lines), ["a", "", "b"]);
    }

    #[test]
    fn task_items_render_as_checkboxes() {
        let lines = render("- [ ] todo\n- [x] done", 40, &[], &th());
        assert_eq!(plain(&lines), ["\u{2610} todo", "\u{2611} done"]);
    }
}
