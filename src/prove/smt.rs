//! The SMT solvers the prover asks, in SMT-LIB 2 over pipes. z3 answers every question; when
//! cvc5 is installed too, every condition z3 finds impossible must be impossible for cvc5 as
//! well, so one solver's bug can't make a false proof. Terms are defined once, and each question
//! is asked inside its own `push`/`pop`, so the definitions stay and the assertions go.
//!
//! Each solver is found as [`find`] finds it: in the `solvers` folder beside the folder that
//! holds lengua-mcp, where a release archive keeps the versions lengua pins, and else on PATH.
//! A WebAssembly z3 comes first: the module [`Z3_WASM`] names, as tests and CI choose one, or
//! `z3.wasm` in the solvers folder. The prover starts it in a lengua-mcp of its own
//! (`--solver-wasm`, [`crate::wasm::solver`]) with z3's usual arguments, so that the pipes, the
//! questions, the logs and the watchdog are the same for either. `lengua-mcp doctor` says where
//! it found each, with the same function.

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::io::{self, BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::term::{Node, Pool, Sort, Term};

/// What the solver says about a set of conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Some inputs meet them all.
    Sat,
    /// No input meets them all: proven impossible.
    Unsat,
    /// The solver ran out of time or couldn't decide.
    Unknown,
}

/// The line each solver prints back to mark the end of each batch of commands.
const MARKER: &str = "@@lengua@@";

/// An SMT solver lengua can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// z3, from Microsoft Research.
    Z3,
    /// cvc5, from Stanford and the University of Iowa.
    Cvc5,
}

impl Kind {
    /// Both solvers, z3 first.
    pub const ALL: [Self; 2] = [Self::Z3, Self::Cvc5];

    /// The solver's program name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Z3 => "z3",
            Self::Cvc5 => "cvc5",
        }
    }

    /// The version lengua is tested with: the one xtask/tools.tsv pins for every runner,
    /// and a release archive ships.
    #[must_use]
    pub const fn pinned(self) -> &'static str {
        match self {
            Self::Z3 => "4.16.0",
            Self::Cvc5 => "1.3.4",
        }
    }

    /// The solver's file: its name, with `.exe` on Windows.
    fn file(self) -> String {
        format!("{}{}", self.name(), std::env::consts::EXE_SUFFIX)
    }

    /// The file of its WebAssembly module in the solvers folder, for a solver that has a
    /// WebAssembly build: z3's.
    #[must_use]
    pub const fn module_file(self) -> Option<&'static str> {
        match self {
            Self::Z3 => Some("z3.wasm"),
            Self::Cvc5 => None,
        }
    }

    /// The environment variable that names its WebAssembly module, for a solver that has a
    /// WebAssembly build: z3's [`Z3_WASM`].
    #[must_use]
    pub const fn module_variable(self) -> Option<&'static str> {
        match self {
            Self::Z3 => Some(Z3_WASM),
            Self::Cvc5 => None,
        }
    }
}

/// The folder, beside the one that holds lengua-mcp, where a release archive keeps the solvers:
/// `<plugin>/server/lengua-mcp` and `<plugin>/solvers/z3`.
pub const SOLVERS_FOLDER: &str = "solvers";

/// The environment variable that names a WebAssembly z3, its module's file, for the prover to
/// start in place of any other z3: tests and CI choose one with it.
pub const Z3_WASM: &str = "LENGUA_Z3_WASM";

/// The environment variable that names the lengua-mcp that hosts a WebAssembly solver, for a test
/// whose own program isn't lengua-mcp: unset, the host is this program.
pub const SOLVER_HOST: &str = "LENGUA_SOLVER_HOST";

/// Where [`find`] found a solver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// Its program, in [`solvers_folder`] beside the server, as a release archive keeps it.
    Beside(PathBuf),
    /// Its program, in a folder on PATH.
    OnPath(PathBuf),
    /// Its WebAssembly module, in [`solvers_folder`] beside the server, which a lengua-mcp hosts.
    ModuleBeside(PathBuf),
    /// Its WebAssembly module, which its environment variable names
    /// ([`Kind::module_variable`]).
    ModuleNamed(PathBuf),
}

impl Found {
    /// The solver's program, or its module.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Beside(path)
            | Self::OnPath(path)
            | Self::ModuleBeside(path)
            | Self::ModuleNamed(path) => path,
        }
    }

    /// Whether it is a WebAssembly module, which a lengua-mcp hosts, rather than a program.
    #[must_use]
    pub const fn is_module(&self) -> bool {
        matches!(self, Self::ModuleBeside(_) | Self::ModuleNamed(_))
    }

    /// What the prover starts for it: its program, or for a module, the lengua-mcp that hosts it
    /// ([`host`]), which keeps the module compiled where it keeps it by itself.
    #[must_use]
    pub fn program(&self) -> Program {
        match self {
            Self::Beside(path) | Self::OnPath(path) => Program::Native(path.as_os_str().to_owned()),
            Self::ModuleBeside(module) | Self::ModuleNamed(module) => Program::Wasm {
                host: host(),
                module: module.clone(),
                cache: None,
            },
        }
    }
}

/// What the prover starts for a solver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Program {
    /// A program the system runs: its path, or its bare name, which the system looks for itself.
    Native(OsString),
    /// A WebAssembly module, which a lengua-mcp runs as the solver (`--solver-wasm`).
    Wasm {
        /// The lengua-mcp that hosts it.
        host: PathBuf,
        /// The module's file.
        module: PathBuf,
        /// Where its compiled module is kept, when not where the host keeps it by itself
        /// ([`crate::wasm::solver::cache_folder`]).
        cache: Option<PathBuf>,
    },
}

impl Program {
    /// The command that starts it, before the solver's own arguments: the program, or its host
    /// given the module, the cache's folder when one is named, and `--`.
    #[must_use]
    pub fn command(&self) -> Command {
        match self {
            Self::Native(program) => Command::new(program),
            Self::Wasm {
                host,
                module,
                cache,
            } => {
                let mut command = Command::new(host);
                command.arg(crate::wasm::solver::FLAG).arg(module);
                if let Some(cache) = cache {
                    command.arg("--cache").arg(cache);
                }
                command.arg("--");
                command
            }
        }
    }
}

