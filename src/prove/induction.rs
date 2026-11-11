//! Proof by induction over a streaming program's read loop. A program that reads standard
//! input a buffer-full at a time through fill, and either writes each buffer-full's bytes mapped
//! one by one, as `upper`, `lower` and `tr` map them, or keeps counts that add up over the
//! buffer-fulls, as `len` and `count` of one character do, and sums of such counts, of the text
//! or of the text mapped byte by byte, and writes them at the end, or writes the same for every
//! text, is proven for every length in two parts:
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
//! count, and goes on adding from any count so far: at most one a byte read for each count of
//! its sum.
//!
//! What a time round changes is any value at the head, with one exception that assumes less of
//! the program: a slot that started as a constant and whose term after a time round has a range
//! narrower than its width, as a byte loaded and widened has, is any value in that range, the
//! start's value included. Each later round holds the slot's term after a time round from such
//! a value to that range again, by the term's own range, which no path's conditions narrow: so
//! the range holds the first time at the head and after every time round, and a program's own
//! assertion that the value it carries is a byte is met. A slot that leaves its range is any
//! value from the next round on.
//!
//! A count whose buffer-full's part depends on the bytes before it, as `words` does on whether
//! the last buffer-full ended inside a word, is checked a buffer-full later: the step reads two
//! buffer-fulls from any state at the head, and the second's count must be what the formula
//! adds for it after the first's last bytes. Whatever the program keeps of what came before, a
//! flag, a byte or a mask's bit, its count must come out so, which needs no telling where it
//! keeps it. The base likewise reads a buffer-full from any state at the head before each final
//! piece, and the shortest inputs are proven from the program's start.
//!
//! A program ([`Program`]) finds its own read loop and its core takes the loop's head its own way
//! ([`Looping`]); what follows is the same for any core.
//!
//! [`Looping`]: super::exec::induction::Looping

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use super::exec::induction::{Fold, Held, Hook, PhaseFold, Slot};
use super::exec::{self, End, State};
use super::formula::{self, Formula, WIDTH};
use super::smt::Answer;
use super::term::{Pool, Term};
use super::world::Arrival;
use super::{
    Budget, Known, MAX_CHOICES, Program, Solvers, Witnessed, conjuncts, disjuncts, settled, text_is,
};
use crate::expr::{Expr, Generator, Piece};
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

/// Rounds of finding what a time round the loop changes before giving up on a fixpoint. A slot
/// first taken to stay in a range and then found to leave it takes a round more than one made
/// any value at once.
const MAX_ROUNDS: usize = 6;

/// The range a slot stays in from one time at the read loop's head to the next, as far as its
/// terms' own ranges say: from `then`, what it held the first time at the head, to each of
/// `ends`, what it holds each time the run is back there. `None` where it didn't start as a
/// constant, or where the range is every value of the slot's width, which says nothing.
fn ranged(pool: &Pool, then: Term, ends: &[Term]) -> Option<(u128, u128)> {
    let start = pool.constant(then)?;
    let (mut low, mut high) = (start, start);
    for end in ends {
        let (least, most) = pool.range(*end);
        low = low.min(least);
        high = high.max(most);
    }
    let all = u128::MAX >> (128 - pool.width(then).clamp(1, 128));
    (low > 0 || high < all).then_some((low, high))
}

/// Whether `value` stays in `range` whatever its variables are, by its term's own range.
fn stays_in(pool: &Pool, value: Term, (low, high): (u128, u128)) -> bool {
    let (least, most) = pool.range(value);
    low <= least && most <= high
}

/// The text a block streams through its read loop.
struct Streamed<'a> {
    /// The variable's name.
    name: &'a str,
    /// Its characters.
    characters: Vec<u8>,
    /// The longest the idea lets it be.
    max: usize,
    /// What the program writes of it.
    output: Output<'a>,
    /// The bytes before each buffer-full its counts need.
    lookback: usize,
    /// The counts of a part that can overlap itself, by placeholder, with the part: each goes
    /// on from the matcher's phase, which a register keeps.
    phased: Vec<(usize, Vec<u8>)>,
}

