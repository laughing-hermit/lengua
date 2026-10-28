//! `tests.toml`: one example per behavior with the spec's own values, plus generated tests whose
//! formulas compute the expected results for fresh random inputs on every run.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Deserialize;
use toml::Spanned;

use crate::expr::{self, Env, EvalError, Generator, Limit, PatternPart, Template, Value};
use crate::hash::Sha256Hex;
use crate::spec::{ASSERTION_EXIT_CODE, Behavior, BehaviorId, Spec};

/// The most runs a generated block may ask for.
pub const MAX_RUNS: u32 = 1000;

/// Runs of a generated block that doesn't say.
pub const DEFAULT_RUNS: u32 = 100;

/// At most this many problems are reported at once.
const MAX_PROBLEMS: usize = 20;

/// A checked tests.toml.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestSuite {
    /// One example per behavior, in file order.
    pub examples: Vec<Example>,
    /// The generated blocks, in file order.
    pub generated: Vec<Generated>,
}

/// A test with fixed values taken from the spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Example {
    /// The behavior it checks.
    pub id: BehaviorId,
    /// Line of its `[[example]]` table.
    pub line: usize,
    /// Command-line arguments.
    pub args: Vec<String>,
    /// Standard input.
    pub stdin: String,
    /// Expected standard output.
    pub stdout: String,
    /// Expected standard error.
    pub stderr: String,
    /// Expected exit code.
    pub exit_code: u8,
}

/// A test that draws new inputs on every run and computes the expected results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    /// The behavior it checks.
    pub id: BehaviorId,
    /// Line of its `[[generated]]` table.
    pub line: usize,
    /// How many runs: boundary values first, then random ones.
    pub runs: u32,
    /// The variables and their generators, in name order.
    pub vars: Vec<(String, Generator)>,
    /// Argument templates.
    pub args: Vec<Template>,
    /// Standard input template.
    pub stdin: Template,
    /// Expected standard output template.
    pub stdout: Template,
    /// Expected standard error template.
    pub stderr: Template,
    /// Expected exit code.
    pub exit_code: ExitCode,
    /// Whether every combination of the variables' values runs, instead of a sample.
    pub every_value: bool,
}

/// A generated block runs every combination of its values when there are at most this many…
const EVERY_VALUE_MAX: u128 = 5000;

/// …and the whole suite stays within this many runs, about 30 seconds of tests.
const SUITE_RUNS_MAX: u128 = 20_000;

/// A generated block's expected exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitCode {
    /// The same code on every run.
    Fixed(u8),
    /// A formula such as `"{if(a < 0, 1, 0)}"`.
    Formula(Template),
}

/// One run of a generated block: concrete inputs and the results they should produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// The variables' values.
    pub inputs: Env,
    /// Command-line arguments.
    pub args: Vec<String>,
    /// Standard input.
    pub stdin: String,
    /// Expected standard output.
    pub stdout: String,
    /// Expected standard error.
    pub stderr: String,
    /// Expected exit code.
    pub exit_code: u8,
}

impl Generated {
    /// The expected results for `inputs`.
    ///
    /// # Errors
    ///
    /// Fails when a formula can't be evaluated or gives an impossible exit code.
    pub fn case(&self, inputs: Env) -> Result<Case, (Env, &'static str, EvalError)> {
        match self.results(&inputs) {
            Ok((args, stdin, stdout, stderr, exit_code)) => Ok(Case {
                inputs,
                args,
                stdin,
                stdout,
                stderr,
                exit_code,
            }),
            Err((field, error)) => Err((inputs, field, error)),
        }
    }

    /// The rendered args, stdin, stdout, stderr and exit code for `inputs`.
    #[allow(clippy::type_complexity)]
    fn results(
        &self,
        inputs: &Env,
    ) -> Result<(Vec<String>, String, String, String, u8), (&'static str, EvalError)> {
        let args = self
            .args
            .iter()
            .map(|arg| arg.render(inputs))
            .collect::<Result<_, _>>()
            .map_err(|error| ("args", error))?;
        let stdin = self
            .stdin
            .render(inputs)
            .map_err(|error| ("stdin", error))?;
        let stdout = self
            .stdout
            .render(inputs)
            .map_err(|error| ("stdout", error))?;
        let stderr = self
            .stderr
            .render(inputs)
            .map_err(|error| ("stderr", error))?;
        let exit_code = match &self.exit_code {
            ExitCode::Fixed(code) => *code,
            ExitCode::Formula(template) => {
                exit_code_value(template, inputs).map_err(|error| ("exit_code", error))?
            }
        };
        Ok((args, stdin, stdout, stderr, exit_code))
    }

    /// Every run for `seed`: boundary values first, then random ones.
    ///
    /// # Errors
    ///
    /// Returns the first inputs whose results can't be computed.
    pub fn cases(
        &self,
        seed: u64,
    ) -> impl ExactSizeIterator<Item = Result<Case, (Env, &'static str, EvalError)>> + '_ {
        let inputs = if self.every_value {
            expr::every_case(&self.vars)
        } else {
            expr::cases(&self.vars, self.run_count(), seed)
        };
        inputs.map(|inputs| self.case(inputs))
    }

    /// How many runs the block makes: every combination when [`Self::every_value`], otherwise
    /// `runs`.
    #[must_use]
    pub fn run_count(&self) -> usize {
        if self.every_value {
            expr::combinations(&self.vars)
                .and_then(|count| usize::try_from(count).ok())
                .unwrap_or(0)
        } else {
            usize::try_from(self.runs).unwrap_or(usize::MAX)
        }
    }

    /// When every combination runs, says so in plain words: "every value of n in its range (all
    /// 201)".
    #[must_use]
    pub fn every_value_text(&self) -> Option<String> {
        if !self.every_value {
            return None;
        }
        let names: Vec<&str> = self.vars.iter().map(|(name, _)| name.as_str()).collect();
        let count = self.run_count();
        Some(match names.as_slice() {
            [name] => format!("every value of {name} in its range (all {count})"),
            [rest @ .., last] => format!(
                "every combination of {} and {last} in their ranges (all {count})",
                rest.join(", ")
            ),
            [] => format!("all {count} runs"),
        })
    }
}

fn exit_code_value(template: &Template, inputs: &Env) -> Result<u8, EvalError> {
    match template.value(inputs)? {
        Value::Int(code) => match u8::try_from(code) {
            Ok(ASSERTION_EXIT_CODE) => Err(EvalError::Type(format!(
                "gives exit code {ASSERTION_EXIT_CODE}, which is reserved for failed assertions"
            ))),
            Ok(code) => Ok(code),
            Err(_) => Err(EvalError::Type(format!(
                "gives exit code {code}; exit codes go from 0 to 255"
            ))),
        },
        other => Err(EvalError::Type(format!(
            "must give a number for the exit code, not {other}"
        ))),
    }
}

/// Marks the generated blocks that run every combination of their values: those with at most
/// [`EVERY_VALUE_MAX`] combinations, smallest first, while the suite stays within
/// [`SUITE_RUNS_MAX`] runs. A block with `choice(…)` never is: its values are hand-picked, not a
/// range.
fn mark_every_value(blocks: &mut [Generated]) {
    let mut candidates: Vec<(u128, usize)> = blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| {
            !block
                .vars
                .iter()
                .any(|(_, generator)| matches!(generator, Generator::Choice(_)))
        })
        .filter_map(|(index, block)| {
            let count = expr::combinations(&block.vars)?;
            (count <= EVERY_VALUE_MAX).then_some((count, index))
        })
        .collect();
    candidates.sort_unstable();
    let mut total: u128 = blocks.iter().map(|block| u128::from(block.runs)).sum();
    for (count, index) in candidates {
        let next = total - u128::from(blocks[index].runs) + count;
        if next <= SUITE_RUNS_MAX {
            blocks[index].every_value = true;
            total = next;
        }
    }
}

