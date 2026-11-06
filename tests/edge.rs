//! The edge's operations, one table of cases for both CPUs. Each case is a list of steps, calls of
//! the operations with the arguments they take, from which this file writes a small program for
//! each CPU in the edge form (it defines `main` and calls `os_*`), so that both CPUs are held to
//! the very same case. The program is built with lengua.s after it, run through the launcher under
//! the system calls its operations make, and held to what it prints and how it ends. A case runs
//! natively for the CPU this computer is, on that CPU's own runner in CI; for the other CPU it
//! runs through QEMU's user mode where that is set up, and in any case it is built, since LLVM
//! builds for both CPUs anywhere.
//!
//! A program keeps what each call returns in a scratch area it zeroes first, and prints the
//! first `report` bytes of it before it exits, so a case says what it expects as bytes.

mod common;
mod per_target;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use lengua::broker::Permit;
use lengua::platform::Profile;
use lengua::runner::{End, Job, Limits, run};
use lengua::idea::Syscall;
use per_target::Way;

fn launcher() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_lengua-mcp"))
}

/// The scratch area's size: 64 words.
const SCRATCH: u32 = 512;

/// Where `main`'s arguments are kept in the scratch area, its last three words.
const ARGC_AT: u32 = 488;
const ARGV_AT: u32 = 496;
const ENVP_AT: u32 = 504;

/// The registers an operation's arguments go in, on x86-64: the system call's, as r10 is the fourth.
const X86_ARGUMENTS: [&str; 6] = ["rdi", "rsi", "rdx", "r10", "r8", "r9"];

/// The registers a function's arguments go in, on x86-64: the fourth is rcx.
const X86_FUNCTION_ARGUMENTS: [&str; 6] = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"];

/// What an argument of a call is.
#[derive(Clone, Copy)]
enum Arg {
    /// A number.
    N(i64),
    /// The address of a data item.
    Data(&'static str),
    /// The address that many bytes into the scratch area.
    Scratch(u32),
    /// What an earlier call kept in that word of the scratch area.
    Kept(u32),
    /// `argv[k]`.
    Argv(u32),
    /// `envp[k]`.
    Envp(u32),
}

/// One step of a program.
#[derive(Clone, Copy)]
enum Step {
    /// Calls an operation, and keeps what it returns in a word of the scratch area.
    Call {
        op: &'static str,
        args: &'static [Arg],
        keep: Option<u32>,
    },
    /// Calls a library function (`fill`, `parallel_for`, `assert_fail`), whose arguments go in the
    /// registers of the CPU's calling convention, and keeps what it returns.
    Function {
        name: &'static str,
        args: &'static [Arg],
        keep: Option<u32>,
    },
    /// Stores a number, in 1, 2, 4 or 8 bytes, that many bytes into the scratch area.
    Set { offset: u32, bytes: u8, value: i64 },
}

use Arg::{Argv, Data, Envp, Kept, N, Scratch};
use Step::{Call, Function, Set};

/// One case.
struct Case {
    name: &'static str,
    /// Byte strings in read-only data, by name.
    data: &'static [(&'static str, &'static [u8])],
    steps: &'static [Step],
    /// How many bytes of the scratch area the program prints, from its start, when its steps are
    /// done, before it exits with `exit`.
    report: u32,
    exit: i64,
    /// What the run gives it.
    stdin: &'static [u8],
    /// What it prints to standard error, all of it, when the case says; anything when it doesn't.
    stderr: &'static [u8],
    arguments: &'static [&'static str],
    environment: &'static [(&'static str, &'static str)],
    files: &'static [(&'static str, &'static str)],
    folders: &'static [&'static str],
    /// The files the working folder holds afterwards, when the case says; none, when it doesn't.
    after: &'static [(&'static str, &'static str)],
    /// The most bytes a file may hold, when the case says; a test's limit, when it doesn't.
    file_limit: Option<u64>,
    /// The system calls its operations make, besides `write` and `exit_group`.
    calls: &'static [Syscall],
    /// Whether it starts threads.
    threads: bool,
    /// Why the prover's model of the operations doesn't follow it, when it doesn't: the case is
    /// then run natively and not in the model.
    model: Option<&'static str>,
    /// Assembly appended to the program for each CPU, such as a `worker`: x86-64's, then AArch64's.
    extra: (&'static str, &'static str),
    /// What it prints, all of it, the report included.
    stdout: Vec<u8>,
}

impl Case {
    /// A case with nothing given to the program, that exits with 0.
    fn new(name: &'static str, steps: &'static [Step], report: u32, stdout: Vec<u8>) -> Self {
        Self {
            name,
            data: &[],
            steps,
            report,
            exit: 0,
            stdin: b"",
            stderr: b"",
            arguments: &[],
            environment: &[],
            files: &[],
            folders: &[],
            after: &[],
            file_limit: None,
            calls: &[],
            threads: false,
            model: None,
            extra: ("", ""),
            stdout,
        }
    }
}

