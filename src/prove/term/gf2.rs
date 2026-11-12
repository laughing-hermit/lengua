//! A CRC step read out of a program's own operations: a program that computes a reflected
//! CRC-32 a bit at a time, with shifts, a mask and exclusive ors, builds for each byte a term
//! that is the same function of the state and the byte as [`Node::Crc`], the term a formula's
//! `crc32` builds. [`Pool::as_crc`] finds that out and gives the one term for both, so a
//! program's checksum and a formula's of the same bytes meet as terms, with no question for a
//! solver about two long chains of exclusive ors, which no solver settles past a few bytes.
//!
//! How it is sound: each bit of a term is written as an exclusive or of bits of *atoms* and
//! maybe of 1 ([`Bit`]). An atom is any term this module doesn't read through: a variable, an
//! earlier CRC step, a sum, anything. Constants, `xor`, `not`, `and` and `or` with a constant,
//! extraction, widening, concatenation and the negation of a 0 or 1 are exact in that form: each
//! is the operator's own definition, bit by bit. Reading an atom as free bits can only lose
//! facts, never add one. Two terms with the same form are therefore equal for every value of the
//! atoms, so for every input. A CRC step is linear over those bits (`crc_byte(p, s, b)` is
//! `crc_byte(p, s ^ b, 0)`, and that is an exclusive or of the columns of one 32 by 32 matrix),
//! so its form is computed from the same definition the solvers are given, and the term is
//! replaced only when every one of its 32 bits has exactly the step's form. A test asks both
//! solvers the same of the terms as a program builds them, for every state and byte.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

use super::{CASTAGNOLI, Node, Pool, Term, ZLIB, crc_byte};

/// The most atoms' bits one bit may be an exclusive or of: past that the term is an atom itself,
/// so a long chain of exclusive ors that is no CRC costs each step a bounded time.
const MAX_COLUMNS: usize = 48;

/// The widest term read through: wider ones are atoms.
const MAX_WIDTH: u32 = 64;

/// How deep one reading goes into terms not read before: deeper ones are atoms.
const MAX_DEPTH: u32 = 48;

/// The columns one atom takes: one for each bit the widest term has.
const ATOM_BITS: u32 = 128;

/// One bit of a term: the exclusive or of the atoms' bits `columns` names, sorted, and of 1
/// where `one`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Bit {
    /// Each atom's bit: the atom's number times [`ATOM_BITS`], plus the bit.
    columns: Vec<u32>,
    /// Whether 1 is in the exclusive or.
    one: bool,
}

impl Bit {
    /// A bit that is always `one`.
    const fn constant(one: bool) -> Self {
        Self {
            columns: Vec::new(),
            one,
        }
    }

    /// The exclusive or of the two.
    fn xor(&self, other: &Self) -> Self {
        let mut columns = Vec::with_capacity(self.columns.len() + other.columns.len());
        let (mut mine, mut theirs) = (0, 0);
        loop {
            match (self.columns.get(mine), other.columns.get(theirs)) {
                (Some(a), Some(b)) => match a.cmp(b) {
                    Ordering::Less => {
                        columns.push(*a);
                        mine += 1;
                    }
                    Ordering::Greater => {
                        columns.push(*b);
                        theirs += 1;
                    }
                    Ordering::Equal => {
                        mine += 1;
                        theirs += 1;
                    }
                },
                (Some(a), None) => {
                    columns.push(*a);
                    mine += 1;
                }
                (None, Some(b)) => {
                    columns.push(*b);
                    theirs += 1;
                }
                (None, None) => break,
            }
        }
        Self {
            columns,
            one: self.one != other.one,
        }
    }

    /// Whether it is always 0.
    fn is_zero(&self) -> bool {
        self.columns.is_empty() && !self.one
    }
}

/// The forms read so far, and the atoms they are over.
#[derive(Debug, Default)]
pub(super) struct Forms {
    /// Each term read, with its bits, the lowest first.
    known: HashMap<Term, Rc<Vec<Bit>>>,
    /// The atoms, by number.
    atoms: Vec<Term>,
}

