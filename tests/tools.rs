//! The tools of a run (W24): a trace of the program's calls to the host, a profile of its time,
//! and a stop at a line or a function of main.wat, each through the `run` tool and from a
//! shell, and each off unless a run asks for it.
//!
//! The tests also pin what each tool does with the three kinds of program that differ: an idea
//! that names a secret (a trace of names only, no stop), a program that splits its work (a trace
//! and a profile of the program itself, no stop) and an outside module (a trace and a profile,
//! no stop).

mod common;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{ADD_CODE, ADD_IDEA, Program, Protocol, REQUEST, Session};
use serde_json::{Value, json};

/// This binary.
fn launcher() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lengua-mcp"))
}

/// lengua-run beside lengua-mcp, where a run of the whole suite builds it.
fn lengua_run() -> Option<PathBuf> {
    let exe = std::env::consts::EXE_SUFFIX;
    let alone = launcher().with_file_name(format!("lengua-run{exe}"));
    if alone.is_file() {
        Some(alone)
    } else {
        lengua::skiplog::note(
            "lengua-run isn't built beside lengua-mcp, which a run of the whole suite does, so \
             the tools of a run from a shell ran through lengua-mcp alone",
        );
        None
    }
}

/// `lengua-mcp` or lengua-run, `program`, run from a shell with the session's data folder, then
/// `words`.
fn from_shell(program: &Path, session: &Session, words: &[&OsStr]) -> Output {
    Command::new(program)
        .arg("--data-dir")
        .arg(session.data_dir.path())
        .args(words)
        .env_remove("LENGUA_LOG")
        .output()
        .expect("it runs")
}

/// The strings of a reply's list.
fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// A program folder with `idea` and `code`, checked until its tests and proofs have ended.
async fn checked(session: &Session, idea: &str, code: &str) -> Program {
    let program = Program::new();
    program.write("idea.md", idea);
    session.record(&program).await;
    program.write("main.wat", code);
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], "done", "{reply}");
    program
}

/// `run`'s arguments for `program` with `args` for the program and `more` beside them.
fn asking(program: &Program, args: &[&str], more: &Value) -> Value {
    let mut arguments = program.arguments();
    arguments["args"] = json!(args);
    if let Some(more) = more.as_object() {
        for (name, value) in more {
            arguments[name.as_str()] = value.clone();
        }
    }
    arguments
}

/// The line of `code`, from 1, that holds `words`.
fn line_with(code: &str, words: &str) -> usize {
    code.lines()
        .position(|line| line.contains(words))
        .unwrap_or_else(|| panic!("no line holds {words}"))
        + 1
}

