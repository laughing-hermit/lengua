//! A WebAssembly module as the model runs it: read back with `wasmparser` from the module lengua
//! built from main.wat, each function's operators in the model's own form with their offsets and
//! every block's end found once, the globals' and the memory's first values, the data segments,
//! and each import tied to its function of WASI preview 1 in lengua's host's table.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use wasmparser::{
    BlockType, CompositeInnerType, ConstExpr, DataKind, ExternalKind, KnownCustom, MemArg, Name,
    Operator, Parser, Payload, TypeRef, ValType,
};

use crate::wasm::host::policy::Policy;
use crate::wasm::text::Built;
use crate::wasm::wasi;

/// A WebAssembly page, in bytes.
pub(super) const PAGE: u64 = 65_536;

/// The most pages a 32-bit memory has.
const MOST_PAGES: u64 = 65_536;

/// A value's type, which the model holds as a bit vector of its width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// A 32-bit integer.
    I32,
    /// A 64-bit integer.
    I64,
    /// A single float's 32 bits.
    F32,
    /// A double float's 64 bits.
    F64,
    /// A vector's 128 bits.
    V128,
}

impl Kind {
    /// The kind of `value`, a value type of WebAssembly 2.0's numbers and vectors; `None` for a
    /// reference.
    fn of(value: ValType) -> Option<Self> {
        match value {
            ValType::I32 => Some(Self::I32),
            ValType::I64 => Some(Self::I64),
            ValType::F32 => Some(Self::F32),
            ValType::F64 => Some(Self::F64),
            ValType::V128 => Some(Self::V128),
            _ => None,
        }
    }

    /// How many bits a value of it has.
    pub(super) const fn width(self) -> u32 {
        match self {
            Self::I32 | Self::F32 => 32,
            Self::I64 | Self::F64 => 64,
            Self::V128 => 128,
        }
    }
}

/// What a block takes from the stack and what it leaves there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Arity {
    /// The values it takes.
    pub(super) params: usize,
    /// The values it leaves.
    pub(super) results: usize,
}

/// An operator on one integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Unary {
    /// `eqz`: 1 for zero, else 0, as an i32.
    Eqz,
    /// `clz`: the zeros above the highest one.
    Clz,
    /// `ctz`: the zeros below the lowest one.
    Ctz,
    /// `popcnt`: the ones.
    Popcnt,
    /// `extend8_s`: the low 8 bits, sign-extended.
    Extend8,
    /// `extend16_s`: the low 16 bits, sign-extended.
    Extend16,
    /// `i64.extend32_s`: the low 32 bits, sign-extended.
    Extend32,
    /// `i32.wrap_i64`: the low 32 bits of an i64.
    Wrap,
    /// `i64.extend_i32_s`: an i32 read signed, as an i64.
    Signed,
    /// `i64.extend_i32_u`: an i32 read unsigned, as an i64.
    Unsigned,
}

/// An operator on two integers of one width: arithmetic, bits and comparisons, which give an i32.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Binary {
    /// `add`.
    Add,
    /// `sub`.
    Sub,
    /// `mul`.
    Mul,
    /// `div_s`: toward zero.
    DivS,
    /// `div_u`.
    DivU,
    /// `rem_s`: with the dividend's sign.
    RemS,
    /// `rem_u`.
    RemU,
    /// `and`.
    And,
    /// `or`.
    Or,
    /// `xor`.
    Xor,
    /// `shl`: the count modulo the width.
    Shl,
    /// `shr_s`.
    ShrS,
    /// `shr_u`.
    ShrU,
    /// `rotl`.
    Rotl,
    /// `rotr`.
    Rotr,
    /// `eq`.
    Eq,
    /// `ne`.
    Ne,
    /// `lt_s`.
    LtS,
    /// `lt_u`.
    LtU,
    /// `gt_s`.
    GtS,
    /// `gt_u`.
    GtU,
    /// `le_s`.
    LeS,
    /// `le_u`.
    LeU,
    /// `ge_s`.
    GeS,
    /// `ge_u`.
    GeU,
}

impl Binary {
    /// Whether it divides, so a divisor of zero traps.
    pub(super) const fn divides(self) -> bool {
        matches!(self, Self::DivS | Self::DivU | Self::RemS | Self::RemU)
    }
}

