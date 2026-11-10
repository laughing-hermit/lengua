//! `cargo xtask solver-wasm`: a solver built from its source as a WebAssembly module of WASI
//! preview 1, which lengua-mcp's solver engine runs. z3 is the one it builds: z3 4.16.0's
//! executable, from the source xtask/tools.tsv pins (`z3-source`, the archive of the commit z3's
//! tag names, by its SHA-256), compiled with the wasi-sdk tools.tsv pins (which `cargo xtask fetch
//! linux-x86_64 wasi` installs and `WASI_SDK` names) through z3's own CMake, Ninja and Python,
//! with the patches in xtask/patches/z3, each of which says why it is there. CI's solver-wasm job
//! builds it once, on linux-x86_64, for every runner.
//!
//! - `cargo xtask solver-wasm z3 <folder>`: `<folder>/z3.wasm`, built, or kept when the folder
//!   holds the module of this same recipe (`<folder>/z3.recipe`, which the build writes); then
//!   checked, and its SHA-256 printed. The build's own files go in target/solver-build/z3: the
//!   source, its build, configure.log and build.log.
//! - `cargo xtask solver-wasm-key z3`: the recipe's SHA-256, which CI's cache keeps the module
//!   under, so that a change to the source, wasi-sdk, a patch, an option or a flag builds anew.
//! - `cargo xtask solver-wasm-check <module>`: a module checked as a build checks its own.
//!
//! The recipe, beside the source and wasi-sdk:
//! - single-threaded (`Z3_SINGLE_THREADED`), with the polling timer (`Z3_POLLING_TIMER`): WASI
//!   preview 1 has no threads, and the limit `try-for` puts on each of lengua's questions is then
//!   a deadline z3 reads the clock for as it works;
//! - static, the executable alone, with z3's own arithmetic (no GMP) and no link-time
//!   optimization, which wasi-sdk 34 says doesn't work with exceptions yet (wasi-sdk#629);
//! - C++ exceptions, which z3 reports its errors and its limits with, in WebAssembly's standard
//!   form (exnref), the one Wasmtime runs. wasi-sdk 34 has two builds of libc++ and libc++abi,
//!   and `-fwasm-exceptions` picks the one with exceptions (lib/wasm32-wasip1/eh) at each compile
//!   and at the link; `-mllvm -wasm-use-legacy-eh=false` makes clang emit the standard
//!   instructions in place of the legacy ones it emits by default, which Wasmtime doesn't run and
//!   won't; and `-lunwind` links the unwinder that a throw goes through (wasi-sdk's
//!   CppExceptions.md);
//! - wasi-libc's emulations of what z3's executable calls and WASI doesn't have: signals (z3's
//!   front ends catch Ctrl-C with `signal` and `raise`) and process clocks (`clock`, in its
//!   DIMACS, optimization and log front ends);
//! - `NO_Z3_DEBUGGER`: a failed assertion ends z3 with an error, as on Emscripten, where z3 would
//!   otherwise start gdb through `system` and `getpid`, which WASI doesn't have;
//! - `CMAKE_HAVE_LIBC_PTHREAD`, the answer z3's CMake gets when it asks CMake's FindThreads for
//!   threads: in libc, with no flag. wasi-libc's single-threaded libc has pthread's stubs, but not
//!   the `pthread_cancel` and `pthread_exit` the find's own test calls, so the find would go on to
//!   try `-pthread`, which on wasm32-wasip1 means shared memory and atomics;
//! - -O3, as z3's release builds, and LLVM's generic CPU with SIMD128, as the bench's builds:
//!   features lengua's engine has (src/wasm/engine.rs);
//! - a 16 MiB stack for z3's recursion, where wasm-ld gives 64 KiB unless told: vibe-lang's
//!   (mizchi/vibe-lang#3883, z3 4.15.8 under Wasmtime), and at the bottom of memory, where wasm-ld
//!   puts it, so that an overflow traps;
//! - a memory that may grow to 4 GiB, all that a 32-bit memory addresses and what lengua lets z3
//!   use (`-memory:4096`): lengua's host holds a module to the most its memory declares, and
//!   wasm-ld declares no most unless told;
//! - the names of z3's functions kept, so that a trap's backtrace names them.
//!
//! The check: the module validates with exactly [`SOLVER_FEATURES`], the features the solver
//! engine turns on; every import is a function of WASI preview 1 (`wasi_snapshot_preview1`); it
//! exports `_start` and its one memory, which may grow to 4 GiB. It reports the module's SHA-256
//! and size, its memory, its imports and the features it uses.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use wasmparser::{ExternalKind, MemoryType, Parser, Payload, TypeRef, Validator, WasmFeatures};

