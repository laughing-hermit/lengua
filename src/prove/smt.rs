//! The SMT solvers the prover asks, in SMT-LIB 2 over pipes. z3 answers every question; when
//! cvc5 is installed too, every condition z3 finds impossible must be impossible for cvc5 as
//! well, so one solver's bug can't make a false proof. Terms are defined once, and each question
//! is asked inside its own `push`/`pop`, so the definitions stay and the assertions go.

use std::collections::HashMap;
use std::io::{self, BufRead as _, BufReader, Write as _};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::term::{Node, Pool, Sort, Term};

/// What the solver says about a set of conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Some inputs meet them all.
    Sat,
    /// No input meets them all: proven impossible.
    Unsat,
    /// The solver ran out of time or couldn't decide.
    Unknown,
}

/// The line each solver prints back to mark the end of each batch of commands.
const MARKER: &str = "@@lengua@@";

/// An SMT solver lengua can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// z3, from Microsoft Research.
    Z3,
    /// cvc5, from Stanford and the University of Iowa.
    Cvc5,
}

impl Kind {
    /// The solver's program name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Z3 => "z3",
            Self::Cvc5 => "cvc5",
        }
    }
}

/// The z3 strategy: bit-blast straight into the SAT solver. z3 4.16's own simplifier gets some
/// bit-vector formulas wrong (it called unsatisfiable formulas satisfiable, and the reverse), so
/// lengua simplifies terms itself and uses z3 only for bit-blasting and SAT. Division, which the
/// bit-blaster can't do alone, is defined by [`Solver::division`].
const STRATEGY: &str = "(then bit-blast sat)";

/// How many times longer than the first solver the second may take to confirm an answer: cvc5
/// takes several times longer than z3's bit-blasting on lengua's questions.
const CONFIRM_FACTOR: u64 = 4;

/// One running solver.
struct Process {
    kind: Kind,
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    /// The time limit the solver was last given, in milliseconds.
    limit_ms: u64,
    /// The time limit it started with, which a reset returns to.
    start_ms: u64,
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Process {
    /// Starts `program`, which speaks like `kind`, giving each question at most `timeout_ms`.
    fn start(kind: Kind, program: &str, timeout_ms: u64) -> io::Result<Self> {
        let mut command = Command::new(program);
        match kind {
            Kind::Z3 => command.args(["-in", "-smt2", "-memory:4096"]),
            Kind::Cvc5 => command
                .args(["--lang=smt2", "--incremental", "--produce-models"])
                .arg(format!("--tlimit-per={timeout_ms}")),
        };
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("the solver has no input"))?;
        let output = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| io::Error::other("the solver has no output"))?,
        );
        let mut process = Self {
            kind,
            child,
            input,
            output,
            limit_ms: timeout_ms,
            start_ms: timeout_ms,
        };
        process.prepare()?;
        Ok(process)
    }

    /// Sets the options and logic every session starts with.
    fn prepare(&mut self) -> io::Result<()> {
        self.send(match self.kind {
            Kind::Z3 => {
                "(set-option :print-success false)\n(set-option :produce-models true)\n\
                 (set-logic QF_BV)\n"
            }
            Kind::Cvc5 => "(set-logic QF_BV)\n",
        })?;
        self.sync()?;
        Ok(())
    }

    /// Forgets every definition and assertion, as if just started: the same process serves the
    /// next question set. Options given on the command line stay.
    fn reset(&mut self) -> io::Result<()> {
        self.send("(reset)\n")?;
        self.limit_ms = self.start_ms;
        self.prepare()
    }

    fn send(&mut self, text: &str) -> io::Result<()> {
        self.input.write_all(text.as_bytes())?;
        self.input.flush()
    }

    /// Waits until the solver has answered everything sent so far; returns what it printed.
    ///
    /// # Errors
    ///
    /// Fails when the solver stops or rejects a command.
    fn sync(&mut self) -> io::Result<Vec<String>> {
        self.send(&format!("(echo \"{MARKER}\")\n"))?;
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            if self.output.read_line(&mut line)? == 0 {
                return Err(io::Error::other(format!("{} stopped", self.kind.name())));
            }
            // cvc5 echoes the quotes too.
            let line = line.trim().trim_matches('"').to_owned();
            if line == MARKER {
                return Ok(lines);
            }
            if line.starts_with("(error") {
                return Err(io::Error::other(format!(
                    "{} rejected a question: {line}",
                    self.kind.name()
                )));
            }
            lines.push(line);
        }
    }

    /// Whether the conditions asserted can all hold, decided within `limit_ms`.
    fn check_sat(&mut self, limit_ms: u64) -> io::Result<Answer> {
        let command = match self.kind {
            Kind::Z3 => format!("(check-sat-using (try-for {STRATEGY} {limit_ms}))\n"),
            Kind::Cvc5 if limit_ms == self.limit_ms => "(check-sat)\n".to_owned(),
            Kind::Cvc5 => format!("(set-option :tlimit-per {limit_ms})\n(check-sat)\n"),
        };
        self.limit_ms = limit_ms;
        self.send(&command)?;
        let lines = self.sync()?;
        Ok(
            match lines
                .iter()
                .find(|line| !line.is_empty())
                .map(String::as_str)
            {
                Some("sat") => Answer::Sat,
                Some("unsat") => Answer::Unsat,
                _ => Answer::Unknown,
            },
        )
    }

    /// Why the solver's last answer was unknown, in its own words: "ran out of time" for a
    /// timeout.
    fn reason(&mut self) -> io::Result<String> {
        self.send("(get-info :reason-unknown)\n")?;
        let text = self.sync()?.join(" ");
        let reason = text
            .split_once(":reason-unknown")
            .map_or(text.as_str(), |(_, reason)| reason)
            .trim_matches(|character: char| character == ')' || character.is_whitespace())
            .trim_matches('"')
            .to_owned();
        Ok(
            if reason.contains("timeout") || reason.contains("canceled") || reason.is_empty() {
                "ran out of time".to_owned()
            } else {
                format!("couldn't decide ({reason})")
            },
        )
    }

    /// The values of `values` and of every variable in the inputs the solver found, when they
    /// really meet every condition: a solver's model is checked, never trusted.
    fn model(
        &mut self,
        pool: &Pool,
        conditions: &[Term],
        values: &[Term],
    ) -> io::Result<Option<HashMap<Term, u128>>> {
        let variables = pool.variables(conditions);
        let asked: Vec<Term> = values.iter().chain(&variables).copied().collect();
        let model = if asked.is_empty() {
            HashMap::new()
        } else {
            let names: Vec<String> = asked
                .iter()
                .map(|term| format!("t{}", term.index()))
                .collect();
            self.send(&format!("(get-value ({}))\n", names.join(" ")))?;
            parse_values(&self.sync()?.join(" "))
        };
        let by_name: HashMap<String, u128> = variables
            .iter()
            .filter_map(|term| Some((pool.name(*term)?.to_owned(), *model.get(term)?)))
            .collect();
        let holds = conditions
            .iter()
            .all(|condition| pool.evaluate(*condition, &by_name) == Some(1));
        Ok(holds.then_some(model))
    }
}

