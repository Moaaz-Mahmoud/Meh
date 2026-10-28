//! Rolling-buffer trigger matcher (D-08, D-15).
//!
//! - Triggers are case-sensitive.
//! - A plain trigger fires on its last character, even mid-word.
//! - A `word: true` trigger fires on the separator typed after it, and only
//!   if it starts the buffer or follows a separator. The separator is erased
//!   with the trigger and re-typed after the replacement.
//! - Tab and Enter (`'\t'`, `'\n'`, `'\r'`) never fire anything: the app
//!   has already acted on them (moved focus, sent a message), so they only
//!   clear the buffer (D-15).
//! - When several triggers match, the longest one wins. A trigger that is a
//!   prefix of another fires first (`:a` shadows `:ab`); this is a documented
//!   limitation.
//! - The buffer is bounded to what the longest trigger can need, and it is
//!   cleared after every hit. The caller resets it on caret moves (D-08).
//! - After a hit, the buffer start is a word boundary only if the caller
//!   says the typed output left one before the caret ([`Matcher::after_expansion`]),
//!   so `:hi` -> `Hello` followed by `brb ` does not fire `brb`.
//!
//! The buffer holds typed text only in memory and is never exposed.

use crate::core::config::Snippet;

/// Word separators (D-08 as amended by D-15: no Tab or Enter).
pub const SEPARATORS: &[char] = &[' ', ',', '.', '?', '!', ';', ':'];

/// Keys that clear the buffer when typed and never fire a trigger (D-15).
fn is_reset_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r')
}

pub fn is_separator(c: char) -> bool {
    SEPARATORS.contains(&c)
}

/// A trigger match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    /// Index into the snippet slice given to [`Matcher::new`].
    pub idx: usize,
    /// Number of typed characters to erase (trigger, plus the separator for
    /// a word trigger).
    pub erase: usize,
    /// Separator to re-type after the replacement (word triggers only).
    pub retype: Option<char>,
}

#[derive(Debug)]
struct Trigger {
    chars: Vec<char>,
    word: bool,
}

#[derive(Debug)]
pub struct Matcher {
    triggers: Vec<Trigger>,
    buf: Vec<char>,
    cap: usize,
    /// Whether the position before `buf[0]` is a word boundary. False once
    /// the bound has dropped characters from the front.
    start_is_boundary: bool,
}

impl Matcher {
    pub fn new(snippets: &[Snippet]) -> Self {
        let triggers: Vec<Trigger> = snippets
            .iter()
            .map(|s| Trigger {
                chars: s.trigger.chars().collect(),
                word: s.word,
            })
            .collect();
        // Longest trigger, plus its separator, plus one char of left context.
        let cap = triggers
            .iter()
            .map(|t| t.chars.len())
            .max()
            .map_or(0, |n| n + 2);
        Matcher {
            triggers,
            buf: Vec::with_capacity(cap),
            cap,
            start_is_boundary: true,
        }
    }

    /// Number of buffered characters.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Feeds one typed character. On a hit the buffer is cleared. Tab and
    /// Enter only clear it (D-15).
    pub fn push_char(&mut self, c: char) -> Option<Hit> {
        if is_reset_char(c) {
            self.reset();
            return None;
        }
        if self.cap == 0 {
            return None;
        }
        self.buf.push(c);
        if self.buf.len() > self.cap {
            let excess = self.buf.len() - self.cap;
            self.buf.drain(..excess);
            self.start_is_boundary = false;
        }

        let sep = is_separator(c);
        let mut best: Option<Hit> = None;
        for (idx, t) in self.triggers.iter().enumerate() {
            let candidate = if t.word {
                if !sep {
                    continue;
                }
                let end = self.buf.len() - 1;
                if !self.buf[..end].ends_with(&t.chars)
                    || !self.boundary_before(end - t.chars.len())
                {
                    continue;
                }
                Hit {
                    idx,
                    erase: t.chars.len() + 1,
                    retype: Some(c),
                }
            } else {
                if !self.buf.ends_with(&t.chars) {
                    continue;
                }
                Hit {
                    idx,
                    erase: t.chars.len(),
                    retype: None,
                }
            };
            if best.is_none_or(|b| candidate.erase > b.erase) {
                best = Some(candidate);
            }
        }
        if let Some(hit) = best {
            // A best guess until the caller says what the replacement left
            // before the caret (`after_expansion`): a re-typed separator is
            // a boundary, the end of a plain replacement is not known to be.
            self.buf.clear();
            self.start_is_boundary = hit.retype.is_some();
        }
        best
    }

