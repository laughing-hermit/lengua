//! Reads main.s: the dialect's layout rules, the constants, the data symbols, and the functions
//! with their instructions and operands.
//!
//! The layout rules make every source line map to exactly one machine instruction: one
//! instruction per line, no statement separators, no macros, constants defined before use.

use std::collections::BTreeMap;

use iced_x86::Register;

use crate::isa::aarch64::{self, A64Cond};
use crate::isa::{Isa, Reg};

use super::{Rule, Violation};

/// What a section holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    /// Functions.
    Text,
    /// Read-only data.
    ReadOnly,
    /// Writable data with initial values.
    Data,
    /// Writable data that starts as zeros.
    Bss,
}

impl SectionKind {
    /// Whether programs may write to it.
    #[must_use]
    pub fn writable(self) -> bool {
        matches!(self, Self::Data | Self::Bss)
    }
}

/// A named constant from the header block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constant {
    /// Its value.
    pub value: i64,
    /// Line of its `.equ`.
    pub line: usize,
}

/// A data symbol, between `.type x, @object` and `.size x, . - x`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSymbol {
    /// Its name.
    pub name: String,
    /// Its section.
    pub section: SectionKind,
    /// Line of its label.
    pub line: usize,
    /// Whether `# shared:` lets several functions use it.
    pub shared: bool,
    /// Its lines from `.type` to `.size`, without comments or extra spaces.
    pub body: Vec<String>,
    /// For a jump table, the local label each entry names (`.long .Lcase - table`), in order:
    /// empty for any other data.
    pub table: Vec<String>,
}

/// A function, between `.type f, @function` and `.size f, . - f`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// Its name.
    pub name: String,
    /// Line of its label.
    pub line: usize,
    /// Whether `# returns: result` marks it: rax < 0 means an error.
    pub returns_result: bool,
    /// The part it was copied from, word for word, when `# from: ../sort` says so.
    pub from: Option<String>,
    /// Its labels and instructions, in order.
    pub items: Vec<Item>,
    /// Its lines from `.type` to `.size`, without comments or extra spaces.
    pub body: Vec<String>,
    /// Its `# unsafe:` blocks.
    pub unsafe_blocks: Vec<UnsafeBlock>,
}

impl Function {
    /// The instructions, in order.
    pub fn instructions(&self) -> impl Iterator<Item = &Instruction> {
        self.items.iter().filter_map(|item| match item {
            Item::Instruction(instruction) => Some(instruction),
            Item::Label(_) => None,
        })
    }
}

/// A label or an instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// A local label.
    Label(Label),
    /// An instruction.
    Instruction(Instruction),
}

/// A local label such as `.Lloop:`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    /// Its name.
    pub name: String,
    /// Its line.
    pub line: usize,
    /// The `# bound: N` written after it, for loop heads.
    pub bound: Option<u64>,
    /// Whether it says `# bound: forever`: a server's loop that takes connections until the
    /// program is stopped.
    pub forever: bool,
}

/// One instruction line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    /// Its line.
    pub line: usize,
    /// The mnemonic in lowercase.
    pub mnemonic: String,
    /// The operands, in order.
    pub operands: Vec<Operand>,
    /// The AVX-512 write mask on the first operand: `{k1}` to `{k7}`.
    pub mask: Option<Reg>,
    /// Whether the mask zeroes the lanes it leaves out (`{z}`) instead of keeping them.
    pub zeroing: bool,
    /// The line's code without its comment.
    pub text: String,
    /// The `# unsafe:` block it's in, by index.
    pub unsafe_block: Option<usize>,
    /// On AArch64, when the line is a `movk` that adds to the `mov` or `movk` right before it,
    /// what they put in the register together.
    pub pieces: Option<Pieces>,
}

/// What a `mov` and the `movk`s right after it put in their register, read at the last of them:
/// AArch64 builds a number 16 bits at a time, as `mov x9, #BIG & 0xffff` and then
/// `movk x9, #BIG >> 16, lsl #16`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pieces {
    /// The number the register holds after the line.
    pub value: i64,
    /// The one constant every line names, when they all name the same one.
    pub mentions: Option<String>,
    /// That constant, when the number is all of it: then the lines together are `mov xR, NAME`.
    pub name: Option<String>,
}

/// An instruction operand as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    /// A register.
    Register(Reg),
    /// A number: a literal, a constant, or an expression of them.
    Immediate {
        /// Its value.
        value: i64,
        /// The constant's name, when the operand is exactly one constant.
        name: Option<String>,
    },
    /// A memory operand in brackets.
    Memory(Memory),
    /// A jump or call target, or a data symbol `adrp` names.
    Target(String),
    /// A shift, extend or condition, or a floating-point number, as written: AArch64's `lsl #12`,
    /// `asr #63`, `eq`, `#1.5`.
    Modifier(String),
}

/// A memory operand such as `qword ptr [rbx + rcx*8 + 16]` or `[rip + msg]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Memory {
    /// The access size in bytes, when written (`byte ptr` is 1).
    pub size: Option<u8>,
    /// The base register; `RIP` for `[rip + symbol]`.
    pub base: Option<Reg>,
    /// The index register.
    pub index: Option<Reg>,
    /// The index's scale: 1, 2, 4 or 8.
    pub scale: u32,
    /// The constant displacement.
    pub displacement: i64,
    /// The data symbol of a `[rip + symbol]` operand.
    pub symbol: Option<String>,
    /// Whether one element is repeated across the vector (`{1to16}`).
    pub broadcast: bool,
}

/// An `# unsafe:` … `# end unsafe` block, where bounds checks may be left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsafeBlock {
    /// Line of `# unsafe:`.
    pub start: usize,
    /// Line of `# end unsafe`.
    pub end: usize,
    /// Why it's safe.
    pub reason: String,
}

/// A parsed main.s.
#[derive(Debug, Default)]
pub struct Source {
    /// The CPU it's written for, as its first line says.
    pub isa: Isa,
    /// The constants, by name.
    pub constants: BTreeMap<String, Constant>,
    /// The data symbols, in order.
    pub data: Vec<DataSymbol>,
    /// The functions, in order.
    pub functions: Vec<Function>,
}

impl Source {
    /// A function by name.
    #[must_use]
    pub fn function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().find(|function| function.name == name)
    }
}

/// General-purpose registers by name.
const REGISTERS: [(&str, Register); 69] = [
    ("rax", Register::RAX),
    ("rbx", Register::RBX),
    ("rcx", Register::RCX),
    ("rdx", Register::RDX),
    ("rsi", Register::RSI),
    ("rdi", Register::RDI),
    ("rbp", Register::RBP),
    ("rsp", Register::RSP),
    ("r8", Register::R8),
    ("r9", Register::R9),
    ("r10", Register::R10),
    ("r11", Register::R11),
    ("r12", Register::R12),
    ("r13", Register::R13),
    ("r14", Register::R14),
    ("r15", Register::R15),
    ("eax", Register::EAX),
    ("ebx", Register::EBX),
    ("ecx", Register::ECX),
    ("edx", Register::EDX),
    ("esi", Register::ESI),
    ("edi", Register::EDI),
    ("ebp", Register::EBP),
    ("esp", Register::ESP),
    ("r8d", Register::R8D),
    ("r9d", Register::R9D),
    ("r10d", Register::R10D),
    ("r11d", Register::R11D),
    ("r12d", Register::R12D),
    ("r13d", Register::R13D),
    ("r14d", Register::R14D),
    ("r15d", Register::R15D),
    ("ax", Register::AX),
    ("bx", Register::BX),
    ("cx", Register::CX),
    ("dx", Register::DX),
    ("si", Register::SI),
    ("di", Register::DI),
    ("bp", Register::BP),
    ("sp", Register::SP),
    ("r8w", Register::R8W),
    ("r9w", Register::R9W),
    ("r10w", Register::R10W),
    ("r11w", Register::R11W),
    ("r12w", Register::R12W),
    ("r13w", Register::R13W),
    ("r14w", Register::R14W),
    ("r15w", Register::R15W),
    ("al", Register::AL),
    ("bl", Register::BL),
    ("cl", Register::CL),
    ("dl", Register::DL),
    ("sil", Register::SIL),
    ("dil", Register::DIL),
    ("bpl", Register::BPL),
    ("spl", Register::SPL),
    ("r8b", Register::R8L),
    ("r9b", Register::R9L),
    ("r10b", Register::R10L),
    ("r11b", Register::R11L),
    ("r12b", Register::R12L),
    ("r13b", Register::R13L),
    ("r14b", Register::R14L),
    ("r15b", Register::R15L),
    ("ah", Register::AH),
    ("bh", Register::BH),
    ("ch", Register::CH),
    ("dh", Register::DH),
    ("rip", Register::RIP),
];

