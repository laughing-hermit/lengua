//! Memory safety, the M rules (since dialect 3), which the checker settles statically wherever it
//! can:
//!
//! - **M1, owners.** Every address comes from a pointer with an owner: a data symbol, the current
//!   frame, a pointer the caller lent in a register, or the program's arguments. None is made
//!   from a plain number.
//! - **M2, bounds.** An access at a constant offset from a symbol's address stays inside it: the
//!   symbol main.s names, even where the next one starts just past its end. Indexed accesses
//!   keep rule 9's checks, which the checker holds to the owner's size.
//! - **M3, lifetimes.** An address in the current frame is never stored outside it or returned,
//!   and every access at a constant offset from rsp stays inside the function's own frame: never
//!   its return address or its caller's frame.
//! - **M4, exclusive writes.** An address a function writes through is lent to it once: no
//!   other register at the call holds the same address, as with Rust's `&mut`. Separate parts of
//!   one buffer, such as objects carved from an arena, may be lent together; the rules don't
//!   check that the parts stay apart. The program's arguments are only read. Which registers a
//!   function reads and writes through comes from its code and what it calls, system calls
//!   included, so no header has to say it.
//! - **M5, initialization.** No frame byte is read before every path has written it.
//! - **M6, immutability.** Read-only data is never written, through any register, and no memory
//!   is both writable and executable.
//! - **M8, stack size.** The deepest path stays within [`MAX_STACK_BYTES`].
//!
//! Threads (M7) have their own pass. What no rule settles, such as a pointer loaded from memory or
//! an index checked against a length in a register that the code just before doesn't set to a
//! constant, the proof settles for every input.

use std::collections::{BTreeMap, BTreeSet};

use super::cfg::{ASSERT_FAIL, Exit, Flow, constant_before};
use super::decode::{MappedFunction, Program};
use super::{Rule, Violation};
use crate::isa::{Abi, BaseMove, Control, Insn, Op, Operand, Reg, Stack, Value};
use crate::platform::Profile;
use crate::spec::Syscall;

/// The most stack a program may use on any path.
pub const MAX_STACK_BYTES: u64 = 1 << 20;

/// The bytes of a saved return address: the top of a frame whose function saved the link register.
const LINK_BYTES: i64 = 8;

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
    /// An address inside what the caller lent in this register.
    Argument {
        /// The register the caller lent it in.
        register: Reg,
        /// The offset from the address lent, when it's known.
        offset: Option<i64>,
    },
    /// The program's argument strings, or an array of them: what lengua's `_start` hands `main`.
    Arguments,
}

impl Owner {
    fn is_pointer(&self) -> bool {
        matches!(
            self,
            Self::Data { .. } | Self::Frame { .. } | Self::Argument { .. } | Self::Arguments
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
            (
                Self::Argument { register, .. },
                Self::Argument {
                    register: other, ..
                },
            ) if register == other => Self::Argument {
                register: *register,
                offset: None,
            },
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
            Self::Argument { register, offset } => Self::Argument {
                register: *register,
                offset: offset.zip(amount).map(|(offset, amount)| offset + amount),
            },
            other => other.clone(),
        }
    }
}

/// What a function, or a system call, does with the registers it's lent, as a caller sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Summary {
    /// The registers whose pointers it reads or writes through, itself or in what it calls.
    uses: BTreeSet<Reg>,
    /// The registers whose pointers it writes through.
    writes: BTreeSet<Reg>,
    /// What it returns in rax, with [`Owner::Argument`] for an address in what the caller lent.
    returns: Owner,
}

impl Summary {
    /// Reads and writes nothing it's lent: `assert_fail`, which only takes a number.
    fn none() -> Self {
        Self {
            uses: BTreeSet::new(),
            writes: BTreeSet::new(),
            returns: Owner::Unknown,
        }
    }

    /// May read and write through any of `registers`.
    fn every(registers: &[Reg]) -> Self {
        let all: BTreeSet<Reg> = registers.iter().copied().collect();
        Self {
            uses: all.clone(),
            writes: all,
            returns: Owner::Unknown,
        }
    }

