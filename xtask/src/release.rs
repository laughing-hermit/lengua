//! `cargo xtask release [<target>]`: lengua's release archive for the system and CPU of the
//! computer it runs on, one of the five lengua is built for: linux-x86_64, linux-aarch64,
//! macos-aarch64, windows-x86_64 and windows-aarch64. It writes
//! dist/lengua-<version>-<target>.tar.gz (.zip on Windows), and its SHA-256 beside it in the same
//! name with .sha256 added.
//!
//! The archive is the plugin, ready to load: the skills and the manifests, server/lengua-mcp
//! (.exe on Windows, which the manifests then start), solvers/ with the z3 and cvc5 that
//! xtask/tools.tsv pins for the target, LICENSE and licenses/ (the licences of everything it
//! ships: lengua's, the solvers' with those of the parts cvc5 links in, and those of the Rust
//! crates in lengua-mcp, which licenses/README.md lists), TARGET (the system and CPU it is for),
//! INSTALL.md, and install.sh (install.ps1 on Windows), which checks that the computer is that
//! system and CPU and puts the plugin where Claude Code finds plugins. CI installs and checks each
//! archive on a clean runner of its own system and CPU (release mode, .github/workflows/ci.yml).
//! It runs git, cargo, readelf or otool (for what z3 links), and Claude Code's own check of the
//! plugin where `claude` is installed. Nothing is published, signed or pushed.
//!
//! `cargo xtask release-check` builds the release check (tests/release_check.rs) into
//! dist/check/, to run where the archive is installed, on a runner that builds nothing.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{ci, clock, fetch, files, http};

/// How the release task is called.
const USAGE: &str = "usage: cargo xtask release [<this computer's target>]";

/// Where GitHub serves a repository's files at a commit or a tag.
const RAW: &str = "https://raw.githubusercontent.com";

/// The names of a licence's file in a solver's archive, by how they start, in lowercase.
const LICENCE_NAMES: [&str; 6] = [
    "license",
    "licence",
    "copying",
    "copyright",
    "notice",
    "authors",
];

/// The names of the licence files a crate ships, likewise.
const CRATE_LICENCE_NAMES: [&str; 6] = [
    "license",
    "licence",
    "copying",
    "copyright",
    "notice",
    "unlicense",
];

/// The licences cvc5's release source keeps, for an archive that holds none: its COPYING, the
/// AUTHORS it refers to, and the licences in its licenses/.
const CVC5_LICENCES: [&str; 7] = [
    "COPYING",
    "AUTHORS",
    "licenses/minisat-LICENSE",
    "licenses/lgpl-3.0.txt",
    "licenses/gpl-3.0.txt",
    "licenses/GCC-exception-3.1.txt",
    "licenses/apache-2.0-with-llvm-exceptions.txt",
];

/// A licence file of a part cvc5's release program links in, which its archive lacks.
struct Part {
    /// The folder under licenses/cvc5/ it goes in.
    folder: &'static str,
    /// The part's repository on GitHub.
    repository: &'static str,
    /// The commit cvc5 1.3.4's build takes the part from (cmake/Find*.cmake at its tag).
    commit: &'static str,
    /// The file's path there.
    file: &'static str,
    /// The git object id GitHub gives the file there.
    object: &'static str,
    /// Whether only Windows's cvc5 links the part in.
    windows_only: bool,
}

/// The parts' licences, each checked against its git object id.
const CVC5_PARTS: [Part; 6] = [
    Part {
        folder: "cadical",
        repository: "arminbiere/cadical",
        commit: "a384d221a920d473b770df6a7221f35fc5d99e90",
        file: "LICENSE",
        object: "ead29d7a34956a9668d3f3e3c69a50c7017543c5",
        windows_only: false,
    },
    Part {
        folder: "symfpu",
        repository: "cvc5/symfpu",
        commit: "227a7246b8ce513b393cc2645d6d65d3490ea1de",
        file: "LICENSE",
        object: "498e55452d14dc4152c03c07b558cabf3f632720",
        windows_only: false,
    },
    Part {
        folder: "symfpu",
        repository: "cvc5/symfpu",
        commit: "227a7246b8ce513b393cc2645d6d65d3490ea1de",
        file: "LICENSE-BSD",
        object: "264cafdb519f07e99287c2e6b46586c0e5a22f0d",
        windows_only: false,
    },
    Part {
        folder: "symfpu",
        repository: "cvc5/symfpu",
        commit: "227a7246b8ce513b393cc2645d6d65d3490ea1de",
        file: "AUTHORS",
        object: "f7db0e3b7c75176c44d035b1dde296740004c31a",
        windows_only: false,
    },
    Part {
        folder: "libpoly",
        repository: "SRI-CSL/libpoly",
        commit: "3d5e6fbd98639cb0941b029e23ca17c0e3c4a80a",
        file: "LICENCE",
        object: "65c5ca88a67c30becee01c5a8816d964b03862f9",
        windows_only: false,
    },
    Part {
        folder: "mingw-w64",
        repository: "mingw-w64/mingw-w64",
        commit: "de62c283f3383bd825c98518cb0a42ba071d4ac2",
        file: "COPYING.MinGW-w64-runtime/COPYING.MinGW-w64-runtime.txt",
        object: "ca6a077e24b81d4f0ae9b952b6fbcaf370a78d6a",
        windows_only: true,
    },
];

/// The Ubuntu packages whose libraries Linux's static cvc5 holds, for its C library and its
/// prompt: their copyright files hold the notices.
const UBUNTU_PACKAGES: [&str; 4] = ["libc6", "libedit2", "libbsd0", "libtinfo6"];

/// INSTALL.md's way to install, on Windows.
const INSTALL_WINDOWS: &str = "In PowerShell, in this folder, run `powershell -ExecutionPolicy \
                               Bypass -File .\\install.ps1`.";