/// A run that asks for a trace gets every call the program made to the host, in order, with
/// the numbers it passed and the host's answers, through the `run` tool, `lengua-mcp run` and
/// lengua-run; a run that doesn't ask gets none.
#[tokio::test]
async fn a_run_traces_the_programs_calls_to_the_host() {
    let session = Session::start(Protocol::Handshake).await;
    let program = checked(&session, ADD_IDEA, ADD_CODE).await;
    let plain = session
        .call_ok("run", asking(&program, &["2", "3"], &json!({})))
        .await;
    assert_eq!(plain["stdout"], "5\n", "{plain}");
    for part in ["host_calls", "profile", "stopped"] {
        assert!(plain.get(part).is_none(), "{part} without asking: {plain}");
    }
    let traced = session
        .call_ok(
            "run",
            asking(&program, &["2", "3"], &json!({ "trace": true })),
        )
        .await;
    assert_eq!(traced["stdout"], "5\n", "{traced}");
    assert_eq!(traced["exit_code"], 0, "{traced}");
    let calls = strings(&traced["host_calls"]["calls"]);
    assert_eq!(
        calls.first().map(String::as_str),
        Some("args_sizes_get(2048, 2052) = 0"),
        "{traced}"
    );
    assert_eq!(
        calls.get(1).map(String::as_str),
        Some("args_get(2064, 2080) = 0"),
        "{traced}"
    );
    assert!(
        calls
            .iter()
            .any(|call| call.starts_with("fd_write(1, ") && call.ends_with(" = 0")),
        "{traced}"
    );
    assert_eq!(
        calls.last().map(String::as_str),
        Some("proc_exit(0) ended the run with exit code 0"),
        "{traced}"
    );
    assert_eq!(traced["host_calls"]["left_out"], 0, "{traced}");
    // The numbers are what the program passed, never the bytes at them: its arguments' text
    // is in no line.
    assert!(calls.iter().all(|call| !call.contains("main.wasm")));

    let folder = program.path().as_os_str();
    let words = |first: &'static str| -> Vec<&OsStr> {
        let mut words: Vec<&OsStr> = Vec::new();
        if !first.is_empty() {
            words.push(OsStr::new(first));
        }
        words.extend([
            folder,
            OsStr::new("--trace"),
            OsStr::new("2"),
            OsStr::new("3"),
        ]);
        words
    };
    let ran = from_shell(&launcher(), &session, &words("run"));
    assert_eq!(ran.stdout, b"5\n", "{ran:?}");
    assert_eq!(ran.status.code(), Some(0), "{ran:?}");
    let said = String::from_utf8_lossy(&ran.stderr).into_owned();
    assert!(
        said.contains("lengua-mcp: trace: args_sizes_get(2048, 2052) = 0"),
        "{said}"
    );
    assert!(
        said.contains("lengua-mcp: trace: proc_exit(0) ended the run with exit code 0"),
        "{said}"
    );
    // Without the option, a run from a shell says nothing of a trace.
    let quiet = from_shell(
        &launcher(),
        &session,
        &[OsStr::new("run"), folder, OsStr::new("2"), OsStr::new("3")],
    );
    assert_eq!(quiet.stdout, b"5\n", "{quiet:?}");
    assert!(
        !String::from_utf8_lossy(&quiet.stderr).contains("trace:"),
        "{quiet:?}"
    );
    if let Some(alone) = lengua_run() {
        let ran = from_shell(&alone, &session, &words(""));
        assert_eq!(ran.stdout, b"5\n", "{ran:?}");
        let said = String::from_utf8_lossy(&ran.stderr).into_owned();
        assert!(
            said.contains(": trace: args_sizes_get(2048, 2052) = 0"),
            "{said}"
        );
    }
    session.service.cancel().await.expect("close the session");
}

