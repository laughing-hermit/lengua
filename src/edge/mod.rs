//! The edge: lengua's own code between a program and the operating system.
//!
//! A program in the edge form defines `main` and reaches the OS only by calling one of 21
//! operations, `os_read`, `os_write` and the rest ([`Op`]). Each build appends to main.s a
//! second text, lengua.s ([`lengua_text`]): `_start`, the operations main.s calls, and the
//! library functions it calls and doesn't define (`assert_fail`, `fill`, `parallel_for`). They are
//! assembled together as one object, so main.s's line numbers do not move, and lengua's code is
//! the same for every program, written once per CPU (`x86_64.s`, `aarch64.s`) and once per
//! operating system and CPU (`linux-x86_64.s`, `linux-aarch64.s`).
//!
//! What a program may rely on from an operation is defined here once: the closed set of errors,
//! the open modes, the descriptor that is lengua's own, and the errors each operation gives.
//! Everything an operating system does differently is in its edge file alone.
//!
//! A program that defines `_start` is in the inline form: it makes its own system calls and
//! carries its own copies of the library, and nothing is appended to it. The two forms exist
//! side by side while programs move from one to the other.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::isa::Isa;
use crate::platform::Profile;
use crate::spec::{Network, Syscall, Use};

/// The descriptor of the channel to lengua's broker, which every operation refuses when a
/// program passes it: a program holds [`crate::runner::OPEN_FILES`] descriptors, 0 to 4,351.
pub const CHANNEL: u32 = 4352;

/// How big the stack is that lengua gives each thread `os_thread_start` starts: 1 MiB, of which a
/// worker has all but [`THREAD_STACK_HEAD`] bytes at the top.
pub const THREAD_STACK: i64 = 1 << 20;

/// What the thread's first frames take before its worker runs: the call of worker and a margin.
pub const THREAD_STACK_HEAD: i64 = 64;

/// The most bytes `os_random` fills in one call.
pub const RANDOM_MOST: u64 = 256;

/// How much of a test's standard input is in its pipe before the program starts, and how much
/// a write to a pipe is taken whole: one read then returns all of it.
pub const PIPE_WRITE: usize = 4096;

/// The modes `os_open` takes.
pub mod mode {
    /// An existing file, to read.
    pub const READ: u32 = 0;
    /// A file to write, made if it isn't there and emptied if it is.
    pub const WRITE: u32 = 1;
    /// A file to write at its end, made if it isn't there.
    pub const APPEND: u32 = 2;
    /// A file to write, which must not exist yet.
    pub const NEW: u32 = 3;
    /// A folder, to list.
    pub const FOLDER: u32 = 4;
}

/// What `os_info` says a name is: the second of its two words.
pub mod kind {
    /// A file.
    pub const FILE: u64 = 0;
    /// A folder.
    pub const FOLDER: u64 = 1;
    /// Anything else.
    pub const OTHER: u64 = 2;
}

/// The closed set of errors an operation gives, by Linux's numbers: an operation returns the
/// negative of one of them, and any error outside its own list becomes [`EIO`](error::EIO).
pub mod error {
    /// No such file or folder.
    pub const ENOENT: u32 = 2;
    /// An input or output error, and what any error outside an operation's list becomes.
    pub const EIO: u32 = 5;
    /// A descriptor that isn't open for this.
    pub const EBADF: u32 = 9;
    /// Nothing to do yet.
    pub const EAGAIN: u32 = 11;
    /// A name the program may not use.
    pub const EACCES: u32 = 13;
    /// A file the program holds open.
    pub const EBUSY: u32 = 16;
    /// A name that is taken.
    pub const EEXIST: u32 = 17;
    /// A folder where a file is wanted, or the other way round.
    pub const ENOTDIR: u32 = 20;
    /// A folder where a file is wanted.
    pub const EISDIR: u32 = 21;
    /// An argument that isn't one.
    pub const EINVAL: u32 = 22;
    /// Too many files open.
    pub const EMFILE: u32 = 24;
    /// Not a terminal.
    pub const ENOTTY: u32 = 25;
    /// A file past its size limit.
    pub const EFBIG: u32 = 27;
    /// No room left.
    pub const ENOSPC: u32 = 28;
    /// A seek on something that has no places.
    pub const ESPIPE: u32 = 29;
    /// A write to a connection or pipe nobody reads.
    pub const EPIPE: u32 = 32;
    /// A name too long.
    pub const ENAMETOOLONG: u32 = 36;
    /// A folder with something in it.
    pub const ENOTEMPTY: u32 = 39;
    /// A name that isn't text.
    pub const EILSEQ: u32 = 84;
    /// A connection the other end reset.
    pub const ECONNRESET: u32 = 104;
    /// A connection that isn't made.
    pub const ENOTCONN: u32 = 107;
}

/// Every error, in order of size.
pub const ERRORS: [u32; 21] = [
    error::ENOENT,
    error::EIO,
    error::EBADF,
    error::EAGAIN,
    error::EACCES,
    error::EBUSY,
    error::EEXIST,
    error::ENOTDIR,
    error::EISDIR,
    error::EINVAL,
    error::EMFILE,
    error::ENOTTY,
    error::EFBIG,
    error::ENOSPC,
    error::ESPIPE,
    error::EPIPE,
    error::ENAMETOOLONG,
    error::ENOTEMPTY,
    error::EILSEQ,
    error::ECONNRESET,
    error::ENOTCONN,
];

