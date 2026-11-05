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

/// The descriptor of the channel to lengua's broker, which every operation refuses when a
/// program passes it: a program holds [`crate::runner::OPEN_FILES`] descriptors, 0 to 4,351.
pub const CHANNEL: u32 = 4352;

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
}

/// How a program is written: with its own system calls, or against the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// It defines `_start`, makes its own system calls and carries its own library.
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
        Profile::LinuxX86_64 => [
            include_str!("x86_64.s"),
            include_str!("linux-x86_64.s"),
        ],
        Profile::LinuxAarch64 => [
            include_str!("aarch64.s"),
            include_str!("linux-aarch64.s"),
        ],
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
    for snippet in [&library, &edge].into_iter().flat_map(|parts| &parts.snippets) {
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
                assert!(ERRORS.contains(errno), "{} gives error {errno}", op.symbol());
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
        assert_eq!(form("# main: a comment\n    call main_loop\n"), Form::Inline);
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
}
