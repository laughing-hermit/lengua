//! WASI preview 1 as lengua's host gives it: the module's name, the numbers and layouts of the
//! interface, and the function table, the shape of the native edge's [`crate::edge::Op`]. Each
//! function has its signature, what an idea names to allow it, the buffers it borrows from the
//! program's memory, and the errors it gives, a subset of lengua's closed set in WASI's own
//! numbering.
//!
//! Every function of preview 1 is in the table, so a module built by a C, C++ or Rust toolchain
//! links whatever it imports: one lengua doesn't give answers `NOSYS`, one the idea doesn't allow
//! answers `NOTCAPABLE`, and the few such toolchains call at startup answer harmlessly instead
//! (an empty environment, no folder, zeros for random bytes).

use crate::edge::Allowance;
use crate::idea::Use;

/// The module WASI preview 1's functions are imported from.
pub const MODULE: &str = "wasi_snapshot_preview1";

/// WASI's error numbers: the closed set's, in WASI's numbering.
pub mod errno {
    /// No error.
    pub const SUCCESS: u16 = 0;
    /// A name the program may not use, such as a link or a way out of its folder.
    pub const ACCES: u16 = 2;
    /// Nothing to do yet.
    pub const AGAIN: u16 = 6;
    /// A descriptor that isn't open, or isn't open for this.
    pub const BADF: u16 = 8;
    /// A file the program holds open.
    pub const BUSY: u16 = 10;
    /// A name that is taken.
    pub const EXIST: u16 = 20;
    /// A file past its size limit, or the files past their total.
    pub const FBIG: u16 = 22;
    /// A name that isn't text.
    pub const ILSEQ: u16 = 25;
    /// An argument that isn't one, or a name some system refuses.
    pub const INVAL: u16 = 28;
    /// An input or output error, and what any error outside a function's list becomes.
    pub const IO: u16 = 29;
    /// A folder where a file is wanted.
    pub const ISDIR: u16 = 31;
    /// Too many files open.
    pub const MFILE: u16 = 33;
    /// A name or path too long.
    pub const NAMETOOLONG: u16 = 37;
    /// No such file or folder.
    pub const NOENT: u16 = 44;
    /// A function lengua doesn't give.
    pub const NOSYS: u16 = 52;
    /// A file where a folder is wanted.
    pub const NOTDIR: u16 = 54;
    /// A folder with something in it.
    pub const NOTEMPTY: u16 = 55;
    /// A socket's function on what isn't a socket.
    pub const NOTSOCK: u16 = 57;
    /// Not a terminal.
    pub const NOTTY: u16 = 59;
    /// A write to a connection or pipe nobody reads.
    pub const PIPE: u16 = 64;
    /// A seek on something that has no places.
    pub const SPIPE: u16 = 70;
    /// What the program's idea doesn't allow.
    pub const NOTCAPABLE: u16 = 76;
}

/// Every error a function gives, in order of size: the closed set.
pub const ERRNOS: &[u16] = &[
    errno::ACCES,
    errno::AGAIN,
    errno::BADF,
    errno::BUSY,
    errno::EXIST,
    errno::FBIG,
    errno::ILSEQ,
    errno::INVAL,
    errno::IO,
    errno::ISDIR,
    errno::MFILE,
    errno::NAMETOOLONG,
    errno::NOENT,
    errno::NOSYS,
    errno::NOTDIR,
    errno::NOTEMPTY,
    errno::NOTSOCK,
    errno::NOTTY,
    errno::PIPE,
    errno::SPIPE,
    errno::NOTCAPABLE,
];

/// The kinds of thing a descriptor or a name is.
pub mod filetype {
    /// Anything else, such as a pipe.
    pub const UNKNOWN: u8 = 0;
    /// A terminal.
    pub const CHARACTER_DEVICE: u8 = 2;
    /// A folder.
    pub const DIRECTORY: u8 = 3;
    /// A file.
    pub const REGULAR_FILE: u8 = 4;
    /// A connection or a listener.
    pub const SOCKET_STREAM: u8 = 6;
    /// A link.
    pub const SYMBOLIC_LINK: u8 = 7;
}

