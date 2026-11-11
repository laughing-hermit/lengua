//! What a program opens itself at run time, with lengua's own functions (`lengua_net_1`): TCP
//! connections, the addresses of names, and UDP datagrams, each only as its idea grants
//! ([`crate::reach`]). Every call reads the computer's name as lengua spells it, finds the rule
//! of the idea that covers it, and only then reaches anything: lengua resolves the name itself,
//! holds each address to the rule, and connects to those addresses alone. Over TLS, lengua does
//! the TLS. In a test, the test's own servers stand in for every computer, and nothing else is
//! reached.
//!
//! A connection the program opened is a descriptor like the ones lengua made: `sock_send`,
//! `sock_recv`, `fd_read`, `fd_write`, `sock_shutdown`, `poll_oneoff` and `fd_close` take it. A
//! program holds at most [`OPEN_AT_ONCE`] of its own at once, and lengua ends one that no byte
//! has crossed for [`IDLE`], whatever the program does meanwhile.

use std::collections::BTreeMap;
use std::io::{self, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use super::fds::Descriptor;
use super::limits::{Limits, SLICE};
use super::net::{FIRST_NAP, Link, Socket, next_nap, nothing_yet};
use super::{Fail, Host, Notes, Stop, memory, stdio};
use crate::connect::CONNECT_TIMEOUT;
use crate::reach::{
    self, ADDRESS_BYTES, Grants, IDLE, MAX_DATAGRAM, MAX_NAME, MAX_SENDERS, OPEN_AT_ONCE,
    PLACE_BYTES, REPLY_FACTOR, Refusal, Resolver, Spelled, Unspelled,
};
use crate::wasm::job::{Internet, Reached};
use crate::wasm::wasi::{errno, net as flags};

/// The most of what a program opened that a run's report says, and the most refusals a run from
/// a shell says on standard error.
const REPORTED: usize = 64;

/// How often lengua looks at the connections it watches.
const LOOK: Duration = Duration::from_millis(250);

/// One connection lengua watches: a second handle on its socket, when a byte last crossed it,
/// and whether the program has closed it.
struct Held {
    /// A handle on the connection's socket, to end it by.
    handle: TcpStream,
    /// When a byte last crossed it, in milliseconds on lengua's own count.
    used: Arc<AtomicU64>,
    /// Set once the program has closed it.
    closed: Arc<AtomicBool>,
    /// How long it may stay with no byte crossing it.
    idle: Duration,
}

/// The connections lengua watches, of every program of this process.
static HELD: Mutex<Vec<Held>> = Mutex::new(Vec::new());

/// Milliseconds since lengua's watch first looked at its clock.
fn now() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    u64::try_from(START.get_or_init(Instant::now).elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Ends every watched connection that no byte has crossed for its time, and forgets those and
/// the closed ones.
fn look() {
    let now = now();
    let mut held = HELD.lock().unwrap_or_else(PoisonError::into_inner);
    held.retain(|held| {
        if held.closed.load(Ordering::SeqCst) {
            return false;
        }
        let idle = now.saturating_sub(held.used.load(Ordering::SeqCst));
        if u128::from(idle) >= held.idle.as_millis() {
            // Both ways: the program's next read gives the end, and its next write fails.
            let _ = held.handle.shutdown(Shutdown::Both);
            return false;
        }
        true
    });
}

/// lengua's watch on one connection a program opened: while the program holds it, lengua ends
/// the connection once no byte has crossed it for its time.
pub(crate) struct Watch {
    /// When a byte last crossed it.
    used: Arc<AtomicU64>,
    /// Set when the program closes it.
    closed: Arc<AtomicBool>,
}

impl Watch {
    /// A byte crossed the connection now.
    pub(crate) fn used(&self) {
        self.used.store(now(), Ordering::SeqCst);
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
    }
}

/// Watches the connection `handle` is a second handle on, which may stay `idle` with no byte
/// crossing it. The watch is a thread of its own, one for the process, so a program that
/// computes without a call of the host is watched all the same.
///
/// # Errors
///
/// When the watch's thread can't start: a connection lengua can't watch isn't opened.
pub(crate) fn watch(handle: TcpStream, idle: Duration) -> io::Result<Watch> {
    static WATCHING: OnceLock<bool> = OnceLock::new();
    let watching = *WATCHING.get_or_init(|| {
        std::thread::Builder::new()
            .name("lengua idle connections".to_owned())
            .spawn(|| {
                loop {
                    std::thread::sleep(LOOK);
                    look();
                }
            })
            .is_ok()
    });
    if !watching {
        return Err(io::Error::other(
            "lengua couldn't start its watch on idle connections",
        ));
    }
    let watch = Watch {
        used: Arc::new(AtomicU64::new(now())),
        closed: Arc::new(AtomicBool::new(false)),
    };
    HELD.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(Held {
            handle,
            used: Arc::clone(&watch.used),
            closed: Arc::clone(&watch.closed),
            idle,
        });
    Ok(watch)
}

/// Who sent datagrams to the port a program receives on, and how much went each way.
struct Heard {
    /// Whether other computers may send to the port.
    anywhere: bool,
    /// Each sender, with the bytes it sent and the bytes the program sent back.
    senders: BTreeMap<SocketAddr, (u64, u64)>,
}

impl Heard {
    /// `from` sent `count` bytes.
    fn heard(&mut self, from: SocketAddr, count: usize) {
        if !self.senders.contains_key(&from) && self.senders.len() >= MAX_SENDERS {
            self.senders.pop_first();
        }
        let entry = self.senders.entry(from).or_insert((0, 0));
        entry.0 = entry
            .0
            .saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
    }
}

/// A socket for UDP datagrams: one a program opened to one computer, which sends there and
/// receives only from there, or the port its idea receives on, which answers its senders.
pub struct Datagrams {
    /// The socket, which never waits in the system; none for a test's computer with nothing
    /// there, which takes what is sent and answers that nothing is.
    socket: Option<UdpSocket>,
    /// For the port the idea receives on: its senders.
    bound: Option<Heard>,
    /// A datagram taken while looking whether one was there, with its sender.
    ahead: Option<(Vec<u8>, SocketAddr)>,
}

impl Datagrams {
    /// The port `port` of this computer's loopback, or of every address when `anywhere`, for the
    /// datagrams an idea receives; 0 for any free port.
    ///
    /// # Errors
    ///
    /// When the port can't be had, such as one that's taken.
    pub fn bind(port: u16, anywhere: bool) -> io::Result<Self> {
        let address = if anywhere {
            Ipv4Addr::UNSPECIFIED
        } else {
            Ipv4Addr::LOCALHOST
        };
        let socket = UdpSocket::bind((address, port))?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket: Some(socket),
            bound: Some(Heard {
                anywhere,
                senders: BTreeMap::new(),
            }),
            ahead: None,
        })
    }

    /// The port it is bound to.
    ///
    /// # Errors
    ///
    /// When the system can't say.
    pub fn port(&self) -> io::Result<u16> {
        match &self.socket {
            Some(socket) => Ok(socket.local_addr()?.port()),
            None => Err(io::Error::other("nothing is bound")),
        }
    }

    /// A socket for datagrams to and from `address` alone.
    fn to(address: SocketAddr) -> io::Result<Self> {
        let own: SocketAddr = match address {
            SocketAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, 0).into(),
            SocketAddr::V6(_) => (Ipv6Addr::UNSPECIFIED, 0).into(),
        };
        let socket = UdpSocket::bind(own)?;
        socket.connect(address)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket: Some(socket),
            bound: None,
            ahead: None,
        })
    }

    /// A test's computer with nothing there.
    const fn nowhere() -> Self {
        Self {
            socket: None,
            bound: None,
            ahead: None,
        }
    }

    /// Whether it is the port the idea receives on, rather than one the program opened.
    #[must_use]
    pub const fn is_bound(&self) -> bool {
        self.bound.is_some()
    }

    /// Takes a datagram that has arrived, if one has, without waiting.
    fn look(&mut self) -> Result<(), u16> {
        if self.ahead.is_some() {
            return Ok(());
        }
        let Some(socket) = &self.socket else {
            return Err(errno::CONNREFUSED);
        };
        let mut chunk = vec![0_u8; 65_536];
        loop {
            match socket.recv_from(&mut chunk) {
                Ok((count, from)) => {
                    chunk.truncate(count);
                    if let Some(heard) = &mut self.bound {
                        heard.heard(from, count);
                    }
                    self.ahead = Some((chunk, from));
                    return Ok(());
                }
                Err(error) if nothing_yet(&error) => return Ok(()),
                // What a system says of an earlier datagram that found nothing there: on the
                // port the idea receives on it is some sender's own affair.
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::ConnectionRefused | ErrorKind::ConnectionReset
                    ) =>
                {
                    if self.bound.is_none() {
                        return Err(errno::CONNREFUSED);
                    }
                }
                Err(_) => return Err(errno::IO),
            }
        }
    }

    /// Whether a receive would give something now: a datagram, or an error.
    pub fn readable(&mut self) -> bool {
        self.look().is_err() || self.ahead.is_some()
    }

    /// How many bytes the datagram that has arrived holds.
    #[must_use]
    pub fn waiting_bytes(&self) -> u64 {
        self.ahead.as_ref().map_or(0, |(bytes, _)| {
            u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        })
    }

    /// The next datagram and its sender, waiting for it within the run's limits. A wait on the
    /// port the idea receives on tells lengua the program waits, as a server's wait for a
    /// connection does.
    fn receive(
        &mut self,
        limits: &Limits,
        notes: &mut Notes,
    ) -> Result<(Vec<u8>, SocketAddr), Fail> {
        let mut nap = FIRST_NAP;
        let mut waits = false;
        loop {
            let looked = self.look();
            if let Some(got) = self.ahead.take() {
                notes.wait(false);
                return Ok(got);
            }
            if let Err(number) = looked {
                notes.wait(false);
                return Err(Fail::Errno(number));
            }
            if self.bound.is_some() && !waits {
                notes.wait(true);
                waits = true;
            }
            let slice = limits.slice(SLICE)?;
            std::thread::sleep(nap.min(slice));
            nap = next_nap(nap);
        }
    }

    /// Sends `bytes` as one datagram, to `to` or to the one computer the socket is for.
    fn send(&self, bytes: &[u8], to: Option<SocketAddr>, limits: &Limits) -> Result<usize, Fail> {
        if bytes.len() > MAX_DATAGRAM {
            return Err(Fail::Errno(errno::MSGSIZE));
        }
        let Some(socket) = &self.socket else {
            return Ok(bytes.len());
        };
        let mut nap = FIRST_NAP;
        loop {
            let sent = match to {
                Some(to) => socket.send_to(bytes, to),
                None => socket.send(bytes),
            };
            match sent {
                Ok(count) => return Ok(count),
                Err(error) if nothing_yet(&error) => {
                    let slice = limits.slice(SLICE)?;
                    std::thread::sleep(nap.min(slice));
                    nap = next_nap(nap);
                }
                // What a system says of an earlier datagram that found nothing there: a
                // datagram may be lost on its way, and this one is.
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::ConnectionRefused | ErrorKind::ConnectionReset
                    ) =>
                {
                    return Ok(bytes.len());
                }
                Err(_) => return Err(Fail::Errno(errno::IO)),
            }
        }
    }
}

