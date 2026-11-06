//! The pool of workers that run a server's WebAssembly programs: child processes of this binary
//! in worker mode (`lengua-mcp --wasm-worker`), each reused for job after job with a fresh store
//! each time, so a test pays for a process once. A worker that crashed or ran past its time is
//! killed and a fresh one starts for the next job; one that a run asks to have alone, such as the
//! `run` tool's or a background server's, is started for it and goes with it. A worker exits when
//! its standard input closes, so a killed server leaves no program behind.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::oneshot;

use crate::connect::{CONNECT_TIMEOUT, Reach};
use crate::runner::{Limits, Run, Trap};
use crate::supply::Supply;
use crate::wasm::engine::Setup;
use crate::wasm::host::policy::Policy;
use crate::wasm::job::{self, Connection, Done, Ending, Event, Job, Made, Request};
use crate::wasm::{Program, WORKER_FLAG, runner};

/// How much longer than its program's own time limit a job may take before lengua gives up on
/// its worker: the module's compiling and the worker's start.
const GRACE: Duration = Duration::from_secs(20);

/// What to run a program with.
#[derive(Debug, Clone, Copy)]
pub struct Spec<'a> {
    /// Its arguments, after its name.
    pub args: &'a [String],
    /// Its standard input, which then ends.
    pub stdin: &'a [u8],
    /// The pieces standard input arrives in, by length; whole when fewer than two.
    pub stdin_pieces: &'a [usize],
    /// Files put in its fresh working folder before it starts.
    pub files: &'a BTreeMap<String, String>,
    /// Empty folders put there.
    pub folders: &'a BTreeSet<String>,
    /// Its environment variables.
    pub env: &'a BTreeMap<String, String>,
    /// A folder to run in instead of a fresh one.
    pub folder: Option<&'a Path>,
    /// The random bytes and clock readings a test supplies.
    pub supply: Option<&'a Supply>,
    /// The connections lengua makes for it, in the idea's order.
    pub connections: &'a [Reach],
    /// The port it serves on, 0 for any free one, and whether every address may reach it.
    pub listen: Option<(u16, bool)>,
    /// Whether each folder lists turned round, as in a test's second run.
    pub reversed: bool,
    /// Whether its standard output is a pipe nobody reads.
    pub stdout_closed: bool,
    /// What its idea lets it reach.
    pub policy: Policy,
    /// The engine's configuration it runs in.
    pub setup: Setup,
    /// The most bytes it may write to files in all, if there's a limit.
    pub max_written: Option<u64>,
    /// Whether it runs in a worker of its own, which goes with it.
    pub alone: bool,
}

impl<'a> Spec<'a> {
    /// A test's run of a program given `args` and `stdin`, reaching what `policy` allows, in a
    /// fresh folder with nothing in it.
    #[must_use]
    pub fn new(args: &'a [String], stdin: &'a [u8], policy: Policy) -> Self {
        static NO_FILES: BTreeMap<String, String> = BTreeMap::new();
        static NO_FOLDERS: BTreeSet<String> = BTreeSet::new();
        Self {
            args,
            stdin,
            stdin_pieces: &[],
            files: &NO_FILES,
            folders: &NO_FOLDERS,
            env: &NO_FILES,
            folder: None,
            supply: None,
            connections: &[],
            listen: None,
            reversed: false,
            stdout_closed: false,
            policy,
            setup: Setup::Test,
            max_written: Some(crate::wasm::host::limits::TEST_WRITTEN),
            alone: false,
        }
    }
}

/// A worker process and its two pipes.
struct Worker {
    /// The process, killed when this is dropped.
    child: Child,
    /// Where the server's requests go.
    stdin: ChildStdin,
    /// Where its events come from.
    stdout: ChildStdout,
}

/// What every handle on a pool shares.
struct Inner {
    /// This binary, which workers are started from.
    launcher: PathBuf,
    /// Where compiled modules are kept, for the workers.
    cache: Option<PathBuf>,
    /// The workers waiting for a job.
    idle: Mutex<Vec<Worker>>,
    /// The next job's number.
    next: AtomicU64,
}

