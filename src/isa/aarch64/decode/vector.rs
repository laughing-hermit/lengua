//! Advanced SIMD words: the integer vector instructions the dialect has, and the loads and
//! stores of whole vectors. Anything else in these classes is refused, as the rest of the
//! decoder refuses what isn't the dialect's.

use super::super::{
    A64Reg, Address, Arrangement, Expand, Inst, Lane, Scalar, Source, VAcross, VImmediate,
    VPermute, VReg, VShift, VThree, VTwo, Writeback,
};
use super::{bit, bits, outside, single_address};

/// A register number of five bits.
fn number(word: u32, low: u32) -> u8 {
    (bits(word, low + 4, low) & 31) as u8
}

/// `number` as a vector of `arrangement`.
const fn v(number: u8, arrangement: Arrangement) -> VReg {
    VReg {
        number,
        arrangement,
    }
}

/// `pmull` and `pmull2`: bytes to halfwords (size 0), or doublewords to a quadword (size 3, the
/// PMULL extension); sizes 1 and 2 aren't instructions.
fn pmull(word: u32) -> Result<Inst, String> {
    let q = bit(word, 30);
    let size = match bits(word, 23, 22) {
        0 => 0,
        3 => 3,
        _ => return Err(outside(word)),
    };
    let arrangement = Arrangement::new(size, q);
    Ok(Inst::Pmull {
        rd: number(word, 0),
        rn: v(number(word, 5), arrangement),
        rm: v(number(word, 16), arrangement),
    })
}

/// A vector word, bits 28 to 25 being 0111: three same, two-register miscellaneous, across
/// lanes, copy, permute, extract, table lookup, modified immediate and shift by immediate.
pub(super) fn vector(word: u32) -> Result<Inst, String> {
    if bit(word, 31) {
        return Err(outside(word));
    }
    let q = bit(word, 30);
    let u = bit(word, 29);
    if bits(word, 28, 23) == 0b01_1110 && bit(word, 10) {
        return if bits(word, 22, 19) == 0 {
            modified_immediate(word)
        } else {
            shift_immediate(word)
        };
    }
    if bits(word, 28, 24) != 0b0_1110 {
        return Err(outside(word));
    }
    if bit(word, 21) {
        if bit(word, 10) {
            return three_same(word);
        }
        if bits(word, 11, 10) == 0b10 {
            return match bits(word, 20, 17) {
                0b0000 => two_misc(word),
                0b1000 => across(word),
                _ => Err(outside(word)),
            };
        }
        // Three different: of them, the dialect has only the carry-less multiply.
        if bits(word, 11, 10) == 0 && !u && bits(word, 15, 12) == 0b1110 {
            return pmull(word);
        }
        return Err(outside(word));
    }
    if bit(word, 15) {
        return Err(outside(word));
    }
    if bits(word, 23, 21) == 0 && bit(word, 10) {
        return copy(word);
    }
    match (u, bits(word, 11, 10)) {
        (false, 0b10) => permute(word),
        (false, 0b00) if bits(word, 23, 22) == 0 => Ok(Inst::VectorTable {
            keep: bit(word, 12),
            rd: v(number(word, 0), Arrangement::new(0, q)),
            table: number(word, 5),
            count: (bits(word, 14, 13) + 1) as u8,
            rm: v(number(word, 16), Arrangement::new(0, q)),
        }),
        (true, 0b00 | 0b10) if bits(word, 23, 22) == 0 => {
            let index = bits(word, 14, 11);
            // Eight bytes reach only the first eight.
            if !q && index >= 8 {
                return Err(outside(word));
            }
            let bytes = Arrangement::new(0, q);
            Ok(Inst::VectorExtract {
                rd: v(number(word, 0), bytes),
                rn: v(number(word, 5), bytes),
                rm: v(number(word, 16), bytes),
                index: index as u8,
            })
        }
        _ => Err(outside(word)),
    }
}

