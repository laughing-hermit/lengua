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

/// The most bytes of the engine's stack the model lets a run's calls take: half of what the
/// engine holds them to, as the model's count of a frame is an estimate, however generous.
const STACK: u64 = crate::wasm::engine::MAX_WASM_STACK as u64 / 2;

/// The bytes the model counts for a frame beside its values: the return address and the frame's
/// link, the registers a function keeps for its caller (at most 224 bytes, on Windows), the
/// arguments a call passes on the stack, and the trampoline a host call goes through.
const FRAME: u64 = 1024;

/// The bytes the model counts for each value a frame may hold: a vector's 16, the widest.
const SLOT: u64 = 16;

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

/// An operator on floats, of the width its operand has, 32 or 64 bits: arithmetic, the
/// comparisons, which give an i32, and the conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Float {
    /// `add`.
    Add,
    /// `sub`.
    Sub,
    /// `mul`.
    Mul,
    /// `div`.
    Div,
    /// `min`: a NaN beside anything is NaN, and -0 is below +0.
    Min,
    /// `max`.
    Max,
    /// `copysign`: the first's magnitude with the second's sign.
    Copysign,
    /// `abs`: the sign bit cleared.
    Abs,
    /// `neg`: the sign bit flipped.
    Neg,
    /// `sqrt`.
    Sqrt,
    /// `ceil`.
    Ceil,
    /// `floor`.
    Floor,
    /// `trunc`.
    Trunc,
    /// `nearest`: to the nearest whole float, ties to even.
    Nearest,
    /// `eq`.
    Eq,
    /// `ne`: true for a NaN.
    Ne,
    /// `lt`.
    Lt,
    /// `gt`.
    Gt,
    /// `le`.
    Le,
    /// `ge`.
    Ge,
    /// `convert`: a whole number of the operand's width as the nearest float of `to` bits.
    Convert {
        /// The float's bits.
        to: u32,
        /// Whether the number is read signed.
        signed: bool,
    },
    /// `demote` and `promote`: the nearest float of `to` bits.
    Resize {
        /// The float's bits.
        to: u32,
    },
    /// `trunc_sat`: toward zero, to a whole number of `to` bits that saturates at either end,
    /// 0 for a NaN.
    TruncSat {
        /// The number's bits.
        to: u32,
        /// Whether it's signed.
        signed: bool,
    },
}

/// An operator of SIMD128 on the lanes of two vectors, `width` bits each: the result's lane is
/// the operator's on the two lanes in its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lane {
    /// `add`, wrapping.
    Add,
    /// `sub`, wrapping.
    Sub,
    /// `mul`, wrapping.
    Mul,
    /// `min_s`.
    MinS,
    /// `min_u`.
    MinU,
    /// `max_s`.
    MaxS,
    /// `max_u`.
    MaxU,
    /// `add_sat_s`: held between the least and the most a lane holds, signed.
    AddSatS,
    /// `add_sat_u`.
    AddSatU,
    /// `sub_sat_s`.
    SubSatS,
    /// `sub_sat_u`.
    SubSatU,
    /// `avgr_u`: the two's sum plus one, halved, without wrapping.
    AvgrU,
    /// A comparison, as the integer one is: all ones in the lane where it holds, else zeros.
    Compare(Binary),
}

