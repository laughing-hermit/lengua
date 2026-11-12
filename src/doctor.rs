//! `lengua-mcp doctor`: whether this computer has what lengua needs, one line for each thing it
//! checks and, under a line that found a problem, one sentence saying what to do, then the
//! verdict. It looks at the server's version and own file; the engine, by compiling and running
//! a module that uses the WebAssembly features lengua turns on; the CPU, as the `machine` line
//! names it; z3 and cvc5, where the prover finds them ([`smt::find`]: the release archive's
//! solvers folder beside the server first, then PATH), and their versions against the ones
//! lengua pins; whether the data folder can be written; and how fine the
//! clocks are. `lengua-mcp doctor` exits 0 only when nothing is wrong.

use std::io::Read as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use crate::data_dir::{DataDir, NOT_FOUND};
use crate::platform::Machine;
use crate::prove::smt::{self, Found, Kind};
use crate::wasm::engine::{self, Setup};
use crate::wasm::host::clock::RESOLUTION;

/// doctor's verdict when nothing is wrong.
pub const NOTHING_WRONG: &str = "nothing is wrong";

/// One thing doctor checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// What it checked and what it found, as `name: finding`.
    pub line: String,
    /// The problem it found, in one sentence that says what to do; `None` when there is none.
    pub problem: Option<String>,
}

impl Finding {
    /// A finding with no problem.
    const fn fine(line: String) -> Self {
        Self {
            line,
            problem: None,
        }
    }

    /// A finding with `problem`, which says what to do.
    const fn wrong(line: String, problem: String) -> Self {
        Self {
            line,
            problem: Some(problem),
        }
    }
}

/// What doctor found, in the order it checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Each thing it checked.
    pub findings: Vec<Finding>,
}

impl Report {
    /// How many problems it found.
    #[must_use]
    pub fn problems(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.problem.is_some())
            .count()
    }

    /// What doctor prints: each finding's line, with its problem's sentence under it indented by
    /// two spaces, and the verdict last: [`NOTHING_WRONG`], or how many problems it found.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for finding in &self.findings {
            lines.push(finding.line.clone());
            if let Some(problem) = &finding.problem {
                lines.push(format!("  {problem}"));
            }
        }
        lines.push(match self.problems() {
            0 => NOTHING_WRONG.to_owned(),
            1 => "1 problem".to_owned(),
            many => format!("{many} problems"),
        });
        lines
    }
}

/// Checks this computer for lengua, with `data_dir` lengua's data folder: `None` when none can
/// be found.
#[must_use]
pub fn doctor(data_dir: Option<&DataDir>) -> Report {
    Report {
        findings: vec![
            server(),
            runner(),
            engine_runs(),
            cpu(),
            solver(Kind::Z3),
            solver(Kind::Cvc5),
            data_folder(data_dir),
            clocks(),
        ],
    }
}

/// The server: its version, and the file it runs from, which it starts its workers from.
fn server() -> Finding {
    let version = env!("CARGO_PKG_VERSION");
    match std::env::current_exe() {
        Ok(path) => Finding::fine(format!("server: lengua-mcp {version}, {}", path.display())),
        Err(error) => Finding::wrong(
            format!("server: lengua-mcp {version}, which can't find its own file: {error}"),
            "lengua starts the workers that run programs from its own file: start lengua-mcp by \
             its full path."
                .to_owned(),
        ),
    }
}