/// Three same: 0 Q U 01110 size 1 Rm opcode 1 Rn Rd.
fn three_same(word: u32) -> Result<Inst, String> {
    let (q, u, size) = (bit(word, 30), bit(word, 29), bits(word, 23, 22));
    let opcode = bits(word, 15, 11);
    let (rd, rn, rm) = (number(word, 0), number(word, 5), number(word, 16));
    if opcode >= 0b1_1000 {
        return float_three(word);
    }
    if opcode == 0b0_0011 {
        // The bitwise operations, whose size field picks the operation.
        let op = match (u, size) {
            (false, 0) => VThree::And,
            (false, 1) => VThree::Bic,
            (false, 2) => VThree::Orr,
            (false, _) => VThree::Orn,
            (true, 0) => VThree::Eor,
            (true, 1) => VThree::Bsl,
            (true, 2) => VThree::Bit,
            (true, _) => VThree::Bif,
        };
        let bytes = Arrangement::new(0, q);
        return Ok(Inst::VectorThree {
            op,
            rd: v(rd, bytes),
            rn: v(rn, bytes),
            rm: v(rm, bytes),
        });
    }
    let op = match (opcode, u) {
        (0b0_0001, false) => VThree::Sqadd,
        (0b0_0001, true) => VThree::Uqadd,
        (0b0_0101, false) => VThree::Sqsub,
        (0b0_0101, true) => VThree::Uqsub,
        (0b0_0110, false) => VThree::Cmgt,
        (0b0_0110, true) => VThree::Cmhi,
        (0b0_0111, false) => VThree::Cmge,
        (0b0_0111, true) => VThree::Cmhs,
        (0b0_1100, false) => VThree::Smax,
        (0b0_1100, true) => VThree::Umax,
        (0b0_1101, false) => VThree::Smin,
        (0b0_1101, true) => VThree::Umin,
        (0b1_0000, false) => VThree::Add,
        (0b1_0000, true) => VThree::Sub,
        (0b1_0001, false) => VThree::Cmtst,
        (0b1_0001, true) => VThree::Cmeq,
        (0b1_0011, false) => VThree::Mul,
        (0b1_0100, false) => VThree::Smaxp,
        (0b1_0100, true) => VThree::Umaxp,
        (0b1_0101, false) => VThree::Sminp,
        (0b1_0101, true) => VThree::Uminp,
        (0b1_0111, false) => VThree::Addp,
        _ => return Err(outside(word)),
    };
    // Doublewords come only two at a time, and never multiply, or take a maximum or minimum.
    let no_doublewords = matches!(
        op,
        VThree::Mul
            | VThree::Smax
            | VThree::Umax
            | VThree::Smin
            | VThree::Umin
            | VThree::Smaxp
            | VThree::Umaxp
            | VThree::Sminp
            | VThree::Uminp
    );
    if size == 3 && (!q || no_doublewords) {
        return Err(outside(word));
    }
    let arrangement = Arrangement::new(size, q);
    Ok(Inst::VectorThree {
        op,
        rd: v(rd, arrangement),
        rn: v(rn, arrangement),
        rm: v(rm, arrangement),
    })
}

/// The lanes of floats, singles or doubles as `double` says: doubles only two at a time.
fn floats(double: bool, q: bool, word: u32) -> Result<Arrangement, String> {
    if double && !q {
        return Err(outside(word));
    }
    Ok(Arrangement::new(2 + u32::from(double), q))
}

