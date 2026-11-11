//! What a program may reach on the network at run time, which its idea grants on `- Network:`
//! lines: the TCP connections it opens itself, the names whose addresses it looks up, and the
//! UDP datagrams it sends and receives. Without such a line a program opens nothing: it gets
//! only the connections lengua makes before it starts ([`crate::connect`]).
//!
//! A grant names computers and ports. lengua holds every connection a program opens to it, in
//! this order:
//! 1. the computer's name is read as lengua spells it ([`spelled`]): lower case, with no dot at
//!    its end, an address in its one usual writing, and never a number only an older parsing
//!    takes for an address;
//! 2. a rule of the idea must cover that name and the port ([`Rule::covers`]);
//! 3. lengua resolves the name itself, once, and every address it found is held to the rule
//!    ([`vetted`]): none is this computer's, none is one many computers answer at, and none is
//!    in a private network unless the rule says so;
//! 4. lengua connects to those addresses and no others ([`dial`]), so a name that resolves
//!    anew between the check and the connection changes nothing, and over TLS it checks the
//!    computer's certificate for the name.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::connect::{Place, this_computer};
use crate::idea::{HostKind, host_kind};

/// The most rules of one kind an idea's `- Network:` lines may give.
pub const MAX_RULES: usize = 16;

/// The most connections and datagram sockets a program holds open at once, of those it opened
/// itself.
pub const OPEN_AT_ONCE: usize = 64;

/// How long a connection a program opened may stay with no byte sent or received on it before
/// lengua ends it.
pub const IDLE: Duration = Duration::from_secs(300);

/// The most addresses lengua keeps of those a name resolves to.
pub const MAX_ADDRESSES: usize = 16;

/// The most bytes a computer's name holds, as DNS has it.
pub const MAX_NAME: usize = 253;

/// The most bytes one datagram holds, as UDP over IPv4 carries them.
pub const MAX_DATAGRAM: usize = 65_507;

/// How many times the bytes an address sent it a program may send back to that address, on a
/// port any computer may reach: a sender can write another computer's address on its
/// datagrams, and the answers then go to that computer.
pub const REPLY_FACTOR: u64 = 3;

/// The most senders lengua remembers for the port a program receives datagrams on.
pub const MAX_SENDERS: usize = 1024;

/// The bytes of one address as `dns_lookup` writes it: its family, 4 or 6, then 16 bytes, an
/// IPv4 address in the first four.
pub const ADDRESS_BYTES: u32 = 17;

/// The bytes of a datagram's sender: an address as [`ADDRESS_BYTES`] has it, then its port,
/// little-endian.
pub const PLACE_BYTES: u32 = 19;

/// The computers a rule names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Computers {
    /// One computer, by its name or its address, as lengua spells it.
    Named(String),
    /// Every name beneath this domain, and not the domain itself: `any subdomain of
    /// example.com`.
    Beneath(String),
    /// The computer the program's argument at this place, from 1, names.
    Argument(usize),
    /// Any computer.
    Any,
}

impl Computers {
    /// The computers in words, as an idea writes them.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Named(name) => name.clone(),
            Self::Beneath(domain) => format!("any subdomain of {domain}"),
            Self::Argument(index) => format!(
                "the computer its {} argument names",
                crate::idea::ORDINALS
                    .get(index.saturating_sub(1))
                    .copied()
                    .unwrap_or("first")
            ),
            Self::Any => "any computer".to_owned(),
        }
    }

    /// The computers `text` names, in lower case.
    ///
    /// # Errors
    ///
    /// What's wrong with the words, and what to write instead.
    pub fn read(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if matches!(text, "any computer" | "any computers") {
            return Ok(Self::Any);
        }
        if let Some(domain) = text.strip_prefix("any subdomain of ") {
            let domain = domain.trim();
            return match spelled(domain) {
                Ok(Spelled {
                    text,
                    address: None,
                }) if text.contains('.') => Ok(Self::Beneath(text)),
                _ => Err(format!(
                    "`{domain}` isn't a domain with subdomains of its own: write a name of two \
                     or more parts, as in `any subdomain of example.com`"
                )),
            };
        }
        if text.starts_with("the computer its ") {
            return crate::idea::argument_named(text)
                .map(Self::Argument)
                .ok_or_else(|| {
                    "name the argument by its place, from first to ninth: `on the computer its \
                     first argument names`"
                        .to_owned()
                });
        }
        match spelled(text) {
            Ok(spelled) => Ok(Self::Named(spelled.text)),
            Err(Unspelled::ThisComputer) => Err(format!(
                "`{text}` is this computer, which a program reaches through a connection lengua \
                 makes for it before it starts: write `- Network: connects to TCP port N`"
            )),
            Err(Unspelled::NotAName | Unspelled::TooLong) => Err(format!(
                "`{text}` isn't a computer's name or address: write a name as DNS does, such as \
                 `example.com`, an address such as `192.0.2.7`, `any subdomain of example.com`, \
                 `the computer its first argument names` or `any computer`"
            )),
        }
    }
}

