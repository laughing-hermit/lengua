//! A session for linux-aarch64 checks, tests, proves and runs AArch64 code: natively on an ARM64
//! computer, and on an x86-64 one through QEMU's user mode, as a developer's `LENGUA_TARGET`
//! asks. Either way the rules hold, the tests run, the proofs are x86-64's, and lengua's line in
//! main.s is an AArch64 comment.

mod common;

use common::{Program, Protocol, Session};
use serde_json::{Value, json};

fn fixture(name: &str, file: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The AArch64 fixture `name`'s spec.md and main.s.
fn aarch64_files(name: &str) -> [(&'static str, String); 2] {
    ["spec.md", "main.s"].map(|file| (file, fixture(&format!("linux-aarch64/{name}"), file)))
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// Whether any of `lines` contains `needle`.
fn any_line(lines: &Value, needle: &str) -> bool {
    lines
        .as_array()
        .is_some_and(|lines| lines.iter().any(|line| text(line).contains(needle)))
}

fn with_args(program: &Program, args: &[&str]) -> Value {
    let mut arguments = program.arguments();
    arguments["args"] = json!(args);
    arguments
}

/// A program folder holding `files`, its spec recorded.
async fn written(session: &Session, files: [(&str, String); 2]) -> Program {
    let program = Program::new();
    for (name, text) in files {
        program.write(name, &text);
    }
    session.record(&program).await;
    program
}

/// Checks until the job ends, which must be done.
async fn done(session: &Session, program: &Program) -> Value {
    let report = session.settle(program).await;
    assert_eq!(report["state"], "done", "{report}");
    report
}

/// A session for linux-aarch64 here, and whether QEMU runs its programs: none where neither
/// this computer's CPU nor QEMU runs AArch64 programs.
async fn aarch64_session() -> Option<(Session, bool)> {
    if cfg!(target_arch = "aarch64") {
        Some((Session::start(Protocol::Discover).await, false))
    } else if common::qemu_runs_aarch64() {
        Some((
            Session::emulating(Protocol::Discover, "linux-aarch64").await,
            true,
        ))
    } else {
        None
    }
}

/// The AArch64 cat passes every rule and its tests, with no time for proofs; lengua's line is a
/// `//` comment and the lock says what ran the tests. Under QEMU the reply and run say what held the
/// runs, and a session for this computer itself runs none of it.
#[tokio::test]
async fn aarch64_code_is_checked_and_tested() {
    let Some((session, emulated)) = aarch64_session().await else {
        return;
    };
    let program = written(&session, aarch64_files("cat-stream")).await;
    let report = done(&session, &program).await;
    assert!(
        any_line(&report["tests"], "follows every rule and passed all"),
        "{report}"
    );
    assert_eq!(
        any_line(&report["tests"], "test runs under qemu-aarch64"),
        emulated,
        "{report}"
    );
    assert_eq!(
        any_line(
            &report["tests"],
            "the kernel couldn't hold its system calls to the spec, which only lengua's rules did"
        ),
        emulated,
        "{report}"
    );
    // With no time for proofs, as x86-64's say with none: the prover models AArch64 code.
    assert!(
        report["proofs"]
            .as_array()
            .is_some_and(|proofs| !proofs.is_empty()
                && proofs.iter().all(|proof| !text(proof).contains("no model"))),
        "{report}"
    );
    assert!(
        text(&report["memory"]).contains("the rest is tested, not proven"),
        "{report}"
    );
    let code = program.read("main.s").expect("main.s");
    let line = code.lines().nth(1).unwrap_or_default();
    assert!(
        line.starts_with("// lengua: compiled from spec.md sha256:")
            && line.ends_with(" for linux-aarch64"),
        "{code}"
    );
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(lock.contains("target = \"linux-aarch64\""), "{lock}");
    assert_eq!(
        lock.contains("emulator = \"qemu-aarch64"),
        emulated,
        "{lock}"
    );
    let mut arguments = program.arguments();
    arguments["stdin"] = Value::from("hello, from far away\n".repeat(400));
    let run = session.call_ok("run", arguments).await;
    assert_eq!(run["stdout"], "hello, from far away\n".repeat(400), "{run}");
    assert_eq!(run["exit_code"], 0, "{run}");
    assert_eq!(
        text(&run["emulated"]).starts_with("ran under qemu-aarch64"),
        emulated,
        "{run}"
    );
    if !emulated {
        session.service.cancel().await.expect("close the session");
        return;
    }
    assert!(run["instructions"].is_null(), "{run}");

    // This computer's own session runs nothing tested under QEMU, and AArch64 code isn't its.
    let session = session.restart_for(None).await;
    let error = session.call_error("run", program.arguments()).await;
    assert!(
        error.contains("was tested as linux-aarch64 code under qemu-aarch64")
            && error.contains("code natively"),
        "{error}"
    );
    let report = session.check(&program).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        any_line(
            &report["problems"],
            "main.s is AArch64 code, and this computer is"
        ),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

/// The AArch64 calc runs each case of its jump table, and x86-64 code isn't what a session for
/// linux-aarch64 checks.
#[tokio::test]
async fn the_aarch64_calc_runs_each_case_of_its_table() {
    let Some((session, _)) = aarch64_session().await else {
        return;
    };
    let program = written(&session, aarch64_files("calc")).await;
    done(&session, &program).await;
    for (args, stdout) in [
        (["2", "*", "3"], "6\n"),
        (["2", "+", "3"], "5\n"),
        (["2", "-", "3"], "-1\n"),
        (["-1000000000", "*", "1000000000"], "-1000000000000000000\n"),
    ] {
        let run = session.call_ok("run", with_args(&program, &args)).await;
        assert_eq!(run["stdout"], stdout, "{run}");
        assert_eq!(run["exit_code"], 0, "{run}");
    }
    let run = session
        .call_ok("run", with_args(&program, &["2", ",", "3"]))
        .await;
    assert_eq!(run["stderr"], "error: unknown operator\n", "{run}");
    assert_eq!(run["exit_code"], 3, "{run}");

    let x86 = written(
        &session,
        ["spec.md", "main.s"].map(|name| (name, fixture("calc", name))),
    )
    .await;
    let report = session.check(&x86).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        any_line(&report["problems"], "main.s is x86-64 code, and"),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

/// The AArch64 hypotenuse, x86-64's example spec unchanged, computes in doubles and passes every
/// test, its decimals rounded as x86-64's are, and its doubles are proven the formula's for whole
/// sides and decimal ones.
#[tokio::test]
async fn the_aarch64_hypotenuse_computes_in_doubles() {
    let session = if cfg!(target_arch = "aarch64") {
        Session::proving_for(Protocol::Discover, 120).await
    } else if common::qemu_runs_aarch64() {
        Session::emulating_for(Protocol::Discover, "linux-aarch64", 120).await
    } else {
        return;
    };
    let program = written(&session, aarch64_files("hypotenuse")).await;
    let report = done(&session, &program).await;
    assert!(
        any_line(&report["tests"], "follows every rule and passed all"),
        "{report}"
    );
    for (number, proof) in report["proofs"]
        .as_array()
        .expect("the proofs")
        .iter()
        .enumerate()
    {
        assert!(
            text(proof).starts_with(&format!(
                "generated B{}: proven for every input in range",
                number + 1
            )),
            "{report}"
        );
    }
    assert!(
        text(&report["memory"]).contains("the proofs of B1, B2, B3, B4 and B5 cover"),
        "{report}"
    );
    for (args, stdout) in [
        (["3", "4"], "5.000\n"),
        (["1.5", "2.25"], "2.704\n"),
        (["0.001", "0.001"], "0.001\n"),
        (["1000000", "1000000"], "1414213.562\n"),
    ] {
        let run = session.call_ok("run", with_args(&program, &args)).await;
        assert_eq!(run["stdout"], stdout, "{run}");
        assert_eq!(run["exit_code"], 0, "{run}");
    }
    let run = session
        .call_ok("run", with_args(&program, &["3", "four"]))
        .await;
    assert_eq!(run["stderr"], "error: side is not a number\n", "{run}");
    assert_eq!(run["exit_code"], 2, "{run}");
    session.service.cancel().await.expect("close the session");
}

/// The AArch64 vector template, x86-64's `shout` spec compiled with NEON, passes its tests and is
/// proven through the server: every line it shouts, with no solver question.
#[tokio::test]
async fn the_aarch64_vector_template_is_proven() {
    let session = if cfg!(target_arch = "aarch64") {
        Session::proving_for(Protocol::Discover, 120).await
    } else if common::qemu_runs_aarch64() {
        Session::emulating_for(Protocol::Discover, "linux-aarch64", 120).await
    } else {
        return;
    };
    let asset = |path: &str| {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("plugin/skills/compile/assets")
                .join(path),
        )
        .expect("the asset")
    };
    let program = written(
        &session,
        [
            ("spec.md", asset("vector-template-spec.md")),
            ("main.s", asset("linux-aarch64/vector-template.s")),
        ],
    )
    .await;
    let report = done(&session, &program).await;
    assert!(
        any_line(&report["tests"], "follows every rule and passed all"),
        "{report}"
    );
    assert!(
        text(&report["proofs"][0]).starts_with("generated B1: proven for every input in range"),
        "{report}"
    );
    let mut arguments = program.arguments();
    arguments["stdin"] = Value::from("neon shouts too\n");
    let run = session.call_ok("run", arguments).await;
    assert_eq!(run["stdout"], "NEON SHOUTS TOO\n", "{run}");
    session.service.cancel().await.expect("close the session");
}

/// The AArch64 streaming template, x86-64's `shout-stream` spec compiled with NEON, passes its
/// tests and is proven for every length through the server, as x86-64's is.
#[tokio::test]
async fn the_aarch64_stream_template_is_proven() {
    let session = if cfg!(target_arch = "aarch64") {
        Session::proving_for(Protocol::Discover, 120).await
    } else if common::qemu_runs_aarch64() {
        Session::emulating_for(Protocol::Discover, "linux-aarch64", 120).await
    } else {
        return;
    };
    let asset = |path: &str| {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("plugin/skills/compile/assets")
                .join(path),
        )
        .expect("the asset")
    };
    let program = written(
        &session,
        [
            ("spec.md", asset("stream-template-spec.md")),
            ("main.s", asset("linux-aarch64/stream-template.s")),
        ],
    )
    .await;
    let report = done(&session, &program).await;
    assert!(
        any_line(&report["tests"], "follows every rule and passed all"),
        "{report}"
    );
    assert!(
        text(&report["proofs"][0]).contains("every length by induction over its read loop"),
        "{report}"
    );
    let mut arguments = program.arguments();
    arguments["stdin"] = Value::from("a stream\nof lines\n");
    let run = session.call_ok("run", arguments).await;
    assert_eq!(run["stdout"], "A STREAM\nOF LINES\n", "{run}");
    session.service.cancel().await.expect("close the session");
}

/// The AArch64 word tally, x86-64's tally spec compiled for AArch64 (a hash table and a merge
/// sort), passes every test, inputs of a megabyte included, as x86-64's does.
#[tokio::test]
async fn the_aarch64_tally_template_passes_its_tests() {
    let session = if cfg!(target_arch = "aarch64") {
        Session::proving_for(Protocol::Discover, 30).await
    } else if common::qemu_runs_aarch64() {
        Session::emulating_for(Protocol::Discover, "linux-aarch64", 30).await
    } else {
        return;
    };
    let asset = |path: &str| {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("plugin/skills/compile/assets")
                .join(path),
        )
        .expect("the asset")
    };
    let program = written(
        &session,
        [
            ("spec.md", asset("tally-template-spec.md")),
            ("main.s", asset("linux-aarch64/tally-template.s")),
        ],
    )
    .await;
    let report = done(&session, &program).await;
    assert!(
        any_line(&report["tests"], "follows every rule and passed all"),
        "{report}"
    );
    let mut arguments = program.arguments();
    arguments["stdin"] = Value::from("the cat and the\tdog the cat\n");
    let run = session.call_ok("run", arguments).await;
    assert_eq!(run["stdout"], "3 the\n2 cat\n1 and\n1 dog\n", "{run}");
    session.service.cancel().await.expect("close the session");
}

/// The AArch64 event-loop server and key reader, x86-64's specs compiled for AArch64 with
/// `ppoll`, pass their tests through the server: clients at once on the listening socket lengua
/// hands the server, and keys from standard input as from a pipe.
#[tokio::test]
async fn the_aarch64_server_and_terminal_templates_pass_their_tests() {
    let session = if cfg!(target_arch = "aarch64") {
        Session::proving_for(Protocol::Discover, 60).await
    } else if common::qemu_runs_aarch64() {
        Session::emulating_for(Protocol::Discover, "linux-aarch64", 60).await
    } else {
        return;
    };
    let asset = |path: &str| {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("plugin/skills/compile/assets")
                .join(path),
        )
        .expect("the asset")
    };
    for (spec, code) in [
        ("server-template-spec.md", "linux-aarch64/server-template.s"),
        (
            "terminal-template-spec.md",
            "linux-aarch64/terminal-template.s",
        ),
    ] {
        let program = written(
            &session,
            [("spec.md", asset(spec)), ("main.s", asset(code))],
        )
        .await;
        let report = done(&session, &program).await;
        assert!(
            any_line(&report["tests"], "follows every rule and passed all"),
            "{code}: {report}"
        );
    }
    session.service.cancel().await.expect("close the session");
}

/// The AArch64 echo server and fetch client, x86-64's examples compiled for AArch64, pass their
/// tests through the server: a server whose clients the tests bring, and a client whose
/// connection lengua makes and hands over, answered by the test's own server. Every test runs,
/// natively and under QEMU alike: lengua's connections need no supervisor.
#[tokio::test]
async fn the_aarch64_echo_and_fetch_pass_their_tests() {
    let native = cfg!(target_arch = "aarch64");
    let session = if native {
        Session::proving_for(Protocol::Discover, 60).await
    } else if common::qemu_runs_aarch64() {
        Session::emulating_for(Protocol::Discover, "linux-aarch64", 60).await
    } else {
        return;
    };
    for name in ["echo", "fetch"] {
        let program = written(&session, aarch64_files(name)).await;
        let report = done(&session, &program).await;
        eprintln!("{name}: {}", report["tests"]);
        let passed = any_line(&report["tests"], "follows every rule and passed all");
        assert!(passed, "{name}: {report}");
    }
    session.service.cancel().await.expect("close the session");
}

/// The AArch64 parallel template, x86-64's spec compiled for AArch64 with clone, futex and ldar,
/// passes its tests through the server: a megabyte capitalized on up to 16 threads.
#[tokio::test]
async fn the_aarch64_parallel_template_passes_its_tests() {
    let session = if cfg!(target_arch = "aarch64") {
        Session::proving_for(Protocol::Discover, 60).await
    } else if common::qemu_runs_aarch64() {
        Session::emulating_for(Protocol::Discover, "linux-aarch64", 60).await
    } else {
        return;
    };
    let asset = |path: &str| {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("plugin/skills/compile/assets")
                .join(path),
        )
        .expect("the asset")
    };
    let program = written(
        &session,
        [
            ("spec.md", asset("parallel-template-spec.md")),
            ("main.s", asset("linux-aarch64/parallel-template.s")),
        ],
    )
    .await;
    let report = done(&session, &program).await;
    assert!(
        any_line(&report["tests"], "follows every rule and passed all"),
        "{report}"
    );
    let mut arguments = program.arguments();
    arguments["stdin"] = Value::from("threads, too\n");
    let run = session.call_ok("run", arguments).await;
    assert_eq!(run["stdout"], "THREADS, TOO\n", "{run}");
    session.service.cancel().await.expect("close the session");
}

