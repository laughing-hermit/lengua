//! main.wat's path through `check`, `run` and the command line. An idea is an Attested
//! Computation (OKF v0.2 §10), compiled to main.wat, which lengua builds into one WebAssembly
//! module, checks against the WebAssembly dialect's rules, tests in the server's pool of workers
//! and runs in its own WASI host, the same way on every system. Once its tests pass, lengua's
//! WebAssembly prover proves each generated test in the background, within the proof's budget,
//! and lengua.lock records what it proved. At done, idea.md's `verified` list holds lengua's
//! entry for the idea and the code it checked, until a proof finds the code wrong, and
//! `lengua-mcp check`, the idea's attester, attests the folder on any computer, with no lock and
//! no key, proving it too. A folder that holds the native lengua's main.s, or an idea of its
//! type, is told where the native lengua went.
//!
//! An idea with a `- Module:` line names main.wasm instead, an outside module that a compiler
//! made from another language ([`outside`]): lengua validates and tests it, runs it in the same
//! host under the same grants, holds it to no rule, proves nothing of it, and says "trusted, not
//! proven" in every reply and before every run.

mod outside;
mod tools;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::watch;

use super::calls;
use super::parts;
use super::{
    BlockProof, CheckParams, CheckReport, Checking, ConnectionReport, FURTHER, Found, IdeaStage,
    LenguaServer, Model, Passed, RUN_SECONDS, RunParams, RunReport, Serving, ShellRun, Stage,
    VERIFIED_NOTE, Witness, already_serving, background_limits, check_wait, cost_line, counted,
    ended_report, idea_problems, keep_tested, may_reach, own_folder_problem, own_folders,
    proof_time, proven_detail, random_seed, reaches, read_optional, settled, state_for_run,
    wait_for, working_folder,
};
use crate::checker::{
    self,
    wasm::{Obligation, machine},
};
use crate::connect::Reach;
use crate::data_dir::DataDir;
use crate::expr;
use crate::graph;
use crate::hash::Sha256Hex;
use crate::idea::{self, Idea, Use};
use crate::lock::{self, CodeStamp, Key};
use crate::platform::{Cache, CacheHolds, Machine};
use crate::prove::{self, Verdict};
use crate::runner::{End, Limits};
use crate::state::{
    CODE_FILE, IDEA_FILE, NATIVE_CODE_FILE, OUTSIDE_FILE, ProgramFolder, State, compiled_for,
    compiled_module, tested_module, wrong_record,
};
use crate::testfile::{self, TestSuite};
use crate::testrun;
use crate::trust;
use crate::wasm::engine::{self, Setup};
use crate::wasm::job::{Connection, Made};
use crate::wasm::runner::{Here, On, Order, Source, run_once_checked};
use crate::wasm::text::{self, Built};
use crate::wasm::{Policy, Pool, Program, Spec};

/// The exit code `lengua-mcp run` gives a program that trapped: an abort's, 128 and SIGABRT's 6,
/// as WebAssembly runtimes give on a Unix, here on every system.
const TRAPPED: i32 = 134;

/// The most bytes of `stdin_file` `run` gives main.wat: its worker gets its standard input
/// whole, with the job.
const MAX_STDIN_FILE: u64 = 16 * 1024 * 1024;

/// The most bytes a file may hold, and the most a program may write to files in all, in a run
/// by the `run` tool: a test's limits in the temporary folder lengua makes for the run and
/// throws away after it, and none in a folder of the user's own (`own_folder`), whose files are
/// the user's to fill.
fn run_file_limits(own_folder: bool) -> (Option<u64>, Option<u64>) {
    if own_folder {
        (None, None)
    } else {
        (
            Some(crate::runner::TEST_FILE_LIMIT),
            Some(crate::wasm::host::limits::TEST_WRITTEN),
        )
    }
}

/// The most of what the rules leave to the proofs that main.wat's memory line names, each on its
/// line of main.wat; it counts the rest.
const SHOWN_OBLIGATIONS: usize = 4;

/// Where the native lengua went, as lengua says it of the native lengua's main.s and ideas.
const NATIVE_LENGUA: &str = "lengua compiles an idea to main.wat since 0.2.0, and the tag \
                             native-final keeps the native lengua";

/// What a reply says of main.wat's memory safety once its tests pass: what the rules settle for
/// every input, what they leave to the proofs, `obligations`, and what the proofs, `proofs`,
/// settle of that on every run they cover. The model the proofs run on holds every access inside
/// the program's memory and every division away from its traps, as the engine does, but follows
/// no region: whether an access the rules leave stays inside its region is as far as the tests
/// show it. A proof that found main.wat wrong comes first, with the obligations its inputs broke.
pub(super) fn memory_line(obligations: &[Obligation], proofs: &[BlockProof]) -> String {
    let rules = "the rules (M1 to M8) hold every address they can follow to its owner, and \
                 WebAssembly holds every access inside the program's own memory";
    if obligations.is_empty() {
        return format!(
            "{rules}; the rules follow every address and division of main.wat, so they leave \
             nothing to the proofs"
        );
    }
    let mut left: Vec<String> = obligations
        .iter()
        .take(SHOWN_OBLIGATIONS)
        .map(|obligation| match obligation.line {
            Some(line) => format!("main.wat line {line}: {}", obligation.what),
            None => obligation.what.clone(),
        })
        .collect();
    let unshown = obligations.len().saturating_sub(SHOWN_OBLIGATIONS);
    if unshown > 0 {
        left.push(format!("and {unshown} more"));
    }
    let proven: Vec<&str> = proofs
        .iter()
        .filter(|proof| {
            proof
                .detail
                .as_deref()
                .is_some_and(|detail| detail.starts_with("proven"))
        })
        .map(|proof| proof.test.trim_start_matches("generated "))
        .collect();
    let running = proofs.iter().any(|proof| proof.detail.is_none());
    let divisions = obligations.iter().any(|obligation| obligation.division);
    let accesses = obligations.iter().any(|obligation| !obligation.division);
    // What a proof that found main.wat wrong found comes first, so that no line calls an
    // obligation its inputs broke tested, not proven.
    let mut covered: Vec<String> = found_wrong(obligations, proofs).into_iter().collect();
    if !proven.is_empty() {
        let held = match (divisions, accesses) {
            (true, true) => "no division traps and every access stays inside the program's memory",
            (true, false) => "no division traps",
            (false, _) => "every access stays inside the program's memory",
        };
        let regions = if accesses {
            ", but the proofs follow no region, so whether the accesses the rules leave stay \
             inside their regions is tested, not proven"
        } else {
            ""
        };
        covered.push(format!(
            "on every run the proofs of {} cover, {held}{regions}",
            listed(&proven)
        ));
    } else if running {
        covered.push("the proofs check them on every run they cover".to_owned());
    } else if covered.is_empty() {
        covered.push("no proof covers a run, so they are tested, not proven".to_owned());
    }
    format!(
        "{rules}; they leave the proofs {}: {}; {}",
        counted(obligations.len(), "obligation"),
        left.join("; "),
        covered.join("; ")
    )
}

/// What the proofs of `proofs` that found main.wat wrong found of what the rules leave to them,
/// `obligations`: inputs where main.wat is wrong, as their lines say, and each obligation on
/// whose line a real run of those inputs trapped, which they break. None when no proof found
/// main.wat wrong.
fn found_wrong(obligations: &[Obligation], proofs: &[BlockProof]) -> Option<String> {
    let wrong: Vec<&BlockProof> = proofs.iter().filter(|proof| proof.wrong).collect();
    if wrong.is_empty() {
        return None;
    }
    let tests: Vec<&str> = wrong
        .iter()
        .map(|proof| proof.test.trim_start_matches("generated "))
        .collect();
    let mut broken: Vec<usize> = wrong
        .iter()
        .filter_map(|proof| proof.detail.as_deref().and_then(trapped_line))
        .filter(|line| {
            obligations
                .iter()
                .any(|obligation| obligation.line == Some(*line))
        })
        .collect();
    broken.sort_unstable();
    broken.dedup();
    let lines: Vec<String> = broken.iter().map(ToString::to_string).collect();
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    let fails = match lines.as_slice() {
        [] => String::new(),
        [line] => format!("; on them the obligation on main.wat line {line} fails"),
        _ => format!(
            "; on them the obligations on main.wat lines {} fail",
            listed(&lines)
        ),
    };
    Some(format!(
        "the proofs of {} found inputs where main.wat is wrong, as their lines say{fails}",
        listed(&tests)
    ))
}