/// The instruction pointer, which `[rip + symbol]` addresses data relative to.
const RIP: Reg = Reg::X86(Register::RIP);

/// The register with this name, in any case: a general-purpose register, a vector register
/// (`xmm0`–`xmm31`, `ymm0`–`ymm31`, `zmm0`–`zmm31`) or an AVX-512 mask register (`k0`–`k7`).
#[must_use]
pub fn register(name: &str) -> Option<Register> {
    let name = name.to_ascii_lowercase();
    REGISTERS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, register)| *register)
        .or_else(|| vector_register(&name))
}

fn vector_register(name: &str) -> Option<Register> {
    let (bank, number) = ["xmm", "ymm", "zmm", "k"]
        .into_iter()
        .find_map(|bank| Some((bank, name.strip_prefix(bank)?)))?;
    let limit = if bank == "k" { 7 } else { 31 };
    let valid = !number.is_empty()
        && number.bytes().all(|byte| byte.is_ascii_digit())
        && (number == "0" || !number.starts_with('0'))
        && number.parse::<u32>().is_ok_and(|value| value <= limit);
    if !valid {
        return None;
    }
    Register::values().find(|register| format!("{register:?}").eq_ignore_ascii_case(name))
}

/// Words that would add prefixes to an instruction.
const PREFIXES: [&str; 12] = [
    "rep", "repe", "repz", "repne", "repnz", "lock", "data16", "data32", "addr32", "notrack",
    "xacquire", "xrelease",
];

/// Whether the mnemonic's operands are jump or call targets.
fn is_branch(mnemonic: &str) -> bool {
    mnemonic == "call" || mnemonic.starts_with('j') || mnemonic.starts_with("loop")
}

/// Splits code from its `#` comment, ignoring `#` inside string and character literals.
fn split_comment(line: &str, isa: Isa) -> (&str, Option<&str>) {
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'/' if isa == Isa::Aarch64 && bytes.get(index + 1) == Some(&b'/') => {
                return (&line[..index], Some(&line[index + 2..]));
            }
            b'"' => {
                index += 1;
                while index < bytes.len() && bytes[index] != b'"' {
                    index += if bytes[index] == b'\\' { 2 } else { 1 };
                }
                index += 1;
            }
            b'\'' => index += 3,
            b'#' if isa == Isa::X86_64 => return (&line[..index], Some(&line[index + 1..])),
            _ => index += 1,
        }
    }
    (line, None)
}

/// Whether `text` has `needle` outside string and character literals.
fn contains_outside_literals(text: &str, needle: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                index += 1;
                while index < bytes.len() && bytes[index] != b'"' {
                    index += if bytes[index] == b'\\' { 2 } else { 1 };
                }
                index += 1;
            }
            b'\'' => index += 3,
            _ if text[index..].starts_with(needle) => return true,
            _ => index += 1,
        }
    }
    false
}

/// Splits on commas outside brackets, parentheses and literals.
fn split_operands(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0_i32;
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                index += 1;
                while index < bytes.len() && bytes[index] != b'"' {
                    index += if bytes[index] == b'\\' { 2 } else { 1 };
                }
            }
            b'\'' => index += 2,
            b'[' | b'(' => depth += 1,
            b']' | b')' => depth -= 1,
            b',' if depth == 0 => {
                parts.push(text[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    let last = text[start..].trim();
    if !last.is_empty() || !parts.is_empty() {
        parts.push(last);
    }
    parts
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn is_local_label(text: &str) -> bool {
    text.strip_prefix(".L").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
    })
}

fn is_constant_name(text: &str) -> bool {
    is_identifier(text)
        && text.starts_with(|first: char| first.is_ascii_uppercase())
        && !text.chars().any(|character| character.is_ascii_lowercase())
}

/// Evaluates an integer expression: numbers, `'c'` characters and constants, with `+ - * /`,
/// `& |`, `<< >>` and parentheses, as llvm-mc reads them: `* / << >>` first, then `& |`, then
/// `+ -`, each from left to right, and `>>` shifting in zeros. A value that doesn't fit in 64
/// bits, or a shift by less than 0 or more than 63, is refused.
pub(crate) fn evaluate(text: &str, constants: &BTreeMap<String, Constant>) -> Result<i64, String> {
    struct Expression<'a> {
        tokens: Vec<&'a str>,
        position: usize,
        constants: &'a BTreeMap<String, Constant>,
    }
    impl<'a> Expression<'a> {
        fn peek(&self) -> Option<&'a str> {
            self.tokens.get(self.position).copied()
        }
        fn sum(&mut self) -> Result<i64, String> {
            let mut value = self.bits()?;
            while let Some(op @ ("+" | "-")) = self.peek() {
                self.position += 1;
                let right = self.bits()?;
                value = if op == "+" {
                    value.checked_add(right)
                } else {
                    value.checked_sub(right)
                }
                .ok_or("the value doesn't fit in 64 bits")?;
            }
            Ok(value)
        }
        fn bits(&mut self) -> Result<i64, String> {
            let mut value = self.product()?;
            while let Some(op @ ("&" | "|")) = self.peek() {
                self.position += 1;
                let right = self.product()?;
                value = if op == "&" {
                    value & right
                } else {
                    value | right
                };
            }
            Ok(value)
        }
        fn product(&mut self) -> Result<i64, String> {
            let mut value = self.unary()?;
            while let Some(op @ ("*" | "/" | "<<" | ">>")) = self.peek() {
                self.position += 1;
                let right = self.unary()?;
                let shift = || {
                    u32::try_from(right)
                        .ok()
                        .filter(|bits| *bits < 64)
                        .ok_or_else(|| format!("it shifts by {right}, outside 0 to 63"))
                };
                value = match op {
                    "*" => value.checked_mul(right),
                    "/" if right == 0 => return Err("it divides by zero".into()),
                    "/" => value.checked_div(right),
                    // Only when shifting back gives the value again do no bits fall off the top.
                    "<<" => {
                        let bits = shift()?;
                        Some(value << bits).filter(|shifted| shifted >> bits == value)
                    }
                    _ => Some((value.cast_unsigned() >> shift()?).cast_signed()),
                }
                .ok_or("the value doesn't fit in 64 bits")?;
            }
            Ok(value)
        }
        fn unary(&mut self) -> Result<i64, String> {
            if self.peek() == Some("-") {
                self.position += 1;
                return self
                    .unary()?
                    .checked_neg()
                    .ok_or_else(|| "the value doesn't fit in 64 bits".into());
            }
            self.primary()
        }
        fn primary(&mut self) -> Result<i64, String> {
            let token = self.peek().ok_or("the expression ends too early")?;
            self.position += 1;
            if token == "(" {
                let value = self.sum()?;
                if self.peek() != Some(")") {
                    return Err("a ( is missing its )".into());
                }
                self.position += 1;
                return Ok(value);
            }
            if let Some(hex) = token
                .strip_prefix("0x")
                .or_else(|| token.strip_prefix("0X"))
            {
                return i64::from_str_radix(hex, 16)
                    .map_err(|_| format!("`{token}` isn't a number"));
            }
            if token.starts_with(|first: char| first.is_ascii_digit()) {
                return token
                    .parse()
                    .map_err(|_| format!("`{token}` isn't a number"));
            }
            if let Some(inner) = token
                .strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
            {
                let mut chars = inner.chars();
                return match (chars.next(), chars.next()) {
                    (Some(character), None)
                        if (' '..='~').contains(&character)
                            && !matches!(character, '\'' | '\\') =>
                    {
                        Ok(i64::from(u32::from(character)))
                    }
                    _ => Err(format!(
                        "{token} isn't a character literal; write one printable character in \
                         quotes, such as '0'"
                    )),
                };
            }
            if let Some(constant) = self.constants.get(token) {
                return Ok(constant.value);
            }
            if is_identifier(token) {
                return Err(format!(
                    "`{token}` isn't a constant defined above; numbers here must be literals or \
                     constants from the block at the top"
                ));
            }
            Err(format!("unexpected `{token}`"))
        }
    }
    let mut tokens = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let length = if let Some(after_quote) = rest.strip_prefix('\'') {
            after_quote.find('\'').map_or(rest.len(), |end| end + 2)
        } else if rest.starts_with(|first: char| first.is_ascii_alphanumeric() || first == '_') {
            rest.find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
                .unwrap_or(rest.len())
        } else if rest.starts_with("<<") || rest.starts_with(">>") {
            2
        } else {
            rest.chars().next().map_or(1, char::len_utf8)
        };
        tokens.push(&rest[..length]);
        rest = rest[length..].trim_start();
    }
    let mut expression = Expression {
        tokens,
        position: 0,
        constants,
    };
    let value = expression.sum()?;
    match expression.peek() {
        None => Ok(value),
        Some(token) => Err(format!("unexpected `{token}`")),
    }
}