/// Three same of floats: 0 Q U 01110 a sz 1 Rm 11 opcode 1 Rn Rd, `sz` choosing doubles. Not
/// `fmulx`, `frecps` or `frsqrts`, whose results the architecture leaves to estimates.
fn float_three(word: u32) -> Result<Inst, String> {
    let (q, u, a, double) = (bit(word, 30), bit(word, 29), bit(word, 23), bit(word, 22));
    let op = match (bits(word, 15, 11), u, a) {
        (0b1_1000, false, false) => VThree::Fmaxnm,
        (0b1_1000, false, true) => VThree::Fminnm,
        (0b1_1001, false, false) => VThree::Fmla,
        (0b1_1001, false, true) => VThree::Fmls,
        (0b1_1010, false, false) => VThree::Fadd,
        (0b1_1010, false, true) => VThree::Fsub,
        (0b1_1100, false, false) => VThree::Fcmeq,
        (0b1_1110, false, false) => VThree::Fmax,
        (0b1_1110, false, true) => VThree::Fmin,
        (0b1_1000, true, false) => VThree::Fmaxnmp,
        (0b1_1000, true, true) => VThree::Fminnmp,
        (0b1_1010, true, false) => VThree::Faddp,
        (0b1_1010, true, true) => VThree::Fabd,
        (0b1_1011, true, false) => VThree::Fmul,
        (0b1_1100, true, false) => VThree::Fcmge,
        (0b1_1100, true, true) => VThree::Fcmgt,
        (0b1_1101, true, false) => VThree::Facge,
        (0b1_1101, true, true) => VThree::Facgt,
        (0b1_1110, true, false) => VThree::Fmaxp,
        (0b1_1110, true, true) => VThree::Fminp,
        (0b1_1111, true, false) => VThree::Fdiv,
        _ => return Err(outside(word)),
    };
    let arrangement = floats(double, q, word)?;
    Ok(Inst::VectorThree {
        op,
        rd: v(number(word, 0), arrangement),
        rn: v(number(word, 5), arrangement),
        rm: v(number(word, 16), arrangement),
    })
}

/// Two-register miscellaneous of floats: 0 Q U 01110 a sz 10000 opcode 10 Rn Rd. Not the
/// conversions that round otherwise than toward zero, `frintx`, `frinti` or the estimates.
fn float_two(word: u32) -> Option<Result<Inst, String>> {
    let (q, u, a, double) = (bit(word, 30), bit(word, 29), bit(word, 23), bit(word, 22));
    let (rd, rn) = (number(word, 0), number(word, 5));
    let op = match (bits(word, 16, 12), u, a) {
        (0b0_1100, false, true) => VTwo::Fcmgt0,
        (0b0_1100, true, true) => VTwo::Fcmge0,
        (0b0_1101, false, true) => VTwo::Fcmeq0,
        (0b0_1101, true, true) => VTwo::Fcmle0,
        (0b0_1110, false, true) => VTwo::Fcmlt0,
        (0b0_1111, false, true) => VTwo::Fabs,
        (0b0_1111, true, true) => VTwo::Fneg,
        // Between singles and doubles: the doubles always two, the singles the low two or,
        // with Q, the high two.
        (0b1_0110 | 0b1_0111, false, false) => {
            if !double {
                return Some(Err(outside(word)));
            }
            let (singles, doubles) = (Arrangement::new(2, q), Arrangement::D2);
            return Some(Ok(if bit(word, 12) {
                Inst::VectorTwo {
                    op: VTwo::Fcvtl,
                    rd: v(rd, doubles),
                    rn: v(rn, singles),
                }
            } else {
                Inst::VectorTwo {
                    op: VTwo::Fcvtn,
                    rd: v(rd, singles),
                    rn: v(rn, doubles),
                }
            }));
        }
        (0b1_1000, false, false) => VTwo::Frintn,
        (0b1_1000, false, true) => VTwo::Frintp,
        (0b1_1000, true, false) => VTwo::Frinta,
        (0b1_1001, false, false) => VTwo::Frintm,
        (0b1_1001, false, true) => VTwo::Frintz,
        (0b1_1011, false, true) => VTwo::Fcvtzs,
        (0b1_1011, true, true) => VTwo::Fcvtzu,
        (0b1_1101, false, false) => VTwo::Scvtf,
        (0b1_1101, true, false) => VTwo::Ucvtf,
        (0b1_1111, true, true) => VTwo::Fsqrt,
        (0b0_1100..=0b0_1111 | 0b1_1000..=0b1_1111, _, _) => return Some(Err(outside(word))),
        _ => return None,
    };
    Some(floats(double, q, word).map(|arrangement| Inst::VectorTwo {
        op,
        rd: v(rd, arrangement),
        rn: v(rn, arrangement),
    }))
}

