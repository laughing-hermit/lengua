//! AArch64: the dialect's instructions in one canonical form, which lengua reads from machine
//! words ([`decode`]) and from main.s's text ([`text`]) alike, so main.s and its machine code can
//! be held to each other field by field; each instruction's facts for the rules; and AAPCS64, the
//! calling convention lengua's AArch64 programs follow.
//!
//! The canonical form is the architecture's own instruction, never an assembler's alias: `mov x0,
//! #5` is `movz x0, #5`, `cmp x1, #3` is `subs xzr, x1, #3`, `lsl x0, x1, #3` is `ubfm x0, x1,
//! #61, #60`. Only the dialect's subset has a form at all: anything else is refused, by
//! construction.

pub mod decode;
pub mod facts;
pub mod text;
pub mod vector;

pub use vector::{
    Arrangement, Expand, Lane, Scalar, Source, VAcross, VImmediate, VPermute, VReg, VShift, VThree,
    VTwo, Writeback,
};

#[cfg(test)]
mod tests;

use std::fmt;

use super::{Abi, Reg};

/// One of the 31 general-purpose registers, or what register number 31 means where it's named:
/// the stack pointer or the zero register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Gpr {
    /// x0 to x30.
    R(u8),
    /// The stack pointer.
    Sp,
    /// The zero register: reads as zero, and writes go nowhere.
    Zr,
}

/// A floating-point register at the width an instruction names it: `s` (32 bits) or `d` (64),
/// the low part of the vector register of its number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FpReg {
    /// Which of the 32, v0 to v31.
    pub number: u8,
    /// Whether it's named as a double, `d`, rather than a single, `s`.
    pub double: bool,
}

/// A floating-point operation of one source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FpOne {
    /// `fmov`: a copy.
    Mov,
    /// `fabs`.
    Abs,
    /// `fneg`.
    Neg,
    /// `fsqrt`.
    Sqrt,
    /// `fcvt`: to the other width, `rd`'s.
    Cvt,
    /// `frintn`: to the nearest whole number, ties to even.
    RintNearest,
    /// `frintp`: up.
    RintUp,
    /// `frintm`: down.
    RintDown,
    /// `frintz`: toward zero.
    RintZero,
    /// `frinta`: to the nearest, ties away from zero.
    RintAway,
}

/// A floating-point operation of two sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FpTwo {
    /// `fmul`.
    Mul,
    /// `fdiv`.
    Div,
    /// `fadd`.
    Add,
    /// `fsub`.
    Sub,
    /// `fmax`.
    Max,
    /// `fmin`.
    Min,
    /// `fmaxnm`: a number over a NaN.
    MaxNm,
    /// `fminnm`.
    MinNm,
    /// `fnmul`: the product negated.
    Nmul,
}

/// A fused multiply and add, rounded once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FpThree {
    /// `fmadd`: `ra + rn × rm`.
    Madd,
    /// `fmsub`: `ra - rn × rm`.
    Msub,
    /// `fnmadd`: `-ra - rn × rm`.
    Nmadd,
    /// `fnmsub`: `-ra + rn × rm`.
    Nmsub,
}

/// A move between a floating-point register and a general one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FpConversion {
    /// `scvtf`: a signed whole number to floating point.
    Scvtf,
    /// `ucvtf`: an unsigned one.
    Ucvtf,
    /// `fcvtzs`: to a signed whole number, toward zero, saturating.
    Fcvtzs,
    /// `fcvtzu`: to an unsigned one.
    Fcvtzu,
    /// `fmov` of the bits to the general register.
    ToGeneral,
    /// `fmov` of the bits from the general register.
    FromGeneral,
}

/// A general-purpose register at the width an instruction names it: `x` (64 bits) or `w` (32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct A64Reg {
    /// Which register.
    pub gpr: Gpr,
    /// Whether it's named whole, `x`, rather than its low half, `w`.
    pub wide: bool,
}

impl A64Reg {
    /// `x<number>` or `w<number>`, or register 31 read as `zero` (the zero register) or the stack
    /// pointer.
    #[must_use]
    pub const fn new(number: u32, wide: bool, sp: bool) -> Self {
        let gpr = match number {
            31 if sp => Gpr::Sp,
            31 => Gpr::Zr,
            // Five bits: 0 to 30 here.
            number => Gpr::R(number as u8),
        };
        Self { gpr, wide }
    }

    /// The register whole: `x5` for `w5`.
    #[must_use]
    pub const fn full(self) -> Self {
        Self {
            gpr: self.gpr,
            wide: true,
        }
    }
}

impl fmt::Display for FpReg {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = if self.double { 'd' } else { 's' };
        write!(formatter, "{kind}{}", self.number)
    }
}

/// The number `fmov`'s 8 bits encode: a sign, an exponent from -3 to 4 and four bits of fraction,
/// `(-1)^s × (16 + fraction) / 16 × 2^exponent`, as the architecture's `VFPExpandImm` reads them.
#[must_use]
pub fn fp_immediate(imm8: u8) -> f64 {
    let sign = if imm8 & 0x80 == 0 { 1.0 } else { -1.0 };
    let exponent = i32::from((imm8 >> 4) & 7 ^ 4) - 3;
    let fraction = f64::from(16 + u32::from(imm8 & 0x0f)) / 16.0;
    sign * fraction * 2_f64.powi(exponent)
}

