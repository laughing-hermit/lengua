//! Memory safety, the M rules of dialect 3, which the checker settles statically wherever it can:
//!
//! - **M1, owners.** Every address comes from a pointer with an owner: a data symbol, the current
//!   frame, a pointer the caller lent in a register, or the program's arguments. None is made
//!   from a plain number.
//! - **M2, bounds.** An access at a constant offset from a symbol's address stays inside it.
//!   Indexed accesses keep rule 9's checks, which the checker holds to the owner's size.
//! - **M3, lifetimes.** An address in the current frame is never stored outside it or returned,
//!   and every access at a constant offset from rsp stays inside the function's own frame: never
//!   its return address or its caller's frame.
//! - **M5, initialization.** No frame byte is read before every path has written it.
//! - **M6, immutability.** Read-only data is never written, through any register, and no memory
//!   is both writable and executable.
//! - **M8, stack size.** The deepest path stays within [`MAX_STACK_BYTES`].
//!
//! Exclusive writes (M4) and threads (M7) have their own passes. What no rule settles, such as a
//! pointer loaded from memory or an index checked against a length in a register, the proof
//! settles for every input.

use std::collections::{BTreeMap, BTreeSet};

use iced_x86::{Instruction, InstructionInfoFactory, OpAccess, OpKind, Register};

use super::cfg::{ASSERT_FAIL, CALLER_SAVED, Flow};
use super::decode::{MappedFunction, Program, mnemonic};
use super::{Rule, Violation};

/// The most stack a program may use on any path.
pub const MAX_STACK_BYTES: u64 = 1 << 20;

/// The 64-bit general-purpose registers, in the order states keep them.
const REGISTERS: [Register; 16] = [
    Register::RAX,
    Register::RCX,
    Register::RDX,
    Register::RBX,
    Register::RSP,
    Register::RBP,
    Register::RSI,
    Register::RDI,
    Register::R8,
    Register::R9,
    Register::R10,
    Register::R11,
    Register::R12,
    Register::R13,
    Register::R14,
    Register::R15,
];

/// The registers a caller lends values in (System V).
const ARGUMENTS: [Register; 6] = [
    Register::RDI,
    Register::RSI,
    Register::RDX,
    Register::RCX,
    Register::R8,
    Register::R9,
];

/// What a register's value is, as far as addresses go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    /// Not followed: loaded from memory, or different on different paths.
    Unknown,
    /// A plain number, never an address.
    Number,
    /// An address inside this data symbol, at this offset when it's known.
    Data {
        /// The symbol.
        symbol: String,
        /// The offset from its start.
        offset: Option<i64>,
    },
    /// An address inside the current frame: `key` is how far its byte lies below the frame's
    /// top, when it's known.
    Frame {
        /// The distance below the frame's top.
        key: Option<i64>,
    },
    /// Whatever the caller lent in this register.
    Argument(Register),
    /// The program's argument strings, or the array of them `_start` finds on its stack.
    Arguments,
}

impl Owner {
    fn is_pointer(&self) -> bool {
        matches!(
            self,
            Self::Data { .. } | Self::Frame { .. } | Self::Argument(_) | Self::Arguments
        )
    }

    /// What either path's value may be.
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            _ if self == other => self.clone(),
            (Self::Data { symbol, .. }, Self::Data { symbol: other, .. }) if symbol == other => {
                Self::Data {
                    symbol: symbol.clone(),
                    offset: None,
                }
            }
            (Self::Frame { .. }, Self::Frame { .. }) => Self::Frame { key: None },
            _ => Self::Unknown,
        }
    }

    /// The same owner, `amount` bytes further on, where offsets are followed.
    fn moved(&self, amount: Option<i64>) -> Self {
        match self {
            Self::Data { symbol, offset } => Self::Data {
                symbol: symbol.clone(),
                offset: offset.zip(amount).map(|(offset, amount)| offset + amount),
            },
            Self::Frame { key } => Self::Frame {
                key: key.zip(amount).map(|(key, amount)| key - amount),
            },
            other => other.clone(),
        }
    }
}

