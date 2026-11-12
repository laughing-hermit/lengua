//! `split` and `piece`, lengua's own: what a program calls to split its work into pieces that
//! run at once, for an idea with a `- Splits:` line ([`crate::split`]).
//!
//! A piece is a run of the program's own module from `_start`, in a store of its own, so with a
//! memory, globals and a stack of its own: nothing a piece writes is anything another piece or
//! the program reads, so there is no data race to rule out. It gets its share of the bytes as
//! its standard input, what it prints is its answer, and its idea's grants are not its own: it
//! reaches nothing else, and it can't split in its turn. The pieces of a call run on threads of
//! lengua's, as many at once as the computer has cores, and the call returns when all have
//! ended.
//!
//! What a call gives depends on its bytes and its numbers alone, never on how many cores there
//! are or on which piece ends first: the cut is arithmetic ([`crate::split::cut`]), a piece
//! reads no clock and no random bytes, the answers are written in the pieces' order, and where
//! a piece doesn't exit with code 0 the run ends as the first such piece in that order ended. A
//! test's second run takes the pieces one at a time, the last first, and must give the same.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use wasmtime::{Linker, Module};

use super::limits::Limits;
use super::policy::Policy;
use super::stdio::{Input, Output};
use super::{Beyond, Fail, Host, Stop, World, memory};
use crate::runner::Trap;
use crate::wasm::engine::Setup;
use crate::wasm::job::Ending;
use crate::wasm::wasi::errno;

/// The stack of the thread a piece runs on: room for the
/// [`crate::wasm::engine::MAX_WASM_STACK`] a program's calls may take, and the host's own calls
/// beneath and above them. A piece's thread compiles nothing.
pub const STACK: usize = 4 * 1024 * 1024;

/// The most a piece writes to standard error, where an assertion that fails says its number:
/// past it, the piece is stopped.
pub const ERRORS: usize = 65_536;

/// What a call that splits its work runs its pieces with.
pub struct Splitting {
    /// The run's own module, which each piece is a run of.
    pub module: Module,
    /// The host's functions, as the run itself has them.
    pub linker: Linker<Host>,
    /// The engine's configuration the run is in.
    pub setup: Setup,
}

/// What stops a piece that still runs once a piece before it, in the pieces' order, didn't end
/// well: its answer can't matter any more.
#[derive(Debug, Clone)]
pub struct Halt {
    /// The first piece, in the pieces' order, that didn't end well so far; `u32::MAX` while
    /// none.
    pub failed: Arc<AtomicU32>,
    /// This piece's place in the order, from 0.
    pub index: u32,
}

impl Halt {
    /// Whether this piece's answer can't matter any more.
    #[must_use]
    pub fn halted(&self) -> bool {
        self.failed.load(Ordering::SeqCst) < self.index
    }
}

/// How one piece went.
struct Ended {
    /// How it ended.
    ending: Ending,
    /// What lengua has to say of it.
    problem: Option<String>,
    /// What it printed: its answer.
    printed: Vec<u8>,
    /// What it wrote to standard error.
    errors: Vec<u8>,
    /// The operations it executed, where the engine counts them.
    counted: u64,
}

impl Ended {
    /// A piece lengua couldn't run, as when the system gives it no thread.
    fn unstarted() -> Self {
        Self {
            ending: Ending::Trapped(Trap::Other, Vec::new()),
            problem: Some(
                "lengua couldn't run a piece of the program's split work: the system gave it no \
                 thread"
                    .to_owned(),
            ),
            printed: Vec::new(),
            errors: Vec::new(),
            counted: 0,
        }
    }

    /// Whether the piece gave its answer: it exited with code 0.
    fn well(&self) -> bool {
        self.ending == Ending::Exited(0)
    }
}

