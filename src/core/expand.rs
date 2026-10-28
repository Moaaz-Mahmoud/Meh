//! Turning a matched snippet into text and key actions.
//!
//! - `{{name}}` is replaced by the snippet's variable `name` (spaces inside the
//!   braces are allowed). Unknown names stay as literal text.
//! - The first `$|$` marks where the cursor ends up; later markers are removed.
//!   The template is split at the marker *before* substitution, so a marker
//!   inside clipboard text is just text.
//! - Variables are evaluated only when the text uses them, at most once each.

use crate::core::config::{Snippet, VarKind};
use chrono::{DateTime, Datelike, FixedOffset, TimeDelta};
use std::collections::HashMap;
use std::fmt::{self, Write as _};

/// Cursor marker inside `replace`.
pub const CURSOR_MARKER: &str = "$|$";

/// Rendered replacement text. `left` is how many Left-arrow presses move the
/// cursor from the end of `text` back to the marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    pub text: String,
    pub left: usize,
    /// Whether `replace` had a `$|$`. A marker at the very end has
    /// `left == 0` but still places the cursor before a re-typed separator.
    pub has_marker: bool,
}

/// Why an expansion failed. Holds only config-derived variable names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpandError {
    /// The date `format` is not a valid strftime string.
    BadDateFormat { var: String },
    /// `offset` moves the date outside the representable range.
    DateOutOfRange { var: String },
    /// The clipboard has no text, or could not be read.
    ClipboardUnavailable { var: String },
}

impl fmt::Display for ExpandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExpandError::BadDateFormat { var } => write!(f, "invalid date format in `{var}`"),
            ExpandError::DateOutOfRange { var } => write!(f, "date offset out of range in `{var}`"),
            ExpandError::ClipboardUnavailable { var } => {
                write!(f, "clipboard text unavailable for `{var}`")
            }
        }
    }
}

impl std::error::Error for ExpandError {}

/// Renders `snippet` at time `now`. `clipboard` is called at most once, and
/// only if a clipboard variable is used.
pub fn render(
    snippet: &Snippet,
    now: DateTime<FixedOffset>,
    clipboard: &dyn Fn() -> Option<String>,
) -> Result<Expansion, ExpandError> {
    let (before, after) = match snippet.replace.split_once(CURSOR_MARKER) {
        Some((b, a)) => (b, Some(a.replace(CURSOR_MARKER, ""))),
        None => (snippet.replace.as_str(), None),
    };

    let mut ctx = Ctx {
        snippet,
        now,
        clipboard,
        cache: HashMap::new(),
    };
    let mut text = ctx.substitute(before)?;
    let has_marker = after.is_some();
    let left = match after {
        Some(after) => {
            let after = ctx.substitute(&after)?;
            text.push_str(&after);
            caret_len(&after)
        }
        None => 0,
    };
    Ok(Expansion {
        text,
        left,
        has_marker,
    })
}

struct Ctx<'a> {
    snippet: &'a Snippet,
    now: DateTime<FixedOffset>,
    clipboard: &'a dyn Fn() -> Option<String>,
    cache: HashMap<String, String>,
}

impl Ctx<'_> {
    fn substitute(&mut self, template: &str) -> Result<String, ExpandError> {
        let mut out = String::with_capacity(template.len());
        let mut rest = template;
        while let Some(open) = rest.find("{{") {
            out.push_str(&rest[..open]);
            let after_open = &rest[open + 2..];
            let Some(close) = after_open.find("}}") else {
                rest = &rest[open..];
                break;
            };
            let name = after_open[..close].trim();
            match self.value(name)? {
                Some(v) => out.push_str(&v),
                None => out.push_str(&rest[open..open + 2 + close + 2]),
            }
            rest = &after_open[close + 2..];
        }
        out.push_str(rest);
        Ok(out)
    }

    /// The value of variable `name`, or `None` if the snippet has no such var.
    fn value(&mut self, name: &str) -> Result<Option<String>, ExpandError> {
        if let Some(v) = self.cache.get(name) {
            return Ok(Some(v.clone()));
        }
        let Some(var) = self.snippet.vars.iter().find(|v| v.name == name) else {
            return Ok(None);
        };
        let value = match &var.kind {
            VarKind::Date { format, offset } => {
                // Years outside 0..=9999 are rejected for every format:
                // `to_rfc2822` panics on them, and a 5-digit year is never
                // what a snippet wants.
                let when = TimeDelta::try_seconds(*offset)
                    .and_then(|d| self.now.checked_add_signed(d))
                    .filter(|w| (0..=9999).contains(&w.year()))
                    .ok_or_else(|| ExpandError::DateOutOfRange {
                        var: var.name.clone(),
                    })?;
                let mut s = String::new();
                let written = match format {
                    // `format()` + `to_string()` would panic on a bad format.
                    Some(f) => write!(s, "{}", when.format(f)),
                    None => {
                        s = when.to_rfc2822();
                        Ok(())
                    }
                };
                written.map_err(|_| ExpandError::BadDateFormat {
                    var: var.name.clone(),
                })?;
                s
            }
            VarKind::Clipboard => {
                (self.clipboard)().ok_or_else(|| ExpandError::ClipboardUnavailable {
                    var: var.name.clone(),
                })?
            }
        };
        self.cache.insert(name.to_string(), value.clone());
        Ok(Some(value))
    }
}