impl fmt::Display for A64Reg {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.gpr, self.wide) {
            (Gpr::R(number), true) => write!(formatter, "x{number}"),
            (Gpr::R(number), false) => write!(formatter, "w{number}"),
            (Gpr::Sp, true) => formatter.write_str("sp"),
            (Gpr::Sp, false) => formatter.write_str("wsp"),
            (Gpr::Zr, true) => formatter.write_str("xzr"),
            (Gpr::Zr, false) => formatter.write_str("wzr"),
        }
    }
}

/// `register`'s place in a [`super::RegSet`]: x0 to x30 are 0 to 30, the stack pointer 31; the
/// zero register holds nothing and has none.
#[must_use]
pub fn slot(register: A64Reg) -> Option<u32> {
    match register.gpr {
        Gpr::R(number) => Some(u32::from(number)),
        Gpr::Sp => Some(31),
        Gpr::Zr => None,
    }
}

const fn x(number: u32) -> Reg {
    Reg::A64(A64Reg::new(number, true, false))
}

const fn d(number: u8) -> Reg {
    Reg::A64Fp(FpReg {
        number,
        double: true,
    })
}

/// AAPCS64 on Linux, as lengua's AArch64 programs follow it.
pub const ABI: Abi = Abi {
    registers: &[
        x(0),
        x(1),
        x(2),
        x(3),
        x(4),
        x(5),
        x(6),
        x(7),
        x(8),
        x(9),
        x(10),
        x(11),
        x(12),
        x(13),
        x(14),
        x(15),
        x(16),
        x(17),
        x(18),
        x(19),
        x(20),
        x(21),
        x(22),
        x(23),
        x(24),
        x(25),
        x(26),
        x(27),
        x(28),
        x(29),
        x(30),
        Reg::A64(A64Reg::new(31, true, true)),
    ],
    stack_pointer: Reg::A64(A64Reg::new(31, true, true)),
    // A call keeps its return address in x30, not on the stack.
    return_address: 0,
    syscall_number: x(8),
    syscall_arguments: &[x(0), x(1), x(2), x(3), x(4), x(5)],
    arguments: &[x(0), x(1), x(2), x(3), x(4), x(5), x(6), x(7)],
    result: x(0),
    returns: [x(0), x(1)],
    first_argument: x(0),
    assertion_number: Reg::A64(A64Reg::new(0, false, false)),
    worker_slot: x(19),
    link: Some(x(30)),
    // x18 is the platform register, which the dialect doesn't use; a call changes x30, the link
    // register, too, and the floating-point registers but d8 to d15.
    caller_saved: &[
        x(0),
        x(1),
        x(2),
        x(3),
        x(4),
        x(5),
        x(6),
        x(7),
        x(8),
        x(9),
        x(10),
        x(11),
        x(12),
        x(13),
        x(14),
        x(15),
        x(16),
        x(17),
        x(18),
        x(30),
        d(0),
        d(1),
        d(2),
        d(3),
        d(4),
        d(5),
        d(6),
        d(7),
        d(16),
        d(17),
        d(18),
        d(19),
        d(20),
        d(21),
        d(22),
        d(23),
        d(24),
        d(25),
        d(26),
        d(27),
        d(28),
        d(29),
        d(30),
        d(31),
    ],
    syscall_clobbers: &[x(0)],
    callee_saved: &[
        x(19),
        x(20),
        x(21),
        x(22),
        x(23),
        x(24),
        x(25),
        x(26),
        x(27),
        x(28),
        x(29),
        // The low 64 bits of v8 to v15, all a scalar uses of them.
        d(8),
        d(9),
        d(10),
        d(11),
        d(12),
        d(13),
        d(14),
        d(15),
    ],
};

/// A condition the flags are tested for, as AArch64 numbers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A64Cond {
    /// Equal (zero).
    Eq,
    /// Not equal.
    Ne,
    /// Carry set: unsigned higher or same.
    Hs,
    /// Carry clear: unsigned lower.
    Lo,
    /// Negative.
    Mi,
    /// Positive or zero.
    Pl,
    /// Signed overflow.
    Vs,
    /// No signed overflow.
    Vc,
    /// Unsigned higher.
    Hi,
    /// Unsigned lower or same.
    Ls,
    /// Signed greater or equal.
    Ge,
    /// Signed less.
    Lt,
    /// Signed greater.
    Gt,
    /// Signed less or equal.
    Le,
    /// Always.
    Al,
    /// Always, the encoding the architecture keeps for it.
    Nv,
}