use crate::fetch::Pin;
use crate::{ci, fetch, files};

/// How the solver tasks are called.
const USAGE: &str = "usage: cargo xtask solver-wasm z3 <folder>
       cargo xtask solver-wasm-key z3
       cargo xtask solver-wasm-check <module>";

/// z3's source in tools.tsv: its tool's name, and the target it is built for.
const SOURCE: (&str, &str) = ("z3-source", "wasm32-wasip1");

/// The runner that builds the module, the one tools.tsv pins wasi-sdk for.
const BUILDER: &str = "linux-x86_64";

/// The module's file, in the folder it is built into.
const MODULE: &str = "z3.wasm";

/// The recipe the module was built by, beside it.
const RECIPE: &str = "z3.recipe";

/// The target of z3's CMake that builds its executable.
const SHELL: &str = "shell";

/// z3's patches, in the order they are applied: each file's name and its text.
const PATCHES: [(&str, &str); 3] = [
    (
        "rounding-modes.patch",
        include_str!("../patches/z3/rounding-modes.patch"),
    ),
    ("no-fork.patch", include_str!("../patches/z3/no-fork.patch")),
    (
        "static-code.patch",
        include_str!("../patches/z3/static-code.patch"),
    ),
];

/// A page of WebAssembly's memory: 64 KiB.
const PAGE: u64 = 64 * 1024;

/// The most z3's memory may grow to: 4 GiB.
const MEMORY: u64 = 4 * 1024 * 1024 * 1024;

/// z3's stack: 16 MiB.
const STACK: u64 = 16 * 1024 * 1024;

/// z3's CMake options: a release build of the executable alone, static, single-threaded with the
/// polling timer, with z3's own arithmetic and no link-time optimization, compiled as plain code
/// (xtask/patches/z3/static-code.patch), with no git (the source is an archive), and the answer
/// FindThreads would get wrong.
const OPTIONS: [&str; 15] = [
    "-DCMAKE_BUILD_TYPE=Release",
    "-DCMAKE_POSITION_INDEPENDENT_CODE=OFF",
    "-DZ3_BUILD_EXECUTABLE=ON",
    "-DZ3_BUILD_LIBZ3_SHARED=OFF",
    "-DZ3_BUILD_TEST_EXECUTABLES=OFF",
    "-DZ3_ENABLE_EXAMPLE_TARGETS=OFF",
    "-DZ3_BUILD_PYTHON_BINDINGS=OFF",
    "-DZ3_BUILD_DOCUMENTATION=OFF",
    "-DZ3_SINGLE_THREADED=ON",
    "-DZ3_POLLING_TIMER=ON",
    "-DZ3_USE_LIB_GMP=OFF",
    "-DZ3_LINK_TIME_OPTIMIZATION=OFF",
    "-DZ3_INCLUDE_GIT_HASH=OFF",
    "-DZ3_INCLUDE_GIT_DESCRIBE=OFF",
    "-DCMAKE_HAVE_LIBC_PTHREAD=ON",
];

/// The flags every compile of z3's takes: exceptions in the standard form, SIMD128, wasi-libc's
/// signals and process clocks, and no debugger.
const COMPILE: [&str; 7] = [
    "-fwasm-exceptions",
    "-mllvm",
    "-wasm-use-legacy-eh=false",
    "-msimd128",
    "-D_WASI_EMULATED_SIGNAL",
    "-D_WASI_EMULATED_PROCESS_CLOCKS",
    "-DNO_Z3_DEBUGGER",
];

/// The libraries z3's link takes after its own code: the unwinder, and wasi-libc's signals and
/// process clocks.
const LIBRARIES: [&str; 3] = [
    "-lunwind",
    "-lwasi-emulated-signal",
    "-lwasi-emulated-process-clocks",
];

