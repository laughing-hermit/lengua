//! Reads the linked executable back and matches it to main.s instruction by instruction,
//! operands included, so the step from main.s to machine code is a checked translation.

use std::collections::BTreeMap;

use iced_x86::{Decoder, DecoderOptions, Instruction, InstructionInfoFactory};
use object::{
    Object, ObjectSection, ObjectSegment, ObjectSymbol, SectionKind, SegmentFlags, SymbolKind,
};

use super::source::{
    Function as SourceFunction, Instruction as SourceInstruction, Item, Label, Memory, Operand,
    Source,
};
use super::{Rule, Violation, aarch64 as aarch64_checks, x86_64};
use crate::isa::{Abi, Insn, Isa, aarch64};
use crate::platform::CpuFeatures;

/// A function in the executable.
#[derive(Debug, Clone)]
pub struct MachineFunction {
    /// Its address.
    pub address: u64,
    /// Its size in bytes.
    pub size: u64,
    /// Its machine code, as its CPU's decoder reads it.
    pub code: Code,
}

/// A function's machine code, as its CPU's decoder reads it.
#[derive(Debug, Clone)]
pub enum Code {
    /// x86-64 instructions, as iced decodes them.
    X86(Vec<Instruction>),
    /// AArch64 words, each in the dialect's canonical form or why it isn't in the dialect.
    Aarch64(Vec<Word>),
}

/// One AArch64 machine word.
#[derive(Debug, Clone)]
pub struct Word {
    /// Its address.
    pub address: u64,
    /// The word.
    pub bits: u32,
    /// What it is, or why it isn't in the dialect.
    pub inst: Result<aarch64::Inst, String>,
}

impl MachineFunction {
    /// Its x86-64 instructions: none when it's another CPU's code.
    #[must_use]
    pub fn x86(&self) -> &[Instruction] {
        match &self.code {
            Code::X86(instructions) => instructions,
            Code::Aarch64(_) => &[],
        }
    }

    /// Its AArch64 words, when it's AArch64 code.
    #[must_use]
    pub fn words(&self) -> &[Word] {
        match &self.code {
            Code::Aarch64(words) => words,
            Code::X86(_) => &[],
        }
    }

    /// How many instructions it has.
    #[must_use]
    pub fn len(&self) -> usize {
        match &self.code {
            Code::X86(instructions) => instructions.len(),
            Code::Aarch64(words) => words.len(),
        }
    }

    /// Whether it has no instructions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A data symbol in the executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineData {
    /// Its address.
    pub address: u64,
    /// Its size in bytes.
    pub size: u64,
    /// Whether its section is writable.
    pub writable: bool,
    /// Its initial bytes (empty for zero-filled data).
    pub bytes: Vec<u8>,
}

/// The executable's functions and data, by name.
#[derive(Debug, Clone, Default)]
pub struct Machine {
    /// The CPU the code is for, from the executable's header.
    pub isa: Isa,
    /// Functions.
    pub functions: BTreeMap<String, MachineFunction>,
    /// Data symbols.
    pub data: BTreeMap<String, MachineData>,
    /// Whether a loaded segment is both writable and executable.
    pub writable_executable: bool,
    /// The data symbol each `[rip + name]` operand names in main.s, by its instruction's
    /// address, once the checker has matched main.s to the executable (see
    /// [`Program::named_symbols`]); empty until then.
    pub named: BTreeMap<u64, String>,
    /// Each checked jump table's `jmp`, by address, and the addresses of its cases in the
    /// table's order, once the checker has found them (see [`super::tables`]); empty until then.
    pub tables: BTreeMap<u64, Vec<u64>>,
}

