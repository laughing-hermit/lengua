//! The secret rule: nothing the idea's `- Secret:` line names (standard input, a file, an
//! environment variable or an argument) decides how main.wat runs. The bytes a host call writes
//! from a secret input are secret, and so is every value computed from them, through the operand
//! stack, locals, globals, memory and calls. The rule refuses, each on its line of main.wat with
//! what to write instead:
//!
//! - a secret condition of `if`, `br_if`, `br_table` or `select`, and so a loop that a secret
//!   condition repeats or leaves: a loop whose count depends on a secret;
//! - a secret address or index of a load, a store, `memory.copy` or `memory.fill`, or a secret
//!   length of a copy or a fill;
//! - a secret operand of a division, a remainder or any float operator;
//! - a secret argument of a host call or of lengua's library, or a secret in a buffer such a call
//!   reads, but the data it writes out: the bytes `fd_write`, `sock_send` and `$write_all` send
//!   may be secret, their descriptors, addresses and lengths may not; and `$fill` reading a
//!   secret up to a stop byte, which it compares with the last byte of each read.
//!
//! Masks made with arithmetic are what passes: `(i32.sub (i32.const 0) (local.get $bit))` is all
//! ones for 1 and zero for 0, and `and`, `or` and `xor` with it choose between two values with no
//! branch, as the compile skill's references/constant-time.md shows.
//!
//! Memory is followed as the memory rules follow it, by region and offset: for each region, the
//! ranges of its bytes that may hold a secret, exact where an access's offset is a constant, so a
//! store of a value that isn't secret over secret bytes makes them public again; a `memory.copy`,
//! `memory.fill` or `memory.init` does so only when its length is a constant too, since one of a
//! length that varies may write fewer bytes. Each function is followed once for each way it's
//! called (which of its arguments are secret, what the addresses they lend point to, and which
//! bytes and globals hold a secret as it starts), and what it leaves is kept per function, so a
//! callee's result is secret where its arguments make it so. What the rule can't tell apart it
//! takes as secret: every argument's text when an argument is secret, as `args_get` writes them
//! into one buffer; every variable's text when a variable is; and every file the program reads
//! when a file is, since an opened file's descriptor comes back through memory, so a read
//! through any descriptor that may be an opened one's, a connection's too, while `sock_recv`,
//! which reads no file, gives no file's bytes.
//!
//! An access the memory rules bound stays inside the region its address is made from. One they
//! leave to the proof (through an address loaded from memory, at an index checked against a
//! limit they don't follow, of a length they don't bound) the proof holds inside the memory, not
//! inside a region: it may touch any region, and bytes outside them all, which stay secret once
//! a secret may be there, as no store the rule follows makes them public. The buffer `args_get`
//! or `environ_get` fills ends with its region at the latest: the memory rules refuse a call
//! whose sizes the program didn't check against its regions first.
//!
//! A secret that leaves may come back: once main.wat writes a secret to a descriptor that may be
//! a file's or a connection's, or sends one with `sock_send`, anywhere in the program, what
//! `fd_read`, `$fill` and `sock_recv` read from every file and connection is secret, in this run
//! or a later one. What goes to standard output or standard error alone doesn't come back.
//!
//! What a program opens by itself follows the same lines. The name of a computer it gives
//! `tcp_open`, `udp_open` or `dns_lookup` isn't data it writes out: the name goes to the resolver
//! and decides where the program connects, so a secret in it is refused, as a secret port is.
//! The bytes of a datagram, `udp_send`'s and `udp_send_to`'s, are data, as `sock_send`'s are; and
//! once a secret has left, what `udp_recv`, `udp_recv_from` and `dns_lookup` write is secret, as
//! what a connection gives is. Since no branch and no loop's count depends on a secret, neither
//! does how many connections a program opens, nor when it opens them.
//!
//! Lengths and counts aren't secret: what `$fill` reads to the end says it read, what a read of
//! a file says, the sizes calls write, and where `args_get` puts each argument. One count is:
//! how many bytes a single `fd_read` of a secret standard input gave, since a terminal gives a
//! line at a time, so that count says where the secret's line ends. Error numbers aren't secret.
//!
//! What the rule can't follow it refuses, and takes as all secret: a call that leads back to its
//! caller, blocks and calls nested more than 256 deep, and a program of more than 4,000,000
//! operators as it follows them. A loop that hasn't settled in 32 rounds has all it may write
//! taken as secret: the locals it writes, every byte of memory and every global that changes.
//!
//! It is a rule over main.wat, like a type. For a module it passes, it also gives every operator
//! that takes a secret operand ([`Taken`]), by its offset in the module, so that the machine
//! code the engine compiles each one to on this CPU is checked as well ([`super::machine`]).

use std::collections::{BTreeMap, BTreeSet};

use wasmparser::{BlockType, FrameKind, Operator};

use super::flow::{Flows, Shape};
use super::memory::{self, Analysis, Offset, Record, Value};
use super::model::{Function, Kind, Module, to_u32, to_usize};
use crate::checker::{Rule, Violation};
use crate::idea::Source;
use crate::wasm::host::fds;
use crate::wasm::wasi::Bytes;

/// The most times a loop's body is followed before everything it writes is taken as secret.
const MAX_ROUNDS: usize = 32;

/// The most operators followed in all, across every call, past which loops stop being refined
/// and calls stop being followed.
const BUDGET: usize = 4_000_000;

/// The deepest the rule follows blocks and calls, counted together.
const MAX_DEPTH: usize = 256;

/// The most ranges kept of one region, past which they become one.
const MAX_RANGES: usize = 64;

/// `$fill`'s stop byte that reads to the end, -1, as the memory rules give an i32 constant: its
/// 32 bits, unsigned.
const READ_TO_END: u64 = 0xFFFF_FFFF;

/// The bytes of the count a read writes: how many bytes it gave, a 32-bit number.
const COUNT_BYTES: u64 = 4;

/// What each fix that chooses with a mask shows.
const MASK: &str = "`(i32.sub (i32.const 0) (local.get $bit))` is all ones for 1 and 0 for 0, \
                    and `(i32.or (i32.and (local.get $a) (local.get $mask)) (i32.and (local.get \
                    $b) (i32.xor (local.get $mask) (i32.const -1))))` is $a or $b by it, as \
                    references/constant-time.md shows";

/// Which inputs the idea names as secret, as far as the rule tells them apart.
#[derive(Debug, Clone, Copy, Default)]
struct Sources {
    /// Standard input.
    standard_input: bool,
    /// A file: every file the program reads, since which one a descriptor is comes back through
    /// memory.
    files: bool,
    /// An environment variable: every variable's text, which `environ_get` writes together.
    environment: bool,
    /// An argument: every argument's text, which `args_get` writes together.
    arguments: bool,
    /// What a file or a peer gives back: the program writes a secret to a file or sends one to
    /// a peer, so what it reads from any file or connection may be that secret.
    returned: bool,
}

impl Sources {
    /// The inputs `secrets` names.
    fn of(secrets: &[Source]) -> Self {
        let mut sources = Self::default();
        for secret in secrets {
            match secret {
                Source::StandardInput => sources.standard_input = true,
                Source::File(_) => sources.files = true,
                Source::Environment(_) => sources.environment = true,
                Source::Argument(_) => sources.arguments = true,
            }
        }
        sources
    }
}

/// The bytes of memory that may hold a secret: for each region, ranges of its bytes counted from
/// its first, each from its start up to its end, sorted and apart; and whether a byte outside
/// every region may.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Marks {
    /// The ranges, by region.
    regions: BTreeMap<usize, Vec<(u64, u64)>>,
    /// Whether a byte in no region may hold a secret: one a write through an address the memory
    /// rules don't follow may have put there. No store the rule follows reaches such a byte, so
    /// nothing makes it public again.
    outside: bool,
}