/// How many Left presses it takes to cross `s` (CRLF counts once).
fn caret_len(s: &str) -> usize {
    s.chars().count() - s.matches("\r\n").count()
}

/// One key action for the injector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Backspace.
    Back,
    /// One UTF-16 code unit, sent as a Unicode event.
    Unit(u16),
    /// Return key, for a line break (`\n`, `\r\n` or `\r`).
    Enter,
    /// Tab key. Not reachable from config: a `replace` with a tab is not
    /// loaded (D-15).
    Tab,
    /// Left arrow.
    Left,
}

/// Turns text into key actions: line breaks become [`Key::Enter`], tabs
/// [`Key::Tab`], everything else UTF-16 units (astral chars as surrogate pairs).
pub fn plan_keys(text: &str) -> Vec<Key> {
    let mut keys = Vec::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                chars.next_if_eq(&'\n');
                keys.push(Key::Enter);
            }
            '\n' => keys.push(Key::Enter),
            '\t' => keys.push(Key::Tab),
            _ => {
                let mut buf = [0u16; 2];
                keys.extend(c.encode_utf16(&mut buf).iter().map(|&u| Key::Unit(u)));
            }
        }
    }
    keys
}

/// The full key sequence for one hit: erase the typed trigger, type the
/// text and the re-typed separator, then move the cursor to the marker.
/// The separator always goes after the whole replacement, so with a marker
/// (even one at the very end) the cursor also moves back over it.
pub fn plan_injection(erase: usize, expansion: &Expansion, retype: Option<char>) -> Vec<Key> {
    let mut keys = vec![Key::Back; erase];
    keys.extend(plan_keys(&expansion.text));
    let mut left = expansion.left;
    if let Some(sep) = retype {
        let mut buf = [0u8; 4];
        keys.extend(plan_keys(sep.encode_utf8(&mut buf)));
        if expansion.has_marker {
            left += 1;
        }
    }
    keys.extend(std::iter::repeat_n(Key::Left, left));
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::{Snippet, Var, VarKind};
    use chrono::DateTime;

    fn now() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-10-28T15:04:05+03:00").unwrap()
    }

    fn no_clip() -> Option<String> {
        None
    }

    fn snip(replace: &str, vars: Vec<Var>) -> Snippet {
        Snippet {
            trigger: ":t".into(),
            replace: replace.into(),
            word: false,
            vars,
        }
    }

    fn date(name: &str, format: Option<&str>, offset: i64) -> Var {
        Var {
            name: name.into(),
            kind: VarKind::Date {
                format: format.map(Into::into),
                offset,
            },
        }
    }

    fn clip(name: &str) -> Var {
        Var {
            name: name.into(),
            kind: VarKind::Clipboard,
        }
    }

    fn text(s: &Snippet) -> String {
        render(s, now(), &no_clip).unwrap().text
    }

    #[test]
    fn date_default_is_rfc2822() {
        let s = snip("{{d}}", vec![date("d", None, 0)]);
        assert_eq!(text(&s), "Wed, 28 Oct 2026 15:04:05 +0300");
    }

    #[test]
    fn date_strftime_format() {
        let s = snip("on {{d}}!", vec![date("d", Some("%Y-%m-%d %H:%M"), 0)]);
        assert_eq!(text(&s), "on 2026-10-28 15:04!");
    }

    #[test]
    fn date_offset_plus_86400() {
        let s = snip("{{d}}", vec![date("d", Some("%Y-%m-%d"), 86400)]);
        assert_eq!(text(&s), "2026-10-29");
    }

    #[test]
    fn date_offset_negative() {
        let s = snip("{{d}}", vec![date("d", Some("%Y-%m-%d %H"), -86400 - 3600)]);
        assert_eq!(text(&s), "2026-10-27 14");
    }

    #[test]
    fn invalid_format_is_err_not_panic() {
        for bad in ["%Q", "abc %", "%E"] {
            let s = snip("{{d}}", vec![date("d", Some(bad), 0)]);
            assert_eq!(
                render(&s, now(), &no_clip),
                Err(ExpandError::BadDateFormat { var: "d".into() }),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn date_offset_out_of_range_is_err() {
        let s = snip("{{d}}", vec![date("d", None, i64::MAX)]);
        assert_eq!(
            render(&s, now(), &no_clip),
            Err(ExpandError::DateOutOfRange { var: "d".into() })
        );
    }

    #[test]
    fn date_year_outside_0_to_9999_is_err_not_panic() {
        // `to_rfc2822` panics outside years 0..=9999, so every format must
        // reject such dates up front (verifier defect 1).
        for offset in [400_000_000_000, -400_000_000_000, 8_000_000_000_000] {
            for format in [None, Some("%Y"), Some("%d %B")] {
                let s = snip("{{d}}", vec![date("d", format, offset)]);
                assert_eq!(
                    render(&s, now(), &no_clip),
                    Err(ExpandError::DateOutOfRange { var: "d".into() }),
                    "offset {offset}, format {format:?}"
                );
            }
        }
        // The edges still render.
        let s = snip("{{d}}", vec![date("d", Some("%Y"), 0)]);
        assert_eq!(text(&s), "2026");
    }

    #[test]
    fn unknown_placeholder_left_literal() {
        let s = snip(
            "a {{nope}} {{d}} {{ d }} {{d",
            vec![date("d", Some("%Y"), 0)],
        );
        assert_eq!(text(&s), "a {{nope}} 2026 2026 {{d");
    }

    #[test]
    fn unused_var_is_not_evaluated() {
        // A broken var that the text never uses does not fail the expansion,
        // and the clipboard is not read unless referenced.
        let s = snip("plain", vec![date("d", Some("%Q"), 0), clip("c")]);
        assert_eq!(
            render(&s, now(), &|| panic!("clipboard read")),
            Ok(Expansion {
                text: "plain".into(),
                left: 0,
                has_marker: false
            })
        );
    }

    #[test]
    fn cursor_marker_splits() {
        let e = render(&snip("Hello $|$ world", vec![]), now(), &no_clip).unwrap();
        assert_eq!(e.text, "Hello  world");
        assert_eq!(e.left, 6);
    }

    #[test]
    fn cursor_newline_counts_one() {
        let e = render(&snip("<p>\n  $|$\n</p>", vec![]), now(), &no_clip).unwrap();
        assert_eq!(e.text, "<p>\n  \n</p>");
        assert_eq!(e.left, 5);
        let e = render(&snip("a$|$\r\nb", vec![]), now(), &no_clip).unwrap();
        assert_eq!(e.left, 2, "CRLF is one Enter, so one Left");
    }

    #[test]
    fn cursor_counts_chars_not_bytes_or_units() {
        let e = render(&snip("$|$\u{20ac}\u{1F600}", vec![]), now(), &no_clip).unwrap();
        assert_eq!(e.left, 2);
    }

    #[test]
    fn only_first_marker_honored_rest_stripped() {
        let e = render(&snip("a$|$b$|$c$|$", vec![]), now(), &no_clip).unwrap();
        assert_eq!(e.text, "abc");
        assert_eq!(e.left, 2);
    }

    #[test]
    fn no_marker_left_zero() {
        let e = render(&snip("plain text", vec![]), now(), &no_clip).unwrap();
        assert_eq!(e.text, "plain text");
        assert_eq!(e.left, 0);
    }

    #[test]
    fn cursor_after_substitution_counts_substituted_text() {
        let s = snip("$|$ {{d}}", vec![date("d", Some("%Y-%m-%d"), 0)]);
        let e = render(&s, now(), &no_clip).unwrap();
        assert_eq!(e.text, " 2026-10-28");
        assert_eq!(e.left, 11);
    }

    #[test]
    fn clipboard_substituted() {
        let s = snip("<a href='{{c}}'>{{c}}</a>", vec![clip("c")]);
        let e = render(&s, now(), &|| Some("https://x.y".into())).unwrap();
        assert_eq!(e.text, "<a href='https://x.y'>https://x.y</a>");
    }

    #[test]
    fn clipboard_none_is_err() {
        let s = snip("{{c}}", vec![clip("c")]);
        assert_eq!(
            render(&s, now(), &no_clip),
            Err(ExpandError::ClipboardUnavailable { var: "c".into() })
        );
    }

    #[test]
    fn clipboard_value_containing_marker_is_not_cursor() {
        let s = snip("[{{c}}]$|$.", vec![clip("c")]);
        let e = render(&s, now(), &|| Some("x$|$y {{c}}".into())).unwrap();
        assert_eq!(e.text, "[x$|$y {{c}}].");
        assert_eq!(e.left, 1);
    }

    #[test]
    fn plan_keys_crlf_is_one_enter() {
        assert_eq!(
            plan_keys("a\r\nb\nc\rd"),
            [
                Key::Unit('a' as u16),
                Key::Enter,
                Key::Unit('b' as u16),
                Key::Enter,
                Key::Unit('c' as u16),
                Key::Enter,
                Key::Unit('d' as u16),
            ]
        );
    }

    #[test]
    fn plan_keys_tab_is_vk_tab() {
        assert_eq!(plan_keys("\tx"), [Key::Tab, Key::Unit('x' as u16)]);
    }

    #[test]
    fn plan_keys_astral_is_surrogate_pair() {
        assert_eq!(
            plan_keys("\u{20ac}\u{1F600}"),
            [Key::Unit(0x20AC), Key::Unit(0xD83D), Key::Unit(0xDE00)]
        );
    }

    #[test]
    fn marker_at_end_with_word_trigger_puts_cursor_before_separator() {
        // `brb` -> "<b>$|$" typed with a space: the cursor must end at the
        // marker (end of the replacement), before the re-typed space, just
        // like a marker anywhere else (verifier defect 7).
        let e = render(&snip("<b>$|$", vec![]), now(), &no_clip).unwrap();
        assert_eq!(e.text, "<b>");
        assert_eq!(e.left, 0);
        assert!(e.has_marker);
        let keys = plan_injection(4, &e, Some(' '));
        assert_eq!(
            &keys[4..],
            [
                Key::Unit('<' as u16),
                Key::Unit('b' as u16),
                Key::Unit('>' as u16),
                Key::Unit(' ' as u16),
                Key::Left
            ]
        );
        // Without a separator, a marker at the end needs no Left.
        let keys = plan_injection(2, &e, None);
        assert_eq!(keys.last(), Some(&Key::Unit('>' as u16)));
    }

    #[test]
    fn marker_mid_text_with_word_trigger_counts_separator() {
        let e = render(&snip("a$|$bc", vec![]), now(), &no_clip).unwrap();
        assert!(e.has_marker);
        let keys = plan_injection(4, &e, Some('.'));
        assert_eq!(
            &keys[4..],
            [
                Key::Unit('a' as u16),
                Key::Unit('b' as u16),
                Key::Unit('c' as u16),
                Key::Unit('.' as u16),
                Key::Left,
                Key::Left,
                Key::Left
            ]
        );
        // No marker: the cursor stays after the separator.
        let e = render(&snip("abc", vec![]), now(), &no_clip).unwrap();
        assert!(!e.has_marker);
        assert_eq!(
            plan_injection(4, &e, Some('.')).last(),
            Some(&Key::Unit('.' as u16))
        );
    }

    #[test]
    fn plan_injection_orders_erase_text_separator_and_lefts() {
        let e = Expansion {
            text: "<b></b>".into(),
            left: 4,
            has_marker: true,
        };
        let keys = plan_injection(4, &e, Some(' '));
        assert_eq!(&keys[..4], [Key::Back; 4]);
        assert_eq!(keys[4], Key::Unit('<' as u16));
        assert_eq!(keys[11], Key::Unit(' ' as u16));
        // The re-typed separator sits after the cursor spot too: 4 + 1 Lefts.
        assert_eq!(&keys[12..], [Key::Left; 5]);
        assert_eq!(keys.len(), 17);
        // No marker: no Lefts, even with a separator.
        let e = Expansion {
            text: "x".into(),
            left: 0,
            has_marker: false,
        };
        assert_eq!(
            plan_injection(2, &e, Some('\n')),
            [Key::Back, Key::Back, Key::Unit('x' as u16), Key::Enter]
        );
    }
}
