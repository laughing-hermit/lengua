//! Random-value tests of the term pool: random expressions are built through the pool, whose
//! builders simplify as they go, and evaluated at random inputs. Each must equal the plain
//! SMT-LIB arithmetic of the expression as written, every term's range must hold its value, and
//! the solvers must read each term's SMT-LIB text as the pool evaluates it.

use std::collections::HashMap;

use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

use super::{Pool, Term};
use crate::prove::smt::{Answer, Solver};

/// An expression as written, before the pool simplifies it.
#[derive(Debug, Clone)]
enum Expr {
    Var(usize),
    Const(u32, u128),
    Bool(bool),
    Binary(Op, Box<Self>, Box<Self>),
    Not(Box<Self>),
    Neg(Box<Self>),
    Extract(Box<Self>, u32, u32),
    ZeroExtend(Box<Self>, u32),
    SignExtend(Box<Self>, u32),
    Concat(Box<Self>, Box<Self>),
    Ite(Box<Self>, Box<Self>, Box<Self>),
    Compare(Comparison, Box<Self>, Box<Self>),
    Logic(bool, Box<Self>, Box<Self>),
    BoolNot(Box<Self>),
    /// A 32-bit state, a byte through a CRC by the polynomial.
    Crc(Box<Self>, Box<Self>, u32),
}

#[derive(Debug, Clone, Copy)]
enum Op {
    Add,
    Sub,
    Mul,
    Udiv,
    Urem,
    Sdiv,
    Srem,
    And,
    Or,
    Xor,
    Shl,
    Lshr,
    Ashr,
}

#[derive(Debug, Clone, Copy)]
enum Comparison {
    Eq,
    Ult,
    Ule,
    Slt,
    Sle,
}

/// A variable: its width and the range its values are drawn from.
#[derive(Debug, Clone)]
struct Variable {
    width: u32,
    low: u128,
    high: u128,
}

const fn mask(width: u32) -> u128 {
    if width >= 128 {
        u128::MAX
    } else {
        (1 << width) - 1
    }
}

const fn top_bit(value: u128, width: u32) -> bool {
    value >> (width - 1) & 1 == 1
}

const fn negate(value: u128, width: u32) -> u128 {
    value.wrapping_neg() & mask(width)
}

/// The value as a signed number of `width` bits.
const fn signed(value: u128, width: u32) -> i128 {
    if width < 128 && top_bit(value, width) {
        value as i128 - (1 << width)
    } else {
        value as i128
    }
}

/// SMT-LIB's bit-vector operations, written from the standard's definitions.
fn apply(op: Op, a: u128, b: u128, width: u32) -> u128 {
    let udiv = |a: u128, b: u128| a.checked_div(b).unwrap_or(mask(width));
    let urem = |a: u128, b: u128| if b == 0 { a } else { a % b };
    let (negative_a, negative_b) = (top_bit(a, width), top_bit(b, width));
    match op {
        Op::Add => a.wrapping_add(b) & mask(width),
        Op::Sub => a.wrapping_sub(b) & mask(width),
        Op::Mul => a.wrapping_mul(b) & mask(width),
        Op::Udiv => udiv(a, b),
        Op::Urem => urem(a, b),
        Op::Sdiv => match (negative_a, negative_b) {
            (false, false) => udiv(a, b),
            (true, false) => negate(udiv(negate(a, width), b), width),
            (false, true) => negate(udiv(a, negate(b, width)), width),
            (true, true) => udiv(negate(a, width), negate(b, width)),
        },
        Op::Srem => match (negative_a, negative_b) {
            (false, false) => urem(a, b),
            (true, false) => negate(urem(negate(a, width), b), width),
            (false, true) => urem(a, negate(b, width)),
            (true, true) => negate(urem(negate(a, width), negate(b, width)), width),
        },
        Op::And => a & b,
        Op::Or => a | b,
        Op::Xor => a ^ b,
        Op::Shl => {
            if b >= u128::from(width) {
                0
            } else {
                (a << b) & mask(width)
            }
        }
        Op::Lshr => {
            if b >= u128::from(width) {
                0
            } else {
                a >> b
            }
        }
        Op::Ashr => {
            let fill = if negative_a { mask(width) } else { 0 };
            if b >= u128::from(width) {
                fill
            } else if b == 0 {
                a
            } else {
                let shift = u32::try_from(b).unwrap_or(0);
                (a >> shift) | (fill << (width - shift) & mask(width))
            }
        }
    }
}

/// The expression's value and width (0 for true and false).
fn reference(expr: &Expr, variables: &[Variable], values: &[u128]) -> (u128, u32) {
    match expr {
        Expr::Var(index) => (values[*index], variables[*index].width),
        Expr::Const(width, value) => (*value, *width),
        Expr::Bool(value) => (u128::from(*value), 0),
        Expr::Binary(op, a, b) => {
            let ((a, width), (b, _)) = (
                reference(a, variables, values),
                reference(b, variables, values),
            );
            (apply(*op, a, b, width), width)
        }
        Expr::Not(a) => {
            let (a, width) = reference(a, variables, values);
            (!a & mask(width), width)
        }
        Expr::Neg(a) => {
            let (a, width) = reference(a, variables, values);
            (negate(a, width), width)
        }
        Expr::Extract(a, high, low) => {
            let (a, _) = reference(a, variables, values);
            ((a >> low) & mask(high - low + 1), high - low + 1)
        }
        Expr::ZeroExtend(a, by) => {
            let (a, width) = reference(a, variables, values);
            (a, width + by)
        }
        Expr::SignExtend(a, by) => {
            let (a, width) = reference(a, variables, values);
            let fill = if top_bit(a, width) {
                mask(*by) << width
            } else {
                0
            };
            (a | fill, width + by)
        }
        Expr::Concat(high, low) => {
            let ((high, high_width), (low, low_width)) = (
                reference(high, variables, values),
                reference(low, variables, values),
            );
            ((high << low_width) | low, high_width + low_width)
        }
        // Bit by bit, from the lowest, as the CRC is defined.
        Expr::Crc(state, byte, poly) => {
            let ((state, _), (byte, _)) = (
                reference(state, variables, values),
                reference(byte, variables, values),
            );
            let mut crc = u32::try_from(state ^ byte).unwrap_or(0);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ poly
                } else {
                    crc >> 1
                };
            }
            (u128::from(crc), 32)
        }
        Expr::Ite(condition, then, otherwise) => {
            if reference(condition, variables, values).0 == 1 {
                reference(then, variables, values)
            } else {
                reference(otherwise, variables, values)
            }
        }
        Expr::Compare(comparison, a, b) => {
            let ((a, width), (b, _)) = (
                reference(a, variables, values),
                reference(b, variables, values),
            );
            let (sa, sb) = (signed(a, width), signed(b, width));
            let truth = match comparison {
                Comparison::Eq => a == b,
                Comparison::Ult => a < b,
                Comparison::Ule => a <= b,
                Comparison::Slt => sa < sb,
                Comparison::Sle => sa <= sb,
            };
            (u128::from(truth), 0)
        }
        Expr::Logic(and, a, b) => {
            let (a, b) = (
                reference(a, variables, values).0 == 1,
                reference(b, variables, values).0 == 1,
            );
            (u128::from(if *and { a && b } else { a || b }), 0)
        }
        Expr::BoolNot(a) => (u128::from(reference(a, variables, values).0 == 0), 0),
    }
}

