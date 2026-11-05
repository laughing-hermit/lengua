//! AArch64's instructions in the prover's model: the dialect's subset, in the canonical form
//! lengua's own decoder reads (`crate::isa::aarch64::Inst`), each as what it does to the
//! registers, the flags and memory. What isn't the dialect's never gets here: the checker
//! refused it, and the decoder doesn't read it.
//!
//! The flags are NZCV. `cmp` (`subs`) keeps its operands, so a condition after it is a plain
//! comparison of values, as on x86-64; a logic operation that sets them keeps its result; and
//! anything else that sets them sets each as a term. AArch64's carry after a subtraction is "no
//! borrow", the opposite of x86-64's, and only AArch64's conditions read it.
//!
//! A call (`bl`) keeps its return address in x30, not on the stack, so the run remembers the call
//! without a slot; once the callee saves x30 in its frame record, that slot is the return
//! address no store may change, as x86-64's pushed one is. `ret` must go back where the call came
//! from, with the stack as the call found it.
//!
//! Floating point is [`float`]'s, and vectors are [`vector`]'s.

mod crypto;
mod float;
mod sve;
mod vector;

use super::{Cpu, End, Held, Last, Owner, Plan, Return, Step, Stop, Through, unsupported};
use crate::isa::aarch64::{
    A64Cond, A64Reg, Access, Address, Extend, Field, Gpr, Indirect, Inst, Logic, One, Select,
    Shift, Three, Two, Wide,
};
use crate::prove::term::{CASTAGNOLI, Term, ZLIB};

/// sp's slot.
const SP: usize = 31;

/// x30's slot: the link register, where `bl` leaves the return address.
const LINK: usize = 30;

/// A register's width in bits.
const fn bits(register: A64Reg) -> u32 {
    if register.wide { 64 } else { 32 }
}

/// The lowest `width` bits set.
const fn mask(width: u32) -> u128 {
    if width >= 128 {
        u128::MAX
    } else {
        (1 << width) - 1
    }
}

/// The slot a register reads from or writes to; xzr has none.
const fn slot(register: A64Reg) -> Option<usize> {
    match register.gpr {
        Gpr::R(number) => Some(number as usize),
        Gpr::Sp => Some(SP),
        Gpr::Zr => None,
    }
}

