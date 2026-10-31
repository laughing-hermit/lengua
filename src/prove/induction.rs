//! Proof by induction over a streaming program's read loop. A program that reads standard
//! input a buffer-full at a time through fill, and either writes each buffer-full's bytes mapped
//! one by one, as `upper`, `lower` and `tr` map them, or keeps counts that add up over the
//! buffer-fulls, as `len` and `count` of one character do, and writes them at the end, is
//! proven for every length in two parts:
//!
//! - the step: from any state the loop can be in at its head with more than a buffer-full
//!   left, one time round reads exactly a buffer-full, writes the formula's bytes for it (or,
//!   for counts, nothing, each count grown by the buffer-full's), and comes back to the head in
//!   a state of the same kind, its bound counters one lower;
//! - the base: from any such state, every input of up to a buffer-full gives the formula's
//!   output and exit code, each count going on from what came before the head, proven as any
//!   text is, length by length.
//!
//! So an input of any length goes round the loop a buffer-full at a time until a buffer-full or
//! less is left, which the base covers. The states the loop can be in are found as a fixpoint:
//! what one time round changes, from the first time at the head and then from the head with all
//! that made arbitrary, until nothing new changes; everything else stays as it was the first
//! time. A count is kept in a register that went from a constant to it and the buffer-full's
//! count, and goes on adding from any count so far.
//!
//! A count whose buffer-full's part depends on the bytes before it, as `words` does on whether
//! the last buffer-full ended inside a word, is checked a buffer-full later: the step reads two
//! buffer-fulls from any state at the head, and the second's count must be what the formula
//! adds for it after the first's last bytes. Whatever the program keeps of what came before, a
//! flag, a byte or a mask's bit, its count must come out so, which needs no telling where it
//! keeps it. The base likewise reads a buffer-full from any state at the head before each final
//! piece, and the shortest inputs are proven from the program's start.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use super::exec::induction::{Fold, Hook};
use super::exec::{self, End, Image, State};
use super::formula::{self, Formula, WIDTH};
use super::smt::Answer;
use super::term::{Pool, Term};
use super::{
    Budget, Known, MAX_CHOICES, Solvers, Witnessed, conjuncts, disjuncts, settled, text_is,
};
use crate::expr::{Expr, Function, Generator, Piece, Value};
use crate::testfile::{ExitCode, Generated};

/// What a proof by induction found, once its step holds.
#[derive(Debug, Clone)]
pub struct Found {
    /// How the base's runs take the read loop's head.
    pub hook: Hook,
    /// The bytes one time round the loop reads.
    pub chunk: usize,
    /// The longest input the induction covers: as many times round as the loop's bound allows.
    pub reach: u128,
    /// Paths the step followed.
    pub paths: usize,
    /// Questions its solvers answered.
    pub questions: usize,
    /// The bytes before each buffer-full its counts need: none for a map, or for counts that
    /// add up as they are.
    pub lookback: usize,
}

/// Why a proof by induction doesn't hold, with an input that may show it.
#[derive(Debug, Clone)]
pub struct Refused {
    /// Why.
    pub why: String,
    /// The buffer-fulls a step that went wrong read, from the solver's model: run from the
    /// program's start, they may show it wrong.
    pub input: Option<Vec<u8>>,
}

impl From<String> for Refused {
    fn from(why: String) -> Self {
        Self { why, input: None }
    }
}

impl From<&str> for Refused {
    fn from(why: &str) -> Self {
        Self {
            why: why.to_owned(),
            input: None,
        }
    }
}

/// Rounds of finding what a time round the loop changes before giving up on a fixpoint.
const MAX_ROUNDS: usize = 4;

/// The text a block streams through its read loop.
struct Streamed<'a> {
    /// The variable's name.
    name: &'a str,
    /// Its characters.
    characters: Vec<u8>,
    /// The longest the spec lets it be.
    max: usize,
    /// What the program writes of it.
    output: Output<'a>,
    /// The bytes before each buffer-full its counts need.
    lookback: usize,
}

