//! lengua's WASI preview 1 host in the model: each function as src/wasm/host gives it, with the
//! same checks in the same order, over the model's linear memory, and what it does to the world
//! done by the world's models ([`World`]) through [`Host`]. The world numbers its descriptors and
//! its errors as Linux does; lengua's host numbers descriptors by role (0 to 2 the standard
//! streams, 3 the working folder, 4 the listener, 5 on the connections lengua made) and answers
//! WASI's errors, so the model keeps the program's descriptors in a table of its own, each tied to
//! the world's, and translates the world's errors by one table ([`LINUX_TO_WASI`]). A name that
//! leaves the working folder is `NOTCAPABLE` before the world sees it, as the host resolves names
//! itself, and no world's error is ever translated to it or to `ACCES`, a link's.

use std::collections::BTreeMap;
use std::rc::Rc;

use super::Wasm;
use super::image::Module;
use super::memory::{self, OUTSIDE};
use crate::prove::exec::{End, State, Step, Stop, unsupported};
use crate::prove::term::{Pool, Term};
use crate::prove::world::{Host, Reading, World};
use crate::runner::{OPEN_FILES, TEST_FILE_LIMIT};
use crate::wasm::host::fds::FOLDER;
use crate::wasm::host::files::names_in;
use crate::wasm::host::limits::{ENTRY, TEST_WRITTEN};
use crate::wasm::host::net::CHUNK;
use crate::wasm::wasi::{
    Function, Unasked, clock, errno, event, fdflags, filetype, oflags, rights, size, sock, whence,
};

/// Each error number the world's models give, as Linux numbers it, and WASI's for it, as lengua's
/// host answers: the one table the model translates by. A number not in it is `IO`, as an error
/// outside a function's list is.
const LINUX_TO_WASI: &[(i64, u16)] = &[
    (2, errno::NOENT),
    (9, errno::BADF),
    (11, errno::AGAIN),
    (16, errno::BUSY),
    (17, errno::EXIST),
    (20, errno::NOTDIR),
    (21, errno::ISDIR),
    (22, errno::INVAL),
    (25, errno::NOTTY),
    (32, errno::PIPE),
    (36, errno::NAMETOOLONG),
    (39, errno::NOTEMPTY),
    (84, errno::ILSEQ),
];

/// The world's descriptor of the listener, as its network keeps it.
const LISTENED: u64 = 3;

/// `unlinkat`'s flag that removes a folder, as the world's model of it takes it.
const REMOVE_FOLDER: u64 = 0x200;

/// Linux's `SHUT_WR`, as the world's `shutdown` takes it.
const SHUT_WRITE: u64 = 1;

/// Why a read the world would take every count of is beyond the model: a proof reads lengua's
/// `$fill` to the end of the input as a whole instead ([`super::fill`]), and a module that reads
/// any other way is proven for its input arriving whole.
const EVERY_CUT: &str = "a read of standard input whose every count a proof takes, which the \
                         WebAssembly model doesn't follow: read with $fill to the end of the \
                         input to be proven however it arrives";

/// How a run ends that waits on poll_oneoff for what doesn't come before a test's time limit
/// stops it.
pub(super) const NEVER_COMES: &str =
    "it waits on poll_oneoff for what doesn't come in a test's time";

/// What a name in the working folder is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Found {
    /// A file.
    File,
    /// A folder.
    Folder,
}

impl Found {
    /// WASI's number for it.
    const fn filetype(self) -> u8 {
        match self {
            Self::File => filetype::REGULAR_FILE,
            Self::Folder => filetype::DIRECTORY,
        }
    }
}

/// One entry of a folder's listing: its name and what it is.
pub(super) type Entry = (String, Found);

/// What a descriptor the program holds is, as lengua's host's table has it.
#[derive(Debug, Clone)]
pub(super) enum Handle {
    /// Standard input.
    Stdin,
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
    /// A folder: the working folder, or one in it the program opened.
    Folder {
        /// Its names from the working folder: none for the working folder.
        path: Vec<String>,
        /// Its entries as `fd_readdir` last began listing them.
        listing: Option<Rc<Vec<Entry>>>,
    },
    /// A file the program opened.
    File {
        /// The world's descriptor of it.
        world: u64,
        /// Its name in the working folder, its names joined by slashes.
        name: String,
        /// Its names from the working folder.
        path: Vec<String>,
        /// Whether the program may read it.
        read: bool,
        /// Whether the program may write it.
        write: bool,
        /// Whether every write goes at its end.
        append: bool,
        /// Where the next write goes, for a file open to write.
        position: u64,
    },
    /// The listener a server's connections come in on.
    Listener,
    /// A connection: one lengua made, or one the program accepted.
    Socket {
        /// The world's descriptor of it.
        world: u64,
        /// Whether the program has shut its reading.
        read_shut: bool,
        /// Whether the program has shut its writing.
        write_shut: bool,
    },
}

/// The connection lengua made for the program `index`th, as the world keeps it.
pub(super) fn made(index: u32) -> Handle {
    let first = u64::try_from(crate::connect::FIRST_DESCRIPTOR).unwrap_or(4);
    Handle::Socket {
        world: first + u64::from(index),
        read_shut: false,
        write_shut: false,
    }
}

/// What a host call comes to when it doesn't succeed.
enum Fail {
    /// An error for the program, a WASI error number.
    Errno(u16),
    /// The run's end: a trap, a stop, or what the model doesn't follow.
    End(End),
}

impl From<End> for Fail {
    fn from(end: End) -> Self {
        Self::End(end)
    }
}

/// WASI's error for a Linux error number of the world's.
fn wasi_errno(linux: i64) -> u16 {
    LINUX_TO_WASI
        .iter()
        .find(|(number, _)| *number == linux)
        .map_or(errno::IO, |(_, wasi)| *wasi)
}

/// A world's result: a count or a descriptor, or, below zero, minus a Linux error number.
fn outcome(result: u64) -> Result<u64, Fail> {
    let signed = result.cast_signed();
    if signed < 0 {
        return Err(Fail::Errno(
            signed.checked_neg().map_or(errno::IO, wasi_errno),
        ));
    }
    Ok(result)
}

/// `number` as `function` may give it: one outside its list is `IO`.
fn closed(function: &Function, number: u16) -> u16 {
    if function.errors.contains(&number) {
        number
    } else {
        errno::IO
    }
}

/// An argument's 32 bits read as the `i32` the host takes.
fn as_i32(value: u64) -> i32 {
    u32::try_from(value).map_or(-1, u32::cast_signed)
}

/// A clock reading's nanoseconds as the host counts them: seconds below zero as none, and the
/// total held at the most 64 bits hold.
fn nanoseconds_since(pool: &mut Pool, seconds: Term, nanoseconds: Term) -> Term {
    let zero = pool.bv(64, 0);
    let before = pool.slt(seconds, zero);
    let seconds = pool.ite(before, zero, seconds);
    let wide = pool.zero_extend(seconds, 64);
    let billion = pool.bv(128, 1_000_000_000);
    let product = pool.mul(wide, billion);
    let fraction = pool.zero_extend(nanoseconds, 64);
    let total = pool.add(product, fraction);
    let most = pool.bv(128, u128::from(u64::MAX));
    let over = pool.ult(most, total);
    let low = pool.extract(total, 63, 0);
    let saturated = pool.bv(64, u128::from(u64::MAX));
    pool.ite(over, saturated, low)
}

/// One event `poll_oneoff` writes.
struct Event {
    /// The subscription's own word, as it gave it.
    userdata: Term,
    /// The error, 0 for none.
    error: u16,
    /// The subscription's kind.
    kind: u8,
    /// How many bytes there are to read.
    bytes: u64,
    /// `HANGUP` where the other end has ended.
    flags: u16,
}

/// What a subscription of `poll_oneoff` waits for.
#[derive(Debug, Clone, Copy)]
enum Wanted {
    /// A time on a clock: a wait of `timeout` nanoseconds, or that reading when `absolute`.
    Clock {
        /// The clock.
        id: u64,
        /// The time.
        timeout: u64,
        /// Whether `timeout` is a reading rather than a wait.
        absolute: bool,
    },
    /// Something to read on a descriptor.
    Read(u32),
    /// Room to write on one.
    Write(u32),
}