/// Why something a program asked to open wasn't: lengua's refusal, with an error number for
/// the program, or a stop of the whole program.
enum Unopened {
    /// lengua didn't open it.
    Refused(Refusal),
    /// The program stops.
    Stop(Fail),
}

impl From<Refusal> for Unopened {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

impl From<Stop> for Unopened {
    fn from(stop: Stop) -> Self {
        Self::Stop(Fail::Stop(stop))
    }
}

/// The error number a refusal gives the program.
const fn errno_of(refusal: &Refusal) -> u16 {
    match refusal {
        Refusal::NotAPlace(_) => errno::INVAL,
        Refusal::TooLong(_) => errno::NAMETOOLONG,
        Refusal::NotGranted(_) => errno::NOTCAPABLE,
        Refusal::NoAddress(_) => errno::NOENT,
        Refusal::NotMade(_) => errno::CONNREFUSED,
        Refusal::NoAnswer(_) => errno::TIMEDOUT,
        Refusal::Full(_) => errno::MFILE,
    }
}

/// A refusal for something the system couldn't do.
fn failed(what: &str, error: &io::Error) -> Unopened {
    Unopened::Refused(Refusal::NotMade(format!("{what}: {error}")))
}

/// The text at `name`, as a report shows it: at most a name's length of it, with what isn't
/// text written out.
fn shown(memory: &[u8], name: (u32, u32)) -> Result<String, Fail> {
    let bytes = memory::bytes(memory, name.0, name.1)?;
    let kept = &bytes[..bytes.len().min(MAX_NAME + 1)];
    Ok(String::from_utf8_lossy(kept).escape_debug().to_string())
}

/// The computer the text at `name` names, as lengua spells it.
fn computer(memory: &[u8], name: (u32, u32)) -> Result<Result<Spelled, Refusal>, Fail> {
    let bytes = memory::bytes(memory, name.0, name.1)?;
    let long = || Refusal::TooLong(format!("a computer's name has at most {MAX_NAME} bytes"));
    if bytes.len() > MAX_NAME + 1 {
        return Ok(Err(long()));
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Ok(Err(Refusal::NotAPlace(
            "the program named a computer in bytes that aren't text".to_owned(),
        )));
    };
    Ok(match reach::spelled(text) {
        Ok(spelled) => Ok(spelled),
        Err(Unspelled::ThisComputer) => Err(Refusal::NotGranted(format!(
            "`{}` is this computer, which a program never reaches by a name or an address: an \
             idea reaches this computer by a port alone",
            text.escape_debug()
        ))),
        Err(Unspelled::NotAName) => Err(Refusal::NotAPlace(format!(
            "`{}` isn't a computer's name or address",
            text.escape_debug()
        ))),
        Err(Unspelled::TooLong) => Err(long()),
    })
}

/// The port `port` is.
fn port_of(port: u32) -> Result<u16, Refusal> {
    u16::try_from(port)
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| Refusal::NotAPlace(format!("{port} is no port: ports go from 1 to 65535")))
}

