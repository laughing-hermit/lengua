use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use lengua_idea::{Build, Case, Mutant, Mutation, Shape, Test};

use crate::build::{Remark, Worker, cargo, lock};
use crate::host::{Seal, unseal};
use crate::{harness, testing};

pub(crate) const MUTANTS: usize = 40;
const CACHE: &str = "target/lengua/mutants";
const FOLDER: &str = "target/lengua/mutant";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    Caught,
    Survived,
    Unbuilt,
    Same,
}

impl Verdict {
    const ALL: [Self; 4] = [Self::Caught, Self::Survived, Self::Unbuilt, Self::Same];

    fn name(self) -> &'static str {
        match self {
            Self::Caught => "caught",
            Self::Survived => "survived",
            Self::Unbuilt => "unbuilt",
            Self::Same => "same",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.name() == text)
    }
}

struct Workspace {
    dir: PathBuf,
    texts: Vec<(String, String)>,
    build: Build,
    trusted: Vec<String>,
}

fn read_cache(path: &Path) -> HashMap<String, Verdict> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let (key, verdict) = line.split_once(' ')?;
            Some((key.to_string(), Verdict::parse(verdict)?))
        })
        .collect()
}

fn write_cache(path: &Path, kept: &[(String, Verdict)]) {
    let mut text = String::new();
    for (key, verdict) in kept {
        text.push_str(key);
        text.push(' ');
        text.push_str(verdict.name());
        text.push('\n');
    }
    let _ = path.parent().map(fs::create_dir_all);
    let _ = fs::write(path, text);
}

fn context(worker: &Worker, mutation: &Mutation) -> String {
    let manifest = fs::read_to_string(worker.folder.join("Cargo.toml")).unwrap_or_default();
    let plain: Vec<Test> = worker
        .build
        .tests
        .iter()
        .map(|t| Test {
            line: 0,
            ..t.clone()
        })
        .collect();
    let all = format!(
        "{}\0{}\0{manifest}\0{}",
        env!("CARGO_PKG_VERSION"),
        lengua_idea::tests_hash(&plain, &worker.build.grants),
        mutation.skeleton
    );
    lengua_idea::sha256_hex(all.as_bytes())
}

fn key(context: &str, mutant: &Mutant) -> String {
    let all = format!(
        "{context}\0{}\0{}\0{}\0{}\0{}\0{}",
        mutant.file, mutant.function, mutant.function_text, mutant.site, mutant.from, mutant.to
    );
    lengua_idea::sha256_hex(all.as_bytes())
}

pub(crate) fn mutant_name(package: &str) -> String {
    format!("{package}-mutant")
}

pub(crate) fn mutant_manifest(text: &str, package: &str, folder: &Path) -> String {
    let snake = package.replace('-', "_");
    let mut section = String::new();
    let mut out = String::new();
    for line in text.lines() {
        if line.starts_with('[') {
            section = line.trim().to_string();
        }
        let changed = if line == format!("name = \"{package}\"") && section != "[lib]" {
            format!("name = \"{}\"", mutant_name(package))
        } else if section == "[lib]" && line == format!("name = \"{snake}\"") {
            format!("name = \"{snake}_mutant\"")
        } else if let Some((head, rest)) = line
            .split_once("{ path = \"")
            .filter(|_| section == "[dependencies]")
        {
            let relative = rest.trim_end_matches("\" }");
            let absolute = folder.join(relative).to_string_lossy().replace('\\', "/");
            format!("{head}{{ path = \"{absolute}\" }}")
        } else {
            line.to_string()
        };
        out.push_str(&changed);
        out.push('\n');
    }
    out
}

fn tested_artifact(build: &Build) -> String {
    let name = mutant_name(&build.package);
    match build.shape {
        Shape::Library => format!(
            "target/{}/release/{}.wasm",
            build.target,
            lengua_idea::harness_name(&name)
        ),
        Shape::Server => format!(
            "target/{}/release/{}.wasm",
            build.target,
            name.replace('-', "_")
        ),
        Shape::Command => format!("target/{}/release/{name}.wasm", build.target),
    }
}

