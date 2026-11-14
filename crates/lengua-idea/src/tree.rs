use std::fs;
use std::path::Path;

use crate::frontmatter::{self, Front};
use crate::{Finding, Sink, okf, sha256, wiki};

const SKIPPED: [&str; 3] = ["target", "fixtures", "node_modules"];

fn skipped(dir: &Path) -> bool {
    let name = dir
        .file_name()
        .map_or_default(|n| n.to_string_lossy().to_string());
    name.starts_with('.') || SKIPPED.contains(&name.as_str()) || dir.join("SKILL.md").is_file()
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

pub(crate) fn markdown(root: &Path, inside_bundles: bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![String::new()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(root.join(&dir)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let relative = join(&dir, &name);
            if path.is_dir() {
                let nested = !inside_bundles && path.join("idea.md").is_file();
                if !skipped(&path) && !nested {
                    stack.push(relative);
                }
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push(relative);
            }
        }
    }
    out.sort();
    out
}

fn read(root: &Path, file: &str) -> String {
    fs::read_to_string(root.join(file))
        .unwrap_or_default()
        .replace("\r\n", "\n")
}

fn front_of(text: &str, file: &str) -> Front {
    frontmatter::split(text, &mut Sink::new(file))
}

fn parent(file: &str) -> String {
    file.rsplit_once('/')
        .map_or_default(|(dir, _)| dir.to_string())
}

fn last(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

struct App {
    path: String,
    title: String,
    description: String,
    label: String,
    kind: String,
    uses: Vec<String>,
}

fn list(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(okf::unquote)
        .filter(|v| !v.is_empty())
        .collect()
}

fn apps(root: &Path, files: &[String]) -> Vec<App> {
    let mut dirs: Vec<String> = files
        .iter()
        .filter(|f| last(f) == "idea.md")
        .map(|f| parent(f))
        .collect();
    dirs.sort();
    let mut out: Vec<App> = Vec::new();
    for dir in dirs {
        let inside = out
            .iter()
            .any(|a| a.path.is_empty() || dir.starts_with(&format!("{}/", a.path)));
        if inside || dir.is_empty() {
            continue;
        }
        let file = join(&dir, "idea.md");
        let front = front_of(&read(root, &file), &file);
        let value = |key: &str| front.value(key).map_or_default(okf::unquote);
        let title = Some(value("title"))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| last(&dir));
        let uses = list(front.value("uses").unwrap_or_default())
            .into_iter()
            .filter(|id| id.starts_with("../"))
            .map(|id| okf::normal(&format!("{dir}/{id}")))
            .filter_map(|id| id.strip_suffix("/idea").map(str::to_string))
            .collect();
        out.push(App {
            path: dir,
            title,
            description: value("description"),
            label: value("label"),
            kind: value("type"),
            uses,
        });
    }
    out
}

fn kind_names(kind: &str) -> (&'static str, &'static str, &'static str) {
    match kind {
        "lengua Server" => ("Servers", "server", "servers"),
        "lengua Library" => ("Libraries", "library", "libraries"),
        _ => ("Programs", "program", "programs"),
    }
}

const KINDS: [&str; 3] = ["lengua Program", "lengua Server", "lengua Library"];

fn counted(apps: &[&App]) -> String {
    let mut parts = Vec::new();
    for kind in KINDS {
        let n = apps
            .iter()
            .filter(|a| kind_names(&a.kind) == kind_names(kind))
            .count();
        let (_, one, many) = kind_names(kind);
        match n {
            0 => {}
            1 => parts.push(format!("1 {one}")),
            _ => parts.push(format!("{n} {many}")),
        }
    }
    let text = if let Some((tail, head)) = parts.split_last()
        && !head.is_empty()
    {
        format!("{} and {tail}", head.join(", "))
    } else {
        parts.concat()
    };
    match apps.len() {
        1 => format!("1 lengua folder: {text}."),
        n => format!("{n} lengua folders: {text}."),
    }
}

fn groups(apps: &[App]) -> Vec<String> {
    let mut out: Vec<String> = apps.iter().map(|a| parent(&a.path)).collect();
    out.push(String::new());
    out.sort();
    out.dedup();
    out
}

fn index_for(root: &Path, group: &str, files: &[String], apps: &[App]) -> String {
    let from = join(group, "index.md");
    let mut out = String::new();
    if group.is_empty() {
        out.push_str("---\nokf_version: \"0.2\"\n---\n");
    }
    let documents: Vec<String> = files
        .iter()
        .filter(|f| {
            parent(f) == group && !["index.md", "log.md", "idea.md"].contains(&last(f).as_str())
        })
        .map(|f| {
            let front = front_of(&read(root, f), f);
            let title = front
                .value("title")
                .map(okf::unquote)
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| last(f).trim_end_matches(".md").to_string());
            let description = front.value("description").map_or_default(okf::unquote);
            format!("* [{title}]({}) - {description}\n", okf::relative(&from, f))
        })
        .collect();
    section(&mut out, "Documents", &documents);
    if group.is_empty() {
        let folders: Vec<String> = groups(apps)
            .into_iter()
            .filter(|g| !g.is_empty())
            .map(|g| {
                let inside: Vec<&App> = apps.iter().filter(|a| parent(&a.path) == g).collect();
                format!(
                    "* [{g}]({}) - {}\n",
                    okf::relative(&from, &join(&g, "index.md")),
                    counted(&inside)
                )
            })
            .collect();
        section(&mut out, "Folders", &folders);
    }
    for kind in KINDS {
        let lines: Vec<String> = apps
            .iter()
            .filter(|a| parent(&a.path) == group && kind_names(&a.kind) == kind_names(kind))
            .map(|a| app_line(a, &from, apps))
            .collect();
        section(&mut out, kind_names(kind).0, &lines);
    }
    out
}

