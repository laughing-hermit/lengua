//! AArch64's AES and SHA instructions as main.s writes them.

use super::{Line, vreg};
use crate::isa::aarch64::crypto::Crypto;
use crate::isa::aarch64::{Arrangement, Inst};

impl Line<'_> {
    /// The line as an AES or SHA instruction, or none where its mnemonic isn't one.
    pub(super) fn crypto(&self) -> Option<Result<Inst, String>> {
        let op = Crypto::named(self.mnemonic)?;
        Some(self.crypto_operands(op))
    }

    fn crypto_operands(&self, op: Crypto) -> Result<Inst, String> {
        self.count(if op.three() { 3 } else { 2 })?;
        // Each operand as its form: a vector of an arrangement, or a q or s register.
        let vector = |at: usize, arrangement: Arrangement| {
            vreg(self.operands[at])
                .filter(|register| register.arrangement == arrangement)
                .map(|register| register.number)
                .ok_or_else(|| {
                    self.problem(&format!(
                        "operand {} is a vector of {} here",
                        at + 1,
                        if arrangement == Arrangement::B16 {
                            "16 bytes, v0.16b to v31.16b"
                        } else {
                            "4 words, v0.4s to v31.4s"
                        }
                    ))
                })
        };
        let scalar = |at: usize, letter: char| {
            self.operands[at]
                .strip_prefix(letter)
                .filter(|digits| {
                    !digits.is_empty()
                        && digits.bytes().all(|byte| byte.is_ascii_digit())
                        && (*digits == "0" || !digits.starts_with('0'))
                })
                .and_then(|digits| digits.parse::<u8>().ok())
                .filter(|number| *number < 32)
                .ok_or_else(|| {
                    self.problem(&format!(
                        "operand {} is {letter}0 to {letter}31 here",
                        at + 1
                    ))
                })
        };
        let (rd, rn, rm) = match op {
            Crypto::Aese | Crypto::Aesd | Crypto::Aesmc | Crypto::Aesimc => (
                vector(0, Arrangement::B16)?,
                vector(1, Arrangement::B16)?,
                0,
            ),
            Crypto::Sha1c | Crypto::Sha1p | Crypto::Sha1m => (
                scalar(0, 'q')?,
                scalar(1, 's')?,
                vector(2, Arrangement::S4)?,
            ),
            Crypto::Sha1h => (scalar(0, 's')?, scalar(1, 's')?, 0),
            Crypto::Sha1su0 | Crypto::Sha256su1 => (
                vector(0, Arrangement::S4)?,
                vector(1, Arrangement::S4)?,
                vector(2, Arrangement::S4)?,
            ),
            Crypto::Sha1su1 | Crypto::Sha256su0 => {
                (vector(0, Arrangement::S4)?, vector(1, Arrangement::S4)?, 0)
            }
            Crypto::Sha256h | Crypto::Sha256h2 => (
                scalar(0, 'q')?,
                scalar(1, 'q')?,
                vector(2, Arrangement::S4)?,
            ),
        };
        Ok(Inst::Crypto { op, rd, rn, rm })
    }
}