/// One operator, as the model runs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Op {
    /// `unreachable`: a trap.
    Unreachable,
    /// `nop`.
    Nop,
    /// `block`.
    Block(Arity),
    /// `loop`.
    Loop(Arity),
    /// `if`.
    If(Arity),
    /// `else`.
    Else,
    /// `end`, of a block or of the function.
    End,
    /// `br`, to the label this many blocks out.
    Br(u32),
    /// `br_if`.
    BrIf(u32),
    /// `br_table`: the labels by index, then the default.
    BrTable(Vec<u32>, u32),
    /// `return`.
    Return,
    /// `call`, of the function with this index.
    Call(u32),
    /// `drop`.
    Drop,
    /// `select`, typed or not.
    Select,
    /// `local.get`.
    LocalGet(u32),
    /// `local.set`.
    LocalSet(u32),
    /// `local.tee`.
    LocalTee(u32),
    /// `global.get`.
    GlobalGet(u32),
    /// `global.set`.
    GlobalSet(u32),
    /// A load of `bytes` bytes at the address plus `offset`, extended to `width` bits, read
    /// signed or not.
    Load {
        /// How many bytes it reads.
        bytes: u32,
        /// How many bits its value has.
        width: u32,
        /// Whether a narrower value is sign-extended.
        signed: bool,
        /// The operator's offset, added to the address.
        offset: u64,
    },
    /// A store of the value's low `bytes` bytes at the address plus `offset`.
    Store {
        /// How many bytes it writes.
        bytes: u32,
        /// The operator's offset.
        offset: u64,
    },
    /// `memory.size`.
    MemorySize,
    /// `memory.grow`.
    MemoryGrow,
    /// `memory.copy`.
    MemoryCopy,
    /// `memory.fill`.
    MemoryFill,
    /// `memory.init` from this data segment.
    MemoryInit(u32),
    /// `data.drop` of this data segment.
    DataDrop(u32),
    /// A constant of this width.
    Const {
        /// Its bits.
        width: u32,
        /// Its value.
        value: u128,
    },
    /// An operator on one integer, of the width its operand has.
    Unary(Unary),
    /// An operator on two integers, of the width their operands have.
    Binary(Binary),
    /// An operator the model doesn't follow yet, by its name.
    Other(String),
}

/// A function of the module, in its index space: the imports first, then the definitions.
#[derive(Debug)]
pub(super) struct Function {
    /// Its name, from the name section or lengua's library.
    pub(super) name: Option<String>,
    /// How many values it takes.
    pub(super) params: usize,
    /// How many values it gives.
    pub(super) results: usize,
    /// Whether it's one of main.wat's own functions, not lengua's library's or an import.
    pub(super) own: bool,
    /// For an import, the WASI function of lengua's host it calls.
    pub(super) import: Option<&'static wasi::Function>,
    /// The kinds of the locals it declares after its parameters.
    pub(super) locals: Vec<Kind>,
    /// Its operators, the final `end` included; none for an import.
    pub(super) ops: Vec<Op>,
    /// Each operator's offset in the module.
    pub(super) offsets: Vec<u64>,
    /// For each `block`, `loop`, `if` and `else`, where its `end` is.
    pub(super) ends: Vec<usize>,
    /// For each `if`, where its `else` is, when it has one.
    pub(super) elses: Vec<usize>,
}

/// None, in [`Function::ends`] and [`Function::elses`].
pub(super) const NOWHERE: usize = usize::MAX;

/// The program as the model runs it: the module lengua built from main.wat, with its library
/// merged in, and what the program's idea lets it reach.
#[derive(Debug)]
pub struct Module {
    /// Every function, the imports first, by index.
    pub(super) functions: Vec<Function>,
    /// Each global's width and first value.
    pub(super) globals: Vec<(u32, u128)>,
    /// The memory's pages when the program starts.
    pub(super) pages: u64,
    /// The most pages the memory may grow to: its declared maximum, or its size without one.
    pub(super) most_pages: u64,
    /// Each data segment's bytes, by index.
    pub(super) data: Vec<Vec<u8>>,
    /// Whether each data segment is active: written into memory at the start, and dropped then.
    pub(super) active: Vec<bool>,
    /// Memory's first bytes as the active data segments leave them; zeros past them.
    pub(super) initial: Vec<u8>,
    /// The function `_start` exports.
    pub(super) entry: u32,
    /// Whether the module exports its memory as `memory`, which every host call reads through.
    pub(super) exports_memory: bool,
    /// What the program's idea lets it reach.
    pub(super) policy: Policy,
    /// main.wat's line of each of its own operators, by offset.
    lines: BTreeMap<u64, usize>,
    /// Where main.wat's own functions' bodies are.
    bodies: Vec<(u64, u64)>,
    /// Whether some run has called each function, by index.
    entered: Vec<AtomicBool>,
}