/// The line of main.wat on which the real run a proof's `detail` describes trapped, in the
/// words the run's end has for it.
fn trapped_line(detail: &str) -> Option<usize> {
    let (_, after) = detail.rsplit_once("trapped at main.wat line ")?;
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Whether a run of `block` reads values none of its variables fixes: random bytes or clock
/// readings its test doesn't supply, which lengua's host gives a program whose idea uses them.
/// Then every value of the variables isn't every input, and a real run can't replay what a
/// proof found.
pub(super) fn host_values(idea: &Idea, block: &testfile::Generated) -> bool {
    (idea.uses.contains(&Use::RandomBytes) && block.random.is_none())
        || (idea.uses.contains(&Use::Clock) && block.clock.is_empty())
}

/// Runs the inputs a proof of `block` found, `values`, for real: main.wat's `program`, in
/// `pool`'s workers, reaching what `idea` allows, so a counterexample is reported only when
/// main.wat really is wrong on them, and a trap with its line of main.wat. A trap the model
/// found runs too: the engine traps there as well, or the model was wrong. Gives the proof's
/// line and whether main.wat is wrong; `host` says whether the run reads values a real run
/// can't be given ([`host_values`]).
pub(super) async fn replay(
    pool: &Pool,
    program: &Program,
    idea: &Idea,
    block: &testfile::Generated,
    values: BTreeMap<String, expr::Value>,
    host: bool,
) -> (String, bool) {
    let Witness {
        inputs,
        delivery,
        broken,
        described,
    } = Witness::of(block, values);
    let case = match block.case(inputs, crate::expect::now()) {
        Ok(case) => case,
        Err((_, field, error)) => {
            return (
                format!("tested, not proven: with {described}, {field} {error}"),
                false,
            );
        }
    };
    let ran =
        testrun::check_wasm_case(pool, program, &case, &idea.uses, &idea.network, delivery).await;
    settled(&described, broken.as_deref(), ran, host)
}

/// What `check` says of `folder` when it holds main.s, the native lengua's code, and no
/// main.wat: where the native lengua went, and how to go on. Of main.s, only that it is there is
/// read.
pub(super) fn native_code(folder: &ProgramFolder) -> Option<CheckReport> {
    if !folder.file(NATIVE_CODE_FILE).exists() || folder.file(CODE_FILE).exists() {
        return None;
    }
    Some(CheckReport::problems(
        "code_problems",
        vec![format!(
            "{folder} holds {NATIVE_CODE_FILE}, the native lengua's code, and no {CODE_FILE}: \
             {NATIVE_LENGUA}"
        )],
        format!(
            "Next: make {folder}/{IDEA_FILE} an Attested Computation with the write-idea skill if \
             it isn't one, compile it into {folder}/{CODE_FILE} with the compile skill, delete \
             {folder}/{NATIVE_CODE_FILE}, then call check with path {folder}."
        ),
    ))
}

/// What's wrong with `folder` when it holds main.s beside main.wat: main.s is the native lengua's
/// code, one code file too many, which the reply says to delete.
pub(super) fn both_code_files(folder: &ProgramFolder) -> Option<CheckReport> {
    if !folder.file(NATIVE_CODE_FILE).exists() || !folder.file(CODE_FILE).exists() {
        return None;
    }
    Some(CheckReport::problems(
        "code_problems",
        vec![format!(
            "{folder} holds both {NATIVE_CODE_FILE} and {CODE_FILE}: {NATIVE_CODE_FILE} is the \
             native lengua's code, and {NATIVE_LENGUA}: delete {NATIVE_CODE_FILE}"
        )],
        format!("Next: delete {folder}/{NATIVE_CODE_FILE}, then call check with path {folder}."),
    ))
}

/// Why `idea`, read from `text`, can't be checked when it is an idea of the native lengua's
/// type: lengua compiles an Attested Computation alone. The problem is on the line that says the
/// type.
pub(super) fn type_problem(text: &str, idea: &Idea) -> Option<String> {
    if idea.attested.is_some() {
        return None;
    }
    let place = text
        .lines()
        .position(|line| line.starts_with("type:"))
        .map_or_else(String::new, |at| format!("line {}: ", at + 1));
    Some(format!(
        "{place}{IDEA_FILE} is an idea of the native lengua, which compiled it to \
         {NATIVE_CODE_FILE}: {NATIVE_LENGUA}; make it an Attested Computation (`type: {}`) with \
         the write-idea skill",
        idea::ATTESTED_TYPE
    ))
}

/// Why an Attested Computation can't have its `- Threads:` line, when it has one: a WebAssembly
/// program is one thread until WebAssembly's threads are a standard.
pub(super) fn threads_problem(text: &str, idea: &Idea) -> Option<String> {
    idea.threads?;
    let place = text
        .lines()
        .position(|line| line.trim_start().starts_with("- Threads:"))
        .map_or_else(String::new, |at| format!("line {}: ", at + 1));
    Some(format!(
        "{place}a WebAssembly program runs as one thread until WebAssembly's threads are a \
         standard: remove the `- Threads:` line"
    ))
}

/// `names` in a list: `a, b and c`.
fn listed(names: &[&str]) -> String {
    match names.split_last() {
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

/// How wide this CPU's vectors go, which Cranelift compiles WebAssembly's 128-bit SIMD to; the
/// `machine` line and `lengua-mcp doctor` both say it.
pub(crate) fn simd_level(machine: &Machine) -> String {
    let cpu = machine.features;
    match machine.arch {
        "x86_64" => [
            (cpu.avx512f, "AVX-512"),
            (cpu.avx2, "AVX2"),
            (cpu.avx, "AVX"),
            (cpu.sse4_2, "SSE4.2"),
            (cpu.sse4_1, "SSE4.1"),
            (cpu.ssse3, "SSSE3"),
            (cpu.sse3, "SSE3"),
        ]
        .into_iter()
        .find(|(present, _)| *present)
        .map_or("SSE2", |(_, name)| name)
        .to_owned(),
        "aarch64" if cpu.sve > 0 => format!("NEON, and SVE of {} bytes", cpu.sve),
        "aarch64" => "NEON".to_owned(),
        other => format!("those of its {other} CPU"),
    }
}

/// The WebAssembly features beyond 1.0 that main.wat may use: the engine's, but reference types,
/// which the engine has on only for the tables of a C, C++ or Rust toolchain's module, and which
/// the dialect leaves out with every table.
fn dialect_features() -> Vec<&'static str> {
    engine::OPTIONAL_FEATURES
        .into_iter()
        .filter(|feature| *feature != "reference-types")
        .collect()
}

/// The machine in one line for main.wat: this computer and its cores, the CPU the engine compiles
/// main.wat for, with how wide its vectors go, the engine, with the WebAssembly features main.wat
/// may use, and the CPU's caches as the system describes them. No LLVM and no CPU target: one
/// module runs on every computer.
pub(super) fn machine_line(machine: &Machine) -> String {
    let cpu = machine.cpu_model.as_deref().map_or_else(
        || format!("its {} CPU", machine.arch),
        |model| format!("its CPU, {model},"),
    );
    format!(
        "{}-{}, {}; Wasmtime {} compiles main.wat with Cranelift for {cpu} whose vectors go up \
         to {}; main.wat is WebAssembly 1.0 with {}, and no tables or references; {}",
        machine.os,
        machine.arch,
        counted(machine.cores, "core"),
        engine::ENGINE_VERSION,
        simd_level(machine),
        listed(&dialect_features()),
        caches_text(&machine.caches),
    )
}

/// The CPU's caches in words, as the system describes them: each level that holds data, with its
/// size, and the size of its lines, which the compile skill's references/caches.md lays work out
/// for. Instruction caches aren't named, and neither is a level the system doesn't describe.
fn caches_text(caches: &[Cache]) -> String {
    let named: Vec<&Cache> = caches
        .iter()
        .filter(|cache| cache.holds != CacheHolds::Instructions)
        .collect();
    if named.is_empty() {
        return "the system describes none of the CPU's caches".to_owned();
    }
    let one_line = named.windows(2).all(|pair| pair[0].line == pair[1].line);
    let each: Vec<String> = named
        .iter()
        .map(|cache| {
            let holds = if cache.holds == CacheHolds::Data {
                " data"
            } else {
                ""
            };
            let line = match cache.line {
                Some(line) if !one_line => format!(" in {line}-byte lines"),
                _ => String::new(),
            };
            format!("L{}{holds} {}{line}", cache.level, bytes_text(cache.bytes))
        })
        .collect();
    let shown: Vec<&str> = each.iter().map(String::as_str).collect();
    let lines = match named.first().and_then(|cache| cache.line) {
        Some(line) if one_line => format!(", in {line}-byte lines"),
        _ => String::new(),
    };
    format!("the CPU's caches are {}{lines}", listed(&shown))
}

/// `bytes` in the largest unit that holds them whole: `48 KiB`, `2 MiB`, or bytes.
fn bytes_text(bytes: u64) -> String {
    const KIB: u64 = 1 << 10;
    const MIB: u64 = 1 << 20;
    if bytes >= MIB && bytes.is_multiple_of(MIB) {
        format!("{} MiB", bytes / MIB)
    } else if bytes >= KIB && bytes.is_multiple_of(KIB) {
        format!("{} KiB", bytes / KIB)
    } else {
        format!("{bytes} bytes")
    }
}

/// The modules `opened`, which a program calls, each kept in a file of its own for the pool's
/// workers.
fn called_modules(opened: &[calls::Opened]) -> Result<Vec<crate::wasm::CalledModule>, String> {
    opened
        .iter()
        .map(|opened| {
            crate::wasm::CalledModule::new(&opened.called.name, &opened.wasm).map_err(|error| {
                format!(
                    "lengua couldn't keep the module of {}: {error}",
                    opened.called.place()
                )
            })
        })
        .collect()
}

/// The next step after main.wat's own problems.
fn fix_code(folder: &ProgramFolder) -> String {
    format!(
        "Next: fix {folder}/{CODE_FILE} with the compile skill (the full rules are in its \
         references/power-of-10-webassembly.md), then call check with path {folder}."
    )
}

/// The seconds since 1970-01-01T00:00:00Z, now.
fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Writes `text` over the file at `path`, whole or not at all, with the file's permissions.
fn replace(path: &Path, text: &str) -> std::io::Result<()> {
    let folder = path
        .parent()
        .ok_or_else(|| std::io::Error::other("the file has no folder"))?;
    let permissions = std::fs::metadata(path)?.permissions();
    let mut file = tempfile::NamedTempFile::new_in(folder)?;
    std::io::Write::write_all(&mut file, text.as_bytes())?;
    file.as_file().set_permissions(permissions)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// Everything a checking job needs about the main.wat it tests and proves.
pub(super) struct WasmCode {
    /// The idea.
    pub(super) idea: Idea,
    /// Its tests.
    pub(super) suite: TestSuite,
    /// The module main.wat builds, kept for the workers and the proofs.
    pub(super) program: Arc<Program>,
    /// The module's SHA-256.
    module: Sha256Hex,
    /// The WebAssembly features the module uses beyond WebAssembly 1.0.
    features: Vec<String>,
    /// The deepest call path's locals and operands, as the rules count them.
    max_stack_bytes: u64,
    /// How many `;; unsafe:` blocks main.wat has.
    unsafe_blocks: usize,
    /// What the rules leave to the proofs.
    obligations: Vec<Obligation>,
    /// The parts main.wat takes functions from, as lengua.lock records them.
    parts: Vec<lock::PartStamp>,
    /// For an idea with a `- Secret:` line, what the check of the engine's machine code read on
    /// this CPU, in one line.
    machine_code: Option<String>,
    /// The modules main.wat calls, as its tests run with them and lengua.lock records them.
    calls: Vec<calls::Opened>,
    /// How full main.wat's functions are, as the reply says it ([`CheckReport::functions`]).
    pub(super) functions: Vec<String>,
    /// main.wat's named texts, as the reply says them ([`CheckReport::data`]).
    pub(super) data: Vec<String>,
    /// For an outside module, main.wasm, what every reply says of it; `None` for main.wat.
    outside: Option<Vec<String>>,
}

/// Why main.wat isn't ready for its tests: what's wrong, each on its line, with what the rules
/// read of it all the same, when they read it.
#[derive(Debug, Default)]
struct Refused {
    /// What's wrong.
    problems: Vec<String>,
    /// How full its functions are ([`CheckReport::functions`]).
    functions: Vec<String>,
    /// Its named texts ([`CheckReport::data`]).
    data: Vec<String>,
}

impl From<Vec<String>> for Refused {
    fn from(problems: Vec<String>) -> Self {
        Self {
            problems,
            ..Self::default()
        }
    }
}

/// A file's text from the bytes a read kept, `bytes`: `None` when the file wasn't there.
fn text_of(
    folder: &ProgramFolder,
    name: &str,
    bytes: Option<Vec<u8>>,
) -> Result<Option<String>, String> {
    bytes
        .map(|bytes| {
            String::from_utf8(bytes)
                .map_err(|_| format!("couldn't read {folder}/{name}: it isn't UTF-8 text"))
        })
        .transpose()
}

/// A program's code as the state's read kept it.
enum Code {
    /// main.wat's text.
    Text(String),
    /// main.wasm's bytes, an outside module, with what lengua says of it before it runs
    /// ([`crate::wasm::outside::words`]).
    Outside {
        /// The module.
        wasm: Vec<u8>,
        /// That it is trusted, not proven.
        words: String,
    },
}

impl Code {
    /// What lengua says before it runs an outside module; nothing of main.wat.
    fn words(&self) -> Option<&str> {
        match self {
            Self::Text(_) => None,
            Self::Outside { words, .. } => Some(words),
        }
    }
}

/// What `run` may run in a folder once its checks pass: main.wat as the state's read kept it,
/// what its idea grants, and the module check tested, by the hash lengua.lock records.
struct Runnable {
    /// The code, the very bytes whose hash the lock was held to.
    code: Code,
    /// What the idea lets the program use, from its `- Uses:` line.
    uses: Vec<Use>,
    /// What it may do on the network, from its `- Network:` line.
    network: idea::Network,
    /// The inputs its `- Secret:` line names: for an idea that names any, the machine code of
    /// the module is checked on this computer before it runs ([`keeps_secrets`]).
    secrets: Vec<idea::Source>,
    /// The modules its idea calls, each opened as check opened it and held to what lengua.lock
    /// records: what runs beside the program.
    calls: Vec<calls::Opened>,
    /// The SHA-256 of the module check tested.
    module: Sha256Hex,
    /// The user's key, which verified the lock: a run from a shell loads the module's compiled
    /// form under it ([`loaded_or_compiled`]).
    key: Key,
}

/// The operators of `built`, main.wat's build in `ready`, that take a secret, as the rules find
/// them, or why main.wat doesn't keep its idea's secrets.
fn secret_operators(
    folder: &ProgramFolder,
    ready: &Runnable,
    built: &Built,
) -> Result<Vec<checker::wasm::secret::Taken>, String> {
    let context = checker::wasm::Context {
        uses: &ready.uses,
        network: &ready.network,
        secrets: &ready.secrets,
    };
    // The secret rule reads main.wat: an outside module has none, and keeps no secret.
    let Code::Text(code) = &ready.code else {
        return Err(leaks(folder, &[outside::NO_SECRET.to_owned()]));
    };
    let report = checker::wasm::check(code, built, &context);
    if report.passed() {
        Ok(report.secret_operators)
    } else {
        Err(leaks(folder, &report.lines()))
    }
}

/// What `run` says of a program that doesn't keep its idea's secrets on this computer, for
/// `refused`, what the rules or the machine code's check found.
fn leaks(folder: &ProgramFolder, refused: &[String]) -> String {
    format!(
        "{IDEA_FILE} names a secret, and {CODE_FILE} doesn't keep it on this computer, so it \
         won't run: {}. Call check with path {folder}, then fix {CODE_FILE}.",
        refused.join("; ")
    )
}

/// For a program whose idea names secrets, checks the machine code the engine compiles `built`
/// to on this computer in `setup`, the configuration about to run it, as `check` does: the rules
/// find the operators that take a secret, and the code made for each keeps the secret rule
/// ([`machine`]). So the code of every run is checked on the CPU that runs it, whichever
/// computer checked the folder. A program whose idea names no secret is checked for nothing,
/// and nothing is compiled for it here.
fn keeps_secrets(
    folder: &ProgramFolder,
    ready: &Runnable,
    built: &Built,
    setup: Setup,
) -> Result<(), String> {
    if ready.secrets.is_empty() {
        return Ok(());
    }
    let operators = secret_operators(folder, ready, built)?;
    match machine::check(&built.wasm, &operators, &[setup]) {
        Ok(_) => Ok(()),
        Err(refused) => {
            let refused: Vec<String> = refused.iter().map(ToString::to_string).collect();
            Err(leaks(folder, &refused))
        }
    }
}

/// [`keeps_secrets`] for a run from a shell, over `compiled`, the compiled module the run has
/// loaded and is about to execute: the rules find the operators that take a secret in main.wat's
/// build, which must be the module check tested, and the code `compiled` holds for each keeps
/// the secret rule. The code read is the code that runs, not another compilation of the module.
fn loaded_keeps_secrets(
    folder: &ProgramFolder,
    ready: &Runnable,
    compiled: &wasmtime::Module,
) -> Result<(), String> {
    if ready.secrets.is_empty() {
        return Ok(());
    }
    let built = built_as_tested(folder, ready)?;
    let operators = secret_operators(folder, ready, &built)?;
    match machine::check_loaded(compiled, &operators) {
        Ok(_) => Ok(()),
        Err(refused) => {
            let refused: Vec<String> = refused.iter().map(ToString::to_string).collect();
            Err(leaks(folder, &refused))
        }
    }
}

/// Checks that `folder` holds main.wat lengua may run, as `run` does from MCP and from a shell:
/// its idea and code match lengua.lock, and no proof the lock records found the module check
/// tested wrong. Environment variables reach only a program whose idea uses the environment.
/// Gives what may run; the bytes that run are held to its module hash, by the MCP tool's build
/// ([`built_as_tested`]) or by a shell's run ([`kept_or_built`]). idea.md and main.wat are read
/// once, by the state's own read: what is built and compared, and the idea's grants, come from
/// the very bytes whose hashes the lock was held to. Of the idea, a run reads only its grants
/// ([`idea::grants`]): check judged the rest of this very text.
fn runnable(
    data_dir: &DataDir,
    folder: &ProgramFolder,
    env: &BTreeMap<String, String>,
) -> Result<Runnable, String> {
    let (state, snapshot, files, key) = state_for_run(data_dir, folder)?;
    if state != State::Ready {
        // The state's next step names main.wat: an outside module's is check's to say.
        let next = if snapshot.outside {
            format!("Next: call check with path {folder}.")
        } else {
            state.next_step(folder)
        };
        return Err(format!(
            "lengua runs a program only when its idea and code match lengua.lock, and {folder} \
             doesn't. {next}"
        ));
    }
    // The lock the state's read verified with lengua's key: a run reads and verifies it once.
    let lock = snapshot
        .valid_lock()
        .ok_or("lengua.lock is missing; call check")?;
    let key = key.ok_or("lengua.lock is missing; call check")?;
    let stamp = lock
        .code
        .as_ref()
        .ok_or_else(|| format!("{CODE_FILE} isn't checked; call check with path {folder}."))?;
    // The parts main.wat takes from, as the lock records them: a part that changed since is no
    // longer the part check held main.wat to.
    if let Some(problem) = parts::changed_since(folder, &stamp.parts) {
        return Err(problem);
    }
    // What a proof found wrong outlasts lengua's restarts: lengua.lock records it.
    if let Some(proofs) = &lock.proofs
        && proofs.module_sha256 == stamp.module_sha256
        && !proofs.wrong.is_empty()
    {
        let wrong: Vec<String> = proofs
            .wrong
            .iter()
            .map(|(id, found)| format!("generated {id}: {found}"))
            .collect();
        return Err(format!(
            "The proof found inputs where {CODE_FILE} is wrong, so it won't run: {}. Call \
             check with path {folder} to see them, then fix {CODE_FILE}.",
            wrong.join("; ")
        ));
    }
    // And an earlier lock put back in the folder: what a proof found wrong about this module
    // of this idea is kept in the data folder too.
    let kept = wrong_record(data_dir.path(), &lock.idea.sha256, &stamp.module_sha256);
    if let Ok(found) = std::fs::read_to_string(&kept) {
        let wrong: Vec<&str> = found.lines().collect();
        return Err(format!(
            "The proof found inputs where {CODE_FILE} is wrong, so it won't run: {}. \
             lengua.lock doesn't record that, as when an earlier lock was put back, but lengua \
             keeps it. Call check with path {folder}, then fix {CODE_FILE}.",
            wrong.join("; ")
        ));
    }
    let idea_text = text_of(folder, IDEA_FILE, files.idea)?.ok_or_else(|| {
        format!("There's no {IDEA_FILE} in {folder}. Write it with the write-idea skill first.")
    })?;
    let (uses, network) =
        idea::grants(&idea_text).map_err(|problems| idea_problems(folder, &problems))?;
    let secrets = idea::secrets(&idea_text).map_err(|problems| idea_problems(folder, &problems))?;
    // Which file the code is comes from the idea, as the state's read took it from these very
    // bytes: main.wasm for an idea with a `- Module:` line, whose stamp must be an outside
    // module's too, so that nothing checked as one kind runs as the other.
    let named = idea::outside(&idea_text).map_err(|problems| idea_problems(folder, &problems))?;
    // The modules the idea calls, opened as check opened them: an outside module read again,
    // a called program with its lock verified again, and each the module the tests ran with.
    let called = calls::as_stamped(data_dir, folder, &network.reach.calls, &stamp.calls)?;
    if named.is_some() != snapshot.outside || named.is_some() != stamp.outside() {
        return Err(format!(
            "lengua.lock doesn't record this kind of code for {folder}. Call check with path \
             {folder}."
        ));
    }
    let code = match named {
        None => Code::Text(text_of(folder, CODE_FILE, files.code)?.ok_or_else(|| {
            format!("There's no {CODE_FILE} in {folder}. Write it with the compile skill.")
        })?),
        Some(named) => {
            if !secrets.is_empty() {
                return Err(format!("{}: it won't run.", outside::NO_SECRET));
            }
            Code::Outside {
                wasm: files
                    .code
                    .ok_or_else(|| format!("There's no {OUTSIDE_FILE} in {folder}. Call check."))?,
                words: crate::wasm::outside::words(&named.built_from),
            }
        }
    };
    if !env.is_empty() && !uses.contains(&Use::Environment) {
        return Err(format!(
            "{IDEA_FILE} doesn't use the environment, so the program gets no environment \
             variables; leave env out, or add `the environment` to its `- Uses:` line"
        ));
    }
    Ok(Runnable {
        code,
        uses,
        network,
        secrets,
        calls: called,
        module: stamp.module_sha256.clone(),
        key,
    })
}

/// The module main.wat in `ready` builds, provided it is byte for byte the module check tested:
/// another build, as after lengua itself changed, doesn't run.
fn built_as_tested(folder: &ProgramFolder, ready: &Runnable) -> Result<Built, String> {
    let code = match &ready.code {
        Code::Text(code) => code,
        // An outside module is its own build: held to the module check tested by its hash, and
        // read again as check read it, with what its idea grants.
        Code::Outside { wasm, .. } => return outside::as_tested(folder, ready, wasm),
    };
    let built =
        text::build(code).map_err(|problem| format!("{CODE_FILE} doesn't build: {problem}"))?;
    if Sha256Hex::of(&built.wasm) != ready.module {
        return Err(format!(
            "This build of {CODE_FILE} differs from the module check tested, as after lengua \
             itself changed, so it won't run. Call check with path {folder} to test this build."
        ));
    }
    Ok(built)
}

/// The module a run from a shell compiles for `ready` when no compiled form of it is kept: the
/// one check kept in this build's folder of the data folder, when its bytes hash to the module
/// check tested, with no build of main.wat; otherwise, missing, unreadable or other bytes,
/// main.wat built from the bytes the state's read kept, provided the build is that very module
/// ([`built_as_tested`]), and kept again for the next run, which never fails for a module it
/// couldn't keep.
fn kept_or_built(
    data_dir: &DataDir,
    folder: &ProgramFolder,
    ready: &Runnable,
) -> Result<Vec<u8>, String> {
    let kept = tested_module(data_dir.path(), text::BUILDER, &ready.module);
    if let Ok(wasm) = std::fs::read(&kept)
        && Sha256Hex::of(&wasm) == ready.module
    {
        return Ok(wasm);
    }
    let built = built_as_tested(folder, ready)?;
    let _ = keep_tested(data_dir, &ready.module, &built.wasm);
    Ok(built.wasm)
}

/// Why a run from a shell didn't happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotRun {
    /// lengua won't run the program, or can't: why, for the user.
    Refused(String),
    /// This process only runs what was compiled before (lengua-run), and the module check
    /// tested has no compiled form it can load, as it says: none is kept yet, the one kept
    /// doesn't carry the user's seal, or the engine doesn't take it on this computer.
    /// lengua-mcp compiles it, keeps it and runs it.
    NotCompiled(String),
    /// This process only runs what was compiled before, and the idea names a secret: the
    /// machine code is read on every run, with main.wat's rules, which lengua-mcp does.
    Secret,
    /// This process only runs what was compiled before, and the run asks for a tool that takes
    /// the compiler, as it says: a profile, whose engine has a clock that moves, or a stop,
    /// which runs a copy of main.wat built for the run.
    Tool(String),
}

impl From<String> for NotRun {
    fn from(why: String) -> Self {
        Self::Refused(why)
    }
}

/// Compiles `wasm`, the module whose SHA-256 is `module`, for a run from a shell on this
/// computer, seals it with `key`, and keeps it in the data folder ([`compiled_module`]), written
/// atomically: what later runs load in place of compiling ([`compile_and_keep_for`]).
///
/// # Errors
///
/// When the engine doesn't compile the module, or this process has no compiler.
pub fn compile_and_keep(
    data_dir: &DataDir,
    key: &Key,
    module: &Sha256Hex,
    wasm: &[u8],
) -> Result<Vec<u8>, String> {
    compile_and_keep_for(Setup::Shell, data_dir, key, module, wasm)
}

/// Compiles `wasm`, the module whose SHA-256 is `module`, for `setup` on this computer, a run
/// from a shell's configuration or the `run` tool's, seals it with `key` as that module compiled
/// for that configuration, and keeps it in the data folder ([`compiled_for`]), written
/// atomically: what later runs load in place of compiling. Gives the sealed bytes, kept or not:
/// a run never fails for a compiled module it couldn't keep. The engine that compiles has no
/// cache: what is sealed is what its compiler made in this call.
///
/// # Errors
///
/// When the engine doesn't compile the module, or this process has no compiler.
pub(crate) fn compile_and_keep_for(
    setup: Setup,
    data_dir: &DataDir,
    key: &Key,
    module: &Sha256Hex,
    wasm: &[u8],
) -> Result<Vec<u8>, String> {
    use std::io::Write as _;
    let compiler = engine::engine(setup)?;
    let sealed = key.compile_sealed_for(setup, &compiler, module, wasm)?;
    let path = compiled_for(data_dir.path(), text::BUILDER, module, setup);
    if let Some(folder) = path.parent()
        && std::fs::create_dir_all(folder).is_ok()
        && let Ok(mut file) = tempfile::NamedTempFile::new_in(folder)
        && file.write_all(&sealed).is_ok()
    {
        let _ = file.persist(&path);
    }
    Ok(sealed)
}

/// The compiled module a run from a shell runs, in the engine that only runs: the one kept in
/// the data folder for `module`, the SHA-256 of the module check tested, once it carries `key`'s
/// seal as that module's and the engine takes it ([`Key::load_sealed`]). No file that isn't
/// sealed so is ever handed to the engine. Where there is none to load, a process that compiles
/// takes `wasm`'s bytes, which the caller holds to that hash, compiles them, seals and keeps the
/// result, and loads it as every later run will; a process that only runs says why it can't.
pub(crate) fn loaded_or_compiled(
    data_dir: &DataDir,
    key: &Key,
    module: &Sha256Hex,
    wasm: impl FnOnce() -> Result<Vec<u8>, String>,
) -> Result<wasmtime::Module, NotRun> {
    let runner = engine::runner()?;
    let path = compiled_module(data_dir.path(), text::BUILDER, module);
    let why = match std::fs::read(&path) {
        Ok(sealed) => match key.load_sealed(&runner, module, &sealed) {
            Ok(loaded) => return Ok(loaded),
            Err(unsealed) => unsealed.to_string(),
        },
        Err(_) => "it isn't compiled for this computer yet".to_owned(),
    };
    if !engine::compiles() {
        return Err(NotRun::NotCompiled(why));
    }
    let sealed = compile_and_keep(data_dir, key, module, &wasm()?)
        .map_err(|error| format!("the engine couldn't compile the module: {error}"))?;
    key.load_sealed(&runner, module, &sealed)
        .map_err(|unsealed| {
            NotRun::Refused(format!("lengua couldn't load what it compiled: {unsealed}"))
        })
}

/// The bytes of `name`, a file inside `folder`, as a run's standard input: at most
/// [`MAX_STDIN_FILE`], since main.wat's worker gets them with the job. The file is opened once,
/// beneath the folder, where cap-std resolves every name of its path, so no name or link leads
/// out; what is then looked at and read is the file that opened, whatever takes its name
/// afterwards. A name that isn't a regular file is refused before anything opens, so a named
/// pipe isn't waited on.
fn read_stdin_file(folder: &Path, name: &str) -> Result<Vec<u8>, String> {
    use std::io::Read as _;
    let unreadable = |error: std::io::Error| format!("stdin_file {name}: {error}");
    let outside = || format!("stdin_file {name} must be a file inside folder");
    // Why a path didn't open: nothing has the name, or it isn't beneath the folder.
    let unopened = |error: std::io::Error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            unreadable(error)
        } else {
            outside()
        }
    };
    let large = |size: u64| {
        format!(
            "stdin_file {name} holds {size} bytes, and main.wat's standard input from a file \
             holds at most {} MiB: give it a smaller file",
            MAX_STDIN_FILE / 1024 / 1024
        )
    };
    let root = cap_std::fs::Dir::open_ambient_dir(folder, cap_std::ambient_authority())
        .map_err(unreadable)?;
    if !root.metadata(name).map_err(unopened)?.is_file() {
        return Err(outside());
    }
    let file = root.open(name).map_err(unopened)?;
    let opened = file.metadata().map_err(unreadable)?;
    if !opened.is_file() {
        return Err(outside());
    }
    if opened.len() > MAX_STDIN_FILE {
        return Err(large(opened.len()));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    file.take(MAX_STDIN_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
    // A file that grew past the most while it was read.
    let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if read > MAX_STDIN_FILE {
        return Err(large(read));
    }
    Ok(bytes)
}

/// How lengua makes the connection `reach` for a run from a shell, given `args`: a place, or
/// none, handed over ended, with why.
fn made(reach: &Reach, args: &[String]) -> Made {
    let resolved = crate::connect::resolved(reach, args);
    let place = resolved.as_ref().unwrap_or(reach).describe();
    let connection = match resolved {
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
        Ok(Reach::Argument { .. } | Reach::Peer | Reach::Nowhere) => {
            Connection::Ended("nothing listens there".to_owned())
        }
        Err(problem) => Connection::Ended(problem),
    };
    Made { place, connection }
}

/// What a run from a shell runs once its checks pass: the module check tested, compiled, and
/// what the program is given, all from the bytes the checks read.
struct Checked<'r> {
    /// The run, as the shell asked for it.
    request: &'r ShellRun,
    /// The folder it runs in.
    workspace: PathBuf,
    /// main.wat, what its idea grants, and the module check tested, by its hash.
    ready: Runnable,
    /// That module, compiled, in the engine that runs it.
    compiled: wasmtime::Module,
    /// The modules its idea calls, each compiled, in that same engine.
    calls: Vec<crate::wasm::host::call::Loaded>,
    /// The connections lengua makes for it, in order.
    connections: Vec<Made>,
    /// lengua's own folders, where no file or folder an argument names may lie.
    own: Vec<PathBuf>,
    /// The tools the run is asked for, with what their reports need.
    equipment: Option<crate::wasm::tools::Equipment>,
}

impl Order for Checked<'_> {
    fn module(&self) -> &wasmtime::Module {
        &self.compiled
    }

    fn calls(&self) -> &[crate::wasm::host::call::Loaded] {
        &self.calls
    }

    fn equipment(&self) -> Option<&crate::wasm::tools::Equipment> {
        self.equipment.as_ref()
    }

    fn here(&self) -> Here<'_> {
        let network = &self.ready.network;
        Here {
            args: &self.request.args,
            env: &self.request.env,
            folder: &self.workspace,
            policy: Policy::of(&self.ready.uses, network),
            listen: network.listens.map(|port| (port, network.serves_anywhere)),
            connections: &self.connections,
            own: &self.own,
            // main.wat is built only if the program traps, to place the trap on its line; an
            // outside module has no line to place one on.
            source: match &self.ready.code {
                Code::Text(text) => Source::Text {
                    text,
                    module: &self.ready.module,
                },
                Code::Outside { .. } => Source::Unknown,
            },
        }
    }
}