/// The lengua-mcp that hosts a WebAssembly solver: the one [`SOLVER_HOST`] names, else this
/// program, which is lengua-mcp everywhere but in tests.
#[must_use]
pub fn host() -> PathBuf {
    std::env::var_os(SOLVER_HOST)
        .filter(|named| !named.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
        .unwrap_or_else(|| PathBuf::from("lengua-mcp"))
}

/// The folder where a release archive keeps the solvers, [`SOLVERS_FOLDER`] beside the folder
/// that holds this lengua-mcp, found through a link to lengua-mcp on Unix; `None` when this
/// process can't tell its own file.
#[must_use]
pub fn solvers_folder() -> Option<PathBuf> {
    let program = std::env::current_exe().ok()?;
    #[cfg(unix)]
    let program = std::fs::canonicalize(&program).unwrap_or(program);
    Some(program.parent()?.parent()?.join(SOLVERS_FOLDER))
}

/// Finds `kind`'s solver: the WebAssembly module its environment variable names
/// ([`Kind::module_variable`]) first, then [`solvers_folder`], then PATH's folders. The prover
/// starts the one it finds, and `lengua-mcp doctor` says which that is.
#[must_use]
pub fn find(kind: Kind) -> Option<Found> {
    let value = kind.module_variable().and_then(std::env::var_os);
    named(kind, value.as_deref()).or_else(|| {
        find_in(
            solvers_folder().as_deref(),
            std::env::var_os("PATH").as_deref(),
            kind,
        )
    })
}

/// The WebAssembly module `value`, the value of `kind`'s environment variable, names, when it
/// names one and `kind` has a WebAssembly build: it counts whether or not its file is there, so
/// that a wrong one says so rather than leaving the prover to another solver.
fn named(kind: Kind, value: Option<&OsStr>) -> Option<Found> {
    kind.module_file()
        .and(value)
        .filter(|value| !value.is_empty())
        .map(|value| Found::ModuleNamed(PathBuf::from(value)))
}

/// [`find`] beside the server and on PATH, with `beside` the solvers folder beside the server and
/// `path` PATH's value. A file in the solvers folder counts whatever its permissions, so that one
/// that can't run says so rather than leaving the prover to another, and there a WebAssembly
/// module comes before a program; on PATH, as a shell looks, only a program that can run counts,
/// and only an absolute folder.
fn find_in(beside: Option<&Path>, path: Option<&OsStr>, kind: Kind) -> Option<Found> {
    if let Some(found) = beside
        .zip(kind.module_file())
        .map(|(folder, file)| folder.join(file))
        .filter(|candidate| candidate.is_file())
    {
        return Some(Found::ModuleBeside(found));
    }
    let file = kind.file();
    if let Some(found) = beside
        .map(|folder| folder.join(&file))
        .filter(|candidate| candidate.is_file())
    {
        return Some(Found::Beside(found));
    }
    std::env::split_paths(path?)
        .filter(|folder| folder.is_absolute())
        .map(|folder| folder.join(&file))
        .find(|candidate| runnable(candidate))
        .map(Found::OnPath)
}

/// Whether `path` is a file the system can run: one with an execute permission, on Unix.
#[cfg(unix)]
fn runnable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// Whether `path` is a file the system can run: any file, on Windows, where `.exe` says it.
#[cfg(not(unix))]
fn runnable(path: &Path) -> bool {
    path.is_file()
}

/// What the prover starts for `kind`: what [`find`] finds, or else its bare name, which the
/// system looks for itself and, finding none, makes the solver "not installed".
fn program(kind: Kind) -> Program {
    find(kind).map_or_else(
        || Program::Native(OsString::from(kind.name())),
        |found| found.program(),
    )
}

/// The version a solver's `--version` prints: `4.16.0` from z3's `Z3 version 4.16.0 - 64 bit`,
/// and `1.3.4` from cvc5's `cvc5 1.3.4 [git tag 1.3.4 branch HEAD]` or its older
/// `This is cvc5 version 1.3.4 …`: the first word shaped like a version on the first line that
/// names the solver, never the compiler's version a later line gives.
#[must_use]
pub fn version_of(printed: &str) -> Option<&str> {
    printed.lines().find_map(|line| {
        let lower = line.to_ascii_lowercase();
        if !lower.contains("z3") && !lower.contains("cvc5") {
            return None;
        }
        line.split_whitespace().find(|word| is_version(word))
    })
}

/// Whether `word` is a version: two or more groups of digits with dots between them.
fn is_version(word: &str) -> bool {
    word.split('.').count() >= 2
        && word
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// The z3 strategy: bit-blast straight into the SAT solver. z3 4.16's own simplifier gets some
/// bit-vector formulas wrong (it called unsatisfiable formulas satisfiable, and the reverse), so
/// lengua simplifies terms itself and uses z3 only for bit-blasting and SAT. Division, which the
/// bit-blaster can't do alone, is defined by [`Solver::division`].
const STRATEGY: &str = "(then bit-blast sat)";

/// z3's strategy where floats come in: its own for floats and bit vectors. Its `fpa2bv`, then
/// bit-blasting, gives wrong answers in z3 4.16, such as a 64-bit whole number turned into the
/// wrong single, where this and cvc5 agree with the CPU.
const FLOAT_STRATEGY: &str = "qffpbv";

/// The logic a session speaks, by whether floats come in.
const fn logic(floats: bool) -> &'static str {
    if floats { "QF_FPBV" } else { "QF_BV" }
}

/// How many times longer than the first solver the second may take to confirm an answer: cvc5
/// takes several times longer than z3's bit-blasting on lengua's questions.
const CONFIRM_FACTOR: u64 = 4;

/// The most earlier results of one float operation a new one is tied to, for each.
const MAX_CONGRUENT: usize = 64;

/// One running solver.
struct Process {
    kind: Kind,
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    /// The time limit the solver was last given, in milliseconds.
    limit_ms: u64,
    /// The time limit it started with, which a reset returns to.
    start_ms: u64,
    /// Whether its last answer came only as its time limit ran out, which a solver cancelled
    /// mid-step may give another reason for.
    ran_out: bool,
    /// Whether its questions may hold floats, which bit-vector settings can't answer.
    floats: bool,
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        super::watch::ended(self.child.id());
    }
}

/// The command that starts `program` as `kind`, giving each question at most `timeout_ms`, for
/// questions that may hold floats when `floats` says so: what starts the program, then the
/// solver's own arguments, the same for a WebAssembly solver as for a native one.
fn command(kind: Kind, program: &Program, timeout_ms: u64, floats: bool) -> Command {
    let mut command = program.command();
    match kind {
        Kind::Z3 => command.args(["-in", "-smt2", "-memory:4096"]),
        // Bit-blasting each question whole: lazily, cvc5 can take minutes over two copies of
        // one small sum that eagerly take it milliseconds. It bit-blasts floats only lazily.
        Kind::Cvc5 => command
            .args(["--lang=smt2", "--incremental", "--produce-models"])
            .args((!floats).then_some("--bitblast=eager"))
            .arg(format!("--tlimit-per={timeout_ms}")),
    };
    command
}

impl Process {
    /// Starts `program`, which speaks like `kind`, giving each question at most `timeout_ms`, for
    /// questions that may hold floats when `floats` says so.
    fn start(kind: Kind, program: &Program, timeout_ms: u64, floats: bool) -> io::Result<Self> {
        if let Program::Wasm { module, .. } = program
            && !module.is_file()
        {
            return Err(io::Error::other(format!(
                "its WebAssembly module {} isn't there",
                module.display()
            )));
        }
        let mut child = command(kind, program, timeout_ms, floats)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        super::watch::started(child.id());
        let input = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("the solver has no input"))?;
        let output = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| io::Error::other("the solver has no output"))?,
        );
        let mut process = Self {
            kind,
            child,
            input,
            output,
            limit_ms: timeout_ms,
            start_ms: timeout_ms,
            ran_out: false,
            floats,
        };
        process.prepare()?;
        Ok(process)
    }

    /// Sets the options and logic every session starts with.
    fn prepare(&mut self) -> io::Result<()> {
        let logic = logic(self.floats);
        self.send(&match self.kind {
            Kind::Z3 => format!(
                "(set-option :print-success false)\n(set-option :produce-models true)\n\
                 (set-logic {logic})\n"
            ),
            Kind::Cvc5 => format!("(set-logic {logic})\n"),
        })?;
        self.sync()?;
        Ok(())
    }

    /// Forgets every definition and assertion, as if just started: the same process serves the
    /// next question set. Options given on the command line stay.
    fn reset(&mut self) -> io::Result<()> {
        self.send("(reset)\n")?;
        self.limit_ms = self.start_ms;
        self.prepare()
    }

    fn send(&mut self, text: &str) -> io::Result<()> {
        self.input
            .write_all(text.as_bytes())
            .and_then(|()| self.input.flush())
            .map_err(|error| self.failed(error))
    }

    /// Why the solver failed: the watchdog's reason when it stopped the solver.
    fn failed(&self, error: io::Error) -> io::Error {
        if super::watch::was_stopped(self.child.id()) {
            io::Error::other(super::watch::LOW_MEMORY)
        } else {
            error
        }
    }

    /// Waits until the solver has answered everything sent so far; returns what it printed.
    ///
    /// # Errors
    ///
    /// Fails when the solver stops or rejects a command.
    fn sync(&mut self) -> io::Result<Vec<String>> {
        self.send(&format!("(echo \"{MARKER}\")\n"))?;
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            let read = self
                .output
                .read_line(&mut line)
                .map_err(|error| self.failed(error))?;
            if read == 0 {
                return Err(self.failed(io::Error::other(format!("{} stopped", self.kind.name()))));
            }
            // cvc5 echoes the quotes too.
            let line = line.trim().trim_matches('"').to_owned();
            if line == MARKER {
                return Ok(lines);
            }
            if line.starts_with("(error") {
                return Err(io::Error::other(format!(
                    "{} rejected a question: {line}",
                    self.kind.name()
                )));
            }
            lines.push(line);
        }
    }

    /// Whether the conditions asserted can all hold, decided within `limit_ms`.
    fn check_sat(&mut self, limit_ms: u64) -> io::Result<Answer> {
        let command = match self.kind {
            Kind::Z3 => format!(
                "(check-sat-using (try-for {} {limit_ms}))\n",
                if self.floats {
                    FLOAT_STRATEGY
                } else {
                    STRATEGY
                }
            ),
            Kind::Cvc5 if limit_ms == self.limit_ms => "(check-sat)\n".to_owned(),
            Kind::Cvc5 => format!("(set-option :tlimit-per {limit_ms})\n(check-sat)\n"),
        };
        self.limit_ms = limit_ms;
        let asked = std::time::Instant::now();
        self.send(&command)?;
        let lines = self.sync()?;
        let answer = match lines
            .iter()
            .find(|line| !line.is_empty())
            .map(String::as_str)
        {
            Some("sat") => Answer::Sat,
            Some("unsat") => Answer::Unsat,
            _ => Answer::Unknown,
        };
        self.ran_out = answer == Answer::Unknown
            && asked.elapsed()
                >= std::time::Duration::from_millis(limit_ms.saturating_sub(limit_ms / 20));
        Ok(answer)
    }

    /// Why the solver's last answer was unknown, in its own words: "ran out of time" for a
    /// timeout.
    fn reason(&mut self) -> io::Result<String> {
        self.send("(get-info :reason-unknown)\n")?;
        let text = self.sync()?.join(" ");
        let reason = text
            .split_once(":reason-unknown")
            .map_or(text.as_str(), |(_, reason)| reason)
            .trim_matches(|character: char| character == ')' || character.is_whitespace())
            .trim_matches('"')
            .to_owned();
        Ok(
            if self.ran_out
                || reason.contains("timeout")
                || reason.contains("canceled")
                || reason.is_empty()
            {
                "ran out of time".to_owned()
            } else {
                format!("couldn't decide ({reason})")
            },
        )
    }

    /// The values of `values` and of every variable in the inputs the solver found, when they
    /// really meet every condition: a solver's model is checked, never trusted.
    fn model(
        &mut self,
        pool: &Pool,
        conditions: &[Term],
        values: &[Term],
    ) -> io::Result<Option<HashMap<Term, u128>>> {
        let every: Vec<Term> = conditions.iter().chain(values).copied().collect();
        let variables = pool.variables(&every);
        let asked: Vec<Term> = values.iter().chain(&variables).copied().collect();
        let model = if asked.is_empty() {
            HashMap::new()
        } else {
            let names: Vec<String> = asked
                .iter()
                .map(|term| format!("t{}", term.index()))
                .collect();
            self.send(&format!("(get-value ({}))\n", names.join(" ")))?;
            parse_values(&self.sync()?.join(" "))
        };
        let by_name: HashMap<String, u128> = variables
            .iter()
            .filter_map(|term| Some((pool.name(*term)?.to_owned(), *model.get(term)?)))
            .collect();
        let holds = conditions
            .iter()
            .all(|condition| pool.evaluate(*condition, &by_name) == Some(1));
        if !holds {
            return Ok(None);
        }
        // A session that doesn't read floats gave its float results any values the
        // conditions allowed: a value with one in it is its inputs' own.
        let mut model = model;
        if !self.floats {
            for value in values {
                if pool.has_float(&[*value]) {
                    let Some(exact) = pool.evaluate(*value, &by_name) else {
                        return Ok(None);
                    };
                    model.insert(*value, exact);
                }
            }
        }
        Ok(Some(model))
    }
}

