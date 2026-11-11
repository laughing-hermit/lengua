//! `lengua-mcp measure`: runs a program the way people run it, in a process of its own, and gives
//! what it cost as JSON: its CPU time in user space and in the kernel, its time on the clock, its
//! peak resident memory and the size of what ran. The system counts the time and the memory, for
//! the child process alone: `getrusage` of the children on Linux and macOS, the process's own
//! counters on Windows (`lengua_os::process_usage`).
//!
//! Linux begins a process's count of its peak memory at what the process it was started from
//! held, there lengua-mcp, whose megabytes are more than many a program's own peak. So on Linux
//! the program starts from lengua-spawn ([`SPAWNER`]), a small process beside lengua-mcp that
//! forks it, counts it as its own child and writes what it counted to a file: the count then
//! begins at the few hundred KiB of lengua-spawn's copy. Where lengua-spawn isn't there, or the
//! CPU's counters would count its work too, the program starts straight from lengua-mcp, and a
//! peak no higher than lengua-mcp's own is no peak of the program's: it is left out.
//!
//! It runs three kinds of program, each as it is run outside lengua's tests:
//! - a program folder whose code is `main.wat`: lengua-run ([`RUNNER`]), the small program
//!   beside lengua-mcp that `check` tells a user to run a checked program with, or `lengua-mcp
//!   run` where it isn't there, with every check `run` makes (the lock, its signature, the
//!   compiled module held to the user's key and the lock's hash of the module) before the
//!   module runs in lengua's host;
//! - any WebAssembly module of WASI preview 1, such as a C, C++ or Rust toolchain makes: in
//!   lengua's host too, by the same program, in the engine's configuration for a run from a
//!   shell, with what its `--uses` and `--network` words allow, as an idea's lines would
//!   ([`MODULE_FLAG`]), and from its compiled form, kept in the data folder under the user's key
//!   as a checked program's is: its first run compiles it, and `measure` hashes its file, so the
//!   run it measures reads neither;
//! - a native executable, as a shell starts it.
//!
//! A server is measured under load: once it takes a connection, `--clients` clients each send
//! `--requests` lines of [`REQUEST_BYTES`] bytes, one at a time, each answered in capitals before
//! the next goes; then the server is stopped. Its time on the clock is from its first connection
//! to its last answer, and its CPU time and memory are its whole run's.
//!
//! On Linux, `--counters` adds the CPU's counts of the program's reads that missed the L1 data
//! cache, the last-level cache and the data TLB, from the moment the program started: a run apart
//! from the timed ones, since the counters are attached while it starts.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::data_dir::DataDir;
use crate::hash::Sha256Hex;
use crate::lock::Key;
use crate::runner::{End, Limits};
use crate::server::NotRun;
use crate::wasm::Policy;
use crate::wasm::engine::{self, Setup};
use crate::wasm::runner::{Here, On, Order, Source, run_once_checked};

/// The hidden flag that starts this binary as the runner of one WebAssembly module, which
/// `measure` gives the module's run after it, as JSON.
pub const MODULE_FLAG: &str = "--run-module";

/// The exit code of a module that trapped, as `lengua-mcp run` gives it.
pub const TRAPPED: i32 = 134;

/// The exit code when lengua can't run what it was asked to run.
pub const REFUSED: i32 = 125;

/// The small program that starts a program on Linux, beside lengua-mcp's own executable, where
/// Cargo builds it (src/bin/lengua-spawn.rs).
pub const SPAWNER: &str = "lengua-spawn";

/// The small program that runs a checked program, beside lengua-mcp's own executable
/// (crates/lengua-run): lengua built without the engine's compiler, which loads what was
/// compiled before.
pub const RUNNER: &str = "lengua-run";

/// The bytes of one request to a server under load, its newline included.
pub const REQUEST_BYTES: usize = 64;

/// How long `measure` waits for a server to take its first connection.
const SERVER_START: Duration = Duration::from_secs(60);

/// How long one attempt to connect to a server that may not listen yet waits.
const CONNECT_ATTEMPT: Duration = Duration::from_millis(50);

/// How long a client waits for an answer before it gives up on the server.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(60);

/// The letters a request is made of.
const LETTERS: &[u8; 26] = b"abcdefghijklmnopqrstuvwxyz";

/// What kind of program a path names, which decides how it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A program folder whose code is main.wat, run as `lengua-mcp run` runs it.
    Wat,
    /// A WebAssembly module of WASI preview 1, run in lengua's host.
    Wasm,
    /// A native executable.
    Native,
}

impl Kind {
    /// The kind of program at `path`: a folder is a main.wat program's, a `.wasm` file is a
    /// module, and anything else an executable.
    #[must_use]
    pub fn of(path: &Path) -> Self {
        if path.is_dir() {
            Self::Wat
        } else if path
            .extension()
            .is_some_and(|extension| extension == "wasm")
        {
            Self::Wasm
        } else {
            Self::Native
        }
    }
}

/// The load a server is measured under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Load {
    /// The port of this computer's loopback it serves on.
    pub port: u16,
    /// How many clients connect at once.
    pub clients: usize,
    /// How many requests each client sends, one at a time.
    pub requests: usize,
}