/// Each register's owner before an instruction, and which frame bytes every path has written.
#[derive(Debug, Clone, PartialEq, Eq)]
struct State {
    registers: Vec<Owner>,
    /// The frame bytes written on every path, by their distance below the frame's top.
    written: BTreeSet<i64>,
}

impl State {
    fn get(&self, register: Register) -> Owner {
        index(register).map_or(Owner::Unknown, |at| self.registers[at].clone())
    }

    fn set(&mut self, register: Register, owner: Owner) {
        if let Some(at) = index(register)
            && REGISTERS[at] != Register::RSP
        {
            self.registers[at] = owner;
        }
    }

    fn join(&self, other: &Self) -> Self {
        Self {
            registers: self
                .registers
                .iter()
                .zip(&other.registers)
                .map(|(a, b)| a.join(b))
                .collect(),
            written: self.written.intersection(&other.written).copied().collect(),
        }
    }
}

fn index(register: Register) -> Option<usize> {
    let full = register.full_register();
    REGISTERS.iter().position(|known| *known == full)
}

/// Where a function's frame starts: the frame's top is at rsp plus the depth less this, the
/// return address for every function but `_start`, which has none.
fn frame_base(function: &MappedFunction<'_>) -> i64 {
    if function.name() == "_start" { 0 } else { 8 }
}

/// Checks M1, M2, M3, M5 and M6 in every function, then M8 and M6's segments for the program.
pub fn check(
    program: &Program<'_>,
    flows: &BTreeMap<String, Flow>,
    max_stack: u64,
    writable_executable: bool,
    violations: &mut Vec<Violation>,
) {
    let mut factory = InstructionInfoFactory::new();
    for function in &program.functions {
        if function.name() == ASSERT_FAIL {
            continue;
        }
        let Some(flow) = flows.get(function.name()) else {
            continue;
        };
        let states = states(function, flow, program, &mut factory);
        for (at, step) in function.steps.iter().enumerate() {
            let Some(state) = &states[at] else {
                continue;
            };
            let depth = flow.depth[at];
            let found = access(function, at, state, depth, program, &mut factory);
            for (rule, problem, fix) in found {
                violations.push(Violation {
                    rule,
                    line: Some(step.source.line),
                    problem,
                    fix,
                });
            }
        }
    }
    if max_stack > MAX_STACK_BYTES {
        violations.push(Violation {
            rule: Rule::StackSize,
            line: None,
            problem: format!(
                "the program's deepest path uses {max_stack} bytes of stack, more than \
                 {MAX_STACK_BYTES}"
            ),
            fix: "keep large buffers in .bss, sized from the spec's limits, instead of on the \
                  stack"
                .into(),
        });
    }
    if writable_executable {
        violations.push(Violation {
            rule: Rule::Immutability,
            line: None,
            problem: "the executable has memory that is both writable and executable".into(),
            fix: "keep code in .text and data in .data, .bss and .rodata, with no section flags \
                  of your own"
                .into(),
        });
    }
}

/// The state before each instruction, where it can run.
fn states(
    function: &MappedFunction<'_>,
    flow: &Flow,
    program: &Program<'_>,
    factory: &mut InstructionInfoFactory,
) -> Vec<Option<State>> {
    let count = function.steps.len();
    let mut states: Vec<Option<State>> = vec![None; count];
    if count == 0 {
        return states;
    }
    let start = function.name() == "_start";
    let mut entry = State {
        registers: vec![Owner::Unknown; REGISTERS.len()],
        written: BTreeSet::new(),
    };
    if !start {
        for register in ARGUMENTS {
            entry.set(register, Owner::Argument(register));
        }
    }
    states[0] = Some(entry);
    let mut pending = vec![0];
    let mut rounds = 0;
    while let Some(at) = pending.pop() {
        rounds += 1;
        if rounds > count * 64 {
            break;
        }
        let Some(before) = states[at].clone() else {
            continue;
        };
        let after = transfer(function, flow, at, &before, program, factory);
        for &next in &flow.successors[at] {
            let joined = match &states[next] {
                None => after.clone(),
                Some(known) => known.join(&after),
            };
            if states[next].as_ref() != Some(&joined) {
                states[next] = Some(joined);
                pending.push(next);
            }
        }
    }
    states
}

