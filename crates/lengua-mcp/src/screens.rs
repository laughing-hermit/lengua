use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use lengua_idea::{Build, Case, Front, Step, Value};

use crate::build::{Problem, Worker, cargo, problem, problems};

pub(crate) const FOLDER: &str = "target/lengua/screens";
pub(crate) const MUTANT_FOLDER: &str = "target/lengua/mutant-screens";
const LEFT_OUT: [&str; 2] = ["gpui-pre-platform", "gpui-pre-web"];
const TEST_SUPPORT: [&str; 2] = ["gpui-pre", "gpui-component"];
const MARK: &str = "lengua-screen ";

pub(crate) fn cases(build: &Build) -> Vec<Case> {
    build
        .tests
        .iter()
        .filter(|t| t.screen)
        .flat_map(lengua_idea::cases)
        .collect()
}

pub(crate) struct Spec<'a> {
    pub package: String,
    pub ui: &'a str,
    pub libraries: &'a str,
}

pub(crate) fn manifest(front: &Front, spec: &Spec) -> String {
    let mut lines: Vec<(String, String)> = front
        .crates
        .iter()
        .filter(|(name, _)| !LEFT_OUT.contains(&name.as_str()))
        .map(|(name, version)| {
            let features = if TEST_SUPPORT.contains(&name.as_str()) {
                ", features = [\"test-support\"]"
            } else {
                ""
            };
            (
                name.clone(),
                format!(
                    "{name} = {{ version = \"={version}\", default-features = false{features} }}"
                ),
            )
        })
        .collect();
    lines.extend(front.libraries.iter().map(|(name, path)| {
        (
            name.clone(),
            format!("{name} = {{ path = \"{}/{path}\" }}", spec.libraries),
        )
    }));
    lines.sort();
    let dependencies: Vec<String> = lines.into_iter().map(|(_, line)| line).collect();
    format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[[bin]]\nname = \"{name}\"\npath = \"main.rs\"\n\n[dependencies]\n{}\n\n[workspace]\n",
        dependencies.join("\n"),
        name = spec.package,
    )
}

const SUPPORT: &str = r#"use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use gpui::{
    AnyView, AppContext, Bounds, Context, DevicePixels, Entity, Font, FontId, FontMetrics,
    FontRun, GlyphId, IntoElement, LineLayout, Modifiers, NoopTextSystem, ParentElement, Pixels,
    PlatformTextSystem, Render, RenderGlyphParams, Size, Styled, TestAppContext, TestDispatcher,
    TextRenderingMode, VisualTestContext, Window, div,
};

struct Shown {
    noop: NoopTextSystem,
    lines: Mutex<Vec<String>>,
}

impl PlatformTextSystem for Shown {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> gpui::Result<()> {
        self.noop.add_fonts(fonts)
    }
    fn all_font_names(&self) -> Vec<String> {
        self.noop.all_font_names()
    }
    fn font_id(&self, descriptor: &Font) -> gpui::Result<FontId> {
        self.noop.font_id(descriptor)
    }
    fn font_metrics(&self, font_id: FontId) -> FontMetrics {
        self.noop.font_metrics(font_id)
    }
    fn typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> gpui::Result<Bounds<f32>> {
        self.noop.typographic_bounds(font_id, glyph_id)
    }
    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> gpui::Result<Size<f32>> {
        self.noop.advance(font_id, glyph_id)
    }
    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId> {
        self.noop.glyph_for_char(font_id, ch)
    }
    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> gpui::Result<Bounds<DevicePixels>> {
        self.noop.glyph_raster_bounds(params)
    }
    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> gpui::Result<(Size<DevicePixels>, Vec<u8>)> {
        self.noop.rasterize_glyph(params, raster_bounds)
    }
    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        if let Ok(mut lines) = self.lines.lock() {
            lines.push(text.to_string());
        }
        self.noop.layout_line(text, font_size, runs)
    }
    fn recommended_rendering_mode(&self, font_id: FontId, font_size: Pixels) -> TextRenderingMode {
        self.noop.recommended_rendering_mode(font_id, font_size)
    }
}

struct Probe {
    screen: AnyView,
    visible: bool,
}

impl Render for Probe {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if self.visible {
            div().size_full().child(self.screen.clone())
        } else {
            div().size_full()
        }
    }
}

struct Run<V: Render + 'static> {
    _app: TestAppContext,
    cx: VisualTestContext,
    shown: Arc<Shown>,
    screen: Entity<V>,
    probe: Entity<Probe>,
}