/// The one constant an operand's expression names, if it names exactly one.
fn mentioned(text: &str, constants: &BTreeMap<String, Constant>) -> Option<String> {
    let mut names = text
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|word| constants.contains_key(*word));
    let first = names.next()?;
    names.all(|other| other == first).then(|| first.to_owned())
}

/// AVX-512 decorations written after an operand.
#[derive(Debug, Default)]
struct Decorations {
    mask: Option<Reg>,
    zeroing: bool,
    broadcast: bool,
}

/// Splits `zmm0 {k1} {z}` or `dword ptr [rax]{1to16}` into the operand and its decorations.
/// Write masks and zeroing belong to the first operand.
fn decorations(text: &str, position: usize) -> Result<(&str, Decorations), String> {
    let mut rest = text.trim_end();
    let mut found = Decorations::default();
    while let Some(inner) = rest.strip_suffix('}') {
        let open = inner.rfind('{').ok_or("a `}` has no `{` before it")?;
        let decoration = inner[open + 1..].trim().to_ascii_lowercase();
        rest = inner[..open].trim_end();
        let mask = register(&decoration)
            .filter(|register| (Register::K1..=Register::K7).contains(register));
        match decoration.as_str() {
            "z" => found.zeroing = true,
            "k0" => return Err("k0 can't be a write mask; use k1 to k7".into()),
            _ if mask.is_some() => found.mask = mask.map(Reg::X86),
            "1to2" | "1to4" | "1to8" | "1to16" | "1to32" | "1to64" => found.broadcast = true,
            "rn-sae" | "rd-sae" | "ru-sae" | "rz-sae" | "sae" => {
                return Err(
                    "rounding overrides ({rn-sae}) aren't in the dialect: they change how \
                            results round"
                        .into(),
                );
            }
            other => return Err(format!("`{{{other}}}` isn't a decoration lengua knows")),
        }
    }
    if (found.mask.is_some() || found.zeroing) && position != 0 {
        return Err("a write mask ({k1}) and {z} go on the first operand".into());
    }
    if found.zeroing && found.mask.is_none() {
        return Err("{z} needs a write mask before it, such as {k1} {z}".into());
    }
    Ok((rest, found))
}

/// Parses one operand. Branches take label names; everything else names registers, numbers or
/// memory.
fn operand(
    text: &str,
    branch: bool,
    constants: &BTreeMap<String, Constant>,
) -> Result<Operand, String> {
    let lower = text.to_ascii_lowercase();
    if text.contains('{') || text.contains('}') {
        return Err(
            "braces go only at the end of an operand: {k1} and {z} after the first \
                    operand, {1to16} after a memory operand"
                .into(),
        );
    }
    if lower.split_whitespace().any(|word| word == "offset") {
        return Err(
            "`offset` isn't in the dialect; load addresses with lea reg, [rip + symbol]".into(),
        );
    }
    if lower.len() == 3
        && lower.starts_with("mm")
        && lower.ends_with(|c: char| ('0'..='7').contains(&c))
    {
        return Err("MMX registers aren't in the dialect; use xmm registers".into());
    }
    if let Some(register) = register(text) {
        if register == Register::RIP {
            return Err("rip can only appear inside [rip + symbol]".into());
        }
        return Ok(Operand::Register(Reg::X86(register)));
    }
    if text.contains('[') || lower.contains(" ptr") {
        return memory(text, constants).map(Operand::Memory);
    }
    if branch && (is_identifier(text) || is_local_label(text)) {
        return Ok(Operand::Target(text.to_owned()));
    }
    if is_identifier(text) && !constants.contains_key(text) {
        return Err(format!(
            "`{text}` isn't a constant: data is read with [rip + {text}], its address taken \
             with lea reg, [rip + {text}], and only jumps and calls name labels"
        ));
    }
    let value = evaluate(text, constants)?;
    let name = constants.contains_key(text).then(|| text.to_owned());
    Ok(Operand::Immediate { value, name })
}

fn memory(text: &str, constants: &BTreeMap<String, Constant>) -> Result<Memory, String> {
    let trimmed = text.trim();
    let bracket = trimmed
        .find('[')
        .ok_or("write memory operands as [ … ], with a size such as qword ptr before them")?;
    let prefix = trimmed[..bracket].trim().to_ascii_lowercase();
    let size = match prefix.as_str() {
        "" => None,
        "byte ptr" => Some(1),
        "word ptr" => Some(2),
        "dword ptr" => Some(4),
        "qword ptr" => Some(8),
        "xmmword ptr" => Some(16),
        "ymmword ptr" => Some(32),
        "zmmword ptr" => Some(64),
        other if other.contains(':') => {
            return Err("segment registers aren't in the dialect".into());
        }
        other => {
            return Err(format!(
                "`{other}` isn't a size; use byte, word, dword, qword, xmmword (16 bytes), \
                 ymmword (32) or zmmword ptr (64)"
            ));
        }
    };
    let rest = &trimmed[bracket..];
    let inner = rest
        .trim()
        .strip_prefix('[')
        .and_then(|inside| inside.strip_suffix(']'))
        .ok_or("write memory operands as [ … ], with nothing outside the brackets")?;
    if inner.contains(':') {
        return Err("segment registers aren't in the dialect".into());
    }
    let mut memory = Memory {
        size,
        base: None,
        index: None,
        scale: 1,
        displacement: 0,
        symbol: None,
        broadcast: false,
    };
    let mut constant_terms = String::new();
    for (negative, term) in signed_terms(inner) {
        let term = term.trim();
        if let Some(register) = register(term) {
            if negative {
                return Err(format!("registers can't be subtracted (`- {term}`)"));
            }
            if memory.base.is_none() {
                memory.base = Some(Reg::X86(register));
            } else if memory.index.is_none() {
                memory.index = Some(Reg::X86(register));
            } else {
                return Err("a memory operand has at most a base and an index register".into());
            }
            continue;
        }
        if let Some((left, right)) = term.split_once('*') {
            let (left, right) = (left.trim(), right.trim());
            let scaled = register(left)
                .map(|index| (index, right))
                .or_else(|| register(right).map(|index| (index, left)));
            if let Some((index, factor)) = scaled {
                if negative || memory.index.is_some() {
                    return Err("a memory operand has one index register, added".into());
                }
                let scale = evaluate(factor, constants)?;
                memory.scale = match scale {
                    1 | 2 | 4 | 8 => u32::try_from(scale).unwrap_or(1),
                    _ => return Err(format!("an index scale is 1, 2, 4 or 8, not {scale}")),
                };
                memory.index = Some(Reg::X86(index));
                continue;
            }
        }
        if is_identifier(term) && !constants.contains_key(term) {
            if negative || memory.symbol.is_some() {
                return Err(format!(
                    "`{term}` can only be added, once, as [rip + {term}]"
                ));
            }
            memory.symbol = Some(term.to_owned());
            continue;
        }
        constant_terms.push_str(if negative { " - (" } else { " + (" });
        constant_terms.push_str(term);
        constant_terms.push(')');
    }
    if !constant_terms.is_empty() {
        memory.displacement = evaluate(&format!("0{constant_terms}"), constants)?;
    }
    if memory.base == Some(RIP) && memory.symbol.is_none() {
        return Err("[rip + …] must name a data symbol".into());
    }
    if memory.symbol.is_some() && memory.base != Some(RIP) {
        return Err(format!(
            "address data relative to rip: [rip + {}]",
            memory.symbol.as_deref().unwrap_or_default()
        ));
    }
    if memory.index == Some(RIP) || (memory.base == Some(RIP) && memory.index.is_some()) {
        return Err("[rip + symbol] can't have an index register".into());
    }
    if memory.base.is_none() && memory.index.is_none() {
        return Err(
            "a memory operand needs a base register; absolute addresses aren't in the dialect"
                .into(),
        );
    }
    Ok(memory)
}

