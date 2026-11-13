use lengua_idea::{WebLintInput, WebSource};

const GOOD: &str = r#"import { useEffect } from "react";
import { Button } from "@/components/ui/button";
import { assert, getNotes, useScreen } from "@/lib/lengua";

export function count(items: readonly string[]): number {
  assert(items.length >= 0, "a list has a length");
  let total = 0;
  for (const item of items) {
    total += item.length > 0 ? 1 : 0;
  }
  for (let i = 0; i < items.length; i++) {
    total += 0;
  }
  assert(total <= items.length, "the count is not more than the items");
  return total;
}

export function HomeScreen() {
  const [state, setState] = useScreen({ notes: [] as const, error: "" });
  useEffect(() => {
    getNotes()
      .then((answer) => {
        setState({ ...state, error: answer.ok ? "" : "failed" });
      })
      .catch(() => {
        setState({ ...state, error: "failed" });
      });
  }, []);
  return <Button onClick={() => setState({ ...state, error: "" })}>{count([])}</Button>;
}
"#;

fn screen(text: &str, names: &[&str]) -> WebLintInput {
    WebLintInput {
        screens: vec![WebSource {
            file: "web/screens/home.tsx".to_string(),
            idea: "ideas/ui/home.md".to_string(),
            text: text.to_string(),
            structure: names
                .iter()
                .enumerate()
                .map(|(k, n)| (k + 10, (*n).to_string()))
                .collect(),
            main: "HomeScreen".to_string(),
        }],
        packages: Vec::new(),
        client: vec!["getNotes".to_string()],
    }
}

fn codes(text: &str, names: &[&str]) -> Vec<&'static str> {
    lengua_ts::lint(&screen(text, names))
        .findings
        .iter()
        .map(|f| f.code)
        .collect()
}

#[test]
fn a_screen_that_keeps_every_rule_has_no_findings_and_its_calls_are_read() {
    let output = lengua_ts::lint(&screen(GOOD, &["count", "HomeScreen"]));
    assert_eq!(output.findings, [], "{:#?}", output.findings);
    assert_eq!(
        output.calls,
        [(
            "web/screens/home.tsx".to_string(),
            vec!["getNotes".to_string()]
        )]
    );
}

#[test]
fn each_broken_rule_is_a_finding() {
    let cases: [(&str, &str); 14] = [
        ("// a note\n", "ts-comment"),
        ("/** docs */\n", "ts-comment"),
        ("// @ts-ignore\n", "ts-ignore"),
        (
            "export function f(x: any): number { assert(true, \"a\"); assert(true, \"b\"); return x; }\n",
            "ts-any",
        ),
        (
            "export function f(x: unknown): number { assert(true, \"a\"); assert(true, \"b\"); return x as number; }\n",
            "ts-cast",
        ),
        (
            "export function f(x: string | null): number { assert(true, \"a\"); assert(true, \"b\"); return x!.length; }\n",
            "ts-non-null",
        ),
        (
            "export function f(): number { assert(true, \"a\"); assert(true, \"b\"); while (true) { return 1; } }\n",
            "ts-loop",
        ),
        (
            "export function f(): void { assert(true, \"a\"); assert(true, \"b\"); getNotes(); }\n",
            "ts-promise",
        ),
        (
            "export function f(): void { assert(true, \"a\"); assert(true, \"b\"); fetch(\"/x\").catch(() => 1); }\n",
            "ts-fetch",
        ),
        ("import { z } from \"zod\";\n", "ts-import"),
        ("let shared = 1;\n", "ts-global"),
        (
            "export function f(): number { assert(true, \"a\"); assert(true, \"b\"); return f(); }\n",
            "rule-12-unknown",
        ),
        ("export function g(): number { return 1; }\n", "ts-unnamed"),
        ("export class A {}\n", "ts-class"),
    ];
    for (extra, code) in cases {
        let text = format!("{GOOD}{extra}");
        let found = codes(&text, &["count", "HomeScreen", "f", "missing"]);
        assert!(found.contains(&code), "{code} for {extra:?}: {found:?}");
    }
}

