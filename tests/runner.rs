//! Running programs: limits, a clean environment, stdin, signals, instruction counts, files, and
//! the kernel's confinement.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use lengua::platform::{Allowed, Profile};
use lengua::runner::{End, Job, Limits, Net, Run, run, start};
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
            .join("plugin/skills/compile/assets/main-template.s"),
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
            .join("plugin/skills/compile/assets/parallel-template.s"),
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

/// Connects to 127.0.0.1:`port` and exits with the negated result of `connect`: 0 when it got
/// through, 13 (EACCES) when the kernel refused the port.
fn connects_to(port: u16) -> String {
    program(
        "    mov eax, 41\n    mov edi, 2\n    mov esi, 1\n    xor edx, edx\n    syscall\n    \
         mov r12, rax\n    mov eax, 42\n    mov rdi, r12\n    lea rsi, [rip + address]\n    \
         mov edx, 16\n    syscall\n    neg eax\n    mov edi, eax\n    mov eax, 60\n    syscall",
        &format!(
            ".data\naddress:\n    .word 2\n    .byte {}, {}\n    .byte 127, 0, 0, 1\n    .zero 8",
            port >> 8,
            port & 0xff
        ),
    )
}

fn network_syscalls(calls: &[Syscall]) -> Vec<Allowed> {
    Profile::LinuxX86_64.allowed_syscalls(calls, false)
}

async fn run_on(executable: &Path, syscalls: &[Allowed], net: &Net) -> Run {
    let job = Job {
        net: Some(net),
        ..Job::new(executable, &[], b"", syscalls)
    };
    run(launcher(), job, Limits::default()).await.expect("run")
}

#[tokio::test]
async fn programs_connect_only_to_approved_ports() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("an address").port();
    let client = build(folder.path(), "client", &connects_to(port)).await;
    let calls = network_syscalls(&[Syscall::Socket, Syscall::Connect, Syscall::Exit]);
    let approved = Net {
        connect: vec![port],
        anywhere: false,
        listen: None,
    };
    assert_eq!(run_on(&client, &calls, &approved).await.end, End::Exited(0));
    let other = Net {
        connect: vec![port.wrapping_add(1).max(1)],
        anywhere: false,
        listen: None,
    };
    assert_eq!(
        run_on(&client, &calls, &other).await.end,
        End::Exited(13),
        "EACCES: the kernel refuses a port the spec doesn't name"
    );
    // A UDP socket isn't TCP: stopped.
    let udp = build(
        folder.path(),
        "udp",
        &program(
            "    mov eax, 41\n    mov edi, 2\n    mov esi, 2\n    xor edx, edx\n    syscall\n    \
             mov eax, 60\n    xor edi, edi\n    syscall",
            "",
        ),
    )
    .await;
    assert_eq!(run_on(&udp, &calls, &approved).await.end, End::Signaled(31));
    // Another computer on an approved port: lengua refuses it, unless the spec says any
    // computer, when the kernel lets the connection try.
    let elsewhere = build(
        folder.path(),
        "elsewhere",
        &connects_to_host(port, [192, 0, 2, 1]),
    )
    .await;
    assert_eq!(
        run_on(&elsewhere, &calls, &approved).await.end,
        End::Exited(13),
        "EACCES: lengua refuses a computer other than this one"
    );
    let anywhere = Net {
        anywhere: true,
        ..approved.clone()
    };
    let tried = run_on(&elsewhere, &calls, &anywhere).await;
    assert_ne!(tried.end, End::Exited(13), "{tried:?}");
}

/// A program that connects to `host` on `port`, without waiting long, and exits with the
/// error number.
fn connects_to_host(port: u16, host: [u8; 4]) -> String {
    program(
        "    mov eax, 41\n    mov edi, 2\n    mov esi, 0x801\n    xor edx, edx\n    syscall\n    \
         mov r12, rax\n    mov eax, 42\n    mov rdi, r12\n    lea rsi, [rip + address]\n    \
         mov edx, 16\n    syscall\n    neg eax\n    mov edi, eax\n    mov eax, 60\n    syscall",
        &format!(
            ".data\naddress:\n    .word 2\n    .byte {}, {}\n    .byte {}, {}, {}, {}\n    .zero 8",
            port >> 8,
            port & 0xff,
            host[0],
            host[1],
            host[2],
            host[3]
        ),
    )
}

#[tokio::test]
async fn servers_take_connections_on_descriptor_3() {
    use tokio::io::AsyncReadExt as _;
    let folder = tempfile::tempdir().expect("temporary folder");
    // Takes one connection, says hi on it and exits.
    let server = build(
        folder.path(),
        "server",
        &program(
            "    mov eax, 288\n    mov edi, 3\n    xor esi, esi\n    xor edx, edx\n    \
             xor r10d, r10d\n    syscall\n    mov rdi, rax\n    mov eax, 1\n    \
             lea rsi, [rip + hi]\n    mov edx, 2\n    syscall\n    mov eax, 60\n    \
             xor edi, edi\n    syscall",
            ".data\nhi:\n    .ascii \"hi\"",
        ),
    )
    .await;
    let calls = network_syscalls(&[Syscall::Accept4, Syscall::Write, Syscall::Exit]);
    let net = Net {
        connect: Vec::new(),
        anywhere: false,
        listen: Some(0),
    };
    let job = Job {
        net: Some(&net),
        ..Job::new(&server, &[], b"", &calls)
    };
    let started = start(launcher(), job, Limits::default())
        .await
        .expect("start");
    let port = started.port().expect("a port");
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connect");
    let mut said = Vec::new();
    stream.read_to_end(&mut said).await.expect("read");
    assert_eq!(said, b"hi");
    assert_eq!(started.wait().await.expect("wait").end, End::Exited(0));

    // A server that keeps serving is stopped by lengua.
    let forever = build(
        folder.path(),
        "forever",
        &program(
            "again:\n    mov eax, 288\n    mov edi, 3\n    xor esi, esi\n    xor edx, edx\n    \
             xor r10d, r10d\n    syscall\n    jmp again",
            "",
        ),
    )
    .await;
    let job = Job {
        net: Some(&net),
        ..Job::new(&forever, &[], b"", &calls)
    };
    let started = start(launcher(), job, Limits::default())
        .await
        .expect("start");
    assert!(started.port().is_some());
    assert_eq!(started.stop().await.expect("stop").end, End::Stopped);
}