impl Marks {
    /// Marks bytes `start` to `end` of `region` as ones that may hold a secret.
    fn mark(&mut self, region: usize, start: u64, end: u64) {
        if start >= end {
            return;
        }
        let ranges = self.regions.entry(region).or_default();
        ranges.push((start, end));
        normalize(ranges);
    }

    /// Marks bytes `start` to `end` of `region` as holding no secret: what a store of a value
    /// that isn't secret leaves there.
    fn clear(&mut self, region: usize, start: u64, end: u64) {
        let Some(ranges) = self.regions.get(&region) else {
            return;
        };
        let mut kept = Vec::with_capacity(ranges.len() + 1);
        for &(low, high) in ranges {
            if high <= start || low >= end {
                kept.push((low, high));
                continue;
            }
            if low < start {
                kept.push((low, start));
            }
            if high > end {
                kept.push((end, high));
            }
        }
        if kept.is_empty() {
            self.regions.remove(&region);
        } else {
            self.regions.insert(region, kept);
        }
    }

    /// Whether any of bytes `start` to `end` of `region` may hold a secret.
    fn holds(&self, region: usize, start: u64, end: u64) -> bool {
        self.regions
            .get(&region)
            .is_some_and(|ranges| ranges.iter().any(|&(low, high)| low < end && start < high))
    }

    /// Whether any byte may hold a secret, in a region or outside them all.
    fn any(&self) -> bool {
        self.outside || !self.regions.is_empty()
    }

    /// Adds `other`'s marks to these: what may hold a secret where paths meet.
    fn join(&mut self, other: &Self) {
        self.outside |= other.outside;
        for (region, ranges) in &other.regions {
            let mine = self.regions.entry(*region).or_default();
            mine.extend(ranges.iter().copied());
            normalize(mine);
        }
    }
}

/// `ranges` sorted, those that overlap or touch made one; past [`MAX_RANGES`], all of them one.
fn normalize(ranges: &mut Vec<(u64, u64)>) {
    ranges.sort_unstable();
    let mut joined: Vec<(u64, u64)> = Vec::with_capacity(ranges.len());
    for &(start, end) in ranges.iter() {
        match joined.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => joined.push((start, end)),
        }
    }
    if joined.len() > MAX_RANGES
        && let (Some(first), Some(last)) = (joined.first().copied(), joined.last().copied())
    {
        joined = vec![(first.0, last.1)];
    }
    *ranges = joined;
}

/// Where an access goes, as the rule follows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    /// Bytes `start` to `end` of a region, counted from its first byte.
    Region {
        /// The region's index.
        region: usize,
        /// The first byte the access may touch.
        start: u64,
        /// Just past the last.
        end: u64,
        /// Whether those are the bytes it touches on every run, not only bytes it may touch.
        exact: bool,
    },
    /// Anywhere: through an address the memory rules don't follow, or past its region.
    Anywhere,
}

/// How many bytes from its address an access touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// This many.
    Bytes(u64),
    /// As many as `args_sizes_get` or `environ_sizes_get` said: as far as its region goes, which
    /// the sizes the program asked for are taken to fit.
    Rest,
    /// A count the memory rules don't bound, which they leave to the proof: the proof holds it
    /// inside the memory, not inside the region, so it may reach anywhere.
    Unbounded,
}

/// What the rule knows at a point of a function: which locals, bytes of memory and globals may
/// hold a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
struct State {
    /// Whether any path reaches it.
    reachable: bool,
    /// Each local, its parameters first.
    locals: Vec<bool>,
    /// The bytes of memory.
    memory: Marks,
    /// The globals that change, by index.
    globals: BTreeSet<u32>,
}

impl State {
    /// What holds where paths from `self` and `other` meet.
    fn joined(&self, other: &Self) -> Self {
        if !self.reachable {
            return other.clone();
        }
        if !other.reachable {
            return self.clone();
        }
        let mut memory = self.memory.clone();
        memory.join(&other.memory);
        Self {
            reachable: true,
            locals: either(&self.locals, &other.locals),
            memory,
            globals: self.globals.union(&other.globals).copied().collect(),
        }
    }
}

/// One way a function is called.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Call {
    /// Which of its arguments are secret.
    secret: Vec<bool>,
    /// What each argument is, as the memory rules follow it, in the caller's terms made
    /// concrete: the region an address lends and how far into it, a descriptor's number.
    bindings: Vec<Value>,
    /// The bytes that may hold a secret as it starts.
    memory: Marks,
    /// The globals that may.
    globals: BTreeSet<u32>,
}

/// What a call of a function leaves.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Outcome {
    /// Which of its results are secret.
    results: Vec<bool>,
    /// The bytes that may hold a secret after it.
    memory: Marks,
    /// The globals that may.
    globals: BTreeSet<u32>,
    /// Whether it returns at all.
    returns: bool,
}

