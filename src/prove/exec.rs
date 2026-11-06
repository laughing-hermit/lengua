//! Runs a program's machine code on terms instead of numbers: each register, flag and byte holds
//! a term over the inputs. A branch the inputs decide splits the run in two, each side keeping
//! its condition, so the finished runs cover every input of a shape. With concrete inputs every
//! term is a number, and the same code is a plain emulator, which the prover checks against real
//! runs.
//!
//! Every run is held to the memory rules where the checker can't settle them, as a borrow
//! checker would at run time: each access stays inside the object its pointer was made from (a
//! data symbol, a live frame, the program's arguments), even where the CPU would let it through
//! into a neighbour that starts just past its end; no return address is written, and each `ret`
//! goes back where its call came from; and no stack byte is read before something wrote it.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use iced_x86::{ConditionCode, Instruction, Mnemonic, OpKind, Register};

use super::term::{Pool, Term};
use crate::checker::decode::Machine;
use crate::isa::{Isa, aarch64 as a64};
use calls::{Call, Convention};

/// The address of `argc` when `_start` begins.
pub const STACK_TOP: u64 = 0x7ffd_0000_0000;

/// How far below that the stack may grow.
const STACK_SIZE: u64 = 8 * 1024 * 1024;

/// Room above the stack top for the argument strings and pointers.
const ARGUMENT_ROOM: u64 = 1024 * 1024;

/// The most instructions one run may take before the prover gives up on it.
pub const MAX_STEPS: u64 = 5_000_000;

/// How many steps a run takes between asking whether the proof still has its time.
const STEPS_BETWEEN_TIME_CHECKS: u64 = 4096;

/// Why a run past [`MAX_STEPS`] proves nothing.
pub const TOO_LONG: &str = "a run took too many instructions to follow";

/// The most bytes of standard input left for which a proof takes every count a read can
/// return: every way 8 bytes can be cut is 128 runs.
const RAW_CUT_BYTES: u64 = 8;

/// The most places an address that depends on the input may be, such as a table's entries for
/// each value of an input byte.
const MAX_PLACES: u128 = 256;

/// The bytes of a vector register: a zmm register's 512 bits.
const VECTOR_BYTES: usize = 64;

/// The general registers' slots, as many as AArch64's x0 to x30 and sp; x86-64 uses the first
/// sixteen, rax to r15.
const SLOTS: usize = 32;

/// A range of addresses the program may use.
#[derive(Debug, Clone, Copy)]
struct Region {
    start: u64,
    end: u64,
    writable: bool,
}

/// A loop whose exits on a condition the input's length decides are merged: the run goes on
/// round it as if it never left, and each run that left waits at the exit, to be merged with
/// the rest once the loop is done. Exact: for each input, the merged run is the one that left
/// first.
#[derive(Debug, Clone)]
struct Merging {
    /// The loop's head.
    head: u64,
    /// Its `jnz` back up.
    latch: u64,
    /// Where the runs that left went.
    target: u64,
    /// Whether the run is still in the loop: no exit taken yet.
    alive: Term,
    /// The last run that left, which links to the ones before it: a fork of the run that
    /// stays shares them.
    parked: Option<Rc<Parked>>,
}

/// A bounded loop, as the dialect writes every one: its time round runs from its head down to a
/// `dec` of its bound counter and a `jnz` back up to the head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loop {
    /// Where each time round starts.
    pub head: u64,
    /// The `jnz` that goes round again.
    pub latch: u64,
    /// Where the loop goes on once its counter runs out: the instruction after the `jnz`.
    pub exit: u64,
    /// The bound counter's register.
    pub counter: usize,
}

/// The bounded loops in `code`, the instructions of one function from `start` to `end`: each
/// `jnz` back up into the function right after a `dec` of a whole register.
fn loops_in(code: &[Instruction], start: u64, end: u64) -> Vec<Loop> {
    code.windows(2)
        .filter_map(|pair| {
            let [counted, back] = pair else {
                return None;
            };
            let target = back.near_branch_target();
            let whole = counted.op0_kind() == OpKind::Register
                && counted.op0_register().full_register() == counted.op0_register();
            (back.mnemonic() == Mnemonic::Jne
                && counted.mnemonic() == Mnemonic::Dec
                && whole
                && target < back.ip()
                && (start..end).contains(&target))
            .then(|| {
                Some(Loop {
                    head: target,
                    latch: back.ip(),
                    exit: back.next_ip(),
                    counter: slot(counted.op0_register())?,
                })
            })
            .flatten()
        })
        .collect()
}

/// The bounded loops in AArch64 code, the words of one function from `start` to `end`: each
/// `b.ne` back up into the function right after a `subs` of one from a whole register, or each
/// `cbnz` of a whole register back up right after a `sub` of one from it, as the dialect ends
/// every loop.
fn loops_in_a64(words: &[crate::checker::decode::Word], start: u64, end: u64) -> Vec<Loop> {
    use crate::isa::aarch64::{A64Cond, Gpr};
    words
        .windows(2)
        .filter_map(|pair| {
            let [counted, back] = pair else {
                return None;
            };
            let (Ok(counted_inst), Ok(back_inst)) = (&counted.inst, &back.inst) else {
                return None;
            };
            let a64::Inst::AddImm {
                sub: true,
                flags,
                rd,
                rn,
                imm: 1,
                shift12: false,
            } = *counted_inst
            else {
                return None;
            };
            let Gpr::R(number) = rd.gpr else {
                return None;
            };
            if rd != rn || !rd.wide {
                return None;
            }
            let target = match *back_inst {
                a64::Inst::BranchCond {
                    cond: A64Cond::Ne,
                    target,
                } if flags => target,
                a64::Inst::CompareBranch {
                    nonzero: true,
                    rt,
                    target,
                } if !flags && rt == rd => target,
                _ => return None,
            };
            (target < back.address && (start..end).contains(&target)).then_some(Loop {
                head: target,
                latch: back.address,
                exit: back.address + 4,
                counter: usize::from(number),
            })
        })
        .collect()
}

/// The program as loaded: its instructions by address, and its memory.
pub struct Image {
    instructions: HashMap<u64, Instruction>,
    /// AArch64's instructions, by address, in the dialect's canonical form.
    words: HashMap<u64, a64::Inst>,
    regions: Vec<Region>,
    initial: HashMap<u64, u8>,
    entry: u64,
    /// Each function's name, by the address range of its code.
    functions: Vec<(u64, u64, String)>,
    /// Each data symbol's name, by its address range, in address order.
    symbols: Vec<(u64, u64, String)>,
    /// The symbol each `[rip + name]` names in main.s, by instruction address, as a place in
    /// `symbols`.
    named: HashMap<u64, usize>,
    /// Each checked jump table's `jmp`, by address, and the addresses of its cases.
    tables: HashMap<u64, Vec<u64>>,
    /// The canonical fill's places, when the program has it.
    fill: Option<fill::Fill>,
    /// Every bounded loop, by its head.
    loops: BTreeMap<u64, Loop>,
    /// Whether some run has called each function, as `functions` lists them: a proof says
    /// nothing of code no run reached.
    entered: Vec<AtomicBool>,
    /// The CPU whose code it is.
    isa: Isa,
    /// The length of SVE's vectors on the CPU it runs on, in bytes; 0 for none.
    sve_bytes: u16,
    /// lengua's operations, by the address of each one's entry: a call to one is modelled
    /// rather than run, in the edge form.
    ops: HashMap<u64, crate::edge::Op>,
    /// Whether the program is in the edge form: it has `main`, which lengua's `_start` calls.
    edge: bool,
}

impl Image {
    /// The details of Linux's ABI on its CPU.
    fn linux(&self) -> &'static crate::syscalls::LinuxAbi {
        match self.isa {
            Isa::X86_64 => &crate::syscalls::x86_64::ABI,
            Isa::Aarch64 => &crate::syscalls::aarch64::ABI,
        }
    }

    /// Loads the functions and data the decoder read from the executable.
    #[must_use]
    pub fn new(machine: &Machine) -> Option<Self> {
        let edge = machine.functions.contains_key("main");
        let entry = machine
            .functions
            .get(if edge { "main" } else { "_start" })?
            .address;
        let ops = if edge {
            machine
                .functions
                .iter()
                .filter_map(|(name, function)| {
                    crate::edge::Op::from_symbol(name)
                        // The thread operations run as the code they are: the clone they make
                        // and the thread's end are the model's own, as for any thread.
                        .filter(|op| {
                            !matches!(
                                op,
                                crate::edge::Op::ThreadStart | crate::edge::Op::ThreadJoin
                            )
                        })
                        .map(|op| (function.address, op))
                })
                .collect()
        } else {
            HashMap::new()
        };
        let mut instructions = HashMap::new();
        let mut words = HashMap::new();
        let mut functions = Vec::new();
        let mut loops = BTreeMap::new();
        for (name, function) in &machine.functions {
            for instruction in function.x86() {
                instructions.insert(instruction.ip(), *instruction);
            }
            for found in loops_in(
                function.x86(),
                function.address,
                function.address + function.size,
            ) {
                loops.insert(found.head, found);
            }
            for found in loops_in_a64(
                function.words(),
                function.address,
                function.address + function.size,
            ) {
                loops.insert(found.head, found);
            }
            for word in function.words() {
                if let Ok(inst) = word.inst {
                    words.insert(word.address, inst);
                }
            }
            functions.push((
                function.address,
                function.address + function.size,
                name.clone(),
            ));
        }
        let mut regions = vec![Region {
            start: STACK_TOP - STACK_SIZE,
            end: STACK_TOP + ARGUMENT_ROOM,
            writable: true,
        }];
        let mut initial = HashMap::new();
        let mut symbols = Vec::new();
        for (name, data) in &machine.data {
            if data.size > 0 {
                symbols.push((data.address, data.address + data.size, name.clone()));
            }
            regions.push(Region {
                start: data.address,
                end: data.address + data.size,
                writable: data.writable,
            });
            for (offset, byte) in data.bytes.iter().enumerate() {
                initial.insert(data.address + offset as u64, *byte);
            }
        }
        symbols.sort_unstable();
        let named = machine
            .named
            .iter()
            .filter_map(|(ip, name)| {
                let place = symbols.iter().position(|(_, _, known)| known == name)?;
                Some((*ip, place))
            })
            .collect();
        let entered = functions.iter().map(|_| AtomicBool::new(false)).collect();
        let mut image = Self {
            instructions,
            words,
            regions,
            initial,
            entry,
            functions,
            symbols,
            named,
            tables: machine
                .tables
                .iter()
                .map(|(jump, cases)| {
                    let mut cases = cases.clone();
                    // A table may name a case more than once; a run goes there once.
                    cases.sort_unstable();
                    cases.dedup();
                    (*jump, cases)
                })
                .collect(),
            fill: None,
            loops,
            entered,
            isa: machine.isa,
            sve_bytes: machine.sve_bytes,
            ops,
            edge,
        };
        image.fill = fill::Fill::find(machine);
        Some(image)
    }

    /// The bounded loop whose head is `head`.
    #[must_use]
    pub fn loop_at(&self, head: u64) -> Option<&Loop> {
        self.loops.get(&head)
    }

    /// Every bounded loop, by its head, fill's own read loop among them.
    pub fn loops(&self) -> impl Iterator<Item = &Loop> {
        self.loops.values()
    }

    /// Whether the program has the canonical fill, which proofs read as a whole for every way
    /// its input can be cut.
    #[must_use]
    pub fn has_fill(&self) -> bool {
        self.fill.is_some()
    }

    /// Where fill's read loop starts, when the program has the canonical fill: the head a
    /// proof by induction goes round.
    #[must_use]
    pub fn fill_head(&self) -> Option<u64> {
        self.fill.map(fill::Fill::head)
    }

    /// The owner of the address `[rip + name]` computes at `ip`: the symbol main.s names there,
    /// or, without main.s, the symbol the address is in or just past the end of, where only one
    /// can be meant.
    fn symbol_owner(&self, ip: u64, address: u64) -> Option<Owner> {
        if let Some(place) = self.named.get(&ip) {
            return Some(Owner::Symbol(*place));
        }
        let holding = self
            .symbols
            .iter()
            .position(|(start, end, _)| (*start..*end).contains(&address));
        let ending = self.symbols.iter().position(|(_, end, _)| *end == address);
        match (holding, ending) {
            (Some(place), None) | (None, Some(place)) => Some(Owner::Symbol(place)),
            _ => None,
        }
    }

    /// The operation of lengua's whose entry is at `address`, in the edge form.
    fn op_at(&self, address: u64) -> Option<crate::edge::Op> {
        self.ops.get(&address).copied()
    }

    /// Notes that a run called the function at `address`.
    fn enter(&self, address: u64) {
        if let Some(index) = self
            .functions
            .iter()
            .position(|(start, _, _)| *start == address)
        {
            self.entered[index].store(true, Ordering::Relaxed);
        }
    }

    /// The functions no run has called, in the order of their code, but `_start`, where every
    /// run begins, and `assert_fail`, which only a failed assertion calls.
    #[must_use]
    pub fn unreached(&self) -> Vec<String> {
        let mut unreached: Vec<(u64, &str)> = self
            .functions
            .iter()
            .zip(&self.entered)
            .filter(|((_, _, name), entered)| {
                !entered.load(Ordering::Relaxed)
                    && !matches!(name.as_str(), "_start" | "assert_fail")
                    // In the edge form `main` is where every run begins, and lengua's own
                    // functions are lengua's to answer for.
                    && !(self.edge && (name == "main" || crate::edge::reserved(name)))
            })
            .map(|((start, _, name), _)| (*start, name.as_str()))
            .collect();
        unreached.sort_unstable();
        unreached
            .into_iter()
            .map(|(_, name)| name.to_owned())
            .collect()
    }

    /// The name of the function whose code holds `address`.
    fn function_at(&self, address: u64) -> &str {
        self.functions
            .iter()
            .find(|(start, end, _)| (*start..*end).contains(&address))
            .map_or("its code", |(_, _, name)| name.as_str())
    }

    fn region(&self, address: u64) -> Option<Region> {
        self.regions
            .iter()
            .copied()
            .find(|region| (region.start..region.end).contains(&address))
    }
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// `exit` or `exit_group`, with the status's low byte.
    Exit(Term),
    /// The CPU or kernel would stop the program with this signal.
    Signal(&'static str),
    /// Something the prover doesn't model, so this run proves nothing.
    Unsupported(String),
    /// The paths through the code ran past the limit before this one was followed.
    TooManyPaths,
    /// Waiting for the test client's next connection after the last one, as a server that
    /// keeps serving does.
    Serving,
    /// Waiting for something the test's peer never sends, which a real run gives up on: why.
    Stuck(String),
    /// An access the memory rules forbid, which the CPU might let through: what it did.
    Unsafe(String),
    /// Back at the read loop's head, where a proof by induction compares the run with where it
    /// started.
    Looped,
}

/// What last set the flags, when a branch can read it as a plain comparison.
#[derive(Debug, Clone, Copy)]
enum Last {
    /// `cmp a, b` or `sub a, b`.
    Compare(Term, Term),
    /// `test`, `and`, `or` or `xor`, with this result.
    Logic(Term),
    /// Anything else: read the flags themselves.
    Other,
}

/// Bytes of memory by address, in pages that the runs a fork makes share until one of them
/// writes to a page: a fork copies the page table, never the bytes.
#[derive(Debug, Clone, Default)]
pub struct Memory {
    pages: BTreeMap<u64, Rc<[Option<Term>; PAGE]>>,
}

/// The bytes in one page of [`Memory`].
const PAGE: usize = 256;

impl Memory {
    /// The byte at `address`, if anything was stored there.
    #[must_use]
    pub fn get(&self, address: u64) -> Option<Term> {
        let page = self.pages.get(&(address / PAGE as u64))?;
        page[(address % PAGE as u64) as usize]
    }

    /// Stores `byte` at `address`.
    pub fn insert(&mut self, address: u64, byte: Term) {
        let page = self
            .pages
            .entry(address / PAGE as u64)
            .or_insert_with(|| Rc::new([None; PAGE]));
        Rc::make_mut(page)[(address % PAGE as u64) as usize] = Some(byte);
    }

    /// Forgets the bytes from `start` up to `end`, as if never written.
    fn forget(&mut self, start: u64, end: u64) {
        for address in start..end {
            if let Some(page) = self.pages.get_mut(&(address / PAGE as u64))
                && page[(address % PAGE as u64) as usize].is_some()
            {
                Rc::make_mut(page)[(address % PAGE as u64) as usize] = None;
            }
        }
    }
}

/// Memory an access through a pointer is held to: a data symbol, a live frame, or one of the
/// program's arguments.
#[derive(Debug, Clone)]
struct Object {
    start: u64,
    end: u64,
    name: String,
}

/// What a pointer was made from, as the run followed it: an access through it is held to that
/// object alone, even where another starts just past its end, as symbols, strings and frames
/// often do, so a pointer one step too far is caught rather than taken for the next object's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    /// A data symbol, by its place in the image's symbols.
    Symbol(usize),
    /// A call's frame, by the call's number: 0 for `_start`'s, then one more for each call.
    Frame(u64),
    /// The array of the program's arguments, or one of its strings, by its place in the run's
    /// arguments.
    Argument(usize),
}

/// What an access is held to: the owner of the pointer it was made from, where the run followed
/// it, or else the object a pointer's value points into.
#[derive(Debug, Clone, Copy)]
enum Held {
    Owner(Owner),
    Pointer(u64),
}

/// What an access whose place the input chooses goes through, to hold it to its object: x86-64's
/// memory operand, asked when it's needed, or what an AArch64 access's base register holds.
#[derive(Debug, Clone, Copy)]
enum Through {
    /// The running x86-64 instruction's memory operand.
    X86,
    /// An AArch64 access's base, as [`Cpu::held_a64`] found it.
    Held(Option<Held>),
}

/// How an instruction passes owners on: the registers it sets, each with its value's owner, at
/// most two as `xchg` sets, and the pointer it stores whole, if any. Every other register it
/// changes holds no pointer the run follows.
#[derive(Debug, Default)]
struct Plan {
    registers: [Option<(usize, Option<Owner>)>; 3],
    stores: [Option<(u64, Term, Owner)>; 2],
}

impl Plan {
    /// Sets `register`'s owner, for a general register other than rsp, whose owner is always
    /// the running call's frame.
    fn set(&mut self, register: Register, owner: Option<Owner>) {
        if register.size() == 8
            && let Some(index) = slot(register)
            && index != 4
            && let Some(free) = self.registers.iter_mut().find(|set| set.is_none())
        {
            *free = Some((index, owner));
        }
    }

    /// Brings `state`'s owners up to date, after the instruction left it, from `before`, its
    /// registers before.
    fn apply(&self, state: &mut State, before: &[Term; SLOTS]) {
        for (owner, (now, then)) in state
            .owners
            .iter_mut()
            .zip(state.registers.iter().zip(before))
        {
            if now != then {
                *owner = None;
            }
        }
        for (index, owner) in self.registers.iter().flatten() {
            state.owners[*index] = *owner;
        }
        for (address, value, owner) in self.stores.iter().flatten() {
            if state.stored.get(address) == Some(value) {
                state.pointers.insert(*address, *owner);
            }
        }
    }

    /// The same for every state `step` leaves.
    fn apply_all(&self, state: &mut State, step: &mut Step, before: &[Term; SLOTS]) {
        self.apply(state, before);
        match step {
            Step::Next => {}
            Step::Split(_, other) => self.apply(other, before),
            Step::Choose(choices) => {
                for (_, choice) in choices {
                    self.apply(choice, before);
                }
            }
        }
    }
}

/// A name a call is given: known, or chosen by the input, byte by byte.
enum Name {
    Known(String),
    Chosen(Vec<Term>),
}

/// A call that hasn't returned: where its callee's frame starts, where its return address is
/// kept, where it returns to, the function called, and the call's number, which names the
/// callee's frame. x86-64's `call` pushes the return address, so it's on the stack from the start
/// and the callee's frame lies below it; AArch64's `bl` keeps it in x30, until the callee saves x30
/// in its frame record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Return {
    /// The top of the callee's frame: the stack pointer once the call was made.
    top: u64,
    /// The bytes the call itself put on the stack above the frame: 8 on x86-64, for the return
    /// address, and none on AArch64.
    pushed: u64,
    /// Where the return address is kept in memory, while it is: no store may change it.
    saved: Option<u64>,
    back: u64,
    callee: u64,
    frame: u64,
}

/// A thread that started another and waits for it: the prover runs the new thread to its end
/// first, since no thread writes what another reads (M7), so the order can't change what either
/// does. What the waiting thread had in its registers, flags and calls, and where it goes on.
#[derive(Debug, Clone)]
struct Suspended {
    registers: [Term; SLOTS],
    last: Last,
    flags: [Term; 5],
    vectors: Vec<Rc<[Term; VECTOR_BYTES]>>,
    masks: [Term; 8],
    rip: u64,
    returns: Vec<Return>,
    owners: [Option<Owner>; SLOTS],
    stack_top: Option<u64>,
    base_frame: u64,
    /// Where the kernel clears the started thread's id as it ends (`CLONE_CHILD_CLEARTID`).
    clear_at: u64,
}

/// The first thread id the model gives a thread it starts.
const FIRST_TID: u64 = 1000;

/// A live frame: from its lowest byte to just below its return address, with an address in the
/// function it belongs to and its call's number.
#[derive(Debug, Clone, Copy)]
struct Frame {
    low: u64,
    top: u64,
    running: u64,
    number: u64,
}

/// One run's state.
#[derive(Debug, Clone)]
pub struct State {
    /// The CPU whose code runs.
    isa: Isa,
    registers: [Term; SLOTS],
    last: Last,
    carry: Term,
    zero: Term,
    sign: Term,
    overflow: Term,
    parity: Term,
    /// The vector registers xmm, ymm and zmm 0 to 31, each its 64 bytes from the lowest,
    /// shared with the runs a fork made until one writes it.
    vectors: Vec<Rc<[Term; VECTOR_BYTES]>>,
    /// The mask registers k0 to k7, 64 bits each.
    masks: [Term; 8],
    /// AArch64's predicates p0 to p15, a condition for each byte of a Z register, shared with
    /// the runs a fork made until one writes it.
    predicates: Vec<Rc<[Term; VECTOR_BYTES]>>,
    /// The bytes written or read so far.
    pub memory: Memory,
    /// The value last stored whole at an address, while none of its bytes has changed since:
    /// loading those bytes gives it back as it was, not rebuilt from bytes the simplifier may
    /// have rewritten.
    stored: BTreeMap<u64, Term>,
    rip: u64,
    /// The conditions the inputs meet on this run.
    pub path: Vec<Term>,
    stdin: Rc<Vec<Term>>,
    stdin_read: usize,
    /// Where standard input ends, when its length is the input's choice: `stdin` then holds as
    /// many bytes as it can be long, those past the end standing for nothing read.
    stdin_length: Option<Term>,
    /// Where in `stdin` the text of that length starts: the bytes before it, such as a whole
    /// buffer-full an induction's base puts first, are there whatever the length.
    stdin_from: usize,
    /// A loop whose exits that length decides are being merged, while the run goes round it.
    merging: Option<Box<Merging>>,
    /// The pieces standard input arrives in, by length, when it doesn't arrive whole: each read
    /// gets at most what its piece has left.
    stdin_pieces: Rc<Vec<usize>>,
    /// Whether standard output is closed, a pipe nobody reads: every write to it fails with
    /// `EPIPE`, as the test's own run has it.
    stdout_closed: bool,
    /// The files in the working folder before the run, by name.
    files: Rc<BTreeMap<String, Vec<Term>>>,
    /// The files the run created or replaced, by name, with everything written to them.
    pub written: BTreeMap<String, Vec<Term>>,
    /// The empty folders in the working folder before the run.
    folders_before: Rc<BTreeSet<String>>,
    /// The folders the run made.
    made: BTreeSet<String>,
    /// The files and folders the run deleted, or renamed away.
    removed: BTreeSet<String>,
    /// The files open, by descriptor.
    open: BTreeMap<u64, Open>,
    /// The test's network.
    network: Rc<net::Network>,
    /// The sockets open, by descriptor, with their connection's number once they have one.
    sockets: BTreeMap<u64, Option<usize>>,
    /// Where each of the test's clients stands, once the program waits on its connections with
    /// poll.
    peers: Option<Vec<net::Peer>>,
    /// The states the runs from one start have come back from poll in, after the clients took
    /// steps, shared by them all: one that comes to a state another already has goes no further.
    polled: Rc<std::cell::RefCell<HashSet<Vec<u64>>>>,
    /// The states this run came back from poll in.
    history: Option<Rc<net::History>>,
    /// Every connection the run made or took, in order.
    pub connections: Vec<net::Connection>,
    /// Bytes written to standard output, shared with the runs a fork made until one writes.
    pub stdout: Rc<Vec<Term>>,
    /// Bytes written to standard error, shared the same way.
    pub stderr: Rc<Vec<Term>>,
    steps: u64,
    /// How the run ended, once it has.
    pub end: Option<End>,
    /// How the run takes the read loop's head, in a proof by induction.
    hook: Option<Rc<induction::Hook>>,
    /// How many times it has reached that head.
    arrivals: u32,
    /// Where it stood the first time.
    head: Option<Box<induction::Snapshot>>,
    /// Where it stood the second time, one buffer-full on.
    second: Option<Box<induction::Second>>,
    /// The data symbols an address the input chooses reached among more places than a run
    /// follows one by one, each with its stores since.
    logs: Rc<Vec<logs::Log>>,
    /// The bytes a test supplies for `getrandom`, in order, when it supplies them: the kernel's
    /// are any value.
    random: Option<Rc<Vec<Term>>>,
    random_used: usize,
    /// The readings a test supplies for `clock_gettime`, seconds and nanoseconds, 64 bits each;
    /// the last one repeats. Empty, the kernel's are any time.
    clock: Rc<Vec<(Term, Term)>>,
    readings_used: usize,
    /// Whether the run read standard input or a connection with `read` itself rather than
    /// through fill, so how its input was cut matters.
    pub raw_reads: bool,
    /// Where fill stopped short of everything that had arrived, after an end byte: the cut a
    /// real run needs to stop there too.
    pub fill_cut: Option<usize>,
    /// Whether fill takes every way its input can be cut, as a proof does, or what one read
    /// gives, as a run of a test's sample does. Where it does, a read of standard input with
    /// at most [`RAW_CUT_BYTES`] left takes every count too.
    every_cut: bool,
    /// Where this run's reads of standard input stopped short of what had arrived: bit `i`
    /// after byte `i + 1`.
    pub raw_cuts: u16,
    /// The return addresses on the stack, the newest last.
    returns: Vec<Return>,
    /// How many calls the run has made: the newest frame's number.
    calls: u64,
    /// Where each general register's pointer was made, while it holds a pointer the run
    /// followed there: through lea, moves, pushes and pops, and adding numbers to it. rsp's is
    /// always the running call's frame.
    owners: [Option<Owner>; SLOTS],
    /// The owner of the pointer last stored whole at an address, while none of its bytes has
    /// changed since.
    pointers: BTreeMap<u64, Owner>,
    /// The threads waiting for the running one, the newest last.
    suspended: Vec<Suspended>,
    /// How many threads the run has started.
    threads: u64,
    /// The top of the running thread's own stack; `None` for the program's first thread, whose
    /// stack holds the arguments above it.
    stack_top: Option<u64>,
    /// The number of the running thread's first frame: 0 for `_start`'s.
    base_frame: u64,
    /// The program's arguments and environment as the run began, each from its start to its
    /// end with its name: the array of pointers first, then each string with its NUL.
    arguments: Rc<Vec<(u64, u64, String)>>,
    /// Accesses at addresses the input chooses that may leave their object: each the condition
    /// under which one does, and what it does then. The proof asks whether any can hold.
    pub obligations: Vec<(Term, String)>,
}

