//! `cargo xtask outside-wasm <folder>`: the programs of tests/outside, each compiled to a
//! WebAssembly module of WASI preview 1 by its language's own compiler, once, on linux-x86_64,
//! as the bench's are (`cargo xtask bench-wasm`); every runner's suite then checks and runs
//! these same modules as outside modules, under the ideas beside their sources
//! (tests/outside.rs, which `LENGUA_OUTSIDE_WASM` names the folder for). It needs wasi-sdk,
//! which `WASI_SDK` names, and Rust's wasm32-wasip1 target (`cargo xtask fetch linux-x86_64
//! wasi`). The folder gets `<program>.wasm` for each folder of tests/outside: from its main.rs,
//! by rustc, or from its main.c, by wasi-sdk's clang.
//!
//! The flags are the ones the compile skill's references/outside.md gives an agent: a release
//! build, no names, and a memory with a stated maximum, 16 MiB here, since lengua holds a
//! program to the memory its module declares and takes no module that declares none.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{ci, files};

/// The most each module's memory may grow to: 16 MiB.
const MEMORY: &str = "16777216";

/// How outside-wasm is called.
const USAGE: &str = "usage: cargo xtask outside-wasm <output folder>";

/// The command that compiles `source`, a program's main.rs or main.c, into `module`.
fn compiler(sdk: &Path, source: &Path, module: &Path) -> Command {
    let rust = source.extension().is_some_and(|extension| extension == "rs");
    let mut command = if rust {
        let mut rustc = Command::new("rustc");
        rustc
            .args(["--edition", "2024", "--target", "wasm32-wasip1"])
            .args(["-C", "opt-level=2", "-C", "panic=abort", "-C", "strip=symbols"])
            .arg("-C")
            .arg(format!("link-arg=--max-memory={MEMORY}"));
        rustc
    } else {
        let mut clang = Command::new(sdk.join("bin").join("clang"));
        clang
            .arg("--target=wasm32-wasip1")
            .arg(format!(
                "--sysroot={}",
                sdk.join("share").join("wasi-sysroot").display()
            ))
            .arg("-O2")
            .arg(format!("-Wl,--max-memory={MEMORY}"))
            .arg("-Wl,--strip-all");
        clang
    };
    command.arg("-o").arg(module).arg(source);
    command
}

/// `cargo xtask outside-wasm <folder>`.
pub fn wasm(arguments: &[String]) -> Result<u8, String> {
    let [out] = arguments else {
        return Err(USAGE.to_owned());
    };
    let root = ci::root()?;
    let sdk = std::env::var_os("WASI_SDK")
        .filter(|sdk| !sdk.is_empty())
        .map(PathBuf::from)
        .ok_or("WASI_SDK names wasi-sdk: run cargo xtask fetch linux-x86_64 wasi first")?;
    files::folder(Path::new(out))?;
    let out = std::path::absolute(out).map_err(|error| format!("{out}: {error}"))?;
    let sources = root.join("tests").join("outside");
    let mut programs: Vec<PathBuf> = std::fs::read_dir(&sources)
        .map_err(|error| format!("{}: {error}", sources.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    programs.sort();
    let mut built = 0;
    for folder in programs {
        let Some(source) = ["main.rs", "main.c"]
            .iter()
            .map(|name| folder.join(name))
            .find(|source| source.is_file())
        else {
            continue;
        };
        let name = folder.file_name().unwrap_or_default().to_string_lossy();
        let module = out.join(format!("{name}.wasm"));
        ci::must(&mut compiler(&sdk, &source, &module))?;
        let size = module.metadata().map_or(0, |metadata| metadata.len());
        println!("{size:>10} {}", module.display());
        built += 1;
    }
    println!("built {built} outside modules in {}", out.display());
    if built == 0 {
        return Err("tests/outside holds no program to build".to_owned());
    }
    Ok(0)
}
