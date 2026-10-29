//! The tests' formulas as terms: whole numbers with the range of values each part can take, so a
//! formula that could overflow or divide by zero is caught before it's trusted, and text byte by
//! byte, with the text functions written out over each byte.

use std::collections::BTreeMap;

use super::term::{Pool, Term};
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

/// 1 where `condition` holds, else 0, as a formula's whole number.
fn indicator(pool: &mut Pool, condition: Term) -> Term {
    let (one, zero) = (pool.bv(WIDTH, 1), pool.bv(WIDTH, 0));
    pool.ite(condition, one, zero)
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
                let start = if index == 0 {
                    word
                } else {
                    pool.bool_and(word, spaces[index - 1])
                };
                starts.push(indicator(pool, start));
            }
            let high = i128::try_from(bytes.len().div_ceil(2)).map_err(|_| overflow())?;
            Ok(Formula::Number(Number::whole(
                pool.sum(WIDTH, &starts),
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
            if from.len() != to.len() || !from.iter().chain(&to).all(char::is_ascii) {
                return Err("tr with sets of different lengths or beyond ASCII".into());
            }
            // Later pairs win, as a map built from them in order does.
            let mut map: BTreeMap<char, char> = BTreeMap::new();
            for (source, target) in from.into_iter().zip(to) {
                map.insert(source, target);
            }
            let pairs: Vec<(char, char)> = map.into_iter().collect();
            Ok(Formula::Text(translate_bytes(pool, &bytes, &pairs)))
        }
        Function::Reverse => {
            let mut bytes = text(next()?)?;
            bytes.reverse();
            Ok(Formula::Text(bytes))
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
        other => Err(format!("the function {other:?}")),
    }
}

/// Each byte with `pairs` applied: a byte equal to a pair's first character becomes its second.
fn translate_bytes(pool: &mut Pool, bytes: &[Term], pairs: &[(char, char)]) -> Vec<Term> {
    bytes
        .iter()
        .map(|byte| {
            let mut result = *byte;
            for (source, target) in pairs {
                let source = pool.bv(8, u128::from(*source as u8));
                let target = pool.bv(8, u128::from(*target as u8));
                let matched = pool.eq(*byte, source);
                result = pool.ite(matched, target, result);
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
    let ones: Vec<Term> = counted
        .iter()
        .map(|counts| indicator(pool, *counts))
        .collect();
    let high = i128::try_from(bytes.len() / length).map_err(|_| overflow())?;
    Ok(Formula::Number(Number::whole(
        pool.sum(WIDTH, &ones),
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
        let alphabets: [&[u8]; 4] = [b"ab", b"a b\n", b"x. \t\n\x0b\x0c\r", b"aAzZ09-_"];
        let formulas = [
            "len(s)",
            "count(s, \"a\")",
            "count(s, \"aa\")",
            "count(s, \"aba\")",
            "count(s, \"\\n\")",
            "words(s)",
            "upper(s)",
            "lower(s)",
            "tr(s, \"a-z\", \"n-za-m\")",
            "reverse(s)",
            "s + \"!\" + upper(s)",
            "if(s == \"ab\", 1, 0)",
        ];
        for _ in 0..300 {
            let alphabet = alphabets[rng.random_range(0..alphabets.len())];
            let length = rng.random_range(0..12);
            let text: String = (0..length)
                .map(|_| char::from(alphabet[rng.random_range(0..alphabet.len())]))
                .collect();
            for source in formulas {
                let expr = crate::expr::parse(source).expect("a formula");
                let env: crate::expr::Env = [("s".to_owned(), Value::Str(text.clone()))]
                    .into_iter()
                    .collect();
                let expected = crate::expr::eval(&expr, &env).expect("evaluates");
                let mut pool = Pool::default();
                let bytes: Vec<Term> = (0..text.len())
                    .map(|index| pool.var(8, &format!("s{index}")))
                    .collect();
                let values: HashMap<String, u128> = text
                    .bytes()
                    .enumerate()
                    .map(|(index, byte)| (format!("s{index}"), u128::from(byte)))
                    .collect();
                let texts: BTreeMap<String, Vec<Term>> = [("s".to_owned(), bytes)].into();
                let formula = translate(&mut pool, &expr, &BTreeMap::new(), &texts)
                    .unwrap_or_else(|error| panic!("{source}: {error}"));
                let found = match formula {
                    Formula::Number(number) => {
                        let value = pool.evaluate(number.term, &values).expect("a value");
                        Value::Int(i128::from_ne_bytes(value.to_ne_bytes()))
                    }
                    Formula::Text(bytes) => Value::Str(
                        bytes
                            .iter()
                            .map(|byte| {
                                char::from(pool.evaluate(*byte, &values).expect("a byte") as u8)
                            })
                            .collect(),
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
