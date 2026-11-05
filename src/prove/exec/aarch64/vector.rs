//! AArch64's NEON vectors in the prover's model: each instruction's lanes over the V registers'
//! 16 bytes, kept where x86-64's vector registers are kept. A write of any arrangement clears the
//! bytes above its lanes, as the CPU does, and a comparison gives each lane all ones or all
//! zeros, a choice between the two, so a program's tests of bytes are the conditions the
//! formula's are.

use std::rc::Rc;

use super::super::{Cpu, Step, Stop};
use crate::isa::aarch64::{
    A64Reg, Address, Arrangement, Expand, FpOne, FpThree, FpTwo, Inst, Lane, Scalar, Source,
    VAcross, VImmediate, VPermute, VReg, VShift, VThree, VTwo, Writeback,
};
use crate::prove::term::{FloatTest, Term};

/// The bytes of a V register.
const V_BYTES: usize = 16;

/// The lowest `width` bits set.
const fn ones(width: u32) -> u128 {
    if width >= 128 {
        u128::MAX
    } else {
        (1 << width) - 1
    }
}

/// A V register's lanes as `size` names them, all 128 bits of it.
const fn whole(number: u8, size: u32) -> VReg {
    VReg {
        number,
        arrangement: Arrangement::new(size, true),
    }
}

