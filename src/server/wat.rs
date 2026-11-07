//! main.wat's path through `check`, `run` and the command line. An idea that is an Attested
//! Computation (OKF v0.2 §10) is compiled to main.wat, which lengua builds into one WebAssembly
//! module, checks against the WebAssembly dialect's rules, tests in the server's pool of workers
//! and runs in its own WASI host, the same way on every system: no LLVM, no Linux kernel and no
//! CPU target take part. main.wat has no proofs yet; they come with lengua's WebAssembly prover.
//! Once its tests pass, idea.md's `verified` list holds lengua's entry for the idea and the code
//! it checked, and `lengua-mcp check`, the idea's attester, attests the folder on any computer,
//! with no lock and no key.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::watch;

use super::{
    BlockProof, CheckParams, CheckReport, Checking, ConnectionReport, Found, IdeaStage,
    LenguaServer, NOT_MODELLED, Passed, RUN_SECONDS, RunParams, RunReport, Serving, ShellRun,
    Stage, already_serving, background_limits, check_wait, cost_line_in, counted, ended_report,
    idea_problems, random_seed, reaches, read_idea, read_optional, state_of, wait_for,
    working_folder,
};
use crate::checker;
use crate::connect::Reach;
use crate::data_dir::DataDir;
use crate::graph;
use crate::hash::Sha256Hex;
use crate::idea::{self, Idea, Use};
use crate::lock::{self, CodeStamp, Key};
use crate::platform::Machine;
use crate::runner::{End, Limits};
use crate::state::{CODE_FILE, IDEA_FILE, ProgramFolder, State, WASM_CODE_FILE};
use crate::testfile::{self, TestSuite};
use crate::testrun;
use crate::trust;
use crate::wasm::engine::{self, Setup};
use crate::wasm::job::{Connection, Made};
use crate::wasm::runner::{Here, run_here};
use crate::wasm::text::{self, Built};
use crate::wasm::{Policy, Pool, Program, Spec, parts};

/// What a reply says of main.wat's proofs, until lengua's WebAssembly prover.
pub(super) const NOT_PROVEN: &str =
    "main.wat is tested, not proven: its proofs come with lengua's WebAssembly prover";

/// The exit code `lengua-mcp run` gives a program that trapped: an abort's, 128 and SIGABRT's 6,
/// as WebAssembly runtimes give on a Unix, here on every system.
const TRAPPED: i32 = 134;

/// The most bytes of `stdin_file` `run` gives main.wat: its worker gets its standard input
/// whole, with the job.
const MAX_STDIN_FILE: u64 = 16 * 1024 * 1024;

/// A generated test's proof line for main.wat, after `generated B1: `.
fn no_proof() -> String {
    format!("{NOT_MODELLED}the proofs of main.wat come with lengua's WebAssembly prover")
}

/// What a reply says of main.wat's memory safety once its tests pass.
pub(super) fn memory_line() -> String {
    "the rules (M1 to M8) hold every address they can follow to its owner, and WebAssembly holds \
     every access inside the program's own memory; the rest is tested, not proven: the proofs of \
     main.wat come with lengua's WebAssembly prover"
        .to_owned()
}

/// Whether `text`, an idea, is an Attested Computation, whose code is main.wat, as its
/// frontmatter's `type` says: as YAML reads it, or as the key's own line writes it when the
/// YAML can't be read, so that the idea stage says what's wrong with it the same way on every
/// system.
pub(super) fn attested(text: &str) -> bool {
    let kind = match graph::okf::frontmatter(text) {
        Some(front) => front.kind,
        None => written_type(text).map(str::to_owned),
    };
    kind.is_some_and(|kind| kind.trim() == idea::ATTESTED_TYPE)
}

/// The value of the top-level `type` key of `text`'s frontmatter as its line writes it, without
/// the comment after it or the quotes around it: read line by line, so YAML that can't be read
/// as a whole doesn't hide it.
fn written_type(text: &str) -> Option<&str> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let close = trust::closing(&lines)?;
    let block = trust::blocks(&lines, 1, close)
        .into_iter()
        .find(|block| block.key == "type")?;
    let (_, value) = lines[block.start].split_once(':')?;
    let value = value.split_once(" #").map_or(value, |(value, _)| value);
    Some(value.trim().trim_matches(['"', '\'']))
}

