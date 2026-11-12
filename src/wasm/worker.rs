//! A worker: `lengua-mcp --wasm-worker <data folder or "none">`, a child of lengua's server that
//! runs WebAssembly jobs one after another, each in a fresh store from a module it keeps once it
//! has it: one it compiled itself, in this process, or, for a job that asks for it, the compiled
//! form lengua's data folder keeps, loaded only when it carries the seal of the user's key
//! ([`Worker::kept`]). It loads no other compiled code: the engine has no cache of its own.
//! It reads [`Request`]s on its standard input and writes [`Event`]s on its standard
//! output ([`crate::wasm::job`]). It exits when its standard input ends, so a server that is
//! killed leaves no program running behind it. Before its first job it makes room for the files
//! a program may hold ([`runner::make_room`]), as many on every system.

use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use wasmtime::{Engine, Linker, Module};

use crate::data_dir::DataDir;
use crate::hash::Sha256Hex;
use crate::lock::Key;
use crate::state::compiled_for;
use crate::wasm::engine::{self, Setup};
use crate::wasm::host::{self, Host};
use crate::wasm::job::{self, Done, Event, Job, Request};
use crate::wasm::runner;
use crate::wasm::text;

/// How many compiled modules a worker keeps.
const KEPT_MODULES: usize = 16;

/// How many jobs' stops a worker remembers: a stop may come just after its job has ended.
const KEPT_STOPS: usize = 16;

/// A worker's engines, linkers and compiled modules.
struct Worker {
    /// lengua's data folder, when the server gave it: where the compiled modules a run loads
    /// are kept under the user's seal, with the key that sealed them.
    data: Option<DataDir>,
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
        let engine = engine::engine(setup)?;
        let linker = host::linker(&engine)?;
        self.engines.insert(setup, (engine.clone(), linker.clone()));
        Ok((engine, linker))
    }

    /// The compiled form of `job`'s module that the data folder keeps for the job's
    /// configuration, loaded into `engine` once it carries the seal of `key` as that module's;
    /// or why there is none to load.
    fn kept(engine: &Engine, job: &Job, key: &Key, path: &Path, hash: &Sha256Hex) -> Result<Module, String> {
        let sealed = std::fs::read(path)
            .map_err(|_| "it isn't compiled for this computer yet".to_owned())?;
        key.load_sealed_for(job.setup, engine, hash, &sealed)
            .map_err(|unsealed| unsealed.to_string())
    }

    /// `job`'s module from its file, checked against its hash.
    fn read(job: &Job) -> Result<Vec<u8>, String> {
        let bytes = std::fs::read(&job.module).map_err(|error| {
            format!("the module {} can't be read: {error}", job.module.display())
        })?;
        if Sha256Hex::of(&bytes).as_str() != job.hash {
            return Err(format!(
                "the module {} changed since it was built",
                job.module.display()
            ));
        }
        Ok(bytes)
    }

    /// `job`'s module for a job that loads its compiled form ([`Job::sealed`]), with the data
    /// folder and the user's key there: the form kept, once its seal holds ([`Self::kept`]);
    /// else the module compiled here, sealed, kept for the next run, and loaded from those
    /// very bytes, as the next run loads them, which this worker says once on standard error,
    /// the server's log. `None` when the job doesn't ask, or there is no data folder or no
    /// key: then the module is compiled as a test's is.
    fn sealed(&self, engine: &Engine, job: &Job) -> Option<Result<Module, String>> {
        if !job.sealed {
            return None;
        }
        let data = self.data.as_ref()?;
        let key = Key::load(data).ok().flatten()?;
        let hash = Sha256Hex::parse(&job.hash)?;
        let path = compiled_for(data.path(), text::BUILDER, &hash, job.setup);
        let why = match Self::kept(engine, job, &key, &path, &hash) {
            Ok(module) => return Some(Ok(module)),
            Err(why) => why,
        };
        eprintln!(
            "{}: the program's module has no compiled form to load here ({why}), so this run \
             compiles it and keeps it for the next",
            engine::name()
        );
        Some(Self::read(job).and_then(|wasm| {
            let sealed = key
                .compile_sealed_for(job.setup, engine, &hash, &wasm)
                .map_err(|error| format!("the engine couldn't compile the module: {error}"))?;
            keep(&path, &sealed);
            key.load_sealed_for(job.setup, engine, &hash, &sealed)
                .map_err(|unsealed| format!("lengua couldn't load what it compiled: {unsealed}"))
        }))
    }

    /// `job`'s module for `engine`: kept from an earlier job; or its compiled form from the data
    /// folder, under the user's seal ([`Self::sealed`]); or read from its file, checked against
    /// its hash, and compiled here.
    fn module(&mut self, engine: &Engine, job: &Job) -> Result<Module, String> {
        let key = (job.setup, job.hash.clone());
        if let Some(module) = self.modules.get(&key) {
            return Ok(module.clone());
        }
        let module = match self.sealed(engine, job) {
            Some(loaded) => loaded?,
            None => engine::compile(engine, &Self::read(job)?)
                .map_err(|error| format!("the engine couldn't compile the module: {error:#}"))?,
        };
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

/// Keeps `sealed`, a compiled module with its seal, at `path`, written atomically. A run never
/// fails for a compiled module it couldn't keep.
fn keep(path: &Path, sealed: &[u8]) {
    use std::io::Write as _;
    if let Some(folder) = path.parent()
        && std::fs::create_dir_all(folder).is_ok()
        && let Ok(mut file) = tempfile::NamedTempFile::new_in(folder)
        && file.write_all(sealed).is_ok()
    {
        let _ = file.persist(path);
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

/// Worker mode, with what follows the flag on the command line: lengua's data folder, or `none`.
#[must_use]
pub fn main(mut args: impl Iterator<Item = OsString>) -> ExitCode {
    let data = args
        .next()
        .filter(|arg| arg != "none")
        .map(|folder| DataDir::new(PathBuf::from(folder)));
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
                data,
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