/// The expression built through the pool's builders, which simplify it.
fn build(pool: &mut Pool, expr: &Expr, variables: &[Term]) -> Term {
    match expr {
        Expr::Var(index) => variables[*index],
        Expr::Const(width, value) => pool.bv(*width, *value),
        Expr::Bool(value) => pool.boolean(*value),
        Expr::Binary(op, a, b) => {
            let (a, b) = (build(pool, a, variables), build(pool, b, variables));
            match op {
                Op::Add => pool.add(a, b),
                Op::Sub => pool.sub(a, b),
                Op::Mul => pool.mul(a, b),
                Op::Udiv => pool.udiv(a, b),
                Op::Urem => pool.urem(a, b),
                Op::Sdiv => pool.sdiv(a, b),
                Op::Srem => pool.srem(a, b),
                Op::And => pool.and(a, b),
                Op::Or => pool.or(a, b),
                Op::Xor => pool.xor(a, b),
                Op::Shl => pool.shl(a, b),
                Op::Lshr => pool.lshr(a, b),
                Op::Ashr => pool.ashr(a, b),
            }
        }
        Expr::Not(a) => {
            let a = build(pool, a, variables);
            pool.not(a)
        }
        Expr::Neg(a) => {
            let a = build(pool, a, variables);
            pool.neg(a)
        }
        Expr::Extract(a, high, low) => {
            let a = build(pool, a, variables);
            pool.extract(a, *high, *low)
        }
        Expr::ZeroExtend(a, by) => {
            let a = build(pool, a, variables);
            pool.zero_extend(a, *by)
        }
        Expr::SignExtend(a, by) => {
            let a = build(pool, a, variables);
            pool.sign_extend(a, *by)
        }
        Expr::Concat(high, low) => {
            let (high, low) = (build(pool, high, variables), build(pool, low, variables));
            pool.concat(high, low)
        }
        Expr::Crc(state, byte, poly) => {
            let (state, byte) = (build(pool, state, variables), build(pool, byte, variables));
            pool.crc(state, byte, *poly)
        }
        Expr::Ite(condition, then, otherwise) => {
            let condition = build(pool, condition, variables);
            let (then, otherwise) = (
                build(pool, then, variables),
                build(pool, otherwise, variables),
            );
            pool.ite(condition, then, otherwise)
        }
        Expr::Compare(comparison, a, b) => {
            let (a, b) = (build(pool, a, variables), build(pool, b, variables));
            match comparison {
                Comparison::Eq => pool.eq(a, b),
                Comparison::Ult => pool.ult(a, b),
                Comparison::Ule => pool.ule(a, b),
                Comparison::Slt => pool.slt(a, b),
                Comparison::Sle => pool.sle(a, b),
            }
        }
        Expr::Logic(and, a, b) => {
            let (a, b) = (build(pool, a, variables), build(pool, b, variables));
            if *and {
                pool.bool_and(a, b)
            } else {
                pool.bool_or(a, b)
            }
        }
        Expr::BoolNot(a) => {
            let a = build(pool, a, variables);
            pool.bool_not(a)
        }
    }
}

/// Writes expressions like the ones the prover builds: digit chains, widenings of sums, divisions
/// by constants, comparisons with constants, all over variables with small ranges.
struct Writer<'a> {
    rng: &'a mut ChaCha8Rng,
    variables: Vec<Variable>,
    /// The widths to write in, narrowest first.
    widths: &'static [u32],
}

const WIDTHS: [u32; 8] = [1, 8, 16, 32, 63, 64, 65, 128];

/// Widths up to 65 bits: a solver multiplies twice the width to check a division, and 256-bit
/// multipliers take seconds even with every input known.
const NARROW: [u32; 7] = [1, 8, 16, 32, 63, 64, 65];

