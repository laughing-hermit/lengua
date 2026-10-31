//! The tests' formulas as terms: whole numbers with the range of values each part can take, so a
//! formula that could overflow or divide by zero is caught before it's trusted; text byte by
//! byte, with the text functions written out over each byte; and decimals as the test language
//! computes them, in doubles or singles, as the same float terms the CPU's instructions make, each
//! with the bounds that show it stays finite.

use std::collections::BTreeMap;

use super::term::{CASTAGNOLI, FloatOp, FloatTest, Pool, Rounding, Term, ZLIB, float};
use crate::expr::{BinaryOp, Expr, Function, Value};

/// The width formulas are computed in: wide enough for any product of two 64-bit numbers.
pub const WIDTH: u32 = 128;

/// A whole number, or a decimal as its digits: its term and the smallest and largest values it
/// can take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Number {
    /// The value, as a signed 128-bit term: a decimal's digits without its point.
    pub term: Term,
    /// The smallest it can be.
    pub low: i128,
    /// The largest it can be.
    pub high: i128,
    /// The decimal places the digits have: 0 for a whole number. Formulas compute on whole
    /// numbers, so a decimal is used through `scaled(x, places)`.
    pub places: u8,
}

impl Number {
    /// A whole number from `low` to `high`.
    #[must_use]
    pub const fn whole(term: Term, low: i128, high: i128) -> Self {
        Self {
            term,
            low,
            high,
            places: 0,
        }
    }
}

/// A translated formula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Formula {
    /// A whole number.
    Number(Number),
    /// True or false.
    Truth(Term),
    /// Text, one 8-bit term a byte.
    Text(Vec<Term>),
    /// A float: a double, or a single where the term is 32 bits wide.
    Float(Term),
    /// One of several texts, each where its condition holds, as `sort` gives for each way a
    /// short text's newlines can fall and its lines compare.
    Texts(Vec<(Term, Vec<Term>)>),
}

fn overflow() -> String {
    "the formula could leave 128-bit whole numbers".into()
}

/// The whole number `formula` is: a decimal must go through `scaled(x, places)` first.
fn number(formula: Formula) -> Result<Number, String> {
    match formula {
        Formula::Number(number) if number.places == 0 => Ok(number),
        Formula::Number(_) => {
            Err("a formula computes on a decimal without scaled(x, places)".into())
        }
        Formula::Truth(_) => Err("a formula uses true/false where a number goes".into()),
        Formula::Text(_) | Formula::Texts(_) => {
            Err("a formula uses text where a number goes".into())
        }
        Formula::Float(_) => {
            Err("a formula uses a decimal where a whole number goes, without round(x)".into())
        }
    }
}

fn truth(formula: Formula) -> Result<Term, String> {
    match formula {
        Formula::Truth(term) => Ok(term),
        Formula::Number(_) | Formula::Text(_) | Formula::Float(_) | Formula::Texts(_) => {
            Err("a formula uses a value where true/false goes".into())
        }
    }
}

fn text(formula: Formula) -> Result<Vec<Term>, String> {
    match formula {
        Formula::Text(bytes) => Ok(bytes),
        Formula::Texts(_) => Err(
            "sorted or deduplicated text inside another formula, which the proof doesn't follow"
                .into(),
        ),
        Formula::Number(_) | Formula::Truth(_) | Formula::Float(_) => {
            Err("a formula uses a number or true/false where text goes".into())
        }
    }
}

/// The largest whole number a double holds exactly, 2^53, and a single, 2^24.
const EXACT_DOUBLE: i128 = 1 << 53;
const EXACT_SINGLE: i128 = 1 << 24;

/// The most places a decimal converts with exactly: 10^15 is a double's, 10^10 a single's.
const DOUBLE_PLACES: u8 = 15;
const SINGLE_PLACES: u8 = 10;

/// Whether `formula` is a decimal or a float, which compute as floats.
fn floating(formula: &Formula) -> bool {
    matches!(formula, Formula::Float(_))
        || matches!(formula, Formula::Number(number) if number.places > 0)
}

/// Whether `formula` is a single.
fn single(pool: &Pool, formula: &Formula) -> bool {
    matches!(formula, Formula::Float(term) if pool.width(*term) == 32)
}

/// Ten to the power `places`, as a float of `width` bits: exact for the places a formula takes.
fn power_of_ten(pool: &mut Pool, places: u8, width: u32) -> Term {
    let value = (0..places).fold(1.0_f64, |power, _| power * 10.0);
    let bits = if width == 32 {
        #[allow(clippy::cast_possible_truncation)] // Exact: 10^10 at most.
        u128::from((value as f32).to_bits())
    } else {
        u128::from(value.to_bits())
    };
    pool.bv(width, bits)
}

/// A whole number as the nearest float of `width` bits, as `cvtsi2sd` converts it.
fn whole_float(pool: &mut Pool, number: Number, width: u32) -> Result<Term, String> {
    if number.low < i128::from(i64::MIN) || number.high > i128::from(i64::MAX) {
        return Err("a whole number outside 64 bits converted to a float".into());
    }
    let low = pool.extract(number.term, 63, 0);
    Ok(pool.float_from_int(low, width))
}

/// A decimal's digits, its places of them after the point, as a float of `width` bits: one
/// division of two exact numbers, as the test language and a program compute it. The same
/// value written with more places is the same float, since both divisions round one number.
fn decimal_float(pool: &mut Pool, number: Number, width: u32) -> Result<Term, String> {
    let (exact, places) = if width == 32 {
        (EXACT_SINGLE, SINGLE_PLACES)
    } else {
        (EXACT_DOUBLE, DOUBLE_PLACES)
    };
    if number.low < -exact || number.high > exact || number.places > places {
        return Err(format!(
            "a decimal with more digits than a {} holds exactly",
            if width == 32 { "single" } else { "double" }
        ));
    }
    let digits = whole_float(pool, number, width)?;
    let scale = power_of_ten(pool, number.places, width);
    Ok(pool.float(FloatOp::Div, digits, scale))
}

/// `formula` as a float of `width` bits for arithmetic: a float of that width, or a decimal;
/// whole numbers and the other precision don't mix in, as in the test language.
fn as_float(pool: &mut Pool, formula: Formula, width: u32) -> Result<Term, String> {
    match formula {
        Formula::Float(term) if pool.width(term) == width => Ok(term),
        Formula::Float(_) => Err("a formula mixes a single and a double".into()),
        Formula::Number(number) if number.places > 0 => decimal_float(pool, number, width),
        Formula::Number(_) => Err("a formula mixes a whole number and a decimal".into()),
        Formula::Truth(_) | Formula::Text(_) | Formula::Texts(_) => {
            Err("a formula uses true/false or text where a decimal goes".into())
        }
    }
}

/// `formula` widened to a double, a single exactly, as rounding and printing take it.
fn widened(pool: &mut Pool, formula: Formula) -> Result<Term, String> {
    match formula {
        Formula::Float(term) if pool.width(term) == 32 => Ok(pool.float_convert(term, 64)),
        other => as_float(pool, other, 64),
    }
}

