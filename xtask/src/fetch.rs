//! `cargo xtask fetch <target> <what>...`: what xtask/tools.tsv pins for a runner, downloaded and
//! checked against its SHA-256:
//!
//! - `solvers`: z3 and cvc5 in ~/solvers/bin, z3 with the files of its folder, first on the job's
//!   PATH;
//! - `wasi`: wasi-sdk in ~/wasi-sdk, which `WASI_SDK` names for the job's later steps, and Rust's
//!   wasm32-wasip1 target: what `cargo xtask bench-wasm` compiles the benchmark's programs with.
//!
//! CI's caches keep both folders. Each holds pinned.tsv, the pins it was fetched by, so that a
//! folder a cache kept for other pins is fetched again.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest as _, Sha256};

use crate::{ci, files, http};

/// What every runner installs, pinned by SHA-256: a tool, a target, the download's address and
/// its SHA-256 on each line.
pub const TOOLS: &str = include_str!("../tools.tsv");

/// The file in a fetched folder that says which pins it was fetched by.
const STAMP: &str = "pinned.tsv";

/// How the fetch task is called.
const USAGE: &str = "usage: cargo xtask fetch <target> solvers|wasi...";

/// A tool's download for one target, as tools.tsv pins it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pin<'a> {
    /// Where it is downloaded from.
    pub url: &'a str,
    /// Its SHA-256, in hex.
    pub sha256: &'a str,
}

/// The download `tools`, tools.tsv's text, pins for `tool` on `target`.
pub fn pin_in<'a>(tools: &'a str, tool: &str, target: &str) -> Option<Pin<'a>> {
    tools
        .lines()
        .filter(|line| !line.starts_with('#'))
        .find_map(|line| {
            let mut columns = line.split('\t');
            let (name, place) = (columns.next()?, columns.next()?);
            let (url, sha256) = (columns.next()?, columns.next()?);
            (name == tool && place == target).then_some(Pin { url, sha256 })
        })
}

/// The download tools.tsv pins for `tool` on `target`.
pub fn pin(tool: &str, target: &str) -> Option<Pin<'static>> {
    pin_in(TOOLS, tool, target)
}

/// The SHA-256 of `bytes`, in hex.
pub fn sha256(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes).iter() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Downloads what `pin` pins into `file`, which must have its SHA-256, and says so as
/// `sha256sum -c` does.
pub fn fetch(pin: &Pin<'_>, file: &Path) -> Result<(), String> {
    let bytes = http::get(pin.url)?;
    let name = file.file_name().unwrap_or_default().to_string_lossy();
    let found = sha256(&bytes);
    if found != pin.sha256 {
        return Err(format!(
            "{name}'s SHA-256 is {found}, not the {} that tools.tsv pins",
            pin.sha256
        ));
    }
    std::fs::write(file, &bytes).map_err(|error| format!("{}: {error}", file.display()))?;
    println!("{name}: OK");
    Ok(())
}

/// Unpacks the zip archive `file` into the folder `into`, as `unzip` does.
pub fn unzip(file: &Path, into: &Path) -> Result<(), String> {
    let failed = |error: zip::result::ZipError| format!("{}: {error}", file.display());
    let opened =
        std::fs::File::open(file).map_err(|error| format!("{}: {error}", file.display()))?;
    let mut archive = zip::ZipArchive::new(opened).map_err(failed)?;
    archive.extract(into).map_err(failed)
}

/// Unpacks the gzipped tar archive `file` into the folder `into` without the one folder it keeps
/// everything in, as `tar -xzf <file> -C <into> --strip-components=1` does.
pub fn untar_stripped(file: &Path, into: &Path) -> Result<(), String> {
    let unpacked = into.with_extension("unpacking");
    files::remove(&unpacked)?;
    let opened =
        std::fs::File::open(file).map_err(|error| format!("{}: {error}", file.display()))?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(opened));
    archive
        .unpack(&unpacked)
        .map_err(|error| format!("{}: {error}", file.display()))?;
    let tops: Vec<PathBuf> = std::fs::read_dir(&unpacked)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .collect();
    let [top] = tops.as_slice() else {
        return Err(format!(
            "{} doesn't keep everything in one folder",
            file.display()
        ));
    };
    std::fs::rename(top, into).map_err(|error| format!("{}: {error}", into.display()))?;
    files::remove(&unpacked)
}

