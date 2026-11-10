//! `cargo xtask plan`: the jobs to run, as GitHub's job matrix, from the one list of runners
//! (xtask/targets.json): each target in each mode. A push runs the suite, the weekly run the
//! suite, the slow ignored tests and the bench, and a dispatch what it asks (`EVENT`, `MODE`,
//! `TARGETS`). The goldens' legs run on linux-x86_64 alone: the dispatched ref and, unless an
//! earlier run's recordings are reused (`GOLDEN_BASE_RUN`), the base (`GOLDEN_BASE`), each on a
//! runner of its own so that the two run at once. A job's checkout is shallow unless it must read
//! other commits: a bench holds each change to KNOWN_BEHIND in the history to the list before it
//! (the list may only shrink) and reads any commit it is measured against, and a golden leg reads
//! the commit it doesn't run. The solver mode runs two legs on each target it names, each on a
//! runner of its own so that the two run at once: `native`, which records the prover's golden
//! with native z3, keeping every solver's session, then replays z3's sessions through both z3s;
//! and `wasm`, which records it with the WebAssembly z3 (`cargo xtask solver-golden`,
//! `solver-parity`). The step's outputs are `matrix`; `bench`, whether a bench runs, which needs
//! the WebAssembly builds of C, C++ and Rust; and `solver`, whether the solver mode runs, which
//! needs z3 built as WebAssembly.

use std::process::{Command, Stdio};

use crate::ci;

/// The runners, by target.
const TARGETS: &str = include_str!("../targets.json");

/// What a golden dispatch with neither of its inputs says.
const GOLDEN_NEEDS: &str =
    "golden needs golden_base (a commit) or golden_base_run (an earlier golden run)";

/// The solver mode's legs on each target, by label: the prover's golden with native z3, its
/// sessions then replayed, and with the WebAssembly z3.
pub const SOLVER_LEGS: [&str; 2] = ["native", "wasm"];

/// A runner: the target it is for, and the label of GitHub's runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runner {
    /// The target, as lengua names it.
    pub target: String,
    /// GitHub's runner.
    pub runner: String,
}

/// The runners `list`, targets.json's text, names, in its order.
pub fn runners(list: &str) -> Result<Vec<Runner>, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(list).map_err(|error| format!("targets.json: {error}"))?;
    let entries = parsed.as_array().ok_or("targets.json isn't a list")?;
    let mut runners = Vec::new();
    for entry in entries {
        let field = |name: &str| {
            entry[name]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("an entry of targets.json has no {name}: {entry}"))
        };
        runners.push(Runner {
            target: field("target")?,
            runner: field("runner")?,
        });
    }
    Ok(runners)
}

/// What a plan is made from: the event, and a dispatch's inputs.
#[derive(Debug, Clone, Default)]
pub struct Asked {
    /// The event that started the run: push, schedule or workflow_dispatch.
    pub event: String,
    /// The mode a dispatch asks for.
    pub mode: String,
    /// The targets a dispatch asks for, between commas, or all.
    pub targets: String,
    /// The commit to record the goldens' base at.
    pub golden_base: String,
    /// An earlier golden run whose base recordings are reused.
    pub golden_base_run: String,
}

/// One job of the matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// Where it runs.
    pub runner: Runner,
    /// Its mode.
    pub mode: String,
    /// The commit it checks out, or none for the dispatched ref.
    pub reference: String,
    /// A golden leg's name: head or base.
    pub label: String,
    /// How deep its checkout is: 0 for the whole history.
    pub depth: u8,
}

impl Job {
    /// The job as the matrix holds it, in the keys' order jq wrote: the runner's, then the
    /// mode's, the ref's, the label's and the depth's, or for a golden leg its label and ref
    /// before its mode.
    pub fn json(&self) -> String {
        let quoted = |text: &str| serde_json::to_string(text).unwrap_or_default();
        let target = quoted(&self.runner.target);
        let runner = quoted(&self.runner.runner);
        let mode = quoted(&self.mode);
        let reference = quoted(&self.reference);
        let label = quoted(&self.label);
        let depth = self.depth;
        if self.mode == "golden" {
            format!(
                "{{\"target\":{target},\"runner\":{runner},\"label\":{label},\"ref\":{reference},\
                 \"mode\":{mode},\"depth\":{depth}}}"
            )
        } else {
            format!(
                "{{\"target\":{target},\"runner\":{runner},\"mode\":{mode},\"ref\":{reference},\
                 \"label\":{label},\"depth\":{depth}}}"
            )
        }
    }
}