/// A file the program has open.
#[derive(Debug, Clone)]
struct Open {
    name: String,
    /// The next byte a read gets.
    position: usize,
    writing: bool,
}

/// `openat`'s directory meaning the working folder, as the kernel reads it: the low 32 bits,
/// an `int`. The flags lengua models.
const AT_FDCWD: u64 = 0xffff_ff9c;
const O_ACCMODE: u64 = 3;
const O_WRONLY: u64 = 1;
const O_CREAT: u64 = 0o100;
const O_TRUNC: u64 = 0o1000;
const ENOENT: i64 = 2;
const EEXIST: i64 = 17;
const ENOTDIR: i64 = 20;
const EISDIR: i64 = 21;
const ENOTEMPTY: i64 = 39;
const AT_REMOVEDIR: u64 = 0x200;
/// A regular file that its owner may read and write and others may read.
const REGULAR_FILE: u128 = 0o100_644;
const EBADF: i64 = 9;
/// `EPIPE`: a write to a pipe nobody reads, such as a closed standard output.
const EPIPE: i64 = 32;
const EINVAL: i64 = 22;
const ENOTTY: i64 = 25;
const EAGAIN: i64 = 11;
/// The bytes of one `struct pollfd`, and its event for something to read.
const POLLFD_LEN: u64 = 8;
/// The bytes of a time as the kernel reads one: seconds, then nanoseconds, 8 bytes each.
const TIMESPEC_LEN: u64 = 16;
const POLLIN: u128 = 1;

/// The general-purpose register a register is part of, 0 for rax to 15 for r15.
fn slot(register: Register) -> Option<usize> {
    let full = register.full_register();
    let index = (full as usize).checked_sub(Register::RAX as usize)?;
    (index < 16).then_some(index)
}

/// Whether `register` is ah, bh, ch or dh.
fn high_byte(register: Register) -> bool {
    matches!(
        register,
        Register::AH | Register::BH | Register::CH | Register::DH
    )
}

impl State {
    /// The state `_start` begins in: `args`, the environment's `NAME=value` strings and `stdin`
    /// as byte terms, laid out as Linux does.
    pub fn start(
        image: &Image,
        pool: &mut Pool,
        args: &[Vec<Term>],
        env: &[Vec<Term>],
        stdin: Vec<Term>,
    ) -> Self {
        let zero = pool.bv(64, 0);
        let zero_byte = pool.bv(8, 0);
        let no = pool.boolean(false);
        let mut state = Self {
            isa: image.isa,
            registers: [zero; SLOTS],
            last: Last::Other,
            carry: no,
            zero: no,
            sign: no,
            overflow: no,
            parity: no,
            // One page of zeros, shared by every register until one is written.
            vectors: {
                let zeros = Rc::new([zero_byte; VECTOR_BYTES]);
                (0..32).map(|_| Rc::clone(&zeros)).collect()
            },
            masks: [zero; 8],
            predicates: {
                let none = Rc::new([no; VECTOR_BYTES]);
                (0..16).map(|_| Rc::clone(&none)).collect()
            },
            memory: Memory::default(),
            stored: BTreeMap::new(),
            hook: None,
            arrivals: 0,
            head: None,
            second: None,
            logs: Rc::new(Vec::new()),
            rip: image.entry,
            path: Vec::new(),
            stdin: Rc::new(stdin),
            stdin_read: 0,
            stdin_length: None,
            stdin_from: 0,
            merging: None,
            stdin_pieces: Rc::new(Vec::new()),
            stdout_closed: false,
            files: Rc::new(BTreeMap::new()),
            written: BTreeMap::new(),
            folders_before: Rc::new(BTreeSet::new()),
            made: BTreeSet::new(),
            removed: BTreeSet::new(),
            open: BTreeMap::new(),
            network: Rc::new(net::Network::default()),
            sockets: BTreeMap::new(),
            peers: None,
            polled: Rc::default(),
            history: None,
            connections: Vec::new(),
            stdout: Rc::new(Vec::new()),
            stderr: Rc::new(Vec::new()),
            steps: 0,
            end: None,
            random: None,
            random_used: 0,
            clock: Rc::new(Vec::new()),
            readings_used: 0,
            raw_reads: false,
            fill_cut: None,
            every_cut: false,
            raw_cuts: 0,
            returns: Vec::new(),
            calls: 0,
            owners: [None; SLOTS],
            pointers: BTreeMap::new(),
            suspended: Vec::new(),
            threads: 0,
            stack_top: None,
            base_frame: 0,
            arguments: Rc::new(Vec::new()),
            obligations: Vec::new(),
        };
        // The strings above the pointers: the program's path, then each argument.
        let program_name: Vec<Term> = b"/lengua/main"
            .iter()
            .map(|byte| pool.bv(8, u128::from(*byte)))
            .collect();
        let mut strings = Vec::new();
        let mut objects = Vec::new();
        let mut cursor = STACK_TOP + 4096;
        let texts = std::iter::once(&program_name).chain(args).chain(env);
        for (index, text) in texts.enumerate() {
            strings.push(cursor);
            let start = cursor;
            for byte in text {
                state.memory.insert(cursor, *byte);
                cursor += 1;
            }
            let nul = pool.bv(8, 0);
            state.memory.insert(cursor, nul);
            cursor += 1;
            let name = match index.checked_sub(args.len() + 1) {
                None => format!("the program's argument {index}"),
                Some(variable) => format!("environment variable {variable}"),
            };
            objects.push((start, cursor, name));
            // A byte between strings, in neither: a read past one string's end through a
            // pointer the run didn't follow isn't taken for a read of the next.
            cursor += 1;
        }
        // argc, then argv's pointers and a null, then the environment's pointers and a null.
        let argc = args.len() + 1;
        let mut words = vec![argc as u64];
        words.extend(&strings[..argc]);
        words.push(0);
        words.extend(&strings[argc..]);
        words.push(0);
        objects.insert(
            0,
            (
                STACK_TOP,
                STACK_TOP + 8 * words.len() as u64,
                "the array of the program's arguments".into(),
            ),
        );
        state.arguments = Rc::new(objects);
        for (index, word) in words.iter().enumerate() {
            let value = pool.bv(64, u128::from(*word));
            let address = STACK_TOP + 8 * index as u64;
            state.store(pool, address, value);
            // Each pointer to a string is owned by that string.
            if index > 0
                && let Some(string) = strings.iter().position(|start| start == word)
            {
                state.pointers.insert(address, Owner::Argument(string + 1));
            }
        }
        let convention = Convention::of(image.isa);
        state.registers[convention.stack] = pool.bv(64, u128::from(STACK_TOP));
        if image.edge {
            // lengua's `_start` has called `main` with the argument count and the two arrays of
            // strings in its first three argument registers, as it reads them off the start
            // stack. Where the call pushes its return address, there is one on the stack,
            // which nothing may return to.
            let argc = words[0];
            let environment = STACK_TOP + 8 * (argc + 2);
            let values = [argc, STACK_TOP + 8, environment];
            for (slot, value) in convention.arguments.iter().zip(values) {
                state.registers[*slot] = pool.bv(64, u128::from(value));
            }
            for slot in &convention.arguments[1..3] {
                state.owners[*slot] = Some(Owner::Argument(0));
            }
            if image.isa == Isa::X86_64 {
                let back = pool.bv(64, 0);
                state.store(pool, STACK_TOP - 8, back);
                state.registers[convention.stack] = pool.bv(64, u128::from(STACK_TOP - 8));
            }
        }
        state
    }

    /// The same state, its standard input ending after `length` of its bytes, a length the
    /// input chooses: `stdin` holds as many bytes as it can be long.
    #[must_use]
    pub fn with_stdin_length(mut self, length: Term) -> Self {
        self.stdin_length = Some(length);
        self
    }

    /// Takes the read loop's head as `hook` says, for a proof by induction.
    #[must_use]
    pub fn with_hook(mut self, hook: Rc<induction::Hook>) -> Self {
        self.hook = Some(hook);
        self
    }

    /// The same state with `files` in its working folder.
    #[must_use]
    pub fn with_files(mut self, files: BTreeMap<String, Vec<Term>>) -> Self {
        self.files = Rc::new(files);
        self
    }

    /// The same state with these empty folders in its working folder.
    #[must_use]
    pub fn with_folders(mut self, folders: BTreeSet<String>) -> Self {
        self.folders_before = Rc::new(folders);
        self
    }

    /// The same state with standard output closed, or not: a pipe nobody reads, so every write
    /// to it fails with `EPIPE`.
    #[must_use]
    pub fn with_stdout_closed(mut self, closed: bool) -> Self {
        self.stdout_closed = closed;
        self
    }

    /// The same state with standard input arriving in pieces of these lengths.
    #[must_use]
    pub fn with_stdin_pieces(mut self, pieces: Vec<usize>) -> Self {
        self.stdin_pieces = Rc::new(pieces);
        self
    }

    /// The same state with the test's network: its server or client, as `network` says.
    #[must_use]
    pub fn with_network(mut self, network: net::Network) -> Self {
        // The connections lengua made before the program started, each at its own number.
        for (index, script) in network.made.iter().enumerate() {
            let number = self.connections.len();
            self.connections.push(net::Connection::made(*script));
            let fd = u64::try_from(crate::connect::FIRST_DESCRIPTOR).unwrap_or(4) + index as u64;
            self.sockets.insert(fd, Some(number));
        }
        self.network = Rc::new(network);
        self
    }

    /// The same state with a test's random bytes and clock readings in place of the kernel's.
    #[must_use]
    pub fn with_supply(mut self, random: Option<Vec<Term>>, clock: Vec<(Term, Term)>) -> Self {
        self.random = random.map(Rc::new);
        self.clock = Rc::new(clock);
        self
    }

    /// What the file `name` holds after the run, if it exists: what the run wrote, or else what
    /// it held before.
    #[must_use]
    pub fn file(&self, name: &str) -> Option<&[Term]> {
        if let Some(written) = self.written.get(name) {
            return Some(written);
        }
        if self.removed.contains(name) {
            return None;
        }
        self.files.get(name).map(Vec::as_slice)
    }

    /// Whether `name` is a folder now: given or made, or holding a file or folder, and not
    /// removed since.
    #[must_use]
    pub fn is_folder(&self, name: &str) -> bool {
        if self.made.contains(name) {
            return true;
        }
        if self.removed.contains(name) {
            return false;
        }
        let inside = format!("{name}/");
        self.folders_before.contains(name)
            || self
                .folders_before
                .iter()
                .chain(&self.made)
                .any(|folder| folder.starts_with(&inside))
            || self
                .files
                .keys()
                .chain(self.written.keys())
                .any(|file| file.starts_with(&inside) && self.file(file).is_some())
    }

    /// The folders the run made that weren't there before it.
    #[must_use]
    pub fn folders_made(&self) -> Vec<&str> {
        self.made
            .iter()
            .filter(|folder| !self.folders_before.contains(*folder))
            .map(String::as_str)
            .collect()
    }

    /// Whether some system would refuse `name` for a file call, as lengua's broker does in a
    /// run (`crate::names`): the error number, when it would. A new name (`creates`) may not
    /// differ only in case or normalization from another in its folder, and a name a call
    /// renames or removes (`removes`) may not be open.
    #[must_use]
    pub fn refused(&self, name: &str, creates: bool, removes: bool) -> Option<i64> {
        if let Some(broken) = crate::names::broken(name.as_bytes()) {
            return Some(broken.errno);
        }
        if creates {
            let (folder, last) = name
                .rsplit_once('/')
                .map_or(("", name), |(folder, last)| (folder, last));
            let inside = if folder.is_empty() {
                String::new()
            } else {
                format!("{folder}/")
            };
            let clash = self
                .files
                .keys()
                .chain(self.written.keys())
                .chain(self.folders_before.iter())
                .chain(&self.made)
                .filter_map(|other| other.strip_prefix(&inside))
                .filter(|rest| !rest.contains('/'))
                .any(|rest| {
                    let full = format!("{inside}{rest}");
                    (self.file(&full).is_some() || self.is_folder(&full))
                        && crate::names::clashes(last, rest)
                });
            if clash {
                return Some(crate::names::EEXIST);
            }
        }
        if removes && self.open.values().any(|open| open.name == name) {
            return Some(crate::names::EBUSY);
        }
        None
    }

    /// Whether the folder holding `name` is there: the working folder, or a folder in it.
    fn parent_is_there(&self, name: &str) -> bool {
        name.rsplit_once('/')
            .is_none_or(|(parent, _)| self.is_folder(parent))
    }

    /// The files the working folder held before the run.
    #[must_use]
    pub fn files_before(&self) -> &BTreeMap<String, Vec<Term>> {
        &self.files
    }

    /// Writes `value` little-endian at `address`, which must be known to be writable.
    fn store(&mut self, pool: &mut Pool, address: u64, value: Term) {
        if self.store_logged(pool, address, value) {
            return;
        }
        let bytes = pool.width(value) / 8;
        // A whole value any of these bytes belonged to is gone; values are at most 8 bytes.
        let end = address + u64::from(bytes);
        let overlapping: Vec<u64> = self
            .stored
            .range(address.saturating_sub(7)..end)
            .filter(|(start, old)| **start + u64::from(pool.width(**old) / 8) > address)
            .map(|(start, _)| *start)
            .collect();
        for start in overlapping {
            self.stored.remove(&start);
        }
        let pointing: Vec<u64> = self
            .pointers
            .range(address.saturating_sub(7)..end)
            .map(|(start, _)| *start)
            .collect();
        for start in pointing {
            self.pointers.remove(&start);
        }
        if bytes > 1 {
            self.stored.insert(address, value);
        }
        for byte in 0..bytes {
            let part = pool.extract(value, byte * 8 + 7, byte * 8);
            self.memory.insert(address + u64::from(byte), part);
        }
    }

    /// The owner of the pointer in `register`, a 64-bit general register, where the run
    /// followed it.
    /// The owner of what slot `slot` holds: the stack pointer's is the function's frame.
    fn owner_of_slot(&self, slot: usize, stack: usize) -> Option<Owner> {
        if slot == stack {
            Some(Owner::Frame(self.frame()))
        } else {
            self.owners.get(slot).copied().flatten()
        }
    }

    fn owner(&self, register: Register) -> Option<Owner> {
        if register.size() != 8 {
            return None;
        }
        match slot(register)? {
            4 => Some(Owner::Frame(self.frame())),
            index => self.owners[index],
        }
    }

    /// The running call's number, which names its frame.
    fn frame(&self) -> u64 {
        self.returns
            .last()
            .map_or(self.base_frame, |back| back.frame)
    }

    fn register(&self, pool: &mut Pool, register: Register) -> Option<Term> {
        let full = self.registers[slot(register)?];
        Some(match (register.size(), high_byte(register)) {
            (8, _) => full,
            (4, _) => pool.extract(full, 31, 0),
            (2, _) => pool.extract(full, 15, 0),
            (1, true) => pool.extract(full, 15, 8),
            _ => pool.extract(full, 7, 0),
        })
    }

    fn set_register(&mut self, pool: &mut Pool, register: Register, value: Term) -> Option<()> {
        let index = slot(register)?;
        let old = self.registers[index];
        self.registers[index] = match (register.size(), high_byte(register)) {
            (8, _) => value,
            (4, _) => pool.zero_extend(value, 32),
            (2, _) => {
                let upper = pool.extract(old, 63, 16);
                pool.concat(upper, value)
            }
            (1, true) => {
                let upper = pool.extract(old, 63, 16);
                let lower = pool.extract(old, 7, 0);
                let middle = pool.concat(value, lower);
                pool.concat(upper, middle)
            }
            _ => {
                let upper = pool.extract(old, 63, 8);
                pool.concat(upper, value)
            }
        };
        Some(())
    }
}

/// Why a step stopped the run, or the condition it split on.
enum Step {
    /// Go on with the state as changed.
    Next,
    /// Split: the state continues where `condition` holds, and `other` where it doesn't.
    Split(Term, Box<State>),
    /// Several ways the run can go for the same inputs, as a read can be cut, each where its
    /// condition holds: the state lives on as all of them.
    Choose(Vec<(Term, State)>),
}

type Stop = End;

fn unsupported(text: impl Into<String>) -> Stop {
    End::Unsupported(text.into())
}

/// Runs one step of `state`.
/// The instruction a step runs: x86-64's as iced decoded it, or AArch64's word, which is four
/// bytes long; where it is, and where the next one is.
#[derive(Clone, Copy)]
struct Running<'a> {
    instruction: &'a Instruction,
    word: Option<a64::Inst>,
    at: u64,
    next: u64,
}

fn step(image: &Image, state: &mut State, pool: &mut Pool) -> Result<Step, Stop> {
    let none = Instruction::default();
    let (instruction, word, next) = match image.isa {
        Isa::X86_64 => {
            let Some(instruction) = image.instructions.get(&state.rip).copied() else {
                return Err(End::Signal("SIGSEGV"));
            };
            let next = instruction.next_ip();
            (instruction, None, next)
        }
        Isa::Aarch64 => {
            let Some(word) = image.words.get(&state.rip).copied() else {
                return Err(End::Signal("SIGSEGV"));
            };
            (none, Some(word), state.rip + 4)
        }
    };
    let running = Running {
        instruction: &instruction,
        word,
        at: state.rip,
        next,
    };
    state.steps += 1;
    if state.steps > MAX_STEPS {
        return Err(unsupported(TOO_LONG));
    }
    let before = state.registers;
    // The read loop's head, in a proof by induction.
    if let Some(hook) = state.hook.clone()
        && state.every_cut
        && hook.head == state.rip
    {
        state.arrive(pool, &hook)?;
    }
    // fill read as a whole, where the proof takes every way its input can be cut; elsewhere,
    // as for a test's sample, its code runs as it is.
    if let Some(fill) = image.fill
        && state.every_cut
        && fill.at_head(state.rip)
    {
        // fill moves its cursor through the buffer it was lent.
        let cursor = fill::FillAbi::of(image.isa).cursor;
        let plan = Plan {
            registers: [Some((cursor, state.owners[cursor])), None, None],
            stores: [None, None],
        };
        let mut cpu = Cpu::new(image, state, pool, running);
        if let Some(mut step) = cpu.fill(fill)? {
            plan.apply_all(state, &mut step, &before);
            return Ok(step);
        }
    }
    state.rip = next;
    let stack = Convention::of(state.isa).stack;
    let below = pool.constant(state.registers[stack]);
    let mut cpu = Cpu::new(image, state, pool, running);
    let plan = cpu.plan();
    let mut step = cpu.execute();
    if let Ok(step) = &mut step {
        plan.apply_all(state, step, &before);
    }
    // Stack the instruction released, by pop, ret or add rsp, is gone: reading it again before
    // writing it reads nothing written.
    if let (Some(below), Some(above)) = (below, pool.constant(state.registers[stack]))
        && let (Ok(below), Ok(above)) = (u64::try_from(below), u64::try_from(above))
        && let (below, above) = (below.max(STACK_TOP - STACK_SIZE), above.min(STACK_TOP))
        && below < above
    {
        state.memory.forget(below, above);
        // A whole value is at most 8 bytes.
        state
            .stored
            .retain(|start, _| start.saturating_add(8) <= below || *start >= above);
        state
            .pointers
            .retain(|start, _| start.saturating_add(8) <= below || *start >= above);
    }
    step
}

#[cfg(test)]
impl State {
    /// What a concrete run left, for Round 9's prover golden: how it ended, its steps, where it
    /// stopped, its registers and flags, and hashes of what it wrote; values that aren't
    /// constants print as `?`.
    pub(crate) fn fingerprint(&self, pool: &Pool) -> String {
        let value = |term: Term| {
            pool.constant(term)
                .map_or_else(|| "?".to_owned(), |value| format!("{value:x}"))
        };
        let bytes = |terms: &[Term]| -> String {
            let known: Option<Vec<u8>> = terms
                .iter()
                .map(|term| {
                    pool.constant(*term)
                        .and_then(|byte| u8::try_from(byte).ok())
                })
                .collect();
            match known {
                Some(known) => crate::hash::Sha256Hex::of(&known).as_str()[..16].to_owned(),
                None => format!("?{}", terms.len()),
            }
        };
        let end = match &self.end {
            Some(End::Exit(status)) => format!("exit {}", value(*status)),
            Some(other) => format!("{other:?}"),
            None => "running".to_owned(),
        };
        let used = match self.isa {
            Isa::X86_64 => 16,
            Isa::Aarch64 => SLOTS,
        };
        let registers: Vec<String> = self.registers[..used]
            .iter()
            .map(|term| value(*term))
            .collect();
        let flags: Vec<String> = [self.carry, self.zero, self.sign, self.overflow, self.parity]
            .iter()
            .map(|term| value(*term))
            .collect();
        let written: Vec<String> = self
            .written
            .iter()
            .map(|(name, contents)| format!("{name}={}", bytes(contents)))
            .collect();
        format!(
            "{end}; {} steps; rip {:x}; registers {}; flags {}; out {}; err {}; written [{}]",
            self.steps,
            self.rip,
            registers.join(","),
            flags.join(","),
            bytes(&self.stdout),
            bytes(&self.stderr),
            written.join(",")
        )
    }
}

impl State {
    /// A system call's return: `result` where the program reads it, and the registers the kernel
    /// changes, for the run to go on at `next`. On x86-64 that's rax, and rcx and r11, which
    /// hold where it goes on and the flags; on AArch64, only x0 and the SVE state
    /// ([`State::clear_sve`]).
    pub(super) fn returns(&mut self, pool: &mut Pool, result: u64, next: u64) {
        self.registers[0] = pool.bv(64, u128::from(result));
        if self.isa == Isa::X86_64 {
            self.registers[1] = pool.bv(64, u128::from(next));
            self.registers[11] = pool.bv(64, 0x202);
        }
        self.clear_sve(pool);
    }

