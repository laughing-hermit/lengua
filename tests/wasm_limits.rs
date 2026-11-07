//! The limits and the promises of lengua's WebAssembly runtime, through its pool of workers as a
//! test runs a program, on every system: time, output, memory and a table; an empty environment and
//! standard input that ends; no descriptor but the ones a program is given; every system's names;
//! a file's limit and the files' total; nothing past the working folder, through a link or `..`;
//! folders listed in order; the test's supplies; sleeps that end at once; connections, made and
//! refused; a listener on port 0; and a worker that ends with its server.

use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use lengua::connect::Reach;
use lengua::runner::{End, Limits, OPEN_FILES, Run};
use lengua::supply::{Supply, Time};
use lengua::wasm::engine::Setup;
use lengua::wasm::host::limits::{ENTRY, TABLE_ELEMENTS};
use lengua::wasm::job::{self, Job, Request};
use lengua::wasm::runner::UNSTARTED;
use lengua::wasm::{Policy, Pool, Program, Spec};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

/// This binary, which the pool's workers are.
fn launcher() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lengua-mcp"))
}

/// The module `text` builds.
fn program(text: &str) -> Program {
    Program::of_text(text).unwrap_or_else(|problem| panic!("{problem}"))
}

const EXIT: &str = r#"(import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))"#;
const WRITE: &str = r#"(import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))"#;
const OPEN: &str = r#"(import "wasi_snapshot_preview1" "path_open"
    (func $open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))"#;

/// A module with `imports`, a page of memory, and `rest`.
fn module(imports: &[&str], rest: &str) -> String {
    format!(
        "(module {} (memory (export \"memory\") 1) {rest})",
        imports.join(" ")
    )
}

/// Runs `text` given `spec`, within `limits`.
async fn run(text: &str, spec: Spec<'_>, limits: Limits) -> Run {
    let pool = Pool::new(launcher(), None);
    pool.run(&program(text), spec, limits)
        .await
        .expect("the program runs")
}

/// The code a run exited with.
fn exited(run: &Run) -> i32 {
    match run.end {
        End::Exited(code) => code,
        other => panic!(
            "it didn't exit: {} ({:?})",
            other.describe(Limits::default()),
            run.supplied
        ),
    }
}

