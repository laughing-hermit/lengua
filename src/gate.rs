//! The gate that holds lengua's programs to C, C++ and Rust (todo.md's finish line, point 4). A
//! row is a workload and another language's version of it, compiled to WebAssembly and run the
//! same way, or built natively; on each row, lengua's measures are set against the other's: CPU
//! and wall time by a one-sided Mann-Whitney U test at p < [`ALPHA`] over runs that alternate,
//! peak memory and size as measured. A row the test can't decide is undecided, never level.
//!
//! The runs of a row grow until both its tests decide: the gate looks at them at each count of
//! [`LOOKS`], up to [`MAX_RUNS`] or [`MAX_ROW`], and each look decides at [`LOOK_ALPHA`], ALPHA
//! split over the looks, so that a row looked at again and again is still decided wrongly with a
//! chance below ALPHA.
//!
//! `KNOWN_BEHIND`, a file at the repository's root, names the rows where lengua isn't ahead yet,
//! and it may only shrink: a run that finds lengua not ahead on a row the list doesn't name, or
//! ahead on one it names, fails ([`ratchet`]), and says the list that would pass ([`proposal`]);
//! and a change to the list that names a row the list before it didn't fails the run too
//! ([`grown`]), as the bench holds each change in the list's history to the list it replaced.

use std::cmp::Ordering;
use std::fmt::Write as _;
use std::time::Duration;

/// The level the gate decides a row's measure at: a one-sided p below it, over all its looks.
pub const ALPHA: f64 = 0.01;

/// The most runs of each version of a workload.
pub const MAX_RUNS: usize = 2000;

/// The counts of runs at which the gate looks at a row, deciding what it can: the first is the
/// fewest runs it decides on, the last [`MAX_RUNS`].
pub const LOOKS: [usize; 8] = [20, 40, 80, 160, 320, 640, 1280, MAX_RUNS];

/// The level of each look: [`ALPHA`] split over the [`LOOKS`] (Bonferroni's bound), so that the
/// chance that any look of a row decides wrongly is below ALPHA.
pub const LOOK_ALPHA: f64 = ALPHA / LOOKS.len() as f64;

/// The longest a workload's rows run.
pub const MAX_ROW: Duration = Duration::from_secs(600);

/// The runners the gate runs on, by the names scripts/ci/targets.json gives them.
pub const TARGETS: [&str; 5] = [
    "linux-x86_64",
    "linux-aarch64",
    "macos-aarch64",
    "windows-x86_64",
    "windows-aarch64",
];

/// The workloads, by name.
pub const WORKLOADS: [&str; 14] = [
    "empty",
    "add",
    "hypotenuse",
    "upcase",
    "upcase-stdin",
    "shout",
    "wc",
    "histogram",
    "crc32c",
    "sort",
    "tally",
    "server",
    "files",
    "readings",
];

/// The other versions a row sets lengua against: C, C++ and Rust compiled to WebAssembly, and
/// the same built natively.
pub const VERSUS: [&str; 6] = ["c-wasm", "cpp-wasm", "rust-wasm", "c", "cpp", "rust"];

/// A measure the gate compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Measure {
    /// CPU time, in user space and in the kernel.
    Cpu,
    /// Time on the clock.
    Wall,
    /// Peak resident memory.
    Memory,
    /// The size of what ran: the module, or the executable.
    Size,
}

impl Measure {
    /// Every measure, in the order the report gives them.
    pub const ALL: [Self; 4] = [Self::Cpu, Self::Wall, Self::Memory, Self::Size];

    /// Its name in KNOWN_BEHIND.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Wall => "wall",
            Self::Memory => "memory",
            Self::Size => "size",
        }
    }

    /// The measure `name` names.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|measure| measure.name() == name)
    }

    /// Whether the gate decides it with the test over runs, rather than as measured.
    #[must_use]
    pub const fn tested(self) -> bool {
        matches!(self, Self::Cpu | Self::Wall)
    }
}

