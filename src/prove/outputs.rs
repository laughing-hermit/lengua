//! Random-value tests of what "the output is right" means to the prover. For printed numbers,
//! with or without text around them, the condition built from the output's bytes must hold when
//! the bytes are the formula's decimal text and never otherwise: a condition too weak would let
//! a wrong program be proven.

use std::collections::{BTreeMap, HashMap};

use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

use super::formula::{Number, WIDTH};
use super::term::{Pool, Term};
use super::{number_is, text_is};
use crate::expr::Template;

fn seed() -> u64 {
    std::env::var("LENGUA_RANDOM_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or_else(|| {
            let mut bytes = [0; 8];
            getrandom::fill(&mut bytes).expect("a seed");
            u64::from_le_bytes(bytes)
        })
}

/// A formula value of any 128-bit size, by its name.
fn formula(pool: &mut Pool, name: &str) -> Number {
    Number {
        term: pool.var(WIDTH, name),
        low: -(1 << 120),
        high: 1 << 120,
    }
}

/// `value` as the solver's 128-bit two's complement.
const fn bits(value: i128) -> u128 {
    value.cast_unsigned()
}

/// A whole number, often an edge: zero, a power of ten or one off it, the ends of 64 bits.
fn number(rng: &mut ChaCha8Rng) -> i128 {
    let magnitude: i128 = match rng.random_range(0..5) {
        0 => 0,
        1 => 10_i128.pow(rng.random_range(0..19)),
        2 => 10_i128.pow(rng.random_range(1..19)) - 1,
        3 => i128::from(i64::MAX),
        _ => i128::from(rng.random::<i64>()).abs(),
    };
    let magnitude = magnitude + i128::from(rng.random_range(-1_i8..=1)) * i128::from(magnitude > 0);
    if rng.random_bool(0.5) {
        -magnitude
    } else {
        magnitude
    }
}

/// Text that's `value` in decimal, or nearly: what a buggy program might print instead.
fn near(rng: &mut ChaCha8Rng, value: i128) -> String {
    let exact = value.to_string();
    match rng.random_range(0..10) {
        0 => format!("0{exact}"),
        1 => format!("{exact}0"),
        2 => format!("-{exact}"),
        3 => format!("+{exact}"),
        4 => exact[..exact.len() - 1].to_owned(),
        5 => exact.replacen('-', "", 1),
        6 => format!(" {exact}"),
        7 => "-0".to_owned(),
        8 => String::new(),
        _ => exact,
    }
}

fn bytes_of(pool: &mut Pool, text: &str) -> Vec<Term> {
    text.bytes()
        .map(|byte| pool.bv(8, u128::from(byte)))
        .collect()
}

fn holds(pool: &Pool, condition: Term, values: &[(&str, u128)]) -> bool {
    let values: HashMap<String, u128> = values
        .iter()
        .map(|(name, value)| ((*name).to_owned(), *value))
        .collect();
    pool.evaluate(condition, &values) == Some(1)
}

#[test]
fn printed_text_matches_exactly_its_formula() {
    let seed = seed();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    for _ in 0..3000 {
        let mut pool = Pool::default();
        let target = number(&mut rng);
        let text = near(&mut rng, target);
        let bytes = bytes_of(&mut pool, &text);
        let value = formula(&mut pool, "F");
        let condition = number_is(&mut pool, &bytes, value);
        let parsed = text.parse::<i128>().ok();
        for candidate in [
            Some(target),
            Some(target + 1),
            Some(target - 1),
            Some(-target),
            Some(0),
            parsed,
        ]
        .into_iter()
        .flatten()
        {
            assert_eq!(
                holds(&pool, condition, &[("F", bits(candidate))]),
                text == candidate.to_string(),
                "seed {seed}: \"{text}\" against {candidate}"
            );
        }
    }
}