/// The solvers a proof asks: the first answers every question, and the second, when there is
/// one, must agree before any set of conditions counts as impossible.
pub struct Solver {
    first: Process,
    second: Option<Process>,
    /// Why no second solver checks the first, when none does.
    pub alone: Option<String>,
    /// Everything sent, when `LENGUA_SMT_LOG` names a folder to keep it in.
    log: Option<std::fs::File>,
    defined: Vec<bool>,
    /// The questions asked so far, for the trace.
    pub questions: usize,
    /// Times the second solver found real inputs for conditions the first called impossible.
    pub overruled: usize,
    /// Why the last unknown answer was unknown.
    pub why_unknown: String,
    timeout_ms: u64,
    /// When to stop asking: questions after it are unknown.
    deadline: Option<std::time::Instant>,
    /// The quotient and remainder variables standing for each division.
    divisions: HashMap<Division, (String, String)>,
    /// The sums defined as counts, in the bits their range needs.
    counts: HashSet<Term>,
    /// The session that answers questions with floats in them, started at the first.
    floats: Option<Box<Solver>>,
    /// In a session that reads no floats, the float results defined so far, by the function
    /// they compute (see [`Node::float_function`]).
    float_results: HashMap<(u8, u32, u32), Vec<Term>>,
}

/// Why questions with floats have one solver: z3 4.16 calls some of them possible, with inputs
/// that don't meet them, such as a single's conversion to a 64-bit whole number.
const FLOATS_ALONE: &str = "cvc5 alone answers the questions with floats, which z3 4.16 gets wrong";

fn sort_text(sort: Sort) -> String {
    match sort {
        Sort::Bool => "Bool".into(),
        Sort::Bv(width) => format!("(_ BitVec {width})"),
    }
}

/// A division the solver gets as quotient and remainder variables: by a constant, by a term, or
/// signed (on the sizes of two terms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Division {
    ByConstant(Term, u128),
    ByTerm(Term, Term),
    Signed(Term, Term),
}

/// `value` as a bit-vector literal `width` bits wide.
fn literal(value: u128, width: u32) -> String {
    if width.is_multiple_of(4) {
        format!("#x{value:0digits$x}", digits = (width / 4) as usize)
    } else {
        format!("#b{value:0digits$b}", digits = width as usize)
    }
}

/// The bits `term`'s range needs, when that's fewer than its width.
fn narrow_width(pool: &Pool, term: Term) -> Option<u32> {
    let width = pool.width(term);
    let needed = (128 - pool.range(term).1.leading_zeros()).max(1);
    (needed < width).then_some(needed)
}

/// Every bit of `width` set.
const fn ones(width: u32) -> u128 {
    if width >= 128 {
        u128::MAX
    } else {
        (1 << width) - 1
    }
}

/// Numbers the solver sessions of this process, for their logs.
static SESSIONS: AtomicUsize = AtomicUsize::new(0);

impl Solver {
    /// Starts z3, and cvc5 to check it, each where [`find`] finds it, giving each question at
    /// most `timeout_ms`. When only one of them is installed, it answers alone.
    ///
    /// # Errors
    ///
    /// Fails when neither solver can start.
    pub fn start(timeout_ms: u64) -> io::Result<Self> {
        let solvers = Kind::ALL.map(|kind| (kind, program(kind)));
        Self::with_programs(&solvers, timeout_ms, false)
    }

    /// [`Self::with_programs`], each a native program given by its name or path as text.
    #[cfg(test)]
    fn with(solvers: &[(Kind, &str)], timeout_ms: u64, floats: bool) -> io::Result<Self> {
        let solvers: Vec<(Kind, Program)> = solvers
            .iter()
            .map(|&(kind, program)| (kind, Program::Native(OsString::from(program))))
            .collect();
        Self::with_programs(&solvers, timeout_ms, floats)
    }

    /// Starts the first of `solvers` that runs, to answer, and the next one that runs, to check
    /// it, giving each question at most `timeout_ms`, for questions that may hold floats when
    /// `floats` says so. Each is a kind of solver and what to start for it.
    ///
    /// # Errors
    ///
    /// Fails when none of them can start, saying why for each.
    pub fn with_programs(
        solvers: &[(Kind, Program)],
        timeout_ms: u64,
        floats: bool,
    ) -> io::Result<Self> {
        let mut running = Vec::new();
        let mut missing = Vec::new();
        for (kind, program) in solvers {
            let kind = *kind;
            if running.len() == 2 {
                break;
            }
            // The second solver gets longer, to confirm what the first found fast.
            let limit = if running.is_empty() {
                timeout_ms
            } else {
                timeout_ms * CONFIRM_FACTOR
            };
            match Process::start(kind, program, limit, floats) {
                Ok(process) => running.push(process),
                Err(error)
                    if error.kind() == io::ErrorKind::NotFound
                        && matches!(program, Program::Native(_)) =>
                {
                    missing.push(format!("{} isn't installed", kind.name()));
                }
                Err(error) => missing.push(format!("{} couldn't start: {error}", kind.name())),
            }
        }
        let mut running = running.into_iter();
        let first = running
            .next()
            .ok_or_else(|| io::Error::other(missing.join("; ")))?;
        let second = running.next();
        let alone = second.is_none().then(|| {
            if missing.is_empty() {
                "no second solver was asked for".to_owned()
            } else {
                missing.join("; ")
            }
        });
        let log = std::env::var_os("LENGUA_SMT_LOG").and_then(|folder| {
            let session = SESSIONS.fetch_add(1, Ordering::Relaxed);
            let name = format!("{}-{session}.smt2", std::process::id());
            let mut file = std::fs::File::create(std::path::Path::new(&folder).join(name)).ok()?;
            file.write_all(format!("(set-logic {})\n", logic(floats)).as_bytes())
                .ok()?;
            Some(file)
        });
        Ok(Self {
            first,
            second,
            alone,
            log,
            defined: Vec::new(),
            questions: 0,
            overruled: 0,
            why_unknown: String::new(),
            timeout_ms,
            deadline: None,
            divisions: HashMap::new(),
            counts: HashSet::new(),
            floats: None,
            float_results: HashMap::new(),
        })
    }

    /// Stops asking after `deadline`: every question after it is unknown.
    pub fn until(&mut self, deadline: std::time::Instant) {
        self.deadline = Some(deadline);
        if let Some(floats) = &mut self.floats {
            floats.until(deadline);
        }
    }

