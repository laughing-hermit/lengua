//! lengua's WebAssembly host, function by function: every case in `tests/host-cases/`, one folder
//! per WASI function and in it each case's module and what the module must do, and a few cases of
//! this file's own, so it means something on its own. Every case runs the way a test does, in a
//! worker of lengua's pool, on every system.
//!
//! A case is `<case>.wat`, a whole module that imports from `wasi_snapshot_preview1`, exports
//! `_start` and `memory` and ends with `proc_exit`, and `<case>.toml`:
//!
//! ```toml
//! about = "fd_read returns 0 at the end of standard input"
//! uses = ["standard input"]        # the idea's Uses words, may be empty
//! network = ""                     # an idea's `- Network:` line after the colon, or empty
//! args = ["first"]                 # after the program's name
//! env = { NAME = "value" }         # only given when uses has "the environment"
//! stdin = "abc"                    # or stdin_hex = "616263"
//! files = { "in.txt" = "hello" }   # the working folder before the run ("dir/" = "" a folder)
//! clock_ns = 1_700_000_000_000_000_000   # optional, a supplied clock
//! random_hex = "00ff"              # optional, supplied random bytes
//! [expect]
//! exit = 0                         # the exit code
//! stdout = "abc"                   # or stdout_hex; exact
//! stderr = ""                      # exact
//! files = { "out.txt" = "hello" }  # optional: the folder after the run holds exactly these
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use lengua::connect::Reach;
use lengua::runner::{End, Limits};
use lengua::supply::{Supply, Time};
use lengua::wasm::job::unhex;
use lengua::wasm::{Policy, Pool, Program, Spec};
use serde::Deserialize;

/// What a case gives its program, and what the program must do.
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

/// What a case's program must do.
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

/// This binary, which the pool's workers are.
fn launcher() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lengua-mcp"))
}