/// A run from a shell of main.wat: `run`'s checks, in the program folder and the working folder
/// `folders` gives, then the module check tested, in this process, attached to the shell's
/// standard input, output and error, with no time limit, on the thread `on` says. What runs is
/// the compiled form of that module kept under the user's key ([`loaded_or_compiled`]); a process
/// that compiles makes it where there is none, from the module check kept or main.wat built
/// again
/// ([`kept_or_built`]). For an idea that names a secret, the machine code about to run is read
/// first ([`loaded_keeps_secrets`]). Nothing of the program is instantiated or run until every
/// check has passed. Gives the program's exit code, or [`TRAPPED`] when it trapped, which it
/// says on standard error.
///
/// # Errors
///
/// Why lengua won't run it, or can't, or why this process, which only runs, leaves it to
/// lengua-mcp.
pub(super) fn run_from_shell(
    data_dir: &DataDir,
    request: &ShellRun,
    on: On,
    folders: impl FnOnce() -> Result<(ProgramFolder, PathBuf), String>,
) -> Result<i32, NotRun> {
    let ended = run_once_checked(on, || -> Result<Checked<'_>, NotRun> {
        let (folder, workspace) = folders()?;
        let ready = runnable(data_dir, &folder, &request.env)?;
        // The working folder of a program that uses files is never one of lengua's own or the
        // system's: said before anything is built or compiled.
        if ready.uses.contains(&Use::Files)
            && let Some(problem) = own_folder_problem(data_dir, &workspace)
        {
            return Err(NotRun::Refused(problem));
        }
        // An idea that names a secret: reading the machine code takes main.wat's rules, which
        // a process that only runs leaves to lengua-mcp.
        if !ready.secrets.is_empty() && !engine::compiles() {
            return Err(NotRun::Secret);
        }
        // A profile or a stop runs a module compiled here for the one run, and a trace the
        // module every run loads.
        let mut equipment = tools::for_shell(&request.tools, &folder, &ready)?;
        let asked_for = equipment.as_mut().and_then(|(_, module)| module.take());
        let for_this_run = asked_for.is_some();
        let compiled = match asked_for {
            Some(module) => module,
            None => loaded_or_compiled(data_dir, &ready.key, &ready.module, || {
                kept_or_built(data_dir, &folder, &ready)
            })?,
        };
        loaded_keeps_secrets(&folder, &ready, &compiled)?;
        // The modules the idea calls, each in the engine the program runs in: the compiled
        // form kept under the user's seal, loaded as the program's own is; or, where the
        // program's module was compiled here for this one run, for a profile or a stop,
        // compiled here beside it.
        let mut calls = Vec::with_capacity(ready.calls.len());
        for called in &ready.calls {
            let module = if for_this_run {
                engine::compile(compiled.engine(), &called.wasm).map_err(|error| {
                    NotRun::Refused(format!(
                        "the engine couldn't compile {}, a module the program calls: {error:#}",
                        called.called.place()
                    ))
                })?
            } else {
                loaded_or_compiled(data_dir, &ready.key, &called.module, || {
                    Ok(called.wasm.clone())
                })?
            };
            calls.push(crate::wasm::host::call::Loaded {
                name: called.called.name.clone(),
                module,
            });
        }
        let connections = reaches(&ready.network)
            .iter()
            .map(|reach| made(reach, &request.args))
            .collect();
        // lengua's own folders are found only for an idea that lets its program open what an
        // argument names: no other run pays for the look.
        let own = if ready.network.reach.files.is_empty() {
            Vec::new()
        } else {
            own_folders(data_dir)
        };
        // Every check has passed: an outside module is said to be one before it starts.
        if let Some(words) = ready.code.words() {
            eprintln!("{}: {words}", engine::name());
        }
        Ok(Checked {
            request,
            workspace,
            ready,
            compiled,
            calls,
            connections,
            own,
            equipment: equipment.map(|(equipment, _)| equipment),
        })
    })?;
    let lengua = engine::name();
    match ended.map_err(|error| format!("couldn't run the program: {error}"))? {
        End::Exited(code) => Ok(code),
        trapped @ End::Trapped(..) => {
            eprintln!(
                "{lengua}: the program {}",
                trapped.describe(Limits::default())
            );
            Ok(TRAPPED)
        }
        other => Err(NotRun::Refused(other.describe(Limits::default()))),
    }
}