/// One grant: the computers it names, and their ports.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Rule {
    /// The computers.
    pub computers: Computers,
    /// The ports; none for a rule about names, which have no port.
    pub ports: Vec<u16>,
    /// Whether the connections are over TLS, which lengua does.
    pub tls: bool,
    /// Whether an address in a private network may be reached: `, private addresses too`.
    pub private: bool,
}

impl Rule {
    /// Whether the rule covers `computer` on `port`. `args` are the program's arguments, its
    /// name first, for a rule about the computer an argument names; with none, as before a run,
    /// such a rule covers every computer.
    #[must_use]
    pub fn covers(&self, computer: &Spelled, port: u16, args: Option<&[String]>) -> bool {
        if !self.ports.is_empty() && !self.ports.contains(&port) {
            return false;
        }
        match &self.computers {
            Computers::Any => true,
            Computers::Named(name) => *name == computer.text,
            // A name beneath the domain: one label or more, a dot, then the domain.
            Computers::Beneath(domain) => {
                computer.address.is_none()
                    && computer
                        .text
                        .strip_suffix(domain.as_str())
                        .is_some_and(|rest| rest.len() > 1 && rest.ends_with('.'))
            }
            Computers::Argument(index) => args.is_none_or(|args| {
                args.get(*index)
                    .and_then(|given| spelled(given).ok())
                    .is_some_and(|given| given == *computer)
            }),
        }
    }

    /// Whether the rule lets `computer` be reached at an address in a private network: when it
    /// says so, or when the idea itself writes that address.
    #[must_use]
    pub fn reaches_private(&self, computer: &Spelled) -> bool {
        self.private
            || (computer.address.is_some() && matches!(self.computers, Computers::Named(_)))
    }

    /// The rule's ports and computers in words: `port 443 on any subdomain of example.com over
    /// TLS`.
    #[must_use]
    pub fn describe(&self) -> String {
        let numbers: Vec<String> = self.ports.iter().map(u16::to_string).collect();
        let ports = match numbers.split_last() {
            Some((last, [])) => format!("port {last} on "),
            Some((last, rest)) => format!("ports {} and {last} on ", rest.join(", ")),
            None => String::new(),
        };
        let tls = if self.tls { " over TLS" } else { "" };
        let private = if self.private {
            ", private addresses too"
        } else {
            ""
        };
        format!("{ports}{}{tls}{private}", self.computers.describe())
    }
}

/// What an idea's `- Network:` lines grant a program at run time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grants {
    /// The TCP connections it may open: `opens TCP connections to port 443 on example.com`.
    pub opens: Vec<Rule>,
    /// The names whose addresses it may look up: `looks up the addresses of example.com`.
    pub names: Vec<Rule>,
    /// The UDP datagrams it may send, and receive the answers to: `sends UDP datagrams to port
    /// 53 on 192.0.2.53`.
    pub datagrams: Vec<Rule>,
    /// The UDP port it receives datagrams on, which lengua binds, and whether other computers
    /// may send to it: `receives UDP datagrams on port 5353`, with `for any computer`.
    pub receives: Option<(u16, bool)>,
}

/// How an idea writes its run-time grants, for a line that doesn't read as one.
const USAGE: &str = "write `- Network: opens TCP connections to port 443 on example.com over TLS`, \
                     `- Network: looks up the addresses of example.com`, `- Network: sends UDP \
                     datagrams to port 53 on 192.0.2.53` or `- Network: receives UDP datagrams \
                     on port 5353`; the computers are a name, an address, `any subdomain of \
                     example.com`, `the computer its first argument names` or `any computer`";

/// The ports `text` lists, with commas and `and` between them.
fn ports(text: &str) -> Result<Vec<u16>, String> {
    let found: Option<Vec<u16>> = text
        .split(',')
        .flat_map(|part| part.split(" and "))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|port| port.parse::<u16>().ok().filter(|port| *port > 0))
        .collect();
    match found {
        Some(found) if !found.is_empty() => Ok(found),
        _ => Err(format!("ports go from 1 to 65535; {USAGE}")),
    }
}

/// What follows `words` in `line`, and a plural's `s` after them.
fn after<'a>(line: &'a str, words: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(words)?;
    Some(rest.strip_prefix('s').unwrap_or(rest))
}

/// Adds `rule` to `rules`, once.
fn add(rules: &mut Vec<Rule>, rule: Rule) -> Result<(), String> {
    if rules.contains(&rule) {
        return Ok(());
    }
    if rules.len() >= MAX_RULES {
        return Err(format!(
            "an idea gives at most {MAX_RULES} such grants; name the computers by a domain, as in \
             `any subdomain of example.com`"
        ));
    }
    rules.push(rule);
    Ok(())
}

impl Grants {
    /// No grants: what a program has without such a line.
    pub const NONE: Self = Self {
        opens: Vec::new(),
        names: Vec::new(),
        datagrams: Vec::new(),
        receives: None,
    };

