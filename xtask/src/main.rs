//! lengua's tooling, in Rust: the tasks CI's workflow runs, the release archive, the goldens'
//! comparison and the bench's WebAssembly builds. `cargo xtask <task>` runs one (the alias in
//! .cargo/config.toml); each task does what one step of ci.yml needs, or what a person runs by
//! hand. A task that fails says why on its error output and exits 1, or with the exit code of
//! the program it ran for the step.

mod bench;
mod ci;
mod clock;
mod fetch;
mod files;
mod format;
mod gate;
mod golden;
mod http;
mod machine;
mod plan;
mod plugin;
mod release;
mod skips;
mod stall;
mod test;

use std::process::ExitCode;

/// How `cargo xtask` is called, and its tasks.
const USAGE: &str = "usage: cargo xtask <task> [<argument>...]

Tasks:
  plan                the jobs to run, as GitHub's matrix (EVENT, MODE, TARGETS, GOLDEN_BASE,
                      GOLDEN_BASE_RUN)
  format              cargo fmt --check, and fmt.patch where it differs
  lock                lock/Cargo.lock where Cargo.lock is behind Cargo.toml
  fetch <target> solvers|wasi...
                      z3 and cvc5, or wasi-sdk, as xtask/tools.tsv pins them
  machine <target>    the runner's facts, in the job's summary
  test [--ignored]    the suite, or the slow ignored tests, as FILTER, TEST_FILES and
                      STALL_SECONDS say
  golden              the checker's and the prover's recordings, made again
  stall <seconds> <program> [<argument>...]
                      a program, stopped with where it stood after that long
  skips [<log>]       what the tests skipped, in the job's summary
  plugin              the server built into plugin/server/
  release [<target>]  this computer's release archive, in dist/
  release-check       the release check, built into dist/check/
  bench               the gate against C, C++ and Rust (LENGUA_BENCH_*)
  bench-report <report>
                      the bench's report, in the job's summary
  golden-download     both commits' recordings (RUN, BASE_RUN)
  gate                whether the run passed (RESULTS, GOLDEN_DIFF, BENCH_WASM, INSTALL)
  help                this list";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some((task, rest)) = arguments.split_first() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let done = match task.as_str() {
        "plan" => plan::task(rest),
        "format" => format::format(rest),
        "lock" => format::lock(rest),
        "fetch" => fetch::task(rest),
        "machine" => machine::task(rest),
        "test" => test::task(rest),
        "golden" => test::golden(rest),
        "stall" => stall::task(rest),
        "skips" => skips::task(rest),
        "plugin" => plugin::task(rest),
        "release" => release::task(rest),
        "release-check" => release::check(rest),
        "bench" => bench::bench(rest),
        "bench-report" => bench::report(rest),
        "golden-download" => golden::download(rest),
        "gate" => gate::task(rest),
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            Ok(0)
        }
        other => {
            eprintln!("cargo xtask: no task named {other:?}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match done {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
