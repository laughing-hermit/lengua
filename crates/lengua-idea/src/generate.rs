use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::constraints::{Constraints, Grant};
use crate::sha256;
use crate::wiki::{Page, Stamp};
use crate::{Finding, Library, Shape};

pub(crate) const CARGO_TOML: &str = "Cargo.toml";
pub(crate) const WORLD: &str = "wit/world.wit";
pub(crate) const WASI: &str = "0.2.12";
pub(crate) const TARGET: &str = "wasm32-wasip2";
pub const ASSETS: &str = "target/lengua/assets";

const NOT_NAMES: [&str; 62] = [
    "abstract",
    "alloc",
    "as",
    "async",
    "aux",
    "await",
    "become",
    "box",
    "break",
    "build",
    "con",
    "const",
    "continue",
    "core",
    "crate",
    "deps",
    "do",
    "dyn",
    "else",
    "enum",
    "examples",
    "extern",
    "false",
    "final",
    "fn",
    "for",
    "gen",
    "if",
    "impl",
    "in",
    "incremental",
    "let",
    "loop",
    "macro",
    "match",
    "mod",
    "move",
    "mut",
    "nul",
    "override",
    "priv",
    "proc_macro",
    "prn",
    "pub",
    "ref",
    "return",
    "self",
    "static",
    "std",
    "struct",
    "super",
    "test",
    "trait",
    "true",
    "try",
    "type",
    "typeof",
    "unsafe",
    "unsized",
    "use",
    "where",
    "while",
];

const ALWAYS: [&str; 15] = [
    "wasi:cli/environment",
    "wasi:cli/exit",
    "wasi:cli/stderr",
    "wasi:cli/stdin",
    "wasi:cli/stdout",
    "wasi:cli/terminal-input",
    "wasi:cli/terminal-output",
    "wasi:cli/terminal-stderr",
    "wasi:cli/terminal-stdin",
    "wasi:cli/terminal-stdout",
    "wasi:clocks/monotonic-clock",
    "wasi:io/error",
    "wasi:io/poll",
    "wasi:io/streams",
    "wasi:random/insecure-seed",
];

pub(crate) fn interfaces(grant: Grant) -> &'static [&'static str] {
    match grant {
        Grant::StandardInput | Grant::Arguments => &[],
        Grant::Clock => &["wasi:clocks/wall-clock"],
        Grant::Random => &[
            "wasi:random/insecure",
            "wasi:random/insecure-seed",
            "wasi:random/random",
        ],
        Grant::Files => &[
            "wasi:clocks/wall-clock",
            "wasi:filesystem/preopens",
            "wasi:filesystem/types",
        ],
        Grant::Network | Grant::Listen => &[
            "wasi:clocks/monotonic-clock",
            "wasi:clocks/wall-clock",
            "wasi:sockets/instance-network",
            "wasi:sockets/ip-name-lookup",
            "wasi:sockets/network",
            "wasi:sockets/tcp",
            "wasi:sockets/tcp-create-socket",
            "wasi:sockets/udp",
            "wasi:sockets/udp-create-socket",
        ],
        Grant::Http => &["wasi:http/types"],
    }
}

pub(crate) fn package_name(name: &str) -> bool {
    let mut chars = name.chars();
    let first = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    first
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && !NOT_NAMES.contains(&name)
}

pub(crate) struct Package<'a> {
    pub name: &'a str,
    pub shape: Shape,
    pub sources: &'a [String],
    pub crates: &'a [(String, String)],
    pub libraries: &'a [(String, String)],
    pub web: bool,
}

fn quoted_list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|i| format!("\"{i}\"")).collect();
    format!("[{}]", quoted.join(", "))
}

fn dependencies(package: &Package) -> Vec<String> {
    let mut lines: Vec<(String, String)> = package
        .crates
        .iter()
        .map(|(name, version)| (name.clone(), format!("{name} = \"={version}\"")))
        .collect();
    lines.extend(
        package
            .libraries
            .iter()
            .map(|(name, path)| (name.clone(), format!("{name} = {{ path = \"{path}\" }}"))),
    );
    if package.web {
        lines.push((
            "lengua_web".to_string(),
            format!("lengua_web = {{ path = \"{ASSETS}\" }}"),
        ));
    }
    lines.sort();
    lines.into_iter().map(|(_, line)| line).collect()
}

