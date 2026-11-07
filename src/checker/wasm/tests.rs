//! The template passes; every rule and every memory rule has a planted violation, each found on
//! its line. M3, M5 and M7 hold by construction, so nothing can break them: WebAssembly's locals
//! can't be addressed and nothing is freed (M3), memory and locals start at zero (M5), and a
//! program has no threads, its atomics refused below (M7).

use super::*;
use crate::wasm::text::build;

/// The WebAssembly main template as shipped, which the planted bugs are made from.
const TEMPLATE: &str = include_str!("../../../plugin/skills/compile/assets/main-template.wat");

/// Checks `text` with an idea whose `- Uses:` line names `uses` and whose `- Network:` line says
/// `network`.
fn check_with(text: &str, uses: &[&str], network: &str) -> Report {
    let built = build(text).unwrap_or_else(|problem| panic!("it builds: {problem}"));
    let (uses, network) = crate::idea::constraints(uses, network).expect("valid constraints");
    check(
        text,
        &built,
        &Context {
            uses: &uses,
            network: &network,
        },
    )
}

/// Checks `text` with an idea that names nothing.
fn check_text(text: &str) -> Report {
    check_with(text, &[], "")
}

/// The line number, counting from 1, where `needle` (which may span lines) first starts.
fn line_of(text: &str, needle: &str) -> usize {
    let position = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} isn't in the text"));
    text[..position].matches('\n').count() + 1
}

/// Replaces the first `old` in the template with `new`.
fn with(old: &str, new: &str) -> String {
    assert!(TEMPLATE.contains(old), "{old:?} isn't in the template");
    TEMPLATE.replacen(old, new, 1)
}

/// Asserts a violation of `rule` on `line` whose report line contains `needle`.
fn assert_violation(report: &Report, rule: Rule, line: usize, needle: &str) {
    let found = report.violations.iter().any(|violation| {
        violation.rule == rule
            && violation.line == Some(line)
            && violation.in_file(FILE).contains(needle)
    });
    assert!(
        found,
        "no {rule:?} violation on line {line} containing {needle:?} in {:#?}",
        report.lines()
    );
}

/// Asserts no violation of `rule`.
fn assert_no(report: &Report, rule: Rule) {
    let found: Vec<String> = report
        .violations
        .iter()
        .filter(|violation| violation.rule == rule)
        .map(|violation| violation.in_file(FILE))
        .collect();
    assert!(found.is_empty(), "{rule:?}: {found:#?}");
}

/// The module wast encodes from `text` with nothing validated and no library merged, as a build
/// it gives: each of its functions main.wat's own, each operator on its line.
fn unvalidated(text: &str) -> Built {
    use wast::core::{FuncKind, ModuleField, ModuleKind};
    let mut buffer = wast::parser::ParseBuffer::new(text).expect("it lexes");
    buffer.track_instr_spans(true);
    let mut wat = wast::parser::parse::<wast::Wat>(&buffer).expect("it parses");
    let starts: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(at, _)| at + 1))
        .collect();
    let line = |offset: usize| starts.partition_point(|start| *start <= offset).max(1);
    let spans: Vec<Vec<usize>> = match &wat {
        wast::Wat::Module(module) => match &module.kind {
            ModuleKind::Text(fields) => fields
                .iter()
                .filter_map(|field| match field {
                    ModuleField::Func(func) => match &func.kind {
                        FuncKind::Inline { expression, .. } => Some(
                            expression
                                .instr_spans
                                .as_deref()
                                .unwrap_or_default()
                                .iter()
                                .map(|span| line(span.offset()))
                                .collect(),
                        ),
                        FuncKind::Import(..) => None,
                    },
                    _ => None,
                })
                .collect(),
            ModuleKind::Binary(_) => Vec::new(),
        },
        wast::Wat::Component(_) => Vec::new(),
    };
    let wasm = wat.encode().expect("it encodes");
    let mut lines = std::collections::BTreeMap::new();
    let mut bodies = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
        let Ok(wasmparser::Payload::CodeSectionEntry(body)) = payload else {
            continue;
        };
        let range = body.range();
        let own = spans.get(bodies.len()).cloned().unwrap_or_default();
        bodies.push((range.start, range.end));
        let mut reader = body.get_operators_reader().expect("operators");
        let mut position = 0;
        while !reader.eof() {
            let (_, offset) = reader.read_with_offset().expect("an operator");
            if let Some(line) = own.get(position) {
                lines.insert(offset, *line);
            }
            position += 1;
        }
    }
    Built {
        wasm,
        lines,
        bodies,
        assertions: std::collections::BTreeMap::new(),
        library: Vec::new(),
    }
}