/// What a streaming program writes.
enum Output<'a> {
    /// Each buffer-full's bytes mapped one by one, as it goes.
    Map,
    /// Counts that add up over the buffer-fulls, written at the end: each placeholder's place
    /// among the template's pieces, with its formula.
    Counts(Vec<(usize, &'a Expr)>),
}

/// The text `block` streams, when it's one the induction applies to: a program whose standard
/// input is one text, whose output is that text mapped byte by byte or counts of it that add
/// up over its pieces, and which takes nothing else in.
fn streamed<'a>(image: &Image, block: &'a Generated) -> Result<Streamed<'a>, String> {
    if !image.has_fill() {
        return Err("it doesn't read with fill".into());
    }
    let [(name, Generator::Str { charset, max, .. })] = block.vars.as_slice() else {
        return Err("its input isn't one text".into());
    };
    if !charset.iter().all(char::is_ascii) {
        return Err("its text has characters beyond ASCII".into());
    }
    let [Piece::Expr(Expr::Var(read))] = block.stdin.pieces()[..] else {
        return Err("its standard input isn't the text alone".into());
    };
    if read != name {
        return Err("its standard input isn't the text alone".into());
    }
    let pieces = block.stdout.pieces();
    let (output, lookback) = match pieces[..] {
        [Piece::Expr(mapped)] if byte_map(mapped, name) => (Output::Map, 0),
        _ => {
            let counts: Vec<(usize, &Expr)> = pieces
                .iter()
                .enumerate()
                .filter_map(|(place, piece)| match piece {
                    Piece::Expr(expr) => Some((place, *expr)),
                    Piece::Text(_) => None,
                })
                .collect();
            let lookbacks: Option<Vec<usize>> = counts
                .iter()
                .map(|(_, expr)| formula::lookback(expr, name))
                .collect();
            let lookback = lookbacks.and_then(|lookbacks| lookbacks.into_iter().max());
            let Some(lookback) = lookback.filter(|_| !counts.is_empty()) else {
                return Err(
                    "its output isn't the text mapped byte by byte, as upper, lower and \
                     tr map it, nor counts that add up over its pieces after the bytes just \
                     before each, as len, count and words do"
                        .into(),
                );
            };
            (Output::Counts(counts), lookback)
        }
    };
    let others = !block.args.is_empty()
        || !block.env.is_empty()
        || !block.files.is_empty()
        || !block.folders.is_empty()
        || !block.files_after.is_empty()
        || !block.gone.is_empty()
        || !block.folders_after.is_empty()
        || block.random.is_some()
        || !block.clock.is_empty()
        || !block.server.is_empty()
        || !block.client.is_empty()
        || block.serves
        || !block.stderr.is_empty()
        || !matches!(block.exit_code, ExitCode::Fixed(_));
    if others {
        return Err("it takes in more than its standard input, or its exit code varies".into());
    }
    Ok(Streamed {
        name,
        characters: charset.iter().map(|character| *character as u8).collect(),
        max: *max,
        output,
        lookback,
    })
}

/// `expr`, a count, over `bytes`.
fn counted(pool: &mut Pool, expr: &Expr, name: &str, bytes: &[Term]) -> Result<Term, String> {
    let texts = BTreeMap::from([(name.to_owned(), bytes.to_vec())]);
    match formula::translate(pool, expr, &BTreeMap::new(), &texts)? {
        Formula::Number(number) => Ok(number.term),
        _ => Err("a count that isn't a number".into()),
    }
}

/// What `expr`, a count, adds for `piece` after `window`, the bytes just before it.
fn added(
    pool: &mut Pool,
    expr: &Expr,
    name: &str,
    window: &[Term],
    piece: &[Term],
) -> Result<Term, String> {
    let texts = BTreeMap::from([(name.to_owned(), piece.to_vec())]);
    match formula::delta(pool, expr, &texts, window)? {
        Formula::Number(number) => Ok(number.term),
        _ => Err("a count that isn't a number".into()),
    }
}

/// The run's standard input held to `characters`, byte by byte: the facts a question that
/// finds inputs is asked again with, so its inputs are ones the spec can give.
fn held(pool: &mut Pool, run: &State, characters: &[u8]) -> Vec<Term> {
    run.stdin_bytes()
        .iter()
        .map(|byte| pool.is_one_of(*byte, characters))
        .collect()
}