/// What to measure, as `measure`'s words say it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Request {
    /// The program: a program folder, a `.wasm` module or an executable.
    pub program: PathBuf,
    /// Its arguments.
    pub args: Vec<String>,
    /// The folder it runs in, where a WebAssembly program's files are: the current one when
    /// `None`.
    pub folder: Option<PathBuf>,
    /// The file its standard input comes from; an input that ends at once when `None`.
    pub stdin: Option<PathBuf>,
    /// The file its standard output goes to, made or emptied first; nowhere when `None`.
    pub stdout: Option<PathBuf>,
    /// The file its standard error goes to, made or emptied first; nowhere when `None`.
    pub stderr: Option<PathBuf>,
    /// Its environment variables: no others reach it.
    pub env: BTreeMap<String, String>,
    /// For a module, what it may reach, in an idea's `- Uses:` words.
    pub uses: Vec<String>,
    /// For a module, an idea's `- Network:` line after its colon, or nothing.
    pub network: String,
    /// The load, for a server.
    pub serve: Option<Load>,
    /// Whether the CPU's counters count its misses too, where the system has them.
    pub counters: bool,
}

/// The value after the option `name`.
fn value(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{name} needs a value"))
}

/// The whole number from 1 after the option `name`.
fn count(args: &mut impl Iterator<Item = String>, name: &str) -> Result<usize, String> {
    let text = value(args, name)?;
    text.parse::<usize>()
        .ok()
        .filter(|count| *count > 0)
        .ok_or_else(|| format!("{name} needs a whole number from 1, not {text:?}"))
}

impl Request {
    /// `measure`'s words: its options, then the program, then the program's arguments, after
    /// `--` or straight after it.
    ///
    /// # Errors
    ///
    /// What's wrong with the words.
    pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut request = Self::default();
        let mut port = None;
        let (mut clients, mut requests) = (None, None);
        let mut program = None;
        while let Some(arg) = args.next() {
            // The first word that isn't an option is the program.
            if !arg.starts_with('-') {
                program = Some(PathBuf::from(arg));
                break;
            }
            match arg.as_str() {
                "--in" => request.folder = Some(PathBuf::from(value(&mut args, "--in")?)),
                "--stdin" => request.stdin = Some(PathBuf::from(value(&mut args, "--stdin")?)),
                "--stdout" => request.stdout = Some(PathBuf::from(value(&mut args, "--stdout")?)),
                "--stderr" => request.stderr = Some(PathBuf::from(value(&mut args, "--stderr")?)),
                "--env" => {
                    let pair = value(&mut args, "--env")?;
                    let (name, value) = pair
                        .split_once('=')
                        .ok_or_else(|| format!("--env {pair:?} isn't NAME=value"))?;
                    request.env.insert(name.to_owned(), value.to_owned());
                }
                "--uses" => {
                    request.uses = value(&mut args, "--uses")?
                        .split(',')
                        .map(str::trim)
                        .filter(|word| !word.is_empty())
                        .map(str::to_owned)
                        .collect();
                }
                "--network" => request.network = value(&mut args, "--network")?,
                "--serve" => {
                    let text = value(&mut args, "--serve")?;
                    let parsed = text.parse::<u16>().ok().filter(|port| *port > 0);
                    port =
                        Some(parsed.ok_or_else(|| format!("--serve needs a port, not {text:?}"))?);
                }
                "--clients" => clients = Some(count(&mut args, "--clients")?),
                "--requests" => requests = Some(count(&mut args, "--requests")?),
                "--counters" => request.counters = true,
                other => return Err(format!("measure has no option {other:?}")),
            }
        }
        request.program =
            program.ok_or("measure needs a program: a folder, a module or an executable")?;
        let mut rest = args.peekable();
        if rest.peek().is_some_and(|arg| arg == "--") {
            rest.next();
        }
        request.args = rest.collect();
        request.serve = match (port, clients, requests) {
            (Some(port), clients, requests) => Some(Load {
                port,
                clients: clients.unwrap_or(64),
                requests: requests.unwrap_or(1000),
            }),
            (None, None, None) => None,
            (None, _, _) => {
                return Err("--clients and --requests are a server's: give --serve".into());
            }
        };
        Ok(request)
    }
}

/// What a run of a program cost, as `measure` prints it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measures {
    /// What kind of program ran.
    pub kind: Kind,
    /// Its exit code, when it ended by itself.
    pub exit: Option<i32>,
    /// Its CPU time in user space, in nanoseconds: its threads', and those of any process it
    /// waited for.
    pub user_ns: u64,
    /// The CPU time the kernel spent for it, in nanoseconds.
    pub system_ns: u64,
    /// Its time on the clock in nanoseconds: from its start to its end, or, for a server, from
    /// its first connection to its last answer.
    pub wall_ns: u64,
    /// The most memory it held resident at once, in bytes, where the system says.
    pub peak_bytes: Option<u64>,
    /// The size of what ran, in bytes: the module main.wat builds, a module's file, or an
    /// executable's.
    pub size: u64,
    /// Its reads that missed the L1 data cache, where counted.
    pub l1d_misses: Option<u64>,
    /// Its reads that missed the last-level cache, where counted.
    pub llc_misses: Option<u64>,
    /// Its reads that missed the data TLB, where counted.
    pub dtlb_misses: Option<u64>,
    /// For a server, how many requests it answered as it should.
    pub answered: Option<u64>,
    /// For a server, what went wrong with its answers, if anything did.
    pub problem: Option<String>,
}

impl Measures {
    /// Its CPU time in all, in nanoseconds.
    #[must_use]
    pub const fn cpu_ns(&self) -> u64 {
        self.user_ns.saturating_add(self.system_ns)
    }
}

/// `path` as an absolute path, from `here` when it's relative.
fn absolute(here: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        here.join(path)
    }
}

