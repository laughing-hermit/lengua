//! The red team: programs written to get past lengua's own checks, on every target. Each attempt
//! is the target's template with one change that tries it, and `check` must refuse it up front:
//! a system call the spec doesn't allow, a system call number chosen by a register or a label,
//! an access past a buffer, an unchecked or late index, an address made from a plain number, a
//! write to read-only data, a return address changed or written over, a read below the stack
//! pointer, a loop the rules miscount, a stray backward jump, recursion, a call through a
//! register, a missing `ret`, a tampered assertion handler, and an ignored result. Two
//! attempts no rule can follow are held to what does hold: a pointer read from memory is never
//! stamped `done` when it is wrong and never reported proven when it is good, and a wrong
//! program is never stamped `done` once the proof has run. None is ever run when a rule
//! refuses it first. Any attempt that isn't refused is a bug in lengua to fix before anything
//! else.
//!
//! There is one table of attempts, and each has a cell for every target (`PerCpu`): the change
//! to that target's template and what a refusal must name, or why the attempt can't be written
//! on that CPU. A target added to lengua breaks this table until its cells are filled. A target
//! is tried where this computer runs its programs, natively or through QEMU's user mode; the
//! other's cells are checked for well-formedness on any computer, and its attempts run on its
//! own runner.

mod common;
mod per_target;

use std::path::PathBuf;

use common::{Program, Protocol, Session};
use lengua::platform::Profile;
use lengua::targets::PerCpu;
use serde_json::Value;

/// A change to a template: the first `from` in it becomes `to`.
type Edit = (&'static str, &'static str);

/// What `check` must do with an attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expect {
    /// Refuse it before anything runs, with a problem naming one of the cell's needles.
    Refused,
    /// A pointer read from memory that is wrong: no rule can follow it, so the tests run and the
    /// sandbox kills the stray read, and the program is never stamped `done`.
    NeverDone,
    /// A pointer read from memory that happens to be good: the program may be `done`, and then
    /// the report says that memory is tested, not proven.
    TestedNotProven,
    /// A bug the tests' samples may miss and the proof covers: whichever finds it, the program
    /// is not `done`. It takes the solvers most of their time.
    ProofFindsIt,
}

/// The attempts that run together, as one test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    /// System calls the spec doesn't allow.
    SystemCalls,
    /// A system call number chosen by something the rules can't see.
    CallChoice,
    /// Accesses outside their owner.
    Accesses,
    /// Addresses made from a plain number or a pointer read from memory.
    Addresses,
    /// Writes to read-only data and to a return address, and reads below the stack pointer.
    Writes,
    /// Loops the rules can't count.
    Loops,
    /// Recursion, calls through registers and functions that don't return.
    Calls,
    /// A tampered assertion handler.
    Handler,
    /// Code that ignores a failed result.
    Results,
    /// A wrong program the proof must catch.
    Proof,
}

/// What one target does with an attempt.
#[derive(Debug, Clone, Copy)]
enum Cell {
    /// The target's template with `edits` made, in order; a refusal names one of `needles`.
    Try {
        edits: &'static [Edit],
        needles: &'static [&'static str],
    },
    /// The target's template with a system call of this number made just before its exit.
    Call(i64),
    /// The attempt can't be written on this CPU, for this reason of its hardware.
    NotApplicable(&'static str),
}

/// One way of trying to get past the checks, on every target.
#[derive(Debug, Clone, Copy)]
struct Attempt {
    family: Family,
    /// What it tries, in words.
    what: &'static str,
    expect: Expect,
    cells: PerCpu<Cell>,
}

/// Each target's template, in the compile skill's assets.
const TEMPLATES: PerCpu<&str> = PerCpu {
    x86_64: "main-template.s",
    aarch64: "linux-aarch64/main-template.s",
};

/// The template's exit, in each target's instructions.
const EXITS: PerCpu<&str> = PerCpu {
    x86_64: "    mov eax, SYS_EXIT\n    syscall\n",
    aarch64: "    mov x8, #SYS_EXIT\n    svc #0\n",
};