/// Checks a module built with nothing validated.
fn check_unvalidated(text: &str) -> Report {
    let built = unvalidated(text);
    check(
        text,
        &built,
        &Context {
            uses: &[],
            network: &crate::idea::Network::default(),
        },
    )
}

#[test]
fn the_template_passes_every_rule() {
    let report = check_text(TEMPLATE);
    assert!(report.passed(), "{:#?}", report.lines());
    assert_eq!(report.facts.functions, 3);
    assert_eq!(report.facts.assertions, 8);
    assert_eq!(
        report.facts.deepest_path.first().map(String::as_str),
        Some("main")
    );
    assert!(report.facts.max_stack_bytes > 0);
    assert!(report.facts.max_stack_bytes <= memory::MAX_STACK_BYTES);
    assert_eq!(report.facts.regions.len(), 7);
    // What args_get writes is sized when the program runs: the proof holds it, and nothing else
    // is left to it.
    assert_eq!(
        report.facts.obligations.len(),
        2,
        "{:#?}",
        report.facts.obligations
    );
    assert!(
        report
            .facts
            .obligations
            .iter()
            .all(|obligation| obligation.what.contains("$args_get")),
        "{:#?}",
        report.facts.obligations
    );
    assert!(report.unsafe_blocks.is_empty());
}

/// Rule 4's limit, the native dialect's 60 instructions at three operators each, is above what
/// the template needs: its functions take 81, 108 and 100 operators where the native template's
/// take 39, 40 and 48 instructions, 2.1 to 2.7 operators an instruction.
#[test]
fn rule_4s_limit_is_calibrated_on_the_template() {
    let built = build(TEMPLATE).expect("it builds");
    let module = model::Module::read(TEMPLATE, &built).expect("it reads");
    let counts: Vec<(String, usize)> = module
        .own()
        .map(|function| {
            (
                function.name.clone().unwrap_or_default(),
                function.body.len() - 1,
            )
        })
        .collect();
    assert_eq!(
        counts,
        [
            ("main".to_owned(), 81),
            ("parse_i64".to_owned(), 108),
            ("write_i64".to_owned(), 100)
        ]
    );
    assert!(
        counts
            .iter()
            .all(|(_, count)| *count <= rules::MAX_OPERATORS)
    );
}

#[test]
fn reports_name_main_wat_s_lines() {
    let violation = Violation {
        rule: Rule::Functions,
        line: Some(12),
        problem: "$main has 200 operators".into(),
        fix: "split it".into(),
    };
    assert_eq!(
        violation.in_file(FILE),
        "rule 4 (small functions) main.wat:12: $main has 200 operators. Fix: split it."
    );
    // The native checker's reports keep naming main.s.
    assert_eq!(
        violation.to_string(),
        "rule 4 (small functions) main.s:12: $main has 200 operators. Fix: split it."
    );
    let memory = Violation {
        rule: Rule::Immutability,
        line: None,
        problem: "a write".into(),
        fix: "don't".into(),
    };
    assert_eq!(
        memory.in_file(FILE),
        "rule M6 (immutability): a write. Fix: don't."
    );
}

#[test]
fn rule_1_refuses_recursion() {
    let text = with(
        "    (local.set $status (global.get $ERROR_INVALID))\n",
        "    (local.set $status (global.get $ERROR_INVALID))\n    (drop (call $parse_i64 \
         (local.get $at) (local.get $into)))\n",
    );
    let report = check_text(&text);
    assert_violation(
        &report,
        Rule::ControlFlow,
        line_of(&text, "(drop (call $parse_i64"),
        "recursion",
    );
}

#[test]
fn rule_1_keeps_unreachable_for_the_end_of_a_path() {
    let stray = with(
        "    (local.set $start (local.get $i))\n",
        "    (local.set $start (local.get $i))\n    unreachable\n",
    );
    let report = check_text(&stray);
    assert_violation(
        &report,
        Rule::ControlFlow,
        line_of(&stray, "    unreachable\n    (block $invalid"),
        "may only follow a call that never returns",
    );
    let missing = with(
        "        (call $proc_exit (global.get $EXIT_USAGE))\n        unreachable)",
        "        (call $proc_exit (global.get $EXIT_USAGE)))",
    );
    let report = check_text(&missing);
    assert_violation(
        &report,
        Rule::ControlFlow,
        line_of(&missing, "(call $proc_exit (global.get $EXIT_USAGE))"),
        "no `unreachable` follows it",
    );
}

