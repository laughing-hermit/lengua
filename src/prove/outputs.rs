//! Random-value tests of what "the output is right" means to the prover. For printed numbers,
//! with or without text around them, the condition built from the output's bytes must hold when
//! the bytes are the formula's decimal text and never otherwise: a condition too weak would let
//! a wrong program be proven.

use std::collections::{BTreeMap, HashMap};

use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

use super::formula::{Number, WIDTH};
use super::term::{Pool, Term};
use super::{number_in, number_is, text_is};
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
    Number::whole(pool.var(WIDTH, name), -(1 << 120), 1 << 120)
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
        let condition = number_is(&mut pool, &bytes, value, 0);
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
        let condition = number_is(&mut pool, &bytes, value, 0);
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
        let known = super::Known {
            translated: std::cell::RefCell::default(),
            offsets: HashMap::new(),
            numbers: &numbers,
            fixed: &BTreeMap::new(),
            texts: &BTreeMap::new(),
        };
        let condition = text_is(&mut pool, &bytes, &template, &known).expect("a condition");
        assert_eq!(
            holds(&pool, condition, &[("a", bits(a)), ("b", bits(b))]),
            text == format!("sum = {}!", a + b),
            "seed {seed}: \"{text}\" with a = {a}, b = {b}"
        );
    }
}

#[test]
fn printed_decimals_match_exactly_their_formula() {
    // places(F, k): a whole number's digits with a point before the last k, padded to 0.05.
    let seed = seed();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    for _ in 0..3000 {
        let places = rng.random_range(1..=4_u8);
        let target = number(&mut rng) / 1000;
        let exact = crate::expr::decimal_text(target, places);
        let text = match rng.random_range(0..9) {
            0 => format!("0{exact}"),
            1 => exact.replacen('.', "", 1),
            2 => format!("{exact}0"),
            3 => exact[..exact.len() - 1].to_owned(),
            4 => exact.replacen("0.", ".", 1),
            5 => format!("-{exact}"),
            6 => crate::expr::decimal_text(target, places + 1),
            _ => exact.clone(),
        };
        let mut pool = Pool::default();
        let bytes = bytes_of(&mut pool, &text);
        let value = formula(&mut pool, "F");
        let condition = number_is(&mut pool, &bytes, value, places);
        for candidate in [target, target + 1, target - 1, -target, 0, target * 10] {
            assert_eq!(
                holds(&pool, condition, &[("F", bits(candidate))]),
                text == crate::expr::decimal_text(candidate, places),
                "seed {seed}: \"{text}\" against {candidate} with {places} places"
            );
        }
    }
}

/// A range around `target`: often one that just holds it or just misses it.
fn range_near(rng: &mut ChaCha8Rng, target: i128) -> (i128, i128) {
    let (a, b) = match rng.random_range(0..4) {
        0 => (target, target),
        1 => (target - 1, target + 1),
        2 => (target + 1, target + 9),
        _ => (number(rng), number(rng)),
    };
    (a.min(b), a.max(b))
}

#[test]
fn any_takes_exactly_the_plain_numbers_in_range() {
    // As the runner reads it: an optional minus, digits with no leading zero, never -0.
    let seed = seed();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let template = Template::parse("got {any(lo, hi)}.").expect("a template");
    for _ in 0..3000 {
        let mut pool = Pool::default();
        let numbers = BTreeMap::from([
            ("lo".to_owned(), formula(&mut pool, "lo")),
            ("hi".to_owned(), formula(&mut pool, "hi")),
        ]);
        let target = number(&mut rng);
        let (lo, hi) = range_near(&mut rng, target);
        let printed = near(&mut rng, target);
        let text = match rng.random_range(0..5) {
            0 => format!("got {printed}"),
            1 => format!("got {printed}.."),
            2 => format!("got{printed}."),
            _ => format!("got {printed}."),
        };
        let bytes = bytes_of(&mut pool, &text);
        let known = super::Known {
            translated: std::cell::RefCell::default(),
            offsets: HashMap::new(),
            numbers: &numbers,
            fixed: &BTreeMap::new(),
            texts: &BTreeMap::new(),
        };
        let condition = text_is(&mut pool, &bytes, &template, &known).expect("a condition");
        let rule = format!(
            "got {}.",
            crate::expect::hole(
                crate::expect::Bound::Number(lo),
                crate::expect::Bound::Number(hi)
            )
        );
        assert_eq!(
            holds(&pool, condition, &[("lo", bits(lo)), ("hi", bits(hi))]),
            crate::expect::matches(&rule, text.as_bytes(), 0),
            "seed {seed}: \"{text}\" against any({lo}, {hi})"
        );
    }
}