/// What a refusal of a system call names: not one the spec allows, or a connection a program
/// never makes by itself.
const CALL_NEEDLES: &[&str] = &["isn't one the spec allows", "never connects by itself"];

/// System calls the template's spec doesn't allow and both CPUs have, each with its number on
/// x86-64 and on AArch64. Each is a call a program could use to reach beyond its folder, its
/// memory or its user; the kernel's filter kills it too, but the rules refuse the program first.
const CALLS: &[(&str, i64, i64)] = &[
    ("openat", 257, 56),
    ("mmap", 9, 222),
    ("mprotect", 10, 226),
    ("clone", 56, 220),
    ("execve", 59, 221),
    ("kill", 62, 129),
    ("ptrace", 101, 117),
    ("setuid", 105, 146),
    ("prctl", 157, 167),
    ("mount", 165, 40),
    ("unshare", 272, 97),
    ("execveat", 322, 281),
    ("io_uring_setup", 425, 425),
    ("socket", 41, 198),
    ("connect", 42, 203),
];

/// Every attempt but the system calls, which `call_attempts` makes.
const ATTEMPTS: &[Attempt] = &[
    // A system call number chosen by a value the rules can't see.
    Attempt {
        family: Family::CallChoice,
        what: "a number from a register",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov eax, SYS_EXIT\n    syscall\n",
                    "    mov eax, edi\n    syscall\n    mov eax, SYS_EXIT\n    syscall\n",
                )],
                needles: &["isn't chosen by a `mov eax, SYS_NAME`"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    mov x8, #SYS_EXIT\n    svc #0\n",
                    "    mov x8, x9\n    svc #0\n    mov x8, #SYS_EXIT\n    svc #0\n",
                )],
                needles: &["isn't chosen by a `mov"],
            },
        },
    },
    Attempt {
        family: Family::CallChoice,
        what: "a label between the number and the call",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov eax, SYS_EXIT\n    syscall\n",
                    "    mov eax, SYS_EXIT\n.Lstart_sneak:\n    syscall\n",
                )],
                needles: &["isn't chosen by a `mov eax, SYS_NAME`", "rule 1"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    mov x8, #SYS_EXIT\n    svc #0\n",
                    "    mov x8, #SYS_EXIT\n.Lstart_sneak:\n    svc #0\n",
                )],
                needles: &["isn't chosen by a `mov", "rule 1"],
            },
        },
    },
    // Accesses outside their owner: one byte past usage_msg, which is 18 bytes, and far past it,
    // where another symbol or nothing lies.
    Attempt {
        family: Family::Accesses,
        what: "a read past a buffer",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov rdi, qword ptr [rsp + 16]\n",
                    concat!(
                        "    movzx ecx, byte ptr [rip + usage_msg + 18]\n",
                        "    mov rdi, qword ptr [rsp + 16]\n"
                    ),
                )],
                needles: &["rule M2", "past the end of"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lstart_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lstart_assert_argv\n",
                        "    adrp x9, usage_msg\n",
                        "    add x9, x9, :lo12:usage_msg\n",
                        "    ldrb w10, [x9, #18]\n"
                    ),
                )],
                needles: &["rule M2"],
            },
        },
    },
    Attempt {
        family: Family::Accesses,
        what: "a read far past a buffer",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov rdi, qword ptr [rsp + 16]\n",
                    concat!(
                        "    movzx ecx, byte ptr [rip + usage_msg + 4096]\n",
                        "    mov rdi, qword ptr [rsp + 16]\n"
                    ),
                )],
                needles: &["rule M2", "past the end of"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lstart_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lstart_assert_argv\n",
                        "    adrp x9, usage_msg\n",
                        "    add x9, x9, :lo12:usage_msg\n",
                        "    ldrb w10, [x9, #4095]\n"
                    ),
                )],
                needles: &["rule M2"],
            },
        },
    },
    Attempt {
        family: Family::Accesses,
        what: "an unchecked index",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov rdi, qword ptr [rsp + 16]\n    test rdi, rdi\n",
                    concat!(
                        "    mov rdi, qword ptr [rsp + 16]\n",
                        "    mov rcx, qword ptr [rsp + 8]\n",
                        "    movzx edx, byte ptr [rdi + rcx]\n",
                        "    test rdi, rdi\n"
                    ),
                )],
                needles: &["rule 9", "rule M2"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lstart_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lstart_assert_argv\n",
                        "    ldr x11, [sp, #8]\n",
                        "    ldrb w10, [x0, x11]\n"
                    ),
                )],
                needles: &["rule 9", "rule M2"],
            },
        },
    },
    Attempt {
        family: Family::Accesses,
        what: "an index checked too late",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov rdi, qword ptr [rsp + 16]\n    test rdi, rdi\n",
                    concat!(
                        "    mov rdi, qword ptr [rsp + 16]\n",
                        "    mov rcx, qword ptr [rsp + 8]\n",
                        "    movzx edx, byte ptr [rdi + rcx]\n",
                        "    cmp rcx, 8\n",
                        "    jae .Lstart_assert_argv\n",
                        "    test rdi, rdi\n"
                    ),
                )],
                needles: &["rule 9", "rule M2"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lstart_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lstart_assert_argv\n",
                        "    ldr x11, [sp, #8]\n",
                        "    ldrb w10, [x0, x11]\n",
                        "    cmp x11, #8\n",
                        "    b.hs .Lstart_assert_argv\n"
                    ),
                )],
                needles: &["rule 9", "rule M2"],
            },
        },
    },
    // An address made from a plain number, and a pointer read from memory, which has no owner
    // the rules can follow: tested, not proven.
    Attempt {
        family: Family::Addresses,
        what: "a fixed address",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov rdi, qword ptr [rsp + 16]\n",
                    concat!(
                        "    mov esi, 4096\n",
                        "    movzx ecx, byte ptr [rsi]\n",
                        "    mov rdi, qword ptr [rsp + 16]\n"
                    ),
                )],
                needles: &["rule M1"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lstart_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lstart_assert_argv\n",
                        "    mov x9, #4096\n",
                        "    ldrb w10, [x9]\n"
                    ),
                )],
                needles: &["rule M1"],
            },
        },
    },
    Attempt {
        family: Family::Addresses,
        what: "a wrong pointer loaded from memory",
        expect: Expect::NeverDone,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov rdi, qword ptr [rsp + 16]\n",
                    concat!(
                        "    lea rax, [rip + usage_msg]\n",
                        "    mov rsi, qword ptr [rax]\n",
                        "    movzx ecx, byte ptr [rsi]\n",
                        "    mov rdi, qword ptr [rsp + 16]\n"
                    ),
                )],
                needles: &[],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lstart_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lstart_assert_argv\n",
                        "    adrp x9, usage_msg\n",
                        "    add x9, x9, :lo12:usage_msg\n",
                        "    ldr x10, [x9]\n",
                        "    ldrb w11, [x10]\n"
                    ),
                )],
                needles: &[],
            },
        },
    },
    Attempt {
        family: Family::Addresses,
        what: "a good pointer loaded from memory (argv[0])",
        expect: Expect::TestedNotProven,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov rdi, qword ptr [rsp + 16]\n",
                    concat!(
                        "    mov rsi, qword ptr [rsp + 8]\n",
                        "    movzx ecx, byte ptr [rsi]\n",
                        "    mov rdi, qword ptr [rsp + 16]\n"
                    ),
                )],
                needles: &[],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lstart_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lstart_assert_argv\n",
                        "    ldr x9, [sp, #8]\n",
                        "    ldrb w10, [x9]\n"
                    ),
                )],
                needles: &[],
            },
        },
    },
    // Writes to read-only data, a return address changed or written over, and a read below the
    // stack pointer, where a signal handler or a callee could have left data.
    Attempt {
        family: Family::Writes,
        what: "a write to read-only data",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov rdi, qword ptr [rsp + 16]\n",
                    concat!(
                        "    mov byte ptr [rip + usage_msg], 0\n",
                        "    mov rdi, qword ptr [rsp + 16]\n"
                    ),
                )],
                needles: &["rule M6", "read-only"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lstart_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lstart_assert_argv\n",
                        "    adrp x9, usage_msg\n",
                        "    add x9, x9, :lo12:usage_msg\n",
                        "    strb wzr, [x9]\n"
                    ),
                )],
                needles: &["rule M6"],
            },
        },
    },
    Attempt {
        family: Family::Writes,
        what: "a changed return address",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::NotApplicable(
                "x86-64 keeps a return address in memory, where `call` pushes it, and not in a \
                 register a function could change: the next attempt writes it there",
            ),
            aarch64: Cell::Try {
                edits: &[("write_all:\n", "write_all:\n    mov x30, xzr\n")],
                needles: &["return address", "rule M3", "rule 1", "rule 3", "dialect"],
            },
        },
    },
    Attempt {
        family: Family::Writes,
        what: "a write over the saved return address",
        expect: Expect::Refused,
        cells: PerCpu {
            // write_all's return address is at [rsp] when it starts.
            x86_64: Cell::Try {
                edits: &[("write_all:\n", "write_all:\n    mov qword ptr [rsp], 0\n")],
                needles: &["rule M3", "rule 3"],
            },
            // The return address a function saved in its frame record, written through memory.
            aarch64: Cell::Try {
                edits: &[(
                    "write_i64:\n    stp x29, x30, [sp, #-16]!\n",
                    "write_i64:\n    stp x29, x30, [sp, #-16]!\n    str xzr, [sp, #8]\n",
                )],
                needles: &["return address", "rule M3"],
            },
        },
    },
    Attempt {
        family: Family::Writes,
        what: "a read below the stack pointer",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "write_all:\n",
                    "write_all:\n    mov rax, qword ptr [rsp - 8]\n",
                )],
                needles: &["rule M3", "rule 3"],
            },
            aarch64: Cell::Try {
                edits: &[("write_all:\n", "write_all:\n    ldr x9, [sp, #-8]\n")],
                needles: &["rule M3", "rule 3"],
            },
        },
    },
    // Loops the rules can't count.
    Attempt {
        family: Family::Loops,
        what: "a bound below the counter",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    ".Lwrite_all_loop:  # bound: 64\n",
                    ".Lwrite_all_loop:  # bound: 63\n",
                )],
                needles: &["rule 2"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    ".Lwrite_all_loop:  // bound: 64\n",
                    ".Lwrite_all_loop:  // bound: 63\n",
                )],
                needles: &["rule 2"],
            },
        },
    },
    Attempt {
        family: Family::Loops,
        what: "a loop with no counter",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    dec r8\n    jnz .Lwrite_all_loop\n",
                    "    jmp .Lwrite_all_loop\n",
                )],
                needles: &["rule 2"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    subs x10, x10, #1\n    b.ne .Lwrite_all_loop\n",
                    "    b .Lwrite_all_loop\n",
                )],
                needles: &["rule 2"],
            },
        },
    },
    Attempt {
        family: Family::Loops,
        what: "a counter reset in the body",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    add rsi, rax\n    sub rdx, rax\n    jz .Lwrite_all_done\n    dec r8\n",
                    concat!(
                        "    add rsi, rax\n",
                        "    sub rdx, rax\n",
                        "    jz .Lwrite_all_done\n",
                        "    mov r8, MAX_WRITES\n",
                        "    dec r8\n"
                    ),
                )],
                needles: &["rule 2"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    subs x2, x2, x0\n    b.eq .Lwrite_all_done\n    subs x10, x10, #1\n",
                    concat!(
                        "    subs x2, x2, x0\n",
                        "    b.eq .Lwrite_all_done\n",
                        "    mov x10, #MAX_WRITES\n",
                        "    subs x10, x10, #1\n"
                    ),
                )],
                needles: &["rule 2"],
            },
        },
    },
    Attempt {
        family: Family::Loops,
        what: "a backward jump that isn't a loop's end",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov edi, EXIT_WRITE_FAILED\n.Lstart_exit:\n",
                    concat!(
                        "    mov edi, EXIT_WRITE_FAILED\n",
                        "    jmp .Lstart_usage\n",
                        ".Lstart_exit:\n"
                    ),
                )],
                needles: &["rule 2", "rule 1"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    mov w0, #EXIT_WRITE_FAILED\n.Lstart_exit:\n",
                    concat!(
                        "    mov w0, #EXIT_WRITE_FAILED\n",
                        "    b .Lstart_usage\n",
                        ".Lstart_exit:\n"
                    ),
                )],
                needles: &["rule 2", "rule 1"],
            },
        },
    },
    Attempt {
        family: Family::Loops,
        what: "a loop with no bound",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(".Lwrite_all_loop:  # bound: 64\n", ".Lwrite_all_loop:\n")],
                needles: &["rule 2"],
            },
            aarch64: Cell::Try {
                edits: &[(".Lwrite_all_loop:  // bound: 64\n", ".Lwrite_all_loop:\n")],
                needles: &["rule 2"],
            },
        },
    },
    // Recursion, a call through a register and a function that doesn't return.
    Attempt {
        family: Family::Calls,
        what: "recursion",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[("write_all:\n", "write_all:\n    call write_all\n")],
                needles: &["rule 1", "rule 4"],
            },
            aarch64: Cell::Try {
                edits: &[("write_all:\n", "write_all:\n    bl write_all\n")],
                needles: &["rule 1", "rule 4", "rule 3"],
            },
        },
    },
    Attempt {
        family: Family::Calls,
        what: "a call through a register",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov rdi, qword ptr [rsp + 16]\n",
                    concat!(
                        "    lea rax, [rip + parse_i64]\n",
                        "    call rax\n",
                        "    mov rdi, qword ptr [rsp + 16]\n"
                    ),
                )],
                needles: &["rule 9", "dialect", "rule M1", "rule 1"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lstart_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lstart_assert_argv\n",
                        "    adrp x9, parse_i64\n",
                        "    add x9, x9, :lo12:parse_i64\n",
                        "    blr x9\n"
                    ),
                )],
                needles: &["rule 9", "dialect", "rule 1"],
            },
        },
    },
    Attempt {
        family: Family::Calls,
        what: "a function with no return",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    ".Lwrite_all_return:\n    ret\n",
                    ".Lwrite_all_return:\n    nop\n",
                )],
                needles: &["rule 1", "rule 4"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    ".Lwrite_all_return:\n    ret\n",
                    ".Lwrite_all_return:\n    nop\n",
                )],
                needles: &["rule 1", "rule 4", "dialect"],
            },
        },
    },
    // The assertion handler must be the template's, word for word.
    Attempt {
        family: Family::Handler,
        what: "an assertion handler that exits with 0",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov eax, 231\n    mov edi, 134\n    syscall\n",
                    "    mov eax, 231\n    mov edi, 0\n    syscall\n",
                )],
                needles: &["assert_fail", "rule 5"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    mov x0, #134\n    mov x8, #94\n    svc #0\n",
                    "    mov x0, #0\n    mov x8, #94\n    svc #0\n",
                )],
                needles: &["assert_fail", "rule 5"],
            },
        },
    },
    Attempt {
        family: Family::Handler,
        what: "an assertion message that says passed",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    .ascii \"lengua: assertion 000 failed\\n\"\n",
                    "    .ascii \"lengua: assertion 000 passed\\n\"\n",
                )],
                needles: &["assert_message", "assert_fail", "rule 5"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    .ascii \"lengua: assertion 000 failed\\n\"\n",
                    "    .ascii \"lengua: assertion 000 passed\\n\"\n",
                )],
                needles: &["assert_message", "assert_fail", "rule 5"],
            },
        },
    },
    // Code that ignores a failed result.
    Attempt {
        family: Family::Results,
        what: "a result used before it's tested",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    call parse_i64\n    test rax, rax\n    js .Lstart_not_a_number\n",
                    concat!(
                        "    call parse_i64\n",
                        "    mov rbx, rax\n",
                        "    test rax, rax\n",
                        "    js .Lstart_not_a_number\n"
                    ),
                )],
                needles: &["rule 7"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    bl parse_i64\n    tbnz x0, #63, .Lstart_not_a_number\n",
                    concat!(
                        "    bl parse_i64\n",
                        "    mov x9, x0\n",
                        "    tbnz x0, #63, .Lstart_not_a_number\n"
                    ),
                )],
                needles: &["rule 7"],
            },
        },
    },
    Attempt {
        family: Family::Results,
        what: "a write result used first",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov eax, SYS_WRITE\n    syscall\n    test rax, rax\n    js .Lwrite_all_return\n",
                    concat!(
                        "    mov eax, SYS_WRITE\n",
                        "    syscall\n",
                        "    mov rbx, rax\n",
                        "    test rax, rax\n",
                        "    js .Lwrite_all_return\n"
                    ),
                )],
                needles: &["rule 7"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    mov x8, #SYS_WRITE\n    svc #0\n    tbnz x0, #63, .Lwrite_all_return\n",
                    concat!(
                        "    mov x8, #SYS_WRITE\n",
                        "    svc #0\n",
                        "    mov x11, x0\n",
                        "    tbnz x0, #63, .Lwrite_all_return\n"
                    ),
                )],
                needles: &["rule 7"],
            },
        },
    },
    // The negative of a number comes out one too big when it is above 2^62: a part of the range
    // the tests' samples may miss and the proof covers.
    Attempt {
        family: Family::Proof,
        what: "a wrong program the samples may miss",
        expect: Expect::ProofFindsIt,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[
                    (
                        ".equ OUT_BUF_LEN, 21\n",
                        ".equ OUT_BUF_LEN, 21\n.equ BIG, 0x4000000000000000\n",
                    ),
                    (
                        "    neg rdx\n    jo .Lstart_assert_negate\n",
                        concat!(
                            "    neg rdx\n",
                            "    jo .Lstart_assert_negate\n",
                            "    mov rcx, BIG\n",
                            "    cmp rdx, rcx\n",
                            "    jle .Lstart_fine\n",
                            "    inc rdx\n",
                            ".Lstart_fine:\n"
                        ),
                    ),
                ],
                needles: &[],
            },
            aarch64: Cell::Try {
                edits: &[
                    (
                        ".equ OUT_BUF_LEN, 21\n",
                        ".equ OUT_BUF_LEN, 21\n.equ BIG, 0x4000000000000000\n",
                    ),
                    (
                        "    negs x0, x1\n    b.vs .Lstart_assert_negate\n",
                        concat!(
                            "    negs x0, x1\n",
                            "    b.vs .Lstart_assert_negate\n",
                            "    mov x9, #BIG\n",
                            "    cmp x0, x9\n",
                            "    b.le .Lstart_fine\n",
                            "    add x0, x0, #1\n",
                            ".Lstart_fine:\n"
                        ),
                    ),
                ],
                needles: &[],
            },
        },
    },
];

