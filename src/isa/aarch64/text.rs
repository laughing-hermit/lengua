//! The dialect's AArch64 instructions as main.s and LLVM's disassembler write them, read into
//! their canonical form: every alias the assemblers accept or print (`mov`, `cmp`, `lsl`, `cset`,
//! `mul`, …) becomes the instruction it stands for, and a number becomes the encoding the
//! assembler picks for it, so a line of main.s and the machine word it became can be compared
//! field by field.

use super::decode::bitmask;
use super::{
    A64Cond, A64Reg, Access, Address, Extend, Field, FpConversion, FpOne, FpReg, FpThree, FpTwo,
    Gpr, Indirect, Inst, Logic, One, Select, Shift, Three, Two, Wide,
};

/// What a name in an operand stands for: a constant's value, or a label's or data symbol's
/// address.
pub type Names<'a> = &'a dyn Fn(&str) -> Option<i64>;

/// The instruction `mnemonic operands`, at `address`: a target written `#offset`, as LLVM's
/// disassembler writes one, is relative to it, and one written as a name is the name's address.
///
/// # Errors
///
/// Why the text isn't an instruction of the dialect.
pub fn parse(
    mnemonic: &str,
    operands: &str,
    address: u64,
    names: Names<'_>,
) -> Result<Inst, String> {
    let operands = split(operands)?;
    let line = Line {
        mnemonic,
        operands: &operands,
        address,
        names,
    };
    line.instruction()
}

/// The operands, split at the commas outside brackets.
fn split(text: &str) -> Result<Vec<&str>, String> {
    let mut found = Vec::new();
    let (mut depth, mut start) = (0_i32, 0);
    for (at, character) in text.char_indices() {
        match character {
            '[' => depth += 1,
            ']' => depth -= 1,
            ',' if depth == 0 => {
                found.push(text[start..at].trim());
                start = at + 1;
            }
            _ => {}
        }
        if depth < 0 {
            return Err("a `]` has no `[` before it".into());
        }
    }
    if depth != 0 {
        return Err("a `[` isn't closed".into());
    }
    let last = text[start..].trim();
    if !last.is_empty() || !found.is_empty() {
        found.push(last);
    }
    Ok(found)
}

/// A register by its name: `x0` to `x30`, `w0` to `w30`, `sp`, `wsp`, `xzr` or `wzr`.
#[must_use]
pub fn register(name: &str) -> Option<A64Reg> {
    let (wide, rest) = match name.as_bytes().first()? {
        b'x' => (true, &name[1..]),
        b'w' => (false, &name[1..]),
        _ => {
            return match name {
                "sp" => Some(A64Reg::new(31, true, true)),
                _ => None,
            };
        }
    };
    match rest {
        "zr" => Some(A64Reg::new(31, wide, false)),
        "sp" if !wide => Some(A64Reg::new(31, false, true)),
        digits if !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) => {
            let number: u32 = digits.parse().ok()?;
            (number <= 30 && (digits == "0" || !digits.starts_with('0')))
                .then(|| A64Reg::new(number, wide, false))
        }
        _ => None,
    }
}

/// A floating-point register by its name: `s0` to `s31`, or `d0` to `d31`.
#[must_use]
pub fn fp_register(name: &str) -> Option<FpReg> {
    let (double, digits) = match name.as_bytes().first()? {
        b's' => (false, &name[1..]),
        b'd' => (true, &name[1..]),
        _ => return None,
    };
    if digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || (digits != "0" && digits.starts_with('0'))
    {
        return None;
    }
    let number: u8 = digits.parse().ok()?;
    (number <= 31).then_some(FpReg { number, double })
}

/// The register's width in bits.
const fn width(register: A64Reg) -> u32 {
    if register.wide { 64 } else { 32 }
}

/// All ones in the low `width` bits.
const fn ones(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1 << width) - 1
    }
}

/// Whether `value` is a number a logical instruction can hold at `width` bits.
fn encodable(value: u64, width: u32) -> bool {
    (0..2).any(|n| {
        (0..64).any(|immr| (0..64).any(|imms| bitmask(n == 1, immr, imms, width) == Some(value)))
    })
}

/// A number written as a literal: decimal, hexadecimal or a character in quotes, maybe negated.
fn literal(text: &str) -> Option<i64> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest.trim()),
        None => (false, text),
    };
    let value = if let Some(hex) = digits.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).ok()?.cast_signed()
    } else if digits.starts_with(|first: char| first.is_ascii_digit()) {
        digits.parse::<i64>().ok()?
    } else {
        let quoted = digits.strip_prefix('\'')?.strip_suffix('\'')?;
        let mut characters = quoted.chars();
        match (characters.next(), characters.next()) {
            (Some(character), None) if character.is_ascii() => i64::from(u32::from(character)),
            _ => return None,
        }
    };
    Some(if negative {
        value.wrapping_neg()
    } else {
        value
    })
}

/// One instruction's text.
struct Line<'a> {
    mnemonic: &'a str,
    operands: &'a [&'a str],
    address: u64,
    names: Names<'a>,
}

