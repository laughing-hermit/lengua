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

use lengua::platform::Profile;
use lengua::runner::{End, Job, Limits, run};
use lengua::spec::Syscall;
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

/// The registers a call's arguments go in, on x86-64.
const X86_ARGUMENTS: [&str; 6] = ["rdi", "rsi", "rdx", "r10", "r8", "r9"];

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
    /// Stores a number, in 1, 2, 4 or 8 bytes, that many bytes into the scratch area.
    Set { offset: u32, bytes: u8, value: i64 },
}

use Arg::{Argv, Data, Envp, Kept, N, Scratch};
use Step::{Call, Set};

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
    arguments: &'static [&'static str],
    environment: &'static [(&'static str, &'static str)],
    files: &'static [(&'static str, &'static str)],
    folders: &'static [&'static str],
    /// The files the working folder holds afterwards, when the case says; none, when it doesn't.
    after: &'static [(&'static str, &'static str)],
    /// The system calls its operations make, besides `write` and `exit_group`.
    calls: &'static [Syscall],
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
            arguments: &[],
            environment: &[],
            files: &[],
            folders: &[],
            after: &[],
            calls: &[],
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
        let _ = writeln!(text, "{name}:\n    .byte {}", list.join(", "));
    }
    text
}

/// The case as an x86-64 program.
fn x86_program(case: &Case) -> String {
    let mut text = String::from(".intel_syntax noprefix\n.text\n.type main, @function\nmain:\n");
    let _ = writeln!(text, "    sub rsp, {}", SCRATCH + 8);
    // main's arguments to the scratch area's last words, and the rest zeroed.
    text.push_str("    mov r8, rdi\n    mov r9, rsi\n    mov r10, rdx\n");
    text.push_str("    xor eax, eax\n    mov rdi, rsp\n    mov ecx, 64\n    rep stosq\n");
    let _ = writeln!(text, "    mov qword ptr [rsp + {ARGC_AT}], r8");
    let _ = writeln!(text, "    mov qword ptr [rsp + {ARGV_AT}], r9");
    let _ = writeln!(text, "    mov qword ptr [rsp + {ENVP_AT}], r10");
    for step in case.steps {
        match step {
            Call { op, args, keep } => {
                for (arg, register) in args.iter().zip(X86_ARGUMENTS) {
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
                            let _ =
                                writeln!(text, "    mov {register}, qword ptr [rsp + {}]", 8 * word);
                        }
                        Argv(k) | Envp(k) => {
                            let at = if matches!(arg, Argv(_)) { ARGV_AT } else { ENVP_AT };
                            let _ = writeln!(text, "    mov {register}, qword ptr [rsp + {at}]");
                            let _ =
                                writeln!(text, "    mov {register}, qword ptr [{register} + {}]", 8 * k);
                        }
                    }
                }
                let _ = writeln!(text, "    call {op}");
                if let Some(word) = keep {
                    let _ = writeln!(text, "    mov qword ptr [rsp + {}], rax", 8 * word);
                }
            }
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
    text.push_str(&data(case));
    text
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
    for step in case.steps {
        match step {
            Call { op, args, keep } => {
                for (number, arg) in args.iter().enumerate() {
                    match arg {
                        N(value) => {
                            let _ = writeln!(text, "    mov x{number}, #{value}");
                        }
                        Data(name) => {
                            let _ = writeln!(
                                text,
                                "    adrp x{number}, {name}\n    add x{number}, x{number}, :lo12:{name}"
                            );
                        }
                        Scratch(offset) => {
                            let _ = writeln!(text, "    add x{number}, sp, #{offset}");
                        }
                        Kept(word) => {
                            let _ = writeln!(text, "    ldr x{number}, [sp, #{}]", 8 * word);
                        }
                        Argv(k) | Envp(k) => {
                            let at = if matches!(arg, Argv(_)) { ARGV_AT } else { ENVP_AT };
                            let _ = writeln!(text, "    ldr x{number}, [sp, #{at}]");
                            let _ = writeln!(text, "    ldr x{number}, [x{number}, #{}]", 8 * k);
                        }
                    }
                }
                let _ = writeln!(text, "    bl {op}");
                if let Some(word) = keep {
                    let _ = writeln!(text, "    str x0, [sp, #{}]", 8 * word);
                }
            }
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
    text.push_str(&data(case));
    text
}

fn program(target: Profile, case: &Case) -> String {
    match target {
        Profile::LinuxX86_64 => x86_program(case),
        Profile::LinuxAarch64 => a64_program(case),
    }
}

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
        // A wait on standard input that has a byte and has ended (data and hang-up, 1 and 16),
        // and one on descriptor 4,352, which is refused.
        Case {
            stdin: b"x",
            calls: &[Syscall::Poll],
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
                    &[(64, &[0, 0, 0, 0, 1, 0, 17, 0]), (72, &[0, 0x11, 0, 0, 1, 0, 0, 0])],
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
        // A file read, its size and kind, a seek, a descriptor closed and used again, a name
        // that isn't there (ENOENT) and a mode that is none (EINVAL). The descriptor itself
        // is kept in a word the report leaves out.
        Case {
            data: &[("in", b"in.txt\0"), ("none", b"none\0")],
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
            data: &[("out", b"out.txt\0"), ("new", b"new.txt\0"), ("greeting", b"hello\n")],
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
        // Listing folders: one entry of a file, one of a folder, a folder with nothing in it
        // (`.` and `..` are left out, so the first read has nothing and the next is the end), a
        // file that is no folder (ENOTDIR), a descriptor that isn't open, and the kind and size of
        // a folder, a file and a pipe.
        Case {
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
                        (256, &[16, 0, 0, 8, b'o', b'n', b'l', b'y', b'.', b't', b'x', b't', 0, 0, 0, 0]),
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
            let executable =
                per_target::build(target, &place, case.name, &program(target, &case)).await;
            built.push((target, executable));
        }
        for (target, executable) in built {
            let Some(how) = per_target::way(target) else {
                continue;
            };
            let mut spec = vec![Syscall::Write, Syscall::Exit];
            spec.extend_from_slice(case.calls);
            let allowed = target.allowed_syscalls(&spec, false);
            let arguments: Vec<String> = case.arguments.iter().map(|text| (*text).to_owned()).collect();
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
            let job = Job {
                emulated: how == Way::Emulated,
                env: &environment,
                files: &files,
                folders: &folders,
                ..Job::new(&executable, &arguments, case.stdin, &allowed)
            };
            let ran = run(launcher(), job, Limits::default())
                .await
                .expect("the program runs");
            let what = format!("{} on {}", case.name, target.name());
            assert_eq!(ran.end, End::Exited(i32::try_from(case.exit).unwrap_or(-1)), "{what}");
            assert_eq!(ran.stdout, case.stdout, "{what}");
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
