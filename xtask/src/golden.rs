//! The goldens' comparison job (ci.yml's golden mode).
//!
//! - `cargo xtask golden-download`: the recordings of both commits, from the runs' artifacts,
//!   through gh (with `GH_TOKEN`): the head's from this run (`RUN`) into golden/head, and the
//!   base's from this run or from an earlier one whose recordings are reused (`BASE_RUN`) into
//!   golden/base.
//! - `cargo xtask golden-diff raw|ledger <base dir> <head dir>`: compares them, in the job's log
//!   and its summary. Each folder holds the two recordings of the WebAssembly lengua somewhere
//!   beneath it: checker-wasm.txt and prover-wasm.txt (src/checker/golden.rs and
//!   src/prove/golden.rs write them under target/golden/). A base from before W11 has the native
//!   lengua's four instead, which nothing here compares: the tag native-final keeps the scripts
//!   that did.
//!
//! The raw view holds the two files to be identical, byte for byte: the view while no program
//! changes, so that a change to the checker or the prover that moves any report or proof shows.
//! The ledger view compares only the prover's proofs, one line for each program and block, with
//! what changes when a program is rewritten left out ([`ledger`]): the view after programs
//! change. xtask/golden-expected.txt lists, one regular expression a line, the lines to leave
//! out of both sides before comparing (a difference that is expected and understood), and
//! [`RENAMES`] renames the base's programs to the head's, for the ledger.
//!
//! The runners view compares no two commits: it holds the recordings the targets made of the
//! dispatched ref, one leg each (`cargo xtask plan`), against each other, for the finish line's
//! "the same verdicts on every runner". `golden-download` puts them in golden/head (`VIEW`),
//! each in its artifact's folder, golden-runner-<target>-<commit>, and `golden-diff runners`
//! reads that folder alone. The checker's recording and the model's runs in the prover's must
//! be the same text on every runner, and each block's proof the same verdict in the ledger; a
//! block is named with every runner's verdict where they differ ([`across`]).
//!
//! It exits 0 when the two agree; 1 when they differ or a recording is missing; and 2 when the
//! only differences are proofs that ran out of time or of memory on one side (the machine's
//! speed and load, not the code): run the legs again.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;

use crate::plan::{RUNNER_LEG, RUNNERS_VIEW};
use crate::{ci, files, plan};

/// The lines the comparison leaves out of both sides, one regular expression a line.
const EXPECTED: &str = include_str!("../golden-expected.txt");

/// How the comparison is called.
const USAGE: &str = "usage: cargo xtask golden-diff raw|ledger|runners <base dir> <head dir>";

/// The checker's recording.
const CHECKER: &str = "checker-wasm.txt";

/// The prover's recording.
const PROVER: &str = "prover-wasm.txt";

/// The renames from a base's program paths to the head's, for the ledger, each a regular
/// expression and its replacement, applied in turn to the start of each of the base's lines. A
/// step that moves programs adds its renames here, as W11's layout did: it moved the WebAssembly
/// programs out of tests/fixtures-wasm, the examples' twins and secret-equal to examples/, the
/// others to tests/fixtures/, so a base recorded before the move names them by their old folders.
pub const RENAMES: [(&str, &str); 2] = [
    (
        concat!(
            "^tests/fixtures-wasm/(absdiff|aes|archive|count|dice|fetch|hypotenuse|mean|",
            "secret-equal|sha256|sort|upcase)/"
        ),
        "examples/$1/",
    ),
    (r"^tests/fixtures-wasm/", "tests/fixtures/"),
];

/// A ledger: each block's verdict, by its program and block.
type Ledger = BTreeMap<String, String>;

