//! SVE's instructions in the dialect: the integer subset a streaming loop over bytes needs, every
//! element a byte. A Z register is as long as the CPU makes it, 16 to 256 bytes, its lowest 16
//! the V register of its number; a P register holds a bit for each of a Z register's bytes.
//! lengua's model holds vectors up to 64 bytes, and a program is proven at its CPU's length,
//! which its lock records. First-fault and gathering loads, the first-fault register, and every
//! element wider than a byte are outside the dialect.

use std::fmt;

use super::A64Reg;

/// What a compare tests in each lane: equality, or order without a sign (`hs`, `hi`, `lo`,
/// `ls`) or with one (`ge`, `gt`, `lt`, `le`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cmp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Higher or the same, without a sign.
    Hs,
    /// Higher, without a sign.
    Hi,
    /// Lower, without a sign.
    Lo,
    /// Lower or the same, without a sign.
    Ls,
    /// Greater or equal, with a sign.
    Ge,
    /// Greater, with a sign.
    Gt,
    /// Less, with a sign.
    Lt,
    /// Less or equal, with a sign.
    Le,
}

impl Cmp {
    /// Its name, as `cmp` and it make the mnemonic.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Hs => "hs",
            Self::Hi => "hi",
            Self::Lo => "lo",
            Self::Ls => "ls",
            Self::Ge => "ge",
            Self::Gt => "gt",
            Self::Lt => "lt",
            Self::Le => "le",
        }
    }

    /// The comparison named `name`.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        [
            Self::Eq,
            Self::Ne,
            Self::Hs,
            Self::Hi,
            Self::Lo,
            Self::Ls,
            Self::Ge,
            Self::Gt,
            Self::Lt,
            Self::Le,
        ]
        .into_iter()
        .find(|cmp| cmp.name() == name)
    }

    /// The same test with its operands the other way round: `lo` is `hi` reversed.
    #[must_use]
    pub const fn swapped(self) -> Self {
        match self {
            Self::Eq => Self::Eq,
            Self::Ne => Self::Ne,
            Self::Hs => Self::Ls,
            Self::Hi => Self::Lo,
            Self::Lo => Self::Hi,
            Self::Ls => Self::Hs,
            Self::Ge => Self::Le,
            Self::Gt => Self::Lt,
            Self::Lt => Self::Gt,
            Self::Le => Self::Ge,
        }
    }

    /// Whether it orders with a sign.
    #[must_use]
    pub const fn signed(self) -> bool {
        matches!(self, Self::Ge | Self::Gt | Self::Lt | Self::Le)
    }
}

/// What a compare compares each lane with: the same lane of another vector, or a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Against {
    /// Another Z register.
    Vector(u8),
    /// A number: 0 to 127 for the unsigned tests, -16 to 15 for the others.
    Number(i32),
}

/// A logical operation of predicates, lane by lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PLogic {
    /// Both.
    And,
    /// The first and not the second.
    Bic,
    /// One or the other, not both.
    Eor,
    /// Either.
    Orr,
}

/// A bitwise operation of two whole Z registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZLogic {
    /// And.
    And,
    /// Or.
    Orr,
    /// Exclusive or.
    Eor,
    /// The first and not the second.
    Bic,
}