/// An operation a program asks of the operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Op {
    /// `os_read(fd, buffer, length)`: how many bytes it read, 0 at the end.
    Read,
    /// `os_write(fd, bytes, length)`: how many bytes it wrote, possibly fewer than asked.
    Write,
    /// `os_open(name, mode)`: a descriptor.
    Open,
    /// `os_close(fd)`: 0.
    Close,
    /// `os_seek(fd, offset, whence)`: the new offset.
    Seek,
    /// `os_info(fd, out)`: 0, with the file's size and kind in the 16 bytes at `out`.
    Info,
    /// `os_list(folder, buffer, length)`: how many bytes of entries, 0 at the end.
    List,
    /// `os_mkdir(name)`: 0.
    Mkdir,
    /// `os_remove(name, kind)`: 0.
    Remove,
    /// `os_rename(from, to)`: 0.
    Rename,
    /// `os_clock(clock, out)`: 0, with seconds and nanoseconds in the 16 bytes at `out`.
    Clock,
    /// `os_sleep(clock, absolute, time)`: 0, once the time has come.
    Sleep,
    /// `os_random(buffer, length)`: how many bytes it filled, all of them up to
    /// [`RANDOM_MOST`].
    Random,
    /// `os_accept(listener)`: the next connection's descriptor.
    Accept,
    /// `os_shutdown(fd, how)`: 0.
    Shutdown,
    /// `os_poll(table, count, milliseconds)`: how many entries have an event.
    Poll,
    /// `os_terminal(mode)`: 0, or `-ENOTTY`.
    Terminal,
    /// `os_window(out)`: 0, with rows and columns in the 8 bytes at `out`, or `-ENOTTY`.
    Window,
    /// `os_thread_start(index, slot)`: 0. Only lengua's library starts threads.
    ThreadStart,
    /// `os_thread_join(index)`: what the worker returned.
    ThreadJoin,
    /// `os_exit(status)`: never returns.
    Exit,
}

impl Op {
    /// Every operation, in the order lengua.s writes them.
    pub const ALL: [Self; 21] = [
        Self::Read,
        Self::Write,
        Self::Open,
        Self::Close,
        Self::Seek,
        Self::Info,
        Self::List,
        Self::Mkdir,
        Self::Remove,
        Self::Rename,
        Self::Clock,
        Self::Sleep,
        Self::Random,
        Self::Accept,
        Self::Shutdown,
        Self::Poll,
        Self::Terminal,
        Self::Window,
        Self::ThreadStart,
        Self::ThreadJoin,
        Self::Exit,
    ];