/// A pool of workers, cheap to clone: every clone is the same pool.
#[derive(Clone)]
pub struct Pool {
    /// What every clone shares.
    inner: Arc<Inner>,
}

/// The milliseconds of `duration`.
fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// How a job went whose worker stopped before it said, for `why`.
fn lost(id: u64, ending: Ending, why: String) -> Done {
    Done {
        ending,
        ..runner::unstarted(id, why, Vec::new())
    }
}

/// Listens to a worker's events about job `id`: whether its program waits, the port it serves
/// on, and how it went, which comes with the worker's output back for its next job.
async fn listen_to(
    mut stdout: ChildStdout,
    id: u64,
    waiting: Arc<AtomicBool>,
    mut port: Option<oneshot::Sender<u16>>,
    done: oneshot::Sender<io::Result<(Done, ChildStdout)>>,
) {
    loop {
        match job::receive::<Event>(&mut stdout).await {
            Ok(Some(Event::Listening {
                id: of,
                port: listening,
            })) if of == id => {
                if let Some(port) = port.take() {
                    let _ = port.send(listening);
                }
            }
            Ok(Some(Event::Waiting {
                id: of,
                waiting: now,
            })) if of == id => {
                waiting.store(now, Ordering::SeqCst);
            }
            Ok(Some(Event::Done(finished))) if finished.id == id => {
                let _ = done.send(Ok((*finished, stdout)));
                return;
            }
            // What a job before this one said too late.
            Ok(Some(_)) => {}
            Ok(None) => {
                let _ = done.send(Err(io::Error::other("the worker ended")));
                return;
            }
            Err(error) => {
                let _ = done.send(Err(error));
                return;
            }
        }
    }
}

impl Pool {
    /// A pool that starts its workers from `launcher`, this binary, keeping compiled modules in
    /// `cache` when given.
    #[must_use]
    pub fn new(launcher: PathBuf, cache: Option<PathBuf>) -> Self {
        Self {
            inner: Arc::new(Inner {
                launcher,
                cache,
                idle: Mutex::new(Vec::new()),
                next: AtomicU64::new(1),
            }),
        }
    }

    /// A worker waiting for a job, if one is.
    fn take(&self) -> Option<Worker> {
        self.inner.idle.lock().ok()?.pop()
    }

    /// Keeps `worker` for the next job.
    fn give_back(&self, worker: Worker) {
        if let Ok(mut idle) = self.inner.idle.lock() {
            idle.push(worker);
        }
    }

    /// A fresh worker.
    fn spawn(&self) -> io::Result<Worker> {
        let cache = self.inner.cache.as_ref().map_or_else(
            || OsString::from("none"),
            |cache| cache.as_os_str().to_owned(),
        );
        let mut child = Command::new(&self.inner.launcher)
            .arg(WORKER_FLAG)
            .arg(cache)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("the worker has no input"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("the worker has no output"))?;
        Ok(Worker {
            child,
            stdin,
            stdout,
        })
    }