/// The state after the instruction at `at`.
fn transfer(
    function: &MappedFunction<'_>,
    flow: &Flow,
    at: usize,
    before: &State,
    program: &Program<'_>,
    factory: &mut InstructionInfoFactory,
) -> State {
    let instruction = &function.steps[at].machine;
    let name = mnemonic(instruction);
    let mut after = before.clone();
    let depth = flow.depth[at];
    let base = frame_base(function);
    // Frame bytes this instruction writes.
    if let Some(depth) = depth {
        if name == "push" {
            let top = depth + 8 - base;
            after.written.extend((top - 7)..=top);
        } else if writes_memory(instruction, factory)
            && instruction.memory_base() == Register::RSP
            && instruction.memory_index() == Register::None
        {
            let displacement = displacement(instruction);
            let width = width(instruction);
            let first = depth - base - displacement;
            after.written.extend((first - width + 1)..=first);
        }
    }
    match name.as_str() {
        "call" => {
            // A frame address lent to the callee may be written there.
            let lent = ARGUMENTS
                .iter()
                .any(|register| matches!(before.get(*register), Owner::Frame { .. }));
            if lent && let Some(depth) = depth {
                after.written.extend(1..=(depth - base));
            }
            for register in CALLER_SAVED {
                after.set(register, Owner::Unknown);
            }
            return after;
        }
        "syscall" => {
            let lent = ARGUMENTS
                .iter()
                .any(|register| matches!(before.get(*register), Owner::Frame { .. }));
            if lent && let Some(depth) = depth {
                after.written.extend(1..=(depth - base));
            }
            for register in [Register::RAX, Register::RCX, Register::R11] {
                after.set(register, Owner::Number);
            }
            return after;
        }
        _ => {}
    }
    let result = result(function, at, before, depth, program);
    let destination = (instruction.op_count() >= 1 && instruction.op_kind(0) == OpKind::Register)
        .then(|| instruction.op_register(0));
    for used in factory.info(instruction).used_registers() {
        let written = matches!(
            used.access(),
            OpAccess::Write | OpAccess::CondWrite | OpAccess::ReadWrite | OpAccess::ReadCondWrite
        );
        if !written || index(used.register()).is_none() {
            continue;
        }
        let full = used.register().full_register();
        let owner = match destination {
            Some(register) if register.full_register() == full => result.clone(),
            _ if name == "xchg" => before.get(other_operand(instruction, full)),
            _ => Owner::Number,
        };
        after.set(full, owner);
    }
    after
}

/// The other register of an `xchg`.
fn other_operand(instruction: &Instruction, register: Register) -> Register {
    (0..instruction.op_count())
        .filter(|&operand| instruction.op_kind(operand) == OpKind::Register)
        .map(|operand| instruction.op_register(operand))
        .find(|found| found.full_register() != register)
        .unwrap_or(Register::None)
}