impl Line<'_> {
    fn problem(&self, why: &str) -> String {
        format!("`{} {}`: {why}", self.mnemonic, self.operands.join(", "))
    }

    /// Exactly `count` operands.
    fn count(&self, count: usize) -> Result<(), String> {
        if self.operands.len() == count {
            Ok(())
        } else {
            Err(self.problem(&format!("it takes {count} operands")))
        }
    }

    /// Operand `at`, as a register.
    fn reg(&self, at: usize) -> Result<A64Reg, String> {
        self.operands
            .get(at)
            .and_then(|operand| register(operand))
            .ok_or_else(|| self.problem(&format!("operand {} isn't a register", at + 1)))
    }

    /// Operand `at`, a floating-point register.
    fn fp(&self, at: usize) -> Result<FpReg, String> {
        let operand = self.operands.get(at).copied().unwrap_or_default();
        fp_register(operand).ok_or_else(|| {
            // The other views of a vector register: its byte, half, and whole 128 bits.
            let other = |kind: char| {
                operand.strip_prefix(kind).is_some_and(|digits| {
                    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
                })
            };
            self.problem(&if other('h') || other('b') {
                format!(
                    "operand {} is a half-precision or byte register, which the dialect doesn't \
                     have: compute in s (single) or d (double) registers",
                    at + 1
                )
            } else if other('q') || other('v') {
                format!(
                    "operand {} is a whole vector register, which the dialect doesn't have yet: \
                     use s or d registers",
                    at + 1
                )
            } else {
                format!("operand {} isn't a floating-point register", at + 1)
            })
        })
    }

    /// Operand `at`, a floating-point register as wide as `double` says.
    fn fp_as(&self, at: usize, double: bool) -> Result<FpReg, String> {
        let found = self.fp(at)?;
        if found.double == double {
            Ok(found)
        } else {
            Err(self.problem("its registers aren't all one width"))
        }
    }

    /// The line as a floating-point instruction, or none where it isn't one: its mnemonic is a
    /// floating-point operation's, or it loads or stores an `s` or `d` register.
    fn floating(&self) -> Option<Result<Inst, String>> {
        let first_fp = self
            .operands
            .first()
            .is_some_and(|operand| fp_register(operand).is_some());
        Some(match self.mnemonic {
            "ldr" | "str" | "ldur" | "stur" if first_fp => self.fp_load_store(),
            "ldp" | "stp" if first_fp => self.fp_pair(),
            "fmov" => self.fmov(),
            "fabs" | "fneg" | "fsqrt" | "fcvt" | "frintn" | "frintp" | "frintm" | "frintz"
            | "frinta" => self.fp_one(),
            "fmul" | "fdiv" | "fadd" | "fsub" | "fmax" | "fmin" | "fmaxnm" | "fminnm" | "fnmul" => {
                self.fp_two()
            }
            "fmadd" | "fmsub" | "fnmadd" | "fnmsub" => self.fp_three(),
            "fcmp" | "fcmpe" => self.fp_compare(),
            "fcsel" => self.fp_select(),
            "scvtf" | "ucvtf" | "fcvtzs" | "fcvtzu" => self.fp_convert(),
            _ => return None,
        })
    }

    fn fp_one(&self) -> Result<Inst, String> {
        self.count(2)?;
        let op = match self.mnemonic {
            "fabs" => FpOne::Abs,
            "fneg" => FpOne::Neg,
            "fsqrt" => FpOne::Sqrt,
            "fcvt" => FpOne::Cvt,
            "frintn" => FpOne::RintNearest,
            "frintp" => FpOne::RintUp,
            "frintm" => FpOne::RintDown,
            "frintz" => FpOne::RintZero,
            _ => FpOne::RintAway,
        };
        let rd = self.fp(0)?;
        let rn = if op == FpOne::Cvt {
            // To the other width.
            self.fp_as(1, !rd.double)?
        } else {
            self.fp_as(1, rd.double)?
        };
        Ok(Inst::FpOneSource { op, rd, rn })
    }

    fn fp_two(&self) -> Result<Inst, String> {
        self.count(3)?;
        let op = match self.mnemonic {
            "fmul" => FpTwo::Mul,
            "fdiv" => FpTwo::Div,
            "fadd" => FpTwo::Add,
            "fsub" => FpTwo::Sub,
            "fmax" => FpTwo::Max,
            "fmin" => FpTwo::Min,
            "fmaxnm" => FpTwo::MaxNm,
            "fminnm" => FpTwo::MinNm,
            _ => FpTwo::Nmul,
        };
        let rd = self.fp(0)?;
        Ok(Inst::FpTwoSource {
            op,
            rd,
            rn: self.fp_as(1, rd.double)?,
            rm: self.fp_as(2, rd.double)?,
        })
    }

    fn fp_three(&self) -> Result<Inst, String> {
        self.count(4)?;
        let op = match self.mnemonic {
            "fmadd" => FpThree::Madd,
            "fmsub" => FpThree::Msub,
            "fnmadd" => FpThree::Nmadd,
            _ => FpThree::Nmsub,
        };
        let rd = self.fp(0)?;
        Ok(Inst::FpThreeSource {
            op,
            rd,
            rn: self.fp_as(1, rd.double)?,
            rm: self.fp_as(2, rd.double)?,
            ra: self.fp_as(3, rd.double)?,
        })
    }

    fn fp_compare(&self) -> Result<Inst, String> {
        self.count(2)?;
        let rn = self.fp(0)?;
        let rm = match self.operands.get(1).copied() {
            Some("#0.0" | "#0") => None,
            _ => Some(self.fp_as(1, rn.double)?),
        };
        Ok(Inst::FpCompare {
            rn,
            rm,
            signaling: self.mnemonic == "fcmpe",
        })
    }

    fn fp_select(&self) -> Result<Inst, String> {
        self.count(4)?;
        let rd = self.fp(0)?;
        Ok(Inst::FpCondSelect {
            rd,
            rn: self.fp_as(1, rd.double)?,
            rm: self.fp_as(2, rd.double)?,
            cond: self.cond(3)?,
        })
    }

    /// `fmov` between floating-point registers, of a number, or of the bits to or from a
    /// general register as wide.
    fn fmov(&self) -> Result<Inst, String> {
        self.count(2)?;
        let (first, second) = (self.operands[0], self.operands[1]);
        if let Some(rd) = fp_register(first) {
            if let Some(rn) = fp_register(second) {
                if rn.double != rd.double {
                    return Err(self.problem("its registers aren't all one width"));
                }
                return Ok(Inst::FpOneSource {
                    op: FpOne::Mov,
                    rd,
                    rn,
                });
            }
            if let Some(gpr) = register(second) {
                if gpr.wide != rd.double {
                    return Err(self.problem("its registers aren't all one width"));
                }
                return Ok(Inst::FpConvert {
                    op: FpConversion::FromGeneral,
                    fp: rd,
                    gpr,
                });
            }
            let value: f64 = second
                .strip_prefix('#')
                .and_then(|number| number.parse().ok())
                .ok_or_else(|| self.problem("operand 2 isn't a register or a number"))?;
            let imm8 = (0..=255_u8)
                .find(|imm8| super::fp_immediate(*imm8) == value)
                .ok_or_else(|| self.problem("its number isn't one fmov can hold"))?;
            return Ok(Inst::FpMoveImm { rd, imm8 });
        }
        let gpr = self.reg(0)?;
        let fp = self.fp(1)?;
        if gpr.wide != fp.double {
            return Err(self.problem("its registers aren't all one width"));
        }
        Ok(Inst::FpConvert {
            op: FpConversion::ToGeneral,
            fp,
            gpr,
        })
    }

    fn fp_convert(&self) -> Result<Inst, String> {
        self.count(2)?;
        let to_fp = matches!(self.mnemonic, "scvtf" | "ucvtf");
        let op = match self.mnemonic {
            "scvtf" => FpConversion::Scvtf,
            "ucvtf" => FpConversion::Ucvtf,
            "fcvtzs" => FpConversion::Fcvtzs,
            _ => FpConversion::Fcvtzu,
        };
        let (fp, gpr) = if to_fp {
            (self.fp(0)?, self.reg(1)?)
        } else {
            (self.fp(1)?, self.reg(0)?)
        };
        Ok(Inst::FpConvert { op, fp, gpr })
    }

    fn fp_load_store(&self) -> Result<Inst, String> {
        let unscaled = self.mnemonic.starts_with("ldur") || self.mnemonic.starts_with("stur");
        let load = self.mnemonic.starts_with("ld");
        let rt = self.fp(0)?;
        let address = self.address(1, if rt.double { 8 } else { 4 })?;
        if unscaled && !matches!(address, Address::Offset { .. }) {
            return Err(self.problem("an unscaled access takes only an offset"));
        }
        Ok(Inst::FpLoadStore { load, rt, address })
    }

    fn fp_pair(&self) -> Result<Inst, String> {
        let load = self.mnemonic == "ldp";
        let rt = self.fp(0)?;
        let rt2 = self.fp_as(1, rt.double)?;
        let address = self.address(2, if rt.double { 8 } else { 4 })?;
        if matches!(address, Address::Register { .. }) {
            return Err(self.problem("a pair's address takes only an offset"));
        }
        if load && rt.number == rt2.number {
            return Err(self.problem("it loads one register twice, which has no defined result"));
        }
        Ok(Inst::FpPair {
            load,
            rt,
            rt2,
            address,
        })
    }

    /// Operand `at`, as a register of `wide`'s width.
    fn reg_as(&self, at: usize, wide: bool) -> Result<A64Reg, String> {
        let found = self.reg(at)?;
        if found.wide == wide {
            Ok(found)
        } else {
            Err(self.problem("its registers aren't all one width"))
        }
    }

    /// A number written `#…`: decimal, hexadecimal or a character in quotes, maybe negated; or
    /// anything else the names stand for, such as a constant's name or an expression of them.
    fn number(&self, text: &str) -> Result<i64, String> {
        let bad = || self.problem(&format!("`{text}` isn't a number"));
        let text = text.strip_prefix('#').ok_or_else(bad)?.trim();
        match literal(text) {
            Some(value) => Ok(value),
            None => (self.names)(text).ok_or_else(bad),
        }
    }

    /// Operand `at`, as a number.
    fn imm(&self, at: usize) -> Result<i64, String> {
        let text = self
            .operands
            .get(at)
            .ok_or_else(|| self.problem(&format!("it has no operand {}", at + 1)))?;
        self.number(text)
    }

    /// Operand `at`, as a number from 0 to `most`.
    fn small(&self, at: usize, most: i64) -> Result<u32, String> {
        let value = self.imm(at)?;
        u32::try_from(value)
            .ok()
            .filter(|_| value <= most)
            .ok_or_else(|| self.problem(&format!("{value} isn't from 0 to {most}")))
    }

    /// Operand `at`, where a branch goes: `#offset` from this instruction, or a label.
    fn target(&self, at: usize) -> Result<u64, String> {
        let text = self
            .operands
            .get(at)
            .ok_or_else(|| self.problem("it has no target"))?;
        if text.starts_with('#') {
            return Ok(self.address.wrapping_add_signed(self.number(text)?));
        }
        (self.names)(text)
            .map(i64::cast_unsigned)
            .ok_or_else(|| self.problem(&format!("`{text}` isn't a label")))
    }

    /// Operand `at`, a shift such as `lsl #12`, or none.
    fn shift(&self, at: usize) -> Result<Option<(Shift, u32)>, String> {
        let Some(text) = self.operands.get(at) else {
            return Ok(None);
        };
        let (name, amount) = text
            .split_once(char::is_whitespace)
            .ok_or_else(|| self.problem(&format!("`{text}` isn't a shift")))?;
        let shift = match name {
            "lsl" => Shift::Lsl,
            "lsr" => Shift::Lsr,
            "asr" => Shift::Asr,
            "ror" => Shift::Ror,
            _ => return Err(self.problem(&format!("`{text}` isn't a shift"))),
        };
        let amount = self.number(amount.trim())?;
        let amount = u32::try_from(amount)
            .map_err(|_| self.problem(&format!("`{text}` shifts by {amount}")))?;
        Ok(Some((shift, amount)))
    }

    /// Operand `at`, a condition.
    fn cond(&self, at: usize) -> Result<A64Cond, String> {
        self.operands
            .get(at)
            .and_then(|name| A64Cond::named(name))
            .ok_or_else(|| self.problem(&format!("operand {} isn't a condition", at + 1)))
    }

    fn instruction(&self) -> Result<Inst, String> {
        if let Some(floating) = self.floating() {
            return floating;
        }
        let mnemonic = self.mnemonic;
        if let Some(name) = mnemonic.strip_prefix("b.") {
            self.count(1)?;
            let cond = A64Cond::named(name).ok_or_else(|| self.problem("no such condition"))?;
            return Ok(Inst::BranchCond {
                cond,
                target: self.target(0)?,
            });
        }
        match mnemonic {
            "add" | "adds" | "sub" | "subs" => self.add(),
            "cmp" | "cmn" => self.compare(),
            "neg" | "negs" => self.negate(),
            "mov" => self.mov(),
            "movz" | "movn" | "movk" => self.move_wide(),
            "and" | "ands" | "orr" | "eor" | "bic" | "bics" | "orn" | "eon" => self.logical(),
            "tst" => self.test(),
            "mvn" => self.not(),
            "lsl" | "lsr" | "asr" | "ror" => self.shift_alias(),
            "sbfm" | "bfm" | "ubfm" => self.bitfield(),
            "ubfx" | "sbfx" | "ubfiz" | "sbfiz" | "bfi" | "bfxil" | "bfc" => self.field(),
            "uxtb" | "uxth" | "sxtb" | "sxth" | "sxtw" => self.extend(),
            "extr" => {
                self.count(4)?;
                let rd = self.reg(0)?;
                Ok(Inst::Extract {
                    rd,
                    rn: self.reg_as(1, rd.wide)?,
                    rm: self.reg_as(2, rd.wide)?,
                    lsb: u8::try_from(self.small(3, i64::from(width(rd)) - 1)?)
                        .map_err(|_| self.problem("too far"))?,
                })
            }
            "adrp" => {
                self.count(2)?;
                let rd = self.reg_as(0, true)?;
                let text = self.operands[1];
                let page = if text.starts_with('#') {
                    (self.address & !0xfff).wrapping_add_signed(self.number(text)?)
                } else {
                    self.target(1)? & !0xfff
                };
                Ok(Inst::Adrp { rd, page })
            }
            "adr" => {
                self.count(2)?;
                Ok(Inst::Adr {
                    rd: self.reg_as(0, true)?,
                    target: self.target(1)?,
                })
            }
            "b" | "bl" => {
                self.count(1)?;
                Ok(Inst::Branch {
                    link: mnemonic == "bl",
                    target: self.target(0)?,
                })
            }
            "cbz" | "cbnz" => {
                self.count(2)?;
                Ok(Inst::CompareBranch {
                    nonzero: mnemonic == "cbnz",
                    rt: self.reg(0)?,
                    target: self.target(1)?,
                })
            }
            "tbz" | "tbnz" => {
                self.count(3)?;
                let rt = self.reg(0)?;
                let bit = self.small(1, i64::from(width(rt)) - 1)?;
                // A bit below 32 is written on the w register, as the encoding has it.
                Ok(Inst::TestBranch {
                    nonzero: mnemonic == "tbnz",
                    rt: A64Reg {
                        wide: bit >= 32,
                        ..rt
                    },
                    bit: u8::try_from(bit).map_err(|_| self.problem("no such bit"))?,
                    target: self.target(2)?,
                })
            }
            "br" | "blr" | "ret" => {
                let kind = match mnemonic {
                    "br" => Indirect::Br,
                    "blr" => Indirect::Blr,
                    _ => Indirect::Ret,
                };
                let rn = if self.operands.is_empty() && kind == Indirect::Ret {
                    A64Reg::new(30, true, false)
                } else {
                    self.count(1)?;
                    self.reg_as(0, true)?
                };
                Ok(Inst::BranchReg { kind, rn })
            }
            "svc" => {
                self.count(1)?;
                Ok(Inst::Svc {
                    imm: u16::try_from(self.small(0, 0xffff)?)
                        .map_err(|_| self.problem("too big"))?,
                })
            }
            "nop" => {
                self.count(0)?;
                Ok(Inst::Nop)
            }
            "csel" | "csinc" | "csinv" | "csneg" => {
                self.count(4)?;
                let op = match mnemonic {
                    "csel" => Select::Csel,
                    "csinc" => Select::Csinc,
                    "csinv" => Select::Csinv,
                    _ => Select::Csneg,
                };
                let rd = self.reg(0)?;
                Ok(Inst::CondSelect {
                    op,
                    rd,
                    rn: self.reg_as(1, rd.wide)?,
                    rm: self.reg_as(2, rd.wide)?,
                    cond: self.cond(3)?,
                })
            }
            "cset" | "csetm" | "cinc" | "cinv" | "cneg" => self.select(),
            "udiv" | "sdiv" | "lslv" | "lsrv" | "asrv" | "rorv" => {
                self.count(3)?;
                let op = match mnemonic {
                    "udiv" => Two::Udiv,
                    "sdiv" => Two::Sdiv,
                    "lslv" => Two::Lslv,
                    "lsrv" => Two::Lsrv,
                    "asrv" => Two::Asrv,
                    _ => Two::Rorv,
                };
                let rd = self.reg(0)?;
                Ok(Inst::TwoSource {
                    op,
                    rd,
                    rn: self.reg_as(1, rd.wide)?,
                    rm: self.reg_as(2, rd.wide)?,
                })
            }
            "rbit" | "rev16" | "rev32" | "rev" | "clz" | "cls" => {
                self.count(2)?;
                let op = match mnemonic {
                    "rbit" => One::Rbit,
                    "rev16" => One::Rev16,
                    "rev32" => One::Rev32,
                    "rev" => One::Rev,
                    "clz" => One::Clz,
                    _ => One::Cls,
                };
                let rd = self.reg(0)?;
                if op == One::Rev32 && !rd.wide {
                    return Err(self.problem("rev32 takes x registers"));
                }
                Ok(Inst::OneSource {
                    op,
                    rd,
                    rn: self.reg_as(1, rd.wide)?,
                })
            }
            "madd" | "msub" | "smaddl" | "smsubl" | "umaddl" | "umsubl" | "smulh" | "umulh" => {
                self.three()
            }
            "mul" | "mneg" | "smull" | "umull" | "smnegl" | "umnegl" => self.product(),
            "ldp" | "stp" | "ldpsw" => self.pair(),
            _ => self.load_store(),
        }
    }

    /// `add`/`sub`/`adds`/`subs`, with a number or a shifted register.
    fn add(&self) -> Result<Inst, String> {
        let (sub, flags) = match self.mnemonic {
            "add" => (false, false),
            "adds" => (false, true),
            "sub" => (true, false),
            _ => (true, true),
        };
        let rd = self.reg(0)?;
        let rn = self.reg_as(1, rd.wide)?;
        self.arithmetic(sub, flags, rd, rn, 2)
    }

    /// What follows the first operand of an `add` or `sub`, from operand `at`: a number
    /// (`:lo12:name` among them) or a shifted register.
    fn arithmetic(
        &self,
        sub: bool,
        flags: bool,
        rd: A64Reg,
        rn: A64Reg,
        at: usize,
    ) -> Result<Inst, String> {
        let operand = self
            .operands
            .get(at)
            .ok_or_else(|| self.problem("it has too few operands"))?;
        if let Some(rm) = register(operand) {
            if rm.wide != rd.wide || rd.gpr == Gpr::Sp || rn.gpr == Gpr::Sp {
                return Err(
                    self.problem("it adds an extended register, which isn't in the dialect")
                );
            }
            let (shift, amount) = self.shift(at + 1)?.unwrap_or((Shift::Lsl, 0));
            if shift == Shift::Ror || amount >= width(rd) || self.operands.len() > at + 2 {
                return Err(self.problem("its shift isn't one an add takes"));
            }
            return Ok(Inst::AddShifted {
                sub,
                flags,
                rd,
                rn,
                rm,
                shift,
                amount: u8::try_from(amount).map_err(|_| self.problem("too far"))?,
            });
        }
        let low = operand.strip_prefix(":lo12:");
        let value = match low {
            Some(name) => (self.names)(name)
                .map(|address| address & 0xfff)
                .ok_or_else(|| self.problem(&format!("`{name}` isn't a data symbol")))?,
            None => self.number(operand)?,
        };
        let explicit = self.shift(at + 1)?;
        if self.operands.len() > at + 2 {
            return Err(self.problem("it has too many operands"));
        }
        // A negative number is the opposite operation's, as the assembler writes it.
        let (sub, value) = if value < 0 && low.is_none() {
            (!sub, value.wrapping_neg())
        } else {
            (sub, value)
        };
        let (imm, shift12) = match explicit {
            Some((Shift::Lsl, 12)) => (value, true),
            Some((Shift::Lsl, 0)) | None if value <= 0xfff => (value, false),
            None if value & 0xfff == 0 && value >> 12 <= 0xfff => (value >> 12, true),
            _ => return Err(self.problem(&format!("{value} isn't a number an add can hold"))),
        };
        let imm = u32::try_from(imm)
            .ok()
            .filter(|imm| *imm <= 0xfff)
            .ok_or_else(|| self.problem(&format!("{value} isn't a number an add can hold")))?;
        Ok(Inst::AddImm {
            sub,
            flags,
            rd,
            rn,
            imm,
            shift12,
        })
    }

    /// `cmp` and `cmn`: a subtraction or addition whose result goes nowhere.
    fn compare(&self) -> Result<Inst, String> {
        let rn = self.reg(0)?;
        let zero = A64Reg::new(31, rn.wide, false);
        self.arithmetic(self.mnemonic == "cmp", true, zero, rn, 1)
    }

    /// `neg` and `negs`: zero less a shifted register.
    fn negate(&self) -> Result<Inst, String> {
        let rd = self.reg(0)?;
        let rm = self.reg_as(1, rd.wide)?;
        let (shift, amount) = self.shift(2)?.unwrap_or((Shift::Lsl, 0));
        Ok(Inst::AddShifted {
            sub: true,
            flags: self.mnemonic == "negs",
            rd,
            rn: A64Reg::new(31, rd.wide, false),
            rm,
            shift,
            amount: u8::try_from(amount).map_err(|_| self.problem("too far"))?,
        })
    }

    /// `mov`: a number (as `movz`, `movn` or `orr`), a register, or to or from the stack
    /// pointer (as `add #0`).
    fn mov(&self) -> Result<Inst, String> {
        self.count(2)?;
        let rd = self.reg(0)?;
        if let Some(rn) = register(self.operands[1]) {
            if rn.wide != rd.wide {
                return Err(self.problem("its registers aren't all one width"));
            }
            if rd.gpr == Gpr::Sp || rn.gpr == Gpr::Sp {
                return Ok(Inst::AddImm {
                    sub: false,
                    flags: false,
                    rd,
                    rn,
                    imm: 0,
                    shift12: false,
                });
            }
            return Ok(Inst::LogicalShifted {
                op: Logic::Orr,
                invert: false,
                rd,
                rn: A64Reg::new(31, rd.wide, false),
                rm: rn,
                shift: Shift::Lsl,
                amount: 0,
            });
        }
        let value = self.imm(1)?.cast_unsigned() & ones(width(rd));
        let fits = |value: u64| {
            (0..width(rd)).step_by(16).find_map(|shift| {
                (value & !(0xffff << shift) == 0).then(|| {
                    // At most 16 bits, by the test above.
                    #[allow(clippy::cast_possible_truncation)]
                    let imm = (value >> shift) as u16;
                    (imm, u8::try_from(shift).unwrap_or(0))
                })
            })
        };
        if let Some((imm, shift)) = fits(value) {
            return Ok(Inst::MoveWide {
                op: Wide::Movz,
                rd,
                imm,
                shift,
            });
        }
        if let Some((imm, shift)) = fits(!value & ones(width(rd))) {
            return Ok(Inst::MoveWide {
                op: Wide::Movn,
                rd,
                imm,
                shift,
            });
        }
        if encodable(value, width(rd)) {
            return Ok(Inst::LogicalImm {
                op: Logic::Orr,
                rd,
                rn: A64Reg::new(31, rd.wide, false),
                imm: value,
            });
        }
        Err(self.problem(&format!(
            "{value:#x} isn't a number one instruction can move; build it with movz and movk"
        )))
    }

    /// `movz`, `movn` and `movk`, written out.
    fn move_wide(&self) -> Result<Inst, String> {
        let op = match self.mnemonic {
            "movz" => Wide::Movz,
            "movn" => Wide::Movn,
            _ => Wide::Movk,
        };
        let rd = self.reg(0)?;
        let imm = u16::try_from(self.small(1, 0xffff)?).map_err(|_| self.problem("too big"))?;
        let shift = match self.shift(2)? {
            None => 0,
            Some((Shift::Lsl, shift)) if shift % 16 == 0 && shift < width(rd) => shift,
            _ => return Err(self.problem("it shifts by 0, 16, 32 or 48")),
        };
        if self.operands.len() > 3 {
            return Err(self.problem("it has too many operands"));
        }
        Ok(Inst::MoveWide {
            op,
            rd,
            imm,
            shift: u8::try_from(shift).map_err(|_| self.problem("too far"))?,
        })
    }

    /// `and`, `orr`, `eor`, `ands` and their inverting forms, with a bitmask or a shifted
    /// register.
    fn logical(&self) -> Result<Inst, String> {
        let (op, invert) = match self.mnemonic {
            "and" => (Logic::And, false),
            "bic" => (Logic::And, true),
            "orr" => (Logic::Orr, false),
            "orn" => (Logic::Orr, true),
            "eor" => (Logic::Eor, false),
            "eon" => (Logic::Eor, true),
            "ands" => (Logic::Ands, false),
            _ => (Logic::Ands, true),
        };
        let rd = self.reg(0)?;
        let rn = self.reg_as(1, rd.wide)?;
        self.logic_operand(op, invert, rd, rn, 2)
    }

    /// What follows a logical instruction's first operands, from operand `at`.
    fn logic_operand(
        &self,
        op: Logic,
        invert: bool,
        rd: A64Reg,
        rn: A64Reg,
        at: usize,
    ) -> Result<Inst, String> {
        let operand = self
            .operands
            .get(at)
            .ok_or_else(|| self.problem("it has too few operands"))?;
        if let Some(rm) = register(operand) {
            if rm.wide != rd.wide {
                return Err(self.problem("its registers aren't all one width"));
            }
            let (shift, amount) = self.shift(at + 1)?.unwrap_or((Shift::Lsl, 0));
            if amount >= width(rd) || self.operands.len() > at + 2 {
                return Err(self.problem("its shift isn't one it takes"));
            }
            return Ok(Inst::LogicalShifted {
                op,
                invert,
                rd,
                rn,
                rm,
                shift,
                amount: u8::try_from(amount).map_err(|_| self.problem("too far"))?,
            });
        }
        if invert || self.operands.len() > at + 1 {
            return Err(self.problem("its number isn't one it takes"));
        }
        let imm = self.number(operand)?.cast_unsigned() & ones(width(rd));
        if !encodable(imm, width(rd)) {
            return Err(self.problem(&format!(
                "{imm:#x} isn't a bitmask a logical instruction can hold"
            )));
        }
        Ok(Inst::LogicalImm { op, rd, rn, imm })
    }

    /// `tst`: `ands` whose result goes nowhere.
    fn test(&self) -> Result<Inst, String> {
        let rn = self.reg(0)?;
        let zero = A64Reg::new(31, rn.wide, false);
        self.logic_operand(Logic::Ands, false, zero, rn, 1)
    }

    /// `mvn`: `orn` from zero.
    fn not(&self) -> Result<Inst, String> {
        let rd = self.reg(0)?;
        let zero = A64Reg::new(31, rd.wide, false);
        self.logic_operand(Logic::Orr, true, rd, zero, 1)
    }

    /// `lsl`, `lsr`, `asr` and `ror`, by a number (a bitfield or extract) or by a register.
    fn shift_alias(&self) -> Result<Inst, String> {
        self.count(3)?;
        let rd = self.reg(0)?;
        let rn = self.reg_as(1, rd.wide)?;
        if let Some(rm) = register(self.operands[2]) {
            if rm.wide != rd.wide {
                return Err(self.problem("its registers aren't all one width"));
            }
            let op = match self.mnemonic {
                "lsl" => Two::Lslv,
                "lsr" => Two::Lsrv,
                "asr" => Two::Asrv,
                _ => Two::Rorv,
            };
            return Ok(Inst::TwoSource { op, rd, rn, rm });
        }
        let bits = width(rd);
        let amount = self.small(2, i64::from(bits) - 1)?;
        let byte = |value: u32| u8::try_from(value).map_err(|_| self.problem("too far"));
        Ok(match self.mnemonic {
            "lsl" => Inst::Bitfield {
                op: Field::Ubfm,
                rd,
                rn,
                immr: byte((bits - amount) % bits)?,
                imms: byte(bits - 1 - amount)?,
            },
            "lsr" | "asr" => Inst::Bitfield {
                op: if self.mnemonic == "lsr" {
                    Field::Ubfm
                } else {
                    Field::Sbfm
                },
                rd,
                rn,
                immr: byte(amount)?,
                imms: byte(bits - 1)?,
            },
            _ => Inst::Extract {
                rd,
                rn,
                rm: rn,
                lsb: byte(amount)?,
            },
        })
    }

    /// `sbfm`, `bfm` and `ubfm`, written out.
    fn bitfield(&self) -> Result<Inst, String> {
        self.count(4)?;
        let op = match self.mnemonic {
            "sbfm" => Field::Sbfm,
            "bfm" => Field::Bfm,
            _ => Field::Ubfm,
        };
        let rd = self.reg(0)?;
        let most = i64::from(width(rd)) - 1;
        let byte = |value: u32| u8::try_from(value).map_err(|_| self.problem("too far"));
        Ok(Inst::Bitfield {
            op,
            rd,
            rn: self.reg_as(1, rd.wide)?,
            immr: byte(self.small(2, most)?)?,
            imms: byte(self.small(3, most)?)?,
        })
    }

    /// The bitfield aliases that name a field by its lowest bit and width: `ubfx`, `sbfx`,
    /// `ubfiz`, `sbfiz`, `bfi`, `bfxil` and `bfc`.
    fn field(&self) -> Result<Inst, String> {
        let rd = self.reg(0)?;
        let bits = width(rd);
        let (rn, from) = if self.mnemonic == "bfc" {
            self.count(3)?;
            (A64Reg::new(31, rd.wide, false), 1)
        } else {
            self.count(4)?;
            (self.reg_as(1, rd.wide)?, 2)
        };
        let lsb = self.small(from, i64::from(bits) - 1)?;
        let length = self.small(from + 1, i64::from(bits - lsb))?;
        if length == 0 {
            return Err(self.problem("a field has at least one bit"));
        }
        let op = match self.mnemonic {
            "ubfx" | "ubfiz" => Field::Ubfm,
            "sbfx" | "sbfiz" => Field::Sbfm,
            _ => Field::Bfm,
        };
        // Extracting fields start at their lowest bit; inserting ones rotate it into place.
        let (immr, imms) = match self.mnemonic {
            "ubfx" | "sbfx" | "bfxil" => (lsb, lsb + length - 1),
            _ => ((bits - lsb) % bits, length - 1),
        };
        let byte = |value: u32| u8::try_from(value).map_err(|_| self.problem("too far"));
        Ok(Inst::Bitfield {
            op,
            rd,
            rn,
            immr: byte(immr)?,
            imms: byte(imms)?,
        })
    }

    /// `uxtb`, `uxth`, `sxtb`, `sxth` and `sxtw`: a field of a w register, extended.
    fn extend(&self) -> Result<Inst, String> {
        self.count(2)?;
        let rd = self.reg(0)?;
        let source = self.reg_as(1, false)?;
        let (op, imms) = match self.mnemonic {
            "uxtb" => (Field::Ubfm, 7),
            "uxth" => (Field::Ubfm, 15),
            "sxtb" => (Field::Sbfm, 7),
            "sxth" => (Field::Sbfm, 15),
            _ => (Field::Sbfm, 31),
        };
        if (op == Field::Ubfm && rd.wide) || (self.mnemonic == "sxtw" && !rd.wide) {
            return Err(self.problem("its destination isn't the width it extends to"));
        }
        // The encoding names the source at the destination's width.
        Ok(Inst::Bitfield {
            op,
            rd,
            rn: A64Reg {
                wide: rd.wide,
                ..source
            },
            immr: 0,
            imms,
        })
    }

    /// `cset`, `csetm`, `cinc`, `cinv` and `cneg`: a conditional select on the opposite
    /// condition.
    fn select(&self) -> Result<Inst, String> {
        let rd = self.reg(0)?;
        let zero = A64Reg::new(31, rd.wide, false);
        let (op, rn, cond) = match self.mnemonic {
            "cset" | "csetm" => {
                self.count(2)?;
                let op = if self.mnemonic == "cset" {
                    Select::Csinc
                } else {
                    Select::Csinv
                };
                (op, zero, self.cond(1)?)
            }
            _ => {
                self.count(3)?;
                let op = match self.mnemonic {
                    "cinc" => Select::Csinc,
                    "cinv" => Select::Csinv,
                    _ => Select::Csneg,
                };
                (op, self.reg_as(1, rd.wide)?, self.cond(2)?)
            }
        };
        if matches!(cond, A64Cond::Al | A64Cond::Nv) {
            return Err(self.problem("its condition always holds"));
        }
        Ok(Inst::CondSelect {
            op,
            rd,
            rn,
            rm: rn,
            cond: cond.invert(),
        })
    }

    /// The three-source operations, written out.
    fn three(&self) -> Result<Inst, String> {
        let op = match self.mnemonic {
            "madd" => Three::Madd,
            "msub" => Three::Msub,
            "smaddl" => Three::Smaddl,
            "smsubl" => Three::Smsubl,
            "umaddl" => Three::Umaddl,
            "umsubl" => Three::Umsubl,
            "smulh" => Three::Smulh,
            _ => Three::Umulh,
        };
        let rd = self.reg(0)?;
        let factors = !matches!(
            op,
            Three::Smaddl | Three::Smsubl | Three::Umaddl | Three::Umsubl
        );
        let (rn, rm) = (
            self.reg_as(1, factors && rd.wide)?,
            self.reg_as(2, factors && rd.wide)?,
        );
        let ra = match op {
            Three::Smulh | Three::Umulh => {
                self.count(3)?;
                A64Reg::new(31, true, false)
            }
            _ => {
                self.count(4)?;
                self.reg_as(3, rd.wide)?
            }
        };
        if !factors && !rd.wide || matches!(op, Three::Smulh | Three::Umulh) && !rd.wide {
            return Err(self.problem("its destination is an x register"));
        }
        Ok(Inst::ThreeSource { op, rd, rn, rm, ra })
    }

    /// `mul`, `mneg`, `smull`, `umull`, `smnegl` and `umnegl`: products added to zero.
    fn product(&self) -> Result<Inst, String> {
        self.count(3)?;
        let op = match self.mnemonic {
            "mul" => Three::Madd,
            "mneg" => Three::Msub,
            "smull" => Three::Smaddl,
            "umull" => Three::Umaddl,
            "smnegl" => Three::Smsubl,
            _ => Three::Umsubl,
        };
        let rd = self.reg(0)?;
        let lengthens = !matches!(op, Three::Madd | Three::Msub);
        if lengthens && !rd.wide {
            return Err(self.problem("its destination is an x register"));
        }
        let wide = rd.wide && !lengthens;
        Ok(Inst::ThreeSource {
            op,
            rd,
            rn: self.reg_as(1, wide)?,
            rm: self.reg_as(2, wide)?,
            ra: A64Reg::new(31, rd.wide, false),
        })
    }

    /// A memory operand from operand `at`, with a post-index after it, and the register count
    /// before it.
    fn address(&self, at: usize, size: u8) -> Result<Address, String> {
        let text = self
            .operands
            .get(at)
            .ok_or_else(|| self.problem("it has no address"))?;
        let (inside, writeback) = match text.strip_suffix('!') {
            Some(rest) => (rest.trim_end(), true),
            None => (*text, false),
        };
        let inner = inside
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
            .ok_or_else(|| self.problem("its address isn't in brackets"))?;
        let parts = split(inner)?;
        let base = parts
            .first()
            .and_then(|name| register(name))
            .filter(|base| base.wide && base.gpr != Gpr::Zr)
            .ok_or_else(|| self.problem("its address has no x register base"))?;
        let post = self.operands.get(at + 1);
        if post.is_some() && (writeback || parts.len() > 1) {
            return Err(self.problem("its address moves twice"));
        }
        if self.operands.len() > at + 2 {
            return Err(self.problem("it has too many operands"));
        }
        if let Some(post) = post {
            return Ok(Address::PostIndex {
                base,
                offset: self.number(post)?,
            });
        }
        match parts.as_slice() {
            [_] => Ok(if writeback {
                Address::PreIndex { base, offset: 0 }
            } else {
                Address::Offset { base, offset: 0 }
            }),
            [_, offset] if offset.starts_with('#') => {
                let offset = self.number(offset)?;
                Ok(if writeback {
                    Address::PreIndex { base, offset }
                } else {
                    Address::Offset { base, offset }
                })
            }
            [_, index, rest @ ..] if !writeback && rest.len() <= 1 => {
                let index =
                    register(index).ok_or_else(|| self.problem("its index isn't a register"))?;
                let (extend, amount) = match rest.first() {
                    None => (Extend::Lsl, None),
                    Some(text) => {
                        let (name, amount) = match text.split_once(char::is_whitespace) {
                            Some((name, amount)) => (name, Some(self.number(amount.trim())?)),
                            None => (*text, None),
                        };
                        let extend = match name {
                            "lsl" | "uxtx" => Extend::Lsl,
                            "uxtw" => Extend::Uxtw,
                            "sxtw" => Extend::Sxtw,
                            "sxtx" => Extend::Sxtx,
                            _ => return Err(self.problem(&format!("`{text}` isn't an extend"))),
                        };
                        (extend, amount)
                    }
                };
                if index.wide != matches!(extend, Extend::Lsl | Extend::Sxtx) {
                    return Err(self.problem("its index's width and extend don't agree"));
                }
                let log = size.trailing_zeros();
                let shift = match amount {
                    None | Some(0) => 0,
                    Some(shift) if shift == i64::from(log) => log,
                    Some(shift) => {
                        return Err(
                            self.problem(&format!("its index shifts by {shift}, not 0 or {log}"))
                        );
                    }
                };
                Ok(Address::Register {
                    base,
                    index,
                    extend,
                    shift: u8::try_from(shift).map_err(|_| self.problem("too far"))?,
                })
            }
            _ => Err(self.problem("its address isn't one the dialect has")),
        }
    }

    /// A load or store of one register.
    fn load_store(&self) -> Result<Inst, String> {
        let name = self.mnemonic;
        let unscaled = name.starts_with("ldur") || name.starts_with("stur");
        let plain = if unscaled {
            name.replacen("ur", "r", 1)
        } else {
            name.to_owned()
        };
        let rt = self.reg(0)?;
        let access = match (plain.as_str(), rt.wide) {
            ("ldr", wide) => Access {
                load: true,
                size: if wide { 8 } else { 4 },
                signed: false,
            },
            ("str", wide) => Access {
                load: false,
                size: if wide { 8 } else { 4 },
                signed: false,
            },
            ("ldrb", false) => Access {
                load: true,
                size: 1,
                signed: false,
            },
            ("ldrh", false) => Access {
                load: true,
                size: 2,
                signed: false,
            },
            ("strb", false) => Access {
                load: false,
                size: 1,
                signed: false,
            },
            ("strh", false) => Access {
                load: false,
                size: 2,
                signed: false,
            },
            ("ldrsb", _) => Access {
                load: true,
                size: 1,
                signed: true,
            },
            ("ldrsh", _) => Access {
                load: true,
                size: 2,
                signed: true,
            },
            ("ldrsw", true) => Access {
                load: true,
                size: 4,
                signed: true,
            },
            _ => {
                return Err(self.problem(&format!(
                    "`{name}` isn't an instruction lengua's AArch64 dialect has"
                )));
            }
        };
        let address = self.address(1, access.size)?;
        if unscaled && !matches!(address, Address::Offset { .. }) {
            return Err(self.problem("an unscaled access takes only an offset"));
        }
        if let Address::PreIndex { base, .. } | Address::PostIndex { base, .. } = address
            && base.gpr == rt.gpr
        {
            return Err(self.problem(
                "its base moves and is also the register it transfers, which has no defined \
                 result",
            ));
        }
        Ok(Inst::LoadStore {
            access,
            rt,
            address,
        })
    }

    /// `ldp`, `stp` and `ldpsw`.
    fn pair(&self) -> Result<Inst, String> {
        let load = self.mnemonic != "stp";
        let signed = self.mnemonic == "ldpsw";
        let rt = self.reg(0)?;
        let rt2 = self.reg_as(1, rt.wide)?;
        if signed && !rt.wide {
            return Err(self.problem("ldpsw loads into x registers"));
        }
        let size = if rt.wide && !signed { 8 } else { 4 };
        let address = self.address(2, size)?;
        let moving = match address {
            Address::Register { .. } => {
                return Err(self.problem("a pair's address takes only an offset"));
            }
            Address::PreIndex { base, .. } | Address::PostIndex { base, .. } => Some(base),
            Address::Offset { .. } => None,
        };
        if (load && rt.gpr == rt2.gpr)
            || moving.is_some_and(|base| base.gpr == rt.gpr || base.gpr == rt2.gpr)
        {
            return Err(self.problem(
                "it loads one register twice, or its base moves and is also a register it \
                 transfers, which has no defined result",
            ));
        }
        Ok(Inst::Pair {
            load,
            signed,
            rt,
            rt2,
            address,
        })
    }
}
