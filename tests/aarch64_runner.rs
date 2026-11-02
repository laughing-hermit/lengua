//! Running AArch64 programs on an ARM64 computer: the kernel holds each to its spec's system
//! calls and its folder, as on x86-64, and the supervisor answers a test's random bytes, clock
//! and sleeps there too. AArch64 can't skip a call the supervisor answers, so the kernel makes
//! it harmlessly and the answer lands at the call's exit, every other register as the program
//! left it.

// These programs run only on an ARM64 computer; an x86-64 one runs AArch64 programs through
// QEMU, whose own system calls the kernel and the supervisor see (tests/runner.rs covers that).
#![cfg(target_arch = "aarch64")]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use lengua::connect::Reach;
use lengua::platform::{Allowed, Profile};
use lengua::runner::{End, Job, Limits, Net, Run, run, start};
use lengua::spec::Syscall;
use lengua::supply::Supply;
use lengua::toolchain::Toolchain;

fn launcher() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_lengua-mcp"))
}

/// Builds AArch64 assembly into `folder` and returns the executable.
async fn build(folder: &Path, name: &str, source: &str) -> PathBuf {
    let toolchain = Toolchain::discover().await.expect("llvm-mc and ld.lld");
    let assembly = folder.join(format!("{name}.s"));
    let object = folder.join(format!("{name}.o"));
    let executable = folder.join(name);
    std::fs::write(&assembly, source).expect("write the source");
    toolchain
        .assemble(Profile::LinuxAarch64, &assembly, &object)
        .await
        .expect("assemble");
    toolchain
        .link(Profile::LinuxAarch64, &object, &executable)
        .await
        .expect("link");
    executable
}

fn syscalls(calls: &[Syscall]) -> Vec<Allowed> {
    Profile::LinuxAarch64.allowed_syscalls(calls, false)
}

async fn run_supplied(executable: &Path, supply: &Supply, calls: &[Syscall]) -> Run {
    let syscalls = syscalls(calls);
    let job = Job {
        supply: Some(supply),
        ..Job::new(executable, &[], b"", &syscalls)
    };
    run(launcher(), job, Limits::default()).await.expect("run")
}

fn time(seconds: i64, nanoseconds: i64) -> Vec<u8> {
    [seconds.to_le_bytes(), nanoseconds.to_le_bytes()].concat()
}

/// Draws 4 random bytes and reads the clock twice, printing each, then exits with 4.
const DRAWS_AND_READS_THE_CLOCK: &str = "\
.arch armv8-a
.text
.globl _start
_start:
    adrp x19, buffer
    add x19, x19, :lo12:buffer
    mov x0, x19
    mov x1, #4
    mov x2, #0
    mov x8, #278
    svc #0
    mov x0, #1
    mov x1, x19
    mov x2, #4
    mov x8, #64
    svc #0
    mov x0, #0
    add x1, x19, #8
    mov x8, #113
    svc #0
    mov x0, #1
    add x1, x19, #8
    mov x2, #16
    mov x8, #64
    svc #0
    mov x0, #0
    add x1, x19, #8
    mov x8, #113
    svc #0
    mov x0, #1
    add x1, x19, #8
    mov x2, #16
    mov x8, #64
    svc #0
    mov x0, #4
    mov x8, #93
    svc #0
.bss
.balign 16
buffer:
    .zero 32
";

const DRAW_AND_CLOCK: [Syscall; 4] = [
    Syscall::Getrandom,
    Syscall::ClockGettime,
    Syscall::Write,
    Syscall::Exit,
];

/// AArch64: reads up to 64 bytes from its connection, prints them, answers `ok` on it, and
/// exits.
const AARCH64_ANSWERS_OK: &str = "\
.arch armv8-a
.text
.globl _start
_start:
    adrp x19, buffer
    add x19, x19, :lo12:buffer
    mov x0, #4
    mov x1, x19
    mov x2, #64
    mov x8, #63
    svc #0
    mov x2, x0
    mov x0, #1
    mov x1, x19
    mov x8, #64
    svc #0
    adrp x1, ok
    add x1, x1, :lo12:ok
    mov x0, #4
    mov x2, #2
    mov x8, #64
    svc #0
    mov x0, #0
    mov x8, #93
    svc #0
