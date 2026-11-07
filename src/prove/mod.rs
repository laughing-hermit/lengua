//! The proof step: for a generated test over whole-number ranges, proves that the program prints
//! exactly what the formulas say for every input in range, not only the sampled ones.
//!
//! It splits the inputs into shapes (each variable's sign and digit count), runs the machine
//! code on terms for each shape ([`exec`]), and asks an SMT solver ([`smt`]) whether any input
//! on any path could make the output differ from the formulas ([`formula`]). A printed number is
//! matched through its digits' common root, so "prints the formula's value" becomes one linear
//! question instead of twenty divisions. What a run did beyond its machine, its output, files and
//! network, and the conditions it met, is its [`world`], which the checks of a finished run read.
//! A WebAssembly program is proven the same way on its own core, [`wasm`]: the driver explores
//! either kind of program and checks their runs alike.

pub mod exec;
pub mod formula;
pub mod induction;
pub mod smt;
pub mod term;
pub mod wasm;
pub mod watch;
pub mod world;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

use crate::checker::decode::Machine;
use crate::expr::{Generator, HexPart, PatternPart, Piece, Template, Value};
use crate::testfile::{ExitCode, Generated};
use crate::testrun::Delivery;

use exec::induction::Slot;
use exec::{End, Image, Oracle, State};
use formula::{Formula, Number, WIDTH};
use smt::{Answer, Solver};
use term::{Node, Pool, Term};
use world::World;

/// What proving a generated block found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every input in range gives exactly what the formulas say.
    Proven {
        /// Input shapes covered: each variable's sign and digit count.
        shapes: usize,
        /// Paths through the code followed.
        paths: usize,
        /// Questions the solvers answered: none when the terms themselves settled every step.
        questions: usize,
        /// Why one solver answered alone, when no second solver confirmed what it ruled out.
        alone: Option<String>,
        /// Where a proof over text stops, when it doesn't cover every length in range.
        bound: Option<Bound>,
        /// Whether the program reads standard input and its connections only through fill, so
        /// the proof covers each message cut into pieces any way, not only the tests' cuts.
        cut_any_way: bool,
        /// The functions no run of the proof called, such as a fast path for inputs longer than
        /// the proof's bound: the proof says nothing of what they do.
        unreached: Vec<String>,
        /// Where every length is proven by induction over the program's read loop: the bytes it
        /// takes each time round.
        induction: Option<usize>,
        /// The bytes before each buffer-full the induction's counts needed, as `words` needs
        /// whether the last buffer-full ended inside a word: none for maps and counts that add
        /// up as they are.
        lookback: usize,
    },
    /// Inputs where the program, as the prover reads it, disagrees with the formulas.
    Counterexample(BTreeMap<String, Value>),
    /// The proof couldn't be finished, and why.
    NotProven(String),
}

/// How far a proof over text reaches: every input whose texts are at most `bytes` long, every
/// combination of the other values included. Longer texts are tested, not proven, for `why`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    /// The longest text proven, with every shorter one.
    pub bytes: usize,
    /// The longest the idea lets the block's texts be.
    pub of: usize,
    /// Whether proving on could reach further: the time ran out, not the model.
    pub more: bool,
    /// What stopped the proof at the next length.
    pub why: String,
    /// Whether the lengths count characters, as they do where a text has characters beyond
    /// ASCII. A text of so many bytes has at most as many characters.
    pub characters: bool,
}

/// How much one proof may take.
#[derive(Debug, Clone)]
pub struct Budget {
    /// When to give up.
    pub deadline: Instant,
    /// The most paths per input shape.
    pub max_paths: usize,
    /// The most input shapes.
    pub max_shapes: usize,
    /// Each solver question's time limit, in milliseconds.
    pub question_ms: u64,
    /// Whether the program has its listener at file descriptor 3, as an idea that listens gives
    /// it.
    pub listens: bool,
    /// Once set, the proof stops at its next input combination: the code it proves changed.
    pub cancel: Arc<AtomicBool>,
    /// How far the proof has got, for whoever waits on it.
    pub progress: Arc<Progress>,
    /// Where the proof starts, when the texts below this length are proven already: a check
    /// goes on from where the last proof of the same build stopped.
    pub from_length: usize,
    /// Whether a text's length is the input's choice in one shape, every length at once, rather
    /// than one shape a length: an induction's base over its final piece.
    pub within: bool,
}

impl Budget {
    /// The usual budget for a proof that must end by `deadline`.
    #[must_use]
    pub fn until(deadline: Instant, listens: bool) -> Self {
        Self {
            deadline,
            max_paths: 256,
            max_shapes: 2_000_000,
            question_ms: 20_000,
            listens,
            cancel: Arc::default(),
            progress: Arc::default(),
            from_length: 0,
            within: false,
        }
    }

    fn stopped(&self) -> bool {
        self.cancel.load(Ordering::Relaxed) || Instant::now() > self.deadline
    }
}

/// How far a running proof has got.
#[derive(Debug, Default)]
pub struct Progress {
    /// Input combinations finished.
    pub done: AtomicUsize,
    /// Input combinations in all, once counted.
    pub total: AtomicUsize,
    /// When the proof goes through text lengths in order: one more than the longest length
    /// proven so far for every input up to it, or 0 before the first.
    pub proven_below: AtomicUsize,
}

/// How many combinations one pair of solver processes serves before it's started afresh.
const RESTART_AFTER: usize = 500;

/// The solvers one worker thread asks: started at its first question the terms don't settle,
/// and kept from one input combination to the next, reset in between.
struct Solvers {
    solver: Option<Solver>,
    question_ms: u64,
    /// Combinations since the processes started.
    served: usize,
    /// Whether the current combination asked anything.
    used: bool,
}

impl Solvers {
    const fn new(question_ms: u64) -> Self {
        Self {
            solver: None,
            question_ms,
            served: 0,
            used: false,
        }
    }

    /// Starts the next combination: the solvers forget the last one's terms.
    fn next(&mut self) {
        if std::mem::take(&mut self.used) {
            self.served += 1;
            let reset = self.served < RESTART_AFTER
                && self
                    .solver
                    .as_mut()
                    .is_some_and(|solver| solver.reset().is_ok());
            if !reset {
                self.solver = None;
                self.served = 0;
            }
        }
    }

    /// The solvers, started if they aren't yet, asking until `deadline`.
    fn get(&mut self, deadline: Instant) -> Result<&mut Solver, String> {
        if self.solver.is_none() {
            let solver = Solver::start(self.question_ms)
                .map_err(|error| format!("no SMT solver could start: {error}"))?;
            self.solver = Some(solver);
        }
        self.used = true;
        let solver = self.solver.as_mut().ok_or("no SMT solver could start")?;
        solver.until(deadline);
        Ok(solver)
    }
}

/// How many inputs a combination keeps that are known to reach a branch.
const WITNESSES: usize = 8;

/// Past this many term values, a witness forgets all but its variables' values.
const MAX_KNOWN: usize = 200_000;

/// Answers whether a branch can be taken by trying inputs known to meet a run's conditions
/// first: one that also meets the branch's condition shows the branch can be taken, so only a
/// branch that may be impossible costs a solver question. Witnesses are checked by evaluation,
/// never trusted, and a variable they don't fix yet gets a value from its declared range.
struct Witnessed<'a> {
    solvers: &'a mut Solvers,
    deadline: Instant,
    /// When a branch stops the run for time: the deadline, or the shape's least time from its
    /// start where that's later, so a short shape begun late still ends and a long one can't
    /// run far past.
    hard: Instant,
    /// Each witness's variable values, and every term value computed from them.
    witnesses: Vec<HashMap<Term, u128>>,
    rng: ChaCha8Rng,
    /// Whether a branch no witness reaches is followed without asking whether it can happen:
    /// the run's own end settles that, as its output check or one question about its path.
    lazy: bool,
    /// Whether some branch was followed that way.
    guessed: bool,
}

impl<'a> Witnessed<'a> {
    fn new(
        solvers: &'a mut Solvers,
        (deadline, hard): (Instant, Instant),
        seed: u64,
        lazy: bool,
    ) -> Self {
        Self {
            solvers,
            deadline,
            hard,
            lazy,
            guessed: false,
            // Every variable at the low end of its range, every one at the high end, and two
            // drawn at random: a branch on a large or a small value finds one of them.
            witnesses: vec![
                HashMap::from([(Term::LOW, 0)]),
                HashMap::from([(Term::HIGH, 0)]),
                HashMap::new(),
                HashMap::new(),
            ],
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }
}

/// A value for `variable` from its declared range: its low end for a witness marked
/// [`Term::LOW`], its high end for one marked [`Term::HIGH`], otherwise drawn at random.
fn random_in(pool: &Pool, variable: Term, rng: &mut ChaCha8Rng, end: Option<bool>) -> u128 {
    // A text's byte from its characters, which a draw from its range would mostly miss.
    if let Some(values) = pool.allowed(variable).filter(|values| !values.is_empty()) {
        let at = match end {
            Some(false) => 0,
            Some(true) => values.len() - 1,
            None => rng.random_range(0..values.len()),
        };
        return u128::from(values[at]);
    }
    let (low, high) = pool.range(variable);
    match (pool.node(variable), end) {
        (_, Some(false)) => low,
        (_, Some(true)) => high,
        (Node::BoolVar(_), None) => u128::from(rng.random::<bool>()),
        _ => rng.random_range(low..=high),
    }
}

/// Whether `witness` meets every one of `conditions`, giving variables it doesn't fix yet
/// values from their declared ranges.
fn meets(
    pool: &Pool,
    conditions: &[Term],
    witness: &mut HashMap<Term, u128>,
    rng: &mut ChaCha8Rng,
) -> bool {
    let end = if witness.contains_key(&Term::LOW) {
        Some(false)
    } else if witness.contains_key(&Term::HIGH) {
        Some(true)
    } else {
        None
    };
    // Past the most it keeps, a witness forgets what it worked out but its variables and its
    // counts: a branch on a long text's count, as a printed number's digits are, reads the count
    // again at once rather than every part of it.
    if witness.len() > MAX_KNOWN {
        witness.retain(|term, _| {
            *term == Term::LOW
                || *term == Term::HIGH
                || matches!(
                    pool.node(*term),
                    Node::Var { .. } | Node::BoolVar(_) | Node::Count(..)
                )
        });
    }
    // The newest condition first: it's the one most likely to fail.
    conditions.iter().rev().all(|condition| {
        pool.evaluate_cached(
            *condition,
            &mut |pool, variable| Some(random_in(pool, variable, rng, end)),
            witness,
        ) == Some(1)
    })
}

/// What a comparison with a constant says of a term: it lies from `low` to `high`, read signed
/// or unsigned.
#[derive(Debug, Clone, Copy)]
struct Bounds {
    signed: bool,
    low: i128,
    high: i128,
}

/// The term a condition compares with a constant, and what the condition says of it.
fn bounds_of(pool: &Pool, condition: Term) -> Option<(Term, Bounds)> {
    let (inner, holds) = match *pool.node(condition) {
        Node::BoolNot(inner) => (inner, false),
        _ => (condition, true),
    };
    let width = pool.width(pool.operands(inner).first().copied()?);
    if width == 0 || width > 128 {
        return None;
    }
    let (signed_min, signed_max) = if width == 128 {
        (i128::MIN, i128::MAX)
    } else {
        (-(1_i128 << (width - 1)), (1_i128 << (width - 1)) - 1)
    };
    // Unsigned values of 127 bits and more don't fit an i128: their top stands for every value
    // above, which is all a bound from below needs, since the values compared with it are
    // whole numbers that do fit.
    let unsigned_max = if width >= 127 {
        i128::MAX
    } else {
        (1_i128 << width) - 1
    };
    let constant = |term: Term, signed: bool| {
        pool.constant(term).and_then(|value| {
            let value = i128::try_from(value).ok()?;
            Some(if signed && value > signed_max {
                value - (1 << width)
            } else {
                value
            })
        })
    };
    let (term, signed, low, high) = match *pool.node(inner) {
        Node::Ult(a, b) | Node::Slt(a, b) => {
            let signed = matches!(pool.node(inner), Node::Slt(..));
            let (min, max) = if signed {
                (signed_min, signed_max)
            } else {
                (0, unsigned_max)
            };
            match (constant(a, signed), constant(b, signed), holds) {
                (None, Some(k), true) => (a, signed, min, k - 1),
                (None, Some(k), false) => (a, signed, k, max),
                (Some(k), None, true) => (b, signed, k + 1, max),
                (Some(k), None, false) => (b, signed, min, k),
                _ => return None,
            }
        }
        Node::Eq(a, b) if holds && pool.width(a) > 0 && width < 127 => {
            match (constant(a, false), constant(b, false)) {
                (None, Some(k)) => (a, false, k, k),
                (Some(k), None) => (b, false, k, k),
                _ => return None,
            }
        }
        _ => return None,
    };
    Some((term, Bounds { signed, low, high }))
}

/// Whole numbers, as intervals.
type Values = Vec<(i128, i128)>;

/// The numbers in both `a` and `b`.
fn both(a: &Values, b: &Values) -> Values {
    let mut found = Vec::new();
    for (low, high) in a {
        for (from, to) in b {
            let (start, end) = (*low.max(from), *high.min(to));
            if start <= end {
                found.push((start, end));
            }
        }
    }
    found
}

/// What `bounds` say of a `width`-bit value read signed: one interval, or two when unsigned
/// bounds reach past the sign bit. `None` where the reading doesn't fit.
fn reading(bounds: Bounds, width: u32) -> Option<Values> {
    if bounds.signed {
        return Some(vec![(bounds.low, bounds.high)]);
    }
    match width {
        // An unsigned top of i128::MAX stands for every value above: the negatives too.
        128 => Some(if bounds.high == i128::MAX {
            vec![(bounds.low, i128::MAX), (i128::MIN, -1)]
        } else {
            vec![(bounds.low, bounds.high)]
        }),
        ..=126 => {
            let (span, half) = (1_i128 << width, 1_i128 << (width - 1));
            let mut values = Vec::new();
            if bounds.low < half {
                values.push((bounds.low.max(0), bounds.high.min(half - 1)));
            }
            if bounds.high >= half {
                values.push((bounds.low.max(half) - span, bounds.high - span));
            }
            Some(values)
        }
        _ => None,
    }
}

/// `values` as few intervals as cover the same numbers, in order.
fn merged(mut values: Values) -> Values {
    values.sort_unstable();
    let mut found: Values = Vec::with_capacity(values.len());
    for (low, high) in values {
        match found.last_mut() {
            Some((_, end)) if low <= end.saturating_add(1) => *end = (*end).max(high),
            _ => found.push((low, high)),
        }
    }
    found
}

/// The numbers from `min` to `max` not in `values`.
fn complement(values: &Values, min: i128, max: i128) -> Values {
    let mut found = Vec::new();
    // The least number not yet covered: none once every number up to i128::MAX is.
    let mut start = Some(min);
    for (low, high) in merged(values.clone()) {
        let Some(from) = start else {
            break;
        };
        if low > from {
            found.push((from, (low - 1).min(max)));
        }
        if high >= from {
            start = high.checked_add(1);
        }
    }
    if let Some(from) = start {
        found.push((from, max));
    }
    found.retain(|(low, high)| low <= high);
    found
}

/// The term `condition` compares, and the numbers it allows, read signed at the term's width: a
/// comparison with a constant, or comparisons of one term joined by and, or and not, as a byte's
/// character set is.
fn values_of(pool: &Pool, condition: Term) -> Option<(Term, Values)> {
    values_within(pool, condition, 8)
}

/// The term `condition` says which values of, as [`values_within`] finds it, without working out
/// the values.
fn subject_within(pool: &Pool, condition: Term, depth: u32) -> Option<Term> {
    if let Some((x, _)) = bounds_of(pool, condition) {
        return Some(x);
    }
    if depth == 0 {
        return None;
    }
    match *pool.node(condition) {
        Node::Lookup(byte, _) => Some(byte),
        Node::BoolOr(a, b) | Node::BoolAnd(a, b) => {
            let x = subject_within(pool, a, depth - 1)?;
            (subject_within(pool, b, depth - 1)? == x).then_some(x)
        }
        Node::BoolNot(inner) => subject_within(pool, inner, depth - 1),
        _ => None,
    }
}

fn values_within(pool: &Pool, condition: Term, depth: u32) -> Option<(Term, Values)> {
    if let Some((x, bounds)) = bounds_of(pool, condition) {
        return Some((x, reading(bounds, pool.width(x))?));
    }
    if depth == 0 {
        return None;
    }
    match *pool.node(condition) {
        Node::Lookup(byte, number) => {
            let table = pool.table(number);
            let values = (0..256_i128)
                .filter(|value| table[(value / 64) as usize] >> (value % 64) & 1 == 1)
                .map(|value| {
                    let read = if value >= 128 { value - 256 } else { value };
                    (read, read)
                })
                .collect();
            Some((byte, merged(values)))
        }
        Node::BoolOr(a, b) | Node::BoolAnd(a, b) => {
            let (x, left) = values_within(pool, a, depth - 1)?;
            let (y, right) = values_within(pool, b, depth - 1)?;
            if x != y {
                return None;
            }
            let values = if matches!(pool.node(condition), Node::BoolOr(..)) {
                merged([left, right].concat())
            } else {
                both(&left, &right)
            };
            Some((x, values))
        }
        Node::BoolNot(inner) => {
            let (x, values) = values_within(pool, inner, depth - 1)?;
            let (min, max) = match pool.width(x) {
                128 => (i128::MIN, i128::MAX),
                width @ 1..128 => (-(1_i128 << (width - 1)), (1_i128 << (width - 1)) - 1),
                _ => return None,
            };
            Some((x, complement(&values, min, max)))
        }
        _ => None,
    }
}

/// `values` of y moved to x: `x = -y + offset` when `negated`, else `x = y + offset`, as whole
/// numbers.
fn moved(values: &Values, negated: bool, offset: i128) -> Option<Values> {
    values
        .iter()
        .map(|(low, high)| {
            if negated {
                Some((offset.checked_sub(*high)?, offset.checked_sub(*low)?))
            } else {
                Some((low.checked_add(offset)?, high.checked_add(offset)?))
            }
        })
        .collect()
}

/// `values` times `scale`, plus `offset`: every value the multiples can be, and the ones
/// between, which a refutation may count too.
fn scaled(values: &Values, scale: i128, offset: i128) -> Option<Values> {
    values
        .iter()
        .map(|(low, high)| {
            let from = low.checked_mul(scale)?.checked_add(offset)?;
            let to = high.checked_mul(scale)?.checked_add(offset)?;
            Some((from.min(to), from.max(to)))
        })
        .collect()
}

/// Whether the path's comparisons rule `condition` out: comparisons of the same sum, its
/// negation, or either plus a constant, at the condition's width or another, taken together. A
/// program that took a sign's branch then compares the number's size: `-x < 10^7` can't fail
/// where `x < 0` and `x` is at least `-3000000`.
fn refuted(pool: &Pool, path: &[Term], condition: Term) -> bool {
    let Some((x, wanted)) = values_of(pool, condition) else {
        return false;
    };
    let parts = pool.parts(x);
    refuted_among(pool, path.iter().copied(), x, &wanted, &parts)
}

/// A run's conditions, each under the parts of the value it bounds, so a question about one
/// value reads only those that can say something of it: a text's thousands of conditions on its
/// bytes, one each, aren't read again for every byte.
struct Indexed<'a> {
    path: &'a [Term],
    /// The conditions whose value has no parts, or that bound no one value: read for any.
    general: Vec<usize>,
    /// The others, under each of their value's parts.
    by_part: HashMap<Term, Vec<usize>>,
}

impl<'a> Indexed<'a> {
    fn new(pool: &Pool, path: &'a [Term]) -> Self {
        let mut general = Vec::new();
        let mut by_part: HashMap<Term, Vec<usize>> = HashMap::new();
        for (index, condition) in path.iter().enumerate() {
            let parts = subject_within(pool, *condition, 8).map(|y| pool.parts(y));
            match parts {
                Some(parts) if !parts.is_empty() => {
                    for part in parts {
                        by_part.entry(part).or_default().push(index);
                    }
                }
                _ => general.push(index),
            }
        }
        Self {
            path,
            general,
            by_part,
        }
    }

