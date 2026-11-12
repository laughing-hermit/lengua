//! lengua's own WASI preview 1 host, on Wasmtime's [`Linker::func_wrap`], with lengua's semantics
//! rather than another host's: the closed error set, the names every system takes, byte-exact
//! names and no links, folders listed in order and turned round in a test's second run, the
//! clock and random bytes a test supplies, sleeps that end at once in a test, `PIPE` on a closed
//! standard output, `FBIG` past a file's limit, and connections lengua made before the program
//! started, at fixed descriptors. Beside WASI's functions it gives lengua's own, in a module of
//! their own, for what an idea grants a program to open on the network by itself ([`open`]).
//! Every function checks the program's idea ([`policy::Policy`])
//! before it does anything, and every buffer it borrows is checked against the program's memory:
//! one outside it stops the program as a load outside it would.

pub mod clock;
pub mod fds;
pub mod files;
pub mod limits;
pub mod named;
pub mod net;
pub mod open;
pub mod policy;
pub mod poll;
pub mod random;
pub mod start;
pub mod stdio;
pub mod term;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::mpsc::Sender;

use wasmtime::{Caller, Engine, Extern, Linker};

use self::fds::{Descriptor, FIRST_CONNECTION, FOLDER, LISTENER, Table};
use self::files::{Files, Folder};
use self::policy::Policy;
use crate::supply::Time;
use crate::wasm::job::Event;
use crate::wasm::wasi::{self, Function, Unasked, errno, fdflags, filetype, rights, size, whence};

/// Why the host stops the whole program rather than answer it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// It called `proc_exit` with this code, as every system keeps it: its low 8 bits.
    Exit(i32),
    /// It printed past its cap.
    OutputTooLarge,
    /// It ran past its deadline.
    TimedOut,
    /// lengua stopped it.
    Stopped,
    /// A host call's buffer outside its memory.
    OutsideMemory,
    /// It calls the host but exports no memory named `memory`.
    NoMemory,
}

impl fmt::Display for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exit(code) => write!(f, "the program exited with code {code}"),
            Self::OutputTooLarge => f.write_str("the program printed past its limit"),
            Self::TimedOut => f.write_str("the program ran past its time"),
            Self::Stopped => f.write_str("lengua stopped the program"),
            Self::OutsideMemory => f.write_str("a host call's buffer is outside the memory"),
            Self::NoMemory => f.write_str(
                "the module calls the host but exports no memory named \"memory\", which every \
                 host call reads and writes through",
            ),
        }
    }
}

impl std::error::Error for Stop {}

/// What a host call comes to when it doesn't succeed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fail {
    /// An error for the program, a WASI error number.
    Errno(u16),
    /// The whole program stops.
    Stop(Stop),
}

impl From<Stop> for Fail {
    fn from(stop: Stop) -> Self {
        Self::Stop(stop)
    }
}

/// Reading and writing the program's memory, every access checked.
pub mod memory {
    use std::ops::Range;

    use super::{Fail, Stop};
    use crate::wasm::wasi::{errno, size};

    /// What an access outside the memory comes to.
    const OUTSIDE: Fail = Fail::Stop(Stop::OutsideMemory);

    /// The most buffers one call takes, as every system's `readv` and `writev` take no more
    /// (`IOV_MAX`): lengua keeps where each is while the call lasts, so a call can't make it keep
    /// more.
    pub const IOVECS: u32 = 1024;

    /// The `len` bytes at `at`, as a range of `memory`.
    ///
    /// # Errors
    ///
    /// A stop, for bytes outside it.
    pub fn range(memory: &[u8], at: u32, len: u32) -> Result<Range<usize>, Fail> {
        let start = usize::try_from(at).map_err(|_| OUTSIDE)?;
        let len = usize::try_from(len).map_err(|_| OUTSIDE)?;
        let end = start
            .checked_add(len)
            .filter(|end| *end <= memory.len())
            .ok_or(OUTSIDE)?;
        Ok(start..end)
    }

    /// Whether the `len` bytes at `at` are inside `memory`.
    ///
    /// # Errors
    ///
    /// A stop, for bytes outside it.
    pub fn check(memory: &[u8], at: u32, len: u32) -> Result<(), Fail> {
        range(memory, at, len).map(|_| ())
    }

    /// The `len` bytes at `at`.
    ///
    /// # Errors
    ///
    /// A stop, for bytes outside it.
    pub fn bytes(memory: &[u8], at: u32, len: u32) -> Result<&[u8], Fail> {
        Ok(&memory[range(memory, at, len)?])
    }

    /// Zeros the `len` bytes at `at`.
    ///
    /// # Errors
    ///
    /// A stop, for bytes outside it.
    pub fn zero(memory: &mut [u8], at: u32, len: u32) -> Result<(), Fail> {
        let range = range(memory, at, len)?;
        memory[range].fill(0);
        Ok(())
    }

    /// The `N` bytes at `at`.
    fn array<const N: usize>(memory: &[u8], at: u32) -> Result<[u8; N], Fail> {
        let range = range(memory, at, u32::try_from(N).map_err(|_| OUTSIDE)?)?;
        let mut bytes = [0_u8; N];
        bytes.copy_from_slice(&memory[range]);
        Ok(bytes)
    }

    /// Puts `bytes` at `at`.
    fn put(memory: &mut [u8], at: u32, bytes: &[u8]) -> Result<(), Fail> {
        let range = range(memory, at, u32::try_from(bytes.len()).map_err(|_| OUTSIDE)?)?;
        memory[range].copy_from_slice(bytes);
        Ok(())
    }

    /// The byte at `at`.
    ///
    /// # Errors
    ///
    /// A stop, for bytes outside the memory.
    pub fn read_u8(memory: &[u8], at: u32) -> Result<u8, Fail> {
        Ok(array::<1>(memory, at)?[0])
    }

    /// The little-endian 16 bits at `at`.
    ///
    /// # Errors
    ///
    /// A stop, for bytes outside the memory.
    pub fn read_u16(memory: &[u8], at: u32) -> Result<u16, Fail> {
        Ok(u16::from_le_bytes(array(memory, at)?))
    }