    /// What a return from a system call leaves of AArch64's SVE state, as Linux has it: Z0 to Z31
    /// keep only their low 128 bits, which are the V registers, and P0 to P15 are all clear. (The
    /// first-fault register isn't modelled.) x86-64's vector and mask registers pass through a
    /// system call as they are.
    pub(super) fn clear_sve(&mut self, pool: &mut Pool) {
        if self.isa != Isa::Aarch64 {
            return;
        }
        let zero = pool.bv(8, 0);
        for vector in &mut self.vectors {
            if vector[16..].iter().any(|byte| *byte != zero) {
                Rc::make_mut(vector)[16..].fill(zero);
            }
        }
        let no = pool.boolean(false);
        for predicate in &mut self.predicates {
            if predicate.iter().any(|lane| *lane != no) {
                Rc::make_mut(predicate).fill(no);
            }
        }
    }
}

/// One instruction being executed on a state.
struct Cpu<'a> {
    image: &'a Image,
    state: &'a mut State,
    pool: &'a mut Pool,
    /// The x86-64 instruction running, on x86-64.
    instruction: &'a Instruction,
    /// The AArch64 instruction running, on AArch64.
    word: Option<a64::Inst>,
    /// Where the instruction running is.
    at: u64,
    /// Where the next one is.
    next: u64,
}

/// The carry-less product of `a` and `b`, two `width`-bit polynomials over GF(2), in `2 *
/// width` bits: x86-64's `pclmulqdq` and AArch64's `pmull` alike.
pub(crate) fn carry_less(pool: &mut Pool, a: Term, b: Term, width: u32) -> Term {
    let wide = pool.zero_extend(a, width);
    let mut product = pool.bv(2 * width, 0);
    for at in 0..width {
        let bit = pool.extract(b, at, at);
        let amount = pool.bv(2 * width, u128::from(at));
        let shifted = pool.shl(wide, amount);
        let term = match pool.constant(bit) {
            Some(0) => continue,
            Some(_) => shifted,
            None => {
                let one = pool.bv(1, 1);
                let set = pool.eq(bit, one);
                let zero = pool.bv(2 * width, 0);
                pool.ite(set, shifted, zero)
            }
        };
        product = pool.xor(product, term);
    }
    product
}

impl<'a> Cpu<'a> {
    fn new(
        image: &'a Image,
        state: &'a mut State,
        pool: &'a mut Pool,
        running: Running<'a>,
    ) -> Self {
        Self {
            image,
            state,
            pool,
            instruction: running.instruction,
            word: running.word,
            at: running.at,
            next: running.next,
        }
    }

