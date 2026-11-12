//! Work split into pieces (W20), through lengua's pool of workers as a test runs a program, on
//! every system, and through the checker and the idea's reader. Each test holds one of the
//! cases of docs/reviews/threads-2026-11-11.md: the answers come in the pieces' order whichever
//! piece ends first, and the same with the pieces one at a time, the last first; a piece has a
//! memory of its own, reaches nothing its idea grants the program, and can't split in its turn;
//! a call outside the idea's line is refused, and so is every call without the line; a piece
//! that doesn't end well ends the run as the first such piece in the order ended, with the
//! pieces after it stopped; the run's deadline holds its pieces; and what a piece prints is
//! held to its room.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use lengua::checker::wasm::{Context, check};
use lengua::runner::{End, Limits, Run, Trap};
use lengua::wasm::engine::Setup;
use lengua::wasm::runner::{Here, Source, run_here};
use lengua::wasm::{Policy, Pool, Program, Spec};

/// This binary, which the pool's workers are.
fn launcher() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lengua-mcp"))
}

/// What the idea of the programs here says: standard input, and work split into up to 4 pieces
/// of up to 8 bytes, each of which gives back 16 bytes.
const LINES: [&str; 2] = [
    "- Uses: standard input",
    "- Splits: its work into up to 4 pieces, each of up to 8 bytes, each gives back 16 bytes",
];

/// What an idea with `lines` in its `# Constraints` lets a program reach.
fn policy(lines: &[&str]) -> Policy {
    let (uses, network) =
        lengua::idea::constraints_of(lines).unwrap_or_else(|problems| panic!("{problems:?}"));
    Policy::of(&uses, &network)
}

/// How the program here is built.
#[derive(Clone, Copy)]
struct Shape<'a> {
    /// How many pieces its call splits into.
    pieces: u32,
    /// How many bytes its call keeps for the answers.
    room: u32,
    /// How many bytes it has to split: `None` for what it reads from standard input.
    held: Option<&'a str>,
}

impl Shape<'_> {
    /// Four pieces with 16 bytes each for their answers, of what standard input gives.
    const USUAL: Self = Shape {
        pieces: 4,
        room: 64,
        held: None,
    };
}