    /// The little-endian 32 bits at `at`.
    ///
    /// # Errors
    ///
    /// A stop, for bytes outside the memory.
    pub fn read_u32(memory: &[u8], at: u32) -> Result<u32, Fail> {
        Ok(u32::from_le_bytes(array(memory, at)?))
    }

    /// The little-endian 64 bits at `at`.
    ///
    /// # Errors
    ///
    /// A stop, for bytes outside the memory.
    pub fn read_u64(memory: &[u8], at: u32) -> Result<u64, Fail> {
        Ok(u64::from_le_bytes(array(memory, at)?))
    }

    /// Writes a byte at `at`.
    ///
    /// # Errors
    ///
    /// A stop, for bytes outside the memory.
    pub fn write_u8(memory: &mut [u8], at: u32, value: u8) -> Result<(), Fail> {
        put(memory, at, &[value])
    }

    /// Writes 16 bits at `at`, little-endian.
    ///
    /// # Errors
    ///
    /// A stop, for bytes outside the memory.
    pub fn write_u16(memory: &mut [u8], at: u32, value: u16) -> Result<(), Fail> {
        put(memory, at, &value.to_le_bytes())
    }

    /// Writes 32 bits at `at`, little-endian.
    ///
    /// # Errors
    ///
    /// A stop, for bytes outside the memory.
    pub fn write_u32(memory: &mut [u8], at: u32, value: u32) -> Result<(), Fail> {
        put(memory, at, &value.to_le_bytes())
    }

    /// Writes 64 bits at `at`, little-endian.
    ///
    /// # Errors
    ///
    /// A stop, for bytes outside the memory.
    pub fn write_u64(memory: &mut [u8], at: u32, value: u64) -> Result<(), Fail> {
        put(memory, at, &value.to_le_bytes())
    }

    /// The buffers of the `count` iovecs at `at`, each checked, as ranges of the memory.
    ///
    /// # Errors
    ///
    /// `INVAL` for more than [`IOVECS`], and a stop for a table or a buffer outside the memory.
    pub fn iovecs(memory: &[u8], at: u32, count: u32) -> Result<Vec<Range<usize>>, Fail> {
        if count > IOVECS {
            return Err(Fail::Errno(errno::INVAL));
        }
        check(memory, at, count.checked_mul(size::IOVEC).ok_or(OUTSIDE)?)?;
        let mut buffers = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
        for index in 0..count {
            let entry = at + index * size::IOVEC;
            let buffer = read_u32(memory, entry)?;
            let length = read_u32(memory, entry + 4)?;
            buffers.push(range(memory, buffer, length)?);
        }
        Ok(buffers)
    }
}

/// What the program tells lengua while it runs, besides its output.
#[derive(Debug, Default)]
pub struct Notes {
    /// Whether it listed a folder.
    pub listed: bool,
    /// Whether it waits for a connection now.
    pub waiting: bool,
    /// Where to tell the server when it starts or stops waiting: a job's number and the
    /// worker's line to the server.
    pub events: Option<(u64, Sender<Event>)>,
    /// What became of what it asked to open itself, in order, for the run's report: the first
    /// of them.
    pub opened: Vec<crate::wasm::job::Reached>,
    /// How many of lengua's refusals a run from a shell has said on standard error.
    pub refused: usize,
}

impl Notes {
    /// The program waits now, or is back from waiting: the server is told when that changes.
    pub fn wait(&mut self, waiting: bool) {
        if self.waiting == waiting {
            return;
        }
        self.waiting = waiting;
        if let Some((id, events)) = &self.events {
            let _ = events.send(Event::Waiting { id: *id, waiting });
        }
    }
}

/// What a run gives its program beyond WASI's own world, for what its idea grants it to reach
/// by itself: the files its arguments name, the programs it starts, and the terminal.
#[derive(Default)]
pub struct Beyond {
    /// Where the paths its arguments name are resolved; nowhere when `None`.
    pub ground: Option<crate::named::Ground>,
    /// Its working folder's path, when it has one: no program it starts lies there.
    pub working: Option<std::path::PathBuf>,
    /// The terminal it runs in, for a run from a shell; a test and the `run` tool have none.
    pub terminal: Option<Box<dyn term::Terminal>>,
}

/// What a run gives its program, from which its host is made.
pub struct World {
    /// What its idea lets it reach.
    pub policy: Policy,
    /// Its arguments, after its name.
    pub args: Vec<String>,
    /// Its environment variables, which it gets only when its idea names the environment.
    pub env: BTreeMap<String, String>,
    /// Its standard input.
    pub stdin: stdio::Input,
    /// Its standard output.
    pub stdout: stdio::Output,
    /// Its standard error.
    pub stderr: stdio::Output,
    /// Its working folder, which it gets only when its idea names files.
    pub folder: Option<cap_std::fs::Dir>,
    /// Whether folders list turned round, as in a test's second run.
    pub reversed: bool,
    /// The listener lengua bound for it, for a server.
    pub listener: Option<net::Listener>,
    /// The connections lengua made for it, in the idea's order.
    pub connections: Vec<net::Socket>,
    /// The UDP port lengua bound for it, when its idea receives datagrams.
    pub datagrams: Option<open::Datagrams>,
    /// A test's own internet, which stands in for the real one in everything it opens itself;
    /// `None` for a run that reaches the real one.
    pub internet: Option<crate::wasm::job::Internet>,
    /// The clock readings a test supplies.
    pub clock: Vec<Time>,
    /// The random bytes a test supplies.
    pub random: Option<Vec<u8>>,
    /// How far it may go.
    pub limits: limits::Limits,
    /// Whether a sleep ends at once, as in a test.
    pub instant_sleeps: bool,
    /// What it gets for what its idea grants it to reach by itself.
    pub beyond: Beyond,
    /// Where to tell the server what it does.
    pub events: Option<(u64, Sender<Event>)>,
}