/// What leaves a proof's verdict out of the ledger, each a regular expression and what it
/// becomes: the counts (shapes, paths, questions, the induction's reach and lookback), which
/// change with every rewrite of a program and say nothing of whether it is proven; the words of
/// a bound's `why`, which name functions and lines; main.wat's line numbers; and hexadecimal
/// numbers. What stays is what a proof covers: the kind of verdict, the bound's bytes, the cut,
/// the induction, and which functions no run reached.
const UNCOUNTED: [(&str, &str); 5] = [
    (r"[0-9]+ (shapes|paths|questions), ", ""),
    (r", (reach|lookback) [0-9]+", ""),
    (r#"why: "([^"\\]|\\.)*""#, "why: _"),
    (r"main\.wat line [0-9]+", "main.wat line _"),
    (r"0x[0-9a-fA-F]+", "0x_"),
];

/// A proof that ran out of time or of memory says nothing of the code: it differs only in the
/// machine (src/prove/watch.rs: how much memory is free as the terms of a proof grow).
const LIMITED: &str = "~ (out of time|proven as far as the time allowed)|terms grew too large for \
                       the memory|ran low on memory";

/// `cargo xtask golden-download`.
pub fn download(arguments: &[String]) -> Result<u8, String> {
    if !arguments.is_empty() {
        return Err("usage: cargo xtask golden-download".to_owned());
    }
    let run = ci::setting("RUN");
    let base_run = ci::setting("BASE_RUN");
    let base_run = if base_run.is_empty() { &run } else { &base_run };
    let runners = format!("golden-{RUNNER_LEG}*");
    let legs = if ci::setting("VIEW") == RUNNERS_VIEW {
        vec![(&run, runners.as_str(), "golden/head")]
    } else {
        vec![
            (&run, "golden-head-*", "golden/head"),
            (base_run, "golden-base-*", "golden/base"),
        ]
    };
    for (from, pattern, folder) in legs {
        let mut command = Command::new("gh");
        command
            .args(["run", "download", from])
            .args(["--pattern", pattern, "--dir", folder]);
        ci::must(&mut command)?;
    }
    Ok(0)
}

/// Each `(pattern, replacement)` of `table`, made.
fn made(table: &[(&str, &'static str)]) -> Result<Vec<(Regex, &'static str)>, String> {
    let mut made = Vec::new();
    for (pattern, replacement) in table {
        let regex = Regex::new(pattern).map_err(|error| format!("{pattern}: {error}"))?;
        made.push((regex, *replacement));
    }
    Ok(made)
}

/// The patterns of `listed`, golden-expected.txt's text, without its comments and blank lines.
pub fn expected(listed: &str) -> Result<Vec<Regex>, String> {
    let mut patterns = Vec::new();
    for line in listed.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        patterns.push(Regex::new(line).map_err(|error| format!("golden-expected.txt: {error}"))?);
    }
    Ok(patterns)
}

/// `text` without the lines `expected` matches, as `grep -v -E -f` gives it; all of `text` when
/// there is no pattern, as `cat` does.
pub fn without_expected(text: &str, expected: &[Regex]) -> String {
    if expected.is_empty() {
        return text.to_owned();
    }
    let mut kept = String::new();
    for line in text.lines() {
        if !expected.iter().any(|pattern| pattern.is_match(line)) {
            kept.push_str(line);
            kept.push('\n');
        }
    }
    kept
}

/// The proof ledger of `recording`, a prover recording: for each program (a `== path` line) and
/// each generated block (a `  B<n> proofs: ...` line), one `path B<n>: verdict` line, without
/// [`UNCOUNTED`]'s parts.
pub fn ledger(recording: &str) -> Result<Vec<String>, String> {
    let uncounted = made(&UNCOUNTED)?;
    let mut program = "";
    let mut lines = Vec::new();
    for line in recording.lines() {
        if let Some(name) = line.strip_prefix("== ") {
            program = name;
            continue;
        }
        let Some(rest) = line.strip_prefix("  B") else {
            continue;
        };
        let after = rest.trim_start_matches(|c: char| c.is_ascii_digit());
        let digits = rest.len() - after.len();
        let Some(verdict) = after.strip_prefix(" proofs: ") else {
            continue;
        };
        if digits == 0 {
            continue;
        }
        let mut said = format!("{program} B{}: {verdict}", &rest[..digits]);
        for (pattern, replacement) in &uncounted {
            said = pattern.replace_all(&said, *replacement).into_owned();
        }
        lines.push(said);
    }
    Ok(lines)
}

/// `lines` as the comparison joins them: each one's key, its program and block, and its
/// verdict, after `renames` and without what `expected` matches.
fn keyed(
    lines: &[String],
    renames: &[(Regex, &str)],
    expected: &[Regex],
) -> Result<Ledger, String> {
    let key = Regex::new(r"^(.* B[0-9]+): ").map_err(|error| error.to_string())?;
    let mut text = String::new();
    for line in lines {
        let mut renamed = line.clone();
        for (pattern, replacement) in renames {
            renamed = pattern.replace(&renamed, *replacement).into_owned();
        }
        text.push_str(&renamed);
        text.push('\n');
    }
    let mut keyed = BTreeMap::new();
    for line in without_expected(&text, expected).lines() {
        let split = key.replace(line, "$1\t");
        let (block, verdict) = split.split_once('\t').unwrap_or((&*split, ""));
        keyed.insert(block.to_owned(), verdict.to_owned());
    }
    Ok(keyed)
}

/// Compares the two ledgers `base` and `head` of the recording `name`: what it says, and the
/// exit code it gives.
fn compare(name: &str, base: &Ledger, head: &Ledger) -> (String, u8) {
    let limited = Regex::new(LIMITED).ok();
    let unknown = |verdict: &str| {
        limited
            .as_ref()
            .is_some_and(|regex| regex.is_match(verdict))
    };
    let mut said = String::new();
    let (mut same, mut differ, mut rerun, mut missing) = (0, 0, 0, 0);
    for (key, was) in base {
        let Some(is) = head.get(key) else {
            let _ = writeln!(said, "{name}: only in the base: {key}");
            missing += 1;
            continue;
        };
        if was == is {
            same += 1;
            continue;
        }
        let kind = if unknown(was) || unknown(is) {
            rerun += 1;
            "LIMIT"
        } else {
            differ += 1;
            "DIFFERS"
        };
        let _ = writeln!(
            said,
            "{name}: {kind} {key}\n    base: {was}\n    head: {is}"
        );
    }
    for key in head.keys().filter(|key| !base.contains_key(*key)) {
        let _ = writeln!(said, "{name}: only in the head: {key}");
        missing += 1;
    }
    let _ = writeln!(
        said,
        "{name}: {same} blocks agree, {differ} differ, {rerun} ran out of time or memory on one \
         side, {missing} missing"
    );
    let code = if differ + missing > 0 {
        1
    } else {
        u8::from(rerun > 0) * 2
    };
    (said, code)
}

/// The recording called `name` beneath `folder` (artifacts download into a folder of their
/// own), as `find` meets it first.
fn recording(folder: &Path, name: &str) -> Option<PathBuf> {
    files::files(folder)
        .into_iter()
        .find(|file| file.file_name().is_some_and(|found| found == name))
}

/// A file's text.
fn read(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// One runner's recordings of a commit.
struct Recorded {
    /// The runner's target.
    target: String,
    /// The checker's recording.
    checker: String,
    /// The prover's recording.
    prover: String,
}

/// What stands for a verdict where a runner's ledger has no line for a block.
const NO_LINE: &str = "(no line)";

/// The lines of `recording`, a prover's, that aren't a block's proofs: the programs and the
/// model's runs of their samples, which no solver's time changes.
fn runs(recording: &str) -> String {
    let mut kept = String::new();
    for line in recording.lines() {
        if !line.contains(" proofs: ") {
            kept.push_str(line);
            kept.push('\n');
        }
    }
    kept
}

/// The runners' recordings beneath `folder`, in targets.json's order: each in the folder its
/// artifact downloaded into, `golden-runner-<target>-<commit>`. A target with no such folder
/// recorded nothing in this run.
fn recorded_beneath(folder: &Path) -> Result<Vec<Recorded>, String> {
    let listing =
        std::fs::read_dir(folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    let mut entries: Vec<PathBuf> = Vec::new();
    for entry in listing.flatten() {
        entries.push(entry.path());
    }
    let mut found = Vec::new();
    for runner in plan::every_runner()? {
        let prefix = format!("golden-{RUNNER_LEG}{}-", runner.target);
        let Some(path) = entries.iter().find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
        }) else {
            continue;
        };
        let text = |name: &str| match recording(path, name) {
            Some(file) => read(&file),
            None => Err(format!("{}: no {name}", path.display())),
        };
        found.push(Recorded {
            checker: text(CHECKER)?,
            prover: text(PROVER)?,
            target: runner.target,
        });
    }
    Ok(found)
}

/// Compares the runners' recordings of one commit with each other, leaving out what `expected`
/// matches: what it says, and the exit code it gives. The checker's reports and the model's
/// runs must be the same text on every runner as on the first; each block's proof must be the
/// same verdict in every runner's ledger. A block whose verdicts differ is named with each
/// verdict and the runners that gave it: LIMIT where the verdicts that didn't run out of time
/// or of memory agree, DIFFERS where they don't, MISSING where a runner has no line for it.
/// A difference or a missing block is 1, a limit alone 2, and agreement 0.
fn across(recorded: &[Recorded], expected: &[Regex]) -> Result<(String, u8), String> {
    if recorded.is_empty() {
        return Ok(("no runner's recordings to compare\n".to_owned(), 1));
    }
    let targets: Vec<&str> = recorded.iter().map(|one| one.target.as_str()).collect();
    let mut said = format!("the recordings of {}\n", targets.join(", "));
    let mut status = u8::from(!same_texts(recorded, expected, &mut said));
    let mut ledgers = Vec::new();
    for one in recorded {
        ledgers.push(keyed(&ledger(&one.prover)?, &[], expected)?);
    }
    let keys: BTreeSet<&String> = ledgers.iter().flat_map(Ledger::keys).collect();
    let limited = Regex::new(LIMITED).map_err(|error| error.to_string())?;
    let (mut same, mut differ, mut rerun, mut missing) = (0, 0, 0, 0);
    for key in keys {
        let mut by_verdict: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (one, ledger) in recorded.iter().zip(&ledgers) {
            let verdict = ledger.get(key).map_or(NO_LINE, String::as_str);
            by_verdict
                .entry(verdict)
                .or_default()
                .push(one.target.as_str());
        }
        let settled = by_verdict
            .keys()
            .filter(|verdict| !limited.is_match(verdict))
            .count();
        let kind = if by_verdict.contains_key(NO_LINE) {
            missing += 1;
            "MISSING"
        } else if by_verdict.len() == 1 {
            same += 1;
            continue;
        } else if settled <= 1 {
            rerun += 1;
            "LIMIT"
        } else {
            differ += 1;
            "DIFFERS"
        };
        let _ = writeln!(said, "{PROVER}: {kind} {key}");
        for (verdict, targets) in &by_verdict {
            let _ = writeln!(said, "    {}: {verdict}", targets.join(", "));
        }
    }
    let _ = writeln!(
        said,
        "{PROVER}: {same} blocks have the same verdict on {} runners, {differ} differ, {rerun} \
         ran out of time or memory on some, {missing} missing on some",
        recorded.len()
    );
    if differ + missing > 0 {
        status = 1;
    } else if rerun > 0 && status == 0 {
        status = 2;
    }
    Ok((said, status))
}

/// Whether the checker's reports, and the model's runs in the prover's recording, are the same
/// text on every runner of `recorded` as on the first, leaving out what `expected` matches.
/// `said` gets a line for each of the two, and the first lines of each difference.
fn same_texts(recorded: &[Recorded], expected: &[Regex], said: &mut String) -> bool {
    let Some((first, others)) = recorded.split_first() else {
        return false;
    };
    let mut all_same = true;
    for (name, what, runs_only) in [
        (CHECKER, "the checker's reports", false),
        (PROVER, "the model's runs", true),
    ] {
        let text_of = |one: &Recorded| {
            if runs_only {
                without_expected(&runs(&one.prover), expected)
            } else {
                without_expected(&one.checker, expected)
            }
        };
        let was = text_of(first);
        let mut differing = Vec::new();
        for other in others {
            let is = text_of(other);
            if is == was {
                continue;
            }
            differing.push(other.target.as_str());
            let labels = (
                format!("{}/{name}", first.target),
                format!("{}/{name}", other.target),
            );
            let diff = similar::udiff::unified_diff(
                similar::Algorithm::Myers,
                &was,
                &is,
                1,
                Some((labels.0.as_str(), labels.1.as_str())),
            );
            for line in diff.lines().take(20) {
                said.push_str(line);
                said.push('\n');
            }
        }
        if differing.is_empty() {
            let _ = writeln!(
                said,
                "{name}: {what} are identical on {} runners, {} lines",
                recorded.len(),
                was.matches('\n').count()
            );
        } else {
            let _ = writeln!(
                said,
                "{name}: {what} DIFFER from {}'s on {}",
                first.target,
                differing.join(", ")
            );
            all_same = false;
        }
    }
    all_same
}

/// `cargo xtask golden-diff raw|ledger|runners <base dir> <head dir>`.
pub fn diff(arguments: &[String]) -> Result<u8, String> {
    let [view, base, head] = arguments else {
        eprintln!("{USAGE}");
        return Ok(64);
    };
    if view == RUNNERS_VIEW {
        let recorded = recorded_beneath(Path::new(head))?;
        let (said, status) = across(&recorded, &expected(EXPECTED)?)?;
        ci::print_and_summarize(&said)?;
        return Ok(status);
    }
    let names: &[&str] = match view.as_str() {
        "raw" => &["checker-wasm.txt", "prover-wasm.txt"],
        "ledger" => &["prover-wasm.txt"],
        _ => {
            eprintln!("{USAGE}");
            return Ok(64);
        }
    };
    let expected = expected(EXPECTED)?;
    let renames = made(&RENAMES)?;
    let mut said = String::new();
    let mut status = 0;
    for name in names {
        let in_base = recording(Path::new(base), name);
        let in_head = recording(Path::new(head), name);
        let (Some(was), Some(is)) = (&in_base, &in_head) else {
            let shown = |found: &Option<PathBuf>| {
                found
                    .as_ref()
                    .map_or_else(|| "none".to_owned(), |path| path.display().to_string())
            };
            let (was, is) = (shown(&in_base), shown(&in_head));
            let _ = writeln!(
                said,
                "{name}: a recording is missing (base: {was}, head: {is})"
            );
            status = 1;
            continue;
        };
        if view == "raw" {
            let was = without_expected(&read(was)?, &expected);
            let is = without_expected(&read(is)?, &expected);
            if was == is {
                let _ = writeln!(
                    said,
                    "{name}: identical, {} lines",
                    is.matches('\n').count()
                );
            } else {
                let _ = writeln!(
                    said,
                    "{name}: DIFFERS (the first 60 lines of the difference follow)"
                );
                let labels = (format!("base/{name}"), format!("head/{name}"));
                let diff = similar::udiff::unified_diff(
                    similar::Algorithm::Myers,
                    &was,
                    &is,
                    3,
                    Some((labels.0.as_str(), labels.1.as_str())),
                );
                for line in diff.lines().take(60) {
                    said.push_str(line);
                    said.push('\n');
                }
                status = 1;
            }
        } else {
            let was = keyed(&ledger(&read(was)?)?, &renames, &expected)?;
            let is = keyed(&ledger(&read(is)?)?, &[], &expected)?;
            let (compared, code) = compare(name, &was, &is);
            said.push_str(&compared);
            status = status.max(code);
        }
    }
    ci::print_and_summarize(&said)?;
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A base's prover recording, as src/prove/golden.rs writes one: each program by its
    /// main.wat, each block's proofs, and a line of another kind.
    const RECORDING: &str = "== examples/add/main.wat\n  B1 proofs: proven: 12 shapes, 3 paths, \
                             40 questions, bound why: \"at \\\"$f\\\"\" main.wat line 14 0x1F, \
                             reach 7, lookback 2\n  B2 proofs: not proven: ~ out of time\n== \
                             tests/fixtures/echo/main.wat\n  B1 proofs: proven\n  B9 checks: \
                             none\n";

    /// The head's recording of the same programs after a rewrite: add's B1 proven again with
    /// other counts, words, lines and numbers, its B2 proven in time, echo's B1 refuted, and a
    /// new program.
    const HEAD: &str = "== examples/add/main.wat\n  B1 proofs: proven: 9 shapes, 2 paths, 31 \
                        questions, bound why: \"in \\\"$g\\\"\" main.wat line 20 0x2A, reach 5, \
                        lookback 1\n  B2 proofs: proven: 1 shapes, 1 paths, 2 questions, bound \
                        None\n== tests/fixtures/echo/main.wat\n  B1 proofs: counterexample of \
                        [\"input\"]\n== tests/fixtures/new/main.wat\n  B1 proofs: proven\n";

    /// What the ledger view says of [`RECORDING`] and [`HEAD`].
    const COMPARED: &str = "prover-wasm.txt: LIMIT examples/add/main.wat B2\n    base: not \
                            proven: ~ out of time\n    head: proven: bound None\n\
                            prover-wasm.txt: DIFFERS tests/fixtures/echo/main.wat B1\n    \
                            base: proven\n    head: counterexample of [\"input\"]\n\
                            prover-wasm.txt: only in the head: tests/fixtures/new/main.wat B1\n\
                            prover-wasm.txt: 1 blocks agree, 1 differ, 1 ran out of time or \
                            memory on one side, 1 missing\n";

    /// The ledger leaves the counts, a bound's words, main.wat's lines and hexadecimal numbers
    /// out of each block's verdict, as the shell's awk and sed did.
    #[test]
    fn the_ledger_leaves_out_what_a_rewrite_changes() {
        let lines = ledger(RECORDING).expect("a ledger");
        assert_eq!(
            lines,
            [
                "examples/add/main.wat B1: proven: bound why: _ main.wat line _ 0x_",
                "examples/add/main.wat B2: not proven: ~ out of time",
                "tests/fixtures/echo/main.wat B1: proven",
            ]
        );
    }

    /// A base's programs are renamed to the head's, W11's moves among them.
    #[test]
    fn a_base_s_programs_are_renamed_to_the_head_s() {
        let renames = made(&RENAMES).expect("the renames");
        let lines = [
            "tests/fixtures-wasm/count/main.wat B1: proven".to_owned(),
            "tests/fixtures-wasm/add/main.wat B1: proven".to_owned(),
        ];
        let keyed = keyed(&lines, &renames, &[]).expect("keyed");
        let keys: Vec<&str> = keyed.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "examples/count/main.wat B1",
                "tests/fixtures/add/main.wat B1",
            ]
        );
    }

    /// The comparison names each block that differs, ran out of time on one side, or is in one
    /// ledger only, and counts them with those that agree: a difference or a missing block is
    /// 1, a limit alone 2, and agreement 0.
    #[test]
    fn the_ledgers_are_compared_block_by_block() {
        let base = keyed(&ledger(RECORDING).expect("a ledger"), &[], &[]).expect("keyed");
        let head = keyed(&ledger(HEAD).expect("a ledger"), &[], &[]).expect("keyed");
        let (said, code) = compare("prover-wasm.txt", &base, &head);
        assert_eq!(said, COMPARED);
        assert_eq!(code, 1);
        let mut limited = base.clone();
        let key = "examples/add/main.wat B2";
        limited.insert(key.to_owned(), "proven: bound None".to_owned());
        assert_eq!(compare("prover-wasm.txt", &base, &limited).1, 2);
        assert_eq!(compare("prover-wasm.txt", &base, &base).1, 0);
    }

    /// A runner's recordings: the checker's two lines, and `prover`.
    fn recorded_on(target: &str, prover: &str) -> Recorded {
        Recorded {
            target: target.to_owned(),
            checker: "== examples/add/main.wat\n  no problem\n".to_owned(),
            prover: prover.to_owned(),
        }
    }

    /// The runners view names each block whose verdict isn't the same on every runner, with
    /// each verdict and the runners that gave it: a limit where the verdicts that didn't run
    /// out of time agree, a difference where they don't, and a block a runner has no line for.
    /// The checker's reports and the model's runs are held to the first runner's text. A
    /// difference is 1, a limit alone 2, and agreement 0.
    #[test]
    fn the_runners_recordings_are_compared_with_each_other() {
        let recorded = [
            recorded_on("linux-x86_64", RECORDING),
            recorded_on("linux-aarch64", RECORDING),
            recorded_on("macos-aarch64", HEAD),
        ];
        let (said, code) = across(&recorded, &[]).expect("compared");
        assert_eq!(code, 1, "{said}");
        for line in [
            "the recordings of linux-x86_64, linux-aarch64, macos-aarch64\n",
            "checker-wasm.txt: the checker's reports are identical on 3 runners, 2 lines\n",
            "prover-wasm.txt: the model's runs DIFFER from linux-x86_64's on macos-aarch64\n",
            "prover-wasm.txt: LIMIT examples/add/main.wat B2\n    linux-x86_64, linux-aarch64: \
             not proven: ~ out of time\n    macos-aarch64: proven: bound None\n",
            "prover-wasm.txt: DIFFERS tests/fixtures/echo/main.wat B1\n    macos-aarch64: \
             counterexample of [\"input\"]\n    linux-x86_64, linux-aarch64: proven\n",
            "prover-wasm.txt: MISSING tests/fixtures/new/main.wat B1\n    linux-x86_64, \
             linux-aarch64: (no line)\n    macos-aarch64: proven\n",
            "prover-wasm.txt: 1 blocks have the same verdict on 3 runners, 1 differ, 1 ran out \
             of time or memory on some, 1 missing on some\n",
        ] {
            assert!(said.contains(line), "no {line:?} in:\n{said}");
        }
        let agreeing = [
            recorded_on("linux-x86_64", RECORDING),
            recorded_on("windows-aarch64", RECORDING),
        ];
        let (said, code) = across(&agreeing, &[]).expect("compared");
        assert_eq!(code, 0, "{said}");
        let in_time = RECORDING.replace(
            "not proven: ~ out of time",
            "proven: 1 shapes, 1 paths, 2 questions, bound None",
        );
        let limited = [
            recorded_on("linux-x86_64", RECORDING),
            recorded_on("windows-aarch64", &in_time),
        ];
        let (said, code) = across(&limited, &[]).expect("compared");
        assert_eq!(code, 2, "{said}");
        assert_eq!(across(&[], &[]).expect("compared").1, 1);
    }

    /// golden-expected.txt's patterns are regular expressions; comments and blank lines are no
    /// pattern, and a pattern leaves its lines out of both sides.
    #[test]
    fn expected_lines_are_left_out() {
        expected(EXPECTED).expect("golden-expected.txt's patterns");
        let patterns = expected("# why\n\n^== examples/sha256$\n").expect("a pattern");
        assert_eq!(patterns.len(), 1);
        let kept = without_expected("a\n== examples/sha256\nb", &patterns);
        assert_eq!(kept, "a\nb\n");
        assert_eq!(without_expected("a\nb", &[]), "a\nb");
    }
}
