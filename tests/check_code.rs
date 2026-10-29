//! Checking main.s through the server: the rules, building, the tests, the proof, and run.

mod common;

use std::path::PathBuf;

use common::{Program, Protocol, Session};
use serde_json::{Value, json};

fn asset(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugin/skills/write-assembly/assets")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// A file of one of the examples in examples/.
fn example(program: &str, name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(program)
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn fixture(program: &str, name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(program)
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
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

fn solvers_installed() -> bool {
    let installed = std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_ok();
    if !installed {
        eprintln!("z3 isn't installed");
    }
    installed
}

fn has_avx512() -> bool {
    let has = lengua::platform::Machine::detect().features.avx512bw;
    if !has {
        eprintln!("this CPU has no AVX-512BW");
    }
    has
}

/// A program folder holding the template's spec (recorded) and tests (checked).
async fn checked_template(session: &Session) -> Program {
    let program = Program::new();
    program.write("spec.md", &asset("template-spec.md"));
    program.write("tests.toml", &asset("template-tests.toml"));
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    program
}

/// A program folder holding all three files of `files`.
async fn written(session: &Session, files: [(&str, String); 3]) -> Program {
    let program = Program::new();
    for (name, text) in files {
        program.write(name, &text);
    }
    session.record(&program).await;
    program
}

/// An example's three files.
fn example_files(name: &str) -> [(&'static str, String); 3] {
    ["spec.md", "tests.toml", "main.s"].map(|file| (file, example(name, file)))
}

/// Checks until the job ends, which must be done.
async fn done(session: &Session, program: &Program) -> Value {
    let report = session.settle(program).await;
    assert_eq!(report["state"], "done", "{report}");
    report
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

async fn session() -> Session {
    Session::start(Protocol::Discover).await
}

fn with_args(program: &Program, args: &[&str]) -> Value {
    let mut arguments = program.arguments();
    arguments["args"] = json!(args);
    arguments
}

#[tokio::test]
async fn the_template_passes_its_tests_and_is_stamped() {
    let session = session().await;
    let program = checked_template(&session).await;
    program.write("main.s", &asset("main-template.s"));
    let report = done(&session, &program).await;
    assert_eq!(
        report["tests"][0], "main.s follows every rule and passed all 253 test runs",
        "{report}"
    );
    let costs = report["performance"]
        .as_array()
        .expect("what each test cost");
    assert_eq!(
        costs.len(),
        5,
        "3 examples and 2 generated blocks: {report}"
    );
    assert!(
        text(&costs[0]).starts_with("example B1: 1 run")
            && text(&costs[0]).contains("its input, run alone 5 times: median "),
        "{report}"
    );
    // The times come from the block's largest input, run alone: the minus sign and 19 digits.
    assert!(
        text(&costs[3]).starts_with("generated B1: 200 runs")
            && text(&costs[3]).contains("its largest input, 20 bytes, run alone 5 times"),
        "{report}"
    );
    assert_eq!(
        report["example"],
        "example B1: given arguments [\"7\"], the program printed \"-7\\n\", exited with code 0",
        "{report}"
    );
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(
        lock.contains("[code]") && lock.contains("executable_sha256 = "),
        "{lock}"
    );
    assert!(lock.contains("toolchain = \"llvm-mc "), "{lock}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn hardcoded_answers_fail_generated_tests() {
    let session = session().await;
    let program = checked_template(&session).await;
    // Always prints -7 for any number: right for the example, wrong for the rest.
    let code = asset("main-template.s").replacen(
        "    js .Lstart_not_a_number\n    neg rdx\n",
        "    js .Lstart_not_a_number\n    mov rdx, 7\n    neg rdx\n",
        1,
    );
    program.write("main.s", &code);
    let problems = failed(&session, &program).await;
    assert!(problems.contains("generated B1"), "{problems}");
    assert!(problems.contains("(seed "), "{problems}");
    assert!(problems.contains("but was \"-7\\n\""), "{problems}");
    assert!(
        !program
            .read("lengua.lock")
            .expect("the lock")
            .contains("[code]")
    );
    // Calling again with nothing changed runs the tests again, with new inputs.
    assert!(failed(&session, &program).await.contains("(seed "));
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn failed_assertions_name_their_line() {
    let session = session().await;
    let program = checked_template(&session).await;
    // The bounds check now trips on the second character.
    let code = asset("main-template.s").replacen("    cmp rcx, MAX_CHARS\n", "    cmp rcx, 1\n", 1);
    program.write("main.s", &code);
    let problems = failed(&session, &program).await;
    let line = code
        .lines()
        .position(|line| line == "    mov edi, 4")
        .expect("assertion 4")
        + 1;
    assert!(
        problems.contains(&format!("assertion 4 failed at main.s:{line}")),
        "{problems}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn run_needs_matching_files_and_the_tested_build() {
    let session = session().await;
    let program = checked_template(&session).await;
    let code = asset("main-template.s");
    program.write("main.s", &code);
    let error = session.call_error("run", program.arguments()).await;
    assert!(
        error.contains("doesn't"),
        "run refuses unchecked code: {error}"
    );
    done(&session, &program).await;

    let report = session.call_ok("run", with_args(&program, &["7"])).await;
    assert_eq!(report["stdout"], "-7\n", "{report}");
    assert_eq!(report["exit_code"], 0, "{report}");
    assert_eq!(report["ended"], "exit code 0", "{report}");
    let report = session.call_ok("run", with_args(&program, &[])).await;
    assert_eq!(report["stderr"], "usage: negate <n>\n", "{report}");
    assert_eq!(report["exit_code"], 1, "{report}");

    // A build that differs from the tested one (as after an LLVM upgrade) doesn't run.
    let key_bytes: [u8; 32] = std::fs::read(session.data_dir.path().join("key"))
        .expect("the signing key")
        .try_into()
        .expect("32 bytes");
    let key = lengua::lock::Key::from_bytes(key_bytes);
    let mut lock = lengua::lock::read(program.path(), Some(&key))
        .expect("a valid lock")
        .expect("a lock");
    let stamp = lock.code.as_mut().expect("code stamp");
    stamp.executable_sha256 = lengua::hash::Sha256Hex::of(b"another build");
    stamp.toolchain = "llvm-mc 21.0.0, ld.lld 21.0.0".into();
    lock.proofs = None;
    lengua::lock::write(program.path(), &lock, &key).expect("write the lock");
    let error = session.call_error("run", with_args(&program, &["7"])).await;
    assert!(
        error.contains("differs from the one check tested"),
        "{error}"
    );
    assert!(error.contains("built with llvm-mc 21.0.0"), "{error}");
    done(&session, &program).await;
    session.call_ok("run", with_args(&program, &["7"])).await;

    // Any edit stops run until it's checked again.
    program.write("main.s", &format!("{code}\n"));
    let error = session.call_error("run", with_args(&program, &["7"])).await;
    assert!(error.contains("call check"), "{error}");
    program.write("main.s", &code);
    let lock_text = program.read("lengua.lock").expect("the lock");
    program.write(
        "lengua.lock",
        &lock_text.replace("max_stack_bytes = ", "max_stack_bytes = 1"),
    );
    let error = session.call_error("run", with_args(&program, &["7"])).await;
    assert!(error.contains("can't be used"), "{error}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn violations_come_with_rule_line_and_fix() {
    let session = session().await;
    let program = checked_template(&session).await;
    let code = asset("main-template.s").replacen(
        "    mov rax, qword ptr [rsp]\n",
        "    and rsp, -16\n    mov rax, qword ptr [rsp]\n",
        1,
    );
    program.write("main.s", &code);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    let line = code
        .lines()
        .position(|line| line == "    and rsp, -16")
        .expect("the inserted line")
        + 1;
    assert!(
        any_line(
            &report["problems"],
            &format!("rule 3 (fixed resources) main.s:{line}: `and` changes rsp")
        ),
        "{report}"
    );
    assert!(
        text(&report["next_step"]).contains("write-assembly skill"),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn main_s_needs_no_label_and_must_build() {
    let session = session().await;
    let program = checked_template(&session).await;
    let code = asset("main-template.s");
    // A label left from older versions of lengua is only a comment.
    program.write(
        "main.s",
        &format!("# lengua: spec sha256:{}\n{code}", "0".repeat(64)),
    );
    done(&session, &program).await;

    program.write(
        "main.s",
        &code.replacen("    xor r9d, r9d\n", "    mov r9d, rbx\n", 1),
    );
    let report = session.check(&program).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        any_line(&report["problems"], "doesn't build: assembling failed"),
        "{report}"
    );
    assert!(
        any_line(&report["problems"], "main.s:"),
        "the assembler's message names the line: {report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn code_waits_for_checked_tests() {
    let session = session().await;
    let program = Program::new();
    program.write("spec.md", &asset("template-spec.md"));
    program.write("main.s", &asset("main-template.s"));
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_tests", "{report}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_vector_template_passes_its_tests() {
    let session = session().await;
    let program = written(
        &session,
        [
            ("spec.md", asset("vector-template-spec.md")),
            ("tests.toml", asset("vector-template-tests.toml")),
            ("main.s", asset("vector-template.s")),
        ],
    )
    .await;
    let report = done(&session, &program).await;
    assert!(
        text(&report["tests"][0]).ends_with("passed all 253 test runs"),
        "{report}"
    );
    let mut arguments = program.arguments();
    arguments["stdin"] = Value::from("lengua checks vectors\n");
    let run = session.call_ok("run", arguments).await;
    assert_eq!(run["stdout"], "LENGUA CHECKS VECTORS\n", "{run}");
    assert_eq!(run["exit_code"], 0, "{run}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn small_ranges_run_every_value() {
    let session = session().await;
    let spec = asset("template-spec.md").replace(
        "between `-9223372036854775807`\nand `9223372036854775807`",
        "between `-100`\nand `100`",
    );
    assert_ne!(spec, asset("template-spec.md"), "the range was narrowed");
    let tests = asset("template-tests.toml").replace(
        "int(-9223372036854775807, 9223372036854775807)",
        "int(-100, 100)",
    );
    let program = written(
        &session,
        [
            ("spec.md", spec),
            ("tests.toml", tests),
            ("main.s", asset("main-template.s")),
        ],
    )
    .await;
    let report = done(&session, &program).await;
    assert_eq!(
        report["tests"],
        json!([
            "main.s follows every rule and passed all 254 test runs",
            "generated B1 tried every value of n in its range (all 201)"
        ]),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_decimal_examples_pass_their_tests() {
    // mean computes in exact cents; hypotenuse in doubles, with sqrtsd.
    for (name, args, stdout) in [
        ("mean", ["2.5", "3.76"], "3.13\n"),
        ("hypotenuse", ["3", "4"], "5.000\n"),
    ] {
        let session = session().await;
        let program = written(&session, example_files(name)).await;
        done(&session, &program).await;
        let run = session.call_ok("run", with_args(&program, &args)).await;
        assert_eq!(run["stdout"], stdout, "{name}: {run}");
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn the_file_example_reads_and_writes_files() {
    // upcase reads one file and writes another; its tests give files and check the files left.
    let session = session().await;
    let program = written(&session, example_files("upcase")).await;
    let report = done(&session, &program).await;
    assert!(
        text(&report["example"]).contains("left out.txt holding \"HELLO, WORLD 42!\\n\""),
        "{report}"
    );

    // Run on a folder of the user's: the program works on the files there, and only there.
    let user = tempfile::tempdir().expect("a user folder");
    std::fs::write(user.path().join("notes.txt"), "Hello, files!\n").expect("a user file");
    let mut arguments = with_args(&program, &["notes.txt", "loud.txt"]);
    arguments["folder"] = json!(user.path());
    let run = session.call_ok("run", arguments.clone()).await;
    assert_eq!(run["exit_code"], 0, "{run}");
    assert_eq!(
        std::fs::read_to_string(user.path().join("loud.txt")).expect("written"),
        "HELLO, FILES!\n"
    );
    arguments["args"] = json!(["/etc/hostname", "loud.txt"]);
    let outside = session.call_ok("run", arguments.clone()).await;
    assert_eq!(
        outside["exit_code"], 2,
        "files outside the folder can't be opened: {outside}"
    );
    arguments["folder"] = json!("relative/folder");
    let error = session.call_error("run", arguments).await;
    assert!(error.contains("folder must be an absolute path"), "{error}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_parallel_template_runs_on_several_cores() {
    let session = session().await;
    let program = written(
        &session,
        [
            ("spec.md", asset("parallel-template-spec.md")),
            ("tests.toml", asset("parallel-template-tests.toml")),
            ("main.s", asset("parallel-template.s")),
        ],
    )
    .await;
    let report = done(&session, &program).await;
    assert!(
        report["performance"].as_array().is_some_and(|costs| costs
            .iter()
            .all(|cost| text(cost).contains("µs on the clock"))),
        "{report}"
    );
    // A megabyte comes back capitalized: sixteen threads each did their part.
    let text: String = "Hello, threads! ".repeat(65536);
    let mut arguments = program.arguments();
    arguments["stdin"] = json!(text);
    let run = session.call_ok("run", arguments).await;
    assert_eq!(run["exit_code"], 0, "{run}");
    assert_eq!(run["stdout"], text.to_ascii_uppercase());
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn tests_can_be_fixed_after_code_ran() {
    let session = session().await;
    let program = checked_template(&session).await;
    program.write("main.s", &asset("main-template.s"));
    let report = done(&session, &program).await;
    assert!(report.get("changes").is_none(), "{report}");
    let tests = asset("template-tests.toml");

    // A fix after code ran is checked against the spec like any version, and shown with lengua's
    // own marks on what tests less; nobody has to approve anything.
    program.write("tests.toml", &tests.replace("runs = 200", "runs = 10"));
    let report = done(&session, &program).await;
    let changed = text(&report["changes"]["tests_changed"]);
    assert!(changed.contains("- ") && changed.contains("+ "), "{report}");
    assert!(changed.contains("runs = 10"), "{report}");
    assert_eq!(
        report["changes"]["tests_less"],
        json!(["generated B1: fewer runs, 10 instead of 200"]),
        "{report}"
    );

    // A change that tests more gets no mark, and going back to the first version shows nothing.
    program.write("tests.toml", &tests.replace("runs = 200", "runs = 300"));
    let report = done(&session, &program).await;
    assert!(report["changes"].get("tests_less").is_none(), "{report}");
    assert!(report["changes"]["tests_changed"].is_string(), "{report}");
    program.write("tests.toml", &tests);
    let report = done(&session, &program).await;
    assert!(report.get("changes").is_none(), "{report}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn new_words_in_the_middle_of_a_build_never_hide_a_change() {
    let session = session().await;
    let program = checked_template(&session).await;
    // The build isn't finished: no code has passed yet. Changing the spec and recording it with
    // other words keeps the first spec as what changes are shown against.
    let spec = asset("template-spec.md");
    let edited = spec.replacen("# Purpose\n", "# Purpose\nIt is quick. ", 1);
    assert_ne!(spec, edited, "the edit applies");
    program.write("spec.md", &edited);
    session
        .call_ok("check", program.request("Also make it quick"))
        .await;
    program.write("main.s", &asset("main-template.s"));
    let report = done(&session, &program).await;
    assert_eq!(
        report["changes"]["requests"],
        json!([common::REQUEST, "Also make it quick"]),
        "{report}"
    );
    assert!(
        text(&report["changes"]["spec_changed"]).contains("It is quick."),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_template_is_proven_for_every_input() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving(Protocol::Discover).await;
    let program = checked_template(&session).await;
    program.write("main.s", &asset("main-template.s"));
    let report = done(&session, &program).await;
    // B3's bad inputs come from choice(…), each proven in turn.
    let proofs = report["proofs"].as_array().expect("proofs");
    assert_eq!(proofs.len(), 2, "{report}");
    assert!(
        proofs
            .iter()
            .all(|proof| text(proof).contains(": proven for every input in range")),
        "{report}"
    );
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(lock.contains("proven = [\"B1\", \"B3\"]"), "{lock}");
    assert!(lock.contains("solvers = \"z3 and cvc5\""), "{lock}");
    // Checking the same files again answers at once from what was found.
    let again = session.check(&program).await;
    assert_eq!(again["state"], "done", "{again}");
    assert_eq!(again["proofs"], report["proofs"], "{again}");
    // A new build is tested and proven again by itself.
    program.write(
        "main.s",
        &asset("main-template.s").replacen("_start:\n", "_start:\n    nop\n", 1),
    );
    let report = done(&session, &program).await;
    assert!(
        any_line(&report["proofs"], "proven for every input in range"),
        "{report}"
    );
    let relocked = program.read("lengua.lock").expect("the lock");
    assert_ne!(lock, relocked);
    assert!(relocked.contains("proven = [\"B1\", \"B3\"]"), "{relocked}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn proofs_survive_a_server_restart() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving(Protocol::Discover).await;
    let program = checked_template(&session).await;
    program.write("main.s", &asset("main-template.s"));
    done(&session, &program).await;
    // A new server has no job in memory: it runs the tests again and takes the proofs the lock
    // records about this very build.
    let restarted = session.restart().await;
    let report = done(&restarted, &program).await;
    assert!(
        any_line(&report["proofs"], "as lengua.lock records"),
        "{report}"
    );
    restarted.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_absolute_difference_example_is_proven() {
    // Claude Code wrote this one through lengua; it picks the size of a - b with cmovs, which
    // the prover settles by writing both the program's choice and abs(a - b) as one term.
    if !solvers_installed() {
        return;
    }
    let session = Session::proving(Protocol::Discover).await;
    let program = written(&session, example_files("absdiff")).await;
    let report = done(&session, &program).await;
    // B3 and B4 draw bad arguments with choice(…): each one is proven with every number.
    assert!(
        report["proofs"].as_array().is_some_and(|proofs| proofs
            .iter()
            .all(|proof| text(proof).contains("proven for every input in range"))),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_avx512_shouts_in_the_benchmark_pass_their_checks() {
    // The benchmark's AVX-512 versions of the parallel template, on 16 threads and on one.
    if !has_avx512() {
        return;
    }
    for variant in ["avx512.s", "avx512-one.s"] {
        let session = session().await;
        let code = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("benches/programs/shout")
                .join(variant),
        )
        .expect("an AVX-512 shout");
        let program = written(
            &session,
            [
                ("spec.md", asset("parallel-template-spec.md")),
                ("tests.toml", asset("parallel-template-tests.toml")),
                ("main.s", code),
            ],
        )
        .await;
        done(&session, &program).await;
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn the_avx512_upcase_in_the_benchmark_passes_its_checks() {
    // The benchmark's AVX-512 upcase is a lengua program too: same spec and tests as the
    // example, 64 bytes at a time. Only where the CPU has AVX-512BW.
    if !has_avx512() {
        return;
    }
    let session = session().await;
    let code = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches/programs/upcase/avx512.s"),
    )
    .expect("the AVX-512 upcase");
    let program = written(
        &session,
        [
            ("spec.md", example("upcase", "spec.md")),
            ("tests.toml", example("upcase", "tests.toml")),
            ("main.s", code),
        ],
    )
    .await;
    done(&session, &program).await;
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_file_example_proves_its_file_handling() {
    // upcase's usage and missing-file behaviors draw their names with choice(…); the proof opens,
    // reads and writes files as the test gives them.
    if !solvers_installed() {
        return;
    }
    let session = Session::proving(Protocol::Discover).await;
    let program = written(&session, example_files("upcase")).await;
    let report = done(&session, &program).await;
    // Usage (B2), a missing input (B3) and a file over the limit (B4, a mebibyte of any of its
    // characters) are proven; capitalizing a file (B1), with SSE2, is proven for every file up
    // to the length a proof covers.
    let proofs = &report["proofs"];
    assert!(
        text(&proofs[0])
            .starts_with("generated B1: proven for every input with text up to 4096 bytes"),
        "{report}"
    );
    for index in 1..4 {
        assert!(
            text(&proofs[index]).contains("proven for every input in range"),
            "{report}"
        );
    }
    assert!(
        text(&proofs[3]).contains("(1 input combination, 1 path through"),
        "{report}"
    );
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(
        lock.contains("bounded = [{ id = \"B1\", bytes = 4096 }]"),
        "{lock}"
    );
    session.service.cancel().await.expect("close the session");
}

/// Three minutes: mean's arithmetic isn't proven yet, so three blocks run to the time limit.
#[tokio::test]
#[ignore = "slow: three of mean's blocks run to the proof's time limit"]
async fn the_decimal_example_is_proven() {
    // mean takes decimals and prints exact cents: scaled(a, 2) and places(x, 2) in its tests.
    if !solvers_installed() {
        return;
    }
    let session = Session::proving(Protocol::Discover).await;
    let program = written(&session, example_files("mean")).await;
    let report = done(&session, &program).await;
    // Decimal inputs prove where the output is fixed text: usage (B4), a bad argument (B5) and
    // an argument out of range (B6).
    for index in 3..6 {
        assert!(
            text(&report["proofs"][index]).contains("proven for every input in range"),
            "{report}"
        );
    }
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_dice_example_rolls_with_the_tests_random_bytes() {
    // The tests supply the random bytes, so each roll is exact: 200 runs, plus the proof that
    // every pair of first bytes rolls as the spec says.
    let proving = solvers_installed();
    let session = if proving {
        Session::proving(Protocol::Discover).await
    } else {
        session().await
    };
    let program = written(&session, example_files("dice")).await;
    done(&session, &program).await;
    // A die that's off by one: the tests catch it with the bytes they supply.
    program.write(
        "main.s",
        &example("dice", "main.s").replace("add edx, '1'", "add edx, '0'"),
    );
    let problems = failed(&session, &program).await;
    assert!(
        problems.contains(r#"stdout should be "rolled 1\n", but was "rolled 0\n""#),
        "{problems}"
    );
    program.write("main.s", &example("dice", "main.s"));
    let report = done(&session, &program).await;
    if proving {
        assert!(
            report["proofs"]
                .as_array()
                .is_some_and(|proofs| proofs.iter().all(|proof| text(proof).contains("proven"))),
            "{report}"
        );
    }
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_echo_server_answers_the_tests_client() {
    // A server: lengua listens for it, hands it the socket as file descriptor 3, has the test's
    // client talk to it, and stops it when it keeps serving.
    let session = session().await;
    let program = written(&session, example_files("echo")).await;
    let report = done(&session, &program).await;
    assert!(
        text(&report["example"]).contains("a client that sent"),
        "{report}"
    );

    // run serves in the background on the spec's port until it's stopped.
    let serving = session.call_ok("run", program.arguments()).await;
    assert_eq!(serving["serving"], "127.0.0.1:7007", "{serving}");
    let again = session.call_error("run", program.arguments()).await;
    assert!(again.contains("already serving"), "{again}");
    {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        let mut stream = tokio::net::TcpStream::connect("127.0.0.1:7007")
            .await
            .expect("connect");
        stream.write_all(b"ping\n").await.expect("send");
        let mut answer = Vec::new();
        stream.read_to_end(&mut answer).await.expect("read");
        assert_eq!(answer, b"ping\n");
    }
    let mut stop = program.arguments();
    stop["stop"] = json!(true);
    let stopped = session.call_ok("run", stop.clone()).await;
    assert!(
        text(&stopped["ended"]).contains("stopped by lengua"),
        "{stopped}"
    );
    let nothing = session.call_error("run", stop).await;
    assert!(nothing.contains("no server"), "{nothing}");

    // A server that stops after one connection fails: the spec says it keeps serving.
    program.write(
        "main.s",
        &example("echo", "main.s")
            .replace(
                ".equ SYS_ACCEPT4, 288\n",
                ".equ SYS_ACCEPT4, 288\n.equ SYS_EXIT, 60\n.equ EXIT_DONE, 0\n",
            )
            .replace(
                ".Lstart_next:\n    jmp .Lstart_serve",
                ".Lstart_next:\n    mov eax, SYS_EXIT\n    mov edi, EXIT_DONE\n    syscall",
            ),
    );
    let problems = failed(&session, &program).await;
    assert!(
        problems.contains("while the test expects it to keep serving")
            || problems.contains("wasn't waiting for the next one"),
        "{problems}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_fetch_client_talks_to_the_tests_server() {
    // A client: the test's own server listens on a free port, which the example names as
    // {port}, and the kernel lets the program connect to that port alone.
    let session = session().await;
    let program = written(&session, example_files("fetch")).await;
    done(&session, &program).await;
    // run reaches a real server on a port the spec approves, and no other.
    let refused = session
        .call_ok("run", with_args(&program, &["9", "/"]))
        .await;
    assert_eq!(refused["exit_code"], 3, "{refused}");
    if let Ok(listener) = tokio::net::TcpListener::bind("127.0.0.1:8080").await {
        let server = tokio::spawn(async move {
            use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut request = vec![0_u8; 64];
            let read = stream.read(&mut request).await.expect("read");
            request.truncate(read);
            stream
                .write_all(b"HTTP/1.0 200 OK\r\n\r\nfrom 8080")
                .await
                .expect("reply");
            request
        });
        let fetched = session
            .call_ok("run", with_args(&program, &["8080", "/hi"]))
            .await;
        assert_eq!(fetched["stdout"], "from 8080", "{fetched}");
        assert_eq!(server.await.expect("served"), b"GET /hi HTTP/1.0\r\n\r\n");
    }
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_benchmarks_own_programs_pass_their_checks() {
    // The skill's fast programs, which the benchmark measures on 64 MiB, each with its own spec
    // and tests.
    if !has_avx512() {
        return;
    }
    for workload in ["wc", "crc32c", "histogram"] {
        let session = session().await;
        let source = |name: &str| {
            std::fs::read_to_string(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("plugin/skills/write-assembly/assets/fast")
                    .join(workload)
                    .join(name),
            )
            .unwrap_or_else(|error| panic!("{workload}/{name}: {error}"))
        };
        let program = written(
            &session,
            ["spec.md", "tests.toml", "main.s"].map(|name| (name, source(name))),
        )
        .await;
        done(&session, &program).await;
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn the_word_count_claude_code_wrote_passes_its_checks() {
    // Written through lengua from a plain request in an interactive Claude Code session.
    if !has_avx512() {
        return;
    }
    let session = session().await;
    let program = written(&session, example_files("count")).await;
    done(&session, &program).await;
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn long_text_is_proven_up_to_the_bound() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving(Protocol::Discover).await;
    let program = written(
        &session,
        ["spec.md", "tests.toml", "main.s"].map(|name| (name, fixture("cat", name))),
    )
    .await;
    let report = done(&session, &program).await;
    // Texts run to 5000 bytes, and the proof covers them to 4096: not every input in range.
    let only = text(&report["proofs"][0]);
    assert!(
        only.starts_with("generated B1: proven for every input with text up to 4096 bytes (")
            && only.ends_with(
                "; longer texts are tested, not proven: a proof covers texts up to 4096 bytes"
            ),
        "{only}"
    );
    // The lock records it apart from full proofs.
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(lock.contains("proven = []"), "{lock}");
    assert!(
        lock.contains("bounded = [{ id = \"B1\", bytes = 4096 }]"),
        "{lock}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn programs_on_the_network_are_proven() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving(Protocol::Discover).await;
    // A server: the echo server, as it is.
    let echo = written(&session, example_files("echo")).await;
    let report = done(&session, &echo).await;
    assert!(
        report["proofs"].as_array().is_some_and(|proofs| proofs
            .iter()
            .all(|proof| text(proof).contains("proven for every input in range"))),
        "{report}"
    );
    assert!(
        text(&report["proofs"][0])
            .ends_with("arriving whole or cut after its first byte, at its middle, or both"),
        "{report}"
    );
    // A client: fetch, with short paths and bodies so the proof covers them all.
    let spec = example("fetch", "spec.md").replacen(
        "path holds `1` to `100` characters",
        "path holds `1` to `100` characters (tests: `6` and `11`)",
        1,
    );
    let tests = example("fetch", "tests.toml")
        .replacen("path = 'string(1, 100, ", "path = 'string(1, 6, ", 1)
        .replacen("body = 'string(0, 65000, ", "body = 'string(0, 11, ", 1);
    let fetch = written(
        &session,
        [
            ("spec.md", spec),
            ("tests.toml", tests),
            ("main.s", example("fetch", "main.s")),
        ],
    )
    .await;
    let report = done(&session, &fetch).await;
    assert!(
        report["proofs"].as_array().is_some_and(|proofs| proofs
            .iter()
            .all(|proof| text(proof).contains("proven for every input in range"))),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn an_edit_mid_proof_starts_over_and_the_old_proof_never_lands() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving_waiting(Protocol::Discover, 500).await;
    let program = written(&session, example_files("add")).await;
    // add's proof takes a while: wait until it's running.
    loop {
        let report = session.check(&program).await;
        match report["state"].as_str() {
            Some("proving") => break,
            Some("testing") => {}
            _ => panic!("{report}"),
        }
    }
    program.write(
        "main.s",
        &example("add", "main.s").replacen("_start:\n", "_start:\n    nop\n", 1),
    );
    let report = done(&session, &program).await;
    assert!(
        any_line(&report["proofs"], "proven for every input in range"),
        "{report}"
    );
    let lock = program.read("lengua.lock").expect("the lock");
    let executable = |table: &str| {
        lock.split(&format!("[{table}]"))
            .nth(1)
            .and_then(|rest| rest.split("executable_sha256 = \"").nth(1))
            .and_then(|rest| rest.split('"').next())
            .map(str::to_owned)
    };
    assert!(executable("code").is_some(), "{lock}");
    assert_eq!(executable("code"), executable("proofs"), "{lock}");
    session.service.cancel().await.expect("close the session");
}