    /// Forgets every term and division defined so far, so the processes can serve terms from a
    /// new pool without being started again.
    ///
    /// # Errors
    ///
    /// Fails when a solver stops responding.
    pub fn reset(&mut self) -> io::Result<()> {
        let logic = logic(self.first.floats);
        self.note(&format!("(reset)\n(set-logic {logic})\n"));
        self.first.reset()?;
        if let Some(second) = &mut self.second {
            second.reset()?;
        }
        self.defined.clear();
        self.divisions.clear();
        self.counts.clear();
        self.float_results.clear();
        self.why_unknown.clear();
        if let Some(floats) = &mut self.floats {
            floats.reset()?;
        }
        Ok(())
    }

    /// Whether the deadline has passed.
    #[must_use]
    pub fn out_of_time(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
    }

    /// The time limit for a question that may take `most` milliseconds, cut to what's left
    /// before the deadline.
    fn limit(&self, most: u64) -> u64 {
        self.deadline.map_or(most, |deadline| {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            most.min(u64::try_from(left.as_millis()).unwrap_or(u64::MAX))
        })
    }

    /// Sends `text` to every solver.
    fn send(&mut self, text: &str) -> io::Result<()> {
        if let Some(log) = &mut self.log {
            let _ = log.write_all(text.as_bytes());
        }
        self.first.send(text)?;
        if let Some(second) = &mut self.second {
            second.send(text)?;
        }
        Ok(())
    }

    /// Defines `term` and everything it's made of, once each.
    fn define(&mut self, pool: &Pool, term: Term) -> io::Result<()> {
        if self.defined.len() < pool.len() {
            self.defined.resize(pool.len(), false);
        }
        let mut text = String::new();
        let mut pending = vec![(term, false)];
        while let Some((current, ready)) = pending.pop() {
            if self.defined[current.index()] {
                continue;
            }
            if !ready {
                pending.push((current, true));
                for operand in pool.operands(current) {
                    if !self.defined[operand.index()] {
                        pending.push((operand, false));
                    }
                }
                continue;
            }
            self.defined[current.index()] = true;
            let sort = sort_text(pool.sort(current));
            match pool.node(current) {
                // Where floats can't be read, a float result is an unknown in the range its
                // bounds give, that the same operation on equal operands gives again.
                node if !self.first.floats && node.is_float_operation() => {
                    let index = current.index();
                    text.push_str(&format!("(declare-const t{index} {sort})\n"));
                    if let Sort::Bv(width) = pool.sort(current) {
                        let (low, high) = pool.range(current);
                        if (low, high) != (0, ones(width)) {
                            text.push_str(&format!(
                                "(assert (bvule {} t{index}))\n(assert (bvule t{index} {}))\n",
                                literal(low, width),
                                literal(high, width)
                            ));
                        }
                    }
                    let operands = pool.operands(current);
                    let width = operands.first().map_or(0, |operand| pool.width(*operand));
                    if let Some(function) = node.float_function(width) {
                        let earlier = self.float_results.entry(function).or_default();
                        for other in earlier.iter().rev().take(MAX_CONGRUENT) {
                            let same: Vec<String> = operands
                                .iter()
                                .zip(pool.operands(*other))
                                .map(|(x, y)| format!("(= t{} t{})", x.index(), y.index()))
                                .collect();
                            let same = match same.as_slice() {
                                [only] => only.clone(),
                                _ => format!("(and {})", same.join(" ")),
                            };
                            text.push_str(&format!(
                                "(assert (=> {same} (= t{index} t{})))\n",
                                other.index()
                            ));
                        }
                        earlier.push(current);
                    }
                }
                Node::Var { width, .. } => {
                    text.push_str(&format!("(declare-const t{} {sort})\n", current.index()));
                    // The solver must know every range the terms were simplified with.
                    let (low, high) = pool.range(current);
                    let full = if *width >= 128 {
                        u128::MAX
                    } else {
                        (1 << width) - 1
                    };
                    if (low, high) != (0, full) {
                        let literal = |value: u128| {
                            if width.is_multiple_of(4) {
                                format!("#x{value:0digits$x}", digits = (width / 4) as usize)
                            } else {
                                format!("#b{value:0digits$b}", digits = *width as usize)
                            }
                        };
                        text.push_str(&format!(
                            "(assert (bvule {} t{index}))\n(assert (bvule t{index} {}))\n",
                            literal(low),
                            literal(high),
                            index = current.index()
                        ));
                    }
                }
                Node::BoolVar(_) => {
                    text.push_str(&format!("(declare-const t{} {sort})\n", current.index()));
                }
                // z3's bit-blaster can't divide, so every division is a quotient and a remainder
                // that multiply back.
                &Node::Udiv(dividend, divisor) | &Node::Urem(dividend, divisor) => {
                    let division = match pool.constant(divisor) {
                        Some(constant) if constant > 0 => Division::ByConstant(dividend, constant),
                        _ => Division::ByTerm(dividend, divisor),
                    };
                    let (quotient, remainder) = self.division(pool, division, &mut text);
                    let name = if matches!(pool.node(current), Node::Udiv(..)) {
                        quotient
                    } else {
                        remainder
                    };
                    text.push_str(&format!(
                        "(define-fun t{} () {sort} {name})\n",
                        current.index()
                    ));
                }
                &Node::Sdiv(dividend, divisor) | &Node::Srem(dividend, divisor) => {
                    // As SMT-LIB defines them: divide the sizes, then the quotient is negative
                    // when the signs differ, and the remainder takes the dividend's sign.
                    let (quotient, remainder) =
                        self.division(pool, Division::Signed(dividend, divisor), &mut text);
                    let width = pool.width(current);
                    let (zero, one) = (literal(0, width), literal(1, width));
                    let (a, b) = (dividend.index(), divisor.index());
                    let value = if matches!(pool.node(current), Node::Sdiv(..)) {
                        format!(
                            "(ite (xor (bvslt t{a} {zero}) (bvslt t{b} {zero})) (bvadd (bvnot \
                             {quotient}) {one}) {quotient})"
                        )
                    } else {
                        format!(
                            "(ite (bvslt t{a} {zero}) (bvadd (bvnot {remainder}) {one}) \
                             {remainder})"
                        )
                    };
                    text.push_str(&format!(
                        "(define-fun t{} () {sort} {value})\n",
                        current.index()
                    ));
                }
                // A count, a sum of values that are each 0 or 1 or counts themselves, adds in the
                // few bits its range needs, which depend only on its parts' same bits: the same
                // count at two widths is then one small circuit. Other sums keep their width,
                // where bit-blasting does best with them.
                &Node::Add(x, y)
                    if [x, y]
                        .iter()
                        .all(|part| pool.range(*part).1 <= 1 || self.counts.contains(part))
                        && narrow_width(pool, current).is_some() =>
                {
                    let (narrow, width) = (
                        narrow_width(pool, current).unwrap_or(1),
                        pool.width(current),
                    );
                    let low =
                        |term: Term| format!("((_ extract {} 0) t{})", narrow - 1, term.index());
                    text.push_str(&format!(
                        "(define-fun t{} () {sort} ((_ zero_extend {}) (bvadd {} {})))\n",
                        current.index(),
                        width - narrow,
                        low(x),
                        low(y),
                    ));
                    self.counts.insert(current);
                }
                // A count adds its ones and zeros in the bits its length needs.
                &Node::Count(number, _) => {
                    let parts = pool.count_parts(number);
                    let width = pool.width(current);
                    let narrow = (128 - (parts.len() as u128).leading_zeros()).clamp(1, width);
                    let (one, zero) = (literal(1, narrow), literal(0, narrow));
                    let ones: Vec<String> = parts
                        .iter()
                        .map(|part| format!("(ite t{} {one} {zero})", part.index()))
                        .collect();
                    let sum = match ones.as_slice() {
                        [only] => only.clone(),
                        _ => format!("(bvadd {})", ones.join(" ")),
                    };
                    let value = if narrow < width {
                        format!("((_ zero_extend {}) {sum})", width - narrow)
                    } else {
                        sum
                    };
                    text.push_str(&format!(
                        "(define-fun t{} () {sort} {value})\n",
                        current.index()
                    ));
                }
                // A float's bits: declared, pinned by its value where it isn't a NaN, and x86's
                // NaN where it is. cvc5 can't turn a float back into bits, so no expression does.
                Node::Float(..)
                | Node::FloatSqrt(_)
                | Node::FloatFma(..)
                | Node::FloatFromInt(..)
                | Node::FloatRound(..)
                | Node::FloatConvert(..) => {
                    let Some(definition) = pool.float_definition(current) else {
                        continue;
                    };
                    let index = current.index();
                    let bits = format!("f{index}");
                    text.push_str(&format!(
                        "(declare-const {bits} {sort})\n(assert (=> (not (fp.isNaN {value})) (= \
                         {} {value})))\n(define-fun t{index} () {sort} (ite (fp.isNaN {value}) \
                         {nan} {bits}))\n",
                        crate::prove::term::fp_of(&bits, definition.width),
                        value = definition.value,
                        nan = definition.nan,
                    ));
                }
                &Node::Mul(factor, by) if pool.constant(by).is_some() => {
                    let constant = pool.constant(by).unwrap_or(0);
                    let width = pool.width(current);
                    text.push_str(&format!(
                        "(define-fun t{} () {sort} {})\n",
                        current.index(),
                        shifted_sum(&format!("t{}", factor.index()), constant, width)
                    ));
                }
                _ => text.push_str(&format!(
                    "(define-fun t{} () {sort} {})\n",
                    current.index(),
                    pool.smt_node(current)
                )),
            }
        }
        self.send(&text)
    }