/// lengua-run, the small program beside the server that runs a checked program from a shell:
/// whether it is there, and of this server's version. Where it isn't there, as in a build that
/// didn't make it, `lengua-mcp run` runs programs, and nothing is wrong.
fn runner() -> Finding {
    let version = env!("CARGO_PKG_VERSION");
    let name = crate::measure::RUNNER;
    let beside = std::env::current_exe()
        .map(|server| server.with_file_name(format!("{name}{}", std::env::consts::EXE_SUFFIX)));
    let Ok(path) = beside else {
        return Finding::fine(
            "runner: lengua-mcp can't find its own file, so `lengua-mcp run` runs checked programs"
                .to_owned(),
        );
    };
    if !path.is_file() {
        return Finding::fine(format!(
            "runner: no {name} beside lengua-mcp, so `lengua-mcp run` runs checked programs"
        ));
    }
    let said = Command::new(&path)
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned());
    let expected = format!("{name} {version}");
    match said {
        Ok(said) if said == expected => Finding::fine(format!(
            "runner: {name} {version}, {}, which runs checked programs with no compiler in it",
            path.display()
        )),
        Ok(said) if said.starts_with(&format!("{expected},")) => {
            Finding::fine(format!("runner: {said}, {}", path.display()))
        }
        Ok(said) => Finding::wrong(
            format!("runner: {} says {said:?}, not {expected}", path.display()),
            format!(
                "{name} and lengua-mcp come from one release archive, and a {name} of another \
                 version leaves every run to lengua-mcp or loads nothing: install lengua again."
            ),
        ),
        Err(error) => Finding::wrong(
            format!("runner: {} doesn't start: {error}", path.display()),
            format!(
                "`lengua-mcp run` still runs checked programs; install lengua again for {name}."
            ),
        ),
    }
}

/// A module that uses the WebAssembly features lengua turns on: sixteen bytes of 7 filled in
/// (bulk memory), read as four lanes with 1 added to each (SIMD), lane 0's low byte widened with
/// its sign (sign extension), 2.5 made a whole number (saturating conversion), and two values
/// from one block (multi-value): 8 + 2 + (30 - 2), [`PROBED`].
const PROBE: &str = "(module
  (memory 1)
  (func (export \"probe\") (result i32)
    (memory.fill (i32.const 0) (i32.const 7) (i32.const 16))
    (i32.extend8_s
      (i32x4.extract_lane 0 (i32x4.add (v128.load (i32.const 0)) (v128.const i32x4 1 1 1 1))))
    (i32.trunc_sat_f32_s (f32.const 2.5))
    (i32.add)
    (block (result i32 i32) (i32.const 30) (i32.const 2))
    (i32.sub)
    (i32.add)))";

/// What [`PROBE`] computes.
const PROBED: i32 = 38;

/// The engine: Wasmtime with Cranelift, compiling [`PROBE`] on this computer as check compiles a
/// module ahead of its runs, and the engine that only runs loading what it compiled and running
/// it, as a run from a shell does.
fn engine_runs() -> Finding {
    let engine = format!("Wasmtime {} with Cranelift", engine::ENGINE_VERSION);
    match probe() {
        Ok(()) => Finding::fine(format!(
            "engine: {engine} compiled and ran a module here, WebAssembly 1.0 with {} on",
            listed(&engine::OPTIONAL_FEATURES)
        )),
        Err(error) => Finding::wrong(
            format!("engine: {engine} can't run WebAssembly here: {error}"),
            "lengua checks and runs no program until it can: report it to lengua's project, with \
             what doctor printed."
                .to_owned(),
        ),
    }
}

/// Compiles [`PROBE`], seals it with a key of its own that nothing else has, loads it into the
/// engine that only runs, and runs it there: it must compute [`PROBED`].
fn probe() -> Result<(), String> {
    let built = crate::wasm::text::build(PROBE).map_err(|problem| problem.to_string())?;
    let compiler = engine::engine(Setup::Shell)?;
    let key = crate::lock::Key::from_bytes([0; 32]);
    let hash = crate::hash::Sha256Hex::of(&built.wasm);
    let sealed = key
        .compile_sealed(&compiler, &hash, &built.wasm)
        .map_err(|error| format!("it compiles no module: {error}"))?;
    let engine = engine::runner()?;
    let module = key
        .load_sealed(&engine, &hash, &sealed)
        .map_err(|error| format!("it loads no compiled module: {error}"))?;
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[])
        .map_err(|error| format!("it starts no module: {error}"))?;
    let function = instance
        .get_typed_func::<(), i32>(&mut store, "probe")
        .map_err(|error| error.to_string())?;
    let computed = function
        .call(&mut store, ())
        .map_err(|error| format!("the module didn't run: {error}"))?;
    if computed == PROBED {
        Ok(())
    } else {
        Err(format!("the module computed {computed}, not {PROBED}"))
    }
}