/// The size of what runs for `program` of `kind`: the module its main.wat builds, with lengua's
/// library merged in, or its file.
fn size_of(kind: Kind, program: &Path) -> Result<u64, String> {
    let unreadable = |path: &Path, error: std::io::Error| format!("{}: {error}", path.display());
    match kind {
        Kind::Wat => {
            let code = program.join(crate::wasm::CODE_FILE);
            let text = std::fs::read_to_string(&code).map_err(|error| unreadable(&code, error))?;
            let built = crate::wasm::text::build(&text).map_err(|problem| problem.to_string())?;
            Ok(u64::try_from(built.wasm.len()).unwrap_or(u64::MAX))
        }
        Kind::Wasm | Kind::Native => std::fs::metadata(program)
            .map(|metadata| metadata.len())
            .map_err(|error| unreadable(program, error)),
    }
}

/// A command with no environment but what every system needs to start a process: on Windows,
/// `SystemRoot`, without which its sockets and some libraries don't start.
fn bare(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    if cfg!(windows)
        && let Some(root) = std::env::var_os("SystemRoot")
    {
        command.env("SystemRoot", root);
    }
    command
}

/// One module's run, as `measure` hands it to [`MODULE_FLAG`].
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModuleRun {
    /// The module's file.
    module: PathBuf,
    /// The module's SHA-256, as `measure` read it from that file before the run it measures:
    /// the run loads the module's compiled form by it, and reads the file only to compile it.
    sha256: String,
    /// The folder it runs in.
    folder: PathBuf,
    /// What it may reach, in an idea's `- Uses:` words.
    uses: Vec<String>,
    /// An idea's `- Network:` line after its colon, or nothing.
    network: String,
    /// Its environment variables, which reach it when its words name the environment.
    env: BTreeMap<String, String>,
    /// Its arguments.
    args: Vec<String>,
    /// lengua's data folder, when `measure` has one: the module's compiled form is kept there
    /// under the user's key, as a checked program's is.
    data: Option<PathBuf>,
}

/// The program that runs a checked program or a module for `measure`, and whether it is
/// lengua-run: lengua-run beside this lengua-mcp when it is there, as `check` tells a user to
/// run a checked program; else this lengua-mcp itself, which runs both too.
fn runner() -> Result<(PathBuf, bool), String> {
    let lengua = std::env::current_exe()
        .map_err(|error| format!("can't find lengua-mcp itself: {error}"))?;
    let alone = lengua.with_file_name(format!("{RUNNER}{}", std::env::consts::EXE_SUFFIX));
    if alone.is_file() {
        Ok((alone, true))
    } else {
        Ok((lengua, false))
    }
}

/// The command that runs `program`, of `kind`, in `folder`, as `request` asks.
fn command_for(
    kind: Kind,
    data_dir: Option<&DataDir>,
    request: &Request,
    program: &Path,
    folder: &Path,
) -> Result<Command, String> {
    let mut command = match kind {
        Kind::Wat => {
            let data_dir = data_dir.ok_or_else(|| crate::data_dir::NOT_FOUND.to_owned())?;
            let (runner, alone) = runner()?;
            let mut command = bare(&runner);
            command.arg("--data-dir").arg(data_dir.path());
            if !alone {
                command.arg("run");
            }
            command.arg(program).arg("--in").arg(folder);
            for (name, value) in &request.env {
                command.arg("--env").arg(format!("{name}={value}"));
            }
            command.arg("--").args(&request.args);
            command
        }
        Kind::Wasm => {
            let wasm = std::fs::read(program)
                .map_err(|error| format!("can't read {}: {error}", program.display()))?;
            let run = ModuleRun {
                module: program.to_path_buf(),
                sha256: Sha256Hex::of(&wasm).as_str().to_owned(),
                folder: folder.to_path_buf(),
                uses: request.uses.clone(),
                network: request.network.clone(),
                env: request.env.clone(),
                args: request.args.clone(),
                data: data_dir.map(|data_dir| data_dir.path().to_path_buf()),
            };
            let spec = serde_json::to_string(&run).map_err(|error| error.to_string())?;
            let mut command = bare(&runner()?.0);
            command.arg(MODULE_FLAG).arg(spec);
            command
        }
        Kind::Native => {
            let mut command = bare(program);
            command.args(&request.args).envs(&request.env);
            command
        }
    };
    command.current_dir(folder);
    Ok(command)
}

/// Standard input from `file`, or one that ends at once.
fn input(file: Option<&Path>) -> Result<Stdio, String> {
    file.map_or_else(
        || Ok(Stdio::null()),
        |file| {
            File::open(file)
                .map(Stdio::from)
                .map_err(|error| format!("{}: {error}", file.display()))
        },
    )
}

/// Standard output or error to `file`, made or emptied first, or nowhere.
fn output(file: Option<&Path>) -> Result<Stdio, String> {
    file.map_or_else(
        || Ok(Stdio::null()),
        |file| {
            File::create(file)
                .map(Stdio::from)
                .map_err(|error| format!("{}: {error}", file.display()))
        },
    )
}

/// How a run ended, before the system's counts of it.
struct Ended {
    /// Its exit code, when it ended by itself.
    exit: Option<i32>,
    /// Its time on the clock.
    wall: Duration,
    /// For a server, how many requests it answered as it should.
    answered: Option<u64>,
    /// For a server, what went wrong with its answers.
    problem: Option<String>,
}

/// What a run cost, as the system counted it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Spent {
    /// Its CPU time in user space, in nanoseconds.
    user_ns: u64,
    /// The CPU time the kernel spent for it, in nanoseconds.
    system_ns: u64,
    /// The most memory it held resident at once, in bytes, where the count is the program's own.
    peak_bytes: Option<u64>,
    /// How it ended, its exit code or none for a signal, and its time on the clock, as the
    /// spawner saw them: none when it started straight from this process.
    ran: Option<(Option<i32>, Duration)>,
}

