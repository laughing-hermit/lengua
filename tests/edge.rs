//! The edge's operations, one table of cases for both CPUs. Each case is a small program in the
//! edge form (it defines `main` and calls `os_*`), built with lengua.s after it, run through the
//! launcher under the system calls its operations need, and held to what it prints and how it
//! ends. A table cell for the CPU this computer is runs natively, on that CPU's own runner in CI;
//! the other CPU's cell runs through QEMU's user mode where that is set up, and every cell is at
//! least built, since LLVM builds for both CPUs anywhere.
//!
//! What an operation does is the same on every CPU and every operating system, so a case's
//! output and exit are written once; only its program differs, by CPU.

mod common;
mod per_target;

use std::path::Path;

use lengua::platform::Profile;
use lengua::runner::{End, Job, Limits, run};
use lengua::spec::Syscall;
use lengua::targets::PerCpu;
use per_target::Way;

fn launcher() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_lengua-mcp"))
}

/// Assembly lines, indented as instructions are unless they are labels or section names.
fn lines(list: &[&str]) -> String {
    let mut text = String::new();
    for line in list {
        let unindented = line.ends_with(':') || (line.starts_with('.') && !line.contains(' '));
        if !unindented {
            text.push_str("    ");
        }
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// An x86-64 program in the edge form: `main` with `frame` bytes of stack to itself, which is
/// 8 more than a multiple of 16 so that its calls are made with `rsp` aligned, then `data`.
fn x86(frame: u32, body: &[&str], data: &[&str]) -> String {
    format!(
        ".intel_syntax noprefix\n.text\n.type main, @function\nmain:\n    sub rsp, {frame}\n{}\
         .size main, . - main\n.section .rodata\n{}",
        lines(body),
        lines(data)
    )
}

/// An AArch64 program in the edge form: `main` with `frame` bytes of stack to itself, a multiple
/// of 16, then `data`.
fn a64(frame: u32, body: &[&str], data: &[&str]) -> String {
    format!(
        ".arch armv8-a\n.text\n.type main, @function\nmain:\n    sub sp, sp, #{frame}\n{}\
         .size main, . - main\n.section .rodata\n{}",
        lines(body),
        lines(data)
    )
}

/// One case: a program for each CPU, what it prints and the code it exits with, and the system
/// calls its operations make.
struct Case {
    name: &'static str,
    program: PerCpu<String>,
    /// The system calls the program's operations make, besides `write` and `exit_group`, which
    /// every program may.
    calls: &'static [Syscall],
    stdin: &'static [u8],
    stdout: Vec<u8>,
    exit: i32,
}

/// The eight bytes of a result, little-endian, as a program stores one to print it.
fn word(value: i64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn cases() -> Vec<Case> {
    vec![
        // The first operations: write, and exit with the status given.
        Case {
            name: "write_and_exit",
            program: PerCpu {
                x86_64: x86(
                    8,
                    &[
                        "mov edi, 1",
                        "lea rsi, [rip + greeting]",
                        "mov edx, 6",
                        "call os_write",
                        "mov edi, 7",
                        "call os_exit",
                    ],
                    &["greeting:", ".ascii \"hello\\n\""],
                ),
                aarch64: a64(
                    16,
                    &[
                        "mov x0, #1",
                        "adrp x1, greeting",
                        "add x1, x1, :lo12:greeting",
                        "mov x2, #6",
                        "bl os_write",
                        "mov x0, #7",
                        "bl os_exit",
                    ],
                    &["greeting:", ".ascii \"hello\\n\""],
                ),
            },
            calls: &[],
            stdin: b"",
            stdout: b"hello\n".to_vec(),
            exit: 7,
        },
        // An error is the negative of its number, and descriptor 4,352 is lengua's own, which
        // every operation refuses as a descriptor that is not open.
        Case {
            name: "write_errors",
            program: PerCpu {
                x86_64: x86(
                    24,
                    &[
                        "mov edi, 99",
                        "lea rsi, [rip + greeting]",
                        "mov edx, 6",
                        "call os_write",
                        "mov qword ptr [rsp], rax",
                        "mov edi, 4352",
                        "lea rsi, [rip + greeting]",
                        "mov edx, 6",
                        "call os_write",
                        "mov qword ptr [rsp + 8], rax",
                        "mov edi, 1",
                        "mov rsi, rsp",
                        "mov edx, 16",
                        "call os_write",
                        "xor edi, edi",
                        "call os_exit",
                    ],
                    &["greeting:", ".ascii \"hello\\n\""],
                ),
                aarch64: a64(
                    32,
                    &[
                        "mov w0, #99",
                        "adrp x1, greeting",
                        "add x1, x1, :lo12:greeting",
                        "mov x2, #6",
                        "bl os_write",
                        "str x0, [sp]",
                        "mov w0, #4352",
                        "adrp x1, greeting",
                        "add x1, x1, :lo12:greeting",
                        "mov x2, #6",
                        "bl os_write",
                        "str x0, [sp, #8]",
                        "mov x0, #1",
                        "mov x1, sp",
                        "mov x2, #16",
                        "bl os_write",
                        "mov w0, #0",
                        "bl os_exit",
                    ],
                    &["greeting:", ".ascii \"hello\\n\""],
                ),
            },
            calls: &[],
            stdin: b"",
            stdout: [word(-9), word(-9)].concat(),
            exit: 0,
        },
    ]
}

/// Every case, on every CPU this computer runs: what it prints and how it ends.
#[tokio::test]
async fn every_operation_does_what_it_promises() {
    let folder = tempfile::tempdir().expect("a folder");
    for case in cases() {
        // Every cell is built, so that a mistake in any fails on every runner.
        let mut built = Vec::new();
        for target in Profile::ALL {
            let place = folder.path().join(target.name());
            let executable =
                per_target::build(target, &place, case.name, case.program.get(target)).await;
            built.push((target, executable));
        }
        for (target, executable) in built {
            let Some(how) = per_target::way(target) else {
                continue;
            };
            let mut spec = vec![Syscall::Write, Syscall::Exit];
            spec.extend_from_slice(case.calls);
            let allowed = target.allowed_syscalls(&spec, false);
            let job = Job {
                emulated: how == Way::Emulated,
                ..Job::new(&executable, &[], case.stdin, &allowed)
            };
            let ran = run(launcher(), job, Limits::default())
                .await
                .expect("the program runs");
            let what = format!("{} on {}", case.name, target.name());
            assert_eq!(ran.end, End::Exited(case.exit), "{what}");
            assert_eq!(ran.stdout, case.stdout, "{what}");
        }
    }
}
