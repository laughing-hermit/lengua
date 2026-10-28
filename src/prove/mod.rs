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

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

use crate::checker::decode::Machine;
use crate::expr::{Generator, PatternPart, Piece, Template};
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
        /// Why one solver answered alone, when no second solver confirmed what it ruled out.
        alone: Option<String>,
    },
    /// Inputs where the program, as the prover reads it, disagrees with the formulas.
    Counterexample(BTreeMap<String, i128>),
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

impl Oracle for Solver {
    fn possible(&mut self, pool: &Pool, path: &[Term], condition: Term) -> Result<bool, String> {
        let mut conditions = path.to_vec();
        conditions.push(condition);
        let (answer, _) = self
            .check(pool, &conditions, &[])
            .map_err(|error| format!("the solver failed: {error}"))?;
        if self.out_of_time() {
            return Err("the proof ran out of time".into());
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

/// A variable's text and value for one shape, with what its digits must satisfy.
struct Input {
    text: Vec<Term>,
    number: Number,
    constraints: Vec<Term>,
}

fn input(pool: &mut Pool, name: &str, number: usize, shape: &Shape) -> Input {
    let (ten, forty_eight) = (pool.bv(WIDTH, 10), pool.bv(WIDTH, u128::from(b'0')));
    let mut text = Vec::new();
    let mut constraints = Vec::new();
    if shape.negative {
        text.push(pool.bv(8, u128::from(b'-')));
    }
    let mut magnitude = pool.bv(WIDTH, 0);
    for (index, (lowest, highest)) in shape.digits.iter().enumerate() {
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
        number: Number {
            term: value,
            low: shape.low,
            high: shape.high,
        },
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
) -> Term {
    let count = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    let (Some(top), Some(bottom)) = (
        10_u128.checked_pow(count),
        10_u128.checked_pow(count.saturating_sub(1)),
    ) else {
        return pool.boolean(false);
    };
    let bottom = if count == 1 { 0 } else { bottom };
    let (bottom, top) = (pool.bv(WIDTH, bottom), pool.bv(WIDTH, top));
    let at_least = pool.ule(bottom, magnitude);
    let below = pool.ult(magnitude, top);
    let mut checks = vec![at_least, below];
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
        None => magnitude,
    };
    let width = pool.width(base);
    let (ten, zero_char) = (pool.bv(width, 10), pool.bv(8, u128::from(b'0')));
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

/// What must hold for `bytes` to be exactly `number` in decimal.
fn number_is(pool: &mut Pool, bytes: &[Term], number: Number) -> Term {
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
        let digits = digits_are(pool, &bytes[1..], magnitude, number.term, true);
        cases.push(pool.all(&[negative, signed, digits]));
    }
    if !bytes.is_empty() && first != Some(u128::from(b'-')) {
        let not_negative = pool.bool_not(negative);
        let digits = digits_are(pool, bytes, magnitude, number.term, false);
        cases.push(pool.all(&[not_negative, digits]));
    }
    pool.any(&cases)
}

/// What must hold for `bytes` to be exactly `template`'s text for these inputs.
fn text_is(
    pool: &mut Pool,
    bytes: &[Term],
    template: &Template,
    numbers: &BTreeMap<String, Number>,
) -> Result<Term, String> {
    let pieces = template.pieces();
    let placeholders: Vec<usize> = pieces
        .iter()
        .enumerate()
        .filter(|(_, piece)| matches!(piece, Piece::Expr(_)))
        .map(|(index, _)| index)
        .collect();
    let literal = |range: &[Piece<'_>]| -> Vec<u8> {
        range
            .iter()
            .filter_map(|piece| match piece {
                Piece::Text(text) => Some(text.as_bytes().to_vec()),
                Piece::Expr(_) => None,
            })
            .flatten()
            .collect()
    };
    let same_bytes = |pool: &mut Pool, actual: &[Term], expected: &[u8]| -> Term {
        let checks: Vec<Term> = actual
            .iter()
            .zip(expected)
            .map(|(byte, expected)| {
                let expected = pool.bv(8, u128::from(*expected));
                pool.eq(*byte, expected)
            })
            .collect();
        pool.all(&checks)
    };
    match placeholders.as_slice() {
        [] => {
            let expected = literal(&pieces);
            if bytes.len() != expected.len() {
                return Ok(pool.boolean(false));
            }
            Ok(same_bytes(pool, bytes, &expected))
        }
        [index] => {
            let (prefix, suffix) = (literal(&pieces[..*index]), literal(&pieces[index + 1..]));
            let Piece::Expr(expr) = pieces[*index] else {
                return Err("a placeholder".into());
            };
            let number = match formula::translate(pool, expr, numbers)? {
                Formula::Number(number) => number,
                Formula::Truth(_) => return Err("a placeholder that prints true or false".into()),
            };
            if bytes.len() < prefix.len() + suffix.len() + 1 {
                return Ok(pool.boolean(false));
            }
            let end = bytes.len() - suffix.len();
            let starts = same_bytes(pool, &bytes[..prefix.len()], &prefix);
            let ends = same_bytes(pool, &bytes[end..], &suffix);
            let middle = number_is(pool, &bytes[prefix.len()..end], number);
            Ok(pool.all(&[starts, ends, middle]))
        }
        _ => Err("a test output with more than one computed value".into()),
    }
}

/// What must hold for a run that exited with `status` to pass the block.
fn passes(
    pool: &mut Pool,
    block: &Generated,
    numbers: &BTreeMap<String, Number>,
    run: &State,
    status: Term,
) -> Result<Term, String> {
    let stdout = text_is(pool, &run.stdout, &block.stdout, numbers)?;
    let stderr = text_is(pool, &run.stderr, &block.stderr, numbers)?;
    let exit = match &block.exit_code {
        ExitCode::Fixed(code) => {
            let code = pool.bv(8, u128::from(*code));
            pool.eq(status, code)
        }
        ExitCode::Formula(template) => {
            let [Piece::Expr(expr)] = template.pieces()[..] else {
                return Err("an exit code template with text around its formula".into());
            };
            let number = match formula::translate(pool, expr, numbers)? {
                Formula::Number(number) => number,
                Formula::Truth(_) => return Err("an exit code that's true or false".into()),
            };
            let (zero, top) = (pool.bv(WIDTH, 0), pool.bv(WIDTH, 255));
            let at_least = pool.sle(zero, number.term);
            let at_most = pool.sle(number.term, top);
            let wide = pool.zero_extend(status, WIDTH - 8);
            let same = pool.eq(wide, number.term);
            pool.all(&[at_least, at_most, same])
        }
    };
    Ok(pool.all(&[stdout, stderr, exit]))
}

/// Proves `block` about the program `machine` holds, within `budget`.
#[must_use]
pub fn prove(machine: &Machine, block: &Generated, budget: Budget) -> Verdict {
    attempt(machine, block, budget).unwrap_or_else(Verdict::NotProven)
}

fn attempt(machine: &Machine, block: &Generated, budget: Budget) -> Result<Verdict, String> {
    if !block.files.is_empty() || !block.files_after.is_empty() {
        return Err("tests with files".into());
    }
    let mut each = Vec::new();
    for (name, generator) in &block.vars {
        let Generator::Int { lo, hi } = generator else {
            return Err(format!("{name} isn't drawn by int(lo, hi)"));
        };
        each.push((name.clone(), shapes(*lo, *hi)));
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
    let paths = AtomicUsize::new(0);
    let alone: Mutex<Option<String>> = Mutex::new(None);
    let stop = AtomicBool::new(false);
    let found: Mutex<Option<BTreeMap<String, i128>>> = Mutex::new(None);
    let failure: Mutex<Option<String>> = Mutex::new(None);
    let workers = std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .min(total.max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while !stop.load(Ordering::Relaxed) {
                    let combination = next.fetch_add(1, Ordering::Relaxed);
                    if combination >= total {
                        break;
                    }
                    let result = if Instant::now() > budget.deadline {
                        Err(Stop::Unproven("the proof ran out of time".into()))
                    } else {
                        prove_shape(&image, block, &each, combination, budget)
                    };
                    match result {
                        Ok((count, reason)) => {
                            paths.fetch_add(count, Ordering::Relaxed);
                            if let (Some(reason), Ok(mut alone)) = (reason, alone.lock()) {
                                alone.get_or_insert(reason);
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
        return Err(reason);
    }
    Ok(Verdict::Proven {
        shapes: total,
        paths: paths.into_inner(),
        alone: alone.into_inner().ok().flatten(),
    })
}

/// Why proving one shape stopped short.
enum Stop {
    /// Inputs where the program disagrees with the formulas.
    Counterexample(BTreeMap<String, i128>),
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
    each: &[(String, Vec<Shape>)],
    combination: usize,
    budget: Budget,
) -> Result<(usize, Option<String>), Stop> {
    let mut solver = Solver::start(budget.question_ms)
        .map_err(|error| format!("no SMT solver could start: {error}"))?;
    solver.until(budget.deadline);
    let mut pool = Pool::default();
    let mut rest = combination;
    let mut inputs = BTreeMap::new();
    let mut numbers = BTreeMap::new();
    let mut constraints = Vec::new();
    for (name, shapes) in each {
        let number = rest % shapes.len();
        rest /= shapes.len();
        let input = input(&mut pool, name, number, &shapes[number]);
        constraints.extend(&input.constraints);
        numbers.insert(name.clone(), input.number);
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
    let mut start = State::start(image, &mut pool, &args, stdin);
    start.path = constraints;
    let began = Instant::now();
    let runs = exec::explore(image, start, &mut pool, &mut solver, budget.max_paths)?;
    let explored = began.elapsed();
    let count = runs.len();
    let questions = solver.questions;
    let trace = std::env::var_os("LENGUA_PROVE_TRACE").is_some();
    for run in runs {
        let holds = match &run.end {
            Some(End::Exit(status)) => passes(&mut pool, block, &numbers, &run, *status)?,
            Some(End::Signal(_)) => pool.boolean(false),
            Some(End::Unsupported(reason)) => return Err(Stop::Unproven(reason.clone())),
            None => return Err("a run that didn't end".into()),
        };
        let broken = pool.bool_not(holds);
        let mut conditions = run.path.clone();
        conditions.push(broken);
        let values: Vec<Term> = numbers.values().map(|number| number.term).collect();
        let (answer, model) = solver
            .check(&pool, &conditions, &values)
            .map_err(|error| format!("the solver failed: {error}"))?;
        match answer {
            Answer::Unsat => {}
            Answer::Sat => {
                let mut found = BTreeMap::new();
                for (name, number) in &numbers {
                    let value = model
                        .get(&number.term)
                        .copied()
                        .ok_or("a counterexample without values")?;
                    found.insert(name.clone(), i128::from_ne_bytes(value.to_ne_bytes()));
                }
                return Err(Stop::Counterexample(found));
            }
            Answer::Unknown => {
                return Err(format!("a path couldn't be settled: {}", solver.why_unknown).into());
            }
        }
    }
    if trace {
        eprintln!(
            "shape {combination}: {count} runs, {questions} questions exploring in {explored:?}, \
             {} checking, {} terms, {} overruled",
            solver.questions - questions,
            pool.len(),
            solver.overruled
        );
    }
    Ok((count, solver.alone.take()))
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
        let start = State::start(&image, &mut pool, &args, stdin);
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
        let same = text(&run.stdout).as_deref() == Some(case.stdout.as_bytes())
            && text(&run.stderr).as_deref() == Some(case.stderr.as_bytes())
            && status == Some(u128::from(case.exit_code));
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
