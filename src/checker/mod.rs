//! The rule checker for dialect 4: the lengua Power of 10 for x86-64 assembly, and the memory
//! rules M1 to M8.
//!
//! It reads main.s ([`source`]), reads the linked executable back and matches every instruction
//! to its line ([`decode`]), builds each function's control flow and stack depths ([`cfg`]), and
//! checks the rules ([`rules`], [`memory`] and [`threads`]).

pub mod aarch64;
pub mod cfg;
pub mod decode;
pub mod memory;
pub mod rules;
pub mod source;
pub mod tables;
pub mod threads;
pub mod x86_64;

#[cfg(test)]
pub(crate) mod golden;
#[cfg(test)]
pub(crate) mod tests;

use std::collections::BTreeMap;
use std::fmt;

use crate::isa::Isa;
use crate::platform::{CpuFeatures, Profile};
use crate::spec::Syscall;

pub use rules::Facts;
pub use source::{Source, UnsafeBlock};

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
}

impl Rule {
    /// How reports name the rule: `rule 4`, `rule M3`, or `dialect`.
    #[must_use]
    pub fn label(self) -> String {
        match (self.number(), self.memory_number()) {
            (Some(number), _) => format!("rule {number}"),
            (None, Some(number)) => format!("rule M{number}"),
            (None, None) => "dialect".to_owned(),
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
        match self.rule {
            Rule::Dialect => f.write_str("dialect")?,
            rule => write!(f, "{} ({})", rule.label(), rule.name())?,
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
    /// The edge's operations the spec allows, which a program may call.
    pub ops: &'a [crate::edge::Op],
    /// How many threads the spec allows at once, when it allows more than one.
    pub threads: Option<u32>,
    /// lengua.s as lengua assembled it after main.s, for a program (see [`lengua_source`]).
    pub lengua: Option<&'a Source>,
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

/// lengua.s as lengua assembles it after `main`, read the way main.s is: for a program in the edge
/// form, else `None`. Its functions are matched to the executable like main.s's; what the dialect
/// says of its layout is not a program's to answer for.
#[must_use]
pub fn lengua_source(profile: Profile, main: &str) -> Option<Source> {
    let text = crate::edge::lengua_text(profile, main)?;
    let first = match profile.isa() {
        Isa::X86_64 => ".intel_syntax noprefix\n",
        Isa::Aarch64 => ".arch armv8-a\n",
    };
    Some(source::parse(&format!("{first}{text}")).0)
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
    let Some(program) = decode::map(
        source,
        context.lengua,
        &machine,
        context.features,
        &mut violations,
    ) else {
        return Report::new(violations, None, source);
    };
    let exit_numbers = [
        context.profile.syscall_number(Syscall::Exit),
        context.profile.syscall_number(Syscall::ExitGroup),
    ];
    // Jumps through checked tables, found first, since the control flow follows their cases.
    let jumps = tables::every(&program, &mut violations);
    let mut flows: BTreeMap<String, cfg::Flow> = program
        .functions
        .iter()
        .map(|function| {
            let jumps = jumps.get(function.name()).map_or(&[][..], Vec::as_slice);
            (
                function.name().to_owned(),
                cfg::analyze(function, &program, &exit_numbers, jumps, &mut violations),
            )
        })
        .collect();
    // lengua's own functions are read for their stack, and answer to no rule of the program's.
    for function in &program.lengua {
        let flow = cfg::analyze(function, &program, &exit_numbers, &[], &mut Vec::new());
        flows.insert(function.name().to_owned(), flow);
    }
    let mut facts = rules::check(&program, &flows, context.ops, &mut violations);
    facts.features = match program.isa {
        Isa::X86_64 => x86_64::features_needed(&machine),
        Isa::Aarch64 => aarch64::features_needed(&machine, context.features),
    };
    facts.sve_bytes = context.features.sve;
    facts.named = program.named_symbols();
    facts.tables = tables::addresses(&program, &jumps);
    memory::check(
        &program,
        &flows,
        context.profile,
        facts.max_stack_bytes,
        machine.writable_executable,
        &mut violations,
    );
    // The thread code is lengua's; what main.s hands it and its worker are held to the rules.
    threads::check(&program, &flows, context.threads, &mut violations);
    if program.isa == Isa::Aarch64 {
        violations.iter_mut().for_each(aarch64::in_words);
    }
    Report::new(violations, Some(facts), source)
}