/// How many connections and datagram sockets of its own the program holds open.
fn held(host: &Host) -> usize {
    host.fds
        .iter()
        .filter(|(_, descriptor)| match descriptor {
            Descriptor::Socket(socket) => socket.is_opened(),
            Descriptor::Datagram(datagrams) => !datagrams.is_bound(),
            _ => false,
        })
        .count()
}

/// Refuses one more connection or socket of the program's own when it holds the most it may.
fn room(host: &Host) -> Result<(), Refusal> {
    if held(host) >= OPEN_AT_ONCE {
        return Err(Refusal::Full(format!(
            "the program holds {OPEN_AT_ONCE} connections of its own open, the most it may at \
             once: it closes one first"
        )));
    }
    Ok(())
}

/// Notes what became of something the program asked to open, for the run's report, and, in a
/// run from a shell, says on standard error what lengua's rules refused.
fn note(host: &mut Host, place: String, refusal: Option<&Refusal>) {
    if let Some(Refusal::NotGranted(why)) = refusal
        && matches!(host.stderr, stdio::Output::Here(_))
        && host.notes.refused < REPORTED
    {
        host.notes.refused += 1;
        eprintln!("lengua-mcp: the program asked for {place}, which lengua refused: {why}");
    }
    if host.notes.opened.len() < REPORTED {
        host.notes.opened.push(Reached {
            place,
            problem: refusal.map(|refusal| refusal.words().to_owned()),
        });
    }
}

/// What a call that opens something comes to: `opened`'s answer, noted for the report as
/// `place`.
fn answer<T>(host: &mut Host, place: String, opened: Result<T, Unopened>) -> Result<T, Fail> {
    match opened {
        Ok(opened) => {
            note(host, place, None);
            Ok(opened)
        }
        Err(Unopened::Stop(fail)) => Err(fail),
        Err(Unopened::Refused(refusal)) => {
            note(host, place, Some(&refusal));
            Err(Fail::Errno(errno_of(&refusal)))
        }
    }
}

/// The addresses lengua may reach `computer` at: its own, when it is an address, or the ones
/// `resolver` finds for its name, asked once, each held to the rule ([`reach::vetted`]).
fn addresses_of(
    resolver: Resolver,
    computer: &Spelled,
    private: bool,
    within: Duration,
) -> Result<Vec<IpAddr>, Refusal> {
    let found = match computer.address {
        Some(address) => vec![address],
        None => resolver(&computer.text, within)?,
    };
    reach::vetted(&computer.text, &found, private)
}