/// A float result, when its bounds show it's always a finite number, as the test language
/// requires of every one.
fn finite(pool: &Pool, term: Term) -> Result<Formula, String> {
    match pool.float_bounds(term) {
        Some((low, high)) if low.is_finite() && high.is_finite() => Ok(Formula::Float(term)),
        _ => Err("the formula could give a number that isn't finite, or divide by zero".into()),
    }
}

/// `a op b` where either is a decimal or a float: arithmetic in doubles, or singles where one is
/// a single; comparisons of exact decimals exactly, and of floats as the CPU compares.
fn float_binary(pool: &mut Pool, op: BinaryOp, a: Formula, b: Formula) -> Result<Formula, String> {
    let singles = single(pool, &a) || single(pool, &b);
    let float_op = match op {
        BinaryOp::Add => Some(FloatOp::Add),
        BinaryOp::Sub => Some(FloatOp::Sub),
        BinaryOp::Mul => Some(FloatOp::Mul),
        BinaryOp::Div => Some(FloatOp::Div),
        _ => None,
    };
    if let Some(float_op) = float_op {
        let width = if singles { 32 } else { 64 };
        let (x, y) = (as_float(pool, a, width)?, as_float(pool, b, width)?);
        let result = pool.float(float_op, x, y);
        return finite(pool, result);
    }
    if op == BinaryOp::Rem {
        return Err("% works on whole numbers only".into());
    }
    if !matches!(
        op,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
    ) {
        return Err("and/or of numbers".into());
    }
    let (x, y) = match (a, b) {
        (Formula::Number(a), Formula::Number(b)) => return Ok(exact_compare(pool, op, a, b)),
        (a, b) => {
            // Singles compare as singles, with a decimal as its nearest single; next to a
            // whole number or a double, as doubles.
            let whole = |formula: &Formula| matches!(formula, Formula::Number(number) if number.places == 0);
            let width = if singles && !whole(&a) && !whole(&b) {
                32
            } else {
                64
            };
            (compared(pool, a, width)?, compared(pool, b, width)?)
        }
    };
    let less = pool.float_test(FloatTest::Less, x, y);
    let greater = pool.float_test(FloatTest::Less, y, x);
    let equal = pool.float_test(FloatTest::Equal, x, y);
    Ok(Formula::Truth(match op {
        BinaryOp::Eq => equal,
        BinaryOp::Ne => pool.bool_not(equal),
        BinaryOp::Lt => less,
        BinaryOp::Gt => greater,
        BinaryOp::Le => pool.bool_or(less, equal),
        _ => pool.bool_or(greater, equal),
    }))
}

/// A side of a float comparison as a float of `width` bits: a whole number rounded to the
/// nearest, as the test language compares it.
fn compared(pool: &mut Pool, formula: Formula, width: u32) -> Result<Term, String> {
    match formula {
        Formula::Number(number) if number.places == 0 => whole_float(pool, number, width),
        Formula::Float(term) if width == 64 && pool.width(term) == 32 => {
            Ok(pool.float_convert(term, 64))
        }
        other => as_float(pool, other, width),
    }
}

/// Two exact numbers, whole or decimal, compared exactly: each scaled to the places of the
/// one with more.
fn exact_compare(pool: &mut Pool, op: BinaryOp, a: Number, b: Number) -> Formula {
    let places = a.places.max(b.places);
    let mut scaled = |number: Number| {
        let factor = 10_i128.pow(u32::from(places - number.places));
        let factor = pool.bv_signed(WIDTH, factor);
        pool.mul(number.term, factor)
    };
    let (x, y) = (scaled(a), scaled(b));
    Formula::Truth(match op {
        BinaryOp::Eq => pool.eq(x, y),
        BinaryOp::Ne => {
            let same = pool.eq(x, y);
            pool.bool_not(same)
        }
        BinaryOp::Lt => pool.slt(x, y),
        BinaryOp::Le => pool.sle(x, y),
        BinaryOp::Gt => pool.slt(y, x),
        _ => pool.sle(y, x),
    })
}

/// A float's sign bit, at its width.
const fn sign_bit(width: u32) -> u128 {
    1 << (width - 1)
}

/// Constant bytes, such as a literal `part` of `count`.
fn constant_bytes(pool: &Pool, bytes: &[Term]) -> Option<Vec<u8>> {
    bytes
        .iter()
        .map(|byte| {
            pool.constant(*byte)
                .and_then(|value| u8::try_from(value).ok())
        })
        .collect()
}

/// Whether `byte` is one of the six bytes `words` splits on: space, tab, newline, vertical tab,
/// form feed and carriage return.
fn space(pool: &mut Pool, byte: Term) -> Term {
    let nine = pool.bv(8, 9);
    let from_tab = pool.sub(byte, nine);
    let five = pool.bv(8, 5);
    let control = pool.ult(from_tab, five);
    let blank = pool.bv(8, u128::from(b' '));
    let is_blank = pool.eq(byte, blank);
    pool.bool_or(control, is_blank)
}

/// The smallest and largest of `values`.
fn span(values: &[Option<i128>]) -> Result<(i128, i128), String> {
    let values: Vec<i128> = values
        .iter()
        .map(|value| value.ok_or_else(overflow))
        .collect::<Result<_, _>>()?;
    let low = values.iter().copied().min().ok_or_else(overflow)?;
    let high = values.iter().copied().max().ok_or_else(overflow)?;
    Ok((low, high))
}

/// Translates `expr`, whose variables are `vars`.
///
/// # Errors
///
/// Says why the formula can't be proven about: text, decimals, functions on text, or a value
/// that could overflow or divide by zero.
pub fn translate(
    pool: &mut Pool,
    expr: &Expr,
    vars: &BTreeMap<String, Number>,
    texts: &BTreeMap<String, Vec<Term>>,
) -> Result<Formula, String> {
    match expr {
        Expr::Literal(Value::Int(value)) => Ok(Formula::Number(Number::whole(
            pool.bv_signed(WIDTH, *value),
            *value,
            *value,
        ))),
        Expr::Literal(Value::Bool(value)) => Ok(Formula::Truth(pool.boolean(*value))),
        Expr::Literal(Value::Dec { m, p }) => Ok(Formula::Number(Number {
            term: pool.bv_signed(WIDTH, *m),
            low: *m,
            high: *m,
            places: *p,
        })),
        Expr::Literal(Value::Float(bits)) => Ok(Formula::Float(
            pool.bv(64, u128::from(bits.value().to_bits())),
        )),
        Expr::Literal(Value::Single(bits)) => Ok(Formula::Float(
            pool.bv(32, u128::from(bits.value().to_bits())),
        )),
        Expr::Literal(Value::Str(value)) => Ok(Formula::Text(
            value
                .bytes()
                .map(|byte| pool.bv(8, u128::from(byte)))
                .collect(),
        )),
        Expr::Var(name) => match (vars.get(name), texts.get(name)) {
            (Some(number), _) => Ok(Formula::Number(*number)),
            (None, Some(bytes)) => Ok(Formula::Text(bytes.clone())),
            (None, None) => Err(format!(
                "the variable {name}, which isn't a whole number or a drawn text"
            )),
        },
        Expr::Neg(inner) => {
            let inner = match translate(pool, inner, vars, texts)? {
                // A float's sign flipped, as `-x` is exactly.
                Formula::Float(term) => {
                    let sign = pool.bv(pool.width(term), sign_bit(pool.width(term)));
                    let flipped = pool.xor(term, sign);
                    return finite(pool, flipped);
                }
                // A decimal negates exactly, keeping its places.
                Formula::Number(number) => number,
                other => number(other)?,
            };
            let (low, high) = span(&[inner.high.checked_neg(), inner.low.checked_neg()])?;
            Ok(Formula::Number(Number {
                term: pool.neg(inner.term),
                low,
                high,
                places: inner.places,
            }))
        }
        Expr::Not(inner) => {
            let inner = truth(translate(pool, inner, vars, texts)?)?;
            Ok(Formula::Truth(pool.bool_not(inner)))
        }
        Expr::Binary(op, left, right) => binary(pool, *op, left, right, vars, texts),
        Expr::Call(function, arguments) => call(pool, *function, arguments, vars, texts),
    }
}