    /// Whether the idea grants nothing at run time.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.opens.is_empty()
            && self.names.is_empty()
            && self.datagrams.is_empty()
            && self.receives.is_none()
    }

    /// Reads a `- Network:` line's words, in lower case, as a run-time grant, and adds it:
    /// `None` when the line isn't one of these.
    ///
    /// # Errors
    ///
    /// What's wrong with the line, and what to write instead.
    pub fn line(&mut self, lower: &str) -> Option<Result<(), String>> {
        let lower = lower.trim();
        let (line, private) = match lower.strip_suffix("private addresses too") {
            Some(rest) => (rest.trim_end().trim_end_matches(',').trim_end(), true),
            None => (lower, false),
        };
        if let Some(rest) = after(line, "opens tcp connections to port") {
            let (rest, tls) = match rest.strip_suffix("over tls") {
                Some(rest) => (rest.trim_end().trim_end_matches(',').trim_end(), true),
                None => (rest, false),
            };
            return Some(placed(rest, tls, private).and_then(|rule| add(&mut self.opens, rule)));
        }
        if let Some(rest) = after(line, "sends udp datagrams to port") {
            return Some(
                placed(rest, false, private).and_then(|rule| add(&mut self.datagrams, rule)),
            );
        }
        if let Some(rest) = line.strip_prefix("looks up the addresses of ") {
            return Some(Computers::read(rest).and_then(|computers| {
                add(
                    &mut self.names,
                    Rule {
                        computers,
                        ports: Vec::new(),
                        tls: false,
                        private,
                    },
                )
            }));
        }
        let rest = line.strip_prefix("receives udp datagrams on port")?;
        Some(self.receives_on(rest, private))
    }

    /// `receives UDP datagrams on port N`, with `for any computer`: the port lengua binds.
    fn receives_on(&mut self, rest: &str, private: bool) -> Result<(), String> {
        if private {
            return Err(format!(
                "`private addresses too` is for the computers a program reaches; {USAGE}"
            ));
        }
        let rest = rest.trim();
        let (number, anywhere) = match rest
            .strip_suffix("for any computer")
            .or_else(|| rest.strip_suffix("on any computer"))
        {
            Some(number) => (number.trim(), true),
            None => (
                rest.strip_suffix("for this computer")
                    .or_else(|| rest.strip_suffix("on this computer"))
                    .unwrap_or(rest)
                    .trim(),
                false,
            ),
        };
        let found = ports(number)?;
        match (found.as_slice(), self.receives) {
            ([port], None) => {
                self.receives = Some((*port, anywhere));
                Ok(())
            }
            ([_], Some(_)) => {
                Err("the port it receives datagrams on is given twice; keep one line".to_owned())
            }
            _ => Err(
                "a program receives datagrams on one port; keep one in `- Network: \
                      receives UDP datagrams on port N`"
                    .to_owned(),
            ),
        }
    }

    /// Each grant in words, for the user who is about to run the program.
    #[must_use]
    pub fn describe(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for rule in &self.opens {
            lines.push(format!(
                "it may open TCP connections to {}",
                rule.describe()
            ));
        }
        for rule in &self.names {
            lines.push(format!(
                "it may look up the addresses of {}",
                rule.describe()
            ));
        }
        for rule in &self.datagrams {
            lines.push(format!(
                "it may send UDP datagrams to {}, and receive the answers",
                rule.describe()
            ));
        }
        match self.receives {
            Some((port, true)) => lines.push(format!(
                "it receives UDP datagrams on port {port} of every address of this computer, so \
                 other computers can send to it, and answers the senders"
            )),
            Some((port, false)) => lines.push(format!(
                "it receives UDP datagrams on port {port} of this computer, and answers the \
                 senders"
            )),
            None => {}
        }
        lines
    }

    /// The first of `rules` that covers `computer` on `port`, over TLS or not as `tls` says
    /// when it says.
    #[must_use]
    pub fn granted<'a>(
        rules: &'a [Rule],
        computer: &Spelled,
        port: u16,
        tls: Option<bool>,
        args: Option<&[String]>,
    ) -> Option<&'a Rule> {
        rules
            .iter()
            .find(|rule| tls.is_none_or(|tls| rule.tls == tls) && rule.covers(computer, port, args))
    }
}

/// The rule `rest` writes: ports, ` on `, and the computers.
fn placed(rest: &str, tls: bool, private: bool) -> Result<Rule, String> {
    let Some((numbers, computers)) = rest.split_once(" on ") else {
        return Err(format!("name the computers after the ports; {USAGE}"));
    };
    Ok(Rule {
        computers: Computers::read(computers)?,
        ports: ports(numbers)?,
        tls,
        private,
    })
}

/// A computer an idea's test stands in for: one the program opens a connection to, or sends
/// datagrams to, at run time. A test's own server answers there, never the real computer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Stand {
    /// Whether it takes UDP datagrams; TCP connections otherwise.
    pub datagrams: bool,
    /// The computer, as lengua spells it.
    pub host: String,
    /// Its port.
    pub port: u16,
}

