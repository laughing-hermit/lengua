//! The tests' formulas as terms: whole numbers with the range of values each part can take, so a
//! formula that could overflow or divide by zero is caught before it's trusted, and text byte by
//! byte, with the text functions written out over each byte.

use std::collections::BTreeMap;

use super::term::{CASTAGNOLI, Pool, Term, ZLIB};
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
        Formula::Text(_) => Err("a formula uses text where a number goes".into()),
    }
}

fn truth(formula: Formula) -> Result<Term, String> {
    match formula {
        Formula::Truth(term) => Ok(term),
        Formula::Number(_) | Formula::Text(_) => {
            Err("a formula uses a value where true/false goes".into())
        }
    }
}

fn text(formula: Formula) -> Result<Vec<Term>, String> {
    match formula {
        Formula::Text(bytes) => Ok(bytes),
        Formula::Number(_) | Formula::Truth(_) => {
            Err("a formula uses a number or true/false where text goes".into())
        }
    }
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
        Expr::Literal(Value::Str(value)) => Ok(Formula::Text(
            value
                .bytes()
                .map(|byte| pool.bv(8, u128::from(byte)))
                .collect(),
        )),
        Expr::Literal(_) => Err("a formula with decimals".into()),
        Expr::Var(name) => match (vars.get(name), texts.get(name)) {
            (Some(number), _) => Ok(Formula::Number(*number)),
            (None, Some(bytes)) => Ok(Formula::Text(bytes.clone())),
            (None, None) => Err(format!(
                "the variable {name}, which isn't a whole number or a drawn text"
            )),
        },
        Expr::Neg(inner) => {
            let inner = number(translate(pool, inner, vars, texts)?)?;
            let (low, high) = span(&[inner.high.checked_neg(), inner.low.checked_neg()])?;
            Ok(Formula::Number(Number::whole(
                pool.neg(inner.term),
                low,
                high,
            )))
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
            let a = number(next()?)?;
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
                (Formula::Number(a), Formula::Number(b)) => {
                    let (a, b) = (number(Formula::Number(a))?, number(Formula::Number(b))?);
                    Ok(Formula::Number(Number::whole(
                        pool.ite(condition, a.term, b.term),
                        a.low.min(b.low),
                        a.high.max(b.high),
                    )))
                }
                (Formula::Truth(a), Formula::Truth(b)) => {
                    Ok(Formula::Truth(pool.ite(condition, a, b)))
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
        other => Err(format!("the function {other:?}")),
    }
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
                };
                assert_eq!(found, expected, "seed {seed}: {source} of {text:?}");
            }
        }
    }
}