/// The first file named `name` or `name.exe` at any depth of `folder`, as `find` meets them.
pub fn program(folder: &Path, name: &str) -> Option<PathBuf> {
    let exe = format!("{name}.exe");
    files::files(folder).into_iter().find(|file| {
        file.file_name()
            .is_some_and(|file| file == name || file == exe.as_str())
    })
}

/// The pins `pins` as a folder's pinned.tsv keeps them.
pub fn stamp(pins: &[(&str, Pin<'_>)]) -> String {
    let mut stamp = String::new();
    for (tool, pin) in pins {
        let _ = writeln!(stamp, "{tool}\t{}\t{}", pin.url, pin.sha256);
    }
    stamp
}

/// The pins the folder `folder` was fetched by, or nothing.
pub fn kept(folder: &Path) -> String {
    std::fs::read_to_string(folder.join(STAMP)).unwrap_or_default()
}

/// The user's home folder, where CI's caches keep ~/solvers and ~/wasi-sdk: `HOME`, or on
/// Windows `USERPROFILE`, the folder a cache's `~` names there.
fn home() -> Result<PathBuf, String> {
    let name = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .ok_or_else(|| format!("{name} names no home folder"))
}

/// What `program --version` prints: the program must run.
fn version(program: &Path) -> Result<String, String> {
    let output = Command::new(program)
        .arg("--version")
        .output()
        .map_err(|error| format!("{} didn't start: {error}", program.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} --version failed: {}",
            program.display(),
            output.status
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `cargo xtask fetch <target> <what>...`.
pub fn task(arguments: &[String]) -> Result<u8, String> {
    let Some((target, wanted)) = arguments.split_first() else {
        return Err(USAGE.to_owned());
    };
    if wanted.is_empty() {
        return Err(USAGE.to_owned());
    }
    for what in wanted {
        match what.as_str() {
            "solvers" => solvers(target)?,
            "wasi" => wasi(target)?,
            other => return Err(format!("nothing to fetch named {other:?}\n{USAGE}")),
        }
    }
    Ok(0)
}

/// z3 and cvc5, as tools.tsv pins them for `target`, in ~/solvers/bin, first on the job's PATH.
fn solvers(target: &str) -> Result<(), String> {
    let dest = home()?.join("solvers");
    let bin = dest.join("bin");
    let exe = std::env::consts::EXE_SUFFIX;
    let mut pins = Vec::new();
    for tool in ["z3", "cvc5"] {
        let pinned =
            pin(tool, target).ok_or_else(|| format!("tools.tsv has no {tool} for {target}"))?;
        pins.push((tool, pinned));
    }
    let wanted = stamp(&pins);
    if !(bin.join(format!("cvc5{exe}")).is_file() && kept(&dest) == wanted) {
        files::remove(&dest)?;
        files::folder(&bin)?;
        for (tool, pinned) in &pins {
            let download = dest.join("download").join(tool);
            files::folder(&download)?;
            let archive = download.join(format!("{tool}.zip"));
            fetch(pinned, &archive)?;
            unzip(&archive, &download)?;
            let found = program(&download, tool)
                .ok_or_else(|| format!("no {tool} binary in the {tool} archive"))?;
            if *tool == "z3" {
                files::copy_folder(found.parent().unwrap_or(download.as_path()), &bin)?;
            } else {
                files::copy(&found, &bin.join(found.file_name().unwrap_or_default()))?;
            }
        }
        files::remove(&dest.join("download"))?;
        let entries =
            std::fs::read_dir(&bin).map_err(|error| format!("{}: {error}", bin.display()))?;
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_file()) {
                ci::executable(&entry.path())?;
            }
        }
        std::fs::write(dest.join(STAMP), &wanted)
            .map_err(|error| format!("{}: {error}", dest.display()))?;
    }
    ci::append("GITHUB_PATH", &format!("{}\n", bin.display()))?;
    print!("{}", version(&bin.join(format!("z3{exe}")))?);
    let cvc5 = version(&bin.join(format!("cvc5{exe}")))?;
    println!("{}", cvc5.lines().next().unwrap_or_default());
    Ok(())
}

/// wasi-sdk, as tools.tsv pins it for `target`, in ~/wasi-sdk, which `WASI_SDK` names for the
/// job's later steps, and Rust's wasm32-wasip1 target.
fn wasi(target: &str) -> Result<(), String> {
    let home = home()?;
    let dest = home.join("wasi-sdk");
    let pinned =
        pin("wasi-sdk", target).ok_or_else(|| format!("tools.tsv has no wasi-sdk for {target}"))?;
    let wanted = stamp(&[("wasi-sdk", pinned)]);
    let clang = dest
        .join("bin")
        .join(format!("clang{}", std::env::consts::EXE_SUFFIX));
    if !(clang.is_file() && kept(&dest) == wanted) {
        files::remove(&dest)?;
        let archive = home.join("wasi-sdk.tar.gz");
        fetch(&pinned, &archive)?;
        untar_stripped(&archive, &dest)?;
        files::remove(&archive)?;
        std::fs::write(dest.join(STAMP), &wanted)
            .map_err(|error| format!("{}: {error}", dest.display()))?;
    }
    ci::append("GITHUB_ENV", &format!("WASI_SDK={}\n", dest.display()))?;
    println!("{}", version(&clang)?.lines().next().unwrap_or_default());
    ci::must(Command::new("rustup").args(["target", "add", "wasm32-wasip1"]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The five runners' names.
    const TARGETS: [&str; 5] = [
        "linux-x86_64",
        "linux-aarch64",
        "macos-aarch64",
        "windows-x86_64",
        "windows-aarch64",
    ];

    /// tools.tsv pins z3 and cvc5 for each of the five runners, and wasi-sdk and z3's source for
    /// the one that compiles the bench's modules and z3's, each from GitHub with a SHA-256.
    #[test]
    fn every_tool_is_pinned_where_it_is_used() {
        let mut pinned: Vec<Pin<'_>> = Vec::new();
        for target in TARGETS {
            for tool in ["z3", "cvc5"] {
                pinned.push(pin(tool, target).expect("pinned"));
            }
        }
        pinned.push(pin("wasi-sdk", "linux-x86_64").expect("pinned"));
        pinned.push(pin("z3-source", "wasm32-wasip1").expect("pinned"));
        for pin in pinned {
            assert!(pin.url.starts_with("https://github.com/"), "{pin:?}");
            assert!(
                pin.sha256.len() == 64 && pin.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "{pin:?}"
            );
        }
        assert_eq!(pin("z3", "macos-x86_64"), None);
        let tools = "# a comment\tz3\nz3\tlinux-x86_64\thttps://a/z3.zip\tabc\n";
        assert_eq!(
            pin_in(tools, "z3", "linux-x86_64"),
            Some(Pin {
                url: "https://a/z3.zip",
                sha256: "abc",
            })
        );
    }

    /// SHA-256 as sha256sum prints it.
    #[test]
    fn sha256_is_sha256sum_s() {
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// A zip unpacks as unzip does and a program in it is found by its name; a tar.gz unpacks
    /// without its one folder, as tar's `--strip-components=1` does.
    #[test]
    fn archives_unpack_as_unzip_and_tar_do() {
        let root = tempfile::tempdir().expect("a folder");
        let zipped = root.path().join("tool.zip");
        let file = std::fs::File::create(&zipped).expect("a zip");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer
            .add_directory("tool-1.0/bin", options)
            .expect("a folder");
        writer
            .start_file("tool-1.0/bin/tool", options)
            .expect("a file");
        std::io::Write::write_all(&mut writer, b"#!/bin/sh\n").expect("its bytes");
        writer.finish().expect("the zip");
        let into = root.path().join("unzipped");
        unzip(&zipped, &into).expect("unzipped");
        let found = into.join("tool-1.0").join("bin").join("tool");
        assert_eq!(program(&into, "tool"), Some(found));
        assert_eq!(program(&into, "other"), None);

        let tarred = root.path().join("sdk.tar.gz");
        let file = std::fs::File::create(&tarred).expect("a tar.gz");
        let gzip = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(gzip);
        let mut header = tar::Header::new_gnu();
        header.set_size(5);
        header.set_mode(0o755);
        builder
            .append_data(&mut header, "sdk-1.0/bin/clang", &b"clang"[..])
            .expect("a file");
        builder
            .into_inner()
            .expect("the tar")
            .finish()
            .expect("the gzip");
        let sdk = root.path().join("sdk");
        untar_stripped(&tarred, &sdk).expect("unpacked");
        let clang = std::fs::read(sdk.join("bin").join("clang")).expect("clang");
        assert_eq!(clang, b"clang");
        assert!(!root.path().join("sdk.unpacking").exists());
    }
}
