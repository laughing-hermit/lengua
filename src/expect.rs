//! Expected output with holes: `{any(lo, hi)}` in a test's expected text stands for any whole
//! number from lo to hi, written plainly. A hole travels inside the expected text as a marker
//! between Unicode noncharacters, which no real text holds, so expected text stays a `String`:
//! stored, compared and searched as before, and matched against a run's output here.

use std::fmt::Write as _;

/// Where a hole starts, between its bounds, and where it ends.
const OPEN: char = '\u{FDD0}';
const BETWEEN: char = '\u{FDD1}';
const CLOSE: char = '\u{FDD2}';

/// One end of a hole's range: a number, or a number of seconds from when the run started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bound {
    /// This number.
    Number(i128),
    /// `now` plus this many seconds.
    Now(i128),
}

impl Bound {
    fn text(self) -> String {
        match self {
            Self::Number(value) => value.to_string(),
            Self::Now(0) => "now".into(),
            Self::Now(offset) if offset > 0 => format!("now + {offset}"),
            Self::Now(offset) => format!("now - {}", offset.unsigned_abs()),
        }
    }

    fn encode(self) -> String {
        match self {
            Self::Number(value) => value.to_string(),
            Self::Now(offset) => format!("n{offset}"),
        }
    }

    fn decode(text: &str) -> Option<Self> {
        match text.strip_prefix('n') {
            Some(offset) => offset.parse().ok().map(Self::Now),
            None => text.parse().ok().map(Self::Number),
        }
    }

    /// The bound's value for a run that started at `now`.
    #[must_use]
    pub const fn at(self, now: i128) -> i128 {
        match self {
            Self::Number(value) => value,
            Self::Now(offset) => now.saturating_add(offset),
        }
    }
}

/// The marker for a hole any whole number from `low` to `high` fills.
#[must_use]
pub fn hole(low: Bound, high: Bound) -> String {
    format!("{OPEN}{}{BETWEEN}{}{CLOSE}", low.encode(), high.encode())
}

/// Whether `text` holds a hole marker or a stray marker character.
#[must_use]
pub fn has_holes(text: &str) -> bool {
    text.contains([OPEN, BETWEEN, CLOSE])
}

/// Whether a hole in `text` is followed directly by a digit or by another hole, where reading
/// the output couldn't tell where its number ends: `{any(1, 6)}0` would take "30" as thirty.
#[must_use]
pub fn crowded(text: &str) -> bool {
    pieces(text).is_some_and(|pieces| {
        pieces.windows(2).any(|pair| match pair {
            [Piece::Hole(..), Piece::Hole(..)] => true,
            [Piece::Hole(..), Piece::Text(after)] => {
                after.starts_with(|c: char| c.is_ascii_digit())
            }
            _ => false,
        })
    })
}

/// A piece of expected text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece<'a> {
    /// Text that must appear exactly.
    Text(&'a str),
    /// Any whole number in this range.
    Hole(Bound, Bound),
}

/// The pieces of `expected`, or `None` when a marker is broken.
#[must_use]
pub fn pieces(expected: &str) -> Option<Vec<Piece<'_>>> {
    let mut pieces = Vec::new();
    let mut rest = expected;
    while let Some(start) = rest.find(OPEN) {
        if start > 0 {
            pieces.push(Piece::Text(&rest[..start]));
        }
        let after = &rest[start + OPEN.len_utf8()..];
        let end = after.find(CLOSE)?;
        let (low, high) = after[..end].split_once(BETWEEN)?;
        pieces.push(Piece::Hole(Bound::decode(low)?, Bound::decode(high)?));
        rest = &after[end + CLOSE.len_utf8()..];
    }
    if rest.contains([BETWEEN, CLOSE]) {
        return None;
    }
    if !rest.is_empty() {
        pieces.push(Piece::Text(rest));
    }
    Some(pieces)
}

/// `expected` as a person reads it: holes written back as `{any(lo, hi)}`.
#[must_use]
pub fn show(expected: &str) -> String {
    let Some(pieces) = pieces(expected) else {
        return expected.to_owned();
    };
    let mut text = String::new();
    for piece in pieces {
        match piece {
            Piece::Text(literal) => text.push_str(literal),
            Piece::Hole(low, high) => {
                let _ = write!(text, "{{any({}, {})}}", low.text(), high.text());
            }
        }
    }
    text
}

/// The whole number written plainly at the start of `bytes` (an optional `-`, then digits with
/// no leading zero, and never `-0`), and how many bytes it takes.
fn number_at(bytes: &[u8]) -> Option<(i128, usize)> {
    let negative = bytes.first() == Some(&b'-');
    let start = usize::from(negative);
    let digits = bytes[start..]
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if digits == 0 || digits > 38 {
        return None;
    }
    let text = std::str::from_utf8(&bytes[start..start + digits]).ok()?;
    if digits > 1 && text.starts_with('0') {
        return None;
    }
    let magnitude: i128 = text.parse().ok()?;
    if negative && magnitude == 0 {
        return None;
    }
    Some((
        if negative { -magnitude } else { magnitude },
        start + digits,
    ))
}