pub(crate) fn cargo_toml(package: &Package) -> String {
    let mut lines: Vec<String> = vec![
        "[package]".to_string(),
        format!("name = \"{}\"", package.name),
        "version = \"0.1.0\"".to_string(),
        "edition = \"2024\"".to_string(),
        "publish = false".to_string(),
        format!("include = {}", quoted_list(package.sources)),
    ];
    if package.shape != Shape::Command {
        lines.push("autobins = false".to_string());
    }
    lines.push(String::new());
    if package.shape == Shape::Command {
        lines.push("[[bin]]".to_string());
        lines.push(format!("name = \"{}\"", package.name));
    } else {
        lines.push("[lib]".to_string());
        lines.push(format!("name = \"{}\"", package.name.replace('-', "_")));
    }
    lines.push("path = \"src/main.rs\"".to_string());
    if package.shape == Shape::Server {
        lines.push("crate-type = [\"cdylib\"]".to_string());
    }
    lines.push(String::new());
    let dependencies = dependencies(package);
    if !dependencies.is_empty() {
        lines.push("[dependencies]".to_string());
        lines.extend(dependencies);
        lines.push(String::new());
    }
    for line in [
        "[lints.rust]",
        "warnings = \"deny\"",
        "unsafe_code = \"forbid\"",
        "unused_must_use = \"deny\"",
        "",
        "[lints.clippy]",
        "pedantic = { level = \"deny\", priority = -1 }",
        "unwrap_used = \"deny\"",
        "expect_used = \"deny\"",
        "indexing_slicing = \"deny\"",
    ] {
        lines.push(line.to_string());
    }
    if package.shape == Shape::Library {
        lines.push("missing_errors_doc = \"allow\"".to_string());
        lines.push("missing_panics_doc = \"allow\"".to_string());
    }
    for line in [
        "",
        "[profile.release]",
        "opt-level = 3",
        "panic = \"abort\"",
        "lto = true",
        "codegen-units = 1",
        "strip = true",
        "",
        "[workspace]",
    ] {
        lines.push(line.to_string());
    }
    if package.web {
        lines.push("exclude = [\"target\"]".to_string());
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

pub(crate) fn world_names(grants: &[Grant]) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = ALWAYS.to_vec();
    for grant in grants {
        names.extend_from_slice(interfaces(*grant));
    }
    names.sort_unstable();
    names.dedup();
    names
}

pub(crate) fn world(grants: &[Grant], shape: Shape) -> String {
    let names = world_names(grants);
    let mut text = String::from("package lengua:program;\n\nworld program {\n");
    for name in names {
        text.push_str("  import ");
        text.push_str(name);
        text.push('@');
        text.push_str(WASI);
        text.push_str(";\n");
    }
    let export = match shape {
        Shape::Command => Some("wasi:cli/run"),
        Shape::Server => Some("wasi:http/incoming-handler"),
        Shape::Library => None,
    };
    if let Some(export) = export {
        text.push_str("\n  export ");
        text.push_str(export);
        text.push('@');
        text.push_str(WASI);
        text.push_str(";\n");
    }
    text.push_str("}\n");
    text
}

pub(crate) fn artifact(name: &str, shape: Shape) -> String {
    let snake = name.replace('-', "_");
    match shape {
        Shape::Library => format!("target/{TARGET}/release/lib{snake}.rlib"),
        Shape::Server => format!("target/{TARGET}/release/{snake}.wasm"),
        Shape::Command => format!("target/{TARGET}/release/{name}.wasm"),
    }
}

fn first_difference(old: &str, new: &str) -> usize {
    let mut old_lines = old.lines();
    let mut new_lines = new.lines();
    let mut number = 1;
    loop {
        match (old_lines.next(), new_lines.next()) {
            (Some(a), Some(b)) if a == b => number += 1,
            _ => return number,
        }
    }
}

pub(crate) fn held(
    folder: &Path,
    file: &str,
    text: &str,
    previous: Option<&str>,
) -> Option<Finding> {
    let old = fs::read(folder.join(file)).ok()?;
    let hash = sha256::hex(&old);
    if hash == sha256::hex(text.as_bytes()) || previous == Some(hash.as_str()) {
        return None;
    }
    let line = first_difference(&String::from_utf8_lossy(&old), text);
    Some(Finding {
        file: file.to_string(),
        line,
        code: "generated-edited",
        message: format!(
            "`{file}` is written by check from the ideas, and its text is not the text check wrote; remove the file, or undo the change at line {line}, and change the ideas instead"
        ),
    })
}

fn crates(
    pages: &[Page],
    constraints: &[Constraints],
    findings: &mut Vec<Finding>,
) -> Vec<(String, String)> {
    let mut found: BTreeMap<String, String> = BTreeMap::new();
    for (page, constraint) in pages.iter().zip(constraints) {
        for used in &constraint.crates {
            match found.get(&used.name) {
                Some(version) if *version != used.version => findings.push(Finding {
                    file: page.file.clone(),
                    line: used.line,
                    code: "crate-version",
                    message: format!(
                        "the crate `{}` is used at {} here and at {version} in another idea; use one version",
                        used.name, used.version
                    ),
                }),
                Some(_) => {}
                None => {
                    found.insert(used.name.clone(), used.version.clone());
                }
            }
        }
    }
    found.into_iter().collect()
}

pub(crate) struct Plan {
    pub package: String,
    pub crates: usize,
    pub used: Vec<(String, String)>,
    pub texts: Vec<(String, String)>,
    pub stamps: Vec<Stamp>,
}

pub(crate) fn plan(
    folder: &Path,
    title: &str,
    pages: &[Page],
    constraints: &[Constraints],
    (shape, libraries): (Shape, &[Library]),
    findings: &mut Vec<Finding>,
) -> Plan {
    let top = constraints.first().cloned().unwrap_or_default();
    if !package_name(title) {
        findings.push(Finding {
            file: "idea.md".to_string(),
            line: 1,
            code: "package-name",
            message: format!(
                "the folder's name, `{title}`, is the package's name, so it starts with a lowercase letter, has only lowercase letters, digits, `-` and `_`, and is not a Rust keyword or a name Cargo keeps"
            ),
        });
    }
    let used = crates(pages, constraints, findings);
    let sources: Vec<String> = pages
        .iter()
        .map(|p| p.source.clone())
        .filter(|s| Path::new(s).extension().is_some_and(|e| e == "rs"))
        .collect();
    let web = pages
        .iter()
        .any(|p| p.kind == crate::idea::IdeaKind::Screen);
    let linked: Vec<(String, String)> = libraries
        .iter()
        .map(|l| (l.name.clone(), l.path.clone()))
        .collect();
    let package = Package {
        name: title,
        shape,
        sources: &sources,
        crates: &used,
        libraries: &linked,
        web,
    };
    let files = [
        ("cargo_sha256", CARGO_TOML, cargo_toml(&package)),
        ("world_sha256", WORLD, world(&top.grants, shape)),
    ];
    let previous = pages.first().map(|p| &p.front);
    let mut plan = Plan {
        package: title.to_string(),
        crates: used.len(),
        used: used.clone(),
        texts: Vec::new(),
        stamps: Vec::new(),
    };
    for (key, file, text) in files {
        let stamp = previous.and_then(|f| f.value(key));
        if let Some(finding) = held(folder, file, &text, stamp) {
            findings.push(finding);
        }
        plan.stamps.push(Stamp {
            key,
            file,
            hash: sha256::hex(text.as_bytes()),
        });
        plan.texts.push((file.to_string(), text));
    }
    plan
}

pub(crate) fn collect(folder: &Path, relative: &str, out: &mut Vec<(String, Vec<u8>)>) {
    collect_skipping(folder, relative, &[], out);
}

pub(crate) fn collect_skipping(
    folder: &Path,
    relative: &str,
    skipped: &[&str],
    out: &mut Vec<(String, Vec<u8>)>,
) {
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
        if skipped.contains(&path.as_str()) {
            continue;
        }
        if dir {
            collect_skipping(folder, &path, skipped, out);
        } else if let Ok(bytes) = fs::read(folder.join(&path)) {
            out.push((path, bytes));
        }
    }
}