.data
ok:
    .ascii \"ok\"
.bss
buffer:
    .zero 64
";

/// AArch64: reads its connection, then writes on it, and exits with the read's result less
/// the write's: 0 and -EPIPE give 32. A SIGPIPE would kill it instead.
const AARCH64_ENDED: &str = "\
.arch armv8-a
.text
.globl _start
_start:
    adrp x19, buffer
    add x19, x19, :lo12:buffer
    mov x0, #4
    mov x1, x19
    mov x2, #64
    mov x8, #63
    svc #0
    mov x20, x0
    mov x0, #4
    mov x1, x19
    mov x2, #2
    mov x8, #64
    svc #0
    sub x0, x20, x0
    mov x8, #93
    svc #0
.bss
buffer:
    .zero 64
";

/// A program's connections, as lengua hands them over: descriptors 4 and on.
fn connected(connections: Vec<Reach>) -> Net {
    Net {
        connections,
        listen: None,
        serve_anywhere: false,
    }
}

#[tokio::test]
async fn connections_are_handed_over_from_descriptor_4() {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let folder = tempfile::tempdir().expect("a folder");
    let client = build(folder.path(), "client", AARCH64_ANSWERS_OK).await;
    let calls = syscalls(&[Syscall::Read, Syscall::Write, Syscall::Exit]);
    let net = connected(vec![Reach::Peer]);
    let job = Job {
        net: Some(&net),
        ..Job::new(&client, &[], b"", &calls)
    };
    let mut started = start(launcher(), job, Limits::default())
        .await
        .expect("start");
    let (_, mut stream) = started.take_peers().remove(0);
    stream.write_all(b"hello").await.expect("send");
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).await.expect("the answer");
    assert_eq!(answer, b"ok");
    let answered = started.wait().await.expect("wait");
    assert_eq!(answered.end, End::Exited(0), "{answered:?}");
    assert_eq!(answered.stdout, b"hello");
    // Ended both ways: a read gives 0, and a write fails with EPIPE rather than a signal.
    let ended = build(folder.path(), "ended", AARCH64_ENDED).await;
    let nowhere = connected(vec![Reach::Nowhere]);
    let job = Job {
        net: Some(&nowhere),
        ..Job::new(&ended, &[], b"", &calls)
    };
    let ended = run(launcher(), job, Limits::default()).await.expect("run");
    assert_eq!(ended.end, End::Exited(32), "{ended:?}");
}

/// AArch64: makes `CON.txt`, which Windows reserves, and exits with the error it got; 99 if x1,
/// which the supervisor makes null to refuse the call, didn't come back as it was.
const AARCH64_MAKES_A_DEVICE_NAME: &str = "\
.arch armv8-a
.text
.globl _start
_start:
    adrp x19, name
    add x19, x19, :lo12:name
    mov x0, #-100
    mov x1, x19
    mov x2, #0x41
    mov x3, #420
    mov x8, #56
    svc #0
    cmp x1, x19
    b.ne 2f
    tbz x0, #63, 1f
    neg x0, x0
    mov x8, #93
    svc #0
1:
    mov x0, #0
    mov x8, #93
    svc #0
2:
    mov x0, #99
    mov x8, #93
    svc #0
.data
name:
    .asciz \"CON.txt\"
";

/// AArch64: opens `f`, removes it while it's open, and exits with the error that got, or 0.
const AARCH64_REMOVES_AN_OPEN_FILE: &str = "\
.arch armv8-a
.text
.globl _start
_start:
    mov x0, #-100
    adrp x1, name
    add x1, x1, :lo12:name
    mov x2, #0
    mov x3, #0
    mov x8, #56
    svc #0
    mov x0, #-100
    adrp x1, name
    add x1, x1, :lo12:name
    mov x2, #0
    mov x8, #35
    svc #0
    tbz x0, #63, 1f
    neg x0, x0
    mov x8, #93
    svc #0
1:
    mov x0, #0
    mov x8, #93
    svc #0
.data
name:
    .asciz \"f\"
";

