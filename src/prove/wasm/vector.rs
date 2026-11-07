//! SIMD128 in the WebAssembly model: a vector is one 128-bit term, its lanes the terms of its
//! bits, lane 0 the lowest, as memory's lowest bytes are. Each operator splits its vectors into
//! lanes, takes each lane as a term of its width with WebAssembly's semantics, and joins the
//! lanes back, so a lane the operator doesn't change is the term it was, and a lane a program and
//! a formula compute alike, as a comparison of a byte the bitmask and its bits count, is one term.

use super::image::{Binary, Lane, Vector};
use crate::prove::term::{Pool, Term};

/// A vector's bits.
const BITS: u32 = 128;

/// The lowest `width` bits set.
const fn ones(width: u32) -> u128 {
    if width >= 128 {
        u128::MAX
    } else {
        (1 << width) - 1
    }
}

/// `vector`'s lanes, `width` bits each, lane 0 first.
fn lanes(pool: &mut Pool, vector: Term, width: u32) -> Vec<Term> {
    (0..BITS / width)
        .map(|lane| pool.extract(vector, lane * width + width - 1, lane * width))
        .collect()
}

/// One term of `parts`, lowest first, joined as a balanced tree so a lane of it is reached in
/// few steps.
fn join(pool: &mut Pool, parts: &[Term]) -> Term {
    match parts {
        [] => pool.bv(BITS, 0),
        [only] => *only,
        _ => {
            let half = parts.len() / 2;
            let low = join(pool, &parts[..half]);
            let high = join(pool, &parts[half..]);
            pool.concat(high, low)
        }
    }
}

/// The low `width` bits of `scalar`, as a lane.
fn low_bits(pool: &mut Pool, scalar: Term, width: u32) -> Term {
    if pool.width(scalar) > width {
        pool.extract(scalar, width - 1, 0)
    } else {
        scalar
    }
}

/// All ones in a lane of `width` bits where `holds`, else zeros.
fn all_ones(pool: &mut Pool, holds: Term, width: u32) -> Term {
    let (set, clear) = (pool.bv(width, ones(width)), pool.bv(width, 0));
    pool.ite(holds, set, clear)
}

/// 1 where `holds`, else 0, as an i32.
fn truth(pool: &mut Pool, holds: Term) -> Term {
    let (one, zero) = (pool.bv(32, 1), pool.bv(32, 0));
    pool.ite(holds, one, zero)
}

/// Whether `comparison` holds of `a` and `b`, as the integer comparison of that name does.
fn compared(pool: &mut Pool, comparison: Binary, a: Term, b: Term) -> Option<Term> {
    Some(match comparison {
        Binary::Eq => pool.eq(a, b),
        Binary::Ne => {
            let same = pool.eq(a, b);
            pool.bool_not(same)
        }
        Binary::LtS => pool.slt(a, b),
        Binary::LtU => pool.ult(a, b),
        Binary::GtS => pool.slt(b, a),
        Binary::GtU => pool.ult(b, a),
        Binary::LeS => pool.sle(a, b),
        Binary::LeU => pool.ule(a, b),
        Binary::GeS => pool.sle(b, a),
        Binary::GeU => pool.ule(b, a),
        _ => return None,
    })
}

/// `wide`, a signed sum or difference of two lanes of `width` bits, computed one bit wider so it
/// never wraps, held between the least and the most a signed lane holds.
fn saturated(pool: &mut Pool, wide: Term, width: u32) -> Term {
    let wider = width + 1;
    let low = pool.extract(wide, width - 1, 0);
    let most = pool.bv(wider, ones(width - 1));
    let least = pool.bv(wider, ones(wider) - ones(width - 1));
    let over = pool.slt(most, wide);
    let under = pool.slt(wide, least);
    let top = pool.bv(width, ones(width - 1));
    let bottom = pool.bv(width, 1 << (width - 1));
    let inner = pool.ite(under, bottom, low);
    pool.ite(over, top, inner)
}

