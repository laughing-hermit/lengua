//! AArch64's Advanced SIMD vectors (NEON) in the dialect's canonical form: the V registers with
//! an arrangement of lanes, and the integer operations the dialect has on them.

use std::fmt;

use super::A64Reg;

/// How a V register's bits are cut into lanes: `8b` is eight lanes of 8 bits in its low 64 bits,
/// `16b` sixteen of them in all 128, and so on to `2d`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arrangement {
    /// 8 bytes, in the low half.
    B8,
    /// 16 bytes.
    B16,
    /// 4 halfwords, in the low half.
    H4,
    /// 8 halfwords.
    H8,
    /// 2 words, in the low half.
    S2,
    /// 4 words.
    S4,
    /// 1 doubleword, the low half.
    D1,
    /// 2 doublewords.
    D2,
}

impl Arrangement {
    /// The arrangement of lanes `8 << size` bits wide that fills the low 64 bits, or all 128
    /// with `whole`.
    #[must_use]
    pub const fn new(size: u32, whole: bool) -> Self {
        match (size & 3, whole) {
            (0, false) => Self::B8,
            (0, true) => Self::B16,
            (1, false) => Self::H4,
            (1, true) => Self::H8,
            (2, false) => Self::S2,
            (2, true) => Self::S4,
            (_, false) => Self::D1,
            (_, true) => Self::D2,
        }
    }

    /// The lanes' size as the encodings give it: 0 for bytes up to 3 for doublewords.
    #[must_use]
    pub const fn size(self) -> u32 {
        match self {
            Self::B8 | Self::B16 => 0,
            Self::H4 | Self::H8 => 1,
            Self::S2 | Self::S4 => 2,
            Self::D1 | Self::D2 => 3,
        }
    }

    /// Whether it fills all 128 bits.
    #[must_use]
    pub const fn whole(self) -> bool {
        matches!(self, Self::B16 | Self::H8 | Self::S4 | Self::D2)
    }

    /// A lane's width in bits.
    #[must_use]
    pub const fn lane_bits(self) -> u32 {
        8 << self.size()
    }

    /// How many lanes it has.
    #[must_use]
    pub const fn lanes(self) -> u32 {
        (if self.whole() { 128 } else { 64 }) / self.lane_bits()
    }

    /// How many bytes it covers: 8 or 16.
    #[must_use]
    pub const fn bytes(self) -> u32 {
        if self.whole() { 16 } else { 8 }
    }

    /// Its name after a register's: `16b`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::B8 => "8b",
            Self::B16 => "16b",
            Self::H4 => "4h",
            Self::H8 => "8h",
            Self::S2 => "2s",
            Self::S4 => "4s",
            Self::D1 => "1d",
            Self::D2 => "2d",
        }
    }

    /// The arrangement by its name, in either case.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        [
            Self::B8,
            Self::B16,
            Self::H4,
            Self::H8,
            Self::S2,
            Self::S4,
            Self::D1,
            Self::D2,
        ]
        .into_iter()
        .find(|arrangement| arrangement.name().eq_ignore_ascii_case(name))
    }
}

/// The letter of a lane `8 << size` bits wide: `b`, `h`, `s` or `d`.
#[must_use]
pub const fn size_letter(size: u32) -> char {
    match size & 3 {
        0 => 'b',
        1 => 'h',
        2 => 's',
        _ => 'd',
    }
}

/// A V register read or written as a vector of lanes: `v3.16b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VReg {
    /// Which of the 32, v0 to v31.
    pub number: u8,
    /// Its lanes.
    pub arrangement: Arrangement,
}

impl fmt::Display for VReg {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "v{}.{}", self.number, self.arrangement.name())
    }
}

/// One lane of a V register: `v1.b[3]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lane {
    /// Which V register.
    pub number: u8,
    /// The lane's size: 0 for a byte up to 3 for a doubleword.
    pub size: u32,
    /// Which lane, from 0 at the lowest bits.
    pub index: u8,
}

impl fmt::Display for Lane {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "v{}.{}[{}]",
            self.number,
            size_letter(self.size),
            self.index
        )
    }
}

/// The low lane of a V register as a scalar, as the operations across lanes write it: `b0`,
/// `h0`, `s0` or `d0`, the rest of the register cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scalar {
    /// Which V register.
    pub number: u8,
    /// Its size: 0 for a byte up to 3 for a doubleword.
    pub size: u32,
}