/// Bytes like a program's print loop writes: `count` digits of `root`, each one
/// `'0' + (root / 10^k) % 10`.
fn printed(pool: &mut Pool, root: Term, count: u32) -> Vec<Term> {
    let width = pool.width(root);
    (0..count)
        .map(|index| {
            let power = 10_u128.pow(count - 1 - index);
            let quotient = if power == 1 {
                root
            } else {
                let power = pool.bv(width, power);
                pool.udiv(root, power)
            };
            let ten = pool.bv(width, 10);
            let digit = pool.urem(quotient, ten);
            let low = pool.extract(digit, 7, 0);
            let zero = pool.bv(8, u128::from(b'0'));
            pool.add(low, zero)
        })
        .collect()
}

#[test]
fn printed_digits_match_only_their_own_value() {
    let seed = seed();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    for _ in 0..400 {
        let mut pool = Pool::default();
        let count = rng.random_range(1..=19_u32);
        let negative = rng.random_bool(0.5);
        let v = pool.var(64, "v");
        // A negative value prints a minus and then the digits of its negation.
        let bytes = if negative {
            let size = pool.neg(v);
            let mut bytes = bytes_of(&mut pool, "-");
            bytes.extend(printed(&mut pool, size, count));
            bytes
        } else {
            printed(&mut pool, v, count)
        };
        let value = formula(&mut pool, "F");
        let condition = number_is(&mut pool, &bytes, value);
        for _ in 0..30 {
            // The value the print loop was made for, a neighbor, or anything at all.
            let fitting = 10_i128.pow(count - 1) + rng.random_range(0..10_i128.pow(count) * 9 / 10);
            let fitting = fitting.min(i128::from(i64::MAX));
            let program = match rng.random_range(0..3) {
                0 if negative => -fitting,
                0 => fitting,
                1 => number(&mut rng),
                _ => i128::from(rng.random::<i64>()),
            };
            let Ok(program) = i64::try_from(program) else {
                continue;
            };
            let v_bits = u128::from(program.cast_unsigned());
            let text: String = bytes
                .iter()
                .map(|byte| {
                    let byte = pool
                        .evaluate(*byte, &HashMap::from([("v".to_owned(), v_bits)]))
                        .expect("a byte");
                    char::from(u8::try_from(byte).expect("a byte"))
                })
                .collect();
            let right = i128::from(program);
            for candidate in [right, right + 1, right - 1, -right, number(&mut rng)] {
                let ok = holds(&pool, condition, &[("v", v_bits), ("F", bits(candidate))]);
                let same = text == candidate.to_string();
                // Never true for other text: that would prove a wrong program.
                assert!(
                    !ok || same,
                    "seed {seed}: \"{text}\" accepted as {candidate} (v = {program})"
                );
                // True for the program's own value when the loop printed all of it.
                let printed_all = right.unsigned_abs().to_string().len() == count as usize
                    && (right < 0) == negative;
                if same && printed_all && candidate == right {
                    assert!(ok, "seed {seed}: \"{text}\" rejected as {candidate}");
                }
            }
        }
    }
}

#[test]
fn text_around_a_number_matches_exactly() {
    let seed = seed();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let template = Template::parse("sum = {a + b}!").expect("a template");
    for _ in 0..1500 {
        let mut pool = Pool::default();
        let numbers = BTreeMap::from([
            ("a".to_owned(), formula(&mut pool, "a")),
            ("b".to_owned(), formula(&mut pool, "b")),
        ]);
        let (a, b) = (number(&mut rng) / 2, number(&mut rng) / 2);
        let sum = near(&mut rng, a + b);
        let text = match rng.random_range(0..5) {
            0 => format!("sum = {sum}"),
            1 => format!("sum ={sum}!"),
            2 => format!("Sum = {sum}!"),
            3 => format!("sum = {sum}!!"),
            _ => format!("sum = {sum}!"),
        };
        let bytes = bytes_of(&mut pool, &text);
        let condition = text_is(&mut pool, &bytes, &template, &numbers).expect("a condition");
        assert_eq!(
            holds(&pool, condition, &[("a", bits(a)), ("b", bits(b))]),
            text == format!("sum = {}!", a + b),
            "seed {seed}: \"{text}\" with a = {a}, b = {b}"
        );
    }
}