/// The install script's options, on Windows.
const OPTIONS_WINDOWS: &str = "`-Check` only checks, and `-Prefix <folder>` makes `<folder>` \
                               lengua's marketplace folder, with the plugin in a `lengua` folder \
                               in it, where what it replaces must be an earlier lengua install.";

/// The way to install, on Linux and macOS.
const INSTALL_UNIX: &str = "In this folder, run `./install.sh`.";

/// The install script's options, on Linux and macOS.
const OPTIONS_UNIX: &str = "`./install.sh --check` only checks, `--prefix=<folder>` makes \
                            `<folder>` lengua's marketplace folder, with the plugin in its \
                            `lengua/` folder, where what it replaces must be an earlier lengua \
                            install, and `--link` also puts `lengua-mcp` in `~/.local/bin`, to \
                            run programs from a shell.";

/// What INSTALL.md adds on macOS: a browser marks what it downloads, and macOS won't start an
/// unsigned program so marked, and whether to trust the archive is the user's, so the install
/// script only says how.
const MARK_MACOS: &str = " On macOS a browser marks what it downloads, and macOS won't start \
                          the unsigned server or solvers so marked: the script says so, with \
                          the command that clears the mark, and clears nothing itself, since \
                          whether to trust this unsigned archive is yours to decide.";

/// How to try the plugin for one session, on macOS.
const TRY_MACOS: &str = "clear that mark from this folder first (`xattr -dr \
                         com.apple.quarantine <this folder>`), then load it for one session:";

/// The target Rust's host `host` builds for, and how people say it.
pub fn target_of(host: &str) -> Option<(&'static str, &'static str)> {
    match host {
        "x86_64-unknown-linux-gnu" => Some(("linux-x86_64", "Linux on x86-64")),
        "aarch64-unknown-linux-gnu" => Some(("linux-aarch64", "Linux on ARM64")),
        "aarch64-apple-darwin" => Some(("macos-aarch64", "macOS on ARM64 (Apple silicon)")),
        "x86_64-pc-windows-msvc" => Some(("windows-x86_64", "Windows on x86-64")),
        "aarch64-pc-windows-msvc" => Some(("windows-aarch64", "Windows on ARM64")),
        _ => None,
    }
}

/// The first `version = "..."` line of `manifest`, a Cargo.toml: the package's version.
pub fn version_in(manifest: &str) -> Option<&str> {
    manifest
        .lines()
        .find_map(|line| line.strip_prefix("version = \"")?.strip_suffix('"'))
}

/// The repository (`owner/name`) and the tag of a GitHub release download's address.
pub fn github_release(url: &str) -> Option<(&str, &str)> {
    let rest = url.strip_prefix("https://github.com/")?;
    let (repository, after) = rest.split_once("/releases/download/")?;
    let (tag, _) = after.split_once('/')?;
    (repository.split('/').count() == 2).then_some((repository, tag))
}

/// Whether the file at `path`, `./` and its path below an unpacked archive, is a licence the
/// archive ships: by its name or in a licenses/ folder, but not Python's.
pub fn licence_like(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    let named = LICENCE_NAMES.iter().any(|start| name.starts_with(start));
    let python = path.contains("/python/") || path.ends_with(".py");
    (named || path.contains("/licenses/")) && !python
}

/// `names` in Markdown, as a sentence says them: `` `a`, `b` and `c` ``.
pub fn listed(names: &[String]) -> String {
    let quoted: Vec<String> = names.iter().map(|name| format!("`{name}`")).collect();
    match quoted.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The files of `folder` at any depth, by their paths below it in byte order, as a sentence.
fn listed_files(folder: &Path) -> String {
    let mut names: Vec<String> = files::files(folder)
        .iter()
        .map(|file| files::relative(file, folder))
        .collect();
    names.sort();
    listed(&names)
}

/// A line of `cargo tree --format '{p}|{l}|{r}'`: the crate's name, its version, its licence and
/// its repository.
pub fn tree_line(line: &str) -> (&str, &str, &str, &str) {
    let mut fields = line.splitn(3, '|');
    let package = fields.next().unwrap_or_default();
    let licence = fields.next().unwrap_or_default();
    let repository = fields.next().unwrap_or_default();
    let name = package.split(' ').next().unwrap_or_default();
    let version = package
        .split_once(" v")
        .map_or("", |(_, rest)| rest.split(' ').next().unwrap_or_default());
    (name, version, licence, repository)
}

/// A size as `du -h` says it: in KiB, MiB or GiB, rounded up, to a tenth below 10.
pub fn human(bytes: u64) -> String {
    let units = ["K", "M", "G", "T"];
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < units.len() {
        value /= 1024.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{:.1}{}", (value * 10.0).ceil() / 10.0, units[unit])
    } else {
        format!("{}{}", value.ceil(), units[unit])
    }
}

/// What `git` prints with `arguments`, in the repository `root`.
fn git(root: &Path, arguments: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.current_dir(root).args(arguments);
    ci::output_of(&mut command).ok_or_else(|| format!("{} failed", ci::shown(&command)))
}

/// What a program lists of what `program` links: readelf's dynamic section, or otool's
/// libraries, or, where neither runs, a line that names libz3, so that z3's library ships.
fn links(program: &Path) -> String {
    for (tool, flag) in [("readelf", "-d"), ("otool", "-L")] {
        if let Some(printed) = ci::output_of(Command::new(tool).arg(flag).arg(program)) {
            return printed;
        }
    }
    "libz3, as nothing here lists".to_owned()
}

/// The regular files directly in `folder`, each by its name and its path, in byte order.
fn files_in(folder: &Path) -> Vec<(String, PathBuf)> {
    let mut found: Vec<(String, PathBuf)> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| (entry.file_name().to_string_lossy().into_owned(), entry.path()))
        .collect();
    found.sort();
    found
}