/// A SIMD128 operator: a 128-bit vector's lanes as terms, each lane `width` bits wide, 8, 16, 32
/// or 64, lane 0 its lowest bits, as memory's lowest bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Vector {
    /// `splat`: a scalar's low `width` bits in every lane.
    Splat(u32),
    /// `extract_lane`: a lane, widened to an i32 signed or not where it's narrower.
    Extract {
        /// The lanes' width.
        width: u32,
        /// Which lane.
        lane: u8,
        /// Whether a narrow lane is sign-extended.
        signed: bool,
    },
    /// `replace_lane`: a lane set to a scalar's low bits.
    Replace {
        /// The lanes' width.
        width: u32,
        /// Which lane.
        lane: u8,
    },
    /// `v128.not`.
    Not,
    /// `v128.and`.
    And,
    /// `v128.andnot`: the first and not the second.
    AndNot,
    /// `v128.or`.
    Or,
    /// `v128.xor`.
    Xor,
    /// `v128.bitselect`: the first's bits where the third's are set, the second's elsewhere.
    Bitselect,
    /// `v128.any_true`: 1 where any bit is set.
    AnyTrue,
    /// `all_true`: 1 where every lane of `width` bits isn't zero.
    AllTrue(u32),
    /// `bitmask`: each lane's top bit, lane 0's lowest, as an i32.
    Bitmask(u32),
    /// An operator lane by lane on two vectors.
    Lanes {
        /// The lanes' width.
        width: u32,
        /// The operator.
        op: Lane,
    },
    /// `neg`, wrapping, lane by lane.
    Neg(u32),
    /// `abs`, wrapping for the least value, lane by lane.
    Abs(u32),
    /// A shift of every lane by a scalar count, modulo the lane's width: `shl`, `shr_s` or
    /// `shr_u` as [`Binary`] names them.
    Shift {
        /// The lanes' width.
        width: u32,
        /// `Shl`, `ShrS` or `ShrU`.
        op: Binary,
    },
    /// `i8x16.shuffle`: each lane a byte of the two vectors, 0 to 15 the first's and 16 to 31
    /// the second's.
    Shuffle([u8; 16]),
    /// `i8x16.swizzle`: each lane the first vector's byte the second's lane picks, or zero for a
    /// pick past 15.
    Swizzle,
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
    /// An operator on floats.
    Float(Float),
    /// A SIMD128 operator.
    Vector(Vector),
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
    /// The most pages the memory may grow to, as the engine holds it: its declared maximum, or
    /// without one, its size if it's exported as `memory`, and else the most a 32-bit memory has.
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
    /// lengua's library's `$fill`, by index, when the module calls it.
    pub(super) fill: Option<u32>,
    /// How the module reads.
    reading: Reading,
    /// main.wat's line of each of its own operators, by offset.
    lines: BTreeMap<u64, usize>,
    /// Where main.wat's own functions' bodies are.
    bodies: Vec<(u64, u64)>,
    /// Whether some run has called each function, by index.
    entered: Vec<AtomicBool>,
    /// Whether the program may change each global, by index.
    pub(super) mutable: Vec<bool>,
    /// What the contracts of the functions the program took say each gives, by the function's
    /// index: a run takes one at a call, in place of the function's body.
    pub(super) claims: BTreeMap<u32, super::unit::Held>,
}

/// How a module reads, as every way its input can be cut and a proof by induction need to know
/// it.
#[derive(Debug, Clone)]
struct Reading {
    /// Whether every read it makes goes through lengua's `$fill`, each with a stop byte known
    /// before it runs: no function but `$fill` calls `fd_read`, and none waits on `poll_oneoff`.
    whole: bool,
    /// The call of `$fill` in a loop of main.wat's own code, by function and operator, or why
    /// there's none a proof by induction takes.
    in_a_loop: Result<(u32, usize), String>,
}

/// Whether `stop`, the operator that leaves `$fill`'s stop operand, leaves a value known before
/// the program runs: a constant, or a global the module never changes.
fn stop_is_known(stop: Option<&Op>, mutable: &[bool]) -> bool {
    match stop {
        Some(Op::Const { width: 32, .. }) => true,
        Some(Op::GlobalGet(index)) => {
            usize::try_from(*index).is_ok_and(|index| mutable.get(index) == Some(&false))
        }
        _ => false,
    }
}

/// How the module of `functions` reads, `$fill` being the function `fill`, its globals'
/// mutability `mutable`.
fn reading(functions: &[Function], fill: Option<u32>, mutable: &[bool]) -> Reading {
    let imports = |index: u32, name: &str| {
        usize::try_from(index)
            .ok()
            .and_then(|index| functions.get(index))
            .and_then(|function| function.import)
            .is_some_and(|import| import.name == name)
    };
    let mut whole = fill.is_some();
    // Each read in a loop of main.wat's own: its function, its operator, and whether it's an
    // fd_read of the program's own.
    let mut looped: Vec<(u32, usize, bool)> = Vec::new();
    for (number, function) in (0_u32..).zip(functions) {
        // Whether each block the operator is in is a loop.
        let mut open: Vec<bool> = Vec::new();
        for (at, op) in function.ops.iter().enumerate() {
            match op {
                Op::Block(_) | Op::If(_) => open.push(false),
                Op::Loop(_) => open.push(true),
                Op::End => {
                    open.pop();
                }
                Op::Call(callee) => {
                    let raw = imports(*callee, "fd_read") && Some(number) != fill;
                    let fills = Some(*callee) == fill;
                    let before = at
                        .checked_sub(1)
                        .and_then(|before| function.ops.get(before));
                    if raw
                        || imports(*callee, "poll_oneoff")
                        || (fills && !stop_is_known(before, mutable))
                    {
                        whole = false;
                    }
                    if function.own && open.contains(&true) && (raw || fills) {
                        looped.push((number, at, raw));
                    }
                }
                _ => {}
            }
        }
    }
    let in_a_loop = match looped.as_slice() {
        [] => Err(NO_READ_LOOP.to_owned()),
        [(_, _, true)] => Err(RAW_READ_LOOP.to_owned()),
        [(function, at, false)] => Ok((*function, *at)),
        _ => Err(READ_LOOPS.to_owned()),
    };
    Reading { whole, in_a_loop }
}