/// The attempts at system calls, one per call of [`CALLS`] and the calls only one CPU has.
fn call_attempts() -> Vec<Attempt> {
    let mut attempts: Vec<Attempt> = CALLS
        .iter()
        .map(|&(what, x86_64, aarch64)| Attempt {
            family: Family::SystemCalls,
            what,
            expect: Expect::Refused,
            cells: PerCpu {
                x86_64: Cell::Call(x86_64),
                aarch64: Cell::Call(aarch64),
            },
        })
        .collect();
    attempts.push(Attempt {
        family: Family::SystemCalls,
        what: "open",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Call(2),
            aarch64: Cell::NotApplicable(
                "AArch64 Linux has no `open`: its table keeps only `openat`, which is its own \
                 attempt",
            ),
        },
    });
    attempts.push(Attempt {
        family: Family::SystemCalls,
        what: "fork",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Call(57),
            aarch64: Cell::NotApplicable(
                "AArch64 Linux has no `fork`: its table keeps only `clone`, which is its own \
                 attempt",
            ),
        },
    });
    attempts
}

/// Every attempt, in the table's order.
fn attempts() -> Vec<Attempt> {
    call_attempts()
        .into_iter()
        .chain(ATTEMPTS.iter().copied())
        .collect()
}

fn asset(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugin/skills/compile/assets")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// Whether any of `lines` contains `needle`.
fn any_line(lines: &Value, needle: &str) -> bool {
    lines
        .as_array()
        .is_some_and(|lines| lines.iter().any(|line| text(line).contains(needle)))
}

/// `target`'s template changed by `edits`, in order: each `from`, which must be in the template
/// as the edits before it left it, becomes `to` once.
fn changed<'a>(target: Profile, edits: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let mut code = asset(TEMPLATES.get(target));
    for (from, to) in edits {
        assert!(
            code.contains(from),
            "the {} template has no {from:?}",
            target.name()
        );
        code = code.replacen(from, to, 1);
    }
    code
}

