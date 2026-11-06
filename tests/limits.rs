//! The limits lengua holds every program to, tested on every target: what a program may do, how
//! it behaves, what it's refused and when, and what lengua promises of it. The user's rule is
//! that every limit one target has is every target's, whatever its assembly or its OS's
//! permissions, so each is pinned here in the same words on x86-64 and on ARM64: the time and
//! output limits, the empty environment and the standard input that ends, a descriptor a parent
//! left open, signals, files and the size they may grow to, the names every system takes
//! (`EINVAL`, `EILSEQ`, `ENAMETOOLONG`, `EEXIST`), `EBUSY` for a file held open, the 4,352 open
//! files, folder listings turned round on a test's second run, the spec's approval of the clock
//! and the random bytes, the test's own bytes and readings, the broker's failure paths and
//! threads, the pin on `clone`'s flags, and a network the program never holds.
//!
//! Each program is a table with a source for each target (`PerCpu`), in that CPU's assembly;
//! a target added to lengua breaks every table until its sources are written. Every target's
//! source is built on every computer (LLVM builds for all of them), so a typo in a cell fails
//! on every runner; the program for the target this computer is runs natively, on that target's
//! own runner in CI. QEMU's user mode can't hold a program to its spec's system calls, so a
//! program of the other CPU runs under it only to check what it does hold: its connections,
//! Landlock and the stop of the calls QEMU never makes.

mod common;
mod per_target;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use lengua::broker::Permit;
use lengua::connect::Reach;
use lengua::platform::{Allowed, Pin};
use lengua::runner::{End, Job, Limits, Net, Run, run, start};
use lengua::spec::{Syscall, Use};
use lengua::syscalls::{aarch64, x86_64};
use lengua::targets::PerCpu;

fn launcher() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_lengua-mcp"))
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| (*arg).to_owned()).collect()
}

/// The text of a file in the compile skill's assets.
fn asset(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugin/skills/compile/assets")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
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

/// A small x86-64 program: `body` after `_start`, then `data`.
fn x86(body: &[&str], data: &[&str]) -> String {
    format!(
        ".intel_syntax noprefix\n.globl _start\n.text\n_start:\n{}{}",
        lines(body),
        lines(data)
    )
}

/// A small AArch64 program: `body` after `_start`, then `data`.
fn a64(body: &[&str], data: &[&str]) -> String {
    format!(
        ".arch armv8-a\n.globl _start\n.text\n_start:\n{}{}",
        lines(body),
        lines(data)
    )
}

/// A program's source for each target.
type Programs = PerCpu<String>;

/// Builds a program for every target, so that a mistake in any cell fails on every runner, and
/// returns the executable for the target this computer is.
async fn built(folder: &Path, name: &str, programs: &Programs) -> PathBuf {
    let host = per_target::host();
    let mut own = None;
    for (target, source) in programs.cells() {
        let place = folder.join(target.name());
        let executable = per_target::build(target, &place, name, source).await;
        if target == host {
            own = Some(executable);
        }
    }
    own.expect("a program for this computer's target")
}

/// The compile skill's assets that are a program for each target.
type Assets = PerCpu<&'static str>;

/// Builds the asset of each target, as [`built`] does.
async fn built_asset(folder: &Path, name: &str, assets: &Assets) -> PathBuf {
    let programs = PerCpu {
        x86_64: asset(assets.x86_64),
        aarch64: asset(assets.aarch64),
    };
    built(folder, name, &programs).await
}

/// The template program.
const TEMPLATES: Assets = PerCpu {
    x86_64: "main-template.s",
    aarch64: "linux-aarch64/main-template.s",
};

/// The system calls of a spec that allows `spec` and no thread.
fn calls(spec: &[Syscall]) -> Vec<Allowed> {
    per_target::host().allowed_syscalls(spec, false)
}

/// The system calls of a spec that reads, writes and opens files.
fn file_syscalls() -> Vec<Allowed> {
    calls(&[
        Syscall::Read,
        Syscall::Write,
        Syscall::Openat,
        Syscall::Close,
        Syscall::Exit,
    ])
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

// The programs: each a source for every target.

/// A program that never ends.
fn forever() -> Programs {
    PerCpu {
        x86_64: x86(&["jmp _start"], &[]),
        aarch64: a64(&["b _start"], &[]),
    }
}

/// A program that prints `y` and a newline for ever.
fn chatter() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov eax, 1",
                "mov edi, 1",
                "lea rsi, [rip + line]",
                "mov edx, 2",
                "syscall",
                "jmp _start",
            ],
            &[".data", "line:", ".ascii \"y\\n\""],
        ),
        aarch64: a64(
            &[
                "mov x0, #1",
                "adrp x1, line",
                "add x1, x1, :lo12:line",
                "mov x2, #2",
                "mov x8, #64",
                "svc #0",
                "b _start",
            ],
            &[".data", "line:", ".ascii \"y\\n\""],
        ),
    }
}

/// A program that exits with 0 when its environment is empty: the first variable is null right
/// after the arguments' null.
fn checks_its_environment() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov rax, qword ptr [rsp]",
                "lea rbx, [rsp + 8*rax + 16]",
                "xor edi, edi",
                "cmp qword ptr [rbx], 0",
                "setne dil",
                "mov eax, 60",
                "syscall",
            ],
            &[],
        ),
        aarch64: a64(
            &[
                "ldr x0, [sp]",
                "add x1, sp, x0, lsl #3",
                "ldr x2, [x1, #16]",
                "cmp x2, #0",
                "cset w0, ne",
                "mov x8, #93",
                "svc #0",
            ],
            &[],
        ),
    }
}

/// A program that copies standard input to standard output until it ends.
fn copies_its_input() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "xor eax, eax",
                "xor edi, edi",
                "lea rsi, [rip + buffer]",
                "mov edx, 4096",
                "syscall",
                "test rax, rax",
                "jle done",
                "mov rdx, rax",
                "mov eax, 1",
                "mov edi, 1",
                "lea rsi, [rip + buffer]",
                "syscall",
                "jmp _start",
                "done:",
                "mov eax, 60",
                "xor edi, edi",
                "syscall",
            ],
            &[".bss", "buffer:", ".zero 4096"],
        ),
        aarch64: a64(
            &[
                "mov x0, #0",
                "adrp x1, buffer",
                "add x1, x1, :lo12:buffer",
                "mov x2, #4096",
                "mov x8, #63",
                "svc #0",
                "cmp x0, #0",
                "b.le 1f",
                "mov x2, x0",
                "mov x0, #1",
                "adrp x1, buffer",
                "add x1, x1, :lo12:buffer",
                "mov x8, #64",
                "svc #0",
                "b _start",
                "1:",
                "mov x0, #0",
                "mov x8, #93",
                "svc #0",
            ],
            &[".bss", "buffer:", ".zero 4096"],
        ),
    }
}

/// A program that writes a byte to `descriptor` and exits with the error that got, negated.
#[cfg(unix)]
fn writes_to(descriptor: i32) -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov eax, 1",
                &format!("mov edi, {descriptor}"),
                "lea rsi, [rip + byte]",
                "mov edx, 1",
                "syscall",
                "neg eax",
                "mov edi, eax",
                "mov eax, 60",
                "syscall",
            ],
            &[".data", "byte:", ".ascii \"x\""],
        ),
        aarch64: a64(
            &[
                &format!("mov x0, #{descriptor}"),
                "adrp x1, byte",
                "add x1, x1, :lo12:byte",
                "mov x2, #1",
                "mov x8, #64",
                "svc #0",
                "neg x0, x0",
                "mov x8, #93",
                "svc #0",
            ],
            &[".data", "byte:", ".ascii \"x\""],
        ),
    }
}