    /// [`refuted`], reading only the conditions that can bound the condition's value.
    fn refutes(&self, pool: &Pool, condition: Term) -> bool {
        let Some((x, wanted)) = values_of(pool, condition) else {
            return false;
        };
        let parts = pool.parts(x);
        if parts.is_empty() {
            return refuted_among(pool, self.path.iter().copied(), x, &wanted, &parts);
        }
        let mut indices = self.general.clone();
        for part in &parts {
            if let Some(found) = self.by_part.get(part) {
                indices.extend(found);
            }
        }
        indices.sort_unstable();
        indices.dedup();
        let conditions = indices.into_iter().map(|index| self.path[index]);
        refuted_among(pool, conditions, x, &wanted, &parts)
    }
}

/// Whether `conditions` rule out every one of `wanted`, the values `x`, made of `parts`, must
/// take for some condition to hold.
fn refuted_among(
    pool: &Pool,
    conditions: impl Iterator<Item = Term>,
    x: Term,
    wanted: &Values,
    parts: &[Term],
) -> bool {
    let width = pool.width(x);
    let mut known: Values = vec![pool.signed_range(x)];
    // Only a value made of some of x's parts can be x moved or scaled: a long text's
    // conditions on each of its bytes say nothing of its checksum, and are passed over unread.
    for path_condition in conditions {
        let path_condition = &path_condition;
        if let Some(y) = subject_within(pool, *path_condition, 8)
            && !pool.shares_parts(parts, y)
        {
            continue;
        }
        let Some((y, held)) = values_of(pool, *path_condition) else {
            continue;
        };
        let values = both(&held, &vec![pool.signed_range(y)]);
        let moved = if let Some((negated, delta)) = pool.related_wider(x, y) {
            moved(&values, negated, delta)
        } else if let Some((scale, offset)) = pool
            .related_scaled(x, y)
            .filter(|(scale, _)| scale.unsigned_abs() > 1)
        {
            scaled(&values, scale, offset)
        } else if width <= 126
            && let Some((negated, offset)) = pool.related(x, y)
        {
            // Modulo 2^width: each interval moved, then into the window the bits read signed.
            let span = 1_i128 << width;
            let offset = i128::try_from(offset).unwrap_or(0);
            let offset = if offset >= span / 2 {
                offset - span
            } else {
                offset
            };
            moved(&values, negated, offset).and_then(|values| {
                values
                    .into_iter()
                    .map(|(low, high)| {
                        let window = (low + span / 2).div_euclid(span);
                        ((high + span / 2).div_euclid(span) == window)
                            .then(|| (low - window * span, high - window * span))
                    })
                    .collect::<Option<Values>>()
            })
        } else {
            continue;
        };
        let Some(moved) = moved else {
            continue;
        };
        known = both(&known, &moved);
        if known.is_empty() {
            return true;
        }
    }
    both(&known, wanted).is_empty()
}

/// The most terms [`settled`] reads of an output check: enough for a count of every byte of a
/// 64 KiB buffer-full, each guarded by whether it's inside the text.
const MAX_SETTLED: usize = 1_000_000;

/// The most times [`settled`] splits an output check on a condition the path leaves open.
const MAX_CHOICES: u32 = 2;

/// What `holds` comes to where `path` holds: comparisons the path's own comparisons settle, as
/// the formula's sign of the same number the program tested, written at another width, a few
/// rounds since settling a sign turns a size into the number itself. Where a condition the path
/// speaks of but leaves open still chooses, as the sign of a total that may be 0 chooses its size,
/// each choice is settled on its own, with the condition on the path.
fn settled(pool: &mut Pool, path: &[Term], holds: Term, splits: u32) -> Term {
    settled_with(pool, path, holds, splits, &mut HashMap::new())
}

/// `condition`, one of a run's own, as `facts` rewrite what's inside it, never itself: down
/// through its own negations and conjunctions, each part built again from its operands as the
/// facts rewrite them, `done` shared by all the run's conditions. Past a few levels it's kept.
fn rewritten_condition(
    pool: &mut Pool,
    condition: Term,
    facts: &HashMap<Term, bool>,
    done: &mut HashMap<Term, Term>,
    depth: usize,
) -> Term {
    if depth > MAX_REWRITTEN_DEPTH {
        return condition;
    }
    match *pool.node(condition) {
        Node::BoolNot(inner) => {
            let inner = rewritten_condition(pool, inner, facts, done, depth + 1);
            pool.bool_not(inner)
        }
        Node::BoolAnd(a, b) => {
            let a = rewritten_condition(pool, a, facts, done, depth + 1);
            let b = rewritten_condition(pool, b, facts, done, depth + 1);
            pool.bool_and(a, b)
        }
        _ => {
            let operands = pool.operands(condition);
            if operands.is_empty() {
                return condition;
            }
            let rebuilt: Vec<Term> = operands
                .iter()
                .map(|operand| pool.assuming_with(*operand, facts, done))
                .collect();
            if rebuilt == operands {
                condition
            } else {
                pool.with_operands(condition, &rebuilt)
            }
        }
    }
}

/// How deep in a run's condition's own negations and conjunctions [`rewritten_condition`] goes.
const MAX_REWRITTEN_DEPTH: usize = 16;

/// What `path`'s conditions say, as facts [`Pool::assuming`] reads: each condition, and each part
/// of one that's a conjunction, true; what a negated one negates false.
fn path_facts(pool: &mut Pool, path: &[Term]) -> HashMap<Term, bool> {
    let mut facts = HashMap::new();
    let mut pending = path.to_vec();
    while let Some(condition) = pending.pop() {
        match *pool.node(condition) {
            Node::BoolAnd(a, b) => pending.extend([a, b]),
            Node::BoolNot(inner) => {
                facts.insert(inner, false);
            }
            Node::Bool(_) => {}
            _ => {
                facts.insert(condition, true);
                // A test of one byte has its complement as its own table.
                let complement = pool.bool_not(condition);
                facts.entry(complement).or_insert(false);
            }
        }
    }
    facts
}

/// [`settled`], noting in `derived` the comparisons in `holds` the path settles, beyond the
/// path's own conditions: the facts that rewrite the path's terms the same way.
fn settled_with(
    pool: &mut Pool,
    path: &[Term],
    holds: Term,
    splits: u32,
    derived: &mut HashMap<Term, bool>,
) -> Term {
    if pool.truth(holds).is_some() {
        return holds;
    }
    let mut facts = path_facts(pool, path);
    // The path's own conditions first, which settle a check made of them, such as a printed
    // number's digit count, at its top, without walking the long counts inside them.
    let mut holds = pool.assuming(holds, &facts);
    if pool.truth(holds).is_some() {
        return holds;
    }
    let indexed = Indexed::new(pool, path);
    for _ in 0..4 {
        let before = holds;
        let mut seen = HashSet::new();
        let mut pending = vec![holds];
        while let Some(term) = pending.pop() {
            if !seen.insert(term) || seen.len() > MAX_SETTLED {
                continue;
            }
            if pool.sort(term) == term::Sort::Bool
                && !facts.contains_key(&term)
                && values_of(pool, term).is_some()
            {
                let negated = pool.bool_not(term);
                if indexed.refutes(pool, term) {
                    facts.insert(term, false);
                    derived.insert(term, false);
                } else if indexed.refutes(pool, negated) {
                    facts.insert(term, true);
                    derived.insert(term, true);
                }
            }
            pending.extend(pool.operands(term));
        }
        holds = pool.assuming(holds, &facts);
        if holds == before {
            break;
        }
    }
    if splits == 0 || pool.truth(holds).is_some() {
        return holds;
    }
    let Some(open) = open_choice(pool, path, holds) else {
        return holds;
    };
    let side = |pool: &mut Pool, truth: bool| {
        let condition = if truth { open } else { pool.bool_not(open) };
        let mut path = path.to_vec();
        path.push(condition);
        settled(pool, &path, holds, splits - 1)
    };
    let (when, otherwise) = (side(pool, true), side(pool, false));
    if when == otherwise {
        return when;
    }
    let not_open = pool.bool_not(open);
    let when = pool.bool_and(open, when);
    let otherwise = pool.bool_and(not_open, otherwise);
    pool.bool_or(when, otherwise)
}

/// A condition in `holds` that chooses between values, and that compares a number one of the
/// path's comparisons speaks of: the first such, nearest the top.
fn open_choice(pool: &Pool, path: &[Term], holds: Term) -> Option<Term> {
    let spoken: Vec<Term> = path
        .iter()
        .filter_map(|known| values_of(pool, *known).map(|(y, _)| y))
        .collect();
    let mut seen = HashSet::new();
    let mut pending = std::collections::VecDeque::from([holds]);
    while let Some(term) = pending.pop_front() {
        if !seen.insert(term) || seen.len() > 20_000 {
            continue;
        }
        if let Node::Ite(condition, ..) = *pool.node(term)
            && let Some((x, _)) = values_of(pool, condition)
            && spoken
                .iter()
                .any(|y| pool.related_wider(x, *y).is_some() || pool.related(x, *y).is_some())
        {
            return Some(condition);
        }
        pending.extend(pool.operands(term));
    }
    None
}

/// The most evaluations [`search`] makes for one condition.
const MAX_SEARCH: usize = 400;

/// The most variable values [`search`] goes through in all its evaluations: each evaluation
/// starts from every variable's value, so a condition on a long text's every byte, such as its
/// checksum or count, gets a few tries, where moving one byte seldom turns it anyway, and a
/// condition on a few numbers gets [`MAX_SEARCH`].
const MAX_SEARCH_VALUES: usize = MAX_SEARCH * 64;

/// The least time a shape gets to end once it has begun, past its block's deadline: a shape
/// that's quick but begun late, as after a mebibyte's samples ran, still ends; a long one stops
/// this long past at most.
const MIN_SHAPE_TIME: Duration = Duration::from_secs(20);

/// How many texts drawn at random, as long as what a step that went wrong read, are run from the
/// program's start for one that shows it.
const REPLAYED_TEXTS: usize = 8;

/// How many times a proof's paths the base that takes every final piece at once may follow.
const AT_ONCE_PATHS: usize = 4;

/// Inputs that meet every one of `conditions`, found by moving one variable of the newest of
/// them in `witness`: to its range's ends and middle, and to the edge where the newest condition
/// turns true, found by halving. A sum or a division compared with a constant turns at one
/// place, which inputs drawn at random seldom land on, as when a number of days must come out
/// below 10.
fn search(
    pool: &Pool,
    conditions: &[Term],
    witness: &HashMap<Term, u128>,
    rng: &mut ChaCha8Rng,
) -> Option<HashMap<Term, u128>> {
    let newest = *conditions.last()?;
    // Every variable fixed once, so each evaluation sees the same inputs but the one moved.
    let mut base: HashMap<Term, u128> = pool
        .variables(conditions)
        .into_iter()
        .map(|variable| {
            let value = witness
                .get(&variable)
                .copied()
                .unwrap_or_else(|| random_in(pool, variable, rng, None));
            (variable, value)
        })
        .collect();
    let mut left = (MAX_SEARCH_VALUES / base.len().max(1)).clamp(4, MAX_SEARCH);
    let truth = |values: &HashMap<Term, u128>, terms: &[Term], left: &mut usize| {
        *left = left.saturating_sub(1);
        let mut known = values.clone();
        terms
            .iter()
            .rev()
            .all(|term| pool.evaluate_cached(*term, &mut |_, _| Some(0), &mut known) == Some(1))
    };
    let moving: Vec<Term> = pool
        .variables(&[newest])
        .into_iter()
        .filter(|variable| matches!(pool.node(*variable), Node::Var { .. }))
        .collect();
    for variable in moving {
        let (low, high) = pool.range(variable);
        let current = base.get(&variable).copied().unwrap_or(low);
        let mut tried: Vec<(u128, bool)> = Vec::new();
        for value in [low, high, low + (high - low) / 2, current] {
            if left == 0 {
                return None;
            }
            base.insert(variable, value);
            let newest_holds = truth(&base, &[newest], &mut left);
            if newest_holds && truth(&base, conditions, &mut left) {
                return Some(base);
            }
            tried.push((value, newest_holds));
        }
        tried.sort_unstable();
        // Halve between two values the newest condition disagrees on, down to the edge.
        let Some(pair) = tried.windows(2).find(|pair| pair[0].1 != pair[1].1) else {
            base.insert(variable, current);
            continue;
        };
        let (mut below, mut above) = ((pair[0].0, pair[0].1), (pair[1].0, pair[1].1));
        while above.0 - below.0 > 1 && left > 0 {
            let middle = below.0 + (above.0 - below.0) / 2;
            base.insert(variable, middle);
            let holds = truth(&base, &[newest], &mut left);
            if holds == below.1 {
                below = (middle, holds);
            } else {
                above = (middle, holds);
            }
        }
        let edge = if below.1 { below.0 } else { above.0 };
        // The edge, and a few values past it on the side where the condition holds.
        let step: i128 = if below.1 { -1 } else { 1 };
        for offset in 0..4_i128 {
            let Some(value) = u128::try_from(i128::try_from(edge).ok()? + step * offset)
                .ok()
                .filter(|value| (low..=high).contains(value))
            else {
                break;
            };
            base.insert(variable, value);
            if truth(&base, conditions, &mut left) {
                return Some(base);
            }
        }
        base.insert(variable, current);
    }
    None
}

impl Oracle for Witnessed<'_> {
    fn in_time(&self) -> Result<(), String> {
        if Instant::now() > self.hard {
            return Err(OUT_OF_TIME.into());
        }
        Ok(())
    }

    fn possible(&mut self, pool: &Pool, path: &[Term], condition: Term) -> Result<bool, String> {
        // A shape that takes every final piece at once can be long without a question: its
        // time is kept at each branch too.
        if Instant::now() > self.hard {
            return Err(OUT_OF_TIME.into());
        }
        if refuted(pool, path, condition) {
            return Ok(false);
        }
        let mut conditions = path.to_vec();
        conditions.push(condition);
        let rng = &mut self.rng;
        if self
            .witnesses
            .iter_mut()
            .any(|witness| meets(pool, &conditions, witness, rng))
        {
            return Ok(true);
        }
        if let Some(found) = self
            .witnesses
            .first()
            .and_then(|witness| search(pool, &conditions, witness, rng))
        {
            if self.witnesses.len() >= WITNESSES {
                self.witnesses.remove(0);
            }
            self.witnesses.push(found);
            return Ok(true);
        }
        if self.lazy {
            self.guessed = true;
            return Ok(true);
        }
        if std::env::var_os("LENGUA_PROVE_EXPLAIN").is_some() {
            eprintln!("a branch asks: {}", pool.sketch(condition, 6));
        }
        let solver = self.solvers.get(self.deadline)?;
        let (answer, model) = solver
            .witness(pool, &conditions)
            .map_err(|error| format!("the solver failed: {error}"))?;
        if solver.out_of_time() {
            return Err("the proof ran out of time".into());
        }
        if answer == Answer::Sat && !model.is_empty() {
            if self.witnesses.len() >= WITNESSES {
                self.witnesses.remove(0);
            }
            self.witnesses.push(model);
        }
        Ok(answer != Answer::Unsat)
    }
}

/// One piece of a variable's values: its sign, and the range of each digit, most significant
/// first. Every number the digit ranges allow is in the generator's range, so value ranges alone
/// can settle most of what the code does with it.
#[derive(Debug, Clone)]
struct Shape {
    negative: bool,
    digits: Vec<(u8, u8)>,
    low: i128,
    high: i128,
}

/// The digits of `value`, most significant first, padded with zeros to `count`.
fn digits_of(value: u128, count: usize) -> Vec<u8> {
    let text = format!("{value:0count$}");
    text.bytes().map(|byte| byte - b'0').collect()
}

/// Digit ranges that together cover exactly the numbers from `low` to `high`, which have the
/// same number of digits: like a regular expression for a number range.
fn cover(low: &[u8], high: &[u8]) -> Vec<Vec<(u8, u8)>> {
    let Some((&first, low_rest)) = low.split_first() else {
        return vec![Vec::new()];
    };
    let (&last, high_rest) = high.split_first().unwrap_or((&first, &[]));
    let with = |digit: (u8, u8), tails: Vec<Vec<(u8, u8)>>| -> Vec<Vec<(u8, u8)>> {
        tails
            .into_iter()
            .map(|mut tail| {
                tail.insert(0, digit);
                tail
            })
            .collect()
    };
    if first == last {
        return with((first, first), cover(low_rest, high_rest));
    }
    let rest = low_rest.len();
    let mut pieces = Vec::new();
    let mut from = first;
    if low_rest.iter().any(|digit| *digit != 0) {
        pieces.extend(with((first, first), cover(low_rest, &vec![9; rest])));
        from = first + 1;
    }
    let mut to = last;
    let mut upper = Vec::new();
    if high_rest.iter().any(|digit| *digit != 9) {
        upper = with((last, last), cover(&vec![0; rest], high_rest));
        to = last - 1;
    }
    if from <= to {
        let mut middle = vec![(from, to)];
        middle.extend(std::iter::repeat_n((0, 9), rest));
        pieces.push(middle);
    }
    pieces.extend(upper);
    pieces
}

/// Every shape of the values from `low` to `high`.
fn shapes(low: i128, high: i128) -> Vec<Shape> {
    let mut shapes = Vec::new();
    for negative in [true, false] {
        for count in 1..=38_u32 {
            let Some(top) = 10_i128.checked_pow(count) else {
                break;
            };
            let smallest = match (count, negative) {
                (1, true) => 1,
                (1, false) => 0,
                _ => top / 10,
            };
            // The magnitudes with this many digits that are in range.
            let (from, to) = if negative {
                ((-high).max(smallest), (-low).min(top - 1))
            } else {
                (low.max(smallest), high.min(top - 1))
            };
            if from > to {
                continue;
            }
            let digits = count as usize;
            let (from, to) = (from.unsigned_abs(), to.unsigned_abs());
            for piece in cover(&digits_of(from, digits), &digits_of(to, digits)) {
                let bound = |pick: fn(&(u8, u8)) -> u8| {
                    piece
                        .iter()
                        .fold(0_i128, |value, digit| value * 10 + i128::from(pick(digit)))
                };
                let (smallest, largest) = (bound(|digit| digit.0), bound(|digit| digit.1));
                let (low, high) = if negative {
                    (-largest, -smallest)
                } else {
                    (smallest, largest)
                };
                shapes.push(Shape {
                    negative,
                    digits: piece,
                    low,
                    high,
                });
            }
        }
    }
    shapes
}

/// The values of one variable a proof covers at once: a shape of whole numbers, or one value of
/// a `choice(…)`, whose text is fixed.
#[derive(Debug, Clone)]
enum Part {
    /// Whole numbers, or decimals' digits with this many places.
    Number(Shape, u8),
    Fixed(Value),
    /// Text of this many bytes, each one of the generator's characters.
    Text(usize, Vec<u8>),
    /// Text of any length from the first to the second, the input's choice, its bytes the
    /// generator's characters.
    Within(usize, usize, Vec<u8>),
    /// Text beyond ASCII: a character of each of these widths in bytes, each one of the
    /// generator's characters that wide.
    Utf8(Vec<u8>, std::sync::Arc<[char]>),
    /// A number the program reads only as this many random bytes: one shape, a variable a byte.
    Bytes(usize),
    /// A number the program reads only as a clock reading, from the first to the second, with
    /// this many decimal places: one shape, since its digits are never read.
    Plain(i128, i128, u8),
    /// How what the test's peer sends arrives: whole, or cut as a run of the tests may cut it.
    Delivery(Delivery),
}

/// The name a counterexample gives the delivery it was found with.
pub const DELIVERY: &str = "(delivery)";

/// The name a counterexample gives the memory rule it breaks, when it breaks one rather than a
/// formula.
pub const UNSAFE: &str = "(unsafe)";