/// Whether a register grows as a count does.
enum Grows {
    /// It holds what it held and the count.
    Yes,
    /// It doesn't, and where the solvers found an input that shows it, the bytes `read` are.
    No(Option<Vec<u8>>),
}

/// Whether the run's `register`, which held `before`, now holds `before` and `count`: the
/// terms say so, or the solvers find no input with `held` that takes the run where it doesn't.
/// Where they find one, its bytes `read` come with the answer.
#[allow(clippy::too_many_arguments)]
fn grows(
    pool: &mut Pool,
    solvers: &mut Solvers,
    budget: &Budget,
    run: &State,
    register: usize,
    before: Term,
    count: Term,
    held: &[Term],
    read: &[Term],
) -> Result<Grows, String> {
    let now = pool.zero_extend(run.register_term(register), WIDTH - 64);
    let before = pool.zero_extend(before, WIDTH - 64);
    let expected = pool.add(before, count);
    let same = pool.eq(now, expected);
    match pool.truth(same) {
        Some(true) => return Ok(Grows::Yes),
        Some(false) => return Ok(Grows::No(None)),
        None => {}
    }
    let mut conditions = run.path.clone();
    conditions.push(pool.bool_not(same));
    let solver = solvers.get(budget.deadline)?;
    let (answer, model) = solver
        .check(pool, &conditions, read)
        .map_err(|error| format!("the solver failed: {error}"))?;
    let (answer, model) = super::within(pool, solver, answer, model, conditions, held, read)
        .map_err(|stop| match stop {
            super::Stop::Unproven(why) | super::Stop::Beyond(why) => why,
            super::Stop::Counterexample(_) => "a counterexample".into(),
        })?;
    Ok(match answer {
        Answer::Unsat => Grows::Yes,
        Answer::Sat => Grows::No(
            read.iter()
                .map(|byte| model.get(byte).map(|value| *value as u8))
                .collect::<Option<Vec<u8>>>()
                .filter(|input| !input.is_empty()),
        ),
        Answer::Unknown => Grows::No(None),
    })
}

/// Whether `expr` maps the text `name` byte by byte, each byte's output a byte that depends on
/// it alone: the text itself, or it through `upper`, `lower` and `tr` with fixed characters.
fn byte_map(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Var(var) => var == name,
        Expr::Call(Function::Upper | Function::Lower, arguments) => {
            matches!(arguments.as_slice(), [inner] if byte_map(inner, name))
        }
        Expr::Call(Function::Tr, arguments) => matches!(
            arguments.as_slice(),
            [inner, Expr::Literal(Value::Str(_)), Expr::Literal(Value::Str(_))]
                if byte_map(inner, name)
        ),
        _ => false,
    }
}

/// The runs from `start`, as a proof's shapes explore them: a branch no witness reaches is
/// followed without asking, since each run's end is asked about anyway.
fn explore(
    image: &Image,
    start: State,
    pool: &mut Pool,
    solvers: &mut Solvers,
    budget: &Budget,
) -> Result<Vec<State>, String> {
    let careful = start.clone();
    let (runs, guessed) = {
        let mut oracle = Witnessed::new(solvers, budget.deadline, 0, true);
        let runs = exec::explore(image, start, pool, &mut oracle, budget.max_paths)?;
        (runs, oracle.guessed)
    };
    if guessed && runs.iter().any(|run| run.end == Some(End::TooManyPaths)) {
        let mut oracle = Witnessed::new(solvers, budget.deadline, 0, false);
        return exec::explore(image, careful, pool, &mut oracle, budget.max_paths);
    }
    Ok(runs)
}