impl fmt::Display for Scalar {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}{}", size_letter(self.size), self.number)
    }
}

/// The operations on the lanes of two vectors, each lane with its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VThree {
    /// `add`
    Add,
    /// `sub`
    Sub,
    /// `mul`, keeping each product's low half.
    Mul,
    /// `uqadd`: added, saturating without a sign.
    Uqadd,
    /// `uqsub`: subtracted, saturating at zero.
    Uqsub,
    /// `sqadd`: added, saturating with a sign.
    Sqadd,
    /// `sqsub`: subtracted, saturating with a sign.
    Sqsub,
    /// `cmeq`: all ones where the lanes are equal, else zero, as every comparison gives.
    Cmeq,
    /// `cmtst`: all ones where the lanes share a set bit.
    Cmtst,
    /// `cmhi`: higher, without a sign.
    Cmhi,
    /// `cmhs`: higher or the same, without a sign.
    Cmhs,
    /// `cmgt`: greater, with a sign.
    Cmgt,
    /// `cmge`: greater or equal, with a sign.
    Cmge,
    /// `umax`
    Umax,
    /// `umin`
    Umin,
    /// `smax`
    Smax,
    /// `smin`
    Smin,
    /// `addp`: the sums of neighbouring lanes, the first vector's in the low lanes and the
    /// second's in the high, as every pairwise operation puts them.
    Addp,
    /// `umaxp`
    Umaxp,
    /// `uminp`
    Uminp,
    /// `smaxp`
    Smaxp,
    /// `sminp`
    Sminp,
    /// `and`, of bytes.
    And,
    /// `bic`: the first, its bits set in the second cleared.
    Bic,
    /// `orr`; `mov` where both sources are one register.
    Orr,
    /// `orn`: the first or'd with the second's bits inverted.
    Orn,
    /// `eor`
    Eor,
    /// `bsl`: the destination selects, bit by bit, the first where it's set and the second
    /// where it's clear.
    Bsl,
    /// `bit`: the first's bits into the destination where the second's are set.
    Bit,
    /// `bif`: the first's bits into the destination where the second's are clear.
    Bif,
}

impl VThree {
    /// Its mnemonic.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
            Self::Uqadd => "uqadd",
            Self::Uqsub => "uqsub",
            Self::Sqadd => "sqadd",
            Self::Sqsub => "sqsub",
            Self::Cmeq => "cmeq",
            Self::Cmtst => "cmtst",
            Self::Cmhi => "cmhi",
            Self::Cmhs => "cmhs",
            Self::Cmgt => "cmgt",
            Self::Cmge => "cmge",
            Self::Umax => "umax",
            Self::Umin => "umin",
            Self::Smax => "smax",
            Self::Smin => "smin",
            Self::Addp => "addp",
            Self::Umaxp => "umaxp",
            Self::Uminp => "uminp",
            Self::Smaxp => "smaxp",
            Self::Sminp => "sminp",
            Self::And => "and",
            Self::Bic => "bic",
            Self::Orr => "orr",
            Self::Orn => "orn",
            Self::Eor => "eor",
            Self::Bsl => "bsl",
            Self::Bit => "bit",
            Self::Bif => "bif",
        }
    }

    /// Whether it works on the register's bits whatever its lanes, so its arrangement is
    /// always of bytes.
    #[must_use]
    pub const fn bitwise(self) -> bool {
        matches!(
            self,
            Self::And
                | Self::Bic
                | Self::Orr
                | Self::Orn
                | Self::Eor
                | Self::Bsl
                | Self::Bit
                | Self::Bif
        )
    }

    /// Whether it reads the destination as well as its two sources.
    #[must_use]
    pub const fn reads_destination(self) -> bool {
        matches!(self, Self::Bsl | Self::Bit | Self::Bif)
    }
}

