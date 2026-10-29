//! Running programs: limits, a clean environment, stdin, signals, instruction counts, files, and
//! the kernel's confinement.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use lengua::platform::{Allowed, Profile};
use lengua::runner::{End, Job, Limits, Run, run};
use lengua::spec::Syscall;
use lengua::supply::Supply;
use lengua::toolchain::Toolchain;

fn launcher() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_lengua-mcp"))
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| (*arg).to_owned()).collect()
}

/// The system calls of a spec that reads, writes and opens files.
fn file_syscalls() -> Vec<Allowed> {
    Profile::LinuxX86_64.allowed_syscalls(
        &[
            Syscall::Read,
            Syscall::Write,
            Syscall::Openat,
            Syscall::Close,
            Syscall::Exit,
        ],
        false,
    )
}

/// Assembles and links `source` into `folder` and returns the executable.
async fn build(folder: &Path, name: &str, source: &str) -> PathBuf {
    let toolchain = Toolchain::discover().await.expect("llvm-mc and ld.lld");
    let assembly = folder.join(format!("{name}.s"));
    let object = folder.join(format!("{name}.o"));
    let executable = folder.join(name);
    std::fs::write(&assembly, source).expect("write the source");
    toolchain
        .assemble(Profile::LinuxX86_64, &assembly, &object)
        .await
        .expect("assemble");
    toolchain.link(&object, &executable).await.expect("link");
    executable
}

/// Builds the assembly template into `folder` and returns the executable.
async fn template(folder: &Path) -> PathBuf {
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("plugin/skills/write-assembly/assets/main-template.s"),
    )
    .expect("the template");
    build(folder, "main", &source).await
}

/// A small static program: `body` after `_start`, then `data`.
fn program(body: &str, data: &str) -> String {
    format!(".intel_syntax noprefix\n.globl _start\n.text\n_start:\n{body}\n{data}\n")
}

/// Runs `executable` in a fresh folder with `files`, allowed `syscalls`, and `limits`.
async fn run_with(
    executable: &Path,
    arguments: &[String],
    stdin: &[u8],
    files: &BTreeMap<String, String>,
    syscalls: &[Allowed],
    limits: Limits,
) -> Run {
    let job = Job {
        files,
        ..Job::new(executable, arguments, stdin, syscalls)
    };
    run(launcher(), job, limits).await.expect("run")
}

async fn run_simply(executable: &Path, arguments: &[String], stdin: &[u8]) -> Run {
    run_with(
        executable,
        arguments,
        stdin,
        &BTreeMap::new(),
        &file_syscalls(),
        Limits::default(),
    )
    .await
}

#[tokio::test]
async fn runs_a_program_and_counts_its_instructions() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let program = template(folder.path()).await;
    let first = run_simply(&program, &args(&["7"]), b"").await;
    assert_eq!(first.stdout, b"-7\n");
    assert_eq!(first.stderr, b"");
    assert_eq!(first.end, End::Exited(0));
    let second = run_simply(&program, &args(&["7"]), b"").await;
    match (first.instructions, second.instructions) {
        (Some(first), Some(second)) => {
            // The CPU counts the odd interrupt or page fault as an instruction.
            assert!(
                first.abs_diff(second) <= 2,
                "the same input executes the same instructions: {first} vs {second}"
            );
            assert!((50..5_000).contains(&first), "{first} instructions");
        }
        // Machines that don't allow performance counters still run programs.
        (None, None) => eprintln!("performance counters aren't available here"),
        other => panic!("counted only one run: {other:?}"),
    }
    let longer = run_simply(&program, &args(&["123456789"]), b"").await;
    if let (Some(short), Some(long)) = (first.instructions, longer.instructions) {
        assert!(
            long > short,
            "more digits take more instructions: {short} vs {long}"
        );
    }
}

#[tokio::test]
async fn reports_exit_codes_and_failed_assertions() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let program = template(folder.path()).await;
    let usage = run_simply(&program, &[], b"").await;
    assert_eq!(usage.end, End::Exited(1));
    assert_eq!(usage.stderr, b"usage: negate <n>\n");
}