    fn constant(&self, term: Term) -> Result<u64, Stop> {
        self.pool
            .constant(term)
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| unsupported("an address or count that depends on the input"))
    }

    /// The address of the instruction's memory operand.
    fn address(&mut self) -> Result<u64, Stop> {
        let instruction = self.instruction;
        if instruction.is_ip_rel_memory_operand() {
            return Ok(instruction.ip_rel_memory_address());
        }
        let mut address = instruction.memory_displacement64();
        if instruction.memory_base() != Register::None {
            let base = self
                .state
                .register(self.pool, instruction.memory_base())
                .ok_or_else(|| unsupported("an unusual base register"))?;
            address = address.wrapping_add(self.constant(base)?);
        }
        if instruction.memory_index() != Register::None {
            let index = self
                .state
                .register(self.pool, instruction.memory_index())
                .ok_or_else(|| unsupported("an unusual index register"))?;
            let scale = u64::from(instruction.memory_index_scale());
            address = address.wrapping_add(self.constant(index)?.wrapping_mul(scale));
        }
        Ok(address)
    }

    fn load(&mut self, address: u64, bytes: u32) -> Result<Term, Stop> {
        // Memory an address the input chooses reached is its log's.
        if let Some(index) = self.state.log_span(address, bytes)? {
            let address = self.pool.bv(64, u128::from(address));
            return Ok(self.logged_value(index, address, bytes));
        }
        if let Some(whole) = self.state.stored.get(&address).copied()
            && self.pool.width(whole) == bytes * 8
            && self.image.region(address).is_some()
            && self.image.region(address + u64::from(bytes) - 1).is_some()
        {
            return Ok(whole);
        }
        let mut value: Option<Term> = None;
        for offset in 0..u64::from(bytes) {
            let at = address.wrapping_add(offset);
            if self.image.region(at).is_none() {
                return Err(End::Signal("SIGSEGV"));
            }
            let byte = match self.state.memory.get(at) {
                Some(byte) => byte,
                None if unwritten_stack(at) => {
                    return Err(End::Unsafe(format!(
                        "{} reads {}, which nothing wrote",
                        self.doing(),
                        self.describe(at)
                    )));
                }
                None => self.peek(at),
            };
            value = Some(match value {
                None => byte,
                Some(low) => self.pool.concat(byte, low),
            });
        }
        value.ok_or_else(|| unsupported("an empty memory access"))
    }

    /// The byte at `at` as it stands, to merge with: never-written stack is whatever was there.
    fn peek(&mut self, at: u64) -> Term {
        if let Some(index) = self.state.log_at(at) {
            let address = self.pool.bv(64, u128::from(at));
            return self.state.logged(self.pool, index, address);
        }
        if let Some(byte) = self.state.memory.get(at) {
            return byte;
        }
        match self.image.initial.get(&at) {
            Some(byte) => self.pool.bv(8, u128::from(*byte)),
            None if unwritten_stack(at) => {
                let unknown = self.pool.var(8, &format!("stack{at:x}"));
                self.state.memory.insert(at, unknown);
                unknown
            }
            // Zero-filled data (.bss) and the arguments' padding, as the kernel leaves them.
            None => self.pool.bv(8, 0),
        }
    }

    /// What the instruction running does, for a report: the function and the instruction.
    fn doing(&self) -> String {
        let function = self.image.function_at(self.at);
        match self.word {
            Some(word) => format!("in {function}, `{word}`"),
            None => format!("in {function}, `{}`", self.instruction),
        }
    }

    /// The live frames, the newest first; `_start`'s reaches over the array of the program's
    /// arguments above it, which it reads through rsp.
    fn frames(&self) -> Vec<Frame> {
        let Some(mut low) = self
            .pool
            .constant(self.state.registers[Convention::of(self.image.isa).stack])
            .and_then(|rsp| u64::try_from(rsp).ok())
        else {
            return Vec::new();
        };
        let mut frames = Vec::new();
        let mut running = self.at;
        for back in self.state.returns.iter().rev() {
            frames.push(Frame {
                low,
                top: back.top,
                running,
                number: back.frame,
            });
            low = back.top + back.pushed;
            running = back.back;
        }
        let top = self.state.stack_top.unwrap_or_else(|| {
            self.state
                .arguments
                .first()
                .map_or(STACK_TOP, |(_, end, _)| *end)
        });
        frames.push(Frame {
            low,
            top,
            running,
            number: self.state.base_frame,
        });
        frames
    }

    /// A frame as an object: the running call's is its own.
    fn frame_object(&self, frame: Frame, own: bool) -> Object {
        let function = self.image.function_at(frame.running);
        Object {
            start: frame.low,
            end: frame.top,
            name: if own {
                format!("{function}'s own frame")
            } else {
                format!("{function}'s frame")
            },
        }
    }

    /// The object `owner` names, while it lives: a frame lives until its call returns.
    fn owned(&self, owner: Owner) -> Option<Object> {
        let object = |(start, end, name): &(u64, u64, String)| Object {
            start: *start,
            end: *end,
            name: name.clone(),
        };
        match owner {
            Owner::Symbol(place) => self.image.symbols.get(place).map(object),
            Owner::Argument(place) => self.state.arguments.get(place).map(object),
            Owner::Frame(number) => self
                .frames()
                .into_iter()
                .enumerate()
                .find(|(_, frame)| frame.number == number)
                .map(|(index, frame)| self.frame_object(frame, index == 0)),
        }
    }

    /// The objects an access can be held to, with their names: data symbols, live frames, and
    /// the program's arguments, which only stack addresses can be in.
    fn objects(&self, stack: bool) -> Vec<Object> {
        let mut objects: Vec<Object> = self
            .image
            .symbols
            .iter()
            .map(|(start, end, name)| Object {
                start: *start,
                end: *end,
                name: name.clone(),
            })
            .collect();
        if stack {
            for (index, frame) in self.frames().into_iter().enumerate() {
                objects.push(self.frame_object(frame, index == 0));
            }
            for (start, end, name) in self.state.arguments.iter() {
                objects.push(Object {
                    start: *start,
                    end: *end,
                    name: name.clone(),
                });
            }
        }
        objects
    }

    /// The object an access at `address` through `pointer` is held to: the one holding the
    /// pointer, or, for an access below a pointer just past an object's end, that object.
    fn object(&self, pointer: u64, address: u64) -> Option<Object> {
        let stack = (STACK_TOP - STACK_SIZE..=STACK_TOP + ARGUMENT_ROOM).contains(&pointer);
        let objects = self.objects(stack);
        let holding = objects
            .iter()
            .find(|object| (object.start..object.end).contains(&pointer));
        let ending = objects
            .iter()
            .find(|object| object.end == pointer && object.start < object.end);
        match (holding, ending) {
            (Some(held), Some(ended)) if address < pointer && held.start == pointer => {
                Some(ended.clone())
            }
            (Some(held), _) => Some(held.clone()),
            (None, ended) => ended.cloned(),
        }
    }

    /// Where `address` is, for a report: in a data symbol, a frame, a return address, or the
    /// program's arguments.
    fn describe(&self, address: u64) -> String {
        if let Some(back) = self.state.returns.iter().find(|back| {
            back.saved
                .is_some_and(|saved| (saved..saved + 8).contains(&address))
        }) {
            return format!(
                "the return address of {}",
                self.image.function_at(back.callee)
            );
        }
        match self
            .objects(true)
            .into_iter()
            .find(|object| (object.start..object.end).contains(&address))
        {
            Some(object) => format!(
                "byte {} of {}, which has {} bytes",
                address - object.start,
                object.name,
                object.end - object.start
            ),
            None => format!("address {address:#x}, outside every object"),
        }
    }

    /// Checks that `bytes` bytes at `address`, reached by `what`, stay inside the object they're
    /// held to.
    fn reach(
        &mut self,
        held: Held,
        address: u64,
        bytes: u64,
        writing: bool,
        what: &dyn Fn(&Self) -> String,
    ) -> Result<(), Stop> {
        let object = match held {
            Held::Owner(owner) => match self.owned(owner) {
                Some(object) => object,
                None => {
                    return Err(End::Unsafe(format!(
                        "{} {} through a pointer into the frame of a call that has returned",
                        what(self),
                        if writing { "writes" } else { "reads" }
                    )));
                }
            },
            Held::Pointer(pointer) => match self.object(pointer, address) {
                Some(object) => object,
                None => return Ok(()),
            },
        };
        let last = address.wrapping_add(bytes);
        if address >= object.start && last <= object.end && last >= address {
            return Ok(());
        }
        // The first byte outside the object, and what holds it.
        let outside = if address < object.start {
            address
        } else {
            address.max(object.end)
        };
        let offset = i128::from(address) - i128::from(object.start);
        Err(End::Unsafe(format!(
            "{} {} {bytes} byte{} at byte {offset} of {}, which has {} bytes, so it reaches {}",
            what(self),
            if writing { "writes" } else { "reads" },
            if bytes == 1 { "" } else { "s" },
            object.name,
            object.end - object.start,
            self.describe(outside)
        )))
    }

    /// What the memory operand's access is held to: the owner of the pointer its address was
    /// made from, or else the object its base, or its index where the base holds no address,
    /// points into. `None` for `[rip + name]`, which rule 6 holds to the symbol main.s names,
    /// and for constant offsets from rsp, which M3 holds to the frame.
    /// What `through` says an access is held to.
    fn held_through(&mut self, through: Through) -> Option<Held> {
        match through {
            Through::X86 => self.held(),
            Through::Held(held) => held,
        }
    }

    fn held(&mut self) -> Option<Held> {
        let instruction = self.instruction;
        if instruction.is_ip_rel_memory_operand()
            || (instruction.memory_base() == Register::RSP
                && instruction.memory_index() == Register::None)
        {
            return None;
        }
        if let Some(owner) = self.address_owner() {
            return Some(Held::Owner(owner));
        }
        let base = self.value_of(instruction.memory_base());
        let index = if instruction.memory_index_scale() == 1 {
            self.value_of(instruction.memory_index())
        } else {
            None
        };
        let points = |cpu: &Self, pointer: u64| cpu.object(pointer, pointer).is_some();
        base.filter(|pointer| points(self, *pointer))
            .or_else(|| index.filter(|pointer| points(self, *pointer)))
            .map(Held::Pointer)
    }

    /// The owner of the address the memory operand computes: for `[rip + name]`, that symbol;
    /// otherwise the owner of its base or of its unscaled index, whichever alone holds a pointer
    /// the run followed.
    fn address_owner(&self) -> Option<Owner> {
        let instruction = self.instruction;
        if instruction.is_ip_rel_memory_operand() {
            return self
                .image
                .symbol_owner(instruction.ip(), instruction.ip_rel_memory_address());
        }
        let base = self.state.owner(instruction.memory_base());
        let index = if instruction.memory_index_scale() == 1 {
            self.state.owner(instruction.memory_index())
        } else {
            None
        };
        match (base, index) {
            (Some(owner), None) | (None, Some(owner)) => Some(owner),
            _ => None,
        }
    }

    /// How the instruction about to run passes owners on, from the state before it.
    fn plan(&mut self) -> Plan {
        if let Some(word) = self.word {
            return self.plan_a64(word);
        }
        let instruction = self.instruction;
        let mut plan = Plan::default();
        let register = |operand: u32| {
            (instruction.op_count() > operand && instruction.op_kind(operand) == OpKind::Register)
                .then(|| instruction.op_register(operand))
        };
        let (first, second) = (register(0), register(1));
        let owner = |cpu: &Self, register: Option<Register>| cpu.state.owner(register?);
        let (mine, theirs) = (owner(self, first), owner(self, second));
        // The pointer a store keeps whole, at its address, while its bytes stay as they are.
        let store = |cpu: &Self, plan: &mut Plan, address: Option<u64>, from: Option<Register>| {
            if let (Some(address), Some(from)) = (address, from)
                && let (Some(owner), Some(index)) = (cpu.state.owner(from), slot(from))
            {
                plan.stores[0] = Some((address, cpu.state.registers[index], owner));
            }
        };
        let rsp = self.value_of(Register::RSP);
        let (Some(to), mnemonic) = (first, instruction.mnemonic()) else {
            // A store of a pointer: `mov [m], r64`.
            if instruction.mnemonic() == Mnemonic::Mov {
                let address = self.address().ok();
                store(self, &mut plan, address, second);
            }
            return plan;
        };
        match mnemonic {
            Mnemonic::Lea => plan.set(to, self.address_owner()),
            Mnemonic::Mov if second.is_some() => plan.set(to, theirs),
            Mnemonic::Mov if instruction.op_kind(1) == OpKind::Memory => {
                let address = self.address().ok();
                plan.set(
                    to,
                    address.and_then(|address| self.state.pointers.get(&address).copied()),
                );
            }
            Mnemonic::Push => store(self, &mut plan, rsp.map(|rsp| rsp.wrapping_sub(8)), first),
            Mnemonic::Pop => plan.set(
                to,
                rsp.and_then(|rsp| self.state.pointers.get(&rsp).copied()),
            ),
            // A pointer plus or minus a number points into the same object; the distance
            // between two pointers is a number.
            Mnemonic::Add | Mnemonic::Sub => plan.set(
                to,
                match (mine, theirs) {
                    (Some(owner), None) => Some(owner),
                    (None, Some(owner)) if mnemonic == Mnemonic::Add => Some(owner),
                    _ => None,
                },
            ),
            Mnemonic::Inc | Mnemonic::Dec => plan.set(to, mine),
            // Rounding a pointer to a boundary keeps its object: an access that leaves it is
            // caught.
            Mnemonic::And if second.is_none() => plan.set(to, mine),
            Mnemonic::Xchg => {
                if let Some(other) = second {
                    plan.set(to, theirs);
                    plan.set(other, mine);
                }
            }
            // A conditional move: the owner both values share, or that of the one it picks
            // where the condition is known.
            _ if instruction.condition_code() != ConditionCode::None
                && instruction.op_count() == 2 =>
            {
                let picked = if mine == theirs {
                    mine
                } else {
                    let condition = self.condition(instruction.condition_code()).ok();
                    match condition.and_then(|condition| self.pool.truth(condition)) {
                        Some(true) => theirs,
                        Some(false) => mine,
                        None => None,
                    }
                };
                plan.set(to, picked);
            }
            _ => {}
        }
        plan
    }

    /// A general register's value, when it's a known number.
    fn value_of(&mut self, register: Register) -> Option<u64> {
        if register == Register::None {
            return None;
        }
        let term = self.state.register(self.pool, register)?;
        self.pool
            .constant(term)
            .and_then(|value| u64::try_from(value).ok())
    }

    /// Checks the memory operand's access of `bytes` bytes at `address` against what it's
    /// held to.
    fn reach_operand(&mut self, address: u64, bytes: u64, writing: bool) -> Result<(), Stop> {
        match self.held() {
            Some(held) => self.reach(held, address, bytes, writing, &Self::doing),
            None => Ok(()),
        }
    }

    /// Checks the `count` bytes at `buffer` that a system call's argument `argument` (from 0)
    /// lends the kernel, to read or, when `writing`, to write.
    fn lent(
        &mut self,
        argument: usize,
        buffer: u64,
        count: u64,
        writing: bool,
        call: &str,
    ) -> Result<(), Stop> {
        if count == 0 {
            return Ok(());
        }
        // Where the call is is `at`, on either CPU: AArch64 has no x86 instruction to ask.
        let what = |cpu: &Self| {
            format!(
                "in {}, the {call} system call",
                cpu.image.function_at(cpu.at)
            )
        };
        let convention = Convention::of(self.image.isa);
        let held = self
            .state
            .owner_of_slot(convention.arguments[argument], convention.stack)
            .map_or(Held::Pointer(buffer), Held::Owner);
        self.reach(held, buffer, count, writing, &what)
    }

    fn save(&mut self, address: u64, value: Term) -> Result<(), Stop> {
        let bytes = self.pool.width(value) / 8;
        for offset in 0..u64::from(bytes) {
            match self.image.region(address.wrapping_add(offset)) {
                Some(region) if region.writable => {}
                _ => return Err(End::Signal("SIGSEGV")),
            }
        }
        let end = address.wrapping_add(u64::from(bytes));
        if let Some(back) = self
            .state
            .returns
            .iter()
            .chain(
                self.state
                    .suspended
                    .iter()
                    .flat_map(|thread| &thread.returns),
            )
            .find(|back| {
                back.saved
                    .is_some_and(|saved| address < saved + 8 && saved < end)
            })
        {
            return Err(End::Unsafe(format!(
                "{} writes over the return address of {}",
                self.doing(),
                self.image.function_at(back.callee)
            )));
        }
        self.state.store(self.pool, address, value);
        Ok(())
    }

    /// The places an address that depends on the input can be: every value its range allows,
    /// when there are few and each holds `bytes` bytes the access may use. A place outside the
    /// object the operand's pointer points into is an obligation: the proof asks whether the
    /// input can choose it.
    fn places(
        &mut self,
        address: Term,
        bytes: u32,
        writing: bool,
        through: Through,
    ) -> Result<Vec<u64>, Stop> {
        let range = self.pool.range(address);
        // An address that's a sum of multiples of 2^s lands only every 2^s bytes.
        let stride = self.pool.stride(address);
        let held = self.held_through(through);
        let places = match self.every_place(range, stride, bytes, writing) {
            Ok(places) => places,
            Err(why) => {
                return self
                    .places_inside(held, address, range, stride, bytes, writing)
                    .ok_or(why);
            }
        };
        let mut outside = Vec::new();
        for place in &places {
            if let Some(held) = held
                && let Err(End::Unsafe(problem)) =
                    self.reach(held, *place, u64::from(bytes), writing, &Self::doing)
            {
                outside.push((*place, problem));
            }
        }
        if let Some((_, problem)) = outside.first() {
            let mut chosen = self.pool.boolean(false);
            for (place, _) in &outside {
                let constant = self.pool.bv(64, u128::from(*place));
                let here = self.pool.eq(address, constant);
                chosen = self.pool.bool_or(chosen, here);
            }
            let problem = format!("for some input, {problem}");
            self.state.obligations.push((chosen, problem));
        }
        Ok(places)
    }

    /// Every place from `low` to `high` an address landing every `step` bytes from `remainder`
    /// can be, its `bytes` all in the program's memory, and writable for a write.
    fn every_place(
        &self,
        (low, high): (u128, u128),
        (step, remainder): (u128, u128),
        bytes: u32,
        writing: bool,
    ) -> Result<Vec<u64>, Stop> {
        let first = low + (remainder + step - low % step) % step;
        if first > high || (high - first) / step >= MAX_PLACES {
            return Err(unsupported(
                "an address that depends on the input in more than 256 ways",
            ));
        }
        let mut places = Vec::new();
        for place in (first..=high).step_by(usize::try_from(step).unwrap_or(1)) {
            let place = u64::try_from(place).map_err(|_| unsupported("an address past 64 bits"))?;
            for offset in 0..u64::from(bytes) {
                match self.image.region(place.wrapping_add(offset)) {
                    Some(region) if region.writable || !writing => {}
                    _ => {
                        return Err(unsupported(
                            "an address the input chooses that may be outside the program's \
                             memory",
                        ));
                    }
                }
            }
            places.push(place);
        }
        Ok(places)
    }

    /// The places inside the object it's held to an address can be, where its range alone
    /// reaches past the program's memory: an index only a compare on the path bounds, as a
    /// checked jump table's `cmp` and `jae` do its index, needs only the places inside, and
    /// whether some input pushes the address outside becomes a question for the solvers.
    fn places_inside(
        &mut self,
        held: Option<Held>,
        address: Term,
        (low, high): (u128, u128),
        stride: (u128, u128),
        bytes: u32,
        writing: bool,
    ) -> Option<Vec<u64>> {
        let object = match held? {
            Held::Owner(owner) => self.owned(owner)?,
            Held::Pointer(pointer) => self.object(pointer, pointer)?,
        };
        let start = u128::from(object.start);
        let last = u128::from(object.end).checked_sub(u128::from(bytes))?;
        let places = self
            .every_place((low.max(start), high.min(last)), stride, bytes, writing)
            .ok()?;
        let (first, final_place) = (self.pool.bv(64, start), self.pool.bv(64, last));
        let below = self.pool.ult(address, first);
        let above = self.pool.ult(final_place, address);
        let outside = self.pool.bool_or(below, above);
        let problem = format!(
            "for some input, {} {} {bytes} byte{} outside {}, which has {} bytes",
            self.doing(),
            if writing { "writes" } else { "reads" },
            if bytes == 1 { "" } else { "s" },
            object.name,
            object.end - object.start
        );
        self.state.obligations.push((outside, problem));
        Some(places)
    }

    /// A load from an address the input chooses: the value at each place it can be, chosen by
    /// the address, so a table indexed by an input byte is a function of the byte.
    fn load_anywhere(&mut self, address: Term, bytes: u32, through: Through) -> Result<Term, Stop> {
        if let Some(object) = self.spread(address, bytes, false, through)? {
            let index = self.log_of(&object)?;
            return Ok(self.logged_value(index, address, bytes));
        }
        let places = self.places(address, bytes, false, through)?;
        let mut value: Option<Term> = None;
        for place in places.into_iter().rev() {
            let here = self.load(place, bytes)?;
            value = Some(match value {
                None => here,
                Some(other) => {
                    let constant = self.pool.bv(64, u128::from(place));
                    let chosen = self.pool.eq(address, constant);
                    self.pool.ite(chosen, here, other)
                }
            });
        }
        value.ok_or_else(|| unsupported("an address with no place"))
    }

    /// A store to an address the input chooses: each place it can be keeps what it held unless
    /// the address is that place.
    fn save_anywhere(&mut self, address: Term, value: Term, through: Through) -> Result<(), Stop> {
        let bytes = self.pool.width(value) / 8;
        if let Some(object) = self.spread(address, bytes, true, through)? {
            let index = self.log_of(&object)?;
            self.log_value(index, address, value);
            return Ok(());
        }
        for place in self.places(address, bytes, true, through)? {
            let constant = self.pool.bv(64, u128::from(place));
            let chosen = self.pool.eq(address, constant);
            for offset in 0..bytes {
                let at = place + u64::from(offset);
                let old = self.peek(at);
                let new = self.pool.extract(value, offset * 8 + 7, offset * 8);
                let byte = self.pool.ite(chosen, new, old);
                self.state.store(self.pool, at, byte);
            }
        }
        Ok(())
    }

    /// The width in bits of operand `index`.
    fn width(&self, index: u32) -> u32 {
        let instruction = self.instruction;
        match instruction.op_kind(index) {
            OpKind::Register => {
                u32::try_from(instruction.op_register(index).size() * 8).unwrap_or(64)
            }
            OpKind::Memory => u32::try_from(instruction.memory_size().size() * 8).unwrap_or(64),
            OpKind::Immediate8 => 8,
            OpKind::Immediate16 => 16,
            OpKind::Immediate32 => 32,
            _ => 64,
        }
    }

    /// Operand `index`, as a term `width` bits wide (immediates are sign-extended to it).
    fn read(&mut self, index: u32, width: u32) -> Result<Term, Stop> {
        let instruction = self.instruction;
        match instruction.op_kind(index) {
            OpKind::Register => self
                .state
                .register(self.pool, instruction.op_register(index))
                .ok_or_else(|| {
                    unsupported(format!("register {:?}", instruction.op_register(index)))
                }),
            OpKind::Memory => {
                let bytes = self.width(index) / 8;
                match self.address() {
                    Ok(address) => {
                        self.reach_operand(address, u64::from(bytes), false)?;
                        self.load(address, bytes)
                    }
                    Err(_) => {
                        let address = self.effective_address()?;
                        self.load_anywhere(address, bytes, Through::X86)
                    }
                }
            }
            OpKind::Immediate8
            | OpKind::Immediate16
            | OpKind::Immediate32
            | OpKind::Immediate64
            | OpKind::Immediate8to16
            | OpKind::Immediate8to32
            | OpKind::Immediate8to64
            | OpKind::Immediate32to64 => Ok(self
                .pool
                .bv(width, u128::from(instruction.immediate(index)))),
            other => Err(unsupported(format!("an operand of kind {other:?}"))),
        }
    }

    fn write(&mut self, index: u32, value: Term) -> Result<(), Stop> {
        let instruction = self.instruction;
        match instruction.op_kind(index) {
            OpKind::Register => self
                .state
                .set_register(self.pool, instruction.op_register(index), value)
                .ok_or_else(|| unsupported("writing an unusual register")),
            OpKind::Memory => match self.address() {
                Ok(address) => {
                    let bytes = self.pool.width(value) / 8;
                    self.reach_operand(address, u64::from(bytes), true)?;
                    self.save(address, value)
                }
                Err(_) => {
                    let address = self.effective_address()?;
                    self.save_anywhere(address, value, Through::X86)
                }
            },
            _ => Err(unsupported(
                "writing an operand that isn't a register or memory",
            )),
        }
    }

    fn bit(&mut self, value: Term, bit: u32) -> Term {
        let part = self.pool.extract(value, bit, bit);
        let one = self.pool.bv(1, 1);
        self.pool.eq(part, one)
    }

    fn unknown(&mut self) -> Term {
        let name = format!("undefined{}", self.pool.len());
        self.pool.bool_var(&name)
    }

    /// Sets ZF, SF and PF from a result.
    fn result_flags(&mut self, result: Term) {
        self.state.last = Last::Other;
        let width = self.pool.width(result);
        let zero = self.pool.bv(width, 0);
        self.state.zero = self.pool.eq(result, zero);
        self.state.sign = self.bit(result, width - 1);
        let low = self.pool.extract(result, 7, 0);
        let mut folded = low;
        for shift in [4_u128, 2, 1] {
            let amount = self.pool.bv(8, shift);
            let shifted = self.pool.lshr(folded, amount);
            folded = self.pool.xor(folded, shifted);
        }
        let odd = self.bit(folded, 0);
        self.state.parity = self.pool.bool_not(odd);
    }

    fn condition(&mut self, code: ConditionCode) -> Result<Term, Stop> {
        // After a comparison or a logic operation, a branch is a plain comparison of values,
        // which value ranges can often settle without the solver.
        let pool = &mut *self.pool;
        match self.state.last {
            Last::Compare(a, b) => {
                let found = match code {
                    ConditionCode::e => Some(pool.eq(a, b)),
                    ConditionCode::ne => {
                        let same = pool.eq(a, b);
                        Some(pool.bool_not(same))
                    }
                    ConditionCode::b => Some(pool.ult(a, b)),
                    ConditionCode::ae => {
                        let below = pool.ult(a, b);
                        Some(pool.bool_not(below))
                    }
                    ConditionCode::be => Some(pool.ule(a, b)),
                    ConditionCode::a => Some(pool.ult(b, a)),
                    ConditionCode::l => Some(pool.slt(a, b)),
                    ConditionCode::ge => {
                        let less = pool.slt(a, b);
                        Some(pool.bool_not(less))
                    }
                    ConditionCode::le => Some(pool.sle(a, b)),
                    ConditionCode::g => Some(pool.slt(b, a)),
                    _ => None,
                };
                if let Some(found) = found {
                    return Ok(found);
                }
            }
            Last::Logic(result) => {
                let width = pool.width(result);
                let zero = pool.bv(width, 0);
                let is_zero = pool.eq(result, zero);
                let negative = pool.slt(result, zero);
                let found = match code {
                    ConditionCode::e | ConditionCode::be => Some(is_zero),
                    ConditionCode::ne | ConditionCode::a => Some(pool.bool_not(is_zero)),
                    ConditionCode::s | ConditionCode::l => Some(negative),
                    ConditionCode::ns | ConditionCode::ge => Some(pool.bool_not(negative)),
                    ConditionCode::le => Some(pool.sle(result, zero)),
                    ConditionCode::g => Some(pool.slt(zero, result)),
                    ConditionCode::b | ConditionCode::o => Some(pool.boolean(false)),
                    ConditionCode::ae | ConditionCode::no => Some(pool.boolean(true)),
                    _ => None,
                };
                if let Some(found) = found {
                    return Ok(found);
                }
            }
            Last::Other => {}
        }
        let state = &*self.state;
        let (carry, zero, sign, overflow, parity) = (
            state.carry,
            state.zero,
            state.sign,
            state.overflow,
            state.parity,
        );
        let pool = &mut *self.pool;
        let less = {
            let same = pool.eq(sign, overflow);
            pool.bool_not(same)
        };
        Ok(match code {
            ConditionCode::o => overflow,
            ConditionCode::no => pool.bool_not(overflow),
            ConditionCode::b => carry,
            ConditionCode::ae => pool.bool_not(carry),
            ConditionCode::e => zero,
            ConditionCode::ne => pool.bool_not(zero),
            ConditionCode::be => pool.bool_or(carry, zero),
            ConditionCode::a => {
                let either = pool.bool_or(carry, zero);
                pool.bool_not(either)
            }
            ConditionCode::s => sign,
            ConditionCode::ns => pool.bool_not(sign),
            ConditionCode::p => parity,
            ConditionCode::np => pool.bool_not(parity),
            ConditionCode::l => less,
            ConditionCode::ge => pool.bool_not(less),
            ConditionCode::le => pool.bool_or(zero, less),
            ConditionCode::g => {
                let either = pool.bool_or(zero, less);
                pool.bool_not(either)
            }
            ConditionCode::None => return Err(unsupported("a branch without a condition")),
        })
    }

    fn push(&mut self, value: Term) -> Result<(), Stop> {
        let rsp = self.state.registers[4];
        let eight = self.pool.bv(64, 8);
        let lower = self.pool.sub(rsp, eight);
        let address = self.constant(lower)?;
        self.save(address, value)?;
        self.state.registers[4] = lower;
        Ok(())
    }

    fn pop(&mut self) -> Result<Term, Stop> {
        let rsp = self.state.registers[4];
        let address = self.constant(rsp)?;
        let value = self.load(address, 8)?;
        let eight = self.pool.bv(64, 8);
        self.state.registers[4] = self.pool.add(rsp, eight);
        Ok(value)
    }

    fn execute(&mut self) -> Result<Step, Stop> {
        if let Some(word) = self.word {
            return self.execute_a64(word);
        }
        let instruction = self.instruction;
        let mnemonic = instruction.mnemonic();
        match mnemonic {
            Mnemonic::Nop => Ok(Step::Next),
            Mnemonic::Mov => {
                let width = self.width(0);
                let value = self.read(1, width)?;
                self.write(0, value)?;
                Ok(Step::Next)
            }
            Mnemonic::Movzx | Mnemonic::Movsx | Mnemonic::Movsxd => {
                let (to, from) = (self.width(0), self.width(1));
                let value = self.read(1, from)?;
                let value = if mnemonic == Mnemonic::Movzx {
                    self.pool.zero_extend(value, to - from)
                } else {
                    self.pool.sign_extend(value, to - from)
                };
                self.write(0, value)?;
                Ok(Step::Next)
            }
            Mnemonic::Lea => {
                let value = self.effective_address()?;
                let width = self.width(0);
                let value = self.pool.extract(value, width - 1, 0);
                self.write(0, value)?;
                Ok(Step::Next)
            }
            Mnemonic::Push => {
                let value = self.read(0, 64)?;
                let value = if self.pool.width(value) < 64 {
                    let width = self.pool.width(value);
                    self.pool.sign_extend(value, 64 - width)
                } else {
                    value
                };
                self.push(value)?;
                Ok(Step::Next)
            }
            Mnemonic::Pop => {
                let value = self.pop()?;
                self.write(0, value)?;
                Ok(Step::Next)
            }
            Mnemonic::Xchg => {
                let width = self.width(0);
                let (a, b) = (self.read(0, width)?, self.read(1, width)?);
                self.write(0, b)?;
                self.write(1, a)?;
                Ok(Step::Next)
            }
            Mnemonic::Bswap | Mnemonic::Movbe => {
                let width = self.width(0);
                // movbe reads its second operand, bswap its only one.
                let from = u32::from(mnemonic == Mnemonic::Movbe);
                let value = self.read(from, width)?;
                let swapped = self.swap_bytes(value)?;
                self.write(0, swapped)?;
                Ok(Step::Next)
            }
            Mnemonic::Add | Mnemonic::Adc | Mnemonic::Sub | Mnemonic::Sbb | Mnemonic::Cmp => {
                self.arithmetic(mnemonic)
            }
            Mnemonic::Inc | Mnemonic::Dec => {
                let width = self.width(0);
                let a = self.read(0, width)?;
                let one = self.pool.bv(width, 1);
                let result = if mnemonic == Mnemonic::Inc {
                    self.pool.add(a, one)
                } else {
                    self.pool.sub(a, one)
                };
                let edge = if mnemonic == Mnemonic::Inc {
                    self.pool.bv(width, (1_u128 << (width - 1)) - 1)
                } else {
                    self.pool.bv(width, 1_u128 << (width - 1))
                };
                self.state.overflow = self.pool.eq(a, edge);
                self.result_flags(result);
                self.write(0, result)?;
                Ok(Step::Next)
            }
            Mnemonic::Neg => {
                let width = self.width(0);
                let a = self.read(0, width)?;
                let result = self.pool.neg(a);
                let zero = self.pool.bv(width, 0);
                let is_zero = self.pool.eq(a, zero);
                self.state.carry = self.pool.bool_not(is_zero);
                let minimum = self.pool.bv(width, 1_u128 << (width - 1));
                self.state.overflow = self.pool.eq(a, minimum);
                self.result_flags(result);
                self.write(0, result)?;
                Ok(Step::Next)
            }
            Mnemonic::And | Mnemonic::Or | Mnemonic::Xor | Mnemonic::Test => {
                let width = self.width(0);
                let (a, b) = (self.read(0, width)?, self.read(1, width)?);
                let result = match mnemonic {
                    Mnemonic::Or => self.pool.or(a, b),
                    Mnemonic::Xor => self.pool.xor(a, b),
                    _ => self.pool.and(a, b),
                };
                let no = self.pool.boolean(false);
                self.state.carry = no;
                self.state.overflow = no;
                self.result_flags(result);
                self.state.last = Last::Logic(result);
                if mnemonic != Mnemonic::Test {
                    self.write(0, result)?;
                }
                Ok(Step::Next)
            }
            Mnemonic::Not => {
                let width = self.width(0);
                let a = self.read(0, width)?;
                let result = self.pool.not(a);
                self.write(0, result)?;
                Ok(Step::Next)
            }
            Mnemonic::Shl | Mnemonic::Shr | Mnemonic::Sar | Mnemonic::Rol | Mnemonic::Ror => {
                self.shift(mnemonic)
            }
            Mnemonic::Imul if instruction.op_count() >= 2 => self.imul_truncated(),
            Mnemonic::Mul | Mnemonic::Imul => self.multiply(mnemonic == Mnemonic::Imul),
            Mnemonic::Div | Mnemonic::Idiv => self.divide(mnemonic == Mnemonic::Idiv),
            Mnemonic::Cbw | Mnemonic::Cwde | Mnemonic::Cdqe => {
                let (from, register) = match mnemonic {
                    Mnemonic::Cbw => (Register::AL, Register::AX),
                    Mnemonic::Cwde => (Register::AX, Register::EAX),
                    _ => (Register::EAX, Register::RAX),
                };
                let value = self
                    .state
                    .register(self.pool, from)
                    .ok_or_else(|| unsupported("cbw"))?;
                let width = self.pool.width(value);
                let wide = self.pool.sign_extend(value, width);
                self.state.set_register(self.pool, register, wide);
                Ok(Step::Next)
            }
            Mnemonic::Cwd | Mnemonic::Cdq | Mnemonic::Cqo => {
                let (from, to) = match mnemonic {
                    Mnemonic::Cwd => (Register::AX, Register::DX),
                    Mnemonic::Cdq => (Register::EAX, Register::EDX),
                    _ => (Register::RAX, Register::RDX),
                };
                let value = self
                    .state
                    .register(self.pool, from)
                    .ok_or_else(|| unsupported("cqo"))?;
                let width = self.pool.width(value);
                let amount = self.pool.bv(width, u128::from(width - 1));
                let fill = self.pool.ashr(value, amount);
                self.state.set_register(self.pool, to, fill);
                Ok(Step::Next)
            }
            Mnemonic::Cmovo
            | Mnemonic::Cmovno
            | Mnemonic::Cmovb
            | Mnemonic::Cmovae
            | Mnemonic::Cmove
            | Mnemonic::Cmovne
            | Mnemonic::Cmovbe
            | Mnemonic::Cmova
            | Mnemonic::Cmovs
            | Mnemonic::Cmovns
            | Mnemonic::Cmovp
            | Mnemonic::Cmovnp
            | Mnemonic::Cmovl
            | Mnemonic::Cmovge
            | Mnemonic::Cmovle
            | Mnemonic::Cmovg => {
                let width = self.width(0);
                let condition = self.condition(instruction.condition_code())?;
                // The source is read, and a 32-bit destination cleared above, either way.
                let source = self.read(1, width)?;
                let current = self.read(0, width)?;
                let value = self.pool.ite(condition, source, current);
                self.write(0, value)?;
                Ok(Step::Next)
            }
            Mnemonic::Seto
            | Mnemonic::Setno
            | Mnemonic::Setb
            | Mnemonic::Setae
            | Mnemonic::Sete
            | Mnemonic::Setne
            | Mnemonic::Setbe
            | Mnemonic::Seta
            | Mnemonic::Sets
            | Mnemonic::Setns
            | Mnemonic::Setp
            | Mnemonic::Setnp
            | Mnemonic::Setl
            | Mnemonic::Setge
            | Mnemonic::Setle
            | Mnemonic::Setg => {
                let condition = self.condition(instruction.condition_code())?;
                let (one, zero) = (self.pool.bv(8, 1), self.pool.bv(8, 0));
                let value = self.pool.ite(condition, one, zero);
                self.write(0, value)?;
                Ok(Step::Next)
            }
            Mnemonic::Jmp if instruction.op_kind(0) == OpKind::Register => self.table_jump(),
            Mnemonic::Jmp => {
                self.state.rip = instruction.near_branch_target();
                Ok(Step::Next)
            }
            Mnemonic::Jo
            | Mnemonic::Jno
            | Mnemonic::Jb
            | Mnemonic::Jae
            | Mnemonic::Je
            | Mnemonic::Jne
            | Mnemonic::Jbe
            | Mnemonic::Ja
            | Mnemonic::Js
            | Mnemonic::Jns
            | Mnemonic::Jp
            | Mnemonic::Jnp
            | Mnemonic::Jl
            | Mnemonic::Jge
            | Mnemonic::Jle
            | Mnemonic::Jg => {
                let condition = self.condition(instruction.condition_code())?;
                let target = instruction.near_branch_target();
                match self.pool.truth(condition) {
                    Some(true) => {
                        self.state.rip = target;
                        Ok(Step::Next)
                    }
                    Some(false) => Ok(Step::Next),
                    None => {
                        let other = self.state.clone();
                        self.state.rip = target;
                        Ok(Step::Split(condition, Box::new(other)))
                    }
                }
            }
            Mnemonic::Call => {
                // An operation of lengua's is modelled whole: nothing is pushed and no code of
                // its runs.
                if let Some(op) = self.image.op_at(instruction.near_branch_target()) {
                    return self.edge_call(op);
                }
                let back = self.pool.bv(64, u128::from(instruction.next_ip()));
                self.push(back)?;
                let slot = self.constant(self.state.registers[4])?;
                self.image.enter(instruction.near_branch_target());
                self.state.calls += 1;
                self.state.returns.push(Return {
                    top: slot,
                    pushed: 8,
                    saved: Some(slot),
                    back: instruction.next_ip(),
                    callee: instruction.near_branch_target(),
                    frame: self.state.calls,
                });
                self.state.rip = instruction.near_branch_target();
                Ok(Step::Next)
            }
            Mnemonic::Ret => {
                let slot = self.constant(self.state.registers[4])?;
                let target = self.pop()?;
                let target = self.constant(target)?;
                match self.state.returns.pop() {
                    Some(back) if back.saved == Some(slot) && back.back == target => {}
                    _ => {
                        return Err(End::Unsafe(format!(
                            "{} returns somewhere its caller didn't call it from",
                            self.doing()
                        )));
                    }
                }
                self.state.rip = target;
                Ok(Step::Next)
            }
            Mnemonic::Syscall => self.syscall(),
            Mnemonic::Popcnt => self.popcnt(),
            Mnemonic::Crc32 => self.crc32(),
            Mnemonic::Tzcnt | Mnemonic::Lzcnt | Mnemonic::Bsf | Mnemonic::Bsr => {
                self.find_bit(mnemonic)
            }
            Mnemonic::Andn
            | Mnemonic::Blsr
            | Mnemonic::Blsi
            | Mnemonic::Blsmsk
            | Mnemonic::Bzhi
            | Mnemonic::Bextr => self.bmi(mnemonic),
            Mnemonic::Shlx | Mnemonic::Shrx | Mnemonic::Sarx | Mnemonic::Rorx => {
                self.shift_keeping_flags(mnemonic)
            }
            Mnemonic::Mulx => self.mulx(),
            Mnemonic::Adcx | Mnemonic::Adox => self.add_with_flag(mnemonic),
            Mnemonic::Bt | Mnemonic::Bts | Mnemonic::Btr | Mnemonic::Btc => self.bit_test(mnemonic),
            Mnemonic::Shld | Mnemonic::Shrd => self.double_shift(mnemonic),
            Mnemonic::Pdep | Mnemonic::Pext => self.deposit(mnemonic),
            Mnemonic::Stc | Mnemonic::Clc | Mnemonic::Cmc => {
                self.set_carry(mnemonic);
                Ok(Step::Next)
            }
            other => self.vector(other),
        }
    }

    /// `jmp` through a register, which only a checked jump table does (dialect 4): the run goes
    /// on at each of the table's cases the target can be, each where the target is that case.
    /// The checker matched the table to the jump and rule 9 holds its index to the table, but a
    /// target that could be anything else still ends the proof, so it rests on neither.
    fn table_jump(&mut self) -> Result<Step, Stop> {
        let target = self.read(0, 64)?;
        self.jump_through_table(target)
    }

    /// A jump to `target` through the checked jump table of the instruction running.
    fn jump_through_table(&mut self, target: Term) -> Result<Step, Stop> {
        let ip = self.at;
        let Some(cases) = self.image.tables.get(&ip) else {
            return Err(unsupported(
                "a jump through a register that isn't a checked jump table's",
            ));
        };
        let stray = || unsupported("a jump through a table to an address that isn't its case");
        if let Some(known) = self.pool.constant(target) {
            let known = u64::try_from(known).map_err(|_| stray())?;
            if !cases.contains(&known) {
                return Err(stray());
            }
            self.state.rip = known;
            return Ok(Step::Next);
        }
        let mut choices = Vec::new();
        let mut elsewhere = self.pool.boolean(true);
        for case in cases {
            let at = self.pool.bv(64, u128::from(*case));
            let here = self.pool.eq(target, at);
            let not_here = self.pool.bool_not(here);
            elsewhere = self.pool.bool_and(elsewhere, not_here);
            let mut choice = self.state.clone();
            choice.rip = *case;
            choices.push((here, choice));
        }
        // Where no case is the target, the proof stops there: it follows only the cases.
        let mut astray = self.state.clone();
        astray.end = Some(stray());
        choices.push((elsewhere, astray));
        Ok(Step::Choose(choices))
    }

    /// The address `lea` computes, as a 64-bit term.
    fn effective_address(&mut self) -> Result<Term, Stop> {
        let instruction = self.instruction;
        if instruction.is_ip_rel_memory_operand() {
            return Ok(self
                .pool
                .bv(64, u128::from(instruction.ip_rel_memory_address())));
        }
        let mut value = self
            .pool
            .bv(64, u128::from(instruction.memory_displacement64()));
        if instruction.memory_base() != Register::None {
            let base = self
                .state
                .register(self.pool, instruction.memory_base())
                .ok_or_else(|| unsupported("lea with an unusual base"))?;
            let base = self.widen(base);
            value = self.pool.add(base, value);
        }
        if instruction.memory_index() != Register::None {
            let index = self
                .state
                .register(self.pool, instruction.memory_index())
                .ok_or_else(|| unsupported("lea with an unusual index"))?;
            let index = self.widen(index);
            let scale = self
                .pool
                .bv(64, u128::from(instruction.memory_index_scale()));
            let scaled = self.pool.mul(index, scale);
            value = self.pool.add(value, scaled);
        }
        Ok(value)
    }

    fn widen(&mut self, value: Term) -> Term {
        let width = self.pool.width(value);
        self.pool.zero_extend(value, 64 - width)
    }

    fn arithmetic(&mut self, mnemonic: Mnemonic) -> Result<Step, Stop> {
        let width = self.width(0);
        let a = self.read(0, width)?;
        let b = self.read(1, width)?;
        let with_carry = matches!(mnemonic, Mnemonic::Adc | Mnemonic::Sbb);
        let carry_in = if with_carry {
            let (one, zero) = (self.pool.bv(width + 1, 1), self.pool.bv(width + 1, 0));
            self.pool.ite(self.state.carry, one, zero)
        } else {
            self.pool.bv(width + 1, 0)
        };
        let adding = matches!(mnemonic, Mnemonic::Add | Mnemonic::Adc);
        let (wide_a, wide_b) = (self.pool.zero_extend(a, 1), self.pool.zero_extend(b, 1));
        let wide = if adding {
            let sum = self.pool.add(wide_a, wide_b);
            self.pool.add(sum, carry_in)
        } else {
            let difference = self.pool.sub(wide_a, wide_b);
            self.pool.sub(difference, carry_in)
        };
        let result = self.pool.extract(wide, width - 1, 0);
        self.state.carry = self.bit(wide, width);
        let (sign_a, sign_b, sign_result) = (
            self.bit(a, width - 1),
            self.bit(b, width - 1),
            self.bit(result, width - 1),
        );
        let operands_agree = self.pool.eq(sign_a, sign_b);
        let operands_differ = self.pool.bool_not(operands_agree);
        let result_kept = self.pool.eq(sign_result, sign_a);
        let result_flipped = self.pool.bool_not(result_kept);
        // Signed operands whose ranges keep the exact result in range never overflow, however
        // their signs fall: the sums and differences of parsed numbers.
        let ((a_low, a_high), (b_low, b_high)) =
            (self.pool.signed_range(a), self.pool.signed_range(b));
        let (minimum, maximum) = (-(1_i128 << (width - 1)), (1_i128 << (width - 1)) - 1);
        let carry_most = i128::from(with_carry);
        let fits = if adding {
            a_low.checked_add(b_low).is_some_and(|low| low >= minimum)
                && a_high
                    .checked_add(b_high)
                    .and_then(|high| high.checked_add(carry_most))
                    .is_some_and(|high| high <= maximum)
        } else {
            a_low
                .checked_sub(b_high)
                .and_then(|low| low.checked_sub(carry_most))
                .is_some_and(|low| low >= minimum)
                && a_high
                    .checked_sub(b_low)
                    .is_some_and(|high| high <= maximum)
        };
        self.state.overflow = if width <= 64 && fits {
            self.pool.boolean(false)
        } else if adding {
            self.pool.bool_and(operands_agree, result_flipped)
        } else {
            self.pool.bool_and(operands_differ, result_flipped)
        };
        if mnemonic == Mnemonic::Add {
            // Carry out of a plain addition is a result below what was added to; with a carry
            // in, the result can equal it, so adc keeps the bit computed above.
            self.state.carry = self.pool.ult(result, a);
        } else if !with_carry {
            self.state.carry = self.pool.ult(a, b);
        }
        self.result_flags(result);
        self.state.last = if matches!(mnemonic, Mnemonic::Sub | Mnemonic::Cmp) {
            Last::Compare(a, b)
        } else {
            Last::Other
        };
        if mnemonic != Mnemonic::Cmp {
            self.write(0, result)?;
        }
        Ok(Step::Next)
    }

    fn shift(&mut self, mnemonic: Mnemonic) -> Result<Step, Stop> {
        let width = self.width(0);
        let a = self.read(0, width)?;
        let count = self.read(1, 8)?;
        let mask = if width == 64 { 63 } else { 31 };
        let count = u32::try_from(self.constant(count)? & mask).unwrap_or(0);
        if count == 0 {
            // The flags stay, but the destination is still written: a 32-bit register's upper
            // half clears.
            self.write(0, a)?;
            return Ok(Step::Next);
        }
        let amount = self.pool.bv(width, u128::from(count));
        let result = match mnemonic {
            Mnemonic::Shl => self.pool.shl(a, amount),
            Mnemonic::Shr => self.pool.lshr(a, amount),
            Mnemonic::Sar => self.pool.ashr(a, amount),
            _ => {
                let rotate = count % width;
                let back = self.pool.bv(width, u128::from((width - rotate) % width));
                let rotate = self.pool.bv(width, u128::from(rotate));
                let (left, right) = if mnemonic == Mnemonic::Rol {
                    (self.pool.shl(a, rotate), self.pool.lshr(a, back))
                } else {
                    (self.pool.lshr(a, rotate), self.pool.shl(a, back))
                };
                let result = self.pool.or(left, right);
                self.state.carry = if mnemonic == Mnemonic::Rol {
                    self.bit(result, 0)
                } else {
                    self.bit(result, width - 1)
                };
                self.state.overflow = self.unknown();
                self.state.last = Last::Other;
                self.write(0, result)?;
                return Ok(Step::Next);
            }
        };
        if count <= width {
            let out = match mnemonic {
                Mnemonic::Shl => width - count,
                _ => count - 1,
            };
            self.state.carry = self.bit(a, out);
        } else {
            self.state.carry = self.unknown();
        }
        self.state.overflow = if count == 1 {
            match mnemonic {
                Mnemonic::Shl => {
                    let top = self.bit(result, width - 1);
                    let same = self.pool.eq(top, self.state.carry);
                    self.pool.bool_not(same)
                }
                Mnemonic::Shr => self.bit(a, width - 1),
                _ => self.pool.boolean(false),
            }
        } else {
            self.unknown()
        };
        self.result_flags(result);
        self.write(0, result)?;
        Ok(Step::Next)
    }

    /// `imul` with two or three operands: the low half, with CF and OF set when it doesn't hold
    /// the whole signed product.
    fn imul_truncated(&mut self) -> Result<Step, Stop> {
        let instruction = self.instruction;
        let width = self.width(0);
        let (a, b) = if instruction.op_count() == 3 {
            (self.read(1, width)?, self.read(2, width)?)
        } else {
            (self.read(0, width)?, self.read(1, width)?)
        };
        let (wide_a, wide_b) = (
            self.pool.sign_extend(a, width),
            self.pool.sign_extend(b, width),
        );
        let product = self.pool.mul(wide_a, wide_b);
        let result = self.pool.extract(product, width - 1, 0);
        let refit = self.pool.sign_extend(result, width);
        let fits = self.pool.eq(refit, product);
        let overflow = self.pool.bool_not(fits);
        self.state.last = Last::Other;
        self.state.carry = overflow;
        self.state.overflow = overflow;
        self.state.zero = self.unknown();
        self.state.sign = self.unknown();
        self.state.parity = self.unknown();
        self.write(0, result)?;
        Ok(Step::Next)
    }

    fn halves(&self, width: u32) -> (Register, Register) {
        match width {
            8 => (Register::AL, Register::AH),
            16 => (Register::AX, Register::DX),
            32 => (Register::EAX, Register::EDX),
            _ => (Register::RAX, Register::RDX),
        }
    }

    /// One-operand `mul` and `imul`: the double-width product in rdx:rax (ah:al for bytes).
    fn multiply(&mut self, signed: bool) -> Result<Step, Stop> {
        let width = self.width(0);
        let source = self.read(0, width)?;
        let (low_register, high_register) = self.halves(width);
        let a = self
            .state
            .register(self.pool, low_register)
            .ok_or_else(|| unsupported("mul"))?;
        let (wide_a, wide_b) = if signed {
            (
                self.pool.sign_extend(a, width),
                self.pool.sign_extend(source, width),
            )
        } else {
            (
                self.pool.zero_extend(a, width),
                self.pool.zero_extend(source, width),
            )
        };
        let product = self.pool.mul(wide_a, wide_b);
        let low = self.pool.extract(product, width - 1, 0);
        let high = self.pool.extract(product, 2 * width - 1, width);
        let fits = if signed {
            let refit = self.pool.sign_extend(low, width);
            self.pool.eq(refit, product)
        } else {
            let zero = self.pool.bv(width, 0);
            self.pool.eq(high, zero)
        };
        let overflow = self.pool.bool_not(fits);
        self.state.last = Last::Other;
        self.state.carry = overflow;
        self.state.overflow = overflow;
        self.state.zero = self.unknown();
        self.state.sign = self.unknown();
        self.state.parity = self.unknown();
        if width == 8 {
            self.state.set_register(self.pool, Register::AX, product);
        } else {
            self.state.set_register(self.pool, low_register, low);
            self.state.set_register(self.pool, high_register, high);
        }
        Ok(Step::Next)
    }

    /// `div` and `idiv`: rdx:rax by the operand, faulting on zero or a quotient too big.
    fn divide(&mut self, signed: bool) -> Result<Step, Stop> {
        let width = self.width(0);
        let divisor = self.read(0, width)?;
        let (low_register, high_register) = self.halves(width);
        let dividend = if width == 8 {
            self.state
                .register(self.pool, Register::AX)
                .ok_or_else(|| unsupported("div"))?
        } else {
            let low = self
                .state
                .register(self.pool, low_register)
                .ok_or_else(|| unsupported("div"))?;
            let high = self
                .state
                .register(self.pool, high_register)
                .ok_or_else(|| unsupported("div"))?;
            self.pool.concat(high, low)
        };
        let wide_divisor = if signed {
            self.pool.sign_extend(divisor, width)
        } else {
            self.pool.zero_extend(divisor, width)
        };
        let (quotient, remainder) = if signed {
            (
                self.pool.sdiv(dividend, wide_divisor),
                self.pool.srem(dividend, wide_divisor),
            )
        } else {
            (
                self.pool.udiv(dividend, wide_divisor),
                self.pool.urem(dividend, wide_divisor),
            )
        };
        let low_quotient = self.pool.extract(quotient, width - 1, 0);
        let zero = self.pool.bv(width, 0);
        let by_zero = self.pool.eq(divisor, zero);
        let too_big = if signed {
            let refit = self.pool.sign_extend(low_quotient, width);
            let fits = self.pool.eq(refit, quotient);
            self.pool.bool_not(fits)
        } else {
            let high = self.pool.extract(quotient, 2 * width - 1, width);
            let fits = self.pool.eq(high, zero);
            self.pool.bool_not(fits)
        };
        let fault = self.pool.bool_or(by_zero, too_big);
        let low_remainder = self.pool.extract(remainder, width - 1, 0);
        self.state.last = Last::Other;
        for flag in 0..5 {
            let unknown = self.unknown();
            match flag {
                0 => self.state.carry = unknown,
                1 => self.state.zero = unknown,
                2 => self.state.sign = unknown,
                3 => self.state.overflow = unknown,
                _ => self.state.parity = unknown,
            }
        }
        let mut faulted = self.state.clone();
        faulted.end = Some(End::Signal("SIGFPE"));
        if width == 8 {
            let both = self.pool.concat(low_remainder, low_quotient);
            self.state.set_register(self.pool, Register::AX, both);
        } else {
            self.state
                .set_register(self.pool, low_register, low_quotient);
            self.state
                .set_register(self.pool, high_register, low_remainder);
        }
        match self.pool.truth(fault) {
            Some(false) => Ok(Step::Next),
            Some(true) => Err(End::Signal("SIGFPE")),
            None => {
                // The run splits: where it doesn't fault it goes on, where it does it ends.
                let no_fault = self.pool.bool_not(fault);
                Ok(Step::Split(no_fault, Box::new(faulted)))
            }
        }
    }

    /// `openat` on a name the input may choose: each test file it can be, or none.
    fn open_chosen(&mut self) -> Result<Step, Stop> {
        let arguments = Convention::of(self.image.isa).arguments;
        let (folder, flags) = (
            self.state.registers[arguments[0]],
            self.state.registers[arguments[2]],
        );
        self.open_with(folder, 1, flags)
    }

    /// `openat` of the name at argument `path`, in `folder`, with `flags`.
    fn open_with(&mut self, folder: Term, path: usize, flags: Term) -> Result<Step, Stop> {
        let flags = self.constant(flags)? & 0xffff_ffff;
        if flags & self.image.linux().o_directory != 0 {
            return Err(unsupported(
                "listing a folder, whose entries the kernel gives in an order of its own",
            ));
        }
        let bytes = match self.name_or_choice(folder, path, "openat")? {
            Name::Known(name) => {
                let result = match self.state.refused(&name, flags & O_CREAT != 0, false) {
                    Some(errno) => (-errno).cast_unsigned(),
                    None => self.open_name(name, flags)?,
                };
                self.state.returns(self.pool, result, self.next);
                return Ok(Step::Next);
            }
            Name::Chosen(bytes) => bytes,
        };
        self.open_choosing(&bytes, flags)
    }

    /// `openat` of `name` with `flags`, as the working folder stands.
    fn open_name(&mut self, name: String, flags: u64) -> Result<u64, Stop> {
        // Flags that change nothing the model tracks: no controlling terminal, large files,
        // closing on exec.
        let harmless = self.image.linux().o_harmless;
        let writing = match flags & O_ACCMODE {
            0 if flags & !O_ACCMODE & !harmless == 0 => false,
            O_WRONLY if flags & !O_ACCMODE & !harmless & !O_TRUNC == O_CREAT => true,
            _ => return Err(unsupported(format!("opening a file with flags {flags:#o}"))),
        };
        if self.state.is_folder(&name) {
            return Ok((-EISDIR).cast_unsigned());
        }
        if writing {
            if !self.state.parent_is_there(&name) {
                return Ok((-ENOENT).cast_unsigned());
            }
            let exists = self.state.file(&name).is_some();
            if exists && flags & O_TRUNC == 0 {
                return Err(unsupported("writing into a file without emptying it first"));
            }
            self.state.written.insert(name.clone(), Vec::new());
        } else if self.state.written.contains_key(&name) {
            return Err(unsupported("reading a file the program wrote"));
        } else if self.state.file(&name).is_none() {
            return Ok((-ENOENT).cast_unsigned());
        }
        let fd = self.free_descriptor();
        self.state.open.insert(
            fd,
            Open {
                name,
                position: 0,
                writing,
            },
        );
        Ok(fd)
    }

    /// The name a call is given in the working folder: a NUL-terminated path relative to it,
    /// which the model follows only inside it.
    fn name_at(&mut self, folder: Term, path: usize, call: &str) -> Result<String, Stop> {
        match self.name_or_choice(folder, path, call)? {
            Name::Known(name) => Ok(name),
            Name::Chosen(_) => Err(unsupported(format!(
                "{call} on a file name the input chooses"
            ))),
        }
    }

    /// The name a call is given, or, where the input chooses its bytes, those bytes: the
    /// length is the shape's, so only which bytes is open.
    fn name_or_choice(&mut self, folder: Term, argument: usize, call: &str) -> Result<Name, Stop> {
        // The kernel reads the folder as an int.
        if self.constant(folder)? & 0xffff_ffff != AT_FDCWD {
            return Err(unsupported(format!(
                "{call} other than in the working folder"
            )));
        }
        let path = self.state.registers[Convention::of(self.image.isa).arguments[argument]];
        let start = self.constant(path)?;
        let mut address = start;
        let mut bytes = Vec::new();
        loop {
            let byte = self.load(address, 1)?;
            match self.pool.constant(byte) {
                Some(0) => break,
                Some(_) => {}
                // A byte that may be the end, or may not, leaves the length open too.
                None if self.pool.range(byte).0 == 0 => {
                    return Err(unsupported(
                        "a file name the input chooses, of a length it chooses too",
                    ));
                }
                None => {}
            }
            bytes.push(byte);
            if bytes.len() > 4096 {
                return Err(unsupported("a file name longer than 4096 bytes"));
            }
            address += 1;
        }
        self.lent(argument, start, bytes.len() as u64 + 1, false, call)?;
        let known: Option<Vec<u8>> = bytes
            .iter()
            .map(|byte| {
                self.pool
                    .constant(*byte)
                    .and_then(|value| u8::try_from(value).ok())
            })
            .collect();
        let Some(name) = known else {
            return Ok(Name::Chosen(bytes));
        };
        let name =
            String::from_utf8(name).map_err(|_| unsupported("a file name that isn't UTF-8"))?;
        // Only the test's folder is modeled: no absolute paths and no way out of it.
        if name.starts_with('/')
            || name
                .split('/')
                .any(|part| part == ".." || part == "." || part.is_empty())
        {
            return Err(unsupported(format!(
                "{call} on {name:?}, outside the working folder"
            )));
        }
        Ok(Name::Known(name.trim_end_matches('/').to_owned()))
    }

    /// A read of standard input that may return any count from 1 to `most`: a run for each,
    /// each noting where it stopped short.
    fn read_every_count(&mut self, buffer: u64, most: u64) -> Result<Step, Stop> {
        let original = self.state.clone();
        let next = self.next;
        let yes = self.pool.boolean(true);
        let mut choices = Vec::new();
        for taken in 1..=most {
            let mut outcome = original.clone();
            std::mem::swap(self.state, &mut outcome);
            let mut saved = Ok(());
            for offset in 0..taken {
                let byte = self.state.stdin[self.state.stdin_read + offset as usize];
                saved = saved.and_then(|()| self.save(buffer + offset, byte));
            }
            match saved {
                Ok(()) => {
                    self.state.stdin_read += taken as usize;
                    if self.state.stdin_read < self.state.stdin.len() && self.state.stdin_read <= 16
                    {
                        self.state.raw_cuts |= 1 << (self.state.stdin_read - 1);
                    }
                    self.state.returns(self.pool, taken, next);
                }
                Err(stop) => self.state.end = Some(stop),
            }
            std::mem::swap(self.state, &mut outcome);
            choices.push((yes, outcome));
        }
        Ok(Step::Choose(choices))
    }

    /// `openat` with `flags` on a name the input chooses: a run for each file or folder of the
    /// test's the name can be, with its bytes those on the run's path, and one where it's none
    /// of them, each as the system call leaves it. Where it's none of them, the name must be
    /// one the working folder could hold, never `/`, `.` or `..` in it, or the kernel's answer
    /// isn't the model's.
    fn open_choosing(&mut self, bytes: &[Term], flags: u64) -> Result<Step, Stop> {
        let mut names: Vec<String> = self
            .state
            .files
            .keys()
            .chain(self.state.written.keys())
            .chain(self.state.folders_before.iter())
            .chain(&self.state.made)
            .filter(|name| name.len() == bytes.len())
            .cloned()
            .collect();
        names.sort_unstable();
        names.dedup();
        let mut outcomes = Vec::new();
        let mut none = self.pool.boolean(true);
        for name in names {
            let mut same = self.pool.boolean(true);
            for (byte, wanted) in bytes.iter().zip(name.bytes()) {
                let wanted = self.pool.bv(8, u128::from(wanted));
                let equal = self.pool.eq(*byte, wanted);
                same = self.pool.bool_and(same, equal);
            }
            let other = self.pool.bool_not(same);
            none = self.pool.bool_and(none, other);
            outcomes.push((same, Some(name)));
        }
        let may = |pool: &Pool, byte: &Term, value: u8| {
            let (low, high) = pool.range(*byte);
            (low..=high).contains(&u128::from(value))
        };
        let strange = bytes.iter().any(|byte| may(self.pool, byte, b'/'))
            || (bytes.len() <= 2 && bytes.iter().all(|byte| may(self.pool, byte, b'.')));
        if strange && self.pool.truth(none) != Some(false) {
            return Err(unsupported(
                "a file name the input chooses that may leave the working folder",
            ));
        }
        // Where it's none of the test's, the name is held to every system's as a run holds it:
        // past every system's length, refused whole; Windows's ASCII rules, a split. A byte
        // past ASCII would need UTF-8's and Unicode's rules, which only a run checks.
        if self.pool.truth(none) != Some(false) {
            if bytes.len() > crate::names::MAX_NAME {
                self.state.returns(
                    self.pool,
                    (-crate::names::ENAMETOOLONG).cast_unsigned(),
                    self.next,
                );
                return Ok(Step::Next);
            }
            if bytes.iter().any(|byte| self.pool.range(*byte).1 >= 0x80) {
                return Err(unsupported(
                    "a file name the input chooses that may hold characters past ASCII, which \
                     lengua's naming rules check when it runs",
                ));
            }
        }
        let broken = self.breaks_windows_rules(bytes);
        let fine = self.pool.bool_not(broken);
        let missing = self.pool.bool_and(none, fine);
        let refused = self.pool.bool_and(none, broken);
        outcomes.push((missing, None));
        let original = self.state.clone();
        let next = self.next;
        let mut choices = Vec::new();
        for (condition, name) in outcomes
            .into_iter()
            .map(|(condition, name)| (condition, name.map(Ok)))
            .chain([(refused, Some(Err(crate::names::EINVAL)))])
        {
            if self.pool.truth(condition) == Some(false) {
                continue;
            }
            let mut outcome = original.clone();
            std::mem::swap(self.state, &mut outcome);
            let opened = match name {
                Some(Ok(name)) => self.open_name(name, flags),
                Some(Err(errno)) => Ok((-errno).cast_unsigned()),
                None if flags & O_ACCMODE == 0 => Ok((-ENOENT).cast_unsigned()),
                None => Err(unsupported("making a file the input names")),
            };
            match opened {
                Ok(result) => {
                    self.state.returns(self.pool, result, next);
                }
                Err(stop) => self.state.end = Some(stop),
            }
            std::mem::swap(self.state, &mut outcome);
            choices.push((condition, outcome));
        }
        Ok(Step::Choose(choices))
    }

    /// Whether a name of these ASCII bytes breaks one of Windows's rules (`crate::names`): a
    /// control character or one of `< > : " \ | ? *`, a dot or space at the end, a device name
    /// Windows reserves, with any extension, in any case, or a short name's shape.
    fn breaks_windows_rules(&mut self, bytes: &[Term]) -> Term {
        let mut broken = Vec::new();
        let space = self.pool.bv(8, 0x20);
        for byte in bytes {
            broken.push(self.pool.ult(*byte, space));
            for refused in *b"<>:\"\\|?*" {
                let value = self.pool.bv(8, u128::from(refused));
                broken.push(self.pool.eq(*byte, value));
            }
        }
        if let Some(last) = bytes.last() {
            for ending in *b". " {
                let value = self.pool.bv(8, u128::from(ending));
                broken.push(self.pool.eq(*last, value));
            }
        }
        // Whether the bytes from `at` spell `word` in any case.
        let spells = |pool: &mut Pool, at: usize, word: &[u8]| -> Term {
            let mut letters = Vec::new();
            for (offset, wanted) in word.iter().enumerate() {
                let byte = bytes[at + offset];
                let upper = pool.bv(8, u128::from(*wanted));
                let lower = pool.bv(8, u128::from(wanted.to_ascii_lowercase()));
                let as_upper = pool.eq(byte, upper);
                let as_lower = pool.eq(byte, lower);
                letters.push(pool.bool_or(as_upper, as_lower));
            }
            pool.all(&letters)
        };
        // A stem of `stem` letters, alone or before a dot.
        let ends_stem = |pool: &mut Pool, stem: usize| -> Option<Term> {
            match bytes.len() {
                length if length == stem => Some(pool.boolean(true)),
                length if length > stem => {
                    let dot = pool.bv(8, u128::from(b'.'));
                    Some(pool.eq(bytes[stem], dot))
                }
                _ => None,
            }
        };
        if let Some(ended) = ends_stem(self.pool, 3) {
            for device in [b"CON", b"PRN", b"AUX", b"NUL"] {
                let named = spells(self.pool, 0, device);
                broken.push(self.pool.bool_and(named, ended));
            }
        }
        if let Some(ended) = ends_stem(self.pool, 4) {
            let one = self.pool.bv(8, u128::from(b'1'));
            let nine = self.pool.bv(8, u128::from(b'9'));
            let low = self.pool.ule(one, bytes[3]);
            let high = self.pool.ule(bytes[3], nine);
            let digit = self.pool.bool_and(low, high);
            for device in [b"COM", b"LPT"] {
                let named = spells(self.pool, 0, device);
                let numbered = self.pool.bool_and(named, digit);
                broken.push(self.pool.bool_and(numbered, ended));
            }
        }
        broken.push(short_name_shape(self.pool, bytes));
        self.pool.any(&broken)
    }

    /// `fstat(fd, details)`: an open file's kind and size, the file's own; the kernel's other
    /// numbers, such as its device, owner and times, are any value.
    fn file_details(&mut self, fd: u64, buffer: u64) -> Result<u64, Stop> {
        // The bytes of `struct stat`, and where its kind and size are, on this CPU.
        let linux = self.image.linux();
        self.lent(1, buffer, linux.stat_len, true, "fstat")?;
        let Some(open) = self.state.open.get(&fd).cloned() else {
            if fd <= 2 {
                return Err(unsupported("the details of standard input or output"));
            }
            return Ok((-EBADF).cast_unsigned());
        };
        let size = self.state.file(&open.name).map_or(0, <[Term]>::len);
        let fresh = self.pool.len();
        for offset in 0..linux.stat_len {
            let byte = self.pool.var(8, &format!("stat{fresh}.{offset}"));
            self.save(buffer + offset, byte)?;
        }
        let mode = self.pool.bv(32, REGULAR_FILE);
        self.save(buffer + linux.stat_mode, mode)?;
        let size = self.pool.bv(64, size as u128);
        self.save(buffer + linux.stat_size, size)?;
        Ok(0)
    }

    /// `mkdirat(folder, name, mode)`: a new folder, where its parent is.
    fn make_folder(&mut self, name: String) -> u64 {
        if self.state.is_folder(&name) || self.state.file(&name).is_some() {
            return (-EEXIST).cast_unsigned();
        }
        if !self.state.parent_is_there(&name) {
            return (-ENOENT).cast_unsigned();
        }
        self.state.removed.remove(&name);
        self.state.made.insert(name);
        0
    }

    /// `unlinkat(folder, name, flags)`: a file deleted, or with `AT_REMOVEDIR`, an empty folder.
    fn remove(&mut self, name: String, flags: u64) -> Result<u64, Stop> {
        let inside = format!("{name}/");
        let errno = match flags {
            0 if self.state.file(&name).is_some() => {
                self.state.written.remove(&name);
                self.state.removed.insert(name);
                return Ok(0);
            }
            0 if self.state.is_folder(&name) => EISDIR,
            AT_REMOVEDIR if self.state.is_folder(&name) => {
                let holds = self
                    .state
                    .files
                    .keys()
                    .chain(self.state.written.keys())
                    .any(|file| file.starts_with(&inside) && self.state.file(file).is_some())
                    || self
                        .state
                        .folders_before
                        .iter()
                        .chain(&self.state.made)
                        .any(|folder| folder.starts_with(&inside) && self.state.is_folder(folder));
                if holds {
                    ENOTEMPTY
                } else {
                    self.state.made.remove(&name);
                    self.state.removed.insert(name);
                    return Ok(0);
                }
            }
            AT_REMOVEDIR if self.state.file(&name).is_some() => ENOTDIR,
            0 | AT_REMOVEDIR => ENOENT,
            other => return Err(unsupported(format!("unlinkat with flags {other:#x}"))),
        };
        Ok((-errno).cast_unsigned())
    }

    /// `renameat(folder, from, folder, to)`: a file moved to a name, replacing what's there.
    fn rename(&mut self, from: String, to: String) -> Result<u64, Stop> {
        if self.state.is_folder(&from) {
            return Err(unsupported("renaming a folder"));
        }
        let Some(contents) = self.state.file(&from).map(<[Term]>::to_vec) else {
            return Ok((-ENOENT).cast_unsigned());
        };
        if self.state.is_folder(&to) {
            return Ok((-EISDIR).cast_unsigned());
        }
        if !self.state.parent_is_there(&to) {
            return Ok((-ENOENT).cast_unsigned());
        }
        if from != to {
            self.state.written.remove(&from);
            self.state.removed.insert(from);
            self.state.removed.remove(&to);
            self.state.written.insert(to, contents);
        }
        Ok(0)
    }

    /// `clone` as `parallel_for` makes it, with [`Profile::CLONE_FLAGS`]: a thread on the stack
    /// it's given, which runs to its end now, while this thread waits to go on with the new
    /// thread's id. Its id is where this thread asked (`CLONE_PARENT_SETTID`) before either runs.
    ///
    /// [`Profile::CLONE_FLAGS`]: crate::platform::Profile::CLONE_FLAGS
    fn start_thread(&mut self) -> Result<Step, Stop> {
        let convention = Convention::of(self.image.isa);
        let registers = self.state.registers;
        let argument = |number: usize| registers[convention.arguments[number]];
        let flags = self.constant(argument(0))?;
        if flags != crate::platform::Profile::CLONE_FLAGS {
            return Err(unsupported(format!("clone with flags {flags:#x}")));
        }
        let (stack, parent_tid, child_tid) = (
            self.constant(argument(1))?,
            self.constant(argument(2))?,
            self.constant(argument(convention.child_tid))?,
        );
        self.state.threads += 1;
        let tid = FIRST_TID + self.state.threads;
        let id = self.pool.bv(32, u128::from(tid));
        self.save(parent_tid, id)?;
        // Both threads come back from the system call, which clears AArch64's SVE state.
        self.state.clear_sve(self.pool);
        let next = self.next;
        let (next_term, flags_word) = (self.pool.bv(64, u128::from(next)), self.pool.bv(64, 0x202));
        let mut waiting = registers;
        waiting[0] = self.pool.bv(64, u128::from(tid));
        if let Some((back, flags_at)) = convention.clobbered {
            waiting[back] = next_term;
            waiting[flags_at] = flags_word;
        }
        let state = &mut *self.state;
        state.suspended.push(Suspended {
            registers: waiting,
            last: state.last,
            flags: [
                state.carry,
                state.zero,
                state.sign,
                state.overflow,
                state.parity,
            ],
            vectors: state.vectors.clone(),
            masks: state.masks,
            rip: next,
            returns: std::mem::take(&mut state.returns),
            owners: state.owners,
            stack_top: state.stack_top,
            base_frame: state.base_frame,
            clear_at: child_tid,
        });
        // The new thread: the same registers but the result, 0, and the stack pointer, its own
        // stack, with no calls yet and a first frame of its own.
        state.registers[0] = self.pool.bv(64, 0);
        if let Some((back, flags_at)) = convention.clobbered {
            state.registers[back] = next_term;
            state.registers[flags_at] = flags_word;
        }
        state.registers[convention.stack] = self.pool.bv(64, u128::from(stack));
        state.stack_top = Some(stack);
        state.calls += 1;
        state.base_frame = state.calls;
        state.rip = next;
        Ok(Step::Next)
    }

    /// A started thread's `exit`: the kernel clears its id (`CLONE_CHILD_CLEARTID`), and the
    /// thread that started it goes on where it waited.
    fn end_thread(&mut self) -> Result<Step, Stop> {
        let Some(waiting) = self.state.suspended.pop() else {
            return Err(unsupported("a thread's end with no thread to go back to"));
        };
        let cleared = self.pool.bv(32, 0);
        self.save(waiting.clear_at, cleared)?;
        let state = &mut *self.state;
        state.registers = waiting.registers;
        state.last = waiting.last;
        [
            state.carry,
            state.zero,
            state.sign,
            state.overflow,
            state.parity,
        ] = waiting.flags;
        state.vectors = waiting.vectors;
        state.masks = waiting.masks;
        // The thread that waited goes on from its system call's return, which cleared the SVE
        // state: the predicates the ended thread left must not reach it.
        state.clear_sve(self.pool);
        state.rip = waiting.rip;
        state.returns = waiting.returns;
        state.owners = waiting.owners;
        state.stack_top = waiting.stack_top;
        state.base_frame = waiting.base_frame;
        Ok(Step::Next)
    }

    fn syscall(&mut self) -> Result<Step, Stop> {
        let convention = Convention::of(self.image.isa);
        let raw = self.state.registers[convention.number];
        let raw = self
            .pool
            .constant(raw)
            .ok_or_else(|| unsupported("a system call chosen by the input"))?;
        // Each CPU's numbers, read from its own table as what the call does.
        let call = i64::try_from(raw)
            .ok()
            .and_then(|number| Call::of(self.image.isa, number));
        self.perform(call, raw)
    }

    /// What the kernel does for `call`, which the program made with `raw` as its number, with
    /// its arguments in the registers: the same for a system call and for an operation of
    /// lengua's that is one.
    fn perform(&mut self, call: Option<Call>, raw: u128) -> Result<Step, Stop> {
        let convention = Convention::of(self.image.isa);
        let arguments = convention.arguments.map(|slot| self.state.registers[slot]);
        let (rdi, rsi, rdx) = (arguments[0], arguments[1], arguments[2]);
        let result = match call {
            Some(Call::Read) => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let buffer = self.constant(rsi)?;
                let count = self.constant(rdx)?;
                self.lent(1, buffer, count, true, "read")?;
                if fd == 0 {
                    self.state.raw_reads = true;
                    if self.state.stdin_length.is_some() {
                        return Err(unsupported(
                            "standard input of a length the input chooses, read with read \
                             itself rather than fill",
                        ));
                    }
                    // A pipe hands over at most 4096 bytes whole; past that, how the input
                    // arrives matters, which only fill makes not matter. A test's sample is run
                    // as one read gives it.
                    if self.state.every_cut && self.state.stdin.len() > super::PIPE_WRITE {
                        return Err(End::Unsupported(format!(
                            "standard input past {} bytes is more than one pipe write, which the \
                             proof reads whole; read it with fill to be proven for any length",
                            super::PIPE_WRITE
                        )));
                    }
                    let available = (self.state.stdin.len() - self.state.stdin_read) as u64;
                    // Short input read with read itself may arrive in any pieces: each count
                    // a read can return is its own run.
                    if self.state.every_cut
                        && self.state.stdin_pieces.is_empty()
                        && available <= RAW_CUT_BYTES
                        && count.min(available) > 1
                    {
                        return self.read_every_count(buffer, count.min(available));
                    }
                    let mut taken = count.min(available);
                    if !self.state.stdin_pieces.is_empty() {
                        let mut boundary = 0;
                        for piece in self.state.stdin_pieces.iter() {
                            boundary += piece;
                            if boundary > self.state.stdin_read {
                                break;
                            }
                        }
                        taken = taken.min(boundary.saturating_sub(self.state.stdin_read) as u64);
                    }
                    for offset in 0..taken {
                        let byte = self.state.stdin[self.state.stdin_read + offset as usize];
                        self.save(buffer + offset, byte)?;
                    }
                    self.state.stdin_read += taken as usize;
                    taken
                } else if let Ok(socket) = self.connection_of(fd) {
                    self.state.raw_reads = true;
                    match socket {
                        Some(number) => self.receive(number, buffer, count)?,
                        None => self.not_connected(),
                    }
                } else if let Some(open) = self.state.open.get(&fd).cloned()
                    && !open.writing
                {
                    // A regular file gives everything asked for, up to its end.
                    let files = Rc::clone(&self.state.files);
                    let contents = files.get(&open.name).map_or(&[][..], Vec::as_slice);
                    let available = (contents.len() - open.position.min(contents.len())) as u64;
                    let taken = count.min(available);
                    for offset in 0..taken {
                        let byte = contents[open.position + offset as usize];
                        self.save(buffer + offset, byte)?;
                    }
                    if let Some(open) = self.state.open.get_mut(&fd) {
                        open.position += taken as usize;
                    }
                    taken
                } else {
                    (-EBADF).cast_unsigned()
                }
            }
            Some(Call::Write)
                if self
                    .connection_of(self.constant(rdi)? & 0xffff_ffff)
                    .is_ok() =>
            {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let (buffer, count) = (self.constant(rsi)?, self.constant(rdx)?);
                self.lent(1, buffer, count, false, "write")?;
                match self.connection_of(fd) {
                    Ok(Some(number)) => self.send(number, buffer, count, 0)?,
                    _ => self.not_connected(),
                }
            }
            Some(Call::Write) => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let buffer = self.constant(rsi)?;
                let count = self.constant(rdx)?;
                self.lent(1, buffer, count, false, "write")?;
                // Standard output or error, a file open for writing, or nothing that can be. A
                // closed standard output takes nothing: the write fails, as on a pipe nobody reads.
                let target = match fd {
                    1 if self.state.stdout_closed => Err(EPIPE),
                    1 | 2 => Ok(None),
                    _ => match self.state.open.get(&fd) {
                        Some(open) if open.writing => Ok(Some(open.name.clone())),
                        _ => Err(EBADF),
                    },
                };
                match target {
                    Err(errno) => (-errno).cast_unsigned(),
                    Ok(file) => {
                        for offset in 0..count {
                            let byte = self.load(buffer + offset, 1)?;
                            match (&file, fd) {
                                (Some(name), _) => {
                                    self.state
                                        .written
                                        .entry(name.clone())
                                        .or_default()
                                        .push(byte);
                                }
                                (None, 1) => Rc::make_mut(&mut self.state.stdout).push(byte),
                                (None, _) => Rc::make_mut(&mut self.state.stderr).push(byte),
                            }
                        }
                        count
                    }
                }
            }
            Some(Call::Close) => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                if self.state.open.remove(&fd).is_some() || self.close_socket(fd) {
                    0
                } else {
                    (-EBADF).cast_unsigned()
                }
            }
            Some(Call::Accept | Call::Accept4) => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let (address, length) = (self.constant(rsi)?, self.constant(rdx)?);
                if call == Some(Call::Accept4) && self.constant(arguments[3])? & !0o2_000_000 != 0 {
                    return Err(unsupported(
                        "accepting a connection with flags beyond close-on-exec",
                    ));
                }
                if address != 0 {
                    self.lent(1, address, 16, true, "accept")?;
                    self.lent(2, length, 4, true, "accept")?;
                }
                self.accept(fd, address, length)?
            }
            Some(Call::Sendto | Call::Recvfrom) => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let (buffer, count) = (self.constant(rsi)?, self.constant(rdx)?);
                let flags = self.constant(arguments[3])? & 0xffff_ffff;
                if self.constant(arguments[4])? != 0 {
                    return Err(unsupported("sending to or receiving from a named address"));
                }
                let sending = call == Some(Call::Sendto);
                let call = if sending { "sendto" } else { "recvfrom" };
                self.lent(1, buffer, count, !sending, call)?;
                match self.connection_of(fd) {
                    Err(()) => (-EBADF).cast_unsigned(),
                    Ok(None) => self.not_connected(),
                    Ok(Some(connection)) if sending => {
                        self.send(connection, buffer, count, flags)?
                    }
                    Ok(Some(connection)) => {
                        if flags != 0 {
                            return Err(unsupported(format!("receiving with flags {flags:#x}")));
                        }
                        self.receive(connection, buffer, count)?
                    }
                }
            }
            Some(Call::Shutdown) => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let how = self.constant(rsi)? & 0xffff_ffff;
                self.shutdown(fd, how)?
            }
            Some(Call::Openat) => return self.open_chosen(),
            Some(Call::Fstat) => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let buffer = self.constant(rsi)?;
                self.file_details(fd, buffer)?
            }
            Some(Call::Getdents64) => {
                return Err(unsupported(
                    "listing a folder, whose entries the kernel gives in an order of its own",
                ));
            }
            // Each name held to every system's, as lengua's broker holds it in a run.
            Some(Call::Mkdirat) => {
                let name = self.name_at(rdi, 1, "mkdirat")?;
                match self.state.refused(&name, true, false) {
                    Some(errno) => (-errno).cast_unsigned(),
                    None => self.make_folder(name),
                }
            }
            Some(Call::Unlinkat) => {
                let name = self.name_at(rdi, 1, "unlinkat")?;
                let flags = self.constant(rdx)? & 0xffff_ffff;
                match self.state.refused(&name, false, true) {
                    Some(errno) => (-errno).cast_unsigned(),
                    None => self.remove(name, flags)?,
                }
            }
            Some(Call::Renameat) => {
                let from = self.name_at(rdi, 1, "renameat")?;
                let to = self.name_at(rdx, 3, "renameat")?;
                let refused = self
                    .state
                    .refused(&from, false, true)
                    .or_else(|| self.state.refused(&to, true, true));
                match refused {
                    Some(errno) => (-errno).cast_unsigned(),
                    None => self.rename(from, to)?,
                }
            }
            // The test's readings, as the broker gives them, the last one repeating.
            Some(Call::ClockGettime) if !self.state.clock.is_empty() => {
                let buffer = self.constant(rsi)?;
                self.lent(1, buffer, 16, true, "clock_gettime")?;
                let readings = Rc::clone(&self.state.clock);
                let index = self.state.readings_used.min(readings.len() - 1);
                let (seconds, nanoseconds) = readings[index];
                self.save(buffer, seconds)?;
                self.save(buffer + 8, nanoseconds)?;
                self.state.readings_used += 1;
                0
            }
            // The test's bytes, as the broker gives them: zeros past their end.
            Some(Call::Getrandom) if self.state.random.is_some() => {
                let buffer = self.constant(rdi)?;
                let count = self.constant(rsi)?;
                self.lent(0, buffer, count, true, "getrandom")?;
                let flags = self.constant(rdx)?;
                if flags & !7 != 0 || flags & 6 == 6 {
                    (-EINVAL).cast_unsigned()
                } else {
                    if count > 4096 {
                        return Err(unsupported("getrandom for more than 4096 bytes at once"));
                    }
                    let bytes = self.state.random.clone().unwrap_or_default();
                    let used = self.state.random_used;
                    for offset in 0..count {
                        let index = used + usize::try_from(offset).unwrap_or(usize::MAX);
                        let byte = match bytes.get(index) {
                            Some(byte) => *byte,
                            None => self.pool.bv(8, 0),
                        };
                        self.save(buffer + offset, byte)?;
                    }
                    self.state.random_used += usize::try_from(count).unwrap_or(usize::MAX);
                    count
                }
            }
            // Values the kernel chooses: any at all, a fresh variable for each.
            Some(Call::ClockGettime) => {
                let clock = self.constant(rdi)? & 0xffff_ffff;
                if !matches!(clock, 0 | 1) {
                    return Err(unsupported(format!("reading clock {clock}")));
                }
                let buffer = self.constant(rsi)?;
                self.lent(1, buffer, 16, true, "clock_gettime")?;
                let fresh = self.pool.len();
                let seconds =
                    self.pool
                        .var_in(64, &format!("clock{fresh}.seconds"), 0, (1 << 62) - 1);
                let nanoseconds =
                    self.pool
                        .var_in(64, &format!("clock{fresh}.nanoseconds"), 0, 999_999_999);
                self.save(buffer, seconds)?;
                self.save(buffer + 8, nanoseconds)?;
                0
            }
            Some(Call::Getrandom) => {
                let buffer = self.constant(rdi)?;
                let count = self.constant(rsi)?;
                self.lent(0, buffer, count, true, "getrandom")?;
                if count > 256 {
                    return Err(unsupported("getrandom for more than 256 bytes at once"));
                }
                let fresh = self.pool.len();
                for offset in 0..count {
                    let byte = self.pool.var(8, &format!("random{fresh}.{offset}"));
                    self.save(buffer + offset, byte)?;
                }
                count
            }
            // A sleep ends at once: what the program does next doesn't depend on how long it
            // slept. It reads the time it's given, and may write what's left of it.
            Some(Call::ClockNanosleep) => {
                let request = self.constant(rdx)?;
                self.lent(2, request, 16, false, "clock_nanosleep")?;
                self.load(request, 16)?;
                let left = self.constant(arguments[3])?;
                if left != 0 {
                    self.lent(3, left, 16, true, "clock_nanosleep")?;
                }
                0
            }
            Some(Call::Clone) => return self.start_thread(),
            // A thread's end: the kernel clears its id and wakes whoever waits on it, and the
            // thread that started it goes on.
            Some(Call::Exit) if !self.state.suspended.is_empty() => return self.end_thread(),
            Some(Call::Exit | Call::ExitGroup) => {
                let status = self.pool.extract(rdi, 7, 0);
                return Err(End::Exit(status));
            }
            // A wait for a thread to end. Threads run to their end as they start, so its id is
            // already cleared: the word no longer holds what the waiter saw.
            Some(Call::Futex) => {
                let (word, operation) = (self.constant(rdi)?, self.constant(rsi)? & 0xffff_ffff);
                // FUTEX_WAIT, with or without FUTEX_PRIVATE_FLAG.
                if operation & !128 != 0 {
                    return Err(unsupported(format!("futex operation {operation}")));
                }
                self.lent(0, word, 4, false, "futex")?;
                let now = self.load(word, 4)?;
                let seen = self.pool.extract(rdx, 31, 0);
                if self.pool.constant(now) == self.pool.constant(seen) {
                    return Err(unsupported(
                        "a wait for a word no thread changes, which would never end",
                    ));
                }
                (-EAGAIN).cast_unsigned()
            }
            // A wait for the next key: as in a test, standard input has all arrived, so it always
            // has a key to read, or its end. A server's poll over its connections is beyond the
            // model.
            Some(call @ (Call::Poll | Call::Ppoll)) => {
                let (table, count) = (self.constant(rdi)?, self.constant(rsi)?);
                // ppoll's time limit is a pointer to how long, none meaning until one is ready,
                // which the kernel reads and then leaves the time left in; its signal mask is
                // what the dialect leaves out.
                let (until_ready, left_at) = if call == Call::Ppoll {
                    if self.constant(arguments[3])? != 0 {
                        return Err(unsupported("ppoll with a signal mask"));
                    }
                    let limit = self.constant(rdx)?;
                    if limit != 0 {
                        self.lent(2, limit, TIMESPEC_LEN, true, "ppoll")?;
                    }
                    (Some(limit == 0), (limit != 0).then_some(limit))
                } else {
                    (None, None)
                };
                let waits_for_input = count == 1 && {
                    self.lent(0, table, POLLFD_LEN, true, "poll")?;
                    let descriptor = self.load(table, 4)?;
                    self.pool.constant(descriptor) == Some(0)
                };
                // A server's wait on its listener and connections.
                if !waits_for_input
                    && (self.state.network.listening || !self.state.sockets.is_empty())
                {
                    if count == 0 || count > 1024 {
                        return Err(unsupported(format!("poll over {count} descriptors")));
                    }
                    self.lent(0, table, count * POLLFD_LEN, true, "poll")?;
                    let forever = match until_ready {
                        Some(forever) => forever,
                        None => self.constant(rdx)? & 0xffff_ffff == 0xffff_ffff,
                    };
                    if !forever {
                        return Err(unsupported(
                            "poll over connections with a time limit, rather than until one is \
                             ready",
                        ));
                    }
                    return self.poll_network(table, count);
                }
                if !waits_for_input {
                    return Err(unsupported(
                        "poll over several descriptors, with which a server serves clients at once",
                    ));
                }
                let ready = self.pool.bv(16, POLLIN);
                self.save(table + 6, ready)?;
                // The time ppoll left, which the proof knows nothing of.
                if let Some(limit) = left_at {
                    let fresh = self.pool.len();
                    for offset in 0..TIMESPEC_LEN {
                        let byte = self.pool.var(8, &format!("ppoll{fresh}.{offset}"));
                        self.save(limit + offset, byte)?;
                    }
                }
                1
            }
            // As in a test, standard input, output and error are never a terminal: its mode
            // can't be read or set, and the program goes on as from a pipe.
            Some(Call::Ioctl) => {
                if self.constant(rdi)? & 0xffff_ffff > 2 {
                    return Err(unsupported("ioctl on a file or a connection"));
                }
                (-ENOTTY).cast_unsigned()
            }
            _ => return Err(unsupported(format!("system call {raw}"))),
        };
        self.state.returns(self.pool, result, self.next);
        Ok(Step::Next)
    }
}