/// Two-register miscellaneous: 0 Q U 01110 size 10000 opcode 10 Rn Rd.
fn two_misc(word: u32) -> Result<Inst, String> {
    let (q, u, size) = (bit(word, 30), bit(word, 29), bits(word, 23, 22));
    let (rd, rn) = (number(word, 0), number(word, 5));
    if let Some(float) = float_two(word) {
        return float;
    }
    let same = |op| {
        let arrangement = Arrangement::new(size, q);
        Ok(Inst::VectorTwo {
            op,
            rd: v(rd, arrangement),
            rn: v(rn, arrangement),
        })
    };
    let bytes = |op| {
        let arrangement = Arrangement::new(0, q);
        Ok(Inst::VectorTwo {
            op,
            rd: v(rd, arrangement),
            rn: v(rn, arrangement),
        })
    };
    // The comparisons with zero, `abs` and `neg` take doublewords only two at a time.
    let any_size = !(size == 3 && !q);
    match (bits(word, 16, 12), u, size) {
        (0b0_0000, false, 0..=2) => same(VTwo::Rev64),
        (0b0_0000, true, 0 | 1) => same(VTwo::Rev32),
        (0b0_0001, false, 0) => same(VTwo::Rev16),
        (0b0_0010, _, 0..=2) => Ok(Inst::VectorTwo {
            op: if u { VTwo::Uaddlp } else { VTwo::Saddlp },
            rd: v(rd, Arrangement::new(size + 1, q)),
            rn: v(rn, Arrangement::new(size, q)),
        }),
        (0b0_0100, false, 0..=2) => same(VTwo::Cls),
        (0b0_0100, true, 0..=2) => same(VTwo::Clz),
        (0b0_0101, false, 0) => bytes(VTwo::Cnt),
        (0b0_0101, true, 0) => bytes(VTwo::Not),
        (0b0_0101, true, 1) => bytes(VTwo::Rbit),
        (0b0_1000, false, _) if any_size => same(VTwo::Cmgt0),
        (0b0_1000, true, _) if any_size => same(VTwo::Cmge0),
        (0b0_1001, false, _) if any_size => same(VTwo::Cmeq0),
        (0b0_1001, true, _) if any_size => same(VTwo::Cmle0),
        (0b0_1010, false, _) if any_size => same(VTwo::Cmlt0),
        (0b0_1011, false, _) if any_size => same(VTwo::Abs),
        (0b0_1011, true, _) if any_size => same(VTwo::Neg),
        // Each lane's low half, into the destination's low half or, with Q, its high half.
        (0b1_0010, false, 0..=2) => Ok(Inst::VectorTwo {
            op: VTwo::Xtn,
            rd: v(rd, Arrangement::new(size, q)),
            rn: v(rn, Arrangement::new(size + 1, true)),
        }),
        _ => Err(outside(word)),
    }
}

/// Across lanes: 0 Q U 01110 size 11000 opcode 10 Rn Rd.
fn across(word: u32) -> Result<Inst, String> {
    let (q, u, size) = (bit(word, 30), bit(word, 29), bits(word, 23, 22));
    if u && matches!(bits(word, 16, 12), 0b0_1100 | 0b0_1111) {
        // Floats: four singles only.
        if bit(word, 22) || !q {
            return Err(outside(word));
        }
        let op = match (bits(word, 16, 12), bit(word, 23)) {
            (0b0_1100, false) => VAcross::Fmaxnmv,
            (0b0_1100, true) => VAcross::Fminnmv,
            (_, false) => VAcross::Fmaxv,
            (_, true) => VAcross::Fminv,
        };
        return Ok(Inst::VectorAcross {
            op,
            rd: Scalar {
                number: number(word, 0),
                size: 2,
            },
            rn: v(number(word, 5), Arrangement::S4),
        });
    }
    // Words come four at a time; doublewords never.
    if size == 3 || (size == 2 && !q) {
        return Err(outside(word));
    }
    let op = match (bits(word, 16, 12), u) {
        (0b0_0011, false) => VAcross::Saddlv,
        (0b0_0011, true) => VAcross::Uaddlv,
        (0b0_1010, false) => VAcross::Smaxv,
        (0b0_1010, true) => VAcross::Umaxv,
        (0b1_1010, false) => VAcross::Sminv,
        (0b1_1010, true) => VAcross::Uminv,
        (0b1_1011, false) => VAcross::Addv,
        _ => return Err(outside(word)),
    };
    Ok(Inst::VectorAcross {
        op,
        rd: Scalar {
            number: number(word, 0),
            size: size + u32::from(op.long()),
        },
        rn: v(number(word, 5), Arrangement::new(size, q)),
    })
}