/// The operations on one vector's lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VTwo {
    /// `cnt`: each byte's set bits.
    Cnt,
    /// `not`, which LLVM writes `mvn`.
    Not,
    /// `rbit`: each byte's bits reversed.
    Rbit,
    /// `rev16`: the bytes of each halfword reversed.
    Rev16,
    /// `rev32`: the lanes of each word reversed.
    Rev32,
    /// `rev64`: the lanes of each doubleword reversed.
    Rev64,
    /// `cmeq …, #0`: all ones where a lane is zero.
    Cmeq0,
    /// `cmgt …, #0`: where a lane is above zero, with a sign.
    Cmgt0,
    /// `cmge …, #0`
    Cmge0,
    /// `cmle …, #0`
    Cmle0,
    /// `cmlt …, #0`
    Cmlt0,
    /// `abs`
    Abs,
    /// `neg`
    Neg,
    /// `cls`: the bits after the top one that match it.
    Cls,
    /// `clz`: the zeros above the highest set bit.
    Clz,
    /// `uaddlp`: neighbouring lanes added into lanes twice as wide, without a sign.
    Uaddlp,
    /// `saddlp`: the same with a sign.
    Saddlp,
    /// `xtn`: each lane's low half, into the destination's low half; `xtn2` into its high half,
    /// keeping the low.
    Xtn,
}

impl VTwo {
    /// Its mnemonic, as LLVM writes it for a destination of `rd`.
    #[must_use]
    pub const fn name(self, rd: VReg) -> &'static str {
        match self {
            Self::Cnt => "cnt",
            Self::Not => "mvn",
            Self::Rbit => "rbit",
            Self::Rev16 => "rev16",
            Self::Rev32 => "rev32",
            Self::Rev64 => "rev64",
            Self::Cmeq0 => "cmeq",
            Self::Cmgt0 => "cmgt",
            Self::Cmge0 => "cmge",
            Self::Cmle0 => "cmle",
            Self::Cmlt0 => "cmlt",
            Self::Abs => "abs",
            Self::Neg => "neg",
            Self::Cls => "cls",
            Self::Clz => "clz",
            Self::Uaddlp => "uaddlp",
            Self::Saddlp => "saddlp",
            Self::Xtn if rd.arrangement.whole() => "xtn2",
            Self::Xtn => "xtn",
        }
    }

    /// Whether it compares each lane with zero, written with a last operand `#0`.
    #[must_use]
    pub const fn with_zero(self) -> bool {
        matches!(
            self,
            Self::Cmeq0 | Self::Cmgt0 | Self::Cmge0 | Self::Cmle0 | Self::Cmlt0
        )
    }
}

/// The operations across a vector's lanes into one scalar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VAcross {
    /// `addv`: the lanes' sum, as wide as a lane.
    Addv,
    /// `umaxv`
    Umaxv,
    /// `uminv`
    Uminv,
    /// `smaxv`
    Smaxv,
    /// `sminv`
    Sminv,
    /// `uaddlv`: the lanes' sum, twice as wide, without a sign.
    Uaddlv,
    /// `saddlv`: the same with a sign.
    Saddlv,
}

impl VAcross {
    /// Its mnemonic.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Addv => "addv",
            Self::Umaxv => "umaxv",
            Self::Uminv => "uminv",
            Self::Smaxv => "smaxv",
            Self::Sminv => "sminv",
            Self::Uaddlv => "uaddlv",
            Self::Saddlv => "saddlv",
        }
    }

    /// Whether its result is twice as wide as a lane.
    #[must_use]
    pub const fn long(self) -> bool {
        matches!(self, Self::Uaddlv | Self::Saddlv)
    }
}

/// The shifts of each lane by a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VShift {
    /// `ushr`: right, zeros coming in.
    Ushr,
    /// `sshr`: right, copies of the sign coming in.
    Sshr,
    /// `usra`: right without a sign, added to the destination.
    Usra,
    /// `ssra`: right with a sign, added to the destination.
    Ssra,
    /// `sri`: right, into the destination, keeping its bits the shift doesn't reach.
    Sri,
    /// `shl`: left.
    Shl,
    /// `sli`: left, into the destination, keeping its bits the shift doesn't reach.
    Sli,
    /// `shrn`: right, then each lane's low half into the destination's low half; `shrn2` into
    /// its high half, keeping the low.
    Shrn,
    /// `ushll`: each lane of a half widened without a sign, then left; `uxtl` with no shift.
    Ushll,
    /// `sshll`: the same with a sign; `sxtl`.
    Sshll,
}

