//! The model of a WebAssembly program, for proofs about main.wat as [`super::exec`]'s core is for
//! machine code: [`Wasm`], a core whose runs take one operator a step over terms, on the module
//! lengua builds from main.wat ([`Module`]). Its host calls are the world's models
//! ([`super::world`]), reached through WASI preview 1 as lengua's host gives it, so what the model
//! says a run does is what lengua's host does on every system.
//!
//! With every input given, the model is a plain interpreter, which the tests hold to the engine
//! ([`run`]). With inputs as terms, a run splits at each branch the inputs decide, and each trap
//! the engine would stop at is an obligation the proof must meet on every path, with main.wat's
//! line: an access outside the memory, a division or remainder by zero, the least value divided
//! by -1, and `unreachable` reached. The stack never runs out: [`Module`] refuses a module whose
//! calls recurse, or may go deeper than the engine's stack holds.

mod host;
mod image;
mod memory;
mod step;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use super::exec::{self, Core, End, Memory, State, Step};
use super::term::{Pool, Term};
use super::world::{World, net};
use host::Handle;
pub use image::Module;

/// A label of a block a call is inside: where a branch to it goes, and what it carries there.
#[derive(Debug, Clone, Copy)]
struct Label {
    /// Whether a branch to it goes back to its start, as a loop's does.
    looping: bool,
    /// The stack's height below the values it takes.
    height: usize,
    /// How many values a branch to it carries: a loop's parameters, a block's results.
    carries: usize,
    /// Where a branch to it goes on: a loop's first operator, or the one after a block's `end`.
    target: usize,
}

/// A call under way.
#[derive(Debug, Clone)]
struct Frame {
    /// The function it runs.
    function: u32,
    /// Its parameters, then the locals its function declares.
    locals: Vec<Term>,
    /// The stack's height where its operands start.
    height: usize,
    /// The blocks it's inside, its function's own outermost.
    labels: Vec<Label>,
    /// The operator it runs next.
    pc: usize,
}

/// A WebAssembly program's machine, as one run holds it: the operand stack, the calls under way
/// with their locals and blocks, the globals, linear memory, and the descriptors the program
/// holds.
#[derive(Debug, Clone)]
pub struct Wasm {
    /// The operand stack: every call's operands, the newest call's last.
    stack: Vec<Term>,
    /// The calls under way, the newest last.
    frames: Vec<Frame>,
    /// Each global's value.
    globals: Vec<Term>,
    /// The bytes of linear memory written so far; the rest are the data segments' or zeros.
    memory: Memory,
    /// How many pages the memory has.
    pages: u64,
    /// Whether each data segment is dropped: the active ones once the program has started.
    dropped: Vec<bool>,
    /// The descriptors the program holds, by number.
    fds: BTreeMap<u32, Handle>,
    /// The lowest number a descriptor the program opens or accepts may take.
    first: u32,
    /// Whether the descriptors lengua hands the program over, its listener and connections,
    /// are in place: they are at its first step, from the world's network.
    handed: bool,
    /// The program's arguments, its name first, as `args_get` gives them.
    args: Rc<Vec<Vec<Term>>>,
    /// Its environment's `NAME=value` strings, as `environ_get` gives them.
    env: Rc<Vec<Vec<Term>>>,
    /// Whether folders list turned round, as in a test's second run: `None` until the first
    /// listing, where a proof takes both orders.
    reversed: Option<bool>,
    /// How many bytes the program has written to files, with a block for each file or folder it
    /// made, as lengua's host counts them against a test's total.
    written: u64,
}

impl Wasm {
    /// `what`, at main.wat's line of the innermost call of main.wat's own functions: the running
    /// operator `at` of the newest call, or the call each caller is making.
    fn placed(&self, module: &Module, at: usize, what: &str) -> String {
        let mut running = Some(at);
        for frame in self.frames.iter().rev() {
            let here = running.unwrap_or_else(|| frame.pc.saturating_sub(1));
            if let Some(line) = module.line(frame.function, here) {
                return format!("main.wat line {line}: {what}");
            }
            running = None;
        }
        what.to_owned()
    }

    /// The run's end at a trap, `what` the engine stops it for, at main.wat's line.
    fn trap(&self, module: &Module, at: usize, what: &str) -> End {
        End::Unsafe(self.placed(module, at, what))
    }
}

