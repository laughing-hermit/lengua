use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::cases::{StateValue, Test, cases};
use crate::constraints::Constraints;
use crate::generate::{collect_skipping, held};
use crate::idea::IdeaKind;
use crate::types::{self, Ty, merge, quoted, render};
use crate::wiki::{Page, Stamp};
use crate::{Finding, json, sha256};

pub const SUM: &str = "web/lengua.sum";
pub const CLIENT: &str = "web/lib/lengua.ts";
pub const PNPM: &str = "12.18.0";

const BASE: [(&str, &str); 3] = [
    ("next", "16.4.6"),
    ("react", "19.3.1"),
    ("react-dom", "19.3.1"),
];
const DEV: [(&str, &str); 11] = [
    ("@tailwindcss/postcss", "4.3.5"),
    ("@testing-library/dom", "10.4.2"),
    ("@testing-library/react", "16.3.3"),
    ("@types/node", "26.6.6"),
    ("@types/react", "19.3.0"),
    ("@types/react-dom", "19.3.0"),
    ("happy-dom", "21.0.4"),
    ("tailwindcss", "4.3.5"),
    ("typescript", "7.0.2"),
    ("vite", "8.3.8"),
    ("vitest", "5.1.0"),
];
const SHADCN_PACKAGES: [(&str, &str); 2] = [("clsx", "2.1.1"), ("tailwind-merge", "3.7.0")];
const SKIPPED: [&str; 3] = ["web/node_modules", "web/.next", "web/out"];
const SCRIPT_EXTENSIONS: [&str; 8] = ["ts", "tsx", "js", "jsx", "mjs", "mts", "cjs", "cts"];