/// One program's host: what the engine's store holds for it.
pub struct Host {
    /// What its idea lets it reach.
    pub policy: Policy,
    /// Its arguments, its name first.
    pub args: Vec<String>,
    /// Its environment, each `NAME=value`.
    pub env: Vec<String>,
    /// Its descriptors.
    pub fds: Table,
    /// Its standard input.
    pub stdin: stdio::Input,
    /// Its standard output.
    pub stdout: stdio::Output,
    /// Its standard error.
    pub stderr: stdio::Output,
    /// Its working folder, when its idea names files.
    pub files: Option<Files>,
    /// The folders its arguments name that it opened, each a root of its own, in the order it
    /// opened them.
    pub named: Vec<Files>,
    /// Which argument, from 1, each of those folders is.
    pub named_arguments: Vec<usize>,
    /// Where the paths its arguments name are resolved.
    pub ground: crate::named::Ground,
    /// Its working folder's path, when it has one.
    pub working: Option<std::path::PathBuf>,
    /// Whether folders list turned round, as in a test's second run.
    pub reversed: bool,
    /// The terminal it runs in. It is put back as it was when the host is dropped, which
    /// every end of a run comes to.
    pub terminal: Box<dyn term::Terminal>,
    /// Its clocks.
    pub clock: clock::Clock,
    /// Its random bytes.
    pub random: random::Random,
    /// How far it may go.
    pub limits: limits::Limits,
    /// What it tells lengua.
    pub notes: Notes,
    /// Whether a sleep ends at once.
    pub instant_sleeps: bool,
    /// A test's own internet, in place of the real one, for what it opens itself.
    pub internet: Option<crate::wasm::job::Internet>,
    /// How lengua finds a name's addresses: the system's resolver, but in lengua's own tests.
    pub(crate) resolver: crate::reach::Resolver,
    /// How lengua finds a program by its name: in the user's `PATH`, but in lengua's own
    /// tests.
    pub(crate) finder: crate::starts::Finder,
}

impl Host {
    /// The host of a program given `world`.
    #[must_use]
    pub fn new(world: World) -> Self {
        let World {
            policy,
            args,
            env,
            stdin,
            stdout,
            stderr,
            folder,
            reversed,
            listener,
            connections,
            datagrams,
            internet,
            clock,
            random,
            limits,
            instant_sleeps,
            events,
            beyond,
        } = world;
        // After the connections lengua made comes the port the idea receives datagrams on.
        let datagrams = datagrams.filter(|_| policy.reach.get().receives.is_some());
        let receiving = u32::try_from(connections.len())
            .unwrap_or(u32::MAX)
            .saturating_add(FIRST_CONNECTION);
        let first = receiving.saturating_add(u32::from(datagrams.is_some()));
        let mut fds = Table::new(first);
        if let Some(datagrams) = datagrams {
            fds.put(receiving, Descriptor::Datagram(Box::new(datagrams)));
        }
        let files = folder.filter(|_| policy.files).map(|root| {
            fds.put(FOLDER, Descriptor::Folder(Folder::default()));
            Files::new(root, reversed)
        });
        if let Some(listener) = listener.filter(|_| policy.listens) {
            fds.put(LISTENER, Descriptor::Listener(Box::new(listener)));
        }
        for (place, socket) in (FIRST_CONNECTION..).zip(connections) {
            fds.put(place, Descriptor::Socket(Box::new(socket)));
        }
        let mut argv = vec![crate::wasm::PROGRAM_NAME.to_owned()];
        argv.extend(args);
        let env = if policy.environment {
            env.into_iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect()
        } else {
            Vec::new()
        };
        Self {
            policy,
            args: argv,
            env,
            fds,
            stdin,
            stdout,
            stderr,
            files,
            named: Vec::new(),
            named_arguments: Vec::new(),
            ground: beyond.ground.unwrap_or(crate::named::Ground::Nowhere),
            working: beyond.working,
            reversed,
            terminal: beyond.terminal.unwrap_or_else(|| Box::new(term::Absent)),
            clock: clock::Clock::new(clock),
            random: random::Random::new(random),
            limits,
            notes: Notes {
                events,
                ..Notes::default()
            },
            instant_sleeps,
            internet,
            resolver: crate::reach::lookup,
            finder: crate::starts::found,
        }
    }
}

/// The table's entry for `name`.
fn spec(name: &str) -> wasmtime::Result<&'static Function> {
    wasi::function(name).ok_or_else(|| wasmtime::Error::msg(format!("WASI has no {name}")))
}

/// `errno` as `function` may give it: one outside its list is `IO`.
fn closed(function: &Function, number: u16) -> u16 {
    if function.errors.contains(&number) {
        number
    } else {
        errno::IO
    }
}

/// Answers a call of `function`: refused when the idea doesn't allow it, else `body`'s answer
/// with the program's memory and host, an error number for the program or a stop of it.
fn answer(
    caller: &mut Caller<'_, Host>,
    function: &'static Function,
    body: impl FnOnce(&mut [u8], &mut Host) -> Result<(), Fail>,
) -> wasmtime::Result<i32> {
    let Some(Extern::Memory(memory)) = caller.get_export("memory") else {
        return Err(wasmtime::Error::new(Stop::NoMemory));
    };
    let (bytes, host) = memory.data_and_store_mut(&mut *caller);
    if !host.policy.allows(function) && function.unasked == Unasked::Refused {
        return Ok(i32::from(errno::NOTCAPABLE));
    }
    match body(bytes, host) {
        Ok(()) => Ok(0),
        Err(Fail::Errno(number)) => Ok(i32::from(closed(function, number))),
        Err(Fail::Stop(stop)) => Err(wasmtime::Error::new(stop)),
    }
}

/// What a function lengua doesn't give answers.
const NOSYS: i32 = 52;

/// The linker with lengua's host: every function of WASI preview 1, for whatever module comes,
/// as a worker that runs module after module needs it.
///
/// # Errors
///
/// When a function can't be defined, which lengua's tests rule out.
pub fn linker(engine: &Engine) -> Result<Linker<Host>, String> {
    let mut linker = Linker::new(engine);
    let mut defining = Defining {
        linker: &mut linker,
        wanted: None,
    };
    define(&mut defining).map_err(|error| format!("lengua's host couldn't be set up: {error}"))?;
    Ok(linker)
}