/// A computer as lengua spells it: a name in lower case with no dot at its end, or an address
/// in its usual writing, an IPv4 address inside an IPv6 one as the IPv4 address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spelled {
    /// The name, or the address in its usual writing.
    pub text: String,
    /// The address, when it is one.
    pub address: Option<IpAddr>,
}

/// Why some text isn't another computer's name or address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unspelled {
    /// It is this computer: `localhost`, the loopback, or the unspecified address.
    ThisComputer,
    /// It is no name DNS has and no address: a number an older parsing reads as an address
    /// (`2130706433`, `127.1`, `0x7f.1`) among them.
    NotAName,
    /// It is longer than a name may be.
    TooLong,
}

/// Reads the computer `text` names.
///
/// # Errors
///
/// Why it names no other computer.
pub fn spelled(text: &str) -> Result<Spelled, Unspelled> {
    if text.len() > MAX_NAME + 1 {
        return Err(Unspelled::TooLong);
    }
    let lower = text.to_ascii_lowercase();
    if let Ok(address) = lower.parse::<IpAddr>() {
        let address = match address {
            IpAddr::V6(six) => six.to_ipv4_mapped().map_or(address, IpAddr::V4),
            IpAddr::V4(_) => address,
        };
        if this_computer(address) {
            return Err(Unspelled::ThisComputer);
        }
        return Ok(Spelled {
            text: address.to_string(),
            address: Some(address),
        });
    }
    let name = lower.strip_suffix('.').unwrap_or(&lower);
    match host_kind(name) {
        HostKind::Other => Ok(Spelled {
            text: name.to_owned(),
            address: None,
        }),
        HostKind::ThisComputer => Err(Unspelled::ThisComputer),
        HostKind::NotAName => Err(Unspelled::NotAName),
    }
}

/// What kind of place an address is, for what a program reaches at run time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// This computer: its loopback, or the unspecified address.
    ThisComputer,
    /// An address of a private network: one a home or an office uses inside (10.0.0.0/8,
    /// 172.16.0.0/12, 192.168.0.0/16, fc00::/7), a link's own (169.254.0.0/16, where a cloud
    /// keeps its instances' credentials, and fe80::/10), a carrier's shared ones
    /// (100.64.0.0/10), or "this network" (0.0.0.0/8).
    Private,
    /// An address many computers answer at, or none does: multicast, broadcast, and the IPv4
    /// addresses kept for later (240.0.0.0/4).
    Many,
    /// Any other: a computer of the internet.
    Public,
}

/// The IPv4 address an IPv6 address carries inside it, when it carries one: written as IPv4
/// inside IPv6 (`::ffff:a.b.c.d`), IPv4-compatible (`::a.b.c.d`), NAT64's (`64:ff9b::/96`) or
/// 6to4's (`2002::/16`). A gateway or the system itself sends such an address on to the IPv4
/// one.
fn inner_four(six: Ipv6Addr) -> Option<Ipv4Addr> {
    if let Some(four) = six.to_ipv4_mapped() {
        return Some(four);
    }
    let octets = six.octets();
    let last = Ipv4Addr::new(octets[12], octets[13], octets[14], octets[15]);
    if octets[..12] == [0, 0x64, 0xff, 0x9b, 0, 0, 0, 0, 0, 0, 0, 0] || octets[..12] == [0; 12] {
        return Some(last);
    }
    if octets[0] == 0x20 && octets[1] == 0x02 {
        return Some(Ipv4Addr::new(octets[2], octets[3], octets[4], octets[5]));
    }
    None
}

/// The class of an IPv4 address that isn't this computer's.
fn class_four(four: Ipv4Addr) -> Class {
    let [first, second, ..] = four.octets();
    if four.is_multicast() || four.is_broadcast() || first >= 240 {
        Class::Many
    } else if four.is_private()
        || four.is_link_local()
        || first == 0
        || (first == 100 && second & 0xc0 == 64)
    {
        Class::Private
    } else {
        Class::Public
    }
}

/// What kind of place `address` is.
#[must_use]
pub fn class(address: IpAddr) -> Class {
    if this_computer(address) {
        return Class::ThisComputer;
    }
    match address {
        IpAddr::V4(four) => class_four(four),
        IpAddr::V6(six) => {
            if let Some(four) = inner_four(six) {
                return class(IpAddr::V4(four));
            }
            let first = six.segments()[0];
            if first & 0xff00 == 0xff00 {
                Class::Many
            } else if first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80 {
                Class::Private
            } else {
                Class::Public
            }
        }
    }
}

