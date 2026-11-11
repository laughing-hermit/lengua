//! The host's functions, each run from a module in this process.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use super::*;
use crate::runner::{End, Trap};
use crate::wasm::engine::{self, Setup};
use crate::wasm::job::Ending;
use crate::wasm::runner::{self, Executed};

const EXIT: &str = r#"(import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))"#;
const WRITE: &str = r#"(import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))"#;
const READ: &str = r#"(import "wasi_snapshot_preview1" "fd_read"
    (func $fd_read (param i32 i32 i32 i32) (result i32)))"#;
const OPEN: &str = r#"(import "wasi_snapshot_preview1" "path_open"
    (func $open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))"#;
const CLOSE: &str =
    r#"(import "wasi_snapshot_preview1" "fd_close" (func $close (param i32) (result i32)))"#;

/// A module with `imports`, one page of memory exported, and `rest`.
fn module(imports: &[&str], rest: &str) -> String {
    format!(
        "(module {} (memory (export \"memory\") 1) {rest})",
        imports.join(" ")
    )
}

/// A test's world: nothing named, no input, output kept.
pub(super) fn world(policy: Policy) -> World {
    World {
        policy,
        args: Vec::new(),
        env: BTreeMap::new(),
        stdin: stdio::Input::given(Vec::new(), &[]),
        stdout: stdio::Output::kept(1 << 20),
        stderr: stdio::Output::kept(1 << 20),
        folder: None,
        reversed: false,
        listener: None,
        connections: Vec::new(),
        datagrams: None,
        internet: None,
        clock: Vec::new(),
        random: None,
        limits: limits::Limits::new(
            None,
            Arc::new(AtomicBool::new(false)),
            1 << 20,
            Some(1 << 20),
            Some(1 << 22),
            0,
        ),
        instant_sleeps: true,
        events: None,
    }
}

/// Runs the module `text` given `world`, for ten seconds at most.
fn run(text: &str, world: World) -> Executed {
    run_for(text, world, Duration::from_secs(10))
}

/// Runs the module `text` given `world`, for `time` at most.
fn run_for(text: &str, world: World, time: Duration) -> Executed {
    let engine = engine::engine(Setup::Test, None).expect("the engine");
    let linker = linker(&engine).expect("the host");
    let built = crate::wasm::text::build(text).unwrap_or_else(|problem| panic!("{problem}"));
    let module = wasmtime::Module::from_binary(&engine, &built.wasm).expect("it compiles");
    runner::execute(
        &engine,
        &linker,
        &module,
        Setup::Test,
        Host::new(world),
        Some(time),
    )
}

/// What a stream kept.
fn kept(output: &stdio::Output) -> Vec<u8> {
    match output {
        stdio::Output::Kept { bytes, .. } => bytes.clone(),
        stdio::Output::Closed | stdio::Output::Here(_) => Vec::new(),
    }
}

/// The code a run exited with.
fn exited(executed: &Executed) -> i32 {
    match &executed.ending {
        Ending::Exited(code) => *code,
        other => panic!("it didn't exit: {other:?} {:?}", executed.problem),
    }
}

/// Runs the module `text` given `world`, as [`run`] does, and says how it went as the runner
/// does, with a trap on main.wat's line.
fn run_placed(text: &str, world: World) -> crate::runner::Run {
    let built = crate::wasm::text::build(text).unwrap_or_else(|problem| panic!("{problem}"));
    let executed = run(text, world);
    runner::run_of(
        runner::done(0, executed, Vec::new()),
        (Ok(BTreeMap::new()), std::collections::BTreeSet::new()),
        |offset| built.line_at(offset),
    )
}