/// Whether `actual` is `expected`, with each hole filled by a whole number in its range, for a
/// run that started at `now` (Unix seconds).
#[must_use]
pub fn matches(expected: &str, actual: &[u8], now: i128) -> bool {
    if !has_holes(expected) {
        return expected.as_bytes() == actual;
    }
    let Some(pieces) = pieces(expected) else {
        return false;
    };
    let mut rest = actual;
    for piece in pieces {
        match piece {
            Piece::Text(literal) => match rest.strip_prefix(literal.as_bytes()) {
                Some(after) => rest = after,
                None => return false,
            },
            Piece::Hole(low, high) => match number_at(rest) {
                Some((value, length)) if (low.at(now)..=high.at(now)).contains(&value) => {
                    rest = &rest[length..];
                }
                _ => return false,
            },
        }
    }
    rest.is_empty()
}

/// Whether a block's computed text agrees with an example's, both for a run that starts at
/// `now`: when the block's text is exact, the example's holes may hold it; otherwise the two must
/// be the same text with the same ranges.
#[must_use]
pub fn agrees(computed: &str, example: &str, now: i128) -> bool {
    if !has_holes(computed) {
        return matches(example, computed.as_bytes(), now);
    }
    let (Some(computed), Some(example)) = (pieces(computed), pieces(example)) else {
        return false;
    };
    computed.len() == example.len()
        && computed.iter().zip(&example).all(|pair| match pair {
            (Piece::Text(a), Piece::Text(b)) => a == b,
            (Piece::Hole(low, high), Piece::Hole(other_low, other_high)) => {
                low.at(now) == other_low.at(now) && high.at(now) == other_high.at(now)
            }
            _ => false,
        })
}

/// The current time in whole Unix seconds, which `now` stands for.
#[must_use]
pub fn now() -> i128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| i128::from(elapsed.as_secs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rolled() -> String {
        format!("rolled {}\n", hole(Bound::Number(1), Bound::Number(6)))
    }

    #[test]
    fn holes_take_plain_whole_numbers_in_range() {
        let expected = rolled();
        assert!(matches(&expected, b"rolled 1\n", 0));
        assert!(matches(&expected, b"rolled 6\n", 0));
        for wrong in [
            &b"rolled 7\n"[..],
            b"rolled 0\n",
            b"rolled 06\n",
            b"rolled -0\n",
            b"rolled +3\n",
            b"rolled 3",
            b"rolled 3\n\n",
            b"rolled \n",
            b"Rolled 3\n",
        ] {
            assert!(
                !matches(&expected, wrong, 0),
                "{}",
                String::from_utf8_lossy(wrong)
            );
        }
        let signed = hole(Bound::Number(-5), Bound::Number(5));
        assert!(matches(&signed, b"-5", 0));
        assert!(matches(&signed, b"0", 0));
        assert!(!matches(&signed, b"-6", 0));
    }

    #[test]
    fn a_hole_needs_a_clear_end() {
        let one = hole(Bound::Number(1), Bound::Number(6));
        assert!(crowded(&format!("{one}0")));
        assert!(crowded(&format!("{one}{one}")));
        assert!(!crowded(&format!("{one} {one}")));
        assert!(!crowded(&format!("0{one}.")));
        assert!(!crowded(&rolled()));
    }

    #[test]
    fn now_holes_follow_the_run() {
        let expected = hole(Bound::Now(-2), Bound::Now(2));
        assert!(matches(&expected, b"1735689600", 1_735_689_601));
        assert!(!matches(&expected, b"1735689600", 1_735_689_603));
        assert_eq!(show(&expected), "{any(now - 2, now + 2)}");
    }

    #[test]
    fn shows_holes_as_written_and_exact_text_as_it_is() {
        assert_eq!(show(&rolled()), "rolled {any(1, 6)}\n");
        assert_eq!(show("plain"), "plain");
        assert!(matches("plain", b"plain", 0));
        assert!(!has_holes("plain"));
        assert!(has_holes(&rolled()));
    }

    #[test]
    fn exact_block_text_may_fill_an_examples_hole() {
        assert!(agrees("rolled 4\n", &rolled(), 0));
        assert!(!agrees("rolled 9\n", &rolled(), 0));
        assert!(agrees(&rolled(), &rolled(), 0));
        assert!(!agrees(&rolled(), "rolled 4\n", 0));
        // A block computes now's bounds when it renders; an example keeps them relative.
        let computed = hole(Bound::Number(98), Bound::Number(102));
        let example = hole(Bound::Now(-2), Bound::Now(2));
        assert!(agrees(&computed, &example, 100));
        assert!(!agrees(&computed, &example, 101));
        assert!(!agrees(&computed, &rolled(), 100));
    }
}