/// Calls the WASI function `function`, which the operator `at` imports, with its arguments off
/// the stack and its error number on it, as lengua's host answers it.
///
/// # Errors
///
/// How the call ends the run: an exit, a trap, a stop, or what the model doesn't follow.
pub(super) fn call(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    function: &'static Function,
    at: usize,
) -> Result<Step<Wasm>, End> {
    // A folder lists in the order of a test's first run or of its second: a proof takes both, the
    // call made again in a run of each.
    if function.name == "fd_readdir" && state.core.reversed.is_none() {
        let mut choices = Vec::new();
        for reversed in [false, true] {
            let mut choice = state.clone();
            choice.core.reversed = Some(reversed);
            if let Some(frame) = choice.core.frames.last_mut() {
                frame.pc = at;
            }
            choices.push((pool.boolean(true), choice));
        }
        return Ok(Step::Choose(choices));
    }
    let start = state
        .core
        .stack
        .len()
        .checked_sub(function.params.len())
        .ok_or_else(|| unsupported("a host call with too few values on the stack"))?;
    let arguments: Vec<Term> = state.core.stack.drain(start..).collect();
    if !function.given {
        let missing = pool.bv(32, u128::from(errno::NOSYS));
        state.core.stack.push(missing);
        return Ok(Step::Next);
    }
    match function.name {
        "proc_exit" => {
            let code = arguments
                .first()
                .copied()
                .ok_or_else(|| unsupported("proc_exit with no code"))?;
            return Err(End::Exit(pool.extract(code, 7, 0)));
        }
        "sched_yield" => {
            let success = pool.bv(32, 0);
            state.core.stack.push(success);
            return Ok(Step::Next);
        }
        _ => {}
    }
    if !module.exports_memory {
        return Err(state.core.trap(
            module,
            at,
            "the module calls the host but exports no memory named \"memory\", which every host \
             call reads and writes through",
        ));
    }
    if !module.policy.allows(function) && function.unasked == Unasked::Refused {
        let refused = pool.bv(32, u128::from(errno::NOTCAPABLE));
        state.core.stack.push(refused);
        return Ok(Step::Next);
    }
    let State { core, world } = state;
    let mut host = Call {
        module,
        core,
        world,
        pool,
        at,
    };
    let number = match host.answer(function, &arguments) {
        Ok(()) => errno::SUCCESS,
        Err(Fail::Errno(number)) => closed(function, number),
        Err(Fail::End(end)) => return Err(end),
    };
    let result = host.pool.bv(32, u128::from(number));
    host.core.stack.push(result);
    Ok(Step::Next)
}

/// One host call's view of a run: the module, the run's machine and world, its pool, and the
/// operator that calls, for a trap's line.
struct Call<'a> {
    /// The module.
    module: &'a Module,
    /// The run's machine.
    core: &'a mut Wasm,
    /// The world it runs in.
    world: &'a mut World,
    /// The pool its terms are in.
    pool: &'a mut Pool,
    /// The operator that calls.
    at: usize,
}

/// The world's models act on the run through its linear memory, every access held inside it.
impl Host for Call<'_> {
    fn world(&mut self) -> &mut World {
        &mut *self.world
    }

    fn pool(&mut self) -> &mut Pool {
        &mut *self.pool
    }

    fn save(&mut self, address: u64, value: Term) -> Result<(), Stop> {
        let bytes = u64::from(self.pool.width(value) / 8);
        if !memory::inside(self.core, address, bytes) {
            return Err(self.outside());
        }
        memory::write(self.core, self.pool, address, value);
        Ok(())
    }

    fn load(&mut self, address: u64, bytes: u32) -> Result<Term, Stop> {
        if !memory::inside(self.core, address, u64::from(bytes)) {
            return Err(self.outside());
        }
        Ok(memory::read(
            self.module,
            self.core,
            self.pool,
            address,
            bytes,
        ))
    }
}

