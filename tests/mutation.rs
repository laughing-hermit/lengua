//! Mutation testing of what lengua proves: for every proven example and template, on both CPUs,
//! semantic mutants of its main.s (a number moved by one, a branch's condition flipped, a named
//! constant moved by one) each go through `check`, and each must be caught, by the rules, a
//! test, or a proof, never come out proven. A mutant that survives is a missing test or proof, or
//! one that changes nothing a spec can see, which its report shows for a person to judge.
//!
//! Slow (a check of every mutant, its proofs included), so it's ignored: run it by hand, or on
//! CI's dispatch, with `--ignored every_proven_programs_mutants_are_caught`, and
//! `LENGUA_MUTATE_ONLY` to run some programs alone.

mod common;

use std::path::PathBuf;

use common::{Program, Protocol, Session};
use serde_json::Value;

/// How many mutants of each program, taken evenly over its lines.
const MUTANTS: usize = 8;

/// How long a mutant's proofs may take, in seconds.
const PROOF_SECONDS: u64 = 60;

/// Mutants that change nothing a spec can see, each with why: they come out proven, rightly.
/// Found by this test's first run, 2026-11-02; any other survivor fails it. write_all's error
/// test flipped came out proven too, until a test could close standard output (2026-11-02):
/// now the write fails, and that test catches it.
const EQUIVALENT: &[(&str, &str)] = &[
    (
        "examples/echo/main.s L15:const+1",
        "LINE_MAX one more, past the spec's longest line",
    ),
    (
        "examples/echo/main.s L61:cond",
        "a jump to the very next line, flipped",
    ),
    (
        "tests/fixtures/linux-aarch64/cat-stream/main.s L18:const+1",
        "fill's end byte 0, which the spec's input never holds",
    ),
    // Found by the first run that moved AArch64's numbers (2026-11-04), after the mutator stopped
    // cutting every line at its first `#`.
    (
        "plugin/skills/compile/assets/linux-aarch64/main-template.s L167:imm+1",
        "write_i64's sign test against 1 sends zero down the negating path, and zero negated is \
         zero: the output is the same for every number",
    ),
    // Found by the first run after the prover's model of AArch64 read the overflow flag, `tbnz`
    // on the top bit and `msub` as x86-64's reads them (2026-11-05): add's proof is complete on
    // ARM64 now, where the mutant used to come out unproven.
    (
        "tests/fixtures/linux-aarch64/add/main.s L273:imm+1",
        "write_all's success value, 1 for 0: every caller tests only its sign (`tbnz x0, #63`) \
         and then sets the result itself, so the output and the exit code are the same",
    ),
];

fn read(path: &str) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// The conditions a mutant flips, each with its opposite: x86-64's jumps and AArch64's
/// branches.
const OPPOSITES: &[(&str, &str)] = &[
    ("je", "jne"),
    ("jz", "jnz"),
    ("jb", "jae"),
    ("jbe", "ja"),
    ("jl", "jge"),
    ("jle", "jg"),
    ("js", "jns"),
    ("b.eq", "b.ne"),
    ("b.lo", "b.hs"),
    ("b.ls", "b.hi"),
    ("b.lt", "b.ge"),
    ("b.le", "b.gt"),
    ("b.mi", "b.pl"),
    ("cbz", "cbnz"),
    ("tbz", "tbnz"),
];

