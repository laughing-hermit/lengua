//! Terms: bit-vector and true/false expressions over a program's inputs, shared and simplified
//! as they're built, so the same computation is always the same term. A solver then proves two
//! terms equal at once when they are the same, which keeps proofs fast.

use std::collections::HashMap;

/// A term in a [`Pool`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Term(u32);

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
            Node::Ite(_, a, b) => Some((s(a).0.min(s(b).0), s(a).1.max(s(b).1))),
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
        self.nodes.push(node.clone());
        self.sorts.push(sort);
        self.ranges.push(range);
        self.signed_ranges.push(signed_range);
        self.index.insert(node, term);
        term
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
                if low > 0 && width < 128 {
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
            Node::Ite(_, a, b) => (r(a).0.min(r(b).0), r(a).1.max(r(b).1)),
            _ => full,
        }
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
        self.make(Node::Add(a, b), Sort::Bv(width))
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
                if let Node::ZeroExtend(inner, by) = *self.node(a) {
                    let narrow = width - by;
                    if y <= mask(narrow) {
                        let divisor = self.bv(narrow, y);
                        let remainder = self.urem(inner, divisor);
                        return self.zero_extend(remainder, by);
                    }
                }
            }
            _ => {}
        }
        self.make(Node::Urem(a, b), Sort::Bv(width))
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

    /// Shift left.
    pub fn shl(&mut self, a: Term, b: Term) -> Term {
        self.shift(a, b, Node::Shl)
    }

    /// Logical shift right.
    pub fn lshr(&mut self, a: Term, b: Term) -> Term {
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
            _ => {}
        }
        self.make(Node::Extract(a, high, low), Sort::Bv(out))
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
            _ => {}
        }

        self.make(Node::ZeroExtend(a, by), Sort::Bv(width))
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
        if let Node::SignExtend(inner, inner_by) = *self.node(a) {
            return self.sign_extend(inner, inner_by + by);
        }
        self.make(Node::SignExtend(a, by), Sort::Bv(width))
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
        // -x = c is x = -c.
        if let (Node::Neg(x), Some(c)) = (self.node(a).clone(), self.constant(b)) {
            let width = self.width(b);
            let negated = self.bv(width, c.wrapping_neg());
            return self.eq(x, negated);
        }
        // x / c = 0 is x < c: what a digit loop's end comes down to.
        if self.constant(b) == Some(0)
            && let Node::Udiv(x, divisor) = *self.node(a)
            && self.constant(divisor).is_some_and(|c| c > 0)
        {
            return self.ult(x, divisor);
        }
        if let (Some(x), Some(y)) = (self.constant(a), self.constant(b)) {
            return self.boolean(x == y);
        }
        if let (Some(x), Some(y)) = (self.truth(a), self.truth(b)) {
            return self.boolean(x == y);
        }
        self.make(Node::Eq(a, b), Sort::Bool)
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
        self.make(Node::Ult(a, b), Sort::Bool)
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
        let (a, b) = self.ordered(a, b);
        self.make(Node::BoolAnd(a, b), Sort::Bool)
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
        let mut known: HashMap<Term, u128> = HashMap::new();
        self.evaluate_into(term, values, &mut known)
    }

    fn evaluate_into(
        &self,
        term: Term,
        values: &HashMap<String, u128>,
        known: &mut HashMap<Term, u128>,
    ) -> Option<u128> {
        if let Some(value) = known.get(&term) {
            return Some(*value);
        }
        let width = self.width(term);
        let get = |operand: Term, known: &mut HashMap<Term, u128>| {
            self.evaluate_into(operand, values, known)
        };
        let value = match self.node(term).clone() {
            Node::Const { value, .. } => value,
            Node::Var { name, .. } | Node::BoolVar(name) => *values.get(&name)?,
            Node::Bool(value) => u128::from(value),
            Node::Add(a, b) => get(a, known)?.wrapping_add(get(b, known)?) & mask(width),
            Node::Mul(a, b) => get(a, known)?.wrapping_mul(get(b, known)?) & mask(width),
            Node::Udiv(a, b) => match get(b, known)? {
                0 => mask(width),
                y => get(a, known)? / y,
            },
            Node::Urem(a, b) => match get(b, known)? {
                0 => get(a, known)?,
                y => get(a, known)? % y,
            },
            Node::Sdiv(a, b) => {
                let (x, y) = (signed(get(a, known)?, width), signed(get(b, known)?, width));
                let quotient = if y == 0 {
                    if x < 0 { 1 } else { -1 }
                } else {
                    x.wrapping_div(y)
                };
                unsigned(quotient, width)
            }
            Node::Srem(a, b) => {
                let x = get(a, known)?;
                let (sx, sy) = (signed(x, width), signed(get(b, known)?, width));
                if sy == 0 {
                    x
                } else {
                    unsigned(sx.wrapping_rem(sy), width)
                }
            }
            Node::And(a, b) => get(a, known)? & get(b, known)?,
            Node::Or(a, b) => get(a, known)? | get(b, known)?,
            Node::Xor(a, b) => get(a, known)? ^ get(b, known)?,
            Node::Not(a) => !get(a, known)? & mask(width),
            Node::Neg(a) => get(a, known)?.wrapping_neg() & mask(width),
            Node::Shl(a, b) => match get(b, known)? {
                y if y >= u128::from(width) => 0,
                y => (get(a, known)? << y) & mask(width),
            },
            Node::Lshr(a, b) => match get(b, known)? {
                y if y >= u128::from(width) => 0,
                y => get(a, known)? >> y,
            },
            Node::Ashr(a, b) => {
                let shift =
                    u32::try_from(get(b, known)?.min(u128::from(width - 1))).unwrap_or(width - 1);
                unsigned(signed(get(a, known)?, width) >> shift, width)
            }
            Node::Extract(a, high, low) => (get(a, known)? >> low) & mask(high - low + 1),
            Node::ZeroExtend(a, _) => get(a, known)?,
            Node::SignExtend(a, _) => {
                let narrow = self.width(a);
                unsigned(signed(get(a, known)?, narrow), width)
            }
            Node::Concat(high, low) => (get(high, known)? << self.width(low)) | get(low, known)?,
            Node::Ite(condition, then, otherwise) => {
                if get(condition, known)? == 1 {
                    get(then, known)?
                } else {
                    get(otherwise, known)?
                }
            }
            Node::Eq(a, b) => u128::from(get(a, known)? == get(b, known)?),
            Node::Ult(a, b) => u128::from(get(a, known)? < get(b, known)?),
            Node::Slt(a, b) => {
                let operand_width = self.width(a);
                u128::from(
                    signed(get(a, known)?, operand_width) < signed(get(b, known)?, operand_width),
                )
            }
            Node::BoolAnd(a, b) => u128::from(get(a, known)? == 1 && get(b, known)? == 1),
            Node::BoolOr(a, b) => u128::from(get(a, known)? == 1 || get(b, known)? == 1),
            Node::BoolNot(a) => u128::from(get(a, known)? == 0),
        };
        known.insert(term, value);
        Some(value)
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
            Node::Const { .. } | Node::Var { .. } | Node::Bool(_) | Node::BoolVar(_) => Vec::new(),
            Node::Not(a)
            | Node::Neg(a)
            | Node::Extract(a, _, _)
            | Node::ZeroExtend(a, _)
            | Node::SignExtend(a, _)
            | Node::BoolNot(a) => vec![a],
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
}