impl Cpu<'_> {
    /// Runs the AArch64 instruction `inst`.
    pub(super) fn execute_a64(&mut self, inst: Inst) -> Result<Step, Stop> {
        match inst {
            Inst::AddImm {
                sub,
                flags,
                rd,
                rn,
                imm,
                shift12,
            } => {
                let a = self.x(rn);
                let shift = if shift12 { 12 } else { 0 };
                let b = self.pool.bv(bits(rd), u128::from(imm) << shift);
                self.add_or_sub(sub, flags, rd, a, b);
                Ok(Step::Next)
            }
            Inst::AddShifted {
                sub,
                flags,
                rd,
                rn,
                rm,
                shift,
                amount,
            } => {
                let a = self.x(rn);
                let m = self.x(rm);
                let b = self.shifted(m, shift, amount);
                self.add_or_sub(sub, flags, rd, a, b);
                Ok(Step::Next)
            }
            Inst::LogicalImm { op, rd, rn, imm } => {
                let a = self.x(rn);
                let b = self.pool.bv(bits(rd), u128::from(imm) & mask(bits(rd)));
                self.logic(op, rd, a, b);
                Ok(Step::Next)
            }
            Inst::LogicalShifted {
                op,
                invert,
                rd,
                rn,
                rm,
                shift,
                amount,
            } => {
                let a = self.x(rn);
                let m = self.x(rm);
                let mut b = self.shifted(m, shift, amount);
                if invert {
                    b = self.pool.not(b);
                }
                self.logic(op, rd, a, b);
                Ok(Step::Next)
            }
            Inst::MoveWide { op, rd, imm, shift } => {
                let width = bits(rd);
                let part = u128::from(imm) << shift;
                let value = match op {
                    Wide::Movz => self.pool.bv(width, part),
                    Wide::Movn => self.pool.bv(width, !part & mask(width)),
                    Wide::Movk => {
                        let old = self.x(rd);
                        let keep = self.pool.bv(width, !(0xffff_u128 << shift) & mask(width));
                        let kept = self.pool.and(old, keep);
                        let new = self.pool.bv(width, part);
                        self.pool.or(kept, new)
                    }
                };
                self.set_x(rd, value);
                Ok(Step::Next)
            }
            Inst::Bitfield {
                op,
                rd,
                rn,
                immr,
                imms,
            } => {
                let value = self.bitfield(op, rd, rn, u32::from(immr), u32::from(imms));
                self.set_x(rd, value);
                Ok(Step::Next)
            }
            Inst::Extract { rd, rn, rm, lsb } => {
                let width = bits(rd);
                let high = self.x(rn);
                let low = self.x(rm);
                let both = self.pool.concat(high, low);
                let lsb = u32::from(lsb);
                let value = self.pool.extract(both, lsb + width - 1, lsb);
                self.set_x(rd, value);
                Ok(Step::Next)
            }
            Inst::Adrp { rd, page } => {
                let value = self.pool.bv(64, u128::from(page));
                self.set_x(rd, value);
                Ok(Step::Next)
            }
            Inst::Adr { .. } => Err(unsupported("adr, which the dialect doesn't have")),
            Inst::Branch { link, target } => {
                if link {
                    self.call_a64(target)?;
                } else {
                    self.state.rip = target;
                }
                Ok(Step::Next)
            }
            Inst::BranchCond { cond, target } => {
                let condition = self.a64_condition(cond);
                Ok(self.branch_if(condition, target))
            }
            Inst::CompareBranch {
                nonzero,
                rt,
                target,
            } => {
                let value = self.x(rt);
                let zero = self.pool.bv(bits(rt), 0);
                let is_zero = self.pool.eq(value, zero);
                let condition = if nonzero {
                    self.pool.bool_not(is_zero)
                } else {
                    is_zero
                };
                Ok(self.branch_if(condition, target))
            }
            Inst::TestBranch {
                nonzero,
                rt,
                bit,
                target,
            } => {
                let value = self.x(rt);
                let bit = u32::from(bit);
                // The top bit is the sign, and x86-64's `js` reads it as `result < 0`, which the
                // value's range settles where a bit's value would be a question for the solvers:
                // the check of every call's result (`tbnz x0, #63`) is of this kind.
                let set = if bit + 1 == bits(rt) {
                    let zero = self.pool.bv(bits(rt), 0);
                    self.pool.slt(value, zero)
                } else {
                    self.pool.bit_is_set(value, bit)
                };
                let condition = if nonzero {
                    set
                } else {
                    self.pool.bool_not(set)
                };
                Ok(self.branch_if(condition, target))
            }
            Inst::BranchReg { kind, rn } => match kind {
                Indirect::Ret => self.return_a64(rn),
                Indirect::Br => {
                    let target = self.x(rn);
                    self.jump_through_table(target)
                }
                Indirect::Blr => Err(unsupported("blr, which the dialect doesn't have")),
            },
            Inst::Svc { imm: 0 } => self.syscall(),
            Inst::Svc { .. } => Err(unsupported("svc other than #0")),
            Inst::CondSelect {
                op,
                rd,
                rn,
                rm,
                cond,
            } => {
                let condition = self.a64_condition(cond);
                let a = self.x(rn);
                let m = self.x(rm);
                let b = match op {
                    Select::Csel => m,
                    Select::Csinc => {
                        let one = self.pool.bv(bits(rd), 1);
                        self.pool.add(m, one)
                    }
                    Select::Csinv => self.pool.not(m),
                    Select::Csneg => self.pool.neg(m),
                };
                let value = self.pool.ite(condition, a, b);
                self.set_x(rd, value);
                Ok(Step::Next)
            }
            Inst::TwoSource { op, rd, rn, rm } => {
                let value = self.two_source(op, rd, rn, rm);
                self.set_x(rd, value);
                Ok(Step::Next)
            }
            // The state takes the data's bytes from the lowest, one CRC step each, as x86-64's
            // crc32 does.
            Inst::Crc {
                castagnoli,
                size,
                rd,
                rn,
                rm,
            } => {
                let poly = if castagnoli { CASTAGNOLI } else { ZLIB };
                let data = self.x(rm);
                let mut state = self.x(rn);
                for byte in 0..u32::from(size) {
                    let part = self.pool.extract(data, byte * 8 + 7, byte * 8);
                    state = self.pool.crc(state, part, poly);
                }
                self.set_x(rd, state);
                Ok(Step::Next)
            }
            Inst::OneSource { op, rd, rn } => {
                let value = self.one_source(op, rd, rn);
                self.set_x(rd, value);
                Ok(Step::Next)
            }
            Inst::ThreeSource { op, rd, rn, rm, ra } => {
                let value = self.three_source(op, rn, rm, ra);
                self.set_x(rd, value);
                Ok(Step::Next)
            }
            Inst::LoadStore {
                access,
                rt,
                address,
            } => self.load_store(access, rt, address),
            Inst::Pair {
                load,
                signed,
                rt,
                rt2,
                address,
            } => self.pair(load, signed, rt, rt2, address),
            Inst::Nop => Ok(Step::Next),
            Inst::FpOneSource { .. }
            | Inst::FpTwoSource { .. }
            | Inst::FpThreeSource { .. }
            | Inst::FpCompare { .. }
            | Inst::FpCondSelect { .. }
            | Inst::FpMoveImm { .. }
            | Inst::FpConvert { .. }
            | Inst::FpLoadStore { .. }
            | Inst::FpPair { .. } => self.execute_fp(inst),
            Inst::Sve(sve) => self.execute_sve(sve),
            Inst::Crypto { op, rd, rn, rm } => self.execute_crypto(op, rd, rn, rm),
            Inst::VectorThree { .. }
            | Inst::Pmull { .. }
            | Inst::VectorTwo { .. }
            | Inst::VectorAcross { .. }
            | Inst::VectorShift { .. }
            | Inst::VectorPermute { .. }
            | Inst::VectorExtract { .. }
            | Inst::VectorTable { .. }
            | Inst::VectorDup { .. }
            | Inst::VectorToGeneral { .. }
            | Inst::VectorInsert { .. }
            | Inst::VectorImmediate { .. }
            | Inst::VectorLoadStore { .. }
            | Inst::VectorPair { .. }
            | Inst::VectorStructures { .. } => self.execute_vector(inst),
        }
    }

    /// `register`'s value: all 64 bits of an `x` register, the low 32 of a `w` one; xzr is zero.
    fn x(&mut self, register: A64Reg) -> Term {
        let Some(slot) = slot(register) else {
            return self.pool.bv(bits(register), 0);
        };
        let whole = self.state.registers[slot];
        if register.wide {
            whole
        } else {
            self.pool.extract(whole, 31, 0)
        }
    }

    /// Sets `register` to `value`: a `w` register's value fills the low 32 bits and zeroes the
    /// rest; xzr takes nothing.
    fn set_x(&mut self, register: A64Reg, value: Term) {
        let Some(slot) = slot(register) else {
            return;
        };
        let value = if self.pool.width(value) < 64 {
            let by = 64 - self.pool.width(value);
            self.pool.zero_extend(value, by)
        } else {
            value
        };
        self.state.registers[slot] = value;
    }

    /// `value` widened by `by` bits, with copies of its sign or with zeros.
    fn widened(&mut self, value: Term, by: u32, signed: bool) -> Term {
        match (by, signed) {
            (0, _) => value,
            (_, true) => self.pool.sign_extend(value, by),
            (_, false) => self.pool.zero_extend(value, by),
        }
    }

    /// The low 32 bits of `value`, as a `w` register's.
    fn low32(&mut self, value: Term) -> Term {
        if self.pool.width(value) > 32 {
            self.pool.extract(value, 31, 0)
        } else {
            value
        }
    }

    /// An operand shifted by a constant amount.
    fn shifted(&mut self, value: Term, shift: Shift, amount: u8) -> Term {
        if amount == 0 {
            return value;
        }
        let width = self.pool.width(value);
        let by = self.pool.bv(width, u128::from(amount));
        match shift {
            Shift::Lsl => self.pool.shl(value, by),
            Shift::Lsr => self.pool.lshr(value, by),
            Shift::Asr => self.pool.ashr(value, by),
            Shift::Ror => self.rotated(value, u32::from(amount)),
        }
    }

    /// `value` rotated right by a constant amount.
    fn rotated(&mut self, value: Term, amount: u32) -> Term {
        let width = self.pool.width(value);
        let amount = amount % width;
        if amount == 0 {
            return value;
        }
        let low = self.pool.extract(value, amount - 1, 0);
        let high = self.pool.extract(value, width - 1, amount);
        self.pool.concat(low, high)
    }

    /// `add`, `sub`, `adds` and `subs` (so `cmp` and `cmn`), with their flags.
    fn add_or_sub(&mut self, sub: bool, flags: bool, rd: A64Reg, a: Term, b: Term) {
        let result = if sub {
            self.pool.sub(a, b)
        } else {
            self.pool.add(a, b)
        };
        if flags {
            let width = self.pool.width(result);
            let zero = self.pool.bv(width, 0);
            // Signed operands whose ranges keep the exact result in range never overflow, however
            // their signs fall: the sums and differences of parsed numbers, as x86-64's flags
            // are set. Without it the check of an overflow (`b.vs`) is a question for the solvers
            // on every path, and a number's print and a sum of amounts end with a path unsettled.
            let ((a_low, a_high), (b_low, b_high)) =
                (self.pool.signed_range(a), self.pool.signed_range(b));
            let (minimum, maximum) = (-(1_i128 << (width - 1)), (1_i128 << (width - 1)) - 1);
            let fits = if sub {
                a_low.checked_sub(b_high).is_some_and(|low| low >= minimum)
                    && a_high
                        .checked_sub(b_low)
                        .is_some_and(|high| high <= maximum)
            } else {
                a_low.checked_add(b_low).is_some_and(|low| low >= minimum)
                    && a_high
                        .checked_add(b_high)
                        .is_some_and(|high| high <= maximum)
            };
            let carry = if sub {
                // No borrow: a at least b, without sign.
                let borrow = self.pool.ult(a, b);
                self.pool.bool_not(borrow)
            } else {
                // A carry out of the top bit: the sum is below an addend.
                self.pool.ult(result, a)
            };
            let overflow = if fits {
                self.pool.boolean(false)
            } else if sub {
                // a and b of different signs, and the result's sign not a's.
                let a_b = self.pool.xor(a, b);
                let a_result = self.pool.xor(a, result);
                let both = self.pool.and(a_b, a_result);
                self.pool.slt(both, zero)
            } else {
                // Both addends' sign differs from the result's.
                let a_result = self.pool.xor(a, result);
                let b_result = self.pool.xor(b, result);
                let both = self.pool.and(a_result, b_result);
                self.pool.slt(both, zero)
            };
            self.state.sign = self.pool.slt(result, zero);
            self.state.zero = self.pool.eq(result, zero);
            self.state.carry = carry;
            self.state.overflow = overflow;
            self.state.last = if sub {
                Last::Compare(a, b)
            } else {
                Last::Other
            };
        }
        self.set_x(rd, result);
    }

    /// `and`, `orr`, `eor` and `ands` (so `tst`), the last with its flags.
    fn logic(&mut self, op: Logic, rd: A64Reg, a: Term, b: Term) {
        let result = match op {
            Logic::And | Logic::Ands => self.pool.and(a, b),
            Logic::Orr => self.pool.or(a, b),
            Logic::Eor => self.pool.xor(a, b),
        };
        if op == Logic::Ands {
            let width = self.pool.width(result);
            let zero = self.pool.bv(width, 0);
            self.state.sign = self.pool.slt(result, zero);
            self.state.zero = self.pool.eq(result, zero);
            self.state.carry = self.pool.boolean(false);
            self.state.overflow = self.pool.boolean(false);
            self.state.last = Last::Logic(result);
        }
        self.set_x(rd, result);
    }

    /// `ubfm`, `sbfm` and `bfm`, which `lsl`, `lsr`, `asr`, `ubfx`, `sbfx`, `bfi`, `bfxil` and
    /// the byte and word extensions are: bits `s` down to `r` of the source, or, where `s` is
    /// below `r`, bits `s` down to 0 moved up to `width - r`.
    fn bitfield(&mut self, op: Field, rd: A64Reg, rn: A64Reg, r: u32, s: u32) -> Term {
        let width = bits(rd);
        let source = self.x(rn);
        let source = if width == 32 {
            self.low32(source)
        } else {
            source
        };
        match op {
            Field::Ubfm | Field::Sbfm => {
                let signed = op == Field::Sbfm;
                if s >= r {
                    let field = self.pool.extract(source, s, r);
                    self.widened(field, width - (s - r + 1), signed)
                } else {
                    let field = self.pool.extract(source, s, 0);
                    let above = self.widened(field, r - s - 1, signed);
                    let below = self.pool.bv(width - r, 0);
                    self.pool.concat(above, below)
                }
            }
            Field::Bfm => {
                let old = self.x(rd);
                let (field, position, length) = if s >= r {
                    (self.pool.extract(source, s, r), 0, s - r + 1)
                } else {
                    (self.pool.extract(source, s, 0), width - r, s + 1)
                };
                let placed = self.widened(field, width - length, false);
                let by = self.pool.bv(width, u128::from(position));
                let placed = self.pool.shl(placed, by);
                let keep = self
                    .pool
                    .bv(width, mask(width) & !(mask(length) << position));
                let kept = self.pool.and(old, keep);
                self.pool.or(kept, placed)
            }
        }
    }

    /// `udiv`, `sdiv` and the shifts by a register.
    fn two_source(&mut self, op: Two, rd: A64Reg, rn: A64Reg, rm: A64Reg) -> Term {
        let width = bits(rd);
        let a = self.x(rn);
        let b = self.x(rm);
        let zero = self.pool.bv(width, 0);
        match op {
            // AArch64 divides by zero to zero, where SMT-LIB's division gives all ones.
            Two::Udiv | Two::Sdiv => {
                let quotient = if op == Two::Udiv {
                    self.pool.udiv(a, b)
                } else {
                    self.pool.sdiv(a, b)
                };
                let by_zero = self.pool.eq(b, zero);
                self.pool.ite(by_zero, zero, quotient)
            }
            // The amount is the register's value modulo the width.
            Two::Lslv | Two::Lsrv | Two::Asrv => {
                let modulo = self.pool.bv(width, u128::from(width - 1));
                let by = self.pool.and(b, modulo);
                match op {
                    Two::Lslv => self.pool.shl(a, by),
                    Two::Lsrv => self.pool.lshr(a, by),
                    _ => self.pool.ashr(a, by),
                }
            }
            Two::Rorv => match self.pool.constant(b) {
                Some(amount) => {
                    let amount = u32::try_from(amount % u128::from(width)).unwrap_or(0);
                    self.rotated(a, amount)
                }
                None => {
                    // a >> n | a << (width - n), n the amount modulo the width; for n of 0 the
                    // left shift is by the whole width, which leaves nothing.
                    let modulo = self.pool.bv(width, u128::from(width - 1));
                    let n = self.pool.and(b, modulo);
                    let right = self.pool.lshr(a, n);
                    let whole = self.pool.bv(width, u128::from(width));
                    let left_by = self.pool.sub(whole, n);
                    let left = self.pool.shl(a, left_by);
                    self.pool.or(right, left)
                }
            },
        }
    }

    /// `rbit`, `rev16`, `rev32`, `rev`, `clz` and `cls`.
    fn one_source(&mut self, op: One, rd: A64Reg, rn: A64Reg) -> Term {
        let width = bits(rd);
        let a = self.x(rn);
        match op {
            One::Rbit => self.reversed(a, 1),
            One::Rev16 => self.reversed_within(a, 16),
            One::Rev32 => self.reversed_within(a, 32),
            One::Rev => self.reversed(a, 8),
            One::Clz => self.leading_zeros(a),
            One::Cls => {
                // The leading bits that match the sign: zeros of each bit xor the next.
                let high = self.pool.extract(a, width - 1, 1);
                let low = self.pool.extract(a, width - 2, 0);
                let differ = self.pool.xor(high, low);
                let count = self.leading_zeros(differ);
                self.pool.zero_extend(count, 1)
            }
        }
    }

    /// `dividend - quotient × divisor`, where `quotient` is the dividend's own quotient by that
    /// constant divisor (`udiv`, or `sdiv` by a positive one): the dividend's remainder, however
    /// the pool wrote the quotient. A quotient by 10 of 5 times a sum is that sum halved, a form
    /// no sum less ten times it folds back to a remainder from, and the digits a print takes off
    /// with `udiv` and `msub` are then the remainders a formula states, as x86-64's `div` gives
    /// them.
    fn remainder_of(&mut self, dividend: Term, quotient: Term, divisor: Term) -> Option<Term> {
        let width = self.pool.width(dividend);
        let constant = self
            .pool
            .constant(divisor)
            .filter(|constant| *constant >= 2)?;
        if self.pool.udiv(dividend, divisor) == quotient {
            return Some(self.pool.urem(dividend, divisor));
        }
        if constant <= mask(width - 1) && self.pool.sdiv(dividend, divisor) == quotient {
            return Some(self.pool.srem(dividend, divisor));
        }
        None
    }

    /// The multiplies: `madd` and `msub` (so `mul`), the long ones from `w` registers, and the
    /// high halves of 128-bit products.
    fn three_source(&mut self, op: Three, rn: A64Reg, rm: A64Reg, ra: A64Reg) -> Term {
        match op {
            Three::Madd | Three::Msub => {
                let a = self.x(rn);
                let b = self.x(rm);
                let c = self.x(ra);
                if op == Three::Msub
                    && let Some(remainder) = self.remainder_of(c, a, b)
                {
                    return remainder;
                }
                let product = self.pool.mul(a, b);
                if op == Three::Madd {
                    self.pool.add(c, product)
                } else {
                    self.pool.sub(c, product)
                }
            }
            Three::Smaddl | Three::Smsubl | Three::Umaddl | Three::Umsubl => {
                let signed = matches!(op, Three::Smaddl | Three::Smsubl);
                let a = self.x(rn);
                let a = self.low32(a);
                let a = self.widened(a, 32, signed);
                let b = self.x(rm);
                let b = self.low32(b);
                let b = self.widened(b, 32, signed);
                let c = self.x(ra);
                let product = self.pool.mul(a, b);
                if matches!(op, Three::Smaddl | Three::Umaddl) {
                    self.pool.add(c, product)
                } else {
                    self.pool.sub(c, product)
                }
            }
            Three::Smulh | Three::Umulh => {
                let signed = op == Three::Smulh;
                let a = self.x(rn);
                let a = self.widened(a, 64, signed);
                let b = self.x(rm);
                let b = self.widened(b, 64, signed);
                let product = self.pool.mul(a, b);
                self.pool.extract(product, 127, 64)
            }
        }
    }

    /// `value` with its `unit`-bit pieces in the opposite order: its bits for 1, its bytes for 8.
    fn reversed(&mut self, value: Term, unit: u32) -> Term {
        let width = self.pool.width(value);
        let mut result: Option<Term> = None;
        // The lowest piece ends up highest: each later one goes below those before it.
        for piece in 0..width / unit {
            let part = self
                .pool
                .extract(value, piece * unit + unit - 1, piece * unit);
            result = Some(match result {
                None => part,
                Some(high) => self.pool.concat(high, part),
            });
        }
        result.unwrap_or(value)
    }

    /// `value` with the bytes of each `size`-bit part in the opposite order.
    fn reversed_within(&mut self, value: Term, size: u32) -> Term {
        let width = self.pool.width(value);
        let mut result: Option<Term> = None;
        for part in (0..width / size).rev() {
            let piece = self
                .pool
                .extract(value, part * size + size - 1, part * size);
            let reversed = self.reversed(piece, 8);
            result = Some(match result {
                None => reversed,
                Some(high) => self.pool.concat(high, reversed),
            });
        }
        result.unwrap_or(value)
    }

    /// How many zeros are above `value`'s highest set bit, as wide as `value`.
    fn leading_zeros(&mut self, value: Term) -> Term {
        let width = self.pool.width(value);
        if let Some(known) = self.pool.constant(value) {
            let length = 128 - known.leading_zeros();
            return self.pool.bv(width, u128::from(width - length.min(width)));
        }
        // From the lowest bit up, so a higher set bit decides.
        let mut count = self.pool.bv(width, u128::from(width));
        for bit in 0..width {
            let set = self.pool.bit_is_set(value, bit);
            let here = self.pool.bv(width, u128::from(width - 1 - bit));
            count = self.pool.ite(set, here, count);
        }
        count
    }

    /// Whether `cond` holds, as AArch64 reads the flags.
    fn a64_condition(&mut self, cond: A64Cond) -> Term {
        let pool = &mut *self.pool;
        if matches!(cond, A64Cond::Al | A64Cond::Nv) {
            return pool.boolean(true);
        }
        // After `cmp` a condition is a plain comparison of its operands, and after `tst` one of
        // its result, which value ranges can often settle without the solver.
        match self.state.last {
            Last::Compare(a, b) => {
                let found = match cond {
                    A64Cond::Eq => Some(pool.eq(a, b)),
                    A64Cond::Ne => {
                        let same = pool.eq(a, b);
                        Some(pool.bool_not(same))
                    }
                    A64Cond::Hs => {
                        let below = pool.ult(a, b);
                        Some(pool.bool_not(below))
                    }
                    A64Cond::Lo => Some(pool.ult(a, b)),
                    A64Cond::Hi => Some(pool.ult(b, a)),
                    A64Cond::Ls => Some(pool.ule(a, b)),
                    A64Cond::Ge => {
                        let less = pool.slt(a, b);
                        Some(pool.bool_not(less))
                    }
                    A64Cond::Lt => Some(pool.slt(a, b)),
                    A64Cond::Gt => Some(pool.slt(b, a)),
                    A64Cond::Le => Some(pool.sle(a, b)),
                    _ => None,
                };
                if let Some(found) = found {
                    return found;
                }
            }
            Last::Logic(result) => {
                let width = pool.width(result);
                let zero = pool.bv(width, 0);
                let is_zero = pool.eq(result, zero);
                let negative = pool.slt(result, zero);
                return match cond {
                    A64Cond::Eq => is_zero,
                    A64Cond::Ne => pool.bool_not(is_zero),
                    A64Cond::Mi | A64Cond::Lt => negative,
                    A64Cond::Pl | A64Cond::Ge => pool.bool_not(negative),
                    A64Cond::Gt => pool.slt(zero, result),
                    A64Cond::Le => pool.sle(result, zero),
                    // C and V are clear.
                    A64Cond::Hs | A64Cond::Hi | A64Cond::Vs => pool.boolean(false),
                    A64Cond::Lo | A64Cond::Ls | A64Cond::Vc | A64Cond::Al | A64Cond::Nv => {
                        pool.boolean(true)
                    }
                };
            }
            Last::Other => {}
        }
        let state = &*self.state;
        let (n, z, c, v) = (state.sign, state.zero, state.carry, state.overflow);
        let both = pool.bool_and(n, v);
        let not_n = pool.bool_not(n);
        let not_v = pool.bool_not(v);
        let neither = pool.bool_and(not_n, not_v);
        let n_is_v = pool.bool_or(both, neither);
        let not_z = pool.bool_not(z);
        match cond {
            A64Cond::Eq => z,
            A64Cond::Ne => not_z,
            A64Cond::Hs => c,
            A64Cond::Lo => pool.bool_not(c),
            A64Cond::Mi => n,
            A64Cond::Pl => not_n,
            A64Cond::Vs => v,
            A64Cond::Vc => not_v,
            A64Cond::Hi => pool.bool_and(c, not_z),
            A64Cond::Ls => {
                let not_c = pool.bool_not(c);
                pool.bool_or(not_c, z)
            }
            A64Cond::Ge => n_is_v,
            A64Cond::Lt => pool.bool_not(n_is_v),
            A64Cond::Gt => pool.bool_and(not_z, n_is_v),
            A64Cond::Le => {
                let differ = pool.bool_not(n_is_v);
                pool.bool_or(z, differ)
            }
            A64Cond::Al | A64Cond::Nv => pool.boolean(true),
        }
    }

    /// A branch to `target` where `condition` holds: taken, not taken, or both, each its own run.
    fn branch_if(&mut self, condition: Term, target: u64) -> Step {
        match self.pool.truth(condition) {
            Some(true) => {
                self.state.rip = target;
                Step::Next
            }
            Some(false) => Step::Next,
            None => {
                let other = self.state.clone();
                self.state.rip = target;
                Step::Split(condition, Box::new(other))
            }
        }
    }

    /// `bl target`: the return address in x30, and a call that hasn't saved it anywhere yet.
    fn call_a64(&mut self, target: u64) -> Result<(), Stop> {
        let back = self.next;
        self.state.registers[LINK] = self.pool.bv(64, u128::from(back));
        let top = self.constant(self.state.registers[SP])?;
        self.image.enter(target);
        self.state.calls += 1;
        self.state.returns.push(Return {
            top,
            pushed: 0,
            saved: None,
            back,
            callee: target,
            frame: self.state.calls,
        });
        self.state.rip = target;
        Ok(())
    }

    /// `ret`: back where the call came from, with the stack as the call found it.
    fn return_a64(&mut self, rn: A64Reg) -> Result<Step, Stop> {
        let target = self.x(rn);
        let target = self.constant(target)?;
        let sp = self.constant(self.state.registers[SP])?;
        match self.state.returns.pop() {
            Some(back) if back.back == target && back.top == sp => {}
            _ => {
                return Err(End::Unsafe(format!(
                    "{} returns somewhere its caller didn't call it from",
                    self.doing()
                )));
            }
        }
        self.state.rip = target;
        Ok(Step::Next)
    }

    /// The address an access reaches, as a term, and the base's new value when the access moves
    /// it.
    fn address_of(&mut self, address: Address) -> (Term, Option<(A64Reg, Term)>) {
        match address {
            Address::Offset { base, offset } => {
                let base_value = self.x(base);
                let offset = self.pool.bv_signed(64, i128::from(offset));
                (self.pool.add(base_value, offset), None)
            }
            Address::PreIndex { base, offset } => {
                let base_value = self.x(base);
                let offset = self.pool.bv_signed(64, i128::from(offset));
                let at = self.pool.add(base_value, offset);
                (at, Some((base, at)))
            }
            Address::PostIndex { base, offset } => {
                let base_value = self.x(base);
                let offset = self.pool.bv_signed(64, i128::from(offset));
                let moved = self.pool.add(base_value, offset);
                (base_value, Some((base, moved)))
            }
            Address::Register {
                base,
                index,
                extend,
                shift,
            } => {
                let base_value = self.x(base);
                let index_value = self.x(index);
                let index_value = match extend {
                    Extend::Lsl | Extend::Sxtx => index_value,
                    Extend::Uxtw | Extend::Sxtw => {
                        let low = self.low32(index_value);
                        self.widened(low, 32, extend == Extend::Sxtw)
                    }
                };
                let by = self.pool.bv(64, u128::from(shift));
                let scaled = self.pool.shl(index_value, by);
                (self.pool.add(base_value, scaled), None)
            }
        }
    }

    /// What holds the pointer an access through `address` goes through: its base register's
    /// owner (the running call's frame, for sp) or the object the base points into. An access
    /// at a constant offset from sp is the checker's to hold to the frame, as x86-64's is.
    fn held_a64(&self, address: Address) -> Option<Held> {
        let (base, indexed) = match address {
            Address::Offset { base, .. }
            | Address::PreIndex { base, .. }
            | Address::PostIndex { base, .. } => (base, false),
            Address::Register { base, .. } => (base, true),
        };
        let slot = slot(base)?;
        if slot == SP && !indexed {
            return None;
        }
        if let Some(owner) = self.state.owner_of_slot(slot, SP) {
            return Some(Held::Owner(owner));
        }
        let pointer = self
            .pool
            .constant(self.state.registers[slot])
            .and_then(|value| u64::try_from(value).ok())?;
        self.object(pointer, pointer)
            .is_some()
            .then_some(Held::Pointer(pointer))
    }

    /// Loads `bytes` bytes at `at`, held to what `held` holds.
    fn load_from(&mut self, at: Term, held: Option<Held>, bytes: u32) -> Result<Term, Stop> {
        match self
            .pool
            .constant(at)
            .and_then(|value| u64::try_from(value).ok())
        {
            Some(address) => {
                if let Some(held) = held {
                    self.reach(held, address, u64::from(bytes), false, &Self::doing)?;
                }
                self.load(address, bytes)
            }
            None => self.load_anywhere(at, bytes, Through::Held(held)),
        }
    }

    /// Stores `value` at `at`, held to what `held` holds, from the general register `from` if
    /// it's one; when it's x30 holding the return address of the running call, that's where the
    /// return address is kept from then on.
    fn store_to(
        &mut self,
        at: Term,
        held: Option<Held>,
        value: Term,
        from: Option<A64Reg>,
    ) -> Result<(), Stop> {
        let Some(address) = self
            .pool
            .constant(at)
            .and_then(|value| u64::try_from(value).ok())
        else {
            return self.save_anywhere(at, value, Through::Held(held));
        };
        if let Some(held) = held {
            let bytes = u64::from(self.pool.width(value) / 8);
            self.reach(held, address, bytes, true, &Self::doing)?;
        }
        self.save(address, value)?;
        if let Some(from) = from
            && slot(from) == Some(LINK)
            && from.wide
            && let Some(known) = self.pool.constant(value)
            && let Some(back) = self.state.returns.last_mut()
            && back.saved.is_none()
            && known == u128::from(back.back)
        {
            back.saved = Some(address);
        }
        Ok(())
    }

    /// `ldr`, `str` and their byte, half and signed forms.
    fn load_store(&mut self, access: Access, rt: A64Reg, address: Address) -> Result<Step, Stop> {
        let bytes = u32::from(access.size);
        let held = self.held_a64(address);
        let (at, moved) = self.address_of(address);
        if access.load {
            let value = self.load_from(at, held, bytes)?;
            let value = self.widened(value, bits(rt) - bytes * 8, access.signed);
            self.set_x(rt, value);
        } else {
            let value = self.x(rt);
            let value = if bits(rt) > bytes * 8 {
                self.pool.extract(value, bytes * 8 - 1, 0)
            } else {
                value
            };
            self.store_to(at, held, value, Some(rt))?;
        }
        if let Some((base, moved)) = moved {
            self.set_x(base, moved);
        }
        Ok(Step::Next)
    }

    /// `ldp`, `stp` and `ldpsw`: two registers at once, the second just above the first.
    fn pair(
        &mut self,
        load: bool,
        signed: bool,
        rt: A64Reg,
        rt2: A64Reg,
        address: Address,
    ) -> Result<Step, Stop> {
        let bytes = if signed || !rt.wide { 4 } else { 8 };
        let held = self.held_a64(address);
        let (at, moved) = self.address_of(address);
        let size = self.pool.bv(64, u128::from(bytes));
        let second = self.pool.add(at, size);
        if load {
            let first_value = self.load_from(at, held, bytes)?;
            let second_value = self.load_from(second, held, bytes)?;
            let by = bits(rt) - bytes * 8;
            let first_value = self.widened(first_value, by, signed);
            let second_value = self.widened(second_value, by, signed);
            self.set_x(rt, first_value);
            self.set_x(rt2, second_value);
        } else {
            let first_value = self.x(rt);
            let second_value = self.x(rt2);
            self.store_to(at, held, first_value, Some(rt))?;
            self.store_to(second, held, second_value, Some(rt2))?;
        }
        if let Some((base, moved)) = moved {
            self.set_x(base, moved);
        }
        Ok(Step::Next)
    }

    /// How `inst` passes owners on, from the state before it: a pointer plus a number points
    /// into the same object, `adrp` and `add :lo12:` make a symbol's address, a moved register
    /// keeps its owner, and a pointer stored whole and loaded back is the same pointer.
    pub(super) fn plan_a64(&self, inst: Inst) -> Plan {
        let mut plan = Plan::default();
        let owner = |cpu: &Self, register: A64Reg| {
            let slot = slot(register)?;
            if !register.wide {
                return None;
            }
            cpu.state.owner_of_slot(slot, SP)
        };
        let set = |plan: &mut Plan, register: A64Reg, owner: Option<Owner>| {
            if let Some(slot) = slot(register)
                && register.wide
                && slot != SP
                && let Some(free) = plan.registers.iter_mut().find(|set| set.is_none())
            {
                *free = Some((slot, owner));
            }
        };
        let value = |cpu: &Self, register: A64Reg| {
            slot(register).and_then(|slot| {
                cpu.pool
                    .constant(cpu.state.registers[slot])
                    .and_then(|value| u64::try_from(value).ok())
            })
        };
        match inst {
            Inst::AddImm {
                sub,
                flags: false,
                rd,
                rn,
                imm,
                shift12,
            } => {
                let named = value(self, rn).and_then(|base| {
                    let offset = u64::from(imm) << if shift12 { 12 } else { 0 };
                    let address = if sub {
                        base.wrapping_sub(offset)
                    } else {
                        base.wrapping_add(offset)
                    };
                    self.image
                        .named
                        .contains_key(&self.at)
                        .then(|| self.image.symbol_owner(self.at, address))
                        .flatten()
                });
                set(&mut plan, rd, named.or_else(|| owner(self, rn)));
            }
            Inst::AddShifted {
                sub,
                flags: false,
                rd,
                rn,
                rm,
                amount: 0,
                ..
            } => {
                let picked = match (owner(self, rn), owner(self, rm)) {
                    (Some(owner), None) => Some(owner),
                    (None, Some(owner)) if !sub => Some(owner),
                    _ => None,
                };
                set(&mut plan, rd, picked);
            }
            // `mov xD, xM` is `orr xD, xzr, xM`.
            Inst::LogicalShifted {
                op: Logic::Orr,
                invert: false,
                rd,
                rn,
                rm,
                amount: 0,
                ..
            } if rn.gpr == Gpr::Zr => set(&mut plan, rd, owner(self, rm)),
            Inst::LoadStore {
                access,
                rt,
                address,
            } if access.size == 8 && rt.wide => {
                let at = self.planned_address(address);
                if access.load {
                    set(
                        &mut plan,
                        rt,
                        at.and_then(|at| self.state.pointers.get(&at).copied()),
                    );
                } else if let (Some(at), Some(owner), Some(slot)) = (at, owner(self, rt), slot(rt))
                {
                    plan.stores[0] = Some((at, self.state.registers[slot], owner));
                }
                if let Some(base) = moved_base(address) {
                    set(&mut plan, base, owner(self, base));
                }
            }
            Inst::Pair {
                load,
                signed: false,
                rt,
                rt2,
                address,
            } if rt.wide => {
                if let Some(at) = self.planned_address(address) {
                    for (number, register) in [rt, rt2].into_iter().enumerate() {
                        let place = at + 8 * number as u64;
                        if load {
                            set(
                                &mut plan,
                                register,
                                self.state.pointers.get(&place).copied(),
                            );
                        } else if let (Some(owner), Some(slot)) =
                            (owner(self, register), slot(register))
                        {
                            plan.stores[number] = Some((place, self.state.registers[slot], owner));
                        }
                    }
                }
                if let Some(base) = moved_base(address) {
                    set(&mut plan, base, owner(self, base));
                }
            }
            // A base an access moves keeps its owner, whatever the access moves: a floating-point
            // register owns no address, so its loads and stores pass none on.
            Inst::LoadStore { address, .. }
            | Inst::Pair { address, .. }
            | Inst::FpLoadStore { address, .. }
            | Inst::FpPair { address, .. }
            | Inst::VectorLoadStore { address, .. }
            | Inst::VectorPair { address, .. } => {
                if let Some(base) = moved_base(address) {
                    set(&mut plan, base, owner(self, base));
                }
            }
            Inst::VectorStructures {
                base,
                step: Some(_),
                ..
            } => set(&mut plan, base, owner(self, base)),
            _ => {}
        }
        plan
    }

    /// The address an access reaches, before it runs, when it's a constant.
    fn planned_address(&self, address: Address) -> Option<u64> {
        let value = |register: A64Reg| {
            slot(register).and_then(|slot| {
                self.pool
                    .constant(self.state.registers[slot])
                    .and_then(|value| u64::try_from(value).ok())
            })
        };
        match address {
            Address::Offset { base, offset } | Address::PreIndex { base, offset } => {
                value(base).map(|base| base.wrapping_add_signed(offset))
            }
            Address::PostIndex { base, .. } => value(base),
            Address::Register { .. } => None,
        }
    }
}

/// The base register an access moves, if it moves one.
const fn moved_base(address: Address) -> Option<A64Reg> {
    match address {
        Address::PreIndex { base, .. } | Address::PostIndex { base, .. } => Some(base),
        Address::Offset { .. } | Address::Register { .. } => None,
    }
}