    /// The quotient and remainder of a division, as variables defined by `dividend = quotient ×
    /// divisor + remainder` and `remainder < divisor`, computed in double width so nothing wraps;
    /// dividing by zero gives all ones and the dividend, as in SMT-LIB. A signed division divides
    /// the sizes. The definitions are asserted once, outside any question.
    fn division(&mut self, pool: &Pool, division: Division, text: &mut String) -> (String, String) {
        if let Some(names) = self.divisions.get(&division) {
            return names.clone();
        }
        let index = self.divisions.len();
        let (quotient, remainder) = (format!("q{index}"), format!("r{index}"));
        let (Division::ByConstant(dividend, _)
        | Division::ByTerm(dividend, _)
        | Division::Signed(dividend, _)) = division;
        let width = pool.width(dividend);
        if let Division::ByConstant(dividend, divisor) = division {
            // At the width the dividend's range needs, which is often far below its own: a
            // count of a few bytes divides as a few bits, not as 64.
            let needed = |value: u128| 128 - value.leading_zeros();
            let narrow = needed(pool.range(dividend).1)
                .max(needed(divisor))
                .clamp(1, width);
            let low = if narrow < width {
                format!("((_ extract {} 0) t{})", narrow - 1, dividend.index())
            } else {
                format!("t{}", dividend.index())
            };
            let wide = |name: &str| format!("((_ zero_extend {narrow}) {name})");
            text.push_str(&format!(
                "(declare-const {quotient} (_ BitVec {narrow}))\n(declare-const {remainder} (_ \
                 BitVec {narrow}))\n(assert (= {} (bvadd {} {})))\n(assert (bvult {remainder} \
                 {}))\n",
                wide(&low),
                shifted_sum(&wide(&quotient), divisor, 2 * narrow),
                wide(&remainder),
                literal(divisor, narrow),
            ));
            let back = |name: String| {
                if narrow < width {
                    format!("((_ zero_extend {}) {name})", width - narrow)
                } else {
                    name
                }
            };
            let names = (back(quotient), back(remainder));
            self.divisions.insert(division, names.clone());
            return names;
        }
        let wide = |name: &str| format!("((_ zero_extend {width}) {name})");
        let size = |term: Term| {
            format!(
                "(ite (bvslt t{index} {zero}) (bvadd (bvnot t{index}) {one}) t{index})",
                index = term.index(),
                zero = literal(0, width),
                one = literal(1, width),
            )
        };
        text.push_str(&format!(
            "(declare-const {quotient} (_ BitVec {width}))\n(declare-const {remainder} (_ BitVec \
             {width}))\n"
        ));
        match division {
            Division::ByConstant(..) => {}
            Division::ByTerm(dividend, divisor) | Division::Signed(dividend, divisor) => {
                let (dividend, divisor) = if matches!(division, Division::Signed(..)) {
                    let (a, b) = (format!("a{index}"), format!("b{index}"));
                    text.push_str(&format!(
                        "(define-fun {a} () (_ BitVec {width}) {})\n(define-fun {b} () (_ \
                         BitVec {width}) {})\n",
                        size(dividend),
                        size(divisor)
                    ));
                    (a, b)
                } else {
                    (
                        format!("t{}", dividend.index()),
                        format!("t{}", divisor.index()),
                    )
                };
                text.push_str(&format!(
                    "(assert (ite (= {divisor} {zero}) (and (= {quotient} {all}) (= {remainder} \
                     {dividend})) (and (= {} (bvadd (bvmul {} {}) {})) (bvult {remainder} \
                     {divisor}))))\n",
                    wide(&dividend),
                    wide(&quotient),
                    wide(&divisor),
                    wide(&remainder),
                    zero = literal(0, width),
                    all = literal(ones(width), width),
                ));
            }
        }
        self.divisions
            .insert(division, (quotient.clone(), remainder.clone()));
        (quotient, remainder)
    }

    /// Writes `text` to the log, when there is one.
    fn note(&mut self, text: &str) {
        if let Some(log) = &mut self.log {
            let _ = log.write_all(text.as_bytes());
        }
    }

    /// Whether some input meets every condition in `conditions`. With `values`, also returns
    /// those terms' values for one such input, by term. The conditions are impossible only when
    /// every solver says so, and inputs a solver finds count only when they meet the conditions.
    ///
    /// # Errors
    ///
    /// Fails when a solver stops responding or rejects a question.
    pub fn check(
        &mut self,
        pool: &Pool,
        conditions: &[Term],
        values: &[Term],
    ) -> io::Result<(Answer, HashMap<Term, u128>)> {
        self.ask(pool, conditions, values, false)
    }

    /// Whether some input meets every condition in `conditions`, and when the solver found one,
    /// every variable's value in it, checked against the conditions.
    ///
    /// # Errors
    ///
    /// Fails when a solver stops responding or rejects a question.
    pub fn witness(
        &mut self,
        pool: &Pool,
        conditions: &[Term],
    ) -> io::Result<(Answer, HashMap<Term, u128>)> {
        self.ask(pool, conditions, &[], true)
    }

    fn ask(
        &mut self,
        pool: &Pool,
        conditions: &[Term],
        values: &[Term],
        model_wanted: bool,
    ) -> io::Result<(Answer, HashMap<Term, u128>)> {
        // A condition the terms already settle as false needs no solver, and one they settle
        // as true says nothing.
        if conditions
            .iter()
            .any(|condition| pool.truth(*condition) == Some(false))
        {
            return Ok((Answer::Unsat, HashMap::new()));
        }
        let unsettled: Vec<Term> = conditions
            .iter()
            .copied()
            .filter(|condition| pool.truth(*condition).is_none())
            .collect();
        let conditions = unsettled.as_slice();
        // A condition and its negation can't both hold, whatever they say: a run's own branch
        // and the check that its output breaks come to that when both sides are one term.
        if conditions.iter().any(|condition| {
            matches!(*pool.node(*condition), Node::BoolNot(inner)
                if conditions.iter().any(|other| pool.equivalent(*other, inner)))
        }) {
            return Ok((Answer::Unsat, HashMap::new()));
        }
        self.questions += 1;
        if !self.first.floats && (pool.has_float(conditions) || pool.has_float(values)) {
            // First as bits, every float result an unknown that equal operands give again,
            // which allows more than the floats do: impossible then is impossible, and inputs
            // found count only where they really meet the conditions.
            let (answer, model) = self.ask_bits(pool, conditions, values, true)?;
            if answer != Answer::Unknown {
                return Ok((answer, model));
            }
            // A branch the bits couldn't rule out is followed, which costs only time; whether
            // an output can break goes to the floats themselves.
            if model_wanted && values.is_empty() {
                return Ok((Answer::Unknown, HashMap::new()));
            }
            // Floats go to cvc5, in a session of their own that reads them; that session
            // answers them itself.
            if self.floats.is_none() {
                let cvc5 = [(Kind::Cvc5, program(Kind::Cvc5))];
                let mut floats = Self::with_programs(&cvc5, self.timeout_ms, true)?;
                if let Some(deadline) = self.deadline {
                    floats.until(deadline);
                }
                self.floats = Some(Box::new(floats));
            }
            let Some(floats) = self.floats.as_deref_mut() else {
                return Ok((Answer::Unknown, HashMap::new()));
            };
            let answer = floats.ask(pool, conditions, values, model_wanted)?;
            self.why_unknown.clone_from(&floats.why_unknown);
            self.alone.get_or_insert_with(|| FLOATS_ALONE.to_owned());
            return Ok(answer);
        }
        self.ask_bits(pool, conditions, values, model_wanted)
    }

