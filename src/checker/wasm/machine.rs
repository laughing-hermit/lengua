//! The secret rule over the machine code: for an idea with a `- Secret:` line, the code the
//! engine compiles main.wat to on this CPU keeps the discipline the secret rule ([`super::secret`])
//! holds main.wat to. The WebAssembly can be free of every branch on a secret while the engine's
//! code for one of its operators isn't: on x86-64, Cranelift shifts 8-bit lanes by an amount that
//! varies with a mask it looks up in a table, at an index made from the amount.
//!
//! The secret rule gives every operator that takes a secret operand, by its offset in the module
//! ([`Taken`]). The engine compiles the module as it will run here, and its address map says, for
//! each stretch of its code, the offset of the operator the code was made for: the offsets
//! `wasmparser` gives the checker for the same bytes, since Cranelift's translator names each
//! operator by where the reader stands in the module before it reads it. The rule reads each
//! stretch made for an operator that takes a secret, and refuses, on the operator's line of
//! main.wat:
//!
//! - a conditional branch (x86-64: `jcc`, `jrcxz`, `loop`; ARM64: `b.cond`, `cbz`, `cbnz`,
//!   `tbz`, `tbnz`);
//! - a jump or a call through a register, and a call of any function, which is one of the
//!   engine's own;
//! - a division or a square root, of integers or of floats;
//! - the address of data in the code taken into a register (x86-64: `lea` of an address counted
//!   from the instruction; ARM64: `adr`, `adrp`), which is how the engine starts a lookup in a
//!   table or a jump through one; a constant loaded from the code with no index is not that;
//! - bytes it can't read as an instruction.
//!
//! What the secret rule already holds of an operator, its code may do ([`Takes`]): the code of
//! a `select` may branch on its condition, which isn't secret; the code of a load, a store, a
//! `global.set` or a bulk memory operator may check its addresses and call the engine's copy or
//! fill; the code of a call of one of main.wat's functions calls it.
//!
//! The address map gives some code to no operator: a function's entry, the constants it ends
//! with, values moved between registers and the stack, and what the engine's optimizer made anew
//! from a function's operators, which carries no offset. The optimizer makes a lane shift anew
//! in some programs, as the tests show, and its table lookup is then such code. So in a
//! function that has code of an operator that takes a secret, the rule reads the code given to
//! no operator as well, all but the function's first stretch, its entry, and its last, its
//! constants, and holds it as code that computes with a secret: a finding there is reported
//! beside the first operator after it that takes a secret, or the last before it.
//!
//! What it trusts: the time the CPU takes for the instructions it leaves, such as a
//! multiplication, a shift or a vector instruction; a function's entry; the code of functions
//! where no operator that takes a secret has code; the code of operators that take no secret,
//! of lengua's library, and of the engine's own functions and the host's; and the engine itself,
//! to compile a module the same way each time on one CPU, and to give its address map the
//! offsets of the operators its code came from.

use std::collections::{BTreeMap, BTreeSet};

use wasmtime::{Engine, Module};

use super::secret::{Taken, Takes};
use crate::checker::{Rule, Violation};
use crate::wasm::engine::{self, Setup};

mod aarch64;
mod x86_64;

/// A CPU whose machine code the rule reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cpu {
    /// x86-64, read with `iced-x86`.
    X86_64,
    /// ARM64, read by the architecture's encoding index.
    Aarch64,
}

impl Cpu {
    /// The CPU lengua was built for, which is the one the engine compiles for here; none when
    /// the rule can't read its code.
    #[must_use]
    pub const fn here() -> Option<Self> {
        if cfg!(target_arch = "x86_64") {
            Some(Self::X86_64)
        } else if cfg!(target_arch = "aarch64") {
            Some(Self::Aarch64)
        } else {
            None
        }
    }

    /// Its name in a report.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::X86_64 => "x86-64",
            Self::Aarch64 => "ARM64",
        }
    }

    /// Reads `code`, which starts at an instruction, `at` bytes into the engine's code.
    #[must_use]
    pub fn read(self, code: &[u8], at: usize) -> Read {
        match self {
            Self::X86_64 => x86_64::read(code, at),
            Self::Aarch64 => aarch64::read(code, at),
        }
    }
}

/// What an instruction is, of the kinds the rule has a word for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Made {
    /// A branch that a condition decides.
    Branch,
    /// A jump through a register.
    IndirectJump,
    /// A call of a function at a fixed place.
    Call,
    /// A call through a register or memory.
    IndirectCall,
    /// A division or a square root.
    Division,
    /// The address of data in the code, taken into a register: a table's.
    Table,
    /// Bytes that are no instruction the rule reads.
    Unread,
}

