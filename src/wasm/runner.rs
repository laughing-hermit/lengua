//! Running a WebAssembly module: the part in this process, which a worker does for each job and
//! a run from a shell does in place, and the parts around it, the working folder prepared and
//! read back and how a job went turned into a [`Run`], which everything that compares results
//! reads.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use wasmtime::{Engine, Linker, Module, Store, UpdateDeadline, WasmBacktrace};

use crate::connect::{CONNECT_TIMEOUT, CONNECT_TOTAL, Made, Reach};
use crate::runner::{End, Run, Trap};
use crate::supply::Time;
use crate::wasm::engine::Setup;
use crate::wasm::host::limits::{self, Limits};
use crate::wasm::host::net::{self, Link, Listener, Socket};
use crate::wasm::host::policy::Policy;
use crate::wasm::host::stdio::{Input, Output, Stream};
use crate::wasm::host::{Host, Stop, World};
use crate::wasm::job::{self, Connection, Done, Ending, Event, Job, Reached};

/// The exit code a run that never started is given, as a shell gives a command it can't find:
/// the run's `supplied` says why.
pub const UNSTARTED: i32 = 127;

/// The stack of the thread a program runs on: room for the engine's own and the program's.
pub const STACK: usize = 16 * 1024 * 1024;

/// The most files a process that runs programs holds open at once: the
/// [`crate::runner::OPEN_FILES`] a program may hold, and room for the process's own, such as its
/// standard streams, the program's working folder and what the engine opens, within macOS's
/// `OPEN_MAX` of 10,240.
pub const PROCESS_FILES: u64 = crate::runner::OPEN_FILES + 256;

/// Lets this process hold [`PROCESS_FILES`] files at once, so a program holds as many as it may
/// on every system before it gets `MFILE`: on a Unix, the soft limit is raised to it when it is
/// lower, and the hard limit kept. Windows has no such limit to raise.
///
/// # Errors
///
/// When this computer's hard limit is lower, saying how to raise it.
pub fn make_room() -> Result<(), String> {
    #[cfg(unix)]
    {
        use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};
        let limit = getrlimit(Resource::Nofile);
        if limit.current.is_some_and(|current| current < PROCESS_FILES) {
            setrlimit(
                Resource::Nofile,
                Rlimit {
                    current: Some(PROCESS_FILES),
                    maximum: limit.maximum,
                },
            )
            .map_err(|error| {
                format!(
                    "this computer won't let lengua hold {PROCESS_FILES} open files, the {} a \
                     program may hold on every system and lengua's own ({error}); raise the hard \
                     limit (`ulimit -Hn`)",
                    crate::runner::OPEN_FILES
                )
            })?;
        }
    }
    Ok(())
}

/// The most files read back from a working folder after a run.
const MAX_FILES_BACK: usize = 256;

/// The most bytes read back from a working folder after a run.
const MAX_BYTES_BACK: u64 = 256 * 1024 * 1024;

/// What a run left in its working folder: every file, by name, with its bytes, or why they
/// can't all be read; and every folder.
pub type Left = (Result<BTreeMap<String, Vec<u8>>, String>, BTreeSet<String>);

/// Every file a run left, by name, with its bytes, and every folder.
type Found = (BTreeMap<String, Vec<u8>>, BTreeSet<String>);

/// What running a module in this process came to.
pub struct Executed {
    /// How it ended.
    pub ending: Ending,
    /// The operations it executed, when the engine counted them.
    pub instructions: Option<u64>,
    /// Its time on the clock, in nanoseconds.
    pub wall_time_ns: u64,
    /// Its host, with what it wrote and what it told lengua.
    pub host: Host,
    /// Why lengua couldn't run it as asked, if it couldn't.
    pub problem: Option<String>,
}

