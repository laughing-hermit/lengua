//! `cargo xtask outside-wasm <folder>`: the programs of tests/outside, each compiled to a
//! WebAssembly module of WASI preview 1 by its language's own compiler, once, on linux-x86_64,
//! as the bench's are (`cargo xtask bench-wasm`); every runner's suite then checks and runs
//! these same modules as outside modules, under the ideas beside their sources
//! (tests/outside.rs, which `LENGUA_OUTSIDE_WASM` names the folder for). It needs wasi-sdk,
//! which `WASI_SDK` names, and Rust's wasm32-wasip1 target (`cargo xtask fetch linux-x86_64
//! wasi`). The folder gets `<program>.wasm` for each folder of tests/outside: from its main.rs,
//! by rustc, or from its main.c, by wasi-sdk's clang.
//!
//! It builds the libraries too, the modules a lengua program calls (`- Calls:`, tests/calls.rs):
//! a folder of tests/outside with a lib.rs or a lib.c gives `<folder>.wasm`, and a Rust or C
//! file beside an example's main.wat, such as examples/checksum-rust/crc32.rs, gives
//! `<example>.<file>.wasm`. A library has no `_start` and imports nothing: Rust's is a `cdylib`
//! of a `no_std` crate, and C's is linked with no standard library and no entry.
//!
//! The flags are the ones the compile skill's references/outside.md and references/calls.md give
//! an agent: a release build, no names, and a memory with a stated maximum, 16 MiB here, since
//! lengua holds a program to the memory its module declares and takes no module that declares
//! none.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{ci, files};

/// The most each module's memory may grow to: 16 MiB.
const MEMORY: &str = "16777216";

/// How outside-wasm is called.
const USAGE: &str = "usage: cargo xtask outside-wasm <output folder>";

/// The command that compiles `source`, a Rust or C file, into `module`: a program with a main
/// function, or, for `library`, a module with no `_start` whose exported functions another
/// module calls.
fn compiler(sdk: &Path, source: &Path, module: &Path, library: bool) -> Command {
    let rust = source
        .extension()
        .is_some_and(|extension| extension == "rs");
    let mut command = if rust {
        let mut rustc = Command::new("rustc");
        rustc
            .args(["--edition", "2024", "--target", "wasm32-wasip1"])
            .args([
                "-C",
                "opt-level=2",
                "-C",
                "panic=abort",
                "-C",
                "strip=symbols",
            ])
            .arg("-C")
            .arg(format!("link-arg=--max-memory={MEMORY}"));
        if library {
            rustc.args(["--crate-type", "cdylib"]);
        }
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
        if library {
            clang.args(["-nostdlib", "-Wl,--no-entry"]);
        }
        clang
    };
    command.arg("-o").arg(module).arg(source);
    command
}

/// The folders in `folder`, in order.
fn folders(folder: &Path) -> Result<Vec<PathBuf>, String> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(folder)
        .map_err(|error| format!("{}: {error}", folder.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    found.sort();
    Ok(found)
}

/// Compiles `source` into `module`, and says how many bytes it came to.
fn build(sdk: &Path, source: &Path, module: &Path, library: bool) -> Result<(), String> {
    ci::must(&mut compiler(sdk, source, module, library))?;
    let size = module.metadata().map_or(0, |metadata| metadata.len());
    println!("{size:>10} {}", module.display());
    Ok(())
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
    let mut built = 0;
    // tests/outside: a program from main.rs or main.c, a library from lib.rs or lib.c.
    for folder in folders(&root.join("tests").join("outside"))? {
        let found = [
            ("main.rs", false),
            ("main.c", false),
            ("lib.rs", true),
            ("lib.c", true),
        ]
        .into_iter()
        .map(|(name, library)| (folder.join(name), library))
        .find(|(source, _)| source.is_file());
        let Some((source, library)) = found else {
            continue;
        };
        let name = folder.file_name().unwrap_or_default().to_string_lossy();
        build(&sdk, &source, &out.join(format!("{name}.wasm")), library)?;
        built += 1;
    }
    // examples: a library from each Rust or C file beside an example's main.wat.
    for folder in folders(&root.join("examples"))? {
        let mut sources: Vec<PathBuf> = std::fs::read_dir(&folder)
            .map_err(|error| format!("{}: {error}", folder.display()))?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|extension| extension == "rs" || extension == "c")
            })
            .collect();
        sources.sort();
        let name = folder.file_name().unwrap_or_default().to_string_lossy();
        for source in sources {
            let stem = source.file_stem().unwrap_or_default().to_string_lossy();
            build(
                &sdk,
                &source,
                &out.join(format!("{name}.{stem}.wasm")),
                true,
            )?;
            built += 1;
        }
    }
    println!("built {built} outside modules in {}", out.display());
    if built == 0 {
        return Err("tests/outside holds no program to build".to_owned());
    }
    Ok(0)
}
