mod behaviours;
mod constraints;
mod frontmatter;
mod generate;
mod idea;
mod rust;
mod sha256;
mod ste;
mod structure;
mod text;
mod wiki;

use std::fs;
use std::path::Path;

use crate::idea::IdeaKind;
use crate::ste::Dictionary;
use crate::wiki::Page;

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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Grants {
    pub reaches: Vec<Reach>,
    pub folders: Vec<Folder>,
    pub hosts: Vec<String>,
    pub memory: u64,
    pub time_ms: u64,
}

impl Grants {
    #[must_use]
    pub fn reach(&self, reach: Reach) -> bool {
        self.reaches.contains(&reach)
    }
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
        memory: top.memory,
        time_ms: top.time_ms,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub notes: Vec<Note>,
    pub written: Vec<String>,
    pub build: Option<Build>,
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
        IdeaKind::Module | IdeaKind::Screen => format!("src/{}.rs", source.name),
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
) -> Option<(Page, constraints::Constraints, Vec<(usize, String)>)> {
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
    let front = frontmatter::split(&text, &mut sink);
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
        title: title.to_string(),
        description: idea.description,
        uses: Vec::new(),
        idea_hash: sha256::hex(front.body_text().as_bytes()),
        front,
        source: source_path,
        source_hash,
        generated: Vec::new(),
    };
    Some((page, idea.constraints, idea.structure))
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
    plan: generate::Plan,
}

fn build(folder: &Path, prepared: &Prepared) -> Build {
    let plan = &prepared.plan;
    Build {
        artifact: generate::artifact(&plan.package, plan.library),
        package: plan.package.clone(),
        target: generate::TARGET,
        crates: plan.crates,
        code: folder.join("src/main.rs").is_file(),
        inputs: generate::inputs(folder),
        grants: grants(&prepared.top),
        world: generate::world_names(&prepared.top.grants)
            .into_iter()
            .map(str::to_string)
            .collect(),
    }
}

fn prepare(folder: &Path) -> Result<Prepared, Vec<Finding>> {
    if !folder.join("idea.md").is_file() {
        return Err(vec![Finding {
            file: "idea.md".to_string(),
            line: 1,
            code: "idea-missing",
            message: "the folder has no `idea.md`".to_string(),
        }]);
    }
    let mut report = Report::default();
    let dictionary = Dictionary::load();
    let mut sink = Sink::new("ideas/");
    let sources = discover(folder, &mut sink);
    report.findings.append(&mut sink.findings);
    let title = fs::canonicalize(folder)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "program".to_string());
    let mut pages = Vec::new();
    let mut constraints = Vec::new();
    let mut structures = Vec::new();
    for source in &sources {
        let page_title = if source.kind == IdeaKind::Program {
            title.as_str()
        } else {
            source.name.as_str()
        };
        if let Some((page, constraint, structure)) = read_page(
            folder,
            source,
            page_title,
            &dictionary,
            &mut report.findings,
        ) {
            pages.push(page);
            constraints.push(constraint);
            structures.push(structure);
        }
    }
    if pages.first().is_some_and(|p| p.kind == IdeaKind::Program) {
        link(&mut pages, &constraints, &mut report.findings);
    }
    if folder.join("src/main.rs").is_file() {
        report
            .findings
            .extend(rust::lint(folder, &pages, &structures, &constraints));
    }
    report.findings.sort();
    if !report.findings.is_empty() {
        return Err(report.findings);
    }
    let plan = generate::plan(folder, &title, &pages, &constraints, &mut report.findings);
    report.findings.sort();
    if !report.findings.is_empty() {
        return Err(report.findings);
    }
    Ok(Prepared {
        pages,
        top: constraints.into_iter().next().unwrap_or_default(),
        plan,
    })
}

#[must_use]
pub fn check(folder: &Path, now: u64) -> Report {
    let mut report = Report::default();
    let mut prepared = match prepare(folder) {
        Ok(prepared) => prepared,
        Err(findings) => {
            report.findings = findings;
            return report;
        }
    };
    if let Some(top) = prepared.pages.first_mut() {
        top.generated.clone_from(&prepared.plan.stamps);
    }
    let written = wiki::write(folder, &prepared.pages, &prepared.plan.texts, now);
    report.written = written.files;
    report.notes = written.notes;
    report.findings = written.findings;
    if report.findings.is_empty() {
        report.build = Some(build(folder, &prepared));
    }
    report
}

#[must_use]
pub fn program(folder: &Path) -> Report {
    match prepare(folder) {
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