/// What the gate finds on a row and measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// lengua's is lower: with the test, at p < [`ALPHA`]; as measured, smaller.
    Ahead,
    /// The other's is lower, as surely.
    Behind,
    /// Neither, as far as the runs go.
    Undecided,
}

impl Verdict {
    /// The word the report and KNOWN_BEHIND use.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ahead => "ahead",
            Self::Behind => "behind",
            Self::Undecided => "undecided",
        }
    }
}

/// The complementary error function, to a fractional error below 1.2e-7 (Numerical Recipes'
/// `erfcc`, a Chebyshev fit).
fn erfc(x: f64) -> f64 {
    // The fit's coefficients, from the power 0 of t to the power 9.
    const COEFFICIENTS: [f64; 10] = [
        -1.265_512_23,
        1.000_023_68,
        0.374_091_96,
        0.096_784_18,
        -0.186_288_06,
        0.278_868_07,
        -1.135_203_98,
        1.488_515_87,
        -0.822_152_23,
        0.170_872_77,
    ];
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    // The polynomial in t, by Horner's rule from its highest power.
    let poly = COEFFICIENTS
        .iter()
        .rev()
        .fold(0.0, |sum, coefficient| sum * t + coefficient);
    let r = t * (-z * z + poly).exp();
    if x >= 0.0 { r } else { 2.0 - r }
}

/// The standard normal distribution's probability of a value at most `z`.
fn normal_cdf(z: f64) -> f64 {
    0.5 * erfc(-z / std::f64::consts::SQRT_2)
}

/// The one-sided p-value of a Mann-Whitney U test that the values drawn as `smaller` tend to be
/// smaller than those drawn as `larger`: the normal approximation, its variance corrected for
/// ties, with a continuity correction of half the step U takes between neighbouring runs of tied
/// values (1/2 when no values are tied). 1 when either sample is empty, or every value is the
/// same.
#[must_use]
pub fn p_smaller(smaller: &[f64], larger: &[f64]) -> f64 {
    let (n1, n2) = (smaller.len(), larger.len());
    if n1 == 0 || n2 == 0 {
        return 1.0;
    }
    let mut all: Vec<(f64, bool)> = smaller
        .iter()
        .map(|value| (*value, true))
        .chain(larger.iter().map(|value| (*value, false)))
        .collect();
    all.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n = all.len();
    // The ranks of `smaller`'s values, each run of tied values sharing the mean of its ranks; the
    // sum of t³ - t over the runs of t tied values; and U's smallest step: a value that moves
    // from a run of t values to the next, of t', moves U by the difference of their ranks,
    // (t + t') / 2.
    let (mut rank_sum, mut ties) = (0.0, 0.0);
    let (mut step, mut before) = (f64::INFINITY, None);
    let mut at = 0;
    while at < n {
        let mut end = at + 1;
        while end < n && all[end].0.total_cmp(&all[at].0) == Ordering::Equal {
            end += 1;
        }
        let tied = (end - at) as f64;
        let rank = (at + 1 + end) as f64 / 2.0;
        rank_sum += rank * all[at..end].iter().filter(|value| value.1).count() as f64;
        ties += tied * tied * tied - tied;
        if let Some(previous) = before {
            step = f64::min(step, (previous + tied) / 2.0);
        }
        before = Some(tied);
        at = end;
    }
    let (n1, n2, n) = (n1 as f64, n2 as f64, n as f64);
    let u = rank_sum - n1 * (n1 + 1.0) / 2.0;
    let mean = n1 * n2 / 2.0;
    let variance = n1 * n2 / 12.0 * ((n + 1.0) - ties / (n * (n - 1.0)));
    if variance <= 0.0 {
        return 1.0;
    }
    // A small U is evidence that `smaller`'s values are smaller; the continuity correction moves
    // it half a step towards the mean. Without ties a step is 1; runs of tied values, as CPU
    // times counted in a clock's ticks are, make U's steps longer, and a correction of 1/2 would
    // then make p smaller than the exact test's.
    normal_cdf((u + step / 2.0 - mean) / variance.sqrt())
}