/// Runs the template's tests, with `edit` applied to its spec, `workers` at a time.
async fn template_tests(edit: impl Fn(&str) -> String, workers: usize) -> lengua::testrun::Outcome {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugin/skills/compile/assets");
    let read = |name: &str| std::fs::read_to_string(assets.join(name)).expect("an asset");
    let spec = lengua::spec::parse(&edit(&read("template-spec.md"))).expect("the template's spec");
    let suite = lengua::testfile::check(&spec).expect("the tests check");
    let (source, _) = lengua::checker::source::parse(&read("main-template.s"));
    let folder = tempfile::tempdir().expect("a folder");
    let executable = template(folder.path()).await;
    let syscalls = Profile::LinuxX86_64.allowed_syscalls(&spec.syscalls, false);
    lengua::testrun::run_tests(
        &suite,
        launcher(),
        &executable,
        &source,
        7,
        &syscalls,
        &spec.network,
        workers,
    )
    .await
    .expect("the tests run")
}

#[tokio::test]
async fn runs_at_once_report_what_runs_one_at_a_time_do() {
    // Passing tests: the same runs, and each test timed alone on its largest input.
    let alone = template_tests(str::to_owned, 1).await;
    let together = template_tests(str::to_owned, 8).await;
    assert!(
        alone.passed() && together.passed(),
        "{alone:?} {together:?}"
    );
    assert_eq!(alone.runs, 3 + 200 + 50);
    assert_eq!(together.runs, alone.runs);
    let shape = |outcome: &lengua::testrun::Outcome| -> Vec<(String, usize, String)> {
        outcome
            .costs
            .iter()
            .map(|cost| (cost.test.clone(), cost.runs, cost.timed.clone()))
            .collect()
    };
    assert_eq!(shape(&together), shape(&alone));
    assert!(
        together.costs[3].timed.starts_with("its largest input, "),
        "{:?}",
        together.costs[3]
    );
    assert!(together.costs[3].timed.ends_with("run alone 5 times"));

    // A wrong formula for large numbers and a wrong example: the same failures, in the same
    // order, naming the block's first failing run.
    let wrong = |text: &str| {
        text.replace(
            "`usage: negate <n>` followed by a newline",
            "`usage: negate <n>\\n` followed by a newline",
        )
        .replace(
            "prints `{-n}` followed by a newline",
            "prints `{if(n > 1000, n, -n)}` followed by a newline",
        )
    };
    let edited = wrong(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("plugin/skills/compile/assets/template-spec.md"),
        )
        .expect("the template's spec"),
    );
    assert!(edited.contains("{if(n > 1000, n, -n)}") && edited.contains("<n>\\n`"));
    let alone = template_tests(wrong, 1).await;
    let together = template_tests(wrong, 8).await;
    assert_eq!(alone.failures.len(), 2, "{:?}", alone.failures);
    assert_eq!(together.failures, alone.failures);
    assert!(
        alone.failures[0].starts_with("example B2"),
        "{:?}",
        alone.failures
    );
    assert!(
        alone.failures[1].contains("(seed 7)"),
        "{:?}",
        alone.failures
    );
    assert!(together.costs.is_empty(), "{:?}", together.costs);
}

/// How much sooner `examples/count`'s 924 runs finish with every core: run with
/// `cargo test --release --test runner -- --ignored --nocapture speedup`.
#[tokio::test]
#[ignore = "a measurement, not a check"]
async fn speedup_of_runs_at_once_on_the_word_count() {
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/count");
    let read = |name: &str| std::fs::read_to_string(example.join(name)).expect("the example");
    let spec = lengua::spec::parse(&read("spec.md")).expect("the spec");
    let suite = lengua::testfile::check(&spec).expect("the tests");
    let (source, _) = lengua::checker::source::parse(&read("main.s"));
    let folder = tempfile::tempdir().expect("a folder");
    let executable = build(folder.path(), "count", &read("main.s")).await;
    let syscalls = Profile::LinuxX86_64.allowed_syscalls(&spec.syscalls, false);
    for workers in [1, lengua::testrun::workers(None)] {
        let began = Instant::now();
        let outcome = lengua::testrun::run_tests(
            &suite,
            launcher(),
            &executable,
            &source,
            7,
            &syscalls,
            &spec.network,
            workers,
        )
        .await
        .expect("the tests run");
        assert!(outcome.passed(), "{:?}", outcome.failures);
        println!(
            "{workers} at once: {} runs in {:.2} s",
            outcome.runs,
            began.elapsed().as_secs_f64()
        );
    }
}