#[test]
fn rule_1_sends_a_br_tables_default_to_an_assertion() {
    let open = with(
        "    (local.set $start (local.get $i))\n",
        "    (local.set $start (local.get $i))\n    (block $one (block $zero (br_table $zero \
         $one (local.get $i))))\n",
    );
    let report = check_text(&open);
    assert_violation(
        &report,
        Rule::ControlFlow,
        line_of(&open, "(br_table"),
        "default",
    );
    let guarded = with(
        "    (local.set $start (local.get $i))\n",
        "    (local.set $start (local.get $i))\n    (block $ok\n      (block $bad\n        \
         (block $zero (br_table $zero $bad (local.get $i)))\n        (br $ok))\n      (call \
         $assert_fail (i32.const 9))\n      unreachable)\n",
    );
    let report = check_text(&guarded);
    assert_no(&report, Rule::ControlFlow);
    assert_no(&report, Rule::Assertions);
}

#[test]
fn rule_2_counts_every_loop() {
    let unbounded = with("(loop $digit  ;; bound: 20", "(loop $digit");
    let report = check_text(&unbounded);
    assert_violation(
        &report,
        Rule::Loops,
        line_of(&unbounded, "(loop $digit\n"),
        "no `;; bound: N`",
    );
    let bare = with(
        "(local.set $left (global.get $MAX_CHARS))",
        "(local.set $left (i32.const 20))",
    );
    let report = check_text(&bare);
    assert_violation(
        &report,
        Rule::Loops,
        line_of(&bare, "(loop $digit  ;; bound: 20"),
        "isn't set from a named constant",
    );
    let past = with("(loop $digit  ;; bound: 20", "(loop $digit  ;; bound: 10");
    let report = check_text(&past);
    assert_violation(
        &report,
        Rule::Loops,
        line_of(&past, "(loop $digit  ;; bound: 10"),
        "outside 1 to the bound 10",
    );
    let uncounted = with(
        "          (local.set $left (i32.sub (local.get $left) (i32.const 1)))\n          \
         (br_if $digit (local.get $left)))\n        ;; More characters",
        "          (br $digit))\n        ;; More characters",
    );
    let report = check_text(&uncounted);
    assert_violation(
        &report,
        Rule::Loops,
        line_of(&uncounted, "(br $digit))"),
        "doesn't end by taking one from its counter",
    );
    let changed = with(
        "          (br_if $end (i32.eqz (local.get $c)))\n",
        "          (br_if $end (i32.eqz (local.get $c)))\n          (local.set $left (global.get \
         $MAX_CHARS))\n",
    );
    let report = check_text(&changed);
    assert_violation(
        &report,
        Rule::Loops,
        line_of(
            &changed,
            "          (local.set $left (global.get $MAX_CHARS))\n",
        ),
        "changes the loop counter $left",
    );
    let backward = with(
        "          (br_if $end (i32.eqz (local.get $c)))\n",
        "          (br_if $end (i32.eqz (local.get $c)))\n          (br_if $digit (i32.eqz \
         (local.get $left)))\n",
    );
    let report = check_text(&backward);
    assert_violation(
        &report,
        Rule::Loops,
        line_of(&backward, "(br_if $digit (i32.eqz"),
        "the only backward branch",
    );
    let forever = with(
        "(loop $digit  ;; bound: 20",
        "(loop $digit  ;; bound: forever",
    );
    let report = check_text(&forever);
    assert_violation(
        &report,
        Rule::Loops,
        line_of(&forever, "(loop $digit  ;; bound: forever"),
        "only a server's loop",
    );
}

