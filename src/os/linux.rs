//! Linux's way of running a program for the tests and for `run`: the launcher that confines
//! itself and becomes the program, the kernel's own confinement (Landlock, seccomp and resource
//! limits), the supervisor that answers the test's clock and random bytes and watches the file
//! names, and the CPU's counters.
//!
//! The count comes from the CPU's performance counters. The counter must be attached before the
//! program starts, without `unsafe` code, so the server starts its own binary in launcher mode:
//! the launcher waits on a socket while the server attaches a counter that turns on at `exec`,
//! then confines itself and replaces itself with the program. Only the program's own
//! instructions are counted.
//!
//! The confinement is the kernel's. Landlock lets the program touch files only inside its working
//! folder and execute only itself, with no network; a resource limit caps the files it writes;
//! and seccomp stops it at any system call outside the list it was given. The checker has already
//! made sure the code makes no other calls: this makes the kernel keep the same promise.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{self, Write as _};
use std::os::unix::process::ExitStatusExt as _;
use std::path::Path;
use std::process::{ExitCode, Stdio};
use std::sync::Arc;
use std::time::Duration;

use landlock::{
    ABI, Access as _, AccessFs, AccessNet, PathBeneath, PathFd, Ruleset, RulesetAttr as _,
    RulesetCreatedAttr as _, RulesetStatus, Scope,
};
use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule, TargetArch,
};

use perf_event::events::{Event, Hardware, Software};
use perf_event::{Builder, Counter, ReadFormat};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::UnixListener;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::broker::{self, Broker, Hello, Mode, Notes, Outcome, flag};
use crate::connect::{self, Made, Reach, Relay};
use crate::platform::{Allowed, Pin};
use crate::runner::{End, Job, LAUNCH_FLAG, Limits, OPEN_FILES, Run, portable_run};
use crate::supervise::SUPERVISE_FLAG;
use crate::supply::Supply;
use crate::syscalls::host::{
    CLOCK_GETTIME, CLOCK_NANOSLEEP, GETDENTS64, GETRANDOM, MKDIRAT, OPENAT, RECVMSG, RENAMEAT,
    SENDMSG, UNLINKAT,
};

/// How long the launcher may take to start.
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(5);

/// The longest standard input written in pieces waits for the program to read a piece before
/// writing the next.
const STDIN_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// How often it looks whether the program has read it.
const STDIN_POLL: std::time::Duration = std::time::Duration::from_millis(1);

/// The most files read back from a working folder after a run.
const MAX_FILES_BACK: usize = 256;

/// The most bytes read back from a working folder after a run.
const MAX_BYTES_BACK: u64 = 256 * 1024 * 1024;

/// A counter on `pid` and every thread it starts, from its `exec`, or `None` when the system
/// doesn't allow it.
fn attach<E: Event>(event: E, pid: i32, with_kernel: bool) -> Option<Counter> {
    let mut builder = Builder::new(event);
    builder
        .observe_pid(pid)
        .inherit(true)
        .enable_on_exec(true)
        .exclude_kernel(!with_kernel)
        .read_format(ReadFormat::TOTAL_TIME_ENABLED | ReadFormat::TOTAL_TIME_RUNNING);
    builder.build().ok()
}

/// A counter's count, unless the CPU had to share it with other counters for part of the run,
/// which would make the count an estimate.
fn exact_count(counter: &mut Counter) -> Option<u64> {
    let data = counter.read_full().ok()?;
    (data.time_running() == data.time_enabled()).then(|| data.count())
}

/// The network as the launcher learns it from its argument: how many connections lengua hands
/// the program, and its listener.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Handover {
    connections: usize,
    listen: Option<u16>,
    serve_anywhere: bool,
    /// Whether the program's end of its channel to the broker comes with the go signal, to be
    /// descriptor 4,352.
    channel: bool,
}

impl Handover {
    /// Whether the program uses the network at all.
    fn is_used(&self) -> bool {
        self.connections > 0 || self.listen.is_some()
    }

    /// Reads what [`Net::encode`] wrote.
    fn decode(text: &str) -> Result<Self, String> {
        let mut net = Self::default();
        let bad = || format!("{text:?} isn't a network setting");
        for part in text.split(';').filter(|part| *part != "none") {
            match part.split_once('=') {
                Some(("connections", count)) => {
                    net.connections = count
                        .parse()
                        .ok()
                        .filter(|count| (1..=connect::MAX_CONNECTIONS).contains(count))
                        .ok_or_else(bad)?;
                }
                Some(("listen", port)) => net.listen = Some(port.parse().map_err(|_| bad())?),
                Some(("listen_any", port)) => {
                    net.listen = Some(port.parse().map_err(|_| bad())?);
                    net.serve_anywhere = true;
                }
                None if part == "channel" => net.channel = true,
                _ => return Err(bad()),
            }
        }
        Ok(net)
    }
}

/// A program that's running.
pub struct Started {
    child: tokio::process::Child,
    pid: i32,
    port: Option<u16>,
    began: std::time::Instant,
    limits: Limits,
    counters: [Option<Counter>; 3],
    writer: tokio::task::JoinHandle<()>,
    stdout: tokio::task::JoinHandle<io::Result<Vec<u8>>>,
    stderr: tokio::task::JoinHandle<io::Result<Vec<u8>>>,
    overflowed: mpsc::Receiver<()>,
    supervisor: Option<(tokio::process::Child, SupervisorLines)>,
    broker: Option<Brokered>,
    connected: Connected,
    /// The fresh working folder to read back, when it isn't one of the user's.
    work: Option<std::path::PathBuf>,
    _scratch: tempfile::TempDir,
}

type SupervisorLines = tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>;

/// A program's broker while the program runs: its service, which ends when the program does, and
/// what the program has told it.
struct Brokered {
    service: std::thread::JoinHandle<Outcome>,
    notes: Arc<Notes>,
    /// Whether the test supplies random bytes, so that how many the program asks for matters.
    random: bool,
}

/// lengua's ends of the connections it made for a running program.
#[derive(Default)]
struct Connected {
    /// The ones it relays to their places.
    relays: Vec<Relay>,
    /// The ones it handed over already ended, held until the program ends.
    held: Vec<tokio::net::UnixStream>,
    /// The test's own servers' ends, by the connection's number in the spec, from 0.
    peers: Vec<(usize, tokio::net::UnixStream)>,
    /// What became of each, in order.
    made: Vec<Made>,
}

impl Started {
    /// The program's process id.
    #[must_use]
    pub const fn pid(&self) -> i32 {
        self.pid
    }

    /// The loopback port it serves on, when it serves.
    #[must_use]
    pub const fn port(&self) -> Option<u16> {
        self.port
    }

    /// lengua's ends of the connections that go to the test's own servers, each with its
    /// connection's number in the spec, from 0: the test's server speaks on them.
    pub fn take_peers(&mut self) -> Vec<(usize, tokio::net::UnixStream)> {
        std::mem::take(&mut self.connected.peers)
    }

    /// What became of each connection lengua made for the program, in order.
    #[must_use]
    pub fn connections(&self) -> &[Made] {
        &self.connected.made
    }

    /// What the program's broker knows of whether the program waits for a connection or a key
    /// now, when the program has a broker: its edge says so before it waits and after.
    #[must_use]
    pub fn waiting(&self) -> Option<bool> {
        self.broker.as_ref().map(|broker| broker.notes.waiting())
    }