/// Holds the run to never meeting `fails`, a trap's condition, with `what` the engine says of it:
/// the run ends there where it surely does, and the proof must show it never does where the input
/// decides.
///
/// # Errors
///
/// The trap, where `fails` surely holds.
fn must_not(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &Pool,
    at: usize,
    fails: Term,
    what: &str,
) -> Result<(), End> {
    match pool.truth(fails) {
        Some(false) => Ok(()),
        Some(true) => Err(state.core.trap(module, at, what)),
        None => {
            let problem = state.core.placed(module, at, what);
            state.world.obligations.push((fails, problem));
            Ok(())
        }
    }
}

/// The label of a call's own body, `ops` operators long: a branch out of it returns, carrying the
/// function's `results`.
fn function_label(height: usize, results: usize, ops: usize) -> Label {
    Label {
        looping: false,
        height,
        carries: results,
        target: ops,
    }
}

/// The state a WebAssembly program's `_start` begins in: `args` after its name, the environment's
/// `NAME=value` strings and `stdin` as byte terms; its globals and memory as the module starts
/// them; and its descriptors as lengua's host lays them out: 0 to 2 the standard streams, and 3
/// the working folder for an idea that names files. The listener and the connections lengua made
/// are put at 4 and 5 on at the first step, from the network the run is given.
#[must_use]
pub fn begin(
    module: &Module,
    pool: &mut Pool,
    args: &[Vec<Term>],
    env: &[Vec<Term>],
    stdin: Vec<Term>,
) -> State<Wasm> {
    let mut strings = vec![exec::bytes(pool, crate::wasm::PROGRAM_NAME.as_bytes())];
    strings.extend(args.iter().cloned());
    // The environment only for an idea that names it, as the host gives it.
    let environment = if module.policy.environment {
        env.to_vec()
    } else {
        Vec::new()
    };
    let globals = module
        .globals
        .iter()
        .map(|(width, value)| pool.bv(*width, *value))
        .collect();
    let mut fds = BTreeMap::from([(0, Handle::Stdin), (1, Handle::Stdout), (2, Handle::Stderr)]);
    if module.policy.files {
        fds.insert(
            crate::wasm::host::fds::FOLDER,
            Handle::Folder {
                path: Vec::new(),
                listing: None,
            },
        );
    }
    let mut frames = Vec::new();
    if let Some(entry) = module.function(module.entry) {
        let locals = entry
            .locals
            .iter()
            .map(|kind| pool.bv(kind.width(), 0))
            .collect();
        frames.push(Frame {
            function: module.entry,
            locals,
            height: 0,
            labels: vec![function_label(0, entry.results, entry.ops.len())],
            pc: 0,
        });
    }
    let core = Wasm {
        stack: Vec::new(),
        frames,
        globals,
        memory: Memory::default(),
        pages: module.pages,
        dropped: module.active.clone(),
        fds,
        first: crate::wasm::host::fds::FIRST_CONNECTION,
        handed: false,
        args: Rc::new(strings),
        env: Rc::new(environment),
        reversed: None,
        written: 0,
    };
    State {
        core,
        world: World::new(stdin),
    }
}

/// WebAssembly's runs, each operator a step.
impl Core for Wasm {
    type Image = Module;

    fn step(image: &Module, state: &mut State<Self>, pool: &mut Pool) -> Result<Step<Self>, End> {
        step::execute(image, state, pool)
    }
}

/// What a run of the model is given, as a test gives the real program.
#[derive(Debug, Clone, Default)]
pub struct Given {
    /// The arguments, after the program's name.
    pub args: Vec<Vec<u8>>,
    /// The environment's variables, by name.
    pub env: BTreeMap<String, String>,
    /// Standard input, which then ends.
    pub stdin: Vec<u8>,
    /// The files in the working folder before the run, by name.
    pub files: BTreeMap<String, Vec<u8>>,
    /// The folders in it before the run.
    pub folders: BTreeSet<String>,
    /// The random bytes a test supplies, if it does.
    pub random: Option<Vec<u8>>,
    /// The clock readings a test supplies, seconds and nanoseconds; the last repeats.
    pub clock: Vec<(i64, u32)>,
    /// How many connections lengua makes for the program, each handed over ended.
    pub connections: usize,
    /// Whether the program has its listener, which no client calls.
    pub listens: bool,
    /// Whether standard output is a pipe nobody reads.
    pub stdout_closed: bool,
    /// Whether folders list turned round, as in a test's second run.
    pub reversed: bool,
}

