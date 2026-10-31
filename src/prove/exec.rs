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

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use iced_x86::{ConditionCode, Instruction, Mnemonic, OpKind, Register};

use super::term::{Pool, Term};
use crate::checker::decode::Machine;

/// The address of `argc` when `_start` begins.
pub const STACK_TOP: u64 = 0x7ffd_0000_0000;

/// How far below that the stack may grow.
const STACK_SIZE: u64 = 8 * 1024 * 1024;

/// Room above the stack top for the argument strings and pointers.
const ARGUMENT_ROOM: u64 = 1024 * 1024;

/// The most instructions one run may take before the prover gives up on it.
pub const MAX_STEPS: u64 = 5_000_000;

/// The most bytes of standard input left for which a proof takes every count a read can
/// return: every way 8 bytes can be cut is 128 runs.
const RAW_CUT_BYTES: u64 = 8;

/// The most places an address that depends on the input may be, such as a table's entries for
/// each value of an input byte.
const MAX_PLACES: u128 = 256;

/// The bytes of a vector register: a zmm register's 512 bits.
const VECTOR_BYTES: usize = 64;

/// A range of addresses the program may use.
#[derive(Debug, Clone, Copy)]
struct Region {
    start: u64,
    end: u64,
    writable: bool,
}

/// The program as loaded: its instructions by address, and its memory.
pub struct Image {
    instructions: HashMap<u64, Instruction>,
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
    /// The canonical fill's places, when the program has it.
    fill: Option<fill::Fill>,
    /// Whether some run has called each function, as `functions` lists them: a proof says
    /// nothing of code no run reached.
    entered: Vec<AtomicBool>,
}