#[tokio::test]
async fn kills_programs_that_run_too_long() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let forever = build(folder.path(), "forever", &program("    jmp _start", "")).await;
    let limits = Limits {
        timeout: Duration::from_millis(300),
        ..Limits::default()
    };
    let started = Instant::now();
    let result = run_with(
        &forever,
        &[],
        b"",
        &BTreeMap::new(),
        &file_syscalls(),
        limits,
    )
    .await;
    assert_eq!(result.end, End::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn kills_programs_that_print_too_much() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let chatter = build(
        folder.path(),
        "chatter",
        &program(
            "    mov eax, 1\n    mov edi, 1\n    lea rsi, [rip + line]\n    mov edx, 2\n    \
             syscall\n    jmp _start",
            ".data\nline:\n    .ascii \"y\\n\"",
        ),
    )
    .await;
    let limits = Limits {
        max_output: 4096,
        ..Limits::default()
    };
    let result = run_with(
        &chatter,
        &[],
        b"",
        &BTreeMap::new(),
        &file_syscalls(),
        limits,
    )
    .await;
    assert_eq!(result.end, End::OutputTooLarge);
    assert_eq!(result.stdout.len(), 4096);
}

#[tokio::test]
async fn gives_an_empty_environment_and_closed_stdin() {
    let folder = tempfile::tempdir().expect("temporary folder");
    // Exits with 0 when the environment is empty: envp[0] is null right after argv's null.
    let environment = build(
        folder.path(),
        "environment",
        &program(
            "    mov rax, qword ptr [rsp]\n    lea rbx, [rsp + 8*rax + 16]\n    xor edi, edi\n    \
             cmp qword ptr [rbx], 0\n    setne dil\n    mov eax, 60\n    syscall",
            "",
        ),
    )
    .await;
    let result = run_simply(&environment, &args(&["a", "b"]), b"").await;
    assert_eq!(
        result.end,
        End::Exited(0),
        "no variables leak into programs"
    );
    // Copies standard input to standard output until it ends.
    let copy = build(
        folder.path(),
        "copy",
        &program(
            "    xor eax, eax\n    xor edi, edi\n    lea rsi, [rip + buffer]\n    mov edx, 4096\n    \
             syscall\n    test rax, rax\n    jle done\n    mov rdx, rax\n    mov eax, 1\n    \
             mov edi, 1\n    lea rsi, [rip + buffer]\n    syscall\n    jmp _start\ndone:\n    \
             mov eax, 60\n    xor edi, edi\n    syscall",
            ".bss\nbuffer:\n    .zero 4096",
        ),
    )
    .await;
    let echo = run_simply(&copy, &[], b"hello\n").await;
    assert_eq!(echo.stdout, b"hello\n", "stdin is given, then ends");
    assert_eq!(echo.end, End::Exited(0));
}

#[tokio::test]
async fn reports_signals() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let crash = build(
        folder.path(),
        "crash",
        &program("    xor eax, eax\n    mov byte ptr [rax], 1", ""),
    )
    .await;
    let result = run_simply(&crash, &[], b"").await;
    assert_eq!(result.end, End::Signaled(11));
    assert!(result.end.describe(Limits::default()).contains("SIGSEGV"));
}

/// Opens `in.txt`, reads it and writes what it read to `out.txt`.
const COPY_FILE: &str = "    mov eax, 257\n    mov edi, -100\n    lea rsi, [rip + input]\n    \
     xor edx, edx\n    syscall\n    mov edi, eax\n    xor eax, eax\n    lea rsi, [rip + buffer]\n    \
     mov edx, 4096\n    syscall\n    mov r12, rax\n    mov eax, 257\n    mov edi, -100\n    \
     lea rsi, [rip + output]\n    mov edx, 0x241\n    mov r10d, 0x1a4\n    syscall\n    \
     mov edi, eax\n    mov eax, 1\n    lea rsi, [rip + buffer]\n    mov rdx, r12\n    syscall\n    \
     mov eax, 60\n    xor edi, edi\n    syscall";