/// Why lengua doesn't open what a program asks for at run time, in words for the run's report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The program named no computer, or no port.
    NotAPlace(String),
    /// The name is longer than a name may be.
    TooLong(String),
    /// The idea doesn't grant it, or lengua's rules for addresses refuse it.
    NotGranted(String),
    /// The name has no address.
    NoAddress(String),
    /// The name has an address lengua's rules refuse. The program hears what it hears of a name
    /// with no address, so it can't learn which names of a private network exist; the run's
    /// report has the reason.
    Unreached(String),
    /// The place didn't take the connection.
    NotMade(String),
    /// The place didn't answer in time.
    NoAnswer(String),
    /// The program holds as many as it may at once.
    Full(String),
}

impl Refusal {
    /// The reason in words.
    #[must_use]
    pub fn words(&self) -> &str {
        match self {
            Self::NotAPlace(words)
            | Self::TooLong(words)
            | Self::NotGranted(words)
            | Self::NoAddress(words)
            | Self::Unreached(words)
            | Self::NotMade(words)
            | Self::NoAnswer(words)
            | Self::Full(words) => words,
        }
    }
}

impl Refusal {
    /// The refusal of a name, where this is the refusal of its addresses: what lengua's rules
    /// refuse of a name's addresses is the name [`Refusal::Unreached`].
    #[must_use]
    pub fn of_a_name(self) -> Self {
        match self {
            Self::NotGranted(why) => Self::Unreached(why),
            other => other,
        }
    }
}

/// The addresses of `found`, which `place` resolved to, that lengua may reach: all of them, or
/// none. One address that is this computer's, or one many computers answer at, refuses the
/// place, as does one in a private network unless `private`: whoever answers for a name
/// chooses its addresses, so a name with one address lengua refuses is refused whole.
///
/// # Errors
///
/// Why the place isn't reached.
pub fn vetted(place: &str, found: &[IpAddr], private: bool) -> Result<Vec<IpAddr>, Refusal> {
    let mut kept: Vec<IpAddr> = Vec::new();
    for address in found {
        match class(*address) {
            Class::ThisComputer => {
                return Err(Refusal::NotGranted(format!(
                    "{place} is this computer ({address}), which a program never reaches by a \
                     name or an address: an idea reaches this computer by a port alone"
                )));
            }
            Class::Many => {
                return Err(Refusal::NotGranted(format!(
                    "{place} has the address {address}, which is no one computer's"
                )));
            }
            Class::Private if !private => {
                return Err(Refusal::NotGranted(format!(
                    "{place} has the address {address}, in a private network, which the idea's \
                     grant doesn't reach: it says `, private addresses too` when it does"
                )));
            }
            Class::Private | Class::Public => {
                if !kept.contains(address) && kept.len() < MAX_ADDRESSES {
                    kept.push(*address);
                }
            }
        }
    }
    if kept.is_empty() {
        Err(Refusal::NoAddress(format!("{place} has no address")))
    } else {
        Ok(kept)
    }
}

/// How lengua finds a name's addresses: the system's resolver, or a test's own.
pub type Resolver = fn(&str, Duration) -> Result<Vec<IpAddr>, Refusal>;

/// The addresses the system's resolver has for `name`, waiting `within` at most.
///
/// # Errors
///
/// When the name has none, or the answer didn't come in time.
pub fn lookup(name: &str, within: Duration) -> Result<Vec<IpAddr>, Refusal> {
    let Some(runtime) = crate::wasm::host::net::runtime() else {
        return Err(Refusal::NotMade(
            "lengua couldn't start what makes its connections".to_owned(),
        ));
    };
    let found = runtime
        .block_on(async { tokio::time::timeout(within, tokio::net::lookup_host((name, 0))).await });
    match found {
        Err(_) => Err(Refusal::NoAnswer(format!(
            "no answer about {name}'s addresses within {} milliseconds",
            within.as_millis()
        ))),
        Ok(Err(error)) => Err(Refusal::NoAddress(format!(
            "{name} isn't a computer lengua can find: {error}"
        ))),
        Ok(Ok(addresses)) => Ok(addresses.map(|address| address.ip()).collect()),
    }
}

/// A TCP connection to `port` of one of `addresses`, tried in turn within `within` in all:
/// these addresses and no others, so nothing is resolved again here.
///
/// # Errors
///
/// When none took the connection, or none answered in time.
pub fn dial(
    place: &str,
    addresses: &[IpAddr],
    port: u16,
    within: Duration,
) -> Result<TcpStream, Refusal> {
    let started = Instant::now();
    let mut last = None;
    for address in addresses {
        let left = within.saturating_sub(started.elapsed());
        if left.is_zero() {
            break;
        }
        match TcpStream::connect_timeout(&SocketAddr::new(*address, port), left) {
            Ok(stream) => return Ok(stream),
            Err(error) => last = Some(format!("{address}: {error}")),
        }
    }
    if started.elapsed() >= within {
        Err(Refusal::NoAnswer(format!(
            "{place} didn't answer within {} milliseconds",
            within.as_millis()
        )))
    } else {
        Err(Refusal::NotMade(format!(
            "{place} didn't take the connection ({})",
            last.unwrap_or_else(|| "it has no address".to_owned())
        )))
    }
}