/// A run that asks for a stop ends at the function or the line it names and says the values
/// there: the function's parameters and locals by main.wat's names, and the bytes of memory
/// asked for. A run that never reaches the place ends as it would, and says so. What is stopped
/// is a copy: the folder's lock and the next run are as they were.
#[tokio::test]
async fn a_run_stops_at_a_function_or_a_line_and_shows_its_values() {
    let session = Session::start(Protocol::Handshake).await;
    let program = checked(&session, ADD_IDEA, ADD_CODE).await;
    let lock = program.read("lengua.lock");
    // The first call of $parse_i64: the first argument's text starts 10 bytes into $args,
    // after the program's name, and the number goes to $number, at 2144.
    let more = json!({ "stop_at": "$parse_i64", "show_memory": "2080:12" });
    let stopped = session
        .call_ok("run", asking(&program, &["2", "3"], &more))
        .await;
    assert_eq!(stopped["stdout"], "", "{stopped}");
    assert!(stopped["exit_code"].is_null(), "{stopped}");
    let entry = line_with(ADD_CODE, "(local.set $status (global.get $ERROR_INVALID))");
    assert_eq!(
        stopped["ended"],
        format!("stopped at line {entry} of main.wat, in $parse_i64, as stop_at asked"),
        "{stopped}"
    );
    assert_eq!(stopped["stopped"]["reached"], true, "{stopped}");
    let locals = strings(&stopped["stopped"]["locals"]);
    assert!(
        locals.contains(&"$at: i32 10 (0x0000000a)".to_owned()),
        "{stopped}"
    );
    assert!(
        locals.contains(&"$into: i32 2144 (0x00000860)".to_owned()),
        "{stopped}"
    );
    assert!(
        locals.contains(&"$n: i64 0 (0x0000000000000000)".to_owned()),
        "{stopped}"
    );
    assert_eq!(
        stopped["stopped"]["memory"], "6d 61 69 6e 2e 77 61 73 6d 00 32 00",
        "{stopped}"
    );
    assert!(
        stopped["stopped"]["note"]
            .as_str()
            .is_some_and(|note| note.contains("check never tested")),
        "{stopped}"
    );

    // A line of $main, after the first number is parsed: it is in memory, and $a isn't set yet.
    let line = line_with(ADD_CODE, "(local.set $a (i64.load (global.get $number)))");
    let more = json!({ "stop_at": line.to_string(), "show_memory": "2144:8" });
    let stopped = session
        .call_ok("run", asking(&program, &["2", "3"], &more))
        .await;
    assert_eq!(stopped["stopped"]["reached"], true, "{stopped}");
    assert_eq!(
        stopped["stopped"]["at"],
        format!("line {line} of main.wat, in $main"),
        "{stopped}"
    );
    let locals = strings(&stopped["stopped"]["locals"]);
    assert!(
        locals.contains(&"$a: i64 0 (0x0000000000000000)".to_owned()),
        "{stopped}"
    );
    assert_eq!(
        stopped["stopped"]["memory"], "02 00 00 00 00 00 00 00",
        "{stopped}"
    );

    // Without arguments the program prints its usage line and never parses a number.
    let more = json!({ "stop_at": "$parse_i64" });
    let unreached = session.call_ok("run", asking(&program, &[], &more)).await;
    assert_eq!(unreached["stopped"]["reached"], false, "{unreached}");
    assert_eq!(unreached["exit_code"], 1, "{unreached}");
    assert_eq!(unreached["stderr"], "usage: add <a> <b>\n", "{unreached}");
    assert!(strings(&unreached["stopped"]["locals"]).is_empty());

    // A place main.wat doesn't have, and memory without a stop, are refused.
    let more = json!({ "stop_at": "$nothing" });
    let refused = session
        .call_error("run", asking(&program, &["2", "3"], &more))
        .await;
    assert!(
        refused.contains("defines no function $nothing"),
        "{refused}"
    );
    assert!(refused.contains("$parse_i64"), "{refused}");
    let more = json!({ "stop_at": "1" });
    let refused = session
        .call_error("run", asking(&program, &["2", "3"], &more))
        .await;
    assert!(refused.contains("starts on line 1"), "{refused}");
    let more = json!({ "show_memory": "0:8" });
    let refused = session
        .call_error("run", asking(&program, &["2", "3"], &more))
        .await;
    assert!(refused.contains("give stop_at too"), "{refused}");

    // The copy left nothing behind: the lock is the one check wrote, and a plain run runs.
    assert_eq!(program.read("lengua.lock"), lock);
    assert_eq!(program.read("main.wat").as_deref(), Some(ADD_CODE));
    let plain = session
        .call_ok("run", asking(&program, &["2", "3"], &json!({})))
        .await;
    assert_eq!(plain["stdout"], "5\n", "{plain}");

    // From a shell: lengua-mcp builds the copy; lengua-run, which compiles nothing, leaves the
    // run to lengua-mcp.
    let folder = program.path().as_os_str();
    let words = |first: &'static str| -> Vec<&OsStr> {
        let mut words: Vec<&OsStr> = Vec::new();
        if !first.is_empty() {
            words.push(OsStr::new(first));
        }
        words.extend([
            folder,
            OsStr::new("--stop-at"),
            OsStr::new("$parse_i64"),
            OsStr::new("--show-memory"),
            OsStr::new("2080:12"),
            OsStr::new("2"),
            OsStr::new("3"),
        ]);
        words
    };
    let mut shells = vec![(launcher(), "run")];
    if let Some(alone) = lengua_run() {
        shells.push((alone, ""));
    }
    for (shell, first) in shells {
        let ran = from_shell(&shell, &session, &words(first));
        assert_eq!(ran.status.code(), Some(0), "{ran:?}");
        assert!(ran.stdout.is_empty(), "{ran:?}");
        let said = String::from_utf8_lossy(&ran.stderr).into_owned();
        assert!(
            said.contains(&format!(
                "lengua-mcp: stop: stopped at line {entry} of main.wat, in $parse_i64"
            )),
            "{said}"
        );
        assert!(
            said.contains("lengua-mcp: stop: local $at: i32 10 (0x0000000a)"),
            "{said}"
        );
        assert!(
            said.contains("lengua-mcp: stop: memory: 6d 61 69 6e 2e 77 61 73 6d 00 32 00"),
            "{said}"
        );
    }
    session.service.cancel().await.expect("close the session");
}

