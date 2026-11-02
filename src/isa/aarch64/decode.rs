//! AArch64 machine words to the dialect's canonical instructions: lengua's own decoder for
//! exactly the subset its AArch64 dialect allows, so that anything outside it is refused by
//! construction and the reading of the machine code is lengua's, not the assembler's. LLVM's
//! disassembler is its test.

use super::{
    A64Cond, A64Reg, Access, Address, Extend, Field, FpConversion, FpOne, FpReg, FpThree, FpTwo,
    Indirect, Inst, Logic, One, Select, Shift, Three, Two, Wide,
};

mod vector;

/// Bits `high` down to `low` of `word`.
const fn bits(word: u32, high: u32, low: u32) -> u32 {
    (word >> low) & ((1 << (high - low + 1)) - 1)
}

/// Bit `at` of `word`.
const fn bit(word: u32, at: u32) -> bool {
    (word >> at) & 1 == 1
}

/// The low `width` bits of `value` read as a signed number.
const fn signed(value: u32, width: u32) -> i64 {
    let shift = 64 - width;
    ((value as i64) << shift) >> shift
}

/// Why `word` has no canonical form.
fn outside(word: u32) -> String {
    format!("{word:#010x} isn't an instruction lengua's AArch64 dialect has")
}

/// The machine word at `address`, in the dialect's canonical form.
///
/// # Errors
///
/// Why the word isn't in the dialect's subset.
pub fn decode(word: u32, address: u64) -> Result<Inst, String> {
    match bits(word, 28, 25) {
        0b1000 | 0b1001 => immediate(word, address),
        0b1010 | 0b1011 => branch(word, address),
        0b0100 | 0b0110 | 0b1100 | 0b1110 => load_store(word),
        0b0101 | 0b1101 => register(word),
        0b0111 => vector::vector(word),
        0b1111 => floating(word),
        // Scalable vectors, and what's unallocated.
        _ => Err(outside(word)),
    }
}

/// The logical operation `opc` names.
const fn logic(opc: u32) -> Logic {
    match opc & 3 {
        0 => Logic::And,
        1 => Logic::Orr,
        2 => Logic::Eor,
        _ => Logic::Ands,
    }
}

/// All ones in the low `width` bits.
const fn ones(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1 << width) - 1
    }
}

/// The number a logical instruction's `N`, `immr` and `imms` fields encode, at `width` bits:
/// the architecture's `DecodeBitMasks`, for immediates.
#[must_use]
pub fn bitmask(n: bool, immr: u32, imms: u32, width: u32) -> Option<u64> {
    let combined = (u32::from(n) << 6) | (!imms & 0x3f);
    if combined == 0 {
        return None;
    }
    let length = 31 - combined.leading_zeros();
    let element_size = 1_u32 << length;
    if length < 1 || element_size > width {
        return None;
    }
    let levels = element_size - 1;
    let (set, rotation) = (imms & levels, immr & levels);
    // An element of all ones isn't a number this encodes.
    if set == levels {
        return None;
    }
    let element = ones(set + 1);
    let rotated = if rotation == 0 {
        element
    } else {
        ((element >> rotation) | (element << (element_size - rotation))) & ones(element_size)
    };
    let mut value = 0;
    let mut position = 0;
    while position < width {
        value |= rotated << position;
        position += element_size;
    }
    Some(value & ones(width))
}