/// Scalar pairwise of floats: 01 1 11110 a sz 11000 opcode 10 Rn Rd, a vector's two floats
/// into one: two singles in the low half, or two doubles.
pub(super) fn pairwise(word: u32) -> Result<Inst, String> {
    if !bit(word, 29) {
        return Err(outside(word));
    }
    let op = match (bits(word, 16, 12), bit(word, 23)) {
        (0b0_1100, false) => VAcross::Fmaxnmp,
        (0b0_1100, true) => VAcross::Fminnmp,
        (0b0_1101, false) => VAcross::Faddp,
        (0b0_1111, false) => VAcross::Fmaxp,
        (0b0_1111, true) => VAcross::Fminp,
        _ => return Err(outside(word)),
    };
    let double = bit(word, 22);
    let size = 2 + u32::from(double);
    Ok(Inst::VectorAcross {
        op,
        rd: Scalar {
            number: number(word, 0),
            size,
        },
        rn: v(number(word, 5), Arrangement::new(size, double)),
    })
}

/// Copy: 0 Q op 01110000 imm5 0 imm4 1 Rn Rd. The lowest set bit of imm5 gives the lane's size,
/// the bits above it the index.
fn copy(word: u32) -> Result<Inst, String> {
    let (q, op) = (bit(word, 30), bit(word, 29));
    let imm5 = bits(word, 20, 16);
    let imm4 = bits(word, 14, 11);
    let size = imm5.trailing_zeros();
    if size > 3 {
        return Err(outside(word));
    }
    let index = (imm5 >> (size + 1)) as u8;
    let (rd, rn) = (number(word, 0), number(word, 5));
    let lane = |number, index| Lane {
        number,
        size,
        index,
    };
    if op {
        // `ins` from a lane, its index in imm4's bits above the size; the ones below are free.
        if !q {
            return Err(outside(word));
        }
        return Ok(Inst::VectorInsert {
            rd: lane(rd, index),
            from: Source::Lane(lane(rn, (imm4 >> size) as u8)),
        });
    }
    let general = |number: u8, wide: bool| A64Reg::new(u32::from(number), wide, false);
    let doublewords_in_half = size == 3 && !q;
    match imm4 {
        0b0000 if !doublewords_in_half => Ok(Inst::VectorDup {
            rd: v(rd, Arrangement::new(size, q)),
            from: Source::Lane(lane(rn, index)),
        }),
        0b0001 if !doublewords_in_half => Ok(Inst::VectorDup {
            rd: v(rd, Arrangement::new(size, q)),
            from: Source::General(general(rn, size == 3)),
        }),
        0b0011 if q => Ok(Inst::VectorInsert {
            rd: lane(rd, index),
            from: Source::General(general(rn, size == 3)),
        }),
        // `smov`: into a w register from a byte or a halfword, into an x one from up to a word.
        0b0101 if (!q && size <= 1) || (q && size <= 2) => Ok(Inst::VectorToGeneral {
            signed: true,
            rd: general(rd, q),
            rn: lane(rn, index),
        }),
        // `umov`: into a w register up to a word, into an x one a doubleword.
        0b0111 if (!q && size <= 2) || (q && size == 3) => Ok(Inst::VectorToGeneral {
            signed: false,
            rd: general(rd, q),
            rn: lane(rn, index),
        }),
        _ => Err(outside(word)),
    }
}

