//! Work split into pieces, which an idea grants on a `- Splits:` line: `- Splits: its work into
//! up to 8 pieces, each of up to 1048576 bytes, each gives back 32 bytes`. Without the line a
//! program is one run on one core.
//!
//! A piece is a run of the same module from `_start`, with memory of its own: it gets its share
//! of the bytes as its standard input, what it prints is its answer, and it reaches nothing
//! else. The pieces of one call run at once, on as many cores as the computer has, and share no
//! memory, so they have no data race, and the program gets their answers in the pieces' order,
//! whichever ended first. The grant bounds every call: how many pieces at most, how many bytes
//! a piece gets at most, and how many bytes it gives back exactly.

use std::ops::Range;

use serde::{Deserialize, Serialize};

/// The most pieces a line may grant: each takes a thread's stack and a memory of the module's
/// size while it runs.
pub const MAX_PIECES: u32 = 64;

/// The most bytes a piece may be granted: a program's whole memory.
pub const MAX_BYTES: u32 = 1 << 30;

/// The most bytes a piece may give back.
pub const MAX_BACK: u32 = 65_536;

/// The variable that says how many pieces run at once, 1 to [`MAX_PIECES`], in place of one for
/// each core: it changes how long a run takes and nothing of what it gives.
pub const AT_ONCE: &str = "LENGUA_PIECES_AT_ONCE";

/// How an idea writes the line.
pub const USAGE: &str = "write `- Splits: its work into up to 8 pieces, each of up to 1048576 \
                         bytes, each gives back 32 bytes`: at most 64 pieces, each of at most \
                         1073741824 bytes, each gives back at most 65536 bytes";

/// What a `- Splits:` line grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant {
    /// The most pieces one call splits its bytes into.
    pub most: u32,
    /// The most bytes a piece gets.
    pub bytes: u32,
    /// How many bytes the program gets back from each piece: what the piece printed, then
    /// zeros.
    pub back: u32,
}

/// The number `text` starts with, and what follows it.
fn number(text: &str) -> Option<(u32, &str)> {
    let end = text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(text.len());
    let value = text[..end].parse::<u32>().ok()?;
    Some((value, &text[end..]))
}

impl Grant {
    /// The grant a line's words give, after `- Splits:`.
    ///
    /// # Errors
    ///
    /// What's wrong with the words, and what to write instead.
    pub fn read(line: &str) -> Result<Self, String> {
        let lower = line
            .trim()
            .trim_end_matches('.')
            .replace('`', "")
            .to_ascii_lowercase();
        let read = || -> Option<Self> {
            let rest = lower.strip_prefix("its work into up to ")?;
            let (most, rest) = number(rest)?;
            let rest = rest.strip_prefix(" pieces, each of up to ")?;
            let (bytes, rest) = number(rest)?;
            let rest = rest
                .strip_prefix(" bytes, each gives back ")
                .or_else(|| rest.strip_prefix(" byte, each gives back "))?;
            let (back, rest) = number(rest)?;
            matches!(rest, " bytes" | " byte").then_some(Self { most, bytes, back })
        };
        let grant = read().ok_or_else(|| USAGE.to_owned())?;
        if !(2..=MAX_PIECES).contains(&grant.most) {
            return Err(format!(
                "a program splits its work into 2 to {MAX_PIECES} pieces; {USAGE}"
            ));
        }
        if !(1..=MAX_BYTES).contains(&grant.bytes) {
            return Err(format!(
                "a piece gets 1 to {MAX_BYTES} bytes; {USAGE}"
            ));
        }
        if !(1..=MAX_BACK).contains(&grant.back) {
            return Err(format!(
                "a piece gives back 1 to {MAX_BACK} bytes; {USAGE}"
            ));
        }
        Ok(grant)
    }

    /// The grant in words, for the user who is about to run the program.
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "it may split its work into up to {} pieces that run at once, each a run of the \
             same program with up to {} bytes of the work as its standard input, which gives \
             back {} bytes and reaches nothing else",
            self.most, self.bytes, self.back
        )
    }

    /// Whether the grant covers a call that splits `length` bytes into `pieces` pieces and
    /// keeps `room` bytes for their answers: 1 to [`Grant::most`] pieces, none longer than
    /// [`Grant::bytes`], and room for exactly [`Grant::back`] bytes from each.
    #[must_use]
    pub fn covers(&self, length: usize, pieces: u32, room: usize) -> bool {
        let count = u64::from(pieces);
        let length = u64::try_from(length).unwrap_or(u64::MAX);
        let room = u64::try_from(room).unwrap_or(u64::MAX);
        (1..=self.most).contains(&pieces)
            && length <= count * u64::from(self.bytes)
            && room == count * u64::from(self.back)
    }
}