fn app_line(app: &App, from: &str, apps: &[App]) -> String {
    let link = |a: &App| {
        format!(
            "[{}]({})",
            a.title,
            okf::relative(from, &join(&a.path, "index.md"))
        )
    };
    let users: Vec<String> = apps
        .iter()
        .filter(|other| other.uses.contains(&app.path))
        .map(link)
        .collect();
    let used = if users.is_empty() {
        String::new()
    } else {
        format!(" Used by: {}.", users.join(", "))
    };
    format!(
        "* {} - {} Label: `{}`.{used}\n",
        link(app),
        app.description,
        app.label
    )
}

fn section(out: &mut String, heading: &str, lines: &[String]) {
    if !lines.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("# ");
        out.push_str(heading);
        out.push_str("\n\n");
        out.push_str(&lines.concat());
    }
}

pub(crate) fn indexes(root: &Path) -> Vec<(String, String)> {
    let files = markdown(root, true);
    let apps = apps(root, &files);
    groups(&apps)
        .into_iter()
        .map(|group| {
            let text = index_for(root, &group, &files, &apps);
            (join(&group, "index.md"), text)
        })
        .collect()
}

fn finding(file: &str, line: usize, code: &'static str, message: String) -> Finding {
    Finding {
        file: file.to_string(),
        line,
        code,
        message,
    }
}

fn idea_file(file: &str, apps: &[App]) -> bool {
    apps.iter().any(|a| {
        let rest = file
            .strip_prefix(&format!("{}/", a.path))
            .unwrap_or_default();
        rest == "idea.md"
            || rest.strip_prefix("ideas/").is_some_and(|r| {
                !r.contains('/') || r.strip_prefix("ui/").is_some_and(|s| !s.contains('/'))
            })
    })
}

