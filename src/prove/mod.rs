//! The proof step: for a generated test over whole-number ranges, proves that the program prints
//! exactly what the formulas say for every input in range, not only the sampled ones.
//!
//! It splits the inputs into shapes (each variable's sign and digit count), runs the machine
//! code on terms for each shape ([`exec`]), and asks an SMT solver ([`smt`]) whether any input
//! on any path could make the output differ from the formulas ([`formula`]). A printed number is
//! matched through its digits' common root, so "prints the formula's value" becomes one linear
//! question instead of twenty divisions.

pub mod exec;
pub mod formula;
pub mod smt;
pub mod term;

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

use crate::checker::decode::Machine;
use crate::expr::{Generator, HexPart, PatternPart, Piece, Template, Value};
use crate::testfile::{ExitCode, Generated};

use exec::{End, Image, Oracle, State};
use formula::{Formula, Number, WIDTH};
use smt::{Answer, Solver};
use term::{Node, Pool, Term};

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
    },
    /// Inputs where the program, as the prover reads it, disagrees with the formulas.
    Counterexample(BTreeMap<String, Value>),
    /// The proof couldn't be finished, and why.
    NotProven(String),
}

/// How much one proof may take.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// When to give up.
    pub deadline: Instant,
    /// The most paths per input shape.
    pub max_paths: usize,
    /// The most input shapes.
    pub max_shapes: usize,
    /// Each solver question's time limit, in milliseconds.
    pub question_ms: u64,
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
    /// Each witness's variable values, and every term value computed from them.
    witnesses: Vec<HashMap<Term, u128>>,
    rng: ChaCha8Rng,
}

