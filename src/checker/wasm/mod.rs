//! The rule checker for the WebAssembly dialect 1: the lengua Power of 10 for `main.wat`, and
//! the memory rules M1 to M8, over the module lengua built from it (see
//! `plugin/skills/compile/references/power-of-10-webassembly.md`).
//!
//! It reads the module back ([`model`]), then checks its structure: imports, exports, memory,
//! regions and data ([`structure`]); each function's control flow, its loops and the call graph
//! ([`flow`]); every value that becomes an address, every access and every buffer a call borrows
//! ([`memory`]); the Power of 10's other rules ([`rules`]); and that every computer computes the
//! same bits ([`determinism`]). Violations are the native checker's [`Rule`]s and
//! [`Violation`]s, reported against main.wat's lines: `rule 4 (small functions) main.wat:12: …`.
//! What the rules can't settle is left to the proof, as an [`Obligation`].

pub mod determinism;
pub mod flow;
pub mod memory;
pub mod model;
pub mod rules;
pub mod structure;

#[cfg(test)]
mod tests;

use crate::checker::{MAX_VIOLATIONS, Rule, UnsafeBlock, Violation};
use crate::idea::{Network, Use};
use crate::wasm::Policy;
use crate::wasm::text::Built;

/// The file violations name.
pub const FILE: &str = crate::wasm::CODE_FILE;

/// What the checker needs besides main.wat and its module: what the idea allows.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    /// What the idea's `- Uses:` line names.
    pub uses: &'a [Use],
    /// What its `- Network:` line says.
    pub network: &'a Network,
}

impl Context<'_> {
    /// What the idea lets the program reach.
    #[must_use]
    pub fn policy(&self) -> Policy {
        Policy::of(self.uses, self.network)
    }
}

/// Something the rules can't settle, which the proof holds on every run it covers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Obligation {
    /// The main.wat line, when there is one.
    pub line: Option<usize>,
    /// What must hold.
    pub what: String,
    /// Whether it's a division's, that it never traps, which a proof settles in full; otherwise
    /// it's an access's, which a proof holds inside the program's memory but not inside its
    /// region, since the proof's model follows no region.
    pub division: bool,
}

/// What the rules learned about the program.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    /// main.wat's own functions.
    pub functions: usize,
    /// Its assertions.
    pub assertions: usize,
    /// The deepest call path from `_start`, by name, `_start` first.
    pub deepest_path: Vec<String>,
    /// The locals and operands of the deepest call path, in bytes, as the checker counts them.
    pub max_stack_bytes: u64,
    /// The regions: each one's name, first address, size, and whether it's read-only.
    pub regions: Vec<(String, u64, u64, bool)>,
    /// What the rules leave to the proof, by line.
    pub obligations: Vec<Obligation>,
}

/// What checking found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Violations, sorted by line, at most [`MAX_VIOLATIONS`].
    pub violations: Vec<Violation>,
    /// Violations beyond those shown.
    pub hidden: usize,
    /// What the rules learned.
    pub facts: Facts,
    /// The `;; unsafe:` blocks, all of which are reported.
    pub unsafe_blocks: Vec<UnsafeBlock>,
}

impl Report {
    /// The report of `violations`, sorted and capped.
    fn new(
        mut violations: Vec<Violation>,
        mut facts: Facts,
        unsafe_blocks: Vec<UnsafeBlock>,
    ) -> Self {
        violations.sort_by_key(|violation| (violation.line.unwrap_or(0), violation.rule));
        violations.dedup();
        let hidden = violations.len().saturating_sub(MAX_VIOLATIONS);
        violations.truncate(MAX_VIOLATIONS);
        facts.obligations.sort();
        facts.obligations.dedup();
        Self {
            violations,
            hidden,
            facts,
            unsafe_blocks,
        }
    }

    /// Whether every rule passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.violations.is_empty()
    }

    /// The findings, one line each, as `rule N (name) main.wat:LINE: problem. Fix: fix.`
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .violations
            .iter()
            .map(|violation| violation.in_file(FILE))
            .collect();
        if self.hidden > 0 {
            lines.push(format!("…and {} more", self.hidden));
        }
        lines
    }
}

/// Checks main.wat's `text`, which lengua built into `built`, against every rule of the
/// WebAssembly dialect, with what its idea allows.
#[must_use]
pub fn check(text: &str, built: &Built, context: &Context<'_>) -> Report {
    let module = match model::Module::read(text, built) {
        Ok(module) => module,
        Err(problem) => {
            let violation = Violation {
                rule: Rule::Tools,
                line: None,
                problem,
                fix: "check main.wat again; if this stays, it is a bug in lengua".to_owned(),
            };
            return Report::new(vec![violation], Facts::default(), Vec::new());
        }
    };
    let policy = context.policy();
    let mut violations = Vec::new();
    let mut facts = Facts {
        functions: module.own().count(),
        regions: module
            .regions
            .iter()
            .map(|region| {
                (
                    region.name.clone(),
                    region.start,
                    region.size,
                    region.read_only,
                )
            })
            .collect(),
        ..Facts::default()
    };
    structure::check(&module, built, &policy, &mut violations);
    let flows = flow::analyze(&module, &policy, &mut violations);
    let analysis = memory::analyze(&module, &flows);
    memory::check(&module, &flows, &analysis, &mut violations, &mut facts);
    rules::check(&module, &flows, &analysis, &mut violations, &mut facts);
    determinism::check(&module, &analysis, &mut violations, &mut facts);
    Report::new(violations, facts, module.unsafe_blocks.clone())
}
