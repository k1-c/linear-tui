//! Fuzzy matching for the command palette and the pickers.
//!
//! A query matches when its characters appear in the text in order, ignoring
//! case. Among matches, a contiguous run beats scattered letters, and letters
//! that start a word (`cs` → **C**hange **s**tatus) beat letters inside one.
//! That is all a short list of menu entries needs; a full matcher such as
//! nucleo is built for tens of thousands of file paths.

/// A successful match: higher `score` is better; `positions` are the char
/// indices of `text` that matched, for highlighting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub score: i32,
    pub positions: Vec<usize>,
}

const WORD_START: i32 = 8;
const CONSECUTIVE: i32 = 5;
const FIRST_CHAR: i32 = 4;
const GAP: i32 = 1;

/// Match `query` against `text`. An empty query matches everything with a
/// score of zero.
pub fn score(query: &str, text: &str) -> Option<Match> {
    let query: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if query.is_empty() {
        return Some(Match {
            score: 0,
            positions: Vec::new(),
        });
    }
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = chars
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let word_start = |i: usize| i == 0 || !chars[i - 1].is_alphanumeric();

    // Two candidates: the greedy leftmost match, and one that prefers word
    // starts for each letter. Keep whichever scores higher.
    [false, true]
        .into_iter()
        .filter_map(|prefer_starts| {
            let mut positions = Vec::with_capacity(query.len());
            let mut from = 0;
            for &q in &query {
                let hit = |i: &usize| lower[*i] == q;
                let found = if prefer_starts {
                    (from..lower.len())
                        .filter(hit)
                        .find(|&i| word_start(i))
                        .or_else(|| (from..lower.len()).find(hit))
                } else {
                    (from..lower.len()).find(hit)
                }?;
                positions.push(found);
                from = found + 1;
            }
            Some(positions)
        })
        .map(|positions| Match {
            score: rate(&positions, word_start),
            positions,
        })
        .max_by_key(|m| m.score)
}

fn rate(positions: &[usize], word_start: impl Fn(usize) -> bool) -> i32 {
    let mut score = 0;
    for (n, &i) in positions.iter().enumerate() {
        if word_start(i) {
            score += WORD_START;
        }
        if i == 0 {
            score += FIRST_CHAR;
        }
        if n > 0 {
            let prev = positions[n - 1];
            if i == prev + 1 {
                score += CONSECUTIVE;
            } else {
                score -= GAP * (i - prev - 1).min(10) as i32;
            }
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_must_appear_in_order() {
        assert!(score("cst", "Change status").is_some());
        assert!(score("zz", "Change status").is_none());
        assert!(score("sc", "Change status").is_none());
    }

    #[test]
    fn case_is_ignored() {
        assert_eq!(score("CS", "change status").unwrap().positions, [0, 7]);
    }

    #[test]
    fn word_starts_beat_letters_inside_words() {
        let initials = score("cs", "Change status").unwrap();
        assert_eq!(initials.positions, [0, 7]);
        let inside = score("cs", "Discuss").unwrap();
        assert!(initials.score > inside.score);
    }

    #[test]
    fn a_contiguous_run_beats_scattered_letters() {
        let run = score("stat", "Change status").unwrap();
        let scattered = score("stat", "Set the assignee today").unwrap();
        assert!(run.score > scattered.score);
    }

    #[test]
    fn positions_count_characters_not_bytes() {
        let m = score("課題", "日本語の課題").unwrap();
        assert_eq!(m.positions, [4, 5]);
    }

    #[test]
    fn an_empty_query_matches_everything() {
        assert_eq!(score("", "anything").unwrap().score, 0);
    }
}
