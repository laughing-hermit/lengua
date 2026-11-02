//! The vector instructions as main.s and LLVM's disassembler write them, read into the canonical
//! form the decoder gives their words: the aliases LLVM prints (`mov` for `orr` of a register
//! with itself, `umov` and `ins`, `mvn` for `not`, `uxtl` for `ushll #0`) and the ones it accepts
//! (`cmle` of two registers for `cmge` the other way round) included.

use super::super::{
    Address, Arrangement, Expand, Inst, Lane, Scalar, Source, VAcross, VImmediate, VPermute, VReg,
    VShift, VThree, VTwo, Writeback,
};
use super::{Gpr, Line, register};

/// A V register's number from its name, `v0` to `v31`.
fn number(name: &str) -> Option<u8> {
    let digits = name.strip_prefix('v')?;
    if digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || (digits != "0" && digits.starts_with('0'))
    {
        return None;
    }
    digits.parse().ok().filter(|number| *number <= 31)
}

/// A V register with its arrangement: `v3.16b`.
#[must_use]
pub fn vreg(text: &str) -> Option<VReg> {
    let (name, arrangement) = text.split_once('.')?;
    Some(VReg {
        number: number(name)?,
        arrangement: Arrangement::named(arrangement)?,
    })
}

/// A lane size's number from its letter.
fn size_of(letter: &str) -> Option<u32> {
    Some(match letter {
        "b" => 0,
        "h" => 1,
        "s" => 2,
        "d" => 3,
        _ => return None,
    })
}

/// One lane of a V register: `v1.b[3]`.
#[must_use]
pub fn lane(text: &str) -> Option<Lane> {
    let (name, rest) = text.split_once('.')?;
    let (letter, index) = rest.strip_suffix(']')?.split_once('[')?;
    let size = size_of(letter)?;
    let index: u8 = index.trim().parse().ok()?;
    (u32::from(index) < 16 >> size).then_some(Lane {
        number: number(name)?,
        size,
        index,
    })
}

/// A V register's low lane as a scalar: `b0`, `h0`, `s0` or `d0`.
#[must_use]
pub fn scalar(text: &str) -> Option<Scalar> {
    let (letter, digits) = text.split_at_checked(1)?;
    let number = number(&format!("v{digits}"))?;
    Some(Scalar {
        number,
        size: size_of(letter)?,
    })
}

/// A whole V register as a load or store names it: `q0` to `q31`.
#[must_use]
pub fn q_register(text: &str) -> Option<u8> {
    number(&format!("v{}", text.strip_prefix('q')?))
}

/// A list of registers, `{ v0.16b, v1.16b }` or `{ v0.16b-v1.16b }`, as its first register, how
/// many there are, and their one arrangement: each register the one after the last, wrapping
/// from v31 to v0.
fn list(text: &str) -> Option<(u8, u8, Arrangement)> {
    let inside = text.strip_prefix('{')?.strip_suffix('}')?.trim();
    let registers: Vec<VReg> = if let Some((first, last)) = inside.split_once('-') {
        let (first, last) = (vreg(first.trim())?, vreg(last.trim())?);
        if last.arrangement != first.arrangement {
            return None;
        }
        let count = (last.number + 32 - first.number) % 32 + 1;
        (0..count)
            .map(|offset| VReg {
                number: (first.number + offset) % 32,
                arrangement: first.arrangement,
            })
            .collect()
    } else {
        inside
            .split(',')
            .map(|name| vreg(name.trim()))
            .collect::<Option<_>>()?
    };
    let first = *registers.first()?;
    let follows = registers.iter().enumerate().all(|(offset, register)| {
        register.arrangement == first.arrangement
            && usize::from(register.number) == (usize::from(first.number) + offset) % 32
    });
    (follows && (1..=4).contains(&registers.len())).then_some((
        first.number,
        registers.len() as u8,
        first.arrangement,
    ))
}

/// Whether doublewords may come `arrangement`'s way: two at a time, never one.
fn not_one_doubleword(arrangement: Arrangement) -> bool {
    arrangement != Arrangement::D1
}

/// Whether `arrangement` is one of floats: two or four singles, or two doubles.
fn float_lanes(arrangement: Arrangement) -> bool {
    matches!(
        arrangement,
        Arrangement::S2 | Arrangement::S4 | Arrangement::D2
    )
}