fn workspace(worker: &Worker) -> Result<Workspace, String> {
    let folder = &worker.folder;
    let fail = |e: std::io::Error| e.to_string();
    let dir = folder.join(FOLDER);
    fs::create_dir_all(&dir).map_err(fail)?;
    let package = &worker.build.package;
    let manifest = fs::read_to_string(folder.join("Cargo.toml")).map_err(fail)?;
    fs::write(
        dir.join("Cargo.toml"),
        mutant_manifest(&manifest, package, folder),
    )
    .map_err(fail)?;
    if let Ok(locked) = fs::read(folder.join("Cargo.lock")) {
        fs::write(dir.join("Cargo.lock"), locked).map_err(fail)?;
    }
    let mut texts = Vec::new();
    for file in &worker.build.sources {
        let text = fs::read_to_string(folder.join(file)).map_err(fail)?;
        texts.push((file.clone(), text.replace("\r\n", "\n")));
    }
    if worker.build.library {
        let name = mutant_name(package);
        let harness = lengua_idea::harness_name(&name);
        let spec = harness::Library {
            package,
            dependency: &name,
            path: "../mutant",
            harness: &harness,
        };
        harness::write(folder, harness::MUTANT_FOLDER, &spec, &testing::cases(&worker.build))?;
    }
    let build = Build {
        artifact: tested_artifact(&worker.build),
        ..worker.build.clone()
    };
    let trusted = lock(&worker.state)
        .component
        .as_ref()
        .map_or_else(Vec::new, |c| c.trusted.clone());
    Ok(Workspace {
        dir,
        texts,
        build,
        trusted,
    })
}

fn verdict(
    worker: &Worker,
    space: &Workspace,
    mutant: &Mutant,
    cases: &[Case],
    original: &str,
) -> Option<Verdict> {
    for (file, text) in &space.texts {
        let written = if *file == mutant.file {
            lengua_idea::apply(text, &mutant.edits)
        } else {
            text.clone()
        };
        let path = space.dir.join(file);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::write(&path, written).is_err() {
            return Some(Verdict::Unbuilt);
        }
    }
    let target_dir = worker.folder.join("target").to_string_lossy().to_string();
    let args = [
        "build",
        "--release",
        "--target",
        worker.build.target,
        "--offline",
        "--message-format=short",
        "--target-dir",
        target_dir.as_str(),
    ];
    let dir = if worker.build.library {
        worker.folder.join(harness::MUTANT_FOLDER)
    } else {
        space.dir.clone()
    };
    let built = matches!(cargo(&dir, &args, &worker.child), Ok((true, _)));
    if worker.stopped() {
        return None;
    }
    if !built {
        return Some(Verdict::Unbuilt);
    }
    let Ok(bytes) = fs::read(worker.folder.join(&space.build.artifact)) else {
        return Some(Verdict::Unbuilt);
    };
    let sha256 = lengua_idea::sha256_hex(&bytes);
    if sha256 == original {
        return Some(Verdict::Same);
    }
    let Ok(seal) = worker
        .host
        .seal(&worker.folder, &bytes, &sha256, &space.trusted)
    else {
        return Some(Verdict::Unbuilt);
    };
    let found = judge(worker, space, cases, &seal);
    unseal(&worker.folder, &seal);
    found
}

fn judge(worker: &Worker, space: &Workspace, cases: &[Case], seal: &Seal) -> Option<Verdict> {
    for (index, case) in cases.iter().enumerate() {
        if worker.stopped() {
            return None;
        }
        if testing::run_case(&worker.folder, &space.build, &seal.lock, (index, case)).is_some() {
            return Some(Verdict::Caught);
        }
    }
    Some(Verdict::Survived)
}

