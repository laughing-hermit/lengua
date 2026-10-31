//! The cipher, hash and carry-less multiply instructions, as the CPU computes them: AES's rounds
//! and key expansion on each 16-byte lane, SHA-1's and SHA-256's rounds and message steps, and
//! `pclmulqdq`'s product of two 64-bit polynomials over GF(2). Each is plain bit arithmetic on
//! the bytes' terms, with the S-box a table its byte picks from, so known values fold to
//! constants and a proof follows the rest.

use iced_x86::{EncodingKind, Mnemonic};

use super::super::{Cpu, Stop};
use crate::prove::term::Term;

/// The AES S-box and its inverse, worked out once from GF(2^8): each byte's inverse, 0 for 0,
/// through the affine map.
fn sboxes() -> &'static ([u8; 256], [u8; 256]) {
    static TABLES: std::sync::OnceLock<([u8; 256], [u8; 256])> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| {
        let multiply = |mut a: u8, mut b: u8| {
            let mut product = 0_u8;
            while b != 0 {
                if b & 1 != 0 {
                    product ^= a;
                }
                let high = a & 0x80 != 0;
                a <<= 1;
                if high {
                    a ^= 0x1b;
                }
                b >>= 1;
            }
            product
        };
        let (mut forward, mut inverse) = ([0_u8; 256], [0_u8; 256]);
        for x in 0..=255_u8 {
            let mut power = 1_u8;
            for _ in 0..254 {
                power = multiply(power, x);
            }
            let b = if x == 0 { 0 } else { power };
            let s = b
                ^ b.rotate_left(1)
                ^ b.rotate_left(2)
                ^ b.rotate_left(3)
                ^ b.rotate_left(4)
                ^ 0x63;
            forward[usize::from(x)] = s;
            inverse[usize::from(s)] = x;
        }
        (forward, inverse)
    })
}

/// SHA-1's round constants, by the function its rounds use.
const SHA1_K: [u32; 4] = [0x5A82_7999, 0x6ED9_EBA1, 0x8F1B_BCDC, 0xCA62_C1D6];

