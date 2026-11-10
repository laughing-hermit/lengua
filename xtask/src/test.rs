//! The suite's runs, as CI's test, ignored and golden steps make them: `cargo test` over the files
//! `TEST_FILES` names and the tests `FILTER` names, all it prints kept in a log, watched for a
//! stall when `STALL_SECONDS` gives a time, and failed when what was asked for matched no test.

use std::path::Path;

use crate::{ci, stall};

/// How the test task is called.
const USAGE: &str = "usage: cargo xtask test [--ignored]";

/// What a run that asked for something and passed nothing says.
const RAN_NOTHING: &str = "No test ran: what was asked for matched nothing. A filter matches \
                           test names, not files;\nname a file in test_files.";

/// The cargo arguments that select test targets, for ci.yml's `test_files` input: `lib` is the
/// library's own tests (`--lib`), any other name is a file under tests/ (`--test <name>`).
/// Nothing given selects nothing, so every test target runs.
pub fn selection(names: &str) -> Vec<String> {
    let mut arguments = Vec::new();
    for name in names.split_whitespace() {
        if name == "lib" {
            arguments.push("--lib".to_owned());
        } else {
            arguments.push("--test".to_owned());
            arguments.push(name.to_owned());
        }
    }
    arguments
}

/// How many tests passed in `log`, a run's output: the counts of libtest's `test result:` lines,
/// one for each test program, added up.
pub fn passed(log: &str) -> u64 {
    let mut total = 0;
    for line in log.lines() {
        if let Some(rest) = line.strip_prefix("test result: ")
            && says_passed(rest)
        {
            // `test result: ok. 12 passed; 0 failed; ...`: the fourth word is the count.
            total += line.split_whitespace().nth(3).map_or(0, leading_number);
        }
    }
    total
}

/// Whether the rest of a `test result:` line says how many passed, as ` <digits> passed`.
fn says_passed(rest: &str) -> bool {
    rest.match_indices(" passed").any(|(at, _)| {
        let before = &rest[..at];
        let number = before.trim_end_matches(|character: char| character.is_ascii_digit());
        number.len() < before.len() && number.ends_with(' ')
    })
}

/// The number `word` starts with, or 0, as awk reads a field as a number.
fn leading_number(word: &str) -> u64 {
    let rest = word.trim_start_matches(|c: char| c.is_ascii_digit());
    word[..word.len() - rest.len()].parse().unwrap_or(0)
}

/// Fails a run whose filter or files matched no test. libtest's filters match test *names* only,
/// so a filter such as `limits` (a file's name) matches nothing and the run would pass green
/// having run nothing. When `selector` is empty the whole suite ran and there is nothing to
/// check; otherwise at least one test must have passed in `log`.
pub fn ran_some(log: &Path, selector: &str) -> Result<(), String> {
    if selector.is_empty() {
        return Ok(());
    }
    let text = std::fs::read(log).unwrap_or_default();
    let count = passed(&String::from_utf8_lossy(&text));
    println!("Tests that passed: {count} (selected by: {selector})");
    if count == 0 {
        return Err(RAN_NOTHING.to_owned());
    }
    Ok(())
}

/// `cargo xtask test [--ignored]`: the suite, kept in test.log, or with `--ignored` the slow
/// ignored tests, what they print included, kept in ignored.log; the files `TEST_FILES` names
/// and the tests `FILTER` names, every one when both are empty; watched for a stall when
/// `STALL_SECONDS` gives a time.
pub fn task(arguments: &[String]) -> Result<u8, String> {
    let ignored = match arguments {
        [] => false,
        [flag] if flag == "--ignored" => true,
        _ => return Err(USAGE.to_owned()),
    };
    let filter = ci::setting("FILTER");
    let files = ci::setting("TEST_FILES");
    let limit = ci::setting("STALL_SECONDS");
    let root = ci::root()?;
    let log = root.join(if ignored { "ignored.log" } else { "test.log" });
    let mut command = ci::cargo();
    command
        .current_dir(&root)
        .args(["test", "--release", "--locked", "--workspace"])
        .arg("--no-fail-fast")
        .args(selection(&files))
        .arg("--");
    if ignored {
        command.args(["--ignored", "--nocapture"]);
    }
    command.args(filter.split_whitespace());
    let status = if limit.is_empty() {
        ci::Logged::start(command, &log)?.wait()?
    } else {
        let after = stall::limit(&limit)?;
        let mut logged = ci::Logged::start(command, &log)?;
        let status = stall::watch(&mut logged.child, after, &limit);
        logged.drain();
        status?
    };
    if !status.success() {
        return Ok(ci::code(status));
    }
    ran_some(&log, &format!("{filter}{files}"))?;
    Ok(0)
}