/// The jobs `asked` means on `runners`, with `full` giving a commit's full hash: actions/checkout
/// takes a branch, a tag or a full commit hash, never a short one.
pub fn plan(
    asked: &Asked,
    runners: &[Runner],
    full: impl Fn(&str) -> Result<String, String>,
) -> Result<Vec<Job>, String> {
    let modes = match asked.event.as_str() {
        "push" => vec!["test"],
        "schedule" => vec!["test", "ignored", "bench"],
        _ => vec![asked.mode.as_str()],
    };
    let dispatched = !matches!(asked.event.as_str(), "push" | "schedule");
    let (targets, mode) = if dispatched {
        (asked.targets.as_str(), asked.mode.as_str())
    } else {
        ("all", "test")
    };
    let targets: String = targets
        .chars()
        .filter(|character| *character != ' ')
        .collect();
    let listed = format!(",{targets},");
    let everything = targets.is_empty() || targets == "all";
    let mut jobs = Vec::new();
    for runner in runners {
        if !everything && !listed.contains(&format!(",{},", runner.target)) {
            continue;
        }
        for mode in &modes {
            let labels: &[&str] = if *mode == "solver" {
                &SOLVER_LEGS
            } else {
                &[""]
            };
            for label in labels {
                jobs.push(Job {
                    runner: runner.clone(),
                    mode: (*mode).to_owned(),
                    reference: String::new(),
                    label: (*label).to_owned(),
                    depth: u8::from(*mode != "bench"),
                });
            }
        }
    }
    if mode == "golden" {
        if asked.golden_base.is_empty() && asked.golden_base_run.is_empty() {
            return Err(GOLDEN_NEEDS.to_owned());
        }
        let mut legs = vec![("head".to_owned(), String::new())];
        if asked.golden_base_run.is_empty() {
            legs.push(("base".to_owned(), full(&asked.golden_base)?));
        }
        jobs.clear();
        for runner in runners
            .iter()
            .filter(|runner| runner.target == "linux-x86_64")
        {
            for (label, reference) in &legs {
                jobs.push(Job {
                    runner: runner.clone(),
                    mode: "golden".to_owned(),
                    reference: reference.clone(),
                    label: label.clone(),
                    depth: 0,
                });
            }
        }
    }
    if jobs.is_empty() {
        return Err(format!("Nothing to run for: {targets} in {mode}"));
    }
    Ok(jobs)
}

/// The step's outputs for `jobs`: the matrix, whether a bench runs and whether the solver mode
/// runs, which need the bench-wasm and the solver-wasm jobs.
pub fn outputs(jobs: &[Job]) -> String {
    let listed: Vec<String> = jobs.iter().map(Job::json).collect();
    let include = listed.join(",");
    let bench = jobs.iter().any(|job| job.mode == "bench");
    let solver = jobs.iter().any(|job| job.mode == "solver");
    format!("matrix={{\"include\":[{include}]}}\nbench={bench}\nsolver={solver}\n")
}

/// The full hash of `commit`, as GitHub's API gives it, through gh (with `GH_TOKEN`).
fn full_hash(commit: &str) -> Result<String, String> {
    let repository = ci::setting("GITHUB_REPOSITORY");
    let mut command = Command::new("gh");
    command
        .args(["api", &format!("repos/{repository}/commits/{commit}")])
        .args(["--jq", ".sha"])
        .stderr(Stdio::inherit());
    let printed =
        ci::output_of(&mut command).ok_or_else(|| format!("{} failed", ci::shown(&command)))?;
    Ok(printed.trim().to_owned())
}