/// Decides whether a condition can hold for some input on a run, given the run's conditions.
pub trait Oracle {
    /// Whether `condition` and every one of `path` can hold together; true when unsure.
    ///
    /// # Errors
    ///
    /// Fails when the solver stops responding.
    fn possible(&mut self, pool: &Pool, path: &[Term], condition: Term) -> Result<bool, String>;

    /// Whether the proof still has its time. [`explore`] asks every few thousand steps, so a run
    /// through code with no branch on the inputs, such as a hash of them, which never asks
    /// [`Self::possible`] anything, still ends with the proof's time.
    ///
    /// # Errors
    ///
    /// Fails when the time is up.
    fn in_time(&self) -> Result<(), String> {
        Ok(())
    }

    /// Whether the computer still has the memory a proof needs. [`explore`] asks each time its
    /// pool has grown by tens of thousands of terms, so a run that makes terms by the million
    /// ends before the kernel's out-of-memory killer ends whatever it picks, the session the
    /// agent runs in among it.
    ///
    /// # Errors
    ///
    /// Fails when free memory is below the floor the watchdog keeps.
    fn has_room(&self) -> Result<(), String> {
        if super::watch::is_low() {
            tracing::warn!("free memory ran low: a run's loop stops its proof");
            return Err(super::watch::OUT_OF_MEMORY.into());
        }
        Ok(())
    }
}

