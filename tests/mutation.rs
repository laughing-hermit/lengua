//! Mutation testing of what lengua proves: for every proven WebAssembly fixture and the main
//! template, semantic mutants of its main.wat (a number moved by one, a comparison turned to its
//! opposite, a named constant moved by one) each go through `check`, and each must be caught, by
//! the rules, a test, or a proof, never come out proven. A mutant that survives is a missing test
//! or proof, or one that changes nothing an idea can see, which its report shows for a person to
//! judge.
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

/// Mutants that change nothing an idea can see, each with why: they come out proven, rightly.
/// Each moves a region by one byte, or makes it one byte larger, into bytes that no other region
/// holds: every read and write lands one byte along, or exactly where it did, and nothing a
/// program prints, sends or exits with changes. A run that finds another names it here, with why.
const EQUIVALENT: &[(&str, &str)] = &[
    (
        "tests/fixtures/add/main.wat L48:const+1",
        "$argv moves from 2064 to 2065: its 12 bytes end at 2077, below $args at 2080, and \
         WebAssembly reads a 32-bit number at any address, so args_get writes the same three \
         addresses and main reads the same two",
    ),
    (
        "tests/fixtures/echo/main.wat L21:const+1",
        "$accepted.size grows from 4 to 5: no code reads the size, sock_accept still writes 4 \
         bytes at 1024, and the next region, $line, starts at 65520",
    ),
    (
        "tests/fixtures/cat/main.wat L22:const+1",
        "$buffer.size grows from 5000 to 5001, inside the one page: for the 0 to 5000 bytes the \
         idea gives, $fill reads them all and then the input's end either way. Only an input past \
         5000 bytes, which the idea doesn't speak of, would show the one byte more",
    ),
    (
        "tests/fixtures/cat-stream/main.wat L24:const+1",
        "$buffer moves from 1024 to 1025: its 4096 bytes end at 5121, inside the one page, it is \
         the only region, and $fill and $write_all are both lent the same address",
    ),
    (
        "plugin/skills/compile/assets/main-template.wat L47:const+1",
        "$sizes.size grows from 8 to 9: no code reads the size, args_sizes_get still writes 8 \
         bytes at 2048, and the next region, $argv, starts at 2064",
    ),
];

/// The programs whose mutants are checked: each idea, then its main.wat.
const PROGRAMS: &[(&str, &str)] = &[
    ("tests/fixtures/add/idea.md", "tests/fixtures/add/main.wat"),
    // The library's parts: a mutant of a part must not come out proven either.
    ("examples/decimal/idea.md", "examples/decimal/main.wat"),
    ("examples/lines/idea.md", "examples/lines/main.wat"),
    ("examples/crc32/idea.md", "examples/crc32/main.wat"),
    ("examples/upcase/idea.md", "examples/upcase/main.wat"),
    ("examples/fetch/idea.md", "examples/fetch/main.wat"),
    (
        "tests/fixtures/echo/idea.md",
        "tests/fixtures/echo/main.wat",
    ),
    ("tests/fixtures/cat/idea.md", "tests/fixtures/cat/main.wat"),
    (
        "tests/fixtures/cat-stream/idea.md",
        "tests/fixtures/cat-stream/main.wat",
    ),
    (
        "plugin/skills/compile/assets/main-template-idea.md",
        "plugin/skills/compile/assets/main-template.wat",
    ),
];

fn read(path: &str) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// The comparisons a mutant turns to their opposites, each with its opposite.
const OPPOSITES: &[(&str, &str)] = &[
    ("i32.eq", "i32.ne"),
    ("i32.lt_s", "i32.ge_s"),
    ("i32.lt_u", "i32.ge_u"),
    ("i32.gt_s", "i32.le_s"),
    ("i32.gt_u", "i32.le_u"),
    ("i64.eq", "i64.ne"),
    ("i64.lt_s", "i64.ge_s"),
    ("i64.lt_u", "i64.ge_u"),
    ("i64.gt_s", "i64.le_s"),
    ("i64.gt_u", "i64.le_u"),
];

/// Where the operator `name` stands as a whole word in `code`: after `(` or a space, and before
/// a space, `)` or the line's end.
fn operator_at(code: &str, name: &str) -> Option<usize> {
    code.match_indices(name).map(|(at, _)| at).find(|at| {
        let before = code[..*at].chars().next_back();
        let after = code[at + name.len()..].chars().next();
        matches!(before, Some('(' | ' ')) && matches!(after, None | Some(' ' | ')'))
    })
}

/// Where the last number of a `const` operator stands in `code`, as `(i32.const 5)`: its start
/// and end, and the number.
fn last_const(code: &str) -> Option<(usize, usize, i64)> {
    let mut found = None;
    for prefix in ["i32.const ", "i64.const "] {
        for (at, _) in code.match_indices(prefix) {
            let start = at + prefix.len();
            let digits = code[start..].bytes().take_while(u8::is_ascii_digit).count();
            let end = start + digits;
            if let Ok(number) = code[start..end].parse::<i64>()
                && found.is_none_or(|(known, _, _)| start > known)
            {
                found = Some((start, end, number));
            }
        }
    }
    found
}