/// The gate's verdict on lengua's runs against another version's, for a measure the test decides
/// at level `alpha`, and the p-value of the side it decided, or the smaller of the two when it
/// decided none.
#[must_use]
pub fn tested_verdict(lengua: &[f64], other: &[f64], alpha: f64) -> (Verdict, f64) {
    let ahead = p_smaller(lengua, other);
    if ahead < alpha {
        return (Verdict::Ahead, ahead);
    }
    let behind = p_smaller(other, lengua);
    if behind < alpha {
        return (Verdict::Behind, behind);
    }
    (Verdict::Undecided, ahead.min(behind))
}

/// Whether the gate looks at a row once it has `runs` runs of each version.
#[must_use]
pub fn looks_at(runs: usize) -> bool {
    LOOKS.contains(&runs)
}

/// The gate's verdict on a measure taken as measured, lengua's against another version's: ahead
/// when lengua's is smaller, behind when it is larger, and undecided when they're the same.
#[must_use]
pub fn measured_verdict(lengua: f64, other: f64) -> Verdict {
    match lengua.total_cmp(&other) {
        Ordering::Less => Verdict::Ahead,
        Ordering::Greater => Verdict::Behind,
        Ordering::Equal => Verdict::Undecided,
    }
}

/// The median of `values`: the middle one, or the mean of the middle two; `None` for none.
#[must_use]
pub fn median(values: &[f64]) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    match sorted.len() {
        0 => None,
        count if count % 2 == 1 => Some(sorted[middle]),
        _ => Some(f64::midpoint(sorted[middle - 1], sorted[middle])),
    }
}

/// One line of KNOWN_BEHIND: a row and measure where lengua isn't ahead yet, each field a name or
/// `*` for every one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// The runner.
    pub target: String,
    /// The workload.
    pub workload: String,
    /// The other version.
    pub versus: String,
    /// The measure.
    pub measure: String,
    /// The line it is on, from 1.
    pub line: usize,
}

impl Listed {
    /// The row on line `line` that `row`, the line without its reason, writes: four fields, or
    /// none.
    fn of(line: usize, row: &str) -> Option<Self> {
        let fields: Vec<&str> = row.split_whitespace().collect();
        let [target, workload, versus, measure] = fields.as_slice() else {
            return None;
        };
        Some(Self {
            target: (*target).to_owned(),
            workload: (*workload).to_owned(),
            versus: (*versus).to_owned(),
            measure: (*measure).to_owned(),
            line,
        })
    }

    /// Whether it names the row and measure given.
    #[must_use]
    pub fn names(&self, target: &str, workload: &str, versus: &str, measure: Measure) -> bool {
        let field = |listed: &str, value: &str| listed == "*" || listed == value;
        field(&self.target, target)
            && field(&self.workload, workload)
            && field(&self.versus, versus)
            && field(&self.measure, measure.name())
    }

    /// Whether it names every row and measure of its target, the list's first state, before a
    /// run measured them.
    #[must_use]
    pub fn every_row(&self) -> bool {
        self.workload == "*" && self.versus == "*" && self.measure == "*"
    }
}

/// KNOWN_BEHIND: the rows where lengua isn't ahead yet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KnownBehind {
    /// Its lines, in order.
    pub rows: Vec<Listed>,
}