/// secret-equal's idea, whose `- Secret:` line names standard input.
const SECRET_IDEA: &str = include_str!("../examples/secret-equal/idea.md");

/// Its code.
const SECRET_CODE: &str = include_str!("../examples/secret-equal/main.wat");

/// For an idea that names a secret, a trace holds the calls' names and the host's answers, and
/// no number the program passed; a stop, which shows values, is refused; and a profile, which
/// shows none, is given.
#[tokio::test]
async fn a_secret_idea_gets_a_trace_of_names_and_no_stop() {
    let session = Session::start(Protocol::Handshake).await;
    let program = checked(&session, SECRET_IDEA, SECRET_CODE).await;
    let mut arguments = asking(&program, &[], &json!({ "trace": true, "profile": true }));
    arguments["stdin"] = json!("aba\naba\n");
    let traced = session.call_ok("run", arguments).await;
    assert_eq!(traced["stdout"], "same\n", "{traced}");
    let calls = strings(&traced["host_calls"]["calls"]);
    assert!(calls.contains(&"fd_read = 0".to_owned()), "{traced}");
    assert!(calls.contains(&"fd_write = 0".to_owned()), "{traced}");
    assert!(
        calls
            .iter()
            .all(|call| !call.contains('(') && !call.contains("exit code")),
        "a number the program passed: {traced}"
    );
    assert!(
        traced["host_calls"]["note"]
            .as_str()
            .is_some_and(|note| note.contains("names a secret")),
        "{traced}"
    );
    assert!(traced["profile"]["samples"].is_u64(), "{traced}");
    let mut arguments = asking(&program, &[], &json!({ "stop_at": "1" }));
    arguments["stdin"] = json!("aba\naba\n");
    let refused = session.call_error("run", arguments).await;
    assert!(refused.contains("names a secret"), "{refused}");
    // From a shell too.
    let folder = program.path().as_os_str();
    let ran = from_shell(
        &launcher(),
        &session,
        &[
            OsStr::new("run"),
            folder,
            OsStr::new("--stop-at"),
            OsStr::new("1"),
        ],
    );
    assert_eq!(ran.status.code(), Some(125), "{ran:?}");
    assert!(
        String::from_utf8_lossy(&ran.stderr).contains("names a secret"),
        "{ran:?}"
    );
    session.service.cancel().await.expect("close the session");
}

/// The example that splits its work.
const SPLIT_IDEA: &str = include_str!("../examples/histogram-split/idea.md");

/// Its code.
const SPLIT_CODE: &str = include_str!("../examples/histogram-split/main.wat");

/// A program that splits its work: its trace is the program's own calls, the call that splits
/// among them, and none of its pieces'; its profile is given; and a stop is refused.
#[tokio::test]
async fn a_program_that_splits_gets_its_own_trace_and_no_stop() {
    let session = Session::start(Protocol::Handshake).await;
    let program = checked(&session, SPLIT_IDEA, SPLIT_CODE).await;
    // Enough for each of the eight pieces to have work.
    let long = "ab\n".repeat(100_000);
    let mut arguments = asking(&program, &[], &json!({ "trace": true, "profile": true }));
    arguments["stdin"] = json!(long);
    let traced = session.call_ok("run", arguments).await;
    assert_eq!(
        traced["stdout"], "10 100000\n97 100000\n98 100000\n",
        "{traced}"
    );
    let calls = strings(&traced["host_calls"]["calls"]);
    assert!(
        calls
            .iter()
            .any(|call| call.starts_with("lengua_split_1.split(") && call.ends_with(" = 0")),
        "{traced}"
    );
    // Each piece exits too, and none of those exits is the program's own call.
    let exits = calls
        .iter()
        .filter(|call| call.starts_with("proc_exit"))
        .count();
    assert!(exits <= 1, "a piece's call is in the trace: {traced}");
    for part in ["host_calls", "profile"] {
        assert!(
            traced[part]["note"]
                .as_str()
                .is_some_and(|note| note.contains("pieces")),
            "{part}: {traced}"
        );
    }
    let mut arguments = asking(&program, &[], &json!({ "stop_at": "1" }));
    arguments["stdin"] = json!("ab\n");
    let refused = session.call_error("run", arguments).await;
    assert!(refused.contains("- Splits:"), "{refused}");
    session.service.cancel().await.expect("close the session");
}