    /// [`Self::ask`] of this session's solvers, as bit vectors.
    fn ask_bits(
        &mut self,
        pool: &Pool,
        conditions: &[Term],
        values: &[Term],
        model_wanted: bool,
    ) -> io::Result<(Answer, HashMap<Term, u128>)> {
        if self.out_of_time() {
            self.why_unknown = "the proof ran out of time".into();
            return Ok((Answer::Unknown, HashMap::new()));
        }
        for term in conditions.iter().chain(values) {
            self.define(pool, *term)?;
        }
        let mut text = String::from("(push 1)\n");
        for condition in conditions {
            text.push_str(&format!("(assert t{})\n", condition.index()));
        }
        self.send(&text)?;
        let asked = std::time::Instant::now();
        let first = self.first.check_sat(self.limit(self.timeout_ms))?;
        let name = self.first.kind.name();
        self.note(&format!(
            "(check-sat)\n; {name}: {first:?} in {:?}\n",
            asked.elapsed()
        ));
        let mut model = HashMap::new();
        let answer = match first {
            Answer::Sat if values.is_empty() && !model_wanted => Answer::Sat,
            Answer::Sat => {
                if let Some(found) = self.first.model(pool, conditions, values)? {
                    model = found;
                    Answer::Sat
                } else {
                    self.why_unknown =
                        format!("{name} found inputs that don't meet the conditions");
                    Answer::Unknown
                }
            }
            Answer::Unsat => self.confirm(pool, conditions, values, &mut model)?,
            Answer::Unknown => {
                let why = format!("{name} {}", self.first.reason()?);
                let wanted = model_wanted || !values.is_empty();
                if self.rescue(pool, conditions, values, wanted, &mut model)? {
                    Answer::Sat
                } else {
                    self.why_unknown = why;
                    Answer::Unknown
                }
            }
        };
        self.send("(pop 1)\n")?;
        if first != Answer::Unsat
            && let Some(second) = &mut self.second
        {
            // Kept in step, so a question it can't read shows at once.
            second.sync()?;
        }
        Ok((answer, model))
    }

    /// The second solver's try at conditions the first couldn't settle: inputs that meet them
    /// show they're possible, which a second solver alone can show. Its impossible alone shows
    /// nothing. With `wanted`, the inputs are checked against the conditions and kept in `model`.
    fn rescue(
        &mut self,
        pool: &Pool,
        conditions: &[Term],
        values: &[Term],
        wanted: bool,
        model: &mut HashMap<Term, u128>,
    ) -> io::Result<bool> {
        let limit = self.limit(self.timeout_ms * CONFIRM_FACTOR);
        let Some(second) = &mut self.second else {
            return Ok(false);
        };
        if limit == 0 {
            return Ok(false);
        }
        let name = second.kind.name();
        let asked = std::time::Instant::now();
        let answer = second.check_sat(limit)?;
        let elapsed = asked.elapsed();
        let possible = match answer {
            Answer::Sat if !wanted => true,
            Answer::Sat => match second.model(pool, conditions, values)? {
                Some(found) => {
                    *model = found;
                    true
                }
                None => false,
            },
            _ => false,
        };
        self.note(&format!("; {name}: {answer:?} in {elapsed:?}\n"));
        Ok(possible)
    }

    /// The second solver's view of conditions the first found impossible: impossible when it
    /// agrees, and possible when it finds inputs that really meet them all, which `model` then
    /// holds.
    fn confirm(
        &mut self,
        pool: &Pool,
        conditions: &[Term],
        values: &[Term],
        model: &mut HashMap<Term, u128>,
    ) -> io::Result<Answer> {
        let first = self.first.kind.name();
        let limit = self.limit(self.timeout_ms * CONFIRM_FACTOR);
        let Some(second) = &mut self.second else {
            return Ok(Answer::Unsat);
        };
        let name = second.kind.name();
        let asked = std::time::Instant::now();
        let answer = second.check_sat(limit)?;
        let elapsed = asked.elapsed();
        let (answer, why) = match answer {
            Answer::Unsat => (Answer::Unsat, String::new()),
            Answer::Sat => match second.model(pool, conditions, values)? {
                Some(found) => {
                    *model = found;
                    self.overruled += 1;
                    (Answer::Sat, String::new())
                }
                None => (Answer::Unknown, format!("{first} and {name} disagree")),
            },
            Answer::Unknown => (
                Answer::Unknown,
                format!(
                    "{name} couldn't confirm {first}'s answer: it {}",
                    second.reason()?
                ),
            ),
        };
        self.note(&format!("; {name}: {answer:?} in {elapsed:?}\n"));
        if answer == Answer::Unknown {
            self.why_unknown = why;
        }
        Ok(answer)
    }
}

/// `value × constant` as a sum of shifted copies, `8x + 2x` for `10x`, which bit-blasting turns
/// into wiring and adders instead of a whole multiplier.
fn shifted_sum(value: &str, constant: u128, width: u32) -> String {
    let literal = |number: u128| {
        if width.is_multiple_of(4) {
            format!("#x{number:0digits$x}", digits = (width / 4) as usize)
        } else {
            format!("#b{number:0digits$b}", digits = width as usize)
        }
    };
    let parts: Vec<String> = (0..width.min(128))
        .filter(|bit| constant >> bit & 1 == 1)
        .map(|bit| {
            if bit == 0 {
                value.to_owned()
            } else {
                format!("(bvshl {value} {})", literal(u128::from(bit)))
            }
        })
        .collect();
    match parts.as_slice() {
        [] => literal(0),
        [only] => only.clone(),
        [first, rest @ ..] => rest
            .iter()
            .fold(first.clone(), |sum, part| format!("(bvadd {sum} {part})")),
    }
}

