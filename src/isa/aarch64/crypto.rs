//! AArch64's AES and SHA instructions (the AES and SHA2 extensions), each an operation of whole
//! V registers: AES's rounds without the key's addition (`aese`, `aesd`) and its column mixing
//! (`aesmc`, `aesimc`), and SHA-1's and SHA-256's rounds and message steps.

use std::fmt;

/// An AES or SHA instruction's operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Crypto {
    /// `aese v0.16b, v1.16b`: the two XORed, then AES's SubBytes and ShiftRows.
    Aese,
    /// `aesd`: the two XORed, then the inverse ShiftRows and SubBytes.
    Aesd,
    /// `aesmc v0.16b, v1.16b`: AES's MixColumns of the second.
    Aesmc,
    /// `aesimc`: the inverse MixColumns.
    Aesimc,
    /// `sha1c q0, s1, v2.4s`: four SHA-1 rounds with its choose function.
    Sha1c,
    /// `sha1p`: four rounds with parity.
    Sha1p,
    /// `sha1m`: four rounds with majority.
    Sha1m,
    /// `sha1h s0, s1`: the word rotated left by 30.
    Sha1h,
    /// `sha1su0 v0.4s, v1.4s, v2.4s`: SHA-1's message schedule, first part.
    Sha1su0,
    /// `sha1su1 v0.4s, v1.4s`: its second part.
    Sha1su1,
    /// `sha256h q0, q1, v2.4s`: four SHA-256 rounds, the hash's first half.
    Sha256h,
    /// `sha256h2`: the second half.
    Sha256h2,
    /// `sha256su0 v0.4s, v1.4s`: SHA-256's message schedule, first part.
    Sha256su0,
    /// `sha256su1 v0.4s, v1.4s, v2.4s`: its second part.
    Sha256su1,
}

impl Crypto {
    /// Its mnemonic.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Aese => "aese",
            Self::Aesd => "aesd",
            Self::Aesmc => "aesmc",
            Self::Aesimc => "aesimc",
            Self::Sha1c => "sha1c",
            Self::Sha1p => "sha1p",
            Self::Sha1m => "sha1m",
            Self::Sha1h => "sha1h",
            Self::Sha1su0 => "sha1su0",
            Self::Sha1su1 => "sha1su1",
            Self::Sha256h => "sha256h",
            Self::Sha256h2 => "sha256h2",
            Self::Sha256su0 => "sha256su0",
            Self::Sha256su1 => "sha256su1",
        }
    }

    /// The operation `name` names.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        ALL.into_iter().find(|op| op.name() == name)
    }

    /// Whether it's AES's, rather than SHA's.
    #[must_use]
    pub const fn aes(self) -> bool {
        matches!(self, Self::Aese | Self::Aesd | Self::Aesmc | Self::Aesimc)
    }

    /// Whether it takes three registers.
    #[must_use]
    pub const fn three(self) -> bool {
        matches!(
            self,
            Self::Sha1c
                | Self::Sha1p
                | Self::Sha1m
                | Self::Sha1su0
                | Self::Sha256h
                | Self::Sha256h2
                | Self::Sha256su1
        )
    }

    /// Whether it reads its destination too.
    #[must_use]
    pub const fn reads_destination(self) -> bool {
        !matches!(self, Self::Aesmc | Self::Aesimc | Self::Sha1h)
    }
}

/// Every operation.
const ALL: [Crypto; 14] = [
    Crypto::Aese,
    Crypto::Aesd,
    Crypto::Aesmc,
    Crypto::Aesimc,
    Crypto::Sha1c,
    Crypto::Sha1p,
    Crypto::Sha1m,
    Crypto::Sha1h,
    Crypto::Sha1su0,
    Crypto::Sha1su1,
    Crypto::Sha256h,
    Crypto::Sha256h2,
    Crypto::Sha256su0,
    Crypto::Sha256su1,
];

/// The canonical text of `op` on V registers `rd`, `rn` and, for three, `rm`.
pub fn write(
    formatter: &mut fmt::Formatter<'_>,
    op: Crypto,
    rd: u8,
    rn: u8,
    rm: u8,
) -> fmt::Result {
    let name = op.name();
    match op {
        Crypto::Aese | Crypto::Aesd | Crypto::Aesmc | Crypto::Aesimc => {
            write!(formatter, "{name} v{rd}.16b, v{rn}.16b")
        }
        Crypto::Sha1c | Crypto::Sha1p | Crypto::Sha1m => {
            write!(formatter, "{name} q{rd}, s{rn}, v{rm}.4s")
        }
        Crypto::Sha1h => write!(formatter, "{name} s{rd}, s{rn}"),
        Crypto::Sha1su0 | Crypto::Sha256su1 => {
            write!(formatter, "{name} v{rd}.4s, v{rn}.4s, v{rm}.4s")
        }
        Crypto::Sha1su1 | Crypto::Sha256su0 => write!(formatter, "{name} v{rd}.4s, v{rn}.4s"),
        Crypto::Sha256h | Crypto::Sha256h2 => {
            write!(formatter, "{name} q{rd}, q{rn}, v{rm}.4s")
        }
    }
}

/// `word` as an AES or SHA instruction, with its registers: the operation, rd, rn and rm (0
/// for two).
#[must_use]
pub fn decode(word: u32) -> Option<(Crypto, u8, u8, u8)> {
    let (rd, rn, rm) = (
        (word & 0x1f) as u8,
        (word >> 5 & 0x1f) as u8,
        (word >> 16 & 0x1f) as u8,
    );
    if word & 0xffff_cc00 == 0x4e28_4800 {
        let op = match word >> 12 & 3 {
            0 => Crypto::Aese,
            1 => Crypto::Aesd,
            2 => Crypto::Aesmc,
            _ => Crypto::Aesimc,
        };
        return Some((op, rd, rn, 0));
    }
    if word & 0xffe0_8c00 == 0x5e00_0000 {
        let op = match word >> 12 & 7 {
            0 => Crypto::Sha1c,
            1 => Crypto::Sha1p,
            2 => Crypto::Sha1m,
            3 => Crypto::Sha1su0,
            4 => Crypto::Sha256h,
            5 => Crypto::Sha256h2,
            6 => Crypto::Sha256su1,
            _ => return None,
        };
        return Some((op, rd, rn, rm));
    }
    if word & 0xffff_cc00 == 0x5e28_0800 {
        let op = match word >> 12 & 3 {
            0 => Crypto::Sha1h,
            1 => Crypto::Sha1su1,
            2 => Crypto::Sha256su0,
            _ => return None,
        };
        return Some((op, rd, rn, 0));
    }
    None
}
