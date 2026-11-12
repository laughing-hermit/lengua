//! What a program's idea lets its WebAssembly reach: the `- Uses:` capabilities and the
//! `- Network:` line, as the host checks them on every call.

use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::idea::{Network, Use};
use crate::reach::Grants;
use crate::wasm::wasi::{Allowance, Function};

/// What an idea lets a program reach on the network at run time ([`Grants`]), kept once for the
/// process, so that a policy stays a small value to copy: equal grants are kept once, and an
/// idea without such grants keeps nothing. It travels to a worker as the grants themselves.
#[derive(Debug, Clone, Copy)]
pub struct Reach(&'static Grants);

/// No grants.
static NONE: Grants = Grants::NONE;

/// The grants kept so far, each once.
static KEPT: Mutex<Vec<&'static Grants>> = Mutex::new(Vec::new());

impl Reach {
    /// `grants`, kept.
    #[must_use]
    pub fn of(grants: &Grants) -> Self {
        if grants.is_empty() {
            return Self::default();
        }
        let mut kept = KEPT.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(found) = kept.iter().copied().find(|found| **found == *grants) {
            return Self(found);
        }
        let grants: &'static Grants = Box::leak(Box::new(grants.clone()));
        kept.push(grants);
        Self(grants)
    }

    /// The grants.
    #[must_use]
    pub const fn get(self) -> &'static Grants {
        self.0
    }
}

impl Default for Reach {
    fn default() -> Self {
        Self(&NONE)
    }
}

impl PartialEq for Reach {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Reach {}

impl Serialize for Reach {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Reach {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Grants::deserialize(deserializer).map(|grants| Self::of(&grants))
    }
}

/// What a program may reach beyond its output and its exit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    /// Standard input.
    pub standard_input: bool,
    /// Files and folders in its working folder.
    pub files: bool,
    /// The clock.
    pub clock: bool,
    /// Random bytes.
    pub random: bool,
    /// The environment variables it is given.
    pub environment: bool,
    /// The terminal: keys as they're pressed.
    pub terminal: bool,
    /// How many connections lengua makes for it.
    pub connections: usize,
    /// Whether it serves, on a listener lengua gives it.
    pub listens: bool,
    /// Whether it serves several clients at once.
    pub at_once: bool,
    /// What it may reach at run time, which it opens itself.
    #[serde(default)]
    pub reach: Reach,
    /// Whether this run is a piece of a program's split work ([`crate::split`]): a run of the
    /// same module that reads its share of the bytes on standard input, prints its answer, and
    /// reaches nothing else.
    #[serde(default)]
    pub piece: bool,
}

impl Policy {
    /// What an idea's `uses` and `network` allow.
    #[must_use]
    pub fn of(uses: &[Use], network: &Network) -> Self {
        Self {
            standard_input: uses.contains(&Use::StandardInput),
            files: uses.contains(&Use::Files),
            clock: uses.contains(&Use::Clock),
            random: uses.contains(&Use::RandomBytes),
            environment: uses.contains(&Use::Environment),
            terminal: uses.contains(&Use::Terminal),
            connections: network.connections.len(),
            listens: network.listens.is_some(),
            at_once: network.at_once.is_some(),
            reach: Reach::of(&network.reach),
            piece: false,
        }
    }

    /// Whether the idea names `capability`.
    #[must_use]
    pub const fn uses(&self, capability: Use) -> bool {
        match capability {
            Use::StandardInput => self.standard_input,
            Use::Files => self.files,
            Use::Clock => self.clock,
            Use::RandomBytes => self.random,
            Use::Environment => self.environment,
            Use::Terminal => self.terminal,
        }
    }

    /// Whether the program is on the network at all: with connections lengua makes, a
    /// listener, or connections or datagram sockets of its own.
    #[must_use]
    pub fn network(&self) -> bool {
        let reach = self.reach.get();
        self.connections > 0
            || self.listens
            || !reach.opens.is_empty()
            || !reach.datagrams.is_empty()
            || reach.receives.is_some()
    }