/// Permute: 0 Q 001110 size 0 Rm 0 opcode 10 Rn Rd.
fn permute(word: u32) -> Result<Inst, String> {
    let (q, size) = (bit(word, 30), bits(word, 23, 22));
    if size == 3 && !q {
        return Err(outside(word));
    }
    let op = match bits(word, 14, 12) {
        0b001 => VPermute::Uzp1,
        0b010 => VPermute::Trn1,
        0b011 => VPermute::Zip1,
        0b101 => VPermute::Uzp2,
        0b110 => VPermute::Trn2,
        0b111 => VPermute::Zip2,
        _ => return Err(outside(word)),
    };
    let arrangement = Arrangement::new(size, q);
    Ok(Inst::VectorPermute {
        op,
        rd: v(number(word, 0), arrangement),
        rn: v(number(word, 5), arrangement),
        rm: v(number(word, 16), arrangement),
    })
}

/// Modified immediate: 0 Q op 0111100000 a b c cmode o2 1 d e f g h Rd.
fn modified_immediate(word: u32) -> Result<Inst, String> {
    let (q, op) = (bit(word, 30), bit(word, 29));
    let cmode = bits(word, 15, 12);
    // o2 is the half-precision fmov.
    if bit(word, 11) {
        return Err(outside(word));
    }
    let imm8 = ((bits(word, 18, 16) << 5) | bits(word, 9, 5)) as u8;
    let lsl = |by: u32| Expand::Lsl((8 * by) as u8);
    let (kind, size, expand) = match (cmode, op) {
        (0b0000 | 0b0010 | 0b0100 | 0b0110, false) => (VImmediate::Movi, 2, lsl(cmode >> 1)),
        (0b0001 | 0b0011 | 0b0101 | 0b0111, false) => (VImmediate::Orr, 2, lsl(cmode >> 1)),
        (0b1000 | 0b1010, false) => (VImmediate::Movi, 1, lsl((cmode >> 1) & 1)),
        (0b1001 | 0b1011, false) => (VImmediate::Orr, 1, lsl((cmode >> 1) & 1)),
        (0b1100 | 0b1101, false) => (
            VImmediate::Movi,
            2,
            Expand::Msl((8 * ((cmode & 1) + 1)) as u8),
        ),
        (0b1110, false) => (VImmediate::Movi, 0, Expand::Lsl(0)),
        (0b0000 | 0b0010 | 0b0100 | 0b0110, true) => (VImmediate::Mvni, 2, lsl(cmode >> 1)),
        (0b0001 | 0b0011 | 0b0101 | 0b0111, true) => (VImmediate::Bic, 2, lsl(cmode >> 1)),
        (0b1000 | 0b1010, true) => (VImmediate::Mvni, 1, lsl((cmode >> 1) & 1)),
        (0b1001 | 0b1011, true) => (VImmediate::Bic, 1, lsl((cmode >> 1) & 1)),
        (0b1100 | 0b1101, true) => (
            VImmediate::Mvni,
            2,
            Expand::Msl((8 * ((cmode & 1) + 1)) as u8),
        ),
        // A doubleword of whole bytes: `movi d0, #…` alone, or `movi v0.2d, #…`.
        (0b1110, true) => (VImmediate::Movi, 3, Expand::Bytes),
        // A float in every lane: singles, or two doubles.
        (0b1111, false) => (VImmediate::Fmov, 2, Expand::Float),
        (0b1111, true) if q => (VImmediate::Fmov, 3, Expand::Float),
        _ => return Err(outside(word)),
    };
    Ok(Inst::VectorImmediate {
        op: kind,
        rd: v(number(word, 0), Arrangement::new(size, q)),
        imm8,
        expand,
    })
}