/// The rights a descriptor reports, which lengua reports and doesn't enforce: its own rules do.
pub mod rights {
    /// Reading.
    pub const FD_READ: u64 = 1 << 1;
    /// Seeking.
    pub const FD_SEEK: u64 = 1 << 2;
    /// Telling the position.
    pub const FD_TELL: u64 = 1 << 5;
    /// Writing.
    pub const FD_WRITE: u64 = 1 << 6;
    /// Listing a folder.
    pub const FD_READDIR: u64 = 1 << 14;
    /// Reading a descriptor's size and kind.
    pub const FD_FILESTAT_GET: u64 = 1 << 21;
    /// Waiting on it with `poll_oneoff`.
    pub const POLL_FD_READWRITE: u64 = 1 << 27;
    /// Shutting a connection.
    pub const SOCK_SHUTDOWN: u64 = 1 << 28;
    /// Taking a connection.
    pub const SOCK_ACCEPT: u64 = 1 << 29;
    /// Every right of preview 1.
    pub const ALL: u64 = (1 << 30) - 1;
}

/// `path_open`'s ways to open.
pub mod oflags {
    /// Make the file when it isn't there.
    pub const CREAT: u16 = 1;
    /// It must be a folder.
    pub const DIRECTORY: u16 = 2;
    /// It must not be there yet.
    pub const EXCL: u16 = 4;
    /// Empty it.
    pub const TRUNC: u16 = 8;
}

/// A descriptor's flags.
pub mod fdflags {
    /// Every write goes at the end.
    pub const APPEND: u16 = 1;
    /// A call that would wait answers `AGAIN` instead.
    pub const NONBLOCK: u16 = 4;
}

/// Where `fd_seek` counts from.
pub mod whence {
    /// The start.
    pub const SET: u8 = 0;
    /// The position now.
    pub const CUR: u8 = 1;
    /// The end.
    pub const END: u8 = 2;
}

/// The clocks.
pub mod clock {
    /// The time of day.
    pub const REALTIME: u32 = 0;
    /// A clock that only moves forward.
    pub const MONOTONIC: u32 = 1;
}

/// `poll_oneoff`'s subscriptions and events.
pub mod event {
    /// A time.
    pub const CLOCK: u8 = 0;
    /// Something to read.
    pub const FD_READ: u8 = 1;
    /// Room to write.
    pub const FD_WRITE: u8 = 2;
    /// The other end has ended: an event's flag.
    pub const HANGUP: u16 = 1;
    /// A clock subscription's time is absolute: its flag.
    pub const ABSTIME: u16 = 1;
}

/// The flags of the socket functions.
pub mod sock {
    /// `sock_recv`: look without taking.
    pub const RECV_PEEK: u16 = 1;
    /// `sock_recv`: wait until the buffers are full.
    pub const RECV_WAITALL: u16 = 2;
    /// `sock_shutdown`: no more reading.
    pub const SHUT_RD: u8 = 1;
    /// `sock_shutdown`: no more writing.
    pub const SHUT_WR: u8 = 2;
}

/// `path_filestat_get`'s flag that follows a link at the end of the path.
pub const SYMLINK_FOLLOW: u32 = 1;

/// The sizes of the structures, in bytes.
pub mod size {
    /// An `iovec` or a `ciovec`: a buffer's address and length.
    pub const IOVEC: u32 = 8;
    /// An `fdstat`.
    pub const FDSTAT: u32 = 24;
    /// A `filestat`.
    pub const FILESTAT: u32 = 64;
    /// A `prestat`.
    pub const PRESTAT: u32 = 8;
    /// A `dirent`, before its name.
    pub const DIRENT: u32 = 24;
    /// A `subscription`.
    pub const SUBSCRIPTION: u32 = 48;
    /// An `event`.
    pub const EVENT: u32 = 32;
}

/// A WebAssembly value a function takes or gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    /// 32 bits.
    I32,
    /// 64 bits.
    I64,
}

