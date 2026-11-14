use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use lengua_idea::{ASSETS, Front, Mutant};
use rmcp::serde_json::{self, Value};

use crate::build::{Problem, Stage, Worker, problem, tool};
use crate::doctor;

pub(crate) const STAGE: &str = "target/lengua/web";
const KEPT: [&str; 3] = ["node_modules", ".next", "out"];
const REPORT: &str = ".lengua/vitest.json";

type Failure = (Stage, Vec<Problem>);

pub(crate) fn plain(path: &Path) -> String {
    let text = path.to_string_lossy().to_string();
    text.strip_prefix(r"\\?\")
        .map_or_else(|| text.clone(), str::to_string)
}

fn pnpm(
    worker: &Worker,
    args: &[&str],
    dir: &Path,
    envs: &[(&str, String)],
) -> Result<(bool, String), String> {
    let path = doctor::find("pnpm").ok_or_else(|| "pnpm is not on the PATH".to_string())?;
    let mut command = Command::new(path);
    command
        .env("CI", "true")
        .env("NEXT_TELEMETRY_DISABLED", "1")
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "0");
    for (name, value) in envs {
        command.env(name, value);
    }
    tool(command, args, dir, &worker.child).map_err(|e| format!("lengua could not run pnpm: {e}"))
}

fn copy_tree(from: &Path, to: &Path, top: bool) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if top && KEPT.iter().any(|k| name == *k) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            copy_tree(&path, &to.join(&name), false)?;
        } else {
            fs::copy(&path, to.join(&name))?;
        }
    }
    Ok(())
}

fn refresh(folder: &Path) -> io::Result<PathBuf> {
    let stage = folder.join(STAGE);
    fs::create_dir_all(&stage)?;
    for entry in fs::read_dir(&stage)? {
        let entry = entry?;
        if entry.file_name() == "node_modules" {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }
    copy_tree(&folder.join("web"), &stage, true)?;
    Ok(stage)
}

fn tail(text: &str, count: usize) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(count);
    lines.get(start..).unwrap_or(&[]).join("\n")
}

fn failed(stage: Stage, file: &str, code: &str, message: String) -> Failure {
    (stage, vec![problem(file, 1, code, message)])
}

fn install(worker: &Worker, stage: &Path) -> Result<(), Failure> {
    let locked = worker.folder.join("web/pnpm-lock.yaml");
    let frozen = locked.is_file();
    let args: &[&str] = if frozen {
        &["install", "--frozen-lockfile", "--prefer-offline"]
    } else {
        &["install", "--prefer-offline"]
    };
    match pnpm(worker, args, stage, &[]) {
        Ok((true, _)) => {}
        Ok((false, text)) => {
            let file = if frozen {
                "web/pnpm-lock.yaml"
            } else {
                "web/package.json"
            };
            let hint = if frozen {
                "; when `web/package.json` changed, remove `web/pnpm-lock.yaml` so that pnpm resolves it again"
            } else {
                ""
            };
            return Err(failed(
                Stage::BuildFailed,
                file,
                "web-install",
                format!("`pnpm install` failed{hint}:\n{}", tail(&text, 20)),
            ));
        }
        Err(message) => {
            return Err(failed(
                Stage::BuildFailed,
                "web/package.json",
                "web-install",
                message,
            ));
        }
    }
    if !frozen {
        fs::copy(stage.join("pnpm-lock.yaml"), &locked).map_err(|e| {
            failed(
                Stage::BuildFailed,
                "web/pnpm-lock.yaml",
                "web-install",
                format!("lengua could not keep pnpm's lock: {e}"),
            )
        })?;
    }
    Ok(())
}

pub(crate) fn type_problems(text: &str) -> Vec<Problem> {
    text.lines()
        .filter_map(|line| {
            let (place, rest) = line.split_once("): error ")?;
            let (file, at) = place.rsplit_once('(')?;
            let (number, column) = at.split_once(',')?;
            let number: usize = number.trim().parse().ok()?;
            Some(problem(
                &format!("web/{}", file.trim().replace('\\', "/")),
                number,
                "ts-error",
                format!("column {}: {}", column.trim(), rest.trim()),
            ))
        })
        .collect()
}