impl Made {
    /// What the code has, and what follows from it for a secret.
    const fn words(self) -> (&'static str, &'static str) {
        match self {
            Self::Branch => (
                "a conditional branch",
                "a secret may decide which code runs",
            ),
            Self::IndirectJump => (
                "a jump through a register",
                "a secret may decide which code runs",
            ),
            Self::Call | Self::IndirectCall => (
                "a call of one of the engine's own functions",
                "the time the call takes may depend on the secret",
            ),
            Self::Division => (
                "a division or a square root",
                "the time it takes can depend on the secret",
            ),
            Self::Table => (
                "the address of a table in the code, which it then reads at an index",
                "a secret may decide which bytes of memory the code reads",
            ),
            Self::Unread => (
                "bytes lengua can't read as an instruction",
                "lengua can't say that the code takes the same time whatever the secret is",
            ),
        }
    }

    /// What to write instead.
    const fn fix(self) -> &'static str {
        match self {
            Self::Table => {
                "shift by an amount that isn't secret, such as a constant; or shift 16-bit \
                 lanes, with `i16x8.shl` or `i16x8.shr_u`, and clear the bits that crossed into \
                 the next lane with `v128.and`, as references/constant-time.md shows"
            }
            _ => {
                "compute it with operators the engine compiles to straight code on every CPU: \
                 `and`, `or`, `xor`, `add`, `sub`, `mul` and shifts of integers, and masks to \
                 choose with, as references/constant-time.md shows"
            }
        }
    }
}

/// An instruction the rule has a word for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    /// Where it is, in bytes from the start of the engine's code.
    pub at: usize,
    /// What it is.
    pub made: Made,
    /// Its name, such as `jb`.
    pub name: String,
}

/// What a stretch of code holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Read {
    /// How many instructions were read.
    pub instructions: usize,
    /// Those the rule has a word for, in order.
    pub seen: Vec<Seen>,
}

/// The code the engine compiled a module to, with what says where each part came from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Code {
    /// The machine code of every function.
    pub text: Vec<u8>,
    /// Where each of the module's own functions is in it: its first byte, and its length.
    pub functions: Vec<(usize, usize)>,
    /// The address map, in order: from each of these places in the code up to the next, the
    /// code was made for the operator at this offset in the module, or for none.
    pub map: Vec<(usize, Option<u32>)>,
}

impl Code {
    /// The code `engine`, one that compiles, makes of the module `wasm`.
    ///
    /// # Errors
    ///
    /// When the module doesn't compile, or the engine kept no address map.
    pub fn of(engine: &Engine, wasm: &[u8]) -> Result<Self, String> {
        let module = engine::compile(engine, wasm)
            .map_err(|error| format!("the engine couldn't compile the module: {error}"))?;
        Self::of_module(&module)
    }

    /// The code of `module` as an engine holds it, compiled just now or loaded from what was
    /// compiled before: the very code a run of that module executes.
    ///
    /// # Errors
    ///
    /// When the engine kept no address map.
    pub fn of_module(module: &Module) -> Result<Self, String> {
        let map: Vec<(usize, Option<u32>)> = module
            .address_map()
            .map(Iterator::collect)
            .unwrap_or_default();
        if map.is_empty() {
            return Err(
                "the engine kept no map from its code to the module's operators".to_owned(),
            );
        }
        Ok(Self {
            text: module.text().to_vec(),
            functions: module
                .functions()
                .map(|function| (function.offset, function.len))
                .collect(),
            map,
        })
    }
}

/// Code the rule refuses: what the engine made for an operator that takes a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The operator's offset in the module.
    pub offset: u64,
    /// What the code has.
    pub made: Made,
    /// The first such instruction's name.
    pub name: String,
    /// Whether the address map gives the code to the operator itself. Otherwise it gives the
    /// code to no operator, in a function that computes with a secret, and the operator is the
    /// nearest that takes one.
    pub own: bool,
}

/// What the rule read of a module's code.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Judged {
    /// How many of the operators that take a secret have code of their own.
    pub operators: usize,
    /// How many instructions were read for them.
    pub instructions: usize,
    /// What the rule refuses, by operator and kind.
    pub findings: Vec<Finding>,
}

