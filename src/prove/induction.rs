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
    let output = match pieces[..] {
        [Piece::Expr(mapped)] if byte_map(mapped, name) => Output::Map,
        _ => {
            let counts: Vec<(usize, &Expr)> = pieces
                .iter()
                .enumerate()
                .filter_map(|(place, piece)| match piece {
                    Piece::Expr(expr) => Some((place, *expr)),
                    Piece::Text(_) => None,
                })
                .collect();
            if counts.is_empty() || !counts.iter().all(|(_, expr)| adds_up(expr, name)) {
                return Err(
                    "its output isn't the text mapped byte by byte, as upper, lower and \
                     tr map it, nor counts that add up over its pieces, as len and count of one \
                     character do"
                        .into(),
                );
            }
            Output::Counts(counts)
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
    })
}

/// Whether `expr` counts something of the text `name` that adds up over its pieces, the count
/// of a whole the sum of its pieces' counts: its length, or how often one character appears.
fn adds_up(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Call(Function::Len, arguments) => {
            matches!(arguments.as_slice(), [Expr::Var(var)] if var == name)
        }
        Expr::Call(Function::Count, arguments) => matches!(
            arguments.as_slice(),
            [Expr::Var(var), Expr::Literal(Value::Str(part))] if var == name && part.len() == 1
        ),
        _ => false,
    }
}

/// `expr`, a count, over `bytes`.
fn counted(pool: &mut Pool, expr: &Expr, name: &str, bytes: &[Term]) -> Result<Term, String> {
    let texts = BTreeMap::from([(name.to_owned(), bytes.to_vec())]);
    match formula::translate(pool, expr, &BTreeMap::new(), &texts)? {
        Formula::Number(number) => Ok(number.term),
        _ => Err("a count that isn't a number".into()),
    }
}