/// A reflected CRC's step by `poly` as a matrix: for each bit of the state after a byte, the
/// bits of the state before it, the byte already in its low 8 bits, that it is the exclusive or
/// of.
fn step_rows(poly: u32) -> [u32; 32] {
    let mut rows = [0_u32; 32];
    for column in 0..32_u32 {
        let after = crc_byte(poly, 1 << column, 0);
        for (bit, row) in (0..32_u32).zip(rows.iter_mut()) {
            if (after >> bit) & 1 == 1 {
                *row |= 1 << column;
            }
        }
    }
    rows
}

impl Pool {
    /// `term` as an atom: each of its `width` bits a column of its own.
    fn atom_bits(&mut self, term: Term, width: u32) -> Vec<Bit> {
        let number = u32::try_from(self.forms.atoms.len()).unwrap_or(u32::MAX / ATOM_BITS - 1);
        self.forms.atoms.push(term);
        (0..width)
            .map(|bit| Bit {
                columns: vec![number.saturating_mul(ATOM_BITS).saturating_add(bit)],
                one: false,
            })
            .collect()
    }

    /// The bits of `term` where this module reads its operator through, the lowest first.
    fn read_bits(&mut self, term: Term, width: u32, depth: u32) -> Option<Vec<Bit>> {
        let mask_of = |pool: &Self, a: Term, b: Term| match (pool.constant(a), pool.constant(b)) {
            (None, Some(mask)) => Some((a, mask)),
            (Some(mask), None) => Some((b, mask)),
            _ => None,
        };
        match *self.node(term) {
            Node::Const { value, .. } => Some(
                (0..width)
                    .map(|bit| Bit::constant((value >> bit) & 1 == 1))
                    .collect(),
            ),
            Node::Xor(a, b) => {
                let (a, b) = (self.bits_of(a, depth + 1), self.bits_of(b, depth + 1));
                Some(a.iter().zip(b.iter()).map(|(x, y)| x.xor(y)).collect())
            }
            Node::Not(a) => {
                let a = self.bits_of(a, depth + 1);
                Some(
                    a.iter()
                        .map(|bit| Bit {
                            columns: bit.columns.clone(),
                            one: !bit.one,
                        })
                        .collect(),
                )
            }
            Node::And(a, b) => {
                let (value, mask) = mask_of(self, a, b)?;
                let value = self.bits_of(value, depth + 1);
                Some(
                    (0..width)
                        .zip(value.iter())
                        .map(|(bit, kept)| {
                            if (mask >> bit) & 1 == 1 {
                                kept.clone()
                            } else {
                                Bit::constant(false)
                            }
                        })
                        .collect(),
                )
            }
            Node::Or(a, b) => {
                let (value, mask) = mask_of(self, a, b)?;
                let value = self.bits_of(value, depth + 1);
                Some(
                    (0..width)
                        .zip(value.iter())
                        .map(|(bit, kept)| {
                            if (mask >> bit) & 1 == 1 {
                                Bit::constant(true)
                            } else {
                                kept.clone()
                            }
                        })
                        .collect(),
                )
            }
            Node::Extract(a, high, low) => {
                let a = self.bits_of(a, depth + 1);
                let (low, high) = (usize::try_from(low).ok()?, usize::try_from(high).ok()?);
                a.get(low..=high).map(<[Bit]>::to_vec)
            }
            Node::ZeroExtend(a, by) => {
                let mut bits = self.bits_of(a, depth + 1).to_vec();
                bits.extend((0..by).map(|_| Bit::constant(false)));
                Some(bits)
            }
            Node::SignExtend(a, by) => {
                let mut bits = self.bits_of(a, depth + 1).to_vec();
                let top = bits.last()?.clone();
                bits.extend((0..by).map(|_| top.clone()));
                Some(bits)
            }
            Node::Concat(high, low) => {
                let mut bits = self.bits_of(low, depth + 1).to_vec();
                bits.extend(self.bits_of(high, depth + 1).iter().cloned());
                Some(bits)
            }
            // The negation of a 0 or 1 is all zeros or all ones: its lowest bit in every place.
            Node::Neg(a) => {
                let a = self.bits_of(a, depth + 1);
                let (lowest, rest) = a.split_first()?;
                rest.iter()
                    .all(Bit::is_zero)
                    .then(|| (0..width).map(|_| lowest.clone()).collect())
            }
            _ => None,
        }
    }