pub use crate::edge::PIPE_WRITE;

/// The most text lengths one proof lays out past where it starts, more than its time reaches: a
/// text is proven up to the length the idea gives it, shortest first, as far as the proof's time
/// goes, and the next check goes on from where it stopped.
const WINDOW: usize = 1 << 16;

/// The longest length of a text from `min` to `max` long that a proof starting at `from` lays
/// out.
fn laid_out(min: usize, max: usize, from: usize) -> usize {
    max.min(min.max(from).saturating_add(WINDOW))
}

/// Texts this long or shorter have each byte held to the generator's characters; longer ones
/// only to the range the characters span, which proves more than needed but nothing false.
const MAX_CHECKED_TEXT: usize = 4096;

/// The parts that cover every value `generator` draws, texts up to the length a proof starting
/// at `from` lays out.
fn parts(name: &str, generator: &Generator, from: usize) -> Result<Vec<Part>, String> {
    match generator {
        Generator::Int { lo, hi } => Ok(shapes(*lo, *hi)
            .into_iter()
            .map(|shape| Part::Number(shape, 0))
            .collect()),
        Generator::Dec { lo, hi, places } => Ok(shapes(*lo, *hi)
            .into_iter()
            .map(|shape| Part::Number(shape, *places))
            .collect()),
        Generator::Choice(values) => {
            // A value listed twice is proven once.
            let mut distinct: Vec<Value> = Vec::new();
            for value in values {
                if !distinct.contains(value) {
                    distinct.push(value.clone());
                }
            }
            Ok(distinct.into_iter().map(Part::Fixed).collect())
        }
        Generator::Str { min, max, charset } => {
            if !charset.iter().all(char::is_ascii) {
                return utf8_parts(name, *min, *max, charset);
            }
            let bytes: Vec<u8> = charset.iter().map(|character| *character as u8).collect();
            Ok((*min..=laid_out(*min, *max, from))
                .map(|length| Part::Text(length, bytes.clone()))
                .collect())
        }
    }
}

/// About how many input combinations a proof of `block` goes through, for proving small blocks
/// first; a block whose inputs can't be split counts as huge.
#[must_use]
pub fn size(block: &Generated) -> usize {
    block
        .vars
        .iter()
        .map(|(name, generator)| parts(name, generator, 0).map_or(usize::MAX, |parts| parts.len()))
        .fold(1, usize::saturating_mul)
}

/// The most shapes a text beyond ASCII is proven over: one for each order of its characters'
/// widths, so the longest text proven depends on how many widths its characters have.
const MAX_UTF8_SHAPES: usize = 1 << 14;

/// How many characters a text beyond ASCII of `min` to `max` characters, from `charset`, is
/// proven up to, and what the proof says past it when that stops short of `max`.
fn utf8_reach(min: usize, max: usize, charset: &[char]) -> (usize, Option<String>) {
    let widths: std::collections::BTreeSet<usize> = charset
        .iter()
        .map(|character| character.len_utf8())
        .collect();
    let widest = widths.iter().max().copied().unwrap_or(1);
    let mut shapes = 0_usize;
    let mut longest = min;
    for length in min..=max.min(PIPE_WRITE / widest) {
        let these = widths
            .len()
            .checked_pow(u32::try_from(length).unwrap_or(u32::MAX));
        match these.and_then(|these| shapes.checked_add(these)) {
            Some(total) if total <= MAX_UTF8_SHAPES || length == min => {
                shapes = total;
                longest = length;
            }
            _ => break,
        }
    }
    let why = (longest < max).then(|| {
        format!(
            "a proof covers text beyond ASCII up to {longest} characters, each of its {} widths \
             in every order",
            widths.len()
        )
    });
    (longest, why)
}

/// The parts that cover text of `min` to `max` characters from `charset`, which has characters
/// beyond ASCII: for each length, every order of its characters' widths, shortest first.
fn utf8_parts(name: &str, min: usize, max: usize, charset: &[char]) -> Result<Vec<Part>, String> {
    let mut widths: Vec<u8> = charset
        .iter()
        .map(|character| character.len_utf8() as u8)
        .collect();
    widths.sort_unstable();
    widths.dedup();
    for width in &widths {
        let count = charset
            .iter()
            .filter(|character| character.len_utf8() == usize::from(*width))
            .count();
        if count > 256 {
            return Err(format!(
                "{name} is drawn from more than 256 characters {width} bytes long"
            ));
        }
    }
    let characters: std::sync::Arc<[char]> = charset.iter().copied().collect();
    let (longest, _) = utf8_reach(min, max, charset);
    let mut parts = Vec::new();
    for length in min..=longest {
        let mut order = vec![0_usize; length];
        loop {
            parts.push(Part::Utf8(
                order.iter().map(|at| widths[*at]).collect(),
                std::sync::Arc::clone(&characters),
            ));
            // The next order, the last character's width fastest.
            let Some(position) = order.iter().rposition(|at| at + 1 < widths.len()) else {
                break;
            };
            order[position] += 1;
            for later in &mut order[position + 1..] {
                *later = 0;
            }
        }
    }
    Ok(parts)
}

/// The input for text of one character per entry of `widths`, each that many bytes and one of
/// `characters` that wide. An ASCII character is a byte variable, as in any text; a wider one is
/// a variable choosing among the characters of its width, and each of its bytes is that choice's
/// byte, so a test of a byte is a test of the choice.
fn utf8_input(pool: &mut Pool, name: &str, widths: &[u8], characters: &[char]) -> Input {
    let mut by_width: BTreeMap<u8, Vec<[u8; 4]>> = BTreeMap::new();
    for character in characters {
        let mut encoded = [0; 4];
        character.encode_utf8(&mut encoded);
        by_width
            .entry(character.len_utf8() as u8)
            .or_default()
            .push(encoded);
    }
    let ascii: Vec<u8> = by_width
        .get(&1)
        .map(|encoded| encoded.iter().map(|bytes| bytes[0]).collect())
        .unwrap_or_default();
    let (lowest, highest) = (
        ascii.iter().copied().min().unwrap_or(0),
        ascii.iter().copied().max().unwrap_or(0),
    );
    let mut text = Vec::new();
    let mut constraints = Vec::new();
    for (index, width) in widths.iter().enumerate() {
        let variable = format!("{name}.c{index}");
        if *width == 1 {
            let byte = pool.var_in(8, &variable, u128::from(lowest), u128::from(highest));
            pool.allow(byte, &ascii);
            constraints.push(pool.is_one_of(byte, &ascii));
            text.push(byte);
            continue;
        }
        let choices = &by_width[width];
        let last = choices.len() - 1;
        let choice = pool.var_in(8, &variable, 0, last as u128);
        let numbers: Vec<u8> = (0..=last as u8).collect();
        pool.allow(choice, &numbers);
        constraints.push(pool.is_one_of(choice, &numbers));
        for (position, final_byte) in choices[last].iter().take(usize::from(*width)).enumerate() {
            let mut byte = pool.bv(8, u128::from(*final_byte));
            for (number, encoded) in choices[..last].iter().enumerate().rev() {
                let here = pool.bv(8, number as u128);
                let chosen = pool.eq(choice, here);
                let value = pool.bv(8, u128::from(encoded[position]));
                byte = pool.ite(chosen, value, byte);
            }
            text.push(byte);
        }
    }
    Input {
        text,
        number: None,
        constraints,
    }
}

/// A variable's text and value for one shape, with what its digits must satisfy. A fixed value
/// that isn't a whole number has no value formulas can use.
struct Input {
    text: Vec<Term>,
    number: Option<Number>,
    constraints: Vec<Term>,
}

/// The input for a number the program reads only as `count` random bytes, least significant
/// first: a variable for each byte, which may be any value. Eight bytes are a 64-bit two's
/// complement value; fewer are a value from 0.
fn bytes_input(pool: &mut Pool, name: &str, count: usize) -> Input {
    let bytes: Vec<Term> = (0..count)
        .map(|index| pool.var(8, &format!("{name}.byte{index}")))
        .collect();
    let mut word = bytes[0];
    for byte in &bytes[1..] {
        word = pool.concat(*byte, word);
    }
    let width = pool.width(word);
    let (term, low, high) = if count == 8 {
        (
            pool.sign_extend(word, WIDTH - width),
            i128::from(i64::MIN),
            i128::from(i64::MAX),
        )
    } else {
        (
            pool.zero_extend(word, WIDTH - width),
            0,
            (1_i128 << (8 * count)) - 1,
        )
    };
    Input {
        text: bytes,
        number: Some(Number::whole(term, low, high)),
        constraints: Vec::new(),
    }
}

/// The input for a number the program reads only as a clock reading: one variable in range, its
/// digits never written out. A decimal's term is its digits without the point.
fn plain_input(pool: &mut Pool, name: &str, low: i128, high: i128, places: u8) -> Input {
    let bound = |value: i128| u128::try_from(value).unwrap_or(0);
    let variable = pool.var_in(64, &format!("{name}.plain"), bound(low), bound(high));
    let (bottom, top) = (pool.bv(64, bound(low)), pool.bv(64, bound(high)));
    let constraints = vec![pool.ule(bottom, variable), pool.ule(variable, top)];
    let term = pool.zero_extend(variable, WIDTH - 64);
    Input {
        text: Vec::new(),
        number: Some(Number {
            term,
            low,
            high,
            places,
        }),
        constraints,
    }
}

/// The bytes a block's `random` template supplies, or `None` when the kernel draws them.
fn random_stream(
    pool: &mut Pool,
    block: &Generated,
    inputs: &BTreeMap<String, Input>,
) -> Result<Option<Vec<Term>>, String> {
    let Some(template) = &block.random else {
        return Ok(None);
    };
    let parts = template
        .hex_parts()
        .ok_or("random bytes that aren't hex digits and hex(x, n)")?;
    let bytes_only = block.random_only();
    let mut stream = Vec::new();
    for part in parts {
        match part {
            HexPart::Digits(digits) => {
                stream.extend(exec::bytes(pool, &crate::supply::hex(&digits)?));
            }
            HexPart::Var(name, count) => {
                let input = inputs
                    .get(&name)
                    .ok_or("random bytes from an unknown variable")?;
                if bytes_only.contains_key(&name) {
                    stream.extend(&input.text[..count]);
                } else {
                    let number = input.number.ok_or("random bytes from text")?;
                    for index in 0..count {
                        let low = u32::try_from(8 * index).unwrap_or(0);
                        stream.push(pool.extract(number.term, low + 7, low));
                    }
                }
            }
        }
    }
    Ok(Some(stream))
}

/// The readings a block's `clock` templates supply, seconds and nanoseconds; empty when the
/// kernel's clock answers.
fn clock_readings(
    pool: &mut Pool,
    block: &Generated,
    inputs: &BTreeMap<String, Input>,
) -> Result<Vec<(Term, Term)>, String> {
    let mut readings = Vec::new();
    for template in &block.clock {
        let reading = match template.pattern().as_deref() {
            Some([PatternPart::Text(text)]) => {
                let time = crate::supply::Time::parse(text)?;
                (
                    pool.bv(64, u128::from(time.seconds.cast_unsigned())),
                    pool.bv(64, u128::from(time.nanoseconds)),
                )
            }
            Some([PatternPart::Var(name)]) => {
                let number = inputs
                    .get(name)
                    .and_then(|input| input.number)
                    .ok_or("a clock reading that isn't a number")?;
                let places = u32::from(number.places);
                let scale = pool.bv(WIDTH, 10_u128.pow(places));
                let seconds = pool.udiv(number.term, scale);
                let rest = pool.urem(number.term, scale);
                let nano = pool.bv(WIDTH, 10_u128.pow(9 - places.min(9)));
                let nanoseconds = pool.mul(rest, nano);
                (
                    pool.extract(seconds, 63, 0),
                    pool.extract(nanoseconds, 63, 0),
                )
            }
            _ => return Err("a clock reading that mixes text and variables".into()),
        };
        readings.push(reading);
    }
    Ok(readings)
}

/// The input for text of `length` bytes from `characters`: a variable for each byte.
fn text_input(pool: &mut Pool, name: &str, length: usize, characters: &[u8]) -> Input {
    let (lowest, highest) = (
        characters.iter().copied().min().unwrap_or(0),
        characters.iter().copied().max().unwrap_or(0),
    );
    let mut text = Vec::with_capacity(length);
    let mut constraints = Vec::new();
    for index in 0..length {
        let byte = pool.var_in(
            8,
            &format!("{name}.text{length}.{index}"),
            u128::from(lowest),
            u128::from(highest),
        );
        if length <= MAX_CHECKED_TEXT {
            pool.allow(byte, characters);
            constraints.push(pool.is_one_of(byte, characters));
        }
        text.push(byte);
    }
    Input {
        text,
        number: None,
        constraints,
    }
}

/// A text of a length the input chooses, from `shortest` to `longest`, its bytes from
/// `characters`: as many bytes as it can be long, each held to the characters where it's inside,
/// and the length.
fn within_input(
    pool: &mut Pool,
    name: &str,
    (shortest, longest): (usize, usize),
    characters: &[u8],
) -> (Input, Term) {
    let length = pool.var_in(
        64,
        &format!("{name}.length"),
        shortest as u128,
        longest as u128,
    );
    // Its bytes are held to its characters where a question finds inputs, not on every
    // question: a condition for each byte, each on the length, would make every comparison of
    // the length read them all.
    let text = (0..longest)
        .map(|index| {
            // A byte past the end stands for nothing read: any byte.
            let byte = pool.var(8, &format!("{name}.within{longest}.{index}"));
            pool.allow(byte, characters);
            byte
        })
        .collect();
    (
        Input {
            text,
            number: None,
            constraints: Vec::new(),
        },
        length,
    )
}

/// That `byte`, the `index`th of a text `length` long, is one of `characters` where it's inside.
fn held_inside(pool: &mut Pool, byte: Term, index: usize, length: Term, characters: &[u8]) -> Term {
    let held = pool.is_one_of(byte, characters);
    let at = pool.bv(64, index as u128);
    let inside = pool.ult(at, length);
    let outside = pool.bool_not(inside);
    pool.bool_or(outside, held)
}

/// The input for one value of a `choice(…)`: its text, and its value when it's a whole number.
fn fixed(pool: &mut Pool, value: &Value) -> Result<Input, String> {
    let text = value
        .render()
        .map_err(|error| format!("a choice that can't be written: {error}"))?;
    Ok(Input {
        text: exec::bytes(pool, text.as_bytes()),
        number: match value {
            Value::Int(number) => Some(Number::whole(
                pool.bv_signed(WIDTH, *number),
                *number,
                *number,
            )),
            Value::Dec { m, p } => Some(Number {
                term: pool.bv_signed(WIDTH, *m),
                low: *m,
                high: *m,
                places: *p,
            }),
            _ => None,
        },
        constraints: Vec::new(),
    })
}

fn input(pool: &mut Pool, name: &str, number: usize, shape: &Shape, places: u8) -> Input {
    let (ten, forty_eight) = (pool.bv(WIDTH, 10), pool.bv(WIDTH, u128::from(b'0')));
    let mut text = Vec::new();
    let mut constraints = Vec::new();
    if shape.negative {
        text.push(pool.bv(8, u128::from(b'-')));
    }
    // A decimal shows at least one digit before its point: 0.05, not .05.
    let mut digits = shape.digits.clone();
    while digits.len() <= usize::from(places) {
        digits.insert(0, (0, 0));
    }
    let point = (places > 0).then(|| digits.len() - usize::from(places));
    let mut magnitude = pool.bv(WIDTH, 0);
    for (index, (lowest, highest)) in digits.iter().enumerate() {
        if point == Some(index) {
            text.push(pool.bv(8, u128::from(b'.')));
        }
        // A digit with one possible value is that value.
        let digit = pool.var_in(
            8,
            &format!("{name}.shape{number}.{index}"),
            u128::from(b'0' + lowest),
            u128::from(b'0' + highest),
        );
        let (lowest, highest) = (
            pool.bv(8, u128::from(b'0' + lowest)),
            pool.bv(8, u128::from(b'0' + highest)),
        );
        constraints.push(pool.ule(lowest, digit));
        constraints.push(pool.ule(digit, highest));
        text.push(digit);
        let wide = pool.zero_extend(digit, WIDTH - 8);
        let value = pool.sub(wide, forty_eight);
        let shifted = pool.mul(magnitude, ten);
        magnitude = pool.add(shifted, value);
    }
    let value = if shape.negative {
        pool.neg(magnitude)
    } else {
        magnitude
    };
    let (low, high) = (
        pool.bv_signed(WIDTH, shape.low),
        pool.bv_signed(WIDTH, shape.high),
    );
    constraints.push(pool.sle(low, value));
    constraints.push(pool.sle(value, high));
    Input {
        text,
        number: Some(Number {
            term: value,
            low: shape.low,
            high: shape.high,
            places,
        }),
        constraints,
    }
}

/// The bytes an args or stdin template gives for these inputs.
fn pattern_bytes(
    pool: &mut Pool,
    template: &Template,
    inputs: &BTreeMap<String, Input>,
) -> Result<Vec<Term>, String> {
    let pattern = template
        .pattern()
        .ok_or("an input template that computes a value")?;
    let mut bytes = Vec::new();
    for part in pattern {
        match part {
            PatternPart::Text(text) => bytes.extend(exec::bytes(pool, text.as_bytes())),
            PatternPart::Var(name) => bytes.extend(
                &inputs
                    .get(&name)
                    .ok_or_else(|| format!("the variable {name}"))?
                    .text,
            ),
        }
    }
    Ok(bytes)
}

/// When `byte` is the digit `'0' + (x / power) % 10`, its root `x` and the power.
fn digit_of(pool: &Pool, byte: Term) -> Option<(Term, u128)> {
    let Node::Add(low, offset) = *pool.node(byte) else {
        return None;
    };
    if pool.constant(offset) != Some(u128::from(b'0')) {
        return None;
    }
    let Node::Extract(remainder, 7, 0) = *pool.node(low) else {
        return None;
    };
    let Node::Urem(quotient, ten) = *pool.node(remainder) else {
        return None;
    };
    if pool.constant(ten) != Some(10) {
        return None;
    }
    match *pool.node(quotient) {
        Node::Udiv(root, power) => Some((root, pool.constant(power)?)),
        _ => Some((quotient, 1)),
    }
}

/// The value whose decimal digits `bytes` are, most significant first, when the ones that aren't
/// fixed all come from dividing one value by powers of ten.
fn common_root(pool: &Pool, bytes: &[Term]) -> Option<Term> {
    let mut root = None;
    for (index, byte) in bytes.iter().enumerate() {
        if pool.constant(*byte).is_some() {
            continue;
        }
        let (value, power) = digit_of(pool, *byte)?;
        let expected = 10_u128.checked_pow(u32::try_from(bytes.len() - 1 - index).ok()?)?;
        if power != expected || root.is_some_and(|known| known != value) {
            return None;
        }
        root = Some(value);
    }
    root
}