/// Why a module that reads in no loop isn't proven by induction.
const NO_READ_LOOP: &str = "it reads in no loop of its own, which a buffer-full at a time needs";

/// Why one whose loop reads with its own fd_read isn't.
const RAW_READ_LOOP: &str = "its read loop reads with fd_read itself, whose reads see how the \
                             input is cut: read with $fill to be proven for every length";

/// Why one that reads in several places in its loops isn't.
const READ_LOOPS: &str =
    "it reads in more than one place in its loops, which the induction doesn't take";

/// Why one that reads other than through `$fill` isn't.
const NOT_WHOLE: &str = "it reads other than through $fill with a stop it knows, or waits on \
                         poll_oneoff, or is on the network, so its reads see how its input is \
                         cut";

/// A constant expression's value when it's one constant of a number or a vector: its width and
/// bits, a float's and a vector's as they are.
fn constant(expression: &ConstExpr<'_>) -> Option<(u32, u128)> {
    let mut reader = expression.get_operators_reader();
    let value = match reader.read().ok()? {
        Operator::I32Const { value } => (32, u128::from(value.cast_unsigned())),
        Operator::I64Const { value } => (64, u128::from(value.cast_unsigned())),
        Operator::F32Const { value } => (32, u128::from(value.bits())),
        Operator::F64Const { value } => (64, u128::from(value.bits())),
        Operator::V128Const { value } => (128, u128::from(value)),
        _ => return None,
    };
    matches!(reader.read().ok()?, Operator::End).then_some(value)
}

/// A function type: its parameters' value types and its results'.
type Signature = (Vec<ValType>, Vec<ValType>);

/// The function type `index` among the module's `types`.
fn signature(types: &[Option<Signature>], index: u32) -> Result<&Signature, String> {
    usize::try_from(index)
        .ok()
        .and_then(|index| types.get(index))
        .and_then(Option::as_ref)
        .ok_or_else(|| "a function of a type the model doesn't follow".to_owned())
}

/// The value type WASI's `value` is, as a module imports it.
const fn value_type(value: wasi::Value) -> ValType {
    match value {
        wasi::Value::I32 => ValType::I32,
        wasi::Value::I64 => ValType::I64,
    }
}

/// Whether an import's value types, `found`, are WASI's, `wanted`, one for one: the engine links
/// an import only to a function of exactly its type, as lengua's host defines each.
fn same_values(wanted: &[wasi::Value], found: &[ValType]) -> bool {
    let types: Vec<ValType> = wanted.iter().copied().map(value_type).collect();
    types == found
}

/// What a block of type `ty` takes and leaves, with the module's function `types`.
fn arity(ty: BlockType, types: &[Option<Signature>]) -> Result<Arity, String> {
    if let BlockType::FuncType(index) = ty {
        let Ok((params, results)) = signature(types, index) else {
            return Err("a block of a type the model doesn't follow".into());
        };
        return Ok(Arity {
            params: params.len(),
            results: results.len(),
        });
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
fn op_of(operator: &Operator<'_>, types: &[Option<Signature>]) -> Result<Op, String> {
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
        other => float_of(other)
            .or_else(|| vector_of(other))
            .unwrap_or_else(|| Op::Other(variant(other))),
    })
}