/// Splits `a + b - c` into signed terms, outside parentheses.
fn signed_terms(text: &str) -> Vec<(bool, &str)> {
    let mut terms = Vec::new();
    let mut depth = 0_i32;
    let mut start = 0;
    let mut negative = false;
    for (index, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth -= 1,
            '+' | '-' if depth == 0 && !text[start..index].trim().is_empty() => {
                terms.push((negative, &text[start..index]));
                negative = character == '-';
                start = index + 1;
            }
            '-' if depth == 0 => {
                negative = !negative;
                start = index + 1;
            }
            _ => {}
        }
    }
    terms.push((negative, &text[start..]));
    terms
}

/// Collapses a code line for word-for-word comparison.
fn normalize(code: &str) -> String {
    code.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Header,
    Section(SectionKind),
}

#[derive(Debug)]
struct OpenFunction {
    function: Function,
    open_unsafe: Option<(usize, String)>,
}

#[derive(Debug)]
struct Parser {
    source: Source,
    violations: Vec<Violation>,
    place: Place,
    pending_returns: Option<usize>,
    pending_from: Option<String>,
    pending_shared: Option<usize>,
    pending_type: Option<(String, bool, usize)>,
    function: Option<OpenFunction>,
    data: Option<DataSymbol>,
    globals: Vec<(String, usize)>,
    /// Whether the first statement, the line that names the CPU, has been read.
    seen_header: bool,
}

impl Parser {
    fn violation(
        &mut self,
        rule: Rule,
        line: usize,
        problem: impl Into<String>,
        fix: impl Into<String>,
    ) {
        self.violations.push(Violation {
            rule,
            line: Some(line),
            problem: problem.into(),
            fix: fix.into(),
        });
    }

    fn dialect(&mut self, line: usize, problem: impl Into<String>, fix: impl Into<String>) {
        self.violation(Rule::Dialect, line, problem, fix);
    }

    fn line(&mut self, number: usize, raw: &str) {
        let isa = self.source.isa;
        let (code, comment) = split_comment(raw, isa);
        let code = code.trim();
        if isa == Isa::Aarch64 && code.starts_with('#') {
            self.dialect(
                number,
                "`#` comments aren't in the AArch64 dialect, where `#` starts a number",
                "start comments with //",
            );
            return;
        }
        if contains_outside_literals(code, ";") {
            self.dialect(
                number,
                "`;` puts several statements on one line",
                "write one instruction per line",
            );
            return;
        }
        if contains_outside_literals(raw, "/*") {
            self.dialect(
                number,
                "`/* */` comments aren't in the dialect",
                if isa == Isa::X86_64 {
                    "start comments with #"
                } else {
                    "start comments with //"
                },
            );
            return;
        }
        // Comments may come first; the first statement names the CPU.
        if !self.seen_header && !code.is_empty() {
            self.seen_header = true;
            let header = header(isa);
            if code != header {
                self.dialect(
                    number,
                    format!("main.s must start with {header}"),
                    format!("make `{header}` the first line, before any directive or instruction"),
                );
            }
            if code.starts_with(".intel_syntax") || code.starts_with(".arch") {
                return;
            }
        }
        if code.is_empty() {
            if let Some(comment) = comment {
                self.comment(number, comment.trim());
            }
            return;
        }
        if let Some((label, rest)) = code.split_once(':')
            && !label.trim().contains(char::is_whitespace)
            && !label.contains(',')
        {
            if !rest.trim().is_empty() {
                self.dialect(
                    number,
                    "a label and an instruction share a line",
                    "put the label on its own line, then the instruction below it",
                );
                return;
            }
            self.label(number, label.trim(), comment);
            return;
        }
        if code.starts_with('.') {
            self.directive(number, code);
            return;
        }
        self.instruction(number, code);
    }

    fn comment(&mut self, number: usize, comment: &str) {
        if comment == "returns: result" {
            self.pending_returns = Some(number);
        } else if let Some(part) = comment.strip_prefix("from:") {
            self.pending_from = Some(part.trim().to_owned());
        } else if let Some(reason) = comment.strip_prefix("shared:") {
            if reason.trim().is_empty() {
                self.violation(
                    Rule::Scope,
                    number,
                    "`# shared:` needs a reason",
                    "say why several functions may use this data, after `# shared:`",
                );
            }
            self.pending_shared = Some(number);
        } else if let Some(reason) = comment.strip_prefix("unsafe:") {
            self.start_unsafe(number, reason.trim());
        } else if comment == "end unsafe" {
            self.end_unsafe(number);
        }
    }

    fn start_unsafe(&mut self, number: usize, reason: &str) {
        match self
            .function
            .as_ref()
            .map(|open| open.open_unsafe.is_some())
        {
            None => {
                self.violation(
                    Rule::Indirection,
                    number,
                    "`# unsafe:` is outside a function",
                    "put unsafe blocks around instructions inside a function",
                );
                return;
            }
            Some(true) => {
                self.violation(
                    Rule::Indirection,
                    number,
                    "unsafe blocks can't nest",
                    "close the first block with `# end unsafe` before starting another",
                );
                return;
            }
            Some(false) => {}
        }
        if reason.is_empty() {
            self.violation(
                Rule::Indirection,
                number,
                "`# unsafe:` needs a reason",
                "say after `# unsafe:` why the unchecked accesses stay in bounds",
            );
        }
        if let Some(open) = self.function.as_mut() {
            open.open_unsafe = Some((number, reason.to_owned()));
        }
    }

    fn end_unsafe(&mut self, number: usize) {
        let closed = self.function.as_mut().and_then(|open| {
            let (start, reason) = open.open_unsafe.take()?;
            open.function.unsafe_blocks.push(UnsafeBlock {
                start,
                end: number,
                reason,
            });
            Some(())
        });
        if closed.is_none() {
            self.violation(
                Rule::Indirection,
                number,
                "`# end unsafe` has no `# unsafe:` before it",
                "start the block with `# unsafe: <reason>`",
            );
        }
    }

    fn label(&mut self, number: usize, name: &str, comment: Option<&str>) {
        let written = comment
            .map(str::trim)
            .and_then(|comment| comment.strip_prefix("bound:"))
            .map(str::trim);
        let forever = written == Some("forever");
        let bound = written.filter(|_| !forever).map(str::parse::<u64>);
        let bound = match bound {
            Some(Ok(bound)) => Some(bound),
            Some(Err(_)) => {
                self.violation(
                    Rule::Loops,
                    number,
                    "`# bound:` needs a whole number",
                    "write the loop's largest iteration count, such as `# bound: 64`",
                );
                None
            }
            None => None,
        };
        if is_local_label(name) {
            match self.function.as_mut() {
                Some(open) => open.function.items.push(Item::Label(Label {
                    name: name.to_owned(),
                    line: number,
                    bound,
                    forever,
                })),
                None => self.dialect(
                    number,
                    format!("local label {name} is outside a function"),
                    "put local labels inside functions",
                ),
            }
            return;
        }
        if name.starts_with(|first: char| first.is_ascii_digit()) {
            self.dialect(
                number,
                "numbered labels aren't in the dialect",
                "name labels .Lsomething inside functions",
            );
            return;
        }
        if !is_identifier(name) {
            self.dialect(
                number,
                format!("`{name}` isn't a valid label"),
                "name functions and data with letters, digits and _; local labels start with .L",
            );
            return;
        }
        let Some((typed, is_function, _)) = self.pending_type.take() else {
            self.dialect(
                number,
                format!("{name} has no .type line before it"),
                format!(
                    "write `.type {name}, @function` or `.type {name}, @object` right before it"
                ),
            );
            return;
        };
        if typed != name {
            self.dialect(
                number,
                format!("the .type above names {typed}, not {name}"),
                "give the label the name its .type line uses",
            );
            return;
        }
        if self.function.is_some() || self.data.is_some() {
            self.dialect(
                number,
                format!("{name} starts before the previous symbol's .size line"),
                "end each function or data symbol with `.size name, . - name`",
            );
            return;
        }
        if is_function {
            self.function = Some(OpenFunction {
                function: Function {
                    name: name.to_owned(),
                    line: number,
                    returns_result: self.pending_returns.take().is_some(),
                    from: self.pending_from.take(),
                    items: Vec::new(),
                    body: vec![
                        normalize(&format!(".type {name}, @function")),
                        format!("{name}:"),
                    ],
                    unsafe_blocks: Vec::new(),
                },
                open_unsafe: None,
            });
        } else {
            let section = match self.place {
                Place::Section(section) => section,
                Place::Header => SectionKind::ReadOnly,
            };
            self.data = Some(DataSymbol {
                name: name.to_owned(),
                section,
                line: number,
                shared: self.pending_shared.take().is_some(),
                body: vec![
                    normalize(&format!(".type {name}, @object")),
                    format!("{name}:"),
                ],
                table: Vec::new(),
            });
        }
    }

