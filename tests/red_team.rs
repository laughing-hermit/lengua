//! The red team: programs written to get past lengua's own checks, on every target. Each attempt
//! is the target's template with one change that tries it, and `check` must refuse it up front:
//! a system call of main.s's own, an operation the spec doesn't allow, a name lengua keeps (a
//! function, a label, an assertion handler or its message), a jump or call that lands anywhere but
//! the start of an operation or a function, lengua's data read or written, an operation called
//! through a register, an exit status or an assertion number lengua keeps for itself, an access
//! past a buffer, an unchecked or late index, an address made from a plain number, a write to
//! read-only data, a return address changed or written over, a read below the stack pointer, a
//! loop the rules miscount, a stray backward jump, recursion, a call through a register, a
//! missing `ret`, and an ignored result.
//! Two attempts no rule can follow are held to what does hold: a pointer read from memory is
//! never stamped `done` when it is wrong and never reported proven when it is good, and a wrong
//! program is never stamped `done` once the proof has run. None is ever run when a rule refuses
//! it first. Any attempt that isn't refused is a bug in lengua to fix before anything else.
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
    /// Ways to the OS other than an operation the spec allows, and into lengua's own code, data
    /// and names.
    Edge,
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

/// A function of main.s's own named for one of lengua's operations, after `main`: `ret` is the
/// same on both CPUs.
const OWN_OS_WRITE: &[Edit] = &[(
    ".size main, . - main\n",
    concat!(
        ".size main, . - main\n\n",
        ".type os_write, @function\n",
        "os_write:\n",
        "    ret\n",
        ".size os_write, . - os_write\n"
    ),
)];

/// A function of main.s's own named with lengua's prefix, after `main`: `ret` is the same on both
/// CPUs.
const OWN_LENGUA_ERROR: &[Edit] = &[(
    ".size main, . - main\n",
    concat!(
        ".size main, . - main\n\n",
        ".type lengua_error, @function\n",
        "lengua_error:\n",
        "    ret\n",
        ".size lengua_error, . - lengua_error\n"
    ),
)];

/// A label named with lengua's prefix, in `main` right before its exit.
const OWN_LENGUA_LABEL: &[Edit] = &[(".Lmain_exit:\n", ".Llengua_exit:\n.Lmain_exit:\n")];

/// A data symbol of main.s's own named `assert_message`, which says an assertion passed, after
/// `nan_msg` in the read-only data: data is the same on both CPUs.
const OWN_ASSERT_MESSAGE: &[Edit] = &[(
    ".section .bss\n",
    concat!(
        ".type assert_message, @object\n",
        "assert_message:\n",
        "    .ascii \"lengua: assertion 000 passed\"\n",
        ".size assert_message, . - assert_message\n\n",
        ".section .bss\n"
    ),
)];

/// The exit status of a failed write set to 134, which assertions use, through its constant.
const EXIT_AS_ASSERTION: &[Edit] = &[(
    ".equ EXIT_WRITE_FAILED, 3\n",
    ".equ EXIT_WRITE_FAILED, 134\n",
)];

