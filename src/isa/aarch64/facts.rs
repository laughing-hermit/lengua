//! What an AArch64 instruction does, as the checker's rules read it ([`Insn`]): the registers it
//! reads and writes, how control leaves it, its memory operand, what it does to the stack, the
//! idiom it is, and what it puts in its destination.

use super::sve::{Against, Sve};
use super::{
    A64Cond, A64Reg, Address, FpConversion, FpOne, FpReg, FpThree, FpTwo, Gpr, Indirect, Inst,
    Logic, Source, VImmediate, VThree, Wide, Writeback,
};
use crate::isa::{BaseMove, Cond, Control, Insn, MemRef, Op, Operand, Reg, RegSet, Stack, Value};

/// `register` as an operand: the zero register is the number 0, which holds no address.
fn operand(register: A64Reg) -> Operand {
    if register.gpr == Gpr::Zr {
        Operand::Imm(0)
    } else {
        Operand::Reg(Reg::A64(register))
    }
}

/// Whether `register` is the stack pointer.
/// A floating-point register, as the shared facts name it.
const fn fp(register: FpReg) -> Reg {
    Reg::A64Fp(register)
}

fn fp_operand(register: FpReg) -> Operand {
    Operand::Reg(fp(register))
}

/// A V register whole, as the shared facts name it: its low double's name, the part a call keeps
/// for d8 to d15.
const fn v(number: u8) -> Reg {
    fp(FpReg {
        number,
        double: true,
    })
}

fn v_operand(number: u8) -> Operand {
    Operand::Reg(v(number))
}

/// What a `dup` or an insert copies from, as an operand.
fn source_operand(from: Source) -> Operand {
    match from {
        Source::Lane(lane) => v_operand(lane.number),
        Source::General(register) => operand(register),
    }
}

/// Whether the conversion's result goes to the general register.
const fn to_general(op: FpConversion) -> bool {
    matches!(
        op,
        FpConversion::Fcvtzs | FpConversion::Fcvtzu | FpConversion::ToGeneral
    )
}

fn is_sp(register: A64Reg) -> bool {
    register.gpr == Gpr::Sp
}

/// All ones in the low `width` bits.
const fn ones(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1 << width) - 1
    }
}

/// When a conditional branch on the flags jumps.
const fn cond(cond: A64Cond) -> Cond {
    match cond {
        A64Cond::Eq => Cond::Equal,
        A64Cond::Ne => Cond::NotEqual,
        A64Cond::Hs => Cond::AboveEqual,
        A64Cond::Lo => Cond::Below,
        A64Cond::Mi => Cond::Sign,
        A64Cond::Pl => Cond::NotSign,
        A64Cond::Vs => Cond::Overflow,
        A64Cond::Vc => Cond::NotOverflow,
        A64Cond::Hi => Cond::Above,
        A64Cond::Ls => Cond::BelowEqual,
        A64Cond::Ge => Cond::GreaterEqual,
        A64Cond::Lt => Cond::Less,
        A64Cond::Gt => Cond::Greater,
        A64Cond::Le => Cond::LessEqual,
        A64Cond::Al | A64Cond::Nv => Cond::Always,
    }
}