/// Whether the code of an operator the secret rule holds as `takes` may have `made`.
const fn allowed(takes: Takes, made: Made) -> bool {
    match (takes, made) {
        (Takes::Plain, _) | (_, Made::Unread | Made::Division) => false,
        (Takes::Choice, made) => matches!(made, Made::Branch),
        (Takes::Call, made) => matches!(made, Made::Call),
        (Takes::Memory, made) => !matches!(made, Made::IndirectJump),
    }
}

/// Holds `code`, compiled for `cpu`, to the rule: reads each stretch the address map gives an
/// operator of `taken`, inside the module's functions, and finds what that operator's code may
/// not have.
#[must_use]
pub fn judge(cpu: Cpu, code: &Code, taken: &[Taken]) -> Judged {
    let by_offset: BTreeMap<u64, &Taken> =
        taken.iter().map(|taken| (taken.offset, taken)).collect();
    let mut with_code = BTreeSet::new();
    let mut found: BTreeMap<(u64, Made, bool), String> = BTreeMap::new();
    let mut instructions = 0_usize;
    for &(start, length) in &code.functions {
        let end = start.saturating_add(length).min(code.text.len());
        let first = code.map.partition_point(|(at, _)| *at < start);
        let last = code.map.partition_point(|(at, _)| *at < end);
        let entries = code.map.get(first..last).unwrap_or_default();
        // A stretch ends where the next begins, or with its function.
        let stretch = |index: usize| {
            let (at, _) = *entries.get(index)?;
            let stop = entries.get(index + 1).map_or(end, |(next, _)| *next);
            Some((at, code.text.get(at..stop)?))
        };
        // The code made for each operator that takes a secret.
        let mut secret: Vec<(usize, &Taken)> = Vec::new();
        for (index, (_, operator)) in entries.iter().enumerate() {
            let found_taken =
                operator.and_then(|offset| by_offset.get(&u64::from(offset)).copied());
            let Some(taken) = found_taken else {
                continue;
            };
            let Some((at, bytes)) = stretch(index) else {
                continue;
            };
            let read = cpu.read(bytes, at);
            instructions = instructions.saturating_add(read.instructions);
            with_code.insert(taken.offset);
            secret.push((index, taken));
            for seen in read.seen {
                if !allowed(taken.takes, seen.made) {
                    found
                        .entry((taken.offset, seen.made, true))
                        .or_insert(seen.name);
                }
            }
        }
        // In a function that computes with a secret, the code no operator is given: all of it
        // but the function's entry, which comes first, and the constants it ends with. What the
        // engine's optimizer makes anew from a function's operators is there, and may be made
        // from one that takes a secret, so it is held as code that computes with one, and found
        // beside the first such operator after it, which is most often the one that takes what
        // the code computes, or else the last before it.
        for (index, (_, operator)) in entries.iter().enumerate() {
            let inside = index > 0 && index + 1 < entries.len();
            let beside = secret
                .iter()
                .find(|(at, _)| *at > index)
                .or_else(|| secret.last());
            let (true, None, Some((_, taken))) = (inside, operator, beside) else {
                continue;
            };
            let Some((at, bytes)) = stretch(index) else {
                continue;
            };
            for seen in cpu.read(bytes, at).seen {
                found
                    .entry((taken.offset, seen.made, false))
                    .or_insert(seen.name);
            }
        }
    }
    Judged {
        operators: with_code.len(),
        instructions,
        findings: found
            .into_iter()
            .map(|((offset, made, own), name)| Finding {
                offset,
                made,
                name,
                own,
            })
            .collect(),
    }
}

/// `findings` of the code compiled for `cpu` as violations of the secret rule, each on the line
/// of main.wat of its operator, one of `taken`.
#[must_use]
pub fn violations(cpu: Cpu, taken: &[Taken], findings: &[Finding]) -> Vec<Violation> {
    findings
        .iter()
        .map(|finding| {
            let operator = taken.iter().find(|taken| taken.offset == finding.offset);
            let name = operator.map_or("operator", |taken| taken.name.as_str());
            let (what, so) = finding.made.words();
            let problem = if finding.own {
                format!(
                    "on {}, the engine compiles this `{name}`, which takes a secret, to code \
                     with {what} (`{}`), so {so}",
                    cpu.name(),
                    finding.name
                )
            } else {
                format!(
                    "on {}, beside the code of this `{name}`, which takes a secret, the engine \
                     made code from this function's operators that it gives to none of them, \
                     with {what} (`{}`), so {so}",
                    cpu.name(),
                    finding.name
                )
            };
            Violation {
                rule: Rule::Secrets,
                line: operator.and_then(|taken| taken.line),
                problem,
                fix: finding.made.fix().to_owned(),
            }
        })
        .collect()
}

