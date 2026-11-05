//! x86-64: each instruction's facts as iced decodes it, and the System V calling convention
//! lengua's x86-64 programs follow.

use iced_x86::{
    ConditionCode, FlowControl, Instruction, InstructionInfoFactory, Mnemonic, OpAccess, OpKind,
    Register,
};

use super::{Abi, Cond, Control, Insn, MemRef, Op, Operand, Reg, RegSet, Stack, Value};

const fn x86(register: Register) -> Reg {
    Reg::X86(register)
}

/// System V, as lengua's x86-64 programs follow it.
pub const ABI: Abi = Abi {
    registers: &[
        x86(Register::RAX),
        x86(Register::RCX),
        x86(Register::RDX),
        x86(Register::RBX),
        x86(Register::RSP),
        x86(Register::RBP),
        x86(Register::RSI),
        x86(Register::RDI),
        x86(Register::R8),
        x86(Register::R9),
        x86(Register::R10),
        x86(Register::R11),
        x86(Register::R12),
        x86(Register::R13),
        x86(Register::R14),
        x86(Register::R15),
    ],
    stack_pointer: x86(Register::RSP),
    return_address: 8,
    syscall_number: x86(Register::RAX),
    syscall_arguments: &[
        x86(Register::RDI),
        x86(Register::RSI),
        x86(Register::RDX),
        x86(Register::R10),
        x86(Register::R8),
        x86(Register::R9),
    ],
    arguments: &[
        x86(Register::RDI),
        x86(Register::RSI),
        x86(Register::RDX),
        x86(Register::RCX),
        x86(Register::R8),
        x86(Register::R9),
    ],
    result: x86(Register::RAX),
    returns: [x86(Register::RAX), x86(Register::RDX)],
    first_argument: x86(Register::RDI),
    assertion_number: x86(Register::EDI),
    worker_slot: x86(Register::RBX),
    link: None,
    caller_saved: &[
        x86(Register::RAX),
        x86(Register::RCX),
        x86(Register::RDX),
        x86(Register::RSI),
        x86(Register::RDI),
        x86(Register::R8),
        x86(Register::R9),
        x86(Register::R10),
        x86(Register::R11),
    ],
    syscall_clobbers: &[x86(Register::RAX), x86(Register::RCX), x86(Register::R11)],
    edge_arguments: &[
        x86(Register::RDI),
        x86(Register::RSI),
        x86(Register::RDX),
        x86(Register::R10),
        x86(Register::R8),
        x86(Register::R9),
    ],
    // rax, rcx and r11 as a system call, every vector register (the edge may use any) and the mask
    // registers.
    edge_clobbers: &[
        x86(Register::RAX),
        x86(Register::RCX),
        x86(Register::R11),
        x86(Register::ZMM0),
        x86(Register::ZMM1),
        x86(Register::ZMM2),
        x86(Register::ZMM3),
        x86(Register::ZMM4),
        x86(Register::ZMM5),
        x86(Register::ZMM6),
        x86(Register::ZMM7),
        x86(Register::ZMM8),
        x86(Register::ZMM9),
        x86(Register::ZMM10),
        x86(Register::ZMM11),
        x86(Register::ZMM12),
        x86(Register::ZMM13),
        x86(Register::ZMM14),
        x86(Register::ZMM15),
        x86(Register::ZMM16),
        x86(Register::ZMM17),
        x86(Register::ZMM18),
        x86(Register::ZMM19),
        x86(Register::ZMM20),
        x86(Register::ZMM21),
        x86(Register::ZMM22),
        x86(Register::ZMM23),
        x86(Register::ZMM24),
        x86(Register::ZMM25),
        x86(Register::ZMM26),
        x86(Register::ZMM27),
        x86(Register::ZMM28),
        x86(Register::ZMM29),
        x86(Register::ZMM30),
        x86(Register::ZMM31),
        x86(Register::K0),
        x86(Register::K1),
        x86(Register::K2),
        x86(Register::K3),
        x86(Register::K4),
        x86(Register::K5),
        x86(Register::K6),
        x86(Register::K7),
    ],
    callee_saved: &[
        x86(Register::RBX),
        x86(Register::RBP),
        x86(Register::R12),
        x86(Register::R13),
        x86(Register::R14),
        x86(Register::R15),
    ],
};

/// `register`'s place in a [`RegSet`]: the 16 general-purpose registers, then the 32 vector
/// registers, then the 8 mask registers, each whole.
#[must_use]
pub fn slot(register: Register) -> Option<u32> {
    let full = register.full_register();
    let number = u32::try_from(full.number()).ok()?;
    if full.is_gpr64() {
        Some(number)
    } else if full.is_zmm() {
        Some(16 + number)
    } else if full.is_k() {
        Some(48 + number)
    } else {
        None
    }
}

