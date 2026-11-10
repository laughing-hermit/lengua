//! Runs a program on terms instead of numbers: each value it holds is a term over the inputs. A
//! branch the inputs decide splits the run in two, each side keeping its condition, so the
//! finished runs cover every input of a shape. With concrete inputs every term is a number, and
//! the same code is a plain interpreter, which the prover checks against real runs.
//!
//! A run's [`State`] is its core, the machine its code runs on, and its [`World`], what the run
//! reads, writes and meets beyond the machine. A front end brings its own [`Core`], as the
//! WebAssembly model does ([`super::wasm`]), and [`explore`] follows its runs, calling the
//! world's models for what a call of the host does.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use super::term::{Pool, Term};
use super::world::{World, net};

/// The most steps one run may take before the prover gives up on it.
pub const MAX_STEPS: u64 = 5_000_000;

/// Why a run past [`MAX_STEPS`] proves nothing.
pub const TOO_LONG: &str = "a run took too many instructions to follow";

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// The program exited, with the status's low byte.
    Exit(Term),
    /// Something the prover doesn't model, so this run proves nothing.
    Unsupported(String),
    /// The paths through the code ran past the limit before this one was followed.
    TooManyPaths,
    /// Waiting for the test client's next connection after the last one, as a server that
    /// keeps serving does.
    Serving,
    /// Waiting for something the test's peer never sends, which a real run gives up on: why.
    Stuck(String),
    /// What the engine would stop the program at, such as an access outside its memory: what it
    /// did.
    Unsafe(String),
    /// Back at the read loop's head, where a proof by induction compares the run with where it
    /// started.
    Looped,
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

    /// The addresses whose bytes differ from `before`'s: written since to other bytes, or written
    /// in one of the two and not the other. Pages the two still share are the same.
    #[must_use]
    pub fn differences(&self, before: &Self) -> BTreeSet<u64> {
        let pages: BTreeSet<u64> = self
            .pages
            .keys()
            .chain(before.pages.keys())
            .copied()
            .collect();
        let mut found = BTreeSet::new();
        for page in pages {
            if let (Some(now), Some(then)) = (self.pages.get(&page), before.pages.get(&page))
                && Rc::ptr_eq(now, then)
            {
                continue;
            }
            for offset in 0..PAGE as u64 {
                let address = page * PAGE as u64 + offset;
                if self.get(address) != before.get(address) {
                    found.insert(address);
                }
            }
        }
        found
    }
}

/// What a front end's runs hold beside their [`World`]: the machine its code runs on, and how a
/// run of that code takes a step. [`explore`] follows the runs of any core the same way.
pub trait Core: Clone + std::fmt::Debug {
    /// The program as the front end loads it.
    type Image;

    /// Runs one step of `state`'s code: an instruction, and what the world does for a call of
    /// the host it makes.
    ///
    /// # Errors
    ///
    /// How the run ends, where the step ends it.
    fn step(
        image: &Self::Image,
        state: &mut State<Self>,
        pool: &mut Pool,
    ) -> Result<Step<Self>, End>;

    /// Where `state` takes a branch both sides of which can happen, `state` going one way and
    /// `other` the other: `other` back, to follow as a run of its own, or `None` where the core
    /// keeps a side to merge with the rest later, as a core may merge a loop's exits. The
    /// default keeps nothing.
    fn park(
        _image: &Self::Image,
        _pool: &mut Pool,
        _state: &mut State<Self>,
        other: State<Self>,
        _condition: Term,
    ) -> Option<State<Self>> {
        Some(other)
    }

    /// Merges what the core kept of `state`'s branches, once the run is done with them: into
    /// `state`, or into runs of their own in `pending`. The default keeps none.
    fn settle(_pool: &mut Pool, _state: &mut State<Self>, _pending: &mut Vec<State<Self>>) {}

    /// While the core keeps branches to merge, where the run is still the one it goes on with:
    /// what the run must meet from then on, it must meet only there.
    fn alive(&self) -> Option<Term> {
        None
    }

    /// Hands the branches the core keeps over to `other`, where only `other`'s side of a branch
    /// can happen.
    fn hand_over(&mut self, _other: &mut Self) {}
}

/// One run's state: the machine its code runs on, and the world it runs in.
#[derive(Debug, Clone)]
pub struct State<C> {
    /// The machine, its core's own.
    pub core: C,
    /// The world: standard input, output and error, the files and folders, the network, the
    /// clock and random bytes, and the proof's account of the run.
    pub world: World,
}

impl<C> State<C> {
    /// The same state, its standard input ending after `length` of its bytes, a length the
    /// input chooses: standard input holds as many bytes as it can be long.
    #[must_use]
    pub fn with_stdin_length(mut self, length: Term) -> Self {
        self.world.stdin_length = Some(length);
        self
    }

    /// The same state with `files` in its working folder.
    #[must_use]
    pub fn with_files(mut self, files: BTreeMap<String, Vec<Term>>) -> Self {
        self.world.files = Rc::new(files);
        self
    }

    /// The same state with these empty folders in its working folder.
    #[must_use]
    pub fn with_folders(mut self, folders: BTreeSet<String>) -> Self {
        self.world.folders_before = Rc::new(folders);
        self
    }

    /// The same state with standard output closed, or not: a pipe nobody reads, so every write
    /// to it fails with `EPIPE`.
    #[must_use]
    pub fn with_stdout_closed(mut self, closed: bool) -> Self {
        self.world.stdout_closed = closed;
        self
    }