#[tokio::test]
async fn names_are_held_to_every_system_on_arm64_too() {
    let folder = tempfile::tempdir().expect("a folder");
    let calls = syscalls(&[
        Syscall::Openat,
        Syscall::Close,
        Syscall::Unlinkat,
        Syscall::Write,
        Syscall::Exit,
    ]);
    let device = build(folder.path(), "device", AARCH64_MAKES_A_DEVICE_NAME).await;
    let job = Job::new(&device, &[], b"", &calls);
    let refused = run(launcher(), job, Limits::default()).await.expect("run");
    assert_eq!(refused.end, End::Exited(22), "EINVAL, x1 kept: {refused:?}");
    let remover = build(folder.path(), "remover", AARCH64_REMOVES_AN_OPEN_FILE).await;
    let files = std::collections::BTreeMap::from([("f".to_owned(), "x".to_owned())]);
    let job = Job {
        files: &files,
        ..Job::new(&remover, &[], b"", &calls)
    };
    let busy = run(launcher(), job, Limits::default()).await.expect("run");
    assert_eq!(busy.end, End::Exited(16), "EBUSY: {busy:?}");
}

#[tokio::test]
async fn tests_supply_the_random_bytes_and_the_clock() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let program = build(folder.path(), "draw", DRAWS_AND_READS_THE_CLOCK).await;
    let supply =
        Supply::parse(Some("01020304ff"), &["5".into(), "7.000000001".into()]).expect("a supply");
    let result = run_supplied(&program, &supply, &DRAW_AND_CLOCK).await;
    assert_eq!(result.supplied, None, "{result:?}");
    assert_eq!(result.end, End::Exited(4), "{result:?}");
    let expected = [vec![1, 2, 3, 4], time(5, 0), time(7, 1)].concat();
    assert_eq!(result.stdout, expected, "{result:?}");

    // Bytes past the end of the supply are zeros, and the run says how many were asked for.
    let short = Supply::parse(Some("0102"), &["5".into()]).expect("a supply");
    let result = run_supplied(&program, &short, &DRAW_AND_CLOCK).await;
    assert_eq!(result.random_asked, Some(4), "{result:?}");
    assert_eq!(result.stdout[..4], [1, 2, 0, 0], "{result:?}");
    assert_eq!(result.stdout[4..], [time(5, 0), time(5, 0)].concat());

    // Only the random bytes supplied: the kernel's clock answers.
    let random_only = Supply::parse(Some("01020304"), &[]).expect("a supply");
    let result = run_supplied(&program, &random_only, &DRAW_AND_CLOCK).await;
    let seconds = i64::from_le_bytes(result.stdout[4..12].try_into().expect("8 bytes"));
    assert!(seconds > 1_700_000_000, "the real time, {seconds}");
}

/// A supplied call keeps every register but x0 as the program left it: x1, which the harmless
/// call changed, is put back, and the others the kernel never touched.
#[tokio::test]
async fn a_supplied_call_leaves_the_other_registers_alone() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let mut source = String::from(
        ".arch armv8-a\n.text\n.globl _start\n_start:\n    adrp x0, buffer\n    add x0, x0, \
         :lo12:buffer\n    mov x1, #4\n    mov x2, #0\n    mov x3, #33\n    mov x4, #44\n    mov \
         x5, #55\n    mov x6, #66\n    mov x7, #77\n    mov x8, #278\n    svc #0\n",
    );
    // Each register checked in turn: the exit code says which one wasn't as expected.
    for (code, (register, value)) in [
        ("x0", 4),
        ("x1", 4),
        ("x2", 0),
        ("x3", 33),
        ("x4", 44),
        ("x5", 55),
        ("x6", 66),
        ("x7", 77),
    ]
    .into_iter()
    .enumerate()
    {
        source.push_str(&format!(
            "    mov x9, #{}\n    cmp {register}, #{value}\n    b.ne .Lfail\n",
            code + 1
        ));
    }
    source.push_str(
        "    mov x9, #0\n.Lfail:\n    mov x0, x9\n    mov x8, #93\n    svc #0\n.bss\n.balign \
         16\nbuffer:\n    .zero 16\n",
    );
    let program = build(folder.path(), "keeps", &source).await;
    let supply = Supply::parse(Some("01020304"), &[]).expect("a supply");
    let result = run_supplied(&program, &supply, &DRAW_AND_CLOCK).await;
    assert_eq!(
        result.end,
        End::Exited(0),
        "register {:?}: {result:?}",
        result.end
    );
}

