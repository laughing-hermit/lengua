//! WASI preview 1 as lengua's host gives it: the module's name, the numbers and layouts of the
//! interface, and the function table. Each function has its signature, what an idea names to
//! allow it ([`Allowance`]), the buffers it borrows from the program's memory, and the errors it
//! gives, a subset of lengua's closed set in WASI's own numbering.
//!
//! Every function of preview 1 is in the table, so a module built by a C, C++ or Rust toolchain
//! links whatever it imports: one lengua doesn't give answers `NOSYS`, one the idea doesn't allow
//! answers `NOTCAPABLE`, and the few such toolchains call at startup answer harmlessly instead
//! (an empty environment, no folder, zeros for random bytes).

use crate::idea::Use;

/// The module WASI preview 1's functions are imported from.
pub const MODULE: &str = "wasi_snapshot_preview1";

/// WASI's error numbers: the closed set's, in WASI's numbering.
pub mod errno {
    /// No error.
    pub const SUCCESS: u16 = 0;
    /// A name the program may not use: a link, what is neither a file nor a folder, or one the
    /// system refuses it.
    pub const ACCES: u16 = 2;
    /// Nothing to do yet.
    pub const AGAIN: u16 = 6;
    /// A descriptor that isn't open, or isn't open for this.
    pub const BADF: u16 = 8;
    /// A file the program holds open.
    pub const BUSY: u16 = 10;
    /// A place that didn't take the connection a program opened, or has nothing there for its
    /// datagrams.
    pub const CONNREFUSED: u16 = 14;
    /// A name that is taken.
    pub const EXIST: u16 = 20;
    /// A file past its size limit, or the files past their total, which a new file or folder
    /// counts against too.
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
    /// A datagram longer than one may be.
    pub const MSGSIZE: u16 = 35;
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
    /// A place that didn't answer in time.
    pub const TIMEDOUT: u16 = 73;
    /// What the program's idea doesn't allow, such as a way out of its working folder.
    pub const NOTCAPABLE: u16 = 76;
}