/// A program that writes through a null pointer.
fn crashes() -> Programs {
    PerCpu {
        x86_64: x86(&["xor eax, eax", "mov byte ptr [rax], 1"], &[]),
        aarch64: a64(&["mov x0, #0", "mov w1, #1", "strb w1, [x0]"], &[]),
    }
}

/// A program that opens `in.txt`, reads it and writes what it read to `out.txt`.
fn copies_a_file() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov eax, 257",
                "mov edi, -100",
                "lea rsi, [rip + input]",
                "xor edx, edx",
                "syscall",
                "mov edi, eax",
                "xor eax, eax",
                "lea rsi, [rip + buffer]",
                "mov edx, 4096",
                "syscall",
                "mov r12, rax",
                "mov eax, 257",
                "mov edi, -100",
                "lea rsi, [rip + output]",
                "mov edx, 0x241",
                "mov r10d, 0x1a4",
                "syscall",
                "mov edi, eax",
                "mov eax, 1",
                "lea rsi, [rip + buffer]",
                "mov rdx, r12",
                "syscall",
                "mov eax, 60",
                "xor edi, edi",
                "syscall",
            ],
            &[
                ".data",
                "input:",
                ".asciz \"in.txt\"",
                "output:",
                ".asciz \"out.txt\"",
                ".bss",
                "buffer:",
                ".zero 4096",
            ],
        ),
        aarch64: a64(
            &[
                "mov x0, #-100",
                "adrp x1, input",
                "add x1, x1, :lo12:input",
                "mov x2, #0",
                "mov x8, #56",
                "svc #0",
                "adrp x1, buffer",
                "add x1, x1, :lo12:buffer",
                "mov x2, #4096",
                "mov x8, #63",
                "svc #0",
                "mov x20, x0",
                "mov x0, #-100",
                "adrp x1, output",
                "add x1, x1, :lo12:output",
                "mov x2, #0x241",
                "mov x3, #0x1a4",
                "mov x8, #56",
                "svc #0",
                "adrp x1, buffer",
                "add x1, x1, :lo12:buffer",
                "mov x2, x20",
                "mov x8, #64",
                "svc #0",
                "mov x0, #0",
                "mov x8, #93",
                "svc #0",
            ],
            &[
                ".data",
                "input:",
                ".asciz \"in.txt\"",
                "output:",
                ".asciz \"out.txt\"",
                ".bss",
                "buffer:",
                ".zero 4096",
            ],
        ),
    }
}

/// A program that makes a system call no spec here allows (`getpid`) and exits with 0.
fn makes_a_call_its_spec_doesnt_allow() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov eax, 39",
                "syscall",
                "mov eax, 60",
                "xor edi, edi",
                "syscall",
            ],
            &[],
        ),
        aarch64: a64(
            &[
                "mov x8, #172",
                "svc #0",
                "mov x0, #0",
                "mov x8, #93",
                "svc #0",
            ],
            &[],
        ),
    }
}

/// A program that opens a file outside its folder and exits with the error it got, negated.
fn opens_a_file_outside_its_folder() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov eax, 257",
                "mov edi, -100",
                "lea rsi, [rip + path]",
                "xor edx, edx",
                "syscall",
                "neg rax",
                "mov edi, eax",
                "mov eax, 60",
                "syscall",
            ],
            &[".data", "path:", ".asciz \"/etc/hostname\""],
        ),
        aarch64: a64(
            &[
                "mov x0, #-100",
                "adrp x1, path",
                "add x1, x1, :lo12:path",
                "mov x2, #0",
                "mov x3, #0",
                "mov x8, #56",
                "svc #0",
                "neg x0, x0",
                "mov x8, #93",
                "svc #0",
            ],
            &[".data", "path:", ".asciz \"/etc/hostname\""],
        ),
    }
}

/// A program that writes to `out.txt` until a write fails, and exits with the error.
fn grows_a_file() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov eax, 257",
                "mov edi, -100",
                "lea rsi, [rip + output]",
                "mov edx, 0x241",
                "mov r10d, 0x1a4",
                "syscall",
                "mov ebx, eax",
                "again:",
                "mov eax, 1",
                "mov edi, ebx",
                "lea rsi, [rip + buffer]",
                "mov edx, 4096",
                "syscall",
                "test rax, rax",
                "jns again",
                "neg eax",
                "mov edi, eax",
                "mov eax, 60",
                "syscall",
            ],
            &[
                ".data",
                "output:",
                ".asciz \"out.txt\"",
                ".bss",
                "buffer:",
                ".zero 4096",
            ],
        ),
        aarch64: a64(
            &[
                "mov x0, #-100",
                "adrp x1, output",
                "add x1, x1, :lo12:output",
                "mov x2, #0x241",
                "mov x3, #0x1a4",
                "mov x8, #56",
                "svc #0",
                "mov x19, x0",
                "1:",
                "mov x0, x19",
                "adrp x1, buffer",
                "add x1, x1, :lo12:buffer",
                "mov x2, #4096",
                "mov x8, #64",
                "svc #0",
                "tbz x0, #63, 1b",
                "neg x0, x0",
                "mov x8, #93",
                "svc #0",
            ],
            &[
                ".data",
                "output:",
                ".asciz \"out.txt\"",
                ".bss",
                "buffer:",
                ".zero 4096",
            ],
        ),
    }
}

/// A program that starts a process with `clone` and `SIGCHLD` alone, not a thread of its own.
fn forks() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov edi, 17",
                "xor esi, esi",
                "mov eax, 56",
                "syscall",
                "mov eax, 60",
                "xor edi, edi",
                "syscall",
            ],
            &[],
        ),
        aarch64: a64(
            &[
                "mov x0, #17",
                "mov x1, #0",
                "mov x8, #220",
                "svc #0",
                "mov x0, #0",
                "mov x8, #93",
                "svc #0",
            ],
            &[],
        ),
    }
}

/// A program that asks for 16 random bytes, then the time of day, and exits with how many bytes
/// it got plus the clock's result (0).
fn asks_for_random_bytes_and_the_clock() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov eax, 318",
                "lea rdi, [rip + buffer]",
                "mov esi, 16",
                "xor edx, edx",
                "syscall",
                "mov ebx, eax",
                "mov eax, 228",
                "xor edi, edi",
                "lea rsi, [rip + time]",
                "syscall",
                "add ebx, eax",
                "mov edi, ebx",
                "mov eax, 60",
                "syscall",
            ],
            &[".bss", "buffer:", ".zero 16", "time:", ".zero 16"],
        ),
        aarch64: a64(
            &[
                "adrp x19, buffer",
                "add x19, x19, :lo12:buffer",
                "mov x0, x19",
                "mov x1, #16",
                "mov x2, #0",
                "mov x8, #278",
                "svc #0",
                "mov x20, x0",
                "mov x0, #0",
                "add x1, x19, #16",
                "mov x8, #113",
                "svc #0",
                "add x0, x20, x0",
                "mov x8, #93",
                "svc #0",
            ],
            &[".bss", "buffer:", ".zero 16", "time:", ".zero 16"],
        ),
    }
}