impl Line<'_> {
    /// Operand `at`, as a vector register.
    fn v(&self, at: usize) -> Result<VReg, String> {
        self.operands
            .get(at)
            .and_then(|operand| vreg(operand))
            .ok_or_else(|| {
                self.problem(&format!(
                    "operand {} isn't a vector register such as v0.16b",
                    at + 1
                ))
            })
    }

    /// Operand `at`, as a lane of a vector register.
    fn lane(&self, at: usize) -> Result<Lane, String> {
        self.operands
            .get(at)
            .and_then(|operand| lane(operand))
            .ok_or_else(|| {
                self.problem(&format!("operand {} isn't a lane such as v1.b[3]", at + 1))
            })
    }

    /// The line as a vector instruction, or none where it isn't one: a mnemonic only vectors
    /// have, or an operand that's a vector, a lane, a list or a q register.
    pub(super) fn vector(&self) -> Option<Result<Inst, String>> {
        let any_vector = self.operands.iter().any(|operand| {
            vreg(operand).is_some() || lane(operand).is_some() || operand.starts_with('{')
        });
        let first_q = self
            .operands
            .first()
            .is_some_and(|operand| q_register(operand).is_some());
        let number_second = self
            .operands
            .get(1)
            .is_some_and(|operand| operand.starts_with('#'));
        let zero_last = self.operands.len() == 3 && self.operands.last() == Some(&"#0");
        let float_zero_last =
            self.operands.len() == 3 && matches!(self.operands.last(), Some(&("#0.0" | "#0")));
        let two_operands = self.operands.len() == 2;
        Some(match self.mnemonic {
            "fmov" if any_vector && number_second => self.float_immediate(),
            "fcmeq" | "fcmgt" | "fcmge" | "fcmle" | "fcmlt" if any_vector && float_zero_last => {
                self.float_compare_zero()
            }
            "fabs" | "fneg" | "fsqrt" | "frintn" | "frinta" | "frintp" | "frintm" | "frintz"
            | "fcvtzs" | "fcvtzu" | "scvtf" | "ucvtf" | "fcvtl" | "fcvtl2" | "fcvtn" | "fcvtn2"
                if any_vector =>
            {
                self.float_two()
            }
            "fmaxv" | "fminv" | "fmaxnmv" | "fminnmv" => self.float_across(),
            "faddp" | "fmaxp" | "fminp" | "fmaxnmp" | "fminnmp" if any_vector && two_operands => {
                self.float_across()
            }
            "fadd" | "fsub" | "fmul" | "fdiv" | "fmax" | "fmin" | "fmaxnm" | "fminnm" | "fabd"
            | "faddp" | "fmaxp" | "fminp" | "fmaxnmp" | "fminnmp" | "fmla" | "fmls" | "fcmeq"
            | "fcmge" | "fcmgt" | "fcmle" | "fcmlt" | "facge" | "facgt" | "facle" | "faclt"
                if any_vector =>
            {
                self.float_three()
            }
            "ldr" | "str" | "ldur" | "stur" if first_q => self.q_load_store(),
            "ldp" | "stp" if first_q => self.q_pair(),
            "ld1" | "st1" => self.structures(),
            "movi" | "mvni" => self.vector_immediate(),
            "orr" | "bic" if any_vector && number_second => self.vector_immediate(),
            "dup" => self.dup(),
            "umov" | "smov" => self.to_general(),
            "ins" => self.insert(),
            "mov" if any_vector => self.vector_mov(),
            "addv" | "umaxv" | "uminv" | "smaxv" | "sminv" | "uaddlv" | "saddlv" => self.across(),
            "ext" => self.extract(),
            "tbl" | "tbx" => self.table(),
            "zip1" | "zip2" | "uzp1" | "uzp2" | "trn1" | "trn2" => self.permute(),
            "ushr" | "sshr" | "usra" | "ssra" | "sri" | "shl" | "sli" | "shrn" | "shrn2"
            | "ushll" | "ushll2" | "sshll" | "sshll2" | "uxtl" | "uxtl2" | "sxtl" | "sxtl2" => {
                self.vector_shift()
            }
            "cmeq" | "cmgt" | "cmge" | "cmle" | "cmlt" if any_vector && zero_last => {
                self.compare_zero()
            }
            "cnt" | "not" | "mvn" | "rbit" | "rev16" | "rev32" | "rev64" | "abs" | "neg"
            | "cls" | "clz" | "uaddlp" | "saddlp" | "xtn" | "xtn2"
                if any_vector =>
            {
                self.vector_two()
            }
            "add" | "sub" | "mul" | "uqadd" | "uqsub" | "sqadd" | "sqsub" | "cmeq" | "cmtst"
            | "cmhi" | "cmhs" | "cmgt" | "cmge" | "cmle" | "cmlt" | "cmlo" | "cmls" | "umax"
            | "umin" | "smax" | "smin" | "addp" | "umaxp" | "uminp" | "smaxp" | "sminp" | "and"
            | "bic" | "orr" | "orn" | "eor" | "bsl" | "bit" | "bif"
                if any_vector =>
            {
                self.vector_three()
            }
            _ => return None,
        })
    }

    /// The operations of two vectors, all three arrangements one.
    fn vector_three(&self) -> Result<Inst, String> {
        self.count(3)?;
        let (rd, first, second) = (self.v(0)?, self.v(1)?, self.v(2)?);
        // The comparisons below and at most are the others with their operands swapped.
        let (op, rn, rm) = match self.mnemonic {
            "add" => (VThree::Add, first, second),
            "sub" => (VThree::Sub, first, second),
            "mul" => (VThree::Mul, first, second),
            "uqadd" => (VThree::Uqadd, first, second),
            "uqsub" => (VThree::Uqsub, first, second),
            "sqadd" => (VThree::Sqadd, first, second),
            "sqsub" => (VThree::Sqsub, first, second),
            "cmeq" => (VThree::Cmeq, first, second),
            "cmtst" => (VThree::Cmtst, first, second),
            "cmhi" => (VThree::Cmhi, first, second),
            "cmhs" => (VThree::Cmhs, first, second),
            "cmgt" => (VThree::Cmgt, first, second),
            "cmge" => (VThree::Cmge, first, second),
            "cmlo" => (VThree::Cmhi, second, first),
            "cmls" => (VThree::Cmhs, second, first),
            "cmlt" => (VThree::Cmgt, second, first),
            "cmle" => (VThree::Cmge, second, first),
            "umax" => (VThree::Umax, first, second),
            "umin" => (VThree::Umin, first, second),
            "smax" => (VThree::Smax, first, second),
            "smin" => (VThree::Smin, first, second),
            "addp" => (VThree::Addp, first, second),
            "umaxp" => (VThree::Umaxp, first, second),
            "uminp" => (VThree::Uminp, first, second),
            "smaxp" => (VThree::Smaxp, first, second),
            "sminp" => (VThree::Sminp, first, second),
            "and" => (VThree::And, first, second),
            "bic" => (VThree::Bic, first, second),
            "orr" => (VThree::Orr, first, second),
            "orn" => (VThree::Orn, first, second),
            "eor" => (VThree::Eor, first, second),
            "bsl" => (VThree::Bsl, first, second),
            "bit" => (VThree::Bit, first, second),
            _ => (VThree::Bif, first, second),
        };
        let arrangement = rd.arrangement;
        if rn.arrangement != arrangement || rm.arrangement != arrangement {
            return Err(self.problem("its vectors aren't all one arrangement"));
        }
        if op.bitwise() && arrangement.size() != 0 {
            return Err(self.problem("a bitwise operation's vectors are 8b or 16b"));
        }
        let no_doublewords = matches!(
            op,
            VThree::Mul
                | VThree::Umax
                | VThree::Umin
                | VThree::Smax
                | VThree::Smin
                | VThree::Umaxp
                | VThree::Uminp
                | VThree::Smaxp
                | VThree::Sminp
        );
        if arrangement == Arrangement::D1 || (no_doublewords && arrangement.size() == 3) {
            return Err(self.problem("it doesn't take these doublewords"));
        }
        Ok(Inst::VectorThree { op, rd, rn, rm })
    }

    /// The operations of two vectors of floats, all three arrangements one: singles, or two
    /// doubles.
    fn float_three(&self) -> Result<Inst, String> {
        self.count(3)?;
        let (rd, first, second) = (self.v(0)?, self.v(1)?, self.v(2)?);
        // The comparisons below and at most are the others with their operands swapped.
        let (op, rn, rm) = match self.mnemonic {
            "fadd" => (VThree::Fadd, first, second),
            "fsub" => (VThree::Fsub, first, second),
            "fmul" => (VThree::Fmul, first, second),
            "fdiv" => (VThree::Fdiv, first, second),
            "fmax" => (VThree::Fmax, first, second),
            "fmin" => (VThree::Fmin, first, second),
            "fmaxnm" => (VThree::Fmaxnm, first, second),
            "fminnm" => (VThree::Fminnm, first, second),
            "fabd" => (VThree::Fabd, first, second),
            "faddp" => (VThree::Faddp, first, second),
            "fmaxp" => (VThree::Fmaxp, first, second),
            "fminp" => (VThree::Fminp, first, second),
            "fmaxnmp" => (VThree::Fmaxnmp, first, second),
            "fminnmp" => (VThree::Fminnmp, first, second),
            "fmla" => (VThree::Fmla, first, second),
            "fmls" => (VThree::Fmls, first, second),
            "fcmeq" => (VThree::Fcmeq, first, second),
            "fcmge" => (VThree::Fcmge, first, second),
            "fcmgt" => (VThree::Fcmgt, first, second),
            "fcmle" => (VThree::Fcmge, second, first),
            "fcmlt" => (VThree::Fcmgt, second, first),
            "facge" => (VThree::Facge, first, second),
            "facgt" => (VThree::Facgt, first, second),
            "facle" => (VThree::Facge, second, first),
            _ => (VThree::Facgt, second, first),
        };
        let arrangement = rd.arrangement;
        if rn.arrangement != arrangement
            || rm.arrangement != arrangement
            || !float_lanes(arrangement)
        {
            return Err(self.problem("its vectors aren't one arrangement of floats: 2s, 4s or 2d"));
        }
        Ok(Inst::VectorThree { op, rd, rn, rm })
    }

    /// The comparisons of each float with zero.
    fn float_compare_zero(&self) -> Result<Inst, String> {
        self.count(3)?;
        let (rd, rn) = (self.v(0)?, self.v(1)?);
        let op = match self.mnemonic {
            "fcmeq" => VTwo::Fcmeq0,
            "fcmgt" => VTwo::Fcmgt0,
            "fcmge" => VTwo::Fcmge0,
            "fcmle" => VTwo::Fcmle0,
            _ => VTwo::Fcmlt0,
        };
        if rd.arrangement != rn.arrangement || !float_lanes(rd.arrangement) {
            return Err(self.problem("its vectors aren't one arrangement of floats: 2s, 4s or 2d"));
        }
        Ok(Inst::VectorTwo { op, rd, rn })
    }

    /// The operations on one vector of floats, and the conversions between floats and whole
    /// numbers as wide, and between singles and doubles.
    fn float_two(&self) -> Result<Inst, String> {
        self.count(2)?;
        let (rd, rn) = (self.v(0)?, self.v(1)?);
        let op = match self.mnemonic {
            "fabs" => VTwo::Fabs,
            "fneg" => VTwo::Fneg,
            "fsqrt" => VTwo::Fsqrt,
            "frintn" => VTwo::Frintn,
            "frinta" => VTwo::Frinta,
            "frintp" => VTwo::Frintp,
            "frintm" => VTwo::Frintm,
            "frintz" => VTwo::Frintz,
            "fcvtzs" => VTwo::Fcvtzs,
            "fcvtzu" => VTwo::Fcvtzu,
            "scvtf" => VTwo::Scvtf,
            "ucvtf" => VTwo::Ucvtf,
            "fcvtl" | "fcvtl2" => VTwo::Fcvtl,
            _ => VTwo::Fcvtn,
        };
        let high = self.mnemonic.ends_with('2');
        let fits = match op {
            // Two singles, the low ones or with fcvtl2 the high ones, as two doubles.
            VTwo::Fcvtl => {
                rd.arrangement == Arrangement::D2 && rn.arrangement == Arrangement::new(2, high)
            }
            VTwo::Fcvtn => {
                rn.arrangement == Arrangement::D2 && rd.arrangement == Arrangement::new(2, high)
            }
            _ => rd.arrangement == rn.arrangement && float_lanes(rd.arrangement),
        };
        if !fits {
            return Err(self.problem("its vectors aren't arrangements of floats it takes"));
        }
        Ok(Inst::VectorTwo { op, rd, rn })
    }

    /// A vector's floats into one scalar: four singles' largest or smallest, or two floats'
    /// sum, largest or smallest.
    fn float_across(&self) -> Result<Inst, String> {
        self.count(2)?;
        let rn = self.v(1)?;
        let rd = self
            .operands
            .first()
            .and_then(|operand| scalar(operand))
            .ok_or_else(|| self.problem("operand 1 isn't a scalar such as s0"))?;
        let op = match self.mnemonic {
            "fmaxv" => VAcross::Fmaxv,
            "fminv" => VAcross::Fminv,
            "fmaxnmv" => VAcross::Fmaxnmv,
            "fminnmv" => VAcross::Fminnmv,
            "faddp" => VAcross::Faddp,
            "fmaxp" => VAcross::Fmaxp,
            "fminp" => VAcross::Fminp,
            "fmaxnmp" => VAcross::Fmaxnmp,
            _ => VAcross::Fminnmp,
        };
        let fits = if self.mnemonic.ends_with('v') {
            rn.arrangement == Arrangement::S4 && rd.size == 2
        } else {
            matches!(
                (rn.arrangement, rd.size),
                (Arrangement::S2, 2) | (Arrangement::D2, 3)
            )
        };
        if !fits {
            return Err(self.problem("its scalar and vector aren't sizes it takes"));
        }
        Ok(Inst::VectorAcross { op, rd, rn })
    }

    /// `fmov` of a float into every lane.
    fn float_immediate(&self) -> Result<Inst, String> {
        self.count(2)?;
        let rd = self.v(0)?;
        if !float_lanes(rd.arrangement) {
            return Err(self.problem("its vector isn't an arrangement of floats: 2s, 4s or 2d"));
        }
        let value: f64 = self.operands[1]
            .strip_prefix('#')
            .and_then(|number| number.parse().ok())
            .ok_or_else(|| self.problem("operand 2 isn't a number"))?;
        let imm8 = (0..=255_u8)
            .find(|imm8| super::super::fp_immediate(*imm8) == value)
            .ok_or_else(|| self.problem("its number isn't one fmov can hold"))?;
        Ok(Inst::VectorImmediate {
            op: VImmediate::Fmov,
            rd,
            imm8,
            expand: Expand::Float,
        })
    }

    /// The comparisons of each lane with zero.
    fn compare_zero(&self) -> Result<Inst, String> {
        self.count(3)?;
        let (rd, rn) = (self.v(0)?, self.v(1)?);
        let op = match self.mnemonic {
            "cmeq" => VTwo::Cmeq0,
            "cmgt" => VTwo::Cmgt0,
            "cmge" => VTwo::Cmge0,
            "cmle" => VTwo::Cmle0,
            _ => VTwo::Cmlt0,
        };
        if rd.arrangement != rn.arrangement || !not_one_doubleword(rd.arrangement) {
            return Err(self.problem("its vectors aren't one arrangement it takes"));
        }
        Ok(Inst::VectorTwo { op, rd, rn })
    }

    /// The operations on one vector's lanes.
    fn vector_two(&self) -> Result<Inst, String> {
        self.count(2)?;
        let (rd, rn) = (self.v(0)?, self.v(1)?);
        let op = match self.mnemonic {
            "cnt" => VTwo::Cnt,
            "not" | "mvn" => VTwo::Not,
            "rbit" => VTwo::Rbit,
            "rev16" => VTwo::Rev16,
            "rev32" => VTwo::Rev32,
            "rev64" => VTwo::Rev64,
            "abs" => VTwo::Abs,
            "neg" => VTwo::Neg,
            "cls" => VTwo::Cls,
            "clz" => VTwo::Clz,
            "uaddlp" => VTwo::Uaddlp,
            "saddlp" => VTwo::Saddlp,
            _ => VTwo::Xtn,
        };
        let size = rn.arrangement.size();
        let fits = match op {
            VTwo::Uaddlp | VTwo::Saddlp => {
                size <= 2 && rd.arrangement == Arrangement::new(size + 1, rn.arrangement.whole())
            }
            // The low half's lanes, or with xtn2 the high half's, from whole lanes twice as wide.
            VTwo::Xtn => {
                rn.arrangement.whole()
                    && (1..=3).contains(&size)
                    && rd.arrangement.size() == size - 1
                    && rd.arrangement.whole() == (self.mnemonic == "xtn2")
            }
            _ => {
                rd.arrangement == rn.arrangement
                    && match op {
                        VTwo::Cnt | VTwo::Not | VTwo::Rbit | VTwo::Rev16 => size == 0,
                        VTwo::Rev32 => size <= 1,
                        VTwo::Rev64 | VTwo::Cls | VTwo::Clz => size <= 2,
                        _ => not_one_doubleword(rn.arrangement),
                    }
            }
        };
        if !fits {
            return Err(self.problem("its vectors aren't arrangements it takes"));
        }
        Ok(Inst::VectorTwo { op, rd, rn })
    }

    /// A vector's lanes into one scalar.
    fn across(&self) -> Result<Inst, String> {
        self.count(2)?;
        let rn = self.v(1)?;
        let rd = self
            .operands
            .first()
            .and_then(|operand| scalar(operand))
            .ok_or_else(|| self.problem("operand 1 isn't a scalar such as b0"))?;
        let op = match self.mnemonic {
            "addv" => VAcross::Addv,
            "umaxv" => VAcross::Umaxv,
            "uminv" => VAcross::Uminv,
            "smaxv" => VAcross::Smaxv,
            "sminv" => VAcross::Sminv,
            "uaddlv" => VAcross::Uaddlv,
            _ => VAcross::Saddlv,
        };
        let size = rn.arrangement.size();
        let lanes_fit = size <= 1 || rn.arrangement == Arrangement::S4;
        if !lanes_fit || rd.size != size + u32::from(op.long()) {
            return Err(self.problem("its scalar and vector aren't sizes it takes"));
        }
        Ok(Inst::VectorAcross { op, rd, rn })
    }

    /// Each lane shifted by a number, with the widening and narrowing forms.
    fn vector_shift(&self) -> Result<Inst, String> {
        let op = match self.mnemonic {
            "ushr" => VShift::Ushr,
            "sshr" => VShift::Sshr,
            "usra" => VShift::Usra,
            "ssra" => VShift::Ssra,
            "sri" => VShift::Sri,
            "shl" => VShift::Shl,
            "sli" => VShift::Sli,
            "shrn" | "shrn2" => VShift::Shrn,
            "ushll" | "ushll2" | "uxtl" | "uxtl2" => VShift::Ushll,
            _ => VShift::Sshll,
        };
        let extend = self.mnemonic.contains("xtl");
        let high = self.mnemonic.ends_with('2');
        self.count(if extend { 2 } else { 3 })?;
        let (rd, rn) = (self.v(0)?, self.v(1)?);
        let amount = if extend { 0 } else { self.small(2, 64)? };
        let (narrow, wide) = match op {
            VShift::Shrn => (rd, rn),
            VShift::Ushll | VShift::Sshll => (rn, rd),
            _ => {
                let size = rd.arrangement.size();
                let esize = 8 << size;
                let in_range = if op.right() {
                    (1..=esize).contains(&amount)
                } else {
                    amount < esize
                };
                if rd.arrangement != rn.arrangement
                    || !not_one_doubleword(rd.arrangement)
                    || !in_range
                {
                    return Err(self.problem("its vectors and shift aren't ones it takes"));
                }
                return Ok(Inst::VectorShift {
                    op,
                    rd,
                    rn,
                    amount: amount as u8,
                });
            }
        };
        // The narrow side's lanes half as wide as the whole other side's, its half the low one,
        // or the high one with the `2` forms.
        let size = narrow.arrangement.size();
        let esize = 8 << size;
        let in_range = if op.right() {
            (1..=esize).contains(&amount)
        } else {
            amount < esize
        };
        if size > 2
            || wide.arrangement != Arrangement::new(size + 1, true)
            || narrow.arrangement.whole() != high
            || !in_range
        {
            return Err(self.problem("its vectors and shift aren't ones it takes"));
        }
        Ok(Inst::VectorShift {
            op,
            rd,
            rn,
            amount: amount as u8,
        })
    }

    /// The interleavings of two vectors.
    fn permute(&self) -> Result<Inst, String> {
        self.count(3)?;
        let (rd, rn, rm) = (self.v(0)?, self.v(1)?, self.v(2)?);
        let op = match self.mnemonic {
            "zip1" => VPermute::Zip1,
            "zip2" => VPermute::Zip2,
            "uzp1" => VPermute::Uzp1,
            "uzp2" => VPermute::Uzp2,
            "trn1" => VPermute::Trn1,
            _ => VPermute::Trn2,
        };
        if rn.arrangement != rd.arrangement
            || rm.arrangement != rd.arrangement
            || !not_one_doubleword(rd.arrangement)
        {
            return Err(self.problem("its vectors aren't one arrangement it takes"));
        }
        Ok(Inst::VectorPermute { op, rd, rn, rm })
    }

    /// `ext`: bytes of two vectors joined.
    fn extract(&self) -> Result<Inst, String> {
        self.count(4)?;
        let (rd, rn, rm) = (self.v(0)?, self.v(1)?, self.v(2)?);
        let bytes = rd.arrangement;
        let index = self.small(3, i64::from(bytes.bytes()) - 1)?;
        if bytes.size() != 0 || rn.arrangement != bytes || rm.arrangement != bytes {
            return Err(self.problem("its vectors aren't all 8b or all 16b"));
        }
        Ok(Inst::VectorExtract {
            rd,
            rn,
            rm,
            index: index as u8,
        })
    }

    /// `tbl` and `tbx`.
    fn table(&self) -> Result<Inst, String> {
        self.count(3)?;
        let (rd, rm) = (self.v(0)?, self.v(2)?);
        let (table, count, arrangement) = self
            .operands
            .get(1)
            .and_then(|operand| list(operand))
            .ok_or_else(|| self.problem("operand 2 isn't a list such as { v1.16b, v2.16b }"))?;
        if arrangement != Arrangement::B16
            || rd.arrangement.size() != 0
            || rm.arrangement != rd.arrangement
        {
            return Err(self.problem("its table is 16b registers, its vectors 8b or 16b"));
        }
        Ok(Inst::VectorTable {
            keep: self.mnemonic == "tbx",
            rd,
            table,
            count,
            rm,
        })
    }

    /// A general register as wide as a lane of `size` needs: `w` up to a word, `x` for a
    /// doubleword.
    fn general_for(&self, at: usize, size: u32) -> Result<super::super::A64Reg, String> {
        let found = self.reg(at)?;
        if found.gpr == Gpr::Sp || found.wide != (size == 3) {
            return Err(self.problem(&format!(
                "operand {} isn't the general register a lane this wide takes",
                at + 1
            )));
        }
        Ok(found)
    }

    /// `dup` of a lane or a general register.
    fn dup(&self) -> Result<Inst, String> {
        self.count(2)?;
        let rd = self.v(0)?;
        if !not_one_doubleword(rd.arrangement) {
            return Err(self.problem("it doesn't take one doubleword"));
        }
        let size = rd.arrangement.size();
        let from = match self.operands.get(1).and_then(|operand| lane(operand)) {
            Some(lane) if lane.size == size => Source::Lane(lane),
            Some(_) => return Err(self.problem("its lane and vector aren't one size")),
            None => Source::General(self.general_for(1, size)?),
        };
        Ok(Inst::VectorDup { rd, from })
    }

    /// `umov` and `smov`: a lane into a general register.
    fn to_general(&self) -> Result<Inst, String> {
        self.count(2)?;
        let rd = self.reg(0)?;
        let rn = self.lane(1)?;
        let signed = self.mnemonic == "smov";
        let fits = if signed {
            (!rd.wide && rn.size <= 1) || (rd.wide && rn.size <= 2)
        } else {
            rd.wide == (rn.size == 3)
        };
        if rd.gpr == Gpr::Sp || !fits {
            return Err(self.problem("its register and lane aren't sizes it takes"));
        }
        Ok(Inst::VectorToGeneral { signed, rd, rn })
    }

    /// `ins`: a general register or a lane into one lane.
    fn insert(&self) -> Result<Inst, String> {
        self.count(2)?;
        let rd = self.lane(0)?;
        let from = match self.operands.get(1).and_then(|operand| lane(operand)) {
            Some(lane) if lane.size == rd.size => Source::Lane(lane),
            Some(_) => return Err(self.problem("its lanes aren't one size")),
            None => Source::General(self.general_for(1, rd.size)?),
        };
        Ok(Inst::VectorInsert { rd, from })
    }

    /// `mov` of vectors: `orr` of a register with itself, `umov` of a word or a doubleword, or
    /// `ins`.
    fn vector_mov(&self) -> Result<Inst, String> {
        self.count(2)?;
        let first = self.operands[0];
        if lane(first).is_some() {
            return self.insert();
        }
        if let Some(rd) = vreg(first) {
            let rn = self.v(1)?;
            if rd.arrangement != rn.arrangement || rd.arrangement.size() != 0 {
                return Err(self.problem("a vector `mov` copies 8b or 16b"));
            }
            return Ok(Inst::VectorThree {
                op: VThree::Orr,
                rd,
                rn,
                rm: rn,
            });
        }
        let rd = self.reg(0)?;
        let rn = self.lane(1)?;
        if rd.gpr == Gpr::Sp || rn.size < 2 || rd.wide != (rn.size == 3) {
            return Err(self.problem("`mov` takes a word lane into w or a doubleword into x"));
        }
        Ok(Inst::VectorToGeneral {
            signed: false,
            rd,
            rn,
        })
    }

    /// `movi`, `mvni`, and `orr` and `bic` of a number.
    fn vector_immediate(&self) -> Result<Inst, String> {
        let op = match self.mnemonic {
            "movi" => VImmediate::Movi,
            "mvni" => VImmediate::Mvni,
            "orr" => VImmediate::Orr,
            _ => VImmediate::Bic,
        };
        if !(2..=3).contains(&self.operands.len()) {
            return Err(self.problem("it takes a vector, a number and maybe a shift"));
        }
        let first = self.operands[0];
        // `movi d0, #mask`: the one doubleword.
        let rd = match (
            vreg(first),
            first
                .strip_prefix('d')
                .and_then(|digits| number(&format!("v{digits}"))),
        ) {
            (Some(rd), _) => rd,
            (None, Some(number)) if op == VImmediate::Movi => VReg {
                number,
                arrangement: Arrangement::D1,
            },
            _ => return Err(self.problem("operand 1 isn't a vector register such as v0.4s")),
        };
        let value = self.imm(1)?;
        let shift = match self.operands.get(2) {
            None => None,
            Some(text) => {
                let (name, amount) = text
                    .split_once(char::is_whitespace)
                    .ok_or_else(|| self.problem(&format!("`{text}` isn't a shift")))?;
                Some((name, self.number(amount.trim())?))
            }
        };
        let size = rd.arrangement.size();
        let bad = || Err(self.problem("its number, shift and vector aren't a form it has"));
        let imm8 = |value: i64| u8::try_from(value).ok();
        let expand = match (op, size, shift) {
            (VImmediate::Movi, 3, None) => {
                // Each byte all ones or all zeros: one bit of the eight each.
                let mask = value.cast_unsigned();
                let mut bits = 0_u8;
                for byte in 0..8 {
                    match (mask >> (8 * byte)) & 0xff {
                        0 => {}
                        0xff => bits |= 1 << byte,
                        _ => return bad(),
                    }
                }
                return Ok(Inst::VectorImmediate {
                    op,
                    rd,
                    imm8: bits,
                    expand: Expand::Bytes,
                });
            }
            (VImmediate::Movi, 0, None | Some(("lsl", 0))) => Expand::Lsl(0),
            (_, 1, None) => Expand::Lsl(0),
            (_, 1, Some(("lsl", by @ (0 | 8)))) => Expand::Lsl(by as u8),
            (_, 2, None) => Expand::Lsl(0),
            (_, 2, Some(("lsl", by @ (0 | 8 | 16 | 24)))) => Expand::Lsl(by as u8),
            (VImmediate::Movi | VImmediate::Mvni, 2, Some(("msl", by @ (8 | 16)))) => {
                Expand::Msl(by as u8)
            }
            _ => return bad(),
        };
        // The 8-bit form is movi's alone.
        let Some(imm8) = imm8(value) else {
            return bad();
        };
        if size == 0 && op != VImmediate::Movi {
            return bad();
        }
        Ok(Inst::VectorImmediate {
            op,
            rd,
            imm8,
            expand,
        })
    }

    /// `ldr`, `str`, `ldur` and `stur` of a q register.
    fn q_load_store(&self) -> Result<Inst, String> {
        let unscaled = self.mnemonic == "ldur" || self.mnemonic == "stur";
        let load = self.mnemonic.starts_with("ld");
        let rt = self
            .operands
            .first()
            .and_then(|operand| q_register(operand))
            .ok_or_else(|| self.problem("operand 1 isn't a q register"))?;
        let address = self.address(1, 16)?;
        if unscaled && !matches!(address, Address::Offset { .. }) {
            return Err(self.problem("an unscaled access takes only an offset"));
        }
        Ok(Inst::VectorLoadStore { load, rt, address })
    }

    /// `ldp` and `stp` of two q registers.
    fn q_pair(&self) -> Result<Inst, String> {
        let load = self.mnemonic == "ldp";
        let register = |at: usize| {
            self.operands
                .get(at)
                .and_then(|operand| q_register(operand))
                .ok_or_else(|| self.problem(&format!("operand {} isn't a q register", at + 1)))
        };
        let (rt, rt2) = (register(0)?, register(1)?);
        let address = self.address(2, 16)?;
        if matches!(address, Address::Register { .. }) {
            return Err(self.problem("a pair's address takes only an offset"));
        }
        if load && rt == rt2 {
            return Err(self.problem("it loads one register twice, which has no defined result"));
        }
        Ok(Inst::VectorPair {
            load,
            rt,
            rt2,
            address,
        })
    }

    /// `ld1` and `st1` of 1 to 4 vectors, the base moving after by the bytes moved or by a
    /// register.
    fn structures(&self) -> Result<Inst, String> {
        let (first, count, arrangement) =
            self.operands
                .first()
                .and_then(|operand| list(operand))
                .ok_or_else(|| self.problem("operand 1 isn't a list such as { v0.16b }"))?;
        let base = self
            .operands
            .get(1)
            .and_then(|operand| operand.strip_prefix('['))
            .and_then(|operand| operand.strip_suffix(']'))
            .and_then(|name| register(name.trim()))
            .filter(|base| base.wide && base.gpr != Gpr::Zr)
            .ok_or_else(|| self.problem("its address isn't an x register in brackets"))?;
        let step = match self.operands.get(2) {
            None => None,
            Some(text) if text.starts_with('#') => {
                let bytes = i64::from(count) * i64::from(arrangement.bytes());
                if self.number(text)? != bytes {
                    return Err(self.problem(&format!(
                        "its base moves by the {bytes} bytes it moves, or by a register"
                    )));
                }
                Some(Writeback::Bytes)
            }
            Some(text) => {
                let by = register(text)
                    .filter(|by| by.wide && matches!(by.gpr, Gpr::R(_)))
                    .ok_or_else(|| self.problem("its base moves by a number or an x register"))?;
                Some(Writeback::Register(by))
            }
        };
        if self.operands.len() > 3 {
            return Err(self.problem("it has too many operands"));
        }
        Ok(Inst::VectorStructures {
            load: self.mnemonic == "ld1",
            first,
            count,
            arrangement,
            base,
            step,
        })
    }
}
