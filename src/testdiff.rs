//! What changed after the user's request: a plain line diff of idea.md against the version
//! lengua recorded first, and for the tests inside it, lengua's own marks on the changes that
//! test less. The AI that writes the code also writes the tests, so that judgment stays here;
//! the agent only explains each change.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use toml::{Table, Value};

use crate::expr::{Function, Generator, Template};

/// The most changed lines a diff shows.
const MAX_LINES: usize = 40;

/// The longest line a diff shows, in characters.
const MAX_LINE_CHARS: usize = 160;

/// Past this many cells, the changed middle of two files isn't aligned line by line.
const MAX_CELLS: usize = 1_000_000;

/// The runs a generated block makes when it doesn't say.
const DEFAULT_RUNS: i64 = 100;

/// The lines that changed from `before` to `after`, as `- 12: old` and `+ 12: new` with each
/// version's own line numbers; empty when nothing changed.
#[must_use]
pub fn line_diff(before: &str, after: &str) -> String {
    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old_middle = &old[prefix..old.len() - suffix];
    let new_middle = &new[prefix..new.len() - suffix];
    let mut changes: Vec<(char, usize, &str)> = Vec::new();
    if old_middle.len().saturating_mul(new_middle.len()) <= MAX_CELLS {
        // Longest common subsequence of the middles, from the end.
        let width = new_middle.len() + 1;
        let mut common = vec![0_u32; (old_middle.len() + 1) * width];
        for i in (0..old_middle.len()).rev() {
            for j in (0..new_middle.len()).rev() {
                common[i * width + j] = if old_middle[i] == new_middle[j] {
                    common[(i + 1) * width + j + 1] + 1
                } else {
                    common[(i + 1) * width + j].max(common[i * width + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < old_middle.len() || j < new_middle.len() {
            if i < old_middle.len() && j < new_middle.len() && old_middle[i] == new_middle[j] {
                i += 1;
                j += 1;
            } else if j == new_middle.len()
                || (i < old_middle.len()
                    && common[(i + 1) * width + j] >= common[i * width + j + 1])
            {
                changes.push(('-', prefix + i + 1, old_middle[i]));
                i += 1;
            } else {
                changes.push(('+', prefix + j + 1, new_middle[j]));
                j += 1;
            }
        }
    } else {
        changes.extend(
            old_middle
                .iter()
                .enumerate()
                .map(|(i, line)| ('-', prefix + i + 1, *line)),
        );
        changes.extend(
            new_middle
                .iter()
                .enumerate()
                .map(|(j, line)| ('+', prefix + j + 1, *line)),
        );
    }
    let mut text = String::new();
    for (sign, number, line) in changes.iter().take(MAX_LINES) {
        let _ = writeln!(text, "{sign} {number}: {}", shorten(line, MAX_LINE_CHARS));
    }
    if changes.len() > MAX_LINES {
        let _ = writeln!(text, "and {} more changed lines", changes.len() - MAX_LINES);
    }
    text
}

/// The changes from `before` to `after`, two versions of idea.md, that test less, one line each:
/// fewer runs, a generator that draws fewer values, exact output that became a rule, and removed
/// blocks, variables or expected files. Empty when none does, or when either version doesn't
/// parse.
#[must_use]
pub fn tests_less(before: &str, after: &str) -> Vec<String> {
    let (Some(before), Some(after)) = (tests_of(before), tests_of(after)) else {
        return Vec::new();
    };
    let mut marks = Vec::new();
    let examples_before = by_id(&before, "example");
    let examples_after = by_id(&after, "example");
    for (id, blocks) in &examples_before {
        let Some(old) = blocks.first() else { continue };
        let Some(new) = examples_after.get(id).and_then(|blocks| blocks.first()) else {
            marks.push(format!("{id}'s example was removed"));
            continue;
        };
        outputs_less(&format!("{id}'s example"), old, new, &mut marks);
    }
    let generated_before = by_id(&before, "generated");
    let generated_after = by_id(&after, "generated");
    for (id, blocks) in &generated_before {
        let news = generated_after.get(id).map_or(&[][..], Vec::as_slice);
        for (index, old) in blocks.iter().enumerate() {
            let name = if blocks.len() == 1 && news.len() <= 1 {
                format!("generated {id}")
            } else {
                format!("generated {id} (block {})", index + 1)
            };
            let Some(new) = news.get(index) else {
                marks.push(format!("{name} was removed"));
                continue;
            };
            let runs = |block: &Table| {
                block
                    .get("runs")
                    .and_then(Value::as_integer)
                    .unwrap_or(DEFAULT_RUNS)
            };
            if runs(new) < runs(old) {
                marks.push(format!(
                    "{name}: fewer runs, {} instead of {}",
                    runs(new),
                    runs(old)
                ));
            }
            vars_less(&name, old, new, &mut marks);
            outputs_less(&name, old, new, &mut marks);
        }
    }
    marks
}

/// An idea's tests as one table: each behavior's example outputs under `example`, and its
/// generated tests under `generated`, one table a row of its table, each with its behavior's `id`.
fn tests_of(idea_text: &str) -> Option<Table> {
    let idea = crate::idea::parse(idea_text).ok()?;
    let mut examples = Vec::new();
    for (id, mut outputs) in crate::testfile::example_outputs(&idea) {
        outputs.insert("id".into(), Value::String(id));
        examples.push(Value::Table(outputs));
    }
    let generated = crate::testfile::generated_tables(&idea)
        .into_iter()
        .map(Value::Table)
        .collect();
    let mut tests = Table::new();
    tests.insert("example".into(), Value::Array(examples));
    tests.insert("generated".into(), Value::Array(generated));
    Some(tests)
}

/// Blocks of one kind (`example` or `generated`) by id, in file order.
fn by_id<'a>(file: &'a Table, kind: &str) -> BTreeMap<String, Vec<&'a Table>> {
    let mut blocks: BTreeMap<String, Vec<&Table>> = BTreeMap::new();
    for block in file
        .get(kind)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_table)
    {
        if let Some(id) = block.get("id").and_then(Value::as_str) {
            blocks.entry(id.to_owned()).or_default().push(block);
        }
    }
    blocks
}

/// Marks variables that were dropped or now draw fewer values.
fn vars_less(name: &str, old: &Table, new: &Table, marks: &mut Vec<String>) {
    let vars = |block: &Table| block.get("vars").and_then(Value::as_table).cloned();
    let (Some(old_vars), new_vars) = (vars(old), vars(new).unwrap_or_default()) else {
        return;
    };
    for (var, old_text) in &old_vars {
        let Some(old_text) = old_text.as_str() else {
            continue;
        };
        let Some(new_text) = new_vars.get(var).and_then(Value::as_str) else {
            marks.push(format!(
                "{name}: no longer draws {var} ({})",
                shorten(old_text, 60)
            ));
            continue;
        };
        if old_text == new_text {
            continue;
        }
        let (Ok(old_generator), Ok(new_generator)) =
            (Generator::parse(old_text), Generator::parse(new_text))
        else {
            continue;
        };
        if let Some(how) = draws_less(&old_generator, &new_generator) {
            marks.push(format!(
                "{name}: {var} draws {how}: {} instead of {}",
                shorten(new_text, 60),
                shorten(old_text, 60)
            ));
        }
    }
}

/// How `new` draws fewer values than `old`, if it does.
fn draws_less(old: &Generator, new: &Generator) -> Option<String> {
    match (old, new) {
        (Generator::Int { lo: a, hi: b }, Generator::Int { lo: c, hi: d }) => {
            (c > a || d < b).then(|| "a narrower range".to_owned())
        }
        (
            Generator::Dec {
                lo: a,
                hi: b,
                places: p,
            },
            Generator::Dec {
                lo: c,
                hi: d,
                places: q,
            },
        ) => {
            // Compare at the finer of the two scales.
            let places = (*p).max(*q);
            let scale = |m: i128, from: u8| m.saturating_mul(10_i128.pow(u32::from(places - from)));
            let narrower = scale(*c, *q) > scale(*a, *p) || scale(*d, *q) < scale(*b, *p);
            let coarser = q < p;
            match (narrower, coarser) {
                (true, _) => Some("a narrower range".to_owned()),
                (false, true) => Some("fewer decimal places".to_owned()),
                (false, false) => None,
            }
        }
        (
            Generator::Str {
                min: a,
                max: b,
                charset: old_chars,
            },
            Generator::Str {
                min: c,
                max: d,
                charset: new_chars,
            },
        ) => {
            let mut how = Vec::new();
            if c > a || d < b {
                how.push("fewer lengths".to_owned());
            }
            let dropped: String = old_chars
                .iter()
                .filter(|character| !new_chars.contains(character))
                .collect();
            if !dropped.is_empty() {
                how.push(format!(
                    "without the characters {}",
                    shorten(&format!("{dropped:?}"), 40)
                ));
            }
            (!how.is_empty()).then(|| how.join(" and "))
        }
        (Generator::Choice(old_values), Generator::Choice(new_values)) => {
            let dropped: Vec<String> = old_values
                .iter()
                .filter(|value| !new_values.contains(value))
                .map(|value| shorten(&format!("{value:?}"), 30))
                .collect();
            (!dropped.is_empty()).then(|| format!("without {}", dropped.join(", ")))
        }
        _ => match (old.domain_size(), new.domain_size()) {
            (Some(before), Some(after)) if after >= before => None,
            _ => Some("from a different generator".to_owned()),
        },
    }
}

/// Marks exact output that became a rule, and expected files that were removed.
fn outputs_less(name: &str, old: &Table, new: &Table, marks: &mut Vec<String>) {
    for field in ["stdout", "stderr"] {
        if let Some(old_value) = old.get(field)
            && !is_rule(old_value)
            && new.get(field).is_some_and(is_rule)
        {
            marks.push(format!(
                "{name}: {field} became a rule instead of exact text"
            ));
        }
    }
    let files = |block: &Table| block.get("files_after").and_then(Value::as_table).cloned();
    if let Some(old_files) = files(old) {
        let new_files = files(new).unwrap_or_default();
        for (file, old_value) in &old_files {
            match new_files.get(file) {
                None => marks.push(format!("{name}: no longer expects the file {file}")),
                Some(new_value) if !is_rule(old_value) && is_rule(new_value) => marks.push(
                    format!("{name}: files_after.{file} became a rule instead of exact text"),
                ),
                Some(_) => {}
            }
        }
    }
}

/// Whether an expected value is a rule: `text matching` in a behavior's steps, which reads as
/// `{ matches = … }`, or a template that calls `any(…)` in a generated block.
fn is_rule(value: &Value) -> bool {
    match value {
        Value::Table(table) => table.contains_key("matches"),
        Value::String(text) => {
            Template::parse(text).is_ok_and(|template| template.calls(Function::Any))
        }
        _ => false,
    }
}

/// `text` cut to `max` characters, with `…` when it was longer.
fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let mut short: String = text.chars().take(max.saturating_sub(1)).collect();
        short.push('…');
        short
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diffs_show_each_versions_line_numbers() {
        assert_eq!(line_diff("a\nb\nc\n", "a\nb\nc\n"), "");
        assert_eq!(line_diff("a\nb\nc\n", "a\nB\nc\n"), "- 2: b\n+ 2: B\n");
        assert_eq!(line_diff("a\nc\n", "a\nb\nc\n"), "+ 2: b\n");
        assert_eq!(line_diff("a\nb\nc\n", "a\nc\n"), "- 2: b\n");
        assert_eq!(line_diff("", "x\n"), "+ 1: x\n");
    }

    #[test]
    fn diffs_stop_at_forty_lines_and_shorten_long_ones() {
        let before: String = (0..100).map(|n| format!("old {n}\n")).collect();
        let after: String = (0..100).map(|n| format!("new {n}\n")).collect();
        let diff = line_diff(&before, &after);
        assert_eq!(diff.lines().count(), 41, "{diff}");
        assert!(diff.ends_with("and 160 more changed lines\n"), "{diff}");
        let long = "x".repeat(500);
        let diff = line_diff("", &long);
        assert!(diff.chars().count() < 180, "{diff}");
        assert!(diff.contains('…'), "{diff}");
    }

    const IDEA: &str = r#"---
type: lengua/program-idea
title: roll
description: Prints a number.
---
# Purpose
Prints a number.

# Behaviors
## B1: Prints it
- Given the argument `2`
- When the program runs
- Then it prints `rolled 4` followed by a newline
- And the file `out.txt` holds `4` followed by a newline
- And it exits with code `0`

### For any `n`
- Given the argument `{n}`
- When the program runs
- Then it prints `{n}` followed by a newline
- And the file `out.txt` holds `{n}` followed by a newline
- And it exits with code `0`

| n | s | c | d | runs |
|---|---|---|---|---|
| `int(0, 100)` | `string(0, 10, "abc")` | `choice("x", "y")` | `decimal(0.00, 9.99)` | 200 |
| `int(-5, -1)` | `string(0, 10, "abc")` | `choice("x", "y")` | `decimal(0.00, 9.99)` | |
"#;

    fn marks(edit: impl Fn(&str) -> String) -> Vec<String> {
        let edited = edit(IDEA);
        assert!(crate::idea::parse(&edited).is_ok(), "{edited}");
        tests_less(IDEA, &edited)
    }

    #[test]
    fn marks_each_change_that_tests_less() {
        assert_eq!(marks(str::to_owned), Vec::<String>::new());
        let fewer_runs = marks(|text| text.replace("| 200 |", "| 50 |"));
        assert_eq!(
            fewer_runs,
            ["generated B1 (block 1): fewer runs, 50 instead of 200"]
        );
        let narrower = marks(|text| text.replace("int(0, 100)", "int(1, 100)"));
        assert!(
            narrower[0].contains("n draws a narrower range"),
            "{narrower:?}"
        );
        let shorter =
            marks(|text| text.replace("string(0, 10, \"abc\")", "string(1, 10, \"abc\")"));
        assert!(shorter[0].contains("fewer lengths"), "{shorter:?}");
        let fewer_chars =
            marks(|text| text.replace("string(0, 10, \"abc\")", "string(0, 10, \"ab\")"));
        assert!(
            fewer_chars[0].contains("without the characters \"c\""),
            "{fewer_chars:?}"
        );
        let fewer_choices = marks(|text| text.replace("choice(\"x\", \"y\")", "choice(\"x\")"));
        assert!(fewer_choices[0].contains("without"), "{fewer_choices:?}");
        let coarser = marks(|text| text.replace("decimal(0.00, 9.99)", "decimal(0.0, 9.9)"));
        assert!(!coarser.is_empty(), "{coarser:?}");
        let dropped = marks(|text| {
            text.replace(
                " d | runs |\n|---|---|---|---|---|",
                " runs |\n|---|---|---|---|",
            )
            .replace(" | `decimal(0.00, 9.99)` |", " |")
        });
        assert!(dropped[0].contains("no longer draws d"), "{dropped:?}");
        let rule = marks(|text| {
            text.replace(
                "prints `rolled 4`",
                "prints text matching `rolled {any(1, 6)}`",
            )
        });
        assert_eq!(
            rule,
            ["B1's example: stdout became a rule instead of exact text"]
        );
        let generated_rule =
            marks(|text| text.replacen("prints `{n}`", "prints `{any(0, 100)}`", 1));
        assert!(
            generated_rule[0].contains("stdout became a rule"),
            "{generated_rule:?}"
        );
        let no_file = marks(|text| {
            text.replace(
                "- And the file `out.txt` holds `4` followed by a newline\n",
                "",
            )
        });
        assert_eq!(
            no_file,
            ["B1's example: no longer expects the file out.txt"]
        );
        let removed = marks(|text| {
            let start = text.rfind("| `int(-5, -1)`").expect("second row");
            text[..start].to_owned()
        });
        assert_eq!(removed, ["generated B1 (block 2) was removed"]);
    }

    #[test]
    fn changes_that_test_as_much_or_more_get_no_mark() {
        let edits: [fn(&str) -> String; 4] = [
            // A corrected formula.
            |text| text.replacen("prints `{n}`", "prints `{n + 0}`", 1),
            // A wider range and more runs.
            |text| {
                text.replace("int(0, 100)", "int(-10, 1000)")
                    .replace("| 200 |", "| 400 |")
            },
            // More characters and choices.
            |text| {
                text.replace("string(0, 10, \"abc\")", "string(0, 20, \"abcd\")")
                    .replace("choice(\"x\", \"y\")", "choice(\"x\", \"y\", \"z\")")
            },
            // One more row.
            |text| {
                format!(
                    "{text}| `int(7, 9)` | `string(0, 10, \"abc\")` | `choice(\"x\", \"y\")` | \
                     `decimal(0.00, 9.99)` | |\n"
                )
            },
        ];
        for edit in edits {
            assert_eq!(marks(edit), Vec::<String>::new(), "{}", edit(IDEA));
        }
        assert_eq!(tests_less("not an idea", IDEA), Vec::<String>::new());
    }
}