/// A program that connects to 127.0.0.1:`port` itself and exits with the negated result of
/// `connect`: 0 when it got through, 13 (`EACCES`) when the kernel refused the port.
fn connects_to(port: u16) -> Programs {
    let (high, low) = (port >> 8, port & 0xff);
    PerCpu {
        x86_64: x86(
            &[
                "mov eax, 41",
                "mov edi, 2",
                "mov esi, 1",
                "xor edx, edx",
                "syscall",
                "mov r12, rax",
                "mov eax, 42",
                "mov rdi, r12",
                "lea rsi, [rip + address]",
                "mov edx, 16",
                "syscall",
                "neg eax",
                "mov edi, eax",
                "mov eax, 60",
                "syscall",
            ],
            &[
                ".data",
                "address:",
                ".word 2",
                &format!(".byte {high}, {low}"),
                ".byte 127, 0, 0, 1",
                ".zero 8",
            ],
        ),
        aarch64: a64(
            &[
                "mov x0, #2",
                "mov x1, #1",
                "mov x2, #0",
                "mov x8, #198",
                "svc #0",
                "adrp x1, address",
                "add x1, x1, :lo12:address",
                "mov x2, #16",
                "mov x8, #203",
                "svc #0",
                "neg x0, x0",
                "mov x8, #93",
                "svc #0",
            ],
            &[
                ".data",
                "address:",
                ".short 2",
                &format!(".byte {high}, {low}"),
                ".byte 127, 0, 0, 1",
                ".zero 8",
            ],
        ),
    }
}

/// A server that takes one connection on the listening socket (descriptor 3), says `hi` on it
/// and exits.
fn serves_one_client() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov eax, 288",
                "mov edi, 3",
                "xor esi, esi",
                "xor edx, edx",
                "xor r10d, r10d",
                "syscall",
                "mov rdi, rax",
                "mov eax, 1",
                "lea rsi, [rip + hi]",
                "mov edx, 2",
                "syscall",
                "mov eax, 60",
                "xor edi, edi",
                "syscall",
            ],
            &[".data", "hi:", ".ascii \"hi\""],
        ),
        aarch64: a64(
            &[
                "mov x0, #3",
                "mov x1, #0",
                "mov x2, #0",
                "mov x3, #0",
                "mov x8, #242",
                "svc #0",
                "adrp x1, hi",
                "add x1, x1, :lo12:hi",
                "mov x2, #2",
                "mov x8, #64",
                "svc #0",
                "mov x0, #0",
                "mov x8, #93",
                "svc #0",
            ],
            &[".data", "hi:", ".ascii \"hi\""],
        ),
    }
}

/// A server that takes connections for ever.
fn serves_for_ever() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "again:",
                "mov eax, 288",
                "mov edi, 3",
                "xor esi, esi",
                "xor edx, edx",
                "xor r10d, r10d",
                "syscall",
                "jmp again",
            ],
            &[],
        ),
        aarch64: a64(
            &[
                "1:",
                "mov x0, #3",
                "mov x1, #0",
                "mov x2, #0",
                "mov x3, #0",
                "mov x8, #242",
                "svc #0",
                "b 1b",
            ],
            &[],
        ),
    }
}

/// A client that reads up to 64 bytes from its connection (descriptor 4), prints them, answers
/// `ok` on it and exits.
fn answers_ok() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "xor eax, eax",
                "mov edi, 4",
                "lea rsi, [rip + buffer]",
                "mov edx, 64",
                "syscall",
                "mov rdx, rax",
                "mov eax, 1",
                "mov edi, 1",
                "lea rsi, [rip + buffer]",
                "syscall",
                "mov eax, 1",
                "mov edi, 4",
                "lea rsi, [rip + ok]",
                "mov edx, 2",
                "syscall",
                "mov eax, 60",
                "xor edi, edi",
                "syscall",
            ],
            &[
                ".data",
                "ok:",
                ".ascii \"ok\"",
                ".bss",
                "buffer:",
                ".zero 64",
            ],
        ),
        aarch64: a64(
            &[
                "adrp x19, buffer",
                "add x19, x19, :lo12:buffer",
                "mov x0, #4",
                "mov x1, x19",
                "mov x2, #64",
                "mov x8, #63",
                "svc #0",
                "mov x2, x0",
                "mov x0, #1",
                "mov x1, x19",
                "mov x8, #64",
                "svc #0",
                "adrp x1, ok",
                "add x1, x1, :lo12:ok",
                "mov x0, #4",
                "mov x2, #2",
                "mov x8, #64",
                "svc #0",
                "mov x0, #0",
                "mov x8, #93",
                "svc #0",
            ],
            &[
                ".data",
                "ok:",
                ".ascii \"ok\"",
                ".bss",
                "buffer:",
                ".zero 64",
            ],
        ),
    }
}

/// A client that reads its connection, then writes on it, and exits with the read's result less
/// the write's: 0 and -`EPIPE` give 32. A `SIGPIPE` would kill it instead.
fn finds_its_connection_ended() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "xor eax, eax",
                "mov edi, 4",
                "lea rsi, [rip + buffer]",
                "mov edx, 64",
                "syscall",
                "mov r12, rax",
                "mov eax, 1",
                "mov edi, 4",
                "lea rsi, [rip + buffer]",
                "mov edx, 2",
                "syscall",
                "sub r12, rax",
                "mov rdi, r12",
                "mov eax, 60",
                "syscall",
            ],
            &[".bss", "buffer:", ".zero 64"],
        ),
        aarch64: a64(
            &[
                "adrp x19, buffer",
                "add x19, x19, :lo12:buffer",
                "mov x0, #4",
                "mov x1, x19",
                "mov x2, #64",
                "mov x8, #63",
                "svc #0",
                "mov x20, x0",
                "mov x0, #4",
                "mov x1, x19",
                "mov x2, #2",
                "mov x8, #64",
                "svc #0",
                "sub x0, x20, x0",
                "mov x8, #93",
                "svc #0",
            ],
            &[".bss", "buffer:", ".zero 64"],
        ),
    }
}

/// A program that closes descriptors 3, 4 and 5 and exits with a bit for each that was open.
fn counts_its_descriptors() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov eax, 3",
                "mov edi, 3",
                "syscall",
                "mov r12, rax",
                "mov eax, 3",
                "mov edi, 4",
                "syscall",
                "mov r13, rax",
                "mov eax, 3",
                "mov edi, 5",
                "syscall",
                "mov r14, rax",
                "xor edi, edi",
                "test r12, r12",
                "jnz 1f",
                "or edi, 1",
                "1:",
                "test r13, r13",
                "jnz 2f",
                "or edi, 2",
                "2:",
                "test r14, r14",
                "jnz 3f",
                "or edi, 4",
                "3:",
                "mov eax, 60",
                "syscall",
            ],
            &[],
        ),
        aarch64: a64(
            &[
                "mov x0, #3",
                "mov x8, #57",
                "svc #0",
                "mov x19, x0",
                "mov x0, #4",
                "mov x8, #57",
                "svc #0",
                "mov x20, x0",
                "mov x0, #5",
                "mov x8, #57",
                "svc #0",
                "mov x21, x0",
                "mov x0, #0",
                "cbnz x19, 1f",
                "orr x0, x0, #1",
                "1:",
                "cbnz x20, 2f",
                "orr x0, x0, #2",
                "2:",
                "cbnz x21, 3f",
                "orr x0, x0, #4",
                "3:",
                "mov x8, #93",
                "svc #0",
            ],
            &[],
        ),
    }
}

/// A program that sends 256 KiB on its connection (descriptor 4) and exits at once.
fn sends_a_lot_and_exits() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov eax, 1",
                "mov edi, 4",
                "lea rsi, [rip + big]",
                "mov edx, 262144",
                "syscall",
                "mov eax, 60",
                "xor edi, edi",
                "syscall",
            ],
            &[".bss", "big:", ".zero 262144"],
        ),
        aarch64: a64(
            &[
                "mov x0, #4",
                "adrp x1, big",
                "add x1, x1, :lo12:big",
                "mov x2, #262144",
                "mov x8, #64",
                "svc #0",
                "mov x0, #0",
                "mov x8, #93",
                "svc #0",
            ],
            &[".bss", "big:", ".zero 262144"],
        ),
    }
}