    /// The buffers a system call reads and writes, by number: its arguments, counted from 0,
    /// in the registers `abi` passes them in.
    fn syscall(number: Option<i64>, profile: Profile, abi: &Abi) -> Self {
        let known = number.and_then(|number| {
            Syscall::ALL
                .into_iter()
                .find(|syscall| profile.syscall_number(*syscall) == number)
        });
        let (uses, writes): (&[usize], &[usize]) = match known {
            Some(Syscall::Read | Syscall::ClockGettime) => (&[1], &[1]),
            // The time to sleep, and what's left of it when a signal wakes it.
            Some(Syscall::ClockNanosleep) => (&[2, 3], &[3]),
            Some(Syscall::Getrandom) => (&[0], &[0]),
            // A file's details, and a folder's entries.
            Some(Syscall::Fstat | Syscall::Getdents64) => (&[1], &[1]),
            Some(Syscall::Mkdirat | Syscall::Unlinkat) => (&[1], &[]),
            // The old name and the new.
            Some(Syscall::Renameat) => (&[1, 3], &[]),
            Some(Syscall::Accept4) => (&[1, 2], &[1, 2]),
            Some(Syscall::Write | Syscall::Openat) => (&[1], &[]),
            Some(Syscall::Sendto) => (&[1, 4], &[]),
            // The table of descriptors, whose events the kernel writes back.
            Some(Syscall::Poll) => (&[0], &[0]),
            // The terminal's mode, read or written, or its window's size.
            Some(Syscall::Ioctl) => (&[2], &[2]),
            Some(
                Syscall::Close
                | Syscall::Lseek
                | Syscall::Exit
                | Syscall::ExitGroup
                | Syscall::Shutdown,
            ) => (&[], &[]),
            // clone, futex, or a call the rules refuse.
            None => return Self::every(abi.syscall_arguments),
        };
        let registers = |positions: &[usize]| -> BTreeSet<Reg> {
            positions
                .iter()
                .map(|position| abi.syscall_arguments[*position])
                .collect()
        };
        Self {
            uses: registers(uses),
            writes: registers(writes),
            returns: Owner::Number,
        }
    }

    /// The buffers a call to an operation of the edge lends: its `lends`, in the registers the
    /// edge's convention passes its arguments in.
    fn operation(op: crate::edge::Op, abi: &Abi) -> Self {
        let registers = |written: bool| -> BTreeSet<Reg> {
            op.lends()
                .iter()
                .filter(|lend| lend.written || !written)
                .map(|lend| abi.edge_arguments[lend.argument])
                .collect()
        };
        Self {
            uses: registers(false),
            writes: registers(true),
            returns: Owner::Number,
        }
    }
}

/// Each general-purpose register's owner before an instruction, by its slot, and which frame
/// bytes every path has written.
#[derive(Debug, Clone, PartialEq, Eq)]
struct State {
    registers: Vec<Owner>,
    /// The frame bytes written on every path, by their distance below the frame's top.
    written: BTreeSet<i64>,
    /// The stack pointer's slot, which keeps no owner: an address in the frame is its own.
    stack: Option<usize>,
}

impl State {
    /// Every register's owner unknown, for `abi`'s registers.
    fn unknown(abi: &Abi) -> Self {
        Self {
            registers: vec![Owner::Unknown; abi.registers.len()],
            written: BTreeSet::new(),
            stack: abi.stack_pointer.slot().map(|slot| slot as usize),
        }
    }

    /// The slot `register` keeps its owner in, when it's a general-purpose register.
    fn slot(&self, register: Reg) -> Option<usize> {
        register
            .slot()
            .map(|slot| slot as usize)
            .filter(|slot| *slot < self.registers.len())
    }

    fn get(&self, register: Reg) -> Owner {
        self.slot(register)
            .map_or(Owner::Unknown, |at| self.registers[at].clone())
    }

