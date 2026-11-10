//! AArch64 machine code: which words of a stretch of code branch on a condition, jump or call
//! through a register, call, divide, or take the address of data in the code. Every instruction
//! is one 32-bit word, and the architecture's encoding index sorts each word by a few fixed bits
//! (Arm Architecture Reference Manual for A-profile, C4.1, "A64 instruction set encoding"), so
//! the kinds the rule has a word for are read from those bits and nothing else of an instruction
//! is decoded.

use super::{Made, Read, Seen};

/// What `word` is, with the instruction's name, when the rule has a word for it.
fn made(word: u32) -> Option<(Made, &'static str)> {
    // Bits 28 to 25 name the group.
    match (word >> 25) & 0xF {
        // Reserved: `udf`, the trap the engine ends a failed check with, has its top half zero.
        0b0000 if word >> 16 == 0 => None,
        // Reserved, unallocated, and SVE, which the engine doesn't make.
        0b0000..=0b0011 => Some((Made::Unread, "a word outside the instructions lengua reads")),
        // Data processing with an immediate: `adr` and `adrp` give an address in the code.
        0b1000 | 0b1001 if word & 0x1F00_0000 == 0x1000_0000 => {
            Some((Made::Table, if word >> 31 == 0 { "adr" } else { "adrp" }))
        }
        0b1010 | 0b1011 => branch(word),
        // Data processing with registers: the divisions.
        0b0101 | 0b1101 if word & 0x7FE0_F800 == 0x1AC0_0800 => Some((
            Made::Division,
            if word & 0x400 == 0 { "udiv" } else { "sdiv" },
        )),
        0b0111 | 0b1111 => float(word),
        // Loads and stores, and everything else that computes.
        _ => None,
    }
}

/// What `word`, of the group of branches, exceptions and system instructions, is.
const fn branch(word: u32) -> Option<(Made, &'static str)> {
    if word & 0x7C00_0000 == 0x1400_0000 {
        // `b` goes one way whatever the data is; `bl` calls.
        return if word >> 31 == 0 {
            None
        } else {
            Some((Made::Call, "bl"))
        };
    }
    if word & 0x7E00_0000 == 0x3400_0000 {
        return Some((
            Made::Branch,
            if word & 0x0100_0000 == 0 {
                "cbz"
            } else {
                "cbnz"
            },
        ));
    }
    if word & 0x7E00_0000 == 0x3600_0000 {
        return Some((
            Made::Branch,
            if word & 0x0100_0000 == 0 {
                "tbz"
            } else {
                "tbnz"
            },
        ));
    }
    if word & 0xFE00_0000 == 0x5400_0000 {
        return Some((Made::Branch, "b.cond"));
    }
    if word & 0xFE00_0000 == 0xD600_0000 {
        // Through a register: `br`, `blr` and `ret`, by bits 22 and 21.
        return match (word >> 21) & 0b11 {
            0b00 => Some((Made::IndirectJump, "br")),
            0b01 => Some((Made::IndirectCall, "blr")),
            0b10 => None,
            _ => Some((Made::Unread, "a word outside the instructions lengua reads")),
        };
    }
    // Exceptions and system instructions, such as `brk` and `nop`.
    None
}

/// What `word`, of the two groups of vector and float instructions, is: a division or a square
/// root, of one float or of a vector of them.
const fn float(word: u32) -> Option<(Made, &'static str)> {
    if word & 0xFF20_FC00 == 0x1E20_1800 || word & 0xBFA0_FC00 == 0x2E20_FC00 {
        return Some((Made::Division, "fdiv"));
    }
    if word & 0xFF3F_FC00 == 0x1E21_C000 || word & 0xBFBF_FC00 == 0x2EA1_F800 {
        return Some((Made::Division, "fsqrt"));
    }
    None
}

/// Whether `word` loads a register from the code eight bytes on: just past the word after it.
const fn loads_past_next(word: u32) -> bool {
    word & 0x3B00_0000 == 0x1800_0000 && (word >> 5) & 0x7_FFFF == 2
}

/// How many words `word` jumps over, when it is a `b` forward: those between it and where it
/// goes.
fn jumps_over(word: u32) -> Option<usize> {
    let distance = word & 0x03FF_FFFF;
    (word & 0xFC00_0000 == 0x1400_0000 && (1..0x0200_0000).contains(&distance))
        .then(|| usize::try_from(distance - 1).ok())
        .flatten()
}

