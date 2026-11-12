//! What a part's function gives, proven of the function once and used at each call of it.
//!
//! A part's contract may state what a function gives as a formula of its arguments
//! ([`crate::wasm::contract::Gives`]). A [`Claim`] is that statement held to the function's
//! text: each parameter's width and what the item says of it. [`settle`] proves a claim of the
//! function alone, as a unit: for every argument in the ranges the item takes, every byte the
//! item says an address holds, and every value of the other parameters, the function returns,
//! stops at no assertion and no trap, and gives the formula's value. A program's proofs then
//! take the formula's value at each call of a function it took from the part, in place of
//! following its body, where the call's arguments are in the claim's ranges
//! ([`crate::prove::wasm`]'s `unit`).
//!
//! Why the two are the same statement: [`value`] is the one translation of the formula to a
//! term, used by the unit's proof and at the call. Why a proof of the function alone holds at
//! every call: `check` holds a taken function to its part word for word, with every constant
//! and function it names, and a function a contract offers names no region and no global that
//! changes; the unit's own rules (`unit_problem`) hold that it has no store, no host call and
//! no use of an address but to read at it, plus a number that doesn't depend on it. So what
//! it gives depends only on its other arguments and on the bytes it reads through the
//! addresses, which the unit's run holds to the bytes the item names: a read past them ends
//! the proof. Nothing else of a run's state can reach it, and it changes nothing of that state.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use super::exec;
use super::formula::{self, Formula, Number, WIDTH};
use super::smt::Answer;
use super::term::{Pool, Term};
use super::{Solvers, Witnessed, wasm};
use crate::expr::Expr;
use crate::wasm::contract::Offer;
use crate::wasm::parts::Field;

/// What a claim says of one parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    /// A number the item takes from `low` to `high`: read with a sign where `low` is below 0.
    Number {
        /// Its least value.
        low: i128,
        /// Its most.
        high: i128,
    },
    /// An address that holds bytes the formula names: as many as the parameter `length` says,
    /// by its place among the parameters.
    Text {
        /// The place of the parameter that says how many bytes.
        length: usize,
    },
    /// Any value: the formula doesn't name it.
    Free,
}

/// One parameter of a claim's function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// Its name, without the `$`; empty where the function's text gives none.
    pub name: String,
    /// Its width in bits.
    pub width: u32,
    /// What the claim says of it.
    pub role: Role,
}

/// What a function gives, held to the function's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// The function's name, without the `$`.
    pub function: String,
    /// The part the program took the function from, as its idea names it; `None` for a
    /// function of the program's own contract.
    pub part: Option<String>,
    /// Its parameters, in order.
    pub params: Vec<Param>,
    /// Its result's width in bits.
    pub result: u32,
    /// The formula, as the item writes it.
    pub formula: String,
    /// The item's clauses that state it, as a reply shows them.
    pub stated: String,
    /// The formula, read.
    expr: Expr,
}

/// A value type's width, for the whole numbers a claim is about.
fn width_of(kind: &str) -> Option<u32> {
    match kind {
        "i32" => Some(32),
        "i64" => Some(64),
        _ => None,
    }
}

/// The parameters, each a name without its `$` and a type, and the results' types of the
/// function whose text is `body`, as [`Field::body`] holds it.
fn signature(body: &str) -> Result<(Vec<(String, String)>, Vec<String>), String> {
    let unread = || "lengua can't read the function's parameters".to_owned();
    let mut params = Vec::new();
    let mut results = Vec::new();
    // The fields right inside `(func …)`: its parameters and results come before anything else
    // but its name and an export.
    let inner = body.strip_prefix("(func").ok_or_else(unread)?;
    let mut rest = inner;
    while let Some(open) = rest.find('(') {
        // Only white space and the function's name stand before a field of the signature.
        let before = rest.get(..open).unwrap_or_default();
        if before
            .split_whitespace()
            .any(|word| !word.starts_with('$'))
        {
            break;
        }
        let field = rest.get(open..).unwrap_or_default();
        let close = field.find(')').ok_or_else(unread)?;
        let words: Vec<&str> = field
            .get(1..close)
            .unwrap_or_default()
            .split_whitespace()
            .collect();
        match words.split_first() {
            Some((&"param", [name, kind])) if name.starts_with('$') => {
                params.push((name.trim_start_matches('$').to_owned(), (*kind).to_owned()));
            }
            Some((&"param", kinds)) => {
                params.extend(kinds.iter().map(|kind| (String::new(), (*kind).to_owned())));
            }
            Some((&"result", kinds)) => {
                results.extend(kinds.iter().map(|kind| (*kind).to_owned()));
            }
            Some((&"export", _)) => {}
            _ => break,
        }
        rest = field.get(close + 1..).unwrap_or_default();
    }
    Ok((params, results))
}