#[test]
fn rule_3_reserves_resources_up_front() {
    let import = with(
        "  (memory (export \"memory\") 1 1)",
        "  (import \"wasi_snapshot_preview1\" \"random_get\" (func $random_get (param i32 i32) \
         (result i32)))\n  (memory (export \"memory\") 1 1)",
    );
    let report = check_text(&import);
    assert_violation(
        &report,
        Rule::Resources,
        line_of(&import, "(func $random_get"),
        "needs `random bytes` in its `- Uses:` line",
    );
    // An idea that names it allows it.
    let report = check_with(&import, &["random bytes"], "");
    assert_no(&report, Rule::Resources);
    let growing = with(
        "(memory (export \"memory\") 1 1)",
        "(memory (export \"memory\") 1 2)",
    );
    let report = check_text(&growing);
    assert_violation(
        &report,
        Rule::Resources,
        line_of(&growing, "(memory (export"),
        "must be equal",
    );
    let grows = with(
        "    (local.set $magnitude (local.get $value))\n",
        "    (local.set $magnitude (local.get $value))\n    (drop (memory.grow (i32.const 0)))\n",
    );
    let report = check_text(&grows);
    assert_violation(
        &report,
        Rule::Resources,
        line_of(&grows, "(drop (memory.grow"),
        "`memory.grow`",
    );
    // lengua's $fill reads, which needs an idea that says so.
    let fill = with(
        "          (local.set $n (i64.load (global.get $number)))\n",
        "          (local.set $n (i64.load (global.get $number)))\n          (br_if $write_failed \
         (i32.lt_s (call $fill (global.get $STDERR) (global.get $number) (global.get \
         $number.size) (global.get $NO_ERROR)) (global.get $NO_ERROR)))\n",
    );
    let report = check_text(&fill);
    assert_violation(
        &report,
        Rule::Resources,
        line_of(&fill, "(i32.lt_s (call $fill"),
        "calls `fd_read`",
    );
    let report = check_with(&fill, &["standard input"], "");
    assert_no(&report, Rule::Resources);
}

#[test]
fn rule_4_keeps_functions_small_with_one_end() {
    let early = with(
        "      (local.set $status (global.get $NO_ERROR)))\n",
        "      (local.set $status (global.get $NO_ERROR))\n      (return (local.get $status)))\n",
    );
    let report = check_text(&early);
    assert_violation(
        &report,
        Rule::Functions,
        line_of(&early, "(return (local.get $status))"),
        "`return` leaves $parse_i64 before its end",
    );
    let padding = "    (drop (i32.const 0))\n".repeat(40);
    let large = with(
        "    (local.set $status (global.get $ERROR_INVALID))\n",
        &format!("    (local.set $status (global.get $ERROR_INVALID))\n{padding}"),
    );
    let report = check_text(&large);
    assert_violation(
        &report,
        Rule::Functions,
        line_of(&large, "(func $parse_i64"),
        "$parse_i64 has 188 operators",
    );
}

#[test]
fn rule_5_holds_assertions_to_their_shape() {
    let always = with(
        "    (local.set $start (local.get $i))\n",
        "    (local.set $start (local.get $i))\n    (call $assert_fail (i32.const 9))\n    \
         unreachable\n",
    );
    let report = check_text(&always);
    assert_violation(
        &report,
        Rule::Assertions,
        line_of(&always, "(call $assert_fail (i32.const 9))"),
        "isn't reached only when a check fails",
    );
    let twice = with(
        "(call $assert_fail (i32.const 4))",
        "(call $assert_fail (i32.const 3))",
    );
    let report = check_text(&twice);
    // The second 3, in the loop, is the one used twice.
    assert_violation(
        &report,
        Rule::Assertions,
        line_of(
            &twice,
            "(then (call $assert_fail (i32.const 3)) unreachable))\n          (local.set $c",
        ),
        "used twice",
    );
    let kept = with(
        "(call $assert_fail (i32.const 8))",
        "(call $assert_fail (i32.const 901))",
    );
    let report = check_text(&kept);
    assert_violation(
        &report,
        Rule::Assertions,
        line_of(&kept, "(i32.const 901)"),
        "one lengua keeps",
    );
    let few = "(module\n  (import \"wasi_snapshot_preview1\" \"proc_exit\" (func $proc_exit (param \
               i32)))\n  (memory (export \"memory\") 1 1)\n  (global $EXIT_OK i32 (i32.const \
               0))\n  (func $main (export \"_start\")\n    (call $proc_exit (global.get \
               $EXIT_OK))\n    unreachable))\n";
    let report = check_text(few);
    assert_violation(
        &report,
        Rule::Assertions,
        line_of(few, "(func $main"),
        "0 assertions in 1 functions",
    );
}

