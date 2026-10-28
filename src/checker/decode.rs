//! Reads the linked executable back and matches it to main.s instruction by instruction,
//! operands included, so the step from main.s to machine code is a checked translation.

use std::collections::BTreeMap;

use iced_x86::{
    CpuidFeature, Decoder, DecoderOptions, FlowControl, Formatter, Instruction, IntelFormatter,
    MemorySize, OpKind, Register,
};
use object::{Object, ObjectSection, ObjectSymbol, SectionKind, SymbolKind};

use super::source::{
    Function as SourceFunction, Instruction as SourceInstruction, Item, Label, Memory, Operand,
    Source,
};
use super::{Rule, Violation};
use crate::platform::CpuFeatures;

/// A function in the executable.
#[derive(Debug, Clone)]
pub struct MachineFunction {
    /// Its address.
    pub address: u64,
    /// Its size in bytes.
    pub size: u64,
    /// Its decoded instructions.
    pub instructions: Vec<Instruction>,
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
    /// Functions.
    pub functions: BTreeMap<String, MachineFunction>,
    /// Data symbols.
    pub data: BTreeMap<String, MachineData>,
}

/// Reads the functions and data symbols of a linked ELF executable and decodes the functions.
///
/// # Errors
///
/// Fails when the file isn't the kind of executable lengua builds.
pub fn read(executable: &[u8]) -> Result<Machine, String> {
    let file = object::File::parse(executable)
        .map_err(|error| format!("can't read the executable: {error}"))?;
    let mut machine = Machine::default();
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
                let mut decoder =
                    Decoder::with_ip(64, &bytes, symbol.address(), DecoderOptions::NONE);
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
                machine.functions.insert(
                    name.to_owned(),
                    MachineFunction {
                        address: symbol.address(),
                        size: symbol.size(),
                        instructions,
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

/// One main.s instruction and the machine instruction it became.
#[derive(Debug, Clone)]
pub struct Step<'a> {
    /// The source line.
    pub source: &'a SourceInstruction,
    /// The decoded instruction.
    pub machine: Instruction,
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
            .binary_search_by_key(&address, |step| step.machine.ip())
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
    /// The parsed source.
    pub source: &'a Source,
    /// The functions, in source order.
    pub functions: Vec<MappedFunction<'a>>,
    /// Data symbols by name.
    pub data: BTreeMap<String, MachineData>,
}

impl Program<'_> {
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

    /// The data symbol containing `address`.
    #[must_use]
    pub fn data_at(&self, address: u64) -> Option<(&str, &MachineData)> {
        self.data
            .iter()
            .find(|(_, data)| address >= data.address && address < data.address + data.size)
            .map(|(name, data)| (name.as_str(), data))
    }
}

/// Instructions dialect 1 allows: integer instructions and `syscall`.
const ALLOWED: &[&str] = &[
    "mov", "movzx", "movsx", "movsxd", "lea", "push", "pop", "xchg", "bswap", "movbe", "cmovo",
    "cmovno", "cmovb", "cmovae", "cmove", "cmovne", "cmovbe", "cmova", "cmovs", "cmovns", "cmovp",
    "cmovnp", "cmovl", "cmovge", "cmovle", "cmovg", "add", "adc", "sub", "sbb", "neg", "inc",
    "dec", "mul", "imul", "div", "idiv", "cbw", "cwde", "cdqe", "cwd", "cdq", "cqo", "adcx",
    "adox", "mulx", "and", "or", "xor", "not", "test", "cmp", "shl", "shr", "sar", "rol", "ror",
    "shld", "shrd", "bt", "bts", "btr", "btc", "bsf", "bsr", "seto", "setno", "setb", "setae",
    "sete", "setne", "setbe", "seta", "sets", "setns", "setp", "setnp", "setl", "setge", "setle",
    "setg", "andn", "bextr", "blsi", "blsmsk", "blsr", "tzcnt", "bzhi", "pdep", "pext", "rorx",
    "sarx", "shlx", "shrx", "lzcnt", "popcnt", "jmp", "jo", "jno", "jb", "jae", "je", "jne", "jbe",
    "ja", "js", "jns", "jp", "jnp", "jl", "jge", "jle", "jg", "call", "ret", "syscall", "nop",
];

/// Alternative spellings the assembler accepts, and the name the decoder gives them.
const ALIASES: &[(&str, &str)] = &[
    ("jz", "je"),
    ("jnz", "jne"),
    ("jc", "jb"),
    ("jnae", "jb"),
    ("jnc", "jae"),
    ("jnb", "jae"),
    ("jna", "jbe"),
    ("jnbe", "ja"),
    ("jnge", "jl"),
    ("jnl", "jge"),
    ("jng", "jle"),
    ("jnle", "jg"),
    ("jpe", "jp"),
    ("jpo", "jnp"),
    ("sal", "shl"),
    ("cmovz", "cmove"),
    ("cmovnz", "cmovne"),
    ("cmovc", "cmovb"),
    ("cmovnae", "cmovb"),
    ("cmovnc", "cmovae"),
    ("cmovnb", "cmovae"),
    ("cmovna", "cmovbe"),
    ("cmovnbe", "cmova"),
    ("cmovnge", "cmovl"),
    ("cmovnl", "cmovge"),
    ("cmovng", "cmovle"),
    ("cmovnle", "cmovg"),
    ("cmovpe", "cmovp"),
    ("cmovpo", "cmovnp"),
    ("setz", "sete"),
    ("setnz", "setne"),
    ("setc", "setb"),
    ("setnae", "setb"),
    ("setnc", "setae"),
    ("setnb", "setae"),
    ("setna", "setbe"),
    ("setnbe", "seta"),
    ("setnge", "setl"),
    ("setnl", "setge"),
    ("setng", "setle"),
    ("setnle", "setg"),
    ("setpe", "setp"),
    ("setpo", "setnp"),
];

/// The decoder's name for an instruction, in lowercase.
#[must_use]
pub fn mnemonic(instruction: &Instruction) -> String {
    format!("{:?}", instruction.mnemonic()).to_ascii_lowercase()
}

fn canonical(source_mnemonic: &str) -> &str {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == source_mnemonic)
        .map_or(source_mnemonic, |(_, canonical)| canonical)
}