/// What `target`'s cell asks of `check`: the program to check, and what a refusal must name. None
/// where the attempt can't be written on that CPU.
fn planned(target: Profile, cell: Cell) -> Option<(String, &'static [&'static str])> {
    match cell {
        Cell::NotApplicable(_) => None,
        Cell::Try { edits, needles } => Some((changed(target, edits.iter().copied()), needles)),
        Cell::Call(number) => {
            let exit = *EXITS.get(target);
            let call = match target {
                Profile::LinuxX86_64 => format!("    mov eax, {number}\n    syscall\n"),
                Profile::LinuxAarch64 => format!("    mov x8, #{number}\n    svc #0\n"),
            };
            let replacement = format!("{call}{exit}");
            Some((
                changed(target, [(exit, replacement.as_str())]),
                CALL_NEEDLES,
            ))
        }
    }
}

/// A program folder holding the template's spec, recorded.
async fn checked_template(session: &Session) -> Program {
    let program = Program::new();
    program.write("spec.md", &asset("template-spec.md"));
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    program
}

/// Makes the attempt on `target`'s session and holds `check` to what the attempt expects.
async fn try_attempt(
    session: &Session,
    target: Profile,
    attempt: &Attempt,
    code: &str,
    needles: &[&str],
) {
    let what = format!("{} on {}", attempt.what, target.name());
    let program = checked_template(session).await;
    program.write("main.s", code);
    match attempt.expect {
        Expect::Refused => {
            let report = session.check(&program).await;
            assert_eq!(
                report["state"], "code_problems",
                "{what} wasn't refused: {report}"
            );
            assert!(
                needles
                    .iter()
                    .any(|needle| any_line(&report["problems"], needle)),
                "{what} was refused, but not for {needles:?}: {report}"
            );
        }
        Expect::NeverDone => {
            // The sandbox kills the stray read, and the program is never stamped done.
            let report = session.check(&program).await;
            assert_ne!(report["state"], "done", "{what} was stamped done: {report}");
            assert!(
                any_line(&report["problems"], "SIGSEGV") || report["state"] == "code_problems",
                "{what} wasn't stopped: {report}"
            );
        }
        Expect::TestedNotProven => {
            // A pointer that happens to be good (argv[0], put there by the kernel) isn't wrong,
            // but the rules can't follow it to an owner: the program may be done, and then the
            // report must say that memory is tested, not proven.
            let report = session.check(&program).await;
            assert!(
                report["memory"]
                    .as_str()
                    .is_some_and(|memory| memory.contains("tested, not proven")),
                "{what} was reported as proven: {report}"
            );
        }
        Expect::ProofFindsIt => {
            let report = session.settle(&program).await;
            assert_eq!(
                report["state"], "tests_failed",
                "{what} got past the tests and the proof: {report}"
            );
        }
    }
}