/// How a run of the model ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ended {
    /// The program exited with this code.
    Exit(u8),
    /// The engine would stop it at a trap: what, with main.wat's line where it has one.
    Trap(String),
    /// The run ended another way, or the model couldn't follow it: why.
    Other(String),
}

/// What a run of the model did, every value known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    /// How it ended.
    pub end: Ended,
    /// What it wrote to standard output.
    pub stdout: Vec<u8>,
    /// What it wrote to standard error.
    pub stderr: Vec<u8>,
    /// The files in the working folder after the run, by name.
    pub files: BTreeMap<String, Vec<u8>>,
    /// The folders in it after the run.
    pub folders: BTreeSet<String>,
}

/// Runs `module` on `given`, as the engine runs it in a test: every input known, so the model is
/// a plain interpreter.
///
/// # Errors
///
/// What the model can't run, or output it can't know.
pub fn run(module: &Module, given: &Given) -> Result<Ran, String> {
    let mut pool = Pool::default();
    let args: Vec<Vec<Term>> = given
        .args
        .iter()
        .map(|arg| exec::bytes(&mut pool, arg))
        .collect();
    let env: Vec<Vec<Term>> = given
        .env
        .iter()
        .map(|(name, value)| exec::bytes(&mut pool, format!("{name}={value}").as_bytes()))
        .collect();
    let stdin = exec::bytes(&mut pool, &given.stdin);
    let files = given
        .files
        .iter()
        .map(|(name, bytes)| (name.clone(), exec::bytes(&mut pool, bytes)))
        .collect();
    let random = given
        .random
        .as_ref()
        .map(|bytes| exec::bytes(&mut pool, bytes));
    let clock = given
        .clock
        .iter()
        .map(|(seconds, nanoseconds)| {
            (
                pool.bv(64, u128::from(seconds.cast_unsigned())),
                pool.bv(64, u128::from(*nanoseconds)),
            )
        })
        .collect();
    let network = net::Network {
        server: Vec::new(),
        made: vec![None; given.connections],
        client: Vec::new(),
        listening: given.listens,
        at_once: false,
        eager: true,
    };
    let mut start = begin(module, &mut pool, &args, &env, stdin)
        .with_files(files)
        .with_folders(given.folders.clone())
        .with_supply(random, clock)
        .with_stdout_closed(given.stdout_closed)
        .with_network(network);
    start.core.reversed = Some(given.reversed);
    let mut runs = exec::explore(module, start, &mut pool, &mut exec::Concrete, 2)?;
    let world = runs.pop().ok_or("the model didn't run")?.world;
    let known = |terms: &[Term]| -> Result<Vec<u8>, String> {
        terms
            .iter()
            .map(|term| {
                pool.constant(*term)
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| "output the model doesn't know".to_owned())
            })
            .collect()
    };
    let end = match &world.end {
        Some(End::Exit(code)) => Ended::Exit(
            pool.constant(*code)
                .and_then(|value| u8::try_from(value).ok())
                .ok_or("an exit code the model doesn't know")?,
        ),
        Some(End::Unsafe(problem)) => Ended::Trap(problem.clone()),
        Some(End::Unsupported(why) | End::Stuck(why)) => Ended::Other(why.clone()),
        other => Ended::Other(format!("{other:?}")),
    };
    let mut names: BTreeSet<&String> = world.files_before().keys().collect();
    names.extend(world.written.keys());
    let mut files = BTreeMap::new();
    for name in names {
        if let Some(bytes) = world.file(name) {
            files.insert(name.clone(), known(bytes)?);
        }
    }
    // Every folder: those given and made, and those a file is in.
    let mut candidates: BTreeSet<String> = world.folders_before.iter().cloned().collect();
    candidates.extend(world.made.iter().cloned());
    for name in world.files_before().keys().chain(world.written.keys()) {
        let mut parent = name.as_str();
        while let Some((above, _)) = parent.rsplit_once('/') {
            candidates.insert(above.to_owned());
            parent = above;
        }
    }
    let folders = candidates
        .into_iter()
        .filter(|name| world.is_folder(name))
        .collect();
    Ok(Ran {
        end,
        stdout: known(&world.stdout)?,
        stderr: known(&world.stderr)?,
        files,
        folders,
    })
}