/// `operator` in the model's form when it's on floats: their loads and stores, which move their
/// bits, constants, arithmetic, comparisons and conversions. A reinterpretation leaves the bits
/// as they are. The trapping conversions to whole numbers the dialect has none of, and the model
/// doesn't follow them.
fn float_of(operator: &Operator<'_>) -> Option<Op> {
    use Float as F;
    let float = |op| Some(Op::Float(op));
    match operator {
        Operator::F32Load { memarg } => Some(load(4, 32, false, memarg)),
        Operator::F64Load { memarg } => Some(load(8, 64, false, memarg)),
        Operator::F32Store { memarg } => Some(store(4, memarg)),
        Operator::F64Store { memarg } => Some(store(8, memarg)),
        Operator::F32Const { value } => Some(Op::Const {
            width: 32,
            value: u128::from(value.bits()),
        }),
        Operator::F64Const { value } => Some(Op::Const {
            width: 64,
            value: u128::from(value.bits()),
        }),
        Operator::I32ReinterpretF32
        | Operator::I64ReinterpretF64
        | Operator::F32ReinterpretI32
        | Operator::F64ReinterpretI64 => Some(Op::Nop),
        Operator::F32Add | Operator::F64Add => float(F::Add),
        Operator::F32Sub | Operator::F64Sub => float(F::Sub),
        Operator::F32Mul | Operator::F64Mul => float(F::Mul),
        Operator::F32Div | Operator::F64Div => float(F::Div),
        Operator::F32Min | Operator::F64Min => float(F::Min),
        Operator::F32Max | Operator::F64Max => float(F::Max),
        Operator::F32Copysign | Operator::F64Copysign => float(F::Copysign),
        Operator::F32Abs | Operator::F64Abs => float(F::Abs),
        Operator::F32Neg | Operator::F64Neg => float(F::Neg),
        Operator::F32Sqrt | Operator::F64Sqrt => float(F::Sqrt),
        Operator::F32Ceil | Operator::F64Ceil => float(F::Ceil),
        Operator::F32Floor | Operator::F64Floor => float(F::Floor),
        Operator::F32Trunc | Operator::F64Trunc => float(F::Trunc),
        Operator::F32Nearest | Operator::F64Nearest => float(F::Nearest),
        Operator::F32Eq | Operator::F64Eq => float(F::Eq),
        Operator::F32Ne | Operator::F64Ne => float(F::Ne),
        Operator::F32Lt | Operator::F64Lt => float(F::Lt),
        Operator::F32Gt | Operator::F64Gt => float(F::Gt),
        Operator::F32Le | Operator::F64Le => float(F::Le),
        Operator::F32Ge | Operator::F64Ge => float(F::Ge),
        Operator::F32ConvertI32S | Operator::F32ConvertI64S => float(F::Convert {
            to: 32,
            signed: true,
        }),
        Operator::F32ConvertI32U | Operator::F32ConvertI64U => float(F::Convert {
            to: 32,
            signed: false,
        }),
        Operator::F64ConvertI32S | Operator::F64ConvertI64S => float(F::Convert {
            to: 64,
            signed: true,
        }),
        Operator::F64ConvertI32U | Operator::F64ConvertI64U => float(F::Convert {
            to: 64,
            signed: false,
        }),
        Operator::F32DemoteF64 => float(F::Resize { to: 32 }),
        Operator::F64PromoteF32 => float(F::Resize { to: 64 }),
        Operator::I32TruncSatF32S | Operator::I32TruncSatF64S => float(F::TruncSat {
            to: 32,
            signed: true,
        }),
        Operator::I32TruncSatF32U | Operator::I32TruncSatF64U => float(F::TruncSat {
            to: 32,
            signed: false,
        }),
        Operator::I64TruncSatF32S | Operator::I64TruncSatF64S => float(F::TruncSat {
            to: 64,
            signed: true,
        }),
        Operator::I64TruncSatF32U | Operator::I64TruncSatF64U => float(F::TruncSat {
            to: 64,
            signed: false,
        }),
        _ => None,
    }
}