    fn directive(&mut self, number: usize, code: &str) {
        let (name, arguments) = code
            .split_once(char::is_whitespace)
            .map_or((code, ""), |(name, rest)| (name, rest.trim()));
        match name {
            ".equ" => self.equ(number, arguments),
            ".text" => self.section(number, SectionKind::Text),
            ".section" => match arguments {
                ".text" => self.section(number, SectionKind::Text),
                ".rodata" => self.section(number, SectionKind::ReadOnly),
                ".data" => self.section(number, SectionKind::Data),
                ".bss" => self.section(number, SectionKind::Bss),
                other => self.dialect(
                    number,
                    format!("section {other} isn't in the dialect"),
                    "use .text, .section .rodata, .section .data or .section .bss",
                ),
            },
            ".globl" | ".global" => {
                if arguments != "_start" {
                    self.violation(
                        Rule::Scope,
                        number,
                        format!("{arguments} is exported"),
                        "export only _start; every other symbol stays local",
                    );
                }
                self.globals.push((arguments.to_owned(), number));
            }
            ".type" => self.type_directive(number, arguments),
            ".size" => self.size(number, arguments),
            ".ascii" | ".asciz" | ".string" | ".byte" | ".short" | ".long" | ".quad"
            | ".double" | ".float" | ".zero" | ".skip" | ".balign" => {
                self.data_directive(number, name, arguments, code)
            }
            ".intel_syntax" => self.dialect(
                number,
                ".intel_syntax appears again",
                "write it once, at the top",
            ),
            ".macro" | ".endm" | ".rept" | ".irp" | ".irpc" | ".endr" | ".if" | ".ifdef"
            | ".ifndef" | ".else" | ".endif" | ".include" | ".incbin" | ".set" => self.violation(
                Rule::Constants,
                number,
                format!(
                    "{name} isn't allowed: no macros, repeats, conditional assembly or includes"
                ),
                "write every instruction out; name numbers with .equ in the block at the top",
            ),
            other => self.dialect(
                number,
                format!("{other} isn't in the dialect"),
                "see the directive list in references/power-of-10.md",
            ),
        }
    }

    fn equ(&mut self, number: usize, arguments: &str) {
        if self.place != Place::Header {
            self.dialect(
                number,
                "a constant is defined below the header block",
                "move every .equ into the block at the top, before any section; the assembler \
                 silently reads constants defined later as memory addresses",
            );
            return;
        }
        let Some((name, expression)) = arguments.split_once(',') else {
            self.dialect(
                number,
                ".equ needs a name and a value",
                "write `.equ NAME, value`",
            );
            return;
        };
        let name = name.trim();
        if !is_constant_name(name) || register(name).is_some() {
            self.violation(
                Rule::Constants,
                number,
                format!("`{name}` isn't a constant name"),
                "name constants in UPPER_CASE with letters, digits and _",
            );
            return;
        }
        if let Some(previous) = self.source.constants.get(name) {
            let previous = previous.line;
            self.violation(
                Rule::Constants,
                number,
                format!("{name} is defined twice (also on line {previous})"),
                "define each constant once",
            );
            return;
        }
        if expression.contains('.') || expression.contains('"') {
            self.violation(
                Rule::Constants,
                number,
                format!("{name}'s value uses an address or text"),
                "constants are numbers: literals or earlier constants with + - * /; write a \
                 string's length as a number and lengua checks it against the data",
            );
            return;
        }
        match evaluate(expression, &self.source.constants) {
            Ok(value) => {
                self.source.constants.insert(
                    name.to_owned(),
                    Constant {
                        value,
                        line: number,
                    },
                );
            }
            Err(problem) => self.violation(
                Rule::Constants,
                number,
                format!("{name}: {problem}"),
                "constants are literals or earlier constants with + - * /",
            ),
        }
    }

    fn section(&mut self, number: usize, section: SectionKind) {
        if self.function.is_some() || self.data.is_some() {
            self.dialect(
                number,
                "a section starts before the previous symbol's .size line",
                "end each function or data symbol with `.size name, . - name`",
            );
        }
        self.place = Place::Section(section);
    }

    fn type_directive(&mut self, number: usize, arguments: &str) {
        let parsed = arguments
            .split_once(',')
            .map(|(name, kind)| (name.trim(), kind.trim()));
        let (name, is_function) = match parsed {
            Some((name, "@function")) => (name, true),
            Some((name, "@object")) => (name, false),
            _ => {
                self.dialect(
                    number,
                    "`.type` needs a name and @function or @object",
                    "write `.type name, @function` or `.type name, @object`",
                );
                return;
            }
        };
        let right_section = match self.place {
            Place::Section(SectionKind::Text) => is_function,
            Place::Section(_) => !is_function,
            Place::Header => false,
        };
        if !right_section {
            self.dialect(
                number,
                format!("{name} is in the wrong section"),
                "put functions in .text and data in .rodata, .data or .bss",
            );
        }
        self.pending_type = Some((name.to_owned(), is_function, number));
    }

    fn size(&mut self, number: usize, arguments: &str) {
        let expected_name = self
            .function
            .as_ref()
            .map(|open| open.function.name.clone())
            .or_else(|| self.data.as_ref().map(|data| data.name.clone()));
        let Some(name) = expected_name else {
            self.dialect(
                number,
                ".size doesn't close a function or data symbol",
                "write .size right after the symbol's last line",
            );
            return;
        };
        if normalize(arguments) != format!("{name}, . - {name}") {
            self.dialect(
                number,
                format!("{name}'s size must be computed by the assembler"),
                format!("write `.size {name}, . - {name}`"),
            );
        }
        let closing = normalize(&format!(".size {arguments}"));
        if let Some(mut open) = self.function.take() {
            if let Some((start, _)) = open.open_unsafe.take() {
                self.violation(
                    Rule::Indirection,
                    start,
                    "this unsafe block isn't closed",
                    "end it with `# end unsafe` before the function ends",
                );
            }
            open.function.body.push(closing);
            self.source.functions.push(open.function);
        } else if let Some(mut data) = self.data.take() {
            data.body.push(closing);
            self.source.data.push(data);
        }
    }