/// The linker with lengua's host for `module` alone, in its engine: the functions of WASI
/// preview 1 and of lengua's own modules that the module imports, each the very function
/// [`linker`] defines, and no others. A
/// run from a shell runs one module once, and defining a function has a cost of its own (all
/// 46 took 0.04 to 0.08 ms more of a run of 1.1 ms on Linux on x86-64, as W19 measured), so it
/// defines the few the module will call. An import lengua's host doesn't have is undefined here
/// as it is there, and the module doesn't start.
///
/// # Errors
///
/// When a function can't be defined, which lengua's tests rule out.
pub fn linker_for(module: &wasmtime::Module) -> Result<Linker<Host>, String> {
    let wanted: Vec<(&str, &str)> = module
        .imports()
        .map(|import| (import.module(), import.name()))
        .collect();
    let mut linker = Linker::new(module.engine());
    let mut defining = Defining {
        linker: &mut linker,
        wanted: Some(wanted.as_slice()),
    };
    define(&mut defining).map_err(|error| format!("lengua's host couldn't be set up: {error}"))?;
    Ok(linker)
}

/// A linker as [`define`] fills it: with every function, or only with those wanted.
struct Defining<'l> {
    /// The linker.
    linker: &'l mut Linker<Host>,
    /// The functions to define, each by its module and its name, or every one.
    wanted: Option<&'l [(&'l str, &'l str)]>,
}

impl Defining<'_> {
    /// Defines `name` of `module` as `function`, as [`Linker::func_wrap`] does, when it is
    /// wanted.
    fn func_wrap<Params, Results>(
        &mut self,
        module: &str,
        name: &str,
        function: impl wasmtime::IntoFunc<Host, Params, Results>,
    ) -> wasmtime::Result<()> {
        if self
            .wanted
            .is_none_or(|wanted| wanted.contains(&(module, name)))
        {
            self.linker.func_wrap(module, name, function)?;
        }
        Ok(())
    }
}

