//! AArch64's scalar floating point in the prover's model: the instructions on s and d registers,
//! the low 32 or 64 bits of the V registers, whose 16 bytes each the run keeps where it keeps
//! x86-64's vector registers.
//!
//! The pool's float terms compute as x86-64's SSE does, and AArch64 with Linux's FPCR (rounding
//! to nearest with ties to even, subnormals kept, NaNs passed on) computes every number the same.
//! It differs in NaNs and in whole numbers: a signaling NaN operand comes through before a quiet
//! one, an operation with no meaning gives a positive default NaN, `fmax` and `fmin` pass a NaN
//! on and order -0 below +0, and a conversion to a whole number saturates, a NaN giving 0. Each
//! instruction is the pool's own term where its operands' bounds rule those cases out, which keeps
//! a program's terms the formula's; elsewhere a choice around it makes it AArch64's.

use std::rc::Rc;

use super::super::{Cpu, Last, Step, Stop, unsupported};
use super::bits;
use crate::isa::aarch64::{
    A64Reg, Address, FpConversion, FpOne, FpReg, FpThree, FpTwo, Inst, fp_immediate,
};
use crate::prove::term::{FloatOp, FloatTest, Rounding, Term, float};

/// The bytes of a V register.
const V_BYTES: usize = 16;