/// The owner of what the instruction at `at` writes to its first operand.
fn result(
    function: &MappedFunction<'_>,
    at: usize,
    before: &State,
    depth: Option<i64>,
    program: &Program<'_>,
) -> Owner {
    let instruction = &function.steps[at].machine;
    let name = mnemonic(instruction);
    if instruction.op_count() == 0 || instruction.op_kind(0) != OpKind::Register {
        return Owner::Unknown;
    }
    let destination = instruction.op_register(0);
    let old = before.get(destination);
    // A 32-bit write zeroes the upper half: a truncated address is no address.
    match destination.size() {
        8 => {}
        4 => return Owner::Number,
        _ => {
            return if old == Owner::Number {
                Owner::Number
            } else {
                Owner::Unknown
            };
        }
    }
    let source = |operand: u32| -> Owner {
        match instruction.op_kind(operand) {
            // rsp itself is the address of the frame's lowest byte.
            OpKind::Register if instruction.op_register(operand) == Register::RSP => Owner::Frame {
                key: depth.map(|depth| depth - frame_base(function)),
            },
            OpKind::Register => before.get(instruction.op_register(operand)),
            OpKind::Memory => Owner::Unknown,
            _ => Owner::Number,
        }
    };
    let immediate = || -> Option<i64> {
        (instruction.op_count() >= 2
            && !matches!(instruction.op_kind(1), OpKind::Register | OpKind::Memory))
        .then(|| i64::from_ne_bytes(instruction.immediate(1).to_ne_bytes()))
    };
    match name.as_str() {
        "lea" => address_owner(function, at, before, depth, program),
        "xchg" => source(1),
        "movzx" | "movsx" | "movsxd" => Owner::Number,
        "mov" => match instruction.op_kind(1) {
            OpKind::Memory => load_owner(function, at, before, depth),
            _ => source(1),
        },
        "add" => {
            let other = source(1);
            match (old.is_pointer(), other.is_pointer()) {
                (true, false) if other == Owner::Number => old.moved(immediate()),
                (false, true) if old == Owner::Number => other,
                (false, false) if old == Owner::Number && other == Owner::Number => Owner::Number,
                _ => Owner::Unknown,
            }
        }
        "sub" => {
            let other = source(1);
            match (old.is_pointer(), other.is_pointer()) {
                (true, false) if other == Owner::Number => {
                    old.moved(immediate().map(|amount| -amount))
                }
                // The distance between two addresses is a length.
                (true, true) => Owner::Number,
                (false, false) if old == Owner::Number && other == Owner::Number => Owner::Number,
                _ if instruction.op_kind(1) == OpKind::Register
                    && instruction.op_register(1).full_register()
                        == destination.full_register() =>
                {
                    Owner::Number
                }
                _ => Owner::Unknown,
            }
        }
        "inc" => old.moved(Some(1)),
        "dec" => old.moved(Some(-1)),
        "and" if old.is_pointer() && immediate().is_some() => old.moved(None),
        "xor"
            if instruction.op_kind(1) == OpKind::Register
                && instruction.op_register(1).full_register() == destination.full_register() =>
        {
            Owner::Number
        }
        "pop" => Owner::Unknown,
        _ if name.starts_with("cmov") => old.join(&source(1)),
        _ => {
            let reads_pointer = (0..instruction.op_count()).any(|operand| {
                instruction.op_kind(operand) == OpKind::Register
                    && before.get(instruction.op_register(operand)).is_pointer()
            }) || old.is_pointer();
            let reads_unknown = (0..instruction.op_count()).any(|operand| {
                instruction.op_kind(operand) == OpKind::Register
                    && before.get(instruction.op_register(operand)) == Owner::Unknown
            });
            if reads_pointer || reads_unknown {
                Owner::Unknown
            } else {
                Owner::Number
            }
        }
    }
}

/// The owner of the address an instruction's memory operand computes.
fn address_owner(
    function: &MappedFunction<'_>,
    at: usize,
    before: &State,
    depth: Option<i64>,
    program: &Program<'_>,
) -> Owner {
    let instruction = &function.steps[at].machine;
    let base = instruction.memory_base();
    let index = instruction.memory_index();
    if base == Register::RIP {
        let target = instruction.memory_displacement64();
        return program
            .data_at(target)
            .and_then(|(symbol, data)| {
                Some(Owner::Data {
                    symbol: symbol.to_owned(),
                    offset: Some(i64::try_from(target.checked_sub(data.address)?).ok()?),
                })
            })
            .unwrap_or(Owner::Unknown);
    }
    let displacement = displacement(instruction);
    if base == Register::RSP {
        let key = (index == Register::None)
            .then(|| depth.map(|depth| depth - frame_base(function) - displacement))
            .flatten();
        return Owner::Frame { key };
    }
    let base_owner = if base == Register::None {
        Owner::Number
    } else {
        before.get(base)
    };
    let index_owner = if index == Register::None {
        Owner::Number
    } else {
        before.get(index)
    };
    match (base_owner.is_pointer(), index_owner.is_pointer()) {
        (true, false) if index_owner == Owner::Number => {
            base_owner.moved((index == Register::None).then_some(displacement))
        }
        (false, true) if base_owner == Owner::Number => index_owner.moved(None),
        (false, false) if base_owner == Owner::Number && index_owner == Owner::Number => {
            Owner::Number
        }
        _ => Owner::Unknown,
    }
}

