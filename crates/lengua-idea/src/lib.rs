mod approval;
mod behaviours;
mod cases;
mod constraints;
mod dependencies;
mod frontmatter;
mod generate;
mod idea;
mod json;
mod library;
pub mod lock;
mod okf;
mod rust;
mod sha256;
mod ste;
mod structure;
mod text;
mod tree;
mod types;
mod web;
mod wiki;

use std::fs;
use std::path::Path;

use crate::idea::{Idea, IdeaKind};
use crate::ste::Dictionary;
use crate::wiki::Page;

pub use crate::approval::{Approval, Approved, Record, VISION, approval, approve};
pub use crate::cases::{CASES_DRAWN, Case, End, Range, StateValue, Test, cases};
pub use crate::generate::ASSETS;
pub use crate::library::Library;
pub use crate::tree::{findings as wiki_findings, write_indexes as write_wiki_indexes};
pub use crate::web::{
    Browser, BrowserFn, CLIENT, Front, PNPM, SUM, ScreenRun, browser_functions, component_name,
    screen_route, screen_runs,
};
pub use lengua_lint::{Edit, Mutant, Mutation, apply, choose};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub file: String,
    pub line: usize,
    pub code: &'static str,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub file: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Label {
    #[default]
    Unchecked,
    Tested,
    Checked,
}