/// Defines every function on `linker`.
#[allow(clippy::too_many_lines)]
fn define(linker: &mut Defining<'_>) -> wasmtime::Result<()> {
    use wasi::MODULE;
    let unsigned = i32::cast_unsigned;

    let f = spec("args_get")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, argv: i32, buffer: i32| {
            answer(&mut caller, f, |memory, host| {
                strings_get(memory, &host.args, unsigned(argv), unsigned(buffer))
            })
        },
    )?;
    let f = spec("args_sizes_get")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, count: i32, bytes: i32| {
            answer(&mut caller, f, |memory, host| {
                strings_sizes(memory, &host.args, unsigned(count), unsigned(bytes))
            })
        },
    )?;
    let f = spec("environ_get")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, environ: i32, buffer: i32| {
            answer(&mut caller, f, |memory, host| {
                strings_get(memory, &host.env, unsigned(environ), unsigned(buffer))
            })
        },
    )?;
    let f = spec("environ_sizes_get")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, count: i32, bytes: i32| {
            answer(&mut caller, f, |memory, host| {
                strings_sizes(memory, &host.env, unsigned(count), unsigned(bytes))
            })
        },
    )?;
    let f = spec("clock_res_get")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, id: i32, out: i32| {
            answer(&mut caller, f, |memory, _| {
                clock::clock_res_get(memory, unsigned(id), unsigned(out))
            })
        },
    )?;
    let f = spec("clock_time_get")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, id: i32, _precision: i64, out: i32| {
            answer(&mut caller, f, |memory, host| {
                clock::clock_time_get(host, memory, unsigned(id), unsigned(out))
            })
        },
    )?;
    let f = spec("fd_close")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32| {
            answer(&mut caller, f, |_, host| fd_close(host, unsigned(fd)))
        },
    )?;
    let f = spec("fd_fdstat_get")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                fd_fdstat_get(host, memory, unsigned(fd), unsigned(out))
            })
        },
    )?;
    let f = spec("fd_filestat_get")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                fd_filestat_get(host, memory, unsigned(fd), unsigned(out))
            })
        },
    )?;
    let f = spec("fd_prestat_get")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                files::fd_prestat_get(host, memory, unsigned(fd), unsigned(out))
            })
        },
    )?;
    let f = spec("fd_prestat_dir_name")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, path: i32, len: i32| {
            answer(&mut caller, f, |memory, host| {
                files::fd_prestat_dir_name(
                    host,
                    memory,
                    unsigned(fd),
                    unsigned(path),
                    unsigned(len),
                )
            })
        },
    )?;
    let f = spec("fd_read")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, iovs: i32, count: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                fd_read(
                    host,
                    memory,
                    unsigned(fd),
                    (unsigned(iovs), unsigned(count)),
                    unsigned(out),
                )
            })
        },
    )?;
    let f = spec("fd_readdir")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>,
              fd: i32,
              buffer: i32,
              len: i32,
              cookie: i64,
              used: i32| {
            answer(&mut caller, f, |memory, host| {
                files::fd_readdir(
                    host,
                    memory,
                    unsigned(fd),
                    (unsigned(buffer), unsigned(len)),
                    cookie.cast_unsigned(),
                    unsigned(used),
                )
            })
        },
    )?;
    let f = spec("fd_seek")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, offset: i64, from: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                let from = u8::try_from(from).unwrap_or(u8::MAX);
                fd_seek(host, memory, unsigned(fd), (offset, from), unsigned(out))
            })
        },
    )?;
    let f = spec("fd_tell")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                fd_seek(host, memory, unsigned(fd), (0, whence::CUR), unsigned(out))
            })
        },
    )?;
    let f = spec("fd_write")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, iovs: i32, count: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                fd_write(
                    host,
                    memory,
                    unsigned(fd),
                    (unsigned(iovs), unsigned(count)),
                    unsigned(out),
                )
            })
        },
    )?;
    let f = spec("path_create_directory")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, path: i32, len: i32| {
            answer(&mut caller, f, |memory, host| {
                files::path_create_directory(
                    host,
                    memory,
                    unsigned(fd),
                    (unsigned(path), unsigned(len)),
                )
            })
        },
    )?;
    let f = spec("path_filestat_get")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, flags: i32, path: i32, len: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                files::path_filestat_get(
                    host,
                    memory,
                    unsigned(fd),
                    unsigned(flags),
                    (unsigned(path), unsigned(len)),
                    unsigned(out),
                )
            })
        },
    )?;
    let f = spec("path_open")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>,
              fd: i32,
              _dirflags: i32,
              path: i32,
              len: i32,
              oflags: i32,
              base: i64,
              _inheriting: i64,
              flags: i32,
              out: i32| {
            answer(&mut caller, f, |memory, host| {
                let opening = files::Opening {
                    oflags: u16::try_from(oflags & 0xffff).unwrap_or(0),
                    rights: base.cast_unsigned(),
                    fdflags: u16::try_from(flags & 0xffff).unwrap_or(0),
                };
                files::path_open(
                    host,
                    memory,
                    unsigned(fd),
                    (unsigned(path), unsigned(len)),
                    opening,
                    unsigned(out),
                )
            })
        },
    )?;
    let f = spec("path_remove_directory")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, path: i32, len: i32| {
            answer(&mut caller, f, |memory, host| {
                files::path_remove_directory(
                    host,
                    memory,
                    unsigned(fd),
                    (unsigned(path), unsigned(len)),
                )
            })
        },
    )?;
    let f = spec("path_rename")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>,
              fd: i32,
              from: i32,
              from_len: i32,
              to_fd: i32,
              to: i32,
              to_len: i32| {
            answer(&mut caller, f, |memory, host| {
                files::path_rename(
                    host,
                    memory,
                    (unsigned(fd), unsigned(from), unsigned(from_len)),
                    (unsigned(to_fd), unsigned(to), unsigned(to_len)),
                )
            })
        },
    )?;
    let f = spec("path_unlink_file")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, path: i32, len: i32| {
            answer(&mut caller, f, |memory, host| {
                files::path_unlink_file(host, memory, unsigned(fd), (unsigned(path), unsigned(len)))
            })
        },
    )?;
    let f = spec("poll_oneoff")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, input: i32, output: i32, count: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                poll::poll_oneoff(
                    host,
                    memory,
                    unsigned(input),
                    unsigned(output),
                    unsigned(count),
                    unsigned(out),
                )
            })
        },
    )?;
    linker.func_wrap(MODULE, "proc_exit", |code: i32| -> wasmtime::Result<()> {
        Err(wasmtime::Error::new(Stop::Exit(code & 0xff)))
    })?;
    linker.func_wrap(MODULE, "sched_yield", || -> i32 {
        std::thread::yield_now();
        0
    })?;
    let f = spec("random_get")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, buffer: i32, len: i32| {
            answer(&mut caller, f, |memory, host| {
                random::random_get(host, memory, unsigned(buffer), unsigned(len))
            })
        },
    )?;
    let f = spec("sock_accept")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, flags: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                let flags = u16::try_from(flags & 0xffff).unwrap_or(0);
                net::sock_accept(host, memory, unsigned(fd), flags, unsigned(out))
            })
        },
    )?;
    let f = spec("sock_recv")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>,
              fd: i32,
              iovs: i32,
              count: i32,
              flags: i32,
              received: i32,
              out_flags: i32| {
            answer(&mut caller, f, |memory, host| {
                let flags = u16::try_from(flags & 0xffff).unwrap_or(u16::MAX);
                net::sock_recv(
                    host,
                    memory,
                    unsigned(fd),
                    (unsigned(iovs), unsigned(count)),
                    flags,
                    (unsigned(received), unsigned(out_flags)),
                )
            })
        },
    )?;
    let f = spec("sock_send")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>,
              fd: i32,
              iovs: i32,
              count: i32,
              _flags: i32,
              sent: i32| {
            answer(&mut caller, f, |memory, host| {
                net::sock_send(
                    host,
                    memory,
                    unsigned(fd),
                    (unsigned(iovs), unsigned(count)),
                    unsigned(sent),
                )
            })
        },
    )?;
    let f = spec("sock_shutdown")?;
    linker.func_wrap(
        MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, how: i32| {
            answer(&mut caller, f, |_, host| {
                let how = u8::try_from(how).unwrap_or(u8::MAX);
                net::sock_shutdown(host, unsigned(fd), how)
            })
        },
    )?;

    define_net(linker)?;
    define_own(linker)?;

    // The functions lengua doesn't give: they answer NOSYS, so a module that imports one and
    // never calls it runs.
    linker.func_wrap(MODULE, "fd_advise", |_: i32, _: i64, _: i64, _: i32| NOSYS)?;
    linker.func_wrap(MODULE, "fd_allocate", |_: i32, _: i64, _: i64| NOSYS)?;
    linker.func_wrap(MODULE, "fd_datasync", |_: i32| NOSYS)?;
    linker.func_wrap(MODULE, "fd_fdstat_set_flags", |_: i32, _: i32| NOSYS)?;
    linker.func_wrap(MODULE, "fd_fdstat_set_rights", |_: i32, _: i64, _: i64| {
        NOSYS
    })?;
    linker.func_wrap(MODULE, "fd_filestat_set_size", |_: i32, _: i64| NOSYS)?;
    linker.func_wrap(
        MODULE,
        "fd_filestat_set_times",
        |_: i32, _: i64, _: i64, _: i32| NOSYS,
    )?;
    linker.func_wrap(
        MODULE,
        "fd_pread",
        |_: i32, _: i32, _: i32, _: i64, _: i32| NOSYS,
    )?;
    linker.func_wrap(
        MODULE,
        "fd_pwrite",
        |_: i32, _: i32, _: i32, _: i64, _: i32| NOSYS,
    )?;
    linker.func_wrap(MODULE, "fd_renumber", |_: i32, _: i32| NOSYS)?;
    linker.func_wrap(MODULE, "fd_sync", |_: i32| NOSYS)?;
    linker.func_wrap(
        MODULE,
        "path_filestat_set_times",
        |_: i32, _: i32, _: i32, _: i32, _: i64, _: i64, _: i32| NOSYS,
    )?;
    linker.func_wrap(
        MODULE,
        "path_link",
        |_: i32, _: i32, _: i32, _: i32, _: i32, _: i32, _: i32| NOSYS,
    )?;
    linker.func_wrap(
        MODULE,
        "path_readlink",
        |_: i32, _: i32, _: i32, _: i32, _: i32, _: i32| NOSYS,
    )?;
    linker.func_wrap(
        MODULE,
        "path_symlink",
        |_: i32, _: i32, _: i32, _: i32, _: i32| NOSYS,
    )?;
    linker.func_wrap(MODULE, "proc_raise", |_: i32| NOSYS)?;
    Ok(())
}

