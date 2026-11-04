//! A record of the tests that skipped themselves, and why, so that a green run says what it did
//! not cover.
//!
//! A test that returns early because this computer lacks something (a CPU extension, QEMU for
//! the other CPU, hardware counters, the solvers) calls [`note`]. libtest hides what a passing
//! test prints, so a message printed on its own is never seen in a green log; set
//! `LENGUA_SKIP_LOG` to a file and every note is appended to it, one line each, which the CI
//! workflows summarize at the end of a run.

use std::io::Write as _;
use std::path::Path;

/// Records that the running test skipped itself, because of `reason`.
pub fn note(reason: &str) {
    eprintln!("skipped: {reason}");
    if let Some(path) = std::env::var_os("LENGUA_SKIP_LOG") {
        let thread = std::thread::current();
        append(Path::new(&path), reason, thread.name().unwrap_or("?"));
    }
}

/// Appends a line `reason`, a tab and the test's name to the log at `path`.
fn append(path: &Path, reason: &str, test: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        // One write for the whole line: tests run on several threads, and a file in append mode
        // keeps a single write whole where several (as `writeln!` makes) interleave. A failed
        // write loses a note, not a result.
        let line = format!("{reason}\t{test}\n");
        let _ = file.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_is_one_line_with_its_reason_and_its_test() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let log = folder.path().join("skips.log");
        append(&log, "this CPU has no AVX-512BW", "tests::one");
        append(&log, "QEMU doesn't run AArch64 programs here", "tests::two");
        let text = std::fs::read_to_string(&log).expect("the log");
        assert_eq!(
            text,
            "this CPU has no AVX-512BW\ttests::one\n\
             QEMU doesn't run AArch64 programs here\ttests::two\n"
        );
    }
}