impl Writer<'_> {
    fn constant(&mut self, width: u32) -> u128 {
        let interesting = [
            0,
            1,
            2,
            9,
            10,
            48,
            57,
            100,
            1000,
            mask(width),
            mask(width) - 1,
            mask(width) >> 1,
            (mask(width) >> 1) + 1,
            10_u128.pow(9),
            10_u128.pow(18),
        ];
        match self.rng.random_range(0..3) {
            0 => self.rng.random::<u128>() & mask(width),
            _ => interesting[self.rng.random_range(0..interesting.len())] & mask(width),
        }
    }

    fn variable(&mut self, width: u32) -> Expr {
        let existing: Vec<usize> = (0..self.variables.len())
            .filter(|index| self.variables[*index].width == width)
            .collect();
        if !existing.is_empty() && self.rng.random_bool(0.6) {
            return Expr::Var(existing[self.rng.random_range(0..existing.len())]);
        }
        let (low, high) = match self.rng.random_range(0..4) {
            0 => (0, mask(width)),
            // A digit's character.
            1 if width >= 8 => (48, 57),
            // A small range, like a value with a few digits.
            2 => {
                let low = self.rng.random_range(0..1000) & mask(width);
                let span = [9, 99, 999_999, 10_u128.pow(12)][self.rng.random_range(0..4)];
                (low, low.saturating_add(span).min(mask(width)))
            }
            _ => {
                let (a, b) = (self.constant(width), self.constant(width));
                (a.min(b), a.max(b))
            }
        };
        self.variables.push(Variable { width, low, high });
        Expr::Var(self.variables.len() - 1)
    }

    fn number(&mut self, width: u32, depth: u32) -> Expr {
        if depth == 0 || self.rng.random_range(0..10) < 2 {
            return if self.rng.random_bool(0.3) {
                Expr::Const(width, self.constant(width))
            } else {
                self.variable(width)
            };
        }
        let deeper = depth - 1;
        match self.rng.random_range(0..27) {
            // A quotient less c times its own quotient by c, as a print loop takes the next
            // digit off with udiv and msub: (p / c1) % c, the second division written
            // p / (c1 c).
            26 => {
                let value = self.number(width, deeper);
                let first = Expr::Const(width, self.rng.random_range(1..=16_u128) & mask(width));
                let by = Expr::Const(width, self.rng.random_range(2..=16_u128) & mask(width));
                let quotient = Expr::Binary(Op::Udiv, Box::new(value), Box::new(first));
                let next = Expr::Binary(Op::Udiv, Box::new(quotient.clone()), Box::new(by.clone()));
                let product = Expr::Binary(Op::Mul, Box::new(next), Box::new(by));
                Expr::Binary(Op::Sub, Box::new(quotient), Box::new(product))
            }
            // A sum less c times its own quotient by c, as AArch64's udiv and msub leave a
            // number's last digit: its remainder.
            25 => {
                let left = self.number(width, deeper);
                let right = self.number(width, deeper);
                let sum = Expr::Binary(Op::Add, Box::new(left), Box::new(right));
                let divisor =
                    Expr::Const(width, (self.rng.random_range(1..=255_u128)) & mask(width));
                let quotient =
                    Expr::Binary(Op::Udiv, Box::new(sum.clone()), Box::new(divisor.clone()));
                let product = Expr::Binary(Op::Mul, Box::new(quotient), Box::new(divisor));
                Expr::Binary(Op::Sub, Box::new(sum), Box::new(product))
            }
            // A bit of the low mask of n bits, (1 << n) - 1, as bzhi builds one with an index
            // the input chooses: sometimes past the width, where every bit is set.
            24 if width >= 8 => {
                let count = if self.rng.random_bool(0.5) {
                    self.variable(width)
                } else {
                    self.variables.push(Variable {
                        width,
                        low: 0,
                        high: u128::from(width) + 3,
                    });
                    Expr::Var(self.variables.len() - 1)
                };
                let shifted =
                    Expr::Binary(Op::Shl, Box::new(Expr::Const(width, 1)), Box::new(count));
                let low_mask =
                    Expr::Binary(Op::Sub, Box::new(shifted), Box::new(Expr::Const(width, 1)));
                let bit = self.rng.random_range(0..width);
                // Sometimes the mask's low bits first, as a byte of a mask register is read,
                // then one of theirs.
                let read = if self.rng.random_bool(0.5) {
                    let top = self.rng.random_range(bit..width);
                    Expr::Extract(Box::new(low_mask), top, 0)
                } else {
                    low_mask
                };
                Expr::ZeroExtend(Box::new(Expr::Extract(Box::new(read), bit, bit)), width - 1)
            }
            // Two values that are each 0 or 1, as setcc leaves them, combined bitwise as a
            // program keeps a flag: pending = is_a and not (is_a and pending).
            23 if width >= 8 => {
                let op = [Op::And, Op::Or, Op::Xor][self.rng.random_range(0..3)];
                let a = self.bit(width, deeper);
                let b = if self.rng.random_bool(0.3) {
                    Expr::Const(width, 1)
                } else {
                    self.bit(width, deeper)
                };
                Expr::Binary(op, Box::new(a), Box::new(b))
            }
            0..=3 => {
                let ops = [
                    Op::Add,
                    Op::Sub,
                    Op::Mul,
                    Op::Udiv,
                    Op::Urem,
                    Op::Sdiv,
                    Op::Srem,
                    Op::And,
                    Op::Or,
                    Op::Xor,
                    Op::Shl,
                    Op::Lshr,
                    Op::Ashr,
                ];
                let op = ops[self.rng.random_range(0..ops.len())];
                let a = self.number(width, deeper);
                // Constants on the right, as the prover's divisions, shifts and products have.
                let b = if self.rng.random_bool(0.5) {
                    let value = match op {
                        Op::Shl | Op::Lshr | Op::Ashr => {
                            u128::from(self.rng.random_range(0..width + 2)) & mask(width)
                        }
                        _ => self.constant(width),
                    };
                    Expr::Const(width, value)
                } else {
                    self.number(width, deeper)
                };
                Expr::Binary(op, Box::new(a), Box::new(b))
            }
            4 => Expr::Not(Box::new(self.number(width, deeper))),
            5 => Expr::Neg(Box::new(self.number(width, deeper))),
            6 if width < self.widths[self.widths.len() - 1] => {
                let widest = self.widths[self.widths.len() - 1];
                let wider = self
                    .widths
                    .iter()
                    .copied()
                    .filter(|candidate| *candidate > width)
                    .nth(self.rng.random_range(0..2))
                    .unwrap_or(widest);
                let low = self.rng.random_range(0..=wider - width);
                Expr::Extract(Box::new(self.number(wider, deeper)), low + width - 1, low)
            }
            7 | 8 if width > 1 => {
                let narrower: Vec<u32> = self
                    .widths
                    .iter()
                    .copied()
                    .filter(|candidate| *candidate < width)
                    .collect();
                let from = narrower[self.rng.random_range(0..narrower.len())];
                let inner = Box::new(self.number(from, deeper));
                if self.rng.random_bool(0.5) {
                    Expr::ZeroExtend(inner, width - from)
                } else {
                    Expr::SignExtend(inner, width - from)
                }
            }
            // A value with copies of its sign above it, as cqo makes for idiv.
            9 if width > 1 && width.is_multiple_of(2) && self.rng.random_bool(0.3) => {
                let half = width / 2;
                let value = self.number(half, deeper);
                let sign = Expr::Binary(
                    Op::Ashr,
                    Box::new(value.clone()),
                    Box::new(Expr::Const(half, u128::from(half - 1))),
                );
                Expr::Concat(Box::new(sign), Box::new(value))
            }
            9 if width > 1 => {
                let split = self.rng.random_range(1..width);
                Expr::Concat(
                    Box::new(self.number(width - split, deeper)),
                    Box::new(self.number(split, deeper)),
                )
            }
            10 => Expr::Ite(
                Box::new(self.truth(deeper)),
                Box::new(self.number(width, deeper)),
                Box::new(self.number(width, deeper)),
            ),
            12 | 13 if width > 8 => {
                // An absolute value, widened with zeros, choosing by the sign of the widened
                // value: as a program prints what cmov picked.
                let narrower: Vec<u32> = self
                    .widths
                    .iter()
                    .copied()
                    .filter(|candidate| *candidate < width && *candidate >= 8)
                    .collect();
                let from = narrower[self.rng.random_range(0..narrower.len())];
                let x = self.number(from, deeper);
                let wide = Expr::SignExtend(Box::new(x.clone()), width - from);
                let zero = Expr::Const(width, 0);
                let comparison = [Comparison::Slt, Comparison::Sle][self.rng.random_range(0..2)];
                let condition = if self.rng.random_bool(0.5) {
                    Expr::Compare(comparison, Box::new(zero), Box::new(wide))
                } else {
                    Expr::Compare(comparison, Box::new(wide), Box::new(zero))
                };
                let negated = Expr::Neg(Box::new(x.clone()));
                let (then, otherwise) = if self.rng.random_bool(0.5) {
                    (x, negated)
                } else {
                    (negated, x)
                };
                Expr::ZeroExtend(
                    Box::new(Expr::Ite(
                        Box::new(condition),
                        Box::new(then),
                        Box::new(otherwise),
                    )),
                    width - from,
                )
            }
            11 if width >= 8 => {
                // A sum of a digit chain's kind: x * 10 + (zext(d) - 48), widened.
                let d = self.variable(8);
                let wide = if width > 8 {
                    Expr::ZeroExtend(Box::new(d), width - 8)
                } else {
                    d
                };
                let digit = Expr::Binary(Op::Sub, Box::new(wide), Box::new(Expr::Const(width, 48)));
                let times_ten = Expr::Binary(
                    Op::Mul,
                    Box::new(self.number(width, deeper)),
                    Box::new(Expr::Const(width, 10 & mask(width))),
                );
                Expr::Binary(Op::Add, Box::new(times_ten), Box::new(digit))
            }
            // A compiler's division by a constant: the top bits of x times a magic number,
            // as `mul` and `shr` leave them, or a near miss that isn't a division.
            14 | 15 if width >= 16 && width.is_multiple_of(2) => {
                let half = width / 2;
                let x = self.variable(half);
                let largest = 1_u128 << half.min(20);
                let divisor = self.rng.random_range(2..=largest);
                let shift = 128 - (divisor - 1).leading_zeros();
                let low = (half + shift).min(width - 1);
                let mut multiplier = (1_u128 << low).div_ceil(divisor);
                match self.rng.random_range(0..4) {
                    0 => multiplier += 1,
                    1 => multiplier -= 1,
                    _ => {}
                }
                let product = Expr::Binary(
                    Op::Mul,
                    Box::new(Expr::ZeroExtend(Box::new(x), half)),
                    Box::new(Expr::Const(width, multiplier & mask(width))),
                );
                // The top bits, widened back to the width asked for, as `shr` leaves them.
                Expr::ZeroExtend(
                    Box::new(Expr::Extract(Box::new(product), width - 1, low)),
                    low,
                )
            }
            // A remainder, quotient or comparison of a sum of widened narrow values by a
            // constant: a formula's 128-bit arithmetic on a program's 64-bit values.
            16 if width > 64 => {
                let narrow = [8, 32, 64][self.rng.random_range(0..3)];
                let wide = Expr::ZeroExtend(Box::new(self.variable(narrow)), width - narrow);
                let sum = Expr::Binary(
                    Op::Add,
                    Box::new(wide),
                    Box::new(Expr::Const(width, self.rng.random_range(0..10))),
                );
                let op = [Op::Udiv, Op::Urem, Op::Sdiv, Op::Srem][self.rng.random_range(0..4)];
                let divisor = [3, 7, 10, 60, 3600, 86400][self.rng.random_range(0..6)];
                Expr::Binary(op, Box::new(sum), Box::new(Expr::Const(width, divisor)))
            }
            // A count, as popcnt and count() make one: values that are each 0 or 1, added up,
            // and often divided by 10 as its digits are printed.
            17 if width >= 8 => {
                let mut count = Expr::Const(width, 0);
                for _ in 0..self.rng.random_range(2..7) {
                    let one = if self.rng.random_bool(0.5) {
                        Expr::Ite(
                            Box::new(self.truth(deeper.min(1))),
                            Box::new(Expr::Const(width, 1)),
                            Box::new(Expr::Const(width, 0)),
                        )
                    } else {
                        let bit = self.rng.random_range(0..width);
                        let value = self.number(width, deeper.min(1));
                        Expr::ZeroExtend(
                            Box::new(Expr::Extract(Box::new(value), bit, bit)),
                            width - 1,
                        )
                    };
                    count = Expr::Binary(Op::Add, Box::new(count), Box::new(one));
                }
                if self.rng.random_bool(0.5) {
                    let op = [Op::Udiv, Op::Urem][self.rng.random_range(0..2)];
                    let divisor = [3, 10][self.rng.random_range(0..2)];
                    Expr::Binary(op, Box::new(count), Box::new(Expr::Const(width, divisor)))
                } else {
                    count
                }
            }
            // A quotient of a sum each of whose parts its divisor divides, or of a choice
            // between two such sums, as a program halves a sum of cents.
            18 if width >= 8 => {
                let divisor: u128 = [2, 3, 10, 100][self.rng.random_range(0..4)];
                let dividend = if self.rng.random_bool(0.5) {
                    let (then, otherwise) = (
                        self.divisible_sum(width, divisor),
                        self.divisible_sum(width, divisor),
                    );
                    Expr::Ite(
                        Box::new(self.truth(deeper.min(1))),
                        Box::new(then),
                        Box::new(otherwise),
                    )
                } else {
                    self.divisible_sum(width, divisor)
                };
                let op = [Op::Udiv, Op::Sdiv][self.rng.random_range(0..2)];
                Expr::Binary(
                    op,
                    Box::new(dividend),
                    Box::new(Expr::Const(width, divisor)),
                )
            }
            // A value shifted right, then divided, as a program halves a number and takes its
            // digits.
            20 if width >= 8 => {
                let shift = self.rng.random_range(1..4).min(width - 1);
                let shifted = Expr::Binary(
                    Op::Lshr,
                    Box::new(self.number(width, deeper)),
                    Box::new(Expr::Const(width, u128::from(shift))),
                );
                let divisor = [3, 5, 10][self.rng.random_range(0..3)];
                Expr::Binary(
                    Op::Udiv,
                    Box::new(shifted),
                    Box::new(Expr::Const(width, divisor)),
                )
            }
            // A quotient of a sum sharing a factor with its divisor, as a formula takes one of
            // ten times what a program divides: the factor cancels.
            19 if width >= 16 => {
                let factor: u128 = [2, 5, 10][self.rng.random_range(0..3)];
                let divisor = factor * [3, 7, 10][self.rng.random_range(0..3)];
                let mut sum = Expr::Const(width, factor * self.rng.random_range(0..50));
                for _ in 0..self.rng.random_range(1..3) {
                    let times = factor * self.rng.random_range(1..9);
                    let part = Expr::Binary(
                        Op::Mul,
                        Box::new(self.variable(width)),
                        Box::new(Expr::Const(width, times)),
                    );
                    sum = Expr::Binary(Op::Add, Box::new(sum), Box::new(part));
                }
                let op = [Op::Udiv, Op::Sdiv][self.rng.random_range(0..2)];
                Expr::Binary(op, Box::new(sum), Box::new(Expr::Const(width, divisor)))
            }
            // A quotient's low bits widened past the quotient's own width, as a formula widens
            // to 128 bits a value a program narrowed: the quotient itself where it fits them.
            22 if width >= 16 => {
                let narrower: Vec<u32> = self
                    .widths
                    .iter()
                    .copied()
                    .filter(|candidate| *candidate < width && *candidate >= 8)
                    .collect();
                let from = narrower[self.rng.random_range(0..narrower.len())];
                let divisor = ([2, 10, 20, 1000][self.rng.random_range(0..4)] & mask(from)).max(2);
                let quotient = Expr::Binary(
                    Op::Udiv,
                    Box::new(self.number(from, deeper)),
                    Box::new(Expr::Const(from, divisor)),
                );
                let kept = self.rng.random_range(from.saturating_sub(12).max(1)..from);
                Expr::ZeroExtend(
                    Box::new(Expr::Extract(Box::new(quotient), kept - 1, 0)),
                    width - kept,
                )
            }
            // A checksum: a few bytes through a CRC, as the `crc32` instruction and the
            // formulas take them.
            21 if width == 32 => {
                let poly = [super::CASTAGNOLI, super::ZLIB][self.rng.random_range(0..2)];
                let mut state = self.number(32, deeper);
                for _ in 0..self.rng.random_range(1..4) {
                    let byte = self.number(8, deeper.min(1));
                    state = Expr::Crc(Box::new(state), Box::new(byte), poly);
                }
                state
            }
            _ => Expr::Binary(
                Op::Add,
                Box::new(self.number(width, deeper)),
                Box::new(self.number(width, deeper)),
            ),
        }
    }

    /// A value of `width` bits that's only ever 0 or 1: a choice of the two, perhaps widened
    /// from a byte as setcc writes one, a variable of those two values, or two of these
    /// combined bitwise.
    fn bit(&mut self, width: u32, depth: u32) -> Expr {
        let choice = |writer: &mut Self, width: u32| {
            let (one, zero) = (Expr::Const(width, 1), Expr::Const(width, 0));
            let (then, otherwise) = if writer.rng.random_bool(0.7) {
                (one, zero)
            } else {
                (zero, one)
            };
            Expr::Ite(
                Box::new(writer.truth(depth.min(2))),
                Box::new(then),
                Box::new(otherwise),
            )
        };
        match self.rng.random_range(0..5) {
            0 if width > 8 => Expr::ZeroExtend(Box::new(choice(self, 8)), width - 8),
            1 => {
                self.variables.push(Variable {
                    width,
                    low: 0,
                    high: 1,
                });
                Expr::Var(self.variables.len() - 1)
            }
            2 if depth > 0 => {
                let op = [Op::And, Op::Or, Op::Xor][self.rng.random_range(0..3)];
                let (a, b) = (self.bit(width, depth - 1), self.bit(width, depth - 1));
                Expr::Binary(op, Box::new(a), Box::new(b))
            }
            _ => choice(self, width),
        }
    }

    /// A sum of variables, each times a multiple of `divisor`, perhaps negated, plus a constant
    /// of less than `divisor` either way.
    fn divisible_sum(&mut self, width: u32, divisor: u128) -> Expr {
        let off = self.rng.random_range(0..2 * divisor - 1);
        let constant = (off as i128 - (divisor as i128 - 1)).cast_unsigned() & mask(width);
        let mut total = Expr::Const(width, constant);
        for _ in 0..self.rng.random_range(1..3) {
            let times = divisor * self.rng.random_range(1..4);
            let part = Expr::Binary(
                Op::Mul,
                Box::new(self.variable(width)),
                Box::new(Expr::Const(width, times & mask(width))),
            );
            let part = if self.rng.random_bool(0.3) {
                Expr::Neg(Box::new(part))
            } else {
                part
            };
            total = Expr::Binary(Op::Add, Box::new(total), Box::new(part));
        }
        total
    }

    fn truth(&mut self, depth: u32) -> Expr {
        if depth == 0 {
            return Expr::Bool(self.rng.random_bool(0.5));
        }
        let deeper = depth - 1;
        match self.rng.random_range(0..13) {
            // Two comparisons of one value with constants, both holding, as a loop's "still
            // running" conditions pile up.
            12 => {
                let width = [8, 16, 32, 64][self.rng.random_range(0..4)];
                let value = self.number(width, deeper);
                let comparison = |writer: &mut Self| {
                    let comparisons = [
                        Comparison::Ult,
                        Comparison::Ule,
                        Comparison::Slt,
                        Comparison::Sle,
                    ];
                    let comparison = comparisons[writer.rng.random_range(0..comparisons.len())];
                    let constant = Box::new(Expr::Const(width, writer.constant(width)));
                    if writer.rng.random_bool(0.5) {
                        Expr::Compare(comparison, Box::new(value.clone()), constant)
                    } else {
                        Expr::Compare(comparison, constant, Box::new(value.clone()))
                    }
                };
                let (first, second) = (comparison(self), comparison(self));
                Expr::Logic(true, Box::new(first), Box::new(second))
            }
            // a and not (a and c), either way round, as a matcher's state after a byte is
            // written: the byte matched and ended no match.
            11 => {
                let (a, c) = (self.truth(deeper), self.truth(deeper));
                let inner = if self.rng.random_bool(0.5) {
                    Expr::Logic(true, Box::new(a.clone()), Box::new(c))
                } else {
                    Expr::Logic(true, Box::new(c), Box::new(a.clone()))
                };
                let not_both = Expr::BoolNot(Box::new(inner));
                if self.rng.random_bool(0.5) {
                    Expr::Logic(true, Box::new(a), Box::new(not_both))
                } else {
                    Expr::Logic(true, Box::new(not_both), Box::new(a))
                }
            }
            // A value shifted right compared with a constant, as a formula bounds a halved
            // number's digits: sometimes past anything the shift leaves.
            10 => {
                let width = [8, 16, 32, 63, 64, 128][self.rng.random_range(0..6)];
                let shift = self.rng.random_range(1..width.min(5));
                let shifted = Expr::Binary(
                    Op::Lshr,
                    Box::new(self.number(width, deeper)),
                    Box::new(Expr::Const(width, u128::from(shift))),
                );
                let largest = mask(width) >> shift;
                let constant = match self.rng.random_range(0..4) {
                    0 => largest,
                    1 => largest.saturating_add(1) & mask(width),
                    _ => self.constant(width),
                };
                let comparison = [Comparison::Ult, Comparison::Ule][self.rng.random_range(0..2)];
                let constant = Box::new(Expr::Const(width, constant));
                if self.rng.random_bool(0.5) {
                    Expr::Compare(comparison, Box::new(shifted), constant)
                } else {
                    Expr::Compare(comparison, constant, Box::new(shifted))
                }
            }
            // A quotient compared with 0, as a program asks whether any digits are left.
            9 => {
                let width = [8, 16, 32, 64, 128][self.rng.random_range(0..5)];
                let divisor = [3, 10, 100, 1000][self.rng.random_range(0..4)] & mask(width);
                let quotient = Expr::Binary(
                    Op::Udiv,
                    Box::new(self.number(width, deeper)),
                    Box::new(Expr::Const(width, divisor.max(2))),
                );
                Expr::Compare(
                    Comparison::Eq,
                    Box::new(quotient),
                    Box::new(Expr::Const(width, 0)),
                )
            }
            // The sign bit of a value, as flags read it.
            6 => {
                let width = self.widths[self.rng.random_range(1..self.widths.len())];
                let value = self.number(width, deeper);
                Expr::Compare(
                    Comparison::Eq,
                    Box::new(Expr::Extract(Box::new(value), width - 1, width - 1)),
                    Box::new(Expr::Const(1, u128::from(self.rng.random_bool(0.5)))),
                )
            }
            // A negation against a constant, or a narrow sum against one, both of which the
            // pool rewrites.
            7 | 8 => {
                let width = [8, 16, 32, 64][self.rng.random_range(0..4)];
                let inner = self.number(width, deeper);
                let value = if self.rng.random_bool(0.5) {
                    Expr::Neg(Box::new(inner))
                } else {
                    inner
                };
                let comparisons = [
                    Comparison::Slt,
                    Comparison::Sle,
                    Comparison::Ult,
                    Comparison::Eq,
                ];
                let comparison = comparisons[self.rng.random_range(0..comparisons.len())];
                let constant = Expr::Const(width, self.constant(width));
                if self.rng.random_bool(0.5) {
                    Expr::Compare(comparison, Box::new(value), Box::new(constant))
                } else {
                    Expr::Compare(comparison, Box::new(constant), Box::new(value))
                }
            }
            0..=3 => {
                let width = self.widths[self.rng.random_range(1..self.widths.len())];
                let comparisons = [
                    Comparison::Eq,
                    Comparison::Ult,
                    Comparison::Ule,
                    Comparison::Slt,
                    Comparison::Sle,
                ];
                let comparison = comparisons[self.rng.random_range(0..comparisons.len())];
                let a = self.number(width, deeper);
                let b = if self.rng.random_bool(0.5) {
                    Expr::Const(width, self.constant(width))
                } else {
                    self.number(width, deeper)
                };
                Expr::Compare(comparison, Box::new(a), Box::new(b))
            }
            4 => Expr::Logic(
                self.rng.random_bool(0.5),
                Box::new(self.truth(deeper)),
                Box::new(self.truth(deeper)),
            ),
            _ => Expr::BoolNot(Box::new(self.truth(deeper))),
        }
    }
}