/// A problem of the check itself, as a violation of the secret rule with no line.
fn unchecked(why: &str) -> Vec<Violation> {
    vec![Violation {
        rule: Rule::Secrets,
        line: None,
        problem: format!(
            "the idea names a secret, and lengua couldn't check the machine code the engine \
             compiles main.wat to: {why}"
        ),
        fix: "check again; if this stays, it is a bug in lengua".to_owned(),
    }]
}

/// What a check of a module's machine code read, when it refused nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Passed {
    /// The CPU whose code it read.
    pub cpu: Cpu,
    /// How many operators take a secret.
    pub operators: usize,
    /// How many of them have code of their own.
    pub with_code: usize,
    /// How many instructions it read for them, over every configuration.
    pub instructions: usize,
}

impl Passed {
    /// What `check` says of it, in one line: the CPU and how many operators take a secret, which
    /// main.wat decides, and not how many instructions were read, which the CPU's features do,
    /// so that one main.wat gives one line on every computer of a CPU.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "the {} code the engine compiles main.wat to on this computer keeps the secret \
             rule: what it made for the {} operators that take a secret has no branch, no \
             table, no call and no division for a secret to decide",
            self.cpu.name(),
            self.operators
        )
    }
}

/// Checks the machine code of the module `wasm` for this CPU, as the engine compiles it in each
/// of `setups`: the code made for each operator of
/// `taken`, the operators the secret rule found to take a secret, has nothing the rule refuses.
///
/// # Errors
///
/// What the code has, as violations of the secret rule on main.wat's lines; or why the code
/// couldn't be checked, as one with no line.
pub fn check(wasm: &[u8], taken: &[Taken], setups: &[Setup]) -> Result<Passed, Vec<Violation>> {
    let Some(cpu) = Cpu::here() else {
        return Err(unchecked(
            "it reads the code of x86-64 and ARM64 CPUs, and this CPU is neither",
        ));
    };
    let mut passed = Passed {
        cpu,
        operators: taken.len(),
        with_code: 0,
        instructions: 0,
    };
    let mut refused: Vec<Violation> = Vec::new();
    for &setup in setups {
        let engine = engine::engine(setup).map_err(|error| unchecked(&error))?;
        let code = Code::of(&engine, wasm).map_err(|error| unchecked(&error))?;
        judge_into(cpu, &code, taken, &mut passed, &mut refused);
    }
    if refused.is_empty() {
        Ok(passed)
    } else {
        Err(refused)
    }
}

/// Holds `code` to the rule as [`check`] does, adding what it read to `passed` and what it
/// refuses to `refused`, each violation once.
fn judge_into(
    cpu: Cpu,
    code: &Code,
    taken: &[Taken],
    passed: &mut Passed,
    refused: &mut Vec<Violation>,
) {
    let judged = judge(cpu, code, taken);
    passed.with_code = passed.with_code.max(judged.operators);
    passed.instructions = passed.instructions.saturating_add(judged.instructions);
    for violation in violations(cpu, taken, &judged.findings) {
        if !refused.contains(&violation) {
            refused.push(violation);
        }
    }
}

/// Checks the machine code of `module`, a compiled module as an engine holds it, as [`check`]
/// checks what an engine compiles: a run from a shell checks so the very code it is about to
/// execute, which it loaded from what was compiled before.
///
/// # Errors
///
/// What the code has, as violations of the secret rule on main.wat's lines; or why the code
/// couldn't be checked, as one with no line.
pub fn check_loaded(module: &Module, taken: &[Taken]) -> Result<Passed, Vec<Violation>> {
    let Some(cpu) = Cpu::here() else {
        return Err(unchecked(
            "it reads the code of x86-64 and ARM64 CPUs, and this CPU is neither",
        ));
    };
    let mut passed = Passed {
        cpu,
        operators: taken.len(),
        with_code: 0,
        instructions: 0,
    };
    let mut refused: Vec<Violation> = Vec::new();
    let code = Code::of_module(module).map_err(|error| unchecked(&error))?;
    judge_into(cpu, &code, taken, &mut passed, &mut refused);
    if refused.is_empty() {
        Ok(passed)
    } else {
        Err(refused)
    }
}

#[cfg(test)]
mod tests;
