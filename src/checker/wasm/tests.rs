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
    check_secret(text, uses, network, &[])
}

/// Checks `text` with an idea whose `- Uses:` line names `uses`, whose `- Network:` line says
/// `network`, and whose `- Secret:` line names `secrets`.
fn check_secret(text: &str, uses: &[&str], network: &str, secrets: &[Source]) -> Report {
    let built = build(text).unwrap_or_else(|problem| panic!("it builds: {problem}"));
    let (uses, network) = crate::idea::constraints(uses, network).expect("valid constraints");
    check(
        text,
        &built,
        &Context {
            uses: &uses,
            network: &network,
            secrets,
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
/// it gives: each of its functions main.wat's own, each operator on its line, and its names in
/// both of its modules.
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
        named: wasm.clone(),
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
            secrets: &[],
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
    // What args_get writes is sized when the program runs, and the template's checks of the
    // two sizes hold it to the regions: nothing is left to the proof.
    assert!(
        report.facts.obligations.is_empty(),
        "{:#?}",
        report.facts.obligations
    );
    assert!(report.unsafe_blocks.is_empty());
}

/// Rule 4's limit, the native lengua's 60 instructions a function at three operators each, is
/// above what the template needs: its functions take 81, 108 and 100 operators where the native
/// lengua's template took 39, 40 and 48 instructions, 2.1 to 2.7 operators an instruction.
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
    // Shown on its own, it names main.wat's line too.
    assert_eq!(violation.to_string(), violation.in_file(FILE));
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
        // The planted use, not the declaration of $args.size, which holds the same text.
        line_of(&bound, "(i32.gt_u (local.get $size) (i32.const 64))"),
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
    // -1 is a bound like any other number.
    let minus_one = with(
        "(i32.gt_u (local.get $size) (global.get $args.size))",
        "(i32.gt_u (local.get $size) (i32.const -1))",
    );
    let report = check_text(&minus_one);
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&minus_one, "(i32.const -1))"),
        "the bound -1 is a bare number",
    );
    // An error number is named, as references/host.md names it.
    let error = with(
        "          (if (local.get $err)\n            (then (call $assert_fail (i32.const 2))",
        "          (if (i32.eq (local.get $err) (i32.const 8))\n            (then (call \
         $assert_fail (i32.const 2))",
    );
    let report = check_text(&error);
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&error, "(if (i32.eq (local.get $err) (i32.const 8))"),
        "the error number 8 is a bare number",
    );
    // So is a host call's mode, here the clock's.
    let clock = with(
        "  (memory (export \"memory\") 1 1)",
        "  (import \"wasi_snapshot_preview1\" \"clock_time_get\"\n    (func $clock_time_get \
         (param i32 i64 i32) (result i32)))\n  (memory (export \"memory\") 1 1)",
    );
    let clock = clock.replacen(
        "          (local.set $n (i64.load (global.get $number)))\n",
        "          (local.set $err (call $clock_time_get (i32.const 1) (i64.const 0) (global.get \
         $number)))\n          (br_if $not_a_number (local.get $err))\n          (local.set $n \
         (i64.load (global.get $number)))\n",
        1,
    );
    let report = check_with(&clock, &["the clock"], "");
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&clock, "(call $clock_time_get"),
        "the mode 1 is a bare number",
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
        "          (local.set $err (i32.div_u (global.get $args.size) (local.get \
         $size)))\n          (local.set $err (call $args_get (global.get $argv) (global.get \
         $args)))\n",
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

/// A program that reads a secret from standard input, for the secret rule's tests: lengua's
/// library reads it into `$text`, and `;; plant` is where each test puts its code. Only the
/// secret rule's findings are the tests': the other rules' in it aren't.
const SECRET: &str = r#"(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $EXIT_OK i32 (i32.const 0))
  (global $READ_TO_END i32 (i32.const -1))
  (global $NEWLINE i32 (i32.const 10))
  (global $kept (mut i32) (i32.const 0))
  (global $text i32 (i32.const 1024))
  (global $text.size i32 (i32.const 64))
  (global $out i32 (i32.const 2048))
  (global $out.size i32 (i32.const 8))
  (global $iovec i32 (i32.const 3072))
  (global $iovec.size i32 (i32.const 16))
  (func $main (export "_start")
    (local $got i32)
    (local $byte i32)
    (local $word i32)
    (local.set $got
      (call $fill (global.get $STDIN) (global.get $text) (global.get $text.size)
        (global.get $READ_TO_END)))
    (local.set $byte (i32.load8_u (global.get $text)))
    ;; plant
    (i32.store8 (global.get $out) (local.get $byte))
    (drop (call $write_all (global.get $STDOUT) (global.get $out) (global.get $out.size)))
    (call $proc_exit (global.get $EXIT_OK))
    unreachable)
  (func $branch (param $value i32)
    (if (local.get $value) (then (nop))))
  (func $same (param $value i32) (result i32)
    (local.get $value))
  (func $first (param $at i32) (result i32)
    (i32.load8_u (local.get $at))))
"#;

/// [`SECRET`] with `code` where it says `;; plant`, checked with standard input secret, or
/// with nothing secret when `secret` is false.
fn planted(code: &str, secret: bool) -> (String, Report) {
    let text = SECRET.replacen("    ;; plant\n", &format!("    {code}\n"), 1);
    let secrets = if secret {
        vec![Source::StandardInput]
    } else {
        Vec::new()
    };
    let report = check_secret(&text, &["standard input"], "", &secrets);
    (text, report)
}

/// The lines the secret rule refuses in `report`.
fn secret_lines(report: &Report) -> Vec<usize> {
    report
        .violations
        .iter()
        .filter(|violation| violation.rule == Rule::Secrets)
        .filter_map(|violation| violation.line)
        .collect()
}