/// Data processing with a number in the instruction.
fn immediate(word: u32, address: u64) -> Result<Inst, String> {
    let wide = bit(word, 31);
    let register = |high, low, sp| A64Reg::new(bits(word, high, low), wide, sp);
    match bits(word, 25, 23) {
        0b000 | 0b001 => {
            // adr and adrp: op immlo 10000 immhi Rd.
            let rd = A64Reg::new(bits(word, 4, 0), true, false);
            let immediate = (bits(word, 23, 5) << 2) | bits(word, 30, 29);
            if bit(word, 31) {
                let page = (address & !0xfff).wrapping_add_signed(signed(immediate, 21) << 12);
                Ok(Inst::Adrp { rd, page })
            } else {
                let target = address.wrapping_add_signed(signed(immediate, 21));
                Ok(Inst::Adr { rd, target })
            }
        }
        0b010 => {
            // add/sub (immediate): sf op S 100010 sh imm12 Rn Rd. Register 31 is the stack
            // pointer, but where the flags are set the result is the zero register's.
            let flags = bit(word, 29);
            Ok(Inst::AddImm {
                sub: bit(word, 30),
                flags,
                rd: register(4, 0, !flags),
                rn: register(9, 5, true),
                imm: bits(word, 21, 10),
                shift12: bit(word, 22),
            })
        }
        0b100 => {
            // Logical (immediate): sf opc 100100 N immr imms Rn Rd.
            let n = bit(word, 22);
            if !wide && n {
                return Err(outside(word));
            }
            let op = logic(bits(word, 30, 29));
            let width = if wide { 64 } else { 32 };
            let imm = bitmask(n, bits(word, 21, 16), bits(word, 15, 10), width)
                .ok_or_else(|| outside(word))?;
            Ok(Inst::LogicalImm {
                op,
                rd: register(4, 0, op != Logic::Ands),
                rn: register(9, 5, false),
                imm,
            })
        }
        0b101 => {
            // Move wide: sf opc 100101 hw imm16 Rd.
            let op = match bits(word, 30, 29) {
                0 => Wide::Movn,
                2 => Wide::Movz,
                3 => Wide::Movk,
                _ => return Err(outside(word)),
            };
            let hw = bits(word, 22, 21);
            if !wide && hw > 1 {
                return Err(outside(word));
            }
            Ok(Inst::MoveWide {
                op,
                rd: register(4, 0, false),
                imm: u16::try_from(bits(word, 20, 5)).map_err(|_| outside(word))?,
                shift: u8::try_from(hw * 16).map_err(|_| outside(word))?,
            })
        }
        0b110 => {
            // Bitfield: sf opc 100110 N immr imms Rn Rd.
            let op = match bits(word, 30, 29) {
                0 => Field::Sbfm,
                1 => Field::Bfm,
                2 => Field::Ubfm,
                _ => return Err(outside(word)),
            };
            let (n, immr, imms) = (bit(word, 22), bits(word, 21, 16), bits(word, 15, 10));
            if n != wide || (!wide && (immr > 31 || imms > 31)) {
                return Err(outside(word));
            }
            Ok(Inst::Bitfield {
                op,
                rd: register(4, 0, false),
                rn: register(9, 5, false),
                immr: u8::try_from(immr).map_err(|_| outside(word))?,
                imms: u8::try_from(imms).map_err(|_| outside(word))?,
            })
        }
        0b111 => {
            // Extract: sf 00 100111 N 0 Rm imms Rn Rd.
            let imms = bits(word, 15, 10);
            if bits(word, 30, 29) != 0
                || bit(word, 21)
                || bit(word, 22) != wide
                || (!wide && imms > 31)
            {
                return Err(outside(word));
            }
            Ok(Inst::Extract {
                rd: register(4, 0, false),
                rn: register(9, 5, false),
                rm: register(20, 16, false),
                lsb: u8::try_from(imms).map_err(|_| outside(word))?,
            })
        }
        // add/sub with tags, and what's newer.
        _ => Err(outside(word)),
    }
}