/// `cargo xtask plan`.
pub fn task(arguments: &[String]) -> Result<u8, String> {
    if !arguments.is_empty() {
        return Err("usage: cargo xtask plan".to_owned());
    }
    let asked = Asked {
        event: ci::setting("EVENT"),
        mode: ci::setting("MODE"),
        targets: ci::setting("TARGETS"),
        golden_base: ci::setting("GOLDEN_BASE"),
        golden_base_run: ci::setting("GOLDEN_BASE_RUN"),
    };
    let jobs = plan(&asked, &runners(TARGETS)?, full_hash)?;
    let outputs = outputs(&jobs);
    print!("{outputs}");
    ci::append("GITHUB_OUTPUT", &outputs)?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The runners' list, as a dispatch asks for some of them.
    fn listed() -> Vec<Runner> {
        runners(TARGETS).expect("targets.json")
    }

    /// No hash is asked for.
    fn none(commit: &str) -> Result<String, String> {
        Err(format!("asked for {commit}"))
    }

    /// targets.json lists the five runners, linux-x86_64 first.
    #[test]
    fn the_list_names_five_runners() {
        let runners = listed();
        assert_eq!(runners.len(), 5);
        assert_eq!(runners[0].target, "linux-x86_64");
        assert_eq!(runners[0].runner, "ubuntu-24.04");
    }

    /// A push runs the suite on every runner, the weekly run the suite, the ignored tests and
    /// the bench, whose checkout is whole; each job reads as jq wrote it.
    #[test]
    fn a_push_and_the_weekly_run_plan_their_modes() {
        let push = Asked {
            event: "push".to_owned(),
            ..Asked::default()
        };
        let jobs = plan(&push, &listed(), none).expect("a plan");
        assert_eq!(jobs.len(), 5);
        assert_eq!(
            jobs[0].json(),
            "{\"target\":\"linux-x86_64\",\"runner\":\"ubuntu-24.04\",\"mode\":\"test\",\
             \"ref\":\"\",\"label\":\"\",\"depth\":1}"
        );
        let weekly = Asked {
            event: "schedule".to_owned(),
            mode: "compile".to_owned(),
            ..Asked::default()
        };
        let jobs = plan(&weekly, &listed(), none).expect("a plan");
        let modes: Vec<&str> = jobs.iter().take(3).map(|job| job.mode.as_str()).collect();
        assert_eq!((jobs.len(), modes), (15, vec!["test", "ignored", "bench"]));
        assert_eq!(jobs[2].depth, 0);
    }

    /// A dispatch runs its mode on the targets it names, spaces left out, or says it names none.
    #[test]
    fn a_dispatch_runs_what_it_asks() {
        let asked = Asked {
            event: "workflow_dispatch".to_owned(),
            mode: "release".to_owned(),
            targets: "macos-aarch64, windows-aarch64".to_owned(),
            ..Asked::default()
        };
        let jobs = plan(&asked, &listed(), none).expect("a plan");
        let targets: Vec<&str> = jobs.iter().map(|job| job.runner.target.as_str()).collect();
        assert_eq!(targets, ["macos-aarch64", "windows-aarch64"]);
        let nothing = Asked {
            targets: "solaris-sparc".to_owned(),
            ..asked
        };
        let refused = plan(&nothing, &listed(), none).expect_err("nothing to run");
        assert_eq!(refused, "Nothing to run for: solaris-sparc in release");
    }

    /// The solver mode runs its two legs on each target a dispatch names, native z3's and the
    /// WebAssembly z3's, and its outputs say that it runs, so that z3's WebAssembly build does;
    /// the suite's say neither a bench nor the solver mode runs.
    #[test]
    fn the_solver_mode_asks_for_z3_s_build() {
        let asked = Asked {
            event: "workflow_dispatch".to_owned(),
            mode: "solver".to_owned(),
            targets: "linux-x86_64".to_owned(),
            ..Asked::default()
        };
        let jobs = plan(&asked, &listed(), none).expect("a plan");
        assert_eq!(jobs.len(), 2);
        assert_eq!(
            outputs(&jobs),
            "matrix={\"include\":[{\"target\":\"linux-x86_64\",\"runner\":\"ubuntu-24.04\",\
             \"mode\":\"solver\",\"ref\":\"\",\"label\":\"native\",\"depth\":1},\
             {\"target\":\"linux-x86_64\",\"runner\":\"ubuntu-24.04\",\"mode\":\"solver\",\
             \"ref\":\"\",\"label\":\"wasm\",\"depth\":1}]}\nbench=false\nsolver=true\n"
        );
        let all = Asked {
            targets: "all".to_owned(),
            ..asked
        };
        assert_eq!(plan(&all, &listed(), none).expect("a plan").len(), 10);
        let push = Asked {
            event: "push".to_owned(),
            ..Asked::default()
        };
        let suite = outputs(&plan(&push, &listed(), none).expect("a plan"));
        assert!(suite.ends_with("\nbench=false\nsolver=false\n"), "{suite}");
    }

    /// The goldens run a head and a base leg on linux-x86_64, the base at its full hash, or the
    /// head alone when an earlier run's base is reused; they need one or the other.
    #[test]
    fn the_goldens_run_their_legs() {
        let asked = Asked {
            event: "workflow_dispatch".to_owned(),
            mode: "golden".to_owned(),
            targets: "all".to_owned(),
            golden_base: "abc123".to_owned(),
            ..Asked::default()
        };
        let full = |commit: &str| Ok(format!("{commit}0000"));
        let jobs = plan(&asked, &listed(), full).expect("a plan");
        assert_eq!(jobs.len(), 2);
        assert_eq!(
            jobs[1].json(),
            "{\"target\":\"linux-x86_64\",\"runner\":\"ubuntu-24.04\",\"label\":\"base\",\
             \"ref\":\"abc1230000\",\"mode\":\"golden\",\"depth\":0}"
        );
        let reused = Asked {
            golden_base_run: "42".to_owned(),
            ..asked.clone()
        };
        assert_eq!(plan(&reused, &listed(), none).expect("a plan").len(), 1);
        let neither = Asked {
            golden_base: String::new(),
            ..asked
        };
        assert!(plan(&neither, &listed(), none).is_err());
    }
}