/// A value for `variable`, often at its edges.
fn draw(rng: &mut ChaCha8Rng, variable: &Variable) -> u128 {
    let Variable { low, high, .. } = *variable;
    match rng.random_range(0..5) {
        0 => low,
        1 => high,
        2 => low + (high - low) / 2,
        _ => {
            let span = high - low;
            if span == u128::MAX {
                rng.random()
            } else {
                low + rng.random::<u128>() % (span + 1)
            }
        }
    }
}

fn seed() -> u64 {
    std::env::var("LENGUA_RANDOM_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or_else(|| {
            let mut bytes = [0; 8];
            getrandom::fill(&mut bytes).expect("a seed");
            u64::from_le_bytes(bytes)
        })
}

/// Pool variables for `variables`, named `{prefix}v{index}`, each with its range when it has one.
fn declare(pool: &mut Pool, variables: &[Variable], prefix: &str) -> Vec<Term> {
    variables
        .iter()
        .enumerate()
        .map(|(index, variable)| {
            let name = format!("{prefix}v{index}");
            if (variable.low, variable.high) == (0, mask(variable.width)) {
                pool.var(variable.width, &name)
            } else {
                pool.var_in(variable.width, &name, variable.low, variable.high)
            }
        })
        .collect()
}

/// Checks every term `term` is made of: its value at `values` must lie in its ranges.
fn ranges_hold(pool: &Pool, term: Term, values: &HashMap<String, u128>) -> Result<(), String> {
    let mut pending = vec![term];
    let mut seen = std::collections::HashSet::new();
    while let Some(current) = pending.pop() {
        if !seen.insert(current) {
            continue;
        }
        pending.extend(pool.operands(current));
        let Some(value) = pool.evaluate(current, values) else {
            continue;
        };
        if matches!(pool.sort(current), super::Sort::Bool) {
            continue;
        }
        let width = pool.width(current);
        let (low, high) = pool.range(current);
        if !(low..=high).contains(&value) {
            return Err(format!(
                "t{} = {} is {value:#x}, outside its range {low:#x}..={high:#x}",
                current.index(),
                pool.smt_node(current)
            ));
        }
        let (low, high) = pool.signed_range(current);
        let signed_value = signed(value, width);
        if !(low..=high).contains(&signed_value) {
            return Err(format!(
                "t{} = {} is {signed_value} signed, outside its range {low}..={high}",
                current.index(),
                pool.smt_node(current)
            ));
        }
    }
    Ok(())
}