/// A test's sleep ends at once, and says it slept.
#[tokio::test]
async fn a_tests_sleep_ends_at_once() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let program = build(
        folder.path(),
        "sleeps",
        ".arch armv8-a\n.text\n.globl _start\n_start:\n    adrp x2, request\n    add x2, x2, \
         :lo12:request\n    mov x0, #1\n    mov x1, #0\n    mov x3, #0\n    mov x8, #115\n    svc \
         #0\n    mov x8, #93\n    svc #0\n.data\n.balign 8\nrequest:\n    .quad 10\n    .quad 0\n",
    )
    .await;
    let began = Instant::now();
    let result = run_supplied(
        &program,
        &Supply::default(),
        &[Syscall::ClockNanosleep, Syscall::Exit],
    )
    .await;
    assert_eq!(result.end, End::Exited(0), "{result:?}");
    assert!(
        began.elapsed() < Duration::from_secs(5),
        "{:?}",
        began.elapsed()
    );
}

/// A call outside the spec's stops the program, and a file outside its folder is out of reach.
#[tokio::test]
async fn the_kernel_keeps_programs_inside_what_their_spec_allows() {
    let scratch = tempfile::tempdir().expect("a folder");
    let names_the_system = build(
        scratch.path(),
        "uname",
        ".arch armv8-a\n.text\n.globl _start\n_start:\n    adrp x0, buffer\n    add x0, x0, \
         :lo12:buffer\n    mov x8, #160\n    svc #0\n    mov x0, #0\n    mov x8, #93\n    svc #0\n\
         .bss\nbuffer:\n    .zero 512\n",
    )
    .await;
    let only_write_and_exit = syscalls(&[Syscall::Write, Syscall::Exit]);
    let result = run(
        launcher(),
        Job::new(&names_the_system, &[], b"", &only_write_and_exit),
        Limits::default(),
    )
    .await
    .expect("run");
    assert_eq!(result.end, End::Signaled(31), "SIGSYS: {result:?}");

    let opens = build(
        scratch.path(),
        "opens",
        ".arch armv8-a\n.text\n.globl _start\n_start:\n    ldr x1, [sp, #16]\n    mov x0, \
         #-100\n    mov x2, #0\n    mov x3, #0\n    mov x8, #56\n    svc #0\n    lsr x0, x0, \
         #63\n    mov x8, #93\n    svc #0\n",
    )
    .await;
    let folder = scratch.path().join("inside");
    std::fs::create_dir(&folder).expect("its folder");
    std::fs::write(folder.join("here"), "inside").expect("a file inside");
    std::fs::write(scratch.path().join("x"), "outside").expect("a file outside");
    let file_syscalls = syscalls(&[Syscall::Openat, Syscall::Exit]);
    for (name, code) in [("here", 0), ("../x", 1)] {
        let arguments = vec![name.to_owned()];
        let job = Job {
            folder: Some(&folder),
            ..Job::new(&opens, &arguments, b"", &file_syscalls)
        };
        let result = run(launcher(), job, Limits::default()).await.expect("run");
        assert_eq!(result.end, End::Exited(code), "{name}: {result:?}");
    }
}

/// The CPU's counters count the program's own instructions where the computer lets them; a
/// virtual machine may not, and then nothing is counted, as the reply says.
#[tokio::test]
async fn counts_the_programs_instructions_where_it_can() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let program = build(folder.path(), "draw", DRAWS_AND_READS_THE_CLOCK).await;
    let supply = Supply::parse(Some("01020304"), &["5".into()]).expect("a supply");
    let result = run_supplied(&program, &supply, &DRAW_AND_CLOCK).await;
    eprintln!(
        "counters on this computer: instructions {:?}, cycles {:?}",
        result.instructions, result.cycles
    );
    if let Some(count) = result.instructions {
        assert!((20..200).contains(&count), "{count} instructions");
    }
}