/// When `condition` holds.
const fn cond(condition: ConditionCode) -> Cond {
    match condition {
        ConditionCode::o => Cond::Overflow,
        ConditionCode::no => Cond::NotOverflow,
        ConditionCode::b => Cond::Below,
        ConditionCode::ae => Cond::AboveEqual,
        ConditionCode::e => Cond::Equal,
        ConditionCode::ne => Cond::NotEqual,
        ConditionCode::be => Cond::BelowEqual,
        ConditionCode::a => Cond::Above,
        ConditionCode::s => Cond::Sign,
        ConditionCode::ns => Cond::NotSign,
        ConditionCode::p => Cond::Parity,
        ConditionCode::np => Cond::NotParity,
        ConditionCode::l => Cond::Less,
        ConditionCode::ge => Cond::GreaterEqual,
        ConditionCode::le => Cond::LessEqual,
        ConditionCode::g => Cond::Greater,
        _ => Cond::Other,
    }
}

/// An immediate operand's value, as the assembler's bits read signed; 0 for an operand that
/// isn't a number, as iced's `immediate` answers in a release build.
fn immediate(instruction: &Instruction, operand: u32) -> i64 {
    instruction
        .try_immediate(operand)
        .map_or(0, |value| i64::from_ne_bytes(value.to_ne_bytes()))
}

/// Operand `operand`, as an idiom needs it.
fn operand(instruction: &Instruction, operand: u32) -> Operand {
    match instruction.op_kind(operand) {
        OpKind::Register => Operand::Reg(x86(instruction.op_register(operand))),
        OpKind::Memory => Operand::Mem,
        OpKind::NearBranch16 | OpKind::NearBranch32 | OpKind::NearBranch64 => {
            Operand::Target(instruction.near_branch_target())
        }
        _ => Operand::Imm(immediate(instruction, operand)),
    }
}

/// Whether the instruction's first operand is the register `register`, as written.
fn first_is(instruction: &Instruction, register: Register) -> bool {
    instruction.op_count() >= 1
        && instruction.op_kind(0) == OpKind::Register
        && instruction.op_register(0) == register
}

/// The idiom `instruction`, called `name`, is.
fn op(instruction: &Instruction, name: &str) -> Op {
    let first = (instruction.op_count() >= 1 && instruction.op_kind(0) == OpKind::Register)
        .then(|| x86(instruction.op_register(0)));
    let second_kind = (instruction.op_count() == 2).then(|| instruction.op_kind(1));
    let second_is_number =
        second_kind.is_some_and(|kind| !matches!(kind, OpKind::Register | OpKind::Memory));
    if name.starts_with("nop") || name.starts_with("prefetch") {
        return Op::Hint;
    }
    match (instruction.mnemonic(), first) {
        (Mnemonic::Mov, Some(dst)) if second_is_number => Op::MovImm {
            dst,
            value: immediate(instruction, 1),
        },
        (Mnemonic::Mov, _) => Op::Move,
        (Mnemonic::Lea, Some(dst)) => Op::AddressOf { dst },
        (Mnemonic::Cmp, _) if instruction.op_count() == 2 => Op::Compare {
            left: operand(instruction, 0),
            right: operand(instruction, 1),
        },
        (Mnemonic::Test, _) => Op::Test,
        (Mnemonic::Dec, Some(counter)) => Op::Decrement {
            counter: counter.full(),
            flags: true,
        },
        (Mnemonic::Sub, Some(counter))
            if second_kind.is_some_and(|kind| kind != OpKind::Register)
                && immediate(instruction, 1) == 1 =>
        {
            Op::Decrement {
                counter: counter.full(),
                flags: true,
            }
        }
        (Mnemonic::Push, register) => Op::Push([register.map(Reg::full), None]),
        (Mnemonic::Pop, register) => Op::Pop([register.map(Reg::full), None]),
        (Mnemonic::Xchg, _) => Op::Exchange,
        _ => Op::Other,
    }
}

/// What `instruction`, called `name`, puts in its first operand.
fn value(instruction: &Instruction, name: &str) -> Value {
    let first = || operand(instruction, 0);
    let second = || operand(instruction, 1);
    match name {
        "lea" => Value::Address,
        "xchg" => Value::Copy(second()),
        "movzx" | "movsx" | "movsxd" => Value::Number,
        "mov" if instruction.op_kind(1) == OpKind::Memory => Value::Load,
        "mov" => Value::Copy(second()),
        "add" => Value::Add(first(), second()),
        "sub" => Value::Sub(first(), second()),
        "inc" => Value::Add(first(), Operand::Imm(1)),
        "dec" => Value::Sub(first(), Operand::Imm(1)),
        "and" => Value::And(first(), second()),
        // A register xor itself is zero.
        "xor"
            if matches!(
                (first(), second()),
                (Operand::Reg(a), Operand::Reg(b)) if a.full() == b.full()
            ) =>
        {
            Value::Number
        }
        "pop" => Value::Pop,
        _ if name.starts_with("cmov") => Value::Select(second()),
        _ => Value::Other,
    }
}