/// The table's entry for `name` of lengua's own network functions.
fn net_spec(name: &str) -> wasmtime::Result<&'static Function> {
    wasi::imported(wasi::NET_MODULE, name)
        .ok_or_else(|| wasmtime::Error::msg(format!("lengua's host has no {name}")))
}

/// Defines lengua's own network functions on `linker`, in their own module: what a program
/// calls to open what its idea grants at run time.
fn define_net(linker: &mut Defining<'_>) -> wasmtime::Result<()> {
    use wasi::NET_MODULE;
    let unsigned = i32::cast_unsigned;

    let f = net_spec("tcp_open")?;
    linker.func_wrap(
        NET_MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, name: i32, len: i32, port: i32, tls: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                open::tcp_open(
                    host,
                    memory,
                    (unsigned(name), unsigned(len)),
                    unsigned(port),
                    unsigned(tls),
                    unsigned(out),
                )
            })
        },
    )?;
    let f = net_spec("dns_lookup")?;
    linker.func_wrap(
        NET_MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>,
              name: i32,
              len: i32,
              out: i32,
              out_len: i32,
              count: i32| {
            answer(&mut caller, f, |memory, host| {
                open::dns_lookup(
                    host,
                    memory,
                    (unsigned(name), unsigned(len)),
                    (unsigned(out), unsigned(out_len)),
                    unsigned(count),
                )
            })
        },
    )?;
    let f = net_spec("udp_open")?;
    linker.func_wrap(
        NET_MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, name: i32, len: i32, port: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                open::udp_open(
                    host,
                    memory,
                    (unsigned(name), unsigned(len)),
                    unsigned(port),
                    unsigned(out),
                )
            })
        },
    )?;
    let f = net_spec("udp_send")?;
    linker.func_wrap(
        NET_MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, data: i32, len: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                open::udp_send(
                    host,
                    memory,
                    unsigned(fd),
                    (unsigned(data), unsigned(len)),
                    unsigned(out),
                )
            })
        },
    )?;
    let f = net_spec("udp_recv")?;
    linker.func_wrap(
        NET_MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, buffer: i32, len: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                open::udp_recv(
                    host,
                    memory,
                    unsigned(fd),
                    (unsigned(buffer), unsigned(len)),
                    unsigned(out),
                )
            })
        },
    )?;
    let f = net_spec("udp_recv_from")?;
    linker.func_wrap(
        NET_MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, buffer: i32, len: i32, out: i32, from: i32| {
            answer(&mut caller, f, |memory, host| {
                open::udp_recv_from(
                    host,
                    memory,
                    unsigned(fd),
                    (unsigned(buffer), unsigned(len)),
                    unsigned(out),
                    unsigned(from),
                )
            })
        },
    )?;
    let f = net_spec("udp_send_to")?;
    linker.func_wrap(
        NET_MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, data: i32, len: i32, to: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                open::udp_send_to(
                    host,
                    memory,
                    unsigned(fd),
                    (unsigned(data), unsigned(len)),
                    unsigned(to),
                    unsigned(out),
                )
            })
        },
    )?;
    Ok(())
}

/// The table's entry for `name` of `module`, one of lengua's own.
fn own_spec(module: &str, name: &str) -> wasmtime::Result<&'static Function> {
    wasi::imported(module, name)
        .ok_or_else(|| wasmtime::Error::msg(format!("lengua's host has no {name}")))
}

/// Defines lengua's own functions for the files a program's arguments name, the programs it
/// starts and the terminal, each in its own module.
fn define_own(linker: &mut Defining<'_>) -> wasmtime::Result<()> {
    use wasi::{FS_MODULE, RUN_MODULE, TERM_MODULE};
    let unsigned = i32::cast_unsigned;

    let f = own_spec(FS_MODULE, "file_open")?;
    linker.func_wrap(
        FS_MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, argument: i32, mode: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                named::file_open(
                    host,
                    memory,
                    unsigned(argument),
                    unsigned(mode),
                    unsigned(out),
                )
            })
        },
    )?;
    let f = own_spec(FS_MODULE, "folder_open")?;
    linker.func_wrap(
        FS_MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, argument: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                named::folder_open(host, memory, unsigned(argument), unsigned(out))
            })
        },
    )?;
    let f = own_spec(RUN_MODULE, "program_run")?;
    linker.func_wrap(
        RUN_MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>,
              command: i32,
              command_len: i32,
              input: i32,
              input_len: i32,
              output: i32,
              output_len: i32,
              size: i32,
              code: i32| {
            answer(&mut caller, f, |memory, host| {
                start::program_run(
                    host,
                    memory,
                    (unsigned(command), unsigned(command_len)),
                    (unsigned(input), unsigned(input_len)),
                    (unsigned(output), unsigned(output_len)),
                    (unsigned(size), unsigned(code)),
                )
            })
        },
    )?;
    let f = own_spec(TERM_MODULE, "term_is")?;
    linker.func_wrap(
        TERM_MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, fd: i32, out: i32| {
            answer(&mut caller, f, |memory, host| {
                term::term_is(host, memory, unsigned(fd), unsigned(out))
            })
        },
    )?;
    let f = own_spec(TERM_MODULE, "term_size")?;
    linker.func_wrap(
        TERM_MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, columns: i32, rows: i32| {
            answer(&mut caller, f, |memory, host| {
                term::term_size(host, memory, unsigned(columns), unsigned(rows))
            })
        },
    )?;
    let f = own_spec(TERM_MODULE, "term_raw")?;
    linker.func_wrap(
        TERM_MODULE,
        f.name,
        move |mut caller: Caller<'_, Host>, mode: i32| {
            answer(&mut caller, f, |_, host| {
                term::term_raw(host, unsigned(mode))
            })
        },
    )?;
    Ok(())
}

