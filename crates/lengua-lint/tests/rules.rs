use lengua_lint::{Finding, LINES, Source, lint};

fn source(text: &str, structure: &[&str]) -> Source {
    Source {
        file: "src/main.rs".to_string(),
        idea: "idea.md".to_string(),
        text: text.to_string(),
        structure: structure
            .iter()
            .enumerate()
            .map(|(k, n)| (k + 30, (*n).to_string()))
            .collect(),
    }
}

fn found(text: &str, structure: &[&str], crates: &[&str]) -> Vec<Finding> {
    let crates: Vec<String> = crates.iter().map(|c| (*c).to_string()).collect();
    lint(&[source(text, structure)], &crates)
}

fn codes(text: &str, structure: &[&str]) -> Vec<&'static str> {
    found(text, structure, &[])
        .into_iter()
        .map(|f| f.code)
        .collect()
}

fn refused(text: &str, structure: &[&str], code: &str) {
    let all = found(text, structure, &[]);
    assert_ne!(all, Vec::<Finding>::new(), "no findings for:\n{text}");
    assert!(
        all.iter().all(|f| f.code == code),
        "only {code} wanted:\n{all:#?}\nfor:\n{text}"
    );
}

const GOOD: &str = "use std::io::Read;

const LIMIT: u32 = 10;

static NAMES: [&str; 2] = [\"one\", \"two\"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Counter {
    total: u32,
}

impl Counter {
    fn add(&mut self, step: u32) {
        assert!(step > 0);
        assert!(self.total <= u32::MAX - step);
        self.total += step;
    }
}

macro_rules! double {
    ($value:expr) => {
        $value * 2
    };
}

fn count(text: &str) -> u32 {
    assert!(!text.is_empty());
    let mut counter = Counter { total: 0 };
    let mut round = 0;
    while round < LIMIT {
        counter.add(1);
        round += 1;
    }
    for _ in text.split_whitespace() {
        counter.add(1);
    }
    let mut left = 3_u32;
    loop {
        if left == 0 || left < 1 {
            break;
        }
        left -= 1;
    }
    debug_assert!(counter.total >= LIMIT);
    double!(counter.total)
}