/// A program that opens the file `f` until it can't, and exits with how many it opened, modulo
/// 256.
fn opens_until_it_cant() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "xor r12d, r12d",
                "1:",
                "mov eax, 257",
                "mov edi, -100",
                "lea rsi, [rip + name]",
                "xor edx, edx",
                "xor r10d, r10d",
                "syscall",
                "test rax, rax",
                "js 2f",
                "inc r12",
                "cmp r12, 10000",
                "jb 1b",
                "2:",
                "mov rdi, r12",
                "mov eax, 60",
                "syscall",
            ],
            &[".data", "name:", ".asciz \"f\""],
        ),
        aarch64: a64(
            &[
                "mov x12, #0",
                "mov x13, #10000",
                "1:",
                "mov x0, #-100",
                "adrp x1, name",
                "add x1, x1, :lo12:name",
                "mov x2, #0",
                "mov x3, #0",
                "mov x8, #56",
                "svc #0",
                "tbnz x0, #63, 2f",
                "add x12, x12, #1",
                "cmp x12, x13",
                "b.lo 1b",
                "2:",
                "mov x0, x12",
                "mov x8, #93",
                "svc #0",
            ],
            &[".data", "name:", ".asciz \"f\""],
        ),
    }
}

/// A program that asks the terminal's question, how many bytes wait to be read (`FIONREAD`),
/// which a terminal program may not.
fn asks_how_many_bytes_wait() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov eax, 16",
                "xor edi, edi",
                "mov esi, 0x541b",
                "lea rdx, [rip + waiting]",
                "syscall",
                "mov eax, 60",
                "xor edi, edi",
                "syscall",
            ],
            &[".data", "waiting:", ".zero 8"],
        ),
        aarch64: a64(
            &[
                "mov x0, #0",
                "mov x1, #0x541b",
                "adrp x2, waiting",
                "add x2, x2, :lo12:waiting",
                "mov x8, #29",
                "svc #0",
                "mov x0, #0",
                "mov x8, #93",
                "svc #0",
            ],
            &[".data", "waiting:", ".zero 8"],
        ),
    }
}

/// A program that opens the file its first argument names and exits with 0 when it can, 1 when
/// it can't.
fn opens_its_first_argument() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov rsi, qword ptr [rsp + 16]",
                "mov edi, -100",
                "xor edx, edx",
                "xor r10d, r10d",
                "mov eax, 257",
                "syscall",
                "shr rax, 63",
                "mov rdi, rax",
                "mov eax, 60",
                "syscall",
            ],
            &[],
        ),
        aarch64: a64(
            &[
                "ldr x1, [sp, #16]",
                "mov x0, #-100",
                "mov x2, #0",
                "mov x3, #0",
                "mov x8, #56",
                "svc #0",
                "lsr x0, x0, #63",
                "mov x8, #93",
                "svc #0",
            ],
            &[],
        ),
    }
}

/// A program that asks for a new user namespace, which no program here may.
fn unshares() -> Programs {
    PerCpu {
        x86_64: x86(
            &[
                "mov edi, 0x10000000",
                "mov eax, 272",
                "syscall",
                "xor edi, edi",
                "mov eax, 60",
                "syscall",
            ],
            &[],
        ),
        aarch64: a64(
            &[
                "mov x0, #0x10000000",
                "mov x8, #97",
                "svc #0",
                "mov x0, #0",
                "mov x8, #93",
                "svc #0",
            ],
            &[],
        ),
    }
}

// The limits.

#[tokio::test]
async fn runs_a_program_and_counts_its_instructions() {
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let program = built_asset(folder.path(), "main", &TEMPLATES).await;
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
        (None, None) => {
            lengua::skiplog::note("this computer gives a process no hardware instruction counters");
        }
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
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let program = built_asset(folder.path(), "main", &TEMPLATES).await;
    let usage = run_simply(&program, &[], b"").await;
    assert_eq!(usage.end, End::Exited(1));
    assert_eq!(usage.stderr, b"usage: negate <n>\n");
}

#[tokio::test]
async fn kills_programs_that_run_too_long() {
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let forever = built(folder.path(), "forever", &forever()).await;
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
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let chatter = built(folder.path(), "chatter", &chatter()).await;
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
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let environment = built(folder.path(), "environment", &checks_its_environment()).await;
    let result = run_simply(&environment, &args(&["a", "b"]), b"").await;
    assert_eq!(
        result.end,
        End::Exited(0),
        "no variables leak into programs"
    );
    let copy = built(folder.path(), "copy", &copies_its_input()).await;
    let echo = run_simply(&copy, &[], b"hello\n").await;
    assert_eq!(echo.stdout, b"hello\n", "stdin is given, then ends");
    assert_eq!(echo.end, End::Exited(0));
}

/// A descriptor a parent of lengua's left open without close-on-exec, as a harness or a CI runner
/// can, never reaches the program: the launcher closes every descriptor it doesn't hand over, so
/// the program's write to that number fails with EBADF, and the file outside its folder is left
/// as it was. (Landlock holds paths, not descriptors already open.)
#[cfg(unix)]
#[tokio::test]
async fn a_descriptor_lengua_s_parent_left_open_never_reaches_the_program() {
    if !per_target::runs_programs() {
        return;
    }
    use std::os::fd::AsRawFd as _;
    let folder = tempfile::tempdir().expect("temporary folder");
    let outside = folder.path().join("outside");
    let stray = std::fs::File::create(&outside).expect("a file");
    rustix::io::fcntl_setfd(&stray, rustix::io::FdFlags::empty()).expect("kept across exec");
    let writer = built(folder.path(), "writer", &writes_to(stray.as_raw_fd())).await;
    let run = run_simply(&writer, &[], b"").await;
    assert_eq!(run.end, End::Exited(9), "EBADF: {run:?}");
    assert!(
        std::fs::read(&outside).expect("the file").is_empty(),
        "nothing written outside"
    );
    drop(stray);
}

#[tokio::test]
async fn reports_signals() {
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let crash = built(folder.path(), "crash", &crashes()).await;
    let result = run_simply(&crash, &[], b"").await;
    assert_eq!(result.end, End::Signaled(11));
    assert!(result.end.describe(Limits::default()).contains("SIGSEGV"));
}

