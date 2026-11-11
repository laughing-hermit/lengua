//! `poll_oneoff`: waits until a descriptor has something to read or room to write, or a time
//! comes. A clock subscription needs the clock, or the terminal, named; a descriptor's needs what
//! reaches that descriptor. In a test, a wait for times alone, a sleep, ends at once. Every wait
//! is bounded by the run's deadline and lengua's stop, and a server's wait on its connections
//! tells lengua it is waiting, as a test of a server that keeps serving asks.

use std::time::{Duration, Instant};

use super::fds::Descriptor;
use super::{Fail, Host, memory};
use crate::wasm::wasi::{errno, event, size};

/// The most subscriptions one call takes: a wait to read and one to write on each descriptor a
/// program may hold, as a system's `poll` takes no more than the descriptors a process may hold.
/// lengua keeps what it reads of each while the call waits, so a call can't make it keep more.
const MOST: u64 = 2 * crate::runner::OPEN_FILES;

/// What one subscription waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wanted {
    /// A time on clock `id`: `timeout` nanoseconds from now, or that reading when `absolute`.
    Clock {
        /// The clock.
        id: u32,
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

/// One event, as `poll_oneoff` writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Happened {
    /// The subscription's own word.
    userdata: u64,
    /// The error, 0 for none.
    error: u16,
    /// The subscription's kind.
    kind: u8,
    /// How many bytes there are to read.
    bytes: u64,
    /// `HANGUP` when the other end has ended.
    flags: u16,
}

/// Whether the program's idea reaches `fd` as a wait for reading (`write` false) or writing.
fn reaches(host: &Host, fd: u32, write: bool) -> bool {
    let policy = &host.policy;
    match host.fds.get(fd) {
        Some(Descriptor::Stdin) => !write && policy.reads_input(),
        Some(Descriptor::Stdout | Descriptor::Stderr) => write,
        Some(Descriptor::Folder(_) | Descriptor::File(_)) => policy.files,
        Some(Descriptor::Listener(_) | Descriptor::Socket(_) | Descriptor::Datagram(_)) => {
            policy.network()
        }
        // An unknown descriptor's event says so.
        None => true,
    }
}

/// What `fd` has now for a wait of `kind`: an event, or `None` while it has nothing yet.
fn ready(host: &mut Host, fd: u32, kind: u8) -> Option<(u16, u64, u16)> {
    let write = kind == event::FD_WRITE;
    match host.fds.get_mut(fd) {
        None => Some((errno::BADF, 0, 0)),
        Some(Descriptor::Stdin) if !write => {
            let (bytes, ended) = host.stdin.ready();
            Some((0, bytes, if ended { event::HANGUP } else { 0 }))
        }
        Some(Descriptor::Stdout | Descriptor::Stderr) if write => Some((0, 0, 0)),
        Some(Descriptor::File(_)) => Some((0, 0, 0)),
        Some(Descriptor::Listener(listener)) if !write => listener.ready().then_some((0, 1, 0)),
        Some(Descriptor::Socket(_)) if write => Some((0, 0, 0)),
        Some(Descriptor::Socket(socket)) => socket.readable().then(|| {
            let (bytes, ended) = socket.waiting_bytes();
            (0, bytes, if ended { event::HANGUP } else { 0 })
        }),
        // A datagram goes out at once, and one that arrived is there with its size.
        Some(Descriptor::Datagram(_)) if write => Some((0, 0, 0)),
        Some(Descriptor::Datagram(datagrams)) => datagrams
            .readable()
            .then(|| (0, datagrams.waiting_bytes(), 0)),
        Some(_) => Some((errno::BADF, 0, 0)),
    }
}

/// When a clock subscription's time comes: now for one that has, or how long until it does.
fn until(host: &Host, wanted: Wanted, started: Instant) -> Result<Duration, u16> {
    let Wanted::Clock {
        id,
        timeout,
        absolute,
    } = wanted
    else {
        return Ok(Duration::ZERO);
    };
    if absolute {
        let now = host.clock.peek(id)?;
        return Ok(Duration::from_nanos(timeout.saturating_sub(now)));
    }
    host.clock.peek(id)?;
    Ok(Duration::from_nanos(timeout).saturating_sub(started.elapsed()))
}