/// Branches, system calls and `nop`.
fn branch(word: u32, address: u64) -> Result<Inst, String> {
    let at = |offset: i64| address.wrapping_add_signed(offset << 2);
    if bits(word, 30, 26) == 0b00101 {
        // b and bl: op 00101 imm26.
        return Ok(Inst::Branch {
            link: bit(word, 31),
            target: at(signed(bits(word, 25, 0), 26)),
        });
    }
    if bits(word, 30, 25) == 0b011010 {
        // cbz and cbnz: sf 011010 op imm19 Rt.
        return Ok(Inst::CompareBranch {
            nonzero: bit(word, 24),
            rt: A64Reg::new(bits(word, 4, 0), bit(word, 31), false),
            target: at(signed(bits(word, 23, 5), 19)),
        });
    }
    if bits(word, 30, 25) == 0b011011 {
        // tbz and tbnz: b5 011011 op b40 imm14 Rt, a w register for bits below 32.
        let number = (bits(word, 31, 31) << 5) | bits(word, 23, 19);
        return Ok(Inst::TestBranch {
            nonzero: bit(word, 24),
            rt: A64Reg::new(bits(word, 4, 0), bit(word, 31), false),
            bit: u8::try_from(number).map_err(|_| outside(word))?,
            target: at(signed(bits(word, 18, 5), 14)),
        });
    }
    if bits(word, 31, 24) == 0b0101_0100 && !bit(word, 4) {
        // b.cond: 01010100 imm19 0 cond.
        return Ok(Inst::BranchCond {
            cond: A64Cond::from_bits(bits(word, 3, 0)),
            target: at(signed(bits(word, 23, 5), 19)),
        });
    }
    if word & 0xffe0_001f == 0xd400_0001 {
        // svc #imm16.
        return Ok(Inst::Svc {
            imm: u16::try_from(bits(word, 20, 5)).map_err(|_| outside(word))?,
        });
    }
    let rn = A64Reg::new(bits(word, 9, 5), true, false);
    let through = |kind| Ok(Inst::BranchReg { kind, rn });
    match word & 0xffff_fc1f {
        0xd61f_0000 => return through(Indirect::Br),
        0xd63f_0000 => return through(Indirect::Blr),
        0xd65f_0000 => return through(Indirect::Ret),
        _ => {}
    }
    if word == 0xd503_201f {
        return Ok(Inst::Nop);
    }
    Err(outside(word))
}

/// Loads and stores of general-purpose registers.
fn load_store(word: u32) -> Result<Inst, String> {
    // Floating-point and vector registers: singles, doubles and whole q registers, and ld1/st1.
    if bit(word, 26) {
        return match bits(word, 29, 27) {
            0b111 => fp_single(word),
            0b101 => fp_pair(word),
            0b001 => vector::structures(word),
            _ => Err(outside(word)),
        };
    }
    match bits(word, 29, 27) {
        0b111 => single(word),
        0b101 => pair(word),
        // Exclusives, literal loads, memory copies.
        _ => Err(outside(word)),
    }
}

/// A load or store of one register: size 111 0 opc …
fn single(word: u32) -> Result<Inst, String> {
    let size = bits(word, 31, 30);
    let (access, wide) = match (size, bits(word, 23, 22)) {
        (_, 0) => (
            Access {
                load: false,
                size: 1 << size,
                signed: false,
            },
            size == 3,
        ),
        (_, 1) => (
            Access {
                load: true,
                size: 1 << size,
                signed: false,
            },
            size == 3,
        ),
        (0..=2, 2) => (
            Access {
                load: true,
                size: 1 << size,
                signed: true,
            },
            true,
        ),
        (0 | 1, 3) => (
            Access {
                load: true,
                size: 1 << size,
                signed: true,
            },
            false,
        ),
        // prfm, and what's unallocated.
        _ => return Err(outside(word)),
    };
    let base = A64Reg::new(bits(word, 9, 5), true, true);
    let rt = A64Reg::new(bits(word, 4, 0), wide, false);
    let address = single_address(word, size)?;
    // A base that moves and is also the register transferred has no defined result.
    if let Address::PreIndex { .. } | Address::PostIndex { .. } = address
        && base.gpr == rt.gpr
    {
        return Err(outside(word));
    }
    Ok(Inst::LoadStore {
        access,
        rt,
        address,
    })
}