/// AArch64's default NaN of `width` bits, which an operation with no meaning gives: positive,
/// where x86-64's is negative.
const fn default_nan(width: u32) -> u128 {
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

/// The bits of an infinity of `width` bits, its sign clear.
const fn infinity(width: u32) -> u128 {
    if width == 32 {
        0x7F80_0000
    } else {
        0x7FF0_0000_0000_0000
    }
}

/// An s register's width, or a d register's.
const fn fp_width(register: FpReg) -> u32 {
    if register.double { 64 } else { 32 }
}

/// The lowest `width` bits set.
const fn ones(width: u32) -> u128 {
    if width >= 128 {
        u128::MAX
    } else {
        (1 << width) - 1
    }
}

impl Cpu<'_> {
    /// Runs the floating-point instruction `inst`.
    pub(super) fn execute_fp(&mut self, inst: Inst) -> Result<Step, Stop> {
        match inst {
            Inst::FpOneSource { op, rd, rn } => {
                let a = self.v(rn);
                let value = self.fp_one(op, a, fp_width(rd));
                self.set_v(rd, value);
            }
            Inst::FpTwoSource { op, rd, rn, rm } => {
                let (a, b) = (self.v(rn), self.v(rm));
                let value = self.fp_two(op, a, b);
                self.set_v(rd, value);
            }
            Inst::FpThreeSource { op, rd, rn, rm, ra } => {
                let (n, m, a) = (self.v(rn), self.v(rm), self.v(ra));
                let value = self.fp_fused(op, n, m, a);
                self.set_v(rd, value);
            }
            Inst::FpCompare { rn, rm, .. } => {
                let a = self.v(rn);
                let b = match rm {
                    Some(rm) => self.v(rm),
                    None => self.pool.bv(fp_width(rn), 0),
                };
                self.fp_compare(a, b);
            }
            Inst::FpCondSelect { rd, rn, rm, cond } => {
                let condition = self.a64_condition(cond);
                let (a, b) = (self.v(rn), self.v(rm));
                let value = self.pool.ite(condition, a, b);
                self.set_v(rd, value);
            }
            Inst::FpMoveImm { rd, imm8 } => {
                let value = self.fp_constant(fp_immediate(imm8), fp_width(rd));
                self.set_v(rd, value);
            }
            Inst::FpConvert { op, fp, gpr } => self.fp_convert(op, fp, gpr),
            Inst::FpLoadStore { load, rt, address } => {
                return self.fp_load_store(load, &[rt], address);
            }
            Inst::FpPair {
                load,
                rt,
                rt2,
                address,
            } => return self.fp_load_store(load, &[rt, rt2], address),
            _ => return Err(unsupported("an integer instruction as floating point")),
        }
        Ok(Step::Next)
    }

    /// The s or d register's value: the low 4 or 8 bytes of its V register.
    fn v(&mut self, register: FpReg) -> Term {
        let bytes = fp_width(register) as usize / 8;
        let parts = self.core.vectors[usize::from(register.number)][..bytes].to_vec();
        self.join(&parts)
    }

    /// Sets the s or d register to `value`, which clears the rest of its V register and the Z
    /// register above it.
    fn set_v(&mut self, register: FpReg, value: Term) {
        let parts = self.split(value, V_BYTES);
        let zero = self.pool.bv(8, 0);
        let vector = Rc::make_mut(&mut self.core.vectors[usize::from(register.number)]);
        vector[..V_BYTES].copy_from_slice(&parts);
        for byte in &mut vector[V_BYTES..] {
            *byte = zero;
        }
    }

    /// `value` as a float of `width` bits, exactly: every number this module makes one of is a
    /// single's too.
    fn fp_constant(&mut self, value: f64, width: u32) -> Term {
        #[allow(clippy::cast_possible_truncation)] // Exact: an fmov number or a power of two.
        let bits = if width == 32 {
            u128::from((value as f32).to_bits())
        } else {
            u128::from(value.to_bits())
        };
        self.pool.bv(width, bits)
    }

    /// What `test` says of `a` and `b`: settled where their bounds settle it.
    pub(super) fn fp_test(&mut self, test: FloatTest, a: Term, b: Term) -> Term {
        let settled = self
            .pool
            .float_bounds(a)
            .zip(self.pool.float_bounds(b))
            .and_then(|(a, b)| float::test_bounds(test, a, b));
        match settled {
            Some(holds) => self.pool.boolean(holds),
            None => self.pool.float_test(test, a, b),
        }
    }

    /// Whether `a` is a NaN.
    fn is_nan(&mut self, a: Term) -> Term {
        self.fp_test(FloatTest::Unordered, a, a)
    }

    /// Whether `a`, a NaN, is a quiet one: its top fraction bit set.
    fn quiet_bit_set(&mut self, a: Term) -> Term {
        let width = self.pool.width(a);
        self.pool.bit_is_set(a, if width == 32 { 22 } else { 51 })
    }

    /// Whether `a` is a quiet NaN.
    fn is_quiet_nan(&mut self, a: Term) -> Term {
        let nan = self.is_nan(a);
        let quiet = self.quiet_bit_set(a);
        self.pool.bool_and(nan, quiet)
    }

    /// Whether `a` is a signaling NaN.
    fn is_signaling(&mut self, a: Term) -> Term {
        let nan = self.is_nan(a);
        let quiet = self.quiet_bit_set(a);
        let loud = self.pool.bool_not(quiet);
        self.pool.bool_and(nan, loud)
    }

    /// `a` with its quiet bit set, as a NaN comes through.
    fn quieted(&mut self, a: Term) -> Term {
        let width = self.pool.width(a);
        let quiet = self.pool.bv(width, float::quiet_bit(width));
        self.pool.or(a, quiet)
    }

    /// `a` with its sign flipped.
    fn negated(&mut self, a: Term) -> Term {
        let width = self.pool.width(a);
        let sign = self.pool.bv(width, sign_bit(width));
        self.pool.xor(a, sign)
    }

    /// Whether `a`, as a float, is the infinity or zero `whole` is with its sign cleared.
    fn magnitude_is(&mut self, a: Term, whole: u128) -> Term {
        let width = self.pool.width(a);
        let rest = self.pool.bv(width, sign_bit(width) - 1);
        let magnitude = self.pool.and(a, rest);
        let whole = self.pool.bv(width, whole);
        self.pool.eq(magnitude, whole)
    }

    /// AArch64's NaN where one of `operands` is a NaN, by its order of priority: the first
    /// signaling one, then the first quiet one, quieted; `otherwise` where none is.
    fn passed_on(&mut self, operands: &[Term], otherwise: Term) -> Term {
        let mut result = otherwise;
        for operand in operands.iter().rev() {
            let nan = self.is_nan(*operand);
            let quiet = self.quieted(*operand);
            result = self.pool.ite(nan, quiet, result);
        }
        for operand in operands.iter().rev() {
            let signaling = self.is_signaling(*operand);
            let quiet = self.quieted(*operand);
            result = self.pool.ite(signaling, quiet, result);
        }
        result
    }

    /// AArch64's result of an operation the pool computes as x86-64 does, `sse`, of `operands`:
    /// the same where no NaN goes in or comes out; AArch64's NaN otherwise.
    fn arm_nans(&mut self, sse: Term, operands: &[Term]) -> Term {
        if self.pool.float_bounds(sse).is_some() {
            return sse;
        }
        let width = self.pool.width(sse);
        // With no NaN among the operands, x86-64's default NaN is the only NaN that comes out.
        let x86 = self.pool.bv(width, float::default_nan(width));
        let arm = self.pool.bv(width, default_nan(width));
        let meaningless = self.pool.eq(sse, x86);
        let numbers = self.pool.ite(meaningless, arm, sse);
        self.passed_on(operands, numbers)
    }

    /// `fmov`, `fabs`, `fneg`, `fsqrt`, `fcvt` to a float `to` bits wide, and the `frint`s.
    pub(super) fn fp_one(&mut self, op: FpOne, a: Term, to: u32) -> Term {
        let width = self.pool.width(a);
        match op {
            FpOne::Mov => a,
            // The sign bit alone, whatever the rest is, a NaN's too.
            FpOne::Abs => {
                let rest = self.pool.bv(width, sign_bit(width) - 1);
                self.pool.and(a, rest)
            }
            FpOne::Neg => self.negated(a),
            FpOne::Sqrt => {
                let root = self.pool.float_sqrt(a);
                self.arm_nans(root, &[a])
            }
            // A NaN keeps its sign and the top of its payload, quieted, on both CPUs.
            FpOne::Cvt => self.pool.float_convert(a, to),
            // A NaN comes through quieted, on both.
            FpOne::RintNearest => self.pool.float_round(a, Rounding::Nearest),
            FpOne::RintUp => self.pool.float_round(a, Rounding::Up),
            FpOne::RintDown => self.pool.float_round(a, Rounding::Down),
            FpOne::RintZero => self.pool.float_round(a, Rounding::Zero),
            FpOne::RintAway => self.pool.float_round(a, Rounding::Away),
        }
    }

    /// The operations of two floats.
    pub(super) fn fp_two(&mut self, op: FpTwo, a: Term, b: Term) -> Term {
        let binary = |cpu: &mut Self, op| {
            let sse = cpu.pool.float(op, a, b);
            cpu.arm_nans(sse, &[a, b])
        };
        match op {
            FpTwo::Add => binary(self, FloatOp::Add),
            FpTwo::Sub => binary(self, FloatOp::Sub),
            FpTwo::Mul => binary(self, FloatOp::Mul),
            FpTwo::Div => binary(self, FloatOp::Div),
            // The product negated, a NaN's sign too.
            FpTwo::Nmul => {
                let product = binary(self, FloatOp::Mul);
                self.negated(product)
            }
            FpTwo::Max => self.max_min(true, a, b),
            FpTwo::Min => self.max_min(false, a, b),
            FpTwo::MaxNm => self.max_min_number(true, a, b),
            FpTwo::MinNm => self.max_min_number(false, a, b),
        }
    }

    /// `fmax` or `fmin`: a NaN comes through; of two zeros, +0 is the larger.
    pub(super) fn max_min(&mut self, max: bool, a: Term, b: Term) -> Term {
        let below = self.fp_test(FloatTest::Less, a, b);
        let above = self.fp_test(FloatTest::Less, b, a);
        // Equal floats have the same bits but for zeros, whose signs this combines.
        let equal = if max {
            self.pool.and(a, b)
        } else {
            self.pool.or(a, b)
        };
        let numbers = if max {
            let inner = self.pool.ite(below, b, equal);
            self.pool.ite(above, a, inner)
        } else {
            let inner = self.pool.ite(above, b, equal);
            self.pool.ite(below, a, inner)
        };
        self.passed_on(&[a, b], numbers)
    }

    /// `fmaxnm` or `fminnm`: a quiet NaN beside anything but another quiet NaN loses, as an
    /// infinity on the losing side would, so the other operand comes out, quieted if it's a
    /// signaling NaN.
    pub(super) fn max_min_number(&mut self, max: bool, a: Term, b: Term) -> Term {
        let chosen = self.max_min(max, a, b);
        let (a_quiet, b_quiet) = (self.is_quiet_nan(a), self.is_quiet_nan(b));
        let (a_loud, b_loud) = (self.pool.bool_not(a_quiet), self.pool.bool_not(b_quiet));
        let b_wins = self.pool.bool_and(a_quiet, b_loud);
        let a_wins = self.pool.bool_and(b_quiet, a_loud);
        let as_comes = |cpu: &mut Self, value: Term| {
            let nan = cpu.is_nan(value);
            let quiet = cpu.quieted(value);
            cpu.pool.ite(nan, quiet, value)
        };
        let (a_out, b_out) = (as_comes(self, a), as_comes(self, b));
        let inner = self.pool.ite(a_wins, a_out, chosen);
        self.pool.ite(b_wins, b_out, inner)
    }

    /// `fmadd`, `fmsub`, `fnmadd` and `fnmsub` of `n` × `m` and `a`, rounded once: the addend
    /// and the first factor negated first as each says, a NaN's sign with them, and the addend
    /// first of the NaNs.
    pub(super) fn fp_fused(&mut self, op: FpThree, n: Term, m: Term, a: Term) -> Term {
        let (addend, first) = match op {
            FpThree::Madd => (a, n),
            FpThree::Msub => (a, self.negated(n)),
            FpThree::Nmadd => (self.negated(a), self.negated(n)),
            FpThree::Nmsub => (self.negated(a), n),
        };
        let sse = self.pool.float_fma(first, m, addend);
        let result = self.arm_nans(sse, &[addend, first, m]);
        if self.pool.float_bounds(sse).is_some() {
            return result;
        }
        // A quiet NaN added to infinity times zero gives the default NaN, not the NaN added.
        let width = self.pool.width(n);
        let quiet = self.is_quiet_nan(addend);
        let first_infinite = self.magnitude_is(first, infinity(width));
        let first_zero = self.magnitude_is(first, 0);
        let m_infinite = self.magnitude_is(m, infinity(width));
        let m_zero = self.magnitude_is(m, 0);
        let one_way = self.pool.bool_and(first_infinite, m_zero);
        let other_way = self.pool.bool_and(first_zero, m_infinite);
        let meaningless = self.pool.bool_or(one_way, other_way);
        let invalid = self.pool.bool_and(quiet, meaningless);
        let default = self.pool.bv(width, default_nan(width));
        self.pool.ite(invalid, default, result)
    }

    /// `fcmp` and `fcmpe`: NZCV 1000 for less, 0110 for equal, 0010 for greater and 0011 for
    /// unordered.
    fn fp_compare(&mut self, a: Term, b: Term) {
        let less = self.fp_test(FloatTest::Less, a, b);
        let equal = self.fp_test(FloatTest::Equal, a, b);
        let unordered = self.fp_test(FloatTest::Unordered, a, b);
        self.core.sign = less;
        self.core.zero = equal;
        self.core.carry = self.pool.bool_not(less);
        self.core.overflow = unordered;
        self.core.last = Last::Other;
    }

    /// The conversions between whole numbers and floats, and `fmov` of the bits between a
    /// general register and a floating-point one.
    fn fp_convert(&mut self, op: FpConversion, fp: FpReg, gpr: A64Reg) {
        let width = fp_width(fp);
        match op {
            FpConversion::Scvtf => {
                let number = self.x(gpr);
                let value = self.pool.float_from_int(number, width);
                self.set_v(fp, value);
            }
            FpConversion::Ucvtf => {
                let number = self.x(gpr);
                let value = self.unsigned_to_float(number, width);
                self.set_v(fp, value);
            }
            FpConversion::Fcvtzs => {
                let a = self.v(fp);
                let value = self.whole_signed(a, bits(gpr));
                self.set_x(gpr, value);
            }
            FpConversion::Fcvtzu => {
                let a = self.v(fp);
                let value = self.whole_unsigned(a, bits(gpr));
                self.set_x(gpr, value);
            }
            FpConversion::ToGeneral => {
                let a = self.v(fp);
                self.set_x(gpr, a);
            }
            FpConversion::FromGeneral => {
                let number = self.x(gpr);
                self.set_v(fp, number);
            }
        }
    }

    /// `ucvtf`: a whole number without a sign as the nearest float of `width` bits.
    pub(super) fn unsigned_to_float(&mut self, number: Term, width: u32) -> Term {
        let from = self.pool.width(number);
        if from < 64 {
            let wide = self.pool.zero_extend(number, 64 - from);
            return self.pool.float_from_int(wide, width);
        }
        let plain = self.pool.float_from_int(number, width);
        if self.pool.signed_range(number).0 >= 0 {
            return plain;
        }
        // From 2^63 up: half of it, its lowest bit kept so the half rounds as the whole would,
        // then doubled, which is exact.
        let one = self.pool.bv(64, 1);
        let half = self.pool.lshr(number, one);
        let lowest = self.pool.and(number, one);
        let kept = self.pool.or(half, lowest);
        let converted = self.pool.float_from_int(kept, width);
        let two = self.fp_constant(2.0, width);
        let doubled = self.pool.float(FloatOp::Mul, converted, two);
        let zero = self.pool.bv(64, 0);
        let high = self.pool.slt(number, zero);
        self.pool.ite(high, doubled, plain)
    }

    /// `fcvtzs`: `a` truncated to a signed whole number of `to` bits, saturating at either end,
    /// and 0 for a NaN. x86-64's conversion, the pool's, gives the most negative number for a NaN
    /// or a number out of range: AArch64's too for one too low.
    pub(super) fn whole_signed(&mut self, a: Term, to: u32) -> Term {
        let width = self.pool.width(a);
        let whole = self.pool.float_to_int(a, to, true);
        let limit = self.fp_constant(2_f64.powi(i32::try_from(to).unwrap_or(64) - 1), width);
        let fits = self.fp_test(FloatTest::Less, a, limit);
        let largest = self.pool.bv(to, ones(to) >> 1);
        let number = self.pool.ite(fits, whole, largest);
        let nan = self.is_nan(a);
        let zero = self.pool.bv(to, 0);
        self.pool.ite(nan, zero, number)
    }

    /// `fcvtzu`: `a` truncated to a whole number of `to` bits without a sign, saturating at
    /// either end, and 0 for a NaN.
    pub(super) fn whole_unsigned(&mut self, a: Term, to: u32) -> Term {
        let width = self.pool.width(a);
        let zero_float = self.pool.bv(width, 0);
        let negative = self.fp_test(FloatTest::Less, a, zero_float);
        let nan = self.is_nan(a);
        let none = self.pool.bool_or(nan, negative);
        let largest = self.pool.bv(to, ones(to));
        let whole = self.pool.float_to_int(a, 64, true);
        let number = if to == 64 {
            // Below 2^63 the signed conversion is the number; from there to 2^64 it's the
            // conversion of what's above 2^63, exact to subtract, with the top bit set.
            let half = self.fp_constant(2_f64.powi(63), width);
            let low = self.fp_test(FloatTest::Less, a, half);
            let all = self.fp_constant(2_f64.powi(64), width);
            let within = self.fp_test(FloatTest::Less, a, all);
            let above = self.pool.float(FloatOp::Sub, a, half);
            let above = self.pool.float_to_int(above, 64, true);
            let top = self.pool.bv(64, 1 << 63);
            let high = self.pool.or(above, top);
            let high = self.pool.ite(within, high, largest);
            self.pool.ite(low, whole, high)
        } else {
            let all = self.fp_constant(2_f64.powi(32), width);
            let within = self.fp_test(FloatTest::Less, a, all);
            let low = self.pool.extract(whole, 31, 0);
            self.pool.ite(within, low, largest)
        };
        let zero = self.pool.bv(to, 0);
        self.pool.ite(none, zero, number)
    }

    /// `ldr`, `str`, `ldp` and `stp` of s and d registers, the second of a pair just above the
    /// first.
    fn fp_load_store(
        &mut self,
        load: bool,
        registers: &[FpReg],
        address: Address,
    ) -> Result<Step, Stop> {
        let held = self.held_a64(address);
        let (mut at, moved) = self.address_of(address);
        for register in registers {
            let bytes = fp_width(*register) / 8;
            if load {
                let value = self.load_from(at, held, bytes)?;
                self.set_v(*register, value);
            } else {
                let value = self.v(*register);
                self.store_to(at, held, value, None)?;
            }
            let size = self.pool.bv(64, u128::from(bytes));
            at = self.pool.add(at, size);
        }
        if let Some((base, moved)) = moved {
            self.set_x(base, moved);
        }
        Ok(Step::Next)
    }
}