impl Claim {
    /// What `offer` says its function gives, held to the function's text among main.wat's
    /// `fields`: `None` for an offer that states no formula.
    ///
    /// # Errors
    ///
    /// A problem's words: a function main.wat doesn't have, a parameter the function doesn't
    /// have or of another type than a whole number, a range the type doesn't hold, or a
    /// function that gives no whole number or more than one.
    pub fn of(offer: &Offer, fields: &[Field]) -> Option<Result<Self, String>> {
        let gives = offer.gives.as_ref()?;
        let name = &offer.name;
        let Some(field) = fields
            .iter()
            .find(|field| field.keyword == "func" && field.name == *name)
        else {
            return Some(Err(format!("main.wat has no function `${name}`")));
        };
        let (params, results) = match signature(&field.body) {
            Ok(signature) => signature,
            Err(problem) => return Some(Err(problem)),
        };
        let [result] = results.as_slice() else {
            return Some(Err(format!(
                "`${name}` gives {} values, and a `gives` clause is for a function that gives \
                 one whole number",
                results.len()
            )));
        };
        let Some(result) = width_of(result) else {
            return Some(Err(format!(
                "`${name}` gives a value of type {result}, and a `gives` clause is for a \
                 function that gives a whole number, i32 or i64"
            )));
        };
        let place = |wanted: &str| {
            params
                .iter()
                .position(|(param, _)| !param.is_empty() && param == wanted)
        };
        for stated in gives
            .takes
            .iter()
            .map(|(taken, ..)| taken)
            .chain(gives.holds.iter().flat_map(|(pointer, length)| [pointer, length]))
        {
            if place(stated).is_none() {
                return Some(Err(format!(
                    "`${name}` has no parameter `${stated}`: a `takes` and a `holds` clause name \
                     the function's parameters as main.wat names them"
                )));
            }
        }
        let mut held = Vec::new();
        for (param, kind) in &params {
            let taken = gives.takes.iter().find(|(taken, ..)| taken == param);
            let holding = gives.holds.iter().find(|(pointer, _)| pointer == param);
            let width = match (width_of(kind), kind.as_str()) {
                (Some(width), _) => width,
                (None, "f32") => 32,
                (None, "f64") => 64,
                (None, "v128") => 128,
                (None, _) => {
                    return Some(Err(format!(
                        "`${name}` has a parameter of type {kind}, which a proof of a function \
                         alone doesn't take"
                    )));
                }
            };
            let role = if let Some((_, length)) = holding.filter(|_| !param.is_empty()) {
                if kind != "i32" {
                    return Some(Err(format!(
                        "`${param}` holds bytes, so it is an address, an i32, not an {kind}"
                    )));
                }
                match place(length) {
                    Some(length) => Role::Text { length },
                    None => Role::Free,
                }
            } else if let Some((_, low, high)) = taken.filter(|_| !param.is_empty()) {
                let Some(bits) = width_of(kind) else {
                    return Some(Err(format!(
                        "`${param}` is taken in a range, so it is a whole number, i32 or i64, \
                         not an {kind}"
                    )));
                };
                let fits = if *low < 0 {
                    *low >= -(1_i128 << (bits - 1)) && *high < (1_i128 << (bits - 1))
                } else {
                    *high < (1_i128 << bits)
                };
                if !fits {
                    return Some(Err(format!(
                        "`${param}` is an {kind}, which doesn't hold every value from {low} to \
                         {high}"
                    )));
                }
                Role::Number {
                    low: *low,
                    high: *high,
                }
            } else {
                Role::Free
            };
            held.push(Param {
                name: param.clone(),
                width,
                role,
            });
        }
        Some(Ok(Self {
            function: name.clone(),
            part: None,
            params: held,
            result,
            formula: gives.formula.clone(),
            stated: gives.stated(),
            expr: gives.expr.clone(),
        }))
    }