#[test]
fn simplified_terms_keep_their_values_and_ranges() {
    let seed = seed();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut problems = Vec::new();
    let mut checked = 0;
    for _ in 0..4000 {
        let mut writer = Writer {
            rng: &mut rng,
            variables: Vec::new(),
            widths: &WIDTHS,
        };
        let expr = if writer.rng.random_bool(0.25) {
            writer.truth(4)
        } else {
            let width = WIDTHS[writer.rng.random_range(0..WIDTHS.len())];
            writer.number(width, 4)
        };
        let variables = writer.variables;
        let mut pool = Pool::default();
        let terms = declare(&mut pool, &variables, "");
        let term = build(&mut pool, &expr, &terms);
        for _ in 0..8 {
            let values: Vec<u128> = variables
                .iter()
                .map(|variable| draw(&mut rng, variable))
                .collect();
            let by_name: HashMap<String, u128> = values
                .iter()
                .enumerate()
                .map(|(index, value)| (format!("v{index}"), *value))
                .collect();
            let (expected, _) = reference(&expr, &variables, &values);
            checked += 1;
            match pool.evaluate(term, &by_name) {
                Some(value) if value == expected => {}
                found => problems.push(format!(
                    "{expr:?} at {values:?}: the pool's terms give {found:?}, plain arithmetic \
                     gives {expected}"
                )),
            }
            if let Err(problem) = ranges_hold(&pool, term, &by_name) {
                problems.push(format!("{expr:?} at {values:?}: {problem}"));
            }
        }
    }
    let count = problems.len();
    problems.truncate(10);
    assert!(
        problems.is_empty(),
        "seed {seed}: {count} of {checked} evaluations went wrong:\n{}",
        problems.join("\n")
    );
}