    /// Waits for the program to end, stopping it at the time or output limit.
    ///
    /// # Errors
    ///
    /// Fails when its outcome can't be read.
    pub async fn wait(self) -> io::Result<Run> {
        self.finish(false).await
    }

    /// Stops the program now, as lengua does a server that's still serving.
    ///
    /// # Errors
    ///
    /// Fails when its outcome can't be read.
    pub async fn stop(self) -> io::Result<Run> {
        self.finish(true).await
    }

    async fn finish(mut self, stop: bool) -> io::Result<Run> {
        let deadline = tokio::time::Instant::from_std(self.began + self.limits.timeout);
        let mut wall_time = Duration::ZERO;
        let exited = if stop { self.child.try_wait()? } else { None };
        let end = if let Some(status) = exited {
            wall_time = self.began.elapsed();
            match status.code() {
                Some(code) => End::Exited(code),
                None => End::Signaled(status.signal().unwrap_or(0)),
            }
        } else if stop {
            let _ = self.child.kill().await;
            wall_time = self.began.elapsed();
            End::Stopped
        } else {
            tokio::select! {
                status = self.child.wait() => {
                    wall_time = self.began.elapsed();
                    let status = status?;
                    match status.code() {
                        Some(code) => End::Exited(code),
                        None => End::Signaled(status.signal().unwrap_or(0)),
                    }
                }
                Some(()) = self.overflowed.recv() => {
                    let _ = self.child.kill().await;
                    End::OutputTooLarge
                }
                () = tokio::time::sleep_until(deadline) => {
                    let _ = self.child.kill().await;
                    End::TimedOut
                }
            }
        };
        self.writer.abort();
        // What the program sent last on its connections reaches their places; then lengua's
        // ends close, now that nothing can read them as a reset.
        for relay in &mut self.connected.relays {
            relay.finish().await;
        }
        self.connected.held.clear();
        let stdout = self.stdout.await.map_err(io::Error::other)??;
        let stderr = self.stderr.await.map_err(io::Error::other)??;
        let (supplied, random_asked, listed) = match (self.supervisor.as_mut(), self.broker.take())
        {
            (Some((supervisor, lines)), _) => match supervisor_verdict(supervisor, lines).await {
                Ok((asked, listed)) => (None, Some(asked), listed),
                Err(problem) => (Some(problem), None, false),
            },
            // The program ended, so its end of the channel has closed and the service with it.
            (
                None,
                Some(Brokered {
                    service,
                    notes,
                    random,
                }),
            ) => {
                let outcome = tokio::task::spawn_blocking(move || service.join())
                    .await
                    .map_err(io::Error::other)?;
                let problem = match outcome {
                    Ok(Outcome::Closed) => None,
                    Ok(Outcome::Refused(refusal)) => Some(format!(
                        "lengua's broker stopped serving the program, which sent what isn't a \
                         message of the protocol ({refusal:?})"
                    )),
                    Ok(Outcome::Failed(error)) => Some(format!(
                        "lengua's broker stopped serving the program: {error}"
                    )),
                    Err(_) => Some("lengua's broker stopped serving the program".to_owned()),
                };
                (
                    problem,
                    random.then(|| notes.random_asked()),
                    notes.listed(),
                )
            }
            (None, None) => (None, None, false),
        };
        let [instructions, cycles, cpu_time_ns] =
            self.counters.each_mut().map(|counter| match end {
                End::Exited(_) | End::Signaled(_) => counter.as_mut().and_then(exact_count),
                End::TimedOut | End::OutputTooLarge | End::Stopped => None,
            });
        let (files, folders) = match &self.work {
            Some(work) => match read_files(work) {
                Ok((files, folders)) => (Ok(files), folders),
                Err(problem) => (Err(problem), BTreeSet::new()),
            },
            None => (Ok(BTreeMap::new()), BTreeSet::new()),
        };
        Ok(Run {
            stdout,
            stderr,
            end,
            folders,
            instructions,
            cycles,
            cpu_time_ns,
            wall_time_ns: u64::try_from(wall_time.as_nanos()).unwrap_or(u64::MAX),
            files,
            supplied,
            random_asked,
            connections: std::mem::take(&mut self.connected.made),
            listed,
        })
    }
}

/// Starts `job` through `launcher` (this server's own binary) and returns once the program is
/// running: confined, counted, and serving on its port if it serves.
///
/// # Errors
///
/// Fails when the program can't be started.
pub async fn start(launcher: &Path, job: Job<'_>, limits: Limits) -> io::Result<Started> {
    let scratch = tempfile::tempdir()?;
    let work = scratch.path().join("work");
    let folder = match job.folder {
        Some(folder) => folder.to_path_buf(),
        None => {
            std::fs::create_dir(&work)?;
            write_files(&work, job.files)?;
            for folder in job.folders {
                std::fs::create_dir_all(work.join(folder))?;
            }
            work.clone()
        }
    };
    let input = match job.stdin_file {
        Some(path) => Stdio::from(std::fs::File::open(path)?),
        None => Stdio::piped(),
    };
    // A closed standard output is a pipe whose reader has gone before the program starts.
    let output = if job.stdout_closed {
        let (reader, writer) = std::io::pipe()?;
        drop(reader);
        Stdio::from(writer)
    } else {
        Stdio::piped()
    };
    let streams = [input, output, Stdio::piped()];
    let Launched {
        mut child,
        pid,
        port,
        supervisor,
        broker,
        counters,
        connected,
    } = launch_job(launcher, &job, &limits, &folder, scratch.path(), streams).await?;
    let began = std::time::Instant::now();

    let input = child.stdin.take();
    let output = child.stdout.take();
    if output.is_none() && !job.stdout_closed {
        return Err(io::Error::other("no stdout pipe"));
    }
    let errors = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("no stderr pipe"))?;
    let bytes = job.stdin.to_vec();
    let pieces = job.stdin_pieces.to_vec();
    let writer = tokio::spawn(async move {
        // A program may exit without reading its input; that's not an error.
        let Some(mut input) = input else {
            return;
        };
        if pieces.len() < 2 {
            let _ = input.write_all(&bytes).await;
            return;
        }
        // Each piece its own write, once the program has read all of the last, so a read ends
        // where a piece does however busy the computer is.
        let mut start = 0;
        let last = pieces.len() - 1;
        for (index, length) in pieces.into_iter().enumerate() {
            let end = (start + length).min(bytes.len());
            if input.write_all(&bytes[start..end]).await.is_err() {
                return;
            }
            let _ = input.flush().await;
            start = end;
            let waited = tokio::time::Instant::now();
            while index < last
                && rustix::io::ioctl_fionread(&input).is_ok_and(|unread| unread > 0)
                && waited.elapsed() < STDIN_WAIT
            {
                tokio::time::sleep(STDIN_POLL).await;
            }
        }
    });
    let (overflow, overflowed) = mpsc::channel::<()>(2);
    let stdout = match output {
        Some(output) => tokio::spawn(read_capped(output, limits.max_output, overflow.clone())),
        None => tokio::spawn(async { Ok(Vec::new()) }),
    };
    let stderr = tokio::spawn(read_capped(errors, limits.max_output, overflow));
    Ok(Started {
        child,
        pid,
        port,
        began,
        limits,
        counters,
        writer,
        stdout,
        stderr,
        overflowed,
        supervisor,
        broker,
        connected,
        work: job.folder.is_none().then_some(work),
        _scratch: scratch,
    })
}

