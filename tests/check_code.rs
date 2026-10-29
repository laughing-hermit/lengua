//! Checking main.s through the server: preconditions, the spec label, building, and the rules.

mod common;

use std::path::PathBuf;

use common::{Program, Protocol, Session};
use serde_json::Value;

fn asset(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugin/skills/write-assembly/assets")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// A program folder holding the template's spec (recorded) and tests (checked).
async fn checked_template(session: &Session) -> Program {
    let program = Program::new();
    program.write("spec.md", &asset("template-spec.md"));
    let record = session.record(&program).await;
    assert_eq!(record["recorded"], true, "{record}");
    program.write("tests.toml", &asset("template-tests.toml"));
    let tests = session.call_ok("check_tests", program.arguments()).await;
    assert_eq!(tests["checked"], true, "{tests}");
    program
}

async fn session() -> Session {
    Session::start(Protocol::Discover).await
}

#[tokio::test]
async fn the_template_passes_its_tests_and_is_stamped() {
    let session = session().await;
    let program = checked_template(&session).await;
    program.write("main.s", &asset("main-template.s"));
    let report: Value = session.call_ok("check_code", program.arguments()).await;
    assert_eq!(report["rules_passed"], true, "{report}");
    assert_eq!(report["tests_passed"], true, "{report}");
    assert_eq!(report["stamped"], true, "{report}");
    assert_eq!(report["functions"], 4, "{report}");
    assert_eq!(report["assertions"], 11, "{report}");
    assert_eq!(report["test_runs"], 3 + 200 + 50, "{report}");
    assert!(
        report["max_stack_bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes >= 48),
        "{report}"
    );
    assert!(
        report["fingerprint"]
            .as_str()
            .is_some_and(|hex| hex.len() == 64),
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
    assert_eq!(costs[0]["test"], "example B1", "{report}");
    assert_eq!(costs[3]["runs"], 200, "{report}");
    for measure in ["instructions", "cycles", "cpu_time_us"] {
        let spread = &costs[3][measure];
        if !spread.is_null() {
            assert!(
                spread["min"].as_u64() <= spread["median"].as_u64(),
                "{report}"
            );
            assert!(
                spread["median"].as_u64() <= spread["max"].as_u64(),
                "{report}"
            );
        }
    }
    assert!(
        report["next_step"]
            .as_str()
            .is_some_and(|step| step.starts_with("Ready")),
        "{report}"
    );
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(
        lock.contains("[code]") && lock.contains("executable_sha256 = "),
        "{lock}"
    );
    assert!(lock.contains("toolchain = \"llvm-mc "), "{lock}");
    let status = session.call_ok("status", program.arguments()).await;
    assert_eq!(status["program"]["state"], "ready", "{status}");
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
    let error = session.call_error("check_code", program.arguments()).await;
    assert!(
        error.contains("follows every rule but fails 1 test"),
        "{error}"
    );
    assert!(error.contains("generated B1"), "{error}");
    assert!(error.contains("(seed "), "{error}");
    assert!(error.contains("but was \"-7\\n\""), "{error}");
    assert!(
        !program
            .read("lengua.lock")
            .expect("the lock")
            .contains("[code]")
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn failed_assertions_name_their_line() {
    let session = session().await;
    let program = checked_template(&session).await;
    // The bounds check now trips on the second character.
    let code = asset("main-template.s").replacen("    cmp rcx, MAX_CHARS\n", "    cmp rcx, 1\n", 1);
    program.write("main.s", &code);
    let error = session.call_error("check_code", program.arguments()).await;
    let line = code
        .lines()
        .position(|line| line == "    mov edi, 4")
        .expect("assertion 4")
        + 1;
    assert!(
        error.contains(&format!("assertion 4 failed at main.s:{line}")),
        "{error}"
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
    session.call_ok("check_code", program.arguments()).await;

    let arguments = |args: &[&str]| {
        let mut value = program.arguments();
        value["args"] = serde_json::json!(args);
        value
    };
    let report = session.call_ok("run", arguments(&["7"])).await;
    assert_eq!(report["stdout"], "-7\n", "{report}");
    assert_eq!(report["exit_code"], 0, "{report}");
    assert_eq!(report["ended"], "exit code 0", "{report}");
    let report = session.call_ok("run", arguments(&[])).await;
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
    lengua::lock::write(program.path(), &lock, &key).expect("write the lock");
    let error = session.call_error("run", arguments(&["7"])).await;
    assert!(
        error.contains("differs from the one check_code tested"),
        "{error}"
    );
    assert!(error.contains("built with llvm-mc 21.0.0"), "{error}");
    session.call_ok("check_code", program.arguments()).await;

    // Any edit stops run until it's checked or recorded again.
    program.write("main.s", &format!("{code}\n"));
    let error = session.call_error("run", arguments(&["7"])).await;
    assert!(error.contains("call check_code"), "{error}");
    program.write("main.s", &code);
    let lock_text = program.read("lengua.lock").expect("the lock");
    program.write(
        "lengua.lock",
        &lock_text.replace("max_stack_bytes = ", "max_stack_bytes = 1"),
    );
    let error = session.call_error("run", arguments(&["7"])).await;
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
    let error = session.call_error("check_code", program.arguments()).await;
    let line = code
        .lines()
        .position(|line| line == "    and rsp, -16")
        .expect("the inserted line")
        + 1;
    assert!(error.contains("so nothing ran"), "{error}");
    assert!(
        error.contains(&format!(
            "rule 3 (fixed resources) main.s:{line}: `and` changes rsp"
        )),
        "{error}"
    );
    assert!(
        error.contains("Fix them with the write-assembly skill"),
        "{error}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn main_s_carries_the_current_spec_label_and_builds() {
    let session = session().await;
    let program = checked_template(&session).await;
    let code = asset("main-template.s");
    let wrong_label = format!(
        "# lengua: spec sha256:{}\n{}",
        "0".repeat(64),
        code.split_once('\n').map_or("", |(_, rest)| rest)
    );
    program.write("main.s", &wrong_label);
    let error = session.call_error("check_code", program.arguments()).await;
    assert!(
        error.contains("first line must be `# lengua: spec sha256:"),
        "{error}"
    );

    program.write(
        "main.s",
        &code.replacen("    xor r9d, r9d\n", "    mov r9d, rbx\n", 1),
    );
    let error = session.call_error("check_code", program.arguments()).await;
    assert!(
        error.contains("doesn't build: assembling failed"),
        "{error}"
    );
    assert!(
        error.contains("main.s:"),
        "the assembler's message names the line: {error}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn code_needs_checked_tests_first() {
    let session = session().await;
    let program = Program::new();
    program.write("spec.md", &asset("template-spec.md"));
    session.record(&program).await;
    program.write("main.s", &asset("main-template.s"));
    let error = session.call_error("check_code", program.arguments()).await;
    assert!(error.contains("recorded spec and checked tests"), "{error}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_vector_template_passes_its_tests() {
    let session = session().await;
    let program = Program::new();
    program.write("spec.md", &asset("vector-template-spec.md"));
    session.record(&program).await;
    program.write("tests.toml", &asset("vector-template-tests.toml"));
    let tests = session.call_ok("check_tests", program.arguments()).await;
    assert_eq!(tests["checked"], true, "{tests}");
    program.write("main.s", &asset("vector-template.s"));
    let report = session.call_ok("check_code", program.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    assert_eq!(report["test_runs"], 3 + 200 + 50, "{report}");

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
    let program = Program::new();
    let spec = asset("template-spec.md").replace(
        "between `-9223372036854775807`\nand `9223372036854775807`",
        "between `-100`\nand `100`",
    );
    assert_ne!(spec, asset("template-spec.md"), "the range was narrowed");
    let old_label = common::label(&asset("template-spec.md"));
    let new_label = common::label(&spec);
    program.write("spec.md", &spec);
    session.record(&program).await;
    program.write(
        "tests.toml",
        &asset("template-tests.toml")
            .replace(&old_label, &new_label)
            .replace(
                "int(-9223372036854775807, 9223372036854775807)",
                "int(-100, 100)",
            ),
    );
    let tests = session.call_ok("check_tests", program.arguments()).await;
    assert_eq!(tests["generated_runs"], 201 + 50, "{tests}");
    program.write(
        "main.s",
        &asset("main-template.s").replace(&old_label, &new_label),
    );
    let report = session.call_ok("check_code", program.arguments()).await;
    assert_eq!(report["test_runs"], 3 + 201 + 50, "{report}");
    assert_eq!(
        report["every_value"],
        serde_json::json!(["generated B1: every value of n in its range (all 201)"]),
        "{report}"
    );
    assert!(
        report["message"].as_str().is_some_and(|message| message
            .contains("Generated B1 tried every value of n in its range (all 201).")),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
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

#[tokio::test]
async fn the_decimal_examples_pass_their_tests() {
    // mean computes in exact cents; hypotenuse in doubles, with sqrtsd.
    for (program, args, stdout) in [
        ("mean", ["2.5", "3.76"], "3.13\n"),
        ("hypotenuse", ["3", "4"], "5.000\n"),
    ] {
        let session = session().await;
        let folder = Program::new();
        for name in ["spec.md", "tests.toml", "main.s"] {
            folder.write(name, &example(program, name));
        }
        session.record(&folder).await;
        let tests = session.call_ok("check_tests", folder.arguments()).await;
        assert_eq!(tests["checked"], true, "{program}: {tests}");
        let report = session.call_ok("check_code", folder.arguments()).await;
        assert_eq!(report["stamped"], true, "{program}: {report}");
        let mut arguments = folder.arguments();
        arguments["args"] = serde_json::json!(args);
        let run = session.call_ok("run", arguments).await;
        assert_eq!(run["stdout"], stdout, "{program}: {run}");
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn the_file_example_reads_and_writes_files() {
    // upcase reads one file and writes another; its tests give files and check the files left.
    let session = session().await;
    let folder = Program::new();
    for name in ["spec.md", "tests.toml", "main.s"] {
        folder.write(name, &example("upcase", name));
    }
    session.record(&folder).await;
    let tests = session.call_ok("check_tests", folder.arguments()).await;
    assert_eq!(tests["checked"], true, "{tests}");
    let report = session.call_ok("check_code", folder.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");

    // Run on a folder of the user's: the program works on the files there, and only there.
    let user = tempfile::tempdir().expect("a user folder");
    std::fs::write(user.path().join("notes.txt"), "Hello, files!\n").expect("a user file");
    let mut arguments = folder.arguments();
    arguments["args"] = serde_json::json!(["notes.txt", "loud.txt"]);
    arguments["folder"] = serde_json::json!(user.path());
    let run = session.call_ok("run", arguments.clone()).await;
    assert_eq!(run["exit_code"], 0, "{run}");
    assert_eq!(
        std::fs::read_to_string(user.path().join("loud.txt")).expect("written"),
        "HELLO, FILES!\n"
    );
    arguments["args"] = serde_json::json!(["/etc/hostname", "loud.txt"]);
    let outside = session.call_ok("run", arguments.clone()).await;
    assert_eq!(
        outside["exit_code"], 2,
        "files outside the folder can't be opened: {outside}"
    );
    arguments["folder"] = serde_json::json!("relative/folder");
    let error = session.call_error("run", arguments).await;
    assert!(error.contains("folder must be an absolute path"), "{error}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_parallel_template_runs_on_several_cores() {
    let session = session().await;
    let program = Program::new();
    program.write("spec.md", &asset("parallel-template-spec.md"));
    session.record(&program).await;
    program.write("tests.toml", &asset("parallel-template-tests.toml"));
    let tests = session.call_ok("check_tests", program.arguments()).await;
    assert_eq!(tests["checked"], true, "{tests}");
    program.write("main.s", &asset("parallel-template.s"));
    let report = session.call_ok("check_code", program.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    let costs = report["performance"].as_array().expect("performance");
    assert!(
        costs
            .iter()
            .all(|cost| cost["wall_time_us"]["median"].is_u64()),
        "{report}"
    );
    // A megabyte comes back capitalized: sixteen threads each did their part.
    let text: String = "Hello, threads! ".repeat(65536);
    let mut arguments = program.arguments();
    arguments["stdin"] = serde_json::json!(text);
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
    let report = session.call_ok("check_code", program.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    assert!(report.get("tests_changed").is_none(), "{report}");
    let tests = asset("template-tests.toml");

    // A fix after code ran is checked against the spec like any version, and shown with lengua's
    // own marks on what tests less; nobody has to approve anything.
    program.write("tests.toml", &tests.replace("runs = 200", "runs = 10"));
    let status = session.call_ok("status", program.arguments()).await;
    assert_eq!(status["program"]["state"], "tests_unchecked", "{status}");
    let report = session.call_ok("check_tests", program.arguments()).await;
    assert_eq!(report["checked"], true, "{report}");
    let changed = report["tests_changed"].as_str().unwrap_or_default();
    assert!(changed.contains("- ") && changed.contains("+ "), "{report}");
    assert!(changed.contains("runs = 10"), "{report}");
    assert_eq!(
        report["tests_less"][0], "generated B1: fewer runs, 10 instead of 200",
        "{report}"
    );
    assert!(
        report["message"]
            .as_str()
            .is_some_and(|message| message.contains("changed after code ran")),
        "{report}"
    );
    let report = session.call_ok("check_code", program.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    assert_eq!(
        report["tests_less"].as_array().map(Vec::len),
        Some(1),
        "{report}"
    );
    let status = session.call_ok("status", program.arguments()).await;
    assert_eq!(
        status["program"]["tests_less"], report["tests_less"],
        "{status}"
    );

    // A change that tests more gets no mark, and going back to the first version shows nothing.
    program.write("tests.toml", &tests.replace("runs = 200", "runs = 300"));
    let report = session.call_ok("check_tests", program.arguments()).await;
    assert!(report.get("tests_less").is_none(), "{report}");
    assert!(report["tests_changed"].is_string(), "{report}");
    program.write("tests.toml", &tests);
    let report = session.call_ok("check_tests", program.arguments()).await;
    assert!(report.get("tests_changed").is_none(), "{report}");
    let report = session.call_ok("check_code", program.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
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
    let record = session
        .call_ok("record_spec", program.request("Also make it quick"))
        .await;
    assert_eq!(
        record["requests"],
        serde_json::json!([common::REQUEST, "Also make it quick"]),
        "{record}"
    );
    assert!(
        record["spec_changed"]
            .as_str()
            .is_some_and(|diff| diff.contains("It is quick.")),
        "{record}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_template_is_proven_for_every_input() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("z3 isn't installed");
        return;
    }
    let session = session().await;
    let program = checked_template(&session).await;
    program.write("main.s", &asset("main-template.s"));
    let report = session.call_ok("check_code", program.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    let proof = session.call_ok("prove", program.arguments()).await;
    assert_eq!(proof["proofs"][0]["proven"], true, "{proof}");
    // B3's bad inputs come from choice(…), each proven in turn.
    assert_eq!(
        proof["message"],
        "Every generated test is proven for every input in its range. lengua.lock records it.",
        "{proof}"
    );
    // The lock records the proof, and status shows it.
    let status = session.call_ok("status", program.arguments()).await;
    assert_eq!(
        status["program"]["proven"],
        serde_json::json!(["B1", "B3"]),
        "{status}"
    );
    assert_eq!(status["program"]["proven_by"], "z3 and cvc5", "{status}");
    // Checking the same build again keeps it; a different build drops it.
    let report = session.call_ok("check_code", program.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    let status = session.call_ok("status", program.arguments()).await;
    assert_eq!(
        status["program"]["proven"],
        serde_json::json!(["B1", "B3"]),
        "{status}"
    );
    program.write(
        "main.s",
        &asset("main-template.s").replacen("_start:\n", "_start:\n    nop\n", 1),
    );
    let status = session.call_ok("status", program.arguments()).await;
    assert_eq!(
        status["program"]["proven"],
        serde_json::json!([]),
        "a changed main.s: {status}"
    );
    let report = session.call_ok("check_code", program.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    let status = session.call_ok("status", program.arguments()).await;
    assert_eq!(
        status["program"]["proven"],
        serde_json::json!([]),
        "a new build: {status}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_absolute_difference_example_is_proven() {
    // Claude Code wrote this one through lengua; it picks the size of a - b with cmovs, which
    // the prover settles by writing both the program's choice and abs(a - b) as one term.
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("z3 isn't installed");
        return;
    }
    let session = session().await;
    let folder = Program::new();
    for name in ["spec.md", "tests.toml", "main.s"] {
        folder.write(name, &example("absdiff", name));
    }
    session.record(&folder).await;
    let tests = session.call_ok("check_tests", folder.arguments()).await;
    assert_eq!(tests["checked"], true, "{tests}");
    let report = session.call_ok("check_code", folder.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    let proof = session.call_ok("prove", folder.arguments()).await;
    assert_eq!(proof["proofs"][0]["test"], "generated B1", "{proof}");
    // B3 and B4 draw bad arguments with choice(…): each one is proven with every number.
    assert_eq!(
        proof["message"],
        "Every generated test is proven for every input in its range. lengua.lock records it.",
        "{proof}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_avx512_shouts_in_the_benchmark_pass_their_checks() {
    // The benchmark's AVX-512 versions of the parallel template, on 16 threads and on one.
    if !lengua::platform::Machine::detect().features.avx512bw {
        eprintln!("this CPU has no AVX-512BW");
        return;
    }
    for variant in ["avx512.s", "avx512-one.s"] {
        let session = session().await;
        let folder = Program::new();
        folder.write("spec.md", &asset("parallel-template-spec.md"));
        folder.write("tests.toml", &asset("parallel-template-tests.toml"));
        folder.write(
            "main.s",
            &std::fs::read_to_string(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("benches/programs/shout")
                    .join(variant),
            )
            .expect("an AVX-512 shout"),
        );
        session.record(&folder).await;
        let tests = session.call_ok("check_tests", folder.arguments()).await;
        assert_eq!(tests["checked"], true, "{variant}: {tests}");
        let report = session.call_ok("check_code", folder.arguments()).await;
        assert_eq!(report["stamped"], true, "{variant}: {report}");
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn the_avx512_upcase_in_the_benchmark_passes_its_checks() {
    // The benchmark's AVX-512 upcase is a lengua program too: same spec and tests as the
    // example, 64 bytes at a time. Only where the CPU has AVX-512BW.
    if !lengua::platform::Machine::detect().features.avx512bw {
        eprintln!("this CPU has no AVX-512BW");
        return;
    }
    let session = session().await;
    let folder = Program::new();
    for name in ["spec.md", "tests.toml"] {
        folder.write(name, &example("upcase", name));
    }
    folder.write(
        "main.s",
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches/programs/upcase/avx512.s"),
        )
        .expect("the AVX-512 upcase"),
    );
    session.record(&folder).await;
    let tests = session.call_ok("check_tests", folder.arguments()).await;
    assert_eq!(tests["checked"], true, "{tests}");
    let report = session.call_ok("check_code", folder.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_file_example_proves_its_file_handling() {
    // upcase's usage and missing-file behaviors draw their names with choice(…); the proof opens,
    // reads and writes files as the test gives them.
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let session = session().await;
    let folder = Program::new();
    for name in ["spec.md", "tests.toml", "main.s"] {
        folder.write(name, &example("upcase", name));
    }
    session.record(&folder).await;
    let tests = session.call_ok("check_tests", folder.arguments()).await;
    assert_eq!(tests["checked"], true, "{tests}");
    let report = session.call_ok("check_code", folder.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    let proof = session.call_ok("prove", folder.arguments()).await;
    // Usage (B2), a missing input (B3) and a file over the limit (B4, a mebibyte of any of its
    // characters) are proven; capitalizing any text (B1) is only tested: its loop is SSE.
    assert_eq!(
        proof["message"],
        "Proven for every input in range: generated B2, generated B3, generated B4. lengua.lock \
         records it.",
        "{proof}"
    );
    assert!(
        proof["proofs"][0]["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("the instruction Movd")),
        "{proof}"
    );
    session.service.cancel().await.expect("close the session");
}

/// Three minutes: mean's arithmetic isn't proven yet, so three blocks run to the time limit.
#[tokio::test]
#[ignore = "slow: three of mean's blocks run to the proof's time limit"]
async fn the_decimal_example_is_proven() {
    // mean takes decimals and prints exact cents: scaled(a, 2) and places(x, 2) in its tests.
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let session = session().await;
    let folder = Program::new();
    for name in ["spec.md", "tests.toml", "main.s"] {
        folder.write(name, &example("mean", name));
    }
    session.record(&folder).await;
    let tests = session.call_ok("check_tests", folder.arguments()).await;
    assert_eq!(tests["checked"], true, "{tests}");
    let report = session.call_ok("check_code", folder.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    let proof = session.call_ok("prove", folder.arguments()).await;
    // Decimal inputs prove where the output is fixed text: usage (B4), a bad argument (B5) and
    // an argument out of range (B6).
    for index in 3..6 {
        assert_eq!(proof["proofs"][index]["proven"], true, "{proof}");
    }
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_dice_example_rolls_with_the_tests_random_bytes() {
    // The tests supply the random bytes, so each roll is exact: 200 runs, plus the proof that
    // every pair of first bytes rolls as the spec says.
    let session = session().await;
    let folder = Program::new();
    for name in ["spec.md", "tests.toml", "main.s"] {
        folder.write(name, &example("dice", name));
    }
    session.record(&folder).await;
    let tests = session.call_ok("check_tests", folder.arguments()).await;
    assert_eq!(tests["checked"], true, "{tests}");
    let report = session.call_ok("check_code", folder.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    // A die that's off by one: the tests catch it with the bytes they supply.
    folder.write(
        "main.s",
        &example("dice", "main.s").replace("add edx, '1'", "add edx, '0'"),
    );
    let text = session.call_error("check_code", folder.arguments()).await;
    assert!(
        text.contains(r#"stdout should be "rolled 1\n", but was "rolled 0\n""#),
        "{text}"
    );
    folder.write("main.s", &example("dice", "main.s"));
    let report = session.call_ok("check_code", folder.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_ok()
    {
        let proof = session.call_ok("prove", folder.arguments()).await;
        assert_eq!(
            proof["message"],
            "Every generated test is proven for every input in its range. lengua.lock records it.",
            "{proof}"
        );
    }
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_echo_server_answers_the_tests_client() {
    // A server: lengua listens for it, hands it the socket as file descriptor 3, has the test's
    // client talk to it, and stops it when it keeps serving.
    let session = session().await;
    let folder = Program::new();
    for name in ["spec.md", "tests.toml", "main.s"] {
        folder.write(name, &example("echo", name));
    }
    session.record(&folder).await;
    let tests = session.call_ok("check_tests", folder.arguments()).await;
    assert_eq!(tests["checked"], true, "{tests}");
    let report = session.call_ok("check_code", folder.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    let proof = session.call_ok("prove", folder.arguments()).await;
    assert_eq!(proof["proofs"][0]["proven"], false, "{proof}");

    // run serves in the background on the spec's port until it's stopped.
    let serving = session.call_ok("run", folder.arguments()).await;
    assert_eq!(serving["serving"], "127.0.0.1:7007", "{serving}");
    let again = session.call_error("run", folder.arguments()).await;
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
    let mut stop = folder.arguments();
    stop["stop"] = serde_json::json!(true);
    let stopped = session.call_ok("run", stop.clone()).await;
    assert!(
        stopped["ended"]
            .as_str()
            .is_some_and(|ended| ended.contains("stopped by lengua")),
        "{stopped}"
    );
    let nothing = session.call_error("run", stop).await;
    assert!(nothing.contains("no server"), "{nothing}");

    // A server that stops after one connection fails: the spec says it keeps serving.
    folder.write(
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
    let text = session.call_error("check_code", folder.arguments()).await;
    assert!(
        text.contains("while the test expects it to keep serving")
            || text.contains("wasn't waiting for the next one"),
        "{text}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_fetch_client_talks_to_the_tests_server() {
    // A client: the test's own server listens on a free port, which the example names as
    // {port}, and the kernel lets the program connect to that port alone.
    let session = session().await;
    let folder = Program::new();
    for name in ["spec.md", "tests.toml", "main.s"] {
        folder.write(name, &example("fetch", name));
    }
    session.record(&folder).await;
    let tests = session.call_ok("check_tests", folder.arguments()).await;
    assert_eq!(tests["checked"], true, "{tests}");
    let report = session.call_ok("check_code", folder.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    // run reaches a real server on a port the spec approves, and no other.
    let mut refused = folder.arguments();
    refused["args"] = serde_json::json!(["9", "/"]);
    let refused = session.call_ok("run", refused).await;
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
        let mut fetch = folder.arguments();
        fetch["args"] = serde_json::json!(["8080", "/hi"]);
        let fetched = session.call_ok("run", fetch).await;
        assert_eq!(fetched["stdout"], "from 8080", "{fetched}");
        assert_eq!(server.await.expect("served"), b"GET /hi HTTP/1.0\r\n\r\n");
    }
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_benchmarks_own_programs_pass_their_checks() {
    // The skill's fast programs, which the benchmark measures on 64 MiB, each with its own spec
    // and tests.
    if !lengua::platform::Machine::detect().features.avx512bw {
        eprintln!("this CPU has no AVX-512BW");
        return;
    }
    for workload in ["wc", "crc32c", "histogram"] {
        let session = session().await;
        let folder = Program::new();
        let source = |name: &str| {
            std::fs::read_to_string(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("plugin/skills/write-assembly/assets/fast")
                    .join(workload)
                    .join(name),
            )
            .unwrap_or_else(|error| panic!("{workload}/{name}: {error}"))
        };
        folder.write("spec.md", &source("spec.md"));
        folder.write("tests.toml", &source("tests.toml"));
        folder.write("main.s", &source("main.s"));
        session.record(&folder).await;
        let tests = session.call_ok("check_tests", folder.arguments()).await;
        assert_eq!(tests["checked"], true, "{workload}: {tests}");
        let report = session.call_ok("check_code", folder.arguments()).await;
        assert_eq!(report["stamped"], true, "{workload}: {report}");
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn the_word_count_claude_code_wrote_passes_its_checks() {
    // Written through lengua from a plain request in an interactive Claude Code session.
    if !lengua::platform::Machine::detect().features.avx512bw {
        eprintln!("this CPU has no AVX-512BW");
        return;
    }
    let session = session().await;
    let folder = Program::new();
    for name in ["spec.md", "tests.toml", "main.s"] {
        folder.write(name, &example("count", name));
    }
    session.record(&folder).await;
    let tests = session.call_ok("check_tests", folder.arguments()).await;
    assert_eq!(tests["checked"], true, "{tests}");
    let report = session.call_ok("check_code", folder.arguments()).await;
    assert_eq!(report["stamped"], true, "{report}");
    session.service.cancel().await.expect("close the session");
}