/// A constant expression's value when it's one integer constant: its width and bits.
fn constant(expression: &ConstExpr<'_>) -> Option<(u32, u128)> {
    let mut reader = expression.get_operators_reader();
    let value = match reader.read().ok()? {
        Operator::I32Const { value } => (32, u128::from(value.cast_unsigned())),
        Operator::I64Const { value } => (64, u128::from(value.cast_unsigned())),
        _ => return None,
    };
    matches!(reader.read().ok()?, Operator::End).then_some(value)
}

/// What a block of type `ty` takes and leaves, with the module's function `types`.
fn arity(ty: BlockType, types: &[Option<(usize, usize)>]) -> Result<Arity, String> {
    if let BlockType::FuncType(index) = ty {
        let (params, results) = usize::try_from(index)
            .ok()
            .and_then(|index| types.get(index).copied().flatten())
            .ok_or("a block of a type the model doesn't follow")?;
        return Ok(Arity { params, results });
    }
    let results = usize::from(matches!(ty, BlockType::Type(_)));
    Ok(Arity { params: 0, results })
}

/// A load of `bytes` bytes, widened to `width` bits.
fn load(bytes: u32, width: u32, signed: bool, memarg: &MemArg) -> Op {
    Op::Load {
        bytes,
        width,
        signed,
        offset: memarg.offset,
    }
}

/// A store of `bytes` bytes.
fn store(bytes: u32, memarg: &MemArg) -> Op {
    Op::Store {
        bytes,
        offset: memarg.offset,
    }
}

/// The name of an operator the model doesn't follow, as `wasmparser` calls it.
fn variant(operator: &Operator<'_>) -> String {
    let text = format!("{operator:?}");
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .next()
        .unwrap_or("an operator")
        .to_owned()
}