/// Runs `job` attached to this process's own standard input, output and error, for as long as
/// it takes, as `lengua-mcp run` does from a shell: confined as any run is, in the folder the
/// job names. A server says where it serves on standard error. An interrupt, hangup or request
/// to terminate that reaches this process goes on to the program, as a service manager's stop
/// would. Returns how the program ended.
///
/// # Errors
///
/// Fails when the program can't be started, or the job names no folder.
pub async fn attached(launcher: &Path, job: Job<'_>) -> io::Result<End> {
    use std::os::unix::process::ExitStatusExt as _;
    let scratch = tempfile::tempdir()?;
    let folder = job
        .folder
        .ok_or_else(|| io::Error::other("a run from a shell needs a folder to run in"))?
        .to_path_buf();
    let limits = Limits {
        max_file_size: None,
        ..Limits::default()
    };
    let streams = [Stdio::inherit(), Stdio::inherit(), Stdio::inherit()];
    use tokio::signal::unix::{SignalKind, signal};
    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut hangup = signal(SignalKind::hangup())?;
    let mut terminate = signal(SignalKind::terminate())?;
    // A program that sets the terminal's mode may end without setting it back: the shell gets
    // it back as it was.
    let terminal = rustix::termios::tcgetattr(std::io::stdin()).ok();
    let Launched {
        mut child,
        pid,
        port,
        supervisor,
        mut connected,
        ..
    } = launch_job(launcher, &job, &limits, &folder, scratch.path(), streams).await?;
    if let Some(port) = port {
        if job.net.is_some_and(|net| net.serve_anywhere) {
            eprintln!("lengua-mcp: serving on port {port} of every address of this computer");
        } else {
            eprintln!("lengua-mcp: serving on 127.0.0.1:{port}");
        }
    }
    // The program can't tell a place that refused it from one that broke off; its user can.
    for made in &connected.made {
        if let Some(problem) = &made.problem {
            eprintln!(
                "lengua-mcp: couldn't connect to {} ({problem}), so the program got that \
                 connection already ended",
                made.place
            );
        }
    }
    let program = rustix::process::Pid::from_raw(pid);
    let forward = |signal: rustix::process::Signal| {
        // The program may have ended already.
        if let Some(program) = program {
            let _ = rustix::process::kill_process(program, signal);
        }
    };
    let status = loop {
        tokio::select! {
            status = child.wait() => break status?,
            _ = interrupt.recv() => forward(rustix::process::Signal::INT),
            _ = hangup.recv() => forward(rustix::process::Signal::HUP),
            _ = terminate.recv() => forward(rustix::process::Signal::TERM),
        }
    };
    for relay in &mut connected.relays {
        relay.finish().await;
    }
    connected.held.clear();
    if let Some((mut supervisor, _)) = supervisor {
        let _ = supervisor.wait().await;
    }
    if let Some(terminal) = terminal {
        let _ = rustix::termios::tcsetattr(
            std::io::stdin(),
            rustix::termios::OptionalActions::Now,
            &terminal,
        );
    }
    Ok(match status.code() {
        Some(code) => End::Exited(code),
        None => End::Signaled(status.signal().unwrap_or(0)),
    })
}

/// The launcher, confined and told to become the program, with its process id, the port it
/// serves on if it serves, the supervisor tracing it if one does, and its counters.
struct Launched {
    child: tokio::process::Child,
    pid: i32,
    port: Option<u16>,
    supervisor: Option<(tokio::process::Child, SupervisorLines)>,
    broker: Option<Brokered>,
    counters: [Option<Counter>; 3],
    connected: Connected,
}