    /// The same state with standard input arriving in pieces of these lengths.
    #[must_use]
    pub fn with_stdin_pieces(mut self, pieces: Vec<usize>) -> Self {
        self.world.stdin_pieces = Rc::new(pieces);
        self
    }

    /// The same state with the test's network: its server or client, as `network` says.
    #[must_use]
    pub fn with_network(mut self, network: net::Network) -> Self {
        // The connections lengua made before the program started, each at its own number.
        for (index, script) in network.made.iter().enumerate() {
            let number = self.world.connections.len();
            self.world.connections.push(net::Connection::made(*script));
            let fd = u64::try_from(crate::connect::FIRST_DESCRIPTOR).unwrap_or(4) + index as u64;
            self.world.sockets.insert(fd, Some(number));
        }
        self.world.network = Rc::new(network);
        self
    }

    /// The same state with a test's random bytes and clock readings in place of the host's.
    #[must_use]
    pub fn with_supply(mut self, random: Option<Vec<Term>>, clock: Vec<(Term, Term)>) -> Self {
        self.world.random = random.map(Rc::new);
        self.world.clock = Rc::new(clock);
        self
    }

    /// The same state, proving for every way the input can be cut rather than running as one
    /// read gives it.
    #[must_use]
    pub fn for_every_cut(mut self) -> Self {
        self.world.every_cut = true;
        self
    }
}

/// Why a step stopped the run, or the condition it split on, for runs of the core `C`.
pub enum Step<C> {
    /// Go on with the state as changed.
    Next,
    /// Split: the state continues where `condition` holds, and `other` where it doesn't.
    Split(Term, Box<State<C>>),
    /// Several ways the run can go for the same inputs, as a read can be cut, each where its
    /// condition holds: the state lives on as all of them.
    Choose(Vec<(Term, State<C>)>),
}

/// How a step stops a run: how the run ends.
pub(super) type Stop = End;

/// A run that ends on something the prover doesn't model, for `text`.
pub(super) fn unsupported(text: impl Into<String>) -> Stop {
    End::Unsupported(text.into())
}

/// What a run asks of the solvers: whether a branch can go each way.
pub trait Oracle {
    /// Whether `condition` and every one of `path` can hold together; true when unsure.
    ///
    /// # Errors
    ///
    /// Fails when the solver stops responding.
    fn possible(&mut self, pool: &Pool, path: &[Term], condition: Term) -> Result<bool, String>;

    /// Whether the proof still has its time. [`explore`] asks before every step, so a run
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

/// Every run from `start`, splitting at branches the inputs decide, at most `max_runs`: the
/// runs of any core.
///
/// # Errors
///
/// Fails when the oracle does.
pub fn explore<C: Core>(
    image: &C::Image,
    start: State<C>,
    pool: &mut Pool,
    oracle: &mut dyn Oracle,
    max_runs: usize,
) -> Result<Vec<State<C>>, String> {
    let mut pending = vec![start];
    let mut finished = Vec::new();
    let mut looked = pool.len();
    while let Some(mut state) = pending.pop() {
        if finished.len() + pending.len() > max_runs {
            state.world.end = Some(End::TooManyPaths);
            finished.push(state);
            break;
        }
        loop {
            // What the core kept of the run's branches, merged where the run is done with them.
            C::settle(pool, &mut state, &mut pending);
            if state.world.end.is_some() {
                finished.push(state);
                break;
            }
            // Before every step, not every so many: one step over a hash's terms, such as a load
            // at an address the input chooses among hundreds of places, can take a second, and
            // thousands of them between two looks took a proof seven times its budget.
            oracle.in_time()?;
            if pool.len() >= looked + super::term::TERMS_BETWEEN_MEMORY_LOOKS {
                looked = pool.len();
                oracle.has_room()?;
            }
            let obligations = state.world.obligations.len();
            let stepped = C::step(image, &mut state, pool);
            // A step can make terms by the million in one extraction, between two looks of this
            // loop's: the pool looks as it grows, and what it made after finding the memory
            // short is nothing to go on.
            if pool.out_of_room() {
                return Err(super::watch::OUT_OF_MEMORY.into());
            }
            // While a loop's exits are merged, the run that stays must meet what it meets only
            // where it's still in the loop.
            if let Some(alive) = state.core.alive()
                && pool.truth(alive) != Some(true)
            {
                for (condition, _) in &mut state.world.obligations[obligations..] {
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
                    let yes = oracle.possible(pool, &state.world.path, condition)?;
                    let no = !yes || oracle.possible(pool, &other.world.path, opposite)?;
                    // A loop's exit the input's length decides waits to be merged.
                    if yes && no {
                        match C::park(image, pool, &mut state, other, condition) {
                            None => continue,
                            Some(back) => other = back,
                        }
                    }
                    // Where only the other side can happen, it carries on the loop's merging.
                    if !yes {
                        state.core.hand_over(&mut other.core);
                    }
                    if no {
                        other.world.path.push(opposite);
                        pending.push(other);
                    }
                    if yes {
                        state.world.path.push(condition);
                    } else {
                        break;
                    }
                }
                Ok(Step::Choose(choices)) => {
                    for (condition, mut choice) in choices {
                        let certain = pool.truth(condition) == Some(true);
                        if certain || oracle.possible(pool, &choice.world.path, condition)? {
                            if !certain {
                                choice.world.path.push(condition);
                            }
                            pending.push(choice);
                        }
                    }
                    break;
                }
                Err(end) => {
                    // Ended: the loop's top merges any exits it was carrying, then finishes it.
                    state.world.end = Some(end);
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

pub mod induction;