impl Image {
    /// Loads the functions and data the decoder read from the executable.
    #[must_use]
    pub fn new(machine: &Machine) -> Option<Self> {
        let entry = machine.functions.get("_start")?.address;
        let mut instructions = HashMap::new();
        let mut functions = Vec::new();
        for (name, function) in &machine.functions {
            for instruction in &function.instructions {
                instructions.insert(instruction.ip(), *instruction);
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
            regions,
            initial,
            entry,
            functions,
            symbols,
            named,
            fill: None,
            entered,
        };
        image.fill = fill::Fill::find(&image, machine);
        Some(image)
    }

    /// Whether the program has the canonical fill, which proofs read as a whole for every way
    /// its input can be cut.
    #[must_use]
    pub fn has_fill(&self) -> bool {
        self.fill.is_some()
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

/// How an instruction passes owners on: the registers it sets, each with its value's owner, at
/// most two as `xchg` sets, and the pointer it stores whole, if any. Every other register it
/// changes holds no pointer the run follows.
#[derive(Debug, Default)]
struct Plan {
    registers: [Option<(usize, Option<Owner>)>; 2],
    store: Option<(u64, Term, Owner)>,
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
    fn apply(&self, state: &mut State, before: &[Term; 16]) {
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
        if let Some((address, value, owner)) = self.store
            && state.stored.get(&address) == Some(&value)
        {
            state.pointers.insert(address, owner);
        }
    }

    /// The same for every state `step` leaves.
    fn apply_all(&self, state: &mut State, step: &mut Step, before: &[Term; 16]) {
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

/// A return address on the stack: where it is, where it returns to, the function called, and
/// the call's number, which names the callee's frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Return {
    slot: u64,
    back: u64,
    callee: u64,
    frame: u64,
}

/// A thread that started another and waits for it: the prover runs the new thread to its end
/// first, since no thread writes what another reads (M7), so the order can't change what either
/// does. What the waiting thread had in its registers, flags and calls, and where it goes on.
#[derive(Debug, Clone)]
struct Suspended {
    registers: [Term; 16],
    last: Last,
    flags: [Term; 5],
    vectors: Vec<Rc<[Term; VECTOR_BYTES]>>,
    masks: [Term; 8],
    rip: u64,
    returns: Vec<Return>,
    owners: [Option<Owner>; 16],
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
    registers: [Term; 16],
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
    /// The pieces standard input arrives in, by length, when it doesn't arrive whole: each read
    /// gets at most what its piece has left.
    stdin_pieces: Rc<Vec<usize>>,
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
    /// Every connection the run made or took, in order.
    pub connections: Vec<net::Connection>,
    /// Bytes written to standard output, shared with the runs a fork made until one writes.
    pub stdout: Rc<Vec<Term>>,
    /// Bytes written to standard error, shared the same way.
    pub stderr: Rc<Vec<Term>>,
    steps: u64,
    /// How the run ended, once it has.
    pub end: Option<End>,
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
    owners: [Option<Owner>; 16],
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
/// Flags that change nothing the model tracks: no controlling terminal, large files, closing on
/// exec.
const O_HARMLESS: u64 = 0o400 | 0o100_000 | 0o2_000_000;
const ENOENT: i64 = 2;
const EEXIST: i64 = 17;
const ENOTDIR: i64 = 20;
const EISDIR: i64 = 21;
const ENOTEMPTY: i64 = 39;
const O_DIRECTORY: u64 = 0o200_000;
const AT_REMOVEDIR: u64 = 0x200;
/// The bytes of `struct stat`, and where its kind and size are.
const STAT_LEN: u64 = 144;
const STAT_MODE: u64 = 24;
const STAT_SIZE: u64 = 48;
/// A regular file that its owner may read and write and others may read.
const REGULAR_FILE: u128 = 0o100_644;
const EBADF: i64 = 9;
const EINVAL: i64 = 22;
const ENOTTY: i64 = 25;
const EAGAIN: i64 = 11;
/// The bytes of one `struct pollfd`, and its event for something to read.
const POLLFD_LEN: u64 = 8;
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
            registers: [zero; 16],
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
            memory: Memory::default(),
            stored: BTreeMap::new(),
            rip: image.entry,
            path: Vec::new(),
            stdin: Rc::new(stdin),
            stdin_read: 0,
            stdin_pieces: Rc::new(Vec::new()),
            files: Rc::new(BTreeMap::new()),
            written: BTreeMap::new(),
            folders_before: Rc::new(BTreeSet::new()),
            made: BTreeSet::new(),
            removed: BTreeSet::new(),
            open: BTreeMap::new(),
            network: Rc::new(net::Network::default()),
            sockets: BTreeMap::new(),
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
            owners: [None; 16],
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
        state.registers[4] = pool.bv(64, u128::from(STACK_TOP));
        state
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

    /// The same state with standard input arriving in pieces of these lengths.
    #[must_use]
    pub fn with_stdin_pieces(mut self, pieces: Vec<usize>) -> Self {
        self.stdin_pieces = Rc::new(pieces);
        self
    }

    /// The same state with the test's network: its server or client, as `network` says.
    #[must_use]
    pub fn with_network(mut self, network: net::Network) -> Self {
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
fn step(image: &Image, state: &mut State, pool: &mut Pool) -> Result<Step, Stop> {
    let Some(instruction) = image.instructions.get(&state.rip).copied() else {
        return Err(End::Signal("SIGSEGV"));
    };
    state.steps += 1;
    if state.steps > MAX_STEPS {
        return Err(unsupported("a run took too many instructions to follow"));
    }
    let before = state.registers;
    // fill read as a whole, where the proof takes every way its input can be cut; elsewhere,
    // as for a test's sample, its code runs as it is.
    if let Some(fill) = image.fill
        && state.every_cut
        && fill.at_head(state.rip)
    {
        // fill moves rsi through the buffer it was lent.
        let plan = Plan {
            registers: [Some((6, state.owners[6])), None],
            store: None,
        };
        let mut cpu = Cpu::new(image, state, pool, &instruction);
        if let Some(mut step) = cpu.fill(fill)? {
            plan.apply_all(state, &mut step, &before);
            return Ok(step);
        }
    }
    let next = instruction.next_ip();
    state.rip = next;
    let below = pool.constant(state.registers[4]);
    let mut cpu = Cpu::new(image, state, pool, &instruction);
    let plan = cpu.plan();
    let mut step = cpu.execute();
    if let Ok(step) = &mut step {
        plan.apply_all(state, step, &before);
    }
    // Stack the instruction released, by pop, ret or add rsp, is gone: reading it again before
    // writing it reads nothing written.
    if let (Some(below), Some(above)) = (below, pool.constant(state.registers[4]))
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

/// One instruction being executed on a state.
struct Cpu<'a> {
    image: &'a Image,
    state: &'a mut State,
    pool: &'a mut Pool,
    instruction: &'a Instruction,
}

impl<'a> Cpu<'a> {
    fn new(
        image: &'a Image,
        state: &'a mut State,
        pool: &'a mut Pool,
        instruction: &'a Instruction,
    ) -> Self {
        Self {
            image,
            state,
            pool,
            instruction,
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
        format!(
            "in {}, `{}`",
            self.image.function_at(self.instruction.ip()),
            self.instruction
        )
    }

    /// The live frames, the newest first; `_start`'s reaches over the array of the program's
    /// arguments above it, which it reads through rsp.
    fn frames(&self) -> Vec<Frame> {
        let Some(mut low) = self
            .pool
            .constant(self.state.registers[4])
            .and_then(|rsp| u64::try_from(rsp).ok())
        else {
            return Vec::new();
        };
        let mut frames = Vec::new();
        let mut running = self.instruction.ip();
        for back in self.state.returns.iter().rev() {
            frames.push(Frame {
                low,
                top: back.slot,
                running,
                number: back.frame,
            });
            low = back.slot + 8;
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
        if let Some(back) = self
            .state
            .returns
            .iter()
            .find(|back| (back.slot..back.slot + 8).contains(&address))
        {
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
                plan.store = Some((address, cpu.state.registers[index], owner));
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

    /// Checks a buffer of `count` bytes at `buffer`, lent to the kernel in `register` by the
    /// system call `call`.
    fn lent(
        &mut self,
        register: Register,
        buffer: u64,
        count: u64,
        writing: bool,
        call: &str,
    ) -> Result<(), Stop> {
        if count == 0 {
            return Ok(());
        }
        let what = |cpu: &Self| {
            format!(
                "in {}, the {call} system call",
                cpu.image.function_at(cpu.instruction.ip())
            )
        };
        let held = self
            .state
            .owner(register)
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
            .find(|back| address < back.slot + 8 && back.slot < end)
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
    fn places(&mut self, address: Term, bytes: u32, writing: bool) -> Result<Vec<u64>, Stop> {
        let (low, high) = self.pool.range(address);
        if high - low >= MAX_PLACES {
            return Err(unsupported(
                "an address that depends on the input in more than 256 ways",
            ));
        }
        let held = self.held();
        let mut places = Vec::new();
        let mut outside = Vec::new();
        for place in low..=high {
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
            if let Some(held) = held
                && let Err(End::Unsafe(problem)) =
                    self.reach(held, place, u64::from(bytes), writing, &Self::doing)
            {
                outside.push((place, problem));
            }
            places.push(place);
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

    /// A load from an address the input chooses: the value at each place it can be, chosen by
    /// the address, so a table indexed by an input byte is a function of the byte.
    fn load_anywhere(&mut self, address: Term, bytes: u32) -> Result<Term, Stop> {
        let places = self.places(address, bytes, false)?;
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
    fn save_anywhere(&mut self, address: Term, value: Term) -> Result<(), Stop> {
        let bytes = self.pool.width(value) / 8;
        for place in self.places(address, bytes, true)? {
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
                        self.load_anywhere(address, bytes)
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
                    self.save_anywhere(address, value)
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
                let back = self.pool.bv(64, u128::from(instruction.next_ip()));
                self.push(back)?;
                let slot = self.constant(self.state.registers[4])?;
                self.image.enter(instruction.near_branch_target());
                self.state.calls += 1;
                self.state.returns.push(Return {
                    slot,
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
                    Some(back) if back.slot == slot && back.back == target => {}
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
        let registers = self.state.registers;
        let (folder, flags) = (registers[7], registers[2]);
        let flags = self.constant(flags)? & 0xffff_ffff;
        if flags & O_DIRECTORY != 0 {
            return Err(unsupported(
                "listing a folder, whose entries the kernel gives in an order of its own",
            ));
        }
        let bytes = match self.name_or_choice(folder, Register::RSI, "openat")? {
            Name::Known(name) => {
                let result = self.open_name(name, flags)?;
                self.state.registers[0] = self.pool.bv(64, u128::from(result));
                self.state.registers[1] = self.pool.bv(64, u128::from(self.instruction.next_ip()));
                self.state.registers[11] = self.pool.bv(64, 0x202);
                return Ok(Step::Next);
            }
            Name::Chosen(bytes) => bytes,
        };
        self.open_choosing(&bytes, flags)
    }

    /// `openat` of `name` with `flags`, as the working folder stands.
    fn open_name(&mut self, name: String, flags: u64) -> Result<u64, Stop> {
        let writing = match flags & O_ACCMODE {
            0 if flags & !O_ACCMODE & !O_HARMLESS == 0 => false,
            O_WRONLY if flags & !O_ACCMODE & !O_HARMLESS & !O_TRUNC == O_CREAT => true,
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
    fn name_at(&mut self, folder: Term, path: Register, call: &str) -> Result<String, Stop> {
        match self.name_or_choice(folder, path, call)? {
            Name::Known(name) => Ok(name),
            Name::Chosen(_) => Err(unsupported(format!(
                "{call} on a file name the input chooses"
            ))),
        }
    }

    /// The name a call is given, or, where the input chooses its bytes, those bytes: the
    /// length is the shape's, so only which bytes is open.
    fn name_or_choice(
        &mut self,
        folder: Term,
        register: Register,
        call: &str,
    ) -> Result<Name, Stop> {
        // The kernel reads the folder as an int.
        if self.constant(folder)? & 0xffff_ffff != AT_FDCWD {
            return Err(unsupported(format!(
                "{call} other than in the working folder"
            )));
        }
        let path = self.state.registers[slot(register).unwrap_or(6)];
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
        self.lent(register, start, bytes.len() as u64 + 1, false, call)?;
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
        let next = self.instruction.next_ip();
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
                    self.state.registers[0] = self.pool.bv(64, u128::from(taken));
                    self.state.registers[1] = self.pool.bv(64, u128::from(next));
                    self.state.registers[11] = self.pool.bv(64, 0x202);
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
        outcomes.push((none, None));
        let original = self.state.clone();
        let next = self.instruction.next_ip();
        let mut choices = Vec::new();
        for (condition, name) in outcomes {
            if self.pool.truth(condition) == Some(false) {
                continue;
            }
            let mut outcome = original.clone();
            std::mem::swap(self.state, &mut outcome);
            let opened = match name {
                Some(name) => self.open_name(name, flags),
                None if flags & O_ACCMODE == 0 => Ok((-ENOENT).cast_unsigned()),
                None => Err(unsupported("making a file the input names")),
            };
            match opened {
                Ok(result) => {
                    self.state.registers[0] = self.pool.bv(64, u128::from(result));
                    self.state.registers[1] = self.pool.bv(64, u128::from(next));
                    self.state.registers[11] = self.pool.bv(64, 0x202);
                }
                Err(stop) => self.state.end = Some(stop),
            }
            std::mem::swap(self.state, &mut outcome);
            choices.push((condition, outcome));
        }
        Ok(Step::Choose(choices))
    }

    /// `fstat(fd, details)`: an open file's kind and size, the file's own; the kernel's other
    /// numbers, such as its device, owner and times, are any value.
    fn file_details(&mut self, fd: u64, buffer: u64) -> Result<u64, Stop> {
        self.lent(Register::RSI, buffer, STAT_LEN, true, "fstat")?;
        let Some(open) = self.state.open.get(&fd).cloned() else {
            if fd <= 2 {
                return Err(unsupported("the details of standard input or output"));
            }
            return Ok((-EBADF).cast_unsigned());
        };
        let size = self.state.file(&open.name).map_or(0, <[Term]>::len);
        let fresh = self.pool.len();
        for offset in 0..STAT_LEN {
            let byte = self.pool.var(8, &format!("stat{fresh}.{offset}"));
            self.save(buffer + offset, byte)?;
        }
        let mode = self.pool.bv(32, REGULAR_FILE);
        self.save(buffer + STAT_MODE, mode)?;
        let size = self.pool.bv(64, size as u128);
        self.save(buffer + STAT_SIZE, size)?;
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
        let registers = self.state.registers;
        let flags = self.constant(registers[7])?;
        if flags != crate::platform::Profile::CLONE_FLAGS {
            return Err(unsupported(format!("clone with flags {flags:#x}")));
        }
        let (stack, parent_tid, child_tid) = (
            self.constant(registers[6])?,
            self.constant(registers[2])?,
            self.constant(registers[10])?,
        );
        self.state.threads += 1;
        let tid = FIRST_TID + self.state.threads;
        let id = self.pool.bv(32, u128::from(tid));
        self.save(parent_tid, id)?;
        let next = self.instruction.next_ip();
        let (next_term, flags_word) = (self.pool.bv(64, u128::from(next)), self.pool.bv(64, 0x202));
        let mut waiting = registers;
        waiting[0] = self.pool.bv(64, u128::from(tid));
        waiting[1] = next_term;
        waiting[11] = flags_word;
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
        // The new thread: the same registers but rax, 0, and rsp, its own stack, with no calls
        // yet and a first frame of its own.
        state.registers[0] = self.pool.bv(64, 0);
        state.registers[1] = next_term;
        state.registers[4] = self.pool.bv(64, u128::from(stack));
        state.registers[11] = flags_word;
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
        state.rip = waiting.rip;
        state.returns = waiting.returns;
        state.owners = waiting.owners;
        state.stack_top = waiting.stack_top;
        state.base_frame = waiting.base_frame;
        Ok(Step::Next)
    }

    fn syscall(&mut self) -> Result<Step, Stop> {
        let number = self.state.registers[0];
        let number = self
            .pool
            .constant(number)
            .ok_or_else(|| unsupported("a system call chosen by the input"))?;
        let registers = self.state.registers;
        let (rdi, rsi, rdx) = (registers[7], registers[6], registers[2]);
        let result = match number {
            0 => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let buffer = self.constant(rsi)?;
                let count = self.constant(rdx)?;
                self.lent(Register::RSI, buffer, count, true, "read")?;
                if fd == 0 {
                    self.state.raw_reads = true;
                    // A pipe hands over at most 4096 bytes whole; past that, how the input
                    // arrives matters, which only fill makes not matter. A test's sample is run
                    // as one read gives it.
                    if self.state.every_cut && self.state.stdin.len() > super::TEXT_BOUND {
                        return Err(End::Unsupported(format!(
                            "standard input past {} bytes is more than one pipe write, which the \
                             proof reads whole; read it with fill to be proven for any length",
                            super::TEXT_BOUND
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
            1 if self
                .connection_of(self.constant(rdi)? & 0xffff_ffff)
                .is_ok() =>
            {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let (buffer, count) = (self.constant(rsi)?, self.constant(rdx)?);
                self.lent(Register::RSI, buffer, count, false, "write")?;
                match self.connection_of(fd) {
                    Ok(Some(number)) => self.send(number, buffer, count, 0)?,
                    _ => self.not_connected(),
                }
            }
            1 => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let buffer = self.constant(rsi)?;
                let count = self.constant(rdx)?;
                self.lent(Register::RSI, buffer, count, false, "write")?;
                // Standard output or error, a file open for writing, or nothing that can be.
                let target = match fd {
                    1 | 2 => Some(None),
                    _ => match self.state.open.get(&fd) {
                        Some(open) if open.writing => Some(Some(open.name.clone())),
                        _ => None,
                    },
                };
                match target {
                    None => (-EBADF).cast_unsigned(),
                    Some(file) => {
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
            3 => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                if self.state.open.remove(&fd).is_some() || self.close_socket(fd) {
                    0
                } else {
                    (-EBADF).cast_unsigned()
                }
            }
            41 => {
                let (domain, kind, protocol) = (
                    self.constant(rdi)? & 0xffff_ffff,
                    self.constant(rsi)? & 0xffff_ffff,
                    self.constant(rdx)? & 0xffff_ffff,
                );
                self.socket(domain, kind, protocol)?
            }
            42 => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let address = self.constant(rsi)?;
                self.lent(
                    Register::RSI,
                    address,
                    self.constant(rdx)? & 0xffff_ffff,
                    false,
                    "connect",
                )?;
                return self.connect(fd, address);
            }
            43 | 288 => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let (address, length) = (self.constant(rsi)?, self.constant(rdx)?);
                if number == 288 && self.constant(registers[10])? & !0o2_000_000 != 0 {
                    return Err(unsupported(
                        "accepting a connection with flags beyond close-on-exec",
                    ));
                }
                if address != 0 {
                    self.lent(Register::RSI, address, 16, true, "accept")?;
                    self.lent(Register::RDX, length, 4, true, "accept")?;
                }
                self.accept(fd, address, length)?
            }
            44 | 45 => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let (buffer, count) = (self.constant(rsi)?, self.constant(rdx)?);
                let flags = self.constant(registers[10])? & 0xffff_ffff;
                if self.constant(registers[8])? != 0 {
                    return Err(unsupported("sending to or receiving from a named address"));
                }
                let call = if number == 44 { "sendto" } else { "recvfrom" };
                self.lent(Register::RSI, buffer, count, number == 45, call)?;
                match self.connection_of(fd) {
                    Err(()) => (-EBADF).cast_unsigned(),
                    Ok(None) => self.not_connected(),
                    Ok(Some(connection)) if number == 44 => {
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
            48 => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let how = self.constant(rsi)? & 0xffff_ffff;
                self.shutdown(fd, how)?
            }
            257 => return self.open_chosen(),
            5 => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let buffer = self.constant(rsi)?;
                self.file_details(fd, buffer)?
            }
            217 => {
                return Err(unsupported(
                    "listing a folder, whose entries the kernel gives in an order of its own",
                ));
            }
            258 => {
                let name = self.name_at(rdi, Register::RSI, "mkdirat")?;
                self.make_folder(name)
            }
            263 => {
                let name = self.name_at(rdi, Register::RSI, "unlinkat")?;
                let flags = self.constant(rdx)? & 0xffff_ffff;
                self.remove(name, flags)?
            }
            264 => {
                let from = self.name_at(rdi, Register::RSI, "renameat")?;
                let to = self.name_at(rdx, Register::R10, "renameat")?;
                self.rename(from, to)?
            }
            // The test's readings, as the supervisor gives them, the last one repeating.
            228 if !self.state.clock.is_empty() => {
                let buffer = self.constant(rsi)?;
                self.lent(Register::RSI, buffer, 16, true, "clock_gettime")?;
                let readings = Rc::clone(&self.state.clock);
                let index = self.state.readings_used.min(readings.len() - 1);
                let (seconds, nanoseconds) = readings[index];
                self.save(buffer, seconds)?;
                self.save(buffer + 8, nanoseconds)?;
                self.state.readings_used += 1;
                0
            }
            // The test's bytes, as the supervisor gives them: zeros past their end.
            318 if self.state.random.is_some() => {
                let buffer = self.constant(rdi)?;
                let count = self.constant(rsi)?;
                self.lent(Register::RDI, buffer, count, true, "getrandom")?;
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
            228 => {
                let clock = self.constant(rdi)? & 0xffff_ffff;
                if !matches!(clock, 0 | 1) {
                    return Err(unsupported(format!("reading clock {clock}")));
                }
                let buffer = self.constant(rsi)?;
                self.lent(Register::RSI, buffer, 16, true, "clock_gettime")?;
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
            318 => {
                let buffer = self.constant(rdi)?;
                let count = self.constant(rsi)?;
                self.lent(Register::RDI, buffer, count, true, "getrandom")?;
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
            230 => {
                let request = self.constant(rdx)?;
                self.lent(Register::RDX, request, 16, false, "clock_nanosleep")?;
                self.load(request, 16)?;
                let left = self.constant(registers[10])?;
                if left != 0 {
                    self.lent(Register::R10, left, 16, true, "clock_nanosleep")?;
                }
                0
            }
            56 => return self.start_thread(),
            // A thread's end: the kernel clears its id and wakes whoever waits on it, and the
            // thread that started it goes on.
            60 if !self.state.suspended.is_empty() => return self.end_thread(),
            60 | 231 => {
                let status = self.pool.extract(rdi, 7, 0);
                return Err(End::Exit(status));
            }
            // A wait for a thread to end. Threads run to their end as they start, so its id is
            // already cleared: the word no longer holds what the waiter saw.
            202 => {
                let (word, operation) = (self.constant(rdi)?, self.constant(rsi)? & 0xffff_ffff);
                // FUTEX_WAIT, with or without FUTEX_PRIVATE_FLAG.
                if operation & !128 != 0 {
                    return Err(unsupported(format!("futex operation {operation}")));
                }
                self.lent(Register::RDI, word, 4, false, "futex")?;
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
            7 => {
                let (table, count) = (self.constant(rdi)?, self.constant(rsi)?);
                let waits_for_input = count == 1 && {
                    self.lent(Register::RDI, table, POLLFD_LEN, true, "poll")?;
                    let descriptor = self.load(table, 4)?;
                    self.pool.constant(descriptor) == Some(0)
                };
                if !waits_for_input {
                    return Err(unsupported(
                        "poll over several descriptors, with which a server serves clients at once",
                    ));
                }
                let ready = self.pool.bv(16, POLLIN);
                self.save(table + 6, ready)?;
                1
            }
            // As in a test, standard input, output and error are never a terminal: its mode
            // can't be read or set, and the program goes on as from a pipe.
            16 => {
                if self.constant(rdi)? & 0xffff_ffff > 2 {
                    return Err(unsupported("ioctl on a file or a connection"));
                }
                (-ENOTTY).cast_unsigned()
            }
            other => return Err(unsupported(format!("system call {other}"))),
        };
        self.state.registers[0] = self.pool.bv(64, u128::from(result));
        self.state.registers[1] = self.pool.bv(64, u128::from(self.instruction.next_ip()));
        self.state.registers[11] = self.pool.bv(64, 0x202);
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
    let mut pending = vec![start];
    let mut finished = Vec::new();
    while let Some(mut state) = pending.pop() {
        if finished.len() + pending.len() > max_runs {
            state.end = Some(End::TooManyPaths);
            finished.push(state);
            break;
        }
        loop {
            if state.end.is_some() {
                finished.push(state);
                break;
            }
            match step(image, &mut state, pool) {
                Ok(Step::Next) => {}
                Ok(Step::Split(condition, other)) => {
                    let mut other = *other;
                    let opposite = pool.bool_not(condition);
                    // A run's conditions can always be met together, so when one side of a
                    // branch is impossible the other is certain, and needs no question.
                    let yes = oracle.possible(pool, &state.path, condition)?;
                    let no = !yes || oracle.possible(pool, &other.path, opposite)?;
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
                    state.end = Some(end);
                    finished.push(state);
                    break;
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

mod bits;
mod fill;
pub mod net;
mod vector;

#[cfg(test)]
mod against_cpu;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::Profile;
    use crate::toolchain::Toolchain;

    /// Builds `source` and loads it.
    async fn image(source: &str) -> Image {
        let toolchain = Toolchain::discover().await.expect("llvm-mc and ld.lld");
        let folder = tempfile::tempdir().expect("temporary folder");
        let (assembly, object, executable) = (
            folder.path().join("main.s"),
            folder.path().join("main.o"),
            folder.path().join("main"),
        );
        std::fs::write(&assembly, source).expect("write");
        toolchain
            .assemble(Profile::LinuxX86_64, &assembly, &object)
            .await
            .expect("assemble");
        toolchain.link(&object, &executable).await.expect("link");
        let bytes = std::fs::read(&executable).expect("read");
        let machine = crate::checker::decode::read(&bytes).expect("decode");
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

    #[tokio::test]
    async fn adds_with_carry_like_the_cpu() {
        // 0xff..ff + 0xff..ff with a carry in: the result equals the first operand, yet it
        // carries out, so the exit status is 1.
        let source = ".intel_syntax noprefix\n.globl _start\n.text\n.type _start, @function\n\
                      _start:\n    mov rax, -1\n    mov rbx, -1\n    stc\n    adc rax, rbx\n    \
                      setb dil\n    movzx edi, dil\n    mov eax, 60\n    syscall\n\
                      .size _start, . - _start\n";
        let image = image(source).await;
        let mut pool = Pool::default();
        let start = State::start(&image, &mut pool, &[], &[], Vec::new());
        let runs = explore(&image, start, &mut pool, &mut Concrete, 2).expect("runs");
        let status = match &runs[0].end {
            Some(End::Exit(status)) => pool.constant(*status),
            other => panic!("ended with {other:?}"),
        };
        assert_eq!(status, Some(1));
    }

    /// fill's function from assets/fill.s, as main.s copies it.
    fn fill_function(asset: &str) -> &str {
        let start = asset.find(".type fill, @function").expect("fill's start");
        let end = asset.find(".size fill, . - fill").expect("fill's end");
        &asset[start..end + ".size fill, . - fill".len()]
    }

    /// A program that calls `fill` on standard input with a buffer of `capacity` bytes and the
    /// end byte `end` (-1 for none), prints what it read and exits with fill's result.
    fn fill_program(fill: &str, capacity: u32, end: i32) -> String {
        format!(
            ".intel_syntax noprefix\n.equ SYS_READ, 0\n.equ ERROR_IO, -5\n\
             .equ MAX_FILL_READS, 1099511627776\n.globl _start\n.text\n\
             .type _start, @function\n_start:\n    xor edi, edi\n    lea rsi, [rip + buffer]\n    \
             mov edx, {capacity}\n    mov ecx, {end}\n    call fill\n    mov rbx, rax\n    \
             test rax, rax\n    js .Lstart_exit\n    mov rdx, rax\n    mov eax, 1\n    \
             mov edi, 1\n    lea rsi, [rip + buffer]\n    syscall\n.Lstart_exit:\n    \
             mov rdi, rbx\n    mov eax, 60\n    syscall\n.size _start, . - _start\n{fill}\n\
             .type assert_fail, @function\nassert_fail:\n    mov edi, 134\n    mov eax, 60\n    \
             syscall\n.size assert_fail, . - assert_fail\n.bss\n.type buffer, @object\n\
             buffer:\n    .zero 16\n.size buffer, 16\n"
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

    /// fill read as a whole gives exactly what its code gives, over every way its input can be
    /// cut: every input of up to 5 bytes of `a` and newlines, every capacity up to 7, with a
    /// newline as the end byte and with none. Bugs planted in its code each give something the
    /// whole doesn't.
    #[tokio::test]
    async fn fill_read_whole_is_its_code_for_every_cut() {
        let canonical = fill_function(crate::checker::FILL_TEMPLATE);
        let mutants = [
            (
                "reads every piece at the buffer's start",
                "    add rsi, rax\n",
                "",
            ),
            (
                "stops a byte short of full",
                "    sub rdx, rax\n    jz .Lfill_done\n",
                "    sub rdx, rax\n    cmp rdx, 1\n    jbe .Lfill_done\n",
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
        ];
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
                let whole = image(&fill_program(canonical, capacity, end)).await;
                assert!(whole.has_fill(), "the canonical fill is found");
                let mut planted = Vec::new();
                for (_, from, to) in mutants {
                    let mutant = canonical.replacen(from, to, 1);
                    assert_ne!(mutant, canonical);
                    planted.push(image(&fill_program(&mutant, capacity, end)).await);
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

    #[tokio::test]
    async fn a_sleep_ends_at_once() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nap/main.s"),
        )
        .expect("the nap fixture");
        let image = image(&source).await;
        assert_eq!(
            run(&image, &[], ""),
            ("awake\n".into(), String::new(), Some(0))
        );
    }

    #[tokio::test]
    async fn runs_the_template_like_the_cpu() {
        let image = image(crate::checker::TEMPLATE).await;
        assert_eq!(
            run(&image, &["7"], ""),
            ("-7\n".into(), String::new(), Some(0))
        );
        assert_eq!(
            run(&image, &["-9223372036854775807"], ""),
            ("9223372036854775807\n".into(), String::new(), Some(0))
        );
        assert_eq!(
            run(&image, &[], ""),
            (String::new(), "usage: negate <n>\n".into(), Some(1))
        );
        assert_eq!(
            run(&image, &["12x"], ""),
            (String::new(), "error: not a number\n".into(), Some(2))
        );
    }
}