impl Cpu<'_> {
    /// Four bytes, lowest first, as one 32-bit word.
    fn word(&mut self, bytes: &[Term]) -> Term {
        self.join(bytes)
    }

    /// 16 bytes as their four 32-bit words, word 0 the lowest.
    fn words(&mut self, bytes: &[Term]) -> [Term; 4] {
        [0, 1, 2, 3].map(|at| self.word(&bytes[4 * at..4 * at + 4]))
    }

    /// Four words back into 16 bytes.
    fn unwords(&mut self, words: [Term; 4]) -> Vec<Term> {
        words.iter().flat_map(|word| self.split(*word, 4)).collect()
    }

    /// A 32-bit word rotated right by `by`.
    fn rotate_right(&mut self, word: Term, by: u32) -> Term {
        let (right, left) = (
            self.pool.bv(32, u128::from(by)),
            self.pool.bv(32, u128::from(32 - by)),
        );
        let low = self.pool.lshr(word, right);
        let high = self.pool.shl(word, left);
        self.pool.or(low, high)
    }

    /// A word shifted right by `by`, zeros coming in.
    fn shift_right(&mut self, word: Term, by: u32) -> Term {
        let amount = self.pool.bv(32, u128::from(by));
        self.pool.lshr(word, amount)
    }

    fn xor3(&mut self, a: Term, b: Term, c: Term) -> Term {
        let ab = self.pool.xor(a, b);
        self.pool.xor(ab, c)
    }

    /// `Ch(e, f, g)`: f where e's bit is set, g where it's clear.
    fn choose(&mut self, e: Term, f: Term, g: Term) -> Term {
        let ef = self.pool.and(e, f);
        let not_e = self.pool.not(e);
        let eg = self.pool.and(not_e, g);
        self.pool.xor(ef, eg)
    }

    /// `Maj(a, b, c)`: each bit as most of the three are.
    fn majority(&mut self, a: Term, b: Term, c: Term) -> Term {
        let (ab, ac, bc) = (
            self.pool.and(a, b),
            self.pool.and(a, c),
            self.pool.and(b, c),
        );
        self.xor3(ab, ac, bc)
    }

    fn sum(&mut self, terms: &[Term]) -> Term {
        self.pool.sum(32, terms)
    }

    /// A byte through the AES S-box, or its inverse.
    fn sub_byte(&mut self, byte: Term, inverse: bool) -> Term {
        let (forward, backward) = sboxes();
        let table = if inverse { backward } else { forward };
        if let Some(known) = self.pool.constant(byte) {
            return self.pool.bv(8, u128::from(table[known as usize % 256]));
        }
        let entries: Vec<Term> = table
            .iter()
            .map(|value| self.pool.bv(8, u128::from(*value)))
            .collect();
        self.pick(&entries, byte)
    }

    /// A byte times 2 in GF(2^8).
    fn xtime(&mut self, byte: Term) -> Term {
        let one = self.pool.bv(8, 1);
        let shifted = self.pool.shl(byte, one);
        let high = self.pool.bit_is_set(byte, 7);
        let reduction = self.pool.bv(8, 0x1b);
        let reduced = self.pool.xor(shifted, reduction);
        self.pool.ite(high, reduced, shifted)
    }

    /// A byte times the constant `by` in GF(2^8).
    fn gf_times(&mut self, byte: Term, by: u8) -> Term {
        let mut product = self.pool.bv(8, 0);
        let (mut power, mut by) = (byte, by);
        while by != 0 {
            if by & 1 != 0 {
                product = self.pool.xor(product, power);
            }
            by >>= 1;
            if by != 0 {
                power = self.xtime(power);
            }
        }
        product
    }

    /// MixColumns, or InvMixColumns, on the four columns of a 16-byte state.
    fn mix_columns(&mut self, state: &[Term], inverse: bool) -> Vec<Term> {
        let matrix: [[u8; 4]; 4] = if inverse {
            [
                [14, 11, 13, 9],
                [9, 14, 11, 13],
                [13, 9, 14, 11],
                [11, 13, 9, 14],
            ]
        } else {
            [[2, 3, 1, 1], [1, 2, 3, 1], [1, 1, 2, 3], [3, 1, 1, 2]]
        };
        let mut out = Vec::with_capacity(16);
        for column in 0..4 {
            for row in matrix {
                let mut byte = self.pool.bv(8, 0);
                for (at, by) in row.iter().enumerate() {
                    let term = self.gf_times(state[4 * column + at], *by);
                    byte = self.pool.xor(byte, term);
                }
                out.push(byte);
            }
        }
        out
    }

    /// One AES round on a 16-byte lane, as `aesenc` (`last` without MixColumns) or, `decrypt`,
    /// `aesdec` computes it: ShiftRows and SubBytes, or their inverses, then MixColumns or its
    /// inverse, then the round key.
    fn aes_round(&mut self, state: &[Term], key: &[Term], decrypt: bool, last: bool) -> Vec<Term> {
        let mut shifted = Vec::with_capacity(16);
        for column in 0..4 {
            for row in 0..4 {
                let from = if decrypt {
                    (column + 4 - row) % 4
                } else {
                    (column + row) % 4
                };
                shifted.push(state[4 * from + row]);
            }
        }
        let substituted: Vec<Term> = shifted
            .into_iter()
            .map(|byte| self.sub_byte(byte, decrypt))
            .collect();
        let mixed = if last {
            substituted
        } else {
            self.mix_columns(&substituted, decrypt)
        };
        mixed
            .iter()
            .zip(key)
            .map(|(byte, round)| self.pool.xor(*byte, *round))
            .collect()
    }

    /// Each byte of a word through the S-box.
    fn sub_word(&mut self, word: Term) -> Term {
        let bytes = self.split(word, 4);
        let substituted: Vec<Term> = bytes
            .into_iter()
            .map(|byte| self.sub_byte(byte, false))
            .collect();
        self.join(&substituted)
    }

    /// The carry-less product of two 64-bit polynomials over GF(2), 128 bits wide.
    fn carry_less(&mut self, a: Term, b: Term) -> Term {
        let wide = self.pool.zero_extend(a, 64);
        let mut product = self.pool.bv(128, 0);
        for at in 0..64 {
            let bit = self.pool.extract(b, at, at);
            let amount = self.pool.bv(128, u128::from(at));
            let shifted = self.pool.shl(wide, amount);
            let term = match self.pool.constant(bit) {
                Some(0) => continue,
                Some(_) => shifted,
                None => {
                    let one = self.pool.bv(1, 1);
                    let set = self.pool.eq(bit, one);
                    let zero = self.pool.bv(128, 0);
                    self.pool.ite(set, shifted, zero)
                }
            };
            product = self.pool.xor(product, term);
        }
        product
    }

    /// The two sources of a VEX or EVEX instruction, or of legacy SSE's destination and source.
    fn crypto_sources(&mut self, bytes: usize) -> Result<(Vec<Term>, Vec<Term>), Stop> {
        if self.instruction.encoding() == EncodingKind::Legacy {
            Ok((self.bytes_in(0, bytes)?, self.bytes_in(1, bytes)?))
        } else {
            Ok((self.bytes_in(1, bytes)?, self.bytes_in(2, bytes)?))
        }
    }

    /// The cipher, hash and carry-less multiply instructions, or `None` for any other.
    pub(super) fn crypto(&mut self, mnemonic: Mnemonic) -> Option<Result<(), Stop>> {
        let result = match mnemonic {
            Mnemonic::Aesenc
            | Mnemonic::Aesenclast
            | Mnemonic::Aesdec
            | Mnemonic::Aesdeclast
            | Mnemonic::Vaesenc
            | Mnemonic::Vaesenclast
            | Mnemonic::Vaesdec
            | Mnemonic::Vaesdeclast => self.aes(mnemonic),
            Mnemonic::Aesimc | Mnemonic::Vaesimc => self.bytes_in(1, 16).and_then(|source| {
                let mixed = self.mix_columns(&source, true);
                self.bytes_out(0, &mixed)
            }),
            Mnemonic::Aeskeygenassist | Mnemonic::Vaeskeygenassist => self.key_assist(),
            Mnemonic::Sha1rnds4
            | Mnemonic::Sha1nexte
            | Mnemonic::Sha1msg1
            | Mnemonic::Sha1msg2
            | Mnemonic::Sha256rnds2
            | Mnemonic::Sha256msg1
            | Mnemonic::Sha256msg2 => self.sha(mnemonic),
            Mnemonic::Pclmulqdq | Mnemonic::Vpclmulqdq => self.pclmul(),
            _ => return None,
        };
        Some(result)
    }

    fn aes(&mut self, mnemonic: Mnemonic) -> Result<(), Stop> {
        let bytes = self.operand_bytes(0).max(16);
        let (state, key) = self.crypto_sources(bytes)?;
        let decrypt = matches!(
            mnemonic,
            Mnemonic::Aesdec | Mnemonic::Aesdeclast | Mnemonic::Vaesdec | Mnemonic::Vaesdeclast
        );
        let last = matches!(
            mnemonic,
            Mnemonic::Aesenclast
                | Mnemonic::Aesdeclast
                | Mnemonic::Vaesenclast
                | Mnemonic::Vaesdeclast
        );
        let mut out = Vec::with_capacity(bytes);
        for lane in 0..bytes / 16 {
            let lane = 16 * lane..16 * lane + 16;
            out.extend(self.aes_round(&state[lane.clone()], &key[lane], decrypt, last));
        }
        self.bytes_out(0, &out)
    }

    /// `aeskeygenassist`: from the source's words 1 and 3, each through the S-box, and each
    /// rotated by a byte with the round constant xored in.
    fn key_assist(&mut self) -> Result<(), Stop> {
        let source = self.bytes_in(1, 16)?;
        let words = self.words(&source);
        let constant = self.pool.bv(32, u128::from(self.instruction.immediate8()));
        let (low, high) = (self.sub_word(words[1]), self.sub_word(words[3]));
        let rotated_low = self.rotate_right(low, 8);
        let rotated_high = self.rotate_right(high, 8);
        let out = [
            low,
            self.pool.xor(rotated_low, constant),
            high,
            self.pool.xor(rotated_high, constant),
        ];
        let bytes = self.unwords(out);
        self.bytes_out(0, &bytes)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per SHA instruction, as Intel defines it"
    )]
    fn sha(&mut self, mnemonic: Mnemonic) -> Result<(), Stop> {
        let (first, second) = (self.bytes_in(0, 16)?, self.bytes_in(1, 16)?);
        let (a, b) = (self.words(&first), self.words(&second));
        let out: [Term; 4] = match mnemonic {
            Mnemonic::Sha1rnds4 => {
                let select = self.instruction.immediate8() & 3;
                let k = self.pool.bv(32, u128::from(SHA1_K[usize::from(select)]));
                // A to D from the destination's words 3 to 0; W0 + E and W1 to W3 from the
                // source's.
                let (mut va, mut vb, mut vc, mut vd) = (a[3], a[2], a[1], a[0]);
                let mut ve = self.pool.bv(32, 0);
                let w = [b[3], b[2], b[1], b[0]];
                for (round, word) in w.iter().enumerate() {
                    let f = match select {
                        0 => self.choose(vb, vc, vd),
                        2 => self.majority(vb, vc, vd),
                        _ => self.xor3(vb, vc, vd),
                    };
                    let rotated = self.rotate_right(va, 27);
                    // The first round's word already holds E.
                    let terms = if round == 0 {
                        vec![f, rotated, *word, k]
                    } else {
                        vec![f, rotated, *word, ve, k]
                    };
                    let next = self.sum(&terms);
                    ve = vd;
                    vd = vc;
                    vc = self.rotate_right(vb, 2);
                    vb = va;
                    va = next;
                }
                [vd, vc, vb, va]
            }
            Mnemonic::Sha1nexte => {
                let rotated = self.rotate_right(a[3], 2);
                let top = self.pool.add(b[3], rotated);
                [b[0], b[1], b[2], top]
            }
            Mnemonic::Sha1msg1 => {
                // W0 to W3 from the destination's words 3 to 0, W4 and W5 from the source's 3 and 2.
                let (w0, w1, w2, w3, w4, w5) = (a[3], a[2], a[1], a[0], b[3], b[2]);
                [
                    self.pool.xor(w5, w3),
                    self.pool.xor(w4, w2),
                    self.pool.xor(w3, w1),
                    self.pool.xor(w2, w0),
                ]
            }
            Mnemonic::Sha1msg2 => {
                let (w13, w14, w15) = (b[2], b[1], b[0]);
                let rol1 = |cpu: &mut Self, x: Term, y: Term| {
                    let mixed = cpu.pool.xor(x, y);
                    cpu.rotate_right(mixed, 31)
                };
                let w16 = rol1(self, a[3], w13);
                let w17 = rol1(self, a[2], w14);
                let w18 = rol1(self, a[1], w15);
                let w19 = rol1(self, a[0], w16);
                [w19, w18, w17, w16]
            }
            Mnemonic::Sha256rnds2 => {
                let k = self.state.vectors[0][..8].to_vec();
                let wk = [self.word(&k[..4]), self.word(&k[4..8])];
                let (mut va, mut vb, mut vc, mut vd) = (b[3], b[2], a[3], a[2]);
                let (mut ve, mut vf, mut vg, mut vh) = (b[1], b[0], a[1], a[0]);
                for word in wk {
                    let ch = self.choose(ve, vf, vg);
                    let sigma1 = {
                        let (x, y, z) = (
                            self.rotate_right(ve, 6),
                            self.rotate_right(ve, 11),
                            self.rotate_right(ve, 25),
                        );
                        self.xor3(x, y, z)
                    };
                    let maj = self.majority(va, vb, vc);
                    let sigma0 = {
                        let (x, y, z) = (
                            self.rotate_right(va, 2),
                            self.rotate_right(va, 13),
                            self.rotate_right(va, 22),
                        );
                        self.xor3(x, y, z)
                    };
                    let new_a = self.sum(&[ch, sigma1, word, vh, maj, sigma0]);
                    let new_e = self.sum(&[ch, sigma1, word, vh, vd]);
                    (vh, vg, vf, ve) = (vg, vf, ve, new_e);
                    (vd, vc, vb, va) = (vc, vb, va, new_a);
                }
                [vf, ve, vb, va]
            }
            Mnemonic::Sha256msg1 => {
                let sigma0 = |cpu: &mut Self, x: Term| {
                    let (p, q, r) = (
                        cpu.rotate_right(x, 7),
                        cpu.rotate_right(x, 18),
                        cpu.shift_right(x, 3),
                    );
                    cpu.xor3(p, q, r)
                };
                let (w0, w1, w2, w3, w4) = (a[0], a[1], a[2], a[3], b[0]);
                let s4 = sigma0(self, w4);
                let s3 = sigma0(self, w3);
                let s2 = sigma0(self, w2);
                let s1 = sigma0(self, w1);
                [
                    self.pool.add(w0, s1),
                    self.pool.add(w1, s2),
                    self.pool.add(w2, s3),
                    self.pool.add(w3, s4),
                ]
            }
            _ => {
                // sha256msg2
                let sigma1 = |cpu: &mut Self, x: Term| {
                    let (p, q, r) = (
                        cpu.rotate_right(x, 17),
                        cpu.rotate_right(x, 19),
                        cpu.shift_right(x, 10),
                    );
                    cpu.xor3(p, q, r)
                };
                let (w14, w15) = (b[2], b[3]);
                let s14 = sigma1(self, w14);
                let w16 = self.pool.add(a[0], s14);
                let s15 = sigma1(self, w15);
                let w17 = self.pool.add(a[1], s15);
                let s16 = sigma1(self, w16);
                let w18 = self.pool.add(a[2], s16);
                let s17 = sigma1(self, w17);
                let w19 = self.pool.add(a[3], s17);
                [w16, w17, w18, w19]
            }
        };
        let bytes = self.unwords(out);
        self.bytes_out(0, &bytes)
    }

    /// `pclmulqdq`: the carry-less product of a quadword from each source, picked by the
    /// immediate's bits 0 and 4, in each 16-byte lane.
    fn pclmul(&mut self) -> Result<(), Stop> {
        let bytes = self.operand_bytes(0).max(16);
        let (a, b) = self.crypto_sources(bytes)?;
        let immediate = self.instruction.immediate8();
        let (from_a, from_b) = (usize::from(immediate & 1), usize::from(immediate >> 4 & 1));
        let mut out = Vec::with_capacity(bytes);
        for lane in 0..bytes / 16 {
            let x = self.join(&a[16 * lane + 8 * from_a..16 * lane + 8 * from_a + 8]);
            let y = self.join(&b[16 * lane + 8 * from_b..16 * lane + 8 * from_b + 8]);
            let product = self.carry_less(x, y);
            out.extend(self.split(product, 16));
        }
        self.bytes_out(0, &out)
    }
}