/// Starts `job` through `launcher` in `folder`, with `streams` as its standard input, output and
/// error, and waits until the launcher has confined itself and been told to go on.
async fn launch_job(
    launcher: &Path,
    job: &Job<'_>,
    limits: &Limits,
    folder: &Path,
    scratch: &Path,
    streams: [Stdio; 3],
) -> io::Result<Launched> {
    portable_run(job.args, job.env).map_err(io::Error::other)?;
    let socket = scratch.join("go");
    let listener = UnixListener::bind(&socket)?;
    let net = job.net.cloned().unwrap_or_default();
    if net.connections.len() > connect::MAX_CONNECTIONS {
        return Err(io::Error::other(format!(
            "a program gets at most {} connections",
            connect::MAX_CONNECTIONS
        )));
    }
    // Each connection's two ends, and the ones lengua makes, made while the launcher starts. A
    // computer an argument names is read from the arguments now.
    let mut pairs = Vec::new();
    let mut making = Vec::new();
    let mut places = Vec::new();
    for reach in &net.connections {
        pairs.push(connect::pair()?);
        let resolved = connect::resolved(reach, job.args);
        places.push(resolved.as_ref().unwrap_or(reach).describe());
        making.push(match resolved {
            Ok(reach @ (Reach::ThisComputer(_) | Reach::Host(..) | Reach::Tls(..))) => {
                Some(tokio::spawn(async move { connect::make(&reach).await }))
            }
            Ok(Reach::Argument { .. } | Reach::Peer | Reach::Nowhere) => None,
            Err(problem) => Some(tokio::spawn(async move { Err(problem) })),
        });
    }
    // A program with a channel is served by the broker; the supervisor follows the rest.
    let supply = job
        .supply
        .filter(|supply| !supply.is_empty() && !job.emulated && !job.channel);
    // The calls the supply answers stop for the supervisor; the rest run as in any test.
    let mut traced = supply.map_or_else(Vec::new, |supply| {
        let mut traced = Vec::new();
        if supply.random.is_some() {
            traced.push(GETRANDOM);
        }
        if !supply.clock.is_empty() {
            traced.push(CLOCK_GETTIME);
        }
        traced
    });
    // A test's sleeps end at once; a real run's take as long as they say.
    let sleeps = job.supply.is_some()
        && !job.emulated
        && !job.channel
        && job
            .syscalls
            .iter()
            .any(|allowed| allowed.number == CLOCK_NANOSLEEP);
    if sleeps {
        traced.push(CLOCK_NANOSLEEP);
    }
    // A program that uses files has its names held to what every system takes, and its listings
    // watched, by the supervisor, in tests and real runs alike, where it can follow them.
    let files = !job.emulated
        && !job.channel
        && crate::supervise::AVAILABLE
        && job
            .syscalls
            .iter()
            .any(|allowed| FILE_CALLS.contains(&allowed.number));
    if files {
        traced.extend(
            FILE_CALLS
                .into_iter()
                .filter(|call| job.syscalls.iter().any(|allowed| allowed.number == *call)),
        );
    }
    let watched = supply.is_some() || sleeps || files;
    let nothing_supplied = Supply::default();
    let supply = supply.or_else(|| (sleeps || files).then_some(&nothing_supplied));
    let mode = match (files, job.reversed) {
        (false, _) => "plain",
        (true, false) => "files",
        (true, true) => "files reversed",
    };
    let syscalls: Vec<String> = if job.emulated {
        vec![EMULATED.to_owned()]
    } else {
        job.syscalls
            .iter()
            .map(|allowed| format!("{}{}", allowed.number, allowed.pin.encode()))
            // The edge looks for its channel at its first need. Where lengua gave it none the
            // look finds nothing, which the kernel lets it do on that descriptor alone.
            .chain([SENDMSG, RECVMSG].map(|number| format!("{number}:channel")))
            .chain(traced.iter().map(|number| format!("{number}:trace")))
            .collect()
    };
    let file_limit = limits
        .max_file_size
        .map_or_else(|| "none".to_owned(), |bytes| bytes.to_string());
    // A program in the edge form that links the channel has a broker, a thread of this process:
    // the program's end of the channel goes to the launcher with the go signal, and the first
    // message is in it already, so that the program finds it when it starts.
    let (channel, broker) = if job.channel {
        let (program, broker) = start_broker(job, limits, folder)?;
        (Some(program), Some(broker))
    } else {
        (None, None)
    };
    let mut supervisor = match supply.filter(|_| watched) {
        None => None,
        Some(_) => Some(
            Command::new(launcher)
                .arg(SUPERVISE_FLAG)
                .env_clear()
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()?,
        ),
    };
    let tracer = supervisor
        .as_ref()
        .and_then(tokio::process::Child::id)
        .map_or_else(|| "none".to_owned(), |pid| pid.to_string());
    let [input, output, errors] = streams;
    let mut child = Command::new(launcher)
        .arg(LAUNCH_FLAG)
        .arg(&socket)
        .arg(syscalls.join(","))
        .arg(file_limit)
        .arg(tracer)
        .arg(if job.channel {
            format!("{};channel", net.encode())
        } else {
            net.encode()
        })
        .arg(encode_environment(job.env))
        .arg(job.program)
        .args(job.args)
        .current_dir(folder)
        .env_clear()
        .stdin(input)
        .stdout(output)
        .stderr(errors)
        .kill_on_drop(true)
        .spawn()?;
    let pid = child
        .id()
        .and_then(|pid| i32::try_from(pid).ok())
        .ok_or_else(|| io::Error::other("the launcher has no process id"))?;
    let accepted = tokio::time::timeout(LAUNCH_TIMEOUT, listener.accept()).await;
    let Ok(Ok((mut go, _))) = accepted else {
        // The launcher says why on its standard error, such as a port that's taken.
        let _ = child.kill().await;
        let mut said = String::new();
        if let Some(mut errors) = child.stderr.take() {
            let _ = errors.read_to_string(&mut said).await;
        }
        let said = said.trim().trim_start_matches("lengua-mcp: ");
        return Err(io::Error::other(if said.is_empty() {
            "the launcher didn't start".to_owned()
        } else {
            said.to_owned()
        }));
    };
    // A server's launcher has already bound its port, and says which.
    let mut port = None;
    if net.listen.is_some() {
        let mut bytes = [0_u8; 2];
        go.read_exact(&mut bytes).await?;
        port = Some(u16::from_be_bytes(bytes));
    }
    // The launcher has let the supervisor trace it; now it may, before the program starts.
    let supervisor = match (supervisor.take(), supply) {
        (Some(mut supervisor), Some(supply)) => {
            let lines = attach_supervisor(&mut supervisor, pid, mode, supply).await?;
            Some((supervisor, lines))
        }
        _ => None,
    };
    // Count from the program's exec to its exit; unavailable when the system doesn't allow
    // performance counters, and not counted under QEMU, whose instructions they'd be.
    let counters = if job.emulated {
        [None, None, None]
    } else {
        [
            attach(Hardware::INSTRUCTIONS, pid, false),
            attach(Hardware::CPU_CYCLES, pid, false),
            attach(Software::TASK_CLOCK, pid, true)
                .or_else(|| attach(Software::TASK_CLOCK, pid, false)),
        ]
    };
    // Every connection is made, or found not to be, before the program starts: its first read
    // or write on one never depends on how soon lengua got through.
    let mut connected = Connected::default();
    let mut ends = Vec::new();
    for (number, (((pair, reach), making), place)) in pairs
        .into_iter()
        .zip(&net.connections)
        .zip(making)
        .zip(places)
        .enumerate()
    {
        ends.push(pair.program);
        let problem = match (reach, making) {
            (Reach::Peer, _) => {
                connected.peers.push((number, pair.lengua));
                None
            }
            (_, Some(making)) => match making.await.map_err(|error| error.to_string()) {
                Ok(Ok(stream)) => {
                    connected.relays.push(Relay::start(pair.lengua, stream));
                    None
                }
                Ok(Err(problem)) | Err(problem) => {
                    connect::shut(&pair.lengua);
                    connected.held.push(pair.lengua);
                    Some(problem)
                }
            },
            (_, None) => {
                connect::shut(&pair.lengua);
                connected.held.push(pair.lengua);
                Some("nothing listens there".to_owned())
            }
        };
        connected.made.push(Made { place, problem });
    }
    // The channel's end is the last the launcher is handed.
    ends.extend(channel);
    send_go(&go, &ends).await?;
    // The program's ends are its own now: lengua's copies go, so the program's exit is the end
    // of what it sends.
    drop(ends);
    drop(go);
    Ok(Launched {
        child,
        pid,
        port,
        supervisor,
        broker,
        counters,
        connected,
    })
}

/// Starts the broker of `job`, which serves the program's folder, and returns the program's end
/// of the channel. The first message is sent now, so that the program finds it waiting, and a
/// thread answers what the program asks until it ends.
fn start_broker(
    job: &Job<'_>,
    limits: &Limits,
    folder: &Path,
) -> io::Result<(std::os::unix::net::UnixStream, Brokered)> {
    use rustix::fs::{Mode as FileMode, OFlags};
    let (program, mut lengua) = std::os::unix::net::UnixStream::pair()?;
    let root = rustix::fs::open(
        folder,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        FileMode::empty(),
    )?;
    let supply = job.supply.cloned().unwrap_or_default();
    let random = supply.random.is_some();
    let mut flags = 0;
    if job.reversed {
        flags |= flag::REVERSED;
    }
    if random {
        flags |= flag::RANDOM;
    }
    if !supply.clock.is_empty() {
        flags |= flag::CLOCK;
    }
    let hello = Hello {
        mode: if job.supply.is_some() {
            Mode::Test
        } else {
            Mode::Run
        },
        flags,
        file_limit: limits.max_file_size.unwrap_or(0),
    };
    let mut broker = Broker::new(root, hello, supply);
    broker::greet(&broker, &mut lengua)?;
    let notes = broker.notes();
    let service = std::thread::Builder::new()
        .name("lengua broker".to_owned())
        .spawn(move || broker::answer_all(&mut broker, &mut lengua))?;
    Ok((
        program,
        Brokered {
            service,
            notes,
            random,
        },
    ))
}

/// Tells the launcher to go on, handing it the program's ends of its connections, and last the
/// channel's, when it has one.
async fn send_go(
    go: &tokio::net::UnixStream,
    ends: &[std::os::unix::net::UnixStream],
) -> io::Result<()> {
    use std::mem::MaybeUninit;
    use std::os::fd::AsFd;

    use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags};
    let descriptors: Vec<_> = ends.iter().map(AsFd::as_fd).collect();
    loop {
        go.writable().await?;
        let mut space =
            [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(connect::MAX_CONNECTIONS + 1))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        if !descriptors.is_empty() && !control.push(SendAncillaryMessage::ScmRights(&descriptors)) {
            return Err(io::Error::other("too many connections to hand over"));
        }
        let sent = go.try_io(tokio::io::Interest::WRITABLE, || {
            rustix::net::sendmsg(
                go,
                &[io::IoSlice::new(&[1])],
                &mut control,
                SendFlags::empty(),
            )
            .map_err(io::Error::from)
        });
        match sent {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error),
        }
    }
}