#[test]
fn a_crc_is_one_term_and_reads_the_same_in_the_solvers() {
    // The known check values of "123456789", from all ones, inverted at the end.
    let mut pool = Pool::default();
    for (poly, check) in [
        (super::CASTAGNOLI, 0xE306_9283_u128),
        (super::ZLIB, 0xCBF4_3926),
    ] {
        let mut state = pool.bv(32, u128::from(u32::MAX));
        for byte in b"123456789" {
            let byte = pool.bv(8, u128::from(*byte));
            state = pool.crc(state, byte, poly);
        }
        let ones = pool.bv(32, u128::from(u32::MAX));
        let value = pool.xor(state, ones);
        assert_eq!(pool.constant(value), Some(check));
    }
    // A qword taken at once, as `crc32 rax, qword ptr [...]` takes it, is its bytes in order.
    let state = pool.var(32, "state");
    let bytes: Vec<Term> = (0..8)
        .map(|index| pool.var(8, &format!("b{index}")))
        .collect();
    let mut by_bytes = state;
    for byte in &bytes {
        by_bytes = pool.crc(by_bytes, *byte, super::CASTAGNOLI);
    }
    let word = bytes[1..]
        .iter()
        .fold(bytes[0], |low, high| pool.concat(*high, low));
    let mut by_word = state;
    for index in 0..8 {
        let byte = pool.extract(word, index * 8 + 7, index * 8);
        by_word = pool.crc(by_word, byte, super::CASTAGNOLI);
    }
    assert_eq!(by_word, by_bytes);
    // The solvers' reading of a step meets the pool's at random points.
    let Ok(mut solver) = Solver::start(3000) else {
        eprintln!("no solver is installed");
        return;
    };
    let mut rng = ChaCha8Rng::seed_from_u64(seed());
    let byte = bytes[0];
    for poly in [super::CASTAGNOLI, super::ZLIB] {
        let step = pool.crc(state, byte, poly);
        for _ in 0..8 {
            let (s, b) = (rng.random::<u32>(), rng.random::<u8>());
            let (s_term, b_term) = (pool.bv(32, u128::from(s)), pool.bv(8, u128::from(b)));
            let fixed = [pool.eq(state, s_term), pool.eq(byte, b_term)];
            let expected = pool.bv(32, u128::from(super::crc_byte(poly, s, b)));
            let same = pool.eq(step, expected);
            let other = pool.bool_not(same);
            let (answer, _) = solver
                .check(&pool, &[fixed[0], fixed[1], other], &[])
                .expect("the solver answers");
            assert_eq!(answer, Answer::Unsat, "{poly:#x} at {s:#x}, {b:#x}");
        }
    }
}

