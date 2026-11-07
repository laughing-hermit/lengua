//! Memory safety, the M rules, over the program's regions.
//!
//! The checker follows each value through each function: a region's address (from
//! `global.get $x`), that address plus a number, a parameter (an address the caller lent, or a
//! number), a plain number with the range the code's checks give it, a value it doesn't follow
//! (one loaded from memory), or, where paths meet, one of several of these addresses, each
//! kept. Functions are followed callees first, so each call knows what its callee does with
//! what it's lent. Every access, and every buffer a call borrows, is then judged against its
//! region, and one of several addresses against each region it may be in:
//!
//! - **M1, owners.** An address is a region's, or one a caller lent; never a plain number, nor
//!   two addresses added together. Regions start at 1024 or above, don't overlap and fit in the
//!   memory: the structure pass checks that.
//! - **M2, bounds.** An access at a constant offset stays inside its region, through every width
//!   from 1 to 16 bytes; one with a computed index is rule 9's, held to the same bounds.
//! - **M3, lifetimes.** Locals can't be addressed and nothing is freed, so no address outlives
//!   its region: this holds by construction.
//! - **M4, exclusive writes.** At a call, no argument names a part of a region that overlaps
//!   another argument the callee writes through.
//! - **M5, initialization.** Memory and locals start at zero, so nothing is read uninitialized:
//!   this holds by construction.
//! - **M6, immutability.** No write reaches a read-only region, through any address.
//! - **M7, threads.** There are none, so no data race: this holds by construction.
//! - **M8, stack.** The deepest call path's locals and operands fit within the engine's stack.
//! - **Borrowed buffers.** Every buffer a host call reads or writes, as `src/wasm/wasi.rs` lists
//!   them (an iovec array and the buffers it points to, a path, the place a count is written),
//!   lies inside a region with room for every byte the call may touch.
//!
//! What the rules can't settle (an address loaded from memory, an index checked against a limit
//! they don't follow, the sizes `args_get` writes) is an [`Obligation`] for the proof.

use std::collections::{BTreeMap, BTreeSet};

use wasmparser::{BlockType, FrameKind, MemArg, Operator, ValType};

use super::flow::{Flows, Shape};
use super::model::{Function, Kind, Module, to_u32, to_usize};
use super::{Facts, Obligation};
use crate::checker::{Rule, Violation};
use crate::wasm::wasi::{self, Bytes};

/// The engine's stack for WebAssembly: Wasmtime's own `max_wasm_stack`, 512 KiB, which
/// `src/wasm/engine.rs` leaves as it is.
pub const ENGINE_STACK_BYTES: u64 = 512 * 1024;

/// The most stack the deepest call path may take as the checker counts it: half the engine's,
/// for what the count doesn't see.
pub const MAX_STACK_BYTES: u64 = ENGINE_STACK_BYTES / 2;

/// What the checker counts for a frame besides its locals and operands.
const FRAME_BYTES: u64 = 64;

/// What the checker counts for each local and operand: a vector's 16 bytes.
const SLOT_BYTES: u64 = 16;

/// The most times a loop's body is followed before its head is taken as anything its body may
/// make it.
const MAX_ITERATIONS: usize = 6;

/// The most operators followed in one function before loops stop being refined.
const BUDGET: usize = 400_000;

/// The deepest nesting of blocks the checker follows.
const MAX_NESTING: usize = 256;

/// The most iovec entries followed at one call.
const MAX_IOVECS: u64 = 64;

/// A version of a local's value: the operator that set it, or the join or loop head where paths
/// meet, and the local.
pub type Version = (usize, u32);

/// A number as a linear function of one version of a local: `sign` × its value + `constant`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Linear {
    /// The version it depends on.
    pub version: Version,
    /// 1 or -1.
    pub sign: i64,
    /// What's added.
    pub constant: i64,
}

impl Linear {
    /// The same, plus `by`.
    fn shifted(self, by: i64) -> Option<Self> {
        Some(Self {
            constant: self.constant.checked_add(by)?,
            ..self
        })
    }

    /// `from` minus it.
    fn subtracted_from(self, from: i64) -> Option<Self> {
        Some(Self {
            sign: -self.sign,
            constant: from.checked_sub(self.constant)?,
            ..self
        })
    }

    /// Whether it and `other` add up to a constant, and which.
    fn sum(self, other: Self) -> Option<i64> {
        if self.version == other.version && self.sign + other.sign == 0 {
            self.constant.checked_add(other.constant)
        } else {
            None
        }
    }
}

/// A plain number's unsigned range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// The least it may be.
    pub low: u64,
    /// The most it may be: `u64::MAX` when nothing bounds it.
    pub high: u64,
    /// When nothing bounds it, whether it passed a check against a limit the rules don't
    /// follow, so the proof holds it; when something does, true.
    pub checked: bool,
    /// What it is in terms of a local's value, when that's known.
    pub linear: Option<Linear>,
}

impl Range {
    /// Exactly `value`.
    #[must_use]
    pub const fn exact(value: u64) -> Self {
        Self::between(value, value)
    }

    /// From `low` to `high`.
    #[must_use]
    pub const fn between(low: u64, high: u64) -> Self {
        Self {
            low,
            high,
            checked: true,
            linear: None,
        }
    }

    /// Anything, which nothing checked.
    #[must_use]
    pub const fn full() -> Self {
        Self {
            low: 0,
            high: u64::MAX,
            checked: false,
            linear: None,
        }
    }

    /// Whether a limit bounds it as a 32-bit number: every 32-bit number is a bound, the largest
    /// too, so an offset of -1 is one the rules judge.
    #[must_use]
    pub fn bounded(&self) -> bool {
        self.high <= u64::from(u32::MAX)
    }

    /// Its one value, when it has one.
    #[must_use]
    pub fn constant(&self) -> Option<u64> {
        (self.low == self.high).then_some(self.low)
    }

    /// Whether everything that went into it was bounded or checked.
    fn sound(&self) -> bool {
        self.high != u64::MAX || self.checked
    }

    /// It as a number of `bits` bits: past them, anything. A number the rules bound past the
    /// width wraps, so nothing checked what it becomes.
    fn width(self, bits: u32) -> Self {
        if self.high <= max_of(bits) {
            self
        } else if self.high == u64::MAX {
            Self {
                checked: self.checked,
                ..Self::full()
            }
        } else {
            Self::full()
        }
    }

    /// Anything, from `inputs`: checked only when one of them was checked against a limit the
    /// rules don't follow, and the others are bounded or checked too, so the proof holds it.
    /// What the rules bound and the result outgrows has wrapped, and nothing checked it.
    fn unknown(inputs: &[Self]) -> Self {
        let unfollowed = inputs.iter().any(|input| input.high == u64::MAX);
        Self {
            checked: unfollowed && inputs.iter().all(Self::sound),
            ..Self::full()
        }
    }
}

/// An offset from an address, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offset {
    /// The least it may be, or `None` when nothing bounds it.
    pub low: Option<i64>,
    /// The most it may be, or `None` when nothing bounds it.
    pub high: Option<i64>,
    /// When something doesn't bound it, whether its computed part passed a check against a limit
    /// the rules don't follow; else true.
    pub checked: bool,
    /// What it is in terms of a local's value, when that's known.
    pub linear: Option<Linear>,
    /// The local whose value is its computed part, for reports.
    pub index: Option<u32>,
}

impl Offset {
    /// No offset.
    pub const ZERO: Self = Self {
        low: Some(0),
        high: Some(0),
        checked: true,
        linear: None,
        index: None,
    };

    /// Its one value, when it has one.
    #[must_use]
    pub fn constant(&self) -> Option<i64> {
        self.low.filter(|low| Some(*low) == self.high)
    }

    /// It plus `by` bytes.
    fn shifted(self, by: i64) -> Self {
        Self {
            low: self.low.and_then(|low| low.checked_add(by)),
            high: self.high.and_then(|high| high.checked_add(by)),
            linear: self.linear.and_then(|linear| linear.shifted(by)),
            ..self
        }
    }
}

/// What a fact the code's checks establish says of a local's value: of the version of it the
/// check read, so once the local holds another value, the fact says nothing of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fact {
    /// Its value is below `limit`, unsigned; `None` when the limit is one the rules don't
    /// bound.
    Below {
        /// The local, at the version the check read.
        local: Version,
        /// The limit.
        limit: Option<u64>,
    },
    /// Its value is at least `limit`, unsigned.
    AtLeast {
        /// The local, at the version the check read.
        local: Version,
        /// The limit.
        limit: u64,
    },
    /// Its value is zero.
    Zero {
        /// The local, at the version the check read.
        local: Version,
    },
    /// Its float isn't NaN.
    NotNan {
        /// The local, at the version the check read.
        local: Version,
    },
    /// A signed comparison looked at it, which bounds nothing.
    Signed {
        /// The local, at the version the check read.
        local: Version,
    },
}

/// A comparison's result, with the facts that hold when it's true and when it's false.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Test {
    /// What holds when it's true.
    pub when_true: Vec<Fact>,
    /// What holds when it's false.
    pub when_false: Vec<Fact>,
}

/// A value as the memory rules follow it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A plain number.
    Number(Range),
    /// An address in region `region`, `offset` bytes from its start.
    Address {
        /// The region's index.
        region: usize,
        /// How far into it.
        offset: Offset,
    },
    /// Parameter `param` as the caller passed it, plus `offset`: an address it lent, or a
    /// number.
    Param {
        /// The parameter.
        param: u32,
        /// What's added to it.
        offset: Offset,
    },
    /// A value loaded from memory, or another the rules don't follow.
    Loaded,
    /// One of several addresses, where paths meet: each owner's (a region's, a parameter's, or
    /// one the rules don't follow) once, in their order.
    Several(Vec<Value>),
    /// Different kinds of value on different paths: `number` when it may be a plain number.
    Mixed {
        /// Whether it may be a plain number.
        number: bool,
    },
    /// A float, `nan` when it may be NaN.
    Float {
        /// Whether it may be NaN.
        nan: bool,
    },
    /// A comparison's result.
    Test(Box<Test>),
    /// A vector, a reference, or a number the rules don't follow.
    Other,
}

impl Value {
    /// Whether it's an address: a region's, a parameter's, or one of several.
    fn is_address(&self) -> bool {
        matches!(self, Self::Address { .. } | Self::Param { .. } | Self::Several(_))
    }

    /// Whether it may be a plain number.
    fn may_be_number(&self) -> bool {
        matches!(
            self,
            Self::Number(_) | Self::Test(_) | Self::Mixed { number: true } | Self::Other
        )
    }

    /// It as a number's range, as an index or a length: anything but a number is anything.
    #[must_use]
    pub fn range(&self) -> Range {
        match self {
            Self::Number(range) => *range,
            Self::Test(_) => Range::between(0, 1),
            _ => Range::full(),
        }
    }

    /// The same, with no linear relation kept: what's stored in a local or memory.
    fn stored(mut self) -> Self {
        match &mut self {
            Self::Number(range) => range.linear = None,
            Self::Address { offset, .. } | Self::Param { offset, .. } => offset.linear = None,
            Self::Several(items) => {
                for item in items.iter_mut() {
                    *item = item.clone().stored();
                }
            }
            _ => {}
        }
        self
    }
}

/// The addresses `value` may be, each the address of one owner, when it's an address: a
/// region's, a parameter's, or one the rules don't follow.
fn addresses(value: &Value) -> Option<Vec<Value>> {
    match value {
        Value::Address { .. } | Value::Param { .. } | Value::Loaded => Some(vec![value.clone()]),
        Value::Mixed { number: false } => Some(vec![Value::Loaded]),
        Value::Several(items) => Some(items.clone()),
        _ => None,
    }
}

/// The addresses a value an access goes through may be: each of several, or itself.
fn parts(value: &Value) -> &[Value] {
    match value {
        Value::Several(items) => items.as_slice(),
        other => std::slice::from_ref(other),
    }
}

/// Whose an address is, to keep each owner's once and in order: a region's, a parameter's, or
/// one the rules don't follow.
fn owner(value: &Value) -> (u8, usize) {
    match value {
        Value::Address { region, .. } => (0, *region),
        Value::Param { param, .. } => (1, to_usize(*param)),
        _ => (2, 0),
    }
}

/// What a value is when it may be any of the addresses `a` or `b`: each owner's once, joined.
fn several(a: Vec<Value>, b: Vec<Value>) -> Value {
    let mut items = a;
    for item in b {
        match items.iter().position(|kept| owner(kept) == owner(&item)) {
            Some(at) => {
                let joined = join(&items[at], &item);
                items[at] = joined;
            }
            None => items.push(item),
        }
    }
    items.sort_by_key(owner);
    match items.len() {
        1 => items.pop().unwrap_or(Value::Loaded),
        _ => Value::Several(items),
    }
}

