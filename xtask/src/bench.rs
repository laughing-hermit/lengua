//! The bench's steps in CI's bench mode.
//!
//! - `cargo xtask bench-wasm <folder>`: the benchmark's C, C++ and Rust programs
//!   (benches/programs/<workload>/main.c, main.cpp and main.rs) compiled to WebAssembly modules
//!   of WASI preview 1, once, on linux-x86_64; every runner's bench then runs these same modules
//!   in lengua's host, as it runs lengua's own (`LENGUA_BENCH_WASM` names the folder). It needs
//!   wasi-sdk, which `WASI_SDK` names, and Rust's wasm32-wasip1 target (`cargo xtask fetch
//!   linux-x86_64 wasi`). The folder gets <workload>/c.wasm, cpp.wasm and rust.wasm, and
//!   toolchains.tsv: each version's compiler and flags, which the report gives.
//! - `cargo xtask bench`: the gate (benches/against_rust_and_c.rs), all it prints kept in
//!   bench.log. It fails where the run and KNOWN_BEHIND disagree, and writes
//!   BENCHMARKS-<target>.md from the run's results. Windows builds the native C and C++ with the
//!   image's LLVM clang, whose folder goes last on PATH where the image has it.
//! - `cargo xtask bench-report <report>`: the report, in the job's summary, when the bench wrote
//!   one.
//!
//! The modules' flags are for speed, as a release build's: -O3 and link-time optimization, and
//! every WebAssembly feature lengua's engine has on, but no other (FEATURES in
//! src/wasm/engine.rs): LLVM's generic CPU, whose sign extension, saturating float-to-int
//! conversions, multiple values, reference types, bulk memory and mutable globals are all among
//! them, and SIMD128. Each memory may grow to 4 GiB, as lengua's host holds a module to the
//! maximum its memory declares and wasm-ld declares none unless told. The stack is 1 MiB, Rust's
//! own for WebAssembly, and the names are stripped, as a module that ships has none.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{ci, files};

/// The most a module's memory may grow to: 4 GiB.
const MEMORY: &str = "4294967296";

/// How bench-wasm is called.
const WASM_USAGE: &str = "usage: cargo xtask bench-wasm <output folder>";

/// The first line `program` prints with `--version`; it must run.
fn version(program: &Path) -> Result<String, String> {
    let printed = ci::output_of(Command::new(program).arg("--version"))
        .ok_or_else(|| format!("{} --version didn't run", program.display()))?;
    Ok(printed.lines().next().unwrap_or_default().to_owned())
}

