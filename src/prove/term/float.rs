//! IEEE-754 floats as the x86 SSE and AVX instructions compute them, on bit patterns 32 bits
//! wide for singles and 64 for doubles: rounding to nearest with ties to even, as the default
//! MXCSR does; a NaN operand comes through quieted, the first one's first; an operation with no
//! meaning, such as 0 / 0, gives the default NaN; and a conversion to a whole number that doesn't
//! fit gives the "integer indefinite", only the top bit set.

/// The four operations of two floats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FloatOp {
    /// `addsd`
    Add,
    /// `subsd`
    Sub,
    /// `mulsd`
    Mul,
    /// `divsd`
    Div,
}

/// How a float rounds to a whole float, as `roundsd`'s immediate picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rounding {
    /// To the nearest, ties to even.
    Nearest,
    /// Toward minus infinity.
    Down,
    /// Toward plus infinity.
    Up,
    /// Toward zero.
    Zero,
}

/// What `ucomisd` tells about two floats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FloatTest {
    /// The first is less than the second: false for a NaN.
    Less,
    /// They're equal: false for a NaN; true for zeros of either sign.
    Equal,
    /// Either is a NaN.
    Unordered,
}

/// The quiet bit of a NaN of `width` bits.
#[must_use]
pub const fn quiet_bit(width: u32) -> u128 {
    if width == 32 { 1 << 22 } else { 1 << 51 }
}

/// The default NaN of `width` bits, which an operation with no meaning gives.
#[must_use]
pub const fn default_nan(width: u32) -> u128 {
    if width == 32 {
        0xFFC0_0000
    } else {
        0xFFF8_0000_0000_0000
    }
}

/// Whether `bits` is a NaN of `width` bits.
#[must_use]
pub fn is_nan(bits: u128, width: u32) -> bool {
    if width == 32 {
        f32::from_bits(bits as u32).is_nan()
    } else {
        f64::from_bits(bits as u64).is_nan()
    }
}

/// A value of `width` bits as a double, exactly.
fn value(bits: u128, width: u32) -> f64 {
    if width == 32 {
        f64::from(f32::from_bits(bits as u32))
    } else {
        f64::from_bits(bits as u64)
    }
}

/// The bits of a result of `width` bits: a NaN becomes the default NaN.
fn bits_of(result: f64, width: u32) -> u128 {
    if result.is_nan() {
        return default_nan(width);
    }
    if width == 32 {
        #[allow(clippy::cast_possible_truncation)] // Exact: computed as a single already.
        u128::from((result as f32).to_bits())
    } else {
        u128::from(result.to_bits())
    }
}

/// The first NaN among `operands`, quieted.
fn first_nan(operands: &[u128], width: u32) -> Option<u128> {
    operands
        .iter()
        .find(|operand| is_nan(**operand, width))
        .map(|operand| operand | quiet_bit(width))
}

/// `op` of two floats of `width` bits.
#[must_use]
pub fn binary(op: FloatOp, a: u128, b: u128, width: u32) -> u128 {
    if let Some(nan) = first_nan(&[a, b], width) {
        return nan;
    }
    if width == 32 {
        let (x, y) = (f32::from_bits(a as u32), f32::from_bits(b as u32));
        let result = match op {
            FloatOp::Add => x + y,
            FloatOp::Sub => x - y,
            FloatOp::Mul => x * y,
            FloatOp::Div => x / y,
        };
        return bits_of(f64::from(result), 32);
    }
    let (x, y) = (f64::from_bits(a as u64), f64::from_bits(b as u64));
    bits_of(
        match op {
            FloatOp::Add => x + y,
            FloatOp::Sub => x - y,
            FloatOp::Mul => x * y,
            FloatOp::Div => x / y,
        },
        64,
    )
}

/// The square root of a float of `width` bits.
#[must_use]
pub fn sqrt(a: u128, width: u32) -> u128 {
    if let Some(nan) = first_nan(&[a], width) {
        return nan;
    }
    if width == 32 {
        return bits_of(f64::from(f32::from_bits(a as u32).sqrt()), 32);
    }
    bits_of(f64::from_bits(a as u64).sqrt(), 64)
}

/// `a × b + c`, rounded once.
#[must_use]
pub fn fma(a: u128, b: u128, c: u128, width: u32) -> u128 {
    if let Some(nan) = first_nan(&[a, b, c], width) {
        return nan;
    }
    if width == 32 {
        let [x, y, z] = [a, b, c].map(|bits| f32::from_bits(bits as u32));
        return bits_of(f64::from(x.mul_add(y, z)), 32);
    }
    let [x, y, z] = [a, b, c].map(|bits| f64::from_bits(bits as u64));
    bits_of(x.mul_add(y, z), 64)
}