/// Semantic mutants of `code`, each named by its line: a named constant moved up by one, a
/// comparison turned to its opposite, or the last number of a line's `const` moved up by one.
/// The lines of an assertion, comments, and the module's imports, memory and data are left
/// alone.
fn mutants(code: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = code.lines().collect();
    let with = |index: usize, new: String| -> String {
        let mut out: Vec<String> = lines.iter().map(|line| (*line).to_owned()).collect();
        out[index] = new;
        out.join("\n") + "\n"
    };
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let code_part = line.split(";;").next().unwrap_or(line);
        let trimmed = code_part.trim();
        if trimmed.is_empty()
            || code_part.contains("$assert_fail")
            || ["(import", "(memory", "(data", "(export", "(module"]
                .iter()
                .any(|start| trimmed.starts_with(*start))
        {
            continue;
        }
        let at_const = last_const(code_part);
        if trimmed.starts_with("(global ") {
            if let Some((start, end, number)) = at_const {
                found.push((
                    format!("L{}:const+1", index + 1),
                    with(
                        index,
                        format!("{}{}{}", &line[..start], number + 1, &line[end..]),
                    ),
                ));
            }
            continue;
        }
        if let Some((name, opposite, at)) = OPPOSITES
            .iter()
            .flat_map(|(a, b)| [(*a, *b), (*b, *a)])
            .find_map(|(name, opposite)| {
                operator_at(code_part, name).map(|at| (name, opposite, at))
            })
        {
            found.push((
                format!("L{}:cond", index + 1),
                with(
                    index,
                    format!("{}{opposite}{}", &line[..at], &line[at + name.len()..]),
                ),
            ));
            continue;
        }
        if let Some((start, end, number)) = at_const
            && number < 1 << 20
        {
            found.push((
                format!("L{}:imm+1", index + 1),
                with(
                    index,
                    format!("{}{}{}", &line[..start], number + 1, &line[end..]),
                ),
            ));
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
    /// It doesn't build, or breaks a rule.
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
        Some("code_problems") => Fate::Rules,
        Some("tests_failed") => Fate::Tests,
        _ if lines.iter().any(|line| line.contains("main.wat is wrong")) => Fate::Proof,
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
/// `LENGUA_MUTATE_ONLY` set to a comma-separated list, only the programs whose main.wat path
/// contains one of its parts, so a program just added can be run alone.
async fn survivors(session: &Session, programs: &[(&str, &str)]) -> Vec<String> {
    let only = std::env::var("LENGUA_MUTATE_ONLY").ok();
    let wanted = |code: &str| {
        only.as_deref()
            .is_none_or(|only| only.split(',').any(|part| code.contains(part)))
    };
    let mut survived = Vec::new();
    for (idea, code) in programs.iter().filter(|(_, code)| wanted(code)) {
        let (idea_text, code_text) = (read(idea), read(code));
        let mut tally = std::collections::BTreeMap::new();
        for (name, mutant) in mutants(&code_text) {
            let program = Program::new();
            // The parts it builds on stand beside it, each checked first.
            let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(code);
            let folder = folder.parent().expect("main.wat stands in a folder");
            if let Err(problem) = session.settle_parts(&program, folder).await {
                panic!("{code}: {problem}");
            }
            program.write("idea.md", &idea_text);
            program.write("main.wat", &mutant);
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
    let session = Session::proving_for(Protocol::Discover, PROOF_SECONDS).await;
    let survived = survivors(&session, PROGRAMS).await;
    session.service.cancel().await.expect("close the session");
    assert!(
        survived.is_empty(),
        "mutants that came out proven: {survived:#?}"
    );
}

#[test]
fn mutants_change_one_line_each_and_leave_assertions_alone() {
    let code = "(module\n  (global $LIMIT i32 (i32.const 64))\n  (func $f (param $n i32)\n    \
                (if (i32.lt_u (local.get $n) (global.get $LIMIT))\n      (then (call \
                $assert_fail (i32.const 3)) unreachable))\n    ;; i32.eq in a comment\n    \
                (drop (i32.add (local.get $n) (i32.const 5)))))\n";
    let found = mutants(code);
    let names: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["L2:const+1", "L4:cond", "L7:imm+1"]);
    assert!(found[0].1.contains("(global $LIMIT i32 (i32.const 65))"));
    assert!(found[1].1.contains("(if (i32.ge_u (local.get $n)"));
    assert!(found[2].1.contains("(i32.const 6)"));
    for (_, mutant) in &found {
        assert_eq!(mutant.lines().count(), code.lines().count());
        assert!(mutant.contains("(call $assert_fail (i32.const 3))"));
    }
}

#[test]
fn a_comparison_turns_only_where_it_stands_whole() {
    assert_eq!(operator_at("(i32.lt_s (local.get $a)", "i32.lt_s"), Some(1));
    assert_eq!(operator_at("(i32.lt_s_x)", "i32.lt_s"), None);
    assert_eq!(operator_at("i64.eqz", "i64.eq"), None);
    assert_eq!(
        last_const("(i32.const 4) (i64.const 70)"),
        Some((25, 27, 70))
    );
    assert_eq!(last_const("(local.get $n)"), None);
}