/// The solvers a proof asks: the first answers every question, and the second, when there is
/// one, must agree before any set of conditions counts as impossible.
pub struct Solver {
    first: Process,
    second: Option<Process>,
    /// Why no second solver checks the first, when none does.
    pub alone: Option<String>,
    /// Everything sent, when `LENGUA_SMT_LOG` names a folder to keep it in.
    log: Option<std::fs::File>,
    defined: Vec<bool>,
    /// The questions asked so far, for the trace.
    pub questions: usize,
    /// Times the second solver found real inputs for conditions the first called impossible.
    pub overruled: usize,
    /// Why the last unknown answer was unknown.
    pub why_unknown: String,
    timeout_ms: u64,
    /// When to stop asking: questions after it are unknown.
    deadline: Option<std::time::Instant>,
    /// The quotient and remainder variables standing for each division.
    divisions: HashMap<Division, (String, String)>,
}

fn sort_text(sort: Sort) -> String {
    match sort {
        Sort::Bool => "Bool".into(),
        Sort::Bv(width) => format!("(_ BitVec {width})"),
    }
}

/// A division the solver gets as quotient and remainder variables: by a constant, by a term, or
/// signed (on the sizes of two terms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Division {
    ByConstant(Term, u128),
    ByTerm(Term, Term),
    Signed(Term, Term),
}

/// `value` as a bit-vector literal `width` bits wide.
fn literal(value: u128, width: u32) -> String {
    if width.is_multiple_of(4) {
        format!("#x{value:0digits$x}", digits = (width / 4) as usize)
    } else {
        format!("#b{value:0digits$b}", digits = width as usize)
    }
}

/// Every bit of `width` set.
const fn ones(width: u32) -> u128 {
    if width >= 128 {
        u128::MAX
    } else {
        (1 << width) - 1
    }
}

/// Numbers the solver sessions of this process, for their logs.
static SESSIONS: AtomicUsize = AtomicUsize::new(0);