/// Gives the supervisor the launcher's process id and the supply, and waits until it traces the
/// launcher. Returns the reader of its last word.
async fn attach_supervisor(
    supervisor: &mut tokio::process::Child,
    launcher: i32,
    mode: &str,
    supply: &Supply,
) -> io::Result<SupervisorLines> {
    use tokio::io::AsyncBufReadExt as _;
    let mut input = supervisor
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("no supervisor stdin"))?;
    input
        .write_all(format!("{launcher}\n{mode}\n{}", supply.encode()).as_bytes())
        .await?;
    drop(input);
    let output = supervisor
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("no supervisor stdout"))?;
    let mut lines = tokio::io::BufReader::new(output).lines();
    let first = tokio::time::timeout(LAUNCH_TIMEOUT, lines.next_line())
        .await
        .map_err(|_| io::Error::other("lengua's test supervisor didn't start"))??;
    match first.as_deref() {
        Some("attached") => Ok(lines),
        Some(line) => Err(io::Error::other(
            line.strip_prefix("error: ").unwrap_or(line).to_owned(),
        )),
        None => Err(io::Error::other(
            "lengua's test supervisor ended before the program started",
        )),
    }
}

/// What the supervisor found once the program ended: how many random bytes the program asked
/// for and whether it listed a folder, or what went wrong.
async fn supervisor_verdict(
    supervisor: &mut tokio::process::Child,
    lines: &mut SupervisorLines,
) -> Result<(usize, bool), String> {
    let last = tokio::time::timeout(LAUNCH_TIMEOUT, lines.next_line()).await;
    let _ = supervisor.kill().await;
    let found = |line: &str| -> Option<(usize, bool)> {
        let (asked, listed) = line.strip_prefix("ok ")?.split_once(' ')?;
        Some((asked.parse().ok()?, listed == "1"))
    };
    match last {
        Ok(Ok(Some(line))) => {
            found(&line).ok_or_else(|| line.strip_prefix("error: ").unwrap_or(&line).to_owned())
        }
        _ => Err("lengua's test supervisor stopped before the program ended".into()),
    }
}

/// The file calls whose names lengua's supervisor holds to what every system takes, and the
/// listing it watches.
const FILE_CALLS: [i64; 5] = [OPENAT, MKDIRAT, UNLINKAT, RENAMEAT, GETDENTS64];