/// A policy that names files.
fn files() -> Policy {
    Policy {
        files: true,
        ..Policy::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_program_past_its_time_is_stopped() {
    let text = module(
        &[],
        r#"(func (export "_start") (loop $forever (br $forever)))"#,
    );
    let limits = Limits {
        timeout: Duration::from_millis(300),
        ..Limits::default()
    };
    let started = Instant::now();
    let ran = run(&text, Spec::new(&[], &[], Policy::default()), limits).await;
    assert_eq!(ran.end, End::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(20));
}

#[tokio::test(flavor = "multi_thread")]
async fn output_past_its_cap_stops_the_program_with_what_fit() {
    let text = module(
        &[WRITE, EXIT],
        r#"(func (export "_start")
    (local $i i32)
    (i32.store (i32.const 0) (i32.const 64))
    (i32.store (i32.const 4) (i32.const 100))
    (loop $more
      (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 8)))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br_if $more (i32.lt_u (local.get $i) (i32.const 1000))))
    (call $exit (i32.const 0)))"#,
    );
    let limits = Limits {
        max_output: 1000,
        ..Limits::default()
    };
    let ran = run(&text, Spec::new(&[], &[], Policy::default()), limits).await;
    assert_eq!(ran.end, End::OutputTooLarge);
    assert_eq!(ran.stdout.len(), 1000);
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_grows_only_as_far_as_its_module_declares() {
    let bounded = format!(
        r#"(module {EXIT} (memory (export "memory") 1 2)
  (func (export "_start")
    (local $first i32)
    (local.set $first (memory.grow (i32.const 1)))
    (call $exit (i32.add (i32.mul (local.get $first) (i32.const 10))
      (i32.eq (memory.grow (i32.const 1)) (i32.const -1))))))"#
    );
    let none = Policy::default();
    // The first page more fits its maximum of two; the next doesn't.
    assert_eq!(
        exited(&run(&bounded, Spec::new(&[], &[], none), Limits::default()).await),
        11
    );
    let unbounded = format!(
        r#"(module {EXIT} (memory (export "memory") 1)
  (func (export "_start")
    (call $exit (i32.eq (memory.grow (i32.const 1)) (i32.const -1)))))"#
    );
    // No maximum declared: its size is all it has.
    assert_eq!(
        exited(&run(&unbounded, Spec::new(&[], &[], none), Limits::default()).await),
        1
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_table_holds_no_more_entries_than_lengua_allows() {
    let table = |entries: usize| {
        format!(
            r#"(module {EXIT} (memory (export "memory") 1) (table {entries} funcref)
  (func (export "_start") (call $exit (i32.const 0))))"#
        )
    };
    let none = Policy::default();
    // As many as lengua allows: the program runs.
    let most = table(TABLE_ELEMENTS);
    let ran = run(&most, Spec::new(&[], &[], none), Limits::default()).await;
    assert_eq!(exited(&ran), 0);
    // One more, which the worker would hold in its own memory: the program never starts, and
    // the run says why.
    let more = table(TABLE_ELEMENTS + 1);
    let refused = run(&more, Spec::new(&[], &[], none), Limits::default()).await;
    assert_eq!(exited(&refused), UNSTARTED);
    assert!(
        refused
            .supplied
            .as_deref()
            .is_some_and(|problem| problem.contains("table")),
        "{:?}",
        refused.supplied
    );
    // A table that grows: to lengua's limit, then no further, with -1, as memory.grow gives.
    let below = TABLE_ELEMENTS - 1;
    let grows = format!(
        r#"(module {EXIT} (memory (export "memory") 1) (table 1 funcref)
  (func (export "_start")
    (local $first i32)
    (local.set $first (table.grow 0 (ref.null func) (i32.const {below})))
    (call $exit (i32.add (i32.mul (local.get $first) (i32.const 10))
      (i32.eq (table.grow 0 (ref.null func) (i32.const 1)) (i32.const -1))))))"#
    );
    let ran = run(&grows, Spec::new(&[], &[], none), Limits::default()).await;
    assert_eq!(exited(&ran), 11);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_environment_is_empty_unless_the_idea_names_it() {
    let sizes = r#"(import "wasi_snapshot_preview1" "environ_sizes_get"
    (func $sizes (param i32 i32) (result i32)))"#;
    let text = module(
        &[sizes, EXIT],
        r#"(func (export "_start")
    (drop (call $sizes (i32.const 0) (i32.const 4)))
    (call $exit (i32.load (i32.const 0))))"#,
    );
    let env = BTreeMap::from([("NAME".to_owned(), "value".to_owned())]);
    let without = Spec {
        env: &env,
        ..Spec::new(&[], &[], Policy::default())
    };
    assert_eq!(exited(&run(&text, without, Limits::default()).await), 0);
    let named = Policy {
        environment: true,
        ..Policy::default()
    };
    let with = Spec {
        env: &env,
        ..Spec::new(&[], &[], named)
    };
    assert_eq!(exited(&run(&text, with, Limits::default()).await), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn standard_input_arrives_in_its_pieces_and_then_ends() {
    let read = r#"(import "wasi_snapshot_preview1" "fd_read"
    (func $fd_read (param i32 i32 i32 i32) (result i32)))"#;
    // Reads until the end, and exits with how many reads gave something, times 10, plus the
    // bytes.
    let text = module(
        &[read, EXIT],
        r#"(func (export "_start")
    (local $reads i32)
    (local $total i32)
    (i32.store (i32.const 0) (i32.const 64))
    (i32.store (i32.const 4) (i32.const 100))
    (block $end
      (loop $more
        (drop (call $fd_read (i32.const 0) (i32.const 0) (i32.const 1) (i32.const 8)))
        (br_if $end (i32.eqz (i32.load (i32.const 8))))
        (local.set $reads (i32.add (local.get $reads) (i32.const 1)))
        (local.set $total (i32.add (local.get $total) (i32.load (i32.const 8))))
        (br $more)))
    (call $exit (i32.add (i32.mul (local.get $reads) (i32.const 10)) (local.get $total))))"#,
    );
    let named = Policy {
        standard_input: true,
        ..Policy::default()
    };
    let whole = Spec::new(&[], b"abc", named);
    assert_eq!(exited(&run(&text, whole, Limits::default()).await), 13);
    let pieced = Spec {
        stdin_pieces: &[1, 2],
        ..Spec::new(&[], b"abc", named)
    };
    assert_eq!(exited(&run(&text, pieced, Limits::default()).await), 23);
}