impl Solver {
    /// Starts z3, and cvc5 to check it, giving each question at most `timeout_ms`. When only
    /// one of them is installed, it answers alone.
    ///
    /// # Errors
    ///
    /// Fails when neither solver can start.
    pub fn start(timeout_ms: u64) -> io::Result<Self> {
        Self::with(&[(Kind::Z3, "z3"), (Kind::Cvc5, "cvc5")], timeout_ms)
    }

    /// Starts the first of `solvers` that runs, to answer, and the next one that runs, to check
    /// it. Each is a kind of solver and the program to run for it.
    fn with(solvers: &[(Kind, &str)], timeout_ms: u64) -> io::Result<Self> {
        let mut running = Vec::new();
        let mut missing = Vec::new();
        for &(kind, program) in solvers {
            if running.len() == 2 {
                break;
            }
            // The second solver gets longer, to confirm what the first found fast.
            let limit = if running.is_empty() {
                timeout_ms
            } else {
                timeout_ms * CONFIRM_FACTOR
            };
            match Process::start(kind, program, limit) {
                Ok(process) => running.push(process),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    missing.push(format!("{} isn't installed", kind.name()));
                }
                Err(error) => missing.push(format!("{} couldn't start: {error}", kind.name())),
            }
        }
        let mut running = running.into_iter();
        let first = running
            .next()
            .ok_or_else(|| io::Error::other(missing.join("; ")))?;
        let second = running.next();
        let alone = second.is_none().then(|| {
            if missing.is_empty() {
                "no second solver was asked for".to_owned()
            } else {
                missing.join("; ")
            }
        });
        let log = std::env::var_os("LENGUA_SMT_LOG").and_then(|folder| {
            let session = SESSIONS.fetch_add(1, Ordering::Relaxed);
            let name = format!("{}-{session}.smt2", std::process::id());
            let mut file = std::fs::File::create(std::path::Path::new(&folder).join(name)).ok()?;
            file.write_all(b"(set-logic QF_BV)\n").ok()?;
            Some(file)
        });
        Ok(Self {
            first,
            second,
            alone,
            log,
            defined: Vec::new(),
            questions: 0,
            overruled: 0,
            why_unknown: String::new(),
            timeout_ms,
            deadline: None,
            divisions: HashMap::new(),
        })
    }

    /// Stops asking after `deadline`: every question after it is unknown.
    pub const fn until(&mut self, deadline: std::time::Instant) {
        self.deadline = Some(deadline);
    }

    /// Forgets every term and division defined so far, so the processes can serve terms from a
    /// new pool without being started again.
    ///
    /// # Errors
    ///
    /// Fails when a solver stops responding.
    pub fn reset(&mut self) -> io::Result<()> {
        self.note("(reset)\n(set-logic QF_BV)\n");
        self.first.reset()?;
        if let Some(second) = &mut self.second {
            second.reset()?;
        }
        self.defined.clear();
        self.divisions.clear();
        self.why_unknown.clear();
        Ok(())
    }

    /// Whether the deadline has passed.
    #[must_use]
    pub fn out_of_time(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
    }

    /// The time limit for a question that may take `most` milliseconds, cut to what's left
    /// before the deadline.
    fn limit(&self, most: u64) -> u64 {
        self.deadline.map_or(most, |deadline| {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            most.min(u64::try_from(left.as_millis()).unwrap_or(u64::MAX))
        })
    }

    /// Sends `text` to every solver.
    fn send(&mut self, text: &str) -> io::Result<()> {
        if let Some(log) = &mut self.log {
            let _ = log.write_all(text.as_bytes());
        }
        self.first.send(text)?;
        if let Some(second) = &mut self.second {
            second.send(text)?;
        }
        Ok(())
    }

    /// Defines `term` and everything it's made of, once each.
    fn define(&mut self, pool: &Pool, term: Term) -> io::Result<()> {
        if self.defined.len() < pool.len() {
            self.defined.resize(pool.len(), false);
        }
        let mut text = String::new();
        let mut pending = vec![(term, false)];
        while let Some((current, ready)) = pending.pop() {
            if self.defined[current.index()] {
                continue;
            }
            if !ready {
                pending.push((current, true));
                for operand in pool.operands(current) {
                    if !self.defined[operand.index()] {
                        pending.push((operand, false));
                    }
                }
                continue;
            }
            self.defined[current.index()] = true;
            let sort = sort_text(pool.sort(current));
            match pool.node(current) {
                Node::Var { width, .. } => {
                    text.push_str(&format!("(declare-const t{} {sort})\n", current.index()));
                    // The solver must know every range the terms were simplified with.
                    let (low, high) = pool.range(current);
                    let full = if *width >= 128 {
                        u128::MAX
                    } else {
                        (1 << width) - 1
                    };
                    if (low, high) != (0, full) {
                        let literal = |value: u128| {
                            if width.is_multiple_of(4) {
                                format!("#x{value:0digits$x}", digits = (width / 4) as usize)
                            } else {
                                format!("#b{value:0digits$b}", digits = *width as usize)
                            }
                        };
                        text.push_str(&format!(
                            "(assert (bvule {} t{index}))\n(assert (bvule t{index} {}))\n",
                            literal(low),
                            literal(high),
                            index = current.index()
                        ));
                    }
                }
                Node::BoolVar(_) => {
                    text.push_str(&format!("(declare-const t{} {sort})\n", current.index()));
                }
                // z3's bit-blaster can't divide, so every division is a quotient and a remainder
                // that multiply back.
                &Node::Udiv(dividend, divisor) | &Node::Urem(dividend, divisor) => {
                    let division = match pool.constant(divisor) {
                        Some(constant) if constant > 0 => Division::ByConstant(dividend, constant),
                        _ => Division::ByTerm(dividend, divisor),
                    };
                    let (quotient, remainder) = self.division(pool, division, &mut text);
                    let name = if matches!(pool.node(current), Node::Udiv(..)) {
                        quotient
                    } else {
                        remainder
                    };
                    text.push_str(&format!(
                        "(define-fun t{} () {sort} {name})\n",
                        current.index()
                    ));
                }
                &Node::Sdiv(dividend, divisor) | &Node::Srem(dividend, divisor) => {
                    // As SMT-LIB defines them: divide the sizes, then the quotient is negative
                    // when the signs differ, and the remainder takes the dividend's sign.
                    let (quotient, remainder) =
                        self.division(pool, Division::Signed(dividend, divisor), &mut text);
                    let width = pool.width(current);
                    let (zero, one) = (literal(0, width), literal(1, width));
                    let (a, b) = (dividend.index(), divisor.index());
                    let value = if matches!(pool.node(current), Node::Sdiv(..)) {
                        format!(
                            "(ite (xor (bvslt t{a} {zero}) (bvslt t{b} {zero})) (bvadd (bvnot \
                             {quotient}) {one}) {quotient})"
                        )
                    } else {
                        format!(
                            "(ite (bvslt t{a} {zero}) (bvadd (bvnot {remainder}) {one}) \
                             {remainder})"
                        )
                    };
                    text.push_str(&format!(
                        "(define-fun t{} () {sort} {value})\n",
                        current.index()
                    ));
                }
                &Node::Mul(factor, by) if pool.constant(by).is_some() => {
                    let constant = pool.constant(by).unwrap_or(0);
                    let width = pool.width(current);
                    text.push_str(&format!(
                        "(define-fun t{} () {sort} {})\n",
                        current.index(),
                        shifted_sum(&format!("t{}", factor.index()), constant, width)
                    ));
                }
                _ => text.push_str(&format!(
                    "(define-fun t{} () {sort} {})\n",
                    current.index(),
                    pool.smt_node(current)
                )),
            }
        }
        self.send(&text)
    }

    /// The quotient and remainder of a division, as variables defined by `dividend = quotient ×
    /// divisor + remainder` and `remainder < divisor`, computed in double width so nothing wraps;
    /// dividing by zero gives all ones and the dividend, as in SMT-LIB. A signed division divides
    /// the sizes. The definitions are asserted once, outside any question.
    fn division(&mut self, pool: &Pool, division: Division, text: &mut String) -> (String, String) {
        if let Some(names) = self.divisions.get(&division) {
            return names.clone();
        }
        let index = self.divisions.len();
        let (quotient, remainder) = (format!("q{index}"), format!("r{index}"));
        let (Division::ByConstant(dividend, _)
        | Division::ByTerm(dividend, _)
        | Division::Signed(dividend, _)) = division;
        let width = pool.width(dividend);
        let wide = |name: &str| format!("((_ zero_extend {width}) {name})");
        let size = |term: Term| {
            format!(
                "(ite (bvslt t{index} {zero}) (bvadd (bvnot t{index}) {one}) t{index})",
                index = term.index(),
                zero = literal(0, width),
                one = literal(1, width),
            )
        };
        text.push_str(&format!(
            "(declare-const {quotient} (_ BitVec {width}))\n(declare-const {remainder} (_ BitVec \
             {width}))\n"
        ));
        match division {
            Division::ByConstant(dividend, divisor) => text.push_str(&format!(
                "(assert (= {} (bvadd {} {})))\n(assert (bvult {remainder} {}))\n",
                wide(&format!("t{}", dividend.index())),
                shifted_sum(&wide(&quotient), divisor, 2 * width),
                wide(&remainder),
                literal(divisor, width),
            )),
            Division::ByTerm(dividend, divisor) | Division::Signed(dividend, divisor) => {
                let (dividend, divisor) = if matches!(division, Division::Signed(..)) {
                    let (a, b) = (format!("a{index}"), format!("b{index}"));
                    text.push_str(&format!(
                        "(define-fun {a} () (_ BitVec {width}) {})\n(define-fun {b} () (_ \
                         BitVec {width}) {})\n",
                        size(dividend),
                        size(divisor)
                    ));
                    (a, b)
                } else {
                    (
                        format!("t{}", dividend.index()),
                        format!("t{}", divisor.index()),
                    )
                };
                text.push_str(&format!(
                    "(assert (ite (= {divisor} {zero}) (and (= {quotient} {all}) (= {remainder} \
                     {dividend})) (and (= {} (bvadd (bvmul {} {}) {})) (bvult {remainder} \
                     {divisor}))))\n",
                    wide(&dividend),
                    wide(&quotient),
                    wide(&divisor),
                    wide(&remainder),
                    zero = literal(0, width),
                    all = literal(ones(width), width),
                ));
            }
        }
        self.divisions
            .insert(division, (quotient.clone(), remainder.clone()));
        (quotient, remainder)
    }

    /// Writes `text` to the log, when there is one.
    fn note(&mut self, text: &str) {
        if let Some(log) = &mut self.log {
            let _ = log.write_all(text.as_bytes());
        }
    }

    /// Whether some input meets every condition in `conditions`. With `values`, also returns
    /// those terms' values for one such input, by term. The conditions are impossible only when
    /// every solver says so, and inputs a solver finds count only when they meet the conditions.
    ///
    /// # Errors
    ///
    /// Fails when a solver stops responding or rejects a question.
    pub fn check(
        &mut self,
        pool: &Pool,
        conditions: &[Term],
        values: &[Term],
    ) -> io::Result<(Answer, HashMap<Term, u128>)> {
        self.ask(pool, conditions, values, false)
    }

    /// Whether some input meets every condition in `conditions`, and when the solver found one,
    /// every variable's value in it, checked against the conditions.
    ///
    /// # Errors
    ///
    /// Fails when a solver stops responding or rejects a question.
    pub fn witness(
        &mut self,
        pool: &Pool,
        conditions: &[Term],
    ) -> io::Result<(Answer, HashMap<Term, u128>)> {
        self.ask(pool, conditions, &[], true)
    }

    fn ask(
        &mut self,
        pool: &Pool,
        conditions: &[Term],
        values: &[Term],
        model_wanted: bool,
    ) -> io::Result<(Answer, HashMap<Term, u128>)> {
        // A condition the terms already settle as false needs no solver, and one they settle
        // as true says nothing.
        if conditions
            .iter()
            .any(|condition| pool.truth(*condition) == Some(false))
        {
            return Ok((Answer::Unsat, HashMap::new()));
        }
        let unsettled: Vec<Term> = conditions
            .iter()
            .copied()
            .filter(|condition| pool.truth(*condition).is_none())
            .collect();
        let conditions = unsettled.as_slice();
        // A condition and its negation can't both hold, whatever they say: a run's own branch
        // and the check that its output breaks come to that when both sides are one term.
        if conditions.iter().any(|condition| {
            matches!(*pool.node(*condition), Node::BoolNot(inner)
                if conditions.iter().any(|other| pool.equivalent(*other, inner)))
        }) {
            return Ok((Answer::Unsat, HashMap::new()));
        }
        self.questions += 1;
        if self.out_of_time() {
            self.why_unknown = "the proof ran out of time".into();
            return Ok((Answer::Unknown, HashMap::new()));
        }
        for term in conditions.iter().chain(values) {
            self.define(pool, *term)?;
        }
        let mut text = String::from("(push 1)\n");
        for condition in conditions {
            text.push_str(&format!("(assert t{})\n", condition.index()));
        }
        self.send(&text)?;
        let asked = std::time::Instant::now();
        let first = self.first.check_sat(self.limit(self.timeout_ms))?;
        let name = self.first.kind.name();
        self.note(&format!(
            "(check-sat)\n; {name}: {first:?} in {:?}\n",
            asked.elapsed()
        ));
        let mut model = HashMap::new();
        let answer = match first {
            Answer::Sat if values.is_empty() && !model_wanted => Answer::Sat,
            Answer::Sat => {
                if let Some(found) = self.first.model(pool, conditions, values)? {
                    model = found;
                    Answer::Sat
                } else {
                    self.why_unknown =
                        format!("{name} found inputs that don't meet the conditions");
                    Answer::Unknown
                }
            }
            Answer::Unsat => self.confirm(pool, conditions, values, &mut model)?,
            Answer::Unknown => {
                self.why_unknown = format!("{name} {}", self.first.reason()?);
                Answer::Unknown
            }
        };
        self.send("(pop 1)\n")?;
        if first != Answer::Unsat
            && let Some(second) = &mut self.second
        {
            // Kept in step, so a question it can't read shows at once.
            second.sync()?;
        }
        Ok((answer, model))
    }

    /// The second solver's view of conditions the first found impossible: impossible when it
    /// agrees, and possible when it finds inputs that really meet them all, which `model` then
    /// holds.
    fn confirm(
        &mut self,
        pool: &Pool,
        conditions: &[Term],
        values: &[Term],
        model: &mut HashMap<Term, u128>,
    ) -> io::Result<Answer> {
        let first = self.first.kind.name();
        let limit = self.limit(self.timeout_ms * CONFIRM_FACTOR);
        let Some(second) = &mut self.second else {
            return Ok(Answer::Unsat);
        };
        let name = second.kind.name();
        let asked = std::time::Instant::now();
        let answer = second.check_sat(limit)?;
        let elapsed = asked.elapsed();
        let (answer, why) = match answer {
            Answer::Unsat => (Answer::Unsat, String::new()),
            Answer::Sat => match second.model(pool, conditions, values)? {
                Some(found) => {
                    *model = found;
                    self.overruled += 1;
                    (Answer::Sat, String::new())
                }
                None => (Answer::Unknown, format!("{first} and {name} disagree")),
            },
            Answer::Unknown => (
                Answer::Unknown,
                format!(
                    "{name} couldn't confirm {first}'s answer: it {}",
                    second.reason()?
                ),
            ),
        };
        self.note(&format!("; {name}: {answer:?} in {elapsed:?}\n"));
        if answer == Answer::Unknown {
            self.why_unknown = why;
        }
        Ok(answer)
    }
}

