//! A solver's WebAssembly module, run as its native program runs: `lengua-mcp --solver-wasm
//! <module> [--cache <folder>] -- <solver's arguments>`, which the prover starts in place of a
//! native solver when it finds a WebAssembly one ([`crate::prove::smt`]), so that its pipes, its
//! protocol, its logs and the watchdog stay as they are. The module runs in the solver's engine
//! ([`Setup::Solver`]: exceptions on, and neither a count nor a deadline, since the prover stops a
//! solver by ending its process) through lengua's own WASI host, on a thread with a native
//! solver's stack ([`engine::SOLVER_STACK`]). It is given this process's standard input, output
//! and error, its arguments and the clock, and nothing else ([`POLICY`]): no files, no network, no
//! environment, and zeros for random bytes, as lengua gives a program that names none of them. Its
//! memory is held to [`MEMORY`] by the store's limiter, on every system, so a solver that asks for
//! more gets an error from its allocator, not the computer's memory.
//!
//! The process exits as the module ends: with the module's exit code; with
//! [`TRAPPED`](crate::measure::TRAPPED) when it traps, or throws an exception that nothing in it
//! catches, as a native solver then aborts; or with [`REFUSED`](crate::measure::REFUSED) when it
//! can't be started. In the last two it says why on standard error.
//!
//! Compiled modules are kept in Wasmtime's cache: in the folder `--cache` names, or else in
//! lengua's own data folder's ([`cache_folder`]), where `lengua-mcp install` and `lengua-mcp
//! doctor` compile a solver once ([`compile`]), so that no proof waits for a compile.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use wasmtime::{Engine, Linker, Module, Store};

use crate::data_dir::DataDir;
use crate::measure::{REFUSED, TRAPPED};
use crate::runner::Trap;
use crate::wasm::engine::{self, Setup};
use crate::wasm::host::limits::Limits;
use crate::wasm::host::stdio::{Input, Output, Stream};
use crate::wasm::host::{self, Host, Stop, World};
use crate::wasm::job::Ending;
use crate::wasm::{Policy, runner};

/// The command-line flag that starts this binary as a solver's host.
pub const FLAG: &str = "--solver-wasm";

/// The most bytes a solver's memory may hold: 4 GiB, all that a 32-bit WebAssembly memory
/// addresses, and the 4,096 MiB the prover lets z3 take (`-memory:4096`).
pub const MEMORY: usize = 1 << 32;

/// What a solver's module may reach beyond its output and its exit: its standard input and the
/// clock.
pub const POLICY: Policy = Policy {
    standard_input: true,
    files: false,
    clock: true,
    random: false,
    environment: false,
    terminal: false,
    connections: 0,
    listens: false,
    at_once: false,
};

/// How a solver's run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ended {
    /// It exited with this code: `proc_exit`'s, or 0 when its `_start` returned.
    Exited(i32),
    /// The engine stopped it at an instruction that can't go on.
    Trapped(Trap),
    /// It threw an exception that nothing in it caught.
    Uncaught,
    /// lengua couldn't go on running it, for this reason.
    Failed(String),
    /// lengua couldn't start it, for this reason.
    Unstarted(String),
}

impl Ended {
    /// The exit code its host gives: the module's own; [`TRAPPED`] when it trapped, threw what it
    /// didn't catch, or couldn't go on; and [`REFUSED`] when it never started.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::Exited(code) => *code,
            Self::Trapped(_) | Self::Uncaught | Self::Failed(_) => TRAPPED,
            Self::Unstarted(_) => REFUSED,
        }
    }

    /// What its host says of it on standard error: nothing when it exited by itself.
    #[must_use]
    pub fn said(&self) -> Option<String> {
        match self {
            Self::Exited(_) => None,
            Self::Trapped(trap) => Some(format!("the solver trapped: {}", trap.describe())),
            Self::Uncaught => {
                Some("the solver threw an exception that nothing in it caught".to_owned())
            }
            Self::Failed(why) => Some(format!("the solver couldn't go on: {why}")),
            Self::Unstarted(why) => Some(why.clone()),
        }
    }
}