const CORE: &str = include_str!("front/lengua-core.ts");
const HELPER: &str = include_str!("front/test-helper.tsx");
const STATIC: [(&str, &str); 6] = [
    (
        "web/pnpm-workspace.yaml",
        include_str!("front/pnpm-workspace.yaml"),
    ),
    ("web/next.config.mjs", include_str!("front/next.config.mjs")),
    ("web/tsconfig.json", include_str!("front/tsconfig.json")),
    (
        "web/vitest.config.mts",
        include_str!("front/vitest.config.mts"),
    ),
    (
        "web/postcss.config.mjs",
        include_str!("front/postcss.config.mjs"),
    ),
    ("web/app/globals.css", include_str!("front/globals.css")),
];
const SHADCN_FILES: [(&str, &str); 5] = [
    ("button", include_str!("shadcn/button.tsx")),
    ("card", include_str!("shadcn/card.tsx")),
    ("input", include_str!("shadcn/input.tsx")),
    ("label", include_str!("shadcn/label.tsx")),
    ("textarea", include_str!("shadcn/textarea.tsx")),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Route {
    pub method: String,
    pub path: String,
    pub line: usize,
    pub function: String,
    pub json: bool,
    pub body: Option<Ty>,
    pub answer: Ty,
    pub failure: Ty,
    pub served_by: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenRun {
    pub index: usize,
    pub file: String,
    pub line: usize,
    pub name: String,
    pub screen: String,
    pub test: Test,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Front {
    pub screens: Vec<String>,
    pub sources: Vec<String>,
    pub generated: Vec<String>,
    pub packages: Vec<(String, String)>,
    pub runs: Vec<ScreenRun>,
}

pub(crate) struct WebPlan {
    pub texts: Vec<(String, String)>,
    pub stamp: Stamp,
    pub front: Front,
}

fn pascal(text: &str) -> String {
    text.split(['-', '_', '.'])
        .filter(|p| !p.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect()
}

fn camel(text: &str) -> String {
    let upper = pascal(text);
    let mut chars = upper.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_ascii_lowercase().to_string() + chars.as_str()
    })
}

#[must_use]
pub fn component_name(screen: &str) -> String {
    format!("{}Screen", pascal(screen))
}

#[must_use]
pub fn screen_route(screen: &str) -> String {
    if screen == "home" {
        "/".to_string()
    } else {
        format!("/{screen}/")
    }
}

fn param(part: &str) -> Option<&str> {
    part.strip_prefix('{').and_then(|p| p.strip_suffix('}'))
}

fn function_name(method: &str, path: &str) -> String {
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    let mut name = method.to_lowercase();
    for (i, part) in parts.iter().enumerate() {
        if i == 0 && *part == "api" && parts.len() > 1 {
            continue;
        }
        match param(part) {
            Some(p) => {
                name.push_str("By");
                name.push_str(&pascal(p));
            }
            None => name.push_str(&pascal(part)),
        }
    }
    if parts.is_empty() {
        name.push_str("Root");
    }
    name
}

fn matches(pattern: &str, path: &str) -> bool {
    let a: Vec<&str> = pattern.split('/').collect();
    let b: Vec<&str> = path.split('/').collect();
    a.len() == b.len()
        && a.iter()
            .zip(&b)
            .all(|(x, y)| x == y || (param(x).is_some() && !y.is_empty()))
}

fn split_request(request: &str) -> Option<(&str, &str)> {
    let (method, target) = request.split_once(' ')?;
    Some((method, target.split('?').next().unwrap_or(target)))
}

fn add_example(route: &mut Route, test: &Test) {
    if let Some(body) = test.request_body.as_deref().and_then(json::parse) {
        let now = route.body.clone().unwrap_or(Ty::Unknown);
        route.body = Some(merge(&now, &types::of(&body)));
    }
    let status = test.status.unwrap_or(200);
    match test.expect_body.as_deref().map(json::parse) {
        Some(Some(value)) if (200..300).contains(&status) => {
            route.answer = merge(&route.answer, &types::of(&value));
        }
        Some(Some(value)) => route.failure = merge(&route.failure, &types::of(&value)),
        Some(None) => route.json = false,
        None => {}
    }
    if !route.served_by.contains(&test.file) {
        route.served_by.push(test.file.clone());
    }
}

pub(crate) fn routes(top: &Constraints, tests: &[Test], findings: &mut Vec<Finding>) -> Vec<Route> {
    let mut out: Vec<Route> = Vec::new();
    for (method, path, line) in &top.routes {
        if out.iter().any(|r| r.method == *method && r.path == *path) {
            findings.push(Finding {
                file: "idea.md".to_string(),
                line: *line,
                code: "route-duplicate",
                message: format!("the route `{method} {path}` is in the idea two times"),
            });
            continue;
        }
        let mut route = Route {
            method: method.clone(),
            path: path.clone(),
            line: *line,
            function: function_name(method, path),
            json: true,
            body: None,
            answer: Ty::Unknown,
            failure: Ty::Unknown,
            served_by: Vec::new(),
        };
        for test in tests.iter().filter(|t| !t.screen) {
            let found = test.request.as_deref().and_then(split_request);
            if found.is_some_and(|(m, p)| m == method && matches(path, p)) {
                add_example(&mut route, test);
            }
        }
        if route.served_by.is_empty() {
            findings.push(Finding {
                file: "idea.md".to_string(),
                line: *line,
                code: "route-untested",
                message: format!(
                    "no example requests the route `{method} {path}`, so its answers have no shape; add an example with `- Request: `{method} ...``"
                ),
            });
        }
        out.push(route);
    }
    out
}

fn type_name(route: &Route, part: &str) -> String {
    format!("{}{part}", pascal(&route.function))
}

fn answer_types(route: &Route) -> (String, String) {
    if route.json {
        (render(&route.answer), render(&route.failure))
    } else {
        ("string".to_string(), "string".to_string())
    }
}

fn path_expression(route: &Route) -> String {
    if !route.path.contains('{') {
        return quoted(&route.path);
    }
    let parts: Vec<String> = route
        .path
        .split('/')
        .map(|part| match param(part) {
            Some(p) => format!("${{encodeURIComponent({})}}", camel(p)),
            None => part.to_string(),
        })
        .collect();
    format!("`{}`", parts.join("/"))
}

fn validator(out: &mut String, name: &str, check: &str) {
    let value = if check == "true" { "_value" } else { "value" };
    let _ = writeln!(
        out,
        "\nfunction is{name}({value}: unknown): {value} is {name} {{\n  return {check};\n}}"
    );
}

fn route_code(route: &Route, out: &mut String) {
    let answer = type_name(route, "Answer");
    let failure = type_name(route, "Failure");
    let body = type_name(route, "Body");
    let (answer_ty, failure_ty) = answer_types(route);
    let checks = if route.json {
        (
            types::check(&route.answer, "value", 0),
            types::check(&route.failure, "value", 0),
        )
    } else {
        (
            "typeof value === \"string\"".to_string(),
            "typeof value === \"string\"".to_string(),
        )
    };
    let _ = writeln!(out, "\nexport type {answer} = {answer_ty};");
    let _ = writeln!(out, "\nexport type {failure} = {failure_ty};");
    let mut params: Vec<String> = route
        .path
        .split('/')
        .filter_map(param)
        .map(|p| format!("{}: string", camel(p)))
        .collect();
    let sent = if let Some(ty) = &route.body {
        let _ = writeln!(out, "\nexport type {body} = {};", render(ty));
        params.push(format!("body: {body}"));
        "body"
    } else {
        "undefined"
    };
    validator(out, &answer, &checks.0);
    validator(out, &failure, &checks.1);
    let label = format!("`{} {}`", route.method, route.path);
    let _ = writeln!(
        out,
        "\nexport async function {}({}): Promise<Answer<{answer}, {failure}>> {{\n  const got = await call({}, {}, {sent}, {});\n  if (got.ok) {{\n    assert(is{answer}(got.value), {});\n    return {{ ok: true, status: got.status, value: got.value }};\n  }}\n  assert(is{failure}(got.value), {});\n  return {{ ok: false, status: got.status, error: got.value }};\n}}",
        route.function,
        params.join(", "),
        quoted(&route.method),
        path_expression(route),
        route.json,
        quoted(&format!(
            "the answer of {label} does not have the shape of its examples"
        )),
        quoted(&format!(
            "the failure of {label} does not have the shape of its examples"
        )),
    );
}

pub(crate) fn client(routes: &[Route]) -> String {
    let mut out = CORE.to_string();
    for route in routes {
        route_code(route, &mut out);
    }
    out
}

fn literal(value: &StateValue) -> String {
    if value.text {
        quoted(&value.value)
    } else {
        value.value.clone()
    }
}

#[must_use]
pub fn screen_runs(tests: &[Test]) -> Vec<ScreenRun> {
    let mut out = Vec::new();
    for test in tests.iter().filter(|t| t.screen) {
        let screen = test
            .file
            .strip_prefix("ideas/ui/")
            .and_then(|f| f.strip_suffix(".md"))
            .unwrap_or("")
            .to_string();
        for case in cases(test) {
            let mut name = format!("{}: {}", test.behaviour, test.name);
            if !case.values.is_empty() {
                let values: Vec<String> = case
                    .values
                    .iter()
                    .map(|(n, v)| format!("{n} = {v:?}"))
                    .collect();
                let _ = write!(name, " ({})", values.join(", "));
            }
            out.push(ScreenRun {
                index: out.len(),
                file: test.file.clone(),
                line: test.line,
                name,
                screen: screen.clone(),
                test: case.test,
            });
        }
    }
    out
}

fn test_body(run: &ScreenRun, used: &mut Vec<&'static str>) -> String {
    let test = &run.test;
    let state: Vec<String> = test
        .state
        .iter()
        .map(|s| format!("{}: {}", quoted(&s.field), literal(s)))
        .collect();
    let element = format!("<{} />", component_name(&run.screen));
    let mut out = String::new();
    used.push("show");
    let start = format!("show({element}, {{ {} }});", state.join(", "));
    if test.expect_state.is_empty() {
        let _ = writeln!(out, "  {start}");
    } else {
        let _ = writeln!(out, "  const record = {start}");
    }
    for (label, text) in &test.typed {
        used.push("typeInto");
        let _ = writeln!(out, "  typeInto({}, {});", quoted(label), quoted(text));
    }
    if let Some(name) = &test.click {
        used.push("click");
        let _ = writeln!(out, "  click({});", quoted(name));
    }
    for value in &test.expect_state {
        used.push("expectState");
        let _ = writeln!(
            out,
            "  await expectState(record, {}, {});",
            quoted(&value.field),
            literal(value)
        );
    }
    if let Some(text) = &test.expect_text {
        used.push("expectText");
        let _ = writeln!(out, "  await expectText({});", quoted(text));
    }
    if let Some(label) = &test.expect_label {
        used.push("expectLabel");
        let _ = writeln!(out, "  await expectLabel({});", quoted(label));
    }
    out
}

fn screen_test(screen: &str, runs: &[&ScreenRun]) -> String {
    let mut used: Vec<&'static str> = Vec::new();
    let mut tests = String::new();
    for run in runs {
        let body = test_body(run, &mut used);
        let title = quoted(&format!("{} {}", run.index, run.name));
        let _ = write!(tests, "\ntest({title}, async () => {{\n{body}}});\n");
    }
    used.sort_unstable();
    used.dedup();
    format!(
        "import {{ test }} from \"vitest\";\nimport {{ {} }} from \"./lengua\";\nimport {{ {} }} from \"@/screens/{screen}\";\n{tests}",
        used.join(", "),
        component_name(screen)
    )
}

fn server_examples(tests: &[Test]) -> String {
    let mut out = String::from(
        "export interface Example {\n  readonly method: string;\n  readonly path: string;\n  readonly body: string | null;\n  readonly status: number;\n  readonly answer: string;\n}\n\nexport const examples: readonly Example[] = [\n",
    );
    for test in tests.iter().filter(|t| !t.screen && t.request.is_some()) {
        for case in cases(test) {
            let t = &case.test;
            let Some((method, path)) = t.request.as_deref().and_then(|r| r.split_once(' ')) else {
                continue;
            };
            let body = t.request_body.as_deref().map_or("null".to_string(), quoted);
            let _ = writeln!(
                out,
                "  {{ method: {}, path: {}, body: {body}, status: {}, answer: {} }},",
                quoted(method),
                quoted(path),
                t.status.unwrap_or(200),
                quoted(t.expect_body.as_deref().unwrap_or(""))
            );
        }
    }
    out.push_str("];\n");
    out
}

fn json_object(pairs: &[(String, String)]) -> String {
    let lines: Vec<String> = pairs
        .iter()
        .map(|(k, v)| format!("    {}: {}", quoted(k), quoted(v)))
        .collect();
    format!("{{\n{}\n  }}", lines.join(",\n"))
}

fn package_json(title: &str, dependencies: &[(String, String)]) -> String {
    let mut dev: Vec<(String, String)> = DEV
        .iter()
        .map(|(n, v)| ((*n).to_string(), (*v).to_string()))
        .collect();
    dev.sort();
    format!(
        "{{\n  \"name\": {},\n  \"version\": \"0.1.0\",\n  \"private\": true,\n  \"packageManager\": \"pnpm@{PNPM}\",\n  \"dependencies\": {},\n  \"devDependencies\": {}\n}}\n",
        quoted(&format!("{title}-web")),
        json_object(dependencies),
        json_object(&dev)
    )
}

fn layout(title: &str) -> String {
    format!(
        "import type {{ ReactNode }} from \"react\";\nimport \"./globals.css\";\n\nexport const metadata = {{ title: {} }};\n\nexport default function RootLayout({{ children }}: {{ readonly children: ReactNode }}) {{\n  return (\n    <html lang=\"en\">\n      <body>{{children}}</body>\n    </html>\n  );\n}}\n",
        quoted(title)
    )
}

fn route_page(screen: &str) -> String {
    let component = component_name(screen);
    format!(
        "\"use client\";\n\nimport {{ {component} }} from \"@/screens/{screen}\";\n\nexport default function Page() {{\n  return <{component} />;\n}}\n"
    )
}

fn packages(
    pages: &[Page],
    constraints: &[Constraints],
    shadcn: bool,
    findings: &mut Vec<Finding>,
) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = BASE
        .iter()
        .map(|(n, v)| ((*n).to_string(), (*v).to_string()))
        .collect();
    if shadcn {
        found.extend(
            SHADCN_PACKAGES
                .iter()
                .map(|(n, v)| ((*n).to_string(), (*v).to_string())),
        );
    }
    for (page, constraint) in pages.iter().zip(constraints) {
        for used in &constraint.packages {
            let base = DEV.iter().find(|(n, _)| *n == used.name).map(|(_, v)| *v);
            let known = found
                .iter()
                .find(|(n, _)| *n == used.name)
                .map(|(_, v)| v.as_str())
                .or(base);
            match known {
                Some(version) if version != used.version => findings.push(Finding {
                    file: page.file.clone(),
                    line: used.line,
                    code: "package-version",
                    message: format!(
                        "the package `{}` is at {} here and at {version} elsewhere (in another idea, or in the packages every front end has); use one version",
                        used.name, used.version
                    ),
                }),
                Some(_) => {}
                None => found.push((used.name.clone(), used.version.clone())),
            }
        }
    }
    found.sort();
    found
}

fn previous_sums(folder: &Path, stamp: Option<&str>) -> Vec<(String, String)> {
    let Ok(bytes) = fs::read(folder.join(SUM)) else {
        return Vec::new();
    };
    if stamp != Some(sha256::hex(&bytes).as_str()) {
        return Vec::new();
    }
    String::from_utf8_lossy(&bytes)
        .lines()
        .filter_map(|l| l.split_once("  "))
        .map(|(h, p)| (p.to_string(), h.to_string()))
        .collect()
}

fn unlisted(folder: &Path, generated: &[String], sources: &[String]) -> Vec<Finding> {
    let mut found = Vec::new();
    collect_skipping(folder, "web", &SKIPPED, &mut found);
    found
        .into_iter()
        .map(|(path, _)| path)
        .filter(|path| {
            Path::new(path)
                .extension()
                .is_some_and(|e| SCRIPT_EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
        })
        .filter(|path| !generated.contains(path) && !sources.contains(path))
        .map(|path| Finding {
            file: path.clone(),
            line: 1,
            code: "ts-source-unlisted",
            message: format!(
                "`{path}` is not the file of a screen idea and not a file check writes, so no idea documents it; move its code to a screen's file in `web/screens/`, or write a screen idea for it"
            ),
        })
        .collect()
}

fn texts(
    title: &str,
    screens: &[String],
    top: &Constraints,
    routes: &[Route],
    tests: &[Test],
    dependencies: &[(String, String)],
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![(
        "web/package.json".to_string(),
        package_json(title, dependencies),
    )];
    out.extend(
        STATIC
            .iter()
            .map(|(f, t)| ((*f).to_string(), (*t).to_string())),
    );
    out.push(("web/app/layout.tsx".to_string(), layout(title)));
    for screen in screens {
        let file = if screen == "home" {
            "web/app/page.tsx".to_string()
        } else {
            format!("web/app/{screen}/page.tsx")
        };
        out.push((file, route_page(screen)));
    }
    if !top.shadcn.is_empty() {
        out.push((
            "web/lib/utils.ts".to_string(),
            include_str!("front/utils.ts").to_string(),
        ));
    }
    for (name, text) in SHADCN_FILES {
        if top.shadcn.iter().any(|(n, _)| n == name) {
            out.push((format!("web/components/ui/{name}.tsx"), text.to_string()));
        }
    }
    out.push((CLIENT.to_string(), client(routes)));
    out.push(("web/tests/lengua.tsx".to_string(), HELPER.to_string()));
    out.push(("web/tests/server.ts".to_string(), server_examples(tests)));
    let runs = screen_runs(tests);
    for screen in screens {
        let mine: Vec<&ScreenRun> = runs.iter().filter(|r| r.screen == *screen).collect();
        if !mine.is_empty() {
            out.push((
                format!("web/tests/{screen}.test.tsx"),
                screen_test(screen, &mine),
            ));
        }
    }
    out
}

pub(crate) fn plan(
    folder: &Path,
    title: &str,
    pages: &[Page],
    constraints: &[Constraints],
    (routes, tests): (&[Route], &[Test]),
    findings: &mut Vec<Finding>,
) -> Option<WebPlan> {
    let screens: Vec<String> = pages
        .iter()
        .filter(|p| p.kind == IdeaKind::Screen)
        .map(|p| p.name.strip_prefix("ui/").unwrap_or(&p.name).to_string())
        .collect();
    if screens.is_empty() {
        return None;
    }
    let top = constraints.first().cloned().unwrap_or_default();
    let dependencies = packages(pages, constraints, !top.shadcn.is_empty(), findings);
    let made = texts(title, &screens, &top, routes, tests, &dependencies);
    let stamp = pages.first().and_then(|p| p.front.value("web_sha256"));
    let previous = previous_sums(folder, stamp);
    let mut sum = String::new();
    for (file, text) in &made {
        let last = previous
            .iter()
            .find(|(p, _)| p == file)
            .map(|(_, h)| h.as_str());
        findings.extend(held(folder, file, text, last));
        let _ = writeln!(sum, "{}  {file}", sha256::hex(text.as_bytes()));
    }
    findings.extend(held(folder, SUM, &sum, stamp));
    let sources: Vec<String> = pages
        .iter()
        .filter(|p| p.kind == IdeaKind::Screen)
        .map(|p| p.source.clone())
        .collect();
    let mut generated: Vec<String> = made.iter().map(|(f, _)| f.clone()).collect();
    generated.push(SUM.to_string());
    findings.extend(unlisted(folder, &generated, &sources));
    let hash = sha256::hex(sum.as_bytes());
    let mut written = made;
    written.push((SUM.to_string(), sum));
    Some(WebPlan {
        texts: written,
        stamp: Stamp {
            key: "web_sha256",
            file: SUM,
            hash,
        },
        front: Front {
            screens,
            sources,
            generated,
            packages: dependencies,
            runs: screen_runs(tests),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{component_name, function_name, matches, screen_route};

    #[test]
    fn names_come_from_routes_and_screens() {
        assert_eq!(function_name("GET", "/api/notes"), "getNotes");
        assert_eq!(function_name("GET", "/api/notes/{id}"), "getNotesById");
        assert_eq!(function_name("POST", "/api/ui"), "postUi");
        assert_eq!(function_name("GET", "/"), "getRoot");
        assert_eq!(component_name("note_form"), "NoteFormScreen");
        assert_eq!(screen_route("home"), "/");
        assert_eq!(screen_route("note"), "/note/");
        assert!(matches("/api/notes/{id}", "/api/notes/2"));
        assert!(!matches("/api/notes/{id}", "/api/notes"));
        assert!(!matches("/api/notes/{id}", "/api/notes/"));
    }
}