#[cfg(target_os = \"wasi\")]
fn main() {
    let mut text = String::new();
    if std::io::stdin().read_to_string(&mut text).is_ok() {
        println!(\"{} {}\", count(&text), NAMES.len());
    }
}
";

#[test]
fn a_program_that_keeps_every_rule_has_no_findings() {
    let all = found(GOOD, &["Counter::add", "count", "main"], &[]);
    assert_eq!(all, Vec::<Finding>::new());
}

#[test]
fn rule_1_direct_recursion_is_refused() {
    refused(
        "fn main() {\n    main();\n}\n",
        &["main"],
        "rule-1-recursion",
    );
}

#[test]
fn rule_1_recursion_through_another_function_is_refused() {
    let text = "fn ping(n: u32) -> u32 {
    assert!(n < 100);
    assert!(n > 0);
    pong(n)
}

fn pong(n: u32) -> u32 {
    assert!(n < 100);
    assert!(n > 0);
    ping(n)
}

fn main() {
    println!(\"{}\", ping(1));
}
";
    let all = found(text, &["ping", "pong", "main"], &[]);
    let lines: Vec<(usize, &str)> = all.iter().map(|f| (f.line, f.code)).collect();
    assert_eq!(lines, [(1, "rule-1-recursion"), (7, "rule-1-recursion")]);
    assert!(all[0].message.contains("through `pong`"), "{all:#?}");
}

#[test]
fn rule_1_a_method_that_calls_itself_is_refused() {
    let text = "struct Walker {
    steps: u32,
}

impl Walker {
    fn walk(&self) -> u32 {
        assert!(self.steps < 10);
        assert!(self.steps > 0);
        self.walk()
    }
}

fn main() {
    let walker = Walker { steps: 1 };
    println!(\"{}\", walker.walk());
}
";
    let all = found(text, &["Walker::walk", "main"], &[]);
    let lines: Vec<(usize, &str)> = all.iter().map(|f| (f.line, f.code)).collect();
    assert_eq!(lines, [(6, "rule-1-recursion")]);
}

#[test]
fn rule_1_a_function_passed_as_a_value_is_in_the_call_graph() {
    let text = "fn twice(f: impl Fn(u32) -> u32) -> u32 {
    assert!(f(1) > 0);
    assert!(f(2) > 0);
    f(3)
}

fn step(n: u32) -> u32 {
    assert!(n < 10);
    assert!(n < 11);
    twice(step)
}

fn main() {
    println!(\"{}\", step(1));
}
";
    let all = found(text, &["twice", "step", "main"], &[]);
    let lines: Vec<(usize, &str)> = all.iter().map(|f| (f.line, f.code)).collect();
    assert_eq!(lines, [(7, "rule-1-recursion")]);
}

#[test]
fn rule_1_a_method_on_a_field_or_a_parameter_of_a_known_type_is_resolved() {
    let text = "struct Words {
    items: Vec<String>,
}

impl Words {
    fn len(&self) -> usize {
        assert!(self.items.len() < 1000);
        assert!(self.items.capacity() >= self.items.len());
        self.items.len()
    }
}

fn total(words: &Words) -> usize {
    let total = words.len();
    assert!(total < 1000);
    assert!(total < 1001);
    total
}

fn main() {
    let words = Words { items: Vec::new() };
    println!(\"{}\", total(&words));
}
";
    assert_eq!(
        found(text, &["Words::len", "total", "main"], &[]),
        Vec::<Finding>::new()
    );
}

#[test]
fn rule_1_dyn_is_refused() {
    refused(
        "fn main() {\n    let shown: Box<dyn std::fmt::Display> = Box::new(1);\n    println!(\"{shown}\");\n}\n",
        &["main"],
        "rule-1-dyn",
    );
}

#[test]
fn rule_1_a_function_pointer_is_refused() {
    let text = "fn one() -> u32 {
    assert!(true);
    assert!(true);
    1
}

fn main() {
    let f: fn() -> u32 = one;
    println!(\"{}\", f());
}
";
    refused(text, &["one", "main"], "rule-1-fn-pointer");
}

#[test]
fn rule_2_a_loop_with_no_counter_is_refused() {
    refused(
        "fn main() {\n    let mut count = 0_u32;\n    loop {\n        count += 2;\n    }\n}\n",
        &["main"],
        "rule-2-loop",
    );
    refused(
        "fn main() {\n    let mut v = vec![1];\n    while let Some(x) = v.pop() {\n        println!(\"{x}\");\n    }\n}\n",
        &["main"],
        "rule-2-loop",
    );
    refused(
        "fn main() {\n    for n in 0.. {\n        println!(\"{n}\");\n    }\n}\n",
        &["main"],
        "rule-2-loop",
    );
}

fn long(lines: usize) -> String {
    let body = "    println!(\"a line\");\n".repeat(lines - 2);
    format!("fn main() {{\n{body}}}\n")
}

#[test]
fn rule_4_a_function_of_more_than_60_lines_is_refused() {
    assert_eq!(codes(&long(LINES), &["main"]), Vec::<&str>::new());
    refused(&long(LINES + 1), &["main"], "rule-4-length");
}

#[test]
fn rule_5_too_few_assertions_are_refused() {
    let text = "fn helper() -> u32 {
    assert!(true);
    1
}

fn main() {
    println!(\"{}\", helper());
}
";
    refused(text, &["helper", "main"], "rule-5-assertions");
    assert_eq!(
        codes("fn main() {\n    println!(\"hello\");\n}\n", &["main"]),
        Vec::<&str>::new()
    );
}

#[test]
fn rule_6_mutable_globals_are_refused() {
    for text in [
        "static mut COUNT: u32 = 0;\n\nfn main() {}\n",
        "static LOCK: std::sync::Mutex<u32> = std::sync::Mutex::new(0);\n\nfn main() {}\n",
        "use std::sync::atomic::AtomicU32;\n\nstatic HITS: AtomicU32 = AtomicU32::new(0);\n\nfn main() {}\n",
        "struct Holder {\n    cell: std::cell::Cell<u32>,\n}\n\nstatic HELD: Holder = Holder {\n    cell: std::cell::Cell::new(0),\n};\n\nfn main() {}\n",
        "thread_local! {\n    static KEPT: u32 = 0;\n}\n\nfn main() {}\n",
    ] {
        refused(text, &["main"], "rule-6-static");
    }
    let plain = "struct Pair {\n    a: u32,\n    b: Option<u8>,\n}\n\nstatic PAIR: Pair = Pair { a: 1, b: None };\n\nfn main() {}\n";
    assert_eq!(codes(plain, &["main"]), Vec::<&str>::new());
}

#[test]
fn rule_8_macros_that_define_items_are_refused() {
    let text = "macro_rules! make {
    ($name:ident) => {
        fn $name() {}
    };
}

make!(hello);

fn main() {}
";
    refused(text, &["main"], "rule-8-macro-rules");
}