#[tokio::test]
async fn gives_files_and_reads_back_what_the_program_left() {
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let copy = built(folder.path(), "copy-file", &copies_a_file()).await;
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
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    // A system call the spec doesn't allow (getpid) stops the program.
    let getpid = built(
        folder.path(),
        "getpid",
        &makes_a_call_its_spec_doesnt_allow(),
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
    let outside = built(folder.path(), "outside", &opens_a_file_outside_its_folder()).await;
    let result = run_simply(&outside, &[], b"").await;
    assert_eq!(result.end, End::Exited(13), "EACCES");

    // A file can't grow past the limit: the write that would is cut short, and the next fails with
    // EFBIG, which ends the program here and doesn't kill it with a signal.
    let grow = built(folder.path(), "grow", &grows_a_file()).await;
    let limits = Limits {
        max_file_size: Some(10_000),
        ..Limits::default()
    };
    let result = run_with(&grow, &[], b"", &BTreeMap::new(), &file_syscalls(), limits).await;
    assert_eq!(result.end, End::Exited(27), "EFBIG");

    // In a folder of the user's, a file inside opens and one beside it, outside, doesn't.
    let opens = built(folder.path(), "opens", &opens_its_first_argument()).await;
    let inside = folder.path().join("inside");
    std::fs::create_dir(&inside).expect("its folder");
    std::fs::write(inside.join("here"), "inside").expect("a file inside");
    std::fs::write(folder.path().join("x"), "outside").expect("a file outside");
    let syscalls = file_syscalls();
    for (name, code) in [("here", 0), ("../x", 1)] {
        let arguments = vec![name.to_owned()];
        let job = Job {
            folder: Some(&inside),
            ..Job::new(&opens, &arguments, b"", &syscalls)
        };
        let result = run(launcher(), job, Limits::default()).await.expect("run");
        assert_eq!(result.end, End::Exited(code), "{name}: {result:?}");
    }
}

#[tokio::test]
async fn threads_start_only_with_the_canonical_flags() {
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    // A fork, clone with SIGCHLD alone: a new process rather than a thread of this one.
    let fork = built(folder.path(), "fork", &forks()).await;
    let threaded = per_target::host().allowed_syscalls(&[Syscall::Write, Syscall::Exit], true);
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
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let reader = built(
        folder.path(),
        "reader",
        &asks_for_random_bytes_and_the_clock(),
    )
    .await;
    let approved = calls(&[Syscall::Getrandom, Syscall::ClockGettime, Syscall::Exit]);
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

fn network_syscalls(spec: &[Syscall]) -> Vec<Allowed> {
    calls(spec)
}

/// The numbers of `socket` and `connect`, which no spec allows a program.
const SOCKET_AND_CONNECT: PerCpu<[i64; 2]> = PerCpu {
    x86_64: [x86_64::SOCKET, x86_64::CONNECT],
    aarch64: [aarch64::SOCKET, aarch64::CONNECT],
};

async fn run_on(executable: &Path, syscalls: &[Allowed], net: &Net) -> Run {
    let job = Job {
        net: Some(net),
        ..Job::new(executable, &[], b"", syscalls)
    };
    run(launcher(), job, Limits::default()).await.expect("run")
}

/// A program's connections, as lengua hands them over: `connections` at descriptors 4 and on,
/// after a listener at 3 when `listen` is set.
fn connected(connections: Vec<Reach>, listen: bool) -> Net {
    Net {
        connections,
        listen: listen.then_some(0),
        serve_anywhere: false,
    }
}

/// A system call that takes a descriptor first, with what the kernel's pin on it looks at.
struct OnTheChannel {
    name: &'static str,
    spec: Syscall,
    numbers: PerCpu<i64>,
    /// The second argument, for the calls whose pin looks at it.
    second: i64,
    /// A descriptor of the program's own to make the same call on.
    own: u32,
}

/// The calls a spec can allow that take a descriptor first and could reach lengua's channel to
/// its broker, which is descriptor 4,352.
const ON_THE_CHANNEL: [OnTheChannel; 10] = [
    OnTheChannel {
        name: "read",
        spec: Syscall::Read,
        numbers: PerCpu {
            x86_64: x86_64::READ,
            aarch64: aarch64::READ,
        },
        second: 0,
        own: 0,
    },
    OnTheChannel {
        name: "write",
        spec: Syscall::Write,
        numbers: PerCpu {
            x86_64: x86_64::WRITE,
            aarch64: aarch64::WRITE,
        },
        second: 0,
        own: 1,
    },
    OnTheChannel {
        name: "close",
        spec: Syscall::Close,
        numbers: PerCpu {
            x86_64: x86_64::CLOSE,
            aarch64: aarch64::CLOSE,
        },
        second: 0,
        own: 9,
    },
    OnTheChannel {
        name: "lseek",
        spec: Syscall::Lseek,
        numbers: PerCpu {
            x86_64: x86_64::LSEEK,
            aarch64: aarch64::LSEEK,
        },
        second: 0,
        own: 0,
    },
    OnTheChannel {
        name: "fstat",
        spec: Syscall::Fstat,
        numbers: PerCpu {
            x86_64: x86_64::FSTAT,
            aarch64: aarch64::FSTAT,
        },
        second: 0,
        own: 0,
    },
    OnTheChannel {
        name: "getdents64",
        spec: Syscall::Getdents64,
        numbers: PerCpu {
            x86_64: x86_64::GETDENTS64,
            aarch64: aarch64::GETDENTS64,
        },
        second: 0,
        own: 0,
    },
    OnTheChannel {
        name: "ioctl",
        spec: Syscall::Ioctl,
        numbers: PerCpu {
            x86_64: x86_64::IOCTL,
            aarch64: aarch64::IOCTL,
        },
        // TCGETS, which the terminal's pin allows on standard input, output and error.
        second: 0x5401,
        own: 1,
    },
    OnTheChannel {
        name: "shutdown",
        spec: Syscall::Shutdown,
        numbers: PerCpu {
            x86_64: x86_64::SHUTDOWN,
            aarch64: aarch64::SHUTDOWN,
        },
        second: 0,
        own: 1,
    },
    OnTheChannel {
        name: "sendto",
        spec: Syscall::Sendto,
        numbers: PerCpu {
            x86_64: x86_64::SENDTO,
            aarch64: aarch64::SENDTO,
        },
        second: 0,
        own: 1,
    },
    OnTheChannel {
        name: "accept4",
        spec: Syscall::Accept4,
        numbers: PerCpu {
            x86_64: x86_64::ACCEPT4,
            aarch64: aarch64::ACCEPT4,
        },
        second: 0,
        own: 3,
    },
];

/// A program that makes one system call, `number` on each CPU, on `descriptor` with every other
/// argument zero but the second, and exits with 0 if the call returned.
fn one_call(numbers: &PerCpu<i64>, descriptor: u32, second: i64) -> Programs {
    let x86_lines = [
        format!("mov eax, {}", numbers.x86_64),
        format!("mov edi, {descriptor}"),
        format!("mov esi, {second}"),
        "xor edx, edx".to_owned(),
        "xor r10d, r10d".to_owned(),
        "xor r8d, r8d".to_owned(),
        "xor r9d, r9d".to_owned(),
        "syscall".to_owned(),
        "xor edi, edi".to_owned(),
        "mov eax, 231".to_owned(),
        "syscall".to_owned(),
    ];
    let a64_lines = [
        format!("mov x8, #{}", numbers.aarch64),
        format!("mov x0, #{descriptor}"),
        format!("mov x1, #{second}"),
        "mov x2, #0".to_owned(),
        "mov x3, #0".to_owned(),
        "mov x4, #0".to_owned(),
        "mov x5, #0".to_owned(),
        "svc #0".to_owned(),
        "mov x0, #0".to_owned(),
        "mov x8, #94".to_owned(),
        "svc #0".to_owned(),
    ];
    PerCpu {
        x86_64: x86(
            &x86_lines.iter().map(String::as_str).collect::<Vec<_>>(),
            &[],
        ),
        aarch64: a64(
            &a64_lines.iter().map(String::as_str).collect::<Vec<_>>(),
            &[],
        ),
    }
}

/// The kernel holds a program off lengua's channel to its broker, descriptor 4,352: every call
/// that takes a descriptor first is stopped on it and goes on on a descriptor of the program's
/// own, and `sendmsg` and `recvmsg`, which the channel is for, are allowed on it alone.
#[tokio::test]
async fn the_kernel_holds_a_program_off_the_channel_but_for_the_two_calls_it_is_for() {
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let spec: Vec<Syscall> = ON_THE_CHANNEL
        .iter()
        .map(|call| call.spec)
        .chain([Syscall::Exit])
        .collect();
    let allowed = calls(&spec);
    let channel = lengua::edge::CHANNEL;
    let run_it = |program: PathBuf| {
        let allowed = allowed.clone();
        async move {
            let job = Job {
                channel: Some(Permit::ALL),
                ..Job::new(&program, &[], b"", &allowed)
            };
            run(launcher(), job, Limits::default()).await.expect("run")
        }
    };
    for call in &ON_THE_CHANNEL {
        let on_it = built(
            folder.path(),
            &format!("{}-on-the-channel", call.name),
            &one_call(&call.numbers, channel, call.second),
        )
        .await;
        let own = built(
            folder.path(),
            &format!("{}-on-its-own", call.name),
            &one_call(&call.numbers, call.own, call.second),
        )
        .await;
        let refused = run_it(on_it).await;
        assert_eq!(
            refused.end,
            End::Signaled(31),
            "{} on the channel: {refused:?}",
            call.name
        );
        let allowed_on_its_own = run_it(own).await;
        assert_eq!(
            allowed_on_its_own.end,
            End::Exited(0),
            "{} on descriptor {}: {allowed_on_its_own:?}",
            call.name,
            call.own
        );
    }
    let messages = PerCpu {
        x86_64: x86_64::RECVMSG,
        aarch64: aarch64::RECVMSG,
    };
    let on_it = built(
        folder.path(),
        "recvmsg-on-the-channel",
        &one_call(&messages, channel, 0),
    )
    .await;
    let elsewhere = built(
        folder.path(),
        "recvmsg-on-another",
        &one_call(&messages, 5, 0),
    )
    .await;
    let allowed_here = run_it(on_it).await;
    assert_eq!(allowed_here.end, End::Exited(0), "{allowed_here:?}");
    let refused = run_it(elsewhere).await;
    assert_eq!(refused.end, End::Signaled(31), "{refused:?}");
}

/// A program with a channel to the broker has the broker make and find every path for it, so the
/// kernel stops its own `openat` and the calls like it: where Landlock would have refused the
/// file, the program is stopped before it can ask, and learns nothing of what is outside.
#[tokio::test]
async fn a_program_with_a_channel_makes_no_path_call_of_its_own() {
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let outside = built(folder.path(), "outside", &opens_a_file_outside_its_folder()).await;
    let allowed = file_syscalls();
    let with_channel = Job {
        channel: Some(Permit::ALL),
        ..Job::new(&outside, &[], b"", &allowed)
    };
    let stopped = run(launcher(), with_channel, Limits::default())
        .await
        .expect("run");
    assert_eq!(stopped.end, End::Signaled(31), "{stopped:?}");
    // Without a channel the same call goes on to Landlock, which refuses the file.
    let alone = run_simply(&outside, &[], b"").await;
    assert_eq!(alone.end, End::Exited(13), "EACCES: {alone:?}");
}

#[tokio::test]
async fn a_program_never_holds_the_network() {
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("an address").port();
    let client = built(folder.path(), "client", &connects_to(port)).await;
    // Even with lengua making a connection there for it, its own socket is a call no spec
    // allows: stopped.
    let net = connected(vec![Reach::ThisComputer(port)], false);
    let calls = network_syscalls(&[Syscall::Read, Syscall::Sendto, Syscall::Exit]);
    assert_eq!(run_on(&client, &calls, &net).await.end, End::Signaled(31));
    // Were seccomp to let it open one, Landlock would still hold it off connecting anywhere.
    let mut raw = network_syscalls(&[Syscall::Exit]);
    raw.extend(
        SOCKET_AND_CONNECT
            .get(per_target::host())
            .iter()
            .map(|number| Allowed {
                number: *number,
                pin: Pin::Any,
            }),
    );
    assert_eq!(
        run_on(&client, &raw, &net).await.end,
        End::Exited(13),
        "EACCES: Landlock allows no program a connection of its own"
    );
}

#[tokio::test]
async fn servers_take_connections_on_descriptor_3() {
    if !per_target::runs_programs() {
        return;
    }
    use tokio::io::AsyncReadExt as _;
    let folder = tempfile::tempdir().expect("temporary folder");
    // Takes one connection, says hi on it and exits.
    let server = built(folder.path(), "server", &serves_one_client()).await;
    let calls = network_syscalls(&[Syscall::Accept4, Syscall::Write, Syscall::Exit]);
    let net = Net {
        connections: Vec::new(),
        listen: Some(0),
        serve_anywhere: false,
    };
    let job = Job {
        net: Some(&net),
        ..Job::new(&server, &[], b"", &calls)
    };
    let started = start(launcher(), job, Limits::default())
        .await
        .expect("start");
    let port = started.port().expect("a port");
    // On the loopback alone: 127.0.0.2 is this computer too, but not the address it listens on.
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.2", port))
            .await
            .is_err()
    );
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connect");
    let mut said = Vec::new();
    stream.read_to_end(&mut said).await.expect("read");
    assert_eq!(said, b"hi");
    assert_eq!(started.wait().await.expect("wait").end, End::Exited(0));

    // A server open to any computer listens on every address.
    let open = Net {
        serve_anywhere: true,
        ..net.clone()
    };
    let job = Job {
        net: Some(&open),
        ..Job::new(&server, &[], b"", &calls)
    };
    let started = start(launcher(), job, Limits::default())
        .await
        .expect("start");
    let port = started.port().expect("a port");
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.2", port))
        .await
        .expect("connect from another address");
    let mut said = Vec::new();
    stream.read_to_end(&mut said).await.expect("read");
    assert_eq!(said, b"hi");
    assert_eq!(started.wait().await.expect("wait").end, End::Exited(0));

    // A server that keeps serving is stopped by lengua.
    let forever = built(folder.path(), "forever", &serves_for_ever()).await;
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

#[tokio::test]
async fn connections_are_handed_over_from_descriptor_4() {
    if !per_target::runs_programs() {
        return;
    }
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let folder = tempfile::tempdir().expect("temporary folder");
    let client = built(folder.path(), "client", &answers_ok()).await;
    let calls = network_syscalls(&[Syscall::Read, Syscall::Write, Syscall::Exit]);
    // The test's own server speaks on the program's stream itself.
    let net = connected(vec![Reach::Peer], false);
    let job = Job {
        net: Some(&net),
        ..Job::new(&client, &[], b"", &calls)
    };
    let mut started = start(launcher(), job, Limits::default())
        .await
        .expect("start");
    let mut peers = started.take_peers();
    assert_eq!(peers.len(), 1);
    let (number, mut stream) = peers.remove(0);
    assert_eq!(number, 0);
    stream.write_all(b"hello").await.expect("send");
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).await.expect("the answer");
    assert_eq!(answer, b"ok", "and then the end, as the program exits");
    let run = started.wait().await.expect("wait");
    assert_eq!(run.end, End::Exited(0), "{run:?}");
    assert_eq!(run.stdout, b"hello");

    // A port of this computer: lengua connects to it and relays both ways.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a free port");
    let port = listener.local_addr().expect("an address").port();
    let place = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("lengua's connection");
        stream.write_all(b"hello").await.expect("send");
        let mut answer = Vec::new();
        stream.read_to_end(&mut answer).await.expect("the answer");
        answer
    });
    let net = connected(vec![Reach::ThisComputer(port)], false);
    let run = run_on(&client, &calls, &net).await;
    assert_eq!(run.end, End::Exited(0), "{run:?}");
    assert_eq!(run.stdout, b"hello");
    assert_eq!(place.await.expect("the place"), b"ok");
    assert_eq!(run.connections.len(), 1);
    assert_eq!(run.connections[0].problem, None);
}

