//! Random bytes: the system's, or a test's supply in order with zeros past its end, which makes
//! output that depends on chance exact. A program whose idea doesn't name random bytes gets
//! zeros, harmlessly, since a C or Rust toolchain's startup asks for a few.

use super::{Fail, Host, memory};
use crate::wasm::wasi::errno;

/// A run's random bytes.
#[derive(Debug, Default)]
pub struct Random {
    /// What a test supplies, if it does.
    supplied: Option<Vec<u8>>,
    /// How many bytes the program asked for, when the test supplies them.
    asked: usize,
}

impl Random {
    /// Random bytes, from `supplied` when a test gives them.
    #[must_use]
    pub fn new(supplied: Option<Vec<u8>>) -> Self {
        Self { supplied, asked: 0 }
    }

    /// How many bytes the program asked for, when the test supplied them.
    #[must_use]
    pub fn asked(&self) -> Option<usize> {
        self.supplied.as_ref().map(|_| self.asked)
    }

    /// Fills `out`.
    ///
    /// # Errors
    ///
    /// `IO` when the system has none to give.
    pub fn fill(&mut self, out: &mut [u8]) -> Result<(), u16> {
        match &self.supplied {
            Some(bytes) => {
                let start = self.asked.min(bytes.len());
                let given = bytes.len().saturating_sub(start).min(out.len());
                out[..given].copy_from_slice(&bytes[start..start + given]);
                out[given..].fill(0);
                self.asked = self.asked.saturating_add(out.len());
                Ok(())
            }
            None => getrandom::fill(out).map_err(|_| errno::IO),
        }
    }
}

/// `random_get(buf, len)`.
///
/// # Errors
///
/// A buffer outside memory.
pub fn random_get(host: &mut Host, memory: &mut [u8], buf: u32, len: u32) -> Result<(), Fail> {
    let range = memory::range(memory, buf, len)?;
    if !host.policy.random {
        memory[range].fill(0);
        return Ok(());
    }
    host.random.fill(&mut memory[range]).map_err(Fail::Errno)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_supply_is_given_in_order_with_zeros_past_its_end() {
        let mut random = Random::new(Some(vec![1, 2, 3]));
        let mut first = [9_u8; 2];
        random.fill(&mut first).expect("bytes");
        assert_eq!(first, [1, 2]);
        let mut rest = [9_u8; 3];
        random.fill(&mut rest).expect("bytes");
        assert_eq!(rest, [3, 0, 0]);
        assert_eq!(random.asked(), Some(5));
    }

    #[test]
    fn the_system_gives_bytes_without_a_supply() {
        let mut random = Random::new(None);
        let mut bytes = [0_u8; 32];
        random.fill(&mut bytes).expect("the system's bytes");
        assert_eq!(random.asked(), None);
    }
}