/// A run that left a loop whose exits are merged: the condition it left on, whether the run was
/// still in the loop then, and the run; and the one that left before it.
#[derive(Debug)]
struct Parked {
    leaves: Term,
    alive: Term,
    state: State,
    before: Option<Rc<Parked>>,
}

impl Merging {
    /// The runs that left, first first.
    fn exits(&self) -> Vec<Rc<Parked>> {
        let mut exits = Vec::new();
        let mut at = self.parked.clone();
        while let Some(node) = at {
            at = node.before.clone();
            exits.push(node);
        }
        exits.reverse();
        exits
    }
}

/// Where `state`, a run whose input's length is its choice, leaves a loop at a branch both sides
/// of which can happen and `other` doesn't, or the other way round: the side that leaves waits at
/// the exit and `state` becomes the one that stays, still going round. `other` back where the
/// branch isn't such an exit, or leaves for another place than this loop's exits so far.
fn park(
    image: &Image,
    pool: &mut Pool,
    state: &mut State,
    other: State,
    condition: Term,
) -> Option<State> {
    if state.stdin_length.is_none() {
        return Some(other);
    }
    let inside = |found: &Loop, rip: u64| (found.head..=found.latch).contains(&rip);
    // The innermost loop the branch leaves.
    let Some(found) = image
        .loops()
        .filter(|found| inside(found, state.rip) != inside(found, other.rip))
        .min_by_key(|found| found.latch - found.head)
        .copied()
    else {
        return Some(other);
    };
    let state_leaves = !inside(&found, state.rip);
    let target = if state_leaves { state.rip } else { other.rip };
    if state
        .merging
        .as_ref()
        .is_some_and(|merging| merging.head != found.head || merging.target != target)
    {
        return Some(other);
    }
    let leaves = if state_leaves {
        condition
    } else {
        pool.bool_not(condition)
    };
    // Only an exit on the length itself, as a loop over what was read leaves where its index
    // reaches the end: a loop that leaves on what it computed, as a print loop on how many
    // digits its number has, keeps its own runs, which the output's checks follow.
    if pool.interval_subject(leaves) != state.stdin_length {
        return Some(other);
    }
    let mut leaving = if state_leaves {
        std::mem::replace(state, other)
    } else {
        other
    };
    leaving.merging = None;
    let yes = pool.boolean(true);
    let mut merging = state.merging.take().unwrap_or_else(|| {
        Box::new(Merging {
            head: found.head,
            latch: found.latch,
            target,
            alive: yes,
            parked: None,
        })
    });
    let alive = merging.alive;
    merging.parked = Some(Rc::new(Parked {
        leaves,
        alive,
        state: leaving,
        before: merging.parked.take(),
    }));
    let stays = pool.bool_not(leaves);
    merging.alive = pool.bool_and(alive, stays);
    state.merging = Some(merging);
    None
}