fn binary(
    pool: &mut Pool,
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    vars: &BTreeMap<String, Number>,
    texts: &BTreeMap<String, Vec<Term>>,
) -> Result<Formula, String> {
    let (a, b) = (
        translate(pool, left, vars, texts)?,
        translate(pool, right, vars, texts)?,
    );
    // Text joins with +, and compares with == and != byte by byte.
    if let (Formula::Text(first), Formula::Text(second)) = (&a, &b) {
        return match op {
            BinaryOp::Add => Ok(Formula::Text([first.clone(), second.clone()].concat())),
            BinaryOp::Eq | BinaryOp::Ne => {
                let same = if first.len() == second.len() {
                    let checks: Vec<Term> = first
                        .iter()
                        .zip(second)
                        .map(|(x, y)| pool.eq(*x, *y))
                        .collect();
                    pool.all(&checks)
                } else {
                    pool.boolean(false)
                };
                Ok(Formula::Truth(if op == BinaryOp::Eq {
                    same
                } else {
                    pool.bool_not(same)
                }))
            }
            _ => Err("arithmetic on text".into()),
        };
    }
    let numeric = |formula: &Formula| matches!(formula, Formula::Number(_) | Formula::Float(_));
    if (floating(&a) || floating(&b)) && numeric(&a) && numeric(&b) {
        return float_binary(pool, op, a, b);
    }
    match op {
        BinaryOp::And | BinaryOp::Or => {
            let (a, b) = (truth(a)?, truth(b)?);
            Ok(Formula::Truth(if op == BinaryOp::And {
                pool.bool_and(a, b)
            } else {
                pool.bool_or(a, b)
            }))
        }
        BinaryOp::Eq | BinaryOp::Ne if matches!(a, Formula::Truth(_)) => {
            let (a, b) = (truth(a)?, truth(b)?);
            let same = pool.eq(a, b);
            Ok(Formula::Truth(if op == BinaryOp::Eq {
                same
            } else {
                pool.bool_not(same)
            }))
        }
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            let (a, b) = (number(a)?.term, number(b)?.term);
            Ok(Formula::Truth(match op {
                BinaryOp::Eq => pool.eq(a, b),
                BinaryOp::Ne => {
                    let same = pool.eq(a, b);
                    pool.bool_not(same)
                }
                BinaryOp::Lt => pool.slt(a, b),
                BinaryOp::Le => pool.sle(a, b),
                BinaryOp::Gt => pool.slt(b, a),
                _ => pool.sle(b, a),
            }))
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
            let (a, b) = (number(a)?, number(b)?);
            let corners = |f: fn(i128, i128) -> Option<i128>| {
                [
                    f(a.low, b.low),
                    f(a.low, b.high),
                    f(a.high, b.low),
                    f(a.high, b.high),
                ]
            };
            if matches!(op, BinaryOp::Div | BinaryOp::Rem) && b.low <= 0 && b.high >= 0 {
                return Err("the formula may divide by zero".into());
            }
            let (term, (low, high)) = match op {
                BinaryOp::Add => (pool.add(a.term, b.term), span(&corners(i128::checked_add))?),
                BinaryOp::Sub => (pool.sub(a.term, b.term), span(&corners(i128::checked_sub))?),
                BinaryOp::Mul => (pool.mul(a.term, b.term), span(&corners(i128::checked_mul))?),
                // Toward zero, like idiv: the extremes are at the corners when b keeps one sign.
                BinaryOp::Div => (
                    pool.sdiv(a.term, b.term),
                    span(&corners(i128::checked_div))?,
                ),
                _ => {
                    let largest = b.low.unsigned_abs().max(b.high.unsigned_abs()) - 1;
                    let largest = i128::try_from(largest).map_err(|_| overflow())?;
                    (
                        pool.srem(a.term, b.term),
                        (
                            if a.low < 0 { -largest } else { 0 },
                            if a.high > 0 { largest } else { 0 },
                        ),
                    )
                }
            };
            Ok(Formula::Number(Number::whole(term, low, high)))
        }
    }
}