/// `poll_oneoff(in, out, count, events)`.
///
/// # Errors
///
/// `INVAL` for no subscriptions, more than a read and a write for each descriptor a program may
/// hold, or one of a kind WASI doesn't have; a buffer outside memory, and a stop at the deadline.
pub fn poll_oneoff(
    host: &mut Host,
    memory: &mut [u8],
    input: u32,
    output: u32,
    count: u32,
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    if count == 0 || u64::from(count) > MOST {
        return Err(Fail::Errno(errno::INVAL));
    }
    let outside = Fail::Stop(super::Stop::OutsideMemory);
    memory::check(
        memory,
        input,
        count.checked_mul(size::SUBSCRIPTION).ok_or(outside)?,
    )?;
    memory::check(
        memory,
        output,
        count.checked_mul(size::EVENT).ok_or(outside)?,
    )?;
    let mut subscriptions = Vec::new();
    for index in 0..count {
        let at = input + index * size::SUBSCRIPTION;
        let userdata = memory::read_u64(memory, at)?;
        let wanted = match memory::read_u8(memory, at + 8)? {
            event::CLOCK => Wanted::Clock {
                id: memory::read_u32(memory, at + 16)?,
                timeout: memory::read_u64(memory, at + 24)?,
                absolute: memory::read_u16(memory, at + 40)? & event::ABSTIME != 0,
            },
            event::FD_READ => Wanted::Read(memory::read_u32(memory, at + 16)?),
            event::FD_WRITE => Wanted::Write(memory::read_u32(memory, at + 16)?),
            _ => return Err(Fail::Errno(errno::INVAL)),
        };
        subscriptions.push((userdata, wanted));
    }
    // In a test, a sleep ends at once.
    let sleep = subscriptions
        .iter()
        .all(|(_, wanted)| matches!(wanted, Wanted::Clock { .. }));
    let instant = host.instant_sleeps && sleep;
    let networked = subscriptions.iter().any(|(_, wanted)| match wanted {
        Wanted::Read(fd) | Wanted::Write(fd) => matches!(
            host.fds.get(*fd),
            Some(Descriptor::Listener(_) | Descriptor::Socket(_) | Descriptor::Datagram(_))
        ),
        Wanted::Clock { .. } => false,
    });
    let clocks = host.policy.clock || host.policy.terminal;
    let started = Instant::now();
    let happened = loop {
        let mut happened = Vec::new();
        let mut soonest: Option<Duration> = None;
        for (userdata, wanted) in &subscriptions {
            let userdata = *userdata;
            let found = match *wanted {
                Wanted::Read(fd) | Wanted::Write(fd) => {
                    let kind = if matches!(wanted, Wanted::Read(_)) {
                        event::FD_READ
                    } else {
                        event::FD_WRITE
                    };
                    if reaches(host, fd, kind == event::FD_WRITE) {
                        ready(host, fd, kind).map(|(error, bytes, flags)| Happened {
                            userdata,
                            error,
                            kind,
                            bytes,
                            flags,
                        })
                    } else {
                        Some(Happened {
                            userdata,
                            error: errno::NOTCAPABLE,
                            kind,
                            bytes: 0,
                            flags: 0,
                        })
                    }
                }
                Wanted::Clock { .. } => {
                    let left = if clocks {
                        until(host, *wanted, started)
                    } else {
                        Err(errno::NOTCAPABLE)
                    };
                    let error = match left {
                        Err(error) => Some(error),
                        Ok(left) if instant || left.is_zero() => Some(0),
                        Ok(left) => {
                            soonest = Some(soonest.map_or(left, |soonest| soonest.min(left)));
                            None
                        }
                    };
                    error.map(|error| Happened {
                        userdata,
                        error,
                        kind: event::CLOCK,
                        bytes: 0,
                        flags: 0,
                    })
                }
            };
            happened.extend(found);
        }
        if !happened.is_empty() {
            break happened;
        }
        if networked {
            host.notes.wait(true);
        }
        host.limits.check().map_err(Fail::Stop)?;
        let nap = soonest.map_or(Duration::from_millis(1), |soonest| {
            soonest.clamp(Duration::from_micros(50), Duration::from_millis(1))
        });
        std::thread::sleep(nap);
    };
    host.notes.wait(false);
    for (index, happening) in (0_u32..).zip(&happened) {
        let at = output + index * size::EVENT;
        memory::zero(memory, at, size::EVENT)?;
        memory::write_u64(memory, at, happening.userdata)?;
        memory::write_u16(memory, at + 8, happening.error)?;
        memory::write_u8(memory, at + 10, happening.kind)?;
        memory::write_u64(memory, at + 16, happening.bytes)?;
        memory::write_u16(memory, at + 24, happening.flags)?;
    }
    memory::write_u32(
        memory,
        out,
        u32::try_from(happened.len()).unwrap_or(u32::MAX),
    )
}