    /// The places of the parameters that are addresses whose bytes the formula names.
    #[must_use]
    pub fn pointers(&self) -> Vec<usize> {
        self.params
            .iter()
            .enumerate()
            .filter(|(_, param)| matches!(param.role, Role::Text { .. }))
            .map(|(place, _)| place)
            .collect()
    }
}

/// The value `claim` says its function gives for `args`, the function's arguments in order, and
/// `texts`, the bytes each address parameter holds, by the parameter's name: the formula's
/// value in the result's bits, a negative value as its two's complement. The one translation of
/// a claim: the unit's proof holds the function to it, and a call takes it.
///
/// # Errors
///
/// A formula that isn't a whole number or true or false, or that the proof's formulas don't
/// take.
pub fn value(
    pool: &mut Pool,
    claim: &Claim,
    args: &[Term],
    texts: &BTreeMap<String, Vec<Term>>,
) -> Result<Term, String> {
    let mut numbers = BTreeMap::new();
    for (param, arg) in claim.params.iter().zip(args) {
        if let Role::Number { low, high } = param.role {
            let by = WIDTH.saturating_sub(param.width);
            let term = if low < 0 {
                pool.sign_extend(*arg, by)
            } else {
                pool.zero_extend(*arg, by)
            };
            numbers.insert(param.name.clone(), Number::whole(term, low, high));
        }
    }
    let wide = match formula::translate(pool, &claim.expr, &numbers, texts)? {
        Formula::Number(number) if number.places == 0 => number.term,
        Formula::Truth(holds) => {
            let (one, zero) = (pool.bv(WIDTH, 1), pool.bv(WIDTH, 0));
            pool.ite(holds, one, zero)
        }
        _ => {
            return Err(format!(
                "the formula `{}` gives no whole number, and no true or false",
                claim.formula
            ));
        }
    };
    Ok(pool.extract(wide, claim.result.saturating_sub(1), 0))
}

/// The most combinations of the lengths of a claim's texts a unit's proof takes, each a run of
/// the function of its own.
const MAX_LENGTHS: u128 = 4096;

/// The most paths one combination of lengths may take through the function.
const MAX_PATHS: usize = 256;

/// The longest one solver question of a unit's proof may take, in milliseconds.
const QUESTION_MS: u64 = 20_000;

/// The most time the claims of one build get, of the time its proofs share.
pub const UNIT_TIME: Duration = Duration::from_secs(40);

/// A claim as `check` settled it.
#[derive(Debug, Clone)]
pub struct Settled {
    /// The claim.
    pub claim: Claim,
    /// How it was proven, in words, or why it wasn't.
    pub proven: Result<String, String>,
}

impl Settled {
    /// The claim in one line of `check`'s reply.
    #[must_use]
    pub fn line(&self) -> String {
        let name = &self.claim.function;
        let stated = &self.claim.stated;
        let whose = self
            .claim
            .part
            .as_ref()
            .map_or_else(String::new, |part| format!(", from {part}"));
        match (&self.proven, &self.claim.part) {
            (Ok(how), Some(_)) => format!(
                "${name}{whose}: {stated}: proven of the function as this program holds it, \
                 {how}; this program's proofs take the formula's value at each call whose \
                 arguments are in those ranges, in place of the function's body, and follow \
                 the body at any other call"
            ),
            (Ok(how), None) => format!(
                "${name}: {stated}: proven of the function alone, {how}; the proofs of a \
                 program that takes the function use it in place of the function's body"
            ),
            (Err(why), Some(_)) => format!(
                "${name}{whose}: {stated}: not proven of the function alone ({why}), so this \
                 program's proofs follow the function's body at each call"
            ),
            (Err(why), None) => format!(
                "${name}: {stated}: tested, not proven of the function alone: {why}; the \
                 proofs of a program that takes the function follow its body"
            ),
        }
    }
}