/// The WebAssembly that a solver's module may import: functions of WASI preview 1.
const WASI: &str = "wasi_snapshot_preview1";

/// The most errors a failed step shows again, together, at its end.
const ERRORS: usize = 400;

/// The WebAssembly features lengua-mcp's solver engine turns on, which a solver's module must
/// validate with: lengua's programs' features (`FEATURES`, src/wasm/engine.rs), and exceptions in
/// their standard form, with the GC types, Wasmtime's switch for the collected heap that it keeps
/// a thrown exception in (`Config::gc_support`; with no such heap a throw fails, "no GC
/// runtime"). The validator needs nothing more for exnref, the type of a caught exception.
pub const SOLVER_FEATURES: WasmFeatures = WasmFeatures::FLOATS
    .union(WasmFeatures::MUTABLE_GLOBAL)
    .union(WasmFeatures::SATURATING_FLOAT_TO_INT)
    .union(WasmFeatures::SIGN_EXTENSION)
    .union(WasmFeatures::REFERENCE_TYPES)
    .union(WasmFeatures::MULTI_VALUE)
    .union(WasmFeatures::BULK_MEMORY)
    .union(WasmFeatures::SIMD)
    .union(WasmFeatures::EXCEPTIONS)
    .union(WasmFeatures::GC_TYPES);

/// Each of [`SOLVER_FEATURES`], by its proposal's name, as the check reports them.
pub const FEATURE_NAMES: [(&str, WasmFeatures); 10] = [
    ("floats", WasmFeatures::FLOATS),
    ("mutable-global", WasmFeatures::MUTABLE_GLOBAL),
    (
        "saturating-float-to-int",
        WasmFeatures::SATURATING_FLOAT_TO_INT,
    ),
    ("sign-extension", WasmFeatures::SIGN_EXTENSION),
    ("reference-types", WasmFeatures::REFERENCE_TYPES),
    ("multi-value", WasmFeatures::MULTI_VALUE),
    ("bulk-memory", WasmFeatures::BULK_MEMORY),
    ("simd", WasmFeatures::SIMD),
    ("exceptions", WasmFeatures::EXCEPTIONS),
    ("gc-types", WasmFeatures::GC_TYPES),
];

/// The solver named `name`, which must be one this task builds.
fn solver(name: &str) -> Result<(), String> {
    if name == "z3" {
        Ok(())
    } else {
        Err(format!(
            "no solver named {name:?} is built as WebAssembly, only z3\n{USAGE}"
        ))
    }
}

/// What tools.tsv pins for `tool` on `target`, which it must pin.
fn pinned(tool: &str, target: &str) -> Result<Pin<'static>, String> {
    fetch::pin(tool, target).ok_or_else(|| format!("tools.tsv pins no {tool} for {target}"))
}

/// The flags z3's link takes: exceptions again, which picks the libraries built with them, the
/// stack, and the most the memory may grow to.
fn link() -> [String; 3] {
    [
        "-fwasm-exceptions".to_owned(),
        format!("-Wl,-z,stack-size={STACK}"),
        format!("-Wl,--max-memory={MEMORY}"),
    ]
}

/// z3's CMake settings, each folder of this computer's left out: [`OPTIONS`], and the flags of
/// the compiles and the link, and the link's libraries.
fn settings() -> Vec<String> {
    let mut settings: Vec<String> = OPTIONS.iter().map(|option| (*option).to_owned()).collect();
    settings.push(format!("-DCMAKE_CXX_FLAGS={}", COMPILE.join(" ")));
    settings.push(format!("-DCMAKE_EXE_LINKER_FLAGS={}", link().join(" ")));
    settings.push(format!(
        "-DCMAKE_CXX_STANDARD_LIBRARIES={}",
        LIBRARIES.join(" ")
    ));
    settings
}