/// Whether `a` and `b` have done the same, so they differ only in what they hold: the same
/// place, calls, owners, output, input read and everything else a run does.
fn mergeable(a: &State, b: &State) -> bool {
    let same_stream = |x: &Rc<Vec<Term>>, y: &Rc<Vec<Term>>| Rc::ptr_eq(x, y) || x[..] == y[..];
    a.rip == b.rip
        && a.end.is_none()
        && b.end.is_none()
        && a.returns == b.returns
        && a.owners == b.owners
        && a.pointers == b.pointers
        && same_stream(&a.stdout, &b.stdout)
        && same_stream(&a.stderr, &b.stderr)
        && a.stdin_read == b.stdin_read
        && a.arrivals == b.arrivals
        && a.calls == b.calls
        && a.threads == b.threads
        && a.random_used == b.random_used
        && a.readings_used == b.readings_used
        && a.fill_cut == b.fill_cut
        && a.raw_cuts == b.raw_cuts
        && a.written.len() == b.written.len()
        && a.open.len() == b.open.len()
        && a.suspended.is_empty()
        && b.suspended.is_empty()
        && (Rc::ptr_eq(&a.logs, &b.logs) || (a.logs.is_empty() && b.logs.is_empty()))
        // A loop that stored nothing between its exits, as a loop that counts what it reads.
        && a.memory.pages.len() == b.memory.pages.len()
        && a
            .memory
            .pages
            .iter()
            .zip(&b.memory.pages)
            .all(|((page, bytes), (other, theirs))| page == other && Rc::ptr_eq(bytes, theirs))
}

/// `a` where `leaves` holds and `b` elsewhere, as one run, for two runs that have done the
/// same ([`mergeable`]): flags, vectors, masks, memory and, where `registers`, registers become
/// choices. `None` where one holds a byte of memory the other never wrote.
fn merged(pool: &mut Pool, leaves: Term, a: &State, b: State, registers: bool) -> Option<State> {
    let mut merged = b;
    if registers {
        for slot in 0..SLOTS {
            if a.registers[slot] != merged.registers[slot] {
                merged.registers[slot] =
                    pool.ite(leaves, a.registers[slot], merged.registers[slot]);
            }
        }
    }
    for (flag, from) in [
        (&mut merged.carry, a.carry),
        (&mut merged.zero, a.zero),
        (&mut merged.sign, a.sign),
        (&mut merged.overflow, a.overflow),
        (&mut merged.parity, a.parity),
    ] {
        if *flag != from {
            *flag = pool.ite(leaves, from, *flag);
        }
    }
    merged.last = Last::Other;
    for (number, vector) in a.vectors.iter().enumerate() {
        if !Rc::ptr_eq(vector, &merged.vectors[number]) && vector[..] != merged.vectors[number][..]
        {
            let mut bytes = *merged.vectors[number];
            for (byte, from) in bytes.iter_mut().zip(vector.iter()) {
                if *byte != *from {
                    *byte = pool.ite(leaves, *from, *byte);
                }
            }
            merged.vectors[number] = Rc::new(bytes);
        }
    }
    for (mask, from) in merged.masks.iter_mut().zip(a.masks) {
        if *mask != from {
            *mask = pool.ite(leaves, from, *mask);
        }
    }
    let pages: BTreeSet<u64> = a
        .memory
        .pages
        .keys()
        .chain(merged.memory.pages.keys())
        .copied()
        .collect();
    for page in pages {
        let (from, to) = (a.memory.pages.get(&page), merged.memory.pages.get(&page));
        match (from, to) {
            (Some(from), Some(to)) if Rc::ptr_eq(from, to) => {}
            (Some(from), Some(to)) => {
                let mut bytes = **to;
                for (byte, earlier) in bytes.iter_mut().zip(from.iter()) {
                    match (*earlier, *byte) {
                        (Some(x), Some(y)) if x != y => *byte = Some(pool.ite(leaves, x, y)),
                        (Some(_), Some(_)) | (None, None) => {}
                        _ => return None,
                    }
                }
                merged.memory.pages.insert(page, Rc::new(bytes));
            }
            _ => return None,
        }
    }
    merged.stored.clear();
    merged.steps = merged.steps.max(a.steps);
    Some(merged)
}

/// The runs that left a loop, `exits`, first first, and `last`, the one that stayed or left
/// last, as one run: for each input, the one that left first. A count that only grew between
/// exits, as a loop's count of what it read does, is one count of each part where the loop
/// hadn't left before the part was added, in one pass; anything else a register or flag holds is
/// a choice by exit.
fn merge_all(pool: &mut Pool, exits: &[Rc<Parked>], last: &State, alive: Term) -> Option<State> {
    // What the runs have done must be the same: only what they hold differs.
    if !exits.iter().all(|exit| mergeable(&exit.state, last)) {
        return None;
    }
    let mut merged_state = last.clone();
    // Whether the loop was still running after each exit's test: the next exit's, or `alive`.
    let after: Vec<Term> = exits
        .iter()
        .skip(1)
        .map(|exit| exit.alive)
        .chain([alive])
        .collect();
    for slot in 0..SLOTS {
        let values: Vec<Term> = exits
            .iter()
            .map(|exit| exit.state.registers[slot])
            .chain([last.registers[slot]])
            .collect();
        if values.iter().all(|value| *value == values[0]) {
            continue;
        }
        merged_state.registers[slot] = match grown_count(pool, &values, &after, last.stdin_length) {
            Some(count) => count,
            None => chosen_by_exit(pool, &values, exits, slot),
        };
    }
    // Flags, vectors, masks and memory, a choice by exit, where there are few enough exits.
    if exits.len() > MAX_CHOICES_MERGED {
        let count = pool.len();
        let fresh = |pool: &mut Pool, name: &str, width: u32| {
            pool.var(width, &format!("merged{count}.{name}"))
        };
        let differs =
            |pick: fn(&State) -> Term| exits.iter().any(|exit| pick(&exit.state) != pick(last));
        if differs(|state| state.carry) {
            merged_state.carry = pool.bool_var(&format!("merged{count}.cf"));
        }
        if differs(|state| state.zero) {
            merged_state.zero = pool.bool_var(&format!("merged{count}.zf"));
        }
        if differs(|state| state.sign) {
            merged_state.sign = pool.bool_var(&format!("merged{count}.sf"));
        }
        if differs(|state| state.overflow) {
            merged_state.overflow = pool.bool_var(&format!("merged{count}.of"));
        }
        if differs(|state| state.parity) {
            merged_state.parity = pool.bool_var(&format!("merged{count}.pf"));
        }
        for number in 0..last.vectors.len() {
            if exits
                .iter()
                .any(|exit| exit.state.vectors[number][..] != last.vectors[number][..])
            {
                let bytes: [Term; VECTOR_BYTES] =
                    std::array::from_fn(|at| fresh(pool, &format!("v{number}.{at}"), 8));
                merged_state.vectors[number] = Rc::new(bytes);
            }
        }
        for number in 0..last.masks.len() {
            if exits
                .iter()
                .any(|exit| exit.state.masks[number] != last.masks[number])
            {
                merged_state.masks[number] = fresh(pool, &format!("k{number}"), 64);
            }
        }
        merged_state.last = Last::Other;
        merged_state.stored.clear();
        return Some(merged_state);
    }
    let mut folded = last.clone();
    for exit in exits.iter().rev() {
        folded = merged(pool, exit.leaves, &exit.state, folded, false)?;
    }
    merged_state.carry = folded.carry;
    merged_state.zero = folded.zero;
    merged_state.sign = folded.sign;
    merged_state.overflow = folded.overflow;
    merged_state.parity = folded.parity;
    merged_state.vectors = folded.vectors;
    merged_state.masks = folded.masks;
    merged_state.memory = folded.memory;
    merged_state.last = Last::Other;
    merged_state.stored.clear();
    Some(merged_state)
}

/// The most exits whose values a merge writes as a choice by exit; past it, a value that differs
/// is a fresh one, which stands for any of them: deep choices would be walked down by every
/// rule that reads a choice's sides.
const MAX_CHOICES_MERGED: usize = 64;

/// `values`, a register's at each exit and then the last, as a choice by exit, or, past
/// [`MAX_CHOICES_MERGED`] exits, a fresh value, which stands for any of them.
fn chosen_by_exit(pool: &mut Pool, values: &[Term], exits: &[Rc<Parked>], slot: usize) -> Term {
    if exits.len() > MAX_CHOICES_MERGED {
        let count = pool.len();
        let width = pool.width(values[0]);
        return pool.var(width, &format!("merged{count}.r{slot}"));
    }
    values[..values.len() - 1]
        .iter()
        .zip(exits)
        .rev()
        .fold(values[values.len() - 1], |later, (value, exit)| {
            pool.ite(exit.leaves, *value, later)
        })
}

/// `values`, a register's at each exit and then the last, as one count, when each is a count
/// whose conditions, in their order, start with the one before's: each condition first counted
/// at an exit's successor counts where the loop was running after that exit, `after`. What each
/// condition says of the input's `length` is written as the formula writes it, a comparison of
/// the length with where the part ends.
fn grown_count(
    pool: &mut Pool,
    values: &[Term],
    after: &[Term],
    length: Option<Term>,
) -> Option<Term> {
    let width = pool.width(values[0]);
    // Each exit's parts against the one before's alone, so a loop of many steps keeps two lists
    // at a time, not one for every exit.
    let mut parts = Vec::new();
    let mut before: Option<Vec<Term>> = None;
    for (index, value) in values.iter().enumerate() {
        let mut now = pool.counted_parts(*value)?;
        now.sort_unstable();
        match &before {
            None => parts.extend_from_slice(&now),
            Some(before) => {
                if !now.starts_with(before) {
                    return None;
                }
                for part in &now[before.len()..] {
                    parts.push(pool.bool_and(*part, after[index - 1]));
                }
            }
        }
        before = Some(now);
    }
    if let Some(length) = length {
        for part in &mut parts {
            *part = pool.on_length(*part, length);
        }
    }
    ((parts.len() as u128) < (1_u128 << width.min(127))).then(|| pool.count(width, &parts))
}

/// The run that stayed in a loop whose exits it merged, done with it: where it left for the
/// exits' place, it and every run that left are one run, for each input the one that left first;
/// where it left another way, the runs that left are one run of their own, in `pending`, and it
/// goes on where it's still in the loop. Where the runs can't be merged, each goes on alone,
/// where it left.
fn finish_merging(pool: &mut Pool, state: &mut State, pending: &mut Vec<State>) {
    let Some(merging) = state.merging.take() else {
        return;
    };
    let exits = merging.exits();
    if state.end.is_none() && state.rip == merging.target {
        // The merge keeps the staying run's path and what it must meet.
        if let Some(one) = merge_all(pool, &exits, state, merging.alive) {
            *state = one;
            return;
        }
    } else if let Some((last, earlier)) = exits.split_last()
        && let Some(mut one) = merge_all(pool, earlier, &last.state, last.alive)
    {
        let left = pool.bool_not(merging.alive);
        one.path.push(left);
        pending.push(one);
        if pool.truth(merging.alive) != Some(true) {
            state.path.push(merging.alive);
        }
        return;
    }
    // Each where it left, as a run of its own.
    for exit in &exits {
        let mut alone = exit.state.clone();
        let here = pool.bool_and(exit.alive, exit.leaves);
        alone.path.push(here);
        pending.push(alone);
    }
    if pool.truth(merging.alive) != Some(true) {
        state.path.push(merging.alive);
    }
}

/// Every run from `start`, splitting at branches the inputs decide, at most `max_runs`.
///
/// # Errors
///
/// Fails when the oracle does.
pub fn explore(
    image: &Image,
    start: State,
    pool: &mut Pool,
    oracle: &mut dyn Oracle,
    max_runs: usize,
) -> Result<Vec<State>, String> {
    // The states poll comes back in are this exploration's own.
    let mut start = start;
    start.polled = Rc::default();
    let mut pending = vec![start];
    let mut finished = Vec::new();
    let mut looked = pool.len();
    while let Some(mut state) = pending.pop() {
        if finished.len() + pending.len() > max_runs {
            state.end = Some(End::TooManyPaths);
            finished.push(state);
            break;
        }
        loop {
            // A run done going round a loop whose exits it merged, or ending in it: the exits
            // and it, one run where they can be.
            if state.merging.as_ref().is_some_and(|merging| {
                state.end.is_some() || !(merging.head..=merging.latch).contains(&state.rip)
            }) {
                finish_merging(pool, &mut state, &mut pending);
            }
            if state.end.is_some() {
                finished.push(state);
                break;
            }
            if state.steps.is_multiple_of(STEPS_BETWEEN_TIME_CHECKS) {
                oracle.in_time()?;
            }
            if pool.len() >= looked + super::term::TERMS_BETWEEN_MEMORY_LOOKS {
                looked = pool.len();
                oracle.has_room()?;
            }
            let obligations = state.obligations.len();
            let stepped = step(image, &mut state, pool);
            // A step can make terms by the million in one extraction, between two looks of this
            // loop's: the pool looks as it grows, and what it made after finding the memory
            // short is nothing to go on.
            if pool.out_of_room() {
                return Err(super::watch::OUT_OF_MEMORY.into());
            }
            // While a loop's exits are merged, the run that stays must meet what it meets only
            // where it's still in the loop.
            if let Some(alive) = state.merging.as_ref().map(|merging| merging.alive)
                && pool.truth(alive) != Some(true)
            {
                for (condition, _) in &mut state.obligations[obligations..] {
                    *condition = pool.bool_and(alive, *condition);
                }
            }
            match stepped {
                Ok(Step::Next) => {}
                Ok(Step::Split(condition, other)) => {
                    let mut other = *other;
                    let opposite = pool.bool_not(condition);
                    // A run's conditions can always be met together, so when one side of a
                    // branch is impossible the other is certain, and needs no question.
                    let yes = oracle.possible(pool, &state.path, condition)?;
                    let no = !yes || oracle.possible(pool, &other.path, opposite)?;
                    // A loop's exit the input's length decides waits to be merged.
                    if yes && no {
                        match park(image, pool, &mut state, other, condition) {
                            None => continue,
                            Some(back) => other = back,
                        }
                    }
                    // Where only the other side can happen, it carries on the loop's merging.
                    if !yes {
                        other.merging = state.merging.take();
                    }
                    if no {
                        other.path.push(opposite);
                        pending.push(other);
                    }
                    if yes {
                        state.path.push(condition);
                    } else {
                        break;
                    }
                }
                Ok(Step::Choose(choices)) => {
                    for (condition, mut choice) in choices {
                        let certain = pool.truth(condition) == Some(true);
                        if certain || oracle.possible(pool, &choice.path, condition)? {
                            if !certain {
                                choice.path.push(condition);
                            }
                            pending.push(choice);
                        }
                    }
                    break;
                }
                Err(end) => {
                    // Ended: the loop's top merges any exits it was carrying, then finishes it.
                    state.end = Some(end);
                }
            }
        }
    }
    Ok(finished)
}

/// An oracle for concrete runs, where no branch depends on anything unknown.
pub struct Concrete;

impl Oracle for Concrete {
    fn possible(&mut self, _: &Pool, _: &[Term], _: Term) -> Result<bool, String> {
        Err("a concrete run reached a branch that depends on something unknown".into())
    }
}

/// Bytes as constant terms.
#[must_use]
pub fn bytes(pool: &mut Pool, text: &[u8]) -> Vec<Term> {
    text.iter()
        .map(|byte| pool.bv(8, u128::from(*byte)))
        .collect()
}

/// Whether `at` is stack below the program's arguments, which holds what the program wrote
/// there and nothing before.
fn unwritten_stack(at: u64) -> bool {
    (STACK_TOP - STACK_SIZE..STACK_TOP - 8).contains(&at)
}

mod aarch64;
mod bits;
mod calls;
mod edge;
mod fill;
pub mod induction;
mod logs;
pub mod net;
mod vector;

// It runs a harness of x86-64 instructions on this computer's CPU: a Linux program, so on Linux.
#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod against_cpu;

/// Whether a name of these ASCII bytes has the shape of a Windows short name
/// (`crate::names::short_name_shape`): a stem of at most eight ending in `~` and digits,
/// then, if any, one dot and one to three more. The name's length is known, so each place
/// its dot and its `~` can be is a case.
fn short_name_shape(pool: &mut Pool, bytes: &[Term]) -> Term {
    let length = bytes.len();
    if length > crate::names::SHORT_STEM + 1 + crate::names::SHORT_EXTENSION {
        return pool.boolean(false);
    }
    let dot = pool.bv(8, u128::from(b'.'));
    let tilde = pool.bv(8, u128::from(b'~'));
    let zero = pool.bv(8, u128::from(b'0'));
    let nine = pool.bv(8, u128::from(b'9'));
    let mut cases = Vec::new();
    // Where the stem ends: at the name's end, with no dot, or at its one dot.
    for stem in 1..=length.min(crate::names::SHORT_STEM) {
        let extension = length - stem;
        if extension == 1 || extension > crate::names::SHORT_EXTENSION + 1 {
            continue;
        }
        let mut dots = Vec::new();
        for (at, byte) in bytes.iter().enumerate() {
            let is_dot = pool.eq(*byte, dot);
            dots.push(if at == stem {
                is_dot
            } else {
                pool.bool_not(is_dot)
            });
        }
        let mut tildes = Vec::new();
        for at in 0..stem.saturating_sub(1) {
            let mut parts = vec![pool.eq(bytes[at], tilde)];
            for byte in &bytes[at + 1..stem] {
                let low = pool.ule(zero, *byte);
                let high = pool.ule(*byte, nine);
                parts.push(pool.bool_and(low, high));
            }
            tildes.push(pool.all(&parts));
        }
        let separated = pool.all(&dots);
        let ended = pool.any(&tildes);
        cases.push(pool.bool_and(separated, ended));
    }
    pool.any(&cases)
}