impl LenguaServer {
    /// `check`'s stages for an Attested Computation, `idea_text`, up to the first that needs
    /// work: the idea stage; main.s, the native lengua's code, beside main.wat or in its place;
    /// main.wat built, held to its parts and checked against every rule; its tests, then its
    /// proofs, in the background; and at done, lengua's entry in idea.md's `verified`, which every
    /// check takes away first once the idea or the code no longer is what it names
    /// ([`Self::keep_verified`]), and a proof that finds main.wat wrong takes away at once
    /// ([`Self::unverify`]).
    pub(super) async fn check_wasm(
        &self,
        folder: &ProgramFolder,
        params: &CheckParams,
        events: &mut Vec<graph::Event>,
        (began, machine): (Instant, &Machine),
        idea_text: &str,
    ) -> Result<CheckReport, String> {
        let (idea, suite) = match self.idea_stage(folder, params, events, idea_text).await? {
            IdeaStage::Recorded(recorded) => *recorded,
            IdeaStage::Stops(report) => return Ok(*report),
        };
        // main.s waits until the idea is recorded, with the user's words, so the next check, once
        // main.wat is there, needs no words again.
        if let Some(report) = both_code_files(folder).or_else(|| native_code(folder)) {
            return Ok(report);
        }
        // An idea with a `- Module:` line: its program is main.wasm, an outside module.
        if idea.outside.is_some() {
            return self
                .check_outside(folder, events, began, idea_text, idea, suite)
                .await;
        }
        let Some(code) = read_optional(folder, CODE_FILE)? else {
            return Ok(CheckReport {
                machine: Some(machine_line(machine)),
                ..CheckReport::new("write_code", State::CodeMissing.next_step(folder))
            });
        };
        let idea_hash = trust::idea_hash(idea_text.as_bytes());
        let files = [idea_hash, Sha256Hex::of(code.as_bytes())];
        let prove_further = params.prove_further;
        let builds_on = idea.parts.clone();
        let called = idea.network.reach.calls.clone();
        // A job on these very files stands only while the lock records the parts as they are
        // now: after a part was checked again, the code is stamped again with it.
        let current = match self.current_checking(folder, &files, prove_further).await {
            Some(checking)
                if self.parts_as_stamped(folder, &builds_on, &code)
                    && self.calls_as_stamped(folder, &called, &code) =>
            {
                Some(checking)
            }
            _ => None,
        };
        let checking = match current {
            Some(checking) => checking,
            None => match self.wasm_code(folder, idea_text, &code, idea, suite) {
                Ok(code) => {
                    self.start_wasm_checking(folder, files, code, prove_further)
                        .await
                }
                Err(refused) => {
                    return Ok(CheckReport {
                        functions: refused.functions,
                        data: refused.data,
                        ..CheckReport::problems("code_problems", refused.problems, fix_code(folder))
                    });
                }
            },
        };
        // The parts as they are now: a job on these very files may be older than a change to a
        // part, and main.wat's copy must be the part's own at every check.
        let taken = match parts::taken(&self.data_dir, folder, &builds_on, &code) {
            Ok(taken) => taken,
            Err(problems) => {
                return Ok(CheckReport::problems(
                    "code_problems",
                    problems,
                    fix_code(folder),
                ));
            }
        };
        // The modules it calls as they are now, for the same reason: a job on these very files
        // may be older than a change to one of them.
        let opened = match calls::open(&self.data_dir, folder, &called) {
            Ok(opened) => opened,
            Err(problems) => {
                return Ok(CheckReport::problems(
                    "code_problems",
                    problems,
                    fix_code(folder),
                ));
            }
        };
        wait_for(&checking, began + check_wait()).await;
        if *checking.stage.borrow() == Stage::Ended {
            self.verify_once_passed(folder, &checking).await;
        }
        let mut report = self.checking_report(folder, &checking);
        // lengua's entry as the reply says done, though proofs may still be running. When it
        // doesn't go in, as when a proof has just found main.wat wrong, the reply says what holds
        // now, and claims no entry.
        if report.state == "done" && !self.write_verified(folder, &checking).await {
            report = self.checking_report(folder, &checking);
            report.next_step = report.next_step.replace(VERIFIED_NOTE, "");
        }
        report.functions.clone_from(&checking.functions);
        report.data.clone_from(&checking.data);
        report.parts = parts::lines(&taken);
        report.calls = calls::lines(&opened);
        if matches!(report.state.as_str(), "done" | "proving")
            && let Some(note) = calls::trusted_note(&opened)
        {
            report.next_step.push_str(&note);
        }
        report.contract = parts::contract_lines(idea_text);
        let proven_wrong = checking.found.lock().is_ok_and(|found| found.wrong);
        if report.state == "done" {
            events.push(graph::Event::Done);
        } else if proven_wrong {
            events.push(graph::Event::ProvenWrong);
        }
        Ok(report)
    }