/// One call's pieces, as the threads that run them share them.
struct Work<'a> {
    /// What the pieces run with.
    splitting: &'a Splitting,
    /// What a piece may reach.
    policy: Policy,
    /// The bytes the call splits.
    bytes: &'a [u8],
    /// How many pieces.
    pieces: u32,
    /// How many bytes a piece gives back at most.
    back: usize,
    /// When the run must have ended, its pieces with it.
    deadline: Option<Instant>,
    /// Set when lengua stops the run.
    stop: Arc<AtomicBool>,
    /// Whether the pieces run one at a time, the last first, as in a test's second run.
    reversed: bool,
    /// How many pieces have been taken.
    next: AtomicU32,
    /// The first piece, in the pieces' order, that didn't end well so far.
    failed: Arc<AtomicU32>,
    /// How each piece went, by its place.
    ended: Vec<Mutex<Option<Ended>>>,
}

impl Work<'_> {
    /// Runs piece `index` to its end, in a store of its own.
    fn piece(&self, index: u32) -> Ended {
        let range = crate::split::cut(self.bytes.len(), self.pieces, index);
        let world = World {
            policy: self.policy,
            args: Vec::new(),
            env: BTreeMap::new(),
            stdin: Input::share(self.bytes[range].to_vec()),
            stdout: Output::kept(self.back),
            stderr: Output::kept(ERRORS),
            folder: None,
            reversed: false,
            listener: None,
            connections: Vec::new(),
            datagrams: None,
            internet: None,
            clock: Vec::new(),
            random: None,
            limits: Limits::new(None, Arc::clone(&self.stop), self.back, None, None, 0),
            instant_sleeps: false,
            beyond: Beyond::default(),
            events: None,
        };
        let halt = Halt {
            failed: Arc::clone(&self.failed),
            index,
        };
        let splitting = self.splitting;
        let executed = crate::wasm::runner::execute_until(
            splitting.module.engine(),
            &splitting.linker,
            &splitting.module,
            splitting.setup,
            Host::new(world),
            self.deadline,
            Some(halt),
        );
        let mut host = executed.host;
        // A module that can't be started as a piece started as the program: the run ends there.
        let ending = match executed.ending {
            Ending::Unstarted => Ending::Trapped(Trap::Other, Vec::new()),
            ending => ending,
        };
        Ended {
            ending,
            problem: executed.problem,
            printed: std::mem::replace(&mut host.stdout, Output::Closed).into_bytes(),
            errors: std::mem::replace(&mut host.stderr, Output::Closed).into_bytes(),
            counted: executed.instructions.unwrap_or(0),
        }
    }

    /// One thread's work: it takes the next piece, in the pieces' order or against it, until
    /// none is left. A piece after one that didn't end well isn't started.
    fn take(&self) {
        loop {
            let taken = self.next.fetch_add(1, Ordering::SeqCst);
            if taken >= self.pieces {
                return;
            }
            let index = if self.reversed {
                self.pieces - 1 - taken
            } else {
                taken
            };
            if self.failed.load(Ordering::SeqCst) < index {
                continue;
            }
            let ended = self.piece(index);
            if !ended.well() {
                self.failed.fetch_min(index, Ordering::SeqCst);
            }
            let place = usize::try_from(index).unwrap_or(usize::MAX);
            if let Some(slot) = self.ended.get(place) {
                *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(ended);
            }
        }
    }

    /// Runs every piece, at once on as many threads as the computer has cores, and gives how
    /// each went, in the pieces' order; `None` for a piece that wasn't run.
    fn run(self) -> Vec<Option<Ended>> {
        let pieces = usize::try_from(self.pieces).unwrap_or(usize::MAX);
        let threads = if self.reversed {
            1
        } else {
            crate::split::at_once().min(pieces)
        };
        std::thread::scope(|scope| {
            let work = &self;
            let mut running = Vec::with_capacity(threads);
            for _ in 0..threads {
                let spawned = std::thread::Builder::new()
                    .name("lengua piece".to_owned())
                    .stack_size(STACK)
                    .spawn_scoped(scope, move || work.take());
                match spawned {
                    Ok(thread) => running.push(thread),
                    Err(_) => break,
                }
            }
            // A thread that failed leaves its piece without an answer, which ends the run.
            for thread in running {
                let _ = thread.join();
            }
        });
        self.ended
            .into_iter()
            .map(|slot| slot.into_inner().unwrap_or_else(PoisonError::into_inner))
            .collect()
    }
}