/// The CPU, as the `machine` line names it: the system and architecture, its model where the
/// system says it, its cores, and how wide its vectors go.
fn cpu() -> Finding {
    let machine = Machine::detect();
    let model = machine
        .cpu_model
        .clone()
        .unwrap_or_else(|| "a CPU whose model the system doesn't say".to_owned());
    Finding::fine(format!(
        "cpu: {}-{}, {model}, {}, vectors up to {}",
        machine.os,
        machine.arch,
        counted(machine.cores, "core"),
        crate::server::simd_level(&machine)
    ))
}

/// How long a solver may take to print its version.
const VERSION_WAIT: Duration = Duration::from_secs(10);

/// What doctor adds on macOS to what to do about a solver that didn't run: macOS won't start an
/// unsigned program a browser downloaded until the mark the download got is cleared.
#[cfg(target_os = "macos")]
const MARKED: &str = "; if macOS refused to start it, as it does an unsigned program a browser \
                      downloaded, run xattr -dr com.apple.quarantine on the folder lengua is in";

/// What doctor adds elsewhere: nothing.
#[cfg(not(target_os = "macos"))]
const MARKED: &str = "";

/// z3 or cvc5: where the prover finds it, and its version against the one lengua pins.
fn solver(kind: Kind) -> Finding {
    let (name, pinned) = (kind.name(), kind.pinned());
    let archive = format!(
        "install lengua from the release archive for this system and CPU, which holds {name} \
         {pinned} in its solvers folder"
    );
    let Some(found) = smt::find(kind) else {
        let beside = smt::solvers_folder().map_or_else(
            || smt::SOLVERS_FOLDER.to_owned(),
            |folder| folder.display().to_string(),
        );
        return Finding::wrong(
            format!("{name}: neither in {beside} nor on PATH"),
            format!(
                "Proofs need {name} {pinned}: {archive}, or put {name} {pinned} on PATH; programs \
                 are still checked and tested without it."
            ),
        );
    };
    let place = match &found {
        Found::Beside(path) => format!("beside the server, {}", path.display()),
        Found::OnPath(path) => format!("on PATH, {}", path.display()),
    };
    let printed = match version_printed(found.path()) {
        Ok(printed) => printed,
        Err(error) => {
            return Finding::wrong(
                format!("{name}: {place}, which didn't run: {error}"),
                format!("Proofs need {name} to run: {archive}{MARKED}."),
            );
        }
    };
    match smt::version_of(&printed) {
        Some(version) if version == pinned => Finding::fine(format!(
            "{name}: {version}, the version lengua pins, {place}"
        )),
        Some(version) => Finding::wrong(
            format!("{name}: {version}, {place}, not the {pinned} lengua pins"),
            format!(
                "lengua's proofs are tested with {name} {pinned}: {archive}, or put {name} \
                 {pinned} first on PATH."
            ),
        ),
        None => Finding::wrong(
            format!("{name}: {place}, which printed no version"),
            format!(
                "lengua's proofs are tested with {name} {pinned}: {archive}, or put {name} \
                 {pinned} first on PATH."
            ),
        ),
    }
}

/// What `program --version` prints, within [`VERSION_WAIT`].
fn version_printed(program: &Path) -> Result<String, String> {
    let mut child = Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| error.to_string())?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if started.elapsed() > VERSION_WAIT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "it printed no version in {} seconds",
                VERSION_WAIT.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut printed = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_end(&mut printed);
    }
    if status.success() {
        Ok(String::from_utf8_lossy(&printed).into_owned())
    } else {
        Err(format!("it ended with {status}"))
    }
}