    fn data_directive(&mut self, number: usize, name: &str, arguments: &str, code: &str) {
        // `.balign N` between `.type name, @object` and `name:` aligns where the symbol starts;
        // the padding before it belongs to no symbol and is never read.
        if self.data.is_none()
            && name == ".balign"
            && matches!(self.pending_type, Some((_, false, _)))
        {
            if !matches!(
                evaluate(arguments, &self.source.constants),
                Ok(1 | 2 | 4 | 8 | 16 | 32 | 64)
            ) {
                self.dialect(
                    number,
                    ".balign takes 1, 2, 4, 8, 16, 32 or 64",
                    "align data to a power of two",
                );
            }
            return;
        }
        let Some(data) = self.data.as_mut() else {
            self.dialect(
                number,
                format!("{name} emits bytes outside a data symbol"),
                "put data between `name:` and `.size name, . - name` in a data section; never in \
                 .text",
            );
            return;
        };
        data.body.push(normalize(code));
        let section = data.section;
        let symbol = data.name.clone();
        match name {
            ".zero" | ".skip" => {
                if !self.source.constants.contains_key(arguments.trim()) {
                    self.violation(
                        Rule::Constants,
                        number,
                        format!("{name} {arguments} uses a bare number"),
                        format!("name the size with a constant, such as `{name} BUF_LEN`"),
                    );
                }
            }
            ".balign" => match evaluate(arguments, &self.source.constants) {
                Ok(1 | 2 | 4 | 8 | 16 | 32 | 64) => {}
                _ => self.dialect(
                    number,
                    ".balign takes 1, 2, 4, 8, 16, 32 or 64",
                    "align data to a power of two",
                ),
            },
            ".double" | ".float" => {
                for value in split_operands(arguments) {
                    if crate::expr::Limit::parse(value.trim()).is_none() {
                        self.violation(
                            Rule::Constants,
                            number,
                            format!("`{name} {value}` isn't a plain decimal number"),
                            format!(
                                "write floating-point data as plain decimals, such as `{name} \
                                 100.0`, with no names or exponents"
                            ),
                        );
                    }
                }
            }
            ".byte" | ".short" | ".long" | ".quad" => {
                for value in split_operands(arguments) {
                    // A jump table's entry: a label of the function that jumps through the table,
                    // as its distance from the table, which the linker fills in.
                    if let Some((label, from)) = value.split_once('-')
                        && label.trim().starts_with(".L")
                    {
                        let (label, from) = (label.trim(), from.trim());
                        if name != ".long" || from != symbol {
                            self.violation(
                                Rule::Indirection,
                                number,
                                format!("data value `{value}` isn't a jump table's entry"),
                                format!(
                                    "a jump table's entries are `.long .Lcase - {symbol}`: a label \
                                     of the function that jumps through it, minus the table itself"
                                ),
                            );
                        } else if section != SectionKind::ReadOnly {
                            self.violation(
                                Rule::Immutability,
                                number,
                                format!("jump table {symbol} is in writable data"),
                                "put a jump table in .section .rodata, where nothing can change \
                                 where it jumps",
                            );
                        } else if let Some(data) = self.data.as_mut() {
                            data.table.push(label.to_owned());
                        }
                        continue;
                    }
                    if let Err(problem) = evaluate(value, &self.source.constants) {
                        self.violation(
                            Rule::Indirection,
                            number,
                            format!("data value `{value}`: {problem}"),
                            "data holds numbers only, never addresses, but for a jump table's \
                             entries",
                        );
                    } else if self
                        .data
                        .as_ref()
                        .is_some_and(|data| !data.table.is_empty())
                    {
                        self.violation(
                            Rule::Indirection,
                            number,
                            format!("jump table {symbol} mixes numbers with its entries"),
                            "a jump table holds only `.long .Lcase - table` entries",
                        );
                    }
                }
            }
            _ => {}
        }
        if section == SectionKind::Bss && !matches!(name, ".zero" | ".skip" | ".balign") {
            self.dialect(
                number,
                format!("{name} in .bss"),
                ".bss holds zeros only: use .zero or .skip, or move the data to .data",
            );
        }
    }

    fn instruction(&mut self, number: usize, code: &str) {
        let (mnemonic, rest) = code
            .split_once(char::is_whitespace)
            .map_or((code, ""), |(mnemonic, rest)| (mnemonic, rest.trim()));
        let mnemonic = mnemonic.to_ascii_lowercase();
        let Some(open) = self.function.as_mut() else {
            self.dialect(
                number,
                "an instruction is outside a function",
                "put instructions between `name:` and `.size name, . - name` in .text",
            );
            return;
        };
        open.function.body.push(normalize(code));
        let unsafe_block = open
            .open_unsafe
            .as_ref()
            .map(|_| open.function.unsafe_blocks.len());
        if self.source.isa == Isa::Aarch64 {
            self.aarch64_instruction(number, code, mnemonic, rest, unsafe_block);
            return;
        }
        if PREFIXES.contains(&mnemonic.as_str()) {
            self.dialect(
                number,
                format!("`{mnemonic}` is a prefix"),
                "prefixes aren't in the dialect; write one plain instruction per line",
            );
            return;
        }
        let branch = is_branch(&mnemonic);
        let mut operands = Vec::new();
        let texts = split_operands(rest);
        if texts.iter().any(|text| text.is_empty()) {
            self.dialect(
                number,
                "an operand is missing",
                "write every operand, separated by commas",
            );
            return;
        }
        let mut mask = None;
        let mut zeroing = false;
        for (position, text) in texts.into_iter().enumerate() {
            let parsed = decorations(text, position).and_then(|(text, decoration)| {
                let mut parsed = operand(text, branch, &self.source.constants)?;
                if decoration.broadcast {
                    let Operand::Memory(memory) = &mut parsed else {
                        return Err("a broadcast ({1to16}) goes on a memory operand".into());
                    };
                    memory.broadcast = true;
                }
                mask = mask.or(decoration.mask);
                zeroing |= decoration.zeroing;
                Ok(parsed)
            });
            match parsed {
                Ok(parsed) => operands.push(parsed),
                Err(problem) => {
                    self.dialect(
                        number,
                        format!("`{text}`: {problem}"),
                        "see the operand forms in references/power-of-10.md",
                    );
                    return;
                }
            }
        }
        if let Some(open) = self.function.as_mut() {
            open.function.items.push(Item::Instruction(Instruction {
                line: number,
                mnemonic,
                operands,
                mask,
                zeroing,
                text: normalize(code),
                unsafe_block,
                pieces: None,
            }));
        }
    }

    /// An AArch64 instruction: its operands into the shared form, and the whole line held to
    /// the dialect's syntax, with any label or symbol standing in for its address.
    fn aarch64_instruction(
        &mut self,
        number: usize,
        code: &str,
        mnemonic: String,
        rest: &str,
        unsafe_block: Option<usize>,
    ) {
        let texts = split_operands(rest);
        if texts.iter().any(|text| text.is_empty()) {
            self.dialect(
                number,
                "an operand is missing",
                "write every operand, separated by commas",
            );
            return;
        }
        let mut operands = Vec::new();
        for &text in &texts {
            match aarch64_operand(text, &self.source.constants) {
                Ok(parsed) => operands.push(parsed),
                Err(problem) => {
                    self.dialect(
                        number,
                        format!("`{text}`: {problem}"),
                        "see the operand forms in references/linux-aarch64.md",
                    );
                    return;
                }
            }
        }
        // A constant, or an expression of them, is its value; a label or symbol stands in as 0.
        let constants = &self.source.constants;
        let stand_in = |name: &str| Some(evaluate(name, constants).unwrap_or(0));
        if let Err(problem) = aarch64::text::parse(&mnemonic, rest, 0, &stand_in) {
            self.dialect(
                number,
                problem,
                "see the instructions and operand forms in references/linux-aarch64.md",
            );
            return;
        }
        let pieces = (mnemonic == "movk")
            .then(|| self.pieces(&operands, texts.get(1).copied().unwrap_or_default()))
            .flatten();
        if let Some(open) = self.function.as_mut() {
            open.function.items.push(Item::Instruction(Instruction {
                line: number,
                mnemonic,
                operands,
                mask: None,
                zeroing: false,
                text: normalize(code),
                unsafe_block,
                pieces,
            }));
        }
    }

    /// What a `movk` and the `mov` or `movk`s right before it, with no label between, put in
    /// its register: the `movk` replaces 16 bits of what the line before put there.
    fn pieces(&self, operands: &[Operand], part_text: &str) -> Option<Pieces> {
        let constants = &self.source.constants;
        let [
            register @ Operand::Register(Reg::A64(written)),
            Operand::Immediate { value: part, .. },
            rest @ ..,
        ] = operands
        else {
            return None;
        };
        let shift = match rest {
            [] => 0,
            [Operand::Modifier(modifier)] => {
                let amount = modifier.strip_prefix("lsl")?.trim().strip_prefix('#')?;
                u32::try_from(evaluate(amount, constants).ok()?)
                    .ok()
                    .filter(|shift| {
                        shift % 16 == 0 && *shift < if written.wide { 64 } else { 32 }
                    })?
            }
            _ => return None,
        };
        let Some(Item::Instruction(before)) = self.function.as_ref()?.function.items.last() else {
            return None;
        };
        if before.operands.first() != Some(register) {
            return None;
        }
        let (value, mentions) = match (before.mnemonic.as_str(), &before.operands[..]) {
            ("movk", _) => {
                let pieces = before.pieces.as_ref()?;
                (pieces.value, pieces.mentions.clone())
            }
            ("mov", [_, Operand::Immediate { value, .. }]) => {
                let (_, operands) = before.text.split_once(' ')?;
                let first = split_operands(operands).get(1).copied()?;
                (*value, mentioned(first, constants))
            }
            _ => return None,
        };
        let width = if written.wide {
            u64::MAX
        } else {
            u64::from(u32::MAX)
        };
        let field = 0xffff_u64 << shift;
        let value =
            ((value.cast_unsigned() & !field) | ((part.cast_unsigned() & 0xffff) << shift)) & width;
        let value = value.cast_signed();
        let mentions =
            mentions.filter(|name| mentioned(part_text, constants).as_ref() == Some(name));
        let name = mentions.clone().filter(|name| {
            constants
                .get(name)
                .is_some_and(|constant| constant.value == value)
        });
        Some(Pieces {
            value,
            mentions,
            name,
        })
    }