/// A program's environment as the launcher's argument: each `NAME=value` in hex, joined by
/// commas, or `none`. Only the program gets it, never the launcher, which runs before its
/// confinement and would read variables such as `LD_PRELOAD`.
fn encode_environment(env: &BTreeMap<String, String>) -> String {
    if env.is_empty() {
        return "none".into();
    }
    env.iter()
        .map(|(name, value)| {
            format!("{name}={value}")
                .bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// The variables [`encode_environment`] wrote, or `None` for anything it didn't.
fn decode_environment(text: &str) -> Option<Vec<(String, String)>> {
    if text == "none" {
        return Some(Vec::new());
    }
    text.split(',')
        .map(|hex| {
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
                .collect::<Option<Vec<u8>>>()?;
            let pair = String::from_utf8(bytes).ok()?;
            let (name, value) = pair.split_once('=')?;
            Some((name.to_owned(), value.to_owned()))
        })
        .collect()
}

/// Puts `files` in `folder`, making the folders their names include.
fn write_files(folder: &Path, files: &BTreeMap<String, String>) -> io::Result<()> {
    for (name, contents) in files {
        let path = folder.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, contents)?;
    }
    Ok(())
}

/// Every file under `folder`, and every folder, by name relative to it, or why they can't all be
/// read.
type Left = (BTreeMap<String, Vec<u8>>, BTreeSet<String>);

fn read_files(folder: &Path) -> Result<Left, String> {
    let unreadable = |error: io::Error| format!("its working folder couldn't be read: {error}");
    let mut files = BTreeMap::new();
    let mut folders = BTreeSet::new();
    let mut total = 0;
    let mut pending = vec![folder.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).map_err(unreadable)? {
            let entry = entry.map_err(unreadable)?;
            let path = entry.path();
            let name = path
                .strip_prefix(folder)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            let kind = entry.file_type().map_err(unreadable)?;
            if kind.is_dir() {
                folders.insert(name);
                pending.push(path);
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
            files.insert(name, std::fs::read(&path).map_err(unreadable)?);
        }
    }
    Ok((files, folders))
}

/// Reads until the end, keeping at most `cap` bytes and reporting when there's more.
async fn read_capped(
    mut reader: impl AsyncRead + Unpin,
    cap: usize,
    overflow: mpsc::Sender<()>,
) -> io::Result<Vec<u8>> {
    let mut kept = Vec::new();
    let mut chunk = vec![0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            return Ok(kept);
        }
        kept.extend_from_slice(&chunk[..read]);
        if kept.len() > cap {
            kept.truncate(cap);
            let _ = overflow.send(()).await;
            return Ok(kept);
        }
    }
}

/// Launcher mode: waits for the server's go signal on `socket`, confines itself, then becomes
/// the program with an empty environment. Arguments: the socket, the allowed system call numbers
/// separated by commas, the file size limit in bytes (or `none`), the supervisor's process id
/// (or `none`), the program, then the program's arguments.
#[must_use]
pub fn launch(mut args: impl Iterator<Item = OsString>) -> ExitCode {
    let (
        Some(socket),
        Some(syscalls),
        Some(file_limit),
        Some(tracer),
        Some(network),
        Some(environment),
        Some(program),
    ) = (
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
    )
    else {
        eprintln!(
            "lengua-mcp: {LAUNCH_FLAG} needs a socket, system calls, a file limit, a supervisor, \
             a network setting, an environment and a program"
        );
        return ExitCode::from(127);
    };
    let rest: Vec<OsString> = args.collect();
    let Some(environment) = decode_environment(&environment.to_string_lossy()) else {
        eprintln!("lengua-mcp: the environment isn't one lengua wrote");
        return ExitCode::from(127);
    };
    let net = match Handover::decode(&network.to_string_lossy()) {
        Ok(net) => net,
        Err(problem) => {
            eprintln!("lengua-mcp: {problem}");
            return ExitCode::from(127);
        }
    };
    // A server's listening socket comes first, so it's file descriptor 3; it stays open into
    // the program, which only takes connections on it.
    let listener = match net
        .listen
        .map(|port| listen_on_fd3(port, net.serve_anywhere))
        .transpose()
    {
        Ok(listener) => listener,
        Err(problem) => {
            eprintln!("lengua-mcp: {problem}");
            return ExitCode::from(127);
        }
    };
    // Only the test's supervisor may trace the program, even where the system restricts tracing.
    let supervisor = tracer
        .to_str()
        .and_then(|pid| pid.parse::<i32>().ok())
        .and_then(rustix::process::Pid::from_raw);
    if let Some(pid) = supervisor
        && let Err(error) = rustix::process::set_ptracer(rustix::process::PTracer::ProcessID(pid))
    {
        eprintln!("lengua-mcp: couldn't let the supervisor trace the program: {error}");
        return ExitCode::from(127);
    }
    let signalled = std::os::unix::net::UnixStream::connect(&socket).and_then(|mut stream| {
        if let Some(listener) = &listener {
            stream.write_all(&listener.local_addr()?.port().to_be_bytes())?;
        }
        receive_go(&stream)
    });
    let mut ends = match signalled {
        Ok(ends) => ends,
        Err(error) => {
            eprintln!("lengua-mcp: no go signal from the server: {error}");
            return ExitCode::from(127);
        }
    };
    // The channel's end is the last the server handed over.
    let channel = if net.channel {
        let Some(end) = ends.pop() else {
            eprintln!("lengua-mcp: the server handed over no channel to its broker");
            return ExitCode::from(127);
        };
        Some(end)
    } else {
        None
    };
    // Each connection at its own number, from 4, kept open into the program.
    let connections = match place_connections(ends, net.connections) {
        Ok(connections) => connections,
        Err(problem) => {
            eprintln!("lengua-mcp: {problem}");
            return ExitCode::from(127);
        }
    };
    // The channel at the number the edge looks for it at, kept open as well.
    let channel = match channel.map(place_channel).transpose() {
        Ok(channel) => channel,
        Err(problem) => {
            eprintln!("lengua-mcp: {problem}");
            return ExitCode::from(127);
        }
    };
    // Nothing else reaches the program: no descriptor a parent of lengua's left open.
    if let Err(problem) = close_strays(listener.is_some(), connections.len(), channel.is_some()) {
        eprintln!("lengua-mcp: {problem}");
        return ExitCode::from(127);
    }
    let confined = parse_confinement(&syscalls, &file_limit).and_then(
        |(syscalls, traced, file_limit, emulated)| {
            confine(
                Path::new(&program),
                &syscalls,
                &traced,
                file_limit,
                &net,
                emulated,
            )
        },
    );
    if let Err(problem) = confined {
        eprintln!("lengua-mcp: couldn't confine the program: {problem}");
        return ExitCode::from(127);
    }
    // The program's own variables reach it only now: the launcher itself ran with none. It
    // keeps the launcher's ignored SIGPIPE, which std's `Command` would set back: a write to a
    // closed pipe or connection is an error on every target, as Windows makes it, not a signal.
    let error = become_program(&program, &rest, &environment);
    drop(connections);
    drop(channel);
    eprintln!(
        "lengua-mcp: couldn't start {}: {error}",
        Path::new(&program).display()
    );
    ExitCode::from(127)
}

/// Closes every descriptor the launcher holds that the program isn't handed: all but standard
/// input, output and error, a server's listener (3), its connections (4 and on) and its channel
/// to the broker. lengua's own descriptors close on exec, but one a parent of lengua's left open
/// without that flag, a harness or a CI runner, would otherwise reach the program, outside its
/// confinement.
fn close_strays(listening: bool, connections: usize, channel: bool) -> Result<(), String> {
    let first = connect::FIRST_DESCRIPTOR;
    let last = first + i32::try_from(connections).unwrap_or(i32::MAX);
    let channel_at = i32::try_from(crate::edge::CHANNEL).unwrap_or(i32::MAX);
    let kept = |fd: i32| {
        fd <= 2
            || (fd == 3 && listening)
            || (first..last).contains(&fd)
            || (channel && fd == channel_at)
    };
    let open: Vec<i32> = std::fs::read_dir("/proc/self/fd")
        .map_err(|error| format!("couldn't list the launcher's descriptors: {error}"))?
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    for fd in open.into_iter().filter(|fd| !kept(*fd)) {
        // The listing's own descriptor has closed already.
        match nix::unistd::close(fd) {
            Ok(()) | Err(nix::errno::Errno::EBADF) => {}
            Err(error) => return Err(format!("couldn't close descriptor {fd}: {error}")),
        }
    }
    Ok(())
}

/// Replaces this process with `program`, given `args` and only `environment`, keeping what the
/// launcher set up: its confinement, its descriptors and its ignored SIGPIPE. Returns only when
/// it can't.
fn become_program(
    program: &OsString,
    args: &[OsString],
    environment: &[(String, String)],
) -> String {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt as _;
    let text = |bytes: &[u8]| CString::new(bytes).map_err(|_| "a NUL in its arguments".to_owned());
    let made = || -> Result<(CString, Vec<CString>, Vec<CString>), String> {
        let path = text(program.as_bytes())?;
        let mut argv = vec![path.clone()];
        for arg in args {
            argv.push(text(arg.as_bytes())?);
        }
        let mut envp = Vec::new();
        for (name, value) in environment {
            envp.push(text(format!("{name}={value}").as_bytes())?);
        }
        Ok((path, argv, envp))
    };
    let (path, argv, envp) = match made() {
        Ok(made) => made,
        Err(problem) => return problem,
    };
    // The signals the launcher blocked, none, are blocked for the program too: start it with
    // none, as std's `Command` would.
    let _ = nix::sys::signal::sigprocmask(
        nix::sys::signal::SigmaskHow::SIG_SETMASK,
        Some(&nix::sys::signal::SigSet::empty()),
        None,
    );
    match nix::unistd::execve(&path, &argv, &envp) {
        Ok(never) => match never {},
        Err(errno) => io::Error::from(errno).to_string(),
    }
}

/// Reads the server's go signal, and the program's ends of its connections with it.
fn receive_go(stream: &std::os::unix::net::UnixStream) -> io::Result<Vec<std::os::fd::OwnedFd>> {
    use std::mem::MaybeUninit;

    use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags};
    let mut go = [0_u8; 1];
    let mut space =
        [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(connect::MAX_CONNECTIONS + 1))];
    let mut control = RecvAncillaryBuffer::new(&mut space);
    let received = rustix::net::recvmsg(
        stream,
        &mut [io::IoSliceMut::new(&mut go)],
        &mut control,
        RecvFlags::CMSG_CLOEXEC,
    )?;
    if received.bytes != 1 {
        return Err(io::Error::other("the server ended without one"));
    }
    let mut ends = Vec::new();
    for message in control.drain() {
        if let RecvAncillaryMessage::ScmRights(descriptors) = message {
            ends.extend(descriptors);
        }
    }
    Ok(ends)
}

/// Puts each of the program's connections at its own number, from
/// [`connect::FIRST_DESCRIPTOR`] in order, kept open across `exec`; checks there are `count`.
fn place_connections(
    ends: Vec<std::os::fd::OwnedFd>,
    count: usize,
) -> Result<Vec<std::os::fd::OwnedFd>, String> {
    use std::os::fd::AsRawFd as _;
    if ends.len() != count {
        return Err(format!(
            "the server handed over {} connections, not {count}",
            ends.len()
        ));
    }
    // Out of the way first, so nothing holds a number a connection needs.
    let aside = ends
        .iter()
        .map(|end| rustix::io::fcntl_dupfd_cloexec(end, 1000))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("couldn't keep the connections: {error}"))?;
    drop(ends);
    let mut placed = Vec::new();
    for (index, end) in aside.iter().enumerate() {
        let wanted = connect::FIRST_DESCRIPTOR + i32::try_from(index).unwrap_or(i32::MAX);
        let moved = rustix::io::fcntl_dupfd_cloexec(end, wanted)
            .map_err(|error| format!("couldn't keep the connections: {error}"))?;
        if moved.as_raw_fd() != wanted {
            return Err(format!(
                "connection {} isn't file descriptor {wanted}",
                index + 1
            ));
        }
        rustix::io::fcntl_setfd(&moved, rustix::io::FdFlags::empty())
            .map_err(|error| format!("couldn't keep the connections open: {error}"))?;
        placed.push(moved);
    }
    Ok(placed)
}

