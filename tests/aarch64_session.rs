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
    let program = written(&session, aarch64_files("cat")).await;
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
    let program = written(&session, aarch64_files("cat")).await;
    let report = session.settle(&program).await;
    assert_eq!(report["state"], "done", "{report}");
    let proof = text(&report["proofs"][0]);
    assert!(
        proof.starts_with("generated B1: proven for every input in range"),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}
