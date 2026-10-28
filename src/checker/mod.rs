//! The rule checker for dialect 2: the lengua Power of 10 for x86-64 assembly, integer and
//! integer vector instructions.
//!
//! It reads main.s ([`source`]), reads the linked executable back and matches every instruction
//! to its line ([`decode`]), builds each function's control flow and stack depths ([`cfg`]), and
//! checks the rules ([`rules`]).

pub mod cfg;
pub mod decode;
pub mod rules;
pub mod source;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fmt;

use crate::platform::{CpuFeatures, Profile};
use crate::spec::Syscall;

pub use rules::Facts;
pub use source::{Source, UnsafeBlock};

/// The template program; its assert_fail and assert_message are the canonical ones.
pub const TEMPLATE: &str =
    include_str!("../../plugin/skills/write-assembly/assets/main-template.s");

/// At most this many violations are reported at once.
pub const MAX_VIOLATIONS: usize = 20;

/// The rule a violation breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rule {
    /// The dialect's layout and the match between main.s and the machine code.
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
}

impl Rule {
    /// The rule's number, or `None` for the dialect's layout.
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
        }
    }
}

/// A broken rule, with where and how to fix it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The rule.
    pub rule: Rule,
    /// The main.s line, when there is one.
    pub line: Option<usize>,
    /// What's wrong.
    pub problem: String,
    /// How to fix it.
    pub fix: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.rule.number() {
            Some(number) => write!(f, "rule {number} ({})", self.rule.name())?,
            None => f.write_str("dialect")?,
        }
        match self.line {
            Some(line) => write!(f, " main.s:{line}: ")?,
            None => f.write_str(": ")?,
        }
        write!(f, "{}. Fix: {}.", self.problem, self.fix)
    }
}

/// What the checker needs besides main.s and the executable.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    /// The system calls the spec allows.
    pub syscalls: &'a [Syscall],
    /// This CPU's extensions.
    pub features: CpuFeatures,
    /// The platform.
    pub profile: Profile,
}

/// What checking found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Violations, sorted by line, at most [`MAX_VIOLATIONS`].
    pub violations: Vec<Violation>,
    /// Violations beyond those shown.
    pub hidden: usize,
    /// What the rules learned, when the program got that far.
    pub facts: Option<Facts>,
    /// The `# unsafe:` blocks, all of which are reported.
    pub unsafe_blocks: Vec<UnsafeBlock>,
}

impl Report {
    fn new(mut violations: Vec<Violation>, facts: Option<Facts>, source: &Source) -> Self {
        violations.sort_by_key(|violation| (violation.line.unwrap_or(0), violation.rule));
        violations.dedup();
        let hidden = violations.len().saturating_sub(MAX_VIOLATIONS);
        violations.truncate(MAX_VIOLATIONS);
        Self {
            violations,
            hidden,
            facts,
            unsafe_blocks: source
                .functions
                .iter()
                .flat_map(|function| function.unsafe_blocks.iter().cloned())
                .collect(),
        }
    }

    /// Whether every rule passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Checks main.s's layout before anything is built.
#[must_use]
pub fn check_layout(text: &str) -> (Source, Report) {
    let (source, violations) = source::parse(text);
    let report = Report::new(violations, None, &source);
    (source, report)
}

/// Checks the executable built from main.s: that it matches main.s instruction by instruction,
/// then every rule.
#[must_use]
pub fn check_program(source: &Source, executable: &[u8], context: Context<'_>) -> Report {
    let mut violations = Vec::new();
    let machine = match decode::read(executable) {
        Ok(machine) => machine,
        Err(problem) => {
            violations.push(Violation {
                rule: Rule::Dialect,
                line: None,
                problem,
                fix: "keep every instruction inside a function and every byte of data inside a \
                      data symbol"
                    .into(),
            });
            return Report::new(violations, None, source);
        }
    };
    let Some(program) = decode::map(source, &machine, context.features, &mut violations) else {
        return Report::new(violations, None, source);
    };
    let exit_numbers = [
        context.profile.syscall_number(Syscall::Exit),
        context.profile.syscall_number(Syscall::ExitGroup),
    ];
    let flows: BTreeMap<String, cfg::Flow> = program
        .functions
        .iter()
        .map(|function| {
            (
                function.name().to_owned(),
                cfg::analyze(function, &program, &exit_numbers, &mut violations),
            )
        })
        .collect();
    let template = source::parse(TEMPLATE).0;
    let facts = rules::check(
        &program,
        &flows,
        context.syscalls,
        context.profile,
        &template,
        &mut violations,
    );
    Report::new(violations, Some(facts), source)
}