/// Where `error` stopped the program: each frame's offset in its module, the innermost first, as
/// far as the engine kept them (its 20 innermost).
fn offsets_of(error: &wasmtime::Error) -> Vec<u64> {
    error
        .downcast_ref::<WasmBacktrace>()
        .map(|trace| {
            trace
                .frames()
                .iter()
                .filter_map(wasmtime::FrameInfo::module_offset)
                .filter_map(|offset| u64::try_from(offset).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// How a run that stopped with `error` ended, and what lengua has to say of it.
fn ending_of(error: &wasmtime::Error) -> (Ending, Option<String>) {
    if let Some(stop) = error.downcast_ref::<Stop>() {
        return match stop {
            Stop::Exit(code) => (Ending::Exited(*code), None),
            Stop::OutputTooLarge => (Ending::OutputTooLarge, None),
            Stop::TimedOut => (Ending::TimedOut, None),
            Stop::Stopped => (Ending::Stopped, None),
            Stop::OutsideMemory => (Ending::Trapped(Trap::OutOfBounds, offsets_of(error)), None),
            Stop::NoMemory => (
                Ending::Trapped(Trap::Other, offsets_of(error)),
                Some(stop.to_string()),
            ),
        };
    }
    if let Some(trap) = error.downcast_ref::<wasmtime::Trap>() {
        let kind = match trap {
            wasmtime::Trap::OutOfFuel | wasmtime::Trap::Interrupt => {
                return (Ending::TimedOut, None);
            }
            wasmtime::Trap::UnreachableCodeReached => Trap::Unreachable,
            wasmtime::Trap::MemoryOutOfBounds | wasmtime::Trap::HeapMisaligned => Trap::OutOfBounds,
            wasmtime::Trap::IntegerDivisionByZero => Trap::DivisionByZero,
            wasmtime::Trap::IntegerOverflow => Trap::Overflow,
            wasmtime::Trap::BadConversionToInteger => Trap::Conversion,
            wasmtime::Trap::StackOverflow => Trap::StackOverflow,
            wasmtime::Trap::TableOutOfBounds
            | wasmtime::Trap::IndirectCallToNull
            | wasmtime::Trap::BadSignature => Trap::IndirectCall,
            _ => Trap::Other,
        };
        return (Ending::Trapped(kind, offsets_of(error)), None);
    }
    (
        Ending::Trapped(Trap::Other, offsets_of(error)),
        Some(format!("{error:#}")),
    )
}

/// Runs `module` with `host`, in this process, until its `_start` returns, it exits, or it is
/// stopped: in `setup`'s configuration of `engine`, its memory held to what the module declares,
/// and stopped `timeout` after it starts, if there's a timeout.
#[must_use]
pub fn execute(
    engine: &Engine,
    linker: &Linker<Host>,
    module: &Module,
    setup: Setup,
    mut host: Host,
    timeout: Option<Duration>,
) -> Executed {
    host.limits.store = limits::store_limits(limits::memory_limit(module));
    host.limits.deadline = timeout.map(|timeout| Instant::now() + timeout);
    let deadline = host.limits.deadline;
    let stop = Arc::clone(&host.limits.stop);
    let mut store = Store::new(engine, host);
    store.limiter(|host| &mut host.limits.store);
    if setup.counts() {
        let _ = store.set_fuel(u64::MAX);
    }
    if setup.interrupts() {
        store.set_epoch_deadline(1);
        store.epoch_deadline_callback(move |_| {
            if stop.load(Ordering::SeqCst) {
                return Err(wasmtime::Error::new(Stop::Stopped));
            }
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                return Err(wasmtime::Error::new(Stop::TimedOut));
            }
            Ok(UpdateDeadline::Continue(1))
        });
    }
    let started = Instant::now();
    let (ending, problem) = match linker.instantiate(&mut store, module) {
        Err(error)
            if error.downcast_ref::<Stop>().is_none()
                && error.downcast_ref::<wasmtime::Trap>().is_none() =>
        {
            (
                Ending::Unstarted,
                Some(format!("lengua couldn't start the module: {error:#}")),
            )
        }
        Err(error) => ending_of(&error),
        Ok(instance) => match instance.get_typed_func::<(), ()>(&mut store, "_start") {
            Err(error) => (
                Ending::Unstarted,
                Some(format!(
                    "the module exports no `_start` that takes and gives nothing: {error:#}"
                )),
            ),
            Ok(start) => match start.call(&mut store, ()) {
                Ok(()) => (Ending::Exited(0), None),
                Err(error) => ending_of(&error),
            },
        },
    };
    let wall_time_ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let instructions = if setup.counts() {
        store.get_fuel().ok().map(|left| u64::MAX - left)
    } else {
        None
    };
    Executed {
        ending,
        instructions,
        wall_time_ns,
        host: store.into_data(),
        problem,
    }
}

/// The connection `connection` names, made now, and why it couldn't be when it couldn't. A run's
/// connections began at `connecting`, and one that would begin once they have taken
/// [`CONNECT_TOTAL`] in all isn't tried: a program doesn't wait on every place that doesn't
/// answer, one after another, before it starts.
fn connect(connection: &Connection, connecting: Instant) -> (Link, Option<String>) {
    if !matches!(connection, Connection::Ended(_)) && connecting.elapsed() >= CONNECT_TOTAL {
        return (
            Link::Ended,
            Some(format!(
                "the connections before it took the {} seconds lengua gives a run's connections \
                 in all",
                CONNECT_TOTAL.as_secs()
            )),
        );
    }
    match connection {
        Connection::Test(port) => {
            let address = SocketAddr::from((Ipv4Addr::LOCALHOST, *port));
            let connected = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT);
            match connected.and_then(Link::tcp) {
                Ok(link) => (link, None),
                Err(error) => (
                    Link::Ended,
                    Some(format!("the test's server didn't answer: {error}")),
                ),
            }
        }
        Connection::Place { host, port, tls } => {
            let reach = match (host, tls) {
                (None, _) => Reach::ThisComputer(*port),
                (Some(host), true) => Reach::Tls(host.clone(), *port),
                (Some(host), false) => Reach::Host(host.clone(), *port),
            };
            let Some(runtime) = net::runtime() else {
                return (
                    Link::Ended,
                    Some("lengua couldn't start what makes its connections".to_owned()),
                );
            };
            match runtime.block_on(crate::connect::make(&reach)) {
                Ok(place) => (Link::Place(place, runtime), None),
                Err(problem) => (Link::Ended, Some(problem)),
            }
        }
        Connection::Ended(problem) => (Link::Ended, Some(problem.clone())),
    }
}

/// What `job` gives its program, and what became of each connection lengua made for it: the
/// connections made first, then the listener bound, which `events` hears of, then the folder.
///
/// # Errors
///
/// Why the program can't be given what it asks, such as a port that's taken.
pub fn world(
    job: &Job,
    stop: Arc<AtomicBool>,
    events: Option<Sender<Event>>,
) -> Result<(World, Vec<Reached>), String> {
    let stdin = job::unhex(&job.stdin)?;
    let random = job.random.as_deref().map(job::unhex).transpose()?;
    let mut connections = Vec::new();
    let mut reached = Vec::new();
    let connecting = Instant::now();
    for made in &job.connections {
        let (link, problem) = connect(&made.connection, connecting);
        connections.push(Socket::new(link));
        reached.push(Reached {
            place: made.place.clone(),
            problem,
        });
    }
    let listener = match job.listen {
        Some((port, anywhere)) => {
            let listener = Listener::bind(port, anywhere)
                .map_err(|error| format!("lengua couldn't listen on port {port}: {error}"))?;
            let port = listener
                .port()
                .map_err(|error| format!("lengua couldn't read its listener's port: {error}"))?;
            if let Some(events) = &events {
                let _ = events.send(Event::Listening {
                    id: job.id,
                    port,
                    connections: reached.clone(),
                });
            }
            Some(listener)
        }
        None => None,
    };
    let folder = match &job.folder {
        Some(path) if job.policy.files => Some(
            cap_std::fs::Dir::open_ambient_dir(path, cap_std::ambient_authority())
                .map_err(|error| format!("the working folder can't be opened: {error}"))?,
        ),
        _ => None,
    };
    let limits = Limits::new(
        None,
        stop,
        job.limits.max_output,
        job.limits.max_file,
        job.limits.max_written,
        0,
    );
    let world = World {
        policy: job.policy,
        args: job.args.clone(),
        env: job.env.clone(),
        stdin: Input::given(stdin, &job.stdin_pieces),
        stdout: if job.stdout_closed {
            Output::Closed
        } else {
            Output::kept(job.limits.max_output)
        },
        stderr: Output::kept(job.limits.max_output),
        folder,
        reversed: job.reversed,
        listener,
        connections,
        clock: job
            .clock
            .iter()
            .map(|(seconds, nanoseconds)| Time {
                seconds: *seconds,
                nanoseconds: *nanoseconds,
            })
            .collect(),
        random,
        limits,
        instant_sleeps: job.instant_sleeps,
        events: events.map(|events| (job.id, events)),
    };
    Ok((world, reached))
}

/// How a job went that never started, for `problem`.
#[must_use]
pub fn unstarted(id: u64, problem: String, connections: Vec<Reached>) -> Done {
    Done {
        id,
        stdout: String::new(),
        stderr: String::new(),
        ending: Ending::Unstarted,
        instructions: None,
        wall_time_ns: 0,
        random_asked: None,
        listed: false,
        connections,
        problem: Some(problem),
    }
}

/// How job `id` went, from what running it came to: its host is closed first, every file and
/// connection with it, before the server hears of it and reads its folder.
#[must_use]
pub fn done(id: u64, executed: Executed, connections: Vec<Reached>) -> Done {
    let Executed {
        ending,
        instructions,
        wall_time_ns,
        mut host,
        problem,
    } = executed;
    let stdout = std::mem::replace(&mut host.stdout, Output::Closed).into_bytes();
    let stderr = std::mem::replace(&mut host.stderr, Output::Closed).into_bytes();
    let random_asked = host.random.asked();
    let listed = host.notes.listed;
    drop(host);
    Done {
        id,
        stdout: job::hex(&stdout),
        stderr: job::hex(&stderr),
        ending,
        instructions,
        wall_time_ns,
        random_asked,
        listed,
        connections,
        problem,
    }
}

/// How a run went, as the runner says it: `line` gives main.wat's line of an offset in its own
/// code, which places a trap at its innermost frame there, and `left` is what the run left in its
/// working folder.
#[must_use]
pub fn run_of(done: Done, left: Left, line: impl Fn(u64) -> Option<usize>) -> Run {
    let instructions = match done.ending {
        Ending::Exited(_) | Ending::Trapped(..) => done.instructions,
        _ => None,
    };
    let end = match done.ending {
        Ending::Exited(code) => End::Exited(code),
        Ending::TimedOut => End::TimedOut,
        Ending::OutputTooLarge => End::OutputTooLarge,
        Ending::Stopped => End::Stopped,
        Ending::Trapped(trap, offsets) => End::Trapped(trap, offsets.into_iter().find_map(line)),
        Ending::Unstarted => End::Exited(UNSTARTED),
    };
    let (files, folders) = left;
    Run {
        stdout: job::unhex(&done.stdout).unwrap_or_default(),
        stderr: job::unhex(&done.stderr).unwrap_or_default(),
        end,
        instructions,
        wall_time_ns: done.wall_time_ns,
        files,
        folders,
        supplied: done.problem,
        random_asked: done.random_asked,
        connections: done
            .connections
            .into_iter()
            .map(|reached| Made {
                place: reached.place,
                problem: reached.problem,
            })
            .collect(),
        listed: done.listed,
    }
}

/// Puts `files` and `folders` in `folder`, making the folders their names include.
///
/// # Errors
///
/// When the system can't.
pub fn prepare(
    folder: &Path,
    files: &BTreeMap<String, String>,
    folders: &BTreeSet<String>,
) -> io::Result<()> {
    for (name, contents) in files {
        let path = folder.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, contents)?;
    }
    for name in folders {
        std::fs::create_dir_all(folder.join(name))?;
    }
    Ok(())
}

/// Every file under `folder`, and every folder, by name relative to it with `/` between names on
/// every system, or why they can't all be read.
pub fn left_in(folder: &Path) -> Left {
    match read_left(folder) {
        Ok((files, folders)) => (Ok(files), folders),
        Err(problem) => (Err(problem), BTreeSet::new()),
    }
}

/// [`left_in`]'s work.
fn read_left(folder: &Path) -> Result<Found, String> {
    let unreadable = |error: io::Error| format!("its working folder couldn't be read: {error}");
    let mut files = BTreeMap::new();
    let mut folders = BTreeSet::new();
    let mut total = 0;
    let mut pending = vec![(folder.to_path_buf(), String::new())];
    while let Some((path, prefix)) = pending.pop() {
        for entry in std::fs::read_dir(&path).map_err(unreadable)? {
            let entry = entry.map_err(unreadable)?;
            let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
            let kind = entry.file_type().map_err(unreadable)?;
            if kind.is_dir() {
                pending.push((entry.path(), format!("{name}/")));
                folders.insert(name);
                continue;
            }
            if !kind.is_file() {
                return Err(format!("it left {name}, which isn't a regular file"));
            }
            if files.len() == MAX_FILES_BACK {
                return Err(format!("it left more than {MAX_FILES_BACK} files"));
            }
            total += entry.metadata().map_err(unreadable)?.len();
            if total > MAX_BYTES_BACK {
                return Err(format!(
                    "its files hold more than {} MiB",
                    MAX_BYTES_BACK / 1024 / 1024
                ));
            }
            files.insert(name, std::fs::read(entry.path()).map_err(unreadable)?);
        }
    }
    Ok((files, folders))
}

/// What a run from a shell gives its program: this process's own standard input, output and
/// error, the shell's folder or the one it names, no time limit and nothing counted.
#[derive(Debug, Clone, Copy)]
pub struct Here<'a> {
    /// Its arguments, after its name.
    pub args: &'a [String],
    /// Its environment variables.
    pub env: &'a BTreeMap<String, String>,
    /// Its working folder.
    pub folder: &'a Path,
    /// What its idea lets it reach.
    pub policy: Policy,
    /// The port it serves on, if it serves, and whether every address may reach it.
    pub listen: Option<(u16, bool)>,
    /// The connections lengua makes for it, in order.
    pub connections: &'a [job::Made],
    /// What places a trap on its line of main.wat.
    pub source: Source<'a>,
}