impl KnownBehind {
    /// The list `text` writes: a row a line, `target workload versus measure`, each a name or `*`
    /// for every one, and what follows a `#` the reason; blank lines say nothing.
    ///
    /// # Errors
    ///
    /// Each line that isn't such a row, by its number.
    pub fn parse(text: &str) -> Result<Self, Vec<String>> {
        let mut rows = Vec::new();
        let mut problems = Vec::new();
        for (number, line) in (1..).zip(text.lines()) {
            let row = line.split_once('#').map_or(line, |(row, _)| row).trim();
            if row.is_empty() {
                continue;
            }
            let Some(listed) = Listed::of(number, row) else {
                problems.push(format!(
                    "KNOWN_BEHIND line {number}: a row is a target, a workload, a version and a \
                     measure, each a name or *, not {row:?}"
                ));
                continue;
            };
            let known = |value: &str, names: &[&str]| value == "*" || names.contains(&value);
            let measures = Measure::ALL.map(Measure::name);
            for (value, names, what) in [
                (listed.target.as_str(), &TARGETS[..], "target"),
                (listed.workload.as_str(), &WORKLOADS[..], "workload"),
                (listed.versus.as_str(), &VERSUS[..], "version"),
                (listed.measure.as_str(), measures.as_slice(), "measure"),
            ] {
                if !known(value, names) {
                    problems.push(format!(
                        "KNOWN_BEHIND line {number}: {value:?} is no {what} the gate knows: {}",
                        names.join(", ")
                    ));
                }
            }
            rows.push(listed);
        }
        if problems.is_empty() {
            Ok(Self { rows })
        } else {
            Err(problems)
        }
    }

    /// The rows of `text`, a list as an earlier commit had it, read as [`KnownBehind::parse`]
    /// reads them but with no line refused: a name the gate no longer has names no row, and a
    /// line that isn't four fields is no row.
    #[must_use]
    pub fn read(text: &str) -> Self {
        let rows = (1..)
            .zip(text.lines())
            .filter_map(|(number, line)| {
                Listed::of(number, line.split_once('#').map_or(line, |(row, _)| row))
            })
            .collect();
        Self { rows }
    }

    /// The first line that names the row and measure given.
    #[must_use]
    pub fn naming(
        &self,
        target: &str,
        workload: &str,
        versus: &str,
        measure: Measure,
    ) -> Option<&Listed> {
        self.rows
            .iter()
            .find(|row| row.names(target, workload, versus, measure))
    }
}

/// What a run found on a row and measure, as the ratchet takes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The workload.
    pub workload: String,
    /// The other version.
    pub versus: String,
    /// The measure.
    pub measure: Measure,
    /// What the gate found.
    pub verdict: Verdict,
}

/// Where `found`, a run's verdicts on `target`, disagree with `known`: each row where lengua isn't
/// ahead that the list doesn't name, and each row the list names where lengua is ahead now, which
/// the list must then leave out, as it only shrinks. Empty when they agree.
#[must_use]
pub fn ratchet(known: &KnownBehind, target: &str, found: &[Found]) -> Vec<String> {
    let mut problems = Vec::new();
    for row in found {
        let named = known.naming(target, &row.workload, &row.versus, row.measure);
        let what = format!(
            "{target} {} {} {}",
            row.workload,
            row.versus,
            row.measure.name()
        );
        match (row.verdict, named) {
            (Verdict::Ahead, Some(listed)) if listed.every_row() => problems.push(format!(
                "{what}: lengua is ahead, and KNOWN_BEHIND line {} still names every row of \
                 {target}: write out the rows where lengua isn't ahead in its place, as the run's \
                 KNOWN_BEHIND-{target}.txt does",
                listed.line
            )),
            (Verdict::Ahead, Some(listed)) => problems.push(format!(
                "{what}: lengua is ahead now, and KNOWN_BEHIND line {} names the row: take it \
                 out, as the list only shrinks",
                listed.line
            )),
            (Verdict::Behind | Verdict::Undecided, None) => problems.push(format!(
                "{what}: lengua is {}, and KNOWN_BEHIND doesn't name the row: make lengua's \
                 program or its engine faster, where the list may not grow",
                row.verdict.name()
            )),
            _ => {}
        }
    }
    problems
}