#[allow(clippy::too_many_lines)]
impl Call<'_> {
    /// The trap of a buffer outside the memory.
    fn outside(&self) -> End {
        self.core.trap(self.module, self.at, OUTSIDE)
    }

    /// Whether the `bytes` bytes at `address` are inside the memory, as the host checks each
    /// buffer a call borrows: else the trap.
    fn check(&self, address: u64, bytes: u64) -> Result<(), Fail> {
        if memory::inside(self.core, address, bytes) {
            Ok(())
        } else {
            Err(Fail::End(self.outside()))
        }
    }

    /// What `term`, an argument or a number a call reads from memory, is: the model follows host
    /// calls on numbers it knows.
    fn number(&self, term: Term) -> Result<u64, Fail> {
        self.pool
            .constant(term)
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| {
                Fail::End(unsupported(self.core.placed(
                    self.module,
                    self.at,
                    "a host call on a number the input chooses, which the model doesn't follow",
                )))
            })
    }

    /// The number of `bytes` bytes a call reads at `address`.
    fn read_number(&mut self, address: u64, bytes: u32) -> Result<u64, Fail> {
        self.check(address, u64::from(bytes))?;
        let value = memory::read(self.module, self.core, self.pool, address, bytes);
        self.number(value)
    }

    /// Writes `value`, `width` bits, at `address`.
    fn put(&mut self, address: u64, width: u32, value: u128) -> Result<(), Fail> {
        self.check(address, u64::from(width / 8))?;
        let constant = self.pool.bv(width, value);
        memory::write(self.core, self.pool, address, constant);
        Ok(())
    }

    /// Writes `value`, a term, at `address`.
    fn put_term(&mut self, address: u64, value: Term) -> Result<(), Fail> {
        self.save(address, value).map_err(Fail::End)
    }

    /// Zeros the `length` bytes at `address`.
    fn zero(&mut self, address: u64, length: u64) -> Result<(), Fail> {
        self.check(address, length)?;
        for offset in 0..length {
            self.put(address + offset, 8, 0)?;
        }
        Ok(())
    }

    /// The `length` bytes at `address`, a name the program gives.
    fn text(&mut self, address: u64, length: u64) -> Result<Vec<u8>, Fail> {
        self.check(address, length)?;
        let mut text = Vec::new();
        for offset in 0..length {
            let byte = memory::read(self.module, self.core, self.pool, address + offset, 1);
            let value = self
                .pool
                .constant(byte)
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| {
                    Fail::End(unsupported(self.core.placed(
                        self.module,
                        self.at,
                        "a name the input chooses, which the WebAssembly model doesn't follow yet",
                    )))
                })?;
            text.push(value);
        }
        Ok(text)
    }

    /// The buffers of the `count` iovecs at `at`, each checked, as the host reads them.
    fn iovecs(&mut self, at: u64, count: u64) -> Result<Vec<(u64, u64)>, Fail> {
        if count > u64::from(crate::wasm::host::memory::IOVECS) {
            return Err(Fail::Errno(errno::INVAL));
        }
        let entry = u64::from(size::IOVEC);
        self.check(at, count * entry)?;
        let mut buffers = Vec::new();
        for index in 0..count {
            let place = at + index * entry;
            let buffer = self.read_number(place, 4)?;
            let length = self.read_number(place + 4, 4)?;
            self.check(buffer, length)?;
            buffers.push((buffer, length));
        }
        Ok(buffers)
    }

    /// Answers a call of `function` the idea allows, with `arguments`.
    fn answer(&mut self, function: &Function, arguments: &[Term]) -> Result<(), Fail> {
        let mut numbers = Vec::with_capacity(arguments.len());
        // Every argument is a number the model must know, but the clock's precision, which the
        // host doesn't read.
        for (index, argument) in arguments.iter().enumerate() {
            if function.name == "clock_time_get" && index == 1 {
                numbers.push(0);
            } else {
                numbers.push(self.number(*argument)?);
            }
        }
        let fd = |index: usize| u32::try_from(numbers[index]).unwrap_or(u32::MAX);
        match function.name {
            "args_get" | "environ_get" => {
                let strings = if function.name == "args_get" {
                    Rc::clone(&self.core.args)
                } else {
                    Rc::clone(&self.core.env)
                };
                self.strings_get(&strings, numbers[0], numbers[1])
            }
            "args_sizes_get" | "environ_sizes_get" => {
                let strings = if function.name == "args_sizes_get" {
                    Rc::clone(&self.core.args)
                } else {
                    Rc::clone(&self.core.env)
                };
                self.strings_sizes(&strings, numbers[0], numbers[1])
            }
            "clock_res_get" => self.clock_res_get(numbers[0], numbers[1]),
            "clock_time_get" => self.clock_time_get(numbers[0], numbers[2]),
            "fd_close" => self.fd_close(fd(0)),
            "fd_fdstat_get" => self.fd_fdstat_get(fd(0), numbers[1]),
            "fd_filestat_get" => self.fd_filestat_get(fd(0), numbers[1]),
            "fd_prestat_get" => self.fd_prestat_get(fd(0), numbers[1]),
            "fd_prestat_dir_name" => self.fd_prestat_dir_name(fd(0), numbers[1], numbers[2]),
            "fd_read" => self.fd_read(fd(0), (numbers[1], numbers[2]), numbers[3]),
            "fd_readdir" => {
                self.fd_readdir(fd(0), (numbers[1], numbers[2]), numbers[3], numbers[4])
            }
            "fd_seek" => {
                let whence = u8::try_from(as_i32(numbers[2])).unwrap_or(u8::MAX);
                self.fd_seek(fd(0), numbers[1].cast_signed(), whence, numbers[3])
            }
            "fd_tell" => self.fd_seek(fd(0), 0, whence::CUR, numbers[1]),
            "fd_write" => self.fd_write(fd(0), (numbers[1], numbers[2]), numbers[3]),
            "path_create_directory" => self.path_create_directory(fd(0), (numbers[1], numbers[2])),
            "path_filestat_get" => {
                self.path_filestat_get(fd(0), (numbers[2], numbers[3]), numbers[4])
            }
            "path_open" => {
                let opening = (
                    u16::try_from(numbers[4] & 0xffff).unwrap_or(0),
                    numbers[5],
                    u16::try_from(numbers[7] & 0xffff).unwrap_or(0),
                );
                self.path_open(fd(0), (numbers[2], numbers[3]), opening, numbers[8])
            }
            "path_remove_directory" => self.path_remove_directory(fd(0), (numbers[1], numbers[2])),
            "path_rename" => self.path_rename(
                (fd(0), numbers[1], numbers[2]),
                (fd(3), numbers[4], numbers[5]),
            ),
            "path_unlink_file" => self.path_unlink_file(fd(0), (numbers[1], numbers[2])),
            "poll_oneoff" => self.poll_oneoff(numbers[0], numbers[1], numbers[2], numbers[3]),
            "random_get" => self.random_get(numbers[0], numbers[1]),
            "sock_accept" => {
                let flags = u16::try_from(numbers[1] & 0xffff).unwrap_or(0);
                self.sock_accept(fd(0), flags, numbers[2])
            }
            "sock_recv" => {
                let flags = u16::try_from(numbers[3] & 0xffff).unwrap_or(u16::MAX);
                self.sock_recv(
                    fd(0),
                    (numbers[1], numbers[2]),
                    flags,
                    (numbers[4], numbers[5]),
                )
            }
            "sock_send" => self.sock_send(fd(0), (numbers[1], numbers[2]), numbers[4]),
            "sock_shutdown" => {
                let how = u8::try_from(as_i32(numbers[1])).unwrap_or(u8::MAX);
                self.sock_shutdown(fd(0), how)
            }
            other => Err(Fail::End(unsupported(format!(
                "WASI's {other}, which the model doesn't follow"
            )))),
        }
    }

    /// `args_sizes_get` and `environ_sizes_get`: how many `strings`, and their bytes with a NUL
    /// each.
    fn strings_sizes(&mut self, strings: &[Vec<Term>], count: u64, bytes: u64) -> Result<(), Fail> {
        self.check(count, 4)?;
        self.check(bytes, 4)?;
        let total: usize = strings.iter().map(|string| string.len() + 1).sum();
        self.put(count, 32, strings.len() as u128)?;
        self.put(bytes, 32, total as u128)
    }

    /// `args_get` and `environ_get`: each of `strings` with a NUL at `buffer`, one after
    /// another, and where each starts at `pointers`.
    fn strings_get(
        &mut self,
        strings: &[Vec<Term>],
        pointers: u64,
        buffer: u64,
    ) -> Result<(), Fail> {
        let total: usize = strings.iter().map(|string| string.len() + 1).sum();
        self.check(pointers, strings.len() as u64 * 4)?;
        self.check(buffer, total as u64)?;
        let mut at = buffer;
        for (index, string) in (0_u64..).zip(strings) {
            self.put(pointers + index * 4, 32, u128::from(at))?;
            for byte in string {
                self.put_term(at, *byte)?;
                at += 1;
            }
            self.put(at, 8, 0)?;
            at += 1;
        }
        Ok(())
    }

    /// `clock_res_get(id, out)`: a microsecond, for both clocks.
    fn clock_res_get(&mut self, id: u64, out: u64) -> Result<(), Fail> {
        if id != u64::from(clock::REALTIME) && id != u64::from(clock::MONOTONIC) {
            return Err(Fail::Errno(errno::INVAL));
        }
        self.put(out, 64, u128::from(crate::wasm::host::clock::RESOLUTION))
    }

    /// The clock's next reading, in nanoseconds, rounded down to a microsecond: a test's, the
    /// last repeating, or, where the test supplies none, any time the host's clock could read.
    fn reading(&mut self) -> Term {
        let readings = Rc::clone(&self.world.clock);
        let total = match readings.len().checked_sub(1) {
            Some(last) => {
                let (seconds, nanoseconds) = readings[self.world.readings_used.min(last)];
                self.world.readings_used += 1;
                nanoseconds_since(self.pool, seconds, nanoseconds)
            }
            None => {
                let fresh = self.pool.len();
                self.pool.var(64, &format!("clock{fresh}.nanoseconds"))
            }
        };
        let resolution = self
            .pool
            .bv(64, u128::from(crate::wasm::host::clock::RESOLUTION));
        let finer = self.pool.urem(total, resolution);
        self.pool.sub(total, finer)
    }

    /// `clock_time_get(id, precision, out)`.
    fn clock_time_get(&mut self, id: u64, out: u64) -> Result<(), Fail> {
        self.check(out, 8)?;
        if id != u64::from(clock::REALTIME) && id != u64::from(clock::MONOTONIC) {
            return Err(Fail::Errno(errno::INVAL));
        }
        let reading = self.reading();
        self.put_term(out, reading)
    }

    /// `random_get(buffer, length)`: a test's bytes in order, zeros past them, or any bytes the
    /// system could give; zeros for an idea that doesn't name random bytes.
    fn random_get(&mut self, buffer: u64, length: u64) -> Result<(), Fail> {
        self.check(buffer, length)?;
        if !self.module.policy.random {
            return self.zero(buffer, length);
        }
        World::random(self, buffer, length)?;
        Ok(())
    }

    /// `fd_close(fd)`: a file or folder needs files named, a connection or the listener the
    /// network.
    fn fd_close(&mut self, fd: u32) -> Result<(), Fail> {
        let policy = self.module.policy;
        let allowed = match self.core.fds.get(&fd) {
            None => return Err(Fail::Errno(errno::BADF)),
            Some(Handle::Listener | Handle::Socket { .. }) => policy.network(),
            Some(_) => policy.files || policy.network(),
        };
        if !allowed {
            return Err(Fail::Errno(errno::NOTCAPABLE));
        }
        if let Some(Handle::File { world: opened, .. } | Handle::Socket { world: opened, .. }) =
            self.core.fds.remove(&fd)
        {
            self.world.close(opened);
        }
        Ok(())
    }

    /// `fd_fdstat_get(fd, out)`: each descriptor's kind and flags, and the rights it reports.
    fn fd_fdstat_get(&mut self, fd: u32, out: u64) -> Result<(), Fail> {
        self.check(out, u64::from(size::FDSTAT))?;
        let poll = rights::POLL_FD_READWRITE;
        let (kind, flags, base, inheriting) = match self.core.fds.get(&fd) {
            None => return Err(Fail::Errno(errno::BADF)),
            // A test's standard streams are never a terminal.
            Some(Handle::Stdin) => (filetype::UNKNOWN, 0, rights::FD_READ | poll, 0),
            Some(Handle::Stdout | Handle::Stderr) => {
                (filetype::UNKNOWN, 0, rights::FD_WRITE | poll, 0)
            }
            Some(Handle::Folder { .. }) => (filetype::DIRECTORY, 0, rights::ALL, rights::ALL),
            Some(Handle::File {
                read,
                write,
                append,
                ..
            }) => {
                let mut base = rights::FD_SEEK | rights::FD_TELL | rights::FD_FILESTAT_GET | poll;
                if *read {
                    base |= rights::FD_READ;
                }
                if *write {
                    base |= rights::FD_WRITE;
                }
                let flags = if *append { fdflags::APPEND } else { 0 };
                (filetype::REGULAR_FILE, flags, base, 0)
            }
            Some(Handle::Listener) => (
                filetype::SOCKET_STREAM,
                0,
                rights::SOCK_ACCEPT | rights::FD_READ | poll,
                0,
            ),
            Some(Handle::Socket { .. }) => (
                filetype::SOCKET_STREAM,
                0,
                rights::FD_READ | rights::FD_WRITE | rights::SOCK_SHUTDOWN | poll,
                0,
            ),
        };
        self.zero(out, u64::from(size::FDSTAT))?;
        self.put(out, 8, u128::from(kind))?;
        self.put(out + 2, 16, u128::from(flags))?;
        self.put(out + 8, 64, u128::from(base))?;
        self.put(out + 16, 64, u128::from(inheriting))
    }

    /// A `filestat` of `kind` and `bytes` at `out`: everything else zero.
    fn filestat(&mut self, out: u64, kind: u8, bytes: usize) -> Result<(), Fail> {
        self.zero(out, u64::from(size::FILESTAT))?;
        self.put(out + 16, 8, u128::from(kind))?;
        self.put(out + 32, 64, bytes as u128)
    }

    /// How many bytes the file `name` holds now.
    fn length_of(&self, name: &str) -> usize {
        self.world.file(name).map_or(0, <[Term]>::len)
    }

    /// `fd_filestat_get(fd, out)`: a descriptor's kind and size, everything else zero.
    fn fd_filestat_get(&mut self, fd: u32, out: u64) -> Result<(), Fail> {
        self.check(out, u64::from(size::FILESTAT))?;
        let (kind, bytes) = match self.core.fds.get(&fd) {
            None => return Err(Fail::Errno(errno::BADF)),
            Some(Handle::File { name, .. }) => (filetype::REGULAR_FILE, self.length_of(name)),
            Some(Handle::Folder { .. }) => (filetype::DIRECTORY, 0),
            Some(Handle::Stdin | Handle::Stdout | Handle::Stderr) => (filetype::UNKNOWN, 0),
            Some(Handle::Listener | Handle::Socket { .. }) => (filetype::SOCKET_STREAM, 0),
        };
        self.filestat(out, kind, bytes)
    }

    /// Whether `fd` is the working folder, descriptor 3, still open, for an idea that names
    /// files.
    fn preopened(&self, fd: u32) -> bool {
        let working = matches!(
            self.core.fds.get(&fd),
            Some(Handle::Folder { path, .. }) if path.is_empty()
        );
        fd == FOLDER && self.module.policy.files && working
    }

    /// `fd_prestat_get(fd, out)`: the working folder is a preopen named `.`; any other
    /// descriptor is `BADF`.
    fn fd_prestat_get(&mut self, fd: u32, out: u64) -> Result<(), Fail> {
        self.check(out, u64::from(size::PRESTAT))?;
        if !self.preopened(fd) {
            return Err(Fail::Errno(errno::BADF));
        }
        self.zero(out, u64::from(size::PRESTAT))?;
        self.put(out + 4, 32, 1)
    }

    /// `fd_prestat_dir_name(fd, path, length)`: `.`.
    fn fd_prestat_dir_name(&mut self, fd: u32, path: u64, length: u64) -> Result<(), Fail> {
        self.check(path, length)?;
        if !self.preopened(fd) {
            return Err(Fail::Errno(errno::BADF));
        }
        if length == 0 {
            return Err(Fail::Errno(errno::NAMETOOLONG));
        }
        self.put(path, 8, u128::from(b'.'))
    }

    /// A read through the world of at most `length` bytes of its descriptor `opened` into
    /// `buffer`: what it took, or the error.
    fn read_world(&mut self, opened: u64, buffer: u64, length: u64) -> Result<u64, Fail> {
        match World::read(self, opened, buffer, length)? {
            Reading::Done(result) => outcome(result),
            Reading::EveryCount => Err(Fail::End(unsupported(EVERY_CUT))),
        }
    }

    /// A read of at most `length` bytes, into `buffer`, of the connection the world keeps as
    /// `opened`, as lengua's host reads it: the host takes what arrives in at most [`CHUNK`]
    /// bytes at a time and gives a read only what it took, so a read of a piece of what the
    /// peer sends ends at the next multiple of [`CHUNK`] in it.
    fn read_socket(&mut self, opened: u64, buffer: u64, length: u64) -> Result<u64, Fail> {
        let into = match self.world.connection_of(opened) {
            Ok(Some(number)) => self.world.connections[number].taken,
            _ => 0,
        };
        let chunk = CHUNK as u64;
        let room = chunk - into as u64 % chunk;
        self.read_world(opened, buffer, length.min(room))
    }

    /// `fd_read(fd, iovs, count, out)`: standard input needs it named, or the terminal; a file,
    /// files; a connection, the network. Standard input and a connection fill the first buffer
    /// with room, as one read of a pipe or a socket does, standard input's at most what one pipe
    /// write holds and a connection's at most what lengua's host took in at once; a file fills
    /// them in order.
    fn fd_read(&mut self, fd: u32, (iovs, count): (u64, u64), out: u64) -> Result<(), Fail> {
        self.check(out, 4)?;
        let buffers = self.iovecs(iovs, count)?;
        let policy = self.module.policy;
        let first = buffers.iter().copied().find(|(_, length)| *length > 0);
        let total = match self.core.fds.get(&fd).cloned() {
            None => return Err(Fail::Errno(errno::BADF)),
            Some(Handle::Stdin) => {
                if !policy.reads_input() {
                    return Err(Fail::Errno(errno::NOTCAPABLE));
                }
                match first {
                    Some((buffer, length)) => {
                        let most = u64::try_from(crate::runner::PIPE_WRITE).unwrap_or(u64::MAX);
                        self.read_world(0, buffer, length.min(most))?
                    }
                    None => 0,
                }
            }
            Some(Handle::File {
                world: opened,
                read,
                write,
                ..
            }) => {
                if !policy.files {
                    return Err(Fail::Errno(errno::NOTCAPABLE));
                }
                if read && write {
                    return Err(Fail::End(unsupported(
                        "reading a file the program may also write, which the model doesn't \
                         follow",
                    )));
                }
                let mut total = 0;
                for (buffer, length) in buffers {
                    let got = match self.read_world(opened, buffer, length) {
                        Ok(got) => got,
                        Err(Fail::Errno(_)) if total > 0 => break,
                        Err(failure) => return Err(failure),
                    };
                    total += got;
                    if got < length {
                        break;
                    }
                }
                total
            }
            Some(Handle::Folder { .. }) => {
                return Err(Fail::Errno(if policy.files {
                    errno::ISDIR
                } else {
                    errno::NOTCAPABLE
                }));
            }
            Some(Handle::Socket {
                world: opened,
                read_shut,
                ..
            }) => {
                if !policy.network() {
                    return Err(Fail::Errno(errno::NOTCAPABLE));
                }
                match first {
                    Some((buffer, length)) if !read_shut => {
                        self.read_socket(opened, buffer, length)?
                    }
                    _ => 0,
                }
            }
            Some(Handle::Stdout | Handle::Stderr | Handle::Listener) => {
                return Err(Fail::Errno(errno::BADF));
            }
        };
        self.put(out, 32, u128::from(total))
    }

    /// A write of the `length` bytes at `buffer` to standard output (`stream` 1) or error (2).
    fn write_stream(&mut self, stream: u64, buffer: u64, length: u64) -> Result<u64, Fail> {
        let held = if stream == 1 {
            self.world.stdout.len()
        } else {
            self.world.stderr.len()
        };
        let most = crate::runner::Limits::default().max_output;
        if held.saturating_add(usize::try_from(length).unwrap_or(usize::MAX)) > most {
            return Err(Fail::End(unsupported(
                "output past a test's limit for a stream, which the model doesn't follow",
            )));
        }
        outcome(World::write(self, stream, buffer, length)?)
    }

    /// A write of the `length` bytes at `buffer` to the file `fd`, as far as a test's limits let
    /// it: at its end, the only place the model writes a file.
    fn write_file(&mut self, fd: u32, buffer: u64, length: u64) -> Result<u64, Fail> {
        let Some(Handle::File {
            world: opened,
            name,
            write,
            append,
            position,
            ..
        }) = self.core.fds.get(&fd).cloned()
        else {
            return Err(Fail::Errno(errno::BADF));
        };
        if !write {
            return Err(Fail::Errno(errno::BADF));
        }
        let end = self.length_of(&name) as u64;
        let at = if append { end } else { position };
        if at != end {
            return Err(Fail::End(unsupported(
                "a write in a file anywhere but at its end, which the model doesn't follow",
            )));
        }
        let room = length
            .min(TEST_FILE_LIMIT.saturating_sub(at))
            .min(TEST_WRITTEN.saturating_sub(self.core.written));
        if room == 0 && length > 0 {
            return Err(Fail::Errno(errno::FBIG));
        }
        let wrote = outcome(World::write(self, opened, buffer, room)?)?;
        self.core.written = self.core.written.saturating_add(wrote);
        if let Some(Handle::File { position, .. }) = self.core.fds.get_mut(&fd) {
            *position = at + wrote;
        }
        Ok(wrote)
    }

    /// The connection the world keeps for its descriptor `opened`.
    fn connection(&self, opened: u64) -> Result<usize, Fail> {
        match self.world.connection_of(opened) {
            Ok(Some(number)) => Ok(number),
            _ => Err(Fail::Errno(errno::IO)),
        }
    }

    /// A send of the `length` bytes at `buffer` on the connection `fd`: every one of them, as
    /// lengua's host waits until the connection has taken them all, as the world's send does.
    fn send(&mut self, fd: u32, buffer: u64, length: u64) -> Result<u64, Fail> {
        let Some(Handle::Socket {
            world: opened,
            write_shut,
            ..
        }) = self.core.fds.get(&fd).cloned()
        else {
            return Err(Fail::Errno(errno::BADF));
        };
        if write_shut {
            return Err(Fail::Errno(errno::PIPE));
        }
        if length == 0 {
            return Ok(0);
        }
        let number = self.connection(opened)?;
        outcome(World::send(self, number, buffer, length, 0)?)
    }

    /// `fd_write(fd, iovs, count, out)`: standard output and error are every program's; a file
    /// needs files named, a connection the network.
    fn fd_write(&mut self, fd: u32, (iovs, count): (u64, u64), out: u64) -> Result<(), Fail> {
        self.check(out, 4)?;
        let buffers = self.iovecs(iovs, count)?;
        let policy = self.module.policy;
        let mut total = 0;
        match self.core.fds.get(&fd).cloned() {
            None => return Err(Fail::Errno(errno::BADF)),
            Some(Handle::Stdout) => {
                for (buffer, length) in buffers {
                    total += self.write_stream(1, buffer, length)?;
                }
            }
            Some(Handle::Stderr) => {
                for (buffer, length) in buffers {
                    total += self.write_stream(2, buffer, length)?;
                }
            }
            Some(Handle::File { .. }) => {
                if !policy.files {
                    return Err(Fail::Errno(errno::NOTCAPABLE));
                }
                for (buffer, length) in buffers {
                    let wrote = match self.write_file(fd, buffer, length) {
                        Ok(wrote) => wrote,
                        Err(Fail::Errno(_)) if total > 0 => break,
                        Err(failure) => return Err(failure),
                    };
                    total += wrote;
                    if wrote < length {
                        break;
                    }
                }
            }
            Some(Handle::Socket { .. }) => {
                if !policy.network() {
                    return Err(Fail::Errno(errno::NOTCAPABLE));
                }
                for (buffer, length) in buffers {
                    let sent = match self.send(fd, buffer, length) {
                        Ok(sent) => sent,
                        Err(Fail::Errno(_)) if total > 0 => break,
                        Err(failure) => return Err(failure),
                    };
                    total += sent;
                    if sent < length {
                        break;
                    }
                }
            }
            Some(Handle::Stdin | Handle::Folder { .. } | Handle::Listener) => {
                return Err(Fail::Errno(errno::BADF));
            }
        }
        self.put(out, 32, u128::from(total))
    }

    /// `fd_seek(fd, offset, whence, out)` and `fd_tell(fd, out)`: only a file has places.
    fn fd_seek(&mut self, fd: u32, offset: i64, from: u8, out: u64) -> Result<(), Fail> {
        self.check(out, 8)?;
        let place = match self.core.fds.get(&fd).cloned() {
            None | Some(Handle::Folder { .. }) => return Err(Fail::Errno(errno::BADF)),
            Some(Handle::File {
                world: opened,
                name,
                write,
                position,
                ..
            }) => {
                let length = self.length_of(&name) as i128;
                // A file open to write is where its last write left it; one open to read, where
                // the world's reads have got to.
                let now = if write {
                    i128::from(position)
                } else {
                    self.world
                        .open
                        .get(&opened)
                        .map_or(0, |open| open.position as i128)
                };
                let place = match from {
                    whence::SET if offset < 0 => return Err(Fail::Errno(errno::INVAL)),
                    whence::SET => i128::from(offset),
                    whence::CUR => now + i128::from(offset),
                    whence::END => length + i128::from(offset),
                    _ => return Err(Fail::Errno(errno::INVAL)),
                };
                let place = u64::try_from(place).map_err(|_| Fail::Errno(errno::INVAL))?;
                if write {
                    if let Some(Handle::File { position, .. }) = self.core.fds.get_mut(&fd) {
                        *position = place;
                    }
                } else if let Some(open) = self.world.open.get_mut(&opened) {
                    open.position = usize::try_from(place).unwrap_or(usize::MAX);
                }
                place
            }
            Some(_) => return Err(Fail::Errno(errno::SPIPE)),
        };
        self.put(out, 64, u128::from(place))
    }

    /// Whether the idea names files, which every call on the working folder needs.
    fn files(&self) -> Result<(), Fail> {
        if self.module.policy.files {
            Ok(())
        } else {
            Err(Fail::Errno(errno::NOTCAPABLE))
        }
    }

    /// The names the `length` bytes at `at`, a path, lead to from the folder `fd`, as the host
    /// resolves them: `NOTCAPABLE` for one that leaves the working folder.
    fn resolve(&mut self, fd: u32, at: u64, length: u64) -> Result<Vec<String>, Fail> {
        let text = self.text(at, length)?;
        let base = match self.core.fds.get(&fd) {
            Some(Handle::Folder { path, .. }) => path.clone(),
            Some(_) => return Err(Fail::Errno(errno::NOTDIR)),
            None => return Err(Fail::Errno(errno::BADF)),
        };
        names_in(&base, &text).map_err(Fail::Errno)
    }

    /// What the name `name` is now.
    fn kind_of(&self, name: &str) -> Option<Found> {
        if self.world.is_folder(name) {
            Some(Found::Folder)
        } else if self.world.file(name).is_some() {
            Some(Found::File)
        } else {
            None
        }
    }

    /// What `names` is, or `None` when its folder has no such entry: every name before the last
    /// must be a folder.
    fn found(&self, names: &[String]) -> Result<Option<Found>, Fail> {
        for (depth, _) in names.iter().enumerate() {
            let kind = self.kind_of(&names[..=depth].join("/"));
            if depth + 1 == names.len() {
                return Ok(kind);
            }
            match kind {
                Some(Found::Folder) => {}
                Some(Found::File) => return Err(Fail::Errno(errno::NOTDIR)),
                None => return Err(Fail::Errno(errno::NOENT)),
            }
        }
        // No names: the working folder itself.
        Ok(Some(Found::Folder))
    }

    /// The entries of the folder `folder`, sorted by name, without `.` and `..`.
    fn listing(&self, folder: &[String]) -> Vec<Entry> {
        let prefix = if folder.is_empty() {
            String::new()
        } else {
            format!("{}/", folder.join("/"))
        };
        let world = &*self.world;
        let mut entries: BTreeMap<String, Found> = BTreeMap::new();
        let names = world
            .files
            .keys()
            .chain(world.written.keys())
            .chain(world.folders_before.iter())
            .chain(world.made.iter());
        for name in names {
            let Some(child) = name
                .strip_prefix(prefix.as_str())
                .and_then(|rest| rest.split('/').next())
                .filter(|child| !child.is_empty())
            else {
                continue;
            };
            if let Some(kind) = self.kind_of(&format!("{prefix}{child}")) {
                entries.insert(child.to_owned(), kind);
            }
        }
        entries.into_iter().collect()
    }

    /// Whether a new name, the last of `names`, differs only in case or normalization from an
    /// entry of its folder.
    fn clashes(&self, names: &[String]) -> bool {
        let Some((leaf, folder)) = names.split_last() else {
            return false;
        };
        self.listing(folder)
            .iter()
            .any(|(name, _)| crate::names::clashes(leaf, name))
    }

    /// Whether the program holds a file or folder open at `names` or beneath it: the working
    /// folder itself, descriptor 3, is never one.
    fn busy(&self, names: &[String]) -> bool {
        self.core.fds.iter().any(|(fd, handle)| match handle {
            Handle::File { path, .. } => path.starts_with(names),
            Handle::Folder { path, .. } => *fd != FOLDER && path.starts_with(names),
            _ => false,
        })
    }

    /// Counts a file or folder the program is about to make against a test's total, as an
    /// entry: `FBIG` when the total has no room for one more.
    fn make(&mut self) -> Result<(), Fail> {
        if TEST_WRITTEN.saturating_sub(self.core.written) < ENTRY {
            return Err(Fail::Errno(errno::FBIG));
        }
        self.core.written = self.core.written.saturating_add(ENTRY);
        Ok(())
    }

    /// Adds `handle` at the lowest free number from the first a program's may take.
    fn add(&mut self, handle: Handle) -> Result<u32, Fail> {
        if self.core.fds.len() as u64 >= OPEN_FILES {
            if let Handle::File { world: opened, .. } | Handle::Socket { world: opened, .. } =
                handle
            {
                self.world.close(opened);
            }
            return Err(Fail::Errno(errno::MFILE));
        }
        let mut fd = self.core.first;
        while self.core.fds.contains_key(&fd) {
            fd = fd.checked_add(1).ok_or(Fail::Errno(errno::MFILE))?;
        }
        self.core.fds.insert(fd, handle);
        Ok(fd)
    }

    /// The file `name` opened in the world, `exists` already: to write, made or emptied, or
    /// kept as it is for appending; else to read.
    fn open_world(
        &mut self,
        name: &str,
        exists: bool,
        write: bool,
        (empty, append): (bool, bool),
    ) -> Result<u64, Fail> {
        if !write {
            return outcome(self.world.open(name.to_owned(), false, false)?);
        }
        if exists && !empty && !append {
            return Err(Fail::End(unsupported(
                "writing into a file without emptying it or appending to it, which the model \
                 doesn't follow",
            )));
        }
        let kept = if append {
            self.world.file(name).map(<[Term]>::to_vec)
        } else {
            None
        };
        let opened = outcome(self.world.open(name.to_owned(), true, true)?)?;
        if let Some(kept) = kept {
            self.world.written.insert(name.to_owned(), kept);
        }
        Ok(opened)
    }

    /// `path_open(fd, dirflags, path, length, oflags, rights, inheriting, fdflags, out)`.
    fn path_open(
        &mut self,
        fd: u32,
        (at, length): (u64, u64),
        (opening, wanted, flags): (u16, u64, u16),
        out: u64,
    ) -> Result<(), Fail> {
        self.check(out, 4)?;
        let names = self.resolve(fd, at, length)?;
        let create = opening & oflags::CREAT != 0;
        let folder = opening & oflags::DIRECTORY != 0;
        let new = opening & oflags::EXCL != 0;
        let empty = opening & oflags::TRUNC != 0;
        let append = flags & fdflags::APPEND != 0;
        let write = wanted & rights::FD_WRITE != 0 || append || empty;
        let read = wanted & rights::FD_READ != 0 || !write;
        self.files()?;
        let found = self.found(&names)?;
        let handle = match found {
            Some(Found::Folder) if create && new => return Err(Fail::Errno(errno::EXIST)),
            Some(Found::Folder) if write => return Err(Fail::Errno(errno::ISDIR)),
            Some(Found::Folder) => Handle::Folder {
                path: names,
                listing: None,
            },
            Some(Found::File) if folder => return Err(Fail::Errno(errno::NOTDIR)),
            Some(Found::File) if create && new => return Err(Fail::Errno(errno::EXIST)),
            None if folder && create => return Err(Fail::Errno(errno::INVAL)),
            None if !create => return Err(Fail::Errno(errno::NOENT)),
            Some(Found::File) | None => {
                // Every system makes these the same way only when they're asked for consistently.
                if (empty && append) || (create && !write) {
                    return Err(Fail::Errno(errno::INVAL));
                }
                if found.is_none() && self.clashes(&names) {
                    return Err(Fail::Errno(errno::EXIST));
                }
                // A new file counts against the total.
                if found.is_none() {
                    self.make()?;
                }
                let name = names.join("/");
                let opened = self.open_world(&name, found.is_some(), write, (empty, append))?;
                Handle::File {
                    world: opened,
                    name,
                    path: names,
                    read,
                    write,
                    append,
                    position: 0,
                }
            }
        };
        let opened = self.add(handle)?;
        self.put(out, 32, u128::from(opened))
    }

    /// `path_filestat_get(fd, flags, path, length, out)`: a name's kind and size.
    fn path_filestat_get(
        &mut self,
        fd: u32,
        (at, length): (u64, u64),
        out: u64,
    ) -> Result<(), Fail> {
        self.check(out, u64::from(size::FILESTAT))?;
        let names = self.resolve(fd, at, length)?;
        self.files()?;
        let (kind, bytes) = match self.found(&names)? {
            None => return Err(Fail::Errno(errno::NOENT)),
            Some(Found::File) => (filetype::REGULAR_FILE, self.length_of(&names.join("/"))),
            Some(Found::Folder) => (filetype::DIRECTORY, 0),
        };
        self.filestat(out, kind, bytes)
    }

    /// `fd_readdir(fd, buffer, length, cookie, used)`: the folder's entries from the `cookie`th
    /// on, each a `dirent` and its name, as many as fit, the last cut short when it doesn't; in
    /// the run's order, turned round in a test's second run.
    fn fd_readdir(
        &mut self,
        fd: u32,
        (buffer, length): (u64, u64),
        cookie: u64,
        used: u64,
    ) -> Result<(), Fail> {
        self.check(used, 4)?;
        self.check(buffer, length)?;
        self.files()?;
        let (path, listing) = match self.core.fds.get(&fd) {
            Some(Handle::Folder { path, listing }) => (path.clone(), listing.clone()),
            Some(_) => return Err(Fail::Errno(errno::NOTDIR)),
            None => return Err(Fail::Errno(errno::BADF)),
        };
        let entries = match listing.filter(|_| cookie != 0) {
            Some(entries) => entries,
            None => {
                let mut entries = self.listing(&path);
                if self.core.reversed == Some(true) {
                    entries.reverse();
                }
                let entries = Rc::new(entries);
                if let Some(Handle::Folder { listing, .. }) = self.core.fds.get_mut(&fd) {
                    *listing = Some(Rc::clone(&entries));
                }
                entries
            }
        };
        let start = usize::try_from(cookie).unwrap_or(usize::MAX);
        let room = usize::try_from(length).unwrap_or(usize::MAX);
        let mut bytes: Vec<u8> = Vec::new();
        for (index, (name, kind)) in entries.iter().enumerate().skip(start) {
            if bytes.len() >= room {
                break;
            }
            let next = u64::try_from(index + 1).unwrap_or(u64::MAX);
            bytes.extend_from_slice(&next.to_le_bytes());
            // The entry's inode number: its place, never 0, the same on every system.
            bytes.extend_from_slice(&next.to_le_bytes());
            bytes.extend_from_slice(&u32::try_from(name.len()).unwrap_or(u32::MAX).to_le_bytes());
            bytes.push(kind.filetype());
            bytes.extend_from_slice(&[0; 3]);
            bytes.extend_from_slice(name.as_bytes());
        }
        let count = bytes.len().min(room);
        for (offset, byte) in (0_u64..).zip(&bytes[..count]) {
            self.put(buffer + offset, 8, u128::from(*byte))?;
        }
        self.put(used, 32, count as u128)
    }

    /// `path_create_directory(fd, path, length)`.
    fn path_create_directory(&mut self, fd: u32, (at, length): (u64, u64)) -> Result<(), Fail> {
        let names = self.resolve(fd, at, length)?;
        self.files()?;
        if self.found(&names)?.is_some() || self.clashes(&names) {
            return Err(Fail::Errno(errno::EXIST));
        }
        // A new folder counts against the total.
        self.make()?;
        outcome(self.world.make_folder(names.join("/")))?;
        Ok(())
    }

    /// `path_remove_directory(fd, path, length)`: an empty folder the program doesn't hold open.
    fn path_remove_directory(&mut self, fd: u32, (at, length): (u64, u64)) -> Result<(), Fail> {
        let names = self.resolve(fd, at, length)?;
        if names.is_empty() {
            return Err(Fail::Errno(errno::INVAL));
        }
        let held = self.busy(&names);
        self.files()?;
        match self.found(&names)? {
            None => return Err(Fail::Errno(errno::NOENT)),
            Some(Found::Folder) => {}
            Some(Found::File) => return Err(Fail::Errno(errno::NOTDIR)),
        }
        if held {
            return Err(Fail::Errno(errno::BUSY));
        }
        if !self.listing(&names).is_empty() {
            return Err(Fail::Errno(errno::NOTEMPTY));
        }
        outcome(self.world.remove(names.join("/"), REMOVE_FOLDER)?)?;
        Ok(())
    }

    /// `path_unlink_file(fd, path, length)`: a file the program doesn't hold open.
    fn path_unlink_file(&mut self, fd: u32, (at, length): (u64, u64)) -> Result<(), Fail> {
        let names = self.resolve(fd, at, length)?;
        if names.is_empty() {
            return Err(Fail::Errno(errno::INVAL));
        }
        let held = self.busy(&names);
        self.files()?;
        match self.found(&names)? {
            None => return Err(Fail::Errno(errno::NOENT)),
            Some(Found::Folder) => return Err(Fail::Errno(errno::ISDIR)),
            Some(Found::File) => {}
        }
        if held {
            return Err(Fail::Errno(errno::BUSY));
        }
        outcome(self.world.remove(names.join("/"), 0)?)?;
        Ok(())
    }

    /// `path_rename(fd, from, from_length, to_fd, to, to_length)`: a file over a file replaces
    /// it, as POSIX has it, on every system.
    fn path_rename(
        &mut self,
        (fd, from, from_length): (u32, u64, u64),
        (to_fd, to, to_length): (u32, u64, u64),
    ) -> Result<(), Fail> {
        let source = self.resolve(fd, from, from_length)?;
        let target = self.resolve(to_fd, to, to_length)?;
        if source.is_empty() || target.is_empty() {
            return Err(Fail::Errno(errno::INVAL));
        }
        if target.len() > source.len() && target.starts_with(&source) {
            return Err(Fail::Errno(errno::INVAL));
        }
        let held = self.busy(&source) || self.busy(&target);
        self.files()?;
        let Some(moving) = self.found(&source)? else {
            return Err(Fail::Errno(errno::NOENT));
        };
        let replaced = self.found(&target)?;
        if held {
            return Err(Fail::Errno(errno::BUSY));
        }
        if source == target {
            return Ok(());
        }
        match (moving, replaced) {
            (_, None) if self.clashes(&target) => return Err(Fail::Errno(errno::EXIST)),
            (Found::Folder, Some(Found::Folder)) if !self.listing(&target).is_empty() => {
                return Err(Fail::Errno(errno::NOTEMPTY));
            }
            (Found::Folder, Some(Found::File)) => return Err(Fail::Errno(errno::NOTDIR)),
            (Found::File, Some(Found::Folder)) => return Err(Fail::Errno(errno::ISDIR)),
            _ => {}
        }
        if moving == Found::Folder {
            return Err(Fail::End(unsupported(
                "renaming a folder, which the model doesn't follow",
            )));
        }
        outcome(self.world.rename(source.join("/"), target.join("/"))?)?;
        Ok(())
    }

    /// Whether the idea reaches `fd` for a wait to read, or to write.
    fn reaches(&self, fd: u32, write: bool) -> bool {
        let policy = self.module.policy;
        match self.core.fds.get(&fd) {
            Some(Handle::Stdin) => !write && policy.reads_input(),
            Some(Handle::Stdout | Handle::Stderr) => write,
            Some(Handle::Folder { .. } | Handle::File { .. }) => policy.files,
            Some(Handle::Listener | Handle::Socket { .. }) => policy.network(),
            // An unknown descriptor's event says so.
            None => true,
        }
    }

    /// What `fd` has now for a wait to read or write: an event's error, bytes and flags, or
    /// `None` while it has nothing and nothing can come, as the listener of a test with no client.
    fn ready(&self, fd: u32, write: bool) -> Result<Option<(u16, u64, u16)>, Fail> {
        let waits = || {
            Fail::End(unsupported(
                "a server's wait on poll_oneoff for its connections, which the WebAssembly model \
                 follows from W9",
            ))
        };
        Ok(match self.core.fds.get(&fd) {
            None => Some((errno::BADF, 0, 0)),
            Some(Handle::Stdin) if !write => {
                if self.world.stdin_length.is_some() {
                    return Err(Fail::End(unsupported(
                        "a wait on standard input of a length the input chooses",
                    )));
                }
                let left = self.world.stdin.len().saturating_sub(self.world.stdin_read) as u64;
                let flags = if left == 0 { event::HANGUP } else { 0 };
                Some((0, left, flags))
            }
            Some(Handle::Stdout | Handle::Stderr) if write => Some((0, 0, 0)),
            Some(Handle::File { .. }) => Some((0, 0, 0)),
            Some(Handle::Listener) if !write => {
                if !self.world.network.client.is_empty() {
                    return Err(waits());
                }
                None
            }
            Some(Handle::Socket { .. }) if write => Some((0, 0, 0)),
            Some(Handle::Socket { .. }) => return Err(waits()),
            Some(_) => Some((errno::BADF, 0, 0)),
        })
    }

    /// The clock's reading now, in nanoseconds, rounded to a microsecond, without moving on to a
    /// test's next one, when the model knows it.
    fn peek(&mut self) -> Option<u64> {
        let readings = Rc::clone(&self.world.clock);
        let last = readings.len().checked_sub(1)?;
        let (seconds, nanoseconds) = readings[self.world.readings_used.min(last)];
        let total = nanoseconds_since(self.pool, seconds, nanoseconds);
        let value = u64::try_from(self.pool.constant(total)?).ok()?;
        let resolution = crate::wasm::host::clock::RESOLUTION;
        Some(value - value % resolution)
    }

    /// `poll_oneoff(in, out, count, events)`: every subscription that's ready now, each as an
    /// event. In a test a wait for times alone, a sleep, ends at once; beside a descriptor, a
    /// wait takes its time, as lengua's host waits in real time then, so where nothing is ready
    /// the waits that come soonest come where that is within a test's time limit, and never
    /// past it. A time on the test's clock that is still to come never does, as the clock's
    /// readings don't move while the program waits.
    #[allow(clippy::too_many_lines)]
    fn poll_oneoff(&mut self, input: u64, output: u64, count: u64, out: u64) -> Result<(), Fail> {
        self.check(out, 4)?;
        if count == 0 || count > 2 * OPEN_FILES {
            return Err(Fail::Errno(errno::INVAL));
        }
        self.check(input, count * u64::from(size::SUBSCRIPTION))?;
        self.check(output, count * u64::from(size::EVENT))?;
        let mut subscriptions = Vec::new();
        for index in 0..count {
            let at = input + index * u64::from(size::SUBSCRIPTION);
            let userdata = memory::read(self.module, self.core, self.pool, at, 8);
            let wanted = match u8::try_from(self.read_number(at + 8, 1)?).unwrap_or(u8::MAX) {
                event::CLOCK => Wanted::Clock {
                    id: self.read_number(at + 16, 4)?,
                    timeout: self.read_number(at + 24, 8)?,
                    absolute: self.read_number(at + 40, 2)? & u64::from(event::ABSTIME) != 0,
                },
                event::FD_READ => {
                    Wanted::Read(u32::try_from(self.read_number(at + 16, 4)?).unwrap_or(u32::MAX))
                }
                event::FD_WRITE => {
                    Wanted::Write(u32::try_from(self.read_number(at + 16, 4)?).unwrap_or(u32::MAX))
                }
                _ => return Err(Fail::Errno(errno::INVAL)),
            };
            subscriptions.push((userdata, wanted));
        }
        // In a test, a sleep ends at once.
        let sleep = subscriptions
            .iter()
            .all(|(_, wanted)| matches!(wanted, Wanted::Clock { .. }));
        let policy = self.module.policy;
        let clocks = policy.clock || policy.terminal;
        // A test's time limit, in nanoseconds: no wait ends past it.
        let test_time = crate::runner::Limits::default().timeout;
        let limit = u64::try_from(test_time.as_nanos()).unwrap_or(u64::MAX);
        let mut happened = Vec::new();
        // The waits that haven't come yet and will within a test's time, with how long until
        // they do.
        let mut coming: Vec<(Term, u64)> = Vec::new();
        for (userdata, wanted) in &subscriptions {
            let userdata = *userdata;
            match *wanted {
                Wanted::Read(fd) | Wanted::Write(fd) => {
                    let write = matches!(wanted, Wanted::Write(_));
                    let kind = if write {
                        event::FD_WRITE
                    } else {
                        event::FD_READ
                    };
                    let found = if self.reaches(fd, write) {
                        self.ready(fd, write)?
                    } else {
                        Some((errno::NOTCAPABLE, 0, 0))
                    };
                    if let Some((error, bytes, flags)) = found {
                        happened.push(Event {
                            userdata,
                            error,
                            kind,
                            bytes,
                            flags,
                        });
                    }
                }
                Wanted::Clock {
                    id,
                    timeout,
                    absolute,
                } => {
                    let known =
                        id == u64::from(clock::REALTIME) || id == u64::from(clock::MONOTONIC);
                    let error = if !clocks {
                        Some(errno::NOTCAPABLE)
                    } else if !known {
                        Some(errno::INVAL)
                    } else if sleep {
                        Some(0)
                    } else {
                        let left = if absolute {
                            let now = self.peek().ok_or_else(|| {
                                Fail::End(unsupported(
                                    "a wait for a time on a clock the test doesn't supply",
                                ))
                            })?;
                            timeout.saturating_sub(now)
                        } else {
                            timeout
                        };
                        if left == 0 {
                            Some(0)
                        } else {
                            // A wait comes once its time has passed, if that is before the
                            // test's limit; a time on the test's clock never does.
                            if !absolute && left < limit {
                                coming.push((userdata, left));
                            }
                            None
                        }
                    };
                    if let Some(error) = error {
                        happened.push(Event {
                            userdata,
                            error,
                            kind: event::CLOCK,
                            bytes: 0,
                            flags: 0,
                        });
                    }
                }
            }
        }
        // Nothing yet: the soonest waits come, as their time passes, and where none will, the
        // test's limit stops the run while it waits.
        if happened.is_empty() {
            let Some(soonest) = coming.iter().map(|(_, left)| *left).min() else {
                return Err(Fail::End(End::Stuck(NEVER_COMES.into())));
            };
            for (userdata, left) in coming {
                if left == soonest {
                    happened.push(Event {
                        userdata,
                        error: 0,
                        kind: event::CLOCK,
                        bytes: 0,
                        flags: 0,
                    });
                }
            }
        }
        for (index, happening) in (0_u64..).zip(&happened) {
            let at = output + index * u64::from(size::EVENT);
            self.zero(at, u64::from(size::EVENT))?;
            self.put_term(at, happening.userdata)?;
            self.put(at + 8, 16, u128::from(happening.error))?;
            self.put(at + 10, 8, u128::from(happening.kind))?;
            self.put(at + 16, 64, u128::from(happening.bytes))?;
            self.put(at + 24, 16, u128::from(happening.flags))?;
        }
        self.put(out, 32, happened.len() as u128)
    }

    /// The connection `fd` is, for a socket's function: its world's descriptor and whether the
    /// program has shut its reading and its writing.
    fn socket(&self, fd: u32) -> Result<(u64, bool, bool), Fail> {
        match self.core.fds.get(&fd) {
            Some(Handle::Socket {
                world,
                read_shut,
                write_shut,
            }) => Ok((*world, *read_shut, *write_shut)),
            Some(Handle::Listener) => Err(Fail::Errno(errno::INVAL)),
            Some(_) => Err(Fail::Errno(errno::NOTSOCK)),
            None => Err(Fail::Errno(errno::BADF)),
        }
    }

    /// `sock_accept(fd, flags, out)`: the test client's next connection on the listener,
    /// descriptor 4, as a new descriptor.
    fn sock_accept(&mut self, fd: u32, flags: u16, out: u64) -> Result<(), Fail> {
        self.check(out, 4)?;
        if self.core.fds.len() as u64 >= OPEN_FILES {
            return Err(Fail::Errno(errno::MFILE));
        }
        match self.core.fds.get(&fd) {
            Some(Handle::Listener) => {}
            Some(Handle::Socket { .. }) => return Err(Fail::Errno(errno::INVAL)),
            Some(_) => return Err(Fail::Errno(errno::NOTSOCK)),
            None => return Err(Fail::Errno(errno::BADF)),
        }
        if flags & fdflags::NONBLOCK != 0 {
            return Err(Fail::End(unsupported(
                "a connection whose reads don't wait, which the model doesn't follow",
            )));
        }
        let opened = outcome(World::accept(self, LISTENED, 0, 0)?)?;
        let accepted = self.add(Handle::Socket {
            world: opened,
            read_shut: false,
            write_shut: false,
        })?;
        self.put(out, 32, u128::from(accepted))
    }

    /// `sock_recv(fd, iovs, count, flags, received, out_flags)`: what arrived, into the first
    /// buffer with room.
    fn sock_recv(
        &mut self,
        fd: u32,
        (iovs, count): (u64, u64),
        flags: u16,
        (received, out_flags): (u64, u64),
    ) -> Result<(), Fail> {
        self.check(received, 4)?;
        self.check(out_flags, 2)?;
        let buffers = self.iovecs(iovs, count)?;
        if flags & !(sock::RECV_PEEK | sock::RECV_WAITALL) != 0 {
            return Err(Fail::Errno(errno::INVAL));
        }
        let (opened, read_shut, _) = self.socket(fd)?;
        if flags != 0 {
            return Err(Fail::End(unsupported(
                "sock_recv that peeks or waits for every buffer, which the model doesn't follow",
            )));
        }
        let total = match buffers.iter().copied().find(|(_, length)| *length > 0) {
            Some((buffer, length)) if !read_shut => self.read_socket(opened, buffer, length)?,
            _ => 0,
        };
        self.put(received, 32, u128::from(total))?;
        self.put(out_flags, 16, 0)
    }

    /// `sock_send(fd, iovs, count, flags, sent)`: every byte of the buffers, in order.
    fn sock_send(&mut self, fd: u32, (iovs, count): (u64, u64), sent: u64) -> Result<(), Fail> {
        self.check(sent, 4)?;
        let buffers = self.iovecs(iovs, count)?;
        self.socket(fd)?;
        let mut total = 0;
        for (buffer, length) in buffers {
            let wrote = match self.send(fd, buffer, length) {
                Ok(wrote) => wrote,
                Err(Fail::Errno(_)) if total > 0 => break,
                Err(failure) => return Err(failure),
            };
            total += wrote;
            if wrote < length {
                break;
            }
        }
        self.put(sent, 32, u128::from(total))
    }

    /// `sock_shutdown(fd, how)`: `how` holds `SHUT_RD`, `SHUT_WR` or both.
    fn sock_shutdown(&mut self, fd: u32, how: u8) -> Result<(), Fail> {
        let (opened, _, shut_before) = self.socket(fd)?;
        if how == 0 || how & !(sock::SHUT_RD | sock::SHUT_WR) != 0 {
            return Err(Fail::Errno(errno::INVAL));
        }
        if let Some(Handle::Socket {
            read_shut,
            write_shut,
            ..
        }) = self.core.fds.get_mut(&fd)
        {
            if how & sock::SHUT_RD != 0 {
                *read_shut = true;
            }
            if how & sock::SHUT_WR != 0 {
                *write_shut = true;
            }
        }
        if how & sock::SHUT_WR != 0 && !shut_before {
            self.world.shutdown(opened, SHUT_WRITE);
        }
        Ok(())
    }
}