    /// Starts `program` as `spec` says, within `limits`, and returns once it is running, with
    /// its port when it serves and the ends of its connections to the test's own servers.
    ///
    /// # Errors
    ///
    /// Fails when its arguments or environment can't reach a program alike on every system, or
    /// no worker can be started.
    pub async fn start(
        &self,
        program: &Program,
        spec: Spec<'_>,
        limits: Limits,
    ) -> io::Result<Running> {
        crate::runner::portable_run(spec.args, spec.env).map_err(io::Error::other)?;
        let id = self.inner.next.fetch_add(1, Ordering::SeqCst);
        // The working folder: the run's own, or a fresh one holding the test's files.
        let (scratch, work, folder) = match spec.folder {
            Some(folder) => (None, None, folder.to_path_buf()),
            None => {
                let scratch = tempfile::tempdir()?;
                let work = scratch.path().join("work");
                std::fs::create_dir(&work)?;
                runner::prepare(&work, spec.files, spec.folders)?;
                (Some(scratch), Some(work.clone()), work)
            }
        };
        // The test's own servers listen here, and the worker connects to them.
        let mut made = Vec::new();
        let mut listeners = Vec::new();
        for (number, reach) in spec.connections.iter().enumerate() {
            let resolved = crate::connect::resolved(reach, spec.args);
            let place = resolved.as_ref().unwrap_or(reach).describe();
            let connection = match resolved {
                Ok(Reach::Peer) => {
                    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
                    let port = listener.local_addr()?.port();
                    listeners.push((number, listener));
                    Connection::Test(port)
                }
                Ok(Reach::ThisComputer(port)) => Connection::Place {
                    host: None,
                    port,
                    tls: false,
                },
                Ok(Reach::Host(host, port)) => Connection::Place {
                    host: Some(host),
                    port,
                    tls: false,
                },
                Ok(Reach::Tls(host, port)) => Connection::Place {
                    host: Some(host),
                    port,
                    tls: true,
                },
                Ok(Reach::Argument { .. } | Reach::Nowhere) => {
                    Connection::Ended("nothing listens there".to_owned())
                }
                Err(problem) => Connection::Ended(problem),
            };
            made.push(Made { place, connection });
        }
        let job = Job {
            id,
            module: program.file().to_path_buf(),
            hash: program.hash().to_owned(),
            setup: spec.setup,
            policy: spec.policy,
            args: spec.args.to_vec(),
            env: spec.env.clone(),
            stdin: job::hex(spec.stdin),
            stdin_pieces: spec.stdin_pieces.to_vec(),
            stdout_closed: spec.stdout_closed,
            folder: spec.policy.files.then_some(folder),
            connections: made,
            listen: spec.listen,
            random: spec
                .supply
                .and_then(|supply| supply.random.as_deref())
                .map(job::hex),
            clock: spec
                .supply
                .map(|supply| {
                    supply
                        .clock
                        .iter()
                        .map(|time| (time.seconds, time.nanoseconds))
                        .collect()
                })
                .unwrap_or_default(),
            reversed: spec.reversed,
            instant_sleeps: spec.setup == Setup::Test,
            limits: job::Limits {
                timeout_ms: Some(millis(limits.timeout)),
                max_output: limits.max_output,
                max_file: limits.max_file_size,
                max_written: spec.max_written,
            },
        };
        let request = Request::Run(Box::new(job));
        let idle = if spec.alone { None } else { self.take() };
        let mut worker = match idle {
            Some(worker) => worker,
            None => self.spawn()?,
        };
        if job::send(&mut worker.stdin, &request).await.is_err() {
            // An idle worker that has gone: a fresh one takes the job.
            worker = self.spawn()?;
            job::send(&mut worker.stdin, &request).await?;
        }
        let Worker {
            child,
            stdin,
            stdout,
        } = worker;
        let waiting = Arc::new(AtomicBool::new(false));
        let (port_sender, port_receiver) = oneshot::channel();
        let (done_sender, done_receiver) = oneshot::channel();
        tokio::spawn(listen_to(
            stdout,
            id,
            Arc::clone(&waiting),
            spec.listen.map(|_| port_sender),
            done_sender,
        ));
        let mut peers = Vec::new();
        for (number, listener) in listeners {
            if let Ok(Ok((stream, _))) =
                tokio::time::timeout(CONNECT_TIMEOUT, listener.accept()).await
            {
                peers.push((number, stream));
            }
        }
        let port = if spec.listen.is_some() {
            tokio::time::timeout(CONNECT_TIMEOUT + GRACE, port_receiver)
                .await
                .ok()
                .and_then(Result::ok)
        } else {
            None
        };
        Ok(Running {
            pool: self.clone(),
            stdin: Some(stdin),
            child: Some(child),
            id,
            port,
            peers,
            waiting,
            done: done_receiver,
            timeout: limits.timeout + GRACE,
            work,
            _scratch: scratch,
            lines: program.lines(),
            alone: spec.alone,
        })
    }

    /// Runs `program` as `spec` says, within `limits`, until it ends.
    ///
    /// # Errors
    ///
    /// As [`Self::start`].
    pub async fn run(&self, program: &Program, spec: Spec<'_>, limits: Limits) -> io::Result<Run> {
        self.start(program, spec, limits).await?.wait().await
    }
}

