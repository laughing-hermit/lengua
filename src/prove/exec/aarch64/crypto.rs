//! AArch64's AES and SHA instructions in the prover's model, as the Arm architecture's
//! pseudocode defines them, built from the pieces x86-64's model of the same algorithms has
//! (`exec/vector/crypto.rs`): `aese` is x86-64's last round of the two XORed with no key, and the
//! SHA rounds and message steps are its words' functions.

use super::super::{Cpu, Step, Stop};
use crate::isa::aarch64::crypto::Crypto;
use crate::prove::term::Term;

impl Cpu<'_> {
    /// A word rotated left by `by`.
    fn rotate_left(&mut self, word: Term, by: u32) -> Term {
        self.rotate_right(word, 32 - by)
    }

    /// SHA's parity of three words.
    fn parity(&mut self, x: Term, y: Term, z: Term) -> Term {
        self.xor3(x, y, z)
    }

    /// Four SHA-1 rounds, as `sha1c`, `sha1p` and `sha1m` take them: the state's words a to d
    /// in `x`, e in `y`, and the message words with their constants added in `w`.
    fn sha1_rounds(
        &mut self,
        op: Crypto,
        x: [Term; 4],
        y: Term,
        w: [Term; 4],
    ) -> Result<[Term; 4], Stop> {
        let (mut x, mut y) = (x, y);
        for word in w {
            let t = match op {
                Crypto::Sha1c => self.choose(x[1], x[2], x[3]),
                Crypto::Sha1p => self.parity(x[1], x[2], x[3]),
                Crypto::Sha1m => self.majority(x[1], x[2], x[3]),
                _ => return Err(self.unmodelled(op)),
            };
            let rotated = self.rotate_left(x[0], 5);
            y = self.sum(&[y, rotated, t, word]);
            x[1] = self.rotate_left(x[1], 30);
            // The 160 bits of y and x, rotated left by a word.
            let top = x[3];
            x = [y, x[0], x[1], x[2]];
            y = top;
        }
        Ok(x)
    }

    /// SHA-256's four rounds as `sha256h` (the first half, `x`) and `sha256h2` (the second, `y`)
    /// compute them, with the message words and constants in `w`.
    fn sha256_rounds(
        &mut self,
        x: [Term; 4],
        y: [Term; 4],
        w: [Term; 4],
        first: bool,
    ) -> [Term; 4] {
        let (mut x, mut y) = (x, y);
        for word in w {
            let chosen = self.choose(y[0], y[1], y[2]);
            let most = self.majority(x[0], x[1], x[2]);
            let sigma1 = {
                let (a, b, c) = (
                    self.rotate_right(y[0], 6),
                    self.rotate_right(y[0], 11),
                    self.rotate_right(y[0], 25),
                );
                self.xor3(a, b, c)
            };
            let t = self.sum(&[y[3], sigma1, chosen, word]);
            x[3] = self.sum(&[t, x[3]]);
            let sigma0 = {
                let (a, b, c) = (
                    self.rotate_right(x[0], 2),
                    self.rotate_right(x[0], 13),
                    self.rotate_right(x[0], 22),
                );
                self.xor3(a, b, c)
            };
            y[3] = self.sum(&[t, sigma0, most]);
            // The 256 bits of y and x, rotated left by a word.
            let (top_x, top_y) = (x[3], y[3]);
            x = [top_y, x[0], x[1], x[2]];
            y = [top_x, y[0], y[1], y[2]];
        }
        if first { x } else { y }
    }

    /// SHA-256's small sigma of a message word: the rotations by `a` and `b` and the shift by
    /// `c`, XORed.
    fn small_sigma(&mut self, word: Term, a: u32, b: u32, c: u32) -> Term {
        let (first, second, shifted) = (
            self.rotate_right(word, a),
            self.rotate_right(word, b),
            self.shift_right(word, c),
        );
        self.xor3(first, second, shifted)
    }

    /// Runs the AES or SHA instruction `op` on V registers `rd`, `rn` and `rm`.
    pub(super) fn execute_crypto(
        &mut self,
        op: Crypto,
        rd: u8,
        rn: u8,
        rm: u8,
    ) -> Result<Step, Stop> {
        let (d, n, m) = (self.v_bytes(rd), self.v_bytes(rn), self.v_bytes(rm));
        let result: Vec<Term> = match op {
            Crypto::Aese | Crypto::Aesd => {
                let state: Vec<Term> = d
                    .iter()
                    .zip(&n)
                    .map(|(a, b)| self.pool.xor(*a, *b))
                    .collect();
                let no_key = vec![self.pool.bv(8, 0); 16];
                self.aes_round(&state, &no_key, op == Crypto::Aesd, true)
            }
            Crypto::Aesmc => self.mix_columns(&n, false),
            Crypto::Aesimc => self.mix_columns(&n, true),
            Crypto::Sha1c | Crypto::Sha1p | Crypto::Sha1m => {
                let (x, w) = (self.words(&d), self.words(&m));
                let y = self.word(&n[..4]);
                let words = self.sha1_rounds(op, x, y, w)?;
                self.unwords(words)
            }
            Crypto::Sha1h => {
                let word = self.word(&n[..4]);
                let rotated = self.rotate_left(word, 30);
                self.split(rotated, 4)
            }
            Crypto::Sha1su0 => {
                let (a, b, c) = (self.words(&d), self.words(&n), self.words(&m));
                // b's low doubleword above a's high one, then XORed with a and c.
                let joined = [a[2], a[3], b[0], b[1]];
                let words = [0, 1, 2, 3].map(|at| self.xor3(joined[at], a[at], c[at]));
                self.unwords(words)
            }
            Crypto::Sha1su1 => {
                let (a, b) = (self.words(&d), self.words(&n));
                let zero = self.pool.bv(32, 0);
                // b shifted right by a word.
                let shifted = [b[1], b[2], b[3], zero];
                let t = [0, 1, 2, 3].map(|at| self.pool.xor(a[at], shifted[at]));
                let mut words = t.map(|word| self.rotate_left(word, 1));
                let extra = self.rotate_left(t[0], 2);
                words[3] = self.pool.xor(words[3], extra);
                self.unwords(words)
            }
            Crypto::Sha256h | Crypto::Sha256h2 => {
                let (a, b, w) = (self.words(&d), self.words(&n), self.words(&m));
                let words = if op == Crypto::Sha256h {
                    self.sha256_rounds(a, b, w, true)
                } else {
                    self.sha256_rounds(b, a, w, false)
                };
                self.unwords(words)
            }
            Crypto::Sha256su0 => {
                let (a, b) = (self.words(&d), self.words(&n));
                let t = [a[1], a[2], a[3], b[0]];
                let words = [0, 1, 2, 3].map(|at| {
                    let sigma = self.small_sigma(t[at], 7, 18, 3);
                    self.sum(&[sigma, a[at]])
                });
                self.unwords(words)
            }
            Crypto::Sha256su1 => {
                let (a, b, c) = (self.words(&d), self.words(&n), self.words(&m));
                let t0 = [b[1], b[2], b[3], c[0]];
                let mut words = [a[0]; 4];
                let mut t1 = [c[2], c[3]];
                for at in 0..4 {
                    if at == 2 {
                        t1 = [words[0], words[1]];
                    }
                    let sigma = self.small_sigma(t1[at % 2], 17, 19, 10);
                    words[at] = self.sum(&[sigma, a[at], t0[at]]);
                }
                self.unwords(words)
            }
        };
        self.set_v_bytes(rd, &result);
        Ok(Step::Next)
    }
}