/// The address of a load or store of one register `1 << size` bytes wide: an unsigned offset
/// scaled by the size, an unscaled one, a base moving before or after, or a register's index.
fn single_address(word: u32, size: u32) -> Result<Address, String> {
    let base = A64Reg::new(bits(word, 9, 5), true, true);
    let address = if bit(word, 24) {
        // An unsigned offset, scaled by the size.
        let offset = i64::from(bits(word, 21, 10)) << size;
        Address::Offset { base, offset }
    } else if !bit(word, 21) {
        let offset = signed(bits(word, 20, 12), 9);
        match bits(word, 11, 10) {
            0b00 => Address::Offset { base, offset },
            0b01 => Address::PostIndex { base, offset },
            0b11 => Address::PreIndex { base, offset },
            // Unprivileged loads and stores.
            _ => return Err(outside(word)),
        }
    } else if bits(word, 11, 10) == 0b10 {
        let extend = match bits(word, 15, 13) {
            0b010 => Extend::Uxtw,
            0b011 => Extend::Lsl,
            0b110 => Extend::Sxtw,
            0b111 => Extend::Sxtx,
            _ => return Err(outside(word)),
        };
        let index = A64Reg::new(
            bits(word, 20, 16),
            matches!(extend, Extend::Lsl | Extend::Sxtx),
            false,
        );
        let shift = if bit(word, 12) {
            u8::try_from(size).map_err(|_| outside(word))?
        } else {
            0
        };
        Address::Register {
            base,
            index,
            extend,
            shift,
        }
    } else {
        // Atomics and pointer authentication.
        return Err(outside(word));
    };
    Ok(address)
}

/// A load or store of one floating-point register: a single (`size` 10) or a double (11), `opc`
/// 00 to store and 01 to load; or of a whole q register (`size` 00, `opc` 1x).
fn fp_single(word: u32) -> Result<Inst, String> {
    let size = bits(word, 31, 30);
    if size == 0 && bit(word, 23) {
        return vector::q_single(word);
    }
    let double = match size {
        0b10 => false,
        0b11 => true,
        // Bytes, halves and whole vectors.
        _ => return Err(outside(word)),
    };
    let load = match bits(word, 23, 22) {
        0b00 => false,
        0b01 => true,
        _ => return Err(outside(word)),
    };
    let rt = FpReg {
        number: u8::try_from(bits(word, 4, 0)).map_err(|_| outside(word))?,
        double,
    };
    let address = single_address(word, size)?;
    Ok(Inst::FpLoadStore { load, rt, address })
}

/// A load or store of two floating-point registers: `opc` 00 singles, 01 doubles or 10 whole
/// q registers.
fn fp_pair(word: u32) -> Result<Inst, String> {
    let load = bit(word, 22);
    let (double, scale) = match bits(word, 31, 30) {
        0b00 => (false, 4),
        0b01 => (true, 8),
        0b10 => (true, 16),
        _ => return Err(outside(word)),
    };
    let offset = signed(bits(word, 21, 15), 7) * scale;
    let base = A64Reg::new(bits(word, 9, 5), true, true);
    let register = |number: u32| -> Result<FpReg, String> {
        Ok(FpReg {
            number: u8::try_from(number).map_err(|_| outside(word))?,
            double,
        })
    };
    let (rt, rt2) = (register(bits(word, 4, 0))?, register(bits(word, 14, 10))?);
    let address = match bits(word, 24, 23) {
        0b01 => Address::PostIndex { base, offset },
        0b10 => Address::Offset { base, offset },
        0b11 => Address::PreIndex { base, offset },
        // Pairs that hint the cache isn't needed.
        _ => return Err(outside(word)),
    };
    if scale == 16 {
        return vector::q_pair(word, address);
    }
    // Two loads into one register have no defined result.
    if load && rt.number == rt2.number {
        return Err(outside(word));
    }
    Ok(Inst::FpPair {
        load,
        rt,
        rt2,
        address,
    })
}