    /// Whether the idea meets `allowance`.
    #[must_use]
    pub fn meets(&self, allowance: Allowance) -> bool {
        let reach = self.reach.get();
        match allowance {
            Allowance::Always => true,
            Allowance::Use(capability) => self.uses(capability),
            Allowance::Network => self.network(),
            Allowance::Listens => self.listens,
            Allowance::AtOnce => self.at_once,
            Allowance::Opens => !reach.opens.is_empty(),
            Allowance::Names => !reach.names.is_empty(),
            Allowance::Datagrams => !reach.datagrams.is_empty(),
            Allowance::Receives => reach.receives.is_some(),
            Allowance::Named => !reach.files.is_empty(),
            Allowance::NamedFolder => reach.files.iter().any(|grant| grant.folder),
            Allowance::Runs => !reach.runs.is_empty(),
            Allowance::Splits => reach.split.is_some(),
        }
    }

    /// Whether the idea allows `function`: it meets one of its allowances.
    #[must_use]
    pub fn allows(&self, function: &Function) -> bool {
        function
            .allowances
            .iter()
            .any(|allowance| self.meets(*allowance))
    }

    /// What a piece of this program's split work may reach: standard input, which holds its
    /// share of the bytes, its output, and nothing else of what the idea grants the program.
    /// It keeps the grant of the split itself, by which it asks whether it is a piece and
    /// which bounds what it prints, and it can't split in its turn.
    #[must_use]
    pub fn for_a_piece(self) -> Self {
        let split = Grants {
            split: self.reach.get().split,
            ..Grants::NONE
        };
        Self {
            standard_input: true,
            reach: Reach::of(&split),
            piece: true,
            ..Self::default()
        }
    }

    /// Whether the program may hold a file or a folder open: it names files, or its idea lets
    /// it open what its arguments name.
    #[must_use]
    pub fn holds_files(&self) -> bool {
        self.files || !self.reach.get().files.is_empty()
    }

    /// Whether the program may read standard input: it names it, or the terminal.
    #[must_use]
    pub const fn reads_input(&self) -> bool {
        self.standard_input || self.terminal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::wasi;

    fn allowed(policy: &Policy) -> Vec<&'static str> {
        wasi::FUNCTIONS
            .iter()
            .filter(|function| function.given && policy.allows(function))
            .map(|function| function.name)
            .collect()
    }

    /// Nothing named: output, exit, the arguments and the descriptors' kinds, and what a
    /// toolchain's startup calls, which answers harmlessly.
    #[test]
    fn a_program_that_names_nothing_reaches_its_output_and_exit() {
        let none = allowed(&Policy::default());
        assert_eq!(
            none,
            [
                "args_get",
                "args_sizes_get",
                "fd_fdstat_get",
                "fd_write",
                "proc_exit",
                "sched_yield"
            ]
        );
    }

    /// Each capability allows its functions, as the idea's words read.
    #[test]
    fn each_capability_allows_its_functions() {
        let (uses, network) =
            crate::idea::constraints(&["files", "the clock"], "").expect("valid words");
        let policy = Policy::of(&uses, &network);
        assert!(policy.files && policy.clock && !policy.random);
        let reached = allowed(&policy);
        for name in [
            "path_open",
            "fd_readdir",
            "clock_time_get",
            "poll_oneoff",
            "fd_close",
        ] {
            assert!(reached.contains(&name), "{name}");
        }
        assert!(!reached.contains(&"random_get"));
        assert!(!reached.contains(&"sock_accept"));
        let (uses, network) =
            crate::idea::constraints(&[], "listens on TCP port 8080").expect("a server's line");
        let server = Policy::of(&uses, &network);
        assert!(server.listens && server.network() && !server.at_once);
        let reached = allowed(&server);
        for name in [
            "sock_accept",
            "sock_recv",
            "sock_send",
            "sock_shutdown",
            "fd_read",
        ] {
            assert!(reached.contains(&name), "{name}");
        }
        assert!(crate::idea::constraints(&["the moon"], "").is_err());
    }
}