/// Puts the program's end of its channel to the broker at descriptor [`crate::edge::CHANNEL`],
/// kept open across `exec`. The launcher's own limit on descriptor numbers may be below it: the
/// program's is set at its confinement, to the number of descriptors it holds, which leaves
/// the channel above them.
fn place_channel(end: std::os::fd::OwnedFd) -> Result<std::os::fd::OwnedFd, String> {
    use std::os::fd::AsRawFd as _;

    use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};
    let wanted = i32::try_from(crate::edge::CHANNEL).unwrap_or(i32::MAX);
    let limit = getrlimit(Resource::Nofile);
    let needed = u64::from(crate::edge::CHANNEL) + 1;
    if limit.current.is_some_and(|current| current < needed) {
        setrlimit(
            Resource::Nofile,
            Rlimit {
                current: Some(needed),
                maximum: limit.maximum,
            },
        )
        .map_err(|error| {
            format!(
                "this computer won't let lengua hold descriptor {wanted} for a program's channel \
                 ({error}); raise the hard limit (`ulimit -Hn`)"
            )
        })?;
    }
    let moved = rustix::io::fcntl_dupfd_cloexec(&end, wanted)
        .map_err(|error| format!("couldn't keep the channel: {error}"))?;
    if moved.as_raw_fd() != wanted {
        return Err(format!("the channel isn't file descriptor {wanted}"));
    }
    rustix::io::fcntl_setfd(&moved, rustix::io::FdFlags::empty())
        .map_err(|error| format!("couldn't keep the channel open: {error}"))?;
    Ok(moved)
}

/// Listens on `port` of the loopback (any free port for 0) with file descriptor 3, kept open
/// across `exec`.
fn listen_on_fd3(port: u16, anywhere: bool) -> Result<std::net::TcpListener, String> {
    use std::os::fd::AsRawFd as _;
    let address = if anywhere {
        std::net::Ipv4Addr::UNSPECIFIED
    } else {
        std::net::Ipv4Addr::LOCALHOST
    };
    let listener = std::net::TcpListener::bind((address, port))
        .map_err(|error| format!("couldn't listen on {address}:{port}: {error}"))?;
    if listener.as_raw_fd() != 3 {
        return Err("the listening socket isn't file descriptor 3".into());
    }
    rustix::io::fcntl_setfd(&listener, rustix::io::FdFlags::empty())
        .map_err(|error| format!("couldn't keep the listening socket open: {error}"))?;
    Ok(listener)
}

/// The launcher's confinement: the calls allowed, the calls the supervisor answers, the file
/// size limit, and whether QEMU's user mode runs the program.
type Confinement = (Vec<Allowed>, Vec<i64>, Option<u64>, bool);

/// The system call list the launcher gets for a program QEMU's user mode runs: the kernel sees
/// QEMU's calls, not the program's, so it can't hold the program to its spec's.
const EMULATED: &str = "emulated";

/// The calls QEMU's user mode never needs to make for a program, which a program run through it
/// may not make either: tracing or reading other processes, the kernel's own business (modules,
/// keys, mounts, namespaces, clocks, rebooting), and the interfaces that reach the kernel past
/// a filter on calls (io_uring, BPF). The program's own calls are held to its spec by lengua's
/// rules; these are the kernel's limit on QEMU.
const NEVER_EMULATED: [i64; 37] = [
    nix::libc::SYS_ptrace,
    nix::libc::SYS_process_vm_readv,
    nix::libc::SYS_process_vm_writev,
    nix::libc::SYS_perf_event_open,
    nix::libc::SYS_bpf,
    nix::libc::SYS_userfaultfd,
    nix::libc::SYS_io_uring_setup,
    nix::libc::SYS_io_uring_enter,
    nix::libc::SYS_io_uring_register,
    nix::libc::SYS_mount,
    nix::libc::SYS_umount2,
    nix::libc::SYS_pivot_root,
    nix::libc::SYS_chroot,
    nix::libc::SYS_unshare,
    nix::libc::SYS_setns,
    nix::libc::SYS_open_tree,
    nix::libc::SYS_move_mount,
    nix::libc::SYS_fsopen,
    nix::libc::SYS_fsconfig,
    nix::libc::SYS_fsmount,
    nix::libc::SYS_fspick,
    nix::libc::SYS_mount_setattr,
    nix::libc::SYS_open_by_handle_at,
    nix::libc::SYS_init_module,
    nix::libc::SYS_finit_module,
    nix::libc::SYS_delete_module,
    nix::libc::SYS_kexec_load,
    nix::libc::SYS_kexec_file_load,
    nix::libc::SYS_keyctl,
    nix::libc::SYS_add_key,
    nix::libc::SYS_request_key,
    nix::libc::SYS_swapon,
    nix::libc::SYS_swapoff,
    nix::libc::SYS_reboot,
    nix::libc::SYS_acct,
    nix::libc::SYS_settimeofday,
    nix::libc::SYS_clock_settime,
];

/// Reads the launcher's system call list ("0,1,56:3477248,318:trace": a number and, after a
/// colon, the only first argument allowed, or `trace` for a call the supervisor answers; or
/// [`EMULATED`]) and file limit ("1048576" or "none").
fn parse_confinement(syscalls: &OsString, file_limit: &OsString) -> Result<Confinement, String> {
    let syscalls = syscalls.to_string_lossy();
    let emulated = syscalls == EMULATED;
    let syscalls = if emulated { "".into() } else { syscalls };
    let mut traced = Vec::new();
    for entry in syscalls.split(',') {
        if let Some(number) = entry.strip_suffix(":trace") {
            traced.push(
                number
                    .parse()
                    .map_err(|_| format!("{entry:?} isn't a system call"))?,
            );
        }
    }
    let numbers = syscalls
        .split(',')
        .filter(|entry| !entry.is_empty() && !entry.ends_with(":trace"))
        .map(|entry| {
            let bad = || format!("{entry:?} isn't a system call");
            let (number, pin) = match entry.split_once(':') {
                Some((number, pin)) => (number, Pin::decode(pin).ok_or_else(bad)?),
                None => (entry, Pin::Any),
            };
            Ok(Allowed {
                number: number.parse().map_err(|_| bad())?,
                pin,
            })
        })
        .collect::<Result<_, String>>()?;
    let file_limit = match file_limit.to_string_lossy().as_ref() {
        "none" => None,
        bytes => Some(
            bytes
                .parse()
                .map_err(|_| format!("{bytes:?} isn't a file size limit"))?,
        ),
    };
    Ok((numbers, traced, file_limit, emulated))
}

