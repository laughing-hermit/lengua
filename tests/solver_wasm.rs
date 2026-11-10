//! A WebAssembly solver's host, `lengua-mcp --solver-wasm`, the same on every system: the
//! stand-in solver of tests/fixtures/solver-wasm, a module with exceptions written by hand, runs
//! in the solver's engine. A throw it catches runs on, and one that nothing in it catches ends the
//! solver's process with an error. It gets its standard input, output and error, its arguments
//! and the clock, and nothing else. The prover asks it questions through its host as it asks a
//! native solver, and a worker, which runs programs, runs no solver.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use lengua::prove::smt::{self, Answer, Kind, Solver};
use lengua::prove::term::Pool;
use lengua::wasm::engine::Setup;
use lengua::wasm::job::{self, Ending, Event, Job, Request};
use lengua::wasm::wasi::errno;
use lengua::wasm::{Policy, Program, solver};

/// The stand-in solver, as WebAssembly text.
const STAND_IN: &str = include_str!("fixtures/solver-wasm/stand-in.wat");

/// lengua-mcp, which hosts the solver.
fn host() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lengua-mcp"))
}

/// The stand-in, as wast encodes it, in a file of `folder`.
fn stand_in(folder: &Path) -> PathBuf {
    let buffer = wast::parser::ParseBuffer::new(STAND_IN).expect("WebAssembly text");
    let wast::Wat::Module(mut module) =
        wast::parser::parse::<wast::Wat>(&buffer).expect("a module")
    else {
        panic!("the stand-in isn't a module");
    };
    let path = folder.join("stand-in.wasm");
    std::fs::write(&path, module.encode().expect("the module's bytes")).expect("the module's file");
    path
}

/// The host's run of the stand-in, with `args` after `--` and `input` on its standard input,
/// compiled modules kept in `folder`, and an environment variable of this process's that the
/// solver must not see.
fn hosted(folder: &Path, args: &[&str], input: &[u8]) -> Output {
    let module = stand_in(folder);
    let mut child = Command::new(host())
        .arg(solver::FLAG)
        .arg(&module)
        .arg("--cache")
        .arg(folder.join("cache"))
        .arg("--")
        .args(args)
        .env("LENGUA_SECRET", "kept from the solver")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the host starts");
    if let Some(mut stdin) = child.stdin.take() {
        // A solver that reads none of it may have ended before it was written.
        let _ = stdin.write_all(input);
    }
    child.wait_with_output().expect("the host ends")
}