// It runs a harness of AArch64 instructions on an AArch64 CPU: this computer's, or QEMU's.
#[cfg(test)]
mod against_a64;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::Profile;
    use crate::targets::PerCpu;
    use crate::toolchain::Toolchain;

    /// The proof's short-name shape, on names whose bytes are known, gives the run's answer
    /// (`crate::names::short_name_shape`) for every name of up to thirteen bytes it's tried on.
    #[test]
    fn the_short_name_shape_the_proof_tests_is_the_runs() {
        let mut names = vec![
            "REPORT~1.TXT",
            "progra~1",
            "a~12",
            "~1",
            "AB12~1.tx",
            "backup~.txt",
            "notes~old.txt",
            "a~1.text",
            "a~1.b.c",
            "a1~",
            "report1.txt",
            "ABCDEFG~1.T",
            "ABCDEF~1.TXT",
            "A~1.",
            "A~1.B",
            ".txt",
            "~",
            "~~1",
            "a~1~2",
            "x.y~1",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        // Every arrangement of a few telling bytes, up to five long.
        let alphabet = *b"a~1.";
        for length in 1..=5 {
            for mut code in 0..alphabet.len().pow(length) {
                let mut name = String::new();
                for _ in 0..length {
                    name.push(char::from(alphabet[code % alphabet.len()]));
                    code /= alphabet.len();
                }
                names.push(name);
            }
        }
        for name in names {
            let mut pool = Pool::default();
            let bytes: Vec<Term> = name
                .bytes()
                .map(|byte| pool.bv(8, u128::from(byte)))
                .collect();
            let shape = short_name_shape(&mut pool, &bytes);
            assert_eq!(
                pool.truth(shape),
                Some(crate::names::short_name_shape(&name)),
                "{name}"
            );
        }
    }

    /// Builds `source` for `profile`'s CPU and loads it.
    async fn image_for(profile: Profile, source: &str) -> Image {
        image_with_sve(profile, source, 0).await
    }

    /// Builds `source` for `profile`'s CPU and loads it, on a CPU whose vectors are `sve_bytes`
    /// long (0 for one without SVE).
    async fn image_with_sve(profile: Profile, source: &str, sve_bytes: u16) -> Image {
        let toolchain = Toolchain::discover().await.expect("llvm-mc and ld.lld");
        let folder = tempfile::tempdir().expect("temporary folder");
        let (assembly, object, executable) = (
            folder.path().join("main.s"),
            folder.path().join("main.o"),
            folder.path().join("main"),
        );
        std::fs::write(&assembly, source).expect("write");
        toolchain
            .assemble(profile, &assembly, &object)
            .await
            .expect("assemble");
        toolchain
            .link(profile, &object, &executable)
            .await
            .expect("link");
        let bytes = std::fs::read(&executable).expect("read");
        let mut machine = crate::checker::decode::read(&bytes).expect("decode");
        machine.sve_bytes = sve_bytes;
        Image::new(&machine).expect("an entry point")
    }

    /// Runs `image` on concrete arguments and stdin: its stdout, stderr and exit status.
    fn run(image: &Image, args: &[&str], stdin: &str) -> (String, String, Option<u128>) {
        let mut pool = Pool::default();
        let args: Vec<Vec<Term>> = args
            .iter()
            .map(|arg| bytes(&mut pool, arg.as_bytes()))
            .collect();
        let stdin = bytes(&mut pool, stdin.as_bytes());
        let start = State::start(image, &mut pool, &args, &[], stdin);
        let mut runs = explore(image, start, &mut pool, &mut Concrete, 4).expect("concrete");
        assert_eq!(runs.len(), 1);
        let state = runs.remove(0);
        let text = |terms: &[Term]| {
            terms
                .iter()
                .map(|term| {
                    char::from(u8::try_from(pool.constant(*term).expect("a byte")).expect("a byte"))
                })
                .collect::<String>()
        };
        let status = match state.end {
            Some(End::Exit(status)) => pool.constant(status),
            other => panic!("ended with {other:?}"),
        };
        (text(&state.stdout), text(&state.stderr), status)
    }

    /// A file of this repository, by its path from the top.
    fn repository_text(path: &str) -> String {
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"))
    }

    /// The line counter on each target: twins from the very same idea.
    const LINES: PerCpu<&str> = PerCpu {
        x86_64: "tests/fixtures/lines/main.s",
        aarch64: "tests/fixtures/linux-aarch64/lines/main.s",
    };

    /// The counters of the line counter's five loops, sorted, on each target: the main loop over
    /// buffer-fulls, the count over a buffer-full's chunks, fill's reads, and the digits and the
    /// writes. x86-64 keeps them in rbx, r8 and r9; AArch64 in x19, x11, x12, x3 and x10, and
    /// ends each loop with `subs` and `b.ne` where x86-64 has `dec` and `jnz`.
    const LOOP_COUNTERS: PerCpu<[usize; 5]> = PerCpu {
        x86_64: [3, 8, 8, 8, 9],
        aarch64: [3, 10, 11, 12, 19],
    };

    /// The counter of fill's read loop on each target.
    const FILL_COUNTER: PerCpu<usize> = PerCpu {
        x86_64: 9,
        aarch64: 12,
    };

    #[tokio::test]
    async fn every_bounded_loop_is_found_with_its_counter() {
        for target in Profile::ALL {
            let source = repository_text(LINES.get(target));
            let image = image_for(target, &source).await;
            let counters: Vec<usize> = image.loops().map(|found| found.counter).collect();
            let mut sorted = counters.clone();
            sorted.sort_unstable();
            assert_eq!(
                sorted,
                LOOP_COUNTERS.get(target).to_vec(),
                "{}: {:?}",
                target.name(),
                image.loops
            );
            for found in image.loops() {
                match target {
                    Profile::LinuxX86_64 => {
                        let latch = image.instructions[&found.latch];
                        assert_eq!(latch.mnemonic(), Mnemonic::Jne);
                        assert_eq!(latch.near_branch_target(), found.head);
                        assert_eq!(latch.next_ip(), found.exit);
                    }
                    Profile::LinuxAarch64 => {
                        // `b.ne` or `cbnz`, four bytes long.
                        let latch = image.words[&found.latch];
                        let back = match latch {
                            a64::Inst::BranchCond { target: back, .. }
                            | a64::Inst::CompareBranch { target: back, .. } => back,
                            other => panic!("a latch that is {other:?}"),
                        };
                        assert_eq!(back, found.head);
                        assert_eq!(found.exit, found.latch + 4);
                    }
                }
                assert!(found.head < found.latch);
            }
            assert_eq!(
                image
                    .loop_at(image.fill_head().expect("fill"))
                    .map(|found| found.counter),
                Some(*FILL_COUNTER.get(target)),
                "{}",
                target.name()
            );
        }
    }

    /// A program that sends `ok` on its first connection, prints what comes back, and exits
    /// with the number its write returned less 2: 0 when the write went through. One for each
    /// target, with that CPU's system calls: write 1 and read 0 and exit 60 on x86-64, write 64
    /// and read 63 and exit 93 on AArch64.
    const SENDS_ON_ITS_CONNECTION: PerCpu<&str> = PerCpu {
        x86_64: ".intel_syntax noprefix\n.globl _start\n.text\n\
            .type _start, @function\n_start:\n    mov eax, 1\n    mov edi, 4\n    \
            lea rsi, [rip + ok]\n    mov edx, 2\n    syscall\n    lea r12, [rax - 2]\n    \
            xor eax, eax\n    mov edi, 4\n    lea rsi, [rip + buffer]\n    mov edx, 64\n    \
            syscall\n    mov rdx, rax\n    mov eax, 1\n    mov edi, 1\n    \
            lea rsi, [rip + buffer]\n    syscall\n    mov rdi, r12\n    mov eax, 60\n    \
            syscall\n.size _start, . - _start\n.data\n.type ok, @object\nok:\n    .ascii \"ok\"\n\
            .size ok, . - ok\n.bss\n.type buffer, @object\nbuffer:\n    .zero 64\n\
            .size buffer, . - buffer\n",
        aarch64: ".arch armv8-a\n.globl _start\n.text\n\
            .type _start, @function\n_start:\n    mov x0, #4\n    adrp x1, ok\n    \
            add x1, x1, :lo12:ok\n    mov x2, #2\n    mov x8, #64\n    svc #0\n    \
            sub x19, x0, #2\n    mov x0, #4\n    adrp x1, buffer\n    add x1, x1, :lo12:buffer\n    \
            mov x2, #64\n    mov x8, #63\n    svc #0\n    mov x2, x0\n    mov x0, #1\n    \
            adrp x1, buffer\n    add x1, x1, :lo12:buffer\n    mov x8, #64\n    svc #0\n    \
            mov x0, x19\n    mov x8, #93\n    svc #0\n.size _start, . - _start\n.data\n\
            .type ok, @object\nok:\n    .ascii \"ok\"\n.size ok, . - ok\n.bss\n\
            .type buffer, @object\nbuffer:\n    .zero 64\n.size buffer, . - buffer\n",
    };

    #[tokio::test]
    async fn the_connections_lengua_makes_are_open_at_the_start() {
        for target in Profile::ALL {
            connections_are_open_at_the_start(target, SENDS_ON_ITS_CONNECTION.get(target)).await;
        }
    }

    /// The connections lengua makes are open at the start of `target`'s `source`: it writes
    /// to the first and reads what the test's server answers; handed over ended, the write fails.
    async fn connections_are_open_at_the_start(target: Profile, source: &str) {
        let image = image_for(target, source).await;
        let mut pool = Pool::default();
        let text = |pool: &Pool, terms: &[Term]| {
            terms
                .iter()
                .map(|term| {
                    char::from(u8::try_from(pool.constant(*term).expect("a byte")).expect("a byte"))
                })
                .collect::<String>()
        };
        // Answered by the test's server: it takes `ok`, then replies `hello`.
        let ok = bytes(&mut pool, b"ok");
        let hello = bytes(&mut pool, b"hello");
        let network = net::Network {
            server: vec![(ok, vec![hello])],
            made: vec![Some(0)],
            ..net::Network::default()
        };
        let start = State::start(&image, &mut pool, &[], &[], Vec::new()).with_network(network);
        let mut runs = explore(&image, start, &mut pool, &mut Concrete, 4).expect("concrete");
        assert_eq!(runs.len(), 1, "{}", target.name());
        let answered = runs.remove(0);
        assert_eq!(text(&pool, &answered.stdout), "hello", "{}", target.name());
        assert_eq!(
            text(&pool, &answered.connections[0].sent),
            "ok",
            "{}",
            target.name()
        );
        assert!(
            matches!(answered.end, Some(End::Exit(status)) if pool.constant(status) == Some(0)),
            "{}: {:?}",
            target.name(),
            answered.end
        );
        // Handed over ended: the write fails with EPIPE, so it exits with -34 & 255, and the
        // read gives nothing.
        let network = net::Network {
            made: vec![None],
            ..net::Network::default()
        };
        let start = State::start(&image, &mut pool, &[], &[], Vec::new()).with_network(network);
        let mut runs = explore(&image, start, &mut pool, &mut Concrete, 4).expect("concrete");
        let ended = runs.remove(0);
        assert_eq!(text(&pool, &ended.stdout), "", "{}", target.name());
        assert!(ended.connections[0].sent.is_empty(), "{}", target.name());
        assert!(
            matches!(ended.end, Some(End::Exit(status)) if pool.constant(status) == Some(222)),
            "{}: {:?}",
            target.name(),
            ended.end
        );
    }

    /// A program for each target that exits with the carries the CPU makes, and the status it
    /// exits with. x86-64: 0xff..ff + 0xff..ff with a carry in (`adc`): the result equals the
    /// first operand, yet it carries out, so the exit status is 1. AArch64's dialect has no
    /// `adc` or `sbc` (a gap todo.md lists, not one of the CPU), so its cell is the carries that
    /// `adds` and `subs` make, read back with `cset hs`: the same all-ones number added to itself
    /// carries out (1), and taken from itself it doesn't borrow (1, shifted left by one to 2),
    /// so the exit status is 3.
    const CARRIES: PerCpu<(&str, u128)> = PerCpu {
        x86_64: (
            ".intel_syntax noprefix\n.globl _start\n.text\n.type _start, @function\n\
             _start:\n    mov rax, -1\n    mov rbx, -1\n    stc\n    adc rax, rbx\n    \
             setb dil\n    movzx edi, dil\n    mov eax, 60\n    syscall\n\
             .size _start, . - _start\n",
            1,
        ),
        aarch64: (
            ".arch armv8-a\n.globl _start\n.text\n.type _start, @function\n\
             _start:\n    mov x0, #-1\n    mov x1, #-1\n    adds x2, x0, x1\n    cset w3, hs\n    \
             subs x4, x0, x1\n    cset w5, hs\n    add w0, w3, w5, lsl #1\n    mov x8, #93\n    \
             svc #0\n.size _start, . - _start\n",
            3,
        ),
    };

    #[tokio::test]
    async fn carries_like_the_cpu() {
        for target in Profile::ALL {
            let (source, expected) = *CARRIES.get(target);
            let image = image_for(target, source).await;
            let mut pool = Pool::default();
            let start = State::start(&image, &mut pool, &[], &[], Vec::new());
            let runs = explore(&image, start, &mut pool, &mut Concrete, 2).expect("runs");
            let status = match &runs[0].end {
                Some(End::Exit(status)) => pool.constant(*status),
                other => panic!("{} ended with {other:?}", target.name()),
            };
            assert_eq!(status, Some(expected), "{}", target.name());
        }
    }

    /// A program for each target that puts 7 in a vector register, makes a system call that fails
    /// (`close` of descriptor -1), and exits with what the vector state still says; the vector
    /// length it runs on, in bytes (0 without SVE); and the status it exits with. x86-64: the call
    /// keeps xmm0, so the 7 comes back. AArch64: Linux keeps the V registers (the low 128 bits of
    /// each Z) and clears the rest of the SVE state, so with 32-byte vectors the predicate set
    /// before the call has no lane active (0), and z1 still holds 7 in 16 lanes only (16).
    const VECTORS_ACROSS_A_CALL: PerCpu<(&str, u16, u128)> = PerCpu {
        x86_64: (
            ".intel_syntax noprefix\n.globl _start\n.text\n.type _start, @function\n\
             _start:\n    mov eax, 7\n    movd xmm0, eax\n    mov rdi, -1\n    mov eax, 3\n    \
             syscall\n    movd edi, xmm0\n    mov eax, 60\n    syscall\n\
             .size _start, . - _start\n",
            0,
            7,
        ),
        aarch64: (
            ".arch armv8-a\n.arch_extension sve\n.globl _start\n.text\n\
             .type _start, @function\n_start:\n    ptrue p0.b\n    mov z1.b, #7\n    \
             mov x0, #-1\n    mov x8, #57\n    svc #0\n    cntp x2, p0, p0.b\n    ptrue p1.b\n    \
             cmpeq p2.b, p1/z, z1.b, #7\n    cntp x3, p1, p2.b\n    add x0, x3, x2\n    \
             mov x8, #93\n    svc #0\n.size _start, . - _start\n",
            32,
            16,
        ),
    };

    #[tokio::test]
    async fn vector_state_across_a_system_call() {
        for target in Profile::ALL {
            let (source, sve_bytes, expected) = *VECTORS_ACROSS_A_CALL.get(target);
            let image = image_with_sve(target, source, sve_bytes).await;
            let mut pool = Pool::default();
            let start = State::start(&image, &mut pool, &[], &[], Vec::new());
            let runs = explore(&image, start, &mut pool, &mut Concrete, 2).expect("runs");
            let status = match &runs[0].end {
                Some(End::Exit(status)) => pool.constant(*status),
                other => panic!("{} ended with {other:?}", target.name()),
            };
            assert_eq!(status, Some(expected), "{}", target.name());
        }
    }

    /// An oracle with no branch to decide, which says the proof's time is up the fourth time it's
    /// asked.
    struct TimeUpAtTheFourthLook(std::cell::Cell<u32>);

    impl Oracle for TimeUpAtTheFourthLook {
        fn possible(&mut self, _: &Pool, _: &[Term], _: Term) -> Result<bool, String> {
            Err("a concrete run reached a branch that depends on something unknown".into())
        }

        fn in_time(&self) -> Result<(), String> {
            self.0.set(self.0.get() + 1);
            if self.0.get() > 3 {
                Err("time is up".into())
            } else {
                Ok(())
            }
        }
    }

    /// A loop of 100,000 times round with nothing to decide, on each target: `dec` and `jnz` on
    /// x86-64, `subs` and `b.ne` on AArch64.
    const HUNDRED_THOUSAND_ROUNDS: PerCpu<&str> = PerCpu {
        x86_64: ".intel_syntax noprefix\n.globl _start\n.text\n.type _start, @function\n\
                      _start:\n    mov rcx, 100000\n.Lround:\n    dec rcx\n    jnz .Lround\n    \
                      mov eax, 60\n    xor edi, edi\n    syscall\n.size _start, . - _start\n",
        aarch64: ".arch armv8-a\n.globl _start\n.text\n.type _start, @function\n\
                      _start:\n    mov x1, #0x86a0\n    movk x1, #1, lsl #16\n.Lround:\n    \
                      subs x1, x1, #1\n    b.ne .Lround\n    mov x8, #93\n    mov x0, #0\n    \
                      svc #0\n.size _start, . - _start\n",
    };

    /// A run of 100,000 times round a loop ends with the proof's time, though it never asks the
    /// oracle about a branch: this is how a hash of the input, all straight-line code over
    /// unknown values, stays within the proof's time.
    #[tokio::test]
    async fn a_run_with_no_branch_on_its_inputs_keeps_the_proofs_time() {
        for target in Profile::ALL {
            let image = image_for(target, HUNDRED_THOUSAND_ROUNDS.get(target)).await;
            let mut pool = Pool::default();
            let start = State::start(&image, &mut pool, &[], &[], Vec::new());
            let mut oracle = TimeUpAtTheFourthLook(std::cell::Cell::new(0));
            let result = explore(&image, start, &mut pool, &mut oracle, 2);
            assert_eq!(
                result.err().as_deref(),
                Some("time is up"),
                "{}",
                target.name()
            );
            assert_eq!(oracle.0.get(), 4, "{}", target.name());
        }
    }

    /// An oracle with no branch to decide, which says the computer has no memory left the first
    /// time it's asked.
    struct NoRoomAtTheFirstLook(std::cell::Cell<u32>);

    impl Oracle for NoRoomAtTheFirstLook {
        fn possible(&mut self, _: &Pool, _: &[Term], _: Term) -> Result<bool, String> {
            Err("a concrete run reached a branch that depends on something unknown".into())
        }

        fn has_room(&self) -> Result<(), String> {
            self.0.set(self.0.get() + 1);
            Err("no room".into())
        }
    }

    /// A loop of a million times round with nothing to decide, on each target.
    const A_MILLION_ROUNDS: PerCpu<&str> = PerCpu {
        x86_64: ".intel_syntax noprefix\n.globl _start\n.text\n.type _start, @function\n\
                      _start:\n    mov rcx, 1000000\n.Lround:\n    dec rcx\n    jnz .Lround\n    \
                      mov eax, 60\n    xor edi, edi\n    syscall\n.size _start, . - _start\n",
        aarch64: ".arch armv8-a\n.globl _start\n.text\n.type _start, @function\n\
                      _start:\n    mov x1, #0x4240\n    movk x1, #0xf, lsl #16\n.Lround:\n    \
                      subs x1, x1, #1\n    b.ne .Lround\n    mov x8, #93\n    mov x0, #0\n    \
                      svc #0\n.size _start, . - _start\n",
    };

    /// A run that makes a new number every step ends with the computer's memory, though it never
    /// asks the oracle about a branch: a hash of the input, all straight-line code over unknown
    /// values, makes terms by the million, and the steps alone don't bound them.
    #[tokio::test]
    async fn a_run_that_fills_the_pool_keeps_the_memory() {
        for target in Profile::ALL {
            let image = image_for(target, A_MILLION_ROUNDS.get(target)).await;
            let mut pool = Pool::default();
            let start = State::start(&image, &mut pool, &[], &[], Vec::new());
            let mut oracle = NoRoomAtTheFirstLook(std::cell::Cell::new(0));
            let result = explore(&image, start, &mut pool, &mut oracle, 2);
            assert_eq!(
                result.err().as_deref(),
                Some("no room"),
                "{}",
                target.name()
            );
            assert_eq!(oracle.0.get(), 1, "{}", target.name());
        }
    }

    /// A program that exits at once, on each target.
    const EXITS_AT_ONCE: PerCpu<&str> = PerCpu {
        x86_64: ".intel_syntax noprefix\n.globl _start\n.text\n.type _start, @function\n\
                      _start:\n    mov eax, 60\n    xor edi, edi\n    syscall\n\
                      .size _start, . - _start\n",
        aarch64: ".arch armv8-a\n.globl _start\n.text\n.type _start, @function\n\
                      _start:\n    mov x8, #93\n    mov x0, #0\n    svc #0\n\
                      .size _start, . - _start\n",
    };

    /// A run on a pool that found the memory short ends at its next step, though the oracle
    /// never has to ask: one step can make the terms by the million in a single extraction, far
    /// between two looks of the loop's, and what the pool made after that is nothing to go on.
    #[tokio::test]
    async fn a_run_on_a_pool_that_found_the_memory_short_ends_at_its_next_step() {
        for target in Profile::ALL {
            let image = image_for(target, EXITS_AT_ONCE.get(target)).await;
            let _no_memory = crate::prove::watch::NoMemoryForTest::pretend();
            let mut pool = Pool::default();
            let start = State::start(&image, &mut pool, &[], &[], Vec::new());
            let one = pool.bv(32, 1);
            while !pool.out_of_room() {
                let next = pool.var(32, &format!("b{}", pool.len()));
                let _ = pool.add(next, one);
            }
            let result = explore(&image, start, &mut pool, &mut Concrete, 2);
            let why = result.err();
            assert_eq!(
                why.as_deref(),
                Some(crate::prove::watch::OUT_OF_MEMORY),
                "{}",
                target.name()
            );
        }
    }

    /// A program that calls `fill` on standard input with a buffer of `capacity` bytes and the
    /// end byte `end` (-1 for none), prints what it read and exits with fill's result.
    fn fill_program(capacity: u32, end: i32) -> String {
        format!(
            ".intel_syntax noprefix\n.text\n.type main, @function\nmain:\n    sub rsp, 8\n    \
             xor edi, edi\n    lea rsi, [rip + buffer]\n    mov edx, {capacity}\n    \
             mov ecx, {end}\n    call fill\n    mov rbx, rax\n    test rax, rax\n    \
             js .Lmain_exit\n    mov rdx, rax\n    mov edi, 1\n    lea rsi, [rip + buffer]\n    \
             call os_write\n.Lmain_exit:\n    mov rdi, rbx\n    call os_exit\n\
             .size main, . - main\n.bss\n.type buffer, @object\nbuffer:\n    .zero 16\n\
             .size buffer, 16\n"
        )
    }

    /// [`fill_program`] for AArch64: the same program in its instructions.
    fn fill_program_aarch64(capacity: u32, end: i32) -> String {
        format!(
            ".arch armv8-a\n.text\n.type main, @function\nmain:\n    mov x0, #0\n    \
             adrp x1, buffer\n    add x1, x1, :lo12:buffer\n    mov x2, #{capacity}\n    \
             mov w3, #{end}\n    bl fill\n    mov x19, x0\n    tbnz x0, #63, .Lmain_exit\n    \
             mov x2, x0\n    mov x0, #1\n    adrp x1, buffer\n    add x1, x1, :lo12:buffer\n    \
             bl os_write\n.Lmain_exit:\n    mov x0, x19\n    bl os_exit\n.size main, . - main\n\
             .bss\n.type buffer, @object\nbuffer:\n    .zero 16\n.size buffer, 16\n"
        )
    }

    /// Every way `length` bytes can arrive: each list of piece lengths that adds up to it.
    fn every_cut_of(length: usize) -> Vec<Vec<usize>> {
        if length == 0 {
            return vec![Vec::new()];
        }
        (0..1_u32 << (length - 1))
            .map(|cuts| {
                let mut pieces = Vec::new();
                let mut size = 1;
                for position in 0..length - 1 {
                    if cuts >> position & 1 == 1 {
                        pieces.push(size);
                        size = 1;
                    } else {
                        size += 1;
                    }
                }
                pieces.push(size);
                pieces
            })
            .collect()
    }

    /// What `image` prints and exits with given `input` on standard input: run as its code
    /// runs, with the input arriving in `pieces`, or, without pieces, with fill read as a whole
    /// for every way the input can be cut. Each outcome once.
    fn outcomes(
        image: &Image,
        input: &[u8],
        pieces: Option<&[usize]>,
    ) -> std::collections::BTreeSet<(Vec<u8>, String)> {
        let mut pool = Pool::default();
        let stdin = bytes(&mut pool, input);
        let start = State::start(image, &mut pool, &[], &[], stdin);
        let start = match pieces {
            Some(pieces) => start.with_stdin_pieces(pieces.to_vec()),
            None => start.for_every_cut(),
        };
        let runs = explore(image, start, &mut pool, &mut Concrete, 64).expect("concrete runs");
        runs.iter()
            .map(|run| {
                let printed = run
                    .stdout
                    .iter()
                    .map(|byte| pool.constant(*byte).expect("a byte") as u8)
                    .collect();
                let ended = match run.end {
                    Some(End::Exit(status)) => {
                        format!("exit {}", pool.constant(status).expect("a status"))
                    }
                    ref other => format!("{other:?}"),
                };
                (printed, ended)
            })
            .collect()
    }

    /// A bug planted in `fill`: what it does, and the edit that plants it.
    type FillMutant = (&'static str, &'static str, &'static str);

    /// One target's cell of the fill test: the program that calls lengua's own `fill`, and four
    /// bugs planted in it.
    #[derive(Clone, Copy)]
    struct FillCell {
        program: fn(u32, i32) -> String,
        mutants: [FillMutant; 4],
    }

    /// The fill test of each target, with the same four bugs planted in each CPU's `fill`.
    const FILLS: PerCpu<FillCell> = PerCpu {
        x86_64: FillCell {
            program: fill_program,
            mutants: [
                (
                    "reads every piece at the buffer's start",
                    "    add rsi, rax\n",
                    "",
                ),
                (
                    "stops a byte short of full",
                    "    sub rdx, rax\n    jz .Llengua_fill_done\n",
                    "    sub rdx, rax\n    cmp rdx, 1\n    jbe .Llengua_fill_done\n",
                ),
                (
                    "checks the byte after the last",
                    "byte ptr [rsi - 1]",
                    "byte ptr [rsi]",
                ),
                (
                    "counts one byte too many",
                    "    mov rax, r8\n    sub rax, rdx\n",
                    "    lea rax, [r8 + 1]\n    sub rax, rdx\n",
                ),
            ],
        },
        aarch64: FillCell {
            program: fill_program_aarch64,
            mutants: [
                (
                    "reads every piece at the buffer's start",
                    "    add x1, x1, x0\n",
                    "",
                ),
                (
                    "stops a byte short of full",
                    "    subs x2, x2, x0\n    b.eq .Llengua_fill_done\n",
                    "    subs x2, x2, x0\n    cmp x2, #1\n    b.ls .Llengua_fill_done\n",
                ),
                (
                    "checks the byte after the last",
                    "ldurb w13, [x1, #-1]",
                    "ldrb w13, [x1]",
                ),
                (
                    "counts one byte too many",
                    "    sub x0, x10, x2\n",
                    "    sub x0, x10, x2\n    add x0, x0, #1\n",
                ),
            ],
        },
    };

    #[tokio::test]
    async fn fill_read_whole_is_its_code_for_every_cut() {
        for target in Profile::ALL {
            let cell = *FILLS.get(target);
            fill_read_whole_is_its_code(target, cell.program, cell.mutants).await;
        }
    }

    /// fill read as a whole gives exactly what its code gives, over every way its input can be
    /// cut: every input of up to 5 bytes of `a` and newlines, every capacity up to 7, with a
    /// newline as the end byte and with none. Bugs planted in lengua's own fill each give
    /// something the whole doesn't. `program` makes the program that calls it.
    async fn fill_read_whole_is_its_code(
        profile: Profile,
        program: fn(u32, i32) -> String,
        mutants: [FillMutant; 4],
    ) {
        let inputs: Vec<Vec<u8>> = (0..=5_u32)
            .flat_map(|length| {
                (0..1_u32 << length).map(move |bits| {
                    (0..length)
                        .map(|at| if bits >> at & 1 == 1 { b'\n' } else { b'a' })
                        .collect()
                })
            })
            .collect();
        let mut caught = [false; 4];
        for capacity in 1..=7 {
            for end in [i32::from(b'\n'), -1] {
                let main = program(capacity, end);
                let whole = image_for(profile, &main).await;
                assert!(whole.has_fill(), "lengua's fill is found");
                // What a build assembles, main.s and lengua.s after it: the bugs go in the fill
                // that lengua.s holds.
                let library = crate::edge::lengua_text(profile, &main).expect("lengua.s for it");
                let linked = format!("{main}\n{library}");
                let mut planted = Vec::new();
                for (_, from, to) in mutants {
                    let mutant = linked.replacen(from, to, 1);
                    assert_ne!(mutant, linked);
                    planted.push(image_for(profile, &mutant).await);
                }
                for input in &inputs {
                    let summary = outcomes(&whole, input, None);
                    let cuts = every_cut_of(input.len());
                    let code: std::collections::BTreeSet<_> = cuts
                        .iter()
                        .flat_map(|pieces| outcomes(&whole, input, Some(pieces)))
                        .collect();
                    assert_eq!(
                        summary, code,
                        "capacity {capacity}, end {end}, input {input:?}"
                    );
                    for (index, mutant) in planted.iter().enumerate() {
                        if caught[index] {
                            continue;
                        }
                        let code: std::collections::BTreeSet<_> = cuts
                            .iter()
                            .flat_map(|pieces| outcomes(mutant, input, Some(pieces)))
                            .collect();
                        caught[index] = code != summary;
                    }
                }
            }
        }
        for (index, (name, ..)) in mutants.iter().enumerate() {
            assert!(caught[index], "a fill that {name} went unnoticed");
        }
    }

    /// The program that sleeps on each target, with `clock_nanosleep` (230 on x86-64, 115 on
    /// AArch64), which a run takes as over at once.
    const NAP: PerCpu<&str> = PerCpu {
        x86_64: "tests/fixtures/nap/main.s",
        aarch64: "tests/fixtures/linux-aarch64/nap/main.s",
    };

    #[tokio::test]
    async fn a_sleep_ends_at_once() {
        for target in Profile::ALL {
            let image = image_for(target, &repository_text(NAP.get(target))).await;
            assert_eq!(
                run(&image, &[], ""),
                ("awake\n".into(), String::new(), Some(0)),
                "{}",
                target.name()
            );
        }
    }

    /// The template of each target, in the compile skill's assets.
    const TEMPLATES: PerCpu<&str> = PerCpu {
        x86_64: "plugin/skills/compile/assets/main-template.s",
        aarch64: "plugin/skills/compile/assets/linux-aarch64/main-template.s",
    };

    #[tokio::test]
    async fn runs_the_template_like_the_cpu() {
        for target in Profile::ALL {
            let image = image_for(target, &repository_text(TEMPLATES.get(target))).await;
            let name = target.name();
            assert_eq!(
                run(&image, &["7"], ""),
                ("-7\n".into(), String::new(), Some(0)),
                "{name}"
            );
            assert_eq!(
                run(&image, &["-9223372036854775807"], ""),
                ("9223372036854775807\n".into(), String::new(), Some(0)),
                "{name}"
            );
            assert_eq!(
                run(&image, &[], ""),
                (String::new(), "usage: negate <n>\n".into(), Some(1)),
                "{name}"
            );
            assert_eq!(
                run(&image, &["12x"], ""),
                (String::new(), "error: not a number\n".into(), Some(2)),
                "{name}"
            );
        }
    }
}