/// Reads `code`, which starts at an instruction, `at` bytes into the engine's code: how many
/// instructions it holds, and each the rule has a word for. The engine keeps a constant inside
/// the code as `ldr` of the bytes after the next word, then `b` over them: those bytes are data,
/// and aren't read as instructions.
pub fn read(code: &[u8], at: usize) -> Read {
    let word_at = |index: usize| -> Option<u32> {
        let start = index.checked_mul(4)?;
        let bytes: [u8; 4] = code.get(start..start.checked_add(4)?)?.try_into().ok()?;
        Some(u32::from_le_bytes(bytes))
    };
    let mut read = Read::default();
    let mut index = 0_usize;
    while let Some(word) = word_at(index) {
        read.instructions += 1;
        if let Some((made, name)) = made(word) {
            read.seen.push(Seen {
                at: at.saturating_add(index.saturating_mul(4)),
                made,
                name: name.to_owned(),
            });
        }
        index += 1;
        if loads_past_next(word)
            && let Some(data) = word_at(index).and_then(jumps_over)
        {
            // The `b`, then the constant's words.
            read.instructions += 1;
            index = index.saturating_add(1).saturating_add(data);
        }
    }
    if !code.len().is_multiple_of(4) {
        read.seen.push(Seen {
            at: at.saturating_add(code.len() / 4 * 4),
            made: Made::Unread,
            name: "a part of a word".to_owned(),
        });
    }
    read
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bytes of `words`, as the code holds them.
    fn code(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    /// What the rule sees in `words`.
    fn seen(words: &[u32]) -> Vec<(usize, Made, String)> {
        read(&code(words), 0)
            .seen
            .into_iter()
            .map(|seen| (seen.at, seen.made, seen.name))
            .collect()
    }

    /// Each kind of instruction the rule has a word for is read as that kind, from its word.
    #[test]
    fn each_kind_is_read_from_its_word() {
        let kinds = [
            // b.eq +8
            (0x5400_0040, Made::Branch, "b.cond"),
            // cbz x0, +8
            (0xB400_0040, Made::Branch, "cbz"),
            // cbnz w0, +8
            (0x3500_0040, Made::Branch, "cbnz"),
            // tbz w0, #0, +8
            (0x3600_0040, Made::Branch, "tbz"),
            // tbnz w0, #0, +8
            (0x3700_0040, Made::Branch, "tbnz"),
            // br x16
            (0xD61F_0200, Made::IndirectJump, "br"),
            // blr x16
            (0xD63F_0200, Made::IndirectCall, "blr"),
            // bl +4
            (0x9400_0001, Made::Call, "bl"),
            // udiv w0, w1, w2
            (0x1AC2_0820, Made::Division, "udiv"),
            // sdiv x0, x1, x2
            (0x9AC2_0C20, Made::Division, "sdiv"),
            // fdiv d0, d1, d2
            (0x1E62_1820, Made::Division, "fdiv"),
            // fdiv v0.4s, v1.4s, v2.4s
            (0x6E22_FC20, Made::Division, "fdiv"),
            // fsqrt d0, d1
            (0x1E61_C020, Made::Division, "fsqrt"),
            // fsqrt v0.4s, v1.4s
            (0x6EA1_F820, Made::Division, "fsqrt"),
            // adr x0, +8
            (0x1000_0040, Made::Table, "adr"),
            // adrp x0, +0
            (0x9000_0000, Made::Table, "adrp"),
        ];
        for (word, made, name) in kinds {
            assert_eq!(seen(&[word]), [(0, made, name.to_owned())], "{word:#010x}");
        }
        // A word of a group with no instructions, and one of SVE's.
        for word in [0x0200_0000_u32, 0x0420_0000, 0x0001_0000] {
            let found = seen(&[word]);
            assert_eq!(found.len(), 1, "{word:#010x}");
            assert_eq!(found[0].1, Made::Unread, "{word:#010x}");
        }
    }

    /// Straight code has nothing the rule has a word for: arithmetic, a conditional select, a
    /// load through registers, a constant loaded from the code, a jump that no condition
    /// decides, a return, and the trap a failed check ends in.
    #[test]
    fn straight_code_has_nothing_to_refuse() {
        let words = [
            0x4A02_0020, // eor w0, w1, w2
            0x8B02_0020, // add x0, x1, x2
            0x1B02_7C20, // mul w0, w1, w2
            0x1A82_0020, // csel w0, w1, w2, eq
            0x3862_6820, // ldrb w0, [x1, x2]
            0xB900_0020, // str w0, [x1]
            0x5800_0040, // ldr x0, +8
            0x4E22_1C20, // and v0.16b, v1.16b, v2.16b
            0x1400_0002, // b +8
            0xD65F_03C0, // ret
            0x0000_C11F, // udf #0xc11f
        ];
        let read = read(&code(&words), 0);
        assert_eq!(read.instructions, words.len());
        assert!(read.seen.is_empty(), "{:?}", read.seen);
    }

    /// A constant the engine keeps inside the code isn't read as instructions: 16 bytes here,
    /// which would read as two conditional branches, after `ldr q0, +8` and `b +20`. The branch
    /// after them is read, at its place.
    #[test]
    fn a_constant_inside_the_code_isnt_read_as_instructions() {
        let words = [
            0x9C00_0040, // ldr q0, +8
            0x1400_0005, // b +20
            0x5400_0040,
            0x5400_0040,
            0x5400_0040,
            0x5400_0040,
            0xB400_0040, // cbz x0, +8
        ];
        let read = read(&code(&words), 64);
        assert_eq!(read.instructions, 3);
        let found: Vec<(usize, Made)> = read.seen.iter().map(|seen| (seen.at, seen.made)).collect();
        assert_eq!(found, [(64 + 24, Made::Branch)]);
        // Without the load before it, a `b` skips nothing: what it jumps over is code.
        let branches = seen(&[0x1400_0002, 0x5400_0040]);
        assert_eq!(branches.len(), 1, "{branches:?}");
    }

    /// Code that ends inside a word is refused, not passed.
    #[test]
    fn a_part_of_a_word_cant_be_read() {
        let mut bytes = code(&[0x4A02_0020]);
        bytes.extend([0x20, 0x00]);
        let read = read(&bytes, 0);
        assert_eq!(read.seen.len(), 1);
        assert_eq!((read.seen[0].at, read.seen[0].made), (4, Made::Unread));
    }
}