#[tokio::test]
async fn gives_files_and_reads_back_what_the_program_left() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let copy = build(
        folder.path(),
        "copy-file",
        &program(
            COPY_FILE,
            ".data\ninput:\n    .asciz \"in.txt\"\noutput:\n    .asciz \"out.txt\"\n.bss\n\
             buffer:\n    .zero 4096",
        ),
    )
    .await;
    let files = BTreeMap::from([("in.txt".to_owned(), "hello\n".to_owned())]);
    let result = run_with(&copy, &[], b"", &files, &file_syscalls(), Limits::default()).await;
    assert_eq!(result.end, End::Exited(0));
    let left = result.files.expect("readable files");
    assert_eq!(
        left,
        BTreeMap::from([
            ("in.txt".to_owned(), b"hello\n".to_vec()),
            ("out.txt".to_owned(), b"hello\n".to_vec()),
        ])
    );

    // In a folder of the user's, the program works on the files there, and nothing is read back.
    let user = tempfile::tempdir().expect("user folder");
    std::fs::write(user.path().join("in.txt"), "mine\n").expect("a user file");
    let syscalls = file_syscalls();
    let empty = BTreeMap::new();
    let job = Job {
        folder: Some(user.path()),
        files: &empty,
        ..Job::new(&copy, &[], b"", &syscalls)
    };
    let result = run(launcher(), job, Limits::default()).await.expect("run");
    assert_eq!(result.end, End::Exited(0));
    assert_eq!(
        std::fs::read_to_string(user.path().join("out.txt")).expect("written"),
        "mine\n"
    );
    assert_eq!(result.files, Ok(BTreeMap::new()));
}

#[tokio::test]
async fn the_kernel_keeps_programs_inside_what_their_spec_allows() {
    let folder = tempfile::tempdir().expect("temporary folder");
    // A system call the spec doesn't allow (getpid) stops the program.
    let getpid = build(
        folder.path(),
        "getpid",
        &program(
            "    mov eax, 39\n    syscall\n    mov eax, 60\n    xor edi, edi\n    syscall",
            "",
        ),
    )
    .await;
    let result = run_simply(&getpid, &[], b"").await;
    assert_eq!(result.end, End::Signaled(31));
    assert!(
        result
            .end
            .describe(Limits::default())
            .contains("a system call its spec doesn't allow")
    );

    // A file outside the working folder can't be opened: the exit code is the error, EACCES.
    let outside = build(
        folder.path(),
        "outside",
        &program(
            "    mov eax, 257\n    mov edi, -100\n    lea rsi, [rip + path]\n    xor edx, edx\n    \
             syscall\n    neg rax\n    mov edi, eax\n    mov eax, 60\n    syscall",
            ".data\npath:\n    .asciz \"/etc/hostname\"",
        ),
    )
    .await;
    let result = run_simply(&outside, &[], b"").await;
    assert_eq!(result.end, End::Exited(13), "EACCES");

    // A file can't grow past the limit.
    let grow = build(
        folder.path(),
        "grow",
        &program(
            "    mov eax, 257\n    mov edi, -100\n    lea rsi, [rip + output]\n    mov edx, 0x241\n    \
             mov r10d, 0x1a4\n    syscall\n    mov ebx, eax\nagain:\n    mov eax, 1\n    \
             mov edi, ebx\n    lea rsi, [rip + buffer]\n    mov edx, 4096\n    syscall\n    \
             jmp again",
            ".data\noutput:\n    .asciz \"out.txt\"\n.bss\nbuffer:\n    .zero 4096",
        ),
    )
    .await;
    let limits = Limits {
        max_file_size: Some(10_000),
        ..Limits::default()
    };
    let result = run_with(&grow, &[], b"", &BTreeMap::new(), &file_syscalls(), limits).await;
    assert_eq!(result.end, End::Signaled(25));
    assert!(result.end.describe(limits).contains("size limit"));
}

#[tokio::test]
async fn threads_start_only_with_the_canonical_flags() {
    let folder = tempfile::tempdir().expect("temporary folder");
    // A fork, clone with SIGCHLD alone: a new process rather than a thread of this one.
    let fork = build(
        folder.path(),
        "fork",
        &program(
            "    mov edi, 17\n    xor esi, esi\n    mov eax, 56\n    syscall\n    mov eax, 60\n    \
             xor edi, edi\n    syscall",
            "",
        ),
    )
    .await;
    let threaded = Profile::LinuxX86_64.allowed_syscalls(&[Syscall::Write, Syscall::Exit], true);
    let result = run_with(
        &fork,
        &[],
        b"",
        &BTreeMap::new(),
        &threaded,
        Limits::default(),
    )
    .await;
    assert_eq!(
        result.end,
        End::Signaled(31),
        "only parallel_for's flags pass"
    );
}