/// `cargo xtask golden`: the checker's and the prover's recordings of every program, made again
/// under target/golden/, kept in golden.log. target/ is cached whole, the recordings included, so
/// they go first: an old recording would be compared with silently. The two filters are the two
/// goldens' tests (the helper `explains_a_mutant` matches neither), and the step runs one test at
/// a time (`RUST_TEST_THREADS`), as the prover's recordings run under fixed time budgets.
pub fn golden(arguments: &[String]) -> Result<u8, String> {
    if !arguments.is_empty() {
        return Err("usage: cargo xtask golden".to_owned());
    }
    let root = ci::root()?;
    let recordings = root.join("target").join("golden");
    if let Err(error) = std::fs::remove_dir_all(&recordings)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(format!("{}: {error}", recordings.display()));
    }
    let log = root.join("golden.log");
    let mut command = ci::cargo();
    command
        .current_dir(&root)
        .env("LENGUA_GOLDEN", "write")
        .args(["test", "--release", "--locked", "--lib"])
        .args(["--", "--ignored", "every_report_is_as_recorded"])
        .arg("every_run_and_proof_is_as_recorded");
    let status = ci::Logged::start(command, &log)?.wait()?;
    if !status.success() {
        return Ok(ci::code(status));
    }
    ran_some(&log, "goldens")?;
    let mut names = Vec::new();
    let entries = std::fs::read_dir(&recordings)
        .map_err(|error| format!("{}: {error}", recordings.display()))?;
    for entry in entries.flatten() {
        let size = entry.metadata().map_or(0, |metadata| metadata.len());
        names.push(format!("{size:>12} {}", entry.file_name().to_string_lossy()));
    }
    names.sort();
    println!("{}:", recordings.display());
    for name in names {
        println!("{name}");
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `lib` is the library's own tests, any other name a file under tests/, and nothing given
    /// selects nothing.
    #[test]
    fn test_files_become_cargo_s_selection() {
        assert_eq!(
            selection(" lib wasm_limits  sessions "),
            ["--lib", "--test", "wasm_limits", "--test", "sessions"]
        );
        assert!(selection("").is_empty());
    }

    /// The tests that passed are counted from each test program's result line, a failing one's
    /// too, and from no other line.
    #[test]
    fn the_passed_tests_are_counted_from_libtest_s_result_lines() {
        let log = "running 3 tests\n\
                   test a ... ok\n\
                   test result: ok. 12 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out\n\
                   test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured\n\
                   test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out\n\
                   the line test result: ok. 5 passed, quoted\n\
                   test result: 7 passed\n";
        assert_eq!(passed(log), 15);
        assert_eq!(passed(""), 0);
    }

    /// A run that asked for something and passed nothing fails; the whole suite has nothing to
    /// check.
    #[test]
    fn a_selection_that_matched_nothing_fails() {
        let folder = tempfile::tempdir().expect("a folder");
        let log = folder.path().join("test.log");
        std::fs::write(&log, "test result: ok. 0 passed; 0 failed; 0 ignored\n").expect("a log");
        assert!(ran_some(&log, "").is_ok());
        let failed = ran_some(&log, "limits").expect_err("nothing ran");
        assert!(failed.contains("name a file in test_files"), "{failed}");
        std::fs::write(&log, "test result: ok. 2 passed; 0 failed; 0 ignored\n").expect("a log");
        assert!(ran_some(&log, "limits").is_ok());
        assert!(ran_some(&folder.path().join("none.log"), "limits").is_err());
    }
}