impl A64Cond {
    /// The condition with this encoding.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        match bits & 0xf {
            0 => Self::Eq,
            1 => Self::Ne,
            2 => Self::Hs,
            3 => Self::Lo,
            4 => Self::Mi,
            5 => Self::Pl,
            6 => Self::Vs,
            7 => Self::Vc,
            8 => Self::Hi,
            9 => Self::Ls,
            10 => Self::Ge,
            11 => Self::Lt,
            12 => Self::Gt,
            13 => Self::Le,
            14 => Self::Al,
            _ => Self::Nv,
        }
    }

    /// Its name in main.s: `eq`, `hs`, …
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Hs => "hs",
            Self::Lo => "lo",
            Self::Mi => "mi",
            Self::Pl => "pl",
            Self::Vs => "vs",
            Self::Vc => "vc",
            Self::Hi => "hi",
            Self::Ls => "ls",
            Self::Ge => "ge",
            Self::Lt => "lt",
            Self::Gt => "gt",
            Self::Le => "le",
            Self::Al => "al",
            Self::Nv => "nv",
        }
    }

    /// The condition by its name, the assemblers' second names included (`cs` for `hs`, `cc`
    /// for `lo`).
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        let all = [
            Self::Eq,
            Self::Ne,
            Self::Hs,
            Self::Lo,
            Self::Mi,
            Self::Pl,
            Self::Vs,
            Self::Vc,
            Self::Hi,
            Self::Ls,
            Self::Ge,
            Self::Lt,
            Self::Gt,
            Self::Le,
            Self::Al,
            Self::Nv,
        ];
        match name {
            "cs" => Some(Self::Hs),
            "cc" => Some(Self::Lo),
            _ => all.into_iter().find(|cond| cond.name() == name),
        }
    }

    /// The opposite condition: the one that holds exactly when this doesn't.
    #[must_use]
    pub const fn invert(self) -> Self {
        match self {
            Self::Eq => Self::Ne,
            Self::Ne => Self::Eq,
            Self::Hs => Self::Lo,
            Self::Lo => Self::Hs,
            Self::Mi => Self::Pl,
            Self::Pl => Self::Mi,
            Self::Vs => Self::Vc,
            Self::Vc => Self::Vs,
            Self::Hi => Self::Ls,
            Self::Ls => Self::Hi,
            Self::Ge => Self::Lt,
            Self::Lt => Self::Ge,
            Self::Gt => Self::Le,
            Self::Le => Self::Gt,
            Self::Al => Self::Nv,
            Self::Nv => Self::Al,
        }
    }
}

/// How a register operand is shifted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shift {
    /// Left.
    Lsl,
    /// Right, filling with zeros.
    Lsr,
    /// Right, filling with the sign.
    Asr,
    /// Rotated right.
    Ror,
}

impl Shift {
    /// The shift with this encoding.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        match bits & 3 {
            0 => Self::Lsl,
            1 => Self::Lsr,
            2 => Self::Asr,
            _ => Self::Ror,
        }
    }

    /// Its name in main.s.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Lsl => "lsl",
            Self::Lsr => "lsr",
            Self::Asr => "asr",
            Self::Ror => "ror",
        }
    }
}

/// How a register offset in an address is widened before it's added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extend {
    /// The low 32 bits, zero-extended: `uxtw`.
    Uxtw,
    /// All 64 bits: `lsl` (or `uxtx`).
    Lsl,
    /// The low 32 bits, sign-extended: `sxtw`.
    Sxtw,
    /// All 64 bits, as signed: `sxtx`.
    Sxtx,
}

/// The logical operations, before any inversion of the second operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Logic {
    /// `and`.
    And,
    /// `orr`.
    Orr,
    /// `eor`.
    Eor,
    /// `ands`, which sets the flags.
    Ands,
}

/// The move-wide operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wide {
    /// `movn`: the inverse of the shifted number.
    Movn,
    /// `movz`: the shifted number, zeros elsewhere.
    Movz,
    /// `movk`: the number into 16 bits, keeping the rest.
    Movk,
}

/// The bitfield operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// `sbfm`: a field sign-extended.
    Sbfm,
    /// `bfm`: a field inserted, the rest kept.
    Bfm,
    /// `ubfm`: a field zero-extended.
    Ubfm,
}

/// The branches through a register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indirect {
    /// `br`: a jump.
    Br,
    /// `blr`: a call.
    Blr,
    /// `ret`: a return.
    Ret,
}

/// The conditional selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Select {
    /// `csel`: the first, or the second.
    Csel,
    /// `csinc`: the first, or the second plus one.
    Csinc,
    /// `csinv`: the first, or the second inverted.
    Csinv,
    /// `csneg`: the first, or the second negated.
    Csneg,
}

/// The two-source operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Two {
    /// Unsigned division.
    Udiv,
    /// Signed division.
    Sdiv,
    /// Shift left by a register.
    Lslv,
    /// Shift right, unsigned, by a register.
    Lsrv,
    /// Shift right, signed, by a register.
    Asrv,
    /// Rotate right by a register.
    Rorv,
}

/// The one-source operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum One {
    /// Bits in reverse order.
    Rbit,
    /// Bytes reversed within each 16 bits.
    Rev16,
    /// Bytes reversed within each 32 bits, of a 64-bit register.
    Rev32,
    /// Bytes reversed.
    Rev,
    /// Leading zeros counted.
    Clz,
    /// Leading sign bits counted.
    Cls,
}

/// The three-source operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Three {
    /// `ra + rn × rm`.
    Madd,
    /// `ra − rn × rm`.
    Msub,
    /// `ra + rn × rm`, from 32-bit signed numbers.
    Smaddl,
    /// `ra − rn × rm`, from 32-bit signed numbers.
    Smsubl,
    /// `ra + rn × rm`, from 32-bit unsigned numbers.
    Umaddl,
    /// `ra − rn × rm`, from 32-bit unsigned numbers.
    Umsubl,
    /// The high 64 bits of the signed product.
    Smulh,
    /// The high 64 bits of the unsigned product.
    Umulh,
}