/// TLS on `stream`, which goes to `computer`, done by lengua within `within`: the computer's
/// certificate is checked for its name with `config`, and the program reads and writes plain
/// bytes.
///
/// # Errors
///
/// When the certificate can't be checked for the name, or the computer didn't answer in time.
pub(crate) fn secure(
    stream: TcpStream,
    computer: &Spelled,
    config: Arc<rustls::ClientConfig>,
    within: Duration,
) -> Result<(Box<dyn Place>, &'static tokio::runtime::Runtime), Refusal> {
    let host = &computer.text;
    let name = rustls::pki_types::ServerName::try_from(host.clone()).map_err(|_| {
        Refusal::NotMade(format!(
            "{host} isn't a name TLS can check a certificate for"
        ))
    })?;
    let Some(runtime) = crate::wasm::host::net::runtime() else {
        return Err(Refusal::NotMade(
            "lengua couldn't start what makes its connections".to_owned(),
        ));
    };
    let failed =
        |error: &dyn std::fmt::Display| Refusal::NotMade(format!("TLS with {host}: {error}"));
    stream
        .set_nonblocking(true)
        .map_err(|error| failed(&error))?;
    let made = runtime.block_on(async {
        let stream = tokio::net::TcpStream::from_std(stream)?;
        tokio::time::timeout(
            within,
            tokio_rustls::TlsConnector::from(config).connect(name, stream),
        )
        .await
        .unwrap_or_else(|_| Err(std::io::Error::from(std::io::ErrorKind::TimedOut)))
    });
    match made {
        Ok(stream) => {
            let place: Box<dyn Place> = Box::new(stream);
            Ok((place, runtime))
        }
        Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
            Err(Refusal::NoAnswer(format!(
                "TLS with {host} wasn't done within {} milliseconds",
                within.as_millis()
            )))
        }
        Err(error) => Err(failed(&error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grant's rule, as an idea's line gives it.
    fn opens(line: &str) -> Rule {
        let mut grants = Grants::default();
        let read = grants.line(&line.to_ascii_lowercase());
        assert_eq!(read, Some(Ok(())), "{line}");
        grants.opens.remove(0)
    }

    fn computer(text: &str) -> Spelled {
        spelled(text).unwrap_or_else(|why| panic!("{text}: {why:?}"))
    }

    fn address(text: &str) -> IpAddr {
        text.parse()
            .unwrap_or_else(|_| panic!("{text} is an address"))
    }

    #[test]
    fn the_lines_read_as_grants() {
        let mut grants = Grants::default();
        for line in [
            "opens tcp connections to port 443 on example.com over tls",
            "opens tcp connections to ports 80 and 8080 on any subdomain of example.org",
            "opens tcp connections to port 443 on the computer its second argument names over tls",
            "opens tcp connections to port 80 on any computer, private addresses too",
            "looks up the addresses of any subdomain of example.org",
            "sends udp datagrams to port 53 on 192.0.2.53",
            "receives udp datagrams on port 5353 for any computer",
        ] {
            assert_eq!(grants.line(line), Some(Ok(())), "{line}");
        }
        assert_eq!(grants.opens.len(), 4);
        assert_eq!(
            grants.opens[0],
            Rule {
                computers: Computers::Named("example.com".to_owned()),
                ports: vec![443],
                tls: true,
                private: false,
            }
        );
        assert_eq!(
            grants.opens[1].computers,
            Computers::Beneath("example.org".to_owned())
        );
        assert_eq!(grants.opens[1].ports, [80, 8080]);
        assert_eq!(grants.opens[2].computers, Computers::Argument(2));
        assert!(grants.opens[3].private && grants.opens[3].computers == Computers::Any);
        assert_eq!(grants.names.len(), 1);
        assert_eq!(
            grants.datagrams[0].computers,
            Computers::Named("192.0.2.53".to_owned())
        );
        assert_eq!(grants.receives, Some((5353, true)));
        assert_eq!(grants.line("connects to tcp port 80"), None);
        assert_eq!(grants.line("listens on tcp port 80"), None);
        // In words, for the user.
        let shown = grants.describe();
        assert_eq!(shown.len(), 7);
        assert_eq!(
            shown[1],
            "it may open TCP connections to ports 80 and 8080 on any subdomain of example.org"
        );
        assert_eq!(
            shown[3],
            "it may open TCP connections to port 80 on any computer, private addresses too"
        );
        assert!(
            shown[6].contains("every address of this computer"),
            "{shown:?}"
        );
    }

    #[test]
    fn a_line_that_names_no_computers_or_this_one_is_refused() {
        let problem = |line: &str| -> String {
            match Grants::default().line(line) {
                Some(Err(problem)) => problem,
                other => panic!("{line}: {other:?}"),
            }
        };
        assert!(problem("opens tcp connections to port 80").contains("name the computers"));
        assert!(problem("opens tcp connections to port 0 on example.com").contains("1 to 65535"));
        for this in [
            "localhost",
            "127.0.0.1",
            "::1",
            "::ffff:127.0.0.1",
            "this computer",
        ] {
            let line = format!("opens tcp connections to port 80 on {this}");
            let found = problem(&line);
            assert!(
                found.contains("is this computer") || found.contains("isn't a computer's name"),
                "{this}: {found}"
            );
        }
        assert!(
            problem("opens tcp connections to port 80 on any subdomain of com")
                .contains("isn't a domain")
        );
        assert!(
            problem("opens tcp connections to port 80 on any subdomain of 192.0.2.7")
                .contains("isn't a domain")
        );
        assert!(problem("opens tcp connections to port 80 on 2130706433").contains("isn't a"));
        let mut twice = Grants::default();
        assert_eq!(twice.line("receives udp datagrams on port 9"), Some(Ok(())));
        assert!(matches!(
            twice.line("receives udp datagrams on port 10"),
            Some(Err(problem)) if problem.contains("given twice")
        ));
    }

    /// The threat: a program follows a redirect, or anything else it read, to a computer its
    /// idea doesn't name. A rule covers only its own computers: a subdomain rule neither the
    /// domain itself, nor a name that only ends or starts alike, nor an address.
    #[test]
    fn a_rule_covers_its_computers_and_no_others() {
        let beneath =
            opens("opens TCP connections to ports 80 and 443 on any subdomain of example.org");
        for inside in ["www.example.org", "a.b.example.org", "WWW.Example.ORG."] {
            assert!(beneath.covers(&computer(inside), 80, None), "{inside}");
        }
        for outside in [
            "example.org",
            "badexample.org",
            "example.org.evil.test",
            "www.example.org.evil.test",
            "example.com",
            "192.0.2.7",
        ] {
            assert!(!beneath.covers(&computer(outside), 80, None), "{outside}");
        }
        assert!(!beneath.covers(&computer("www.example.org"), 8080, None));
        let named = opens("opens TCP connections to port 443 on Example.com over TLS");
        assert!(named.covers(&computer("example.com."), 443, None));
        assert!(!named.covers(&computer("www.example.com"), 443, None));
        // The computer an argument names: that one, as lengua spells it, and no other.
        let typed =
            opens("opens TCP connections to port 80 on the computer its first argument names");
        let args = ["main.wasm".to_owned(), "Example.NET".to_owned()];
        assert!(typed.covers(&computer("example.net"), 80, Some(&args)));
        assert!(!typed.covers(&computer("example.org"), 80, Some(&args)));
        assert!(!typed.covers(&computer("example.net"), 80, Some(&args[..1])));
        // Over TLS or not is the rule's: a program can't have a TLS grant's computer in plain.
        let grants = [named];
        let there = computer("example.com");
        assert!(Grants::granted(&grants, &there, 443, Some(true), None).is_some());
        assert!(Grants::granted(&grants, &there, 443, Some(false), None).is_none());
    }

    /// The threat: this computer under another spelling. A number only an older parsing reads
    /// as an address is no name; the loopback inside an IPv6 address is the loopback; and an
    /// IPv4 address inside an IPv6 one is spelled as the IPv4 address, so a rule about one
    /// covers the other.
    #[test]
    fn every_spelling_of_a_computer_is_one_spelling() {
        for nothing in [
            "2130706433",
            "127.1",
            "0x7f.1",
            "0177.0.0.1",
            "[::1]",
            "a b",
            "",
            "fe80::1%eth0",
        ] {
            assert_eq!(spelled(nothing), Err(Unspelled::NotAName), "{nothing}");
        }
        for this in [
            "localhost",
            "LOCALHOST.",
            "app.localhost",
            "127.0.0.1",
            "127.9.9.9",
            "::1",
            "0.0.0.0",
            "::",
            "::ffff:127.0.0.1",
            "::ffff:7f00:1",
        ] {
            assert_eq!(spelled(this), Err(Unspelled::ThisComputer), "{this}");
        }
        assert_eq!(computer("::ffff:192.0.2.7").text, "192.0.2.7");
        assert_eq!(computer("2001:DB8:0:0::1").text, "2001:db8::1");
        assert_eq!(computer("Example.COM.").text, "example.com");
        assert_eq!(spelled(&"a".repeat(300)), Err(Unspelled::TooLong));
        let named = opens("opens TCP connections to port 80 on 192.0.2.7");
        assert!(named.covers(&computer("::ffff:192.0.2.7"), 80, None));
    }

    /// The threat: the loopback and the private ranges, by any writing of them.
    #[test]
    fn each_address_has_its_class() {
        for own in [
            "127.0.0.1",
            "127.255.255.254",
            "0.0.0.0",
            "::1",
            "::",
            "::ffff:127.0.0.1",
        ] {
            assert_eq!(class(address(own)), Class::ThisComputer, "{own}");
        }
        // The loopback carried inside an IPv6 address a gateway or the system sends on.
        for carried in ["::127.0.0.1", "64:ff9b::7f00:1", "2002:7f00:1::"] {
            assert_eq!(class(address(carried)), Class::ThisComputer, "{carried}");
        }
        for private in [
            "10.0.0.1",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "100.127.255.255",
            "0.1.2.3",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "::ffff:10.0.0.1",
            "64:ff9b::a00:1",
            "2002:c0a8:101::1",
            "::10.0.0.1",
        ] {
            assert_eq!(class(address(private)), Class::Private, "{private}");
        }
        for many in [
            "224.0.0.1",
            "239.255.255.250",
            "255.255.255.255",
            "240.0.0.1",
            "ff02::1",
        ] {
            assert_eq!(class(address(many)), Class::Many, "{many}");
        }
        for public in [
            "192.0.2.7",
            "8.8.8.8",
            "172.32.0.1",
            "100.128.0.1",
            "2001:db8::1",
            "2606:4700::1111",
            "64:ff9b::c000:207",
        ] {
            assert_eq!(class(address(public)), Class::Public, "{public}");
        }
    }

    /// The threat: DNS tricks. Whoever answers for a name chooses its addresses, so a name any
    /// of whose addresses lengua refuses is refused whole, whatever comes first.
    #[test]
    fn a_name_is_reached_only_when_every_address_of_it_may_be() {
        let found = |texts: &[&str]| texts.iter().map(|text| address(text)).collect::<Vec<_>>();
        let public = found(&["192.0.2.7", "2001:db8::1", "192.0.2.7"]);
        assert_eq!(
            vetted("example.org", &public, false),
            Ok(found(&["192.0.2.7", "2001:db8::1"]))
        );
        for own in [
            "127.0.0.1",
            "::1",
            "0.0.0.0",
            "::ffff:127.0.0.1",
            "64:ff9b::7f00:1",
        ] {
            for mixed in [
                found(&[own]),
                found(&["192.0.2.7", own]),
                found(&[own, "192.0.2.7"]),
            ] {
                // Even a grant of private addresses never reaches this computer.
                let refused = vetted("evil.example", &mixed, true);
                assert!(
                    matches!(&refused, Err(Refusal::NotGranted(why)) if why.contains("this computer")),
                    "{own}: {refused:?}"
                );
            }
        }
        for private in [
            "10.0.0.1",
            "169.254.169.254",
            "192.168.0.1",
            "fd00::1",
            "::ffff:10.1.1.1",
        ] {
            let mixed = found(&["192.0.2.7", private]);
            let refused = vetted("evil.example", &mixed, false);
            assert!(
                matches!(&refused, Err(Refusal::NotGranted(why)) if why.contains("private network")),
                "{private}: {refused:?}"
            );
            assert_eq!(
                vetted("nas.example", &mixed, true),
                Ok(mixed.clone()),
                "{private}"
            );
        }
        // The threat: UDP used to reach many computers at once.
        for many in ["255.255.255.255", "224.0.0.251", "ff02::fb"] {
            let refused = vetted("all.example", &found(&[many]), true);
            assert!(matches!(refused, Err(Refusal::NotGranted(_))), "{many}");
        }
        assert!(matches!(
            vetted("nothing.example", &[], false),
            Err(Refusal::NoAddress(_))
        ));
        let many: Vec<IpAddr> = (1..=40_u8)
            .map(|last| IpAddr::V4(Ipv4Addr::new(192, 0, 2, last)))
            .collect();
        assert_eq!(
            vetted("big.example", &many, false).map(|kept| kept.len()),
            Ok(MAX_ADDRESSES)
        );
    }

    /// An address the idea itself writes is reached as written, in a private network too; a
    /// name's addresses are held to what the rule says.
    #[test]
    fn an_address_the_idea_writes_is_reached_as_written() {
        let written = opens("opens TCP connections to port 8080 on 192.168.1.20");
        assert!(written.reaches_private(&computer("192.168.1.20")));
        let named = opens("opens TCP connections to port 8080 on nas.example");
        assert!(!named.reaches_private(&computer("nas.example")));
        let any = opens("opens TCP connections to port 8080 on any computer");
        assert!(!any.reaches_private(&computer("192.168.1.20")));
        let told =
            opens("opens TCP connections to port 8080 on any computer, private addresses too");
        assert!(told.reaches_private(&computer("192.168.1.20")));
    }

    /// A place that takes no connection is said so, and nothing is tried past the time given.
    #[test]
    fn a_place_that_takes_no_connection_is_not_made() {
        let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
        let port = listener.local_addr().expect("its address").port();
        drop(listener);
        let nowhere = dial(
            "the test's place",
            &[IpAddr::V4(Ipv4Addr::LOCALHOST)],
            port,
            Duration::from_secs(5),
        );
        assert!(matches!(nowhere, Err(Refusal::NotMade(_))), "{nowhere:?}");
        let untried = dial(
            "the test's place",
            &[IpAddr::V4(Ipv4Addr::LOCALHOST)],
            port,
            Duration::ZERO,
        );
        assert!(matches!(untried, Err(Refusal::NoAnswer(_))), "{untried:?}");
    }
}