/// `op` of the lanes `a` and `b`, `width` bits each.
fn lane(pool: &mut Pool, op: Lane, width: u32, a: Term, b: Term) -> Option<Term> {
    let widened = |pool: &mut Pool, value: Term, signed: bool| {
        if signed {
            pool.sign_extend(value, 1)
        } else {
            pool.zero_extend(value, 1)
        }
    };
    Some(match op {
        Lane::Add => pool.add(a, b),
        Lane::Sub => pool.sub(a, b),
        Lane::Mul => pool.mul(a, b),
        Lane::MinS => {
            let below = pool.slt(a, b);
            pool.ite(below, a, b)
        }
        Lane::MinU => {
            let below = pool.ult(a, b);
            pool.ite(below, a, b)
        }
        Lane::MaxS => {
            let below = pool.slt(a, b);
            pool.ite(below, b, a)
        }
        Lane::MaxU => {
            let below = pool.ult(a, b);
            pool.ite(below, b, a)
        }
        Lane::AddSatS | Lane::SubSatS => {
            let (x, y) = (widened(pool, a, true), widened(pool, b, true));
            let wide = if op == Lane::AddSatS {
                pool.add(x, y)
            } else {
                pool.sub(x, y)
            };
            saturated(pool, wide, width)
        }
        Lane::AddSatU => {
            let (x, y) = (widened(pool, a, false), widened(pool, b, false));
            let wide = pool.add(x, y);
            let over = pool.bit_is_set(wide, width);
            let most = pool.bv(width, ones(width));
            let low = pool.extract(wide, width - 1, 0);
            pool.ite(over, most, low)
        }
        Lane::SubSatU => {
            let under = pool.ult(a, b);
            let difference = pool.sub(a, b);
            let zero = pool.bv(width, 0);
            pool.ite(under, zero, difference)
        }
        Lane::AvgrU => {
            let (x, y) = (widened(pool, a, false), widened(pool, b, false));
            let one = pool.bv(width + 1, 1);
            let sum = pool.add(x, y);
            let rounded = pool.add(sum, one);
            pool.extract(rounded, width, 1)
        }
        Lane::Compare(comparison) => {
            let holds = compared(pool, comparison, a, b)?;
            all_ones(pool, holds, width)
        }
    })
}

/// A lane of `width` bits shifted by `count`, an i32 already taken modulo the width.
fn shifted(pool: &mut Pool, op: Binary, value: Term, count: Term, width: u32) -> Option<Term> {
    let count = if width < 32 {
        pool.extract(count, width - 1, 0)
    } else if width > 32 {
        pool.zero_extend(count, width - 32)
    } else {
        count
    };
    Some(match op {
        Binary::Shl => pool.shl(value, count),
        Binary::ShrS => pool.ashr(value, count),
        Binary::ShrU => pool.lshr(value, count),
        _ => return None,
    })
}

/// The byte of `table`, 16 bytes, that `index`, a byte, picks, or zero for an index past 15.
fn picked(pool: &mut Pool, table: &[Term], index: Term) -> Term {
    let zero = pool.bv(8, 0);
    if let Some(known) = pool.constant(index) {
        return usize::try_from(known)
            .ok()
            .and_then(|known| table.get(known))
            .copied()
            .unwrap_or(zero);
    }
    let sixteen = pool.bv(8, 16);
    let inside = pool.ult(index, sixteen);
    let low = pool.extract(index, 3, 0);
    let mut chosen = table[0];
    for (value, byte) in (1_u128..).zip(&table[1..]) {
        let constant = pool.bv(4, value);
        let here = pool.eq(low, constant);
        chosen = pool.ite(here, *byte, chosen);
    }
    pool.ite(inside, chosen, zero)
}

/// How many operands `operation` takes: the vectors, and a scalar where it takes one.
pub(super) const fn operands(operation: &Vector) -> usize {
    match operation {
        Vector::Splat(_)
        | Vector::Extract { .. }
        | Vector::Not
        | Vector::AnyTrue
        | Vector::AllTrue(_)
        | Vector::Bitmask(_)
        | Vector::Neg(_)
        | Vector::Abs(_) => 1,
        Vector::Bitselect => 3,
        Vector::Replace { .. }
        | Vector::And
        | Vector::AndNot
        | Vector::Or
        | Vector::Xor
        | Vector::Lanes { .. }
        | Vector::Shift { .. }
        | Vector::Shuffle(_)
        | Vector::Swizzle => 2,
    }
}