/// The instruction's name as LLVM's disassembler calls its canonical form, for messages.
#[must_use]
pub fn mnemonic(inst: &Inst) -> String {
    let name = match inst {
        Inst::Sve(sve) => sve.mnemonic(),
        Inst::Crypto { op, .. } => op.name(),
        Inst::AddImm { sub, flags, .. } | Inst::AddShifted { sub, flags, .. } => {
            match (sub, flags) {
                (false, false) => "add",
                (false, true) => "adds",
                (true, false) => "sub",
                (true, true) => "subs",
            }
        }
        Inst::LogicalImm { op, .. } => match op {
            Logic::And => "and",
            Logic::Orr => "orr",
            Logic::Eor => "eor",
            Logic::Ands => "ands",
        },
        Inst::LogicalShifted { op, invert, .. } => match (op, invert) {
            (Logic::And, false) => "and",
            (Logic::And, true) => "bic",
            (Logic::Orr, false) => "orr",
            (Logic::Orr, true) => "orn",
            (Logic::Eor, false) => "eor",
            (Logic::Eor, true) => "eon",
            (Logic::Ands, false) => "ands",
            (Logic::Ands, true) => "bics",
        },
        Inst::MoveWide { op, .. } => match op {
            Wide::Movn => "movn",
            Wide::Movz => "movz",
            Wide::Movk => "movk",
        },
        Inst::Bitfield { op, .. } => match op {
            super::Field::Sbfm => "sbfm",
            super::Field::Bfm => "bfm",
            super::Field::Ubfm => "ubfm",
        },
        Inst::Extract { .. } => "extr",
        Inst::Adrp { .. } => "adrp",
        Inst::Adr { .. } => "adr",
        Inst::Branch { link: false, .. } => "b",
        Inst::Branch { link: true, .. } => "bl",
        Inst::BranchCond { cond, .. } => return format!("b.{}", cond.name()),
        Inst::CompareBranch { nonzero, .. } => {
            if *nonzero {
                "cbnz"
            } else {
                "cbz"
            }
        }
        Inst::TestBranch { nonzero, .. } => {
            if *nonzero {
                "tbnz"
            } else {
                "tbz"
            }
        }
        Inst::BranchReg { kind, .. } => match kind {
            Indirect::Br => "br",
            Indirect::Blr => "blr",
            Indirect::Ret => "ret",
        },
        Inst::Svc { .. } => "svc",
        Inst::CondSelect { op, .. } => match op {
            super::Select::Csel => "csel",
            super::Select::Csinc => "csinc",
            super::Select::Csinv => "csinv",
            super::Select::Csneg => "csneg",
        },
        Inst::Crc {
            castagnoli, size, ..
        } => match (castagnoli, size) {
            (false, 1) => "crc32b",
            (false, 2) => "crc32h",
            (false, 4) => "crc32w",
            (false, _) => "crc32x",
            (true, 1) => "crc32cb",
            (true, 2) => "crc32ch",
            (true, 4) => "crc32cw",
            (true, _) => "crc32cx",
        },
        Inst::TwoSource { op, .. } => match op {
            super::Two::Udiv => "udiv",
            super::Two::Sdiv => "sdiv",
            super::Two::Lslv => "lslv",
            super::Two::Lsrv => "lsrv",
            super::Two::Asrv => "asrv",
            super::Two::Rorv => "rorv",
        },
        Inst::OneSource { op, .. } => match op {
            super::One::Rbit => "rbit",
            super::One::Rev16 => "rev16",
            super::One::Rev32 => "rev32",
            super::One::Rev => "rev",
            super::One::Clz => "clz",
            super::One::Cls => "cls",
        },
        Inst::ThreeSource { op, .. } => match op {
            super::Three::Madd => "madd",
            super::Three::Msub => "msub",
            super::Three::Smaddl => "smaddl",
            super::Three::Smsubl => "smsubl",
            super::Three::Umaddl => "umaddl",
            super::Three::Umsubl => "umsubl",
            super::Three::Smulh => "smulh",
            super::Three::Umulh => "umulh",
        },
        Inst::LoadStore { access, .. } if access.acquire => "ldar",
        Inst::LoadStore { access, .. } => match (access.load, access.size, access.signed) {
            (true, 1, false) => "ldrb",
            (true, 2, false) => "ldrh",
            (true, 1, true) => "ldrsb",
            (true, 2, true) => "ldrsh",
            (true, 4, true) => "ldrsw",
            (true, _, _) => "ldr",
            (false, 1, _) => "strb",
            (false, 2, _) => "strh",
            (false, _, _) => "str",
        },
        Inst::Pair { load, signed, .. } => match (load, signed) {
            (true, true) => "ldpsw",
            (true, false) => "ldp",
            (false, _) => "stp",
        },
        Inst::Nop => "nop",
        Inst::FpOneSource { op, .. } => match op {
            FpOne::Mov => "fmov",
            FpOne::Abs => "fabs",
            FpOne::Neg => "fneg",
            FpOne::Sqrt => "fsqrt",
            FpOne::Cvt => "fcvt",
            FpOne::RintNearest => "frintn",
            FpOne::RintUp => "frintp",
            FpOne::RintDown => "frintm",
            FpOne::RintZero => "frintz",
            FpOne::RintAway => "frinta",
        },
        Inst::FpTwoSource { op, .. } => match op {
            FpTwo::Mul => "fmul",
            FpTwo::Div => "fdiv",
            FpTwo::Add => "fadd",
            FpTwo::Sub => "fsub",
            FpTwo::Max => "fmax",
            FpTwo::Min => "fmin",
            FpTwo::MaxNm => "fmaxnm",
            FpTwo::MinNm => "fminnm",
            FpTwo::Nmul => "fnmul",
        },
        Inst::FpThreeSource { op, .. } => match op {
            FpThree::Madd => "fmadd",
            FpThree::Msub => "fmsub",
            FpThree::Nmadd => "fnmadd",
            FpThree::Nmsub => "fnmsub",
        },
        Inst::FpCompare { signaling, .. } => {
            if *signaling {
                "fcmpe"
            } else {
                "fcmp"
            }
        }
        Inst::FpCondSelect { .. } => "fcsel",
        Inst::FpMoveImm { .. } => "fmov",
        Inst::FpConvert { op, .. } => match op {
            FpConversion::Scvtf => "scvtf",
            FpConversion::Ucvtf => "ucvtf",
            FpConversion::Fcvtzs => "fcvtzs",
            FpConversion::Fcvtzu => "fcvtzu",
            FpConversion::ToGeneral | FpConversion::FromGeneral => "fmov",
        },
        Inst::FpLoadStore { load, .. } => {
            if *load {
                "ldr"
            } else {
                "str"
            }
        }
        Inst::FpPair { load, .. } | Inst::VectorPair { load, .. } => {
            if *load {
                "ldp"
            } else {
                "stp"
            }
        }
        Inst::VectorThree { op, rn, rm, .. } => {
            if *op == VThree::Orr && rn == rm {
                "mov"
            } else {
                op.name()
            }
        }
        Inst::VectorTwo { op, rd, rn } => op.name(*rd, *rn),
        Inst::Pmull { rn, .. } => {
            if super::pmull_upper(*rn) {
                "pmull2"
            } else {
                "pmull"
            }
        }
        Inst::VectorAcross { op, .. } => op.name(),
        Inst::VectorShift { op, rd, rn, .. } => op.name(*rd, *rn),
        Inst::VectorPermute { op, .. } => op.name(),
        Inst::VectorExtract { .. } => "ext",
        Inst::VectorTable { keep, .. } => {
            if *keep {
                "tbx"
            } else {
                "tbl"
            }
        }
        Inst::VectorDup { .. } => "dup",
        // LLVM writes `umov` of a word or a doubleword as `mov`.
        Inst::VectorToGeneral { signed: true, .. } => "smov",
        Inst::VectorToGeneral { rn, .. } if rn.size >= 2 => "mov",
        Inst::VectorToGeneral { .. } => "umov",
        Inst::VectorInsert { .. } => "mov",
        Inst::VectorImmediate { op, .. } => op.name(),
        Inst::VectorLoadStore { load, address, .. } => {
            // An offset `ldr` can't hold is `ldur`'s, as LLVM writes it.
            let unscaled = matches!(
                address,
                Address::Offset { offset, .. } if *offset < 0 || offset % 16 != 0
            );
            match (*load, unscaled) {
                (true, false) => "ldr",
                (true, true) => "ldur",
                (false, false) => "str",
                (false, true) => "stur",
            }
        }
        Inst::VectorStructures { load, .. } => {
            if *load {
                "ld1"
            } else {
                "st1"
            }
        }
    };
    name.to_owned()
}