/// Floats: the solvers' IEEE-754 reading of each operation, NaNs and conversions included,
/// meets the pool's, which is the CPU's, at special values and at random ones.
#[test]
fn the_solvers_read_floats_as_the_pool_evaluates_them() {
    use super::{FloatOp, FloatTest, Rounding};
    // A double's square root takes cvc5 about 3 seconds when its operand isn't simply
    // substituted: the 10 seconds a proof's question gets.
    let Ok(mut solver) = Solver::start(10_000) else {
        eprintln!("no solver is installed");
        return;
    };
    let mut rng = ChaCha8Rng::seed_from_u64(seed());
    let draw = |rng: &mut ChaCha8Rng, width: u32| -> u128 {
        let specials: [f64; 12] = [
            0.0,
            -0.0,
            1.0,
            -2.5,
            0.5,
            1e300,
            -1e-300,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
            9_007_199_254_740_993.0,
            3.0e9,
        ];
        let pick = specials[rng.random_range(0..specials.len())];
        match (rng.random_range(0..3), width) {
            (0, 64) => u128::from(rng.random::<u64>()),
            (0, _) => u128::from(rng.random::<u32>()),
            (_, 64) => u128::from(pick.to_bits()),
            #[allow(clippy::cast_possible_truncation)]
            _ => u128::from((pick as f32).to_bits()),
        }
    };
    let mut problems = Vec::new();
    let mut slow = 0;
    for round in 0..120 {
        // A float's definition stays asserted for its session, as in a proof, where each input
        // combination starts over.
        solver.reset().expect("the solvers start over");
        let mut pool = Pool::default();
        let width = if round % 3 == 0 { 32 } else { 64 };
        let names = [
            format!("fa{round}"),
            format!("fb{round}"),
            format!("fc{round}"),
        ];
        let [a, b, c] = names.clone().map(|name| pool.var(width, &name));
        let int = pool.var(64, &format!("fi{round}"));
        let term = match round % 10 {
            0 => pool.float(FloatOp::Add, a, b),
            1 => pool.float(FloatOp::Sub, a, b),
            2 => pool.float(FloatOp::Mul, a, b),
            3 => pool.float(FloatOp::Div, a, b),
            4 => pool.float_sqrt(a),
            5 => pool.float_fma(a, b, c),
            6 => pool.float_from_int(int, width),
            7 => pool.float_to_int(a, [32, 64][round % 2], round % 4 == 1),
            8 => pool.float_round(
                a,
                [
                    Rounding::Nearest,
                    Rounding::Down,
                    Rounding::Up,
                    Rounding::Zero,
                ][round % 4],
            ),
            _ => pool.float_convert(a, 96 - width),
        };
        let mut values: Vec<(Term, u128)> = [a, b, c]
            .into_iter()
            .map(|variable| (variable, draw(&mut rng, width)))
            .collect();
        values.push((int, u128::from(rng.random::<u64>())));
        let by_name: HashMap<String, u128> = names
            .iter()
            .cloned()
            .zip(values.iter().map(|(_, value)| *value))
            .chain([(format!("fi{round}"), values[3].1)])
            .collect();
        let Some(expected) = pool.evaluate(term, &by_name) else {
            continue;
        };
        let mut conditions: Vec<Term> = values
            .iter()
            .map(|(variable, value)| {
                let width = pool.width(*variable);
                let constant = pool.bv(width, *value);
                pool.eq(*variable, constant)
            })
            .collect();
        let constant = pool.bv(pool.width(term), expected);
        let same = pool.eq(term, constant);
        conditions.push(pool.bool_not(same));
        // A comparison of the same values, too.
        let test = [FloatTest::Less, FloatTest::Equal, FloatTest::Unordered][round % 3];
        let tested = pool.float_test(test, a, b);
        let holds = pool.evaluate(tested, &by_name);
        let (answer, _) = solver
            .check(&pool, &conditions, &[])
            .expect("the solver answers");
        // Out of time is no answer, which a busy machine makes more of; any other answer but
        // impossible is a mistake.
        if answer == Answer::Unknown && solver.why_unknown.ends_with("ran out of time") {
            slow += 1;
        } else if answer != Answer::Unsat {
            problems.push(format!(
                "round {round}: {} at {values:?}: the pool says {expected:#x}, the solvers {answer:?}",
                pool.sketch(term, 2)
            ));
        }
        if let Some(holds) = holds {
            let fixed = conditions[..4].to_vec();
            let other = if holds == 1 {
                pool.bool_not(tested)
            } else {
                tested
            };
            let mut asked = fixed;
            asked.push(other);
            let (answer, _) = solver
                .check(&pool, &asked, &[])
                .expect("the solver answers");
            if answer == Answer::Unknown && solver.why_unknown.ends_with("ran out of time") {
                slow += 1;
            } else if answer != Answer::Unsat {
                problems.push(format!("round {round}: {test:?} at {values:?}: {answer:?}"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert!(slow < 12, "{slow} of 240 questions ran out of time");
}

#[test]
fn the_solvers_read_terms_as_the_pool_evaluates_them() {
    let Ok(mut solver) = Solver::start(3000) else {
        eprintln!("no solver is installed");
        return;
    };
    let seed = seed();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut problems = Vec::new();
    let (mut slow, mut unfound) = (0, 0);
    let mut pool = Pool::default();
    for round in 0..150 {
        let mut writer = Writer {
            rng: &mut rng,
            variables: Vec::new(),
            widths: &NARROW,
        };
        let expr = if round % 4 == 0 {
            writer.truth(3)
        } else {
            let width = NARROW[writer.rng.random_range(0..NARROW.len())];
            writer.number(width, 3)
        };
        let variables = writer.variables;
        // A solver knows the terms of one pool, so both start over together now and then.
        if round % 25 == 0 {
            solver = Solver::start(3000).expect("the solver starts again");
            pool = Pool::default();
        }
        let prefix = format!("r{round}");
        let terms = declare(&mut pool, &variables, &prefix);
        let term = build(&mut pool, &expr, &terms);
        let values: Vec<u128> = variables
            .iter()
            .map(|variable| draw(&mut rng, variable))
            .collect();
        let by_name: HashMap<String, u128> = values
            .iter()
            .enumerate()
            .map(|(index, value)| (format!("{prefix}v{index}"), *value))
            .collect();
        let Some(value) = pool.evaluate(term, &by_name) else {
            continue;
        };
        let mut fixed: Vec<Term> = terms
            .iter()
            .zip(&values)
            .map(|(variable, value)| {
                let width = pool.width(*variable);
                let constant = pool.bv(width, *value);
                pool.eq(*variable, constant)
            })
            .collect();
        // With every variable fixed, the term can't be anything but the pool's value.
        let other = if matches!(pool.sort(term), super::Sort::Bool) {
            if value == 1 {
                pool.bool_not(term)
            } else {
                term
            }
        } else {
            let constant = pool.bv(pool.width(term), value);
            let same = pool.eq(term, constant);
            pool.bool_not(same)
        };
        let same = pool.bool_not(other);
        let mut exact = fixed.clone();
        fixed.push(other);
        exact.push(same);
        // The term can't be anything else, and it can be that: an encoding that ruled the
        // right value out would make every question impossible, and every proof false. That
        // shows as a quick impossible, so finding the inputs gets only a moment.
        for (conditions, expected) in [(&fixed, Answer::Unsat), (&exact, Answer::Sat)] {
            let moment = std::time::Instant::now() + std::time::Duration::from_millis(500);
            solver.until(if expected == Answer::Sat {
                moment
            } else {
                moment + std::time::Duration::from_secs(3600)
            });
            let (answer, _) = solver
                .check(&pool, conditions, &[])
                .expect("the solver answers");
            // Out of time is no answer, which a proof reports as not proven; any other answer
            // but the expected one is a mistake.
            if answer == Answer::Unknown && solver.why_unknown.ends_with("ran out of time") {
                if expected == Answer::Unsat {
                    slow += 1;
                } else {
                    unfound += 1;
                }
            } else if answer != expected {
                problems.push(format!(
                    "{expr:?} at {values:?}: the pool says {value:#x}, but the solvers answer \
                     {answer:?} where {expected:?} is right ({})",
                    solver.why_unknown
                ));
            }
        }
    }
    problems.truncate(10);
    assert!(problems.is_empty(), "seed {seed}:\n{}", problems.join("\n"));
    assert!(
        slow < 15,
        "seed {seed}: {slow} of 150 questions ran out of time"
    );
    // Under a busy machine many run out of their moment, which says nothing; enough must be
    // found for the check to mean something.
    assert!(
        unfound < 125,
        "seed {seed}: the solvers found the inputs for only {} of 150 terms' values",
        150 - unfound
    );
}

/// One of the ways a program or a formula writes `variable × times`: a product, a shift, a
/// negation, or the multiple split in two.
fn multiple(
    pool: &mut Pool,
    rng: &mut ChaCha8Rng,
    variable: Term,
    times: u128,
    width: u32,
) -> Term {
    let constant = |pool: &mut Pool, value: u128| pool.bv(width, value & mask(width));
    match rng.random_range(0..4) {
        0 if times.is_power_of_two() && times.trailing_zeros() < width => {
            let shift = constant(pool, u128::from(times.trailing_zeros()));
            pool.shl(variable, shift)
        }
        1 => {
            let negated = constant(pool, negate(times, width));
            let product = pool.mul(negated, variable);
            pool.neg(product)
        }
        2 => {
            let first = rng.random::<u128>() & mask(width);
            let rest = times.wrapping_sub(first) & mask(width);
            let (first, rest) = (constant(pool, first), constant(pool, rest));
            let (a, b) = (pool.mul(variable, first), pool.mul(rest, variable));
            pool.add(a, b)
        }
        _ => {
            let times = constant(pool, times);
            pool.mul(variable, times)
        }
    }
}

/// A sum of `parts` and `constant`, written one random way: its parts shuffled, each a random
/// way, grouped to the left, to the right, or subtracted.
fn written(
    pool: &mut Pool,
    rng: &mut ChaCha8Rng,
    parts: &[(Term, u128)],
    constant: u128,
    width: u32,
) -> Term {
    let mut order: Vec<usize> = (0..parts.len()).collect();
    for index in (1..order.len()).rev() {
        order.swap(index, rng.random_range(0..=index));
    }
    let mut terms: Vec<Term> = order
        .iter()
        .map(|&index| multiple(pool, rng, parts[index].0, parts[index].1, width))
        .collect();
    terms.push(pool.bv(width, constant));
    if rng.random_bool(0.5) {
        let mut total = terms[0];
        for term in &terms[1..] {
            total = if rng.random_bool(0.3) {
                let negated = pool.neg(*term);
                pool.sub(total, negated)
            } else {
                pool.add(total, *term)
            };
        }
        total
    } else {
        let mut total = *terms.last().unwrap_or(&terms[0]);
        for term in terms.iter().rev().skip(1) {
            total = pool.add(*term, total);
        }
        total
    }
}

/// A sum is one term however it's written: parts in any order and grouping, each as a product,
/// a shift, a negation or a split multiple, and its value is the plain arithmetic's.
#[test]
fn a_sum_is_one_term_however_it_is_written() {
    let seed = seed();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    for round in 0..400 {
        let mut pool = Pool::default();
        let width = [8, 16, 32, 64, 128][rng.random_range(0..5)];
        let parts: Vec<(Term, u128)> = (0..rng.random_range(1..6))
            .map(|index| {
                let variable = pool.var(width, &format!("v{index}"));
                let times = match rng.random_range(0..4) {
                    0 => 1 << rng.random_range(0..width.min(8)),
                    1 => negate(rng.random_range(1..20), width),
                    _ => rng.random::<u128>() & mask(width),
                };
                (variable, times)
            })
            .collect();
        let constant = rng.random::<u128>() & mask(width);
        let first = written(&mut pool, &mut rng, &parts, constant, width);
        let second = written(&mut pool, &mut rng, &parts, constant, width);
        assert_eq!(
            first,
            second,
            "seed {seed}, round {round}: {} and {} are the same sum",
            pool.sketch(first, 6),
            pool.sketch(second, 6)
        );
        let values: HashMap<String, u128> = (0..parts.len())
            .map(|index| (format!("v{index}"), rng.random::<u128>() & mask(width)))
            .collect();
        let expected = parts
            .iter()
            .enumerate()
            .fold(constant, |total, (index, (_, times))| {
                total.wrapping_add(values[&format!("v{index}")].wrapping_mul(*times)) & mask(width)
            });
        assert_eq!(
            pool.evaluate(first, &values),
            Some(expected),
            "seed {seed}, round {round}"
        );
    }
}

/// Widening a sum part by part is right only when the sum never wraps: in range it's one term
/// with the wide arithmetic, out of range it stays apart and keeps its value.
#[test]
fn a_sum_widens_part_by_part_only_when_it_fits() {
    let mut pool = Pool::default();
    let (x, y) = (pool.var_in(8, "x", 0, 100), pool.var_in(8, "y", 0, 100));
    let sum = pool.add(x, y);
    let widened = pool.zero_extend(sum, 8);
    let (wide_x, wide_y) = (pool.zero_extend(x, 8), pool.zero_extend(y, 8));
    assert_eq!(widened, pool.add(wide_x, wide_y), "200 fits in 8 bits");
    let difference = pool.sub(x, y);
    let signed_wide = pool.sign_extend(difference, 8);
    assert_eq!(
        signed_wide,
        pool.sub(wide_x, wide_y),
        "-100..100 fits signed"
    );

    let (a, b) = (pool.var(8, "a"), pool.var(8, "b"));
    let wraps = pool.add(a, b);
    let widened = pool.zero_extend(wraps, 8);
    let (wide_a, wide_b) = (pool.zero_extend(a, 8), pool.zero_extend(b, 8));
    assert_ne!(
        widened,
        pool.add(wide_a, wide_b),
        "255 + 255 wraps in 8 bits"
    );
    let values = HashMap::from([("a".to_owned(), 200), ("b".to_owned(), 100)]);
    assert_eq!(pool.evaluate(widened, &values), Some(44));
}

/// A shift right and a division by a power of two, or a remainder and the low bits, are one
/// term.
#[test]
fn a_power_of_two_divides_as_a_shift() {
    let mut pool = Pool::default();
    let x = pool.var(64, "x");
    for shift in [1_u32, 3, 12, 63] {
        let by = pool.bv(64, u128::from(shift));
        let power = pool.bv(64, 1 << shift);
        assert_eq!(pool.lshr(x, by), pool.udiv(x, power), "x >> {shift}");
        let low = pool.extract(x, shift - 1, 0);
        let kept = pool.zero_extend(low, 64 - shift);
        assert_eq!(kept, pool.urem(x, power), "x % 2^{shift}");
    }
}

#[test]
fn a_halved_value_compares_as_the_value_itself() {
    // A program asks whether x / 2 / 10^4 is 0; a formula whether x >> 1 is below 10^4.
    let mut pool = Pool::default();
    let x = pool.var(64, "x");
    let (one, two, ten_thousand) = (pool.bv(64, 1), pool.bv(64, 2), pool.bv(64, 10_000));
    let halved = pool.lshr(x, one);
    let below = pool.ult(halved, ten_thousand);
    let twenty_thousand = pool.bv(64, 20_000);
    assert_eq!(below, pool.ult(x, twenty_thousand));
    let half = pool.udiv(x, two);
    let quotient = pool.udiv(half, ten_thousand);
    let zero = pool.bv(64, 0);
    assert_eq!(pool.eq(quotient, zero), below, "x / 2 / 10^4 = 0");
    // 10^4 < x >> 1 is 20002 <= x.
    let above = pool.ult(ten_thousand, halved);
    let least = pool.bv(64, 20_002);
    assert_eq!(above, pool.ule(least, x));
    // Nothing the top 63 bits hold is above the largest of them.
    let top = pool.extract(x, 63, 1);
    let largest = pool.bv(63, (1 << 63) - 1);
    let past = pool.ult(largest, top);
    assert_eq!(pool.truth(past), Some(false));
}

#[test]
fn low_bits_widened_past_their_value_are_the_value() {
    // A 64-bit quotient a program narrowed to 63 bits, widened to 128 by a formula.
    let mut pool = Pool::default();
    let x = pool.var(64, "x");
    let twenty = pool.bv(64, 20);
    let quotient = pool.udiv(x, twenty);
    let low = pool.extract(quotient, 62, 0);
    let wide = pool.zero_extend(low, 65);
    assert_eq!(wide, pool.zero_extend(quotient, 64));
    // Not where the bits kept don't hold the value: x / 2 needs all 63.
    let two = pool.bv(64, 2);
    let half = pool.udiv(x, two);
    let low = pool.extract(half, 61, 0);
    let wide = pool.zero_extend(low, 66);
    assert_ne!(wide, pool.zero_extend(half, 64));
}

#[test]
fn where_a_condition_on_one_value_holds_is_exactly_its_set() {
    // Random conditions on one value x, as a program asks whether a byte is inside the text:
    // comparisons of x moved by a constant, of its low byte, of a choice of a constant or a moved
    // x, and a bit of the low mask of that many bits, joined by and, or and not. The set
    // holds_where gives must hold at each value of the range and nowhere else.
    let seed = seed();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut read = 0;
    for round in 0..400 {
        let mut pool = Pool::default();
        let low = rng.random_range(0..300_u128);
        let high = low + rng.random_range(0..600_u128);
        let x = pool.var_in(64, "x", low, high);
        let moved = |pool: &mut Pool, rng: &mut ChaCha8Rng| {
            let offset = pool.bv_signed(64, rng.random_range(-700_i128..700));
            let value = pool.add(x, offset);
            match rng.random_range(0..4) {
                0 => {
                    let byte = pool.extract(value, 7, 0);
                    pool.zero_extend(byte, 56)
                }
                1 => {
                    let k = pool.bv(64, rng.random_range(0..40));
                    let wide = pool.bv(64, rng.random_range(0..40));
                    let small = pool.ult(k, value);
                    pool.ite(small, wide, value)
                }
                _ => value,
            }
        };
        let atom = |pool: &mut Pool, rng: &mut ChaCha8Rng| {
            let value = moved(pool, rng);
            let k = pool.bv(64, rng.random_range(0..800));
            match rng.random_range(0..5) {
                0 => pool.ult(value, k),
                1 => pool.ult(k, value),
                2 => pool.eq(value, k),
                3 => {
                    // bit b of (1 << n) - 1, n the value's low byte, as bzhi reads its index
                    let byte = pool.extract(value, 7, 0);
                    let n = pool.zero_extend(byte, 24);
                    let one = pool.bv(32, 1);
                    let shifted = pool.shl(one, n);
                    let low_mask = pool.sub(shifted, one);
                    let bit = rng.random_range(0..32);
                    let set = pool.extract(low_mask, bit, bit);
                    let zero = pool.bv(1, 0);
                    let clear = pool.eq(set, zero);
                    pool.bool_not(clear)
                }
                _ => {
                    let other = pool.bv(64, rng.random_range(0..800));
                    pool.ule(value, other)
                }
            }
        };
        let mut condition = atom(&mut pool, &mut rng);
        for _ in 0..rng.random_range(0..4) {
            let next = atom(&mut pool, &mut rng);
            condition = match rng.random_range(0..3) {
                0 => pool.bool_and(condition, next),
                1 => pool.bool_or(condition, next),
                _ => {
                    let not = pool.bool_not(next);
                    pool.bool_and(condition, not)
                }
            };
        }
        let Some(set) = pool.holds_where(condition, x, (low, high)) else {
            continue;
        };
        read += 1;
        for value in low..=high {
            let values = HashMap::from([("x".to_owned(), value)]);
            let holds = pool.evaluate(condition, &values) == Some(1);
            let inside = set.iter().any(|(from, to)| (*from..=*to).contains(&value));
            assert_eq!(
                holds,
                inside,
                "seed {seed}, round {round}: x = {value}, {} gave {set:?}",
                pool.sketch(condition, 8)
            );
        }
        // Written back as comparisons, the condition is the same.
        let written = pool.on_length(condition, x);
        for value in low..=high {
            let values = HashMap::from([("x".to_owned(), value)]);
            assert_eq!(
                pool.evaluate(written, &values),
                pool.evaluate(condition, &values),
                "seed {seed}, round {round}: x = {value}, {} written as {}",
                pool.sketch(condition, 8),
                pool.sketch(written, 8)
            );
        }
    }
    // Every condition here is of that kind, so nearly all are read.
    assert!(
        read > 300,
        "seed {seed}: only {read} of 400 conditions read"
    );
}

#[test]
fn a_condition_as_deep_as_a_long_text_is_read_without_a_call_for_each_level() {
    // A matcher's state built byte by byte, as a count of a part that overlaps itself keeps it,
    // `a_k and not (a_k-1 and not (...))`, 100,000 deep: it isn't about x, which its first byte
    // shows at once, and a condition guarded by it keeps its form. A choice of x's values as
    // deep, x one of 100,000 lengths, is read as its one interval.
    let mut pool = Pool::default();
    let range = (0, 200_000);
    let x = pool.var_in(64, "x", range.0, range.1);
    let letter = pool.bv(8, u128::from(b'a'));
    let mut state = pool.boolean(false);
    for at in 0..100_000 {
        // The state before first, as a program's is: the deeper operand is the earlier term.
        let not_state = pool.bool_not(state);
        let byte = pool.var(8, &format!("in.{at}"));
        let is_letter = pool.eq(byte, letter);
        state = pool.bool_and(is_letter, not_state);
    }
    assert_eq!(pool.holds_where(state, x, range), None);
    let five = pool.bv(64, 5);
    let inside = pool.ult(five, x);
    let guarded = pool.bool_and(state, inside);
    assert_eq!(pool.on_length(guarded, x), guarded);
    let mut any = pool.boolean(false);
    for at in 0..100_000_u128 {
        let length = pool.bv(64, at);
        let here = pool.eq(x, length);
        any = pool.bool_or(here, any);
    }
    assert_eq!(pool.holds_where(any, x, range), Some(vec![(0, 99_999)]));
}