/// The bytes of piece `index`, from 0, of `length` bytes cut into `pieces` pieces: the pieces
/// follow each other, cover every byte once, and differ in length by one byte at most. The
/// cut depends on the two numbers alone, so it is the same on every computer.
#[must_use]
pub fn cut(length: usize, pieces: u32, index: u32) -> Range<usize> {
    let at = |index: u32| {
        let length = u128::try_from(length).unwrap_or(u128::MAX);
        let place = length * u128::from(index) / u128::from(pieces.max(1));
        usize::try_from(place).unwrap_or(usize::MAX)
    };
    at(index.min(pieces))..at(index.saturating_add(1).min(pieces))
}

/// How many pieces run at once on this computer: one for each core it gives this process, or
/// what [`AT_ONCE`] says, and never more than [`MAX_PIECES`].
#[must_use]
pub fn at_once() -> usize {
    static AT_ONCE_HERE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *AT_ONCE_HERE.get_or_init(|| {
        let most = usize::try_from(MAX_PIECES).unwrap_or(64);
        let asked = std::env::var(AT_ONCE)
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .filter(|count| (1..=most).contains(count));
        asked.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map_or(1, |cores| cores.get())
                .min(most)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_grants_its_three_numbers() {
        let grant = Grant::read(
            "its work into up to 8 pieces, each of up to 1048576 bytes, each gives back 32 bytes",
        );
        assert_eq!(
            grant,
            Ok(Grant {
                most: 8,
                bytes: 1_048_576,
                back: 32
            })
        );
        let ticked = Grant::read(
            "its work into up to `2` pieces, each of up to `1` byte, each gives back `1` byte.",
        );
        assert_eq!(
            ticked,
            Ok(Grant {
                most: 2,
                bytes: 1,
                back: 1
            })
        );
        assert!(
            grant
                .expect("a grant")
                .describe()
                .contains("up to 8 pieces")
        );
    }

    /// A line outside the bounds, or in other words, is refused with what to write.
    #[test]
    fn a_line_outside_the_bounds_is_refused() {
        for line in [
            "up to 8",
            "its work into up to 1 pieces, each of up to 10 bytes, each gives back 4 bytes",
            "its work into up to 65 pieces, each of up to 10 bytes, each gives back 4 bytes",
            "its work into up to 8 pieces, each of up to 0 bytes, each gives back 4 bytes",
            "its work into up to 8 pieces, each of up to 1073741825 bytes, each gives back 4 bytes",
            "its work into up to 8 pieces, each of up to 10 bytes, each gives back 0 bytes",
            "its work into up to 8 pieces, each of up to 10 bytes, each gives back 65537 bytes",
            "its work into up to 8 pieces, each of up to 10 bytes, each gives back 4 bytes or so",
            "its work into up to 8 pieces",
        ] {
            let refused = Grant::read(line);
            assert!(
                refused.as_ref().is_err_and(|why| why.contains("- Splits:")),
                "{line}: {refused:?}"
            );
        }
    }

    /// A call is inside its grant by its pieces, by its longest piece and by its room, each
    /// at its limit and one past it.
    #[test]
    fn a_grant_covers_a_call_inside_its_bounds() {
        let grant = Grant {
            most: 4,
            bytes: 10,
            back: 8,
        };
        assert!(grant.covers(40, 4, 32));
        assert!(grant.covers(0, 1, 8));
        assert!(!grant.covers(40, 5, 40), "a piece too many");
        assert!(!grant.covers(40, 0, 0), "no piece");
        assert!(!grant.covers(41, 4, 32), "a piece too long");
        assert!(!grant.covers(11, 1, 8), "one piece too long");
        assert!(!grant.covers(40, 4, 31), "too little room");
        assert!(!grant.covers(40, 4, 33), "too much room");
    }

    /// The pieces follow each other, cover every byte once, and differ by a byte at most,
    /// whatever the length, more pieces than bytes included.
    #[test]
    fn the_cut_covers_every_byte_once() {
        for length in [0_usize, 1, 2, 7, 64, 1000, 1_048_577] {
            for pieces in [1_u32, 2, 3, 8, 64] {
                let mut next = 0;
                let (mut shortest, mut longest) = (usize::MAX, 0);
                for index in 0..pieces {
                    let range = cut(length, pieces, index);
                    assert_eq!(range.start, next, "{length} in {pieces}, piece {index}");
                    next = range.end;
                    shortest = shortest.min(range.len());
                    longest = longest.max(range.len());
                }
                assert_eq!(next, length, "{length} in {pieces}");
                assert!(longest - shortest <= 1, "{length} in {pieces}");
            }
        }
        assert_eq!(cut(10, 4, 0), 0..2);
        assert_eq!(cut(10, 4, 1), 2..5);
        assert_eq!(cut(10, 4, 3), 7..10);
        // An index past the pieces names nothing.
        assert_eq!(cut(10, 4, 4), 10..10);
    }
}