impl<V: Render + 'static> Run<V> {
    fn open(make: impl FnOnce(&mut Window, &mut Context<V>) -> V) -> Self {
        let shown = Arc::new(Shown {
            noop: NoopTextSystem,
            lines: Mutex::new(Vec::new()),
        });
        let mut app =
            TestAppContext::build_with_text_system(TestDispatcher::new(0), None, shown.clone());
        let mut slot = None;
        let window = open_window(&mut app, |window, cx| {
            let screen = cx.new(|cx| make(window, cx));
            let probe = cx.new(|_| Probe {
                screen: screen.clone().into(),
                visible: true,
            });
            slot = Some((screen, probe.clone()));
            probe
        });
        let (screen, probe) = slot.expect("the screen");
        let cx = VisualTestContext::from_window(window, &app);
        let mut run = Self {
            _app: app,
            cx,
            shown,
            screen,
            probe,
        };
        run.settle();
        run
    }

    fn settle(&mut self) {
        self.cx.run_until_parked();
        self.cx.update(|window, _| window.refresh());
        self.cx.run_until_parked();
    }

    fn state(&mut self, change: impl FnOnce(&mut V)) {
        self.screen.update(&mut self.cx, |screen, cx| {
            change(screen);
            cx.notify();
        });
        self.settle();
    }

    fn click(&mut self, element: &'static str) -> Result<(), String> {
        self.settle();
        let bounds = self.cx.debug_bounds(element).ok_or_else(|| {
            format!("the screen shows no element `{element}`; give it `.debug_selector(|| \"{element}\".to_string())`")
        })?;
        self.cx.simulate_click(bounds.center(), Modifiers::none());
        self.settle();
        Ok(())
    }

    fn type_into(&mut self, element: &'static str, text: &str) -> Result<(), String> {
        self.click(element)?;
        self.cx.simulate_input(text);
        self.settle();
        Ok(())
    }

    fn expect_state(&mut self, field: &str, read: impl FnOnce(&V) -> String, wanted: &str) -> Result<(), String> {
        self.settle();
        let got = self.screen.read_with(&self.cx, |screen, _| read(screen));
        if got == wanted {
            Ok(())
        } else {
            Err(format!("state `{field}`: expected {wanted:?} and got {got:?}"))
        }
    }

    fn shown_now(&mut self) -> Vec<String> {
        self.probe.update(&mut self.cx, |probe, cx| {
            probe.visible = false;
            cx.notify();
        });
        self.settle();
        if let Ok(mut lines) = self.shown.lines.lock() {
            lines.clear();
        }
        self.probe.update(&mut self.cx, |probe, cx| {
            probe.visible = true;
            cx.notify();
        });
        self.settle();
        self.shown.lines.lock().map(|l| l.clone()).unwrap_or_default()
    }

    fn expect_text(&mut self, wanted: &str) -> Result<(), String> {
        let shown = self.shown_now();
        if shown.iter().any(|line| line.contains(wanted)) || shown.join("\n").contains(wanted) {
            Ok(())
        } else {
            Err(format!("text: expected {wanted:?} on the screen, and it shows {shown:?}"))
        }
    }
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let only: Option<usize> = std::env::args().nth(1).and_then(|a| a.parse().ok());
    for (index, case) in CASES.iter().enumerate() {
        if only.is_some_and(|o| o != index) {
            continue;
        }
        let result = std::panic::catch_unwind(case).unwrap_or_else(|panic| {
            let text = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_else(|| "a panic".to_string());
            Err(format!("the screen panicked: {text}"))
        });
        match result {
            Ok(()) => println!("lengua-screen {index} ok"),
            Err(message) => println!("lengua-screen {index} failed {}", message.replace('\n', "\\n")),
        }
    }
}
"#;

const PLAIN_WINDOW: &str = r"
fn open_window(
    app: &mut TestAppContext,
    make: impl FnOnce(&mut Window, &mut Context<Probe>) -> Entity<Probe>,
) -> gpui::AnyWindowHandle {
    let mut made = None;
    let window = app.add_window(|window, cx| {
        let probe = make(window, cx);
        made = Some(probe.clone());
        Holder { probe }
    });
    let _ = made;
    window.into()
}

struct Holder {
    probe: Entity<Probe>,
}

impl Render for Holder {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.probe.clone())
    }
}
";