fn remark(mutant: &Mutant) -> Remark {
    let change = if mutant.from == "the body" {
        format!("the body became `{}`", mutant.to)
    } else {
        format!("`{}` became `{}`", mutant.from, mutant.to)
    };
    Remark {
        file: mutant.file.clone(),
        line: Some(mutant.line),
        message: format!(
            "a mutant survived: in `{}`, {change}, and every test still passed; add a test whose result tells the two apart",
            mutant.function
        ),
    }
}

pub(crate) fn run(worker: &Worker, cases: &[Case]) -> bool {
    let mutation = lengua_idea::mutants(&worker.folder, &worker.build, MUTANTS);
    let original = lock(&worker.state)
        .component
        .as_ref()
        .map_or_else(String::new, |c| c.tested.clone());
    worker.update(|s| s.mutants.total = mutation.mutants.len());
    let cache = worker.folder.join(CACHE);
    let old = read_cache(&cache);
    let context = context(worker, &mutation);
    let mut space: Option<Workspace> = None;
    let mut kept: Vec<(String, Verdict)> = Vec::new();
    let mut remarks = Vec::new();
    for mutant in &mutation.mutants {
        let key = key(&context, mutant);
        let hit = old.get(&key).copied();
        let found = if let Some(found) = hit {
            found
        } else {
            if space.is_none() {
                space = workspace(worker).ok();
            }
            let decided = match &space {
                Some(ready) => verdict(worker, ready, mutant, cases, &original),
                None => Some(Verdict::Unbuilt),
            };
            let Some(found) = decided else {
                return false;
            };
            found
        };
        if worker.stopped() {
            return false;
        }
        kept.push((key, found));
        if found == Verdict::Survived {
            remarks.push(remark(mutant));
        }
        worker.update(|s| {
            let m = &mut s.mutants;
            m.done += 1;
            m.cached += usize::from(hit.is_some());
            match found {
                Verdict::Caught => m.caught += 1,
                Verdict::Survived => m.survived += 1,
                Verdict::Unbuilt => m.unbuilt += 1,
                Verdict::Same => m.same += 1,
            }
        });
    }
    write_cache(&cache, &kept);
    worker.update(|s| s.remarks.extend(remarks));
    true
}

#[cfg(test)]
mod tests {
    use super::{Verdict, read_cache, write_cache};

    #[test]
    fn the_cache_keeps_each_verdict() {
        let path = std::env::temp_dir()
            .join(format!("lengua-mcp-{}-cache", std::process::id()))
            .join("mutants");
        let kept = vec![
            ("a".to_string(), Verdict::Caught),
            ("b".to_string(), Verdict::Survived),
            ("c".to_string(), Verdict::Unbuilt),
            ("d".to_string(), Verdict::Same),
        ];
        write_cache(&path, &kept);
        let read = read_cache(&path);
        assert_eq!(read.len(), 4);
        for (key, verdict) in kept {
            assert_eq!(read.get(&key), Some(&verdict));
        }
    }

    #[test]
    fn a_mutant_gets_its_own_names_and_absolute_library_paths() {
        let manifest = "[package]\nname = \"shop\"\n\n[lib]\nname = \"shop\"\npath = \"src/main.rs\"\n\n[dependencies]\njson = { path = \"../libraries/json\" }\nwasip2 = \"=2.0.1\"\n\n[workspace]\n";
        let folder = std::path::Path::new("/work/shop");
        let text = super::mutant_manifest(manifest, "shop", folder);
        assert!(text.contains("[package]\nname = \"shop-mutant\"\n"), "{text}");
        assert!(text.contains("[lib]\nname = \"shop_mutant\"\n"), "{text}");
        assert!(
            text.contains("json = { path = \"/work/shop/../libraries/json\" }"),
            "{text}"
        );
        assert!(text.contains("wasip2 = \"=2.0.1\"\n"), "{text}");
        assert!(text.contains("path = \"src/main.rs\"\n"), "{text}");
    }
}
