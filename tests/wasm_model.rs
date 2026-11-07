//! The prover's model of WebAssembly against what lengua's host and engine do, on every system:
//! every case of `tests/host-cases/` run in the model gives what its file says, as the host's own
//! runs do (tests/host.rs); and on every test of the WebAssembly fixtures' ideas, and of the
//! compile skill's WebAssembly templates where it has them, the model and the engine do the same.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use lengua::prove::wasm::{Ended, Given, Module, Ran, run};
use lengua::runner::{End, Limits, Run};
use lengua::supply::Supply;
use lengua::testfile::{Case as Sample, Example, TestSuite};
use lengua::wasm::job::unhex;
use lengua::wasm::{Policy, Pool, Program, Spec};
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
        args: case.args.iter().map(|arg| arg.as_bytes().to_vec()).collect(),
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

/// Every WebAssembly program whose ideas' tests the model must run as the engine does: each
/// fixture of `tests/fixtures-wasm/` with a main.wat, and each of the compile skill's templates
/// `*-template.wat` with its `*-template-idea.md`.
fn programs() -> Vec<(PathBuf, PathBuf)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root.join("tests/fixtures-wasm")) {
        for entry in entries.flatten() {
            let folder = entry.path();
            if folder.join("main.wat").is_file() && folder.join("idea.md").is_file() {
                found.push((folder.join("idea.md"), folder.join("main.wat")));
            }
        }
    }
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
            let idea = assets.join(format!("{stem}-template-idea.md"));
            if idea.is_file() {
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
            args: self.args.iter().map(|arg| arg.as_bytes().to_vec()).collect(),
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
    assert!(
        programs.len() >= 3,
        "found only {} WebAssembly programs",
        programs.len()
    );
    let pool = Pool::new(launcher(), None);
    let mut failures = Vec::new();
    for (idea_path, wat_path) in &programs {
        let shown = wat_path.display().to_string();
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
                other => failures.push(format!("{shown}: the model {other:?}")),
            }
            continue;
        }
        let module =
            Module::new(&built, policy).unwrap_or_else(|problem| panic!("{shown}: {problem}"));
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
                    failures.push(format!("{shown}, {}: the model couldn't run: {why}", test.name));
                    continue;
                }
            };
            let found = differences(&engine, &model);
            if !found.is_empty() {
                failures.push(format!("{shown}, {}: {}", test.name, found.join("; ")));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