    /// Whether position `i` in the buffer (the start of a candidate trigger)
    /// is preceded by a word boundary.
    fn boundary_before(&self, i: usize) -> bool {
        if i == 0 {
            self.start_is_boundary
        } else {
            is_separator(self.buf[i - 1])
        }
    }

    /// Drops the last buffered character (the user pressed Backspace). On
    /// an empty buffer it erases text we never saw, so whatever is now left
    /// of the caret is unknown and no longer counts as a word boundary.
    pub fn backspace(&mut self) {
        if self.buf.pop().is_none() {
            self.start_is_boundary = false;
        }
    }

    /// Call after a hit's replacement has been typed (or has failed).
    /// `boundary` is whether the character now left of the caret is a word
    /// boundary (see `Expansion::boundary_before_caret`). After a failed
    /// expansion the typed trigger is still there, so pass whether it ended
    /// in a separator (`hit.retype.is_some()`).
    pub fn after_expansion(&mut self, boundary: bool) {
        self.buf.clear();
        self.start_is_boundary = boundary;
    }

    /// Clears the buffer (caret moved, focus changed, hit, ...).
    pub fn reset(&mut self) {
        self.buf.clear();
        self.start_is_boundary = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Snippet;

    fn snip(trigger: &str, word: bool) -> Snippet {
        Snippet {
            trigger: trigger.into(),
            replace: format!("<{trigger}>"),
            word,
            vars: vec![],
        }
    }

    /// Types `s`, returning every hit along the way.
    fn type_str(m: &mut Matcher, s: &str) -> Vec<Hit> {
        s.chars().filter_map(|c| m.push_char(c)).collect()
    }

    fn hit(idx: usize, erase: usize, retype: Option<char>) -> Hit {
        Hit { idx, erase, retype }
    }

    #[test]
    fn plain_fires_on_last_char() {
        let mut m = Matcher::new(&[snip(":hi", false)]);
        assert_eq!(m.push_char(':'), None);
        assert_eq!(m.push_char('h'), None);
        assert_eq!(m.push_char('i'), Some(hit(0, 3, None)));
    }

    #[test]
    fn plain_matches_mid_word() {
        let mut m = Matcher::new(&[snip("ther", false)]);
        assert_eq!(type_str(&mut m, "other"), [hit(0, 4, None)]);
    }

    #[test]
    fn case_sensitive_no_match() {
        let mut m = Matcher::new(&[snip(":hi", false), snip("brb", true)]);
        assert!(type_str(&mut m, ":Hi :HI :hI BRB Brb ").is_empty());
    }

    #[test]
    fn longest_suffix_wins() {
        let mut m = Matcher::new(&[snip("bc", false), snip("abc", false)]);
        assert_eq!(type_str(&mut m, "abc"), [hit(1, 3, None)]);
        m.reset();
        assert_eq!(type_str(&mut m, "xbc"), [hit(0, 2, None)]);
    }

    #[test]
    fn prefix_trigger_fires_first() {
        // Documented limitation: with `:a` and `:ab`, `:ab` can never fire
        // because `:a` expands as soon as it is typed.
        let mut m = Matcher::new(&[snip(":a", false), snip(":ab", false)]);
        assert_eq!(type_str(&mut m, ":ab"), [hit(0, 2, None)]);
    }

    #[test]
    fn word_waits_for_separator() {
        let mut m = Matcher::new(&[snip("brb", true)]);
        assert!(type_str(&mut m, "brb").is_empty());
        assert_eq!(m.push_char(' '), Some(hit(0, 4, Some(' '))));
    }

    #[test]
    fn word_fires_on_each_separator() {
        for sep in [' ', ',', '.', '?', '!', ';', ':'] {
            let mut m = Matcher::new(&[snip("brb", true)]);
            let mut typed = String::from("brb");
            typed.push(sep);
            assert_eq!(
                type_str(&mut m, &typed),
                [hit(0, 4, Some(sep))],
                "separator {sep:?}"
            );
        }
        assert_eq!(SEPARATORS.len(), 7);
    }

    #[test]
    fn tab_and_enter_reset_without_firing() {
        // D-15: Tab and Enter never fire a trigger (the app has already
        // moved focus or sent the message); they only clear the buffer.
        for key in ['\t', '\n', '\r'] {
            let mut m = Matcher::new(&[snip("brb", true), snip(":hi", false)]);
            assert!(type_str(&mut m, "brb").is_empty());
            assert_eq!(m.push_char(key), None, "{key:?}");
            assert_eq!(m.len(), 0, "{key:?} resets");
            // A plain trigger split by Tab/Enter does not fire either.
            assert!(type_str(&mut m, ":h").is_empty());
            assert_eq!(m.push_char(key), None);
            assert_eq!(m.push_char('i'), None, "{key:?} split the trigger");
            // After the reset, the next word starts at a boundary.
            m.push_char(key);
            assert_eq!(type_str(&mut m, "brb "), [hit(0, 4, Some(' '))]);
            assert!(!is_separator(key));
        }
    }

    #[test]
    fn word_needs_left_boundary() {
        let mut m = Matcher::new(&[snip("brb", true)]);
        // Buffer start counts as a boundary.
        assert_eq!(type_str(&mut m, "brb "), [hit(0, 4, Some(' '))]);
        // After a separator.
        m.reset();
        assert_eq!(type_str(&mut m, "ok,brb."), [hit(0, 4, Some('.'))]);
        // Glued to a word: no.
        m.reset();
        assert!(type_str(&mut m, "xbrb ").is_empty());
        // And the trailing separator then counts as a boundary.
        assert_eq!(type_str(&mut m, "brb "), [hit(0, 4, Some(' '))]);
    }

    #[test]
    fn separator_without_match_stays_in_buffer() {
        // A plain trigger spanning a separator still matches.
        let mut m = Matcher::new(&[snip("a b", false), snip("brb", true)]);
        assert_eq!(type_str(&mut m, "a b"), [hit(0, 3, None)]);
        // A separator typed with no match is the left boundary for the next word.
        m.reset();
        m.push_char('x');
        assert_eq!(m.push_char(' '), None);
        assert_eq!(type_str(&mut m, "brb "), [hit(1, 4, Some(' '))]);
    }

    #[test]
    fn backspace_drops_one_char() {
        let mut m = Matcher::new(&[snip(":hi", false)]);
        assert!(type_str(&mut m, ":hx").is_empty());
        m.backspace();
        assert_eq!(m.push_char('i'), Some(hit(0, 3, None)));
    }

    #[test]
    fn backspace_on_empty_keeps_buffer_empty() {
        let mut m = Matcher::new(&[snip(":hi", false), snip("brb", true)]);
        m.backspace();
        m.backspace();
        assert_eq!(m.len(), 0);
        // Plain triggers do not care what came before.
        assert_eq!(type_str(&mut m, ":hi"), [hit(0, 3, None)]);
    }

    #[test]
    fn backspace_on_empty_makes_start_not_a_boundary() {
        // After a click and a Backspace we no longer know what is left of
        // the caret (it may be the end of a word), so `brb ` must not fire
        // as a word trigger (verifier defect 16).
        let mut m = Matcher::new(&[snip("brb", true)]);
        m.reset();
        m.backspace();
        assert!(type_str(&mut m, "brb ").is_empty());
        // The separator just typed is a boundary again.
        assert_eq!(type_str(&mut m, "brb "), [hit(0, 4, Some(' '))]);
        // Same right after an expansion (buffer cleared by the hit).
        assert_eq!(type_str(&mut m, "brb "), [hit(0, 4, Some(' '))]);
        m.backspace();
        assert!(type_str(&mut m, "brb ").is_empty());
        // A reset (click, arrow) makes the start a boundary again.
        m.reset();
        assert_eq!(type_str(&mut m, "brb "), [hit(0, 4, Some(' '))]);
    }

    #[test]
    fn plain_hit_then_word_trigger_needs_a_boundary() {
        // `:hi` -> "<:hi>" ends in a letter-like char, so `brb ` typed right
        // after it is glued to the replacement and must not fire.
        let mut m = Matcher::new(&[snip(":hi", false), snip("brb", true)]);
        assert_eq!(type_str(&mut m, ":hi"), [hit(0, 3, None)]);
        assert!(type_str(&mut m, "brb ").is_empty(), "no worker call");
        m.reset();
        assert_eq!(type_str(&mut m, ":hi"), [hit(0, 3, None)]);
        m.after_expansion(false);
        assert!(type_str(&mut m, "brb ").is_empty());
        // The worker says the output ended in a separator: the next word
        // trigger may fire.
        assert_eq!(type_str(&mut m, ":hi"), [hit(0, 3, None)]);
        m.after_expansion(true);
        assert_eq!(type_str(&mut m, "brb "), [hit(1, 4, Some(' '))]);
    }

    #[test]
    fn word_hit_then_word_trigger_still_fires() {
        // `brb brb `: the re-typed space is a boundary.
        let mut m = Matcher::new(&[snip("brb", true)]);
        assert_eq!(type_str(&mut m, "brb "), [hit(0, 4, Some(' '))]);
        assert_eq!(type_str(&mut m, "brb "), [hit(0, 4, Some(' '))]);
        m.after_expansion(true);
        assert_eq!(type_str(&mut m, "brb "), [hit(0, 4, Some(' '))]);
        m.after_expansion(true);
        assert_eq!(m.len(), 0);
        assert_eq!(type_str(&mut m, "brb."), [hit(0, 4, Some('.'))]);
    }

    #[test]
    fn reset_clears_buffer() {
        let mut m = Matcher::new(&[snip(":hi", false)]);
        type_str(&mut m, ":h");
        m.reset();
        assert_eq!(m.len(), 0);
        assert_eq!(m.push_char('i'), None);
    }

    #[test]
    fn buffer_cleared_after_hit() {
        let mut m = Matcher::new(&[snip("aa", false)]);
        assert_eq!(type_str(&mut m, "aa"), [hit(0, 2, None)]);
        assert_eq!(m.len(), 0);
        // The second `a` of "aaa" must not reuse the expanded trigger.
        assert_eq!(m.push_char('a'), None);
        assert_eq!(m.push_char('a'), Some(hit(0, 2, None)));
    }

    #[test]
    fn buffer_is_bounded() {
        let mut m = Matcher::new(&[snip(":longest", false), snip("brb", true)]);
        for _ in 0..10_000 {
            m.push_char('x');
            m.push_char(' ');
        }
        assert!(m.len() <= ":longest".chars().count() + 2, "len {}", m.len());
        // Still matches after heavy typing, and old text cannot form a boundary.
        assert_eq!(type_str(&mut m, ":longest"), [hit(0, 8, None)]);
        let mut m = Matcher::new(&[snip("brb", true)]);
        type_str(&mut m, &"y".repeat(100));
        assert!(
            type_str(&mut m, "brb ").is_empty(),
            "truncated start is not a boundary"
        );
    }

    #[test]
    fn no_snippets_never_hits() {
        let mut m = Matcher::new(&[]);
        assert!(type_str(&mut m, "anything at all ").is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn astral_trigger_counts_chars() {
        let mut m = Matcher::new(&[snip(":\u{1F600}", false)]);
        assert_eq!(type_str(&mut m, ":\u{1F600}"), [hit(0, 2, None)]);
    }
}