/// `split(input, input_len, pieces, output, output_len)`: cuts the bytes at `input` into
/// `pieces` pieces, runs the program's own module once for each with the piece's bytes as its
/// standard input, at once, waits for all, and writes what each printed at `output`, in the
/// pieces' order, each at the start of its share of `output_len`, with zeros after it.
///
/// # Errors
///
/// A buffer outside memory; `NOTCAPABLE` in a piece, and for an idea with no `- Splits:` line;
/// `INVAL` for a call the line doesn't cover; and a stop of the whole run where it is past its
/// deadline, or where a piece didn't exit with code 0: the run then ends as the first such
/// piece ended, and nothing is written.
pub fn split(
    host: &mut Host,
    memory: &mut [u8],
    input: (u32, u32),
    pieces: u32,
    output: (u32, u32),
) -> Result<(), Fail> {
    let from = memory::range(memory, input.0, input.1)?;
    let to = memory::range(memory, output.0, output.1)?;
    let grant = host.policy.reach.get().split.filter(|_| !host.policy.piece);
    let (Some(grant), Some(splitting)) = (grant, host.splitting.as_ref()) else {
        return Err(Fail::Errno(errno::NOTCAPABLE));
    };
    if !grant.covers(from.len(), pieces, to.len()) {
        return Err(Fail::Errno(errno::INVAL));
    }
    host.limits.check()?;
    let back = usize::try_from(grant.back).unwrap_or(usize::MAX).max(1);
    let work = Work {
        splitting,
        policy: host.policy.for_a_piece(),
        bytes: &memory[from],
        pieces,
        back,
        deadline: host.limits.deadline,
        stop: Arc::clone(&host.limits.stop),
        reversed: host.reversed,
        next: AtomicU32::new(0),
        failed: Arc::new(AtomicU32::new(u32::MAX)),
        ended: (0..pieces).map(|_| Mutex::new(None)).collect(),
    };
    let mut answers = Vec::new();
    let mut failure = None;
    let mut counted: u64 = 0;
    // The pieces' order decides, whichever ended first: the answers go in that order, and the
    // first piece in it that didn't end well ends the run, with what the pieces up to it
    // executed. What a later piece did counts for nothing.
    for ended in work.run() {
        let ended = ended.unwrap_or_else(Ended::unstarted);
        counted = counted.saturating_add(ended.counted);
        if ended.well() {
            answers.push(ended.printed);
        } else {
            failure = Some(ended);
            break;
        }
    }
    host.notes.split = true;
    host.notes.counted = host.notes.counted.saturating_add(counted);
    if let Some(ended) = failure {
        // What the piece said on standard error, such as its failed assertion, the run says.
        let _ = host.stderr.write(&ended.errors);
        host.notes.piece = Some((ended.ending, ended.problem));
        return Err(Fail::Stop(Stop::Piece));
    }
    let room = &mut memory[to];
    room.fill(0);
    for (slot, answer) in room.chunks_exact_mut(back).zip(&answers) {
        let length = answer.len().min(slot.len());
        slot[..length].copy_from_slice(&answer[..length]);
    }
    Ok(())
}

/// `piece()`: whether this run is a piece of a program's split work.
///
/// # Errors
///
/// `NOENT` in the run that is the whole program.
pub fn piece(host: &Host) -> Result<(), Fail> {
    if host.policy.piece {
        Ok(())
    } else {
        Err(Fail::Errno(errno::NOENT))
    }
}