/// Proves the step of `block`'s induction: finds the states its read loop can be in at its
/// head, and shows one more time round keeps it in them and writes what the formula says.
///
/// # Errors
///
/// Why the induction doesn't apply or its step doesn't hold, with the input a step that went
/// wrong read: the proof then goes by length.
pub fn step(image: &Image, block: &Generated, budget: &Budget) -> Result<Found, Refused> {
    let streamed = streamed(image, block)?;
    let head = image.fill_head().ok_or("it has no fill")?;
    let looks_back = streamed.lookback > 0;
    let mut solvers = Solvers::new(budget.question_ms);
    let mut hook = Hook {
        head,
        low: 3,
        stop: true,
        feed: Some(streamed.characters.clone()),
        ..Hook::default()
    };
    let mut paths = 0;
    // The bytes a run read where a count stopped growing as the formula's does: from the
    // program's start, they may show it wrong.
    let mut shown: Option<Vec<u8>> = None;
    for round in 0..MAX_ROUNDS {
        if budget.stopped() {
            return Err(super::OUT_OF_TIME.into());
        }
        solvers.next();
        let mut pool = Pool::default();
        let start = State::start(image, &mut pool, &[], &[], Vec::new())
            .for_every_cut()
            .with_hook(Rc::new(hook.clone()));
        let runs = explore(image, start, &mut pool, &mut solvers, budget)?;
        paths += runs.len();
        let mut grown = hook.clone();
        let mut chunk = None;
        // For counts that look back: the registers that went from a constant to it and the
        // first buffer-full's count on every run.
        let mut candidates: Option<BTreeMap<usize, Fold>> = None;
        for run in &runs {
            if run.end != Some(End::Looped) {
                continue;
            }
            let snapshot = run
                .at_head()
                .ok_or("a run came back to a head it never left")?;
            // What one buffer-full changes: to the run's end, or to its second time at the
            // head where it reads two.
            let (changes, taken) = if hook.fulls() > 1 {
                let second = run
                    .at_second()
                    .ok_or("a run read two buffer-fulls without coming back between them")?;
                let changes = second
                    .changes
                    .clone()
                    .ok_or("a run's first buffer-full left no changes")?;
                (changes, second.stdin_read - snapshot.stdin_read)
            } else {
                (
                    run.changes(snapshot),
                    run.stdin_taken() - snapshot.stdin_read,
                )
            };
            if let Some(why) = changes.other.first() {
                return Err(why.clone().into());
            }
            let held = held(&mut pool, run, &streamed.characters);
            let fed = run.stdin_bytes()[snapshot.stdin_read..].to_vec();
            // A bound counter goes from its start one lower each time round; a count, from its
            // start, by what the buffer-full adds.
            if round == 0 {
                for register in &changes.registers {
                    let then = pool.constant(snapshot.registers[*register]);
                    let now = pool.constant(run.register_term(*register));
                    if let (Some(then), Some(now)) = (then, now)
                        && now.wrapping_add(1) == then
                        && then >= 3
                    {
                        grown.counters.insert(*register, then);
                    }
                }
                if let Output::Counts(counts) = &streamed.output
                    && (looks_back || grown.folds.is_empty())
                {
                    // Every register that went from a constant to it and the buffer-full's
                    // count may keep the count: a later round keeps only those that go on
                    // adding, as a register that holds only the last buffer-full's doesn't.
                    let first = &fed[..taken.min(fed.len())];
                    let mut these = BTreeMap::new();
                    for (placeholder, expr) in counts {
                        let count = counted(&mut pool, expr, streamed.name, first)?;
                        for register in &changes.registers {
                            let before = snapshot.registers[*register];
                            if let Some(start) = pool.constant(before)
                                && !grown.counters.contains_key(register)
                                && !these.contains_key(register)
                                && matches!(
                                    grows(
                                        &mut pool,
                                        &mut solvers,
                                        budget,
                                        run,
                                        *register,
                                        before,
                                        count,
                                        &held,
                                        &[],
                                    )?,
                                    Grows::Yes
                                )
                            {
                                these.insert(
                                    *register,
                                    Fold {
                                        start,
                                        most: 0,
                                        placeholder: *placeholder,
                                    },
                                );
                            }
                        }
                    }
                    if looks_back {
                        // The first buffer-full's count is checked here alone, so it must hold
                        // on every run.
                        candidates = Some(match candidates {
                            None => these,
                            Some(kept) => kept
                                .into_iter()
                                .filter(|(register, fold)| these.get(register) == Some(fold))
                                .collect(),
                        });
                    } else {
                        grown.folds = these;
                    }
                }
            } else if let Output::Counts(counts) = &streamed.output {
                let kept: Vec<(usize, Fold)> = grown.folds.clone().into_iter().collect();
                for (register, fold) in kept {
                    let Some((_, expr)) =
                        counts.iter().find(|(place, _)| *place == fold.placeholder)
                    else {
                        continue;
                    };
                    let (before, count) = if looks_back {
                        // The second buffer-full's count after the first's last bytes.
                        let second = run
                            .at_second()
                            .ok_or("a run read two buffer-fulls without coming back")?;
                        let back = taken.saturating_sub(streamed.lookback);
                        let read = run.stdin_taken() - snapshot.stdin_read;
                        let count = added(
                            &mut pool,
                            expr,
                            streamed.name,
                            &fed[back..taken],
                            &fed[taken..read.min(fed.len())],
                        )?;
                        (second.registers[register], count)
                    } else {
                        let count =
                            counted(&mut pool, expr, streamed.name, &fed[..taken.min(fed.len())])?;
                        (snapshot.registers[register], count)
                    };
                    let read = &fed[..(run.stdin_taken() - snapshot.stdin_read).min(fed.len())];
                    if let Grows::No(input) = grows(
                        &mut pool,
                        &mut solvers,
                        budget,
                        run,
                        register,
                        before,
                        count,
                        &held,
                        read,
                    )? {
                        grown.folds.remove(&register);
                        grown.registers.insert(register);
                        shown = shown.or(input);
                    }
                }
            }
            for register in changes.registers {
                if !grown.counters.contains_key(&register) && !grown.folds.contains_key(&register) {
                    grown.registers.insert(register);
                }
            }
            grown.memory.extend(changes.memory);
            grown.vectors.extend(changes.vectors);
            grown.masks.extend(changes.masks);
            chunk = Some(taken);
        }
        if let Some(found) = candidates {
            for register in found.keys() {
                grown.registers.remove(register);
            }
            grown.folds = found;
            // From here on a run reads two buffer-fulls, the second checked after the first.
            grown.fulls = 2;
        }
        let chunk = chunk
            .filter(|chunk| *chunk > 0)
            .ok_or("no run of one buffer-full came back to the read loop's head")?;
        if let Output::Counts(counts) = &streamed.output
            && counts
                .iter()
                .any(|(place, _)| !grown.folds.values().any(|fold| fold.placeholder == *place))
        {
            return Err(Refused {
                why: "it keeps a count in no register of its own as it goes".into(),
                input: shown,
            });
        }
        if streamed.max <= chunk {
            return Err(
                "its texts fit in one buffer-full, which the proof by length covers".into(),
            );
        }
        if streamed.lookback >= chunk {
            return Err("its counts look back further than a buffer-full".into());
        }
        // The step holds while every counter is at least 3, the base while it's at least 2 (3
        // where a buffer-full comes before each final piece): as many times round as the
        // smallest counter's start less one, as far as the spec's lengths go. A count is at
        // most what was read before the head.
        let rounds = grown
            .counters
            .values()
            .min()
            .map_or(u128::MAX, |start| start - 1);
        let reach = rounds
            .saturating_mul(chunk as u128)
            .min(streamed.max as u128);
        for fold in grown.folds.values_mut() {
            fold.most = reach - chunk as u128;
        }
        if grown == hook {
            let questions = verify(
                &mut pool,
                block,
                &streamed,
                &runs,
                &hook,
                &mut solvers,
                budget,
                chunk,
            )?;
            let mut folds = hook.folds.clone();
            for fold in folds.values_mut() {
                fold.most = reach;
            }
            let base = if looks_back {
                Hook {
                    low: 3,
                    stop: false,
                    fulls: 0,
                    feed: None,
                    lead: Some(streamed.characters.clone()),
                    folds,
                    ..hook
                }
            } else {
                Hook {
                    low: 2,
                    stop: false,
                    feed: None,
                    folds,
                    ..hook
                }
            };
            return Ok(Found {
                hook: base,
                chunk,
                reach: rounds.saturating_mul(chunk as u128),
                paths,
                questions,
                lookback: streamed.lookback,
            });
        }
        hook = grown;
    }
    Err("what one time round the read loop changes didn't settle".into())
}