/// Reads the functions and data symbols of a linked ELF executable and decodes the functions.
///
/// # Errors
///
/// Fails when the file isn't the kind of executable lengua builds.
pub fn read(executable: &[u8]) -> Result<Machine, String> {
    let file = object::File::parse(executable)
        .map_err(|error| format!("can't read the executable: {error}"))?;
    let mut machine = Machine {
        isa: match file.architecture() {
            object::Architecture::X86_64 => Isa::X86_64,
            object::Architecture::Aarch64 => Isa::Aarch64,
            other => {
                return Err(format!(
                    "it's code for {other:?}, which lengua doesn't check"
                ));
            }
        },
        ..Machine::default()
    };
    let both = object::elf::PF_W.0 | object::elf::PF_X.0;
    machine.writable_executable = file.segments().any(|segment| {
        matches!(
            segment.flags(),
            SegmentFlags::Elf { p_flags, .. } if p_flags.0 & both == both
        )
    });
    let mut text_ranges = Vec::new();
    for symbol in file.symbols() {
        let Ok(name) = symbol.name() else { continue };
        if name.is_empty() || symbol.size() == 0 {
            continue;
        }
        let Some(section) = symbol
            .section_index()
            .and_then(|index| file.section_by_index(index).ok())
        else {
            continue;
        };
        let start = symbol
            .address()
            .checked_sub(section.address())
            .ok_or("a symbol lies outside its section")?;
        let data = section
            .data()
            .map_err(|error| format!("can't read section data: {error}"))?;
        let bytes = usize::try_from(start)
            .ok()
            .zip(usize::try_from(symbol.size()).ok())
            .and_then(|(start, size)| data.get(start..start.checked_add(size)?))
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        match symbol.kind() {
            SymbolKind::Text => {
                text_ranges.push((symbol.address(), symbol.size(), section.size()));
                let code = match machine.isa {
                    Isa::X86_64 => Code::X86(x86_instructions(name, &bytes, symbol.address())?),
                    Isa::Aarch64 => Code::Aarch64(words(name, &bytes, symbol.address())?),
                };
                machine.functions.insert(
                    name.to_owned(),
                    MachineFunction {
                        address: symbol.address(),
                        size: symbol.size(),
                        code,
                    },
                );
            }
            SymbolKind::Data => {
                let writable = matches!(
                    section.kind(),
                    SectionKind::Data | SectionKind::UninitializedData
                );
                machine.data.insert(
                    name.to_owned(),
                    MachineData {
                        address: symbol.address(),
                        size: symbol.size(),
                        writable,
                        bytes,
                    },
                );
            }
            _ => {}
        }
    }
    // Every byte of code must belong to a function, so nothing runs that wasn't checked.
    let covered: u64 = text_ranges.iter().map(|(_, size, _)| size).sum();
    let section_size = text_ranges.first().map_or(0, |(_, _, size)| *size);
    if covered != section_size {
        return Err(format!(
            "the code section holds {section_size} bytes but the functions cover {covered}"
        ));
    }
    Ok(machine)
}

/// The x86-64 instructions in `bytes`, the function `name` at `address`.
fn x86_instructions(name: &str, bytes: &[u8], address: u64) -> Result<Vec<Instruction>, String> {
    let mut decoder = Decoder::with_ip(64, bytes, address, DecoderOptions::NONE);
    let mut instructions = Vec::new();
    while decoder.can_decode() {
        let instruction = decoder.decode();
        if instruction.is_invalid() {
            return Err(format!(
                "{name} holds bytes that aren't an instruction at {:#x}",
                instruction.ip()
            ));
        }
        instructions.push(instruction);
    }
    Ok(instructions)
}

/// The AArch64 words in `bytes`, the function `name` at `address`, each decoded.
fn words(name: &str, bytes: &[u8], address: u64) -> Result<Vec<Word>, String> {
    let (whole, rest) = bytes.as_chunks::<4>();
    if !rest.is_empty() {
        return Err(format!("{name} holds bytes that aren't whole instructions"));
    }
    Ok(whole
        .iter()
        .zip((address..).step_by(4))
        .map(|(bytes, address)| {
            let bits = u32::from_le_bytes(*bytes);
            Word {
                address,
                bits,
                inst: aarch64::decode::decode(bits, address),
            }
        })
        .collect())
}

/// One main.s instruction and the machine instruction it became.
#[derive(Debug, Clone)]
pub struct Step<'a> {
    /// The source line.
    pub source: &'a SourceInstruction,
    /// The machine instruction's facts, as the rules read them.
    pub insn: Insn,
}

/// A local label and the address it names.
#[derive(Debug, Clone)]
pub struct LabelAt<'a> {
    /// The label.
    pub label: &'a Label,
    /// The index of the step that follows it.
    pub index: usize,
    /// Its address.
    pub address: u64,
}