#[test]
fn rule_6_keeps_data_in_the_smallest_scope() {
    let changing = with(
        "  (global $NINE i32 (i32.const 9))\n",
        "  (global $NINE i32 (i32.const 9))\n  (global $count (mut i32) (i32.const 0))\n",
    );
    let report = check_text(&changing);
    assert_violation(
        &report,
        Rule::Scope,
        line_of(&changing, "(global $count"),
        "no `;; shared: <reason>`",
    );
    let unshared = with(
        "  ;; shared: main has the host fill it; parse_i64 reads it",
        "",
    );
    let report = check_text(&unshared);
    assert_violation(
        &report,
        Rule::Scope,
        line_of(&unshared, "(global $args i32"),
        "$args is used by $main, $parse_i64",
    );
    let exported = with(
        "  (memory (export \"memory\") 1 1)",
        "  (memory (export \"memory\") 1 1)\n  (export \"extra\" (func $write_i64))",
    );
    let report = check_text(&exported);
    assert_violation(
        &report,
        Rule::Scope,
        line_of(&exported, "(export \"extra\""),
        "exports `extra`",
    );
}

#[test]
fn rule_7_tests_every_result() {
    let dropped = with(
        "(local.set $err (call $args_get (global.get $argv) (global.get $args)))",
        "(drop (call $args_get (global.get $argv) (global.get $args)))",
    );
    let report = check_text(&dropped);
    assert_violation(
        &report,
        Rule::Results,
        line_of(&dropped, "(drop (call $args_get"),
        "$args_get's error number isn't tested: it's dropped",
    );
    let late = with(
        "          (br_if $not_a_number (local.get $err))\n",
        "          (local.set $n (i64.load (global.get $number)))\n          (br_if \
         $not_a_number (local.get $err))\n",
    );
    let report = check_text(&late);
    assert_violation(
        &report,
        Rule::Results,
        line_of(&late, "(call $parse_i64"),
        "$parse_i64's result isn't tested",
    );
}

#[test]
fn rule_8_names_every_constant() {
    let bare = with(
        "(call $proc_exit (global.get $EXIT_OK))",
        "(call $proc_exit (i32.const 0))",
    );
    let report = check_text(&bare);
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&bare, "(call $proc_exit (i32.const 0))"),
        "the bare number 0",
    );
    let assertion = with(
        "(global $EXIT_WRITE_FAILED i32 (i32.const 3))",
        "(global $EXIT_WRITE_FAILED i32 (i32.const 134))",
    );
    let report = check_text(&assertion);
    assert_violation(
        &report,
        Rule::Constants,
        line_of(
            &assertion,
            "(call $proc_exit (global.get $EXIT_WRITE_FAILED))",
        ),
        "134 is the exit status of a failed assertion",
    );
    let descriptor = with(
        "(call $write_all (global.get $STDOUT)",
        "(call $write_all (i32.const 1)",
    );
    let report = check_text(&descriptor);
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&descriptor, "(call $write_all (i32.const 1)"),
        "descriptor 1 is a bare number",
    );
    let bound = with(
        "(i32.gt_u (local.get $size) (global.get $args.size))",
        "(i32.gt_u (local.get $size) (i32.const 64))",
    );
    let report = check_text(&bound);
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&bound, "(i32.const 64))"),
        "the bound 64 is a bare number",
    );
    let size = with(
        "(global $nan.rosize i32 (i32.const 20))",
        "(global $nan.rosize i32 (i32.const 24))",
    );
    let report = check_text(&size);
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&size, "(global $nan.rosize"),
        "$nan.rosize is 24, but the data in $nan holds 20 bytes",
    );
    let falls = with(
        "    (call $proc_exit (global.get $EXIT_WRITE_FAILED))\n    unreachable)",
        "    )",
    );
    let report = check_text(&falls);
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&falls, "(func $main"),
        "`_start` can reach its end",
    );
}