fn call(
    pool: &mut Pool,
    function: Function,
    arguments: &[Expr],
    vars: &BTreeMap<String, Number>,
    texts: &BTreeMap<String, Vec<Term>>,
) -> Result<Formula, String> {
    let mut values = Vec::new();
    for argument in arguments {
        values.push(translate(pool, argument, vars, texts)?);
    }
    let mut values = values.into_iter();
    let mut next = || values.next().ok_or("a function without its arguments");
    match function {
        Function::Len => {
            let length = text(next()?)?.len();
            let length = i128::try_from(length).map_err(|_| overflow())?;
            Ok(Formula::Number(Number::whole(
                pool.bv_signed(WIDTH, length),
                length,
                length,
            )))
        }
        Function::Count => {
            let bytes = text(next()?)?;
            let part = text(next()?)?;
            let part =
                constant_bytes(pool, &part).ok_or("count of a part that isn't fixed text")?;
            count(pool, &bytes, &part)
        }
        Function::Words => {
            let bytes = text(next()?)?;
            let spaces: Vec<Term> = bytes.iter().map(|byte| space(pool, *byte)).collect();
            let mut starts = Vec::with_capacity(bytes.len());
            for index in 0..bytes.len() {
                let word = pool.bool_not(spaces[index]);
                starts.push(if index == 0 {
                    word
                } else {
                    pool.bool_and(word, spaces[index - 1])
                });
            }
            let high = i128::try_from(bytes.len().div_ceil(2)).map_err(|_| overflow())?;
            Ok(Formula::Number(Number::whole(
                pool.count(WIDTH, &starts),
                0,
                high,
            )))
        }
        Function::Upper | Function::Lower => {
            let bytes = text(next()?)?;
            let (from, to) = if function == Function::Upper {
                (b'a', b'A')
            } else {
                (b'A', b'a')
            };
            let mapped: Vec<(char, char)> = (0..26)
                .map(|offset| (char::from(from + offset), char::from(to + offset)))
                .collect();
            Ok(Formula::Text(translate_bytes(pool, &bytes, &mapped)))
        }
        Function::Tr => {
            let bytes = text(next()?)?;
            let sets = (text(next()?)?, text(next()?)?);
            let (Some(from), Some(to)) =
                (constant_bytes(pool, &sets.0), constant_bytes(pool, &sets.1))
            else {
                return Err("tr with sets that aren't fixed text".into());
            };
            let from = crate::expr::tr_set(&String::from_utf8_lossy(&from));
            let to = crate::expr::tr_set(&String::from_utf8_lossy(&to));
            if from.len() != to.len() {
                return Err("tr with sets of different lengths".into());
            }
            // Later pairs win, as a map built from them in order does.
            let mut map: BTreeMap<char, char> = BTreeMap::new();
            for (source, target) in from.into_iter().zip(to) {
                map.insert(source, target);
            }
            let pairs: Vec<(char, char)> = map.into_iter().collect();
            if pairs
                .iter()
                .all(|(source, target)| source.is_ascii() && target.is_ascii())
            {
                return Ok(Formula::Text(translate_bytes(pool, &bytes, &pairs)));
            }
            Ok(Formula::Text(translate_characters(pool, &bytes, &pairs)?))
        }
        Function::Reverse => {
            // By characters, as the test language reverses.
            let bytes = text(next()?)?;
            let mut characters = characters(pool, &bytes)
                .ok_or("reverse of text whose characters can't be told apart")?;
            characters.reverse();
            Ok(Formula::Text(characters.concat()))
        }
        Function::Abs => {
            let a = match next()? {
                // A float's sign cleared.
                Formula::Float(term) => {
                    let magnitude = pool.bv(pool.width(term), sign_bit(pool.width(term)) - 1);
                    let cleared = pool.and(term, magnitude);
                    return finite(pool, cleared);
                }
                other => number(other)?,
            };
            let zero = pool.bv(WIDTH, 0);
            let negative = pool.slt(a.term, zero);
            let negated = pool.neg(a.term);
            let low = if a.low <= 0 && a.high >= 0 {
                0
            } else {
                a.low
                    .checked_abs()
                    .ok_or_else(overflow)?
                    .min(a.high.checked_abs().ok_or_else(overflow)?)
            };
            let high = a
                .low
                .checked_abs()
                .ok_or_else(overflow)?
                .max(a.high.checked_abs().ok_or_else(overflow)?);
            Ok(Formula::Number(Number::whole(
                pool.ite(negative, negated, a.term),
                low,
                high,
            )))
        }
        Function::Min | Function::Max => {
            let mut best = number(next()?)?;
            let rest: Vec<Formula> = std::iter::from_fn(|| next().ok()).collect();
            for value in rest {
                let next = number(value)?;
                let better = if function == Function::Min {
                    pool.slt(next.term, best.term)
                } else {
                    pool.slt(best.term, next.term)
                };
                best = Number::whole(
                    pool.ite(better, next.term, best.term),
                    if function == Function::Min {
                        best.low.min(next.low)
                    } else {
                        best.low.max(next.low)
                    },
                    if function == Function::Min {
                        best.high.min(next.high)
                    } else {
                        best.high.max(next.high)
                    },
                );
            }
            Ok(Formula::Number(best))
        }
        Function::If => {
            let condition = truth(next()?)?;
            match (next()?, next()?) {
                // Two numbers with the same places, such as two decimals printed alike.
                (Formula::Number(a), Formula::Number(b)) if a.places == b.places => {
                    Ok(Formula::Number(Number {
                        term: pool.ite(condition, a.term, b.term),
                        low: a.low.min(b.low),
                        high: a.high.max(b.high),
                        places: a.places,
                    }))
                }
                (Formula::Truth(a), Formula::Truth(b)) => {
                    Ok(Formula::Truth(pool.ite(condition, a, b)))
                }
                (Formula::Float(a), Formula::Float(b)) if pool.width(a) == pool.width(b) => {
                    let chosen = pool.ite(condition, a, b);
                    finite(pool, chosen)
                }
                (Formula::Text(a), Formula::Text(b)) if a.len() == b.len() => Ok(Formula::Text(
                    a.iter()
                        .zip(&b)
                        .map(|(x, y)| pool.ite(condition, *x, *y))
                        .collect(),
                )),
                _ => Err(
                    "an if between values of different kinds or texts of different lengths".into(),
                ),
            }
        }
        Function::Scaled => {
            let Formula::Number(value) = next()? else {
                return Err("scaled of true or false".into());
            };
            let places = match &arguments[1] {
                Expr::Literal(Value::Int(places)) => u8::try_from(*places)
                    .ok()
                    .filter(|places| *places <= 30)
                    .ok_or("scaled with an unusual number of places")?,
                _ => return Err("scaled with places given by a formula".into()),
            };
            // The digits are the value at its own places; more places multiply by ten each.
            let more = places
                .checked_sub(value.places)
                .ok_or("scaled to fewer places than the decimal has")?;
            let factor = 10_i128.checked_pow(u32::from(more)).ok_or_else(overflow)?;
            let (low, high) = span(&[
                value.low.checked_mul(factor),
                value.high.checked_mul(factor),
            ])?;
            let factor = pool.bv_signed(WIDTH, factor);
            Ok(Formula::Number(Number::whole(
                pool.mul(value.term, factor),
                low,
                high,
            )))
        }
        // From all ones, a CRC step a byte, inverted at the end: one chain of steps, the term a
        // program's `crc32` instructions build however many bytes each takes.
        Function::Crc32 | Function::Crc32c => {
            let bytes = text(next()?)?;
            let poly = if function == Function::Crc32 {
                ZLIB
            } else {
                CASTAGNOLI
            };
            let mut state = pool.bv(32, u128::from(u32::MAX));
            for byte in bytes {
                state = pool.crc(state, byte, poly);
            }
            let inverted = pool.not(state);
            Ok(Formula::Number(Number::whole(
                pool.zero_extend(inverted, WIDTH - 32),
                0,
                i128::from(u32::MAX),
            )))
        }
        Function::Double => match next()? {
            Formula::Number(number) if number.places == 0 => {
                if number.low < -EXACT_DOUBLE || number.high > EXACT_DOUBLE {
                    return Err("double(n) of a whole number a double can't hold exactly".into());
                }
                let term = whole_float(pool, number, 64)?;
                finite(pool, term)
            }
            other => {
                let term = widened(pool, other)?;
                finite(pool, term)
            }
        },
        Function::Single => match next()? {
            Formula::Number(number) if number.places == 0 => {
                if number.low < -EXACT_SINGLE || number.high > EXACT_SINGLE {
                    return Err("single(n) of a whole number a single can't hold exactly".into());
                }
                let term = whole_float(pool, number, 32)?;
                finite(pool, term)
            }
            // Rounds to the nearest, as cvtsd2ss does.
            Formula::Float(term) if pool.width(term) == 64 => {
                let narrowed = pool.float_convert(term, 32);
                finite(pool, narrowed)
            }
            other => {
                let term = as_float(pool, other, 32)?;
                finite(pool, term)
            }
        },
        Function::Fma => {
            let values: Vec<Formula> = std::iter::from_fn(|| next().ok()).collect();
            let [a, b, c] =
                <[Formula; 3]>::try_from(values).map_err(|_| "fma without its three arguments")?;
            let width = if [&a, &b, &c].iter().any(|value| single(pool, value)) {
                32
            } else {
                64
            };
            let (x, y, z) = (
                as_float(pool, a, width)?,
                as_float(pool, b, width)?,
                as_float(pool, c, width)?,
            );
            let result = pool.float_fma(x, y, z);
            finite(pool, result)
        }
        Function::Round | Function::Trunc | Function::Floor | Function::Ceil => {
            let x = widened(pool, next()?)?;
            // cvtsd2si rounds to nearest, ties to even, and cvttsd2si truncates; floor and
            // ceil round to a whole float first, as roundsd does.
            let (x, truncate) = match function {
                Function::Round => (x, false),
                Function::Trunc => (x, true),
                Function::Floor => (pool.float_round(x, Rounding::Down), true),
                _ => (pool.float_round(x, Rounding::Up), true),
            };
            let (low, high) = pool
                .float_bounds(x)
                .and_then(|bounds| float::to_int_range(bounds, 64, truncate))
                .ok_or("a rounding that could leave 64-bit whole numbers")?;
            let whole = pool.float_to_int(x, 64, truncate);
            Ok(Formula::Number(Number::whole(
                pool.sign_extend(whole, WIDTH - 64),
                low,
                high,
            )))
        }
        Function::Sqrt => {
            let value = next()?;
            let x = if single(pool, &value) {
                as_float(pool, value, 32)?
            } else {
                as_float(pool, value, 64)?
            };
            // -0.0 has a square root of its own; below it, the test language stops.
            if !pool.float_bounds(x).is_some_and(|(low, _)| low >= 0.0) {
                return Err("sqrt of a number that could be below zero".into());
            }
            let root = pool.float_sqrt(x);
            finite(pool, root)
        }
        // One multiplication by an exact power of ten, then to the nearest whole number, ties
        // to even: mulsd, then cvtsd2si. The digits print with the places after a point.
        Function::Fixed => {
            let x = widened(pool, next()?)?;
            let places = match &arguments[1] {
                Expr::Literal(Value::Int(places)) => u8::try_from(*places)
                    .ok()
                    .filter(|places| *places <= DOUBLE_PLACES)
                    .ok_or("fixed's places go from 0 to 15")?,
                _ => return Err("fixed with places given by a formula".into()),
            };
            let scale = power_of_ten(pool, places, 64);
            let scaled = pool.float(FloatOp::Mul, x, scale);
            #[allow(clippy::cast_precision_loss)] // 2^53 is exact.
            let limit = EXACT_DOUBLE as f64;
            let (low, high) = pool
                .float_bounds(scaled)
                .filter(|(low, high)| low.abs() < limit && high.abs() < limit)
                .and_then(|bounds| float::to_int_range(bounds, 64, false))
                .ok_or("fixed of a value that could be too large to round exactly")?;
            let whole = pool.float_to_int(scaled, 64, false);
            Ok(Formula::Number(Number {
                term: pool.sign_extend(whole, WIDTH - 64),
                low,
                high,
                places,
            }))
        }
        Function::Sort | Function::Uniq => {
            let call = Expr::Call(function, arguments.to_vec());
            let newline = pool.bv(8, u128::from(b'\n'));
            let texts = lined(pool, &call, vars, texts)?
                .into_iter()
                .map(|(condition, lines)| {
                    let mut text = Vec::new();
                    for line in lines {
                        text.extend(line);
                        text.push(newline);
                    }
                    (condition, text)
                })
                .collect();
            Ok(Formula::Texts(texts))
        }
        Function::Tally => {
            Err("tally counts distinct words, which the proof doesn't follow yet".into())
        }
        other => Err(format!("the function {other:?}")),
    }
}

