use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use lengua_idea::{ASSETS, Browser, Front, Mutant};
use rmcp::serde_json::{self, Value};

use crate::build::{Problem, Stage, Worker, lock, problem, tool};
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

fn pnpm_limited(
    worker: &Worker,
    args: &[&str],
    dir: &Path,
    seconds: u64,
) -> Result<(bool, String), String> {
    let finished = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&finished);
    let slot = Arc::clone(&worker.child);
    thread::spawn(move || {
        for _ in 0..seconds * 10 {
            if flag.load(Ordering::Relaxed) {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        if !flag.load(Ordering::Relaxed)
            && let Some(child) = lock(&slot).as_mut()
        {
            let _ = child.kill();
        }
    });
    let out = pnpm(worker, args, dir, &[]);
    finished.store(true, Ordering::Relaxed);
    out
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

fn electron(worker: &Worker, stage: &Path) -> Result<(), Failure> {
    let installed = stage.join("node_modules/electron/path.txt").is_file();
    if installed {
        return Ok(());
    }
    match pnpm_limited(worker, &["exec", "install-electron"], stage, 600) {
        Ok((true, _)) => Ok(()),
        Ok((false, text)) => Err(failed(
            Stage::BuildFailed,
            "web/package.json",
            "web-install",
            format!("Electron's installer failed:\n{}", tail(&text, 20)),
        )),
        Err(message) => Err(failed(
            Stage::BuildFailed,
            "web/package.json",
            "web-install",
            message,
        )),
    }
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
    let ran = pnpm_limited(
        worker,
        &["exec", "vitest", "run", "--reporter=json", &output],
        stage,
        600,
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
    match pnpm_limited(worker, &["exec", "next", "build"], stage, 600) {
        Ok((true, _)) if stage.join("out").is_dir() => flatten_segments(&stage.join("out"))
            .map_err(|e| {
                failed(
                    Stage::BuildFailed,
                    "web/package.json",
                    "web-build",
                    format!("lengua could not name the export's segment files: {e}"),
                )
            }),
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

fn flatten_segments(dir: &Path) -> io::Result<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries.into_iter().filter(|p| p.is_dir()) {
        let name = path
            .file_name()
            .map_or_default(|n| n.to_string_lossy().to_string());
        if name.starts_with("__next.") {
            flatten_into(&path, dir, &name)?;
            fs::remove_dir_all(&path)?;
        } else {
            flatten_segments(&path)?;
        }
    }
    Ok(())
}

fn flatten_into(from: &Path, to: &Path, prefix: &str) -> io::Result<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(from)?.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        let name = format!(
            "{prefix}.{}",
            path.file_name()
                .map_or_default(|n| n.to_string_lossy().to_string())
        );
        if path.is_dir() {
            flatten_into(&path, to, &name)?;
        } else {
            fs::rename(&path, to.join(name))?;
        }
    }
    Ok(())
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

pub(crate) const WASM_BINDGEN: &str = "0.2.130";

pub(crate) fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = u32::from(chunk.first().copied().unwrap_or(0));
        let second = u32::from(chunk.get(1).copied().unwrap_or(0));
        let third = u32::from(chunk.get(2).copied().unwrap_or(0));
        let joined = (first << 16) | (second << 8) | third;
        for place in 0..4 {
            if place <= chunk.len() {
                let index = usize::try_from((joined >> (18 - 6 * place)) & 63).unwrap_or(0);
                out.push(char::from(TABLE.get(index).copied().unwrap_or(b'A')));
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub(crate) fn wrapper_text(browser: &Browser) -> String {
    let mut out = String::from("use wasm_bindgen::prelude::wasm_bindgen;\n");
    for function in &browser.functions {
        let params: Vec<String> = function
            .params
            .iter()
            .map(|(n, t)| format!("{n}: {t}"))
            .collect();
        let names: Vec<&str> = function.params.iter().map(|(n, _)| n.as_str()).collect();
        let _ = write!(
            out,
            "\n#[wasm_bindgen]\npub fn {}({}) -> {} {{\n    {}::{}({})\n}}\n",
            function.name,
            params.join(", "),
            function.output,
            browser.crate_name,
            function.name,
            names.join(", ")
        );
    }
    out
}

fn browser_failed(name: &str, message: &str) -> Failure {
    failed(
        Stage::BuildFailed,
        "idea.md",
        "web-browser",
        format!("lengua could not build the library `{name}` for the browser: {message}"),
    )
}

fn browser(worker: &Worker, item: &Browser, stage: &Path) -> Result<(), Failure> {
    let dir = worker.folder.join("target/lengua/browser").join(&item.name);
    let library = plain(&worker.folder.join(&item.path)).replace('\\', "/");
    let package = format!("{}-browser", item.name);
    let manifest = format!(
        "[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[lib]\ncrate-type = [\"cdylib\"]\npath = \"src/lib.rs\"\n\n[dependencies]\n{} = {{ path = \"{library}\" }}\nwasm-bindgen = \"={WASM_BINDGEN}\"\n\n[profile.release]\nopt-level = \"s\"\nlto = true\n\n[workspace]\n",
        item.crate_name
    );
    let written = fs::create_dir_all(dir.join("src"))
        .and_then(|()| fs::write(dir.join("Cargo.toml"), manifest))
        .and_then(|()| fs::write(dir.join("src/lib.rs"), wrapper_text(item)));
    written.map_err(|e| browser_failed(&item.name, &e.to_string()))?;
    let built = crate::build::cargo(
        &dir,
        &[
            "build",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--message-format=short",
        ],
        &worker.child,
    );
    match built {
        Ok((true, _)) => {}
        Ok((false, text)) => return Err(browser_failed(&item.name, &tail(&text, 20))),
        Err(message) => return Err(browser_failed(&item.name, &message)),
    }
    let bindgen = doctor::find("wasm-bindgen")
        .ok_or_else(|| browser_failed(&item.name, "wasm-bindgen is not on the PATH"))?;
    let out = stage.join("lib/wasm").join(&item.name);
    let input = dir
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("{}.wasm", package.replace('-', "_")));
    let mut command = Command::new(bindgen);
    command.arg("--out-name").arg(&item.name);
    let ran = tool(
        command,
        &["--target", "web", "--out-dir", &plain(&out), &plain(&input)],
        &dir,
        &worker.child,
    );
    match ran {
        Ok((true, _)) => {}
        Ok((false, text)) => return Err(browser_failed(&item.name, &tail(&text, 20))),
        Err(message) => return Err(browser_failed(&item.name, &message)),
    }
    let bytes = fs::read(out.join(format!("{}_bg.wasm", item.name)))
        .map_err(|e| browser_failed(&item.name, &e.to_string()))?;
    let name = &item.name;
    let module = format!(
        "import {{ initSync }} from \"./{name}/{name}.js\";\nimport {{ bytes }} from \"./{name}/bytes\";\n\ninitSync({{ module: bytes }});\n\nexport * from \"./{name}/{name}.js\";\n"
    );
    let data = format!(
        "const text = \"{}\";\n\nexport const bytes = Uint8Array.from(atob(text), (c) => c.charCodeAt(0));\n",
        base64(&bytes)
    );
    fs::write(out.join("bytes.ts"), data)
        .and_then(|()| fs::write(stage.join("lib/wasm").join(format!("{name}.ts")), module))
        .map_err(|e| browser_failed(&item.name, &e.to_string()))
}

fn browsers(worker: &Worker, front: &Front, stage: &Path) -> Result<(), Failure> {
    for item in &front.browser {
        browser(worker, item, stage)?;
    }
    Ok(())
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
    if front.desktop {
        electron(worker, &stage)?;
    }
    browsers(worker, front, &stage)?;
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
    fs::write(&original, changed).ok()?;
    let ran = pnpm_limited(
        worker,
        &["exec", "vitest", "run", "--reporter=dot"],
        &stage,
        120,
    );
    let restored = fs::write(&original, &text).is_ok();
    if worker.stopped() || !restored {
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
    use super::{assets_text, base64, flatten_segments, media_type, type_problems, wrapper_text};
    use lengua_idea::{Browser, BrowserFn};
    use std::fs;

    #[test]
    fn a_segment_folder_becomes_the_file_name_next_writes_on_linux() {
        let out = std::env::temp_dir().join(format!("lengua-mcp-{}-segments", std::process::id()));
        let _ = fs::remove_dir_all(&out);
        fs::create_dir_all(out.join("note/__next.note")).expect("folders");
        fs::write(out.join("note/__next.note/__PAGE__.txt"), "p").expect("a file");
        fs::write(out.join("note/index.html"), "i").expect("a file");
        flatten_segments(&out).expect("flattened");
        assert_eq!(
            fs::read_to_string(out.join("note/__next.note.__PAGE__.txt")).expect("renamed"),
            "p"
        );
        assert!(!out.join("note/__next.note").exists());
        assert!(out.join("note/index.html").is_file());
        let _ = fs::remove_dir_all(&out);
    }

    #[test]
    fn bytes_become_base64_and_a_library_gets_its_wrapper() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b""), "");
        let item = Browser {
            name: "titles".to_string(),
            crate_name: "titles".to_string(),
            path: "../titles".to_string(),
            functions: vec![BrowserFn {
                name: "title_error".to_string(),
                params: vec![("title".to_string(), "&str".to_string())],
                output: "String".to_string(),
            }],
        };
        let text = wrapper_text(&item);
        assert!(text.contains("#[wasm_bindgen]\npub fn title_error(title: &str) -> String {\n    titles::title_error(title)\n}\n"), "{text}");
    }

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