/// A signed whole number of `from` bits as the nearest float of `width` bits.
#[must_use]
pub fn from_int(number: u128, from: u32, width: u32) -> u128 {
    let shift = 128 - from;
    let signed = ((number << shift) as i128) >> shift;
    #[allow(clippy::cast_possible_truncation)] // A 32- or 64-bit number, sign-extended.
    let signed = signed as i64;
    if width == 32 {
        #[allow(clippy::cast_precision_loss)] // Rounds to nearest, ties to even, as cvtsi2ss.
        return u128::from((signed as f32).to_bits());
    }
    #[allow(clippy::cast_precision_loss)] // Rounds to nearest, ties to even, as cvtsi2sd.
    u128::from((signed as f64).to_bits())
}

/// A float of `width` bits rounded to a whole float.
#[must_use]
pub fn round(a: u128, rounding: Rounding, width: u32) -> u128 {
    if let Some(nan) = first_nan(&[a], width) {
        return nan;
    }
    let x = value(a, width);
    bits_of(
        match rounding {
            Rounding::Nearest => x.round_ties_even(),
            Rounding::Down => x.floor(),
            Rounding::Up => x.ceil(),
            Rounding::Zero => x.trunc(),
        },
        width,
    )
}

/// A float of `width` bits as a signed whole number of `to` bits, rounded to nearest with ties
/// to even or truncated; the integer indefinite for NaN or a number out of range.
#[must_use]
pub fn to_int(a: u128, width: u32, to: u32, truncate: bool) -> u128 {
    let indefinite = 1_u128 << (to - 1);
    if is_nan(a, width) {
        return indefinite;
    }
    let x = value(a, width);
    let whole = if truncate {
        x.trunc()
    } else {
        x.round_ties_even()
    };
    let limit = f64::from(2_u32).powi(i32::try_from(to - 1).unwrap_or(63));
    if !(whole >= -limit && whole < limit) {
        return indefinite;
    }
    #[allow(clippy::cast_possible_truncation)] // A whole number inside `to` bits.
    let whole = whole as i64;
    (whole as u128) & (u128::MAX >> (128 - to))
}

/// A float of `from` bits as the nearest float of `to` bits; a NaN keeps its sign and the top of
/// its payload, quieted.
#[must_use]
pub fn convert(a: u128, from: u32, to: u32) -> u128 {
    if is_nan(a, from) {
        let quieted = a | quiet_bit(from);
        return if from == 64 && to == 32 {
            (quieted >> 63) << 31 | 0x7F80_0000 | ((quieted >> 29) & 0x7F_FFFF)
        } else {
            (quieted >> 31) << 63 | 0x7FF0_0000_0000_0000 | ((quieted & 0x7F_FFFF) << 29)
        };
    }
    let x = value(a, from);
    if to == 32 {
        #[allow(clippy::cast_possible_truncation)] // Rounds to nearest, ties to even.
        return u128::from((x as f32).to_bits());
    }
    u128::from(x.to_bits())
}

/// What `test` says of two floats of `width` bits.
#[must_use]
pub fn test(test: FloatTest, a: u128, b: u128, width: u32) -> bool {
    let (x, y) = (value(a, width), value(b, width));
    match test {
        FloatTest::Less => x < y,
        FloatTest::Equal => x == y,
        FloatTest::Unordered => x.is_nan() || y.is_nan(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_follow_the_cpu() {
        let one = u128::from(1.0_f64.to_bits());
        let two = u128::from(2.0_f64.to_bits());
        assert_eq!(binary(FloatOp::Add, one, one, 64), two);
        let zero = u128::from(0.0_f64.to_bits());
        assert_eq!(binary(FloatOp::Div, zero, zero, 64), default_nan(64));
        // A NaN operand comes through quieted, the first one's first.
        let signaling = 0x7FF0_0000_0000_0001;
        assert_eq!(
            binary(FloatOp::Mul, one, signaling, 64),
            signaling | quiet_bit(64)
        );
        assert_eq!(sqrt(u128::from((-1.0_f64).to_bits()), 64), default_nan(64));
        // cvtsd2si rounds ties to even; out of range is the integer indefinite.
        assert_eq!(to_int(u128::from(2.5_f64.to_bits()), 64, 64, false), 2);
        assert_eq!(to_int(u128::from(2.5_f64.to_bits()), 64, 64, true), 2);
        assert_eq!(to_int(u128::from(3.5_f64.to_bits()), 64, 64, false), 4);
        assert_eq!(
            to_int(u128::from(1e300_f64.to_bits()), 64, 32, false),
            0x8000_0000
        );
        assert_eq!(to_int(u128::from((-0.5_f64).to_bits()), 64, 64, false), 0);
        assert_eq!(
            from_int(u128::from(u64::MAX), 64, 64),
            u128::from((-1.0_f64).to_bits())
        );
        assert_eq!(
            round(u128::from((-0.5_f64).to_bits()), Rounding::Nearest, 64),
            u128::from((-0.0_f64).to_bits())
        );
        assert!(test(
            FloatTest::Equal,
            zero,
            u128::from((-0.0_f64).to_bits()),
            64
        ));
        assert_eq!(
            convert(u128::from(1.5_f64.to_bits()), 64, 32),
            u128::from(1.5_f32.to_bits())
        );
    }
}