/// Reads `((t1 #x0a) (t2 true) …)` into values by term.
fn parse_values(text: &str) -> HashMap<Term, u128> {
    let mut values = HashMap::new();
    let cleaned = text.replace(['(', ')'], " ");
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();
    let mut index = 0;
    while index + 1 < tokens.len() {
        let Some(number) = tokens[index]
            .strip_prefix('t')
            .and_then(|digits| digits.parse::<u32>().ok())
        else {
            index += 1;
            continue;
        };
        let value = tokens[index + 1];
        let parsed = if let Some(hex) = value.strip_prefix("#x") {
            u128::from_str_radix(hex, 16).ok()
        } else if let Some(bits) = value.strip_prefix("#b") {
            u128::from_str_radix(bits, 2).ok()
        } else {
            match value {
                "true" => Some(1),
                "false" => Some(0),
                _ => None,
            }
        };
        if let Some(parsed) = parsed {
            values.insert(super::term::term_at(number), parsed);
            index += 2;
        } else {
            index += 1;
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every pairing of the installed solvers: z3 checked by cvc5, and each alone.
    fn solvers() -> Vec<Solver> {
        [
            &[(Kind::Z3, "z3"), (Kind::Cvc5, "cvc5")][..],
            &[(Kind::Z3, "z3")],
            &[(Kind::Cvc5, "cvc5")],
        ]
        .iter()
        .filter_map(|solvers| Solver::with(solvers, 5000, false).ok())
        .collect()
    }

    #[test]
    fn answers_and_gives_models() {
        for mut solver in solvers() {
            let mut pool = Pool::default();
            let a = pool.var(64, "a");
            let ten = pool.bv(64, 10);
            let product = pool.mul(a, ten);
            let seventy = pool.bv(64, 70);
            let is_seventy = pool.eq(product, seventy);
            let (answer, model) = solver
                .check(&pool, &[is_seventy], &[a])
                .expect("the solver answers");
            assert_eq!(answer, Answer::Sat);
            let found = model.get(&a).copied().expect("a value for a");
            assert_eq!(found.wrapping_mul(10) & u128::from(u64::MAX), 70);
            // x - 48 widened is x widened minus 48 when x is at least 48: the query the review
            // saw z3 4.16 get wrong when written with bvsub.
            let byte = pool.var(8, "b");
            let zero_char = pool.bv(8, 0x30);
            let at_least = pool.ule(zero_char, byte);
            let wide = pool.zero_extend(byte, 24);
            let forty_eight = pool.bv(32, 0x30);
            let narrow_difference = pool.sub(wide, forty_eight);
            let left = pool.zero_extend(narrow_difference, 32);
            let wide64 = pool.zero_extend(byte, 56);
            let forty_eight64 = pool.bv(64, 0x30);
            let right = pool.sub(wide64, forty_eight64);
            let same = pool.eq(left, right);
            let differs = pool.bool_not(same);
            let (answer, _) = solver
                .check(&pool, &[at_least, differs], &[])
                .expect("the solver answers");
            assert_eq!(answer, Answer::Unsat);
            assert_eq!(solver.questions, 2);
            assert_eq!(solver.overruled, 0);
        }
    }

    /// A stand-in solver that reads commands and answers `answer` to every question, with an
    /// empty model: a shell script, so Unix's alone until the fake is the test binary itself;
    /// elsewhere there is none, and the test that wants it ends with a note.
    fn fake(folder: &std::path::Path, answer: &str) -> Option<String> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let path = folder.join(format!("always-{answer}"));
            std::fs::write(
                &path,
                format!(
                    "#!/bin/sh\nwhile read -r line; do\n  case \"$line\" in\n    \"(echo \"*) echo \
                     '{MARKER}' ;;\n    \"(check-sat\"*) echo {answer} ;;\n    \"(get-value\"*) echo \
                     '()' ;;\n  esac\ndone\n"
                ),
            )
            .expect("write the stand-in");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("make it runnable");
            // A process another test forked while the file was open for writing keeps it busy
            // until that process execs; once one run starts, none can.
            for _ in 0..100 {
                match Command::new(&path).stdin(Stdio::null()).status() {
                    Err(error) if error.raw_os_error() == Some(26) => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    _ => break,
                }
            }
            Some(path.to_string_lossy().into_owned())
        }
        #[cfg(not(unix))]
        {
            let _ = (folder, answer);
            crate::skiplog::note("the stand-in solver is a shell script, which this system lacks");
            None
        }
    }

    /// x + 1 = 0 for an 8-bit x: true only for 255.
    fn wraps(pool: &mut Pool) -> (Term, Term) {
        let x = pool.var(8, "x");
        let one = pool.bv(8, 1);
        let next = pool.add(x, one);
        let zero = pool.bv(8, 0);
        (x, pool.eq(next, zero))
    }

    #[test]
    fn the_second_solver_overrules_a_wrong_impossible() {
        let folder = tempfile::tempdir().expect("a folder");
        let Some(liar) = fake(folder.path(), "unsat") else {
            return;
        };
        let Ok(mut solver) = Solver::with(&[(Kind::Z3, &liar), (Kind::Cvc5, "cvc5")], 5000, false)
        else {
            return;
        };
        if let Some(reason) = &solver.alone {
            eprintln!("{reason}");
            return;
        }
        let mut pool = Pool::default();
        let (x, wraps) = wraps(&mut pool);
        let (answer, model) = solver.check(&pool, &[wraps], &[x]).expect("answers");
        assert_eq!((answer, model.get(&x)), (Answer::Sat, Some(&255)));
        // Without values asked for, the inputs are still checked before they count.
        let (answer, _) = solver.check(&pool, &[wraps], &[]).expect("answers");
        assert_eq!(answer, Answer::Sat);
        assert_eq!(solver.overruled, 2);
        let never = pool.boolean(false);
        let x_is_zero = {
            let zero = pool.bv(8, 0);
            pool.eq(x, zero)
        };
        let (answer, _) = solver
            .check(&pool, &[wraps, x_is_zero], &[])
            .expect("answers");
        assert_eq!(answer, Answer::Unsat);
        let (answer, _) = solver.check(&pool, &[never], &[]).expect("answers");
        assert_eq!(answer, Answer::Unsat);
    }

    #[test]
    fn impossible_needs_both_solvers() {
        let folder = tempfile::tempdir().expect("a folder");
        let Some(doubter) = fake(folder.path(), "sat") else {
            return;
        };
        let Ok(mut solver) = Solver::with(&[(Kind::Z3, "z3"), (Kind::Cvc5, &doubter)], 5000, false)
        else {
            return;
        };
        if solver.second.is_none() {
            crate::skiplog::note("z3 isn't installed");
            return;
        }
        let mut pool = Pool::default();
        let (x, wraps) = wraps(&mut pool);
        let zero = pool.bv(8, 0);
        let x_is_zero = pool.eq(x, zero);
        // z3 rightly says impossible, but the second solver claims inputs that don't hold up.
        let (answer, _) = solver
            .check(&pool, &[wraps, x_is_zero], &[])
            .expect("answers");
        assert_eq!(answer, Answer::Unknown);
        assert!(
            solver.why_unknown.contains("disagree"),
            "{}",
            solver.why_unknown
        );
        let Some(unknown) = fake(folder.path(), "unknown") else {
            return;
        };
        let mut solver =
            Solver::with(&[(Kind::Z3, "z3"), (Kind::Cvc5, &unknown)], 5000, false).expect("starts");
        let (answer, _) = solver
            .check(&pool, &[wraps, x_is_zero], &[])
            .expect("answers");
        assert_eq!(answer, Answer::Unknown);
        assert!(
            solver.why_unknown.contains("couldn't confirm"),
            "{}",
            solver.why_unknown
        );
    }

    #[test]
    fn stops_asking_at_the_deadline() {
        let Ok(mut solver) = Solver::start(5000) else {
            return;
        };
        solver.until(std::time::Instant::now());
        let mut pool = Pool::default();
        let (_, wraps) = wraps(&mut pool);
        let (answer, _) = solver.check(&pool, &[wraps], &[]).expect("answers");
        assert_eq!(answer, Answer::Unknown);
        assert!(solver.out_of_time());
        assert_eq!(solver.why_unknown, "the proof ran out of time");
    }

    #[test]
    fn a_reset_solver_answers_new_terms() {
        for mut solver in solvers() {
            let mut pool = Pool::default();
            let (x, wraps) = wraps(&mut pool);
            let (answer, model) = solver.witness(&pool, &[wraps]).expect("answers");
            assert_eq!((answer, model.get(&x)), (Answer::Sat, Some(&255)));
            // A division leaves its quotient and remainder behind.
            let ten = pool.bv(8, 10);
            let quotient = pool.udiv(x, ten);
            let thirty = pool.bv(8, 30);
            let over = pool.ult(thirty, quotient);
            let (answer, _) = solver.check(&pool, &[over], &[]).expect("answers");
            assert_eq!(answer, Answer::Unsat);
            solver.reset().expect("resets");
            // A new pool's terms reuse the same names, and its division the same quotient.
            let mut pool = Pool::default();
            let y = pool.var(8, "y");
            let ten = pool.bv(8, 10);
            let quotient = pool.udiv(y, ten);
            let twenty = pool.bv(8, 20);
            let is_twenty = pool.eq(quotient, twenty);
            let (answer, model) = solver.witness(&pool, &[is_twenty]).expect("answers");
            assert_eq!(answer, Answer::Sat);
            assert_eq!(model.get(&y).map(|y| y / 10), Some(20));
            let fifty = pool.bv(8, 50);
            let never = pool.eq(quotient, fifty);
            let (answer, _) = solver.check(&pool, &[never], &[]).expect("answers");
            assert_eq!(answer, Answer::Unsat);
        }
    }

    /// A question with floats starts one session that reads them, which answers it itself and
    /// never starts a session of its own: when it did, each float question started another,
    /// and 824 cvc5 processes filled the computer's memory.
    #[test]
    fn a_float_session_starts_no_session_of_its_own() {
        let Ok(mut solver) = Solver::start(10_000) else {
            return;
        };
        let mut pool = Pool::default();
        // x + 1.0 == x, for a 16-bit whole number x as a double: never, which the bits alone
        // can't show, so the question goes to the floats.
        let number = pool.var(16, "n");
        let x = pool.float_from_int(number, 64);
        let one = pool.bv(64, u128::from(1.0_f64.to_bits()));
        let sum = pool.float(crate::prove::term::FloatOp::Add, x, one);
        let same = pool.eq(sum, x);
        let (answer, _) = solver.check(&pool, &[same], &[]).expect("an answer");
        assert_eq!(answer, Answer::Unsat);
        let floats = solver.floats.as_ref().expect("a session for the floats");
        assert!(
            floats.floats.is_none(),
            "the float session started one of its own"
        );
    }

    /// A question to a solver the watchdog stopped fails, saying the memory ran low.
    #[test]
    fn a_solver_stopped_for_memory_says_so() {
        let Ok(mut solver) = Solver::start(1000) else {
            return;
        };
        assert!(crate::prove::watch::stop_for_test(solver.first.child.id()));
        let mut pool = Pool::default();
        let x = pool.var(8, "x");
        let three = pool.bv(8, 3);
        let same = pool.eq(x, three);
        let error = solver
            .check(&pool, &[same], &[])
            .expect_err("the solver is gone");
        assert_eq!(error.to_string(), crate::prove::watch::LOW_MEMORY);
    }

    #[test]
    fn settled_conditions_need_no_solver() {
        let Ok(mut solver) = Solver::start(5000) else {
            return;
        };
        let mut pool = Pool::default();
        let (_, wraps) = wraps(&mut pool);
        let never = pool.boolean(false);
        let (answer, _) = solver.check(&pool, &[wraps, never], &[]).expect("answers");
        assert_eq!(answer, Answer::Unsat);
        assert_eq!(solver.questions, 0);
    }

    #[test]
    fn says_why_a_solver_answers_alone() {
        let Ok(solver) = Solver::with(
            &[(Kind::Z3, "z3"), (Kind::Cvc5, "/nowhere/cvc5")],
            5000,
            false,
        ) else {
            return;
        };
        assert_eq!(solver.alone.as_deref(), Some("cvc5 isn't installed"));
        let error = Solver::with(&[(Kind::Z3, "/nowhere/z3")], 5000, false)
            .err()
            .expect("no solver");
        assert_eq!(error.to_string(), "z3 isn't installed");
    }

    /// A file named for the solver, in `folder`, that the system can run.
    fn a_program(folder: &Path, kind: Kind) -> PathBuf {
        std::fs::create_dir_all(folder).expect("a folder");
        let path = folder.join(kind.file());
        std::fs::write(&path, "").expect("a program");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("runnable");
        }
        path
    }

    /// A solver in the solvers folder beside the server comes before one on PATH; without one
    /// there, PATH's first that can run is taken, and a relative folder on PATH is passed over.
    #[test]
    fn a_solver_beside_the_server_comes_before_path() {
        let root = tempfile::tempdir().expect("a folder");
        let beside = root.path().join("plugin").join(SOLVERS_FOLDER);
        let first = root.path().join("first");
        let second = root.path().join("second");
        let on_path = a_program(&second, Kind::Z3);
        std::fs::create_dir_all(&first).expect("a folder");
        let path = std::env::join_paths([Path::new("relative"), first.as_path(), second.as_path()])
            .expect("a PATH");
        let (beside, path) = (beside.as_path(), path.as_os_str());
        assert_eq!(
            find_in(Some(beside), Some(path), Kind::Z3),
            Some(Found::OnPath(on_path.clone()))
        );
        let archived = a_program(beside, Kind::Z3);
        assert_eq!(
            find_in(Some(beside), Some(path), Kind::Z3),
            Some(Found::Beside(archived))
        );
        assert_eq!(find_in(Some(beside), Some(path), Kind::Cvc5), None);
        assert_eq!(find_in(None, None, Kind::Cvc5), None);
        assert_eq!(
            find_in(None, Some(path), Kind::Z3)
                .as_ref()
                .map(Found::path),
            Some(on_path.as_path())
        );
    }

    /// On PATH, as a shell looks, a file the system can't run is passed over.
    #[cfg(unix)]
    #[test]
    fn a_solver_on_path_must_be_able_to_run() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = tempfile::tempdir().expect("a folder");
        let stale = a_program(&root.path().join("stale"), Kind::Cvc5);
        std::fs::set_permissions(&stale, std::fs::Permissions::from_mode(0o644))
            .expect("not runnable");
        let fresh = a_program(&root.path().join("fresh"), Kind::Cvc5);
        let path = std::env::join_paths([root.path().join("stale"), root.path().join("fresh")])
            .expect("a PATH");
        assert_eq!(
            find_in(None, Some(path.as_os_str()), Kind::Cvc5),
            Some(Found::OnPath(fresh))
        );
    }

    /// A WebAssembly z3 comes first: the module its variable names, whether or not its file is
    /// there, so that a wrong one says so; else `z3.wasm` in the solvers folder, before a z3
    /// program there. cvc5, which has no WebAssembly build, takes no module.
    #[test]
    fn a_webassembly_z3_comes_before_a_program() {
        let root = tempfile::tempdir().expect("a folder");
        let folder = root.path().join("plugin").join(SOLVERS_FOLDER);
        let program = a_program(&folder, Kind::Z3);
        let beside = folder.as_path();
        assert_eq!(
            find_in(Some(beside), None, Kind::Z3),
            Some(Found::Beside(program))
        );
        let module = beside.join("z3.wasm");
        std::fs::write(&module, "").expect("a module");
        assert_eq!(
            find_in(Some(beside), None, Kind::Z3),
            Some(Found::ModuleBeside(module))
        );
        let elsewhere = root.path().join("elsewhere").join("z3.wasm");
        assert_eq!(
            named(Kind::Z3, Some(elsewhere.as_os_str())),
            Some(Found::ModuleNamed(elsewhere.clone()))
        );
        assert_eq!(named(Kind::Z3, Some(OsStr::new(""))), None);
        assert_eq!(named(Kind::Z3, None), None);
        assert_eq!(named(Kind::Cvc5, Some(elsewhere.as_os_str())), None);
        let cvc5 = a_program(beside, Kind::Cvc5);
        std::fs::write(beside.join("cvc5.wasm"), "").expect("a module that isn't cvc5's");
        assert_eq!(
            find_in(Some(beside), None, Kind::Cvc5),
            Some(Found::Beside(cvc5))
        );
        assert!(Found::ModuleNamed(elsewhere).is_module());
        assert!(!Found::OnPath(PathBuf::from("z3")).is_module());
    }

    /// The words of `command`: its program, then its arguments.
    fn words(command: &Command) -> Vec<String> {
        std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|word| word.to_string_lossy().into_owned())
            .collect()
    }

    /// A WebAssembly z3 starts in its host, given its module, the cache's folder when one is
    /// named, and after `--` z3's usual arguments; a program starts with the same arguments.
    #[test]
    fn a_webassembly_z3_starts_in_its_host_with_z3_s_arguments() {
        let hosted = Program::Wasm {
            host: PathBuf::from("lengua-mcp"),
            module: PathBuf::from("z3.wasm"),
            cache: Some(PathBuf::from("cache")),
        };
        assert_eq!(
            words(&command(Kind::Z3, &hosted, 20_000, false)),
            [
                "lengua-mcp",
                "--solver-wasm",
                "z3.wasm",
                "--cache",
                "cache",
                "--",
                "-in",
                "-smt2",
                "-memory:4096"
            ]
        );
        let native = Program::Native(OsString::from("z3"));
        assert_eq!(
            words(&command(Kind::Z3, &native, 20_000, false)),
            ["z3", "-in", "-smt2", "-memory:4096"]
        );
        // The prover leaves where the compiled module is kept to the host.
        let found = Found::ModuleNamed(PathBuf::from("z3.wasm")).program();
        let Program::Wasm { module, cache, .. } = &found else {
            panic!("{found:?} is no WebAssembly solver");
        };
        assert_eq!((module.as_path(), cache), (Path::new("z3.wasm"), &None));
    }

    /// A WebAssembly z3 whose module isn't there says so, rather than leaving the prover to
    /// another solver.
    #[test]
    fn a_missing_webassembly_z3_says_so() {
        let hosted = Program::Wasm {
            host: PathBuf::from("lengua-mcp"),
            module: PathBuf::from("/nowhere/z3.wasm"),
            cache: None,
        };
        let error = Solver::with_programs(&[(Kind::Z3, hosted)], 5000, false)
            .err()
            .expect("no solver");
        assert_eq!(
            error.to_string(),
            "z3 couldn't start: its WebAssembly module /nowhere/z3.wasm isn't there"
        );
    }

    /// The version each solver prints is read from its first line that names one.
    #[test]
    fn a_solver_s_version_is_read_from_what_it_prints() {
        assert_eq!(version_of("Z3 version 4.16.0 - 64 bit\n"), Some("4.16.0"));
        assert_eq!(
            version_of(
                "This is cvc5 version 1.3.4 [git tag 1.3.4 branch HEAD]\r\ncompiled with GCC \
                 version 13.3.0\r\n"
            ),
            Some("1.3.4")
        );
        // cvc5 1.3.4's own first line, then the compiler's version, which isn't the solver's.
        assert_eq!(
            version_of(
                "cvc5 1.3.4 [git tag 1.3.4 branch HEAD]\ncompiled with GCC version 11.4.0\n"
            ),
            Some("1.3.4")
        );
        assert_eq!(version_of("compiled with GCC version 11.4.0\n"), None);
        assert_eq!(version_of("lengua-mcp 0.1.0\n"), None);
    }

    /// The versions lengua pins are the ones every runner installs and a release archive ships,
    /// as xtask/tools.tsv names them for each of the five targets.
    #[test]
    fn the_pinned_solvers_are_the_ones_ci_installs() {
        let tools = include_str!("../../xtask/tools.tsv");
        for kind in Kind::ALL {
            let rows: Vec<&str> = tools
                .lines()
                .filter(|line| line.split('\t').next() == Some(kind.name()))
                .collect();
            assert_eq!(rows.len(), 5, "{} for five targets: {rows:?}", kind.name());
            let release = format!("/{}-{}/", kind.name(), kind.pinned());
            for row in rows {
                assert!(row.contains(&release), "{row} isn't {release}");
            }
        }
    }
}
