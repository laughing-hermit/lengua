//! What the review of lengua's WebAssembly host found (docs/reviews/host-2026-11-10.md), each
//! finding held by a test that failed before its fix: a run's working folder never holds lengua's
//! own folders; a folder's listings are held within a budget; a handed folder's files are read
//! only as regular files of a bounded size, never through a link or a pipe; a module a proof
//! found wrong stays refused when an earlier lock is put back; and a host call ends at the run's
//! deadline however much it was asked to do.

mod common;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use common::{ADD_CODE, ADD_IDEA, Program as Folder, Protocol, Session};
use lengua::runner::{End, Limits, Run};
use lengua::wasm::engine::Setup;
use lengua::wasm::{Policy, Pool, Program, Spec};
use serde_json::{Value, json};

/// This binary, which the pool's workers are.
fn launcher() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lengua-mcp"))
}

/// A file of the fixture `name`, in tests/fixtures.
fn fixture(name: &str, file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A program folder holding the fixture `name`, checked to done in `session`.
async fn checked(session: &Session, name: &str) -> Folder {
    let program = Folder::new();
    for file in ["idea.md", "main.wat"] {
        program.write(file, &fixture(name, file));
    }
    session.record(&program).await;
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    program
}

/// `run`'s arguments for `program` in the working folder `folder`.
fn in_folder(program: &Folder, folder: &Path) -> Value {
    let mut arguments = program.arguments();
    arguments["folder"] = json!(folder.to_str().expect("a UTF-8 path"));
    arguments
}

/// `lengua-mcp` with the session's data folder, and so its key, and `args`, from a shell, with
/// the environment variable `plugin_data` as `PLUGIN_DATA` when given.
fn from_shell(session: &Session, args: &[&OsStr], plugin_data: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"));
    command
        .arg("--data-dir")
        .arg(session.data_dir.path())
        .args(args)
        .stdin(Stdio::null())
        .env_remove("PLUGIN_DATA")
        .env_remove("CLAUDE_PLUGIN_DATA");
    if let Some(folder) = plugin_data {
        command.env("PLUGIN_DATA", folder);
    }
    command.output().expect("lengua-mcp runs")
}

/// What `ran` said on standard error.
fn said(ran: &Output) -> String {
    String::from_utf8_lossy(&ran.stderr).into_owned()
}

/// Finding 1. A program whose idea uses files reads and writes everything beneath its working
/// folder, so the working folder is never lengua's data folder, which holds the key that signs
/// locks, nor the folder of compiled modules, whose machine code the engine runs as it finds it,
/// nor a folder inside one of them: `run` refuses it, from MCP and from a shell, before anything
/// runs. The program's own folder is a working folder like any other.
#[tokio::test]
async fn a_working_folder_is_never_one_of_lengua_s_own() {
    let session = Session::start(Protocol::Discover).await;
    let program = checked(&session, "files").await;
    let data = session.data_dir.path().to_path_buf();
    let tested = data.join("tested");
    assert!(tested.is_dir(), "check kept the module it tested");
    for folder in [&data, &tested] {
        let refused = session.call_error("run", in_folder(&program, folder)).await;
        assert!(
            refused.contains("lengua's data folder"),
            "{}: {refused}",
            folder.display()
        );
        let ran = from_shell(
            &session,
            &[
                OsStr::new("run"),
                program.path().as_os_str(),
                OsStr::new("--in"),
                folder.as_os_str(),
            ],
            None,
        );
        assert_eq!(ran.status.code(), Some(125), "{ran:?}");
        assert!(ran.stdout.is_empty(), "{ran:?}");
        assert!(said(&ran).contains("lengua's data folder"), "{ran:?}");
    }
    // The folder of compiled modules the agent's harness gives the plugin, and a folder that
    // holds it.
    let harness = tempfile::tempdir().expect("the harness's folder");
    let plugin = harness.path().join("plugin");
    std::fs::create_dir_all(plugin.join("cache")).expect("the cache's folder");
    for folder in [plugin.join("cache"), plugin.clone()] {
        let ran = from_shell(
            &session,
            &[
                OsStr::new("run"),
                program.path().as_os_str(),
                OsStr::new("--in"),
                folder.as_os_str(),
            ],
            Some(&plugin),
        );
        assert_eq!(ran.status.code(), Some(125), "{ran:?}");
        assert!(said(&ran).contains("compiled modules"), "{ran:?}");
    }
    // The program's own folder, and any other, is a working folder.
    let own = session
        .call_ok("run", in_folder(&program, program.path()))
        .await;
    assert_eq!(own["exit_code"], 0, "{own}");
    let other = tempfile::tempdir().expect("a folder of the user's");
    std::fs::write(other.path().join("a.txt"), "hello").expect("a file");
    let ran = session
        .call_ok("run", in_folder(&program, other.path()))
        .await;
    assert_eq!(ran["stdout"], "1 5\n", "{ran}");
    session.service.cancel().await.expect("close the session");
}

/// Finding 1, on Linux: the folders where the system shows its processes, its devices and
/// itself as files are no working folder, nor is anything beneath them.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn a_working_folder_is_never_the_system_s_own_on_linux() {
    let session = Session::start(Protocol::Discover).await;
    let program = checked(&session, "files").await;
    for folder in ["/proc", "/proc/self", "/sys", "/dev", "/dev/shm"] {
        let refused = session
            .call_error("run", in_folder(&program, Path::new(folder)))
            .await;
        assert!(refused.contains("the system's own"), "{folder}: {refused}");
    }
    session.service.cancel().await.expect("close the session");
}