/// Where `after`, KNOWN_BEHIND as a change left it, names a row and measure that `before`, the
/// list the change replaced, doesn't name: the list may only shrink, by lines taken out or made
/// narrower, as a line that names every row of a runner is when the runner's own list takes its
/// place. `existed` says of each name, of a runner, a workload, a version or a measure, whether
/// the gate had it before the change: a row the gate didn't have, new with the change, isn't held
/// to the list before it. Empty when the list didn't grow.
#[must_use]
pub fn grown(
    before: &KnownBehind,
    after: &KnownBehind,
    existed: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut grown = Vec::new();
    let measures: Vec<Measure> = Measure::ALL
        .into_iter()
        .filter(|measure| existed(measure.name()))
        .collect();
    for target in TARGETS.into_iter().filter(|name| existed(name)) {
        for workload in WORKLOADS.into_iter().filter(|name| existed(name)) {
            for versus in VERSUS.into_iter().filter(|name| existed(name)) {
                for &measure in &measures {
                    let Some(listed) = after.naming(target, workload, versus, measure) else {
                        continue;
                    };
                    if before.naming(target, workload, versus, measure).is_none() {
                        grown.push(format!(
                            "{target} {workload} {versus} {}: KNOWN_BEHIND line {} names the \
                             row, which the list before it didn't: the list may only shrink, so \
                             make lengua's program or its engine faster instead",
                            measure.name(),
                            listed.line
                        ));
                    }
                }
            }
        }
    }
    grown
}