/// How many bytes a buffer a function borrows holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bytes {
    /// The number in this other argument.
    Argument(usize),
    /// This many.
    Fixed(u32),
    /// A table of entries: as many as the number in this argument, each this big.
    Table {
        /// The argument that holds how many.
        count: usize,
        /// The size of one.
        entry: u32,
    },
    /// As many as the matching sizes call said, `args_sizes_get` or `environ_sizes_get`.
    Sized,
}

/// A buffer a function takes by its address in the program's memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lend {
    /// The argument that holds its address, counted from 0.
    pub argument: usize,
    /// How big it is.
    pub bytes: Bytes,
    /// Whether the function writes into it; otherwise it only reads it.
    pub written: bool,
}

/// What a function does when the program's idea doesn't allow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unasked {
    /// It answers `NOTCAPABLE` and does nothing.
    Refused,
    /// It answers harmlessly, as a toolchain's startup expects of it: no environment, no folder,
    /// zeros for random bytes.
    Harmless,
}

/// One function of WASI preview 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Function {
    /// Its name in [`MODULE`].
    pub name: &'static str,
    /// What it takes.
    pub params: &'static [Value],
    /// What it gives: an error number for every function but `proc_exit`.
    pub results: &'static [Value],
    /// What an idea names to allow it: any one of these.
    pub allowances: &'static [Allowance],
    /// The buffers it borrows directly; an `iovec`'s own buffers are the program's.
    pub lends: &'static [Lend],
    /// The errors it gives. Any other becomes `IO`.
    pub errors: &'static [u16],
    /// What it does when the idea doesn't allow it.
    pub unasked: Unasked,
    /// Whether lengua gives it: one it doesn't answers `NOSYS`.
    pub given: bool,
}

use Value::{I32, I64};
use errno::{
    ACCES, AGAIN, BADF, BUSY, EXIST, FBIG, ILSEQ, INVAL, IO, ISDIR, MFILE, NAMETOOLONG, NOENT,
    NOSYS, NOTCAPABLE, NOTDIR, NOTEMPTY, NOTSOCK, PIPE, SPIPE,
};

/// Every program may call it.
const ALWAYS: &[Allowance] = &[Allowance::Always];
/// A program that names files.
const FILES: &[Allowance] = &[Allowance::Use(Use::Files)];
/// A program on the network at all.
const NETWORK: &[Allowance] = &[Allowance::Network];
/// What a function that gives an error number gives.
const ERROR_NUMBER: &[Value] = &[I32];
/// No buffers.
const NO_LENDS: &[Lend] = &[];
/// What a function lengua doesn't give answers.
const UNGIVEN: &[u16] = &[NOSYS];

/// A buffer the function reads.
const fn read(argument: usize, bytes: Bytes) -> Lend {
    Lend {
        argument,
        bytes,
        written: false,
    }
}

/// A buffer the function writes.
const fn written(argument: usize, bytes: Bytes) -> Lend {
    Lend {
        argument,
        bytes,
        written: true,
    }
}

/// A function lengua gives.
const fn given(
    name: &'static str,
    params: &'static [Value],
    allowances: &'static [Allowance],
    lends: &'static [Lend],
    errors: &'static [u16],
) -> Function {
    Function {
        name,
        params,
        results: ERROR_NUMBER,
        allowances,
        lends,
        errors,
        unasked: Unasked::Refused,
        given: true,
    }
}

/// A function that answers harmlessly when its idea doesn't allow it.
const fn harmless(
    name: &'static str,
    params: &'static [Value],
    allowances: &'static [Allowance],
    lends: &'static [Lend],
    errors: &'static [u16],
) -> Function {
    Function {
        unasked: Unasked::Harmless,
        ..given(name, params, allowances, lends, errors)
    }
}

/// A function lengua doesn't give: it answers `NOSYS`.
const fn missing(name: &'static str, params: &'static [Value]) -> Function {
    Function {
        given: false,
        ..given(name, params, ALWAYS, NO_LENDS, UNGIVEN)
    }
}

/// An iovec table in argument 1 with its count in argument 2.
const IOVECS: Bytes = Bytes::Table {
    count: 2,
    entry: size::IOVEC,
};