/// Each combination of `ranges`, one value from each, the last changing fastest.
fn combinations(ranges: &[(u64, u64)]) -> Vec<Vec<u64>> {
    let mut found: Vec<Vec<u64>> = vec![Vec::new()];
    for (low, high) in ranges {
        let mut next = Vec::new();
        for chosen in &found {
            for value in *low..=*high {
                let mut longer = chosen.clone();
                longer.push(value);
                next.push(longer);
            }
        }
        found = next;
    }
    found
}

/// Proves `claim` of the function `index` of `module`, alone, by `deadline`: how, in words.
///
/// # Errors
///
/// Why the claim isn't proven for every argument in its ranges: what the function is that a
/// unit's proof doesn't take, a path that doesn't return, a trap or a value some argument
/// reaches, or the time running out, with how far the proof got.
fn prove_unit(
    module: &wasm::Module,
    index: u32,
    claim: &Claim,
    deadline: Instant,
) -> Result<String, String> {
    if let Some(problem) = module.unit_problem(index, claim) {
        return Err(problem);
    }
    // Each text's length, from its range: one run of the function for each combination.
    let texts: Vec<(usize, usize)> = claim
        .params
        .iter()
        .enumerate()
        .filter_map(|(place, param)| match param.role {
            Role::Text { length } => Some((place, length)),
            _ => None,
        })
        .collect();
    let mut lengths: Vec<usize> = texts.iter().map(|(_, length)| *length).collect();
    lengths.sort_unstable();
    lengths.dedup();
    let mut ranges = Vec::new();
    let mut count: u128 = 1;
    for length in &lengths {
        let Some(Role::Number { low, high }) = claim.params.get(*length).map(|param| &param.role)
        else {
            return Err("a text whose length the claim takes in no range".into());
        };
        let (Ok(low), Ok(high)) = (u64::try_from(*low), u64::try_from(*high)) else {
            return Err("a text of a length below zero".into());
        };
        count = count.saturating_mul(u128::from(high - low) + 1);
        ranges.push((low, high));
    }
    if count > MAX_LENGTHS {
        return Err(format!(
            "its texts have {count} combinations of lengths, more than the {MAX_LENGTHS} a proof \
             of a function alone takes: take the lengths in smaller ranges"
        ));
    }
    let mut solvers = Solvers::new(QUESTION_MS);
    let mut paths = 0_usize;
    let mut questions = 0_usize;
    let mut alone: Option<String> = None;
    let all = combinations(&ranges);
    let total = all.len();
    for (done, chosen) in all.into_iter().enumerate() {
        let short = |why: &str| {
            if total > 1 {
                format!("{why}, with {done} of {total} combinations of lengths proven")
            } else {
                why.to_owned()
            }
        };
        if Instant::now() > deadline {
            return Err(short(super::OUT_OF_TIME));
        }
        let length_of = |place: usize| {
            lengths
                .iter()
                .position(|length| *length == place)
                .and_then(|at| chosen.get(at))
                .copied()
        };
        solvers.next();
        let mut pool = Pool::default();
        let mut args = Vec::new();
        let mut held: BTreeMap<String, Vec<Term>> = BTreeMap::new();
        let mut laid: Vec<(u64, Vec<Term>)> = Vec::new();
        let mut conditions = Vec::new();
        // Where the next text stands: past the library's own first bytes, each text after the
        // last with a gap.
        let mut next_address = wasm::UNIT_TEXTS_FROM;
        for (place, param) in claim.params.iter().enumerate() {
            let name = if param.name.is_empty() {
                format!("argument{place}")
            } else {
                param.name.clone()
            };
            let arg = match param.role {
                Role::Number { low, high } => {
                    if let Some(length) = length_of(place) {
                        pool.bv(param.width, u128::from(length))
                    } else if low < 0 {
                        let arg = pool.var(param.width, &name);
                        let (least, most) = (
                            pool.bv_signed(param.width, low),
                            pool.bv_signed(param.width, high),
                        );
                        conditions.extend([pool.sle(least, arg), pool.sle(arg, most)]);
                        arg
                    } else {
                        let (low, high) = (low.unsigned_abs(), high.unsigned_abs());
                        let arg = pool.var_in(param.width, &name, low, high);
                        let (least, most) =
                            (pool.bv(param.width, low), pool.bv(param.width, high));
                        conditions.extend([pool.ule(least, arg), pool.ule(arg, most)]);
                        arg
                    }
                }
                Role::Text { length } => {
                    let bytes = length_of(length).unwrap_or(0);
                    let text: Vec<Term> = (0..bytes)
                        .map(|at| pool.var(8, &format!("{name}.{at}")))
                        .collect();
                    let address = next_address;
                    next_address = address.saturating_add(bytes).saturating_add(wasm::UNIT_GAP);
                    held.insert(param.name.clone(), text.clone());
                    laid.push((address, text));
                    pool.bv(32, u128::from(address))
                }
                Role::Free => pool.var(param.width, &name),
            };
            args.push(arg);
        }
        let Some(mut start) = wasm::unit_begin(module, &mut pool, index, &args, &laid) else {
            return Err("the module's memory has no room for the texts of a unit's proof".into());
        };
        start.world.path = conditions;
        let expected = value(&mut pool, claim, &args, &held)?;
        let hard = deadline.max(Instant::now() + super::MIN_SHAPE_TIME);
        let runs = {
            let mut oracle = Witnessed::new(&mut solvers, (deadline, hard), done as u64, false);
            exec::explore(module, start, &mut pool, &mut oracle, MAX_PATHS)
                .map_err(|why| short(&why))?
        };
        paths += runs.len();
        for run in &runs {
            if Instant::now() > hard {
                return Err(short(super::OUT_OF_TIME));
            }
            // Whether some argument takes this path with `more` holding as well.
            let mut reached = |pool: &mut Pool, more: Option<Term>| -> Result<bool, String> {
                let mut conditions = run.world.path.clone();
                conditions.extend(more);
                // Nothing to meet: every argument takes it.
                if conditions
                    .iter()
                    .all(|condition| pool.truth(*condition) == Some(true))
                {
                    return Ok(true);
                }
                let solver = solvers.get(deadline)?;
                let (answer, _) = solver
                    .check(pool, &conditions, &[])
                    .map_err(|error| format!("the solver failed: {error}"))?;
                match answer {
                    Answer::Unsat => Ok(false),
                    Answer::Sat => Ok(true),
                    Answer::Unknown if solver.out_of_time() => Err(super::OUT_OF_TIME.to_owned()),
                    Answer::Unknown => {
                        Err(format!("a path couldn't be settled: {}", solver.why_unknown))
                    }
                }
            };
            let given = match wasm::unit_end(module, &mut pool, run) {
                Ok(given) => given,
                // A path that doesn't return must be one no argument takes.
                Err(why) => {
                    if reached(&mut pool, None).map_err(|why| short(&why))? {
                        return Err(why);
                    }
                    continue;
                }
            };
            for (condition, problem) in &run.world.obligations {
                if pool.truth(*condition) == Some(false) {
                    continue;
                }
                if reached(&mut pool, Some(*condition)).map_err(|why| short(&why))? {
                    return Err(format!("for some argument in its ranges, {problem}"));
                }
            }
            let same = pool.eq(given, expected);
            if pool.truth(same) == Some(true) {
                continue;
            }
            let differs = pool.bool_not(same);
            if reached(&mut pool, Some(differs)).map_err(|why| short(&why))? {
                return Err(format!(
                    "for some argument in its ranges the function gives another value than \
                     `{}`",
                    claim.formula
                ));
            }
        }
        if let Some(solver) = solvers.solver.as_ref() {
            questions = questions.max(solver.questions);
            if alone.is_none() && solvers.used {
                alone.clone_from(&solver.alone);
            }
        }
        if pool.out_of_room() {
            return Err(super::watch::OUT_OF_MEMORY.into());
        }
    }
    let over = if total > 1 {
        format!("{total} combinations of lengths and {paths} paths")
    } else {
        format!("{paths} paths")
    };
    Ok(match (questions, alone) {
        (0, _) => format!("for every argument in its ranges: {over}, settled by the terms"),
        (_, None) => format!(
            "for every argument in its ranges: {over}, each question confirmed by two solvers"
        ),
        (_, Some(reason)) => {
            format!("for every argument in its ranges: {over}, by one solver alone: {reason}")
        }
    })
}