const KIT_WINDOW: &str = r"
fn open_window(
    app: &mut TestAppContext,
    make: impl FnOnce(&mut Window, &mut Context<gpui_component::Root>) -> Entity<Probe>,
) -> gpui::AnyWindowHandle {
    app.update(gpui_component::init);
    let window = app.add_window(|window, cx| {
        let probe = make(window, cx);
        gpui_component::Root::new(probe, window, cx)
    });
    window.into()
}
";

fn value(value: &Value) -> String {
    match value {
        Value::Text(text) => format!("{text:?}.into()"),
        Value::Integer(number) => number.clone(),
        Value::Boolean(flag) => flag.to_string(),
    }
}

fn step_line(step: &Step) -> String {
    match step {
        Step::State(field, v) => format!("    run.state(|s| s.{field} = {});\n", value(v)),
        Step::Click(element) => format!("    run.click({element:?})?;\n"),
        Step::Type(element, text) => format!("    run.type_into({element:?}, {text:?})?;\n"),
        Step::ExpectState(field, v) => format!(
            "    run.expect_state({field:?}, |s| s.{field}.to_string(), {:?})?;\n",
            v.shown()
        ),
        Step::ExpectText(text) => format!("    run.expect_text({text:?})?;\n"),
    }
}

pub(crate) struct Text {
    pub text: String,
    pub starts: Vec<usize>,
}

