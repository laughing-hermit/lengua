//! Runs a program's tests: every example, then every generated block with fresh inputs from a
//! new seed, boundary values first. Failures say what was expected and what happened, down to
//! the line of a failed assertion.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::checker::source::{Item, Operand, Source};
use crate::runner::{self, End, Limits, Run};
use crate::spec::ASSERTION_EXIT_CODE;
use crate::testfile::{TestSuite, describe_inputs};

/// The longest a program's tests may take together.
pub const TOTAL_TIMEOUT: Duration = Duration::from_secs(90);

/// At most this many failures are reported.
pub const MAX_FAILURES: usize = 10;

/// The smallest, middle and largest of a measurement over runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Spread {
    /// The smallest.
    pub min: u64,
    /// The median.
    pub median: u64,
    /// The largest.
    pub max: u64,
}

impl Spread {
    fn of(mut values: Vec<u64>) -> Option<Self> {
        values.sort_unstable();
        Some(Self {
            min: *values.first()?,
            median: values[values.len() / 2],
            max: *values.last()?,
        })
    }
}

/// What one test, or all runs of a generated block, cost. Each measure is missing when the
/// system doesn't allow the CPU's performance counters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Cost {
    /// The test, such as `example B1` or `generated B1`.
    pub test: String,
    /// Runs measured.
    pub runs: usize,
    /// Instructions executed in the program's own code: the steadiest measure of its work.
    pub instructions: Option<Spread>,
    /// CPU cycles in the program's own code: its time, with slow instructions such as division
    /// weighing more.
    pub cycles: Option<Spread>,
    /// CPU time in microseconds, including what the kernel does for its system calls.
    pub cpu_time_us: Option<Spread>,
}

/// Measurements collected over runs.
#[derive(Debug, Default)]
struct Samples {
    runs: usize,
    instructions: Vec<u64>,
    cycles: Vec<u64>,
    cpu_time_us: Vec<u64>,
}

impl Samples {
    fn add(&mut self, run: &Run) {
        self.runs += 1;
        self.instructions.extend(run.instructions);
        self.cycles.extend(run.cycles);
        self.cpu_time_us
            .extend(run.cpu_time_ns.map(|nanoseconds| nanoseconds / 1000));
    }

    fn cost(self, test: String) -> Cost {
        Cost {
            test,
            runs: self.runs,
            instructions: Spread::of(self.instructions),
            cycles: Spread::of(self.cycles),
            cpu_time_us: Spread::of(self.cpu_time_us),
        }
    }
}

/// What running the tests found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Runs made.
    pub runs: usize,
    /// Failures, at most [`MAX_FAILURES`].
    pub failures: Vec<String>,
    /// Instruction counts per test, when the CPU's counters are available.
    pub costs: Vec<Cost>,
}

impl Outcome {
    /// Whether every test passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

/// The source line of each assertion, by number.
#[must_use]
pub fn assertion_lines(source: &Source) -> BTreeMap<i64, usize> {
    let mut lines = BTreeMap::new();
    for function in &source.functions {
        let instructions: Vec<_> = function
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Instruction(instruction) => Some(instruction),
                Item::Label(_) => None,
            })
            .collect();
        for pair in instructions.windows(2) {
            let calls_assert = pair[1].mnemonic == "call"
                && pair[1].operands == [Operand::Target("assert_fail".into())];
            if let (
                true,
                [
                    Operand::Register(_),
                    Operand::Immediate { value, name: None },
                ],
            ) = (calls_assert, pair[0].operands.as_slice())
            {
                lines.insert(*value, pair[0].line);
            }
        }
    }
    lines
}

/// Shows bytes as a quoted string, shortened when long.
fn show(bytes: &[u8]) -> String {
    const LIMIT: usize = 200;
    let text = String::from_utf8_lossy(bytes);
    if text.chars().count() > LIMIT {
        let short: String = text.chars().take(LIMIT).collect();
        format!("{short:?}… ({} bytes)", bytes.len())
    } else {
        format!("{text:?}")
    }
}