/// A folder holding `files`, a name ending in `/` for a folder, as a program's working folder.
fn folder_of(files: &[(&str, &str)]) -> (tempfile::TempDir, cap_std::fs::Dir) {
    let folder = tempfile::tempdir().expect("a folder");
    for (name, contents) in files {
        match name.strip_suffix('/') {
            Some(name) => std::fs::create_dir_all(folder.path().join(name)).expect("a folder"),
            None => std::fs::write(folder.path().join(name), contents).expect("a file"),
        }
    }
    let dir = cap_std::fs::Dir::open_ambient_dir(folder.path(), cap_std::ambient_authority())
        .expect("the folder");
    (folder, dir)
}

/// A policy that names files.
fn files() -> Policy {
    Policy {
        files: true,
        ..Policy::default()
    }
}

#[test]
fn fd_write_writes_standard_output_and_the_count() {
    let text = module(
        &[WRITE, EXIT],
        r#"(data (i32.const 100) "hi")
  (func (export "_start")
    (i32.store (i32.const 0) (i32.const 100))
    (i32.store (i32.const 4) (i32.const 2))
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 8)))
    (call $exit (i32.load (i32.const 8))))"#,
    );
    let executed = run(&text, world(Policy::default()));
    assert_eq!(exited(&executed), 2);
    assert_eq!(kept(&executed.host.stdout), b"hi");
    assert!(executed.instructions.is_some_and(|count| count > 0));
}

#[test]
fn args_sizes_get_and_args_get_give_the_name_and_the_arguments() {
    let sizes = r#"(import "wasi_snapshot_preview1" "args_sizes_get"
    (func $sizes (param i32 i32) (result i32)))"#;
    let args = r#"(import "wasi_snapshot_preview1" "args_get"
    (func $args (param i32 i32) (result i32)))"#;
    let text = module(
        &[sizes, args, WRITE, EXIT],
        r#"(func (export "_start")
    (drop (call $sizes (i32.const 0) (i32.const 4)))
    (drop (call $args (i32.const 16) (i32.const 64)))
    (i32.store (i32.const 8) (i32.load (i32.const 20)))
    (i32.store (i32.const 12) (i32.sub (i32.load (i32.const 4)) (i32.const 11)))
    (drop (call $fd_write (i32.const 1) (i32.const 8) (i32.const 1) (i32.const 32)))
    (call $exit (i32.load (i32.const 0))))"#,
    );
    let executed = run(
        &text,
        World {
            args: vec!["first".to_owned()],
            ..world(Policy::default())
        },
    );
    // Its name, main.wasm, and the argument.
    assert_eq!(exited(&executed), 2);
    assert_eq!(kept(&executed.host.stdout), b"first");
}

#[test]
fn the_environment_is_empty_unless_the_idea_names_it() {
    let sizes = r#"(import "wasi_snapshot_preview1" "environ_sizes_get"
    (func $sizes (param i32 i32) (result i32)))"#;
    let text = module(
        &[sizes, EXIT],
        r#"(func (export "_start")
    (drop (call $sizes (i32.const 0) (i32.const 4)))
    (call $exit (i32.add (i32.mul (i32.load (i32.const 0)) (i32.const 100))
      (i32.load (i32.const 4)))))"#,
    );
    let env = BTreeMap::from([("NAME".to_owned(), "value".to_owned())]);
    let without = run(
        &text,
        World {
            env: env.clone(),
            ..world(Policy::default())
        },
    );
    assert_eq!(exited(&without), 0);
    let with = run(
        &text,
        World {
            env,
            ..world(Policy {
                environment: true,
                ..Policy::default()
            })
        },
    );
    // One variable, "NAME=value" and its NUL.
    assert_eq!(exited(&with), 111);
}

#[test]
fn proc_exit_keeps_the_low_eight_bits_as_every_system_does() {
    let text = module(
        &[EXIT],
        r#"(func (export "_start") (call $exit (i32.const 300)))"#,
    );
    assert_eq!(exited(&run(&text, world(Policy::default()))), 44);
}