pub(crate) fn main_text(front: &Front, ui: &str, cases: &[Case]) -> Text {
    let mut text = String::from(
        "#![allow(unused_imports, unused_braces, unused_mut, dead_code, clippy::all, clippy::pedantic)]\n",
    );
    for screen in &front.screens {
        let _ = writeln!(
            text,
            "#[path = \"{ui}/{}.rs\"]\nmod {};",
            screen.name, screen.name
        );
    }
    text.push_str(SUPPORT);
    text.push_str(if front.kit() {
        KIT_WINDOW
    } else {
        PLAIN_WINDOW
    });
    let _ = writeln!(
        text,
        "\nconst CASES: [fn() -> Result<(), String>; {}] = [{}];",
        cases.len(),
        (0..cases.len())
            .map(|i| format!("case_{i}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut starts = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let screen = front.screen_of(&source_of(front, &case.test.file));
        let (module, kind) = screen.map_or(("missing", "Missing"), |s| {
            (s.name.as_str(), s.type_name.as_str())
        });
        text.push('\n');
        starts.push(text.lines().count() + 1);
        let _ = writeln!(
            text,
            "fn case_{index}() -> Result<(), String> {{\n    let mut run = Run::open(|window, cx| {module}::{kind}::new(window, cx));"
        );
        for step in &case.test.steps {
            text.push_str(&step_line(step));
        }
        text.push_str("    Ok(())\n}\n");
    }
    Text { text, starts }
}

fn source_of(front: &Front, idea: &str) -> String {
    front
        .screens
        .iter()
        .find(|s| s.idea == idea)
        .map_or_else(String::new, |s| s.file.clone())
}

pub(crate) fn write(
    folder: &Path,
    dir: &str,
    front: &Front,
    spec: &Spec,
    cases: &[Case],
) -> Result<Vec<usize>, String> {
    let place = folder.join(dir);
    let fail =
        |e: std::io::Error| format!("lengua could not write the screens' harness `{dir}`: {e}");
    fs::create_dir_all(&place).map_err(fail)?;
    fs::write(place.join("Cargo.toml"), manifest(front, spec)).map_err(fail)?;
    let main = main_text(front, spec.ui, cases);
    fs::write(place.join("main.rs"), &main.text).map_err(fail)?;
    if !place.join("Cargo.lock").is_file()
        && let Ok(locked) = fs::read(folder.join("Cargo.lock"))
    {
        fs::write(place.join("Cargo.lock"), locked).map_err(fail)?;
    }
    Ok(main.starts)
}

fn tidy(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub(crate) fn placed(folder: &Path, dir: &Path, found: Vec<Problem>) -> Vec<Problem> {
    found
        .into_iter()
        .map(|p| {
            if p.file.starts_with("../") || p.file.starts_with("..\\") {
                let whole = tidy(&dir.join(&p.file));
                let inside = whole.strip_prefix(tidy(folder)).map_or_else(
                    |_| p.file.clone(),
                    |r| r.to_string_lossy().replace('\\', "/"),
                );
                Problem { file: inside, ..p }
            } else {
                p
            }
        })
        .collect()
}

fn mapped(found: Vec<Problem>, starts: &[usize], cases: &[Case]) -> Vec<Problem> {
    found
        .into_iter()
        .map(|p| {
            if p.file != "main.rs" {
                return p;
            }
            let index = starts.iter().rposition(|start| *start <= p.line);
            match index.and_then(|i| cases.get(i)) {
                Some(case) => problem(
                    &case.test.file,
                    case.test.line,
                    "harness-error",
                    format!(
                        "the screen example `{}` of the behaviour `{}` does not build against the screen: {}",
                        case.test.name, case.test.behaviour, p.message
                    ),
                ),
                None => p,
            }
        })
        .collect()
}

fn fetched(folder: &Path, dir: &Path, worker: &Worker) -> Result<(), Vec<Problem>> {
    let manifest = fs::read(dir.join("Cargo.toml")).unwrap_or_default();
    let hash = lengua_idea::sha256_hex(&manifest);
    let stamp = folder.join("target/lengua/screens-fetched");
    if fs::read_to_string(&stamp).is_ok_and(|s| s == hash) {
        return Ok(());
    }
    match cargo(dir, &["fetch"], &worker.child) {
        Ok((true, _)) => {
            let _ = fs::write(&stamp, hash);
            Ok(())
        }
        Ok((false, text)) => Err(problems(dir, &text)),
        Err(message) => Err(vec![problem("Cargo.toml", 1, "build-error", message)]),
    }
}

pub(crate) fn binary(folder: &Path, package: &str) -> PathBuf {
    let exe = if cfg!(windows) { ".exe" } else { "" };
    folder
        .join("target")
        .join("debug")
        .join(format!("{package}{exe}"))
}

pub(crate) fn build(
    worker: &Worker,
    dir: &str,
    spec: &Spec,
    cases: &[Case],
) -> Result<PathBuf, Vec<Problem>> {
    let folder = &worker.folder;
    let Some(front) = &worker.build.front else {
        return Err(Vec::new());
    };
    let starts = write(folder, dir, front, spec, cases)
        .map_err(|m| vec![problem("Cargo.toml", 1, "build-error", m)])?;
    let place = folder.join(dir);
    fetched(folder, &place, worker)?;
    let target_dir = folder.join("target").to_string_lossy().to_string();
    let args = [
        "build",
        "--offline",
        "--message-format=short",
        "--target-dir",
        target_dir.as_str(),
    ];
    match cargo(&place, &args, &worker.child) {
        Ok((true, _)) => Ok(binary(folder, &spec.package)),
        Ok((false, text)) => Err(mapped(
            placed(folder, &place, problems(&place, &text)),
            &starts,
            cases,
        )),
        Err(message) => Err(vec![problem("Cargo.toml", 1, "build-error", message)]),
    }
}

pub(crate) fn run(binary: &Path, count: usize) -> Vec<Option<String>> {
    let output = Command::new(binary)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    let text = output
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let mut results: Vec<Option<String>> =
        vec![Some("the screen's test did not report".to_string()); count];
    for line in text.lines() {
        let Some(rest) = line.strip_prefix(MARK) else {
            continue;
        };
        let (index, outcome) = rest.split_once(' ').unwrap_or((rest, ""));
        let Some(slot) = index.parse::<usize>().ok().and_then(|i| results.get_mut(i)) else {
            continue;
        };
        *slot = if outcome == "ok" {
            None
        } else {
            Some(
                outcome
                    .strip_prefix("failed ")
                    .unwrap_or(outcome)
                    .replace("\\n", "\n"),
            )
        };
    }
    results
}

pub(crate) fn finding(case: &Case, detail: &str) -> Problem {
    let drawn = if case.values.is_empty() {
        String::new()
    } else {
        let values: Vec<String> = case
            .values
            .iter()
            .map(|(name, value)| format!("`{name}` = {value}"))
            .collect();
        format!(" (the case with {})", values.join(", "))
    };
    let code = if case.values.is_empty() {
        "test-failed"
    } else {
        "case-failed"
    };
    problem(
        &case.test.file,
        case.test.line,
        code,
        format!(
            "the screen example `{}` of the behaviour `{}`{drawn} failed: {detail}",
            case.test.name, case.test.behaviour
        ),
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use lengua_idea::{Case, Front, Screen, Step, Test, Value};

    use super::{Spec, main_text, manifest, placed, run};
    use crate::build::problem;

    fn front(kit: bool) -> Front {
        let mut crates = vec![
            ("gpui-pre".to_string(), "0.3.12".to_string()),
            ("gpui-pre-platform".to_string(), "0.3.12".to_string()),
            ("wasm-bindgen".to_string(), "0.2.130".to_string()),
        ];
        if kit {
            crates.push(("gpui-component".to_string(), "0.9.1".to_string()));
        }
        Front {
            package: "notes-app".to_string(),
            artifact: String::new(),
            target: "wasm32-unknown-unknown",
            out: "target/lengua/web",
            start: "list".to_string(),
            screens: vec![Screen {
                name: "list".to_string(),
                type_name: "List".to_string(),
                file: "src/ui/list.rs".to_string(),
                idea: "ideas/ui/list.md".to_string(),
            }],
            crates,
            libraries: vec![("json".to_string(), "../../libraries/json".to_string())],
            assets: Vec::new(),
        }
    }

    #[test]
    fn the_harness_has_test_support_and_leaves_the_browser_out() {
        let spec = Spec {
            package: "notes-screens".to_string(),
            ui: "../../../src/ui",
            libraries: "../../..",
        };
        let text = manifest(&front(true), &spec);
        assert!(text.contains("gpui-pre = { version = \"=0.3.12\", default-features = false, features = [\"test-support\"] }"), "{text}");
        assert!(text.contains("gpui-component = { version = \"=0.9.1\", default-features = false, features = [\"test-support\"] }"), "{text}");
        assert!(!text.contains("gpui-pre-platform"), "{text}");
        assert!(
            text.contains("json = { path = \"../../../../../libraries/json\" }"),
            "{text}"
        );
        assert!(text.ends_with("[workspace]\n"), "{text}");
    }

    #[test]
    fn each_step_becomes_a_line_of_its_case() {
        let case = Case {
            values: Vec::new(),
            test: Test {
                file: "ideas/ui/list.md".to_string(),
                line: 9,
                screen: true,
                steps: vec![
                    Step::State("title".to_string(), Value::Text("Milk".to_string())),
                    Step::State("count".to_string(), Value::Integer("2".to_string())),
                    Step::Click("add".to_string()),
                    Step::Type("name".to_string(), "tea".to_string()),
                    Step::ExpectState("count".to_string(), Value::Integer("3".to_string())),
                    Step::ExpectText("3 notes".to_string()),
                ],
                ..Test::default()
            },
        };
        let made = main_text(&front(false), "../../../src/ui", &[case]);
        let text = &made.text;
        assert!(
            text.contains("#[path = \"../../../src/ui/list.rs\"]\nmod list;\n"),
            "{text}"
        );
        assert!(
            text.contains("let mut run = Run::open(|window, cx| list::List::new(window, cx));"),
            "{text}"
        );
        assert!(
            text.contains("    run.state(|s| s.title = \"Milk\".into());\n"),
            "{text}"
        );
        assert!(text.contains("    run.state(|s| s.count = 2);\n"), "{text}");
        assert!(text.contains("    run.click(\"add\")?;\n"), "{text}");
        assert!(
            text.contains("    run.type_into(\"name\", \"tea\")?;\n"),
            "{text}"
        );
        assert!(
            text.contains("    run.expect_state(\"count\", |s| s.count.to_string(), \"3\")?;\n"),
            "{text}"
        );
        assert!(
            text.contains("    run.expect_text(\"3 notes\")?;\n"),
            "{text}"
        );
        assert!(
            text.contains("const CASES: [fn() -> Result<(), String>; 1] = [case_0];"),
            "{text}"
        );
        assert!(text.contains("struct Holder"), "{text}");
        let line = text.lines().nth(made.starts[0] - 1).unwrap_or_default();
        assert!(line.starts_with("fn case_0()"), "{line}");
        assert!(
            main_text(&front(true), "u", &[])
                .text
                .contains("gpui_component::Root::new")
        );
    }

    #[test]
    fn a_path_through_the_harness_is_the_screen_file() {
        let folder = Path::new("/work/notes");
        let dir = folder.join("target/lengua/screens");
        let found = placed(
            folder,
            &dir,
            vec![problem(
                "../../../src/ui/list.rs",
                3,
                "rust-error",
                "no".to_string(),
            )],
        );
        assert_eq!(found[0].file, "src/ui/list.rs");
    }

    #[test]
    fn a_missing_harness_reports_every_case_as_failed() {
        let results = run(Path::new("/no/such/harness"), 2);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(Option::is_some));
    }
}