/// An SVE instruction of the dialect. Predicates and Z registers are numbers: p0 to p15 and z0
/// to z31; a governing predicate of a load, a store, a compare and the predicated vector
/// operations is p0 to p7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sve {
    /// `ptrue p0.b`: every lane active.
    Ptrue {
        /// The predicate set.
        pd: u8,
    },
    /// `whilelo p0.b, x1, x2`: lane i active while `x1 + i` is below `x2`, without a sign; the
    /// flags as the result says: N its first lane, Z none active, C not its last lane.
    Whilelo {
        /// The predicate set.
        pd: u8,
        /// The first lane's count.
        rn: A64Reg,
        /// The limit.
        rm: A64Reg,
    },
    /// `and p0.b, p1/z, p2.b, p3.b`, and `bic`, `eor` and `orr`: lane by lane where p1 is
    /// active, zero elsewhere. `not p0.b, p1/z, p2.b` is `eor` with p1 for p3; `mov p0.b,
    /// p1/z, p2.b` is `and` with p2 twice, and `mov p0.b, p2.b` is `orr` with p2 thrice.
    PredLogic {
        /// The operation.
        op: PLogic,
        /// Where the result goes.
        pd: u8,
        /// The lanes it acts in.
        pg: u8,
        /// The first operand.
        pn: u8,
        /// The second operand.
        pm: u8,
    },
    /// `cntp x0, p1, p2.b`: how many lanes both have active.
    Cntp {
        /// Where the count goes.
        rd: A64Reg,
        /// The lanes it counts in.
        pg: u8,
        /// The lanes counted.
        pn: u8,
    },
    /// `ld1b { z0.b }, p0/z, [x1, x2]`: each active lane's byte from `x1 + x2` on, zero in the
    /// others; `st1b { z0.b }, p0, [x1, x2]` stores each active lane's byte.
    Access {
        /// A store.
        store: bool,
        /// The Z register loaded or stored.
        zt: u8,
        /// The active lanes.
        pg: u8,
        /// The base address.
        base: A64Reg,
        /// The index added to it, in bytes.
        index: A64Reg,
    },
    /// `cmpeq p1.b, p0/z, z0.b, z1.b` or `, #10`: each lane's test where p0 is active, zero
    /// elsewhere; the flags as the result says, over p0's lanes.
    Compare {
        /// The test.
        cmp: Cmp,
        /// Where the result goes.
        pd: u8,
        /// The lanes it tests.
        pg: u8,
        /// The vector tested.
        zn: u8,
        /// What it's tested against.
        against: Against,
    },
    /// `add z0.b, z1.b, z2.b` and `sub`: each lane's sum or difference, wrapping.
    Arith {
        /// A subtraction.
        sub: bool,
        /// Where the result goes.
        zd: u8,
        /// The first operand.
        zn: u8,
        /// The second.
        zm: u8,
    },
    /// `and z0.d, z1.d, z2.d`, and `orr`, `eor` and `bic`, of all the bits. `mov z0.d, z1.d` is
    /// `orr` with z1 twice.
    Logic {
        /// The operation.
        op: ZLogic,
        /// Where the result goes.
        zd: u8,
        /// The first operand.
        zn: u8,
        /// The second.
        zm: u8,
    },
    /// `cnt z0.b, p0/m, z1.b`: each active lane's count of set bits, the others kept.
    Cnt {
        /// Where the counts go.
        zd: u8,
        /// The lanes counted.
        pg: u8,
        /// The bytes counted.
        zn: u8,
    },
    /// `sel z0.b, p0, z1.b, z2.b`: z1's byte where p0 is active, z2's elsewhere. `mov z0.b,
    /// p0/m, z1.b` is `sel` with z0 for z2.
    Sel {
        /// Where the result goes.
        zd: u8,
        /// The lanes that take the first.
        pg: u8,
        /// The first.
        zn: u8,
        /// The second.
        zm: u8,
    },
    /// `mov z0.b, #10` (`dup`): a number in every lane.
    DupImm {
        /// Where it goes.
        zd: u8,
        /// The number, -128 to 127.
        imm: i8,
    },
    /// `mov z0.b, w1` (`dup`): a register's low byte in every lane.
    DupReg {
        /// Where it goes.
        zd: u8,
        /// The register.
        rn: A64Reg,
    },
    /// `mov z0.b, p0/z, #1` (`cpy`): a number in each active lane, zero in the others, or with
    /// `p0/m` the others kept.
    Cpy {
        /// Where it goes.
        zd: u8,
        /// The lanes that take it.
        pg: u8,
        /// Whether the other lanes keep their bytes, rather than being zeroed.
        merge: bool,
        /// The number, -128 to 127.
        imm: i8,
    },
    /// `uaddv d0, p0, z1.b`: the sum of the active lanes' bytes, without a sign, in d0.
    Uaddv {
        /// The V register whose low doubleword takes the sum, the rest cleared.
        vd: u8,
        /// The lanes added.
        pg: u8,
        /// The bytes.
        zn: u8,
    },
    /// `incb x0`, or `incb x0, all, mul #4`: x0 plus the vector's length in bytes, times the
    /// number; `cntb x0` sets it to that.
    Count {
        /// The register.
        rd: A64Reg,
        /// Whether it adds to the register, rather than setting it.
        add: bool,
        /// The multiplier, 1 to 16.
        times: u8,
    },
    /// `insr z0.b, w1`: each lane moved up one, the last dropped, and w1's low byte in lane 0.
    Insr {
        /// The vector.
        zdn: u8,
        /// The register put in, `wzr` for zero.
        rm: A64Reg,
    },
    /// `lastb w0, p0, z1.b`: the byte in p0's last active lane, or in the vector's last lane
    /// when none is active.
    Lastb {
        /// Where it goes, `wzr` for nowhere.
        rd: A64Reg,
        /// The lanes looked at.
        pg: u8,
        /// The vector.
        zn: u8,
    },
}