/// Where a load or store's address comes from, and whether the base register moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Address {
    /// `[base, #offset]`: the base, unchanged.
    Offset {
        /// The base register.
        base: A64Reg,
        /// The bytes added.
        offset: i64,
    },
    /// `[base, #offset]!`: the base moves by the offset, then is the address.
    PreIndex {
        /// The base register.
        base: A64Reg,
        /// The bytes it moves.
        offset: i64,
    },
    /// `[base], #offset`: the base is the address, then moves by the offset.
    PostIndex {
        /// The base register.
        base: A64Reg,
        /// The bytes it moves.
        offset: i64,
    },
    /// `[base, index{, extend #shift}]`: the base plus the index, widened and shifted.
    Register {
        /// The base register.
        base: A64Reg,
        /// The index register.
        index: A64Reg,
        /// How the index is widened.
        extend: Extend,
        /// How far it's shifted left: 0, or the log of the access's width.
        shift: u8,
    },
}

/// A load or store of one register: which, and how wide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Access {
    /// A load, or a store.
    pub load: bool,
    /// The bytes it moves: 1, 2, 4 or 8.
    pub size: u8,
    /// Whether a load sign-extends what it reads.
    pub signed: bool,
    /// `ldar`: a load-acquire, which later loads can't pass, so a thread's id read cleared by
    /// the kernel comes before the result the thread left. Only of a whole `w` or `x`, from a
    /// base with no offset.
    pub acquire: bool,
}

/// Whether a `pmull`'s operand is doublewords, the PMULL extension's form, rather than bytes.
#[must_use]
pub const fn pmull_wide(operand: VReg) -> bool {
    matches!(operand.arrangement, Arrangement::D1 | Arrangement::D2)
}

/// Whether a `pmull`'s operand is its register's upper half: `pmull2`.
#[must_use]
pub const fn pmull_upper(operand: VReg) -> bool {
    matches!(operand.arrangement, Arrangement::B16 | Arrangement::D2)
}