/// Tries every attempt of `family` on every target this computer runs.
async fn run_family(family: Family) {
    let attempts: Vec<Attempt> = attempts()
        .into_iter()
        .filter(|attempt| attempt.family == family)
        .collect();
    assert!(!attempts.is_empty(), "no attempts in {family:?}");
    let proves = attempts
        .iter()
        .any(|attempt| attempt.expect == Expect::ProofFindsIt);
    // The proof takes the solvers most of their time, so no other proof runs beside it.
    if proves {
        let installed = std::process::Command::new("z3")
            .arg("--version")
            .output()
            .is_ok();
        if !installed {
            lengua::skiplog::note("z3 isn't installed");
            return;
        }
    }
    for target in Profile::ALL {
        let session = if proves {
            per_target::proving_session(target, Protocol::Discover, 300).await
        } else {
            per_target::session(target, Protocol::Discover).await
        };
        let Some(session) = session else {
            continue;
        };
        for attempt in &attempts {
            if let Some((code, needles)) = planned(target, *attempt.cells.get(target)) {
                try_attempt(&session, target, attempt, &code, needles).await;
            }
        }
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn a_system_call_the_spec_doesnt_allow_is_refused() {
    run_family(Family::SystemCalls).await;
}

#[tokio::test]
async fn a_system_call_chosen_by_a_value_the_rules_cant_see_is_refused() {
    run_family(Family::CallChoice).await;
}

#[tokio::test]
async fn accesses_outside_their_owner_are_refused() {
    run_family(Family::Accesses).await;
}

#[tokio::test]
async fn an_address_made_from_a_plain_number_is_refused() {
    run_family(Family::Addresses).await;
}

#[tokio::test]
async fn writing_read_only_data_or_a_return_address_is_refused() {
    run_family(Family::Writes).await;
}

#[tokio::test]
async fn loops_the_rules_cant_count_are_refused() {
    run_family(Family::Loops).await;
}

#[tokio::test]
async fn recursion_calls_through_registers_and_a_missing_return_are_refused() {
    run_family(Family::Calls).await;
}

#[tokio::test]
async fn a_tampered_assertion_handler_is_refused() {
    run_family(Family::Handler).await;
}

#[tokio::test]
async fn code_that_ignores_a_failed_result_is_refused() {
    run_family(Family::Results).await;
}

#[tokio::test]
async fn a_wrong_program_is_never_stamped_done() {
    run_family(Family::Proof).await;
}

/// The table is well formed on any computer, even for a target it cannot run: every edit's text
/// is in the template as the edits before it left it, a cell that can't be written says why, and
/// no target tries fewer attempts than the two files this table replaced did.
#[test]
fn every_cell_edits_text_its_template_has() {
    let mut tried = PerCpu {
        x86_64: 0,
        aarch64: 0,
    };
    for attempt in attempts() {
        let mut applies = false;
        for (target, cell) in attempt.cells.cells() {
            match planned(target, *cell) {
                Some((code, _)) => {
                    assert_ne!(code, asset(TEMPLATES.get(target)), "{}", attempt.what);
                    applies = true;
                    match target {
                        Profile::LinuxX86_64 => tried.x86_64 += 1,
                        Profile::LinuxAarch64 => tried.aarch64 += 1,
                    }
                }
                None => {
                    let Cell::NotApplicable(why) = cell else {
                        panic!("{}: a cell with no program", attempt.what);
                    };
                    assert!(
                        why.len() > 40,
                        "{}: say why {} can't",
                        attempt.what,
                        target.name()
                    );
                }
            }
        }
        assert!(applies, "{}: no target tries it", attempt.what);
    }
    // The 41 attempts each file held, and the x86-64 `openat` this table adds.
    assert!(tried.x86_64 >= 42, "{} on x86-64", tried.x86_64);
    assert!(tried.aarch64 >= 41, "{} on ARM64", tried.aarch64);
}