impl Spent {
    /// The spawner's report, `text`: one line of the program's exit code, or `signal` when a
    /// signal ended it, then its CPU time in user space and in the kernel and its time on the
    /// clock, in nanoseconds, and its peak resident memory, in bytes.
    fn reported(text: &str) -> Option<Self> {
        let mut fields = text.split_whitespace();
        let exit = match fields.next()? {
            "signal" => None,
            code => Some(code.parse::<i32>().ok()?),
        };
        let mut count = || fields.next()?.parse::<u64>().ok();
        let (user_ns, system_ns, wall_ns, peak_bytes) = (count()?, count()?, count()?, count()?);
        if fields.next().is_some() {
            return None;
        }
        Some(Self {
            user_ns,
            system_ns,
            peak_bytes: Some(peak_bytes),
            ran: Some((exit, Duration::from_nanos(wall_ns))),
        })
    }
}

/// How a run starts, which decides what counts it.
enum Start {
    /// Straight from this process, which the system counts it for.
    Straight,
    /// On Linux, from the spawner, which counts it as its own child.
    Spawned {
        /// The spawner, beside this executable.
        spawner: PathBuf,
        /// The file the spawner writes what it counted to.
        report: tempfile::NamedTempFile,
    },
}

impl Start {
    /// From the spawner on Linux, where it is beside this executable, unless the CPU's counters,
    /// which would count its work too, are asked for; else straight from here.
    fn choose(request: &Request) -> Result<Self, String> {
        if !cfg!(target_os = "linux") || request.counters {
            return Ok(Self::Straight);
        }
        let spawner = std::env::current_exe().map(|exe| exe.with_file_name(SPAWNER));
        match spawner {
            Ok(spawner) if spawner.is_file() => {
                let report = tempfile::NamedTempFile::new()
                    .map_err(|error| format!("a file for {SPAWNER}'s report: {error}"))?;
                Ok(Self::Spawned { spawner, report })
            }
            _ => Ok(Self::Straight),
        }
    }

    /// `command`, as this start starts it: through the spawner, the command is the spawner's,
    /// its report's file and then the program and its arguments, with the program's environment
    /// and folder.
    fn command(&self, command: Command) -> Command {
        let Self::Spawned { spawner, report } = self else {
            return command;
        };
        let envs = command
            .get_envs()
            .filter_map(|(name, value)| Some((name, value?)));
        let mut spawned = bare(spawner);
        spawned
            .arg(report.path())
            .arg(command.get_program())
            .args(command.get_args())
            .envs(envs);
        if let Some(folder) = command.get_current_dir() {
            spawned.current_dir(folder);
        }
        spawned
    }

    /// Stops the program, as a server is stopped once measured: straight, or through the
    /// spawner with SIGTERM, which the spawner turns into the program's SIGKILL before it writes
    /// what it counted.
    fn stop(&self, child: &mut Child) {
        match self {
            Self::Straight => {
                let _ = child.kill();
            }
            Self::Spawned { .. } => terminate(child),
        }
    }

    /// What the run cost, once `child` has ended: as the system counted it for this process's
    /// children, or as the spawner's report says.
    fn spent(&self, child: &Child) -> Result<Spent, String> {
        let Self::Spawned { report, .. } = self else {
            return usage(child);
        };
        let text = std::fs::read_to_string(report.path())
            .map_err(|error| format!("{SPAWNER}'s report: {error}"))?;
        Spent::reported(&text).ok_or_else(|| format!("{SPAWNER}'s report isn't one: {text:?}"))
    }
}

/// Asks `child`, the spawner, to stop the program it started: SIGTERM, which it turns into the
/// program's SIGKILL.
#[cfg(unix)]
fn terminate(child: &mut Child) {
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;
    if let Ok(pid) = i32::try_from(child.id()) {
        let _ = kill(Pid::from_raw(pid), Signal::SIGTERM);
    }
}

/// Where there are no signals there is no spawner, and `child` is the program itself.
#[cfg(not(unix))]
fn terminate(child: &mut Child) {
    let _ = child.kill();
}

/// Runs `request`'s program once, in a process of its own, and gives what it cost. A main.wat
/// program needs `data_dir`, where the key of its lock is, as `lengua-mcp run` does.
///
/// # Errors
///
/// When the program can't be started, waited for or measured.
pub fn measure(data_dir: Option<&DataDir>, request: &Request) -> Result<Measures, String> {
    let here = std::env::current_dir()
        .map_err(|error| format!("can't read the current folder: {error}"))?;
    let program = absolute(&here, &request.program);
    let kind = Kind::of(&program);
    let folder = request
        .folder
        .as_ref()
        .map_or_else(|| here.clone(), |folder| absolute(&here, folder));
    let size = size_of(kind, &program)?;
    let start = Start::choose(request)?;
    let mut command = start.command(command_for(kind, data_dir, request, &program, &folder)?);
    command
        .stdin(input(request.stdin.as_deref())?)
        .stdout(output(request.stdout.as_deref())?)
        .stderr(output(request.stderr.as_deref())?);
    let started = Instant::now();
    let mut child = command
        .spawn()
        .map_err(|error| format!("couldn't start {}: {error}", program.display()))?;
    let mut counters = if request.counters {
        counters::attach(child.id())
    } else {
        Vec::new()
    };
    let ended = match request.serve {
        None => {
            let status = child
                .wait()
                .map_err(|error| format!("couldn't wait for the program: {error}"))?;
            Ended {
                exit: status.code(),
                wall: started.elapsed(),
                answered: None,
                problem: None,
            }
        }
        Some(load) => serve(&mut child, load, &start)?,
    };
    let spent = start.spent(&child)?;
    let [l1d_misses, llc_misses, dtlb_misses] = counters::read(&mut counters);
    // The spawner saw how the program ended, and its time on the clock without the spawner's own
    // start; a server's time runs from its first connection, and it was stopped.
    let (exit, wall) = match (spent.ran, request.serve) {
        (Some(ran), None) => ran,
        _ => (ended.exit, ended.wall),
    };
    Ok(Measures {
        kind,
        exit,
        user_ns: spent.user_ns,
        system_ns: spent.system_ns,
        wall_ns: u64::try_from(wall.as_nanos()).unwrap_or(u64::MAX),
        peak_bytes: spent.peak_bytes,
        size,
        l1d_misses,
        llc_misses,
        dtlb_misses,
        answered: ended.answered,
        problem: ended.problem,
    })
}