/// What the rule refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Leak {
    /// A secret condition of `if`, `br_if` or `br_table`.
    Branch(&'static str),
    /// A secret condition of a `br_if` or a `br_table` that repeats or leaves the loop on this
    /// line, so that a secret decides its count.
    Count(&'static str, Option<usize>),
    /// A secret condition of `select`.
    Select,
    /// A secret address of this access.
    Address(&'static str),
    /// A secret length of `memory.copy`, `memory.fill` or `memory.init`.
    Length(&'static str),
    /// A secret operand of this division or remainder.
    Division(String),
    /// A secret operand of this float operator.
    Float(String),
    /// A secret argument, counted from 1, of a call of this function.
    Argument(String, usize),
    /// A secret in the buffer of this argument, counted from 1, that a call of this function
    /// reads.
    Buffer(String, usize),
    /// `$fill` reading a secret up to a stop byte.
    Stop,
    /// What the rule doesn't follow.
    Unfollowed(&'static str),
}

impl Leak {
    /// What's wrong, and how to fix it.
    fn words(&self) -> (String, String) {
        match self {
            Self::Branch(what) => (
                format!(
                    "the condition of this `{what}` is computed from a secret, an input the \
                     idea's `- Secret:` line names, so a secret decides which code runs"
                ),
                format!(
                    "run the same code whatever the secret is: compute both outcomes and choose \
                     one with a mask. {MASK}"
                ),
            ),
            Self::Count(what, line) => {
                let which = line.map_or_else(
                    || "the loop around it".to_owned(),
                    |line| format!("the loop on line {line}"),
                );
                (
                    format!(
                        "this `{what}` repeats or leaves {which} by a condition computed from a \
                         secret, so a secret decides how many times the loop runs"
                    ),
                    "run the loop as many times whatever the secret is, a count from a length or \
                     a named constant, and fold each step's result in with `i32.or`, `i32.and` \
                     or a mask"
                        .to_owned(),
                )
            }
            Self::Select => (
                "the condition of this `select` is computed from a secret, and the engine may \
                 compile a `select` to a branch"
                    .to_owned(),
                format!("choose with a mask instead. {MASK}"),
            ),
            Self::Address(what) => (
                format!(
                    "the address of this `{what}` is computed from a secret, so a secret decides \
                     which bytes of memory it touches"
                ),
                "touch the same bytes whatever the secret is: read every place the secret may \
                 choose and keep the one it names with a mask, or index with values that aren't \
                 secret"
                    .to_owned(),
            ),
            Self::Length(what) => (
                format!(
                    "the length of this `{what}` is computed from a secret, so a secret decides \
                     how many bytes it touches"
                ),
                "copy or fill a length that isn't secret, such as the region's size, and choose \
                 what the bytes hold with a mask"
                    .to_owned(),
            ),
            Self::Division(what) => (
                format!(
                    "an operand of this `{what}` is computed from a secret, and the time a \
                     division takes can depend on its operands"
                ),
                "divide only values that aren't secret; for a secret, use shifts, masks or a \
                 multiplication"
                    .to_owned(),
            ),
            Self::Float(what) => (
                format!(
                    "an operand of this `{what}` is computed from a secret, and the time a float \
                     operation takes can depend on its operands"
                ),
                "keep secrets in integers, and compute with them in integer operators".to_owned(),
            ),
            Self::Argument(callee, argument) => (
                format!(
                    "argument {argument} of this call of {callee} is computed from a secret: a \
                     call's descriptors, addresses, lengths and flags decide what it does"
                ),
                "pass values that aren't secret: only the bytes a call writes out, such as the \
                 data of `fd_write` or `$write_all`, may be secret"
                    .to_owned(),
            ),
            Self::Buffer(callee, argument) => (
                format!(
                    "{callee} reads a secret in the buffer of argument {argument}, which isn't \
                     data it writes out"
                ),
                "keep secrets out of what a host call reads, but the data it writes out: an iovec \
                 array's addresses and lengths, a path and a wait aren't secret"
                    .to_owned(),
            ),
            Self::Stop => (
                "`$fill` reads a secret here up to its stop byte, which it compares with the \
                 last byte of each read, so a secret decides where it stops"
                    .to_owned(),
                "read to the end with the stop byte -1, `(global.get $READ_TO_END)`, and find \
                 where the data ends from its length"
                    .to_owned(),
            ),
            Self::Unfollowed(what) => (
                format!("the secret rule can't follow {what}"),
                "make it simpler: fewer nested blocks and calls, and no recursion, so the rule \
                 can follow each value"
                    .to_owned(),
            ),
        }
    }
}

/// What the secret rule already holds of an operator that takes a secret, and so what the
/// machine code the engine makes for it may do besides compute ([`super::machine`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Takes {
    /// It computes with the secret: its code may not branch, look a value up or call.
    Plain,
    /// A `select`: the rule holds its condition as not secret, so its code may branch on it, as
    /// the engine's code for a `select` between vectors does.
    Choice,
    /// A load, a store, a `global.set`, or a `memory.copy`, `memory.fill` or `memory.init`: the
    /// rule holds its addresses and lengths as not secret, so its code may check them against
    /// the memory's size, and may call the engine's own copy or fill.
    Memory,
    /// A call of one of main.wat's functions with a secret argument: its code calls that
    /// function.
    Call,
}

impl Takes {
    /// What the rule holds of `operator`.
    fn of(operator: &Operator<'_>) -> Self {
        match operator {
            Operator::Select | Operator::TypedSelect { .. } => Self::Choice,
            Operator::Call { .. } => Self::Call,
            Operator::GlobalSet { .. }
            | Operator::MemoryCopy { .. }
            | Operator::MemoryFill { .. }
            | Operator::MemoryInit { .. } => Self::Memory,
            other if memory::memory_op(other).is_some() => Self::Memory,
            _ => Self::Plain,
        }
    }
}

/// An operator of main.wat that takes a secret operand on some path the secret rule follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Taken {
    /// Its offset in the module, which is where the engine's address map says its machine code
    /// came from.
    pub offset: u64,
    /// Its line of main.wat.
    pub line: Option<usize>,
    /// Its name as main.wat writes it, such as `i32.xor`.
    pub name: String,
    /// What the rule holds of it.
    pub takes: Takes,
}

/// Follows main.wat from `_start`, with the idea's secrets.
struct Follower<'m, 'a> {
    /// The module.
    module: &'m Module<'a>,
    /// Every function's control flow.
    flows: &'m Flows,
    /// What the memory rules found: each operator's operands, as they follow them.
    analysis: &'m Analysis,
    /// Which inputs are secret.
    sources: Sources,
    /// What each call of a function left, by function, with the way it was called.
    known: BTreeMap<u32, Vec<(Call, Outcome)>>,
    /// The functions being followed, the outermost first.
    calling: Vec<u32>,
    /// How deep inside blocks and calls the rule is.
    depth: usize,
    /// How many operators it has followed.
    steps: usize,
    /// Whether the program may write a secret to a file or send one to a peer, where a later
    /// read, in this run or another, may get it back.
    sent: bool,
    /// What it refuses: each line, problem and fix, once.
    found: BTreeSet<(Option<usize>, String, String)>,
    /// The operators that take a secret operand on some path it follows, by offset.
    taken: BTreeMap<u64, Taken>,
}

impl Follower<'_, '_> {
    /// Follows function `index`, called as `call` on `line` for `results` values: what it
    /// leaves, as it was found before when it was called the same way already.
    fn follow(&mut self, index: u32, call: Call, line: Option<usize>, results: usize) -> Outcome {
        if let Some((_, outcome)) = self
            .known
            .get(&index)
            .and_then(|known| known.iter().find(|(seen, _)| *seen == call))
        {
            return outcome.clone();
        }
        let (module, flows, analysis) = (self.module, self.flows, self.analysis);
        let (Some(function), Some(shape)) = (module.function(index), flows.shapes.get(&index))
        else {
            return self.unfollowed(line, "a call of a function it can't read", &call, results);
        };
        if self.calling.contains(&index) {
            return self.unfollowed(line, "a call that leads back to its caller", &call, results);
        }
        if self.depth >= MAX_DEPTH {
            return self.unfollowed(line, "calls and blocks nested this deep", &call, results);
        }
        if self.steps > BUDGET {
            return self.unfollowed(line, "a program this long", &call, results);
        }
        let records = analysis
            .followed
            .get(&index)
            .map_or(&[][..], |followed| followed.records.as_slice());
        self.calling.push(index);
        self.depth += 1;
        let outcome = Run::new(self, function, shape, records, &call.bindings).body(&call);
        self.depth = self.depth.saturating_sub(1);
        self.calling.pop();
        self.known
            .entry(index)
            .or_default()
            .push((call, outcome.clone()));
        outcome
    }

    /// What a call the rule doesn't follow, `call` on `line` for `results` values, may leave:
    /// everything secret, as the rule refuses the program for it.
    fn unfollowed(
        &mut self,
        line: Option<usize>,
        what: &'static str,
        call: &Call,
        results: usize,
    ) -> Outcome {
        self.report(line, &Leak::Unfollowed(what));
        let mut memory = call.memory.clone();
        everywhere(self.module, &mut memory);
        Outcome {
            results: vec![true; results],
            memory,
            globals: changing(self.module),
            returns: true,
        }
    }

    /// Refuses `leak` on `line`.
    fn report(&mut self, line: Option<usize>, leak: &Leak) {
        let (problem, fix) = leak.words();
        self.found.insert((line, problem, fix));
    }
}

/// A block being followed.
#[derive(Debug)]
struct Frame {
    /// What kind of block it is.
    kind: FrameKind,
    /// Its opener's index; none for the function's body.
    opener: Option<usize>,
    /// The stack's height below its parameters.
    base: usize,
    /// For a loop, how many parameters a branch back to its start carries.
    params: usize,
    /// How many results it gives.
    results: usize,
    /// The states of the branches that leave to its end, with what they carry.
    exits: Vec<(State, Vec<bool>)>,
    /// For a loop, the states of the branches back to its start, with what they carry.
    backs: Vec<(State, Vec<bool>)>,
}

impl Frame {
    /// A block of `kind` opened at `opener`, `base` the stack's height below its parameters.
    fn new(
        kind: FrameKind,
        opener: Option<usize>,
        base: usize,
        params: usize,
        results: usize,
    ) -> Self {
        Self {
            kind,
            opener,
            base,
            params,
            results,
            exits: Vec::new(),
            backs: Vec::new(),
        }
    }
}

/// A function followed for one way it's called.
struct Run<'f, 'm, 'a> {
    /// What follows the program, which this reports to.
    follower: &'f mut Follower<'m, 'a>,
    /// The function.
    function: &'m Function<'a>,
    /// Its control structure.
    shape: &'m Shape,
    /// What each of its operators took, as the memory rules follow it.
    records: &'m [Option<Record>],
    /// What each of its arguments is, made concrete.
    bindings: &'f [Value],
    /// The operand stack: whether each value may be secret.
    stack: Vec<bool>,
    /// The blocks it's inside, the function's body first.
    frames: Vec<Frame>,
}

impl<'f, 'm, 'a> Run<'f, 'm, 'a> {
    /// `function` followed for `follower`, whose control structure is `shape`, whose operators
    /// took `records`, and whose arguments are `bindings`.
    fn new(
        follower: &'f mut Follower<'m, 'a>,
        function: &'m Function<'a>,
        shape: &'m Shape,
        records: &'m [Option<Record>],
        bindings: &'f [Value],
    ) -> Self {
        Self {
            follower,
            function,
            shape,
            records,
            bindings,
            stack: Vec::new(),
            frames: Vec::new(),
        }
    }

    /// The module.
    fn module(&self) -> &'m Module<'a> {
        self.follower.module
    }

    /// Follows the function from its start, as `call` calls it, to what it leaves.
    fn body(mut self, call: &Call) -> Outcome {
        let function = self.function;
        let mut locals = call.secret.clone();
        locals.resize(function.params.len(), false);
        locals.resize(function.local_count(), false);
        let mut state = State {
            reachable: true,
            locals,
            memory: call.memory.clone(),
            globals: call.globals.clone(),
        };
        let results = function.results.len();
        self.frames
            .push(Frame::new(FrameKind::Block, None, 0, 0, results));
        self.run(0, &mut state);
        let mut left: Option<(State, Vec<bool>)> =
            state.reachable.then(|| (state.clone(), self.top(results)));
        if let Some(frame) = self.frames.pop() {
            for (exit, values) in frame.exits {
                left = Some(match left {
                    None => (exit, values),
                    Some((known, carried)) => (known.joined(&exit), either(&carried, &values)),
                });
            }
        }
        match left {
            Some((after, values)) => Outcome {
                results: values,
                memory: after.memory,
                globals: after.globals,
                returns: true,
            },
            None => Outcome {
                results: vec![false; results],
                memory: call.memory.clone(),
                globals: call.globals.clone(),
                returns: false,
            },
        }
    }

    /// Follows the code from `at` to the `end` or `else` that ends its block, and gives that
    /// operator's index.
    fn run(&mut self, mut at: usize, state: &mut State) -> usize {
        let function = self.function;
        while at < function.body.len() {
            match function.body[at].operator {
                Operator::Block { blockty } => at = self.block(at, blockty, state),
                Operator::Loop { blockty } => at = self.looped(at, blockty, state),
                Operator::If { blockty } => at = self.branch_if(at, blockty, state),
                Operator::Else | Operator::End => return at,
                _ => {
                    self.step(at, state);
                    at += 1;
                }
            }
        }
        at
    }

    /// Goes one block deeper, when the rule follows that deep.
    fn enter(&mut self) -> bool {
        if self.follower.depth >= MAX_DEPTH {
            return false;
        }
        self.follower.depth += 1;
        true
    }

    /// Comes back out of a block.
    fn leave(&mut self) {
        self.follower.depth = self.follower.depth.saturating_sub(1);
    }

    /// Skips the block opened at `at` and ended at `end`, too deep to follow, which gives
    /// `results` values: everything it may write may hold a secret, and the rule refuses the
    /// program for it.
    fn skip(&mut self, at: usize, end: usize, results: usize, state: &mut State) -> usize {
        self.report(at, Leak::Unfollowed("blocks nested this deep"));
        self.havoc(state, at, end);
        self.stack.extend(std::iter::repeat_n(true, results));
        end + 1
    }

    /// Takes everything the code from `at` to `end` may write as secret: the locals it writes,
    /// every byte of memory and every global that changes.
    fn havoc(&self, state: &mut State, at: usize, end: usize) {
        for local in written_between(self.function, at, end) {
            if let Some(slot) = state.locals.get_mut(to_usize(local)) {
                *slot = true;
            }
        }
        everywhere(self.module(), &mut state.memory);
        state.globals = changing(self.module());
    }

    /// Follows the `block` at `at`, and gives the index after its end.
    fn block(&mut self, at: usize, ty: BlockType, state: &mut State) -> usize {
        let function = self.function;
        let Some(&end) = self.shape.ends.get(&at) else {
            return function.body.len();
        };
        let (params, results) = self.module().block_arity(ty);
        if !self.enter() {
            return self.skip(at, end, results, state);
        }
        let base = self.stack.len().saturating_sub(params);
        self.frames
            .push(Frame::new(FrameKind::Block, Some(at), base, 0, results));
        let stop = self.run(at + 1, state);
        self.leave();
        let frame = self.frames.pop();
        self.close(frame, state);
        stop + 1
    }

    /// Follows the `loop` at `at` until what holds at its start settles, and gives the index
    /// after its end: what holds at its start is what it's entered with, joined with what each
    /// branch back to it brings.
    fn looped(&mut self, at: usize, ty: BlockType, state: &mut State) -> usize {
        let function = self.function;
        let Some(&end) = self.shape.ends.get(&at) else {
            return function.body.len();
        };
        let (params, results) = self.module().block_arity(ty);
        if !self.enter() {
            return self.skip(at, end, results, state);
        }
        let base = self.stack.len().saturating_sub(params);
        let entry_stack = self.stack.clone();
        let mut head = state.clone();
        let mut carried = self.top(params);
        let mut rounds = 0;
        loop {
            rounds += 1;
            self.stack.clone_from(&entry_stack);
            self.stack.truncate(base);
            self.stack.extend(carried.iter().copied());
            self.frames
                .push(Frame::new(FrameKind::Loop, Some(at), base, params, results));
            let mut body = head.clone();
            let stop = self.run(at + 1, &mut body);
            let backs = self
                .frames
                .pop()
                .map(|frame| frame.backs)
                .unwrap_or_default();
            let mut next = head.clone();
            let mut arriving = carried.clone();
            for (back, values) in &backs {
                next = next.joined(back);
                arriving = either(&arriving, values);
            }
            if next == head && arriving == carried {
                self.leave();
                let values = self.top(results);
                self.stack.truncate(base);
                self.stack.extend(values);
                *state = body;
                return stop + 1;
            }
            // A loop that hasn't settled takes all it writes as secret, which settles it.
            if rounds >= MAX_ROUNDS || self.follower.steps > BUDGET {
                self.havoc(&mut next, at, end);
                arriving = vec![true; params];
            }
            head = next;
            carried = arriving;
        }
    }

    /// Follows the `if` at `at`, and gives the index after its end.
    fn branch_if(&mut self, at: usize, ty: BlockType, state: &mut State) -> usize {
        let function = self.function;
        let Some(&end) = self.shape.ends.get(&at) else {
            return function.body.len();
        };
        let (params, results) = self.module().block_arity(ty);
        if state.reachable && self.pop() {
            self.took(at);
            self.report(at, Leak::Branch("if"));
        }
        if !self.enter() {
            return self.skip(at, end, results, state);
        }
        let base = self.stack.len().saturating_sub(params);
        let entry_stack = self.stack.clone();
        let mut then_state = state.clone();
        let mut else_state = state.clone();
        self.frames
            .push(Frame::new(FrameKind::If, Some(at), base, 0, results));
        let stop = self.run(at + 1, &mut then_state);
        let last = if self.shape.elses.get(&at) == Some(&stop) {
            let carried = self.top(results);
            if then_state.reachable
                && let Some(frame) = self.frames.last_mut()
            {
                frame.exits.push((then_state, carried));
            }
            self.stack.clone_from(&entry_stack);
            let last = self.run(stop + 1, &mut else_state);
            *state = else_state;
            last
        } else {
            // With no `else`, a false condition goes straight to the end, its parameters its
            // results.
            let carried = entry_stack.get(base..).unwrap_or_default().to_vec();
            if else_state.reachable
                && let Some(frame) = self.frames.last_mut()
            {
                frame.exits.push((else_state, carried));
            }
            *state = then_state;
            stop
        };
        self.leave();
        let frame = self.frames.pop();
        self.close(frame, state);
        last + 1
    }

    /// Ends the block `frame`: what holds after it is what holds on every way out of it.
    fn close(&mut self, frame: Option<Frame>, state: &mut State) {
        let Some(frame) = frame else {
            return;
        };
        let mut joined: Option<(State, Vec<bool>)> = state
            .reachable
            .then(|| (state.clone(), self.top(frame.results)));
        for (exit, values) in frame.exits {
            joined = Some(match joined {
                None => (exit, values),
                Some((known, carried)) => (known.joined(&exit), either(&carried, &values)),
            });
        }
        self.stack.truncate(frame.base);
        match joined {
            Some((after, values)) => {
                *state = after;
                self.stack.extend(values);
            }
            None => {
                state.reachable = false;
                self.stack.extend(std::iter::repeat_n(false, frame.results));
            }
        }
    }

    /// Sends `state` along a branch to the block `depth` levels out.
    fn branch(&mut self, depth: u32, state: State) {
        let Some(index) = self
            .frames
            .len()
            .checked_sub(to_usize(depth).saturating_add(1))
        else {
            return;
        };
        let (kind, params, results) = {
            let frame = &self.frames[index];
            (frame.kind, frame.params, frame.results)
        };
        if kind == FrameKind::Loop {
            let carried = self.top(params);
            self.frames[index].backs.push((state, carried));
        } else {
            let carried = self.top(results);
            self.frames[index].exits.push((state, carried));
        }
    }

    /// What a secret condition of a branch to `depths`, a `what`, is: the count of the innermost
    /// loop it repeats or leaves, when it does, or else a branch.
    fn branch_leak(&self, what: &'static str, depths: &[u32]) -> Leak {
        let mut innermost: Option<usize> = None;
        for depth in depths {
            let Some(target) = self
                .frames
                .len()
                .checked_sub(to_usize(*depth).saturating_add(1))
            else {
                continue;
            };
            // A branch to a loop repeats it; one to a block around a loop leaves it.
            let from = if self.frames[target].kind == FrameKind::Loop {
                target
            } else {
                target + 1
            };
            if let Some(found) = (from..self.frames.len())
                .rev()
                .find(|index| self.frames[*index].kind == FrameKind::Loop)
            {
                innermost = Some(innermost.map_or(found, |known| known.max(found)));
            }
        }
        let function = self.function;
        match innermost {
            Some(index) => Leak::Count(
                what,
                self.frames[index]
                    .opener
                    .and_then(|opener| function.body.get(opener))
                    .and_then(|op| op.line),
            ),
            None => Leak::Branch(what),
        }
    }

    /// Refuses `leak` on the line of the operator at `at`.
    fn report(&mut self, at: usize, leak: Leak) {
        let line = self.function.body.get(at).and_then(|op| op.line);
        self.follower.report(line, &leak);
    }

    /// Keeps the operator at `at` as one that takes a secret operand.
    fn took(&mut self, at: usize) {
        let function = self.function;
        let Some(op) = function.body.get(at) else {
            return;
        };
        self.follower
            .taken
            .entry(op.offset)
            .or_insert_with(|| Taken {
                offset: op.offset,
                line: op.line,
                name: written_name(&op.operator),
                takes: Takes::of(&op.operator),
            });
    }

    /// Takes the top value off the stack: whether it may be secret.
    fn pop(&mut self) -> bool {
        self.stack.pop().unwrap_or(false)
    }

    /// Takes the top `count` values off the stack, the deepest first.
    fn pop_n(&mut self, count: usize) -> Vec<bool> {
        let start = self.stack.len().saturating_sub(count);
        let mut taken = self.stack.split_off(start);
        while taken.len() < count {
            taken.insert(0, false);
        }
        taken
    }

    /// The top `count` values of the stack, the deepest first.
    fn top(&self, count: usize) -> Vec<bool> {
        let start = self.stack.len().saturating_sub(count);
        let mut values = self.stack[start..].to_vec();
        while values.len() < count {
            values.insert(0, false);
        }
        values
    }

    /// Puts a value on the stack: whether it may be secret.
    fn push(&mut self, secret: bool) {
        self.stack.push(secret);
    }

    /// What the operator at `at` took, as the memory rules follow it, in the terms of this call;
    /// nothing when no path they follow reaches it.
    fn operands(&self, at: usize) -> Vec<Value> {
        let records = self.records;
        records
            .get(at)
            .and_then(Option::as_ref)
            .map(|record| {
                record
                    .operands
                    .iter()
                    .map(|value| resolve(value, self.bindings))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Where an access of `reach` bytes through `address`, a value made concrete, may go:
    /// anywhere when the memory rules don't say what the address is.
    fn places(&self, address: Option<&Value>, reach: Reach) -> Vec<Place> {
        let mut found = Vec::new();
        match address {
            Some(address) => places_of(self.module(), address, reach, &mut found),
            None => found.push(Place::Anywhere),
        }
        found
    }

    /// Marks the bytes `places` name as ones that may hold a secret.
    fn taint(&self, state: &mut State, places: &[Place]) {
        for place in places {
            match place {
                Place::Region {
                    region, start, end, ..
                } => state.memory.mark(*region, *start, *end),
                Place::Anywhere => everywhere(self.module(), &mut state.memory),
            }
        }
    }

    /// What a write of a value, `secret` or not, through `places` does: one exact place then
    /// holds it; otherwise a secret may be in any of them, and a value that isn't changes
    /// nothing.
    fn write(&self, state: &mut State, places: &[Place], secret: bool) {
        match places {
            [
                Place::Region {
                    region,
                    start,
                    end,
                    exact: true,
                },
            ] if !secret => state.memory.clear(*region, *start, *end),
            _ if secret => self.taint(state, places),
            _ => {}
        }
    }

    /// Follows the operator at `at`, which isn't a block.
    fn step(&mut self, at: usize, state: &mut State) {
        if !state.reachable {
            return;
        }
        self.follower.steps += 1;
        let function = self.function;
        let operator = &function.body[at].operator;
        match operator {
            Operator::Unreachable => state.reachable = false,
            Operator::Nop | Operator::DataDrop { .. } => {}
            Operator::Br { relative_depth } => {
                self.branch(*relative_depth, state.clone());
                state.reachable = false;
            }
            Operator::BrIf { relative_depth } => {
                if self.pop() {
                    self.took(at);
                    let leak = self.branch_leak("br_if", &[*relative_depth]);
                    self.report(at, leak);
                }
                self.branch(*relative_depth, state.clone());
            }
            Operator::BrTable { targets } => {
                let mut depths: Vec<u32> = targets.targets().flatten().collect();
                depths.push(targets.default());
                if self.pop() {
                    self.took(at);
                    let leak = self.branch_leak("br_table", &depths);
                    self.report(at, leak);
                }
                for depth in depths {
                    self.branch(depth, state.clone());
                }
                state.reachable = false;
            }
            Operator::Return => {
                let depth = to_u32(self.frames.len().saturating_sub(1));
                self.branch(depth, state.clone());
                state.reachable = false;
            }
            Operator::Call { function_index } => self.call(at, *function_index, state),
            Operator::Drop => {
                self.pop();
            }
            Operator::Select | Operator::TypedSelect { .. } => {
                let condition = self.pop();
                let second = self.pop();
                let first = self.pop();
                if condition {
                    self.report(at, Leak::Select);
                }
                if first || second || condition {
                    self.took(at);
                }
                self.push(first || second || condition);
            }
            Operator::LocalGet { local_index } => {
                let secret = state
                    .locals
                    .get(to_usize(*local_index))
                    .copied()
                    .unwrap_or(false);
                self.push(secret);
            }
            Operator::LocalSet { local_index } => {
                let secret = self.pop();
                if let Some(slot) = state.locals.get_mut(to_usize(*local_index)) {
                    *slot = secret;
                }
            }
            Operator::LocalTee { local_index } => {
                let secret = self.pop();
                if let Some(slot) = state.locals.get_mut(to_usize(*local_index)) {
                    *slot = secret;
                }
                self.push(secret);
            }
            Operator::GlobalGet { global_index } => {
                let secret = state.globals.contains(global_index);
                self.push(secret);
            }
            Operator::GlobalSet { global_index } => {
                if self.pop() {
                    self.took(at);
                    state.globals.insert(*global_index);
                } else {
                    state.globals.remove(global_index);
                }
            }
            Operator::MemoryCopy { .. } => self.copy(at, state),
            Operator::MemoryFill { .. } => self.fill(at, "memory.fill", state),
            Operator::MemoryInit { .. } => self.fill(at, "memory.init", state),
            _ => match memory::memory_op(operator) {
                Some(access) => self.access(at, access, state),
                None => self.generic(at, operator),
            },
        }
    }

    /// Follows the load or store at `at`, `(memarg, width, store, takes, name)`.
    fn access(
        &mut self,
        at: usize,
        (memarg, width, store, takes, name): memory::MemoryOp,
        state: &mut State,
    ) {
        let operands = self.pop_n(takes);
        if operands.contains(&true) {
            self.took(at);
        }
        let address = operands.first().copied().unwrap_or(false);
        if address {
            self.report(at, Leak::Address(name));
        }
        let through = self
            .operands(at)
            .first()
            .map(|value| memory::shifted(value, memarg.offset));
        let places = self.places(through.as_ref(), Reach::Bytes(width));
        let other = operands.get(1).copied().unwrap_or(false);
        if store {
            self.write(state, &places, other || address);
        } else {
            // A lane's load takes a vector too, whose other lanes stay as they were.
            let lane = takes == 2 && other;
            let loaded = holds(&state.memory, &places) || address || lane;
            self.push(loaded);
        }
    }

    /// Follows the `memory.copy` at `at`: its destination holds what its source held.
    fn copy(&mut self, at: usize, state: &mut State) {
        let operands = self.pop_n(3);
        if operands.contains(&true) {
            self.took(at);
        }
        if operands[0] || operands[1] {
            self.report(at, Leak::Address("memory.copy"));
        }
        if operands[2] {
            self.report(at, Leak::Length("memory.copy"));
        }
        let values = self.operands(at);
        let reach = values.get(2).map_or(Reach::Unbounded, reach_of);
        let from = self.places(values.get(1), reach);
        let to = written(self.places(values.first(), reach), values.get(2));
        let secret = holds(&state.memory, &from) || operands.contains(&true);
        self.write(state, &to, secret);
    }

    /// Follows the `memory.fill`, or the `memory.init` of a data segment, which is never secret,
    /// at `at`: its destination holds its value.
    fn fill(&mut self, at: usize, name: &'static str, state: &mut State) {
        let operands = self.pop_n(3);
        if operands.contains(&true) {
            self.took(at);
        }
        // `memory.init` takes an offset into its segment where `memory.fill` takes the value.
        if operands[0] || (operands[1] && name == "memory.init") {
            self.report(at, Leak::Address(name));
        }
        if operands[2] {
            self.report(at, Leak::Length(name));
        }
        let values = self.operands(at);
        let reach = values.get(2).map_or(Reach::Unbounded, reach_of);
        let to = written(self.places(values.first(), reach), values.get(2));
        self.write(state, &to, operands.contains(&true));
    }

    /// Follows an operator the rule knows by its arity alone: what it gives is secret when
    /// anything it takes is, and a division or a float operator takes no secret.
    fn generic(&mut self, at: usize, operator: &Operator<'_>) {
        let (takes, gives) = self.module().arity(&[], operator);
        let operands = self.pop_n(takes);
        let secret = operands.contains(&true);
        if secret {
            self.took(at);
        }
        if secret && divides(operator) {
            self.report(at, Leak::Division(wat_name(operator)));
        } else if secret && floats(operator) {
            self.report(at, Leak::Float(wat_name(operator)));
        }
        self.stack.extend(std::iter::repeat_n(secret, gives));
    }

    /// Follows the call at `at` of function `index`.
    fn call(&mut self, at: usize, index: u32, state: &mut State) {
        let module = self.module();
        let Some(callee) = module.function(index) else {
            return;
        };
        let secret = self.pop_n(callee.params.len());
        if secret.contains(&true) {
            self.took(at);
        }
        match callee.kind {
            Kind::Own => self.own(at, callee, secret, state),
            Kind::Library => self.library(at, callee, &secret, state),
            Kind::Import | Kind::LibraryImport => self.host(at, callee, &secret, state),
        }
        if callee.never_returns() {
            state.reachable = false;
        }
    }

    /// Follows the call at `at` of `callee`, one of main.wat's own functions, whose arguments'
    /// secrets are `secret`: once for each way it's called.
    fn own(&mut self, at: usize, callee: &Function<'_>, secret: Vec<bool>, state: &mut State) {
        let call = Call {
            secret,
            bindings: self.operands(at),
            memory: state.memory.clone(),
            globals: state.globals.clone(),
        };
        let line = self.function.body.get(at).and_then(|op| op.line);
        let outcome = self
            .follower
            .follow(callee.index, call, line, callee.results.len());
        state.memory = outcome.memory;
        state.globals = outcome.globals;
        self.stack.extend(outcome.results);
        if !outcome.returns {
            state.reachable = false;
        }
    }

    /// Refuses each secret argument of the call at `at` of `callee`.
    fn arguments(&mut self, at: usize, callee: &str, secret: &[bool]) {
        for (index, is_secret) in secret.iter().enumerate() {
            if *is_secret {
                self.report(at, Leak::Argument(callee.to_owned(), index + 1));
            }
        }
    }

    /// Follows the call at `at` of `callee`, a function of lengua's library, by what it does:
    /// `$fill` reads, `$write_all` writes out, `$assert_fail` ends the program.
    fn library(&mut self, at: usize, callee: &Function<'_>, secret: &[bool], state: &mut State) {
        self.arguments(at, &callee.shown(), secret);
        if callee.is_library("fill") {
            self.fill_call(at, state);
            return;
        }
        if callee.is_library("write_all") {
            // A secret it writes to a file or a connection may be read back.
            let operands = self.operands(at);
            let reach = operands.get(2).map_or(Reach::Unbounded, reach_of);
            let data = self.places(operands.get(1), reach);
            if holds(&state.memory, &data) && may_be_opened(operands.first()) {
                self.follower.sent = true;
            }
        }
        // `$write_all` sends its buffer's bytes and gives an error number; what any other
        // gives is secret when an argument is.
        let given = !callee.is_library("write_all") && secret.contains(&true);
        self.stack
            .extend(std::iter::repeat_n(given, callee.results.len()));
    }

    /// Follows the call of `$fill` at `at`: what it reads from a secret input is secret, but how
    /// many bytes it read, unless it reads up to a stop byte, which it compares with the last
    /// byte of each read.
    fn fill_call(&mut self, at: usize, state: &mut State) {
        let operands = self.operands(at);
        let read = self.secret_read(operands.first());
        let to_end = operands
            .get(3)
            .is_some_and(|stop| stop.range().constant() == Some(READ_TO_END));
        if read && !to_end {
            self.report(at, Leak::Stop);
        }
        if read {
            let reach = operands.get(2).map_or(Reach::Unbounded, reach_of);
            let places = self.places(operands.get(1), reach);
            self.taint(state, &places);
        }
        self.push(read && !to_end);
    }

    /// Whether a read of the descriptor `fd` may read a secret: standard input's, when it may be
    /// 0; and when it may be one a program opens or a connection's, a file's, or what the
    /// program wrote to a file or sent to a peer.
    fn secret_read(&self, fd: Option<&Value>) -> bool {
        let sources = self.follower.sources;
        (sources.standard_input && may_be_input(fd))
            || ((sources.files || sources.returned) && may_be_opened(fd))
    }

    /// The buffers the iovecs of the host call at `at` point to, each as the places it may be;
    /// `None` when the memory rules can't say what the iovecs hold.
    fn iovec_places(&self, at: usize) -> Option<Vec<Vec<Place>>> {
        let records = self.records;
        let entries = records
            .get(at)
            .and_then(Option::as_ref)
            .and_then(|record| record.iovecs.as_ref())?;
        let buffers = entries.iter().map(|(buffer, length)| {
            let buffer = resolve(buffer, self.bindings);
            let reach = reach_of(&resolve(length, self.bindings));
            self.places(Some(&buffer), reach)
        });
        Some(buffers.collect())
    }

    /// Marks the buffers the iovecs of the host call at `at` point to as ones that may hold a
    /// secret, which a read put there: every byte, when the rule can't say where they are.
    fn taint_iovecs(&self, at: usize, state: &mut State) {
        match self.iovec_places(at) {
            Some(buffers) => {
                for places in &buffers {
                    self.taint(state, places);
                }
            }
            None => everywhere(self.module(), &mut state.memory),
        }
    }

    /// Whether the data the host call at `at` writes out, the buffers its iovecs point to, may
    /// hold a secret: any secret there is, when the rule can't say where they are.
    fn sends_secret(&self, at: usize, state: &State) -> bool {
        match self.iovec_places(at) {
            Some(buffers) => buffers.iter().any(|places| holds(&state.memory, places)),
            None => state.memory.any(),
        }
    }

    /// Follows the call at `at` of `callee`, a host function: the host reads no secret but the
    /// data it writes out, and writes the secret inputs' bytes where it puts them.
    fn host(&mut self, at: usize, callee: &Function<'_>, secret: &[bool], state: &mut State) {
        let shown = callee.shown();
        self.arguments(at, &shown, secret);
        let operands = self.operands(at);
        if let Some(host) = callee.host() {
            for lend in host.lends {
                let reach = lend_reach(lend.bytes, &operands);
                let places = self.places(operands.get(lend.argument), reach);
                if !lend.written {
                    // A datagram's bytes are data the program sends out, as `sock_send`'s are: a
                    // secret among them may come back. Any other buffer the host reads decides
                    // what it does: the name of a computer goes to the resolver and the network,
                    // and a sender's address says where a datagram goes.
                    let data =
                        matches!(host.name, "udp_send" | "udp_send_to") && lend.argument == 1;
                    if data {
                        if holds(&state.memory, &places) {
                            self.follower.sent = true;
                        }
                        continue;
                    }
                    if holds(&state.memory, &places) {
                        self.report(at, Leak::Buffer(shown.clone(), lend.argument + 1));
                    }
                    continue;
                }
                // The arguments' text, or the variables', all of it when one of them is secret.
                let text = match host.name {
                    "args_get" => self.follower.sources.arguments,
                    "environ_get" => self.follower.sources.environment,
                    _ => false,
                };
                if text && lend.argument == 1 {
                    self.taint(state, &places);
                }
                // What the network gives, a datagram or the addresses of a name, is secret once
                // the program has sent a secret, as what `sock_recv` reads is.
                let received = matches!(
                    (host.name, lend.argument),
                    ("udp_recv" | "udp_recv_from", 1) | ("dns_lookup", 2)
                );
                if received && !crate::reach::UNDEFENDED && self.follower.sources.returned {
                    self.taint(state, &places);
                }
            }
            let sources = self.follower.sources;
            // The buffers its iovecs point to get what it reads: `sock_recv` reads no file and
            // no standard input, only what a peer sends, which is secret once the program has
            // sent a secret.
            let reads = match host.name {
                "fd_read" => self.secret_read(operands.first()),
                "sock_recv" => sources.returned,
                _ => false,
            };
            if reads {
                self.taint_iovecs(at, state);
            }
            // How many bytes one read of a secret standard input gave says where a line of it
            // ends, as a terminal gives a line at a time.
            if host.name == "fd_read" && sources.standard_input && may_be_input(operands.first()) {
                let count = self.places(operands.get(3), Reach::Bytes(COUNT_BYTES));
                self.taint(state, &count);
            }
            // A secret written to a file or sent to a peer may be read back.
            let sends = match host.name {
                "fd_write" => may_be_opened(operands.first()),
                "sock_send" => true,
                _ => false,
            };
            if sends && self.sends_secret(at, state) {
                self.follower.sent = true;
            }
        }
        self.stack
            .extend(std::iter::repeat_n(false, callee.results.len()));
    }
}

/// Each of `a`, or the matching one of `b`: what may be secret where paths meet.
fn either(a: &[bool], b: &[bool]) -> Vec<bool> {
    a.iter().zip(b).map(|(x, y)| *x || *y).collect()
}

/// Whether any of `places` may hold a secret in `memory`.
fn holds(memory: &Marks, places: &[Place]) -> bool {
    places.iter().any(|place| match place {
        Place::Region {
            region, start, end, ..
        } => memory.holds(*region, *start, *end),
        Place::Anywhere => memory.any(),
    })
}

/// Marks every byte of every region of `module`, and the bytes outside them all, as ones that
/// may hold a secret: what a write the rule doesn't follow may reach.
fn everywhere(module: &Module<'_>, memory: &mut Marks) {
    for (index, region) in module.regions.iter().enumerate() {
        memory.mark(index, 0, region.size);
    }
    memory.outside = true;
}

/// The places a `memory.copy`, `memory.fill` or `memory.init` of `length` bytes writes, of the
/// `places` it may write: all their bytes on every run only when the length is one constant, so
/// with any other length they are bytes it may write, and a value that isn't secret makes none
/// of them public.
fn written(mut places: Vec<Place>, length: Option<&Value>) -> Vec<Place> {
    let constant = length.is_some_and(|length| length.range().constant().is_some());
    if !constant {
        for place in &mut places {
            if let Place::Region { exact, .. } = place {
                *exact = false;
            }
        }
    }
    places
}

/// The numbers the descriptor `fd` may be: any, for one the rule knows nothing of.
fn descriptor(fd: Option<&Value>) -> memory::Range {
    fd.map_or_else(memory::Range::full, Value::range)
}

/// Whether the descriptor `fd` may be standard input's, 0.
fn may_be_input(fd: Option<&Value>) -> bool {
    descriptor(fd).low == u64::from(fds::STDIN)
}

/// Whether the descriptor `fd` may be one a program opens, a file's, or a connection's: those
/// are numbered from the first connection's up.
fn may_be_opened(fd: Option<&Value>) -> bool {
    descriptor(fd).high >= u64::from(fds::FIRST_CONNECTION)
}

/// Every global of `module` that changes.
fn changing(module: &Module<'_>) -> BTreeSet<u32> {
    module
        .globals
        .iter()
        .filter(|global| global.mutable)
        .map(|global| global.index)
        .collect()
}

/// The locals the code of `function` from `at` to `end` writes.
fn written_between(function: &Function<'_>, at: usize, end: usize) -> BTreeSet<u32> {
    function
        .body
        .iter()
        .take(end)
        .skip(at)
        .filter_map(|op| match op.operator {
            Operator::LocalSet { local_index } | Operator::LocalTee { local_index } => {
                Some(local_index)
            }
            _ => None,
        })
        .collect()
}

/// `value`, a value of the function being followed, in the terms of its call, whose arguments
/// are `bindings`: a parameter is what its caller passed, moved by its offset.
fn resolve(value: &Value, bindings: &[Value]) -> Value {
    match value {
        Value::Param { param, offset } => match bindings.get(to_usize(*param)) {
            Some(bound) if offset.constant() == Some(0) => bound.clone(),
            Some(bound) => moved(bound, *offset),
            None => Value::Mixed { number: true },
        },
        Value::Several(items) => {
            Value::Several(items.iter().map(|item| resolve(item, bindings)).collect())
        }
        other => other.clone(),
    }
}

/// The address `value` moved by `by`: anything but an address, the rule can't move.
fn moved(value: &Value, by: Offset) -> Value {
    match value {
        Value::Address { region, offset } => Value::Address {
            region: *region,
            offset: added(*offset, by),
        },
        Value::Several(items) => Value::Several(items.iter().map(|item| moved(item, by)).collect()),
        Value::Loaded => Value::Loaded,
        _ => Value::Mixed { number: true },
    }
}

/// Two offsets added together.
fn added(a: Offset, b: Offset) -> Offset {
    Offset {
        low: a.low.zip(b.low).and_then(|(x, y)| x.checked_add(y)),
        high: a.high.zip(b.high).and_then(|(x, y)| x.checked_add(y)),
        checked: a.checked && b.checked,
        linear: None,
        index: None,
    }
}

/// Each place `address`, a value made concrete, may name for an access of `reach` bytes, added
/// to `found`.
fn places_of(module: &Module<'_>, address: &Value, reach: Reach, found: &mut Vec<Place>) {
    match address {
        Value::Address { region, offset } => found.push(place(module, *region, *offset, reach)),
        Value::Several(items) => {
            for item in items {
                places_of(module, item, reach, found);
            }
        }
        _ => found.push(Place::Anywhere),
    }
}

/// Where an access of `reach` bytes at `offset` into region `region` of `module` goes: the bytes
/// it may touch, when the memory rules bound them inside the region; anywhere when they lie
/// outside it, or when the rules bound them on neither side and leave them to the proof, which
/// holds them inside the memory alone.
fn place(module: &Module<'_>, region: usize, offset: Offset, reach: Reach) -> Place {
    inside(module, region, offset, reach).unwrap_or(Place::Anywhere)
}

/// The bytes of [`place`], when the memory rules bound them inside the region.
fn inside(module: &Module<'_>, region: usize, offset: Offset, reach: Reach) -> Option<Place> {
    let size = module.regions.get(region)?.size;
    let start = u64::try_from(offset.low?).ok()?;
    let end = match reach {
        Reach::Bytes(bytes) => u64::try_from(offset.high?).ok()?.checked_add(bytes)?,
        Reach::Rest => size,
        Reach::Unbounded => return None,
    };
    (start <= end && end <= size).then_some(Place::Region {
        region,
        start,
        end,
        exact: offset.constant().is_some() && matches!(reach, Reach::Bytes(_)),
    })
}

/// How far an access of `length` bytes reaches: as many as the memory rules let it be, or
/// anywhere when they don't bound it.
fn reach_of(length: &Value) -> Reach {
    let range = length.range();
    if range.high <= u64::from(u32::MAX) {
        Reach::Bytes(range.high)
    } else {
        Reach::Unbounded
    }
}

/// How far a buffer of `bytes` that a host call borrows reaches, given the call's `operands`.
fn lend_reach(bytes: Bytes, operands: &[Value]) -> Reach {
    match bytes {
        Bytes::Fixed(count) => Reach::Bytes(u64::from(count)),
        Bytes::Argument(argument) => operands.get(argument).map_or(Reach::Unbounded, reach_of),
        Bytes::Table { count, entry } => match operands.get(count).map(Value::range) {
            Some(range) if range.high <= u64::from(u32::MAX) => {
                Reach::Bytes(range.high.saturating_mul(u64::from(entry)))
            }
            _ => Reach::Unbounded,
        },
        Bytes::Sized => Reach::Rest,
    }
}

/// Whether `operator` divides integers or takes a remainder.
fn divides(operator: &Operator<'_>) -> bool {
    matches!(
        operator,
        Operator::I32DivS
            | Operator::I32DivU
            | Operator::I32RemS
            | Operator::I32RemU
            | Operator::I64DivS
            | Operator::I64DivU
            | Operator::I64RemS
            | Operator::I64RemU
    )
}

/// Whether `operator` computes with floats: any operator on `f32` or `f64` values, alone or in
/// vectors, from them or to them, but a load, a store or a constant, which only move bits.
fn floats(operator: &Operator<'_>) -> bool {
    if matches!(
        operator,
        Operator::F32Load { .. }
            | Operator::F64Load { .. }
            | Operator::F32Store { .. }
            | Operator::F64Store { .. }
            | Operator::F32Const { .. }
            | Operator::F64Const { .. }
    ) {
        return false;
    }
    let name = variant(operator);
    name.contains("F32") || name.contains("F64")
}

/// The name of `operator`'s variant in `wasmparser`, such as `F64Add`.
fn variant(operator: &Operator<'_>) -> String {
    let shown = format!("{operator:?}");
    shown
        .split([' ', '{', '('])
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// The name main.wat writes `operator` with, made from its variant's: `F64Add` is `f64.add`,
/// `I32TruncSatF32S` is `i32.trunc_sat_f32_s`.
fn wat_name(operator: &Operator<'_>) -> String {
    const TYPES: [&str; 11] = [
        "I8x16", "I16x8", "I32x4", "I64x2", "F32x4", "F64x2", "V128", "I32", "I64", "F32", "F64",
    ];
    let full = variant(operator);
    let Some(prefix) = TYPES.iter().find(|prefix| full.starts_with(**prefix)) else {
        return full.to_lowercase();
    };
    let mut name = prefix.to_lowercase();
    name.push('.');
    for (index, character) in full[prefix.len()..].chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 {
                name.push('_');
            }
            name.push(character.to_ascii_lowercase());
        } else {
            name.push(character);
        }
    }
    name
}

/// The name main.wat writes `operator` with, for the operators that take a secret: [`wat_name`]
/// for those named by a type, and the names of the others.
fn written_name(operator: &Operator<'_>) -> String {
    let name = match operator {
        Operator::If { .. } => "if",
        Operator::BrIf { .. } => "br_if",
        Operator::BrTable { .. } => "br_table",
        Operator::Call { .. } => "call",
        Operator::Select | Operator::TypedSelect { .. } => "select",
        Operator::GlobalSet { .. } => "global.set",
        Operator::MemoryCopy { .. } => "memory.copy",
        Operator::MemoryFill { .. } => "memory.fill",
        Operator::MemoryInit { .. } => "memory.init",
        other => return wat_name(other),
    };
    name.to_owned()
}

/// Checks that nothing `secrets` names, the inputs of the idea's `- Secret:` line, decides how
/// main.wat runs, following it from `_start`; nothing to check when the line names nothing.
/// Gives every operator that takes a secret operand on a path it follows, in the module's order:
/// what the check of the machine code holds the engine's code to ([`super::machine`]).
pub fn check(
    module: &Module<'_>,
    flows: &Flows,
    analysis: &Analysis,
    secrets: &[Source],
    violations: &mut Vec<Violation>,
) -> Vec<Taken> {
    if secrets.is_empty() {
        return Vec::new();
    }
    let Some(entry) = module.entry() else {
        return Vec::new();
    };
    let Some(start) = module
        .function(entry)
        .filter(|function| function.kind == Kind::Own)
    else {
        return Vec::new();
    };
    let mut sources = Sources::of(secrets);
    let mut found = BTreeSet::new();
    let mut taken = BTreeMap::new();
    // Followed once; and once more when it may write a secret to a file or send one to a peer,
    // with what every file and connection gives secret, wherever in the program it reads them.
    loop {
        let mut follower = Follower {
            module,
            flows,
            analysis,
            sources,
            known: BTreeMap::new(),
            calling: Vec::new(),
            depth: 0,
            steps: 0,
            sent: false,
            found: BTreeSet::new(),
            taken: BTreeMap::new(),
        };
        let call = Call {
            secret: Vec::new(),
            bindings: Vec::new(),
            memory: Marks::default(),
            globals: BTreeSet::new(),
        };
        follower.follow(entry, call, start.line, start.results.len());
        found.append(&mut follower.found);
        taken.append(&mut follower.taken);
        if sources.returned || !follower.sent {
            break;
        }
        sources.returned = true;
    }
    violations.extend(found.into_iter().map(|(line, problem, fix)| Violation {
        rule: Rule::Secrets,
        line,
        problem,
        fix,
    }));
    taken.into_values().collect()
}