impl Sve {
    /// Its mnemonic, as the canonical text writes it.
    #[must_use]
    pub const fn mnemonic(&self) -> &'static str {
        match self {
            Self::Ptrue { .. } => "ptrue",
            Self::Whilelo { .. } => "whilelo",
            Self::PredLogic { op, pg, pn, pm, .. } => match op {
                PLogic::Eor if *pm == *pg => "not",
                PLogic::And if *pn == *pm => "mov",
                PLogic::Orr if *pn == *pm && *pg == *pn => "mov",
                PLogic::And => "and",
                PLogic::Bic => "bic",
                PLogic::Eor => "eor",
                PLogic::Orr => "orr",
            },
            Self::Cntp { .. } => "cntp",
            Self::Access { store: false, .. } => "ld1b",
            Self::Access { store: true, .. } => "st1b",
            Self::Compare { cmp, .. } => match cmp {
                Cmp::Eq => "cmpeq",
                Cmp::Ne => "cmpne",
                Cmp::Hs => "cmphs",
                Cmp::Hi => "cmphi",
                Cmp::Lo => "cmplo",
                Cmp::Ls => "cmpls",
                Cmp::Ge => "cmpge",
                Cmp::Gt => "cmpgt",
                Cmp::Lt => "cmplt",
                Cmp::Le => "cmple",
            },
            Self::Arith { sub: false, .. } => "add",
            Self::Arith { sub: true, .. } => "sub",
            Self::Logic { op, zn, zm, .. } => match op {
                ZLogic::Orr if *zn == *zm => "mov",
                ZLogic::And => "and",
                ZLogic::Orr => "orr",
                ZLogic::Eor => "eor",
                ZLogic::Bic => "bic",
            },
            Self::Cnt { .. } => "cnt",
            Self::Sel { zd, zm, .. } if *zd == *zm => "mov",
            Self::Sel { .. } => "sel",
            Self::DupImm { .. } | Self::DupReg { .. } | Self::Cpy { .. } => "mov",
            Self::Uaddv { .. } => "uaddv",
            Self::Count { add: true, .. } => "incb",
            Self::Count { add: false, .. } => "cntb",
            Self::Insr { .. } => "insr",
            Self::Lastb { .. } => "lastb",
        }
    }

    /// Whether it sets the flags.
    #[must_use]
    pub const fn sets_flags(&self) -> bool {
        matches!(self, Self::Whilelo { .. } | Self::Compare { .. })
    }
}