    /// The name a program calls it by.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Read => "os_read",
            Self::Write => "os_write",
            Self::Open => "os_open",
            Self::Close => "os_close",
            Self::Seek => "os_seek",
            Self::Info => "os_info",
            Self::List => "os_list",
            Self::Mkdir => "os_mkdir",
            Self::Remove => "os_remove",
            Self::Rename => "os_rename",
            Self::Clock => "os_clock",
            Self::Sleep => "os_sleep",
            Self::Random => "os_random",
            Self::Accept => "os_accept",
            Self::Shutdown => "os_shutdown",
            Self::Poll => "os_poll",
            Self::Terminal => "os_terminal",
            Self::Window => "os_window",
            Self::ThreadStart => "os_thread_start",
            Self::ThreadJoin => "os_thread_join",
            Self::Exit => "os_exit",
        }
    }

    /// The operation a name is, if it is one.
    #[must_use]
    pub fn from_symbol(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|op| op.symbol() == name)
    }

    /// The errors it gives, the negative of one of them in rax or x0. Any other error the
    /// operating system gives becomes [`error::EIO`], on the error path only.
    #[must_use]
    pub const fn errors(self) -> &'static [u32] {
        use error::{
            EACCES, EAGAIN, EBADF, EBUSY, ECONNRESET, EEXIST, EFBIG, EILSEQ, EINVAL, EIO, EISDIR,
            EMFILE, ENAMETOOLONG, ENOENT, ENOSPC, ENOTCONN, ENOTDIR, ENOTEMPTY, ENOTTY, EPIPE,
            ESPIPE,
        };
        match self {
            Self::Read => &[EIO, EBADF, EAGAIN, EISDIR, EINVAL, ECONNRESET],
            Self::Write => &[EIO, EBADF, EAGAIN, EFBIG, ENOSPC, EPIPE, EINVAL, ECONNRESET],
            Self::Open => &[
                ENOENT,
                EIO,
                EACCES,
                EBUSY,
                EEXIST,
                ENOTDIR,
                EISDIR,
                EINVAL,
                EMFILE,
                ENOSPC,
                ENAMETOOLONG,
                EILSEQ,
            ],
            Self::Close | Self::Info => &[EIO, EBADF],
            Self::Seek => &[EBADF, EINVAL, ESPIPE],
            Self::List => &[EIO, EBADF, ENOTDIR, EINVAL],
            Self::Mkdir => &[
                ENOENT,
                EIO,
                EACCES,
                EEXIST,
                ENOTDIR,
                EINVAL,
                ENOSPC,
                ENAMETOOLONG,
                EILSEQ,
            ],
            Self::Remove => &[
                ENOENT,
                EIO,
                EACCES,
                EBUSY,
                ENOTDIR,
                EISDIR,
                EINVAL,
                ENOTEMPTY,
                ENAMETOOLONG,
                EILSEQ,
            ],
            Self::Rename => &[
                ENOENT,
                EIO,
                EACCES,
                EBUSY,
                EEXIST,
                ENOTDIR,
                EISDIR,
                EINVAL,
                ENOSPC,
                ENOTEMPTY,
                ENAMETOOLONG,
                EILSEQ,
            ],
            Self::Clock | Self::Sleep | Self::Poll | Self::ThreadJoin => &[EINVAL],
            Self::Random => &[EIO, EINVAL],
            Self::Accept => &[EIO, EBADF, EAGAIN, EINVAL, EMFILE, ECONNRESET],
            Self::Shutdown => &[EBADF, EINVAL, ENOTCONN],
            Self::Terminal | Self::Window => &[ENOTTY],
            Self::ThreadStart => &[EAGAIN, EINVAL],
            Self::Exit => &[],
        }
    }

    /// [`Self::errors`] as two words of bits, errors 0 to 63 and 64 to 127, which the edge
    /// tests an error against.
    #[must_use]
    pub fn error_mask(self) -> [u64; 2] {
        let mut mask = [0_u64; 2];
        for errno in self.errors() {
            mask[usize::from(*errno >= 64)] |= 1_u64 << (errno % 64);
        }
        mask
    }

    /// How many arguments it takes, in the registers of the CPU's convention.
    #[must_use]
    pub const fn arguments(self) -> usize {
        match self {
            Self::Read | Self::Write | Self::Seek | Self::List | Self::Sleep | Self::Poll => 3,
            Self::Open
            | Self::Info
            | Self::Remove
            | Self::Rename
            | Self::Clock
            | Self::Random
            | Self::Shutdown
            | Self::ThreadStart => 2,
            Self::Close
            | Self::Mkdir
            | Self::Accept
            | Self::Terminal
            | Self::Window
            | Self::ThreadJoin
            | Self::Exit => 1,
        }
    }

    /// The buffers it takes by their address, which it reads or writes through.
    #[must_use]
    pub const fn lends(self) -> &'static [Lend] {
        match self {
            Self::Read | Self::List => &[Lend {
                argument: 1,
                bytes: Bytes::Argument(2),
                written: true,
            }],
            Self::Write => &[Lend {
                argument: 1,
                bytes: Bytes::Argument(2),
                written: false,
            }],
            Self::Open | Self::Mkdir | Self::Remove => &[Lend {
                argument: 0,
                bytes: Bytes::Name,
                written: false,
            }],
            Self::Rename => &[
                Lend {
                    argument: 0,
                    bytes: Bytes::Name,
                    written: false,
                },
                Lend {
                    argument: 1,
                    bytes: Bytes::Name,
                    written: false,
                },
            ],
            Self::Info | Self::Clock => &[Lend {
                argument: 1,
                bytes: Bytes::Fixed(16),
                written: true,
            }],
            Self::Sleep => &[Lend {
                argument: 2,
                bytes: Bytes::Fixed(16),
                written: false,
            }],
            Self::Random => &[Lend {
                argument: 0,
                bytes: Bytes::Argument(1),
                written: true,
            }],
            Self::Poll => &[Lend {
                argument: 0,
                bytes: Bytes::Table { count: 1, entry: 8 },
                written: true,
            }],
            Self::Window => &[Lend {
                argument: 0,
                bytes: Bytes::Fixed(8),
                written: true,
            }],
            // The slot is the program's own and only its worker looks into it.
            Self::Close
            | Self::Seek
            | Self::Accept
            | Self::Shutdown
            | Self::Terminal
            | Self::ThreadStart
            | Self::ThreadJoin
            | Self::Exit => &[],
        }
    }

    /// What a spec names to allow it: any one of these.
    #[must_use]
    pub const fn allowances(self) -> &'static [Allowance] {
        use crate::spec::Use::{Clock, Files, RandomBytes, StandardInput, Terminal};
        match self {
            Self::Write | Self::Exit => &[Allowance::Always],
            Self::Read => &[
                Allowance::Use(StandardInput),
                Allowance::Use(Files),
                Allowance::Use(Terminal),
                Allowance::Network,
            ],
            Self::Open
            | Self::Seek
            | Self::Info
            | Self::List
            | Self::Mkdir
            | Self::Remove
            | Self::Rename => &[Allowance::Use(Files)],
            Self::Close => &[Allowance::Use(Files), Allowance::Network],
            Self::Clock | Self::Sleep => &[Allowance::Use(Clock)],
            Self::Random => &[Allowance::Use(RandomBytes)],
            Self::Accept => &[Allowance::Listens],
            Self::Shutdown => &[Allowance::Network],
            Self::Poll => &[Allowance::Use(Terminal), Allowance::AtOnce],
            Self::Terminal | Self::Window => &[Allowance::Use(Terminal)],
            Self::ThreadStart | Self::ThreadJoin => &[Allowance::Threads],
        }
    }

    /// The Linux system calls it makes, in the vocabulary of a spec's `Syscall`s: none for the
    /// thread operations, whose `clone` and `futex` a spec never names.
    #[must_use]
    pub const fn syscalls(self) -> &'static [Syscall] {
        match self {
            Self::Read => &[Syscall::Read],
            Self::Write => &[Syscall::Write],
            Self::Open => &[Syscall::Openat],
            Self::Close => &[Syscall::Close],
            Self::Seek => &[Syscall::Lseek],
            Self::Info => &[Syscall::Fstat],
            Self::List => &[Syscall::Getdents64],
            Self::Mkdir => &[Syscall::Mkdirat],
            Self::Remove => &[Syscall::Unlinkat],
            Self::Rename => &[Syscall::Renameat],
            Self::Clock => &[Syscall::ClockGettime],
            Self::Sleep => &[Syscall::ClockNanosleep],
            Self::Random => &[Syscall::Getrandom],
            Self::Accept => &[Syscall::Accept4],
            Self::Shutdown => &[Syscall::Shutdown],
            Self::Poll => &[Syscall::Poll],
            Self::Terminal | Self::Window => &[Syscall::Ioctl],
            Self::ThreadStart | Self::ThreadJoin => &[],
            Self::Exit => &[Syscall::ExitGroup],
        }
    }
}

/// What a spec names to allow an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allowance {
    /// Every program may call it.
    Always,
    /// A program that names this capability.
    Use(Use),
    /// A program on the network at all, with connections or serving.
    Network,
    /// A program that serves: lengua gives it a listening socket.
    Listens,
    /// A server of several clients at once.
    AtOnce,
    /// A program that may run threads: only lengua's library calls the operation.
    Threads,
}