/// One AArch64 instruction of the dialect's subset, in its canonical form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inst {
    /// `add`/`sub`/`adds`/`subs` with a 12-bit number, shifted left by 12 or not.
    AddImm {
        /// A subtraction.
        sub: bool,
        /// Whether it sets the flags.
        flags: bool,
        /// Where the result goes.
        rd: A64Reg,
        /// The first operand.
        rn: A64Reg,
        /// The number.
        imm: u32,
        /// Whether the number is shifted left by 12.
        shift12: bool,
    },
    /// `add`/`sub`/`adds`/`subs` with a shifted register.
    AddShifted {
        /// A subtraction.
        sub: bool,
        /// Whether it sets the flags.
        flags: bool,
        /// Where the result goes.
        rd: A64Reg,
        /// The first operand.
        rn: A64Reg,
        /// The second operand, before its shift.
        rm: A64Reg,
        /// Its shift.
        shift: Shift,
        /// By how many bits.
        amount: u8,
    },
    /// `and`/`orr`/`eor`/`ands` with a bitmask number.
    LogicalImm {
        /// The operation.
        op: Logic,
        /// Where the result goes.
        rd: A64Reg,
        /// The first operand.
        rn: A64Reg,
        /// The bitmask, at the register's width.
        imm: u64,
    },
    /// `and`/`bic`/`orr`/`orn`/`eor`/`eon`/`ands`/`bics` with a shifted register.
    LogicalShifted {
        /// The operation.
        op: Logic,
        /// Whether the second operand is inverted first: `bic`, `orn`, `eon`, `bics`.
        invert: bool,
        /// Where the result goes.
        rd: A64Reg,
        /// The first operand.
        rn: A64Reg,
        /// The second operand, before its shift.
        rm: A64Reg,
        /// Its shift.
        shift: Shift,
        /// By how many bits.
        amount: u8,
    },
    /// `movn`/`movz`/`movk`.
    MoveWide {
        /// The operation.
        op: Wide,
        /// Where the number goes.
        rd: A64Reg,
        /// The 16 bits.
        imm: u16,
        /// How far they're shifted left: 0, 16, 32 or 48.
        shift: u8,
    },
    /// `sbfm`/`bfm`/`ubfm`.
    Bitfield {
        /// The operation.
        op: Field,
        /// Where the result goes.
        rd: A64Reg,
        /// The source.
        rn: A64Reg,
        /// The rotation.
        immr: u8,
        /// The field's last bit.
        imms: u8,
    },
    /// `extr`: bits of `rn:rm` from bit `lsb`.
    Extract {
        /// Where the result goes.
        rd: A64Reg,
        /// The high half.
        rn: A64Reg,
        /// The low half.
        rm: A64Reg,
        /// The lowest bit taken.
        lsb: u8,
    },
    /// `adrp`: the address of a 4 KiB page.
    Adrp {
        /// Where the address goes.
        rd: A64Reg,
        /// The page's address.
        page: u64,
    },
    /// `adr`, outside the dialect: decoded so the checker can say so.
    Adr {
        /// Where the address goes.
        rd: A64Reg,
        /// The address.
        target: u64,
    },
    /// `b`/`bl`.
    Branch {
        /// A call, keeping the return address in x30.
        link: bool,
        /// Where it goes.
        target: u64,
    },
    /// `b.cond`.
    BranchCond {
        /// When it jumps.
        cond: A64Cond,
        /// Where.
        target: u64,
    },
    /// `cbz`/`cbnz`.
    CompareBranch {
        /// Whether it jumps when the register isn't zero.
        nonzero: bool,
        /// The register.
        rt: A64Reg,
        /// Where it goes.
        target: u64,
    },
    /// `tbz`/`tbnz`.
    TestBranch {
        /// Whether it jumps when the bit is set.
        nonzero: bool,
        /// The register.
        rt: A64Reg,
        /// The bit.
        bit: u8,
        /// Where it goes.
        target: u64,
    },
    /// `br`/`blr`/`ret`.
    BranchReg {
        /// Which.
        kind: Indirect,
        /// The register holding the address.
        rn: A64Reg,
    },
    /// `svc`.
    Svc {
        /// The number in the instruction.
        imm: u16,
    },
    /// `csel`/`csinc`/`csinv`/`csneg`.
    CondSelect {
        /// The operation.
        op: Select,
        /// Where the result goes.
        rd: A64Reg,
        /// What it is when the condition holds.
        rn: A64Reg,
        /// What it's made from when it doesn't.
        rm: A64Reg,
        /// The condition.
        cond: A64Cond,
    },
    /// `crc32b` to `crc32x`, and `crc32cb` to `crc32cx`: a CRC-32 state in a `w` register, taking
    /// `size` bytes of data from the lowest, one CRC step each, in the IEEE polynomial or, with
    /// `castagnoli`, CRC-32C's.
    Crc {
        /// CRC-32C's polynomial, as x86-64's `crc32` uses, rather than the IEEE one.
        castagnoli: bool,
        /// How many bytes of data: 1, 2, 4 or 8.
        size: u8,
        /// Where the new state goes.
        rd: A64Reg,
        /// The state.
        rn: A64Reg,
        /// The data: a `w` register, or an `x` register for 8 bytes.
        rm: A64Reg,
    },
    /// `udiv`/`sdiv`/`lslv`/`lsrv`/`asrv`/`rorv`.
    TwoSource {
        /// The operation.
        op: Two,
        /// Where the result goes.
        rd: A64Reg,
        /// The first operand.
        rn: A64Reg,
        /// The second.
        rm: A64Reg,
    },
    /// `rbit`/`rev16`/`rev32`/`rev`/`clz`/`cls`.
    OneSource {
        /// The operation.
        op: One,
        /// Where the result goes.
        rd: A64Reg,
        /// The operand.
        rn: A64Reg,
    },
    /// `madd`/`msub`/`smaddl`/`smsubl`/`umaddl`/`umsubl`/`smulh`/`umulh`.
    ThreeSource {
        /// The operation.
        op: Three,
        /// Where the result goes.
        rd: A64Reg,
        /// The first factor.
        rn: A64Reg,
        /// The second.
        rm: A64Reg,
        /// What's added or subtracted (the zero register for `smulh`/`umulh`).
        ra: A64Reg,
    },
    /// A load or store of one register.
    LoadStore {
        /// Which, and how wide.
        access: Access,
        /// The register loaded or stored.
        rt: A64Reg,
        /// The address.
        address: Address,
    },
    /// `ldp`/`stp`/`ldpsw`: two registers, next to each other in memory.
    Pair {
        /// A load, or a store.
        load: bool,
        /// Whether a load sign-extends 32-bit words: `ldpsw`.
        signed: bool,
        /// The first register, at the address.
        rt: A64Reg,
        /// The second, right after it.
        rt2: A64Reg,
        /// The address: the base plus an offset, the base moving before or after where it
        /// writes back.
        address: Address,
    },
    /// `fmov`/`fabs`/`fneg`/`fsqrt`/`fcvt`/`frint*`: one floating-point source.
    FpOneSource {
        /// The operation.
        op: FpOne,
        /// Where the result goes.
        rd: FpReg,
        /// The operand.
        rn: FpReg,
    },
    /// `fmul`/`fdiv`/`fadd`/`fsub`/`fmax`/`fmin`/`fmaxnm`/`fminnm`/`fnmul`.
    FpTwoSource {
        /// The operation.
        op: FpTwo,
        /// Where the result goes.
        rd: FpReg,
        /// The first operand.
        rn: FpReg,
        /// The second.
        rm: FpReg,
    },
    /// `fmadd`/`fmsub`/`fnmadd`/`fnmsub`.
    FpThreeSource {
        /// The operation.
        op: FpThree,
        /// Where the result goes.
        rd: FpReg,
        /// The first factor.
        rn: FpReg,
        /// The second.
        rm: FpReg,
        /// What's added.
        ra: FpReg,
    },
    /// `fcmp`/`fcmpe`, with a register or zero.
    FpCompare {
        /// The first operand.
        rn: FpReg,
        /// The second, or none for `#0.0`.
        rm: Option<FpReg>,
        /// `fcmpe`: a quiet NaN signals too.
        signaling: bool,
    },
    /// `fcsel`.
    FpCondSelect {
        /// Where the result goes.
        rd: FpReg,
        /// What it is when the condition holds.
        rn: FpReg,
        /// What it is when it doesn't.
        rm: FpReg,
        /// The condition.
        cond: A64Cond,
    },
    /// `fmov` of a number its 8 bits encode.
    FpMoveImm {
        /// Where it goes.
        rd: FpReg,
        /// The encoded number: sign, 3 bits of exponent, 4 of fraction.
        imm8: u8,
    },
    /// `scvtf`/`ucvtf`/`fcvtzs`/`fcvtzu`, and `fmov` of the bits between register files.
    FpConvert {
        /// The operation.
        op: FpConversion,
        /// The floating-point register.
        fp: FpReg,
        /// The general one.
        gpr: A64Reg,
    },
    /// A load or store of one floating-point register.
    FpLoadStore {
        /// A load, or a store.
        load: bool,
        /// The register loaded or stored.
        rt: FpReg,
        /// The address.
        address: Address,
    },
    /// `ldp`/`stp` of two floating-point registers, next to each other in memory.
    FpPair {
        /// A load, or a store.
        load: bool,
        /// The first register, at the address.
        rt: FpReg,
        /// The second, right after it.
        rt2: FpReg,
        /// The address.
        address: Address,
    },
    /// The lanes of two vectors, each with its own: `add v0.16b, v1.16b, v2.16b`; `mov v0.16b,
    /// v1.16b` is `orr` of one register with itself.
    VectorThree {
        /// The operation.
        op: VThree,
        /// Where the result goes.
        rd: VReg,
        /// The first operand.
        rn: VReg,
        /// The second.
        rm: VReg,
    },
    /// The carry-less products of two vectors' lanes, polynomials over GF(2): `pmull v0.8h,
    /// v1.8b, v2.8b`, each pair of bytes' product in a halfword, or `pmull v0.1q, v1.1d, v2.1d`,
    /// the doublewords' product in all 128 bits (the PMULL extension). `pmull2` takes the upper
    /// half of each operand, its arrangement `16b` or `2d`.
    Pmull {
        /// Where the products go, the whole V register.
        rd: u8,
        /// The first operand: bytes (`8b`, `16b`) or doublewords (`1d`, `2d`).
        rn: VReg,
        /// The second, arranged as the first.
        rm: VReg,
    },
    /// One vector's lanes: `cnt v0.16b, v1.16b`, the comparisons with zero, and the widening and
    /// narrowing ones, whose two arrangements differ.
    VectorTwo {
        /// The operation.
        op: VTwo,
        /// Where the result goes.
        rd: VReg,
        /// The operand.
        rn: VReg,
    },
    /// A vector's lanes into one scalar: `addv b0, v1.16b`.
    VectorAcross {
        /// The operation.
        op: VAcross,
        /// Where the result goes.
        rd: Scalar,
        /// The vector.
        rn: VReg,
    },
    /// Each lane shifted by a number: `ushr v0.16b, v1.16b, #3`.
    VectorShift {
        /// The operation.
        op: VShift,
        /// Where the result goes.
        rd: VReg,
        /// The vector shifted.
        rn: VReg,
        /// By how many bits.
        amount: u8,
    },
    /// Lanes of two vectors, interleaved or picked: `zip1 v0.16b, v1.16b, v2.16b`.
    VectorPermute {
        /// The operation.
        op: VPermute,
        /// Where the result goes.
        rd: VReg,
        /// The first vector.
        rn: VReg,
        /// The second.
        rm: VReg,
    },
    /// The bytes of two vectors joined, from byte `index` of the first up: `ext v0.16b, v1.16b,
    /// v2.16b, #3`.
    VectorExtract {
        /// Where the result goes.
        rd: VReg,
        /// The low part.
        rn: VReg,
        /// The high part.
        rm: VReg,
        /// The first byte taken.
        index: u8,
    },
    /// Each byte of `rm` looked up in a table of 1 to 4 vectors: `tbl`, 0 where it's past the
    /// table, or `tbx`, which keeps the destination's byte there.
    VectorTable {
        /// `tbx`.
        keep: bool,
        /// Where the result goes.
        rd: VReg,
        /// The table's first register; the rest follow it, wrapping from v31 to v0.
        table: u8,
        /// How many registers the table has.
        count: u8,
        /// The indexes.
        rm: VReg,
    },
    /// A lane or a general register in every lane: `dup v0.16b, v1.b[3]`, `dup v0.4s, w1`.
    VectorDup {
        /// Where it goes.
        rd: VReg,
        /// What's copied.
        from: Source,
    },
    /// A lane into a general register, widened without a sign or with one: `umov w0, v1.b[3]`,
    /// which LLVM writes `mov` for a word or a doubleword, and `smov`.
    VectorToGeneral {
        /// `smov`.
        signed: bool,
        /// Where it goes.
        rd: A64Reg,
        /// The lane.
        rn: Lane,
    },
    /// A general register or a lane into one lane, the others kept: `mov v0.b[3], w1`.
    VectorInsert {
        /// The lane written.
        rd: Lane,
        /// What's copied.
        from: Source,
    },
    /// A number in every lane, `movi` and `mvni`, or `orr` and `bic` of it with every lane.
    VectorImmediate {
        /// The operation.
        op: VImmediate,
        /// The vector; a `1d` one is `movi d0, #…`.
        rd: VReg,
        /// The 8 bits the instruction holds.
        imm8: u8,
        /// How they become each lane's number.
        expand: Expand,
    },
    /// A load or store of one whole q register.
    VectorLoadStore {
        /// A load, or a store.
        load: bool,
        /// The register, q0 to q31.
        rt: u8,
        /// The address.
        address: Address,
    },
    /// `ldp`/`stp` of two q registers, next to each other in memory.
    VectorPair {
        /// A load, or a store.
        load: bool,
        /// The first register, at the address.
        rt: u8,
        /// The second, 16 bytes after it.
        rt2: u8,
        /// The address.
        address: Address,
    },
    /// `ld1`/`st1` of 1 to 4 vectors one after another in memory: `ld1 { v0.16b, v1.16b },
    /// [x1], #32`.
    VectorStructures {
        /// A load, or a store.
        load: bool,
        /// The first register; the rest follow it, wrapping from v31 to v0.
        first: u8,
        /// How many registers.
        count: u8,
        /// Their lanes, the same for all.
        arrangement: Arrangement,
        /// The address.
        base: A64Reg,
        /// How the base moves after, if it does.
        step: Option<Writeback>,
    },
    /// `nop`.
    Nop,
}