/// `operator` in the model's form when it's one of SIMD128's the model follows: a vector's loads
/// and stores and constants, the lanes' splats, extractions and replacements, the bitwise
/// operators, the integer lanes' arithmetic, comparisons, shifts, tests and bitmasks, and the
/// shuffles. The float lanes' arithmetic, the lanes' narrowing, widening and pairwise forms, and
/// the loads of a lane, the model doesn't follow.
#[allow(clippy::too_many_lines)]
fn vector_of(operator: &Operator<'_>) -> Option<Op> {
    use Binary as B;
    use Lane as L;
    let vector = |op| Some(Op::Vector(op));
    let lanes = |width, op| Some(Op::Vector(Vector::Lanes { width, op }));
    let compare = |width, op| lanes(width, L::Compare(op));
    let shift = |width, op| Some(Op::Vector(Vector::Shift { width, op }));
    let extract = |width, lane: &u8, signed| {
        Some(Op::Vector(Vector::Extract {
            width,
            lane: *lane,
            signed,
        }))
    };
    let replace = |width, lane: &u8| Some(Op::Vector(Vector::Replace { width, lane: *lane }));
    match operator {
        Operator::V128Load { memarg } => Some(load(16, 128, false, memarg)),
        Operator::V128Store { memarg } => Some(store(16, memarg)),
        Operator::V128Const { value } => Some(Op::Const {
            width: 128,
            value: u128::from(*value),
        }),
        Operator::I8x16Splat => vector(Vector::Splat(8)),
        Operator::I16x8Splat => vector(Vector::Splat(16)),
        Operator::I32x4Splat | Operator::F32x4Splat => vector(Vector::Splat(32)),
        Operator::I64x2Splat | Operator::F64x2Splat => vector(Vector::Splat(64)),
        Operator::I8x16ExtractLaneS { lane } => extract(8, lane, true),
        Operator::I8x16ExtractLaneU { lane } => extract(8, lane, false),
        Operator::I16x8ExtractLaneS { lane } => extract(16, lane, true),
        Operator::I16x8ExtractLaneU { lane } => extract(16, lane, false),
        Operator::I32x4ExtractLane { lane } | Operator::F32x4ExtractLane { lane } => {
            extract(32, lane, false)
        }
        Operator::I64x2ExtractLane { lane } | Operator::F64x2ExtractLane { lane } => {
            extract(64, lane, false)
        }
        Operator::I8x16ReplaceLane { lane } => replace(8, lane),
        Operator::I16x8ReplaceLane { lane } => replace(16, lane),
        Operator::I32x4ReplaceLane { lane } | Operator::F32x4ReplaceLane { lane } => {
            replace(32, lane)
        }
        Operator::I64x2ReplaceLane { lane } | Operator::F64x2ReplaceLane { lane } => {
            replace(64, lane)
        }
        Operator::V128Not => vector(Vector::Not),
        Operator::V128And => vector(Vector::And),
        Operator::V128AndNot => vector(Vector::AndNot),
        Operator::V128Or => vector(Vector::Or),
        Operator::V128Xor => vector(Vector::Xor),
        Operator::V128Bitselect => vector(Vector::Bitselect),
        Operator::V128AnyTrue => vector(Vector::AnyTrue),
        Operator::I8x16AllTrue => vector(Vector::AllTrue(8)),
        Operator::I16x8AllTrue => vector(Vector::AllTrue(16)),
        Operator::I32x4AllTrue => vector(Vector::AllTrue(32)),
        Operator::I64x2AllTrue => vector(Vector::AllTrue(64)),
        Operator::I8x16Bitmask => vector(Vector::Bitmask(8)),
        Operator::I16x8Bitmask => vector(Vector::Bitmask(16)),
        Operator::I32x4Bitmask => vector(Vector::Bitmask(32)),
        Operator::I64x2Bitmask => vector(Vector::Bitmask(64)),
        Operator::I8x16Shuffle { lanes } => vector(Vector::Shuffle(*lanes)),
        Operator::I8x16Swizzle => vector(Vector::Swizzle),
        Operator::I8x16Neg => vector(Vector::Neg(8)),
        Operator::I16x8Neg => vector(Vector::Neg(16)),
        Operator::I32x4Neg => vector(Vector::Neg(32)),
        Operator::I64x2Neg => vector(Vector::Neg(64)),
        Operator::I8x16Abs => vector(Vector::Abs(8)),
        Operator::I16x8Abs => vector(Vector::Abs(16)),
        Operator::I32x4Abs => vector(Vector::Abs(32)),
        Operator::I64x2Abs => vector(Vector::Abs(64)),
        Operator::I8x16Shl => shift(8, B::Shl),
        Operator::I8x16ShrS => shift(8, B::ShrS),
        Operator::I8x16ShrU => shift(8, B::ShrU),
        Operator::I16x8Shl => shift(16, B::Shl),
        Operator::I16x8ShrS => shift(16, B::ShrS),
        Operator::I16x8ShrU => shift(16, B::ShrU),
        Operator::I32x4Shl => shift(32, B::Shl),
        Operator::I32x4ShrS => shift(32, B::ShrS),
        Operator::I32x4ShrU => shift(32, B::ShrU),
        Operator::I64x2Shl => shift(64, B::Shl),
        Operator::I64x2ShrS => shift(64, B::ShrS),
        Operator::I64x2ShrU => shift(64, B::ShrU),
        Operator::I8x16Add => lanes(8, L::Add),
        Operator::I8x16Sub => lanes(8, L::Sub),
        Operator::I8x16AddSatS => lanes(8, L::AddSatS),
        Operator::I8x16AddSatU => lanes(8, L::AddSatU),
        Operator::I8x16SubSatS => lanes(8, L::SubSatS),
        Operator::I8x16SubSatU => lanes(8, L::SubSatU),
        Operator::I8x16MinS => lanes(8, L::MinS),
        Operator::I8x16MinU => lanes(8, L::MinU),
        Operator::I8x16MaxS => lanes(8, L::MaxS),
        Operator::I8x16MaxU => lanes(8, L::MaxU),
        Operator::I8x16AvgrU => lanes(8, L::AvgrU),
        Operator::I16x8Add => lanes(16, L::Add),
        Operator::I16x8Sub => lanes(16, L::Sub),
        Operator::I16x8Mul => lanes(16, L::Mul),
        Operator::I16x8AddSatS => lanes(16, L::AddSatS),
        Operator::I16x8AddSatU => lanes(16, L::AddSatU),
        Operator::I16x8SubSatS => lanes(16, L::SubSatS),
        Operator::I16x8SubSatU => lanes(16, L::SubSatU),
        Operator::I16x8MinS => lanes(16, L::MinS),
        Operator::I16x8MinU => lanes(16, L::MinU),
        Operator::I16x8MaxS => lanes(16, L::MaxS),
        Operator::I16x8MaxU => lanes(16, L::MaxU),
        Operator::I16x8AvgrU => lanes(16, L::AvgrU),
        Operator::I32x4Add => lanes(32, L::Add),
        Operator::I32x4Sub => lanes(32, L::Sub),
        Operator::I32x4Mul => lanes(32, L::Mul),
        Operator::I32x4MinS => lanes(32, L::MinS),
        Operator::I32x4MinU => lanes(32, L::MinU),
        Operator::I32x4MaxS => lanes(32, L::MaxS),
        Operator::I32x4MaxU => lanes(32, L::MaxU),
        Operator::I64x2Add => lanes(64, L::Add),
        Operator::I64x2Sub => lanes(64, L::Sub),
        Operator::I64x2Mul => lanes(64, L::Mul),
        Operator::I8x16Eq => compare(8, B::Eq),
        Operator::I8x16Ne => compare(8, B::Ne),
        Operator::I8x16LtS => compare(8, B::LtS),
        Operator::I8x16LtU => compare(8, B::LtU),
        Operator::I8x16GtS => compare(8, B::GtS),
        Operator::I8x16GtU => compare(8, B::GtU),
        Operator::I8x16LeS => compare(8, B::LeS),
        Operator::I8x16LeU => compare(8, B::LeU),
        Operator::I8x16GeS => compare(8, B::GeS),
        Operator::I8x16GeU => compare(8, B::GeU),
        Operator::I16x8Eq => compare(16, B::Eq),
        Operator::I16x8Ne => compare(16, B::Ne),
        Operator::I16x8LtS => compare(16, B::LtS),
        Operator::I16x8LtU => compare(16, B::LtU),
        Operator::I16x8GtS => compare(16, B::GtS),
        Operator::I16x8GtU => compare(16, B::GtU),
        Operator::I16x8LeS => compare(16, B::LeS),
        Operator::I16x8LeU => compare(16, B::LeU),
        Operator::I16x8GeS => compare(16, B::GeS),
        Operator::I16x8GeU => compare(16, B::GeU),
        Operator::I32x4Eq => compare(32, B::Eq),
        Operator::I32x4Ne => compare(32, B::Ne),
        Operator::I32x4LtS => compare(32, B::LtS),
        Operator::I32x4LtU => compare(32, B::LtU),
        Operator::I32x4GtS => compare(32, B::GtS),
        Operator::I32x4GtU => compare(32, B::GtU),
        Operator::I32x4LeS => compare(32, B::LeS),
        Operator::I32x4LeU => compare(32, B::LeU),
        Operator::I32x4GeS => compare(32, B::GeS),
        Operator::I32x4GeU => compare(32, B::GeU),
        Operator::I64x2Eq => compare(64, B::Eq),
        Operator::I64x2Ne => compare(64, B::Ne),
        Operator::I64x2LtS => compare(64, B::LtS),
        Operator::I64x2GtS => compare(64, B::GtS),
        Operator::I64x2LeS => compare(64, B::LeS),
        Operator::I64x2GeS => compare(64, B::GeS),
        _ => None,
    }
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

/// What one call of `function` takes of the engine's stack, at most, by the model's count: a
/// frame's fixed part, a slot for each of its parameters, results and locals and for another
/// copy of each at every block, where the compiled code may keep one, and two slots for each
/// operator. A host call's frame is its trampoline's, with its arguments and its result.
fn frame(function: &Function) -> u64 {
    let blocks = function
        .ops
        .iter()
        .filter(|op| matches!(op, Op::Block(_) | Op::Loop(_) | Op::If(_)))
        .count();
    let values = function.params + function.results + function.locals.len();
    let slots = values
        .saturating_mul(blocks.saturating_add(1))
        .saturating_add(function.ops.len().saturating_mul(2));
    FRAME.saturating_add(SLOT.saturating_mul(slots as u64))
}

/// The functions `function` calls, each once, by index.
fn called(function: &Function) -> Vec<usize> {
    let mut callees: Vec<usize> = function
        .ops
        .iter()
        .filter_map(|op| match op {
            Op::Call(index) => usize::try_from(*index).ok(),
            _ => None,
        })
        .collect();
    callees.sort_unstable();
    callees.dedup();
    callees
}

/// How far the model has followed a function's calls, for [`deepest`].
#[derive(Debug, Clone, Copy)]
enum Followed {
    /// Not yet.
    Not,
    /// Its calls are being followed.
    Under,
    /// Done: the most bytes of the stack a call of it takes, its callees' calls with it.
    Done(u64),
}

/// Why the model refuses a module whose function `index` calls itself, directly or through others.
fn recursion(functions: &[Function], index: usize) -> String {
    let name = functions
        .get(index)
        .and_then(|function| function.name.clone())
        .map_or_else(|| format!("function {index}"), |name| format!("${name}"));
    format!(
        "{name} calls itself, directly or through others: the dialect has no recursion \
         (rule 1), and the model can't count the stack such calls take"
    )
}

/// The most bytes of the engine's stack a run's calls take, by the model's count: the frames of
/// the deepest calls from the function `entry`, `_start`. Without recursion, every run's calls
/// are one of the call graph's paths from it.
///
/// # Errors
///
/// A function that calls itself, directly or through others.
fn deepest(functions: &[Function], entry: usize) -> Result<u64, String> {
    if entry >= functions.len() {
        return Err("the module exports no `_start`".into());
    }
    let callees: Vec<Vec<usize>> = functions.iter().map(called).collect();
    let mut followed = vec![Followed::Not; functions.len()];
    followed[entry] = Followed::Under;
    // The calls being followed, the newest last: each function, with how many of its callees
    // are followed.
    let mut path = vec![(entry, 0)];
    while let Some(top) = path.len().checked_sub(1) {
        let (function, done) = path[top];
        let Some(&callee) = callees[function].get(done) else {
            // Every callee followed: its own frame, and the deepest of theirs.
            let below = callees[function]
                .iter()
                .filter_map(|callee| match followed[*callee] {
                    Followed::Done(bytes) => Some(bytes),
                    Followed::Not | Followed::Under => None,
                })
                .max()
                .unwrap_or(0);
            let bytes = frame(&functions[function]).saturating_add(below);
            followed[function] = Followed::Done(bytes);
            path.pop();
            continue;
        };
        path[top].1 += 1;
        match followed.get(callee).copied() {
            Some(Followed::Not) => {
                followed[callee] = Followed::Under;
                path.push((callee, 0));
            }
            Some(Followed::Under) => return Err(recursion(functions, callee)),
            Some(Followed::Done(_)) => {}
            None => return Err("a call of a function the module doesn't have".into()),
        }
    }
    match followed.get(entry) {
        Some(Followed::Done(bytes)) => Ok(*bytes),
        _ => Err("the module exports no `_start`".into()),
    }
}

impl Module {
    /// Reads the module `built` holds, for a program whose idea lets it reach what `policy`
    /// allows: as wast encoded it, with its names ([`Built::named`]), whose bytes before the
    /// names are the module that runs.
    ///
    /// # Errors
    ///
    /// What the model can't run as the engine does: a module it can't read, an import lengua's
    /// host doesn't give or of another type than WASI's, which the engine doesn't link, a start
    /// function, a table or an element segment, a value of a type it doesn't hold, data past the
    /// memory, a function that calls itself, directly or through others, or calls that may go
    /// deeper than the engine's stack holds.
    #[allow(clippy::too_many_lines)]
    pub fn new(built: &Built, policy: Policy) -> Result<Self, String> {
        let failed = |error: wasmparser::BinaryReaderError| {
            format!("the model couldn't read the module: {error}")
        };
        // Each type's parameters and results, when it's a function's.
        let mut types: Vec<Option<Signature>> = Vec::new();
        let mut imports: Vec<(String, String, u32)> = Vec::new();
        let mut defined: Vec<u32> = Vec::new();
        let mut memory: Option<(u64, Option<u64>)> = None;
        let mut globals = Vec::new();
        let mut mutable = Vec::new();
        let mut entry = None;
        let mut exports_memory = false;
        let mut segments: Vec<(Vec<u8>, Option<u64>)> = Vec::new();
        let mut bodies = Vec::new();
        let mut names: BTreeMap<u32, String> = BTreeMap::new();
        for payload in Parser::new(0).parse_all(&built.named) {
            match payload.map_err(failed)? {
                Payload::TypeSection(reader) => {
                    for group in reader {
                        for sub in group.map_err(failed)?.into_types() {
                            types.push(match &sub.composite_type.inner {
                                CompositeInnerType::Func(func) => {
                                    Some((func.params().to_vec(), func.results().to_vec()))
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
                        mutable.push(global.ty.mutable);
                    }
                }
                Payload::ExportSection(reader) => {
                    for export in reader {
                        let export = export.map_err(failed)?;
                        if export.name == "_start" && matches!(export.kind, ExternalKind::Func) {
                            entry = Some(export.index);
                        }
                        if export.name == "memory" && matches!(export.kind, ExternalKind::Memory) {
                            exports_memory = true;
                        }
                    }
                }
                Payload::StartSection { .. } => {
                    return Err(
                        "a start function, which the model doesn't run before `_start`".into(),
                    );
                }
                // The engine sets a module's tables up as it starts it, and may trap there or
                // refuse to start it: the model follows neither, as the dialect has no tables.
                Payload::TableSection(_) | Payload::ElementSection(_) => {
                    return Err(
                        "a table or an element segment, which the dialect has none of (rule 9) \
                         and the model doesn't set up"
                            .into(),
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
        // The engine holds a memory exported as `memory` to its maximum, or to its size without
        // one, and a memory the module keeps to itself to its maximum, or to the most a 32-bit
        // memory has (`crate::wasm::host::limits::memory_limit`).
        let unbounded = if exports_memory { pages } else { MOST_PAGES };
        let most_pages = maximum.unwrap_or(unbounded).min(MOST_PAGES);
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

        let mut functions = Vec::new();
        for (module, name, index) in &imports {
            let (params, results) = signature(&types, *index)?;
            // What a program opens by itself at run time, the model doesn't follow: which
            // computer it reaches is the program's own choice while it runs, and the model's
            // network is the connections lengua makes before the start.
            if module.as_str() == wasi::NET_MODULE {
                return Err(format!(
                    "it imports {module}.{name}, which opens what its idea grants while it runs: \
                     the model follows the connections lengua makes before a program starts, not \
                     the ones a program opens by itself, so this program is tested, not proven"
                ));
            }
            // Nor a file the user names when the program runs, a program it starts, or the
            // terminal: the model's files are the working folder's, it has no other program,
            // and its standard input is the test's bytes.
            // Work split into pieces is its own: the model follows a run that is a piece, and
            // stops at a call that splits (host.rs).
            let splits = module.as_str() == wasi::SPLIT_MODULE;
            if wasi::own_module(module) && !splits {
                return Err(format!(
                    "it imports {module}.{name}, which reaches what its idea grants while it \
                     runs: the model follows the working folder, the connections lengua makes \
                     before a program starts and the input a test gives, not a file an argument \
                     names, a program it starts or the terminal, so this program is tested, not \
                     proven"
                ));
            }
            let host = (module.as_str() == wasi::MODULE || splits)
                .then(|| wasi::imported(module, name))
                .flatten()
                .ok_or_else(|| {
                    format!("it imports {module}.{name}, which lengua's host doesn't give")
                })?;
            if !same_values(host.params, params) || !same_values(host.results, results) {
                return Err(format!(
                    "it imports {name} with a signature other than WASI's, which the engine won't \
                     link"
                ));
            }
            functions.push(Function {
                name: Some(name.clone()),
                params: params.len(),
                results: results.len(),
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
            let (params, results) = signature(&types, *type_index)?;
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
                params: params.len(),
                results: results.len(),
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
        // The stack never runs out on a run the model takes: the deepest calls fit in it.
        let stack = deepest(&functions, usize::try_from(entry).unwrap_or(usize::MAX))?;
        if stack > STACK {
            return Err(format!(
                "its deepest calls may take {stack} bytes of the engine's stack, past the \
                 {STACK} the model is sure it has (M8)"
            ));
        }
        let entered = functions.iter().map(|_| AtomicBool::new(false)).collect();
        // lengua's `$fill`: the library's function of the name, never one of main.wat's own.
        let fill = (0_u32..).zip(&functions).find_map(|(index, function)| {
            (!function.own && function.import.is_none() && function.name.as_deref() == Some("fill"))
                .then_some(index)
        });
        let reading = reading(&functions, fill, &mutable);
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
            fill,
            reading,
            lines: built.lines.clone(),
            bodies: built.bodies.clone(),
            entered,
            mutable,
            claims: BTreeMap::new(),
        })
    }

    /// The function with index `index`.
    pub(super) fn function(&self, index: u32) -> Option<&Function> {
        self.functions.get(usize::try_from(index).ok()?)
    }

    /// Whether a proof can take every way the program's standard input can be cut: it reads only
    /// through lengua's `$fill`, each call's stop byte known before the program runs, never waits
    /// on `poll_oneoff`, and isn't on the network. A proof then reads each `$fill` of standard
    /// input as a whole, every place it can stop a run of its own, for input of any length
    /// ([`super::fill`]).
    #[must_use]
    pub fn reads_whole(&self) -> bool {
        self.reading.whole && !self.policy.network()
    }

    /// Where the read loop a proof by induction goes round starts: the call of `$fill` in a loop
    /// of main.wat's own code, as a hook names it.
    ///
    /// # Errors
    ///
    /// Why the program has no read loop the induction takes: it reads in no loop, in several
    /// places, with its own `fd_read`, or other than through `$fill` to the end of its input.
    pub fn read_loop_head(&self) -> Result<u64, String> {
        let (function, at) = self.reading.in_a_loop.clone()?;
        if !self.reads_whole() {
            return Err(NOT_WHOLE.into());
        }
        Ok(super::induction::head_of(function, at))
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