    fn set(&mut self, register: Reg, owner: Owner) {
        if let Some(at) = self.slot(register)
            && Some(at) != self.stack
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
            stack: self.stack,
        }
    }
}

/// Checks M1 to M6 in every function, each after the functions it calls, then M8 and M6's
/// segments for the program.
pub fn check(
    program: &Program<'_>,
    flows: &BTreeMap<String, Flow>,
    profile: Profile,
    max_stack: u64,
    writable_executable: bool,
    violations: &mut Vec<Violation>,
) {
    let abi = program.abi();
    let functions: BTreeMap<&str, &MappedFunction<'_>> = program
        .functions
        .iter()
        .map(|function| (function.name(), function))
        .collect();
    let mut summaries = BTreeMap::from([(ASSERT_FAIL.to_owned(), Summary::none())]);
    for name in callees_first(flows) {
        let (Some(function), Some(flow)) = (functions.get(name), flows.get(name)) else {
            continue;
        };
        if name == ASSERT_FAIL {
            continue;
        }
        let lent = lent(function, flow, &summaries, profile, abi);
        let states = states(function, flow, &lent, program);
        for (at, step) in function.steps.iter().enumerate() {
            let Some(state) = &states[at] else {
                continue;
            };
            let depth = flow.depth[at];
            let mut found = access(function, at, state, depth, program);
            if let Exit::Call { callee } = &flow.exits[at]
                && let Some(summary) = summaries.get(callee)
            {
                let frame = depth.map(|depth| depth - abi.return_address);
                found.extend(exclusive(
                    &step.source.text,
                    callee,
                    summary,
                    state,
                    frame,
                    abi,
                ));
            }
            for (rule, problem, fix) in found {
                violations.push(Violation {
                    rule,
                    line: Some(step.source.line),
                    problem,
                    fix,
                });
            }
        }
        let summary = summary(function, &states, &lent, abi);
        summaries.insert(name.to_owned(), summary);
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

/// The functions, each after every function it calls; recursion, which rule 1 refuses, is cut
/// where it closes.
fn callees_first(flows: &BTreeMap<String, Flow>) -> Vec<&str> {
    fn visit<'a>(
        name: &'a str,
        flows: &'a BTreeMap<String, Flow>,
        seen: &mut BTreeSet<&'a str>,
        order: &mut Vec<&'a str>,
    ) {
        if !seen.insert(name) {
            return;
        }
        if let Some(flow) = flows.get(name) {
            for exit in &flow.exits {
                if let Exit::Call { callee } = exit {
                    visit(callee, flows, seen, order);
                }
            }
        }
        order.push(name);
    }
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    for name in flows.keys() {
        visit(name, flows, &mut seen, &mut order);
    }
    order
}

/// What each call and system call in a function does with the registers it's lent.
fn lent(
    function: &MappedFunction<'_>,
    flow: &Flow,
    summaries: &BTreeMap<String, Summary>,
    profile: Profile,
    abi: &Abi,
) -> Vec<Option<Summary>> {
    function
        .steps
        .iter()
        .enumerate()
        .map(|(at, step)| match step.insn.control {
            Control::Call(_) | Control::IndirectCall => Some(match &flow.exits[at] {
                Exit::Call { callee } => summaries
                    .get(callee)
                    .cloned()
                    .unwrap_or_else(|| Summary::every(abi.arguments)),
                Exit::Edge { op } => Summary::operation(*op, abi),
                _ => Summary::every(abi.arguments),
            }),
            Control::Syscall => Some(Summary::syscall(
                constant_before(function, at, abi.syscall_number, abi).map(|(_, number)| number),
                profile,
                abi,
            )),
            _ => None,
        })
        .collect()
}

/// The state before each instruction, where it can run.
fn states(
    function: &MappedFunction<'_>,
    flow: &Flow,
    lent: &[Option<Summary>],
    program: &Program<'_>,
) -> Vec<Option<State>> {
    let count = function.steps.len();
    let mut states: Vec<Option<State>> = vec![None; count];
    if count == 0 {
        return states;
    }
    let abi = program.abi();
    let mut entry = State::unknown(abi);
    // lengua's `_start` calls `main` with the argument count and the two arrays of strings in its
    // first three argument registers.
    if function.name() == "main" {
        entry.set(abi.edge_arguments[0], Owner::Number);
        entry.set(abi.edge_arguments[1], Owner::Arguments);
        entry.set(abi.edge_arguments[2], Owner::Arguments);
    } else {
        for &register in abi.arguments {
            entry.set(
                register,
                Owner::Argument {
                    register,
                    offset: Some(0),
                },
            );
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
        let after = transfer(function, flow, at, &before, lent, program);
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
    lent: &[Option<Summary>],
    program: &Program<'_>,
) -> State {
    let insn = &function.steps[at].insn;
    let abi = program.abi();
    let mut after = before.clone();
    let depth = flow.depth[at];
    let base = abi.return_address;
    // Frame bytes this instruction writes: by push, at a constant offset from rsp, or through a
    // pointer to a known place in the frame.
    if let Some(depth) = depth {
        if matches!(insn.op, Op::Push(_)) {
            let bytes = match insn.stack {
                Stack::Moves(bytes) if bytes > 0 => bytes,
                _ => 8,
            };
            let top = depth + bytes - base;
            after.written.extend((top - bytes + 1)..=top);
        } else if insn.writes_memory && insn.memory.is_none_or(|memory| memory.index.is_none()) {
            let memory_base = insn.memory.and_then(|memory| memory.base);
            let first = match (memory_base, owner_of(before, memory_base)) {
                (Some(register), _) if register == abi.stack_pointer => {
                    Some(depth - base - displacement(insn))
                }
                (_, Owner::Frame { key: Some(key) }) => Some(key - displacement(insn)),
                _ => None,
            };
            if let Some(first) = first {
                after.written.extend((first - width(insn) + 1)..=first);
            }
        }
    }
    if let Some(summary) = &lent[at] {
        // A frame address lent for writing may be written there.
        let written = summary
            .writes
            .iter()
            .any(|register| matches!(before.get(*register), Owner::Frame { .. }));
        if written && let Some(depth) = depth {
            after.written.extend(1..=(depth - base));
        }
        if insn.control == Control::Syscall {
            for register in abi.syscall_clobbers {
                after.set(*register, Owner::Number);
            }
        } else {
            // A call to an operation of the edge keeps what its convention keeps, the registers
            // it was given its arguments in included.
            for register in insn.call_clobbers.unwrap_or(abi.caller_saved) {
                after.set(*register, Owner::Unknown);
            }
            after.set(abi.result, returned(&summary.returns, before));
        }
        return after;
    }
    let result = result(function, at, before, depth, program);
    // A base an access moves, as AArch64's writeback does, points into what it pointed into.
    let moved_base = insn.memory.and_then(|memory| {
        let base = memory.base?.full();
        let by = match memory.base_moves? {
            BaseMove::By(bytes) => Some(bytes),
            BaseMove::ByRegister => None,
        };
        Some((base, by))
    });
    for full in abi.registers {
        if !insn.writes.contains(*full) {
            continue;
        }
        let owner = match insn.dst {
            Some(register) if register.full() == *full => result.clone(),
            _ if insn.op == Op::Exchange => {
                other_operand(insn, *full).map_or(Owner::Unknown, |other| before.get(other))
            }
            _ => match moved_base {
                Some((base, by)) if base == *full => before.get(base).moved(by),
                _ => Owner::Number,
            },
        };
        after.set(*full, owner);
    }
    after
}

/// What a callee returns, at the call: what the caller lent, where it returns a lent pointer.
fn returned(returns: &Owner, before: &State) -> Owner {
    match returns {
        Owner::Argument { register, offset } => before.get(*register).moved(*offset),
        other => other.clone(),
    }
}

/// What a function does with what it's lent, from its states: the pointers it reads and writes
/// through, itself and in its calls, and what it returns.
fn summary(
    function: &MappedFunction<'_>,
    states: &[Option<State>],
    lent: &[Option<Summary>],
    abi: &Abi,
) -> Summary {
    let mut summary = Summary::none();
    let mut returns: Option<Owner> = None;
    for (at, step) in function.steps.iter().enumerate() {
        let Some(state) = &states[at] else {
            continue;
        };
        let insn = &step.insn;
        let mut lend = |register: Reg, writes: bool| {
            if let Owner::Argument { register: from, .. } = state.get(register) {
                summary.uses.insert(from);
                if writes {
                    summary.writes.insert(from);
                }
            }
        };
        if let Some(callee) = &lent[at] {
            for register in &callee.uses {
                lend(*register, callee.writes.contains(register));
            }
        } else if insn.control == Control::Return {
            // A frame address returned is M3's to report; the caller can't use it.
            let owner = match state.get(abi.result) {
                Owner::Frame { .. } => Owner::Unknown,
                owner => owner,
            };
            returns = Some(returns.map_or_else(|| owner.clone(), |known| known.join(&owner)));
        } else if let Some(memory) = insn.memory
            && accesses_memory(insn)
            && memory.base != Some(abi.stack_pointer)
            && !memory.pc_relative
        {
            let through = if owner_of(state, memory.base).is_pointer() {
                memory.base
            } else {
                memory.index
            };
            if let Some(through) = through {
                lend(through, insn.writes_memory);
            }
        }
    }
    summary.returns = returns.unwrap_or(Owner::Unknown);
    summary
}

/// M4: at a call, an address the callee writes through is in no other register it uses. `frame`
/// is the size of the caller's frame there, when it's known.
fn exclusive(
    text: &str,
    callee: &str,
    summary: &Summary,
    before: &State,
    frame: Option<i64>,
    abi: &Abi,
) -> Option<(Rule, String, String)> {
    for written in &summary.writes {
        for other in summary.uses.iter().filter(|other| *other != written) {
            let owner = before.get(*written);
            if owner != before.get(*other) {
                continue;
            }
            let Some(address) = address_name(&owner, frame, abi) else {
                continue;
            };
            // In the order the calling convention lends them.
            let position =
                |register: &Reg| abi.arguments.iter().position(|known| known == register);
            let (first, second) = if position(written) <= position(other) {
                (written, other)
            } else {
                (other, written)
            };
            return Some((
                Rule::ExclusiveWrites,
                format!(
                    "`{text}` lends one address, {address}, in both {first} and {second}, and \
                     {callee} writes through {written}, so its writes change what it reads"
                ),
                "lend an address that's written in one register only: separate buffers or \
                 separate parts of one, or one pointer the function both reads and writes through"
                    .into(),
            ));
        }
    }
    None
}

/// The address an owner names, when it's one known address: `frame` is the size of the frame,
/// when it's known.
fn address_name(owner: &Owner, frame: Option<i64>, abi: &Abi) -> Option<String> {
    let plus = |offset: i64| {
        if offset == 0 {
            String::new()
        } else {
            format!(" + {offset}")
        }
    };
    match owner {
        Owner::Data {
            symbol,
            offset: Some(offset),
        } => Some(format!("{symbol}{}", plus(*offset))),
        Owner::Frame { key: Some(key) } => {
            Some(format!("{}{}", abi.stack_pointer, plus(frame? - key)))
        }
        Owner::Argument {
            register,
            offset: Some(offset),
        } => Some(format!(
            "the pointer it was lent in {register}{}",
            plus(*offset)
        )),
        _ => None,
    }
}

/// Whether an instruction reads or writes memory through an operand: not `lea`, which only
/// computes an address, nor hints.
fn accesses_memory(insn: &Insn) -> bool {
    insn.memory.is_some() && !matches!(insn.op, Op::AddressOf { .. } | Op::Hint)
}

/// A register's owner, where no register is a plain number (an address without a base).
fn owner_of(state: &State, register: Option<Reg>) -> Owner {
    register.map_or(Owner::Number, |register| state.get(register))
}

/// The other register of an `xchg`.
fn other_operand(insn: &Insn, register: Reg) -> Option<Reg> {
    insn.operands.iter().find_map(|operand| match operand {
        Operand::Reg(found) if found.full() != register => Some(*found),
        _ => None,
    })
}

/// The owner of what the instruction at `at` writes to its first operand.
fn result(
    function: &MappedFunction<'_>,
    at: usize,
    before: &State,
    depth: Option<i64>,
    program: &Program<'_>,
) -> Owner {
    let insn = &function.steps[at].insn;
    let abi = program.abi();
    let Some(destination) = insn.dst else {
        return Owner::Unknown;
    };
    let old = before.get(destination);
    // A 32-bit write zeroes the upper half: a truncated address is no address.
    match destination.bits() {
        64 => {}
        32 => return Owner::Number,
        _ => {
            return if old == Owner::Number {
                Owner::Number
            } else {
                Owner::Unknown
            };
        }
    }
    let source = |operand: Operand| -> Owner {
        match operand {
            // The stack pointer itself is the address of the frame's lowest byte.
            Operand::Reg(register) if register == abi.stack_pointer => Owner::Frame {
                key: depth.map(|depth| depth - abi.return_address),
            },
            Operand::Reg(register) => before.get(register),
            Operand::Mem => Owner::Unknown,
            Operand::Imm(_) | Operand::Target(_) => Owner::Number,
        }
    };
    let number = |operand: Operand| match operand {
        Operand::Imm(value) => Some(value),
        _ => None,
    };
    let same = |a: Operand, b: Operand| matches!((a, b), (Operand::Reg(a), Operand::Reg(b)) if a.full() == b.full());
    match insn.value {
        Value::Address => address_owner(function, at, before, depth, program),
        Value::Copy(operand) => source(operand),
        Value::Number => Owner::Number,
        Value::Load => load_owner(function, at, before),
        Value::Add(base, by) => {
            let (from, other) = (source(base), source(by));
            match (from.is_pointer(), other.is_pointer()) {
                (true, false) if other == Owner::Number => from.moved(number(by)),
                (false, true) if from == Owner::Number => other,
                (false, false) if from == Owner::Number && other == Owner::Number => Owner::Number,
                _ => Owner::Unknown,
            }
        }
        Value::Sub(base, by) => {
            let (from, other) = (source(base), source(by));
            match (from.is_pointer(), other.is_pointer()) {
                (true, false) if other == Owner::Number => {
                    from.moved(number(by).map(|amount| -amount))
                }
                // The distance between two addresses is a length.
                (true, true) => Owner::Number,
                (false, false) if from == Owner::Number && other == Owner::Number => Owner::Number,
                // A register less itself is zero.
                _ if same(base, by) => Owner::Number,
                _ => Owner::Unknown,
            }
        }
        Value::And(base, mask) if source(base).is_pointer() && number(mask).is_some() => {
            source(base).moved(None)
        }
        Value::Pop => Owner::Unknown,
        Value::Select(operand) => old.join(&source(operand)),
        Value::And(..) | Value::Other => {
            let registers = || {
                insn.operands.iter().filter_map(|operand| match operand {
                    Operand::Reg(register) => Some(before.get(*register)),
                    _ => None,
                })
            };
            let reads_pointer = registers().any(|owner| owner.is_pointer()) || old.is_pointer();
            let reads_unknown = registers().any(|owner| owner == Owner::Unknown);
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
    let insn = &function.steps[at].insn;
    let Some(memory) = insn.memory else {
        return Owner::Unknown;
    };
    if memory.pc_relative {
        return program
            .named(&function.steps[at])
            .map_or(Owner::Unknown, |(symbol, _, offset)| Owner::Data {
                symbol: symbol.to_owned(),
                offset: Some(offset),
            });
    }
    let displacement = memory.displacement;
    if memory.base == Some(program.abi().stack_pointer) {
        let key = memory
            .index
            .is_none()
            .then(|| depth.map(|depth| depth - program.abi().return_address - displacement))
            .flatten();
        return Owner::Frame { key };
    }
    let base_owner = owner_of(before, memory.base);
    let index_owner = owner_of(before, memory.index);
    match (base_owner.is_pointer(), index_owner.is_pointer()) {
        (true, false) if index_owner == Owner::Number => {
            base_owner.moved(memory.index.is_none().then_some(displacement))
        }
        (false, true) if base_owner == Owner::Number => index_owner.moved(None),
        (false, false) if base_owner == Owner::Number && index_owner == Owner::Number => {
            Owner::Number
        }
        _ => Owner::Unknown,
    }
}

/// The owner of a 64-bit value an instruction loads: an entry of the array of strings, or
/// otherwise not followed.
fn load_owner(function: &MappedFunction<'_>, at: usize, before: &State) -> Owner {
    let insn = &function.steps[at].insn;
    let (Some(destination), Some(memory)) = (insn.dst, insn.memory) else {
        return Owner::Unknown;
    };
    if destination.bits() != 64 {
        return Owner::Number;
    }
    if owner_of(before, memory.base) == Owner::Arguments {
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
) -> Vec<(Rule, String, String)> {
    let step = &function.steps[at];
    let insn = &step.insn;
    let abi = program.abi();
    let text = &step.source.text;
    let mut found = Vec::new();
    // M3: an address in this frame never leaves it, stored or returned.
    if insn.control == Control::Return {
        for register in abi.returns {
            if matches!(before.get(register), Owner::Frame { .. }) {
                found.push((
                    Rule::Lifetimes,
                    format!(
                        "{} returns an address in its own frame in {register}, which ends as it \
                         returns",
                        function.name(),
                    ),
                    "return a value, or let the caller lend the buffer to write into".into(),
                ));
            }
        }
        return found;
    }
    let Some(memory) = insn.memory.filter(|_| accesses_memory(insn)) else {
        return found;
    };
    if matches!(insn.op, Op::Push(_) | Op::Pop(_)) {
        return found;
    }
    let writes = insn.writes_memory;
    let (base, index) = (memory.base, memory.index);
    let unchecked = step.source.unsafe_block.is_some();
    // M2: a bit test in memory with a register offset reaches as far as the offset goes.
    if matches!(insn.mnemonic.as_str(), "bt" | "bts" | "btr" | "btc")
        && matches!(insn.operands.as_slice(), [Operand::Mem, Operand::Reg(_)])
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
    if writes && base != Some(abi.stack_pointer) {
        let stored = insn.operands.iter().find_map(|operand| match operand {
            Operand::Reg(register) if matches!(before.get(*register), Owner::Frame { .. }) => {
                Some(*register)
            }
            _ => None,
        });
        let into_frame = matches!(owner_of(before, base), Owner::Frame { .. });
        if let Some(register) = stored
            && !into_frame
        {
            found.push((
                Rule::Lifetimes,
                format!(
                    "`{text}` stores {register}, an address in this function's frame, outside \
                     the frame"
                ),
                "lend a frame address to a callee in a register only; never store it where it \
                 could outlive the function"
                    .into(),
            ));
        }
    }
    // Rule 6 holds [rip + name] to its symbol.
    if memory.pc_relative {
        return found;
    }
    // M3: constant offsets from rsp, or from a pointer to a known place in the frame, stay
    // inside the function's own frame. `first` is how far below the frame's top the access's
    // first byte is.
    let first = match (index, base, owner_of(before, base)) {
        (None, Some(register), _) if register == abi.stack_pointer => {
            Some(depth.map(|depth| depth - abi.return_address - memory.displacement))
        }
        (None, _, Owner::Frame { key: Some(key) }) => {
            Some(depth.map(|_| key - memory.displacement))
        }
        _ => None,
    };
    if let Some(first) = first {
        if let (Some(depth), Some(first)) = (depth, first) {
            let width = width(insn);
            let own = depth - abi.return_address;
            let inside = first <= own && first - width + 1 >= 1;
            // Where the CPU keeps the return address in a register, the function saves it with
            // its first instruction, as the top of its frame: ret returns through those bytes,
            // as x86-64's does through the slot above the frame.
            let saves_link = abi.link.is_some_and(|link| {
                matches!(
                    function.steps.first().map(|opening| opening.insn.op),
                    Some(Op::Push(registers)) if registers.contains(&Some(link))
                )
            });
            if inside && saves_link && first - width < LINK_BYTES {
                found.push((
                    Rule::Lifetimes,
                    format!(
                        "`{text}` reaches the return address this function saved in its frame, \
                         which ret returns through"
                    ),
                    "keep every access at [sp + d] to the frame's own data, past the frame \
                     record's saved return address"
                        .into(),
                ));
            } else if !inside {
                found.push((
                    Rule::Lifetimes,
                    format!(
                        "`{text}` reaches past this function's frame of {own} bytes, into its \
                         return address or its caller's frame"
                    ),
                    "keep every access at [rsp + d] inside the frame the function made with push \
                     and sub rsp"
                        .into(),
                ));
            } else if inside && !matches!(insn.op, Op::Pop(_)) && insn.reads_memory {
                // M5: what's read was written first, on every path.
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
    if base.is_none() && index.is_none() {
        found.push((
            Rule::Owners,
            format!("`{text}` uses a fixed address, which no pointer owns"),
            "address data symbols as [rip + name], and buffers through a pointer to them".into(),
        ));
        return found;
    }
    let base_owner = owner_of(before, base);
    let index_owner = owner_of(before, index);
    // M1: an address comes from a pointer, never a plain number.
    if base_owner == Owner::Number && !index_owner.is_pointer() {
        found.push((
            Rule::Owners,
            format!(
                "`{text}` uses {} as an address, but it holds a plain number here, not a pointer \
                 into a buffer",
                base.or(index)
                    .map_or_else(String::new, |register| register.to_string())
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
    // M4: the program's arguments are lent to it for reading.
    if writes && owner == Owner::Arguments {
        found.push((
            Rule::ExclusiveWrites,
            format!("`{text}` writes into the program's arguments, which it only reads"),
            "copy the argument into a buffer in .bss and change the copy".into(),
        ));
    }
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
            && index.is_none()
            && !unchecked
        {
            let first = offset + memory.displacement;
            let last = first + width(insn) - 1;
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

/// The constant the memory operand adds, if it has one.
fn displacement(insn: &Insn) -> i64 {
    insn.memory.map_or(0, |memory| memory.displacement)
}

/// How many bytes the memory operand covers, at least one.
fn width(insn: &Insn) -> i64 {
    insn.memory
        .map_or(1, |memory| i64::try_from(memory.width).unwrap_or(1))
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge::Op as EdgeOp;

    /// An operation lends what the system call it makes does, except where the edge is stricter by
    /// design: it never has the kernel write what is left of a sleep or the address of a peer, and
    /// its terminal operation keeps the mode it reads and writes in its own memory.
    #[test]
    fn an_operation_lends_what_its_system_call_does() {
        for profile in Profile::ALL {
            let abi = profile.isa().abi();
            for op in EdgeOp::ALL {
                let [call] = op.syscalls() else { continue };
                if matches!(op, EdgeOp::Sleep | EdgeOp::Accept | EdgeOp::Terminal) {
                    continue;
                }
                let summary = Summary::syscall(Some(profile.syscall_number(*call)), profile, abi);
                let what = format!("{} on {profile:?}", op.symbol());
                assert_eq!(summary.uses.len(), op.lends().len(), "{what}");
                let written = op.lends().iter().filter(|lend| lend.written).count();
                assert_eq!(summary.writes.len(), written, "{what}");
            }
        }
    }
}
