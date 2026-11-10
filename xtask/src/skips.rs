//! `cargo xtask skips [<log>]`: what the tests skipped, as Markdown in the job's summary and its
//! log, from the file `LENGUA_SKIP_LOG` names (`lengua::skiplog` writes it, skips.log when none is
//! given): one row for each reason, with the number of tests that skipped for it. A green run so
//! says what it did not cover, which libtest hides.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::ci;

/// `cargo xtask skips [<log>]`.
pub fn task(arguments: &[String]) -> Result<u8, String> {
    let log = arguments.first().map_or("skips.log", String::as_str);
    let text = std::fs::read(log).unwrap_or_default();
    ci::print_and_summarize(&summary(&String::from_utf8_lossy(&text)))?;
    Ok(0)
}

/// The skip log `log` as Markdown. A line is a reason, a tab and a test, and a test that notes
/// the same reason twice counts once; the reasons with the most tests come first, and the
/// reasons with as many in reverse order, as `sort -u | cut -f1 | sort | uniq -c | sort -rn`
/// leaves them; a reason's words are kept, each run of spaces made one.
pub fn summary(log: &str) -> String {
    let mut text = String::from("### What the tests skipped\n\n");
    if log.is_empty() {
        text.push_str("Nothing: no test skipped itself.\n");
        return text;
    }
    let lines: BTreeSet<&str> = log.lines().collect();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for line in lines {
        let reason = line.split_once('\t').map_or(line, |(reason, _)| reason);
        *counts.entry(reason).or_default() += 1;
    }
    let mut rows: Vec<(&str, usize)> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.cmp(a.0)));
    text.push_str("| tests | because |\n|---:|---|\n");
    for (reason, count) in rows {
        let words: Vec<&str> = reason.split_whitespace().collect();
        let _ = writeln!(text, "| {count} | {} |", words.join(" "));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No log, or an empty one, says nothing was skipped.
    #[test]
    fn an_empty_log_says_nothing_was_skipped() {
        assert_eq!(
            summary(""),
            "### What the tests skipped\n\nNothing: no test skipped itself.\n"
        );
    }

    /// Each reason counts its tests once each, the most first, as the shell's pipeline counted
    /// them.
    #[test]
    fn each_reason_counts_its_tests_once() {
        let log = "z3 isn't installed\ttests::one\n\
                   z3 isn't installed\ttests::one\n\
                   z3 isn't installed\ttests::two\n\
                   no home  folder\ttests::three\n\
                   a counter\ttests::four\n";
        assert_eq!(
            summary(log),
            "### What the tests skipped\n\n\
             | tests | because |\n\
             |---:|---|\n\
             | 2 | z3 isn't installed |\n\
             | 1 | no home folder |\n\
             | 1 | a counter |\n"
        );
    }
}