/// What `f` gives for each of `items`, joined: an operation on one of several addresses.
fn each(items: &[Value], f: impl Fn(&Value) -> Value) -> Value {
    items
        .iter()
        .map(f)
        .reduce(|a, b| join(&a, &b))
        .unwrap_or(Value::Mixed { number: true })
}

/// The smallest range both `a` and `b` are in.
fn range_join(a: Range, b: Range) -> Range {
    Range {
        low: a.low.min(b.low),
        high: a.high.max(b.high),
        checked: a.checked && b.checked,
        linear: if a.linear == b.linear { a.linear } else { None },
    }
}

/// The smallest offset range both `a` and `b` are in.
fn offset_join(a: Offset, b: Offset) -> Offset {
    Offset {
        low: a.low.zip(b.low).map(|(a, b)| a.min(b)),
        high: a.high.zip(b.high).map(|(a, b)| a.max(b)),
        checked: a.checked && b.checked,
        linear: if a.linear == b.linear { a.linear } else { None },
        index: if a.index == b.index { a.index } else { None },
    }
}

/// What a value is when it may be `a` or `b`.
#[must_use]
pub fn join(a: &Value, b: &Value) -> Value {
    match (a, b) {
        _ if a == b => a.clone(),
        (Value::Number(x), Value::Number(y)) => Value::Number(range_join(*x, *y)),
        (Value::Number(x), Value::Test(_)) | (Value::Test(_), Value::Number(x)) => {
            Value::Number(range_join(*x, Range::between(0, 1)))
        }
        (Value::Test(_), Value::Test(_)) => Value::Number(Range::between(0, 1)),
        (
            Value::Address { region, offset },
            Value::Address {
                region: other,
                offset: also,
            },
        ) if region == other => Value::Address {
            region: *region,
            offset: offset_join(*offset, *also),
        },
        (
            Value::Param { param, offset },
            Value::Param {
                param: other,
                offset: also,
            },
        ) if param == other => Value::Param {
            param: *param,
            offset: offset_join(*offset, *also),
        },
        (Value::Float { nan }, Value::Float { nan: also }) => Value::Float { nan: *nan || *also },
        _ => match (addresses(a), addresses(b)) {
            // Addresses of different owners: each kept, so an access through the value is judged
            // against every region it may be in.
            (Some(mine), Some(theirs)) => several(mine, theirs),
            _ => Value::Mixed {
                number: a.may_be_number() || b.may_be_number(),
            },
        },
    }
}

/// The largest number of `bits` bits.
fn max_of(bits: u32) -> u64 {
    if bits >= 64 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    }
}

/// `value` as an offset's signed bytes, when it fits.
fn signed(value: u64) -> Option<i64> {
    i64::try_from(value).ok()
}

/// `a + b` as numbers of `bits` bits.
fn range_plus(a: Range, b: Range, bits: u32) -> Range {
    let (a, b) = (a.width(bits), b.width(bits));
    let max = max_of(bits);
    if let (Some(x), Some(y)) = (a.linear, b.linear)
        && let Some(sum) = x.sum(y).and_then(|sum| u64::try_from(sum).ok())
        && sum <= max
    {
        return Range::exact(sum);
    }
    let linear = match (a.linear, a.constant(), b.linear, b.constant()) {
        (Some(linear), _, _, Some(value)) | (_, Some(value), Some(linear), _) => {
            signed(value).and_then(|value| linear.shifted(value))
        }
        _ => None,
    };
    match a.high.checked_add(b.high) {
        Some(high) if high <= max && a.high != u64::MAX && b.high != u64::MAX => Range {
            low: a.low + b.low,
            high,
            checked: true,
            linear,
        },
        _ => Range {
            linear,
            ..Range::unknown(&[a, b])
        },
    }
}

/// `a - b` as numbers of `bits` bits.
fn range_minus(a: Range, b: Range, bits: u32) -> Range {
    let (a, b) = (a.width(bits), b.width(bits));
    let max = max_of(bits);
    if let (Some(x), Some(y)) = (a.linear, b.linear)
        && x.version == y.version
        && x.sign == y.sign
        && let Some(difference) = x.constant.checked_sub(y.constant)
        && let Ok(difference) = u64::try_from(difference)
        && difference <= max
    {
        return Range::exact(difference);
    }
    let linear = match (a.linear, a.constant(), b.linear, b.constant()) {
        (Some(linear), _, _, Some(value)) => {
            signed(value).and_then(|value| linear.shifted(value.checked_neg()?))
        }
        (_, Some(value), Some(linear), _) => {
            signed(value).and_then(|value| linear.subtracted_from(value))
        }
        _ => None,
    };
    if a.high != u64::MAX && a.low >= b.high {
        Range {
            low: a.low - b.high,
            high: a.high - b.low,
            checked: true,
            linear,
        }
    } else {
        Range {
            linear,
            checked: false,
            ..Range::full()
        }
    }
}

/// What an integer operator on numbers of `bits` bits gives, for those the rules bound.
fn range_of(operator: &Operator<'_>, a: Range, b: Range, bits: u32) -> Range {
    let (a, b) = (a.width(bits), b.width(bits));
    let max = max_of(bits);
    match operator {
        Operator::I32Mul | Operator::I64Mul => match a.high.checked_mul(b.high) {
            Some(high) if high <= max && a.high != u64::MAX && b.high != u64::MAX => {
                Range::between(a.low * b.low, high)
            }
            _ => Range::unknown(&[a, b]),
        },
        Operator::I32And | Operator::I64And => {
            let high = a.high.min(b.high);
            if high == u64::MAX {
                Range::unknown(&[a, b])
            } else {
                Range::between(0, high)
            }
        }
        Operator::I32Or | Operator::I64Or | Operator::I32Xor | Operator::I64Xor => {
            let most = a.high.max(b.high);
            if most == u64::MAX {
                return Range::unknown(&[a, b]);
            }
            // The smallest number of all ones at least as big: no bit above the highest of
            // either is set.
            let ones = if most == 0 {
                0
            } else {
                u64::MAX >> most.leading_zeros()
            };
            if ones <= max {
                Range::between(0, ones)
            } else {
                Range::unknown(&[a, b])
            }
        }
        // WebAssembly shifts by the amount modulo the width.
        Operator::I32Shl | Operator::I64Shl => match b.constant() {
            Some(shift) if a.high != u64::MAX => {
                let shift = u32::try_from(shift % u64::from(bits)).unwrap_or(0);
                match a
                    .high
                    .checked_shl(shift)
                    .filter(|high| high >> shift == a.high)
                {
                    Some(high) if high <= max => Range::between(a.low << shift, high),
                    _ => Range::unknown(&[a]),
                }
            }
            _ => Range::unknown(&[a, b]),
        },
        Operator::I32ShrU | Operator::I64ShrU => match b.constant() {
            Some(shift) => {
                let shift = u32::try_from(shift % u64::from(bits)).unwrap_or(0);
                let high = if a.high == u64::MAX { max } else { a.high };
                Range::between(a.low >> shift, high >> shift)
            }
            // However far it shifts, never more than it was.
            None if a.high != u64::MAX => Range::between(0, a.high),
            None => Range::unknown(&[a, b]),
        },
        // Below the divisor and never more than the dividend; a zero divisor traps, which the
        // checks of determinism report.
        Operator::I32RemU | Operator::I64RemU if b.high != u64::MAX => {
            Range::between(0, a.high.min(b.high.saturating_sub(1)))
        }
        Operator::I32RemU | Operator::I64RemU if a.high != u64::MAX => Range::between(0, a.high),
        Operator::I32DivU | Operator::I64DivU if b.low >= 1 || a.high != u64::MAX => {
            let high = if a.high == u64::MAX { max } else { a.high };
            Range::between(a.low / b.high.max(1), high / b.low.max(1))
        }
        Operator::I32Clz | Operator::I32Ctz | Operator::I32Popcnt => Range::between(0, 32),
        Operator::I64Clz | Operator::I64Ctz | Operator::I64Popcnt => Range::between(0, 64),
        _ => Range::unknown(&[a, b]),
    }
}

/// `offset` plus the number `value`, whose local, if it was just read from one, is `index`.
fn offset_plus(offset: Offset, value: &Value, index: Option<u32>) -> Offset {
    let Value::Number(range) = value else {
        // Anything else as an index: nothing checked it.
        return Offset {
            low: offset.low,
            high: None,
            checked: false,
            linear: None,
            index: index.or(offset.index),
        };
    };
    let range = range.width(32);
    // A constant of half the 32 bits or more takes the address back, as `i32.add` wraps.
    if let Some(value) = range.constant()
        && value >= 1_u64 << 31
        && let Ok(value) = i64::try_from(value)
    {
        return offset.shifted(value - (1_i64 << 32));
    }
    let bounded = range.bounded();
    let low = offset
        .low
        .and_then(|low| low.checked_add(signed(range.low)?));
    let high = if bounded {
        offset
            .high
            .and_then(|high| high.checked_add(signed(range.high)?))
    } else {
        None
    };
    let linear = match (
        offset.constant(),
        offset.linear,
        range.constant(),
        range.linear,
    ) {
        (Some(base), None, _, Some(linear)) => linear.shifted(base),
        (_, Some(linear), Some(value), _) => signed(value).and_then(|value| linear.shifted(value)),
        _ => None,
    };
    Offset {
        low,
        high,
        checked: offset.checked && (bounded || range.checked),
        linear,
        index: if range.constant().is_some() {
            offset.index
        } else {
            index.or(offset.index)
        },
    }
}

/// `offset` minus the number `value`.
fn offset_minus(offset: Offset, value: &Value) -> Offset {
    let Value::Number(range) = value else {
        return Offset {
            low: None,
            high: offset.high,
            checked: false,
            linear: None,
            index: offset.index,
        };
    };
    let range = range.width(32);
    let bounded = range.bounded();
    let low = if bounded {
        offset
            .low
            .and_then(|low| low.checked_sub(signed(range.high)?))
    } else {
        None
    };
    let high = offset
        .high
        .and_then(|high| high.checked_sub(signed(range.low)?));
    let linear = match (offset.linear, range.constant()) {
        (Some(linear), Some(value)) => {
            signed(value).and_then(|value| linear.shifted(value.checked_neg()?))
        }
        _ => None,
    };
    Offset {
        low,
        high,
        checked: offset.checked && (bounded || range.checked),
        linear,
        index: offset.index,
    }
}

/// A value on the operand stack: what it is, and the local it was just read from, if it was.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Slot {
    /// The value.
    value: Value,
    /// The local it was just read from, at the version it read: a check of the value says
    /// something of that version alone.
    local: Option<Version>,
}

impl Slot {
    /// A value read from no local.
    fn new(value: Value) -> Self {
        Self { value, local: None }
    }

    /// The local it was just read from, for reports.
    fn index(&self) -> Option<u32> {
        self.local.map(|(_, index)| index)
    }

    /// `value`, as if read from the same local as this one: one of the addresses it may be.
    fn with(&self, value: &Value) -> Self {
        Self {
            value: value.clone(),
            local: self.local,
        }
    }
}

/// `a + b`, for numbers of `bits` bits.
fn plus(a: &Slot, b: &Slot, bits: u32) -> Value {
    // One of several addresses: each moved alike.
    if let Value::Several(items) = &a.value {
        return each(items, |item| plus(&a.with(item), b, bits));
    }
    if let Value::Several(items) = &b.value {
        return each(items, |item| plus(a, &b.with(item), bits));
    }
    let moved = |address: &Slot, other: &Slot| match &address.value {
        Value::Address { region, offset } => Value::Address {
            region: *region,
            offset: offset_plus(*offset, &other.value, other.index()),
        },
        Value::Param { param, offset } => Value::Param {
            param: *param,
            offset: offset_plus(*offset, &other.value, other.index()),
        },
        _ => Value::Mixed { number: false },
    };
    match (&a.value, &b.value) {
        // A parameter added to another, or to a region's address: the parameter is an index
        // nothing checked.
        (Value::Param { param, offset }, Value::Param { .. }) => Value::Param {
            param: *param,
            offset: offset_plus(*offset, &Value::Loaded, b.index()),
        },
        (Value::Address { region, offset }, Value::Param { .. }) => Value::Address {
            region: *region,
            offset: offset_plus(*offset, &Value::Loaded, b.index()),
        },
        (Value::Param { .. }, Value::Address { region, offset }) => Value::Address {
            region: *region,
            offset: offset_plus(*offset, &Value::Loaded, a.index()),
        },
        // Two regions' addresses added: a plain number, the address of neither.
        (Value::Address { .. }, Value::Address { .. }) => Value::Number(Range::full()),
        (x, _) if x.is_address() => moved(a, b),
        (_, y) if y.is_address() => moved(b, a),
        (Value::Loaded, _) | (_, Value::Loaded) => Value::Loaded,
        (Value::Mixed { number }, _) | (_, Value::Mixed { number }) => {
            Value::Mixed { number: *number }
        }
        (x, y) => Value::Number(range_plus(x.range(), y.range(), bits)),
    }
}

