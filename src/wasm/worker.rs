//! A worker: `lengua-mcp --wasm-worker <cache folder or "none">`, a child of lengua's server that
//! runs WebAssembly jobs one after another, each in a fresh store from a module it compiled once
//! and keeps. It reads [`Request`]s on its standard input and writes [`Event`]s on its standard
//! output ([`crate::wasm::job`]). It exits when its standard input ends, so a server that is
//! killed leaves no program running behind it. Before its first job it makes room for the files
//! a program may hold ([`runner::make_room`]), as many on every system.

use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use wasmtime::{Engine, Linker, Module};

use crate::hash::Sha256Hex;
use crate::wasm::engine::{self, Setup};
use crate::wasm::host::{self, Host};
use crate::wasm::job::{self, Done, Event, Job, Request};
use crate::wasm::runner;

/// How many compiled modules a worker keeps.
const KEPT_MODULES: usize = 16;

/// How many jobs' stops a worker remembers: a stop may come just after its job has ended.
const KEPT_STOPS: usize = 16;

/// A worker's engines, linkers and compiled modules.
struct Worker {
    /// Where compiled modules are kept on disk.
    cache: Option<PathBuf>,
    /// Why this process can't hold the files a program may, when it can't: then no job starts.
    no_room: Option<String>,
    /// Each configuration's engine and linker, once made.
    engines: BTreeMap<Setup, (Engine, Linker<Host>)>,
    /// Compiled modules, by configuration and hash.
    modules: BTreeMap<(Setup, String), Module>,
    /// The order they were compiled in, oldest first.
    order: VecDeque<(Setup, String)>,
}

impl Worker {
    /// The engine and linker for `setup`.
    fn engine(&mut self, setup: Setup) -> Result<(Engine, Linker<Host>), String> {
        if let Some((engine, linker)) = self.engines.get(&setup) {
            return Ok((engine.clone(), linker.clone()));
        }
        let engine = engine::engine(setup, self.cache.as_deref())?;
        let linker = host::linker(&engine)?;
        self.engines.insert(setup, (engine.clone(), linker.clone()));
        Ok((engine, linker))
    }

    /// `job`'s module, compiled for `engine`: kept from an earlier job, or read from its file,
    /// checked against its hash, and compiled.
    fn module(&mut self, engine: &Engine, job: &Job) -> Result<Module, String> {
        let key = (job.setup, job.hash.clone());
        if let Some(module) = self.modules.get(&key) {
            return Ok(module.clone());
        }
        let bytes = std::fs::read(&job.module).map_err(|error| {
            format!("the module {} can't be read: {error}", job.module.display())
        })?;
        if Sha256Hex::of(&bytes).as_str() != job.hash {
            return Err(format!(
                "the module {} changed since it was built",
                job.module.display()
            ));
        }
        let module = Module::from_binary(engine, &bytes)
            .map_err(|error| format!("the engine couldn't compile the module: {error:#}"))?;
        if self.order.len() == KEPT_MODULES
            && let Some(oldest) = self.order.pop_front()
        {
            self.modules.remove(&oldest);
        }
        self.order.push_back(key.clone());
        self.modules.insert(key, module.clone());
        Ok(module)
    }

    /// Runs `job` to its end.
    fn run(&mut self, job: &Job, stop: Arc<AtomicBool>, events: mpsc::Sender<Event>) -> Done {
        if let Some(problem) = &self.no_room {
            return runner::unstarted(job.id, problem.clone(), Vec::new());
        }
        // The connections and the listener first: the server waits on them as the job starts.
        let (world, reached) = match runner::world(job, stop, Some(events)) {
            Ok(world) => world,
            Err(problem) => return runner::unstarted(job.id, problem, Vec::new()),
        };
        let (engine, linker) = match self.engine(job.setup) {
            Ok(engine) => engine,
            Err(problem) => return runner::unstarted(job.id, problem, reached),
        };
        let module = match self.module(&engine, job) {
            Ok(module) => module,
            Err(problem) => return runner::unstarted(job.id, problem, reached),
        };
        let timeout = job.limits.timeout_ms.map(Duration::from_millis);
        let executed = runner::execute(
            &engine,
            &linker,
            &module,
            job.setup,
            Host::new(world),
            timeout,
        );
        runner::done(job.id, executed, reached)
    }
}

/// Reads the server's requests: each job goes to `jobs` with its stop, and a stop sets that
/// job's. Their end is the server's end, and this worker's, so whatever runs goes with it.
fn read_requests(jobs: &mpsc::Sender<(Job, Arc<AtomicBool>)>) {
    let mut input = std::io::stdin().lock();
    let mut stops: BTreeMap<u64, Arc<AtomicBool>> = BTreeMap::new();
    loop {
        match job::read_message(&mut input) {
            Ok(Some(Request::Run(job))) => {
                let stop = Arc::new(AtomicBool::new(false));
                stops.insert(job.id, Arc::clone(&stop));
                while stops.len() > KEPT_STOPS {
                    stops.pop_first();
                }
                if jobs.send((*job, stop)).is_err() {
                    std::process::exit(0);
                }
            }
            Ok(Some(Request::Stop(id))) => {
                if let Some(stop) = stops.get(&id) {
                    stop.store(true, Ordering::SeqCst);
                }
            }
            Ok(None) | Err(_) => std::process::exit(0),
        }
    }
}

/// Worker mode, with what follows the flag on the command line: the cache folder, or `none`.
#[must_use]
pub fn main(mut args: impl Iterator<Item = OsString>) -> ExitCode {
    let cache = args.next().filter(|arg| arg != "none").map(PathBuf::from);
    // Room for the files a program may hold, before any job: where there's none, each job says
    // why instead of starting.
    let no_room = runner::make_room().err();
    let (jobs, queue) = mpsc::channel::<(Job, Arc<AtomicBool>)>();
    let (events, outbox) = mpsc::channel::<Event>();

    let reader = std::thread::Builder::new()
        .name("lengua worker requests".to_owned())
        .spawn(move || read_requests(&jobs));
    if reader.is_err() {
        return ExitCode::FAILURE;
    }

    // What the worker says, in order.
    let writer = std::thread::Builder::new()
        .name("lengua worker events".to_owned())
        .spawn(move || {
            let mut output = std::io::stdout().lock();
            for event in outbox {
                if job::write_message(&mut output, &event).is_err() {
                    std::process::exit(0);
                }
            }
        });
    if writer.is_err() {
        return ExitCode::FAILURE;
    }

    // The jobs, on a thread with room for the engine's stack and a program's.
    let runner = std::thread::Builder::new()
        .name("lengua worker".to_owned())
        .stack_size(runner::STACK)
        .spawn(move || {
            let mut worker = Worker {
                cache,
                no_room,
                engines: BTreeMap::new(),
                modules: BTreeMap::new(),
                order: VecDeque::new(),
            };
            for (job, stop) in queue {
                let done = worker.run(&job, stop, events.clone());
                if events.send(Event::Done(Box::new(done))).is_err() {
                    return;
                }
            }
        });
    match runner.map(std::thread::JoinHandle::join) {
        Ok(Ok(())) => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}