fn typecheck(worker: &Worker, stage: &Path) -> Result<(), Failure> {
    match pnpm(
        worker,
        &["exec", "tsc", "--noEmit", "--pretty", "false"],
        stage,
        &[],
    ) {
        Ok((true, _)) => Ok(()),
        Ok((false, text)) => {
            let found = type_problems(&text);
            if found.is_empty() {
                Err(failed(
                    Stage::BuildFailed,
                    "web/tsconfig.json",
                    "ts-error",
                    format!("`tsc --noEmit` failed:\n{}", tail(&text, 20)),
                ))
            } else {
                Err((Stage::BuildFailed, found))
            }
        }
        Err(message) => Err(failed(
            Stage::BuildFailed,
            "web/tsconfig.json",
            "ts-error",
            message,
        )),
    }
}

fn first_message(result: &Value) -> String {
    let text = result["failureMessages"]
        .as_array()
        .and_then(|m| m.first())
        .and_then(Value::as_str)
        .unwrap_or("it failed");
    let short: String = text.lines().take(6).collect::<Vec<_>>().join(" ");
    short.chars().take(400).collect()
}

pub(crate) fn test_problems(front: &Front, report: &Value) -> Vec<Problem> {
    let mut out = Vec::new();
    let files = report["testResults"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for file in &files {
        let results = file["assertionResults"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for result in results.iter().filter(|r| r["status"] == "failed") {
            let title = result["title"].as_str().unwrap_or("");
            let index: Option<usize> = title.split(' ').next().and_then(|n| n.parse().ok());
            let Some(run) = index.and_then(|i| front.runs.get(i)) else {
                continue;
            };
            let code = if run.name.contains(" = ") {
                "case-failed"
            } else {
                "test-failed"
            };
            out.push(problem(
                &run.file,
                run.line,
                code,
                format!(
                    "the screen example `{}` failed: {}",
                    run.name,
                    first_message(result)
                ),
            ));
        }
    }
    out
}

fn tests(worker: &Worker, front: &Front, stage: &Path) -> Result<usize, Failure> {
    let total = front.runs.len();
    worker.update(|s| s.tests.total += total);
    let report = stage.join(REPORT);
    let output = format!("--outputFile={REPORT}");
    let ran = pnpm(
        worker,
        &["exec", "vitest", "run", "--reporter=json", &output],
        stage,
        &[],
    );
    let text = match ran {
        Ok((_, text)) => text,
        Err(message) => {
            return Err(failed(
                Stage::BuildFailed,
                "web/vitest.config.mts",
                "web-test",
                message,
            ));
        }
    };
    let read = fs::read_to_string(&report)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let Some(read) = read else {
        return Err(failed(
            Stage::BuildFailed,
            "web/vitest.config.mts",
            "web-test",
            format!("Vitest gave no report:\n{}", tail(&text, 20)),
        ));
    };
    let found = test_problems(front, &read);
    let failures = found.len();
    worker.update(|s| {
        s.tests.run += total;
        s.tests.failed += failures;
    });
    if found.is_empty() {
        Ok(total)
    } else {
        Err((Stage::TestsFailed, found))
    }
}

fn next_build(worker: &Worker, stage: &Path) -> Result<(), Failure> {
    match pnpm(worker, &["exec", "next", "build"], stage, &[]) {
        Ok((true, _)) if stage.join("out").is_dir() => Ok(()),
        Ok((_, text)) => Err(failed(
            Stage::BuildFailed,
            "web/package.json",
            "web-build",
            format!("`next build` failed:\n{}", tail(&text, 30)),
        )),
        Err(message) => Err(failed(
            Stage::BuildFailed,
            "web/package.json",
            "web-build",
            message,
        )),
    }
}

pub(crate) fn media_type(path: &str) -> &'static str {
    let extension = Path::new(path)
        .extension()
        .map_or_default(|e| e.to_string_lossy().to_ascii_lowercase());
    match extension.as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "txt" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "xml" => "application/xml",
        "webmanifest" => "application/manifest+json",
        _ => "application/octet-stream",
    }
}

fn exported(dir: &Path, prefix: &str, out: &mut Vec<(String, u64)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut names: Vec<(String, PathBuf)> = entries
        .flatten()
        .map(|e| (e.file_name().to_string_lossy().to_string(), e.path()))
        .collect();
    names.sort();
    for (name, path) in names {
        let relative = format!("{prefix}{name}");
        if path.is_dir() {
            exported(&path, &format!("{relative}/"), out);
        } else {
            out.push((relative, path.metadata().map_or(0, |m| m.len())));
        }
    }
}