/// A program that splits what it reads from standard input, or holds, and prints its pieces'
/// answers. A piece answers with its bytes and a `|`, unless its first byte says otherwise:
/// `T` traps, `S` never ends, `D` answers after a while, `E` exits with code 9 after a while,
/// `F` exits with code 7 at once, `N` splits in its turn and exits with what that call
/// answered, `C` reads the clock and exits with what that call answered, `L` prints a byte
/// more than its room, and `M` answers with the digit of the byte at 1500 of its memory, where
/// the program wrote an `X` before it split, and writes a `Y` at 1600, which the program then
/// exits with code 99 for when it finds one there. `R` answers with the digit of the byte at
/// 1700 of its memory added to a global of its own, then makes the byte 4 and the global 3.
fn text(shape: Shape<'_>) -> String {
    let Shape { pieces, room, held } = shape;
    let (data, taken) = match held {
        Some(bytes) => (
            format!("(data (i32.const 1024) \"{bytes}\")"),
            format!("(i32.const {})", bytes.len()),
        ),
        None => (
            String::new(),
            "(call $take (i32.const 1024) (i32.const 400))".to_owned(),
        ),
    };
    format!(
        r#"(module
  (import "wasi_snapshot_preview1" "fd_read" (func $read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write" (func $write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "clock_time_get"
    (func $clock (param i32 i64 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (import "lengua_split_1" "split" (func $split (param i32 i32 i32 i32 i32) (result i32)))
  (import "lengua_split_1" "piece" (func $piece (result i32)))
  (memory (export "memory") 1 1)
  (global $seen (mut i32) (i32.const 0))
  {data}
  (func $spin (param $rounds i32)
    (loop $more
      (local.set $rounds (i32.sub (local.get $rounds) (i32.const 1)))
      (br_if $more (local.get $rounds))))
  (func $print (param $at i32) (param $len i32)
    (i32.store (i32.const 0) (local.get $at))
    (i32.store (i32.const 4) (local.get $len))
    (drop (call $write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 8))))
  (func $take (param $at i32) (param $room i32) (result i32)
    (i32.store (i32.const 0) (local.get $at))
    (i32.store (i32.const 4) (local.get $room))
    (i32.store (i32.const 8) (i32.const 0))
    (drop (call $read (i32.const 0) (i32.const 0) (i32.const 1) (i32.const 8)))
    (i32.load (i32.const 8)))
  (func $for_a_piece
    (local $n i32)
    (local $first i32)
    (local.set $n (call $take (i32.const 4096) (i32.const 64)))
    (local.set $first (i32.load8_u (i32.const 4096)))
    (if (i32.eq (local.get $first) (i32.const 84)) (then unreachable))
    (if (i32.eq (local.get $first) (i32.const 83)) (then (loop $ever (br $ever))))
    (if (i32.eq (local.get $first) (i32.const 68)) (then (call $spin (i32.const 20000000))))
    (if (i32.eq (local.get $first) (i32.const 69))
      (then (call $spin (i32.const 5000000)) (call $exit (i32.const 9))))
    (if (i32.eq (local.get $first) (i32.const 70)) (then (call $exit (i32.const 7))))
    (if (i32.eq (local.get $first) (i32.const 78))
      (then
        (call $exit
          (call $split (i32.const 4096) (local.get $n) (i32.const 2) (i32.const 8192)
            (i32.const 32)))))
    (if (i32.eq (local.get $first) (i32.const 67))
      (then (call $exit (call $clock (i32.const 0) (i64.const 0) (i32.const 16)))))
    (if (i32.eq (local.get $first) (i32.const 76))
      (then (call $print (i32.const 4096) (i32.const 17))))
    (if (i32.eq (local.get $first) (i32.const 77))
      (then
        (i32.store8 (i32.const 1600) (i32.const 89))
        (i32.store8 (i32.const 4096) (i32.add (i32.const 48) (i32.load8_u (i32.const 1500))))
        (call $print (i32.const 4096) (i32.const 1))
        (call $exit (i32.const 0))))
    (if (i32.eq (local.get $first) (i32.const 82))
      (then
        (i32.store8 (i32.const 4096)
          (i32.add (i32.const 48) (i32.add (i32.load8_u (i32.const 1700)) (global.get $seen))))
        (i32.store8 (i32.const 1700) (i32.const 4))
        (global.set $seen (i32.const 3))
        (call $print (i32.const 4096) (i32.const 1))
        (call $exit (i32.const 0))))
    (i32.store8 (i32.add (i32.const 4096) (local.get $n)) (i32.const 124))
    (call $print (i32.const 4096) (i32.add (local.get $n) (i32.const 1)))
    (call $exit (i32.const 0)))
  (func (export "_start")
    (local $n i32)
    (local $err i32)
    (if (i32.eqz (call $piece)) (then (call $for_a_piece)))
    (i32.store8 (i32.const 1500) (i32.const 88))
    (local.set $n {taken})
    (local.set $err
      (call $split (i32.const 1024) (local.get $n) (i32.const {pieces}) (i32.const 2048)
        (i32.const {room})))
    (if (local.get $err) (then (call $exit (local.get $err))))
    (if (i32.load8_u (i32.const 1600)) (then (call $exit (i32.const 99))))
    (call $print (i32.const 2048) (i32.const {room}))
    (call $exit (i32.const 0))))
"#
    )
}

/// Runs the program of `shape` on `stdin`, as `spec` changes a test's run, within `limits`.
async fn run_as(
    shape: Shape<'_>,
    stdin: &str,
    policy: Policy,
    change: impl FnOnce(Spec<'_>) -> Spec<'_>,
    limits: Limits,
) -> Run {
    let program = Program::of_text(&text(shape)).unwrap_or_else(|problem| panic!("{problem}"));
    let pool = Pool::new(launcher(), None);
    let spec = change(Spec::new(&[], stdin.as_bytes(), policy));
    pool.run(&program, spec, limits)
        .await
        .expect("the program runs")
}

/// Runs the usual program on `stdin`, as a test runs it.
async fn run(stdin: &str) -> Run {
    run_as(
        Shape::USUAL,
        stdin,
        policy(&LINES),
        |spec| spec,
        Limits::default(),
    )
    .await
}

/// The answers `pieces` give, each at the start of its 16 bytes, with zeros after it.
fn answers(pieces: &[&str]) -> Vec<u8> {
    let mut all = Vec::new();
    for piece in pieces {
        let mut slot = piece.as_bytes().to_vec();
        slot.resize(16, 0);
        all.extend(slot);
    }
    all
}

/// The answers come in the pieces' order, each at the start of its room with zeros after it,
/// also when the first piece ends last; the cut gives pieces that differ by a byte at most; a
/// run with the pieces one at a time, the last first, gives the same bytes, and counts the same
/// operations; and a run for someone, which counts nothing, gives the same bytes too.
#[tokio::test(flavor = "multi_thread")]
async fn the_answers_come_in_the_pieces_order_however_the_pieces_run() {
    let slow_first = "Daaabbbbccccdddd";
    let expected = answers(&["Daaa|", "bbbb|", "cccc|", "dddd|"]);
    let ran = run(slow_first).await;
    assert_eq!(ran.end, End::Exited(0), "{:?}", ran.supplied);
    assert_eq!(ran.stdout, expected);
    assert!(ran.split, "the run says it split its work");
    let turned = run_as(
        Shape::USUAL,
        slow_first,
        policy(&LINES),
        |spec| Spec {
            reversed: true,
            ..spec
        },
        Limits::default(),
    )
    .await;
    assert_eq!(turned.end, End::Exited(0), "{:?}", turned.supplied);
    assert_eq!(turned.stdout, expected);
    assert!(ran.instructions.is_some(), "a test's run is counted");
    assert_eq!(ran.instructions, turned.instructions);
    let for_someone = run_as(
        Shape::USUAL,
        slow_first,
        policy(&LINES),
        |spec| Spec {
            setup: Setup::Run,
            ..spec
        },
        Limits::default(),
    )
    .await;
    assert_eq!(for_someone.end, End::Exited(0));
    assert_eq!(for_someone.stdout, expected);
    // Ten bytes in four pieces: 2, 3, 2 and 3.
    let uneven = run("abcdefghij").await;
    assert_eq!(uneven.stdout, answers(&["ab|", "cde|", "fg|", "hij|"]));
    // No bytes are four pieces of none.
    let none = run("").await;
    assert_eq!(none.stdout, answers(&["|", "|", "|", "|"]));
}

/// Without the idea's line, `piece` and `split` answer `NOTCAPABLE` and nothing runs; with it,
/// a call the line doesn't cover answers `INVAL`: a piece more than it grants, room other than
/// the pieces times what each gives back, and a piece longer than its bytes.
#[tokio::test(flavor = "multi_thread")]
async fn a_call_outside_the_ideas_line_is_refused() {
    const NOTCAPABLE: i32 = 76;
    const INVAL: i32 = 28;
    let ungranted = run_as(
        Shape::USUAL,
        "aaaabbbbccccdddd",
        policy(&LINES[..1]),
        |spec| spec,
        Limits::default(),
    )
    .await;
    assert_eq!(ungranted.end, End::Exited(NOTCAPABLE));
    assert!(ungranted.stdout.is_empty() && !ungranted.split);
    let shapes = [
        (
            Shape {
                pieces: 5,
                room: 80,
                held: None,
            },
            "a piece more than the line grants",
        ),
        (
            Shape {
                pieces: 0,
                room: 0,
                held: None,
            },
            "no pieces",
        ),
        (
            Shape {
                pieces: 4,
                room: 63,
                held: None,
            },
            "too little room",
        ),
        (
            Shape {
                pieces: 4,
                room: 65,
                held: None,
            },
            "too much room",
        ),
    ];
    for (shape, what) in shapes {
        let refused = run_as(
            shape,
            "aaaabbbbccccdddd",
            policy(&LINES),
            |spec| spec,
            Limits::default(),
        )
        .await;
        assert_eq!(refused.end, End::Exited(INVAL), "{what}");
        assert!(refused.stdout.is_empty() && !refused.split, "{what}");
    }
    // 33 bytes in four pieces make one of 9, and the line gives a piece 8.
    let long = run(&"a".repeat(33)).await;
    assert_eq!(long.end, End::Exited(INVAL));
    let full = run(&"a".repeat(32)).await;
    assert_eq!(full.end, End::Exited(0));
    // Fewer pieces than the line grants are covered.
    let fewer = run_as(
        Shape {
            pieces: 2,
            room: 32,
            held: None,
        },
        "aaaabbbb",
        policy(&LINES),
        |spec| spec,
        Limits::default(),
    )
    .await;
    assert_eq!(fewer.stdout, answers(&["aaaa|", "bbbb|"]));
}

/// A piece is a run of its own: it sees nothing the program wrote in its memory, what it
/// writes there the program never sees, it can't split in its turn, and it reaches nothing
/// the idea grants the program, the clock here.
#[tokio::test(flavor = "multi_thread")]
async fn a_piece_has_its_own_memory_and_reaches_nothing() {
    const NOTCAPABLE: i32 = 76;
    let own = run("Maaabbbbccccdddd").await;
    assert_eq!(
        own.end,
        End::Exited(0),
        "a piece's write reached the program"
    );
    assert_eq!(own.stdout, answers(&["0", "bbbb|", "cccc|", "dddd|"]));
    let nested = run("aaaaNbbbccccdddd").await;
    assert_eq!(nested.end, End::Exited(NOTCAPABLE));
    assert!(nested.stdout.is_empty());
    let clock = run_as(
        Shape::USUAL,
        "aaaabbbbCcccdddd",
        policy(&["- Uses: standard input, the clock", LINES[1]]),
        |spec| spec,
        Limits::default(),
    )
    .await;
    assert_eq!(clock.end, End::Exited(NOTCAPABLE));
}

/// A piece that doesn't exit with code 0 ends the run as it ended, the first such piece in
/// the pieces' order whichever ended first, and nothing is printed; the pieces after it are
/// stopped, so pieces that would never end don't keep the run; and a piece that prints past its
/// room ends the run so.
#[tokio::test(flavor = "multi_thread")]
async fn a_piece_that_fails_ends_the_run_as_the_first_in_order_ended() {
    let limits = Limits {
        timeout: Duration::from_secs(30),
        ..Limits::default()
    };
    let started = Instant::now();
    let trapped = run_as(
        Shape::USUAL,
        "TaaaSbbbSbbbSbbb",
        policy(&LINES),
        |spec| spec,
        limits,
    )
    .await;
    assert!(
        matches!(trapped.end, End::Trapped(Trap::Unreachable, _)),
        "{:?}",
        trapped.end
    );
    assert!(trapped.stdout.is_empty());
    assert!(
        started.elapsed() < Duration::from_secs(15),
        "the pieces after the one that trapped kept the run for {:?}",
        started.elapsed()
    );
    // The second piece exits with code 7 at once, the first with code 9 after a while: 9.
    for reversed in [false, true] {
        let first = run_as(
            Shape::USUAL,
            "EaaaFbbbccccdddd",
            policy(&LINES),
            |spec| Spec { reversed, ..spec },
            Limits::default(),
        )
        .await;
        assert_eq!(first.end, End::Exited(9), "reversed: {reversed}");
        assert!(first.stdout.is_empty());
    }
    let long = run("aaaabbbbLcccdddd").await;
    assert_eq!(long.end, End::OutputTooLarge);
    assert!(long.stdout.is_empty());
}

/// The run's deadline holds its pieces: pieces that never end are stopped at it.
#[tokio::test(flavor = "multi_thread")]
async fn the_runs_deadline_stops_its_pieces() {
    let limits = Limits {
        timeout: Duration::from_millis(400),
        ..Limits::default()
    };
    let started = Instant::now();
    let ran = run_as(
        Shape::USUAL,
        "SaaaSbbbSbbbSbbb",
        policy(&LINES),
        |spec| spec,
        limits,
    )
    .await;
    assert_eq!(ran.end, End::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(20));
}

/// A run from a shell, in the engine that stops nothing, runs its pieces too.
#[test]
fn a_run_from_a_shell_runs_its_pieces() {
    let shape = Shape {
        held: Some("aaaabbbbccccdddd"),
        ..Shape::USUAL
    };
    let built = lengua::wasm::text::build(&text(shape)).expect("it builds");
    let folder = tempfile::tempdir().expect("a folder");
    let args: Vec<String> = Vec::new();
    let env: BTreeMap<String, String> = BTreeMap::new();
    let here = Here {
        args: &args,
        env: &env,
        folder: folder.path(),
        policy: policy(&LINES),
        listen: None,
        connections: &[],
        own: &[],
        source: Source::Built(&built),
    };
    let ended = run_here(&built.wasm, &here, None).expect("it runs");
    assert_eq!(ended, End::Exited(0));
    // A piece that traps ends that run as well.
    let trapping = Shape {
        held: Some("aaaaTbbbccccdddd"),
        ..Shape::USUAL
    };
    let built = lengua::wasm::text::build(&text(trapping)).expect("it builds");
    let here = Here {
        source: Source::Built(&built),
        ..here
    };
    let ended = run_here(&built.wasm, &here, None).expect("it runs");
    assert!(
        matches!(ended, End::Trapped(Trap::Unreachable, _)),
        "{ended:?}"
    );
}

/// The example that splits its work.
const IDEA: &str = include_str!("../examples/histogram-split/idea.md");

/// Its code.
const CODE: &str = include_str!("../examples/histogram-split/main.wat");

/// What the checker finds in `code` for an idea with `lines` in its `# Constraints`.
fn violations(code: &str, lines: &[&str]) -> Vec<String> {
    let built = lengua::wasm::text::build(code).unwrap_or_else(|problem| panic!("{problem}"));
    let (uses, network) =
        lengua::idea::constraints_of(lines).unwrap_or_else(|problems| panic!("{problems:?}"));
    let context = Context {
        uses: &uses,
        network: &network,
        secrets: &[],
    };
    check(code, &built, &context).lines()
}

/// The checker holds main.wat to the idea's line: without the line it imports neither
/// function; with it, the program and the pieces the line grants hold no more memory together
/// than a program may, a call splits into a named number of pieces the line covers, and a
/// module that splits asks whether it is a piece.
#[test]
fn the_checker_holds_main_wat_to_the_ideas_line() {
    let uses = "- Uses: standard input";
    let line = "- Splits: its work into up to 8 pieces, each of up to 1048576 bytes, each gives \
                back 8192 bytes";
    assert_eq!(violations(CODE, &[uses, line]), Vec::<String>::new());
    let ungranted = violations(CODE, &[uses]);
    for function in ["`split`", "`piece`"] {
        assert!(
            ungranted
                .iter()
                .any(|found| found.contains(function) && found.contains("- Splits:")),
            "{function}: {ungranted:?}"
        );
    }
    // The line grants 7 pieces, and the call splits into 8.
    let fewer = line.replace("up to 8 pieces", "up to 7 pieces");
    let outside = violations(CODE, &[uses, &fewer]);
    assert!(
        outside
            .iter()
            .any(|found| found.contains("splits its work into 8 pieces")),
        "{outside:?}"
    );
    // A number of pieces that isn't a named constant can't be held to the line.
    let bare = CODE.replace(
        "(local.get $len) (global.get $PIECES)",
        "(local.get $len) (i32.const 8)",
    );
    assert_ne!(bare, CODE);
    let unnamed = violations(&bare, &[uses, line]);
    assert!(
        unnamed
            .iter()
            .any(|found| found.contains("isn't a named constant")),
        "{unnamed:?}"
    );
    // 64 pieces and the program, each with 130 pages, hold 8450 pages: inside 16384. With 400
    // pages each they would hold 26000.
    let many = line.replace("up to 8 pieces", "up to 64 pieces");
    assert_eq!(violations(CODE, &[uses, &many]), Vec::<String>::new());
    let large = CODE.replace(
        "(memory (export \"memory\") 130 130)",
        "(memory (export \"memory\") 400 400)",
    );
    assert_ne!(large, CODE);
    let too_much = violations(&large, &[uses, &many]);
    assert!(
        too_much.iter().any(|found| found.contains("26000 pages")),
        "{too_much:?}"
    );
    // A module that splits and never asks whether it is a piece would split in each piece.
    let never_asks = CODE
        .replace(
            "(import \"lengua_split_1\" \"piece\" (func $piece (result i32)))",
            "",
        )
        .replace("(call $piece)", "(global.get $NOT_A_PIECE)");
    let unasked = violations(&never_asks, &[uses, line]);
    assert!(
        unasked
            .iter()
            .any(|found| found.contains("never asks whether it is a piece")),
        "{unasked:?}"
    );
}

/// The idea's reader holds the line to its bounds and to the rest of the idea: a program that
/// splits its work reads standard input and names no secret, and only an idea with the line
/// has a test of a piece.
#[test]
fn the_idea_holds_the_line_to_the_rest_of_the_idea() {
    let idea = lengua::idea::parse(IDEA).unwrap_or_else(|problems| panic!("{problems:?}"));
    let grant = idea.network.reach.split.expect("the line's grant");
    assert_eq!((grant.most, grant.bytes, grant.back), (8, 1_048_576, 8192));
    let suite = lengua::testfile::check(&idea).unwrap_or_else(|problems| panic!("{problems:?}"));
    assert!(
        suite.examples.iter().any(|example| example.piece),
        "the idea has a test of a piece"
    );
    assert!(suite.generated.iter().any(|block| block.piece));
    let problems = |text: &str| -> Vec<String> {
        match lengua::idea::parse(text) {
            Err(problems) => problems.iter().map(ToString::to_string).collect(),
            Ok(idea) => match lengua::testfile::check(&idea) {
                Err(problems) => problems.iter().map(ToString::to_string).collect(),
                Ok(_) => Vec::new(),
            },
        }
    };
    let line = "- Splits: its work into up to 8 pieces, each of up to 1048576 bytes, each gives \
                back 8192 bytes";
    assert!(IDEA.contains(line));
    let secret = IDEA.replace(line, &format!("{line}\n- Secret: standard input"));
    assert!(
        problems(&secret)
            .iter()
            .any(|problem| problem.contains("names no secret")),
        "{:?}",
        problems(&secret)
    );
    let twice = IDEA.replace(line, &format!("{line}\n{line}"));
    assert!(
        problems(&twice)
            .iter()
            .any(|problem| problem.contains("given twice")),
        "{:?}",
        problems(&twice)
    );
    let unbounded = IDEA.replace("up to 8 pieces", "up to 65 pieces");
    assert!(
        problems(&unbounded)
            .iter()
            .any(|problem| problem.contains("2 to 64 pieces")),
        "{:?}",
        problems(&unbounded)
    );
    // Without the line, the test of a piece has nothing to stand on.
    let unsplit = IDEA.replace(&format!("{line}\n"), "");
    assert_ne!(unsplit, IDEA);
    assert!(
        problems(&unsplit)
            .iter()
            .any(|problem| problem.contains("grants no split")),
        "{:?}",
        problems(&unsplit)
    );
}

/// A piece starts from the module's first state whatever ran before it on its thread: each
/// piece is a new store of the engine's, with a new memory and new globals, though the threads
/// are kept through the run. Four pieces that each leave a byte and a global changed, run one
/// after the other on one thread, and four at once, each find both as the module declares
/// them; and so do the pieces of a second run in the same worker.
#[tokio::test(flavor = "multi_thread")]
async fn no_piece_sees_what_another_left() {
    let fresh = answers(&["0", "0", "0", "0"]);
    for reversed in [true, false, true] {
        let ran = run_as(
            Shape::USUAL,
            "RaaaRbbbRcccRddd",
            policy(&LINES),
            |spec| Spec { reversed, ..spec },
            Limits::default(),
        )
        .await;
        assert_eq!(ran.end, End::Exited(0), "reversed: {reversed}");
        assert_eq!(ran.stdout, fresh, "reversed: {reversed}");
    }
}

/// The MCP server's `run` tool runs a program that splits its work, once `check` says done:
/// the example's counts of a short input, and the grant in words.
#[tokio::test]
async fn the_run_tool_runs_a_program_that_splits_its_work() {
    let session = common::Session::start(common::Protocol::Handshake).await;
    let program = common::Program::new();
    program.write("idea.md", IDEA);
    session.record(&program).await;
    program.write("main.wat", CODE);
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    let mut arguments = program.arguments();
    arguments["stdin"] = serde_json::json!("\n aa a\n");
    let ran = session.call_ok("run", arguments).await;
    assert_eq!(ran["stdout"], "10 2\n32 2\n97 3\n", "{ran}");
    assert!(
        ran.to_string()
            .contains("it may split its work into up to 8 pieces"),
        "{ran}"
    );
    // Enough for each of the eight pieces to have work.
    let long = "ab\n".repeat(100_000);
    let mut arguments = program.arguments();
    arguments["stdin"] = serde_json::json!(long);
    let ran = session.call_ok("run", arguments).await;
    assert_eq!(ran["stdout"], "10 100000\n97 100000\n98 100000\n", "{ran}");
    session.service.cancel().await.expect("close the session");
}