#[tokio::test]
async fn the_clock_and_random_bytes_need_the_specs_approval() {
    let folder = tempfile::tempdir().expect("temporary folder");
    // 16 random bytes, then the time of day: exits with how many bytes it got, plus the clock's
    // result (0).
    let reader = build(
        folder.path(),
        "reader",
        &program(
            "    mov eax, 318\n    lea rdi, [rip + buffer]\n    mov esi, 16\n    xor edx, edx\n    \
             syscall\n    mov ebx, eax\n    mov eax, 228\n    xor edi, edi\n    \
             lea rsi, [rip + time]\n    syscall\n    add ebx, eax\n    mov edi, ebx\n    \
             mov eax, 60\n    syscall",
            ".bss\nbuffer:\n    .zero 16\ntime:\n    .zero 16",
        ),
    )
    .await;
    let approved = Profile::LinuxX86_64.allowed_syscalls(
        &[Syscall::Getrandom, Syscall::ClockGettime, Syscall::Exit],
        false,
    );
    let result = run_with(
        &reader,
        &[],
        b"",
        &BTreeMap::new(),
        &approved,
        Limits::default(),
    )
    .await;
    assert_eq!(result.end, End::Exited(16), "16 bytes, and the clock read");
    let result = run_simply(&reader, &[], b"").await;
    assert_eq!(result.end, End::Signaled(31), "not approved: stopped");
}

/// Reads 4 random bytes and the clock twice, writes all of it to stdout, and exits with what
/// getrandom returned.
fn draws_and_reads_the_clock(buffer: &str) -> String {
    program(
        &format!(
            "    mov eax, 318\n    lea rdi, {buffer}\n    mov esi, 4\n    xor edx, edx\n    \
             syscall\n    mov ebx, eax\n    \
             mov eax, 228\n    xor edi, edi\n    lea rsi, [rip + first]\n    syscall\n    \
             mov eax, 228\n    mov edi, 1\n    lea rsi, [rip + second]\n    syscall\n    \
             mov eax, 1\n    mov edi, 1\n    lea rsi, [rip + buffer]\n    mov edx, 36\n    \
             syscall\n    mov edi, ebx\n    mov eax, 60\n    syscall"
        ),
        ".bss\nbuffer:\n    .zero 4\nfirst:\n    .zero 16\nsecond:\n    .zero 16",
    )
}

fn clock_and_random() -> Vec<Allowed> {
    Profile::LinuxX86_64.allowed_syscalls(
        &[
            Syscall::Getrandom,
            Syscall::ClockGettime,
            Syscall::Write,
            Syscall::Exit,
        ],
        false,
    )
}

async fn run_supplied(executable: &Path, supply: &Supply, limits: Limits) -> Run {
    let syscalls = clock_and_random();
    let job = Job {
        supply: Some(supply),
        ..Job::new(executable, &[], b"", &syscalls)
    };
    run(launcher(), job, limits).await.expect("run")
}

fn time(seconds: i64, nanoseconds: i64) -> Vec<u8> {
    [seconds.to_le_bytes(), nanoseconds.to_le_bytes()].concat()
}

#[tokio::test]
async fn tests_supply_the_random_bytes_and_the_clock() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let program = build(
        folder.path(),
        "draw",
        &draws_and_reads_the_clock("[rip + buffer]"),
    )
    .await;
    let supply =
        Supply::parse(Some("01020304ff"), &["5".into(), "7.000000001".into()]).expect("a supply");
    let result = run_supplied(&program, &supply, Limits::default()).await;
    assert_eq!(result.supplied, None);
    assert_eq!(result.end, End::Exited(4), "{result:?}");
    let expected = [vec![1, 2, 3, 4], time(5, 0), time(7, 1)].concat();
    assert_eq!(result.stdout, expected);
    assert!(
        result.instructions.is_some_and(|count| count < 40),
        "{result:?}"
    );

    // The last reading repeats.
    let one_reading = Supply::parse(Some("01020304"), &["5".into()]).expect("a supply");
    let result = run_supplied(&program, &one_reading, Limits::default()).await;
    assert_eq!(result.stdout[4..], [time(5, 0), time(5, 0)].concat());

    // Bytes past the end of the supply are zeros, and the run says how many were asked for.
    let short = Supply::parse(Some("0102"), &["5".into()]).expect("a supply");
    let result = run_supplied(&program, &short, Limits::default()).await;
    assert_eq!(result.supplied, None);
    assert_eq!(result.random_asked, Some(4));
    assert_eq!(result.stdout[..4], [1, 2, 0, 0]);
    assert_eq!(result.end, End::Exited(4));

    // Only the random bytes supplied: the kernel's clock answers.
    let random_only = Supply::parse(Some("01020304"), &[]).expect("a supply");
    let result = run_supplied(&program, &random_only, Limits::default()).await;
    let seconds = i64::from_le_bytes(result.stdout[4..12].try_into().expect("8 bytes"));
    assert!(seconds > 1_700_000_000, "the real time, {seconds}");
}