impl fmt::Display for Sve {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.mnemonic();
        match *self {
            Self::Ptrue { pd } => write!(formatter, "{name} p{pd}.b"),
            Self::Whilelo { pd, rn, rm } => write!(formatter, "{name} p{pd}.b, {rn}, {rm}"),
            Self::PredLogic { op, pd, pg, pn, pm } => match name {
                "not" => write!(formatter, "not p{pd}.b, p{pg}/z, p{pn}.b"),
                "mov" if op == PLogic::Orr => write!(formatter, "mov p{pd}.b, p{pn}.b"),
                "mov" => write!(formatter, "mov p{pd}.b, p{pg}/z, p{pn}.b"),
                _ => write!(formatter, "{name} p{pd}.b, p{pg}/z, p{pn}.b, p{pm}.b"),
            },
            Self::Cntp { rd, pg, pn } => write!(formatter, "{name} {rd}, p{pg}, p{pn}.b"),
            Self::Access {
                store,
                zt,
                pg,
                base,
                index,
            } => write!(
                formatter,
                "{name} {{ z{zt}.b }}, p{pg}{}, [{base}, {index}]",
                if store { "" } else { "/z" }
            ),
            Self::Compare {
                pd,
                pg,
                zn,
                against,
                ..
            } => match against {
                Against::Vector(zm) => {
                    write!(formatter, "{name} p{pd}.b, p{pg}/z, z{zn}.b, z{zm}.b")
                }
                Against::Number(number) => {
                    write!(formatter, "{name} p{pd}.b, p{pg}/z, z{zn}.b, #{number}")
                }
            },
            Self::Arith { zd, zn, zm, .. } => {
                write!(formatter, "{name} z{zd}.b, z{zn}.b, z{zm}.b")
            }
            Self::Logic { zd, zn, .. } if name == "mov" => {
                write!(formatter, "mov z{zd}.d, z{zn}.d")
            }
            Self::Logic { zd, zn, zm, .. } => {
                write!(formatter, "{name} z{zd}.d, z{zn}.d, z{zm}.d")
            }
            Self::Cnt { zd, pg, zn } => write!(formatter, "{name} z{zd}.b, p{pg}/m, z{zn}.b"),
            Self::Sel { zd, pg, zn, .. } if name == "mov" => {
                write!(formatter, "mov z{zd}.b, p{pg}/m, z{zn}.b")
            }
            Self::Sel { zd, pg, zn, zm } => {
                write!(formatter, "{name} z{zd}.b, p{pg}, z{zn}.b, z{zm}.b")
            }
            Self::DupImm { zd, imm } => write!(formatter, "mov z{zd}.b, #{imm}"),
            Self::DupReg { zd, rn } => write!(formatter, "mov z{zd}.b, {rn}"),
            Self::Cpy { zd, pg, merge, imm } => write!(
                formatter,
                "mov z{zd}.b, p{pg}/{}, #{imm}",
                if merge { "m" } else { "z" }
            ),
            Self::Uaddv { vd, pg, zn } => write!(formatter, "{name} d{vd}, p{pg}, z{zn}.b"),
            Self::Count { rd, times: 1, .. } => write!(formatter, "{name} {rd}"),
            Self::Count { rd, times, .. } => write!(formatter, "{name} {rd}, all, mul #{times}"),
            Self::Insr { zdn, rm } => write!(formatter, "{name} z{zdn}.b, {rm}"),
            Self::Lastb { rd, pg, zn } => write!(formatter, "{name} {rd}, p{pg}, z{zn}.b"),
        }
    }
}

/// Bits `high` down to `low` of `word`.
const fn field(word: u32, high: u32, low: u32) -> u32 {
    (word >> low) & ((1 << (high - low + 1)) - 1)
}

/// The register a five-bit field names, `x` or `w`, 31 the zero register.
const fn general(word: u32, low: u32, wide: bool) -> A64Reg {
    A64Reg::new(field(word, low + 4, low), wide, false)
}