#[tokio::test]
async fn a_connection_lengua_couldnt_make_is_ended_both_ways() {
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let ended = built(folder.path(), "ended", &finds_its_connection_ended()).await;
    let calls = network_syscalls(&[Syscall::Read, Syscall::Write, Syscall::Exit]);
    let nowhere = connected(vec![Reach::Nowhere], false);
    let run = run_on(&ended, &calls, &nowhere).await;
    assert_eq!(run.end, End::Exited(32), "{run:?}");
    // A port nothing listens on is the same to the program; the report says why.
    let free = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = free.local_addr().expect("an address").port();
    drop(free);
    let refused = connected(vec![Reach::ThisComputer(port)], false);
    let run = run_on(&ended, &calls, &refused).await;
    assert_eq!(run.end, End::Exited(32), "{run:?}");
    assert!(
        run.connections[0].problem.is_some(),
        "{:?}",
        run.connections
    );
    // So is the computer an argument names, where the argument is missing or names this
    // computer: lengua reads it before the program starts, and the report says why.
    let typed = connected(
        vec![Reach::Argument {
            index: 1,
            port,
            tls: false,
        }],
        false,
    );
    for (args, why) in [
        (vec![], "no argument 1"),
        (vec!["localhost".to_owned()], "is this computer"),
    ] {
        let job = Job {
            net: Some(&typed),
            ..Job::new(&ended, &args, b"", &calls)
        };
        let typed_run = lengua::runner::run(launcher(), job, Limits::default())
            .await
            .expect("run");
        assert_eq!(typed_run.end, End::Exited(32), "{typed_run:?}");
        assert!(
            typed_run.connections[0]
                .problem
                .as_deref()
                .is_some_and(|problem| problem.contains(why)),
            "{why}: {:?}",
            typed_run.connections
        );
    }
}