impl fmt::Display for Address {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Offset { base, offset: 0 } => write!(formatter, "[{base}]"),
            Self::Offset { base, offset } => write!(formatter, "[{base}, #{offset}]"),
            Self::PreIndex { base, offset } => write!(formatter, "[{base}, #{offset}]!"),
            Self::PostIndex { base, offset } => write!(formatter, "[{base}], #{offset}"),
            Self::Register {
                base,
                index,
                extend,
                shift,
            } => {
                let name = match extend {
                    Extend::Uxtw => "uxtw",
                    Extend::Lsl => "lsl",
                    Extend::Sxtw => "sxtw",
                    Extend::Sxtx => "sxtx",
                };
                match (extend, shift) {
                    (Extend::Lsl, 0) => write!(formatter, "[{base}, {index}]"),
                    (_, 0) => write!(formatter, "[{base}, {index}, {name}]"),
                    _ => write!(formatter, "[{base}, {index}, {name} #{shift}]"),
                }
            }
        }
    }
}

impl fmt::Display for Inst {
    /// The canonical form, as LLVM would write it without its aliases.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = facts::mnemonic(self);
        let shifted = |shift: Shift, amount: u8| {
            if amount == 0 && shift == Shift::Lsl {
                String::new()
            } else {
                format!(", {} #{amount}", shift.name())
            }
        };
        match *self {
            Self::AddImm {
                rd,
                rn,
                imm,
                shift12,
                ..
            } => {
                let shift = if shift12 { ", lsl #12" } else { "" };
                write!(formatter, "{name} {rd}, {rn}, #{imm}{shift}")
            }
            Self::AddShifted {
                rd,
                rn,
                rm,
                shift,
                amount,
                ..
            }
            | Self::LogicalShifted {
                rd,
                rn,
                rm,
                shift,
                amount,
                ..
            } => write!(
                formatter,
                "{name} {rd}, {rn}, {rm}{}",
                shifted(shift, amount)
            ),
            Self::LogicalImm { rd, rn, imm, .. } => {
                write!(formatter, "{name} {rd}, {rn}, #{imm:#x}")
            }
            Self::MoveWide { rd, imm, shift, .. } => {
                let shift = if shift == 0 {
                    String::new()
                } else {
                    format!(", lsl #{shift}")
                };
                write!(formatter, "{name} {rd}, #{imm}{shift}")
            }
            Self::Bitfield {
                rd, rn, immr, imms, ..
            } => write!(formatter, "{name} {rd}, {rn}, #{immr}, #{imms}"),
            Self::Extract { rd, rn, rm, lsb } => {
                write!(formatter, "{name} {rd}, {rn}, {rm}, #{lsb}")
            }
            Self::Adrp { rd, page: target } | Self::Adr { rd, target } => {
                write!(formatter, "{name} {rd}, {target:#x}")
            }
            Self::Branch { target, .. } | Self::BranchCond { target, .. } => {
                write!(formatter, "{name} {target:#x}")
            }
            Self::CompareBranch { rt, target, .. } => write!(formatter, "{name} {rt}, {target:#x}"),
            Self::TestBranch {
                rt, bit, target, ..
            } => write!(formatter, "{name} {rt}, #{bit}, {target:#x}"),
            Self::BranchReg { kind, rn } => {
                if kind == Indirect::Ret && rn.gpr == Gpr::R(30) {
                    write!(formatter, "{name}")
                } else {
                    write!(formatter, "{name} {rn}")
                }
            }
            Self::Svc { imm } => write!(formatter, "{name} #{imm}"),
            Self::CondSelect {
                rd, rn, rm, cond, ..
            } => write!(formatter, "{name} {rd}, {rn}, {rm}, {}", cond.name()),
            Self::TwoSource { rd, rn, rm, .. } | Self::Crc { rd, rn, rm, .. } => {
                write!(formatter, "{name} {rd}, {rn}, {rm}")
            }
            Self::OneSource { rd, rn, .. } => write!(formatter, "{name} {rd}, {rn}"),
            Self::ThreeSource {
                op, rd, rn, rm, ra, ..
            } => match op {
                Three::Smulh | Three::Umulh => write!(formatter, "{name} {rd}, {rn}, {rm}"),
                _ => write!(formatter, "{name} {rd}, {rn}, {rm}, {ra}"),
            },
            Self::LoadStore { rt, address, .. } => write!(formatter, "{name} {rt}, {address}"),
            Self::Pair {
                rt, rt2, address, ..
            } => write!(formatter, "{name} {rt}, {rt2}, {address}"),
            Self::Nop => write!(formatter, "{name}"),
            Self::FpOneSource { rd, rn, .. } => write!(formatter, "{name} {rd}, {rn}"),
            Self::FpTwoSource { rd, rn, rm, .. } => write!(formatter, "{name} {rd}, {rn}, {rm}"),
            Self::FpThreeSource { rd, rn, rm, ra, .. } => {
                write!(formatter, "{name} {rd}, {rn}, {rm}, {ra}")
            }
            Self::FpCompare { rn, rm, .. } => match rm {
                Some(rm) => write!(formatter, "{name} {rn}, {rm}"),
                None => write!(formatter, "{name} {rn}, #0.0"),
            },
            Self::FpCondSelect { rd, rn, rm, cond } => {
                write!(formatter, "{name} {rd}, {rn}, {rm}, {}", cond.name())
            }
            Self::FpMoveImm { rd, imm8 } => {
                write!(formatter, "{name} {rd}, #{:.8}", fp_immediate(imm8))
            }
            Self::FpConvert { op, fp, gpr } => match op {
                FpConversion::Scvtf | FpConversion::Ucvtf | FpConversion::FromGeneral => {
                    write!(formatter, "{name} {fp}, {gpr}")
                }
                FpConversion::Fcvtzs | FpConversion::Fcvtzu | FpConversion::ToGeneral => {
                    write!(formatter, "{name} {gpr}, {fp}")
                }
            },
            Self::FpLoadStore { rt, address, .. } => write!(formatter, "{name} {rt}, {address}"),
            Self::FpPair {
                rt, rt2, address, ..
            } => write!(formatter, "{name} {rt}, {rt2}, {address}"),
            Self::VectorThree { op, rd, rn, rm } if op == VThree::Orr && rn == rm => {
                write!(formatter, "{name} {rd}, {rn}")
            }
            Self::VectorThree { rd, rn, rm, .. } | Self::VectorPermute { rd, rn, rm, .. } => {
                write!(formatter, "{name} {rd}, {rn}, {rm}")
            }
            Self::Pmull { rd, rn, rm } => {
                let product = if pmull_wide(rn) { "1q" } else { "8h" };
                write!(formatter, "{name} v{rd}.{product}, {rn}, {rm}")
            }
            Self::VectorTwo { op, rd, rn } if op.with_zero() => {
                let zero = if op.float() { "#0.0" } else { "#0" };
                write!(formatter, "{name} {rd}, {rn}, {zero}")
            }
            Self::VectorTwo { rd, rn, .. } => write!(formatter, "{name} {rd}, {rn}"),
            Self::VectorAcross { rd, rn, .. } => write!(formatter, "{name} {rd}, {rn}"),
            Self::VectorShift { rd, rn, amount, .. } => {
                write!(formatter, "{name} {rd}, {rn}, #{amount}")
            }
            Self::VectorExtract { rd, rn, rm, index } => {
                write!(formatter, "{name} {rd}, {rn}, {rm}, #{index}")
            }
            Self::VectorTable {
                rd,
                table,
                count,
                rm,
                ..
            } => write!(
                formatter,
                "{name} {rd}, {}, {rm}",
                vector::list(table, count, Arrangement::B16)
            ),
            Self::VectorDup { rd, from } => write!(formatter, "{name} {rd}, {from}"),
            Self::VectorToGeneral { rd, rn, .. } => write!(formatter, "{name} {rd}, {rn}"),
            Self::VectorInsert { rd, from } => write!(formatter, "{name} {rd}, {from}"),
            Self::VectorImmediate {
                rd, imm8, expand, ..
            } => {
                let register = if rd.arrangement == Arrangement::D1 {
                    format!("d{}", rd.number)
                } else {
                    rd.to_string()
                };
                match expand {
                    Expand::Float => {
                        write!(formatter, "{name} {register}, #{:.8}", fp_immediate(imm8))
                    }
                    Expand::Bytes => write!(
                        formatter,
                        "{name} {register}, #{:#018x}",
                        expand.value(imm8, 64)
                    ),
                    Expand::Lsl(0) => write!(formatter, "{name} {register}, #{imm8}"),
                    Expand::Lsl(by) => write!(formatter, "{name} {register}, #{imm8}, lsl #{by}"),
                    Expand::Msl(by) => write!(formatter, "{name} {register}, #{imm8}, msl #{by}"),
                }
            }
            Self::VectorLoadStore { rt, address, .. } => {
                write!(formatter, "{name} q{rt}, {address}")
            }
            Self::VectorPair {
                rt, rt2, address, ..
            } => write!(formatter, "{name} q{rt}, q{rt2}, {address}"),
            Self::VectorStructures {
                first,
                count,
                arrangement,
                base,
                step,
                ..
            } => {
                let list = vector::list(first, count, arrangement);
                match step {
                    None => write!(formatter, "{name} {list}, [{base}]"),
                    Some(Writeback::Bytes) => write!(
                        formatter,
                        "{name} {list}, [{base}], #{}",
                        u32::from(count) * arrangement.bytes()
                    ),
                    Some(Writeback::Register(by)) => {
                        write!(formatter, "{name} {list}, [{base}], {by}")
                    }
                }
            }
        }
    }
}