    /// The bits of `term`, the lowest first: read through where this module reads its operator,
    /// and an atom's own otherwise.
    fn bits_of(&mut self, term: Term, depth: u32) -> Rc<Vec<Bit>> {
        if let Some(known) = self.forms.known.get(&term) {
            return Rc::clone(known);
        }
        let width = self.width(term);
        let read = if depth > MAX_DEPTH || width == 0 || width > MAX_WIDTH {
            None
        } else {
            self.read_bits(term, width, depth)
        };
        let fits = |bits: &Vec<Bit>| {
            bits.len() == width as usize && bits.iter().all(|bit| bit.columns.len() <= MAX_COLUMNS)
        };
        let bits = match read.filter(fits) {
            Some(bits) => bits,
            None => self.atom_bits(term, width),
        };
        let bits = Rc::new(bits);
        self.forms.known.insert(term, Rc::clone(&bits));
        bits
    }

    /// `term` as one step of a reflected CRC-32, [`Node::Crc`], when it is one: a 32-bit term
    /// whose every bit is, for every value of what it is computed from, the bit a CRC step by
    /// the zlib or the Castagnoli polynomial gives, from a state that is another term, its
    /// complement, all ones or zero, and a byte that is an 8-bit term. `None` for any other
    /// term, and for a constant.
    pub fn as_crc(&mut self, term: Term) -> Option<Term> {
        if self.width(term) != 32
            || self.constant(term).is_some()
            || matches!(self.node(term), Node::Crc(..))
        {
            return None;
        }
        let bits = self.bits_of(term, 0);
        // The atoms it is over: one byte, and at most one state.
        let mut byte: Option<u32> = None;
        let mut state: Option<u32> = None;
        for column in bits.iter().flat_map(|bit| bit.columns.iter()) {
            let atom = column / ATOM_BITS;
            let found = *self.forms.atoms.get(atom as usize)?;
            let slot = match self.width(found) {
                8 => &mut byte,
                32 => &mut state,
                _ => return None,
            };
            if slot.is_some_and(|known| known != atom) {
                return None;
            }
            *slot = Some(atom);
        }
        let byte = byte?;
        let byte_term = *self.forms.atoms.get(byte as usize)?;
        let state_term = match state {
            Some(atom) => Some(*self.forms.atoms.get(atom as usize)?),
            None => None,
        };
        // Each bit as three masks: the byte's bits, the state's bits, and the constant.
        let mut byte_masks = [0_u32; 32];
        let mut state_masks = [0_u32; 32];
        let mut ones = 0_u32;
        for (place, bit) in (0..32_u32).zip(bits.iter()) {
            for column in &bit.columns {
                let (atom, at) = (column / ATOM_BITS, column % ATOM_BITS);
                let masks = if atom == byte {
                    &mut byte_masks
                } else {
                    &mut state_masks
                };
                *masks.get_mut(place as usize)? |= 1_u32.checked_shl(at)?;
            }
            if bit.one {
                ones |= 1 << place;
            }
        }
        for poly in [ZLIB, CASTAGNOLI] {
            let rows = step_rows(poly);
            // The byte comes in through the state's low 8 bits.
            if rows
                .iter()
                .zip(&byte_masks)
                .any(|(row, mask)| row & 0xff != *mask)
            {
                continue;
            }
            let from = match state_term {
                Some(before) => {
                    if rows.iter().zip(&state_masks).any(|(row, mask)| row != mask) {
                        continue;
                    }
                    // What the step makes of a state of all ones and a zero byte: the constant
                    // a complemented state leaves in each bit.
                    let complemented = crc_byte(poly, u32::MAX, 0);
                    if ones == 0 {
                        before
                    } else if ones == complemented {
                        self.not(before)
                    } else {
                        continue;
                    }
                }
                None => {
                    let Some(start) = [u32::MAX, 0]
                        .into_iter()
                        .find(|start| crc_byte(poly, *start, 0) == ones)
                    else {
                        continue;
                    };
                    self.bv(32, u128::from(start))
                }
            };
            return Some(self.crc(from, byte_term, poly));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One byte through a reflected CRC as a program with no table computes it: the byte
    /// xored into the state, then eight times one bit down, the polynomial coming in through a
    /// mask where the bit that left was set.
    fn by_shifts(pool: &mut Pool, state: Term, byte: Term, poly: u32, steps: u32) -> Term {
        let wide = pool.zero_extend(byte, 24);
        let mut c = pool.xor(state, wide);
        let one = pool.bv(32, 1);
        let zero = pool.bv(32, 0);
        let poly = pool.bv(32, u128::from(poly));
        for _ in 0..steps {
            let down = pool.lshr(c, one);
            let low = pool.and(c, one);
            let mask = pool.sub(zero, low);
            let comes_in = pool.and(poly, mask);
            c = pool.xor(down, comes_in);
        }
        c
    }

    #[test]
    fn a_crc_by_shifts_is_the_crc_s_own_term() {
        for poly in [ZLIB, CASTAGNOLI] {
            let mut pool = Pool::default();
            let state = pool.var(32, "state");
            let byte = pool.var(8, "byte");
            // From a state that is a term, and from its complement, as a CRC starts.
            let shifted = by_shifts(&mut pool, state, byte, poly, 8);
            let expected = pool.crc(state, byte, poly);
            assert_eq!(
                pool.as_crc(shifted),
                Some(expected),
                "{}",
                pool.sketch(shifted, 12)
            );
            let inverted = pool.not(state);
            let shifted = by_shifts(&mut pool, inverted, byte, poly, 8);
            let expected = pool.crc(inverted, byte, poly);
            assert_eq!(pool.as_crc(shifted), Some(expected));
            // From all ones, where a CRC of a text starts, and then byte after byte.
            let ones = pool.bv(32, u128::from(u32::MAX));
            let mut program = ones;
            let mut formula = ones;
            for index in 0..6 {
                let next = pool.var(8, &format!("text.{index}"));
                let shifted = by_shifts(&mut pool, program, next, poly, 8);
                program = pool.as_crc(shifted).expect("a CRC step");
                formula = pool.crc(formula, next, poly);
                assert_eq!(program, formula, "byte {index}");
            }
        }
    }

    #[test]
    fn what_is_no_crc_step_stays_as_it_is() {
        let mut pool = Pool::default();
        let state = pool.var(32, "state");
        let byte = pool.var(8, "byte");
        // Seven steps, nine steps, and another polynomial.
        for steps in [1, 7, 9] {
            let shifted = by_shifts(&mut pool, state, byte, ZLIB, steps);
            assert_eq!(pool.as_crc(shifted), None, "{steps} steps");
        }
        let other = by_shifts(&mut pool, state, byte, 0x8408_0000, 8);
        assert_eq!(pool.as_crc(other), None);
        // A sum is an atom: no CRC, and no reading through it.
        let wide = pool.zero_extend(byte, 24);
        let sum = pool.add(state, wide);
        assert_eq!(pool.as_crc(sum), None);
        let one = pool.bv(32, 1);
        let shifted = pool.lshr(sum, one);
        let mixed = pool.xor(shifted, state);
        assert_eq!(pool.as_crc(mixed), None);
        // A constant, a term that is a CRC step already, and a term of another width.
        let constant = pool.bv(32, 7);
        assert_eq!(pool.as_crc(constant), None);
        let step = pool.crc(state, byte, ZLIB);
        assert_eq!(pool.as_crc(step), None);
        assert_eq!(pool.as_crc(byte), None);
    }

    /// Both solvers hold the reading for every state and byte: the term as a program's shifts
    /// build it never differs from the CRC step it is read as.
    #[test]
    fn both_solvers_confirm_the_reading() {
        use crate::prove::smt::{Answer, Solver};
        for poly in [ZLIB, CASTAGNOLI] {
            let mut pool = Pool::default();
            let Ok(mut solver) = Solver::start(20_000) else {
                eprintln!("no solver is installed");
                return;
            };
            let state = pool.var(32, "state");
            let byte = pool.var(8, "byte");
            let shifted = by_shifts(&mut pool, state, byte, poly, 8);
            let read = pool.as_crc(shifted).expect("a CRC step");
            assert_ne!(read, shifted, "the two are one term before the reading");
            let same = pool.eq(shifted, read);
            let differs = pool.bool_not(same);
            let (answer, _) = solver.check(&pool, &[differs], &[]).expect("an answer");
            assert_eq!(answer, Answer::Unsat);
            assert_eq!(solver.alone, None, "both solvers answered");
        }
    }
}