#[tokio::test]
async fn each_connection_has_its_own_number() {
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    // Closes descriptors 3, 4 and 5, and exits with a bit for each that was open.
    let numbers = built(folder.path(), "numbers", &counts_its_descriptors()).await;
    let calls = network_syscalls(&[Syscall::Close, Syscall::Exit]);
    let client = connected(vec![Reach::Nowhere], false);
    assert_eq!(run_on(&numbers, &calls, &client).await.end, End::Exited(2));
    let two = connected(vec![Reach::Nowhere, Reach::Nowhere], false);
    assert_eq!(run_on(&numbers, &calls, &two).await.end, End::Exited(6));
    let server = connected(vec![Reach::Nowhere, Reach::Nowhere], true);
    assert_eq!(run_on(&numbers, &calls, &server).await.end, End::Exited(7));
}

#[tokio::test]
async fn what_a_program_sends_last_reaches_its_place() {
    if !per_target::runs_programs() {
        return;
    }
    use tokio::io::AsyncReadExt as _;
    let folder = tempfile::tempdir().expect("temporary folder");
    // Sends 256 KiB and exits at once.
    let sender = built(folder.path(), "sender", &sends_a_lot_and_exits()).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a free port");
    let port = listener.local_addr().expect("an address").port();
    let place = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("lengua's connection");
        // Slow to read, as a busy server is.
        tokio::time::sleep(Duration::from_millis(200)).await;
        let mut got = Vec::new();
        stream.read_to_end(&mut got).await.expect("everything");
        got.len()
    });
    let calls = network_syscalls(&[Syscall::Write, Syscall::Exit]);
    let net = connected(vec![Reach::ThisComputer(port)], false);
    let run = run_on(&sender, &calls, &net).await;
    assert_eq!(run.end, End::Exited(0), "{run:?}");
    assert_eq!(place.await.expect("the place"), 262_144);
}

#[tokio::test]
async fn a_program_holds_as_many_open_files_on_every_system() {
    if !per_target::runs_programs() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let opener = built(folder.path(), "opener", &opens_until_it_cant()).await;
    let files = BTreeMap::from([("f".to_owned(), "x".to_owned())]);
    let run = run_with(
        &opener,
        &[],
        b"",
        &files,
        &file_syscalls(),
        Limits::default(),
    )
    .await;
    // lengua's limit, less standard input, output and error.
    let opened = lengua::runner::OPEN_FILES - 3;
    assert_eq!(
        run.end,
        End::Exited(i32::try_from(opened % 256).expect("a byte")),
        "{run:?}"
    );
}

/// Runs the tests of the template's spec, with `edit` applied to it, `workers` at a time.
async fn template_tests(edit: impl Fn(&str) -> String, workers: usize) -> lengua::testrun::Outcome {
    let spec = lengua::spec::parse(&edit(&asset("template-spec.md"))).expect("the template's spec");
    let suite = lengua::testfile::check(&spec).expect("the tests check");
    let host = per_target::host();
    let code = asset(TEMPLATES.get(host));
    let (source, _) = lengua::checker::source::parse(&code);
    let folder = tempfile::tempdir().expect("a folder");
    let executable = built_asset(folder.path(), "main", &TEMPLATES).await;
    let syscalls = host.allowed_syscalls(&spec.syscalls, false);
    lengua::testrun::run_tests(
        &suite,
        launcher(),
        &executable,
        &source,
        7,
        &syscalls,
        &spec.network,
        workers,
        lengua::testrun::Setting::default(),
    )
    .await
    .expect("the tests run")
}

/// Runs the tests of the compile skill's asset spec `spec` against the program `code`, which is
/// the assembly of the target this computer is.
async fn asset_tests(spec: &str, code: &str) -> lengua::testrun::Outcome {
    let spec = lengua::spec::parse(&asset(spec)).expect("the asset's spec");
    let suite = lengua::testfile::check(&spec).expect("the tests check");
    let (source, _) = lengua::checker::source::parse(code);
    let folder = tempfile::tempdir().expect("a folder");
    let host = per_target::host();
    let executable = per_target::build(host, folder.path(), "main", code).await;
    let syscalls = host.allowed_syscalls(&spec.syscalls, false);
    lengua::testrun::run_tests(
        &suite,
        launcher(),
        &executable,
        &source,
        7,
        &syscalls,
        &spec.network,
        8,
        lengua::testrun::Setting::default(),
    )
    .await
    .expect("the tests run")
}

/// The event-loop server template of each target, and a server that serves a client at a time,
/// which the first test must tell from it: the echo example.
const SERVERS: PerCpu<(&str, &str)> = PerCpu {
    x86_64: ("server-template.s", "examples/echo/main.s"),
    aarch64: (
        "linux-aarch64/server-template.s",
        "tests/fixtures/linux-aarch64/echo/main.s",
    ),
};