impl VShift {
    /// Its mnemonic, as LLVM writes it for these registers.
    #[must_use]
    pub const fn name(self, rd: VReg, rn: VReg) -> &'static str {
        match self {
            Self::Ushr => "ushr",
            Self::Sshr => "sshr",
            Self::Usra => "usra",
            Self::Ssra => "ssra",
            Self::Sri => "sri",
            Self::Shl => "shl",
            Self::Sli => "sli",
            Self::Shrn if rd.arrangement.whole() => "shrn2",
            Self::Shrn => "shrn",
            Self::Ushll if rn.arrangement.whole() => "ushll2",
            Self::Ushll => "ushll",
            Self::Sshll if rn.arrangement.whole() => "sshll2",
            Self::Sshll => "sshll",
        }
    }

    /// Whether it shifts right.
    #[must_use]
    pub const fn right(self) -> bool {
        matches!(
            self,
            Self::Ushr | Self::Sshr | Self::Usra | Self::Ssra | Self::Sri | Self::Shrn
        )
    }

    /// Whether it reads the destination as well.
    #[must_use]
    pub const fn reads_destination(self, rd: VReg) -> bool {
        match self {
            Self::Usra | Self::Ssra | Self::Sri | Self::Sli => true,
            Self::Shrn => rd.arrangement.whole(),
            _ => false,
        }
    }
}

/// The interleavings of two vectors' lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VPermute {
    /// `zip1`: the low halves' lanes, one from each in turn.
    Zip1,
    /// `zip2`: the high halves'.
    Zip2,
    /// `uzp1`: the even lanes of both, the first's then the second's.
    Uzp1,
    /// `uzp2`: the odd lanes.
    Uzp2,
    /// `trn1`: the even lanes of the first beside the even lanes of the second.
    Trn1,
    /// `trn2`: the odd lanes.
    Trn2,
}

impl VPermute {
    /// Its mnemonic.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Zip1 => "zip1",
            Self::Zip2 => "zip2",
            Self::Uzp1 => "uzp1",
            Self::Uzp2 => "uzp2",
            Self::Trn1 => "trn1",
            Self::Trn2 => "trn2",
        }
    }
}

/// What a number written in a vector instruction does to each lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VImmediate {
    /// `movi`: the number in every lane.
    Movi,
    /// `mvni`: its inverse in every lane.
    Mvni,
    /// `orr`: or'd into every lane.
    Orr,
    /// `bic`: its bits cleared from every lane.
    Bic,
}

impl VImmediate {
    /// Its mnemonic.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Movi => "movi",
            Self::Mvni => "mvni",
            Self::Orr => "orr",
            Self::Bic => "bic",
        }
    }
}

/// How the 8 bits a vector instruction holds become a lane's number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expand {
    /// Shifted left by this many bits, zeros coming in: `lsl #8`.
    Lsl(u8),
    /// Shifted left by 8 or 16 bits, ones coming in: `msl #8`.
    Msl(u8),
    /// Each bit a byte of all ones or all zeros: `movi` of a 64-bit mask.
    Bytes,
}

impl Expand {
    /// The lane's number from the 8 bits `imm8`, within a lane `width` bits wide.
    #[must_use]
    pub const fn value(self, imm8: u8, width: u32) -> u64 {
        let imm = imm8 as u64;
        let value = match self {
            Self::Lsl(by) => imm << by,
            Self::Msl(by) => (imm << by) | ((1 << by) - 1),
            Self::Bytes => {
                let mut value = 0;
                let mut bit = 0;
                while bit < 8 {
                    if imm8 >> bit & 1 == 1 {
                        value |= 0xff << (8 * bit);
                    }
                    bit += 1;
                }
                value
            }
        };
        if width >= 64 {
            value
        } else {
            value & ((1 << width) - 1)
        }
    }
}

/// Where a `dup` or an insert takes its value: a lane, or a general register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A lane of a V register.
    Lane(Lane),
    /// A general register, `w` for lanes up to 32 bits and `x` for doublewords.
    General(A64Reg),
}

impl fmt::Display for Source {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lane(lane) => write!(formatter, "{lane}"),
            Self::General(register) => write!(formatter, "{register}"),
        }
    }
}

/// How `ld1` and `st1` move their base after the access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Writeback {
    /// By the bytes they move: `[x1], #32`.
    Bytes,
    /// By a register: `[x1], x2`.
    Register(A64Reg),
}

/// The registers of a list, from `first`, `count` of them, wrapping from v31 to v0, as LLVM
/// writes it: `{ v0.16b, v1.16b }`.
#[must_use]
pub fn list(first: u8, count: u8, arrangement: Arrangement) -> String {
    let names: Vec<String> = (0..count)
        .map(|offset| {
            VReg {
                number: (first + offset) % 32,
                arrangement,
            }
            .to_string()
        })
        .collect();
    format!("{{ {} }}", names.join(", "))
}