/// Settles each of `claims` about the program `built`, whose idea lets it reach what `policy`
/// allows: each proven of its function alone, the smallest share of the time to `deadline`
/// each, or why not.
#[must_use]
pub fn settle(
    built: &crate::wasm::Built,
    policy: crate::wasm::Policy,
    claims: &[Claim],
    deadline: Instant,
) -> Vec<Settled> {
    if claims.is_empty() {
        return Vec::new();
    }
    let module = wasm::Module::new(built, policy);
    let mut settled = Vec::new();
    for (turn, claim) in claims.iter().enumerate() {
        let left = u32::try_from(claims.len() - turn).unwrap_or(u32::MAX).max(1);
        let now = Instant::now();
        let share = deadline.saturating_duration_since(now) / left;
        let proven = match &module {
            Err(why) => Err(why.clone()),
            Ok(module) => match module.own_function(&claim.function) {
                None => Err(format!("main.wat has no function ${}", claim.function)),
                Some(index) => prove_unit(module, index, claim, (now + share).min(deadline)),
            },
        };
        settled.push(Settled {
            claim: claim.clone(),
            proven,
        });
    }
    settled
}

/// The claims among `settled` a program's proofs use at the calls of its functions: the proven
/// ones about functions it took from a part.
#[must_use]
pub fn usable(settled: &[Settled]) -> Vec<Claim> {
    settled
        .iter()
        .filter(|settled| settled.proven.is_ok() && settled.claim.part.is_some())
        .map(|settled| settled.claim.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_function_s_parameters_from_its_text() {
        let body = "(func $crc32_update (param $crc i32) (param $at i32) (param $length i32) \
                    (result i32) (local $i i32) (i32.const 0))";
        let (params, results) = signature(body).expect("a signature");
        let named: Vec<(&str, &str)> = params
            .iter()
            .map(|(name, kind)| (name.as_str(), kind.as_str()))
            .collect();
        assert_eq!(named, [("crc", "i32"), ("at", "i32"), ("length", "i32")]);
        assert_eq!(results, ["i32"]);
        // An export before them, parameters with no name, and a block's own type further in.
        let body = "(func $f (export \"f\") (param i32 i64) (result i64) (block (param i32) \
                    (result i32) (nop)))";
        let (params, results) = signature(body).expect("a signature");
        assert_eq!(params.len(), 2, "{params:?}");
        assert!(params.iter().all(|(name, _)| name.is_empty()));
        assert_eq!(results, ["i64"]);
        assert!(signature("(global $x i32 (i32.const 0))").is_err());
    }

    #[test]
    fn lays_out_each_combination_of_lengths() {
        assert_eq!(combinations(&[]), [Vec::<u64>::new()]);
        assert_eq!(
            combinations(&[(0, 1), (2, 3)]),
            [vec![0, 2], vec![0, 3], vec![1, 2], vec![1, 3]]
        );
    }

    /// A module with functions a contract could say something of: `$double`, which doubles a
    /// number up to 1000 and gives 7 past it; `$checked`, which stops the program past 1000;
    /// `$same`, whether two texts of up to 4 bytes are the same; `$counts`, which changes a
    /// global; `$aligned`, which computes with an address; and `$second`, which reads the byte
    /// after the one it is given. `_start` calls `$double` and `$same`, each inside and outside
    /// the ranges the contract takes them in, and exits with what they gave.
    const MODULE: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (global $LIMIT i32 (i32.const 1000))
  (global $seen (mut i32) (i32.const 0))
  (data (i32.const 1024) "abcd")
  (data (i32.const 2048) "abcx")
  (func $double (param $n i32) (result i32)
    (if (result i32) (i32.gt_u (local.get $n) (global.get $LIMIT))
      (then (i32.const 7))
      (else (i32.shl (local.get $n) (i32.const 1)))))
  (func $checked (param $n i32) (result i32)
    (if (i32.gt_u (local.get $n) (global.get $LIMIT))
      (then (call $assert_fail (i32.const 1)) unreachable))
    (i32.add (local.get $n) (local.get $n)))
  (func $same (param $a i32) (param $b i32) (param $length i32) (result i32)
    (local $i i32)
    (local $left i32)
    (local $result i32)
    (local.set $result (i32.const 1))
    (block $done
      (local.set $left (i32.const 4))
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (local.get $length)))
        (if (i32.ne (i32.load8_u (i32.add (local.get $a) (local.get $i)))
                    (i32.load8_u (i32.add (local.get $b) (local.get $i))))
          (then (local.set $result (i32.const 0)) (br $done)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $next (local.get $left))))
    (local.get $result))
  (func $counts (param $n i32) (result i32)
    (global.set $seen (i32.add (global.get $seen) (i32.const 1)))
    (local.get $n))
  (func $aligned (param $at i32) (param $length i32) (result i32)
    (i32.and (local.get $at) (i32.const 3)))
  (func $second (param $at i32) (param $length i32) (result i32)
    (i32.load8_u (i32.add (local.get $at) (i32.const 1))))
  (func $main (export "_start")
    (call $proc_exit
      (i32.add
        (i32.add
          (call $double (i32.const 5))
          (call $double (i32.const 2000)))
        (i32.add
          (i32.add
            (i32.shl (call $same (i32.const 1024) (i32.const 2048) (i32.const 3)) (i32.const 6))
            (i32.shl (call $same (i32.const 1024) (i32.const 2048) (i32.const 4)) (i32.const 5)))
          (i32.shl (call $same (i32.const 1024) (i32.const 2048) (i32.const 5)) (i32.const 4)))))
    unreachable))
