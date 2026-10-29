//! Terms: bit-vector and true/false expressions over a program's inputs, shared and simplified
//! as they're built, so the same computation is always the same term. A solver then proves two
//! terms equal at once when they are the same, which keeps proofs fast.

use std::collections::HashMap;

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

/// Marks a shape's number where a term would be, in a node that stands for a shape.
const SHAPE: u32 = 1 << 31;

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
    /// Each one-byte term's shape: the term with its byte's identity left out, so tests of
    /// different bytes that compute the same thing share one. `u32::MAX` for other terms.
    shapes: Vec<u32>,
    shape_index: HashMap<Node, u32>,
    /// The table each one-byte condition's shape computes, worked out once per shape.
    shape_tables: HashMap<u32, Table>,
    /// The sets lookups test, by number.
    tables: Vec<Table>,
    table_index: HashMap<Table, u32>,
    /// The conditions each count counts, sorted, by number.
    counts: Vec<Vec<Term>>,
    count_index: HashMap<Vec<Term>, u32>,
    /// The values a byte variable is held to by the conditions it's drawn with, such as a
    /// text's characters, for drawing inputs that meet them.
    allowed: HashMap<Term, Vec<u8>>,
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
        | Node::Lookup(a, _) => vec![a],
        Node::Ite(c, a, b) => vec![c, a, b],
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
        | Node::BoolOr(a, b) => vec![a, b],
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
        | Node::Lookup(a, _) => vec![a],
        Node::Ite(c, a, b) => vec![c, a, b],
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
        | Node::BoolOr(a, b) => vec![a, b],
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

    /// The conditions a count counts.
    #[must_use]
    pub fn count_parts(&self, number: u32) -> &[Term] {
        &self.counts[number as usize]
    }

    /// How many of `parts`, true/false terms, hold, as a `width`-bit number: a count, the same
    /// term whatever order the parts come in.
    pub fn count(&mut self, width: u32, parts: &[Term]) -> Term {
        let mut parts: Vec<Term> = parts
            .iter()
            .copied()
            .filter(|part| self.truth(*part) != Some(false))
            .collect();
        if parts.len() as u128 > mask(width) {
            // It would wrap: add it up instead.
            let (one, zero) = (self.bv(width, 1), self.bv(width, 0));
            let ones: Vec<Term> = parts
                .iter()
                .map(|part| self.ite(*part, one, zero))
                .collect();
            return self.sum(width, &ones);
        }
        parts.sort_unstable();
        if parts.is_empty() {
            return self.bv(width, 0);
        }
        if parts.iter().all(|part| self.truth(*part) == Some(true)) {
            return self.bv(width, parts.len() as u128);
        }
        let next = u32::try_from(self.counts.len()).unwrap_or(u32::MAX);
        let number = *self.count_index.entry(parts.clone()).or_insert(next);
        if number == next {
            self.counts.push(parts);
        }
        self.make(Node::Count(number, width), Sort::Bv(width))
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
            Node::Count(number, _) => Some(self.counts[number as usize].clone()),
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
                let parts = &self.counts[*number as usize];
                let sure = parts
                    .iter()
                    .filter(|part| matches!(self.node(**part), Node::Bool(true)))
                    .count();
                (sure as u128, parts.len() as u128)
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
            _ => full,
        }
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
        let negation_of = |other: Term| {
            matches!(*self.node(value), Node::Neg(inner) if inner == other)
                && self.signed_range(other).0 > smallest
        };
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

    /// The range of `value` where `condition` is `holds`.
    fn range_given(&self, condition: Term, holds: bool, value: Term) -> (u128, u128) {
        let (low, high) = self.range(value);
        if self.non_negative_when(condition, holds, value) {
            let (_, signed_high) = self.signed_range(value);
            if let Ok(signed_high) = u128::try_from(signed_high) {
                return (low.min(signed_high), signed_high.min(high));
            }
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

    /// `a + b`.
    pub fn add(&mut self, a: Term, b: Term) -> Term {
        let width = self.width(a);
        let (a, b) = self.ordered(a, b);
        match (self.constant(a), self.constant(b)) {
            (Some(x), Some(y)) => return self.bv(width, x.wrapping_add(y)),
            (_, Some(0)) => return a,
            (None, Some(y)) => {
                // (x + c1) + c2 is x + (c1 + c2).
                if let Node::Add(inner, constant) = *self.node(a)
                    && let Some(c1) = self.constant(constant)
                {
                    let sum = self.bv(width, c1.wrapping_add(y));
                    return self.add(inner, sum);
                }
            }
            _ => {}
        }
        // Two counts, or a count and a 1-or-0, are one count of all their conditions.
        if self.is_counted(a)
            && self.is_counted(b)
            && let (Some(mut left), Some(right)) = (self.counted(a), self.counted(b))
            && (left.len() + right.len()) as u128 <= mask(width)
        {
            left.extend(right);
            return self.count(width, &left);
        }
        // x - (x / c) × c is x % c, the remainder however it was written.
        for (x, other) in [(a, b), (b, a)] {
            if let Node::Neg(product) = *self.node(other)
                && let Node::Mul(quotient, by) = *self.node(product)
                && let Node::Udiv(dividend, divisor) = *self.node(quotient)
                && dividend == x
                && self.constant(by).is_some()
                && self.constant(by) == self.constant(divisor)
            {
                return self.urem(x, divisor);
            }
        }
        self.make(Node::Add(a, b), Sort::Bv(width))
    }

    /// The sum of every term in `terms`, all one width, added left to right.
    pub fn sum(&mut self, width: u32, terms: &[Term]) -> Term {
        let mut total = self.bv(width, 0);
        for term in terms {
            total = self.add(total, *term);
        }
        total
    }

    /// `a - b`, as `a + -b`, the form solvers handle best.
    pub fn sub(&mut self, a: Term, b: Term) -> Term {
        let negated = self.neg(b);
        self.add(a, negated)
    }

    /// `a × b`, keeping the low bits.
    pub fn mul(&mut self, a: Term, b: Term) -> Term {
        let width = self.width(a);
        let (a, b) = self.ordered(a, b);
        match (self.constant(a), self.constant(b)) {
            (Some(x), Some(y)) => return self.bv(width, x.wrapping_mul(y)),
            (_, Some(0)) => return self.bv(width, 0),
            (_, Some(1)) => return a,
            _ => {}
        }
        self.make(Node::Mul(a, b), Sort::Bv(width))
    }

    /// Unsigned `a / b`; dividing by zero gives all ones, as in SMT-LIB.
    pub fn udiv(&mut self, a: Term, b: Term) -> Term {
        let width = self.width(a);
        match (self.constant(a), self.constant(b)) {
            (_, Some(0)) => return self.bv(width, mask(width)),
            (Some(x), Some(y)) => return self.bv(width, x / y),
            (_, Some(1)) => return a,
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
            (_, Some(y)) => {
                // A value below the divisor is its own remainder.
                if self.range(a).1 < y {
                    return a;
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
        // By a positive constant: divide the magnitude, then put the sign back, toward zero.
        if let Some(c) = self.constant(b)
            && c > 0
            && c <= mask(width - 1)
        {
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
            _ => {}
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
            _ => {}
        }
        self.make(Node::Xor(a, b), Sort::Bv(width))
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

    /// Two's-complement negation.
    pub fn neg(&mut self, a: Term) -> Term {
        let width = self.width(a);
        if let Some(x) = self.constant(a) {
            return self.bv(width, x.wrapping_neg());
        }
        if let Node::Neg(inner) = *self.node(a) {
            return inner;
        }
        self.make(Node::Neg(a), Sort::Bv(width))
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
        match *self.node(a) {
            Node::Extract(inner, _, inner_low) => {
                return self.extract(inner, high + inner_low, low + inner_low);
            }
            // A count's low bits are the count when it fits them.
            Node::Count(number, _)
                if low == 0 && self.counts[number as usize].len() as u128 <= mask(out) =>
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
            // The low bits of a value that fits in them, widened back, are the value.
            Node::Extract(inner, high, 0)
                if self.width(inner) == width && self.range(inner).1 <= mask(high + 1) =>
            {
                return inner;
            }
            // A sum or product that never wraps widens part by part, so the same arithmetic
            // done in different widths ends up the same term.
            Node::Add(x, y) => {
                let ((xl, xh), (_, yh)) = (self.range(x), self.range(y));
                if xh.checked_add(yh).is_some_and(|high| high <= mask(narrow)) {
                    let (x, y) = (self.zero_extend(x, by), self.zero_extend(y, by));
                    return self.add(x, y);
                }
                if let Some(c) = self.constant(y)
                    && narrow < 128
                    && c > mask(narrow - 1)
                    && xl > mask(narrow) - c
                {
                    // x + c wraps for every x: it's x - (2^narrow - c) without wrapping.
                    let below = mask(narrow) - c + 1;
                    let wide_x = self.zero_extend(x, by);
                    let wide_c = self.bv(width, 0_u128.wrapping_sub(below));
                    return self.add(wide_x, wide_c);
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
            Node::Add(x, y)
                if narrow < 128 && fits(self, a) && {
                    let ((xl, xh), (yl, yh)) = (self.signed_range(x), self.signed_range(y));
                    xl.checked_add(yl)
                        .zip(xh.checked_add(yh))
                        .is_some_and(|(low, high)| {
                            low >= -(1_i128 << (narrow - 1)) && high < (1_i128 << (narrow - 1))
                        })
                } =>
            {
                let (x, y) = (self.sign_extend(x, by), self.sign_extend(y, by));
                return Some(self.add(x, y));
            }
            Node::Neg(x) if narrow < 128 && self.signed_range(x).0 > -(1_i128 << (narrow - 1)) => {
                let x = self.sign_extend(x, by);
                return Some(self.neg(x));
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
            && matches!(*self.node(otherwise), Node::Neg(inner) if inner == right)
        {
            let negative = self.slt(right, left);
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
        self.make(Node::Ite(condition, then, otherwise), sort)
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
        // Joined parts equal a constant when each equals its own bits of it: a mask of byte
        // tests is 0 when none of the tests holds.
        for (joined, other) in [(a, b), (b, a)] {
            if let (Node::Concat(high, low), Some(k)) =
                (self.node(joined).clone(), self.constant(other))
            {
                let low_width = self.width(low);
                let (high_value, low_value) = (k >> low_width, k & mask(low_width));
                let high_constant = self.bv(self.width(high), high_value);
                let low_constant = self.bv(low_width, low_value);
                let (high_same, low_same) =
                    (self.eq(high, high_constant), self.eq(low, low_constant));
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
        if let Some((a, b)) = self.narrow_pair(a, b) {
            return self.ult(a, b);
        }
        if let Some((a, b)) = self.lifted_pair(a, b, false) {
            return self.ult(a, b);
        }
        self.make(Node::Ult(a, b), Sort::Bool)
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
                for part in &self.counts[number as usize] {
                    held += u128::from(get(*part)? == 1);
                }
                held
            }
        })
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
                let ones: Vec<String> = self.counts[*number as usize]
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
            Node::Count(number, _) => self.counts[number as usize].clone(),
            ref node => node_operands(node),
        }
    }
}

#[cfg(test)]
mod random;

#[cfg(test)]
mod tests {
    use super::*;

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