/// What z3's module is made from, a line each: the source and wasi-sdk as tools.tsv pins them,
/// each patch by its name and SHA-256, z3's CMake settings and the target built. Another of any
/// of them makes another module.
pub fn recipe() -> Result<String, String> {
    let (tool, target) = SOURCE;
    let source = pinned(tool, target)?;
    let sdk = pinned("wasi-sdk", BUILDER)?;
    let mut recipe = String::from("solver\tz3\n");
    let _ = writeln!(recipe, "source\t{}\t{}", source.url, source.sha256);
    let _ = writeln!(recipe, "wasi-sdk\t{}\t{}", sdk.url, sdk.sha256);
    for (name, text) in PATCHES {
        let _ = writeln!(recipe, "patch\t{name}\t{}", fetch::sha256(text.as_bytes()));
    }
    for setting in settings() {
        let _ = writeln!(recipe, "cmake\t{setting}");
    }
    let _ = writeln!(recipe, "target\t{SHELL}");
    Ok(recipe)
}

/// wasi-sdk, as `WASI_SDK` names it, which must be the one tools.tsv pins for the builder: `cargo
/// xtask fetch linux-x86_64 wasi` installs it with its pin in pinned.tsv, so that the recipe
/// names the wasi-sdk the module is built with.
fn sdk() -> Result<PathBuf, String> {
    let sdk = std::env::var_os("WASI_SDK")
        .filter(|sdk| !sdk.is_empty())
        .map(PathBuf::from)
        .ok_or("WASI_SDK names wasi-sdk: run cargo xtask fetch linux-x86_64 wasi first")?;
    let pin = pinned("wasi-sdk", BUILDER)?;
    if fetch::kept(&sdk) != fetch::stamp(&[("wasi-sdk", pin)]) {
        return Err(format!(
            "{} isn't the wasi-sdk tools.tsv pins ({}): run cargo xtask fetch linux-x86_64 wasi",
            sdk.display(),
            pin.url
        ));
    }
    Ok(sdk)
}

/// The first line `program` prints with `--version`, or that it didn't run.
fn version(program: &Path) -> String {
    ci::output_of(Command::new(program).arg("--version")).map_or_else(
        || format!("{} didn't run", program.display()),
        |printed| printed.lines().next().unwrap_or_default().to_owned(),
    )
}

/// The lines of a build's log that say what failed: the compiler's and the linker's errors,
/// Ninja's failed steps and CMake's errors, each once, in their order, at most [`ERRORS`].
pub fn errors(log: &str) -> Vec<&str> {
    let mut found: Vec<&str> = Vec::new();
    for line in log.lines() {
        let failed = line.contains("error:")
            || line.starts_with("FAILED:")
            || line.starts_with("CMake Error");
        if failed && !found.contains(&line) {
            found.push(line);
            if found.len() == ERRORS {
                break;
            }
        }
    }
    found
}

/// Runs `command`, what it prints shown and kept in `log`; when it fails, the log's errors are
/// shown again, together, and go in the step's summary.
fn logged(command: Command, log: &Path) -> Result<(), String> {
    let typed = ci::shown(&command);
    let status = ci::Logged::start(command, log)?.wait()?;
    if status.success() {
        return Ok(());
    }
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let found = errors(&text);
    let mut summary = format!(
        "{typed} failed: {status}. The errors in {}, {} of them:\n\n```\n",
        log.display(),
        found.len()
    );
    for line in found {
        summary.push_str(line);
        summary.push('\n');
    }
    summary.push_str("```\n");
    ci::print_and_summarize(&summary)?;
    Err(format!("{typed} failed: {status}"))
}

/// Applies the patch `text`, named `name`, to the source in `source`, as `patch -p1` would. In a
/// folder of a git repository (target/ is in lengua's), git reads a patch's paths as the
/// repository's, skips each file it doesn't find there and still succeeds: so git looks for no
/// repository above `work` (`GIT_CEILING_DIRECTORIES`), and a file it skips fails the step.
fn apply(work: &Path, source: &Path, name: &str, text: &str) -> Result<(), String> {
    let file = work.join(name);
    std::fs::write(&file, text).map_err(|error| format!("{}: {error}", file.display()))?;
    let mut command = Command::new("git");
    command
        .current_dir(source)
        .env("GIT_CEILING_DIRECTORIES", work)
        .args(["apply", "--verbose"])
        .arg(&file);
    let output = command
        .output()
        .map_err(|error| format!("{} didn't start: {error}", ci::shown(&command)))?;
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{said}");
    if !output.status.success() || said.contains("Skipped patch") || !said.contains("Applied") {
        return Err(format!(
            "{name} didn't apply to z3's source: {}",
            output.status
        ));
    }
    Ok(())
}

