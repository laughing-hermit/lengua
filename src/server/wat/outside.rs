//! An outside module's path through `check` and `run`: main.wasm, which a compiler made from
//! Rust, C or another language, for an idea whose `- Module:` line names it
//! ([`crate::wasm::outside`]). `check` reads the module and refuses what lengua's host can't
//! hold by itself, runs the idea's tests against it in the workers, as it runs main.wat's, and
//! stamps it; it checks no rule and proves nothing, writes no entry in idea.md's `verified`, and
//! replies `trusted_not_proven` where main.wat gets `done`, with every guarantee that does not
//! hold in the reply's `outside`. A run holds the module to the hash the lock records and reads
//! it again as check did.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use super::super::{
    ChangesReport, CheckReport, Checking, LenguaServer, again, check_wait, wait_for,
};
use super::{Runnable, WasmCode};
use crate::graph;
use crate::hash::Sha256Hex;
use crate::idea::Idea;
use crate::state::{CODE_FILE, IDEA_FILE, OUTSIDE_FILE, ProgramFolder};
use crate::testfile::TestSuite;
use crate::trust;
use crate::wasm::outside::{self, STATE};
use crate::wasm::text::Built;
use crate::wasm::{Policy, Program};

/// What each generated test's proof line says for an outside module.
pub(super) const NOT_PROVEN: &str =
    "not proven: an outside module is tested, and lengua proves nothing of it";

/// Why an idea that names a secret doesn't run as an outside module.
pub(super) const NO_SECRET: &str = "idea.md names a secret, and its program is main.wasm, an \
                                    outside module, which keeps no secret that lengua can check";

/// What `lengua-mcp check`, the attester, says of an outside module's folder.
pub(super) const NOT_ATTESTED: &str = "main.wasm: an outside module, which is trusted, not \
                                       proven: lengua attests an idea and its main.wat, by its \
                                       rules and its proofs, and holds main.wasm to neither";

/// The module of `ready`, an outside module's run, whose bytes are `wasm`: provided they are
/// byte for byte the module check tested, and still a module lengua takes with what the idea
/// grants, as check read it ([`outside::read`]).
pub(super) fn as_tested(
    folder: &ProgramFolder,
    ready: &Runnable,
    wasm: &[u8],
) -> Result<Built, String> {
    if Sha256Hex::of(wasm) != ready.module {
        return Err(format!(
            "{OUTSIDE_FILE} differs from the module check tested, so it won't run. Call check \
             with path {folder} to test it."
        ));
    }
    let policy = Policy::of(&ready.uses, &ready.network);
    outside::read(wasm.to_vec(), &policy)
        .map(|module| module.built)
        .map_err(|problems| {
            format!(
                "{OUTSIDE_FILE} isn't a module lengua runs, so it won't run: {}. Call check with \
                 path {folder}.",
                problems.join("; ")
            )
        })
}

impl LenguaServer {
    /// `check`'s stages for an outside module, once its idea is recorded: main.wat beside it is
    /// a code file too many; main.wasm is read and held to what lengua takes
    /// ([`outside::read`]); then its tests run in the background, as main.wat's do, and the
    /// lock stamps it. Nothing is checked against a rule or proven, and `events` gets
    /// [`graph::Event::Trusted`], never done.
    pub(super) async fn check_outside(
        &self,
        folder: &ProgramFolder,
        events: &mut Vec<graph::Event>,
        began: Instant,
        idea_text: &str,
        idea: Idea,
        suite: TestSuite,
    ) -> Result<CheckReport, String> {
        let built_from = idea
            .outside
            .as_ref()
            .map(|named| named.built_from.clone())
            .unwrap_or_default();
        let unread = || outside::said(&built_from, None);
        // An outside module is no part: a contract offers functions of main.wat's text, which
        // the memory rules measure, and main.wasm has neither. (That it builds on no part is
        // the idea's own problem, said at the idea.)
        if !matches!(crate::wasm::contract::read(idea_text), Ok(None)) {
            let problem = format!(
                "{IDEA_FILE} has a `# Contract` section, and its program is {OUTSIDE_FILE}, an \
                 outside module, which offers no function to another program: a part's \
                 functions are taken from its {CODE_FILE}, word for word, and held to the \
                 memory rules. Remove the section; or, for a part, remove the `- Module:` line \
                 and compile the idea to {CODE_FILE}"
            );
            let next = format!(
                "Next: remove `# Contract` from {folder}/{IDEA_FILE}, then call check with path \
                 {folder}."
            );
            return Ok(CheckReport {
                outside: unread(),
                ..CheckReport::problems("code_problems", vec![problem], next)
            });
        }
        if folder.file(CODE_FILE).exists() {
            let problem = format!(
                "{folder} holds {CODE_FILE} beside an idea whose `- Module:` line names \
                 {OUTSIDE_FILE} as its program: one code file too many. Delete {CODE_FILE}; or, \
                 for a program that lengua checks and proves, remove the `- Module:` line and \
                 write `computation: {CODE_FILE}`"
            );
            let next = format!(
                "Next: delete {folder}/{CODE_FILE}, or make {folder}/{IDEA_FILE} an idea of \
                 {CODE_FILE} again, then call check with path {folder}."
            );
            return Ok(CheckReport {
                outside: unread(),
                ..CheckReport::problems("code_problems", vec![problem], next)
            });
        }
        let bytes = crate::state::read_file(&folder.file(OUTSIDE_FILE))
            .map_err(|error| format!("couldn't read {folder}/{OUTSIDE_FILE}: {error}"))?;
        let Some(bytes) = bytes else {
            return Ok(CheckReport {
                outside: unread(),
                ..CheckReport::new(
                    "write_code",
                    format!(
                        "Next: build the program for wasm32-wasip1 into {folder}/{OUTSIDE_FILE}, \
                         as the compile skill's references/outside.md says, then call check \
                         with path {folder}."
                    ),
                )
            });
        };
        let files = [
            trust::idea_hash(idea_text.as_bytes()),
            Sha256Hex::of(&bytes),
        ];
        let checking = match self.current_checking(folder, &files, false).await {
            Some(checking) => checking,
            None => {
                let policy = Policy::of(&idea.uses, &idea.network);
                let module = match outside::read(bytes, &policy) {
                    Ok(module) => module,
                    Err(problems) => {
                        let next = format!(
                            "Next: build {OUTSIDE_FILE} again as the compile skill's \
                             references/outside.md says, put it in {folder}, then call check \
                             with path {folder}."
                        );
                        return Ok(CheckReport {
                            outside: unread(),
                            ..CheckReport::problems("code_problems", problems, next)
                        });
                    }
                };
                let said = outside::said(&built_from, Some(&module));
                let features = module.features.clone();
                let program = Program::new(module.built).map_err(|error| {
                    format!("lengua couldn't keep {OUTSIDE_FILE}'s module: {error}")
                })?;
                let code = WasmCode {
                    idea,
                    suite,
                    program: Arc::new(program),
                    module: files[1].clone(),
                    features,
                    max_stack_bytes: 0,
                    unsafe_blocks: 0,
                    obligations: Vec::new(),
                    parts: Vec::new(),
                    claims: Vec::new(),
                    machine_code: None,
                    calls: Vec::new(),
                    outside: Some(said),
                    functions: Vec::new(),
                    data: Vec::new(),
                };
                self.start_wasm_checking(folder, files, code, false).await
            }
        };
        wait_for(&checking, began + check_wait()).await;
        let report = self.checking_report(folder, &checking);
        if report.state == STATE {
            events.push(graph::Event::Trusted);
        }
        Ok(report)
    }