/// A module that exits with a bit for each descriptor from 3 to 10 that is open.
fn open_descriptors() -> String {
    let stat = r#"(import "wasi_snapshot_preview1" "fd_fdstat_get"
    (func $stat (param i32 i32) (result i32)))"#;
    module(
        &[stat, EXIT],
        r#"(func (export "_start")
    (local $fd i32)
    (local $open i32)
    (local.set $fd (i32.const 3))
    (loop $more
      (if (i32.eqz (call $stat (local.get $fd) (i32.const 0)))
        (then (local.set $open (i32.or (local.get $open)
          (i32.shl (i32.const 1) (i32.sub (local.get $fd) (i32.const 3)))))))
      (local.set $fd (i32.add (local.get $fd) (i32.const 1)))
      (br_if $more (i32.le_u (local.get $fd) (i32.const 10))))
    (call $exit (local.get $open)))"#,
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn a_program_holds_only_the_descriptors_its_idea_gives() {
    let text = open_descriptors();
    let nothing = Spec::new(&[], &[], Policy::default());
    assert_eq!(exited(&run(&text, nothing, Limits::default()).await), 0);
    // Files: the working folder, 3.
    assert_eq!(
        exited(&run(&text, Spec::new(&[], &[], files()), Limits::default()).await),
        0b1
    );
    // A server with two connections: the listener, 4, and the connections, 5 and 6.
    let network = Policy {
        listens: true,
        connections: 2,
        ..Policy::default()
    };
    let places = [Reach::Nowhere, Reach::Nowhere];
    let served = Spec {
        connections: &places,
        listen: Some((0, false)),
        ..Spec::new(&[], &[], network)
    };
    assert_eq!(exited(&run(&text, served, Limits::default()).await), 0b1110);
}