/// Builds z3's module by the recipe into the file `module`, in target/solver-build/z3, which it
/// empties first.
fn build(module: &Path) -> Result<(), String> {
    let started = Instant::now();
    let sdk = sdk()?;
    let work = ci::root()?.join("target").join("solver-build").join("z3");
    files::remove(&work)?;
    files::folder(&work)?;
    let clang = sdk.join("bin").join("clang");
    for program in [
        clang.as_path(),
        Path::new("cmake"),
        Path::new("ninja"),
        Path::new("python3"),
        Path::new("git"),
    ] {
        println!("{}: {}", program.display(), version(program));
    }

    let (tool, target) = SOURCE;
    let archive = work.join("z3.tar.gz");
    fetch::fetch(&pinned(tool, target)?, &archive)?;
    let source = work.join("source");
    fetch::untar_stripped(&archive, &source)?;
    for (name, text) in PATCHES {
        apply(&work, &source, name, text)?;
    }

    let built = work.join("build");
    let toolchain = sdk.join("share").join("cmake").join("wasi-sdk-p1.cmake");
    let mut configure = Command::new("cmake");
    configure
        .arg("-S")
        .arg(&source)
        .arg("-B")
        .arg(&built)
        .args(["-G", "Ninja", "-Wno-dev"])
        .arg(format!("-DCMAKE_TOOLCHAIN_FILE={}", toolchain.display()))
        .arg(format!("-DWASI_SDK_PREFIX={}", sdk.display()))
        .args(settings());
    logged(configure, &work.join("configure.log"))?;

    let jobs = std::thread::available_parallelism().map_or(2, std::num::NonZeroUsize::get);
    let mut make = Command::new("cmake");
    make.arg("--build")
        .arg(&built)
        .args(["--target", SHELL, "--parallel"])
        .arg(jobs.to_string())
        // Ninja goes on past a step that fails, so that one build shows every error.
        .args(["--", "-k", "0"]);
    logged(make, &work.join("build.log"))?;

    // CMake names the executable z3, without the .wasm wasi-sdk's toolchain file asks for.
    let made = [MODULE, "z3"]
        .iter()
        .map(|file| built.join(file))
        .find(|file| file.is_file())
        .ok_or_else(|| format!("z3's build made no z3 or z3.wasm in {}", built.display()))?;
    files::copy(&made, module)?;
    let seconds = started.elapsed().as_secs();
    println!(
        "z3 built in {} min {} s, {jobs} at a time",
        seconds / 60,
        seconds % 60
    );
    Ok(())
}

/// What the check reads of a module.
#[derive(Debug, Default)]
struct Contents {
    /// Each import: the module it is from, its name and its type.
    imports: Vec<(String, String, TypeRef)>,
    /// Each export: its name and its kind.
    exports: Vec<(String, ExternalKind)>,
    /// Each memory, imported or defined.
    memories: Vec<MemoryType>,
}

/// `error`, as a message.
fn message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// The imports, the exports and the memories of `wasm`.
fn contents(wasm: &[u8]) -> Result<Contents, String> {
    let mut contents = Contents::default();
    for payload in Parser::new(0).parse_all(wasm) {
        match payload.map_err(message)? {
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = import.map_err(message)?;
                    if let TypeRef::Memory(memory) = import.ty {
                        contents.memories.push(memory);
                    }
                    contents.imports.push((
                        import.module.to_owned(),
                        import.name.to_owned(),
                        import.ty,
                    ));
                }
            }
            Payload::MemorySection(reader) => {
                for memory in reader {
                    contents.memories.push(memory.map_err(message)?);
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.map_err(message)?;
                    contents.exports.push((export.name.to_owned(), export.kind));
                }
            }
            _ => {}
        }
    }
    Ok(contents)
}

/// The names of the features of [`FEATURE_NAMES`] that `features` holds, joined.
fn names(features: WasmFeatures) -> String {
    let named: Vec<&str> = FEATURE_NAMES
        .iter()
        .filter(|(_, feature)| features.contains(*feature))
        .map(|(name, _)| *name)
        .collect();
    named.join(", ")
}