#[test]
fn fd_read_reads_standard_input_only_when_the_idea_names_it() {
    let text = module(
        &[READ, EXIT],
        r#"(func (export "_start")
    (local $errno i32)
    (i32.store (i32.const 0) (i32.const 64))
    (i32.store (i32.const 4) (i32.const 16))
    (local.set $errno (call $fd_read (i32.const 0) (i32.const 0) (i32.const 1) (i32.const 8)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (call $exit (i32.load (i32.const 8))))"#,
    );
    let input = || stdio::Input::given(b"abc".to_vec(), &[]);
    let refused = run(
        &text,
        World {
            stdin: input(),
            ..world(Policy::default())
        },
    );
    assert_eq!(exited(&refused), i32::from(errno::NOTCAPABLE));
    let read = run(
        &text,
        World {
            stdin: input(),
            ..world(Policy {
                standard_input: true,
                ..Policy::default()
            })
        },
    );
    assert_eq!(exited(&read), 3);
    // Input in pieces: a read ends where a piece does.
    let pieces = run(
        &text,
        World {
            stdin: stdio::Input::given(b"abc".to_vec(), &[1, 2]),
            ..world(Policy {
                standard_input: true,
                ..Policy::default()
            })
        },
    );
    assert_eq!(exited(&pieces), 1);
}

#[test]
fn fd_fdstat_get_knows_only_the_descriptors_in_use() {
    let stat = r#"(import "wasi_snapshot_preview1" "fd_fdstat_get"
    (func $stat (param i32 i32) (result i32)))"#;
    let of = |fd: u32| {
        let call = format!("(call $stat (i32.const {fd}) (i32.const 0))");
        module(
            &[stat, EXIT],
            &format!(r#"(func (export "_start") (call $exit {call}))"#),
        )
    };
    let none = Policy::default();
    assert_eq!(exited(&run(&of(1), world(none))), 0);
    for stray in [3, 4, 5, 9] {
        assert_eq!(
            exited(&run(&of(stray), world(none))),
            i32::from(errno::BADF),
            "{stray}"
        );
    }
    let (_folder, dir) = folder_of(&[]);
    let with_folder = run(
        &of(3),
        World {
            folder: Some(dir),
            ..world(files())
        },
    );
    assert_eq!(exited(&with_folder), 0);
}

#[test]
fn random_get_gives_the_supply_and_zeros_to_who_doesnt_name_it() {
    let random = r#"(import "wasi_snapshot_preview1" "random_get"
    (func $random (param i32 i32) (result i32)))"#;
    let text = module(
        &[random, EXIT],
        r#"(func (export "_start")
    (drop (call $random (i32.const 0) (i32.const 2)))
    (call $exit (i32.add (i32.load8_u (i32.const 0)) (i32.load8_u (i32.const 1)))))"#,
    );
    let supplied = run(
        &text,
        World {
            random: Some(vec![40, 2]),
            ..world(Policy {
                random: true,
                ..Policy::default()
            })
        },
    );
    assert_eq!(exited(&supplied), 42);
    assert_eq!(supplied.host.random.asked(), Some(2));
    let unasked = run(
        &text,
        World {
            random: Some(vec![40, 2]),
            ..world(Policy::default())
        },
    );
    assert_eq!(exited(&unasked), 0);
}

#[test]
fn clock_time_get_reads_the_supply_and_needs_the_clock_named() {
    let time = r#"(import "wasi_snapshot_preview1" "clock_time_get"
    (func $time (param i32 i64 i32) (result i32)))"#;
    let text = module(
        &[time, EXIT],
        r#"(func (export "_start")
    (local $errno i32)
    (local.set $errno (call $time (i32.const 0) (i64.const 0) (i32.const 0)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (call $exit (i32.wrap_i64 (i64.div_u (i64.load (i32.const 0)) (i64.const 1000000000)))))"#,
    );
    let supplied = run(
        &text,
        World {
            clock: vec![Time {
                seconds: 7,
                nanoseconds: 500,
            }],
            ..world(Policy {
                clock: true,
                ..Policy::default()
            })
        },
    );
    assert_eq!(exited(&supplied), 7);
    let refused = run(&text, world(Policy::default()));
    assert_eq!(exited(&refused), i32::from(errno::NOTCAPABLE));
}

