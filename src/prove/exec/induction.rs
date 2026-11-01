//! A run's part in a proof by induction over a streaming program's read loop, whose head is
//! fill's. The base, every input up to one buffer-full, runs with the head made arbitrary
//! wherever one more buffer-full can change it; the step runs one buffer-full from that same
//! arbitrary head back to it, where what changed is compared with where it started.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use super::{End, Last, Memory, Owner, Return, STACK_SIZE, STACK_TOP, State, VECTOR_BYTES};
use crate::prove::term::{Pool, Term};

/// How a run takes the read loop's head.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hook {
    /// fill's loop head.
    pub head: u64,
    /// The memory bytes one buffer-full can change, made arbitrary at the head.
    pub memory: BTreeSet<u64>,
    /// The general registers it can change, other than the bound counters.
    pub registers: BTreeSet<usize>,
    /// The vector registers it can change.
    pub vectors: BTreeSet<usize>,
    /// The mask registers it can change.
    pub masks: BTreeSet<usize>,
    /// The loop's bound counters, by register, each with the value it starts at: made any value
    /// from `low` to that.
    pub counters: BTreeMap<usize, u128>,
    /// The least a bound counter is at the head: enough for the loop to take what's left.
    pub low: u128,
    /// The counts the loop keeps as it goes, as `len` and `count` fold the text, by register.
    pub folds: BTreeMap<usize, Fold>,
    /// Whether the run stops when it comes back to the head, as the step's does.
    pub stop: bool,
    /// How many buffer-fulls a stopping run reads from the head before it stops there: two for
    /// a count that needs the bytes before each buffer-full, whose second buffer-full is then
    /// checked after the first's last bytes. None given means one.
    pub fulls: u32,
    /// For the step: the characters of the input it reads from the head, fresh bytes one more
    /// than the buffer-fulls fill is given there, so the loop goes round again.
    pub feed: Option<Vec<u8>>,
    /// For the base of a count that needs the bytes before each buffer-full: the characters of
    /// a fresh buffer-full put before what's left of standard input at the head, so each final
    /// piece follows a whole buffer-full read from any state the loop can be in.
    pub lead: Option<Vec<u8>>,
}

impl Hook {
    /// The buffer-fulls a stopping run reads from the head.
    #[must_use]
    pub fn fulls(&self) -> u32 {
        self.fulls.max(1)
    }
}

/// A count a read loop keeps in a register as it goes: at the head, what it starts at plus the
/// count of everything read so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fold {
    /// What the register holds the first time at the head.
    pub start: u128,
    /// The most the count of what was read before the head can be.
    pub most: u128,
    /// The output's placeholder it counts for, by its place among the template's pieces.
    pub placeholder: usize,
}

impl Fold {
    /// The register's value at the head, any from its start to its start and its most.
    pub fn value(&self, pool: &mut Pool, register: usize) -> Term {
        pool.var_in(
            64,
            &format!("induction.f{register}"),
            self.start,
            self.start + self.most,
        )
    }
}

/// Where a run stood the first time it reached the head.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// The general registers.
    pub registers: [Term; super::SLOTS],
    memory: Memory,
    vectors: Vec<Rc<[Term; VECTOR_BYTES]>>,
    masks: [Term; 8],
    owners: [Option<Owner>; super::SLOTS],
    pointers: BTreeMap<u64, Owner>,
    returns: Vec<Return>,
    threads: u64,
    /// How much it had written to standard output.
    pub stdout: usize,
    /// How much to standard error.
    pub stderr: usize,
    /// How much of standard input it had read.
    pub stdin_read: usize,
    /// Where the stack's free part ends: the stack pointer, when it's known.
    stack: Option<u64>,
}

/// Where a run stood the second time it reached the head, one buffer-full on from the first.
#[derive(Debug, Clone)]
pub struct Second {
    /// The general registers.
    pub registers: [Term; super::SLOTS],
    /// What that buffer-full changed, for a run that reads two from the head.
    pub changes: Option<Changes>,
    /// How much it had written to standard output.
    pub stdout: usize,
    /// How much to standard error.
    pub stderr: usize,
    /// How much of standard input it had read.
    pub stdin_read: usize,
}

/// What differs between where a run stood at the head and where it stands back at it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    /// Memory bytes, other than the stack below the head's stack pointer, which is free.
    pub memory: BTreeSet<u64>,
    /// General registers.
    pub registers: BTreeSet<usize>,
    /// Vector registers.
    pub vectors: BTreeSet<usize>,
    /// Mask registers.
    pub masks: BTreeSet<usize>,
    /// What else differs, which no hook makes arbitrary: the calls under way, threads, files.
    pub other: Vec<String>,
}