/// Checks `wasm`, a solver's module named `name`: it must validate with exactly
/// [`SOLVER_FEATURES`], import functions of WASI preview 1 and nothing else, and export `_start`
/// and its one memory, which may grow to 4 GiB. The report gives its size, its SHA-256, the
/// features it uses (each one it doesn't validate without), its memory and its imports.
pub fn checked(name: &str, wasm: &[u8]) -> Result<String, String> {
    let all = names(SOLVER_FEATURES);
    Validator::new_with_features(SOLVER_FEATURES)
        .validate_all(wasm)
        .map_err(|error| {
            format!("{name} doesn't validate with the solver engine's features ({all}): {error}")
        })?;
    let contents = contents(wasm).map_err(|error| format!("{name}: {error}"))?;
    let wrong: Vec<String> = contents
        .imports
        .iter()
        .filter(|(module, _, ty)| module != WASI || !matches!(ty, TypeRef::Func(_)))
        .map(|(module, field, ty)| format!("{module}.{field} ({ty:?})"))
        .collect();
    if !wrong.is_empty() {
        return Err(format!(
            "{name} imports what isn't a function of WASI preview 1: {}",
            wrong.join(", ")
        ));
    }
    for (export, kind) in [
        ("_start", ExternalKind::Func),
        ("memory", ExternalKind::Memory),
    ] {
        let exported = contents
            .exports
            .iter()
            .any(|(named, its_kind)| named == export && *its_kind == kind);
        if !exported {
            return Err(format!("{name} exports no {export} ({kind:?})"));
        }
    }
    let [memory] = contents.memories.as_slice() else {
        return Err(format!(
            "{name} has {} memories, not one",
            contents.memories.len()
        ));
    };
    if memory.memory64 || memory.shared || memory.maximum != Some(MEMORY / PAGE) {
        return Err(format!(
            "{name}'s memory isn't one of 32 bits, not shared, that may grow to 4 GiB: {memory:?}"
        ));
    }
    let used: Vec<&str> = FEATURE_NAMES
        .iter()
        .filter(|(_, feature)| {
            Validator::new_with_features(SOLVER_FEATURES.difference(*feature))
                .validate_all(wasm)
                .is_err()
        })
        .map(|(named, _)| *named)
        .collect();
    let mut imported: Vec<&str> = contents
        .imports
        .iter()
        .map(|(_, field, _)| field.as_str())
        .collect();
    imported.sort_unstable();
    let mut report = format!(
        "{name}: {} bytes, SHA-256 {}\n",
        wasm.len(),
        fetch::sha256(wasm)
    );
    let _ = writeln!(
        report,
        "- validates with the solver engine's features: {all}"
    );
    let uses = if used.is_empty() {
        "none of them".to_owned()
    } else {
        used.join(", ")
    };
    let _ = writeln!(report, "- uses: {uses}");
    let _ = writeln!(
        report,
        "- memory: {} pages at first, at most {} (4 GiB)",
        memory.initial,
        MEMORY / PAGE
    );
    let _ = writeln!(
        report,
        "- imports {} functions of {WASI}, and nothing else: {}",
        imported.len(),
        imported.join(", ")
    );
    Ok(report)
}

/// `cargo xtask solver-wasm <solver> <folder>`.
pub fn wasm(arguments: &[String]) -> Result<u8, String> {
    let [name, folder] = arguments else {
        return Err(USAGE.to_owned());
    };
    solver(name)?;
    files::folder(Path::new(folder))?;
    let folder = std::path::absolute(folder).map_err(|error| format!("{folder}: {error}"))?;
    let recipe = recipe()?;
    print!("z3's recipe:\n{recipe}");
    let module = folder.join(MODULE);
    let written = folder.join(RECIPE);
    let kept =
        module.is_file() && std::fs::read_to_string(&written).is_ok_and(|text| text == recipe);
    if kept {
        println!("{} was built by this recipe: kept", module.display());
    } else {
        files::remove(&written)?;
        files::remove(&module)?;
        build(&module)?;
        std::fs::write(&written, &recipe)
            .map_err(|error| format!("{}: {error}", written.display()))?;
    }
    let wasm = std::fs::read(&module).map_err(|error| format!("{}: {error}", module.display()))?;
    ci::print_and_summarize(&checked(MODULE, &wasm)?)?;
    Ok(0)
}