/// A function with its source and machine instructions matched up.
#[derive(Debug, Clone)]
pub struct MappedFunction<'a> {
    /// The source function.
    pub source: &'a SourceFunction,
    /// Its first address.
    pub address: u64,
    /// The address just past its end.
    pub end: u64,
    /// The instructions, in order.
    pub steps: Vec<Step<'a>>,
    /// The local labels, in order.
    pub labels: Vec<LabelAt<'a>>,
}

impl MappedFunction<'_> {
    /// The index of the instruction at `address`.
    #[must_use]
    pub fn index_of(&self, address: u64) -> Option<usize> {
        self.steps
            .binary_search_by_key(&address, |step| step.insn.address)
            .ok()
    }

    /// The name of the function.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.source.name
    }

    /// The label right before the instruction at `index`, if any.
    #[must_use]
    pub fn label_before(&self, index: usize) -> Option<&LabelAt<'_>> {
        self.labels.iter().find(|label| label.index == index)
    }
}

/// main.s and the executable, matched up.
#[derive(Debug, Clone)]
pub struct Program<'a> {
    /// The CPU the code is for.
    pub isa: Isa,
    /// The parsed source.
    pub source: &'a Source,
    /// The functions, in source order.
    pub functions: Vec<MappedFunction<'a>>,
    /// Data symbols by name.
    pub data: BTreeMap<String, MachineData>,
}

impl Program<'_> {
    /// The calling convention of the CPU the code is for.
    #[must_use]
    pub const fn abi(&self) -> &'static Abi {
        self.isa.abi()
    }

    /// A function by name.
    #[must_use]
    pub fn function(&self, name: &str) -> Option<&MappedFunction<'_>> {
        self.functions
            .iter()
            .find(|function| function.name() == name)
    }

    /// The function starting at `address`.
    #[must_use]
    pub fn function_at(&self, address: u64) -> Option<&MappedFunction<'_>> {
        self.functions
            .iter()
            .find(|function| function.address == address)
    }

    /// The data symbol a step's `[rip + name]` operand names in main.s, with the offset main.s
    /// writes after the name, which may reach past the symbol's end. The name decides, never the
    /// address: where one symbol ends just as the next starts, `[rip + a + 8]` and `[rip + b]`
    /// are one address, but only one of them is an access to b.
    #[must_use]
    pub fn named(&self, step: &Step<'_>) -> Option<(&str, &MachineData, i64)> {
        step.source
            .operands
            .iter()
            .find_map(|operand| match operand {
                Operand::Memory(Memory {
                    symbol: Some(symbol),
                    displacement,
                    ..
                }) => self
                    .data
                    .get_key_value(symbol)
                    .map(|(name, data)| (name.as_str(), data, *displacement)),
                _ => None,
            })
    }

    /// The data symbol each `[rip + name]` operand names, by its instruction's address, for
    /// proofs, which run the machine code and so see only addresses.
    #[must_use]
    pub fn named_symbols(&self) -> BTreeMap<u64, String> {
        self.functions
            .iter()
            .flat_map(|function| &function.steps)
            .filter_map(|step| Some((step.insn.address, self.named(step)?.0.to_owned())))
            .collect()
    }
}

