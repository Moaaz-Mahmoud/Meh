//! Pure helpers for the keyboard hook: decisions that do not need Win32, so
//! they can be tested on any OS.

/// What a key event means for the matcher's buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
    Char(char),
    Reset,
}

/// Joins UTF-16 code units that arrive one key event at a time. The emoji
/// panel and other tools send an astral character (an emoji) as two
/// `VK_PACKET` events: a high surrogate, then a low surrogate.
///
/// - A high surrogate waits for the next unit.
/// - A low surrogate right after it completes one character.
/// - Anything else after a high surrogate drops it with a reset, and is then
///   handled as usual.
/// - A low surrogate with no high one before it is a reset.
/// - Control characters are not text and reset.
///
/// `Copy`, so the hook can keep it in a thread-local `Cell`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Utf16Joiner {
    high: Option<u16>,
}

fn is_high(u: u16) -> bool {
    (0xD800..0xDC00).contains(&u)
}

fn is_low(u: u16) -> bool {
    (0xDC00..0xE000).contains(&u)
}

impl Utf16Joiner {
    pub const fn new() -> Self {
        Utf16Joiner { high: None }
    }

    /// Feeds one UTF-16 unit; calls `out` zero, one or two times.
    pub fn push(&mut self, unit: u16, out: &mut impl FnMut(Typed)) {
        if let Some(high) = self.high.take() {
            if is_low(unit) {
                let c = char::decode_utf16([high, unit]).next().and_then(Result::ok);
                return out(c.map_or(Typed::Reset, Typed::Char));
            }
            out(Typed::Reset);
        }
        if is_high(unit) {
            self.high = Some(unit);
            return;
        }
        match char::from_u32(unit.into()) {
            Some(c) if !c.is_control() => out(Typed::Char(c)),
            // A lone low surrogate or a control character.
            _ => out(Typed::Reset),
        }
    }

    /// Drops a waiting high surrogate because a non-text event came in.
    /// Returns whether one was waiting (the caller should then reset).
    pub fn interrupt(&mut self) -> bool {
        self.high.take().is_some()
    }
}

/// Whether a key pressed with these modifiers is a shortcut that may move
/// the caret, so the buffer resets (D-08): any Win combo, or Ctrl or Alt
/// alone. Ctrl and Alt together are AltGr and go on to be translated.
pub fn is_shortcut(ctrl: bool, alt: bool, win: bool) -> bool {
    win || ctrl != alt
}

/// What a `ToUnicodeEx` result means for the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Translation {
    /// The first `n` units of the output buffer are text.
    Units(usize),
    /// Reset the buffer.
    Reset,
    /// Not a text key (F-keys, Insert, ...): leave the buffer alone.
    Nothing,
}

/// Interprets `ToUnicodeEx`'s return value `n` for a buffer of `cap` units.
/// - `n < 0`: a dead key. The app composes what comes next, which we cannot
///   follow, so reset.
/// - `n == 0` with Ctrl and Alt held: a Ctrl+Alt shortcut, not AltGr text,
///   so reset like any other shortcut (verifier defect 9).
/// - `n == 0` otherwise: not a text key.
pub fn interpret_to_unicode(n: i32, ctrl_and_alt: bool, cap: usize) -> Translation {
    match n {
        n if n < 0 => Translation::Reset,
        0 if ctrl_and_alt => Translation::Reset,
        0 => Translation::Nothing,
        n => Translation::Units((n as usize).min(cap)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(j: &mut Utf16Joiner, units: &[u16]) -> Vec<Typed> {
        let mut out = Vec::new();
        for &u in units {
            j.push(u, &mut |t| out.push(t));
        }
        out
    }

    #[test]
    fn surrogate_pair_joins_into_one_char() {
        let mut j = Utf16Joiner::default();
        // U+1F600 is D83D DE00.
        assert_eq!(feed(&mut j, &[0xD83D]), []);
        assert_eq!(feed(&mut j, &[0xDE00]), [Typed::Char('\u{1F600}')]);
        assert!(!j.interrupt(), "nothing left waiting");
    }

    #[test]
    fn bmp_units_pass_through() {
        let mut j = Utf16Joiner::default();
        assert_eq!(
            feed(&mut j, &['a' as u16, 0x20AC]),
            [Typed::Char('a'), Typed::Char('\u{20ac}')]
        );
    }

    #[test]
    fn high_without_low_resets_then_handles_next_unit() {
        let mut j = Utf16Joiner::default();
        assert_eq!(
            feed(&mut j, &[0xD83D, 'x' as u16]),
            [Typed::Reset, Typed::Char('x')]
        );
        // Two highs in a row: the first is dropped, the second waits.
        assert_eq!(feed(&mut j, &[0xD83D, 0xD83D]), [Typed::Reset]);
        assert_eq!(feed(&mut j, &[0xDE00]), [Typed::Char('\u{1F600}')]);
    }

    #[test]
    fn lone_low_surrogate_resets() {
        let mut j = Utf16Joiner::default();
        assert_eq!(feed(&mut j, &[0xDE00]), [Typed::Reset]);
        assert_eq!(feed(&mut j, &['a' as u16]), [Typed::Char('a')]);
    }

    #[test]
    fn interrupt_drops_pending_high() {
        let mut j = Utf16Joiner::default();
        assert!(!j.interrupt());
        feed(&mut j, &[0xD83D]);
        assert!(j.interrupt());
        assert!(!j.interrupt());
        // The low half that follows is now a lone surrogate.
        assert_eq!(feed(&mut j, &[0xDE00]), [Typed::Reset]);
    }

    #[test]
    fn control_units_reset() {
        let mut j = Utf16Joiner::default();
        assert_eq!(feed(&mut j, &[0x01, 0x7F]), [Typed::Reset, Typed::Reset]);
    }

    #[test]
    fn shortcuts_reset_but_altgr_does_not() {
        assert!(!is_shortcut(false, false, false));
        assert!(!is_shortcut(true, true, false), "AltGr");
        assert!(is_shortcut(true, false, false));
        assert!(is_shortcut(false, true, false));
        assert!(is_shortcut(false, false, true));
        assert!(is_shortcut(true, true, true));
    }

    #[test]
    fn ctrl_alt_with_no_character_resets() {
        assert_eq!(interpret_to_unicode(0, true, 8), Translation::Reset);
        // AltGr that types a character (e.g. `@` on a German layout).
        assert_eq!(interpret_to_unicode(1, true, 8), Translation::Units(1));
        // A plain non-text key (F5, Insert) leaves the buffer alone.
        assert_eq!(interpret_to_unicode(0, false, 8), Translation::Nothing);
        // Dead keys reset, with or without AltGr.
        assert_eq!(interpret_to_unicode(-1, false, 8), Translation::Reset);
        assert_eq!(interpret_to_unicode(-1, true, 8), Translation::Reset);
        // The count is clamped to the buffer.
        assert_eq!(interpret_to_unicode(20, false, 8), Translation::Units(8));
    }
}