#[test]
fn rule_9_checks_every_computed_index() {
    let check = "          (if (i32.ge_u (local.get $i) (global.get $args.size))\n            \
                 (then (call $assert_fail (i32.const 4)) unreachable))\n";
    let unchecked = with(check, "");
    let report = check_text(&unchecked);
    assert_violation(
        &report,
        Rule::Indirection,
        line_of(&unchecked, "(local.set $c (i32.load8_u"),
        "indexes $args with $i, which isn't bounds-checked first",
    );
    let loose = with(
        check,
        "          (if (i32.ge_u (local.get $i) (i32.add (global.get $args.size) (global.get \
         $STDOUT)))\n            (then (call $assert_fail (i32.const 4)) unreachable))\n",
    );
    let report = check_text(&loose);
    assert_violation(
        &report,
        Rule::Indirection,
        line_of(&loose, "(local.set $c (i32.load8_u"),
        "reaches byte 64 of $args, which has 64 bytes",
    );
    // An unsafe block lets the check go, and is reported; the proof holds the access.
    let trusted = with(
        check,
        "          ;; unsafe: args_get ends every argument with a NUL inside $args\n",
    );
    let trusted = trusted.replacen(
        "          (br_if $end (i32.eqz (local.get $c)))\n",
        "          ;; end unsafe\n          (br_if $end (i32.eqz (local.get $c)))\n",
        1,
    );
    let report = check_text(&trusted);
    assert_no(&report, Rule::Indirection);
    assert_eq!(report.unsafe_blocks.len(), 1);
    assert!(
        report
            .facts
            .obligations
            .iter()
            .any(|obligation| obligation.what.contains("unsafe block")),
        "{:#?}",
        report.facts.obligations
    );
    let unclosed = with(
        "    (local.set $start (local.get $i))\n",
        "    ;; unsafe: nothing to see\n    (local.set $start (local.get $i))\n",
    );
    let report = check_text(&unclosed);
    assert_violation(
        &report,
        Rule::Indirection,
        line_of(&unclosed, ";; unsafe: nothing to see"),
        "has no `;; end unsafe`",
    );
    let table = with(
        "  (memory (export \"memory\") 1 1)",
        "  (memory (export \"memory\") 1 1)\n  (table 1 funcref)",
    );
    let report = check_text(&table);
    assert_violation(
        &report,
        Rule::Indirection,
        line_of(&table, "(table 1 funcref)"),
        "a table",
    );
    let start = with(
        "  (memory (export \"memory\") 1 1)",
        "  (memory (export \"memory\") 1 1)\n  (start $main)",
    );
    let report = check_text(&start);
    assert_violation(
        &report,
        Rule::Indirection,
        line_of(&start, "(start $main)"),
        "a start function",
    );
}

#[test]
fn rule_10_takes_the_validator_and_the_lints_without_warnings() {
    let spare = with(
        "  ;; Writes $value in decimal",
        "  (func $spare (result i32) (global.get $NO_ERROR))\n\n  ;; Writes $value in decimal",
    );
    let report = check_text(&spare);
    assert_violation(
        &report,
        Rule::Tools,
        line_of(&spare, "(func $spare"),
        "$spare is never called",
    );
    let unread = with(
        "    (local $magnitude i64)\n",
        "    (local $magnitude i64)\n    (local $unread i32)\n",
    );
    let report = check_text(&unread);
    assert_violation(
        &report,
        Rule::Tools,
        line_of(&unread, "(func $write_i64"),
        "$write_i64's local $unread is never read",
    );
    let dead = with(
        "        (br $invalid))\n",
        "        (br $invalid)\n        (local.set $c (global.get $NO_ERROR)))\n",
    );
    let report = check_text(&dead);
    assert_violation(
        &report,
        Rule::Tools,
        line_of(&dead, "        (local.set $c (global.get $NO_ERROR)))\n"),
        "never runs",
    );
    // A module the engine's features don't take, though wast encodes it.
    let report = check_unvalidated(
        "(module (memory (export \"memory\") 1 1) (memory 1 1) (func (export \"_start\")))",
    );
    assert!(
        report
            .violations
            .iter()
            .any(|violation| violation.rule == Rule::Tools
                && violation
                    .problem
                    .contains("the validator refuses the module")),
        "{:#?}",
        report.lines()
    );
}

#[test]
fn m1_makes_addresses_only_from_regions_placed_apart() {
    let plain = with(
        "    (local.set $magnitude (local.get $value))\n",
        "    (local.set $magnitude (local.get $value))\n    (i32.store8 (i32.const 2160) \
         (global.get $NEWLINE))\n",
    );
    let report = check_text(&plain);
    assert_violation(
        &report,
        Rule::Owners,
        line_of(&plain, "(i32.store8 (i32.const 2160)"),
        "a plain number",
    );
    let low = with(
        "  (global $sizes i32 (i32.const 2048))",
        "  (global $sizes i32 (i32.const 512))",
    );
    let report = check_text(&low);
    assert_violation(
        &report,
        Rule::Owners,
        line_of(&low, "(global $sizes i32"),
        "inside lengua's addresses 0 to 1023",
    );
    let overlapping = with(
        "  (global $argv i32 (i32.const 2064))",
        "  (global $argv i32 (i32.const 2052))",
    );
    let report = check_text(&overlapping);
    assert_violation(
        &report,
        Rule::Owners,
        line_of(&overlapping, "(global $argv i32"),
        "inside $sizes",
    );
    let outside = with(
        "  (global $out i32 (i32.const 2160))",
        "  (global $out i32 (i32.const 65530))",
    );
    let report = check_text(&outside);
    assert_violation(
        &report,
        Rule::Owners,
        line_of(&outside, "(global $out i32"),
        "past the memory's 65536 bytes",
    );
    let orphan = with(
        "  (global $out.size i32 (i32.const 21))\n",
        "  (global $out.size i32 (i32.const 21))\n  (global $nowhere.size i32 (i32.const 4))\n",
    );
    let report = check_text(&orphan);
    assert_violation(
        &report,
        Rule::Owners,
        line_of(&orphan, "(global $nowhere.size"),
        "$nowhere.size sizes $nowhere, which isn't a global",
    );
}