/// Semantic mutants of `code`, each named by its line: the last number of an instruction moved
/// up by one, a condition flipped, a named constant moved up by one. Lines that set an
/// assertion's number, and comments, are left alone.
fn mutants(code: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = code.lines().collect();
    let with = |index: usize, new: String| -> String {
        let mut out: Vec<String> = lines.iter().map(|line| (*line).to_owned()).collect();
        out[index] = new;
        out.join("\n") + "\n"
    };
    // A comment starts with `#` in x86-64's dialect and with `//` in AArch64's, where `#` writes
    // a number: `mov x0, #1`, `[sp, #16]`.
    let comment = if lines.iter().any(|line| line.starts_with(".arch ")) {
        "//"
    } else {
        "#"
    };
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let code_part = line.split(comment).next().unwrap_or(line);
        let trimmed = code_part.trim();
        if trimmed.is_empty() || trimmed.ends_with(':') {
            continue;
        }
        let next = lines.get(index + 1).copied().unwrap_or("");
        if next.contains("assert_fail") {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix(".equ ") {
            if let Some((name, value)) = rest.split_once(',')
                && let Ok(number) = value.trim().parse::<i64>()
            {
                let indent = &line[..line.len() - line.trim_start().len()];
                found.push((
                    format!("L{}:const+1", index + 1),
                    with(index, format!("{indent}.equ {name}, {}", number + 1)),
                ));
            }
            continue;
        }
        if trimmed.starts_with('.') {
            continue;
        }
        let (mnemonic, _) = trimmed.split_once(' ').unwrap_or((trimmed, ""));
        if let Some((_, opposite)) = OPPOSITES
            .iter()
            .flat_map(|(a, b)| [(*a, *b), (*b, *a)])
            .find(|(name, _)| *name == mnemonic)
        {
            found.push((
                format!("L{}:cond", index + 1),
                with(index, line.replacen(mnemonic, opposite, 1)),
            ));
            continue;
        }
        // The last decimal number in the operands, as `5`, `#5` or `[x0, #16]`.
        let bytes = code_part.as_bytes();
        let mut end = bytes.len();
        while end > 0 && !bytes[end - 1].is_ascii_digit() {
            end -= 1;
        }
        let mut start = end;
        while start > 0 && bytes[start - 1].is_ascii_digit() {
            start -= 1;
        }
        let in_name =
            start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_');
        if start < end && !in_name && start > mnemonic.len() {
            let number: i64 = code_part[start..end].parse().unwrap_or(0);
            if number < 1 << 20 {
                found.push((
                    format!("L{}:imm+1", index + 1),
                    with(
                        index,
                        format!("{}{}{}", &line[..start], number + 1, &line[end..]),
                    ),
                ));
            }
        }
    }
    if found.len() > MUTANTS {
        let step = found.len().div_ceil(MUTANTS);
        found = found.into_iter().step_by(step).collect();
    }
    found
}

/// What became of a mutant.
#[derive(Debug, PartialEq, Eq)]
enum Fate {
    /// It doesn't assemble, or breaks a rule.
    Rules,
    /// A test fails.
    Tests,
    /// A proof found inputs where it's wrong.
    Proof,
    /// Every proof says it's right: a test or proof is missing, or it changes nothing seen.
    Survived,
    /// Tested, but not every block proven: neither caught nor cleared.
    Unknown,
}