/// What places a trap of a run from a shell on its line of main.wat.
#[derive(Debug, Clone, Copy)]
pub enum Source<'a> {
    /// Nothing: the module is no build of a main.wat's, and a trap is named without a line.
    Unknown,
    /// The build the module comes from.
    Built(&'a crate::wasm::text::Built),
    /// main.wat's text, which is built only once the program traps, and whose build places the
    /// trap only when it hashes to `module`, the SHA-256 of the module that ran.
    Text {
        /// main.wat.
        text: &'a str,
        /// The module that ran, by its SHA-256.
        module: &'a crate::hash::Sha256Hex,
    },
}

/// What a run's checks hand the program's thread once they pass ([`run_once_checked`]): the
/// module that runs and what it is given, which the thread reads through a shared reference.
pub trait Order: Send + Sync {
    /// The module's bytes.
    fn wasm(&self) -> &[u8];

    /// What the module is given.
    fn here(&self) -> Here<'_>;
}

/// A module and what it is given, handed over as they are: [`run_here`]'s order, which no checks
/// come before.
struct Given<'a> {
    /// The module's bytes.
    wasm: &'a [u8],
    /// What it is given.
    here: Here<'a>,
}

impl Order for Given<'_> {
    fn wasm(&self) -> &[u8] {
        self.wasm
    }

    fn here(&self) -> Here<'_> {
        self.here
    }
}