/// lengua's data folder, which holds its signing key and checked copies: whether it can be
/// written.
fn data_folder(data_dir: Option<&DataDir>) -> Finding {
    let Some(data_dir) = data_dir else {
        return Finding::wrong(
            "data folder: none found".to_owned(),
            format!("lengua {NOT_FOUND}."),
        );
    };
    let folder = data_dir.path();
    match writable(folder) {
        Ok(()) => Finding::fine(format!("data folder: {}, writable", folder.display())),
        Err(error) => Finding::wrong(
            format!(
                "data folder: {}, which lengua can't write: {error}",
                folder.display()
            ),
            "lengua keeps its signing key and checked copies there: give your user write access \
             to it, or start lengua-mcp with --data-dir and a folder it can write."
                .to_owned(),
        ),
    }
}

/// Whether lengua can write in `folder`: it makes the folder when it's missing, then writes a
/// file there and takes it away.
fn writable(folder: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(folder)?;
    let mut file = tempfile::NamedTempFile::new_in(folder)?;
    std::io::Write::write_all(&mut file, b"lengua")?;
    file.close()
}

/// How long doctor watches each clock.
const CLOCK_WATCH: Duration = Duration::from_millis(100);

/// The most steps of a clock doctor waits for.
const CLOCK_STEPS: usize = 1000;

/// The clocks programs read: the one that only moves forward and the time of day, each of which
/// lengua gives programs to the microsecond ([`RESOLUTION`]). A clock that moves in coarser
/// steps makes a program's times, and how long lengua lets it run, only that fine.
fn clocks() -> Finding {
    let started = Instant::now();
    let forward = finest_step(|| started.elapsed().as_nanos());
    let of_day = finest_step(|| {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos())
    });
    let resolution = u128::from(RESOLUTION);
    match (forward, of_day) {
        (Some(forward), Some(of_day)) if forward <= resolution && of_day <= resolution => {
            Finding::fine(format!(
                "clock: steps of {} or less, and {} or less for the time of day; programs read \
                 both to the microsecond",
                nanoseconds(forward),
                nanoseconds(of_day)
            ))
        }
        (Some(forward), Some(of_day)) => Finding::wrong(
            format!(
                "clock: steps of {}, and {} for the time of day, coarser than the microsecond \
                 programs read them to",
                nanoseconds(forward),
                nanoseconds(of_day)
            ),
            "A program's times, and how long lengua lets it run, are only as fine as this \
             clock: where a program needs finer times, run lengua on a computer or virtual \
             machine whose clock is finer."
                .to_owned(),
        ),
        _ => Finding::wrong(
            format!(
                "clock: a clock didn't move in {} ms",
                CLOCK_WATCH.as_millis()
            ),
            "lengua times every run with it: set the system's clock going, or run lengua on a \
             computer whose clock moves."
                .to_owned(),
        ),
    }
}

/// The smallest step forward `read`, a clock in nanoseconds, moved in, over [`CLOCK_STEPS`]
/// steps or [`CLOCK_WATCH`]: at most the clock's resolution, as each read takes time too.
/// `None` when it didn't move forward.
fn finest_step(read: impl Fn() -> u128) -> Option<u128> {
    let deadline = Instant::now() + CLOCK_WATCH;
    let mut last = read();
    let mut finest: Option<u128> = None;
    let mut steps = 0;
    let mut reads: u32 = 0;
    while steps < CLOCK_STEPS {
        reads = reads.wrapping_add(1);
        // Looking at the time takes time too, so not on every read.
        if reads.is_multiple_of(256) && Instant::now() >= deadline {
            break;
        }
        let now = read();
        if now > last {
            let step = now - last;
            finest = Some(finest.map_or(step, |finest| finest.min(step)));
            steps += 1;
        }
        last = now;
    }
    finest
}

/// `ns` nanoseconds as doctor says them: in nanoseconds below 100 microseconds, else in
/// milliseconds to a tenth.
fn nanoseconds(ns: u128) -> String {
    if ns < 100_000 {
        format!("{ns} ns")
    } else {
        format!("{}.{} ms", ns / 1_000_000, ns % 1_000_000 / 100_000)
    }
}

/// `count` of `thing`, as `1 core` or `4 cores`.
fn counted(count: usize, thing: &str) -> String {
    if count == 1 {
        format!("1 {thing}")
    } else {
        format!("{count} {thing}s")
    }
}