/// Scalar floating point on singles and doubles, never halves: data processing with one, two or
/// three sources, compares, selects, immediates, and moves to and from general registers.
fn floating(word: u32) -> Result<Inst, String> {
    // A vector's two floats into a scalar: 01 U 11110 size 11000 opcode 10 Rn Rd.
    if bits(word, 31, 30) == 0b01
        && bits(word, 28, 24) == 0b1_1110
        && bits(word, 21, 17) == 0b1_1000
        && bits(word, 11, 10) == 0b10
    {
        return vector::pairwise(word);
    }
    // Vectors, and the forms that set flags or are unallocated.
    if bit(word, 30) || bit(word, 29) {
        return Err(outside(word));
    }
    let double = match bits(word, 23, 22) {
        0b00 => false,
        0b01 => true,
        _ => return Err(outside(word)),
    };
    let reg = |number: u32| FpReg {
        number: (number & 31) as u8,
        double,
    };
    let (rd, rn, rm) = (bits(word, 4, 0), bits(word, 9, 5), bits(word, 20, 16));
    if bits(word, 28, 24) == 0b1_1111 {
        if bit(word, 31) {
            return Err(outside(word));
        }
        let op = match (bit(word, 21), bit(word, 15)) {
            (false, false) => FpThree::Madd,
            (false, true) => FpThree::Msub,
            (true, false) => FpThree::Nmadd,
            (true, true) => FpThree::Nmsub,
        };
        return Ok(Inst::FpThreeSource {
            op,
            rd: reg(rd),
            rn: reg(rn),
            rm: reg(rm),
            ra: reg(bits(word, 14, 10)),
        });
    }
    // Conversions to and from fixed point have bit 21 clear.
    if bits(word, 28, 24) != 0b1_1110 || !bit(word, 21) {
        return Err(outside(word));
    }
    if bits(word, 15, 10) == 0 {
        return conversion(word, double);
    }
    // Only a conversion names a general register's width in bit 31.
    if bit(word, 31) {
        return Err(outside(word));
    }
    match bits(word, 11, 10) {
        0b10 => {
            let op = match bits(word, 15, 12) {
                0 => FpTwo::Mul,
                1 => FpTwo::Div,
                2 => FpTwo::Add,
                3 => FpTwo::Sub,
                4 => FpTwo::Max,
                5 => FpTwo::Min,
                6 => FpTwo::MaxNm,
                7 => FpTwo::MinNm,
                8 => FpTwo::Nmul,
                _ => return Err(outside(word)),
            };
            Ok(Inst::FpTwoSource {
                op,
                rd: reg(rd),
                rn: reg(rn),
                rm: reg(rm),
            })
        }
        0b11 => Ok(Inst::FpCondSelect {
            rd: reg(rd),
            rn: reg(rn),
            rm: reg(rm),
            cond: A64Cond::from_bits(bits(word, 15, 12)),
        }),
        // fccmp and fccmpe.
        0b01 => Err(outside(word)),
        _ => {
            if bit(word, 12) {
                // An immediate: its 5 bits below must be zero.
                if bits(word, 9, 5) != 0 {
                    return Err(outside(word));
                }
                Ok(Inst::FpMoveImm {
                    rd: reg(rd),
                    imm8: (bits(word, 20, 13) & 0xff) as u8,
                })
            } else if bit(word, 13) {
                if bits(word, 15, 14) != 0 {
                    return Err(outside(word));
                }
                let (zero, signaling) = match bits(word, 4, 0) {
                    0b0_0000 => (false, false),
                    0b0_1000 => (true, false),
                    0b1_0000 => (false, true),
                    0b1_1000 => (true, true),
                    _ => return Err(outside(word)),
                };
                // Compared with zero, the second register's field is ignored, as LLVM reads it.
                Ok(Inst::FpCompare {
                    rn: reg(rn),
                    rm: (!zero).then(|| reg(rm)),
                    signaling,
                })
            } else if bit(word, 14) {
                one_fp(word, double, reg(rd), reg(rn))
            } else {
                Err(outside(word))
            }
        }
    }
}

/// One floating-point source: the operation in bits 20 to 15.
fn one_fp(word: u32, double: bool, rd: FpReg, rn: FpReg) -> Result<Inst, String> {
    let op = match bits(word, 20, 15) {
        0 => FpOne::Mov,
        1 => FpOne::Abs,
        2 => FpOne::Neg,
        3 => FpOne::Sqrt,
        // To a single, from a double; to a double, from a single.
        4 if double => {
            return Ok(Inst::FpOneSource {
                op: FpOne::Cvt,
                rd: FpReg {
                    number: rd.number,
                    double: false,
                },
                rn,
            });
        }
        5 if !double => {
            return Ok(Inst::FpOneSource {
                op: FpOne::Cvt,
                rd: FpReg {
                    number: rd.number,
                    double: true,
                },
                rn,
            });
        }
        8 => FpOne::RintNearest,
        9 => FpOne::RintUp,
        10 => FpOne::RintDown,
        11 => FpOne::RintZero,
        12 => FpOne::RintAway,
        _ => return Err(outside(word)),
    };
    Ok(Inst::FpOneSource { op, rd, rn })
}