/// The most bytes a text `sort` or `uniq` reads may have: each way its newlines can fall, and
/// each order its lines can sort in, is a text of its own.
const MAX_LINED: usize = 8;

/// A text as `sort` and `uniq` read it: for each way its newlines can fall and its lines
/// compare, where that holds, and its lines, each without its newline.
type Lined = Vec<(Term, Vec<Vec<Term>>)>;

/// `expr`'s lines: a text split at its newlines, a last line without one still a line; `sort`
/// of one, its lines in byte order; `uniq` of one, each run of the same line kept once.
fn lined(
    pool: &mut Pool,
    expr: &Expr,
    vars: &BTreeMap<String, Number>,
    texts: &BTreeMap<String, Vec<Term>>,
) -> Result<Lined, String> {
    match expr {
        Expr::Call(function @ (Function::Sort | Function::Uniq), arguments) => {
            let [inner] = arguments.as_slice() else {
                return Err("sort or uniq without its one text".into());
            };
            let mut out = Vec::new();
            for (condition, lines) in lined(pool, inner, vars, texts)? {
                let ways = if *function == Function::Sort {
                    sorted(pool, &lines)
                } else {
                    deduplicated(pool, &lines)
                };
                for (holds, lines) in ways {
                    let both = pool.bool_and(condition, holds);
                    if pool.truth(both) != Some(false) {
                        out.push((both, lines));
                    }
                }
            }
            Ok(out)
        }
        other => {
            let bytes = text(translate(pool, other, vars, texts)?)?;
            if bytes.len() > MAX_LINED {
                return Err(format!(
                    "sort and uniq of a text over {MAX_LINED} bytes, whose ways its newlines can \
                     fall the proof doesn't follow"
                ));
            }
            let newline = pool.bv(8, u128::from(b'\n'));
            let breaks: Vec<Term> = bytes.iter().map(|byte| pool.eq(*byte, newline)).collect();
            let mut out = Vec::new();
            for falls in 0..1_u32 << bytes.len() {
                let mut conditions = Vec::with_capacity(bytes.len());
                let mut lines = Vec::new();
                let mut line = Vec::new();
                for (index, byte) in bytes.iter().enumerate() {
                    if falls >> index & 1 == 1 {
                        conditions.push(breaks[index]);
                        lines.push(std::mem::take(&mut line));
                    } else {
                        conditions.push(pool.bool_not(breaks[index]));
                        line.push(*byte);
                    }
                }
                if !line.is_empty() {
                    lines.push(line);
                }
                let condition = pool.all(&conditions);
                if pool.truth(condition) != Some(false) {
                    out.push((condition, lines));
                }
            }
            Ok(out)
        }
    }
}