/// Runs the module `wasm` here, attached to this process's own streams, as `lengua-mcp run` does
/// from a shell, on a thread of its own, with compiled modules kept in `cache`: how it ended. It
/// is [`run_once_checked`] with nothing to check.
///
/// # Errors
///
/// When the module can't be compiled or given what it asks, or this process can't hold the files
/// it may.
pub fn run_here(wasm: &[u8], here: &Here<'_>, cache: Option<&Path>) -> io::Result<End> {
    run_once_checked(cache, || Ok(Given { wasm, here: *here }))
        .unwrap_or_else(|why| Err(io::Error::other(why)))
}

/// Runs a module here as [`run_here`] does, once `checks`, on the caller's thread, give it: the
/// program's thread starts first, and makes the engine, with compiled modules kept in `cache`,
/// and its linker while the checks run, so the engine's start and the checks take their time
/// side by side, on each path that runs a module this way. It compiles nothing until the checks
/// pass: nothing is compiled, instantiated or run before then, a run they refuse ends the thread
/// with nothing compiled, and making the engine reads nothing of the program's. Where `cache`
/// names a folder that isn't there yet, the engine waits for the checks too, so a run they
/// refuse makes no folder. Everything that runs the module happens on that thread, outside any
/// async runtime the caller is in, since the connections lengua makes to places have a runtime
/// of their own.
///
/// # Errors
///
/// The checks' refusal, as they say it. Inside, how the run went, or why it couldn't: the module
/// can't be compiled or given what it asks, or this process can't hold the files it may.
pub fn run_once_checked<O: Order>(
    cache: Option<&Path>,
    checks: impl FnOnce() -> Result<O, String>,
) -> Result<io::Result<End>, String> {
    let order: OnceLock<Option<O>> = OnceLock::new();
    let (refused, ended) = std::thread::scope(|scope| {
        let given = &order;
        let program = std::thread::Builder::new()
            .name("lengua program".to_owned())
            .stack_size(STACK)
            .spawn_scoped(scope, move || {
                let early = cache.is_none_or(Path::is_dir).then(|| prepared(cache));
                let checked = given.wait().as_ref()?;
                let made = early.unwrap_or_else(|| prepared(cache));
                Some(made.and_then(|(engine, linker)| {
                    run_on_this_thread(&engine, &linker, checked.wasm(), &checked.here())
                }))
            });
        // Whatever the checks come to, a panic too, the program's thread hears of it, so it
        // never waits for ever.
        let release = Release(given);
        let refused = match checks() {
            Ok(checked) => {
                let _ = given.set(Some(checked));
                None
            }
            Err(why) => Some(why),
        };
        drop(release);
        let ended = program.and_then(|running| {
            running
                .join()
                .map_err(|_| io::Error::other("the program's thread failed"))
        });
        (refused, ended)
    });
    match (refused, ended) {
        (Some(why), _) => Err(why),
        (None, Ok(Some(ended))) => Ok(ended),
        (None, Ok(None)) => Ok(Err(io::Error::other("the program's thread failed"))),
        (None, Err(error)) => Ok(Err(error)),
    }
}