/// `word`, an instruction of SVE's encodings (bits 28 to 25 `0010`), as the dialect has it, or
/// none where the dialect doesn't have it. Every form takes bytes (a size field of 0) and
/// predicates' every pattern but all is outside the dialect.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn decode(word: u32) -> Option<Sve> {
    let bytes = field(word, 23, 22) == 0;
    let pd = field(word, 3, 0) as u8;
    let pg3 = field(word, 12, 10) as u8;
    let pg4 = field(word, 13, 10) as u8;
    let pn = field(word, 8, 5) as u8;
    let zd = field(word, 4, 0) as u8;
    let zn = field(word, 9, 5) as u8;
    let zm = field(word, 20, 16) as u8;
    if word & 0xff3e_fc10 == 0x2518_e000 {
        // PTRUE, of every lane, not PTRUES.
        (bytes && field(word, 16, 16) == 0 && field(word, 9, 5) == 0b11111)
            .then_some(Sve::Ptrue { pd })
    } else if word & 0xff20_ec10 == 0x2520_0c00 {
        // WHILELO of x registers.
        (bytes && field(word, 12, 12) == 1).then(|| Sve::Whilelo {
            pd,
            rn: general(word, 5, true),
            rm: general(word, 16, true),
        })
    } else if word & 0xff30_c000 == 0x2500_4000 {
        // The predicates' logic, without setting the flags.
        let op = match (field(word, 23, 23), field(word, 9, 9), field(word, 4, 4)) {
            (0, 0, 0) => Some(PLogic::And),
            (0, 0, 1) => Some(PLogic::Bic),
            (0, 1, 0) => Some(PLogic::Eor),
            (1, 0, 0) => Some(PLogic::Orr),
            _ => None,
        };
        op.filter(|_| field(word, 22, 22) == 0)
            .map(|op| Sve::PredLogic {
                op,
                pd,
                pg: pg4,
                pn,
                pm: field(word, 19, 16) as u8,
            })
    } else if word & 0xff3f_c200 == 0x2520_8000 {
        bytes.then(|| Sve::Cntp {
            rd: general(word, 0, true),
            pg: pg4,
            pn,
        })
    } else if word & 0xffe0_e000 == 0xa400_4000 || word & 0xffe0_e000 == 0xe400_4000 {
        // LD1B and ST1B of a register plus a register; xzr isn't an index there, nor sp a base
        // in the dialect.
        let (base, index) = (field(word, 9, 5), field(word, 20, 16));
        (base != 31 && index != 31).then(|| Sve::Access {
            store: word >> 30 & 1 == 1,
            zt: zd,
            pg: pg3,
            base: general(word, 5, true),
            index: general(word, 16, true),
        })
    } else if word & 0xff20_4000 == 0x2400_0000 {
        // Compares of two vectors; the wide elements' forms are outside the dialect.
        let cmp = match (field(word, 15, 15), field(word, 13, 13), field(word, 4, 4)) {
            (0, 0, 0) => Some(Cmp::Hs),
            (0, 0, 1) => Some(Cmp::Hi),
            (1, 0, 0) => Some(Cmp::Ge),
            (1, 0, 1) => Some(Cmp::Gt),
            (1, 1, 0) => Some(Cmp::Eq),
            (1, 1, 1) => Some(Cmp::Ne),
            _ => None,
        };
        cmp.filter(|_| bytes).map(|cmp| Sve::Compare {
            cmp,
            pd,
            pg: pg3,
            zn,
            against: Against::Vector(zm),
        })
    } else if word & 0xff20_0000 == 0x2420_0000 {
        // Compares with a number without a sign, 0 to 127.
        let cmp = match (field(word, 13, 13), field(word, 4, 4)) {
            (0, 0) => Cmp::Hs,
            (0, _) => Cmp::Hi,
            (_, 0) => Cmp::Lo,
            _ => Cmp::Ls,
        };
        bytes.then(|| Sve::Compare {
            cmp,
            pd,
            pg: pg3,
            zn,
            against: Against::Number(field(word, 20, 14) as i32),
        })
    } else if word & 0xff20_4000 == 0x2500_0000 {
        // Compares with a number with a sign, -16 to 15.
        let cmp = match (field(word, 15, 15), field(word, 13, 13), field(word, 4, 4)) {
            (0, 0, 0) => Some(Cmp::Ge),
            (0, 0, 1) => Some(Cmp::Gt),
            (0, 1, 0) => Some(Cmp::Lt),
            (0, 1, 1) => Some(Cmp::Le),
            (1, 0, 0) => Some(Cmp::Eq),
            (1, 0, 1) => Some(Cmp::Ne),
            _ => None,
        };
        let number = (field(word, 20, 16) as i32) << 27 >> 27;
        cmp.filter(|_| bytes).map(|cmp| Sve::Compare {
            cmp,
            pd,
            pg: pg3,
            zn,
            against: Against::Number(number),
        })
    } else if word & 0xff20_e000 == 0x0420_0000 {
        // Addition and subtraction of two vectors; the saturating forms are outside the dialect.
        match field(word, 12, 10) {
            0 | 1 if bytes => Some(Sve::Arith {
                sub: field(word, 12, 10) == 1,
                zd,
                zn,
                zm,
            }),
            _ => None,
        }
    } else if word & 0xff20_fc00 == 0x0420_3000 {
        let op = match field(word, 23, 22) {
            0 => ZLogic::And,
            1 => ZLogic::Orr,
            2 => ZLogic::Eor,
            _ => ZLogic::Bic,
        };
        Some(Sve::Logic { op, zd, zn, zm })
    } else if word & 0xff3f_e000 == 0x041a_a000 {
        bytes.then_some(Sve::Cnt { zd, pg: pg3, zn })
    } else if word & 0xff20_c000 == 0x0520_c000 {
        bytes.then_some(Sve::Sel {
            zd,
            pg: pg4,
            zn,
            zm,
        })
    } else if word & 0xff3f_c000 == 0x2538_c000 {
        (bytes && field(word, 13, 13) == 0).then(|| Sve::DupImm {
            zd,
            imm: field(word, 12, 5) as u8 as i8,
        })
    } else if word & 0xff3f_fc00 == 0x0520_3800 {
        // DUP of a w register; register 31 is wsp there, outside the dialect.
        (bytes && field(word, 9, 5) != 31).then(|| Sve::DupReg {
            zd,
            rn: general(word, 5, false),
        })
    } else if word & 0xff30_8000 == 0x0510_0000 {
        (bytes && field(word, 13, 13) == 0).then(|| Sve::Cpy {
            zd,
            pg: field(word, 19, 16) as u8,
            merge: field(word, 14, 14) == 1,
            imm: field(word, 12, 5) as u8 as i8,
        })
    } else if word & 0xff3f_e000 == 0x0401_2000 {
        bytes.then_some(Sve::Uaddv {
            vd: zd,
            pg: pg3,
            zn,
        })
    } else if word & 0xff30_f800 == 0x0430_e000 {
        // INCB, not DECB, of every lane, into a register (xzr is no place for a count).
        (bytes && field(word, 10, 10) == 0 && field(word, 9, 5) == 0b11111 && zd != 31).then(|| {
            Sve::Count {
                rd: general(word, 0, true),
                add: true,
                times: field(word, 19, 16) as u8 + 1,
            }
        })
    } else if word & 0xff30_fc00 == 0x0420_e000 {
        (bytes && field(word, 9, 5) == 0b11111 && zd != 31).then(|| Sve::Count {
            rd: general(word, 0, true),
            add: false,
            times: field(word, 19, 16) as u8 + 1,
        })
    } else if word & 0xff3f_fc00 == 0x0524_3800 {
        bytes.then(|| Sve::Insr {
            zdn: zd,
            rm: general(word, 5, false),
        })
    } else if word & 0xff3f_e000 == 0x0521_a000 {
        bytes.then(|| Sve::Lastb {
            rd: general(word, 0, false),
            pg: pg3,
            zn,
        })
    } else {
        None
    }
}