/// Every function of WASI preview 1, by name.
pub const FUNCTIONS: &[Function] = &[
    given(
        "args_get",
        &[I32, I32],
        ALWAYS,
        &[written(0, Bytes::Sized), written(1, Bytes::Sized)],
        &[],
    ),
    given(
        "args_sizes_get",
        &[I32, I32],
        ALWAYS,
        &[written(0, Bytes::Fixed(4)), written(1, Bytes::Fixed(4))],
        &[],
    ),
    harmless(
        "environ_get",
        &[I32, I32],
        &[Allowance::Use(Use::Environment)],
        &[written(0, Bytes::Sized), written(1, Bytes::Sized)],
        &[],
    ),
    harmless(
        "environ_sizes_get",
        &[I32, I32],
        &[Allowance::Use(Use::Environment)],
        &[written(0, Bytes::Fixed(4)), written(1, Bytes::Fixed(4))],
        &[],
    ),
    given(
        "clock_res_get",
        &[I32, I32],
        &[Allowance::Use(Use::Clock)],
        &[written(1, Bytes::Fixed(8))],
        &[INVAL, NOTCAPABLE],
    ),
    given(
        "clock_time_get",
        &[I32, I64, I32],
        &[Allowance::Use(Use::Clock)],
        &[written(2, Bytes::Fixed(8))],
        &[INVAL, NOTCAPABLE],
    ),
    missing("fd_advise", &[I32, I64, I64, I32]),
    missing("fd_allocate", &[I32, I64, I64]),
    given(
        "fd_close",
        &[I32],
        &[Allowance::Use(Use::Files), Allowance::Network],
        &[],
        &[BADF, IO, NOTCAPABLE],
    ),
    missing("fd_datasync", &[I32]),
    given(
        "fd_fdstat_get",
        &[I32, I32],
        ALWAYS,
        &[written(1, Bytes::Fixed(size::FDSTAT))],
        &[BADF],
    ),
    missing("fd_fdstat_set_flags", &[I32, I32]),
    missing("fd_fdstat_set_rights", &[I32, I64, I64]),
    given(
        "fd_filestat_get",
        &[I32, I32],
        FILES,
        &[written(1, Bytes::Fixed(size::FILESTAT))],
        &[BADF, IO, NOTCAPABLE],
    ),
    missing("fd_filestat_set_size", &[I32, I64]),
    missing("fd_filestat_set_times", &[I32, I64, I64, I32]),
    missing("fd_pread", &[I32, I32, I32, I64, I32]),
    harmless(
        "fd_prestat_get",
        &[I32, I32],
        FILES,
        &[written(1, Bytes::Fixed(size::PRESTAT))],
        &[BADF],
    ),
    harmless(
        "fd_prestat_dir_name",
        &[I32, I32, I32],
        FILES,
        &[written(1, Bytes::Argument(2))],
        &[BADF, NAMETOOLONG],
    ),
    missing("fd_pwrite", &[I32, I32, I32, I64, I32]),
    given(
        "fd_read",
        &[I32, I32, I32, I32],
        &[
            Allowance::Use(Use::StandardInput),
            Allowance::Use(Use::Files),
            Allowance::Use(Use::Terminal),
            Allowance::Network,
        ],
        &[read(1, IOVECS), written(3, Bytes::Fixed(4))],
        &[AGAIN, BADF, INVAL, IO, ISDIR, NOTCAPABLE],
    ),
    given(
        "fd_readdir",
        &[I32, I32, I32, I64, I32],
        FILES,
        &[written(1, Bytes::Argument(2)), written(4, Bytes::Fixed(4))],
        &[BADF, IO, NOTDIR, NOTCAPABLE],
    ),
    missing("fd_renumber", &[I32, I32]),
    given(
        "fd_seek",
        &[I32, I64, I32, I32],
        FILES,
        &[written(3, Bytes::Fixed(8))],
        &[BADF, INVAL, IO, SPIPE, NOTCAPABLE],
    ),
    missing("fd_sync", &[I32]),
    given(
        "fd_tell",
        &[I32, I32],
        FILES,
        &[written(1, Bytes::Fixed(8))],
        &[BADF, IO, SPIPE, NOTCAPABLE],
    ),
    given(
        "fd_write",
        &[I32, I32, I32, I32],
        ALWAYS,
        &[read(1, IOVECS), written(3, Bytes::Fixed(4))],
        &[AGAIN, BADF, FBIG, INVAL, IO, PIPE, NOTCAPABLE],
    ),
    given(
        "path_create_directory",
        &[I32, I32, I32],
        FILES,
        &[read(1, Bytes::Argument(2))],
        &[
            ACCES,
            BADF,
            EXIST,
            ILSEQ,
            INVAL,
            IO,
            NAMETOOLONG,
            NOENT,
            NOTDIR,
            NOTCAPABLE,
        ],
    ),
    given(
        "path_filestat_get",
        &[I32, I32, I32, I32, I32],
        FILES,
        &[
            read(2, Bytes::Argument(3)),
            written(4, Bytes::Fixed(size::FILESTAT)),
        ],
        &[
            ACCES,
            BADF,
            ILSEQ,
            INVAL,
            IO,
            NAMETOOLONG,
            NOENT,
            NOTDIR,
            NOTCAPABLE,
        ],
    ),
    missing(
        "path_filestat_set_times",
        &[I32, I32, I32, I32, I64, I64, I32],
    ),
    missing("path_link", &[I32, I32, I32, I32, I32, I32, I32]),
    given(
        "path_open",
        &[I32, I32, I32, I32, I32, I64, I64, I32, I32],
        FILES,
        &[read(2, Bytes::Argument(3)), written(8, Bytes::Fixed(4))],
        &[
            ACCES,
            BADF,
            EXIST,
            ILSEQ,
            INVAL,
            IO,
            ISDIR,
            MFILE,
            NAMETOOLONG,
            NOENT,
            NOTDIR,
            NOTCAPABLE,
        ],
    ),
    missing("path_readlink", &[I32, I32, I32, I32, I32, I32]),
    given(
        "path_remove_directory",
        &[I32, I32, I32],
        FILES,
        &[read(1, Bytes::Argument(2))],
        &[
            ACCES,
            BADF,
            BUSY,
            ILSEQ,
            INVAL,
            IO,
            NAMETOOLONG,
            NOENT,
            NOTDIR,
            NOTEMPTY,
            NOTCAPABLE,
        ],
    ),
    given(
        "path_rename",
        &[I32, I32, I32, I32, I32, I32],
        FILES,
        &[read(1, Bytes::Argument(2)), read(4, Bytes::Argument(5))],
        &[
            ACCES,
            BADF,
            BUSY,
            EXIST,
            ILSEQ,
            INVAL,
            IO,
            ISDIR,
            NAMETOOLONG,
            NOENT,
            NOTDIR,
            NOTEMPTY,
            NOTCAPABLE,
        ],
    ),
    missing("path_symlink", &[I32, I32, I32, I32, I32]),
    given(
        "path_unlink_file",
        &[I32, I32, I32],
        FILES,
        &[read(1, Bytes::Argument(2))],
        &[
            ACCES,
            BADF,
            BUSY,
            ILSEQ,
            INVAL,
            IO,
            ISDIR,
            NAMETOOLONG,
            NOENT,
            NOTDIR,
            NOTCAPABLE,
        ],
    ),
    given(
        "poll_oneoff",
        &[I32, I32, I32, I32],
        &[
            Allowance::Use(Use::Clock),
            Allowance::Use(Use::StandardInput),
            Allowance::Use(Use::Terminal),
            Allowance::Network,
        ],
        &[
            read(
                0,
                Bytes::Table {
                    count: 2,
                    entry: size::SUBSCRIPTION,
                },
            ),
            written(
                1,
                Bytes::Table {
                    count: 2,
                    entry: size::EVENT,
                },
            ),
            written(3, Bytes::Fixed(4)),
        ],
        &[INVAL, NOTCAPABLE],
    ),
    Function {
        results: &[],
        ..given("proc_exit", &[I32], ALWAYS, &[], &[])
    },
    missing("proc_raise", &[I32]),
    given("sched_yield", &[], ALWAYS, &[], &[]),
    harmless(
        "random_get",
        &[I32, I32],
        &[Allowance::Use(Use::RandomBytes)],
        &[written(0, Bytes::Argument(1))],
        &[IO],
    ),
    given(
        "sock_accept",
        &[I32, I32, I32],
        &[Allowance::Listens],
        &[written(2, Bytes::Fixed(4))],
        &[AGAIN, BADF, INVAL, IO, MFILE, NOTSOCK, NOTCAPABLE],
    ),
    given(
        "sock_recv",
        &[I32, I32, I32, I32, I32, I32],
        NETWORK,
        &[
            read(1, IOVECS),
            written(4, Bytes::Fixed(4)),
            written(5, Bytes::Fixed(2)),
        ],
        &[AGAIN, BADF, INVAL, IO, NOTSOCK, NOTCAPABLE],
    ),
    given(
        "sock_send",
        &[I32, I32, I32, I32, I32],
        NETWORK,
        &[read(1, IOVECS), written(4, Bytes::Fixed(4))],
        &[AGAIN, BADF, INVAL, IO, PIPE, NOTSOCK, NOTCAPABLE],
    ),
    given(
        "sock_shutdown",
        &[I32, I32],
        NETWORK,
        &[],
        &[BADF, INVAL, NOTSOCK, NOTCAPABLE],
    ),
];