/// The rounds the outside module below spins with an argument: enough of its own code for a
/// profile to sample, on any runner.
const ROUNDS: u32 = 200_000_000;

/// An idea of an outside module that prints `ok` and a newline when it gets no arguments.
fn outside_idea() -> String {
    "---\ntype: Attested Computation\ntitle: spin\ndescription: Prints one line of \
     text.\nruntime: webassembly\ncomputation: main.wasm\nexecutor:\n  resource: \
     https://github.com/laughing-hermit/lengua/blob/main/docs/run.md\n  receipt: [stdout, \
     stderr, exit]\nattester:\n  resource: \
     https://github.com/laughing-hermit/lengua/blob/main/docs/check.md\n---\n# Purpose\nThe \
     program prints one line of text. A test uses it.\n\n# Behaviors\n## B1: Prints its \
     line\n- Given no arguments\n- When the program runs\n- Then it prints `ok` and a \
     newline\n- And it exits with code `0`\n\n# Constraints\n- Uses: nothing\n- Module: \
     main.wasm, built from WebAssembly text, in a test\n"
        .to_owned()
}

/// A module that prints `ok` and a newline, then exits with code 0; with an argument, it first
/// spins in `$spin`, a loop of [`ROUNDS`] rounds, which no rule of main.wat's would take.
fn spinning_module() -> String {
    format!(
        r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $sizes (param i32 i32) (result i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 64) "ok\0a")
  (func $spin (param $rounds i32) (result i32)
    (local $i i32)
    (local $sum i32)
    (loop $again
      (local.set $sum (i32.add (local.get $sum) (i32.mul (local.get $i) (i32.const 3))))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br_if $again (i32.lt_u (local.get $i) (local.get $rounds))))
    (local.get $sum))
  (func $start (export "_start")
    (drop (call $sizes (i32.const 0) (i32.const 4)))
    (if (i32.gt_u (i32.load (i32.const 0)) (i32.const 1))
      (then (i32.store (i32.const 8) (call $spin (i32.const {ROUNDS})))))
    (i32.store (i32.const 16) (i32.const 64))
    (i32.store (i32.const 20) (i32.const 3))
    (drop (call $fd_write (i32.const 1) (i32.const 16) (i32.const 1) (i32.const 32)))
    (call $exit (i32.const 0))))"#
    )
}

/// An outside module gets a trace and a profile as main.wat does, by the names its own name
/// section gives, and no stop: it has no main.wat to stop in. The profile of a run that spins
/// in one function names that function first, through the `run` tool and from a shell.
#[tokio::test]
async fn an_outside_module_gets_a_trace_and_a_profile_and_no_stop() {
    let session = Session::start(Protocol::Handshake).await;
    let program = Program::new();
    program.write("idea.md", &outside_idea());
    // The module with its names, as a compiler leaves them in.
    let built =
        lengua::wasm::text::build(&spinning_module()).unwrap_or_else(|problem| panic!("{problem}"));
    std::fs::write(program.file("main.wasm"), &built.named).expect("write main.wasm");
    session.call_ok("check", program.request(REQUEST)).await;
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], lengua::wasm::outside::STATE, "{reply}");

    let traced = session
        .call_ok("run", asking(&program, &[], &json!({ "trace": true })))
        .await;
    assert_eq!(traced["stdout"], "ok\n", "{traced}");
    assert_eq!(
        strings(&traced["host_calls"]["calls"]),
        [
            "args_sizes_get(0, 4) = 0",
            "fd_write(1, 16, 1, 32) = 0",
            "proc_exit(0) ended the run with exit code 0"
        ],
        "{traced}"
    );
    assert!(traced["outside"].is_string(), "{traced}");

    let more = json!({ "profile": true, "timeout_seconds": 40 });
    let profiled = session
        .call_ok("run", asking(&program, &["spin"], &more))
        .await;
    assert_eq!(profiled["stdout"], "ok\n", "{profiled}");
    let samples = profiled["profile"]["samples"].as_u64().unwrap_or(0);
    assert!(samples > 0, "{profiled}");
    let first = &profiled["profile"]["functions"][0];
    assert_eq!(first["function"], "$spin", "{profiled}");
    assert!(
        first["own"].as_u64().unwrap_or(0) * 2 > samples,
        "{profiled}"
    );
    assert!(
        first["with_calls"].as_u64() >= first["own"].as_u64(),
        "{profiled}"
    );

    let more = json!({ "stop_at": "$spin" });
    let refused = session
        .call_error("run", asking(&program, &[], &more))
        .await;
    assert!(refused.contains("an outside module"), "{refused}");

    // From a shell: the profile is said on standard error, in lines of one form.
    let folder = program.path().as_os_str();
    let ran = from_shell(
        &launcher(),
        &session,
        &[
            OsStr::new("run"),
            folder,
            OsStr::new("--profile"),
            OsStr::new("spin"),
        ],
    );
    assert_eq!(ran.stdout, b"ok\n", "{ran:?}");
    let said = String::from_utf8_lossy(&ran.stderr).into_owned();
    let shares: Vec<&str> = said
        .lines()
        .filter(|line| line.starts_with("lengua-mcp: profile: function "))
        .collect();
    assert!(
        shares.first().is_some_and(|line| line
            .starts_with("lengua-mcp: profile: function $spin: ")
            && line.ends_with(" with calls")),
        "{said}"
    );
    session.service.cancel().await.expect("close the session");
}