/// `cargo xtask solver-wasm-key <solver>`.
pub fn key(arguments: &[String]) -> Result<u8, String> {
    let [name] = arguments else {
        return Err(USAGE.to_owned());
    };
    solver(name)?;
    println!("{}", fetch::sha256(recipe()?.as_bytes()));
    Ok(0)
}

/// `cargo xtask solver-wasm-check <module>`.
pub fn check(arguments: &[String]) -> Result<u8, String> {
    let [module] = arguments else {
        return Err(USAGE.to_owned());
    };
    let wasm = std::fs::read(module).map_err(|error| format!("{module}: {error}"))?;
    let name = Path::new(module).file_name().map_or_else(
        || module.clone(),
        |name| name.to_string_lossy().into_owned(),
    );
    ci::print_and_summarize(&checked(&name, &wasm)?)?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `value` in LEB128, as WebAssembly's binary writes a number.
    fn leb(mut value: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        loop {
            let low = u8::try_from(value & 0x7f).expect("seven bits");
            value >>= 7;
            if value == 0 {
                bytes.push(low);
                return bytes;
            }
            bytes.push(low | 0x80);
        }
    }

    /// A name, as the binary writes one: its length, then its bytes.
    fn name(text: &str) -> Vec<u8> {
        let mut bytes = leb(u64::try_from(text.len()).expect("a length"));
        bytes.extend_from_slice(text.as_bytes());
        bytes
    }

    /// A section: its id, the length of its contents, and its contents.
    fn section(id: u8, contents: &[u8]) -> Vec<u8> {
        let mut bytes = vec![id];
        bytes.extend(leb(u64::try_from(contents.len()).expect("a length")));
        bytes.extend_from_slice(contents);
        bytes
    }

    /// A module that imports `field` from `from`, a function of one i32, and defines and exports
    /// `_start` and a memory of one page that may grow to `maximum` pages.
    fn module(from: &str, field: &str, maximum: u64) -> Vec<u8> {
        let mut wasm = b"\0asm\x01\0\0\0".to_vec();
        // Two types: a function of one i32, and a function of nothing.
        wasm.extend(section(1, &[2, 0x60, 1, 0x7f, 0, 0x60, 0, 0]));
        let mut imports = vec![1];
        imports.extend(name(from));
        imports.extend(name(field));
        imports.extend([0, 0]);
        wasm.extend(section(2, &imports));
        // One function, of the second type.
        wasm.extend(section(3, &[1, 1]));
        // One memory, with a most: one page at first.
        let mut memory = vec![1, 1, 1];
        memory.extend(leb(maximum));
        wasm.extend(section(5, &memory));
        let mut exports = vec![2];
        exports.extend(name("memory"));
        exports.extend([2, 0]);
        exports.extend(name("_start"));
        exports.extend([0, 1]);
        wasm.extend(section(7, &exports));
        // The function's body: no locals, and its end.
        wasm.extend(section(10, &[1, 2, 0, 0x0b]));
        wasm
    }

    /// A module of WASI preview 1's functions that exports `_start` and a memory that may grow
    /// to 4 GiB passes, its SHA-256 and imports reported; one that imports from elsewhere, or
    /// whose memory stops sooner, doesn't, nor does what isn't WebAssembly.
    #[test]
    fn the_check_takes_a_wasi_command_and_nothing_else() {
        let good = module(WASI, "proc_exit", 65536);
        let report = checked("good.wasm", &good).expect("a WASI command");
        assert!(report.contains(&fetch::sha256(&good)), "{report}");
        assert!(
            report.contains("1 functions of wasi_snapshot_preview1"),
            "{report}"
        );
        assert!(report.contains("proc_exit"), "{report}");
        assert!(report.contains("uses: none of them"), "{report}");
        let other = checked("env.wasm", &module("env", "proc_exit", 65536)).expect_err("env");
        assert!(other.contains("env.proc_exit"), "{other}");
        let small = checked("small.wasm", &module(WASI, "proc_exit", 16)).expect_err("16 pages");
        assert!(small.contains("4 GiB"), "{small}");
        assert!(checked("text.wasm", b"(module)").is_err());
    }

    /// The solver engine's features are lengua's programs' (src/wasm/engine.rs's `FEATURES`),
    /// exceptions and the GC types, and the check names each of them.
    #[test]
    fn the_solver_features_are_the_programs_and_exceptions() {
        let named = FEATURE_NAMES
            .iter()
            .fold(WasmFeatures::empty(), |all, (_, feature)| {
                all.union(*feature)
            });
        assert_eq!(named, SOLVER_FEATURES);
        let engine = include_str!("../../src/wasm/engine.rs");
        let start = engine
            .find("pub const FEATURES: WasmFeatures =")
            .expect("the programs' features");
        let end = start + engine[start..].find(';').expect("their end");
        let mut programs = WasmFeatures::empty();
        for part in engine[start..end].split("WasmFeatures::").skip(1) {
            let flag: String = part
                .chars()
                .take_while(|character| character.is_ascii_uppercase() || *character == '_')
                .collect();
            programs |= WasmFeatures::from_name(&flag).expect("a feature");
        }
        assert_eq!(
            programs
                .union(WasmFeatures::EXCEPTIONS)
                .union(WasmFeatures::GC_TYPES),
            SOLVER_FEATURES
        );
        assert!(!programs.contains(WasmFeatures::EXCEPTIONS));
    }

    /// The recipe names the source, by the commit of z3's tag, and wasi-sdk as tools.tsv pins
    /// them, each patch by its SHA-256, and the options and flags; it is the same each time. Each
    /// patch says why it is there before its first file.
    #[test]
    fn the_recipe_names_what_the_module_is_made_from() {
        let recipe = recipe().expect("a recipe");
        assert_eq!(recipe, super::recipe().expect("a recipe"));
        let source = fetch::pin("z3-source", "wasm32-wasip1").expect("z3's source");
        let commit = source
            .url
            .strip_prefix("https://github.com/Z3Prover/z3/archive/")
            .and_then(|rest| rest.strip_suffix(".tar.gz"))
            .expect("the archive of a commit");
        assert!(
            commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{commit}"
        );
        for needed in [
            source.url,
            source.sha256,
            "wasi-sdk-34",
            "-fwasm-exceptions -mllvm -wasm-use-legacy-eh=false",
            "-lunwind",
            "-Wl,-z,stack-size=16777216",
            "-Wl,--max-memory=4294967296",
            "-DZ3_SINGLE_THREADED=ON",
            "-DZ3_POLLING_TIMER=ON",
            "target\tshell",
        ] {
            assert!(recipe.contains(needed), "{needed} isn't in\n{recipe}");
        }
        for (name, text) in PATCHES {
            let line = format!("patch\t{name}\t{}", fetch::sha256(text.as_bytes()));
            assert!(recipe.contains(&line), "{line} isn't in\n{recipe}");
            let (why, _) = text.split_once("\ndiff --git ").expect("a patch");
            assert!(why.len() > 200, "{name} doesn't say why it is there");
        }
    }

    /// A build's log gives its compiler's and linker's errors, Ninja's failed steps and CMake's
    /// errors, each once; the solvers built are z3 alone.
    #[test]
    fn a_build_s_errors_are_picked_from_its_log() {
        let log = "[1/9] Building CXX object a.o\n\
                   FAILED: a.o\n\
                   src/a.cpp:3:1: error: unknown type name 'x'\n\
                   src/a.cpp:3:1: error: unknown type name 'x'\n\
                   1 warning generated.\n\
                   wasm-ld: error: undefined symbol: pthread_atfork\n\
                   CMake Error at CMakeLists.txt:1 (project):\n";
        assert_eq!(
            errors(log),
            [
                "FAILED: a.o",
                "src/a.cpp:3:1: error: unknown type name 'x'",
                "wasm-ld: error: undefined symbol: pthread_atfork",
                "CMake Error at CMakeLists.txt:1 (project):",
            ]
        );
        assert!(solver("z3").is_ok());
        assert!(solver("cvc5").is_err());
    }
}