/// `cargo xtask bench-wasm <folder>`.
pub fn wasm(arguments: &[String]) -> Result<u8, String> {
    let [out] = arguments else {
        return Err(WASM_USAGE.to_owned());
    };
    let root = ci::root()?;
    let sdk = std::env::var_os("WASI_SDK")
        .filter(|sdk| !sdk.is_empty())
        .map(PathBuf::from)
        .ok_or("WASI_SDK names wasi-sdk: run cargo xtask fetch linux-x86_64 wasi first")?;
    files::folder(Path::new(out))?;
    let out = std::path::absolute(out).map_err(|error| format!("{out}: {error}"))?;
    let shown = sdk.display();
    let common = vec![
        "--target=wasm32-wasip1".to_owned(),
        format!("--sysroot={shown}/share/wasi-sysroot"),
        "-O3".to_owned(),
        "-flto".to_owned(),
        "-msimd128".to_owned(),
        format!("-Wl,--max-memory={MEMORY}"),
        "-Wl,-z,stack-size=1048576".to_owned(),
        "-Wl,--strip-all".to_owned(),
    ];
    let cflags = common.clone();
    let mut cxxflags = common;
    cxxflags.push("-fno-exceptions".to_owned());
    let mut rustflags: Vec<String> = [
        "--edition",
        "2024",
        "--target",
        "wasm32-wasip1",
        "-C",
        "opt-level=3",
        "-C",
        "codegen-units=1",
        "-C",
        "lto=fat",
        "-C",
        "panic=abort",
        "-C",
        "target-feature=+simd128",
        "-C",
        "strip=symbols",
        "-C",
    ]
    .iter()
    .map(|flag| (*flag).to_owned())
    .collect();
    rustflags.push(format!("link-arg=--max-memory={MEMORY}"));

    let release = std::fs::read_to_string(sdk.join("VERSION"))
        .map(|text| text.lines().next().unwrap_or_default().to_owned())
        .unwrap_or_else(|_| "(version unknown)".to_owned());
    let sdk_version = format!("wasi-sdk {release}");
    let clang = sdk.join("bin").join("clang");
    let clang_pp = sdk.join("bin").join("clang++");
    let rustc = PathBuf::from("rustc");
    let clang_version = version(&clang)?;
    let rustc_version = version(&rustc)?;
    let c = cflags.join(" ");
    let cpp = cxxflags.join(" ");
    let rust = rustflags.join(" ");
    let toolchains = format!(
        "c-wasm\t{sdk_version}, {clang_version}\tclang {c}\n\
         cpp-wasm\t{sdk_version}, {clang_version}\tclang++ {cpp}\n\
         rust-wasm\t{rustc_version}\trustc {rust}\n"
    );
    std::fs::write(out.join("toolchains.tsv"), &toolchains)
        .map_err(|error| format!("toolchains.tsv: {error}"))?;
    print!("{toolchains}");

    let mut programs: Vec<PathBuf> = std::fs::read_dir(root.join("benches").join("programs"))
        .map_err(|error| format!("benches/programs: {error}"))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    programs.sort();
    let mut built = 0;
    for folder in programs {
        // A folder with no C has nothing to build.
        if !folder.join("main.c").is_file() {
            continue;
        }
        let name = folder.file_name().unwrap_or_default();
        let modules = out.join(name);
        files::folder(&modules)?;
        let builds = [
            (&clang, &cflags, "c.wasm", "main.c"),
            (&clang_pp, &cxxflags, "cpp.wasm", "main.cpp"),
            (&rustc, &rustflags, "rust.wasm", "main.rs"),
        ];
        for (compiler, flags, module, source) in builds {
            let mut command = Command::new(compiler);
            command
                .args(flags)
                .arg("-o")
                .arg(modules.join(module))
                .arg(folder.join(source));
            ci::must(&mut command)?;
        }
        for module in files::files(&modules) {
            let size = module.metadata().map_or(0, |metadata| metadata.len());
            println!("{size:>10} {}", module.display());
        }
        built += 1;
    }
    println!(
        "built the C, C++ and Rust modules of {built} workloads in {}",
        out.display()
    );
    if built == 0 {
        return Err("benches/programs holds no workload with C to build".to_owned());
    }
    Ok(0)
}

/// Where Windows's runner images keep LLVM's programs.
const LLVM: &str = r"C:\Program Files\LLVM\bin";

/// `cargo xtask bench`.
pub fn bench(arguments: &[String]) -> Result<u8, String> {
    if !arguments.is_empty() {
        return Err("usage: cargo xtask bench".to_owned());
    }
    let root = ci::root()?;
    let mut command = ci::cargo();
    command
        .current_dir(&root)
        .args(["bench", "--locked", "--bench", "against_rust_and_c"]);
    let llvm = Path::new(LLVM);
    if cfg!(windows) && llvm.is_dir() {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut folders: Vec<PathBuf> = std::env::split_paths(&path).collect();
        folders.push(llvm.to_path_buf());
        let path = std::env::join_paths(folders).map_err(|error| format!("PATH: {error}"))?;
        command.env("PATH", path);
    }
    let status = ci::Logged::start(command, &root.join("bench.log"))?.wait()?;
    Ok(ci::code(status))
}

/// `cargo xtask bench-report <report>`.
pub fn report(arguments: &[String]) -> Result<u8, String> {
    let [report] = arguments else {
        return Err("usage: cargo xtask bench-report <report>".to_owned());
    };
    if let Ok(text) = std::fs::read_to_string(report) {
        ci::append("GITHUB_STEP_SUMMARY", &text)?;
    }
    Ok(0)
}