/// `value × constant` as a sum of shifted copies, `8x + 2x` for `10x`, which bit-blasting turns
/// into wiring and adders instead of a whole multiplier.
fn shifted_sum(value: &str, constant: u128, width: u32) -> String {
    let literal = |number: u128| {
        if width.is_multiple_of(4) {
            format!("#x{number:0digits$x}", digits = (width / 4) as usize)
        } else {
            format!("#b{number:0digits$b}", digits = width as usize)
        }
    };
    let parts: Vec<String> = (0..width.min(128))
        .filter(|bit| constant >> bit & 1 == 1)
        .map(|bit| {
            if bit == 0 {
                value.to_owned()
            } else {
                format!("(bvshl {value} {})", literal(u128::from(bit)))
            }
        })
        .collect();
    match parts.as_slice() {
        [] => literal(0),
        [only] => only.clone(),
        [first, rest @ ..] => rest
            .iter()
            .fold(first.clone(), |sum, part| format!("(bvadd {sum} {part})")),
    }
}

/// Reads `((t1 #x0a) (t2 true) …)` into values by term.
fn parse_values(text: &str) -> HashMap<Term, u128> {
    let mut values = HashMap::new();
    let cleaned = text.replace(['(', ')'], " ");
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();
    let mut index = 0;
    while index + 1 < tokens.len() {
        let Some(number) = tokens[index]
            .strip_prefix('t')
            .and_then(|digits| digits.parse::<u32>().ok())
        else {
            index += 1;
            continue;
        };
        let value = tokens[index + 1];
        let parsed = if let Some(hex) = value.strip_prefix("#x") {
            u128::from_str_radix(hex, 16).ok()
        } else if let Some(bits) = value.strip_prefix("#b") {
            u128::from_str_radix(bits, 2).ok()
        } else {
            match value {
                "true" => Some(1),
                "false" => Some(0),
                _ => None,
            }
        };
        if let Some(parsed) = parsed {
            values.insert(super::term::term_at(number), parsed);
            index += 2;
        } else {
            index += 1;
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every pairing of the installed solvers: z3 checked by cvc5, and each alone.
    fn solvers() -> Vec<Solver> {
        [
            &[(Kind::Z3, "z3"), (Kind::Cvc5, "cvc5")][..],
            &[(Kind::Z3, "z3")],
            &[(Kind::Cvc5, "cvc5")],
        ]
        .iter()
        .filter_map(|solvers| Solver::with(solvers, 5000).ok())
        .collect()
    }

    #[test]
    fn answers_and_gives_models() {
        for mut solver in solvers() {
            let mut pool = Pool::default();
            let a = pool.var(64, "a");
            let ten = pool.bv(64, 10);
            let product = pool.mul(a, ten);
            let seventy = pool.bv(64, 70);
            let is_seventy = pool.eq(product, seventy);
            let (answer, model) = solver
                .check(&pool, &[is_seventy], &[a])
                .expect("the solver answers");
            assert_eq!(answer, Answer::Sat);
            let found = model.get(&a).copied().expect("a value for a");
            assert_eq!(found.wrapping_mul(10) & u128::from(u64::MAX), 70);
            // x - 48 widened is x widened minus 48 when x is at least 48: the query the review
            // saw z3 4.16 get wrong when written with bvsub.
            let byte = pool.var(8, "b");
            let zero_char = pool.bv(8, 0x30);
            let at_least = pool.ule(zero_char, byte);
            let wide = pool.zero_extend(byte, 24);
            let forty_eight = pool.bv(32, 0x30);
            let narrow_difference = pool.sub(wide, forty_eight);
            let left = pool.zero_extend(narrow_difference, 32);
            let wide64 = pool.zero_extend(byte, 56);
            let forty_eight64 = pool.bv(64, 0x30);
            let right = pool.sub(wide64, forty_eight64);
            let same = pool.eq(left, right);
            let differs = pool.bool_not(same);
            let (answer, _) = solver
                .check(&pool, &[at_least, differs], &[])
                .expect("the solver answers");
            assert_eq!(answer, Answer::Unsat);
            assert_eq!(solver.questions, 2);
            assert_eq!(solver.overruled, 0);
        }
    }

    /// A stand-in solver that reads commands and answers `answer` to every question, with an
    /// empty model.
    fn fake(folder: &std::path::Path, answer: &str) -> String {
        use std::os::unix::fs::PermissionsExt as _;
        let path = folder.join(format!("always-{answer}"));
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\nwhile read -r line; do\n  case \"$line\" in\n    \"(echo \"*) echo \
                 '{MARKER}' ;;\n    \"(check-sat\"*) echo {answer} ;;\n    \"(get-value\"*) echo \
                 '()' ;;\n  esac\ndone\n"
            ),
        )
        .expect("write the stand-in");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("make it runnable");
        // A process another test forked while the file was open for writing keeps it busy until
        // that process execs; once one run starts, none can.
        for _ in 0..100 {
            match Command::new(&path).stdin(Stdio::null()).status() {
                Err(error) if error.raw_os_error() == Some(26) => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                _ => break,
            }
        }
        path.to_string_lossy().into_owned()
    }

    /// x + 1 = 0 for an 8-bit x: true only for 255.
    fn wraps(pool: &mut Pool) -> (Term, Term) {
        let x = pool.var(8, "x");
        let one = pool.bv(8, 1);
        let next = pool.add(x, one);
        let zero = pool.bv(8, 0);
        (x, pool.eq(next, zero))
    }

    #[test]
    fn the_second_solver_overrules_a_wrong_impossible() {
        let folder = tempfile::tempdir().expect("a folder");
        let liar = fake(folder.path(), "unsat");
        let Ok(mut solver) = Solver::with(&[(Kind::Z3, &liar), (Kind::Cvc5, "cvc5")], 5000) else {
            return;
        };
        if let Some(reason) = &solver.alone {
            eprintln!("{reason}");
            return;
        }
        let mut pool = Pool::default();
        let (x, wraps) = wraps(&mut pool);
        let (answer, model) = solver.check(&pool, &[wraps], &[x]).expect("answers");
        assert_eq!((answer, model.get(&x)), (Answer::Sat, Some(&255)));
        // Without values asked for, the inputs are still checked before they count.
        let (answer, _) = solver.check(&pool, &[wraps], &[]).expect("answers");
        assert_eq!(answer, Answer::Sat);
        assert_eq!(solver.overruled, 2);
        let never = pool.boolean(false);
        let x_is_zero = {
            let zero = pool.bv(8, 0);
            pool.eq(x, zero)
        };
        let (answer, _) = solver
            .check(&pool, &[wraps, x_is_zero], &[])
            .expect("answers");
        assert_eq!(answer, Answer::Unsat);
        let (answer, _) = solver.check(&pool, &[never], &[]).expect("answers");
        assert_eq!(answer, Answer::Unsat);
    }

    #[test]
    fn impossible_needs_both_solvers() {
        let folder = tempfile::tempdir().expect("a folder");
        let doubter = fake(folder.path(), "sat");
        let Ok(mut solver) = Solver::with(&[(Kind::Z3, "z3"), (Kind::Cvc5, &doubter)], 5000) else {
            return;
        };
        if solver.second.is_none() {
            eprintln!("z3 isn't installed");
            return;
        }
        let mut pool = Pool::default();
        let (x, wraps) = wraps(&mut pool);
        let zero = pool.bv(8, 0);
        let x_is_zero = pool.eq(x, zero);
        // z3 rightly says impossible, but the second solver claims inputs that don't hold up.
        let (answer, _) = solver
            .check(&pool, &[wraps, x_is_zero], &[])
            .expect("answers");
        assert_eq!(answer, Answer::Unknown);
        assert!(
            solver.why_unknown.contains("disagree"),
            "{}",
            solver.why_unknown
        );
        let unknown = fake(folder.path(), "unknown");
        let mut solver =
            Solver::with(&[(Kind::Z3, "z3"), (Kind::Cvc5, &unknown)], 5000).expect("starts");
        let (answer, _) = solver
            .check(&pool, &[wraps, x_is_zero], &[])
            .expect("answers");
        assert_eq!(answer, Answer::Unknown);
        assert!(
            solver.why_unknown.contains("couldn't confirm"),
            "{}",
            solver.why_unknown
        );
    }

    #[test]
    fn stops_asking_at_the_deadline() {
        let Ok(mut solver) = Solver::start(5000) else {
            return;
        };
        solver.until(std::time::Instant::now());
        let mut pool = Pool::default();
        let (_, wraps) = wraps(&mut pool);
        let (answer, _) = solver.check(&pool, &[wraps], &[]).expect("answers");
        assert_eq!(answer, Answer::Unknown);
        assert!(solver.out_of_time());
        assert_eq!(solver.why_unknown, "the proof ran out of time");
    }

    #[test]
    fn a_reset_solver_answers_new_terms() {
        for mut solver in solvers() {
            let mut pool = Pool::default();
            let (x, wraps) = wraps(&mut pool);
            let (answer, model) = solver.witness(&pool, &[wraps]).expect("answers");
            assert_eq!((answer, model.get(&x)), (Answer::Sat, Some(&255)));
            // A division leaves its quotient and remainder behind.
            let ten = pool.bv(8, 10);
            let quotient = pool.udiv(x, ten);
            let thirty = pool.bv(8, 30);
            let over = pool.ult(thirty, quotient);
            let (answer, _) = solver.check(&pool, &[over], &[]).expect("answers");
            assert_eq!(answer, Answer::Unsat);
            solver.reset().expect("resets");
            // A new pool's terms reuse the same names, and its division the same quotient.
            let mut pool = Pool::default();
            let y = pool.var(8, "y");
            let ten = pool.bv(8, 10);
            let quotient = pool.udiv(y, ten);
            let twenty = pool.bv(8, 20);
            let is_twenty = pool.eq(quotient, twenty);
            let (answer, model) = solver.witness(&pool, &[is_twenty]).expect("answers");
            assert_eq!(answer, Answer::Sat);
            assert_eq!(model.get(&y).map(|y| y / 10), Some(20));
            let fifty = pool.bv(8, 50);
            let never = pool.eq(quotient, fifty);
            let (answer, _) = solver.check(&pool, &[never], &[]).expect("answers");
            assert_eq!(answer, Answer::Unsat);
        }
    }

    #[test]
    fn settled_conditions_need_no_solver() {
        let Ok(mut solver) = Solver::start(5000) else {
            return;
        };
        let mut pool = Pool::default();
        let (_, wraps) = wraps(&mut pool);
        let never = pool.boolean(false);
        let (answer, _) = solver.check(&pool, &[wraps, never], &[]).expect("answers");
        assert_eq!(answer, Answer::Unsat);
        assert_eq!(solver.questions, 0);
    }

    #[test]
    fn says_why_a_solver_answers_alone() {
        let Ok(solver) = Solver::with(&[(Kind::Z3, "z3"), (Kind::Cvc5, "/nowhere/cvc5")], 5000)
        else {
            return;
        };
        assert_eq!(solver.alone.as_deref(), Some("cvc5 isn't installed"));
        let error = Solver::with(&[(Kind::Z3, "/nowhere/z3")], 5000)
            .err()
            .expect("no solver");
        assert_eq!(error.to_string(), "z3 isn't installed");
    }
}
