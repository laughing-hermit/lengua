//! Terms: bit-vector and true/false expressions over a program's inputs, shared and simplified
//! as they're built, so the same computation is always the same term. A solver then proves two
//! terms equal at once when they are the same, which keeps proofs fast.

use std::collections::{BTreeMap, HashMap};

/// A term in a [`Pool`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Term(u32);

impl Term {
    /// Never a term of any pool: marks a witness whose unset variables take their low ends.
    pub const LOW: Self = Self(u32::MAX);
    /// Never a term of any pool: marks a witness whose unset variables take their high ends.
    pub const HIGH: Self = Self(u32::MAX - 1);
}

impl Term {
    /// The term's number in its pool, which names it for the solver.
    #[must_use]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// The term with this number, as the solver names it back.
pub(super) fn term_at(number: u32) -> Term {
    Term(number)
}

/// The greatest common divisor of two numbers, `a` when `b` is 0.
fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// What a term is: true/false, or a bit vector of a width from 1 to 128 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sort {
    /// True or false.
    Bool,
    /// A bit vector of this many bits.
    Bv(u32),
}

/// The operations terms are made of, as SMT-LIB names them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Node {
    /// A bit-vector constant.
    Const {
        /// Its width in bits.
        width: u32,
        /// Its value, below 2^width.
        value: u128,
    },
    /// A bit-vector variable.
    Var {
        /// Its width in bits.
        width: u32,
        /// Its name, unique in the pool.
        name: String,
    },
    /// True or false.
    Bool(bool),
    /// A true/false variable, such as a flag the CPU leaves undefined.
    BoolVar(String),
    /// `bvadd`
    Add(Term, Term),
    /// `bvmul`
    Mul(Term, Term),
    /// `bvudiv`
    Udiv(Term, Term),
    /// `bvurem`
    Urem(Term, Term),
    /// `bvsdiv`
    Sdiv(Term, Term),
    /// `bvsrem`
    Srem(Term, Term),
    /// `bvand`
    And(Term, Term),
    /// `bvor`
    Or(Term, Term),
    /// `bvxor`
    Xor(Term, Term),
    /// `bvnot`
    Not(Term),
    /// `bvneg`
    Neg(Term),
    /// `bvshl`
    Shl(Term, Term),
    /// `bvlshr`
    Lshr(Term, Term),
    /// `bvashr`
    Ashr(Term, Term),
    /// `(_ extract high low)`
    Extract(Term, u32, u32),
    /// `(_ zero_extend by)`
    ZeroExtend(Term, u32),
    /// `(_ sign_extend by)`
    SignExtend(Term, u32),
    /// `concat`: the high part, then the low part.
    Concat(Term, Term),
    /// `ite`: a condition, then the value if true and if false.
    Ite(Term, Term, Term),
    /// `=`
    Eq(Term, Term),
    /// `bvult`
    Ult(Term, Term),
    /// `bvslt`
    Slt(Term, Term),
    /// `and`
    BoolAnd(Term, Term),
    /// `or`
    BoolOr(Term, Term),
    /// `not`
    BoolNot(Term),
    /// Whether a byte variable's value is in a set: a table of 256 bits, by number. Every
    /// condition on one byte alone becomes one, so two ways of writing the same test of a byte,
    /// a program's and a formula's, are one term.
    Lookup(Term, u32),
    /// How many of a list of conditions hold, by the list's number, as a number this many bits
    /// wide: a sum of ones and zeros in any order and grouping, so a popcount of a mask and a
    /// formula's count of the same tests are one term.
    Count(u32, u32),
    /// One byte through a reflected CRC-32: a 32-bit state, then a byte, by the polynomial, as
    /// the `crc32` instruction takes each byte for CRC-32C and zlib for CRC-32. Bytes taken 8 at
    /// a time are 8 steps, so a program's checksum and a formula's of the same bytes are one
    /// term.
    Crc(Term, Term, u32),
    /// An IEEE-754 operation on two floats' bit patterns, 32 bits wide for singles and 64 for
    /// doubles, as SSE computes it (see [`float`]).
    Float(FloatOp, Term, Term),
    /// A float's square root, as `sqrtsd` takes it.
    FloatSqrt(Term),
    /// `a × b + c`, rounded once, as `vfmadd` takes it.
    FloatFma(Term, Term, Term),
    /// A signed whole number as the nearest float of this many bits, as `cvtsi2sd` takes it.
    FloatFromInt(Term, u32),
    /// A float as a signed whole number of this many bits, to the nearest with ties to even, as
    /// `cvtsd2si` gives it, or truncated, as `cvttsd2si` does, when the flag says so; only the top
    /// bit set for a NaN or a number out of range.
    FloatToInt(Term, u32, bool),
    /// A float rounded to a whole float, as `roundsd` rounds it.
    FloatRound(Term, Rounding),
    /// A float as the nearest float of this many bits, as `cvtsd2ss` and `cvtss2sd` take it.
    FloatConvert(Term, u32),
    /// What `ucomisd` tells of two floats: true or false.
    FloatTest(FloatTest, Term, Term),
}

/// The reflected CRC-32C (Castagnoli) polynomial, as the `crc32` instruction uses it.
pub const CASTAGNOLI: u32 = 0x82F6_3B78;

/// The reflected CRC-32 polynomial, as zlib, gzip and PNG use it.
pub const ZLIB: u32 = 0xEDB8_8320;

pub use float::{FloatOp, FloatTest, Rounding};

impl Node {
    /// Whether the node is an operation on floats, which only solvers that read floats can
    /// compute.
    #[must_use]
    pub const fn is_float_operation(&self) -> bool {
        matches!(
            self,
            Self::Float(..)
                | Self::FloatSqrt(_)
                | Self::FloatFma(..)
                | Self::FloatFromInt(..)
                | Self::FloatToInt(..)
                | Self::FloatRound(..)
                | Self::FloatConvert(..)
                | Self::FloatTest(..)
        )
    }

    /// What two float operations must share to be the same function of their operands: the
    /// kind, its parameter, and the operands' width.
    #[must_use]
    pub fn float_function(&self, operand_width: u32) -> Option<(u8, u32, u32)> {
        let (kind, parameter) = match self {
            Self::Float(op, ..) => (0, *op as u32),
            Self::FloatSqrt(_) => (1, 0),
            Self::FloatFma(..) => (2, 0),
            Self::FloatFromInt(_, width) => (3, *width),
            Self::FloatToInt(_, to, truncate) => (4, to * 2 + u32::from(*truncate)),
            Self::FloatRound(_, rounding) => (5, *rounding as u32),
            Self::FloatConvert(_, to) => (6, *to),
            Self::FloatTest(test, ..) => (7, *test as u32),
            _ => return None,
        };
        Some((kind, parameter, operand_width))
    }
}

/// How the solvers read a float result (see [`Pool::float_definition`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloatDefinition {
    /// Its value as an SMT-LIB float.
    pub value: String,
    /// Its bits when the value is a NaN.
    pub nan: String,
    /// Its width in bits: 32 or 64.
    pub width: u32,
}

/// The SMT-LIB conversion to a float of `width` bits.
#[must_use]
pub fn fp_sort_cast(width: u32) -> &'static str {
    if width == 32 {
        "(_ to_fp 8 24)"
    } else {
        "(_ to_fp 11 53)"
    }
}

/// The bits `name` names, read as a float of `width` bits.
#[must_use]
pub fn fp_of(name: &str, width: u32) -> String {
    format!("({} {name})", fp_sort_cast(width))
}

/// `value` as an SMT-LIB float of `width` bits, exactly.
fn fp_literal(value: f64, width: u32) -> String {
    #[allow(clippy::cast_possible_truncation)] // Powers of two, exact as singles.
    let bits = if width == 32 {
        u128::from((value as f32).to_bits())
    } else {
        u128::from(value.to_bits())
    };
    fp_of(&bits_literal(bits, width), width)
}

/// `bits` as an SMT-LIB bit-vector literal of `width` bits.
fn bits_literal(bits: u128, width: u32) -> String {
    format!("#x{bits:0digits$x}", digits = (width / 4) as usize)
}

/// One byte through a reflected CRC-32 by `poly`, as the `crc32` instruction steps for
/// CRC-32C: no inversion before or after.
#[must_use]
pub const fn crc_byte(poly: u32, state: u32, byte: u8) -> u32 {
    let mut state = state ^ byte as u32;
    let mut bit = 0;
    while bit < 8 {
        state = (state >> 1) ^ (poly & (state & 1).wrapping_neg());
        bit += 1;
    }
    state
}

/// A set of byte values: bit `v` of word `v / 64` is set for each value `v` in it.
pub type Table = [u64; 4];

/// The one byte variable a term is computed from, when there is exactly one: its value is then
/// a function of at most 256 values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sole {
    /// No variable: a constant.
    Constant,
    /// One 8-bit variable.
    Byte(Term),
    /// Several variables, or one that isn't a byte.
    Many,
}

/// A sum as each of its parts times a constant multiple, plus a constant: the one form of a sum
/// however a program or a formula grouped, ordered or scaled it, so the same arithmetic is the
/// same term.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Linear {
    /// Each part, a term that isn't itself a sum, with its multiple: never 0, below 2^width.
    parts: BTreeMap<Term, u128>,
    /// The constant, below 2^width.
    constant: u128,
}

impl Linear {
    /// Adds `other` times `scale`, all `width` bits wide.
    fn add(&mut self, other: &Self, scale: u128, width: u32) {
        for (part, times) in &other.parts {
            self.add_part(*part, times.wrapping_mul(scale), width);
        }
        self.constant = self
            .constant
            .wrapping_add(other.constant.wrapping_mul(scale))
            & mask(width);
    }

    /// Adds `part` times `times`.
    fn add_part(&mut self, part: Term, times: u128, width: u32) {
        let entry = self.parts.entry(part).or_insert(0);
        *entry = entry.wrapping_add(times) & mask(width);
        if *entry == 0 {
            self.parts.remove(&part);
        }
    }
}

/// Marks a shape's number where a term would be, in a node that stands for a shape.
const SHAPE: u32 = 1 << 31;

/// The conditions one count counts, sorted: those of the count it extends, if any, then its own.
/// A running total a loop adds to keeps only what each step adds, not a copy of all it counted
/// before, so the counts over a long text take memory in proportion to the text, not its square.
#[derive(Debug)]
struct Counted {
    /// The count this one extends, by number.
    base: Option<u32>,
    /// The conditions this count adds to its base's, sorted.
    own: Vec<Term>,
    /// How many conditions it counts in all.
    len: usize,
    /// How many of them are the constant true.
    sure: usize,
    /// The largest condition.
    last: Term,
    /// Whether the base's conditions, then the count's own, are in order.
    sorted: bool,
    /// The conditions' hash: the sum of each one's [`mix`], however they're held.
    hash: u64,
    /// The count made before this one with the same hash.
    same_hash: Option<u32>,
}

/// A condition's share of its count's hash: splitmix64's finalizer of its number, so the sums of
/// two different sets of conditions hardly ever agree.
fn mix(term: Term) -> u64 {
    let mut z = u64::from(term.0).wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Every term made so far, each made once, with the unsigned range of values each can take.
#[derive(Debug, Default)]
pub struct Pool {
    nodes: Vec<Node>,
    sorts: Vec<Sort>,
    ranges: Vec<(u128, u128)>,
    signed_ranges: Vec<(i128, i128)>,
    index: HashMap<Node, Term>,
    /// Variables known to stay in a range, by name: digits are '0' to '9'.
    declared: HashMap<String, (u128, u128)>,
    /// Each term's one byte variable, if it has exactly one.
    soles: Vec<Sole>,
    /// Whether each term has a float operation in it, so a question about it goes to solvers
    /// that read floats.
    floats: Vec<bool>,
    /// The smallest and largest value of each float result, when its operands' bounds rule out
    /// a NaN: from the ranges of the whole numbers it comes from, operation by operation.
    float_bounds: Vec<Option<(f64, f64)>>,
    /// Each one-byte term's shape: the term with its byte's identity left out, so tests of
    /// different bytes that compute the same thing share one. `u32::MAX` for other terms.
    shapes: Vec<u32>,
    shape_index: HashMap<Node, u32>,
    /// The table each one-byte condition's shape computes, worked out once per shape.
    shape_tables: HashMap<u32, Table>,
    /// The sets lookups test, by number.
    tables: Vec<Table>,
    table_index: HashMap<Table, u32>,
    /// The conditions each count counts, by number.
    counts: Vec<Counted>,
    /// The last count made with each hash of its conditions.
    count_index: HashMap<u64, u32>,
    /// Each conjunction a count has counted, in its one form ([`Pool::canonical_part`]).
    canonical_parts: HashMap<Term, Term>,
    /// The values a byte variable is held to by the conditions it's drawn with, such as a
    /// text's characters, for drawing inputs that meet them.
    allowed: HashMap<Term, Vec<u8>>,
    /// The terms bytes were split from, by those bytes lowest first, where a byte simplified so
    /// that joining them again wouldn't give the term back: the low byte of a sum is the sum of
    /// its parts' low bytes.
    split_from: HashMap<Vec<Term>, Term>,
}

/// The smallest all-ones number at least `value`: the range an or or xor stays in.
fn smear(value: u128) -> u128 {
    if value == 0 {
        0
    } else {
        u128::MAX >> value.leading_zeros()
    }
}

fn mask(width: u32) -> u128 {
    if width >= 128 {
        u128::MAX
    } else {
        (1_u128 << width) - 1
    }
}

/// The most terms [`Pool::on_length`] walks in a condition's other conjuncts for comparisons of
/// the length it settles.
const MAX_WALKED: usize = 4096;

/// The most conjuncts a count's part is written in one form for ([`Pool::canonical_part`]).
const MAX_CONJUNCTS: usize = 32;

/// The most intervals a set of a variable's values [`Pool::holds_where`] works out may have.
const MAX_INTERVALS: usize = 1 << 16;

/// A question [`Pool::holds_where`] answers of a part of a condition: the values of a variable
/// where a condition holds, or where a value is below a constant, unsigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Where {
    Holds(Term),
    Below(Term, u128),
}

/// How the sets of a question's parts make its own.
#[derive(Debug, Clone, Copy)]
enum Joined {
    /// The one part's set.
    Same,
    /// The range's values outside the one part's set.
    Not,
    /// The values in both parts' sets.
    And,
    /// The values in either part's set.
    Or,
    /// The values in the first part's set and not the second's.
    Except,
    /// The second part's values where the first part holds, and the third's where it doesn't.
    Choice,
}

/// What a question comes to: a set at once, nothing that can be read, or its parts' sets joined.
enum Reading {
    Set(Vec<(u128, u128)>),
    Unread,
    Parts(Joined, Vec<Where>),
}