impl Memory {
    /// Forgets every byte from `start` up to `end`, going through the pages that hold some.
    fn clear(&mut self, start: u64, end: u64) {
        let pages: Vec<u64> = self
            .pages
            .range(start / super::PAGE as u64..=end.saturating_sub(1) / super::PAGE as u64)
            .map(|(page, _)| *page)
            .collect();
        for page in pages {
            let base = page * super::PAGE as u64;
            if let Some(bytes) = self.pages.get_mut(&page) {
                let bytes = Rc::make_mut(bytes);
                for (offset, byte) in bytes.iter_mut().enumerate() {
                    let address = base + offset as u64;
                    if address >= start && address < end {
                        *byte = None;
                    }
                }
            }
        }
    }
}

/// The stack below `stack`, which holds nothing the run can read before writing it.
fn free_below(stack: Option<u64>, address: u64) -> bool {
    stack.is_some_and(|stack| address < stack && address >= STACK_TOP - STACK_SIZE)
}

impl State {
    /// Takes the read loop's head for `hook`: counts the arrival; the first time, feeds the
    /// step's input, makes what one buffer-full can change arbitrary, and notes where the run
    /// stands; the second time, the step's run ends there.
    ///
    /// # Errors
    ///
    /// Ends the step's run back at the head.
    pub(super) fn arrive(&mut self, pool: &mut Pool, hook: &Hook) -> Result<(), End> {
        self.arrivals += 1;
        if hook.stop && self.arrivals == hook.fulls() + 1 {
            return Err(End::Looped);
        }
        if self.arrivals == 2 {
            // One buffer-full on: where a count that looks back is checked from.
            if let Some(start) = self.head.as_deref() {
                let changes = (hook.fulls() > 1).then(|| self.changes(start));
                self.second = Some(Box::new(Second {
                    registers: self.registers,
                    changes,
                    stdout: self.stdout.len(),
                    stderr: self.stderr.len(),
                    stdin_read: self.stdin_read,
                }));
            }
            return Ok(());
        }
        if self.arrivals != 1 {
            return Ok(());
        }
        let constant = |pool: &Pool, term: Term| {
            pool.constant(term)
                .and_then(|value| usize::try_from(value).ok())
        };
        if let Some(characters) = &hook.feed
            && self.stdin_read == 0
            && let Some(space) = constant(
                pool,
                self.registers[super::fill::FillAbi::of(self.isa).space],
            )
        {
            let fulls = usize::try_from(hook.fulls()).unwrap_or(1);
            let input =
                crate::prove::text_input(pool, "induction.chunk", space * fulls + 1, characters);
            self.path.extend(input.constraints);
            self.stdin = Rc::new(input.text);
        }
        if let Some(characters) = &hook.lead
            && let Some(space) = constant(
                pool,
                self.registers[super::fill::FillAbi::of(self.isa).space],
            )
        {
            let lead = crate::prove::text_input(pool, "induction.lead", space, characters);
            self.path.extend(lead.constraints);
            let read = self.stdin_read.min(self.stdin.len());
            let mut stdin = self.stdin[..read].to_vec();
            stdin.extend(lead.text);
            stdin.extend_from_slice(&self.stdin[read..]);
            self.stdin = Rc::new(stdin);
        }
        for address in &hook.memory {
            let byte = pool.var(8, &format!("induction.m{address:x}"));
            self.memory.insert(*address, byte);
            self.pointers.remove(address);
        }
        self.stored.clear();
        // The stack below the head's stack pointer holds what earlier calls left, which differs
        // from one time round to the next: nothing there can be read before it's written.
        let stack = pool
            .constant(self.registers[super::calls::Convention::of(self.isa).stack])
            .and_then(|value| u64::try_from(value).ok());
        if let Some(stack) = stack {
            let bottom = STACK_TOP - STACK_SIZE;
            self.memory.clear(bottom, stack);
            self.pointers.retain(|at, _| !free_below(Some(stack), *at));
        }
        for register in &hook.registers {
            self.registers[*register] = pool.var(64, &format!("induction.r{register}"));
            self.owners[*register] = None;
        }
        for (register, start) in &hook.counters {
            self.registers[*register] =
                pool.var_in(64, &format!("induction.c{register}"), hook.low, *start);
            self.owners[*register] = None;
        }
        for (register, fold) in &hook.folds {
            self.registers[*register] = fold.value(pool, *register);
            self.owners[*register] = None;
        }
        for vector in &hook.vectors {
            let bytes: [Term; VECTOR_BYTES] =
                std::array::from_fn(|at| pool.var(8, &format!("induction.v{vector}.{at}")));
            self.vectors[*vector] = Rc::new(bytes);
        }
        for mask in &hook.masks {
            self.masks[*mask] = pool.var(64, &format!("induction.k{mask}"));
        }
        // The flags don't outlive the call that reaches the head.
        self.carry = pool.bool_var("induction.cf");
        self.zero = pool.bool_var("induction.zf");
        self.sign = pool.bool_var("induction.sf");
        self.overflow = pool.bool_var("induction.of");
        self.parity = pool.bool_var("induction.pf");
        self.last = Last::Other;
        self.head = Some(Box::new(Snapshot {
            registers: self.registers,
            memory: self.memory.clone(),
            vectors: self.vectors.clone(),
            masks: self.masks,
            owners: self.owners,
            pointers: self.pointers.clone(),
            returns: self.returns.clone(),
            threads: self.threads,
            stdout: self.stdout.len(),
            stderr: self.stderr.len(),
            stdin_read: self.stdin_read,
            stack,
        }));
        Ok(())
    }