pub(crate) fn assets_text(files: &[(String, u64)]) -> String {
    let mut out = format!("const FILES: [(&str, &str, &[u8]); {}] = [\n", files.len());
    for (name, _) in files {
        let path = format!("../../web/out/{name}");
        let _ = writeln!(
            out,
            "    ({name:?}, {:?}, include_bytes!({path:?})),",
            media_type(name),
        );
    }
    out.push_str("];\n\n#[must_use]\npub fn asset(path: &str) -> Option<(&'static str, &'static [u8])> {\n    let path = path.split(['?', '#']).next().unwrap_or(\"\").trim_start_matches('/');\n    let wanted = if path.is_empty() || path.ends_with('/') {\n        format!(\"{path}index.html\")\n    } else {\n        path.to_string()\n    };\n    let inside = format!(\"{wanted}/index.html\");\n    FILES\n        .iter()\n        .find(|(name, _, _)| *name == wanted)\n        .or_else(|| FILES.iter().find(|(name, _, _)| *name == inside))\n        .map(|(_, kind, bytes)| (*kind, *bytes))\n}\n");
    out
}

fn assets(folder: &Path, stage: &Path) -> Result<(usize, u64), Failure> {
    let mut files = Vec::new();
    exported(&stage.join("out"), "", &mut files);
    let dir = folder.join(ASSETS);
    let manifest = "[package]\nname = \"lengua_web\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[lib]\npath = \"src/lib.rs\"\n";
    let written = fs::create_dir_all(dir.join("src"))
        .and_then(|()| fs::write(dir.join("Cargo.toml"), manifest))
        .and_then(|()| fs::write(dir.join("src/lib.rs"), assets_text(&files)));
    written.map_err(|e| {
        failed(
            Stage::BuildFailed,
            "web/package.json",
            "web-build",
            format!("lengua could not write the crate of the export: {e}"),
        )
    })?;
    Ok((files.len(), files.iter().map(|(_, size)| size).sum()))
}

pub(crate) fn build(worker: &Worker, front: &Front) -> Result<usize, Failure> {
    let stage = refresh(&worker.folder).map_err(|e| {
        failed(
            Stage::BuildFailed,
            "web/package.json",
            "web-build",
            format!("lengua could not copy `web/` to build it: {e}"),
        )
    })?;
    install(worker, &stage)?;
    typecheck(worker, &stage)?;
    let runs = tests(worker, front, &stage)?;
    next_build(worker, &stage)?;
    let size = assets(&worker.folder, &stage)?;
    worker.update(|s| s.export = Some(size));
    Ok(runs)
}

pub(crate) fn judge(worker: &Worker, mutant: &Mutant) -> Option<bool> {
    let stage = worker.folder.join(STAGE);
    let inside = mutant.file.strip_prefix("web/").unwrap_or(&mutant.file);
    let original = stage.join(inside);
    let text = fs::read_to_string(&original).ok()?;
    let changed = lengua_idea::apply(&text, &mutant.edits);
    let place = stage.join(".lengua").join("mutant.tsx");
    let _ = fs::create_dir_all(stage.join(".lengua"));
    fs::write(&place, changed).ok()?;
    let envs = [
        ("LENGUA_MUTANT_FROM", plain(&original)),
        ("LENGUA_MUTANT_TO", plain(&place)),
    ];
    let ran = pnpm(
        worker,
        &["exec", "vitest", "run", "--reporter=dot"],
        &stage,
        &envs,
    );
    if worker.stopped() {
        return None;
    }
    ran.ok().map(|(passed, _)| !passed)
}

pub(crate) fn screen_files(folder: &Path, front: &Front) -> Vec<(String, String)> {
    front
        .sources
        .iter()
        .filter_map(|file| {
            let text = fs::read_to_string(folder.join(file)).ok()?;
            Some((file.clone(), text.replace("\r\n", "\n")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{assets_text, media_type, type_problems};

    #[test]
    fn type_errors_give_their_file_and_line() {
        let found = type_problems(
            "screens/home.tsx(12,5): error TS2322: Type 'string' is not assignable to type 'number'.\nnoise\n",
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "web/screens/home.tsx");
        assert_eq!(found[0].line, 12);
        assert!(found[0].message.starts_with("column 5: TS2322: "));
    }

    #[test]
    fn the_export_becomes_a_crate_that_finds_each_file() {
        let text = assets_text(&[
            ("index.html".to_string(), 10),
            ("_next/static/a.js".to_string(), 5),
        ]);
        assert!(text.starts_with("const FILES: [(&str, &str, &[u8]); 2] = [\n"));
        assert!(text.contains("include_bytes!(\"../../web/out/_next/static/a.js\")"));
        assert!(text.contains("\"text/javascript; charset=utf-8\""));
        assert_eq!(media_type("x/y.woff2"), "font/woff2");
        assert_eq!(media_type("README"), "application/octet-stream");
    }
}