/// What a streaming program writes.
enum Output<'a> {
    /// Each buffer-full's bytes mapped one by one, as it goes.
    Map,
    /// Counts that add up over the buffer-fulls, written at the end: each placeholder's place
    /// among the template's pieces, with its formula.
    Counts(Vec<(usize, &'a Expr)>),
}

/// The text `block` streams, when it's one the induction applies to: a program with a read loop
/// the induction takes, whose standard input is one text, whose output is that text mapped byte
/// by byte or counts of it that add up over its pieces, and which takes nothing else in.
fn streamed<'a, P: Program>(image: &P, block: &'a Generated) -> Result<Streamed<'a>, String> {
    image.read_loop()?;
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
        [Piece::Expr(mapped)] if formula::byte_map(mapped, name) => (Output::Map, 0),
        _ => {
            let counts: Vec<(usize, &Expr)> = pieces
                .iter()
                .enumerate()
                .filter_map(|(place, piece)| match piece {
                    Piece::Expr(expr) => Some((place, *expr)),
                    Piece::Text(_) => None,
                })
                .collect();
            // A count of a part that overlaps itself goes on from the matcher's phase, not
            // from bytes before the piece.
            let lookbacks: Option<Vec<usize>> = counts
                .iter()
                .map(|(_, expr)| {
                    formula::lookback(expr, name)
                        .or_else(|| formula::bordered_part(expr, name).map(|_| 0))
                })
                .collect();
            // Output with no placeholder is the same for every text: no count to keep.
            let lookback =
                lookbacks.map(|lookbacks| lookbacks.into_iter().max().unwrap_or_default());
            let Some(lookback) = lookback else {
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
    let phased: Vec<(usize, Vec<u8>)> = match &output {
        Output::Counts(counts) => counts
            .iter()
            .filter_map(|(place, expr)| Some((*place, formula::bordered_part(expr, name)?)))
            .collect(),
        Output::Map => Vec::new(),
    };
    if !phased.is_empty() && lookback > 0 {
        return Err(
            "it counts a part that overlaps itself beside counts that look back at the bytes \
             before each piece, which the induction doesn't take together yet"
                .into(),
        );
    }
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
        phased,
    })
}

/// What the count `expr`, the output's placeholder at `placeholder`, adds for `piece` from where
/// the run stood at the head, `at_head`: as from a text's start, or, for a part that overlaps
/// itself, from the phase the head's phase slot stands for.
fn grown_by(
    pool: &mut Pool,
    streamed: &Streamed<'_>,
    hook: &Hook,
    at_head: &Arrival,
    (placeholder, expr): (usize, &Expr),
    piece: &[Term],
) -> Result<Term, String> {
    let Some(part) = phased_part(streamed, placeholder) else {
        return counted(pool, expr, streamed.name, piece);
    };
    let (slot, fold) = hook
        .phases
        .iter()
        .find(|(_, fold)| fold.placeholder == placeholder)
        .ok_or("a count of a part that overlaps itself whose phase no register keeps")?;
    let phase = fold.phase(pool, at_head.held(*slot), part.len());
    let texts = BTreeMap::from([(streamed.name.to_owned(), piece.to_vec())]);
    match formula::count_from(pool, expr, &texts, &phase)? {
        Formula::Number(number) => Ok(number.term),
        _ => Err("a count that isn't a number".into()),
    }
}

/// That each phase slot of `run`, which held what `at_head` says at the head, holds one of its
/// values after the time round, standing for the phase the formula's matcher is in after
/// `piece`.
fn phases_kept<C>(
    pool: &mut Pool,
    streamed: &Streamed<'_>,
    hook: &Hook,
    (at_head, run): (&Arrival, &State<C>),
    piece: &[Term],
) -> Result<Term, String>
where
    State<C>: Held,
{
    let mut checks = Vec::new();
    for (slot, fold) in &hook.phases {
        let part = phased_part(streamed, fold.placeholder).ok_or("a phase of no count")?;
        let before = fold.phase(pool, at_head.held(*slot), part.len());
        let (_, after) = formula::matches_from(pool, piece, part, &before);
        let value = run.held(*slot);
        checks.push(fold.holds(pool, value));
        let kept = fold.phase(pool, value, part.len());
        for (said, wanted) in kept.conditions().iter().zip(after.conditions()) {
            checks.push(pool.eq(*said, *wanted));
        }
    }
    Ok(pool.all(&checks))
}