/// The facts of `inst`, at `address`.
#[must_use]
pub fn facts(inst: &Inst, address: u64) -> Insn {
    let mut facts = Facts::default();
    facts.read(inst);
    let a64 = Reg::A64;
    let (control, dst, operands, memory, op, value, stack) = match *inst {
        Inst::Sve(sve) => sve_facts(sve),
        Inst::Crypto { op, rd, rn, rm } => {
            let mut operands = vec![v_operand(rd), v_operand(rn)];
            if op.three() {
                operands.push(v_operand(rm));
            }
            (
                Control::Next,
                Some(v(rd)),
                operands,
                None,
                Op::Other,
                Value::Other,
                Stack::Same,
            )
        }
        Inst::AddImm {
            sub,
            flags,
            rd,
            rn,
            imm,
            shift12,
        } => {
            let number = i64::from(imm) << if shift12 { 12 } else { 0 };
            let compare = flags && rd.gpr == Gpr::Zr;
            let op = if compare && sub {
                Op::Compare {
                    left: operand(rn),
                    right: Operand::Imm(number),
                }
            } else if !flags && !sub && number == 0 && (is_sp(rd) || is_sp(rn)) {
                Op::Move
            } else if sub && number == 1 && rd.full() == rn.full() && matches!(rd.gpr, Gpr::R(_)) {
                Op::Decrement {
                    counter: a64(rd.full()),
                    flags,
                }
            } else {
                Op::Other
            };
            let value = if sub {
                Value::Sub(operand(rn), Operand::Imm(number))
            } else {
                Value::Add(operand(rn), Operand::Imm(number))
            };
            let stack = match (is_sp(rd), is_sp(rn)) {
                (true, true) => Stack::Moves(if sub { number } else { -number }),
                (true, false) => Stack::Refused(format!("`{}` sets sp", mnemonic(inst))),
                _ => Stack::Same,
            };
            (
                Control::Next,
                Some(a64(rd)),
                vec![operand(rd), operand(rn), Operand::Imm(number)],
                None,
                op,
                value,
                stack,
            )
        }
        Inst::AddShifted {
            sub,
            flags,
            rd,
            rn,
            rm,
            amount,
            ..
        } => {
            let plain = amount == 0;
            let op = if flags && sub && rd.gpr == Gpr::Zr && plain {
                Op::Compare {
                    left: operand(rn),
                    right: operand(rm),
                }
            } else {
                Op::Other
            };
            let value = match (plain, sub) {
                (true, false) => Value::Add(operand(rn), operand(rm)),
                (true, true) => Value::Sub(operand(rn), operand(rm)),
                _ => Value::Other,
            };
            (
                Control::Next,
                Some(a64(rd)),
                vec![operand(rd), operand(rn), operand(rm)],
                None,
                op,
                value,
                facts.stack_written(inst, rd),
            )
        }
        Inst::LogicalImm { op, rd, rn, imm } => {
            let width = if rd.wide { 64 } else { 32 };
            let idiom = match op {
                Logic::Ands if rd.gpr == Gpr::Zr => Op::Test,
                Logic::Orr if rn.gpr == Gpr::Zr => Op::MovImm {
                    dst: a64(rd),
                    value: (imm & ones(width)).cast_signed(),
                },
                _ => Op::Other,
            };
            let number = Operand::Imm(imm.cast_signed());
            let value = match op {
                Logic::And => Value::And(operand(rn), number),
                Logic::Orr if rn.gpr == Gpr::Zr => Value::Number,
                _ => Value::Other,
            };
            (
                Control::Next,
                Some(a64(rd)),
                vec![operand(rd), operand(rn), number],
                None,
                idiom,
                value,
                facts.stack_written(inst, rd),
            )
        }
        Inst::LogicalShifted {
            op,
            invert,
            rd,
            rn,
            rm,
            amount,
            ..
        } => {
            let moves = op == Logic::Orr && !invert && rn.gpr == Gpr::Zr && amount == 0;
            let idiom = if moves {
                Op::Move
            } else if op == Logic::Ands && rd.gpr == Gpr::Zr {
                Op::Test
            } else {
                Op::Other
            };
            (
                Control::Next,
                Some(a64(rd)),
                vec![operand(rd), operand(rn), operand(rm)],
                None,
                idiom,
                if moves {
                    Value::Copy(operand(rm))
                } else {
                    Value::Other
                },
                Stack::Same,
            )
        }
        Inst::MoveWide { op, rd, imm, shift } => {
            let width = if rd.wide { 64 } else { 32 };
            let shifted = u64::from(imm) << shift;
            let (idiom, value) = match op {
                Wide::Movz => (
                    Op::MovImm {
                        dst: a64(rd),
                        value: shifted.cast_signed(),
                    },
                    Value::Number,
                ),
                Wide::Movn => (
                    Op::MovImm {
                        dst: a64(rd),
                        value: (!shifted & ones(width)).cast_signed(),
                    },
                    Value::Number,
                ),
                Wide::Movk => (Op::Other, Value::Other),
            };
            (
                Control::Next,
                Some(a64(rd)),
                vec![operand(rd), Operand::Imm(i64::from(imm))],
                None,
                idiom,
                value,
                Stack::Same,
            )
        }
        Inst::Bitfield { rd, rn, .. } | Inst::OneSource { rd, rn, .. } => (
            Control::Next,
            Some(a64(rd)),
            vec![operand(rd), operand(rn)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::Extract { rd, rn, rm, .. }
        | Inst::CondSelect { rd, rn, rm, .. }
        | Inst::TwoSource { rd, rn, rm, .. }
        | Inst::Crc { rd, rn, rm, .. } => (
            Control::Next,
            Some(a64(rd)),
            vec![operand(rd), operand(rn), operand(rm)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::ThreeSource { rd, rn, rm, ra, .. } => (
            Control::Next,
            Some(a64(rd)),
            vec![operand(rd), operand(rn), operand(rm), operand(ra)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::Adrp { rd, page } => (
            Control::Next,
            Some(a64(rd)),
            vec![operand(rd), Operand::Target(page)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::Adr { rd, target } => (
            Control::Next,
            Some(a64(rd)),
            vec![operand(rd), Operand::Target(target)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::Branch { link, target } => (
            if link {
                Control::Call(target)
            } else {
                Control::Jump(target)
            },
            None,
            vec![Operand::Target(target)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::BranchCond { cond: when, target } => (
            // `b.al` and `b.nv` always jump, so they are a jump, as `b` is: the code after one
            // can't run, and an assertion it leads to always fails.
            if matches!(when, A64Cond::Al | A64Cond::Nv) {
                Control::Jump(target)
            } else {
                Control::Branch {
                    target,
                    cond: cond(when),
                }
            },
            None,
            vec![Operand::Target(target)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::CompareBranch {
            nonzero,
            rt,
            target,
        } => (
            Control::Branch {
                target,
                cond: if nonzero { Cond::NotZero } else { Cond::Zero },
            },
            None,
            vec![operand(rt), Operand::Target(target)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::TestBranch {
            nonzero,
            rt,
            bit,
            target,
        } => (
            Control::Branch {
                target,
                cond: if nonzero {
                    Cond::BitSet(bit)
                } else {
                    Cond::BitClear(bit)
                },
            },
            None,
            vec![
                operand(rt),
                Operand::Imm(i64::from(bit)),
                Operand::Target(target),
            ],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::BranchReg { kind, rn } => (
            match kind {
                Indirect::Br => Control::IndirectJump,
                Indirect::Blr => Control::IndirectCall,
                Indirect::Ret => Control::Return,
            },
            None,
            vec![operand(rn)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::Svc { imm } => (
            Control::Syscall,
            None,
            vec![Operand::Imm(i64::from(imm))],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::LoadStore {
            access,
            rt,
            address: place,
        } => {
            let memory = memory(place, u64::from(access.size), access.load);
            let (idiom, stack) = frame(place, &[a64(rt)], access.load);
            (
                Control::Next,
                access.load.then_some(a64(rt)),
                vec![operand(rt), Operand::Mem],
                Some(memory),
                idiom,
                if access.load {
                    Value::Load
                } else {
                    Value::Other
                },
                stack,
            )
        }
        Inst::Pair {
            load,
            signed,
            rt,
            rt2,
            address: place,
        } => {
            let size = if rt.wide && !signed { 8 } else { 4 };
            let memory = memory(place, 2 * size, load);
            let (idiom, stack) = frame(place, &[a64(rt), a64(rt2)], load);
            (
                Control::Next,
                load.then_some(a64(rt)),
                vec![operand(rt), operand(rt2), Operand::Mem],
                Some(memory),
                idiom,
                if load { Value::Load } else { Value::Other },
                stack,
            )
        }
        Inst::Nop => (
            Control::Next,
            None,
            Vec::new(),
            None,
            Op::Hint,
            Value::Other,
            Stack::Same,
        ),
        // Floating point. Its registers hold no addresses the memory rules follow, as x86-64's
        // vector registers don't: what comes out of one is unknown to them.
        Inst::FpOneSource { rd, rn, .. } => (
            Control::Next,
            Some(fp(rd)),
            vec![fp_operand(rd), fp_operand(rn)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::FpTwoSource { rd, rn, rm, .. } | Inst::FpCondSelect { rd, rn, rm, .. } => (
            Control::Next,
            Some(fp(rd)),
            vec![fp_operand(rd), fp_operand(rn), fp_operand(rm)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::FpThreeSource { rd, rn, rm, ra, .. } => (
            Control::Next,
            Some(fp(rd)),
            vec![
                fp_operand(rd),
                fp_operand(rn),
                fp_operand(rm),
                fp_operand(ra),
            ],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::FpCompare { rn, rm, .. } => (
            Control::Next,
            None,
            vec![fp_operand(rn), rm.map_or(Operand::Imm(0), fp_operand)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::FpMoveImm { rd, imm8 } => (
            Control::Next,
            Some(fp(rd)),
            vec![fp_operand(rd), Operand::Imm(i64::from(imm8))],
            None,
            Op::Other,
            Value::Number,
            Stack::Same,
        ),
        Inst::FpConvert {
            op,
            fp: register,
            gpr,
        } => {
            let (dst, operands) = if to_general(op) {
                (a64(gpr), vec![operand(gpr), fp_operand(register)])
            } else {
                (fp(register), vec![fp_operand(register), operand(gpr)])
            };
            (
                Control::Next,
                Some(dst),
                operands,
                None,
                Op::Other,
                Value::Other,
                Stack::Same,
            )
        }
        Inst::FpLoadStore {
            load,
            rt,
            address: place,
        } => {
            let memory = memory(place, if rt.double { 8 } else { 4 }, load);
            let (idiom, stack) = frame(place, &[fp(rt)], load);
            (
                Control::Next,
                load.then_some(fp(rt)),
                vec![fp_operand(rt), Operand::Mem],
                Some(memory),
                idiom,
                if load { Value::Load } else { Value::Other },
                stack,
            )
        }
        Inst::FpPair {
            load,
            rt,
            rt2,
            address: place,
        } => {
            let memory = memory(place, if rt.double { 16 } else { 8 }, load);
            let (idiom, stack) = frame(place, &[fp(rt), fp(rt2)], load);
            (
                Control::Next,
                load.then_some(fp(rt)),
                vec![fp_operand(rt), fp_operand(rt2), Operand::Mem],
                Some(memory),
                idiom,
                if load { Value::Load } else { Value::Other },
                stack,
            )
        }
        // Vectors. Their registers, the V registers whole, hold no addresses the memory rules
        // follow, as floating point's don't.
        Inst::VectorThree { rd, rn, rm, .. }
        | Inst::VectorPermute { rd, rn, rm, .. }
        | Inst::VectorExtract { rd, rn, rm, .. } => (
            Control::Next,
            Some(v(rd.number)),
            vec![
                v_operand(rd.number),
                v_operand(rn.number),
                v_operand(rm.number),
            ],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::VectorTwo { rd, rn, .. } | Inst::VectorShift { rd, rn, .. } => (
            Control::Next,
            Some(v(rd.number)),
            vec![v_operand(rd.number), v_operand(rn.number)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::Pmull { rd, rn, rm } => (
            Control::Next,
            Some(v(rd)),
            vec![v_operand(rd), v_operand(rn.number), v_operand(rm.number)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::VectorAcross { rd, rn, .. } => (
            Control::Next,
            Some(v(rd.number)),
            vec![v_operand(rd.number), v_operand(rn.number)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::VectorTable { rd, table, rm, .. } => (
            Control::Next,
            Some(v(rd.number)),
            vec![v_operand(rd.number), v_operand(table), v_operand(rm.number)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::VectorDup { rd, from } => (
            Control::Next,
            Some(v(rd.number)),
            vec![v_operand(rd.number), source_operand(from)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::VectorInsert { rd, from } => (
            Control::Next,
            Some(v(rd.number)),
            vec![v_operand(rd.number), source_operand(from)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::VectorToGeneral { rd, rn, .. } => (
            Control::Next,
            Some(a64(rd)),
            vec![operand(rd), v_operand(rn.number)],
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        ),
        Inst::VectorImmediate { rd, .. } => (
            Control::Next,
            Some(v(rd.number)),
            vec![v_operand(rd.number)],
            None,
            Op::Other,
            Value::Number,
            Stack::Same,
        ),
        Inst::VectorLoadStore {
            load,
            rt,
            address: place,
        } => {
            let memory = memory(place, 16, load);
            let (idiom, stack) = frame(place, &[v(rt)], load);
            (
                Control::Next,
                load.then_some(v(rt)),
                vec![v_operand(rt), Operand::Mem],
                Some(memory),
                idiom,
                if load { Value::Load } else { Value::Other },
                stack,
            )
        }
        Inst::VectorPair {
            load,
            rt,
            rt2,
            address: place,
        } => {
            let memory = memory(place, 32, load);
            let (idiom, stack) = frame(place, &[v(rt), v(rt2)], load);
            (
                Control::Next,
                load.then_some(v(rt)),
                vec![v_operand(rt), v_operand(rt2), Operand::Mem],
                Some(memory),
                idiom,
                if load { Value::Load } else { Value::Other },
                stack,
            )
        }
        Inst::VectorStructures {
            load,
            first,
            count,
            arrangement,
            base,
            step,
        } => {
            let width = u64::from(count) * u64::from(arrangement.bytes());
            let stack = if is_sp(base) && step.is_some() {
                Stack::Refused(format!("`{}` moves sp", mnemonic(inst)))
            } else {
                Stack::Same
            };
            (
                Control::Next,
                load.then_some(v(first)),
                vec![v_operand(first), Operand::Mem],
                Some(MemRef {
                    base: Some(Reg::A64(base)),
                    index: None,
                    scale: 1,
                    displacement: 0,
                    pc_relative: false,
                    width,
                    reads: load,
                    writes: !load,
                    base_moves: step.map(|step| match step {
                        Writeback::Bytes => BaseMove::By(width.cast_signed()),
                        Writeback::Register(_) => BaseMove::ByRegister,
                    }),
                }),
                Op::Other,
                if load { Value::Load } else { Value::Other },
                stack,
            )
        }
    };
    // AArch64 faults on an access through sp that isn't a multiple of 16.
    let stack = match stack {
        Stack::Moves(bytes) if bytes % 16 != 0 => Stack::Refused(format!(
            "`{}` moves sp by {bytes} bytes, but sp stays a multiple of 16",
            mnemonic(inst)
        )),
        other => other,
    };
    Insn {
        address,
        mnemonic: mnemonic(inst),
        control,
        reads: facts.reads,
        writes: facts.writes,
        dst: dst
            .filter(|register| !matches!(register, Reg::A64(written) if written.gpr == Gpr::Zr)),
        operands,
        reads_memory: memory.is_some_and(|memory: MemRef| memory.reads),
        writes_memory: memory.is_some_and(|memory: MemRef| memory.writes),
        memory,
        op,
        value,
        stack,
    }
}

/// The memory operand of an access to `width` bytes at `place`.
/// The facts of an SVE instruction. Its predicates aren't registers the rules follow; its Z
/// registers are the V registers they extend, and a load or store is held to
/// [`crate::platform::SVE_MAX_BYTES`], the longest vector, so it fits on every CPU.
fn sve_facts(
    sve: Sve,
) -> (
    Control,
    Option<Reg>,
    Vec<Operand>,
    Option<MemRef>,
    Op,
    Value,
    Stack,
) {
    let a64 = Reg::A64;
    let general = |register: A64Reg| Operand::Reg(a64(register));
    let vector = |dst: u8, sources: &[u8]| {
        let mut operands = vec![v_operand(dst)];
        operands.extend(sources.iter().map(|source| v_operand(*source)));
        (
            Control::Next,
            Some(v(dst)),
            operands,
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        )
    };
    let plain = |dst: Option<Reg>, operands: Vec<Operand>| {
        (
            Control::Next,
            dst,
            operands,
            None,
            Op::Other,
            Value::Other,
            Stack::Same,
        )
    };
    match sve {
        Sve::Ptrue { .. } | Sve::PredLogic { .. } => plain(None, Vec::new()),
        Sve::Whilelo { rn, rm, .. } => plain(None, vec![general(rn), general(rm)]),
        Sve::Cntp { rd, .. } | Sve::Count { rd, .. } | Sve::Lastb { rd, .. } => {
            plain(Some(a64(rd)), vec![general(rd)])
        }
        Sve::Access {
            store,
            zt,
            base,
            index,
            ..
        } => (
            Control::Next,
            (!store).then_some(v(zt)),
            vec![v_operand(zt), Operand::Mem],
            Some(MemRef {
                base: Some(a64(base)),
                index: Some(a64(index)),
                scale: 1,
                displacement: 0,
                pc_relative: false,
                width: u64::from(crate::platform::SVE_MAX_BYTES),
                reads: !store,
                writes: store,
                base_moves: None,
            }),
            Op::Other,
            if store { Value::Other } else { Value::Load },
            Stack::Same,
        ),
        Sve::Compare { zn, against, .. } => {
            let mut operands = vec![v_operand(zn)];
            if let Against::Vector(zm) = against {
                operands.push(v_operand(zm));
            }
            plain(None, operands)
        }
        Sve::Arith { zd, zn, zm, .. }
        | Sve::Logic { zd, zn, zm, .. }
        | Sve::Sel { zd, zn, zm, .. } => vector(zd, &[zn, zm]),
        Sve::Cnt { zd, zn, .. } => vector(zd, &[zn]),
        Sve::DupImm { zd, .. } | Sve::Cpy { zd, .. } => vector(zd, &[]),
        Sve::DupReg { zd, rn } => {
            let mut facts = vector(zd, &[]);
            facts.2.push(general(rn));
            facts
        }
        Sve::Uaddv { vd, zn, .. } => vector(vd, &[zn]),
        Sve::Insr { zdn, rm } => {
            let mut facts = vector(zdn, &[]);
            facts.2.push(general(rm));
            facts
        }
    }
}

fn memory(place: Address, width: u64, load: bool) -> MemRef {
    let (base, index, scale, displacement) = match place {
        Address::Offset { base, offset } | Address::PreIndex { base, offset } => {
            (base, None, 1, offset)
        }
        // A post-index access is at the base; the base moves after.
        Address::PostIndex { base, .. } => (base, None, 1, 0),
        Address::Register {
            base, index, shift, ..
        } => (base, Some(Reg::A64(index)), 1 << shift, 0),
    };
    let base_moves = match place {
        Address::PreIndex { offset, .. } | Address::PostIndex { offset, .. } => {
            Some(BaseMove::By(offset))
        }
        Address::Offset { .. } | Address::Register { .. } => None,
    };
    MemRef {
        base: Some(Reg::A64(base)),
        index,
        scale,
        displacement,
        pc_relative: false,
        width,
        reads: load,
        writes: !load,
        base_moves,
    }
}

/// What an access at `place` of `registers` does to the stack: a store before which sp moves
/// down pushes them, a load after which it moves back up pops them, and any other move of sp is
/// counted as it is.
fn frame(place: Address, registers: &[Reg], load: bool) -> (Op, Stack) {
    let (Address::PreIndex { base, offset } | Address::PostIndex { base, offset }) = place else {
        return (Op::Other, Stack::Same);
    };
    if !is_sp(base) {
        return (Op::Other, Stack::Same);
    }
    let whole = [
        registers.first().map(|register| register.full()),
        registers.get(1).map(|register| register.full()),
    ];
    let idiom = match place {
        Address::PreIndex { .. } if !load && offset < 0 => Op::Push(whole),
        Address::PostIndex { .. } if load && offset > 0 => Op::Pop(whole),
        _ => Op::Other,
    };
    (idiom, Stack::Moves(-offset))
}

/// The registers an instruction reads and writes, gathered.
#[derive(Default)]
struct Facts {
    reads: RegSet,
    writes: RegSet,
}

impl Facts {
    fn reads_register(&mut self, register: A64Reg) {
        self.reads.insert(Reg::A64(register));
    }

    fn writes_register(&mut self, register: A64Reg) {
        self.writes.insert(Reg::A64(register));
    }

    fn reads_fp(&mut self, register: FpReg) {
        self.reads.insert(fp(register));
    }

    fn writes_fp(&mut self, register: FpReg) {
        self.writes.insert(fp(register));
    }

    /// What it does to sp when its destination is `rd`: only the instructions the dialect
    /// moves sp with may.
    fn stack_written(&self, inst: &Inst, rd: A64Reg) -> Stack {
        if is_sp(rd) {
            Stack::Refused(format!("`{}` sets sp", mnemonic(inst)))
        } else {
            Stack::Same
        }
    }

    /// Gathers what `inst` reads and writes.
    fn read(&mut self, inst: &Inst) {
        let x = |number| A64Reg::new(number, true, false);
        match *inst {
            Inst::Sve(sve) => self.read_sve(sve),
            Inst::Crypto { op, rd, rn, rm } => {
                if op.reads_destination() {
                    self.reads_v(rd);
                }
                self.reads_v(rn);
                if op.three() {
                    self.reads_v(rm);
                }
                self.writes_v(rd);
            }
            Inst::AddImm { rd, rn, .. }
            | Inst::LogicalImm { rd, rn, .. }
            | Inst::OneSource { rd, rn, .. } => {
                self.reads_register(rn);
                self.writes_register(rd);
            }
            Inst::Bitfield { op, rd, rn, .. } => {
                self.reads_register(rn);
                // An insertion keeps the rest of its destination.
                if op == super::Field::Bfm {
                    self.reads_register(rd);
                }
                self.writes_register(rd);
            }
            Inst::AddShifted { rd, rn, rm, .. }
            | Inst::LogicalShifted { rd, rn, rm, .. }
            | Inst::Extract { rd, rn, rm, .. }
            | Inst::CondSelect { rd, rn, rm, .. }
            | Inst::TwoSource { rd, rn, rm, .. }
            | Inst::Crc { rd, rn, rm, .. } => {
                self.reads_register(rn);
                self.reads_register(rm);
                self.writes_register(rd);
            }
            Inst::ThreeSource { rd, rn, rm, ra, .. } => {
                self.reads_register(rn);
                self.reads_register(rm);
                self.reads_register(ra);
                self.writes_register(rd);
            }
            Inst::MoveWide { op, rd, .. } => {
                if op == Wide::Movk {
                    self.reads_register(rd);
                }
                self.writes_register(rd);
            }
            Inst::Adrp { rd, .. } | Inst::Adr { rd, .. } => self.writes_register(rd),
            Inst::Branch { link, .. } => {
                if link {
                    self.writes_register(x(30));
                }
            }
            Inst::CompareBranch { rt, .. } | Inst::TestBranch { rt, .. } => {
                self.reads_register(rt);
            }
            Inst::BranchReg { kind, rn } => {
                self.reads_register(rn);
                if kind == Indirect::Blr {
                    self.writes_register(x(30));
                }
            }
            Inst::Svc { .. } => {
                for number in [8, 0, 1, 2, 3, 4, 5] {
                    self.reads_register(x(number));
                }
                self.writes_register(x(0));
            }
            Inst::LoadStore {
                access,
                rt,
                address,
            } => {
                self.address(address);
                if access.load {
                    self.writes_register(rt);
                } else {
                    self.reads_register(rt);
                }
            }
            Inst::Pair {
                load,
                rt,
                rt2,
                address,
                ..
            } => {
                self.address(address);
                for register in [rt, rt2] {
                    if load {
                        self.writes_register(register);
                    } else {
                        self.reads_register(register);
                    }
                }
            }
            Inst::BranchCond { .. } | Inst::Nop => {}
            // A write of a single or a double clears the rest of its vector register, so it
            // writes the whole, as a w register's write does its x.
            Inst::FpOneSource { rd, rn, .. } => {
                self.reads_fp(rn);
                self.writes_fp(rd);
            }
            Inst::FpTwoSource { rd, rn, rm, .. } | Inst::FpCondSelect { rd, rn, rm, .. } => {
                self.reads_fp(rn);
                self.reads_fp(rm);
                self.writes_fp(rd);
            }
            Inst::FpThreeSource { rd, rn, rm, ra, .. } => {
                self.reads_fp(rn);
                self.reads_fp(rm);
                self.reads_fp(ra);
                self.writes_fp(rd);
            }
            Inst::FpCompare { rn, rm, .. } => {
                self.reads_fp(rn);
                if let Some(rm) = rm {
                    self.reads_fp(rm);
                }
            }
            Inst::FpMoveImm { rd, .. } => self.writes_fp(rd),
            Inst::FpConvert { op, fp, gpr } => {
                if to_general(op) {
                    self.reads_fp(fp);
                    self.writes_register(gpr);
                } else {
                    self.reads_register(gpr);
                    self.writes_fp(fp);
                }
            }
            Inst::FpLoadStore { load, rt, address } => {
                self.address(address);
                if load {
                    self.writes_fp(rt);
                } else {
                    self.reads_fp(rt);
                }
            }
            Inst::FpPair {
                load,
                rt,
                rt2,
                address,
            } => {
                self.address(address);
                for register in [rt, rt2] {
                    if load {
                        self.writes_fp(register);
                    } else {
                        self.reads_fp(register);
                    }
                }
            }
            // A vector write of any arrangement writes the whole V register, as the CPU clears
            // what's above the lanes it writes; the operations that keep part of the destination
            // read it too.
            Inst::VectorThree { op, rd, rn, rm } => {
                self.reads_v(rn.number);
                self.reads_v(rm.number);
                if op.reads_destination() {
                    self.reads_v(rd.number);
                }
                self.writes_v(rd.number);
            }
            Inst::Pmull { rd, rn, rm } => {
                self.reads_v(rn.number);
                self.reads_v(rm.number);
                self.writes_v(rd);
            }
            Inst::VectorTwo { op, rd, rn } => {
                self.reads_v(rn.number);
                if op.reads_destination(rd) {
                    self.reads_v(rd.number);
                }
                self.writes_v(rd.number);
            }
            Inst::VectorAcross { rd, rn, .. } => {
                self.reads_v(rn.number);
                self.writes_v(rd.number);
            }
            Inst::VectorShift { op, rd, rn, .. } => {
                self.reads_v(rn.number);
                if op.reads_destination(rd) {
                    self.reads_v(rd.number);
                }
                self.writes_v(rd.number);
            }
            Inst::VectorPermute { rd, rn, rm, .. } | Inst::VectorExtract { rd, rn, rm, .. } => {
                self.reads_v(rn.number);
                self.reads_v(rm.number);
                self.writes_v(rd.number);
            }
            Inst::VectorTable {
                keep,
                rd,
                table,
                count,
                rm,
            } => {
                for offset in 0..count {
                    self.reads_v((table + offset) % 32);
                }
                self.reads_v(rm.number);
                if keep {
                    self.reads_v(rd.number);
                }
                self.writes_v(rd.number);
            }
            Inst::VectorDup { rd, from } => {
                self.reads_source(from);
                self.writes_v(rd.number);
            }
            Inst::VectorInsert { rd, from } => {
                self.reads_source(from);
                self.reads_v(rd.number);
                self.writes_v(rd.number);
            }
            Inst::VectorToGeneral { rd, rn, .. } => {
                self.reads_v(rn.number);
                self.writes_register(rd);
            }
            Inst::VectorImmediate { op, rd, .. } => {
                if matches!(op, VImmediate::Orr | VImmediate::Bic) {
                    self.reads_v(rd.number);
                }
                self.writes_v(rd.number);
            }
            Inst::VectorLoadStore { load, rt, address } => {
                self.address(address);
                if load {
                    self.writes_v(rt);
                } else {
                    self.reads_v(rt);
                }
            }
            Inst::VectorPair {
                load,
                rt,
                rt2,
                address,
            } => {
                self.address(address);
                for register in [rt, rt2] {
                    if load {
                        self.writes_v(register);
                    } else {
                        self.reads_v(register);
                    }
                }
            }
            Inst::VectorStructures {
                load,
                first,
                count,
                base,
                step,
                ..
            } => {
                self.reads_register(base);
                match step {
                    Some(Writeback::Register(by)) => {
                        self.reads_register(by);
                        self.writes_register(base);
                    }
                    Some(Writeback::Bytes) => self.writes_register(base),
                    None => {}
                }
                for offset in 0..count {
                    let register = (first + offset) % 32;
                    if load {
                        self.writes_v(register);
                    } else {
                        self.reads_v(register);
                    }
                }
            }
        }
    }

    /// Gathers what an SVE instruction reads and writes of the registers the rules follow.
    fn read_sve(&mut self, sve: Sve) {
        match sve {
            Sve::Ptrue { .. } | Sve::PredLogic { .. } => {}
            Sve::Whilelo { rn, rm, .. } => {
                self.reads_register(rn);
                self.reads_register(rm);
            }
            Sve::Cntp { rd, .. } | Sve::Count { add: false, rd, .. } => self.writes_register(rd),
            Sve::Count { add: true, rd, .. } => {
                self.reads_register(rd);
                self.writes_register(rd);
            }
            Sve::Access {
                store,
                zt,
                base,
                index,
                ..
            } => {
                self.reads_register(base);
                self.reads_register(index);
                if store {
                    self.reads_v(zt);
                } else {
                    self.writes_v(zt);
                }
            }
            Sve::Compare { zn, against, .. } => {
                self.reads_v(zn);
                if let Against::Vector(zm) = against {
                    self.reads_v(zm);
                }
            }
            Sve::Arith { zd, zn, zm, .. }
            | Sve::Logic { zd, zn, zm, .. }
            | Sve::Sel { zd, zn, zm, .. } => {
                self.reads_v(zn);
                self.reads_v(zm);
                self.writes_v(zd);
            }
            Sve::Cnt { zd, zn, .. } => {
                // The inactive lanes keep the destination's bytes.
                self.reads_v(zd);
                self.reads_v(zn);
                self.writes_v(zd);
            }
            Sve::DupImm { zd, .. } => self.writes_v(zd),
            Sve::Cpy { zd, merge, .. } => {
                if merge {
                    self.reads_v(zd);
                }
                self.writes_v(zd);
            }
            Sve::DupReg { zd, rn } => {
                self.reads_register(rn);
                self.writes_v(zd);
            }
            Sve::Uaddv { vd, zn, .. } => {
                self.reads_v(zn);
                self.writes_v(vd);
            }
            Sve::Insr { zdn, rm } => {
                self.reads_v(zdn);
                self.reads_register(rm);
                self.writes_v(zdn);
            }
            Sve::Lastb { rd, zn, .. } => {
                self.reads_v(zn);
                self.writes_register(rd);
            }
        }
    }

    fn reads_v(&mut self, number: u8) {
        self.reads.insert(v(number));
    }

    fn writes_v(&mut self, number: u8) {
        self.writes.insert(v(number));
    }

    /// The register a `dup` or an insert reads.
    fn reads_source(&mut self, from: Source) {
        match from {
            Source::Lane(lane) => self.reads_v(lane.number),
            Source::General(register) => self.reads_register(register),
        }
    }

    /// The registers an address reads, and the base it writes back.
    fn address(&mut self, address: Address) {
        match address {
            Address::Offset { base, .. } => self.reads_register(base),
            Address::PreIndex { base, .. } | Address::PostIndex { base, .. } => {
                self.reads_register(base);
                self.writes_register(base);
            }
            Address::Register { base, index, .. } => {
                self.reads_register(base);
                self.reads_register(index);
            }
        }
    }
}