/// Whether line `a` comes before line `b` or is the same, in byte order: at the first byte they
/// differ, `a`'s is lower, or `a` ends first.
fn at_most(pool: &mut Pool, a: &[Term], b: &[Term]) -> Term {
    let mut holds = pool.boolean(a.len() <= b.len());
    for index in (0..a.len().min(b.len())).rev() {
        let lower = pool.ult(a[index], b[index]);
        let same = pool.eq(a[index], b[index]);
        let then = pool.bool_and(same, holds);
        holds = pool.bool_or(lower, then);
    }
    holds
}

/// Whether two lines are the same.
fn same_line(pool: &mut Pool, a: &[Term], b: &[Term]) -> Term {
    if a.len() != b.len() {
        return pool.boolean(false);
    }
    let checks: Vec<Term> = a.iter().zip(b).map(|(x, y)| pool.eq(*x, *y)).collect();
    pool.all(&checks)
}

/// `lines` in byte order: each order they can come in, where it's sorted. Lines that are the
/// same term for term give one order between them, and of lines that compare the same, the
/// one that came first stays first, so each sorted text comes once.
fn sorted(pool: &mut Pool, lines: &[Vec<Term>]) -> Vec<(Term, Vec<Vec<Term>>)> {
    let mut out = Vec::new();
    let mut order: Vec<usize> = Vec::new();
    let mut used = vec![false; lines.len()];
    let yes = pool.boolean(true);
    arrange(pool, lines, &mut order, &mut used, yes, &mut out);
    out
}

/// The sorted orders that go on from `order`, each where `holds` and the rest of its order do.
fn arrange(
    pool: &mut Pool,
    lines: &[Vec<Term>],
    order: &mut Vec<usize>,
    used: &mut [bool],
    holds: Term,
    out: &mut Vec<(Term, Vec<Vec<Term>>)>,
) {
    if pool.truth(holds) == Some(false) {
        return;
    }
    if order.len() == lines.len() {
        out.push((
            holds,
            order.iter().map(|index| lines[*index].clone()).collect(),
        ));
        return;
    }
    for next in 0..lines.len() {
        // A line the same as one not yet placed before it comes after that one.
        if used[next] || (0..next).any(|earlier| !used[earlier] && lines[earlier] == lines[next]) {
            continue;
        }
        let step = match order.last() {
            Some(&last) => {
                let ordered = at_most(pool, &lines[last], &lines[next]);
                // Equal lines keep the order they came in.
                if last > next {
                    let same = same_line(pool, &lines[last], &lines[next]);
                    let differ = pool.bool_not(same);
                    pool.bool_and(ordered, differ)
                } else {
                    ordered
                }
            }
            None => pool.boolean(true),
        };
        let holds = pool.bool_and(holds, step);
        used[next] = true;
        order.push(next);
        arrange(pool, lines, order, used, holds, out);
        order.pop();
        used[next] = false;
    }
}

/// `lines` with each run of the same line kept once: for each way neighbours can be the same
/// or differ.
fn deduplicated(pool: &mut Pool, lines: &[Vec<Term>]) -> Vec<(Term, Vec<Vec<Term>>)> {
    let Some((first, rest)) = lines.split_first() else {
        return vec![(pool.boolean(true), Vec::new())];
    };
    let mut ways = vec![(pool.boolean(true), vec![first.clone()])];
    let mut previous = first;
    for line in rest {
        let same = same_line(pool, previous, line);
        let differ = pool.bool_not(same);
        let mut next = Vec::new();
        for (holds, kept) in ways {
            let dropped = pool.bool_and(holds, same);
            if pool.truth(dropped) != Some(false) {
                next.push((dropped, kept.clone()));
            }
            let added = pool.bool_and(holds, differ);
            if pool.truth(added) != Some(false) {
                let mut kept = kept;
                kept.push(line.clone());
                next.push((added, kept));
            }
        }
        ways = next;
        previous = line;
    }
    ways
}

/// A text's characters, each its bytes: a byte that can only continue a character, 0x80 to
/// 0xbf, stays with the byte before it. `None` when a byte could be either kind.
fn characters(pool: &Pool, bytes: &[Term]) -> Option<Vec<Vec<Term>>> {
    let mut characters: Vec<Vec<Term>> = Vec::new();
    for byte in bytes {
        let (low, high) = pool.range(*byte);
        let (low, high) = if high < 0x80 || low >= 0x80 && high <= 0xbf || low >= 0xc0 {
            (low, high)
        } else {
            // A range that spans kinds of byte may still hold only one kind.
            match pool.byte_values(*byte) {
                Some(values) if !values.is_empty() => {
                    (u128::from(values[0]), u128::from(values[values.len() - 1]))
                }
                _ => (low, high),
            }
        };
        let continues = (0x80..=0xbf).contains(&low) && high <= 0xbf;
        let starts = high < 0x80 || low >= 0xc0;
        match characters.last_mut() {
            Some(character) if continues => character.push(*byte),
            _ if continues || starts => characters.push(vec![*byte]),
            _ => return None,
        }
    }
    Some(characters)
}

/// `tr` with characters beyond ASCII: each character, as its bytes, becomes its pair's when it
/// is that pair's, a pair at a time; a character keeps its width in bytes, so the text keeps
/// its shape. A pair that changes a character's width isn't one the proof can follow.
fn translate_characters(
    pool: &mut Pool,
    bytes: &[Term],
    pairs: &[(char, char)],
) -> Result<Vec<Term>, String> {
    let utf8 = |character: char| character.to_string().into_bytes();
    if pairs
        .iter()
        .any(|(source, target)| source.len_utf8() != target.len_utf8())
    {
        return Err("tr that changes a character's width in bytes".into());
    }
    let characters =
        characters(pool, bytes).ok_or("tr on text whose characters can't be told apart")?;
    let mut out = Vec::with_capacity(bytes.len());
    for character in characters {
        let mut result = character.clone();
        for (source, target) in pairs {
            let (source, target) = (utf8(*source), utf8(*target));
            if source.len() != character.len() {
                continue;
            }
            let mut matched = pool.boolean(true);
            for (byte, wanted) in character.iter().zip(&source) {
                let wanted = pool.bv(8, u128::from(*wanted));
                let same = pool.eq(*byte, wanted);
                matched = pool.bool_and(matched, same);
            }
            for (slot, value) in result.iter_mut().zip(&target) {
                let value = pool.bv(8, u128::from(*value));
                *slot = pool.ite(matched, value, *slot);
            }
        }
        out.extend(result);
    }
    Ok(out)
}

