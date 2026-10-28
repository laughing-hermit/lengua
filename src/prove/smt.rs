//! A z3 process the prover talks to in SMT-LIB 2, over a pipe: terms are defined once, and each
//! question is asked inside its own `push`/`pop`, so the definitions stay and the assertions go.

use std::collections::HashMap;
use std::io::{self, BufRead as _, BufReader, Write as _};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

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

/// The line z3 prints back to mark the end of each batch of commands.
const MARKER: &str = "@@lengua@@";

/// A running solver.
pub struct Solver {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    /// Everything sent, when `LENGUA_SMT_LOG` names a file to keep it in.
    log: Option<std::fs::File>,
    defined: Vec<bool>,
    /// The solver's answers so far, for the report.
    pub questions: usize,
    timeout_ms: u64,
    /// The quotient and remainder variables standing for each division by a constant.
    divisions: HashMap<(Term, u128), (String, String)>,
}

/// The z3 strategy: bit-blast straight into the SAT solver. z3 4.16's own simplifier gets some
/// bit-vector formulas wrong (it called unsatisfiable formulas satisfiable, and the reverse), so
/// lengua simplifies terms itself and uses z3 only for bit-blasting and SAT. Division, which the
/// bit-blaster can't do alone, is defined by [`Solver::division`].
const STRATEGY: &str = "(then bit-blast sat)";

impl Drop for Solver {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn sort_text(sort: Sort) -> String {
    match sort {
        Sort::Bool => "Bool".into(),
        Sort::Bv(width) => format!("(_ BitVec {width})"),
    }
}

impl Solver {
    /// Starts z3, which gives each question at most `timeout_ms`.
    ///
    /// # Errors
    ///
    /// Fails when z3 isn't installed or can't start.
    pub fn start(timeout_ms: u64) -> io::Result<Self> {
        let mut child = Command::new("z3")
            .args(["-in", "-smt2", "-memory:4096"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("z3 has no input"))?;
        let output = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| io::Error::other("z3 has no output"))?,
        );
        let log =
            std::env::var_os("LENGUA_SMT_LOG").and_then(|path| std::fs::File::create(path).ok());
        let mut solver = Self {
            child,
            input,
            output,
            log,
            defined: Vec::new(),
            questions: 0,
            timeout_ms,
            divisions: HashMap::new(),
        };
        solver.send(
            "(set-option :print-success false)\n(set-option :produce-models true)\n\
             (set-logic QF_BV)\n",
        )?;
        solver.sync()?;
        Ok(solver)
    }

    fn send(&mut self, text: &str) -> io::Result<()> {
        if let Some(log) = &mut self.log {
            let _ = log.write_all(text.as_bytes());
        }
        self.input.write_all(text.as_bytes())?;
        self.input.flush()
    }

