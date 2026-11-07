//! The rule checker: the lengua Power of 10 and the memory rules M1 to M8 for main.wat, in the
//! WebAssembly dialect 1 ([`wasm`]). The rules a report names and their violations are here.

pub mod wasm;

#[cfg(test)]
pub(crate) mod golden;

use std::fmt;

/// At most this many violations are reported at once.
pub const MAX_VIOLATIONS: usize = 20;

/// The rule a violation breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rule {
    /// The dialect's layout: what main.wat must be to be read at all.
    Dialect,
    /// Rule 1: simple control flow.
    ControlFlow,
    /// Rule 2: bounded loops.
    Loops,
    /// Rule 3: fixed resources.
    Resources,
    /// Rule 4: small functions.
    Functions,
    /// Rule 5: assertions.
    Assertions,
    /// Rule 6: smallest scope.
    Scope,
    /// Rule 7: checked results.
    Results,
    /// Rule 8: no macros, named constants.
    Constants,
    /// Rule 9: no indirection, checked indexes.
    Indirection,
    /// Rule 10: checked by tools.
    Tools,
    /// M1: every address comes from a pointer, never a plain number.
    Owners,
    /// M2: every access stays inside its owner.
    Bounds,
    /// M3: a frame's addresses never outlive it, and its accesses stay inside it.
    Lifetimes,
    /// M4: a function writes only what it owns or was lent, never through overlapping
    /// arguments.
    ExclusiveWrites,
    /// M5: no frame byte is read before it's written.
    Initialization,
    /// M6: read-only memory stays read-only, and no memory is both writable and executable.
    Immutability,
    /// M7: threads never write what another thread reads or writes.
    Races,
    /// M8: the stack stays within its limit.
    StackSize,
    /// The secret rule: no value computed from an input the idea's `- Secret:` line names
    /// decides a branch, an address, a length, a loop's count, a division or a float operation,
    /// or reaches a host call but as the bytes it writes out.
    Secrets,
}

impl Rule {
    /// How reports name the rule: `rule 4`, `rule M3`, `dialect` or `secrets`.
    #[must_use]
    pub fn label(self) -> String {
        match (self.number(), self.memory_number()) {
            (Some(number), _) => format!("rule {number}"),
            (None, Some(number)) => format!("rule M{number}"),
            (None, None) => self.name().to_owned(),
        }
    }

    /// The memory rule's number, M1 to M8.
    #[must_use]
    pub fn memory_number(self) -> Option<u8> {
        match self {
            Self::Owners => Some(1),
            Self::Bounds => Some(2),
            Self::Lifetimes => Some(3),
            Self::ExclusiveWrites => Some(4),
            Self::Initialization => Some(5),
            Self::Immutability => Some(6),
            Self::Races => Some(7),
            Self::StackSize => Some(8),
            _ => None,
        }
    }

    /// The Power of 10 rule's number, or `None` for the dialect's layout and the memory rules.
    #[must_use]
    pub fn number(self) -> Option<u8> {
        match self {
            Self::Dialect => None,
            Self::ControlFlow => Some(1),
            Self::Loops => Some(2),
            Self::Resources => Some(3),
            Self::Functions => Some(4),
            Self::Assertions => Some(5),
            Self::Scope => Some(6),
            Self::Results => Some(7),
            Self::Constants => Some(8),
            Self::Indirection => Some(9),
            Self::Tools => Some(10),
            _ => None,
        }
    }

    /// The rule's short name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Dialect => "dialect",
            Self::ControlFlow => "simple control flow",
            Self::Loops => "bounded loops",
            Self::Resources => "fixed resources",
            Self::Functions => "small functions",
            Self::Assertions => "assertions",
            Self::Scope => "smallest scope",
            Self::Results => "checked results",
            Self::Constants => "named constants",
            Self::Indirection => "no indirection",
            Self::Tools => "checked by tools",
            Self::Owners => "owners",
            Self::Bounds => "bounds",
            Self::Lifetimes => "lifetimes",
            Self::ExclusiveWrites => "exclusive writes",
            Self::Initialization => "initialization",
            Self::Immutability => "immutability",
            Self::Races => "no races",
            Self::StackSize => "stack size",
            Self::Secrets => "secrets",
        }
    }
}

/// A broken rule, with where and how to fix it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The rule.
    pub rule: Rule,
    /// The main.wat line, when there is one.
    pub line: Option<usize>,
    /// What's wrong.
    pub problem: String,
    /// How to fix it.
    pub fix: String,
}

impl Violation {
    /// How a report shows the violation of a line of `file`: `rule 4 (small functions)
    /// main.wat:12: problem. Fix: fix.`, and `dialect` or `secrets` for a rule with no number.
    #[must_use]
    pub fn in_file(&self, file: &str) -> String {
        let rule = match self.rule {
            rule @ (Rule::Dialect | Rule::Secrets) => rule.label(),
            rule => format!("{} ({})", rule.label(), rule.name()),
        };
        let place = self
            .line
            .map_or_else(|| ": ".to_owned(), |line| format!(" {file}:{line}: "));
        format!("{rule}{place}{}. Fix: {}.", self.problem, self.fix)
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.in_file(crate::wasm::CODE_FILE))
    }
}

/// An `;; unsafe:` … `;; end unsafe` block of main.wat, where bounds checks may be left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsafeBlock {
    /// Line of `;; unsafe:`.
    pub start: usize,
    /// Line of `;; end unsafe`.
    pub end: usize,
    /// Why it's safe.
    pub reason: String,
}