/// The owner of a 64-bit value an instruction loads: `_start`'s argument count and strings on
/// the stack it starts with, an entry of the array of strings, or otherwise not followed.
fn load_owner(
    function: &MappedFunction<'_>,
    at: usize,
    before: &State,
    depth: Option<i64>,
) -> Owner {
    let instruction = &function.steps[at].machine;
    if instruction.op_register(0).size() != 8 {
        return Owner::Number;
    }
    let base = instruction.memory_base();
    if function.name() == "_start"
        && base == Register::RSP
        && instruction.memory_index() == Register::None
        && let Some(depth) = depth
    {
        let displacement = displacement(instruction);
        // The count sits at the stack's top as the program starts, the strings above it.
        if displacement == depth {
            return Owner::Number;
        }
        if displacement > depth {
            return Owner::Arguments;
        }
    }
    if base != Register::None && before.get(base) == Owner::Arguments {
        return Owner::Arguments;
    }
    Owner::Unknown
}

/// What breaks a rule in the instruction at `at`'s memory access, if anything.
fn access(
    function: &MappedFunction<'_>,
    at: usize,
    before: &State,
    depth: Option<i64>,
    program: &Program<'_>,
    factory: &mut InstructionInfoFactory,
) -> Vec<(Rule, String, String)> {
    let step = &function.steps[at];
    let instruction = &step.machine;
    let name = mnemonic(instruction);
    let text = &step.source.text;
    let mut found = Vec::new();
    // M3: an address in this frame never leaves it, stored or returned.
    if name == "ret" {
        for register in [Register::RAX, Register::RDX] {
            if matches!(before.get(register), Owner::Frame { .. }) {
                found.push((
                    Rule::Lifetimes,
                    format!(
                        "{} returns an address in its own frame in {}, which ends as it returns",
                        function.name(),
                        register_name(register)
                    ),
                    "return a value, or let the caller lend the buffer to write into".into(),
                ));
            }
        }
        return found;
    }
    let has_memory =
        (0..instruction.op_count()).any(|operand| instruction.op_kind(operand) == OpKind::Memory);
    if !has_memory || name == "lea" || name.starts_with("nop") || name.starts_with("prefetch") {
        return found;
    }
    let writes = writes_memory(instruction, factory);
    let base = instruction.memory_base();
    let index = instruction.memory_index();
    let unchecked = step.source.unsafe_block.is_some();
    // M2: a bit test in memory with a register offset reaches as far as the offset goes.
    if matches!(name.as_str(), "bt" | "bts" | "btr" | "btc")
        && instruction.op_kind(0) == OpKind::Memory
        && instruction.op_kind(1) == OpKind::Register
    {
        found.push((
            Rule::Bounds,
            format!(
                "`{text}` tests a bit in memory at an offset in a register, which can reach past \
                 its operand"
            ),
            "load the bytes into a register and test the bit there, or use a constant bit \
             offset"
                .into(),
        ));
    }
    if writes && base != Register::RSP {
        let stored = (0..instruction.op_count())
            .filter(|&operand| instruction.op_kind(operand) == OpKind::Register)
            .map(|operand| instruction.op_register(operand))
            .find(|register| matches!(before.get(*register), Owner::Frame { .. }));
        let into_frame = matches!(before.get(base), Owner::Frame { .. });
        if let Some(register) = stored
            && !into_frame
        {
            found.push((
                Rule::Lifetimes,
                format!(
                    "`{text}` stores {}, an address in this function's frame, outside the frame",
                    register_name(register)
                ),
                "lend a frame address to a callee in a register only; never store it where it \
                 could outlive the function"
                    .into(),
            ));
        }
    }
    // Rule 6 holds [rip + name] to its symbol.
    if instruction.is_ip_rel_memory_operand() {
        return found;
    }
    // M3: constant offsets from rsp stay inside the function's own frame.
    if base == Register::RSP && index == Register::None {
        if let Some(depth) = depth {
            let displacement = displacement(instruction);
            let width = width(instruction);
            let own = depth - frame_base(function);
            let start = function.name() == "_start";
            let inside = displacement >= 0 && displacement + width <= own;
            // _start reads the argument count and strings above its frame.
            let reads_arguments = start && !writes && displacement >= 0;
            if !inside && !reads_arguments {
                found.push((
                    Rule::Lifetimes,
                    format!(
                        "`{text}` reaches past this function's frame of {own} bytes, into {}",
                        if start {
                            "the program's arguments"
                        } else {
                            "its return address or its caller's frame"
                        }
                    ),
                    "keep every access at [rsp + d] inside the frame the function made with push \
                     and sub rsp"
                        .into(),
                ));
            } else if inside && name != "pop" && reads_memory(instruction, factory) {
                // M5: what's read was written first, on every path.
                let first = own - displacement;
                let unwritten = ((first - width + 1)..=first)
                    .rev()
                    .find(|key| !before.written.contains(key));
                if let Some(key) = unwritten {
                    found.push((
                        Rule::Initialization,
                        format!(
                            "`{text}` reads the frame at rsp + {}, which some path reaches \
                             without writing",
                            own - key
                        ),
                        "write every frame byte before reading it, on every path".into(),
                    ));
                }
            }
        }
        return found;
    }
    if base == Register::None && index == Register::None {
        found.push((
            Rule::Owners,
            format!("`{text}` uses a fixed address, which no pointer owns"),
            "address data symbols as [rip + name], and buffers through a pointer to them".into(),
        ));
        return found;
    }
    let base_owner = if base == Register::None {
        Owner::Number
    } else {
        before.get(base)
    };
    let index_owner = if index == Register::None {
        Owner::Number
    } else {
        before.get(index)
    };
    // M1: an address comes from a pointer, never a plain number.
    if base_owner == Owner::Number && !index_owner.is_pointer() {
        found.push((
            Rule::Owners,
            format!(
                "`{text}` uses {} as an address, but it holds a plain number here, not a pointer \
                 into a buffer",
                register_name(if base == Register::None { index } else { base })
            ),
            "address memory only through a pointer from lea [rip + name], the frame, a \
             pointer argument or the program's arguments"
                .into(),
        ));
        return found;
    }
    let owner = if base_owner.is_pointer() {
        base_owner
    } else {
        index_owner
    };
    if let Owner::Data { symbol, offset } = &owner
        && let Some(data) = program.data.get(symbol)
    {
        // M6: read-only data stays read-only, through any register.
        if writes && !data.writable {
            found.push((
                Rule::Immutability,
                format!("`{text}` writes to read-only {symbol}"),
                "move data the program changes to .data or .bss".into(),
            ));
        }
        // M2: a constant offset from the symbol's address stays inside it.
        if let Some(offset) = offset
            && index == Register::None
            && !unchecked
        {
            let first = offset + displacement(instruction);
            let last = first + width(instruction) - 1;
            let size = i64::try_from(data.size).unwrap_or(i64::MAX);
            if first < 0 || last >= size {
                found.push((
                    Rule::Bounds,
                    format!(
                        "`{text}` reaches byte {} of {symbol}, which has {size} bytes",
                        if first < 0 { first } else { last }
                    ),
                    format!("keep the whole access inside {symbol}"),
                ));
            }
        }
    }
    found
}

fn register_name(register: Register) -> String {
    format!("{register:?}").to_ascii_lowercase()
}

fn displacement(instruction: &Instruction) -> i64 {
    i64::from_ne_bytes(instruction.memory_displacement64().to_ne_bytes())
}

/// How many bytes the memory operand covers, at least one.
fn width(instruction: &Instruction) -> i64 {
    i64::try_from(instruction.memory_size().size())
        .unwrap_or(1)
        .max(1)
}

fn writes_memory(instruction: &Instruction, factory: &mut InstructionInfoFactory) -> bool {
    factory
        .info(instruction)
        .used_memory()
        .iter()
        .any(|memory| {
            matches!(
                memory.access(),
                OpAccess::Write
                    | OpAccess::ReadWrite
                    | OpAccess::CondWrite
                    | OpAccess::ReadCondWrite
            )
        })
}

fn reads_memory(instruction: &Instruction, factory: &mut InstructionInfoFactory) -> bool {
    factory
        .info(instruction)
        .used_memory()
        .iter()
        .any(|memory| {
            matches!(
                memory.access(),
                OpAccess::Read | OpAccess::CondRead | OpAccess::ReadWrite | OpAccess::ReadCondWrite
            )
        })
}