/// The secret rule refuses each way a value computed from a secret can decide how the program
/// runs, on its line: a branch, a loop's count, a `select`, an address, a length, a division, a
/// float operator, a host call's argument, a buffer a host call reads, and `$fill`'s stop byte;
/// and a call it can't follow.
#[test]
fn the_secret_rule_refuses_every_way_a_secret_decides_how_the_program_runs() {
    let refused = [
        (
            "(if (local.get $byte) (then (nop)))",
            "the condition of this `if` is computed from a secret",
        ),
        (
            "(block $b (br_if $b (local.get $byte)))",
            "the condition of this `br_if` is computed from a secret",
        ),
        (
            "(block $a (block $b (br_table $a $b (local.get $byte))))",
            "the condition of this `br_table` is computed from a secret",
        ),
        (
            "(block $done (loop $again (br_if $done (local.get $byte)) (br $again)))",
            "this `br_if` repeats or leaves the loop on line",
        ),
        (
            "(local.set $word (select (i32.const 1) (i32.const 2) (local.get $byte)))",
            "the condition of this `select` is computed from a secret",
        ),
        (
            "(local.set $word (i32.load8_u (i32.add (global.get $text) (local.get $byte))))",
            "the address of this `i32.load8_u` is computed from a secret",
        ),
        (
            "(i32.store8 (i32.add (global.get $out) (local.get $byte)) (i32.const 0))",
            "the address of this `i32.store8` is computed from a secret",
        ),
        (
            "(memory.copy (global.get $out) (global.get $text) (local.get $byte))",
            "the length of this `memory.copy` is computed from a secret",
        ),
        (
            "(memory.fill (i32.add (global.get $out) (local.get $byte)) (i32.const 0) (i32.const \
             1))",
            "the address of this `memory.fill` is computed from a secret",
        ),
        (
            "(local.set $word (i32.div_u (i32.const 100) (local.get $byte)))",
            "an operand of this `i32.div_u` is computed from a secret",
        ),
        (
            "(local.set $word (i32.rem_s (local.get $byte) (i32.const 7)))",
            "an operand of this `i32.rem_s` is computed from a secret",
        ),
        (
            "(drop (f64.convert_i32_u (local.get $byte)))",
            "an operand of this `f64.convert_i32_u` is computed from a secret",
        ),
        (
            "(drop (call $write_all (local.get $byte) (global.get $out) (global.get $out.size)))",
            "argument 1 of this call of $write_all is computed from a secret",
        ),
        (
            "(drop (call $write_all (global.get $STDOUT) (global.get $out) (local.get $byte)))",
            "argument 3 of this call of $write_all is computed from a secret",
        ),
        (
            "(i32.store (global.get $iovec) (global.get $out)) (i32.store offset=4 (global.get \
             $iovec) (local.get $byte)) (drop (call $fd_write (global.get $STDOUT) (global.get \
             $iovec) (i32.const 1) (i32.add (global.get $iovec) (i32.const 8))))",
            "$fd_write reads a secret in the buffer of argument 2",
        ),
        (
            "(local.set $got (call $fill (global.get $STDIN) (global.get $text) (global.get \
             $text.size) (global.get $NEWLINE)))",
            "`$fill` reads a secret here up to its stop byte",
        ),
        // What the rule can't follow it refuses too, as rule 1 does recursion.
        (
            "(call $main)",
            "the secret rule can't follow a call that leads back to its caller",
        ),
    ];
    for (code, needle) in refused {
        let (text, report) = planted(code, true);
        assert_violation(&report, Rule::Secrets, line_of(&text, code), needle);
        // With nothing secret, the same code decides freely.
        let (_, public) = planted(code, false);
        assert_no(&public, Rule::Secrets);
    }
    let (text, report) = planted("(if (local.get $byte) (then (nop)))", true);
    let line = line_of(&text, "(if (local.get $byte)");
    assert!(
        report
            .lines()
            .iter()
            .any(|shown| shown.starts_with(&format!(
                "secrets main.wat:{line}: the condition of this `if` is computed from a secret"
            ))),
        "{:#?}",
        report.lines()
    );
}

/// What isn't secret decides freely: how many bytes came, a value stored over a secret one, and
/// a callee called with what isn't secret, though another call gives it a secret; a mask chooses
/// with no branch, and secret bytes may be written out.
#[test]
fn the_secret_rule_lets_what_isnt_secret_decide() {
    let passing = [
        "(if (local.get $got) (then (nop)))",
        "(local.set $word (i32.or (i32.and (i32.const 1) (i32.sub (i32.const 0) (local.get \
         $byte))) (i32.and (i32.const 2) (i32.xor (i32.sub (i32.const 0) (local.get $byte)) \
         (i32.const -1)))))",
        "(drop (call $write_all (global.get $STDOUT) (global.get $text) (local.get $got)))",
        "(i32.store8 (global.get $text) (i32.const 0)) (if (i32.load8_u (global.get $text)) \
         (then (nop)))",
        "(call $branch (local.get $got))",
        "(if (call $same (local.get $got)) (then (nop)))",
        "(if (call $first (global.get $out)) (then (nop)))",
    ];
    for code in passing {
        let (_, report) = planted(code, true);
        assert_no(&report, Rule::Secrets);
    }
}

/// A secret goes through calls, globals and memory: a callee branches on a secret argument, on
/// its own line, a callee's result is secret where its argument is, and so is what it reads
/// through an address it's lent, a global set from a secret, and the bytes a secret was stored
/// in.
#[test]
fn a_secret_goes_through_calls_globals_and_memory() {
    let (text, report) = planted(
        "(call $branch (local.get $got)) (call $branch (local.get $byte))",
        true,
    );
    assert_violation(
        &report,
        Rule::Secrets,
        line_of(&text, "(if (local.get $value)"),
        "the condition of this `if`",
    );
    for code in [
        "(if (call $same (local.get $byte)) (then (nop)))",
        "(if (call $first (global.get $text)) (then (nop)))",
        "(global.set $kept (local.get $byte)) (if (global.get $kept) (then (nop)))",
        "(i32.store (global.get $out) (local.get $byte)) (if (i32.load (global.get $out)) (then \
         (nop)))",
    ] {
        let (text, report) = planted(code, true);
        assert_violation(
            &report,
            Rule::Secrets,
            line_of(&text, code),
            "the condition of this `if`",
        );
    }
}

/// Asserts that the secret rule refuses the `if` of `code`, planted in [`SECRET`] on one line,
/// as a branch on a secret.
fn assert_secret_branch(code: &str) {
    let (text, report) = planted(code, true);
    assert_violation(
        &report,
        Rule::Secrets,
        line_of(&text, code),
        "the condition of this `if`",
    );
}

/// A `memory.fill` or a `memory.copy` of a length that isn't one constant may write fewer bytes
/// than the most its length may be, so it leaves a secret under it secret; one of a constant
/// length writes them all, and makes them public.
#[test]
fn a_secret_stays_under_a_fill_or_a_copy_whose_length_varies() {
    assert_secret_branch(
        "(memory.fill (global.get $text) (i32.const 0) (i32.and (local.get $got) (i32.const 1))) \
         (if (i32.load8_u (global.get $text)) (then (nop)))",
    );
    assert_secret_branch(
        "(memory.copy (global.get $text) (global.get $out) (i32.and (local.get $got) (i32.const \
         1))) (if (i32.load8_u (global.get $text)) (then (nop)))",
    );
    let (_, report) = planted(
        "(memory.fill (global.get $text) (i32.const 0) (global.get $text.size)) (if (i32.load8_u \
         (global.get $text)) (then (nop)))",
        true,
    );
    assert_no(&report, Rule::Secrets);
}