    /// Whether lengua.lock records the parts main.wat, `code`, takes from as they are now, or
    /// doesn't record this main.wat yet, or main.wat doesn't hold to its parts, which the check that
    /// follows says.
    fn parts_as_stamped(
        &self,
        folder: &ProgramFolder,
        builds_on: &[idea::Part],
        code: &str,
    ) -> bool {
        let Ok(taken) = parts::taken(&self.data_dir, folder, builds_on, code) else {
            return true;
        };
        let stamped = Key::load(&self.data_dir)
            .ok()
            .flatten()
            .and_then(|key| lock::read(folder.path(), Some(&key)).ok().flatten())
            .and_then(|lock| lock.code);
        let current = Sha256Hex::of(code.as_bytes());
        stamped
            .filter(|stamp| stamp.sha256 == current)
            .is_none_or(|stamp| stamp.parts == parts::stamps(&taken))
    }

    /// Whether lengua.lock records the modules main.wat, `code`, calls as they are now, or
    /// doesn't record this main.wat yet, or a called module isn't one lengua takes, which the
    /// check that follows says.
    fn calls_as_stamped(
        &self,
        folder: &ProgramFolder,
        called: &[crate::call::Called],
        code: &str,
    ) -> bool {
        if called.is_empty() {
            return true;
        }
        let Ok(opened) = calls::open(&self.data_dir, folder, called) else {
            return true;
        };
        let stamped = Key::load(&self.data_dir)
            .ok()
            .flatten()
            .and_then(|key| lock::read(folder.path(), Some(&key)).ok().flatten())
            .and_then(|lock| lock.code);
        let current = Sha256Hex::of(code.as_bytes());
        stamped
            .filter(|stamp| stamp.sha256 == current)
            .is_none_or(|stamp| stamp.calls == calls::stamps(&opened))
    }

    /// main.wat's `code` built, held to its parts and checked against every rule of the
    /// WebAssembly dialect, ready for its tests; or what's wrong, each on its line.
    fn wasm_code(
        &self,
        folder: &ProgramFolder,
        idea_text: &str,
        code: &str,
        idea: Idea,
        suite: TestSuite,
    ) -> Result<WasmCode, Refused> {
        let built = text::build(code).map_err(|problem| vec![problem.to_string()])?;
        let taken = parts::taken(&self.data_dir, folder, &idea.parts, code)?;
        let opened = calls::open(&self.data_dir, folder, &idea.network.reach.calls)?;
        let context = checker::wasm::Context {
            uses: &idea.uses,
            network: &idea.network,
            secrets: &idea.secrets,
        };
        let report = checker::wasm::check(code, &built, &context);
        // What the reply says of main.wat's size and its named texts, whether or not it passes.
        let functions = report.size_lines();
        let data = report.texts.lines();
        if !report.passed() {
            return Err(Refused {
                problems: report.lines(),
                functions,
                data,
            });
        }
        // A part's contract, held to what the rules found of the functions it offers.
        let stated = parts::contract_problems(idea_text, code, &report.lends);
        if !stated.is_empty() {
            return Err(Refused {
                problems: stated,
                functions,
                data,
            });
        }
        // An idea with a `- Secret:` line: the code the engine compiles the module to on this
        // CPU, as `run` and a run from a shell compile it, keeps the secret rule too. An idea
        // with none compiles nothing here.
        let machine_code = if idea.secrets.is_empty() {
            None
        } else {
            let passed = machine::check(
                &built.wasm,
                &report.secret_operators,
                &[Setup::Run, Setup::Shell],
            )
            .map_err(|refused| refused.iter().map(ToString::to_string).collect::<Vec<_>>())?;
            Some(passed.line())
        };
        let features = engine::features_used(&built.wasm);
        let module = Sha256Hex::of(&built.wasm);
        let program = Program::new(built)
            .map_err(|error| vec![format!("lengua couldn't keep main.wat's module: {error}")])?
            .with_calls(called_modules(&opened).map_err(|problem| vec![problem])?);
        Ok(WasmCode {
            idea,
            suite,
            program: Arc::new(program),
            module,
            features,
            max_stack_bytes: report.facts.max_stack_bytes,
            unsafe_blocks: report.unsafe_blocks.len(),
            obligations: report.facts.obligations.clone(),
            parts: parts::stamps(&taken),
            calls: opened,
            machine_code,
            functions,
            data,
            outside: None,
        })
    }

    /// Starts testing and proving `code` in the background, stopping any job on older files;
    /// with `further`, the proof goes on from where lengua.lock says it stopped short.
    async fn start_wasm_checking(
        &self,
        folder: &ProgramFolder,
        files: [Sha256Hex; 2],
        code: WasmCode,
        further: bool,
    ) -> Arc<Checking> {
        let proofs: Vec<BlockProof> = code
            .suite
            .generated
            .iter()
            .map(|block| BlockProof {
                test: format!("generated {}", block.id),
                progress: None,
                detail: None,
                wrong: false,
            })
            .collect();
        let checking = Arc::new(Checking {
            files,
            module: code.module.clone(),
            cancel: Arc::default(),
            found: std::sync::Mutex::new(Found {
                proofs,
                ..Found::default()
            }),
            stage: watch::Sender::new(Stage::Testing),
            green_told: AtomicBool::new(false),
            further,
            obligations: code.obligations.clone(),
            machine_code: code.machine_code.clone(),
            functions: code.functions.clone(),
            data: code.data.clone(),
            outside: code.outside.clone(),
            calls: !code.calls.is_empty(),
        });
        if let Some(old) = self
            .checks
            .lock()
            .await
            .insert(folder.path().to_path_buf(), Arc::clone(&checking))
        {
            old.cancel.store(true, Ordering::Relaxed);
        }
        let (server, folder, job) = (self.clone(), folder.clone(), Arc::clone(&checking));
        tokio::spawn(async move {
            server.test_and_prove_wasm(&folder, &job, code).await;
            job.stage.send_replace(Stage::Ended);
        });
        checking
    }

    /// The job itself for main.wat: every test, in the pool's workers; once they all pass, the
    /// code's stamp in lengua.lock, then the proofs, each generated test proven with lengua's
    /// WebAssembly prover, or taken from what lengua.lock records of this very module; and once
    /// they end with no input found wrong, lengua's entry in idea.md's `verified`.
    async fn test_and_prove_wasm(
        &self,
        folder: &ProgramFolder,
        checking: &Checking,
        code: WasmCode,
    ) {
        let fail = |failures: Vec<String>| {
            if let Ok(mut found) = checking.found.lock() {
                found.failures = failures;
            }
        };
        let [idea_hash, code_hash] = checking.files.clone();
        let seed = match random_seed() {
            Ok(seed) => seed,
            Err(error) => return fail(vec![error]),
        };
        let outcome = match testrun::run_wasm_tests(
            &code.suite,
            &self.pool(),
            Arc::clone(&code.program),
            seed,
            &code.idea.uses,
            &code.idea.network,
            testrun::workers(),
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(error) => return fail(vec![format!("couldn't run the program: {error}")]),
        };
        if !outcome.passed() {
            return fail(outcome.failures);
        }
        // The module the tests ran, kept for a run from a shell, which runs it once its bytes
        // hash to the module the stamp records; a stamp it couldn't be kept for isn't written.
        let tested = &code.program.built().wasm;
        if let Err(error) = keep_tested(&self.data_dir, &checking.module, tested) {
            return fail(vec![error]);
        }
        // And compiled for this computer ahead of its runs, sealed with the user's key, once for
        // a run from a shell and once for the `run` tool, whose engine can stop a program: each
        // loads its own and compiles nothing. A module that couldn't be compiled or kept here
        // still runs, since its first run compiles it.
        if let Ok(Some(key)) = Key::load(&self.data_dir) {
            for setup in [Setup::Shell, Setup::Run] {
                if let Err(error) =
                    compile_and_keep_for(setup, &self.data_dir, &key, &checking.module, tested)
                {
                    tracing::warn!("couldn't compile the module ahead of its runs: {error}");
                }
            }
        }
        // The modules it calls, kept and compiled ahead of its runs as its own module is: a
        // run loads each in its compiled form, under the user's seal, or not at all.
        for called in &code.calls {
            if let Err(error) = keep_tested(&self.data_dir, &called.module, &called.wasm) {
                return fail(vec![error]);
            }
            if let Ok(Some(key)) = Key::load(&self.data_dir) {
                for setup in [Setup::Shell, Setup::Run] {
                    let kept = compile_and_keep_for(
                        setup,
                        &self.data_dir,
                        &key,
                        &called.module,
                        &called.wasm,
                    );
                    if let Err(error) = kept {
                        tracing::warn!(
                            "couldn't compile a called module ahead of its runs: {error}"
                        );
                    }
                }
            }
        }
        // An outside module's stamp says what it is in the lock's own text: its two hashes are
        // one, and its engine's line ends in the note.
        let engine = if code.outside.is_some() {
            format!("{}{}", engine::ENGINE, lock::OUTSIDE_NOTE)
        } else {
            engine::ENGINE.to_owned()
        };
        let stamp = CodeStamp {
            sha256: code_hash,
            module_sha256: checking.module.clone(),
            engine,
            max_stack_bytes: code.max_stack_bytes,
            unsafe_blocks: u32::try_from(code.unsafe_blocks).unwrap_or(u32::MAX),
            features: code.features.clone(),
            parts: code.parts.clone(),
            calls: calls::stamps(&code.calls),
        };
        if let Err(error) = self.stamp_code(folder, &idea_hash, stamp).await {
            return fail(vec![error]);
        }
        let every_value: Vec<String> = code
            .suite
            .generated
            .iter()
            .filter_map(|block| {
                block
                    .every_value_text()
                    .map(|text| format!("generated {} tried {text}", block.id))
            })
            .collect();
        let performance: Vec<String> = outcome.costs.iter().map(cost_line).collect();
        if let Ok(mut found) = checking.found.lock() {
            found.passed = Some(Passed {
                runs: outcome.runs,
                every_value,
                performance,
                shown: outcome.shown,
            });
        }
        // An outside module ends here: nothing of it is proven, and lengua writes no entry for
        // it in idea.md's `verified`.
        if code.outside.is_some() {
            if let Ok(mut found) = checking.found.lock() {
                for proof in &mut found.proofs {
                    proof.detail = Some(outside::NOT_PROVEN.to_owned());
                }
            }
            return;
        }
        checking.stage.send_replace(Stage::Proving(Instant::now()));
        match self.recorded_proofs(folder, checking, &code.suite) {
            None => self.prove_all(folder, checking, &code, seed, &[]).await,
            Some(further) if !further.is_empty() => {
                self.prove_all(folder, checking, &code, seed, &further)
                    .await;
            }
            Some(_) => {}
        }
        if checking.cancel.load(Ordering::Relaxed) {
            return;
        }
        // A proof that found main.wat wrong, now or as lengua.lock records, leaves lengua no
        // entry in idea.md's `verified`; otherwise the entry goes in, as at done.
        let wrong = checking.found.lock().is_ok_and(|found| found.wrong);
        if wrong {
            self.unverify(folder, checking).await;
        }
        self.verify_once_passed(folder, checking).await;
    }

    /// Writes lengua's entry into idea.md's `verified` for a job whose tests all passed, which
    /// stamped the code, and whose proofs found no input wrong.
    async fn verify_once_passed(&self, folder: &ProgramFolder, checking: &Checking) {
        let passed = checking
            .found
            .lock()
            .is_ok_and(|found| found.passed.is_some() && found.failures.is_empty() && !found.wrong);
        if passed {
            self.write_verified(folder, checking).await;
        }
    }