    fn finish(mut self, last_line: usize) -> (Source, Vec<Violation>) {
        if !self.seen_header {
            self.dialect(
                1,
                "main.s is too short",
                "start from the template in assets/main-template.s",
            );
        }
        if let Some(open) = self.function.take() {
            self.dialect(
                last_line,
                format!("{} has no .size line", open.function.name),
                format!("end it with `.size {0}, . - {0}`", open.function.name),
            );
        }
        if let Some(data) = self.data.take() {
            self.dialect(
                last_line,
                format!("{} has no .size line", data.name),
                format!("end it with `.size {0}, . - {0}`", data.name),
            );
        }
        if let Some((name, _, line)) = self.pending_type.take() {
            self.dialect(
                line,
                format!("{name} has a .type but no label"),
                format!("write `{name}:` below it"),
            );
        }
        let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
        let symbols = self
            .source
            .functions
            .iter()
            .map(|function| (function.name.as_str(), function.line))
            .chain(
                self.source
                    .data
                    .iter()
                    .map(|data| (data.name.as_str(), data.line)),
            );
        let mut duplicates = Vec::new();
        for (name, line) in symbols {
            if let Some(first) = seen.insert(name, line) {
                duplicates.push((name.to_owned(), line, first));
            }
        }
        for (name, line, first) in duplicates {
            self.dialect(
                line,
                format!("{name} is defined twice (also on line {first})"),
                "give each symbol its own name",
            );
        }
        if self.source.function("_start").is_none() {
            self.dialect(
                last_line,
                "there's no _start function",
                "start from the template, which has one",
            );
        }
        if !self.globals.iter().any(|(name, _)| name == "_start") {
            self.violation(
                Rule::Scope,
                last_line,
                "_start isn't exported",
                "add `.globl _start` in .text",
            );
        }
        (self.source, self.violations)
    }
}

/// The line main.s for `isa` must start with.
#[must_use]
pub const fn header(isa: Isa) -> &'static str {
    match isa {
        Isa::X86_64 => ".intel_syntax noprefix",
        Isa::Aarch64 => ".arch armv8-a",
    }
}

/// The CPU main.s names in its first statement: AArch64 for `.arch`, x86-64 otherwise.
#[must_use]
pub fn isa_of(text: &str) -> Isa {
    let first = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("//"));
    if first.is_some_and(|line| line.starts_with(".arch")) {
        Isa::Aarch64
    } else {
        Isa::X86_64
    }
}

/// An AArch64 operand in the shared form: a register, a number (`#…`, named when it's exactly
/// one constant), memory (`[…]`), the low 12 bits of a data symbol's address (`:lo12:name`, as
/// memory naming the symbol, the way `lea`'s operand does on x86-64), a target or data symbol by
/// name, or a shift, extend or condition.
fn aarch64_operand(text: &str, constants: &BTreeMap<String, Constant>) -> Result<Operand, String> {
    if text.starts_with('=') {
        return Err(
            "a number from a literal pool isn't in the dialect: put one in a register \
                    with `mov xR, #NAME`, or with `mov xR, #NAME & 0xffff` and a `movk` for \
                    each further 16 bits, such as `movk xR, #NAME >> 16, lsl #16`"
                .into(),
        );
    }
    if let Some(expression) = text.strip_prefix('#') {
        let expression = expression.trim();
        // A floating-point number, such as fmov's `#1.5` or fcmp's `#0.0`, which matching reads
        // as the instruction does.
        if is_float(expression) {
            return Ok(Operand::Modifier(text.to_owned()));
        }
        let value = evaluate(expression, constants)?;
        let name = constants
            .contains_key(expression)
            .then(|| expression.to_owned());
        return Ok(Operand::Immediate { value, name });
    }
    if let Some(name) = text.strip_prefix(":lo12:") {
        let name = name.trim();
        if !is_identifier(name) {
            return Err("`:lo12:` names a data symbol".into());
        }
        return Ok(Operand::Memory(Memory {
            size: None,
            base: None,
            index: None,
            scale: 1,
            displacement: 0,
            symbol: Some(name.to_owned()),
            broadcast: false,
        }));
    }
    if text.starts_with('[') {
        return aarch64_memory(text, constants).map(Operand::Memory);
    }
    if let Some(register) = aarch64::text::register(text) {
        return Ok(Operand::Register(Reg::A64(register)));
    }
    if let Some(register) = aarch64::text::fp_register(text) {
        return Ok(Operand::Register(Reg::A64Fp(register)));
    }
    if A64Cond::named(text).is_some() || text.contains(char::is_whitespace) {
        return Ok(Operand::Modifier(text.to_owned()));
    }
    if is_identifier(text) || is_local_label(text) {
        return Ok(Operand::Target(text.to_owned()));
    }
    Err("isn't an operand the dialect has".into())
}

/// Whether `text` is a floating-point number with a point: `1.5`, `-0.25`, `2.0e3`.
fn is_float(text: &str) -> bool {
    let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
    digits.starts_with(|first: char| first.is_ascii_digit())
        && digits.contains('.')
        && digits.parse::<f64>().is_ok()
}

/// An AArch64 memory operand: `[base]`, `[base, #offset]` (with `!` when the base moves first),
/// or `[base, index{, extend #shift}]`.
fn aarch64_memory(text: &str, constants: &BTreeMap<String, Constant>) -> Result<Memory, String> {
    let inside = text.strip_suffix('!').unwrap_or(text).trim_end();
    let inner = inside
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .ok_or("a memory operand is in brackets")?;
    let parts = split_operands(inner);
    if parts.iter().any(|part| part.starts_with(":lo12:")) {
        return Err(
            "`:lo12:` goes on the `add` after `adrp`, never inside an address: \
                    load through the register that add makes"
                .into(),
        );
    }
    let base = parts
        .first()
        .and_then(|name| aarch64::text::register(name))
        .ok_or("a memory operand starts with its base register")?;
    let mut memory = Memory {
        size: None,
        base: Some(Reg::A64(base)),
        index: None,
        scale: 1,
        displacement: 0,
        symbol: None,
        broadcast: false,
    };
    match parts.get(1..).unwrap_or_default() {
        [] => {}
        [offset] if offset.starts_with('#') => {
            memory.displacement = evaluate(offset[1..].trim(), constants)?;
        }
        [index, rest @ ..] if rest.len() <= 1 => {
            let index =
                aarch64::text::register(index).ok_or("a memory operand's index is a register")?;
            memory.index = Some(Reg::A64(index));
            if let Some(extend) = rest.first()
                && let Some((_, amount)) = extend.split_once('#')
            {
                let shift = evaluate(amount.trim(), constants)?;
                memory.scale = 1_u32
                    .checked_shl(u32::try_from(shift).map_err(|_| "a negative shift")?)
                    .ok_or("too big a shift")?;
            }
        }
        _ => return Err("a memory operand has a base, then an offset or an index".into()),
    }
    Ok(memory)
}