/// Every attempt.
const ATTEMPTS: &[Attempt] = &[
    // Ways to the OS other than an operation the spec allows. main.s reaches the OS by calling the
    // operations in lengua's text, which is assembled after it: it makes no system call of its
    // own, calls no operation its spec doesn't name, defines none of lengua's names, and enters
    // lengua's code only by a call to an operation or to a library function.
    Attempt {
        family: Family::Edge,
        what: "a system call of its own",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[("    call os_exit\n", "    syscall\n    call os_exit\n")],
                needles: &["makes a system call by itself"],
            },
            aarch64: Cell::Try {
                edits: &[("    bl os_exit\n", "    svc #0\n    bl os_exit\n")],
                needles: &["makes a system call by itself"],
            },
        },
    },
    Attempt {
        family: Family::Edge,
        what: "an operation the spec doesn't allow",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[("    call os_exit\n", "    call os_open\n    call os_exit\n")],
                needles: &["isn't one the spec allows"],
            },
            aarch64: Cell::Try {
                edits: &[("    bl os_exit\n", "    bl os_open\n    bl os_exit\n")],
                needles: &["isn't one the spec allows"],
            },
        },
    },
    Attempt {
        family: Family::Edge,
        what: "an os_write of its own",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: OWN_OS_WRITE,
                needles: &["is a name lengua keeps"],
            },
            aarch64: Cell::Try {
                edits: OWN_OS_WRITE,
                needles: &["is a name lengua keeps"],
            },
        },
    },
    // A jump to an operation the spec doesn't allow, which is no call of it for the rule on
    // operations to see.
    Attempt {
        family: Family::Edge,
        what: "a jump into lengua's code",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[("    call os_exit\n", "    jmp os_open\n    call os_exit\n")],
                needles: &["jumps outside main"],
            },
            aarch64: Cell::Try {
                edits: &[("    bl os_exit\n", "    b os_open\n    bl os_exit\n")],
                needles: &["jumps outside main"],
            },
        },
    },
    // A jump or call that lands anywhere but the start of a function or an operation: at a label
    // inside an operation (os_write's), at a label of another function, at a label of main's own,
    // or just past an operation's first instruction (os_exit's says which system call it makes).
    // The first two name no label of main and no function; the third calls no function's start;
    // the fourth is a sum, which x86-64's operands have no place for and which names no label on
    // AArch64.
    Attempt {
        family: Family::Edge,
        what: "a jump into the middle of an operation",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    call os_exit\n",
                    "    jmp .Llengua_write_refused\n    call os_exit\n",
                )],
                needles: &["isn't a label"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    bl os_exit\n",
                    "    b .Llengua_write_refused\n    bl os_exit\n",
                )],
                needles: &["isn't a label"],
            },
        },
    },
    Attempt {
        family: Family::Edge,
        what: "a jump into another function's label",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    call os_exit\n",
                    "    jmp .Lwrite_all_return\n    call os_exit\n",
                )],
                needles: &["isn't a label"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    bl os_exit\n",
                    "    b .Lwrite_all_return\n    bl os_exit\n",
                )],
                needles: &["isn't a label"],
            },
        },
    },
    Attempt {
        family: Family::Edge,
        what: "a call to a label of main's own",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    call os_exit\n",
                    "    call .Lmain_not_a_number\n    call os_exit\n",
                )],
                needles: &["doesn't call the start of a function"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    bl os_exit\n",
                    "    bl .Lmain_not_a_number\n    bl os_exit\n",
                )],
                needles: &["doesn't call the start of a function"],
            },
        },
    },
    Attempt {
        family: Family::Edge,
        what: "a call just past an operation's first instruction",
        expect: Expect::Refused,
        cells: PerCpu {
            // `mov eax, LENGUA_SYS_EXIT_GROUP` is 5 bytes: the `syscall` after it would make the
            // system call the caller left the number of in eax.
            x86_64: Cell::Try {
                edits: &[(
                    "    call os_exit\n",
                    "    call os_exit + 5\n    call os_exit\n",
                )],
                needles: &["isn't a constant defined above"],
            },
            // `mov x8, #LENGUA_SYS_EXIT_GROUP` is 4 bytes: the `svc #0` after it would make the
            // system call the caller left the number of in x8.
            aarch64: Cell::Try {
                edits: &[("    bl os_exit\n", "    bl os_exit + 4\n    bl os_exit\n")],
                needles: &["isn't a label"],
            },
        },
    },
    // lengua's data is lengua's, though it is assembled beside main.s: main.s names none of it,
    // neither the error table of an operation it calls, which it reads here, nor the assertion
    // message and the buffer of the channel to lengua's broker, which it writes (a program that
    // opens files links the buffer, so that attempt calls os_open too, which the data is refused
    // before).
    Attempt {
        family: Family::Edge,
        what: "a read of lengua's error table",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov rdi, qword ptr [rsi + 8]\n",
                    concat!(
                        "    mov rax, qword ptr [rip + os_write_errors]\n",
                        "    mov rdi, qword ptr [rsi + 8]\n"
                    ),
                )],
                needles: &["isn't a data symbol"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lmain_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lmain_assert_argv\n",
                        "    adrp x9, os_write_errors\n",
                        "    add x9, x9, :lo12:os_write_errors\n",
                        "    ldr x10, [x9]\n"
                    ),
                )],
                needles: &["isn't a data symbol"],
            },
        },
    },
    Attempt {
        family: Family::Edge,
        what: "a write to lengua's assertion message",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov rdi, qword ptr [rsi + 8]\n",
                    concat!(
                        "    mov byte ptr [rip + assert_message], 0\n",
                        "    mov rdi, qword ptr [rsi + 8]\n"
                    ),
                )],
                needles: &["isn't a data symbol"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lmain_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lmain_assert_argv\n",
                        "    adrp x9, assert_message\n",
                        "    add x9, x9, :lo12:assert_message\n",
                        "    strb wzr, [x9]\n"
                    ),
                )],
                needles: &["isn't a data symbol"],
            },
        },
    },
    Attempt {
        family: Family::Edge,
        what: "a write to lengua's channel buffer",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[
                    ("    call os_exit\n", "    call os_open\n    call os_exit\n"),
                    (
                        "    mov rdi, qword ptr [rsi + 8]\n",
                        concat!(
                            "    mov byte ptr [rip + lengua_wire], 0\n",
                            "    mov rdi, qword ptr [rsi + 8]\n"
                        ),
                    ),
                ],
                needles: &["isn't a data symbol"],
            },
            aarch64: Cell::Try {
                edits: &[
                    ("    bl os_exit\n", "    bl os_open\n    bl os_exit\n"),
                    (
                        "    cbz x0, .Lmain_assert_argv\n",
                        concat!(
                            "    cbz x0, .Lmain_assert_argv\n",
                            "    adrp x9, lengua_wire\n",
                            "    add x9, x9, :lo12:lengua_wire\n",
                            "    strb wzr, [x9]\n"
                        ),
                    ),
                ],
                needles: &["isn't a data symbol"],
            },
        },
    },
    // An operation called through a register. x86-64 makes no address of an operation (`lea` takes
    // a data symbol's), and its call through a register is refused on its own; AArch64 makes one
    // with adrp and add, and its `blr` is refused.
    Attempt {
        family: Family::Edge,
        what: "an operation called through a register",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov rdi, qword ptr [rsi + 8]\n",
                    concat!(
                        "    lea rax, [rip + os_write]\n",
                        "    call rax\n",
                        "    mov rdi, qword ptr [rsi + 8]\n"
                    ),
                )],
                needles: &["jumps through a register or memory"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lmain_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lmain_assert_argv\n",
                        "    adrp x9, os_write\n",
                        "    add x9, x9, :lo12:os_write\n",
                        "    blr x9\n"
                    ),
                )],
                needles: &["calls through a register"],
            },
        },
    },
    // What lengua keeps for itself: 134, the exit status of a failed assertion, which no named
    // constant may give as a status either, and the assertion numbers of lengua's own library
    // (901 to 903 and 990 to 992), above the 899 a program numbers its own.
    Attempt {
        family: Family::Edge,
        what: "an exit status of 134",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: EXIT_AS_ASSERTION,
                needles: &["or to 134, which assertions use"],
            },
            aarch64: Cell::Try {
                edits: EXIT_AS_ASSERTION,
                needles: &["or to 134, which assertions use"],
            },
        },
    },
    Attempt {
        family: Family::Edge,
        what: "an assertion number lengua keeps for its own code",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[("    mov edi, 2\n", "    mov edi, 990\n")],
                needles: &["one lengua keeps for its own code"],
            },
            aarch64: Cell::Try {
                edits: &[("    mov w0, #2\n", "    mov w0, #990\n")],
                needles: &["one lengua keeps for its own code"],
            },
        },
    },
    // lengua's names are lengua's. main.s has no assertion handler of its own (one that exits with
    // 0 would make every failed assertion pass) and no message for one that says passed, and
    // nothing with lengua's prefix: `lengua_` for a function, `.Llengua_` for a label.
    Attempt {
        family: Family::Edge,
        what: "an assertion handler of its own that exits with 0",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    ".size main, . - main\n",
                    concat!(
                        ".size main, . - main\n\n",
                        ".type assert_fail, @function\n",
                        "assert_fail:\n",
                        "    mov edi, EXIT_OK\n",
                        "    call os_exit\n",
                        ".size assert_fail, . - assert_fail\n"
                    ),
                )],
                needles: &["assert_fail is a name lengua keeps"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    ".size main, . - main\n",
                    concat!(
                        ".size main, . - main\n\n",
                        ".type assert_fail, @function\n",
                        "assert_fail:\n",
                        "    mov w0, #EXIT_OK\n",
                        "    bl os_exit\n",
                        ".size assert_fail, . - assert_fail\n"
                    ),
                )],
                needles: &["assert_fail is a name lengua keeps"],
            },
        },
    },
    Attempt {
        family: Family::Edge,
        what: "an assertion message of its own that says passed",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: OWN_ASSERT_MESSAGE,
                needles: &["assert_message is a name lengua keeps"],
            },
            aarch64: Cell::Try {
                edits: OWN_ASSERT_MESSAGE,
                needles: &["assert_message is a name lengua keeps"],
            },
        },
    },
    Attempt {
        family: Family::Edge,
        what: "a function named with lengua's prefix",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: OWN_LENGUA_ERROR,
                needles: &["lengua_error is a name lengua keeps"],
            },
            aarch64: Cell::Try {
                edits: OWN_LENGUA_ERROR,
                needles: &["lengua_error is a name lengua keeps"],
            },
        },
    },
    Attempt {
        family: Family::Edge,
        what: "a label named with lengua's prefix",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: OWN_LENGUA_LABEL,
                needles: &[".Llengua_exit is a name lengua keeps"],
            },
            aarch64: Cell::Try {
                edits: OWN_LENGUA_LABEL,
                needles: &[".Llengua_exit is a name lengua keeps"],
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
                    "    mov rdi, qword ptr [rsi + 8]\n",
                    concat!(
                        "    movzx ecx, byte ptr [rip + usage_msg + 18]\n",
                        "    mov rdi, qword ptr [rsi + 8]\n"
                    ),
                )],
                needles: &["rule M2", "past the end of"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lmain_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lmain_assert_argv\n",
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
                    "    mov rdi, qword ptr [rsi + 8]\n",
                    concat!(
                        "    movzx ecx, byte ptr [rip + usage_msg + 4096]\n",
                        "    mov rdi, qword ptr [rsi + 8]\n"
                    ),
                )],
                needles: &["rule M2", "past the end of"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lmain_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lmain_assert_argv\n",
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
                    "    mov rdi, qword ptr [rsi + 8]\n    test rdi, rdi\n",
                    concat!(
                        "    mov rdi, qword ptr [rsi + 8]\n",
                        "    mov rcx, qword ptr [rsi]\n",
                        "    movzx edx, byte ptr [rdi + rcx]\n",
                        "    test rdi, rdi\n"
                    ),
                )],
                needles: &["rule 9", "rule M2"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lmain_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lmain_assert_argv\n",
                        "    ldr x11, [x1]\n",
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
                    "    mov rdi, qword ptr [rsi + 8]\n    test rdi, rdi\n",
                    concat!(
                        "    mov rdi, qword ptr [rsi + 8]\n",
                        "    mov rcx, qword ptr [rsi]\n",
                        "    movzx edx, byte ptr [rdi + rcx]\n",
                        "    cmp rcx, 8\n",
                        "    jae .Lmain_assert_argv\n",
                        "    test rdi, rdi\n"
                    ),
                )],
                needles: &["rule 9", "rule M2"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lmain_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lmain_assert_argv\n",
                        "    ldr x11, [x1]\n",
                        "    ldrb w10, [x0, x11]\n",
                        "    cmp x11, #8\n",
                        "    b.hs .Lmain_assert_argv\n"
                    ),
                )],
                needles: &["rule 9", "rule M2"],
            },
        },
    },
    // An address made from a plain number, and pointers read from memory: a wrong one, which
    // the sandbox stops, and a good one, which the report says is tested, not proven.
    Attempt {
        family: Family::Addresses,
        what: "a fixed address",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[(
                    "    mov rdi, qword ptr [rsi + 8]\n",
                    concat!(
                        "    mov eax, 4096\n",
                        "    movzx ecx, byte ptr [rax]\n",
                        "    mov rdi, qword ptr [rsi + 8]\n"
                    ),
                )],
                needles: &["rule M1"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lmain_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lmain_assert_argv\n",
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
        what: "a page address used without its offset",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::NotApplicable(
                "x86-64 has no instruction that makes a page address: `lea` of `[rip + symbol]` \
                 makes the whole address, whose offset the rules follow",
            ),
            // The page of out_buf holds more than out_buf: a store 64 bytes in reaches whatever
            // is there, with no add that names where the pointer points.
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lmain_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lmain_assert_argv\n",
                        "    adrp x9, out_buf\n",
                        "    strb wzr, [x9, #64]\n"
                    ),
                )],
                needles: &["isn't followed right away by `add`"],
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
                    "    mov rdi, qword ptr [rsi + 8]\n",
                    concat!(
                        "    lea rax, [rip + usage_msg]\n",
                        "    mov rdx, qword ptr [rax]\n",
                        "    movzx ecx, byte ptr [rdx]\n",
                        "    mov rdi, qword ptr [rsi + 8]\n"
                    ),
                )],
                needles: &[],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lmain_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lmain_assert_argv\n",
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
                    "    mov rdi, qword ptr [rsi + 8]\n",
                    concat!(
                        "    mov rdx, qword ptr [rsi]\n",
                        "    movzx ecx, byte ptr [rdx]\n",
                        "    mov rdi, qword ptr [rsi + 8]\n"
                    ),
                )],
                needles: &[],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lmain_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lmain_assert_argv\n",
                        "    ldr x9, [x1]\n",
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
                    "    mov rdi, qword ptr [rsi + 8]\n",
                    concat!(
                        "    mov byte ptr [rip + usage_msg], 0\n",
                        "    mov rdi, qword ptr [rsi + 8]\n"
                    ),
                )],
                needles: &["rule M6", "read-only"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lmain_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lmain_assert_argv\n",
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
        what: "a pop of memory over the saved return address",
        expect: Expect::Refused,
        cells: PerCpu {
            // write_all's return address is at [rsp] when it starts: the push makes room, and the
            // pop's address is that of [rsp] after rsp has moved back up, so it is the return
            // address.
            x86_64: Cell::Try {
                edits: &[(
                    "write_all:\n",
                    "write_all:\n    push rcx\n    pop qword ptr [rsp]\n",
                )],
                needles: &["of memory reaches memory no rule follows"],
            },
            aarch64: Cell::NotApplicable(
                "AArch64 has no push or pop: the return address a function saved in its frame \
                 record is written by an ordinary store, which the row above tries",
            ),
        },
    },
    Attempt {
        family: Family::Writes,
        what: "a stack pointer popped from the stack",
        expect: Expect::Refused,
        cells: PerCpu {
            x86_64: Cell::Try {
                edits: &[("write_all:\n", "write_all:\n    push rcx\n    pop rsp\n")],
                needles: &["loads the stack pointer from memory"],
            },
            aarch64: Cell::NotApplicable(
                "AArch64 has no pop: the ways to load sp from a register, `mov sp, x9` and \
                 `add sp, x9, #0`, are refused as setting sp",
            ),
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
                    "    mov edi, EXIT_WRITE_FAILED\n.Lmain_exit:\n",
                    concat!(
                        "    mov edi, EXIT_WRITE_FAILED\n",
                        "    jmp .Lmain_usage\n",
                        ".Lmain_exit:\n"
                    ),
                )],
                needles: &["rule 2", "rule 1"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    mov w0, #EXIT_WRITE_FAILED\n.Lmain_exit:\n",
                    concat!(
                        "    mov w0, #EXIT_WRITE_FAILED\n",
                        "    b .Lmain_usage\n",
                        ".Lmain_exit:\n"
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
                    "    mov rdi, qword ptr [rsi + 8]\n",
                    concat!(
                        "    lea rax, [rip + parse_i64]\n",
                        "    call rax\n",
                        "    mov rdi, qword ptr [rsi + 8]\n"
                    ),
                )],
                needles: &["rule 9", "dialect", "rule M1", "rule 1"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    cbz x0, .Lmain_assert_argv\n",
                    concat!(
                        "    cbz x0, .Lmain_assert_argv\n",
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
                    ".Lwrite_all_return:\n    add rsp, 8\n    ret\n",
                    ".Lwrite_all_return:\n    add rsp, 8\n    nop\n",
                )],
                needles: &["rule 1", "rule 4"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    ".Lwrite_all_return:\n    ldp x29, x30, [sp], #16\n    ret\n",
                    ".Lwrite_all_return:\n    ldp x29, x30, [sp], #16\n    nop\n",
                )],
                needles: &["rule 1", "rule 4", "dialect"],
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
                    "    call parse_i64\n    test rax, rax\n    js .Lmain_not_a_number\n",
                    concat!(
                        "    call parse_i64\n",
                        "    mov rbx, rax\n",
                        "    test rax, rax\n",
                        "    js .Lmain_not_a_number\n"
                    ),
                )],
                needles: &["rule 7"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    bl parse_i64\n    tbnz x0, #63, .Lmain_not_a_number\n",
                    concat!(
                        "    bl parse_i64\n",
                        "    mov x9, x0\n",
                        "    tbnz x0, #63, .Lmain_not_a_number\n"
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
                    "    call os_write\n    test rax, rax\n    js .Lwrite_all_return\n",
                    concat!(
                        "    call os_write\n",
                        "    mov rbx, rax\n",
                        "    test rax, rax\n",
                        "    js .Lwrite_all_return\n"
                    ),
                )],
                needles: &["rule 7"],
            },
            aarch64: Cell::Try {
                edits: &[(
                    "    bl os_write\n    tbnz x0, #63, .Lwrite_all_return\n",
                    concat!(
                        "    bl os_write\n",
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
                        "    neg rdx\n    jo .Lmain_assert_negate\n",
                        concat!(
                            "    neg rdx\n",
                            "    jo .Lmain_assert_negate\n",
                            "    mov rcx, BIG\n",
                            "    cmp rdx, rcx\n",
                            "    jle .Lmain_fine\n",
                            "    inc rdx\n",
                            ".Lmain_fine:\n"
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
                        "    negs x0, x1\n    b.vs .Lmain_assert_negate\n",
                        concat!(
                            "    negs x0, x1\n",
                            "    b.vs .Lmain_assert_negate\n",
                            "    mov x9, #BIG\n",
                            "    cmp x0, x9\n",
                            "    b.le .Lmain_fine\n",
                            "    add x0, x0, #1\n",
                            ".Lmain_fine:\n"
                        ),
                    ),
                ],
                needles: &[],
            },
        },
    },
];