#[test]
fn poll_oneoff_ends_a_tests_sleep_at_once() {
    let poll = r#"(import "wasi_snapshot_preview1" "poll_oneoff"
    (func $poll (param i32 i32 i32 i32) (result i32)))"#;
    let text = module(
        &[poll, EXIT],
        r#"(func (export "_start")
    (i32.store (i32.const 16) (i32.const 1))
    (i64.store (i32.const 24) (i64.const 10000000000))
    (call $exit (i32.add
      (call $poll (i32.const 0) (i32.const 64) (i32.const 1) (i32.const 128))
      (i32.mul (i32.load (i32.const 128)) (i32.const 10)))))"#,
    );
    let slept = run(
        &text,
        world(Policy {
            clock: true,
            ..Policy::default()
        }),
    );
    // No error, one event, and far less than the ten seconds asked.
    assert_eq!(exited(&slept), 10);
    assert!(slept.wall_time_ns < 5_000_000_000);
}

#[test]
fn a_trap_names_its_kind_and_its_place() {
    let text = module(&[], r#"(func (export "_start") (unreachable))"#);
    let executed = run(&text, world(Policy::default()));
    let Ending::Trapped(Trap::Unreachable, offsets) = &executed.ending else {
        panic!("{:?}", executed.ending);
    };
    assert!(!offsets.is_empty());
}

#[test]
fn a_trap_and_a_host_calls_stop_are_placed_on_main_wat_s_lines() {
    // An `unreachable` on line 5, in a function after another.
    let unreachable = "(module
  (memory (export \"memory\") 1)
  (func $first (nop))
  (func (export \"_start\")
    (unreachable)))";
    let ran = run_placed(unreachable, world(Policy::default()));
    assert_eq!(ran.end, End::Trapped(Trap::Unreachable, Some(5)));
    // A buffer outside the memory, given to the host on line 7: the call's line.
    let outside = "(module
  (import \"wasi_snapshot_preview1\" \"fd_write\"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (memory (export \"memory\") 1)
  (func (export \"_start\")
    (i32.store (i32.const 0) (i32.const 70000))
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 8)))))";
    let ran = run_placed(outside, world(Policy::default()));
    assert_eq!(ran.end, End::Trapped(Trap::OutOfBounds, Some(7)));
    // The same buffer given to lengua's library, which stops in its own code: the line of
    // main.wat's call of it, line 4.
    let library = "(module
  (memory (export \"memory\") 1)
  (func (export \"_start\")
    (drop (call $write_all (i32.const 1) (i32.const 70000) (i32.const 2)))))";
    let ran = run_placed(library, world(Policy::default()));
    assert_eq!(ran.end, End::Trapped(Trap::OutOfBounds, Some(4)));
}

#[test]
fn a_buffer_outside_memory_stops_the_program_as_a_load_would() {
    let text = module(
        &[WRITE],
        r#"(func (export "_start")
    (i32.store (i32.const 0) (i32.const 70000))
    (i32.store (i32.const 4) (i32.const 2))
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 8))))"#,
    );
    let executed = run(&text, world(Policy::default()));
    assert!(
        matches!(executed.ending, Ending::Trapped(Trap::OutOfBounds, _)),
        "{:?}",
        executed.ending
    );
}

#[test]
fn a_module_without_memory_cant_reach_the_host() {
    let text =
        format!("(module {CLOSE} (func (export \"_start\") (drop (call $close (i32.const 9)))))");
    let executed = run(&text, world(Policy::default()));
    assert!(matches!(executed.ending, Ending::Trapped(Trap::Other, _)));
    assert!(
        executed
            .problem
            .as_deref()
            .is_some_and(|problem| problem.contains("memory")),
        "{:?}",
        executed.problem
    );
}