/// Parses main.s, returning what it found and every layout violation.
#[must_use]
pub fn parse(text: &str) -> (Source, Vec<Violation>) {
    let mut parser = Parser {
        source: Source {
            isa: isa_of(text),
            ..Source::default()
        },
        violations: Vec::new(),
        place: Place::Header,
        pending_returns: None,
        pending_from: None,
        pending_shared: None,
        pending_type: None,
        function: None,
        data: None,
        globals: Vec::new(),
        seen_header: false,
    };
    let mut last = 1;
    for (index, line) in text.lines().enumerate() {
        last = index + 1;
        parser.line(index + 1, line);
    }
    parser.finish(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = include_str!("../../plugin/skills/compile/assets/main-template.s");

    fn violations(text: &str) -> Vec<String> {
        parse(text).1.iter().map(ToString::to_string).collect()
    }

    fn assert_violation(text: &str, needle: &str) {
        let found = violations(text);
        assert!(
            found.iter().any(|violation| violation.contains(needle)),
            "no violation containing {needle:?} in {found:#?}"
        );
    }

    #[test]
    fn reads_the_template() {
        let (source, violations) = parse(TEMPLATE);
        assert!(violations.is_empty(), "{violations:#?}");
        assert_eq!(source.constants["SYS_EXIT"].value, 60);
        assert_eq!(source.constants["ERROR_INVALID"].value, -22);
        let names: Vec<&str> = source.functions.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "_start",
                "parse_i64",
                "write_i64",
                "write_all",
                "assert_fail"
            ]
        );
        let parse_i64 = source.function("parse_i64").expect("parse_i64");
        assert!(parse_i64.returns_result);
        assert!(!source.function("_start").expect("_start").returns_result);
        let bounds: Vec<u64> = parse_i64
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Label(label) => label.bound,
                Item::Instruction(_) => None,
            })
            .collect();
        assert_eq!(bounds, [20]);
        let data: Vec<(&str, SectionKind)> = source
            .data
            .iter()
            .map(|data| (data.name.as_str(), data.section))
            .collect();
        assert_eq!(
            data,
            [
                ("usage_msg", SectionKind::ReadOnly),
                ("nan_msg", SectionKind::ReadOnly),
                ("assert_message", SectionKind::ReadOnly),
                ("out_buf", SectionKind::Bss),
            ]
        );
    }

    #[test]
    fn parses_operands() {
        let (source, _) = parse(TEMPLATE);
        let constants = &source.constants;
        assert_eq!(
            operand("rax", false, constants),
            Ok(Operand::Register(Reg::X86(Register::RAX)))
        );
        assert_eq!(
            operand("SYS_WRITE", false, constants),
            Ok(Operand::Immediate {
                value: 1,
                name: Some("SYS_WRITE".into())
            })
        );
        assert_eq!(
            operand("OUT_BUF_LEN - 1", false, constants),
            Ok(Operand::Immediate {
                value: 20,
                name: None
            })
        );
        assert_eq!(
            operand("'0'", false, constants),
            Ok(Operand::Immediate {
                value: 48,
                name: None
            })
        );
        assert_eq!(
            operand("-0x10", false, constants),
            Ok(Operand::Immediate {
                value: -16,
                name: None
            })
        );
        assert_eq!(
            operand("qword ptr [rsp + 16]", false, constants),
            Ok(Operand::Memory(Memory {
                size: Some(8),
                base: Some(Reg::X86(Register::RSP)),
                index: None,
                scale: 1,
                displacement: 16,
                symbol: None,
                broadcast: false,
            }))
        );
        assert_eq!(
            operand(
                "byte ptr [rip + out_buf + OUT_BUF_LEN - 1]",
                false,
                constants
            ),
            Ok(Operand::Memory(Memory {
                size: Some(1),
                base: Some(Reg::X86(Register::RIP)),
                index: None,
                scale: 1,
                displacement: 20,
                symbol: Some("out_buf".into()),
                broadcast: false,
            }))
        );
        assert_eq!(
            operand("[rbx + rcx*8 - 8]", false, constants),
            Ok(Operand::Memory(Memory {
                size: None,
                base: Some(Reg::X86(Register::RBX)),
                index: Some(Reg::X86(Register::RCX)),
                scale: 8,
                displacement: -8,
                symbol: None,
                broadcast: false,
            }))
        );
        assert_eq!(
            operand(".Lloop", true, constants),
            Ok(Operand::Target(".Lloop".into()))
        );
        for (text, problem) in [
            ("offset usage_msg", "offset"),
            ("usage_msg", "isn't a constant"),
            ("[usage_msg]", "address data relative to rip"),
            ("[0x1000]", "needs a base register"),
            ("[rip + 8]", "must name a data symbol"),
            ("[rax + rbx + rcx]", "at most a base and an index"),
            ("[rax + rbx*3]", "scale is 1, 2, 4 or 8"),
            ("fs:[rax]", "segment"),
            ("LATER + 1", "isn't a constant defined above"),
        ] {
            let result = operand(text, false, constants);
            assert!(
                result.as_ref().is_err_and(|error| error.contains(problem)),
                "{text}: {result:?}"
            );
        }
    }

    #[test]
    fn enforces_one_instruction_per_line() {
        let with = |old: &str, new: &str| TEMPLATE.replacen(old, new, 1);
        assert_violation(
            &with("    xor r9d, r9d\n", "    xor r9d, r9d; xor eax, eax\n"),
            "`;` puts several statements",
        );
        assert_violation(
            &with("    xor r9d, r9d\n", "    xor r9d, r9d /* clear */\n"),
            "`/* */` comments",
        );
        assert_violation(
            &with(
                ".Lparse_digits:\n",
                ".Lparse_digits: cmp byte ptr [rdi], 0\n",
            ),
            "a label and an instruction share a line",
        );
        assert_violation(
            &with("    xor r9d, r9d\n", "    rep\n"),
            "`rep` is a prefix",
        );
        assert_violation(
            &with("    xor r9d, r9d\n", "    lock inc rax\n"),
            "`lock` is a prefix",
        );
        assert_violation(&with(".Lparse_digits:\n", "1:\n"), "numbered labels");
    }

    #[test]
    fn requires_constants_before_use() {
        let late = TEMPLATE.replacen(".equ MAX_WRITES, 64\n", "", 1).replacen(
            ".section .rodata\n",
            ".section .rodata\n.equ MAX_WRITES, 64\n",
            1,
        );
        assert_violation(&late, "a constant is defined below the header block");
        assert_violation(
            &TEMPLATE.replacen(
                ".equ NEWLINE, 10\n",
                ".equ NEWLINE, 10\n.equ NEWLINE, 13\n",
                1,
            ),
            "NEWLINE is defined twice",
        );
        assert_violation(
            &TEMPLATE.replacen(
                ".equ USAGE_MSG_LEN, 18",
                ".equ USAGE_MSG_LEN, . - usage_msg",
                1,
            ),
            "uses an address or text",
        );
        assert_violation(
            &TEMPLATE.replacen(".equ NEWLINE, 10", ".equ newline, 10", 1),
            "isn't a constant name",
        );
    }

    #[test]
    fn rejects_macros_and_unknown_directives() {
        let with = |new: &str| TEMPLATE.replacen(".text\n", &format!(".text\n{new}\n"), 1);
        assert_violation(&with(".macro twice"), "no macros, repeats");
        assert_violation(&with(".rept 4"), "no macros, repeats");
        assert_violation(&with(".p2align 4"), ".p2align isn't in the dialect");
        assert_violation(&with(".globl parse_i64"), "parse_i64 is exported");
    }

    #[test]
    fn checks_the_header_and_structure() {
        assert_violation(
            &TEMPLATE.replacen(".intel_syntax noprefix", ".intel_syntax", 1),
            "must start with .intel_syntax noprefix",
        );
        assert_violation(
            &format!(".equ ONE, 1\n{TEMPLATE}"),
            "must start with .intel_syntax noprefix",
        );
        assert!(
            violations(&format!("# A comment first.\n\n{TEMPLATE}")).is_empty(),
            "comments may come before .intel_syntax"
        );
        assert_violation(
            &TEMPLATE.replacen(".size nan_msg, . - nan_msg", ".size nan_msg, 20", 1),
            "size must be computed by the assembler",
        );
        assert_violation(
            &TEMPLATE.replacen(".type write_all, @function\n", "", 1),
            "write_all has no .type line",
        );
        assert_violation(
            &TEMPLATE.replacen("    .zero OUT_BUF_LEN", "    .zero 21", 1),
            ".zero 21 uses a bare number",
        );
        assert_violation(
            &TEMPLATE.replacen(".text\n", ".text\n.byte 0x90\n", 1),
            ".byte emits bytes outside a data symbol",
        );
    }

    #[test]
    fn tracks_unsafe_blocks() {
        let marked = TEMPLATE.replacen(
            "    movzx edx, byte ptr [rdi + rcx]\n",
            "    # unsafe: the loop counter keeps rcx below MAX_CHARS\n    movzx edx, byte ptr [rdi + rcx]\n    # end unsafe\n",
            1,
        );
        let (source, violations) = parse(&marked);
        assert!(violations.is_empty(), "{violations:#?}");
        let parse_i64 = source.function("parse_i64").expect("parse_i64");
        assert_eq!(parse_i64.unsafe_blocks.len(), 1);
        assert!(
            parse_i64
                .instructions()
                .any(|instruction| instruction.unsafe_block == Some(0))
        );
        assert_violation(
            &TEMPLATE.replacen(
                "    xor r9d, r9d\n",
                "    # unsafe:\n    xor r9d, r9d\n    # end unsafe\n",
                1,
            ),
            "needs a reason",
        );
        assert_violation(
            &TEMPLATE.replacen(
                "    xor r9d, r9d\n",
                "    # unsafe: x\n    xor r9d, r9d\n",
                1,
            ),
            "unsafe block isn't closed",
        );
    }
}