#[test]
fn printed_digits_fit_any_only_in_range() {
    // The shortcut through the digits' common root must never accept what reading the digits
    // back would refuse.
    let seed = seed();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    for _ in 0..300 {
        let mut pool = Pool::default();
        let count = rng.random_range(1..=19_u32);
        let negative = rng.random_bool(0.3);
        let v = pool.var(64, "v");
        let bytes = if negative {
            let size = pool.neg(v);
            let mut bytes = bytes_of(&mut pool, "-");
            bytes.extend(printed(&mut pool, size, count));
            bytes
        } else {
            printed(&mut pool, v, count)
        };
        let (low, high) = (formula(&mut pool, "lo"), formula(&mut pool, "hi"));
        let condition = number_in(&mut pool, &bytes, low, high);
        for _ in 0..20 {
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
            let text: Vec<u8> = bytes
                .iter()
                .map(|byte| {
                    let byte = pool
                        .evaluate(*byte, &HashMap::from([("v".to_owned(), v_bits)]))
                        .expect("a byte");
                    u8::try_from(byte).expect("a byte")
                })
                .collect();
            let (lo, hi) = range_near(&mut rng, i128::from(program));
            let rule = crate::expect::hole(
                crate::expect::Bound::Number(lo),
                crate::expect::Bound::Number(hi),
            );
            assert_eq!(
                holds(
                    &pool,
                    condition,
                    &[("v", v_bits), ("lo", bits(lo)), ("hi", bits(hi))]
                ),
                crate::expect::matches(&rule, &text, 0),
                "seed {seed}: {:?} (v = {program}) against any({lo}, {hi})",
                String::from_utf8_lossy(&text)
            );
        }
    }
}

#[test]
fn several_numbers_in_one_line_match_exactly() {
    // Three computed values with text between them: true only for the exact text.
    let seed = seed();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let template = Template::parse("{a} + {b} = {a + b}\n").expect("a template");
    for _ in 0..300 {
        let mut pool = Pool::default();
        let numbers = BTreeMap::from([
            ("a".to_owned(), formula(&mut pool, "a")),
            ("b".to_owned(), formula(&mut pool, "b")),
        ]);
        let (a, b) = (number(&mut rng) / 4, number(&mut rng) / 4);
        let exact = format!("{a} + {b} = {}\n", a + b);
        let text = match rng.random_range(0..6) {
            0 => format!("{} + {b} = {}\n", near(&mut rng, a), a + b),
            1 => format!("{a} + {} = {}\n", near(&mut rng, b), a + b),
            2 => format!("{a} + {b} = {}\n", near(&mut rng, a + b)),
            3 => format!("{a}+ {b} = {}\n", a + b),
            4 => format!("{b} + {a} = {}\n", a + b),
            _ => exact.clone(),
        };
        let bytes = bytes_of(&mut pool, &text);
        let known = super::Known {
            translated: std::cell::RefCell::default(),
            offsets: HashMap::new(),
            numbers: &numbers,
            fixed: &BTreeMap::new(),
            texts: &BTreeMap::new(),
        };
        let condition = text_is(&mut pool, &bytes, &template, &known).expect("a condition");
        assert_eq!(
            holds(&pool, condition, &[("a", bits(a)), ("b", bits(b))]),
            text == exact,
            "seed {seed}: {text:?} with a = {a}, b = {b}"
        );
    }
}

#[test]
fn a_chosen_text_matches_only_its_own_choice() {
    // Zero padding: `if(a < 10, "0", "")` before a number. True only for the exact text.
    let seed = seed();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let template = Template::parse("{if(a < 10, \"0\", \"\")}{a}:{if(b < 10, \"0\", \"\")}{b}\n")
        .expect("a template");
    for _ in 0..600 {
        let mut pool = Pool::default();
        let numbers = BTreeMap::from([
            ("a".to_owned(), formula(&mut pool, "a")),
            ("b".to_owned(), formula(&mut pool, "b")),
        ]);
        let (a, b) = (rng.random_range(-3..30_i128), rng.random_range(-3..30_i128));
        let pad = |value: i128| {
            if value < 10 {
                format!("0{value}")
            } else {
                value.to_string()
            }
        };
        let exact = format!("{}:{}\n", pad(a), pad(b));
        let text = match rng.random_range(0..5) {
            0 => format!("{a}:{}\n", pad(b)),
            1 => format!("{}:{b}\n", pad(a)),
            2 => format!("0{}:{}\n", pad(a), pad(b)),
            3 => format!("{}:{}\n", pad(a + 1), pad(b)),
            _ => exact.clone(),
        };
        let bytes = bytes_of(&mut pool, &text);
        let known = super::Known {
            translated: std::cell::RefCell::default(),
            offsets: HashMap::new(),
            numbers: &numbers,
            fixed: &BTreeMap::new(),
            texts: &BTreeMap::new(),
        };
        let condition = text_is(&mut pool, &bytes, &template, &known).expect("a condition");
        assert_eq!(
            holds(&pool, condition, &[("a", bits(a)), ("b", bits(b))]),
            text == exact,
            "seed {seed}: {text:?} with a = {a}, b = {b}"
        );
    }
}