fn current(root: &Path, file: &str, front: &Front, apps: &[App], out: &mut Vec<Finding>) {
    if !idea_file(file, apps) {
        return;
    }
    let body = wiki::without_links(&front.body);
    let hash = sha256::hex(body.trim_start_matches('\n').as_bytes());
    if front.value("idea_sha256") != Some(hash.as_str()) {
        out.push(finding(
            file,
            1,
            "wiki-stale",
            "the idea changed after its last check, so its wiki page is not current; run `check` on its folder".to_string(),
        ));
    }
    let app = apps
        .iter()
        .filter(|a| file.starts_with(&format!("{}/", a.path)))
        .map(|a| a.path.as_str())
        .max_by_key(|p| p.len())
        .unwrap_or_default();
    let Some(source) = front.value("source") else {
        return;
    };
    let now = fs::read(root.join(join(app, source)))
        .ok()
        .map_or_else(|| "none".to_string(), |bytes| sha256::hex(&bytes));
    if front.value("source_sha256") != Some(now.as_str()) {
        out.push(finding(
            file,
            1,
            "wiki-stale",
            format!("the code `{source}` changed after the last check, so the wiki page is not current; run `check` on its folder"),
        ));
    }
}

fn links(root: &Path, file: &str, text: &str, out: &mut Vec<Finding>) {
    for (line, target) in okf::links(text) {
        let Some(path) = okf::local(&target) else {
            continue;
        };
        let resolved = okf::normal(&join(&parent(file), path));
        if !root.join(&resolved).exists() {
            out.push(finding(
                file,
                line,
                "wiki-link",
                format!("the link `{target}` leads to no file"),
            ));
        }
    }
}

#[must_use]
pub fn findings(root: &Path) -> Vec<Finding> {
    let files = markdown(root, true);
    let apps = apps(root, &files);
    let mut out = Vec::new();
    for file in &files {
        let text = read(root, file);
        let name = last(file);
        let dir = parent(file);
        if name == "index.md" {
            let bundle = dir.is_empty() || apps.iter().any(|a| a.path == dir);
            for (line, message) in okf::index_problems(&text, bundle) {
                out.push(finding(file, line, "okf-index", message));
            }
        } else if name == "log.md" {
            for (line, message) in okf::log_problems(&text) {
                out.push(finding(file, line, "okf-log", message));
            }
        } else {
            let mut sink = Sink::new(file);
            let front = frontmatter::split(&text, &mut sink);
            if front.entries.is_empty() {
                sink.add(
                    1,
                    "okf-type",
                    "an OKF concept starts with frontmatter between two `---` lines".to_string(),
                );
            } else if front
                .value("type")
                .map(okf::unquote)
                .is_none_or(|t| t.is_empty())
            {
                sink.add(
                    1,
                    "okf-type",
                    "an OKF concept's frontmatter has a `type` that is not empty".to_string(),
                );
            }
            okf::check_values(&front, false, &mut sink);
            out.append(&mut sink.findings);
            current(root, file, &front, &apps, &mut out);
        }
        links(root, file, &text, &mut out);
    }
    for app in &apps {
        let folder = root.join(&app.path);
        if !folder.join("src/main.rs").is_file() {
            continue;
        }
        let approval = crate::approval(&folder);
        if !approval.approved() {
            out.push(finding(
                &join(&app.path, "idea.md"),
                1,
                "wiki-unapproved",
                format!(
                    "the folder has code, and the person did not approve its ideas as they are now (the ideas `{}`, the vision `{}`); the tool `approve` records the person's approval",
                    approval.ideas.as_deref().unwrap_or("none"),
                    approval.vision.as_deref().unwrap_or("none")
                ),
            ));
        }
    }
    for (file, text) in indexes(root) {
        if read(root, &file) != text {
            out.push(finding(
                &file,
                1,
                "wiki-index-stale",
                "this index is not the one `lengua-mcp wiki` writes from the folders' frontmatter; run `lengua-mcp wiki` on the repository's root".to_string(),
            ));
        }
    }
    out.sort();
    out
}

pub fn write_indexes(root: &Path) -> Result<Vec<String>, String> {
    let mut written = Vec::new();
    for (file, text) in indexes(root) {
        if read(root, &file) == text {
            continue;
        }
        fs::write(root.join(&file), &text).map_err(|e| format!("{file}: {e}"))?;
        written.push(file);
    }
    Ok(written)
}