/// The addresses a test's internet has for `computer`: its own, when it is an address, or the
/// ones the test gives its name; none for a name the test gives none.
fn test_addresses(internet: &Internet, computer: &Spelled) -> Vec<IpAddr> {
    match computer.address {
        Some(address) => vec![address],
        None => internet
            .names
            .iter()
            .find(|(name, _)| *name == computer.text)
            .map(|(_, addresses)| {
                addresses
                    .iter()
                    .filter_map(|address| address.parse().ok())
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// The port of the loopback where a test's own server stands in for `computer` on `port`, when
/// the test has one. The addresses the test gives the computer are held to the rule first, as a
/// run holds the real ones.
fn stand_in(
    internet: &Internet,
    computer: &Spelled,
    port: u16,
    datagrams: bool,
    private: bool,
) -> Result<Option<u16>, Refusal> {
    let found = test_addresses(internet, computer);
    if !found.is_empty() {
        reach::vetted(&computer.text, &found, private)?;
    }
    Ok(internet
        .places
        .iter()
        .find(|(stand, _)| {
            stand.datagrams == datagrams && stand.host == computer.text && stand.port == port
        })
        .map(|(_, local)| *local))
}

/// The connection `tcp_open` asks for, when the idea grants it and the place takes it.
fn open_tcp(
    host: &Host,
    computer: Result<Spelled, Refusal>,
    port: u32,
    tls: u32,
) -> Result<Socket, Unopened> {
    let tls = match tls {
        flags::PLAIN => false,
        flags::TLS => true,
        other => {
            return Err(Refusal::NotAPlace(format!(
                "{other} says neither a plain connection (0) nor one over TLS (1)"
            ))
            .into());
        }
    };
    let port = port_of(port)?;
    let computer = computer?;
    let place = format!("{}:{port}", computer.text);
    let reach = host.policy.reach.get();
    let Some(rule) = Grants::granted(&reach.opens, &computer, port, Some(tls), Some(&host.args))
    else {
        return Err(Refusal::NotGranted(format!(
            "the idea grants no TCP connection to {place}{}",
            if tls { " over TLS" } else { ", in plain" }
        ))
        .into());
    };
    room(host)?;
    let private = rule.reaches_private(&computer);
    let within = host.limits.slice(CONNECT_TIMEOUT)?;
    let started = Instant::now();
    let left = || {
        within
            .saturating_sub(started.elapsed())
            .max(Duration::from_millis(1))
    };
    let stream = match &host.internet {
        Some(internet) => match stand_in(internet, &computer, port, false, private)? {
            Some(local) => reach::dial(&place, &[IpAddr::V4(Ipv4Addr::LOCALHOST)], local, within)?,
            None => {
                return Err(Refusal::NotMade(format!(
                    "the test has no server at {place}, so nothing listens there"
                ))
                .into());
            }
        },
        None => {
            let addresses = addresses_of(host.resolver, &computer, private, within)?;
            reach::dial(&place, &addresses, port, left())?
        }
    };
    let handle = stream
        .try_clone()
        .map_err(|error| failed("lengua couldn't watch the connection", &error))?;
    let watch = watch(handle, IDLE)
        .map_err(|error| failed("lengua couldn't watch the connection", &error))?;
    // In a test the bytes are plain: lengua does TLS only in a run, where the program can't
    // tell the difference.
    let link = if tls && host.internet.is_none() {
        let (secured, runtime) =
            reach::secure(stream, &computer, crate::connect::tls_config(), left())?;
        Link::Place(secured, runtime)
    } else {
        Link::tcp(stream).map_err(|error| failed("the connection couldn't be kept", &error))?
    };
    Ok(Socket::opened(link, watch))
}

/// `tcp_open(name, name_len, port, tls, out_fd)`: opens a TCP connection to `port` of the
/// computer the text at `name` names, over TLS when `tls` is 1, and writes its descriptor.
///
/// # Errors
///
/// `NOTCAPABLE` for what the idea doesn't grant, this computer and the addresses lengua refuses;
/// `INVAL` for what names no computer or no port; `NAMETOOLONG`; `NOENT` for a name with no
/// address; `CONNREFUSED` for a place that didn't take the connection, or whose certificate
/// lengua couldn't check; `TIMEDOUT`; `MFILE` at the most a program holds; a buffer outside
/// memory, and a stop at the deadline.
pub fn tcp_open(
    host: &mut Host,
    memory: &mut [u8],
    name: (u32, u32),
    port: u32,
    tls: u32,
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    let place = format!(
        "a TCP connection to port {port} of `{}`{}",
        shown(memory, name)?,
        if tls == flags::TLS { " over TLS" } else { "" }
    );
    let opened = open_tcp(host, computer(memory, name)?, port, tls);
    let socket = answer(host, place, opened)?;
    let fd = host
        .fds
        .add(Descriptor::Socket(Box::new(socket)))
        .map_err(Fail::Errno)?;
    memory::write_u32(memory, out, fd)
}

/// The addresses `dns_lookup` asks for, when the idea grants them: IPv4 first, each family in
/// order, so they come the same way on every system.
fn look_up(host: &Host, computer: Result<Spelled, Refusal>) -> Result<Vec<IpAddr>, Unopened> {
    let computer = computer?;
    let reach = host.policy.reach.get();
    let Some(rule) = Grants::granted(&reach.names, &computer, 0, None, Some(&host.args)) else {
        return Err(Refusal::NotGranted(format!(
            "the idea grants no look at the addresses of {}",
            computer.text
        ))
        .into());
    };
    let private = rule.reaches_private(&computer);
    let within = host.limits.slice(CONNECT_TIMEOUT)?;
    let mut addresses = match &host.internet {
        Some(internet) => {
            let found = test_addresses(internet, &computer);
            reach::vetted(&computer.text, &found, private)?
        }
        None => addresses_of(host.resolver, &computer, private, within)?,
    };
    addresses.sort_by_key(|address| (address.is_ipv6(), *address));
    Ok(addresses)
}

/// Writes `address` at `at`: its family, then 16 bytes, an IPv4 address in the first four.
fn put_address(memory: &mut [u8], at: u32, address: IpAddr) -> Result<(), Fail> {
    let range = memory::range(memory, at, ADDRESS_BYTES)?;
    let slot = &mut memory[range];
    slot.fill(0);
    match address {
        IpAddr::V4(four) => {
            slot[0] = flags::FAMILY_4;
            slot[1..5].copy_from_slice(&four.octets());
        }
        IpAddr::V6(six) => {
            slot[0] = flags::FAMILY_6;
            slot[1..17].copy_from_slice(&six.octets());
        }
    }
    Ok(())
}

/// `dns_lookup(name, name_len, out, out_len, out_count)`: writes the addresses of the name at
/// `name`, [`ADDRESS_BYTES`] each, as many as `out_len` has room for, and how many it wrote.
///
/// # Errors
///
/// `NOTCAPABLE` for a name the idea doesn't grant, or one with an address lengua refuses;
/// `INVAL`; `NAMETOOLONG`; `NOENT` for a name with no address; `TIMEDOUT`; a buffer outside
/// memory, and a stop at the deadline.
pub fn dns_lookup(
    host: &mut Host,
    memory: &mut [u8],
    name: (u32, u32),
    out: (u32, u32),
    count: u32,
) -> Result<(), Fail> {
    memory::check(memory, count, 4)?;
    memory::check(memory, out.0, out.1)?;
    let place = format!("the addresses of `{}`", shown(memory, name)?);
    let found = look_up(host, computer(memory, name)?);
    let addresses = answer(host, place, found)?;
    let room = out.1 / ADDRESS_BYTES;
    let mut written = 0;
    for (index, address) in (0..room).zip(addresses) {
        put_address(memory, out.0 + index * ADDRESS_BYTES, address)?;
        written = index + 1;
    }
    memory::write_u32(memory, count, written)
}

/// The socket `udp_open` asks for, when the idea grants it.
fn open_udp(
    host: &Host,
    computer: Result<Spelled, Refusal>,
    port: u32,
) -> Result<Datagrams, Unopened> {
    let port = port_of(port)?;
    let computer = computer?;
    let place = format!("{}:{port}", computer.text);
    let reach = host.policy.reach.get();
    let Some(rule) = Grants::granted(&reach.datagrams, &computer, port, None, Some(&host.args))
    else {
        return Err(
            Refusal::NotGranted(format!("the idea grants no UDP datagrams to {place}")).into(),
        );
    };
    room(host)?;
    let private = rule.reaches_private(&computer);
    let within = host.limits.slice(CONNECT_TIMEOUT)?;
    let address = match &host.internet {
        Some(internet) => match stand_in(internet, &computer, port, true, private)? {
            Some(local) => SocketAddr::from((Ipv4Addr::LOCALHOST, local)),
            None => return Ok(Datagrams::nowhere()),
        },
        None => {
            let addresses = addresses_of(host.resolver, &computer, private, within)?;
            let Some(first) = addresses.first() else {
                return Err(Refusal::NoAddress(format!("{place} has no address")).into());
            };
            SocketAddr::new(*first, port)
        }
    };
    Datagrams::to(address).map_err(|error| failed("the socket couldn't be made", &error))
}

/// `udp_open(name, name_len, port, out_fd)`: a socket for datagrams to and from `port` of the
/// computer the text at `name` names, and writes its descriptor.
///
/// # Errors
///
/// As [`tcp_open`], but nothing is connected, so no place refuses here.
pub fn udp_open(
    host: &mut Host,
    memory: &mut [u8],
    name: (u32, u32),
    port: u32,
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    let place = format!(
        "UDP datagrams to port {port} of `{}`",
        shown(memory, name)?
    );
    let opened = open_udp(host, computer(memory, name)?, port);
    let datagrams = answer(host, place, opened)?;
    let fd = host
        .fds
        .add(Descriptor::Datagram(Box::new(datagrams)))
        .map_err(Fail::Errno)?;
    memory::write_u32(memory, out, fd)
}

/// The datagram socket `fd` is, with the run's limits and what the program tells lengua.
fn datagrams(host: &mut Host, fd: u32) -> Result<(&mut Datagrams, &Limits, &mut Notes), Fail> {
    let Host {
        fds, limits, notes, ..
    } = host;
    match fds.get_mut(fd) {
        Some(Descriptor::Datagram(datagrams)) => Ok((datagrams.as_mut(), &*limits, notes)),
        Some(_) => Err(Fail::Errno(errno::NOTSOCK)),
        None => Err(Fail::Errno(errno::BADF)),
    }
}

/// The count `bytes` is, as a function writes it.
fn count_of(bytes: usize) -> u32 {
    u32::try_from(bytes).unwrap_or(u32::MAX)
}

/// `udp_send(fd, data, len, out_sent)`: sends the bytes as one datagram to the computer the
/// socket was opened for.
///
/// # Errors
///
/// `MSGSIZE` for more than a datagram holds, `BADF` for the port the idea receives on, which
/// answers a sender with `udp_send_to`; a buffer outside memory, and a stop at the deadline.
pub fn udp_send(
    host: &mut Host,
    memory: &mut [u8],
    fd: u32,
    data: (u32, u32),
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    let range = memory::range(memory, data.0, data.1)?;
    let (datagrams, limits, _) = datagrams(host, fd)?;
    if datagrams.is_bound() {
        return Err(Fail::Errno(errno::BADF));
    }
    let sent = datagrams.send(&memory[range], None, limits)?;
    memory::write_u32(memory, out, count_of(sent))
}

/// `udp_recv(fd, buffer, len, out_size)`: the next datagram from the computer the socket was
/// opened for, cut to the buffer, and its whole size.
///
/// # Errors
///
/// `CONNREFUSED` when nothing is there to answer, `BADF` for the port the idea receives on,
/// which `udp_recv_from` reads; a buffer outside memory, and a stop at the deadline.
pub fn udp_recv(
    host: &mut Host,
    memory: &mut [u8],
    fd: u32,
    buffer: (u32, u32),
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    let range = memory::range(memory, buffer.0, buffer.1)?;
    let (datagrams, limits, notes) = datagrams(host, fd)?;
    if datagrams.is_bound() {
        return Err(Fail::Errno(errno::BADF));
    }
    let (bytes, _) = datagrams.receive(limits, notes)?;
    let kept = bytes.len().min(range.len());
    memory[range.start..range.start + kept].copy_from_slice(&bytes[..kept]);
    memory::write_u32(memory, out, count_of(bytes.len()))
}

/// Writes `place` at `at`: its address, then its port, little-endian.
fn put_place(memory: &mut [u8], at: u32, place: SocketAddr) -> Result<(), Fail> {
    memory::check(memory, at, PLACE_BYTES)?;
    put_address(memory, at, place.ip())?;
    memory::write_u16(memory, at + ADDRESS_BYTES, place.port())
}

/// The place written at `at`, if it is one.
fn place_at(memory: &[u8], at: u32) -> Result<Option<SocketAddr>, Fail> {
    let bytes = memory::bytes(memory, at, PLACE_BYTES)?;
    let port = u16::from_le_bytes([bytes[17], bytes[18]]);
    let address = match bytes[0] {
        flags::FAMILY_4 => IpAddr::V4(Ipv4Addr::new(bytes[1], bytes[2], bytes[3], bytes[4])),
        flags::FAMILY_6 => {
            let mut octets = [0_u8; 16];
            octets.copy_from_slice(&bytes[1..17]);
            IpAddr::V6(Ipv6Addr::from(octets))
        }
        _ => return Ok(None),
    };
    Ok(Some(SocketAddr::new(address, port)))
}

/// `udp_recv_from(fd, buffer, len, out_size, out_from)`: the next datagram on the port the idea
/// receives on, cut to the buffer, its whole size, and its sender, [`PLACE_BYTES`] bytes.
///
/// # Errors
///
/// `BADF` for a socket the program opened, which `udp_recv` reads; a buffer outside memory, and
/// a stop at the deadline.
pub fn udp_recv_from(
    host: &mut Host,
    memory: &mut [u8],
    fd: u32,
    buffer: (u32, u32),
    out: u32,
    from: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    memory::check(memory, from, PLACE_BYTES)?;
    let range = memory::range(memory, buffer.0, buffer.1)?;
    let (datagrams, limits, notes) = datagrams(host, fd)?;
    if !datagrams.is_bound() {
        return Err(Fail::Errno(errno::BADF));
    }
    let (bytes, sender) = datagrams.receive(limits, notes)?;
    let kept = bytes.len().min(range.len());
    memory[range.start..range.start + kept].copy_from_slice(&bytes[..kept]);
    put_place(memory, from, sender)?;
    memory::write_u32(memory, out, count_of(bytes.len()))
}

/// `udp_send_to(fd, data, len, to, out_sent)`: sends the bytes as one datagram to `to`, a
/// sender of the port the idea receives on. A program answers only who sent it something; on a
/// port any computer may reach, it sends an address at most [`REPLY_FACTOR`] times the bytes
/// that address sent, since a sender can write another computer's address on its datagrams.
///
/// # Errors
///
/// `NOTCAPABLE` for an address that sent nothing, or more than it may be sent; `INVAL` for what
/// is no address; `MSGSIZE`; `BADF` for a socket the program opened; a buffer outside memory,
/// and a stop at the deadline.
pub fn udp_send_to(
    host: &mut Host,
    memory: &mut [u8],
    fd: u32,
    data: (u32, u32),
    to: u32,
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    let range = memory::range(memory, data.0, data.1)?;
    let Some(to) = place_at(memory, to)? else {
        return Err(Fail::Errno(errno::INVAL));
    };
    let (datagrams, limits, _) = datagrams(host, fd)?;
    let size = u64::try_from(range.len()).unwrap_or(u64::MAX);
    let Some(heard) = &datagrams.bound else {
        return Err(Fail::Errno(errno::BADF));
    };
    let Some((received, sent)) = heard.senders.get(&to).copied() else {
        return Err(Fail::Errno(errno::NOTCAPABLE));
    };
    if heard.anywhere && sent.saturating_add(size) > received.saturating_mul(REPLY_FACTOR) {
        return Err(Fail::Errno(errno::NOTCAPABLE));
    }
    let count = datagrams.send(&memory[range], Some(to), limits)?;
    if let Some(heard) = &mut datagrams.bound
        && let Some(entry) = heard.senders.get_mut(&to)
    {
        entry.1 = entry.1.saturating_add(size);
    }
    memory::write_u32(memory, out, count_of(count))
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;
    use std::sync::atomic::AtomicUsize;

    use super::*;
    use crate::reach::Stand;
    use crate::wasm::host::policy::{Policy, Reach};
    use crate::wasm::host::tests::world;

    /// A host whose idea's `- Network:` lines say `lines`, in a test's internet when given.
    fn host_with(lines: &[&str], internet: Option<Internet>) -> Host {
        let mut grants = Grants::default();
        for line in lines {
            assert_eq!(grants.line(line), Some(Ok(())), "{line}");
        }
        let policy = Policy {
            reach: Reach::of(&grants),
            ..Policy::default()
        };
        let mut world = world(policy);
        world.internet = internet;
        world.args = vec!["second.example".to_owned()];
        Host::new(world)
    }

    /// A page of memory with `name` at 100.
    fn memory_with(name: &str) -> (Vec<u8>, (u32, u32)) {
        let mut memory = vec![0_u8; 65_536];
        memory[100..100 + name.len()].copy_from_slice(name.as_bytes());
        (memory, (100, u32::try_from(name.len()).expect("a short name")))
    }

    /// What `tcp_open` answers for `name` on `port`: the descriptor, or the error number.
    fn opened(host: &mut Host, name: &str, port: u16, tls: u32) -> Result<u32, u16> {
        let (mut memory, at) = memory_with(name);
        match tcp_open(host, &mut memory, at, u32::from(port), tls, 8) {
            Ok(()) => Ok(memory::read_u32(&memory, 8).expect("the descriptor")),
            Err(Fail::Errno(number)) => Err(number),
            Err(Fail::Stop(stop)) => panic!("{name}: {stop}"),
        }
    }

    /// A test's internet with a server for `host` on `port`, TCP or UDP, at the loopback's port
    /// `local`.
    fn internet(datagrams: bool, host: &str, port: u16, local: u16) -> Internet {
        Internet {
            places: vec![(
                Stand {
                    datagrams,
                    host: host.to_owned(),
                    port,
                },
                local,
            )],
            names: Vec::new(),
        }
    }

    /// A listener on a free port of the loopback, which takes every connection and keeps it.
    fn keeping_listener() -> u16 {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
        let port = listener.local_addr().expect("its address").port();
        std::thread::spawn(move || {
            let mut kept = Vec::new();
            for stream in listener.incoming() {
                kept.push(stream);
            }
        });
        port
    }

    /// The threat: a program reaches a computer its idea doesn't grant, as after a redirect it
    /// read. Only what a rule covers is opened: the computers, the ports, and TLS or not, as the
    /// rule says.
    #[test]
    fn a_program_opens_only_what_its_idea_grants() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
        let local = listener.local_addr().expect("its address").port();
        let lines = [
            "opens tcp connections to port 80 on any subdomain of example.org",
            "opens tcp connections to port 443 on the computer its first argument names over tls",
        ];
        let mut host = host_with(
            &lines,
            Some(internet(false, "www.example.org", 80, local)),
        );
        let fd = opened(&mut host, "WWW.Example.org.", 80, flags::PLAIN).expect("it opens");
        assert!(fd >= super::super::fds::FIRST_CONNECTION, "{fd}");
        // The connection is one like any other: what the program writes arrives.
        let (mut other, _) = listener.accept().expect("the program's connection");
        let Some(Descriptor::Socket(socket)) = host.fds.get_mut(fd) else {
            panic!("no connection at {fd}");
        };
        assert_eq!(socket.write(b"hi", &host.limits).ok(), Some(2));
        let mut got = [0_u8; 2];
        other.read_exact(&mut got).expect("what it wrote");
        assert_eq!(&got, b"hi");
        other.write_all(b"yo").expect("an answer");
        let mut back = [0_u8; 2];
        assert_eq!(socket.read(&mut back, &host.limits, false).ok(), Some(2));
        assert_eq!(&back, b"yo");
        // A computer the idea grants, with no server in this test: nothing listens there.
        assert_eq!(
            opened(&mut host, "api.example.org", 80, flags::PLAIN),
            Err(errno::CONNREFUSED)
        );
        // The computer its argument names is granted, over TLS as its rule says.
        assert_eq!(
            opened(&mut host, "second.example", 443, flags::TLS),
            Err(errno::CONNREFUSED)
        );
        // A program whose idea grants nothing opens nothing.
        let mut bare = host_with(&[], Some(internet(false, "www.example.org", 80, local)));
        assert_eq!(
            opened(&mut bare, "www.example.org", 80, flags::PLAIN),
            Err(errno::NOTCAPABLE)
        );
    }

    /// The threat: a redirect, or anything else a program read, to a computer its idea doesn't
    /// name, or to this computer under some spelling. Each is refused before anything is
    /// resolved or reached, and the run's report says so.
    #[test]
    fn what_no_rule_covers_is_refused_and_reported() {
        let lines = [
            "opens tcp connections to port 80 on any subdomain of example.org",
            "opens tcp connections to port 443 on the computer its first argument names over tls",
        ];
        let mut host = host_with(&lines, Some(Internet::default()));
        for (name, port, tls) in [
            ("evil.test", 80, flags::PLAIN),
            ("example.org", 80, flags::PLAIN),
            ("www.example.org.evil.test", 80, flags::PLAIN),
            ("www.example.org", 8080, flags::PLAIN),
            ("www.example.org", 80, flags::TLS),
            ("second.example", 443, flags::PLAIN),
            ("third.example", 443, flags::TLS),
            ("192.0.2.7", 80, flags::PLAIN),
            ("localhost", 80, flags::PLAIN),
            ("127.0.0.1", 80, flags::PLAIN),
            ("::ffff:127.0.0.1", 80, flags::PLAIN),
        ] {
            assert_eq!(
                opened(&mut host, name, port, tls),
                Err(errno::NOTCAPABLE),
                "{name}:{port}"
            );
        }
        for nothing in ["2130706433", "127.1", "0x7f.1", "", "a b", "www.example.org:80"] {
            assert_eq!(
                opened(&mut host, nothing, 80, flags::PLAIN),
                Err(errno::INVAL),
                "{nothing}"
            );
        }
        assert_eq!(opened(&mut host, "www.example.org", 80, 2), Err(errno::INVAL));
        let (mut memory, at) = memory_with("www.example.org");
        assert_eq!(
            tcp_open(&mut host, &mut memory, at, 0, flags::PLAIN, 8),
            Err(Fail::Errno(errno::INVAL))
        );
        assert_eq!(
            tcp_open(&mut host, &mut memory, at, 65_536, flags::PLAIN, 8),
            Err(Fail::Errno(errno::INVAL))
        );
        assert_eq!(
            opened(&mut host, &"a".repeat(300), 80, flags::PLAIN),
            Err(errno::NAMETOOLONG)
        );
        // What became of each is in the run's report.
        let first = &host.notes.opened[0];
        assert!(first.place.contains("port 80 of `evil.test`"), "{first:?}");
        assert!(
            first.problem.as_deref().is_some_and(|why| {
                why.contains("the idea grants no TCP connection to evil.test:80")
            }),
            "{first:?}"
        );
    }

    /// What a test's resolver says of each name: this computer under four writings, a private
    /// network, and a mix.
    fn tricked(name: &str, _: Duration) -> Result<Vec<IpAddr>, Refusal> {
        let texts: &[&str] = match name {
            "loop.example" => &["127.0.0.1"],
            "six.example" => &["::1"],
            "mapped.example" => &["::ffff:127.0.0.1"],
            "any.example" => &["0.0.0.0"],
            "mixed.example" => &["192.0.2.1", "127.0.0.1"],
            "private.example" => &["192.0.2.1", "10.0.0.1"],
            "cloud.example" => &["169.254.169.254"],
            _ => return Err(Refusal::NoAddress(format!("{name} has no address"))),
        };
        Ok(texts.iter().filter_map(|text| text.parse().ok()).collect())
    }

    /// The threat: DNS tricks, the loopback and the private ranges. A name any computer may
    /// have resolves to this computer, or into a private network: lengua looks at the addresses
    /// its own resolver found, and connects to none of them. Something listens on the loopback,
    /// so only the refusal keeps the connection from being made.
    #[test]
    fn a_name_that_resolves_to_this_computer_or_a_private_network_is_not_reached() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
        listener.set_nonblocking(true).expect("a listener that never waits");
        let port = listener.local_addr().expect("its address").port();
        let line = format!("opens tcp connections to port {port} on any computer");
        let mut host = host_with(&[line.as_str()], None);
        host.resolver = tricked;
        for name in [
            "loop.example",
            "six.example",
            "mapped.example",
            "any.example",
            "mixed.example",
            "private.example",
            "cloud.example",
        ] {
            assert_eq!(
                opened(&mut host, name, port, flags::PLAIN),
                Err(errno::NOTCAPABLE),
                "{name}"
            );
        }
        assert_eq!(
            opened(&mut host, "nothing.example", port, flags::PLAIN),
            Err(errno::NOENT)
        );
        // An address of a private network, written by the program: refused as a name's is.
        assert_eq!(
            opened(&mut host, "10.0.0.1", port, flags::PLAIN),
            Err(errno::NOTCAPABLE)
        );
        assert_eq!(
            opened(&mut host, "::ffff:10.0.0.1", port, flags::PLAIN),
            Err(errno::NOTCAPABLE)
        );
        assert!(
            listener.accept().is_err(),
            "a connection to this computer was made"
        );
    }

    /// How many times [`turning`] was asked.
    static ASKED: AtomicUsize = AtomicUsize::new(0);

    /// A resolver whose answer changes: another computer the first time, this one the next
    /// times, and then none.
    fn turning(name: &str, _: Duration) -> Result<Vec<IpAddr>, Refusal> {
        match ASKED.fetch_add(1, Ordering::SeqCst) {
            0 => Ok(vec![Ipv4Addr::new(192, 0, 2, 1).into()]),
            1..=8 => Ok(vec![Ipv4Addr::LOCALHOST.into()]),
            _ => Err(Refusal::NoAddress(format!("{name} has no address"))),
        }
    }

    /// The threat: a name that resolves anew between lengua's check and its connection
    /// (rebinding). lengua asks once, and connects to the addresses it checked and no others: a
    /// second answer, this computer's, is never asked for, and the listener there gets nothing.
    #[test]
    fn a_name_is_resolved_once_and_only_its_checked_addresses_are_dialed() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
        listener.set_nonblocking(true).expect("a listener that never waits");
        let port = listener.local_addr().expect("its address").port();
        let line = format!("opens tcp connections to port {port} on any computer");
        let mut host = host_with(&[line.as_str()], None);
        host.resolver = turning;
        // No more than this for an address that doesn't answer.
        host.limits.deadline = Some(Instant::now() + Duration::from_millis(500));
        let refused = opened(&mut host, "turning.example", port, flags::PLAIN);
        assert!(
            matches!(refused, Err(errno::CONNREFUSED | errno::TIMEDOUT)),
            "{refused:?}"
        );
        assert_eq!(ASKED.load(Ordering::SeqCst), 1, "the name was asked for again");
        assert!(
            listener.accept().is_err(),
            "a connection to this computer was made"
        );
    }

    /// The threat: connections held past a limit, in number. A program holds at most
    /// [`OPEN_AT_ONCE`] of its own; closing one makes room for one.
    #[test]
    fn a_program_holds_at_most_so_many_connections_of_its_own() {
        // Room for this test's sockets, as a process that runs programs makes it.
        crate::wasm::runner::make_room().expect("room for the files a program may hold");
        let local = keeping_listener();
        let mut host = host_with(
            &["opens tcp connections to port 80 on www.example.org"],
            Some(internet(false, "www.example.org", 80, local)),
        );
        let mut held = Vec::new();
        for _ in 0..OPEN_AT_ONCE {
            held.push(opened(&mut host, "www.example.org", 80, flags::PLAIN).expect("room"));
        }
        assert_eq!(
            opened(&mut host, "www.example.org", 80, flags::PLAIN),
            Err(errno::MFILE)
        );
        let closed = held.pop().expect("the last one");
        assert!(host.fds.remove(closed).is_some());
        assert!(opened(&mut host, "www.example.org", 80, flags::PLAIN).is_ok());
        assert_eq!(
            opened(&mut host, "www.example.org", 80, flags::PLAIN),
            Err(errno::MFILE)
        );
    }

    /// The threat: connections held past a limit, in time. lengua ends a connection the program
    /// opened once no byte has crossed it for its time, though the program never calls the host
    /// meanwhile: its next read gives the end at once, where an unwatched connection would wait
    /// for the other end. One that bytes keep crossing stays.
    #[test]
    fn a_connection_no_byte_crosses_is_ended() {
        let pair = || {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
            let port = listener.local_addr().expect("its address").port();
            let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).expect("a connection");
            let (other, _) = listener.accept().expect("its other end");
            let watch = watch(
                stream.try_clone().expect("a second handle"),
                Duration::from_millis(600),
            )
            .expect("the watch");
            let link = Link::tcp(stream).expect("a stream that never waits");
            (Socket::opened(link, watch), other)
        };
        let limits = Limits::new(
            Some(Duration::from_secs(20)),
            Arc::new(AtomicBool::new(false)),
            100,
            None,
            None,
            0,
        );
        let (mut idle, _silent) = pair();
        let (mut busy, mut talking) = pair();
        let mut buffer = [0_u8; 4];
        // Bytes cross the busy one for two seconds, well past the idle time.
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(200));
            talking.write_all(b"x").expect("a byte");
            assert_eq!(busy.read(&mut buffer, &limits, false).ok(), Some(1));
        }
        let started = Instant::now();
        assert_eq!(idle.read(&mut buffer, &limits, false).ok(), Some(0));
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the read waited for the other end"
        );
        assert!(matches!(
            idle.write(b"x", &limits),
            Err(Fail::Errno(errno::PIPE))
        ));
    }

    /// `dns_lookup` gives a name's addresses only as the idea grants them, in one order, and
    /// never an address lengua wouldn't connect to.
    #[test]
    fn a_names_addresses_are_given_only_as_the_idea_grants() {
        let net = Internet {
            places: Vec::new(),
            names: vec![
                (
                    "www.example.org".to_owned(),
                    vec!["2001:db8::1".to_owned(), "192.0.2.7".to_owned()],
                ),
                ("inside.example.org".to_owned(), vec!["10.0.0.1".to_owned()]),
                ("here.example.org".to_owned(), vec!["127.0.0.1".to_owned()]),
            ],
        };
        let mut host = host_with(
            &["looks up the addresses of any subdomain of example.org"],
            Some(net),
        );
        let looked = |host: &mut Host, name: &str, room: u32| {
            let (mut memory, at) = memory_with(name);
            let answered = dns_lookup(host, &mut memory, at, (1000, room), 8);
            let count = memory::read_u32(&memory, 8).expect("the count");
            (answered, count, memory[1000..1034].to_vec())
        };
        let (answered, count, bytes) = looked(&mut host, "www.example.org", 34);
        assert_eq!(answered, Ok(()));
        assert_eq!(count, 2);
        assert_eq!(&bytes[..5], &[4, 192, 0, 2, 7]);
        assert_eq!(&bytes[5..17], &[0; 12]);
        assert_eq!(&bytes[17..21], &[6, 0x20, 0x01, 0x0d]);
        // Room for one: one is written.
        let (answered, count, _) = looked(&mut host, "www.example.org", 20);
        assert_eq!((answered, count), (Ok(()), 1));
        for (name, number) in [
            ("inside.example.org", errno::NOTCAPABLE),
            ("here.example.org", errno::NOTCAPABLE),
            ("other.example.org", errno::NOENT),
            ("example.com", errno::NOTCAPABLE),
            ("localhost", errno::NOTCAPABLE),
            ("0x7f.1", errno::INVAL),
        ] {
            let (answered, _, _) = looked(&mut host, name, 34);
            assert_eq!(answered, Err(Fail::Errno(number)), "{name}");
        }
    }

    /// A program sends datagrams only to the computers and ports its idea grants, and never to
    /// an address many computers answer at, which one datagram would reach all of: the threat of
    /// UDP used to amplify.
    #[test]
    fn datagrams_go_only_where_the_idea_grants() {
        let peer = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
        peer.set_read_timeout(Some(Duration::from_secs(10)))
            .expect("a timeout");
        let local = peer.local_addr().expect("its address").port();
        let mut host = host_with(
            &[
                "sends udp datagrams to port 53 on 192.0.2.53",
                "sends udp datagrams to port 5353 on any computer, private addresses too",
            ],
            Some(internet(true, "192.0.2.53", 53, local)),
        );
        let open = |host: &mut Host, name: &str, port: u32| {
            let (mut memory, at) = memory_with(name);
            udp_open(host, &mut memory, at, port, 8)
                .map(|()| memory::read_u32(&memory, 8).expect("the descriptor"))
        };
        let fd = open(&mut host, "192.0.2.53", 53).expect("it opens");
        let (mut memory, _) = memory_with("question");
        assert_eq!(udp_send(&mut host, &mut memory, fd, (100, 8), 8), Ok(()));
        assert_eq!(memory::read_u32(&memory, 8).ok(), Some(8));
        let mut got = [0_u8; 64];
        let (count, from) = peer.recv_from(&mut got).expect("the datagram");
        assert_eq!(&got[..count], b"question");
        peer.send_to(b"a long answer", from).expect("an answer");
        // The buffer takes 6 bytes; the size is the whole datagram's.
        assert_eq!(udp_recv(&mut host, &mut memory, fd, (200, 6), 8), Ok(()));
        assert_eq!(memory::read_u32(&memory, 8).ok(), Some(13));
        assert_eq!(&memory[200..207], b"a long\0");
        for (name, port) in [("192.0.2.53", 54), ("192.0.2.54", 53), ("localhost", 53)] {
            assert_eq!(
                open(&mut host, name, port),
                Err(Fail::Errno(errno::NOTCAPABLE)),
                "{name}:{port}"
            );
        }
        // Every computer of a network at once, or a group of them: never, whatever is granted.
        for many in ["255.255.255.255", "224.0.0.251", "ff02::fb"] {
            assert_eq!(
                open(&mut host, many, 5353),
                Err(Fail::Errno(errno::NOTCAPABLE)),
                "{many}"
            );
        }
        // More than one datagram holds.
        let size = u32::try_from(MAX_DATAGRAM).expect("a size") + 1;
        let mut large = vec![0_u8; 131_072];
        assert_eq!(
            udp_send(&mut host, &mut large, fd, (1000, size), 8),
            Err(Fail::Errno(errno::MSGSIZE))
        );
        // A computer the idea grants with nothing there in this test: what is sent is lost, and
        // nothing answers.
        let nowhere = open(&mut host, "192.0.2.99", 5353).expect("it opens");
        assert_eq!(udp_send(&mut host, &mut memory, nowhere, (100, 8), 8), Ok(()));
        assert_eq!(
            udp_recv(&mut host, &mut memory, nowhere, (200, 6), 8),
            Err(Fail::Errno(errno::CONNREFUSED))
        );
        assert_eq!(
            udp_send(&mut host, &mut memory, 1, (100, 8), 8),
            Err(Fail::Errno(errno::NOTSOCK))
        );
    }

    /// The threat: UDP used to amplify. On the port its idea receives on, a program answers
    /// only an address that sent it something, and, where any computer may send, with at most
    /// [`REPLY_FACTOR`] times the bytes that address sent: a sender can write another computer's
    /// address on its datagrams, and the answers go there.
    #[test]
    fn a_program_answers_only_its_senders_and_not_with_much_more() {
        for anywhere in [false, true] {
            let bound = Datagrams::bind(0, anywhere).expect("a free port");
            let port = bound.port().expect("its port");
            let mut world = world(Policy::default());
            world.datagrams = Some(bound);
            let mut grants = Grants::default();
            assert_eq!(grants.line("receives udp datagrams on port 9"), Some(Ok(())));
            world.policy.reach = Reach::of(&grants);
            let mut host = Host::new(world);
            let fd = super::super::fds::FIRST_CONNECTION;
            assert!(matches!(host.fds.get(fd), Some(Descriptor::Datagram(_))));
            let peer = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
            peer.set_read_timeout(Some(Duration::from_secs(10)))
                .expect("a timeout");
            peer.send_to(b"0123456789", (Ipv4Addr::LOCALHOST, port))
                .expect("a datagram");
            let mut memory = vec![7_u8; 4096];
            assert_eq!(
                udp_recv_from(&mut host, &mut memory, fd, (200, 64), 8, 300),
                Ok(())
            );
            assert_eq!(memory::read_u32(&memory, 8).ok(), Some(10));
            assert_eq!(&memory[200..210], b"0123456789");
            let sender = place_at(&memory, 300).expect("inside memory");
            assert_eq!(sender, peer.local_addr().ok());
            // Thirty bytes back, three times what it sent.
            assert_eq!(
                udp_send_to(&mut host, &mut memory, fd, (1000, 30), 300, 8),
                Ok(())
            );
            let mut got = [0_u8; 64];
            assert_eq!(peer.recv(&mut got).ok(), Some(30));
            let more = udp_send_to(&mut host, &mut memory, fd, (1000, 1), 300, 8);
            if anywhere {
                assert_eq!(more, Err(Fail::Errno(errno::NOTCAPABLE)));
            } else {
                assert_eq!(more, Ok(()));
            }
            // An address that sent nothing gets nothing: here, the same computer's next port.
            let other = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
            put_place(&mut memory, 400, other.local_addr().expect("its address"))
                .expect("inside memory");
            assert_eq!(
                udp_send_to(&mut host, &mut memory, fd, (1000, 1), 400, 8),
                Err(Fail::Errno(errno::NOTCAPABLE))
            );
            memory[500] = 9;
            assert_eq!(
                udp_send_to(&mut host, &mut memory, fd, (1000, 1), 500, 8),
                Err(Fail::Errno(errno::INVAL))
            );
            // The port the idea receives on is read and answered with its own two functions.
            assert_eq!(
                udp_send(&mut host, &mut memory, fd, (1000, 1), 8),
                Err(Fail::Errno(errno::BADF))
            );
        }
    }
}