/// `a - b`, for numbers of `bits` bits.
fn minus(a: &Slot, b: &Slot, bits: u32) -> Value {
    // One of several addresses: each moved alike.
    if let Value::Several(items) = &a.value {
        return each(items, |item| minus(&a.with(item), b, bits));
    }
    if let Value::Several(items) = &b.value {
        return each(items, |item| minus(a, &b.with(item), bits));
    }
    match (&a.value, &b.value) {
        (
            Value::Address { region, offset },
            Value::Address {
                region: other,
                offset: also,
            },
        ) if region == other => match (offset.constant(), also.constant()) {
            (Some(x), Some(y)) if x >= y => Value::Number(Range::exact(x.abs_diff(y))),
            _ => Value::Number(Range::full()),
        },
        (Value::Address { region, offset }, y) if !y.is_address() => Value::Address {
            region: *region,
            offset: offset_minus(*offset, y),
        },
        (Value::Param { param, offset }, y) if !y.is_address() => Value::Param {
            param: *param,
            offset: offset_minus(*offset, y),
        },
        // How far a value is from an address: a plain number, to check before it's used.
        (_, y) if y.is_address() => Value::Number(Range::full()),
        (Value::Loaded, _) => Value::Loaded,
        (Value::Mixed { number }, _) => Value::Mixed { number: *number },
        (x, y) => Value::Number(range_minus(x.range(), y.range(), bits)),
    }
}

/// The limit a comparison's other side gives: the least and the most it may be, the most `None`
/// when nothing bounds it.
fn limit(slot: &Slot) -> (u64, Option<u64>) {
    let range = slot.value.range();
    (range.low, (range.high != u64::MAX).then_some(range.high))
}

/// What an unsigned comparison `local OP other` says when it's true and when it's false, of the
/// version of the local it read.
fn bound_facts(local: Version, op: Compare, other: &Slot) -> Test {
    let (low, high) = limit(other);
    let below = |limit: Option<u64>| Fact::Below { local, limit };
    let at_least = |limit: u64| Fact::AtLeast { local, limit };
    let plus_one = |value: Option<u64>| value.and_then(|value| value.checked_add(1));
    match op {
        Compare::Below => Test {
            when_true: vec![below(high)],
            when_false: vec![at_least(low)],
        },
        Compare::AtLeast => Test {
            when_true: vec![at_least(low)],
            when_false: vec![below(high)],
        },
        Compare::AtMost => Test {
            when_true: vec![below(plus_one(high))],
            when_false: vec![at_least(low.saturating_add(1))],
        },
        Compare::Above => Test {
            when_true: vec![at_least(low.saturating_add(1))],
            when_false: vec![below(plus_one(high))],
        },
        Compare::Equal => {
            let exact = high.filter(|high| *high == low);
            Test {
                when_true: exact
                    .map(|value| vec![at_least(value), below(value.checked_add(1))])
                    .unwrap_or_default(),
                when_false: if exact == Some(0) {
                    vec![at_least(1)]
                } else {
                    Vec::new()
                },
            }
        }
        Compare::NotEqual => {
            let equal = bound_facts(local, Compare::Equal, other);
            Test {
                when_true: equal.when_false,
                when_false: equal.when_true,
            }
        }
    }
}

/// An integer comparison, as the facts read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compare {
    /// `<`, unsigned.
    Below,
    /// `>=`, unsigned.
    AtLeast,
    /// `<=`, unsigned.
    AtMost,
    /// `>`, unsigned.
    Above,
    /// `==`.
    Equal,
    /// `!=`.
    NotEqual,
}

impl Compare {
    /// The same comparison with its sides swapped: `a < b` is `b > a`.
    fn swapped(self) -> Self {
        match self {
            Self::Below => Self::Above,
            Self::Above => Self::Below,
            Self::AtLeast => Self::AtMost,
            Self::AtMost => Self::AtLeast,
            other => other,
        }
    }

    /// The comparison `operator` makes, when it's an integer one that bounds; `Err(())` for a
    /// signed one.
    fn of(operator: &Operator<'_>) -> Option<Result<Self, ()>> {
        Some(Ok(match operator {
            Operator::I32LtU | Operator::I64LtU => Self::Below,
            Operator::I32GeU | Operator::I64GeU => Self::AtLeast,
            Operator::I32LeU | Operator::I64LeU => Self::AtMost,
            Operator::I32GtU | Operator::I64GtU => Self::Above,
            Operator::I32Eq | Operator::I64Eq => Self::Equal,
            Operator::I32Ne | Operator::I64Ne => Self::NotEqual,
            Operator::I32LtS
            | Operator::I32GtS
            | Operator::I32LeS
            | Operator::I32GeS
            | Operator::I64LtS
            | Operator::I64GtS
            | Operator::I64LeS
            | Operator::I64GeS => return Some(Err(())),
            _ => return None,
        }))
    }
}

/// What the comparison `operator` of `a` and `b` gives, when it is one.
fn compare(operator: &Operator<'_>, a: &Slot, b: &Slot) -> Option<Value> {
    if let Some(kind) = Compare::of(operator) {
        let test = match (kind, a.local, b.local) {
            (Err(()), local, other) => {
                let facts: Vec<Fact> = [local, other]
                    .into_iter()
                    .flatten()
                    .map(|local| Fact::Signed { local })
                    .collect();
                Test {
                    when_true: facts.clone(),
                    when_false: facts,
                }
            }
            (Ok(kind), Some(local), _) => bound_facts(local, kind, b),
            (Ok(kind), None, Some(local)) => bound_facts(local, kind.swapped(), a),
            (Ok(_), None, None) => Test::default(),
        };
        return Some(Value::Test(Box::new(test)));
    }
    let same = a.local.filter(|local| b.local == Some(*local));
    let test = match operator {
        Operator::F32Eq | Operator::F64Eq => Test {
            when_true: same
                .map(|local| vec![Fact::NotNan { local }])
                .unwrap_or_default(),
            when_false: Vec::new(),
        },
        Operator::F32Ne | Operator::F64Ne => Test {
            when_true: Vec::new(),
            when_false: same
                .map(|local| vec![Fact::NotNan { local }])
                .unwrap_or_default(),
        },
        Operator::F32Lt
        | Operator::F32Gt
        | Operator::F32Le
        | Operator::F32Ge
        | Operator::F64Lt
        | Operator::F64Gt
        | Operator::F64Le
        | Operator::F64Ge => Test::default(),
        _ => return None,
    };
    Some(Value::Test(Box::new(test)))
}

/// What a condition says when it's true and when it's false: a comparison's facts, or for a
/// local tested as it is, that it isn't zero or is.
fn condition_facts(slot: &Slot) -> (Vec<Fact>, Vec<Fact>) {
    match (&slot.value, slot.local) {
        (Value::Test(test), _) => (test.when_true.clone(), test.when_false.clone()),
        (_, Some(local)) => (
            vec![Fact::AtLeast { local, limit: 1 }],
            vec![Fact::Zero { local }],
        ),
        _ => (Vec::new(), Vec::new()),
    }
}

/// A local as the checker follows it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Local {
    /// Its value.
    value: Value,
    /// Which version of it this is.
    version: Version,
}

/// What the checker knows at a point of a function.
#[derive(Debug, Clone, PartialEq, Eq)]
struct State {
    /// Whether any path reaches it.
    reachable: bool,
    /// Each local, its parameters first.
    locals: Vec<Local>,
    /// What the code wrote at a constant offset of a region, with a 4-byte store, by region and
    /// offset: what an iovec array holds.
    memory: BTreeMap<(usize, i64), Value>,
    /// The versions of locals a check has shown aren't zero, whatever their values are: a
    /// divisor's guard.
    nonzero: BTreeSet<Version>,
}

impl State {
    /// What holds where paths from `self` and `other` meet, at the operator `at`.
    fn joined(&self, other: &Self, at: usize) -> Self {
        if !self.reachable {
            return other.clone();
        }
        if !other.reachable {
            return self.clone();
        }
        let nonzero = self.nonzero.intersection(&other.nonzero).copied().collect();
        let locals = self
            .locals
            .iter()
            .zip(&other.locals)
            .enumerate()
            .map(|(index, (mine, theirs))| Local {
                value: join(&mine.value, &theirs.value),
                version: if mine.version == theirs.version {
                    mine.version
                } else {
                    (at, to_u32(index))
                },
            })
            .collect();
        let memory = self
            .memory
            .iter()
            .filter(|(key, value)| other.memory.get(*key) == Some(*value))
            .map(|(key, value)| (*key, value.clone()))
            .collect();
        Self {
            reachable: true,
            locals,
            memory,
            nonzero,
        }
    }

    /// Applies `facts`, which hold here: each of the version of its local it was established
    /// for, none of a value the local has held since.
    fn apply(&mut self, facts: &[Fact]) {
        for fact in facts {
            if !self.reachable {
                return;
            }
            let version = match fact {
                Fact::Below { local, .. }
                | Fact::AtLeast { local, .. }
                | Fact::Zero { local }
                | Fact::NotNan { local }
                | Fact::Signed { local } => *local,
            };
            let (_, index) = version;
            let Some(slot) = self.locals.get_mut(to_usize(index)) else {
                continue;
            };
            if slot.version != version {
                continue;
            }
            match fact {
                Fact::AtLeast { limit, .. } if *limit >= 1 => {
                    self.nonzero.insert(version);
                }
                Fact::Zero { .. } => {
                    self.nonzero.remove(&version);
                }
                _ => {}
            }
            let mut impossible = false;
            let replaced = match (*fact, &mut slot.value) {
                (Fact::Below { limit: Some(0), .. }, _) => {
                    impossible = true;
                    None
                }
                (Fact::Below { limit, .. }, Value::Number(range)) => {
                    if let Some(limit) = limit {
                        range.high = range.high.min(limit - 1);
                        impossible = range.low > range.high;
                    }
                    range.checked = true;
                    None
                }
                // A bounds check turns what it checks into an index: a number below its limit.
                (
                    Fact::Below { limit, .. },
                    Value::Loaded
                    | Value::Param { .. }
                    | Value::Mixed { number: true }
                    | Value::Other,
                ) => Some(Value::Number(Range {
                    checked: true,
                    ..limit.map_or_else(Range::full, |limit| Range::between(0, limit - 1))
                })),
                (Fact::AtLeast { limit, .. }, Value::Number(range)) => {
                    range.low = range.low.max(limit);
                    impossible = range.low > range.high;
                    None
                }
                (Fact::Zero { .. }, value) => {
                    impossible = matches!(value, Value::Number(range) if range.low > 0);
                    Some(Value::Number(Range::exact(0)))
                }
                (Fact::NotNan { .. }, Value::Float { nan }) => {
                    *nan = false;
                    None
                }
                _ => None,
            };
            if let Some(value) = replaced {
                slot.value = value;
            }
            if impossible {
                self.reachable = false;
            }
        }
    }
}

/// What an operator took off the stack, and for a call that borrows an iovec array, what its
/// entries hold.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    /// The values it took, the deepest first: a call's arguments in order.
    pub operands: Vec<Value>,
    /// For a host call with an iovec array: each entry's address and length, as the code before
    /// the call wrote them; `None` when the rules can't say.
    pub iovecs: Option<Vec<(Value, Value)>>,
}

/// How far a function reaches through what a parameter lends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// This many bytes from the address.
    Bytes(u64),
    /// As far as the rules can't say.
    Unsettled,
}

impl Reach {
    /// The farther of the two.
    fn farther(self, other: Self) -> Self {
        match (self, other) {
            (Self::Bytes(a), Self::Bytes(b)) => Self::Bytes(a.max(b)),
            _ => Self::Unsettled,
        }
    }
}

/// What a function does through an address a parameter lends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Borrow {
    /// Whether it writes through it.
    pub written: bool,
    /// How far it reaches.
    pub reach: Reach,
}

/// What a function of main.wat's does with what it's lent, as its callers see it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// What it does through each parameter it uses as an address.
    pub borrows: BTreeMap<u32, Borrow>,
    /// Pairs of its parameters that must name parts of memory apart: it writes through the first
    /// while it reads or writes through the second.
    pub apart: BTreeSet<(u32, u32)>,
    /// What it returns, for one result: a parameter stands for what the caller passes.
    pub returns: Option<Value>,
}

/// How many bytes an access, or a borrowed buffer, may touch from its address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Room {
    /// This many.
    Bytes(u64),
    /// As many as this value.
    Length(Value),
    /// As many as the rules can't say.
    Unsettled,
}