/// `operator` in the model's form, its blocks' types read with the module's function `types`.
#[allow(clippy::too_many_lines)]
fn op_of(operator: &Operator<'_>, types: &[Option<(usize, usize)>]) -> Result<Op, String> {
    use Binary as B;
    use Unary as U;
    Ok(match operator {
        Operator::Unreachable => Op::Unreachable,
        Operator::Nop => Op::Nop,
        Operator::Block { blockty } => Op::Block(arity(*blockty, types)?),
        Operator::Loop { blockty } => Op::Loop(arity(*blockty, types)?),
        Operator::If { blockty } => Op::If(arity(*blockty, types)?),
        Operator::Else => Op::Else,
        Operator::End => Op::End,
        Operator::Br { relative_depth } => Op::Br(*relative_depth),
        Operator::BrIf { relative_depth } => Op::BrIf(*relative_depth),
        Operator::BrTable { targets } => {
            let depths = targets
                .targets()
                .collect::<Result<Vec<u32>, _>>()
                .map_err(|error| format!("the model couldn't read a br_table: {error}"))?;
            Op::BrTable(depths, targets.default())
        }
        Operator::Return => Op::Return,
        Operator::Call { function_index } => Op::Call(*function_index),
        Operator::Drop => Op::Drop,
        Operator::Select | Operator::TypedSelect { .. } => Op::Select,
        Operator::LocalGet { local_index } => Op::LocalGet(*local_index),
        Operator::LocalSet { local_index } => Op::LocalSet(*local_index),
        Operator::LocalTee { local_index } => Op::LocalTee(*local_index),
        Operator::GlobalGet { global_index } => Op::GlobalGet(*global_index),
        Operator::GlobalSet { global_index } => Op::GlobalSet(*global_index),
        Operator::I32Load { memarg } => load(4, 32, false, memarg),
        Operator::I64Load { memarg } => load(8, 64, false, memarg),
        Operator::I32Load8S { memarg } => load(1, 32, true, memarg),
        Operator::I32Load8U { memarg } => load(1, 32, false, memarg),
        Operator::I32Load16S { memarg } => load(2, 32, true, memarg),
        Operator::I32Load16U { memarg } => load(2, 32, false, memarg),
        Operator::I64Load8S { memarg } => load(1, 64, true, memarg),
        Operator::I64Load8U { memarg } => load(1, 64, false, memarg),
        Operator::I64Load16S { memarg } => load(2, 64, true, memarg),
        Operator::I64Load16U { memarg } => load(2, 64, false, memarg),
        Operator::I64Load32S { memarg } => load(4, 64, true, memarg),
        Operator::I64Load32U { memarg } => load(4, 64, false, memarg),
        Operator::I32Store { memarg } => store(4, memarg),
        Operator::I64Store { memarg } => store(8, memarg),
        Operator::I32Store8 { memarg } | Operator::I64Store8 { memarg } => store(1, memarg),
        Operator::I32Store16 { memarg } | Operator::I64Store16 { memarg } => store(2, memarg),
        Operator::I64Store32 { memarg } => store(4, memarg),
        Operator::MemorySize { .. } => Op::MemorySize,
        Operator::MemoryGrow { .. } => Op::MemoryGrow,
        Operator::MemoryCopy { .. } => Op::MemoryCopy,
        Operator::MemoryFill { .. } => Op::MemoryFill,
        Operator::MemoryInit { data_index, .. } => Op::MemoryInit(*data_index),
        Operator::DataDrop { data_index } => Op::DataDrop(*data_index),
        Operator::I32Const { value } => Op::Const {
            width: 32,
            value: u128::from(value.cast_unsigned()),
        },
        Operator::I64Const { value } => Op::Const {
            width: 64,
            value: u128::from(value.cast_unsigned()),
        },
        Operator::I32Eqz => Op::Unary(U::Eqz),
        Operator::I32Eq => Op::Binary(B::Eq),
        Operator::I32Ne => Op::Binary(B::Ne),
        Operator::I32LtS => Op::Binary(B::LtS),
        Operator::I32LtU => Op::Binary(B::LtU),
        Operator::I32GtS => Op::Binary(B::GtS),
        Operator::I32GtU => Op::Binary(B::GtU),
        Operator::I32LeS => Op::Binary(B::LeS),
        Operator::I32LeU => Op::Binary(B::LeU),
        Operator::I32GeS => Op::Binary(B::GeS),
        Operator::I32GeU => Op::Binary(B::GeU),
        Operator::I64Eqz => Op::Unary(U::Eqz),
        Operator::I64Eq => Op::Binary(B::Eq),
        Operator::I64Ne => Op::Binary(B::Ne),
        Operator::I64LtS => Op::Binary(B::LtS),
        Operator::I64LtU => Op::Binary(B::LtU),
        Operator::I64GtS => Op::Binary(B::GtS),
        Operator::I64GtU => Op::Binary(B::GtU),
        Operator::I64LeS => Op::Binary(B::LeS),
        Operator::I64LeU => Op::Binary(B::LeU),
        Operator::I64GeS => Op::Binary(B::GeS),
        Operator::I64GeU => Op::Binary(B::GeU),
        Operator::I32Clz => Op::Unary(U::Clz),
        Operator::I32Ctz => Op::Unary(U::Ctz),
        Operator::I32Popcnt => Op::Unary(U::Popcnt),
        Operator::I32Add => Op::Binary(B::Add),
        Operator::I32Sub => Op::Binary(B::Sub),
        Operator::I32Mul => Op::Binary(B::Mul),
        Operator::I32DivS => Op::Binary(B::DivS),
        Operator::I32DivU => Op::Binary(B::DivU),
        Operator::I32RemS => Op::Binary(B::RemS),
        Operator::I32RemU => Op::Binary(B::RemU),
        Operator::I32And => Op::Binary(B::And),
        Operator::I32Or => Op::Binary(B::Or),
        Operator::I32Xor => Op::Binary(B::Xor),
        Operator::I32Shl => Op::Binary(B::Shl),
        Operator::I32ShrS => Op::Binary(B::ShrS),
        Operator::I32ShrU => Op::Binary(B::ShrU),
        Operator::I32Rotl => Op::Binary(B::Rotl),
        Operator::I32Rotr => Op::Binary(B::Rotr),
        Operator::I64Clz => Op::Unary(U::Clz),
        Operator::I64Ctz => Op::Unary(U::Ctz),
        Operator::I64Popcnt => Op::Unary(U::Popcnt),
        Operator::I64Add => Op::Binary(B::Add),
        Operator::I64Sub => Op::Binary(B::Sub),
        Operator::I64Mul => Op::Binary(B::Mul),
        Operator::I64DivS => Op::Binary(B::DivS),
        Operator::I64DivU => Op::Binary(B::DivU),
        Operator::I64RemS => Op::Binary(B::RemS),
        Operator::I64RemU => Op::Binary(B::RemU),
        Operator::I64And => Op::Binary(B::And),
        Operator::I64Or => Op::Binary(B::Or),
        Operator::I64Xor => Op::Binary(B::Xor),
        Operator::I64Shl => Op::Binary(B::Shl),
        Operator::I64ShrS => Op::Binary(B::ShrS),
        Operator::I64ShrU => Op::Binary(B::ShrU),
        Operator::I64Rotl => Op::Binary(B::Rotl),
        Operator::I64Rotr => Op::Binary(B::Rotr),
        Operator::I32WrapI64 => Op::Unary(U::Wrap),
        Operator::I64ExtendI32S => Op::Unary(U::Signed),
        Operator::I64ExtendI32U => Op::Unary(U::Unsigned),
        Operator::I32Extend8S => Op::Unary(U::Extend8),
        Operator::I32Extend16S => Op::Unary(U::Extend16),
        Operator::I64Extend8S => Op::Unary(U::Extend8),
        Operator::I64Extend16S => Op::Unary(U::Extend16),
        Operator::I64Extend32S => Op::Unary(U::Extend32),
        other => Op::Other(variant(other)),
    })
}