#[test]
fn m2_keeps_every_access_and_borrowed_buffer_in_its_region() {
    let past = with(
        "    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))\n",
        "    (i32.store8 (i32.add (global.get $out) (global.get $out.size)) (global.get \
         $NEWLINE))\n",
    );
    let report = check_text(&past);
    assert_violation(
        &report,
        Rule::Bounds,
        line_of(&past, "(i32.add (global.get $out) (global.get $out.size))"),
        "reaches byte 21 of $out, which has 21 bytes",
    );
    // A 16-byte vector at the end of a region reaches past it.
    let wide = with(
        "    (local.set $magnitude (local.get $value))\n",
        "    (local.set $magnitude (local.get $value))\n    (v128.store offset=8 (global.get \
         $out) (v128.const i32x4 0 0 0 0))\n",
    );
    let report = check_text(&wide);
    assert_violation(
        &report,
        Rule::Bounds,
        line_of(&wide, "(v128.store offset=8"),
        "reaches byte 23 of $out",
    );
    let borrowed = with(
        "(call $write_all (global.get $STDERR) (global.get $usage) (global.get $usage.rosize))",
        "(call $write_all (global.get $STDERR) (global.get $usage) (global.get $nan.rosize))",
    );
    let report = check_text(&borrowed);
    assert_violation(
        &report,
        Rule::Bounds,
        line_of(&borrowed, "(global.get $nan.rosize))"),
        "reaches byte 19 of $usage, which has 18 bytes",
    );
}

#[test]
fn m4_lends_a_written_region_once() {
    let twice = with(
        "(call $args_sizes_get (global.get $sizes) (i32.add (global.get $sizes) (i32.const 4)))",
        "(call $args_sizes_get (global.get $sizes) (global.get $sizes))",
    );
    let report = check_text(&twice);
    assert_violation(
        &report,
        Rule::ExclusiveWrites,
        line_of(
            &twice,
            "(call $args_sizes_get (global.get $sizes) (global.get $sizes))",
        ),
        "may name the same bytes",
    );
}

#[test]
fn m6_never_writes_read_only_memory() {
    let written = with(
        "    (local.set $magnitude (local.get $value))\n",
        "    (local.set $magnitude (local.get $value))\n    (i32.store8 (global.get $usage) \
         (global.get $NEWLINE))\n",
    );
    let report = check_text(&written);
    assert_violation(
        &report,
        Rule::Immutability,
        line_of(&written, "(i32.store8 (global.get $usage)"),
        "writes to $usage, a read-only region",
    );
    let data = with(
        "  (global $out.size i32 (i32.const 21))\n",
        "  (global $out.size i32 (i32.const 21))\n  (data (i32.const 2160) \"-\")\n",
    );
    let report = check_text(&data);
    assert_violation(
        &report,
        Rule::Immutability,
        line_of(&data, "(data (i32.const 2160)"),
        "isn't inside a read-only region",
    );
}

#[test]
fn m8_keeps_the_deepest_call_path_within_the_engines_stack() {
    let locals = format!("    (local {})\n", "i64 ".repeat(17_000));
    let deep = with(
        "    (local $magnitude i64)\n",
        &format!("    (local $magnitude i64)\n{locals}"),
    );
    let report = check_text(&deep);
    assert_violation(
        &report,
        Rule::StackSize,
        line_of(&deep, "(func $main"),
        "the deepest call path, main → write_i64",
    );
}

#[test]
fn a_reserved_name_is_lenguas() {
    let defined = with(
        "  ;; Writes $value in decimal",
        "  (func $write_all (param i32 i32 i32) (result i32) (global.get $NO_ERROR))\n\n  ;; \
         Writes $value in decimal",
    );
    let report = check_text(&defined);
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(&defined, "(func $write_all"),
        "a name lengua keeps",
    );
}

