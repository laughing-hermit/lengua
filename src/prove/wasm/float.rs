//! WebAssembly's floats, f32 and f64, on the pool's float terms ([`crate::prove::term::float`]).
//!
//! The pool's terms compute IEEE 754 as x86's SSE does, rounding to nearest with ties to even and
//! subnormals kept, which is every number WebAssembly computes too. They differ only in NaNs, and
//! the engine leaves none of those to the CPU: it makes every NaN an operation gives canonical,
//! the positive quiet NaN with no payload, as lengua's engine is set to (`src/wasm/engine.rs`).
//! So each operation is the pool's own term where its operands' bounds rule a NaN out, which
//! keeps a program's terms the formula's, and elsewhere a choice around it gives the canonical
//! NaN where it is one. `abs`, `neg` and `copysign` change the sign bit alone, of a NaN's too;
//! `min` and `max` give a NaN beside a NaN, and order -0 below +0; `trunc_sat` saturates at either
//! end and gives 0 for a NaN, as AArch64's conversions do, whose model this one shares.

use super::image::Float;
use crate::prove::term::float as ieee;
use crate::prove::term::{FloatOp, FloatTest, Pool, Rounding, Term};

/// The canonical NaN of `width` bits: positive, quiet, with no payload.
const fn canonical_nan(width: u32) -> u128 {
    if width == 32 {
        0x7FC0_0000
    } else {
        0x7FF8_0000_0000_0000
    }
}

/// The sign bit of a float of `width` bits.
const fn sign_bit(width: u32) -> u128 {
    1 << (width - 1)
}

/// The lowest `width` bits set.
const fn ones(width: u32) -> u128 {
    if width >= 128 {
        u128::MAX
    } else {
        (1 << width) - 1
    }
}

/// `value` as a float of `width` bits, exactly: every number made here is a single's too.
fn constant(pool: &mut Pool, value: f64, width: u32) -> Term {
    #[allow(clippy::cast_possible_truncation)] // Exact: powers of two.
    let bits = if width == 32 {
        u128::from((value as f32).to_bits())
    } else {
        u128::from(value.to_bits())
    };
    pool.bv(width, bits)
}

/// What `test` says of `a` and `b`: settled where their bounds settle it.
fn test(pool: &mut Pool, test: FloatTest, a: Term, b: Term) -> Term {
    let settled = pool
        .float_bounds(a)
        .zip(pool.float_bounds(b))
        .and_then(|(a, b)| ieee::test_bounds(test, a, b));
    match settled {
        Some(holds) => pool.boolean(holds),
        None => pool.float_test(test, a, b),
    }
}

/// Whether `a` is a NaN.
fn is_nan(pool: &mut Pool, a: Term) -> Term {
    test(pool, FloatTest::Unordered, a, a)
}

/// `result` as the engine gives it: the canonical NaN where it's a NaN, else itself.
pub(super) fn canonical(pool: &mut Pool, result: Term) -> Term {
    if pool.float_bounds(result).is_some() {
        return result;
    }
    let width = pool.width(result);
    let nan = is_nan(pool, result);
    let canonical = pool.bv(width, canonical_nan(width));
    pool.ite(nan, canonical, result)
}

/// `min` or `max`: a NaN beside anything gives the canonical NaN; of two zeros, -0 is the less.
fn min_max(pool: &mut Pool, max: bool, a: Term, b: Term) -> Term {
    let below = test(pool, FloatTest::Less, a, b);
    let above = test(pool, FloatTest::Less, b, a);
    // Equal floats have the same bits but for zeros, whose signs this combines.
    let equal = if max { pool.and(a, b) } else { pool.or(a, b) };
    let numbers = if max {
        let inner = pool.ite(below, b, equal);
        pool.ite(above, a, inner)
    } else {
        let inner = pool.ite(above, b, equal);
        pool.ite(below, a, inner)
    };
    let (a_nan, b_nan) = (is_nan(pool, a), is_nan(pool, b));
    let nan = pool.bool_or(a_nan, b_nan);
    let width = pool.width(a);
    let canonical = pool.bv(width, canonical_nan(width));
    pool.ite(nan, canonical, numbers)
}

/// A whole number without a sign as the nearest float of `width` bits.
fn unsigned_to_float(pool: &mut Pool, number: Term, width: u32) -> Term {
    let from = pool.width(number);
    if from < 64 {
        let wide = pool.zero_extend(number, 64 - from);
        return pool.float_from_int(wide, width);
    }
    let plain = pool.float_from_int(number, width);
    if pool.signed_range(number).0 >= 0 {
        return plain;
    }
    // From 2^63 up: half of it, its lowest bit kept so the half rounds as the whole would, then
    // doubled, which is exact.
    let one = pool.bv(64, 1);
    let half = pool.lshr(number, one);
    let lowest = pool.and(number, one);
    let kept = pool.or(half, lowest);
    let converted = pool.float_from_int(kept, width);
    let two = constant(pool, 2.0, width);
    let doubled = pool.float(FloatOp::Mul, converted, two);
    let zero = pool.bv(64, 0);
    let high = pool.slt(number, zero);
    pool.ite(high, doubled, plain)
}

/// `trunc_sat_s`: `a` toward zero, as a signed whole number of `to` bits, the least or the most
/// past either end, and 0 for a NaN. The pool's conversion, x86's, gives the least for a NaN or a
/// number out of range: the least is right below the range.
fn whole_signed(pool: &mut Pool, a: Term, to: u32) -> Term {
    let width = pool.width(a);
    let whole = pool.float_to_int(a, to, true);
    let limit = constant(pool, 2_f64.powi(i32::try_from(to).unwrap_or(64) - 1), width);
    let fits = test(pool, FloatTest::Less, a, limit);
    let most = pool.bv(to, ones(to) >> 1);
    let number = pool.ite(fits, whole, most);
    let nan = is_nan(pool, a);
    let zero = pool.bv(to, 0);
    pool.ite(nan, zero, number)
}