/// `operation` on `operands`, the first the deepest on the stack, as WebAssembly computes it:
/// `None` for an operator of a form the model doesn't take, which the validator would refuse.
#[allow(clippy::too_many_lines)]
pub(super) fn operate(pool: &mut Pool, operation: &Vector, operands: &[Term]) -> Option<Term> {
    let first = *operands.first()?;
    let second = operands.get(1).copied();
    Some(match operation {
        Vector::Splat(width) => {
            let value = low_bits(pool, first, *width);
            let every = vec![value; usize::try_from(BITS / width).ok()?];
            join(pool, &every)
        }
        Vector::Extract {
            width,
            lane,
            signed,
        } => {
            let all = lanes(pool, first, *width);
            let value = *all.get(usize::from(*lane))?;
            match (*width < 32, *signed) {
                (true, true) => pool.sign_extend(value, 32 - width),
                (true, false) => pool.zero_extend(value, 32 - width),
                (false, _) => value,
            }
        }
        Vector::Replace { width, lane } => {
            let mut all = lanes(pool, first, *width);
            let value = low_bits(pool, second?, *width);
            *all.get_mut(usize::from(*lane))? = value;
            join(pool, &all)
        }
        Vector::Not => pool.not(first),
        Vector::And => pool.and(first, second?),
        Vector::AndNot => {
            let inverted = pool.not(second?);
            pool.and(first, inverted)
        }
        Vector::Or => pool.or(first, second?),
        Vector::Xor => pool.xor(first, second?),
        Vector::Bitselect => {
            let chooser = *operands.get(2)?;
            let taken = pool.and(first, chooser);
            let others = pool.not(chooser);
            let kept = pool.and(second?, others);
            pool.or(taken, kept)
        }
        Vector::AnyTrue => {
            let zero = pool.bv(BITS, 0);
            let none = pool.eq(first, zero);
            let any = pool.bool_not(none);
            truth(pool, any)
        }
        Vector::AllTrue(width) => {
            let zero = pool.bv(*width, 0);
            let set: Vec<Term> = lanes(pool, first, *width)
                .into_iter()
                .map(|value| {
                    let clear = pool.eq(value, zero);
                    pool.bool_not(clear)
                })
                .collect();
            let all = pool.all(&set);
            truth(pool, all)
        }
        Vector::Bitmask(width) => {
            let tops: Vec<Term> = (0..BITS / width)
                .map(|lane| {
                    let top = lane * width + width - 1;
                    pool.extract(first, top, top)
                })
                .collect();
            let joined = join(pool, &tops);
            let bits = pool.width(joined);
            pool.zero_extend(joined, 32 - bits)
        }
        Vector::Lanes { width, op } => {
            let (a, b) = (lanes(pool, first, *width), lanes(pool, second?, *width));
            let mut out = Vec::with_capacity(a.len());
            for (x, y) in a.into_iter().zip(b) {
                out.push(lane(pool, *op, *width, x, y)?);
            }
            join(pool, &out)
        }
        Vector::Neg(width) => {
            let negated: Vec<Term> = lanes(pool, first, *width)
                .into_iter()
                .map(|value| pool.neg(value))
                .collect();
            join(pool, &negated)
        }
        Vector::Abs(width) => {
            let zero = pool.bv(*width, 0);
            let sized: Vec<Term> = lanes(pool, first, *width)
                .into_iter()
                .map(|value| {
                    let negative = pool.slt(value, zero);
                    let negated = pool.neg(value);
                    pool.ite(negative, negated, value)
                })
                .collect();
            join(pool, &sized)
        }
        Vector::Shift { width, op } => {
            let below = pool.bv(32, u128::from(width - 1));
            let count = pool.and(second?, below);
            let mut out = Vec::new();
            for value in lanes(pool, first, *width) {
                out.push(shifted(pool, *op, value, count, *width)?);
            }
            join(pool, &out)
        }
        Vector::Shuffle(picks) => {
            let mut both = lanes(pool, first, 8);
            both.extend(lanes(pool, second?, 8));
            let mut out = Vec::with_capacity(16);
            for pick in picks {
                out.push(*both.get(usize::from(*pick))?);
            }
            join(pool, &out)
        }
        Vector::Swizzle => {
            let table = lanes(pool, first, 8);
            let mut out = Vec::with_capacity(16);
            for index in lanes(pool, second?, 8) {
                out.push(picked(pool, &table, index));
            }
            join(pool, &out)
        }
    })
}
