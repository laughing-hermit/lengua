use std::fs;
use std::path::Path;

use crate::Finding;
use crate::constraints::Constraints;
use crate::wiki::Page;

fn rust_files(folder: &Path, relative: &str, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(folder.join(relative)) else {
        return;
    };
    let mut names: Vec<(String, bool)> = entries
        .flatten()
        .map(|e| {
            (
                e.file_name().to_string_lossy().to_string(),
                e.path().is_dir(),
            )
        })
        .collect();
    names.sort();
    for (name, dir) in names {
        let path = format!("{relative}/{name}");
        if dir {
            rust_files(folder, &path, out);
        } else if Path::new(&name).extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn finding(file: &str, code: &'static str, message: String) -> Finding {
    Finding {
        file: file.to_string(),
        line: 1,
        code,
        message,
    }
}

pub(crate) fn lint(
    folder: &Path,
    pages: &[Page],
    structures: &[Vec<(usize, String)>],
    constraints: &[Constraints],
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut sources = Vec::new();
    for (page, structure) in pages.iter().zip(structures) {
        match fs::read(folder.join(&page.source)).map(String::from_utf8) {
            Ok(Ok(text)) => sources.push(lengua_lint::Source {
                file: page.source.clone(),
                idea: page.file.clone(),
                text: text.replace("\r\n", "\n"),
                structure: structure.clone(),
            }),
            Ok(Err(_)) => findings.push(finding(
                &page.source,
                "rust-read",
                "the file is not UTF-8 text".to_string(),
            )),
            Err(_) => findings.push(finding(
                &page.source,
                "rule-12-source-missing",
                format!(
                    "the idea `{}` has no Rust file `{}`; write it, or remove the idea",
                    page.file, page.source
                ),
            )),
        }
    }
    let mut files = Vec::new();
    rust_files(folder, "src", &mut files);
    for file in files {
        if !pages.iter().any(|p| p.source == file) {
            findings.push(finding(
                &file,
                "rule-12-source-unlisted",
                format!(
                    "`{file}` is not the Rust file of an idea, so no idea documents it; move its code to the file of an idea, or write an idea for it in `ideas/` and name it in `- Modules:`"
                ),
            ));
        }
    }
    if folder.join("build.rs").exists() {
        findings.push(finding(
            "build.rs",
            "rule-8-build-script",
            "a `build.rs` runs code at build time that no idea documents; remove it".to_string(),
        ));
    }
    let crates: Vec<String> = constraints
        .iter()
        .flat_map(|c| c.crates.iter().map(|k| k.name.clone()))
        .collect();
    findings.extend(
        lengua_lint::lint(&sources, &crates)
            .into_iter()
            .map(|f| Finding {
                file: f.file,
                line: f.line,
                code: f.code,
                message: f.message,
            }),
    );
    findings
}
