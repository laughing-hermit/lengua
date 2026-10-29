//! The tests' whole-number formulas as terms, with the range of values each part can take, so a
//! formula that could overflow or divide by zero is caught before it's trusted.

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formula {
    /// A whole number.
    Number(Number),
    /// True or false.
    Truth(Term),
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
    }
}

fn truth(formula: Formula) -> Result<Term, String> {
    match formula {
        Formula::Truth(term) => Ok(term),
        Formula::Number(_) => Err("a formula uses a number where true/false goes".into()),
    }
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
) -> Result<Formula, String> {
    match expr {
        Expr::Literal(Value::Int(value)) => Ok(Formula::Number(Number::whole(
            pool.bv_signed(WIDTH, *value),
            *value,
            *value,
        ))),
        Expr::Literal(Value::Bool(value)) => Ok(Formula::Truth(pool.boolean(*value))),
        Expr::Literal(_) => Err("a formula with text or decimals".into()),
        Expr::Var(name) => vars
            .get(name)
            .copied()
            .map(Formula::Number)
            .ok_or_else(|| format!("the variable {name}, which isn't a whole number")),
        Expr::Neg(inner) => {
            let inner = number(translate(pool, inner, vars)?)?;
            let (low, high) = span(&[inner.high.checked_neg(), inner.low.checked_neg()])?;
            Ok(Formula::Number(Number::whole(
                pool.neg(inner.term),
                low,
                high,
            )))
        }
        Expr::Not(inner) => {
            let inner = truth(translate(pool, inner, vars)?)?;
            Ok(Formula::Truth(pool.bool_not(inner)))
        }
        Expr::Binary(op, left, right) => binary(pool, *op, left, right, vars),
        Expr::Call(function, arguments) => call(pool, *function, arguments, vars),
    }
}

fn binary(
    pool: &mut Pool,
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    vars: &BTreeMap<String, Number>,
) -> Result<Formula, String> {
    let (a, b) = (translate(pool, left, vars)?, translate(pool, right, vars)?);
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
) -> Result<Formula, String> {
    let mut values = Vec::new();
    for argument in arguments {
        values.push(translate(pool, argument, vars)?);
    }
    match function {
        Function::Abs => {
            let a = number(values[0])?;
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
            let mut best = number(values[0])?;
            for value in &values[1..] {
                let next = number(*value)?;
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
            let condition = truth(values[0])?;
            match (values[1], values[2]) {
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
                _ => Err("an if with a number on one side and true/false on the other".into()),
            }
        }
        Function::Scaled => {
            let Formula::Number(value) = values[0] else {
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