/// main.wat's lines by operator offset, and where its functions' bodies are.
pub type Lines = Arc<(BTreeMap<u64, usize>, Vec<(u64, u64)>)>;

/// A WebAssembly program that's running in a worker.
pub struct Running {
    /// The pool its worker goes back to.
    pool: Pool,
    /// Where requests go to its worker.
    stdin: Option<ChildStdin>,
    /// Its worker.
    child: Option<Child>,
    /// Its job's number.
    id: u64,
    /// The port it serves on, when it serves.
    port: Option<u16>,
    /// lengua's ends of its connections to the test's own servers, by the connection's number.
    peers: Vec<(usize, TcpStream)>,
    /// Whether it waits for a connection now.
    waiting: Arc<AtomicBool>,
    /// How it went, with its worker's output back.
    done: oneshot::Receiver<io::Result<(Done, ChildStdout)>>,
    /// How long lengua waits for that.
    timeout: Duration,
    /// Its fresh working folder, to read back, when it isn't one of the user's.
    work: Option<PathBuf>,
    /// The folder that holds it.
    _scratch: Option<tempfile::TempDir>,
    /// main.wat's lines, for a trap's.
    lines: Lines,
    /// Whether its worker is its own.
    alone: bool,
}

impl Running {
    /// The loopback port it serves on, when it serves.
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        self.port
    }

    /// lengua's ends of the connections that go to the test's own servers, each with its
    /// connection's number in the idea, from 0.
    pub fn take_peers(&mut self) -> Vec<(usize, TcpStream)> {
        std::mem::take(&mut self.peers)
    }

    /// Whether it waits for a connection now.
    #[must_use]
    pub fn waiting(&self) -> bool {
        self.waiting.load(Ordering::SeqCst)
    }

    /// Waits for it to end.
    ///
    /// # Errors
    ///
    /// Never yet: a worker that fails gives a run that says so.
    pub async fn wait(self) -> io::Result<Run> {
        self.finish(false).await
    }

    /// Stops it now, as lengua does a server that's still serving.
    ///
    /// # Errors
    ///
    /// Never yet: a worker that fails gives a run that says so.
    pub async fn stop(self) -> io::Result<Run> {
        self.finish(true).await
    }

    async fn finish(mut self, stop: bool) -> io::Result<Run> {
        if stop && let Some(stdin) = self.stdin.as_mut() {
            let _ = job::send(stdin, &Request::Stop(self.id)).await;
        }
        let waited = tokio::time::timeout(self.timeout, &mut self.done).await;
        let (done, stdout) = match waited {
            Ok(Ok(Ok((done, stdout)))) => (done, Some(stdout)),
            Ok(Ok(Err(error))) => (
                lost(
                    self.id,
                    Ending::Trapped(Trap::Other, None),
                    format!(
                        "lengua's WebAssembly worker stopped while it ran the program: {error}"
                    ),
                ),
                None,
            ),
            Ok(Err(_)) => (
                lost(
                    self.id,
                    Ending::Trapped(Trap::Other, None),
                    "lengua's WebAssembly worker stopped while it ran the program".to_owned(),
                ),
                None,
            ),
            Err(_) => (
                lost(
                    self.id,
                    Ending::TimedOut,
                    "lengua's WebAssembly worker didn't answer in time".to_owned(),
                ),
                None,
            ),
        };
        // A worker that finished goes back to the pool; one that didn't goes, with its program.
        match (stdout, self.stdin.take(), self.child.take()) {
            (Some(stdout), Some(stdin), Some(child)) if !self.alone => {
                self.pool.give_back(Worker {
                    child,
                    stdin,
                    stdout,
                });
            }
            (_, _, Some(mut child)) => {
                let _ = child.kill().await;
            }
            _ => {}
        }
        let left = match &self.work {
            Some(work) => runner::left_in(work),
            None => (Ok(BTreeMap::new()), BTreeSet::new()),
        };
        let lines = Arc::clone(&self.lines);
        Ok(runner::run_of(done, left, |offset| {
            crate::wasm::text::line_at(&lines.0, &lines.1, offset)
        }))
    }
}