/// Linux refuses to start a program with an argument longer than this.
pub const MAX_ARG_BYTES: usize = 128 * 1024;

/// lengua keeps a program's arguments together under this, well inside Linux's limit.
pub const MAX_ARGS_BYTES: usize = 1024 * 1024;

/// Why Linux couldn't start a run with these arguments, if it couldn't.
fn oversized_args(case: &Case) -> Option<String> {
    let total: usize = case.args.iter().map(String::len).sum();
    if let Some((index, arg)) = case
        .args
        .iter()
        .enumerate()
        .find(|(_, arg)| arg.len() > MAX_ARG_BYTES)
    {
        return Some(format!(
            "with {}, argument {} is {} bytes, but Linux limits each argument to {MAX_ARG_BYTES} \
             bytes; narrow the range, or pass long text on standard input",
            describe_inputs(&case.inputs),
            index + 1,
            arg.len()
        ));
    }
    (total > MAX_ARGS_BYTES).then(|| {
        format!(
            "with {}, the arguments total {total} bytes, over lengua's limit of {MAX_ARGS_BYTES}; \
             pass long text on standard input",
            describe_inputs(&case.inputs)
        )
    })
}

/// Shows variable values as `a = 2, s = "x"`.
#[must_use]
pub fn describe_inputs(inputs: &Env) -> String {
    inputs
        .iter()
        .map(|(name, value)| format!("{name} = {value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A problem found in tests.toml.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The line it's on, when it belongs to one.
    pub line: Option<usize>,
    /// What's wrong and how to fix it.
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TestsFile {
    meta: Spanned<Meta>,
    #[serde(default)]
    example: Vec<Spanned<ExampleTable>>,
    #[serde(default)]
    generated: Vec<Spanned<GeneratedTable>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Meta {
    spec: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExampleTable {
    id: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    stdin: String,
    #[serde(default)]
    stdout: String,
    #[serde(default)]
    stderr: String,
    exit_code: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedTable {
    id: String,
    runs: Option<i64>,
    vars: BTreeMap<String, String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    stdin: String,
    #[serde(default)]
    stdout: String,
    #[serde(default)]
    stderr: String,
    exit_code: ExitCodeField,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ExitCodeField {
    Number(i64),
    Formula(String),
}

/// Checks tests.toml against the approved spec, reporting every problem found (up to 20).
///
/// `spec_label` is `sha256:<hex>` of the approved spec.md.
///
/// # Errors
///
/// Returns the problems when the tests aren't valid.
pub fn check(text: &str, spec: &Spec, spec_label: &str) -> Result<TestSuite, Vec<Problem>> {
    let mut checker = Checker {
        text,
        spec,
        problems: Vec::new(),
        hidden: 0,
    };
    let file: TestsFile = match toml::from_str(text) {
        Ok(file) => file,
        Err(error) => {
            checker.problem(
                None,
                format!("tests.toml isn't valid: {}", error.to_string().trim()),
            );
            return Err(checker.problems);
        }
    };
    let meta_line = checker.line(file.meta.span().start);
    if file.meta.get_ref().spec != spec_label {
        checker.problem(
            Some(meta_line),
            format!(
                "[meta] says spec = {:?}, but the approved spec is {spec_label:?}. Write the \
                 tests from the current spec.md, then set spec = {spec_label:?}",
                file.meta.get_ref().spec
            ),
        );
    }
    let examples = checker.examples(file.example);
    let seed = dry_run_seed(text);
    let generated = checker.generated_blocks(file.generated, seed);
    checker.coverage(&examples, &generated);
    checker.formulas(&examples, &generated);
    if checker.problems.is_empty() {
        Ok(TestSuite {
            examples,
            generated,
        })
    } else {
        if checker.hidden > 0 {
            checker.problems.push(Problem {
                line: None,
                message: format!("…and {} more problems", checker.hidden),
            });
        }
        Err(checker.problems)
    }
}

/// The seed of the dry run, fixed by the file's content so `check` always gives the same verdict.
fn dry_run_seed(text: &str) -> u64 {
    let digest = Sha256Hex::of(text.as_bytes());
    u64::from_str_radix(&digest.as_str()[..16], 16).unwrap_or(0)
}

struct Checker<'a> {
    text: &'a str,
    spec: &'a Spec,
    problems: Vec<Problem>,
    hidden: usize,
}

impl<'a> Checker<'a> {
    fn problem(&mut self, line: Option<usize>, message: impl Into<String>) {
        if self.problems.len() < MAX_PROBLEMS {
            self.problems.push(Problem {
                line,
                message: message.into(),
            });
        } else {
            self.hidden += 1;
        }
    }

    fn line(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .bytes()
            .filter(|&byte| byte == b'\n')
            .count()
            + 1
    }

    /// The behavior `id` names, reporting unknown IDs.
    fn behavior(&mut self, id: &str, line: usize, table: &str) -> Option<&'a Behavior> {
        let spec = self.spec;
        let found = BehaviorId::parse(id).and_then(|id| spec.behavior(id));
        if found.is_none() {
            let known: Vec<String> = spec
                .behaviors
                .iter()
                .map(|behavior| behavior.id.to_string())
                .collect();
            self.problem(
                Some(line),
                format!(
                    "{table} has id = {id:?}, which isn't a behavior in spec.md (they are {})",
                    known.join(", ")
                ),
            );
        }
        found
    }

    fn examples(&mut self, tables: Vec<Spanned<ExampleTable>>) -> Vec<Example> {
        let mut examples: Vec<Example> = Vec::new();
        for table in tables {
            let line = self.line(table.span().start);
            let table = table.into_inner();
            let Some(behavior) = self.behavior(&table.id, line, "[[example]]") else {
                continue;
            };
            let id = behavior.id;
            let expected_exit = behavior.exit_code;
            let values: Vec<String> = behavior.values().map(str::to_owned).collect();
            if let Some(first) = examples.iter().find(|example| example.id == id) {
                let first = first.line;
                self.problem(
                    Some(line),
                    format!("{id} has two examples (also on line {first}); keep one per behavior"),
                );
                continue;
            }
            let exit_code = match u8::try_from(table.exit_code) {
                Ok(ASSERTION_EXIT_CODE) => {
                    self.problem(
                        Some(line),
                        format!(
                            "example {id} expects exit code {ASSERTION_EXIT_CODE}, which is \
                             reserved for failed assertions"
                        ),
                    );
                    continue;
                }
                Ok(code) => code,
                Err(_) => {
                    self.problem(
                        Some(line),
                        format!(
                            "example {id}: exit codes go from 0 to 255, not {}",
                            table.exit_code
                        ),
                    );
                    continue;
                }
            };
            if exit_code != expected_exit {
                self.problem(
                    Some(line),
                    format!(
                        "example {id} expects exit code {exit_code}, but the spec says {id} \
                         exits with code {expected_exit}"
                    ),
                );
            }
            let example = Example {
                id,
                line,
                args: table.args,
                stdin: table.stdin,
                stdout: table.stdout,
                stderr: table.stderr,
                exit_code,
            };
            for value in values {
                let present = example.args.contains(&value)
                    || example.stdin.contains(&value)
                    || example.stdout.contains(&value)
                    || example.stderr.contains(&value)
                    || example.exit_code.to_string() == value;
                if !present {
                    self.problem(
                        Some(line),
                        format!(
                            "example {id} doesn't contain the spec's value `{value}`; copy every \
                             backtick value of {id} into its args, stdin, stdout, stderr or \
                             exit_code"
                        ),
                    );
                }
            }
            examples.push(example);
        }
        let missing: Vec<String> = self
            .spec
            .behaviors
            .iter()
            .filter(|behavior| !examples.iter().any(|example| example.id == behavior.id))
            .map(|behavior| behavior.id.to_string())
            .collect();
        for id in missing {
            self.problem(
                None,
                format!("{id} has no [[example]]; add one with the values from its spec steps"),
            );
        }
        examples
    }

    fn generated_blocks(
        &mut self,
        tables: Vec<Spanned<GeneratedTable>>,
        seed: u64,
    ) -> Vec<Generated> {
        let mut blocks = Vec::new();
        for table in tables {
            let line = self.line(table.span().start);
            if let Some(block) = self.generated(line, table.into_inner()) {
                blocks.push(block);
            }
        }
        mark_every_value(&mut blocks);
        for block in &blocks {
            self.dry_run(block, seed);
        }
        blocks
    }

    fn generated(&mut self, line: usize, table: GeneratedTable) -> Option<Generated> {
        let behavior = self.behavior(&table.id, line, "[[generated]]")?;
        let id = behavior.id;
        let behavior_exit = behavior.exit_code;
        let name = format!("generated {id}");
        let before = self.problems.len() + self.hidden;
        let runs = match table.runs.map(u32::try_from) {
            None => DEFAULT_RUNS,
            Some(Ok(runs)) if (1..=MAX_RUNS).contains(&runs) => runs,
            Some(_) => {
                self.problem(
                    Some(line),
                    format!("{name}: runs must be from 1 to {MAX_RUNS}"),
                );
                DEFAULT_RUNS
            }
        };
        if table.vars.is_empty() {
            self.problem(
                Some(line),
                format!("{name} has no vars; declare the inputs it draws, such as vars = {{ a = \"int(-10, 10)\" }}"),
            );
        }
        let mut vars = Vec::new();
        for (var, source) in &table.vars {
            if !expr::is_variable_name(var) {
                self.problem(
                    Some(line),
                    format!(
                        "{name}: `{var}` can't be a variable name; use lowercase letters, digits \
                         and _, not a function name"
                    ),
                );
                continue;
            }
            match Generator::parse(source) {
                Ok(generator) => {
                    self.check_limits(line, &name, var, &generator);
                    vars.push((var.clone(), generator));
                }
                Err(error) => self.problem(Some(line), format!("{name}: vars.{var}: {error}")),
            }
        }
        let mut template = |field: &str, source: &str| match Template::parse(source) {
            Ok(template) => Some(template),
            Err(error) => {
                self.problem(Some(line), format!("{name}: {field}: {error}"));
                None
            }
        };
        let args: Vec<Option<Template>> =
            table.args.iter().map(|arg| template("args", arg)).collect();
        let stdin = template("stdin", &table.stdin);
        let stdout = template("stdout", &table.stdout);
        let stderr = template("stderr", &table.stderr);
        let exit_code = match &table.exit_code {
            ExitCodeField::Number(code) => match u8::try_from(*code) {
                Ok(code) if code == behavior_exit => Some(ExitCode::Fixed(code)),
                Ok(code) => {
                    self.problem(
                        Some(line),
                        format!(
                            "{name} expects exit code {code}, but the spec says {id} exits with \
                             code {behavior_exit}"
                        ),
                    );
                    None
                }
                Err(_) => {
                    self.problem(
                        Some(line),
                        format!("{name}: exit codes go from 0 to 255, not {code}"),
                    );
                    None
                }
            },
            ExitCodeField::Formula(source) => match Template::parse(source) {
                Ok(template) => Some(ExitCode::Formula(template)),
                Err(error) => {
                    self.problem(Some(line), format!("{name}: exit_code: {error}"));
                    None
                }
            },
        };
        let (Some(args), Some(stdin), Some(stdout), Some(stderr), Some(exit_code)) = (
            args.into_iter().collect::<Option<Vec<_>>>(),
            stdin,
            stdout,
            stderr,
            exit_code,
        ) else {
            return None;
        };
        let block = Generated {
            id,
            line,
            runs,
            vars,
            args,
            stdin,
            stdout,
            stderr,
            exit_code,
            every_value: false,
        };
        self.check_variables(&block, &name);
        (self.problems.len() + self.hidden == before).then_some(block)
    }

    /// Generators may only use numbers that Purpose states, so their ranges come from the spec.
    /// Numbers from behaviors don't count: an example's own inputs would make `int(2, 3)` pass.
    fn check_limits(&mut self, line: usize, name: &str, var: &str, generator: &Generator) {
        // Each value Purpose states, with the most places it's written with.
        let mut stated: BTreeMap<Limit, u8> = BTreeMap::new();
        for (limit, places) in self
            .spec
            .purpose_values
            .iter()
            .filter_map(|value| Limit::parse(value))
        {
            let most = stated.entry(limit).or_insert(places);
            *most = (*most).max(places);
        }
        let unreadable: Vec<&str> = self
            .spec
            .purpose_values
            .iter()
            .map(String::as_str)
            .filter(|value| {
                Limit::parse(value).is_none() && value.bytes().any(|byte| byte.is_ascii_digit())
            })
            .collect();
        let hint = if unreadable.is_empty() {
            String::new()
        } else {
            format!(
                " (Purpose writes {}: write numbers plainly, like `12.50`, with no `$`, comma or \
                 exponent)",
                unreadable
                    .iter()
                    .map(|value| format!("`{value}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        // "At most N bytes" implies the empty text, so a text length may start at 0 unstated.
        let empty_text = matches!(generator, Generator::Str { min: 0, .. });
        for limit in generator.limits() {
            if empty_text && limit == Limit::whole(0) {
                continue;
            }
            if !stated.contains_key(&limit) {
                self.problem(
                    Some(line),
                    format!(
                        "{name}: vars.{var} = {generator} uses {limit}, which spec.md's Purpose \
                         doesn't state. Generators take their ranges from Purpose: use the \
                         numbers it gives in backticks, or ask the user to add the range to \
                         Purpose and approve spec.md again{hint}"
                    ),
                );
            }
        }
        if let Some(places) = generator.places()
            && let Some(written) = generator
                .limits()
                .iter()
                .filter_map(|limit| stated.get(limit))
                .max()
            && *written != places
        {
            self.problem(
                Some(line),
                format!(
                    "{name}: vars.{var} = {generator} draws numbers with {places} decimal \
                     places, but Purpose writes them with {written}; write the bounds the way \
                     Purpose does"
                ),
            );
        }
    }

    fn check_variables(&mut self, block: &Generated, name: &str) {
        let declared: BTreeSet<String> = block.vars.iter().map(|(var, _)| var.clone()).collect();
        let mut inputs = BTreeSet::new();
        for (field, template) in block
            .args
            .iter()
            .map(|arg| ("args", arg))
            .chain([("stdin", &block.stdin)])
        {
            if template.pattern().is_none() {
                self.problem(
                    Some(block.line),
                    format!(
                        "{name}: {field} {:?} computes a value; args and stdin may only hold \
                         bare variables like {{a}}, so move the computation to stdout, stderr or \
                         exit_code",
                        template.source()
                    ),
                );
            }
            inputs.extend(template.variables());
        }
        let mut used = inputs.clone();
        used.extend(block.stdout.variables());
        used.extend(block.stderr.variables());
        if let ExitCode::Formula(template) = &block.exit_code {
            used.extend(template.variables());
        }
        for var in used.difference(&declared) {
            self.problem(
                Some(block.line),
                format!("{name} uses `{var}`, which isn't declared in vars"),
            );
        }
        for var in declared.difference(&inputs) {
            self.problem(
                Some(block.line),
                format!(
                    "{name}: `{var}` never reaches the program; use it in args or stdin, or \
                     remove it"
                ),
            );
        }
    }

    /// Evaluates every run's expected results now, so a formula that overflows, divides by zero
    /// or prints the same thing for every input is caught before any code exists.
    fn dry_run(&mut self, block: &Generated, seed: u64) {
        let name = format!("generated {}", block.id);
        let varies = block.stdout.has_placeholders() || block.stderr.has_placeholders();
        // Two different outputs are enough to know they vary; keep no more, since text is large.
        let mut outputs: Vec<(String, String)> = Vec::new();
        let mut runs = 0;
        for result in block.cases(seed) {
            let case = match result {
                Ok(case) => case,
                Err((inputs, field, error)) => {
                    self.problem(
                        Some(block.line),
                        format!(
                            "{name}: with {}, {field} {error}; narrow the generators or fix the \
                             formula",
                            describe_inputs(&inputs)
                        ),
                    );
                    return;
                }
            };
            if let Some(problem) = oversized_args(&case) {
                self.problem(Some(block.line), format!("{name}: {problem}"));
                return;
            }
            runs += 1;
            if outputs.len() < 2
                && !outputs
                    .iter()
                    .any(|(stdout, stderr)| *stdout == case.stdout && *stderr == case.stderr)
            {
                outputs.push((case.stdout, case.stderr));
            }
        }
        if varies && outputs.len() < 2 {
            self.problem(
                Some(block.line),
                format!(
                    "{name}: all {runs} runs expect the same output, so a program that always \
                     prints it would pass. Make the output depend on the inputs, widen the \
                     generators, raise runs, or drop the placeholders"
                ),
            );
        }
    }

    /// Every behavior whose example has inputs needs a generated block, and all blocks of a
    /// behavior share their templates.
    fn coverage(&mut self, examples: &[Example], generated: &[Generated]) {
        for example in examples {
            let blocks: Vec<&Generated> = generated
                .iter()
                .filter(|block| block.id == example.id)
                .collect();
            let has_inputs = !example.args.is_empty() || !example.stdin.is_empty();
            if has_inputs && blocks.is_empty() {
                self.problem(
                    Some(example.line),
                    format!(
                        "{} has inputs, so it needs a [[generated]] block that tries fresh \
                         inputs on every run",
                        example.id
                    ),
                );
            }
            if let [first, rest @ ..] = blocks.as_slice() {
                for block in rest {
                    if templates(block) != templates(first) {
                        self.problem(
                            Some(block.line),
                            format!(
                                "the [[generated]] blocks of {} must share args, stdin, stdout, \
                                 stderr and exit_code (as on line {}); only vars and runs may \
                                 differ",
                                block.id, first.line
                            ),
                        );
                    }
                }
            }
        }
    }

    /// Each behavior's formulas must reproduce its example, whose values come from the
    /// approved spec.
    fn formulas(&mut self, examples: &[Example], generated: &[Generated]) {
        for example in examples {
            let blocks: Vec<&Generated> = generated
                .iter()
                .filter(|block| block.id == example.id)
                .collect();
            let Some(first) = blocks.first() else {
                continue;
            };
            let id = example.id;
            let inputs = match bind(first, example) {
                Ok(inputs) => inputs,
                Err(reason) => {
                    self.problem(
                        Some(first.line),
                        format!("generated {id} can't be matched to example {id}: {reason}"),
                    );
                    continue;
                }
            };
            let Some(block) = blocks.iter().find(|block| {
                block.vars.iter().all(|(name, generator)| {
                    inputs
                        .get(name)
                        .is_some_and(|value| generator.contains(value))
                })
            }) else {
                let outside: Vec<String> = first
                    .vars
                    .iter()
                    .filter_map(|(name, generator)| {
                        let value = inputs.get(name)?;
                        (!generator.contains(value))
                            .then(|| format!("{name} = {value} is outside {generator}"))
                    })
                    .collect();
                self.problem(
                    Some(first.line),
                    format!(
                        "example {id}'s inputs aren't in the generated ranges ({}); the ranges \
                         must include the spec's example",
                        outside.join("; ")
                    ),
                );
                continue;
            };
            let case = match block.case(inputs) {
                Ok(case) => case,
                Err((inputs, field, error)) => {
                    self.problem(
                        Some(block.line),
                        format!(
                            "generated {id}: with example {id}'s inputs ({}), {field} {error}",
                            describe_inputs(&inputs)
                        ),
                    );
                    continue;
                }
            };
            let mismatch = if case.stdout != example.stdout {
                Some((
                    "stdout",
                    format!("{:?}", case.stdout),
                    format!("{:?}", example.stdout),
                ))
            } else if case.stderr != example.stderr {
                Some((
                    "stderr",
                    format!("{:?}", case.stderr),
                    format!("{:?}", example.stderr),
                ))
            } else if case.exit_code != example.exit_code {
                Some((
                    "exit code",
                    case.exit_code.to_string(),
                    example.exit_code.to_string(),
                ))
            } else {
                None
            };
            if let Some((field, formula, spec)) = mismatch {
                self.problem(
                    Some(block.line),
                    format!(
                        "generated {id}'s formula disagrees with the spec: with example {id}'s \
                         inputs ({}) it expects {field} {formula}, but example {id} expects \
                         {spec}. Fix the formula",
                        describe_inputs(&case.inputs)
                    ),
                );
            }
        }
    }
}

type TemplateSources<'a> = (
    Vec<&'a str>,
    &'a str,
    &'a str,
    &'a str,
    Option<&'a str>,
    Option<u8>,
);

fn templates(block: &Generated) -> TemplateSources<'_> {
    let (formula, fixed) = match &block.exit_code {
        ExitCode::Fixed(code) => (None, Some(*code)),
        ExitCode::Formula(template) => (Some(template.source()), None),
    };
    (
        block.args.iter().map(Template::source).collect(),
        block.stdin.source(),
        block.stdout.source(),
        block.stderr.source(),
        formula,
        fixed,
    )
}

/// Reads the variables' values back out of the example's args and stdin.
fn bind(block: &Generated, example: &Example) -> Result<Env, String> {
    if block.args.len() != example.args.len() {
        return Err(format!(
            "it has {} args and the example has {}",
            block.args.len(),
            example.args.len()
        ));
    }
    let generators: BTreeMap<&str, &Generator> = block
        .vars
        .iter()
        .map(|(name, generator)| (name.as_str(), generator))
        .collect();
    let mut pairs = Vec::new();
    for (template, text) in block
        .args
        .iter()
        .zip(&example.args)
        .chain([(&block.stdin, &example.stdin)])
    {
        let pattern = template
            .pattern()
            .ok_or_else(|| "its args and stdin must be bare variables".to_owned())?;
        pairs.push((pattern, text.as_str()));
    }
    let mut solutions = Vec::new();
    solve(&pairs, 0, &generators, &mut Env::new(), &mut solutions);
    match solutions.len() {
        0 => Err(format!(
            "no values of its vars turn args {:?} and stdin {:?} into the example's args {:?} \
             and stdin {:?}",
            block.args.iter().map(Template::source).collect::<Vec<_>>(),
            block.stdin.source(),
            example.args,
            example.stdin
        )),
        1 => Ok(solutions.remove(0)),
        _ => Err(
            "the example's inputs fit its placeholders in more than one way; separate \
                  neighboring placeholders with fixed text"
                .into(),
        ),
    }
}

/// Backtracking search for variable values that render each pattern into its text. Stops after
/// two solutions, which is enough to know the match is ambiguous.
fn solve(
    pairs: &[(Vec<PatternPart>, &str)],
    index: usize,
    generators: &BTreeMap<&str, &Generator>,
    env: &mut Env,
    solutions: &mut Vec<Env>,
) {
    fn within(
        pairs: &[(Vec<PatternPart>, &str)],
        index: usize,
        parts: &[PatternPart],
        text: &str,
        generators: &BTreeMap<&str, &Generator>,
        env: &mut Env,
        solutions: &mut Vec<Env>,
    ) {
        if solutions.len() > 1 {
            return;
        }
        match parts.split_first() {
            None => {
                if text.is_empty() {
                    solve(pairs, index + 1, generators, env, solutions);
                }
            }
            Some((PatternPart::Text(literal), tail)) => {
                if let Some(after) = text.strip_prefix(literal.as_str()) {
                    within(pairs, index, tail, after, generators, env, solutions);
                }
            }
            Some((PatternPart::Var(name), tail)) => {
                if let Some(bound) = env.get(name) {
                    let rendered = bound.render().unwrap_or_default();
                    if let Some(after) = text.strip_prefix(rendered.as_str()) {
                        within(pairs, index, tail, after, generators, env, solutions);
                    }
                    return;
                }
                let Some(generator) = generators.get(name.as_str()) else {
                    return;
                };
                let splits: Vec<usize> = text
                    .char_indices()
                    .map(|(position, _)| position)
                    .chain([text.len()])
                    .collect();
                for split in splits {
                    if let Some(value) = generator.read(&text[..split]) {
                        env.insert(name.clone(), value);
                        within(
                            pairs,
                            index,
                            tail,
                            &text[split..],
                            generators,
                            env,
                            solutions,
                        );
                        env.remove(name);
                    }
                }
            }
        }
    }
    match pairs.get(index) {
        None => solutions.push(env.clone()),
        Some((parts, text)) => within(pairs, index, parts, text, generators, env, solutions),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec;

    const SPEC: &str = "---
type: lengua/program-spec
title: add
---
# Purpose
Adds two whole numbers between `-1000000000` and `1000000000`.

# Behaviors
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` followed by a newline
- And it exits with code `0`

## B2: Missing arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` followed by a newline to standard error
- And it exits with code `1`

# Constraints
- Allowed system calls: read, write, exit
";

    const LABEL: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    fn tests(body: &str) -> String {
        format!("[meta]\nspec = \"{LABEL}\"\n{body}")
    }

    const EXAMPLES: &str = r#"
[[example]]
id = "B1"
args = ["2", "3"]
stdout = "5\n"
exit_code = 0

[[example]]
id = "B2"
stderr = "usage: add <a> <b>\n"
exit_code = 1
"#;

    const GENERATED: &str = r#"
[[generated]]
id = "B1"
runs = 200
vars = { a = "int(-1000000000, 1000000000)", b = "int(-1000000000, 1000000000)" }
args = ["{a}", "{b}"]
stdout = "{a + b}\n"
exit_code = 0
"#;

    fn spec() -> Spec {
        spec::parse(SPEC).expect("valid spec")
    }

    fn valid() -> String {
        tests(&format!("{EXAMPLES}{GENERATED}"))
    }

    fn problems(text: &str) -> Vec<String> {
        match check(text, &spec(), LABEL) {
            Ok(suite) => panic!("expected problems, got {suite:#?}"),
            Err(problems) => problems.iter().map(ToString::to_string).collect(),
        }
    }

    fn assert_problem(text: &str, needle: &str) {
        let problems = problems(text);
        assert!(
            problems.iter().any(|problem| problem.contains(needle)),
            "no problem containing {needle:?} in {problems:#?}"
        );
    }

    #[test]
    fn accepts_valid_tests() {
        let suite = check(&valid(), &spec(), LABEL).expect("valid tests");
        assert_eq!(suite.examples.len(), 2);
        assert_eq!(suite.examples[0].args, ["2", "3"]);
        assert_eq!(suite.examples[1].line, 10);
        let block = &suite.generated[0];
        assert_eq!(block.runs, 200);
        assert_eq!(block.line, 15);
        let cases: Vec<Case> = block
            .cases(42)
            .collect::<Result<_, _>>()
            .expect("computable");
        assert_eq!(cases.len(), 200);
        let first = &cases[0];
        assert_eq!(first.args, ["-1000000000", "-1000000000"]);
        assert_eq!(first.stdout, "-2000000000\n");
        assert_eq!(first.exit_code, 0);
    }

    #[test]
    fn requires_the_current_spec_label() {
        let text = valid().replace(LABEL, "sha256:1111");
        assert_problem(&text, "[meta] says spec = \"sha256:1111\"");
    }

    #[test]
    fn reports_toml_errors() {
        assert_problem("[meta]\nspec = ", "tests.toml isn't valid");
        assert_problem(
            &valid().replace("exit_code = 1", "exit_code = 1\ncolour = \"red\""),
            "unknown field `colour`",
        );
        assert_problem(
            &valid().replace("exit_code = 1\n", ""),
            "missing field `exit_code`",
        );
    }

    #[test]
    fn requires_one_example_per_behavior() {
        let without_b2 = tests(&format!(
            "{}{GENERATED}",
            EXAMPLES
                .split("\n[[example]]\nid = \"B2\"")
                .next()
                .expect("B1 part")
        ));
        assert_problem(&without_b2, "B2 has no [[example]]");
        let twice = valid().replace("id = \"B2\"", "id = \"B1\"");
        assert_problem(&twice, "B1 has two examples (also on line 4)");
        let unknown = valid().replace("id = \"B2\"", "id = \"B7\"");
        assert_problem(
            &unknown,
            "id = \"B7\", which isn't a behavior in spec.md (they are B1, B2)",
        );
    }

    #[test]
    fn requires_generated_tests_for_behaviors_with_inputs() {
        assert_problem(
            &tests(EXAMPLES),
            "B1 has inputs, so it needs a [[generated]] block",
        );
    }

    #[test]
    fn requires_every_spec_value_in_the_example() {
        let text = valid().replace("stdout = \"5\\n\"", "stdout = \"6\\n\"");
        assert_problem(&text, "example B1 doesn't contain the spec's value `5`");
    }

    #[test]
    fn checks_exit_codes() {
        assert_problem(
            &valid().replace("exit_code = 1", "exit_code = 134"),
            "expects exit code 134, which is reserved",
        );
        assert_problem(
            &valid().replace("exit_code = 1", "exit_code = 2"),
            "example B2 expects exit code 2, but the spec says B2 exits with code 1",
        );
        assert_problem(
            &valid().replace(
                "stdout = \"{a + b}\\n\"\nexit_code = 0",
                "stdout = \"{a + b}\\n\"\nexit_code = 3",
            ),
            "generated B1 expects exit code 3",
        );
        assert_problem(
            &valid().replace(
                "stdout = \"{a + b}\\n\"\nexit_code = 0",
                "stdout = \"{a + b}\\n\"\nexit_code = \"{if(a > 0, 0, 134)}\"",
            ),
            "reserved for failed assertions",
        );
    }

    #[test]
    fn reports_bad_expressions() {
        assert_problem(
            &valid().replace("{a + b}", "{a +* b}"),
            "generated B1: stdout: in {a +* b}",
        );
        assert_problem(
            &valid().replace("{a + b}", "{a + c}"),
            "uses `c`, which isn't declared in vars",
        );
        assert_problem(
            &valid().replace("\"int(-1000000000, 1000000000)\" }", "\"float(0, 1)\" }"),
            "vars.b: unknown generator `float`",
        );
    }

    #[test]
    fn rejects_computed_inputs_and_unused_variables() {
        assert_problem(
            &valid().replace("args = [\"{a}\", \"{b}\"]", "args = [\"{a}\", \"{b + 1}\"]"),
            "args \"{b + 1}\" computes a value",
        );
        assert_problem(
            &valid().replace(
                "b = \"int(-1000000000, 1000000000)\" }",
                "b = \"int(-1000000000, 1000000000)\", c = \"int(-1000000000, 1000000000)\" }",
            ),
            "`c` never reaches the program",
        );
    }

    #[test]
    fn catches_formulas_that_fail_on_some_inputs() {
        assert_problem(
            &valid().replace("{a + b}", "{a / b}"),
            "stdout divides by zero",
        );
        assert_problem(
            &valid().replace("{a + b}", "{a * b * a * b * a * b * a * b}"),
            "overflows",
        );
    }

    #[test]
    fn rejects_constant_output_despite_placeholders() {
        assert_problem(
            &valid().replace("{a + b}", "{min(5, 5)}"),
            "all 200 runs expect the same output",
        );
    }

    #[test]
    fn formulas_must_reproduce_the_example() {
        assert_problem(
            &valid().replace("{a + b}", "{a - b}"),
            "with example B1's inputs (a = 2, b = 3) it expects stdout \"-1\\n\", but example \
             B1 expects \"5\\n\"",
        );
        assert_problem(
            &valid().replace(
                "int(-1000000000, 1000000000)\" }",
                "int(-1000000000, -1000000000)\" }",
            ),
            "b = 3 is outside int(-1000000000, -1000000000)",
        );
        assert_problem(
            &valid().replace(
                "args = [\"{a}\", \"{b}\"]",
                "args = [\"{a}\", \"{b}\", \"x\"]",
            ),
            "it has 3 args and the example has 2",
        );
        assert_problem(
            &valid()
                .replace("args = [\"{a}\", \"{b}\"]", "args = [\"{a}{b}\"]")
                .replace("args = [\"2\", \"3\"]", "args = [\"123\"]"),
            "more than one way",
        );
    }

    /// A spec for negating whole numbers from `lo` to `hi`.
    fn negate_spec(lo: i64, hi: i64) -> Spec {
        spec::parse(&format!(
            "---
type: lengua/program-spec
title: negate
---
# Purpose
Negates a whole number between `{lo}` and `{hi}`.

# Behaviors
## B1: Negates a number
- Given the argument `7`
- When the program runs
- Then it prints `-7` followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: write, exit
"
        ))
        .expect("valid spec")
    }

    fn negate_tests(blocks: &[&str]) -> String {
        let mut text =
            tests("[[example]]\nid = \"B1\"\nargs = [\"7\"]\nstdout = \"-7\\n\"\nexit_code = 0\n");
        for vars in blocks {
            text.push_str(&format!(
                "\n[[generated]]\nid = \"B1\"\nvars = {{ {vars} }}\nargs = [\"{{n}}\"]\n\
                 stdout = \"{{-n}}\\n\"\nexit_code = 0\n"
            ));
        }
        text
    }

    #[test]
    fn text_may_start_empty_without_purpose_saying_so() {
        let spec = spec::parse(
            "---
type: lengua/program-spec
title: lines
---
# Purpose
Counts the lines of at most `1048576` bytes of text.

# Behaviors
## B1: Counts lines
- Given the input `a` followed by a newline
- When the program runs
- Then it prints `1` followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: read, write, exit
",
        )
        .expect("valid spec");
        let tests = tests(
            "[[example]]\nid = \"B1\"\nstdin = \"a\\n\"\nstdout = \"1\\n\"\nexit_code = 0\n\n\
             [[generated]]\nid = \"B1\"\nvars = { text = 'string(0, 1048576, \"a\\n\")' }\n\
             stdin = \"{text}\"\nstdout = \"{count(text, \\\"\\\\n\\\")}\\n\"\nexit_code = 0\n",
        );
        let result = check(&tests, &spec, LABEL);
        assert!(result.is_ok(), "{result:#?}");
    }

    #[test]
    fn decimal_ranges_come_from_purpose_as_written() {
        let spec = |purpose: &str| {
            spec::parse(&format!(
                "---
type: lengua/program-spec
title: half
---
# Purpose
Halves a price between {purpose}.

# Behaviors
## B1: Halves a price
- Given the argument `3.00`
- When the program runs
- Then it prints `1.50` followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: write, exit
"
            ))
            .expect("valid spec")
        };
        let tests = |generator: &str| {
            tests(&format!(
                "[[example]]\nid = \"B1\"\nargs = [\"3.00\"]\nstdout = \"1.50\\n\"\nexit_code = 0\n\n\
                 [[generated]]\nid = \"B1\"\nvars = {{ x = \"{generator}\" }}\nargs = [\"{{x}}\"]\n\
                 stdout = \"{{fixed(x / 2.0, 2)}}\\n\"\nexit_code = 0\n"
            ))
        };
        let plain = spec("`0.00` and `100.00`");
        assert!(check(&tests("decimal(0.00, 100.00)"), &plain, LABEL).is_ok());
        let problems = |text: &str, spec: &Spec| match check(text, spec, LABEL) {
            Ok(_) => panic!("accepted {text}"),
            Err(problems) => problems.iter().map(ToString::to_string).collect::<Vec<_>>(),
        };
        let found = problems(&tests("decimal(0.000, 100.000)"), &plain);
        assert!(
            found.iter().any(|problem| problem
                .contains("draws numbers with 3 decimal places, but Purpose writes them with 2")),
            "{found:#?}"
        );
        let found = problems(
            &tests("decimal(0.00, 100.00)"),
            &spec("`0.00` and `$100.00`"),
        );
        assert!(
            found.iter().any(
                |problem| problem.contains("uses 100, which spec.md's Purpose")
                    && problem.contains("Purpose writes `$100.00`")
            ),
            "{found:#?}"
        );
    }

    #[test]
    fn small_ranges_run_every_value() {
        let suite = check(
            &negate_tests(&[r#"n = "int(-100, 100)""#]),
            &negate_spec(-100, 100),
            LABEL,
        )
        .expect("valid tests");
        let block = &suite.generated[0];
        assert!(block.every_value);
        assert_eq!(block.run_count(), 201);
        assert_eq!(
            block.every_value_text().as_deref(),
            Some("every value of n in its range (all 201)")
        );
        let values: BTreeSet<String> = block
            .cases(1)
            .map(|case| case.expect("computable").args[0].clone())
            .collect();
        assert_eq!(values.len(), 201);

        // A large range is sampled, as are hand-picked values.
        let suite = check(&valid(), &spec(), LABEL).expect("valid tests");
        assert!(!suite.generated[0].every_value);
        assert_eq!(suite.generated[0].run_count(), 200);
        let suite = check(
            &negate_tests(&[r#"n = "choice(-100, 7, 100)""#]),
            &negate_spec(-100, 100),
            LABEL,
        )
        .expect("valid tests");
        assert!(
            !suite.generated[0].every_value,
            "choice values aren't a range"
        );
    }

    #[test]
    fn every_value_stays_within_the_suite_budget() {
        let block = r#"n = "int(-2500, 2499)""#;
        let suite = check(&negate_tests(&[block; 5]), &negate_spec(-2500, 2499), LABEL)
            .expect("valid tests");
        let full = suite
            .generated
            .iter()
            .filter(|block| block.every_value)
            .count();
        assert_eq!(
            full, 3,
            "5000 runs each, while the suite stays within 20000"
        );
        let runs: usize = suite.generated.iter().map(Generated::run_count).sum();
        assert_eq!(runs, 3 * 5000 + 2 * 100);
    }

    #[test]
    fn arguments_stay_within_linux_limits() {
        let spec = spec::parse(
            "---
type: lengua/program-spec
title: len
---
# Purpose
Prints the length of a word of `1` to `200000` letters.

# Behaviors
## B1: Prints the length
- Given the argument `abc`
- When the program runs
- Then it prints `3` followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: write, exit
",
        )
        .expect("valid spec");
        let tests = |generator: &str| {
            tests(&format!(
                "[[example]]\nid = \"B1\"\nargs = [\"abc\"]\nstdout = \"3\\n\"\nexit_code = 0\n\n\
                 [[generated]]\nid = \"B1\"\nvars = {{ w = '{generator}' }}\nargs = [\"{{w}}\"]\n\
                 stdout = \"{{len(w)}}\\n\"\nexit_code = 0\n"
            ))
        };
        let problems = match check(&tests(r#"string(1, 200000, "abc")"#), &spec, LABEL) {
            Ok(_) => panic!("a 200000-byte argument was accepted"),
            Err(problems) => problems.iter().map(ToString::to_string).collect::<Vec<_>>(),
        };
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("Linux limits each argument to 131072 bytes")),
            "{problems:#?}"
        );
        assert!(
            problems.iter().all(|problem| problem.len() < 2000),
            "messages shorten long values: {problems:#?}"
        );
    }

    #[test]
    fn generator_limits_come_from_purpose() {
        assert_problem(
            &valid().replace(
                "b = \"int(-1000000000, 1000000000)\"",
                "b = \"int(-1000, 1000000000)\"",
            ),
            "uses -1000, which spec.md's Purpose doesn't state",
        );
        // 2 and 3 are B1's example inputs, not a stated range: int(2, 3) would test almost nothing.
        assert_problem(
            &valid().replace(
                "vars = { a = \"int(-1000000000, 1000000000)\", b = \"int(-1000000000, \
                 1000000000)\" }",
                "vars = { a = \"int(2, 3)\", b = \"int(2, 3)\" }",
            ),
            "uses 2, which spec.md's Purpose doesn't state",
        );
        // String lengths too: the spec has 1 and 3 only as an exit code and an argument.
        assert_problem(
            &valid().replace(
                "b = \"int(-1000000000, 1000000000)\"",
                "b = 'string(1, 3, \"123\")'",
            ),
            "uses 1, which spec.md's Purpose doesn't state",
        );
    }

    #[test]
    fn blocks_of_one_behavior_share_their_formulas() {
        let second = GENERATED.replace("{a + b}", "{b + a}");
        let text = tests(&format!("{EXAMPLES}{GENERATED}{second}"));
        assert_problem(
            &text,
            "must share args, stdin, stdout, stderr and exit_code",
        );
        let narrower = GENERATED.replace("runs = 200", "runs = 50");
        let ok = tests(&format!("{EXAMPLES}{GENERATED}{narrower}"));
        assert!(check(&ok, &spec(), LABEL).is_ok());
    }

    #[test]
    fn reads_inputs_from_stdin_and_text_around_placeholders() {
        let spec_text = SPEC.replace("Given the arguments `2` and `3`", "Given the input `2 3`");
        let spec = spec::parse(&spec_text).expect("valid spec");
        let text = tests(
            r#"
[[example]]
id = "B1"
stdin = "2 3\n"
stdout = "5\n"
exit_code = 0

[[example]]
id = "B2"
stderr = "usage: add <a> <b>\n"
exit_code = 1

[[generated]]
id = "B1"
vars = { a = "int(-1000000000, 1000000000)", b = "int(-1000000000, 1000000000)" }
stdin = "{a} {b}\n"
stdout = "{a + b}\n"
exit_code = 0
"#,
        );
        let suite = check(&text, &spec, LABEL).expect("valid tests");
        assert_eq!(suite.generated[0].runs, DEFAULT_RUNS);
        let wrong = text.replace("{a + b}", "{a * b}");
        assert!(check(&wrong, &spec, LABEL).is_err());
    }

    #[test]
    fn dry_runs_are_deterministic() {
        let text = valid();
        let first = check(&text, &spec(), LABEL);
        assert_eq!(first, check(&text, &spec(), LABEL));
        assert_eq!(dry_run_seed(&text), dry_run_seed(&text));
    }
}