/// Whether the run's `register`, which held `start` at the head, holds `start` and `count`
/// back at it: the terms say so, or the solvers find no input where it doesn't.
fn grows(
    pool: &mut Pool,
    solvers: &mut Solvers,
    budget: &Budget,
    run: &State,
    register: usize,
    before: Term,
    count: Term,
) -> Result<bool, String> {
    let now = pool.zero_extend(run.register_term(register), WIDTH - 64);
    let before = pool.zero_extend(before, WIDTH - 64);
    let expected = pool.add(before, count);
    let same = pool.eq(now, expected);
    if let Some(truth) = pool.truth(same) {
        return Ok(truth);
    }
    let mut conditions = run.path.clone();
    conditions.push(pool.bool_not(same));
    let solver = solvers.get(budget.deadline)?;
    let (answer, _) = solver
        .check(pool, &conditions, &[])
        .map_err(|error| format!("the solver failed: {error}"))?;
    Ok(answer == Answer::Unsat)
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
/// Why the induction doesn't apply or its step doesn't hold: the proof then goes by length.
pub fn step(image: &Image, block: &Generated, budget: &Budget) -> Result<Found, String> {
    let streamed = streamed(image, block)?;
    let head = image.fill_head().ok_or("it has no fill")?;
    let mut solvers = Solvers::new(budget.question_ms);
    let mut hook = Hook {
        head,
        low: 3,
        stop: true,
        feed: Some(streamed.characters.clone()),
        ..Hook::default()
    };
    let mut paths = 0;
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
        for run in &runs {
            if run.end != Some(End::Looped) {
                continue;
            }
            let snapshot = run
                .at_head()
                .ok_or("a run came back to a head it never left")?;
            let changes = run.changes(snapshot);
            if let Some(why) = changes.other.first() {
                return Err(why.clone());
            }
            let taken = run.stdin_taken() - snapshot.stdin_read;
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
                    && grown.folds.is_empty()
                {
                    // Every register that went from a constant to it and the buffer-full's
                    // count may keep the count: a later round keeps only those that go on
                    // adding, as a register that holds only the last buffer-full's doesn't.
                    let fed = run.stdin_bytes()[..taken.min(run.stdin_bytes().len())].to_vec();
                    for (placeholder, expr) in counts {
                        let count = counted(&mut pool, expr, streamed.name, &fed)?;
                        for register in &changes.registers {
                            let before = snapshot.registers[*register];
                            if let Some(start) = pool.constant(before)
                                && !grown.counters.contains_key(register)
                                && !grown.folds.contains_key(register)
                                && grows(
                                    &mut pool,
                                    &mut solvers,
                                    budget,
                                    run,
                                    *register,
                                    before,
                                    count,
                                )?
                            {
                                grown.folds.insert(
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
                }
            } else if let Output::Counts(counts) = &streamed.output {
                let fed = run.stdin_bytes()[..taken.min(run.stdin_bytes().len())].to_vec();
                let kept: Vec<(usize, Fold)> = grown.folds.clone().into_iter().collect();
                for (register, fold) in kept {
                    let Some((_, expr)) =
                        counts.iter().find(|(place, _)| *place == fold.placeholder)
                    else {
                        continue;
                    };
                    let count = counted(&mut pool, expr, streamed.name, &fed)?;
                    let before = snapshot.registers[register];
                    if !grows(
                        &mut pool,
                        &mut solvers,
                        budget,
                        run,
                        register,
                        before,
                        count,
                    )? {
                        grown.folds.remove(&register);
                        grown.registers.insert(register);
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
        let chunk = chunk
            .filter(|chunk| *chunk > 0)
            .ok_or("no run of one buffer-full came back to the read loop's head")?;
        if let Output::Counts(counts) = &streamed.output
            && counts
                .iter()
                .any(|(place, _)| !grown.folds.values().any(|fold| fold.placeholder == *place))
        {
            return Err("it keeps a count in no register of its own as it goes".into());
        }
        if streamed.max <= chunk {
            return Err(
                "its texts fit in one buffer-full, which the proof by length covers".into(),
            );
        }
        // The step holds while every counter is at least 3, the base while it's at least 2: as
        // many times round as the smallest counter's start less one, as far as the spec's
        // lengths go. A count is at most what was read before the head.
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
            return Ok(Found {
                hook: Hook {
                    low: 2,
                    stop: false,
                    feed: None,
                    folds,
                    ..hook
                },
                chunk,
                reach: rounds.saturating_mul(chunk as u128),
                paths,
                questions,
            });
        }
        hook = grown;
    }
    Err("what one time round the read loop changes didn't settle".into())
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
) -> Result<usize, String> {
    let before = solvers.solver.as_ref().map_or(0, |solver| solver.questions);
    let none = BTreeMap::new();
    let fixed = crate::expr::Env::new();
    for run in runs {
        // The fed bytes are more than a proof holds to their characters on every question, so
        // they're held to them where a question finds inputs.
        let held: Vec<Term> = run
            .stdin_bytes()
            .iter()
            .map(|byte| pool.is_one_of(*byte, &streamed.characters))
            .collect();
        let mut questions: Vec<(Term, String)> = run.obligations.clone();
        if let Some(End::Unsafe(problem)) = &run.end {
            questions.insert(0, (pool.boolean(true), problem.clone()));
        }
        let broken = match &run.end {
            Some(End::Looped) => {
                let snapshot = run
                    .at_head()
                    .ok_or("a run came back to a head it never left")?;
                let fed = run.stdin_bytes();
                if run.stdin_taken() - snapshot.stdin_read != chunk || fed.len() <= chunk {
                    return Err("a time round the read loop reads other than a buffer-full".into());
                }
                let mut checks = Vec::new();
                match &streamed.output {
                    // The formula's bytes for the buffer-full, as it goes.
                    Output::Map => {
                        let texts =
                            BTreeMap::from([(streamed.name.to_owned(), fed[..chunk].to_vec())]);
                        let known = Known {
                            numbers: &none,
                            fixed: &fixed,
                            texts: &texts,
                            translated: RefCell::new(HashMap::new()),
                            offsets: HashMap::new(),
                        };
                        checks.push(text_is(
                            pool,
                            &run.stdout[snapshot.stdout..],
                            &block.stdout,
                            &known,
                        )?);
                    }
                    // Nothing yet, and each count grown by the buffer-full's.
                    Output::Counts(counts) => {
                        if run.stdout.len() != snapshot.stdout {
                            checks.push(pool.boolean(false));
                        }
                        for (register, fold) in &hook.folds {
                            let Some((_, expr)) =
                                counts.iter().find(|(place, _)| *place == fold.placeholder)
                            else {
                                return Err("a count with no placeholder".into());
                            };
                            let count = counted(pool, expr, streamed.name, &fed[..chunk])?;
                            let now = pool.zero_extend(run.register_term(*register), WIDTH - 64);
                            let before =
                                pool.zero_extend(snapshot.registers[*register], WIDTH - 64);
                            let expected = pool.add(before, count);
                            checks.push(pool.eq(now, expected));
                        }
                    }
                }
                if run.stderr.len() != snapshot.stderr {
                    checks.push(pool.boolean(false));
                }
                // Each counter one lower than it was at the head.
                for register in hook.counters.keys() {
                    let one = pool.bv(64, 1);
                    let lower = pool.sub(snapshot.registers[*register], one);
                    checks.push(pool.eq(run.register_term(*register), lower));
                }
                let holds = pool.all(&checks);
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
        ));
        for (condition, problem) in questions {
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
            let solver = solvers.get(budget.deadline)?;
            let (answer, model) = solver
                .check(pool, &conditions, &[])
                .map_err(|error| format!("the solver failed: {error}"))?;
            let (answer, _) = super::within(pool, solver, answer, model, conditions, &held, &[])
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
                    return Err(problem);
                }
                Answer::Unknown => {
                    if solver.out_of_time() {
                        return Err(super::OUT_OF_TIME.into());
                    }
                    return Err(format!(
                        "a time round the read loop couldn't be settled: {}",
                        solver.why_unknown
                    ));
                }
            }
        }
    }
    Ok(solvers.solver.as_ref().map_or(0, |solver| solver.questions) - before)
}