/// Measures `child` as a server under `load`: it waits until the server takes a connection,
/// connects every client, has each send its requests, and then stops the server as `start`
/// stops it. Its time on the clock runs from the first connection to the last answer.
fn serve(child: &mut Child, load: Load, start: &Start) -> Result<Ended, String> {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, load.port));
    let deadline = Instant::now() + SERVER_START;
    let first = loop {
        if let Ok(stream) = TcpStream::connect_timeout(&address, CONNECT_ATTEMPT) {
            break stream;
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!(
                "the server ended before it took a connection, with {status}"
            ));
        }
        if Instant::now() >= deadline {
            start.stop(child);
            let _ = child.wait();
            return Err(format!(
                "the server took no connection on port {} in {} s",
                load.port,
                SERVER_START.as_secs()
            ));
        }
        std::thread::sleep(CONNECT_ATTEMPT);
    };
    let connected = Instant::now();
    let mut streams = vec![Ok(first)];
    for _ in 1..load.clients {
        streams.push(TcpStream::connect_timeout(&address, ANSWER_TIMEOUT));
    }
    let outcomes: Vec<Result<u64, String>> = std::thread::scope(|scope| {
        let clients: Vec<_> = streams
            .into_iter()
            .enumerate()
            .map(|(client, stream)| {
                scope.spawn(move || {
                    let stream = stream.map_err(|error| format!("client {client}: {error}"))?;
                    exchange(stream, client, load.requests)
                })
            })
            .collect();
        clients
            .into_iter()
            .map(|client| {
                client
                    .join()
                    .unwrap_or_else(|_| Err("a client's thread failed".to_owned()))
            })
            .collect()
    });
    let wall = connected.elapsed();
    start.stop(child);
    child
        .wait()
        .map_err(|error| format!("couldn't wait for the server: {error}"))?;
    let answered = outcomes.iter().flatten().sum();
    let problem = outcomes.into_iter().find_map(Result::err);
    Ok(Ended {
        exit: None,
        wall,
        answered: Some(answered),
        problem,
    })
}

/// The request `number` of client `client`: lowercase letters, then a newline.
fn request_line(client: usize, number: usize) -> [u8; REQUEST_BYTES] {
    let mut line = [b'\n'; REQUEST_BYTES];
    for (index, byte) in line.iter_mut().take(REQUEST_BYTES - 1).enumerate() {
        *byte = LETTERS[(client + number + index) % LETTERS.len()];
    }
    line
}

/// Client `client`'s requests, each sent once the last is answered, and each answer checked:
/// the line in capitals. Gives how many came back as they should.
fn exchange(mut stream: TcpStream, client: usize, requests: usize) -> Result<u64, String> {
    let failed = |what: &str, error: std::io::Error| format!("client {client}: {what}: {error}");
    stream
        .set_read_timeout(Some(ANSWER_TIMEOUT))
        .map_err(|error| failed("set a timeout", error))?;
    stream
        .set_nodelay(true)
        .map_err(|error| failed("send at once", error))?;
    let mut answer = [0_u8; REQUEST_BYTES];
    let mut answered = 0;
    for number in 0..requests {
        let line = request_line(client, number);
        stream
            .write_all(&line)
            .map_err(|error| failed("send a request", error))?;
        stream
            .read_exact(&mut answer)
            .map_err(|error| failed("read an answer", error))?;
        if answer != line.to_ascii_uppercase().as_slice() {
            return Err(format!(
                "client {client}: request {number} was answered {:?}, not {:?}",
                String::from_utf8_lossy(&answer),
                String::from_utf8_lossy(&line.to_ascii_uppercase())
            ));
        }
        answered += 1;
    }
    Ok(answered)
}

/// What the system counted for `child`, which has ended: its CPU time in user space and in the
/// kernel, and its peak resident memory where the count is its own. On a Unix, the children this
/// process waited for, of which `child` is the one.
#[cfg(unix)]
fn usage(_child: &Child) -> Result<Spent, String> {
    use nix::sys::resource::{UsageWho, getrusage};
    use nix::sys::time::TimeVal;
    let usage =
        getrusage(UsageWho::RUSAGE_CHILDREN).map_err(|error| format!("getrusage: {error}"))?;
    let nanoseconds = |time: TimeVal| {
        let seconds = u64::try_from(time.tv_sec()).unwrap_or(0);
        let microseconds = u64::try_from(time.tv_usec()).unwrap_or(0);
        let whole = seconds.saturating_mul(1_000_000_000);
        whole.saturating_add(microseconds.saturating_mul(1000))
    };
    // macOS gives the peak in bytes, Linux in KiB.
    let peak = u64::try_from(usage.max_rss()).ok().map(|peak| {
        if cfg!(target_vendor = "apple") {
            peak
        } else {
            peak.saturating_mul(1024)
        }
    });
    Ok(Spent {
        user_ns: nanoseconds(usage.user_time()),
        system_ns: nanoseconds(usage.system_time()),
        peak_bytes: peak.filter(|peak| the_childs_own(*peak)),
        ran: None,
    })
}