/// An access to memory, or a buffer a call borrows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Access {
    /// The operator's index in its function.
    pub at: usize,
    /// Its line in main.wat.
    pub line: Option<usize>,
    /// What makes it, for reports.
    pub what: String,
    /// The address it starts at.
    pub address: Value,
    /// How many bytes from there it may touch.
    pub room: Room,
    /// Whether it writes.
    pub written: bool,
    /// For a call's borrow, the argument it's lent in.
    pub argument: Option<usize>,
}

/// One function as the checker followed it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Followed {
    /// What each operator took, by index; `None` where no path reaches it.
    pub records: Vec<Option<Record>>,
    /// Its accesses and the buffers its calls borrow.
    pub accesses: Vec<Access>,
    /// Whether its blocks nest too deep to follow.
    pub too_deep: bool,
}

/// What following every function of main.wat found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Analysis {
    /// Each function as followed, by index.
    pub followed: BTreeMap<u32, Followed>,
    /// What each does with what it's lent, by index.
    pub summaries: BTreeMap<u32, Summary>,
}

/// A memory instruction: its immediate, the bytes it touches, whether it writes, how many values
/// it takes and its name.
type MemoryOp = (MemArg, u64, bool, usize, &'static str);

/// The memory instruction `operator` is, when it is one.
#[must_use]
#[allow(clippy::too_many_lines)]
fn memory_op(operator: &Operator<'_>) -> Option<MemoryOp> {
    use Operator as O;
    Some(match *operator {
        O::I32Load { memarg } => (memarg, 4, false, 1, "i32.load"),
        O::I64Load { memarg } => (memarg, 8, false, 1, "i64.load"),
        O::F32Load { memarg } => (memarg, 4, false, 1, "f32.load"),
        O::F64Load { memarg } => (memarg, 8, false, 1, "f64.load"),
        O::I32Load8S { memarg } => (memarg, 1, false, 1, "i32.load8_s"),
        O::I32Load8U { memarg } => (memarg, 1, false, 1, "i32.load8_u"),
        O::I32Load16S { memarg } => (memarg, 2, false, 1, "i32.load16_s"),
        O::I32Load16U { memarg } => (memarg, 2, false, 1, "i32.load16_u"),
        O::I64Load8S { memarg } => (memarg, 1, false, 1, "i64.load8_s"),
        O::I64Load8U { memarg } => (memarg, 1, false, 1, "i64.load8_u"),
        O::I64Load16S { memarg } => (memarg, 2, false, 1, "i64.load16_s"),
        O::I64Load16U { memarg } => (memarg, 2, false, 1, "i64.load16_u"),
        O::I64Load32S { memarg } => (memarg, 4, false, 1, "i64.load32_s"),
        O::I64Load32U { memarg } => (memarg, 4, false, 1, "i64.load32_u"),
        O::I32Store { memarg } => (memarg, 4, true, 2, "i32.store"),
        O::I64Store { memarg } => (memarg, 8, true, 2, "i64.store"),
        O::F32Store { memarg } => (memarg, 4, true, 2, "f32.store"),
        O::F64Store { memarg } => (memarg, 8, true, 2, "f64.store"),
        O::I32Store8 { memarg } => (memarg, 1, true, 2, "i32.store8"),
        O::I32Store16 { memarg } => (memarg, 2, true, 2, "i32.store16"),
        O::I64Store8 { memarg } => (memarg, 1, true, 2, "i64.store8"),
        O::I64Store16 { memarg } => (memarg, 2, true, 2, "i64.store16"),
        O::I64Store32 { memarg } => (memarg, 4, true, 2, "i64.store32"),
        O::V128Load { memarg } => (memarg, 16, false, 1, "v128.load"),
        O::V128Load8x8S { memarg } => (memarg, 8, false, 1, "v128.load8x8_s"),
        O::V128Load8x8U { memarg } => (memarg, 8, false, 1, "v128.load8x8_u"),
        O::V128Load16x4S { memarg } => (memarg, 8, false, 1, "v128.load16x4_s"),
        O::V128Load16x4U { memarg } => (memarg, 8, false, 1, "v128.load16x4_u"),
        O::V128Load32x2S { memarg } => (memarg, 8, false, 1, "v128.load32x2_s"),
        O::V128Load32x2U { memarg } => (memarg, 8, false, 1, "v128.load32x2_u"),
        O::V128Load8Splat { memarg } => (memarg, 1, false, 1, "v128.load8_splat"),
        O::V128Load16Splat { memarg } => (memarg, 2, false, 1, "v128.load16_splat"),
        O::V128Load32Splat { memarg } => (memarg, 4, false, 1, "v128.load32_splat"),
        O::V128Load64Splat { memarg } => (memarg, 8, false, 1, "v128.load64_splat"),
        O::V128Load32Zero { memarg } => (memarg, 4, false, 1, "v128.load32_zero"),
        O::V128Load64Zero { memarg } => (memarg, 8, false, 1, "v128.load64_zero"),
        O::V128Store { memarg } => (memarg, 16, true, 2, "v128.store"),
        O::V128Load8Lane { memarg, .. } => (memarg, 1, false, 2, "v128.load8_lane"),
        O::V128Load16Lane { memarg, .. } => (memarg, 2, false, 2, "v128.load16_lane"),
        O::V128Load32Lane { memarg, .. } => (memarg, 4, false, 2, "v128.load32_lane"),
        O::V128Load64Lane { memarg, .. } => (memarg, 8, false, 2, "v128.load64_lane"),
        O::V128Store8Lane { memarg, .. } => (memarg, 1, true, 2, "v128.store8_lane"),
        O::V128Store16Lane { memarg, .. } => (memarg, 2, true, 2, "v128.store16_lane"),
        O::V128Store32Lane { memarg, .. } => (memarg, 4, true, 2, "v128.store32_lane"),
        O::V128Store64Lane { memarg, .. } => (memarg, 8, true, 2, "v128.store64_lane"),
        _ => return None,
    })
}

/// What a load gives.
fn loaded(operator: &Operator<'_>) -> Value {
    match operator {
        Operator::I32Load8U { .. } | Operator::I64Load8U { .. } => {
            Value::Number(Range::between(0, 255))
        }
        Operator::I32Load16U { .. } | Operator::I64Load16U { .. } => {
            Value::Number(Range::between(0, 65_535))
        }
        Operator::I64Load32U { .. } => Value::Number(Range::between(0, u64::from(u32::MAX))),
        Operator::I32Load { .. } => Value::Loaded,
        Operator::F32Load { .. } | Operator::F64Load { .. } => Value::Float { nan: true },
        Operator::I32Load8S { .. }
        | Operator::I32Load16S { .. }
        | Operator::I64Load { .. }
        | Operator::I64Load8S { .. }
        | Operator::I64Load16S { .. }
        | Operator::I64Load32S { .. } => Value::Number(Range::full()),
        _ => Value::Other,
    }
}

/// Whether `operator` divides integers, and so traps on a zero divisor.
fn is_division(operator: &Operator<'_>) -> bool {
    matches!(
        operator,
        Operator::I32DivS
            | Operator::I32DivU
            | Operator::I32RemS
            | Operator::I32RemU
            | Operator::I64DivS
            | Operator::I64DivU
            | Operator::I64RemS
            | Operator::I64RemU
    )
}

/// The width in bits of the integers `operator` works on, for those whose results the rules
/// follow.
fn bits_of(operator: &Operator<'_>) -> Option<u32> {
    use Operator as O;
    match operator {
        O::I32Add
        | O::I32Sub
        | O::I32Mul
        | O::I32And
        | O::I32Or
        | O::I32Xor
        | O::I32Shl
        | O::I32ShrU
        | O::I32RemU
        | O::I32DivU
        | O::I32Clz
        | O::I32Ctz
        | O::I32Popcnt => Some(32),
        O::I64Add
        | O::I64Sub
        | O::I64Mul
        | O::I64And
        | O::I64Or
        | O::I64Xor
        | O::I64Shl
        | O::I64ShrU
        | O::I64RemU
        | O::I64DivU
        | O::I64Clz
        | O::I64Ctz
        | O::I64Popcnt => Some(64),
        _ => None,
    }
}

/// What a float operator gives, when it gives a float: whether it may be NaN, from its operands.
fn float_result(operator: &Operator<'_>, operands: &[Slot]) -> Option<Value> {
    use Operator as O;
    let first_nan = operands
        .first()
        .is_none_or(|slot| !matches!(slot.value, Value::Float { nan: false }));
    let any_nan = operands
        .iter()
        .any(|slot| !matches!(slot.value, Value::Float { nan: false }));
    let nan = match operator {
        O::F32Abs
        | O::F32Neg
        | O::F32Copysign
        | O::F32Ceil
        | O::F32Floor
        | O::F32Trunc
        | O::F32Nearest
        | O::F64Abs
        | O::F64Neg
        | O::F64Copysign
        | O::F64Ceil
        | O::F64Floor
        | O::F64Trunc
        | O::F64Nearest
        | O::F32DemoteF64
        | O::F64PromoteF32 => first_nan,
        O::F32Min | O::F32Max | O::F64Min | O::F64Max => any_nan,
        O::F32Add
        | O::F32Sub
        | O::F32Mul
        | O::F32Div
        | O::F32Sqrt
        | O::F64Add
        | O::F64Sub
        | O::F64Mul
        | O::F64Div
        | O::F64Sqrt
        | O::F32ReinterpretI32
        | O::F64ReinterpretI64 => true,
        O::F32ConvertI32S
        | O::F32ConvertI32U
        | O::F32ConvertI64S
        | O::F32ConvertI64U
        | O::F64ConvertI32S
        | O::F64ConvertI32U
        | O::F64ConvertI64S
        | O::F64ConvertI64U => false,
        _ => return None,
    };
    Some(Value::Float { nan })
}

/// What `$fill(fd, at, room, stop)` borrows: the `room` bytes at `at`, which it writes.
const FILL_LENDS: &[wasi::Lend] = &[wasi::Lend {
    argument: 1,
    bytes: Bytes::Argument(2),
    written: true,
}];

/// What `$write_all(fd, at, len)` borrows: the `len` bytes at `at`, which it reads.
const WRITE_ALL_LENDS: &[wasi::Lend] = &[wasi::Lend {
    argument: 1,
    bytes: Bytes::Argument(2),
    written: false,
}];

/// No buffers.
const NO_LENDS: &[wasi::Lend] = &[];

/// The buffers a call of `function` borrows directly, as WASI's table lists them for the host's
/// functions, and the library's own say for theirs.
fn lends_of(function: &Function<'_>) -> &'static [wasi::Lend] {
    match function.kind {
        Kind::Import | Kind::LibraryImport => match function.host() {
            Some(host) => host.lends,
            None => NO_LENDS,
        },
        Kind::Library => match function.name.as_deref() {
            Some("fill") => FILL_LENDS,
            Some("write_all") => WRITE_ALL_LENDS,
            _ => NO_LENDS,
        },
        Kind::Own => NO_LENDS,
    }
}

/// Whether a host call writes into the buffers its iovecs point to.
fn writes_iovec_buffers(function: &Function<'_>) -> bool {
    function.is_host("fd_read") || function.is_host("sock_recv")
}

/// A block being followed.
#[derive(Debug)]
struct Frame {
    /// What kind of block it is.
    kind: FrameKind,
    /// The stack's height below its parameters.
    base: usize,
    /// For a loop, how many parameters it takes: what a branch back to its start carries.
    params: usize,
    /// How many results it gives.
    results: usize,
    /// The states of the branches that leave to its end, with the values they carry.
    exits: Vec<(State, Vec<Value>)>,
    /// For a loop, the states of the branches back to its start, with the parameters they carry.
    backs: Vec<(State, Vec<Value>)>,
}

impl Frame {
    /// A block of `kind`, `base` the stack's height below its parameters.
    fn new(kind: FrameKind, base: usize, results: usize) -> Self {
        Self {
            kind,
            base,
            params: 0,
            results,
            exits: Vec::new(),
            backs: Vec::new(),
        }
    }
}

/// Follows the values of one function.
struct Interpreter<'m, 'a> {
    /// The module.
    module: &'m Module<'a>,
    /// The function.
    function: &'m Function<'a>,
    /// Its control structure.
    shape: &'m Shape,
    /// What its callees do with what they're lent.
    summaries: &'m BTreeMap<u32, Summary>,
    /// The operand stack.
    stack: Vec<Slot>,
    /// The blocks it's inside, the function's body first.
    frames: Vec<Frame>,
    /// What each operator took.
    records: Vec<Option<Record>>,
    /// How many operators it has followed.
    steps: usize,
    /// How deep inside blocks it is.
    nesting: usize,
    /// Whether the blocks nest too deep to follow.
    too_deep: bool,
}

/// The locals written between `at` and `end`.
fn written_between(function: &Function<'_>, at: usize, end: usize) -> BTreeSet<u32> {
    function
        .body
        .iter()
        .take(end)
        .skip(at)
        .filter_map(|op| match op.operator {
            Operator::LocalSet { local_index } | Operator::LocalTee { local_index } => {
                Some(local_index)
            }
            _ => None,
        })
        .collect()
}

/// `value` grown past `before` where it grew, so a loop's head settles: a bound that moved is
/// dropped.
fn widened(before: &Value, after: &Value) -> Value {
    match (before, after) {
        (Value::Number(was), Value::Number(now)) => Value::Number(Range {
            low: if now.low < was.low { 0 } else { now.low },
            high: if now.high > was.high {
                u64::MAX
            } else {
                now.high
            },
            checked: now.checked && now.high <= was.high,
            linear: None,
        }),
        (
            Value::Address {
                region: was_region,
                offset: was,
            },
            Value::Address { region, offset },
        ) if was_region == region => Value::Address {
            region: *region,
            offset: offset_widened(*was, *offset),
        },
        (
            Value::Param {
                param: was_param,
                offset: was,
            },
            Value::Param { param, offset },
        ) if was_param == param => Value::Param {
            param: *param,
            offset: offset_widened(*was, *offset),
        },
        // One of several: each owner's grown past what it was.
        (_, Value::Several(now)) => {
            let was = addresses(before).unwrap_or_default();
            let grown = now.iter().map(|item| {
                let old = was.iter().find(|old| owner(old) == owner(item));
                old.map_or_else(|| item.clone(), |old| widened(old, item))
            });
            Value::Several(grown.collect())
        }
        _ => after.clone(),
    }
}

/// An offset grown past `was` where it grew.
fn offset_widened(was: Offset, now: Offset) -> Offset {
    let low = match (was.low, now.low) {
        (Some(was), Some(now)) if now >= was => Some(now),
        _ => None,
    };
    let high = match (was.high, now.high) {
        (Some(was), Some(now)) if now <= was => Some(now),
        _ => None,
    };
    Offset {
        low,
        high,
        checked: now.checked && high.is_some() && low.is_some(),
        linear: None,
        index: now.index,
    }
}

impl Interpreter<'_, '_> {
    /// What a local of type `ty` may hold when the checker can't say: anything of its type.
    fn anything(ty: Option<ValType>) -> Value {
        match ty {
            Some(ValType::I32) => Value::Mixed { number: true },
            Some(ValType::I64) => Value::Number(Range::full()),
            Some(ValType::F32 | ValType::F64) => Value::Float { nan: true },
            _ => Value::Other,
        }
    }

    /// The state when the function starts: each parameter what the caller passes, each other
    /// local zero.
    fn entry(&self) -> State {
        let function = self.function;
        let entry = function.body.len();
        let mut locals = Vec::with_capacity(function.local_count());
        for (index, ty) in function.params.iter().enumerate() {
            let value = match ty {
                ValType::I32 => Value::Param {
                    param: to_u32(index),
                    offset: Offset::ZERO,
                },
                other => Self::anything(Some(*other)),
            };
            locals.push(Local {
                value,
                version: (entry, to_u32(index)),
            });
        }
        for (position, ty) in function.locals.iter().enumerate() {
            let value = match ty {
                ValType::I32 | ValType::I64 => Value::Number(Range::exact(0)),
                ValType::F32 | ValType::F64 => Value::Float { nan: false },
                _ => Value::Other,
            };
            locals.push(Local {
                value,
                version: (entry, to_u32(function.params.len() + position)),
            });
        }
        State {
            reachable: true,
            locals,
            memory: BTreeMap::new(),
            nonzero: BTreeSet::new(),
        }
    }

    /// Takes the top value off the stack.
    fn pop(&mut self) -> Slot {
        self.stack
            .pop()
            .unwrap_or_else(|| Slot::new(Value::Mixed { number: true }))
    }

    /// Takes the top `count` values off the stack, the deepest first.
    fn pop_n(&mut self, count: usize) -> Vec<Slot> {
        let start = self.stack.len().saturating_sub(count);
        let mut taken = self.stack.split_off(start);
        while taken.len() < count {
            taken.insert(0, Slot::new(Value::Mixed { number: true }));
        }
        taken
    }

    /// The top `count` values of the stack, the deepest first.
    fn top(&self, count: usize) -> Vec<Value> {
        let start = self.stack.len().saturating_sub(count);
        let mut values: Vec<Value> = self.stack[start..]
            .iter()
            .map(|slot| slot.value.clone())
            .collect();
        while values.len() < count {
            values.insert(0, Value::Mixed { number: true });
        }
        values
    }

    /// Puts `value` on the stack.
    fn push(&mut self, value: Value) {
        self.stack.push(Slot::new(value));
    }

    /// Keeps what the operator at `at` took.
    fn record(&mut self, at: usize, operands: Vec<Value>) {
        if let Some(record) = self.records.get_mut(at) {
            *record = Some(Record {
                operands,
                iovecs: None,
            });
        }
    }

    /// Sends `state` along a branch to the block `depth` levels out.
    fn branch(&mut self, depth: u32, state: State) {
        let Some(index) = self
            .frames
            .len()
            .checked_sub(to_usize(depth).saturating_add(1))
        else {
            return;
        };
        let (params, results) = (self.frames[index].params, self.frames[index].results);
        if self.frames[index].kind == FrameKind::Loop {
            // A branch back to a loop's start carries its parameters.
            let carried = self.top(params);
            self.frames[index].backs.push((state, carried));
        } else {
            let carried = self.top(results);
            self.frames[index].exits.push((state, carried));
        }
    }

    /// Takes every local the code from `at` writes as anything, and memory as unknown: what a
    /// block the checker doesn't follow leaves.
    fn havoc(&self, state: &mut State, written: &BTreeSet<u32>, at: usize) {
        for local in written {
            let ty = self.function.local_type(*local);
            if let Some(slot) = state.locals.get_mut(to_usize(*local)) {
                slot.value = Self::anything(ty);
                slot.version = (at, *local);
            }
        }
        state.nonzero.retain(|(_, index)| !written.contains(index));
        state.memory.clear();
    }

    /// Follows the code from `at` to the `end` or `else` that ends its block, and gives that
    /// operator's index.
    fn run(&mut self, mut at: usize, state: &mut State) -> usize {
        let function = self.function;
        while at < function.body.len() {
            match function.body[at].operator {
                Operator::Block { blockty } => at = self.block(at, blockty, state),
                Operator::Loop { blockty } => at = self.looped(at, blockty, state),
                Operator::If { blockty } => at = self.branch_if(at, blockty, state),
                Operator::Else | Operator::End => return at,
                _ => {
                    self.step(at, state);
                    at += 1;
                }
            }
        }
        at
    }

    /// Skips the block opened at `at`, too deep to follow, leaving anything it may.
    fn skip(&mut self, at: usize, end: usize, results: usize, state: &mut State) -> usize {
        self.too_deep = true;
        let written = written_between(self.function, at, end);
        self.havoc(state, &written, at);
        for _ in 0..results {
            self.push(Value::Mixed { number: true });
        }
        end + 1
    }

    /// Ends the block `frame`, at `at`: what holds after it is what holds on every way out.
    fn close(&mut self, frame: Option<Frame>, state: &mut State, at: usize) {
        let Some(frame) = frame else {
            return;
        };
        let mut joined: Option<(State, Vec<Value>)> = state
            .reachable
            .then(|| (state.clone(), self.top(frame.results)));
        for (exit, values) in frame.exits {
            joined = Some(match joined {
                None => (exit, values),
                Some((known, carried)) => {
                    let values = carried
                        .iter()
                        .zip(&values)
                        .map(|(mine, theirs)| join(mine, theirs))
                        .collect();
                    (known.joined(&exit, at), values)
                }
            });
        }
        self.stack.truncate(frame.base);
        match joined {
            Some((after, values)) => {
                *state = after;
                self.stack.extend(values.into_iter().map(Slot::new));
            }
            None => {
                state.reachable = false;
                for _ in 0..frame.results {
                    self.push(Value::Mixed { number: true });
                }
            }
        }
    }

    /// Follows the `block` at `at`, and gives the index after its end.
    fn block(&mut self, at: usize, ty: BlockType, state: &mut State) -> usize {
        let Some(&end) = self.shape.ends.get(&at) else {
            return self.function.body.len();
        };
        let (params, results) = self.module.block_arity(ty);
        if self.nesting >= MAX_NESTING {
            return self.skip(at, end, results, state);
        }
        let base = self.stack.len().saturating_sub(params);
        self.frames
            .push(Frame::new(FrameKind::Block, base, results));
        self.nesting += 1;
        let stop = self.run(at + 1, state);
        self.nesting -= 1;
        let frame = self.frames.pop();
        self.close(frame, state, stop);
        stop + 1
    }

    /// What the parameters of a block of type `ty` may hold when the checker can't say:
    /// anything of their types.
    fn anything_of(&self, ty: BlockType) -> Vec<Value> {
        self.module
            .block_params(ty)
            .into_iter()
            .map(|param| Self::anything(Some(param)))
            .collect()
    }

    /// Follows the `loop` at `at` until what holds at its start settles, and gives the index
    /// after its end. What holds at its start is what it's entered with, joined with what each
    /// branch back to it brings: the locals, and the parameters the branch carries.
    fn looped(&mut self, at: usize, ty: BlockType, state: &mut State) -> usize {
        let Some(&end) = self.shape.ends.get(&at) else {
            return self.function.body.len();
        };
        let (params, results) = self.module.block_arity(ty);
        if self.nesting >= MAX_NESTING {
            return self.skip(at, end, results, state);
        }
        let base = self.stack.len().saturating_sub(params);
        let entry_stack = self.stack.clone();
        let written = written_between(self.function, at, end);
        let mut head = state.clone();
        for local in &written {
            if let Some(slot) = head.locals.get_mut(to_usize(*local)) {
                slot.version = (at, *local);
            }
        }
        // The parameters at its start, first as it's entered.
        let mut carried = self.top(params);
        let mut forced = self.steps > BUDGET;
        if forced {
            self.havoc(&mut head, &written, at);
            carried = self.anything_of(ty);
        }
        let mut iteration = 0;
        loop {
            iteration += 1;
            self.stack.clone_from(&entry_stack);
            self.stack.truncate(base);
            self.stack.extend(carried.iter().cloned().map(Slot::new));
            self.frames.push(Frame {
                params,
                ..Frame::new(FrameKind::Loop, base, results)
            });
            let mut body = head.clone();
            self.nesting += 1;
            let stop = self.run(at + 1, &mut body);
            self.nesting -= 1;
            let backs = self
                .frames
                .pop()
                .map(|frame| frame.backs)
                .unwrap_or_default();
            let mut next = head.clone();
            let mut arriving = carried.clone();
            for (back, values) in &backs {
                next = next.joined(back, at);
                arriving = arriving
                    .iter()
                    .zip(values)
                    .map(|(mine, theirs)| join(mine, theirs))
                    .collect();
            }
            for (index, local) in next.locals.iter_mut().enumerate() {
                if let Some(was) = head.locals.get(index) {
                    local.value = widened(&was.value, &local.value);
                    if written.contains(&to_u32(index)) {
                        local.version = (at, to_u32(index));
                    }
                }
            }
            let mut grown: Vec<Value> = carried
                .iter()
                .zip(&arriving)
                .map(|(was, now)| widened(was, now))
                .collect();
            if (next == head && grown == carried) || forced {
                let values = self.top(results);
                self.stack.truncate(base);
                self.stack.extend(values.into_iter().map(Slot::new));
                *state = body;
                return stop + 1;
            }
            if iteration >= MAX_ITERATIONS || self.steps > BUDGET {
                self.havoc(&mut next, &written, at);
                grown = self.anything_of(ty);
                forced = true;
            }
            head = next;
            carried = grown;
        }
    }

    /// Follows the `if` at `at`, each arm with what its condition says, and gives the index after
    /// its end.
    fn branch_if(&mut self, at: usize, ty: BlockType, state: &mut State) -> usize {
        let Some(&end) = self.shape.ends.get(&at) else {
            return self.function.body.len();
        };
        let (params, results) = self.module.block_arity(ty);
        let condition = if state.reachable {
            let condition = self.pop();
            self.record(at, vec![condition.value.clone()]);
            condition
        } else {
            Slot::new(Value::Other)
        };
        if self.nesting >= MAX_NESTING {
            return self.skip(at, end, results, state);
        }
        let base = self.stack.len().saturating_sub(params);
        let entry_stack = self.stack.clone();
        let (when_true, when_false) = condition_facts(&condition);
        let mut then_state = state.clone();
        then_state.apply(&when_true);
        let mut else_state = state.clone();
        else_state.apply(&when_false);
        self.frames.push(Frame::new(FrameKind::If, base, results));
        self.nesting += 1;
        let stop = self.run(at + 1, &mut then_state);
        let last = if self.shape.elses.get(&at) == Some(&stop) {
            let carried = self.top(results);
            if then_state.reachable
                && let Some(frame) = self.frames.last_mut()
            {
                frame.exits.push((then_state, carried));
            }
            self.stack.clone_from(&entry_stack);
            let last = self.run(stop + 1, &mut else_state);
            *state = else_state;
            last
        } else {
            // With no `else`, a false condition goes straight to the end, its parameters its
            // results.
            let carried: Vec<Value> = entry_stack
                .get(base..)
                .unwrap_or_default()
                .iter()
                .map(|slot| slot.value.clone())
                .collect();
            if else_state.reachable
                && let Some(frame) = self.frames.last_mut()
            {
                frame.exits.push((else_state, carried));
            }
            *state = then_state;
            stop
        };
        self.nesting -= 1;
        let frame = self.frames.pop();
        self.close(frame, state, last);
        last + 1
    }

    /// The value `global.get` of global `index` gives.
    fn global(&self, index: u32) -> Value {
        if let Some(region) = self.module.region_of(index) {
            return Value::Address {
                region,
                offset: Offset::ZERO,
            };
        }
        let Some(global) = self.module.global(index) else {
            return Value::Other;
        };
        match (global.ty, global.unsigned()) {
            (ValType::I32 | ValType::I64, Some(value)) => Value::Number(Range::exact(value)),
            // A global that changes may hold an address the rules don't follow.
            (ValType::I32, None) => Value::Loaded,
            (ValType::I64, None) => Value::Number(Range::full()),
            (ValType::F32 | ValType::F64, _) => Value::Float { nan: true },
            _ => Value::Other,
        }
    }

    /// Reads local `index`: its value, a number in terms of its own version.
    fn read_local(state: &State, index: u32) -> Slot {
        let Some(local) = state.locals.get(to_usize(index)) else {
            return Slot::new(Value::Other);
        };
        let mut value = local.value.clone();
        if let Value::Number(range) = &mut value {
            range.linear = Some(Linear {
                version: local.version,
                sign: 1,
                constant: 0,
            });
        }
        Slot {
            value,
            local: Some(local.version),
        }
    }

    /// Writes `value` to local `index` at `at`: a new version of it, of which no check has said
    /// anything yet.
    fn write_local(state: &mut State, at: usize, index: u32, value: Value) {
        if let Some(local) = state.locals.get_mut(to_usize(index)) {
            local.value = value.stored();
            local.version = (at, index);
        }
        state.nonzero.retain(|(_, local)| *local != index);
    }

    /// What a store of `width` bytes through `address` at `at` does to what memory is known to
    /// hold; a 4-byte store keeps `value`.
    fn stored(state: &mut State, address: &Value, width: u64, value: Option<Value>) {
        match address {
            Value::Address { region, offset } => match offset.constant() {
                Some(start) => {
                    let width = signed(width).unwrap_or(i64::MAX);
                    state.memory.retain(|(known, at), _| {
                        *known != *region || *at + 4 <= start || *at >= start.saturating_add(width)
                    });
                    if let Some(value) = value {
                        state.memory.insert((*region, start), value.stored());
                    }
                }
                None => state.memory.retain(|(known, _), _| *known != *region),
            },
            _ => state.memory.clear(),
        }
    }

    /// The entries of the iovec array `iovs` holds, `count` of them, as memory is known to
    /// hold them.
    fn iovecs(state: &State, iovs: &Value, count: &Value) -> Option<Vec<(Value, Value)>> {
        let Value::Address { region, offset } = iovs else {
            return None;
        };
        let start = offset.constant()?;
        let count = count
            .range()
            .constant()
            .filter(|count| *count <= MAX_IOVECS)?;
        let mut entries = Vec::new();
        for entry in 0..count {
            let at = start.checked_add(signed(entry)?.checked_mul(8)?)?;
            let buffer = state.memory.get(&(*region, at))?;
            let length = state.memory.get(&(*region, at.checked_add(4)?))?;
            entries.push((buffer.clone(), length.clone()));
        }
        Some(entries)
    }

    /// What a call of `function` with `arguments` gives back.
    fn results(&self, function: &Function<'_>, arguments: &[Value]) -> Vec<Value> {
        let summary = self.summaries.get(&function.index);
        let host = matches!(function.kind, Kind::Import | Kind::LibraryImport);
        function
            .results
            .iter()
            .map(|ty| match (ty, function.kind) {
                // A host call gives an error number.
                (ValType::I32, _) if host => Value::Number(Range::between(0, 65_535)),
                (ValType::I32, Kind::Library) => Value::Number(Range::full()),
                (ValType::I32, Kind::Own) if function.results.len() == 1 => {
                    match summary.and_then(|summary| summary.returns.clone()) {
                        Some(value) => returned(value, arguments),
                        None => Value::Mixed { number: true },
                    }
                }
                _ => Self::anything(Some(*ty)),
            })
            .collect()
    }

    /// Follows a call of function `callee` at `at`.
    fn call(&mut self, at: usize, callee: u32, state: &mut State) {
        let module = self.module;
        let Some(function) = module.function(callee) else {
            return;
        };
        let arguments: Vec<Value> = self
            .pop_n(function.params.len())
            .into_iter()
            .map(|slot| slot.value)
            .collect();
        let mut iovecs = None;
        for lend in lends_of(function) {
            if let Bytes::Table { count, entry } = lend.bytes
                && entry == wasi::size::IOVEC
                && let (Some(iovs), Some(count)) =
                    (arguments.get(lend.argument), arguments.get(count))
            {
                iovecs = Self::iovecs(state, iovs, count);
            }
        }
        // What the call may write, memory no longer holds what the code wrote there.
        match function.kind {
            Kind::Own => state.memory.clear(),
            _ => {
                for lend in lends_of(function).iter().filter(|lend| lend.written) {
                    if let Some(address) = arguments.get(lend.argument) {
                        Self::stored(state, address, u64::MAX, None);
                    }
                }
                if writes_iovec_buffers(function) {
                    match &iovecs {
                        Some(entries) => {
                            for (buffer, _) in entries {
                                Self::stored(state, buffer, u64::MAX, None);
                            }
                        }
                        None => state.memory.clear(),
                    }
                }
            }
        }
        let results = self.results(function, &arguments);
        if let Some(record) = self.records.get_mut(at) {
            *record = Some(Record {
                operands: arguments,
                iovecs,
            });
        }
        for value in results {
            self.push(value);
        }
        if function.never_returns() {
            state.reachable = false;
        }
    }

    /// Follows the operator at `at`, which isn't a block.
    #[allow(clippy::too_many_lines)]
    fn step(&mut self, at: usize, state: &mut State) {
        if !state.reachable {
            return;
        }
        self.steps += 1;
        let function = self.function;
        let operator = &function.body[at].operator;
        match operator {
            Operator::Unreachable => state.reachable = false,
            Operator::Nop | Operator::DataDrop { .. } => {}
            Operator::Br { relative_depth } => {
                self.branch(*relative_depth, state.clone());
                state.reachable = false;
            }
            Operator::BrIf { relative_depth } => {
                let condition = self.pop();
                self.record(at, vec![condition.value.clone()]);
                let (when_true, when_false) = condition_facts(&condition);
                let mut taken = state.clone();
                taken.apply(&when_true);
                if taken.reachable {
                    self.branch(*relative_depth, taken);
                }
                state.apply(&when_false);
            }
            Operator::BrTable { targets } => {
                let index = self.pop();
                self.record(at, vec![index.value]);
                for depth in targets.targets().flatten() {
                    self.branch(depth, state.clone());
                }
                self.branch(targets.default(), state.clone());
                state.reachable = false;
            }
            Operator::Return => {
                let depth = to_u32(self.frames.len().saturating_sub(1));
                self.branch(depth, state.clone());
                state.reachable = false;
            }
            Operator::Call { function_index } => self.call(at, *function_index, state),
            Operator::Drop => {
                let value = self.pop();
                self.record(at, vec![value.value]);
            }
            Operator::Select | Operator::TypedSelect { .. } => {
                let condition = self.pop();
                let second = self.pop();
                let first = self.pop();
                let joined = join(&first.value, &second.value);
                self.record(at, vec![first.value, second.value, condition.value]);
                self.push(joined);
            }
            Operator::LocalGet { local_index } => {
                let slot = Self::read_local(state, *local_index);
                self.stack.push(slot);
            }
            Operator::LocalSet { local_index } => {
                let slot = self.pop();
                self.record(at, vec![slot.value.clone()]);
                Self::write_local(state, at, *local_index, slot.value);
            }
            Operator::LocalTee { local_index } => {
                let slot = self.pop();
                self.record(at, vec![slot.value.clone()]);
                Self::write_local(state, at, *local_index, slot.value);
                let read = Self::read_local(state, *local_index);
                self.stack.push(read);
            }
            Operator::GlobalGet { global_index } => {
                let value = self.global(*global_index);
                self.push(value);
            }
            Operator::GlobalSet { .. } => {
                let value = self.pop();
                self.record(at, vec![value.value]);
            }
            Operator::I32Const { value } => {
                self.push(Value::Number(Range::exact(u64::from(
                    value.cast_unsigned(),
                ))));
            }
            Operator::I64Const { value } => {
                self.push(Value::Number(Range::exact(value.cast_unsigned())));
            }
            Operator::F32Const { value } => self.push(Value::Float {
                nan: f32::from_bits(value.bits()).is_nan(),
            }),
            Operator::F64Const { value } => self.push(Value::Float {
                nan: f64::from_bits(value.bits()).is_nan(),
            }),
            Operator::MemorySize { .. } => self.push(Value::Number(Range::between(0, 65_536))),
            Operator::MemoryCopy { .. }
            | Operator::MemoryFill { .. }
            | Operator::MemoryInit { .. } => {
                let operands = self.pop_n(3);
                if let Some(destination) = operands.first() {
                    Self::stored(state, &destination.value, u64::MAX, None);
                }
                self.record(at, operands.into_iter().map(|slot| slot.value).collect());
            }
            _ => {
                if let Some((memarg, width, store, takes, _)) = memory_op(operator) {
                    let operands = self.pop_n(takes);
                    let values: Vec<Value> = operands.into_iter().map(|slot| slot.value).collect();
                    if store {
                        let kept = matches!(operator, Operator::I32Store { .. })
                            .then(|| values.get(1).cloned())
                            .flatten();
                        let address = values
                            .first()
                            .map(|address| shifted(address, memarg.offset));
                        if let Some(address) = address {
                            Self::stored(state, &address, width, kept);
                        }
                    } else {
                        self.push(loaded(operator));
                    }
                    self.record(at, values);
                    return;
                }
                self.generic(at, operator, state);
            }
        }
    }

    /// Follows an operator the checker knows only by its arity and the kind of value it gives,
    /// in `state`.
    fn generic(&mut self, at: usize, operator: &Operator<'_>, state: &State) {
        let (takes, gives) = self.module.arity(&[], operator);
        let operands = self.pop_n(takes);
        let result = match operands.as_slice() {
            [a, b] => compare(operator, a, b).or_else(|| match operator {
                Operator::I32Add => Some(plus(a, b, 32)),
                Operator::I64Add => Some(plus(a, b, 64)),
                Operator::I32Sub => Some(minus(a, b, 32)),
                Operator::I64Sub => Some(minus(a, b, 64)),
                Operator::I32And | Operator::I64And
                    if matches!((&a.value, &b.value), (Value::Test(_), Value::Test(_))) =>
                {
                    let (Value::Test(x), Value::Test(y)) = (&a.value, &b.value) else {
                        return None;
                    };
                    Some(Value::Test(Box::new(Test {
                        when_true: x.when_true.iter().chain(&y.when_true).copied().collect(),
                        when_false: Vec::new(),
                    })))
                }
                Operator::I32Or | Operator::I64Or
                    if matches!((&a.value, &b.value), (Value::Test(_), Value::Test(_))) =>
                {
                    let (Value::Test(x), Value::Test(y)) = (&a.value, &b.value) else {
                        return None;
                    };
                    Some(Value::Test(Box::new(Test {
                        when_true: Vec::new(),
                        when_false: x.when_false.iter().chain(&y.when_false).copied().collect(),
                    })))
                }
                _ => bits_of(operator).map(|bits| {
                    Value::Number(range_of(operator, a.value.range(), b.value.range(), bits))
                }),
            }),
            [a] => match operator {
                Operator::I32Eqz | Operator::I64Eqz => Some(match (&a.value, a.local) {
                    (Value::Test(test), _) => Value::Test(Box::new(Test {
                        when_true: test.when_false.clone(),
                        when_false: test.when_true.clone(),
                    })),
                    (_, Some(local)) => Value::Test(Box::new(Test {
                        when_true: vec![Fact::Zero { local }],
                        when_false: vec![Fact::AtLeast { local, limit: 1 }],
                    })),
                    _ => Value::Test(Box::default()),
                }),
                Operator::I32WrapI64 => Some(Value::Number(a.value.range().width(32))),
                Operator::I64ExtendI32U => Some(Value::Number(Range {
                    linear: None,
                    ..a.value.range().width(32)
                })),
                Operator::I64ExtendI32S => {
                    let range = a.value.range().width(32);
                    Some(Value::Number(if range.high < 1 << 31 {
                        Range {
                            linear: None,
                            ..range
                        }
                    } else {
                        Range::unknown(&[range])
                    }))
                }
                _ => bits_of(operator).map(|bits| {
                    Value::Number(range_of(operator, a.value.range(), Range::full(), bits))
                }),
            },
            _ => None,
        };
        let result = result
            .or_else(|| float_result(operator, &operands))
            .unwrap_or(Value::Other);
        // A divisor a check showed isn't zero is kept so, whatever else its value is: the
        // version of its local the check read, not one written since.
        let divisor_checked = is_division(operator)
            && operands
                .get(1)
                .and_then(|slot| slot.local)
                .is_some_and(|version| state.nonzero.contains(&version));
        let mut values: Vec<Value> = operands.into_iter().map(|slot| slot.value).collect();
        if divisor_checked && let Some(divisor) = values.get_mut(1) {
            let range = divisor.range();
            if range.low == 0 {
                *divisor = Value::Number(Range { low: 1, ..range });
            }
        }
        self.record(at, values);
        for _ in 0..gives {
            self.push(result.clone());
        }
    }
}

/// An address `by` bytes on, as a memory instruction's offset moves it.
fn shifted(address: &Value, by: u64) -> Value {
    let Some(bytes) = signed(by) else {
        return Value::Mixed { number: false };
    };
    match address {
        Value::Address { region, offset } => Value::Address {
            region: *region,
            offset: offset.shifted(bytes),
        },
        Value::Param { param, offset } => Value::Param {
            param: *param,
            offset: offset.shifted(bytes),
        },
        Value::Several(items) => {
            Value::Several(items.iter().map(|item| shifted(item, by)).collect())
        }
        other => other.clone(),
    }
}

/// What a callee's result `value` is for its caller, which passed `arguments`: what the callee
/// was lent is what the caller lent it, and nothing that names one of the callee's own locals
/// crosses the call, neither a comparison's facts nor an index's name.
fn returned(value: Value, arguments: &[Value]) -> Value {
    match value {
        Value::Param { param, offset } => rebased(arguments.get(to_usize(param)), offset),
        Value::Address { region, offset } => Value::Address {
            region,
            offset: Offset {
                linear: None,
                index: None,
                ..offset
            },
        },
        Value::Number(range) => Value::Number(Range {
            linear: None,
            ..range
        }),
        Value::Test(_) => Value::Number(Range::between(0, 1)),
        Value::Several(items) => each(&items, |item| returned(item.clone(), arguments)),
        other => other,
    }
}

/// What a callee's result that is its parameter plus `offset` is for the caller, which passed
/// `argument`.
fn rebased(argument: Option<&Value>, offset: Offset) -> Value {
    let add = |base: Offset| Offset {
        low: base.low.zip(offset.low).and_then(|(a, b)| a.checked_add(b)),
        high: base
            .high
            .zip(offset.high)
            .and_then(|(a, b)| a.checked_add(b)),
        checked: base.checked && offset.checked,
        linear: None,
        index: None,
    };
    match argument {
        Some(Value::Address {
            region,
            offset: base,
        }) => Value::Address {
            region: *region,
            offset: add(*base),
        },
        Some(Value::Param {
            param,
            offset: base,
        }) => Value::Param {
            param: *param,
            offset: add(*base),
        },
        Some(Value::Several(items)) => each(items, |item| rebased(Some(item), offset)),
        Some(Value::Loaded) => Value::Loaded,
        _ => Value::Mixed { number: true },
    }
}

/// Follows every value of `function`.
fn follow(
    module: &Module<'_>,
    function: &Function<'_>,
    shape: &Shape,
    summaries: &BTreeMap<u32, Summary>,
) -> (Vec<Option<Record>>, Option<Value>, bool) {
    let mut interpreter = Interpreter {
        module,
        function,
        shape,
        summaries,
        stack: Vec::new(),
        frames: Vec::new(),
        records: vec![None; function.body.len()],
        steps: 0,
        nesting: 0,
        too_deep: false,
    };
    let mut state = interpreter.entry();
    let results = function.results.len();
    interpreter
        .frames
        .push(Frame::new(FrameKind::Block, 0, results));
    interpreter.run(0, &mut state);
    let frame = interpreter.frames.pop();
    let mut returned: Option<Vec<Value>> = state.reachable.then(|| interpreter.top(results));
    if let Some(frame) = frame {
        for (_, values) in frame.exits {
            returned = Some(match returned {
                None => values,
                Some(known) => known
                    .iter()
                    .zip(&values)
                    .map(|(mine, theirs)| join(mine, theirs))
                    .collect(),
            });
        }
    }
    let returns = returned
        .filter(|values| values.len() == 1)
        .and_then(|values| values.into_iter().next())
        .map(Value::stored);
    (interpreter.records, returns, interpreter.too_deep)
}

/// The room a lent buffer of `bytes` takes, given the call's `arguments`.
fn room_of(bytes: Bytes, arguments: &[Value]) -> Room {
    match bytes {
        Bytes::Fixed(count) => Room::Bytes(u64::from(count)),
        Bytes::Argument(argument) => arguments
            .get(argument)
            .map_or(Room::Unsettled, |length| Room::Length(length.clone())),
        Bytes::Table { count, entry } => {
            let range = arguments
                .get(count)
                .map_or_else(Range::full, Value::range)
                .width(32);
            if range.bounded() {
                Room::Bytes(range.high.saturating_mul(u64::from(entry)))
            } else {
                Room::Unsettled
            }
        }
        Bytes::Sized => Room::Unsettled,
    }
}

/// Every access of `function`, and every buffer its calls borrow, from what it was followed to
/// take.
fn accesses(
    module: &Module<'_>,
    function: &Function<'_>,
    records: &[Option<Record>],
    summaries: &BTreeMap<u32, Summary>,
) -> Vec<Access> {
    let mut found = Vec::new();
    for (at, (op, record)) in function.body.iter().zip(records).enumerate() {
        let Some(record) = record else {
            continue;
        };
        let operand = |position: usize| record.operands.get(position).cloned();
        let mut push =
            |what: String, address: Option<Value>, room: Room, written: bool, argument| {
                if let Some(address) = address {
                    found.push(Access {
                        at,
                        line: op.line,
                        what,
                        address,
                        room,
                        written,
                        argument,
                    });
                }
            };
        if let Some((memarg, width, store, _, name)) = memory_op(&op.operator) {
            let address = operand(0).map(|address| shifted(&address, memarg.offset));
            push(
                format!("`{name}`"),
                address,
                Room::Bytes(width),
                store,
                None,
            );
            continue;
        }
        match op.operator {
            Operator::MemoryCopy { .. } => {
                let length = operand(2).map_or(Room::Unsettled, Room::Length);
                push(
                    "`memory.copy`'s destination".to_owned(),
                    operand(0),
                    length.clone(),
                    true,
                    None,
                );
                push(
                    "`memory.copy`'s source".to_owned(),
                    operand(1),
                    length,
                    false,
                    None,
                );
            }
            Operator::MemoryFill { .. } | Operator::MemoryInit { .. } => {
                let name = if matches!(op.operator, Operator::MemoryFill { .. }) {
                    "`memory.fill`"
                } else {
                    "`memory.init`"
                };
                let length = operand(2).map_or(Room::Unsettled, Room::Length);
                push(
                    format!("{name}'s destination"),
                    operand(0),
                    length,
                    true,
                    None,
                );
            }
            Operator::Call { function_index } => {
                let Some(callee) = module.function(function_index) else {
                    continue;
                };
                let shown = callee.shown();
                for lend in lends_of(callee) {
                    let room = room_of(lend.bytes, &record.operands);
                    push(
                        format!(
                            "the buffer {shown} borrows in argument {}",
                            lend.argument + 1
                        ),
                        operand(lend.argument),
                        room,
                        lend.written,
                        Some(lend.argument),
                    );
                }
                if let Some(entries) = &record.iovecs {
                    for (entry, (buffer, length)) in entries.iter().enumerate() {
                        push(
                            format!("the buffer {shown}'s iovec {} points to", entry + 1),
                            Some(buffer.clone()),
                            Room::Length(length.clone()),
                            writes_iovec_buffers(callee),
                            None,
                        );
                    }
                }
                if let Some(summary) = summaries.get(&function_index) {
                    for (param, borrow) in &summary.borrows {
                        let room = match borrow.reach {
                            Reach::Bytes(bytes) => Room::Bytes(bytes),
                            Reach::Unsettled => Room::Unsettled,
                        };
                        push(
                            format!("what {shown} reaches through argument {}", param + 1),
                            operand(to_usize(*param)),
                            room,
                            borrow.written,
                            Some(to_usize(*param)),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    found
}

/// What `function` does with what it's lent, as its accesses and calls show it.
fn summarize(function: &Function<'_>, accesses: &[Access], returns: Option<Value>) -> Summary {
    let mut summary = Summary {
        returns,
        ..Summary::default()
    };
    for access in accesses {
        let room = match &access.room {
            Room::Bytes(bytes) => Some(*bytes),
            Room::Length(length) => {
                let range = length.range().width(32);
                range.bounded().then_some(range.high)
            }
            Room::Unsettled => None,
        };
        // Each address the access may go through, of several.
        for address in parts(&access.address) {
            let Value::Param { param, offset } = address else {
                continue;
            };
            if function.params.get(to_usize(*param)) != Some(&ValType::I32) {
                continue;
            }
            let reach = match (offset.high, room) {
                (Some(high), Some(room)) => u64::try_from(high)
                    .ok()
                    .and_then(|high| high.checked_add(room))
                    .map_or(Reach::Unsettled, Reach::Bytes),
                _ => Reach::Unsettled,
            };
            let borrow = summary.borrows.entry(*param).or_insert(Borrow {
                written: false,
                reach: Reach::Bytes(0),
            });
            borrow.written |= access.written;
            borrow.reach = borrow.reach.farther(reach);
        }
    }
    // A call that writes through one lent address while it reads another needs them apart.
    for written in accesses.iter().filter(|access| access.written) {
        for other in accesses {
            let both_lent = written.argument.is_some() && other.argument.is_some();
            if written.at != other.at || !both_lent || written.argument == other.argument {
                continue;
            }
            for mine in parts(&written.address).iter().filter_map(lent) {
                for theirs in parts(&other.address).iter().filter_map(lent) {
                    if mine != theirs {
                        summary.apart.insert((mine, theirs));
                    }
                }
            }
        }
    }
    summary
}

/// The parameter an address is, when it's one a caller lent.
fn lent(value: &Value) -> Option<u32> {
    match value {
        Value::Param { param, .. } => Some(*param),
        _ => None,
    }
}

/// Follows every function of main.wat, callees first.
#[must_use]
pub fn analyze(module: &Module<'_>, flows: &Flows) -> Analysis {
    let mut analysis = Analysis::default();
    for &index in &flows.order {
        let (Some(function), Some(shape)) = (module.function(index), flows.shapes.get(&index))
        else {
            continue;
        };
        let (records, returns, too_deep) = follow(module, function, shape, &analysis.summaries);
        let accesses = accesses(module, function, &records, &analysis.summaries);
        let summary = summarize(function, &accesses, returns);
        analysis.summaries.insert(index, summary);
        analysis.followed.insert(
            index,
            Followed {
                records,
                accesses,
                too_deep,
            },
        );
    }
    analysis
}

/// How an access stands against its region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// M1: it goes through a plain number, not an address.
    Number(String),
    /// M6: it writes to a read-only region.
    ReadOnly(String),
    /// M2: at a constant offset, it leaves its region.
    Outside(String),
    /// Rule 9: its computed index isn't checked.
    Unchecked(String),
    /// Rule 9: its index's check lets it leave its region.
    Loose(String),
    /// The proof's to hold.
    Unsettled(String),
}

/// How `access`, an access of `function`, stands against its region.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn judge(module: &Module<'_>, function: &Function<'_>, access: &Access) -> Vec<Verdict> {
    let what = &access.what;
    let in_unsafe = module.in_unsafe_block(access.line);
    let (offset, owner, name) = match &access.address {
        Value::Address { region, offset } => {
            let Some(region) = module.regions.get(*region) else {
                return Vec::new();
            };
            (*offset, Some(region), format!("${}", region.name))
        }
        Value::Param { param, offset } => (*offset, None, function.local_shown(*param)),
        // One of several addresses: judged as each of them, against each one's region.
        Value::Several(items) => {
            return items
                .iter()
                .flat_map(|item| {
                    let one = Access {
                        address: item.clone(),
                        ..access.clone()
                    };
                    judge(module, function, &one)
                })
                .collect();
        }
        Value::Loaded | Value::Mixed { number: false } => {
            return vec![Verdict::Unsettled(format!(
                "{what} goes through an address loaded from memory, or another the rules don't \
                 follow"
            ))];
        }
        Value::Mixed { number: true } => {
            return vec![Verdict::Number(format!(
                "{what} goes through an address that on some path is a plain number"
            ))];
        }
        _ => {
            return vec![Verdict::Number(format!(
                "{what} goes through an address made from a plain number, not from a region's \
                 global"
            ))];
        }
    };
    let mut verdicts = Vec::new();
    if let Some(region) = owner
        && access.written
        && region.read_only
    {
        verdicts.push(Verdict::ReadOnly(format!(
            "{what} writes to {name}, a read-only region"
        )));
    }
    let (room_high, room_constant, room_checked, room_linear) = match &access.room {
        Room::Bytes(bytes) => (Some(*bytes), true, true, None),
        Room::Length(value) => {
            let range = value.range().width(32);
            (
                range.bounded().then_some(range.high),
                range.constant().is_some(),
                range.sound(),
                range.linear.filter(|_| range.bounded()),
            )
        }
        Room::Unsettled => {
            verdicts.push(Verdict::Unsettled(format!(
                "{what} touches as many bytes of {name} as the call is given, which the rules \
                 don't follow"
            )));
            return verdicts;
        }
    };
    let computed = offset.constant().is_none() || !room_constant;
    let index = offset.index.map_or_else(
        || "a computed index".to_owned(),
        |local| function.local_shown(local),
    );
    let exact_end = offset
        .linear
        .zip(room_linear)
        .and_then(|(start, length)| start.sum(length));
    let end = exact_end.or_else(|| {
        offset
            .high
            .zip(room_high)
            .and_then(|(high, room)| high.checked_add(signed(room)?))
    });
    let leaves = |problem: String| {
        if !computed {
            Verdict::Outside(problem)
        } else if in_unsafe {
            Verdict::Unsettled(format!("{problem}, in an unsafe block"))
        } else {
            Verdict::Loose(problem)
        }
    };
    match (offset.low, end) {
        (Some(low), _) if low < 0 => {
            let before = match owner {
                Some(_) => format!("the start of {name}"),
                None => format!("the address {name} lends it"),
            };
            verdicts.push(leaves(format!(
                "{what} reaches {} byte{} before {before}",
                low.unsigned_abs(),
                if low == -1 { "" } else { "s" }
            )));
        }
        (Some(_), Some(end)) => {
            if let Some(region) = owner
                && u64::try_from(end).is_ok_and(|end| end > region.size)
            {
                verdicts.push(leaves(format!(
                    "{what} reaches byte {} of {name}, which has {} bytes",
                    end - 1,
                    region.size
                )));
            }
        }
        _ => {
            if in_unsafe {
                verdicts.push(Verdict::Unsettled(format!(
                    "{what} in an unsafe block, with an index the rules don't bound"
                )));
            } else if offset.checked && room_checked {
                verdicts.push(Verdict::Unsettled(format!(
                    "{what} is checked against a limit the rules don't follow"
                )));
            } else {
                verdicts.push(Verdict::Unchecked(format!(
                    "{what} indexes {name} with {index}, which isn't bounds-checked first"
                )));
            }
        }
    }
    verdicts
}

/// The byte range of `access` in its region, when the rules bound it: from its first byte to
/// just past its last.
fn span(access: &Access) -> Option<(i64, i64)> {
    let offset = match &access.address {
        Value::Address { offset, .. } | Value::Param { offset, .. } => *offset,
        _ => return None,
    };
    let room = match &access.room {
        Room::Bytes(bytes) => *bytes,
        Room::Length(value) => {
            let range = value.range().width(32);
            if !range.bounded() {
                return None;
            }
            range.high
        }
        Room::Unsettled => return None,
    };
    Some((offset.low?, offset.high?.checked_add(signed(room)?)?))
}

/// Whether `a` and `b` may name the same region or the same lent address.
fn same_owner(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Several(items), other) | (other, Value::Several(items)) => {
            items.iter().any(|item| same_owner(item, other))
        }
        (Value::Address { region, .. }, Value::Address { region: other, .. }) => region == other,
        (Value::Param { param, .. }, Value::Param { param: other, .. }) => param == other,
        _ => false,
    }
}

/// Checks M1, M2, M4, M6, the borrowed buffers and M8, and records what's left to the proof.
pub fn check(
    module: &Module<'_>,
    flows: &Flows,
    analysis: &Analysis,
    violations: &mut Vec<Violation>,
    facts: &mut Facts,
) {
    let mut violation = |rule: Rule, line: Option<usize>, problem: String, fix: &str| {
        violations.push(Violation {
            rule,
            line,
            problem,
            fix: fix.to_owned(),
        });
    };
    for (index, followed) in &analysis.followed {
        let Some(function) = module.function(*index) else {
            continue;
        };
        if followed.too_deep {
            violation(
                Rule::Functions,
                function.line,
                format!(
                    "{}'s blocks nest more than {MAX_NESTING} deep, past what the checker \
                     follows",
                    function.shown()
                ),
                "split it into smaller functions",
            );
        }
        for access in &followed.accesses {
            for verdict in judge(module, function, access) {
                match verdict {
                    Verdict::Number(problem) => violation(
                        Rule::Owners,
                        access.line,
                        problem,
                        "make every address from a region's global plus offsets, `(i32.add \
                         (global.get $buf) (local.get $i))`; lower addresses are lengua's",
                    ),
                    Verdict::ReadOnly(problem) => violation(
                        Rule::Immutability,
                        access.line,
                        problem,
                        "keep what the program changes in a writable region, `$x.size`; a \
                         read-only one, `$x.rosize`, holds only its data",
                    ),
                    Verdict::Outside(problem) => violation(
                        Rule::Bounds,
                        access.line,
                        problem,
                        "keep every byte of the access inside its region, its width included",
                    ),
                    Verdict::Unsettled(what) => facts.obligations.push(Obligation {
                        line: access.line,
                        what,
                    }),
                    // Rule 9's, reported with the rules.
                    Verdict::Unchecked(_) | Verdict::Loose(_) => {}
                }
            }
        }
        exclusive_writes(module, function, followed, analysis, &mut violation);
        iovecs_followed(module, function, followed, facts);
    }
    stack(module, flows, violations, facts);
}

/// M4: at each call, an argument the callee writes through names no part of a region another
/// argument names.
fn exclusive_writes(
    module: &Module<'_>,
    function: &Function<'_>,
    followed: &Followed,
    analysis: &Analysis,
    violation: &mut impl FnMut(Rule, Option<usize>, String, &str),
) {
    let fix = "lend a region a callee writes through once; pass separate parts as separate \
               regions";
    for (at, op) in function.body.iter().enumerate() {
        let Operator::Call { function_index } = op.operator else {
            continue;
        };
        let borrowed: Vec<&Access> = followed
            .accesses
            .iter()
            .filter(|access| access.at == at && access.argument.is_some())
            .collect();
        let callee = module.function(function_index);
        let shown = callee.map_or_else(String::new, Function::shown);
        let mut reported = BTreeSet::new();
        for written in borrowed.iter().filter(|access| access.written) {
            for other in &borrowed {
                if written.argument == other.argument
                    || !same_owner(&written.address, &other.address)
                {
                    continue;
                }
                let overlap = match (span(written), span(other)) {
                    (Some((start, end)), Some((other_start, other_end))) => {
                        start < other_end && other_start < end
                    }
                    _ => true,
                };
                let pair = (written.argument, other.argument);
                if overlap && reported.insert(pair) {
                    let (Some(first), Some(second)) = pair else {
                        continue;
                    };
                    violation(
                        Rule::ExclusiveWrites,
                        op.line,
                        format!(
                            "{shown} writes through argument {} while argument {} may name the \
                             same bytes",
                            first + 1,
                            second + 1
                        ),
                        fix,
                    );
                }
            }
        }
        // The pairs the callee needs apart, as its own callees need them.
        let Some(summary) = analysis.summaries.get(&function_index) else {
            continue;
        };
        let Some(record) = followed.records.get(at).and_then(Option::as_ref) else {
            continue;
        };
        for (first, second) in &summary.apart {
            let (Some(a), Some(b)) = (
                record.operands.get(to_usize(*first)),
                record.operands.get(to_usize(*second)),
            ) else {
                continue;
            };
            if same_owner(a, b)
                && reported.insert((Some(to_usize(*first)), Some(to_usize(*second))))
            {
                violation(
                    Rule::ExclusiveWrites,
                    op.line,
                    format!(
                        "{shown} writes through argument {} while argument {} names the same \
                         region",
                        first + 1,
                        second + 1
                    ),
                    fix,
                );
            }
        }
    }
}

/// The host calls whose iovecs' buffers the rules can't follow, for the proof.
fn iovecs_followed(
    module: &Module<'_>,
    function: &Function<'_>,
    followed: &Followed,
    facts: &mut Facts,
) {
    for (op, record) in function.body.iter().zip(&followed.records) {
        let (Operator::Call { function_index }, Some(record)) = (&op.operator, record) else {
            continue;
        };
        let Some(callee) = module.function(*function_index) else {
            continue;
        };
        let borrows_iovecs = lends_of(callee).iter().any(
            |lend| matches!(lend.bytes, Bytes::Table { entry, .. } if entry == wasi::size::IOVEC),
        );
        if borrows_iovecs && record.iovecs.is_none() {
            facts.obligations.push(Obligation {
                line: op.line,
                what: format!(
                    "the buffers {}'s iovecs point to lie inside regions with room: fill the \
                     iovecs at constant offsets in this function for the rules to follow them",
                    callee.shown()
                ),
            });
        }
    }
}

/// M8: the deepest call path from `_start`, its locals and operands within the engine's stack.
fn stack(module: &Module<'_>, flows: &Flows, violations: &mut Vec<Violation>, facts: &mut Facts) {
    let frame = |function: &Function<'_>| -> u64 {
        let shape = flows.shapes.get(&function.index);
        let slots = function.local_count() + shape.map_or(0, |shape| shape.max_stack);
        FRAME_BYTES
            .saturating_add(SLOT_BYTES.saturating_mul(u64::try_from(slots).unwrap_or(u64::MAX)))
    };
    // The deepest each function reaches, and the callee on its way, its callees settled first:
    // lengua's library's (which call only each other and the host) by repeating, then main.wat's
    // own in their order.
    let mut deepest: BTreeMap<u32, (u64, Option<u32>)> = BTreeMap::new();
    let settle = |function: &Function<'_>, deepest: &mut BTreeMap<u32, (u64, Option<u32>)>| {
        let own = frame(function);
        let mut best = (own, None);
        if let Some(shape) = flows.shapes.get(&function.index) {
            for (_, callee) in &shape.calls {
                if let Some((bytes, _)) = deepest.get(callee) {
                    let total = own.saturating_add(*bytes);
                    if total > best.0 {
                        best = (total, Some(*callee));
                    }
                }
            }
        }
        deepest.insert(function.index, best);
    };
    let library: Vec<&Function<'_>> = module
        .functions
        .iter()
        .filter(|function| function.kind == Kind::Library)
        .collect();
    for _ in 0..library.len().max(1) {
        for &function in &library {
            settle(function, &mut deepest);
        }
    }
    for index in &flows.order {
        if let Some(function) = module.function(*index) {
            settle(function, &mut deepest);
        }
    }
    let Some(entry) = module.entry() else {
        return;
    };
    let Some(&(bytes, _)) = deepest.get(&entry) else {
        return;
    };
    let mut path = Vec::new();
    let mut current = Some(entry);
    let mut seen = BTreeSet::new();
    while let Some(index) = current {
        if !seen.insert(index) {
            break;
        }
        if let Some(function) = module.function(index) {
            path.push(function.name.clone().unwrap_or_else(|| function.shown()));
        }
        current = deepest.get(&index).and_then(|(_, next)| *next);
    }
    facts.max_stack_bytes = bytes;
    facts.deepest_path.clone_from(&path);
    if bytes > MAX_STACK_BYTES {
        violations.push(Violation {
            rule: Rule::StackSize,
            line: module.function(entry).and_then(|function| function.line),
            problem: format!(
                "the deepest call path, {}, takes {bytes} bytes of the engine's stack as lengua \
                 counts it, past the {MAX_STACK_BYTES} the dialect allows",
                path.join(" → ")
            ),
            fix: "keep large buffers in regions sized from the idea rather than in locals, and \
                  shorten the call path"
                .to_owned(),
        });
    }
}