#[test]
fn recursion_assertions_length_and_the_main_component_are_held() {
    let cycle = format!(
        "{GOOD}export function f(): number {{ assert(true, \"a\"); assert(true, \"b\"); return g(); }}\nexport function g(): number {{ assert(true, \"a\"); assert(true, \"b\"); return f(); }}\n"
    );
    assert!(codes(&cycle, &["count", "HomeScreen", "f", "g"]).contains(&"ts-recursion"));
    let weak = format!(
        "{GOOD}export function f(): number {{ return 1; }}\nexport function g(): number {{ return 2; }}\n"
    );
    assert!(codes(&weak, &["count", "HomeScreen", "f", "g"]).contains(&"ts-assertions"));
    let long = format!(
        "{GOOD}export function f(): number {{\n  assert(true, \"a\");\n  assert(true, \"b\");\n{}  return 1;\n}}\n",
        "  const a = 1;\n".repeat(60)
    );
    assert!(codes(&long, &["count", "HomeScreen", "f"]).contains(&"ts-length"));
    let renamed = GOOD.replace("HomeScreen", "Home");
    assert!(codes(&renamed, &["count", "Home"]).contains(&"ts-main-missing"));
    let handler = GOOD.replace("onClick={() => setState", "onClick={async () => setState");
    assert!(codes(&handler, &["count", "HomeScreen"]).contains(&"ts-promise"));
    let unsafe_html = GOOD.replace(
        "<Button onClick",
        "<Button dangerouslySetInnerHTML={{ __html: \"x\" }} onClick",
    );
    assert!(codes(&unsafe_html, &["count", "HomeScreen"]).contains(&"ts-unsafe"));
}

#[test]
fn a_file_that_does_not_parse_is_one_finding() {
    assert_eq!(codes("export function (", &[]), ["ts-read"]);
}

#[test]
fn the_mutants_change_operators_constants_booleans_and_conditions_but_not_assertions() {
    let text = "export function f(a: number): boolean {\n  assert(a > 0, \"a\");\n  if (a < 10) {\n    return a + 1 === 3 && true;\n  }\n  return false;\n}\n".to_string();
    let (groups, skeleton) =
        lengua_ts::groups(&[("web/screens/home.tsx".to_string(), text.clone())]);
    let changes: Vec<(String, String)> = groups
        .iter()
        .flatten()
        .map(|m| (m.from.clone(), m.to.clone()))
        .collect();
    for wanted in [
        ("<", "<="),
        ("+", "-"),
        ("1", "2"),
        ("===", "!=="),
        ("&&", "||"),
        ("true", "false"),
        ("false", "true"),
    ] {
        assert!(
            changes.contains(&(wanted.0.to_string(), wanted.1.to_string())),
            "{wanted:?} in {changes:?}"
        );
    }
    assert!(changes.iter().any(|(_, to)| to == "!(a < 10)"));
    assert!(!changes.iter().any(|(from, _)| from == ">"), "{changes:?}");
    assert!(skeleton.contains("web/screens/home.tsx\nf\n"));
    let one = &groups[0][0];
    assert_eq!(
        lengua_lint::apply(&text, &one.edits).len(),
        text.len() + one.to.len() - one.from.len()
    );
}

#[test]
fn lenguas_own_copies_of_shadcn_and_the_generated_parts_have_no_comments() {
    for (name, text) in [
        (
            "button",
            include_str!("../../lengua-idea/src/shadcn/button.tsx"),
        ),
        (
            "card",
            include_str!("../../lengua-idea/src/shadcn/card.tsx"),
        ),
        (
            "input",
            include_str!("../../lengua-idea/src/shadcn/input.tsx"),
        ),
        (
            "label",
            include_str!("../../lengua-idea/src/shadcn/label.tsx"),
        ),
        (
            "textarea",
            include_str!("../../lengua-idea/src/shadcn/textarea.tsx"),
        ),
        (
            "utils",
            include_str!("../../lengua-idea/src/front/utils.ts"),
        ),
        (
            "core",
            include_str!("../../lengua-idea/src/front/lengua-core.ts"),
        ),
        (
            "helper",
            include_str!("../../lengua-idea/src/front/test-helper.tsx"),
        ),
        (
            "vitest",
            include_str!("../../lengua-idea/src/front/vitest.config.mts"),
        ),
        (
            "next",
            include_str!("../../lengua-idea/src/front/next.config.mjs"),
        ),
    ] {
        assert_eq!(
            lengua_ts::comment_lines(text),
            Vec::<usize>::new(),
            "{name}"
        );
    }
}