/// A server for clients at once answers each client as its request completes, while others are
/// still sending; one that serves a client at a time is caught waiting on the first.
#[tokio::test]
async fn a_server_for_clients_at_once_answers_each_while_others_send() {
    if !per_target::runs_programs() {
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let (template, one_at_a_time) = *SERVERS.get(per_target::host());
    let template = asset(template);
    let outcome = asset_tests("server-template-spec.md", &template).await;
    assert!(outcome.passed(), "{:#?}", outcome.failures);
    assert!(outcome.runs > 2, "{}", outcome.runs);
    let one_at_a_time = std::fs::read_to_string(root.join(one_at_a_time)).expect("the echo");
    let outcome = asset_tests("server-template-spec.md", &one_at_a_time).await;
    assert!(
        outcome
            .failures
            .iter()
            .any(|failure| failure.contains("while connection 1 was still sending")),
        "{:#?}",
        outcome.failures
    );
}

/// A program for the terminal takes keys from a pipe as it would from a terminal, and the kernel
/// lets it read and set the terminal's mode and nothing else.
#[tokio::test]
async fn a_terminal_program_takes_keys_from_a_pipe_too() {
    if !per_target::runs_programs() {
        return;
    }
    let terminals = PerCpu {
        x86_64: "terminal-template.s",
        aarch64: "linux-aarch64/terminal-template.s",
    };
    let host = per_target::host();
    let template = asset(terminals.get(host));
    let outcome = asset_tests("terminal-template-spec.md", &template).await;
    assert!(outcome.passed(), "{:#?}", outcome.failures);
    // Asking how many bytes wait to be read, FIONREAD, isn't one of the terminal's requests.
    let folder = tempfile::tempdir().expect("temporary folder");
    let asks = built(folder.path(), "asks", &asks_how_many_bytes_wait()).await;
    let calls = host.allowed_syscalls(Use::Terminal.syscalls(), false);
    let ran = run(
        launcher(),
        Job::new(&asks, &[], b"", &calls),
        Limits::default(),
    )
    .await
    .expect("run");
    assert_eq!(ran.end, End::Signaled(31), "{ran:?}");
}

#[tokio::test]
async fn runs_at_once_report_what_runs_one_at_a_time_do() {
    if !per_target::runs_programs() {
        return;
    }
    // Passing tests: the same runs, and each test timed alone on its largest input.
    let alone = template_tests(str::to_owned, 1).await;
    let together = template_tests(str::to_owned, 8).await;
    assert!(
        alone.passed() && together.passed(),
        "{alone:?} {together:?}"
    );
    assert_eq!(alone.runs, 4 + 200 + 50 + 20);
    assert_eq!(together.runs, alone.runs);
    let shape = |outcome: &lengua::testrun::Outcome| -> Vec<(String, usize, String)> {
        outcome
            .costs
            .iter()
            .map(|cost| (cost.test.clone(), cost.runs, cost.timed.clone()))
            .collect()
    };
    assert_eq!(shape(&together), shape(&alone));
    // The examples first, B1 to B4, then the generated blocks.
    assert!(
        together.costs[4].timed.starts_with("its largest input, "),
        "{:?}",
        together.costs[4]
    );
    assert!(together.costs[4].timed.ends_with("run alone 5 times"));

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
    let edited = wrong(&asset("template-spec.md"));
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

/// The targets this computer runs only through QEMU's user mode (a note says when it can't run
/// one): the kernel then sees QEMU's own system calls, not the program's.
fn emulated_targets() -> Vec<lengua::platform::Profile> {
    lengua::platform::Profile::ALL
        .into_iter()
        .filter(|target| per_target::way(*target) == Some(per_target::Way::Emulated))
        .collect()
}

/// QEMU's user mode leaves the connections where lengua put them, and the launcher's ignored
/// SIGPIPE ignored.
#[tokio::test]
async fn an_emulated_program_gets_its_connections_too() {
    if !per_target::runs_programs() {
        return;
    }
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let scratch = tempfile::tempdir().expect("a folder");
    for target in emulated_targets() {
        let place = scratch.path().join(target.name());
        let client = per_target::build(target, &place, "client", answers_ok().get(target)).await;
        let net = connected(vec![Reach::Peer], false);
        let job = Job {
            net: Some(&net),
            emulated: true,
            ..Job::new(&client, &[], b"", &[])
        };
        let mut started = start(launcher(), job, Limits::default())
            .await
            .expect("start");
        let (_, mut stream) = started.take_peers().remove(0);
        stream.write_all(b"hello").await.expect("send");
        let mut answer = Vec::new();
        stream.read_to_end(&mut answer).await.expect("the answer");
        assert_eq!(answer, b"ok", "{}", target.name());
        let answered = started.wait().await.expect("wait");
        assert_eq!(answered.end, End::Exited(0), "{answered:?}");
        assert_eq!(answered.stdout, b"hello");
        let ended = per_target::build(
            target,
            &place,
            "ended",
            finds_its_connection_ended().get(target),
        )
        .await;
        let nowhere = connected(vec![Reach::Nowhere], false);
        let job = Job {
            net: Some(&nowhere),
            emulated: true,
            ..Job::new(&ended, &[], b"", &[])
        };
        let ended = run(launcher(), job, Limits::default()).await.expect("run");
        assert_eq!(ended.end, End::Exited(32), "{ended:?}");
    }
}

/// A program QEMU's user mode runs is held by Landlock and the limits as any program is: it
/// opens files only inside its folder. The kernel can't hold its calls to its spec's, since it
/// sees QEMU's, but it stops the calls QEMU never needs to make, such as `unshare`; and nothing
/// is counted, since the instructions would be QEMU's.
#[tokio::test]
async fn an_emulated_program_is_confined_by_landlock_and_the_kernel() {
    if !per_target::runs_programs() {
        return;
    }
    let scratch = tempfile::tempdir().expect("a folder");
    for target in emulated_targets() {
        let place = scratch.path().join(target.name());
        let opens = per_target::build(
            target,
            &place,
            "opens",
            opens_its_first_argument().get(target),
        )
        .await;
        let unshares = per_target::build(target, &place, "unshares", unshares().get(target)).await;
        let folder = place.join("inside");
        std::fs::create_dir(&folder).expect("its folder");
        std::fs::write(folder.join("here"), "inside").expect("a file inside");
        std::fs::write(place.join("x"), "outside").expect("a file outside");
        let run_emulated = |program: &Path, arguments: Vec<String>| {
            let (program, folder) = (program.to_owned(), folder.clone());
            async move {
                let job = Job {
                    folder: Some(&folder),
                    emulated: true,
                    ..Job::new(&program, &arguments, &[], &[])
                };
                run(launcher(), job, Limits::default())
                    .await
                    .expect("the run")
            }
        };
        let inside = run_emulated(&opens, args(&["here"])).await;
        assert_eq!(inside.end, End::Exited(0), "{inside:?}");
        assert_eq!(inside.instructions, None, "{inside:?}");
        let outside = run_emulated(&opens, args(&["../x"])).await;
        assert_eq!(outside.end, End::Exited(1), "{outside:?}");
        let unshared = run_emulated(&unshares, Vec::new()).await;
        assert_eq!(unshared.end, End::Signaled(31), "{unshared:?}");
    }
}

/// How much sooner `count`'s 924 runs finish with every core: run with
/// `cargo test --release --test limits -- --ignored --nocapture speedup`.
#[tokio::test]
#[ignore = "a measurement, not a check"]
async fn speedup_of_runs_at_once_on_the_word_count() {
    if !per_target::runs_programs() {
        return;
    }
    let counts = PerCpu {
        x86_64: "examples/count",
        aarch64: "tests/fixtures/linux-aarch64/count",
    };
    let host = per_target::host();
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(counts.get(host));
    let read = |name: &str| std::fs::read_to_string(example.join(name)).expect("the example");
    let spec = lengua::spec::parse(&read("spec.md")).expect("the spec");
    let suite = lengua::testfile::check(&spec).expect("the tests");
    let (source, _) = lengua::checker::source::parse(&read("main.s"));
    let folder = tempfile::tempdir().expect("a folder");
    let executable = per_target::build(host, folder.path(), "count", &read("main.s")).await;
    let syscalls = host.allowed_syscalls(&spec.syscalls, false);
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
            lengua::testrun::Setting::default(),
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