    /// Waits until the solver has answered everything sent so far; returns what it printed.
    fn sync(&mut self) -> io::Result<Vec<String>> {
        self.send(&format!("(echo \"{MARKER}\")\n"))?;
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            if self.output.read_line(&mut line)? == 0 {
                return Err(io::Error::other("z3 stopped"));
            }
            let line = line.trim().to_owned();
            if line == MARKER {
                return Ok(lines);
            }
            lines.push(line);
        }
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
                &Node::Udiv(dividend, divisor) | &Node::Urem(dividend, divisor)
                    if pool.constant(divisor).is_some_and(|c| c > 0) =>
                {
                    let constant = pool.constant(divisor).unwrap_or(1);
                    let (quotient, remainder) = self.division(pool, dividend, constant, &mut text);
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

    /// The quotient and remainder of `dividend` by the constant `divisor`, as variables defined
    /// by `dividend = quotient × divisor + remainder` and `remainder < divisor`, computed in
    /// double width so nothing wraps. The definitions are asserted once, outside any question.
    fn division(
        &mut self,
        pool: &Pool,
        dividend: Term,
        divisor: u128,
        text: &mut String,
    ) -> (String, String) {
        if let Some(names) = self.divisions.get(&(dividend, divisor)) {
            return names.clone();
        }
        let width = pool.width(dividend);
        let index = self.divisions.len();
        let (quotient, remainder) = (format!("q{index}"), format!("r{index}"));
        let constant = |value: u128, bits: u32| {
            if bits.is_multiple_of(4) {
                format!("#x{value:0digits$x}", digits = (bits / 4) as usize)
            } else {
                format!("#b{value:0digits$b}", digits = bits as usize)
            }
        };
        let wide = |name: &str| format!("((_ zero_extend {width}) {name})");
        text.push_str(&format!(
            "(declare-const {quotient} (_ BitVec {width}))\n(declare-const {remainder} (_ BitVec \
             {width}))\n(assert (= {} (bvadd {} {})))\n(assert (bvult {remainder} {}))\n",
            wide(&format!("t{}", dividend.index())),
            shifted_sum(&wide(&quotient), divisor, 2 * width),
            wide(&remainder),
            constant(divisor, width),
        ));
        self.divisions
            .insert((dividend, divisor), (quotient.clone(), remainder.clone()));
        (quotient, remainder)
    }

    /// Whether some input meets every condition in `conditions`. With `values`, also returns
    /// those terms' values for one such input, by term.
    ///
    /// # Errors
    ///
    /// Fails when the solver stops responding.
    pub fn check(
        &mut self,
        pool: &Pool,
        conditions: &[Term],
        values: &[Term],
    ) -> io::Result<(Answer, HashMap<Term, u128>)> {
        self.questions += 1;
        for term in conditions.iter().chain(values) {
            self.define(pool, *term)?;
        }
        let mut text = String::from("(push 1)\n");
        for condition in conditions {
            text.push_str(&format!("(assert t{})\n", condition.index()));
        }
        text.push_str(&format!(
            "(check-sat-using (try-for {STRATEGY} {}))\n",
            self.timeout_ms
        ));
        self.send(&text)?;
        let asked = std::time::Instant::now();
        let lines = self.sync()?;
        if let Some(log) = &mut self.log {
            let _ = writeln!(
                log,
                "; answered in {:?}: {:?}",
                asked.elapsed(),
                lines.first()
            );
        }
        let answer = match lines
            .iter()
            .find(|line| !line.is_empty())
            .map(String::as_str)
        {
            Some("sat") if !lines.iter().any(|line| line.starts_with("(error")) => Answer::Sat,
            Some("unsat") => Answer::Unsat,
            _ => Answer::Unknown,
        };
        let mut model = HashMap::new();
        let mut answer = answer;
        if answer == Answer::Sat && !values.is_empty() {
            // Ask for every variable too, so the model can be checked without the solver.
            let variables = pool.variables(conditions);
            let asked: Vec<Term> = values.iter().chain(&variables).copied().collect();
            let names: Vec<String> = asked
                .iter()
                .map(|term| format!("t{}", term.index()))
                .collect();
            self.send(&format!("(get-value ({}))\n", names.join(" ")))?;
            let text = self.sync()?.join(" ");
            model = parse_values(&text);
            let by_name: HashMap<String, u128> = variables
                .iter()
                .filter_map(|term| Some((pool.name(*term)?.to_owned(), *model.get(term)?)))
                .collect();
            let holds = conditions
                .iter()
                .all(|condition| pool.evaluate(*condition, &by_name) == Some(1));
            if !holds {
                answer = Answer::Unknown;
            }
        }
        self.send("(pop 1)\n")?;
        Ok((answer, model))
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

    fn solver() -> Option<Solver> {
        Solver::start(5000).ok()
    }

    #[test]
    fn answers_and_gives_models() {
        let Some(mut solver) = solver() else {
            eprintln!("z3 isn't installed");
            return;
        };
        let mut pool = Pool::default();
        let a = pool.var(64, "a");
        let ten = pool.bv(64, 10);
        let product = pool.mul(a, ten);
        let seventy = pool.bv(64, 70);
        let is_seventy = pool.eq(product, seventy);
        let (answer, model) = solver
            .check(&pool, &[is_seventy], &[a])
            .expect("z3 answers");
        assert_eq!(answer, Answer::Sat);
        let found = model.get(&a).copied().expect("a value for a");
        assert_eq!(found.wrapping_mul(10) & u128::from(u64::MAX), 70);
        // x - 48 widened is x widened minus 48 when x is at least 48: the query the review saw
        // z3 4.16 get wrong when written with bvsub.
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
            .expect("z3 answers");
        assert_eq!(answer, Answer::Unsat);
        assert_eq!(solver.questions, 2);
    }
}