/// The operations a spec allows: those any of whose [`Allowance`]s it meets.
#[must_use]
pub fn ops_for(uses: &[Use], network: &Network, threads: Option<u32>) -> Vec<Op> {
    Op::ALL
        .into_iter()
        .filter(|op| {
            op.allowances().iter().any(|allowance| match allowance {
                Allowance::Always => true,
                Allowance::Use(found) => uses.contains(found),
                Allowance::Network => network.is_used(),
                Allowance::Listens => network.listens.is_some(),
                Allowance::AtOnce => network.at_once.is_some(),
                Allowance::Threads => threads.is_some(),
            })
        })
        .collect()
}

/// How many bytes a buffer an operation is lent holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bytes {
    /// The number in this other argument.
    Argument(usize),
    /// This many.
    Fixed(u64),
    /// A name: up to and including its NUL.
    Name,
    /// A table of entries: as many as the number in this argument, each this big.
    Table {
        /// The argument that holds how many.
        count: usize,
        /// The size of one.
        entry: u64,
    },
}

/// A buffer an operation takes by its address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lend {
    /// The argument that holds its address, counted from 0.
    pub argument: usize,
    /// How big it is.
    pub bytes: Bytes,
    /// Whether the operation writes through it, and may read it too; otherwise it only reads.
    pub written: bool,
}

/// The library functions a program in the edge form may call, besides the operations: lengua's
/// own, so main.s defines none of them.
pub const LIBRARY: [&str; 3] = ["assert_fail", "fill", "parallel_for"];

/// The operations one of the library's functions calls, which a program that calls it needs
/// allowed.
#[must_use]
pub fn library_calls(name: &str) -> &'static [Op] {
    match name {
        "fill" => &[Op::Read],
        "parallel_for" => &[Op::ThreadStart, Op::ThreadJoin],
        _ => &[],
    }
}

/// Whether lengua keeps `name` for itself in a program's text: `_start`, the operations and
/// their tables, the library's functions and data, and what carries lengua's prefix.
#[must_use]
pub fn reserved(name: &str) -> bool {
    ["_start", "assert_message"].contains(&name)
        || LIBRARY.contains(&name)
        || ["os_", "lengua_", "LENGUA_", ".Llengua_"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

/// How a program is written: with its own system calls, or against the edge.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// It defines `_start`, makes its own system calls and carries its own library.
    #[default]
    Inline,
    /// It defines `main` and calls the edge.
    Edge,
}

/// Whether `text` defines the label `name` at the start of a line.
fn defines(text: &str, name: &str) -> bool {
    text.lines().any(|line| {
        line.trim_start()
            .strip_prefix(name)
            .is_some_and(|rest| rest.starts_with(':'))
    })
}

/// The form main.s is in: the edge form when it defines `main` and not `_start`.
#[must_use]
pub fn form(main: &str) -> Form {
    if defines(main, "main") && !defines(main, "_start") {
        Form::Edge
    } else {
        Form::Inline
    }
}

/// A piece of an edge file: what it holds between one `== name` line and the next.
struct Snippet {
    /// What it needs, besides itself, to be linked.
    needs: Vec<String>,
    /// Its text, which begins by naming the section it is in.
    text: String,
}

/// An edge file read as what comes before its first snippet, and its snippets by name.
struct Parts {
    header: String,
    snippets: BTreeMap<String, Snippet>,
}

/// Reads an edge file: a line `<comment> == name: needed other` starts the snippet `name`, which
/// runs to the next such line, and what comes before the first is the header.
fn parse(file: &str, comment: &str) -> Parts {
    let marker = format!("{comment} == ");
    let mut parts = Parts {
        header: String::new(),
        snippets: BTreeMap::new(),
    };
    let mut current: Option<String> = None;
    for line in file.lines() {
        if let Some(rest) = line.strip_prefix(&marker) {
            let (name, needs) = rest.split_once(':').unwrap_or((rest, ""));
            let name = name.trim().to_owned();
            parts.snippets.insert(
                name.clone(),
                Snippet {
                    needs: needs.split_whitespace().map(str::to_owned).collect(),
                    text: String::new(),
                },
            );
            current = Some(name);
        } else if let Some(name) = &current {
            if let Some(snippet) = parts.snippets.get_mut(name) {
                snippet.text.push_str(line);
                snippet.text.push('\n');
            }
        } else {
            parts.header.push_str(line);
            parts.header.push('\n');
        }
    }
    parts
}

/// The files for a target: the CPU's library, then the operating system's edge on it.
const fn files(profile: Profile) -> [&'static str; 2] {
    match profile {
        Profile::LinuxX86_64 => [include_str!("x86_64.s"), include_str!("linux-x86_64.s")],
        Profile::LinuxAarch64 => [include_str!("aarch64.s"), include_str!("linux-aarch64.s")],
    }
}

/// What starts a comment in a CPU's assembly.
const fn comment(isa: Isa) -> &'static str {
    match isa {
        Isa::X86_64 => "#",
        Isa::Aarch64 => "//",
    }
}

/// The symbols main.s branches to, by `call` and `jmp` on x86-64 and `bl` and `b` on AArch64.
fn branch_targets(main: &str, isa: Isa) -> BTreeSet<String> {
    let branches: &[&str] = match isa {
        Isa::X86_64 => &["call", "jmp"],
        Isa::Aarch64 => &["bl", "b"],
    };
    main.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let (mnemonic, target) = (words.next()?, words.next()?);
            branches
                .contains(&mnemonic)
                .then(|| target.trim_end_matches(',').to_owned())
        })
        .collect()
}

/// The words of a data block giving each linked operation's errors as bits, which `lengua_error`
/// tests an error against.
fn error_tables(ops: &[Op]) -> String {
    let mut text = String::from(".section .rodata\n.balign 8\n");
    for op in ops.iter().filter(|op| !op.errors().is_empty()) {
        let [low, high] = op.error_mask();
        let symbol = format!("{}_errors", op.symbol());
        let _ = write!(
            text,
            ".type {symbol}, @object\n{symbol}:\n    .quad {low:#018x}, {high:#018x}\n\
             .size {symbol}, . - {symbol}\n"
        );
    }
    text
}