impl Label {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Unchecked => "unchecked",
            Self::Tested => "tested",
            Self::Checked => "checked",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Trust {
    pub label: Label,
    pub score: Option<(usize, usize)>,
}

impl Trust {
    #[must_use]
    pub fn text(&self) -> String {
        let label = format!("label `{}`", self.label.name());
        match self.score {
            Some((_, 0)) => format!("{label}, no mutant counted"),
            Some((caught, counted)) => format!(
                "{label}, mutation score {caught} of {counted} ({}%)",
                caught * 100 / counted
            ),
            None => label,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder {
    pub path: String,
    pub write: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    StandardInput,
    Arguments,
    Clock,
    Random,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ai {
    pub calls: u64,
    pub tokens: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Grants {
    pub reaches: Vec<Reach>,
    pub folders: Vec<Folder>,
    pub hosts: Vec<String>,
    pub listens: Vec<u16>,
    pub server: Option<u16>,
    pub memory: u64,
    pub time_ms: u64,
    pub ai: Option<Ai>,
    pub calls: Vec<String>,
    pub secrets: Vec<String>,
}

impl Grants {
    #[must_use]
    pub fn reach(&self, reach: Reach) -> bool {
        self.reaches.contains(&reach)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    Command,
    Server,
    Library,
}

impl Shape {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Command => "program",
            Self::Server => "server",
            Self::Library => "library",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trusted {
    pub file: String,
    pub built: String,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Build {
    pub package: String,
    pub artifact: String,
    pub target: &'static str,
    pub crates: usize,
    pub code: bool,
    pub inputs: String,
    pub grants: Grants,
    pub world: Vec<String>,
    pub library: bool,
    pub shape: Shape,
    pub libraries: Vec<Library>,
    pub components: Vec<Trusted>,
    pub sources: Vec<String>,
    pub tests: Vec<Test>,
    pub tests_hash: String,
    pub front: Option<Front>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebSource {
    pub file: String,
    pub idea: String,
    pub text: String,
    pub structure: Vec<(usize, String)>,
    pub main: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WebLintInput {
    pub screens: Vec<WebSource>,
    pub packages: Vec<String>,
    pub client: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WebLintOutput {
    pub findings: Vec<Finding>,
    pub calls: Vec<(String, Vec<String>)>,
}

pub type WebLint = fn(&WebLintInput) -> WebLintOutput;

impl Build {
    #[must_use]
    pub fn harness(&self) -> Self {
        Self {
            package: harness_name(&self.package),
            artifact: format!(
                "target/{}/release/{}.wasm",
                self.target,
                harness_name(&self.package)
            ),
            grants: Grants {
                reaches: vec![Reach::Arguments],
                ..self.grants.clone()
            },
            world: generate::world_names(&[])
                .into_iter()
                .map(str::to_string)
                .collect(),
            library: false,
            shape: Shape::Command,
            ..self.clone()
        }
    }
}

#[must_use]
pub fn harness_name(package: &str) -> String {
    format!("{package}-harness")
}

fn grants(top: &constraints::Constraints) -> Grants {
    let pairs = [
        (constraints::Grant::StandardInput, Reach::StandardInput),
        (constraints::Grant::Arguments, Reach::Arguments),
        (constraints::Grant::Clock, Reach::Clock),
        (constraints::Grant::Random, Reach::Random),
    ];
    let mut folders: Vec<Folder> = Vec::new();
    for (path, write) in &top.folders {
        if let Some(found) = folders.iter_mut().find(|f| f.path == *path) {
            found.write |= *write;
        } else {
            folders.push(Folder {
                path: path.clone(),
                write: *write,
            });
        }
    }
    Grants {
        reaches: pairs
            .into_iter()
            .filter(|(grant, _)| top.grants.contains(grant))
            .map(|(_, reach)| reach)
            .collect(),
        folders,
        hosts: top.hosts.clone(),
        listens: top.listens.clone(),
        server: top.serves.map(|(port, _)| port),
        memory: top.memory,
        time_ms: top.time_ms,
        ai: top.ai.map(|(calls, tokens)| Ai { calls, tokens }),
        calls: top.calls.clone(),
        secrets: top.secrets.clone(),
    }
}

fn shape_of(top: &constraints::Constraints) -> Shape {
    if !top.exports.is_empty() {
        Shape::Library
    } else if top.serves.is_some() {
        Shape::Server
    } else {
        Shape::Command
    }
}

fn top_finding(line: usize, code: &'static str, message: String) -> Finding {
    Finding {
        file: "idea.md".to_string(),
        line,
        code,
        message,
    }
}

fn shape_rules(top: &constraints::Constraints, findings: &mut Vec<Finding>) {
    let exports_line = top.exports.first().map(|(_, line)| *line);
    if let (Some(_), Some((_, line))) = (exports_line, top.serves) {
        findings.push(top_finding(
            line,
            "constraint-kind",
            "an idea is a program, a server (`- Serves:`) or a library (`- Exports:`), not two of them".to_string(),
        ));
    }
    if let Some(line) = exports_line {
        let mut reached = Vec::new();
        if !top.grants.is_empty() {
            reached.push("grants");
        }
        if !top.components.is_empty() {
            reached.push("components");
        }
        if !reached.is_empty() {
            findings.push(top_finding(
                line,
                "library-grant",
                format!("a library has no {} of its own: the program that uses it grants what it reaches; remove them, or make the idea a program", reached.join(" and ")),
            ));
        }
    }
    let input = [
        constraints::Grant::StandardInput,
        constraints::Grant::Arguments,
    ];
    if let Some((_, line)) = top.serves
        && top.grants.iter().any(|g| input.contains(g))
    {
        findings.push(top_finding(
            line,
            "server-input",
            "a server gets requests, not a standard input or arguments; remove `- Reads: standard input` and `- Reads: arguments`".to_string(),
        ));
    }
}

fn model_rules(tests: &[Test], asks: bool, findings: &mut Vec<Finding>) {
    for test in tests.iter().filter(|t| t.model.is_some() && !asks) {
        findings.push(Finding {
            file: test.file.clone(),
            line: test.line,
            code: "example-model",
            message: "`- Model answers:` gives the answer of an AI model, and the idea does not grant `- Asks: an AI model, for ...`".to_string(),
        });
    }
}

fn test_rules(tests: &[Test], shape: Shape, findings: &mut Vec<Finding>) {
    for test in tests.iter().filter(|t| !t.screen) {
        let problem = match shape {
            Shape::Library if test.call.is_none() => Some((
                "example-library-call",
                "a library has no entry of its own, so each of its examples is a `- Call:` of its functions with an `- Expect value:`",
            )),
            Shape::Server if test.request.is_none() => Some((
                "example-server-request",
                "a server answers requests, so each of its examples is a `- Request:` with an `- Expect status:`",
            )),
            Shape::Command | Shape::Server if test.call.is_some() => Some((
                "example-library-call",
                "`- Call:` is for the examples of a library, an idea with `- Exports:`",
            )),
            Shape::Command | Shape::Library if test.request.is_some() => Some((
                "example-server-request",
                "`- Request:` is for the examples of a server, an idea with `- Serves:`",
            )),
            _ => None,
        };
        if let Some((code, message)) = problem {
            findings.push(Finding {
                file: test.file.clone(),
                line: test.line,
                code,
                message: message.to_string(),
            });
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub notes: Vec<Note>,
    pub written: Vec<String>,
    pub build: Option<Build>,
    pub trust: Trust,
    pub approval: Approval,
}

#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    sha256::hex(data)
}

pub(crate) struct Sink {
    pub file: String,
    pub findings: Vec<Finding>,
}

impl Sink {
    pub(crate) fn new(file: &str) -> Self {
        Self {
            file: file.to_string(),
            findings: Vec::new(),
        }
    }

    pub(crate) fn add(&mut self, line: usize, code: &'static str, message: String) {
        self.findings.push(Finding {
            file: self.file.clone(),
            line,
            code,
            message,
        });
    }
}

struct Source {
    file: String,
    kind: IdeaKind,
    name: String,
}

fn markdown_files(folder: &Path, prefix: &str, sink: &mut Sink) -> Vec<String> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if !(prefix == "ideas/" && name == "ui") {
                sink.findings.push(Finding {
                    file: format!("{prefix}{name}/"),
                    line: 1,
                    code: "idea-place",
                    message: "ideas are in `ideas/` and screen ideas in `ideas/ui/`; this folder is not read"
                        .to_string(),
                });
            }
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(name);
        }
    }
    out.sort();
    out
}

fn discover(folder: &Path, sink: &mut Sink) -> Vec<Source> {
    let mut out = vec![Source {
        file: "idea.md".to_string(),
        kind: IdeaKind::Program,
        name: "main".to_string(),
    }];
    let places = [
        ("ideas/", IdeaKind::Module, ""),
        ("ideas/ui/", IdeaKind::Screen, "ui/"),
    ];
    for (prefix, kind, module_prefix) in places {
        for file in markdown_files(&folder.join(prefix), prefix, sink) {
            let stem = file.strip_suffix(".md").unwrap_or(&file);
            let name = format!("{module_prefix}{stem}");
            if !constraints::module_name(&name) {
                sink.findings.push(Finding {
                    file: format!("{prefix}{file}"),
                    line: 1,
                    code: "idea-name",
                    message: format!(
                        "`{stem}` is not a module name; use `snake_case`, and not `main`, `ui`, `index` or `log`"
                    ),
                });
                continue;
            }
            out.push(Source {
                file: format!("{prefix}{file}"),
                kind,
                name,
            });
        }
    }
    out
}

fn source_file(source: &Source) -> String {
    match source.kind {
        IdeaKind::Program => "src/main.rs".to_string(),
        IdeaKind::Module => format!("src/{}.rs", source.name),
        IdeaKind::Screen => format!(
            "web/screens/{}.tsx",
            source.name.strip_prefix("ui/").unwrap_or(&source.name)
        ),
    }
}

fn concept(name: &str) -> String {
    format!("ideas/{name}")
}

fn read_page(
    folder: &Path,
    source: &Source,
    title: &str,
    dictionary: &Dictionary,
    findings: &mut Vec<Finding>,
) -> Option<(Page, Idea)> {
    let mut sink = Sink::new(&source.file);
    let text = match fs::read(folder.join(&source.file)).map(String::from_utf8) {
        Ok(Ok(text)) => text.replace("\r\n", "\n"),
        Ok(Err(_)) => {
            sink.add(1, "file-not-utf8", "the file is not UTF-8 text".to_string());
            findings.append(&mut sink.findings);
            return None;
        }
        Err(error) => {
            sink.add(
                1,
                "file-unreadable",
                format!("lengua could not read the file: {error}"),
            );
            findings.append(&mut sink.findings);
            return None;
        }
    };
    let mut front = frontmatter::split(&text, &mut sink);
    front.body = wiki::without_links(&front.body);
    okf::check_values(&front, true, &mut sink);
    let idea = idea::parse(
        source.kind,
        &front.body,
        front.body_line,
        dictionary,
        &mut sink,
    );
    let source_path = source_file(source);
    let source_hash = fs::read(folder.join(&source_path))
        .ok()
        .map(|bytes| sha256::hex(&bytes));
    findings.append(&mut sink.findings);
    let page = Page {
        file: source.file.clone(),
        kind: source.kind,
        name: source.name.clone(),
        title: front
            .value("title")
            .map(okf::unquote)
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| title.to_string()),
        description: idea.description.clone(),
        uses: Vec::new(),
        idea_hash: sha256::hex(front.body_text().as_bytes()),
        front,
        source: source_path,
        source_hash,
        generated: Vec::new(),
    };
    Some((page, idea))
}

fn link(pages: &mut [Page], constraints: &[constraints::Constraints], findings: &mut Vec<Finding>) {
    let names: Vec<String> = pages.iter().map(|p| p.name.clone()).collect();
    let listed: Vec<(String, usize)> = constraints
        .first()
        .map_or_else(Vec::new, |c| c.modules.clone());
    for (k, (name, line)) in listed.iter().enumerate() {
        if listed[..k].iter().any(|(n, _)| n == name) {
            findings.push(Finding {
                file: "idea.md".to_string(),
                line: *line,
                code: "module-duplicate",
                message: format!("`{name}` is in `- Modules:` two times"),
            });
        } else if !names.iter().skip(1).any(|n| n == name) {
            findings.push(Finding {
                file: "idea.md".to_string(),
                line: *line,
                code: "module-missing",
                message: format!(
                    "the module `{name}` has no idea file, `{}.md`",
                    concept(name)
                ),
            });
        }
    }
    for page in pages.iter().skip(1) {
        if !listed.iter().any(|(n, _)| *n == page.name) {
            findings.push(Finding {
                file: page.file.clone(),
                line: 1,
                code: "module-unlisted",
                message: format!(
                    "the module `{}` is not in the `- Modules:` line of `idea.md`",
                    page.name
                ),
            });
        }
    }
    for (page, constraint) in pages.iter_mut().zip(constraints) {
        let mut uses: Vec<String> = Vec::new();
        if page.kind == IdeaKind::Program {
            uses.extend(listed.iter().map(|(n, _)| concept(n)));
        }
        for (name, line) in &constraint.uses {
            if *name == page.name {
                findings.push(Finding {
                    file: page.file.clone(),
                    line: *line,
                    code: "uses-self",
                    message: format!("the module `{name}` uses itself"),
                });
            } else if !names.iter().skip(1).any(|n| n == name) {
                findings.push(Finding {
                    file: page.file.clone(),
                    line: *line,
                    code: "uses-unknown",
                    message: format!("`{name}` is not a module of this program"),
                });
            }
            uses.push(concept(name));
        }
        uses.dedup();
        page.uses = uses;
    }
}

struct Prepared {
    pages: Vec<Page>,
    top: constraints::Constraints,
    shape: Shape,
    libraries: Vec<Library>,
    plan: generate::Plan,
    tests: Vec<Test>,
    web: Option<web::WebPlan>,
    routes: Vec<web::Route>,
    calls: Vec<(String, Vec<String>)>,
}

#[must_use]
pub fn tests_hash(tests: &[Test], grants: &Grants) -> String {
    sha256::hex(format!("{tests:?}\0{grants:?}").as_bytes())
}

fn outside_inputs(folder: &Path, prepared: &Prepared) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = prepared
        .libraries
        .iter()
        .map(|l| (l.lock.clone(), l.lock_sha256.clone().into_bytes()))
        .collect();
    for (file, _, _) in &prepared.top.components {
        out.push((
            file.clone(),
            fs::read(folder.join(file)).unwrap_or_default(),
        ));
    }
    out
}

fn build(folder: &Path, prepared: &Prepared) -> Build {
    let plan = &prepared.plan;
    let grants = grants(&prepared.top);
    let placed: Vec<Test> = prepared
        .tests
        .iter()
        .map(|t| {
            let start = prepared
                .pages
                .iter()
                .find(|p| p.file == t.file)
                .map_or(1, |p| p.front.body_line);
            Test {
                line: t.line.saturating_sub(start),
                ..t.clone()
            }
        })
        .collect();
    let tests_hash = tests_hash(&placed, &grants);
    let mut extra = outside_inputs(folder, prepared);
    if prepared.web.is_some() {
        generate::collect_skipping(folder, "web", &WEB_SKIPPED, &mut extra);
    }
    Build {
        artifact: generate::artifact(&plan.package, prepared.shape),
        package: plan.package.clone(),
        target: generate::TARGET,
        crates: plan.crates,
        code: folder.join("src/main.rs").is_file(),
        inputs: generate::inputs(folder, &plan.texts, &tests_hash, &extra),
        grants,
        world: generate::world_names(&prepared.top.grants)
            .into_iter()
            .map(str::to_string)
            .collect(),
        library: prepared.shape == Shape::Library,
        shape: prepared.shape,
        libraries: prepared.libraries.clone(),
        components: prepared
            .top
            .components
            .iter()
            .map(|(file, built, line)| Trusted {
                file: file.clone(),
                built: built.clone(),
                line: *line,
            })
            .collect(),
        sources: prepared
            .pages
            .iter()
            .filter(|p| p.kind != IdeaKind::Screen)
            .map(|p| p.source.clone())
            .collect(),
        tests: prepared.tests.clone(),
        tests_hash,
        front: prepared.web.as_ref().map(|w| w.front.clone()),
    }
}

const WEB_SKIPPED: [&str; 3] = ["web/node_modules", "web/.next", "web/out"];

struct Read {
    pages: Vec<Page>,
    constraints: Vec<constraints::Constraints>,
    structures: Vec<Vec<(usize, String)>>,
    named: Vec<Vec<(String, usize)>>,
    tests: Vec<Test>,
}

fn read_all(folder: &Path, title: &str, findings: &mut Vec<Finding>) -> Read {
    let dictionary = Dictionary::load();
    let mut sink = Sink::new("ideas/");
    let sources = discover(folder, &mut sink);
    findings.append(&mut sink.findings);
    let mut read = Read {
        pages: Vec::new(),
        constraints: Vec::new(),
        structures: Vec::new(),
        named: Vec::new(),
        tests: Vec::new(),
    };
    for source in &sources {
        let page_title = if source.kind == IdeaKind::Program {
            title
        } else {
            source.name.as_str()
        };
        if let Some((page, idea)) = read_page(folder, source, page_title, &dictionary, findings) {
            read.pages.push(page);
            read.constraints.push(idea.constraints);
            read.structures.push(idea.structure);
            read.named.push(idea.named);
            read.tests.extend(idea.tests);
        }
    }
    read
}

fn screen_rules(read: &Read, shape: Shape, findings: &mut Vec<Finding>) {
    let screens = read.pages.iter().any(|p| p.kind == IdeaKind::Screen);
    let top = read.constraints.first().cloned().unwrap_or_default();
    if screens && shape != Shape::Server {
        findings.push(top_finding(
            1,
            "screen-server",
            "an idea with screens is a server, since its back end serves the front end; add `- Serves: HTTP on TCP port <number>`".to_string(),
        ));
    }
    let front_keys =
        !top.shadcn.is_empty() || read.constraints.iter().any(|c| !c.packages.is_empty());
    if front_keys && !screens {
        findings.push(top_finding(
            1,
            "screen-missing",
            "`- Uses package:` and `- Uses shadcn:` are for the front end, and the idea has no screens in `ideas/ui/`".to_string(),
        ));
    }
    if !top.routes.is_empty() && shape != Shape::Server {
        findings.push(top_finding(
            top.routes.first().map_or(1, |r| r.2),
            "route-server",
            "`- Route:` is for a server, an idea with `- Serves:`".to_string(),
        ));
    }
}

fn web_lint(
    folder: &Path,
    read: &Read,
    routes: &[web::Route],
    lint: WebLint,
    findings: &mut Vec<Finding>,
) -> Vec<(String, Vec<String>)> {
    let mut input = WebLintInput {
        client: routes.iter().map(|r| r.function.clone()).collect(),
        ..WebLintInput::default()
    };
    for constraint in &read.constraints {
        input
            .packages
            .extend(constraint.packages.iter().map(|p| p.name.clone()));
    }
    for (page, structure) in read.pages.iter().zip(&read.structures) {
        if page.kind != IdeaKind::Screen {
            continue;
        }
        let screen = page.name.strip_prefix("ui/").unwrap_or(&page.name);
        match fs::read(folder.join(&page.source)).map(String::from_utf8) {
            Ok(Ok(text)) => input.screens.push(WebSource {
                file: page.source.clone(),
                idea: page.file.clone(),
                text: text.replace("\r\n", "\n"),
                structure: structure.clone(),
                main: component_name(screen),
            }),
            Ok(Err(_)) => findings.push(Finding {
                file: page.source.clone(),
                line: 1,
                code: "ts-read",
                message: "the file is not UTF-8 text".to_string(),
            }),
            Err(_) => findings.push(Finding {
                file: page.source.clone(),
                line: 1,
                code: "rule-12-source-missing",
                message: format!(
                    "the screen idea `{}` has no file `{}`; write it, or remove the idea",
                    page.file, page.source
                ),
            }),
        }
    }
    let output = lint(&input);
    findings.extend(output.findings);
    output.calls
}

fn known_libraries(folder: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(start) = fs::canonicalize(folder) else {
        return out;
    };
    for place in start.ancestors().skip(1) {
        let Ok(entries) = fs::read_dir(place.join("libraries")) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.path().join("idea.md").is_file() {
                out.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    out
}

fn last_name(path: &str) -> String {
    let path = path.trim_end_matches('/');
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn purpose_rules(
    folder: &Path,
    read: &Read,
    top: &constraints::Constraints,
    shape: Shape,
    findings: &mut Vec<Finding>,
) {
    let modules: Vec<&str> = read.pages.iter().skip(1).map(|p| p.name.as_str()).collect();
    let used: Vec<String> = top.libraries.iter().map(|(p, _)| last_name(p)).collect();
    let mut libraries = if shape == Shape::Library {
        Vec::new()
    } else {
        known_libraries(folder)
    };
    libraries.extend(used.iter().cloned());
    for (k, page) in read.pages.iter().enumerate() {
        let listed: Vec<&str> = if k == 0 {
            top.modules.iter().map(|(n, _)| n.as_str()).collect()
        } else {
            read.constraints
                .get(k)
                .map_or_default(|c| c.uses.iter().map(|(n, _)| n.as_str()).collect())
        };
        let key = if k == 0 { "Modules" } else { "Uses" };
        for (name, line) in read.named.get(k).into_iter().flatten() {
            let message = if *name == page.name {
                None
            } else if modules.contains(&name.as_str()) {
                (!listed.contains(&name.as_str())).then(|| format!(
                    "the purpose names the module `{name}`, and the constraints do not list it; add it to `- {key}:`, or name it another way"
                ))
            } else if libraries.contains(name) && !used.contains(name) {
                Some(format!(
                    "the purpose names the lengua library `{name}`, and the top idea does not use it; add its folder to the `- Uses:` of `idea.md`, or name it another way"
                ))
            } else {
                None
            };
            if let Some(message) = message {
                findings.push(Finding {
                    file: page.file.clone(),
                    line: *line,
                    code: "purpose-unlisted",
                    message,
                });
            }
        }
    }
}

fn conformance(
    folder: &Path,
    pages: &[Page],
    components: &[(String, String, usize)],
    findings: &mut Vec<Finding>,
) {
    let mut known: Vec<String> = pages.iter().map(|p| p.file.clone()).collect();
    known.extend(
        components
            .iter()
            .map(|(file, _, _)| wiki::component_page(file)),
    );
    for file in tree::markdown(folder, false) {
        let name = file.rsplit('/').next().unwrap_or_default();
        let reserved = name == "index.md" || name == "log.md";
        if reserved || known.contains(&file) || file.starts_with("ideas/") {
            continue;
        }
        let text = fs::read_to_string(folder.join(&file))
            .unwrap_or_default()
            .replace("\r\n", "\n");
        let mut sink = Sink::new(&file);
        let front = frontmatter::split(&text, &mut sink);
        let typed = front
            .value("type")
            .map(okf::unquote)
            .is_some_and(|t| !t.is_empty());
        if !typed {
            sink.add(
                1,
                "okf-type",
                "every page in the folder but `index.md` and `log.md` is an OKF concept, so it starts with frontmatter that has a `type`; add one, or move the file out of the folder".to_string(),
            );
        }
        okf::check_values(&front, false, &mut sink);
        findings.append(&mut sink.findings);
    }
}

fn component_rules(folder: &Path, top: &constraints::Constraints, findings: &mut Vec<Finding>) {
    for (file, _, line) in &top.components {
        if !folder.join(file).is_file() {
            findings.push(top_finding(
                *line,
                "component-missing",
                format!("the component `{file}` is not there; build it, or remove the line"),
            ));
        }
    }
}

fn folder_title(folder: &Path) -> String {
    fs::canonicalize(folder)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "program".to_string())
}

pub(crate) fn idea_findings(folder: &Path) -> Vec<Finding> {
    if !folder.join("idea.md").is_file() {
        return vec![top_finding(
            1,
            "idea-missing",
            "the folder has no `idea.md`".to_string(),
        )];
    }
    let mut findings = Vec::new();
    let mut read = read_all(folder, &folder_title(folder), &mut findings);
    if read
        .pages
        .first()
        .is_some_and(|p| p.kind == IdeaKind::Program)
    {
        link(&mut read.pages, &read.constraints, &mut findings);
    }
    vision_rules(folder, &mut findings);
    findings.sort();
    findings
}

fn vision_rules(folder: &Path, findings: &mut Vec<Finding>) {
    let Ok(text) = fs::read_to_string(folder.join(VISION)) else {
        return;
    };
    let text = text.replace("\r\n", "\n");
    let mut sink = Sink::new(VISION);
    let front = frontmatter::split(&text, &mut sink);
    if front.value("type").map(okf::unquote).as_deref() != Some("lengua Vision") {
        sink.add(
            1,
            "vision-type",
            "`vision.md` is the OKF page of the vision, so its frontmatter has `type: lengua Vision`".to_string(),
        );
    }
    let mut fence = false;
    let mut found = false;
    for line in front.body.lines() {
        if text::is_fence(line) {
            fence = !fence;
        } else if !fence && line.trim_end() == "# Where it runs" {
            found = true;
        }
    }
    if !found {
        sink.add(
            front.body_line,
            "vision-section",
            "`vision.md` says where the application runs, in a section `# Where it runs`, before any code".to_string(),
        );
    }
    findings.append(&mut sink.findings);
}

fn prepare(folder: &Path, lint: Option<WebLint>) -> Result<Prepared, Vec<Finding>> {
    if !folder.join("idea.md").is_file() {
        return Err(vec![top_finding(
            1,
            "idea-missing",
            "the folder has no `idea.md`".to_string(),
        )]);
    }
    let mut findings = Vec::new();
    let title = folder_title(folder);
    vision_rules(folder, &mut findings);
    let mut read = read_all(folder, &title, &mut findings);
    let program = read
        .pages
        .first()
        .is_some_and(|p| p.kind == IdeaKind::Program);
    if program {
        link(&mut read.pages, &read.constraints, &mut findings);
    }
    let top = read.constraints.first().cloned().unwrap_or_default();
    let shape = shape_of(&top);
    if program {
        shape_rules(&top, &mut findings);
        test_rules(&read.tests, shape, &mut findings);
        model_rules(&read.tests, top.ai.is_some(), &mut findings);
        screen_rules(&read, shape, &mut findings);
    }
    if shape != Shape::Command {
        findings.retain(|f| f.code != "structure-main-missing");
    }
    let libraries = library::resolve(folder, &top.libraries, &mut findings);
    if program {
        purpose_rules(folder, &read, &top, shape, &mut findings);
        conformance(folder, &read.pages, &top.components, &mut findings);
    }
    component_rules(folder, &top, &mut findings);
    let routes = web::routes(&top, &read.tests, &mut findings);
    if folder.join("src/main.rs").is_file() {
        findings.extend(rust::lint(
            folder,
            &read.pages,
            &read.structures,
            &read.constraints,
            &libraries,
        ));
    }
    let calls = match lint {
        Some(lint) => web_lint(folder, &read, &routes, lint, &mut findings),
        None => Vec::new(),
    };
    findings.sort();
    if !findings.is_empty() {
        return Err(findings);
    }
    let mut plan = generate::plan(
        folder,
        &title,
        &read.pages,
        &read.constraints,
        (shape, &libraries),
        &mut findings,
    );
    let web = web::plan(
        folder,
        &title,
        &read.pages,
        &read.constraints,
        (&routes, &read.tests, &libraries),
        &mut findings,
    );
    if let Some(found) = &web {
        plan.texts.extend(found.texts.iter().cloned());
        plan.stamps.push(found.stamp.clone());
    }
    findings.sort();
    if !findings.is_empty() {
        return Err(findings);
    }
    if let Some(first) = read.pages.first_mut() {
        first
            .uses
            .extend(libraries.iter().map(|l| format!("{}/idea", l.path)));
    }
    Ok(Prepared {
        pages: read.pages,
        top,
        shape,
        libraries,
        plan,
        tests: read.tests,
        web,
        routes,
        calls,
    })
}

#[must_use]
pub fn check(folder: &Path, now: u64) -> Report {
    check_with(folder, now, |_| Some(Trust::default()))
}

#[must_use]
pub fn check_with(folder: &Path, now: u64, trust: impl FnOnce(&Build) -> Option<Trust>) -> Report {
    check_web(folder, now, trust, None)
}

#[must_use]
pub fn check_web(
    folder: &Path,
    now: u64,
    trust: impl FnOnce(&Build) -> Option<Trust>,
    lint: Option<WebLint>,
) -> Report {
    let mut report = Report::default();
    let mut prepared = match prepare(folder, lint) {
        Ok(prepared) => prepared,
        Err(findings) => {
            report.findings = findings;
            return report;
        }
    };
    if let Some(top) = prepared.pages.first_mut() {
        top.generated.clone_from(&prepared.plan.stamps);
    }
    let planned = build(folder, &prepared);
    let given = trust(&planned);
    report.trust = given.unwrap_or_default();
    let title = prepared.pages.first().map_or_default(|p| p.title.clone());
    let large = prepared
        .web
        .as_ref()
        .map_or_else(Vec::new, |w| large_packages(folder, &w.front.packages));
    let interface = wiki::Interface {
        routes: &prepared.routes,
        calls: &prepared.calls,
        client_hash: prepared
            .plan
            .texts
            .iter()
            .find(|(f, _)| f == CLIENT)
            .map(|(_, t)| sha256::hex(t.as_bytes())),
        packages: prepared
            .web
            .as_ref()
            .map_or(&[][..], |w| w.front.packages.as_slice()),
        large: &large,
    };
    report.approval = approval(folder);
    let crates = dependencies::count(folder, &prepared.plan.used);
    let native_crates: Vec<Note> = crates
        .native
        .iter()
        .map(|name| Note {
            file: "idea.md".to_string(),
            message: format!(
                "the crate `{name}` is not pure Rust: it builds C or links a native library; take a pure-Rust crate, or say in its `- Uses crate:` reason why none merits it"
            ),
        })
        .collect();
    let parts = wiki::Parts {
        title: &title,
        shape: prepared.shape,
        libraries: &prepared.libraries,
        components: &prepared.top.components,
        crates,
        vision: folder.join(VISION).is_file(),
        interface: &interface,
        approval: &report.approval,
    };
    let written = wiki::write(
        folder,
        &prepared.pages,
        &prepared.plan.texts,
        now,
        given.as_ref(),
        &parts,
    );
    report.written = written.files;
    report.notes = written.notes;
    report.notes.extend(native_crates);
    report.findings = written.findings;
    if report.findings.is_empty() {
        report.build = Some(prepare(folder, lint).map_or(planned, |again| build(folder, &again)));
    }
    report
}

#[must_use]
pub fn program(folder: &Path) -> Report {
    match prepare(folder, None) {
        Ok(prepared) => Report {
            build: Some(build(folder, &prepared)),
            ..Report::default()
        },
        Err(findings) => Report {
            findings,
            ..Report::default()
        },
    }
}

#[must_use]
pub fn mutants(folder: &Path, build: &Build, cap: usize) -> Mutation {
    let sources: Vec<lengua_lint::Source> = build
        .sources
        .iter()
        .filter_map(|file| {
            let text = fs::read_to_string(folder.join(file)).ok()?;
            Some(lengua_lint::Source {
                file: file.clone(),
                idea: String::new(),
                text: text.replace("\r\n", "\n"),
                structure: Vec::new(),
            })
        })
        .collect();
    lengua_lint::mutants(&sources, cap)
}

#[must_use]
pub fn mutant_groups(folder: &Path, build: &Build) -> (Vec<Vec<Mutant>>, String) {
    let sources: Vec<lengua_lint::Source> = build
        .sources
        .iter()
        .filter_map(|file| {
            let text = fs::read_to_string(folder.join(file)).ok()?;
            Some(lengua_lint::Source {
                file: file.clone(),
                idea: String::new(),
                text: text.replace("\r\n", "\n"),
                structure: Vec::new(),
            })
        })
        .collect();
    lengua_lint::groups(&sources)
}

fn dir_size(root: &Path) -> u64 {
    let Ok(start) = fs::canonicalize(root) else {
        return 0;
    };
    let mut stack = vec![start];
    let mut total = 0u64;
    let mut seen = 0usize;
    while let Some(dir) = stack.pop() {
        seen += 1;
        if seen > 100_000 {
            break;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                total += meta.len();
            }
        }
    }
    total
}

fn large_packages(folder: &Path, packages: &[(String, String)]) -> Vec<(String, u64)> {
    let modules = folder.join("target/lengua/web/node_modules");
    packages
        .iter()
        .map(|(name, _)| (name.clone(), dir_size(&modules.join(name))))
        .filter(|(_, size)| *size > 1 << 20)
        .collect()
}