/// `trunc_sat_u`: `a` toward zero, as a whole number of `to` bits without a sign, 0 below the
/// range and for a NaN, the most above it.
fn whole_unsigned(pool: &mut Pool, a: Term, to: u32) -> Term {
    let width = pool.width(a);
    let zero_float = pool.bv(width, 0);
    let negative = test(pool, FloatTest::Less, a, zero_float);
    let nan = is_nan(pool, a);
    let none = pool.bool_or(nan, negative);
    let most = pool.bv(to, ones(to));
    let whole = pool.float_to_int(a, 64, true);
    let number = if to == 64 {
        // Below 2^63 the signed conversion is the number; from there to 2^64 it's the conversion
        // of what's above 2^63, exact to subtract, with the top bit set.
        let half = constant(pool, 2_f64.powi(63), width);
        let low = test(pool, FloatTest::Less, a, half);
        let all = constant(pool, 2_f64.powi(64), width);
        let within = test(pool, FloatTest::Less, a, all);
        let above = pool.float(FloatOp::Sub, a, half);
        let above = pool.float_to_int(above, 64, true);
        let top = pool.bv(64, 1 << 63);
        let high = pool.or(above, top);
        let high = pool.ite(within, high, most);
        pool.ite(low, whole, high)
    } else {
        let all = constant(pool, 2_f64.powi(32), width);
        let within = test(pool, FloatTest::Less, a, all);
        let low = pool.extract(whole, 31, 0);
        pool.ite(within, low, most)
    };
    let zero = pool.bv(to, 0);
    pool.ite(none, zero, number)
}

/// 1 where `holds`, else 0, as an i32: what a comparison gives.
fn truth(pool: &mut Pool, holds: Term) -> Term {
    let (one, zero) = (pool.bv(32, 1), pool.bv(32, 0));
    pool.ite(holds, one, zero)
}

/// Whether `operation` takes two operands.
pub(super) const fn binary(operation: Float) -> bool {
    matches!(
        operation,
        Float::Add
            | Float::Sub
            | Float::Mul
            | Float::Div
            | Float::Min
            | Float::Max
            | Float::Copysign
            | Float::Eq
            | Float::Ne
            | Float::Lt
            | Float::Gt
            | Float::Le
            | Float::Ge
    )
}

/// `operation` on `a`, and on `b` where it takes two, as the engine computes it.
pub(super) fn operate(pool: &mut Pool, operation: Float, a: Term, b: Term) -> Term {
    let width = pool.width(a);
    let arithmetic = |pool: &mut Pool, op| {
        let result = pool.float(op, a, b);
        canonical(pool, result)
    };
    let rounded = |pool: &mut Pool, rounding| {
        let result = pool.float_round(a, rounding);
        canonical(pool, result)
    };
    match operation {
        Float::Add => arithmetic(pool, FloatOp::Add),
        Float::Sub => arithmetic(pool, FloatOp::Sub),
        Float::Mul => arithmetic(pool, FloatOp::Mul),
        Float::Div => arithmetic(pool, FloatOp::Div),
        Float::Min => min_max(pool, false, a, b),
        Float::Max => min_max(pool, true, a, b),
        Float::Copysign => {
            let rest = pool.bv(width, sign_bit(width) - 1);
            let sign = pool.bv(width, sign_bit(width));
            let magnitude = pool.and(a, rest);
            let signed = pool.and(b, sign);
            pool.or(magnitude, signed)
        }
        Float::Abs => {
            let rest = pool.bv(width, sign_bit(width) - 1);
            pool.and(a, rest)
        }
        Float::Neg => {
            let sign = pool.bv(width, sign_bit(width));
            pool.xor(a, sign)
        }
        Float::Sqrt => {
            let root = pool.float_sqrt(a);
            canonical(pool, root)
        }
        Float::Ceil => rounded(pool, Rounding::Up),
        Float::Floor => rounded(pool, Rounding::Down),
        Float::Trunc => rounded(pool, Rounding::Zero),
        Float::Nearest => rounded(pool, Rounding::Nearest),
        Float::Eq => {
            let holds = test(pool, FloatTest::Equal, a, b);
            truth(pool, holds)
        }
        Float::Ne => {
            let equal = test(pool, FloatTest::Equal, a, b);
            let holds = pool.bool_not(equal);
            truth(pool, holds)
        }
        Float::Lt => {
            let holds = test(pool, FloatTest::Less, a, b);
            truth(pool, holds)
        }
        Float::Gt => {
            let holds = test(pool, FloatTest::Less, b, a);
            truth(pool, holds)
        }
        Float::Le | Float::Ge => {
            let (low, high) = if operation == Float::Le { (a, b) } else { (b, a) };
            let less = test(pool, FloatTest::Less, low, high);
            let equal = test(pool, FloatTest::Equal, low, high);
            let holds = pool.bool_or(less, equal);
            truth(pool, holds)
        }
        Float::Convert { to, signed: true } => pool.float_from_int(a, to),
        Float::Convert { to, signed: false } => unsigned_to_float(pool, a, to),
        Float::Resize { to } => {
            let resized = pool.float_convert(a, to);
            canonical(pool, resized)
        }
        Float::TruncSat { to, signed: true } => whole_signed(pool, a, to),
        Float::TruncSat { to, signed: false } => whole_unsigned(pool, a, to),
    }
}
