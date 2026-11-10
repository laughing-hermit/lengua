//! `cargo xtask gate`: the one check that says whether the whole run passed, from its jobs'
//! results as the workflow gives them: `RESULTS`, plan's, format's and test's, which must all
//! have succeeded; and `GOLDEN_DIFF`, `BENCH_WASM`, `SOLVER_WASM`, `SOLVER_REPORT` and `INSTALL`,
//! which run in some modes only, so may have been skipped.

use crate::ci;

/// Whether the run passed: every one of `results` succeeded, and each of `optional` succeeded
/// or was skipped.
pub fn passed(results: &str, optional: &[&str]) -> bool {
    results.split_whitespace().all(|result| result == "success")
        && optional
            .iter()
            .all(|result| matches!(*result, "success" | "skipped"))
}

/// `cargo xtask gate`.
pub fn task(arguments: &[String]) -> Result<u8, String> {
    if !arguments.is_empty() {
        return Err("usage: cargo xtask gate".to_owned());
    }
    let results = ci::setting("RESULTS");
    let golden = ci::setting("GOLDEN_DIFF");
    let bench = ci::setting("BENCH_WASM");
    let solver = ci::setting("SOLVER_WASM");
    let report = ci::setting("SOLVER_REPORT");
    let install = ci::setting("INSTALL");
    println!(
        "plan, format, test: {results}; golden diff: {golden}; bench-wasm: {bench}; \
         solver-wasm: {solver}; solver report: {report}; install: {install}"
    );
    Ok(u8::from(!passed(
        &results,
        &[&golden, &bench, &solver, &report, &install],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every required job must succeed; a job that runs in some modes only may be skipped.
    #[test]
    fn the_run_passes_when_every_job_did() {
        let skipped = ["skipped", "success", "skipped"];
        assert!(passed("success success success", &skipped));
        assert!(!passed("success failure success", &skipped));
        assert!(!passed("success success success", &["cancelled"]));
        assert!(passed("", &[]));
    }
}