/// What the system counted for `child`, which has ended: its CPU time in user space and in the
/// kernel, and its peak working set, as Windows counts them for the process.
#[cfg(windows)]
fn usage(child: &Child) -> Result<Spent, String> {
    let usage = lengua_os::process_usage(child)?;
    Ok(Spent {
        user_ns: usage.user_ns,
        system_ns: usage.kernel_ns,
        peak_bytes: Some(usage.peak_bytes),
        ran: None,
    })
}

/// Whether `peak`, the peak resident memory the system counted for a child of this process, is
/// the child's own. Linux begins a child's count at what this process held when it started the
/// child, so a peak no higher than this process's own may be this process's and not the child's;
/// macOS starts a child with memory of its own.
fn the_childs_own(peak: u64) -> bool {
    !cfg!(target_os = "linux") || own_peak().is_some_and(|own| peak > own)
}

/// The most memory this process has held resident at once, in bytes, as Linux says in
/// /proc/self/status.
fn own_peak() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let mut lines = status.lines();
    let line = lines.find_map(|line| line.strip_prefix("VmHWM:"))?;
    let kib = line.trim().strip_suffix("kB")?;
    let kib = kib.trim().parse::<u64>().ok()?;
    Some(kib.saturating_mul(1024))
}

/// The CPU's counters of a program's misses, Linux's generic cache events on reads in user space.
#[cfg(target_os = "linux")]
mod counters {
    use perf_event::events::{Cache, CacheId, CacheOp, CacheResult};
    use perf_event::{Builder, Counter};

    /// The caches whose misses are counted: the L1 data cache, the last level and the data TLB.
    const CACHES: [CacheId; 3] = [CacheId::L1D, CacheId::LL, CacheId::DTLB];

    /// Counters on process `pid` and the threads and processes it starts, from now on: one for
    /// each of [`CACHES`], `None` where the CPU or the system doesn't count it.
    pub fn attach(pid: u32) -> Vec<Option<Counter>> {
        let Ok(pid) = i32::try_from(pid) else {
            return Vec::new();
        };
        CACHES
            .into_iter()
            .map(|which| {
                let mut builder = Builder::new(Cache {
                    which,
                    operation: CacheOp::READ,
                    result: CacheResult::MISS,
                });
                builder.observe_pid(pid).inherit(true);
                builder.exclude_kernel(true);
                let mut counter = builder.build().ok()?;
                counter.enable().ok()?;
                Some(counter)
            })
            .collect()
    }

    /// What `counters` counted, in [`CACHES`]' order.
    pub fn read(counters: &mut [Option<Counter>]) -> [Option<u64>; 3] {
        let mut counts = [None; 3];
        for (count, counter) in counts.iter_mut().zip(counters.iter_mut()) {
            *count = counter.as_mut().and_then(|counter| counter.read().ok());
        }
        counts
    }
}

/// No counters, where the system has none lengua reads.
#[cfg(not(target_os = "linux"))]
mod counters {
    /// A counter there is none of.
    pub type Counter = std::convert::Infallible;

    /// No counters.
    pub fn attach(_pid: u32) -> Vec<Option<Counter>> {
        Vec::new()
    }

    /// Nothing counted.
    pub fn read(_counters: &mut [Option<Counter>]) -> [Option<u64>; 3] {
        [None; 3]
    }
}

/// Runs the module that `spec`, a run as JSON, names, in lengua's host and attached to this
/// process's own streams, as [`run_module_alone`] does, in lengua-mcp, which compiles a module
/// that has no compiled form yet. Gives the exit code: the module's own, [`TRAPPED`] when it
/// trapped, or [`REFUSED`] when lengua couldn't run it, which it says on standard error.
#[must_use]
pub fn run_module(spec: &str) -> i32 {
    match run_module_alone(spec) {
        Ok(code) => code,
        Err(NotRun::Refused(message) | NotRun::NotCompiled(message)) => {
            eprintln!("lengua-mcp: {message}");
            REFUSED
        }
        Err(NotRun::Secret) => REFUSED,
    }
}

/// A module of any toolchain, compiled, with what its words allow: what a module's run hands
/// its program's thread.
struct ModuleOrder<'r> {
    /// The run, as `measure` gave it.
    run: &'r ModuleRun,
    /// The module, compiled, in the engine that runs it.
    compiled: wasmtime::Module,
    /// What its `uses` and `network` words allow.
    policy: Policy,
    /// The port it serves on, if it serves, and whether every address may reach it.
    listen: Option<(u16, bool)>,
}

impl Order for ModuleOrder<'_> {
    fn module(&self) -> &wasmtime::Module {
        &self.compiled
    }

    fn here(&self) -> Here<'_> {
        Here {
            args: &self.run.args,
            env: &self.run.env,
            folder: &self.run.folder,
            policy: self.policy,
            listen: self.listen,
            connections: &[],
            source: Source::Unknown,
        }
    }
}