pub(crate) fn inputs(
    folder: &Path,
    generated: &[(String, String)],
    tests: &str,
    extra: &[(String, Vec<u8>)],
) -> String {
    let mut files: Vec<(String, Vec<u8>)> = generated
        .iter()
        .map(|(file, text)| (file.clone(), text.as_bytes().to_vec()))
        .collect();
    collect(folder, "src", &mut files);
    files.push(("tests".to_string(), tests.as_bytes().to_vec()));
    files.extend(extra.iter().cloned());
    let mut all = Vec::new();
    for (path, bytes) in files {
        all.extend_from_slice(path.as_bytes());
        all.push(0);
        all.extend_from_slice(bytes.len().to_string().as_bytes());
        all.push(0);
        all.extend_from_slice(&bytes);
    }
    sha256::hex(&all)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{Package, cargo_toml, first_difference, held, package_name, world};
    use crate::Shape;
    use crate::constraints::Grant;
    use crate::sha256;

    const HELLO: &str = "[package]
name = \"hello\"
version = \"0.1.0\"
edition = \"2024\"
publish = false
include = [\"src/main.rs\"]

[[bin]]
name = \"hello\"
path = \"src/main.rs\"

[lints.rust]
warnings = \"deny\"
unsafe_code = \"forbid\"
unused_must_use = \"deny\"

[lints.clippy]
pedantic = { level = \"deny\", priority = -1 }
unwrap_used = \"deny\"
expect_used = \"deny\"
indexing_slicing = \"deny\"

[profile.release]
opt-level = 3
panic = \"abort\"
lto = true
codegen-units = 1
strip = true

[workspace]
";

    #[test]
    fn hello_world_gets_a_binary_with_no_dependencies() {
        let sources = ["src/main.rs".to_string()];
        let package = Package {
            name: "hello",
            shape: Shape::Command,
            sources: &sources,
            crates: &[],
            libraries: &[],
            web: false,
        };
        assert_eq!(cargo_toml(&package), HELLO);
    }

    #[test]
    fn a_library_with_crates_and_modules_gets_lib_and_dependencies() {
        let sources = ["src/main.rs".to_string(), "src/words.rs".to_string()];
        let crates = [("widgets".to_string(), "0.2.0".to_string())];
        let package = Package {
            name: "word-count",
            shape: Shape::Library,
            sources: &sources,
            crates: &crates,
            libraries: &[("json".to_string(), "../json".to_string())],
            web: false,
        };
        let text = cargo_toml(&package);
        assert!(text.starts_with("[package]\nname = \"word-count\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\ninclude = [\"src/main.rs\", \"src/words.rs\"]\nautobins = false\n\n[lib]\nname = \"word_count\"\npath = \"src/main.rs\"\n\n[dependencies]\njson = { path = \"../json\" }\nwidgets = \"=0.2.0\"\n\n[lints.rust]\n"));
        assert!(text.contains("indexing_slicing = \"deny\"\nmissing_errors_doc = \"allow\"\nmissing_panics_doc = \"allow\"\n"));
        assert!(text.ends_with("codegen-units = 1\nstrip = true\n\n[workspace]\n"));
    }

    #[test]
    fn hello_world_gets_the_always_set_only() {
        assert_eq!(
            world(&[], Shape::Command),
            "package lengua:program;

world program {
  import wasi:cli/environment@0.2.12;
  import wasi:cli/exit@0.2.12;
  import wasi:cli/stderr@0.2.12;
  import wasi:cli/stdin@0.2.12;
  import wasi:cli/stdout@0.2.12;
  import wasi:cli/terminal-input@0.2.12;
  import wasi:cli/terminal-output@0.2.12;
  import wasi:cli/terminal-stderr@0.2.12;
  import wasi:cli/terminal-stdin@0.2.12;
  import wasi:cli/terminal-stdout@0.2.12;
  import wasi:clocks/monotonic-clock@0.2.12;
  import wasi:io/error@0.2.12;
  import wasi:io/poll@0.2.12;
  import wasi:io/streams@0.2.12;
  import wasi:random/insecure-seed@0.2.12;

  export wasi:cli/run@0.2.12;
}
"
        );
    }

    #[test]
    fn each_grant_adds_its_interfaces_once() {
        let text = world(
            &[
                Grant::StandardInput,
                Grant::Arguments,
                Grant::Clock,
                Grant::Random,
                Grant::Files,
                Grant::Files,
                Grant::Network,
            ],
            Shape::Library,
        );
        for name in [
            "wasi:cli/stdin",
            "wasi:cli/terminal-stdin",
            "wasi:cli/environment",
            "wasi:clocks/wall-clock",
            "wasi:clocks/monotonic-clock",
            "wasi:random/random",
            "wasi:filesystem/preopens",
            "wasi:filesystem/types",
            "wasi:sockets/tcp",
            "wasi:sockets/ip-name-lookup",
        ] {
            let line = format!("  import {name}@0.2.12;\n");
            assert_eq!(text.matches(&line).count(), 1, "{name} in {text}");
        }
        assert!(!text.contains("export"));
        assert!(text.contains("  import wasi:sockets/udp@0.2.12;\n"));
        assert_eq!(text.matches("import").count(), 27);
    }

    #[test]
    fn standard_input_and_arguments_add_nothing_and_the_clock_adds_the_wall_clock() {
        let none = world(&[], Shape::Command);
        assert_eq!(
            world(&[Grant::StandardInput, Grant::Arguments], Shape::Command),
            none
        );
        let clock = world(&[Grant::Clock], Shape::Command);
        assert_eq!(clock.matches("import").count(), 16);
        assert!(clock.contains("  import wasi:clocks/wall-clock@0.2.12;\n"));
        assert!(!none.contains("wall-clock"));
        assert!(!none.contains("wasi:random/random"));
        assert!(none.contains("  import wasi:random/insecure-seed@0.2.12;\n"));
        assert!(!none.contains("wasi:filesystem"));
    }

    #[test]
    fn package_names_are_what_cargo_takes() {
        assert!(package_name("hello"));
        assert!(package_name("word-count_2"));
        assert!(!package_name("Hello"));
        assert!(!package_name("2hello"));
        assert!(!package_name("hello world"));
        assert!(!package_name("test"));
        assert!(!package_name(""));
    }

    #[test]
    fn a_hand_edit_is_a_finding_and_the_text_check_wrote_is_not() {
        let folder = std::env::temp_dir().join(format!("lengua-idea-{}-held", std::process::id()));
        fs::create_dir_all(&folder).expect("a folder");
        let _ = fs::remove_file(folder.join("Cargo.toml"));
        assert_eq!(held(&folder, "Cargo.toml", "a\n", None), None);
        fs::write(folder.join("Cargo.toml"), "a\n").expect("a file");
        assert_eq!(held(&folder, "Cargo.toml", "a\n", None), None);
        let last = sha256::hex(b"a\n");
        assert_eq!(held(&folder, "Cargo.toml", "b\n", Some(&last)), None);
        assert!(held(&folder, "Cargo.toml", "b\n", None).is_some());
        fs::write(folder.join("Cargo.toml"), "a\nedited\n").expect("a file");
        let finding = held(&folder, "Cargo.toml", "a\n", Some(&last)).expect("a finding");
        assert_eq!(finding.code, "generated-edited");
        assert_eq!(finding.file, "Cargo.toml");
        assert_eq!(finding.line, 2);
    }

    #[test]
    fn the_first_difference_is_a_line_number() {
        assert_eq!(first_difference("a\nb\nc\n", "a\nb\nc\n"), 4);
        assert_eq!(first_difference("a\nx\nc\n", "a\nb\nc\n"), 2);
        assert_eq!(first_difference("a\n", "a\nb\n"), 2);
    }

    #[test]
    fn a_server_is_a_cdylib_that_exports_the_handler() {
        let sources = ["src/main.rs".to_string()];
        let package = Package {
            name: "shop",
            shape: Shape::Server,
            sources: &sources,
            crates: &[],
            libraries: &[],
            web: false,
        };
        let text = cargo_toml(&package);
        assert!(text.contains(
            "[lib]\nname = \"shop\"\npath = \"src/main.rs\"\ncrate-type = [\"cdylib\"]\n"
        ));
        assert!(!text.contains("missing_errors_doc"));
        let world = world(&[Grant::Http], Shape::Server);
        assert!(world.contains("  import wasi:http/types@0.2.12;\n"));
        assert!(world.contains("  export wasi:http/incoming-handler@0.2.12;\n"));
        assert!(!world.contains("wasi:cli/run"));
        assert_eq!(
            super::artifact("my-shop", Shape::Server),
            "target/wasm32-wasip2/release/my_shop.wasm"
        );
        assert_eq!(
            super::artifact("my-shop", Shape::Command),
            "target/wasm32-wasip2/release/my-shop.wasm"
        );
    }
}