#[test]
fn a_function_lengua_doesnt_give_answers_nosys() {
    let sync =
        r#"(import "wasi_snapshot_preview1" "fd_sync" (func $sync (param i32) (result i32)))"#;
    let text = module(
        &[sync, EXIT],
        r#"(func (export "_start") (call $exit (call $sync (i32.const 1))))"#,
    );
    assert_eq!(
        exited(&run(&text, world(Policy::default()))),
        i32::from(errno::NOSYS)
    );
}

#[test]
fn a_program_past_its_time_is_stopped() {
    let text = module(
        &[],
        r#"(func (export "_start") (loop $forever (br $forever)))"#,
    );
    let executed = run_for(&text, world(Policy::default()), Duration::from_millis(200));
    assert_eq!(executed.ending, Ending::TimedOut);
}

#[test]
fn output_past_its_cap_stops_the_program() {
    let text = module(
        &[WRITE],
        r#"(data (i32.const 100) "hello")
  (func (export "_start")
    (i32.store (i32.const 0) (i32.const 100))
    (i32.store (i32.const 4) (i32.const 5))
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 8))))"#,
    );
    let executed = run(
        &text,
        World {
            stdout: stdio::Output::kept(4),
            ..world(Policy::default())
        },
    );
    assert_eq!(executed.ending, Ending::OutputTooLarge);
    assert_eq!(kept(&executed.host.stdout), b"hell");
}

#[test]
fn a_closed_standard_output_is_a_broken_pipe() {
    let text = module(
        &[WRITE, EXIT],
        r#"(func (export "_start")
    (i32.store (i32.const 0) (i32.const 100))
    (i32.store (i32.const 4) (i32.const 1))
    (call $exit (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 8))))"#,
    );
    let executed = run(
        &text,
        World {
            stdout: stdio::Output::Closed,
            ..world(Policy::default())
        },
    );
    assert_eq!(exited(&executed), i32::from(errno::PIPE));
}