/// The checks a run back at the head meets: it read a buffer-full (two, where its counts look
/// back), wrote the formula's bytes for it and nothing to standard error, kept each count in
/// step with the formula, and has each counter one lower each time round.
fn looped_checks(
    pool: &mut Pool,
    block: &Generated,
    streamed: &Streamed<'_>,
    run: &State,
    hook: &Hook,
    chunk: usize,
) -> Result<Term, String> {
    let snapshot = run
        .at_head()
        .ok_or("a run came back to a head it never left")?;
    let fed = &run.stdin_bytes()[snapshot.stdin_read..];
    let fulls = usize::try_from(hook.fulls()).unwrap_or(1);
    let second = run.at_second();
    let short = |read: usize| read != chunk * fulls || fed.len() <= chunk * fulls;
    if short(run.stdin_taken() - snapshot.stdin_read)
        || (fulls > 1
            && second.is_none_or(|second| second.stdin_read - snapshot.stdin_read != chunk))
    {
        return Err("a time round the read loop reads other than a buffer-full".into());
    }
    let none = BTreeMap::new();
    let fixed = crate::expr::Env::new();
    let mut checks = Vec::new();
    match &streamed.output {
        // The formula's bytes for the buffer-full, as it goes.
        Output::Map => {
            let texts = BTreeMap::from([(streamed.name.to_owned(), fed[..chunk].to_vec())]);
            let known = Known {
                numbers: &none,
                fixed: &fixed,
                texts: &texts,
                translated: RefCell::new(HashMap::new()),
                offsets: HashMap::new(),
                relative: None,
            };
            checks.push(text_is(
                pool,
                &run.stdout[snapshot.stdout..],
                &block.stdout,
                &known,
            )?);
        }
        // Nothing yet, and each count grown by the buffer-full's: where the counts look back,
        // by what the second adds after the first's last bytes.
        Output::Counts(counts) => {
            if run.stdout.len() != snapshot.stdout {
                checks.push(pool.boolean(false));
            }
            for (register, fold) in &hook.folds {
                let Some((_, expr)) = counts.iter().find(|(place, _)| *place == fold.placeholder)
                else {
                    return Err("a count with no placeholder".into());
                };
                let (before, count) = match second {
                    Some(second) if fulls > 1 => {
                        let back = chunk - streamed.lookback;
                        let count = added(
                            pool,
                            expr,
                            streamed.name,
                            &fed[back..chunk],
                            &fed[chunk..2 * chunk],
                        )?;
                        (second.registers[*register], count)
                    }
                    _ => (
                        snapshot.registers[*register],
                        counted(pool, expr, streamed.name, &fed[..chunk])?,
                    ),
                };
                let now = pool.zero_extend(run.register_term(*register), WIDTH - 64);
                let before = pool.zero_extend(before, WIDTH - 64);
                let expected = pool.add(before, count);
                checks.push(pool.eq(now, expected));
            }
        }
    }
    if run.stderr.len() != snapshot.stderr {
        checks.push(pool.boolean(false));
    }
    // Each counter one lower than it was at the head each time round.
    for register in hook.counters.keys() {
        let then = snapshot.registers[*register];
        if let Some(second) = second.filter(|_| fulls > 1) {
            let one = pool.bv(64, 1);
            let lower = pool.sub(then, one);
            checks.push(pool.eq(second.registers[*register], lower));
        }
        let rounds = pool.bv(64, fulls as u128);
        let lower = pool.sub(then, rounds);
        checks.push(pool.eq(run.register_term(*register), lower));
    }
    Ok(pool.all(&checks))
}

