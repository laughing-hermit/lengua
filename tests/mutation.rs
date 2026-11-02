//! Mutation testing of what lengua proves: for every proven example and template, on both CPUs,
//! semantic mutants of its main.s (a number moved by one, a branch's condition flipped, a named
//! constant moved by one) each go through `check`, and each must be caught, by the rules, a
//! test, or a proof, never come out proven. A mutant that survives is a missing test or proof, or
//! one that changes nothing a spec can see, which its report shows for a person to judge.
//!
//! Slow (a check of every mutant, its proofs included), so it's ignored: run it by hand, or on
//! CI's dispatch, with `--ignored every_proven_programs_mutants_are_caught`.

mod common;

use std::path::PathBuf;

use common::{Program, Protocol, Session};
use serde_json::Value;

/// How many mutants of each program, taken evenly over its lines.
const MUTANTS: usize = 8;

/// How long a mutant's proofs may take, in seconds.
const PROOF_SECONDS: u64 = 60;

/// Mutants that change nothing a spec can see, each with why: they come out proven, rightly.
/// Found by this test's first run, 2026-11-02; any other survivor fails it.
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
        "tests/fixtures/linux-aarch64/cat/main.s L18:const+1",
        "fill's end byte 0, which the spec's input never holds",
    ),
    (
        "tests/fixtures/linux-aarch64/cat/main.s L148:cond",
        "write_all's error test flipped: it differs only for a write that fails or falls short, \
         which a blocking write to a pipe doesn't",
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
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let code_part = line
            .split("//")
            .next()
            .unwrap_or(line)
            .split('#')
            .next()
            .unwrap_or(line);
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

/// Every mutant of each of `programs`, checked in `session`: the survivors, described.
async fn survivors(session: &Session, programs: &[(&str, &str)]) -> Vec<String> {
    let mut survived = Vec::new();
    for (spec, code) in programs {
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
        let programs: Vec<(String, String)> = ["add", "upcase", "fetch", "echo", "cat"]
            .into_iter()
            .map(fixture)
            .collect();
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