/// The function `name` names, if WASI preview 1 has one.
#[must_use]
pub fn function(name: &str) -> Option<&'static Function> {
    FUNCTIONS.iter().find(|function| function.name == name)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The errors the functions give are the closed set: none outside it, and every one of it
    /// given by some function, but `NOTTY`, kept for the terminal's two functions to come.
    #[test]
    fn the_errors_are_the_closed_set() {
        let mut used = BTreeSet::new();
        for function in FUNCTIONS {
            for errno in function.errors {
                assert!(ERRNOS.contains(errno), "{} gives {errno}", function.name);
                used.insert(*errno);
            }
        }
        let set: BTreeSet<u16> = ERRNOS
            .iter()
            .copied()
            .filter(|errno| *errno != errno::NOTTY)
            .collect();
        assert_eq!(used, set);
        assert!(ERRNOS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    /// Each name is there once, and every function lengua refuses can say so.
    #[test]
    fn the_names_are_one_each() {
        let names: BTreeSet<&str> = FUNCTIONS.iter().map(|function| function.name).collect();
        assert_eq!(names.len(), FUNCTIONS.len());
        // Preview 1 has 46 functions.
        assert_eq!(FUNCTIONS.len(), 46);
        for function in FUNCTIONS {
            assert_eq!(super::function(function.name), Some(function));
            let refusable = function.unasked == Unasked::Refused
                && function.given
                && function.allowances != ALWAYS;
            if refusable {
                assert!(
                    function.errors.contains(&NOTCAPABLE),
                    "{} can't say NOTCAPABLE",
                    function.name
                );
            }
            if !function.given {
                assert_eq!(function.errors, &[NOSYS], "{}", function.name);
            }
        }
        assert_eq!(super::function("fd_nothing"), None);
    }

    /// A buffer a function borrows is in one of its 32-bit arguments, and its size is in
    /// another.
    #[test]
    fn a_functions_lends_are_inside_its_arguments() {
        for function in FUNCTIONS {
            for lend in function.lends {
                assert_eq!(
                    function.params.get(lend.argument),
                    Some(&I32),
                    "{}",
                    function.name
                );
                match lend.bytes {
                    Bytes::Argument(other) | Bytes::Table { count: other, .. } => {
                        assert_eq!(function.params.get(other), Some(&I32), "{}", function.name);
                        assert_ne!(other, lend.argument, "{}", function.name);
                    }
                    Bytes::Fixed(_) | Bytes::Sized => {}
                }
            }
        }
    }

    /// Every function but `proc_exit` gives an error number.
    #[test]
    fn every_function_gives_an_error_number() {
        for function in FUNCTIONS {
            if function.name == "proc_exit" {
                assert!(function.results.is_empty());
            } else {
                assert_eq!(function.results, &[I32], "{}", function.name);
            }
        }
    }
}