/// The eight bytes of a result, little-endian, as a program stores it.
fn word(value: i64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

/// The first `length` bytes of a scratch area that holds `words` from its start and the bytes
/// `at` the offsets, and zeros elsewhere.
fn image(length: usize, words: &[i64], at: &[(usize, &[u8])]) -> Vec<u8> {
    let mut bytes = vec![0_u8; length];
    for (index, value) in words.iter().enumerate() {
        bytes[8 * index..8 * index + 8].copy_from_slice(&value.to_le_bytes());
    }
    for (offset, found) in at {
        bytes[*offset..*offset + found.len()].copy_from_slice(found);
    }
    bytes
}

/// The data items as assembly.
fn data(case: &Case) -> String {
    let mut text = String::from(".section .rodata\n");
    for (name, bytes) in case.data {
        let list: Vec<String> = bytes.iter().map(u8::to_string).collect();
        let _ = writeln!(
            text,
            ".type {name}, @object\n{name}:\n    .byte {}\n.size {name}, . - {name}",
            list.join(", ")
        );
    }
    text
}

/// The registers an operation keeps on x86-64 that none of its arguments is in: each is given a
/// value before the call and must hold it after. A call changes rax, rcx, r11, the flags and the
/// vector registers, and no other register.
const X86_KEPT: [&str; 6] = ["rbx", "rbp", "r12", "r13", "r14", "r15"];

/// The same on AArch64: a call changes x0, x8, x16, x17, x30, the flags and the vector registers,
/// and no other register. x1 to x5 are checked as arguments.
const A64_KEPT: [&str; 21] = [
    "x6", "x7", "x9", "x10", "x11", "x12", "x13", "x14", "x15", "x18", "x19", "x20", "x21", "x22",
    "x23", "x24", "x25", "x26", "x27", "x28", "x29",
];

/// The value a register that holds none of a call's arguments is given before it, and must still
/// hold after: one for each, small enough for a `cmp` immediate on both CPUs.
const fn sentinel(position: usize) -> usize {
    0x500 + position
}

/// The instructions that put an argument in a register, on x86-64.
fn x86_load(text: &mut String, arg: &Arg, register: &str) {
    match arg {
        N(value) => {
            let _ = writeln!(text, "    mov {register}, {value}");
        }
        Data(name) => {
            let _ = writeln!(text, "    lea {register}, [rip + {name}]");
        }
        Scratch(offset) => {
            let _ = writeln!(text, "    lea {register}, [rsp + {offset}]");
        }
        Kept(word) => {
            let _ = writeln!(text, "    mov {register}, qword ptr [rsp + {}]", 8 * word);
        }
        Argv(k) | Envp(k) => {
            let at = if matches!(arg, Argv(_)) {
                ARGV_AT
            } else {
                ENVP_AT
            };
            let _ = writeln!(text, "    mov {register}, qword ptr [rsp + {at}]");
            let _ = writeln!(
                text,
                "    mov {register}, qword ptr [{register} + {}]",
                8 * k
            );
        }
    }
}

/// The call of a step on x86-64. Around an operation every register it keeps holds a value of its
/// own, and after it each is checked, the arguments' registers too: a register that changed ends
/// the program with 100 and the step's number. A library function is called as it is.
fn x86_call(
    text: &mut String,
    index: usize,
    op: &str,
    args: &[Arg],
    keep: Option<u32>,
    function: bool,
) {
    let registers = if function {
        X86_FUNCTION_ARGUMENTS
    } else {
        X86_ARGUMENTS
    };
    if !function {
        for (position, register) in X86_KEPT.iter().enumerate() {
            let _ = writeln!(text, "    mov {register}, {}", sentinel(position));
        }
    }
    for (position, register) in registers.iter().enumerate() {
        match args.get(position) {
            Some(arg) => x86_load(text, arg, register),
            None if !function => {
                let value = sentinel(X86_KEPT.len() + position);
                let _ = writeln!(text, "    mov {register}, {value}");
            }
            None => {}
        }
    }
    let _ = writeln!(text, "    call {op}");
    if !function {
        let bad = format!(".Lbad{index}");
        for (position, register) in X86_KEPT.iter().enumerate() {
            let value = sentinel(position);
            let _ = writeln!(text, "    cmp {register}, {value}\n    jne {bad}");
        }
        for (position, register) in X86_ARGUMENTS.iter().enumerate() {
            if let Some(arg) = args.get(position) {
                x86_load(text, arg, "r11");
                let _ = writeln!(text, "    cmp {register}, r11\n    jne {bad}");
            } else {
                let value = sentinel(X86_KEPT.len() + position);
                let _ = writeln!(text, "    cmp {register}, {value}\n    jne {bad}");
            }
        }
        let _ = writeln!(
            text,
            "    jmp .Lok{index}\n{bad}:\n    mov edi, {}\n    call os_exit\n.Lok{index}:",
            100 + index
        );
    }
    if let Some(word) = keep {
        let _ = writeln!(text, "    mov qword ptr [rsp + {}], rax", 8 * word);
    }
}

/// The case as an x86-64 program.
fn x86_program(case: &Case) -> String {
    let mut text = String::from(".intel_syntax noprefix\n.text\n.type main, @function\nmain:\n");
    let _ = writeln!(text, "    sub rsp, {}", SCRATCH + 8);
    // main's arguments to the scratch area's last words, and the rest zeroed.
    text.push_str("    mov r8, rdi\n    mov r9, rsi\n    mov r10, rdx\n");
    text.push_str("    xor eax, eax\n");
    for word in 0..SCRATCH / 8 {
        let _ = writeln!(text, "    mov qword ptr [rsp + {}], rax", 8 * word);
    }
    let _ = writeln!(text, "    mov qword ptr [rsp + {ARGC_AT}], r8");
    let _ = writeln!(text, "    mov qword ptr [rsp + {ARGV_AT}], r9");
    let _ = writeln!(text, "    mov qword ptr [rsp + {ENVP_AT}], r10");
    for (index, step) in case.steps.iter().enumerate() {
        match step {
            Call { op, args, keep }
            | Function {
                name: op,
                args,
                keep,
            } => x86_call(
                &mut text,
                index,
                op,
                args,
                *keep,
                matches!(step, Function { .. }),
            ),
            Set {
                offset,
                bytes,
                value,
            } => {
                let size = match bytes {
                    1 => "byte",
                    2 => "word",
                    4 => "dword",
                    _ => "qword",
                };
                let _ = writeln!(text, "    mov {size} ptr [rsp + {offset}], {value}");
            }
        }
    }
    let _ = writeln!(
        text,
        "    mov edi, 1\n    mov rsi, rsp\n    mov edx, {}\n    call os_write\n    mov edi, {}\n    call os_exit\n.size main, . - main",
        case.report, case.exit
    );
    text.push_str(case.extra.0);
    text.push_str(&data(case));
    text
}

/// The instructions that put an argument in a register, on AArch64.
fn a64_load(text: &mut String, arg: &Arg, register: &str) {
    match arg {
        N(value) => {
            let _ = writeln!(text, "    mov {register}, #{value}");
        }
        Data(name) => {
            let _ = writeln!(
                text,
                "    adrp {register}, {name}\n    add {register}, {register}, :lo12:{name}"
            );
        }
        Scratch(offset) => {
            let _ = writeln!(text, "    add {register}, sp, #{offset}");
        }
        Kept(word) => {
            let _ = writeln!(text, "    ldr {register}, [sp, #{}]", 8 * word);
        }
        Argv(k) | Envp(k) => {
            let at = if matches!(arg, Argv(_)) {
                ARGV_AT
            } else {
                ENVP_AT
            };
            let _ = writeln!(text, "    ldr {register}, [sp, #{at}]");
            let _ = writeln!(text, "    ldr {register}, [{register}, #{}]", 8 * k);
        }
    }
}

/// The call of a step on AArch64, checked as on x86-64 (x0 holds the result, so it isn't).
fn a64_call(
    text: &mut String,
    index: usize,
    op: &str,
    args: &[Arg],
    keep: Option<u32>,
    function: bool,
) {
    if !function {
        for (position, register) in A64_KEPT.iter().enumerate() {
            let _ = writeln!(text, "    mov {register}, #{}", sentinel(position));
        }
    }
    for number in 0..6 {
        match args.get(number) {
            Some(arg) => a64_load(text, arg, &format!("x{number}")),
            None if !function && number > 0 => {
                let value = sentinel(A64_KEPT.len() + number);
                let _ = writeln!(text, "    mov x{number}, #{value}");
            }
            None => {}
        }
    }
    let _ = writeln!(text, "    bl {op}");
    if !function {
        let bad = format!(".Lbad{index}");
        for (position, register) in A64_KEPT.iter().enumerate() {
            let value = sentinel(position);
            let _ = writeln!(text, "    cmp {register}, #{value}\n    b.ne {bad}");
        }
        for number in 1..6 {
            if let Some(arg) = args.get(number) {
                a64_load(text, arg, "x16");
                let _ = writeln!(text, "    cmp x{number}, x16\n    b.ne {bad}");
            } else {
                let value = sentinel(A64_KEPT.len() + number);
                let _ = writeln!(text, "    cmp x{number}, #{value}\n    b.ne {bad}");
            }
        }
        let _ = writeln!(
            text,
            "    b .Lok{index}\n{bad}:\n    mov x0, #{}\n    bl os_exit\n.Lok{index}:",
            100 + index
        );
    }
    if let Some(word) = keep {
        let _ = writeln!(text, "    str x0, [sp, #{}]", 8 * word);
    }
}

/// The case as an AArch64 program.
fn a64_program(case: &Case) -> String {
    let mut text = String::from(".arch armv8-a\n.text\n.type main, @function\nmain:\n");
    let _ = writeln!(text, "    sub sp, sp, #{}", SCRATCH + 16);
    let _ = writeln!(text, "    stp x0, x1, [sp, #{ARGC_AT}]");
    let _ = writeln!(text, "    str x2, [sp, #{ENVP_AT}]");
    for pair in 0..30 {
        let _ = writeln!(text, "    stp xzr, xzr, [sp, #{}]", 16 * pair);
    }
    let _ = writeln!(text, "    str xzr, [sp, #480]");
    for (index, step) in case.steps.iter().enumerate() {
        match step {
            Call { op, args, keep }
            | Function {
                name: op,
                args,
                keep,
            } => a64_call(
                &mut text,
                index,
                op,
                args,
                *keep,
                matches!(step, Function { .. }),
            ),
            Set {
                offset,
                bytes,
                value,
            } => {
                let (store, register) = match bytes {
                    1 => ("strb", "w16"),
                    2 => ("strh", "w16"),
                    4 => ("str", "w16"),
                    _ => ("str", "x16"),
                };
                let _ = writeln!(
                    text,
                    "    mov {register}, #{value}\n    {store} {register}, [sp, #{offset}]"
                );
            }
        }
    }
    let _ = writeln!(
        text,
        "    mov x0, #1\n    mov x1, sp\n    mov x2, #{}\n    bl os_write\n    mov x0, #{}\n    bl os_exit\n.size main, . - main",
        case.report, case.exit
    );
    text.push_str(case.extra.1);
    text.push_str(&data(case));
    text
}

fn program(target: Profile, case: &Case) -> String {
    match target {
        Profile::LinuxX86_64 => x86_program(case),
        Profile::LinuxAarch64 => a64_program(case),
    }
}

/// A worker for `parallel_for`: it writes 0x40 and its index in its slot's first byte and returns
/// nine times its index and one.
const X86_WORKER: &str = ".type worker, @function\nworker:\n    lea eax, [rdi + 64]\n    mov byte ptr [rsi], al\n    lea rax, [rdi + 8*rdi + 1]\n    ret\n.size worker, . - worker\n";

/// The same worker for AArch64.
const A64_WORKER: &str = ".type worker, @function\nworker:\n    add w2, w0, #64\n    strb w2, [x1]\n    mov x2, #9\n    mul x0, x0, x2\n    add x0, x0, #1\n    ret\n.size worker, . - worker\n";

fn cases() -> Vec<Case> {
    const GREETING: &[(&str, &[u8])] = &[("greeting", b"hello\n")];
    vec![
        // The first operations: write, and exit with the status given.
        Case {
            data: GREETING,
            exit: 7,
            ..Case::new(
                "write_and_exit",
                &[Call {
                    op: "os_write",
                    args: &[N(1), Data("greeting"), N(6)],
                    keep: Some(0),
                }],
                8,
                [b"hello\n".to_vec(), image(8, &[6], &[])].concat(),
            )
        },
        // An error is the negative of its number, and descriptor 4,352 is lengua's own, which
        // every operation refuses as a descriptor that is not open.
        Case {
            data: GREETING,
            ..Case::new(
                "write_errors",
                &[
                    Call {
                        op: "os_write",
                        args: &[N(99), Data("greeting"), N(6)],
                        keep: Some(0),
                    },
                    Call {
                        op: "os_write",
                        args: &[N(4352), Data("greeting"), N(6)],
                        keep: Some(1),
                    },
                ],
                16,
                image(16, &[-9, -9], &[]),
            )
        },
        // A read returns the bytes there are and then 0 at the end of the input.
        Case {
            stdin: b"abc",
            calls: &[Syscall::Read],
            ..Case::new(
                "read",
                &[
                    Call {
                        op: "os_read",
                        args: &[N(0), Scratch(64), N(16)],
                        keep: Some(0),
                    },
                    Call {
                        op: "os_read",
                        args: &[N(0), Scratch(64), N(16)],
                        keep: Some(1),
                    },
                    Call {
                        op: "os_read",
                        args: &[N(99), Scratch(64), N(16)],
                        keep: Some(2),
                    },
                    Call {
                        op: "os_read",
                        args: &[N(4352), Scratch(64), N(16)],
                        keep: Some(3),
                    },
                ],
                72,
                image(72, &[3, 0, -9, -9], &[(64, b"abc")]),
            )
        },
        // Closing and seeking a descriptor that isn't open or is lengua's own, and seeking
        // where nothing has places: a pipe is ESPIPE.
        Case {
            calls: &[Syscall::Close, Syscall::Lseek],
            ..Case::new(
                "close_and_seek",
                &[
                    Call {
                        op: "os_close",
                        args: &[N(99)],
                        keep: Some(0),
                    },
                    Call {
                        op: "os_close",
                        args: &[N(4352)],
                        keep: Some(1),
                    },
                    Call {
                        op: "os_seek",
                        args: &[N(0), N(0), N(0)],
                        keep: Some(2),
                    },
                    Call {
                        op: "os_seek",
                        args: &[N(4352), N(0), N(0)],
                        keep: Some(3),
                    },
                ],
                32,
                image(32, &[-9, -9, -29, -9], &[]),
            )
        },
        // Making and removing a folder, and what goes wrong: a name that is taken (EEXIST), one
        // that isn't there (ENOENT), a kind that is none (EINVAL).
        Case {
            data: &[("d", b"d\0"), ("x", b"x\0"), ("y", b"y\0")],
            calls: &[Syscall::Mkdirat, Syscall::Unlinkat, Syscall::Renameat],
            ..Case::new(
                "folders_and_names",
                &[
                    Call {
                        op: "os_mkdir",
                        args: &[Data("d")],
                        keep: Some(0),
                    },
                    Call {
                        op: "os_mkdir",
                        args: &[Data("d")],
                        keep: Some(1),
                    },
                    Call {
                        op: "os_remove",
                        args: &[Data("d"), N(1)],
                        keep: Some(2),
                    },
                    Call {
                        op: "os_remove",
                        args: &[Data("d"), N(1)],
                        keep: Some(3),
                    },
                    Call {
                        op: "os_remove",
                        args: &[Data("d"), N(7)],
                        keep: Some(4),
                    },
                    Call {
                        op: "os_rename",
                        args: &[Data("x"), Data("y")],
                        keep: Some(5),
                    },
                ],
                48,
                image(48, &[0, -17, 0, -2, -22, -2], &[]),
            )
        },
        // A file renamed is where it was named, and the other name is free.
        Case {
            data: &[("x", b"x\0"), ("y", b"y\0")],
            files: &[("x", "kept")],
            calls: &[Syscall::Renameat],
            ..Case::new(
                "a_file_renamed",
                &[Call {
                    op: "os_rename",
                    args: &[Data("x"), Data("y")],
                    keep: Some(0),
                }],
                8,
                image(8, &[0], &[]),
            )
        },
        // The clocks, a clock that doesn't exist (EINVAL), and a wait of a microsecond.
        Case {
            calls: &[Syscall::ClockGettime, Syscall::ClockNanosleep],
            ..Case::new(
                "clocks_and_sleep",
                &[
                    Call {
                        op: "os_clock",
                        args: &[N(1), Scratch(64)],
                        keep: Some(0),
                    },
                    Call {
                        op: "os_clock",
                        args: &[N(0), Scratch(80)],
                        keep: Some(1),
                    },
                    Call {
                        op: "os_clock",
                        args: &[N(2), Scratch(96)],
                        keep: Some(2),
                    },
                    Set {
                        offset: 120,
                        bytes: 8,
                        value: 1000,
                    },
                    Call {
                        op: "os_sleep",
                        args: &[N(1), N(0), Scratch(112)],
                        keep: Some(3),
                    },
                ],
                32,
                image(32, &[0, 0, -22, 0], &[]),
            )
        },
        // Random bytes: all that were asked for, up to 256, and no more.
        Case {
            calls: &[Syscall::Getrandom],
            ..Case::new(
                "random",
                &[
                    Call {
                        op: "os_random",
                        args: &[Scratch(64), N(8)],
                        keep: Some(0),
                    },
                    Call {
                        op: "os_random",
                        args: &[Scratch(64), N(300)],
                        keep: Some(1),
                    },
                ],
                16,
                image(16, &[8, -22], &[]),
            )
        },
        // The network's operations with no network: nothing is listening on 3, the first
        // connection is 4 and not a listener, and shutdown on a pipe is none of the errors it
        // gives, so it is EIO.
        Case {
            calls: &[Syscall::Accept4, Syscall::Shutdown],
            ..Case::new(
                "network_without_a_network",
                &[
                    Call {
                        op: "os_accept",
                        args: &[N(3)],
                        keep: Some(0),
                    },
                    Call {
                        op: "os_accept",
                        args: &[N(4)],
                        keep: Some(1),
                    },
                    Call {
                        op: "os_shutdown",
                        args: &[N(1), N(1)],
                        keep: Some(2),
                    },
                    Call {
                        op: "os_shutdown",
                        args: &[N(4352), N(1)],
                        keep: Some(3),
                    },
                ],
                32,
                image(32, &[-9, -9, -5, -9], &[]),
            )
        },
        // Descriptor 4,352 is lengua's own, and the operations that take a descriptor and have no
        // case of their own for it refuse it too, as one that isn't open.
        Case {
            calls: &[Syscall::Fstat, Syscall::Getdents64, Syscall::Accept4],
            ..Case::new(
                "the_channel_is_not_a_file",
                &[
                    Call {
                        op: "os_info",
                        args: &[N(4352), Scratch(64)],
                        keep: Some(0),
                    },
                    Call {
                        op: "os_list",
                        args: &[N(4352), Scratch(128), N(64)],
                        keep: Some(1),
                    },
                    Call {
                        op: "os_accept",
                        args: &[N(4352)],
                        keep: Some(2),
                    },
                ],
                24,
                image(24, &[-9, -9, -9], &[]),
            )
        },
        // A wait on standard input that has a byte and has ended (data and hang-up, 1 and 16),
        // and one on descriptor 4,352, which is refused.
        Case {
            stdin: b"x",
            calls: &[Syscall::Poll],
            model: Some("it says a key is ready, where the pipe has also hung up"),
            ..Case::new(
                "poll",
                &[
                    Set {
                        offset: 64,
                        bytes: 4,
                        value: 0,
                    },
                    Set {
                        offset: 68,
                        bytes: 2,
                        value: 1,
                    },
                    Call {
                        op: "os_poll",
                        args: &[Scratch(64), N(1), N(0)],
                        keep: Some(0),
                    },
                    Set {
                        offset: 72,
                        bytes: 4,
                        value: 4352,
                    },
                    Set {
                        offset: 76,
                        bytes: 2,
                        value: 1,
                    },
                    Call {
                        op: "os_poll",
                        args: &[Scratch(72), N(1), N(0)],
                        keep: Some(1),
                    },
                ],
                80,
                image(
                    80,
                    &[1, -9],
                    &[
                        (64, &[0, 0, 0, 0, 1, 0, 17, 0]),
                        (72, &[0, 0x11, 0, 0, 1, 0, 0, 0]),
                    ],
                ),
            )
        },
        // What `main` is given: the number of arguments, the arguments and the environment,
        // each as the operating system lays them out.
        Case {
            arguments: &["alpha", "be"],
            environment: &[("NAME", "value")],
            ..Case::new(
                "main_arguments",
                &[
                    Call {
                        op: "os_write",
                        args: &[N(1), Scratch(ARGC_AT), N(8)],
                        keep: Some(0),
                    },
                    Call {
                        op: "os_write",
                        args: &[N(1), Argv(1), N(5)],
                        keep: Some(1),
                    },
                    Call {
                        op: "os_write",
                        args: &[N(1), Argv(2), N(2)],
                        keep: Some(2),
                    },
                    Call {
                        op: "os_write",
                        args: &[N(1), Envp(0), N(10)],
                        keep: Some(3),
                    },
                ],
                32,
                [
                    word(3),
                    b"alpha".to_vec(),
                    b"be".to_vec(),
                    b"NAME=value".to_vec(),
                    image(32, &[8, 5, 2, 10], &[]),
                ]
                .concat(),
            )
        },
        // fill reads until its buffer is full or the input ends, however the input comes, and
        // gives an operation's error as it is.
        Case {
            stdin: b"abcdefgh",
            calls: &[Syscall::Read],
            ..Case::new(
                "fill",
                &[
                    Function {
                        name: "fill",
                        args: &[N(0), Scratch(64), N(4), N(-1)],
                        keep: Some(0),
                    },
                    Function {
                        name: "fill",
                        args: &[N(0), Scratch(128), N(16), N(-1)],
                        keep: Some(1),
                    },
                    Function {
                        name: "fill",
                        args: &[N(0), Scratch(192), N(4), N(-1)],
                        keep: Some(2),
                    },
                    Function {
                        name: "fill",
                        args: &[N(99), Scratch(64), N(4), N(-1)],
                        keep: Some(3),
                    },
                ],
                200,
                image(200, &[4, 4, 0, -9], &[(64, b"abcd"), (128, b"efgh")]),
            )
        },
        // An assertion that fails ends the program with 134, printing nothing to standard output.
        Case {
            exit: 134,
            stderr: b"lengua: assertion 005 failed\n",
            ..Case::new(
                "an_assertion_fails",
                &[Function {
                    name: "assert_fail",
                    args: &[N(5)],
                    keep: None,
                }],
                0,
                vec![],
            )
        },
        // Four threads, each running main.s's worker on a slot of its own: what each returns lands
        // in the results, and what each wrote is in its slot.
        Case {
            threads: true,
            extra: (X86_WORKER, A64_WORKER),
            model: Some("it starts no threads of its own: lengua's library does"),
            ..Case::new(
                "parallel_for",
                &[Function {
                    name: "parallel_for",
                    args: &[N(4), Scratch(256), N(16), Scratch(64)],
                    keep: None,
                }],
                320,
                image(
                    320,
                    &[0, 0, 0, 0, 0, 0, 0, 0, 1, 10, 19, 28],
                    &[
                        (256, &[0x40]),
                        (272, &[0x41]),
                        (288, &[0x42]),
                        (304, &[0x43]),
                    ],
                ),
            )
        },
        // The terminal's operations where there is no terminal, which is every test's case: taking
        // keys, putting the mode back, and the window's size are all "not a terminal", ENOTTY.
        Case {
            calls: &[Syscall::Ioctl],
            ..Case::new(
                "terminal_without_a_terminal",
                &[
                    Call {
                        op: "os_terminal",
                        args: &[N(1)],
                        keep: Some(0),
                    },
                    Call {
                        op: "os_terminal",
                        args: &[N(0)],
                        keep: Some(1),
                    },
                    Call {
                        op: "os_window",
                        args: &[Scratch(64)],
                        keep: Some(2),
                    },
                ],
                24,
                image(24, &[-25, -25, -25], &[]),
            )
        },
        // A file read, its size and kind, a seek, a descriptor closed and used again, a name
        // that isn't there (ENOENT) and a mode that is none (EINVAL). The descriptor itself
        // is kept in a word the report leaves out.
        Case {
            data: &[("in", b"in.txt\0"), ("none", b"none\0")],
            model: Some("it doesn't follow a seek in an open file"),
            files: &[("in.txt", "hello")],
            calls: &[
                Syscall::Openat,
                Syscall::Read,
                Syscall::Close,
                Syscall::Lseek,
                Syscall::Fstat,
            ],
            ..Case::new(
                "a_file_read",
                &[
                    Call {
                        op: "os_open",
                        args: &[Data("in"), N(0)],
                        keep: Some(30),
                    },
                    Call {
                        op: "os_read",
                        args: &[Kept(30), Scratch(128), N(16)],
                        keep: Some(0),
                    },
                    Call {
                        op: "os_info",
                        args: &[Kept(30), Scratch(72)],
                        keep: Some(1),
                    },
                    Call {
                        op: "os_seek",
                        args: &[Kept(30), N(1), N(0)],
                        keep: Some(2),
                    },
                    Call {
                        op: "os_read",
                        args: &[Kept(30), Scratch(144), N(16)],
                        keep: Some(3),
                    },
                    Call {
                        op: "os_close",
                        args: &[Kept(30)],
                        keep: Some(4),
                    },
                    Call {
                        op: "os_read",
                        args: &[Kept(30), Scratch(144), N(16)],
                        keep: Some(5),
                    },
                    Call {
                        op: "os_open",
                        args: &[Data("none"), N(0)],
                        keep: Some(6),
                    },
                    Call {
                        op: "os_open",
                        args: &[Data("in"), N(9)],
                        keep: Some(7),
                    },
                ],
                88,
                image(88, &[5, 0, 1, 4, 0, -9, -2, -22, 0, 5, 0], &[]),
            )
        },
        // The ways to write a file: made and emptied, appended to, and made only if it is not
        // there (EEXIST when it is). The folder holds what they wrote afterwards.
        Case {
            model: Some("it doesn't add to a file or make one only when it is new"),
            data: &[
                ("out", b"out.txt\0"),
                ("new", b"new.txt\0"),
                ("greeting", b"hello\n"),
            ],
            after: &[("out.txt", "hello\nhello\n"), ("new.txt", "hel")],
            calls: &[Syscall::Openat, Syscall::Close],
            ..Case::new(
                "files_written",
                &[
                    Call {
                        op: "os_open",
                        args: &[Data("out"), N(1)],
                        keep: Some(30),
                    },
                    Call {
                        op: "os_write",
                        args: &[Kept(30), Data("greeting"), N(6)],
                        keep: Some(0),
                    },
                    Call {
                        op: "os_close",
                        args: &[Kept(30)],
                        keep: Some(1),
                    },
                    Call {
                        op: "os_open",
                        args: &[Data("out"), N(2)],
                        keep: Some(30),
                    },
                    Call {
                        op: "os_write",
                        args: &[Kept(30), Data("greeting"), N(6)],
                        keep: Some(2),
                    },
                    Call {
                        op: "os_close",
                        args: &[Kept(30)],
                        keep: Some(3),
                    },
                    Call {
                        op: "os_open",
                        args: &[Data("out"), N(3)],
                        keep: Some(4),
                    },
                    Call {
                        op: "os_open",
                        args: &[Data("new"), N(3)],
                        keep: Some(30),
                    },
                    Call {
                        op: "os_write",
                        args: &[Kept(30), Data("greeting"), N(3)],
                        keep: Some(5),
                    },
                    Call {
                        op: "os_close",
                        args: &[Kept(30)],
                        keep: Some(6),
                    },
                ],
                56,
                image(56, &[6, 0, 6, 0, -17, 3, 0], &[]),
            )
        },
        // A file's size limit (ten bytes here): the write that crosses it is cut short, the next
        // fails with EFBIG, and the file holds what fitted.
        Case {
            model: Some("it has no limit on the size of a file"),
            data: &[("out", b"out.txt\0"), ("greeting", b"hello\n")],
            after: &[("out.txt", "hello\nhell")],
            file_limit: Some(10),
            calls: &[Syscall::Openat, Syscall::Close],
            ..Case::new(
                "a_write_past_the_file_limit",
                &[
                    Call {
                        op: "os_open",
                        args: &[Data("out"), N(1)],
                        keep: Some(30),
                    },
                    Call {
                        op: "os_write",
                        args: &[Kept(30), Data("greeting"), N(6)],
                        keep: Some(0),
                    },
                    Call {
                        op: "os_write",
                        args: &[Kept(30), Data("greeting"), N(6)],
                        keep: Some(1),
                    },
                    Call {
                        op: "os_write",
                        args: &[Kept(30), Data("greeting"), N(6)],
                        keep: Some(2),
                    },
                    Call {
                        op: "os_close",
                        args: &[Kept(30)],
                        keep: Some(3),
                    },
                ],
                32,
                image(32, &[6, 4, -27, 0], &[]),
            )
        },
        // Listing folders: one entry of a file, one of a folder, a folder with nothing in it
        // (`.` and `..` are left out, so the first read has nothing and the next is the end), a
        // file that is no folder (ENOTDIR), a descriptor that isn't open, and the kind and size of
        // a folder, a file and a pipe.
        Case {
            model: Some("it doesn't list a folder, whose order is the system's"),
            data: &[
                ("one", b"one\0"),
                ("two", b"two\0"),
                ("empty", b"empty\0"),
                ("file", b"one/only.txt\0"),
            ],
            files: &[("one/only.txt", "abcd")],
            folders: &["two/s", "empty"],
            calls: &[
                Syscall::Openat,
                Syscall::Close,
                Syscall::Getdents64,
                Syscall::Fstat,
            ],
            ..Case::new(
                "folders_listed",
                &[
                    Call {
                        op: "os_open",
                        args: &[Data("one"), N(4)],
                        keep: Some(30),
                    },
                    Call {
                        op: "os_list",
                        args: &[Kept(30), Scratch(256), N(128)],
                        keep: Some(0),
                    },
                    Call {
                        op: "os_list",
                        args: &[Kept(30), Scratch(384), N(128)],
                        keep: Some(1),
                    },
                    Call {
                        op: "os_info",
                        args: &[Kept(30), Scratch(64)],
                        keep: None,
                    },
                    Call {
                        op: "os_close",
                        args: &[Kept(30)],
                        keep: None,
                    },
                    Call {
                        op: "os_open",
                        args: &[Data("two"), N(4)],
                        keep: Some(30),
                    },
                    Call {
                        op: "os_list",
                        args: &[Kept(30), Scratch(416), N(64)],
                        keep: Some(2),
                    },
                    Call {
                        op: "os_list",
                        args: &[Kept(30), Scratch(440), N(32)],
                        keep: Some(3),
                    },
                    Call {
                        op: "os_close",
                        args: &[Kept(30)],
                        keep: None,
                    },
                    Call {
                        op: "os_open",
                        args: &[Data("empty"), N(4)],
                        keep: Some(30),
                    },
                    Call {
                        op: "os_list",
                        args: &[Kept(30), Scratch(256), N(128)],
                        keep: Some(4),
                    },
                    Call {
                        op: "os_close",
                        args: &[Kept(30)],
                        keep: None,
                    },
                    Call {
                        op: "os_open",
                        args: &[Data("file"), N(0)],
                        keep: Some(30),
                    },
                    Call {
                        op: "os_list",
                        args: &[Kept(30), Scratch(256), N(128)],
                        keep: Some(5),
                    },
                    Call {
                        op: "os_info",
                        args: &[Kept(30), Scratch(80)],
                        keep: None,
                    },
                    Call {
                        op: "os_close",
                        args: &[Kept(30)],
                        keep: None,
                    },
                    Call {
                        op: "os_list",
                        args: &[N(99), Scratch(256), N(128)],
                        keep: Some(6),
                    },
                    Call {
                        op: "os_info",
                        args: &[N(0), Scratch(96)],
                        keep: None,
                    },
                ],
                488,
                image(
                    488,
                    // The results, then the kind and size of the folder, the file and the pipe.
                    &[16, 0, 8, 0, 0, -20, -9, 0, 0, 1, 4, 0, 0, 2],
                    &[
                        // The descriptor the last open gave, which the report takes in.
                        (240, &[3]),
                        (
                            256,
                            &[
                                16, 0, 0, 8, b'o', b'n', b'l', b'y', b'.', b't', b'x', b't', 0, 0,
                                0, 0,
                            ],
                        ),
                        (416, &[8, 0, 1, 1, b's', 0, 0, 0]),
                    ],
                ),
            )
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
            let text = program(target, &case);
            let executable = per_target::build(target, &place, case.name, &text).await;
            built.push((
                target,
                executable,
                lengua::edge::uses_channel(target, &text),
            ));
        }
        for (target, executable, channel) in built {
            let Some(how) = per_target::way(target) else {
                continue;
            };
            let mut idea = vec![Syscall::Write, Syscall::Exit];
            idea.extend_from_slice(case.calls);
            let allowed = target.allowed_syscalls(&idea, case.threads);
            let arguments: Vec<String> = case
                .arguments
                .iter()
                .map(|text| (*text).to_owned())
                .collect();
            let environment: BTreeMap<String, String> = case
                .environment
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect();
            let files: BTreeMap<String, String> = case
                .files
                .iter()
                .map(|(name, contents)| ((*name).to_owned(), (*contents).to_owned()))
                .collect();
            let folders: BTreeSet<String> =
                case.folders.iter().map(|name| (*name).to_owned()).collect();
            // A program that links the channel runs twice: on its own, calling the system itself,
            // and with lengua's broker serving it. It does the same both ways.
            for brokered in [false, true] {
                if brokered && !channel {
                    continue;
                }
                let job = Job {
                    emulated: how == Way::Emulated,
                    env: &environment,
                    files: &files,
                    folders: &folders,
                    channel: brokered.then(|| Permit::of(&idea)),
                    ..Job::new(&executable, &arguments, case.stdin, &allowed)
                };
                let limits = Limits {
                    max_file_size: case.file_limit.or(Limits::default().max_file_size),
                    ..Limits::default()
                };
                let ran = run(launcher(), job, limits)
                    .await
                    .expect("the program runs");
                let what = format!(
                    "{} on {}{}",
                    case.name,
                    target.name(),
                    if brokered { ", through the broker" } else { "" }
                );
                let exit = i32::try_from(case.exit).unwrap_or(-1);
                // 100 and a step's number: a register that call must keep has changed.
                let hint = match ran.end {
                    End::Exited(code) if code != exit && (100..200).contains(&code) => {
                        format!(" (step {} changed a register it must keep)", code - 100)
                    }
                    _ => String::new(),
                };
                assert_eq!(
                    ran.end,
                    End::Exited(exit),
                    "{what}{hint}: {}",
                    String::from_utf8_lossy(&ran.stderr)
                );
                assert_eq!(ran.stdout, case.stdout, "{what}");
                if !case.stderr.is_empty() {
                    assert_eq!(ran.stderr, case.stderr, "{what}");
                }
                if !case.after.is_empty() {
                    let found = ran.files.as_ref().expect("the files it left");
                    let wanted: BTreeMap<String, Vec<u8>> = case
                        .after
                        .iter()
                        .map(|(name, contents)| ((*name).to_owned(), contents.as_bytes().to_vec()))
                        .collect();
                    assert_eq!(*found, wanted, "{what}");
                }
            }
        }
    }
}

/// A test gives the program its clock and its random bytes through the broker, in the order it
/// asks, the last reading repeating, and a wait of an hour ends at once; and the broker knows how
/// many random bytes the program asked for.
#[tokio::test]
async fn a_test_gives_the_program_its_clock_and_random_bytes_through_the_broker() {
    use lengua::supply::{Supply, Time};

    let case = Case {
        calls: &[
            Syscall::ClockGettime,
            Syscall::ClockNanosleep,
            Syscall::Getrandom,
        ],
        ..Case::new(
            "supplied",
            &[
                Set {
                    offset: 112,
                    bytes: 8,
                    value: 3600,
                },
                Call {
                    op: "os_clock",
                    args: &[N(0), Scratch(64)],
                    keep: Some(0),
                },
                Call {
                    op: "os_random",
                    args: &[Scratch(80), N(4)],
                    keep: Some(1),
                },
                Call {
                    op: "os_clock",
                    args: &[N(1), Scratch(96)],
                    keep: Some(2),
                },
                Call {
                    op: "os_sleep",
                    args: &[N(1), N(0), Scratch(112)],
                    keep: Some(3),
                },
            ],
            128,
            image(
                128,
                &[0, 4, 0, 0],
                &[
                    (64, &word(100)),
                    (72, &word(5)),
                    (80, &[1, 2, 3, 4]),
                    (96, &word(200)),
                    (104, &word(7)),
                    (112, &word(3600)),
                ],
            ),
        )
    };
    let supply = Supply {
        random: Some(vec![1, 2, 3, 4]),
        clock: vec![
            Time {
                seconds: 100,
                nanoseconds: 5,
            },
            Time {
                seconds: 200,
                nanoseconds: 7,
            },
        ],
    };
    let folder = tempfile::tempdir().expect("a folder");
    for target in Profile::ALL {
        let place = folder.path().join(target.name());
        let text = program(target, &case);
        let executable = per_target::build(target, &place, case.name, &text).await;
        assert!(
            lengua::edge::uses_channel(target, &text),
            "{}",
            target.name()
        );
        let Some(how) = per_target::way(target) else {
            continue;
        };
        let mut idea = vec![Syscall::Write, Syscall::Exit];
        idea.extend_from_slice(case.calls);
        let allowed = target.allowed_syscalls(&idea, false);
        let job = Job {
            emulated: how == Way::Emulated,
            supply: Some(&supply),
            channel: Some(Permit::ALL),
            ..Job::new(&executable, &[], b"", &allowed)
        };
        let ran = run(launcher(), job, Limits::default())
            .await
            .expect("the program runs");
        let what = target.name();
        assert_eq!(
            ran.end,
            End::Exited(0),
            "{what}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        assert_eq!(ran.stdout, case.stdout, "{what}");
        assert_eq!(ran.supplied, None, "{what}");
        assert_eq!(ran.random_asked, Some(4), "{what}");
        assert!(ran.wall_time_ns < 1_500_000_000, "{what}: {ran:?}");
        // With only the random bytes supplied, the clock is the system's.
        let random_only = Supply {
            random: Some(vec![1, 2, 3, 4]),
            clock: Vec::new(),
        };
        let job = Job {
            emulated: how == Way::Emulated,
            supply: Some(&random_only),
            channel: Some(Permit::ALL),
            ..Job::new(&executable, &[], b"", &allowed)
        };
        let ran = run(launcher(), job, Limits::default())
            .await
            .expect("the program runs");
        assert_eq!(ran.end, End::Exited(0), "{what}");
        assert_eq!(ran.stdout[80..84], [1, 2, 3, 4], "{what}");
        let seconds = i64::from_le_bytes(ran.stdout[64..72].try_into().expect("8 bytes"));
        assert!(
            seconds > 1_700_000_000,
            "{what}: the system's time, {seconds}"
        );
    }
}

/// A test's second run has each folder listing turned round, the last entry first, and the broker
/// is told that the program lists.
#[tokio::test]
async fn a_listing_is_turned_round_for_a_second_run_and_the_broker_is_told_of_it() {
    use lengua::supply::Supply;

    let case = Case {
        calls: &[Syscall::Openat, Syscall::Getdents64],
        data: &[("dot", b".\0")],
        ..Case::new(
            "listing",
            &[
                Call {
                    op: "os_open",
                    args: &[Data("dot"), N(4)],
                    keep: Some(30),
                },
                Call {
                    op: "os_list",
                    args: &[Kept(30), Scratch(256), N(128)],
                    keep: Some(0),
                },
            ],
            280,
            Vec::new(),
        )
    };
    let files: BTreeMap<String, String> = ["a", "b", "c"]
        .iter()
        .map(|name| ((*name).to_owned(), "1".to_owned()))
        .collect();
    let supply = Supply::default();
    let folder = tempfile::tempdir().expect("a folder");
    for target in Profile::ALL {
        let place = folder.path().join(target.name());
        let executable =
            per_target::build(target, &place, case.name, &program(target, &case)).await;
        let Some(how) = per_target::way(target) else {
            continue;
        };
        let mut idea = vec![Syscall::Write, Syscall::Exit];
        idea.extend_from_slice(case.calls);
        let allowed = target.allowed_syscalls(&idea, false);
        let mut listed = Vec::new();
        for reversed in [false, true] {
            let job = Job {
                emulated: how == Way::Emulated,
                files: &files,
                supply: Some(&supply),
                channel: Some(Permit::ALL),
                reversed,
                ..Job::new(&executable, &[], b"", &allowed)
            };
            let ran = run(launcher(), job, Limits::default())
                .await
                .expect("the program runs");
            let what = format!("{}, turned round {reversed}", target.name());
            assert_eq!(
                ran.end,
                End::Exited(0),
                "{what}: {}",
                String::from_utf8_lossy(&ran.stderr)
            );
            assert!(ran.listed, "{what}: the broker wasn't told");
            assert_eq!(ran.supplied, None, "{what}");
            // Three entries of 8 bytes each: a length, a kind, the name's length, the name, a NUL.
            let names: Vec<u8> = ran.stdout[256..280]
                .chunks(8)
                .map(|entry| entry[4])
                .collect();
            listed.push(names);
        }
        let mut turned = listed[1].clone();
        turned.reverse();
        assert_eq!(listed[0], turned, "{}", target.name());
        let mut sorted = listed[0].clone();
        sorted.sort_unstable();
        assert_eq!(sorted, b"abc", "{}", target.name());
    }
}

/// A name of 1,024 bytes, and the NUL a name ends with.
const LONG_NAME: [u8; 1025] = {
    let mut bytes = [b'a'; 1025];
    bytes[1024] = 0;
    bytes
};

/// What every system does with a name, which the broker holds the program to and the system
/// itself doesn't: a file the program holds open is busy for a rename and a remove until it is
/// closed, a name past a path's length, or none, is refused, and a new name that differs from one
/// in its folder only in case is taken.
#[tokio::test]
async fn the_broker_holds_names_to_what_every_system_does() {
    let case = Case {
        calls: &[
            Syscall::Openat,
            Syscall::Close,
            Syscall::Unlinkat,
            Syscall::Renameat,
        ],
        data: &[
            ("f", b"f\0"),
            ("g", b"g\0"),
            ("long", &LONG_NAME),
            ("none", b"\0"),
            ("upper", b"A\0"),
        ],
        files: &[("f", "x"), ("a", "y")],
        ..Case::new(
            "names",
            &[
                Call {
                    op: "os_open",
                    args: &[Data("f"), N(0)],
                    keep: Some(0),
                },
                Call {
                    op: "os_remove",
                    args: &[Data("f"), N(0)],
                    keep: Some(1),
                },
                Call {
                    op: "os_rename",
                    args: &[Data("f"), Data("g")],
                    keep: Some(2),
                },
                Call {
                    op: "os_close",
                    args: &[Kept(0)],
                    keep: Some(3),
                },
                Call {
                    op: "os_remove",
                    args: &[Data("f"), N(0)],
                    keep: Some(4),
                },
                Call {
                    op: "os_open",
                    args: &[Data("long"), N(1)],
                    keep: Some(5),
                },
                Call {
                    op: "os_open",
                    args: &[Data("none"), N(1)],
                    keep: Some(6),
                },
                Call {
                    op: "os_open",
                    args: &[Data("upper"), N(3)],
                    keep: Some(7),
                },
            ],
            64,
            // The descriptor, the two refusals of a file held open, the close and the remove that
            // then work, a name too long, none, and a name that is taken.
            image(64, &[3, -16, -16, 0, 0, -36, -2, -17], &[]),
        )
    };
    let files: BTreeMap<String, String> = case
        .files
        .iter()
        .map(|(name, contents)| ((*name).to_owned(), (*contents).to_owned()))
        .collect();
    let folder = tempfile::tempdir().expect("a folder");
    for target in Profile::ALL {
        let place = folder.path().join(target.name());
        let executable =
            per_target::build(target, &place, case.name, &program(target, &case)).await;
        let Some(how) = per_target::way(target) else {
            continue;
        };
        let mut idea = vec![Syscall::Write, Syscall::Exit];
        idea.extend_from_slice(case.calls);
        let allowed = target.allowed_syscalls(&idea, false);
        let job = Job {
            emulated: how == Way::Emulated,
            files: &files,
            channel: Some(Permit::ALL),
            ..Job::new(&executable, &[], b"", &allowed)
        };
        let ran = run(launcher(), job, Limits::default())
            .await
            .expect("the program runs");
        let what = target.name();
        assert_eq!(
            ran.end,
            End::Exited(0),
            "{what}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        assert_eq!(ran.stdout, case.stdout, "{what}");
    }
}

/// Every operation and library function builds on its own, with what it needs, and all of them
/// build together, on both CPUs, whatever the arguments: what the table of cases doesn't call is
/// still assembled, and no name is twice.
#[tokio::test]
async fn every_operation_builds_alone_and_with_all_the_others() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut names: Vec<&'static str> = lengua::edge::Op::ALL.iter().map(|op| op.symbol()).collect();
    names.extend(["fill", "parallel_for", "assert_fail"]);
    let mut groups: Vec<(String, Vec<&'static str>)> = names
        .iter()
        .map(|name| (format!("build_{name}"), vec![*name]))
        .collect();
    groups.push(("build_everything".to_owned(), names.clone()));
    for (name, calls) in groups {
        let steps: Vec<Step> = calls
            .iter()
            .map(|call| {
                if call.starts_with("os_") {
                    Call {
                        op: call,
                        args: &[],
                        keep: None,
                    }
                } else {
                    Function {
                        name: call,
                        args: &[],
                        keep: None,
                    }
                }
            })
            .collect();
        let steps: &'static [Step] = Box::leak(steps.into_boxed_slice());
        let name: &'static str = Box::leak(name.into_boxed_str());
        let case = Case {
            extra: (X86_WORKER, A64_WORKER),
            ..Case::new(name, steps, 0, Vec::new())
        };
        for target in Profile::ALL {
            let place = folder.path().join(target.name());
            per_target::build(target, &place, case.name, &program(target, &case)).await;
        }
    }
}

/// The terminal's operations on a terminal: a pseudo-terminal is the program's standard input and
/// the test holds its other end. The program takes keys and waits, the test sees the mode as the
/// program set it, the program puts it back and waits again, and the test sees it as it was.
#[cfg(target_os = "linux")]
fn on_a_terminal(executable: &Path) -> Vec<u8> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    use rustix::fs::{Mode, OFlags};
    use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
    use rustix::termios::{
        LocalModes, OptionalActions, SpecialCodeIndex, Winsize, tcgetattr, tcsetattr, tcsetwinsize,
    };

    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("a pseudo-terminal");
    grantpt(&master).expect("its slave's owner");
    unlockpt(&master).expect("its slave unlocked");
    let name = ptsname(&master, Vec::new()).expect("its slave's name");
    let slave = rustix::fs::open(
        name.as_c_str(),
        OFlags::RDWR | OFlags::NOCTTY,
        Mode::empty(),
    )
    .expect("its slave");
    tcsetwinsize(
        &master,
        Winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
    )
    .expect("a window size");
    // Codes the raw mode changes and the restoring must bring back.
    let mut start = tcgetattr(&master).expect("the mode");
    start.special_codes[SpecialCodeIndex::VMIN] = 3;
    start.special_codes[SpecialCodeIndex::VTIME] = 5;
    tcsetattr(&master, OptionalActions::Now, &start).expect("a mode to start from");
    let before = tcgetattr(&master).expect("the mode it starts from");
    let mut child = {
        let mut command = Command::new(executable);
        command
            .stdin(Stdio::from(slave))
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        command.spawn().expect("the program starts")
    };
    let mut output = child.stdout.take().expect("its output");
    let mut mark = [0_u8; 1];

    output.read_exact(&mut mark).expect("the first mark");
    assert_eq!(&mark, b"r");
    let raw = tcgetattr(&master).expect("the mode the program set");
    assert!(
        !raw.local_modes
            .intersects(LocalModes::ICANON | LocalModes::ECHO)
    );
    assert_eq!(raw.special_codes[SpecialCodeIndex::VMIN], 1);
    assert_eq!(raw.special_codes[SpecialCodeIndex::VTIME], 0);
    // Signals and the rest of the modes stay as they were.
    assert_eq!(
        raw.local_modes.contains(LocalModes::ISIG),
        before.local_modes.contains(LocalModes::ISIG)
    );
    assert_eq!(raw.input_modes, before.input_modes);
    assert_eq!(raw.output_modes, before.output_modes);
    assert_eq!(raw.control_modes, before.control_modes);
    // One key, which the raw mode hands over without a line end.
    rustix::io::write(&master, b"k").expect("a key");

    output.read_exact(&mut mark).expect("the second mark");
    assert_eq!(&mark, b"d");
    let after = tcgetattr(&master).expect("the mode the program put back");
    assert_eq!(after.local_modes, before.local_modes);
    assert_eq!(after.input_modes, before.input_modes);
    assert_eq!(after.output_modes, before.output_modes);
    assert_eq!(after.control_modes, before.control_modes);
    assert_eq!(after.special_codes[SpecialCodeIndex::VMIN], 3);
    assert_eq!(after.special_codes[SpecialCodeIndex::VTIME], 5);
    // The program waits for a line, as the mode it has again says.
    rustix::io::write(&master, b"k\n").expect("a line");

    let mut rest = Vec::new();
    output
        .read_to_end(&mut rest)
        .expect("the rest of its output");
    assert!(child.wait().expect("the program ends").success());
    [b"rd".to_vec(), rest].concat()
}

/// Every cell on a terminal: keys as they are pressed and the mode put back, and the window's size
/// as the terminal says it, asked of output and error first, which are not terminals here.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn the_terminal_operations_change_the_mode_and_put_it_back() {
    let case = Case {
        data: &[("mark", b"r"), ("done", b"d")],
        ..Case::new(
            "on_a_terminal",
            &[
                Call {
                    op: "os_terminal",
                    args: &[N(1)],
                    keep: Some(0),
                },
                Call {
                    op: "os_window",
                    args: &[Scratch(64)],
                    keep: Some(1),
                },
                Call {
                    op: "os_write",
                    args: &[N(1), Data("mark"), N(1)],
                    keep: Some(2),
                },
                Call {
                    op: "os_read",
                    args: &[N(0), Scratch(80), N(1)],
                    keep: Some(3),
                },
                Call {
                    op: "os_terminal",
                    args: &[N(0)],
                    keep: Some(4),
                },
                Call {
                    op: "os_terminal",
                    args: &[N(0)],
                    keep: Some(5),
                },
                Call {
                    op: "os_write",
                    args: &[N(1), Data("done"), N(1)],
                    keep: Some(6),
                },
                Call {
                    op: "os_read",
                    args: &[N(0), Scratch(80), N(1)],
                    keep: Some(7),
                },
            ],
            72,
            [
                b"rd".to_vec(),
                image(
                    72,
                    &[0, 0, 1, 1, 0, 0, 1, 1],
                    &[(64, &[24, 0, 0, 0, 80, 0, 0, 0])],
                ),
            ]
            .concat(),
        )
    };
    let folder = tempfile::tempdir().expect("a folder");
    for target in Profile::ALL {
        let place = folder.path().join(target.name());
        let executable =
            per_target::build(target, &place, case.name, &program(target, &case)).await;
        if per_target::way(target).is_none() {
            continue;
        }
        let printed = tokio::task::spawn_blocking(move || on_a_terminal(&executable))
            .await
            .expect("the terminal test ends");
        assert_eq!(printed, case.stdout, "{} on {}", case.name, target.name());
    }
}

/// Every case the prover's model follows runs in it as it does natively: how it ends and what it
/// prints, with no code of the edge run, on both CPUs.
#[tokio::test]
async fn the_model_runs_every_operation_as_the_system_does() {
    use lengua::prove::exec::{Concrete, End, Image, State, bytes, explore};
    use lengua::prove::term::{Pool, Term};

    let folder = tempfile::tempdir().expect("a folder");
    // Every run's difference is kept, so that one test run shows them all.
    let mut differs: Vec<String> = Vec::new();
    for case in cases() {
        if case.model.is_some() {
            continue;
        }
        for target in Profile::ALL {
            let what = format!("{} on {}", case.name, target.name());
            let place = folder.path().join(format!("model-{}", target.name()));
            let executable =
                per_target::build(target, &place, case.name, &program(target, &case)).await;
            let machine =
                lengua::checker::decode::read(&std::fs::read(&executable).expect("the executable"))
                    .expect("an executable lengua reads");
            let image = Image::new(&machine).expect("an image of it");
            let mut pool = Pool::default();
            let arguments: Vec<Vec<Term>> = case
                .arguments
                .iter()
                .map(|text| bytes(&mut pool, text.as_bytes()))
                .collect();
            let environment: Vec<Vec<Term>> = case
                .environment
                .iter()
                .map(|(name, value)| bytes(&mut pool, format!("{name}={value}").as_bytes()))
                .collect();
            let stdin = bytes(&mut pool, case.stdin);
            let files: BTreeMap<String, Vec<Term>> = case
                .files
                .iter()
                .map(|(name, contents)| ((*name).to_owned(), bytes(&mut pool, contents.as_bytes())))
                .collect();
            let folders: BTreeSet<String> =
                case.folders.iter().map(|name| (*name).to_owned()).collect();
            let start = State::start(&image, &mut pool, &arguments, &environment, stdin)
                .with_files(files)
                .with_folders(folders);
            let finished = match explore(&image, start, &mut pool, &mut Concrete, 16) {
                Ok(finished) if finished.len() == 1 => finished,
                Ok(finished) => {
                    differs.push(format!("{what}: {} runs, not one", finished.len()));
                    continue;
                }
                Err(error) => {
                    differs.push(format!("{what}: {error}"));
                    continue;
                }
            };
            let run = &finished[0];
            let Some(End::Exit(status)) = &run.end else {
                differs.push(format!("{what}: it ended {:?}", run.end));
                continue;
            };
            if pool.constant(*status) != u128::try_from(case.exit).ok() {
                differs.push(format!(
                    "{what}: it exited with {:?}, not {}",
                    pool.constant(*status),
                    case.exit
                ));
            }
            let known = |terms: &[Term]| -> Option<Vec<u8>> {
                terms
                    .iter()
                    .map(|term| {
                        pool.constant(*term)
                            .and_then(|value| u8::try_from(value).ok())
                    })
                    .collect()
            };
            if known(&run.stdout).as_ref() != Some(&case.stdout) {
                differs.push(format!(
                    "{what}: it printed {:?}, not {:?}",
                    known(&run.stdout),
                    case.stdout
                ));
            }
            if !case.stderr.is_empty() && known(&run.stderr).as_deref() != Some(case.stderr) {
                differs.push(format!(
                    "{what}: it printed {:?} to standard error, not {:?}",
                    known(&run.stderr),
                    case.stderr
                ));
            }
        }
    }
    assert!(
        differs.is_empty(),
        "the model differs from the system in {} runs:\n{}",
        differs.len(),
        differs.join("\n")
    );
}
