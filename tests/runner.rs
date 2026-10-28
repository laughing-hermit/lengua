//! Running programs: limits, a clean environment, stdin, signals, and instruction counts.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use lengua::platform::Profile;
use lengua::runner::{End, Limits, run};
use lengua::toolchain::Toolchain;

fn launcher() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_lengua-mcp"))
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| (*arg).to_owned()).collect()
}

/// Builds the assembly template into `folder` and returns the executable.
async fn template(folder: &Path) -> PathBuf {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugin/skills/write-assembly/assets/main-template.s");
    let toolchain = Toolchain::discover().await.expect("llvm-mc and ld.lld");
    let object = folder.join("main.o");
    let executable = folder.join("main");
    toolchain
        .assemble(Profile::LinuxX86_64, &source, &object)
        .await
        .expect("assemble the template");
    toolchain
        .link(&object, &executable)
        .await
        .expect("link the template");
    executable
}

#[tokio::test]
async fn runs_a_program_and_counts_its_instructions() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let program = template(folder.path()).await;
    let first = run(launcher(), &program, &args(&["7"]), b"", Limits::default())
        .await
        .expect("run");
    assert_eq!(first.stdout, b"-7\n");
    assert_eq!(first.stderr, b"");
    assert_eq!(first.end, End::Exited(0));
    let second = run(launcher(), &program, &args(&["7"]), b"", Limits::default())
        .await
        .expect("run again");
    match (first.instructions, second.instructions) {
        (Some(first), Some(second)) => {
            // The CPU counts the odd interrupt or page fault as an instruction.
            assert!(
                first.abs_diff(second) <= 2,
                "the same input executes the same instructions: {first} vs {second}"
            );
            assert!((50..5_000).contains(&first), "{first} instructions");
        }
        // Machines that don't allow performance counters still run programs.
        (None, None) => eprintln!("performance counters aren't available here"),
        other => panic!("counted only one run: {other:?}"),
    }
    let longer = run(
        launcher(),
        &program,
        &args(&["123456789"]),
        b"",
        Limits::default(),
    )
    .await
    .expect("run a longer input");
    if let (Some(short), Some(long)) = (first.instructions, longer.instructions) {
        assert!(
            long > short,
            "more digits take more instructions: {short} vs {long}"
        );
    }
}

#[tokio::test]
async fn reports_exit_codes_and_failed_assertions() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let program = template(folder.path()).await;
    let usage = run(launcher(), &program, &[], b"", Limits::default())
        .await
        .expect("run");
    assert_eq!(usage.end, End::Exited(1));
    assert_eq!(usage.stderr, b"usage: negate <n>\n");
}

#[tokio::test]
async fn kills_programs_that_run_too_long() {
    let limits = Limits {
        timeout: Duration::from_millis(300),
        ..Limits::default()
    };
    let started = Instant::now();
    let result = run(
        launcher(),
        Path::new("/usr/bin/sleep"),
        &args(&["10"]),
        b"",
        limits,
    )
    .await
    .expect("run sleep");
    assert_eq!(result.end, End::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn kills_programs_that_print_too_much() {
    let limits = Limits {
        max_output: 4096,
        ..Limits::default()
    };
    let result = run(launcher(), Path::new("/usr/bin/yes"), &[], b"", limits)
        .await
        .expect("run yes");
    assert_eq!(result.end, End::OutputTooLarge);
    assert_eq!(result.stdout.len(), 4096);
}

#[tokio::test]
async fn gives_an_empty_environment_and_closed_stdin() {
    let environment = run(
        launcher(),
        Path::new("/usr/bin/env"),
        &[],
        b"",
        Limits::default(),
    )
    .await
    .expect("run env");
    assert_eq!(environment.stdout, b"", "no variables leak into programs");
    assert_eq!(environment.end, End::Exited(0));
    let echo = run(
        launcher(),
        Path::new("/usr/bin/cat"),
        &[],
        b"hello\n",
        Limits::default(),
    )
    .await
    .expect("run cat");
    assert_eq!(echo.stdout, b"hello\n", "stdin is given, then ends");
    assert_eq!(echo.end, End::Exited(0));
}

#[tokio::test]
async fn reports_signals() {
    let result = run(
        launcher(),
        Path::new("/bin/sh"),
        &args(&["-c", "kill -SEGV $$"]),
        b"",
        Limits::default(),
    )
    .await
    .expect("run sh");
    assert_eq!(result.end, End::Signaled(11));
    assert!(result.end.describe(Limits::default()).contains("SIGSEGV"));
}