/// The seccomp rules a pin stands for: any one of them allows the call.
fn pin_rules(pin: Pin) -> Result<Vec<SeccompRule>, seccompiler::BackendError> {
    use SeccompCmpArgLen::{Dword, Qword};
    let condition = |argument, length, operator, value| {
        SeccompCondition::new(argument, length, operator, value)
    };
    const MSG_NOSIGNAL: u64 = 0x4000;
    match pin {
        Pin::Any => Ok(Vec::new()),
        Pin::First(value) => Ok(vec![SeccompRule::new(vec![condition(
            0,
            Qword,
            SeccompCmpOp::Eq,
            value,
        )?])?]),
        Pin::ListeningSocket => Ok(vec![SeccompRule::new(vec![condition(
            0,
            Dword,
            SeccompCmpOp::Eq,
            3,
        )?])?]),
        // lengua's channel to its broker, and nothing else.
        Pin::Channel => Ok(vec![SeccompRule::new(vec![condition(
            0,
            Dword,
            SeccompCmpOp::Eq,
            u64::from(crate::edge::CHANNEL),
        )?])?]),
        // TCGETS, TCSETS, TCSETSW, TCSETSF and TIOCGWINSZ, on standard input, output or error.
        Pin::Terminal => {
            let mut rules = Vec::new();
            for descriptor in [0, 1, 2] {
                for request in [0x5401, 0x5402, 0x5403, 0x5404, 0x5413] {
                    rules.push(SeccompRule::new(vec![
                        condition(0, Dword, SeccompCmpOp::Eq, descriptor)?,
                        condition(1, Qword, SeccompCmpOp::Eq, request)?,
                    ])?);
                }
            }
            Ok(rules)
        }
        // No flag but MSG_NOSIGNAL (MSG_FASTOPEN would connect), and no address.
        Pin::NoAddress => Ok(vec![SeccompRule::new(vec![
            condition(
                3,
                Dword,
                SeccompCmpOp::MaskedEq(!MSG_NOSIGNAL & 0xFFFF_FFFF),
                0,
            )?,
            condition(4, Qword, SeccompCmpOp::Eq, 0)?,
            condition(5, Dword, SeccompCmpOp::Eq, 0)?,
        ])?]),
    }
}

/// Confines this process, and so the program it becomes: files only inside the current folder
/// (its working folder), executing only `program`, no network and no signals to other
/// processes (Landlock); files of at most `file_limit` bytes and no core files; and only the
/// system calls in `syscalls` (seccomp), so any other stops it with SIGSYS. A program QEMU's
/// user mode runs (`emulated`) is held by Landlock and the limits alike, but its calls reach
/// the kernel as QEMU's, so the kernel stops only [`NEVER_EMULATED`].
fn confine(
    program: &Path,
    syscalls: &[Allowed],
    traced: &[i64],
    file_limit: Option<u64>,
    net: &Handover,
    emulated: bool,
) -> Result<(), String> {
    let abi = ABI::V6;
    let folder = PathFd::new(".").map_err(|error| format!("working folder: {error}"))?;
    let executable = PathFd::new(program).map_err(|error| format!("program: {error}"))?;
    let status = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .and_then(|ruleset| ruleset.handle_access(AccessNet::from_all(abi)))
        .and_then(|ruleset| ruleset.scope(Scope::from_all(abi)))
        .and_then(Ruleset::create)
        .and_then(|ruleset| ruleset.add_rule(PathBeneath::new(folder, AccessFs::from_all(abi))))
        .and_then(|ruleset| {
            ruleset.add_rule(PathBeneath::new(
                executable,
                AccessFs::Execute | AccessFs::ReadFile,
            ))
        })
        // No TCP connection or binding at all: lengua makes the program's connections and
        // binds a server's socket before it starts.
        .and_then(|ruleset| ruleset.restrict_self())
        .map_err(|error| format!("Landlock: {error}"))?;
    // A program on the network is held off connecting or binding by Landlock as well as
    // seccomp: a kernel that can't hold it so doesn't run it.
    if net.is_used() && status.ruleset != RulesetStatus::FullyEnforced {
        return Err(
            "this kernel can't hold the program off the network by Landlock; that needs Linux \
             6.7 or newer"
                .into(),
        );
    }
    let limit = |bytes| rustix::process::Rlimit {
        current: Some(bytes),
        maximum: Some(bytes),
    };
    if let Some(bytes) = file_limit {
        rustix::process::setrlimit(rustix::process::Resource::Fsize, limit(bytes))
            .map_err(|error| format!("file size limit: {error}"))?;
    }
    // A crash leaves no core file behind.
    rustix::process::setrlimit(rustix::process::Resource::Core, limit(0))
        .map_err(|error| format!("core file limit: {error}"))?;
    // The files it holds at once: one number on every target.
    rustix::process::setrlimit(rustix::process::Resource::Nofile, limit(OPEN_FILES)).map_err(
        |error| {
            format!(
                "this computer won't let a program hold {OPEN_FILES} open files, lengua's limit \
                 on every system ({error}); raise the hard limit (`ulimit -Hn`)"
            )
        },
    )?;
    let arch = TargetArch::try_from(std::env::consts::ARCH)
        .map_err(|error| format!("seccomp: {error}"))?;
    if emulated {
        let never = NEVER_EMULATED
            .iter()
            .map(|number| (*number, Vec::new()))
            .collect();
        let filter: BpfProgram = SeccompFilter::new(
            never,
            SeccompAction::Allow,
            SeccompAction::KillProcess,
            arch,
        )
        .and_then(TryInto::try_into)
        .map_err(|error| format!("seccomp: {error}"))?;
        return seccompiler::apply_filter(&filter).map_err(|error| format!("seccomp: {error}"));
    }
    // No rules allow any arguments; each rule allows the arguments its conditions name.
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    let mut any = BTreeSet::new();
    for allowed in syscalls {
        if allowed.pin == Pin::Any {
            any.insert(allowed.number);
        } else {
            let pinned = pin_rules(allowed.pin).map_err(|error| format!("seccomp: {error}"))?;
            rules.entry(allowed.number).or_default().extend(pinned);
        }
    }
    for number in any {
        rules.insert(number, Vec::new());
    }
    // The calls a test's supply answers stop for its supervisor. This filter goes first: once
    // the next one is in, installing another would be a call it kills. The kernel applies the
    // stricter answer of the two, so these calls stop, and every other call is as below.
    if !traced.is_empty() {
        let stops = traced.iter().map(|number| (*number, Vec::new())).collect();
        let filter: BpfProgram =
            SeccompFilter::new(stops, SeccompAction::Allow, SeccompAction::Trace(0), arch)
                .and_then(TryInto::try_into)
                .map_err(|error| format!("seccomp: {error}"))?;
        seccompiler::apply_filter(&filter).map_err(|error| format!("seccomp: {error}"))?;
    }
    let filter = SeccompFilter::new(
        rules,
        SeccompAction::KillProcess,
        SeccompAction::Allow,
        arch,
    )
    .map_err(|error| format!("seccomp: {error}"))?;
    let filter: BpfProgram = filter
        .try_into()
        .map_err(|error| format!("seccomp: {error}"))?;
    seccompiler::apply_filter(&filter).map_err(|error| format!("seccomp: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_reaches_the_launcher_as_one_argument() {
        let env = BTreeMap::from([
            ("HOME".to_owned(), "/home/ada".to_owned()),
            ("NAME".to_owned(), "Ada, Countess = Lovelace".to_owned()),
        ]);
        let encoded = encode_environment(&env);
        assert!(!encoded.contains('='), "{encoded}");
        let decoded: BTreeMap<String, String> = decode_environment(&encoded)
            .expect("lengua's own encoding")
            .into_iter()
            .collect();
        assert_eq!(decoded, env);
        assert_eq!(encode_environment(&BTreeMap::new()), "none");
        assert_eq!(decode_environment("none"), Some(Vec::new()));
        // No `=`, and not hex: not what lengua writes.
        assert_eq!(decode_environment("4e414d45"), None);
        assert_eq!(decode_environment("zz"), None);
    }
}