/// What must hold for `bytes` to be exactly the decimal digits of the non-negative `magnitude`,
/// the size of `value`, which is negative when `negative`.
fn digits_are(
    pool: &mut Pool,
    bytes: &[Term],
    magnitude: Term,
    value: Term,
    negative: bool,
    padded_to: usize,
) -> Term {
    let count = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    let (Some(top), Some(bottom)) = (
        10_u128.checked_pow(count),
        10_u128.checked_pow(count.saturating_sub(1)),
    ) else {
        return pool.boolean(false);
    };
    if bytes.len() < padded_to {
        return pool.boolean(false);
    }
    // Leading zeros only pad to the shortest length: 0.05's 005, never 007's for 7.
    let bottom = if bytes.len() <= padded_to { 0 } else { bottom };
    let (bottom, top) = (pool.bv(WIDTH, bottom), pool.bv(WIDTH, top));
    let at_least = pool.ule(bottom, magnitude);
    let below = pool.ult(magnitude, top);
    let mut checks = vec![at_least, below];
    // When every byte is already the magnitude's own digit, as when the program and the
    // formula compute the value the same way, nothing more needs saying.
    let by_magnitude = bytes_are_digits(pool, bytes, magnitude);
    if pool.truth(by_magnitude) == Some(true) {
        return pool.all(&checks);
    }
    // With a root, the printed value must be the magnitude, and each byte is checked against
    // the root's own digit: the same computation is the same term, and a fixed digit is settled
    // by the root's range. Without one, each byte is checked against the magnitude's digit.
    let base = match common_root(pool, bytes) {
        Some(root) => {
            // The program's value read as a signed number must be the formula's: then the
            // digits of root, its size, are the digits of the formula's size. Signed, the
            // program's arithmetic and the formula's come out the same term.
            // A negative value's root is its size: the program's value is the root negated.
            let width = pool.width(root);
            let signed = if negative { pool.neg(root) } else { root };
            let check = if negative && pool.signed_range(signed).0 > -(1_i128 << (width - 1)) {
                let wide = pool.sign_extend(signed, WIDTH - width);
                pool.eq(wide, value)
            } else if negative {
                let wide = pool.zero_extend(root, WIDTH - width);
                pool.eq(wide, magnitude)
            } else {
                let wide = pool.sign_extend(root, WIDTH - width);
                pool.eq(wide, value)
            };
            checks.push(check);
            root
        }
        None => {
            checks.push(by_magnitude);
            return pool.all(&checks);
        }
    };
    checks.push(bytes_are_digits(pool, bytes, base));
    pool.all(&checks)
}

/// What must hold for `bytes` to be the decimal digits of `base`, one byte each, where the
/// caller also requires `base` to be below 10 to the number of bytes: then the first digit is the
/// quotient itself, and one digit is the value. Where the program computed its first digit as a
/// remainder too, that form meets the program's and is kept.
fn bytes_are_digits(pool: &mut Pool, bytes: &[Term], base: Term) -> Term {
    let count = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    let width = pool.width(base);
    let (ten, zero_char) = (pool.bv(width, 10), pool.bv(8, u128::from(b'0')));
    let mut checks = Vec::new();
    for (index, byte) in bytes.iter().enumerate() {
        let power = 10_u128.pow(count - 1 - u32::try_from(index).unwrap_or(0));
        let power = pool.bv(width, power);
        let quotient = pool.udiv(base, power);
        let digit = pool.urem(quotient, ten);
        let is = |pool: &mut Pool, digit: Term| {
            let low = pool.extract(digit, 7, 0);
            let character = pool.add(low, zero_char);
            pool.eq(*byte, character)
        };
        let check = is(pool, digit);
        checks.push(if index == 0 && pool.truth(check).is_none() {
            is(pool, quotient)
        } else {
            check
        });
    }
    pool.all(&checks)
}

/// What must hold for `bytes` to be exactly `number` in decimal, its last `places` digits after a
/// point, as `places(number, places)` writes it.
fn number_is(pool: &mut Pool, bytes: &[Term], number: Number, places: u8) -> Term {
    let zero = pool.bv(WIDTH, 0);
    let negative = pool.slt(number.term, zero);
    let negated = pool.neg(number.term);
    let magnitude = pool.ite(negative, negated, number.term);
    // The first byte, when the code wrote a fixed one, says which case it can be.
    let first = bytes.first().and_then(|byte| pool.constant(*byte));
    let mut cases = Vec::new();
    if bytes.len() >= 2 && first.is_none_or(|byte| byte == u128::from(b'-')) {
        let minus = pool.bv(8, u128::from(b'-'));
        let signed = pool.eq(bytes[0], minus);
        let digits = pointed_digits(pool, &bytes[1..], magnitude, number.term, true, places);
        cases.push(pool.all(&[negative, signed, digits]));
    }
    if !bytes.is_empty() && first != Some(u128::from(b'-')) {
        let not_negative = pool.bool_not(negative);
        let digits = pointed_digits(pool, bytes, magnitude, number.term, false, places);
        cases.push(pool.all(&[not_negative, digits]));
    }
    pool.any(&cases)
}

/// What must hold for `bytes` to be the digits of `magnitude` with a point before the last
/// `places`, and at least one digit before it.
fn pointed_digits(
    pool: &mut Pool,
    bytes: &[Term],
    magnitude: Term,
    value: Term,
    negative: bool,
    places: u8,
) -> Term {
    let places = usize::from(places);
    if places == 0 {
        return digits_are(pool, bytes, magnitude, value, negative, 1);
    }
    if bytes.len() < places + 2 {
        return pool.boolean(false);
    }
    let point = bytes.len() - places - 1;
    let dot = pool.bv(8, u128::from(b'.'));
    let has_point = pool.eq(bytes[point], dot);
    let mut digits = bytes[..point].to_vec();
    digits.extend_from_slice(&bytes[point + 1..]);
    let digits = digits_are(pool, &digits, magnitude, value, negative, places + 1);
    pool.all(&[has_point, digits])
}

/// What a shape knows about the variables: the whole numbers' values, the fixed values chosen,
/// and the texts drawn, byte by byte.
struct Known<'a> {
    numbers: &'a BTreeMap<String, Number>,
    fixed: &'a crate::expr::Env,
    texts: &'a BTreeMap<String, Vec<Term>>,
    /// The lengths of the texts whose length the input chooses, which `texts` holds as many
    /// bytes of as they can be long.
    lengths: Option<&'a BTreeMap<String, Term>>,
    /// Formulas translated in this shape, by expression: every run of a shape asks for the
    /// same ones, and a text formula over thousands of bytes is worth translating once.
    translated: std::cell::RefCell<HashMap<*const crate::expr::Expr, Result<Formula, String>>>,
    /// In an induction's base, the count of what was read before the read loop's head, for
    /// each count placeholder.
    offsets: HashMap<*const crate::expr::Expr, Offset>,
    /// In the base of an induction whose counts look back, what each run's own offsets come
    /// from.
    relative: Option<&'a Relative>,
}

/// In an induction's base, what a count placeholder's formula goes on from: the count of what
/// came before the text, from its least to its most, and, where the count looks back, the bytes
/// just before the text, after which it counts what the text adds.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Offset {
    before: Term,
    low: i128,
    high: i128,
    window: Option<Vec<Term>>,
    /// For a count of a part that overlaps itself, the phase the head left its matcher in.
    phase: Option<formula::Phase>,
}

/// In the base of an induction whose counts look back, where each run reads a fresh
/// buffer-full from the head before its final piece: for each count, its slot, the value it
/// starts at, its placeholder's formula and how far back it looks; and the text's characters,
/// which that buffer-full is held to.
struct Relative {
    folds: Vec<(Slot, u128, *const crate::expr::Expr, usize)>,
    characters: Vec<u8>,
}

impl Known<'_> {
    /// `expr` as terms in this shape, translated once.
    fn translate(&self, pool: &mut Pool, expr: &crate::expr::Expr) -> Result<Formula, String> {
        let key = std::ptr::from_ref(expr);
        if let Some(found) = self.translated.borrow().get(&key) {
            return found.clone();
        }
        let offset = self.offsets.get(&key);
        let chosen_length = self.lengths.filter(|lengths| {
            crate::expr::variables_of(expr)
                .iter()
                .any(|name| lengths.contains_key(name))
        });
        let mut result = match offset {
            // A count of a text whose length the input chooses, each part where it's inside.
            _ if let Some(lengths) = chosen_length => formula::within(
                pool,
                expr,
                self.texts,
                lengths,
                offset.and_then(|offset| offset.phase.as_ref()),
                offset
                    .and_then(|offset| offset.window.as_deref())
                    .unwrap_or_default(),
            ),
            Some(Offset {
                phase: Some(phase), ..
            }) => formula::count_from(pool, expr, self.texts, phase),
            Some(Offset {
                window: Some(window),
                ..
            }) => formula::delta(pool, expr, self.texts, window),
            _ => formula::translate(pool, expr, self.numbers, self.texts),
        };
        // A count over the text from the head on, and what came before it.
        if let (Ok(Formula::Number(number)), Some(offset)) = (&result, offset) {
            result = Ok(Formula::Number(Number {
                term: pool.add(offset.before, number.term),
                low: number.low.saturating_add(offset.low),
                high: number.high.saturating_add(offset.high),
                ..*number
            }));
        }
        self.translated.borrow_mut().insert(key, result.clone());
        result
    }
}