/// How a run ended, naming the line of a failed assertion.
#[must_use]
pub fn explain_end(run: &Run, assertions: &BTreeMap<i64, usize>, limits: Limits) -> String {
    match run.end {
        End::Exited(code) if code == i32::from(ASSERTION_EXIT_CODE) => {
            compare(run, "", "", 0, assertions).unwrap_or_else(|| run.end.describe(limits))
        }
        other => other.describe(limits),
    }
}

/// Why a run didn't match, or `None` when it did.
fn compare(
    run: &Run,
    stdout: &str,
    stderr: &str,
    exit_code: u8,
    assertions: &BTreeMap<i64, usize>,
) -> Option<String> {
    let code = match run.end {
        End::Exited(code) => code,
        other => {
            return Some(format!(
                "the program was {}",
                other.describe(Limits::default())
            ));
        }
    };
    if code == i32::from(ASSERTION_EXIT_CODE) {
        let text = String::from_utf8_lossy(&run.stderr);
        let number = text
            .lines()
            .rev()
            .find_map(|line| line.strip_prefix("lengua: assertion "))
            .and_then(|rest| rest.strip_suffix(" failed"))
            .and_then(|digits| digits.parse::<i64>().ok());
        return Some(match number {
            Some(number) => match assertions.get(&number) {
                Some(line) => format!("assertion {number} failed at main.s:{line}"),
                None => format!("assertion {number} failed"),
            },
            None => "it exited with 134, the failed-assertion code".to_owned(),
        });
    }
    let mut problems = Vec::new();
    if run.stdout != stdout.as_bytes() {
        problems.push(format!(
            "stdout should be {}, but was {}",
            show(stdout.as_bytes()),
            show(&run.stdout)
        ));
    }
    if run.stderr != stderr.as_bytes() {
        problems.push(format!(
            "stderr should be {}, but was {}",
            show(stderr.as_bytes()),
            show(&run.stderr)
        ));
    }
    if code != i32::from(exit_code) {
        problems.push(format!(
            "the exit code should be {exit_code}, but was {code}"
        ));
    }
    (!problems.is_empty()).then(|| problems.join("; "))
}

fn record(outcome: &mut Outcome, message: String) {
    if outcome.failures.len() < MAX_FAILURES {
        outcome.failures.push(message);
    }
}