#[test]
fn annotations_are_refused() {
    let named = with(
        "  (global $NINE i32 (i32.const 9))",
        "  (global $NINE (@name \"TEN\") i32 (i32.const 9))",
    );
    let report = check_text(&named);
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(&named, "(@name"),
        "annotation",
    );
}

#[test]
fn every_computer_computes_the_same_bits() {
    let trapping = with(
        "    (local.set $magnitude (local.get $value))\n",
        "    (local.set $magnitude (local.get $value))\n    (local.set $magnitude \
         (i64.trunc_f64_s (f64.const 1.5)))\n",
    );
    let report = check_text(&trapping);
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(&trapping, "(i64.trunc_f64_s"),
        "use `i64.trunc_sat_f64_s`",
    );
    let saturating = trapping.replacen("i64.trunc_f64_s", "i64.trunc_sat_f64_s", 1);
    assert_no(&check_text(&saturating), Rule::Dialect);
    let nan = with(
        "    (local.set $magnitude (local.get $value))\n",
        "    (local.set $magnitude (local.get $value))\n    (local.set $magnitude \
         (i64.reinterpret_f64 (f64.div (f64.const 0) (f64.const 0))))\n",
    );
    let report = check_text(&nan);
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(&nan, "(i64.reinterpret_f64"),
        "a float that may be NaN",
    );
    // A float the code checked isn't NaN shows its bits as they are on every computer.
    let checked = with(
        "    (local $left i32)\n    (local.set $magnitude (local.get $value))\n",
        "    (local $left i32)\n    (local $x f64)\n    (local.set $magnitude (local.get \
         $value))\n    (local.set $x (f64.div (f64.const 0) (f64.const 0)))\n    (if (f64.eq \
         (local.get $x) (local.get $x))\n      (then (local.set $magnitude (i64.reinterpret_f64 \
         (local.get $x)))))\n",
    );
    assert_no(&check_text(&checked), Rule::Dialect);
    // Relaxed SIMD and atomics, which the engine's validator refuses first.
    let relaxed = "(module\n  (memory (export \"memory\") 1 1)\n  (func (export \"_start\")\n    \
                   (drop (i32x4.relaxed_trunc_f32x4_s (v128.const f32x4 0 0 0 0)))))\n";
    let report = check_unvalidated(relaxed);
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(relaxed, "(i32x4.relaxed_trunc_f32x4_s"),
        "relaxed SIMD",
    );
    let atomic = "(module\n  (memory (export \"memory\") 1 1)\n  (func (export \"_start\")\n    \
                  (drop (i32.atomic.load (i32.const 1024)))))\n";
    let report = check_unvalidated(atomic);
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(atomic, "(i32.atomic.load"),
        "atomic",
    );
}

#[test]
fn every_division_is_guarded() {
    let unguarded = with(
        "          (local.set $err (call $args_get (global.get $argv) (global.get $args)))\n",
        "          (local.set $err (i32.div_u (global.get $args.size) (local.get $size)))\n          \
         (local.set $err (call $args_get (global.get $argv) (global.get $args)))\n",
    );
    let report = check_text(&unguarded);
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(&unguarded, "(i32.div_u"),
        "nothing checks isn't zero",
    );
    let guarded = unguarded.replacen(
        "          (local.set $err (i32.div_u",
        "          (br_if $not_a_number (i32.eqz (local.get $size)))\n          (local.set $err \
         (i32.div_u",
        1,
    );
    let report = check_text(&guarded);
    assert_no(&report, Rule::Dialect);
    // A signed division by a value checked only not to be zero may divide the most negative
    // number by -1: the proof's to rule out.
    let signed = with(
        "          (local.set $size (i32.load offset=4 (global.get $sizes)))\n",
        "          (local.set $size (i32.load offset=4 (global.get $sizes)))\n          (br_if \
         $not_a_number (i32.eqz (local.get $size)))\n          (local.set $err (i32.div_s \
         (i32.load (global.get $sizes)) (local.get $size)))\n",
    );
    let report = check_text(&signed);
    assert_no(&report, Rule::Dialect);
    assert!(
        report
            .facts
            .obligations
            .iter()
            .any(|obligation| obligation.what.contains("`i32.div_s` never divides")),
        "{:#?}",
        report.facts.obligations
    );
}