/// The most of what a program asked to reach by itself that a run's report holds, and the most
/// refusals a run from a shell says on standard error.
const NOTED: usize = 64;

impl Host {
    /// Notes what became of something the program asked to reach by itself, for the run's
    /// report: `what` it asked for, and why it didn't get it when it didn't. A refusal by
    /// lengua's rules, `refused`, is said on standard error too in a run from a shell.
    pub fn note(&mut self, what: String, problem: Option<String>, refused: bool) {
        if refused
            && let Some(why) = &problem
            && matches!(self.stderr, stdio::Output::Here(_))
            && self.notes.refused < NOTED
        {
            self.notes.refused += 1;
            eprintln!("lengua-mcp: the program asked for {what}, which lengua refused: {why}");
        }
        if self.notes.opened.len() < NOTED {
            self.notes.opened.push(crate::wasm::job::Reached {
                place: what,
                problem,
            });
        }
    }
}

/// `args_sizes_get` and `environ_sizes_get`: how many `strings`, and their bytes with a NUL each.
fn strings_sizes(
    memory: &mut [u8],
    strings: &[String],
    count: u32,
    bytes: u32,
) -> Result<(), Fail> {
    memory::check(memory, count, 4)?;
    memory::check(memory, bytes, 4)?;
    let total: usize = strings.iter().map(|string| string.len() + 1).sum();
    memory::write_u32(
        memory,
        count,
        u32::try_from(strings.len()).unwrap_or(u32::MAX),
    )?;
    memory::write_u32(memory, bytes, u32::try_from(total).unwrap_or(u32::MAX))
}

/// `args_get` and `environ_get`: each of `strings` with a NUL at `buffer`, one after another, and
/// where each starts at `pointers`.
fn strings_get(
    memory: &mut [u8],
    strings: &[String],
    pointers: u32,
    buffer: u32,
) -> Result<(), Fail> {
    let outside = Fail::Stop(Stop::OutsideMemory);
    let total: usize = strings.iter().map(|string| string.len() + 1).sum();
    let table = u32::try_from(strings.len())
        .ok()
        .and_then(|count| count.checked_mul(4))
        .ok_or(outside)?;
    memory::check(memory, pointers, table)?;
    let area = memory::range(memory, buffer, u32::try_from(total).map_err(|_| outside)?)?;
    let mut at = area.start;
    for (index, string) in (0_u32..).zip(strings) {
        memory::write_u32(
            memory,
            pointers + index * 4,
            u32::try_from(at).map_err(|_| outside)?,
        )?;
        memory[at..at + string.len()].copy_from_slice(string.as_bytes());
        memory[at + string.len()] = 0;
        at += string.len() + 1;
    }
    Ok(())
}

/// WASI's kind for a standard stream: a terminal, or anything else.
const fn stream_kind(terminal: bool) -> u8 {
    if terminal {
        filetype::CHARACTER_DEVICE
    } else {
        filetype::UNKNOWN
    }
}

/// `fd_fdstat_get(fd, out)`: each descriptor's kind and flags, and the rights it reports.
fn fd_fdstat_get(host: &Host, memory: &mut [u8], fd: u32, out: u32) -> Result<(), Fail> {
    memory::check(memory, out, size::FDSTAT)?;
    let poll = rights::POLL_FD_READWRITE;
    let (kind, flags, base, inheriting) = match host.fds.get(fd) {
        None => return Err(Fail::Errno(errno::BADF)),
        Some(Descriptor::Stdin) => (
            stream_kind(host.stdin.terminal()),
            0,
            rights::FD_READ | poll,
            0,
        ),
        Some(Descriptor::Stdout) => (
            stream_kind(host.stdout.terminal()),
            0,
            rights::FD_WRITE | poll,
            0,
        ),
        Some(Descriptor::Stderr) => (
            stream_kind(host.stderr.terminal()),
            0,
            rights::FD_WRITE | poll,
            0,
        ),
        Some(Descriptor::Folder(_)) => (filetype::DIRECTORY, 0, rights::ALL, rights::ALL),
        Some(Descriptor::File(open)) => {
            let mut base = rights::FD_SEEK | rights::FD_TELL | rights::FD_FILESTAT_GET | poll;
            if open.read {
                base |= rights::FD_READ;
            }
            if open.write {
                base |= rights::FD_WRITE;
            }
            let flags = if open.append { fdflags::APPEND } else { 0 };
            (filetype::REGULAR_FILE, flags, base, 0)
        }
        Some(Descriptor::Listener(_)) => (
            filetype::SOCKET_STREAM,
            0,
            rights::SOCK_ACCEPT | rights::FD_READ | poll,
            0,
        ),
        Some(Descriptor::Socket(_)) => (
            filetype::SOCKET_STREAM,
            0,
            rights::FD_READ | rights::FD_WRITE | rights::SOCK_SHUTDOWN | poll,
            0,
        ),
        Some(Descriptor::Datagram(_)) => (filetype::SOCKET_DGRAM, 0, poll, 0),
    };
    memory::zero(memory, out, size::FDSTAT)?;
    memory::write_u8(memory, out, kind)?;
    memory::write_u16(memory, out + 2, flags)?;
    memory::write_u64(memory, out + 8, base)?;
    memory::write_u64(memory, out + 16, inheriting)
}

/// `fd_filestat_get(fd, out)`: a descriptor's kind and size, everything else zero.
fn fd_filestat_get(host: &Host, memory: &mut [u8], fd: u32, out: u32) -> Result<(), Fail> {
    memory::check(memory, out, size::FILESTAT)?;
    let (kind, bytes) = match host.fds.get(fd) {
        None => return Err(Fail::Errno(errno::BADF)),
        Some(Descriptor::File(open)) => (filetype::REGULAR_FILE, files::length_of(open)?),
        Some(Descriptor::Folder(_)) => (filetype::DIRECTORY, 0),
        Some(Descriptor::Stdin) => (stream_kind(host.stdin.terminal()), 0),
        Some(Descriptor::Stdout) => (stream_kind(host.stdout.terminal()), 0),
        Some(Descriptor::Stderr) => (stream_kind(host.stderr.terminal()), 0),
        Some(Descriptor::Listener(_) | Descriptor::Socket(_)) => (filetype::SOCKET_STREAM, 0),
        Some(Descriptor::Datagram(_)) => (filetype::SOCKET_DGRAM, 0),
    };
    files::write_filestat(memory, out, kind, bytes)
}