/// lengua.s for a main.s in the edge form, else `None`: `_start`, every operation main.s calls,
/// the library functions it calls and doesn't define, and what they need, in the same order
/// every time, so that the checker can match it line by line to what was assembled.
#[must_use]
pub fn lengua_text(profile: Profile, main: &str) -> Option<String> {
    if form(main) != Form::Edge {
        return None;
    }
    let isa = profile.isa();
    let comment = comment(isa);
    let [library, edge] = files(profile).map(|file| parse(file, comment));
    let mut available: BTreeMap<&str, &Snippet> = BTreeMap::new();
    for snippet in [&library, &edge]
        .into_iter()
        .flat_map(|parts| &parts.snippets)
    {
        available.insert(snippet.0.as_str(), snippet.1);
    }
    let mut wanted: BTreeSet<String> = BTreeSet::from(["_start".to_owned()]);
    for target in branch_targets(main, isa) {
        if available.contains_key(target.as_str()) && !defines(main, &target) {
            wanted.insert(target);
        }
    }
    let mut pending: Vec<String> = wanted.iter().cloned().collect();
    while let Some(name) = pending.pop() {
        let snippet = available
            .get(name.as_str())
            .unwrap_or_else(|| panic!("the edge has no {name}"));
        for need in &snippet.needs {
            if wanted.insert(need.clone()) {
                pending.push(need.clone());
            }
        }
    }
    let ops: Vec<Op> = Op::ALL
        .into_iter()
        .filter(|op| wanted.contains(op.symbol()))
        .collect();
    let mut text = String::new();
    text.push_str(&library.header);
    text.push_str(&edge.header);
    // `_start`, then the operations in their order, then everything else by name.
    let mut order: Vec<&str> = vec!["_start"];
    order.extend(ops.iter().map(|op| op.symbol()));
    order.extend(
        wanted
            .iter()
            .map(String::as_str)
            .filter(|name| *name != "_start" && Op::from_symbol(name).is_none()),
    );
    for name in order {
        text.push_str(&available[name].text);
    }
    text.push_str(&error_tables(&ops));
    Some(text)
}

/// Whether a program in the edge form links the channel's code, and so talks to lengua's broker
/// where it has one: the programs that open files, make or remove folders, ask the clock or for
/// random bytes, sleep, wait for a connection or list a folder.
#[must_use]
pub fn uses_channel(profile: Profile, main: &str) -> bool {
    lengua_text(profile, main).is_some_and(|text| text.contains("lengua_channel_ready:"))
}