impl Cpu<'_> {
    /// Runs the vector instruction `inst`.
    pub(super) fn execute_vector(&mut self, inst: Inst) -> Result<Step, Stop> {
        match inst {
            Inst::VectorThree { op, rd, rn, rm } => {
                let lanes = self.v_three(op, rd, rn, rm)?;
                self.v_set_lanes(rd, &lanes);
            }
            Inst::VectorTwo { op, rd, rn } => {
                let lanes = self.v_two(op, rd, rn)?;
                self.v_set_lanes(rd, &lanes);
            }
            Inst::Pmull { rd, rn, rm } => {
                let bytes = self.v_pmull(rn, rm);
                self.set_v_bytes(rd, &bytes);
            }
            Inst::VectorAcross { op, rd, rn } => {
                let value = self.v_across(op, rd, rn)?;
                self.v_set_scalar(rd, value);
            }
            Inst::VectorShift { op, rd, rn, amount } => {
                let lanes = self.v_shift(op, rd, rn, u32::from(amount))?;
                self.v_set_lanes(rd, &lanes);
            }
            Inst::VectorPermute { op, rd, rn, rm } => {
                let lanes = self.v_permute(op, rn, rm);
                self.v_set_lanes(rd, &lanes);
            }
            Inst::VectorExtract { rd, rn, rm, index } => {
                let length = rd.arrangement.bytes() as usize;
                let mut joined = self.v_bytes(rn.number)[..length].to_vec();
                joined.extend_from_slice(&self.v_bytes(rm.number)[..length]);
                let start = usize::from(index);
                let bytes = joined[start..start + length].to_vec();
                self.set_v_bytes(rd.number, &bytes);
            }
            Inst::VectorTable {
                keep,
                rd,
                table,
                count,
                rm,
            } => {
                let bytes = self.v_table(keep, rd, table, count, rm);
                self.set_v_bytes(rd.number, &bytes);
            }
            Inst::VectorDup { rd, from } => {
                let value = self.v_source_lane(from, rd.arrangement.lane_bits());
                let lanes = vec![value; rd.arrangement.lanes() as usize];
                self.v_set_lanes(rd, &lanes);
            }
            Inst::VectorToGeneral { signed, rd, rn } => {
                let value = self.v_lane_of(rn);
                let width = if rd.wide { 64 } else { 32 };
                let by = width - self.pool.width(value);
                let value = match (by, signed) {
                    (0, _) => value,
                    (_, true) => self.pool.sign_extend(value, by),
                    (_, false) => self.pool.zero_extend(value, by),
                };
                self.set_x(rd, value);
            }
            Inst::VectorInsert { rd, from } => {
                let register = whole(rd.number, rd.size);
                let mut lanes = self.v_lanes(register);
                lanes[usize::from(rd.index)] = self.v_source_lane(from, 8 << rd.size);
                self.v_set_lanes(register, &lanes);
            }
            Inst::VectorImmediate {
                op,
                rd,
                imm8,
                expand,
            } => {
                let lanes = self.v_immediate(op, rd, imm8, expand);
                self.v_set_lanes(rd, &lanes);
            }
            Inst::VectorLoadStore { load, rt, address } => {
                return self.v_memory(load, &[rt], 16, address, None);
            }
            Inst::VectorPair {
                load,
                rt,
                rt2,
                address,
            } => return self.v_memory(load, &[rt, rt2], 16, address, None),
            Inst::VectorStructures {
                load,
                first,
                count,
                arrangement,
                base,
                step,
            } => {
                let registers: Vec<u8> = (0..count).map(|at| (first + at) % 32).collect();
                let address = Address::Offset { base, offset: 0 };
                return self.v_memory(
                    load,
                    &registers,
                    arrangement.bytes(),
                    address,
                    step.map(|step| (base, step)),
                );
            }
            _ => {
                return Err(super::super::unsupported(
                    "an instruction that isn't a vector one, as one",
                ));
            }
        }
        Ok(Step::Next)
    }

    /// V`number`'s 16 bytes, lowest first.
    /// `pmull` and `pmull2`: the carry-less product of each pair of lanes, bytes into halfwords
    /// or doublewords into all 128 bits, from the low half of each operand or, for `pmull2`,
    /// the high half. The 16 bytes of the result.
    fn v_pmull(&mut self, rn: VReg, rm: VReg) -> Vec<Term> {
        let (a, b) = (self.v_bytes(rn.number), self.v_bytes(rm.number));
        let half = if crate::isa::aarch64::pmull_upper(rn) {
            8
        } else {
            0
        };
        let mut out = Vec::with_capacity(V_BYTES);
        if crate::isa::aarch64::pmull_wide(rn) {
            let x = self.join_bytes(&a[half..half + 8]);
            let y = self.join_bytes(&b[half..half + 8]);
            let product = crate::prove::exec::carry_less(self.pool, x, y, 64);
            for at in 0..16 {
                out.push(self.pool.extract(product, 8 * at + 7, 8 * at));
            }
        } else {
            for lane in 0..8 {
                let product =
                    crate::prove::exec::carry_less(self.pool, a[half + lane], b[half + lane], 8);
                out.push(self.pool.extract(product, 7, 0));
                out.push(self.pool.extract(product, 15, 8));
            }
        }
        out
    }

    /// Bytes, the lowest first, as one number.
    fn join_bytes(&mut self, bytes: &[Term]) -> Term {
        let mut value = bytes[bytes.len() - 1];
        for byte in bytes[..bytes.len() - 1].iter().rev() {
            value = self.pool.concat(value, *byte);
        }
        value
    }

    pub(super) fn v_bytes(&self, number: u8) -> Vec<Term> {
        self.state.vectors[usize::from(number)][..V_BYTES].to_vec()
    }

    /// Sets V`number` to `bytes` from its lowest, zeros above them, up through the Z register
    /// it's the low part of, as a NEON write clears SVE's bytes above it.
    pub(super) fn set_v_bytes(&mut self, number: u8, bytes: &[Term]) {
        let zero = self.pool.bv(8, 0);
        let vector = Rc::make_mut(&mut self.state.vectors[usize::from(number)]);
        for (at, byte) in vector.iter_mut().enumerate() {
            *byte = bytes.get(at).copied().unwrap_or(zero);
        }
    }

    /// `register`'s lanes, lowest first.
    fn v_lanes(&mut self, register: VReg) -> Vec<Term> {
        let bytes = self.v_bytes(register.number);
        let width = (register.arrangement.lane_bits() / 8) as usize;
        (0..register.arrangement.lanes() as usize)
            .map(|lane| self.join(&bytes[lane * width..(lane + 1) * width]))
            .collect()
    }

    /// Sets `register` to `lanes`, the bytes above them cleared.
    fn v_set_lanes(&mut self, register: VReg, lanes: &[Term]) {
        let width = (register.arrangement.lane_bits() / 8) as usize;
        let mut bytes = Vec::with_capacity(V_BYTES);
        for lane in lanes {
            bytes.extend(self.split(*lane, width));
        }
        self.set_v_bytes(register.number, &bytes);
    }

    /// One lane's value.
    fn v_lane_of(&mut self, lane: Lane) -> Term {
        self.v_lanes(whole(lane.number, lane.size))[usize::from(lane.index)]
    }

    /// What a `dup` or an insert copies, `width` bits of it: a lane, or a general register's
    /// low bits.
    fn v_source_lane(&mut self, from: Source, width: u32) -> Term {
        match from {
            Source::Lane(lane) => self.v_lane_of(lane),
            Source::General(register) => {
                let value = self.x(register);
                if self.pool.width(value) > width {
                    self.pool.extract(value, width - 1, 0)
                } else {
                    value
                }
            }
        }
    }

    /// Sets a scalar across lanes wrote: its V register's low bytes, the rest cleared.
    fn v_set_scalar(&mut self, rd: Scalar, value: Term) {
        let bytes = self.split(value, (8 << rd.size) / 8);
        self.set_v_bytes(rd.number, &bytes);
    }

    /// All ones where `condition` holds, else zero, `width` bits wide.
    fn v_mask_where(&mut self, condition: Term, width: u32) -> Term {
        let all = self.pool.bv(width, ones(width));
        let none = self.pool.bv(width, 0);
        self.pool.ite(condition, all, none)
    }

    /// The operations of two vectors' lanes, or the stop for one this model has no meaning for.
    fn v_three(&mut self, op: VThree, rd: VReg, rn: VReg, rm: VReg) -> Result<Vec<Term>, Stop> {
        let (a, b) = (self.v_lanes(rn), self.v_lanes(rm));
        if op.float() {
            return self.v_float_three(op, rd, &a, &b);
        }
        if op.bitwise() {
            let d = self.v_lanes(rd);
            return (0..a.len())
                .map(|at| self.v_bitwise(op, d[at], a[at], b[at]))
                .collect();
        }
        let pairwise = match op {
            VThree::Addp => Some(VThree::Add),
            VThree::Umaxp => Some(VThree::Umax),
            VThree::Uminp => Some(VThree::Umin),
            VThree::Smaxp => Some(VThree::Smax),
            VThree::Sminp => Some(VThree::Smin),
            _ => None,
        };
        if let Some(each) = pairwise {
            // The first vector's neighbours give the low lanes, the second's the high.
            let pairs: Vec<(Term, Term)> = a
                .chunks(2)
                .chain(b.chunks(2))
                .map(|pair| (pair[0], pair[1]))
                .collect();
            return pairs
                .into_iter()
                .map(|(x, y)| self.v_lane_op(each, x, y))
                .collect();
        }
        a.iter()
            .zip(&b)
            .map(|(x, y)| self.v_lane_op(op, *x, *y))
            .collect()
    }

    /// One lane of `op` of `x` and `y`, or the stop for an op this model has no meaning for.
    fn v_lane_op(&mut self, op: VThree, x: Term, y: Term) -> Result<Term, Stop> {
        let width = self.pool.width(x);
        let zero = self.pool.bv(width, 0);
        let all = self.pool.bv(width, ones(width));
        let lowest = self.pool.bv(width, 1 << (width - 1));
        let highest = self.pool.bv(width, ones(width) >> 1);
        let pool = &mut *self.pool;
        Ok(match op {
            VThree::Add => pool.add(x, y),
            VThree::Sub => pool.sub(x, y),
            VThree::Mul => pool.mul(x, y),
            // Past the top, all ones.
            VThree::Uqadd => {
                let sum = pool.add(x, y);
                let carried = pool.ult(sum, x);
                pool.ite(carried, all, sum)
            }
            // Below zero, zero.
            VThree::Uqsub => {
                let below = pool.ult(x, y);
                let difference = pool.sub(x, y);
                pool.ite(below, zero, difference)
            }
            // Past either end, that end: the operands' sign says which.
            VThree::Sqadd | VThree::Sqsub => {
                let result = if op == VThree::Sqadd {
                    pool.add(x, y)
                } else {
                    pool.sub(x, y)
                };
                let overflow = if op == VThree::Sqadd {
                    let x_result = pool.xor(x, result);
                    let y_result = pool.xor(y, result);
                    pool.and(x_result, y_result)
                } else {
                    let x_y = pool.xor(x, y);
                    let x_result = pool.xor(x, result);
                    pool.and(x_y, x_result)
                };
                let overflowed = pool.slt(overflow, zero);
                let negative = pool.slt(x, zero);
                let end = pool.ite(negative, lowest, highest);
                pool.ite(overflowed, end, result)
            }
            VThree::Cmeq => {
                let equal = pool.eq(x, y);
                pool.ite(equal, all, zero)
            }
            VThree::Cmtst => {
                let both = pool.and(x, y);
                let none = pool.eq(both, zero);
                pool.ite(none, zero, all)
            }
            VThree::Cmhi => {
                let higher = pool.ult(y, x);
                pool.ite(higher, all, zero)
            }
            VThree::Cmhs => {
                let lower = pool.ult(x, y);
                pool.ite(lower, zero, all)
            }
            VThree::Cmgt => {
                let greater = pool.slt(y, x);
                pool.ite(greater, all, zero)
            }
            VThree::Cmge => {
                let less = pool.slt(x, y);
                pool.ite(less, zero, all)
            }
            VThree::Umax => {
                let below = pool.ult(x, y);
                pool.ite(below, y, x)
            }
            VThree::Umin => {
                let above = pool.ult(y, x);
                pool.ite(above, y, x)
            }
            VThree::Smax => {
                let below = pool.slt(x, y);
                pool.ite(below, y, x)
            }
            VThree::Smin => {
                let above = pool.slt(y, x);
                pool.ite(above, y, x)
            }
            // The pairwise, bitwise and floating-point ops are other functions', and never get
            // here; anything else is an op nobody has written a meaning for. It stops the proof
            // rather than run as something it isn't.
            _ => return Err(self.unmodelled(op)),
        })
    }

    /// One byte of a bitwise operation: `d` the destination's, `x` the first source's and `y`
    /// the second's.
    fn v_bitwise(&mut self, op: VThree, d: Term, x: Term, y: Term) -> Result<Term, Stop> {
        let pool = &mut *self.pool;
        Ok(match op {
            VThree::And => pool.and(x, y),
            VThree::Bic => {
                let not_y = pool.not(y);
                pool.and(x, not_y)
            }
            VThree::Orr => pool.or(x, y),
            VThree::Orn => {
                let not_y = pool.not(y);
                pool.or(x, not_y)
            }
            VThree::Eor => pool.xor(x, y),
            // The destination's bits choose the first source's where set, the second's where
            // clear.
            VThree::Bsl => {
                let first = pool.and(d, x);
                let not_d = pool.not(d);
                let second = pool.and(not_d, y);
                pool.or(first, second)
            }
            // The first source's bits where the second's are set, the destination's elsewhere.
            VThree::Bit => {
                let not_y = pool.not(y);
                let kept = pool.and(d, not_y);
                let taken = pool.and(x, y);
                pool.or(kept, taken)
            }
            // The first source's bits where the second's are clear.
            VThree::Bif => {
                let not_y = pool.not(y);
                let kept = pool.and(d, y);
                let taken = pool.and(x, not_y);
                pool.or(kept, taken)
            }
            _ => return Err(self.unmodelled(op)),
        })
    }

    /// The operations on one vector's lanes, or the stop for one this model has no meaning for.
    fn v_two(&mut self, op: VTwo, rd: VReg, rn: VReg) -> Result<Vec<Term>, Stop> {
        let a = self.v_lanes(rn);
        let width = rn.arrangement.lane_bits();
        let zero = self.pool.bv(width, 0);
        Ok(match op {
            VTwo::Cnt => a
                .iter()
                .map(|byte| {
                    let bits: Vec<Term> =
                        (0..8).map(|bit| self.pool.bit_is_set(*byte, bit)).collect();
                    self.pool.count(8, &bits)
                })
                .collect(),
            VTwo::Not => a.iter().map(|lane| self.pool.not(*lane)).collect(),
            VTwo::Rbit => a.iter().map(|byte| self.reversed(*byte, 1)).collect(),
            VTwo::Rev16 | VTwo::Rev32 | VTwo::Rev64 => {
                let container = match op {
                    VTwo::Rev16 => 16,
                    VTwo::Rev32 => 32,
                    _ => 64,
                };
                let per = (container / width) as usize;
                a.chunks(per)
                    .flat_map(|chunk| chunk.iter().rev().copied())
                    .collect()
            }
            VTwo::Cmeq0 | VTwo::Cmgt0 | VTwo::Cmge0 | VTwo::Cmle0 | VTwo::Cmlt0 => a
                .iter()
                .map(|lane| {
                    let holds = match op {
                        VTwo::Cmeq0 => self.pool.eq(*lane, zero),
                        VTwo::Cmgt0 => self.pool.slt(zero, *lane),
                        VTwo::Cmge0 => {
                            let negative = self.pool.slt(*lane, zero);
                            self.pool.bool_not(negative)
                        }
                        VTwo::Cmle0 => self.pool.sle(*lane, zero),
                        _ => self.pool.slt(*lane, zero),
                    };
                    self.v_mask_where(holds, width)
                })
                .collect(),
            VTwo::Abs => a
                .iter()
                .map(|lane| {
                    let negative = self.pool.slt(*lane, zero);
                    let negated = self.pool.neg(*lane);
                    self.pool.ite(negative, negated, *lane)
                })
                .collect(),
            VTwo::Neg => a.iter().map(|lane| self.pool.neg(*lane)).collect(),
            VTwo::Clz => a.iter().map(|lane| self.leading_zeros(*lane)).collect(),
            // The leading bits that match the sign: zeros of each bit xor the one below it.
            VTwo::Cls => a
                .iter()
                .map(|lane| {
                    let high = self.pool.extract(*lane, width - 1, 1);
                    let low = self.pool.extract(*lane, width - 2, 0);
                    let differ = self.pool.xor(high, low);
                    let count = self.leading_zeros(differ);
                    self.pool.zero_extend(count, 1)
                })
                .collect(),
            VTwo::Uaddlp | VTwo::Saddlp => {
                let signed = op == VTwo::Saddlp;
                a.chunks(2)
                    .map(|pair| {
                        let x = self.v_extended(pair[0], width, signed);
                        let y = self.v_extended(pair[1], width, signed);
                        if signed {
                            self.pool.add(x, y)
                        } else {
                            self.v_sum(2 * width, &[x, y])
                        }
                    })
                    .collect()
            }
            VTwo::Fabs
            | VTwo::Fneg
            | VTwo::Fsqrt
            | VTwo::Frintn
            | VTwo::Frinta
            | VTwo::Frintp
            | VTwo::Frintm
            | VTwo::Frintz
            | VTwo::Fcvtzs
            | VTwo::Fcvtzu
            | VTwo::Scvtf
            | VTwo::Ucvtf
            | VTwo::Fcmeq0
            | VTwo::Fcmgt0
            | VTwo::Fcmge0
            | VTwo::Fcmle0
            | VTwo::Fcmlt0
            | VTwo::Fcvtl
            | VTwo::Fcvtn => return self.v_float_two(op, rd, &a),
            VTwo::Xtn => {
                let narrow = width / 2;
                let narrowed: Vec<Term> = a
                    .iter()
                    .map(|lane| self.pool.extract(*lane, narrow - 1, 0))
                    .collect();
                if rd.arrangement.whole() {
                    // xtn2 keeps the low half.
                    let mut kept = self.v_lanes(rd);
                    kept.truncate(narrowed.len());
                    kept.extend(narrowed);
                    kept
                } else {
                    narrowed
                }
            }
        })
    }

    /// One float operation of `x` and `y`, a lane of each, as its scalar form computes it, or
    /// the stop for an op this model has no meaning for.
    fn v_float_lane(&mut self, op: VThree, x: Term, y: Term) -> Result<Term, Stop> {
        Ok(match op {
            VThree::Fadd | VThree::Faddp => self.fp_two(FpTwo::Add, x, y),
            VThree::Fsub => self.fp_two(FpTwo::Sub, x, y),
            VThree::Fmul => self.fp_two(FpTwo::Mul, x, y),
            VThree::Fdiv => self.fp_two(FpTwo::Div, x, y),
            VThree::Fmax | VThree::Fmaxp => self.max_min(true, x, y),
            VThree::Fmin | VThree::Fminp => self.max_min(false, x, y),
            VThree::Fmaxnm | VThree::Fmaxnmp => self.max_min_number(true, x, y),
            VThree::Fminnm | VThree::Fminnmp => self.max_min_number(false, x, y),
            // The difference's magnitude: its sign cleared, a NaN's too.
            VThree::Fabd => {
                let difference = self.fp_two(FpTwo::Sub, x, y);
                self.fp_one(FpOne::Abs, difference, 0)
            }
            VThree::Fcmeq | VThree::Fcmge | VThree::Fcmgt | VThree::Facge | VThree::Facgt => {
                let width = self.pool.width(x);
                let (x, y) = if matches!(op, VThree::Facge | VThree::Facgt) {
                    (self.fp_one(FpOne::Abs, x, 0), self.fp_one(FpOne::Abs, y, 0))
                } else {
                    (x, y)
                };
                // Every comparison is false where either is a NaN.
                let holds = match op {
                    VThree::Fcmeq => self.fp_test(FloatTest::Equal, x, y),
                    VThree::Fcmgt | VThree::Facgt => self.fp_test(FloatTest::Less, y, x),
                    // Greater or equal: `fcmge`, and `facge` of the magnitudes.
                    _ => {
                        let above = self.fp_test(FloatTest::Less, y, x);
                        let equal = self.fp_test(FloatTest::Equal, x, y);
                        self.pool.bool_or(above, equal)
                    }
                };
                self.v_mask_where(holds, width)
            }
            _ => return Err(self.unmodelled(op)),
        })
    }

    /// The operations of two vectors of floats, `a` and `b` their lanes: each lane with its
    /// own, or neighbouring lanes paired as `addp` pairs them, or each product added to or
    /// subtracted from the destination's lane, rounded once.
    fn v_float_three(
        &mut self,
        op: VThree,
        rd: VReg,
        a: &[Term],
        b: &[Term],
    ) -> Result<Vec<Term>, Stop> {
        match op {
            VThree::Faddp | VThree::Fmaxp | VThree::Fminp | VThree::Fmaxnmp | VThree::Fminnmp => {
                let pairs: Vec<(Term, Term)> = a
                    .chunks(2)
                    .chain(b.chunks(2))
                    .map(|pair| (pair[0], pair[1]))
                    .collect();
                pairs
                    .into_iter()
                    .map(|(x, y)| self.v_float_lane(op, x, y))
                    .collect()
            }
            VThree::Fmla | VThree::Fmls => {
                let d = self.v_lanes(rd);
                let fused = if op == VThree::Fmla {
                    FpThree::Madd
                } else {
                    FpThree::Msub
                };
                Ok((0..a.len())
                    .map(|at| self.fp_fused(fused, a[at], b[at], d[at]))
                    .collect())
            }
            _ => a
                .iter()
                .zip(b)
                .map(|(x, y)| self.v_float_lane(op, *x, *y))
                .collect(),
        }
    }

    /// The operations on one vector of floats, `a` its lanes, and the conversions between
    /// floats and whole numbers as wide, and between singles and doubles.
    fn v_float_two(&mut self, op: VTwo, rd: VReg, a: &[Term]) -> Result<Vec<Term>, Stop> {
        let one = |cpu: &mut Self, op: FpOne| -> Vec<Term> {
            a.iter().map(|lane| cpu.fp_one(op, *lane, 0)).collect()
        };
        Ok(match op {
            VTwo::Fabs => one(self, FpOne::Abs),
            VTwo::Fneg => one(self, FpOne::Neg),
            VTwo::Fsqrt => one(self, FpOne::Sqrt),
            VTwo::Frintn => one(self, FpOne::RintNearest),
            VTwo::Frinta => one(self, FpOne::RintAway),
            VTwo::Frintp => one(self, FpOne::RintUp),
            VTwo::Frintm => one(self, FpOne::RintDown),
            VTwo::Frintz => one(self, FpOne::RintZero),
            VTwo::Fcvtzs => a
                .iter()
                .map(|lane| {
                    let width = self.pool.width(*lane);
                    self.whole_signed(*lane, width)
                })
                .collect(),
            VTwo::Fcvtzu => a
                .iter()
                .map(|lane| {
                    let width = self.pool.width(*lane);
                    self.whole_unsigned(*lane, width)
                })
                .collect(),
            VTwo::Scvtf => a
                .iter()
                .map(|lane| {
                    let width = self.pool.width(*lane);
                    self.pool.float_from_int(*lane, width)
                })
                .collect(),
            VTwo::Ucvtf => a
                .iter()
                .map(|lane| {
                    let width = self.pool.width(*lane);
                    self.unsigned_to_float(*lane, width)
                })
                .collect(),
            // Two singles, the low ones or with fcvtl2 the high ones, as two doubles.
            VTwo::Fcvtl => a[a.len() - 2..]
                .iter()
                .map(|lane| self.fp_one(FpOne::Cvt, *lane, 64))
                .collect(),
            VTwo::Fcvtn => {
                let narrowed: Vec<Term> = a
                    .iter()
                    .map(|lane| self.fp_one(FpOne::Cvt, *lane, 32))
                    .collect();
                if rd.arrangement.whole() {
                    // fcvtn2 keeps the low half.
                    let mut kept = self.v_lanes(rd);
                    kept.truncate(narrowed.len());
                    kept.extend(narrowed);
                    kept
                } else {
                    narrowed
                }
            }
            // The comparisons with zero, either sign of it, false for a NaN.
            VTwo::Fcmeq0 | VTwo::Fcmgt0 | VTwo::Fcmge0 | VTwo::Fcmle0 | VTwo::Fcmlt0 => a
                .iter()
                .map(|lane| {
                    let width = self.pool.width(*lane);
                    let zero = self.pool.bv(width, 0);
                    let holds = match op {
                        VTwo::Fcmeq0 => self.fp_test(FloatTest::Equal, *lane, zero),
                        VTwo::Fcmgt0 => self.fp_test(FloatTest::Less, zero, *lane),
                        VTwo::Fcmlt0 => self.fp_test(FloatTest::Less, *lane, zero),
                        VTwo::Fcmge0 => {
                            let above = self.fp_test(FloatTest::Less, zero, *lane);
                            let equal = self.fp_test(FloatTest::Equal, *lane, zero);
                            self.pool.bool_or(above, equal)
                        }
                        // `fcmle`: less or equal.
                        _ => {
                            let below = self.fp_test(FloatTest::Less, *lane, zero);
                            let equal = self.fp_test(FloatTest::Equal, *lane, zero);
                            self.pool.bool_or(below, equal)
                        }
                    };
                    self.v_mask_where(holds, width)
                })
                .collect(),
            _ => return Err(self.unmodelled(op)),
        })
    }

    /// A vector's floats into one: four singles reduced in pairs, the low two's and the high
    /// two's results then together, as the architecture's `Reduce` does, or two floats.
    fn v_float_across(&mut self, op: VAcross, a: &[Term]) -> Result<Term, Stop> {
        let each = match op {
            VAcross::Fmaxv | VAcross::Fmaxp => VThree::Fmax,
            VAcross::Fminv | VAcross::Fminp => VThree::Fmin,
            VAcross::Fmaxnmv | VAcross::Fmaxnmp => VThree::Fmaxnm,
            VAcross::Fminnmv | VAcross::Fminnmp => VThree::Fminnm,
            VAcross::Faddp => VThree::Fadd,
            _ => return Err(self.unmodelled(op)),
        };
        let mut level = a.to_vec();
        while level.len() > 1 {
            level = level
                .chunks(2)
                .map(|pair| self.v_float_lane(each, pair[0], pair[1]))
                .collect::<Result<Vec<Term>, Stop>>()?;
        }
        Ok(level[0])
    }

    /// `value`, `by` bits wider, with copies of its sign or with zeros.
    fn v_extended(&mut self, value: Term, by: u32, signed: bool) -> Term {
        if signed {
            self.pool.sign_extend(value, by)
        } else {
            self.pool.zero_extend(value, by)
        }
    }

    /// The sum of `lanes`, `width` bits wide: where each is a 0 or a 1 on a condition, the count
    /// of those conditions, as x86-64's `popcnt` of a mask is, so a program's counted lanes and a
    /// formula's count of the same tests are one term.
    fn v_sum(&mut self, width: u32, lanes: &[Term]) -> Term {
        let mut parts = Vec::with_capacity(lanes.len());
        for lane in lanes {
            match self.pool.counted_parts(*lane) {
                Some(found) => parts.extend(found),
                None => return self.pool.sum(width, lanes),
            }
        }
        self.pool.count(width, &parts)
    }

    /// A vector's lanes into one scalar, or the stop for an op this model has no meaning for.
    fn v_across(&mut self, op: VAcross, rd: Scalar, rn: VReg) -> Result<Term, Stop> {
        let a = self.v_lanes(rn);
        let width = rn.arrangement.lane_bits();
        if op.float() {
            return self.v_float_across(op, &a);
        }
        Ok(match op {
            VAcross::Addv => self.v_sum(width, &a),
            VAcross::Uaddlv => {
                let by = (8 << rd.size) - width;
                let widened: Vec<Term> = a
                    .iter()
                    .map(|lane| self.v_extended(*lane, by, false))
                    .collect();
                self.v_sum(width + by, &widened)
            }
            VAcross::Saddlv => {
                let by = (8 << rd.size) - width;
                let signed = op == VAcross::Saddlv;
                let widened: Vec<Term> = a
                    .iter()
                    .map(|lane| self.v_extended(*lane, by, signed))
                    .collect();
                self.pool.sum(width + by, &widened)
            }
            VAcross::Umaxv | VAcross::Uminv | VAcross::Smaxv | VAcross::Sminv => {
                let mut best = a[0];
                for lane in &a[1..] {
                    let replaces = match op {
                        VAcross::Umaxv => self.pool.ult(best, *lane),
                        VAcross::Uminv => self.pool.ult(*lane, best),
                        VAcross::Smaxv => self.pool.slt(best, *lane),
                        // `sminv`.
                        _ => self.pool.slt(*lane, best),
                    };
                    best = self.pool.ite(replaces, *lane, best);
                }
                best
            }
            _ => return Err(self.unmodelled(op)),
        })
    }

    /// Each lane shifted by `amount`, with the widening and narrowing forms, or the stop for a
    /// shift this model has no meaning for.
    fn v_shift(&mut self, op: VShift, rd: VReg, rn: VReg, amount: u32) -> Result<Vec<Term>, Stop> {
        let a = self.v_lanes(rn);
        let width = rn.arrangement.lane_bits();
        Ok(match op {
            VShift::Shrn => {
                let narrow = width / 2;
                let by = self.pool.bv(width, u128::from(amount));
                let narrowed: Vec<Term> = a
                    .iter()
                    .map(|lane| {
                        let shifted = self.pool.lshr(*lane, by);
                        self.pool.extract(shifted, narrow - 1, 0)
                    })
                    .collect();
                if rd.arrangement.whole() {
                    let mut kept = self.v_lanes(rd);
                    kept.truncate(narrowed.len());
                    kept.extend(narrowed);
                    kept
                } else {
                    narrowed
                }
            }
            VShift::Ushll | VShift::Sshll => {
                // The high half's lanes with the `2` form, the low half's without.
                let half = if rn.arrangement.whole() {
                    a[a.len() / 2..].to_vec()
                } else {
                    a
                };
                let signed = op == VShift::Sshll;
                let by = self.pool.bv(2 * width, u128::from(amount));
                half.iter()
                    .map(|lane| {
                        let widened = self.v_extended(*lane, width, signed);
                        self.pool.shl(widened, by)
                    })
                    .collect()
            }
            _ => {
                let d = self.v_lanes(rd);
                (0..a.len())
                    .map(|at| self.v_lane_shift(op, d[at], a[at], width, amount))
                    .collect::<Result<Vec<Term>, Stop>>()?
            }
        })
    }

    /// One lane of a shift that keeps its width: `d` the destination's lane, `x` the source's.
    /// The stop for a shift this model has no meaning for.
    fn v_lane_shift(
        &mut self,
        op: VShift,
        d: Term,
        x: Term,
        width: u32,
        amount: u32,
    ) -> Result<Term, Stop> {
        let zero = self.pool.bv(width, 0);
        // A shift right by the whole width leaves nothing, or nothing but copies of the sign.
        let right = |cpu: &mut Self, signed: bool| {
            if signed {
                let by = cpu.pool.bv(width, u128::from(amount.min(width - 1)));
                cpu.pool.ashr(x, by)
            } else if amount >= width {
                zero
            } else {
                let by = cpu.pool.bv(width, u128::from(amount));
                cpu.pool.lshr(x, by)
            }
        };
        Ok(match op {
            VShift::Ushr => right(self, false),
            VShift::Sshr => right(self, true),
            VShift::Usra => {
                let shifted = right(self, false);
                self.pool.add(d, shifted)
            }
            VShift::Ssra => {
                let shifted = right(self, true);
                self.pool.add(d, shifted)
            }
            // The destination keeps its top `amount` bits.
            VShift::Sri => {
                if amount >= width {
                    return Ok(d);
                }
                let shifted = right(self, false);
                let keep = self.pool.bv(width, ones(width) & !(ones(width) >> amount));
                let kept = self.pool.and(d, keep);
                self.pool.or(kept, shifted)
            }
            VShift::Shl => {
                let by = self.pool.bv(width, u128::from(amount));
                self.pool.shl(x, by)
            }
            // The destination keeps its low `amount` bits.
            VShift::Sli => {
                let by = self.pool.bv(width, u128::from(amount));
                let shifted = self.pool.shl(x, by);
                let keep = self.pool.bv(width, ones(amount));
                let kept = self.pool.and(d, keep);
                self.pool.or(kept, shifted)
            }
            _ => return Err(self.unmodelled(op)),
        })
    }

    /// The interleavings of two vectors' lanes.
    fn v_permute(&mut self, op: VPermute, rn: VReg, rm: VReg) -> Vec<Term> {
        let (a, b) = (self.v_lanes(rn), self.v_lanes(rm));
        let n = a.len();
        let both: Vec<Term> = a.iter().chain(&b).copied().collect();
        match op {
            VPermute::Zip1 | VPermute::Zip2 => {
                let from = if op == VPermute::Zip1 { 0 } else { n / 2 };
                (0..n / 2)
                    .flat_map(|at| [a[from + at], b[from + at]])
                    .collect()
            }
            VPermute::Uzp1 => (0..n).map(|at| both[2 * at]).collect(),
            VPermute::Uzp2 => (0..n).map(|at| both[2 * at + 1]).collect(),
            VPermute::Trn1 => (0..n / 2).flat_map(|at| [a[2 * at], b[2 * at]]).collect(),
            VPermute::Trn2 => (0..n / 2)
                .flat_map(|at| [a[2 * at + 1], b[2 * at + 1]])
                .collect(),
        }
    }

    /// `tbl` and `tbx`: each index byte looks up a byte of the table, 16 bytes a register.
    fn v_table(&mut self, keep: bool, rd: VReg, table: u8, count: u8, rm: VReg) -> Vec<Term> {
        let mut bytes = Vec::with_capacity(16 * usize::from(count));
        for offset in 0..count {
            bytes.extend(self.v_bytes((table + offset) % 32));
        }
        let length = rd.arrangement.bytes() as usize;
        let indexes = self.v_bytes(rm.number);
        let kept = self.v_bytes(rd.number);
        (0..length)
            .map(|at| {
                let index = indexes[at];
                let outside = if keep { kept[at] } else { self.pool.bv(8, 0) };
                match self.pool.constant(index) {
                    Some(known) => bytes.get(known as usize).copied().unwrap_or(outside),
                    None => {
                        let mut value = outside;
                        for (place, byte) in bytes.iter().enumerate().rev() {
                            let here = self.pool.bv(8, place as u128);
                            let chosen = self.pool.eq(index, here);
                            value = self.pool.ite(chosen, *byte, value);
                        }
                        value
                    }
                }
            })
            .collect()
    }

    /// `movi`, `mvni`, and `orr` and `bic` of a number with each lane.
    fn v_immediate(&mut self, op: VImmediate, rd: VReg, imm8: u8, expand: Expand) -> Vec<Term> {
        let width = rd.arrangement.lane_bits();
        let value = u128::from(expand.value(imm8, width));
        let lanes = rd.arrangement.lanes() as usize;
        match op {
            VImmediate::Movi | VImmediate::Fmov => vec![self.pool.bv(width, value); lanes],
            VImmediate::Mvni => vec![self.pool.bv(width, !value & ones(width)); lanes],
            VImmediate::Orr | VImmediate::Bic => {
                let number = self.pool.bv(
                    width,
                    if op == VImmediate::Orr {
                        value
                    } else {
                        !value & ones(width)
                    },
                );
                let d = self.v_lanes(rd);
                d.iter()
                    .map(|lane| {
                        if op == VImmediate::Orr {
                            self.pool.or(*lane, number)
                        } else {
                            self.pool.and(*lane, number)
                        }
                    })
                    .collect()
            }
        }
    }

    /// The loads and stores of whole registers, `bytes` each, one after another from `address`,
    /// held to the object their base's pointer was made from; `step` moves the base after, as
    /// `ld1` and `st1` do.
    fn v_memory(
        &mut self,
        load: bool,
        registers: &[u8],
        bytes: u32,
        address: Address,
        step: Option<(A64Reg, Writeback)>,
    ) -> Result<Step, Stop> {
        let held = self.held_a64(address);
        let (mut at, moved) = self.address_of(address);
        let start = at;
        for register in registers {
            if load {
                let value = self.load_from(at, held, bytes)?;
                let parts = self.split(value, bytes as usize);
                self.set_v_bytes(*register, &parts);
            } else {
                let parts = self.v_bytes(*register);
                let value = self.join(&parts[..bytes as usize]);
                self.store_to(at, held, value, None)?;
            }
            let size = self.pool.bv(64, u128::from(bytes));
            at = self.pool.add(at, size);
        }
        if let Some((base, moved)) = moved {
            self.set_x(base, moved);
        }
        if let Some((base, step)) = step {
            let by = match step {
                Writeback::Bytes => {
                    let total = u128::from(bytes) * registers.len() as u128;
                    self.pool.bv(64, total)
                }
                Writeback::Register(by) => self.x(by),
            };
            let moved = self.pool.add(start, by);
            self.set_x(base, moved);
        }
        Ok(Step::Next)
    }
}