/// Runs every test of `suite` against `program`, drawing generated inputs from `seed`.
///
/// # Errors
///
/// Fails when the program can't be started at all.
pub async fn run_tests(
    suite: &TestSuite,
    launcher: &Path,
    program: &Path,
    source: &Source,
    seed: u64,
) -> io::Result<Outcome> {
    let assertions = assertion_lines(source);
    let deadline = Instant::now() + TOTAL_TIMEOUT;
    let limits = Limits::default();
    let mut outcome = Outcome::default();
    for example in &suite.examples {
        if Instant::now() > deadline {
            record(
                &mut outcome,
                format!("the tests took longer than {TOTAL_TIMEOUT:?}"),
            );
            return Ok(outcome);
        }
        let run = runner::run(
            launcher,
            program,
            &example.args,
            example.stdin.as_bytes(),
            limits,
        )
        .await?;
        outcome.runs += 1;
        let name = format!("example {}", example.id);
        match compare(
            &run,
            &example.stdout,
            &example.stderr,
            example.exit_code,
            &assertions,
        ) {
            Some(problem) => {
                let mut message = format!("{name} (tests.toml line {}): {problem}", example.line);
                if !example.args.is_empty() {
                    let _ = write!(message, " (args {:?})", example.args);
                }
                record(&mut outcome, message);
            }
            None => {
                let mut samples = Samples::default();
                samples.add(&run);
                outcome.costs.push(samples.cost(name));
            }
        }
    }
    for block in &suite.generated {
        let name = format!("generated {}", block.id);
        let cases = match block.cases(seed) {
            Ok(cases) => cases,
            Err((inputs, field, error)) => {
                record(
                    &mut outcome,
                    format!(
                        "{name} (tests.toml line {}, seed {seed}): with {}, the expected {field} \
                         {error}",
                        block.line,
                        describe_inputs(&inputs)
                    ),
                );
                continue;
            }
        };
        let total = cases.len();
        let mut samples = Samples::default();
        let mut failed = false;
        for (number, case) in cases.iter().enumerate() {
            if Instant::now() > deadline {
                record(
                    &mut outcome,
                    format!("the tests took longer than {TOTAL_TIMEOUT:?}"),
                );
                return Ok(outcome);
            }
            let run =
                runner::run(launcher, program, &case.args, case.stdin.as_bytes(), limits).await?;
            outcome.runs += 1;
            if let Some(problem) = compare(
                &run,
                &case.stdout,
                &case.stderr,
                case.exit_code,
                &assertions,
            ) {
                record(
                    &mut outcome,
                    format!(
                        "{name} (tests.toml line {}), run {} of {total} (seed {seed}): with {}: \
                         {problem}",
                        block.line,
                        number + 1,
                        describe_inputs(&case.inputs)
                    ),
                );
                failed = true;
                break;
            }
            samples.add(&run);
        }
        if !failed {
            outcome.costs.push(samples.cost(name));
        }
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finished(stdout: &str, stderr: &str, code: i32) -> Run {
        Run {
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
            end: End::Exited(code),
            instructions: Some(100),
            cycles: Some(300),
            cpu_time_ns: Some(1000),
        }
    }

    #[test]
    fn compares_every_stream_and_the_exit_code() {
        let none = BTreeMap::new();
        assert_eq!(compare(&finished("5\n", "", 0), "5\n", "", 0, &none), None);
        let wrong = compare(&finished("6\n", "oops\n", 1), "5\n", "", 0, &none).expect("differs");
        assert!(
            wrong.contains("stdout should be \"5\\n\", but was \"6\\n\""),
            "{wrong}"
        );
        assert!(
            wrong.contains("stderr should be \"\", but was \"oops\\n\""),
            "{wrong}"
        );
        assert!(
            wrong.contains("the exit code should be 0, but was 1"),
            "{wrong}"
        );
    }

    #[test]
    fn names_the_line_of_a_failed_assertion() {
        let lines = BTreeMap::from([(7, 42)]);
        let run = finished("", "lengua: assertion 007 failed\n", 134);
        assert_eq!(
            compare(&run, "", "", 0, &lines),
            Some("assertion 7 failed at main.s:42".to_owned())
        );
        let killed = Run {
            end: End::Signaled(11),
            ..finished("", "", 0)
        };
        assert!(
            compare(&killed, "", "", 0, &lines).is_some_and(|problem| problem.contains("SIGSEGV"))
        );
    }

    #[test]
    fn finds_assertion_lines_in_the_source() {
        let (source, _) = crate::checker::source::parse(crate::checker::TEMPLATE);
        let lines = assertion_lines(&source);
        assert_eq!(lines.len(), 11);
        let line = lines[&4];
        let text = crate::checker::TEMPLATE
            .lines()
            .nth(line - 1)
            .expect("the line");
        assert_eq!(text.trim(), "mov edi, 4");
    }

    #[test]
    fn summarizes_measurements() {
        assert_eq!(
            Spread::of(vec![30, 10, 20]),
            Some(Spread {
                min: 10,
                median: 20,
                max: 30
            })
        );
        assert_eq!(Spread::of(Vec::new()), None);
        let mut samples = Samples::default();
        samples.add(&finished("", "", 0));
        samples.add(&Run {
            instructions: None,
            ..finished("", "", 0)
        });
        let cost = samples.cost("generated B1".into());
        assert_eq!(cost.runs, 2);
        assert_eq!(cost.instructions.map(|spread| spread.median), Some(100));
        assert_eq!(cost.cpu_time_us.map(|spread| spread.max), Some(1));
    }
}