/// Each byte with `pairs` applied: a byte equal to a pair's first character becomes its second.
fn translate_bytes(pool: &mut Pool, bytes: &[Term], pairs: &[(char, char)]) -> Vec<Term> {
    // Later pairs win; then the bytes that move by the same amount move together: upper is
    // a to z moving down 32, one choice per byte.
    let mapping: BTreeMap<u8, u8> = pairs
        .iter()
        .map(|(source, target)| (*source as u8, *target as u8))
        .collect();
    let mut moves: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
    for (source, target) in mapping {
        let by = target.wrapping_sub(source);
        if by != 0 {
            moves.entry(by).or_default().push(source);
        }
    }
    bytes
        .iter()
        .map(|byte| {
            let mut result = *byte;
            for (by, sources) in &moves {
                let moved = pool.bv(8, u128::from(*by));
                let moved = pool.add(*byte, moved);
                let matched = pool.is_one_of(*byte, sources);
                result = pool.ite(matched, moved, result);
            }
            result
        })
        .collect()
}

/// How many times `part` appears in `bytes` without overlaps, leftmost first, as `count` counts:
/// a match counts when none of the counted ones before it overlaps it.
fn count(pool: &mut Pool, bytes: &[Term], part: &[u8]) -> Result<Formula, String> {
    if part.is_empty() {
        return Err("count of an empty part".into());
    }
    let length = part.len();
    let mut counted: Vec<Term> = Vec::new();
    for start in 0..bytes.len() {
        let matched = if start + length <= bytes.len() {
            let checks: Vec<Term> = part
                .iter()
                .enumerate()
                .map(|(offset, wanted)| {
                    let wanted = pool.bv(8, u128::from(*wanted));
                    pool.eq(bytes[start + offset], wanted)
                })
                .collect();
            pool.all(&checks)
        } else {
            pool.boolean(false)
        };
        // A counted match within the part's length before this one overlaps it.
        let earlier = start.saturating_sub(length - 1)..start;
        let mut counts = matched;
        for before in earlier {
            let free = pool.bool_not(counted[before]);
            counts = pool.bool_and(counts, free);
        }
        counted.push(counts);
    }
    let high = i128::try_from(bytes.len() / length).map_err(|_| overflow())?;
    Ok(Formula::Number(Number::whole(
        pool.count(WIDTH, &counted),
        0,
        high,
    )))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rand::rngs::ChaCha8Rng;
    use rand::{RngExt, SeedableRng};

    use super::*;

    /// sort and uniq over short texts, a choice of texts for each way the newlines can fall and
    /// the lines compare: on random texts, exactly one choice holds, and it's the text the test
    /// language computes.
    #[test]
    fn sorted_lines_agree_with_the_test_language() {
        let seed = std::env::var("LENGUA_RANDOM_SEED")
            .ok()
            .and_then(|seed| seed.parse().ok())
            .unwrap_or_else(|| {
                let mut bytes = [0; 8];
                getrandom::fill(&mut bytes).expect("a seed");
                u64::from_le_bytes(bytes)
            });
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let alphabet = b"ab\nB";
        let formulas = ["sort(s)", "uniq(s)", "uniq(sort(s))", "sort(s) + \"!\""];
        for _ in 0..300 {
            let length = rng.random_range(0..=MAX_LINED);
            let text: Vec<u8> = (0..length)
                .map(|_| alphabet[rng.random_range(0..alphabet.len())])
                .collect();
            for source in formulas {
                let expr = crate::expr::parse(source).expect("a formula");
                let shown = String::from_utf8(text.clone()).expect("ASCII");
                let env: crate::expr::Env = [("s".to_owned(), Value::Str(shown.clone()))]
                    .into_iter()
                    .collect();
                let expected = crate::expr::eval(&expr, &env).expect("evaluates");
                let mut pool = Pool::default();
                let bytes: Vec<Term> = (0..length)
                    .map(|index| pool.var(8, &format!("s{index}")))
                    .collect();
                let values: HashMap<String, u128> = (0..length)
                    .map(|index| (format!("s{index}"), u128::from(text[index])))
                    .collect();
                let texts: BTreeMap<String, Vec<Term>> = [("s".to_owned(), bytes)].into();
                let formula = translate(&mut pool, &expr, &BTreeMap::new(), &texts);
                let choices = match formula {
                    Ok(Formula::Texts(choices)) => choices,
                    // A choice joined to more text is beyond the translation.
                    Err(_) if source.contains('+') => continue,
                    other => panic!("{source}: {other:?}"),
                };
                let holding: Vec<String> = choices
                    .iter()
                    .filter(|(condition, _)| pool.evaluate(*condition, &values) == Some(1))
                    .map(|(_, bytes)| {
                        bytes
                            .iter()
                            .map(|byte| {
                                pool.evaluate(*byte, &values).expect("a byte") as u8 as char
                            })
                            .collect()
                    })
                    .collect();
                assert_eq!(
                    holding,
                    vec![match &expected {
                        Value::Str(text) => text.clone(),
                        other => panic!("{other:?}"),
                    }],
                    "seed {seed}: {source} of {shown:?}"
                );
            }
        }
    }

    /// The formulas over random texts, evaluated at their bytes, must give what the test
    /// language computes.
    #[test]
    fn text_functions_agree_with_the_test_language() {
        let seed = std::env::var("LENGUA_RANDOM_SEED")
            .ok()
            .and_then(|seed| seed.parse().ok())
            .unwrap_or_else(|| {
                let mut bytes = [0; 8];
                getrandom::fill(&mut bytes).expect("a seed");
                u64::from_le_bytes(bytes)
            });
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let alphabets = [
            "ab",
            "a b\n",
            "x. \t\n\x0b\x0c\r",
            "aAzZ09-_",
            "aé€😀",
            " é\u{a0}x",
        ];
        let formulas = [
            "len(s)",
            "count(s, \"a\")",
            "count(s, \"aa\")",
            "count(s, \"aba\")",
            "count(s, \"\\n\")",
            "count(s, \"é\")",
            "words(s)",
            "upper(s)",
            "lower(s)",
            "tr(s, \"a-z\", \"n-za-m\")",
            "tr(s, \"é€😀\", \"è₤😃\")",
            "tr(s, \"aé\", \"bè\")",
            "reverse(s)",
            "reverse(upper(s)) + s",
            "s + \"!\" + upper(s)",
            "if(s == \"ab\", 1, 0)",
        ];
        for _ in 0..400 {
            let alphabet: Vec<char> = alphabets[rng.random_range(0..alphabets.len())]
                .chars()
                .collect();
            let length = rng.random_range(0..12);
            let chosen: Vec<char> = (0..length)
                .map(|_| alphabet[rng.random_range(0..alphabet.len())])
                .collect();
            let text: String = chosen.iter().collect();
            let widths: Vec<u8> = chosen
                .iter()
                .map(|character| character.len_utf8() as u8)
                .collect();
            // Each character's variable: an ASCII character's byte, or a wider one's place
            // among the alphabet's characters of its width, as a proof's input has them.
            let values: HashMap<String, u128> = chosen
                .iter()
                .enumerate()
                .map(|(index, character)| {
                    let value = if character.is_ascii() {
                        u128::from(*character as u8)
                    } else {
                        alphabet
                            .iter()
                            .filter(|other| other.len_utf8() == character.len_utf8())
                            .position(|other| other == character)
                            .expect("in the alphabet") as u128
                    };
                    (format!("s.c{index}"), value)
                })
                .collect();
            for source in formulas {
                let expr = crate::expr::parse(source).expect("a formula");
                let env: crate::expr::Env = [("s".to_owned(), Value::Str(text.clone()))]
                    .into_iter()
                    .collect();
                let expected = crate::expr::eval(&expr, &env).expect("evaluates");
                let mut pool = Pool::default();
                let input = super::super::utf8_input(&mut pool, "s", &widths, &alphabet);
                let texts: BTreeMap<String, Vec<Term>> = [("s".to_owned(), input.text)].into();
                let formula = translate(&mut pool, &expr, &BTreeMap::new(), &texts)
                    .unwrap_or_else(|error| panic!("{source}: {error}"));
                let found = match formula {
                    Formula::Number(number) => {
                        let value = pool.evaluate(number.term, &values).expect("a value");
                        Value::Int(i128::from_ne_bytes(value.to_ne_bytes()))
                    }
                    Formula::Text(bytes) => Value::Str(
                        String::from_utf8(
                            bytes
                                .iter()
                                .map(|byte| pool.evaluate(*byte, &values).expect("a byte") as u8)
                                .collect(),
                        )
                        .expect("UTF-8"),
                    ),
                    Formula::Truth(term) => {
                        Value::Bool(pool.evaluate(term, &values).expect("a value") == 1)
                    }
                    Formula::Float(_) => panic!("{source} gave a float"),
                    Formula::Texts(_) => panic!("{source} gave a choice of texts"),
                };
                assert_eq!(found, expected, "seed {seed}: {source} of {text:?}");
            }
        }
    }

    /// Decimal and float formulas over random decimals and whole numbers, evaluated at their
    /// digits, must give what the test language computes, bit for bit.
    #[test]
    fn float_functions_agree_with_the_test_language() {
        let seed = std::env::var("LENGUA_RANDOM_SEED")
            .ok()
            .and_then(|seed| seed.parse().ok())
            .unwrap_or_else(|| {
                let mut bytes = [0; 8];
                getrandom::fill(&mut bytes).expect("a seed");
                u64::from_le_bytes(bytes)
            });
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        // a and b: decimals of 3 places up to 1000000.000; c and d: up to 16777.215, which
        // singles hold; n and m: whole numbers up to a million.
        let formulas = [
            "fixed(sqrt(a * a + b * b), 3)",
            "fixed(sqrt(double(n) * double(n) + double(m) * double(m)), 3)",
            "round(a * 100.0)",
            "trunc(a / b)",
            "floor(a - b)",
            "ceil(a * b)",
            "fixed(fma(a, b, a), 2)",
            "fixed(a / 3.0, 6)",
            "fixed(-a, 1)",
            "fixed(abs(a - b), 3)",
            "round(double(n) / 7.0)",
            "fixed(double(n) / 1000.0, 3)",
            "fixed(single(c) * single(d), 2)",
            "fixed(sqrt(single(c)), 4)",
            "fixed(fma(single(c), d, c), 1)",
            "fixed(single(double(n) / 3.0), 3)",
            "a < b",
            "a * 2.0 == b",
            "double(n) <= a",
            "single(c) > d",
            "single(c) >= double(m)",
            "a < 500.25",
            "if(a < b, fixed(a, 2), fixed(b, 2))",
        ];
        let decimal = |rng: &mut ChaCha8Rng, high: i128| {
            // Edges and ties as often as anything else.
            match rng.random_range(0..4) {
                0 => [1, 5, 500, 1000, 1500, 2500, high][rng.random_range(0..7)],
                _ => rng.random_range(1..=high),
            }
        };
        let mut compared = 0;
        for _ in 0..400 {
            let values = [
                ("a", decimal(&mut rng, 1_000_000_000), 3),
                ("b", decimal(&mut rng, 1_000_000_000), 3),
                ("c", decimal(&mut rng, 16_777_215), 3),
                ("d", decimal(&mut rng, 16_777_215), 3),
                ("n", rng.random_range(0..=1_000_000), 0),
                ("m", rng.random_range(0..=1_000_000), 0),
            ];
            let env: crate::expr::Env = values
                .iter()
                .map(|(name, m, p)| {
                    let value = if *p == 0 {
                        Value::Int(*m)
                    } else {
                        Value::Dec { m: *m, p: *p }
                    };
                    ((*name).to_owned(), value)
                })
                .collect();
            let by_name: HashMap<String, u128> = values
                .iter()
                .map(|(name, m, _)| ((*name).to_owned(), *m as u128))
                .collect();
            for source in formulas {
                let expr = crate::expr::parse(source).expect("a formula");
                let Ok(expected) = crate::expr::eval(&expr, &env) else {
                    continue;
                };
                let mut pool = Pool::default();
                let vars: BTreeMap<String, Number> = [
                    ("a", 1_000_000_000, 3),
                    ("b", 1_000_000_000, 3),
                    ("c", 16_777_215, 3),
                    ("d", 16_777_215, 3),
                    ("n", 1_000_000, 0),
                    ("m", 1_000_000, 0),
                ]
                .iter()
                .map(|(name, high, places)| {
                    let low = i128::from(*places > 0);
                    let digits = pool.var_in(64, name, low as u128, *high as u128);
                    let term = pool.zero_extend(digits, WIDTH - 64);
                    let number = Number {
                        term,
                        low,
                        high: *high,
                        places: *places,
                    };
                    ((*name).to_owned(), number)
                })
                .collect();
                let formula = translate(&mut pool, &expr, &vars, &BTreeMap::new())
                    .unwrap_or_else(|error| panic!("{source}: {error}"));
                let evaluate = |term: Term| pool.evaluate(term, &by_name).expect("a value");
                compared += 1;
                let found = match formula {
                    Formula::Number(number) => {
                        let value = i128::from_ne_bytes(evaluate(number.term).to_ne_bytes());
                        if number.places > 0 {
                            Value::Str(crate::expr::decimal_text(value, number.places))
                        } else {
                            Value::Int(value)
                        }
                    }
                    Formula::Truth(term) => Value::Bool(evaluate(term) == 1),
                    Formula::Float(term) => {
                        let bits = evaluate(term);
                        let wanted = match &expected {
                            Value::Float(value) => u128::from(value.value().to_bits()),
                            Value::Single(value) => u128::from(value.value().to_bits()),
                            other => panic!("{source} gave {other:?} in the test language"),
                        };
                        assert_eq!(bits, wanted, "seed {seed}: {source} with {values:?}");
                        continue;
                    }
                    Formula::Text(_) | Formula::Texts(_) => panic!("{source} gave text"),
                };
                assert_eq!(found, expected, "seed {seed}: {source} with {values:?}");
            }
        }
        // Only a few draws make the test language refuse a formula, such as a sum past 2^53.
        assert!(
            compared > 400 * formulas.len() * 9 / 10,
            "seed {seed}: {compared} compared"
        );
    }
}