/// Shift by immediate: 0 Q U 011110 immh immb opcode 1 Rn Rd, immh not zero. immh's highest
/// set bit gives the lanes' size; a right shift is 2 × esize less immh:immb, a left one
/// immh:immb less esize.
fn shift_immediate(word: u32) -> Result<Inst, String> {
    let (q, u) = (bit(word, 30), bit(word, 29));
    let immh = bits(word, 22, 19);
    let size = 31 - immh.leading_zeros();
    let esize = 8 << size;
    let both = bits(word, 22, 16);
    let (rd, rn) = (number(word, 0), number(word, 5));
    let same = |op, amount: u32| {
        let arrangement = Arrangement::new(size, q);
        Ok(Inst::VectorShift {
            op,
            rd: v(rd, arrangement),
            rn: v(rn, arrangement),
            amount: amount as u8,
        })
    };
    let doublewords_in_half = size == 3 && !q;
    match (bits(word, 15, 11), u) {
        (0b0_0000, false) if !doublewords_in_half => same(VShift::Sshr, 2 * esize - both),
        (0b0_0000, true) if !doublewords_in_half => same(VShift::Ushr, 2 * esize - both),
        (0b0_0010, false) if !doublewords_in_half => same(VShift::Ssra, 2 * esize - both),
        (0b0_0010, true) if !doublewords_in_half => same(VShift::Usra, 2 * esize - both),
        (0b0_1000, true) if !doublewords_in_half => same(VShift::Sri, 2 * esize - both),
        (0b0_1010, false) if !doublewords_in_half => same(VShift::Shl, both - esize),
        (0b0_1010, true) if !doublewords_in_half => same(VShift::Sli, both - esize),
        // Narrowing: the destination's lanes are esize wide, the source's twice that.
        (0b1_0000, false) if size < 3 => Ok(Inst::VectorShift {
            op: VShift::Shrn,
            rd: v(rd, Arrangement::new(size, q)),
            rn: v(rn, Arrangement::new(size + 1, true)),
            amount: (2 * esize - both) as u8,
        }),
        // Widening: the source's lanes are esize wide, from its low half or, with Q, its high.
        (0b1_0100, _) if size < 3 => Ok(Inst::VectorShift {
            op: if u { VShift::Ushll } else { VShift::Sshll },
            rd: v(rd, Arrangement::new(size + 1, true)),
            rn: v(rn, Arrangement::new(size, q)),
            amount: (both - esize) as u8,
        }),
        _ => Err(outside(word)),
    }
}

/// A load or store of one q register: size 00, opc 1x, its offsets scaled by 16.
pub(super) fn q_single(word: u32) -> Result<Inst, String> {
    let address = single_address(word, 4)?;
    Ok(Inst::VectorLoadStore {
        load: bit(word, 22),
        rt: number(word, 0),
        address,
    })
}

/// A load or store of two q registers: opc 10.
pub(super) fn q_pair(word: u32, address: Address) -> Result<Inst, String> {
    let load = bit(word, 22);
    let (rt, rt2) = (number(word, 0), number(word, 10));
    // Two loads into one register have no defined result.
    if load && rt == rt2 {
        return Err(outside(word));
    }
    Ok(Inst::VectorPair {
        load,
        rt,
        rt2,
        address,
    })
}

/// `ld1` and `st1` of 1 to 4 registers: 0 Q 0011000 L 000000 opcode size Rn Rt, or with the
/// base moving after, 0 Q 0011001 L 0 Rm opcode size Rn Rt, by the bytes moved where Rm is 31.
pub(super) fn structures(word: u32) -> Result<Inst, String> {
    // Bit 24 is a single lane's load or store.
    if bit(word, 31) || bit(word, 24) {
        return Err(outside(word));
    }
    let moves = bit(word, 23);
    if (!moves && bits(word, 21, 16) != 0) || (moves && bit(word, 21)) {
        return Err(outside(word));
    }
    // The interleaving ld2 to ld4 and st2 to st4 aren't the dialect's.
    let count = match bits(word, 15, 12) {
        0b0111 => 1,
        0b1010 => 2,
        0b0110 => 3,
        0b0010 => 4,
        _ => return Err(outside(word)),
    };
    let rm = bits(word, 20, 16);
    let step = moves.then(|| {
        if rm == 31 {
            Writeback::Bytes
        } else {
            Writeback::Register(A64Reg::new(rm, true, false))
        }
    });
    Ok(Inst::VectorStructures {
        load: bit(word, 22),
        first: number(word, 0),
        count,
        arrangement: Arrangement::new(bits(word, 11, 10), bit(word, 30)),
        base: A64Reg::new(bits(word, 9, 5), true, true),
        step,
    })
}