/// The lines of KNOWN_BEHIND that `found`, a run's verdicts on `target`, would pass with: a line
/// for each row and measure where lengua isn't ahead, with what the gate found.
#[must_use]
pub fn proposal(target: &str, found: &[Found]) -> String {
    let mut lines = String::new();
    for row in found.iter().filter(|row| row.verdict != Verdict::Ahead) {
        let _ = writeln!(
            lines,
            "{target} {} {} {}  # {}",
            row.workload,
            row.versus,
            row.measure.name(),
            row.verdict.name()
        );
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers from `from` to `to`, as floats.
    fn run_of(from: u32, to: u32) -> Vec<f64> {
        (from..=to).map(f64::from).collect()
    }

    /// Samples apart are decided, samples alike aren't, and the side is the smaller one's.
    #[test]
    fn the_test_decides_samples_that_differ_and_not_samples_alike() {
        let low = run_of(1, 20);
        let high = run_of(21, 40);
        assert!(p_smaller(&low, &high) < 1e-6, "{}", p_smaller(&low, &high));
        assert!(p_smaller(&high, &low) > 0.99);
        assert_eq!(tested_verdict(&low, &high, ALPHA).0, Verdict::Ahead);
        assert_eq!(tested_verdict(&high, &low, ALPHA).0, Verdict::Behind);
        // Twenty runs apart are enough for the first look, at its stricter level.
        assert_eq!(tested_verdict(&low, &high, LOOK_ALPHA).0, Verdict::Ahead);
        // Interleaved: neither side is smaller.
        let odd: Vec<f64> = (0..20).map(|n| f64::from(2 * n + 1)).collect();
        let even: Vec<f64> = (0..20).map(|n| f64::from(2 * n + 2)).collect();
        assert_eq!(tested_verdict(&odd, &even, ALPHA).0, Verdict::Undecided);
        // Every value the same: no evidence either way.
        let same = vec![5.0; 30];
        assert!((p_smaller(&same, &same) - 1.0).abs() < f64::EPSILON);
        assert_eq!(tested_verdict(&same, &same, ALPHA).0, Verdict::Undecided);
        assert!((p_smaller(&[], &low) - 1.0).abs() < f64::EPSILON);
    }

    /// The looks begin at twenty runs, end at the most, and split the level between them.
    #[test]
    fn the_gate_looks_at_counts_that_double_up_to_the_most() {
        for runs in [20, 40, MAX_RUNS] {
            assert!(looks_at(runs), "{runs}");
        }
        for runs in [0, 19, 21] {
            assert!(!looks_at(runs), "{runs}");
        }
        assert!(LOOKS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!((LOOK_ALPHA * 8.0 - ALPHA).abs() < 1e-12);
        // 5 against 5 values apart (p about 0.0061) pass at ALPHA but not at a look's level.
        let p = p_smaller(&run_of(1, 5), &run_of(6, 10));
        assert!(p < ALPHA && p > LOOK_ALPHA, "{p}");
    }

    /// The approximation agrees with a value worked out by hand: U = 0 for 5 against 5 values
    /// gives z = (0.5 - 12.5) / sqrt(25 * 11 / 12), whose lower tail is about 0.0061.
    #[test]
    fn the_p_value_is_the_normal_approximations() {
        let p = p_smaller(&run_of(1, 5), &run_of(6, 10));
        assert!((p - 0.006_1).abs() < 2e-4, "{p}");
        // The normal distribution at 0 is a half, and the error function's fit holds far out.
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-7);
        assert!((normal_cdf(-3.0) - 0.001_35).abs() < 1e-5);
    }

    /// Runs tied in two values, as short CPU times counted in a clock's ticks are: 15 at 0 and 5
    /// at 1 against 5 at 0 and 15 at 1. U is 100, its mean 200, and it moves in steps of 20, so
    /// the correction is 10: p is about 0.0025. The exact one-sided p, from the hypergeometric
    /// distribution of the 20 values at 0, is 265,184,142 / 137,846,528,820, about 0.00192; a
    /// correction of 1/2 gave 0.00095, below a look's level, which the exact test isn't.
    #[test]
    fn tied_runs_are_decided_no_more_boldly_than_the_exact_test() {
        let mut lengua = vec![0.0; 15];
        lengua.extend([1.0; 5]);
        let mut other = vec![0.0; 5];
        other.extend([1.0; 15]);
        let p = p_smaller(&lengua, &other);
        assert!(p >= 265_184_142.0 / 137_846_528_820.0, "{p}");
        assert!((p - 0.002_5).abs() < 1e-4, "{p}");
        assert_eq!(tested_verdict(&lengua, &other, LOOK_ALPHA).0, Verdict::Undecided);
        assert_eq!(tested_verdict(&lengua, &other, ALPHA).0, Verdict::Ahead);
    }

    #[test]
    fn a_median_is_the_middle_value() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&[]), None);
        assert_eq!(measured_verdict(1.0, 2.0), Verdict::Ahead);
        assert_eq!(measured_verdict(2.0, 1.0), Verdict::Behind);
        assert_eq!(measured_verdict(2.0, 2.0), Verdict::Undecided);
    }

    /// KNOWN_BEHIND's lines: rows of four fields, names or `*`, reasons after `#`.
    #[test]
    fn known_behind_lists_rows_of_known_names() {
        let known = KnownBehind::parse(
            "# the list\n\nlinux-x86_64 add c-wasm cpu  # the start\nmacos-aarch64 * * *\n",
        )
        .expect("a list");
        assert_eq!(known.rows.len(), 2);
        assert!(known.rows[1].every_row());
        let cpu = known.naming("linux-x86_64", "add", "c-wasm", Measure::Cpu);
        let wall = known.naming("linux-x86_64", "add", "c-wasm", Measure::Wall);
        let size = known.naming("macos-aarch64", "sort", "rust", Measure::Size);
        assert!(cpu.is_some() && wall.is_none() && size.is_some());
        let bad = KnownBehind::parse("linux-x86_64 add c-wasm\nlinux-riscv add c cpu\n")
            .expect_err("two bad lines");
        assert_eq!(bad.len(), 2, "{bad:?}");
        assert!(bad[0].contains("line 1"), "{bad:?}");
        assert!(bad[1].contains("\"linux-riscv\""), "{bad:?}");
    }

    /// A row behind that the list doesn't name fails the run, and so does one ahead that it
    /// names; the list that passes names exactly the rows not ahead.
    #[test]
    fn the_ratchet_holds_the_list_to_what_the_run_found() {
        let found = |workload: &str, measure, verdict| Found {
            workload: workload.to_owned(),
            versus: "c-wasm".to_owned(),
            measure,
            verdict,
        };
        let runs = [
            found("add", Measure::Cpu, Verdict::Behind),
            found("add", Measure::Wall, Verdict::Undecided),
            found("add", Measure::Size, Verdict::Ahead),
        ];
        let known =
            KnownBehind::parse("linux-x86_64 add c-wasm cpu\nlinux-x86_64 add c-wasm size\n")
                .expect("a list");
        let problems = ratchet(&known, "linux-x86_64", &runs);
        assert_eq!(problems.len(), 2, "{problems:?}");
        let undecided = problems[0].contains("wall: lengua is undecided");
        let ahead = problems[1].contains("size: lengua is ahead now");
        assert!(undecided && ahead, "{problems:?}");
        // The list the run proposes passes it.
        let proposed = KnownBehind::parse(&proposal("linux-x86_64", &runs)).expect("a list");
        assert!(ratchet(&proposed, "linux-x86_64", &runs).is_empty());
        // A line that names every row passes nothing ahead.
        let every = KnownBehind::parse("linux-x86_64 * * *\n").expect("a list");
        let problems = ratchet(&every, "linux-x86_64", &runs);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("names every row"), "{problems:?}");
        // Another target's lines say nothing of this one's.
        assert_eq!(ratchet(&every, "macos-aarch64", &runs).len(), 2);
    }

    /// A change may take lines out of the list and make a line narrower, as a runner's own list
    /// does in place of its line that names every row, but not name a row the list before it
    /// didn't, unless the row is new to the gate.
    #[test]
    fn the_list_may_only_shrink() {
        let list = |text: &str| KnownBehind::parse(text).expect("a list");
        let every = list("linux-x86_64 * * *\nmacos-aarch64 * * *\n");
        let narrowed = list("linux-x86_64 add c-wasm cpu\nlinux-x86_64 sort * *\n");
        let all = |_: &str| true;
        assert!(grown(&every, &narrowed, all).is_empty());
        let fewer = list("linux-x86_64 sort * *\n");
        assert!(grown(&narrowed, &fewer, all).is_empty());
        let wider = list("linux-x86_64 add c-wasm *\nlinux-x86_64 sort * *\n");
        let problems = grown(&narrowed, &wider, all);
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems[0].starts_with("linux-x86_64 add c-wasm wall:"), "{problems:?}");
        assert!(problems[0].contains("line 1"), "{problems:?}");
        let elsewhere = list("linux-x86_64 sort * *\nwindows-x86_64 add rust size\n");
        assert_eq!(grown(&narrowed, &elsewhere, all).len(), 1);
        // A workload the gate didn't have before starts with the lines it needs.
        let new = |name: &str| name != "sort";
        let before = list("linux-x86_64 add c-wasm cpu\n");
        assert!(grown(&before, &narrowed, new).is_empty());
        assert_eq!(grown(&before, &narrowed, all).len(), 24);
        // An earlier list is read with no line refused: a name the gate no longer has names no
        // row.
        let text = "linux-x86_64 grep c cpu  # gone\nlinux-x86_64 add c-wasm cpu\n";
        let old = KnownBehind::read(text);
        assert_eq!(old.rows.len(), 2);
        assert!(grown(&old, &list("linux-x86_64 add c-wasm cpu\n"), all).is_empty());
    }

    /// The repository's own KNOWN_BEHIND is a list the gate reads.
    #[test]
    fn the_repositorys_known_behind_parses() {
        let text = include_str!("../KNOWN_BEHIND");
        let known = KnownBehind::parse(text).unwrap_or_else(|problems| panic!("{problems:#?}"));
        for target in TARGETS {
            assert!(
                known.rows.iter().any(|row| row.target == target),
                "KNOWN_BEHIND says nothing of {target}"
            );
        }
    }
}