/// The module `run` names, compiled, in the engine that runs it: with a data folder, its
/// compiled form kept there under the user's key, as a checked program's is, loaded once its
/// seal holds, or made by a process that compiles, from the module's file, which must still be
/// the bytes `measure` hashed; with no data folder, compiled here and kept nowhere.
fn module_compiled(run: &ModuleRun) -> Result<wasmtime::Module, NotRun> {
    let module = Sha256Hex::parse(&run.sha256)
        .ok_or_else(|| "the module's run names no SHA-256 of the module".to_owned())?;
    let read = || {
        let wasm = std::fs::read(&run.module)
            .map_err(|error| format!("can't read {}: {error}", run.module.display()))?;
        if Sha256Hex::of(&wasm) == module {
            Ok(wasm)
        } else {
            Err(format!(
                "{} changed since measure read it",
                run.module.display()
            ))
        }
    };
    let Some(data) = &run.data else {
        if !engine::compiles() {
            return Err(NotRun::NotCompiled(
                "measure has no data folder to keep its compiled form in".to_owned(),
            ));
        }
        let compiler = engine::engine(Setup::Shell, None)?;
        return engine::compile(&compiler, &read()?)
            .map_err(|error| NotRun::Refused(format!("the module doesn't compile: {error}")));
    };
    let data_dir = DataDir::new(data.clone());
    let key = match Key::load(&data_dir).map_err(|error| error.to_string())? {
        Some(key) => key,
        None if engine::compiles() => {
            Key::load_or_create(&data_dir).map_err(|error| error.to_string())?
        }
        None => {
            return Err(NotRun::NotCompiled(
                "the data folder has no key to seal it with yet".to_owned(),
            ));
        }
    };
    crate::server::loaded_or_compiled(&data_dir, &key, &module, read)
}