/// Matches every source function to its machine code. Returns `None` when they don't match,
/// after recording why: then nothing else can be trusted.
pub fn map<'a>(
    source: &'a Source,
    machine: &Machine,
    features: CpuFeatures,
    violations: &mut Vec<Violation>,
) -> Option<Program<'a>> {
    let before = violations.len();
    let mut functions = Vec::new();
    for function in &source.functions {
        let Some(code) = machine.functions.get(&function.name) else {
            violations.push(Violation {
                rule: Rule::Dialect,
                line: Some(function.line),
                problem: format!("{} isn't in the executable", function.name),
                fix: "give it `.type` and `.size` lines and keep it in .text".into(),
            });
            continue;
        };
        if let Some(mapped) = map_function(function, code, violations) {
            functions.push(mapped);
        }
    }
    if violations.len() > before {
        return None;
    }
    let program = Program {
        isa: machine.isa,
        source,
        functions,
        data: machine.data.clone(),
    };
    for function in &program.functions {
        let code = match &machine.functions[function.name()].code {
            Code::X86(code) => code,
            Code::Aarch64(words) => {
                for (step, word) in function.steps.iter().zip(words) {
                    if let Err(problem) =
                        aarch64_checks::same_instruction(step, word, function, &program)
                    {
                        violations.push(Violation {
                            rule: Rule::Dialect,
                            line: Some(step.source.line),
                            problem: format!(
                                "`{}` became `{}` in the machine code: {problem}",
                                step.source.text,
                                aarch64_checks::display(word)
                            ),
                            fix: "write the instruction so it means what the machine does; \
                                  constants must come from the block at the top and data must \
                                  be addressed as adrp and add :lo12:"
                                .into(),
                        });
                    }
                    aarch64_checks::check_instruction(step, word, features, violations);
                }
                aarch64_checks::check_pairs(function, violations);
                continue;
            }
        };
        for (step, instruction) in function.steps.iter().zip(code) {
            if let Err(problem) =
                x86_64::same_instruction(step, instruction, function, &program, machine)
            {
                violations.push(Violation {
                    rule: Rule::Dialect,
                    line: Some(step.source.line),
                    problem: format!(
                        "`{}` became `{}` in the machine code: {problem}",
                        step.source.text,
                        x86_64::display(instruction)
                    ),
                    fix: "write the instruction so it means what the machine does; constants \
                          must come from the block at the top and data must be addressed as \
                          [rip + name]"
                        .into(),
                });
            }
            x86_64::check_instruction(step, instruction, features, violations);
        }
    }
    (violations.len() == before).then_some(program)
}

fn map_function<'a>(
    function: &'a SourceFunction,
    code: &MachineFunction,
    violations: &mut Vec<Violation>,
) -> Option<MappedFunction<'a>> {
    let sources: Vec<&SourceInstruction> = function.instructions().collect();
    let steps = match &code.code {
        Code::X86(instructions) => x86_steps(function, sources, instructions, violations)?,
        Code::Aarch64(words) => aarch64_checks::steps(function, sources, words, violations)?,
    };
    let end = code.address + code.size;
    let mut labels = Vec::new();
    let mut index = 0;
    for item in &function.items {
        match item {
            Item::Instruction(_) => index += 1,
            Item::Label(label) => labels.push(LabelAt {
                label,
                index,
                address: steps.get(index).map_or(end, |step| step.insn.address),
            }),
        }
    }
    Some(MappedFunction {
        source: function,
        address: code.address,
        end,
        steps,
        labels,
    })
}

/// The steps of an x86-64 function: each line of main.s with the instruction it became, when
/// they match line for line, mnemonic for mnemonic.
fn x86_steps<'a>(
    function: &'a SourceFunction,
    sources: Vec<&'a SourceInstruction>,
    instructions: &[Instruction],
    violations: &mut Vec<Violation>,
) -> Option<Vec<Step<'a>>> {
    let first_difference = sources
        .iter()
        .zip(instructions)
        .position(|(source, machine)| {
            x86_64::canonical(&source.mnemonic) != x86_64::mnemonic(machine)
        });
    if sources.len() != instructions.len() || first_difference.is_some() {
        let index = first_difference.unwrap_or(sources.len().min(instructions.len()));
        let (line, found) = match (sources.get(index), instructions.get(index)) {
            (Some(source), Some(machine)) => (
                source.line,
                format!("`{}` became `{}`", source.text, x86_64::display(machine)),
            ),
            (Some(source), None) => (
                source.line,
                format!("`{}` has no machine instruction", source.text),
            ),
            (None, Some(machine)) => (
                function.line,
                format!(
                    "the machine code has an extra `{}`",
                    x86_64::display(machine)
                ),
            ),
            (None, None) => (function.line, String::new()),
        };
        violations.push(Violation {
            rule: Rule::Dialect,
            line: Some(line),
            problem: format!(
                "main.s and the machine code of {} don't match line for line: {found}",
                function.name
            ),
            fix: "write exactly one plain instruction per line, with no prefixes or directives \
                  between instructions"
                .into(),
        });
        return None;
    }
    let mut factory = InstructionInfoFactory::new();
    Some(
        sources
            .into_iter()
            .zip(instructions)
            .map(|(source, machine)| Step {
                source,
                insn: crate::isa::x86_64::facts(machine, &mut factory),
            })
            .collect(),
    )
}