    /// Writes lengua's entry into idea.md's `verified` once check says main.wat is done, or its
    /// job ends with every test passed: lengua and its version, the time, and the idea and the
    /// code it checked, provided both files are still those, no proof found the code wrong, and
    /// lengua's entry doesn't say so already. idea.md's hash leaves `verified` out, so the lock
    /// still holds. Gives whether idea.md holds that entry once it returns, so that a reply
    /// claims only an entry that's there.
    pub(super) async fn write_verified(&self, folder: &ProgramFolder, checking: &Checking) -> bool {
        // lengua's entry says lengua checked the idea and its main.wat: an outside module, which
        // it tested and holds to no rule and no proof, gets none.
        // Nor does a program that calls a module: the entry names an idea and its main.wat, and
        // what such a program does rests on a third file, which the entry can't name.
        if checking.outside.is_some() || checking.calls {
            return false;
        }
        let [idea, code] = &checking.files;
        let _writing = self.lock_writes.lock().await;
        // Under the lock of the writes: a proof that finds main.wat wrong records it in
        // lengua.lock, then takes the entry away, each under this lock too, so whichever comes
        // first, no entry is left for a build found wrong.
        if checking.found.lock().is_ok_and(|found| found.wrong) {
            return false;
        }
        let recorded_wrong = Key::load(&self.data_dir)
            .ok()
            .flatten()
            .and_then(|key| lock::read(folder.path(), Some(&key)).ok().flatten())
            .and_then(|lock| lock.proofs)
            .is_some_and(|proofs| {
                proofs.module_sha256 == checking.module && !proofs.wrong.is_empty()
            });
        if recorded_wrong {
            return false;
        }
        let path = folder.file(IDEA_FILE);
        let Ok(Some(text)) = crate::state::read_text(&path) else {
            return false;
        };
        let current = crate::state::read_file(&folder.file(CODE_FILE))
            .ok()
            .flatten()
            .map(|bytes| Sha256Hex::of(&bytes));
        if trust::idea_hash(text.as_bytes()) != *idea || current.as_ref() != Some(code) {
            return false;
        }
        if let [entry] = trust::lengua_entries(&text).as_slice()
            && entry.by == trust::ACTOR
            && entry.idea == *idea
            && entry.code == *code
        {
            return true;
        }
        let entry = trust::Verified::new(trust::utc(now_seconds()), idea.clone(), code.clone());
        let written = trust::with_lengua_verified(&text, &entry);
        // An idea.md with no frontmatter has no `verified` to hold it.
        if written == text {
            return false;
        }
        match replace(&path, &written) {
            Ok(()) => true,
            Err(error) => {
                eprintln!(
                    "lengua: couldn't write lengua's entry in {}: {error}",
                    path.display()
                );
                false
            }
        }
    }

    /// Takes lengua's entry out of idea.md's `verified` when it no longer names the idea and the
    /// main.wat that are in `folder`: an edit to either starts a new, unverified definition (OKF
    /// v0.2 §10.3) until check takes it to done again. Everyone else's entries stay. `text` is
    /// idea.md as check read it; gives idea.md as it is now, for the rest of the check, so each
    /// problem's line is the file's.
    pub(super) async fn keep_verified(&self, folder: &ProgramFolder, text: String) -> String {
        let entries = trust::lengua_entries(&text);
        if entries.is_empty() {
            return text;
        }
        let idea = trust::idea_hash(text.as_bytes());
        let code = crate::state::read_file(&folder.file(CODE_FILE))
            .ok()
            .flatten()
            .map(|bytes| Sha256Hex::of(&bytes));
        let current =
            |entry: &trust::Verified| entry.idea == idea && Some(&entry.code) == code.as_ref();
        if entries.iter().all(current) {
            return text;
        }
        let Some(without) = trust::without_lengua_verified(&text) else {
            return text;
        };
        let _writing = self.lock_writes.lock().await;
        let path = folder.file(IDEA_FILE);
        // An idea edited since this check read it is the next check's.
        if crate::state::read_text(&path).ok().flatten().as_deref() != Some(text.as_str()) {
            return text;
        }
        match replace(&path, &without) {
            Ok(()) => without,
            Err(error) => {
                eprintln!(
                    "lengua: couldn't take lengua's entry out of {}: {error}",
                    path.display()
                );
                text
            }
        }
    }

    /// Takes lengua's entry out of idea.md's `verified` once a proof found the main.wat that
    /// `checking` checked wrong: the entry says lengua checked this idea and this code, which no
    /// longer holds. Only an entry that names those very files goes, and everyone else's stay.
    pub(super) async fn unverify(&self, folder: &ProgramFolder, checking: &Checking) {
        let [idea, code] = &checking.files;
        let _writing = self.lock_writes.lock().await;
        let path = folder.file(IDEA_FILE);
        let Ok(Some(text)) = crate::state::read_text(&path) else {
            return;
        };
        let names_them = trust::lengua_entries(&text)
            .iter()
            .any(|entry| entry.idea == *idea && entry.code == *code);
        if !names_them {
            return;
        }
        let Some(without) = trust::without_lengua_verified(&text) else {
            return;
        };
        if let Err(error) = replace(&path, &without) {
            eprintln!(
                "lengua: couldn't take lengua's entry out of {}: {error}",
                path.display()
            );
        }
    }

    /// `run` for main.wat: in a worker of its own, in the engine's configuration for runs (a
    /// deadline, nothing counted), reaching only what its idea allows, in `folder` or a fresh
    /// one; a server stays serving in the background. A build a proof found wrong doesn't run.
    pub(super) async fn run_wasm(
        &self,
        folder: &ProgramFolder,
        params: &RunParams,
    ) -> Result<RunReport, String> {
        let ready = runnable(&self.data_dir, folder, &params.env)?;
        // What this server's job found wrong in the module the run holds, which lengua.lock may
        // not record, refuses it too, before anything is built.
        if let Some(wrong) = self.proven_wrong(folder, &ready.module).await {
            return Err(format!(
                "The proof found inputs where {CODE_FILE} is wrong, so it won't run: \
                 {wrong}. Call check with path {folder} to see them, then fix {CODE_FILE}."
            ));
        }
        // The tool's reply and its worker need the build's assertions and lines: it builds
        // main.wat, held to the module check tested.
        let built = built_as_tested(folder, &ready)?;
        // An idea that names a secret: the code this computer runs is checked first.
        keeps_secrets(folder, &ready, &built, Setup::Run)?;
        // The tools the run asks for: a stop runs a copy of main.wat built for this run.
        let asked = crate::wasm::tools::Asked {
            trace: params.trace,
            profile: params.profile,
            stop_at: params.stop_at.clone(),
            show_memory: params.show_memory.clone(),
        };
        let tools::Equipped {
            tools: run_tools,
            names,
            instead,
        } = tools::equipped(&asked, &ready, &built)?;
        // An outside module's every reply says what it is.
        let said = ready.code.words().map(str::to_owned);
        let (uses, network) = (ready.uses, ready.network);
        let workspace = params.folder.as_deref().map(working_folder).transpose()?;
        // A folder the run reads or writes in, for a program that uses files or for its standard
        // input, is never one of lengua's own or the system's.
        if let Some(given) = &workspace
            && (uses.contains(&Use::Files) || params.stdin_file.is_some())
            && let Some(problem) = own_folder_problem(&self.data_dir, given)
        {
            return Err(problem);
        }
        let seconds = params.timeout_seconds.unwrap_or(10);
        if !(1..=RUN_SECONDS).contains(&seconds) {
            return Err(format!("timeout_seconds goes from 1 to {RUN_SECONDS}"));
        }
        let stdin = match (params.stdin_file.as_deref(), workspace.as_deref()) {
            (None, _) => params.stdin.as_bytes().to_vec(),
            (Some(_), None) => {
                return Err("stdin_file names a file in folder; give folder too".into());
            }
            (Some(name), Some(folder)) => read_stdin_file(folder, name)?,
        };
        // A copy with a stop is compiled for this one run: no compiled form of it is loaded or kept.
        let checked_module = instead.is_none();
        let assertions = built.assertions.clone();
        let program = Arc::new(
            Program::new(instead.unwrap_or(built))
                .map_err(|error| format!("lengua couldn't keep the module: {error}"))?
                .with_calls(called_modules(&ready.calls)?),
        );
        let connections = reaches(&network);
        let own = if network.reach.files.is_empty() {
            Vec::new()
        } else {
            own_folders(&self.data_dir)
        };
        let (max_file_size, max_written) = run_file_limits(workspace.is_some());
        let spec = Spec {
            folder: workspace.as_deref(),
            env: &params.env,
            connections: &connections,
            listen: network.listens.map(|port| (port, network.serves_anywhere)),
            // A run reaches the real computers its idea grants, not a test's own.
            stands: None,
            own: &own,
            setup: Setup::Run,
            // Its worker loads the compiled form check kept, under the user's seal.
            sealed: checked_module,
            max_written,
            alone: true,
            tools: run_tools,
            ..Spec::new(&params.args, &stdin, Policy::of(&uses, &network))
        };
        let may_reach = may_reach(&uses, &network);
        // A server, of TCP connections or of UDP datagrams, serves in the background.
        if network.listens.is_some() || network.reach.receives.is_some() {
            if asked.any() {
                return Err(tools::NO_TOOLS_SERVING.to_owned());
            }
            let mut report = self
                .serve_wasm_in_background(folder, &program, spec, assertions)
                .await?;
            report.may_reach = may_reach;
            report.outside = said;
            return Ok(report);
        }
        let limits = Limits {
            timeout: Duration::from_secs(seconds),
            max_file_size,
            ..Limits::default()
        };
        let (run, found) = self
            .pool()
            .run_found(&program, spec, limits)
            .await
            .map_err(|error| format!("couldn't run the program: {error}"))?;
        let mut report = ended_report(&run, &assertions, limits);
        if asked.any() {
            tools::into_report(&mut report, found, run_tools, &names);
        }
        report.may_reach = may_reach;
        report.outside = said;
        Ok(report)
    }

    /// Starts main.wat's server in a worker of its own and leaves it serving, at most one per
    /// program folder; one that never came to serve is stopped, and the reply says how it ended.
    async fn serve_wasm_in_background(
        &self,
        folder: &ProgramFolder,
        program: &Arc<Program>,
        spec: Spec<'_>,
        assertions: BTreeMap<i64, usize>,
    ) -> Result<RunReport, String> {
        let mut background = self.background.lock().await;
        already_serving(&background, folder)?;
        let receives = spec.policy.reach.get().receives;
        let datagrams = spec.listen.is_none() && receives.is_some();
        let anywhere = spec
            .listen
            .or(receives)
            .is_some_and(|(_, anywhere)| anywhere);
        let (max_file_size, _) = run_file_limits(spec.folder.is_some());
        let limits = background_limits(max_file_size);
        let running = self
            .pool()
            .start(program, spec, limits)
            .await
            .map_err(|error| format!("couldn't start the server: {error}"))?;
        if running.port().is_none() {
            drop(background);
            let run = running
                .stop()
                .await
                .map_err(|error| format!("couldn't stop the program: {error}"))?;
            return Ok(ended_report(&run, &assertions, limits));
        }
        let connections = running
            .connections()
            .iter()
            .map(ConnectionReport::of)
            .collect();
        let server = Serving {
            running: Box::new(running),
            program: Arc::clone(program),
            anywhere,
            datagrams,
        };
        Ok(self.keep_serving(&mut background, folder, server, assertions, connections))
    }
}

/// What `lengua-mcp check` found in a folder, as the idea's attester (OKF v0.2 §10).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attestation {
    /// What it found, one line a finding, and its verdict last: `attested` or `not attested`.
    pub lines: Vec<String>,
    /// Whether the folder is attested: its idea and main.wat build, follow every rule and pass
    /// every test, no proof found main.wat wrong, and lengua's entry in the idea's `verified`
    /// names them both.
    pub attested: bool,
}

/// Attests the program folder at `path`, absolute or from the current folder, as the attester an
/// Attested Computation names: reads its idea.md and main.wat, builds main.wat, checks it
/// against every rule, runs every test of the idea, proves each generated test within the
/// attester's own time, and compares lengua's entry in the idea's `verified` with the files. It
/// writes nothing (no lock, no index.md, no log.md) and needs no lock and no signing key, so it
/// attests a folder on any computer.
pub async fn attest(path: &str) -> Attestation {
    let mut lines = Vec::new();
    let attests = attest_into(path, &mut lines).await;
    let verdict = if attests { "attested" } else { "not attested" };
    lines.push(verdict.to_owned());
    Attestation {
        lines,
        attested: attests,
    }
}

