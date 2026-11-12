//! The prover's model of WebAssembly against what lengua's host and engine do, on every system:
//! every case of `tests/host-cases/` run in the model gives what its file says, as the host's own
//! runs do (tests/host.rs); on every test of the ideas of the programs the model follows in full,
//! their floats and SIMD128 among them, and of the other programs of examples/ and tests/fixtures
//! and the compile skill's WebAssembly templates where the model follows what they do, the model
//! and the engine do the same; and each float and SIMD128 operator the model follows gives the
//! engine's bits on chosen and random operands.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use lengua::prove::wasm::{Ended, Given, Module, Ran, run};
use lengua::runner::{End, Limits, Run, Trap};
use lengua::supply::{Supply, Time};
use lengua::testfile::{Case as Sample, Example, TestSuite};
use lengua::wasm::job::unhex;
use lengua::wasm::{Policy, Pool, Program, Spec};
use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};
use serde::Deserialize;

/// What a host case gives its program, and what the program must do, as tests/host.rs reads it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// What the case shows, in words.
    about: String,
    /// The idea's Uses words.
    #[serde(default)]
    uses: Vec<String>,
    /// An idea's `- Network:` line, after the colon.
    #[serde(default)]
    network: String,
    /// The arguments, after the program's name.
    #[serde(default)]
    args: Vec<String>,
    /// The environment variables.
    #[serde(default)]
    env: BTreeMap<String, String>,
    /// Standard input, as text.
    stdin: Option<String>,
    /// Standard input, as hex.
    stdin_hex: Option<String>,
    /// The working folder's files before the run: a name ending in `/` is a folder.
    #[serde(default)]
    files: BTreeMap<String, String>,
    /// A supplied clock reading, in nanoseconds since 1970.
    clock_ns: Option<u64>,
    /// Supplied random bytes, in hex.
    random_hex: Option<String>,
    /// What the program must do.
    expect: Expected,
}

/// What a host case's program must do.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// The exit code.
    exit: i32,
    /// Standard output, as text.
    stdout: Option<String>,
    /// Standard output, as hex.
    stdout_hex: Option<String>,
    /// Standard error, exactly.
    #[serde(default)]
    stderr: String,
    /// The working folder's files and folders after the run, exactly, when given.
    files: Option<BTreeMap<String, String>>,
}

/// The files and the folders a case's table names.
fn split(table: &BTreeMap<String, String>) -> (BTreeMap<String, String>, BTreeSet<String>) {
    let mut files = BTreeMap::new();
    let mut folders = BTreeSet::new();
    for (name, contents) in table {
        match name.strip_suffix('/') {
            Some(folder) => {
                folders.insert(folder.to_owned());
            }
            None => {
                files.insert(name.clone(), contents.clone());
            }
        }
    }
    (files, folders)
}

/// Bytes as text, for a message.
fn shown(bytes: &[u8]) -> String {
    format!("{:?}", String::from_utf8_lossy(bytes))
}

/// The system's clock now, as the host reads it where a test supplies none: one reading of it
/// stands for every reading a case's run makes.
fn now() -> (i64, u32) {
    let since = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    (
        i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
        since.subsec_nanos(),
    )
}