/// What a solver's host is asked to run.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Order {
    /// The module's file.
    module: PathBuf,
    /// Where compiled modules are kept, when `--cache` names a folder.
    cache: Option<PathBuf>,
    /// The solver's arguments, after its name.
    args: Vec<String>,
}

/// What follows [`FLAG`] on the command line: the module, `--cache` and a folder if given, and
/// after `--` the solver's arguments, each Unicode text, as lengua passes no other on.
fn parse(mut words: impl Iterator<Item = OsString>) -> Result<Order, String> {
    let module = PathBuf::from(words.next().ok_or("--solver-wasm needs a module")?);
    let mut cache = None;
    while let Some(word) = words.next() {
        if word == "--" {
            break;
        }
        if word != "--cache" {
            return Err(format!(
                "--solver-wasm takes a module, --cache <folder>, and after -- the solver's \
                 arguments, not {}",
                word.to_string_lossy()
            ));
        }
        cache = Some(PathBuf::from(words.next().ok_or("--cache needs a folder")?));
    }
    let args = words
        .map(|word| {
            word.into_string().map_err(|word| {
                format!(
                    "the solver's argument {} isn't Unicode text, which not every system can \
                     pass on",
                    word.to_string_lossy()
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Order {
        module,
        cache,
        args,
    })
}

/// The folder a solver's compiled module is kept in when `--cache` names none: the `cache` folder
/// of lengua's own data folder, the same for a shell, where `lengua-mcp install` and `doctor`
/// compile the solver once, and for every agent harness's server, whatever data folder the
/// harness gives the plugin; `None` when this computer names no data folder.
#[must_use]
pub fn cache_folder() -> Option<PathBuf> {
    DataDir::from_env().map(|data| data.path().join("cache"))
}

/// Compiles the solver's module at `module` for this computer, keeping it in Wasmtime's cache in
/// `cache`, where a solver's host then finds it compiled: what `lengua-mcp install` and `doctor`
/// do once, so that no proof waits for a compile.
///
/// # Errors
///
/// Says why when the module can't be read or compiled, or nothing can be kept in `cache`.
pub fn compile(module: &Path, cache: Option<&Path>) -> Result<(), String> {
    let wasm = std::fs::read(module).map_err(|error| format!("it can't be read: {error}"))?;
    let engine = engine::engine(Setup::Solver, cache)?;
    Module::from_binary(&engine, &wasm)
        .map(|_| ())
        .map_err(|error| format!("{error:#}"))
}

/// Runs `module` as a solver on the calling thread, which needs room for a solver's calls
/// ([`engine::SOLVER_STACK`]): with `args` after its name, this process's standard input, output
/// and error, and the clock, and its memory held to `memory` bytes.
#[must_use]
pub fn run(
    engine: &Engine,
    linker: &Linker<Host>,
    module: &Module,
    args: Vec<String>,
    memory: usize,
) -> Ended {
    let limits = Limits::new(
        None,
        Arc::new(AtomicBool::new(false)),
        usize::MAX,
        None,
        None,
        memory,
    );
    let world = World {
        policy: POLICY,
        args,
        env: BTreeMap::new(),
        stdin: Input::Here,
        stdout: Output::Here(Stream::Out),
        stderr: Output::Here(Stream::Err),
        folder: None,
        reversed: false,
        listener: None,
        connections: Vec::new(),
        clock: Vec::new(),
        random: None,
        limits,
        instant_sleeps: false,
        events: None,
    };
    let mut store = Store::new(engine, Host::new(world));
    store.limiter(|host| &mut host.limits.store);
    let instance = match linker.instantiate(&mut store, module) {
        Ok(instance) => instance,
        // A module's start function runs as it is instantiated, and may end the solver there.
        Err(error) if its_own(&error) => return ended(&error),
        Err(error) => {
            return Ended::Unstarted(format!(
                "lengua couldn't start the solver's module: {error:#}"
            ));
        }
    };
    match instance.get_typed_func::<(), ()>(&mut store, "_start") {
        Ok(start) => match start.call(&mut store, ()) {
            Ok(()) => Ended::Exited(0),
            Err(error) => ended(&error),
        },
        Err(error) => Ended::Unstarted(format!(
            "the solver's module exports no `_start` that takes and gives nothing: {error:#}"
        )),
    }
}

/// Whether `error` ended the module's own run, as a stop of the host's, a trap or a thrown
/// exception does, rather than lengua's start of it.
fn its_own(error: &wasmtime::Error) -> bool {
    error.downcast_ref::<Stop>().is_some()
        || error.downcast_ref::<wasmtime::Trap>().is_some()
        || error.downcast_ref::<wasmtime::ThrownException>().is_some()
}

/// How a solver that stopped with `error` ended.
fn ended(error: &wasmtime::Error) -> Ended {
    if error.downcast_ref::<wasmtime::ThrownException>().is_some() {
        return Ended::Uncaught;
    }
    match runner::ending_of(error) {
        (Ending::Exited(code), _) => Ended::Exited(code),
        (Ending::Trapped(trap, _), None) => Ended::Trapped(trap),
        (_, Some(why)) => Ended::Failed(why),
        (ending, None) => Ended::Failed(format!("it ended as no solver can: {ending:?}")),
    }
}

/// The solver's engine, with compiled modules kept in `cache`; without them, saying so on
/// standard error, where that folder can't keep them.
fn solver_engine(cache: Option<&Path>) -> Result<Engine, String> {
    let config = match engine::config(Setup::Solver, cache) {
        Ok(config) => config,
        Err(problem) => {
            eprintln!("lengua-mcp: {problem}, so the solver is compiled for this run alone");
            engine::config(Setup::Solver, None)?
        }
    };
    Engine::new(&config).map_err(|error| format!("the WebAssembly engine couldn't start: {error}"))
}

/// Runs what `order` asks, on a thread with room for a solver's calls: the module read, compiled
/// or taken from the cache, and run.
fn hosted(order: Order) -> Ended {
    let thread = std::thread::Builder::new()
        .name("lengua solver".to_owned())
        .stack_size(engine::SOLVER_STACK)
        .spawn(move || {
            let shown = order.module.display().to_string();
            let wasm = match std::fs::read(&order.module) {
                Ok(wasm) => wasm,
                Err(error) => {
                    return Ended::Unstarted(format!(
                        "can't read the solver's module {shown}: {error}"
                    ));
                }
            };
            let cache = order.cache.or_else(cache_folder);
            let engine = match solver_engine(cache.as_deref()) {
                Ok(engine) => engine,
                Err(problem) => return Ended::Unstarted(problem),
            };
            let module = match Module::from_binary(&engine, &wasm) {
                Ok(module) => module,
                Err(error) => {
                    return Ended::Unstarted(format!(
                        "the engine couldn't compile the solver's module {shown}: {error:#}"
                    ));
                }
            };
            match host::linker(&engine) {
                Ok(linker) => run(&engine, &linker, &module, order.args, MEMORY),
                Err(problem) => Ended::Unstarted(problem),
            }
        });
    match thread {
        Ok(running) => running
            .join()
            .unwrap_or_else(|_| Ended::Failed("its thread failed".to_owned())),
        Err(error) => Ended::Unstarted(format!("the solver's thread couldn't start: {error}")),
    }
}

/// Solver mode, with what follows [`FLAG`] on the command line: runs the module as [`run`] does,
/// on a thread with room for a solver's calls, and exits as it ends, saying why on standard error
/// when it didn't exit by itself.
#[must_use]
pub fn main(words: impl Iterator<Item = OsString>) -> ExitCode {
    let ended = match parse(words) {
        Ok(order) => hosted(order),
        Err(problem) => Ended::Unstarted(problem),
    };
    if let Some(said) = ended.said() {
        eprintln!("lengua-mcp: {said}");
    }
    ExitCode::from(u8::try_from(ended.code()).unwrap_or(u8::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `list` as a command line's words.
    fn words(list: &[&str]) -> impl Iterator<Item = OsString> {
        list.iter().map(|word| OsString::from(*word))
    }

    /// A solver's command line: its module, `--cache` and a folder if given, and after `--` the
    /// solver's own arguments, which may look like options.
    #[test]
    fn a_solver_s_command_line_takes_its_module_cache_and_arguments() {
        let order = parse(words(&["z3.wasm", "--", "-in", "-smt2", "-memory:4096"]))
            .expect("a solver's command line");
        assert_eq!(order.module, PathBuf::from("z3.wasm"));
        assert_eq!(order.cache, None);
        assert_eq!(order.args, ["-in", "-smt2", "-memory:4096"]);
        let cached = parse(words(&["z3.wasm", "--cache", "kept", "--", "--version"]))
            .expect("a solver's command line");
        assert_eq!(cached.cache, Some(PathBuf::from("kept")));
        assert_eq!(cached.args, ["--version"]);
        let bare = parse(words(&["z3.wasm"])).map(|order| order.args.len());
        assert_eq!(bare, Ok(0));
        assert!(parse(words(&[])).is_err());
        assert!(parse(words(&["z3.wasm", "--cache"])).is_err());
        assert!(parse(words(&["z3.wasm", "-in"])).is_err());
    }

    /// The stand-in solver of tests/fixtures/solver-wasm, as wast encodes it.
    fn stand_in() -> Vec<u8> {
        let text = include_str!("../../tests/fixtures/solver-wasm/stand-in.wat");
        let buffer = wast::parser::ParseBuffer::new(text).expect("WebAssembly text");
        let wast::Wat::Module(mut module) =
            wast::parser::parse::<wast::Wat>(&buffer).expect("a module")
        else {
            panic!("the stand-in isn't a module");
        };
        module.encode().expect("the module's bytes")
    }

    /// How the stand-in's `grow` ends with its memory held to `memory` bytes: it exits 0 when its
    /// memory grew by 4 MiB, and 1 when the host refused.
    fn grown(memory: usize) -> Ended {
        let wasm = stand_in();
        std::thread::Builder::new()
            .stack_size(engine::SOLVER_STACK)
            .spawn(move || {
                let engine = engine::engine(Setup::Solver, None).expect("the solver's engine");
                let linker = host::linker(&engine).expect("the host");
                let module = Module::from_binary(&engine, &wasm).expect("it compiles");
                run(&engine, &linker, &module, vec!["grow".to_owned()], memory)
            })
            .expect("a thread")
            .join()
            .expect("the thread ends")
    }

    /// A solver's memory is held to its cap, on every system: past it, a growth fails as an
    /// allocator's request for more does, and below it the same growth is made.
    #[test]
    fn a_solver_s_memory_is_held_to_its_cap() {
        assert_eq!(grown(1 << 20), Ended::Exited(1));
        assert_eq!(grown(MEMORY), Ended::Exited(0));
    }

    /// The exit code and the words of each end: a solver's own exit says nothing.
    #[test]
    fn each_end_has_its_code_and_words() {
        assert_eq!(
            (Ended::Exited(3).code(), Ended::Exited(3).said()),
            (3, None)
        );
        assert_eq!(Ended::Uncaught.code(), TRAPPED);
        let trapped = Ended::Trapped(Trap::StackOverflow);
        assert_eq!(trapped.code(), TRAPPED);
        assert!(
            trapped
                .said()
                .is_some_and(|said| said.contains("deeper than the stack")),
            "{trapped:?}"
        );
        assert_eq!(Ended::Unstarted("no module".to_owned()).code(), REFUSED);
    }
}