/// [`attest`]'s work, each finding a line in `lines`: whether the folder is attested.
async fn attest_into(path: &str, lines: &mut Vec<String>) -> bool {
    let found = std::env::current_dir()
        .map_err(|error| format!("can't read the current folder: {error}"))
        .and_then(|here| {
            let named = here.join(path);
            std::fs::canonicalize(&named).map_err(|error| format!("{}: {error}", named.display()))
        })
        .and_then(|folder| ProgramFolder::new(&folder.to_string_lossy()));
    let folder = match found {
        Ok(folder) => folder,
        Err(problem) => {
            lines.push(problem);
            return false;
        }
    };
    // main.s in main.wat's place is the native lengua's code, of which only that it is there is
    // read: the line says where the native lengua went, as check's reply does.
    if let Some(native) = native_code(&folder) {
        lines.extend(native.problems);
        return false;
    }
    // An outside module is never attested: the attester attests an idea and its main.wat, by
    // the rules and the proofs, and main.wasm is held to neither.
    if let Ok(Some(text)) = read_optional(&folder, IDEA_FILE)
        && crate::state::names_outside(text.as_bytes())
    {
        lines.push(outside::NOT_ATTESTED.to_owned());
        return false;
    }
    let (idea_text, code) = match (
        read_optional(&folder, IDEA_FILE),
        read_optional(&folder, CODE_FILE),
    ) {
        (Ok(Some(idea_text)), Ok(Some(code))) => (idea_text, code),
        (idea_text, code) => {
            for (name, read) in [(IDEA_FILE, idea_text), (CODE_FILE, code)] {
                match read {
                    Ok(Some(_)) => {}
                    Ok(None) => lines.push(format!("{name}: {folder} has none")),
                    Err(problem) => lines.push(problem),
                }
            }
            return false;
        }
    };
    // The trust record first: comparing it needs nothing built.
    let mut attests = verified_lines(&idea_text, &code, lines);
    let idea = match idea::parse(&idea_text) {
        Ok(idea) => idea,
        Err(problems) => {
            lines.extend(
                problems
                    .iter()
                    .map(|problem| format!("{IDEA_FILE} {problem}")),
            );
            return false;
        }
    };
    // The type's problem names idea.md itself; the threads' is on a line of it.
    if let Some(problem) = type_problem(&idea_text, &idea) {
        lines.push(problem);
        return false;
    }
    if let Some(problem) = threads_problem(&idea_text, &idea) {
        lines.push(format!("{IDEA_FILE} {problem}"));
        return false;
    }
    // A program that calls a module isn't attested: the attester attests an idea and its
    // main.wat, with no lock and no key, and what such a program does rests on a module that
    // neither file is.
    if !idea.network.reach.calls.is_empty() {
        lines.push(calls::NOT_ATTESTED.to_owned());
        return false;
    }
    let suite = match testfile::check(&idea) {
        Ok(suite) => suite,
        Err(problems) => {
            lines.extend(
                problems
                    .iter()
                    .map(|problem| format!("{IDEA_FILE} {problem}")),
            );
            return false;
        }
    };
    let built = match text::build(&code) {
        Ok(built) => built,
        Err(problem) => {
            lines.push(problem.to_string());
            return false;
        }
    };
    lines.push(format!(
        "{CODE_FILE}: builds into a module of {} bytes, sha256:{}",
        built.wasm.len(),
        Sha256Hex::of(&built.wasm)
    ));
    let context = checker::wasm::Context {
        uses: &idea.uses,
        network: &idea.network,
        secrets: &idea.secrets,
    };
    let report = checker::wasm::check(&code, &built, &context);
    if !report.passed() {
        lines.extend(report.lines());
        return false;
    }
    lines.push(format!(
        "{CODE_FILE}: follows every rule of lengua's WebAssembly dialect"
    ));
    // A part's contract, held to what the rules found of the functions it offers.
    let stated = parts::contract_problems(&idea_text, &code, &report.lends);
    if !stated.is_empty() {
        lines.extend(stated);
        return false;
    }
    for offered in parts::contract_lines(&idea_text) {
        lines.push(format!("{IDEA_FILE}: its contract holds: {offered}"));
    }
    // An idea that names a secret: the code the engine compiles the module to on this computer
    // keeps the secret rule too, compiled here with nothing kept.
    if !idea.secrets.is_empty() {
        let setups = [Setup::Run, Setup::Shell];
        match machine::check(&built.wasm, &report.secret_operators, &setups) {
            Ok(passed) => lines.push(format!("{CODE_FILE}: {}", passed.line())),
            Err(refused) => {
                lines.extend(refused.iter().map(ToString::to_string));
                return false;
            }
        }
    }
    let launcher = match std::env::current_exe() {
        Ok(launcher) => launcher,
        Err(error) => {
            lines.push(format!("tests: can't find lengua-mcp itself: {error}"));
            return false;
        }
    };
    let program = match Program::new(built) {
        Ok(program) => Arc::new(program),
        Err(error) => {
            lines.push(format!("tests: lengua couldn't keep the module: {error}"));
            return false;
        }
    };
    let seed = match random_seed() {
        Ok(seed) => seed,
        Err(error) => {
            lines.push(format!("tests: {error}"));
            return false;
        }
    };
    // Workers of its own, with no data folder: the attester writes nothing.
    let pool = Pool::new(launcher, None);
    match testrun::run_wasm_tests(
        &suite,
        &pool,
        Arc::clone(&program),
        seed,
        &idea.uses,
        &idea.network,
        testrun::workers(),
    )
    .await
    {
        Ok(outcome) if outcome.passed() => {
            lines.push(format!(
                "tests: passed all {} test runs, the generated ones drawn from seed {seed}",
                outcome.runs
            ));
            // The proofs take minutes: a folder that isn't attested for another reason needs
            // none to say so.
            if attests {
                let proofs = attest_proofs(&pool, &program, &idea, &suite, seed).await;
                for proof in &proofs {
                    lines.push(format!("proofs: {}", proof.line()));
                }
                let obligations = &report.facts.obligations;
                lines.push(format!("memory: {}", memory_line(obligations, &proofs)));
                attests = !proofs.iter().any(|proof| proof.wrong);
            }
        }
        Ok(outcome) => {
            lines.extend(
                outcome
                    .failures
                    .iter()
                    .map(|failure| format!("tests: {failure}")),
            );
            attests = false;
        }
        Err(error) => {
            lines.push(format!("tests: couldn't run the program: {error}"));
            attests = false;
        }
    }
    attests
}

/// The attester's proofs of main.wat, `program`, whose idea is `idea`, as check proves them but
/// within the attester's own time: each generated test of `suite` in turn, the smallest first,
/// each with its share of [`proof_time`], the model first held to the runs the tests just made
/// from `seed`, and what a proof finds run for real in `pool`'s workers. Gives each test's
/// proof, in the tests' order.
async fn attest_proofs(
    pool: &Pool,
    program: &Arc<Program>,
    idea: &Idea,
    suite: &TestSuite,
    seed: u64,
) -> Vec<BlockProof> {
    let model = Model {
        program: Arc::clone(program),
        policy: Policy::of(&idea.uses, &idea.network),
    };
    let listens = idea.network.listens.is_some();
    let deadline = Instant::now() + proof_time();
    let blocks = suite.generated.len();
    let mut order: Vec<usize> = (0..blocks).collect();
    order.sort_by_key(|index| prove::size(&suite.generated[*index]));
    let mut proofs: Vec<BlockProof> = suite
        .generated
        .iter()
        .map(|block| BlockProof {
            test: format!("generated {}", block.id),
            progress: None,
            detail: None,
            wrong: false,
        })
        .collect();
    for (turn, &index) in order.iter().enumerate() {
        let block = &suite.generated[index];
        // Each test gets its share of the time left, as check gives it.
        let now = Instant::now();
        let share = deadline.saturating_duration_since(now)
            / u32::try_from(blocks - turn).unwrap_or(u32::MAX).max(1);
        let host = host_values(idea, block);
        let (detail, wrong) = match block
            .every_value_text()
            .filter(|_| !host && !block.uses_now())
        {
            Some(every) => (format!("proven: the tests tried {every}"), false),
            None => {
                let budget = prove::Budget::until(now + share, listens);
                let samples: Vec<testfile::Case> =
                    block.cases(seed).take(24).filter_map(Result::ok).collect();
                match model.verdict(block, samples, listens, budget).await {
                    Err(reason) | Ok(Verdict::NotProven(reason)) => {
                        (format!("tested, not proven: {reason}"), false)
                    }
                    Ok(Verdict::Proven {
                        shapes,
                        paths,
                        questions,
                        alone,
                        bound,
                        cut_any_way,
                        unreached,
                        induction,
                        lookback,
                    }) => {
                        let detail = proven_detail(
                            block,
                            shapes,
                            paths,
                            questions,
                            alone,
                            bound,
                            cut_any_way,
                            &unreached,
                            induction.map(|chunk| (chunk, lookback)),
                        );
                        // Proving on from where a proof stopped is check's, which keeps a lock.
                        (detail.replace(FURTHER, ""), false)
                    }
                    Ok(Verdict::Counterexample(values)) => {
                        replay(pool, program, idea, block, values, host).await
                    }
                }
            }
        };
        if let Some(proof) = proofs.get_mut(index) {
            proof.detail = Some(detail);
            proof.wrong = wrong;
        }
    }
    proofs
}