/// A store of a secret through an index checked against a limit the memory rules don't follow
/// may land outside the region its address is made from, so a byte of another region is secret
/// after it: here `$out` plus 1024 is `$iovec`'s first byte.
#[test]
fn a_secret_stored_past_a_limit_the_rules_dont_follow_may_be_in_any_region() {
    assert_secret_branch(
        "(i32.store (global.get $iovec) (i32.const 1024)) (i32.store offset=4 (global.get \
         $iovec) (i32.const 1025)) (local.set $word (i32.load (global.get $iovec))) (if \
         (i32.lt_u (local.get $word) (i32.load offset=4 (global.get $iovec))) (then (i32.store8 \
         (i32.add (global.get $out) (local.get $word)) (local.get $byte)))) (if (i32.load8_u \
         (global.get $iovec)) (then (nop)))",
    );
}

/// A secret stored through an address loaded from memory may lie outside every region, where no
/// store at a region's constant offset reaches it: it's still secret when every region has been
/// filled with zeros, and a load through such an address reads it.
#[test]
fn a_secret_stored_outside_every_region_stays_secret() {
    assert_secret_branch(
        "(i32.store (global.get $iovec) (i32.add (global.get $out) (i32.const 512))) (i32.store8 \
         (i32.load (global.get $iovec)) (local.get $byte)) (memory.fill (global.get $text) \
         (i32.const 0) (global.get $text.size)) (memory.fill (global.get $out) (i32.const 0) \
         (global.get $out.size)) (memory.fill (global.get $iovec) (i32.const 0) (global.get \
         $iovec.size)) (i32.store (global.get $iovec) (i32.add (global.get $out) (i32.const \
         512))) (if (i32.load8_u (i32.load (global.get $iovec))) (then (nop)))",
    );
}

/// A store that isn't secret makes a secret byte public only at a constant offset: at an index
/// it may write another byte. A copy carries a secret to its destination, a fill writes its
/// secret value, and a secret a loop's second round brings to a local is one at its start.
#[test]
fn a_secret_stays_where_a_store_may_miss_it_and_goes_where_a_copy_or_a_loop_takes_it() {
    for code in [
        "(i32.store8 (i32.add (global.get $text) (i32.and (local.get $got) (i32.const 1))) \
         (i32.const 0)) (if (i32.load8_u (global.get $text)) (then (nop)))",
        "(memory.copy (global.get $out) (global.get $text) (global.get $out.size)) (if \
         (i32.load8_u (global.get $out)) (then (nop)))",
        "(memory.fill (global.get $out) (local.get $byte) (global.get $out.size)) (if \
         (i32.load8_u (global.get $out)) (then (nop)))",
        "(loop $again (if (local.get $word) (then (nop))) (local.set $word (local.get $byte)) \
         (br_if $again (i32.eqz (local.get $got))))",
    ] {
        assert_secret_branch(code);
    }
}

/// A loop that takes more rounds to settle than the secret rule follows: each round moves the
/// secret one local on, through more locals than rounds. Past its rounds the rule takes all the
/// loop writes as secret, so the last local is secret after it.
#[test]
fn a_secret_a_loop_moves_for_more_rounds_than_the_rule_follows_is_still_secret() {
    const LOCALS: usize = 40;
    let locals: String = (0..LOCALS)
        .map(|local| format!(" (local $l{local} i32)"))
        .collect();
    let moves: String = (1..LOCALS)
        .rev()
        .map(|local| format!("      (local.set $l{local} (local.get $l{}))\n", local - 1))
        .collect();
    let last = format!("(if (local.get $l{}) (then (nop)))", LOCALS - 1);
    let text = format!(
        r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (global $STDIN i32 (i32.const 0))
  (global $EXIT_OK i32 (i32.const 0))
  (global $READ_TO_END i32 (i32.const -1))
  (global $text i32 (i32.const 1024))
  (global $text.size i32 (i32.const 64))
  (func $main (export "_start")
    (local $got i32){locals}
    (local.set $got
      (call $fill (global.get $STDIN) (global.get $text) (global.get $text.size)
        (global.get $READ_TO_END)))
    (loop $again
{moves}      (local.set $l0 (i32.load8_u (global.get $text)))
      (br_if $again (i32.eqz (local.get $got))))
    {last}
    (call $proc_exit (global.get $EXIT_OK))
    unreachable))
"#
    );
    let report = check_secret(&text, &["standard input"], "", &[Source::StandardInput]);
    assert_violation(
        &report,
        Rule::Secrets,
        line_of(&text, &last),
        "the condition of this `if`",
    );
}

/// A program that reads standard input into `$key`, writes `$key` to a file, and reads the file
/// back into `$back` through a descriptor its assertions show isn't standard input's, for the
/// secret rule's test of a secret that comes back.
const ROUND_TRIP: &str = r#"(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $fd_close (param i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (global $STDIN i32 (i32.const 0))
  (global $FOLDER i32 (i32.const 3))
  (global $FIRST_OPENED i32 (i32.const 5))
  (global $MAX_OPENED i32 (i32.const 64))
  (global $CREATE_EMPTY i32 (i32.const 9))
  (global $EXIT_OK i32 (i32.const 0))
  (global $READ_TO_END i32 (i32.const -1))
  (global $name i32 (i32.const 1024))
  (global $name.rosize i32 (i32.const 7))
  (data (i32.const 1024) "tmp.txt")
  (global $opened i32 (i32.const 2048))
  (global $opened.size i32 (i32.const 4))
  (global $key i32 (i32.const 3072))
  (global $key.size i32 (i32.const 32))
  (global $back i32 (i32.const 4096))
  (global $back.size i32 (i32.const 32))
  (func $main (export "_start")
    (local $fd i32)
    (drop
      (call $fill (global.get $STDIN) (global.get $key) (global.get $key.size)
        (global.get $READ_TO_END)))
    (drop
      (call $path_open (global.get $FOLDER) (i32.const 0) (global.get $name)
        (global.get $name.rosize) (global.get $CREATE_EMPTY) (i64.const 64) (i64.const 0)
        (i32.const 0) (global.get $opened)))
    (local.set $fd (i32.load (global.get $opened)))
    (drop (call $write_all (local.get $fd) (global.get $key) (global.get $key.size)))
    (drop (call $fd_close (local.get $fd)))
    (drop
      (call $path_open (global.get $FOLDER) (i32.const 0) (global.get $name)
        (global.get $name.rosize) (i32.const 0) (i64.const 2) (i64.const 0) (i32.const 0)
        (global.get $opened)))
    (local.set $fd (i32.load (global.get $opened)))
    (if (i32.ge_u (local.get $fd) (global.get $MAX_OPENED))
      (then (call $assert_fail (i32.const 1)) unreachable))
    (if (i32.lt_u (local.get $fd) (global.get $FIRST_OPENED))
      (then (call $assert_fail (i32.const 2)) unreachable))
    (drop
      (call $fill (local.get $fd) (global.get $back) (global.get $back.size)
        (global.get $READ_TO_END)))
    (if (i32.load8_u (global.get $back)) (then (nop)))
    (call $proc_exit (global.get $EXIT_OK))
    unreachable))