impl<'a> Witnessed<'a> {
    fn new(solvers: &'a mut Solvers, deadline: Instant, seed: u64) -> Self {
        Self {
            solvers,
            deadline,
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
    if witness.len() > MAX_KNOWN {
        witness.retain(|term, _| {
            *term == Term::LOW
                || *term == Term::HIGH
                || matches!(pool.node(*term), Node::Var { .. } | Node::BoolVar(_))
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

impl Oracle for Witnessed<'_> {
    fn possible(&mut self, pool: &Pool, path: &[Term], condition: Term) -> Result<bool, String> {
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
    /// A number the program reads only as this many random bytes: one shape, a variable a byte.
    Bytes(usize),
    /// A number the program reads only as a clock reading, from the first to the second, with
    /// this many decimal places: one shape, since its digits are never read.
    Plain(i128, i128, u8),
}

/// The most lengths of text one variable may be proven over, one shape each.
const MAX_LENGTHS: usize = 4096;

/// Texts this long or shorter have each byte held to the generator's characters; longer ones
/// only to the range the characters span, which proves more than needed but nothing false.
const MAX_CHECKED_TEXT: usize = 4096;

/// The parts that cover every value `generator` draws.
fn parts(name: &str, generator: &Generator) -> Result<Vec<Part>, String> {
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
                return Err(format!("{name} is drawn from characters beyond ASCII"));
            }
            if max - min >= MAX_LENGTHS {
                return Err(format!(
                    "{name} is text of more than {MAX_LENGTHS} lengths, each a shape of its own"
                ));
            }
            let bytes: Vec<u8> = charset.iter().map(|character| *character as u8).collect();
            Ok((*min..=*max)
                .map(|length| Part::Text(length, bytes.clone()))
                .collect())
        }
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
            let choices: Vec<Term> = characters
                .iter()
                .map(|character| {
                    let character = pool.bv(8, u128::from(*character));
                    pool.eq(byte, character)
                })
                .collect();
            constraints.push(pool.any(&choices));
        }
        text.push(byte);
    }
    Input {
        text,
        number: None,
        constraints,
    }
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
            let check = match (negative, pool.node(root).clone()) {
                (true, Node::Neg(signed)) => {
                    let width = pool.width(signed);
                    let wide = pool.sign_extend(signed, WIDTH - width);
                    pool.eq(wide, value)
                }
                (false, _) => {
                    let width = pool.width(root);
                    let wide = pool.sign_extend(root, WIDTH - width);
                    pool.eq(wide, value)
                }
                (true, _) => {
                    let width = pool.width(root);
                    let wide = pool.zero_extend(root, WIDTH - width);
                    pool.eq(wide, magnitude)
                }
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

/// What must hold for `bytes` to be the last decimal digits of `base`, one byte each.
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
        let low = pool.extract(digit, 7, 0);
        let character = pool.add(low, zero_char);
        checks.push(pool.eq(*byte, character));
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
}

/// A run of a template once what's known in this shape is filled in: bytes, or a number's
/// formula, printed with this many decimal places (`places(x, 2)`), or none, or any whole number
/// from one formula's value to another's (`any(lo, hi)`).
enum Segment<'a> {
    Bytes(Vec<Term>),
    Number(&'a crate::expr::Expr, u8),
    Any(&'a crate::expr::Expr, &'a crate::expr::Expr),
    /// `if(condition, "text", "other text")`: one of two fixed texts, as a condition says.
    Either(&'a crate::expr::Expr, Vec<Term>, Vec<Term>),
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
            Piece::Expr(crate::expr::Expr::Call(crate::expr::Function::If, arguments))
                if arguments.len() == 3 && fixed_text(&arguments[1], known).is_some() =>
            {
                let texts = (
                    fixed_text(&arguments[1], known),
                    fixed_text(&arguments[2], known),
                );
                let (Some(then), Some(otherwise)) = texts else {
                    return Err("an if(…) that chooses between text and a number".into());
                };
                Segment::Either(
                    &arguments[0],
                    exec::bytes(pool, then.as_bytes()),
                    exec::bytes(pool, otherwise.as_bytes()),
                )
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
            Piece::Expr(expr) => Segment::Number(expr, 0),
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
            Segment::Either(condition, then, otherwise) => {
                let condition = match formula::translate(pool, condition, known.numbers)? {
                    Formula::Truth(condition) => condition,
                    Formula::Number(_) => return Err("an if(…) whose condition is a number".into()),
                };
                computed.push(Computed::Either(condition, then, otherwise));
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
                Computed::Exact(number, places) => number_is(pool, piece, *number, *places),
                Computed::Range(low, high) => number_in(pool, piece, *low, *high),
                Computed::Either(condition, then, otherwise) => {
                    let first = same_terms(pool, piece, then);
                    let first = pool.all(&[*condition, first]);
                    let not = pool.bool_not(*condition);
                    let second = same_terms(pool, piece, otherwise);
                    let second = pool.all(&[not, second]);
                    pool.any(&[first, second])
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
/// any whole number in a range, or one of two fixed texts as a condition says.
enum Computed {
    Exact(Number, u8),
    Range(Number, Number),
    Either(Term, Vec<Term>, Vec<Term>),
}

impl Computed {
    /// How many bytes it can take.
    fn lengths(&self) -> Vec<usize> {
        match self {
            Self::Exact(..) | Self::Range(..) => (1..=MAX_PRINTED).collect(),
            Self::Either(_, then, otherwise) => {
                let mut lengths = vec![then.len(), otherwise.len()];
                lengths.dedup();
                lengths
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
    let number = match formula::translate(pool, expr, known.numbers)? {
        Formula::Number(number) => number,
        Formula::Truth(_) => return Err("a placeholder that prints true or false".into()),
    };
    // places(x, k) prints a whole number with k places; a decimal itself prints with its own.
    let places = match (places, number.places) {
        (0, own) => own,
        (_, 0) => places,
        _ => return Err("places of a decimal; scale it to a whole number first".into()),
    };
    Ok(Computed::Exact(number, places))
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
    match formula::translate(pool, expr, known.numbers)? {
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
    run: &State,
    status: Term,
) -> Result<Term, String> {
    let (numbers, fixed) = (known.numbers, known.fixed);
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
            let known = crate::expr::variables_of(expr)
                .iter()
                .all(|name| fixed.contains_key(name));
            let number = if known {
                match crate::expr::eval(expr, fixed) {
                    Ok(Value::Int(code)) => Number::whole(pool.bv_signed(WIDTH, code), code, code),
                    _ => return Err("an exit code that isn't a whole number".into()),
                }
            } else {
                match formula::translate(pool, expr, numbers)? {
                    Formula::Number(number) => number,
                    Formula::Truth(_) => return Err("an exit code that's true or false".into()),
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
        if block.files_after.contains_key(name) {
            continue;
        }
        let after = run.file(name).unwrap_or_default();
        checks.push(same_terms(pool, before, after));
    }
    if run
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

/// Proves `block` about the program `machine` holds, within `budget`.
#[must_use]
pub fn prove(machine: &Machine, block: &Generated, budget: Budget) -> Verdict {
    attempt(machine, block, budget).unwrap_or_else(Verdict::NotProven)
}

fn attempt(machine: &Machine, block: &Generated, budget: Budget) -> Result<Verdict, String> {
    if block.uses_now() {
        return Err("it uses now, the second a run starts, which a proof can't fix".into());
    }
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
            _ => parts(name, generator)?,
        };
        each.push((name.clone(), these));
    }
    let total = each
        .iter()
        .try_fold(1_usize, |total, (_, shapes)| {
            total.checked_mul(shapes.len())
        })
        .filter(|total| *total <= budget.max_shapes)
        .ok_or_else(|| format!("more than {} input shapes", budget.max_shapes))?;
    let image = Image::new(machine).ok_or("the program has no _start")?;
    // The shapes are independent: every core takes the next one until all are done, a
    // counterexample turns up, or something can't be proven.
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let paths = AtomicUsize::new(0);
    let questions = AtomicUsize::new(0);
    let alone: Mutex<Option<String>> = Mutex::new(None);
    let stop = AtomicBool::new(false);
    let found: Mutex<Option<BTreeMap<String, Value>>> = Mutex::new(None);
    let failure: Mutex<Option<String>> = Mutex::new(None);
    let workers = std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .min(total.max(1));
    let began = Instant::now();
    // The combinations go in a scattered order, so the pace of the first ones shows the pace of
    // all of them, and a counterexample anywhere turns up early.
    let stride = stride(total);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                let mut solvers = Solvers::new(budget.question_ms);
                while !stop.load(Ordering::Relaxed) {
                    let turn = next.fetch_add(1, Ordering::Relaxed);
                    if turn >= total {
                        break;
                    }
                    let combination = (turn as u128 * stride as u128 % total as u128) as usize;
                    solvers.next();
                    let result = if Instant::now() > budget.deadline {
                        Err(Stop::Unproven(OUT_OF_TIME.into()))
                    } else {
                        prove_shape(&image, block, &each, combination, budget, &mut solvers)
                    };
                    match result {
                        Ok((count, asked, reason)) => {
                            paths.fetch_add(count, Ordering::Relaxed);
                            questions.fetch_add(asked, Ordering::Relaxed);
                            if let (Some(reason), Ok(mut alone)) = (reason, alone.lock()) {
                                alone.get_or_insert(reason);
                            }
                            // Once every worker has done two, and again at four times as many
                            // each time, the pace shows whether the rest fits the time left; if
                            // it can't, stop now and say so.
                            let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                            let checkpoint = finished >= 2 * workers
                                && (finished / (2 * workers)).is_power_of_two()
                                && finished.is_multiple_of(2 * workers)
                                && (finished / (2 * workers)).trailing_zeros().is_multiple_of(2);
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
                        Err(Stop::Unproven(reason)) => {
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
    if let Some(reason) = failure.into_inner().ok().flatten() {
        if reason == OUT_OF_TIME {
            return Err(format!(
                "ran out of time after {} of {total} input combinations",
                done.into_inner()
            ));
        }
        return Err(reason);
    }
    Ok(Verdict::Proven {
        shapes: total,
        paths: paths.into_inner(),
        questions: questions.into_inner(),
        alone: alone.into_inner().ok().flatten(),
    })
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
fn prove_shape(
    image: &Image,
    block: &Generated,
    each: &[(String, Vec<Part>)],
    combination: usize,
    budget: Budget,
    solvers: &mut Solvers,
) -> Result<(usize, usize, Option<String>), Stop> {
    let mut pool = Pool::default();
    let mut rest = combination;
    let mut inputs = BTreeMap::new();
    let mut numbers = BTreeMap::new();
    let mut constraints = Vec::new();
    // The fixed values chosen, which a counterexample reports with the numbers found, and the
    // texts drawn, byte by byte.
    let mut chosen = BTreeMap::new();
    let mut texts: BTreeMap<String, Vec<Term>> = BTreeMap::new();
    for (name, parts) in each {
        let number = rest % parts.len();
        rest /= parts.len();
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
            Part::Bytes(count) => bytes_input(&mut pool, name, *count),
            Part::Plain(low, high, places) => plain_input(&mut pool, name, *low, *high, *places),
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
    let stdin = pattern_bytes(&mut pool, &block.stdin, &inputs)?;
    // The model has one read return everything that's there, which a pipe guarantees only for
    // what one write of at most 4096 bytes put in it.
    if stdin.len() > 4096 {
        return Err("standard input over 4096 bytes".into());
    }
    let mut files = BTreeMap::new();
    for (name, template) in &block.files {
        files.insert(name.clone(), pattern_bytes(&mut pool, template, &inputs)?);
    }
    let random = random_stream(&mut pool, block, &inputs)?;
    let clock = clock_readings(&mut pool, block, &inputs)?;
    let mut start = State::start(image, &mut pool, &args, stdin)
        .with_files(files)
        .with_supply(random, clock);
    start.path = constraints;
    let began = Instant::now();
    let questions_before = solvers.solver.as_ref().map_or(0, |solver| solver.questions);
    let runs = {
        let mut oracle = Witnessed::new(solvers, budget.deadline, combination as u64);
        exec::explore(image, start, &mut pool, &mut oracle, budget.max_paths)?
    };
    let explored = began.elapsed();
    let count = runs.len();
    let asked = |solvers: &Solvers| solvers.solver.as_ref().map_or(0, |solver| solver.questions);
    let questions = asked(solvers);
    let trace = std::env::var_os("LENGUA_PROVE_TRACE").is_some();
    for run in runs {
        let holds = match &run.end {
            Some(End::Exit(status)) => {
                let known = Known {
                    numbers: &numbers,
                    fixed: &chosen,
                    texts: &texts,
                };
                passes(&mut pool, block, &known, &run, *status)?
            }
            Some(End::Signal(_)) => pool.boolean(false),
            Some(End::Unsupported(reason)) => return Err(Stop::Unproven(reason.clone())),
            None => return Err("a run that didn't end".into()),
        };
        let broken = pool.bool_not(holds);
        // Output whose terms are the formulas' own can't break: nothing to ask. Nor can output
        // that breaks only where the run's own branch says it doesn't.
        let holding = pool.bool_not(broken);
        if pool.truth(broken) == Some(false)
            || run
                .path
                .iter()
                .any(|condition| pool.equivalent(*condition, holding))
        {
            continue;
        }
        if std::env::var_os("LENGUA_PROVE_EXPLAIN").is_some() {
            eprintln!("combination {combination} asks: {}", pool.sketch(broken, 9));
        }
        let mut conditions = run.path.clone();
        conditions.push(broken);
        // A counterexample shows each number, and each text short enough to show.
        let shown: Vec<(&String, &Vec<Term>)> = texts
            .iter()
            .filter(|(_, bytes)| bytes.len() <= MAX_CHECKED_TEXT)
            .collect();
        let values: Vec<Term> = numbers
            .values()
            .map(|number| number.term)
            .chain(shown.iter().flat_map(|(_, bytes)| bytes.iter().copied()))
            .collect();
        let solver = solvers.get(budget.deadline)?;
        let (answer, model) = solver
            .check(&pool, &conditions, &values)
            .map_err(|error| format!("the solver failed: {error}"))?;
        match answer {
            Answer::Unsat => {}
            Answer::Sat => {
                if shown.len() < texts.len() {
                    return Err("a counterexample in a text too long to show".into());
                }
                let mut found = chosen.clone();
                for (name, bytes) in &shown {
                    let text: Vec<u8> = bytes
                        .iter()
                        .map(|byte| model.get(byte).map_or(b'?', |value| *value as u8))
                        .collect();
                    found.insert(
                        (*name).clone(),
                        Value::Str(String::from_utf8_lossy(&text).into_owned()),
                    );
                }
                for (name, number) in &numbers {
                    let value = model
                        .get(&number.term)
                        .copied()
                        .ok_or("a counterexample without values")?;
                    found.insert(
                        name.clone(),
                        Value::Int(i128::from_ne_bytes(value.to_ne_bytes())),
                    );
                }
                return Err(Stop::Counterexample(found));
            }
            Answer::Unknown => {
                if solver.out_of_time() {
                    return Err(OUT_OF_TIME.into());
                }
                return Err(format!("a path couldn't be settled: {}", solver.why_unknown).into());
            }
        }
    }
    if trace {
        eprintln!(
            "shape {combination}: {count} runs, {questions} questions exploring in {explored:?}, \
             {} checking, {} terms",
            asked(solvers) - questions,
            pool.len(),
        );
    }
    let alone = solvers
        .solver
        .as_ref()
        .filter(|_| solvers.used)
        .and_then(|solver| solver.alone.clone());
    Ok((count, asked(solvers) - questions_before, alone))
}

/// Runs the prover's model of the code on concrete inputs that the real program has passed, and
/// says where the model disagrees: then the model can't be trusted for a proof.
///
/// # Errors
///
/// Describes the first disagreement, or what the model can't run.
pub fn agrees(machine: &Machine, cases: &[crate::testfile::Case]) -> Result<(), String> {
    let image = Image::new(machine).ok_or("the program has no _start")?;
    for case in cases {
        let mut pool = Pool::default();
        let args: Vec<Vec<Term>> = case
            .args
            .iter()
            .map(|arg| exec::bytes(&mut pool, arg.as_bytes()))
            .collect();
        let stdin = exec::bytes(&mut pool, case.stdin.as_bytes());
        let files = case
            .files
            .iter()
            .map(|(name, contents)| (name.clone(), exec::bytes(&mut pool, contents.as_bytes())))
            .collect();
        let supply = case.supply();
        let random = supply
            .random
            .as_ref()
            .map(|bytes| exec::bytes(&mut pool, bytes));
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
        let start = State::start(&image, &mut pool, &args, stdin)
            .with_files(files)
            .with_supply(random, clock);
        let mut runs = exec::explore(&image, start, &mut pool, &mut exec::Concrete, 2)?;
        let run = runs.pop().ok_or("the model didn't run")?;
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
            .all(|(name, contents)| agrees_with(run.file(name), contents));
        let same = agrees_with(Some(&run.stdout), &case.stdout)
            && agrees_with(Some(&run.stderr), &case.stderr)
            && status == Some(u128::from(case.exit_code))
            && files_match;
        if !same {
            return Err(format!(
                "the prover's model of the code disagreed with the real program on {}",
                crate::testfile::describe_inputs(&case.inputs)
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod outputs;
#[cfg(test)]
mod tests;