    /// What `check` replies about an outside module's checking job, with `said`, what every
    /// reply says of the module: its tests still running, failed, or all passed, which is
    /// `trusted_not_proven` and never done. No line names a rule it follows, a proof or memory
    /// safety, and no reply claims lengua's entry in idea.md's `verified`.
    pub(in crate::server) fn outside_report(
        &self,
        folder: &ProgramFolder,
        checking: &Checking,
        said: &[String],
    ) -> CheckReport {
        let outside = said.to_vec();
        let Ok(found) = checking.found.lock() else {
            return CheckReport {
                outside,
                ..CheckReport::new("testing", again(folder))
            };
        };
        if !found.failures.is_empty() {
            return CheckReport {
                outside,
                ..CheckReport::problems(
                    "tests_failed",
                    found.failures.clone(),
                    format!(
                        "Next: decide from {IDEA_FILE} whether {OUTSIDE_FILE} or the test is \
                         wrong; fix the program's source and build {OUTSIDE_FILE} again, or fix \
                         the test in {IDEA_FILE} with the write-idea skill, then call check with \
                         path {folder}. Boundary values run first every time; the other \
                         generated inputs are new on every check."
                    ),
                )
            };
        }
        let Some(passed) = &found.passed else {
            return CheckReport {
                outside,
                ..CheckReport::new(
                    "testing",
                    format!("The tests are running. {}", again(folder)),
                )
            };
        };
        checking.green_told.store(true, Ordering::Relaxed);
        let mut tests = vec![format!(
            "{OUTSIDE_FILE} passed all {} test runs; lengua holds it to none of {CODE_FILE}'s \
             rules",
            passed.runs
        )];
        tests.extend(passed.every_value.iter().cloned());
        let proofs: Vec<String> = found
            .proofs
            .iter()
            .map(|proof| {
                let detail = proof.detail.as_deref().unwrap_or(NOT_PROVEN);
                format!("{}: {detail}", proof.test)
            })
            .collect();
        let changes = self.current_changes(folder);
        let changes = (changes.requests.len() > 1
            || changes.idea.is_some()
            || !changes.tests_less.is_empty())
        .then_some(ChangesReport {
            requests: changes.requests,
            idea_changed: changes.idea,
            tests_less: changes.tests_less,
        });
        let shown = if changes.is_some() {
            " changes lists what changed after the user's request: tell them each change, and why."
        } else {
            ""
        };
        CheckReport {
            tests,
            proofs,
            example: passed.shown.clone(),
            performance: passed.performance.clone(),
            changes,
            shell: Some(self.shell_command(folder)),
            outside,
            ..CheckReport::new(
                STATE,
                format!(
                    "Trusted, not proven: tell the user where the program is, what its idea \
                     promises, that {OUTSIDE_FILE} passed every test, and each line of \
                     `outside`: that it is an outside module, what lengua does not hold or \
                     prove for it, and what lengua's host holds.{shown} run runs it on their \
                     own inputs here; give them the shell command, followed by its arguments, \
                     for their own terminal, scripts and services."
                ),
            )
        }
    }
}
