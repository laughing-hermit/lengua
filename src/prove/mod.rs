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
use crate::expr::{Generator, PatternPart, Piece, Template, Value};
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

/// The values of one variable a proof covers at once: a shape of whole numbers, or one value of
/// a `choice(…)`, whose text is fixed.
#[derive(Debug, Clone)]
enum Part {
    /// Whole numbers, or decimals' digits with this many places.
    Number(Shape, u8),
    Fixed(Value),
    /// Text of this many bytes, each one of the generator's characters.
    Text(usize, Vec<u8>),
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
        Generator::Choice(values) => Ok(values.iter().cloned().map(Part::Fixed).collect()),
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
}

/// What must hold for `bytes` to be exactly `template`'s text for these inputs. Literal text, a
/// placeholder whose variables all have fixed values, and a placeholder that's just a drawn text
/// are known bytes; at most one placeholder may compute a number or allow a range of them.
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
    let empty = Vec::new();
    // Neighboring bytes joined, so a computed segment stands between bytes or the ends.
    let (prefix, middle, suffix) = match segments.as_slice() {
        [] => (&empty, None, &empty),
        [Segment::Bytes(all)] => (all, None, &empty),
        [middle] => (&empty, Some(middle), &empty),
        [Segment::Bytes(before), middle] => (before, Some(middle), &empty),
        [middle, Segment::Bytes(after)] => (&empty, Some(middle), after),
        [Segment::Bytes(before), middle, Segment::Bytes(after)] => (before, Some(middle), after),
        _ => return Err("a test output with more than one computed value".into()),
    };
    let (expr, places) = match middle {
        None => return Ok(same_terms(pool, bytes, prefix)),
        Some(Segment::Number(expr, places)) => (*expr, *places),
        Some(Segment::Any(low, high)) => {
            let (low, high) = (bound(pool, low, known)?, bound(pool, high, known)?);
            if bytes.len() < prefix.len() + suffix.len() + 1 {
                return Ok(pool.boolean(false));
            }
            let end = bytes.len() - suffix.len();
            let starts = same_terms(pool, &bytes[..prefix.len()], prefix);
            let ends = same_terms(pool, &bytes[end..], suffix);
            let middle = number_in(pool, &bytes[prefix.len()..end], low, high);
            return Ok(pool.all(&[starts, ends, middle]));
        }
        Some(Segment::Bytes(_)) => return Err("bytes next to bytes in a test output".into()),
    };
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
    if bytes.len() < prefix.len() + suffix.len() + 1 {
        return Ok(pool.boolean(false));
    }
    let end = bytes.len() - suffix.len();
    let starts = same_terms(pool, &bytes[..prefix.len()], prefix);
    let ends = same_terms(pool, &bytes[end..], suffix);
    let middle = number_is(pool, &bytes[prefix.len()..end], number, places);
    Ok(pool.all(&[starts, ends, middle]))
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
    for (name, generator) in &block.vars {
        each.push((name.clone(), parts(name, generator)?));
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
    let found: Mutex<Option<BTreeMap<String, Value>>> = Mutex::new(None);
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
) -> Result<(usize, Option<String>), Stop> {
    let mut solver = Solver::start(budget.question_ms)
        .map_err(|error| format!("no SMT solver could start: {error}"))?;
    solver.until(budget.deadline);
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
    let mut start = State::start(image, &mut pool, &args, stdin).with_files(files);
    start.path = constraints;
    let began = Instant::now();
    let runs = exec::explore(image, start, &mut pool, &mut solver, budget.max_paths)?;
    let explored = began.elapsed();
    let count = runs.len();
    let questions = solver.questions;
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
        let files = case
            .files
            .iter()
            .map(|(name, contents)| (name.clone(), exec::bytes(&mut pool, contents.as_bytes())))
            .collect();
        let start = State::start(&image, &mut pool, &args, stdin).with_files(files);
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