/// Checks the step's runs: each one breaks no memory rule, and each one that can happen comes
/// back to the head having read one buffer-full, written the formula's bytes for it and
/// nothing to standard error, with each counter one lower. Returns the questions asked.
#[allow(clippy::too_many_arguments)]
fn verify(
    pool: &mut Pool,
    block: &Generated,
    streamed: &Streamed<'_>,
    runs: &[State],
    hook: &Hook,
    solvers: &mut Solvers,
    budget: &Budget,
    chunk: usize,
) -> Result<usize, Refused> {
    let before = solvers.solver.as_ref().map_or(0, |solver| solver.questions);
    let fulls = usize::try_from(hook.fulls()).unwrap_or(1);
    for run in runs {
        // The fed bytes are more than a proof holds to their characters on every question, so
        // they're held to them where a question finds inputs.
        let held = held(pool, run, &streamed.characters);
        // The buffer-fulls the step reads: what a counterexample would be made of.
        let read: Vec<Term> = run
            .at_head()
            .map(|snapshot| {
                let from = snapshot.stdin_read.min(run.stdin_bytes().len());
                let to = (from + chunk * fulls).min(run.stdin_bytes().len());
                run.stdin_bytes()[from..to].to_vec()
            })
            .unwrap_or_default();
        let mut questions: Vec<(Term, String, bool)> = run
            .obligations
            .iter()
            .map(|(condition, problem)| (*condition, problem.clone(), false))
            .collect();
        if let Some(End::Unsafe(problem)) = &run.end {
            questions.insert(0, (pool.boolean(true), problem.clone(), false));
        }
        let broken = match &run.end {
            Some(End::Looped) => {
                let holds = looped_checks(pool, block, streamed, run, hook, chunk)?;
                let holds = settled(pool, &run.path, holds, MAX_CHOICES);
                pool.bool_not(holds)
            }
            // A run that broke a memory rule is asked about as one above.
            Some(End::Unsafe(_)) => pool.boolean(false),
            // Any other end means more input than a buffer-full went unread, or the loop
            // stopped short: no input may take it.
            _ => pool.boolean(true),
        };
        questions.push((
            broken,
            "one more buffer-full from the read loop's head goes wrong".into(),
            true,
        ));
        for (condition, problem, shows) in questions {
            if pool.truth(condition) == Some(false) {
                continue;
            }
            // Output whose terms are the formula's own can't break: nothing to ask.
            let holding = pool.bool_not(condition);
            let branched = |part: &Term| {
                pool.truth(*part) == Some(true)
                    || run.path.iter().any(|given| pool.equivalent(*given, *part))
            };
            if disjuncts(pool, holding)
                .iter()
                .any(|case| conjuncts(pool, *case).iter().all(branched))
            {
                continue;
            }
            let mut conditions = run.path.clone();
            conditions.push(condition);
            let values: &[Term] = if shows { &read } else { &[] };
            let solver = solvers.get(budget.deadline)?;
            let (answer, model) = solver
                .check(pool, &conditions, values)
                .map_err(|error| format!("the solver failed: {error}"))?;
            let (answer, model) = super::within(
                pool, solver, answer, model, conditions, &held, values,
            )
            .map_err(|stop| match stop {
                super::Stop::Unproven(why) | super::Stop::Beyond(why) => why,
                super::Stop::Counterexample(_) => "a counterexample".into(),
            })?;
            match answer {
                Answer::Unsat => {}
                Answer::Sat => {
                    if std::env::var_os("LENGUA_PROVE_EXPLAIN").is_some() {
                        let sketch: String = pool.sketch(condition, 6).chars().take(2000).collect();
                        eprintln!("induction step: {problem}, where {sketch}");
                    }
                    let input = shows
                        .then(|| {
                            read.iter()
                                .map(|byte| model.get(byte).map(|value| *value as u8))
                                .collect::<Option<Vec<u8>>>()
                        })
                        .flatten()
                        .filter(|input| !input.is_empty());
                    return Err(Refused {
                        why: problem,
                        input,
                    });
                }
                Answer::Unknown => {
                    if solver.out_of_time() {
                        return Err(super::OUT_OF_TIME.into());
                    }
                    return Err(format!(
                        "a time round the read loop couldn't be settled: {}",
                        solver.why_unknown
                    )
                    .into());
                }
            }
        }
    }
    Ok(solvers.solver.as_ref().map_or(0, |solver| solver.questions) - before)
}