/// `fd_close(fd)`: a file or folder needs files named, a connection or the listener the network.
fn fd_close(host: &mut Host, fd: u32) -> Result<(), Fail> {
    let policy = host.policy;
    let allowed = match host.fds.get(fd) {
        None => return Err(Fail::Errno(errno::BADF)),
        Some(Descriptor::Listener(_) | Descriptor::Socket(_) | Descriptor::Datagram(_)) => {
            policy.network()
        }
        Some(_) => policy.holds_files() || policy.network(),
    };
    if !allowed {
        return Err(Fail::Errno(errno::NOTCAPABLE));
    }
    if let Some(Descriptor::Socket(mut socket)) = host.fds.remove(fd) {
        socket.close(&host.limits);
    }
    Ok(())
}

/// `fd_seek(fd, offset, whence, out)` and `fd_tell(fd, out)`: only a file has places.
fn fd_seek(
    host: &mut Host,
    memory: &mut [u8],
    fd: u32,
    (offset, from): (i64, u8),
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 8)?;
    let place = match host.fds.get_mut(fd) {
        None | Some(Descriptor::Folder(_)) => return Err(Fail::Errno(errno::BADF)),
        Some(Descriptor::File(open)) => files::seek(open, offset, from)?,
        Some(_) => return Err(Fail::Errno(errno::SPIPE)),
    };
    memory::write_u64(memory, out, place)
}

/// `fd_read(fd, iovs, count, out)`: standard input needs it named, or the terminal; a file, files;
/// a connection, the network. Standard input and a connection fill the first buffer with room,
/// as one read of a pipe or a socket does; a file fills them in order.
fn fd_read(
    host: &mut Host,
    memory: &mut [u8],
    fd: u32,
    (iovs, count): (u32, u32),
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    let buffers = memory::iovecs(memory, iovs, count)?;
    let policy = host.policy;
    let mut total = 0;
    match host.fds.get_mut(fd) {
        None => return Err(Fail::Errno(errno::BADF)),
        Some(Descriptor::Stdin) => {
            if !policy.reads_input() {
                return Err(Fail::Errno(errno::NOTCAPABLE));
            }
            if let Some(range) = buffers.into_iter().find(|range| !range.is_empty()) {
                total = host.stdin.read(&mut memory[range]).map_err(Fail::Errno)?;
            }
        }
        Some(Descriptor::File(open)) => {
            if !policy.holds_files() {
                return Err(Fail::Errno(errno::NOTCAPABLE));
            }
            for range in buffers {
                // A look at the run's deadline before each buffer, which may be large.
                host.limits.check()?;
                let wanted = range.len();
                let got = match files::read(open, &mut memory[range]) {
                    Ok(got) => got,
                    Err(Fail::Errno(_)) if total > 0 => break,
                    Err(failure) => return Err(failure),
                };
                total += got;
                if got < wanted {
                    break;
                }
            }
        }
        Some(Descriptor::Folder(_)) => {
            return Err(Fail::Errno(if policy.holds_files() {
                errno::ISDIR
            } else {
                errno::NOTCAPABLE
            }));
        }
        Some(Descriptor::Socket(socket)) => {
            if !policy.network() {
                return Err(Fail::Errno(errno::NOTCAPABLE));
            }
            if let Some(range) = buffers.into_iter().find(|range| !range.is_empty()) {
                total = socket.read(&mut memory[range], &host.limits, false)?;
            }
        }
        Some(
            Descriptor::Stdout
            | Descriptor::Stderr
            | Descriptor::Listener(_)
            | Descriptor::Datagram(_),
        ) => {
            return Err(Fail::Errno(errno::BADF));
        }
    }
    memory::write_u32(memory, out, u32::try_from(total).unwrap_or(u32::MAX))
}

/// `fd_write(fd, iovs, count, out)`: standard output and error are every program's; a file needs
/// files named, a connection the network.
fn fd_write(
    host: &mut Host,
    memory: &mut [u8],
    fd: u32,
    (iovs, count): (u32, u32),
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    let buffers = memory::iovecs(memory, iovs, count)?;
    let policy = host.policy;
    let mut total = 0;
    match host.fds.get_mut(fd) {
        None => return Err(Fail::Errno(errno::BADF)),
        Some(Descriptor::Stdout) => {
            for range in buffers {
                total += host.stdout.write(&memory[range])?;
            }
        }
        Some(Descriptor::Stderr) => {
            for range in buffers {
                total += host.stderr.write(&memory[range])?;
            }
        }
        Some(Descriptor::File(open)) => {
            if !policy.holds_files() {
                return Err(Fail::Errno(errno::NOTCAPABLE));
            }
            for range in buffers {
                let wanted = range.len();
                let wrote = match files::write(open, &mut host.limits, &memory[range]) {
                    Ok(wrote) => wrote,
                    Err(Fail::Errno(_)) if total > 0 => break,
                    Err(failure) => return Err(failure),
                };
                total += wrote;
                if wrote < wanted {
                    break;
                }
            }
        }
        Some(Descriptor::Socket(socket)) => {
            if !policy.network() {
                return Err(Fail::Errno(errno::NOTCAPABLE));
            }
            for range in buffers {
                let wanted = range.len();
                let sent = match socket.write(&memory[range], &host.limits) {
                    Ok(sent) => sent,
                    Err(Fail::Errno(_)) if total > 0 => break,
                    Err(failure) => return Err(failure),
                };
                total += sent;
                if sent < wanted {
                    break;
                }
            }
        }
        Some(
            Descriptor::Stdin
            | Descriptor::Folder(_)
            | Descriptor::Listener(_)
            | Descriptor::Datagram(_),
        ) => {
            return Err(Fail::Errno(errno::BADF));
        }
    }
    memory::write_u32(memory, out, u32::try_from(total).unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests;