/// A run of a template once what's known in this shape is filled in: bytes, or a number's
/// formula, printed with this many decimal places (`places(x, 2)`), or none, or any whole number
/// from one formula's value to another's (`any(lo, hi)`).
enum Segment<'a> {
    Bytes(Vec<Term>),
    Number(&'a crate::expr::Expr, u8),
    Any(&'a crate::expr::Expr, &'a crate::expr::Expr),
    /// `if(condition, "text", "other text")`: one of two fixed texts, as a condition says.
    Either(&'a crate::expr::Expr),
    /// One of several texts, each where its condition holds, as `sort(s)` gives.
    Texts(Vec<(Term, Vec<Term>)>),
}

/// What must hold for `bytes` to be exactly `template`'s text for these inputs. Literal text, a
/// placeholder whose variables all have fixed values, and a placeholder that's just a drawn text
/// are known bytes; the other placeholders compute a number, allow a range of them, or choose
/// between two fixed texts.
fn text_is(
    pool: &mut Pool,
    bytes: &[Term],
    template: &Template,
    known: &Known<'_>,
) -> Result<Term, String> {
    let mut segments: Vec<Segment<'_>> = Vec::new();
    for piece in template.pieces() {
        let segment = match piece {
            Piece::Text(text) => Segment::Bytes(exec::bytes(pool, text.as_bytes())),
            Piece::Expr(crate::expr::Expr::Call(crate::expr::Function::Any, arguments)) => {
                match arguments.as_slice() {
                    [low, high] => Segment::Any(low, high),
                    _ => return Err("any(…) without its two bounds".into()),
                }
            }
            Piece::Expr(crate::expr::Expr::Var(name)) if known.texts.contains_key(name) => {
                Segment::Bytes(known.texts[name].clone())
            }
            Piece::Expr(expr)
                if crate::expr::variables_of(expr)
                    .iter()
                    .all(|name| known.fixed.contains_key(name)) =>
            {
                let value = crate::expr::eval(expr, known.fixed)
                    .map_err(|error| format!("a placeholder that can't be computed: {error}"))?;
                let text = value
                    .render()
                    .map_err(|error| format!("a placeholder that can't be written: {error}"))?;
                Segment::Bytes(exec::bytes(pool, text.as_bytes()))
            }
            Piece::Expr(expr @ crate::expr::Expr::Call(crate::expr::Function::If, arguments))
                if arguments.len() == 3 && chooses_text(expr, known) =>
            {
                Segment::Either(expr)
            }
            Piece::Expr(crate::expr::Expr::Call(crate::expr::Function::Places, arguments)) => {
                match arguments.as_slice() {
                    [value, crate::expr::Expr::Literal(Value::Int(places))] => Segment::Number(
                        value,
                        u8::try_from(*places)
                            .ok()
                            .filter(|places| (1..=30).contains(places))
                            .ok_or("places with an unusual number of places")?,
                    ),
                    _ => return Err("places with its places given by a formula".into()),
                }
            }
            // A placeholder whose value is text, such as upper(text), is its bytes; sort(text),
            // one of its texts.
            Piece::Expr(expr) => match known.translate(pool, expr) {
                Ok(Formula::Text(bytes)) => Segment::Bytes(bytes),
                Ok(Formula::Texts(texts)) => Segment::Texts(texts),
                Err(why) if is_lined(expr) => return Err(why),
                _ => Segment::Number(expr, 0),
            },
        };
        // Neighboring bytes join, so a template is at most bytes, a number, bytes.
        match (segments.last_mut(), segment) {
            (Some(Segment::Bytes(before)), Segment::Bytes(more)) => before.extend(more),
            (_, segment) => segments.push(segment),
        }
    }
    // Neighboring bytes joined, so the template is literal text and computed values in turn:
    // literals[i] comes before computed[i], and the last literal after the last value.
    let mut literals: Vec<Vec<Term>> = vec![Vec::new()];
    let mut computed = Vec::new();
    for segment in segments {
        match segment {
            Segment::Bytes(text) => *literals.last_mut().unwrap_or(&mut Vec::new()) = text,
            Segment::Number(expr, places) => {
                computed.push(printed(pool, expr, places, known)?);
                literals.push(Vec::new());
            }
            Segment::Any(low, high) => {
                computed.push(Computed::Range(
                    bound(pool, low, known)?,
                    bound(pool, high, known)?,
                ));
                literals.push(Vec::new());
            }
            Segment::Either(expr) => {
                let always = pool.boolean(true);
                computed.push(Computed::Either(alternatives(pool, expr, known, always)?));
                literals.push(Vec::new());
            }
            Segment::Texts(texts) => {
                let texts = texts
                    .into_iter()
                    .map(|(condition, text)| (condition, Outcome::Text(text)))
                    .collect();
                computed.push(Computed::Either(texts));
                literals.push(Vec::new());
            }
        }
    }
    if computed.is_empty() {
        return Ok(same_terms(pool, bytes, &literals[0]));
    }
    // A range has no fixed text after it to show where its number ends.
    let ranged = |index: usize| matches!(computed.get(index), Some(Computed::Range(..)));
    if (1..computed.len())
        .any(|index| literals[index].is_empty() && (ranged(index - 1) || ranged(index)))
    {
        return Err("a range of numbers right next to another computed value".into());
    }
    // The values' lengths aren't known ahead, so try every way to split the rest of the output
    // among them: each split is exact, and the output is right when one of them holds.
    let fixed: usize = literals.iter().map(Vec::len).sum();
    let Some(rest) = bytes.len().checked_sub(fixed) else {
        return Ok(pool.boolean(false));
    };
    let lengths: Vec<Vec<usize>> = computed.iter().map(Computed::lengths).collect();
    let splits = splits(rest, &lengths)?;
    let mut cases = Vec::with_capacity(splits.len());
    for lengths in splits {
        let mut checks = Vec::new();
        let mut at = 0;
        for (index, length) in lengths.iter().enumerate() {
            let literal = &literals[index];
            checks.push(same_terms(pool, &bytes[at..at + literal.len()], literal));
            at += literal.len();
            let piece = &bytes[at..at + length];
            checks.push(match &computed[index] {
                Computed::Exact(number, places, _) => number_is(pool, piece, *number, *places),
                Computed::Range(low, high) => number_in(pool, piece, *low, *high),
                Computed::Either(alternatives) => {
                    let mut chosen = Vec::new();
                    for (condition, outcome) in alternatives {
                        let same = match outcome {
                            Outcome::Text(text) if text.len() == piece.len() => {
                                same_terms(pool, piece, text)
                            }
                            Outcome::Printed(number, places, lengths)
                                if lengths.contains(&piece.len()) =>
                            {
                                number_is(pool, piece, *number, *places)
                            }
                            _ => continue,
                        };
                        chosen.push(pool.bool_and(*condition, same));
                    }
                    pool.any(&chosen)
                }
            });
            at += length;
        }
        let last = &literals[computed.len()];
        checks.push(same_terms(pool, &bytes[at..], last));
        cases.push(pool.all(&checks));
    }
    Ok(pool.any(&cases))
}

/// A computed part of a test's output: a formula's value printed with this many decimal places,
/// any whole number in a range, or one of several outcomes, each where its condition holds.
enum Computed {
    /// A value printed with this many places, which can take these many bytes.
    Exact(Number, u8, Vec<usize>),
    Range(Number, Number),
    Either(Vec<(Term, Outcome)>),
}

/// One outcome an `if(…)` chain in a test's output can give: text, or a value printed.
enum Outcome {
    Text(Vec<Term>),
    /// A value printed with this many places, which can take these many bytes.
    Printed(Number, u8, Vec<usize>),
}

impl Computed {
    /// How many bytes it can take.
    fn lengths(&self) -> Vec<usize> {
        match self {
            Self::Exact(_, _, lengths) => lengths.clone(),
            Self::Range(..) => (1..=MAX_PRINTED).collect(),
            Self::Either(alternatives) => {
                let mut lengths: Vec<usize> = alternatives
                    .iter()
                    .flat_map(|(_, outcome)| match outcome {
                        Outcome::Text(text) => vec![text.len()],
                        Outcome::Printed(_, _, lengths) => lengths.clone(),
                    })
                    .collect();
                lengths.sort_unstable();
                lengths.dedup();
                lengths
            }
        }
    }
}

/// Whether `expr` is `sort`, `uniq` or `tally`, whose reason a proof gives when it can't follow
/// them.
fn is_lined(expr: &crate::expr::Expr) -> bool {
    matches!(
        expr,
        crate::expr::Expr::Call(
            crate::expr::Function::Sort
                | crate::expr::Function::Uniq
                | crate::expr::Function::Tally,
            _
        )
    )
}

/// Whether `expr` is an `if(…)` chain with a fixed text among its outcomes, such as padding
/// that depends on a number's size, or a value printed only where the input is valid.
fn chooses_text(expr: &crate::expr::Expr, known: &Known<'_>) -> bool {
    match expr {
        crate::expr::Expr::Call(crate::expr::Function::If, arguments) if arguments.len() == 3 => {
            chooses_text(&arguments[1], known) || chooses_text(&arguments[2], known)
        }
        leaf => fixed_text(leaf, known).is_some(),
    }
}

/// The fixed texts an `if(…)` chain can give, each with the condition, under `guard`, that it's
/// the one given.
fn alternatives(
    pool: &mut Pool,
    expr: &crate::expr::Expr,
    known: &Known<'_>,
    guard: Term,
) -> Result<Vec<(Term, Outcome)>, String> {
    match expr {
        crate::expr::Expr::Call(crate::expr::Function::If, arguments) if arguments.len() == 3 => {
            let condition = match known.translate(pool, &arguments[0])? {
                Formula::Truth(condition) => condition,
                Formula::Number(_) | Formula::Text(_) | Formula::Float(_) | Formula::Texts(_) => {
                    return Err("an if(…) whose condition isn't true or false".into());
                }
            };
            let yes = pool.bool_and(guard, condition);
            let not = pool.bool_not(condition);
            let no = pool.bool_and(guard, not);
            let mut found = alternatives(pool, &arguments[1], known, yes)?;
            found.extend(alternatives(pool, &arguments[2], known, no)?);
            Ok(found)
        }
        leaf => {
            if let Some(text) = fixed_text(leaf, known) {
                return Ok(vec![(
                    guard,
                    Outcome::Text(exec::bytes(pool, text.as_bytes())),
                )]);
            }
            let (value, places) = match leaf {
                crate::expr::Expr::Call(crate::expr::Function::Places, arguments) => {
                    match arguments.as_slice() {
                        [value, crate::expr::Expr::Literal(Value::Int(places))] => (
                            value,
                            u8::try_from(*places)
                                .ok()
                                .filter(|places| (1..=30).contains(places))
                                .ok_or("places with an unusual number of places")?,
                        ),
                        _ => return Err("places with its places given by a formula".into()),
                    }
                }
                _ => (leaf, 0),
            };
            if places == 0
                && let Formula::Text(bytes) = known.translate(pool, value)?
            {
                return Ok(vec![(guard, Outcome::Text(bytes))]);
            }
            match printed(pool, value, places, known)? {
                Computed::Exact(number, places, lengths) => {
                    Ok(vec![(guard, Outcome::Printed(number, places, lengths))])
                }
                _ => Err("an if(…) outcome that isn't text or a number".into()),
            }
        }
    }
}

/// A placeholder's text when its variables all have fixed values and it's text.
fn fixed_text(expr: &crate::expr::Expr, known: &Known<'_>) -> Option<String> {
    let fixed = crate::expr::variables_of(expr)
        .iter()
        .all(|name| known.fixed.contains_key(name));
    match crate::expr::eval(expr, known.fixed) {
        Ok(Value::Str(text)) if fixed && !crate::expect::has_holes(&text) => Some(text),
        _ => None,
    }
}

/// The value a placeholder prints, with the places it prints.
fn printed(
    pool: &mut Pool,
    expr: &crate::expr::Expr,
    places: u8,
    known: &Known<'_>,
) -> Result<Computed, String> {
    let number = match known.translate(pool, expr)? {
        Formula::Number(number) => number,
        Formula::Truth(_) => return Err("a placeholder that prints true or false".into()),
        Formula::Text(_) | Formula::Texts(_) => {
            return Err("a text placeholder where a number goes".into());
        }
        // The test language writes a float its shortest exact way, which proofs don't follow.
        Formula::Float(_) => {
            return Err(
                "a placeholder that prints a float as it is, not with fixed(x, places)".into(),
            );
        }
    };
    // places(x, k) prints a whole number with k places; a decimal itself prints with its own.
    let places = match (places, number.places) {
        (0, own) => own,
        (_, 0) => places,
        _ => return Err("places of a decimal; scale it to a whole number first".into()),
    };
    // A whole number takes as many bytes as its term's range allows its digits and sign: the
    // range the terms settle, so a run's own branch on a digit count still meets its length.
    let lengths = if places == 0 {
        let (low, high) = pool.signed_range(number.term);
        let digits = |value: i128| value.unsigned_abs().to_string().len();
        let shortest = if low <= 0 && high >= 0 {
            1
        } else if low > 0 {
            digits(low)
        } else {
            digits(high) + 1
        };
        let longest = digits(high).max(if low < 0 { digits(low) + 1 } else { 1 });
        (shortest..=longest.min(MAX_PRINTED)).collect()
    } else {
        (1..=MAX_PRINTED).collect()
    };
    Ok(Computed::Exact(number, places, lengths))
}

/// The most characters one printed value may take: 39 digits, a sign and a point.
const MAX_PRINTED: usize = 41;

/// The most ways to split an output among its values that a proof tries.
const MAX_SPLITS: usize = 4096;

/// Every way to give the computed values `total` bytes in all, each one a length it can take.
fn splits(total: usize, lengths: &[Vec<usize>]) -> Result<Vec<Vec<usize>>, String> {
    fn fill(
        total: usize,
        lengths: &[Vec<usize>],
        prefix: &mut Vec<usize>,
        out: &mut Vec<Vec<usize>>,
    ) {
        if out.len() > MAX_SPLITS {
            return;
        }
        let Some((first, rest)) = lengths.split_first() else {
            if total == 0 {
                out.push(prefix.clone());
            }
            return;
        };
        let most: usize = rest
            .iter()
            .map(|lengths| lengths.iter().max().unwrap_or(&0))
            .sum();
        for length in first.iter().copied().filter(|length| *length <= total) {
            if total - length > most {
                continue;
            }
            prefix.push(length);
            fill(total - length, rest, prefix, out);
            prefix.pop();
        }
    }
    let mut out = Vec::new();
    fill(total, lengths, &mut Vec::new(), &mut out);
    if out.len() > MAX_SPLITS {
        return Err(format!(
            "an output whose {} computed values could split it more than {MAX_SPLITS} ways",
            lengths.len()
        ));
    }
    Ok(out)
}

/// A bound of `any(lo, hi)` as a whole number: a formula, or a fixed value in this shape.
fn bound(pool: &mut Pool, expr: &crate::expr::Expr, known: &Known<'_>) -> Result<Number, String> {
    let fixed = crate::expr::variables_of(expr)
        .iter()
        .all(|name| known.fixed.contains_key(name));
    if fixed {
        return match crate::expr::eval(expr, known.fixed) {
            Ok(Value::Int(value)) => Ok(Number::whole(pool.bv_signed(WIDTH, value), value, value)),
            _ => Err("an any(…) bound that isn't a whole number".into()),
        };
    }
    match known.translate(pool, expr)? {
        Formula::Number(number) if number.places == 0 => Ok(number),
        _ => Err("an any(…) bound that isn't a whole number".into()),
    }
}

/// What must hold for `bytes` to be a whole number written plainly (an optional `-`, then digits
/// with no leading zero, and never `-0`) from `low` to `high`, as `any(low, high)` allows. The
/// value is read back from the bytes digit by digit, so the question has no quantifier.
fn number_in(pool: &mut Pool, bytes: &[Term], low: Number, high: Number) -> Term {
    let (zero_char, nine_char) = (pool.bv(8, u128::from(b'0')), pool.bv(8, u128::from(b'9')));
    let minus = pool.bv(8, u128::from(b'-'));
    let mut cases = Vec::new();
    for negative in [false, true] {
        let digits = &bytes[usize::from(negative)..];
        // Up to 38 digits, as the runner reads them: any more could leave 128 bits.
        if digits.is_empty() || digits.len() > 38 {
            continue;
        }
        let mut checks = Vec::new();
        if negative {
            checks.push(pool.eq(bytes[0], minus));
        }
        let (mut value, ten) = (pool.bv(WIDTH, 0), pool.bv(WIDTH, 10));
        for (index, byte) in digits.iter().enumerate() {
            let at_least = pool.ule(zero_char, *byte);
            let at_most = pool.ule(*byte, nine_char);
            checks.extend([at_least, at_most]);
            // No leading zero, and no -0.
            if index == 0 && (digits.len() > 1 || negative) {
                let zero = pool.eq(*byte, zero_char);
                checks.push(pool.bool_not(zero));
            }
            let digit = pool.sub(*byte, zero_char);
            let digit = pool.zero_extend(digit, WIDTH - 8);
            let shifted = pool.mul(value, ten);
            value = pool.add(shifted, digit);
        }
        if negative {
            value = pool.neg(value);
        }
        let above = pool.sle(low.term, value);
        let below = pool.sle(value, high.term);
        checks.extend([above, below]);
        cases.push(pool.all(&checks));
    }
    let exact = pool.any(&cases);
    // When the code printed the digits of one computed value, saying the bytes are that value
    // in range settles most runs without reading the digits back. It's only a shortcut: it
    // implies `exact`.
    let Some(root) = common_root(pool, bytes) else {
        return exact;
    };
    let width = pool.width(root);
    let value = pool.sign_extend(root, WIDTH - width);
    let printed = number_is(pool, bytes, Number::whole(value, i128::MIN, i128::MAX), 0);
    let above = pool.sle(low.term, value);
    let below = pool.sle(value, high.term);
    let shortcut = pool.all(&[printed, above, below]);
    pool.any(&[shortcut, exact])
}

/// What must hold for a run that exited with `status` to pass the block.
fn passes(
    pool: &mut Pool,
    block: &Generated,
    known: &Known<'_>,
    run: &World,
    status: Term,
) -> Result<Term, String> {
    let fixed = known.fixed;
    let stdout = text_is(pool, &run.stdout, &block.stdout, known)?;
    let stderr = text_is(pool, &run.stderr, &block.stderr, known)?;
    let exit = match &block.exit_code {
        ExitCode::Fixed(code) => {
            let code = pool.bv(8, u128::from(*code));
            pool.eq(status, code)
        }
        ExitCode::Formula(template) => {
            let [Piece::Expr(expr)] = template.pieces()[..] else {
                return Err("an exit code template with text around its formula".into());
            };
            // With every variable fixed, the formula is just a number.
            let all_fixed = crate::expr::variables_of(expr)
                .iter()
                .all(|name| fixed.contains_key(name));
            let number = if all_fixed {
                match crate::expr::eval(expr, fixed) {
                    Ok(Value::Int(code)) => Number::whole(pool.bv_signed(WIDTH, code), code, code),
                    _ => return Err("an exit code that isn't a whole number".into()),
                }
            } else {
                match known.translate(pool, expr)? {
                    Formula::Number(number) => number,
                    Formula::Truth(_)
                    | Formula::Text(_)
                    | Formula::Float(_)
                    | Formula::Texts(_) => {
                        return Err("an exit code that isn't a whole number".into());
                    }
                }
            };
            let (zero, top) = (pool.bv(WIDTH, 0), pool.bv(WIDTH, 255));
            let at_least = pool.sle(zero, number.term);
            let at_most = pool.sle(number.term, top);
            let wide = pool.zero_extend(status, WIDTH - 8);
            let same = pool.eq(wide, number.term);
            pool.all(&[at_least, at_most, same])
        }
    };
    let mut checks = vec![stdout, stderr, exit];
    // As the runner checks files: each expected one holds exactly its text, the others given
    // stay as they were, and nothing else is written.
    for (name, template) in &block.files_after {
        checks.push(match run.file(name) {
            Some(bytes) => text_is(pool, bytes, template, known)?,
            None => pool.boolean(false),
        });
    }
    for (name, before) in run.files_before() {
        if block.files_after.contains_key(name) || block.gone.contains(name) {
            continue;
        }
        checks.push(match run.file(name) {
            Some(after) => same_terms(pool, before, after),
            None => pool.boolean(false),
        });
    }
    // Folders: what's gone is gone, what's expected is there, what was given stays, and
    // nothing else is made.
    let inside = |folder: &str, name: &String| name.starts_with(&format!("{folder}/"));
    let folders_hold = block
        .gone
        .iter()
        .all(|name| run.file(name).is_none() && !run.is_folder(name))
        && block.folders_after.iter().all(|name| run.is_folder(name))
        && block
            .folders
            .iter()
            .all(|name| block.gone.contains(name) || run.is_folder(name))
        && run.folders_made().into_iter().all(|made| {
            block.folders_after.contains(made)
                || block
                    .files_after
                    .keys()
                    .chain(&block.folders_after)
                    .any(|name| inside(made, name))
        });
    if !folders_hold
        || run
            .written
            .keys()
            .any(|name| !block.files.contains_key(name) && !block.files_after.contains_key(name))
    {
        checks.push(pool.boolean(false));
    }
    Ok(pool.all(&checks))
}

/// Whether two runs of bytes are the same.
fn same_terms(pool: &mut Pool, a: &[Term], b: &[Term]) -> Term {
    if a.len() != b.len() {
        return pool.boolean(false);
    }
    let checks: Vec<Term> = a.iter().zip(b).map(|(x, y)| pool.eq(*x, *y)).collect();
    pool.all(&checks)
}

/// A program a proof runs: native machine code ([`Image`]) or a WebAssembly module
/// ([`wasm::Module`]), each with the core its runs take their steps on. The driver lays out the
/// inputs, explores the runs and checks each against the block the same way for both.
trait Program: Sync + Sized {
    /// The core its runs take their steps on.
    type Core: exec::Core<Image = Self>;

    /// The state its runs start in: `args`, the environment's `NAME=value` strings and `stdin`
    /// as byte terms.
    fn begin(
        &self,
        pool: &mut Pool,
        args: &[Vec<Term>],
        env: &[Vec<Term>],
        stdin: Vec<Term>,
    ) -> State<Self::Core>;

    /// Whether it reads its input through the canonical fill, which a proof reads as a whole for
    /// every way its input can be cut.
    fn fills(&self) -> bool;

    /// The functions no run of the proof called, of which the proof says nothing.
    fn uncalled(&self) -> Vec<String>;

    /// `state` taking the read loop's head as `hook` says, in an induction's base.
    ///
    /// # Errors
    ///
    /// Where the core has no read loop's head to take.
    fn hooked(
        state: State<Self::Core>,
        hook: std::rc::Rc<exec::induction::Hook>,
    ) -> Result<State<Self::Core>, String>;
}

impl Program for Image {
    type Core = exec::Native;

    fn begin(
        &self,
        pool: &mut Pool,
        args: &[Vec<Term>],
        env: &[Vec<Term>],
        stdin: Vec<Term>,
    ) -> State {
        State::start(self, pool, args, env, stdin)
    }

    fn fills(&self) -> bool {
        self.has_fill()
    }

    fn uncalled(&self) -> Vec<String> {
        self.unreached()
    }

    fn hooked(state: State, hook: std::rc::Rc<exec::induction::Hook>) -> Result<State, String> {
        Ok(state.with_hook(hook))
    }
}

impl Program for wasm::Module {
    type Core = wasm::Wasm;

    fn begin(
        &self,
        pool: &mut Pool,
        args: &[Vec<Term>],
        env: &[Vec<Term>],
        stdin: Vec<Term>,
    ) -> State<wasm::Wasm> {
        wasm::begin(self, pool, args, env, stdin)
    }

    /// lengua's library's `$fill` runs as the code it is, each of its reads one the world gives.
    fn fills(&self) -> bool {
        false
    }

    fn uncalled(&self) -> Vec<String> {
        self.unreached()
    }

    fn hooked(
        _state: State<wasm::Wasm>,
        _hook: std::rc::Rc<exec::induction::Hook>,
    ) -> Result<State<wasm::Wasm>, String> {
        Err("the WebAssembly model takes no read loop's head yet: every length by induction is \
             the prover's third step"
            .into())
    }
}

/// Why a block that uses `now` isn't proven.
const USES_NOW: &str = "it uses now, the second a run starts, which a proof can't fix";

/// Proves `block` about the program `machine` holds, within `budget`.
#[must_use]
pub fn prove(machine: &Machine, block: &Generated, budget: &Budget) -> Verdict {
    attempt(machine, block, budget).unwrap_or_else(Verdict::NotProven)
}

/// Proves `block` about the WebAssembly program `built`, whose idea lets it reach what `policy`
/// allows, within `budget`, as [`prove`] proves a native program: the same shapes of the inputs,
/// the same paths, solvers and checks of every run, on the WebAssembly model's runs, and the
/// same verdicts. A trap the engine would stop at is a counterexample that names main.wat's
/// line. Texts are proven up to a length, a [`Bound`]; every length is the prover's next step.
#[must_use]
pub fn prove_wasm(
    built: &crate::wasm::Built,
    policy: crate::wasm::Policy,
    block: &Generated,
    budget: &Budget,
) -> Verdict {
    attempt_wasm(built, policy, block, budget).unwrap_or_else(Verdict::NotProven)
}

fn attempt_wasm(
    built: &crate::wasm::Built,
    policy: crate::wasm::Policy,
    block: &Generated,
    budget: &Budget,
) -> Result<Verdict, String> {
    if block.uses_now() {
        return Err(USES_NOW.into());
    }
    let module = wasm::Module::new(built, policy)?;
    by_length(&module, block, budget, None)
}

fn attempt(machine: &Machine, block: &Generated, budget: &Budget) -> Result<Verdict, String> {
    if block.uses_now() {
        return Err(USES_NOW.into());
    }
    let image = Image::new(machine).ok_or("the program has no _start")?;
    // A streaming filter is proven for every length at once where its read loop's step holds;
    // otherwise, and for every other program, the proof goes length by length.
    if budget.from_length == 0 {
        match by_induction(&image, block, budget) {
            Ok(Some(verdict)) => return Ok(verdict),
            Ok(None) => {}
            Err(why) => {
                if std::env::var_os("LENGUA_PROVE_TRACE").is_some() {
                    eprintln!("no proof by induction: {why}");
                }
            }
        }
    }
    by_length(&image, block, budget, None)
}

/// Proves `block` for every length by induction over its read loop: first the states the loop
/// can be in at its head, then every input up to one buffer-full from such a state. Where its
/// counts look back, the shortest inputs are proven from the program's start and every final
/// piece after a buffer-full read from such a state. `None` where the step holds but the base
/// doesn't: the proof by length says what's wrong.
///
/// # Errors
///
/// Why no induction applies, or its step doesn't hold.
fn by_induction(
    image: &Image,
    block: &Generated,
    budget: &Budget,
) -> Result<Option<Verdict>, String> {
    let found = match induction::step(image, block, budget) {
        Ok(found) => found,
        Err(refused) => {
            // A step that went wrong read buffer-fulls which, from the program's start, may
            // show it wrong outright. Where its own bytes don't, since what went wrong there
            // took what the head left too, texts of the same length drawn at random may: a word
            // across two buffer-fulls, say, which more than half of them have.
            if let Some(input) = &refused.input {
                if let Some(verdict) = replay(image, block, input)? {
                    return Ok(Some(verdict));
                }
                if let [(_, Generator::Str { charset, .. })] = block.vars.as_slice()
                    && let characters = charset
                        .iter()
                        .filter_map(|character| u8::try_from(*character).ok())
                        .filter(u8::is_ascii)
                        .collect::<Vec<u8>>()
                    && !characters.is_empty()
                {
                    let mut rng = ChaCha8Rng::seed_from_u64(input.len() as u64);
                    for _ in 0..REPLAYED_TEXTS {
                        let text: Vec<u8> = (0..input.len())
                            .map(|_| characters[rng.random_range(0..characters.len())])
                            .collect();
                        if let Some(verdict) = replay(image, block, &text)? {
                            return Ok(Some(verdict));
                        }
                    }
                }
            }
            return Err(refused.why);
        }
    };
    let [(name, Generator::Str { min, max, charset })] = block.vars.as_slice() else {
        return Ok(None);
    };
    let texts = |min: usize, max: usize| {
        let mut short = block.clone();
        short.vars = vec![(
            name.clone(),
            Generator::Str {
                min,
                max,
                charset: charset.clone(),
            },
        )];
        short
    };
    // Every input up to a buffer-full from any state at the head: every length the loop's
    // bound allows.
    let reached = || {
        usize::try_from(found.reach)
            .ok()
            .filter(|reach| reach < max)
            .map(|reach| Bound {
                bytes: reach,
                of: *max,
                more: true,
                why: format!(
                    "the induction covers {} buffer-fulls of {} bytes, one fewer than its read \
                     loop's bound allows",
                    found.reach / found.chunk as u128,
                    found.chunk
                ),
                characters: false,
            })
    };
    // One shape for every final length, where one length at a time would take a buffer-full of
    // shapes: it may take as many paths as a few of them do, as two numbers printed after a
    // final piece that fills the buffer or doesn't.
    let within = Budget {
        within: true,
        max_paths: budget.max_paths.saturating_mul(AT_ONCE_PATHS),
        ..budget.clone()
    };
    // Only counts of the text are written for a text of any length: a mapped text's bytes
    // would each be a choice by where it ends.
    let counts = block.stdout.pieces().iter().all(|piece| match piece {
        Piece::Text(_) => true,
        Piece::Expr(expr) => {
            formula::lookback(expr, name).is_some() || formula::bordered_part(expr, name).is_some()
        }
    });
    // Every text `shape` gives at once, its length the input's choice; where that doesn't prove
    // it, one length at a time.
    let at_once = |shape: &Generated, hook: Option<&exec::induction::Hook>| {
        let verdict = if counts {
            by_length(image, shape, &within, hook)
        } else {
            Err("its output isn't counts of the text".into())
        };
        match verdict {
            Ok(verdict @ Verdict::Proven { bound: None, .. }) => Ok(verdict),
            other => {
                if std::env::var_os("LENGUA_PROVE_TRACE").is_some() {
                    eprintln!("every length at once isn't proven: {other:?}");
                }
                by_length(image, shape, budget, hook)
            }
        }
    };
    if found.lookback == 0 {
        // The base: whatever a time round leaves, from one byte up to a buffer-full, and the
        // shortest texts at the start: every such final piece at once where it can.
        let verdict = at_once(&texts((*min).min(1), found.chunk), Some(&found.hook))?;
        let Verdict::Proven {
            shapes,
            paths,
            questions,
            alone,
            bound,
            cut_any_way,
            unreached,
            ..
        } = verdict
        else {
            return Ok(None);
        };
        let (bound, induction) = match bound {
            None => (reached(), Some(found.chunk)),
            // Time ran out before the base was done: what it proved holds as a proof by length.
            Some(bound) if bound.more => (Some(Bound { of: *max, ..bound }), None),
            // Something the base couldn't prove from any state at the head, which the proof by
            // length may prove from the state the loop is really in.
            Some(_) => return Ok(None),
        };
        return Ok(Some(Verdict::Proven {
            shapes: shapes + 1,
            paths: paths + found.paths,
            questions: questions + found.questions,
            alone,
            bound,
            cut_any_way,
            unreached,
            induction,
            lookback: 0,
        }));
    }
    // The inputs of up to a buffer-full, from the program's start: a counterexample there is
    // the program's own.
    let first = if *min <= found.chunk {
        let verdict = at_once(&texts(*min, found.chunk), None)?;
        match verdict {
            Verdict::Proven { bound: None, .. } => Some(verdict),
            Verdict::Proven {
                shapes,
                paths,
                questions,
                alone,
                bound: Some(bound),
                cut_any_way,
                unreached,
                ..
            } if bound.more => {
                return Ok(Some(Verdict::Proven {
                    shapes,
                    paths,
                    questions,
                    alone,
                    bound: Some(Bound { of: *max, ..bound }),
                    cut_any_way,
                    unreached,
                    induction: None,
                    lookback: 0,
                }));
            }
            Verdict::Counterexample(_) => return Ok(Some(verdict)),
            _ => return Ok(None),
        }
    } else {
        None
    };
    // Every final piece after a buffer-full read from any state at the head.
    let verdict = at_once(&texts(1, found.chunk), Some(&found.hook))?;
    let Verdict::Proven {
        shapes,
        paths,
        questions,
        alone,
        bound,
        cut_any_way,
        unreached,
        ..
    } = verdict
    else {
        return Ok(None);
    };
    let (bound, induction) = match bound {
        None => (reached(), Some(found.chunk)),
        // Every input of up to a buffer-full, and each longer one whose final piece is as far as
        // the base got.
        Some(bound) if bound.more => (
            Some(Bound {
                bytes: found.chunk + bound.bytes,
                of: *max,
                ..bound
            }),
            None,
        ),
        Some(_) => return Ok(None),
    };
    let (shapes, paths, questions, alone, cut_any_way, unreached) = match first {
        Some(Verdict::Proven {
            shapes: before,
            paths: followed,
            questions: asked,
            alone: lone,
            cut_any_way: cut,
            unreached: missed,
            ..
        }) => (
            shapes + before,
            paths + followed,
            questions + asked,
            alone.or(lone),
            cut_any_way && cut,
            missed
                .into_iter()
                .filter(|function| unreached.contains(function))
                .collect(),
        ),
        _ => (shapes, paths, questions, alone, cut_any_way, unreached),
    };
    Ok(Some(Verdict::Proven {
        shapes: shapes + 1,
        paths: paths + found.paths,
        questions: questions + found.questions,
        alone,
        bound,
        cut_any_way,
        unreached,
        lookback: induction.map_or(0, |_| found.lookback),
        induction,
    }))
}

/// Runs the prover's model of the code from its start on `input`, a text a step that went
/// wrong read, and gives it as a counterexample where the program's output or exit code isn't
/// the test language's for it, or a memory rule breaks.
///
/// # Errors
///
/// What the model can't run.
fn replay(image: &Image, block: &Generated, input: &[u8]) -> Result<Option<Verdict>, String> {
    let [(name, Generator::Str { max, charset, .. })] = block.vars.as_slice() else {
        return Ok(None);
    };
    if input.len() > *max
        || !input
            .iter()
            .all(|byte| charset.contains(&char::from(*byte)))
    {
        return Ok(None);
    }
    let Ok(text) = String::from_utf8(input.to_vec()) else {
        return Ok(None);
    };
    let now = crate::expect::now();
    let inputs: crate::expr::Env = [(name.clone(), Value::Str(text))].into_iter().collect();
    let Ok(case) = block.case(inputs, now) else {
        return Ok(None);
    };
    let mut pool = Pool::default();
    let stdin = exec::bytes(&mut pool, case.stdin.as_bytes());
    let start =
        State::start(image, &mut pool, &[], &[], stdin).with_stdout_closed(case.stdout_closed);
    let mut runs = exec::explore(image, start, &mut pool, &mut exec::Concrete, 2)?;
    let Some(run) = runs.pop().map(|run| run.world) else {
        return Ok(None);
    };
    let bytes = |terms: &[Term]| -> Option<Vec<u8>> {
        terms
            .iter()
            .map(|term| {
                pool.constant(*term)
                    .and_then(|value| u8::try_from(value).ok())
            })
            .collect()
    };
    let wrong = match &run.end {
        Some(End::Exit(status)) => {
            pool.constant(*status) != Some(u128::from(case.exit_code))
                || bytes(&run.stdout)
                    .is_none_or(|out| !crate::expect::matches(&case.stdout, &out, now))
                || bytes(&run.stderr)
                    .is_none_or(|err| !crate::expect::matches(&case.stderr, &err, now))
        }
        Some(End::Unsafe(problem)) => {
            let mut values = case.inputs.clone();
            values.insert(UNSAFE.to_owned(), Value::Str(problem.clone()));
            return Ok(Some(Verdict::Counterexample(values)));
        }
        _ => false,
    };
    Ok(wrong.then_some(Verdict::Counterexample(case.inputs)))
}

/// Proves `block` length by length, as far as the budget goes: each input combination its own
/// shape. With `hook`, each run takes the read loop's head as an induction's base does.
fn by_length<P: Program>(
    image: &P,
    block: &Generated,
    budget: &Budget,
    hook: Option<&exec::induction::Hook>,
) -> Result<Verdict, String> {
    let mut each = Vec::new();
    let (random, clock) = (block.random_only(), block.clock_only());
    for (name, generator) in &block.vars {
        let these = match (random.get(name), generator) {
            (Some(counts), _) => vec![Part::Bytes(counts.iter().copied().max().unwrap_or(1))],
            (None, Generator::Int { lo, hi }) if clock.contains(name) => {
                vec![Part::Plain(*lo, *hi, 0)]
            }
            (None, Generator::Dec { lo, hi, places }) if clock.contains(name) => {
                vec![Part::Plain(*lo, *hi, *places)]
            }
            (None, Generator::Str { min, max, charset })
                if budget.within && charset.iter().all(char::is_ascii) =>
            {
                let bytes = charset.iter().map(|character| *character as u8).collect();
                vec![Part::Within(*min, *max, bytes)]
            }
            _ => parts(name, generator, budget.from_length)?,
        };
        each.push((name.clone(), these));
    }
    // What a test's peer sends arrives whole or cut, each way the tests' runs and the merges of
    // their pieces cut it; through fill, every way at once.
    if (!block.server.is_empty() || !block.client.is_empty()) && !image.fills() {
        let deliveries = Delivery::ALL
            .iter()
            .map(|delivery| Part::Delivery(*delivery));
        each.push((DELIVERY.to_owned(), deliveries.collect()));
    }
    let total = each
        .iter()
        .try_fold(1_usize, |total, (_, shapes)| {
            total.checked_mul(shapes.len())
        })
        .ok_or("too many input shapes to count")?;
    budget.progress.total.store(total, Ordering::Relaxed);
    // Texts of many lengths go shortest first, so where time or the model runs out the proof
    // still holds for every input up to some length; everything else goes in a scattered order,
    // so the pace of the first combinations shows the pace of all, and a counterexample anywhere
    // turns up early.
    let by_length = each.iter().any(|(_, parts)| {
        parts
            .iter()
            .filter(|part| matches!(part, Part::Text(..) | Part::Utf8(..)))
            .count()
            > 1
    });
    if !by_length && total > budget.max_shapes {
        return Err(format!("more than {} input shapes", budget.max_shapes));
    }
    let stride = stride(total);
    let scattered = |turn: usize| (turn as u128 * stride as u128 % total as u128) as usize;
    let levels = Mutex::new(Levels::new(&each, budget.from_length));
    let shortest = each
        .iter()
        .map(|(_, parts)| parts.iter().map(part_length).min().unwrap_or(0))
        .max()
        .unwrap_or(0)
        .max(budget.from_length);
    // Lengths below where the proof starts are proven already.
    budget
        .progress
        .proven_below
        .store(budget.from_length, Ordering::Relaxed);
    // The shapes are independent: every core takes the next one until all are done, a
    // counterexample turns up, or something can't be proven.
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let paths = AtomicUsize::new(0);
    let questions = AtomicUsize::new(0);
    let alone: Mutex<Option<String>> = Mutex::new(None);
    let stop = AtomicBool::new(false);
    let cut_any_way = AtomicBool::new(image.fills());
    let found: Mutex<Option<BTreeMap<String, Value>>> = Mutex::new(None);
    let failure: Mutex<Option<String>> = Mutex::new(None);
    // In length order, the shortest text length something couldn't be proven at, and why:
    // nothing that long or longer can extend the proof.
    let gap: Mutex<Option<(usize, String)>> = Mutex::new(None);
    let gap_at = AtomicUsize::new(usize::MAX);
    let note_gap = |length: usize, why: String| {
        if let Ok(mut gap) = gap.lock()
            && gap.as_ref().is_none_or(|(at, _)| length < *at)
        {
            *gap = Some((length, why));
            gap_at.store(length, Ordering::Relaxed);
        }
    };
    // In length order, each length's combinations handed out and finished: every length
    // below one with a longer length handed out after it, and all its own finished, is proven.
    let tally: Mutex<BTreeMap<usize, (usize, usize)>> = Mutex::new(BTreeMap::new());
    let count_finished = |length: usize| {
        let Ok(mut tally) = tally.lock() else {
            return;
        };
        if let Some((_, finished)) = tally.get_mut(&length) {
            *finished += 1;
        }
        let mut lengths = tally.iter().peekable();
        let mut proven_below = 0;
        while let Some((&length, &(handed, finished))) = lengths.next() {
            if handed != finished || lengths.peek().is_none() {
                break;
            }
            proven_below = length + 1;
        }
        budget
            .progress
            .proven_below
            .fetch_max(proven_below, Ordering::Relaxed);
    };
    // One worker a core, as many as the free memory leaves room for.
    let workers =
        watch::workers(std::thread::available_parallelism().map_or(1, std::num::NonZero::get))
            .min(total.max(1));
    let began = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                watch::yield_to_others();
                let mut solvers = Solvers::new(budget.question_ms);
                while !stop.load(Ordering::Relaxed) {
                    let turn = next.fetch_add(1, Ordering::Relaxed);
                    let (combination, length) = if by_length {
                        let Some(next) = levels.lock().ok().and_then(|mut levels| {
                            let next = levels.next()?;
                            if let Ok(mut tally) = tally.lock() {
                                tally.entry(next.1).or_default().0 += 1;
                            }
                            Some(next)
                        }) else {
                            break;
                        };
                        next
                    } else if turn < total {
                        (scattered(turn), 0)
                    } else {
                        break;
                    };
                    if by_length && length >= gap_at.load(Ordering::Relaxed) {
                        break;
                    }
                    if by_length && turn >= budget.max_shapes {
                        note_gap(
                            length,
                            format!(
                                "the proof covers at most {} input combinations",
                                budget.max_shapes
                            ),
                        );
                        break;
                    }
                    solvers.next();
                    let result = if budget.stopped() {
                        Err(Stop::Unproven(OUT_OF_TIME.into()))
                    } else {
                        prove_shape(image, block, &each, combination, budget, &mut solvers, hook)
                    };
                    match result {
                        Ok((count, asked, reason, any_cut)) => {
                            if !any_cut {
                                cut_any_way.store(false, Ordering::Relaxed);
                            }
                            budget.progress.done.fetch_add(1, Ordering::Relaxed);
                            if by_length {
                                count_finished(length);
                            }
                            paths.fetch_add(count, Ordering::Relaxed);
                            questions.fetch_add(asked, Ordering::Relaxed);
                            if let (Some(reason), Ok(mut alone)) = (reason, alone.lock()) {
                                alone.get_or_insert(reason);
                            }
                            // Once every worker has done two, and again at four times as many
                            // each time, the pace shows whether the rest fits the time left; if
                            // it can't, stop now and say so. In length order the proof instead
                            // goes as far as its time allows.
                            let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                            let checkpoint = !by_length
                                && finished >= 2 * workers
                                && (finished / (2 * workers)).is_power_of_two()
                                && finished.is_multiple_of(2 * workers)
                                && (finished / (2 * workers))
                                    .trailing_zeros()
                                    .is_multiple_of(2);
                            if checkpoint && finished < total {
                                let pace = began.elapsed().as_secs_f64() / finished as f64;
                                let needed = Duration::from_secs_f64(pace * total as f64);
                                if began + needed > budget.deadline {
                                    if let Ok(mut failure) = failure.lock() {
                                        failure.get_or_insert(format!(
                                            "its {total} input combinations would take about \
                                             {}; the proof has {}",
                                            minutes(needed),
                                            minutes(
                                                budget.deadline.saturating_duration_since(began)
                                            )
                                        ));
                                    }
                                    stop.store(true, Ordering::Relaxed);
                                }
                            }
                        }
                        Err(Stop::Counterexample(values)) => {
                            if let Ok(mut found) = found.lock() {
                                found.get_or_insert(values);
                            }
                            stop.store(true, Ordering::Relaxed);
                        }
                        Err(Stop::Unproven(reason) | Stop::Beyond(reason)) if by_length => {
                            note_gap(length, reason);
                        }
                        Err(Stop::Unproven(reason) | Stop::Beyond(reason)) => {
                            if let Ok(mut failure) = failure.lock() {
                                failure.get_or_insert(reason);
                            }
                            stop.store(true, Ordering::Relaxed);
                        }
                    }
                }
            });
        }
    });
    if let Some(values) = found.into_inner().ok().flatten() {
        return Ok(Verdict::Counterexample(values));
    }
    let done = done.into_inner();
    let out_of_time = |reason: String| {
        if reason == OUT_OF_TIME {
            format!("ran out of time after {done} of {total} input combinations")
        } else {
            reason
        }
    };
    if let Some(reason) = failure.into_inner().ok().flatten() {
        return Err(out_of_time(reason));
    }
    let characters = block.vars.iter().any(|(_, generator)| {
        matches!(generator, Generator::Str { charset, .. } if !charset.iter().all(char::is_ascii))
    });
    // The longest the idea lets any of the block's texts be.
    let of = block
        .vars
        .iter()
        .filter_map(|(_, generator)| match generator {
            Generator::Str { max, .. } => Some(*max),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let from = budget.from_length;
    let bound = match gap.into_inner().ok().flatten() {
        // Not even the shortest texts were all proven, when no earlier proof proved any.
        Some((length, why)) if length <= shortest && from == 0 => {
            return Err(out_of_time(why));
        }
        Some((length, why)) => Some(Bound {
            bytes: length.max(from) - 1,
            of,
            more: why == OUT_OF_TIME,
            why: if why == OUT_OF_TIME {
                let unit = if characters { "characters" } else { "bytes" };
                format!("the proof's time ran out at {length} {unit}")
            } else {
                why
            },
            characters,
        }),
        // Every length laid out was proven: the rest are the next check's.
        None => block
            .vars
            .iter()
            .filter_map(|(_, generator)| match generator {
                Generator::Str { min, max, charset } if !charset.iter().all(char::is_ascii) => {
                    let (longest, why) = utf8_reach(*min, *max, charset);
                    why.map(|why| (longest, why, false))
                }
                Generator::Str { min, max, .. } => {
                    let longest = laid_out(*min, *max, from);
                    (longest < *max).then(|| {
                        (
                            longest,
                            format!("the proof went through every text up to {longest} bytes"),
                            true,
                        )
                    })
                }
                _ => None,
            })
            .min_by_key(|(cap, ..)| *cap)
            .map(|(cap, why, more)| Bound {
                bytes: cap,
                of,
                more,
                why,
                characters,
            }),
    };
    Ok(Verdict::Proven {
        shapes: done,
        paths: paths.into_inner(),
        questions: questions.into_inner(),
        alone: alone.into_inner().ok().flatten(),
        bound,
        cut_any_way: cut_any_way.into_inner(),
        unreached: image.uncalled(),
        induction: None,
        lookback: 0,
    })
}

/// `bytes` in the pieces `delivery` cuts them into.
fn cut(bytes: &[Term], delivery: Delivery) -> Vec<Vec<Term>> {
    let mut pieces = Vec::new();
    let mut start = 0;
    for length in delivery.cuts(bytes.len()) {
        pieces.push(bytes[start..start + length].to_vec());
        start += length;
    }
    pieces
}

/// What must hold for a run to have used the network as the test's peers expect: exactly their
/// connections, the server's each sent exactly what the server waits for, the client's each
/// answered as the test says, and the program still serving at the end when it must be.
fn talks(
    pool: &mut Pool,
    block: &Generated,
    known: &Known<'_>,
    run: &World,
    receives: &[Vec<Term>],
) -> Result<Term, String> {
    let ended_serving = matches!(run.end, Some(End::Serving));
    let made: Vec<_> = run
        .connections
        .iter()
        .filter(|connection| !connection.accepted)
        .collect();
    let taken: Vec<_> = run
        .connections
        .iter()
        .filter(|connection| connection.accepted)
        .collect();
    // Clients at once are followed in every order their steps can come only where the program
    // waits on them with poll; a server that doesn't fails their tests anyway.
    if block.clients_at_once && !run.polled() {
        return Err(
            "its clients connect at once, which a proof follows only for a server that waits on \
             them with poll"
                .into(),
        );
    }
    // The connections lengua made: each answered by a test's server must have sent exactly what
    // that server waits for. An ended one takes nothing.
    let answered: Vec<_> = made.iter().filter(|connection| !connection.ended).collect();
    if answered.len() != block.server.len()
        || taken.len() != block.client.len()
        || block.serves != ended_serving
    {
        return Ok(pool.boolean(false));
    }
    let mut checks = Vec::new();
    for connection in answered {
        let Some(wanted) = receives.get(connection.exchange) else {
            return Ok(pool.boolean(false));
        };
        if connection.sent.len() != wanted.len() {
            return Ok(pool.boolean(false));
        }
        for (sent, byte) in connection.sent.iter().zip(wanted) {
            checks.push(pool.eq(*sent, *byte));
        }
    }
    for (connection, (_, answer)) in taken.iter().zip(&block.client) {
        checks.push(text_is(pool, &connection.sent, answer, known)?);
    }
    Ok(pool.all(&checks))
}

/// What must hold for a server that is still serving at the end: its output so far is what the
/// test says. Its exit code is never seen.
fn serving(
    pool: &mut Pool,
    block: &Generated,
    known: &Known<'_>,
    run: &World,
) -> Result<Term, String> {
    if !block.files_after.is_empty() {
        return Err("a server's files after its run".into());
    }
    let stdout = text_is(pool, &run.stdout, &block.stdout, known)?;
    let stderr = text_is(pool, &run.stderr, &block.stderr, known)?;
    Ok(pool.bool_and(stdout, stderr))
}

/// The conditions `condition` is the disjunction of.
fn disjuncts(pool: &Pool, condition: Term) -> Vec<Term> {
    let mut parts = Vec::new();
    let mut pending = vec![condition];
    while let Some(current) = pending.pop() {
        match *pool.node(current) {
            Node::BoolOr(a, b) => pending.extend([a, b]),
            _ => parts.push(current),
        }
    }
    parts
}

/// The conditions `condition` is the conjunction of.
fn conjuncts(pool: &Pool, condition: Term) -> Vec<Term> {
    let mut parts = Vec::new();
    let mut pending = vec![condition];
    while let Some(current) = pending.pop() {
        match *pool.node(current) {
            Node::BoolAnd(a, b) => pending.extend([a, b]),
            _ => parts.push(current),
        }
    }
    parts
}

/// A part's text length: 0 for a part that isn't text.
fn part_length(part: &Part) -> usize {
    match part {
        Part::Text(length, _) | Part::Within(_, length, _) => *length,
        Part::Utf8(widths, _) => widths.len(),
        _ => 0,
    }
}

/// The combinations of the variables' parts in order of their longest text, one length at a
/// time: every combination whose longest text is `L` long before any longer one, without
/// listing them all first. At each length, a combination belongs to the first variable whose
/// text is that long, so none comes twice.
struct Levels {
    /// Each variable's parts as (text length, part number), shortest first.
    sorted: Vec<Vec<(usize, usize)>>,
    /// The combination number's place value of each variable.
    places: Vec<usize>,
    /// The lengths left to go through, longest last.
    lengths: Vec<usize>,
    /// The length being gone through, the variable first at it, and for each variable the
    /// range of its sorted parts allowed and where it is in it.
    at: Option<(usize, usize)>,
    ranges: Vec<(usize, usize)>,
    counter: Vec<usize>,
}

impl Levels {
    /// The levels of `each`'s combinations from the longest text length `from` on.
    fn new(each: &[(String, Vec<Part>)], from: usize) -> Self {
        let sorted: Vec<Vec<(usize, usize)>> = each
            .iter()
            .map(|(_, parts)| {
                let mut sorted: Vec<(usize, usize)> = parts
                    .iter()
                    .enumerate()
                    .map(|(number, part)| (part_length(part), number))
                    .collect();
                sorted.sort_unstable();
                sorted
            })
            .collect();
        let mut places = Vec::with_capacity(each.len());
        let mut place = 1_usize;
        for (_, parts) in each {
            places.push(place);
            place = place.saturating_mul(parts.len());
        }
        let mut lengths: Vec<usize> = sorted.iter().flatten().map(|(length, _)| *length).collect();
        lengths.sort_unstable();
        lengths.dedup();
        lengths.retain(|length| *length >= from);
        lengths.reverse();
        Self {
            sorted,
            places,
            lengths,
            at: None,
            ranges: Vec::new(),
            counter: Vec::new(),
        }
    }

    /// Sets up the combinations at `length` whose first variable that long is `first`; false
    /// when there are none.
    fn start(&mut self, length: usize, first: usize) -> bool {
        self.ranges = self
            .sorted
            .iter()
            .enumerate()
            .map(|(variable, parts)| {
                let below = parts.partition_point(|(part, _)| *part < length);
                let through = parts.partition_point(|(part, _)| *part <= length);
                match variable.cmp(&first) {
                    std::cmp::Ordering::Less => (0, below),
                    std::cmp::Ordering::Equal => (below, through),
                    std::cmp::Ordering::Greater => (0, through),
                }
            })
            .collect();
        self.counter = self.ranges.iter().map(|(from, _)| *from).collect();
        self.at = Some((length, first));
        self.ranges.iter().all(|(from, to)| from < to)
    }

    /// The next combination's number and its longest text's length.
    fn next(&mut self) -> Option<(usize, usize)> {
        loop {
            if let Some((length, first)) = self.at {
                if self
                    .counter
                    .iter()
                    .zip(&self.ranges)
                    .all(|(at, (_, to))| at < to)
                {
                    let combination = self
                        .counter
                        .iter()
                        .zip(&self.sorted)
                        .zip(&self.places)
                        .map(|((at, parts), place)| parts[*at].1 * place)
                        .sum();
                    // Count up, the first variable fastest.
                    for (at, (from, to)) in self.counter.iter_mut().zip(&self.ranges) {
                        *at += 1;
                        if *at < *to {
                            break;
                        }
                        *at = *from;
                    }
                    if self
                        .counter
                        .iter()
                        .zip(&self.ranges)
                        .all(|(at, (from, _))| at == from)
                    {
                        // Wrapped around: this group is done.
                        let done = self.ranges.iter().map(|(_, to)| *to).collect();
                        self.counter = done;
                    }
                    return Some((combination, length));
                }
                if first + 1 < self.sorted.len() && self.start(length, first + 1) {
                    continue;
                }
                if first + 1 < self.sorted.len() {
                    // An empty group: try the next first variable.
                    self.at = Some((length, first + 1));
                    let to: Vec<usize> = self.ranges.iter().map(|(_, to)| *to).collect();
                    self.counter = to;
                    continue;
                }
                self.at = None;
            }
            let length = self.lengths.pop()?;
            if !self.start(length, 0) {
                let to: Vec<usize> = self.ranges.iter().map(|(_, to)| *to).collect();
                self.counter = to;
            }
        }
    }
}

/// A step through `0..total` that visits every combination once in a scattered order: an odd
/// number near the golden section of `total` with no factor in common with it.
fn stride(total: usize) -> usize {
    if total < 3 {
        return 1;
    }
    let gcd = |mut a: usize, mut b: usize| {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    };
    let mut step = (total as f64 * 0.618_034) as usize | 1;
    while gcd(step, total) != 1 {
        step += 2;
    }
    step
}

/// What a proof says when its time is up.
const OUT_OF_TIME: &str = "the proof ran out of time";

/// A duration in whole minutes, or seconds when under two minutes.
fn minutes(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 120 {
        format!("{seconds} seconds")
    } else {
        format!("{} minutes", seconds.div_ceil(60))
    }
}

/// Why proving one shape stopped short.
enum Stop {
    /// Inputs where the program disagrees with the formulas.
    Counterexample(BTreeMap<String, Value>),
    /// Something the proof can't settle.
    Unproven(String),
    /// Inputs past what a proof covers, and why.
    Beyond(String),
}

impl From<String> for Stop {
    fn from(reason: String) -> Self {
        Self::Unproven(reason)
    }
}

impl From<&str> for Stop {
    fn from(reason: &str) -> Self {
        Self::Unproven(reason.to_owned())
    }
}

/// Proves one combination of shapes, with its own terms and solver so questions only carry what
/// they're about. Returns how many paths it followed, and why one solver answered alone when no
/// second one checked it.
fn prove_shape<P: Program>(
    image: &P,
    block: &Generated,
    each: &[(String, Vec<Part>)],
    combination: usize,
    budget: &Budget,
    solvers: &mut Solvers,
    hook: Option<&exec::induction::Hook>,
) -> Result<(usize, usize, Option<String>, bool), Stop> {
    let mut pool = Pool::default();
    let mut rest = combination;
    let mut inputs = BTreeMap::new();
    let mut numbers = BTreeMap::new();
    let mut constraints = Vec::new();
    // The fixed values chosen, which a counterexample reports with the numbers found, and the
    // texts drawn, byte by byte.
    let mut chosen = BTreeMap::new();
    let mut texts: BTreeMap<String, Vec<Term>> = BTreeMap::new();
    // The lengths of the texts whose length the input chooses.
    let mut lengths: BTreeMap<String, Term> = BTreeMap::new();
    let mut delivery = Delivery::Whole;
    for (name, parts) in each {
        let number = rest % parts.len();
        rest /= parts.len();
        if let Part::Delivery(chosen) = parts[number] {
            delivery = chosen;
            continue;
        }
        let input = match &parts[number] {
            Part::Number(shape, places) => input(&mut pool, name, number, shape, *places),
            Part::Fixed(value) => {
                chosen.insert(name.clone(), value.clone());
                fixed(&mut pool, value)?
            }
            Part::Text(length, characters) => {
                let input = text_input(&mut pool, name, *length, characters);
                texts.insert(name.clone(), input.text.clone());
                input
            }
            Part::Within(shortest, longest, characters) => {
                let (input, length) =
                    within_input(&mut pool, name, (*shortest, *longest), characters);
                texts.insert(name.clone(), input.text.clone());
                lengths.insert(name.clone(), length);
                input
            }
            Part::Utf8(widths, characters) => {
                let input = utf8_input(&mut pool, name, widths, characters);
                texts.insert(name.clone(), input.text.clone());
                input
            }
            Part::Bytes(count) => bytes_input(&mut pool, name, *count),
            Part::Plain(low, high, places) => plain_input(&mut pool, name, *low, *high, *places),
            Part::Delivery(_) => continue,
        };
        constraints.extend(&input.constraints);
        if let Some(number) = input.number {
            numbers.insert(name.clone(), number);
        }
        inputs.insert(name.clone(), input);
    }
    let args = block
        .args
        .iter()
        .map(|arg| pattern_bytes(&mut pool, arg, &inputs))
        .collect::<Result<Vec<_>, _>>()?;
    // Each environment variable as the kernel gives it: `NAME=value`.
    let env = block
        .env
        .iter()
        .map(|(name, value)| {
            let mut bytes = exec::bytes(&mut pool, format!("{name}=").as_bytes());
            bytes.extend(pattern_bytes(&mut pool, value, &inputs)?);
            Ok(bytes)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let stdin = pattern_bytes(&mut pool, &block.stdin, &inputs)?;
    // The model has one read return everything that's there, which a pipe guarantees only for
    // what one write of at most 4096 bytes put in it; fill reads any length cut any way.
    if stdin.len() > PIPE_WRITE && !image.fills() {
        return Err(Stop::Beyond(format!(
            "standard input past {PIPE_WRITE} bytes is more than one pipe write, which the proof \
             reads whole; read it with fill to be proven for any length"
        )));
    }
    let mut files = BTreeMap::new();
    for (name, template) in &block.files {
        files.insert(name.clone(), pattern_bytes(&mut pool, template, &inputs)?);
    }
    let random = random_stream(&mut pool, block, &inputs)?;
    let clock = clock_readings(&mut pool, block, &inputs)?;
    // The test's server waits for exactly what it must receive; what it and the test's client
    // send arrives cut as a delivery cuts it.
    let receives: Vec<Vec<Term>> = block
        .server
        .iter()
        .map(|(_, wanted, _)| pattern_bytes(&mut pool, wanted, &inputs))
        .collect::<Result<_, _>>()?;
    let replies: Vec<Vec<Term>> = block
        .server
        .iter()
        .map(|(_, _, reply)| pattern_bytes(&mut pool, reply, &inputs))
        .collect::<Result<_, _>>()?;
    // Each connection the idea names is made before the program starts: answered by the test's
    // server for it, or ended.
    let made = made_connections(
        block.connections,
        block.server.iter().map(|(connection, _, _)| *connection),
    );
    let sends: Vec<(Vec<Term>, bool)> = block
        .client
        .iter()
        .map(|(sends, receives)| {
            Ok((
                pattern_bytes(&mut pool, sends, &inputs)?,
                !receives.is_empty(),
            ))
        })
        .collect::<Result<_, String>>()?;
    // Clients at once take their steps in every order, and also with each but the newest
    // holding its last byte until every newer client has its answer, as the test's do: then a
    // server that waits on one client while another waits on it is caught.
    let newest = sends.len().saturating_sub(1);
    let network_for = |delivery: Delivery, hold: bool| world::net::Network {
        server: receives
            .iter()
            .zip(&replies)
            .map(|(wanted, reply)| (wanted.clone(), cut(reply, delivery)))
            .collect(),
        made: made.clone(),
        client: sends
            .iter()
            .enumerate()
            .map(|(index, (bytes, waits))| {
                let held = hold && index < newest && !bytes.is_empty();
                let mut pieces = if held {
                    cut(&bytes[..bytes.len() - 1], delivery)
                } else {
                    cut(bytes, delivery)
                };
                if held {
                    pieces.push(bytes[bytes.len() - 1..].to_vec());
                }
                pieces.retain(|piece| !piece.is_empty());
                world::net::Client {
                    pieces,
                    waits: *waits,
                    held,
                }
            })
            .collect(),
        listening: budget.listens,
        at_once: block.clients_at_once,
        eager: false,
    };
    // fill, where the program has it, is read as a whole for every way its input can be cut.
    let every_cut = image.fills();
    let mut base = image
        .begin(&mut pool, &args, &env, stdin)
        .with_files(files)
        .with_folders(block.folders.clone())
        .with_supply(random, clock)
        .with_stdout_closed(block.stdout_closed);
    if every_cut {
        base = base.for_every_cut();
    }
    // A text whose length the input chooses is all of standard input, which ends after it.
    if !lengths.is_empty() {
        let read = match block.stdin.pieces()[..] {
            [Piece::Expr(crate::expr::Expr::Var(read))] => lengths.get(read).copied(),
            _ => None,
        };
        let length = read
            .filter(|_| lengths.len() == 1 && block.args.is_empty() && block.files.is_empty())
            .ok_or("a text whose length the input chooses that isn't all of standard input")?;
        base = base.with_stdin_length(length);
    }
    // In an induction's base, each count the loop keeps holds what was read before the head
    // too: the formula's count goes on from there. Where the counts look back, each run reads
    // a buffer-full from the head first, and its own count there is what the final piece's
    // goes on from.
    let mut offsets = HashMap::new();
    let mut relative = None;
    if let Some(hook) = hook {
        base = P::hooked(base, std::rc::Rc::new(hook.clone()))?;
        let pieces = block.stdout.pieces();
        let mut folds = Vec::new();
        for (slot, fold) in &hook.folds {
            let Some(Piece::Expr(expr)) = pieces.get(fold.placeholder) else {
                return Err("a count with no placeholder".into());
            };
            if hook.lead.is_some() {
                let back = block
                    .vars
                    .first()
                    .and_then(|(name, _)| formula::lookback(expr, name))
                    .ok_or("a count that doesn't look back")?;
                folds.push((*slot, fold.start, std::ptr::from_ref(*expr), back));
                continue;
            }
            let value = fold.value(&mut pool, *slot);
            let wide = pool.zero_extend(value, WIDTH - 64);
            let start = pool.bv(WIDTH, fold.start);
            let before = pool.sub(wide, start);
            let most = i128::try_from(fold.most).map_err(|_| "a count too large")?;
            // A count of a part that overlaps itself goes on from the phase its slot stands
            // for at the head, the same value the head gives it.
            let phase = match hook
                .phases
                .iter()
                .find(|(_, phase)| phase.placeholder == fold.placeholder)
            {
                Some((phase_slot, phase)) => {
                    let part = block
                        .vars
                        .first()
                        .and_then(|(name, _)| formula::bordered_part(expr, name))
                        .ok_or("a phase for a count of a part with no border")?;
                    let held = phase.value(&mut pool, *phase_slot);
                    Some(phase.phase(&mut pool, held, part.len()))
                }
                None => None,
            };
            offsets.insert(
                std::ptr::from_ref(*expr),
                Offset {
                    before,
                    low: 0,
                    high: most,
                    window: None,
                    phase,
                },
            );
        }
        if let Some(characters) = &hook.lead {
            relative = Some(Relative {
                folds,
                characters: characters.clone(),
            });
        }
    }
    base.world.path = constraints;
    let networked = !block.server.is_empty() || !block.client.is_empty();
    let began = Instant::now();
    let limits = (budget.deadline, budget.deadline.max(began + MIN_SHAPE_TIME));
    let questions_before = solvers.solver.as_ref().map_or(0, |solver| solver.questions);
    let holds: &[bool] = if block.clients_at_once {
        &[true, false]
    } else {
        &[false]
    };
    let mut deliveries: Vec<(Delivery, bool)> =
        holds.iter().map(|hold| (delivery, *hold)).collect();
    let mut any_cut = every_cut;
    let mut count = 0;
    let mut turn = 0;
    while let Some(&(delivery, hold)) = deliveries.get(turn) {
        turn += 1;
        let start = base.clone().with_network(network_for(delivery, hold));
        // Branches no witness reaches are followed without asking first; a run that can't
        // happen is then settled where it ends. If that makes too many runs, the branches are
        // asked about after all.
        let careful = start.clone();
        let (runs, guessed) = {
            let mut oracle = Witnessed::new(solvers, limits, combination as u64, true);
            let runs = exec::explore(image, start, &mut pool, &mut oracle, budget.max_paths)?;
            (runs, oracle.guessed)
        };
        let too_many = Some(End::TooManyPaths);
        let (runs, guessed) = if guessed && runs.iter().any(|run| run.world.end == too_many) {
            let mut oracle = Witnessed::new(solvers, limits, combination as u64, false);
            let runs = exec::explore(image, careful, &mut pool, &mut oracle, budget.max_paths)?;
            (runs, false)
        } else {
            (runs, guessed)
        };
        // A run that reads a stream with read itself sees the pieces it arrives in: what a
        // test's peer sends is then proven as each of the tests' cuts delivers it.
        if runs.iter().any(|run| run.world.raw_reads) {
            if any_cut && networked {
                for other in Delivery::ALL.into_iter().filter(|other| *other != delivery) {
                    deliveries.extend(holds.iter().map(|hold| (other, *hold)));
                }
            }
            any_cut = false;
        }
        count += runs.len();
        let explored = began.elapsed();
        let asked =
            |solvers: &Solvers| solvers.solver.as_ref().map_or(0, |solver| solver.questions);
        let questions = asked(solvers);
        let known = Known {
            numbers: &numbers,
            fixed: &chosen,
            texts: &texts,
            lengths: Some(&lengths),
            translated: std::cell::RefCell::new(HashMap::new()),
            offsets: offsets.clone(),
            relative: relative.as_ref(),
        };
        check_runs(
            &mut pool,
            block,
            &known,
            runs.into_iter().map(|run| run.world).collect(),
            &receives,
            guessed,
            budget,
            solvers,
            combination,
            networked.then_some(delivery),
        )?;
        // What the pool made after finding the memory short is nothing to go on: none of its
        // shape is proven.
        if pool.out_of_room() {
            return Err(watch::OUT_OF_MEMORY.into());
        }
        if std::env::var_os("LENGUA_PROVE_TRACE").is_some() {
            eprintln!(
                "shape {combination}: {count} runs, {questions} questions exploring in \
                 {explored:?}, {} checking in {:?}, {} terms",
                asked(solvers) - questions,
                began.elapsed() - explored,
                pool.len(),
            );
        }
    }
    let alone = solvers
        .solver
        .as_ref()
        .filter(|_| solvers.used)
        .and_then(|solver| solver.alone.clone());
    let questions = solvers.solver.as_ref().map_or(0, |solver| solver.questions);
    Ok((count, questions - questions_before, alone, any_cut))
}

/// Checks each of `runs`, by the world each finished in, against the test's formulas: where the
/// terms don't settle it, the solvers look for inputs that take the run and break the check.
#[allow(clippy::too_many_arguments)]
fn check_runs(
    pool: &mut Pool,
    block: &Generated,
    known: &Known<'_>,
    runs: Vec<World>,
    receives: &[Vec<Term>],
    guessed: bool,
    budget: &Budget,
    solvers: &mut Solvers,
    combination: usize,
    delivery: Option<Delivery>,
) -> Result<(), Stop> {
    let (numbers, chosen, texts) = (known.numbers, known.fixed, known.texts);
    let lengths = known.lengths.cloned().unwrap_or_default();
    // A counterexample shows each number and each text; a question asks for the texts short
    // enough to have every byte held to its characters, and the rest only when it finds inputs.
    let values: Vec<Term> = numbers
        .values()
        .map(|number| number.term)
        .chain(lengths.values().copied())
        .chain(
            texts
                .values()
                .filter(|bytes| bytes.len() <= MAX_CHECKED_TEXT)
                .flat_map(|bytes| bytes.iter().copied()),
        )
        .collect();
    let every_value: Vec<Term> = numbers
        .values()
        .map(|number| number.term)
        .chain(lengths.values().copied())
        .chain(texts.values().flat_map(|bytes| bytes.iter().copied()))
        .collect();
    // A text too long to have each byte held to its characters on every question is held to
    // them where a question finds inputs: inputs with other characters prove nothing.
    let mut held: Vec<Term> = Vec::new();
    for (name, bytes) in texts {
        let characters = block
            .vars
            .iter()
            .find_map(|(var, generator)| match generator {
                Generator::Str { charset, .. }
                    if var == name && charset.iter().all(char::is_ascii) =>
                {
                    Some(
                        charset
                            .iter()
                            .map(|character| *character as u8)
                            .collect::<Vec<u8>>(),
                    )
                }
                _ => None,
            });
        if let Some(characters) = characters {
            match lengths.get(name) {
                // A text whose length the input chooses, whatever its length, only where it's
                // inside.
                Some(length) => held.extend(
                    bytes
                        .iter()
                        .enumerate()
                        .map(|(index, byte)| held_inside(pool, *byte, index, *length, &characters)),
                ),
                None if bytes.len() > MAX_CHECKED_TEXT => {
                    held.extend(bytes.iter().map(|byte| pool.is_one_of(*byte, &characters)));
                }
                None => {}
            }
        }
    }
    // The inputs a model gives, with how what the program read arrived.
    let found =
        |model: &HashMap<Term, u128>, run: &World| -> Result<BTreeMap<String, Value>, Stop> {
            let mut found = chosen.clone();
            // Cut where fill stopped short, or as the test's peer delivered it.
            let arrived = run
                .fill_cut
                .map(Delivery::At)
                .or((run.raw_cuts != 0).then_some(Delivery::Cuts(run.raw_cuts)))
                .or(delivery);
            if let Some(arrived) = arrived {
                found.insert(DELIVERY.to_owned(), Value::Str(arrived.describe()));
            }
            for (name, bytes) in texts {
                // A text whose length the input chose is as long as the model says.
                let inside = match lengths.get(name) {
                    Some(length) => {
                        let length = model
                            .get(length)
                            .copied()
                            .ok_or("a counterexample without its text's length")?;
                        usize::try_from(length)
                            .unwrap_or(usize::MAX)
                            .min(bytes.len())
                    }
                    None => bytes.len(),
                };
                let text: Vec<u8> = bytes[..inside]
                    .iter()
                    .map(|byte| model.get(byte).map(|value| *value as u8))
                    .collect::<Option<_>>()
                    .ok_or("a counterexample in a text too long to show")?;
                found.insert(
                    name.clone(),
                    Value::Str(String::from_utf8_lossy(&text).into_owned()),
                );
            }
            for (name, number) in numbers {
                let value = model
                    .get(&number.term)
                    .copied()
                    .ok_or("a counterexample without values")?;
                found.insert(
                    name.clone(),
                    Value::Int(i128::from_ne_bytes(value.to_ne_bytes())),
                );
            }
            Ok(found)
        };
    // In the base of an induction whose counts look back: the formulas as the runs' own counts
    // at the head's second arrival set them, kept while they're the same.
    let mut own: Option<Known<'_>> = None;
    // The runs' checks stop for time as a shape's branches do: at the deadline, or the least
    // time a shape gets from when they start.
    let hard = budget.deadline.max(Instant::now() + MIN_SHAPE_TIME);
    for run in runs {
        if budget.cancel.load(Ordering::Relaxed) || Instant::now() > hard {
            return Err(OUT_OF_TIME.into());
        }
        if pool.out_of_room() {
            return Err(watch::OUT_OF_MEMORY.into());
        }
        // In the base of an induction whose counts look back, the buffer-full each run reads
        // from the head is held to the text's characters too.
        let mut held = held.clone();
        if let Some(relative) = known.relative {
            held.extend(
                run.stdin_bytes()
                    .iter()
                    .map(|byte| pool.is_one_of(*byte, &relative.characters)),
            );
        }
        // The memory rules first: a run that broke one, or an access whose place the input
        // chooses that some input pushes out of its object.
        let mut broken_rules = run.obligations.clone();
        if let Some(End::Unsafe(problem)) = &run.end {
            broken_rules.insert(0, (pool.boolean(true), problem.clone()));
        }
        for (condition, problem) in broken_rules {
            if pool.truth(condition) == Some(false) {
                continue;
            }
            let mut conditions = run.path.clone();
            conditions.push(condition);
            let solver = solvers.get(budget.deadline)?;
            let (answer, model) = solver
                .check(pool, &conditions, &values)
                .map_err(|error| format!("the solver failed: {error}"))?;
            let (answer, model) =
                within(pool, solver, answer, model, conditions, &held, &every_value)?;
            match answer {
                Answer::Unsat => {}
                Answer::Sat => {
                    let mut values = found(&model, &run)?;
                    values.insert(UNSAFE.to_owned(), Value::Str(problem));
                    return Err(Stop::Counterexample(values));
                }
                Answer::Unknown => {
                    if solver.out_of_time() {
                        return Err(OUT_OF_TIME.into());
                    }
                    return Err(
                        format!("a path couldn't be settled: {}", solver.why_unknown).into(),
                    );
                }
            }
        }
        // There, the run's own count after that buffer-full is what its final piece's goes on
        // from, and a run that never read the buffer-full must be one no input takes.
        let known = match known.relative {
            None => known,
            Some(relative) => {
                let (Some(snapshot), Some(second)) = (run.at_head(), run.at_second()) else {
                    let solver = solvers.get(budget.deadline)?;
                    let (answer, _) = solver
                        .check(pool, &run.path, &[])
                        .map_err(|error| format!("the solver failed: {error}"))?;
                    if answer == Answer::Unsat {
                        continue;
                    }
                    return Err(Stop::Unproven(
                        "a run of the induction's base ended before the buffer-full it reads \
                         from the read loop's head"
                            .into(),
                    ));
                };
                let lead = &run.stdin_bytes()[snapshot.stdin_read..second.stdin_read];
                let mut offsets = HashMap::new();
                for (slot, start, expr, back) in &relative.folds {
                    let value = second.held(*slot);
                    let wide = pool.zero_extend(value, WIDTH - 64);
                    let first = pool.bv(WIDTH, *start);
                    let before = pool.sub(wide, first);
                    let (lowest, highest) = pool.range(value);
                    let start = i128::try_from(*start).map_err(|_| "a count too large")?;
                    let low = i128::try_from(lowest).map_err(|_| "a count too large")? - start;
                    let high = i128::try_from(highest).map_err(|_| "a count too large")? - start;
                    let window = lead[lead.len().saturating_sub(*back)..].to_vec();
                    offsets.insert(
                        *expr,
                        Offset {
                            before,
                            low,
                            high,
                            window: Some(window),
                            phase: None,
                        },
                    );
                }
                // Runs that left the head the same way share their formulas' terms.
                if own.as_ref().is_none_or(|own| own.offsets != offsets) {
                    own = Some(Known {
                        numbers: known.numbers,
                        fixed: known.fixed,
                        texts: known.texts,
                        lengths: known.lengths,
                        translated: std::cell::RefCell::new(HashMap::new()),
                        offsets,
                        relative: None,
                    });
                }
                own.as_ref().ok_or("a run with no formulas")?
            }
        };
        let holds = match &run.end {
            Some(End::Exit(status)) => {
                let output = passes(pool, block, known, &run, *status)?;
                let talked = talks(pool, block, known, &run, receives)?;
                pool.bool_and(output, talked)
            }
            Some(End::Serving) if block.serves => {
                let output = serving(pool, block, known, &run)?;
                let talked = talks(pool, block, known, &run, receives)?;
                pool.bool_and(output, talked)
            }
            Some(End::Serving | End::Stuck(_)) => pool.boolean(false),
            Some(End::Signal(_)) => pool.boolean(false),
            // Only the step of a proof by induction stops at the read loop's head.
            Some(End::Looped) => {
                return Err(Stop::Unproven("a run stopped at its read loop".into()));
            }
            // No input takes this run: the memory rules found none.
            Some(End::Unsafe(_)) => continue,
            Some(End::Unsupported(reason)) => {
                // A run followed without asking may be one that can't happen.
                if guessed {
                    let solver = solvers.get(budget.deadline)?;
                    let (answer, _) = solver
                        .check(pool, &run.path, &[])
                        .map_err(|error| format!("the solver failed: {error}"))?;
                    if answer == Answer::Unsat {
                        continue;
                    }
                }
                return Err(Stop::Unproven(reason.clone()));
            }
            Some(End::TooManyPaths) => {
                return Err(
                    format!("more than {} paths through the code", budget.max_paths).into(),
                );
            }
            None => return Err("a run that didn't end".into()),
        };
        // What the output check comes to where the run's own branches went as they did.
        let mut derived = HashMap::new();
        let holds = settled_with(pool, &run.path, holds, MAX_CHOICES, &mut derived);
        let broken = pool.bool_not(holds);
        // The run's own conditions as the same facts rewrite them, each with what the others
        // say and the comparisons they settle, never with its own facts, which would make it
        // true: a count the program built before its path said how long the text is, there as
        // the formula's is here.
        let mut facts = path_facts(pool, &run.path);
        facts.extend(derived.iter().map(|(term, truth)| (*term, *truth)));
        let mut done = HashMap::new();
        let rewritten: Vec<Term> = run
            .path
            .iter()
            .map(|condition| rewritten_condition(pool, *condition, &facts, &mut done, 0))
            .collect();
        // Output whose terms are the formulas' own can't break: nothing to ask. Nor can output
        // whose every condition is one of the run's own branches, as a printed number's digit
        // count is the print loop's.
        let holding = pool.bool_not(broken);
        let branched = |part: &Term| {
            pool.truth(*part) == Some(true)
                || run
                    .path
                    .iter()
                    .chain(&rewritten)
                    .any(|condition| pool.equivalent(*condition, *part))
        };
        if pool.truth(broken) == Some(false)
            || disjuncts(pool, holding)
                .iter()
                .any(|case| conjuncts(pool, *case).iter().all(branched))
        {
            continue;
        }
        if let Some(explain) = std::env::var_os("LENGUA_PROVE_EXPLAIN") {
            eprintln!(
                "combination {combination} asks: {}",
                pool.sketch(broken, 14)
            );
            // `path` shows the run's own conditions too.
            if explain == "path" {
                for condition in &run.path {
                    eprintln!("  given: {}", pool.sketch(*condition, 7));
                }
            }
        }
        let mut conditions = run.path.clone();
        conditions.push(broken);
        let solver = solvers.get(budget.deadline)?;
        let (answer, model) = solver
            .check(pool, &conditions, &values)
            .map_err(|error| format!("the solver failed: {error}"))?;
        let (answer, model) = within(pool, solver, answer, model, conditions, &held, &every_value)?;
        match answer {
            Answer::Unsat => {}
            Answer::Sat => return Err(Stop::Counterexample(found(&model, &run)?)),
            Answer::Unknown => {
                if solver.out_of_time() {
                    return Err(OUT_OF_TIME.into());
                }
                return Err(format!("a path couldn't be settled: {}", solver.why_unknown).into());
            }
        }
    }
    Ok(())
}

/// A question's answer once the long texts' bytes are held to their characters: where it found
/// inputs, it's asked again with `held`, so a counterexample is one the idea can give.
fn within(
    pool: &mut Pool,
    solver: &mut Solver,
    answer: Answer,
    model: HashMap<Term, u128>,
    mut conditions: Vec<Term>,
    held: &[Term],
    values: &[Term],
) -> Result<(Answer, HashMap<Term, u128>), Stop> {
    if answer != Answer::Sat || held.is_empty() {
        return Ok((answer, model));
    }
    conditions.extend(held);
    solver
        .check(pool, &conditions, values)
        .map_err(|error| Stop::Unproven(format!("the solver failed: {error}")))
}

/// The connections lengua makes before a test's program starts, in the idea's order: each the
/// place in the test's servers of the one that answers it, or `None` for one handed over ended.
fn made_connections(count: usize, answering: impl Iterator<Item = usize>) -> Vec<Option<usize>> {
    let mut made = vec![None; count];
    for (server, connection) in answering.enumerate() {
        if let Some(slot) = made.get_mut(connection) {
            *slot = Some(server);
        }
    }
    made
}

/// A sample's text as the model takes it.
fn sample_text(text: &str) -> String {
    text.to_owned()
}

/// The state a sample's run starts in: its arguments, input, files, folders, environment,
/// network, random bytes and clock, all as the sample has them.
fn sample_start<P: Program>(
    image: &P,
    pool: &mut Pool,
    case: &crate::testfile::Case,
    listens: bool,
) -> State<P::Core> {
    let fill = sample_text;
    let args: Vec<Vec<Term>> = case
        .args
        .iter()
        .map(|arg| exec::bytes(pool, fill(arg).as_bytes()))
        .collect();
    let stdin = exec::bytes(pool, fill(&case.stdin).as_bytes());
    let files = case
        .files
        .iter()
        .map(|(name, contents)| (name.clone(), exec::bytes(pool, fill(contents).as_bytes())))
        .collect();
    let server = case
        .server
        .iter()
        .map(|reply| {
            (
                exec::bytes(pool, fill(&reply.receives).as_bytes()),
                vec![exec::bytes(pool, fill(&reply.replies).as_bytes())],
            )
        })
        .collect();
    let made = made_connections(
        case.connections,
        case.server.iter().map(|reply| reply.connection),
    );
    let client = case
        .client
        .iter()
        .enumerate()
        .map(|(index, exchange)| {
            let bytes = exec::bytes(pool, fill(&exchange.sends).as_bytes());
            let held = case.clients_at_once && index + 1 < case.client.len() && !bytes.is_empty();
            let mut pieces = if held {
                vec![
                    bytes[..bytes.len() - 1].to_vec(),
                    bytes[bytes.len() - 1..].to_vec(),
                ]
            } else {
                vec![bytes]
            };
            pieces.retain(|piece| !piece.is_empty());
            world::net::Client {
                pieces,
                waits: !exchange.receives.is_empty(),
                held,
            }
        })
        .collect();
    let network = world::net::Network {
        server,
        made,
        client,
        listening: listens,
        at_once: case.clients_at_once,
        eager: true,
    };
    let supply = case.supply();
    let random = supply.random.as_ref().map(|bytes| exec::bytes(pool, bytes));
    let clock = supply
        .clock
        .iter()
        .map(|time| {
            (
                pool.bv(64, u128::from(time.seconds.cast_unsigned())),
                pool.bv(64, u128::from(time.nanoseconds)),
            )
        })
        .collect();
    let env: Vec<Vec<Term>> = case
        .env
        .iter()
        .map(|(name, value)| exec::bytes(pool, format!("{name}={value}").as_bytes()))
        .collect();
    image
        .begin(pool, &args, &env, stdin)
        .with_files(files)
        .with_folders(case.folders.clone())
        .with_supply(random, clock)
        .with_network(network)
        .with_stdout_closed(case.stdout_closed)
}

/// Runs the prover's model of the code on concrete inputs that the real program has passed, and
/// says where the model disagrees: then the model can't be trusted for a proof. A sample on
/// which the program breaks a memory rule, which a real run can pass without showing it, is the
/// verdict already: its inputs, with what broke.
///
/// # Errors
///
/// Describes the first disagreement, or what the model can't run.
pub fn agrees(
    machine: &Machine,
    cases: &[crate::testfile::Case],
    listens: bool,
) -> Result<Option<Verdict>, String> {
    agrees_until(
        machine,
        cases,
        listens,
        Instant::now() + Duration::from_secs(24 * 60 * 60),
    )
}

/// A concrete run's oracle that also keeps the proof's time: a run through code with no branch on
/// its inputs, such as a hash of them, asks it nothing else.
struct ConcreteUntil(Instant);

impl Oracle for ConcreteUntil {
    fn possible(&mut self, pool: &Pool, path: &[Term], condition: Term) -> Result<bool, String> {
        exec::Concrete.possible(pool, path, condition)
    }

    fn in_time(&self) -> Result<(), String> {
        if Instant::now() > self.0 {
            return Err(OUT_OF_TIME.into());
        }
        Ok(())
    }
}

/// [`agrees`], giving up at `deadline`: the model's runs of the samples are part of the proof's
/// time, and a hash of a long sample takes minutes of it.
///
/// # Errors
///
/// Describes the first disagreement, what the model can't run, or that the time is up.
pub fn agrees_until(
    machine: &Machine,
    cases: &[crate::testfile::Case],
    listens: bool,
    deadline: Instant,
) -> Result<Option<Verdict>, String> {
    let image = Image::new(machine).ok_or("the program has no _start")?;
    agrees_with(&image, cases, listens, deadline)
}

/// [`agrees_until`] for the WebAssembly program `built`, whose idea lets it reach what `policy`
/// allows: its model's runs of the samples against what the real program did on them.
///
/// # Errors
///
/// Describes the first disagreement, what the model can't run, or that the time is up.
pub fn agrees_wasm(
    built: &crate::wasm::Built,
    policy: crate::wasm::Policy,
    cases: &[crate::testfile::Case],
    listens: bool,
    deadline: Instant,
) -> Result<Option<Verdict>, String> {
    let module = wasm::Module::new(built, policy)?;
    agrees_with(&module, cases, listens, deadline)
}

/// [`agrees_until`]'s work, for either kind of program.
///
/// # Errors
///
/// Describes the first disagreement, what the model can't run, or that the time is up.
fn agrees_with<P: Program>(
    image: &P,
    cases: &[crate::testfile::Case],
    listens: bool,
    deadline: Instant,
) -> Result<Option<Verdict>, String> {
    // Smallest first: once one is too long for the model to follow, the larger ones are too.
    let mut cases: Vec<&crate::testfile::Case> = cases.iter().collect();
    cases.sort_by_key(|case| {
        case.stdin.len()
            + case.files.values().map(String::len).sum::<usize>()
            + case.args.iter().map(String::len).sum::<usize>()
    });
    for case in cases {
        let mut pool = Pool::default();
        let fill = sample_text;
        let start = sample_start(image, &mut pool, case, listens);
        let mut runs = exec::explore(image, start, &mut pool, &mut ConcreteUntil(deadline), 2)?;
        let run = runs.pop().ok_or("the model didn't run")?.world;
        if let Some(End::Unsafe(problem)) = &run.end {
            let mut values = case.inputs.clone();
            values.insert(UNSAFE.to_owned(), Value::Str(problem.clone()));
            return Ok(Some(Verdict::Counterexample(values)));
        }
        let text = |bytes: &[Term]| -> Option<Vec<u8>> {
            bytes
                .iter()
                .map(|byte| {
                    pool.constant(*byte)
                        .and_then(|value| u8::try_from(value).ok())
                })
                .collect()
        };
        let status = match &run.end {
            Some(End::Exit(status)) => pool.constant(*status),
            // A sample too long for the model to follow, such as a mebibyte sorted, says nothing
            // of it: the shorter ones the proof covers still check it.
            Some(End::Unsupported(reason)) if reason == exec::TOO_LONG => break,
            Some(End::Unsupported(reason)) => return Err(reason.clone()),
            _ => None,
        };
        // Output from the clock or random bytes is any value, which a real run can't be
        // compared with; only a rule such as any(1, 6) allows it, and the proof checks the rule.
        let now = crate::expect::now();
        let agrees_with = |bytes: Option<&[Term]>, expected: &str| match bytes {
            None => false,
            Some(bytes) => match text(bytes) {
                Some(text) => crate::expect::matches(expected, &text, now),
                None => crate::expect::has_holes(expected),
            },
        };
        let open = |bytes: Option<&[Term]>, expected: &str| {
            bytes.is_some_and(|bytes| text(bytes).is_none()) && !crate::expect::has_holes(expected)
        };
        if open(Some(&run.stdout), &case.stdout)
            || open(Some(&run.stderr), &case.stderr)
            || case
                .files_after
                .iter()
                .any(|(name, contents)| open(run.file(name), contents))
        {
            return Err("the output depends on the clock or random bytes".into());
        }
        let files_match = case
            .files_after
            .iter()
            .all(|(name, contents)| agrees_with(run.file(name), contents))
            && case
                .gone
                .iter()
                .all(|name| run.file(name).is_none() && !run.is_folder(name))
            && case.folders_after.iter().all(|name| run.is_folder(name));
        // The network as the test's peers saw it: what the server received, what the client
        // was answered, and a server still serving when it should be.
        let answered: Vec<_> = run
            .connections
            .iter()
            .filter(|c| !c.accepted && !c.ended)
            .collect();
        let taken: Vec<_> = run.connections.iter().filter(|c| c.accepted).collect();
        let talked = answered.len() == case.server.len()
            && taken.len() == case.client.len()
            && answered.iter().all(|connection| {
                case.server.get(connection.exchange).is_some_and(|reply| {
                    text(&connection.sent)
                        .is_some_and(|sent| sent == fill(&reply.receives).as_bytes())
                })
            })
            && taken
                .iter()
                .zip(&case.client)
                .all(|(connection, exchange)| {
                    agrees_with(Some(&connection.sent), &fill(&exchange.receives))
                });
        let serving = matches!(run.end, Some(End::Serving));
        let ended = if case.serves {
            serving
        } else {
            status == Some(u128::from(case.exit_code))
        };
        let same = agrees_with(Some(&run.stdout), &fill(&case.stdout))
            && agrees_with(Some(&run.stderr), &fill(&case.stderr))
            && ended
            && talked
            && files_match;
        if !same {
            return Err(format!(
                "the prover's model of the code disagreed with the real program on {}",
                crate::testfile::describe_inputs(&case.inputs)
            ));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod golden;
#[cfg(test)]
mod outputs;
#[cfg(test)]
mod tests;
