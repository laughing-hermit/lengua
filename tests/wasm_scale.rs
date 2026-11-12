//! What the scale test (W22) asked of `check`'s reply: every problem of an idea in one reply,
//! in the order of its lines; how full main.wat's functions are, whether or not it follows the
//! rules; and the named texts of its data, with the numbers that say where they are held to
//! them.

mod common;

use common::{ADD_CODE, ADD_IDEA, Program, Protocol, Session};
use serde_json::Value;

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// The lines of a field of a reply.
fn lines<'r>(report: &'r Value, field: &str) -> Vec<&'r str> {
    report[field]
        .as_array()
        .into_iter()
        .flatten()
        .map(text)
        .collect()
}

/// The line a problem names, when it starts with `line N: `.
fn line_named(problem: &str) -> Option<usize> {
    let (number, _) = problem.strip_prefix("line ")?.split_once(": ")?;
    number.parse().ok()
}

/// The first sentence of add's `# Purpose`.
const PURPOSE: &str = "The program adds two whole numbers between `-1000000000` and `1000000000`.";

/// A sentence of 32 words, more than STE allows.
const LONG: &str = "The program is small and it is quick and it is plain and it is exact and it \
                    is safe and it is good and it is here and it is done.";

/// One check reports a problem of the idea's sentences and a problem of its tests together, in
/// the order of the idea's lines: before, the first hid the second until it was fixed.
#[tokio::test]
async fn every_problem_of_an_idea_comes_in_one_reply() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    let long = ADD_IDEA.replace(PURPOSE, &format!("{PURPOSE} {LONG}"));
    assert_ne!(long, ADD_IDEA);
    let both = long.replace("{a + b}", "{a - b}");
    assert_ne!(both, long);
    program.write("idea.md", &both);
    let report = session.record(&program).await;
    assert_eq!(report["state"], "idea_problems", "{report}");
    let problems = lines(&report, "problems");
    let sentence = problems
        .iter()
        .position(|problem| problem.contains("STE: this sentence has 32 words"));
    let formula = problems
        .iter()
        .position(|problem| problem.contains("formula disagrees with the idea"));
    assert!(sentence.is_some(), "the sentence's problem: {report}");
    assert!(formula.is_some(), "the test's problem: {report}");
    assert!(sentence < formula, "in the order of the lines: {report}");
    let named: Vec<usize> = problems
        .iter()
        .filter_map(|problem| line_named(problem))
        .collect();
    assert!(named.len() >= 2, "{report}");
    assert!(
        named.windows(2).all(|pair| pair[0] <= pair[1]),
        "in the order of the lines: {report}"
    );
    assert!(program.read("lengua.lock").is_none(), "nothing is recorded");

    // The sentence fixed, the test's problem is what is left.
    program.write("idea.md", &ADD_IDEA.replace("{a + b}", "{a - b}"));
    let report = session.check(&program).await;
    assert_eq!(report["state"], "idea_problems", "{report}");
    let problems = lines(&report, "problems");
    assert_eq!(problems.len(), 1, "{report}");
    assert!(
        problems[0].contains("formula disagrees with the idea"),
        "{report}"
    );

    // The test fixed and the sentence not: the sentence's problem alone.
    program.write("idea.md", &long);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "idea_problems", "{report}");
    let problems = lines(&report, "problems");
    assert_eq!(problems.len(), 1, "{report}");
    assert!(
        problems[0].contains("STE: this sentence has 32 words"),
        "{report}"
    );

    program.write("idea.md", ADD_IDEA);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    session.service.cancel().await.expect("close the session");
}

/// add's main.wat with its usage text named, and its region's size marked as the text's length.
fn named_add() -> String {
    let data = "(data (i32.const 1024) \"usage: add <a> <b>\\0a\")";
    let size = "(global $usage.rosize i32 (i32.const 19))";
    assert!(ADD_CODE.contains(data) && ADD_CODE.contains(size));
    ADD_CODE
        .replace(data, &format!("{data}  ;; text: usage"))
        .replace(size, &format!("{size}  ;; len: usage"))
}

/// `check` says how full main.wat's functions are and where its named texts are, when the
/// rules pass and when they don't; a text that changes its length makes the number marked with
/// its name a problem that says the right number, and the reply's table has it too.
#[tokio::test]
async fn check_counts_operators_and_places_named_texts() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_idea();
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    assert!(report["functions"].is_null(), "no code yet: {report}");
    assert!(report["data"].is_null(), "no code yet: {report}");

    // The text gets two bytes shorter and its length stays 19 in the code.
    let named = named_add();
    let shorter = named.replace("usage: add <a> <b>\\0a", "use: add <a> <b>\\0a");
    assert_ne!(shorter, named);
    program.write("main.wat", &shorter);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    let line = shorter
        .lines()
        .position(|line| line.contains("(global $usage.rosize"))
        .expect("the size's line")
        + 1;
    let wanted = format!(
        "rule 8 (named constants) main.wat:{line}: $usage.rosize is 19, and the text `usage` \
         has 17 bytes. Fix: write `(i32.const 17)`: `check`'s reply lists every named text's \
         address, offset and length under `data`."
    );
    assert!(
        lines(&report, "problems").contains(&wanted.as_str()),
        "{report}"
    );
    assert_eq!(
        lines(&report, "data"),
        [
            "1 named text, each with its address, its offset in its region and its length in \
             bytes: write each number marked `;; at:`, `;; offset:` or `;; len:` from its line",
            "usage: at 1024, offset 0 in $usage, len 17",
        ],
        "{report}"
    );
    let functions = lines(&report, "functions");
    assert!(
        functions.first().is_some_and(|line| line.starts_with(
            "rule 4: 3 functions, at most 180 operators in each; the largest is $"
        )),
        "the count comes with the problems: {report}"
    );

    // The numbers right: done, with the count and one line for the texts.
    program.write("main.wat", &named);
    let report = session.settle(&program).await;
    assert_eq!(report["state"], "done", "{report}");
    assert_eq!(
        lines(&report, "data"),
        ["1 named text: every number marked `;; at:`, `;; offset:` or `;; len:` is its text's \
          (1 marked)"],
        "{report}"
    );
    let functions = lines(&report, "functions");
    assert_eq!(functions.len(), 2, "{report}");
    assert!(
        functions[0].starts_with(
            "rule 4: 3 functions, at most 180 operators in each; the largest is $"
        ),
        "{report}"
    );
    assert!(
        functions[1].starts_with("rule 5: ") && functions[1].contains(" in 3 functions, "),
        "{report}"
    );

    // A main.wat with no named text has no table.
    program.write("main.wat", ADD_CODE);
    let report = session.settle(&program).await;
    assert_eq!(report["state"], "done", "{report}");
    assert!(report["data"].is_null(), "{report}");
    assert_eq!(lines(&report, "functions").len(), 2, "{report}");
    session.service.cancel().await.expect("close the session");
}