/// A move between a floating-point register and a general one: `sf` the general register's
/// width, `rmode` and `opcode` which.
fn conversion(word: u32, double: bool) -> Result<Inst, String> {
    let wide = bit(word, 31);
    let fp = |number: u32| FpReg {
        number: (number & 31) as u8,
        double,
    };
    let gpr = |number: u32| A64Reg::new(number, wide, false);
    let (rd, rn) = (bits(word, 4, 0), bits(word, 9, 5));
    let (op, fp_reg, gpr_reg) = match (bits(word, 20, 19), bits(word, 18, 16)) {
        (0b00, 0b010) => (FpConversion::Scvtf, fp(rd), gpr(rn)),
        (0b00, 0b011) => (FpConversion::Ucvtf, fp(rd), gpr(rn)),
        (0b11, 0b000) => (FpConversion::Fcvtzs, fp(rn), gpr(rd)),
        (0b11, 0b001) => (FpConversion::Fcvtzu, fp(rn), gpr(rd)),
        // The bits, between registers as wide: a single and a w register, a double and an x.
        (0b00, 0b110) if wide == double => (FpConversion::ToGeneral, fp(rn), gpr(rd)),
        (0b00, 0b111) if wide == double => (FpConversion::FromGeneral, fp(rd), gpr(rn)),
        _ => return Err(outside(word)),
    };
    Ok(Inst::FpConvert {
        op,
        fp: fp_reg,
        gpr: gpr_reg,
    })
}

/// A load or store of two registers: opc 101 0 mode L imm7 Rt2 Rn Rt.
fn pair(word: u32) -> Result<Inst, String> {
    let load = bit(word, 22);
    let (wide, signed_words, scale) = match (bits(word, 31, 30), load) {
        (0b00, _) => (false, false, 4),
        (0b01, true) => (true, true, 4),
        (0b10, _) => (true, false, 8),
        // stgp, and what's unallocated.
        _ => return Err(outside(word)),
    };
    let offset = signed(bits(word, 21, 15), 7) * scale;
    let base = A64Reg::new(bits(word, 9, 5), true, true);
    let rt = A64Reg::new(bits(word, 4, 0), wide, false);
    let rt2 = A64Reg::new(bits(word, 14, 10), wide, false);
    let address = match bits(word, 24, 23) {
        0b01 => Address::PostIndex { base, offset },
        0b10 => Address::Offset { base, offset },
        0b11 => Address::PreIndex { base, offset },
        // Pairs that hint the cache isn't needed.
        _ => return Err(outside(word)),
    };
    let moves = !matches!(address, Address::Offset { .. });
    // Two loads into one register, or a moving base also transferred, have no defined result.
    if (load && rt.gpr == rt2.gpr) || (moves && (base.gpr == rt.gpr || base.gpr == rt2.gpr)) {
        return Err(outside(word));
    }
    Ok(Inst::Pair {
        load,
        signed: signed_words,
        rt,
        rt2,
        address,
    })
}