"#;

/// A program that reads standard input into `$key`, sends `$key` to a peer, and takes what the
/// peer sends into `$back`, for the secret rule's test of a secret a peer sends back.
const ECHO: &str = r#"(module
  (import "wasi_snapshot_preview1" "sock_send"
    (func $sock_send (param i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "sock_recv"
    (func $sock_recv (param i32 i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (global $STDIN i32 (i32.const 0))
  (global $PEER i32 (i32.const 5))
  (global $EXIT_OK i32 (i32.const 0))
  (global $READ_TO_END i32 (i32.const -1))
  (global $iovec i32 (i32.const 1024))
  (global $iovec.size i32 (i32.const 16))
  (global $key i32 (i32.const 2048))
  (global $key.size i32 (i32.const 32))
  (global $back i32 (i32.const 3072))
  (global $back.size i32 (i32.const 32))
  (func $main (export "_start")
    (drop
      (call $fill (global.get $STDIN) (global.get $key) (global.get $key.size)
        (global.get $READ_TO_END)))
    (i32.store (global.get $iovec) (global.get $key))
    (i32.store offset=4 (global.get $iovec) (global.get $key.size))
    (drop
      (call $sock_send (global.get $PEER) (global.get $iovec) (i32.const 1) (i32.const 0)
        (i32.add (global.get $iovec) (i32.const 8))))
    (i32.store (global.get $iovec) (global.get $back))
    (i32.store offset=4 (global.get $iovec) (global.get $back.size))
    (drop
      (call $sock_recv (global.get $PEER) (global.get $iovec) (i32.const 1) (i32.const 0)
        (i32.add (global.get $iovec) (i32.const 8)) (i32.add (global.get $iovec) (i32.const 12))))
    (if (i32.load8_u (global.get $back)) (then (nop)))
    (call $proc_exit (global.get $EXIT_OK))
    unreachable))
"#;

/// A secret a program writes to a file or sends to a peer is secret when it reads it back, from
/// the file or from the peer: a program that writes no secret there reads what isn't secret.
#[test]
fn a_secret_written_to_a_file_or_sent_to_a_peer_is_secret_when_it_comes_back() {
    let branch = "(if (i32.load8_u (global.get $back)) (then (nop)))";
    let secret = [Source::StandardInput];
    let report = check_secret(ROUND_TRIP, &["standard input", "files"], "", &secret);
    assert_violation(
        &report,
        Rule::Secrets,
        line_of(ROUND_TRIP, branch),
        "the condition of this `if`",
    );
    let report = check_secret(
        ECHO,
        &["standard input"],
        "listens on TCP port 8080",
        &secret,
    );
    assert_violation(
        &report,
        Rule::Secrets,
        line_of(ECHO, branch),
        "the condition of this `if`",
    );
    // With no secret written to the file, what the file gives isn't secret.
    let written = "    (drop (call $write_all (local.get $fd) (global.get $key) (global.get \
                   $key.size)))\n";
    assert!(ROUND_TRIP.contains(written));
    let unwritten = ROUND_TRIP.replacen(written, "", 1);
    let report = check_secret(&unwritten, &["standard input", "files"], "", &secret);
    assert_no(&report, Rule::Secrets);
    // And with no secret sent, what the peer sends isn't.
    let sent = "(i32.store (global.get $iovec) (global.get $key))";
    assert!(ECHO.contains(sent));
    let unsent = ECHO.replacen(
        sent,
        "(i32.store (global.get $iovec) (global.get $back))",
        1,
    );
    let report = check_secret(
        &unsent,
        &["standard input"],
        "listens on TCP port 8080",
        &secret,
    );
    assert_no(&report, Rule::Secrets);
}

/// A program that opens a connection by itself, to a computer whose name it holds, for the tests
/// of what its idea must grant it.
const OPENER: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (import "lengua_net_1" "tcp_open"
    (func $tcp_open (param i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1 1)
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_FAILED i32 (i32.const 1))
  (global $HTTP_PORT i32 (i32.const 80))
  (global $PLAIN i32 (i32.const 0))
  (global $name i32 (i32.const 1024))
  (global $name.rosize i32 (i32.const 15))
  (data (i32.const 1024) "www.example.org")
  (global $opened i32 (i32.const 2048))
  (global $opened.size i32 (i32.const 4))
  (func $main (export "_start")
    (local $err i32)
    (block $failed
      (local.set $err
        (call $tcp_open (global.get $name) (global.get $name.rosize) (global.get $HTTP_PORT)
          (global.get $PLAIN) (global.get $opened)))
      (br_if $failed (local.get $err))
      (call $proc_exit (global.get $EXIT_OK))
      unreachable)
    (call $proc_exit (global.get $EXIT_FAILED))
    unreachable))
"#;

/// Rule 3 for what a program opens by itself: lengua's own network functions are imported from
/// their own module, each with its own type, and only when the idea grants what it opens; no
/// other module gives a program anything, and a mode is a named constant there too.
#[test]
fn rule_3_takes_lenguas_own_functions_only_as_the_idea_grants_them() {
    // An import is reported on the line of its `func`.
    let import = line_of(OPENER, "(func $tcp_open (param");
    let granted = "opens TCP connections to port 80 on any subdomain of example.org";
    let report = check_with(OPENER, &[], granted);
    assert_no(&report, Rule::Resources);
    assert_no(&report, Rule::Constants);
    // A connection lengua makes before the start grants no connection the program opens, and
    // neither does a look at a name's addresses.
    for other in [
        "",
        "connects to TCP port 80 on www.example.org",
        "looks up the addresses of www.example.org",
        "sends UDP datagrams to port 80 on www.example.org",
    ] {
        let report = check_with(OPENER, &[], other);
        assert_violation(
            &report,
            Rule::Resources,
            import,
            "it needs `- Network: opens TCP connections to port N on …`",
        );
    }
    let renamed = OPENER.replacen("\"tcp_open\"", "\"tcp_listen\"", 1);
    let report = check_with(&renamed, &[], granted);
    assert_violation(
        &report,
        Rule::Resources,
        import,
        "`lengua_net_1` has no function `tcp_listen`",
    );
    // WASI's module has no such function, and no other module has any.
    for module in ["wasi_snapshot_preview1", "env"] {
        let moved = OPENER.replacen("\"lengua_net_1\"", &format!("\"{module}\""), 1);
        let built = unvalidated(&moved);
        let (uses, network) = crate::idea::constraints(&[], granted).expect("valid constraints");
        let report = check(
            &moved,
            &built,
            &Context {
                uses: &uses,
                network: &network,
                secrets: &[],
            },
        );
        assert!(
            report.violations.iter().any(
                |violation| violation.rule == Rule::Resources && violation.line == Some(import)
            ),
            "{module}: {:#?}",
            report.lines()
        );
    }
    let typed = OPENER.replacen(
        "(func $tcp_open (param i32 i32 i32 i32 i32) (result i32))",
        "(func $tcp_open (param i32 i32 i32 i32 i64) (result i32))",
        1,
    );
    let built = unvalidated(&typed);
    let (uses, network) = crate::idea::constraints(&[], granted).expect("valid constraints");
    let report = check(
        &typed,
        &built,
        &Context {
            uses: &uses,
            network: &network,
            secrets: &[],
        },
    );
    assert_violation(&report, Rule::Resources, import, "`tcp_open` takes");
    // Over TLS or not is a mode, named like any other.
    let bare = OPENER.replacen(
        "(global.get $PLAIN) (global.get $opened)",
        "(i32.const 1) (global.get $opened)",
        1,
    );
    let report = check_with(&bare, &[], granted);
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&bare, "(call $tcp_open"),
        "the mode 1 is a bare number",
    );
}

/// A program that reads standard input into `$key`, sends `$key` as a datagram to a computer, and
/// takes the datagram that comes back into `$back`, for the secret rule's tests of what a
/// program opens by itself.
const DATAGRAM: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (import "lengua_net_1" "udp_open" (func $udp_open (param i32 i32 i32 i32) (result i32)))
  (import "lengua_net_1" "udp_send" (func $udp_send (param i32 i32 i32 i32) (result i32)))
  (import "lengua_net_1" "udp_recv" (func $udp_recv (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1 1)
  (global $STDIN i32 (i32.const 0))
  (global $EXIT_OK i32 (i32.const 0))
  (global $READ_TO_END i32 (i32.const -1))
  (global $PORT i32 (i32.const 53))
  (global $name i32 (i32.const 1024))
  (global $name.rosize i32 (i32.const 10))
  (data (i32.const 1024) "192.0.2.53")
  (global $counts i32 (i32.const 1088))
  (global $counts.size i32 (i32.const 12))
  (global $key i32 (i32.const 2048))
  (global $key.size i32 (i32.const 32))
  (global $back i32 (i32.const 3072))
  (global $back.size i32 (i32.const 32))
  (func $main (export "_start")
    (local $fd i32)
    (drop
      (call $fill (global.get $STDIN) (global.get $key) (global.get $key.size)
        (global.get $READ_TO_END)))
    (drop
      (call $udp_open (global.get $name) (global.get $name.rosize) (global.get $PORT)
        (global.get $counts)))
    (local.set $fd (i32.load (global.get $counts)))
    (drop
      (call $udp_send (local.get $fd) (global.get $key) (global.get $key.size)
        (i32.add (global.get $counts) (i32.const 4))))
    (drop
      (call $udp_recv (local.get $fd) (global.get $back) (global.get $back.size)
        (i32.add (global.get $counts) (i32.const 8))))
    (if (i32.load8_u (global.get $back)) (then (nop)))
    (call $proc_exit (global.get $EXIT_OK))
    unreachable))
"#;

/// The secret rule on what a program opens by itself. A datagram's bytes are data a program
/// sends out, so a secret may be among them, and what comes back is then secret, as on a
/// connection. A computer's name and a port decide where the program connects, and a name goes
/// to the resolver besides: a secret in either is refused, so no secret chooses a computer, and
/// none is told by which computers a program reaches, how many, or when.
#[test]
fn a_secret_never_names_a_computer_and_a_datagram_sent_makes_what_comes_back_secret() {
    let secret = [Source::StandardInput];
    let uses = ["standard input"];
    let granted = "sends UDP datagrams to port 53 on 192.0.2.53";
    let branch = "(if (i32.load8_u (global.get $back)) (then (nop)))";
    let report = check_secret(DATAGRAM, &uses, granted, &secret);
    assert_violation(
        &report,
        Rule::Secrets,
        line_of(DATAGRAM, branch),
        "the condition of this `if`",
    );
    // With no secret sent, what the computer sends back isn't secret.
    let sent = "(call $udp_send (local.get $fd) (global.get $key) (global.get $key.size)";
    assert!(DATAGRAM.contains(sent));
    let unsent = DATAGRAM.replacen(
        sent,
        "(call $udp_send (local.get $fd) (global.get $back) (global.get $back.size)",
        1,
    );
    assert_no(
        &check_secret(&unsent, &uses, granted, &secret),
        Rule::Secrets,
    );
    // The secret as the computer's name: the host reads it, and it isn't data.
    let named = "(call $udp_open (global.get $name) (global.get $name.rosize) (global.get $PORT)";
    assert!(DATAGRAM.contains(named));
    let secret_name = unsent.replacen(
        named,
        "(call $udp_open (global.get $key) (global.get $key.size) (global.get $PORT)",
        1,
    );
    let report = check_secret(&secret_name, &uses, granted, &secret);
    assert_violation(
        &report,
        Rule::Secrets,
        line_of(&secret_name, "(call $udp_open (global.get $key)"),
        "reads a secret in the buffer of argument 1",
    );
    // A byte of the secret as the port.
    let secret_port = unsent.replacen(
        named,
        "(call $udp_open (global.get $name) (global.get $name.rosize) (i32.load8_u (global.get \
         $key))",
        1,
    );
    let report = check_secret(&secret_port, &uses, granted, &secret);
    assert_violation(
        &report,
        Rule::Secrets,
        line_of(&secret_port, "(call $udp_open (global.get $name)"),
        "argument 3 of this call",
    );
}

/// A program that reads standard input with `fd_read` itself, and branches on the error number
/// and then on how many bytes the read gave, for the secret rule's test of a read's count.
const COUNT_READ: &str = r#"(module
  (import "wasi_snapshot_preview1" "fd_read"
    (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (global $STDIN i32 (i32.const 0))
  (global $EXIT_OK i32 (i32.const 0))
  (global $iovec i32 (i32.const 1024))
  (global $iovec.size i32 (i32.const 12))
  (global $line i32 (i32.const 2048))
  (global $line.size i32 (i32.const 64))
  (func $main (export "_start")
    (i32.store (global.get $iovec) (global.get $line))
    (i32.store offset=4 (global.get $iovec) (global.get $line.size))
    (if
      (call $fd_read (global.get $STDIN) (global.get $iovec) (i32.const 1)
        (i32.add (global.get $iovec) (i32.const 8)))
      (then (nop)))
    (if (i32.load offset=8 (global.get $iovec)) (then (nop)))
    (call $proc_exit (global.get $EXIT_OK))
    unreachable))
"#;

/// How many bytes one `fd_read` of a secret standard input gave is secret: a terminal gives a
/// line at a time, so the count says where the secret's line ends. Its error number isn't, and
/// neither is what `$fill` gives when it reads to the end, which [`SECRET`]'s tests branch on.
#[test]
fn the_count_of_one_read_of_a_secret_standard_input_is_secret() {
    let report = check_secret(
        COUNT_READ,
        &["standard input"],
        "",
        &[Source::StandardInput],
    );
    assert_eq!(
        secret_lines(&report),
        [line_of(
            COUNT_READ,
            "(if (i32.load offset=8 (global.get $iovec))"
        )],
        "{:#?}",
        report.lines()
    );
}

/// A program that reads its arguments' text, for the secret rule's test of an argument: where
/// each argument starts isn't secret, its text is.
const ARGUMENTS: &str = r#"(module
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (global $EXIT_OK i32 (i32.const 0))
  (global $argv i32 (i32.const 1024))
  (global $argv.size i32 (i32.const 64))
  (global $args i32 (i32.const 2048))
  (global $args.size i32 (i32.const 256))
  (func $main (export "_start")
    (drop (call $args_get (global.get $argv) (global.get $args)))
    (if (i32.load (global.get $argv)) (then (nop)))
    (if (i32.load8_u (global.get $args)) (then (nop)))
    (call $proc_exit (global.get $EXIT_OK))
    unreachable))
"#;

/// A program that reads a file it opens, for the secret rule's test of a file.
const FILE_READ: &str = r#"(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_read"
    (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (global $FOLDER i32 (i32.const 3))
  (global $EXIT_OK i32 (i32.const 0))
  (global $name i32 (i32.const 1024))
  (global $name.rosize i32 (i32.const 7))
  (data (i32.const 1024) "key.txt")
  (global $opened i32 (i32.const 2048))
  (global $opened.size i32 (i32.const 4))
  (global $iovec i32 (i32.const 2064))
  (global $iovec.size i32 (i32.const 12))
  (global $key i32 (i32.const 3072))
  (global $key.size i32 (i32.const 32))
  (func $main (export "_start")
    (drop
      (call $path_open (global.get $FOLDER) (i32.const 0) (global.get $name)
        (global.get $name.rosize) (i32.const 0) (i64.const 2) (i64.const 0) (i32.const 0)
        (global.get $opened)))
    (i32.store (global.get $iovec) (global.get $key))
    (i32.store offset=4 (global.get $iovec) (global.get $key.size))
    (drop
      (call $fd_read (i32.load (global.get $opened)) (global.get $iovec) (i32.const 1)
        (i32.add (global.get $iovec) (i32.const 8))))
    (if (i32.load8_u (global.get $key)) (then (nop)))
    (call $proc_exit (global.get $EXIT_OK))
    unreachable))
"#;

/// Each input a `- Secret:` line names is secret where the host puts it: an argument's text
/// (but where each argument starts), a variable's text, and a file's bytes read through the
/// descriptor `path_open` gave; a program that parses a secret argument is refused at its first
/// branch on the text, and at each loop the text ends.
#[test]
fn each_secret_input_is_secret_where_the_host_puts_it() {
    let pointer = line_of(ARGUMENTS, "(if (i32.load (global.get $argv))");
    let text = line_of(ARGUMENTS, "(if (i32.load8_u (global.get $args))");
    let report = check_secret(ARGUMENTS, &[], "", &[Source::Argument(1)]);
    assert_eq!(secret_lines(&report), [text], "{:#?}", report.lines());
    let environment = ARGUMENTS.replace("\"args_get\"", "\"environ_get\"");
    let report = check_secret(
        &environment,
        &["the environment"],
        "",
        &[Source::Environment("KEY".to_owned())],
    );
    assert_eq!(secret_lines(&report), [text], "{:#?}", report.lines());
    assert!(secret_lines(&report).iter().all(|line| *line != pointer));
    // A variable secret makes no argument so.
    let report = check_secret(
        ARGUMENTS,
        &["the environment"],
        "",
        &[Source::Environment("KEY".to_owned())],
    );
    assert!(secret_lines(&report).is_empty(), "{:#?}", report.lines());
    let report = check_secret(
        FILE_READ,
        &["files"],
        "",
        &[Source::File("key.txt".to_owned())],
    );
    assert_eq!(
        secret_lines(&report),
        [line_of(FILE_READ, "(if (i32.load8_u (global.get $key))")],
        "{:#?}",
        report.lines()
    );
    // The template parses its argument, so with the argument secret its parse is refused.
    let report = check_secret(TEMPLATE, &[], "", &[Source::Argument(1)]);
    assert_violation(
        &report,
        Rule::Secrets,
        line_of(
            TEMPLATE,
            "(if (i32.eq (i32.load8_u (i32.add (global.get $args) (local.get $i))) (global.get \
             $MINUS))",
        ),
        "the condition of this `if`",
    );
    assert_violation(
        &report,
        Rule::Secrets,
        line_of(TEMPLATE, "(br_if $end (i32.eqz (local.get $c)))"),
        &format!(
            "this `br_if` repeats or leaves the loop on line {}",
            line_of(TEMPLATE, "(loop $digit  ;; bound: 20")
        ),
    );
    assert!(check_text(TEMPLATE).passed());
}

/// The secret rule's reports read `secrets main.wat:LINE: …`, as the dialect's read `dialect`.
#[test]
fn the_secret_rules_reports_name_it() {
    let violation = Violation {
        rule: Rule::Secrets,
        line: Some(7),
        problem: "a secret decides it".into(),
        fix: "mask it".into(),
    };
    assert_eq!(
        violation.in_file(FILE),
        "secrets main.wat:7: a secret decides it. Fix: mask it."
    );
    assert_eq!(Rule::Secrets.label(), "secrets");
    assert_eq!(Rule::Dialect.label(), "dialect");
}

/// A program's memory holds at most 16,384 pages, 1 GiB: one page more is refused on its line,
/// and the most is taken.
#[test]
fn a_memory_past_the_most_pages_is_refused() {
    let most = with(
        "(memory (export \"memory\") 1 1)",
        "(memory (export \"memory\") 16384 16384)",
    );
    assert_no(&check_text(&most), Rule::Resources);
    let past = with(
        "(memory (export \"memory\") 1 1)",
        "(memory (export \"memory\") 16385 16385)",
    );
    let report = check_text(&past);
    assert_violation(
        &report,
        Rule::Resources,
        line_of(&past, "(memory (export"),
        "at most 16384 pages",
    );
}

/// What the two reviews of 2026-11-10 left open (todo.md), each held by a test that failed
/// before its fix, and the tests the reviews found missing.
mod review_gaps {
    use super::*;

    /// Whether `report` leaves the buffers of `$fd_write`'s iovecs to the proof: the rules don't know
    /// what the iovec array holds at the call.
    fn iovecs_left_to_the_proof(report: &Report) -> bool {
        report
            .facts
            .obligations
            .iter()
            .any(|obligation| obligation.what.contains("$fd_write's iovecs point to"))
    }

    /// A write the rules leave to the proof, or one made in an unsafe block, is held inside the
    /// program's memory, not inside the region its address names: in [`SECRET`], `$out` plus 1024
    /// is `$iovec`'s first byte. So after such a write the rules no longer know what an iovec array
    /// holds, and the buffers of the call after it are the proof's too. Where every write between
    /// is one the rules bound, they still judge the buffer the code stored.
    #[test]
    fn a_write_left_to_the_proof_makes_the_rules_forget_what_every_region_holds() {
        let filled = "(i32.store (global.get $iovec) (global.get $out)) (i32.store offset=4 \
                  (global.get $iovec) (global.get $out.size))";
        let call = "(drop (call $fd_write (global.get $STDOUT) (global.get $iovec) (i32.const 1) \
                (i32.add (global.get $iovec) (i32.const 8))))";
        let limit = "(local.set $word (i32.load offset=8 (global.get $iovec))) (if (i32.lt_u \
                 (local.get $word) (i32.load offset=12 (global.get $iovec)))";
        let (_, report) = planted(
            &format!("{filled} (i32.store8 offset=7 (global.get $out) (i32.const 0)) {call}"),
            false,
        );
        assert!(
            !iovecs_left_to_the_proof(&report),
            "{:#?}",
            report.facts.obligations
        );
        let writes = [
        // A store at an index checked against a limit loaded from memory.
        format!(
            "{limit} (then (i32.store (i32.add (global.get $out) (local.get $word)) (i32.const \
             0))))"
        ),
        // A fill of a length checked against such a limit.
        format!("{limit} (then (memory.fill (global.get $out) (i32.const 0) (local.get $word))))"),
        // A store at an index nothing bounds, in an unsafe block.
        "\n    ;; unsafe: the test's own\n    (i32.store (i32.add (global.get $out) (local.get \
         $got)) (i32.const 0))\n    ;; end unsafe\n   "
            .to_owned(),
    ];
        for write in writes {
            let (_, report) = planted(&format!("{filled} {write} {call}"), false);
            assert!(
                iovecs_left_to_the_proof(&report),
                "{write}: {:#?}",
                report.facts.obligations
            );
        }
    }

    /// `args_get` and `environ_get` take no length: the host writes an address for each argument
    /// and every byte of their text. So the rules hold the program to check both of the sizes
    /// `args_sizes_get` wrote against the two regions before the call, and refuse the call where a
    /// size is unchecked, checked too loosely, or no longer the host's; the template checks both,
    /// and nothing of the call is left to the proof.
    #[test]
    fn what_args_get_writes_is_held_to_its_regions_by_checks_of_the_sizes() {
        let call = "(local.set $err (call $args_get (global.get $argv) (global.get $args)))";
        let count = "          (br_if $usage (i32.ne (i32.load (global.get $sizes)) (global.get \
                 $ARGC_EXPECTED)))\n";
        let size = "          (br_if $not_a_number (i32.gt_u (local.get $size) (global.get \
                $args.size)))\n";
        let report = check_text(TEMPLATE);
        assert!(report.passed(), "{:#?}", report.lines());
        assert!(
            report.facts.obligations.is_empty(),
            "{:#?}",
            report.facts.obligations
        );
        // Without the check of the count, or of the text's size.
        for (check, argument) in [(count, 1), (size, 2)] {
            let unchecked = with(check, "");
            let report = check_text(&unchecked);
            assert_violation(
                &report,
                Rule::Bounds,
                line_of(&unchecked, call),
                &format!(
                    "the buffer $args_get borrows in argument {argument} gets as many bytes as the \
                 host has for it"
                ),
            );
        }
        // A check that lets through one byte more than the region holds.
        let loose = with(
            size,
            "          (br_if $not_a_number (i32.gt_u (local.get $size) (i32.add (global.get \
         $args.size) (global.get $STDOUT))))\n",
        );
        let report = check_text(&loose);
        assert_violation(
            &report,
            Rule::Bounds,
            line_of(&loose, call),
            "the buffer $args_get borrows in argument 2 reaches byte 64 of $args, which has 64 bytes",
        );
        // A size the program stored over the host's isn't the host's.
        let own = with(
            size,
            &format!(
                "          (i32.store offset=4 (global.get $sizes) (i32.const 0))\n          \
             (local.set $size (i32.load offset=4 (global.get $sizes)))\n{size}"
            ),
        );
        let report = check_text(&own);
        assert_violation(
            &report,
            Rule::Bounds,
            line_of(&own, call),
            "the buffer $args_get borrows in argument 2 gets as many bytes as the host has for it",
        );
        // The environment's two calls are held the same way.
        let environment = |text: &str| {
            text.replace("args_sizes_get", "environ_sizes_get")
                .replace("args_get", "environ_get")
        };
        let report = check_with(&environment(TEMPLATE), &["the environment"], "");
        assert!(report.passed(), "{:#?}", report.lines());
        assert!(
            report.facts.obligations.is_empty(),
            "{:#?}",
            report.facts.obligations
        );
        let unchecked = environment(&with(size, ""));
        let report = check_with(&unchecked, &["the environment"], "");
        assert_violation(
            &report,
            Rule::Bounds,
            line_of(&unchecked, &environment(call)),
            "the buffer $environ_get borrows in argument 2 gets as many bytes as the host has for it",
        );
    }

    /// Every finding of the secret rule alone on `text`, with standard input secret: a report
    /// shows only the first findings of all the rules together, and the programs below break
    /// others on purpose.
    fn secret_findings(text: &str) -> Vec<String> {
        let built = build(text).unwrap_or_else(|problem| panic!("it builds: {problem}"));
        let module = model::Module::read(text, &built).expect("it reads");
        let (uses, network) =
            crate::idea::constraints(&["standard input"], "").expect("valid constraints");
        let secrets = [Source::StandardInput];
        let context = Context {
            uses: &uses,
            network: &network,
            secrets: &secrets,
        };
        let mut others = Vec::new();
        let flows = flow::analyze(&module, &context.policy(), &mut others);
        let analysis = memory::analyze(&module, &flows);
        let mut violations = Vec::new();
        let _ = secret::check(&module, &flows, &analysis, context.secrets, &mut violations);
        violations
            .iter()
            .map(|violation| violation.in_file(FILE))
            .collect()
    }

    /// A branch on the secret's first byte.
    const SECRET_BRANCH: &str = "(if (local.get $byte) (then (nop)))";

    /// [`SECRET`] with `code` where it says `;; plant`, on one line.
    fn secret_with(code: &str) -> String {
        SECRET.replacen("    ;; plant\n", &format!("    {code}\n"), 1)
    }

    /// [`SECRET_BRANCH`] inside `depth` blocks, each inside the one before.
    fn nested(depth: usize) -> String {
        format!(
            "{}{SECRET_BRANCH}{}",
            "(block ".repeat(depth),
            ")".repeat(depth)
        )
    }

    /// The secret rule follows blocks and calls 256 deep, counted together. Within that it finds a
    /// branch on a secret where it is; deeper, it says it can't follow the block or the call, and
    /// refuses the program for it, so a branch too deep to see is never passed. The modules are
    /// made near the limit on both sides, 200 and 300 deep.
    #[test]
    fn the_secret_rule_refuses_what_nests_deeper_than_it_follows() {
        let cant_follow = "the secret rule can't follow";
        let within = secret_findings(&secret_with(&nested(200)));
        assert!(
            within
                .iter()
                .any(|line| line.contains("the condition of this `if` is computed from a secret")),
            "{within:#?}"
        );
        assert!(
            !within.iter().any(|line| line.contains(cant_follow)),
            "{within:#?}"
        );
        let past = secret_findings(&secret_with(&nested(300)));
        assert!(
            past.iter()
                .any(|line| line.contains("the secret rule can't follow blocks nested this deep")),
            "{past:#?}"
        );
        // Calls: each function passes the secret to the next, and the last branches on it.
        let chain = |length: usize| {
            let mut functions: String = (0..length)
                .map(|at| {
                    format!(
                        "  (func $f{at} (param $value i32)\n    (call $f{} (local.get $value)))\n",
                        at + 1
                    )
                })
                .collect();
            functions.push_str(&format!(
                "  (func $f{length} (param $value i32)\n    (if (local.get $value) (then (nop))))\n"
            ));
            secret_with("(call $f0 (local.get $byte))").replacen(
                "  (func $branch",
                &format!("{functions}  (func $branch"),
                1,
            )
        };
        let within = secret_findings(&chain(200));
        assert!(
            within
                .iter()
                .any(|line| line.contains("the condition of this `if` is computed from a secret")),
            "{within:#?}"
        );
        assert!(
            !within.iter().any(|line| line.contains(cant_follow)),
            "{within:#?}"
        );
        let past = secret_findings(&chain(300));
        assert!(
            past.iter().any(|line| line
                .contains("the secret rule can't follow calls and blocks nested this deep")),
            "{past:#?}"
        );
    }

    /// The secret rule follows 4,000,000 operators in all. Here a loop of 140,000 operators moves
    /// the secret one local on each round, through more locals than the rule's rounds, so the rule
    /// follows the loop round after round until it has followed that many; the call after the loop
    /// is then one it says it can't follow, and refuses the program for.
    #[test]
    fn the_secret_rule_refuses_a_program_longer_than_it_follows() {
        const LOCALS: usize = 40;
        const FILLER: usize = 140_000;
        let locals: String = (0..LOCALS)
            .map(|local| format!(" (local $l{local} i32)"))
            .collect();
        let moves: String = (1..LOCALS)
            .rev()
            .map(|local| format!(" (local.set $l{local} (local.get $l{}))", local - 1))
            .collect();
        let filler = "(nop) ".repeat(FILLER);
        let text = SECRET
        .replacen("    (local $word i32)\n", &format!("    (local $word i32){locals}\n"), 1)
        .replacen(
            "    ;; plant\n",
            &format!(
                "    (loop $again{moves} (local.set $l0 (local.get $byte)) {filler}(br_if $again \
                 (i32.eqz (local.get $got))))\n    (call $branch (local.get $got))\n"
            ),
            1,
        );
        let found = secret_findings(&text);
        assert!(
            found
                .iter()
                .any(|line| line.contains("the secret rule can't follow a program this long")),
            "{found:#?}"
        );
    }

    /// Blocks nested far deeper than any rule follows, 10,000 deep, are refused by the rules that
    /// stop at their own limits, on the checker's stack as it is: no pass of the checker goes one
    /// call deeper for each block.
    #[test]
    fn blocks_nested_very_deep_are_refused_within_the_checkers_stack() {
        let text = secret_with(&nested(10_000));
        let report = check_secret(&text, &["standard input"], "", &[Source::StandardInput]);
        assert!(!report.passed());
        let found = secret_findings(&text);
        assert!(
            found
                .iter()
                .any(|line| line.contains("the secret rule can't follow blocks nested this deep")),
            "{found:#?}"
        );
    }
}