/// The benchmark's own AArch64 programs, the fast wc, crc32c and histogram, each with its
/// spec, pass every check through the server, as x86-64's do.
#[tokio::test]
async fn the_aarch64_benchmarks_own_programs_pass_their_checks() {
    let session = if cfg!(target_arch = "aarch64") {
        Session::proving_for(Protocol::Discover, 60).await
    } else if common::qemu_runs_aarch64() {
        Session::emulating_for(Protocol::Discover, "linux-aarch64", 60).await
    } else {
        return;
    };
    let assets =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("plugin/skills/compile/assets");
    // The SVE wc where this CPU, or QEMU's, has SVE; its spec is the wc's.
    let sve = lengua::platform::Target {
        profile: lengua::platform::Profile::LinuxAarch64,
        emulator: (!cfg!(target_arch = "aarch64")).then(|| lengua::platform::Emulator {
            name: "qemu-aarch64".into(),
        }),
    }
    .features(&lengua::platform::Machine::detect())
    .sve > 0;
    let workloads: &[&str] = if sve {
        &["wc", "crc32c", "histogram", "wc-sve"]
    } else {
        &["wc", "crc32c", "histogram"]
    };
    for workload in workloads {
        let read = |path: std::path::PathBuf| {
            std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        };
        let spec_of = workload.trim_end_matches("-sve");
        let program = written(
            &session,
            [
                (
                    "spec.md",
                    read(assets.join("fast").join(spec_of).join("spec.md")),
                ),
                (
                    "main.s",
                    read(
                        assets
                            .join("linux-aarch64/fast")
                            .join(workload)
                            .join("main.s"),
                    ),
                ),
            ],
        )
        .await;
        let report = done(&session, &program).await;
        assert!(
            any_line(&report["tests"], "follows every rule and passed all"),
            "{workload}: {report}"
        );
    }
    session.service.cancel().await.expect("close the session");
}