    /// What differs between `start`, where the run stood at the head, and where it stands now.
    #[must_use]
    pub fn changes(&self, start: &Snapshot) -> Changes {
        let mut changes = Changes::default();
        // The calls under way are the same ones, each a frame of its own: a frame made since
        // stands where the one it replaces stood.
        let same_calls = self.returns.len() == start.returns.len()
            && self.returns.iter().zip(&start.returns).all(|(now, then)| {
                (now.top, now.saved, now.back, now.callee)
                    == (then.top, then.saved, then.back, then.callee)
            });
        if !same_calls {
            changes.other.push(
                "the calls under way differ from one time round the read loop to the next".into(),
            );
        }
        let frame = |owner: Option<Owner>, returns: &[Return]| match owner {
            Some(Owner::Frame(number)) => returns
                .iter()
                .position(|back| back.frame == number)
                .map_or(Some(Owner::Frame(number)), |depth| {
                    Some(Owner::Frame(u64::MAX - depth as u64))
                }),
            other => other,
        };
        let same_owner = |now: Option<Owner>, then: Option<Owner>| {
            frame(now, &self.returns) == frame(then, &start.returns)
        };
        let stack = super::calls::Convention::of(self.isa).stack;
        for (number, (now, then)) in self.registers.iter().zip(&start.registers).enumerate() {
            if now != then
                || (number != stack && !same_owner(self.owners[number], start.owners[number]))
            {
                changes.registers.insert(number);
            }
        }
        for (number, (now, then)) in self.vectors.iter().zip(&start.vectors).enumerate() {
            if !Rc::ptr_eq(now, then) && now[..] != then[..] {
                changes.vectors.insert(number);
            }
        }
        for (number, (now, then)) in self.masks.iter().zip(&start.masks).enumerate() {
            if now != then {
                changes.masks.insert(number);
            }
        }
        let free = |address: u64| free_below(start.stack, address);
        let pages: BTreeSet<u64> = self
            .memory
            .pages
            .keys()
            .chain(start.memory.pages.keys())
            .copied()
            .collect();
        for page in pages {
            let (now, then) = (self.memory.pages.get(&page), start.memory.pages.get(&page));
            if let (Some(now), Some(then)) = (now, then)
                && Rc::ptr_eq(now, then)
            {
                continue;
            }
            for offset in 0..super::PAGE as u64 {
                let address = page * super::PAGE as u64 + offset;
                if self.memory.get(address) != start.memory.get(address) && !free(address) {
                    changes.memory.insert(address);
                }
            }
        }
        // A pointer stored in memory keeps its owner, or its bytes count as changed.
        let stored: BTreeSet<u64> = self
            .pointers
            .keys()
            .chain(start.pointers.keys())
            .copied()
            .collect();
        for address in stored {
            let (now, then) = (self.pointers.get(&address), start.pointers.get(&address));
            if !free(address) && !same_owner(now.copied(), then.copied()) {
                changes.memory.extend(address..address + 8);
            }
        }
        if self.threads != start.threads || !self.suspended.is_empty() {
            changes
                .other
                .push("a time round the read loop starts threads".into());
        }
        if !self.written.is_empty() || !self.open.is_empty() || !self.sockets.is_empty() {
            changes
                .other
                .push("a time round the read loop opens files or connections".into());
        }
        if self.raw_reads {
            changes
                .other
                .push("the read loop reads standard input with read itself, not only fill".into());
        }
        if !self.logs.is_empty() {
            changes.other.push(
                "a time round the read loop reaches memory the input chooses among more places \
                 than a proof follows one by one"
                    .into(),
            );
        }
        changes
    }

    /// Where the run stood at the head, once it has reached it.
    #[must_use]
    pub fn at_head(&self) -> Option<&Snapshot> {
        self.head.as_deref()
    }

    /// Where it stood the second time, one buffer-full on, once it has come back.
    #[must_use]
    pub fn at_second(&self) -> Option<&Second> {
        self.second.as_deref()
    }

    /// Standard input as the run reads it.
    #[must_use]
    pub fn stdin_bytes(&self) -> &[Term] {
        &self.stdin
    }

    /// How much of standard input the run has read.
    #[must_use]
    pub const fn stdin_taken(&self) -> usize {
        self.stdin_read
    }

    /// The general register numbered `number`.
    #[must_use]
    pub const fn register_term(&self, number: usize) -> Term {
        self.registers[number]
    }
}