/// Runs the module that `spec`, a run as JSON, names, in lengua's host and attached to this
/// process's own streams: in the engine's configuration for a run from a shell, counting nothing
/// and stopping nothing, with what the run's `uses` and `network` words allow, as an idea's
/// lines would, from its compiled form, as a checked program runs ([`module_compiled`]). Gives
/// the exit code: the module's own, or [`TRAPPED`] when it trapped, which it says on standard
/// error.
///
/// # Errors
///
/// Why lengua couldn't run it; or, in a process that only runs what was compiled before, that
/// the module has no compiled form it can load, which lengua-mcp makes when it is given the
/// same run.
pub fn run_module_alone(spec: &str) -> Result<i32, NotRun> {
    let run: ModuleRun = serde_json::from_str(spec)
        .map_err(|error| format!("the module's run can't be read: {error}"))?;
    let on = if engine::compiles() {
        On::Own
    } else {
        On::for_a_run_alone()
    };
    let ended = run_once_checked(on, || -> Result<ModuleOrder<'_>, NotRun> {
        let words: Vec<&str> = run.uses.iter().map(String::as_str).collect();
        let (uses, network) =
            crate::idea::constraints(&words, &run.network).map_err(|problems| {
                problems
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            })?;
        Ok(ModuleOrder {
            run: &run,
            compiled: module_compiled(&run)?,
            policy: Policy::of(&uses, &network),
            listen: network.listens.map(|port| (port, network.serves_anywhere)),
        })
    })?;
    match ended.map_err(|error| format!("couldn't run the module: {error}"))? {
        End::Exited(code) => Ok(code),
        trapped @ End::Trapped(..) => {
            eprintln!(
                "{}: the module {}",
                engine::name(),
                trapped.describe(Limits::default())
            );
            Ok(TRAPPED)
        }
        other => Err(NotRun::Refused(other.describe(Limits::default()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `list` as a command line's words.
    fn words(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter().map(|word| (*word).to_owned())
    }

    /// The options come first, then the program, then its arguments, with or without `--`.
    #[test]
    fn measure_takes_its_options_then_the_program_then_its_arguments() {
        let request = Request::parse(words(&[
            "--in",
            "/tmp/work",
            "--stdin",
            "in.txt",
            "--stdout",
            "out.txt",
            "--env",
            "A=b",
            "--uses",
            "standard input, files",
            "--network",
            "listens on TCP port 8080",
            "main.wasm",
            "--",
            "-2",
            "3",
        ]))
        .expect("a request");
        assert_eq!(request.program, PathBuf::from("main.wasm"));
        assert_eq!(request.folder, Some(PathBuf::from("/tmp/work")));
        assert_eq!(request.stdin, Some(PathBuf::from("in.txt")));
        assert_eq!(request.stdout, Some(PathBuf::from("out.txt")));
        assert_eq!(request.env.get("A").map(String::as_str), Some("b"));
        assert_eq!(request.uses, ["standard input", "files"]);
        assert_eq!(request.network, "listens on TCP port 8080");
        assert_eq!(request.args, ["-2", "3"]);
        assert_eq!(request.serve, None);
        let plain = Request::parse(words(&["./add", "2", "3"])).expect("a request");
        assert_eq!(plain.args, ["2", "3"]);
        assert!(Request::parse(words(&["--unknown", "./add"])).is_err());
        assert!(Request::parse(words(&["--in"])).is_err());
        assert!(Request::parse(words(&[])).is_err());
    }

    /// A server's load: 64 clients of 1,000 requests unless the words say otherwise, and no
    /// clients without a server.
    #[test]
    fn a_server_is_measured_under_load() {
        let request = Request::parse(words(&["--serve", "8080", "./server"])).expect("a request");
        assert_eq!(
            request.serve,
            Some(Load {
                port: 8080,
                clients: 64,
                requests: 1000
            })
        );
        let few = Request::parse(words(&[
            "--serve",
            "8080",
            "--clients",
            "2",
            "--requests",
            "3",
            "./server",
        ]))
        .expect("a request");
        let load = few.serve.map(|load| (load.clients, load.requests));
        assert_eq!(load, Some((2, 3)));
        assert!(Request::parse(words(&["--clients", "2", "./server"])).is_err());
        assert!(Request::parse(words(&["--serve", "0", "./server"])).is_err());
        assert!(Request::parse(words(&["--serve", "8080", "--requests", "0", "./s"])).is_err());
    }

    /// A request is 63 letters and a newline, and the next differs from it.
    #[test]
    fn a_request_is_a_line_of_letters() {
        let line = request_line(3, 7);
        assert_eq!(line.len(), REQUEST_BYTES);
        assert_eq!(line[REQUEST_BYTES - 1], b'\n');
        let letters = &line[..REQUEST_BYTES - 1];
        assert!(letters.iter().all(u8::is_ascii_lowercase));
        assert_ne!(line, request_line(3, 8));
    }

    /// What kind of program a path is: a folder, a module, or anything else.
    #[test]
    fn a_folder_is_main_wat_and_a_wasm_file_a_module() {
        let folder = tempfile::tempdir().expect("a folder");
        assert_eq!(Kind::of(folder.path()), Kind::Wat);
        assert_eq!(Kind::of(&folder.path().join("main.wasm")), Kind::Wasm);
        assert_eq!(Kind::of(&folder.path().join("main")), Kind::Native);
    }

    /// The measures go out as JSON and come back the same.
    #[test]
    fn the_measures_are_json() {
        let measures = Measures {
            kind: Kind::Wasm,
            exit: Some(0),
            user_ns: 1,
            system_ns: 2,
            wall_ns: 3,
            peak_bytes: Some(4),
            size: 5,
            l1d_misses: None,
            llc_misses: None,
            dtlb_misses: None,
            answered: None,
            problem: None,
        };
        let text = serde_json::to_string(&measures).expect("JSON");
        assert!(text.contains("\"kind\":\"wasm\""), "{text}");
        let back: Measures = serde_json::from_str(&text).expect("the measures");
        assert_eq!(back, measures);
        assert_eq!(back.cpu_ns(), 3);
    }

    /// A native program's command has the program's arguments and environment and nothing of
    /// this process's.
    #[test]
    fn a_native_program_gets_its_arguments_and_environment_alone() {
        let request = Request {
            args: vec!["2".to_owned()],
            env: BTreeMap::from([("A".to_owned(), "b".to_owned())]),
            ..Request::default()
        };
        let folder = tempfile::tempdir().expect("a folder");
        let native = command_for(
            Kind::Native,
            None,
            &request,
            Path::new("/bin/true"),
            folder.path(),
        )
        .expect("a command");
        let args: Vec<_> = native.get_args().collect();
        assert_eq!(args, ["2"]);
        let env: Vec<_> = native
            .get_envs()
            .filter_map(|(name, value)| Some((name.to_str()?, value?.to_str()?)))
            .filter(|(name, _)| *name != "SystemRoot")
            .collect();
        assert_eq!(env, [("A", "b")]);
        // A main.wat program needs the data folder, where its lock's key is.
        assert!(command_for(Kind::Wat, None, &request, folder.path(), folder.path()).is_err());
    }

    /// Through the spawner, the command is the spawner's: the file it reports to, then the
    /// program and its arguments, with the program's environment and folder.
    #[test]
    fn the_spawner_starts_the_program_with_its_words() {
        let request = Request {
            args: vec!["2".to_owned()],
            env: BTreeMap::from([("A".to_owned(), "b".to_owned())]),
            ..Request::default()
        };
        let folder = tempfile::tempdir().expect("a folder");
        let program = Path::new("/bin/true");
        let native = command_for(Kind::Native, None, &request, program, folder.path());
        let start = Start::Spawned {
            spawner: PathBuf::from("/lengua/lengua-spawn"),
            report: tempfile::NamedTempFile::new().expect("a file"),
        };
        let spawned = start.command(native.expect("a command"));
        assert_eq!(spawned.get_program(), "/lengua/lengua-spawn");
        let args: Vec<_> = spawned.get_args().collect();
        assert_eq!(args.len(), 3, "{args:?}");
        assert_eq!(args[1..], ["/bin/true", "2"]);
        assert_eq!(spawned.get_current_dir(), Some(folder.path()));
        let env: Vec<_> = spawned
            .get_envs()
            .filter_map(|(name, value)| Some((name.to_str()?, value?.to_str()?)))
            .filter(|(name, _)| *name != "SystemRoot")
            .collect();
        assert_eq!(env, [("A", "b")]);
    }

    /// The spawner's report: an exit code or `signal`, then four counts; anything else is none.
    #[test]
    fn the_spawners_report_is_read_back() {
        let spent = Spent::reported("3 10 20 30 40\n").expect("a report");
        assert_eq!((spent.user_ns, spent.system_ns), (10, 20));
        assert_eq!(spent.peak_bytes, Some(40));
        assert_eq!(spent.ran, Some((Some(3), Duration::from_nanos(30))));
        let ended = Spent::reported("signal 1 2 3 4").expect("a report");
        assert_eq!(ended.ran.map(|ran| ran.0), Some(None));
        for text in ["", "3 1 2 3", "3 1 2 3 4 5", "x 1 2 3 4", "3 1 2 -3 4"] {
            assert_eq!(Spent::reported(text), None, "{text:?}");
        }
    }

    /// Linux says the most this process has held resident, and a child's peak no higher is no
    /// peak of the child's own.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_childs_peak_no_higher_than_this_processs_is_not_its_own() {
        let own = own_peak().expect("this process's peak");
        assert!(own > 0);
        assert!(!the_childs_own(own));
        assert!(the_childs_own(own + (1 << 40)));
    }
}