/// The lines on lengua's entry in `idea_text`'s `verified`, compared with the idea and with
/// main.wat's `code`: whether it names both.
fn verified_lines(idea_text: &str, code: &str, lines: &mut Vec<String>) -> bool {
    let idea = trust::idea_hash(idea_text.as_bytes());
    let code = Sha256Hex::of(code.as_bytes());
    match trust::lengua_entries(idea_text).as_slice() {
        [] => {
            lines.push(format!(
                "verified: {IDEA_FILE} has no entry of lengua's, so no check of lengua's took \
                 this idea and this code to done"
            ));
            false
        }
        [.., entry] => {
            let mut named = true;
            if entry.idea != idea {
                lines.push(format!(
                    "verified: {}'s entry names the idea {}, and {IDEA_FILE} is {} now",
                    entry.by,
                    entry.idea.label(),
                    idea.label()
                ));
                named = false;
            }
            if entry.code != code {
                lines.push(format!(
                    "verified: {}'s entry names the code {}, and {CODE_FILE} is {} now",
                    entry.by,
                    entry.code.label(),
                    code.label()
                ));
                named = false;
            }
            if named {
                lines.push(format!(
                    "verified: {} checked this {IDEA_FILE} and this {CODE_FILE} at {}",
                    entry.by, entry.at
                ));
            }
            named
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The add idea as an Attested Computation.
    const IDEA: &str = include_str!("../../tests/fixtures/add/idea.md");

    /// What the review of the host left open (todo.md), held by a test that failed before its
    /// fix: on Linux and macOS, where the test can make the link it swaps in.
    #[cfg(unix)]
    mod review_gaps {
        use super::*;

        /// `stdin_file` is opened once, and what is checked and read is the file that opened:
        /// another process that puts a link to a file outside the folder under the name, between
        /// the look and the read, gets nothing of that file read. Here a thread swaps the name
        /// between a file of the folder and such a link for as long as the reads go on, and every
        /// read that gives bytes gives the folder's own.
        #[test]
        fn stdin_file_is_the_file_that_opened_whatever_takes_its_name() {
            use std::sync::atomic::{AtomicBool, Ordering};
            let outside = tempfile::tempdir().expect("a folder outside");
            let secret = outside.path().join("secret.txt");
            std::fs::write(&secret, "outside").expect("a file outside");
            let made = tempfile::tempdir().expect("the working folder");
            let folder = made.path().canonicalize().expect("the folder");
            let (name, file, link) = (
                folder.join("in.txt"),
                folder.join("file"),
                folder.join("link"),
            );
            std::fs::write(&file, "inside").expect("the folder's file");
            std::os::unix::fs::symlink(&secret, &link).expect("a link");
            let done = AtomicBool::new(false);
            let mut read = 0_u32;
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    while !done.load(Ordering::Relaxed) {
                        for other in [&file, &link] {
                            let _ = std::fs::rename(other, &name);
                            let _ = std::fs::rename(&name, other);
                        }
                    }
                });
                let until = std::time::Instant::now() + Duration::from_secs(3);
                let mut leaked = None;
                while leaked.is_none() && std::time::Instant::now() < until {
                    match read_stdin_file(&folder, "in.txt") {
                        Ok(bytes) if bytes == b"inside" => read += 1,
                        Ok(bytes) => leaked = Some(bytes),
                        Err(_) => {}
                    }
                }
                done.store(true, Ordering::Relaxed);
                assert_eq!(
                    leaked.as_deref().map(String::from_utf8_lossy),
                    None,
                    "after {read} reads of the folder's own file"
                );
            });
            assert!(read > 0, "the folder's own file was never read");
        }
    }

    /// A run in lengua's own temporary folder is held to a test's file limits; one in a folder
    /// of the user's is held to none.
    #[test]
    fn a_run_s_throw_away_folder_has_a_test_s_file_limits() {
        let limits = (
            Some(crate::runner::TEST_FILE_LIMIT),
            Some(crate::wasm::host::limits::TEST_WRITTEN),
        );
        assert_eq!(run_file_limits(false), limits);
        assert_eq!(run_file_limits(true), (None, None));
    }

    /// The add idea as an idea of the native lengua's type, whose line 2 says the type.
    fn native_idea() -> String {
        let (_, body) = IDEA
            .split_once("\n---\n")
            .expect("the add idea's frontmatter");
        format!(
            "---\ntype: {}\ntitle: add\ndescription: Adds two numbers.\n---\n{body}",
            idea::IDEA_TYPE
        )
    }

    /// An idea of the native lengua's type still parses, and check refuses it on its type's
    /// line, saying where the native lengua went; an Attested Computation has no such problem.
    #[test]
    fn an_idea_of_the_native_lengua_s_type_is_told_where_it_went() {
        let native = native_idea();
        let parsed = idea::parse(&native).unwrap_or_else(|problems| panic!("{problems:#?}"));
        let problem = type_problem(&native, &parsed).expect("the native lengua's type");
        assert!(
            problem.starts_with("line 2: idea.md is an idea of the native lengua"),
            "{problem}"
        );
        assert!(
            problem.contains("main.wat since 0.2.0") && problem.contains("native-final"),
            "{problem}"
        );
        let idea = idea::parse(IDEA).expect("the add idea");
        assert_eq!(type_problem(IDEA, &idea), None);
    }

    /// main.s, the native lengua's code, is never read: alone, the reply says where the native
    /// lengua went and how to go on; beside main.wat, it is one code file too many.
    #[test]
    fn main_s_is_the_native_lengua_s_code() {
        let program = tempfile::tempdir().expect("a folder");
        let folder = ProgramFolder::new(program.path().to_str().expect("UTF-8")).expect("absolute");
        assert!(native_code(&folder).is_none());
        assert!(both_code_files(&folder).is_none());
        std::fs::write(folder.file(CODE_FILE), "(module)").expect("write main.wat");
        assert!(native_code(&folder).is_none());
        assert!(both_code_files(&folder).is_none());
        std::fs::write(folder.file(NATIVE_CODE_FILE), "_start:\n").expect("write main.s");
        assert!(native_code(&folder).is_none());
        let both = both_code_files(&folder).expect("both code files");
        assert_eq!(both.state, "code_problems");
        assert!(
            both.problems[0].contains("holds both main.s and main.wat"),
            "{both:?}"
        );
        let wanted = format!("Next: delete {folder}/main.s, then call check");
        assert!(both.next_step.starts_with(&wanted), "{both:?}");
        // With main.s alone, the next step makes main.wat first: there is none to keep.
        std::fs::remove_file(folder.file(CODE_FILE)).expect("remove main.wat");
        assert!(both_code_files(&folder).is_none());
        let alone = native_code(&folder).expect("main.s alone");
        assert_eq!(alone.state, "code_problems");
        assert!(
            alone.problems[0].contains("main.wat since 0.2.0")
                && alone.problems[0].contains("the tag native-final keeps the native lengua"),
            "{alone:?}"
        );
        let wanted = format!(
            "compile it into {folder}/main.wat with the compile skill, delete {folder}/main.s"
        );
        assert!(alone.next_step.contains(&wanted), "{alone:?}");
    }

    /// `lengua-mcp check` says where the native lengua went as check does: of main.s in
    /// main.wat's place, and of an idea of the native lengua's type beside main.wat, on its
    /// type's line, which names idea.md once.
    #[tokio::test]
    async fn the_attester_says_where_the_native_lengua_went() {
        let program = tempfile::tempdir().expect("a folder");
        let path = program.path().to_str().expect("UTF-8").to_owned();
        let folder = ProgramFolder::new(&path).expect("absolute");
        std::fs::write(folder.file(IDEA_FILE), native_idea()).expect("write idea.md");
        std::fs::write(folder.file(NATIVE_CODE_FILE), "_start:\n").expect("write main.s");
        let alone = attest(&path).await;
        assert!(!alone.attested, "{alone:?}");
        assert!(
            alone.lines[0].contains("main.wat since 0.2.0")
                && alone.lines[0].contains("the tag native-final keeps the native lengua"),
            "{alone:?}"
        );
        std::fs::write(folder.file(CODE_FILE), "(module)").expect("write main.wat");
        let typed = attest(&path).await;
        assert!(!typed.attested, "{typed:?}");
        let line = "line 2: idea.md is an idea of the native lengua";
        assert!(
            typed.lines.iter().any(|said| said.starts_with(line)),
            "{typed:?}"
        );
    }

    #[test]
    fn an_attested_computation_has_no_threads() {
        let threads = IDEA.replace("- Uses: nothing", "- Uses: nothing\n- Threads: up to 4");
        let idea = idea::parse(&threads).unwrap_or_else(|problems| panic!("{problems:#?}"));
        let line = threads.lines().count();
        let problem = threads_problem(&threads, &idea).expect("threads");
        assert!(problem.starts_with(&format!("line {line}: ")), "{problem}");
        let idea = idea::parse(IDEA).expect("the add idea");
        assert_eq!(threads_problem(IDEA, &idea), None);
    }

    #[test]
    fn the_machine_line_names_the_engine_and_no_llvm() {
        let machine = Machine::detect();
        let line = machine_line(&machine);
        assert!(
            line.starts_with(&format!("{}-{}, ", machine.os, machine.arch)),
            "{line}"
        );
        assert!(line.contains("core"), "{line}");
        assert!(
            line.contains(&format!("Wasmtime {}", engine::ENGINE_VERSION)),
            "{line}"
        );
        assert!(
            line.contains("Cranelift") && line.contains("simd"),
            "{line}"
        );
        assert!(!line.contains("LLVM") && !line.contains('\n'), "{line}");
        // Only what the dialect lets main.wat use: no reference types, and mutable globals are
        // 1.0's.
        assert!(
            line.contains("bulk-memory and simd, and no tables or references"),
            "{line}"
        );
        assert!(
            !line.contains("reference-types") && !line.contains("mutable-global"),
            "{line}"
        );
    }

    /// The machine line names the caches the system describes, each level that holds data with
    /// its size, and the size of the lines; and none the system doesn't describe.
    #[test]
    fn the_machine_line_names_the_caches_the_system_describes() {
        let cache = |level, holds, bytes, line| Cache {
            level,
            holds,
            bytes,
            line,
        };
        let described = [
            cache(1, CacheHolds::Instructions, 32 << 10, Some(64)),
            cache(1, CacheHolds::Data, 48 << 10, Some(64)),
            cache(2, CacheHolds::Both, 2 << 20, Some(64)),
            cache(3, CacheHolds::Both, 36 << 20, Some(64)),
        ];
        assert_eq!(
            caches_text(&described),
            "the CPU's caches are L1 data 48 KiB, L2 2 MiB and L3 36 MiB, in 64-byte lines"
        );
        let apple = [
            cache(1, CacheHolds::Data, 128 << 10, Some(128)),
            cache(2, CacheHolds::Both, 16 << 20, Some(128)),
        ];
        assert_eq!(
            caches_text(&apple),
            "the CPU's caches are L1 data 128 KiB and L2 16 MiB, in 128-byte lines"
        );
        let mixed = [
            cache(1, CacheHolds::Data, 1280 << 10, Some(64)),
            cache(2, CacheHolds::Both, 3 << 19, None),
        ];
        assert_eq!(
            caches_text(&mixed),
            "the CPU's caches are L1 data 1280 KiB in 64-byte lines and L2 1536 KiB"
        );
        assert_eq!(
            caches_text(&[]),
            "the system describes none of the CPU's caches"
        );
        assert_eq!(bytes_text(100), "100 bytes");
        let machine = Machine::detect();
        let line = machine_line(&machine);
        assert!(
            line.ends_with(&format!("; {}", caches_text(&machine.caches))),
            "{line}"
        );
    }

    #[test]
    fn lengua_s_entry_is_compared_with_the_files() {
        let code = "(module)\n";
        let mut lines = Vec::new();
        assert!(!verified_lines(IDEA, code, &mut lines));
        assert!(lines[0].contains("no entry of lengua's"), "{lines:?}");
        let entry = trust::Verified::new(
            "2026-11-06T23:00:00Z",
            trust::idea_hash(IDEA.as_bytes()),
            Sha256Hex::of(code.as_bytes()),
        );
        let verified = trust::with_lengua_verified(IDEA, &entry);
        let mut lines = Vec::new();
        assert!(verified_lines(&verified, code, &mut lines), "{lines:?}");
        let mut lines = Vec::new();
        assert!(!verified_lines(&verified, "(module (func))\n", &mut lines));
        assert!(lines[0].contains("names the code"), "{lines:?}");
        let edited = verified.replace("title: add", "title: sum");
        let mut lines = Vec::new();
        assert!(!verified_lines(&edited, code, &mut lines));
        assert!(lines[0].contains("names the idea"), "{lines:?}");
    }

    /// A generated test's proof with `detail`, or still running without one.
    fn proof(detail: Option<&str>) -> BlockProof {
        BlockProof {
            test: "generated B1".to_owned(),
            progress: None,
            detail: detail.map(str::to_owned),
            wrong: false,
        }
    }

    /// An access the rules leave to the proofs, on line 12.
    fn access() -> Obligation {
        Obligation {
            line: Some(12),
            what: "the buffer $args_get borrows in argument 2 touches as many bytes of $args as \
                   the call is given, which the rules don't follow"
                .to_owned(),
            division: false,
        }
    }

    /// A division the rules leave to the proofs, on line 30.
    fn division() -> Obligation {
        Obligation {
            line: Some(30),
            what: "`i32.div_s` never divides the most negative number by -1, which traps"
                .to_owned(),
            division: true,
        }
    }

    /// main.wat's memory line says what the rules settle, what they leave to the proofs, and what
    /// the proofs settle of that: a division in full, an access inside the program's memory but
    /// not inside its region, which the model doesn't follow; "tested, not proven" is said only
    /// of what no proof settles.
    #[test]
    fn the_memory_line_says_what_the_proofs_settle_of_the_rules_obligations() {
        let proven = [proof(Some("proven for every input in range"))];
        let line = memory_line(&[], &proven);
        assert!(
            line.ends_with("so they leave nothing to the proofs"),
            "{line}"
        );
        assert!(!line.contains("tested, not proven"), "{line}");
        let line = memory_line(&[access()], &proven);
        assert!(
            line.contains("; they leave the proofs 1 obligation: main.wat line 12: the buffer"),
            "{line}"
        );
        assert!(
            line.contains(
                "; on every run the proofs of B1 cover, every access stays inside the program's \
                 memory, but the proofs follow no region"
            ),
            "{line}"
        );
        assert!(line.ends_with("is tested, not proven"), "{line}");
        let line = memory_line(&[division()], &proven);
        assert!(
            line.ends_with("; on every run the proofs of B1 cover, no division traps"),
            "{line}"
        );
        assert!(!line.contains("tested, not proven"), "{line}");
        let line = memory_line(&[access(), division()], &[proof(None)]);
        assert!(
            line.contains("they leave the proofs 2 obligations: ")
                && line.ends_with("; the proofs check them on every run they cover"),
            "{line}"
        );
        let unproven = [proof(Some("tested, not proven: the proof ran out of time"))];
        let line = memory_line(&[access()], &unproven);
        assert!(
            line.ends_with("; no proof covers a run, so they are tested, not proven"),
            "{line}"
        );
        // Past four, the rest are counted.
        let many: Vec<Obligation> = (0..6).map(|_| access()).collect();
        let line = memory_line(&many, &proven);
        assert!(line.contains("; and 2 more; "), "{line}");
    }

    /// A proof that found main.wat wrong comes first in its memory line, with the obligation a
    /// real run of its inputs trapped on, which they break: nothing a proof found broken is
    /// called tested, not proven, and what the other proofs settle follows.
    #[test]
    fn the_memory_line_says_what_a_proof_found_wrong() {
        let trapped = "the proof found inputs the tests didn't try where main.wat is wrong: \
                       with a = 1: the program was trapped at main.wat line 30: integer overflow";
        let found = BlockProof {
            wrong: true,
            ..proof(Some(trapped))
        };
        let line = memory_line(&[access(), division()], &[found]);
        assert!(
            line.ends_with(
                "; the proofs of B1 found inputs where main.wat is wrong, as their lines say; on \
                 them the obligation on main.wat line 30 fails"
            ),
            "{line}"
        );
        assert!(!line.contains("tested, not proven"), "{line}");
        // Output a test didn't expect breaks no obligation; another test's proof still settles
        // its runs.
        let printed = "the proof found inputs the tests didn't try where main.wat is wrong: \
                       with a = 1: stdout should be \"1\\n\", but was \"2\\n\"";
        let found = BlockProof {
            wrong: true,
            ..proof(Some(printed))
        };
        let mut other = proof(Some("proven for every input in range"));
        other.test = "generated B2".to_owned();
        let line = memory_line(&[division()], &[found, other]);
        assert!(
            line.ends_with(
                "; the proofs of B1 found inputs where main.wat is wrong, as their lines say; on \
                 every run the proofs of B2 cover, no division traps"
            ),
            "{line}"
        );
    }
}