/// The text a program is built from: main.s, and for a program in the edge form lengua.s after it,
/// for the CPU main.s's first line names and the system this is. It is what lengua assembles, so
/// that a program can be built outside lengua.
///
/// # Errors
///
/// Fails when main.s doesn't start with a CPU's first line, or lengua has no edge for this system.
pub fn linked(main: &str) -> Result<String, String> {
    let (isa, arch) = [(Isa::X86_64, "x86_64"), (Isa::Aarch64, "aarch64")]
        .into_iter()
        .find(|(isa, _)| main.starts_with(crate::checker::source::header(*isa)))
        .ok_or(
            "main.s doesn't start with a CPU's first line: .intel_syntax noprefix or .arch armv8-a",
        )?;
    let profile = Profile::for_target(std::env::consts::OS, arch).ok_or_else(|| {
        format!(
            "lengua has no edge for {} on this system yet",
            match isa {
                Isa::X86_64 => "x86-64",
                Isa::Aarch64 => "AArch64",
            }
        )
    })?;
    Ok(match lengua_text(profile, main) {
        Some(text) if main.ends_with('\n') => format!("{main}{text}"),
        Some(text) => format!("{main}\n{text}"),
        None => main.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The errors the operations give are exactly the closed set: no operation names one outside
    /// it and none of the set is unused.
    #[test]
    fn the_errors_are_the_closed_set() {
        let mut used: BTreeSet<u32> = BTreeSet::new();
        for op in Op::ALL {
            for errno in op.errors() {
                assert!(
                    ERRORS.contains(errno),
                    "{} gives error {errno}",
                    op.symbol()
                );
                used.insert(*errno);
            }
        }
        let set: BTreeSet<u32> = ERRORS.into_iter().collect();
        assert_eq!(used, set);
        assert!(ERRORS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    /// Every error an operation gives is a bit of its mask, and no other.
    #[test]
    fn a_mask_is_its_errors() {
        for op in Op::ALL {
            let mask = op.error_mask();
            for errno in 0..128_u32 {
                let set = mask[usize::from(errno >= 64)] >> (errno % 64) & 1 == 1;
                assert_eq!(set, op.errors().contains(&errno), "{} {errno}", op.symbol());
            }
        }
    }

    /// An operation has the name its symbol says, and only one.
    #[test]
    fn the_symbols_are_one_each() {
        let names: BTreeSet<&str> = Op::ALL.iter().map(|op| op.symbol()).collect();
        assert_eq!(names.len(), Op::ALL.len());
        for op in Op::ALL {
            assert!(op.symbol().starts_with("os_"));
            assert_eq!(Op::from_symbol(op.symbol()), Some(op));
        }
        assert_eq!(Op::from_symbol("os_nothing"), None);
        assert_eq!(Op::from_symbol("write"), None);
    }

    /// The form is the edge's when main.s defines `main` and not `_start`.
    #[test]
    fn the_entry_symbol_says_the_form() {
        assert_eq!(form(".text\nmain:\n    ret\n"), Form::Edge);
        assert_eq!(form(".text\n_start:\n    ret\n"), Form::Inline);
        // A mix is the inline form's: the checker refuses it.
        assert_eq!(form("_start:\n    ret\nmain:\n    ret\n"), Form::Inline);
        assert_eq!(
            form("# main: a comment\n    call main_loop\n"),
            Form::Inline
        );
        assert_eq!(form("    main: nop\n"), Form::Edge);
    }

    /// Nothing is appended to an inline program, and an edge one gets `_start` and what it calls.
    #[test]
    fn lengua_text_is_for_the_edge_form_alone() {
        for profile in Profile::ALL {
            assert_eq!(lengua_text(profile, "_start:\n    ret\n"), None);
            let call = match profile.isa() {
                Isa::X86_64 => "call os_exit",
                Isa::Aarch64 => "bl os_exit",
            };
            let main = format!("main:\n    {call}\n");
            let text = lengua_text(profile, &main).expect("the edge form");
            assert!(text.contains("_start:"), "{text}");
            assert!(text.contains("os_exit:"), "{text}");
            assert!(!text.contains("os_write:"), "{text}");
            // The same text every time.
            assert_eq!(lengua_text(profile, &main), Some(text));
        }
    }

    /// The `.equ NAME, value` lines of an edge file's header, by name.
    fn constants(header: &str) -> BTreeMap<String, i64> {
        header
            .lines()
            .filter_map(|line| line.strip_prefix(".equ "))
            .map(|rest| {
                let (name, value) = rest.split_once(',').expect("a name and a value");
                let (name, value) = (name.trim(), value.trim());
                let (negative, digits) = value
                    .strip_prefix('-')
                    .map_or((false, value), |digits| (true, digits));
                let magnitude = digits
                    .strip_prefix("0x")
                    .map_or_else(|| digits.parse::<i64>(), |hex| i64::from_str_radix(hex, 16))
                    .unwrap_or_else(|_| panic!("{name} is {value}"));
                (
                    name.to_owned(),
                    if negative { -magnitude } else { magnitude },
                )
            })
            .collect()
    }

    /// A CPU's system calls by the names the edge files give them.
    macro_rules! system_calls {
        ($module:ident: $($name:ident),+) => {
            &[$((stringify!($name), crate::syscalls::$module::$name)),+]
        };
    }

    /// What an edge file holds in `.equ` lines is what the tables hold: the system calls' numbers
    /// (which `syscalls`' own tests hold to the kernel's headers), the closed set of errors, the
    /// descriptor that is lengua's own, the most random bytes, the thread flags and the kinds.
    #[test]
    fn the_edge_files_agree_with_the_tables() {
        const NAMES: [(&str, u32); 21] = [
            ("ENOENT", error::ENOENT),
            ("EIO", error::EIO),
            ("EBADF", error::EBADF),
            ("EAGAIN", error::EAGAIN),
            ("EACCES", error::EACCES),
            ("EBUSY", error::EBUSY),
            ("EEXIST", error::EEXIST),
            ("ENOTDIR", error::ENOTDIR),
            ("EISDIR", error::EISDIR),
            ("EINVAL", error::EINVAL),
            ("EMFILE", error::EMFILE),
            ("ENOTTY", error::ENOTTY),
            ("EFBIG", error::EFBIG),
            ("ENOSPC", error::ENOSPC),
            ("ESPIPE", error::ESPIPE),
            ("EPIPE", error::EPIPE),
            ("ENAMETOOLONG", error::ENAMETOOLONG),
            ("ENOTEMPTY", error::ENOTEMPTY),
            ("EILSEQ", error::EILSEQ),
            ("ECONNRESET", error::ECONNRESET),
            ("ENOTCONN", error::ENOTCONN),
        ];
        let big = |number: u64| i64::try_from(number).expect("a number that fits");
        for profile in Profile::ALL {
            let [_, edge] = files(profile);
            let found = constants(&parse(edge, comment(profile.isa())).header);
            let calls: &[(&str, i64)] = match profile {
                Profile::LinuxX86_64 => system_calls!(
                    x86_64: READ, WRITE, CLOSE, FSTAT, POLL, LSEEK, IOCTL, SHUTDOWN, CLONE, EXIT,
                    FUTEX, CLOCK_GETTIME, CLOCK_NANOSLEEP, EXIT_GROUP, MKDIRAT, UNLINKAT, RENAMEAT,
                    GETDENTS64, OPENAT, ACCEPT4, GETRANDOM, SENDMSG, RECVMSG
                ),
                Profile::LinuxAarch64 => system_calls!(
                    aarch64: IOCTL, MKDIRAT, UNLINKAT, RENAMEAT, OPENAT, CLOSE, GETDENTS64, LSEEK,
                    READ, WRITE, PPOLL, FSTAT, EXIT, EXIT_GROUP, FUTEX, CLOCK_GETTIME,
                    CLOCK_NANOSLEEP, SHUTDOWN, ACCEPT4, CLONE, GETRANDOM, SENDMSG, RECVMSG
                ),
            };
            for (name, value) in &found {
                let what = format!("{profile:?} {name}");
                let rest = name.strip_prefix("LENGUA_").expect("lengua's own prefix");
                if let Some(call) = rest.strip_prefix("SYS_") {
                    let number = calls
                        .iter()
                        .find(|(known, _)| *known == call)
                        .map(|(_, number)| *number);
                    assert_eq!(Some(*value), number, "{what}");
                } else if let Some((_, errno)) = NAMES.iter().find(|(known, _)| *known == rest) {
                    assert_eq!(*value, -i64::from(*errno), "{what}");
                }
            }
            let used = found
                .keys()
                .filter(|name| name.starts_with("LENGUA_SYS_"))
                .count();
            assert_eq!(
                used,
                calls.len(),
                "{profile:?}: a system call in one place only"
            );
            assert_eq!(found["LENGUA_CHANNEL"], i64::from(CHANNEL), "{profile:?}");
            assert_eq!(found["LENGUA_RANDOM_MOST"], big(RANDOM_MOST), "{profile:?}");
            assert_eq!(
                found["LENGUA_CLONE_FLAGS"],
                big(Profile::CLONE_FLAGS),
                "{profile:?}"
            );
            // AArch64 builds the flags in two halves, as one instruction can't hold them.
            if let (Some(low), Some(high)) = (
                found.get("LENGUA_CLONE_FLAGS_LOW"),
                found.get("LENGUA_CLONE_FLAGS_HIGH"),
            ) {
                assert_eq!(
                    low | (high << 16),
                    found["LENGUA_CLONE_FLAGS"],
                    "{profile:?}"
                );
            }
            assert_eq!(found["LENGUA_KIND_FILE"], big(kind::FILE), "{profile:?}");
            assert_eq!(
                found["LENGUA_KIND_FOLDER"],
                big(kind::FOLDER),
                "{profile:?}"
            );
            assert_eq!(found["LENGUA_KIND_OTHER"], big(kind::OTHER), "{profile:?}");
            // The channel's messages are the broker's, and its flags are the kernel's.
            let size = |bytes: usize| i64::try_from(bytes).expect("a size");
            let ask = |kind: crate::broker::Kind| kind as i64;
            let widen = i64::from;
            let reply_at = size(
                (crate::broker::REQUEST_LEN + crate::broker::MAX_BODY).next_multiple_of(8),
            );
            let wire: [(&str, i64); 28] = [
                ("LENGUA_MAGIC", i64::from(crate::broker::MAGIC)),
                ("LENGUA_VERSION", i64::from(crate::broker::VERSION)),
                ("LENGUA_HELLO_LEN", size(crate::broker::HELLO_LEN)),
                ("LENGUA_REQUEST_LEN", size(crate::broker::REQUEST_LEN)),
                ("LENGUA_REPLY_LEN", size(crate::broker::REPLY_LEN)),
                ("LENGUA_PATH_MAX", size(crate::names::MAX_PATH)),
                ("LENGUA_PAYLOAD_MAX", big(RANDOM_MOST)),
                ("LENGUA_ASK_OPEN", ask(crate::broker::Kind::Open)),
                ("LENGUA_ASK_MKDIR", ask(crate::broker::Kind::Mkdir)),
                ("LENGUA_ASK_REMOVE", ask(crate::broker::Kind::Remove)),
                ("LENGUA_ASK_RENAME", ask(crate::broker::Kind::Rename)),
                ("LENGUA_ASK_CLOCK", ask(crate::broker::Kind::Clock)),
                ("LENGUA_ASK_RANDOM", ask(crate::broker::Kind::Random)),
                ("LENGUA_NOTE_CLOSED", ask(crate::broker::Kind::Closed)),
                ("LENGUA_NOTE_WAITING", ask(crate::broker::Kind::Waiting)),
                ("LENGUA_NOTE_LISTED", ask(crate::broker::Kind::Listed)),
                ("LENGUA_NOTE_RESUMED", ask(crate::broker::Kind::Resumed)),
                ("LENGUA_FLAG_REVERSED", widen(crate::broker::flag::REVERSED)),
                ("LENGUA_FLAG_RANDOM", widen(crate::broker::flag::RANDOM)),
                ("LENGUA_FLAG_CLOCK", widen(crate::broker::flag::CLOCK)),
                // From the kernel's headers, the same on both CPUs.
                ("LENGUA_MSG_DONTWAIT", 0x40),
                ("LENGUA_MSG_WAITALL", 0x100),
                ("LENGUA_MSG_NOSIGNAL", 0x4000),
                ("LENGUA_SOL_SOCKET", 1),
                ("LENGUA_SCM_RIGHTS", 1),
                // The edge's own: where the reply and its payload are in the wire's buffer, which
                // holds a request of two paths first.
                ("LENGUA_REPLY_AT", reply_at),
                ("LENGUA_PAYLOAD_AT", reply_at + size(crate::broker::REPLY_LEN)),
                ("LENGUA_LOOKED", 1 << 31),
            ];
            for (name, wanted) in wire {
                assert_eq!(found[name], wanted, "{profile:?} {name}");
            }
        }
    }

    /// Each OS's edge file has `_start` and every operation, the CPU's library and the edge file
    /// share no name, what a snippet needs is there, and the names are the reserved ones.
    #[test]
    fn the_files_hold_every_snippet_once() {
        for profile in Profile::ALL {
            let [library, edge] = files(profile).map(|file| parse(file, comment(profile.isa())));
            for op in Op::ALL {
                assert!(
                    edge.snippets.contains_key(op.symbol()),
                    "{profile:?} {}",
                    op.symbol()
                );
            }
            assert!(edge.snippets.contains_key("_start"), "{profile:?}");
            for name in edge.snippets.keys() {
                assert!(
                    !library.snippets.contains_key(name),
                    "{profile:?} {name} is in both"
                );
                assert!(
                    name == "_start"
                        || name.starts_with("lengua_")
                        || Op::from_symbol(name).is_some(),
                    "{profile:?} {name} is not an operation"
                );
            }
            for name in library.snippets.keys() {
                assert!(
                    ["assert_fail", "fill", "parallel_for"].contains(&name.as_str()),
                    "{profile:?} {name} is not a library function"
                );
            }
            for snippet in library.snippets.values().chain(edge.snippets.values()) {
                for need in &snippet.needs {
                    assert!(
                        library.snippets.contains_key(need) || edge.snippets.contains_key(need),
                        "{profile:?} needs {need}"
                    );
                }
            }
        }
    }

    /// A main.s that calls everything the edge offers gets all of it, and no label is twice in it.
    #[test]
    fn everything_links_on_every_cpu() {
        for profile in Profile::ALL {
            let call = match profile.isa() {
                Isa::X86_64 => "call",
                Isa::Aarch64 => "bl",
            };
            let mut names: Vec<&str> = Op::ALL.iter().map(|op| op.symbol()).collect();
            names.extend(["assert_fail", "fill", "parallel_for"]);
            let mut main = String::from("main:\n");
            for name in &names {
                let _ = writeln!(main, "    {call} {name}");
            }
            let text = lengua_text(profile, &main).expect("the edge form");
            let mut labels: BTreeSet<&str> = BTreeSet::new();
            let defined = text
                .lines()
                .filter(|line| !line.starts_with(char::is_whitespace))
                .filter_map(|line| line.split_once(':'))
                .map(|(label, _)| label)
                .filter(|label| !label.is_empty() && !label.contains(char::is_whitespace));
            for label in defined {
                assert!(
                    labels.insert(label),
                    "{profile:?}: {label} is defined twice"
                );
            }
            for name in &names {
                assert!(labels.contains(name), "{profile:?} lacks {name}");
            }
        }
    }

    /// What each capability allows, by operation, is what it allows by system call: the
    /// operations make the capability's calls and the output and exit every program has.
    #[test]
    fn a_use_allows_the_operations_its_calls_need() {
        let none = Network::default();
        for found in Use::ALL {
            let needed: BTreeSet<&str> = ops_for(&[found], &none, None)
                .iter()
                .flat_map(|op| op.syscalls())
                .map(|call| call.name())
                .collect();
            let allowed: BTreeSet<&str> = found
                .syscalls()
                .iter()
                .chain(&[Syscall::Write, Syscall::ExitGroup])
                .map(|call| call.name())
                .collect();
            assert_eq!(needed, allowed, "{}", found.name());
        }
    }

    /// A program on the network is allowed the operations its calls need, and a server of several
    /// clients the wait; the calls the spec also lists, `sendto` for a write on a connection and the
    /// thread's own `exit`, no operation makes.
    #[test]
    fn the_network_allows_the_operations_its_calls_need() {
        let client = Network {
            connections: vec![crate::spec::Endpoint {
                host: None,
                argument: None,
                port: 80,
                tls: false,
            }],
            ..Network::default()
        };
        let server = Network {
            listens: Some(8080),
            ..Network::default()
        };
        let busy = Network {
            at_once: Some(64),
            ..server.clone()
        };
        let calls = |network: &Network| -> BTreeSet<&'static str> {
            ops_for(&[], network, None)
                .iter()
                .flat_map(|op| op.syscalls())
                .map(|call| call.name())
                .collect()
        };
        let base = ["write", "exit_group", "read", "close", "shutdown"];
        assert_eq!(calls(&client), BTreeSet::from(base));
        let mut serving = BTreeSet::from(base);
        serving.insert("accept4");
        assert_eq!(calls(&server), serving);
        serving.insert("poll");
        assert_eq!(calls(&busy), serving);
        assert_eq!(
            calls(&Network::default()),
            BTreeSet::from(["write", "exit_group"])
        );
    }

    /// The thread operations are allowed by threads and by nothing else.
    #[test]
    fn threads_allow_the_thread_operations() {
        let none = Network::default();
        let on = ops_for(&[], &none, Some(4));
        assert!(on.contains(&Op::ThreadStart) && on.contains(&Op::ThreadJoin));
        let off = ops_for(&Use::ALL, &none, None);
        assert!(!off.contains(&Op::ThreadStart) && !off.contains(&Op::ThreadJoin));
    }

    /// A buffer an operation takes is in one of its arguments, and its size is in another.
    #[test]
    fn an_operations_lends_are_inside_its_arguments() {
        for op in Op::ALL {
            for lend in op.lends() {
                assert!(lend.argument < op.arguments(), "{}", op.symbol());
                match lend.bytes {
                    Bytes::Argument(other) => {
                        assert!(
                            other < op.arguments() && other != lend.argument,
                            "{}",
                            op.symbol()
                        );
                    }
                    Bytes::Table { count, .. } => {
                        assert!(
                            count < op.arguments() && count != lend.argument,
                            "{}",
                            op.symbol()
                        );
                    }
                    Bytes::Fixed(_) | Bytes::Name => {}
                }
            }
        }
    }

    /// A CPU's edge convention takes its arguments where a system call does, changes what a system
    /// call changes and its own scratch, and none of what a function gives back.
    #[test]
    fn the_edge_convention_keeps_what_a_function_gives_back() {
        for isa in [Isa::X86_64, Isa::Aarch64] {
            let abi = isa.abi();
            assert_eq!(abi.edge_arguments, abi.syscall_arguments, "{isa:?}");
            assert!(abi.edge_clobbers.contains(&abi.result), "{isa:?}");
            for register in abi.syscall_clobbers {
                assert!(abi.edge_clobbers.contains(register), "{isa:?} {register:?}");
            }
            for register in abi.callee_saved {
                assert!(
                    !abi.edge_clobbers.contains(register),
                    "{isa:?} {register:?}"
                );
            }
            for op in Op::ALL {
                assert!(
                    op.arguments() <= abi.edge_arguments.len(),
                    "{isa:?} {}",
                    op.symbol()
                );
            }
        }
    }

    /// A program in the edge form is built from main.s and lengua.s after it; one in the inline
    /// form from main.s alone; and main.s names its CPU.
    #[test]
    fn a_program_is_linked_with_lengua_s_only_in_the_edge_form() {
        if Profile::for_target(std::env::consts::OS, "x86_64").is_none() {
            return;
        }
        let edge = ".intel_syntax noprefix\n.text\nmain:\n    call os_exit\n";
        let linked = linked(edge).expect("the edge form links");
        assert!(linked.starts_with(edge), "{linked}");
        assert!(
            linked.contains("_start:") && linked.contains("os_exit:"),
            "{linked}"
        );
        let inline = ".intel_syntax noprefix\n.text\n_start:\n    ret\n";
        assert_eq!(linked_inline(inline), inline);
        assert!(super::linked("main:\n").is_err());
    }

    fn linked_inline(text: &str) -> String {
        super::linked(text).expect("the inline form is its own text")
    }
}