/// The part of the count at `placeholder`, when it's a count of a part that overlaps itself.
fn phased_part<'a>(streamed: &'a Streamed<'_>, placeholder: usize) -> Option<&'a [u8]> {
    streamed
        .phased
        .iter()
        .find(|(place, _)| *place == placeholder)
        .map(|(_, part)| part.as_slice())
}

/// Random buffer-fulls a phase is learned from.
const PHASE_SAMPLES: usize = 256;

/// The values `term`, a register after the first time round the loop from the program's start,
/// takes on random inputs of `characters` the run's path allows, each with the state the
/// formula's matcher of `part` is in after the buffer-full of `chunk` bytes the run read,
/// `phase`: how many bytes of `part` the text since its last match ends with. `initial`, the
/// register's value before, stands for none. `None` where one value stands for two states, so
/// the register doesn't keep the phase.
#[allow(clippy::too_many_arguments)]
fn phase_states<C>(
    pool: &Pool,
    run: &State<C>,
    term: Term,
    phase: &formula::Phase,
    chunk: usize,
    part: &[u8],
    initial: u128,
    characters: &[u8],
) -> Option<BTreeMap<u128, usize>> {
    use rand::{RngExt, SeedableRng};
    // Every byte the run's input holds, the buffer-full and what's past it alike.
    let names: Vec<String> = run
        .world
        .stdin_bytes()
        .iter()
        .map(|byte| pool.name(*byte).map(str::to_owned))
        .collect::<Option<_>>()?;
    let chunk = chunk.min(names.len());
    let mut rng = rand::rngs::ChaCha8Rng::seed_from_u64(0x9a5e_f01d);
    let mut states = BTreeMap::from([(initial, 0)]);
    for sample in 0..PHASE_SAMPLES {
        let mut bytes: Vec<u8> = (0..names.len())
            .map(|_| characters[rng.random_range(0..characters.len())])
            .collect();
        // Every other buffer-full ends with a start of the part, so each phase shows.
        if sample % 2 == 1 && part.len() > 1 {
            let start = &part[..rng.random_range(1..part.len())];
            if start.iter().all(|byte| characters.contains(byte)) && start.len() <= chunk {
                bytes[chunk - start.len()..chunk].copy_from_slice(start);
            }
        }
        let values: HashMap<String, u128> = names
            .iter()
            .cloned()
            .zip(bytes.iter().map(|byte| u128::from(*byte)))
            .collect();
        if run
            .world
            .path
            .iter()
            .any(|condition| pool.evaluate(*condition, &values) != Some(1))
        {
            continue;
        }
        let value = pool.evaluate(term, &values)?;
        let mut state = 0;
        for (index, condition) in phase.conditions().iter().enumerate() {
            if pool.evaluate(*condition, &values)? == 1 {
                if state != 0 {
                    return None;
                }
                state = index + 1;
            }
        }
        if *states.entry(value).or_insert(state) != state {
            return None;
        }
    }
    (states.values().any(|state| *state != 0)).then_some(states)
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
/// finds inputs is asked again with, so its inputs are ones the idea can give.
fn held<C>(pool: &mut Pool, run: &State<C>, characters: &[u8]) -> Vec<Term> {
    run.world
        .stdin_bytes()
        .iter()
        .map(|byte| pool.is_one_of(*byte, characters))
        .collect()
}

/// Random inputs that may show a register isn't a count before a solver is asked.
const REFUTING_SAMPLES: usize = 8;

/// Whether a register grows as a count does.
enum Grows {
    /// It holds what it held and the count.
    Yes,
    /// It doesn't, and where the solvers found an input that shows it, the bytes `read` are.
    No(Option<Vec<u8>>),
}

/// Whether the run's `slot`, which held `before`, now holds `before` and `count`: the terms say
/// so, or the solvers find no input with `held` that takes the run where it doesn't. Where they
/// find one, its bytes `read` come with the answer.
#[allow(clippy::too_many_arguments)]
fn grows<C>(
    pool: &mut Pool,
    solvers: &mut Solvers,
    budget: &Budget,
    run: &State<C>,
    (slot, before, count): (Slot, Term, Term),
    (held, characters): (&[Term], &[u8]),
    read: &[Term],
) -> Result<Grows, String>
where
    State<C>: Held,
{
    use rand::{RngExt, SeedableRng};
    let now = widened(pool, run.held(slot));
    let before = widened(pool, before);
    let expected = pool.add(before, count);
    let same = pool.eq(now, expected);
    match pool.truth(same) {
        Some(true) => return Ok(Grows::Yes),
        Some(false) => return Ok(Grows::No(None)),
        None => {}
    }
    let mut conditions = run.world.path.clone();
    conditions.push(pool.bool_not(same));
    // Most registers a time round changes aren't a count: an input of the text's characters
    // that the run's path allows and that shows this one isn't settles it with no solver.
    let mut rng = rand::rngs::ChaCha8Rng::seed_from_u64(0x6e05_5eed);
    for _ in 0..REFUTING_SAMPLES {
        let mut witness: HashMap<Term, u128> = run
            .world
            .stdin_bytes()
            .iter()
            .map(|byte| {
                let character = characters[rng.random_range(0..characters.len())];
                (*byte, u128::from(character))
            })
            .collect();
        if super::meets(pool, &conditions, &mut witness, &mut rng) {
            let input = read
                .iter()
                .map(|byte| witness.get(byte).map(|value| *value as u8))
                .collect::<Option<Vec<u8>>>()
                .filter(|input| !input.is_empty());
            return Ok(Grows::No(input));
        }
    }
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

/// `value`, a slot's, widened without its sign to the formulas' width from a WebAssembly
/// value's own.
fn widened(pool: &mut Pool, value: Term) -> Term {
    let width = pool.width(value);
    pool.zero_extend(value, WIDTH - width)
}

/// The runs from `start`, as a proof's shapes explore them: a branch no witness reaches is
/// followed without asking, since each run's end is asked about anyway.
fn explore<P: Program>(
    image: &P,
    start: State<P::Core>,
    pool: &mut Pool,
    solvers: &mut Solvers,
    budget: &Budget,
) -> Result<Vec<State<P::Core>>, String> {
    let careful = start.clone();
    let limits = (
        budget.deadline,
        budget
            .deadline
            .max(std::time::Instant::now() + super::MIN_SHAPE_TIME),
    );
    let (runs, guessed) = {
        let mut oracle = Witnessed::new(solvers, limits, 0, true);
        let runs = exec::explore(image, start, pool, &mut oracle, budget.max_paths)?;
        (runs, oracle.guessed)
    };
    let too_many = Some(End::TooManyPaths);
    if guessed && runs.iter().any(|run| run.world.end == too_many) {
        let mut oracle = Witnessed::new(solvers, limits, 0, false);
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
pub(super) fn step<P: Program>(
    image: &P,
    block: &Generated,
    budget: &Budget,
) -> Result<Found, Refused> {
    let streamed = streamed(image, block)?;
    let head = image.read_loop()?;
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
        let start = image.begin(&mut pool, &[], &[], Vec::new()).for_every_cut();
        let start = P::hooked(start, Rc::new(hook.clone()))?;
        let runs = explore(image, start, &mut pool, &mut solvers, budget)?;
        paths += runs.len();
        let mut grown = hook.clone();
        let mut chunk = None;
        // For counts that look back: the slots that went from a constant to it and the first
        // buffer-full's count on every run.
        let mut candidates: Option<BTreeMap<Slot, Fold>> = None;
        for run in &runs {
            if run.world.end != Some(End::Looped) {
                continue;
            }
            let snapshot = run
                .world
                .at_head()
                .ok_or("a run came back to a head it never left")?;
            // What one buffer-full changes: to the run's end, or to its second time at the
            // head where it reads two.
            let (changes, taken) = if hook.fulls() > 1 {
                let second = run
                    .world
                    .at_second()
                    .ok_or("a run read two buffer-fulls without coming back between them")?;
                let changes = run
                    .changed()
                    .cloned()
                    .ok_or("a run's first buffer-full left no changes")?;
                (changes, second.stdin_read - snapshot.stdin_read)
            } else {
                (run.changes(), run.world.stdin_taken() - snapshot.stdin_read)
            };
            if let Some(why) = changes.other.first() {
                return Err(why.clone().into());
            }
            let held = held(&mut pool, run, &streamed.characters);
            let fed = run.world.stdin_bytes()[snapshot.stdin_read..].to_vec();
            // A bound counter goes from its start one lower each time round; a count, from its
            // start, by what the buffer-full adds.
            if round == 0 {
                for slot in &changes.slots {
                    let then = pool.constant(snapshot.held(*slot));
                    let now = pool.constant(run.held(*slot));
                    if let (Some(then), Some(now)) = (then, now)
                        && now.wrapping_add(1) == then
                        && then >= 3
                    {
                        grown.counters.insert(*slot, then);
                    }
                }
                if let Output::Counts(counts) = &streamed.output
                    && (looks_back || grown.folds.is_empty())
                {
                    // Every slot that went from a constant to it and the buffer-full's count
                    // may keep the count: a later round keeps only those that go on adding, as
                    // a register that holds only the last buffer-full's doesn't.
                    let first = &fed[..taken.min(fed.len())];
                    let mut these = BTreeMap::new();
                    for (placeholder, expr) in counts {
                        let count = counted(&mut pool, expr, streamed.name, first)?;
                        for slot in &changes.slots {
                            let before = snapshot.held(*slot);
                            if let Some(start) = pool.constant(before)
                                && !grown.counters.contains_key(slot)
                                && !these.contains_key(slot)
                                && matches!(
                                    grows(
                                        &mut pool,
                                        &mut solvers,
                                        budget,
                                        run,
                                        (*slot, before, count),
                                        (&held, &streamed.characters),
                                        &[],
                                    )?,
                                    Grows::Yes
                                )
                            {
                                these.insert(
                                    *slot,
                                    Fold {
                                        start,
                                        most: 0,
                                        placeholder: *placeholder,
                                        width: pool.width(before),
                                    },
                                );
                            }
                        }
                    }
                    // Where a count is of a part that overlaps itself, the slot whose value
                    // after the first time round says the matcher's phase then, on every random
                    // buffer-full: it starts out standing for nothing matched, and the step shows
                    // each later time round keeps it so.
                    for (placeholder, part) in &streamed.phased {
                        let start = formula::Phase::start(&mut pool, part);
                        let (_, after) = formula::matches_from(&mut pool, first, part, &start);
                        let found = changes.slots.iter().find_map(|slot| {
                            if grown.counters.contains_key(slot)
                                || these.contains_key(slot)
                                || grown.phases.contains_key(slot)
                            {
                                return None;
                            }
                            let initial = pool.constant(snapshot.held(*slot))?;
                            let states = phase_states(
                                &pool,
                                run,
                                run.held(*slot),
                                &after,
                                first.len(),
                                part,
                                initial,
                                &streamed.characters,
                            )?;
                            Some((*slot, states, pool.width(snapshot.held(*slot))))
                        });
                        if let Some((slot, states, width)) = found {
                            grown.phases.insert(
                                slot,
                                PhaseFold {
                                    states,
                                    placeholder: *placeholder,
                                    width,
                                },
                            );
                        }
                    }
                    if looks_back {
                        // The first buffer-full's count is checked here alone, so it must hold
                        // on every run.
                        candidates = Some(match candidates {
                            None => these,
                            Some(kept) => kept
                                .into_iter()
                                .filter(|(slot, fold)| these.get(slot) == Some(fold))
                                .collect(),
                        });
                    } else {
                        grown.folds = these;
                    }
                }
            } else if let Output::Counts(counts) = &streamed.output {
                let kept: Vec<(Slot, Fold)> = grown.folds.clone().into_iter().collect();
                for (slot, fold) in kept {
                    let Some((_, expr)) =
                        counts.iter().find(|(place, _)| *place == fold.placeholder)
                    else {
                        continue;
                    };
                    let (before, count) = if looks_back {
                        // The second buffer-full's count after the first's last bytes.
                        let second = run
                            .world
                            .at_second()
                            .ok_or("a run read two buffer-fulls without coming back")?;
                        let back = taken.saturating_sub(streamed.lookback);
                        let read = run.world.stdin_taken() - snapshot.stdin_read;
                        let count = added(
                            &mut pool,
                            expr,
                            streamed.name,
                            &fed[back..taken],
                            &fed[taken..read.min(fed.len())],
                        )?;
                        (second.held(slot), count)
                    } else {
                        let count = grown_by(
                            &mut pool,
                            &streamed,
                            &hook,
                            snapshot,
                            (fold.placeholder, expr),
                            &fed[..taken.min(fed.len())],
                        )?;
                        (snapshot.held(slot), count)
                    };
                    let since = run.world.stdin_taken() - snapshot.stdin_read;
                    let read = &fed[..since.min(fed.len())];
                    if let Grows::No(input) = grows(
                        &mut pool,
                        &mut solvers,
                        budget,
                        run,
                        (slot, before, count),
                        (&held, &streamed.characters),
                        read,
                    )? {
                        grown.folds.remove(&slot);
                        grown.slots.insert(slot);
                        shown = shown.or(input);
                    }
                }
            }
            for slot in changes.slots {
                if grown.counters.contains_key(&slot)
                    || grown.folds.contains_key(&slot)
                    || grown.phases.contains_key(&slot)
                    || grown.slots.contains(&slot)
                {
                    continue;
                }
                // What the slot holds each time the run is back at the head.
                let mut ends = vec![run.held(slot)];
                if let Some(second) = run.world.at_second() {
                    ends.push(second.held(slot));
                }
                let kept = if round == 0 {
                    let found = ranged(&pool, snapshot.held(slot), &ends);
                    let known = grown.ranged.get(&slot).copied();
                    found.map(|(low, high)| {
                        known.map_or((low, high), |(least, most)| {
                            (low.min(least), high.max(most))
                        })
                    })
                } else {
                    hook.ranged
                        .get(&slot)
                        .copied()
                        .filter(|range| ends.iter().all(|end| stays_in(&pool, *end, *range)))
                };
                match kept {
                    Some(range) => {
                        grown.ranged.insert(slot, range);
                    }
                    None => {
                        grown.ranged.remove(&slot);
                        grown.slots.insert(slot);
                    }
                }
            }
            grown.memory.extend(changes.memory);
            chunk = Some(taken);
        }
        if let Some(found) = candidates {
            for slot in found.keys() {
                grown.slots.remove(slot);
                grown.ranged.remove(slot);
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
        if streamed
            .phased
            .iter()
            .any(|(place, _)| !grown.phases.values().any(|fold| fold.placeholder == *place))
        {
            return Err(Refused {
                why: "it counts a part that can overlap itself, and no register of its own says \
                      after each buffer-full how much of the part the text ends with"
                    .into(),
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
        // smallest counter's start less one, as far as the idea's lengths go. A count is at
        // most what was read before the head.
        let rounds = grown
            .counters
            .values()
            .min()
            .map_or(u128::MAX, |start| start - 1);
        let reach = rounds
            .saturating_mul(chunk as u128)
            .min(streamed.max as u128);
        // A count goes up by at most one a byte, and a sum of counts by at most one a byte for
        // each of them.
        let a_byte = |fold: &Fold| match &streamed.output {
            Output::Counts(counts) => counts
                .iter()
                .find(|(place, _)| *place == fold.placeholder)
                .map_or(1, |(_, expr)| formula::most_a_byte(expr)),
            Output::Map => 1,
        };
        for fold in grown.folds.values_mut() {
            fold.most = (reach - chunk as u128).saturating_mul(a_byte(fold));
        }
        // One kept in fewer bits than the idea's longest text needs would wrap.
        if grown.folds.values().any(|fold| {
            let most = u128::MAX >> (128 - fold.width.clamp(1, 128));
            fold.start
                .checked_add(reach.saturating_mul(a_byte(fold)))
                .is_none_or(|top| top > most)
        }) {
            return Err("it keeps a count in fewer bits than the idea's longest text needs".into());
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
            // What the pool made after finding the memory short is nothing to go on: the step
            // isn't shown.
            if pool.out_of_room() {
                return Err(super::watch::OUT_OF_MEMORY.into());
            }
            let mut folds = hook.folds.clone();
            for fold in folds.values_mut() {
                fold.most = reach.saturating_mul(a_byte(fold));
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
fn looped_checks<C>(
    pool: &mut Pool,
    block: &Generated,
    streamed: &Streamed<'_>,
    run: &State<C>,
    hook: &Hook,
    chunk: usize,
) -> Result<Term, String>
where
    State<C>: Held,
{
    let world = &run.world;
    let snapshot = world
        .at_head()
        .ok_or("a run came back to a head it never left")?;
    let fed = &world.stdin_bytes()[snapshot.stdin_read..];
    let fulls = usize::try_from(hook.fulls()).unwrap_or(1);
    let second = world.at_second();
    let short = |read: usize| read != chunk * fulls || fed.len() <= chunk * fulls;
    if short(world.stdin_taken() - snapshot.stdin_read)
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
                lengths: None,
                translated: RefCell::new(HashMap::new()),
                offsets: HashMap::new(),
                relative: None,
            };
            checks.push(text_is(
                pool,
                &world.stdout[snapshot.stdout..],
                &block.stdout,
                &known,
            )?);
        }
        // Nothing yet, and each count grown by the buffer-full's: where the counts look back,
        // by what the second adds after the first's last bytes.
        Output::Counts(counts) => {
            if world.stdout.len() != snapshot.stdout {
                checks.push(pool.boolean(false));
            }
            for (slot, fold) in &hook.folds {
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
                        (second.held(*slot), count)
                    }
                    _ => (
                        snapshot.held(*slot),
                        grown_by(
                            pool,
                            streamed,
                            hook,
                            snapshot,
                            (fold.placeholder, expr),
                            &fed[..chunk],
                        )?,
                    ),
                };
                let now = widened(pool, run.held(*slot));
                let before = widened(pool, before);
                let expected = pool.add(before, count);
                checks.push(pool.eq(now, expected));
            }
            // Each phase slot back at one of its values, saying the phase after the
            // buffer-full.
            checks.push(phases_kept(
                pool,
                streamed,
                hook,
                (snapshot, run),
                &fed[..chunk],
            )?);
        }
    }
    if world.stderr.len() != snapshot.stderr {
        checks.push(pool.boolean(false));
    }
    // Each counter one lower than it was at the head each time round.
    for slot in hook.counters.keys() {
        let then = snapshot.held(*slot);
        let width = pool.width(then);
        if let Some(second) = second.filter(|_| fulls > 1) {
            let one = pool.bv(width, 1);
            let lower = pool.sub(then, one);
            checks.push(pool.eq(second.held(*slot), lower));
        }
        let rounds = pool.bv(width, fulls as u128);
        let lower = pool.sub(then, rounds);
        checks.push(pool.eq(run.held(*slot), lower));
    }
    Ok(pool.all(&checks))
}

/// Checks the step's runs: each one breaks no memory rule, and each one that can happen comes
/// back to the head having read one buffer-full, written the formula's bytes for it and
/// nothing to standard error, with each counter one lower. Returns the questions asked.
#[allow(clippy::too_many_arguments)]
fn verify<C>(
    pool: &mut Pool,
    block: &Generated,
    streamed: &Streamed<'_>,
    runs: &[State<C>],
    hook: &Hook,
    solvers: &mut Solvers,
    budget: &Budget,
    chunk: usize,
) -> Result<usize, Refused>
where
    State<C>: Held,
{
    let before = solvers.solver.as_ref().map_or(0, |solver| solver.questions);
    let fulls = usize::try_from(hook.fulls()).unwrap_or(1);
    for run in runs {
        // The fed bytes are more than a proof holds to their characters on every question, so
        // they're held to them where a question finds inputs.
        let held = held(pool, run, &streamed.characters);
        // The buffer-fulls the step reads: what a counterexample would be made of.
        let stdin = run.world.stdin_bytes();
        let read: Vec<Term> = run
            .world
            .at_head()
            .map(|snapshot| {
                let from = snapshot.stdin_read.min(stdin.len());
                let to = (from + chunk * fulls).min(stdin.len());
                stdin[from..to].to_vec()
            })
            .unwrap_or_default();
        let mut questions: Vec<(Term, String, bool)> = run
            .world
            .obligations
            .iter()
            .map(|(condition, problem)| (*condition, problem.clone(), false))
            .collect();
        if let Some(End::Unsafe(problem)) = &run.world.end {
            questions.insert(0, (pool.boolean(true), problem.clone(), false));
        }
        let broken = match &run.world.end {
            Some(End::Looped) => {
                let holds = looped_checks(pool, block, streamed, run, hook, chunk)?;
                let holds = settled(pool, &run.world.path, holds, MAX_CHOICES);
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
            let path = &run.world.path;
            let branched = |part: &Term| {
                pool.truth(*part) == Some(true)
                    || path.iter().any(|given| pool.equivalent(*given, *part))
            };
            if disjuncts(pool, holding)
                .iter()
                .any(|case| conjuncts(pool, *case).iter().all(branched))
            {
                continue;
            }
            let mut conditions = path.clone();
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
