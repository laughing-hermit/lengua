//! What a program's idea lets its WebAssembly reach: the `- Uses:` capabilities and the
//! `- Network:` line, as the host checks them on every call.

use serde::{Deserialize, Serialize};

use crate::edge::Allowance;
use crate::idea::{Network, Use};
use crate::wasm::wasi::Function;

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

    /// Whether the program is on the network at all.
    #[must_use]
    pub const fn network(&self) -> bool {
        self.connections > 0 || self.listens
    }

    /// Whether the idea meets `allowance`.
    #[must_use]
    pub const fn meets(&self, allowance: Allowance) -> bool {
        match allowance {
            Allowance::Always => true,
            Allowance::Use(capability) => self.uses(capability),
            Allowance::Network => self.network(),
            Allowance::Listens => self.listens,
            Allowance::AtOnce => self.at_once,
            // A WebAssembly program has no threads.
            Allowance::Threads => false,
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
        let (uses, network) = crate::idea::constraints(&["files", "the clock"], "")
            .expect("valid words");
        let policy = Policy::of(&uses, &network);
        assert!(policy.files && policy.clock && !policy.random);
        let reached = allowed(&policy);
        for name in ["path_open", "fd_readdir", "clock_time_get", "poll_oneoff", "fd_close"] {
            assert!(reached.contains(&name), "{name}");
        }
        assert!(!reached.contains(&"random_get"));
        assert!(!reached.contains(&"sock_accept"));
        let (uses, network) = crate::idea::constraints(&[], "listens on TCP port 8080")
            .expect("a server's line");
        let server = Policy::of(&uses, &network);
        assert!(server.listens && server.network() && !server.at_once);
        let reached = allowed(&server);
        for name in ["sock_accept", "sock_recv", "sock_send", "sock_shutdown", "fd_read"] {
            assert!(reached.contains(&name), "{name}");
        }
        assert!(crate::idea::constraints(&["the moon"], "").is_err());
    }
}