/// What `instruction` does to rsp, given what it writes.
fn stack(instruction: &Instruction, name: &str, writes: RegSet) -> Stack {
    match instruction.mnemonic() {
        Mnemonic::Push | Mnemonic::Pop => {
            let wide = match instruction.op_kind(0) {
                OpKind::Register => instruction.op_register(0).size() == 8,
                _ => instruction.stack_pointer_increment().abs() == 8,
            };
            if !wide {
                return Stack::Refused(format!(
                    "`{name}` moves 16 bits; push and pop 64-bit registers"
                ));
            }
            Stack::Moves(if instruction.mnemonic() == Mnemonic::Push {
                8
            } else {
                -8
            })
        }
        Mnemonic::Sub | Mnemonic::Add if first_is(instruction, Register::RSP) => {
            if matches!(instruction.op_kind(1), OpKind::Register | OpKind::Memory) {
                return Stack::Refused(format!("`{name} rsp` uses a register or memory"));
            }
            let amount = immediate(instruction, 1);
            Stack::Moves(if instruction.mnemonic() == Mnemonic::Sub {
                amount
            } else {
                amount.wrapping_neg()
            })
        }
        Mnemonic::Call | Mnemonic::Ret | Mnemonic::Syscall => Stack::Same,
        _ if writes.contains(x86(Register::RSP)) => Stack::Refused(format!("`{name}` changes rsp")),
        _ => Stack::Same,
    }
}

/// The facts of `instruction`, with `factory` for the registers and memory it uses.
#[must_use]
pub fn facts(instruction: &Instruction, factory: &mut InstructionInfoFactory) -> Insn {
    let name = format!("{:?}", instruction.mnemonic()).to_ascii_lowercase();
    let info = factory.info(instruction);
    let (mut reads, mut writes) = (RegSet::EMPTY, RegSet::EMPTY);
    for used in info.used_registers() {
        let register = x86(used.register());
        if matches!(
            used.access(),
            OpAccess::Read | OpAccess::CondRead | OpAccess::ReadWrite | OpAccess::ReadCondWrite
        ) {
            reads.insert(register);
        }
        if matches!(
            used.access(),
            OpAccess::Write | OpAccess::CondWrite | OpAccess::ReadWrite | OpAccess::ReadCondWrite
        ) {
            writes.insert(register);
        }
    }
    let reads_memory = info.used_memory().iter().any(|memory| {
        matches!(
            memory.access(),
            OpAccess::Read | OpAccess::CondRead | OpAccess::ReadWrite | OpAccess::ReadCondWrite
        )
    });
    let writes_memory = info.used_memory().iter().any(|memory| {
        matches!(
            memory.access(),
            OpAccess::Write | OpAccess::ReadWrite | OpAccess::CondWrite | OpAccess::ReadCondWrite
        )
    });
    let control = match instruction.flow_control() {
        FlowControl::Next => Control::Next,
        FlowControl::UnconditionalBranch => Control::Jump(instruction.near_branch_target()),
        FlowControl::ConditionalBranch => Control::Branch {
            target: instruction.near_branch_target(),
            cond: cond(instruction.condition_code()),
        },
        FlowControl::Return => Control::Return,
        FlowControl::Call if instruction.mnemonic() == Mnemonic::Syscall => Control::Syscall,
        FlowControl::Call if instruction.mnemonic() == Mnemonic::Call => {
            Control::Call(instruction.near_branch_target())
        }
        FlowControl::IndirectBranch => Control::IndirectJump,
        FlowControl::IndirectCall => Control::IndirectCall,
        _ => Control::Other,
    };
    let memory_operand =
        (0..instruction.op_count()).find(|operand| instruction.op_kind(*operand) == OpKind::Memory);
    let memory = memory_operand.map(|operand| {
        let access = info.op_access(operand);
        let register = |register: Register| {
            (!matches!(register, Register::None | Register::RIP | Register::EIP))
                .then_some(x86(register))
        };
        MemRef {
            base_moves: None,
            base: register(instruction.memory_base()),
            index: register(instruction.memory_index()),
            scale: instruction.memory_index_scale(),
            displacement: i64::from_ne_bytes(instruction.memory_displacement64().to_ne_bytes()),
            pc_relative: instruction.is_ip_rel_memory_operand(),
            width: if instruction.mnemonic() == Mnemonic::Lea {
                0
            } else {
                u64::try_from(instruction.memory_size().size()).unwrap_or(0)
            },
            reads: matches!(
                access,
                OpAccess::Read | OpAccess::CondRead | OpAccess::ReadWrite | OpAccess::ReadCondWrite
            ),
            writes: matches!(
                access,
                OpAccess::Write
                    | OpAccess::CondWrite
                    | OpAccess::ReadWrite
                    | OpAccess::ReadCondWrite
            ),
        }
    });
    let dst = (instruction.op_count() >= 1 && instruction.op_kind(0) == OpKind::Register)
        .then(|| x86(instruction.op_register(0)));
    let (op, value, stack) = (
        op(instruction, &name),
        value(instruction, &name),
        stack(instruction, &name, writes),
    );
    Insn {
        address: instruction.ip(),
        mnemonic: name,
        control,
        reads,
        writes,
        dst,
        operands: (0..instruction.op_count())
            .map(|index| operand(instruction, index))
            .collect(),
        memory,
        reads_memory,
        writes_memory,
        op,
        value,
        stack,
        call_clobbers: None,
    }
}