#[test]
fn rule_8_macros_derives_attributes_and_cfg_from_elsewhere_are_refused() {
    refused(
        "fn main() {\n    other::thing!(1);\n}\n",
        &["main"],
        "rule-8-macro",
    );
    refused(
        "fn main() {\n    include!(\"x.rs\");\n}\n",
        &["main"],
        "rule-8-macro",
    );
    refused(
        "#[derive(Serialize)]\nstruct A;\n\nfn main() {}\n",
        &["main"],
        "rule-8-derive",
    );
    refused("#[test]\nfn main() {}\n", &["main"], "rule-8-attribute");
    refused(
        "#[cfg_attr(unix, inline)]\nfn main() {}\n",
        &["main"],
        "rule-8-attribute",
    );
    refused("#[cfg(test)]\nfn main() {}\n", &["main"], "rule-8-cfg");
    refused(
        "fn main() {\n    if cfg!(debug_assertions) {\n        println!(\"debug\");\n    }\n}\n",
        &["main"],
        "rule-8-cfg",
    );
}

#[test]
fn rule_8_macros_and_derives_of_a_declared_crate_are_taken() {
    let text = "use gpui::div;\n\n#[derive(gpui::Render)]\nstruct View;\n\nfn main() {\n    div!();\n    gpui::other!();\n}\n";
    assert_eq!(found(text, &["main"], &["gpui"]), Vec::<Finding>::new());
    assert_ne!(found(text, &["main"], &[]), Vec::<Finding>::new());
}

#[test]
fn rule_10_turning_a_warning_off_is_refused() {
    refused(
        "#[allow(clippy::unwrap_used)]\nfn main() {}\n",
        &["main"],
        "rule-10-allow",
    );
    refused(
        "#![allow(unused)]\n\nfn main() {}\n",
        &["main"],
        "rule-10-allow",
    );
    refused(
        "#[expect(dead_code)]\nfn main() {}\n",
        &["main"],
        "rule-10-allow",
    );
}

#[test]
fn rule_11_comments_and_doc_comments_are_refused() {
    let text = "#!/usr/bin/env run\n// a comment\n/// a doc comment\n/* a block */\n#[doc = \"text\"]\nfn main() {}\n";
    let all = found(text, &["main"], &[]);
    let lines: Vec<(usize, &str)> = all.iter().map(|f| (f.line, f.code)).collect();
    assert_eq!(
        lines,
        [
            (1, "rule-11-comment"),
            (2, "rule-11-comment"),
            (3, "rule-11-comment"),
            (4, "rule-11-comment"),
            (5, "rule-11-comment")
        ]
    );
}

#[test]
fn rule_12_the_structure_and_the_functions_must_match() {
    let text = "fn helper() -> u32 {
    assert!(true);
    assert!(true);
    1
}

fn main() {
    println!(\"{}\", helper());
}
";
    let all = found(text, &["main", "gone"], &[]);
    let places: Vec<(&str, usize, &str)> = all
        .iter()
        .map(|f| (f.file.as_str(), f.line, f.code))
        .collect();
    assert_eq!(
        places,
        [
            ("idea.md", 31, "rule-12-unknown"),
            ("src/main.rs", 1, "rule-12-unnamed")
        ]
    );
}

#[test]
fn rule_12_nested_items_inline_modules_and_impls_of_other_types_are_refused() {
    refused(
        "fn main() {\n    fn inner() {}\n    inner();\n}\n",
        &["main"],
        "rule-12-nested",
    );
    refused(
        "mod inner {}\n\nfn main() {}\n",
        &["main"],
        "rule-12-module",
    );
    refused(
        "trait Twice {\n    fn twice(&self) -> u32;\n}\n\nimpl Twice for u32 {\n    fn twice(&self) -> u32 {\n        assert!(true);\n        assert!(true);\n        self * 2\n    }\n}\n\nfn main() {}\n",
        &["main"],
        "rule-12-impl",
    );
}

#[test]
fn what_the_reader_can_t_read_is_a_finding() {
    refused(
        "fn main() {\n    let s = \"open;\n}\n",
        &["main"],
        "rust-read",
    );
    refused("fn main() {\n    let s = (1;\n}\n", &["main"], "rust-read");
    let all = codes(
        "unsafe extern \"C\" {\n    fn abs(x: i32) -> i32;\n}\n\nfn main() {}\n",
        &["main"],
    );
    assert!(all.contains(&"rust-read"), "{all:?}");
}