/// The values in both sets, each sorted, disjoint, inclusive intervals.
fn intersect(a: &[(u128, u128)], b: &[(u128, u128)]) -> Vec<(u128, u128)> {
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < a.len() && j < b.len() {
        let (low, high) = (a[i].0.max(b[j].0), a[i].1.min(b[j].1));
        if low <= high {
            out.push((low, high));
        }
        if a[i].1 < b[j].1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/// The values in either set, as sorted, disjoint intervals, neighbors joined.
fn union(a: &[(u128, u128)], b: &[(u128, u128)]) -> Vec<(u128, u128)> {
    let mut all: Vec<(u128, u128)> = a.iter().chain(b).copied().collect();
    all.sort_unstable();
    let mut out: Vec<(u128, u128)> = Vec::new();
    for (low, high) in all {
        match out.last_mut() {
            Some(last) if low <= last.1.saturating_add(1) => last.1 = last.1.max(high),
            _ => out.push((low, high)),
        }
    }
    out
}

/// The values of `range` not in `set`.
fn complement(set: &[(u128, u128)], (low, high): (u128, u128)) -> Vec<(u128, u128)> {
    let mut out = Vec::new();
    let mut next = Some(low);
    for (from, to) in set {
        if let Some(start) = next
            && *from > start
        {
            out.push((start, (*from - 1).min(high)));
        }
        next = to.checked_add(1);
    }
    if let Some(start) = next
        && start <= high
    {
        out.push((start, high));
    }
    out.retain(|(from, to)| from <= to && *from >= low);
    out
}

/// The values `x` of `range` where `(x + offset) mod 2^bits` is below `k`: once each 2^bits,
/// or, with 128 bits, below and above where adding `offset` wraps.
fn modular_below(
    (low, high): (u128, u128),
    offset: u128,
    bits: u32,
    k: u128,
) -> Option<Vec<(u128, u128)>> {
    if k == 0 || low > high {
        return Some(Vec::new());
    }
    if bits >= 128 {
        // x + offset below where it wraps, and x - (2^128 - offset) past it.
        let wraps = offset.wrapping_neg();
        let mut out = Vec::new();
        if offset == 0 {
            if low < k {
                out.push((low, high.min(k - 1)));
            }
            return Some(out);
        }
        if k > offset && low < wraps {
            let last = high.min(wraps - 1).min(k - offset - 1);
            if low <= last {
                out.push((low, last));
            }
        }
        let first = low.max(wraps);
        if first <= high && wraps.checked_add(k - 1).is_some_and(|last| first <= last) {
            out.push((first, high.min(wraps + (k - 1))));
        }
        return Some(out);
    }
    let modulus = 1_u128 << bits;
    if k >= modulus {
        return Some(vec![(low, high)]);
    }
    let mut out = Vec::new();
    let mut x = low;
    loop {
        let residue = (x + offset) % modulus;
        if residue < k {
            let last = x
                .checked_add(k - 1 - residue)
                .map_or(high, |last| last.min(high));
            out.push((x, last));
            if out.len() > MAX_INTERVALS {
                return None;
            }
        }
        match x.checked_add(modulus - residue) {
            Some(next) if next <= high => x = next,
            _ => break,
        }
    }
    Some(out)
}

/// Whether every item of `small`, sorted, is in `large`, sorted, as often.
fn within_multiset(small: &[Term], large: &[Term]) -> bool {
    let mut large = large.iter().peekable();
    small.iter().all(|item| {
        while large.peek().is_some_and(|next| *next < item) {
            large.next();
        }
        large.next_if(|next| *next == item).is_some()
    })
}

/// A constant read as a signed number of its width.
fn signed(value: u128, width: u32) -> i128 {
    if width >= 128 {
        i128::from_ne_bytes(value.to_ne_bytes())
    } else if value >> (width - 1) & 1 == 1 {
        i128::from_ne_bytes((value | !mask(width)).to_ne_bytes())
    } else {
        i128::from_ne_bytes(value.to_ne_bytes())
    }
}

/// A signed number as a constant of `width` bits.
fn unsigned(value: i128, width: u32) -> u128 {
    u128::from_ne_bytes(value.to_ne_bytes()) & mask(width)
}

/// The terms a node is made of; none for a count, whose parts its pool keeps.
fn node_operands(node: &Node) -> Vec<Term> {
    match *node {
        Node::Const { .. }
        | Node::Var { .. }
        | Node::Bool(_)
        | Node::BoolVar(_)
        | Node::Count(..) => Vec::new(),
        Node::Not(a)
        | Node::Neg(a)
        | Node::Extract(a, _, _)
        | Node::ZeroExtend(a, _)
        | Node::SignExtend(a, _)
        | Node::BoolNot(a)
        | Node::Lookup(a, _)
        | Node::FloatSqrt(a)
        | Node::FloatFromInt(a, _)
        | Node::FloatToInt(a, _, _)
        | Node::FloatRound(a, _)
        | Node::FloatConvert(a, _) => vec![a],
        Node::Ite(c, a, b) | Node::FloatFma(c, a, b) => vec![c, a, b],
        Node::Add(a, b)
        | Node::Mul(a, b)
        | Node::Udiv(a, b)
        | Node::Urem(a, b)
        | Node::Sdiv(a, b)
        | Node::Srem(a, b)
        | Node::And(a, b)
        | Node::Or(a, b)
        | Node::Xor(a, b)
        | Node::Shl(a, b)
        | Node::Lshr(a, b)
        | Node::Ashr(a, b)
        | Node::Concat(a, b)
        | Node::Eq(a, b)
        | Node::Ult(a, b)
        | Node::Slt(a, b)
        | Node::BoolAnd(a, b)
        | Node::BoolOr(a, b)
        | Node::Crc(a, b, _)
        | Node::Float(_, a, b)
        | Node::FloatTest(_, a, b) => vec![a, b],
    }
}

/// The node's operands, to change in place.
fn node_operands_mut(node: &mut Node) -> Vec<&mut Term> {
    match node {
        Node::Const { .. }
        | Node::Var { .. }
        | Node::Bool(_)
        | Node::BoolVar(_)
        | Node::Count(..) => Vec::new(),
        Node::Not(a)
        | Node::Neg(a)
        | Node::Extract(a, _, _)
        | Node::ZeroExtend(a, _)
        | Node::SignExtend(a, _)
        | Node::BoolNot(a)
        | Node::Lookup(a, _)
        | Node::FloatSqrt(a)
        | Node::FloatFromInt(a, _)
        | Node::FloatToInt(a, _, _)
        | Node::FloatRound(a, _)
        | Node::FloatConvert(a, _) => vec![a],
        Node::Ite(c, a, b) | Node::FloatFma(c, a, b) => vec![c, a, b],
        Node::Add(a, b)
        | Node::Mul(a, b)
        | Node::Udiv(a, b)
        | Node::Urem(a, b)
        | Node::Sdiv(a, b)
        | Node::Srem(a, b)
        | Node::And(a, b)
        | Node::Or(a, b)
        | Node::Xor(a, b)
        | Node::Shl(a, b)
        | Node::Lshr(a, b)
        | Node::Ashr(a, b)
        | Node::Concat(a, b)
        | Node::Eq(a, b)
        | Node::Ult(a, b)
        | Node::Slt(a, b)
        | Node::BoolAnd(a, b)
        | Node::BoolOr(a, b)
        | Node::Crc(a, b, _)
        | Node::Float(_, a, b)
        | Node::FloatTest(_, a, b) => vec![a, b],
    }
}

impl Pool {
    /// The node of a term.
    #[must_use]
    pub fn node(&self, term: Term) -> &Node {
        &self.nodes[term.index()]
    }

    /// The sort of a term.
    #[must_use]
    pub fn sort(&self, term: Term) -> Sort {
        self.sorts[term.index()]
    }

    /// A bit vector's width; 0 for a true/false term.
    #[must_use]
    pub fn width(&self, term: Term) -> u32 {
        match self.sort(term) {
            Sort::Bv(width) => width,
            Sort::Bool => 0,
        }
    }

    /// Remembers that `parts`, lowest first, are `whole`'s bytes, so joining them gives `whole`
    /// back: a value stored to memory or a vector register and read back whole is the value.
    pub fn remember_split(&mut self, parts: &[Term], whole: Term) {
        self.split_from.insert(parts.to_vec(), whole);
    }

    /// The term `parts`, lowest first, were split from, where [`Self::remember_split`] holds it.
    #[must_use]
    pub fn split_of(&self, parts: &[Term]) -> Option<Term> {
        self.split_from.get(parts).copied()
    }

    /// How many terms exist.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether there are no terms yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The constant value of a bit-vector term, if it's a constant.
    #[must_use]
    pub fn constant(&self, term: Term) -> Option<u128> {
        match self.node(term) {
            Node::Const { value, .. } => Some(*value),
            _ => None,
        }
    }

    /// The value of a true/false term, if it's a constant.
    #[must_use]
    pub fn truth(&self, term: Term) -> Option<bool> {
        match self.node(term) {
            Node::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// The unsigned range of values `term` can take: true is 1, false 0.
    #[must_use]
    pub fn range(&self, term: Term) -> (u128, u128) {
        self.ranges[term.index()]
    }

    /// The range of values `term` can take read as signed numbers.
    #[must_use]
    pub fn signed_range(&self, term: Term) -> (i128, i128) {
        self.signed_ranges[term.index()]
    }

    /// The step between the values `term` can take, and their remainder by it: read as a sum,
    /// each part times a multiple of 2^s plus a constant, every value is the constant plus a
    /// multiple of 2^s, as an index scaled by 8 into a table only lands on its entries.
    #[must_use]
    pub fn stride(&self, term: Term) -> (u128, u128) {
        let width = self.width(term);
        let linear = self.linear_view(term);
        let shift = linear
            .parts
            .values()
            .map(|times| times.trailing_zeros())
            .min()
            .unwrap_or(width)
            .min(width)
            .min(63);
        let step = 1_u128 << shift;
        (step, linear.constant & (step - 1))
    }

    /// Whether `a` and `b`, read as sums, differ by multiples of 2^s and a constant that isn't
    /// one, so they're never equal: aligned addresses a few bytes apart never meet.
    #[must_use]
    pub fn apart(&self, a: Term, b: Term) -> bool {
        let width = self.width(a);
        let (x, y) = (self.linear_view(a), self.linear_view(b));
        let mut parts = x.parts;
        for (part, times) in y.parts {
            let entry = parts.entry(part).or_insert(0);
            *entry = entry.wrapping_sub(times) & mask(width);
        }
        parts.retain(|_, times| *times != 0);
        let constant = x.constant.wrapping_sub(y.constant) & mask(width);
        let Some(shift) = parts.values().map(|times| times.trailing_zeros()).min() else {
            return false;
        };
        shift > 0 && constant.trailing_zeros() < shift.min(width)
    }

    /// The signed range of a node's values, from its operands' signed ranges.
    fn signed_range_of(
        &self,
        node: &Node,
        width: u32,
        unsigned_range: (u128, u128),
    ) -> (i128, i128) {
        let minimum = if width >= 128 {
            i128::MIN
        } else {
            -(1_i128 << (width - 1))
        };
        let maximum = if width >= 128 {
            i128::MAX
        } else {
            (1_i128 << (width - 1)) - 1
        };
        let full = (minimum, maximum);
        let inside = |(low, high): (i128, i128)| low >= minimum && high <= maximum;
        let s = |term: &Term| self.signed_range(*term);
        // Values that are never negative read the same either way.
        let from_unsigned = if unsigned_range.1 <= mask(width.max(1) - 1) {
            Some((
                i128::try_from(unsigned_range.0).unwrap_or(i128::MAX),
                i128::try_from(unsigned_range.1).unwrap_or(i128::MAX),
            ))
        } else {
            None
        };
        let computed = match node {
            Node::Const { value, .. } => Some((signed(*value, width), signed(*value, width))),
            Node::Add(a, b) => {
                let ((al, ah), (bl, bh)) = (s(a), s(b));
                al.checked_add(bl)
                    .zip(ah.checked_add(bh))
                    .filter(|range| inside(*range))
            }
            Node::Mul(a, b) => {
                let ((al, ah), (bl, bh)) = (s(a), s(b));
                let corners = [
                    al.checked_mul(bl),
                    al.checked_mul(bh),
                    ah.checked_mul(bl),
                    ah.checked_mul(bh),
                ];
                if corners.iter().all(Option::is_some) {
                    let values: Vec<i128> = corners.iter().flatten().copied().collect();
                    let range = (
                        values.iter().copied().min().unwrap_or(minimum),
                        values.iter().copied().max().unwrap_or(maximum),
                    );
                    inside(range).then_some(range)
                } else {
                    None
                }
            }
            Node::Neg(a) => {
                let (low, high) = s(a);
                (low > minimum).then(|| (-high, -low))
            }
            Node::SignExtend(a, _) => Some(s(a)),
            Node::ZeroExtend(a, _) => {
                let (low, high) = self.range(*a);
                i128::try_from(low).ok().zip(i128::try_from(high).ok())
            }
            Node::FloatToInt(a, to, truncate) => self
                .float_bounds(*a)
                .and_then(|bounds| float::to_int_range(bounds, *to, *truncate)),
            Node::Ite(condition, a, b) => {
                let given = |value: Term, holds: bool| {
                    let (low, high) = self.signed_range(value);
                    if self.non_negative_when(*condition, holds, value) {
                        (low.max(0), high.max(0))
                    } else {
                        (low, high)
                    }
                };
                let (then, otherwise) = (given(*a, true), given(*b, false));
                Some((then.0.min(otherwise.0), then.1.max(otherwise.1)))
            }
            _ => None,
        };
        match (computed, from_unsigned) {
            (Some((low, high)), Some((from, to))) => (low.max(from), high.min(to)),
            (Some(range), None) | (None, Some(range)) => range,
            (None, None) => full,
        }
    }

    fn make(&mut self, node: Node, sort: Sort) -> Term {
        if let Some(term) = self.index.get(&node) {
            return *term;
        }
        let range = self.range_of(&node, sort);
        // A term that can only have one value is that value.
        if range.0 == range.1 && !matches!(node, Node::Const { .. } | Node::Bool(_)) {
            return match sort {
                Sort::Bv(width) => self.bv(width, range.0),
                Sort::Bool => self.boolean(range.0 == 1),
            };
        }
        let term = Term(u32::try_from(self.nodes.len()).unwrap_or(u32::MAX));
        let signed_range = match sort {
            Sort::Bv(width) => self.signed_range_of(&node, width, range),
            Sort::Bool => (0, 1),
        };
        let bounds = self.float_bounds_of(&node);
        let sole = self.sole_of(&node, term);
        let shape = match sole {
            Sole::Byte(_) => self.shape_of(&node, term),
            _ => u32::MAX,
        };
        self.nodes.push(node.clone());
        self.sorts.push(sort);
        self.ranges.push(range);
        self.signed_ranges.push(signed_range);
        self.soles.push(sole);
        self.shapes.push(shape);
        let float = node.is_float_operation()
            || node_operands(&node)
                .iter()
                .any(|operand| self.floats.get(operand.index()).copied().unwrap_or(false));
        self.floats.push(float);
        self.float_bounds.push(bounds);
        // A condition on one byte is the set of the byte's values where it holds.
        if let (Sort::Bool, Sole::Byte(byte)) = (sort, sole)
            && !matches!(node, Node::Lookup(..))
            && let Some(table) = self.table_of(term, byte, shape)
        {
            let lookup = self.lookup(byte, table);
            self.index.insert(node, lookup);
            return lookup;
        }
        self.index.insert(node, term);
        term
    }

    /// The one byte variable `node`, about to be `term`, is computed from, if any.
    fn sole_of(&self, node: &Node, term: Term) -> Sole {
        match node {
            Node::Const { .. } | Node::Bool(_) => Sole::Constant,
            Node::Var { width: 8, .. } => Sole::Byte(term),
            Node::Var { .. } | Node::BoolVar(_) | Node::Count(..) => Sole::Many,
            Node::Lookup(byte, _) => Sole::Byte(*byte),
            _ => {
                let mut sole = Sole::Constant;
                for operand in node_operands(node) {
                    sole = match (sole, self.soles[operand.index()]) {
                        (Sole::Many, _) | (_, Sole::Many) => return Sole::Many,
                        (Sole::Constant, other) | (other, Sole::Constant) => other,
                        (Sole::Byte(a), Sole::Byte(b)) if a == b => Sole::Byte(a),
                        _ => return Sole::Many,
                    };
                }
                sole
            }
        }
    }

    /// The shape of `node`, about to be `term`, which is computed from one byte: the node with
    /// each operand computed from the byte replaced by that operand's shape.
    fn shape_of(&mut self, node: &Node, term: Term) -> u32 {
        let shaped = match node {
            // A byte is known only by its range.
            Node::Var { width: 8, .. } => {
                let (low, high) = self.declared_range(node);
                Node::Const {
                    width: 0,
                    value: low << 8 | high,
                }
            }
            _ => {
                let mut shaped = node.clone();
                for operand in node_operands_mut(&mut shaped) {
                    let shape = self.shapes[operand.index()];
                    if shape != u32::MAX {
                        *operand = Term(SHAPE | shape);
                    }
                }
                shaped
            }
        };
        let next = u32::try_from(self.shape_index.len()).unwrap_or(u32::MAX);
        let _ = term;
        *self.shape_index.entry(shaped).or_insert(next)
    }

    /// The range a byte variable's node declares.
    fn declared_range(&self, node: &Node) -> (u128, u128) {
        match node {
            Node::Var { name, .. } => self.declared.get(name).copied().unwrap_or((0, 255)),
            _ => (0, 255),
        }
    }

    /// The values of `byte` where the condition `term` holds, worked out once per shape.
    fn table_of(&mut self, term: Term, byte: Term, shape: u32) -> Option<Table> {
        if let Some(table) = self.shape_tables.get(&shape) {
            return Some(*table);
        }
        let (low, high) = self.range(byte);
        let mut table = [0_u64; 4];
        for value in low..=high.min(255) {
            let holds =
                self.evaluate_with(term, &mut |_, variable| (variable == byte).then_some(value))?;
            if holds == 1 {
                table[(value / 64) as usize] |= 1 << (value % 64);
            }
        }
        self.shape_tables.insert(shape, table);
        Some(table)
    }

    /// The condition that `byte` is one of `table`'s values: true or false when its range
    /// settles that.
    fn lookup(&mut self, byte: Term, table: Table) -> Term {
        let (low, high) = self.range(byte);
        let mut inside = [0_u64; 4];
        for value in low..=high.min(255) {
            inside[(value / 64) as usize] |= 1 << (value % 64);
        }
        let held: Table = std::array::from_fn(|word| table[word] & inside[word]);
        if held == [0; 4] {
            return self.boolean(false);
        }
        if held == inside {
            return self.boolean(true);
        }
        let next = u32::try_from(self.tables.len()).unwrap_or(u32::MAX);
        let number = *self.table_index.entry(held).or_insert(next);
        if number == next {
            self.tables.push(held);
        }
        self.make(Node::Lookup(byte, number), Sort::Bool)
    }

    /// The condition that `byte`, an 8-bit term of one variable, is one of `values`.
    pub fn is_one_of(&mut self, byte: Term, values: &[u8]) -> Term {
        let mut table = [0_u64; 4];
        for value in values {
            table[usize::from(*value / 64)] |= 1 << (value % 64);
        }
        match self.node(byte) {
            Node::Var { width: 8, .. } => self.lookup(byte, table),
            _ => {
                let choices: Vec<Term> = values
                    .iter()
                    .map(|value| {
                        let value = self.bv(8, u128::from(*value));
                        self.eq(byte, value)
                    })
                    .collect();
                self.any(&choices)
            }
        }
    }

    /// Notes that conditions hold the byte variable `variable` to `values`, sorted.
    pub fn allow(&mut self, variable: Term, values: &[u8]) {
        let mut values = values.to_vec();
        values.sort_unstable();
        values.dedup();
        self.allowed.insert(variable, values);
    }

    /// The values conditions hold `variable` to, when they're noted.
    #[must_use]
    pub fn allowed(&self, variable: Term) -> Option<&[u8]> {
        self.allowed.get(&variable).map(Vec::as_slice)
    }

    /// The set a lookup tests.
    #[must_use]
    pub fn table(&self, number: u32) -> Table {
        self.tables[number as usize]
    }

    /// The conditions a count counts, sorted.
    #[must_use]
    pub fn count_parts(&self, number: u32) -> Vec<Term> {
        let mut chain = Vec::new();
        let mut at = Some(number);
        while let Some(here) = at {
            chain.push(here);
            at = self.counts[here as usize].base;
        }
        let counted = &self.counts[number as usize];
        let mut parts = Vec::with_capacity(counted.len);
        for here in chain.iter().rev() {
            parts.extend_from_slice(&self.counts[*here as usize].own);
        }
        if !counted.sorted {
            parts.sort_unstable();
        }
        parts
    }

    /// How many of `parts`, true/false terms, hold, as a `width`-bit number: a count, the same
    /// term whatever order the parts come in.
    pub fn count(&mut self, width: u32, parts: &[Term]) -> Term {
        self.count_with(width, None, parts)
    }

    /// [`Self::count`] of count `base`'s conditions, if any, and `more`: a count that extends
    /// `base` keeps only `more`.
    fn count_with(&mut self, width: u32, base: Option<u32>, more: &[Term]) -> Term {
        let mut own = Vec::with_capacity(more.len());
        for part in more {
            if self.truth(*part) != Some(false) {
                own.push(self.canonical_part(*part));
            }
        }
        let below = base.map_or(0, |base| self.counts[base as usize].len);
        if (below + own.len()) as u128 > mask(width) {
            // It would wrap: add it up instead.
            let mut parts = base.map_or_else(Vec::new, |base| self.count_parts(base));
            parts.extend(own);
            let (one, zero) = (self.bv(width, 1), self.bv(width, 0));
            let ones: Vec<Term> = parts
                .iter()
                .map(|part| self.ite(*part, one, zero))
                .collect();
            return self.sum(width, &ones);
        }
        own.sort_unstable();
        if let Some(base) = base {
            // The base counts two or more conditions, not all true: so does this count.
            if own.is_empty() {
                return self.make(Node::Count(base, width), Sort::Bv(width));
            }
            return self.stored_count(width, Some(base), own);
        }
        if own.is_empty() {
            return self.bv(width, 0);
        }
        if own.iter().all(|part| self.truth(*part) == Some(true)) {
            return self.bv(width, own.len() as u128);
        }
        // A count of one condition is that condition's 1 or 0, the form a flag a program adds
        // takes, so the two are one term.
        if let [only] = own[..] {
            let (one, zero) = (self.bv(width, 1), self.bv(width, 0));
            return self.ite(only, one, zero);
        }
        self.stored_count(width, None, own)
    }

    /// The count of count `base`'s conditions, if any, then `own`, sorted, as a `width`-bit
    /// number: an earlier count of the same conditions, however it holds them, or a new one.
    fn stored_count(&mut self, width: u32, base: Option<u32>, own: Vec<Term>) -> Term {
        let (mut len, mut sure, mut hash, mut sorted, mut last) =
            base.map_or((0, 0, 0_u64, true, None), |base| {
                let counted = &self.counts[base as usize];
                (
                    counted.len,
                    counted.sure,
                    counted.hash,
                    counted.sorted,
                    Some(counted.last),
                )
            });
        for part in &own {
            len += 1;
            sure += usize::from(matches!(self.node(*part), Node::Bool(true)));
            hash = hash.wrapping_add(mix(*part));
        }
        if let (Some(before), Some(first)) = (last, own.first()) {
            sorted &= before <= *first;
        }
        last = last.max(own.last().copied());
        let mut candidate = self.count_index.get(&hash).copied();
        while let Some(number) = candidate {
            if self.same_count(number, base, &own, len) {
                return self.make(Node::Count(number, width), Sort::Bv(width));
            }
            candidate = self.counts[number as usize].same_hash;
        }
        let number = u32::try_from(self.counts.len()).unwrap_or(u32::MAX);
        let same_hash = self.count_index.insert(hash, number);
        self.counts.push(Counted {
            base,
            own,
            len,
            sure,
            last: last.unwrap_or(Term(0)),
            sorted,
            hash,
            same_hash,
        });
        self.make(Node::Count(number, width), Sort::Bv(width))
    }

    /// Whether count `number` counts what count `base`, if any, and then `own` do.
    fn same_count(&self, number: u32, base: Option<u32>, own: &[Term], len: usize) -> bool {
        let counted = &self.counts[number as usize];
        if counted.len != len {
            return false;
        }
        if counted.base == base && counted.own == own {
            return true;
        }
        let mut mine = base.map_or_else(Vec::new, |base| self.count_parts(base));
        mine.extend_from_slice(own);
        mine.sort_unstable();
        self.count_parts(number) == mine
    }

    /// The number of the count `term` is, or is widened from.
    fn count_number(&self, term: Term) -> Option<u32> {
        match *self.node(term) {
            Node::Count(number, _) => Some(number),
            Node::ZeroExtend(inner, _) if self.width(inner) != 1 => self.count_number(inner),
            _ => None,
        }
    }

    /// `part`, a condition a count counts, in one form however a program or a formula grouped
    /// its conjunctions: its conjuncts, sorted, joined in that order. So a part a merge guarded
    /// by where the input ends and the formula's own guarded part, the same conditions joined
    /// in another order, are one term. One of more than [`MAX_CONJUNCTS`] conjuncts is kept as
    /// it is.
    fn canonical_part(&mut self, part: Term) -> Term {
        if !matches!(self.node(part), Node::BoolAnd(..)) {
            return part;
        }
        if let Some(found) = self.canonical_parts.get(&part) {
            return *found;
        }
        let mut conjuncts = Vec::new();
        let mut pending = vec![part];
        while let Some(current) = pending.pop() {
            if let Node::BoolAnd(a, b) = *self.node(current) {
                if conjuncts.len() + pending.len() >= MAX_CONJUNCTS {
                    self.canonical_parts.insert(part, part);
                    return part;
                }
                pending.extend([a, b]);
            } else {
                conjuncts.push(current);
            }
        }
        conjuncts.sort_unstable();
        conjuncts.dedup();
        let mut joined = conjuncts[0];
        for conjunct in &conjuncts[1..] {
            joined = self.bool_and(joined, *conjunct);
        }
        self.canonical_parts.insert(part, joined);
        self.canonical_parts.insert(joined, joined);
        joined
    }

    /// `op` of two floats, both `width(a)` bits.
    pub fn float(&mut self, op: FloatOp, a: Term, b: Term) -> Term {
        let width = self.width(a);
        if let (Some(x), Some(y)) = (self.constant(a), self.constant(b)) {
            return self.bv(width, float::binary(op, x, y, width));
        }
        self.make(Node::Float(op, a, b), Sort::Bv(width))
    }

    /// A float's square root.
    pub fn float_sqrt(&mut self, a: Term) -> Term {
        let width = self.width(a);
        if let Some(x) = self.constant(a) {
            return self.bv(width, float::sqrt(x, width));
        }
        self.make(Node::FloatSqrt(a), Sort::Bv(width))
    }

    /// `a × b + c`, rounded once.
    pub fn float_fma(&mut self, a: Term, b: Term, c: Term) -> Term {
        let width = self.width(a);
        if let (Some(x), Some(y), Some(z)) = (self.constant(a), self.constant(b), self.constant(c))
        {
            return self.bv(width, float::fma(x, y, z, width));
        }
        self.make(Node::FloatFma(a, b, c), Sort::Bv(width))
    }

    /// A signed whole number as the nearest float of `width` bits.
    pub fn float_from_int(&mut self, number: Term, width: u32) -> Term {
        let from = self.width(number);
        if let Some(x) = self.constant(number) {
            return self.bv(width, float::from_int(x, from, width));
        }
        self.make(Node::FloatFromInt(number, width), Sort::Bv(width))
    }

    /// A float as a signed whole number of `to` bits, rounded to nearest or truncated. A float
    /// rounded to a whole one first converts as the rounding would, so `roundsd` then
    /// `cvttsd2si` and a formula's `round(x)` are one term.
    pub fn float_to_int(&mut self, a: Term, to: u32, truncate: bool) -> Term {
        let (a, truncate) = match *self.node(a) {
            Node::FloatRound(inner, Rounding::Nearest) => (inner, false),
            Node::FloatRound(inner, Rounding::Zero) => (inner, true),
            Node::FloatRound(..) => (a, true),
            _ => (a, truncate),
        };
        let width = self.width(a);
        if let Some(x) = self.constant(a) {
            return self.bv(to, float::to_int(x, width, to, truncate));
        }
        self.make(Node::FloatToInt(a, to, truncate), Sort::Bv(to))
    }

    /// A float rounded to a whole float.
    pub fn float_round(&mut self, a: Term, rounding: Rounding) -> Term {
        let width = self.width(a);
        if let Some(x) = self.constant(a) {
            return self.bv(width, float::round(x, rounding, width));
        }
        self.make(Node::FloatRound(a, rounding), Sort::Bv(width))
    }

    /// A float as the nearest float of `to` bits.
    pub fn float_convert(&mut self, a: Term, to: u32) -> Term {
        let from = self.width(a);
        if from == to {
            return a;
        }
        if let Some(x) = self.constant(a) {
            return self.bv(to, float::convert(x, from, to));
        }
        self.make(Node::FloatConvert(a, to), Sort::Bv(to))
    }

    /// What `test` says of two floats.
    pub fn float_test(&mut self, test: FloatTest, a: Term, b: Term) -> Term {
        let width = self.width(a);
        if let (Some(x), Some(y)) = (self.constant(a), self.constant(b)) {
            return self.boolean(float::test(test, x, y, width));
        }
        self.make(Node::FloatTest(test, a, b), Sort::Bool)
    }

    /// The CRC state after `byte`, from the 32-bit `state`, by the reflected polynomial `poly`.
    pub fn crc(&mut self, state: Term, byte: Term, poly: u32) -> Term {
        if let (Some(state), Some(byte)) = (self.constant(state), self.constant(byte)) {
            // Constants are 32 and 8 bits wide.
            let value = crc_byte(poly, state as u32, byte as u8);
            return self.bv(32, u128::from(value));
        }
        self.make(Node::Crc(state, byte, poly), Sort::Bv(32))
    }

    /// The conditions `term` counts, when it's a count or a 0 or 1, as [`Self::counted`] finds
    /// them.
    pub fn counted_parts(&mut self, term: Term) -> Option<Vec<Term>> {
        if self.sort(term) == Sort::Bool || !self.is_counted(term) && self.constant(term) != Some(0)
        {
            return None;
        }
        self.counted(term)
    }

    /// Whether [`Self::counted`] finds conditions in `term`, without making any terms.
    fn is_counted(&self, term: Term) -> bool {
        match *self.node(term) {
            Node::Count(..) => true,
            Node::Ite(_, then, otherwise) => matches!(
                (self.constant(then), self.constant(otherwise)),
                (Some(1), Some(0)) | (Some(0), Some(1))
            ),
            Node::ZeroExtend(inner, _) => self.width(inner) == 1 || self.is_counted(inner),
            _ => false,
        }
    }

    /// The conditions `term` counts, when it's a count or a 0 or 1: a count, a choice of 1 or 0,
    /// or one of those widened.
    fn counted(&mut self, term: Term) -> Option<Vec<Term>> {
        match *self.node(term) {
            Node::Count(number, _) => Some(self.count_parts(number)),
            Node::Const { value: 0, .. } => Some(Vec::new()),
            Node::Ite(condition, then, otherwise) => {
                match (self.constant(then), self.constant(otherwise)) {
                    (Some(1), Some(0)) => Some(vec![condition]),
                    (Some(0), Some(1)) => Some(vec![self.bool_not(condition)]),
                    _ => None,
                }
            }
            Node::ZeroExtend(inner, _) if self.width(inner) == 1 => {
                let one = self.bv(1, 1);
                Some(vec![self.eq(inner, one)])
            }
            Node::ZeroExtend(inner, _) => self.counted(inner),
            _ => None,
        }
    }

    /// The range of a node's values, from its operands' ranges.
    fn range_of(&self, node: &Node, sort: Sort) -> (u128, u128) {
        let width = match sort {
            Sort::Bv(width) => width,
            Sort::Bool => {
                return match node {
                    Node::Bool(value) => (u128::from(*value), u128::from(*value)),
                    Node::FloatTest(test, a, b) => self
                        .float_bounds(*a)
                        .zip(self.float_bounds(*b))
                        .and_then(|(a, b)| float::test_bounds(*test, a, b))
                        .map_or((0, 1), |holds| (u128::from(holds), u128::from(holds))),
                    _ => (0, 1),
                };
            }
        };
        let full = (0, mask(width));
        let r = |term: &Term| self.range(*term);
        let fits = |value: u128| value <= mask(width);
        match node {
            Node::Const { value, .. } => (*value, *value),
            Node::Var { name, .. } => self.declared.get(name).copied().unwrap_or(full),
            Node::Count(number, _) => {
                let counted = &self.counts[*number as usize];
                (counted.sure as u128, counted.len as u128)
            }
            Node::Add(a, b) => {
                // Each end's sum, and whether it wrapped past the width.
                let wrap = |x: u128, y: u128| {
                    if width == 128 {
                        x.overflowing_add(y)
                    } else {
                        let sum = x + y;
                        (sum & mask(width), sum > mask(width))
                    }
                };
                let ((al, ah), (bl, bh)) = (r(a), r(b));
                let ((low, low_wrapped), (high, high_wrapped)) = (wrap(al, bl), wrap(ah, bh));
                // When both ends wrap the same way, so does everything between: adding a
                // constant below zero, as `x - 48` does, keeps a range.
                if low_wrapped == high_wrapped && low <= high {
                    (low, high)
                } else {
                    full
                }
            }
            Node::Mul(a, b) => {
                let ((al, ah), (bl, bh)) = (r(a), r(b));
                match ah.checked_mul(bh) {
                    Some(high) if fits(high) => (al * bl, high),
                    _ => full,
                }
            }
            Node::Udiv(a, b) => {
                let ((al, ah), (bl, bh)) = (r(a), r(b));
                match ah.checked_div(bl) {
                    Some(high) => (al / bh, high),
                    None => full,
                }
            }
            Node::Urem(a, b) => {
                let ((al, ah), (bl, bh)) = (r(a), r(b));
                if bl == 0 {
                    full
                } else if ah < bl {
                    (al, ah)
                } else {
                    (0, ah.min(bh - 1))
                }
            }
            Node::And(a, b) => (0, r(a).1.min(r(b).1)),
            Node::Or(a, b) | Node::Xor(a, b) => (0, smear(r(a).1.max(r(b).1))),
            Node::Not(a) => {
                let (low, high) = r(a);
                (mask(width) - high, mask(width) - low)
            }
            Node::Neg(a) => {
                let (low, high) = r(a);
                if low > 0 {
                    (mask(width) - high + 1, mask(width) - low + 1)
                } else {
                    full
                }
            }
            Node::Shl(a, b) => match (self.constant(*b), r(a)) {
                (Some(shift), (low, high))
                    if shift < u128::from(width)
                        && fits(high << shift)
                        && high.leading_zeros() as u128 >= shift =>
                {
                    (low << shift, high << shift)
                }
                _ => full,
            },
            Node::Lshr(a, b) => match self.constant(*b) {
                Some(shift) if shift < u128::from(width) => (r(a).0 >> shift, r(a).1 >> shift),
                _ => (0, r(a).1),
            },
            Node::Extract(a, high, low) => {
                let (from, to) = r(a);
                if to >> (*high + 1).min(127) == 0 || *high == 127 {
                    (from >> *low, to >> *low)
                } else {
                    full
                }
            }
            Node::ZeroExtend(a, _) => r(a),
            Node::SignExtend(a, _) => {
                let inner = self.width(*a);
                if r(a).1 <= mask(inner - 1) {
                    r(a)
                } else {
                    full
                }
            }
            Node::Concat(high, low) => {
                let shift = self.width(*low);
                (
                    (r(high).0 << shift) | r(low).0,
                    (r(high).1 << shift) | r(low).1,
                )
            }
            Node::Ite(condition, a, b) => {
                let (then, otherwise) = (
                    self.range_given(*condition, true, *a),
                    self.range_given(*condition, false, *b),
                );
                (then.0.min(otherwise.0), then.1.max(otherwise.1))
            }
            // A conversion of a float whose bounds stay inside the width, read unsigned.
            Node::FloatToInt(a, to, truncate) => {
                match self
                    .float_bounds(*a)
                    .and_then(|bounds| float::to_int_range(bounds, *to, *truncate))
                {
                    Some((low, high)) if low >= 0 || high < 0 => {
                        let unsigned = |value: i128| value.cast_unsigned() & mask(width);
                        (unsigned(low), unsigned(high))
                    }
                    _ => full,
                }
            }
            _ => full,
        }
    }

    /// The bounds of a float result: from its operands' bounds, or for a whole number
    /// converted, from its range.
    fn float_bounds_of(&self, node: &Node) -> Option<(f64, f64)> {
        let bounds = |term: &Term| self.float_bounds(*term);
        match node {
            Node::FloatFromInt(number, width) => {
                let (low, high) = self.signed_range(*number);
                Some(float::from_int_bounds(
                    low,
                    high,
                    self.width(*number),
                    *width,
                ))
            }
            Node::Float(op, a, b) => {
                float::binary_bounds(*op, bounds(a)?, bounds(b)?, self.width(*a))
            }
            Node::FloatSqrt(a) => float::sqrt_bounds(bounds(a)?, self.width(*a)),
            Node::FloatFma(a, b, c) => {
                float::fma_bounds(bounds(a)?, bounds(b)?, bounds(c)?, self.width(*a))
            }
            Node::FloatRound(a, rounding) => {
                float::round_bounds(bounds(a)?, *rounding, self.width(*a))
            }
            Node::FloatConvert(a, to) => float::convert_bounds(bounds(a)?, self.width(*a), *to),
            // A float with its sign flipped, as `-x`, or cleared, as `abs(x)`.
            Node::Xor(a, b) | Node::And(a, b) if matches!(self.width(*a), 32 | 64) => {
                let sign = 1_u128 << (self.width(*a) - 1);
                let (value, constant) = match (self.constant(*a), self.constant(*b)) {
                    (None, Some(constant)) => (*a, constant),
                    (Some(constant), None) => (*b, constant),
                    _ => return None,
                };
                let (low, high) = bounds(&value)?;
                match node {
                    Node::Xor(..) if constant == sign => Some((-high, -low)),
                    Node::And(..) if constant == sign - 1 => Some(if low >= 0.0 {
                        (low, high)
                    } else if high <= 0.0 {
                        (-high, -low)
                    } else {
                        (0.0, high.max(-low))
                    }),
                    _ => None,
                }
            }
            // One of two floats.
            Node::Ite(_, a, b) if matches!(self.width(*a), 32 | 64) => {
                let ((al, ah), (bl, bh)) = (bounds(a)?, bounds(b)?);
                Some((al.min(bl), ah.max(bh)))
            }
            _ => None,
        }
    }

    /// The smallest and largest value `term` can have read as a float of its width, 32 or 64
    /// bits, when it's a constant or a float result whose bounds are known; `None` where it
    /// could be a NaN.
    #[must_use]
    pub fn float_bounds(&self, term: Term) -> Option<(f64, f64)> {
        let width = self.width(term);
        if let Some(bits) = self.constant(term) {
            let value = float::value(bits, width);
            return (matches!(width, 32 | 64) && !value.is_nan()).then_some((value, value));
        }
        self.float_bounds.get(term.index()).copied().flatten()
    }

    /// Whether `condition` being `holds` settles that `value`, read as signed, is at least zero:
    /// `0 < v` or `not v < 0` for `v` itself, `v < 0` or `not 0 < v` for `-v`.
    fn non_negative_when(&self, condition: Term, holds: bool, value: Term) -> bool {
        let (condition, holds) = match *self.node(condition) {
            Node::BoolNot(inner) => (inner, !holds),
            _ => (condition, holds),
        };
        let Node::Slt(left, right) = *self.node(condition) else {
            return false;
        };
        let width = self.width(left);
        if self.width(value) != width {
            return false;
        }
        let smallest = if width >= 128 {
            i128::MIN
        } else {
            -(1_i128 << (width - 1))
        };
        let negation_of =
            |other: Term| self.negates_value(value, other) && self.signed_range(other).0 > smallest;
        match (self.constant(left), self.constant(right), holds) {
            (Some(0), None, true) | (None, Some(0), false) => {
                value
                    == if self.constant(left) == Some(0) {
                        right
                    } else {
                        left
                    }
            }
            (Some(0), None, false) => negation_of(right),
            (None, Some(0), true) => negation_of(left),
            _ => false,
        }
    }

    /// The values a sum is made of, each as the value it's widened from, sorted: two terms with
    /// no part in common are never one sum moved or scaled, so no relation can hold between
    /// them.
    #[must_use]
    pub fn parts(&self, term: Term) -> Vec<Term> {
        let mut parts: Vec<Term> = self
            .linear_view(term)
            .parts
            .keys()
            .map(|part| self.widened_from(*part).0)
            .collect();
        parts.sort_unstable();
        parts.dedup();
        parts
    }

    /// Whether `y` has a part among `parts`, or either has none: then `y` may be the sum of
    /// `parts`, moved or scaled.
    #[must_use]
    pub fn shares_parts(&self, parts: &[Term], y: Term) -> bool {
        let theirs = self.parts(y);
        parts.is_empty()
            || theirs.is_empty()
            || theirs.iter().any(|part| parts.binary_search(part).is_ok())
    }

    /// How `x` is made from `y`, modulo 2^width of `x`: `x = y + offset` (`false`) or
    /// `x = -y + offset` (`true`), when their sums have the same parts. `y` may be wider, as a
    /// comparison lifted to 128 bits is: a part widened from a value counts as that value.
    #[must_use]
    pub fn related(&self, x: Term, y: Term) -> Option<(bool, u128)> {
        let width = self.width(x);
        if width == 0 || self.width(y) < width {
            return None;
        }
        let reduced = |term: Term| {
            let linear = self.linear_view(term);
            let mut parts: BTreeMap<(Term, bool), u128> = BTreeMap::new();
            for (part, times) in linear.parts {
                let key = self.widened_from(part);
                if self.width(key.0) > width {
                    return None;
                }
                let entry = parts.entry(key).or_insert(0);
                *entry = entry.wrapping_add(times) & mask(width);
            }
            parts.retain(|_, times| *times != 0);
            Some((parts, linear.constant & mask(width)))
        };
        let ((left, left_constant), (right, right_constant)) = (reduced(x)?, reduced(y)?);
        if left == right {
            return Some((
                false,
                left_constant.wrapping_sub(right_constant) & mask(width),
            ));
        }
        let negated: BTreeMap<(Term, bool), u128> = right
            .iter()
            .map(|(key, times)| (*key, times.wrapping_neg() & mask(width)))
            .collect();
        (left == negated).then(|| {
            (
                true,
                left_constant.wrapping_add(right_constant) & mask(width),
            )
        })
    }

    /// How `x` is made from `y` when `x` is the wider, as whole numbers read signed: `x = y + d`
    /// (`false`) or `x = -y + d` (`true`), when their sums have the same parts with the same
    /// multiples and neither ever wraps, so each is exactly its parts' sum.
    #[must_use]
    pub fn related_wider(&self, x: Term, y: Term) -> Option<(bool, i128)> {
        let (wide, narrow) = (self.width(x), self.width(y));
        if narrow == 0 || wide < narrow || wide > 128 || (narrow > 126 && narrow != 128) {
            return None;
        }
        let exact = |term: Term, width: u32| {
            let linear = self.linear_view(term);
            let mut parts: BTreeMap<(Term, bool), i128> = BTreeMap::new();
            for (part, times) in &linear.parts {
                *parts.entry(self.widened_from(*part)).or_insert(0) += signed(*times, width);
            }
            parts.retain(|_, times| *times != 0);
            // Never wrapping: the exact value stays inside what the bits hold, read signed.
            let (low, high) = self.exact_bounds(&linear, width, true)?;
            Self::window(low, high, width, true).filter(|shift| *shift == 0)?;
            Some((parts, signed(linear.constant, width)))
        };
        let ((left, left_constant), (right, right_constant)) = (exact(x, wide)?, exact(y, narrow)?);
        if left == right {
            return Some((false, left_constant.checked_sub(right_constant)?));
        }
        let negated: BTreeMap<(Term, bool), i128> =
            right.iter().map(|(key, times)| (*key, -*times)).collect();
        (left == negated).then_some((true, left_constant.checked_add(right_constant)?))
    }

    /// What `other` comes to wherever `given` is `holds`, when both compare a sum with a
    /// constant, signed, and the sums are the same but for a constant or a sign, neither
    /// wrapping: where the one sum lies decides the other. `None` when that doesn't settle it.
    fn implied(&self, given: Term, holds: bool, other: Term) -> Option<bool> {
        let (x, x_low, x_high) = self.interval(given, holds)?;
        // Only the values the sum can take.
        let (range_low, range_high) = self.signed_range(x);
        let (x_low, x_high) = (x_low.max(range_low), x_high.min(range_high));
        if x_low > x_high {
            return None;
        }
        let (y, y_low, y_high) = self.interval(other, true)?;
        let (negated, delta) = if self.width(y) >= self.width(x) {
            self.related_wider(y, x)?
        } else {
            let (negated, delta) = self.related_wider(x, y)?;
            // x = ±y + d, so y = ±(x - d).
            (negated, if negated { delta } else { delta.checked_neg()? })
        };
        let (low, high) = if negated {
            (delta.checked_sub(x_high)?, delta.checked_sub(x_low)?)
        } else {
            (x_low.checked_add(delta)?, x_high.checked_add(delta)?)
        };
        if low >= y_low && high <= y_high {
            Some(true)
        } else if high < y_low || low > y_high {
            Some(false)
        } else {
            None
        }
    }

    /// `condition` with what it says of `x` alone, a value from its range such as a text's
    /// length, written as one comparison or two: the conjuncts made only of comparisons of `x`
    /// moved by constants (or of their low bits) with constants, joined by and, or, not and
    /// choices, are the set of `x`'s values where they hold. So however a program works out
    /// whether a byte is inside the text, as a mask's bit of the bytes left in its last chunk,
    /// the condition comes out as the formula's `position < length`.
    pub fn on_length(&mut self, condition: Term, x: Term) -> Term {
        let range = self.range(x);
        let mut pending = vec![condition];
        let mut said = Vec::new();
        let mut others = Vec::new();
        let mut set = vec![range];
        while let Some(part) = pending.pop() {
            if let Node::BoolAnd(a, b) = *self.node(part) {
                pending.extend([a, b]);
                continue;
            }
            if let Some(where_) = self.holds_where(part, x, range) {
                set = intersect(&set, &where_);
                said.push(part);
            } else {
                others.push(part);
            }
        }
        let Some(&first) = said.first() else {
            return condition;
        };
        let comparison = self.within_intervals(&set, x, range);
        // Where the condition holds, x is in the set: comparisons of x inside its other
        // conjuncts that the set settles are settled, as a choice between a whole step and a
        // partial one inside what a word's carry says.
        let mut replaced: HashMap<Term, Term> = HashMap::new();
        if !set.is_empty() {
            let facts = self.settled_on(&others, x, range, &set);
            if !facts.is_empty() {
                for other in &others {
                    let read = self.assuming(*other, &facts);
                    if read != *other {
                        replaced.insert(*other, read);
                    }
                }
            }
        }
        if said.len() == 1 && comparison == first && replaced.is_empty() {
            return condition;
        }
        // The first conjunct about x says all of them do, the rest say nothing more, each in
        // its place: what else the condition says keeps its form, as a formula's would.
        let true_ = self.boolean(true);
        for part in &said {
            replaced.insert(*part, true_);
        }
        replaced.insert(first, comparison);
        self.conjuncts_replaced(condition, &replaced)
    }

    /// The comparisons of `x` inside `conditions` that `x` lying in `set` settles: true where
    /// they hold all through the set, false where nowhere in it. Only terms of more than one
    /// byte are walked, at most [`MAX_WALKED`] of them, since nothing of one byte speaks of `x`.
    fn settled_on(
        &self,
        conditions: &[Term],
        x: Term,
        range: (u128, u128),
        set: &[(u128, u128)],
    ) -> HashMap<Term, bool> {
        let mut facts = HashMap::new();
        let mut seen = std::collections::HashSet::new();
        let mut pending = conditions.to_vec();
        while let Some(term) = pending.pop() {
            if seen.len() >= MAX_WALKED || !seen.insert(term) {
                continue;
            }
            if !matches!(self.soles[term.index()], Sole::Many) {
                continue;
            }
            if matches!(self.node(term), Node::Ult(..) | Node::Eq(..))
                && let Some(holds) = self.holds_where(term, x, range)
            {
                let both = intersect(set, &holds);
                if both == set {
                    facts.insert(term, true);
                    continue;
                }
                if both.is_empty() {
                    facts.insert(term, false);
                    continue;
                }
            }
            pending.extend(self.operands(term));
        }
        facts
    }

    /// `condition` with each of its conjuncts in `replaced` replaced, and the conjunctions
    /// around them built again, one at a time, without a call for each level.
    fn conjuncts_replaced(&mut self, condition: Term, replaced: &HashMap<Term, Term>) -> Term {
        let mut done: HashMap<Term, Term> = HashMap::new();
        let mut pending = vec![(condition, false)];
        while let Some((current, expanded)) = pending.pop() {
            if done.contains_key(&current) {
                continue;
            }
            if let Some(by) = replaced.get(&current) {
                done.insert(current, *by);
                continue;
            }
            let Node::BoolAnd(a, b) = *self.node(current) else {
                done.insert(current, current);
                continue;
            };
            if expanded {
                let (new_a, new_b) = (
                    done.get(&a).copied().unwrap_or(a),
                    done.get(&b).copied().unwrap_or(b),
                );
                let rebuilt = if (new_a, new_b) == (a, b) {
                    current
                } else {
                    self.bool_and(new_a, new_b)
                };
                done.insert(current, rebuilt);
            } else {
                pending.extend([(current, true), (a, false), (b, false)]);
            }
        }
        done.get(&condition).copied().unwrap_or(condition)
    }

    /// That `x`, which lies in `range`, lies in one of `set`'s intervals: true, false, or a
    /// comparison with each end that isn't `range`'s.
    fn within_intervals(&mut self, set: &[(u128, u128)], x: Term, range: (u128, u128)) -> Term {
        let width = self.width(x);
        let mut ways = Vec::new();
        for (low, high) in set {
            let mut sides = Vec::new();
            if *low > range.0 {
                let before = self.bv(width, low - 1);
                sides.push(self.ult(before, x));
            }
            if *high < range.1 {
                let after = self.bv(width, high + 1);
                sides.push(self.ult(x, after));
            }
            ways.push(self.all(&sides));
        }
        self.any(&ways)
    }

    /// The values of `x` in `range` where `condition` holds, as sorted, disjoint intervals, when
    /// it's made only of comparisons of `x` moved by constants, or of their low bits, with
    /// constants, joined by and, or, not and choices. `None` for anything else. Each part is read
    /// once, without a call for each level, so a condition built byte by byte, as a matcher's
    /// state is, can be as deep as a text is long; and the parts read at once are read before the
    /// joined ones, so a part that isn't about `x` ends the walk before a long chain is walked.
    #[must_use]
    pub fn holds_where(
        &self,
        condition: Term,
        x: Term,
        range: (u128, u128),
    ) -> Option<Vec<(u128, u128)>> {
        let mut sets: HashMap<Where, Vec<(u128, u128)>> = HashMap::new();
        let mut pending = vec![(Where::Holds(condition), false)];
        while let Some((question, expanded)) = pending.pop() {
            if sets.contains_key(&question) {
                continue;
            }
            match self.reading(question, x, range) {
                Reading::Set(set) => {
                    sets.insert(question, set);
                }
                Reading::Unread => return None,
                Reading::Parts(joined, parts) if expanded => {
                    let mut read = Vec::with_capacity(parts.len());
                    for part in &parts {
                        read.push(sets.get(part)?);
                    }
                    let set = match (joined, read.as_slice()) {
                        (Joined::Same, [only]) => (*only).clone(),
                        (Joined::Not, [only]) => complement(only, range),
                        (Joined::And, [a, b]) => intersect(a, b),
                        (Joined::Or, [a, b]) => union(a, b),
                        (Joined::Except, [kept, left_out]) => {
                            intersect(kept, &complement(left_out, range))
                        }
                        (Joined::Choice, [chosen, then, otherwise]) => union(
                            &intersect(chosen, then),
                            &intersect(&complement(chosen, range), otherwise),
                        ),
                        _ => return None,
                    };
                    sets.insert(question, set);
                }
                Reading::Parts(_, parts) => {
                    pending.push((question, true));
                    // Joined parts first, so the rest, which are read at once, come off first.
                    let (joined, at_once): (Vec<Where>, Vec<Where>) = parts
                        .into_iter()
                        .filter(|part| !sets.contains_key(part))
                        .partition(|part| self.joins(*part, x));
                    pending.extend(joined.into_iter().map(|part| (part, false)));
                    pending.extend(at_once.into_iter().map(|part| (part, false)));
                }
            }
        }
        sets.remove(&Where::Holds(condition))
    }

    /// Whether [`Self::reading`] reads `question` from parts of its own.
    fn joins(&self, question: Where, x: Term) -> bool {
        match question {
            Where::Holds(condition) => match *self.node(condition) {
                Node::BoolNot(_) | Node::BoolAnd(..) | Node::BoolOr(..) => true,
                Node::Ite(_, then, _) => self.sort(then) == Sort::Bool,
                _ => false,
            },
            Where::Below(value, _) => match *self.node(value) {
                Node::Ite(..) => true,
                Node::ZeroExtend(inner, _) => inner != x,
                _ => false,
            },
        }
    }

    /// One step of [`Self::holds_where`]: the set `question` asks for, at once, or the parts
    /// whose sets make it, for a value that's a constant, `x` moved by a constant, its low bits,
    /// a widening of one, or a choice of two on a condition it reads.
    fn reading(&self, question: Where, x: Term, range: (u128, u128)) -> Reading {
        let full = |holds: bool| Reading::Set(if holds { vec![range] } else { Vec::new() });
        let moved = |term: Term, low_bits: Option<u32>, k: u128| {
            let Some((bits, offset)) = self.moved(term, x) else {
                return Reading::Unread;
            };
            let (bits, offset) = match low_bits {
                Some(high) => {
                    let bits = bits.min(high + 1);
                    (bits, offset & mask(bits))
                }
                None => (bits, offset),
            };
            modular_below(range, offset, bits, k).map_or(Reading::Unread, Reading::Set)
        };
        match question {
            Where::Holds(condition) => {
                if let Some(truth) = self.truth(condition) {
                    return full(truth);
                }
                match *self.node(condition) {
                    Node::BoolNot(inner) => Reading::Parts(Joined::Not, vec![Where::Holds(inner)]),
                    Node::BoolAnd(a, b) => {
                        Reading::Parts(Joined::And, vec![Where::Holds(a), Where::Holds(b)])
                    }
                    Node::BoolOr(a, b) => {
                        Reading::Parts(Joined::Or, vec![Where::Holds(a), Where::Holds(b)])
                    }
                    Node::Ite(choice, then, otherwise) if self.sort(then) == Sort::Bool => {
                        Reading::Parts(
                            Joined::Choice,
                            vec![
                                Where::Holds(choice),
                                Where::Holds(then),
                                Where::Holds(otherwise),
                            ],
                        )
                    }
                    Node::Ult(a, b) => {
                        if let Some(k) = self.constant(b) {
                            return Reading::Parts(Joined::Same, vec![Where::Below(a, k)]);
                        }
                        let Some(k) = self.constant(a) else {
                            return Reading::Unread;
                        };
                        // k < v is not v < k + 1.
                        match k.checked_add(1) {
                            Some(next) => Reading::Parts(Joined::Not, vec![Where::Below(b, next)]),
                            None => full(false),
                        }
                    }
                    Node::Eq(a, b) => {
                        let (value, k) = match (self.constant(a), self.constant(b)) {
                            (None, Some(k)) => (a, k),
                            (Some(k), None) => (b, k),
                            _ => return Reading::Unread,
                        };
                        // At most k and not below it.
                        match k.checked_add(1) {
                            Some(next) => Reading::Parts(
                                Joined::Except,
                                vec![Where::Below(value, next), Where::Below(value, k)],
                            ),
                            None => Reading::Parts(Joined::Not, vec![Where::Below(value, k)]),
                        }
                    }
                    _ => Reading::Unread,
                }
            }
            Where::Below(value, k) => {
                if let Some(constant) = self.constant(value) {
                    return full(constant < k);
                }
                match *self.node(value) {
                    Node::ZeroExtend(inner, _) if inner != x => {
                        Reading::Parts(Joined::Same, vec![Where::Below(inner, k)])
                    }
                    Node::Ite(choice, then, otherwise) => Reading::Parts(
                        Joined::Choice,
                        vec![
                            Where::Holds(choice),
                            Where::Below(then, k),
                            Where::Below(otherwise, k),
                        ],
                    ),
                    Node::Extract(inner, high, 0) => moved(inner, Some(high), k),
                    _ => moved(value, None, k),
                }
            }
        }
    }

    /// When `term` is `x`, widened with zeros or its low bits or not, plus a constant, modulo 2
    /// to its width: the width and the constant. Low bits of `x` plus a constant are `x` plus it,
    /// modulo as many bits.
    fn moved(&self, term: Term, x: Term) -> Option<(u32, u128)> {
        let is_x = |part: Term| match *self.node(part) {
            _ if part == x => true,
            Node::ZeroExtend(inner, _) => inner == x,
            Node::Extract(inner, high, 0) => inner == x && high + 1 >= self.width(term),
            _ => false,
        };
        if is_x(term) {
            return Some((self.width(term), 0));
        }
        let linear = self.linear_view(term);
        let [(part, times)] = linear.parts.iter().collect::<Vec<_>>()[..] else {
            return None;
        };
        (*times == 1 && is_x(*part)).then_some((self.width(term), linear.constant))
    }

    /// The value `condition` compares with a constant, when it's such a comparison.
    #[must_use]
    pub fn interval_subject(&self, condition: Term) -> Option<Term> {
        self.interval(condition, true).map(|(subject, ..)| subject)
    }

    /// The values a signed comparison of a value with a constant leaves the value where the
    /// comparison is `holds`: the value, and the bounds of one interval, read signed.
    fn interval(&self, condition: Term, holds: bool) -> Option<(Term, i128, i128)> {
        let bounds = |width: u32| (-(1_i128 << (width - 1)), (1_i128 << (width - 1)) - 1);
        match *self.node(condition) {
            Node::BoolNot(inner) => self.interval(inner, !holds),
            // a <= b is not (b < a), which the arm above reads.
            Node::Slt(a, b) => {
                let width = self.width(a);
                if !(2..=128).contains(&width) {
                    return None;
                }
                let (min, max) = bounds(width);
                if let Some(k) = self.constant(b) {
                    // a < k: below the line, or at it and above.
                    let last = signed(k, width).checked_sub(1)?;
                    return Some(if holds {
                        (a, min, last)
                    } else {
                        (a, last.checked_add(1)?, max)
                    });
                }
                if let Some(k) = self.constant(a) {
                    // k < b.
                    let first = signed(k, width).checked_add(1)?;
                    return Some(if holds {
                        (b, first, max)
                    } else {
                        (b, min, first.checked_sub(1)?)
                    });
                }
                None
            }
            // An unsigned comparison of a value that's never negative read signed, with a
            // constant that isn't either: the same interval read either way.
            Node::Ult(a, b) => {
                let width = self.width(a);
                if !(2..=128).contains(&width) {
                    return None;
                }
                let (_, max) = bounds(width);
                let positive = |term: Term| self.range(term).1 <= mask(width - 1);
                if let Some(k) = self.constant(b)
                    && positive(a)
                    && positive(b)
                {
                    let k = signed(k, width);
                    return Some(if holds {
                        (a, 0, k.checked_sub(1)?)
                    } else {
                        (a, k, max)
                    });
                }
                if let Some(k) = self.constant(a)
                    && positive(a)
                    && positive(b)
                {
                    let k = signed(k, width);
                    return Some(if holds {
                        (b, k.checked_add(1)?, max)
                    } else {
                        (b, 0, k)
                    });
                }
                None
            }
            _ => None,
        }
    }

    /// How `x` is made from `y`, as whole numbers read signed: `x = s × y + d`, when each of
    /// `x`'s parts is `s` times the same part of `y` and neither ever wraps, so each is exactly
    /// its parts' sum. A formula that prints fifty times a sum compares what the program
    /// compared of the sum itself.
    #[must_use]
    pub fn related_scaled(&self, x: Term, y: Term) -> Option<(i128, i128)> {
        let usable = |width: u32| width > 0 && (width <= 126 || width == 128);
        if !usable(self.width(x)) || !usable(self.width(y)) {
            return None;
        }
        let exact = |term: Term| {
            let width = self.width(term);
            let linear = self.linear_view(term);
            let mut parts: BTreeMap<(Term, bool), i128> = BTreeMap::new();
            for (part, times) in &linear.parts {
                *parts.entry(self.widened_from(*part)).or_insert(0) += signed(*times, width);
            }
            parts.retain(|_, times| *times != 0);
            let (low, high) = self.exact_bounds(&linear, width, true)?;
            Self::window(low, high, width, true).filter(|shift| *shift == 0)?;
            Some((parts, signed(linear.constant, width)))
        };
        let ((left, left_constant), (right, right_constant)) = (exact(x)?, exact(y)?);
        let (key, times) = left.iter().next()?;
        let other = *right.get(key)?;
        if left.len() != right.len() || other == 0 || times % other != 0 {
            return None;
        }
        let scale = times / other;
        for (key, times) in &left {
            if right.get(key)?.checked_mul(scale)? != *times {
                return None;
            }
        }
        Some((
            scale,
            left_constant.checked_sub(right_constant.checked_mul(scale)?)?,
        ))
    }

    /// A sum's parts, as the value each is widened from and its multiple read signed, for
    /// explaining why two sums differ.
    #[must_use]
    pub fn describe_parts(&self, term: Term) -> String {
        let width = self.width(term);
        let linear = self.linear_view(term);
        let mut text: Vec<String> = linear
            .parts
            .iter()
            .map(|(part, times)| {
                let (from, signed_part) = self.widened_from(*part);
                format!(
                    "{} × t{}{} = {}",
                    signed(*times, width),
                    from.index(),
                    if signed_part { "s" } else { "" },
                    self.sketch(from, 3)
                )
            })
            .collect();
        text.push(format!("+ {}", signed(linear.constant, width)));
        text.join("\n      ")
    }

    /// The value a term is widened from, and whether with its sign: a term not widened is its
    /// own, and a sign-extended value that's never negative widens either way.
    fn widened_from(&self, mut term: Term) -> (Term, bool) {
        let mut signed = false;
        loop {
            match *self.node(term) {
                Node::ZeroExtend(inner, _) if !signed => term = inner,
                Node::SignExtend(inner, _) => {
                    let width = self.width(inner);
                    signed = self.range(inner).1 > mask(width - 1);
                    term = inner;
                }
                _ => return (term, signed),
            }
        }
    }

    /// Whether `value` is `other` negated, as [`Self::neg`] builds it.
    fn negates_value(&self, value: Term, other: Term) -> bool {
        if self.width(value) != self.width(other) || self.width(value) == 0 {
            return false;
        }
        let width = self.width(value);
        let mut negated = Linear::default();
        negated.add(&self.linear_view(other), mask(width), width);
        negated == self.linear_view(value)
    }

    /// The range of `value` where `condition` is `holds`.
    fn range_given(&self, condition: Term, holds: bool, value: Term) -> (u128, u128) {
        let (mut low, mut high) = self.range(value);
        if self.non_negative_when(condition, holds, value) {
            let (_, signed_high) = self.signed_range(value);
            if let Ok(signed_high) = u128::try_from(signed_high) {
                return (low.min(signed_high), signed_high.min(high));
            }
        }
        // Where the condition compares the value with a constant, the value is on that side of
        // it: `ite(x < k, x, k)`, the smaller of the two, is at most k.
        let (condition, holds) = match *self.node(condition) {
            Node::BoolNot(inner) => (inner, !holds),
            _ => (condition, holds),
        };
        if let Node::Ult(a, b) = *self.node(condition) {
            match (a == value, b == value, self.constant(a), self.constant(b)) {
                // value < k, or value >= k.
                (true, false, _, Some(k)) => {
                    if holds {
                        high = high.min(k.saturating_sub(1));
                    } else {
                        low = low.max(k);
                    }
                }
                // k < value, or value <= k.
                (false, true, Some(k), _) => {
                    if holds {
                        low = low.max(k.saturating_add(1));
                    } else {
                        high = high.min(k);
                    }
                }
                _ => {}
            }
        }
        if low > high {
            // The branch is never taken; any range is the value's.
            return self.range(value);
        }
        (low, high)
    }

    /// A new bit-vector variable named `name` that only takes values from `low` to `high`.
    pub fn var_in(&mut self, width: u32, name: &str, low: u128, high: u128) -> Term {
        self.declared.insert(name.to_owned(), (low, high));
        self.var(width, name)
    }

    /// A bit-vector constant.
    pub fn bv(&mut self, width: u32, value: u128) -> Term {
        self.make(
            Node::Const {
                width,
                value: value & mask(width),
            },
            Sort::Bv(width),
        )
    }

    /// A bit-vector constant from a signed number.
    pub fn bv_signed(&mut self, width: u32, value: i128) -> Term {
        self.bv(width, unsigned(value, width))
    }

    /// A new bit-vector variable named `name`.
    pub fn var(&mut self, width: u32, name: &str) -> Term {
        self.make(
            Node::Var {
                width,
                name: name.to_owned(),
            },
            Sort::Bv(width),
        )
    }

    /// True or false.
    pub fn boolean(&mut self, value: bool) -> Term {
        self.make(Node::Bool(value), Sort::Bool)
    }

    /// A new true/false variable named `name`.
    pub fn bool_var(&mut self, name: &str) -> Term {
        self.make(Node::BoolVar(name.to_owned()), Sort::Bool)
    }

    /// Orders the operands of a commutative operation: constants last, then by term.
    fn ordered(&self, a: Term, b: Term) -> (Term, Term) {
        match (self.constant(a), self.constant(b)) {
            (Some(_), None) => (b, a),
            (None, Some(_)) => (a, b),
            _ if b < a => (b, a),
            _ => (a, b),
        }
    }

    /// `a + b`, as one [`Linear`] sum of both.
    pub fn add(&mut self, a: Term, b: Term) -> Term {
        let width = self.width(a);
        match (self.constant(a), self.constant(b)) {
            (Some(x), Some(y)) => return self.bv(width, x.wrapping_add(y)),
            (Some(0), _) => return b,
            (_, Some(0)) => return a,
            _ => {}
        }
        let mut linear = self.linear(a);
        let other = self.linear(b);
        linear.add(&other, 1, width);
        self.linear_term(width, linear)
    }

    /// The sum of every term in `terms`, all one width.
    pub fn sum(&mut self, width: u32, terms: &[Term]) -> Term {
        let mut linear = Linear::default();
        for term in terms {
            let part = self.linear(*term);
            linear.add(&part, 1, width);
        }
        self.linear_term(width, linear)
    }

    /// `a - b`, as `a + -b`.
    pub fn sub(&mut self, a: Term, b: Term) -> Term {
        let negated = self.neg(b);
        self.add(a, negated)
    }

    /// `a × b`, keeping the low bits. A multiple of a sum is the sum of the parts' multiples.
    pub fn mul(&mut self, a: Term, b: Term) -> Term {
        let width = self.width(a);
        let (a, b) = self.ordered(a, b);
        match (self.constant(a), self.constant(b)) {
            (Some(x), Some(y)) => return self.bv(width, x.wrapping_mul(y)),
            (_, Some(0)) => return self.bv(width, 0),
            (_, Some(1)) => return a,
            (None, Some(by)) => {
                let mut linear = Linear::default();
                let part = self.linear(a);
                linear.add(&part, by, width);
                return self.linear_term(width, linear);
            }
            _ => {}
        }
        self.make(Node::Mul(a, b), Sort::Bv(width))
    }

    /// `term` as a sum of parts, each a term that isn't a sum, times a constant.
    fn linear(&mut self, term: Term) -> Linear {
        let width = self.width(term);
        let mut linear = Linear::default();
        let mut pending = vec![(term, 1_u128)];
        while let Some((current, times)) = pending.pop() {
            let times = times & mask(width);
            if times == 0 {
                continue;
            }
            match *self.node(current) {
                Node::Const { value, .. } => {
                    linear.constant =
                        linear.constant.wrapping_add(value.wrapping_mul(times)) & mask(width);
                }
                Node::Add(x, y) => pending.extend([(x, times), (y, times)]),
                Node::Neg(x) => pending.push((x, times.wrapping_neg())),
                Node::Mul(x, by) if self.constant(by).is_some() => {
                    let by = self.constant(by).unwrap_or(0);
                    pending.push((x, times.wrapping_mul(by)));
                }
                // A value shifted left is the value times a power of two; the bits shifted out
                // are ones the multiple drops anyway.
                Node::Concat(high, low) if self.constant(low) == Some(0) => {
                    let shift = self.width(low);
                    let shifted = match *self.node(high) {
                        Node::Extract(inner, top, 0)
                            if top + 1 + shift == width && self.width(inner) == width =>
                        {
                            inner
                        }
                        _ => self.zero_extend(high, shift),
                    };
                    pending.push((shifted, times.wrapping_mul(1 << shift)));
                }
                _ => linear.add_part(current, times, width),
            }
        }
        linear
    }

    /// [`Self::linear`] without making terms: a shift by a constant counts as a multiple only
    /// when it shifts a whole value.
    fn linear_view(&self, term: Term) -> Linear {
        let width = self.width(term);
        let mut linear = Linear::default();
        let mut pending = vec![(term, 1_u128)];
        while let Some((current, times)) = pending.pop() {
            let times = times & mask(width);
            if times == 0 {
                continue;
            }
            match *self.node(current) {
                Node::Const { value, .. } => {
                    linear.constant =
                        linear.constant.wrapping_add(value.wrapping_mul(times)) & mask(width);
                }
                Node::Add(x, y) => pending.extend([(x, times), (y, times)]),
                Node::Neg(x) => pending.push((x, times.wrapping_neg())),
                Node::Mul(x, by) if self.constant(by).is_some() => {
                    let by = self.constant(by).unwrap_or(0);
                    pending.push((x, times.wrapping_mul(by)));
                }
                Node::Concat(high, low)
                    if self.constant(low) == Some(0)
                        && matches!(*self.node(high), Node::Extract(inner, top, 0)
                            if top + 1 + self.width(low) == width && self.width(inner) == width) =>
                {
                    let shift = self.width(low);
                    let Node::Extract(inner, ..) = *self.node(high) else {
                        continue;
                    };
                    pending.push((inner, times.wrapping_mul(1 << shift)));
                }
                _ => linear.add_part(current, times, width),
            }
        }
        linear
    }

    /// The term for `linear`, `width` bits wide: its parts in order, each times its multiple,
    /// then the constant.
    fn linear_term(&mut self, width: u32, mut linear: Linear) -> Term {
        self.fold_remainders(width, &mut linear);
        self.fold_counts(width, &mut linear);
        let parts: Vec<(Term, u128)> = linear
            .parts
            .iter()
            .map(|(part, times)| (*part, *times))
            .collect();
        let mut total: Option<Term> = None;
        for (part, times) in parts {
            let term = self.times(part, times, width);
            total = Some(match total {
                None => term,
                Some(sum) => self.make(Node::Add(sum, term), Sort::Bv(width)),
            });
        }
        let total = match total {
            None => return self.bv(width, linear.constant),
            Some(sum) if linear.constant == 0 => sum,
            Some(sum) => {
                let constant = self.bv(width, linear.constant);
                self.make(Node::Add(sum, constant), Sort::Bv(width))
            }
        };
        if matches!(self.node(total), Node::Add(..)) {
            self.narrow_ranges(total, &linear, width);
        }
        total
    }

    /// Narrows a sum's ranges to its exact values, which its chain of additions, where a part
    /// may overflow what the whole never does, can't see.
    fn narrow_ranges(&mut self, total: Term, linear: &Linear, width: u32) {
        let index = total.index();
        for signed_parts in [false, true] {
            let Some((low, high)) = self.exact_bounds(linear, width, signed_parts) else {
                continue;
            };
            if let Some(shift) = Self::window(low, high, width, false)
                && let (Ok(low), Ok(high)) = (
                    u128::try_from(low.saturating_add(shift)),
                    u128::try_from(high.saturating_add(shift)),
                )
            {
                let (from, to) = self.ranges[index];
                if low.max(from) <= high.min(to) {
                    self.ranges[index] = (low.max(from), high.min(to));
                }
            }
            if let Some(shift) = Self::window(low, high, width, true) {
                let (low, high) = (low.saturating_add(shift), high.saturating_add(shift));
                let (from, to) = self.signed_ranges[index];
                if low.max(from) <= high.min(to) {
                    self.signed_ranges[index] = (low.max(from), high.min(to));
                }
            }
        }
    }

    /// `part` times `times`: itself, its negation, shifted left for a power of two (the form a
    /// shift by a constant takes), or a product.
    fn times(&mut self, part: Term, times: u128, width: u32) -> Term {
        if times == 1 {
            return part;
        }
        if times == mask(width) {
            return self.make(Node::Neg(part), Sort::Bv(width));
        }
        if times.is_power_of_two() {
            let shift = times.trailing_zeros();
            let lower = self.extract(part, width - 1 - shift, 0);
            let zeros = self.bv(shift, 0);
            return self.concat(lower, zeros);
        }
        let by = self.bv(width, times);
        self.make(Node::Mul(part, by), Sort::Bv(width))
    }

    /// `x - (x / c) × c`, a sum holding `x` and `x / c` times `-c`, has `x % c` instead: the
    /// remainder however it was written.
    fn fold_remainders(&mut self, width: u32, linear: &mut Linear) {
        let quotients: Vec<(Term, Term, u128)> = linear
            .parts
            .iter()
            .filter_map(|(part, times)| {
                let Node::Udiv(dividend, divisor) = *self.node(*part) else {
                    return None;
                };
                let c = self.constant(divisor).filter(|c| *c > 0)?;
                let held = linear
                    .parts
                    .get(&dividend)
                    .is_some_and(|times| signed(*times, width) >= 1);
                (held && *times == c.wrapping_neg() & mask(width)).then_some((*part, dividend, c))
            })
            .collect();
        for (quotient, dividend, c) in quotients {
            if !linear.parts.contains_key(&quotient) || !linear.parts.contains_key(&dividend) {
                continue;
            }
            linear.parts.remove(&quotient);
            linear.add_part(dividend, mask(width), width);
            let divisor = self.bv(width, c);
            let remainder = self.urem(dividend, divisor);
            let remainder = self.linear(remainder);
            linear.add(&remainder, 1, width);
        }
        // A sum less c times its own quotient by c is its remainder too, though its parts are
        // the whole's: what AArch64's `udiv` and `msub` leave of a number being printed, where
        // x86-64's `div` gives the remainder itself.
        let sums: Vec<(Term, Term, u128)> = linear
            .parts
            .iter()
            .filter_map(|(part, times)| {
                let Node::Udiv(dividend, divisor) = *self.node(*part) else {
                    return None;
                };
                let c = self.constant(divisor).filter(|c| *c > 0)?;
                (*times == c.wrapping_neg() & mask(width)
                    && matches!(self.node(dividend), Node::Add(..)))
                .then_some((*part, dividend, c))
            })
            .collect();
        for (quotient, dividend, c) in sums {
            let own = self.linear_view(dividend);
            if !linear.parts.contains_key(&quotient)
                || !own
                    .parts
                    .iter()
                    .all(|(part, times)| linear.parts.get(part) == Some(times))
            {
                continue;
            }
            linear.parts.remove(&quotient);
            for part in own.parts.keys() {
                linear.parts.remove(part);
            }
            linear.constant = linear.constant.wrapping_sub(own.constant) & mask(width);
            let divisor = self.bv(width, c);
            let remainder = self.urem(dividend, divisor);
            let remainder = self.linear(remainder);
            linear.add(&remainder, 1, width);
        }
        // A quotient less c times the next one is its remainder by c, though the next was
        // written as one division, `p / (c1 c)` for `(p / c1) / c`: the digits a print loop
        // takes off one division at a time.
        let chained: Vec<(Term, Term, u128)> = linear
            .parts
            .iter()
            .filter_map(|(part, times)| {
                let Node::Udiv(dividend, divisor) = *self.node(*part) else {
                    return None;
                };
                let whole = self.constant(divisor).filter(|whole| *whole > 1)?;
                let by = times.wrapping_neg() & mask(width);
                (by > 1 && by < whole && whole % by == 0).then_some((*part, dividend, by))
            })
            .collect();
        for (quotient, dividend, by) in chained {
            let Some(whole) = self.constant(match *self.node(quotient) {
                Node::Udiv(_, divisor) => divisor,
                _ => continue,
            }) else {
                continue;
            };
            let first = self.bv(width, whole / by);
            let before = self.udiv(dividend, first);
            if !linear.parts.contains_key(&quotient)
                || !linear
                    .parts
                    .get(&before)
                    .is_some_and(|times| signed(*times, width) >= 1)
            {
                continue;
            }
            linear.parts.remove(&quotient);
            linear.add_part(before, mask(width), width);
            let divisor = self.bv(width, by);
            let remainder = self.urem(before, divisor);
            let remainder = self.linear(remainder);
            linear.add(&remainder, 1, width);
        }
    }

    /// Counts, and values that are each 0 or 1, added together are one count of all their
    /// conditions.
    fn fold_counts(&mut self, width: u32, linear: &mut Linear) {
        let counted: Vec<Term> = linear
            .parts
            .iter()
            .filter(|(part, times)| **times == 1 && self.is_counted(**part))
            .map(|(part, _)| *part)
            .collect();
        if counted.len() < 2 {
            return;
        }
        // The largest count among them is extended by the others' conditions, so what it counts
        // already isn't copied.
        let base = counted
            .iter()
            .filter_map(|term| self.count_number(*term))
            .max_by_key(|number| self.counts[*number as usize].len);
        let (mut taken, mut len) = (false, 0);
        let mut conditions = Vec::new();
        for term in &counted {
            if !taken && base.is_some() && self.count_number(*term) == base {
                taken = true;
                len += base.map_or(0, |base| self.counts[base as usize].len);
                continue;
            }
            match self.counted(*term) {
                Some(list) => {
                    len += list.len();
                    conditions.extend(list);
                }
                None => return,
            }
        }
        if len as u128 > mask(width) {
            return;
        }
        for term in &counted {
            linear.parts.remove(term);
        }
        let count = self.count_with(width, base, &conditions);
        let count = self.linear(count);
        linear.add(&count, 1, width);
    }

    /// `sum` widened by `by` bits part by part, when it never wraps: its exact value, from its
    /// parts' ranges, lies inside what its bits hold, read unsigned for zero-extension or signed
    /// for sign-extension. Then the same arithmetic done at different widths, as a program's 32
    /// bits and a formula's 64, ends up the same term.
    /// The exact value of a sum as a whole number, from its parts' ranges: each part counts with
    /// its multiple read as a signed number, and its own value read signed or unsigned as
    /// `signed_parts` says. Any such reading is the sum modulo 2^width. `None` when the bounds
    /// don't fit 128 bits.
    fn exact_bounds(
        &self,
        linear: &Linear,
        width: u32,
        signed_parts: bool,
    ) -> Option<(i128, i128)> {
        let (mut low, mut high) = (0_i128, 0_i128);
        for (part, times) in &linear.parts {
            let times = signed(*times, width);
            let (from, to) = if signed_parts {
                self.signed_range(*part)
            } else {
                let (from, to) = self.range(*part);
                (i128::try_from(from).ok()?, i128::try_from(to).ok()?)
            };
            let (p, q) = (times.checked_mul(from)?, times.checked_mul(to)?);
            low = low.checked_add(p.min(q))?;
            high = high.checked_add(p.max(q))?;
        }
        let constant = signed(linear.constant, width);
        Some((low.checked_add(constant)?, high.checked_add(constant)?))
    }

    /// Where the bits of a `width`-bit value lie, read unsigned (`floor` 0) or signed (`floor`
    /// -2^(width-1)), when its exact value runs from `low` to `high`: the multiple of 2^width to
    /// add to it, or `None` when it crosses from one window of 2^width values into the next.
    fn window(low: i128, high: i128, width: u32, signed_read: bool) -> Option<i128> {
        if width > 126 {
            // Every i128 is inside the signed window at 128 bits, and unsigned when not negative.
            return (width == 128 && (signed_read || low >= 0)).then_some(0);
        }
        let span: i128 = 1 << width;
        let floor = if signed_read { -(span / 2) } else { 0 };
        let window = low.checked_sub(floor)?.div_euclid(span);
        (high.checked_sub(floor)?.div_euclid(span) == window).then_some(-window * span)
    }

    fn widened_sum(&mut self, sum: Term, by: u32, signed_read: bool) -> Option<Term> {
        let narrow = self.width(sum);
        if narrow > 126 {
            return None;
        }
        let linear = self.linear(sum);
        let (low, high) = self.exact_bounds(&linear, narrow, signed_read)?;
        let shift = Self::window(low, high, narrow, signed_read)?;
        let constant = signed(linear.constant, narrow);
        let wide = narrow + by;
        let mut widened = Linear::default();
        for (part, times) in linear.parts {
            let extended = if signed_read {
                self.sign_extend(part, by)
            } else {
                self.zero_extend(part, by)
            };
            let extended = self.linear(extended);
            widened.add(&extended, unsigned(signed(times, narrow), wide), wide);
        }
        widened.constant = unsigned(constant.checked_add(shift)?, wide);
        Some(self.linear_term(wide, widened))
    }

    /// Unsigned `a / b`; dividing by zero gives all ones, as in SMT-LIB.
    pub fn udiv(&mut self, a: Term, b: Term) -> Term {
        let width = self.width(a);
        match (self.constant(a), self.constant(b)) {
            (_, Some(0)) => return self.bv(width, mask(width)),
            (Some(x), Some(y)) => return self.bv(width, x / y),
            (_, Some(1)) => return a,
            (_, Some(y)) if let Some(quotient) = self.divided(a, y, false) => return quotient,
            (_, Some(y)) if let Some(quotient) = self.reduced(a, y, false) => return quotient,
            // By a power of two it's a shift right: one form however it's written.
            (_, Some(y)) if y.is_power_of_two() => {
                let shift = self.bv(width, u128::from(y.trailing_zeros()));
                return self.lshr(a, shift);
            }
            (_, Some(y)) => {
                // (x / c1) / c2 is x / (c1 × c2), and 0 once c1 × c2 passes every x.
                if let Node::Udiv(inner, first) = *self.node(a)
                    && let Some(c1) = self.constant(first)
                    && c1 > 0
                {
                    return match c1.checked_mul(y).filter(|product| *product <= mask(width)) {
                        Some(product) => {
                            let divisor = self.bv(width, product);
                            self.udiv(inner, divisor)
                        }
                        None => self.bv(width, 0),
                    };
                }
                // The top bits of x from bit k on, divided by c, are x / (c × 2^k): one form
                // for a value halved by a shift, then divided, and divided at once.
                if let Node::Extract(inner, high, low) = *self.node(a)
                    && low > 0
                    && high + 1 == self.width(inner)
                    && let Some(combined) = y
                        .checked_shl(low)
                        .filter(|combined| *combined <= mask(self.width(inner)))
                {
                    let divisor = self.bv(self.width(inner), combined);
                    let quotient = self.udiv(inner, divisor);
                    return self.extract(quotient, width - 1, 0);
                }
                // zext(x) / c is zext(x / c) when c fits in x.
                if let Node::ZeroExtend(inner, by) = *self.node(a) {
                    let narrow = width - by;
                    if y <= mask(narrow) {
                        let divisor = self.bv(narrow, y);
                        let quotient = self.udiv(inner, divisor);
                        return self.zero_extend(quotient, by);
                    }
                }
                if let Some(narrowed) = self.narrowed(a, y, Self::udiv) {
                    return narrowed;
                }
            }
            _ => {}
        }
        self.make(Node::Udiv(a, b), Sort::Bv(width))
    }

    /// Unsigned `a % b`; the remainder of dividing by zero is `a`, as in SMT-LIB.
    pub fn urem(&mut self, a: Term, b: Term) -> Term {
        let width = self.width(a);
        match (self.constant(a), self.constant(b)) {
            (_, Some(0)) => return a,
            (Some(x), Some(y)) => return self.bv(width, x % y),
            (_, Some(1)) => return self.bv(width, 0),
            // By a power of two it's the low bits: one form however it's written.
            (_, Some(y)) if y.is_power_of_two() && self.range(a).1 >= y => {
                let low = self.extract(a, y.trailing_zeros() - 1, 0);
                return self.zero_extend(low, width - y.trailing_zeros());
            }
            (_, Some(y)) => {
                // A value below the divisor is its own remainder, and one whose range lies
                // within one multiple of the divisor is the value less that multiple: the
                // year of an era, once the year's range fixes the era.
                let (low, high) = self.range(a);
                if low / y == high / y {
                    let multiple = self.bv(width, low / y * y);
                    return self.sub(a, multiple);
                }
                // (x % a) % b is x % b when b divides a: seconds past the minute, however many
                // remainders a program takes on the way.
                if let Node::Urem(inner, first) = *self.node(a)
                    && let Some(c1) = self.constant(first)
                    && c1 > 0
                    && c1 % y == 0
                {
                    return self.urem(inner, b);
                }
                if let Node::ZeroExtend(inner, by) = *self.node(a) {
                    let narrow = width - by;
                    if y <= mask(narrow) {
                        let divisor = self.bv(narrow, y);
                        let remainder = self.urem(inner, divisor);
                        return self.zero_extend(remainder, by);
                    }
                }
                if let Some(narrowed) = self.narrowed(a, y, Self::urem) {
                    return narrowed;
                }
            }
            _ => {}
        }
        self.make(Node::Urem(a, b), Sort::Bv(width))
    }

    /// `a / c`, toward zero, read signed or not, as a sum when `a` is one: the parts of a sum
    /// whose every multiple `c` divides divide part by part, and so do both sides of a choice
    /// between such sums. The sum must be the exact integer its bits are read as, never
    /// wrapping; then `(c × q + r) / c` is `q` plus `r`'s own quotient, and where truncation
    /// toward zero meets a remainder, a choice on the sum's sign. `None` when `a` isn't such a
    /// sum.
    fn divided(&mut self, a: Term, c: u128, signed_read: bool) -> Option<Term> {
        let width = self.width(a);
        if c < 2 || c > mask(width - 1) {
            return None;
        }
        if let Node::Ite(condition, then, otherwise) = *self.node(a) {
            let then = self.divided(then, c, signed_read)?;
            let otherwise = self.divided(otherwise, c, signed_read)?;
            return Some(self.ite(condition, then, otherwise));
        }
        let linear = self.linear_view(a);
        if linear.parts.is_empty() {
            return None;
        }
        let divisor = i128::try_from(c).ok()?;
        if !self.exact_in_window(&linear, width, signed_read) {
            return None;
        }
        let mut quotient = Linear::default();
        for (part, times) in &linear.parts {
            let times = signed(*times, width);
            if times % divisor != 0 {
                return None;
            }
            quotient.add_part(*part, ((times / divisor) as u128) & mask(width), width);
        }
        let constant = signed(linear.constant, width);
        let (whole, remainder) = (constant.div_euclid(divisor), constant.rem_euclid(divisor));
        quotient.constant = (whole as u128) & mask(width);
        let floor = self.linear_term(width, quotient);
        if !signed_read || remainder == 0 {
            return Some(floor);
        }
        // Below zero, truncation goes up from the floor.
        let zero = self.bv(width, 0);
        let negative = self.slt(a, zero);
        let one = self.bv(width, 1);
        let ceiling = self.add(floor, one);
        Some(self.ite(negative, ceiling, floor))
    }

    /// `a / c` with the factor they share cancelled, when `a` is a sum whose every multiple and
    /// constant share a factor with `c`, and its exact value never wraps: `(g × a') / (g × c')`
    /// is `a' / c'`, read signed or not, so a quotient a program takes of a value and one a
    /// formula takes of ten times it are one term.
    fn reduced(&mut self, a: Term, c: u128, signed_read: bool) -> Option<Term> {
        let width = self.width(a);
        if c < 2 || c > mask(width - 1) {
            return None;
        }
        let linear = self.linear_view(a);
        if linear.parts.is_empty() || !self.exact_in_window(&linear, width, signed_read) {
            return None;
        }
        let divisor = i128::try_from(c).ok()?;
        let constant = signed(linear.constant, width);
        let shared = linear
            .parts
            .values()
            .map(|times| signed(*times, width))
            .chain([constant])
            .fold(divisor, |g, value| {
                gcd(g, value.unsigned_abs().try_into().unwrap_or(1))
            });
        if shared < 2 {
            return None;
        }
        let mut smaller = Linear::default();
        for (part, times) in &linear.parts {
            let times = signed(*times, width) / shared;
            smaller.add_part(*part, (times as u128) & mask(width), width);
        }
        smaller.constant = ((constant / shared) as u128) & mask(width);
        let dividend = self.linear_term(width, smaller);
        let divisor = self.bv(width, (divisor / shared) as u128);
        Some(if signed_read {
            self.sdiv(dividend, divisor)
        } else {
            self.udiv(dividend, divisor)
        })
    }

    /// Whether a sum's exact value, read with its parts signed or unsigned, always lies in the
    /// window of `width`-bit values read signed or unsigned: then its bits are that value.
    fn exact_in_window(&self, linear: &Linear, width: u32, signed_read: bool) -> bool {
        let (low_bound, high_bound) = if signed_read {
            (-(1_i128 << (width - 1)), (1_i128 << (width - 1)) - 1)
        } else {
            (0, i128::try_from(mask(width)).unwrap_or(i128::MAX))
        };
        [false, true].into_iter().any(|signed_parts| {
            self.exact_bounds(linear, width, signed_parts)
                .is_some_and(|(low, high)| low >= low_bound && high <= high_bound)
        })
    }

    /// A division or remainder by `divisor` of a dividend wider than 64 bits that fits in 64,
    /// done at 64 bits and widened back: the same division a program does with 64-bit `div`
    /// and a formula does in 128 bits then meet as one term. Always 64 bits, never the
    /// smallest width that fits, so both sides narrow the same way.
    fn narrowed(
        &mut self,
        dividend: Term,
        divisor: u128,
        operation: fn(&mut Self, Term, Term) -> Term,
    ) -> Option<Term> {
        let width = self.width(dividend);
        if width <= 64 || divisor > mask(64) || self.range(dividend).1 > mask(64) {
            return None;
        }
        let low = self.extract(dividend, 63, 0);
        let divisor = self.bv(64, divisor);
        let result = operation(self, low, divisor);
        Some(self.zero_extend(result, width - 64))
    }

    /// Signed `a / b`, toward zero, as SMT-LIB defines it (also for zero and overflow).
    pub fn sdiv(&mut self, a: Term, b: Term) -> Term {
        let width = self.width(a);
        if let (Some(x), Some(y)) = (self.constant(a), self.constant(b)) {
            let (x, y) = (signed(x, width), signed(y, width));
            let quotient = if y == 0 {
                if x < 0 { 1 } else { -1 }
            } else {
                x.wrapping_div(y)
            };
            return self.bv_signed(width, quotient);
        }
        if self.constant(b) == Some(1) {
            return a;
        }
        if self.constant(b) == Some(0) {
            // 1 for a negative dividend and -1 otherwise, as SMT-LIB defines it.
            let zero = self.bv(width, 0);
            let negative = self.slt(a, zero);
            let (one, minus_one) = (self.bv(width, 1), self.bv(width, mask(width)));
            return self.ite(negative, one, minus_one);
        }
        // By a positive constant: a sum each of whose parts it divides divides part by part;
        // otherwise divide the magnitude, then put the sign back, toward zero.
        if let Some(c) = self.constant(b)
            && c > 0
            && c <= mask(width - 1)
        {
            if let Some(quotient) = self
                .divided(a, c, true)
                .or_else(|| self.reduced(a, c, true))
            {
                return quotient;
            }
            // A dividend that's never negative divides as unsigned.
            if self.signed_range(a).0 >= 0 {
                return self.udiv(a, b);
            }
            let zero = self.bv(width, 0);
            let negative = self.slt(a, zero);
            let negated = self.neg(a);
            let magnitude = self.ite(negative, negated, a);
            let quotient = self.udiv(magnitude, b);
            let flipped = self.neg(quotient);
            return self.ite(negative, flipped, quotient);
        }
        self.make(Node::Sdiv(a, b), Sort::Bv(width))
    }

    /// Signed `a % b`, with the sign of `a`, as SMT-LIB defines it.
    pub fn srem(&mut self, a: Term, b: Term) -> Term {
        let width = self.width(a);
        if let (Some(x), Some(y)) = (self.constant(a), self.constant(b)) {
            let (sx, sy) = (signed(x, width), signed(y, width));
            if sy == 0 {
                return a;
            }
            return self.bv_signed(width, sx.wrapping_rem(sy));
        }
        if self.constant(b) == Some(0) {
            return a;
        }
        if let Some(c) = self.constant(b)
            && c > 0
            && c <= mask(width - 1)
        {
            // A dividend that's never negative has the unsigned remainder, the one a
            // program's `div` gives.
            if self.signed_range(a).0 >= 0 {
                return self.urem(a, b);
            }
            let quotient = self.sdiv(a, b);
            let product = self.mul(quotient, b);
            return self.sub(a, product);
        }
        self.make(Node::Srem(a, b), Sort::Bv(width))
    }

    /// Bitwise and.
    pub fn and(&mut self, a: Term, b: Term) -> Term {
        let width = self.width(a);
        let (a, b) = self.ordered(a, b);
        if a == b {
            return a;
        }
        match (self.constant(a), self.constant(b)) {
            (Some(x), Some(y)) => return self.bv(width, x & y),
            (_, Some(0)) => return b,
            (_, Some(y)) if y == mask(width) => return a,
            // A value that's 0 or 1 is itself in its lowest bit.
            (_, Some(1)) if self.range(a).1 <= 1 => return a,
            _ => {}
        }
        if let Some(both) = self.bit_logic(a, b, Self::bool_and) {
            return both;
        }
        self.make(Node::And(a, b), Sort::Bv(width))
    }

    /// Bitwise or.
    pub fn or(&mut self, a: Term, b: Term) -> Term {
        let width = self.width(a);
        let (a, b) = self.ordered(a, b);
        if a == b {
            return a;
        }
        match (self.constant(a), self.constant(b)) {
            (Some(x), Some(y)) => return self.bv(width, x | y),
            (_, Some(0)) => return a,
            (_, Some(y)) if y == mask(width) => return b,
            _ => {}
        }
        if let Some(either) = self.bit_logic(a, b, Self::bool_or) {
            return either;
        }
        self.make(Node::Or(a, b), Sort::Bv(width))
    }

    /// Bitwise exclusive or.
    pub fn xor(&mut self, a: Term, b: Term) -> Term {
        let width = self.width(a);
        let (a, b) = self.ordered(a, b);
        if a == b {
            return self.bv(width, 0);
        }
        match (self.constant(a), self.constant(b)) {
            (Some(x), Some(y)) => return self.bv(width, x ^ y),
            (_, Some(0)) => return a,
            // So `xor eax, -1` and `not eax` are one term.
            (_, Some(ones)) if ones == mask(width) => return self.not(a),
            _ => {}
        }
        if let Some(one) = self.bit_logic(a, b, Self::bool_xor) {
            return one;
        }
        self.make(Node::Xor(a, b), Sort::Bv(width))
    }

    /// `op` of two values that are each 0 or 1, as setcc leaves them and a program combines
    /// them with `and`, `or` and `xor`: 1 where `op` of their conditions holds. So a flag a
    /// program keeps as a number and a formula's condition are one form, and a sum of such
    /// values is a count.
    fn bit_logic(
        &mut self,
        a: Term,
        b: Term,
        op: fn(&mut Self, Term, Term) -> Term,
    ) -> Option<Term> {
        if self.constant(a).is_some() && self.constant(b).is_some() {
            return None;
        }
        let x = self.bit_condition(a)?;
        let y = self.bit_condition(b)?;
        let width = self.width(a);
        let holds = op(self, x, y);
        let (one, zero) = (self.bv(width, 1), self.bv(width, 0));
        Some(self.ite(holds, one, zero))
    }

    /// The condition that `term`, a value that's only ever 0 or 1, is 1: a choice of 1 or 0
    /// says it, widened or not, and any other such value is 1 where it isn't 0.
    fn bit_condition(&mut self, term: Term) -> Option<Term> {
        if self.sort(term) == Sort::Bool || self.range(term).1 > 1 {
            return None;
        }
        if let Some(value) = self.constant(term) {
            return Some(self.boolean(value == 1));
        }
        match *self.node(term) {
            Node::Ite(condition, then, otherwise) => {
                match (self.constant(then), self.constant(otherwise)) {
                    (Some(1), Some(0)) => return Some(condition),
                    (Some(0), Some(1)) => return Some(self.bool_not(condition)),
                    _ => {}
                }
            }
            Node::ZeroExtend(inner, _) => return self.bit_condition(inner),
            _ => {}
        }
        let zero = self.bv(self.width(term), 0);
        let none = self.eq(term, zero);
        Some(self.bool_not(none))
    }

    /// Exactly one of `a` and `b` true.
    pub fn bool_xor(&mut self, a: Term, b: Term) -> Term {
        match (self.truth(a), self.truth(b)) {
            (Some(x), Some(y)) => self.boolean(x != y),
            (Some(false), _) => b,
            (_, Some(false)) => a,
            (Some(true), _) => self.bool_not(b),
            (_, Some(true)) => self.bool_not(a),
            _ => {
                let same = self.eq(a, b);
                self.bool_not(same)
            }
        }
    }

    /// Bitwise not.
    pub fn not(&mut self, a: Term) -> Term {
        let width = self.width(a);
        if let Some(x) = self.constant(a) {
            return self.bv(width, !x);
        }
        if let Node::Not(inner) = *self.node(a) {
            return inner;
        }
        self.make(Node::Not(a), Sort::Bv(width))
    }

    /// Two's-complement negation: of a sum, the sum of its parts negated.
    pub fn neg(&mut self, a: Term) -> Term {
        let width = self.width(a);
        if let Some(x) = self.constant(a) {
            return self.bv(width, x.wrapping_neg());
        }
        let mut linear = Linear::default();
        let part = self.linear(a);
        linear.add(&part, mask(width), width);
        self.linear_term(width, linear)
    }

    fn shift(&mut self, a: Term, b: Term, kind: fn(Term, Term) -> Node) -> Term {
        let width = self.width(a);
        if self.constant(b) == Some(0) {
            return a;
        }
        if let (Some(x), Some(y)) = (self.constant(a), self.constant(b)) {
            let probe = kind(a, b);
            let value = match probe {
                Node::Shl(..) if y >= u128::from(width) => 0,
                Node::Shl(..) => x << y,
                Node::Lshr(..) if y >= u128::from(width) => 0,
                Node::Lshr(..) => x >> y,
                _ => {
                    let shift = u32::try_from(y.min(u128::from(width - 1))).unwrap_or(width - 1);
                    unsigned(signed(x, width) >> shift, width)
                }
            };
            return self.bv(width, value);
        }
        self.make(kind(a, b), Sort::Bv(width))
    }

    /// Shift left. By a constant it's the lower bits with zeros below them, one form however a
    /// program shifts, whose single bits are the operand's own.
    pub fn shl(&mut self, a: Term, b: Term) -> Term {
        let width = self.width(a);
        if let Some(by) = self.constant(b)
            && by > 0
            && by < u128::from(width)
        {
            let by = u32::try_from(by).unwrap_or(width);
            let lower = self.extract(a, width - 1 - by, 0);
            let zeros = self.bv(by, 0);
            return self.concat(lower, zeros);
        }
        self.shift(a, b, Node::Shl)
    }

    /// Logical shift right. By a constant it's the upper bits widened back, one form however a
    /// program shifts.
    pub fn lshr(&mut self, a: Term, b: Term) -> Term {
        let width = self.width(a);
        // A wide value that fits in 64 bits shifts at 64 bits, as a program's does, so a
        // formula's 128-bit shift and the program's meet as one term.
        if width > 64
            && self.range(a).1 <= mask(64)
            && let Some(by) = self.constant(b).filter(|by| *by > 0 && *by < 64)
        {
            let low = self.extract(a, 63, 0);
            let by = self.bv(64, by);
            let shifted = self.lshr(low, by);
            return self.zero_extend(shifted, width - 64);
        }
        if let Some(by) = self.constant(b)
            && by > 0
            && by < u128::from(width)
        {
            let by = u32::try_from(by).unwrap_or(width);
            let upper = self.extract(a, width - 1, by);
            return self.zero_extend(upper, by);
        }
        self.shift(a, b, Node::Lshr)
    }

    /// Arithmetic shift right.
    pub fn ashr(&mut self, a: Term, b: Term) -> Term {
        self.shift(a, b, Node::Ashr)
    }

    /// `n` where `term`, `width` bits wide, is 1 shifted left by `n`, or that shift's low
    /// `width` bits.
    fn shifted_one(&self, term: Term, width: u32) -> Option<Term> {
        let shift = match *self.node(term) {
            Node::Extract(inner, high, 0) if high + 1 == width => inner,
            _ => term,
        };
        match *self.node(shift) {
            Node::Shl(one, n) if self.constant(one) == Some(1) => Some(n),
            _ => None,
        }
    }

    /// Bits `low` to `high` of `a`, inclusive.
    pub fn extract(&mut self, a: Term, high: u32, low: u32) -> Term {
        let width = self.width(a);
        let out = high - low + 1;
        if low == 0 && high + 1 == width {
            return a;
        }
        if let Some(x) = self.constant(a) {
            return self.bv(out, x >> low);
        }
        // A bit of the low mask of n bits, (1 << n) - 1 as bzhi builds one, is set where it's
        // below n, and every bit where n is past the width, since 1 shifted that far is 0 and
        // 0 - 1 is every bit: one form for each bit, however the low bits of a sum would fold.
        // The mask's low bits, the low bits of 1 << n less one, are the same bits: a byte of a
        // mask register is read so.
        if high == low
            && let Node::Add(shifted, ones) = *self.node(a)
            && self.constant(ones) == Some(mask(width))
            && let Some(n) = self.shifted_one(shifted, width)
        {
            let at = self.bv(self.width(n), u128::from(low));
            let below = self.ult(at, n);
            let (set, clear) = (self.bv(1, 1), self.bv(1, 0));
            return self.ite(below, set, clear);
        }
        match *self.node(a) {
            Node::Extract(inner, _, inner_low) => {
                return self.extract(inner, high + inner_low, low + inner_low);
            }
            // A count's low bits are the count when it fits them.
            Node::Count(number, _)
                if low == 0 && self.counts[number as usize].len as u128 <= mask(out) =>
            {
                return self.make(Node::Count(number, out), Sort::Bv(out));
            }
            // x + x is x shifted left once: its bits are x's, one place up.
            Node::Add(x, y) if x == y => {
                let one = self.bv(width, 1);
                let shifted = self.shl(x, one);
                return self.extract(shifted, high, low);
            }
            Node::Concat(upper, lower) => {
                let lower_width = self.width(lower);
                if high < lower_width {
                    return self.extract(lower, high, low);
                }
                if low >= lower_width {
                    return self.extract(upper, high - lower_width, low - lower_width);
                }
            }
            Node::ZeroExtend(inner, _) => {
                let inner_width = self.width(inner);
                if high < inner_width {
                    return self.extract(inner, high, low);
                }
                if low >= inner_width {
                    return self.bv(out, 0);
                }
                let part = self.extract(inner, inner_width - 1, low);
                return self.zero_extend(part, high + 1 - inner_width);
            }
            Node::SignExtend(inner, _) => {
                let inner_width = self.width(inner);
                if high < inner_width {
                    return self.extract(inner, high, low);
                }
            }
            // The low bits of these depend only on the low bits of their operands.
            Node::Add(x, y) if low == 0 => {
                let (x, y) = (self.extract(x, high, 0), self.extract(y, high, 0));
                return self.add(x, y);
            }
            Node::Mul(x, y) if low == 0 => {
                let (x, y) = (self.extract(x, high, 0), self.extract(y, high, 0));
                return self.mul(x, y);
            }
            Node::Neg(x) if low == 0 => {
                let x = self.extract(x, high, 0);
                return self.neg(x);
            }
            Node::And(x, y) => {
                let (x, y) = (self.extract(x, high, low), self.extract(y, high, low));
                return self.and(x, y);
            }
            Node::Or(x, y) => {
                let (x, y) = (self.extract(x, high, low), self.extract(y, high, low));
                return self.or(x, y);
            }
            Node::Xor(x, y) => {
                let (x, y) = (self.extract(x, high, low), self.extract(y, high, low));
                return self.xor(x, y);
            }
            Node::Not(x) => {
                let x = self.extract(x, high, low);
                return self.not(x);
            }
            Node::Ite(condition, then, otherwise) => {
                let (then, otherwise) = (
                    self.extract(then, high, low),
                    self.extract(otherwise, high, low),
                );
                return self.ite(condition, then, otherwise);
            }
            Node::Mul(factor, by) if high + 1 == width => {
                if let Some(quotient) = self.reciprocal(factor, by, low, out) {
                    return quotient;
                }
            }
            _ => {}
        }
        self.make(Node::Extract(a, high, low), Sort::Bv(out))
    }

    /// The top bits of `zext(x) × m` from bit `shift` up, as a compiler divides by a constant:
    /// they're `x / c` for `c = ⌈2^shift / m⌉` when, with `e = m·c − 2^shift`, every x the range
    /// allows has `x·e < 2^shift`, so the rounding never reaches the next whole number. `out` is
    /// the width wanted.
    fn reciprocal(&mut self, factor: Term, by: Term, shift: u32, out: u32) -> Option<Term> {
        let Node::ZeroExtend(x, _) = *self.node(factor) else {
            return None;
        };
        let multiplier = self.constant(by).filter(|m| *m > 0)?;
        let width = self.width(factor);
        let largest = self.range(x).1;
        // The product must never wrap, or its top bits aren't the quotient.
        if largest.checked_mul(multiplier)? > mask(width) || shift >= 127 {
            return None;
        }
        let power = 1_u128 << shift;
        let divisor = power.div_ceil(multiplier);
        let excess = multiplier.checked_mul(divisor)? - power;
        if largest.checked_mul(excess)? >= power {
            return None;
        }
        let narrow = self.width(x);
        let quotient = if divisor > mask(narrow) {
            self.bv(narrow, 0)
        } else {
            let divisor = self.bv(narrow, divisor);
            self.udiv(x, divisor)
        };
        Some(if out >= narrow {
            self.zero_extend(quotient, out - narrow)
        } else {
            self.extract(quotient, out - 1, 0)
        })
    }

    /// `a` widened by `by` zero bits.
    pub fn zero_extend(&mut self, a: Term, by: u32) -> Term {
        if by == 0 {
            return a;
        }
        let narrow = self.width(a);
        let width = narrow + by;
        if let Some(x) = self.constant(a) {
            return self.bv(width, x);
        }
        match *self.node(a) {
            Node::ZeroExtend(inner, inner_by) => return self.zero_extend(inner, inner_by + by),
            Node::Count(number, _) => {
                return self.make(Node::Count(number, width), Sort::Bv(width));
            }
            // The low bits of a value that fits in them, widened back, are the value, widened
            // as far: a 64-bit quotient a program narrowed to 63 bits and a formula widened to
            // 128 is the quotient itself, so both divide it further at 64 bits.
            Node::Extract(inner, high, 0)
                if self.width(inner) <= width && self.range(inner).1 <= mask(high + 1) =>
            {
                let more = width - self.width(inner);
                return self.zero_extend(inner, more);
            }
            // A sum or product that never wraps widens part by part, so the same arithmetic
            // done in different widths ends up the same term.
            Node::Add(..) | Node::Neg(_) => {
                if let Some(widened) = self.widened_sum(a, by, false) {
                    return widened;
                }
            }
            Node::Mul(_, factor) if self.constant(factor).is_some() => {
                if let Some(widened) = self.widened_sum(a, by, false) {
                    return widened;
                }
            }
            Node::Mul(x, y)
                if self
                    .range(x)
                    .1
                    .checked_mul(self.range(y).1)
                    .is_some_and(|high| high <= mask(narrow)) =>
            {
                let (x, y) = (self.zero_extend(x, by), self.zero_extend(y, by));
                return self.mul(x, y);
            }
            // A choice widens branch by branch, each knowing which way the choice went.
            Node::Ite(condition, then, otherwise) => {
                let then = self.zero_extend_given(condition, true, then, by);
                let otherwise = self.zero_extend_given(condition, false, otherwise, by);
                return self.ite(condition, then, otherwise);
            }
            _ => {}
        }

        self.make(Node::ZeroExtend(a, by), Sort::Bv(width))
    }

    /// `value` zero-extended by `by`, where `condition` is `holds`. When that makes `value` never
    /// negative, as `0 < x` does for `x` and `x <= 0` does for `-x`, widening with zeros is
    /// widening with the sign, which simplifies the way signed arithmetic does.
    fn zero_extend_given(&mut self, condition: Term, holds: bool, value: Term, by: u32) -> Term {
        if self.non_negative_given(condition, holds, value) {
            self.sign_extend(value, by)
        } else {
            self.zero_extend(value, by)
        }
    }

    /// Whether `condition` being `holds` makes `value`, read as signed, at least zero.
    fn non_negative_given(&mut self, condition: Term, holds: bool, value: Term) -> bool {
        let (condition, holds) = match *self.node(condition) {
            Node::BoolNot(inner) => (inner, !holds),
            _ => (condition, holds),
        };
        let Node::Slt(left, right) = *self.node(condition) else {
            return false;
        };
        let width = self.width(value);
        let wide = if width < self.width(left) {
            self.sign_extend(value, self.width(left) - width)
        } else {
            value
        };
        if self.width(wide) != self.width(left) {
            return false;
        }
        let smallest = if self.width(left) >= 128 {
            i128::MIN
        } else {
            -(1_i128 << (self.width(left) - 1))
        };
        // The value itself, or the negation of a value that isn't the most negative one.
        let compared = |pool: &mut Self, other: Term, negated: bool| {
            if negated {
                pool.signed_range(other).0 > smallest && pool.neg(other) == wide
            } else {
                other == wide
            }
        };
        match (self.constant(left), self.constant(right), holds) {
            // 0 < v makes v positive; not 0 < v makes v at most zero, so -v at least zero.
            (Some(0), None, true) => compared(self, right, false),
            (Some(0), None, false) => compared(self, right, true),
            // not v < 0 makes v at least zero; v < 0 makes -v positive.
            (None, Some(0), false) => compared(self, left, false),
            (None, Some(0), true) => compared(self, left, true),
            _ => false,
        }
    }

    /// `a` widened by `by` copies of its sign bit.
    pub fn sign_extend(&mut self, a: Term, by: u32) -> Term {
        if by == 0 {
            return a;
        }
        let narrow = self.width(a);
        if let Some(pushed) = self.sign_push(a, by) {
            return pushed;
        }
        // A value that's never negative widens with zeros.
        if self.range(a).1 <= mask(narrow - 1) {
            return self.zero_extend(a, by);
        }
        let width = narrow + by;
        if let Some(x) = self.constant(a) {
            return self.bv_signed(width, signed(x, narrow));
        }
        match *self.node(a) {
            Node::SignExtend(inner, inner_by) => return self.sign_extend(inner, inner_by + by),
            // A choice widens branch by branch.
            Node::Ite(condition, then, otherwise) => {
                let (then, otherwise) =
                    (self.sign_extend(then, by), self.sign_extend(otherwise, by));
                return self.ite(condition, then, otherwise);
            }
            _ => {}
        }
        self.make(Node::SignExtend(a, by), Sort::Bv(width))
    }

    /// `a` widened to 128 bits, when widening undoes the narrow arithmetic all the way down: a
    /// sum, product or negation pushed to widened leaves, a constant, or a choice between such.
    /// The program's 64-bit arithmetic and the formula's 128-bit arithmetic then meet as one term.
    fn lifted(&mut self, a: Term, signed: bool) -> Option<Term> {
        let width = self.width(a);
        if width >= 128 {
            return None;
        }
        let wide = if signed {
            self.sign_extend(a, 128 - width)
        } else {
            self.zero_extend(a, 128 - width)
        };
        let extended = |pool: &Self, term: Term| {
            matches!(pool.node(term), Node::SignExtend(..) | Node::ZeroExtend(..))
        };
        match *self.node(wide) {
            Node::SignExtend(..) | Node::ZeroExtend(..) => None,
            Node::Ite(_, then, otherwise) if extended(self, then) || extended(self, otherwise) => {
                None
            }
            _ => Some(wide),
        }
    }

    /// Both of `a` and `b` lifted to 128 bits, when both lift and neither is a constant: then a
    /// comparison of them is the same comparison of the wide values.
    fn lifted_pair(&mut self, a: Term, b: Term, signed: bool) -> Option<(Term, Term)> {
        if self.sort(a) == Sort::Bool || self.constant(a).is_some() && self.constant(b).is_some() {
            return None;
        }
        let wide_a = self.lifted(a, signed)?;
        let wide_b = self.lifted(b, signed)?;
        Some((wide_a, wide_b))
    }

    /// `a` sign-extended part by part, when it's a sum, negation or product that never
    /// overflows as signed numbers, so the same signed arithmetic in different widths ends up
    /// the same term.
    fn sign_push(&mut self, a: Term, by: u32) -> Option<Term> {
        let narrow = self.width(a);
        let fits = |pool: &Self, term: Term| {
            let (low, high) = pool.signed_range(term);
            let minimum = -(1_i128 << (narrow - 1).min(126));
            low >= minimum && high < (1_i128 << (narrow - 1).min(126))
        };
        match *self.node(a) {
            Node::Add(..) | Node::Neg(_) => return self.widened_sum(a, by, true),
            Node::Mul(_, factor) if self.constant(factor).is_some() => {
                return self.widened_sum(a, by, true);
            }
            Node::Mul(x, y)
                if narrow < 128 && fits(self, a) && {
                    let ((xl, xh), (yl, yh)) = (self.signed_range(x), self.signed_range(y));
                    [
                        xl.checked_mul(yl),
                        xl.checked_mul(yh),
                        xh.checked_mul(yl),
                        xh.checked_mul(yh),
                    ]
                    .iter()
                    .all(|corner| {
                        corner.is_some_and(|value| {
                            value >= -(1_i128 << (narrow - 1)) && value < (1_i128 << (narrow - 1))
                        })
                    })
                } =>
            {
                let (x, y) = (self.sign_extend(x, by), self.sign_extend(y, by));
                return Some(self.mul(x, y));
            }
            _ => {}
        }
        None
    }

    /// `high` above `low`.
    pub fn concat(&mut self, high: Term, low: Term) -> Term {
        let (high_width, low_width) = (self.width(high), self.width(low));
        let width = high_width + low_width;
        if let (Some(x), Some(y)) = (self.constant(high), self.constant(low)) {
            return self.bv(width, (x << low_width) | y);
        }
        if self.constant(high) == Some(0) {
            return self.zero_extend(low, high_width);
        }
        // Copies of a value's sign above it, as cqo and cdq make for idiv, sign-extend it.
        if let Node::Ashr(inner, amount) = *self.node(high)
            && inner == low
            && high_width == low_width
            && self.constant(amount) == Some(u128::from(low_width - 1))
        {
            return self.sign_extend(low, high_width);
        }
        // Neighboring parts of one term join back into it.
        if let (
            Node::Extract(upper, upper_high, upper_low),
            Node::Extract(lower, lower_high, lower_low),
        ) = (self.node(high).clone(), self.node(low).clone())
            && upper == lower
            && upper_low == lower_high + 1
        {
            return self.extract(upper, upper_high, lower_low);
        }
        // A constant that the term's range fixes those bits to, next to one of the term's
        // parts, is that part too: a value saved to memory and loaded back is the value, even
        // where its range had made some of its bytes constants.
        if let (Some(c), Node::Extract(inner, lower_high, lower_low)) =
            (self.constant(high), self.node(low).clone())
            && lower_high + high_width < self.width(inner)
        {
            let fixed = self.extract(inner, lower_high + high_width, lower_high + 1);
            if self.constant(fixed) == Some(c) {
                return self.extract(inner, lower_high + high_width, lower_low);
            }
        }
        if let (Node::Extract(inner, upper_high, upper_low), Some(c)) =
            (self.node(high).clone(), self.constant(low))
            && upper_low >= low_width
        {
            let fixed = self.extract(inner, upper_low - 1, upper_low - low_width);
            if self.constant(fixed) == Some(c) {
                return self.extract(inner, upper_high, upper_low - low_width);
            }
        }
        self.make(Node::Concat(high, low), Sort::Bv(width))
    }

    /// `then` if `condition`, else `otherwise`.
    pub fn ite(&mut self, condition: Term, then: Term, otherwise: Term) -> Term {
        match self.truth(condition) {
            Some(true) => return then,
            Some(false) => return otherwise,
            None => {}
        }
        // A choice inside a choice whose condition settles it: where s < 0, so is s - 1.
        let inner = |pool: &Self, side: Term, holds: bool| match *pool.node(side) {
            Node::Ite(inside, chosen, other) => match pool.implied(condition, holds, inside) {
                Some(true) => chosen,
                Some(false) => other,
                None => side,
            },
            _ => side,
        };
        let (then, otherwise) = (inner(self, then, true), inner(self, otherwise, false));
        if then == otherwise {
            return then;
        }
        // A negated choice is the choice the other way round.
        if let Node::BoolNot(inner) = *self.node(condition) {
            return self.ite(inner, otherwise, then);
        }
        // The size of x has one form however it was chosen: ite(0 < x, x, -x) is
        // ite(x < 0, -x, x), even for the most negative x, which both leave as it is.
        if let Node::Slt(left, right) = *self.node(condition)
            && self.constant(left) == Some(0)
            && then == right
            && self.negates_value(otherwise, right)
        {
            let negative = self.slt(right, left);
            return self.ite(negative, otherwise, then);
        }
        // The size of x is the size of -x: of the two, the one whose first part counts
        // positively is the one written, so |a - b| and |b - a| are one term.
        if let Node::Slt(left, right) = *self.node(condition)
            && self.constant(right) == Some(0)
            && otherwise == left
            && self.negates_value(then, left)
            && self
                .linear_view(left)
                .parts
                .first_key_value()
                .is_some_and(|(_, times)| signed(*times, self.width(left)) < 0)
            // A multiple that's its own negation, the top bit alone, counts negatively both
            // ways round: written either way, it would turn forever.
            && self
                .linear_view(then)
                .parts
                .first_key_value()
                .is_some_and(|(_, times)| signed(*times, self.width(then)) > 0)
        {
            let zero = self.bv(self.width(then), 0);
            let negative = self.slt(then, zero);
            return self.ite(negative, otherwise, then);
        }
        let sort = self.sort(then);
        if sort == Sort::Bool {
            // A true/false choice is plain logic.
            match (self.truth(then), self.truth(otherwise)) {
                (Some(true), Some(false)) => return condition,
                (Some(false), Some(true)) => return self.bool_not(condition),
                _ => {}
            }
        }
        if let Some(count) = self.chosen_count(condition, then, otherwise) {
            return count;
        }
        self.make(Node::Ite(condition, then, otherwise), sort)
    }

    /// A choice between two counts one of which counts some of the other's conditions: the
    /// smaller count, and each of the larger one's other conditions where the choice picks it.
    /// So the counts of a loop whose exits were merged, each a count of what was read before
    /// leaving, are one count of each part where the loop hadn't left by then.
    fn chosen_count(&mut self, condition: Term, then: Term, otherwise: Term) -> Option<Term> {
        if self.sort(then) == Sort::Bool || !self.is_counted(then) || !self.is_counted(otherwise) {
            return None;
        }
        let width = self.width(then);
        let (mut picked, mut other) = (self.counted(then)?, self.counted(otherwise)?);
        picked.sort_unstable();
        other.sort_unstable();
        // Which is the smaller, and where the larger is chosen.
        let (smaller, larger, chosen, small) = if within_multiset(&picked, &other) {
            (picked, other, self.bool_not(condition), then)
        } else if within_multiset(&other, &picked) {
            (other, picked, condition, otherwise)
        } else {
            return None;
        };
        let mut more = Vec::new();
        let mut rest = smaller.iter().peekable();
        for part in larger {
            if rest.peek() == Some(&&part) {
                rest.next();
                continue;
            }
            more.push(self.bool_and(part, chosen));
        }
        if (smaller.len() + more.len()) as u128 > mask(width) {
            return None;
        }
        // The smaller count's conditions are kept once, extended by the larger's others.
        match self.count_number(small) {
            Some(base) => Some(self.count_with(width, Some(base), &more)),
            None => {
                let mut parts = smaller;
                parts.extend(more);
                Some(self.count(width, &parts))
            }
        }
    }

    /// `a = b`.
    pub fn eq(&mut self, a: Term, b: Term) -> Term {
        if a == b {
            return self.boolean(true);
        }
        let (a, b) = self.ordered(a, b);
        let ((al, ah), (bl, bh)) = (self.range(a), self.range(b));
        if ah < bl || bh < al {
            return self.boolean(false);
        }
        // A value's sign shifted down over all of it, as AArch64 compares a product's high half
        // with its low half's: none where the value is at least 0, all ones where it's below,
        // which its signed range settles.
        for (shifted, other) in [(a, b), (b, a)] {
            if let Node::Ashr(value, amount) = *self.node(shifted)
                && self.constant(amount) == Some(u128::from(self.width(value) - 1))
                && let Some(fill) = self.constant(other)
            {
                let width = self.width(value);
                let zero = self.bv(width, 0);
                let negative = self.slt(value, zero);
                if fill == 0 {
                    return self.bool_not(negative);
                }
                if fill == mask(width) {
                    return negative;
                }
                return self.boolean(false);
            }
        }
        // A quotient is 0 exactly when the dividend is below the divisor: a program counting
        // digits by dividing by 10 until nothing is left and a formula comparing with powers of
        // 10 then ask the same.
        for (quotient, other) in [(a, b), (b, a)] {
            if self.constant(other) == Some(0)
                && let Node::Udiv(dividend, divisor) = *self.node(quotient)
                && self.constant(divisor).is_some_and(|c| c > 0)
            {
                return self.ult(dividend, divisor);
            }
        }
        // Read signed, too: a small number below zero is never the most negative one.
        if self.sort(a) != Sort::Bool {
            let ((al, ah), (bl, bh)) = (self.signed_range(a), self.signed_range(b));
            if ah < bl || bh < al {
                return self.boolean(false);
            }
        }
        // The top bit of v set is v < 0, which comparisons below can lift.
        for (bit, other) in [(a, b), (b, a)] {
            if let (Node::Extract(v, high, low), Some(c)) =
                (self.node(bit).clone(), self.constant(other))
                && high == low
                && high + 1 == self.width(v)
            {
                let zero = self.bv(high + 1, 0);
                let negative = self.slt(v, zero);
                return if c == 1 {
                    negative
                } else {
                    self.bool_not(negative)
                };
            }
        }
        // Joined parts equal a value when each equals its own bits of it: a mask of byte tests
        // is 0 when none of the tests holds, and a port the program put together byte by byte
        // is the number it came from when each byte is that number's.
        for (joined, other) in [(a, b), (b, a)] {
            if let Node::Concat(high, low) = *self.node(joined)
                && !matches!(self.node(other), Node::Concat(..))
            {
                let (width, low_width) = (self.width(joined), self.width(low));
                let other_high = self.extract(other, width - 1, low_width);
                let other_low = self.extract(other, low_width - 1, 0);
                let (high_same, low_same) = (self.eq(high, other_high), self.eq(low, other_low));
                return self.bool_and(high_same, low_same);
            }
        }
        // A choice between constants, compared with one, is the condition that picks it.
        for (choice, other) in [(a, b), (b, a)] {
            if let (Node::Ite(condition, then, otherwise), Some(k)) =
                (self.node(choice).clone(), self.constant(other))
                && let (Some(x), Some(y)) = (self.constant(then), self.constant(otherwise))
            {
                return match (x == k, y == k) {
                    (true, true) => self.boolean(true),
                    (true, false) => condition,
                    (false, true) => self.bool_not(condition),
                    (false, false) => self.boolean(false),
                };
            }
        }
        // A value that's only ever 0 or 1 is 1 where it isn't 0: one condition however a
        // program or a formula asks it.
        for (bit, other) in [(a, b), (b, a)] {
            if self.width(bit) > 1
                && self.constant(other) == Some(1)
                && self.constant(bit).is_none()
                && self.range(bit).1 <= 1
            {
                let zero = self.bv(self.width(bit), 0);
                let none = self.eq(bit, zero);
                return self.bool_not(none);
            }
        }
        // A 1-bit and, or or not of bits is 1 where the and, or or not of their conditions
        // holds: a mask's bit is the test that made it.
        if self.width(a) == 1
            && let Some(k) = self.constant(b)
            && let Some(truth) = self.bit_truth(a)
        {
            return if k == 1 { truth } else { self.bool_not(truth) };
        }
        // -x = c is x = -c.
        if let (Node::Neg(x), Some(c)) = (self.node(a).clone(), self.constant(b)) {
            let width = self.width(b);
            let negated = self.bv(width, c.wrapping_neg());
            return self.eq(x, negated);
        }
        // x / c = k is c·k ≤ x < c·(k + 1): what a digit loop's end comes down to.
        if let (Node::Udiv(x, divisor), Some(k)) = (self.node(a).clone(), self.constant(b))
            && let Some(c) = self.constant(divisor).filter(|c| *c > 0)
        {
            let width = self.width(x);
            let Some(from) = c.checked_mul(k).filter(|from| *from <= mask(width)) else {
                return self.boolean(false);
            };
            let from = self.bv(width, from);
            let at_least = self.ule(from, x);
            return match c.checked_mul(k + 1).filter(|to| *to <= mask(width)) {
                Some(to) => {
                    let to = self.bv(width, to);
                    let below = self.ult(x, to);
                    self.bool_and(at_least, below)
                }
                None => at_least,
            };
        }
        if let Some((a, b)) = self.narrow_pair(a, b) {
            return self.eq(a, b);
        }
        if let (Some(x), Some(y)) = (self.constant(a), self.constant(b)) {
            return self.boolean(x == y);
        }
        if let (Some(x), Some(y)) = (self.truth(a), self.truth(b)) {
            return self.boolean(x == y);
        }
        if let Some((a, b)) = self.lifted_pair(a, b, true) {
            return self.eq(a, b);
        }
        self.make(Node::Eq(a, b), Sort::Bool)
    }

    /// The condition that the 1-bit `bit` is 1, when it's an and, or or not of bits.
    fn bit_truth(&mut self, bit: Term) -> Option<Term> {
        Some(match *self.node(bit) {
            Node::And(x, y) => {
                let (x, y) = (self.bit_is_set(x, 0), self.bit_is_set(y, 0));
                self.bool_and(x, y)
            }
            Node::Or(x, y) => {
                let (x, y) = (self.bit_is_set(x, 0), self.bit_is_set(y, 0));
                self.bool_or(x, y)
            }
            Node::Not(x) => {
                let x = self.bit_is_set(x, 0);
                self.bool_not(x)
            }
            _ => return None,
        })
    }

    /// The condition that bit `bit` of `value` is 1.
    pub fn bit_is_set(&mut self, value: Term, bit: u32) -> Term {
        let part = self.extract(value, bit, bit);
        let one = self.bv(1, 1);
        self.eq(part, one)
    }

    /// Unsigned `a < b`.
    pub fn ult(&mut self, a: Term, b: Term) -> Term {
        if a == b {
            return self.boolean(false);
        }
        let ((al, ah), (bl, bh)) = (self.range(a), self.range(b));
        if ah < bl {
            return self.boolean(true);
        }
        if al >= bh {
            return self.boolean(false);
        }
        match (self.constant(a), self.constant(b)) {
            (Some(x), Some(y)) => return self.boolean(x < y),
            (_, Some(0)) => return self.boolean(false),
            _ => {}
        }
        // x / c < k is x < c·k, and k < x / c is c·(k + 1) ≤ x.
        if let (Node::Udiv(x, divisor), Some(k)) = (self.node(a).clone(), self.constant(b))
            && let Some(c) = self.constant(divisor).filter(|c| *c > 0)
        {
            let width = self.width(x);
            return match c.checked_mul(k).filter(|bound| *bound <= mask(width)) {
                Some(bound) => {
                    let bound = self.bv(width, bound);
                    self.ult(x, bound)
                }
                None => self.boolean(true),
            };
        }
        if let (Some(k), Node::Udiv(x, divisor)) = (self.constant(a), self.node(b).clone())
            && let Some(c) = self.constant(divisor).filter(|c| *c > 0)
        {
            let width = self.width(x);
            return match k
                .checked_add(1)
                .and_then(|next| c.checked_mul(next))
                .filter(|bound| *bound <= mask(width))
            {
                Some(bound) => {
                    let bound = self.bv(width, bound);
                    self.ule(bound, x)
                }
                None => self.boolean(false),
            };
        }
        // The top bits of x from bit s on are x / 2^s: x >> s < k is x < k·2^s, and
        // k < x >> s is (k + 1)·2^s ≤ x, as for a quotient, so a value halved by a shift and
        // one divided compare alike.
        if let (Node::Extract(x, high, low), Some(k)) = (self.node(a).clone(), self.constant(b))
            && low > 0
            && high + 1 == self.width(x)
        {
            let width = self.width(x);
            return match k
                .checked_mul(1_u128 << low)
                .filter(|bound| *bound <= mask(width))
            {
                Some(bound) => {
                    let bound = self.bv(width, bound);
                    self.ult(x, bound)
                }
                None => self.boolean(true),
            };
        }
        if let (Some(k), Node::Extract(x, high, low)) = (self.constant(a), self.node(b).clone())
            && low > 0
            && high + 1 == self.width(x)
        {
            let width = self.width(x);
            return match k
                .checked_add(1)
                .and_then(|next| next.checked_mul(1_u128 << low))
                .filter(|bound| *bound <= mask(width))
            {
                Some(bound) => {
                    let bound = self.bv(width, bound);
                    self.ule(bound, x)
                }
                None => self.boolean(false),
            };
        }
        if let Some((a, b)) = self.widened_pair(a, b) {
            return self.ult(a, b);
        }
        if let Some((a, b)) = self.narrow_pair(a, b) {
            return self.ult(a, b);
        }
        if let Some((a, b)) = self.lifted_pair(a, b, false) {
            return self.ult(a, b);
        }
        self.make(Node::Ult(a, b), Sort::Bool)
    }

    /// `a` and `b` at a wider width, when one is the low bits of a value that fits in them and
    /// the other a constant: comparing the value itself is the same comparison. So a vector's
    /// lanes compared with the low byte of a count of bytes left, as NEON's `dup` of a w
    /// register takes it, compare with the count as a formula's guards do.
    fn widened_pair(&mut self, a: Term, b: Term) -> Option<(Term, Term)> {
        let low_bits_of = |pool: &Self, term: Term| match *pool.node(term) {
            Node::Extract(inner, high, 0)
                if high + 1 < pool.width(inner) && pool.range(inner).1 <= mask(high + 1) =>
            {
                Some(inner)
            }
            _ => None,
        };
        match (low_bits_of(self, a), low_bits_of(self, b)) {
            (None, Some(inner)) => {
                let k = self.constant(a)?;
                let width = self.width(inner);
                Some((self.bv(width, k), inner))
            }
            (Some(inner), None) => {
                let k = self.constant(b)?;
                let width = self.width(inner);
                Some((inner, self.bv(width, k)))
            }
            _ => None,
        }
    }

    /// `a` and `b` at a narrower width, when one is a zero-extended value and the other a
    /// constant that fits it or another zero-extended value: comparing them there is the same
    /// comparison, and the program's and the formula's widths then meet.
    fn narrow_pair(&mut self, a: Term, b: Term) -> Option<(Term, Term)> {
        let width = self.width(a);
        let inner = |pool: &Self, term: Term| match *pool.node(term) {
            Node::ZeroExtend(x, by) => Some((x, width - by)),
            _ => None,
        };
        match (inner(self, a), inner(self, b)) {
            (Some((x, narrow)), None) => {
                let c = self.constant(b).filter(|c| *c <= mask(narrow))?;
                Some((x, self.bv(narrow, c)))
            }
            (None, Some((y, narrow))) => {
                let c = self.constant(a).filter(|c| *c <= mask(narrow))?;
                Some((self.bv(narrow, c), y))
            }
            (Some((x, from)), Some((y, to))) => {
                let narrow = from.max(to);
                let x = self.zero_extend(x, narrow - from);
                let y = self.zero_extend(y, narrow - to);
                Some((x, y))
            }
            (None, None) => None,
        }
    }

    /// Unsigned `a <= b`.
    pub fn ule(&mut self, a: Term, b: Term) -> Term {
        let greater = self.ult(b, a);
        self.bool_not(greater)
    }

    /// Signed `a < b`.
    pub fn slt(&mut self, a: Term, b: Term) -> Term {
        if a == b {
            return self.boolean(false);
        }
        let width = self.width(a);
        // Between values that are never negative, signed and unsigned agree.
        if self.range(a).1 <= mask(width - 1) && self.range(b).1 <= mask(width - 1) {
            return self.ult(a, b);
        }
        if let (Some(x), Some(y)) = (self.constant(a), self.constant(b)) {
            return self.boolean(signed(x, width) < signed(y, width));
        }
        let ((al, ah), (bl, bh)) = (self.signed_range(a), self.signed_range(b));
        if ah < bl {
            return self.boolean(true);
        }
        if al >= bh {
            return self.boolean(false);
        }
        // -x < c is -c < x, and c < -x is x < -c, when neither x nor c is the most negative
        // value, which negating leaves as it is.
        let minimum = -(1_i128 << (width - 1));
        for (negated, other, flipped) in [(a, b, false), (b, a, true)] {
            if let (Node::Neg(x), Some(c)) = (self.node(negated).clone(), self.constant(other))
                && self.signed_range(x).0 > minimum
                && signed(c, width) > minimum
            {
                let minus_c = self.bv_signed(width, -signed(c, width));
                return if flipped {
                    self.slt(x, minus_c)
                } else {
                    self.slt(minus_c, x)
                };
            }
        }
        if let Some((a, b)) = self.lifted_pair(a, b, true) {
            return self.slt(a, b);
        }
        self.make(Node::Slt(a, b), Sort::Bool)
    }

    /// Signed `a <= b`.
    pub fn sle(&mut self, a: Term, b: Term) -> Term {
        let greater = self.slt(b, a);
        self.bool_not(greater)
    }

    /// Both true.
    pub fn bool_and(&mut self, a: Term, b: Term) -> Term {
        match (self.truth(a), self.truth(b)) {
            (Some(false), _) | (_, Some(false)) => return self.boolean(false),
            (Some(true), _) => return b,
            (_, Some(true)) => return a,
            _ => {}
        }
        if a == b {
            return a;
        }
        // x and not x.
        if self.negates(a, b) {
            return self.boolean(false);
        }
        // Two comparisons of one value with constants: the one that says more where it holds
        // wherever the other does, as a loop still running after its kth time round ran every
        // one before; nothing where they can't both hold.
        if let (Some((x, x_low, x_high)), Some((y, y_low, y_high))) =
            (self.interval(a, true), self.interval(b, true))
            && x == y
        {
            if x_low >= y_low && x_high <= y_high {
                return a;
            }
            if y_low >= x_low && y_high <= x_high {
                return b;
            }
            if x_high < y_low || y_high < x_low {
                return self.boolean(false);
            }
        }
        // A comparison joining a conjunction that holds one of the same value joins that one,
        // so the conditions a merged loop's exits put on each part come to one.
        for (whole, comparison) in [(a, b), (b, a)] {
            if let Node::BoolAnd(x, y) = *self.node(whole)
                && let Some((subject, ..)) = self.interval(comparison, true)
            {
                for (kept, joined) in [(x, y), (y, x)] {
                    if self
                        .interval(joined, true)
                        .is_some_and(|(other, ..)| other == subject)
                    {
                        let combined = self.bool_and(joined, comparison);
                        return self.bool_and(kept, combined);
                    }
                }
            }
        }
        // a and not (a and c) is a and not c: a matcher's state after a byte that ended no
        // match, written either way.
        for (first, second) in [(a, b), (b, a)] {
            if let Node::BoolNot(inner) = *self.node(second)
                && let Node::BoolAnd(x, y) = *self.node(inner)
                && (x == first || y == first)
            {
                let other = if x == first { y } else { x };
                let not_other = self.bool_not(other);
                return self.bool_and(first, not_other);
            }
        }
        let (a, b) = self.ordered(a, b);
        self.make(Node::BoolAnd(a, b), Sort::Bool)
    }

    /// Whether `a` and `b` are the same value by their structure alone, sums compared as sets
    /// of parts whatever their grouping and order: the same arithmetic built by a program and by
    /// a formula in different steps. Never true for terms that could differ; false when unsure.
    #[must_use]
    pub fn equivalent(&self, a: Term, b: Term) -> bool {
        self.equivalent_in(a, b, &mut HashMap::new())
    }

    fn equivalent_in(&self, a: Term, b: Term, known: &mut HashMap<(Term, Term), bool>) -> bool {
        if a == b {
            return true;
        }
        if self.sort(a) != self.sort(b) {
            return false;
        }
        if let Some(answer) = known.get(&(a, b)) {
            return *answer;
        }
        known.insert((a, b), false);
        let answer = match (self.node(a), self.node(b)) {
            (Node::Add(..), Node::Add(..)) => {
                let parts = |term: Term| {
                    let (mut parts, mut constant, mut pending) = (Vec::new(), 0_u128, vec![term]);
                    while let Some(current) = pending.pop() {
                        match *self.node(current) {
                            Node::Add(x, y) => pending.extend([x, y]),
                            Node::Const { value, .. } => constant = constant.wrapping_add(value),
                            _ => parts.push(current),
                        }
                    }
                    (parts, constant & mask(self.width(term)))
                };
                let ((left, c1), (mut right, c2)) = (parts(a), parts(b));
                c1 == c2
                    && left.len() == right.len()
                    && left.iter().all(|part| {
                        let found = right
                            .iter()
                            .position(|other| self.equivalent_in(*part, *other, known));
                        found.map(|index| right.swap_remove(index)).is_some()
                    })
            }
            (x, y) if std::mem::discriminant(x) == std::mem::discriminant(y) => {
                let same_parameters = match (x, y) {
                    (Node::Extract(_, h1, l1), Node::Extract(_, h2, l2)) => (h1, l1) == (h2, l2),
                    (Node::ZeroExtend(_, b1), Node::ZeroExtend(_, b2))
                    | (Node::SignExtend(_, b1), Node::SignExtend(_, b2)) => b1 == b2,
                    (Node::Crc(_, _, p), Node::Crc(_, _, q)) => p == q,
                    (Node::Float(p, ..), Node::Float(q, ..)) => p == q,
                    (Node::FloatTest(p, ..), Node::FloatTest(q, ..)) => p == q,
                    (Node::FloatRound(_, p), Node::FloatRound(_, q)) => p == q,
                    (Node::FloatFromInt(_, p), Node::FloatFromInt(_, q))
                    | (Node::FloatConvert(_, p), Node::FloatConvert(_, q)) => p == q,
                    (Node::FloatToInt(_, p, x), Node::FloatToInt(_, q, y)) => (p, x) == (q, y),
                    (Node::Const { .. }, _)
                    | (Node::Var { .. }, _)
                    | (Node::Bool(_), _)
                    | (Node::BoolVar(_), _)
                    | (Node::Lookup(..), _)
                    | (Node::Count(..), _) => false,
                    _ => true,
                };
                let (left, right) = (self.operands(a), self.operands(b));
                let in_order = left
                    .iter()
                    .zip(&right)
                    .all(|(x, y)| self.equivalent_in(*x, *y, known));
                let commutative = matches!(
                    x,
                    Node::Mul(..)
                        | Node::And(..)
                        | Node::Or(..)
                        | Node::Xor(..)
                        | Node::Eq(..)
                        | Node::BoolAnd(..)
                        | Node::BoolOr(..)
                );
                same_parameters
                    && left.len() == right.len()
                    && (in_order
                        || commutative
                            && left.len() == 2
                            && self.equivalent_in(left[0], right[1], known)
                            && self.equivalent_in(left[1], right[0], known))
            }
            _ => false,
        };
        known.insert((a, b), answer);
        answer
    }

    /// Whether one of `a` and `b` is the other negated.
    fn negates(&self, a: Term, b: Term) -> bool {
        matches!(*self.node(a), Node::BoolNot(inner) if inner == b)
            || matches!(*self.node(b), Node::BoolNot(inner) if inner == a)
    }

    /// Either true.
    pub fn bool_or(&mut self, a: Term, b: Term) -> Term {
        match (self.truth(a), self.truth(b)) {
            (Some(true), _) | (_, Some(true)) => return self.boolean(true),
            (Some(false), _) => return b,
            (_, Some(false)) => return a,
            _ => {}
        }
        // x < y or x = y is x <= y, which a range may settle.
        for (first, second) in [(a, b), (b, a)] {
            if let (Node::Ult(x, y), Node::Eq(p, q)) =
                (self.node(first).clone(), self.node(second).clone())
                && ((p, q) == (x, y) || (p, q) == (y, x))
            {
                return self.ule(x, y);
            }
        }
        if a == b {
            return a;
        }
        // x or not x.
        if self.negates(a, b) {
            return self.boolean(true);
        }
        // (x and c) or (y and c) is (x or y) and c: what two compares under one mask, joined as
        // korq joins them, say of a byte, with the mask said once.
        if let (&Node::BoolAnd(x1, y1), &Node::BoolAnd(x2, y2)) = (self.node(a), self.node(b)) {
            for (common, rest) in [(x1, y1), (y1, x1)] {
                for (other, rest_other) in [(x2, y2), (y2, x2)] {
                    if common == other {
                        let either = self.bool_or(rest, rest_other);
                        return self.bool_and(either, common);
                    }
                }
            }
        }
        let (a, b) = self.ordered(a, b);
        self.make(Node::BoolOr(a, b), Sort::Bool)
    }

    /// Not true.
    pub fn bool_not(&mut self, a: Term) -> Term {
        if let Some(x) = self.truth(a) {
            return self.boolean(!x);
        }
        if let Node::BoolNot(inner) = *self.node(a) {
            return inner;
        }
        self.make(Node::BoolNot(a), Sort::Bool)
    }

    /// All of `terms` true.
    pub fn all(&mut self, terms: &[Term]) -> Term {
        let mut result = self.boolean(true);
        for term in terms {
            result = self.bool_and(result, *term);
        }
        result
    }

    /// Any of `terms` true.
    pub fn any(&mut self, terms: &[Term]) -> Term {
        let mut result = self.boolean(false);
        for term in terms {
            result = self.bool_or(result, *term);
        }
        result
    }

    /// Every value `term`, eight bits computed from at most one byte variable, can take, in
    /// order: the byte's own range, worked through.
    #[must_use]
    pub fn byte_values(&self, term: Term) -> Option<Vec<u8>> {
        if self.width(term) != 8 {
            return None;
        }
        match self.soles[term.index()] {
            Sole::Constant => self.constant(term).map(|value| vec![value as u8]),
            Sole::Byte(byte) => {
                let (low, high) = self.range(byte);
                let mut values = Vec::new();
                for value in low..=high.min(255) {
                    let result = self.evaluate_with(term, &mut |_, variable| {
                        (variable == byte).then_some(value)
                    })?;
                    values.push(result as u8);
                }
                values.sort_unstable();
                values.dedup();
                Some(values)
            }
            Sole::Many => None,
        }
    }

    /// The term's value when every variable has the value `values` gives it (by name), or
    /// `None` when a variable has none. True is 1 and false 0.
    pub fn evaluate(&self, term: Term, values: &HashMap<String, u128>) -> Option<u128> {
        self.evaluate_with(term, &mut |pool, variable| {
            values.get(pool.name(variable)?).copied()
        })
    }

    /// The term's value with each variable's value from `value_of`, or `None` when it gives
    /// none. Only the chosen side of an `ite` is evaluated. Iterative, so terms of any depth
    /// evaluate on a small stack.
    pub fn evaluate_with(
        &self,
        term: Term,
        value_of: &mut dyn FnMut(&Self, Term) -> Option<u128>,
    ) -> Option<u128> {
        self.evaluate_cached(term, value_of, &mut HashMap::new())
    }

    /// [`Self::evaluate_with`], keeping every value it computes in `known`, which may hold
    /// values from earlier calls with the same variables' values.
    pub fn evaluate_cached(
        &self,
        term: Term,
        value_of: &mut dyn FnMut(&Self, Term) -> Option<u128>,
        known: &mut HashMap<Term, u128>,
    ) -> Option<u128> {
        enum Frame {
            /// Evaluate the term's operands, then the term.
            Enter(Term),
            /// The operands are known: compute the term.
            Exit(Term),
            /// An `ite`'s condition is known: evaluate the chosen side, then the term.
            Choose(Term),
        }
        let mut frames = vec![Frame::Enter(term)];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(current) => {
                    if known.contains_key(&current) {
                        continue;
                    }
                    match self.node(current) {
                        Node::Const { value, .. } => {
                            known.insert(current, *value);
                        }
                        Node::Bool(value) => {
                            known.insert(current, u128::from(*value));
                        }
                        Node::Var { .. } | Node::BoolVar(_) => {
                            let value = value_of(self, current)?;
                            known.insert(current, value);
                        }
                        Node::Ite(condition, ..) => {
                            frames.push(Frame::Choose(current));
                            frames.push(Frame::Enter(*condition));
                        }
                        _ => {
                            frames.push(Frame::Exit(current));
                            for operand in self.operands(current) {
                                frames.push(Frame::Enter(operand));
                            }
                        }
                    }
                }
                Frame::Choose(current) => {
                    let Node::Ite(condition, then, otherwise) = *self.node(current) else {
                        return None;
                    };
                    let chosen = if known.get(&condition)? == &1 {
                        then
                    } else {
                        otherwise
                    };
                    frames.push(Frame::Exit(current));
                    frames.push(Frame::Enter(chosen));
                }
                Frame::Exit(current) => {
                    let value = self.compute(current, known)?;
                    known.insert(current, value);
                }
            }
        }
        known.get(&term).copied()
    }

    /// A term's value from its operands' values in `known`; an `ite` reads its chosen side.
    fn compute(&self, term: Term, known: &HashMap<Term, u128>) -> Option<u128> {
        let width = self.width(term);
        let get = |operand: Term| known.get(&operand).copied();
        Some(match self.node(term).clone() {
            Node::Const { value, .. } => value,
            Node::Var { .. } | Node::BoolVar(_) => get(term)?,
            Node::Bool(value) => u128::from(value),
            Node::Add(a, b) => get(a)?.wrapping_add(get(b)?) & mask(width),
            Node::Mul(a, b) => get(a)?.wrapping_mul(get(b)?) & mask(width),
            Node::Udiv(a, b) => match get(b)? {
                0 => mask(width),
                y => get(a)? / y,
            },
            Node::Urem(a, b) => match get(b)? {
                0 => get(a)?,
                y => get(a)? % y,
            },
            Node::Sdiv(a, b) => {
                let (x, y) = (signed(get(a)?, width), signed(get(b)?, width));
                let quotient = if y == 0 {
                    if x < 0 { 1 } else { -1 }
                } else {
                    x.wrapping_div(y)
                };
                unsigned(quotient, width)
            }
            Node::Srem(a, b) => {
                let x = get(a)?;
                let (sx, sy) = (signed(x, width), signed(get(b)?, width));
                if sy == 0 {
                    x
                } else {
                    unsigned(sx.wrapping_rem(sy), width)
                }
            }
            Node::And(a, b) => get(a)? & get(b)?,
            Node::Or(a, b) => get(a)? | get(b)?,
            Node::Xor(a, b) => get(a)? ^ get(b)?,
            Node::Not(a) => !get(a)? & mask(width),
            Node::Neg(a) => get(a)?.wrapping_neg() & mask(width),
            Node::Shl(a, b) => match get(b)? {
                y if y >= u128::from(width) => 0,
                y => (get(a)? << y) & mask(width),
            },
            Node::Lshr(a, b) => match get(b)? {
                y if y >= u128::from(width) => 0,
                y => get(a)? >> y,
            },
            Node::Ashr(a, b) => {
                let shift = u32::try_from(get(b)?.min(u128::from(width - 1))).unwrap_or(width - 1);
                unsigned(signed(get(a)?, width) >> shift, width)
            }
            Node::Extract(a, high, low) => (get(a)? >> low) & mask(high - low + 1),
            Node::ZeroExtend(a, _) => get(a)?,
            Node::SignExtend(a, _) => {
                let narrow = self.width(a);
                unsigned(signed(get(a)?, narrow), width)
            }
            Node::Concat(high, low) => (get(high)? << self.width(low)) | get(low)?,
            Node::Ite(condition, then, otherwise) => {
                if get(condition)? == 1 {
                    get(then)?
                } else {
                    get(otherwise)?
                }
            }
            Node::Eq(a, b) => u128::from(get(a)? == get(b)?),
            Node::Ult(a, b) => u128::from(get(a)? < get(b)?),
            Node::Slt(a, b) => {
                let operand_width = self.width(a);
                u128::from(signed(get(a)?, operand_width) < signed(get(b)?, operand_width))
            }
            Node::BoolAnd(a, b) => u128::from(get(a)? == 1 && get(b)? == 1),
            Node::BoolOr(a, b) => u128::from(get(a)? == 1 || get(b)? == 1),
            Node::BoolNot(a) => u128::from(get(a)? == 0),
            Node::Lookup(byte, number) => {
                let value = get(byte)?;
                let table = self.tables[number as usize];
                u128::from(value < 256 && table[(value / 64) as usize] >> (value % 64) & 1 == 1)
            }
            Node::Count(number, _) => {
                let mut held = 0_u128;
                let mut at = Some(number);
                while let Some(here) = at {
                    let counted = &self.counts[here as usize];
                    for part in &counted.own {
                        held += u128::from(get(*part)? == 1);
                    }
                    at = counted.base;
                }
                held
            }
            Node::Crc(state, byte, poly) => {
                u128::from(crc_byte(poly, get(state)? as u32, get(byte)? as u8))
            }
            Node::Float(op, a, b) => float::binary(op, get(a)?, get(b)?, self.width(a)),
            Node::FloatSqrt(a) => float::sqrt(get(a)?, self.width(a)),
            Node::FloatFma(a, b, c) => float::fma(get(a)?, get(b)?, get(c)?, self.width(a)),
            Node::FloatFromInt(a, width) => float::from_int(get(a)?, self.width(a), width),
            Node::FloatToInt(a, to, truncate) => {
                float::to_int(get(a)?, self.width(a), to, truncate)
            }
            Node::FloatRound(a, rounding) => float::round(get(a)?, rounding, self.width(a)),
            Node::FloatConvert(a, to) => float::convert(get(a)?, self.width(a), to),
            Node::FloatTest(test, a, b) => {
                u128::from(float::test(test, get(a)?, get(b)?, self.width(a)))
            }
        })
    }

    /// Whether any of `terms` has a float operation in it.
    #[must_use]
    pub fn has_float(&self, terms: &[Term]) -> bool {
        terms
            .iter()
            .any(|term| self.floats.get(term.index()).copied().unwrap_or(false))
    }

    /// How the solvers read a float result: its value as an SMT-LIB float, `value`, which
    /// pins its bits when it isn't a NaN, and its bits when it is, `nan`, as x86 gives them: the
    /// first NaN operand quieted, or the default NaN. `None` for a term that isn't one.
    #[must_use]
    pub fn float_definition(&self, term: Term) -> Option<FloatDefinition> {
        let name = |operand: &Term| format!("t{}", operand.index());
        let width = self.width(term);
        let quiet = |operand: &Term| {
            let width = self.width(*operand);
            format!(
                "(bvor {} {})",
                name(operand),
                bits_literal(float::quiet_bit(width), width)
            )
        };
        let first_nan = |operands: &[Term]| {
            let mut nan = bits_literal(float::default_nan(width), width);
            for operand in operands.iter().rev() {
                nan = format!(
                    "(ite (fp.isNaN {}) {} {nan})",
                    fp_of(&name(operand), width),
                    quiet(operand)
                );
            }
            nan
        };
        let of = |operand: &Term| fp_of(&name(operand), self.width(*operand));
        let (value, nan) = match self.node(term) {
            Node::Float(op, a, b) => {
                let op = match op {
                    FloatOp::Add => "fp.add",
                    FloatOp::Sub => "fp.sub",
                    FloatOp::Mul => "fp.mul",
                    FloatOp::Div => "fp.div",
                };
                (
                    format!("({op} RNE {} {})", of(a), of(b)),
                    first_nan(&[*a, *b]),
                )
            }
            Node::FloatSqrt(a) => (format!("(fp.sqrt RNE {})", of(a)), first_nan(&[*a])),
            Node::FloatFma(a, b, c) => (
                format!("(fp.fma RNE {} {} {})", of(a), of(b), of(c)),
                first_nan(&[*a, *b, *c]),
            ),
            // Never a NaN.
            Node::FloatFromInt(a, _) => (
                format!("({} RNE {})", fp_sort_cast(width), name(a)),
                bits_literal(float::default_nan(width), width),
            ),
            Node::FloatRound(a, rounding) => {
                let mode = match rounding {
                    Rounding::Nearest => "RNE",
                    Rounding::Down => "RTN",
                    Rounding::Up => "RTP",
                    Rounding::Zero => "RTZ",
                    Rounding::Away => "RNA",
                };
                (
                    format!("(fp.roundToIntegral {mode} {})", of(a)),
                    first_nan(&[*a]),
                )
            }
            // A NaN keeps its sign and the top of its payload, quieted.
            Node::FloatConvert(a, to) => {
                let a = name(a);
                let nan = if *to == 32 {
                    format!("(concat ((_ extract 63 63) {a}) #b111111111 ((_ extract 50 29) {a}))")
                } else {
                    format!(
                        "(concat ((_ extract 31 31) {a}) #b111111111111 ((_ extract 21 0) {a}) \
                         #b00000000000000000000000000000)"
                    )
                };
                let from = if *to == 32 { 64 } else { 32 };
                (
                    format!("({} RNE {})", fp_sort_cast(*to), fp_of(&a, from)),
                    nan,
                )
            }
            _ => return None,
        };
        Some(FloatDefinition { value, nan, width })
    }

    /// A term written out as an expression, `depth` levels deep, for finding where two terms
    /// that should be one differ.
    #[must_use]
    pub fn sketch(&self, term: Term, depth: u32) -> String {
        let operands = self.operands(term);
        let own = self.smt_node(term);
        if depth == 0 || operands.is_empty() {
            return format!("t{}", term.index()).replace(&format!("t{}", term.index()), &own);
        }
        let mut text = own;
        for operand in operands {
            text = text.replace(
                &format!("t{} ", operand.index()),
                &format!("{} ", self.sketch(operand, depth - 1)),
            );
            text = text.replace(
                &format!("t{})", operand.index()),
                &format!("{})", self.sketch(operand, depth - 1)),
            );
        }
        text
    }

    /// The SMT-LIB text of `term`'s own operation, naming its operands `t<index>`.
    #[must_use]
    pub fn smt_node(&self, term: Term) -> String {
        let name = |operand: &Term| format!("t{}", operand.index());
        match self.node(term) {
            Node::Const { width, value } => {
                if width.is_multiple_of(4) {
                    format!("#x{value:0digits$x}", digits = (*width / 4) as usize)
                } else {
                    format!("#b{value:0digits$b}", digits = *width as usize)
                }
            }
            Node::Var { name: var, .. } | Node::BoolVar(var) => var.clone(),
            Node::Bool(value) => value.to_string(),
            Node::Add(a, b) => format!("(bvadd {} {})", name(a), name(b)),
            Node::Mul(a, b) => format!("(bvmul {} {})", name(a), name(b)),
            Node::Udiv(a, b) => format!("(bvudiv {} {})", name(a), name(b)),
            Node::Urem(a, b) => format!("(bvurem {} {})", name(a), name(b)),
            Node::Sdiv(a, b) => format!("(bvsdiv {} {})", name(a), name(b)),
            Node::Srem(a, b) => format!("(bvsrem {} {})", name(a), name(b)),
            Node::And(a, b) => format!("(bvand {} {})", name(a), name(b)),
            Node::Or(a, b) => format!("(bvor {} {})", name(a), name(b)),
            Node::Xor(a, b) => format!("(bvxor {} {})", name(a), name(b)),
            Node::Not(a) => format!("(bvnot {})", name(a)),
            // Never bvneg or bvsub: z3 4.16's default bit-vector strategy answers unsat for
            // satisfiable formulas that use them. -x is ~x + 1.
            Node::Neg(a) => {
                let width = self.width(*a);
                let one = if width.is_multiple_of(4) {
                    format!("#x{:0digits$x}", 1, digits = (width / 4) as usize)
                } else {
                    format!("#b{:0digits$b}", 1, digits = width as usize)
                };
                format!("(bvadd (bvnot {}) {one})", name(a))
            }
            Node::Shl(a, b) => format!("(bvshl {} {})", name(a), name(b)),
            Node::Lshr(a, b) => format!("(bvlshr {} {})", name(a), name(b)),
            Node::Ashr(a, b) => format!("(bvashr {} {})", name(a), name(b)),
            Node::Extract(a, high, low) => format!("((_ extract {high} {low}) {})", name(a)),
            Node::ZeroExtend(a, by) => format!("((_ zero_extend {by}) {})", name(a)),
            Node::SignExtend(a, by) => format!("((_ sign_extend {by}) {})", name(a)),
            Node::Concat(a, b) => format!("(concat {} {})", name(a), name(b)),
            Node::Ite(c, a, b) => format!("(ite {} {} {})", name(c), name(a), name(b)),
            Node::Eq(a, b) => format!("(= {} {})", name(a), name(b)),
            Node::Ult(a, b) => format!("(bvult {} {})", name(a), name(b)),
            Node::Slt(a, b) => format!("(bvslt {} {})", name(a), name(b)),
            Node::BoolAnd(a, b) => format!("(and {} {})", name(a), name(b)),
            Node::BoolOr(a, b) => format!("(or {} {})", name(a), name(b)),
            Node::BoolNot(a) => format!("(not {})", name(a)),
            Node::Lookup(byte, number) => {
                // The set as runs of values: a byte in one of them.
                let table = self.tables[*number as usize];
                let has = |value: u32| table[(value / 64) as usize] >> (value % 64) & 1 == 1;
                let byte = name(byte);
                let mut runs = Vec::new();
                let mut value = 0;
                while value < 256 {
                    if !has(value) {
                        value += 1;
                        continue;
                    }
                    let start = value;
                    while value < 256 && has(value) {
                        value += 1;
                    }
                    runs.push(if value - start == 1 {
                        format!("(= {byte} #x{start:02x})")
                    } else {
                        format!(
                            "(and (bvule #x{start:02x} {byte}) (bvule {byte} #x{:02x}))",
                            value - 1
                        )
                    });
                }
                match runs.len() {
                    0 => "false".to_owned(),
                    1 => runs.remove(0),
                    _ => format!("(or {})", runs.join(" ")),
                }
            }
            Node::FloatToInt(a, to, truncate) => {
                let width = self.width(*a);
                let mode = if *truncate { "RTZ" } else { "RNE" };
                let whole = format!("(fp.roundToIntegral {mode} {})", fp_of(&name(a), width));
                let limit = 2_f64.powi(i32::try_from(*to).unwrap_or(64) - 1);
                let (low, high) = (fp_literal(-limit, width), fp_literal(limit, width));
                let indefinite = format!(
                    "#x{:0digits$x}",
                    1_u128 << (to - 1),
                    digits = (*to / 4) as usize
                );
                format!(
                    "(ite (and (not (fp.isNaN {whole})) (fp.geq {whole} {low}) (fp.lt {whole} \
                     {high})) ((_ fp.to_sbv {to}) RTZ {whole}) {indefinite})"
                )
            }
            Node::FloatTest(test, a, b) => {
                let width = self.width(*a);
                let (x, y) = (fp_of(&name(a), width), fp_of(&name(b), width));
                match test {
                    FloatTest::Less => format!("(fp.lt {x} {y})"),
                    FloatTest::Equal => format!("(fp.eq {x} {y})"),
                    FloatTest::Unordered => format!("(or (fp.isNaN {x}) (fp.isNaN {y}))"),
                }
            }
            // The solvers declare a float result's bits and pin them (see float_definition);
            // z3 alone can write it as one expression, for a sketch.
            Node::Float(..)
            | Node::FloatSqrt(_)
            | Node::FloatFma(..)
            | Node::FloatFromInt(..)
            | Node::FloatRound(..)
            | Node::FloatConvert(..) => self
                .float_definition(term)
                .map(|definition| {
                    format!(
                        "(ite (fp.isNaN {value}) {nan} (fp.to_ieee_bv {value}))",
                        value = definition.value,
                        nan = definition.nan
                    )
                })
                .unwrap_or_default(),
            // The state's low byte xored with the byte picks columns of the step's table, which
            // is linear: one column for each of the 8 bits, xored with the state shifted right by
            // a byte.
            Node::Crc(state, byte, poly) => {
                let (state, byte) = (name(state), name(byte));
                let mixed = format!("(bvxor ((_ extract 7 0) {state}) {byte})");
                let columns: Vec<String> = (0..8)
                    .map(|bit| {
                        format!(
                            "(ite (= ((_ extract {bit} {bit}) {mixed}) #b1) #x{:08x} #x00000000)",
                            crc_byte(*poly, 0, 1 << bit)
                        )
                    })
                    .collect();
                format!("(bvxor (bvlshr {state} #x00000008) {})", columns.join(" "))
            }
            Node::Count(number, _) => {
                let width = self.width(term);
                let (one, zero) = if width.is_multiple_of(4) {
                    (
                        format!("#x{:0digits$x}", 1, digits = (width / 4) as usize),
                        format!("#x{:0digits$x}", 0, digits = (width / 4) as usize),
                    )
                } else {
                    (
                        format!("#b{:0digits$b}", 1, digits = width as usize),
                        format!("#b{:0digits$b}", 0, digits = width as usize),
                    )
                };
                let ones: Vec<String> = self
                    .count_parts(*number)
                    .iter()
                    .map(|part| format!("(ite {} {one} {zero})", name(part)))
                    .collect();
                match ones.len() {
                    1 => ones[0].clone(),
                    _ => format!("(bvadd {})", ones.join(" ")),
                }
            }
        }
    }

    /// Every variable `terms` are made of.
    #[must_use]
    pub fn variables(&self, terms: &[Term]) -> Vec<Term> {
        let mut seen = vec![false; self.len()];
        let mut pending = terms.to_vec();
        let mut found = Vec::new();
        while let Some(term) = pending.pop() {
            if std::mem::replace(&mut seen[term.index()], true) {
                continue;
            }
            match self.node(term) {
                Node::Var { .. } | Node::BoolVar(_) => found.push(term),
                _ => pending.extend(self.operands(term)),
            }
        }
        found
    }

    /// `term` with each condition `facts` settles replaced by its truth, simplified again: what
    /// it comes to on a run whose branches settled those conditions.
    pub fn assuming(&mut self, term: Term, facts: &HashMap<Term, bool>) -> Term {
        self.assuming_with(term, facts, &mut HashMap::new())
    }

    /// [`Self::assuming`], with `done` holding what earlier calls with the same facts worked
    /// out: terms that share long parts, as a run's conditions on one count do, rewrite them once.
    pub fn assuming_with(
        &mut self,
        term: Term,
        facts: &HashMap<Term, bool>,
        done: &mut HashMap<Term, Term>,
    ) -> Term {
        let mut pending = vec![(term, false)];
        while let Some((current, expanded)) = pending.pop() {
            if done.contains_key(&current) {
                continue;
            }
            if let Some(truth) = facts.get(&current) {
                let value = self.boolean(*truth);
                done.insert(current, value);
                continue;
            }
            // A count's conditions are its operands here: they settle as any others do.
            let operands = match *self.node(current) {
                Node::Count(number, _) => self.count_parts(number),
                _ => node_operands(self.node(current)),
            };
            if operands.is_empty() {
                done.insert(current, current);
                continue;
            }
            if !expanded {
                pending.push((current, true));
                pending.extend(
                    operands
                        .iter()
                        .filter(|operand| !done.contains_key(operand))
                        .map(|operand| (*operand, false)),
                );
                continue;
            }
            let rebuilt: Vec<Term> = operands.iter().map(|operand| done[operand]).collect();
            let result = if rebuilt == operands {
                current
            } else if let Node::Count(_, width) = *self.node(current) {
                self.count(width, &rebuilt)
            } else {
                let node = self.node(current).clone();
                self.rebuild(&node, &rebuilt)
            };
            done.insert(current, result);
        }
        done.get(&term).copied().unwrap_or(term)
    }

    /// `term` built again with `operands` in place of its own, through the builders; a count's
    /// operands are its conditions.
    pub fn with_operands(&mut self, term: Term, operands: &[Term]) -> Term {
        if let Node::Count(_, width) = *self.node(term) {
            return self.count(width, operands);
        }
        let node = self.node(term).clone();
        self.rebuild(&node, operands)
    }

    /// `node` built again with `operands` in place of its own, through the builders.
    fn rebuild(&mut self, node: &Node, operands: &[Term]) -> Term {
        let (a, b, c) = (
            operands.first().copied(),
            operands.get(1).copied(),
            operands.get(2).copied(),
        );
        let (Some(a), b) = (a, b) else {
            return self.make(node.clone(), Sort::Bool);
        };
        let two = |pool: &mut Self, build: fn(&mut Self, Term, Term) -> Term| {
            b.map_or(a, |b| build(pool, a, b))
        };
        match *node {
            Node::Add(..) => two(self, Self::add),
            Node::Mul(..) => two(self, Self::mul),
            Node::Udiv(..) => two(self, Self::udiv),
            Node::Urem(..) => two(self, Self::urem),
            Node::Sdiv(..) => two(self, Self::sdiv),
            Node::Srem(..) => two(self, Self::srem),
            Node::And(..) => two(self, Self::and),
            Node::Or(..) => two(self, Self::or),
            Node::Xor(..) => two(self, Self::xor),
            Node::Shl(..) => two(self, Self::shl),
            Node::Lshr(..) => two(self, Self::lshr),
            Node::Ashr(..) => two(self, Self::ashr),
            Node::Concat(..) => two(self, Self::concat),
            Node::Eq(..) => two(self, Self::eq),
            Node::Ult(..) => two(self, Self::ult),
            Node::Slt(..) => two(self, Self::slt),
            Node::BoolAnd(..) => two(self, Self::bool_and),
            Node::BoolOr(..) => two(self, Self::bool_or),
            Node::Crc(_, _, poly) => b.map_or(a, |b| self.crc(a, b, poly)),
            Node::Float(op, ..) => b.map_or(a, |b| self.float(op, a, b)),
            Node::FloatTest(test, ..) => b.map_or(a, |b| self.float_test(test, a, b)),
            Node::FloatFma(..) => match (b, c) {
                (Some(b), Some(c)) => self.float_fma(a, b, c),
                _ => a,
            },
            Node::FloatSqrt(_) => self.float_sqrt(a),
            Node::FloatFromInt(_, width) => self.float_from_int(a, width),
            Node::FloatToInt(_, to, truncate) => self.float_to_int(a, to, truncate),
            Node::FloatRound(_, rounding) => self.float_round(a, rounding),
            Node::FloatConvert(_, to) => self.float_convert(a, to),
            Node::Not(_) => self.not(a),
            Node::Neg(_) => self.neg(a),
            Node::BoolNot(_) => self.bool_not(a),
            Node::Extract(_, high, low) => self.extract(a, high, low),
            Node::ZeroExtend(_, by) => self.zero_extend(a, by),
            Node::SignExtend(_, by) => self.sign_extend(a, by),
            Node::Ite(..) => match (b, c) {
                (Some(then), Some(otherwise)) => self.ite(a, then, otherwise),
                _ => a,
            },
            Node::Lookup(_, table) => {
                let table = self.tables[table as usize];
                self.lookup(a, table)
            }
            Node::Const { .. }
            | Node::Var { .. }
            | Node::Bool(_)
            | Node::BoolVar(_)
            | Node::Count(..) => a,
        }
    }

    /// The name of a variable term.
    #[must_use]
    pub fn name(&self, term: Term) -> Option<&str> {
        match self.node(term) {
            Node::Var { name, .. } | Node::BoolVar(name) => Some(name),
            _ => None,
        }
    }

    /// The terms `term` is made of, in the order they must be defined: operands first.
    #[must_use]
    pub fn operands(&self, term: Term) -> Vec<Term> {
        match *self.node(term) {
            Node::Count(number, _) => self.count_parts(number),
            ref node => node_operands(node),
        }
    }
}

pub mod float;
#[cfg(test)]
mod random;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_running_count_keeps_each_condition_once() {
        let mut pool = Pool::default();
        let mut total = pool.bv(64, 0);
        let mut all = Vec::new();
        for step in 0..512 {
            let parts: Vec<Term> = (0..16)
                .map(|lane| pool.bool_var(&format!("c{step}_{lane}")))
                .collect();
            all.extend_from_slice(&parts);
            let piece = pool.count(64, &parts);
            total = pool.add(total, piece);
        }
        let kept: usize = pool.counts.iter().map(|counted| counted.own.len()).sum();
        assert!(
            kept <= 2 * all.len(),
            "{kept} conditions kept for {} counted",
            all.len()
        );
        assert_eq!(
            total,
            pool.count(64, &all),
            "the total is the count of them all"
        );
        assert_eq!(pool.range(total), (0, all.len() as u128));
        let values: HashMap<String, u128> = (0..512)
            .flat_map(|step| (0..16).map(move |lane| (step, lane)))
            .map(|(step, lane)| (format!("c{step}_{lane}"), u128::from(lane % 3 == 0)))
            .collect();
        assert_eq!(pool.evaluate(total, &values), Some(512 * 6));

        // A count extended by conditions made before its own still is the count of them all,
        // its conditions in order.
        let early: Vec<Term> = (0..3)
            .map(|n| pool.bool_var(&format!("early{n}")))
            .collect();
        let late: Vec<Term> = (0..4).map(|n| pool.bool_var(&format!("late{n}"))).collect();
        let base = pool.count(64, &late);
        let more = pool.count(64, &early);
        let both = pool.add(base, more);
        let mut every = [early, late].concat();
        assert_eq!(both, pool.count(64, &every));
        every.sort_unstable();
        let Node::Count(number, _) = *pool.node(both) else {
            panic!("not a count: {both:?}");
        };
        assert_eq!(pool.count_parts(number), every);
    }

    #[test]
    fn equal_computations_are_one_term() {
        let mut pool = Pool::default();
        let a = pool.var(64, "a");
        let b = pool.var(64, "b");
        let first = pool.add(a, b);
        let second = pool.add(b, a);
        assert_eq!(first, second, "operands are ordered");
        let three = pool.bv(64, 3);
        let four = pool.bv(64, 4);
        let plus_three = pool.add(a, three);
        let plus_seven = pool.add(plus_three, four);
        let seven = pool.bv(64, 7);
        assert_eq!(plus_seven, pool.add(a, seven), "constants gather");
        let minus = pool.sub(a, a);
        let _ = minus;
        let x = pool.xor(a, a);
        assert_eq!(pool.constant(x), Some(0));
    }

    #[test]
    fn bytes_written_and_read_back_are_the_same_term() {
        let mut pool = Pool::default();
        let rax = pool.var(64, "rax");
        let bytes: Vec<Term> = (0..8)
            .map(|byte| pool.extract(rax, byte * 8 + 7, byte * 8))
            .collect();
        let mut value = bytes[0];
        for byte in &bytes[1..] {
            value = pool.concat(*byte, value);
        }
        assert_eq!(value, rax);
        // The low bits of a sum are the sum of the low bits.
        let b = pool.var(64, "b");
        let sum = pool.add(rax, b);
        let low = pool.extract(sum, 31, 0);
        let (low_a, low_b) = (pool.extract(rax, 31, 0), pool.extract(b, 31, 0));
        assert_eq!(low, pool.add(low_a, low_b));
        // Digits come from dividing by powers of ten, and a loop ends when x < 10^k.
        let ten64 = pool.bv(64, 10);
        let once = pool.udiv(rax, ten64);
        let twice = pool.udiv(once, ten64);
        let hundred = pool.bv(64, 100);
        assert_eq!(twice, pool.udiv(rax, hundred));
        let zero64 = pool.bv(64, 0);
        let ended = pool.eq(twice, zero64);
        assert_eq!(ended, pool.ult(rax, hundred));
        // Dividing a zero-extended value divides the value.
        let wide = pool.zero_extend(rax, 64);
        let ten = pool.bv(128, 10);
        let quotient = pool.udiv(wide, ten);
        let narrow_ten = pool.bv(64, 10);
        let narrow = pool.udiv(rax, narrow_ten);
        assert_eq!(quotient, pool.zero_extend(narrow, 64));
    }

    #[test]
    fn evaluates_like_the_cpu() {
        let mut pool = Pool::default();
        let a = pool.var(8, "a");
        let b = pool.var(8, "b");
        let quotient = pool.sdiv(a, b);
        let remainder = pool.srem(a, b);
        let values = HashMap::from([("a".to_owned(), 0xf9_u128), ("b".to_owned(), 2)]);
        assert_eq!(pool.evaluate(quotient, &values), Some(0xfd), "-7 / 2 is -3");
        assert_eq!(
            pool.evaluate(remainder, &values),
            Some(0xff),
            "-7 % 2 is -1"
        );
        let less = pool.slt(a, b);
        assert_eq!(pool.evaluate(less, &values), Some(1));
        let one = pool.bv(8, 1);
        let shifted = pool.ashr(a, one);
        assert_eq!(pool.evaluate(shifted, &values), Some(0xfc));
    }
    #[test]
    fn halving_a_rounded_sum_of_cents_is_half_the_sum() {
        // mean: s = 100 x cents, one away from zero added, halved toward zero, is 50 x, as
        // one term; and the digits a program takes of it by 10 are the formula's by 100.
        let mut pool = Pool::default();
        let (a, b) = (pool.var_in(8, "a", 0, 9), pool.var_in(8, "b", 0, 9));
        let (a, b) = (pool.zero_extend(a, 56), pool.zero_extend(b, 56));
        let x = pool.sub(a, b);
        let hundred = pool.bv(64, 100);
        let cents = pool.mul(x, hundred);
        let one = pool.bv(64, 1);
        let (up, down) = (pool.add(cents, one), pool.sub(cents, one));
        let zero = pool.bv(64, 0);
        let negative = pool.slt(cents, zero);
        let rounded = pool.ite(negative, down, up);
        let two = pool.bv(64, 2);
        let half = pool.sdiv(rounded, two);
        let fifty = pool.bv(64, 50);
        let expected = pool.mul(x, fifty);
        assert_eq!(half, expected);
        // A sum that shares its factor with the divisor: (10 y) / 100 is y / 10.
        let ten = pool.bv(64, 10);
        let y = pool.mul(a, ten);
        let by_hundred = pool.udiv(y, hundred);
        assert_eq!(by_hundred, pool.udiv(a, ten));
        // A value halved by a shift, then divided by 10, is the value divided by 20.
        let halved = pool.udiv(y, two);
        let twenty = pool.bv(64, 20);
        assert_eq!(pool.udiv(halved, ten), pool.udiv(y, twenty));
        // A quotient is 0 just where the dividend is below the divisor.
        let nothing = pool.udiv(a, ten);
        assert_eq!(pool.eq(nothing, zero), pool.ult(a, ten));
    }

    #[test]
    fn a_programs_divisions_and_a_formulas_meet_as_one_term() {
        // (t / 86400 + 4) % 7: the program divides with 64-bit `div` (128-bit dividends),
        // the formula in signed 128 bits.
        let mut pool = Pool::default();
        let t = pool.var_in(64, "t", 0, 4_102_444_799);
        let (day, four, seven) = (86_400, 4, 7);
        let program = {
            let wide = pool.zero_extend(t, 64);
            let divisor = pool.bv(128, day);
            let quotient = pool.udiv(wide, divisor);
            let low = pool.extract(quotient, 63, 0);
            let four = pool.bv(64, four);
            let shifted = pool.add(low, four);
            let wide = pool.zero_extend(shifted, 64);
            let divisor = pool.bv(128, seven);
            let remainder = pool.urem(wide, divisor);
            pool.extract(remainder, 63, 0)
        };
        let formula = {
            let wide = pool.zero_extend(t, 64);
            let divisor = pool.bv(128, day);
            let quotient = pool.sdiv(wide, divisor);
            let four = pool.bv(128, four);
            let shifted = pool.add(quotient, four);
            let divisor = pool.bv(128, seven);
            let remainder = pool.srem(shifted, divisor);
            pool.extract(remainder, 63, 0)
        };
        assert_eq!(program, formula);
        // hh:mm:ss: t % 86400 / 3600 both ways.
        let program_hours = {
            let wide = pool.zero_extend(t, 64);
            let divisor = pool.bv(128, day);
            let remainder = pool.urem(wide, divisor);
            let low = pool.extract(remainder, 63, 0);
            let wide = pool.zero_extend(low, 64);
            let divisor = pool.bv(128, 3600);
            let quotient = pool.udiv(wide, divisor);
            pool.extract(quotient, 63, 0)
        };
        let formula_hours = {
            let wide = pool.zero_extend(t, 64);
            let divisor = pool.bv(128, day);
            let remainder = pool.srem(wide, divisor);
            let divisor = pool.bv(128, 3600);
            let quotient = pool.sdiv(remainder, divisor);
            pool.extract(quotient, 63, 0)
        };
        assert_eq!(program_hours, formula_hours);
    }

    #[test]
    fn reciprocal_multiplies_are_divisions() {
        let mut pool = Pool::default();
        let x = pool.var(64, "x");
        let wide = pool.zero_extend(x, 64);
        for divisor in [3_u128, 7, 10, 60, 1000, 3600, 86_400, 146_097] {
            // Granlund and Montgomery's round-up multiplier for 64-bit dividends.
            let shift = 128 - (divisor - 1).leading_zeros();
            let low = 64 + shift;
            let multiplier = (1_u128 << low).div_ceil(divisor);
            if multiplier > u128::from(u64::MAX) {
                // Needs 65 bits: the add-back form, left to the solver.
                continue;
            }
            let m = pool.bv(128, multiplier);
            let product = pool.mul(wide, m);
            let top = pool.extract(product, 127, low);
            let expected = {
                let divisor = pool.bv(64, divisor);
                let quotient = pool.udiv(x, divisor);
                let width = pool.width(top);
                if width >= 64 {
                    pool.zero_extend(quotient, width - 64)
                } else {
                    pool.extract(quotient, width - 1, 0)
                }
            };
            assert_eq!(top, expected, "dividing by {divisor}");
            // `shr` after the high half of `mul` is the same division.
            let high = pool.extract(product, 127, 64);
            let shifted = {
                let by = pool.bv(64, u128::from(shift));
                pool.lshr(high, by)
            };
            let narrow = pool.extract(shifted, 63 - shift, 0);
            let quotient = {
                let divisor = pool.bv(64, divisor);
                pool.udiv(x, divisor)
            };
            let fits = pool.extract(quotient, 63 - shift, 0);
            assert_eq!(narrow, fits, "dividing by {divisor} with shr");
        }
    }
}
