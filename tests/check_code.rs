//! Checking main.s through the server: the rules, building, the tests, the proof, and run.

// Its programs are x86-64 code, which runs only on an x86-64 computer; an ARM64 one runs the
// AArch64 suites (tests/aarch64.rs, tests/aarch64_session.rs) and the ones for any CPU.
#![cfg(target_arch = "x86_64")]

mod common;

use std::path::PathBuf;

use common::{Program, Protocol, Session};
use serde_json::{Value, json};

fn asset(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugin/skills/compile/assets")
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

/// A program folder holding the template's spec, its tests included, recorded.
async fn checked_template(session: &Session) -> Program {
    let program = Program::new();
    program.write("spec.md", &asset("template-spec.md"));
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    program
}

/// A program folder holding both files of `files`.
async fn written(session: &Session, files: [(&str, String); 2]) -> Program {
    let program = Program::new();
    for (name, text) in files {
        program.write(name, &text);
    }
    session.record(&program).await;
    program
}

/// An example's two files.
fn example_files(name: &str) -> [(&'static str, String); 2] {
    ["spec.md", "main.s"].map(|file| (file, example(name, file)))
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
        text(&report["next_step"]).contains("compile skill"),
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
async fn code_waits_for_a_spec_whose_tests_check() {
    let session = session().await;
    let program = Program::new();
    program.write(
        "spec.md",
        &asset("template-spec.md").replace("| 50 |", "| 0 |"),
    );
    program.write("main.s", &asset("main-template.s"));
    let report = session.record(&program).await;
    assert_eq!(report["state"], "spec_problems", "{report}");
    assert!(
        any_line(&report["problems"], "runs must be from 1"),
        "{report}"
    );
    assert!(program.read("lengua.lock").is_none());
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn code_compiled_for_another_computer_is_compiled_again() {
    let session = session().await;
    let program = checked_template(&session).await;
    let elsewhere = asset("main-template.s").replacen(
        ".intel_syntax noprefix\n",
        ".intel_syntax noprefix\n# lengua: compiled from spec.md sha256:00 for macos-arm64\n",
        1,
    );
    program.write("main.s", &elsewhere);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        any_line(&report["problems"], "compiled for macos-arm64"),
        "{report}"
    );
    assert!(
        text(&report["next_step"]).contains("for this computer with the compile skill"),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_vector_template_passes_its_tests() {
    let session = session().await;
    let program = written(
        &session,
        [
            ("spec.md", asset("vector-template-spec.md")),
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

/// The streaming filter goes through check to done with its proof for every length, by
/// induction over its read loop, where a proof by length would reach a few thousand bytes.
#[tokio::test]
async fn the_streaming_filter_is_proven_for_every_length() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving_for(Protocol::Discover, 120).await;
    let program = written(
        &session,
        [
            ("spec.md", asset("stream-template-spec.md")),
            ("main.s", asset("stream-template.s")),
        ],
    )
    .await;
    let report = session.settle(&program).await;
    assert_eq!(report["state"], "done", "{report}");
    let proof = text(&report["proofs"][0]);
    assert!(
        proof.starts_with("generated B1: proven for every input in range")
            && proof.contains("every length by induction over its read loop")
            && proof.contains("one more buffer-full of 4096 bytes"),
        "{report}"
    );
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(lock.contains("proven = [\"B1\"]"), "{lock}");
    let mut arguments = program.arguments();
    arguments["stdin"] = Value::from("stream me\n");
    let run = session.call_ok("run", arguments).await;
    assert_eq!(run["stdout"], "STREAM ME\n", "{run}");
    session.service.cancel().await.expect("close the session");
}

/// The word tally building block goes through check to done, every test passing, and runs.
/// Its proof, whose questions about where words' hashes meet are heavy for the solvers, is the
/// prover's own test's; here it gets a few seconds, so the machine stays free for the others.
#[tokio::test]
async fn the_tally_block_passes_its_checks() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving_for(Protocol::Discover, 5).await;
    let program = written(
        &session,
        [
            ("spec.md", asset("tally-template-spec.md")),
            ("main.s", asset("tally-template.s")),
        ],
    )
    .await;
    let report = session.settle(&program).await;
    assert_eq!(report["state"], "done", "{report}");
    assert!(
        text(&report["proofs"][0]).starts_with("generated B1: "),
        "{report}"
    );
    let mut arguments = program.arguments();
    arguments["stdin"] = Value::from("the cat and the\tdog the cat\n");
    let run = session.call_ok("run", arguments).await;
    assert_eq!(run["stdout"], "3 the\n2 cat\n1 and\n1 dog\n", "{run}");
    session.service.cancel().await.expect("close the session");
}

/// A word count, whose count for each buffer-full depends on whether the one before ended
/// inside a word, goes through check to done with its proof for every length: two buffer-fulls
/// at a time, the second counted after the first's last byte. Its buffer is made small, so the
/// proof's base finishes in the test's time.
#[tokio::test]
async fn a_word_count_is_proven_for_every_length() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving_alone_for(Protocol::Discover, 120).await;
    let program = written(
        &session,
        [
            ("spec.md", fixture("words", "spec.md")),
            (
                "main.s",
                fixture("words", "main.s").replace(".equ BUF_LEN, 4096\n", ".equ BUF_LEN, 128\n"),
            ),
        ],
    )
    .await;
    let report = session.settle(&program).await;
    assert_eq!(report["state"], "done", "{report}");
    let proof = text(&report["proofs"][0]);
    assert!(
        proof.starts_with("generated B1: proven for every input in range")
            && proof.contains("two more buffer-fulls of 128 bytes")
            && proof.contains("after the first's last 1 byte"),
        "{report}"
    );
    let mut arguments = program.arguments();
    arguments["stdin"] = Value::from("one two\nthree\n");
    let run = session.call_ok("run", arguments).await;
    assert_eq!(run["stdout"], "2 3\n", "{run}");
    session.service.cancel().await.expect("close the session");
}

/// A count of a part that overlaps itself, `aa`, goes through check to done proven for every
/// length: the program keeps, across each buffer-full, whether its last byte is an `a` that
/// starts no pair yet, and the proof shows every time round keeps that so.
#[tokio::test]
async fn a_count_of_a_part_that_overlaps_itself_is_proven_for_every_length() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving_alone_for(Protocol::Discover, 120).await;
    let files = ["spec.md", "main.s"].map(|file| (file, fixture("pairs", file)));
    let program = written(&session, files).await;
    let report = session.settle(&program).await;
    assert_eq!(report["state"], "done", "{report}");
    let proof = text(&report["proofs"][0]);
    assert!(
        proof.starts_with("generated B1: proven for every input in range")
            && proof.contains("one more buffer-full of 256 bytes"),
        "{report}"
    );
    let mut arguments = program.arguments();
    arguments["stdin"] = Value::from("aaa baaaa\naa");
    let run = session.call_ok("run", arguments).await;
    assert_eq!(run["stdout"], "4\n", "{run}");
    session.service.cancel().await.expect("close the session");
}

/// A `switch` on an operator, as a checked jump table (dialect 4): it follows every rule, passes
/// its tests, and each case runs.
#[tokio::test]
async fn a_checked_jump_table_passes_its_checks_and_runs_each_case() {
    let session = session().await;
    let files = ["spec.md", "main.s"].map(|file| (file, fixture("calc", file)));
    let program = written(&session, files).await;
    let report = done(&session, &program).await;
    assert!(
        text(&report["tests"][0]).starts_with("main.s follows every rule and passed all"),
        "{report}"
    );
    for (operator, printed) in [("+", "9\n"), ("-", "3\n"), ("*", "18\n")] {
        let run = session
            .call_ok("run", with_args(&program, &["6", operator, "3"]))
            .await;
        assert_eq!(run["stdout"], printed, "{run}");
        assert_eq!(run["exit_code"], 0, "{run}");
    }
    // `,` is in the table's range, as its unknown case.
    for operator in [",", "/", ""] {
        let run = session
            .call_ok("run", with_args(&program, &["6", operator, "3"]))
            .await;
        assert_eq!(run["exit_code"], 3, "{operator:?}: {run}");
    }
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn small_ranges_run_every_value() {
    let session = session().await;
    let purpose = asset("template-spec.md").replace(
        "between `-9223372036854775807`\nand `9223372036854775807`",
        "between `-100`\nand `100`",
    );
    assert_ne!(purpose, asset("template-spec.md"), "the range was narrowed");
    let spec = purpose.replace(
        "int(-9223372036854775807, 9223372036854775807)",
        "int(-100, 100)",
    );
    assert_ne!(spec, purpose, "the tests draw from it");
    let program = written(
        &session,
        [("spec.md", spec), ("main.s", asset("main-template.s"))],
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
    let spec = asset("template-spec.md");

    // A fix after code ran is checked like any version of the spec, and shown with lengua's
    // own marks on what tests less; nobody has to approve anything.
    program.write("spec.md", &spec.replace("| 200 |", "| 10 |"));
    let report = done(&session, &program).await;
    let changed = text(&report["changes"]["spec_changed"]);
    assert!(changed.contains("- ") && changed.contains("+ "), "{report}");
    assert!(changed.contains("| 10 |"), "{report}");
    assert_eq!(
        report["changes"]["tests_less"],
        json!(["generated B1: fewer runs, 10 instead of 200"]),
        "{report}"
    );

    // A change that tests more gets no mark, and going back to the first version shows nothing.
    program.write("spec.md", &spec.replace("| 200 |", "| 300 |"));
    let report = done(&session, &program).await;
    assert!(report["changes"].get("tests_less").is_none(), "{report}");
    assert!(report["changes"]["spec_changed"].is_string(), "{report}");
    program.write("spec.md", &spec);
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
    // Every step settles by the terms alone: the program's parse and the formula's number are
    // one sum.
    assert!(
        lock.contains(
            "solvers = \"none: the tests tried every value, or the terms settled every step\""
        ),
        "{lock}"
    );
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
async fn a_bug_the_samples_miss_is_found_by_the_proof_and_run_for_real() {
    // The negative of 1234567 comes out one too big: one input in 18 quintillion, which the
    // tests' samples miss and the proof finds; a real run with that input confirms it. Finding
    // it takes the solvers most of their time for a question, so no other proof runs beside it.
    if !solvers_installed() {
        return;
    }
    let session = Session::proving_alone(Protocol::Discover).await;
    let program = checked_template(&session).await;
    let buggy = asset("main-template.s")
        .replacen(
            ".equ OUT_BUF_LEN, 21\n",
            ".equ OUT_BUF_LEN, 21\n.equ UNLUCKY, -1234567\n",
            1,
        )
        .replacen(
            "    neg rdx\n    jo .Lstart_assert_negate\n",
            "    neg rdx\n    jo .Lstart_assert_negate\n    cmp rdx, UNLUCKY\n    jne \
             .Lstart_lucky\n    inc rdx\n.Lstart_lucky:\n",
            1,
        );
    assert_ne!(buggy, asset("main-template.s"));
    program.write("main.s", &buggy);
    let problems = failed(&session, &program).await;
    assert!(
        problems.contains(
            "the proof found inputs the tests didn't try where main.s is wrong: with n = 1234567"
        ),
        "{problems}"
    );
    // The tests passed, but a build the proof found wrong doesn't run.
    let refused = session
        .call_error("run", with_args(&program, &["1234567"]))
        .await;
    assert!(
        refused.contains("The proof found inputs where main.s is wrong"),
        "{refused}"
    );
    // Nor after lengua restarts: lengua.lock records what the proof found, and check shows it.
    let restarted = session.restart().await;
    let refused = restarted
        .call_error("run", with_args(&program, &["1234567"]))
        .await;
    assert!(
        refused.contains("The proof found inputs where main.s is wrong")
            && refused.contains("n = 1234567"),
        "{refused}"
    );
    let problems = failed(&restarted, &program).await;
    assert!(problems.contains("with n = 1234567"), "{problems}");
    restarted.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn done_comes_once_the_tests_pass_while_the_proofs_go_on() {
    if !solvers_installed() {
        return;
    }
    // add's proof takes most of a minute, so done comes first, with the proofs still running.
    let session = Session::proving_with_grace(Protocol::Discover, 0).await;
    let program = Program::new();
    for (name, text) in example_files("add") {
        program.write(name, &text);
    }
    let report = session.record(&program).await;
    assert_eq!(report["state"], "done", "{report}");
    assert!(any_line(&report["proofs"], ": proving"), "{report}");
    assert!(
        text(&report["next_step"]).contains("still running"),
        "{report}"
    );
    // The program runs while its proofs go on.
    let run = session
        .call_ok("run", with_args(&program, &["2", "3"]))
        .await;
    assert_eq!(run["stdout"], "5\n", "{run}");
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
async fn a_program_makes_folders_and_moves_files_inside_its_own() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving(Protocol::Discover).await;
    let program = written(&session, example_files("archive")).await;
    let report = done(&session, &program).await;
    assert_eq!(
        report["tests"][0], "main.s follows every rule and passed all 404 test runs",
        "{report}"
    );
    // The proof follows the folder it makes and the file it moves, for every text in range.
    assert!(
        text(&report["proofs"][0]).contains("proven for every input"),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_program_lists_its_folder_and_deletes_inside_it() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving(Protocol::Discover).await;
    let files = ["spec.md", "main.s"].map(|file| (file, fixture("sweep", file)));
    let program = written(&session, files).await;
    let report = done(&session, &program).await;
    assert_eq!(
        report["tests"][0], "main.s follows every rule and passed all 202 test runs",
        "{report}"
    );
    // The kernel lists a folder in an order of its own, which the proof doesn't take.
    assert!(
        text(&report["proofs"][0]).contains("tested, not proven: listing a folder"),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_test_answers_a_sleep_at_once() {
    // nap sleeps 30 seconds, more than a test may run: the test's supervisor ends the sleep at
    // once, so the test passes in a moment.
    let session = session().await;
    let files = ["spec.md", "main.s"].map(|file| (file, fixture("nap", file)));
    let program = written(&session, files).await;
    let started = std::time::Instant::now();
    let report = done(&session, &program).await;
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_program_gets_only_the_environment_its_test_or_run_gives() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving(Protocol::Discover).await;
    let program = written(&session, example_files("greet")).await;
    let report = done(&session, &program).await;
    assert_eq!(
        report["tests"][0], "main.s follows every rule and passed all 303 test runs",
        "{report}"
    );
    // Each name the proof can draw is greeted, and every run kept to the memory rules.
    assert!(
        text(&report["proofs"][0]).contains("proven for every input in range"),
        "{report}"
    );
    let mut arguments = program.arguments();
    arguments["env"] = json!({"NAME": "Grace Hopper"});
    let run = session.call_ok("run", arguments).await;
    assert_eq!(run["stdout"], "Hello, Grace Hopper!\n", "{run}");
    // Nothing else reaches it: without NAME it shows how to use it.
    let run = session.call_ok("run", program.arguments()).await;
    assert_eq!(run["stderr"], "usage: NAME=<who> greet\n", "{run}");
    session.service.cancel().await.expect("close the session");
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
    // Memory too, on every run those proofs cover.
    assert!(
        text(&report["memory"]).contains("on every run the proofs of B1, B2, B3 and B4 cover"),
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
        [("spec.md", example("upcase", "spec.md")), ("main.s", code)],
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
    // Two minutes, so a machine busy with other tests' proofs still proves some lengths: the
    // model's runs of the samples, mebibyte files among them, come out of the same time.
    let session = Session::proving_for(Protocol::Discover, 120).await;
    let program = written(&session, example_files("upcase")).await;
    let report = done(&session, &program).await;
    // Usage (B2), a missing input (B3) and a file over the limit (B4, a mebibyte of any of its
    // characters) are proven; capitalizing a file (B1), with SSE2, is proven for every file up
    // to the length the proof's time reaches, of the mebibyte the spec allows.
    let proofs = &report["proofs"];
    let first = text(&proofs[0]);
    assert!(
        first.starts_with("generated B1: proven for every input with text up to ")
            && first.contains(" of 1048576 bytes (")
            && first.contains("; check with prove_further to prove on from there"),
        "{report}"
    );
    let reached = |line: &str| -> usize {
        line["generated B1: proven for every input with text up to ".len()..]
            .split(' ')
            .next()
            .and_then(|bytes| bytes.parse().ok())
            .expect("a length")
    };
    let before = reached(first);
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
        lock.contains(&format!(
            "bounded = [{{ id = \"B1\", bytes = {before}, of = 1048576, more = true }}]"
        )),
        "{lock}"
    );
    // Proving further goes on from there, and the lock keeps the rest.
    let mut arguments = program.arguments();
    arguments["prove_further"] = serde_json::json!(true);
    let report = session.settle_with(&program, arguments).await;
    assert_eq!(report["state"], "done", "{report}");
    let after = reached(text(&report["proofs"][0]));
    assert!(after > before, "{before} then {after}: {report}");
    for index in 1..4 {
        assert!(
            text(&report["proofs"][index]).contains("proven for every input in range"),
            "{report}"
        );
    }
    session.service.cancel().await.expect("close the session");
}

/// About three minutes with 16 cores: mean's two blocks of decimal means take over a minute each.
#[tokio::test]
#[ignore = "slow: the proof takes minutes"]
async fn the_decimal_example_is_proven() {
    // mean takes decimals and prints exact cents: scaled(a, 2) and places(x, 2) in its tests.
    if !solvers_installed() {
        return;
    }
    let session = Session::proving(Protocol::Discover).await;
    let program = written(&session, example_files("mean")).await;
    let report = done(&session, &program).await;
    // Every block: the decimal means, the whole ones, and the three errors.
    for index in 0..6 {
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
    // A client: lengua makes its connection before it starts, to the test's own server in its
    // tests and to this computer's port 8080 in a run, and hands it over as descriptor 4.
    let session = session().await;
    let program = written(&session, example_files("fetch")).await;
    done(&session, &program).await;
    // Only where nothing else holds port 8080 here.
    if std::net::TcpListener::bind("127.0.0.1:8080").is_err() {
        eprintln!("skipped the runs: something on this computer holds port 8080");
        session.service.cancel().await.expect("close the session");
        return;
    }
    // With nothing there, the program gets its connection already ended, and the report says
    // why.
    let refused = session.call_ok("run", with_args(&program, &["/"])).await;
    assert_eq!(refused["exit_code"], 3, "{refused}");
    assert_eq!(
        refused["connections"][0]["place"], "port 8080 of this computer",
        "{refused}"
    );
    assert!(
        refused["connections"][0]["problem"].is_string(),
        "{refused}"
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .expect("port 8080");
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut request = Vec::new();
        let mut chunk = [0_u8; 64];
        while !request.ends_with(b"\r\n\r\n") {
            let read = stream.read(&mut chunk).await.expect("read");
            assert!(read > 0, "the request ended early: {request:?}");
            request.extend_from_slice(&chunk[..read]);
        }
        stream
            .write_all(b"HTTP/1.0 200 OK\r\n\r\nfrom 8080")
            .await
            .expect("reply");
        request
    });
    let fetched = session.call_ok("run", with_args(&program, &["/hi"])).await;
    assert_eq!(fetched["stdout"], "from 8080", "{fetched}");
    assert!(fetched["connections"][0]["problem"].is_null(), "{fetched}");
    assert_eq!(server.await.expect("served"), b"GET /hi HTTP/1.0\r\n\r\n");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_client_over_tls_is_tested_in_plain_bytes() {
    // fetch, pointed at a named computer over TLS: lengua would do the TLS in a run, so its
    // tests' stand-in server speaks the plain bytes the program sends and reads.
    let session = session().await;
    let spec = example("fetch", "spec.md")
        .replace(
            "- Network: connects to TCP port 8080",
            "- Network: connects to TCP port 443 on example.com over TLS",
        )
        .replace("on port `8080`", "on port `443`");
    let program = written(
        &session,
        [("spec.md", spec), ("main.s", example("fetch", "main.s"))],
    )
    .await;
    done(&session, &program).await;
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
                    .join("plugin/skills/compile/assets/fast")
                    .join(workload)
                    .join(name),
            )
            .unwrap_or_else(|error| panic!("{workload}/{name}: {error}"))
        };
        let program = written(
            &session,
            ["spec.md", "main.s"].map(|name| (name, source(name))),
        )
        .await;
        done(&session, &program).await;
        session.service.cancel().await.expect("close the session");
    }
}

/// Programs stack like nodes of a graph: one spec links to another it builds on, and its main.s
/// takes the part's checked functions word for word, which check holds to the part's current
/// code.
#[tokio::test]
async fn a_program_builds_on_another() {
    let session = session().await;
    let parent = tempfile::tempdir().expect("a folder for both");
    let part = Program::new_in(parent.path(), "sort");
    let node = Program::new_in(parent.path(), "sorted");
    for (name, text) in example_files("sort") {
        part.write(name, &text);
    }
    let link = format!("../{}", part.name());
    node.write(
        "spec.md",
        &example("sort", "spec.md")
            .replace("title: sort", "title: sorted")
            .replace(
                "- Uses: standard input",
                &format!("- Uses: standard input\n- Builds on: [sort]({link}/spec.md)"),
            ),
    );
    let code = example("sort", "main.s");
    node.write("main.s", &code);
    let problems = |report: &Value| report["problems"].to_string();
    // A part it builds on but takes nothing from.
    let report = session.record(&node).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        problems(&report).contains("takes nothing from it"),
        "{report}"
    );
    // A part that isn't checked yet.
    let taken = code.replace(
        ".type merge_sort, @function",
        &format!("# from: {link}\n.type merge_sort, @function"),
    );
    node.write("main.s", &taken);
    let report = session.check(&node).await;
    assert!(
        problems(&report).contains("isn't checked on this computer"),
        "{report}"
    );
    // Once the part is done, so is the program built on it.
    session.record(&part).await;
    done(&session, &part).await;
    done(&session, &node).await;
    // A copy that drifts from the part's code.
    node.write(
        "main.s",
        &taken.replace(".Lmerge_sort_done", ".Lmerge_sort_finished"),
    );
    let report = session.check(&node).await;
    assert!(
        problems(&report).contains("merge_sort differs from")
            && problems(&report).contains("copy it again"),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_sort_claude_code_wrote_passes_its_checks() {
    // Sorting lines as LC_ALL=C sort does, with -u: written through lengua from one plain
    // request in an interactive Claude Code session, with sort(s) and uniq(s) in its tests.
    let session = session().await;
    let program = written(&session, example_files("sort")).await;
    done(&session, &program).await;
    session.service.cancel().await.expect("close the session");
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
        ["spec.md", "main.s"].map(|name| (name, fixture("cat", name))),
    )
    .await;
    let report = done(&session, &program).await;
    // Texts run to 5000 bytes, read with read, and a pipe write holds 4096: proving further
    // can't help, so the line doesn't offer it.
    let only = text(&report["proofs"][0]);
    assert!(
        only.starts_with(
            "generated B1: proven for every input with text up to 4096 of 5000 bytes ("
        ) && only.ends_with(
            "; longer texts are tested, not proven: standard input past 4096 bytes is more \
             than one pipe write, which the proof reads whole; read it with fill to be proven \
             for any length"
        ),
        "{only}"
    );
    // The lock records it apart from full proofs.
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(lock.contains("proven = []"), "{lock}");
    assert!(
        lock.contains("bounded = [{ id = \"B1\", bytes = 4096, of = 5000, more = false }]"),
        "{lock}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_cut_only_the_proof_tries_is_found_and_run_for_real() {
    // second reads both lines with one fill, which stops at the first newline a read ends
    // with: right whenever both lines arrive together, as they do in the tests. The proof takes
    // every cut and finds the one after the first line, and a real run with standard input cut
    // there confirms it.
    if !solvers_installed() {
        return;
    }
    let session = Session::proving(Protocol::Discover).await;
    let files = ["spec.md", "main.s"].map(|file| (file, fixture("second", file)));
    let program = written(&session, files).await;
    let problems = failed(&session, &program).await;
    assert!(
        problems.contains("the proof found inputs the tests didn't try where main.s is wrong"),
        "{problems}"
    );
    assert!(
        problems.contains("standard input arriving in two pieces, cut after byte"),
        "{problems}"
    );
    // Reading to the end instead: proven however standard input arrives.
    let whole = fixture("second", "main.s").replacen(
        "    mov ecx, NEWLINE\n    call fill\n",
        "    mov ecx, -1\n    call fill\n",
        1,
    );
    program.write("main.s", &whole);
    let report = done(&session, &program).await;
    assert!(
        text(&report["proofs"][0]).ends_with("standard input arriving cut into pieces any way"),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_raw_read_of_short_input_is_proven_for_every_count() {
    // second with one read of its own in place of fill: right only when both lines arrive in
    // one read. With at most 8 bytes left, the proof takes every count a read can return,
    // finds a short one, and a real run with standard input cut there confirms it.
    if !solvers_installed() {
        return;
    }
    let session = Session::proving(Protocol::Discover).await;
    let raw = fixture("second", "main.s").replacen(
        "    mov ecx, NEWLINE\n    call fill\n",
        "    mov eax, SYS_READ\n    syscall\n",
        1,
    );
    assert_ne!(raw, fixture("second", "main.s"));
    let program = written(
        &session,
        [("spec.md", fixture("second", "spec.md")), ("main.s", raw)],
    )
    .await;
    let problems = failed(&session, &program).await;
    assert!(
        problems.contains("the proof found inputs the tests didn't try where main.s is wrong"),
        "{problems}"
    );
    assert!(
        problems.contains("standard input arriving in pieces, cut after byte"),
        "{problems}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_server_that_waits_for_its_client_to_close_fails_as_with_curl() {
    // curl and browsers keep their side of the connection open until they have the answer: a
    // server that reads to the end of the request before answering waits on a client that
    // waits for it, and the test's client does as they do.
    let session = session().await;
    let waits = example("echo", "main.s").replacen(
        "    mov ecx, NEWLINE\n    call fill\n",
        "    mov ecx, -1\n    call fill\n",
        1,
    );
    assert_ne!(waits, example("echo", "main.s"));
    let program = written(
        &session,
        [("spec.md", example("echo", "spec.md")), ("main.s", waits)],
    )
    .await;
    let problems = failed(&session, &program).await;
    assert!(
        problems.contains("curl and browsers keep it open until they have the whole answer"),
        "{problems}"
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
    // It reads its line with fill, so the proof covers every way the line can be cut.
    assert!(
        text(&report["proofs"][0]).ends_with("arriving cut into pieces any way"),
        "{report}"
    );
    // A client: fetch, with short paths and bodies so the proof covers them all.
    let spec = example("fetch", "spec.md")
        .replacen(
            "path holds `1` to `100` characters",
            "path holds `1` to `100` characters (tests: `6` and `11`)",
            1,
        )
        .replacen("`string(1, 100, ", "`string(1, 6, ", 1)
        .replacen("`string(0, 65000, ", "`string(0, 11, ", 1);
    let fetch = written(
        &session,
        [("spec.md", spec), ("main.s", example("fetch", "main.s"))],
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
    // Each check waits 50 ms at most, so it sees the proof running: cat's, of every text up to
    // 4096 bytes, takes over a second even on an idle computer, where add's now takes less
    // than one check's wait.
    let session = Session::proving_waiting(Protocol::Discover, 50).await;
    let program = written(
        &session,
        ["spec.md", "main.s"].map(|name| (name, fixture("cat", name))),
    )
    .await;
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
        &fixture("cat", "main.s").replacen("_start:\n", "_start:\n    nop\n", 1),
    );
    let report = done(&session, &program).await;
    assert!(
        any_line(&report["proofs"], "proven for every input"),
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