/// Bytes as text, for a message.
fn shown(bytes: &[u8]) -> String {
    format!("{:?}", String::from_utf8_lossy(bytes))
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

/// Runs the module `wat` as `case` says, in `pool`, and says what it did that the case doesn't.
async fn check(pool: &Pool, wat: &str, case: &Case) -> Result<(), String> {
    let program = Program::of_text(wat)?;
    let words: Vec<&str> = case.uses.iter().map(String::as_str).collect();
    let (uses, network) = lengua::idea::constraints(&words, &case.network).map_err(|problems| {
        problems
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let policy = Policy::of(&uses, &network);
    let stdin = match (&case.stdin, &case.stdin_hex) {
        (Some(_), Some(_)) => return Err("a case gives stdin or stdin_hex, not both".to_owned()),
        (Some(text), None) => text.as_bytes().to_vec(),
        (None, Some(hex)) => unhex(hex)?,
        (None, None) => Vec::new(),
    };
    let (files, folders) = split(&case.files);
    let supply = Supply {
        random: case.random_hex.as_deref().map(unhex).transpose()?,
        clock: case
            .clock_ns
            .map(|nanoseconds| Time {
                seconds: i64::try_from(nanoseconds / 1_000_000_000).unwrap_or(i64::MAX),
                nanoseconds: u32::try_from(nanoseconds % 1_000_000_000).unwrap_or(0),
            })
            .into_iter()
            .collect(),
    };
    // A case has no server of its own: each connection its network names is handed over ended.
    let connections: Vec<Reach> = network
        .connections
        .iter()
        .map(|_| Reach::Nowhere)
        .collect();
    let spec = Spec {
        files: &files,
        folders: &folders,
        env: &case.env,
        supply: Some(&supply),
        connections: &connections,
        listen: network.listens.map(|_| (0, false)),
        ..Spec::new(&case.args, &stdin, policy)
    };
    let run = pool
        .run(&program, spec, Limits::default())
        .await
        .map_err(|error| format!("it couldn't run: {error}"))?;
    let mut problems = Vec::new();
    if let Some(problem) = &run.supplied {
        problems.push(problem.clone());
    }
    match run.end {
        End::Exited(code) if code == case.expect.exit => {}
        other => problems.push(format!(
            "it ended with {}, where the case wants exit code {}",
            other.describe(Limits::default()),
            case.expect.exit
        )),
    }
    let stdout = match (&case.expect.stdout, &case.expect.stdout_hex) {
        (Some(_), Some(_)) => {
            return Err("a case expects stdout or stdout_hex, not both".to_owned());
        }
        (Some(text), None) => text.as_bytes().to_vec(),
        (None, Some(hex)) => unhex(hex)?,
        (None, None) => Vec::new(),
    };
    if run.stdout != stdout {
        problems.push(format!(
            "stdout was {}, where the case wants {}",
            shown(&run.stdout),
            shown(&stdout)
        ));
    }
    if run.stderr != case.expect.stderr.as_bytes() {
        problems.push(format!(
            "stderr was {}, where the case wants {}",
            shown(&run.stderr),
            shown(case.expect.stderr.as_bytes())
        ));
    }
    if let Some(wanted) = &case.expect.files {
        let (wanted_files, wanted_folders) = split(wanted);
        match &run.files {
            Err(problem) => problems.push(format!("its folder couldn't be read: {problem}")),
            Ok(found) => {
                let found_text: BTreeMap<String, String> = found
                    .iter()
                    .map(|(name, bytes)| {
                        (name.clone(), String::from_utf8_lossy(bytes).into_owned())
                    })
                    .collect();
                if found_text != wanted_files {
                    problems.push(format!(
                        "its folder held the files {found_text:?}, where the case wants \
                         {wanted_files:?}"
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
                let unnamed: Vec<&String> =
                    run.folders.iter().filter(|folder| !named(*folder)).collect();
                let missing: Vec<&String> = wanted_folders
                    .iter()
                    .filter(|folder| !run.folders.contains(*folder))
                    .collect();
                if !unnamed.is_empty() || !missing.is_empty() {
                    problems.push(format!(
                        "its folder held the folders {:?}, where the case wants {wanted_folders:?}",
                        run.folders
                    ));
                }
            }
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
            if path.extension().is_some_and(|extension| extension == "toml") {
                found.push((path.with_extension("wat"), path));
            }
        }
    }
    found.sort();
    found
}

#[tokio::test(flavor = "multi_thread")]
async fn every_case_in_host_cases_does_what_its_file_says() {
    let cases = case_files();
    if cases.is_empty() {
        lengua::skiplog::note("tests/host-cases holds no cases yet");
        return;
    }
    let pool = Pool::new(launcher(), None);
    let mut failures = Vec::new();
    for (wat, toml) in &cases {
        let outcome = match (std::fs::read_to_string(wat), std::fs::read_to_string(toml)) {
            (Ok(wat), Ok(text)) => match toml::from_str::<Case>(&text) {
                Ok(case) => check(&pool, &wat, &case)
                    .await
                    .map_err(|problem| format!("{}: {problem}", case.about)),
                Err(error) => Err(format!("the case doesn't read: {error}")),
            },
            (Err(error), _) => Err(format!("its module {} doesn't read: {error}", wat.display())),
            (_, Err(error)) => Err(format!("it doesn't read: {error}")),
        };
        if let Err(problem) = outcome {
            failures.push(format!("{}: {problem}", toml.display()));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

/// Writes "hello" and a newline, and exits with 0.
const HELLO: &str = r#"(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (data (i32.const 64) "hello\n")
  (func (export "_start")
    (i32.store (i32.const 0) (i32.const 64))
    (i32.store (i32.const 4) (i32.const 6))
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 8)))
    (call $exit (i32.const 0))))
"#;

/// Reads standard input once and writes what it read; exits with the read's error, if any.
const ECHO: &str = r#"(module
  (import "wasi_snapshot_preview1" "fd_read"
    (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (local $errno i32)
    (i32.store (i32.const 0) (i32.const 64))
    (i32.store (i32.const 4) (i32.const 64))
    (local.set $errno (call $fd_read (i32.const 0) (i32.const 0) (i32.const 1) (i32.const 8)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (i32.store (i32.const 4) (i32.load (i32.const 8)))
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 8)))
    (call $exit (i32.const 0))))
"#;

/// Writes its first argument.
const FIRST_ARGUMENT: &str = r#"(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $sizes (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get" (func $args (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (drop (call $sizes (i32.const 0) (i32.const 4)))
    (drop (call $args (i32.const 16) (i32.const 64)))
    ;; argv[1], after "main.wasm" and its NUL.
    (i32.store (i32.const 8) (i32.load (i32.const 20)))
    (i32.store (i32.const 12) (i32.sub (i32.load (i32.const 4)) (i32.const 11)))
    (drop (call $fd_write (i32.const 1) (i32.const 8) (i32.const 1) (i32.const 32)))
    (call $exit (i32.const 0))))
"#;

/// Writes its one environment variable.
const ENVIRONMENT: &str = r#"(module
  (import "wasi_snapshot_preview1" "environ_sizes_get"
    (func $sizes (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "environ_get" (func $environ (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (drop (call $sizes (i32.const 0) (i32.const 4)))
    (drop (call $environ (i32.const 16) (i32.const 64)))
    (i32.store (i32.const 8) (i32.load (i32.const 16)))
    (i32.store (i32.const 12) (i32.sub (i32.load (i32.const 4)) (i32.const 1)))
    (drop (call $fd_write (i32.const 1) (i32.const 8) (i32.const 1) (i32.const 32)))
    (call $exit (i32.load (i32.const 0)))))
"#;

/// Copies in.txt to out.txt in its working folder.
const COPY: &str = r#"(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_read"
    (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $close (param i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (data (i32.const 100) "in.txt")
  (data (i32.const 120) "out.txt")
  (func (export "_start")
    (local $errno i32)
    (local.set $errno (call $open (i32.const 3) (i32.const 0) (i32.const 100) (i32.const 6)
      (i32.const 0) (i64.const 2) (i64.const 0) (i32.const 0) (i32.const 0)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (i32.store (i32.const 16) (i32.const 200))
    (i32.store (i32.const 20) (i32.const 64))
    (local.set $errno
      (call $fd_read (i32.load (i32.const 0)) (i32.const 16) (i32.const 1) (i32.const 24)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (local.set $errno (call $open (i32.const 3) (i32.const 0) (i32.const 120) (i32.const 7)
      (i32.const 9) (i64.const 64) (i64.const 0) (i32.const 0) (i32.const 4)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (i32.store (i32.const 20) (i32.load (i32.const 24)))
    (local.set $errno
      (call $fd_write (i32.load (i32.const 4)) (i32.const 16) (i32.const 1) (i32.const 28)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (drop (call $close (i32.load (i32.const 0))))
    (call $exit (call $close (i32.load (i32.const 4))))))
"#;

/// Exits with its first random byte.
const RANDOM: &str = r#"(module
  (import "wasi_snapshot_preview1" "random_get" (func $random (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (drop (call $random (i32.const 0) (i32.const 1)))
    (call $exit (i32.load8_u (i32.const 0)))))
"#;

/// Exits with the clock's seconds, or its error.
const CLOCK: &str = r#"(module
  (import "wasi_snapshot_preview1" "clock_time_get"
    (func $time (param i32 i64 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (local $errno i32)
    (local.set $errno (call $time (i32.const 0) (i64.const 0) (i32.const 0)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (call $exit (i32.wrap_i64 (i64.div_u (i64.load (i32.const 0)) (i64.const 1000000000))))))
"#;

/// Fails assertion 7 with lengua's library.
const ASSERTION: &str = r#"(module
  (memory (export "memory") 1)
  (func (export "_start") (call $assert_fail (i32.const 7))))
"#;

/// Reads a line with lengua's library and writes it back.
const LINE: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (local $got i32)
    (local.set $got (call $fill (i32.const 0) (i32.const 64) (i32.const 100) (i32.const 10)))
    (if (i32.lt_s (local.get $got) (i32.const 0))
      (then (call $exit (i32.sub (i32.const 0) (local.get $got)))))
    (call $exit (i32.sub (i32.const 0)
      (call $write_all (i32.const 1) (i32.const 64) (local.get $got))))))
"#;

/// This file's own cases: a module and its case, in the format of `tests/host-cases/`.
const SMOKE: &[(&str, &str)] = &[
    (
        HELLO,
        r#"about = "fd_write writes standard output"
[expect]
exit = 0
stdout = "hello\n"
"#,
    ),
    (
        ECHO,
        r#"about = "fd_read reads standard input, which then ends"
uses = ["standard input"]
stdin = "abc"
[expect]
exit = 0
stdout = "abc"
"#,
    ),
    (
        ECHO,
        r#"about = "fd_read refuses standard input to a program whose idea doesn't name it"
stdin = "abc"
[expect]
exit = 76
stdout = ""
"#,
    ),
    (
        FIRST_ARGUMENT,
        r#"about = "args_get gives the arguments after the program's name"
args = ["first"]
[expect]
exit = 0
stdout = "first"
"#,
    ),
    (
        ENVIRONMENT,
        r#"about = "environ_get gives the environment the idea names"
uses = ["the environment"]
env = { NAME = "value" }
[expect]
exit = 1
stdout = "NAME=value"
"#,
    ),
    (
        COPY,
        r#"about = "path_open, fd_read and fd_write copy a file in the working folder"
uses = ["files"]
files = { "in.txt" = "hello" }
[expect]
exit = 0
files = { "in.txt" = "hello", "out.txt" = "hello" }
"#,
    ),
    (
        RANDOM,
        r#"about = "random_get gives the test's bytes"
uses = ["random bytes"]
random_hex = "2a"
[expect]
exit = 42
"#,
    ),
    (
        CLOCK,
        r#"about = "clock_time_get reads the test's clock"
uses = ["the clock"]
clock_ns = 7_000_000_000
[expect]
exit = 7
"#,
    ),
    (
        CLOCK,
        r#"about = "clock_time_get refuses the clock to a program whose idea doesn't name it"
[expect]
exit = 76
"#,
    ),
    (
        ASSERTION,
        r#"about = "lengua's $assert_fail writes its message and exits with 134"
[expect]
exit = 134
stderr = "lengua: assertion 007 failed\n"
"#,
    ),
    (
        LINE,
        r#"about = "lengua's $fill and $write_all read a line and write it back"
uses = ["standard input"]
stdin = "a line\n"
[expect]
exit = 0
stdout = "a line\n"
"#,
    ),
];

#[tokio::test(flavor = "multi_thread")]
async fn this_files_own_cases_do_what_they_say() {
    let pool = Pool::new(launcher(), None);
    let mut failures = Vec::new();
    for (wat, text) in SMOKE {
        let case: Case = toml::from_str(text).expect("a case");
        if let Err(problem) = check(&pool, wat, &case).await {
            failures.push(format!("{}: {problem}", case.about));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_case_reads_as_the_format_says() {
    let case: Case = toml::from_str(
        r#"about = "every field"
uses = ["standard input"]
network = ""
args = ["first"]
env = { NAME = "value" }
stdin_hex = "616263"
files = { "in.txt" = "hello", "dir/" = "" }
clock_ns = 1_700_000_000_000_000_000
random_hex = "00ff"
[expect]
exit = 0
stdout_hex = "616263"
stderr = ""
files = { "out.txt" = "hello" }
"#,
    )
    .expect("the format's case");
    assert_eq!(case.args, ["first"]);
    assert_eq!(case.clock_ns, Some(1_700_000_000_000_000_000));
    let (files, folders) = split(&case.files);
    assert_eq!(files.len(), 1);
    assert!(folders.contains("dir"));
    assert!(toml::from_str::<Case>("about = \"x\"\nsurprise = 1\n[expect]\nexit = 0\n").is_err());
}
