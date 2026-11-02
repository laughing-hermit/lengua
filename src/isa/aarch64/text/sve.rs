//! SVE's instructions as main.s writes them, LLVM's aliases among them: `not`, the `mov`s of
//! predicates and Z registers, `dup` as `mov`, and the compares that swap their operands (`cmplo`
//! of two vectors is `cmphi` the other way round).

use super::{Line, register};
use crate::isa::aarch64::sve::{Against, Cmp, PLogic, Sve, ZLogic};
use crate::isa::aarch64::{A64Reg, Gpr, Inst};

/// A predicate with its element, `p3.b`: its number.
fn predicate(text: &str) -> Option<u8> {
    number(text.strip_prefix('p')?.strip_suffix(".b")?, 15)
}

/// A governing predicate, `p1/z` or `p1/m`: its number and whether it merges.
fn governing(text: &str) -> Option<(u8, bool)> {
    let (name, how) = text.split_once('/')?;
    let merge = match how.trim() {
        "z" => false,
        "m" => true,
        _ => return None,
    };
    Some((number(name.trim().strip_prefix('p')?, 15)?, merge))
}

/// A predicate named alone, `p1`, as `cntp`, `sel`, `uaddv`, `lastb` and `st1b` take theirs.
fn bare_predicate(text: &str) -> Option<u8> {
    number(text.strip_prefix('p')?, 15)
}

/// A Z register of bytes, `z3.b`: its number.
fn z_bytes(text: &str) -> Option<u8> {
    number(text.strip_prefix('z')?.strip_suffix(".b")?, 31)
}

/// A Z register as doublewords, `z3.d`, as the bitwise operations name theirs.
fn z_doubles(text: &str) -> Option<u8> {
    number(text.strip_prefix('z')?.strip_suffix(".d")?, 31)
}

/// A register number from 0 to `most`, written without leading zeros.
fn number(digits: &str, most: u8) -> Option<u8> {
    if digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || (digits != "0" && digits.starts_with('0'))
    {
        return None;
    }
    digits.parse().ok().filter(|number| *number <= most)
}

/// A Z register as an operand names it, of bytes or doublewords: its number.
#[must_use]
pub fn z_register(text: &str) -> Option<u8> {
    z_bytes(text).or_else(|| z_doubles(text))
}

/// Whether `text` is a predicate as an operand names one: `p1.b`, `p1/z`, `p1/m` or `p1`.
#[must_use]
pub fn is_predicate(text: &str) -> bool {
    predicate(text).is_some() || governing(text).is_some() || bare_predicate(text).is_some()
}

/// Whether `text` names something only SVE has: a predicate with its element or its way
/// (`p1.b`, `p1/z`), or a Z register with its element (`z1.b`), alone or in a list.
fn sve_operand(text: &str) -> bool {
    let text = text.trim_start_matches('{').trim();
    let named = |prefix: char, after: &[u8]| {
        text.strip_prefix(prefix).is_some_and(|rest| {
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            digits > 0
                && rest
                    .as_bytes()
                    .get(digits)
                    .is_some_and(|byte| after.contains(byte))
        })
    };
    named('p', b"./") || named('z', b".")
}