/// A module that opens `name` in its working folder, made new when `create`, and exits with
/// the error, 0 for none.
fn opens(name: &str, create: bool) -> String {
    let escaped: String = name.bytes().map(|byte| format!("\\{byte:02x}")).collect();
    let length = name.len();
    let (oflags, rights) = if create { (9, 64) } else { (0, 2) };
    module(
        &[OPEN, EXIT],
        &format!(
            r#"(data (i32.const 100) "{escaped}")
  (func (export "_start")
    (call $exit (call $open (i32.const 3) (i32.const 0) (i32.const 100) (i32.const {length})
      (i32.const {oflags}) (i64.const {rights}) (i64.const 0) (i32.const 0) (i32.const 0))))"#
        ),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn every_name_is_held_to_every_systems_rules() {
    let given = BTreeMap::from([("readme.txt".to_owned(), "hello".to_owned())]);
    for (name, create, wanted) in [
        ("readme.txt", false, 0),
        ("README.TXT", false, 44),
        ("README.txt", true, 20),
        ("CON", true, 28),
        ("nul.txt", true, 28),
        ("a?b", true, 28),
        ("trailing.", true, 28),
        ("REPORT~1.TXT", true, 28),
        ("\u{1f970}.txt", true, 25),
    ] {
        let spec = Spec {
            files: &given,
            ..Spec::new(&[], &[], files())
        };
        let ran = run(&opens(name, create), spec, Limits::default()).await;
        assert_eq!(exited(&ran), wanted, "{name}");
    }
    let long = "a".repeat(256);
    let spec = Spec::new(&[], &[], files());
    assert_eq!(
        exited(&run(&opens(&long, true), spec, Limits::default()).await),
        37
    );
}

/// A module that writes `bytes` bytes to each of `names`, in turn, `times` times each, and exits
/// with the first error, 0 for none.
fn writes(names: &[&str], bytes: usize, times: usize) -> String {
    let mut body = String::new();
    for (index, name) in names.iter().enumerate() {
        let at = 100 + index * 32;
        body.push_str(&format!(
            r#"(data (i32.const {at}) "{name}")
"#
        ));
    }
    let write = r#"(local.set $errno
      (call $fd_write (i32.load (i32.const 0)) (i32.const 8) (i32.const 1) (i32.const 16)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
"#;
    let mut calls = String::new();
    for (index, name) in names.iter().enumerate() {
        let at = 100 + index * 32;
        let length = name.len();
        calls.push_str(&format!(
            r#"(local.set $errno (call $open (i32.const 3) (i32.const 0) (i32.const {at})
      (i32.const {length}) (i32.const 9) (i64.const 64) (i64.const 0) (i32.const 0)
      (i32.const 0)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
"#
        ));
        calls.push_str(&write.repeat(times));
    }
    module(
        &[OPEN, WRITE, EXIT],
        &format!(
            r#"{body}
  (func (export "_start")
    (local $errno i32)
    (i32.store (i32.const 8) (i32.const 1024))
    (i32.store (i32.const 12) (i32.const {bytes}))
    {calls}
    (call $exit (i32.const 0)))"#
        ),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_past_its_limit_is_cut_short_and_the_next_write_fails() {
    // Three writes of 6 bytes to one file of at most 10: 6, then 4, cut short with no error, and
    // then nothing more fits, so the third fails with FBIG.
    let text = writes(&["out.txt"], 6, 3);
    let limits = Limits {
        max_file_size: Some(10),
        ..Limits::default()
    };
    let ran = run(&text, Spec::new(&[], &[], files()), limits).await;
    assert_eq!(exited(&ran), 22);
    let written = ran.files.expect("its files");
    assert_eq!(written.get("out.txt").map(Vec::len), Some(10));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_files_total_holds_across_files() {
    // Two files, each an entry of the total, and two writes of 6 each, against a total of two
    // entries and 14 bytes: 12 go to the first, 2 to the second, and its next write fails with
    // FBIG.
    let text = writes(&["a.txt", "b.txt"], 6, 2);
    let spec = Spec {
        max_written: Some(2 * ENTRY + 14),
        ..Spec::new(&[], &[], files())
    };
    let ran = run(&text, spec, Limits::default()).await;
    assert_eq!(exited(&ran), 22);
    let written = ran.files.expect("its files");
    assert_eq!(written.get("a.txt").map(Vec::len), Some(12));
    assert_eq!(written.get("b.txt").map(Vec::len), Some(2));
}

#[tokio::test(flavor = "multi_thread")]
async fn each_file_and_folder_made_counts_against_the_files_total() {
    let mkdir = r#"(import "wasi_snapshot_preview1" "path_create_directory"
    (func $mkdir (param i32 i32 i32) (result i32)))"#;
    // Folders a and b, then the file d and the folder c, against a total of two entries: the
    // last two fail with FBIG, and neither is made.
    let text = module(
        &[mkdir, OPEN, EXIT],
        r#"(data (i32.const 100) "abcd")
  (func (export "_start")
    (local $errno i32)
    (local.set $errno (call $mkdir (i32.const 3) (i32.const 100) (i32.const 1)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (local.set $errno (call $mkdir (i32.const 3) (i32.const 101) (i32.const 1)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (local.set $errno (call $open (i32.const 3) (i32.const 0) (i32.const 103) (i32.const 1)
      (i32.const 9) (i64.const 64) (i64.const 0) (i32.const 0) (i32.const 0)))
    (if (i32.ne (local.get $errno) (i32.const 22))
      (then (call $exit (i32.add (i32.const 100) (local.get $errno)))))
    (call $exit (call $mkdir (i32.const 3) (i32.const 102) (i32.const 1))))"#,
    );
    let spec = Spec {
        max_written: Some(2 * ENTRY),
        ..Spec::new(&[], &[], files())
    };
    let ran = run(&text, spec, Limits::default()).await;
    assert_eq!(exited(&ran), 22);
    assert!(ran.files.expect("its files").is_empty());
    assert_eq!(
        ran.folders,
        BTreeSet::from(["a".to_owned(), "b".to_owned()])
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_write_past_a_files_end_counts_its_gap_against_the_files_total() {
    let seek = r#"(import "wasi_snapshot_preview1" "fd_seek"
    (func $seek (param i32 i64 i32 i32) (result i32)))"#;
    // out.txt, an entry of the total, then a byte at 50 and one at 1,000, against a total of an
    // entry and 100 bytes: the first byte and its gap of 50 fit, the second's gap of 949 doesn't.
    let text = module(
        &[OPEN, seek, WRITE, EXIT],
        r#"(data (i32.const 100) "out.txt")
  (data (i32.const 200) "x")
  (func (export "_start")
    (local $errno i32)
    (local.set $errno (call $open (i32.const 3) (i32.const 0) (i32.const 100) (i32.const 7)
      (i32.const 9) (i64.const 64) (i64.const 0) (i32.const 0) (i32.const 0)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (i32.store (i32.const 8) (i32.const 200))
    (i32.store (i32.const 12) (i32.const 1))
    (local.set $errno (call $seek (i32.load (i32.const 0)) (i64.const 50) (i32.const 0)
      (i32.const 24)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (local.set $errno (call $fd_write (i32.load (i32.const 0)) (i32.const 8) (i32.const 1)
      (i32.const 16)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (local.set $errno (call $seek (i32.load (i32.const 0)) (i64.const 1000) (i32.const 0)
      (i32.const 24)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (call $exit (call $fd_write (i32.load (i32.const 0)) (i32.const 8) (i32.const 1)
      (i32.const 16))))"#,
    );
    let spec = Spec {
        max_written: Some(ENTRY + 100),
        ..Spec::new(&[], &[], files())
    };
    let ran = run(&text, spec, Limits::default()).await;
    assert_eq!(exited(&ran), 22);
    let written = ran.files.expect("its files");
    assert_eq!(written.get("out.txt").map(Vec::len), Some(51));
}

/// Makes a link at `at` to the folder `target`, if this system lets the test.
#[cfg(unix)]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

/// Makes a link at `at` to the folder `target`, if this system lets the test.
#[cfg(windows)]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, at)
}

/// A module that opens `name` in `folder`, as [`opens`] does, and the run's spec.
fn in_folder<'a>(folder: &'a Path, name: &str, create: bool) -> (String, Spec<'a>) {
    let spec = Spec {
        folder: Some(folder),
        ..Spec::new(&[], &[], files())
    };
    (opens(name, create), spec)
}

#[tokio::test(flavor = "multi_thread")]
async fn nothing_past_the_working_folder_is_reached() {
    let outside = tempfile::tempdir().expect("a folder outside");
    std::fs::write(outside.path().join("secret.txt"), "no").expect("a file outside");
    let folder = tempfile::tempdir().expect("the working folder");
    // A way out of the working folder, which the idea doesn't give: NOTCAPABLE.
    for name in ["../secret.txt", "a/../../secret.txt", "/secret.txt"] {
        let (text, spec) = in_folder(folder.path(), name, false);
        assert_eq!(
            exited(&run(&text, spec, Limits::default()).await),
            76,
            "{name}"
        );
    }
    if link(outside.path(), &folder.path().join("away")).is_err() {
        lengua::skiplog::note("this system doesn't let the test make a link");
        return;
    }
    // A link, a name no program may use: ACCES.
    for name in ["away/secret.txt", "away"] {
        let (text, spec) = in_folder(folder.path(), name, false);
        assert_eq!(
            exited(&run(&text, spec, Limits::default()).await),
            2,
            "{name}"
        );
    }
    let (text, spec) = in_folder(folder.path(), "away/made.txt", true);
    assert_eq!(exited(&run(&text, spec, Limits::default()).await), 2);
    assert!(!outside.path().join("made.txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_folder_lists_in_order_and_turned_round_in_a_second_run() {
    let readdir = r#"(import "wasi_snapshot_preview1" "fd_readdir"
    (func $readdir (param i32 i32 i32 i64 i32) (result i32)))"#;
    let text = module(
        &[readdir, WRITE, EXIT],
        r#"(func (export "_start")
    (drop (call $readdir (i32.const 3) (i32.const 1024) (i32.const 4096) (i64.const 0)
      (i32.const 0)))
    (i32.store (i32.const 8) (i32.const 1024))
    (i32.store (i32.const 12) (i32.load (i32.const 0)))
    (drop (call $fd_write (i32.const 1) (i32.const 8) (i32.const 1) (i32.const 16)))
    (call $exit (i32.const 0)))"#,
    );
    let given = BTreeMap::from([
        ("b.txt".to_owned(), String::new()),
        ("a.txt".to_owned(), String::new()),
    ]);
    let folders = BTreeSet::from(["c".to_owned()]);
    for (reversed, wanted) in [
        (false, ["a.txt", "b.txt", "c"]),
        (true, ["c", "b.txt", "a.txt"]),
    ] {
        let spec = Spec {
            files: &given,
            folders: &folders,
            reversed,
            ..Spec::new(&[], &[], files())
        };
        let ran = run(&text, spec, Limits::default()).await;
        assert_eq!(exited(&ran), 0);
        assert!(ran.listed);
        let mut names = Vec::new();
        let mut at = 0;
        while at + 24 <= ran.stdout.len() {
            let mut length = [0_u8; 4];
            length.copy_from_slice(&ran.stdout[at + 16..at + 20]);
            let length = usize::try_from(u32::from_le_bytes(length)).expect("a length");
            let end = (at + 24 + length).min(ran.stdout.len());
            names.push(String::from_utf8_lossy(&ran.stdout[at + 24..end]).into_owned());
            at += 24 + length;
        }
        assert_eq!(names, wanted, "turned round: {reversed}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_test_supplies_the_clock_and_random_bytes() {
    let random = r#"(import "wasi_snapshot_preview1" "random_get"
    (func $random (param i32 i32) (result i32)))"#;
    let time = r#"(import "wasi_snapshot_preview1" "clock_time_get"
    (func $time (param i32 i64 i32) (result i32)))"#;
    let text = module(
        &[random, time, EXIT],
        r#"(func (export "_start")
    (drop (call $random (i32.const 0) (i32.const 1)))
    (drop (call $time (i32.const 0) (i64.const 0) (i32.const 8)))
    (call $exit (i32.add (i32.load8_u (i32.const 0))
      (i32.wrap_i64 (i64.div_u (i64.load (i32.const 8)) (i64.const 1000000000))))))"#,
    );
    let supply = Supply {
        random: Some(vec![40]),
        clock: vec![Time {
            seconds: 2,
            nanoseconds: 0,
        }],
    };
    let policy = Policy {
        clock: true,
        random: true,
        ..Policy::default()
    };
    let spec = Spec {
        supply: Some(&supply),
        ..Spec::new(&[], &[], policy)
    };
    let ran = run(&text, spec, Limits::default()).await;
    assert_eq!(exited(&ran), 42);
    assert_eq!(ran.random_asked, Some(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_call_takes_only_as_many_subscriptions_and_buffers_as_a_system_would() {
    let poll = r#"(import "wasi_snapshot_preview1" "poll_oneoff"
    (func $poll (param i32 i32 i32 i32) (result i32)))"#;
    // `count` subscriptions of zeros at 0, each a wait of no time on the time of day, and their
    // events at 512 KiB.
    let polls = |count: u64| {
        format!(
            r#"(module {poll} {EXIT} (memory (export "memory") 16)
  (func (export "_start")
    (call $exit (call $poll (i32.const 0) (i32.const 524288) (i32.const {count})
      (i32.const 1000000)))))"#
        )
    };
    let clock = Policy {
        clock: true,
        ..Policy::default()
    };
    // A read and a write for each descriptor a program may hold: every wait ends at once.
    let most = polls(2 * OPEN_FILES);
    let ran = run(&most, Spec::new(&[], &[], clock), Limits::default()).await;
    assert_eq!(exited(&ran), 0);
    // One more is INVAL, before lengua keeps any of them.
    let more = polls(2 * OPEN_FILES + 1);
    let ran = run(&more, Spec::new(&[], &[], clock), Limits::default()).await;
    assert_eq!(exited(&ran), 28);
    // As many buffers as every system's writev takes, all empty, and one more.
    let writes = |count: u32| {
        module(
            &[WRITE, EXIT],
            &format!(
                r#"(func (export "_start")
    (call $exit (call $fd_write (i32.const 1) (i32.const 0) (i32.const {count})
      (i32.const 16384))))"#
            ),
        )
    };
    let none = Policy::default();
    let most = writes(1024);
    let ran = run(&most, Spec::new(&[], &[], none), Limits::default()).await;
    assert_eq!(exited(&ran), 0);
    let more = writes(1025);
    let ran = run(&more, Spec::new(&[], &[], none), Limits::default()).await;
    assert_eq!(exited(&ran), 28);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tests_sleep_ends_at_once() {
    let poll = r#"(import "wasi_snapshot_preview1" "poll_oneoff"
    (func $poll (param i32 i32 i32 i32) (result i32)))"#;
    // A sleep of a minute.
    let text = module(
        &[poll, EXIT],
        r#"(func (export "_start")
    (i32.store (i32.const 16) (i32.const 1))
    (i64.store (i32.const 24) (i64.const 60000000000))
    (call $exit (call $poll (i32.const 0) (i32.const 64) (i32.const 1) (i32.const 128))))"#,
    );
    let policy = Policy {
        clock: true,
        ..Policy::default()
    };
    let started = Instant::now();
    let ran = run(&text, Spec::new(&[], &[], policy), Limits::default()).await;
    assert_eq!(exited(&ran), 0);
    assert!(started.elapsed() < Duration::from_secs(20));
}

/// A client that sends "ping" on its connection, descriptor 5, and writes what comes back.
const CLIENT: &str = r#"(module
  (import "wasi_snapshot_preview1" "sock_send"
    (func $send (param i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "sock_recv"
    (func $recv (param i32 i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (data (i32.const 100) "ping")
  (func (export "_start")
    (local $errno i32)
    (i32.store (i32.const 0) (i32.const 100))
    (i32.store (i32.const 4) (i32.const 4))
    (local.set $errno
      (call $send (i32.const 5) (i32.const 0) (i32.const 1) (i32.const 0) (i32.const 8)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (i32.store (i32.const 16) (i32.const 200))
    (i32.store (i32.const 20) (i32.const 64))
    (local.set $errno (call $recv (i32.const 5) (i32.const 16) (i32.const 1) (i32.const 2)
      (i32.const 24) (i32.const 28)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (i32.store (i32.const 20) (i32.load (i32.const 24)))
    (drop (call $fd_write (i32.const 1) (i32.const 16) (i32.const 1) (i32.const 32)))
    (call $exit (i32.const 0))))
"#;

#[tokio::test(flavor = "multi_thread")]
async fn a_connection_lengua_made_reaches_the_tests_server_and_a_refused_one_ends() {
    let policy = Policy {
        connections: 1,
        ..Policy::default()
    };
    let pool = Pool::new(launcher(), None);
    let client = program(CLIENT);
    let places = [Reach::Peer];
    let spec = Spec {
        connections: &places,
        ..Spec::new(&[], &[], policy)
    };
    let mut running = pool
        .start(&client, spec, Limits::default())
        .await
        .expect("it starts");
    let mut peers = running.take_peers();
    assert_eq!(peers.len(), 1);
    let (_, mut server) = peers.remove(0);
    let mut asked = [0_u8; 4];
    server.read_exact(&mut asked).await.expect("the request");
    assert_eq!(&asked, b"ping");
    server.write_all(b"pong").await.expect("the answer");
    server.shutdown().await.expect("the end");
    let ran = running.wait().await.expect("it ends");
    assert_eq!(exited(&ran), 0);
    assert_eq!(ran.stdout, b"pong");
    assert_eq!(ran.connections.len(), 1);
    assert_eq!(ran.connections[0].problem, None);

    // A place nobody answers: the connection comes ended, and a write to it is a broken pipe.
    let places = [Reach::Nowhere];
    let spec = Spec {
        connections: &places,
        ..Spec::new(&[], &[], policy)
    };
    let refused = pool
        .run(&client, spec, Limits::default())
        .await
        .expect("it runs");
    assert_eq!(exited(&refused), 64);
    assert!(refused.connections[0].problem.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_connection_lengua_makes_to_a_place_reaches_it_or_ends_refused() {
    let policy = Policy {
        connections: 1,
        ..Policy::default()
    };
    let pool = Pool::new(launcher(), None);
    let client = program(CLIENT);
    // A port of this computer that the test listens on: lengua connects to it as to any place,
    // and the program's bytes go there and back.
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("a free port");
    let port = listener.local_addr().expect("its address").port();
    let served = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("lengua's connection");
        let mut asked = [0_u8; 4];
        stream.read_exact(&mut asked).await.expect("the request");
        stream.write_all(b"pong").await.expect("the answer");
        stream.shutdown().await.expect("the end");
        asked
    });
    let places = [Reach::ThisComputer(port)];
    let spec = Spec {
        connections: &places,
        ..Spec::new(&[], &[], policy)
    };
    let ran = pool
        .run(&client, spec, Limits::default())
        .await
        .expect("it runs");
    assert_eq!(exited(&ran), 0);
    assert_eq!(ran.stdout, b"pong");
    assert_eq!(ran.connections[0].problem, None);
    assert_eq!(&served.await.expect("the test's server"), b"ping");

    // A port nobody listens on any more: the connection is refused, so the program gets it
    // already ended, a write to it is a broken pipe, and the run says why.
    let closed = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
    let port = closed.local_addr().expect("its address").port();
    drop(closed);
    let places = [Reach::ThisComputer(port)];
    let spec = Spec {
        connections: &places,
        ..Spec::new(&[], &[], policy)
    };
    let refused = pool
        .run(&client, spec, Limits::default())
        .await
        .expect("it runs");
    assert_eq!(exited(&refused), 64);
    assert!(refused.connections[0].problem.is_some());
}

/// A server that takes one connection, echoes what arrives once, and closes it.
const ECHO_SERVER: &str = r#"(module
  (import "wasi_snapshot_preview1" "sock_accept"
    (func $accept (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "sock_recv"
    (func $recv (param i32 i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "sock_send"
    (func $send (param i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $close (param i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (local $errno i32)
    (local.set $errno (call $accept (i32.const 4) (i32.const 0) (i32.const 0)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (i32.store (i32.const 16) (i32.const 200))
    (i32.store (i32.const 20) (i32.const 64))
    (local.set $errno (call $recv (i32.load (i32.const 0)) (i32.const 16) (i32.const 1)
      (i32.const 0) (i32.const 24) (i32.const 28)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (i32.store (i32.const 20) (i32.load (i32.const 24)))
    (local.set $errno (call $send (i32.load (i32.const 0)) (i32.const 16) (i32.const 1)
      (i32.const 0) (i32.const 32)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (call $exit (call $close (i32.load (i32.const 0))))))
"#;

#[tokio::test(flavor = "multi_thread")]
async fn a_server_listens_on_a_free_port_and_says_when_it_waits() {
    let policy = Policy {
        listens: true,
        ..Policy::default()
    };
    let pool = Pool::new(launcher(), None);
    let server = program(ECHO_SERVER);
    let spec = Spec {
        listen: Some((0, false)),
        ..Spec::new(&[], &[], policy)
    };
    let running = pool
        .start(&server, spec, Limits::default())
        .await
        .expect("it starts");
    let port = running.port().expect("its port");
    assert_ne!(port, 0);
    // It waits for its connection, and says so.
    let mut said = false;
    for _ in 0..200 {
        if running.waiting() {
            said = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(said, "the server didn't say it was waiting");
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("a connection");
    stream.write_all(b"hi").await.expect("sent");
    let mut answer = Vec::new();
    stream
        .read_to_end(&mut answer)
        .await
        .expect("the answer and the end");
    assert_eq!(answer, b"hi");
    let ran = running.wait().await.expect("it ends");
    assert_eq!(exited(&ran), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_still_serving_is_stopped() {
    let policy = Policy {
        listens: true,
        ..Policy::default()
    };
    let pool = Pool::new(launcher(), None);
    let server = program(ECHO_SERVER);
    let spec = Spec {
        listen: Some((0, false)),
        ..Spec::new(&[], &[], policy)
    };
    let running = pool
        .start(&server, spec, Limits::default())
        .await
        .expect("it starts");
    let ran = running.stop().await.expect("it stops");
    assert_eq!(ran.end, End::Stopped);
}

#[test]
fn a_worker_ends_when_its_server_does() {
    let looping = program(&module(
        &[],
        r#"(func (export "_start") (loop $forever (br $forever)))"#,
    ));
    let mut worker = std::process::Command::new(launcher())
        .arg(lengua::wasm::WORKER_FLAG)
        .arg("none")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("a worker");
    let mut requests = worker.stdin.take().expect("its input");
    let job = Job {
        id: 1,
        module: looping.file().to_path_buf(),
        hash: looping.hash().to_owned(),
        setup: Setup::Test,
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
            timeout_ms: Some(120_000),
            max_output: 1000,
            max_file: None,
            max_written: None,
        },
    };
    job::write_message(&mut requests, &Request::Run(Box::new(job))).expect("the job");
    // The program runs; then the server goes, and its end of the worker's input with it.
    std::thread::sleep(Duration::from_millis(500));
    drop(requests);
    let started = Instant::now();
    let ended = loop {
        if worker.try_wait().expect("the worker's state").is_some() {
            break true;
        }
        if started.elapsed() > Duration::from_secs(10) {
            break false;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if !ended {
        let _ = worker.kill();
    }
    assert!(
        ended,
        "the worker kept its program running after its server ended"
    );
}