fn fate(report: &Value) -> Fate {
    let lines: Vec<&str> = report["proofs"]
        .as_array()
        .map(|proofs| proofs.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    match report["state"].as_str() {
        Some("code_problems" | "build_failed") => Fate::Rules,
        Some("tests_failed") => Fate::Tests,
        _ if lines.iter().any(|line| {
            line.contains("main.s is wrong") || line.contains("breaks the memory rules")
        }) =>
        {
            Fate::Proof
        }
        Some("done")
            if !lines.is_empty()
                && lines
                    .iter()
                    .all(|line| line.contains("proven for every input in range")) =>
        {
            Fate::Survived
        }
        _ => Fate::Unknown,
    }
}

/// Every mutant of each of `programs`, checked in `session`: the survivors, described. With
/// `LENGUA_MUTATE_ONLY` set to a comma-separated list, only the programs whose main.s path
/// contains one of its parts, so a program just added can be run alone.
async fn survivors(session: &Session, programs: &[(&str, &str)]) -> Vec<String> {
    let only = std::env::var("LENGUA_MUTATE_ONLY").ok();
    let wanted = |code: &str| {
        only.as_deref()
            .is_none_or(|only| only.split(',').any(|part| code.contains(part)))
    };
    let mut survived = Vec::new();
    for (spec, code) in programs.iter().filter(|(_, code)| wanted(code)) {
        let (spec_text, code_text) = (read(spec), read(code));
        let mut tally = std::collections::BTreeMap::new();
        for (name, mutant) in mutants(&code_text) {
            let program = Program::new();
            program.write("spec.md", &spec_text);
            program.write("main.s", &mutant);
            session.record(&program).await;
            let report = session.settle(&program).await;
            let found = fate(&report);
            eprintln!("{code} {name}: {found:?}");
            let named = format!("{code} {name}");
            if found == Fate::Survived && !EQUIVALENT.iter().any(|(known, _)| *known == named) {
                survived.push(named);
            }
            *tally.entry(format!("{found:?}")).or_insert(0) += 1;
        }
        eprintln!("{code}: {tally:?}");
    }
    survived
}

#[tokio::test]
#[ignore = "checks every mutant through to its proofs: by hand, or on CI's dispatch"]
async fn every_proven_programs_mutants_are_caught() {
    let mut survived = Vec::new();
    if cfg!(target_arch = "x86_64") {
        let session = Session::proving_for(Protocol::Discover, PROOF_SECONDS).await;
        survived.extend(
            survivors(
                &session,
                &[
                    ("examples/add/spec.md", "examples/add/main.s"),
                    ("examples/upcase/spec.md", "examples/upcase/main.s"),
                    ("examples/fetch/spec.md", "examples/fetch/main.s"),
                    ("examples/echo/spec.md", "examples/echo/main.s"),
                    (
                        "plugin/skills/compile/assets/template-spec.md",
                        "plugin/skills/compile/assets/main-template.s",
                    ),
                    ("tests/fixtures/cat/spec.md", "tests/fixtures/cat/main.s"),
                    (
                        "tests/fixtures/cat-stream/spec.md",
                        "tests/fixtures/cat-stream/main.s",
                    ),
                ],
            )
            .await,
        );
        session.service.cancel().await.expect("close the session");
    }
    let aarch64 = cfg!(target_arch = "aarch64") || common::qemu_runs_aarch64();
    if aarch64 {
        let session = if cfg!(target_arch = "aarch64") {
            Session::proving_for(Protocol::Discover, PROOF_SECONDS).await
        } else {
            Session::emulating_for(Protocol::Discover, "linux-aarch64", PROOF_SECONDS).await
        };
        let fixture = |name: &str| {
            (
                format!("tests/fixtures/linux-aarch64/{name}/spec.md"),
                format!("tests/fixtures/linux-aarch64/{name}/main.s"),
            )
        };
        let mut programs: Vec<(String, String)> =
            ["add", "upcase", "fetch", "echo", "cat", "cat-stream"]
                .into_iter()
                .map(fixture)
                .collect();
        // The template, as x86-64's is among its programs.
        programs.push((
            "plugin/skills/compile/assets/template-spec.md".to_owned(),
            "plugin/skills/compile/assets/linux-aarch64/main-template.s".to_owned(),
        ));
        let borrowed: Vec<(&str, &str)> = programs
            .iter()
            .map(|(spec, code)| (spec.as_str(), code.as_str()))
            .collect();
        survived.extend(survivors(&session, &borrowed).await);
        session.service.cancel().await.expect("close the session");
    }
    assert!(
        survived.is_empty(),
        "mutants that came out proven: {survived:#?}"
    );
}

#[test]
fn mutants_change_one_line_each_and_leave_assertions_alone() {
    let code = "_start:\n    cmp rdi, 5\n    jb .Lok\n    mov edi, 3\n    call assert_fail\n\
                .equ LIMIT, 64\n.Lok:\n    ret\n";
    let found = mutants(code);
    let names: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["L2:imm+1", "L3:cond", "L6:const+1"]);
    assert!(found[0].1.contains("cmp rdi, 6"));
    assert!(found[1].1.contains("jae .Lok"));
    assert!(found[2].1.contains(".equ LIMIT, 65"));
    for (_, mutant) in &found {
        assert_eq!(mutant.lines().count(), code.lines().count());
    }
}

#[test]
fn aarch64_mutants_move_its_immediates_as_x86_64_ones_move_theirs() {
    let code = ".arch armv8-a\n_start:\n    cmp x0, #5  // five\n    b.lo .Lok\n    mov w0, #3\n    \
                bl assert_fail\n.equ LIMIT, 64\n.Lok:\n    ldr x1, [sp, #16]\n    ret\n";
    let found = mutants(code);
    let names: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["L3:imm+1", "L4:cond", "L7:const+1", "L9:imm+1"]);
    assert!(found[0].1.contains("cmp x0, #6  // five"));
    assert!(found[1].1.contains("b.hs .Lok"));
    assert!(found[2].1.contains(".equ LIMIT, 65"));
    assert!(found[3].1.contains("[sp, #17]"));
}