/// How a run ended and what it printed, for a message.
fn said(output: &Output) -> String {
    format!(
        "{}, printing {:?} and on standard error {:?}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A throw the solver catches runs on to its exit; one that nothing in it catches ends its
/// process with the code of a trap, saying so on standard error.
#[test]
fn a_caught_throw_runs_on_and_an_uncaught_one_ends_the_solver_with_an_error() {
    let folder = tempfile::tempdir().expect("a folder");
    let caught = hosted(folder.path(), &["catch"], b"");
    assert_eq!(caught.status.code(), Some(0), "{}", said(&caught));
    assert_eq!(
        String::from_utf8_lossy(&caught.stdout),
        "caught 42\n",
        "{}",
        said(&caught)
    );
    assert!(caught.stderr.is_empty(), "{}", said(&caught));
    let thrown = hosted(folder.path(), &["throw"], b"");
    assert_eq!(
        thrown.status.code(),
        Some(lengua::measure::TRAPPED),
        "{}",
        said(&thrown)
    );
    assert_eq!(
        String::from_utf8_lossy(&thrown.stdout),
        "throwing 7\n",
        "{}",
        said(&thrown)
    );
    assert!(
        String::from_utf8_lossy(&thrown.stderr)
            .contains("the solver threw an exception that nothing in it caught"),
        "{}",
        said(&thrown)
    );
}

/// The host gives a solver its standard input, output and error, its arguments and the clock,
/// and nothing else: no environment, though this process gives the host one; zeros for random
/// bytes; and no file, no folder and no connection.
#[test]
fn the_host_gives_a_solver_its_streams_arguments_and_clock_and_nothing_else() {
    let folder = tempfile::tempdir().expect("a folder");
    let ran = hosted(folder.path(), &["grants", "second"], b"abc");
    assert_eq!(ran.status.code(), Some(0), "{}", said(&ran));
    let (refused, unopened) = (errno::NOTCAPABLE, errno::BADF);
    assert_eq!(
        String::from_utf8_lossy(&ran.stdout),
        format!(
            "arguments 3\ninput 3\nclock 0 1\nenvironment 0\nrandom 0 0\nfiles {refused}\nfolder \
             {unopened}\nnetwork {refused}\n"
        ),
        "{}",
        said(&ran)
    );
    assert_eq!(
        String::from_utf8_lossy(&ran.stderr),
        "to standard error\n",
        "{}",
        said(&ran)
    );
}

/// The prover asks a WebAssembly solver its questions through its host as it asks a native one:
/// the stand-in, which says `sat` to each, answers a question the terms don't settle, before a
/// reset and after it.
#[test]
fn the_prover_asks_a_webassembly_solver_through_its_host() {
    let folder = tempfile::tempdir().expect("a folder");
    let program = smt::Program::Wasm {
        host: host(),
        module: stand_in(folder.path()),
        cache: Some(folder.path().join("cache")),
    };
    let mut solver =
        Solver::with_programs(&[(Kind::Z3, program)], 5000, false).expect("the stand-in starts");
    let mut pool = Pool::default();
    let x = pool.var(8, "x");
    let one = pool.bv(8, 1);
    let next = pool.add(x, one);
    let zero = pool.bv(8, 0);
    let wraps = pool.eq(next, zero);
    let (answer, _) = solver
        .check(&pool, &[wraps], &[])
        .expect("the stand-in answers");
    assert_eq!(answer, Answer::Sat);
    solver.reset().expect("the stand-in starts afresh");
    let (answer, _) = solver
        .check(&pool, &[wraps], &[])
        .expect("the stand-in answers again");
    assert_eq!(answer, Answer::Sat);
    assert_eq!(solver.questions, 2);
}

/// A worker runs programs alone: a job in the solver's configuration, which turns exceptions on,
/// never starts there.
#[test]
fn a_worker_runs_no_solver() {
    let program =
        Program::of_text("(module (memory (export \"memory\") 1) (func (export \"_start\")))")
            .expect("a program");
    let mut worker = Command::new(host())
        .arg(lengua::wasm::WORKER_FLAG)
        .arg("none")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("a worker");
    let mut requests = worker.stdin.take().expect("its input");
    let mut events = worker.stdout.take().expect("its output");
    let job = Job {
        id: 1,
        module: program.file().to_path_buf(),
        hash: program.hash().to_owned(),
        setup: Setup::Solver,
        policy: Policy::default(),
        args: Vec::new(),
        env: BTreeMap::new(),
        stdin: String::new(),
        stdin_pieces: Vec::new(),
        stdout_closed: false,
        folder: None,
        connections: Vec::new(),
        listen: None,
        random: None,
        clock: Vec::new(),
        reversed: false,
        instant_sleeps: true,
        limits: job::Limits {
            timeout_ms: Some(10_000),
            max_output: 1000,
            max_file: None,
            max_written: None,
        },
    };
    job::write_message(&mut requests, &Request::Run(Box::new(job))).expect("the job");
    let event: Option<Event> = job::read_message(&mut events).expect("the worker's answer");
    drop(requests);
    let _ = worker.wait();
    let done = match event {
        Some(Event::Done(done)) => done,
        other => panic!("the worker said {other:?}"),
    };
    assert_eq!(done.ending, Ending::Unstarted);
    assert!(
        done.problem
            .as_deref()
            .is_some_and(|problem| problem.contains("is no program's")),
        "{:?}",
        done.problem
    );
}