/// Writes `bytes` to `path`.
fn write(path: &Path, bytes: impl AsRef<[u8]>) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|error| format!("{}: {error}", path.display()))
}

/// Reads the file `path`.
fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
}

/// Copies `from` to `path`, making the folder it goes in.
fn copy_into(from: &Path, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        files::folder(parent)?;
    }
    files::copy(from, path)
}

/// lengua-mcp, built as a release: with none of the shell's RUSTFLAGS, so that it is the same
/// wherever it is built, and on Windows with the C runtime linked in, so that it needs no Visual
/// C++ redistributable (`--target` keeps that flag off build scripts and procedural macros).
fn build(root: &Path, host: &str, windows: bool) -> Result<PathBuf, String> {
    let mut command = ci::cargo();
    command
        .current_dir(root)
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .args(["build", "--release", "--locked", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .args(["--bin", "lengua-mcp"]);
    if windows {
        command
            .env("RUSTFLAGS", "-C target-feature=+crt-static")
            .args(["--target", host]);
        ci::must(&mut command)?;
        let release = root.join("target").join(host).join("release");
        Ok(release.join("lengua-mcp.exe"))
    } else {
        command.env("RUSTFLAGS", "");
        ci::must(&mut command)?;
        Ok(root.join("target").join("release").join("lengua-mcp"))
    }
}

/// The plugin as the repository keeps it, by git's own list of plugin/: the skills, the
/// manifests, its README and LICENSE; on Windows both manifests start the server by its name
/// with .exe.
fn plugin(root: &Path, out: &Path, windows: bool) -> Result<(), String> {
    let listed = git(root, &["ls-files", "-z", "plugin"])?;
    for file in listed.split('\0').filter(|file| !file.is_empty()) {
        let kept = file.strip_prefix("plugin/").unwrap_or(file);
        copy_into(&root.join(file), &out.join(kept))?;
    }
    if windows {
        for manifest in ["mcp.json", ".mcp.json"] {
            let path = out.join(manifest);
            let text = String::from_utf8_lossy(&read(&path)?).into_owned();
            let started: String = text
                .split_inclusive('\n')
                .map(|line| line.replacen("/server/lengua-mcp\"", "/server/lengua-mcp.exe\"", 1))
                .collect();
            if !started.contains("/server/lengua-mcp.exe\"") {
                return Err(format!("{manifest} doesn't start server/lengua-mcp"));
            }
            write(&path, started)?;
        }
    }
    Ok(())
}

/// z3's files that go beside it, from its archive's folder `folder`: on Windows the DLLs beside
/// z3.exe but Microsoft.Z3.dll, z3's .NET binding, which z3 never loads; elsewhere z3's own
/// library only when the z3 program links it (as readelf or otool lists it), since a z3 with it
/// built in, such as Linux's, needs nothing beside it.
fn beside_z3(program: &Path, folder: &Path, solvers: &Path, windows: bool) -> Result<(), String> {
    if !windows && !links(program).contains("libz3") {
        return Ok(());
    }
    for (name, path) in files_in(folder) {
        let shipped = if windows {
            name.ends_with(".dll") && name != "Microsoft.Z3.dll"
        } else {
            name.starts_with("libz3.so") || name == "libz3.dylib"
        };
        if shipped {
            files::copy(&path, &solvers.join(&name))?;
        }
    }
    Ok(())
}

/// Copies the licences of the solver `tool`, whose release archive is unpacked in `unpacked` and
/// was downloaded from `url`, into `licences`: those its archive holds, with the list of authors
/// its copyright notice refers to, by their paths below its one top folder, or, for an archive
/// that holds none, those of its release's source, fetched from its tag, each with its SHA-256
/// in the log.
fn solver_licences(tool: &str, unpacked: &Path, url: &str, licences: &Path) -> Result<(), String> {
    files::folder(licences)?;
    let tops: Vec<PathBuf> = std::fs::read_dir(unpacked)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .map(|entry| entry.path())
        .collect();
    let top = match tops.as_slice() {
        [only] if only.is_dir() => Some(only.as_path()),
        _ => None,
    };
    let mut found: Vec<String> = files::files(unpacked)
        .iter()
        .map(|file| files::relative(file, unpacked))
        .filter(|relative| licence_like(&format!("./{relative}")))
        .collect();
    found.sort();
    for relative in &found {
        let file = unpacked.join(relative);
        let kept = files::relative(&file, top.unwrap_or(unpacked));
        copy_into(&file, &licences.join(&kept))?;
        println!("{tool}'s licence {kept}, from its archive");
    }
    if !found.is_empty() {
        return Ok(());
    }
    let (repository, tag) =
        github_release(url).ok_or_else(|| format!("{url} isn't a GitHub release's download"))?;
    let kept: &[&str] = if tool == "z3" {
        &["LICENSE.txt"]
    } else {
        &CVC5_LICENCES
    };
    for name in kept {
        let source = format!("{RAW}/{repository}/{tag}/{name}");
        let bytes = http::get(&source)?;
        let path = licences.join(name);
        if let Some(parent) = path.parent() {
            files::folder(parent)?;
        }
        write(&path, &bytes)?;
        println!(
            "{tool}'s licence {name}, which its archive lacks, from {source}: SHA-256 {}",
            fetch::sha256(&bytes)
        );
    }
    Ok(())
}

/// cvc5's release build is one program with the parts it links in, and its archive holds the
/// licences of only some of them: the others' are fetched from [`CVC5_PARTS`], each checked
/// against its git object id.
fn cvc5_parts(root: &Path, licences: &Path, windows: bool) -> Result<(), String> {
    for part in &CVC5_PARTS {
        if part.windows_only && !windows {
            continue;
        }
        let name = part.file.rsplit('/').next().unwrap_or(part.file);
        let path = licences.join(part.folder).join(name);
        let source = format!("{RAW}/{}/{}/{}", part.repository, part.commit, part.file);
        files::folder(&licences.join(part.folder))?;
        write(&path, http::get(&source)?)?;
        let shown = path.to_string_lossy();
        let object = git(root, &["hash-object", "--no-filters", &shown])?;
        if object.trim() != part.object {
            return Err(format!(
                "{source} is the git object {}, not the {} the release pins",
                object.trim(),
                part.object
            ));
        }
        println!("cvc5's part's licence {}/{name}, from {source}", part.folder);
    }
    Ok(())
}

/// On Linux cvc5's program is static: it holds the glibc, and for its prompt the libedit, libbsd
/// and ncurses' libtinfo, of the Ubuntu 22.04 that cvc5's build runs on. Their notices are the
/// copyright files of Ubuntu's packages of them, taken from this computer's, an Ubuntu as CI's
/// Linux runners are, whose packages of these libraries carry the same licences, with the GNU
/// LGPL 2.1 that glibc's names.
fn ubuntu_licences(licences: &Path) -> Result<(), String> {
    let ubuntu = licences.join("ubuntu");
    for package in UBUNTU_PACKAGES {
        let candidates = [
            PathBuf::from(format!("/usr/share/doc/{package}/copyright")),
            PathBuf::from(format!("/usr/share/doc/{package}t64/copyright")),
        ];
        let Some(copyright) = candidates.iter().find(|candidate| candidate.is_file()) else {
            return Err(format!(
                "This computer has no copyright file of Ubuntu's {package}, which holds the \
                 notices of a library cvc5 links in: build the release on Ubuntu, with {package} \
                 installed."
            ));
        };
        copy_into(copyright, &ubuntu.join(package).join("copyright"))?;
        println!(
            "cvc5's part's licence ubuntu/{package}/copyright, from {}",
            copyright.display()
        );
    }
    let lgpl = Path::new("/usr/share/common-licenses/LGPL-2.1");
    files::copy(lgpl, &ubuntu.join("LGPL-2.1"))
}

/// Cargo's home, which holds its copies of the crates' sources: `CARGO_HOME`, or `.cargo` in the
/// user's home.
fn cargo_home() -> Result<PathBuf, String> {
    if let Some(home) = std::env::var_os("CARGO_HOME").filter(|home| !home.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    let name = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(name)
        .map(|home| PathBuf::from(home).join(".cargo"))
        .ok_or_else(|| format!("neither CARGO_HOME nor {name} names Cargo's home"))
}

/// The licences of the Rust crates compiled into lengua-mcp for this target, from Cargo's own
/// copies of their sources: each crate's name, version, licence and repository, then the licence
/// files it ships. lengua's own two crates are under LICENSE.
fn rust_crates(root: &Path, host: &str, version: &str, target: &str) -> Result<Vec<u8>, String> {
    let mut command = ci::cargo();
    command
        .current_dir(root)
        .args(["tree", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .args(["--locked", "--package", "lengua-mcp", "--target", host])
        .args(["--edges", "no-build,no-dev,no-proc-macro"])
        .args(["--prefix", "none", "--format", "{p}|{l}|{r}"])
        .stderr(Stdio::inherit());
    let printed =
        ci::output_of(&mut command).ok_or_else(|| format!("{} failed", ci::shown(&command)))?;
    let lines: BTreeSet<&str> = printed
        .lines()
        .map(|line| line.strip_suffix(" (*)").unwrap_or(line))
        .collect();
    let registry = cargo_home()?.join("registry").join("src");
    let mut indexes: Vec<PathBuf> = std::fs::read_dir(&registry)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .collect();
    indexes.sort();
    let mut text = Vec::new();
    let _ = write!(
        text,
        "The Rust crates compiled into lengua-mcp {version} for {target}: for each, a line\n\
         \"== <crate> <version>: <licence> (<repository>)\", then the licence files the crate \
         ships,\neach after a line \"-- <file>\".\n"
    );
    for line in lines {
        let (name, crate_version, licence, repository) = tree_line(line);
        if name == "lengua-mcp" || name == "lengua-os" {
            continue;
        }
        let stated = if licence.is_empty() {
            "no licence named"
        } else {
            licence
        };
        let place = if repository.is_empty() {
            "no repository named"
        } else {
            repository
        };
        let _ = write!(text, "\n== {name} {crate_version}: {stated} ({place})\n");
        let folder = indexes
            .iter()
            .map(|index| index.join(format!("{name}-{crate_version}")))
            .find(|candidate| candidate.is_dir())
            .ok_or_else(|| {
                format!(
                    "Cargo's registry, {}, has no copy of {name} {crate_version}",
                    registry.display()
                )
            })?;
        let mut shipped = 0;
        for (file_name, path) in files_in(&folder) {
            let lower = file_name.to_ascii_lowercase();
            if CRATE_LICENCE_NAMES
                .iter()
                .any(|start| lower.starts_with(start))
            {
                let _ = writeln!(text, "-- {file_name}");
                text.extend_from_slice(&read(&path)?);
                text.push(b'\n');
                shipped += 1;
            }
        }
        if shipped == 0 {
            let stated = if licence.is_empty() {
                "unnamed"
            } else {
                licence
            };
            let _ = writeln!(
                text,
                "-- (the crate ships no licence file; its licence is {stated})"
            );
        }
    }
    Ok(text)
}

/// What solvers/ keeps beside z3 and cvc5, as Markdown lists: z3's own library where z3 loads
/// it, and on Windows Microsoft's Visual C++ runtime.
fn beside(names: &[String], exe: &str) -> (String, String) {
    let solvers = [format!("z3{exe}"), format!("cvc5{exe}")];
    let (mut own, mut microsoft) = (Vec::new(), Vec::new());
    for name in names.iter().filter(|name| !solvers.contains(name)) {
        let runtime = ["vcruntime", "msvcp", "vcomp", "concrt"]
            .iter()
            .any(|start| name.starts_with(start));
        if runtime {
            microsoft.push(format!("`{name}`"));
        } else {
            own.push(format!("`{name}`"));
        }
    }
    (own.join(", "), microsoft.join(", "))
}

/// licenses/README.md: what the archive ships, and under which licence each part is.
struct Readme<'a> {
    /// lengua's version.
    version: &'a str,
    /// The target.
    target: &'a str,
    /// The target, as people say it.
    described: &'a str,
    /// The target's programs' suffix.
    exe: &'a str,
    /// The files of solvers/, by name.
    solvers: &'a [String],
    /// z3's source, and cvc5's.
    sources: &'a [String; 2],
    /// The licences' folder.
    licences: &'a Path,
}

impl Readme<'_> {
    /// The README's text.
    fn text(&self) -> String {
        let (version, described, exe) = (self.version, self.described, self.exe);
        let (own, microsoft) = beside(self.solvers, exe);
        let z3_beside = if own.is_empty() {
            String::new()
        } else {
            format!(" with its {own} beside it")
        };
        let z3_runtime = if microsoft.is_empty() {
            String::new()
        } else {
            format!(
                "
- **Microsoft's Visual C++ runtime**, {microsoft} in `solvers/`, which z3's archive ships for
  z3 and which Microsoft lets a program built with Visual C++ ship with it:
  https://learn.microsoft.com/cpp/windows/redistributing-visual-cpp-files."
            )
        };
        let [z3_source, cvc5_source] = self.sources;
        let z3_listed = listed_files(&self.licences.join("z3"));
        let cvc5_listed = listed_files(&self.licences.join("cvc5"));
        let (gmp, system) = gmp_and_system(self.target);
        format!(
            "# What lengua {version} for {described} ships, and its licences

- **lengua**: the skills, the manifests and `server/lengua-mcp{exe}`, under the Apache License 2.0,
  in `LICENSE` beside this folder.
- **The Rust crates compiled into lengua-mcp**: each one's licence and the licence files it
  ships, in `rust-crates.txt`.
- **z3**, `solvers/z3{exe}`{z3_beside}, as its project releases it, under the MIT License:
  `z3/` holds {z3_listed}. Its source is at {z3_source}.{z3_runtime}
- **cvc5**, `solvers/cvc5{exe}`, as its project releases it: one program with the parts it links
  in, under the BSD 3-Clause License in `cvc5/COPYING`, which `cvc5/AUTHORS` goes with. Its
  source is at {cvc5_source}. `cvc5/` holds {cvc5_listed}:
  - from cvc5's archive, `COPYING`, `AUTHORS` and `licenses/`: MiniSat's licence (cvc5
    holds MiniSat's code), the GNU LGPL 3 and the GNU GPL 3 it builds on, the GCC Runtime
    Library Exception, and the Apache License 2.0 with LLVM Exceptions;
  - `cadical/`: CaDiCaL, cvc5's SAT solver, under the MIT License, from
    https://github.com/arminbiere/cadical/tree/a384d221a920d473b770df6a7221f35fc5d99e90;
  - `symfpu/`: SymFPU, its floating-point arithmetic, under the BSD 3-Clause License, which it
    offers beside the GNU GPL 3, from
    https://github.com/cvc5/symfpu/tree/227a7246b8ce513b393cc2645d6d65d3490ea1de;
  - `libpoly/`: LibPoly 0.2.1, its polynomial arithmetic, under the GNU LGPL 3, from
    https://github.com/SRI-CSL/libpoly/tree/v0.2.1;
  - GMP, the GNU Multiple Precision Arithmetic Library, under the GNU LGPL 3
    (`licenses/lgpl-3.0.txt`, with the GNU GPL 3 in `licenses/gpl-3.0.txt`): {gmp};
  - and {system}
"
        )
    }
}

/// What cvc5 links in of GMP, and of its system's own libraries, on `target`'s system.
fn gmp_and_system(target: &str) -> (&'static str, &'static str) {
    let gmp = "version 6.3.0, which cvc5's build compiles from the source at
    https://github.com/cvc5/cvc5-deps/blob/main/gmp-6.3.0.tar.bz2 (GMP's own releases are at
    https://gmplib.org)";
    if target.starts_with("linux-") {
        let system = "its system's libraries, which its static program holds, as cvc5's build \
takes them
    from Ubuntu 22.04: glibc, under the GNU LGPL 2.1 (`ubuntu/LGPL-2.1`); for its prompt,
    libedit with libbsd and ncurses' libtinfo, whose notices are in the copyright files of
    Ubuntu's packages of them (`ubuntu/`, from the Ubuntu that built this archive); and GCC's
    libstdc++ and libgcc, under the GNU GPL 3 with the GCC Runtime Library Exception
    (`licenses/`).";
        (gmp, system)
    } else if target.starts_with("macos-") {
        let system = "none of its system's libraries: cvc5 loads macOS's own libc++ and libedit, \
which
    this archive doesn't ship.";
        (gmp, system)
    } else {
        let gmp = "the build of it in the MSYS2 environment cvc5's Windows build runs in \
(CLANG64, or
    CLANGARM64 on ARM64), whose source is at https://packages.msys2.org/base/mingw-w64-gmp";
        let system = "the libraries of its build's toolchain: LLVM's libc++, libunwind and \
compiler-rt,
    under the Apache License 2.0 with LLVM Exceptions (`licenses/`), and the MinGW-w64 runtime,
    whose notices are in `mingw-w64/`, from
    https://github.com/mingw-w64/mingw-w64/tree/v13.0.0.";
        (gmp, system)
    }
}

/// INSTALL.md, and the install script it names, which goes into `out` beside it.
fn install(
    root: &Path,
    out: &Path,
    target: &str,
    version: &str,
    described: &str,
) -> Result<(), String> {
    let windows = target.starts_with("windows-");
    let exe = if windows { ".exe" } else { "" };
    let (install, mut options, mut destination) = if windows {
        let script = root.join("scripts").join("install.ps1");
        files::copy(&script, &out.join("install.ps1"))?;
        let destination = "`%LOCALAPPDATA%\\lengua\\plugins\\lengua`";
        (INSTALL_WINDOWS, OPTIONS_WINDOWS.to_owned(), destination)
    } else {
        let script = out.join("install.sh");
        files::copy(&root.join("scripts").join("install.sh"), &script)?;
        ci::executable(&script)?;
        let destination =
            "`~/.local/share/lengua/plugins/lengua` (under `$XDG_DATA_HOME` when it is set)";
        (INSTALL_UNIX, OPTIONS_UNIX.to_owned(), destination)
    };
    let mut how = "load this folder for one session:";
    if target.starts_with("macos-") {
        destination = "`~/Library/Application Support/lengua/plugins/lengua`";
        options.push_str(MARK_MACOS);
        how = TRY_MACOS;
    }
    let text = format!(
        "# lengua {version} for {described}

This folder is the lengua plugin: the skills `write-idea` and `compile`, the MCP server
`server/lengua-mcp{exe}` with its tools `check` and `run`, the z3 and cvc5 solvers its
proofs need, in `solvers/`, and the licences of all of it, in `LICENSE` and `licenses/`.

{install} It checks that this computer is {described}, copies the plugin to {destination}, adds
it to Claude Code when the `claude` command is installed (or says the two commands that do),
and runs `lengua-mcp doctor`, which says whether lengua has what it needs here and what to do
about anything missing. {options}

To try it without installing, {how} `claude --plugin-dir <this folder>`. Other agent
harnesses load it as an Agent Plugins plugin, from `plugin.json` and `mcp.json`. The archive
is unsigned.
"
    );
    write(&out.join("INSTALL.md"), text)
}

/// Every folder and file below `top`, in order, each as its path and whether it is a folder.
fn entries(top: &Path) -> Vec<(PathBuf, bool)> {
    let mut listed: Vec<(PathBuf, bool)> = std::fs::read_dir(top)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| Some((entry.path(), entry.file_type().ok()?.is_dir())))
        .collect();
    listed.sort();
    let mut all = Vec::new();
    for (path, folder) in listed {
        let below = if folder { entries(&path) } else { Vec::new() };
        all.push((path, folder));
        all.extend(below);
    }
    all
}

/// The folder `stage/name` as a gzipped tar archive at `archive`, with each file's mode and time.
fn tarred(stage: &Path, name: &str, archive: &Path) -> Result<(), String> {
    let failed = |error: std::io::Error| format!("{}: {error}", archive.display());
    let file = std::fs::File::create(archive).map_err(failed)?;
    let gzip = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(gzip);
    let top = stage.join(name);
    builder.append_dir(name, &top).map_err(failed)?;
    for (path, folder) in entries(&top) {
        let inside = format!("{name}/{}", files::relative(&path, &top));
        if folder {
            builder.append_dir(&inside, &path).map_err(failed)?;
        } else {
            builder
                .append_path_with_name(&path, &inside)
                .map_err(failed)?;
        }
    }
    builder
        .into_inner()
        .map_err(failed)?
        .finish()
        .map_err(failed)?;
    Ok(())
}

/// The folder `stage/name` as a zip archive at `archive`, deflated, dated now.
fn zipped(stage: &Path, name: &str, archive: &Path) -> Result<(), String> {
    let failed = |error: zip::result::ZipError| format!("{}: {error}", archive.display());
    let unwritten = |error: std::io::Error| format!("{}: {error}", archive.display());
    let file = std::fs::File::create(archive).map_err(unwritten)?;
    let mut writer = zip::ZipWriter::new(file);
    let now = clock::Civil::now();
    let date = zip::DateTime::from_date_and_time(
        now.year as u16,
        now.month as u8,
        now.day as u8,
        now.hour as u8,
        now.minute as u8,
        now.second as u8,
    )
    .map_err(|_| "the clock is outside the dates a zip archive holds".to_owned())?;
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(date);
    let top = stage.join(name);
    writer
        .add_directory(format!("{name}/"), options)
        .map_err(failed)?;
    for (path, folder) in entries(&top) {
        let inside = format!("{name}/{}", files::relative(&path, &top));
        if folder {
            writer
                .add_directory(format!("{inside}/"), options)
                .map_err(failed)?;
        } else {
            writer.start_file(inside, options).map_err(failed)?;
            writer.write_all(&read(&path)?).map_err(unwritten)?;
        }
    }
    writer.finish().map_err(failed)?;
    Ok(())
}

/// `cargo xtask release [<target>]`.
pub fn task(arguments: &[String]) -> Result<u8, String> {
    let asked = match arguments {
        [] => None,
        [target] => Some(target.as_str()),
        _ => return Err(USAGE.to_owned()),
    };
    let root = ci::root()?;
    let manifest = String::from_utf8_lossy(&read(&root.join("Cargo.toml"))?).into_owned();
    let version = version_in(&manifest)
        .ok_or("Cargo.toml names no version")?
        .to_owned();
    let rustc = ci::output_of(Command::new("rustc").arg("-vV")).ok_or("rustc -vV didn't run")?;
    let host = rustc
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or("rustc -vV names no host")?
        .to_owned();
    let Some((target, described)) = target_of(&host) else {
        return Err(format!(
            "lengua is built for linux-x86_64, linux-aarch64, macos-aarch64, windows-x86_64 and \
             windows-aarch64, and Rust here builds for {host}."
        ));
    };
    if let Some(asked) = asked
        && asked != target
    {
        return Err(format!(
            "cargo xtask release builds the archive of the computer it runs on, {target}, not \
             {asked}."
        ));
    }
    let windows = target.starts_with("windows-");
    let exe = if windows { ".exe" } else { "" };
    let name = format!("lengua-{version}-{target}");
    let built = build(&root, &host, windows)?;

    let stage = tempfile::tempdir().map_err(|error| format!("a staging folder: {error}"))?;
    let out = stage.path().join(&name);
    plugin(&root, &out, windows)?;
    let server = out.join("server").join(format!("lengua-mcp{exe}"));
    copy_into(&built, &server)?;
    ci::executable(&server)?;

    // The solvers, each as tools.tsv pins it for the target, checked against its SHA-256.
    let solvers = out.join("solvers");
    let licences = out.join("licenses");
    files::folder(&solvers)?;
    files::folder(&licences)?;
    let mut sources = [String::new(), String::new()];
    for (tool, source) in ["z3", "cvc5"].into_iter().zip(&mut sources) {
        let pinned = fetch::pin(tool, target)
            .ok_or_else(|| format!("tools.tsv pins no {tool} for {target}"))?;
        let url = pinned.url;
        let unpacked = stage.path().join("download").join(tool);
        files::folder(&unpacked)?;
        let archive = unpacked.join(format!("{tool}.zip"));
        fetch::fetch(&pinned, &archive)?;
        fetch::unzip(&archive, &unpacked)?;
        files::remove(&archive)?;
        let program = fetch::program(&unpacked, tool)
            .ok_or_else(|| format!("the {tool} archive from {url} holds no {tool} program"))?;
        let shipped = solvers.join(format!("{tool}{exe}"));
        files::copy(&program, &shipped)?;
        ci::executable(&shipped)?;
        if tool == "z3" {
            let folder = program.parent().unwrap_or(unpacked.as_path());
            beside_z3(&program, folder, &solvers, windows)?;
        }
        solver_licences(tool, &unpacked, url, &licences.join(tool))?;
        let (repository, tag) =
            github_release(url).ok_or_else(|| format!("{url} isn't a GitHub release's download"))?;
        *source = format!("https://github.com/{repository}/tree/{tag}");
    }
    let names: Vec<String> = files_in(&solvers)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    let mut shown = String::new();
    for name in &names {
        let _ = write!(shown, "{name} ");
    }
    println!("solvers/: {shown}");

    cvc5_parts(&root, &licences.join("cvc5"), windows)?;
    if target.starts_with("linux-") {
        ubuntu_licences(&licences.join("cvc5"))?;
    }
    let crates = rust_crates(&root, &host, &version, target)?;
    let counted = String::from_utf8_lossy(&crates)
        .lines()
        .filter(|line| line.starts_with("== "))
        .count();
    write(&licences.join("rust-crates.txt"), &crates)?;
    println!("licenses/rust-crates.txt: {counted} crates");
    let readme = Readme {
        version: &version,
        target,
        described,
        exe,
        solvers: &names,
        sources: &sources,
        licences: &licences,
    };
    write(&licences.join("README.md"), readme.text())?;

    // What the archive is for, which the install script holds the computer to, and how to
    // install it.
    write(&out.join("TARGET"), format!("{target}\n"))?;
    install(&root, &out, target, &version, described)?;

    // Claude Code's own check of the plugin as the archive holds it, where claude is installed
    // (CI's runners have none; tests/manifests.rs holds the manifests to the Agent Plugins
    // schemas).
    let validated = Command::new("claude")
        .args(["plugin", "validate"])
        .arg(&out)
        .arg("--strict")
        .status();
    match validated {
        Ok(status) if !status.success() => {
            return Err(format!("claude plugin validate found the plugin wrong: {status}"));
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("claude didn't start: {error}"));
        }
        _ => {}
    }

    let dist = root.join("dist");
    files::folder(&dist)?;
    let archive = if windows {
        format!("{name}.zip")
    } else {
        format!("{name}.tar.gz")
    };
    let path = dist.join(&archive);
    files::remove(&path)?;
    if windows {
        zipped(stage.path(), &name, &path)?;
    } else {
        tarred(stage.path(), &name, &path)?;
    }
    let bytes = read(&path)?;
    let digest = fetch::sha256(&bytes);
    let sums = dist.join(format!("{archive}.sha256"));
    write(&sums, format!("{digest}  {archive}\n"))?;
    println!("Built dist/{archive} ({})", human(bytes.len() as u64));
    ci::append("GITHUB_OUTPUT", &format!("version={version}\n"))?;
    Ok(0)
}

/// `cargo xtask release-check`: the release check, tests/release_check.rs built as a test
/// program, copied to dist/check/release-check (.exe on Windows), to run where the archive is
/// installed, on a runner that builds nothing.
pub fn check(arguments: &[String]) -> Result<u8, String> {
    if !arguments.is_empty() {
        return Err("usage: cargo xtask release-check".to_owned());
    }
    let root = ci::root()?;
    let mut command = ci::cargo();
    command
        .current_dir(&root)
        .args(["test", "--release", "--locked"])
        .args(["--test", "release_check", "--no-run"])
        .arg("--message-format=json")
        .stderr(Stdio::inherit());
    let printed =
        ci::output_of(&mut command).ok_or_else(|| format!("{} failed", ci::shown(&command)))?;
    // The test program's path, from the artifact message that names it.
    let built = printed
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| message["target"]["name"] == "release_check")
        .find_map(|message| message["executable"].as_str().map(PathBuf::from))
        .ok_or("cargo built no release check")?;
    let program = root
        .join("dist")
        .join("check")
        .join(format!("release-check{}", std::env::consts::EXE_SUFFIX));
    copy_into(&built, &program)?;
    ci::executable(&program)?;
    println!("Built {}", program.display());
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `list` as owned names.
    fn owned(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    /// Each of the five hosts Rust builds for is its target, and no other host is one; the
    /// version is the manifest's first.
    #[test]
    fn each_host_is_its_target() {
        assert_eq!(
            target_of("aarch64-apple-darwin"),
            Some(("macos-aarch64", "macOS on ARM64 (Apple silicon)"))
        );
        let windows = target_of("x86_64-pc-windows-msvc").map(|(target, _)| target);
        assert_eq!(windows, Some("windows-x86_64"));
        assert_eq!(target_of("x86_64-apple-darwin"), None);
        let manifest = "[package]\nname = \"lengua-mcp\"\nversion = \"0.2.0\"\n";
        assert_eq!(version_in(manifest), Some("0.2.0"));
    }

    /// A GitHub release download names its repository and tag.
    #[test]
    fn a_release_download_names_its_repository_and_tag() {
        let url = "https://github.com/cvc5/cvc5/releases/download/cvc5-1.3.4/cvc5-Win64.zip";
        assert_eq!(github_release(url), Some(("cvc5/cvc5", "cvc5-1.3.4")));
        assert_eq!(github_release("https://example.com/a.zip"), None);
    }

    /// A licence is found by its name or its folder, and Python's are left out.
    #[test]
    fn licences_are_found_by_name_or_folder() {
        assert!(licence_like("./z3-4.16.0-x64-win/LICENSE.txt"));
        assert!(licence_like("./cvc5/share/licenses/gpl-3.0.txt"));
        assert!(licence_like("./cvc5/AUTHORS"));
        assert!(!licence_like("./z3/bin/python/z3/LICENSE.txt"));
        assert!(!licence_like("./z3/bin/license.py"));
        assert!(!licence_like("./z3/bin/z3"));
    }

    /// Files are listed in Markdown as a sentence says them.
    #[test]
    fn files_are_listed_as_a_sentence_says_them() {
        assert_eq!(listed(&[]), "");
        assert_eq!(listed(&owned(&["a"])), "`a`");
        assert_eq!(listed(&owned(&["a", "b"])), "`a` and `b`");
        assert_eq!(listed(&owned(&["a", "b", "c"])), "`a`, `b` and `c`");
    }

    /// cargo tree's line gives the crate's name, version, licence and repository; a crate of a
    /// path gives its version too.
    #[test]
    fn a_tree_line_gives_the_crate_s_parts() {
        let line = "anyhow v1.0.98|MIT OR Apache-2.0|https://github.com/dtolnay/anyhow";
        let (name, version, licence, repository) = tree_line(line);
        assert_eq!((name, version), ("anyhow", "1.0.98"));
        assert_eq!(licence, "MIT OR Apache-2.0");
        assert_eq!(repository, "https://github.com/dtolnay/anyhow");
        let (name, version, licence, repository) =
            tree_line("lengua-os v0.2.0 (/work/crates/lengua-os)|Apache-2.0|");
        assert_eq!((name, version), ("lengua-os", "0.2.0"));
        assert_eq!((licence, repository), ("Apache-2.0", ""));
    }

    /// What goes beside the solvers is z3's own library or Microsoft's runtime.
    #[test]
    fn what_goes_beside_the_solvers_is_named() {
        let names = owned(&[
            "cvc5.exe",
            "libz3.dll",
            "msvcp140.dll",
            "vcruntime140.dll",
            "z3.exe",
        ]);
        let (own, microsoft) = beside(&names, ".exe");
        assert_eq!(own, "`libz3.dll`");
        assert_eq!(microsoft, "`msvcp140.dll`, `vcruntime140.dll`");
        assert_eq!(beside(&names[..1], ".exe"), (String::new(), String::new()));
    }

    /// Sizes read as `du -h` gives them.
    #[test]
    fn sizes_read_as_du_gives_them() {
        assert_eq!(human(41 * 1024 * 1024), "41M");
        assert_eq!(human(41 * 1024 * 1024 + 1), "42M");
        assert_eq!(human(9 * 1024 * 1024 + 300 * 1024), "9.3M");
        assert_eq!(human(2048), "2.0K");
    }

    /// The README of a Windows archive names Microsoft's runtime and the libraries of cvc5's
    /// toolchain, and lists the licences it holds.
    #[test]
    fn the_readme_names_what_the_archive_ships() {
        let folder = tempfile::tempdir().expect("a folder");
        let licences = folder.path();
        for file in ["z3/LICENSE.txt", "cvc5/COPYING", "cvc5/AUTHORS"] {
            let path = licences.join(file);
            files::folder(path.parent().expect("a folder")).expect("its folder");
            std::fs::write(&path, file).expect("a licence");
        }
        let names = owned(&["cvc5.exe", "libz3.dll", "vcruntime140.dll", "z3.exe"]);
        let sources = [
            "https://github.com/Z3Prover/z3/tree/z3-4.16.0".to_owned(),
            "https://github.com/cvc5/cvc5/tree/cvc5-1.3.4".to_owned(),
        ];
        let readme = Readme {
            version: "0.2.0",
            target: "windows-x86_64",
            described: "Windows on x86-64",
            exe: ".exe",
            solvers: &names,
            sources: &sources,
            licences,
        };
        let text = readme.text();
        let title = "# What lengua 0.2.0 for Windows on x86-64 ships, and its licences\n";
        assert!(text.starts_with(title), "{text}");
        assert!(text.contains("with its `libz3.dll` beside it,"), "{text}");
        assert!(text.contains("runtime**, `vcruntime140.dll` in"), "{text}");
        assert!(text.contains("holds `LICENSE.txt`. Its source"), "{text}");
        assert!(text.contains("holds `AUTHORS` and `COPYING`:"), "{text}");
        assert!(text.ends_with("tree/v13.0.0.\n"), "{text}");
    }
}