#[tokio::test]
async fn supplied_calls_behave_like_the_kernels() {
    let folder = tempfile::tempdir().expect("temporary folder");
    // A buffer the program can't write: getrandom fails with EFAULT, as the kernel's would.
    let unwritable = build(
        folder.path(),
        "unwritable",
        &draws_and_reads_the_clock("[rip + _start]"),
    )
    .await;
    let supply = Supply::parse(Some("01020304"), &["5".into()]).expect("a supply");
    let result = run_supplied(&unwritable, &supply, Limits::default()).await;
    assert_eq!(result.end, End::Exited(256 - 14), "-EFAULT: {result:?}");
    assert_eq!(result.supplied, None);

    // A crash after a supplied call ends the program as it would alone.
    let crash = build(
        folder.path(),
        "crash",
        &program(
            "    mov eax, 318\n    lea rdi, [rip + buffer]\n    mov esi, 1\n    xor edx, edx\n    \
             syscall\n    mov qword ptr [0], rax",
            ".bss\nbuffer:\n    .zero 1",
        ),
    )
    .await;
    let result = run_supplied(&crash, &supply, Limits::default()).await;
    assert_eq!(result.end, End::Signaled(11), "{result:?}");
    assert_eq!(result.supplied, None);

    // A program still running at the time limit is killed, supervisor and all.
    let spin = build(
        folder.path(),
        "spin",
        &program(
            "    mov eax, 318\n    lea rdi, [rip + buffer]\n    mov esi, 1\n    xor edx, edx\n    \
             syscall\nagain:\n    jmp again",
            ".bss\nbuffer:\n    .zero 1",
        ),
    )
    .await;
    let limits = Limits {
        timeout: Duration::from_millis(300),
        ..Limits::default()
    };
    let started = Instant::now();
    let result = run_supplied(&spin, &supply, limits).await;
    assert_eq!(result.end, End::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn the_supervisor_follows_threaded_programs() {
    // The parallel template starts workers with clone; the supervisor follows each one and the
    // program prints just what it prints alone.
    let folder = tempfile::tempdir().expect("temporary folder");
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("plugin/skills/write-assembly/assets/parallel-template.s"),
    )
    .expect("the parallel template");
    let program = build(folder.path(), "shout", &source).await;
    let syscalls = Profile::LinuxX86_64.allowed_syscalls(
        &[
            Syscall::Read,
            Syscall::Write,
            Syscall::Exit,
            Syscall::Getrandom,
        ],
        true,
    );
    let text = "hello, threads! ".repeat(40_000);
    let alone = run(
        launcher(),
        Job::new(&program, &[], text.as_bytes(), &syscalls),
        Limits::default(),
    )
    .await
    .expect("run");
    let supply = Supply::parse(Some("00"), &[]).expect("a supply");
    let supervised = run(
        launcher(),
        Job {
            supply: Some(&supply),
            ..Job::new(&program, &[], text.as_bytes(), &syscalls)
        },
        Limits::default(),
    )
    .await
    .expect("run");
    assert_eq!(alone.end, End::Exited(0), "{alone:?}");
    assert_eq!(supervised.end, End::Exited(0), "{supervised:?}");
    assert_eq!(supervised.stdout, text.to_uppercase().into_bytes());
    assert_eq!(supervised.supplied, None);
    assert_eq!(supervised.random_asked, Some(0));
}