/// A module that opens `name` from the working folder with `oflags` and `rights` and exits with
/// the error, or with 0 and the file written `written` when there is one.
fn opening(name: &str, oflags: u32, rights: u64, written: &str) -> String {
    let escaped: String = name.bytes().map(|byte| format!("\\{byte:02x}")).collect();
    let length = name.len();
    let size = written.len();
    module(
        &[OPEN, WRITE, CLOSE, EXIT],
        &format!(
            r#"(data (i32.const 100) "{escaped}")
  (data (i32.const 2000) "{written}")
  (func (export "_start")
    (local $errno i32)
    (local.set $errno (call $open (i32.const 3) (i32.const 0) (i32.const 100) (i32.const {length})
      (i32.const {oflags}) (i64.const {rights}) (i64.const 0) (i32.const 0) (i32.const 0)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (i32.store (i32.const 8) (i32.const 2000))
    (i32.store (i32.const 12) (i32.const {size}))
    (if (i32.const {size}) (then
      (local.set $errno (call $fd_write (i32.load (i32.const 0)) (i32.const 8) (i32.const 1)
        (i32.const 16)))
      (if (local.get $errno) (then (call $exit (local.get $errno))))))
    (call $exit (call $close (i32.load (i32.const 0)))))"#
        ),
    )
}

/// `path_open`'s ways: make, empty.
const CREATE: u32 = 1 | 8;

#[test]
fn path_open_makes_and_writes_a_file_in_the_working_folder() {
    let (folder, dir) = folder_of(&[]);
    let executed = run(
        &opening("out.txt", CREATE, wasi::rights::FD_WRITE, "hi"),
        World {
            folder: Some(dir),
            ..world(files())
        },
    );
    assert_eq!(exited(&executed), 0);
    assert_eq!(
        std::fs::read_to_string(folder.path().join("out.txt")).expect("the file"),
        "hi"
    );
}

#[test]
fn path_open_holds_every_name_to_every_systems_rules() {
    let (_folder, dir) = folder_of(&[("readme.txt", "hello"), ("sub/", "")]);
    let world_of = |dir: &cap_std::fs::Dir| World {
        folder: Some(dir.try_clone().expect("the folder")),
        ..world(files())
    };
    let read = wasi::rights::FD_READ;
    let write = wasi::rights::FD_WRITE;
    for (name, oflags, rights, wanted) in [
        ("readme.txt", 0, read, 0),
        // Byte-exact, even where the system's names ignore case.
        ("README.TXT", 0, read, errno::NOENT),
        // A new name that differs from one in its folder only in case.
        ("README.txt", CREATE, write, errno::EXIST),
        ("CON", CREATE, write, errno::INVAL),
        ("a:b", CREATE, write, errno::INVAL),
        ("REPORT~1.TXT", CREATE, write, errno::INVAL),
        ("../outside", CREATE, write, errno::NOTCAPABLE),
        ("readme.txt/inside", 0, read, errno::NOTDIR),
        ("nowhere/x", CREATE, write, errno::NOENT),
        ("sub", 0, write, errno::ISDIR),
    ] {
        let executed = run(&opening(name, oflags, rights, ""), world_of(&dir));
        assert_eq!(exited(&executed), i32::from(wanted), "{name}");
    }
}

#[test]
fn a_write_past_a_files_limit_is_cut_short_and_the_next_fails() {
    let (folder, dir) = folder_of(&[]);
    let mut limited = world(files());
    limited.limits.max_file = Some(4);
    // "hello" twice to a file of at most 4 bytes: the first write is cut to the limit, with no
    // error, so the program goes on and writes again; at the limit, that write fails.
    let text = module(
        &[OPEN, WRITE, EXIT],
        r#"(data (i32.const 100) "out.txt")
  (data (i32.const 200) "hello")
  (func (export "_start")
    (local $errno i32)
    (local.set $errno (call $open (i32.const 3) (i32.const 0) (i32.const 100) (i32.const 7)
      (i32.const 9) (i64.const 64) (i64.const 0) (i32.const 0) (i32.const 0)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (i32.store (i32.const 8) (i32.const 200))
    (i32.store (i32.const 12) (i32.const 5))
    (local.set $errno (call $fd_write (i32.load (i32.const 0)) (i32.const 8) (i32.const 1)
      (i32.const 16)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (if (i32.ne (i32.load (i32.const 16)) (i32.const 4)) (then (call $exit (i32.const 1))))
    (call $exit (call $fd_write (i32.load (i32.const 0)) (i32.const 8) (i32.const 1)
      (i32.const 16))))"#,
    );
    let executed = run(
        &text,
        World {
            folder: Some(dir),
            ..limited
        },
    );
    assert_eq!(exited(&executed), i32::from(errno::FBIG));
    assert_eq!(
        std::fs::read_to_string(folder.path().join("out.txt")).expect("the file"),
        "hell"
    );
}

#[test]
fn a_file_held_open_isnt_removed() {
    let unlink = r#"(import "wasi_snapshot_preview1" "path_unlink_file"
    (func $unlink (param i32 i32 i32) (result i32)))"#;
    let text = module(
        &[OPEN, unlink, EXIT],
        r#"(data (i32.const 100) "a.txt")
  (func (export "_start")
    (drop (call $open (i32.const 3) (i32.const 0) (i32.const 100) (i32.const 5) (i32.const 0)
      (i64.const 2) (i64.const 0) (i32.const 0) (i32.const 0)))
    (call $exit (call $unlink (i32.const 3) (i32.const 100) (i32.const 5))))"#,
    );
    let (folder, dir) = folder_of(&[("a.txt", "x")]);
    let executed = run(
        &text,
        World {
            folder: Some(dir),
            ..world(files())
        },
    );
    assert_eq!(exited(&executed), i32::from(errno::BUSY));
    assert!(folder.path().join("a.txt").exists());
}

/// The names of the `dirent`s in `bytes`, in order.
fn names_listed(bytes: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let mut at = 0;
    while at + 24 <= bytes.len() {
        let length = usize::try_from(u32::from_le_bytes([
            bytes[at + 16],
            bytes[at + 17],
            bytes[at + 18],
            bytes[at + 19],
        ]))
        .expect("a length");
        let name = &bytes[at + 24..(at + 24 + length).min(bytes.len())];
        names.push(String::from_utf8_lossy(name).into_owned());
        at += 24 + length;
    }
    names
}

#[test]
fn fd_readdir_lists_sorted_and_turned_round_in_a_second_run() {
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
    for (reversed, wanted) in [(false, ["a", "b", "c"]), (true, ["c", "b", "a"])] {
        let (_folder, dir) = folder_of(&[("b", ""), ("a", ""), ("c/", "")]);
        let executed = run(
            &text,
            World {
                folder: Some(dir),
                reversed,
                ..world(files())
            },
        );
        assert_eq!(exited(&executed), 0);
        assert!(executed.host.notes.listed);
        assert_eq!(names_listed(&kept(&executed.host.stdout)), wanted);
    }
}

#[test]
fn the_working_folder_is_a_preopen_named_dot_only_when_the_idea_names_files() {
    let prestat = r#"(import "wasi_snapshot_preview1" "fd_prestat_get"
    (func $prestat (param i32 i32) (result i32)))"#;
    let name = r#"(import "wasi_snapshot_preview1" "fd_prestat_dir_name"
    (func $name (param i32 i32 i32) (result i32)))"#;
    let text = module(
        &[prestat, name, EXIT],
        r#"(func (export "_start")
    (local $errno i32)
    (local.set $errno (call $prestat (i32.const 3) (i32.const 0)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    ;; The next descriptor isn't one: a toolchain's search for preopens ends there.
    (if (i32.ne (call $prestat (i32.const 4) (i32.const 32)) (i32.const 8))
      (then (call $exit (i32.const 1))))
    (local.set $errno (call $name (i32.const 3) (i32.const 16) (i32.load (i32.const 4))))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (call $exit (i32.add (i32.load8_u (i32.const 16))
      (i32.mul (i32.load (i32.const 4)) (i32.const 100)))))"#,
    );
    let (_folder, dir) = folder_of(&[]);
    let named = run(
        &text,
        World {
            folder: Some(dir),
            ..world(files())
        },
    );
    // `.`, 46, one byte long.
    assert_eq!(exited(&named), 146);
    let unnamed = run(&text, world(Policy::default()));
    assert_eq!(exited(&unnamed), i32::from(errno::BADF));
}

#[test]
fn folders_are_made_renamed_and_removed_only_when_empty() {
    let mkdir = r#"(import "wasi_snapshot_preview1" "path_create_directory"
    (func $mkdir (param i32 i32 i32) (result i32)))"#;
    let rename = r#"(import "wasi_snapshot_preview1" "path_rename"
    (func $rename (param i32 i32 i32 i32 i32 i32) (result i32)))"#;
    let rmdir = r#"(import "wasi_snapshot_preview1" "path_remove_directory"
    (func $rmdir (param i32 i32 i32) (result i32)))"#;
    let text = module(
        &[mkdir, rename, rmdir, OPEN, CLOSE, EXIT],
        r#"(data (i32.const 100) "d")
  (data (i32.const 110) "e")
  (data (i32.const 120) "e/f")
  (func (export "_start")
    (local $errno i32)
    (local.set $errno (call $mkdir (i32.const 3) (i32.const 100) (i32.const 1)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (local.set $errno (call $rename (i32.const 3) (i32.const 100) (i32.const 1) (i32.const 3)
      (i32.const 110) (i32.const 1)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (local.set $errno (call $open (i32.const 3) (i32.const 0) (i32.const 120) (i32.const 3)
      (i32.const 9) (i64.const 64) (i64.const 0) (i32.const 0) (i32.const 0)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    ;; Held open, the folder is busy; closed, it isn't empty.
    (if (i32.ne (call $rmdir (i32.const 3) (i32.const 110) (i32.const 1)) (i32.const 10))
      (then (call $exit (i32.const 1))))
    (drop (call $close (i32.load (i32.const 0))))
    (call $exit (call $rmdir (i32.const 3) (i32.const 110) (i32.const 1))))"#,
    );
    let (folder, dir) = folder_of(&[]);
    let executed = run(
        &text,
        World {
            folder: Some(dir),
            ..world(files())
        },
    );
    assert_eq!(exited(&executed), i32::from(errno::NOTEMPTY));
    assert!(folder.path().join("e").join("f").is_file());
    assert!(!folder.path().join("d").exists());
}

#[test]
fn fd_seek_and_path_filestat_get_give_places_and_sizes() {
    let seek = r#"(import "wasi_snapshot_preview1" "fd_seek"
    (func $seek (param i32 i64 i32 i32) (result i32)))"#;
    let stat = r#"(import "wasi_snapshot_preview1" "path_filestat_get"
    (func $stat (param i32 i32 i32 i32 i32) (result i32)))"#;
    let text = module(
        &[OPEN, seek, stat, EXIT],
        r#"(data (i32.const 100) "in.txt")
  (func (export "_start")
    (local $errno i32)
    (local.set $errno (call $open (i32.const 3) (i32.const 0) (i32.const 100) (i32.const 6)
      (i32.const 0) (i64.const 2) (i64.const 0) (i32.const 0) (i32.const 0)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (local.set $errno (call $seek (i32.load (i32.const 0)) (i64.const 0) (i32.const 2)
      (i32.const 8)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    ;; Standard output has no places.
    (if (i32.ne (call $seek (i32.const 1) (i64.const 0) (i32.const 0) (i32.const 8))
      (i32.const 70)) (then (call $exit (i32.const 1))))
    (local.set $errno (call $stat (i32.const 3) (i32.const 0) (i32.const 100) (i32.const 6)
      (i32.const 256)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (call $exit (i32.add
      (i32.mul (i32.wrap_i64 (i64.load (i32.const 8))) (i32.const 10))
      (i32.load8_u (i32.const 272)))))"#,
    );
    let (_folder, dir) = folder_of(&[("in.txt", "hello")]);
    let executed = run(
        &text,
        World {
            folder: Some(dir),
            ..world(files())
        },
    );
    // The end is at 5, and the name is a regular file, 4.
    assert_eq!(exited(&executed), 54);
}

#[test]
fn every_function_is_defined_with_the_tables_signature() {
    let engine = engine::engine(Setup::Test, None).expect("the engine");
    let linker = linker(&engine).expect("the host");
    let mut store = wasmtime::Store::new(&engine, Host::new(world(Policy::default())));
    for function in wasi::FUNCTIONS.iter().chain(wasi::NET_FUNCTIONS) {
        let defined = linker
            .get(&mut store, wasi::module_of(function), function.name)
            .ok()
            .and_then(wasmtime::Extern::into_func)
            .unwrap_or_else(|| panic!("{} isn't defined", function.name));
        let ty = defined.ty(&store);
        let as_value = |value: wasmtime::ValType| match value {
            wasmtime::ValType::I32 => Some(wasi::Value::I32),
            wasmtime::ValType::I64 => Some(wasi::Value::I64),
            _ => None,
        };
        let params: Vec<_> = ty.params().map(as_value).collect();
        let results: Vec<_> = ty.results().map(as_value).collect();
        let wanted: Vec<_> = function.params.iter().copied().map(Some).collect();
        assert_eq!(params, wanted, "{}", function.name);
        let wanted: Vec<_> = function.results.iter().copied().map(Some).collect();
        assert_eq!(results, wanted, "{}", function.name);
    }
}