/// A profile of main.wat names its functions as main.wat does. add ends within a sample's
/// time, so its profile may hold no sample, and says so; the report has its form either way.
#[tokio::test]
async fn a_profile_of_a_short_run_has_its_form() {
    let session = Session::start(Protocol::Handshake).await;
    let program = checked(&session, ADD_IDEA, ADD_CODE).await;
    let more = json!({ "profile": true });
    let profiled = session
        .call_ok("run", asking(&program, &["2", "3"], &more))
        .await;
    assert_eq!(profiled["stdout"], "5\n", "{profiled}");
    let profile = &profiled["profile"];
    assert!(profile["samples"].is_u64(), "{profiled}");
    assert_eq!(profile["every_us"], 1000, "{profiled}");
    assert!(profile["functions"].is_array(), "{profiled}");
    assert!(profile["lines"].is_array(), "{profiled}");
    assert!(profile["note"].is_string(), "{profiled}");
    for share in profile["functions"].as_array().into_iter().flatten() {
        assert!(
            share["function"]
                .as_str()
                .is_some_and(|name| name.starts_with('$')),
            "{profiled}"
        );
    }
    session.service.cancel().await.expect("close the session");
}

/// `lengua-mcp lsp` serves an editor over standard input and output: it answers `initialize`,
/// publishes the findings of a folder's main.wat when the editor saves it, here the builder's
/// for a line that doesn't build, and exits with 0 after `shutdown` and `exit`.
#[test]
fn the_language_server_publishes_the_findings_of_a_saved_file() {
    use std::io::Write as _;
    use std::process::Stdio;

    let program = Program::new();
    program.write("idea.md", ADD_IDEA);
    let broken = ADD_CODE.replacen("(local.set $err", "(local.sett $err", 1);
    assert_ne!(broken, ADD_CODE);
    program.write("main.wat", &broken);
    let path = program.path().to_string_lossy().replace('\\', "/");
    let root = if path.starts_with('/') { "" } else { "/" };
    let uri = format!("file://{root}{path}/main.wat");
    let frame = |message: &Value| {
        let body = message.to_string();
        format!("Content-Length: {}\r\n\r\n{body}", body.len())
    };
    let mut server = Command::new(launcher())
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env_remove("LENGUA_LOG")
        .spawn()
        .expect("it starts");
    let mut input = server.stdin.take().expect("its input");
    for message in [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        json!({ "jsonrpc": "2.0", "method": "textDocument/didSave",
                "params": { "textDocument": { "uri": uri } } }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }),
        json!({ "jsonrpc": "2.0", "method": "exit" }),
    ] {
        input
            .write_all(frame(&message).as_bytes())
            .expect("the message is written");
    }
    drop(input);
    let output = server.wait_with_output().expect("it ends");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let said = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(said.starts_with("Content-Length: "), "{said}");
    assert!(said.contains("\"textDocumentSync\""), "{said}");
    assert!(
        said.contains("\"method\":\"textDocument/publishDiagnostics\""),
        "{said}"
    );
    assert!(said.contains("\"source\":\"lengua\""), "{said}");
    assert!(said.contains(&format!("\"uri\":\"{uri}\"")), "{said}");
}