/// Gives a run's program thread no order when it is dropped, unless the checks gave it one: on
/// a panic in the checks too, so the thread never waits for ever.
struct Release<'a, O>(&'a OnceLock<Option<O>>);

impl<O> Drop for Release<'_, O> {
    fn drop(&mut self) {
        let _ = self.0.set(None);
    }
}

/// What the program's thread prepares before it compiles a module: room for the files a program
/// may hold, the engine for a run from a shell, with compiled modules kept in `cache`, and its
/// linker. None of it reads anything of the program's.
fn prepared(cache: Option<&Path>) -> io::Result<(Engine, Linker<Host>)> {
    make_room().map_err(io::Error::other)?;
    let engine = crate::wasm::engine::engine(Setup::Shell, cache).map_err(io::Error::other)?;
    let linker = crate::wasm::host::linker(&engine).map_err(io::Error::other)?;
    Ok((engine, linker))
}

/// [`run_once_checked`]'s work, on the program's own thread, once the checks pass: `wasm` run
/// with what `here` gives, in `engine`, through `linker`.
fn run_on_this_thread(
    engine: &Engine,
    linker: &Linker<Host>,
    wasm: &[u8],
    here: &Here<'_>,
) -> io::Result<End> {
    let module = Module::from_binary(engine, wasm).map_err(io::Error::other)?;
    let mut connections = Vec::new();
    let connecting = Instant::now();
    for made in here.connections {
        let (link, problem) = connect(&made.connection, connecting);
        if let Some(problem) = problem {
            eprintln!(
                "lengua-mcp: couldn't connect to {} ({problem}), so the program got that \
                 connection already ended",
                made.place
            );
        }
        connections.push(Socket::new(link));
    }
    let listener = match here.listen {
        Some((port, anywhere)) => {
            let listener = Listener::bind(port, anywhere)?;
            let port = listener.port()?;
            if anywhere {
                eprintln!("lengua-mcp: serving on port {port} of every address of this computer");
            } else {
                eprintln!("lengua-mcp: serving on 127.0.0.1:{port}");
            }
            Some(listener)
        }
        None => None,
    };
    let folder = if here.policy.files {
        Some(cap_std::fs::Dir::open_ambient_dir(
            here.folder,
            cap_std::ambient_authority(),
        )?)
    } else {
        None
    };
    let world = World {
        policy: here.policy,
        args: here.args.to_vec(),
        env: here.env.clone(),
        stdin: Input::Here,
        stdout: Output::Here(Stream::Out),
        stderr: Output::Here(Stream::Err),
        folder,
        reversed: false,
        listener,
        connections,
        clock: Vec::new(),
        random: None,
        limits: Limits::new(
            None,
            Arc::new(AtomicBool::new(false)),
            usize::MAX,
            None,
            None,
            0,
        ),
        instant_sleeps: false,
        events: None,
    };
    let executed = execute(
        engine,
        linker,
        &module,
        Setup::Shell,
        Host::new(world),
        None,
    );
    if let Some(problem) = &executed.problem {
        eprintln!("lengua-mcp: {problem}");
    }
    // main.wat's text is built once, and only on a trap, which is all its build is for here.
    let rebuilt = std::cell::OnceCell::new();
    let line = |offset: u64| match here.source {
        Source::Unknown => None,
        Source::Built(built) => built.line_at(offset),
        Source::Text { text, module } => rebuilt
            .get_or_init(|| {
                crate::wasm::text::build(text)
                    .ok()
                    .filter(|built| crate::hash::Sha256Hex::of(&built.wasm) == *module)
            })
            .as_ref()
            .and_then(|built| built.line_at(offset)),
    };
    let run = run_of(
        done(0, executed, Vec::new()),
        (Ok(BTreeMap::new()), BTreeSet::new()),
        line,
    );
    Ok(run.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::Sha256Hex;

    /// A main.wat whose `_start` reaches `unreachable` on its line 4.
    const TRAPS: &str = "(module\n  (memory (export \"memory\") 1)\n  (func (export \"_start\")\
                         \n    unreachable))\n";

    /// A run from a shell with no build of its module, only main.wat's text, builds main.wat once
    /// the program traps, and places the trap on main.wat's line only when that build hashes to
    /// the module that ran; with another hash, the trap is named without a line.
    #[test]
    fn a_trap_takes_its_line_from_a_build_of_the_module_that_ran() {
        let built = crate::wasm::text::build(TRAPS).expect("it builds");
        let folder = tempfile::tempdir().expect("a folder");
        let args: Vec<String> = Vec::new();
        let env: BTreeMap<String, String> = BTreeMap::new();
        let ended = |module: &Sha256Hex| {
            let here = Here {
                args: &args,
                env: &env,
                folder: folder.path(),
                policy: Policy::default(),
                listen: None,
                connections: &[],
                source: Source::Text {
                    text: TRAPS,
                    module,
                },
            };
            run_here(&built.wasm, &here, None).expect("it runs")
        };
        let module = Sha256Hex::of(&built.wasm);
        assert_eq!(ended(&module), End::Trapped(Trap::Unreachable, Some(4)));
        let other = Sha256Hex::of(b"another module");
        assert_eq!(ended(&other), End::Trapped(Trap::Unreachable, None));
    }

    /// A main.wat whose `_start` exits with code 3.
    const EXITS: &str = "(module\n  (import \"wasi_snapshot_preview1\" \"proc_exit\" (func $exit \
                         (param i32)))\n  (memory (export \"memory\") 1)\n  (func (export \
                         \"_start\")\n    (call $exit (i32.const 3))))\n";

    /// A run whose checks pass runs what they give; one they refuse gives their refusal, runs
    /// nothing, and makes no cache folder that wasn't there; and a panic in the checks comes back
    /// as the caller's, its program's thread ended, not waiting for ever. (The engine for a run
    /// from a shell is the process's, made at its first use, so the runs that pass keep no
    /// cache, as every other test's do.)
    #[test]
    fn a_run_runs_only_once_its_checks_pass() {
        let built = crate::wasm::text::build(EXITS).expect("it builds");
        let folder = tempfile::tempdir().expect("a folder");
        let args: Vec<String> = Vec::new();
        let env: BTreeMap<String, String> = BTreeMap::new();
        let here = Here {
            args: &args,
            env: &env,
            folder: folder.path(),
            policy: Policy::default(),
            listen: None,
            connections: &[],
            source: Source::Built(&built),
        };
        let cache = folder.path().join("cache");
        let refused = run_once_checked(Some(cache.as_path()), || {
            Err::<Given<'_>, _>("lengua won't run it".to_owned())
        });
        assert_eq!(refused.err().as_deref(), Some("lengua won't run it"));
        assert!(!cache.exists(), "a refused run made the cache's folder");
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_once_checked::<Given<'_>>(None, || panic!("the checks failed"))
        }));
        assert!(panicked.is_err(), "the checks' panic came back");
        let ran = run_once_checked(None, || {
            Ok(Given {
                wasm: &built.wasm,
                here,
            })
        });
        assert!(matches!(ran, Ok(Ok(End::Exited(3)))), "{ran:?}");
        let again = run_here(&built.wasm, &here, None);
        assert!(matches!(again, Ok(End::Exited(3))), "{again:?}");
    }
}