impl Line<'_> {
    /// The line as an SVE instruction, or none where it isn't one: an operand is a predicate
    /// or a Z register, or the mnemonic is SVE's own.
    pub(super) fn sve(&self) -> Option<Result<Inst, String>> {
        let own = matches!(
            self.mnemonic,
            "ptrue" | "whilelo" | "incb" | "cntb" | "cntp"
        );
        if !own && !self.operands.iter().any(|operand| sve_operand(operand)) {
            return None;
        }
        Some(self.sve_instruction().map(Inst::Sve))
    }

    /// Operand `at`, a predicate with its element, `p0.b`.
    fn p_at(&self, at: usize) -> Result<u8, String> {
        self.operands
            .get(at)
            .and_then(|operand| predicate(operand))
            .ok_or_else(|| {
                self.problem(&format!(
                    "operand {} isn't a predicate of bytes, p0.b to p15.b",
                    at + 1
                ))
            })
    }

    /// Operand `at`, a governing predicate `p0/z` (zeroing) or `p0/m` (merging), the number at
    /// most `most`: 7 where the encoding has three bits for it.
    fn governing_at(&self, at: usize, most: u8, merge: bool) -> Result<u8, String> {
        let way = if merge { "m" } else { "z" };
        match self.operands.get(at).and_then(|operand| governing(operand)) {
            Some((number, merging)) if merging == merge && number <= most => Ok(number),
            _ => Err(self.problem(&format!(
                "operand {} is a governing predicate here, p0/{way} to p{most}/{way}",
                at + 1
            ))),
        }
    }

    /// Operand `at`, a predicate named alone, p0 to p`most`.
    fn bare_at(&self, at: usize, most: u8) -> Result<u8, String> {
        self.operands
            .get(at)
            .and_then(|operand| bare_predicate(operand))
            .filter(|number| *number <= most)
            .ok_or_else(|| {
                self.problem(&format!(
                    "operand {} is a governing predicate here, p0 to p{most}",
                    at + 1
                ))
            })
    }

    /// Operand `at`, a Z register of bytes.
    fn z_at(&self, at: usize) -> Result<u8, String> {
        self.operands
            .get(at)
            .and_then(|operand| z_bytes(operand))
            .ok_or_else(|| {
                self.problem(&format!(
                    "operand {} isn't a Z register of bytes, z0.b to z31.b: the dialect's SVE \
                     takes only bytes",
                    at + 1
                ))
            })
    }

    /// Operand `at`, a general register as wide as `wide` says, never sp; the zero register
    /// only where `zero` allows it.
    fn general_at(&self, at: usize, wide: bool, zero: bool) -> Result<A64Reg, String> {
        let found = self.operands.get(at).and_then(|operand| register(operand));
        match found {
            Some(reg) if reg.wide == wide && reg.gpr != Gpr::Sp && (zero || reg.gpr != Gpr::Zr) => {
                Ok(reg)
            }
            _ => Err(self.problem(&format!(
                "operand {} is {} register here",
                at + 1,
                match (wide, zero) {
                    (true, true) => "an x",
                    (true, false) => "an x (not xzr)",
                    (false, true) => "a w",
                    (false, false) => "a w (not wzr)",
                }
            ))),
        }
    }

    /// A number operand from `low` to `high`.
    fn sve_number(&self, at: usize, low: i64, high: i64) -> Result<i64, String> {
        let text = self.operands.get(at).copied().unwrap_or_default();
        let value = self.number(text)?;
        if (low..=high).contains(&value) {
            Ok(value)
        } else {
            Err(self.problem(&format!("its number is {low} to {high}")))
        }
    }

    /// A byte's number, -128 to 127, or 128 to 255 written as the byte it is.
    fn sve_byte(&self, at: usize) -> Result<i8, String> {
        let value = self.sve_number(at, -128, 255)?;
        Ok((value as u8).cast_signed())
    }

    #[allow(clippy::too_many_lines)]
    fn sve_instruction(&self) -> Result<Sve, String> {
        let first = self.operands.first().copied().unwrap_or_default();
        match self.mnemonic {
            "ptrue" => {
                self.count(1)?;
                Ok(Sve::Ptrue { pd: self.p_at(0)? })
            }
            "whilelo" => {
                self.count(3)?;
                Ok(Sve::Whilelo {
                    pd: self.p_at(0)?,
                    rn: self.general_at(1, true, true)?,
                    rm: self.general_at(2, true, true)?,
                })
            }
            "and" | "bic" | "eor" | "orr" if predicate(first).is_some() => {
                self.count(4)?;
                let op = match self.mnemonic {
                    "and" => PLogic::And,
                    "bic" => PLogic::Bic,
                    "eor" => PLogic::Eor,
                    _ => PLogic::Orr,
                };
                Ok(Sve::PredLogic {
                    op,
                    pd: self.p_at(0)?,
                    pg: self.governing_at(1, 15, false)?,
                    pn: self.p_at(2)?,
                    pm: self.p_at(3)?,
                })
            }
            "not" => {
                self.count(3)?;
                let pg = self.governing_at(1, 15, false)?;
                Ok(Sve::PredLogic {
                    op: PLogic::Eor,
                    pd: self.p_at(0)?,
                    pg,
                    pn: self.p_at(2)?,
                    pm: pg,
                })
            }
            "mov" if predicate(first).is_some() => {
                let pd = self.p_at(0)?;
                if self.operands.len() == 2 {
                    let pn = self.p_at(1)?;
                    return Ok(Sve::PredLogic {
                        op: PLogic::Orr,
                        pd,
                        pg: pn,
                        pn,
                        pm: pn,
                    });
                }
                self.count(3)?;
                let pn = self.p_at(2)?;
                Ok(Sve::PredLogic {
                    op: PLogic::And,
                    pd,
                    pg: self.governing_at(1, 15, false)?,
                    pn,
                    pm: pn,
                })
            }
            "cntp" => {
                self.count(3)?;
                Ok(Sve::Cntp {
                    rd: self.general_at(0, true, true)?,
                    pg: self.bare_at(1, 15)?,
                    pn: self.p_at(2)?,
                })
            }
            "ld1b" | "st1b" => self.sve_access(),
            name if name.starts_with("cmp") => self.sve_compare(&name[3..]),
            "add" | "sub" => {
                self.count(3)?;
                Ok(Sve::Arith {
                    sub: self.mnemonic == "sub",
                    zd: self.z_at(0)?,
                    zn: self.z_at(1)?,
                    zm: self.z_at(2)?,
                })
            }
            "and" | "orr" | "eor" | "bic" => {
                self.count(3)?;
                let op = match self.mnemonic {
                    "and" => ZLogic::And,
                    "orr" => ZLogic::Orr,
                    "eor" => ZLogic::Eor,
                    _ => ZLogic::Bic,
                };
                let doubles = |at: usize| {
                    self.operands
                        .get(at)
                        .and_then(|operand| z_doubles(operand))
                        .ok_or_else(|| {
                            self.problem(&format!(
                                "operand {} is a whole Z register as doublewords here, z0.d to \
                                 z31.d",
                                at + 1
                            ))
                        })
                };
                Ok(Sve::Logic {
                    op,
                    zd: doubles(0)?,
                    zn: doubles(1)?,
                    zm: doubles(2)?,
                })
            }
            "mov" if z_doubles(first).is_some() => {
                self.count(2)?;
                let zd = z_doubles(first).unwrap_or_default();
                let zn = self
                    .operands
                    .get(1)
                    .and_then(|operand| z_doubles(operand))
                    .ok_or_else(|| self.problem("operand 2 is z0.d to z31.d here"))?;
                Ok(Sve::Logic {
                    op: ZLogic::Orr,
                    zd,
                    zn,
                    zm: zn,
                })
            }
            "cnt" => {
                self.count(3)?;
                Ok(Sve::Cnt {
                    zd: self.z_at(0)?,
                    pg: self.governing_at(1, 7, true)?,
                    zn: self.z_at(2)?,
                })
            }
            "sel" => {
                self.count(4)?;
                Ok(Sve::Sel {
                    zd: self.z_at(0)?,
                    pg: self.bare_at(1, 15)?,
                    zn: self.z_at(2)?,
                    zm: self.z_at(3)?,
                })
            }
            "mov" | "dup" | "cpy" => self.sve_mov(),
            "uaddv" => {
                self.count(3)?;
                let vd = first
                    .strip_prefix('d')
                    .and_then(|digits| number(digits, 31))
                    .ok_or_else(|| self.problem("its sum goes in a d register, d0 to d31"))?;
                Ok(Sve::Uaddv {
                    vd,
                    pg: self.bare_at(1, 7)?,
                    zn: self.z_at(2)?,
                })
            }
            "incb" | "cntb" => {
                let rd = self.general_at(0, true, false)?;
                let times = match self.operands.len() {
                    1 => 1,
                    3 if self.operands[1] == "all" => {
                        let text = self.operands[2]
                            .strip_prefix("mul")
                            .map(str::trim)
                            .unwrap_or_default();
                        self.number(text)
                            .ok()
                            .filter(|times| (1..=16).contains(times))
                            .ok_or_else(|| {
                                self.problem("its multiplier is `mul #1` to `mul #16`")
                            })?
                    }
                    _ => {
                        return Err(self.problem(
                            "it counts every lane: `incb x0`, or `incb x0, all, mul #4`",
                        ));
                    }
                };
                Ok(Sve::Count {
                    rd,
                    add: self.mnemonic == "incb",
                    times: times as u8,
                })
            }
            "insr" => {
                self.count(2)?;
                Ok(Sve::Insr {
                    zdn: self.z_at(0)?,
                    rm: self.general_at(1, false, true)?,
                })
            }
            "lastb" => {
                self.count(3)?;
                Ok(Sve::Lastb {
                    rd: self.general_at(0, false, true)?,
                    pg: self.bare_at(1, 7)?,
                    zn: self.z_at(2)?,
                })
            }
            _ => Err(self.problem(
                "isn't in the dialect's SVE, the integer subset of bytes a streaming loop needs: \
                 see the SVE facts in references/linux-aarch64.md",
            )),
        }
    }

    /// `ld1b { z0.b }, p0/z, [x1, x2]` and `st1b { z0.b }, p0, [x1, x2]`.
    fn sve_access(&self) -> Result<Sve, String> {
        self.count(3)?;
        let store = self.mnemonic == "st1b";
        let zt = self
            .operands
            .first()
            .and_then(|operand| operand.strip_prefix('{'))
            .and_then(|rest| rest.strip_suffix('}'))
            .and_then(|inside| z_bytes(inside.trim()))
            .ok_or_else(|| self.problem("operand 1 is one Z register of bytes, `{ z0.b }`"))?;
        let pg = if store {
            self.bare_at(1, 7)?
        } else {
            self.governing_at(1, 7, false)?
        };
        let inside = self.operands[2]
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
            .ok_or_else(|| self.problem("its address is `[x1, x2]`, a base and an index"))?;
        let (base, index) = inside
            .split_once(',')
            .ok_or_else(|| self.problem("its address is `[x1, x2]`, a base and an index"))?;
        let (base, index) = (register(base.trim()), register(index.trim()));
        match (base, index) {
            (Some(base), Some(index))
                if base.wide
                    && index.wide
                    && matches!(base.gpr, Gpr::R(_))
                    && matches!(index.gpr, Gpr::R(_)) =>
            {
                Ok(Sve::Access {
                    store,
                    zt,
                    pg,
                    base,
                    index,
                })
            }
            _ => Err(self.problem(
                "its address is `[x1, x2]`: a base and an index, both x registers, neither sp \
                 nor xzr",
            )),
        }
    }

    /// `cmp<cc> p1.b, p0/z, z0.b, z1.b` or with a number. Of two vectors `lo`, `ls`, `lt` and
    /// `le` are `hi`, `hs`, `gt` and `ge` with the vectors swapped, as LLVM writes them.
    fn sve_compare(&self, name: &str) -> Result<Sve, String> {
        self.count(4)?;
        let cmp = Cmp::named(name).ok_or_else(|| self.problem("no such comparison"))?;
        let (pd, pg, zn) = (
            self.p_at(0)?,
            self.governing_at(1, 7, false)?,
            self.z_at(2)?,
        );
        let last = self.operands[3];
        if last.starts_with('#') {
            let number = if matches!(cmp, Cmp::Hs | Cmp::Hi | Cmp::Lo | Cmp::Ls) {
                self.sve_number(3, 0, 127)?
            } else {
                self.sve_number(3, -16, 15)?
            };
            return Ok(Sve::Compare {
                cmp,
                pd,
                pg,
                zn,
                against: Against::Number(number as i32),
            });
        }
        let zm = self.z_at(3)?;
        let (cmp, zn, zm) = match cmp {
            Cmp::Lo | Cmp::Ls | Cmp::Lt | Cmp::Le => (cmp.swapped(), zm, zn),
            _ => (cmp, zn, zm),
        };
        Ok(Sve::Compare {
            cmp,
            pd,
            pg,
            zn,
            against: Against::Vector(zm),
        })
    }

    /// The `mov`s of Z registers of bytes, and `dup` and `cpy` as written: a number or a w
    /// register in every lane, a number in the active lanes, or a vector's bytes in them.
    fn sve_mov(&self) -> Result<Sve, String> {
        let zd = self.z_at(0)?;
        if self.operands.len() == 2 {
            let source = self.operands[1];
            if source.starts_with('#') {
                return Ok(Sve::DupImm {
                    zd,
                    imm: self.sve_byte(1)?,
                });
            }
            return Ok(Sve::DupReg {
                zd,
                rn: self.general_at(1, false, false)?,
            });
        }
        self.count(3)?;
        let (pg, merge) = self
            .operands
            .get(1)
            .and_then(|operand| governing(operand))
            .filter(|(number, _)| *number <= 15)
            .ok_or_else(|| {
                self.problem("operand 2 is a governing predicate, p0/z to p15/z or p0/m to p15/m")
            })?;
        if self.operands[2].starts_with('#') {
            return Ok(Sve::Cpy {
                zd,
                pg,
                merge,
                imm: self.sve_byte(2)?,
            });
        }
        if !merge {
            return Err(self.problem(
                "a vector into the active lanes keeps the others: `mov z0.b, p0/m, z1.b`",
            ));
        }
        Ok(Sve::Sel {
            zd,
            pg,
            zn: self.z_at(2)?,
            zm: zd,
        })
    }
}