/// The AArch64 cat, a streaming filter through `fill`, is proven for every length through the
/// server, as x86-64's is: natively on an ARM64 computer, and through QEMU here.
#[tokio::test]
async fn aarch64_code_is_proven() {
    let session = if cfg!(target_arch = "aarch64") {
        Session::proving_for(Protocol::Discover, 120).await
    } else if common::qemu_runs_aarch64() {
        Session::emulating_for(Protocol::Discover, "linux-aarch64", 120).await
    } else {
        return;
    };
    let program = written(&session, aarch64_files("cat-stream")).await;
    let report = session.settle(&program).await;
    assert_eq!(report["state"], "done", "{report}");
    let proof = text(&report["proofs"][0]);
    assert!(
        proof.starts_with("generated B1: proven for every input in range"),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

/// Every program folder of `root`'s `folders`, each with a main.s: its name and path.
fn programs(root: &std::path::Path, folders: &[&str]) -> Vec<(String, std::path::PathBuf)> {
    let mut found = Vec::new();
    for folder in folders {
        for entry in std::fs::read_dir(root.join(folder)).expect("a folder") {
            let path = entry.expect("an entry").path();
            if path.join("main.s").is_file()
                && let Some(name) = path.file_name().and_then(|name| name.to_str())
            {
                found.push((name.to_owned(), path.clone()));
            }
        }
    }
    found.sort();
    found
}

/// Every x86-64 example and fixture has its AArch64 twin, compiled from the very same spec, and
/// every AArch64 fixture its x86-64 one: one product on both Linux targets, only the hardware
/// differing.
#[test]
fn every_program_has_its_twin_on_the_other_cpu() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let x86 = programs(root, &["examples", "tests/fixtures"]);
    let arm = programs(root, &["tests/fixtures/linux-aarch64"]);
    let mut problems = Vec::new();
    for (name, folder) in &x86 {
        match arm.iter().find(|(other, _)| other == name) {
            None => problems.push(format!("{name}: no AArch64 twin")),
            Some((_, twin)) => {
                let spec = |folder: &std::path::Path| std::fs::read(folder.join("spec.md")).ok();
                if spec(folder) != spec(twin) {
                    problems.push(format!("{name}: its AArch64 twin's spec differs"));
                }
            }
        }
    }
    for (name, _) in &arm {
        if !x86.iter().any(|(other, _)| other == name) {
            problems.push(format!("{name}: no x86-64 twin"));
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// Every AArch64 twin follows every rule and passes its tests through the server.
#[tokio::test]
async fn every_aarch64_twin_passes_its_checks() {
    let Some((session, _)) = aarch64_session().await else {
        return;
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (name, folder) in programs(root, &["tests/fixtures/linux-aarch64"]) {
        let read = |file: &str| {
            std::fs::read_to_string(folder.join(file))
                .unwrap_or_else(|error| panic!("{name}/{file}: {error}"))
        };
        let program = written(
            &session,
            [("spec.md", read("spec.md")), ("main.s", read("main.s"))],
        )
        .await;
        let report = session.settle(&program).await;
        if name == "second" {
            // Wrong by design, as x86-64's is: one fill reads both lines only when no read ends
            // right after the first, and the tests' own pieces of standard input find it.
            assert_eq!(report["state"], "tests_failed", "{name}: {report}");
            continue;
        }
        assert_eq!(report["state"], "done", "{name}: {report}");
        assert!(
            any_line(&report["tests"], "follows every rule and passed all"),
            "{name}: {report}"
        );
    }
    session.service.cancel().await.expect("close the session");
}

/// Checks until the job ends, which must be with failed tests; returns the problems.
async fn failed(session: &Session, program: &Program) -> String {
    let report = session.settle(program).await;
    assert_eq!(report["state"], "tests_failed", "{report}");
    report["problems"]
        .as_array()
        .map(|problems| {
            problems
                .iter()
                .map(|problem| text(problem).to_owned())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

/// second's spec with a short first line and a long second one, so the tests' own pieces of
/// standard input never end a read right after the first line: only the proof's cuts do, as
/// x86-64's check_code tests it.
fn second_with_lines_the_tests_cut_past() -> String {
    let spec = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/linux-aarch64/second/spec.md"),
    )
    .expect("the spec");
    let changed = spec
        .replace(
            "Each line is `1` to `20`\nlowercase letters followed by a newline.",
            "The first line is `1` to `5`\nlowercase letters and the second `12` to `20`, each \
             followed by a newline.",
        )
        .replace("`ab\\ncd\\n`", "`ab\\nijklmnopqrst\\n`")
        .replace("prints `cd` followed", "prints `ijklmnopqrst` followed")
        .replacen("string(1, 20,", "string(1, 5,", 1)
        .replacen("string(1, 20,", "string(12, 20,", 1);
    assert!(changed.contains("string(12, 20,"), "{changed}");
    assert!(changed.contains("`ab\\nijklmnopqrst\\n`"), "{changed}");
    changed
}

/// As on x86-64: AArch64's second reads both lines with one fill, which stops at the first
/// newline a read ends with. The proof takes every cut and finds the one after the first line,
/// and a real run with standard input cut there confirms it; reading to the end instead is
/// proven however standard input arrives.
#[tokio::test]
async fn a_cut_only_the_proof_tries_is_found_and_run_for_real_on_aarch64() {
    let session = if cfg!(target_arch = "aarch64") {
        Session::proving_for(Protocol::Discover, 120).await
    } else if common::qemu_runs_aarch64() {
        Session::emulating_for(Protocol::Discover, "linux-aarch64", 120).await
    } else {
        return;
    };
    let code = aarch64_files("second")[1].1.clone();
    let program = written(
        &session,
        [
            ("spec.md", second_with_lines_the_tests_cut_past()),
            ("main.s", code.clone()),
        ],
    )
    .await;
    let problems = failed(&session, &program).await;
    assert!(
        problems.contains("the proof found inputs the tests didn't try where main.s is wrong"),
        "{problems}"
    );
    assert!(
        problems.contains("standard input arriving in two pieces, cut after byte"),
        "{problems}"
    );
    let whole = code.replacen(
        "    mov w3, #NEWLINE\n    bl fill\n",
        "    mov w3, #-1\n    bl fill\n",
        1,
    );
    assert_ne!(whole, code);
    program.write("main.s", &whole);
    let report = done(&session, &program).await;
    assert!(
        text(&report["proofs"][0]).ends_with("standard input arriving cut into pieces any way"),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

/// As on x86-64: AArch64's add, whose B4 closes its standard output, exits with the code its spec
/// promises when writing fails, and one that exits with another code fails it.
#[tokio::test]
async fn a_closed_standard_output_fails_every_write_on_aarch64() {
    let Some((session, _)) = aarch64_session().await else {
        return;
    };
    let [(_, spec), (_, code)] = aarch64_files("add");
    assert!(spec.contains("- And standard output is closed"), "add's B4");
    let program = written(&session, [("spec.md", spec), ("main.s", code.clone())]).await;
    done(&session, &program).await;
    let wrong = code.replacen(".equ EXIT_WRITE_FAILED, 3", ".equ EXIT_WRITE_FAILED, 4", 1);
    assert_ne!(wrong, code);
    program.write("main.s", &wrong);
    let problems = failed(&session, &program).await;
    assert!(problems.contains("B4"), "{problems}");
    session.service.cancel().await.expect("close the session");
}

/// As on x86-64: AArch64's fetch with its server's computer named by its first argument passes
/// its tests, which stand in for any value the argument takes, and a real run reads the argument
/// first: `/hello` names no computer, so the connection comes ended and fetch exits with 3.
#[tokio::test]
async fn a_connection_to_the_computer_an_argument_names_on_aarch64() {
    let Some((session, _)) = aarch64_session().await else {
        return;
    };
    let [(_, spec), code] = aarch64_files("fetch");
    let typed = spec.replace(
        "- Network: connects to TCP port 8080",
        "- Network: connects to TCP port 8080 on the computer its first argument names",
    );
    assert_ne!(typed, spec);
    let program = written(&session, [("spec.md", typed), code]).await;
    done(&session, &program).await;
    let run = session
        .call_ok("run", with_args(&program, &["/hello"]))
        .await;
    assert_eq!(run["exit_code"], 3, "{run}");
    assert!(
        text(&run["connections"][0]["problem"]).contains("isn't a computer's name"),
        "{run}"
    );
    session.service.cancel().await.expect("close the session");
}

/// As on x86-64: second with one read of its own in place of fill is right only when both lines
/// arrive in one read, and the tests, which send standard input in pieces, find it.
#[tokio::test]
async fn a_raw_read_fails_when_standard_input_arrives_in_pieces_on_aarch64() {
    let Some((session, _)) = aarch64_session().await else {
        return;
    };
    let [spec, (_, code)] = aarch64_files("second");
    let raw = code.replacen(
        "    mov w3, #NEWLINE\n    bl fill\n",
        "    mov x8, #SYS_READ\n    svc #0\n",
        1,
    );
    assert_ne!(raw, code);
    let program = written(&session, [spec, ("main.s", raw)]).await;
    let problems = failed(&session, &program).await;
    assert!(
        problems.contains(
            "standard input arriving in three pieces: the first byte, up to the middle, and the \
             rest:"
        ),
        "{problems}"
    );
    session.service.cancel().await.expect("close the session");
}

/// One folder's check, for porting a program: `LENGUA_CHECK_FOLDER` names it.
#[tokio::test]
#[ignore = "checks the folder LENGUA_CHECK_FOLDER names: by hand"]
async fn check_one_aarch64_folder() {
    let Some(folder) = std::env::var("LENGUA_CHECK_FOLDER")
        .ok()
        .filter(|folder| !folder.is_empty())
    else {
        return;
    };
    // LENGUA_CHECK_NATIVE checks it as this computer's own program: an x86-64 twin on x86-64.
    let native = std::env::var_os("LENGUA_CHECK_NATIVE").is_some();
    let session = if cfg!(target_arch = "aarch64") || native {
        Session::proving_for(Protocol::Discover, 60).await
    } else if common::qemu_runs_aarch64() {
        Session::emulating_for(Protocol::Discover, "linux-aarch64", 60).await
    } else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    let read = |file: &str| std::fs::read_to_string(folder.join(file)).expect("the file");
    let program = written(
        &session,
        [("spec.md", read("spec.md")), ("main.s", read("main.s"))],
    )
    .await;
    let report = session.settle(&program).await;
    println!("PORT state: {}", report["state"]);
    for field in ["problems", "tests", "proofs"] {
        for line in report[field].as_array().into_iter().flatten() {
            println!("PORT {field}: {}", text(line));
        }
    }
    session.service.cancel().await.expect("close the session");
}