/// The text of a file in the compile skill's assets.
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
            // A pointer that happens to be good (argv[0], put there by the kernel) isn't wrong:
            // the program may be done, and then the report must say that memory is tested, not
            // proven.
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
    let attempts: Vec<&Attempt> = ATTEMPTS
        .iter()
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
async fn the_os_is_reached_only_by_an_operation_the_spec_allows() {
    run_family(Family::Edge).await;
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
async fn code_that_ignores_a_failed_result_is_refused() {
    run_family(Family::Results).await;
}

#[tokio::test]
async fn a_wrong_program_is_never_stamped_done() {
    run_family(Family::Proof).await;
}

/// The table is well formed on any computer, even for a target it cannot run: every edit's text
/// is in the template as the edits before it left it, a cell that can't be written says why, and
/// no target tries fewer attempts than it does now.
#[test]
fn every_cell_edits_text_its_template_has() {
    let mut tried = PerCpu {
        x86_64: 0,
        aarch64: 0,
    };
    for attempt in ATTEMPTS {
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
    // Every attempt, but those one CPU alone can write: ARM64's changed return address and page
    // address, x86-64's two pops.
    assert!(tried.x86_64 >= 41, "{} on x86-64", tried.x86_64);
    assert!(tried.aarch64 >= 41, "{} on ARM64", tried.aarch64);
}