/// Whether `folder`'s idea is an Attested Computation.
///
/// # Errors
///
/// When its idea.md is there and can't be read.
pub(super) fn attested_folder(folder: &ProgramFolder) -> Result<bool, String> {
    Ok(read_optional(folder, IDEA_FILE)?.is_some_and(|text| attested(&text)))
}

/// What's wrong with `folder`'s code files for its idea, an Attested Computation compiled to
/// main.wat when `wasm`, else an idea compiled to main.s: both of them, or only the other one.
/// The reply says so, with the next step for the files that are there.
pub(super) fn code_file_problem(folder: &ProgramFolder, wasm: bool) -> Option<CheckReport> {
    let native = folder.file(CODE_FILE).exists();
    let module = folder.file(WASM_CODE_FILE).exists();
    let attested_type = format!("`type: {}`", idea::ATTESTED_TYPE);
    let check = format!("then call check with path {folder}.");
    let (problem, next_step) = match (wasm, native, module) {
        (true, true, true) => (
            format!(
                "{folder} holds both {CODE_FILE} and {WASM_CODE_FILE}, and {IDEA_FILE} is an \
                 Attested Computation, which lengua compiles to {WASM_CODE_FILE}: delete \
                 {CODE_FILE}"
            ),
            format!("Next: delete {folder}/{CODE_FILE}, {check}"),
        ),
        (true, true, false) => (
            format!(
                "{folder} holds {CODE_FILE}, and {IDEA_FILE} is an Attested Computation, which \
                 lengua compiles to {WASM_CODE_FILE}: compile {IDEA_FILE} into {WASM_CODE_FILE} \
                 with the compile skill, then delete {CODE_FILE}"
            ),
            format!(
                "Next: compile {folder}/{IDEA_FILE} into {folder}/{WASM_CODE_FILE} with the \
                 compile skill, delete {folder}/{CODE_FILE}, {check}"
            ),
        ),
        (false, true, true) => (
            format!(
                "{folder} holds both {CODE_FILE} and {WASM_CODE_FILE}, and {IDEA_FILE} is `type: \
                 {}`, which lengua compiles to {CODE_FILE}: delete {WASM_CODE_FILE}, or make \
                 {IDEA_FILE} an Attested Computation ({attested_type}) with the write-idea skill \
                 and delete {CODE_FILE}",
                idea::IDEA_TYPE
            ),
            format!(
                "Next: delete {folder}/{WASM_CODE_FILE}, or make {folder}/{IDEA_FILE} an Attested \
                 Computation with the write-idea skill and delete {folder}/{CODE_FILE}, {check}"
            ),
        ),
        (false, false, true) => (
            format!(
                "{folder} holds {WASM_CODE_FILE}, and {IDEA_FILE} is `type: {}`, which lengua \
                 compiles to {CODE_FILE}: make {IDEA_FILE} an Attested Computation \
                 ({attested_type}) with the write-idea skill, or compile it into {CODE_FILE} and \
                 delete {WASM_CODE_FILE}",
                idea::IDEA_TYPE
            ),
            format!(
                "Next: make {folder}/{IDEA_FILE} an Attested Computation with the write-idea \
                 skill, or compile it into {folder}/{CODE_FILE} with the compile skill and delete \
                 {folder}/{WASM_CODE_FILE}, {check}"
            ),
        ),
        _ => return None,
    };
    Some(CheckReport::problems("code_problems", vec![problem], next_step))
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

/// How wide this CPU's vectors go, which Cranelift compiles WebAssembly's 128-bit SIMD to.
fn simd_level(machine: &Machine) -> String {
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
/// main.wat for, with how wide its vectors go, and the engine, with the WebAssembly features
/// main.wat may use. No LLVM and no CPU target: one module runs on every computer.
pub(super) fn machine_line(machine: &Machine) -> String {
    let cpu = machine.cpu_model.as_deref().map_or_else(
        || format!("its {} CPU", machine.arch),
        |model| format!("its CPU, {model},"),
    );
    format!(
        "{}-{}, {}; Wasmtime {} compiles main.wat with Cranelift for {cpu} whose vectors go up \
         to {}; main.wat is WebAssembly 1.0 with {}, and no tables or references",
        machine.os,
        machine.arch,
        counted(machine.cores, "core"),
        engine::ENGINE_VERSION,
        simd_level(machine),
        listed(&dialect_features()),
    )
}

/// The next step after main.wat's own problems.
fn fix_code(folder: &ProgramFolder) -> String {
    format!(
        "Next: fix {folder}/{WASM_CODE_FILE} with the compile skill (the full rules are in its \
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

/// Everything a checking job needs about the main.wat it tests.
struct WasmCode {
    /// The idea.
    idea: Idea,
    /// Its tests.
    suite: TestSuite,
    /// The module main.wat builds, kept for the workers.
    program: Arc<Program>,
    /// The module's SHA-256.
    module: Sha256Hex,
    /// The WebAssembly features the module uses beyond WebAssembly 1.0.
    features: Vec<String>,
    /// The deepest call path's locals and operands, as the rules count them.
    max_stack_bytes: u64,
    /// How many `;; unsafe:` blocks main.wat has.
    unsafe_blocks: usize,
}

/// What keeps main.wat from building on its idea's parts, as for main.s: a part that isn't
/// checked on this computer or has no main.wat, a function taken from one that isn't its current
/// code word for word, a function that names a part the idea doesn't build on, and a part the
/// idea builds on that main.wat takes nothing from.
fn parts_problems(
    data_dir: &DataDir,
    folder: &ProgramFolder,
    idea: &Idea,
    code: &str,
) -> Vec<String> {
    let mut problems = Vec::new();
    let mut used: Vec<&idea::Part> = Vec::new();
    let mut checked: Vec<String> = Vec::new();
    for function in parts::functions(code) {
        let Some(from) = &function.from else {
            continue;
        };
        let wanted = idea::folder_of_link(from);
        let Some(part) = idea.parts.iter().find(|part| part.folder() == wanted) else {
            problems.push(format!(
                "{WASM_CODE_FILE}'s ${} says `;; from: {from}`, a part {IDEA_FILE} doesn't build \
                 on: add `[{wanted}]({wanted}/idea.md)` to its `- Builds on:` line, or drop the \
                 line",
                function.name
            ));
            continue;
        };
        if !used.contains(&part) {
            used.push(part);
        }
        // The part's own path, `..` resolved, as a program folder takes it.
        let joined = folder.path().join(part.folder());
        let part_folder = std::fs::canonicalize(&joined).unwrap_or(joined);
        let shown = part_folder.display().to_string();
        if !checked.contains(&part.idea) {
            checked.push(part.idea.clone());
            let ready = part_folder
                .to_str()
                .and_then(|path| super::folder_of(path).ok())
                .and_then(|part_folder| state_of(data_dir, &part_folder).ok())
                .is_some_and(|state| state == State::Ready);
            if !ready {
                problems.push(format!(
                    "the part [{}]({}) isn't checked on this computer: call check with path \
                     {shown} until it's done, then check this program",
                    part.name, part.idea
                ));
                continue;
            }
        }
        let Ok(theirs) = std::fs::read_to_string(part_folder.join(WASM_CODE_FILE)) else {
            problems.push(format!(
                "the part [{}]({}) has no {WASM_CODE_FILE}: {WASM_CODE_FILE} takes functions only \
                 from parts in WebAssembly; make the part an Attested Computation compiled to \
                 {WASM_CODE_FILE} and call check with path {shown} until it's done, or write ${} \
                 in {WASM_CODE_FILE} yourself, without its `;; from:` line, and drop the part \
                 from `- Builds on:`",
                part.name, part.idea, function.name
            ));
            continue;
        };
        match parts::functions(&theirs)
            .into_iter()
            .find(|their| their.name == function.name)
        {
            None => problems.push(format!(
                "{WASM_CODE_FILE}'s ${} says `;; from: {from}`, but {shown}/{WASM_CODE_FILE} has \
                 no ${}: take it from the part under the name it has there",
                function.name, function.name
            )),
            Some(their) if their.body != function.body => problems.push(format!(
                "{WASM_CODE_FILE}'s ${} differs from {shown}/{WASM_CODE_FILE}'s, where it comes \
                 from: copy it again word for word, keeping its `;; from: {from}` line",
                function.name
            )),
            Some(_) => {}
        }
    }
    for part in &idea.parts {
        if !used.contains(&part) {
            problems.push(format!(
                "{IDEA_FILE} builds on [{}]({}), but {WASM_CODE_FILE} takes nothing from it: copy \
                 each function it uses with a `;; from: {}` line above it, or drop the part from \
                 `- Builds on:`",
                part.name,
                part.idea,
                part.folder()
            ));
        }
    }
    problems
}

/// Checks that `folder` holds main.wat lengua may run, as `run` does from MCP and from a shell:
/// its idea and code match lengua.lock, whose stamp is main.wat's, and the module built from it
/// now is byte for byte the one check tested. Environment variables reach only a program whose
/// idea uses the environment. Gives the build and the idea.
fn runnable(
    data_dir: &DataDir,
    folder: &ProgramFolder,
    env: &BTreeMap<String, String>,
) -> Result<(Built, Idea), String> {
    let state = state_of(data_dir, folder)?;
    if state != State::Ready {
        return Err(format!(
            "lengua runs a program only when its idea and code match lengua.lock, and {folder} \
             doesn't. {}",
            state.next_step_in(folder, WASM_CODE_FILE)
        ));
    }
    let key = Key::load(data_dir)
        .map_err(|error| error.to_string())?
        .ok_or("lengua's signing key is missing; call check")?;
    let lock = lock::read(folder.path(), Some(&key))
        .map_err(|error| format!("lengua.lock can't be trusted: {error}"))?
        .ok_or("lengua.lock is missing; call check")?;
    let stamp = lock
        .code
        .as_ref()
        .filter(|stamp| stamp.is_wasm())
        .ok_or_else(|| format!("{WASM_CODE_FILE} isn't checked; call check with path {folder}."))?;
    let code = read_optional(folder, WASM_CODE_FILE)?.ok_or_else(|| {
        format!("There's no {WASM_CODE_FILE} in {folder}. Write it with the compile skill.")
    })?;
    let idea = read_idea(folder)
        .and_then(|text| idea::parse(&text).map_err(|problems| idea_problems(folder, &problems)))?;
    if !env.is_empty() && !idea.uses.contains(&Use::Environment) {
        return Err(format!(
            "{IDEA_FILE} doesn't use the environment, so the program gets no environment \
             variables; leave env out, or add `the environment` to its `- Uses:` line"
        ));
    }
    let built = text::build(&code)
        .map_err(|problem| format!("{WASM_CODE_FILE} doesn't build: {problem}"))?;
    if Sha256Hex::of(&built.wasm) != stamp.executable_sha256 {
        return Err(format!(
            "This build of {WASM_CODE_FILE} differs from the module check tested, as after lengua \
             itself changed, so it won't run. Call check with path {folder} to test this build."
        ));
    }
    Ok((built, idea))
}

/// The bytes of `name`, a file inside `folder`, as a run's standard input: at most
/// [`MAX_STDIN_FILE`], since main.wat's worker gets them with the job.
fn read_stdin_file(folder: &Path, name: &str) -> Result<Vec<u8>, String> {
    let unreadable = |error: std::io::Error| format!("stdin_file {name}: {error}");
    let inside = folder.join(name).canonicalize().map_err(unreadable)?;
    if !inside.starts_with(folder) || !inside.is_file() {
        return Err(format!("stdin_file {name} must be a file inside folder"));
    }
    let size = std::fs::metadata(&inside).map_err(unreadable)?.len();
    if size > MAX_STDIN_FILE {
        return Err(format!(
            "stdin_file {name} holds {size} bytes, and main.wat's standard input from a file \
             holds at most {} MiB: give it a smaller file",
            MAX_STDIN_FILE / 1024 / 1024
        ));
    }
    std::fs::read(&inside).map_err(unreadable)
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

/// `lengua-mcp run` for main.wat: `run`'s checks, then the module in this process, attached to
/// the shell's standard input, output and error, with no time limit, in `workspace`. Gives the
/// program's exit code, or [`TRAPPED`] when it trapped, which it says on standard error.
///
/// # Errors
///
/// Why lengua won't run it, or can't.
pub(super) fn run_from_shell(
    data_dir: &DataDir,
    folder: &ProgramFolder,
    request: &ShellRun,
    workspace: &Path,
) -> Result<i32, String> {
    let (built, idea) = runnable(data_dir, folder, &request.env)?;
    let connections: Vec<Made> = reaches(&idea.network)
        .iter()
        .map(|reach| made(reach, &request.args))
        .collect();
    let cache = engine::cache_folder(data_dir);
    let here = Here {
        args: &request.args,
        env: &request.env,
        folder: workspace,
        policy: Policy::of(&idea.uses, &idea.network),
        listen: idea
            .network
            .listens
            .map(|port| (port, idea.network.serves_anywhere)),
        connections: &connections,
        cache: Some(cache.as_path()),
        built: Some(&built),
    };
    match run_here(&built.wasm, &here)
        .map_err(|error| format!("couldn't run the program: {error}"))?
    {
        End::Exited(code) => Ok(code),
        trapped @ End::Trapped(..) => {
            eprintln!(
                "lengua-mcp: the program {}",
                trapped.describe(Limits::default())
            );
            Ok(TRAPPED)
        }
        other => Err(other.describe(Limits::default())),
    }
}

impl LenguaServer {
    /// `check`'s stages for an Attested Computation, `idea_text`, up to the first that needs
    /// work, as for main.s: the idea stage; main.wat built, held to its parts and checked against
    /// every rule; its tests in the background; and, once they pass, lengua's entry in idea.md's
    /// `verified`, which every check takes away first once the idea or the code no longer is what
    /// it names ([`Self::keep_verified`]).
    pub(super) async fn check_wasm(
        &self,
        folder: &ProgramFolder,
        params: &CheckParams,
        events: &mut Vec<graph::Event>,
        (began, machine): (Instant, &Machine),
        idea_text: &str,
    ) -> Result<CheckReport, String> {
        let (idea, suite) = match self
            .idea_stage(folder, params, events, idea_text, WASM_CODE_FILE)
            .await?
        {
            IdeaStage::Recorded(recorded) => *recorded,
            IdeaStage::Stops(report) => return Ok(*report),
        };
        if let Some(report) = code_file_problem(folder, true) {
            return Ok(report);
        }
        let Some(code) = read_optional(folder, WASM_CODE_FILE)? else {
            return Ok(CheckReport {
                machine: Some(machine_line(machine)),
                ..CheckReport::new(
                    "write_code",
                    State::CodeMissing.next_step_in(folder, WASM_CODE_FILE),
                )
            });
        };
        let idea_hash = trust::idea_hash(idea_text.as_bytes());
        let files = [idea_hash, Sha256Hex::of(code.as_bytes())];
        let checking = match self.current_checking(folder, &files, false).await {
            Some(checking) => checking,
            None => match self.wasm_code(folder, &code, idea, suite) {
                Ok(code) => self.start_wasm_checking(folder, files, code).await,
                Err(problems) => {
                    return Ok(CheckReport::problems("code_problems", problems, fix_code(folder)));
                }
            },
        };
        wait_for(&checking, began + check_wait()).await;
        if *checking.stage.borrow() == Stage::Ended {
            self.write_header(folder, &checking).await;
        }
        let report = self.checking_report(folder, &checking);
        if report.state == "done" {
            events.push(graph::Event::Done);
        }
        Ok(report)
    }

    /// main.wat's `code` built, held to its parts and checked against every rule of the
    /// WebAssembly dialect, ready for its tests; or what's wrong, each on its line.
    fn wasm_code(
        &self,
        folder: &ProgramFolder,
        code: &str,
        idea: Idea,
        suite: TestSuite,
    ) -> Result<WasmCode, Vec<String>> {
        let built = text::build(code).map_err(|problem| vec![problem.to_string()])?;
        let from_parts = parts_problems(&self.data_dir, folder, &idea, code);
        if !from_parts.is_empty() {
            return Err(from_parts);
        }
        let context = checker::wasm::Context {
            uses: &idea.uses,
            network: &idea.network,
        };
        let report = checker::wasm::check(code, &built, &context);
        if !report.passed() {
            return Err(report.lines());
        }
        let features = engine::features_used(&built.wasm);
        let module = Sha256Hex::of(&built.wasm);
        let program = Program::new(built)
            .map_err(|error| vec![format!("lengua couldn't keep main.wat's module: {error}")])?;
        Ok(WasmCode {
            idea,
            suite,
            program: Arc::new(program),
            module,
            features,
            max_stack_bytes: report.facts.max_stack_bytes,
            unsafe_blocks: report.unsafe_blocks.len(),
        })
    }

    /// Starts testing `code` in the background, stopping any job on older files.
    async fn start_wasm_checking(
        &self,
        folder: &ProgramFolder,
        files: [Sha256Hex; 2],
        code: WasmCode,
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
            code: WASM_CODE_FILE,
            executable: code.module.clone(),
            cancel: Arc::default(),
            found: std::sync::Mutex::new(Found {
                proofs,
                ..Found::default()
            }),
            stage: watch::Sender::new(Stage::Testing),
            green_told: AtomicBool::new(false),
            further: false,
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
            server.test_wasm(&folder, &job, code).await;
            job.stage.send_replace(Stage::Ended);
        });
        checking
    }

    /// The job itself for main.wat: every test, in the pool's workers; once they all pass, the
    /// code's stamp in lengua.lock and lengua's entry in idea.md's `verified`. No proof runs:
    /// each generated test's line says so.
    async fn test_wasm(&self, folder: &ProgramFolder, checking: &Checking, code: WasmCode) {
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
            testrun::workers(None),
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(error) => return fail(vec![format!("couldn't run the program: {error}")]),
        };
        if !outcome.passed() {
            return fail(outcome.failures);
        }
        let stamp = CodeStamp {
            sha256: code_hash,
            executable_sha256: checking.executable.clone(),
            toolchain: engine::ENGINE.to_owned(),
            max_stack_bytes: code.max_stack_bytes,
            unsafe_blocks: u32::try_from(code.unsafe_blocks).unwrap_or(u32::MAX),
            target: lock::WASM_TARGET.to_owned(),
            features: code.features.clone(),
            emulator: None,
        };
        if let Err(error) = self.stamp_code(folder, &idea_hash, stamp).await {
            return fail(vec![error]);
        }
        if checking.cancel.load(Ordering::Relaxed) {
            return;
        }
        // lengua's entry first, so a reply that says done finds it there.
        self.write_verified(folder, checking).await;
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
        let performance: Vec<String> = outcome
            .costs
            .iter()
            .map(|cost| cost_line_in(cost, "WebAssembly operations"))
            .collect();
        if let Ok(mut found) = checking.found.lock() {
            for proof in &mut found.proofs {
                proof.detail = Some(no_proof());
            }
            found.passed = Some(Passed {
                runs: outcome.runs,
                every_value,
                performance,
                shown: outcome.shown,
                header: None,
                under: None,
            });
        }
    }

    /// Writes lengua's entry into idea.md's `verified` once main.wat's tests pass: lengua and its
    /// version, the time, and the idea and the code it checked, provided both files are still
    /// those and lengua's entry doesn't say so already. idea.md's hash leaves `verified` out, so
    /// the lock still holds.
    pub(super) async fn write_verified(&self, folder: &ProgramFolder, checking: &Checking) {
        let [idea, code] = &checking.files;
        let _writing = self.lock_writes.lock().await;
        let path = folder.file(IDEA_FILE);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let current = std::fs::read(folder.file(WASM_CODE_FILE))
            .ok()
            .map(|bytes| Sha256Hex::of(&bytes));
        if trust::idea_hash(text.as_bytes()) != *idea || current.as_ref() != Some(code) {
            return;
        }
        if let [entry] = trust::lengua_entries(&text).as_slice()
            && entry.by == trust::ACTOR
            && entry.idea == *idea
            && entry.code == *code
        {
            return;
        }
        let entry = trust::Verified::new(trust::utc(now_seconds()), idea.clone(), code.clone());
        let written = trust::with_lengua_verified(&text, &entry);
        if written != text
            && let Err(error) = replace(&path, &written)
        {
            eprintln!(
                "lengua: couldn't write lengua's entry in {}: {error}",
                path.display()
            );
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
        let code = std::fs::read(folder.file(WASM_CODE_FILE))
            .ok()
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
        if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
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

    /// `run` for main.wat: in a worker of its own, in the engine's configuration for runs (a
    /// deadline, nothing counted), reaching only what its idea allows, in `folder` or a fresh
    /// one; a server stays serving in the background. Its reply is native's.
    pub(super) async fn run_wasm(
        &self,
        folder: &ProgramFolder,
        params: &RunParams,
    ) -> Result<RunReport, String> {
        let (built, idea) = runnable(&self.data_dir, folder, &params.env)?;
        let workspace = params.folder.as_deref().map(working_folder).transpose()?;
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
        let assertions = built.assertions.clone();
        let program = Arc::new(
            Program::new(built)
                .map_err(|error| format!("lengua couldn't keep the module: {error}"))?,
        );
        let connections = reaches(&idea.network);
        let spec = Spec {
            folder: workspace.as_deref(),
            env: &params.env,
            connections: &connections,
            listen: idea
                .network
                .listens
                .map(|port| (port, idea.network.serves_anywhere)),
            setup: Setup::Run,
            max_written: None,
            alone: true,
            ..Spec::new(&params.args, &stdin, Policy::of(&idea.uses, &idea.network))
        };
        if idea.network.listens.is_some() {
            return self
                .serve_wasm_in_background(folder, &program, spec, assertions)
                .await;
        }
        let limits = Limits {
            timeout: Duration::from_secs(seconds),
            max_file_size: None,
            ..Limits::default()
        };
        let run = self
            .pool()
            .run(&program, spec, limits)
            .await
            .map_err(|error| format!("couldn't run the program: {error}"))?;
        Ok(ended_report(&run, &assertions, limits, WASM_CODE_FILE))
    }

    /// Starts main.wat's server in a worker of its own and leaves it serving, at most one per
    /// program folder, as `run` does a native one; one that never came to serve is stopped, and
    /// the reply says how it ended.
    async fn serve_wasm_in_background(
        &self,
        folder: &ProgramFolder,
        program: &Arc<Program>,
        spec: Spec<'_>,
        assertions: BTreeMap<i64, usize>,
    ) -> Result<RunReport, String> {
        let mut background = self.background.lock().await;
        already_serving(&background, folder)?;
        let limits = background_limits();
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
            return Ok(ended_report(&run, &assertions, limits, WASM_CODE_FILE));
        }
        let connections = running
            .connections()
            .iter()
            .map(ConnectionReport::of)
            .collect();
        let server = Serving::Wasm(Box::new(running), Arc::clone(program));
        Ok(self.keep_serving(
            &mut background,
            folder,
            server,
            (assertions, WASM_CODE_FILE),
            connections,
        ))
    }
}

/// What `lengua-mcp check` found in a folder, as the idea's attester (OKF v0.2 §10).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attestation {
    /// What it found, one line a finding, and its verdict last: `attested` or `not attested`.
    pub lines: Vec<String>,
    /// Whether the folder is attested: its idea and main.wat build, follow every rule and pass
    /// every test, and lengua's entry in the idea's `verified` names them both.
    pub attested: bool,
}

/// Attests the program folder at `path`, absolute or from the current folder, as the attester an
/// Attested Computation names: reads its idea.md and main.wat, builds main.wat, checks it
/// against every rule, runs every test of the idea, and compares lengua's entry in the idea's
/// `verified` with the files. It writes nothing (no lock, no index.md, no log.md) and needs no
/// lock and no signing key, so it attests a folder on any computer.
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
    let (idea_text, code) = match (
        read_optional(&folder, IDEA_FILE),
        read_optional(&folder, WASM_CODE_FILE),
    ) {
        (Ok(Some(idea_text)), Ok(Some(code))) => (idea_text, code),
        (idea_text, code) => {
            for (name, read) in [(IDEA_FILE, idea_text), (WASM_CODE_FILE, code)] {
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
            lines.extend(problems.iter().map(|problem| format!("{IDEA_FILE} {problem}")));
            return false;
        }
    };
    if idea.attested.is_none() {
        lines.push(format!(
            "{IDEA_FILE} is `type: {}`, compiled to {CODE_FILE}: lengua-mcp check attests an \
             Attested Computation, compiled to {WASM_CODE_FILE}",
            idea::IDEA_TYPE
        ));
        return false;
    }
    if let Some(problem) = threads_problem(&idea_text, &idea) {
        lines.push(format!("{IDEA_FILE} {problem}"));
        return false;
    }
    let suite = match testfile::check(&idea) {
        Ok(suite) => suite,
        Err(problems) => {
            lines.extend(problems.iter().map(|problem| format!("{IDEA_FILE} {problem}")));
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
        "{WASM_CODE_FILE}: builds into a module of {} bytes, sha256:{}",
        built.wasm.len(),
        Sha256Hex::of(&built.wasm)
    ));
    let context = checker::wasm::Context {
        uses: &idea.uses,
        network: &idea.network,
    };
    let report = checker::wasm::check(&code, &built, &context);
    if !report.passed() {
        lines.extend(report.lines());
        return false;
    }
    lines.push(format!(
        "{WASM_CODE_FILE}: follows every rule of lengua's WebAssembly dialect"
    ));
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
    // Workers of its own, with no cache: the attester writes nothing.
    let pool = Pool::new(launcher, None);
    match testrun::run_wasm_tests(
        &suite,
        &pool,
        program,
        seed,
        &idea.uses,
        &idea.network,
        testrun::workers(None),
    )
    .await
    {
        Ok(outcome) if outcome.passed() => lines.push(format!(
            "tests: passed all {} test runs, the generated ones drawn from seed {seed}",
            outcome.runs
        )),
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
                    "verified: {}'s entry names the code {}, and {WASM_CODE_FILE} is {} now",
                    entry.by,
                    entry.code.label(),
                    code.label()
                ));
                named = false;
            }
            if named {
                lines.push(format!(
                    "verified: {} checked this {IDEA_FILE} and this {WASM_CODE_FILE} at {}",
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
    const IDEA: &str = include_str!("../../tests/fixtures-wasm/add/idea.md");

    #[test]
    fn an_attested_computation_s_code_is_main_wat() {
        assert!(attested(IDEA));
        let native = IDEA.replace(
            "type: Attested Computation",
            &format!("type: {}", idea::IDEA_TYPE),
        );
        assert!(!attested(&native));
        assert!(!attested("# Purpose\n"));
        // YAML that can't be read still says which code the idea asks for, so the idea stage
        // reports the YAML's problem on every system.
        let unreadable = IDEA.replace("title: add", "title: [add");
        assert!(graph::okf::frontmatter(&unreadable).is_none());
        assert!(attested(&unreadable));
        assert!(!attested(&native.replace("title: add", "title: [add")));
        let quoted = unreadable.replace(
            "type: Attested Computation",
            "type: \"Attested Computation\" # OKF",
        );
        assert!(attested(&quoted));
    }

    #[test]
    fn one_code_file_and_the_idea_s_own() {
        let program = tempfile::tempdir().expect("a folder");
        let folder =
            ProgramFolder::new(program.path().to_str().expect("UTF-8")).expect("absolute");
        assert!(code_file_problem(&folder, true).is_none());
        std::fs::write(folder.file(WASM_CODE_FILE), "(module)").expect("write main.wat");
        assert!(code_file_problem(&folder, true).is_none());
        let other = code_file_problem(&folder, false).expect("main.wat for main.s's idea");
        assert_eq!(other.state, "code_problems");
        assert!(
            other.problems[0].contains("make idea.md an Attested Computation"),
            "{other:?}"
        );
        assert!(
            other.next_step.contains("Attested Computation"),
            "{other:?}"
        );
        std::fs::write(folder.file(CODE_FILE), "_start:\n").expect("write main.s");
        for wasm in [true, false] {
            let both = code_file_problem(&folder, wasm).expect("both code files");
            assert!(
                both.problems[0].contains("holds both main.s and main.wat"),
                "{both:?}"
            );
        }
        let both = code_file_problem(&folder, true).expect("both code files");
        let wanted = format!("Next: delete {folder}/main.s, then call check");
        assert!(both.next_step.starts_with(&wanted), "{both:?}");
        // With main.s alone, the next step makes main.wat first: there is none to keep.
        std::fs::remove_file(folder.file(WASM_CODE_FILE)).expect("remove main.wat");
        let other = code_file_problem(&folder, true).expect("main.s for main.wat's idea");
        assert!(
            other.problems[0].contains("compile idea.md into main.wat"),
            "{other:?}"
        );
        let wanted = format!("Next: compile {folder}/idea.md into {folder}/main.wat");
        assert!(other.next_step.starts_with(&wanted), "{other:?}");
        assert!(code_file_problem(&folder, false).is_none());
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
}