/// Runs the host case `case`, of the module `wat`, in the model, and says what it did that the
/// case doesn't.
fn check(wat: &str, case: &Case) -> Result<(), String> {
    let built = lengua::wasm::text::build(wat).map_err(|problem| problem.to_string())?;
    let words: Vec<&str> = case.uses.iter().map(String::as_str).collect();
    let (uses, network) = lengua::idea::constraints(&words, &case.network).map_err(|problems| {
        problems
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let module = Module::new(&built, Policy::of(&uses, &network))?;
    let stdin = match (&case.stdin, &case.stdin_hex) {
        (Some(_), Some(_)) => return Err("a case gives stdin or stdin_hex, not both".to_owned()),
        (Some(text), None) => text.as_bytes().to_vec(),
        (None, Some(hex)) => unhex(hex)?,
        (None, None) => Vec::new(),
    };
    let (files, folders) = split(&case.files);
    let clock = match case.clock_ns {
        Some(nanoseconds) => (
            i64::try_from(nanoseconds / 1_000_000_000).unwrap_or(i64::MAX),
            u32::try_from(nanoseconds % 1_000_000_000).unwrap_or(0),
        ),
        None => now(),
    };
    let given = Given {
        args: case
            .args
            .iter()
            .map(|arg| arg.as_bytes().to_vec())
            .collect(),
        env: case.env.clone(),
        stdin,
        files: files
            .iter()
            .map(|(name, text)| (name.clone(), text.as_bytes().to_vec()))
            .collect(),
        folders,
        random: case.random_hex.as_deref().map(unhex).transpose()?,
        clock: vec![clock],
        connections: network.connections.len(),
        listens: network.listens.is_some(),
        stdout_closed: false,
        reversed: false,
    };
    let ran = run(&module, &given)?;
    let mut problems = Vec::new();
    if ran.end != Ended::Exit(u8::try_from(case.expect.exit).unwrap_or(u8::MAX)) {
        problems.push(format!(
            "it ended {:?}, where the case wants exit code {}",
            ran.end, case.expect.exit
        ));
    }
    let stdout = match (&case.expect.stdout, &case.expect.stdout_hex) {
        (Some(_), Some(_)) => return Err("a case expects stdout or stdout_hex, not both".into()),
        (Some(text), None) => text.as_bytes().to_vec(),
        (None, Some(hex)) => unhex(hex)?,
        (None, None) => Vec::new(),
    };
    if ran.stdout != stdout {
        problems.push(format!(
            "stdout was {}, where the case wants {}",
            shown(&ran.stdout),
            shown(&stdout)
        ));
    }
    if ran.stderr != case.expect.stderr.as_bytes() {
        problems.push(format!(
            "stderr was {}, where the case wants {}",
            shown(&ran.stderr),
            shown(case.expect.stderr.as_bytes())
        ));
    }
    if let Some(wanted) = &case.expect.files {
        let (wanted_files, wanted_folders) = split(wanted);
        let found: BTreeMap<String, String> = ran
            .files
            .iter()
            .map(|(name, bytes)| (name.clone(), String::from_utf8_lossy(bytes).into_owned()))
            .collect();
        if found != wanted_files {
            problems.push(format!(
                "its folder held the files {found:?}, where the case wants {wanted_files:?}"
            ));
        }
        // A folder that holds a file the case names is named with it.
        let named = |folder: &String| {
            wanted_folders.contains(folder)
                || wanted_files
                    .keys()
                    .chain(&wanted_folders)
                    .any(|name| name.starts_with(&format!("{folder}/")))
        };
        let unnamed = ran.folders.iter().any(|folder| !named(folder));
        let missing = wanted_folders
            .iter()
            .any(|folder| !ran.folders.contains(folder));
        if unnamed || missing {
            problems.push(format!(
                "its folder held the folders {:?}, where the case wants {wanted_folders:?}",
                ran.folders
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// Every case under `tests/host-cases/`: its module's file and its own.
fn case_files() -> Vec<(PathBuf, PathBuf)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("host-cases");
    let Ok(functions) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for function in functions.flatten() {
        let Ok(entries) = std::fs::read_dir(function.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "toml")
            {
                found.push((path.with_extension("wat"), path));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn every_host_case_runs_in_the_model_as_its_file_says() {
    let cases = case_files();
    assert!(
        cases.len() >= 100,
        "tests/host-cases holds {} cases",
        cases.len()
    );
    let mut failures = Vec::new();
    for (wat, toml) in &cases {
        let outcome = match (std::fs::read_to_string(wat), std::fs::read_to_string(toml)) {
            (Ok(wat), Ok(text)) => match toml::from_str::<Case>(&text) {
                Ok(case) => {
                    check(&wat, &case).map_err(|problem| format!("{}: {problem}", case.about))
                }
                Err(error) => Err(format!("the case doesn't read: {error}")),
            },
            (Err(error), _) => Err(format!("its module doesn't read: {error}")),
            (_, Err(error)) => Err(format!("it doesn't read: {error}")),
        };
        if let Err(problem) = outcome {
            failures.push(format!("{}: {problem}", toml.display()));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases failed in the model:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

/// This binary, which the pool's workers are.
fn launcher() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lengua-mcp"))
}

/// The programs the model must follow on every test of their ideas, their floats and SIMD128
/// among them, by their folders from the repository's root. add, echo and greet here are the
/// prover's small fixtures in tests/fixtures; the full ports of the native ideas in examples/ are
/// compared as every other program is. Of every other WebAssembly program, the ports and the
/// templates, a run the model doesn't follow yet, such as one too long for it, is listed with its
/// reason rather than failed: the engine runs every test of theirs in tests/wasm_every_program.rs
/// and tests/wasm_templates.rs.
const FOLLOWED_IN_FULL: [&str; 8] = [
    "examples/aes",
    "examples/count",
    "examples/hypotenuse",
    "tests/fixtures/add",
    "tests/fixtures/echo",
    "tests/fixtures/greet",
    "tests/fixtures/lines",
    "tests/fixtures/padded",
];

/// The fewest programs of examples/ and tests/fixtures that [`programs`] finds: the 15 examples
/// and the 29 fixtures. The templates come besides.
const AT_LEAST: usize = 44;

/// Whether the program in `folder` is one of [`FOLLOWED_IN_FULL`].
fn followed_in_full(folder: &Path) -> bool {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    folder
        .strip_prefix(root)
        .is_ok_and(|relative| FOLLOWED_IN_FULL.map(Path::new).contains(&relative))
}

/// The most input, in bytes, a test here runs in the model and on the engine to compare them: a
/// mebibyte's run takes the model seconds, and the tests of the programs that read that much
/// would take this test past an hour. The engine runs every test of every program in
/// tests/wasm_every_program.rs, and the longer ones take the same code round more times.
const MODEL_INPUT: usize = 65_536;

/// Every WebAssembly program whose ideas' tests the model must run as the engine does: each
/// folder of `examples/` and `tests/fixtures/` with an idea.md and a main.wat, and each of the
/// compile skill's templates `*-template.wat` with its `*-template-idea.md`.
fn programs() -> Vec<(PathBuf, PathBuf)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    for place in ["examples", "tests/fixtures"] {
        for entry in std::fs::read_dir(root.join(place)).expect("a folder of programs") {
            let folder = entry.expect("an entry").path();
            if folder.join("main.wat").is_file() && folder.join("idea.md").is_file() {
                found.push((folder.join("idea.md"), folder.join("main.wat")));
            }
        }
    }
    assert!(
        found.len() >= AT_LEAST,
        "examples/ and tests/fixtures hold {} programs",
        found.len()
    );
    let assets = root.join("plugin/skills/compile/assets");
    if let Ok(entries) = std::fs::read_dir(&assets) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(stem) = path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_suffix("-template.wat"))
            else {
                continue;
            };
            // A template whose idea once had the native lengua's beside it keeps `-wat-idea.md`.
            let idea = [
                format!("{stem}-template-wat-idea.md"),
                format!("{stem}-template-idea.md"),
            ]
            .into_iter()
            .map(|name| assets.join(name))
            .find(|idea| idea.is_file());
            if let Some(idea) = idea {
                found.push((idea, path));
            }
        }
    }
    found.sort();
    found
}

/// One test's run: what it gives the program, as an example or a generated run has it.
struct Test<'a> {
    /// Which test it is, for a message.
    name: String,
    /// The arguments.
    args: &'a [String],
    /// Standard input.
    stdin: &'a str,
    /// The files before the run.
    files: &'a BTreeMap<String, String>,
    /// The folders before the run.
    folders: &'a BTreeSet<String>,
    /// The environment.
    env: &'a BTreeMap<String, String>,
    /// The random bytes and clock readings it supplies.
    supply: Supply,
    /// Whether standard output is a pipe nobody reads.
    stdout_closed: bool,
}

impl<'a> Test<'a> {
    /// An example's run.
    fn example(example: &'a Example) -> Self {
        Self {
            name: format!("{:?}'s example", example.id),
            args: &example.args,
            stdin: &example.stdin,
            files: &example.files,
            folders: &example.folders,
            env: &example.env,
            supply: example.supply(),
            stdout_closed: example.stdout_closed,
        }
    }

    /// A generated run.
    fn sample(sample: &'a Sample, block: usize) -> Self {
        Self {
            name: format!("block {block} on {:?}", sample.inputs),
            args: &sample.args,
            stdin: &sample.stdin,
            files: &sample.files,
            folders: &sample.folders,
            env: &sample.env,
            supply: sample.supply(),
            stdout_closed: sample.stdout_closed,
        }
    }

    /// What the model is given for it.
    fn given(&self) -> Given {
        Given {
            args: self
                .args
                .iter()
                .map(|arg| arg.as_bytes().to_vec())
                .collect(),
            env: self.env.clone(),
            stdin: self.stdin.as_bytes().to_vec(),
            files: self
                .files
                .iter()
                .map(|(name, text)| (name.clone(), text.as_bytes().to_vec()))
                .collect(),
            folders: self.folders.clone(),
            random: self.supply.random.clone(),
            clock: self
                .supply
                .clock
                .iter()
                .map(|time| (time.seconds, time.nanoseconds))
                .collect(),
            connections: 0,
            listens: false,
            stdout_closed: self.stdout_closed,
            reversed: false,
        }
    }
}

/// What the engine did that the model didn't, or the other way round.
fn differences(engine: &Run, model: &Ran) -> Vec<String> {
    let mut found = Vec::new();
    let ended = match (&engine.end, &model.end) {
        (End::Exited(code), Ended::Exit(exit)) => *code == i32::from(*exit),
        (End::Trapped(_, Some(line)), Ended::Trap(problem)) => {
            problem.starts_with(&format!("main.wat line {line}: "))
        }
        (End::Trapped(_, None), Ended::Trap(_)) => true,
        _ => false,
    };
    if !ended {
        found.push(format!(
            "the engine ended {:?}, the model {:?}",
            engine.end, model.end
        ));
    }
    if engine.stdout != model.stdout {
        found.push(format!(
            "stdout: the engine's {}, the model's {}",
            shown(&engine.stdout),
            shown(&model.stdout)
        ));
    }
    if engine.stderr != model.stderr {
        found.push(format!(
            "stderr: the engine's {}, the model's {}",
            shown(&engine.stderr),
            shown(&model.stderr)
        ));
    }
    match &engine.files {
        Ok(files) if *files != model.files => found.push(format!(
            "files: the engine's {:?}, the model's {:?}",
            files.keys().collect::<Vec<_>>(),
            model.files.keys().collect::<Vec<_>>()
        )),
        Ok(_) => {}
        Err(problem) => found.push(format!("the engine's folder couldn't be read: {problem}")),
    }
    if engine.folders != model.folders {
        found.push(format!(
            "folders: the engine's {:?}, the model's {:?}",
            engine.folders, model.folders
        ));
    }
    found
}

/// An example as a generated run with no inputs, as the samples the model is held to are.
fn as_sample(example: &Example) -> Sample {
    Sample {
        inputs: BTreeMap::new(),
        args: example.args.clone(),
        stdin: example.stdin.clone(),
        files: example.files.clone(),
        folders: example.folders.clone(),
        env: example.env.clone(),
        stdout: example.stdout.clone(),
        stderr: example.stderr.clone(),
        files_after: example.files_after.clone(),
        gone: example.gone.clone(),
        folders_after: example.folders_after.clone(),
        exit_code: example.exit_code,
        random: example.random.clone(),
        clock: example.clock.clone(),
        server: example.server.clone(),
        client: example.client.clone(),
        clients_at_once: example.clients_at_once,
        stdout_closed: example.stdout_closed,
        piece: example.piece,
        serves: example.serves,
        connections: example.connections,
    }
}

/// The seed every test here draws its generated runs from.
const SEED: u64 = 1;

/// Every example of `suite`, and every generated run.
fn samples(suite: &TestSuite) -> Vec<Sample> {
    let mut samples: Vec<Sample> = suite.examples.iter().map(as_sample).collect();
    for block in &suite.generated {
        samples.extend(block.cases(SEED).filter_map(Result::ok));
    }
    samples
}

#[tokio::test(flavor = "multi_thread")]
async fn the_model_does_what_the_engine_does_on_every_test_of_the_fixtures() {
    let programs = programs();
    let pool = Pool::new(launcher(), None);
    let mut failures = Vec::new();
    // What the model doesn't follow in a program but those of FOLLOWED_IN_FULL, such as the
    // vector template's SIMD or a port's floats, is listed here and not failed: the engine runs
    // every test of theirs in tests/wasm_every_program.rs and tests/wasm_templates.rs. Every test
    // of those of FOLLOWED_IN_FULL the model must follow.
    let mut not_modelled = Vec::new();
    // The tests past MODEL_INPUT, which this test leaves to the engine's own runs.
    let mut too_long = Vec::new();
    for (idea_path, wat_path) in &programs {
        let shown = wat_path.display().to_string();
        let folder = wat_path.parent();
        let template = folder.is_some_and(|folder| folder.ends_with("assets"));
        let in_full = !template && folder.is_some_and(followed_in_full);
        let idea_text = std::fs::read_to_string(idea_path).expect("the idea reads");
        let wat = std::fs::read_to_string(wat_path).expect("the program reads");
        let idea = lengua::idea::parse(&idea_text)
            .unwrap_or_else(|problems| panic!("{shown}: {problems:?}"));
        let suite = lengua::testfile::check(&idea)
            .unwrap_or_else(|problems| panic!("{shown}: {problems:?}"));
        let program = Program::of_text(&wat).unwrap_or_else(|problem| panic!("{shown}: {problem}"));
        let policy = Policy::of(&idea.uses, &idea.network);
        let built = program.built().clone();
        let networked = idea.network.listens.is_some() || !idea.network.connections.is_empty();
        if networked {
            // A test's peers talk to the engine through the test runner, and to the model through
            // its world's network: each must pass every test.
            let outcome = lengua::testrun::run_wasm_tests(
                &suite,
                &pool,
                Arc::new(program),
                SEED,
                &idea.uses,
                &idea.network,
                4,
            )
            .await
            .unwrap_or_else(|error| panic!("{shown}: {error}"));
            if !outcome.passed() {
                failures.push(format!("{shown}: the engine failed {:?}", outcome.failures));
            }
            let deadline = Instant::now() + Duration::from_secs(600);
            let listens = idea.network.listens.is_some();
            match lengua::prove::agrees_wasm(&built, policy, &samples(&suite), listens, deadline) {
                Ok(None) => {}
                Ok(Some(lengua::prove::Verdict::NotProven(why))) | Err(why) if !in_full => {
                    not_modelled.push(format!("{shown}: {why}"));
                }
                other => failures.push(format!("{shown}: the model {other:?}")),
            }
            continue;
        }
        let module = match Module::new(&built, policy) {
            Ok(module) => module,
            // A module the model doesn't start, such as one with a float global, is listed too.
            Err(why) if !in_full => {
                not_modelled.push(format!("{shown}: {why}"));
                continue;
            }
            Err(why) => panic!("{shown}: {why}"),
        };
        let mut tests: Vec<Test<'_>> = suite.examples.iter().map(Test::example).collect();
        let generated: Vec<(usize, Sample)> = suite
            .generated
            .iter()
            .enumerate()
            .flat_map(|(number, block)| {
                block
                    .cases(SEED)
                    .filter_map(Result::ok)
                    .map(move |sample| (number, sample))
            })
            .collect();
        tests.extend(
            generated
                .iter()
                .map(|(number, sample)| Test::sample(sample, *number)),
        );
        for test in &tests {
            let input = test.stdin.len()
                + test.files.values().map(String::len).sum::<usize>()
                + test.args.iter().map(String::len).sum::<usize>();
            if input > MODEL_INPUT {
                let name: String = test.name.chars().take(80).collect();
                too_long.push(format!("{shown}, {name}…: {input} bytes of input"));
                continue;
            }
            let spec = Spec {
                files: test.files,
                folders: test.folders,
                env: test.env,
                supply: Some(&test.supply),
                stdout_closed: test.stdout_closed,
                ..Spec::new(test.args, test.stdin.as_bytes(), policy)
            };
            let engine = pool
                .run(&program, spec, Limits::default())
                .await
                .unwrap_or_else(|error| panic!("{shown}: {error}"));
            let model = match run(&module, &test.given()) {
                Ok(model) => model,
                Err(why) => {
                    failures.push(format!(
                        "{shown}, {}: the model couldn't run: {why}",
                        test.name
                    ));
                    continue;
                }
            };
            if let Ended::NotModelled(why) = &model.end
                && !in_full
            {
                // A test's name holds its input, which can be a mebibyte long.
                let name: String = test.name.chars().take(80).collect();
                not_modelled.push(format!("{shown}, {name}…: {why}"));
                continue;
            }
            let found = differences(&engine, &model);
            if !found.is_empty() {
                failures.push(format!("{shown}, {}: {}", test.name, found.join("; ")));
            }
        }
    }
    for line in &not_modelled {
        lengua::skiplog::note(&format!("not modelled: {line}"));
    }
    if !too_long.is_empty() {
        lengua::skiplog::note(&format!(
            "{} tests of more than {MODEL_INPUT} bytes of input left to the engine's own runs, \
             the first {}",
            too_long.len(),
            too_long[0]
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A module that imports clock_time_get with a 32-bit precision, where WASI's is 64 bits.
const NARROW_CLOCK: &str = r#"(module
  (import "wasi_snapshot_preview1" "clock_time_get"
    (func $time (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1 1)
  (func (export "_start")
    (call $exit (call $time (i32.const 0) (i32.const 0) (i32.const 1024)))))
"#;

/// A module that imports fd_seek with a 32-bit offset, where WASI's is 64 bits.
const NARROW_SEEK: &str = r#"(module
  (import "wasi_snapshot_preview1" "fd_seek"
    (func $seek (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1 1)
  (func (export "_start")
    (call $exit (call $seek (i32.const 1) (i32.const -1) (i32.const 0) (i32.const 1024)))))
"#;

/// A module that imports proc_exit with a 64-bit code, where WASI's is 32 bits.
const WIDE_EXIT: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i64)))
  (memory (export "memory") 1 1)
  (func (export "_start")
    (call $exit (i64.const 0))))
"#;

/// A module with a table and an element segment past its end.
const TABLED: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1 1)
  (table 1 funcref)
  (func $f)
  (elem (i32.const 1) $f)
  (func (export "_start")
    (call $exit (i32.const 0))))
"#;

/// What the engine doesn't start, or traps on as it starts it, the model refuses: the model, which
/// once ran each of these to an exit, can't prove what the engine never does.
#[tokio::test(flavor = "multi_thread")]
async fn the_model_refuses_what_the_engine_doesnt_start() {
    let pool = Pool::new(launcher(), None);
    let policy = Policy {
        clock: true,
        files: true,
        ..Policy::default()
    };
    for text in [NARROW_CLOCK, NARROW_SEEK, WIDE_EXIT, TABLED] {
        let program = Program::of_text(text).unwrap_or_else(|problem| panic!("{problem}"));
        let engine = pool
            .run(&program, Spec::new(&[], &[], policy), Limits::default())
            .await
            .unwrap_or_else(|error| panic!("{error}: {text}"));
        assert!(
            !matches!(engine.end, End::Exited(0)),
            "the engine ran it: {text}"
        );
        let model = Module::new(program.built(), policy);
        assert!(model.is_err(), "the model took it: {text}");
    }
}

/// The start of a module whose `_start` calls `$deep` with DEPTH, its locals still to come.
const DEEP: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1 1)
  (func (export "_start")
    (drop (call $deep (i32.const DEPTH)))
    (call $exit (i32.const 0)))
  (func $deep (param $n i32) (result i64)
    (local $sum i64)"#;

/// A module whose `$deep` calls itself `depth` times, each call keeping `values` values of 64
/// bits across the next.
fn recursing(depth: u32, values: u32) -> String {
    let mut locals = String::new();
    let mut loads = String::new();
    let mut sums = String::new();
    for value in 0..values {
        let offset = value * 8;
        locals.push_str(&format!("\n    (local $v{value} i64)"));
        loads.push_str(&format!(
            "\n    (local.set $v{value} (i64.load offset={offset} (i32.const 1024)))"
        ));
        sums.push_str(&format!(
            "\n    (local.set $sum (i64.add (local.get $sum) (local.get $v{value})))"
        ));
    }
    let call = "\n    (if (local.get $n)\n      (then (local.set $sum\n        (call $deep \
                (i32.sub (local.get $n) (i32.const 1))))))";
    let start = DEEP.replace("DEPTH", &depth.to_string());
    format!("{start}{locals}{loads}{call}{sums}\n    (local.get $sum)))\n")
}

/// A recursion of 1001 calls, fewer than the 1024 the model once let a run make, each keeping
/// 128 values of 64 bits across the next, 1 KiB: the engine's stack, 512 KiB, runs out, and the
/// model refuses the module, as it refuses every recursion.
#[tokio::test(flavor = "multi_thread")]
async fn the_model_refuses_a_recursion_the_engines_stack_runs_out_on() {
    let pool = Pool::new(launcher(), None);
    let text = recursing(1000, 128);
    let program = Program::of_text(&text).unwrap_or_else(|problem| panic!("{problem}"));
    let spec = Spec::new(&[], &[], Policy::default());
    let engine = pool
        .run(&program, spec, Limits::default())
        .await
        .expect("the engine runs it");
    assert!(
        matches!(engine.end, End::Trapped(Trap::StackOverflow, _)),
        "{:?}",
        engine.end
    );
    let model = Module::new(program.built(), Policy::default());
    assert!(model.is_err(), "the model took it");
}

/// A module with the memory MEMORY that exits with what growing it a page gives, plus one: 2
/// where it grows, from its one page, and 0 where it doesn't.
const GROWS: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  MEMORY
  (func (export "_start")
    (call $exit (i32.add (memory.grow (i32.const 1)) (i32.const 1)))))
"#;

/// The memory grows in the model as far as the engine lets it: one the module keeps to itself to
/// its maximum, or without one to the most a 32-bit memory has; one exported as `memory` to its
/// maximum, or without one not at all.
#[tokio::test(flavor = "multi_thread")]
async fn the_memory_grows_in_the_model_as_on_the_engine() {
    let pool = Pool::new(launcher(), None);
    for memory in [
        "(memory 1)",
        "(memory 1 3)",
        "(memory (export \"memory\") 1)",
        "(memory (export \"memory\") 1 3)",
    ] {
        let text = GROWS.replace("MEMORY", memory);
        let program = Program::of_text(&text).unwrap_or_else(|problem| panic!("{problem}"));
        let spec = Spec::new(&[], &[], Policy::default());
        let engine = pool
            .run(&program, spec, Limits::default())
            .await
            .expect("the engine runs it");
        let module = Module::new(program.built(), Policy::default()).expect("the model");
        let model = run(&module, &Given::default()).expect("the model runs it");
        let found = differences(&engine, &model);
        assert!(found.is_empty(), "{memory}: {}", found.join("; "));
    }
}

/// Waits with poll_oneoff on the listener, which no client calls, and on the time of day's
/// clock, for TIMEOUT nanoseconds, or till that reading where FLAGS is 1; then exits with the
/// first event's own word: 2, the clock's.
const WAITS: &str = r#"(module
  (import "wasi_snapshot_preview1" "poll_oneoff"
    (func $poll (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1 1)
  (func (export "_start")
    ;; A read on descriptor 4, the listener, with the word 1.
    (i64.store (i32.const 1280) (i64.const 1))
    (i32.store8 (i32.const 1288) (i32.const 1))
    (i32.store (i32.const 1296) (i32.const 4))
    ;; The time of day's clock, with the word 2.
    (i64.store (i32.const 1328) (i64.const 2))
    (i32.store8 (i32.const 1336) (i32.const 0))
    (i32.store (i32.const 1344) (i32.const 0))
    (i64.store (i32.const 1352) (i64.const TIMEOUT))
    (i32.store16 (i32.const 1368) (i32.const FLAGS))
    (drop (call $poll (i32.const 1280) (i32.const 1216) (i32.const 2) (i32.const 1064)))
    (call $exit (i32.wrap_i64 (i64.load (i32.const 1216))))))
"#;

/// A wait on poll_oneoff beside the listener, which no client calls, ends in the model as on the
/// engine, the test's clock at a second: a wait of a second, and a time the clock has passed,
/// come, and the program exits with the clock's word; a wait of 5 seconds, past a test's time
/// limit, and a time still to come on the test's clock never do, so the engine stops the run at
/// the limit, and in the model the run waits for what doesn't come.
#[tokio::test(flavor = "multi_thread")]
async fn a_wait_beside_the_listener_ends_in_the_model_as_on_the_engine() {
    let pool = Pool::new(launcher(), None);
    let policy = Policy {
        clock: true,
        listens: true,
        ..Policy::default()
    };
    let reading = Time {
        seconds: 1,
        nanoseconds: 0,
    };
    let supply = Supply {
        random: None,
        clock: vec![reading],
    };
    let given = Given {
        clock: vec![(1, 0)],
        listens: true,
        ..Given::default()
    };
    let waits = [
        ("1000000000", "0"),
        ("500000000", "1"),
        ("5000000000", "0"),
        ("2000000000", "1"),
    ];
    for (timeout, flags) in waits {
        let text = WAITS.replace("TIMEOUT", timeout).replace("FLAGS", flags);
        let program = Program::of_text(&text).unwrap_or_else(|problem| panic!("{problem}"));
        let spec = Spec {
            supply: Some(&supply),
            listen: Some((0, false)),
            ..Spec::new(&[], &[], policy)
        };
        let engine = pool
            .run(&program, spec, Limits::default())
            .await
            .expect("the engine runs it");
        let module = Module::new(program.built(), policy).expect("the model");
        let model = run(&module, &given).expect("the model runs it");
        let alike = match (engine.end, &model.end) {
            (End::Exited(code), Ended::Exit(exit)) => code == i32::from(*exit),
            (End::TimedOut, Ended::Other(why)) => why.contains("poll_oneoff"),
            _ => false,
        };
        assert!(
            alike,
            "{timeout} {flags}: the engine {:?}, the model {:?}",
            engine.end, model.end
        );
    }
}

/// A value's type in WebAssembly, as a probe of an operator gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    I32,
    I64,
    F32,
    F64,
    V128,
}

impl Kind {
    /// Its name in WebAssembly text.
    const fn name(self) -> &'static str {
        match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::V128 => "v128",
        }
    }

    /// Its bytes.
    const fn bytes(self) -> usize {
        match self {
            Self::I32 | Self::F32 => 4,
            Self::I64 | Self::F64 => 8,
            Self::V128 => 16,
        }
    }
}

/// One expression an operator is tried on, and the type of what it gives.
type Probe = (Kind, String);

/// Where the probes' results start in memory.
const RESULTS: usize = 4096;

/// A module that stores each of `probes`' results, one after another from [`RESULTS`], and writes
/// them all to standard output.
fn probing(probes: &[Probe]) -> String {
    let mut body = String::new();
    let mut at = RESULTS;
    for (kind, expression) in probes {
        body.push_str(&format!(
            "    ({}.store (i32.const {at}) {expression})\n",
            kind.name()
        ));
        at += kind.bytes();
    }
    let length = at - RESULTS;
    format!(
        "(module\n  (import \"wasi_snapshot_preview1\" \"fd_write\"\n    (func $fd_write (param \
         i32 i32 i32 i32) (result i32)))\n  (import \"wasi_snapshot_preview1\" \"proc_exit\" \
         (func $exit (param i32)))\n  (memory (export \"memory\") 2 2)\n  (func (export \
         \"_start\")\n{body}    (i32.store (i32.const 1024) (i32.const {RESULTS}))\n    \
         (i32.store (i32.const 1028) (i32.const {length}))\n    (drop (call $fd_write (i32.const \
         1) (i32.const 1024) (i32.const 1) (i32.const 1032)))\n    (call $exit (i32.const 0))))\n"
    )
}

/// The doubles tried: zeros of both signs, ones, halves and ties, the largest and the smallest,
/// infinities, NaNs quiet and signaling with payloads and signs, and the edges of the whole
/// numbers a conversion takes.
const CHOSEN_F64: [u64; 24] = [
    0,
    0x8000_0000_0000_0000,
    0x3ff0_0000_0000_0000,
    0xbff0_0000_0000_0000,
    0x3fe0_0000_0000_0000,
    0x4004_0000_0000_0000,
    0x400c_0000_0000_0000,
    0xc004_0000_0000_0000,
    0x7fef_ffff_ffff_ffff,
    0x0000_0000_0000_0001,
    0x7ff0_0000_0000_0000,
    0xfff0_0000_0000_0000,
    0x7ff8_0000_0000_0000,
    0xfff8_0000_0000_0001,
    0x7ff0_0000_0000_0001,
    0x41e0_0000_0000_0000,
    0x41f0_0000_0000_0000,
    0x43e0_0000_0000_0000,
    0x43f0_0000_0000_0000,
    0xc1e0_0000_0010_0000,
    0xc1e0_0000_0020_0000,
    0x41ef_ffff_ffe0_0000,
    0xc3e0_0000_0000_0000,
    0x4340_0000_0000_0001,
];

/// The singles tried, as [`CHOSEN_F64`] are chosen.
const CHOSEN_F32: [u32; 20] = [
    0,
    0x8000_0000,
    0x3f80_0000,
    0xbf80_0000,
    0x3f00_0000,
    0x4020_0000,
    0x4060_0000,
    0x7f7f_ffff,
    0x0000_0001,
    0x7f80_0000,
    0xff80_0000,
    0x7fc0_0000,
    0xffc0_0001,
    0x7f80_0001,
    0x4f00_0000,
    0x4f80_0000,
    0x5f00_0000,
    0x5f80_0000,
    0xcf00_0000,
    0xdf00_0000,
];

/// The whole numbers tried in a conversion to a float.
const CHOSEN_I64: [u64; 10] = [
    0,
    1,
    u64::MAX,
    0x7fff_ffff_ffff_ffff,
    0x8000_0000_0000_0000,
    0x0020_0000_0000_0001,
    0x0000_0000_0100_0001,
    0x8000_0000_0000_0401,
    0xffff_ffff_8000_0000,
    0x0000_0000_ffff_ffff,
];

/// A double of `bits`, as WebAssembly text.
fn f64_of(bits: u64) -> String {
    format!("(f64.reinterpret_i64 (i64.const {bits:#x}))")
}

/// A single of `bits`, as WebAssembly text.
fn f32_of(bits: u32) -> String {
    format!("(f32.reinterpret_i32 (i32.const {bits:#x}))")
}

/// The floats tried at `kind`'s width: the chosen ones and `random` drawn ones, raw bits and
/// numbers of a size a program works with.
fn floats(kind: Kind, rng: &mut ChaCha8Rng, random: usize) -> Vec<String> {
    let mut values = Vec::new();
    if kind == Kind::F64 {
        values.extend(CHOSEN_F64.iter().map(|bits| f64_of(*bits)));
        for _ in 0..random {
            values.push(f64_of(rng.random::<u64>()));
            let number = f64::from(rng.random::<i32>()) / 1024.0;
            values.push(f64_of(number.to_bits()));
        }
    } else {
        values.extend(CHOSEN_F32.iter().map(|bits| f32_of(*bits)));
        for _ in 0..random {
            values.push(f32_of(rng.random::<u32>()));
            let number = f32::from(rng.random::<i16>()) / 64.0;
            values.push(f32_of(number.to_bits()));
        }
    }
    values
}

/// Probes of every float operator at `kind`'s width, on chosen and random operands.
fn float_probes(kind: Kind, rng: &mut ChaCha8Rng) -> Vec<Vec<Probe>> {
    let name = kind.name();
    let values = floats(kind, rng, 8);
    let mut modules = Vec::new();
    for op in ["add", "sub", "mul", "div", "min", "max", "copysign"] {
        let mut probes = Vec::new();
        for a in &values {
            for b in &values {
                probes.push((kind, format!("({name}.{op} {a} {b})")));
            }
        }
        modules.push(probes);
    }
    for op in ["eq", "ne", "lt", "gt", "le", "ge"] {
        let mut probes = Vec::new();
        for a in &values {
            for b in &values {
                probes.push((Kind::I32, format!("({name}.{op} {a} {b})")));
            }
        }
        modules.push(probes);
    }
    let mut unary = Vec::new();
    for value in &values {
        for op in ["abs", "neg", "sqrt", "ceil", "floor", "trunc", "nearest"] {
            unary.push((kind, format!("({name}.{op} {value})")));
        }
        let (other, resize) = if kind == Kind::F64 {
            (Kind::F32, "f32.demote_f64")
        } else {
            (Kind::F64, "f64.promote_f32")
        };
        unary.push((other, format!("({resize} {value})")));
        for (to, sign) in [
            (Kind::I32, "s"),
            (Kind::I32, "u"),
            (Kind::I64, "s"),
            (Kind::I64, "u"),
        ] {
            let to_name = to.name();
            unary.push((to, format!("({to_name}.trunc_sat_{name}_{sign} {value})")));
        }
    }
    let mut numbers: Vec<u64> = CHOSEN_I64.to_vec();
    numbers.extend((0..16).map(|_| rng.random::<u64>()));
    for number in numbers {
        for sign in ["s", "u"] {
            unary.push((
                kind,
                format!("({name}.convert_i64_{sign} (i64.const {number:#x}))"),
            ));
            let low = number & 0xffff_ffff;
            unary.push((
                kind,
                format!("({name}.convert_i32_{sign} (i32.const {low:#x}))"),
            ));
        }
    }
    modules.push(unary);
    modules
}

/// A vector of four 32-bit lanes, as WebAssembly text.
fn v128_of(lanes: [u32; 4]) -> String {
    let [a, b, c, d] = lanes;
    format!("(v128.const i32x4 {a:#x} {b:#x} {c:#x} {d:#x})")
}

/// The vectors tried: zeros, ones, each byte's top bit, a ramp of bytes, alternating signs, and
/// `random` drawn ones.
fn vectors(rng: &mut ChaCha8Rng, random: usize) -> Vec<String> {
    let mut values = vec![
        v128_of([0; 4]),
        v128_of([u32::MAX; 4]),
        v128_of([0x8080_8080; 4]),
        v128_of([0x0302_0100, 0x0706_0504, 0x0b0a_0908, 0x0f0e_0d0c]),
        v128_of([0x7fff_8000, 0x8000_7fff, 0x0001_ffff, 0xffff_0001]),
        v128_of([0x8000_0000, 0x7fff_ffff, 1, 0xffff_fffe]),
    ];
    for _ in 0..random {
        values.push(v128_of([
            rng.random::<u32>(),
            rng.random::<u32>(),
            rng.random::<u32>(),
            rng.random::<u32>(),
        ]));
    }
    values
}

/// The lane operators of SIMD128's 8- and 16-bit lanes, but the 16-bit lanes' `mul`.
const NARROW_LANES: [&str; 21] = [
    "add",
    "sub",
    "add_sat_s",
    "add_sat_u",
    "sub_sat_s",
    "sub_sat_u",
    "min_s",
    "min_u",
    "max_s",
    "max_u",
    "avgr_u",
    "eq",
    "ne",
    "lt_s",
    "lt_u",
    "gt_s",
    "gt_u",
    "le_s",
    "le_u",
    "ge_s",
    "ge_u",
];

/// The lane operators of its 32-bit lanes.
const WORD_LANES: [&str; 17] = [
    "add", "sub", "mul", "min_s", "min_u", "max_s", "max_u", "eq", "ne", "lt_s", "lt_u", "gt_s",
    "gt_u", "le_s", "le_u", "ge_s", "ge_u",
];

/// The lane operators of its 64-bit lanes.
const WIDE_LANES: [&str; 9] = [
    "add", "sub", "mul", "eq", "ne", "lt_s", "gt_s", "le_s", "ge_s",
];

/// Probes of every SIMD128 operator the model follows, on chosen and random vectors, lanes,
/// counts and picks.
fn vector_probes(rng: &mut ChaCha8Rng) -> Vec<Vec<Probe>> {
    let values = vectors(rng, 10);
    let mut modules = Vec::new();
    let mut binary = Vec::new();
    let mut pairs: Vec<(&String, &String)> = Vec::new();
    for (index, a) in values.iter().enumerate() {
        pairs.push((a, &values[(index + 1) % values.len()]));
        pairs.push((a, a));
    }
    let lanewise: [(&str, &[&str]); 5] = [
        ("i8x16", &NARROW_LANES),
        ("i16x8", &NARROW_LANES),
        ("i16x8", &["mul"]),
        ("i32x4", &WORD_LANES),
        ("i64x2", &WIDE_LANES),
    ];
    for (shape, ops) in lanewise {
        for op in ops {
            for (a, b) in &pairs {
                binary.push((Kind::V128, format!("({shape}.{op} {a} {b})")));
            }
        }
    }
    for op in ["and", "andnot", "or", "xor"] {
        for (a, b) in &pairs {
            binary.push((Kind::V128, format!("(v128.{op} {a} {b})")));
        }
    }
    for (a, b) in &pairs {
        binary.push((
            Kind::V128,
            format!("(v128.bitselect {a} {b} {})", values[2]),
        ));
        binary.push((Kind::V128, format!("(i8x16.swizzle {a} {b})")));
        let picks: Vec<String> = (0..16)
            .map(|_| rng.random_range(0..32_u32).to_string())
            .collect();
        binary.push((
            Kind::V128,
            format!("(i8x16.shuffle {} {a} {b})", picks.join(" ")),
        ));
    }
    modules.push(binary);
    let mut unary = Vec::new();
    for value in &values {
        unary.push((Kind::V128, format!("(v128.not {value})")));
        unary.push((Kind::I32, format!("(v128.any_true {value})")));
        for shape in ["i8x16", "i16x8", "i32x4", "i64x2"] {
            for op in ["all_true", "bitmask"] {
                unary.push((Kind::I32, format!("({shape}.{op} {value})")));
            }
            for op in ["neg", "abs"] {
                unary.push((Kind::V128, format!("({shape}.{op} {value})")));
            }
            for count in [0, 1, 7, 8, 9, 15, 16, 31, 33, 63, 64, 100] {
                for op in ["shl", "shr_s", "shr_u"] {
                    unary.push((
                        Kind::V128,
                        format!("({shape}.{op} {value} (i32.const {count}))"),
                    ));
                }
            }
        }
        for lane in [0, 5, 15] {
            unary.push((Kind::I32, format!("(i8x16.extract_lane_s {lane} {value})")));
            unary.push((Kind::I32, format!("(i8x16.extract_lane_u {lane} {value})")));
            unary.push((
                Kind::V128,
                format!("(i8x16.replace_lane {lane} {value} (i32.const 0x1ab))"),
            ));
        }
        for lane in [0, 7] {
            unary.push((Kind::I32, format!("(i16x8.extract_lane_s {lane} {value})")));
            unary.push((Kind::I32, format!("(i16x8.extract_lane_u {lane} {value})")));
        }
        unary.push((Kind::I32, format!("(i32x4.extract_lane 3 {value})")));
        unary.push((Kind::I64, format!("(i64x2.extract_lane 1 {value})")));
        unary.push((Kind::F64, format!("(f64x2.extract_lane 0 {value})")));
        unary.push((
            Kind::V128,
            format!("(i64x2.replace_lane 1 {value} (i64.const -2))"),
        ));
    }
    for scalar in [0_u32, 1, 0x7f, 0x80, 0xff, 0x1234_5678, u32::MAX] {
        for shape in ["i8x16", "i16x8", "i32x4"] {
            unary.push((
                Kind::V128,
                format!("({shape}.splat (i32.const {scalar:#x}))"),
            ));
        }
        unary.push((Kind::V128, format!("(i64x2.splat (i64.const {scalar:#x}))")));
    }
    modules.push(unary);
    modules
}

/// Each float and SIMD128 operator the model follows does what the engine does, bit for bit, on
/// chosen operands, a NaN's sign and payload among them, and on random ones: every NaN an
/// operation gives canonical, as the engine makes it; `abs`, `neg` and `copysign` on the sign bit
/// alone; `min`, `max` and the comparisons with NaNs and zeros; the conversions at their edges,
/// `trunc_sat` saturating; and the lanes of every shape.
#[tokio::test(flavor = "multi_thread")]
async fn each_float_and_vector_operator_does_what_the_engine_does() {
    let pool = Pool::new(launcher(), None);
    let mut rng = ChaCha8Rng::seed_from_u64(0x0f10_a75e);
    let mut modules = float_probes(Kind::F64, &mut rng);
    modules.extend(float_probes(Kind::F32, &mut rng));
    modules.extend(vector_probes(&mut rng));
    let mut failures = Vec::new();
    for probes in &modules {
        // A module of at most a few thousand probes, so its function stays well inside the
        // engine's stack and the model's.
        for chunk in probes.chunks(600) {
            let text = probing(chunk);
            let program = Program::of_text(&text).unwrap_or_else(|problem| panic!("{problem}"));
            let spec = Spec::new(&[], &[], Policy::default());
            let engine = pool
                .run(&program, spec, Limits::default())
                .await
                .expect("the engine runs it");
            let module = Module::new(program.built(), Policy::default()).expect("the model");
            let model = run(&module, &Given::default()).expect("the model runs it");
            if engine.end != End::Exited(0) || model.end != Ended::Exit(0) {
                failures.push(format!(
                    "the engine ended {:?}, the model {:?}, on {:?}",
                    engine.end, model.end, chunk[0].1
                ));
                continue;
            }
            let mut at = 0;
            for (kind, expression) in chunk {
                let end = at + kind.bytes();
                let (theirs, ours) = (engine.stdout.get(at..end), model.stdout.get(at..end));
                if theirs != ours {
                    failures.push(format!(
                        "{expression}: the engine gave {theirs:02x?}, the model {ours:02x?}"
                    ));
                }
                at = end;
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