"#;

    /// The claims the contract `items` states of [`MODULE`]'s functions, as a program that took
    /// them from a part holds them.
    fn claims(items: &str) -> Vec<Claim> {
        let contract = crate::wasm::contract::read(&format!("# Contract\n{items}\n"))
            .unwrap_or_else(|problems| panic!("{problems:?}"))
            .expect("a contract");
        let fields = crate::wasm::parts::fields(MODULE);
        contract
            .offers
            .iter()
            .map(|offer| {
                let mut claim = Claim::of(offer, &fields)
                    .expect("a formula")
                    .unwrap_or_else(|problem| panic!("{problem}"));
                claim.part = Some("part".to_owned());
                claim
            })
            .collect()
    }

    /// What `check` settles of the claims `items` states, in a minute.
    fn settled(items: &str) -> Vec<Settled> {
        let built = crate::wasm::text::build(MODULE).unwrap_or_else(|problem| panic!("{problem}"));
        let deadline = Instant::now() + Duration::from_secs(60);
        settle(
            &built,
            crate::wasm::Policy::default(),
            &claims(items),
            deadline,
        )
    }

    /// Whether z3 is installed, which a unit's proof asks: a test without it says so and passes.
    fn solvers_installed() -> bool {
        let installed = std::process::Command::new("z3")
            .arg("--version")
            .output()
            .is_ok();
        if !installed {
            crate::skiplog::note("z3 isn't installed");
        }
        installed
    }

    const DOUBLE: &str = "- `$double`: uses no memory; takes `$n` from 0 to 1000; gives `n * 2`.";
    const SAME: &str = "- `$same`: reads 4 bytes at `$a`; reads 4 bytes at `$b`; takes `$length` \
                        from 0 to 4; `$a` holds `$length` bytes; `$b` holds `$length` bytes; gives \
                        `if(a == b, 1, 0)`.";

    #[test]
    fn a_function_is_proven_alone_for_every_argument_in_its_ranges() {
        if !solvers_installed() {
            return;
        }
        let found = settled(&format!("{DOUBLE}\n{SAME}"));
        assert_eq!(found.len(), 2);
        for settled in &found {
            let how = settled
                .proven
                .as_ref()
                .unwrap_or_else(|why| panic!("{}: {why}", settled.claim.function));
            assert!(how.starts_with("for every argument in its ranges"), "{how}");
            assert!(settled.line().contains("in place of the function's body"));
        }
        assert_eq!(usable(&found).len(), 2);
    }

    #[test]
    fn a_claim_that_doesn_t_hold_for_every_argument_is_not_proven() {
        if !solvers_installed() {
            return;
        }
        let why = |items: &str| {
            let found = settled(items);
            assert!(usable(&found).is_empty(), "{found:?}");
            found[0].proven.clone().expect_err("not proven")
        };
        // A formula the function doesn't give, and one it gives only in a smaller range.
        let wrong = why("- `$double`: uses no memory; takes `$n` from 0 to 1000; gives `n * 3`.");
        assert!(wrong.contains("another value than `n * 3`"), "{wrong}");
        let past = why("- `$double`: uses no memory; takes `$n` from 0 to 1001; gives `n * 2`.");
        assert!(past.contains("another value than `n * 2`"), "{past}");
        // An assertion an argument in range reaches.
        let stops = why("- `$checked`: uses no memory; takes `$n` from 0 to 1001; gives `n * 2`.");
        assert!(stops.contains("an assertion of the function stops"), "{stops}");
        // A text the formula says less of than the function reads.
        let same = why("- `$same`: reads 4 bytes at `$a`; reads 4 bytes at `$b`; takes `$length` \
                        from 0 to 4; `$a` holds `$length` bytes; `$b` holds `$length` bytes; gives \
                        `1`.");
        assert!(same.contains("another value than `1`"), "{same}");
        let outside = why("- `$second`: reads 2 bytes at `$at`; takes `$length` from 0 to 1; `$at` \
                           holds `$length` bytes; gives `len(at)`.");
        assert!(outside.contains("reads outside the bytes"), "{outside}");
    }

    #[test]
    fn a_function_that_breaks_a_unit_s_rules_is_not_proven_alone() {
        let why = |items: &str| settled(items)[0].proven.clone().expect_err("not proven");
        let counts = why("- `$counts`: uses no memory; takes `$n` from 0 to 9; gives `n`.");
        assert!(counts.contains("$counts"), "{counts}");
        assert!(counts.contains("global"), "{counts}");
        let aligned = why("- `$aligned`: reads 1 byte at `$at`; takes `$length` from 0 to 1; `$at` \
                           holds `$length` bytes; gives `0`.");
        assert!(aligned.contains("computes with an address"), "{aligned}");
    }

    #[test]
    fn a_call_takes_a_claim_s_value_only_inside_its_ranges() {
        let built = crate::wasm::text::build(MODULE).unwrap_or_else(|problem| panic!("{problem}"));
        let policy = crate::wasm::Policy::default();
        let given = wasm::Given::default();
        // With no claim, every call follows its function: 10 and 7, then 1, 0 and 0.
        let plain = wasm::Module::new(&built, policy).expect("a module");
        let wanted = wasm::Ended::Exit(10 + 7 + 64);
        assert_eq!(wasm::run(&plain, &given).expect("a run").end, wanted);
        assert!(plain.claims_taken().is_empty());
        // With the claims, the calls inside their ranges take the formulas' values, and the
        // call of `$double` with 2000 and of `$same` with 5 bytes follow the bodies, which give
        // what the formulas would not.
        let module = wasm::Module::new(&built, policy)
            .expect("a module")
            .with_claims(&claims(&format!("{DOUBLE}\n{SAME}")));
        assert_eq!(wasm::run(&module, &given).expect("a run").end, wanted);
        let mut taken = module.claims_taken();
        taken.sort();
        assert_eq!(
            taken,
            [("double".to_owned(), 1, 1), ("same".to_owned(), 2, 1)]
        );
        // A claim about a function that breaks a unit's rules is never taken.
        let refused = wasm::Module::new(&built, policy)
            .expect("a module")
            .with_claims(&claims(
                "- `$counts`: uses no memory; takes `$n` from 0 to 9; gives `n`.",
            ));
        assert!(refused.claims_taken().is_empty());
    }

    /// A claim whose proof ran out of time part of the way is proven only to a length: no
    /// program's proofs use it.
    #[test]
    fn a_claim_proven_only_to_a_length_is_not_used() {
        let built = crate::wasm::text::build(MODULE).unwrap_or_else(|problem| panic!("{problem}"));
        let now = Instant::now();
        let past = now.checked_sub(Duration::from_secs(1)).unwrap_or(now);
        let found = settle(&built, crate::wasm::Policy::default(), &claims(SAME), past);
        let why = found[0].proven.clone().expect_err("not proven");
        assert!(why.contains("ran out of time"), "{why}");
        assert!(why.contains("of 5 combinations of lengths proven"), "{why}");
        assert!(usable(&found).is_empty());
        assert!(
            found[0]
                .line()
                .contains("this program's proofs follow the function's body at each call"),
            "{}",
            found[0].line()
        );
    }
}