/// The instruction as the decoder reads it, for messages.
#[must_use]
pub fn display(instruction: &Instruction) -> String {
    let mut formatter = IntelFormatter::new();
    formatter
        .options_mut()
        .set_space_after_operand_separator(true);
    let mut text = String::new();
    formatter.format(instruction, &mut text);
    text
}

/// The CPU features an instruction needs beyond the x86-64 baseline, checked against this CPU.
fn missing_feature(instruction: &Instruction, features: CpuFeatures) -> Option<&'static str> {
    instruction
        .cpuid_features()
        .iter()
        .find_map(|feature| match feature {
            CpuidFeature::INTEL8086
            | CpuidFeature::INTEL186
            | CpuidFeature::INTEL286
            | CpuidFeature::INTEL386
            | CpuidFeature::INTEL486
            | CpuidFeature::X64
            | CpuidFeature::CMOV
            | CpuidFeature::SYSCALL => None,
            CpuidFeature::BMI1 => (!features.bmi1).then_some("BMI1"),
            CpuidFeature::BMI2 => (!features.bmi2).then_some("BMI2"),
            CpuidFeature::POPCNT => (!features.popcnt).then_some("POPCNT"),
            CpuidFeature::LZCNT => (!features.lzcnt).then_some("LZCNT"),
            CpuidFeature::ADX => (!features.adx).then_some("ADX"),
            CpuidFeature::MOVBE => (!features.movbe).then_some("MOVBE"),
            _ => Some("a feature outside dialect 1"),
        })
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
        source,
        functions,
        data: machine.data.clone(),
    };
    for function in &program.functions {
        for step in &function.steps {
            if let Err(problem) = same_instruction(step, function, &program, machine) {
                violations.push(Violation {
                    rule: Rule::Dialect,
                    line: Some(step.source.line),
                    problem: format!(
                        "`{}` became `{}` in the machine code: {problem}",
                        step.source.text,
                        display(&step.machine)
                    ),
                    fix: "write the instruction so it means what the machine does; constants \
                          must come from the block at the top and data must be addressed as \
                          [rip + name]"
                        .into(),
                });
            }
            check_instruction(step, features, violations);
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
    let first_difference = sources
        .iter()
        .zip(&code.instructions)
        .position(|(source, machine)| canonical(&source.mnemonic) != mnemonic(machine));
    if sources.len() != code.instructions.len() || first_difference.is_some() {
        let index = first_difference.unwrap_or(sources.len().min(code.instructions.len()));
        let (line, found) = match (sources.get(index), code.instructions.get(index)) {
            (Some(source), Some(machine)) => (
                source.line,
                format!("`{}` became `{}`", source.text, display(machine)),
            ),
            (Some(source), None) => (
                source.line,
                format!("`{}` has no machine instruction", source.text),
            ),
            (None, Some(machine)) => (
                function.line,
                format!("the machine code has an extra `{}`", display(machine)),
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
    let steps: Vec<Step<'a>> = sources
        .into_iter()
        .zip(&code.instructions)
        .map(|(source, machine)| Step {
            source,
            machine: *machine,
        })
        .collect();
    let end = code.address + code.size;
    let mut labels = Vec::new();
    let mut index = 0;
    for item in &function.items {
        match item {
            Item::Instruction(_) => index += 1,
            Item::Label(label) => labels.push(LabelAt {
                label,
                index,
                address: steps.get(index).map_or(end, |step| step.machine.ip()),
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

/// Compares each operand as written with the operand the decoder found.
fn same_instruction(
    step: &Step<'_>,
    function: &MappedFunction<'_>,
    program: &Program<'_>,
    machine: &Machine,
) -> Result<(), String> {
    let instruction = &step.machine;
    let count = instruction.op_count();
    if usize::try_from(count).ok() != Some(step.source.operands.len()) {
        return Err(format!(
            "it has {} operands as written and {count} in the machine code",
            step.source.operands.len()
        ));
    }
    for (index, operand) in (0..count).zip(&step.source.operands) {
        let kind = instruction.op_kind(index);
        let position = index + 1;
        match operand {
            Operand::Register(register) => {
                if kind != OpKind::Register || instruction.op_register(index) != *register {
                    return Err(format!(
                        "operand {position} isn't the register {register:?}"
                    ));
                }
            }
            Operand::Immediate { value, .. } => {
                let bits = immediate_bits(kind).ok_or_else(|| {
                    format!("operand {position} isn't a number in the machine code")
                })?;
                let mask = if bits == 64 {
                    u64::MAX
                } else {
                    (1_u64 << bits) - 1
                };
                // Reinterpret the value's bits, as the assembler does.
                let written = u64::from_ne_bytes(value.to_ne_bytes());
                if written & mask != instruction.immediate(index) & mask {
                    return Err(format!(
                        "operand {position} is {value} as written but {} in the machine code",
                        instruction.immediate(index)
                    ));
                }
            }
            Operand::Memory(memory) => {
                if kind != OpKind::Memory {
                    return Err(format!(
                        "operand {position} isn't memory in the machine code"
                    ));
                }
                same_memory(memory, instruction, program)
                    .map_err(|problem| format!("operand {position}: {problem}"))?;
            }
            Operand::Target(name) => {
                if !matches!(
                    kind,
                    OpKind::NearBranch16 | OpKind::NearBranch32 | OpKind::NearBranch64
                ) {
                    return Err(format!(
                        "operand {position} isn't a jump target in the machine code"
                    ));
                }
                let expected = function
                    .labels
                    .iter()
                    .find(|label| label.label.name == *name)
                    .map(|label| label.address)
                    .or_else(|| machine.functions.get(name).map(|code| code.address))
                    .ok_or_else(|| {
                        format!(
                            "`{name}` isn't a label in {} or a function",
                            function.name()
                        )
                    })?;
                if instruction.near_branch_target() != expected {
                    return Err(format!("operand {position} doesn't land on {name}"));
                }
            }
        }
    }
    Ok(())
}

fn same_memory(
    memory: &Memory,
    instruction: &Instruction,
    program: &Program<'_>,
) -> Result<(), String> {
    let base = memory.base.unwrap_or(Register::None);
    let index = memory.index.unwrap_or(Register::None);
    if instruction.memory_base() != base {
        return Err(format!(
            "the base register is {:?} in the machine code",
            instruction.memory_base()
        ));
    }
    if instruction.memory_index() != index
        || (memory.index.is_some() && instruction.memory_index_scale() != memory.scale)
    {
        return Err("the index or scale differs in the machine code".into());
    }
    let displacement = u64::from_ne_bytes(memory.displacement.to_ne_bytes());
    let expected = match &memory.symbol {
        Some(symbol) => program
            .data
            .get(symbol)
            .map(|data| data.address.wrapping_add(displacement))
            .ok_or_else(|| format!("`{symbol}` isn't a data symbol"))?,
        None => displacement,
    };
    if instruction.memory_displacement64() != expected {
        return Err("the address differs in the machine code".into());
    }
    if let Some(size) = memory.size {
        let decoded = instruction.memory_size();
        if decoded != MemorySize::Unknown && decoded.size() != usize::from(size) {
            return Err(format!(
                "it accesses {} bytes in the machine code",
                decoded.size()
            ));
        }
    }
    Ok(())
}

fn immediate_bits(kind: OpKind) -> Option<u32> {
    match kind {
        OpKind::Immediate8 | OpKind::Immediate8_2nd => Some(8),
        OpKind::Immediate16 | OpKind::Immediate8to16 => Some(16),
        OpKind::Immediate32 | OpKind::Immediate8to32 => Some(32),
        OpKind::Immediate64 | OpKind::Immediate8to64 | OpKind::Immediate32to64 => Some(64),
        _ => None,
    }
}

/// Checks the machine instruction itself: the instruction set, CPU features, prefixes and
/// addressing.
fn check_instruction(step: &Step<'_>, features: CpuFeatures, violations: &mut Vec<Violation>) {
    let instruction = &step.machine;
    let line = Some(step.source.line);
    let name = mnemonic(instruction);
    if !ALLOWED.contains(&name.as_str()) {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem: format!("`{name}` isn't in dialect 1, which allows integer instructions only"),
            fix: "use the integer instructions listed in references/power-of-10.md; vector \
                  instructions come in dialect 2"
                .into(),
        });
        return;
    }
    if let Some(feature) = missing_feature(instruction, features) {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem: format!("`{name}` needs {feature}, which this CPU doesn't have"),
            fix: "use baseline x86-64 instructions instead".into(),
        });
    }
    if instruction.segment_prefix() != Register::None {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem: "segment registers aren't in the dialect".into(),
            fix: "address memory through general-purpose registers or [rip + name]".into(),
        });
    }
    if matches!(
        instruction.flow_control(),
        FlowControl::IndirectBranch | FlowControl::IndirectCall
    ) {
        violations.push(Violation {
            rule: Rule::Indirection,
            line,
            problem: format!("`{}` jumps through a register or memory", step.source.text),
            fix: "jump and call only to labels and function names; choose between targets with \
                  compare-and-jump chains"
                .into(),
        });
    }
    let has_memory =
        (0..instruction.op_count()).any(|index| instruction.op_kind(index) == OpKind::Memory);
    if has_memory
        && instruction.memory_base() == Register::None
        && !instruction.is_ip_rel_memory_operand()
    {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem: format!(
                "`{}` reads or writes a fixed address ({})",
                step.source.text,
                display(instruction)
            ),
            fix: "use a register base or [rip + name]; a constant used before its .equ or a \
                  `. - name` turns into a memory address"
                .into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(bytes: &[u8]) -> Instruction {
        Decoder::with_ip(64, bytes, 0x1000, DecoderOptions::NONE).decode()
    }

    #[test]
    fn iced_treats_syscall_as_a_call() {
        // The control-flow graph relies on this: syscall returns unless it's an exit.
        let syscall = decode(&[0x0f, 0x05]);
        assert_eq!(mnemonic(&syscall), "syscall");
        assert_eq!(syscall.flow_control(), FlowControl::Call);
    }

    #[test]
    fn names_mnemonics_like_the_source() {
        assert_eq!(mnemonic(&decode(&[0x74, 0x00])), "je");
        assert_eq!(mnemonic(&decode(&[0x48, 0x99])), "cqo");
        assert_eq!(canonical("jz"), "je");
        assert_eq!(canonical("sal"), "shl");
        assert_eq!(canonical("mov"), "mov");
    }

    #[test]
    fn gates_integer_extensions_on_the_cpu() {
        // shlx rax, rbx, rcx
        let shlx = decode(&[0xc4, 0xe2, 0xf1, 0xf7, 0xc3]);
        assert_eq!(mnemonic(&shlx), "shlx");
        assert_eq!(missing_feature(&shlx, CpuFeatures::all()), None);
        let without_bmi2 = CpuFeatures {
            bmi2: false,
            ..CpuFeatures::all()
        };
        assert_eq!(missing_feature(&shlx, without_bmi2), Some("BMI2"));
        assert_eq!(
            missing_feature(&decode(&[0x0f, 0x05]), CpuFeatures::default()),
            None
        );
    }
}