/// Every error a function gives, in order of size: the closed set.
pub const ERRNOS: &[u16] = &[
    errno::ACCES,
    errno::AGAIN,
    errno::BADF,
    errno::BUSY,
    errno::CONNREFUSED,
    errno::EXIST,
    errno::FBIG,
    errno::ILSEQ,
    errno::INVAL,
    errno::IO,
    errno::ISDIR,
    errno::MFILE,
    errno::MSGSIZE,
    errno::NAMETOOLONG,
    errno::NOENT,
    errno::NOSYS,
    errno::NOTDIR,
    errno::NOTEMPTY,
    errno::NOTSOCK,
    errno::NOTTY,
    errno::PIPE,
    errno::SPIPE,
    errno::TIMEDOUT,
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
    /// A socket for datagrams.
    pub const SOCKET_DGRAM: u8 = 5;
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

/// What an idea names to allow a function.
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
    /// A program that opens TCP connections itself: `opens TCP connections to …`.
    Opens,
    /// A program that looks up the addresses of names: `looks up the addresses of …`.
    Names,
    /// A program that sends UDP datagrams: `sends UDP datagrams to …`.
    Datagrams,
    /// A program that receives UDP datagrams on a port lengua binds: `receives UDP datagrams
    /// on …`.
    Receives,
    /// A program whose idea lets it open a file or a folder one of its arguments names: a
    /// `- Files:` line.
    Named,
    /// A program whose idea lets it open a folder one of its arguments names: `- Files: reads
    /// the folder its first argument names`.
    NamedFolder,
    /// A program whose idea lets it split its work into pieces: a `- Splits:` line.
    Splits,
    /// A program whose idea lets it call a module: a `- Calls:` line.
    Calls,
    /// A program whose idea lets it start a program: a `- Runs:` line.
    Runs,
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
/// A program that names files, or a folder one of its arguments names: what works on a folder
/// and the paths in it.
const FILES: &[Allowance] = &[Allowance::Use(Use::Files), Allowance::NamedFolder];
/// A program that holds files open: one that names files, or one whose arguments name them.
const HELD: &[Allowance] = &[Allowance::Use(Use::Files), Allowance::Named];
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
        &[
            Allowance::Use(Use::Files),
            Allowance::Network,
            Allowance::Named,
        ],
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
        HELD,
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
            Allowance::Named,
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
        HELD,
        &[written(3, Bytes::Fixed(8))],
        &[BADF, INVAL, IO, SPIPE, NOTCAPABLE],
    ),
    missing("fd_sync", &[I32]),
    given(
        "fd_tell",
        &[I32, I32],
        HELD,
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
            FBIG,
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
            FBIG,
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

/// The module lengua's own network functions are imported from. WASI preview 1 has no function
/// that opens a connection, so these aren't WASI's and don't take its module's name: a module
/// that imports one says so in its imports, and another WASI runtime refuses to link it rather
/// than give it something else. The name says whose they are, what they reach, and their
/// version: a change to what a function means gets a new number, never the same name.
pub const NET_MODULE: &str = "lengua_net_1";

/// The flags of [`NET_MODULE`]'s functions.
pub mod net {
    /// `tcp_open`: a plain connection.
    pub const PLAIN: u32 = 0;
    /// `tcp_open`: over TLS, which lengua does.
    pub const TLS: u32 = 1;
    /// An address's family: IPv4, its four bytes first.
    pub const FAMILY_4: u8 = 4;
    /// An address's family: IPv6.
    pub const FAMILY_6: u8 = 6;
}

use errno::{CONNREFUSED, MSGSIZE, TIMEDOUT};

/// lengua's own network functions, by name: what a program calls to reach what its idea grants
/// at run time. Each gives an error number, as WASI's do, from the same closed set.
///
/// - `tcp_open(name, name_len, port, tls, out_fd)`: opens a TCP connection to `port` of the
///   computer the text at `name` names, over TLS when `tls` is 1, and writes its descriptor,
///   which `sock_send`, `sock_recv`, `fd_read`, `fd_write`, `sock_shutdown`, `poll_oneoff` and
///   `fd_close` then take.
/// - `dns_lookup(name, name_len, out, out_len, out_count)`: writes the addresses of the name,
///   17 bytes each (the family, 4 or 6, then 16 bytes, an IPv4 address in the first four), as
///   many as `out_len` has room for, IPv4 first and each family in order, and how many it
///   wrote.
/// - `udp_open(name, name_len, port, out_fd)`: a socket for datagrams to and from `port` of
///   that one computer.
/// - `udp_send(fd, data, len, out_sent)` and `udp_recv(fd, buffer, len, out_size)`: one
///   datagram each; a received one longer than the buffer is cut, and `out_size` is its whole
///   size.
/// - `udp_recv_from(fd, buffer, len, out_size, out_from)` and `udp_send_to(fd, data, len, to,
///   out_sent)`: on the port the idea receives on, one datagram and its sender, 19 bytes (an
///   address as above, then the port, little-endian), and one datagram back to a sender.
pub const NET_FUNCTIONS: &[Function] = &[
    given(
        "tcp_open",
        &[I32, I32, I32, I32, I32],
        &[Allowance::Opens],
        &[read(0, Bytes::Argument(1)), written(4, Bytes::Fixed(4))],
        &[
            CONNREFUSED,
            INVAL,
            IO,
            MFILE,
            NAMETOOLONG,
            NOENT,
            TIMEDOUT,
            NOTCAPABLE,
        ],
    ),
    given(
        "dns_lookup",
        &[I32, I32, I32, I32, I32],
        &[Allowance::Names],
        &[
            read(0, Bytes::Argument(1)),
            written(2, Bytes::Argument(3)),
            written(4, Bytes::Fixed(4)),
        ],
        &[INVAL, IO, NAMETOOLONG, NOENT, TIMEDOUT, NOTCAPABLE],
    ),
    given(
        "udp_open",
        &[I32, I32, I32, I32],
        &[Allowance::Datagrams],
        &[read(0, Bytes::Argument(1)), written(3, Bytes::Fixed(4))],
        &[INVAL, IO, MFILE, NAMETOOLONG, NOENT, TIMEDOUT, NOTCAPABLE],
    ),
    given(
        "udp_send",
        &[I32, I32, I32, I32],
        &[Allowance::Datagrams],
        &[read(1, Bytes::Argument(2)), written(3, Bytes::Fixed(4))],
        &[BADF, IO, MSGSIZE, NOTSOCK, NOTCAPABLE],
    ),
    given(
        "udp_recv",
        &[I32, I32, I32, I32],
        &[Allowance::Datagrams],
        &[written(1, Bytes::Argument(2)), written(3, Bytes::Fixed(4))],
        &[BADF, CONNREFUSED, IO, NOTSOCK, NOTCAPABLE],
    ),
    given(
        "udp_recv_from",
        &[I32, I32, I32, I32, I32],
        &[Allowance::Receives],
        &[
            written(1, Bytes::Argument(2)),
            written(3, Bytes::Fixed(4)),
            written(4, Bytes::Fixed(crate::reach::PLACE_BYTES)),
        ],
        &[BADF, IO, NOTSOCK, NOTCAPABLE],
    ),
    given(
        "udp_send_to",
        &[I32, I32, I32, I32, I32],
        &[Allowance::Receives],
        &[
            read(1, Bytes::Argument(2)),
            read(3, Bytes::Fixed(crate::reach::PLACE_BYTES)),
            written(4, Bytes::Fixed(4)),
        ],
        &[BADF, INVAL, IO, MSGSIZE, NOTSOCK, NOTCAPABLE],
    ),
];

/// The module lengua's own functions for the files a program's arguments name are imported
/// from: WASI preview 1 opens a path beneath a folder it was given, and never one the user
/// names when the program runs.
pub const FS_MODULE: &str = "lengua_fs_1";

/// A program that opens a file or a folder one of its arguments names.
const NAMED: &[Allowance] = &[Allowance::Named];

/// lengua's own functions for the files and folders a program's arguments name. The program
/// gives the place of an argument, from 1, and never a path: lengua reads the argument, which
/// the user typed, and opens what it names.
///
/// - `file_open(argument, mode, out_fd)`: opens the file the argument at `argument` names, to
///   read (1), to write (2) or both, making it when it isn't there (4), emptying it (8), or
///   with every write at its end (16), as the idea's `- Files:` line for that argument allows,
///   and writes its descriptor, which `fd_read`, `fd_write`, `fd_seek`, `fd_tell`,
///   `fd_filestat_get` and `fd_close` then take.
/// - `folder_open(argument, out_fd)`: opens the folder the argument at `argument` names and
///   writes its descriptor, which the `path_` functions and `fd_readdir` take as the working
///   folder's descriptor is taken; one granted for reading alone takes no change.
pub const FS_FUNCTIONS: &[Function] = &[
    given(
        "file_open",
        &[I32, I32, I32],
        NAMED,
        &[written(2, Bytes::Fixed(4))],
        &[
            ACCES,
            EXIST,
            FBIG,
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
    given(
        "folder_open",
        &[I32, I32],
        &[Allowance::NamedFolder],
        &[written(1, Bytes::Fixed(4))],
        &[
            ACCES,
            ILSEQ,
            INVAL,
            IO,
            MFILE,
            NAMETOOLONG,
            NOENT,
            NOTDIR,
            NOTCAPABLE,
        ],
    ),
];

/// The module lengua's own function that starts a program is imported from: WASI preview 1 has
/// none.
pub const RUN_MODULE: &str = "lengua_run_1";

/// lengua's own function that starts a program the idea names, and waits for it.
///
/// - `program_run(command, command_len, input, input_len, output, output_len, out_size,
///   out_code)`: `command` holds the program's name and then each argument, each ended by a NUL
///   byte. lengua starts the program when a `- Runs:` line of the idea covers that command,
///   gives it the `input_len` bytes at `input` as its standard input, waits until it ends,
///   writes what it printed to `output`, how many bytes that is to `out_size`, and its exit
///   code to `out_code`. A program that runs past its line's time is ended (`TIMEDOUT`), and
///   so is one that prints more than `output_len` bytes (`FBIG`, with the buffer full).
pub const RUN_FUNCTIONS: &[Function] = &[given(
    "program_run",
    &[I32, I32, I32, I32, I32, I32, I32, I32],
    &[Allowance::Runs],
    &[
        read(0, Bytes::Argument(1)),
        read(2, Bytes::Argument(3)),
        written(4, Bytes::Argument(5)),
        written(6, Bytes::Fixed(4)),
        written(7, Bytes::Fixed(4)),
    ],
    &[FBIG, ILSEQ, INVAL, IO, NOENT, TIMEDOUT, NOTCAPABLE],
)];

/// The module lengua's own functions for the terminal are imported from: WASI preview 1 says
/// whether a descriptor is a terminal, and nothing of its mode or its size.
pub const TERM_MODULE: &str = "lengua_term_1";

/// The modes of [`TERM_MODULE`]'s `term_raw`.
pub mod term {
    /// The mode the terminal had: a line at a time, with echo.
    pub const AS_BEFORE: u32 = 0;
    /// Keys as they're pressed, with no echo and no line editing.
    pub const RAW: u32 = 1;
}

/// A program that names the terminal.
const TERMINAL: &[Allowance] = &[Allowance::Use(Use::Terminal)];

/// lengua's own functions for the terminal a program runs in, for an idea that uses `the
/// terminal`. A test and the `run` tool have no terminal.
///
/// - `term_is(fd, out)`: writes 1 when standard input (0), output (1) or error (2) is a
///   terminal, and 0 when it isn't.
/// - `term_size(out_columns, out_rows)`: writes the terminal's columns and rows; `NOTTY` when
///   there is no terminal.
/// - `term_raw(mode)`: with 1, standard input gives keys as they're pressed, with no echo and
///   no line editing; with 0, the terminal is as it was. `NOTTY` when standard input is no
///   terminal. lengua puts the terminal back as it was when the program ends, however it ends.
pub const TERM_FUNCTIONS: &[Function] = &[
    given(
        "term_is",
        &[I32, I32],
        TERMINAL,
        &[written(1, Bytes::Fixed(4))],
        &[BADF, NOTCAPABLE],
    ),
    given(
        "term_size",
        &[I32, I32],
        TERMINAL,
        &[written(0, Bytes::Fixed(4)), written(1, Bytes::Fixed(4))],
        &[errno::NOTTY, NOTCAPABLE],
    ),
    given(
        "term_raw",
        &[I32],
        TERMINAL,
        &[],
        &[INVAL, IO, errno::NOTTY, NOTCAPABLE],
    ),
];

/// The module lengua's own functions for work split into pieces are imported from: WASI
/// preview 1 has no threads, and lengua's pieces share no memory.
pub const SPLIT_MODULE: &str = "lengua_split_1";

/// A program whose idea lets it split its work.
const SPLITS: &[Allowance] = &[Allowance::Splits];

/// lengua's own functions for work split into pieces that run at once, for an idea with a
/// `- Splits:` line ([`crate::split`]).
///
/// - `split(input, input_len, pieces, output, output_len)`: cuts the `input_len` bytes at
///   `input` into `pieces` pieces that follow each other and differ in length by a byte at
///   most, runs the same module once for each, from `_start`, with memory of its own and the
///   piece's bytes as its standard input, at once on as many cores as the computer has, waits
///   for all, and writes what each printed to `output`, in the pieces' order, each at the
///   start of its `output_len / pieces` bytes, with zeros after it. `INVAL` when the idea's
///   line doesn't cover the call: more pieces than it grants or none, a piece longer than its
///   bytes, or an `output_len` other than the pieces times the bytes each gives back. A piece
///   that doesn't exit with code 0 ends the whole run as it ended, the first such piece in
///   the pieces' order, and nothing is written. In a piece it answers `NOTCAPABLE`: a piece
///   doesn't split.
/// - `piece()`: 0 in a run that is a piece of the work, and `NOENT` in the run that is the
///   whole program.
pub const SPLIT_FUNCTIONS: &[Function] = &[
    given(
        "split",
        &[I32, I32, I32, I32, I32],
        SPLITS,
        &[read(0, Bytes::Argument(1)), written(3, Bytes::Argument(4))],
        &[INVAL, NOTCAPABLE],
    ),
    given("piece", &[], SPLITS, NO_LENDS, &[NOENT, NOTCAPABLE]),
];

/// The module lengua's own functions for a called module's bytes are imported from: a called
/// module has a memory of its own, which the program reaches only through these two copies.
pub const CALL_MODULE: &str = "lengua_call_1";

/// A program whose idea lets it call a module.
const CALLS: &[Allowance] = &[Allowance::Calls];

/// lengua's own functions for the bytes of a module the program calls, for an idea with a
/// `- Calls:` line ([`crate::call`]). A called module is an instance of its own, with a memory
/// of its own: numbers pass in a call of one of its functions, and bytes pass by copy, here.
/// `module` counts the idea's `- Calls:` lines from 0, in their order.
///
/// - `give(module, at, from, len)`: copies the `len` bytes at `from` in the program's memory to
///   `at` in the called module's memory. `INVAL` when the idea calls no such module, the module
///   has no memory, or the bytes at `at` don't lie inside its memory, and nothing is copied.
/// - `take(module, at, into, len)`: copies the `len` bytes at `at` in the called module's
///   memory to `into` in the program's memory. `INVAL` as for `give`.
pub const CALL_FUNCTIONS: &[Function] = &[
    given(
        "give",
        &[I32, I32, I32, I32],
        CALLS,
        &[read(2, Bytes::Argument(3))],
        &[INVAL, NOTCAPABLE],
    ),
    given(
        "take",
        &[I32, I32, I32, I32],
        CALLS,
        &[written(2, Bytes::Argument(3))],
        &[INVAL, NOTCAPABLE],
    ),
];

/// lengua's own modules, each with its functions: what a program imports besides WASI preview
/// 1's, for what its idea grants it to reach by itself while it runs.
pub const OWN: &[(&str, &[Function])] = &[
    (NET_MODULE, NET_FUNCTIONS),
    (FS_MODULE, FS_FUNCTIONS),
    (RUN_MODULE, RUN_FUNCTIONS),
    (TERM_MODULE, TERM_FUNCTIONS),
    (SPLIT_MODULE, SPLIT_FUNCTIONS),
    (CALL_MODULE, CALL_FUNCTIONS),
];

/// Whether `module` is one of lengua's own.
#[must_use]
pub fn own_module(module: &str) -> bool {
    OWN.iter().any(|(name, _)| *name == module)
}

/// Every function of lengua's own modules, each with its module.
pub fn own_functions() -> impl Iterator<Item = (&'static str, &'static Function)> {
    OWN.iter()
        .flat_map(|(module, functions)| functions.iter().map(move |function| (*module, function)))
}

/// The function of lengua's host that `name` in `module` names: one of WASI preview 1, or one
/// of lengua's own.
#[must_use]
pub fn imported(module: &str, name: &str) -> Option<&'static Function> {
    if module == MODULE {
        return function(name);
    }
    own_functions()
        .find(|(from, function)| *from == module && function.name == name)
        .map(|(_, function)| function)
}

/// The module `function` is imported from.
#[must_use]
pub fn module_of(function: &Function) -> &'static str {
    own_functions()
        .find(|(_, own)| own.name == function.name)
        .map_or(MODULE, |(module, _)| module)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The errors the functions give, WASI's and lengua's own, are the closed set: none outside
    /// it, and every one of it given by some function.
    #[test]
    fn the_errors_are_the_closed_set() {
        let mut used = BTreeSet::new();
        let own = own_functions().map(|(_, function)| function);
        for function in FUNCTIONS.iter().chain(own) {
            for errno in function.errors {
                assert!(ERRNOS.contains(errno), "{} gives {errno}", function.name);
                used.insert(*errno);
            }
        }
        let set: BTreeSet<u16> = ERRNOS.iter().copied().collect();
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

    /// lengua's own functions are found in their own module alone, each once, under no name
    /// WASI has, and each can say `NOTCAPABLE`: none is every program's.
    #[test]
    fn lenguas_own_functions_are_in_their_own_module() {
        let names: BTreeSet<&str> = own_functions().map(|(_, function)| function.name).collect();
        assert_eq!(names.len(), own_functions().count());
        assert_eq!(names.len(), 17);
        for (module, function) in own_functions() {
            assert!(own_module(module) && module != MODULE);
            assert_eq!(imported(module, function.name), Some(function));
            assert_eq!(imported(MODULE, function.name), None, "{}", function.name);
            assert_eq!(module_of(function), module);
            for (other, _) in OWN.iter().filter(|(other, _)| *other != module) {
                assert_eq!(imported(other, function.name), None, "{}", function.name);
            }
            assert!(function.given && function.unasked == Unasked::Refused);
            assert_ne!(function.allowances, ALWAYS, "{}", function.name);
            assert!(function.errors.contains(&NOTCAPABLE), "{}", function.name);
            assert_eq!(function.results, &[I32], "{}", function.name);
        }
        assert_eq!(imported(MODULE, "fd_write"), function("fd_write"));
        assert_eq!(imported(NET_MODULE, "fd_write"), None);
        assert_eq!(imported("env", "tcp_open"), None);
        assert_eq!(imported(FS_MODULE, "tcp_open"), None);
        assert_eq!(imported(NET_MODULE, "file_open"), None);
        assert!(!own_module(MODULE) && !own_module("env"));
        assert_eq!(function("fd_write").map(module_of), Some(MODULE));
    }

    /// A buffer a function borrows is in one of its 32-bit arguments, and its size is in
    /// another.
    #[test]
    fn a_functions_lends_are_inside_its_arguments() {
        let own = own_functions().map(|(_, function)| function);
        for function in FUNCTIONS.iter().chain(own) {
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