/// The module `text` builds.
fn module(text: &str) -> Program {
    Program::of_text(text).unwrap_or_else(|problem| panic!("{problem}"))
}

/// Runs `text` given `spec`, within `limits`.
async fn run(text: &str, spec: Spec<'_>, limits: Limits) -> Run {
    let pool = Pool::new(launcher(), None);
    pool.run(&module(text), spec, limits)
        .await
        .expect("the program runs")
}

/// A program that, 900 times, makes a folder, opens the working folder again and begins to list
/// it, keeping every descriptor: each listing is of a folder that changed since the one before,
/// so each is held apart. It exits with the first error, 0 for none.
const LISTS: &str = r#"(module
  (import "wasi_snapshot_preview1" "path_create_directory"
    (func $mkdir (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "path_open"
    (func $open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_readdir"
    (func $readdir (param i32 i32 i32 i64 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (local $i i32)
    (local $errno i32)
    (i32.store8 (i32.const 64) (i32.const 46))
    (i32.store8 (i32.const 80) (i32.const 110))
    (loop $next
      (i32.store8 (i32.const 81)
        (i32.add (i32.const 97) (i32.and (local.get $i) (i32.const 15))))
      (i32.store8 (i32.const 82)
        (i32.add (i32.const 97) (i32.and (i32.shr_u (local.get $i) (i32.const 4)) (i32.const 15))))
      (i32.store8 (i32.const 83)
        (i32.add (i32.const 97) (i32.and (i32.shr_u (local.get $i) (i32.const 8)) (i32.const 15))))
      (local.set $errno (call $mkdir (i32.const 3) (i32.const 80) (i32.const 4)))
      (if (local.get $errno) (then (call $exit (local.get $errno))))
      (local.set $errno (call $open (i32.const 3) (i32.const 0) (i32.const 64) (i32.const 1)
        (i32.const 2) (i64.const 0) (i64.const 0) (i32.const 0) (i32.const 96)))
      (if (local.get $errno) (then (call $exit (local.get $errno))))
      (local.set $errno (call $readdir (i32.load (i32.const 96)) (i32.const 128) (i32.const 1)
        (i64.const 0) (i32.const 104)))
      (if (local.get $errno) (then (call $exit (local.get $errno))))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br_if $next (i32.lt_u (local.get $i) (i32.const 900))))
    (call $exit (i32.const 0))))
"#;

/// Finding 2. The listings a program's descriptors hold are lengua's memory, not the program's,
/// so lengua holds them to 1,048,576 entries in all: the listing that would pass it answers
/// `IO` (29). Here the folder holds 1,200 files and one folder more each time, so the listings
/// kept pass a million entries before the 900th.
#[tokio::test(flavor = "multi_thread")]
async fn the_listings_a_program_holds_stay_within_a_budget() {
    let given: BTreeMap<String, String> = (0..1200)
        .map(|number| (format!("p{number:04}"), String::new()))
        .collect();
    let policy = Policy {
        files: true,
        ..Policy::default()
    };
    let spec = Spec {
        files: &given,
        ..Spec::new(&[], &[], policy)
    };
    let limits = Limits {
        timeout: Duration::from_secs(300),
        ..Limits::default()
    };
    let ran = run(LISTS, spec, limits).await;
    assert_eq!(ran.end, End::Exited(29), "{:?}", ran.supplied);
}

/// A program with 3 GiB of memory that asks for 3 GiB of random bytes in one call.
const RANDOM: &str = r#"(module
  (import "wasi_snapshot_preview1" "random_get" (func $random (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 49152 49152)
  (func (export "_start")
    (call $exit (call $random (i32.const 0) (i32.const -1073741824)))))
"#;

/// A program that makes a file and writes 96 buffers of 64 MiB to it in one call.
const WRITES: &str = r#"(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1100 1100)
  (func (export "_start")
    (local $i i32)
    (local $errno i32)
    (i32.store8 (i32.const 16) (i32.const 111))
    (local.set $errno (call $open (i32.const 3) (i32.const 0) (i32.const 16) (i32.const 1)
      (i32.const 1) (i64.const 64) (i64.const 0) (i32.const 0) (i32.const 8)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (loop $next
      (i32.store (i32.add (i32.const 1024) (i32.shl (local.get $i) (i32.const 3)))
        (i32.const 65536))
      (i32.store (i32.add (i32.const 1028) (i32.shl (local.get $i) (i32.const 3)))
        (i32.const 67108864))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br_if $next (i32.lt_u (local.get $i) (i32.const 96))))
    (call $exit
      (call $fd_write (i32.load (i32.const 8)) (i32.const 1024) (i32.const 96) (i32.const 12)))))
"#;

/// Finding 6. A host call ends at the run's deadline however much it was asked to do: one call
/// for 3 GiB of random bytes, and one write of 6 GiB to a file with no limit, each in a run of
/// one second, end as past their time within three seconds, not when the call is through.
#[tokio::test(flavor = "multi_thread")]
async fn a_host_call_ends_at_the_deadline() {
    let limits = Limits {
        timeout: Duration::from_secs(1),
        max_file_size: None,
        ..Limits::default()
    };
    let random = Policy {
        random: true,
        ..Policy::default()
    };
    let files = Policy {
        files: true,
        ..Policy::default()
    };
    for (name, text, policy) in [("random_get", RANDOM, random), ("fd_write", WRITES, files)] {
        let spec = Spec {
            setup: Setup::Run,
            max_written: None,
            alone: true,
            ..Spec::new(&[], &[], policy)
        };
        let ran = run(text, spec, limits).await;
        assert_eq!(ran.end, End::TimedOut, "{name}: {:?}", ran.supplied);
        let took = Duration::from_nanos(ran.wall_time_ns);
        assert!(took < Duration::from_secs(3), "{name} took {took:?}");
    }
}

/// `lengua-mcp check` of `folder`, the attester, with no time for proofs: what it printed, on
/// standard output and standard error. A check that still waits after `patience` is stopped, and
/// the test fails.
fn attest_within(folder: &Path, patience: Duration) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .arg("check")
        .arg(folder)
        .env("LENGUA_PROOF_SECONDS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("lengua-mcp starts");
    let started = Instant::now();
    while child.try_wait().expect("its state").is_none() {
        if started.elapsed() > patience {
            let _ = child.kill();
            let _ = child.wait();
            panic!("lengua-mcp check still waits after {patience:?}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let output = child.wait_with_output().expect("its output");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// How long a check of a handed folder may take before the test says it waits for ever.
const PATIENCE: Duration = Duration::from_secs(60);

/// Finding 3. A file of a handed folder that holds more than lengua reads of one is refused by
/// its size, not read whole: an idea.md of 32 MiB and a byte.
#[test]
fn a_file_past_its_size_is_not_read() {
    let program = Folder::new();
    let mut idea = String::from(ADD_IDEA);
    idea.push_str(&" ".repeat(32 * 1024 * 1024 + 1 - idea.len()));
    program.write("idea.md", &idea);
    program.write("main.wat", ADD_CODE);
    let said = attest_within(program.path(), PATIENCE);
    assert!(said.contains("holds more than"), "{said}");
    assert!(said.contains("not attested"), "{said}");
}

/// Finding 3, on a Unix. A file of a handed folder that is a link is not followed, and one that
/// is a named pipe is not waited on: the attester says each isn't a regular file, and ends.
#[cfg(unix)]
#[test]
fn a_link_or_a_pipe_is_not_read_as_a_program_s_file() {
    let outside = tempfile::tempdir().expect("a folder outside");
    let elsewhere = outside.path().join("private.md");
    std::fs::write(&elsewhere, ADD_IDEA).expect("a file outside");
    let linked = Folder::new();
    std::os::unix::fs::symlink(&elsewhere, linked.file("idea.md")).expect("a link");
    linked.write("main.wat", ADD_CODE);
    let said = attest_within(linked.path(), PATIENCE);
    assert!(said.contains("idea.md isn't a regular file"), "{said}");
    assert!(said.contains("not attested"), "{said}");

    let piped = Folder::new();
    piped.write("idea.md", ADD_IDEA);
    let made = Command::new("mkfifo")
        .arg(piped.file("main.wat"))
        .status()
        .expect("mkfifo runs");
    assert!(made.success(), "mkfifo made the pipe");
    let said = attest_within(piped.path(), PATIENCE);
    assert!(said.contains("main.wat isn't a regular file"), "{said}");
    assert!(said.contains("not attested"), "{said}");
}

/// Finding 3, through the server, on a Unix: `check` and `run` say the same of a linked idea.md,
/// and neither waits on a lengua.lock that is a named pipe.
#[cfg(unix)]
#[tokio::test]
async fn the_server_reads_a_handed_folder_s_files_the_same_way() {
    let session = Session::start(Protocol::Discover).await;
    let outside = tempfile::tempdir().expect("a folder outside");
    let elsewhere = outside.path().join("private.md");
    std::fs::write(&elsewhere, ADD_IDEA).expect("a file outside");
    let linked = Folder::new();
    std::os::unix::fs::symlink(&elsewhere, linked.file("idea.md")).expect("a link");
    for tool in ["check", "run"] {
        let refused = session.call_error(tool, linked.arguments()).await;
        assert!(refused.contains("isn't a regular file"), "{tool}: {refused}");
    }
    let piped = Folder::with_add_idea();
    let made = Command::new("mkfifo")
        .arg(piped.file("lengua.lock"))
        .status()
        .expect("mkfifo runs");
    assert!(made.success(), "mkfifo made the pipe");
    for tool in ["check", "run"] {
        let answered = tokio::time::timeout(PATIENCE, session.call(tool, piped.arguments())).await;
        assert!(answered.is_ok(), "{tool} waits on a pipe named lengua.lock");
    }
    session.service.cancel().await.expect("close the session");
}

/// The key `session`'s server signs locks with, from its data folder, to sign a lock as lengua
/// does.
fn signing_key(session: &Session) -> lengua::lock::Key {
    let key_bytes: [u8; 32] = std::fs::read(session.data_dir.path().join("key"))
        .expect("the signing key")
        .try_into()
        .expect("32 bytes");
    lengua::lock::Key::from_bytes(key_bytes)
}

/// What `run` says of a build a proof found wrong.
const WRONG_BUILD: &str = "The proof found inputs where main.wat is wrong";

/// What the locks below record a proof found wrong on add's B1.
const FOUND: &str = "with a = 1 and b = -1: it printed \"-0\"";

/// add, checked to done in `session`, then its lock made to record that a proof found its module
/// wrong, signed with the session's key, as the proof's own record is: the folder, and the bytes
/// of the lock from before the record.
async fn add_found_wrong(session: &Session) -> (Folder, Vec<u8>) {
    let program = Folder::new();
    program.write("idea.md", ADD_IDEA);
    session.record(&program).await;
    program.write("main.wat", ADD_CODE);
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    let earlier = std::fs::read(program.file("lengua.lock")).expect("the lock");
    let key = signing_key(session);
    let mut lock = lengua::lock::read(program.path(), Some(&key))
        .expect("a valid lock")
        .expect("a lock");
    let module = lock
        .code
        .as_ref()
        .expect("the code's stamp")
        .module_sha256
        .clone();
    lock.proofs = Some(Box::new(lengua::lock::ProofStamp {
        module_sha256: module,
        proven: Vec::new(),
        bounded: Vec::new(),
        solvers: "z3 and cvc5".to_owned(),
        wrong: vec![("B1".to_owned(), FOUND.to_owned())],
        unproven: Vec::new(),
    }));
    lengua::lock::write(program.path(), &lock, &key).expect("write the lock");
    (program, earlier)
}

/// `lengua-mcp run` of `program` with add's arguments, from a shell: whether it was refused as
/// a build a proof found wrong.
fn refused_as_wrong(session: &Session, program: &Folder) -> Result<(), String> {
    let ran = from_shell(
        session,
        &[
            OsStr::new("run"),
            program.path().as_os_str(),
            OsStr::new("2"),
            OsStr::new("3"),
        ],
        None,
    );
    if ran.status.code() == Some(125) && ran.stdout.is_empty() && said(&ran).contains(WRONG_BUILD)
    {
        Ok(())
    } else {
        Err(format!("{ran:?}"))
    }
}

/// Finding 5. A module a proof found wrong stays refused when the lock from before the proof's
/// record, which the user's key signed too, is put back in the folder: lengua keeps what a proof
/// found wrong in its data folder, by the idea and the module, once a run has read the record.
#[tokio::test]
async fn a_wrong_module_stays_refused_when_an_earlier_lock_is_put_back_after_a_run() {
    let session = Session::start(Protocol::Discover).await;
    let (program, earlier) = add_found_wrong(&session).await;
    refused_as_wrong(&session, &program).expect("the lock's record refuses it");
    std::fs::write(program.file("lengua.lock"), &earlier).expect("the earlier lock, put back");
    refused_as_wrong(&session, &program).expect("the earlier lock doesn't let it run");
    let refused = session
        .call_error("run", json!({ "path": program.path(), "args": ["2", "3"] }))
        .await;
    assert!(refused.contains(WRONG_BUILD), "{refused}");
    session.service.cancel().await.expect("close the session");
}

/// Finding 5, when it is `check` that read the record: the same, with no run in between.
#[tokio::test]
async fn a_wrong_module_stays_refused_when_an_earlier_lock_is_put_back_after_a_check() {
    let session = Session::start(Protocol::Discover).await;
    let (program, earlier) = add_found_wrong(&session).await;
    let again = session.settle(&program).await;
    assert_eq!(again["state"], "tests_failed", "{again}");
    std::fs::write(program.file("lengua.lock"), &earlier).expect("the earlier lock, put back");
    refused_as_wrong(&session, &program).expect("the earlier lock doesn't let it run");
    session.service.cancel().await.expect("close the session");
}