/// For each `block`, `loop`, `if` and `else` among `ops`, the place of its `end`, and for each
/// `if`, the place of its `else`: [`NOWHERE`] for the rest.
fn blocks(ops: &[Op]) -> Result<(Vec<usize>, Vec<usize>), String> {
    let mut ends = vec![NOWHERE; ops.len()];
    let mut elses = vec![NOWHERE; ops.len()];
    let mut open: Vec<usize> = Vec::new();
    for (at, op) in ops.iter().enumerate() {
        match op {
            Op::Block(_) | Op::Loop(_) | Op::If(_) => open.push(at),
            Op::Else => {
                let start = *open.last().ok_or("an `else` outside an `if`")?;
                elses[start] = at;
            }
            Op::End => {
                if let Some(start) = open.pop() {
                    ends[start] = at;
                    if elses[start] != NOWHERE {
                        ends[elses[start]] = at;
                    }
                }
            }
            _ => {}
        }
    }
    Ok((ends, elses))
}

impl Module {
    /// Reads the module `built` holds, for a program whose idea lets it reach what `policy`
    /// allows.
    ///
    /// # Errors
    ///
    /// What the model can't run: a module it can't read, an import lengua's host doesn't give, a
    /// start function, a value of a type it doesn't hold, or data past the memory.
    #[allow(clippy::too_many_lines)]
    pub fn new(built: &Built, policy: Policy) -> Result<Self, String> {
        let failed = |error: wasmparser::BinaryReaderError| {
            format!("the model couldn't read the module: {error}")
        };
        // Each type's parameters and results, when it's a function's.
        let mut types: Vec<Option<(usize, usize)>> = Vec::new();
        let mut imports: Vec<(String, String, u32)> = Vec::new();
        let mut defined: Vec<u32> = Vec::new();
        let mut memory: Option<(u64, Option<u64>)> = None;
        let mut globals = Vec::new();
        let mut entry = None;
        let mut exports_memory = false;
        let mut segments: Vec<(Vec<u8>, Option<u64>)> = Vec::new();
        let mut bodies = Vec::new();
        let mut names: BTreeMap<u32, String> = BTreeMap::new();
        for payload in Parser::new(0).parse_all(&built.wasm) {
            match payload.map_err(failed)? {
                Payload::TypeSection(reader) => {
                    for group in reader {
                        for sub in group.map_err(failed)?.into_types() {
                            types.push(match &sub.composite_type.inner {
                                CompositeInnerType::Func(func) => {
                                    Some((func.params().len(), func.results().len()))
                                }
                                _ => None,
                            });
                        }
                    }
                }
                Payload::ImportSection(reader) => {
                    for import in reader.into_imports() {
                        let import = import.map_err(failed)?;
                        let (TypeRef::Func(index) | TypeRef::FuncExact(index)) = import.ty else {
                            return Err(format!(
                                "it imports {}.{}, which isn't a function lengua's host gives",
                                import.module, import.name
                            ));
                        };
                        imports.push((import.module.to_owned(), import.name.to_owned(), index));
                    }
                }
                Payload::FunctionSection(reader) => {
                    for index in reader {
                        defined.push(index.map_err(failed)?);
                    }
                }
                Payload::MemorySection(reader) => {
                    for found in reader {
                        let found = found.map_err(failed)?;
                        memory = Some((found.initial, found.maximum));
                    }
                }
                Payload::GlobalSection(reader) => {
                    for global in reader {
                        let global = global.map_err(failed)?;
                        let first = constant(&global.init_expr).ok_or(
                            "a global whose first value isn't an integer constant, which the \
                             model doesn't start",
                        )?;
                        globals.push(first);
                    }
                }
                Payload::ExportSection(reader) => {
                    for export in reader {
                        let export = export.map_err(failed)?;
                        if export.name == "_start" && matches!(export.kind, ExternalKind::Func) {
                            entry = Some(export.index);
                        }
                        if export.name == "memory" && matches!(export.kind, ExternalKind::Memory)
                        {
                            exports_memory = true;
                        }
                    }
                }
                Payload::StartSection { .. } => {
                    return Err(
                        "a start function, which the model doesn't run before `_start`".into(),
                    );
                }
                Payload::DataSection(reader) => {
                    for segment in reader {
                        let segment = segment.map_err(failed)?;
                        let at = match &segment.kind {
                            DataKind::Active { offset_expr, .. } => {
                                let (_, address) = constant(offset_expr)
                                    .ok_or("a data segment at a place that isn't a constant")?;
                                Some(
                                    u64::try_from(address)
                                        .map_err(|_| "a data segment past the memory")?,
                                )
                            }
                            _ => None,
                        };
                        segments.push((segment.data.to_vec(), at));
                    }
                }
                Payload::CodeSectionEntry(body) => bodies.push(body),
                Payload::CustomSection(reader) => {
                    if let KnownCustom::Name(section) = reader.as_known() {
                        for subsection in section.flatten() {
                            if let Name::Function(map) = subsection {
                                for naming in map.flatten() {
                                    names.insert(naming.index, naming.name.to_owned());
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let entry = entry.ok_or("the module exports no `_start`")?;
        let (pages, maximum) = memory.unwrap_or((0, Some(0)));
        let most_pages = maximum.unwrap_or(pages).min(MOST_PAGES);
        let memory_bytes = pages.saturating_mul(PAGE);

        // The data segments: the active ones written into memory's first bytes, in order.
        let mut initial: Vec<u8> = Vec::new();
        let mut data = Vec::new();
        let mut active = Vec::new();
        for (bytes, at) in segments {
            if let Some(at) = at {
                let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
                let end = at
                    .checked_add(length)
                    .filter(|end| *end <= memory_bytes)
                    .ok_or("a data segment past the memory's end, which the engine won't start")?;
                let (start, end) = (
                    usize::try_from(at).map_err(|_| "a data segment past the memory")?,
                    usize::try_from(end).map_err(|_| "a data segment past the memory")?,
                );
                if initial.len() < end {
                    initial.resize(end, 0);
                }
                initial[start..end].copy_from_slice(&bytes);
            }
            active.push(at.is_some());
            data.push(bytes);
        }

        let signature = |index: u32| -> Result<(usize, usize), String> {
            usize::try_from(index)
                .ok()
                .and_then(|index| types.get(index).copied().flatten())
                .ok_or_else(|| "a function of a type the model doesn't follow".to_owned())
        };
        let mut functions = Vec::new();
        for (module, name, index) in &imports {
            let (params, results) = signature(*index)?;
            let host = (module.as_str() == wasi::MODULE)
                .then(|| wasi::function(name))
                .flatten()
                .ok_or_else(|| {
                    format!("it imports {module}.{name}, which lengua's host doesn't give")
                })?;
            if host.params.len() != params || host.results.len() != results {
                return Err(format!(
                    "it imports {name} with a signature other than WASI's, which the engine won't \
                     link"
                ));
            }
            functions.push(Function {
                name: Some(name.clone()),
                params,
                results,
                own: false,
                import: Some(host),
                locals: Vec::new(),
                ops: Vec::new(),
                offsets: Vec::new(),
                ends: Vec::new(),
                elses: Vec::new(),
            });
        }
        if defined.len() != bodies.len() {
            return Err("the module's functions and their bodies don't match".into());
        }
        let mut library = built.library.iter();
        for (type_index, body) in defined.iter().zip(&bodies) {
            let index = u32::try_from(functions.len()).map_err(|_| "too many functions")?;
            let (params, results) = signature(*type_index)?;
            let range = body.range();
            let own = built.bodies.contains(&(range.start, range.end));
            // The library's functions come after main.wat's, in the order it merged them.
            let merged = if own { None } else { library.next().cloned() };
            let name = names.get(&index).cloned().or(merged);
            let mut locals = Vec::new();
            for declared in body.get_locals_reader().map_err(failed)? {
                let (count, value) = declared.map_err(failed)?;
                let kind = Kind::of(value)
                    .ok_or("a local of a reference type, which the model doesn't hold")?;
                locals.extend(std::iter::repeat_n(
                    kind,
                    usize::try_from(count).unwrap_or(0),
                ));
            }
            let mut reader = body.get_operators_reader().map_err(failed)?;
            let mut ops = Vec::new();
            let mut offsets = Vec::new();
            while !reader.eof() {
                let (operator, offset) = reader.read_with_offset().map_err(failed)?;
                ops.push(op_of(&operator, &types)?);
                offsets.push(offset);
            }
            let (ends, elses) = blocks(&ops)?;
            functions.push(Function {
                name,
                params,
                results,
                own,
                import: None,
                locals,
                ops,
                offsets,
                ends,
                elses,
            });
        }
        let starts = usize::try_from(entry)
            .ok()
            .and_then(|index| functions.get(index))
            .is_some_and(|function| {
                function.import.is_none() && function.params == 0 && function.results == 0
            });
        if !starts {
            return Err(
                "its `_start` isn't a function of its own that takes and gives nothing".into(),
            );
        }
        let entered = functions.iter().map(|_| AtomicBool::new(false)).collect();
        Ok(Self {
            functions,
            globals,
            pages,
            most_pages,
            data,
            active,
            initial,
            entry,
            exports_memory,
            policy,
            lines: built.lines.clone(),
            bodies: built.bodies.clone(),
            entered,
        })
    }

    /// The function with index `index`.
    pub(super) fn function(&self, index: u32) -> Option<&Function> {
        self.functions.get(usize::try_from(index).ok()?)
    }

    /// main.wat's line of operator `at` of the function `function`, when it's one of main.wat's
    /// own.
    pub(super) fn line(&self, function: u32, at: usize) -> Option<usize> {
        let found = self.function(function).filter(|found| found.own)?;
        let offset = *found.offsets.get(at)?;
        crate::wasm::text::line_at(&self.lines, &self.bodies, offset)
    }

    /// Notes that a run called the function `function`.
    pub(super) fn enter(&self, function: u32) {
        if let Some(entered) = usize::try_from(function)
            .ok()
            .and_then(|index| self.entered.get(index))
        {
            entered.store(true, Ordering::Relaxed);
        }
    }

    /// main.wat's functions no run has called, in the module's order, but `_start`, where every
    /// run begins: a proof says nothing of what they do.
    #[must_use]
    pub fn unreached(&self) -> Vec<String> {
        self.functions
            .iter()
            .zip(&self.entered)
            .enumerate()
            .filter(|(index, (function, entered))| {
                function.own
                    && usize::try_from(self.entry).ok() != Some(*index)
                    && !entered.load(Ordering::Relaxed)
            })
            .map(|(index, (function, _))| {
                function
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("function {index}"))
            })
            .collect()
    }
}