/// Data processing on registers.
fn register(word: u32) -> Result<Inst, String> {
    let wide = bit(word, 31);
    let register = |high, low| A64Reg::new(bits(word, high, low), wide, false);
    let (rd, rn, rm) = (register(4, 0), register(9, 5), register(20, 16));
    let amount = bits(word, 15, 10);
    let in_range = |amount: u32| u8::try_from(amount).map_err(|_| outside(word));
    if !bit(word, 28) {
        if !bit(word, 24) {
            // Logical (shifted register): sf opc 01010 shift N Rm imm6 Rn Rd.
            if !wide && amount > 31 {
                return Err(outside(word));
            }
            return Ok(Inst::LogicalShifted {
                op: logic(bits(word, 30, 29)),
                invert: bit(word, 21),
                rd,
                rn,
                rm,
                shift: Shift::from_bits(bits(word, 23, 22)),
                amount: in_range(amount)?,
            });
        }
        if !bit(word, 21) {
            // add/sub (shifted register): sf op S 01011 shift 0 Rm imm6 Rn Rd.
            let shift = bits(word, 23, 22);
            if shift == 3 || (!wide && amount > 31) {
                return Err(outside(word));
            }
            return Ok(Inst::AddShifted {
                sub: bit(word, 30),
                flags: bit(word, 29),
                rd,
                rn,
                rm,
                shift: Shift::from_bits(shift),
                amount: in_range(amount)?,
            });
        }
        // add/sub with an extended register.
        return Err(outside(word));
    }
    match bits(word, 24, 21) {
        0b0100 => {
            // Conditional select: sf op 0 11010100 Rm cond 0 o2 Rn Rd.
            if bit(word, 29) || bit(word, 11) {
                return Err(outside(word));
            }
            let op = match (bit(word, 30), bit(word, 10)) {
                (false, false) => Select::Csel,
                (false, true) => Select::Csinc,
                (true, false) => Select::Csinv,
                (true, true) => Select::Csneg,
            };
            Ok(Inst::CondSelect {
                op,
                rd,
                rn,
                rm,
                cond: A64Cond::from_bits(bits(word, 15, 12)),
            })
        }
        0b0110 if !bit(word, 29) && bit(word, 30) => {
            // One source: sf 1 0 11010110 00000 opcode Rn Rd.
            if bits(word, 20, 16) != 0 {
                return Err(outside(word));
            }
            let op = match (amount, wide) {
                (0, _) => One::Rbit,
                (1, _) => One::Rev16,
                (2, false) | (3, true) => One::Rev,
                (2, true) => One::Rev32,
                (4, _) => One::Clz,
                (5, _) => One::Cls,
                _ => return Err(outside(word)),
            };
            Ok(Inst::OneSource { op, rd, rn })
        }
        0b0110 if !bit(word, 29) => {
            // Two sources: sf 0 0 11010110 Rm opcode Rn Rd.
            let op = match amount {
                0b00_0010 => Two::Udiv,
                0b00_0011 => Two::Sdiv,
                0b00_1000 => Two::Lslv,
                0b00_1001 => Two::Lsrv,
                0b00_1010 => Two::Asrv,
                0b00_1011 => Two::Rorv,
                _ => return Err(outside(word)),
            };
            Ok(Inst::TwoSource { op, rd, rn, rm })
        }
        0b1000..=0b1111 if bits(word, 30, 29) == 0 => {
            // Three sources: sf 00 11011 op31 Rm o0 Ra Rn Rd.
            let ra = register(14, 10);
            let (high, lengthens) = (ra.gpr == super::Gpr::Zr, wide);
            let op = match (bits(word, 23, 21), bit(word, 15)) {
                (0, false) => Three::Madd,
                (0, true) => Three::Msub,
                (1, false) if lengthens => Three::Smaddl,
                (1, true) if lengthens => Three::Smsubl,
                (2, false) if lengthens && high => Three::Smulh,
                (5, false) if lengthens => Three::Umaddl,
                (5, true) if lengthens => Three::Umsubl,
                (6, false) if lengthens && high => Three::Umulh,
                _ => return Err(outside(word)),
            };
            // The lengthening products take 32-bit factors.
            let (rn, rm) = match op {
                Three::Smaddl | Three::Smsubl | Three::Umaddl | Three::Umsubl => (
                    A64Reg::new(bits(word, 9, 5), false, false),
                    A64Reg::new(bits(word, 20, 16), false, false),
                ),
                _ => (rn, rm),
            };
            Ok(Inst::ThreeSource { op, rd, rn, rm, ra })
        }
        // With carry, conditional compares, flag manipulation, and what's newer.
        _ => Err(outside(word)),
    }
}