/// `items` as a list: `a`, `a and b`, `a, b and c`.
fn listed(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A problem's sentence goes under its line, indented, and the verdict counts the problems.
    #[test]
    fn a_problem_goes_under_its_line_and_the_verdict_counts_them() {
        let fine = Finding::fine("clock: steps of 41 ns or less".to_owned());
        let wrong = Finding::wrong("z3: neither here nor there".to_owned(), "Do so.".to_owned());
        let healthy = Report {
            findings: vec![fine.clone()],
        };
        assert_eq!(healthy.problems(), 0);
        assert_eq!(
            healthy.lines(),
            ["clock: steps of 41 ns or less", NOTHING_WRONG]
        );
        let ill = Report {
            findings: vec![wrong.clone(), fine, wrong],
        };
        assert_eq!(ill.problems(), 2);
        assert_eq!(
            ill.lines(),
            [
                "z3: neither here nor there",
                "  Do so.",
                "clock: steps of 41 ns or less",
                "z3: neither here nor there",
                "  Do so.",
                "2 problems",
            ]
        );
    }

    /// The engine compiles and runs the module that uses every feature lengua turns on, here as
    /// on every runner.
    #[test]
    fn the_engine_runs_the_probe() {
        assert_eq!(probe(), Ok(()));
        let finding = engine_runs();
        assert_eq!(finding.problem, None, "{}", finding.line);
        let features = "sign-extension, saturating-float-to-int, multi-value, reference-types, \
                        bulk-memory and simd on";
        assert!(finding.line.ends_with(features), "{}", finding.line);
    }

    /// A clock's finest step is the smallest it moved forward in, and a clock that never moves
    /// has none.
    #[test]
    fn a_clock_s_finest_step_is_its_smallest_move() {
        let ticks = std::cell::Cell::new(0_u128);
        let steady = finest_step(|| {
            ticks.set(ticks.get() + 7);
            ticks.get()
        });
        assert_eq!(steady, Some(7));
        assert_eq!(finest_step(|| 5), None);
        // A reading that goes back, as a time of day set back does, is no step: from 1000 back
        // to 500, then on in steps of 20.
        let reads = std::cell::Cell::new(0_u128);
        let set_back = finest_step(|| {
            let read = reads.get();
            reads.set(read + 1);
            if read == 0 { 1000 } else { 480 + read * 20 }
        });
        assert_eq!(set_back, Some(20));
    }

    /// This computer's clocks move, finely enough to say so in a line.
    #[test]
    fn the_clock_line_names_both_clocks() {
        let finding = clocks();
        assert!(finding.line.starts_with("clock: "), "{}", finding.line);
        assert!(finding.line.contains("time of day"), "{}", finding.line);
    }

    #[test]
    fn durations_counts_and_lists_read_as_doctor_says_them() {
        assert_eq!(nanoseconds(41), "41 ns");
        assert_eq!(nanoseconds(1000), "1000 ns");
        assert_eq!(nanoseconds(15_625_000), "15.6 ms");
        assert_eq!(counted(1, "core"), "1 core");
        assert_eq!(counted(4, "core"), "4 cores");
        assert_eq!(listed(&["a"]), "a");
        assert_eq!(listed(&["a", "b", "c"]), "a, b and c");
    }

    /// A folder lengua can write is made when it's missing and left empty; one under a file
    /// can't be.
    #[test]
    fn a_folder_is_writable_or_says_why_not() {
        let root = tempfile::tempdir().expect("a folder");
        let folder = root.path().join("data").join("lengua");
        assert!(writable(&folder).is_ok());
        assert_eq!(
            std::fs::read_dir(&folder).expect("the folder").count(),
            0,
            "doctor left a file behind"
        );
        let file = root.path().join("a file");
        std::fs::write(&file, "").expect("a file");
        assert!(writable(&file.join("data")).is_err());
        let finding = data_folder(Some(&DataDir::new(file.join("data"))));
        assert!(finding.problem.is_some(), "{}", finding.line);
        let finding = data_folder(None);
        assert_eq!(finding.line, "data folder: none found");
    }
}
