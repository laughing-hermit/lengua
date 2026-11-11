//! Temporary, for W19's measurements: where a shell run's time and memory go, and what each
//! candidate change would buy, measured by `lengua-mcp measure` on this runner.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{Program, Protocol, Session};
use lengua::wasm::engine::{self, Setup};

/// The add program's idea.
const IDEA: &str = include_str!("fixtures/add/idea.md");

/// Its code.
const CODE: &str = include_str!("fixtures/add/main.wat");

/// A module that exits at once.
const TINY: &str = "(module\n  (import \"wasi_snapshot_preview1\" \"proc_exit\" (func $exit \
                    (param i32)))\n  (memory (export \"memory\") 1)\n  (func (export \
                    \"_start\")\n    (call $exit (i32.const 0))))\n";

/// What a case runs.
enum Target {
    /// An executable, with its arguments.
    Native(PathBuf, Vec<String>),
    /// A module, in lengua's host.
    Module(PathBuf),
    /// A checked program's folder, with its arguments.
    Folder(PathBuf, Vec<String>),
}

/// One thing measured.
struct Case {
    /// Its name in the table.
    name: &'static str,
    /// `LENGUA_VARIANT` for it.
    variant: &'static str,
    /// What runs.
    target: Target,
}

/// Where the measurements keep their files.
struct Places {
    /// The data folder.
    data: PathBuf,
    /// The precompiled modules' folder.
    aot: PathBuf,
}

/// Runs `case` once through `lengua-mcp measure`: what it printed.
fn measured(case: &Case, places: &Places, stages: Option<&Path>) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"));
    command.arg("--data-dir").arg(&places.data).arg("measure");
    if let Some(file) = stages {
        command.arg("--stderr").arg(file);
        command.env("LENGUA_STAGES", "maps");
    }
    command
        .env("LENGUA_VARIANT", case.variant)
        .env("LENGUA_AOT_DIR", &places.aot);
    let args = match &case.target {
        Target::Native(program, args) | Target::Folder(program, args) => {
            command.arg(program);
            args.clone()
        }
        Target::Module(module) => {
            command.arg(module);
            Vec::new()
        }
    };
    command.arg("--").args(args);
    let output = command.output().expect("lengua-mcp starts");
    assert!(
        output.status.success(),
        "{}: {}",
        case.name,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("measure's JSON")
}

/// The median of `values`.
fn median(values: &mut [u64]) -> u64 {
    values.sort_unstable();
    values.get(values.len() / 2).copied().unwrap_or(0)
}

#[tokio::test]
#[ignore = "temporary: W19's measurements"]
async fn w19_probe() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let server = PathBuf::from(env!("CARGO_BIN_EXE_lengua-mcp"));
    let spawner = PathBuf::from(env!("CARGO_BIN_EXE_lengua-spawn"));

    // The probe, built alone, so that its Wasmtime has no compiler.
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let built = Command::new(cargo)
        .current_dir(root)
        .args(["build", "--release", "--locked", "-p", "lean-probe"])
        .args(["--target-dir", "target/lean"])
        .status()
        .expect("cargo starts");
    assert!(built.success(), "the probe didn't build");
    let probe = root
        .join("target/lean/release")
        .join(format!("lean-probe{}", std::env::consts::EXE_SUFFIX));
    for file in [&server, &spawner, &probe] {
        let size = std::fs::metadata(file).expect("the file").len();
        println!("SIZE {} {size}", file.display());
    }

    // add, checked to done.
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    program.write("idea.md", IDEA);
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    program.write("main.wat", CODE);
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");

    let folder = tempfile::tempdir().expect("a folder");
    let places = Places {
        data: session.data_dir.path().to_path_buf(),
        aot: folder.path().join("aot"),
    };
    let tiny = lengua::wasm::text::build(TINY).expect("it builds").wasm;
    let tiny_wasm = folder.path().join("tiny.wasm");
    std::fs::write(&tiny_wasm, &tiny).expect("written");
    let config = engine::config(Setup::Shell, None).expect("the configuration");
    let compiler = wasmtime::Engine::new(&config).expect("the engine");
    let precompiled = compiler.precompile_module(&tiny).expect("it compiles");
    println!("SIZE tiny.wasm {}", tiny.len());
    println!("SIZE tiny.cwasm {}", precompiled.len());
    let add_wasm = lengua::wasm::text::build(CODE).expect("it builds").wasm;
    println!("SIZE add.wasm {}", add_wasm.len());
    println!(
        "SIZE add.cwasm {}",
        compiler
            .precompile_module(&add_wasm)
            .expect("it compiles")
            .len()
    );
    let tiny_cwasm = folder.path().join("tiny.cwasm");
    std::fs::write(&tiny_cwasm, &precompiled).expect("written");

    let words = |list: &[&str]| list.iter().map(|word| (*word).to_owned()).collect::<Vec<_>>();
    let cwasm_path = tiny_cwasm.to_string_lossy().into_owned();
    let cwasm = cwasm_path.as_str();
    let module = |name, variant| Case {
        name,
        variant,
        target: Target::Module(tiny_wasm.clone()),
    };
    let run = |name, variant| Case {
        name,
        variant,
        target: Target::Folder(program.path().to_path_buf(), words(&["2", "3"])),
    };
    let native = |name, program: &Path, args: &[&str]| Case {
        name,
        variant: "",
        target: Target::Native(program.to_path_buf(), words(args)),
    };
    let cases = [
        native("spawn (a small Rust program)", &spawner, &[]),
        native("lengua-mcp --run-module ''", &server, &["--run-module", ""]),
        native("lengua-mcp --version", &server, &["--version"]),
        native("probe: engine only", &probe, &[cwasm, "none"]),
        native("probe: bytes", &probe, &[cwasm, "bytes"]),
        native("probe: file", &probe, &[cwasm, "file"]),
        module("module: today", ""),
        module("module: main thread", "main"),
        module("module: small stack", "smallstack"),
        module("module: no cow", "nocow"),
        module("module: aot", "aot"),
        module("module: aot file", "aotfile"),
        module("module: aot, main thread", "aot,main"),
        module("module: aot file, main thread", "aotfile,main"),
        module("module: aot, main, no cow", "aot,main,nocow"),
        run("run: today", ""),
        run("run: main thread", "main"),
        run("run: aot", "aot"),
        run("run: aot, main thread", "aot,main"),
        run("run: aot file, main thread", "aotfile,main"),
    ];
    let mut results: Vec<[Vec<u64>; 3]> = cases.iter().map(|_| Default::default()).collect();
    let rounds = 120;
    for round in 0..rounds + 3 {
        for (at, case) in cases.iter().enumerate() {
            let measures = measured(case, &places, None);
            if round < 3 {
                println!("FIRST {} exit={}", case.name, measures["exit"]);
                continue;
            }
            let number = |name: &str| measures[name].as_u64().unwrap_or(0);
            results[at][0].push(number("user_ns") + number("system_ns"));
            results[at][1].push(number("wall_ns"));
            results[at][2].push(number("peak_bytes"));
        }
    }
    println!("TABLE case | cpu µs | wall µs | peak KiB ({rounds} runs each, medians)");
    for (case, result) in cases.iter().zip(results.iter_mut()) {
        let [cpu, wall, peak] = result;
        println!(
            "TABLE {} | {} | {} | {}",
            case.name,
            median(cpu) / 1000,
            median(wall) / 1000,
            median(peak) / 1024
        );
    }
    let stages = folder.path().join("stages.txt");
    for case in &cases {
        if matches!(case.target, Target::Native(..)) {
            continue;
        }
        for _ in 0..3 {
            let _ = measured(case, &places, Some(&stages));
            let text = std::fs::read_to_string(&stages).unwrap_or_default();
            println!("TRACE {}", case.name);
            for line in text.lines().filter(|line| line.starts_with("STAGE")) {
                println!("  {line}");
            }
        }
        let text = std::fs::read_to_string(&stages).unwrap_or_default();
        for line in text.lines().filter(|line| line.starts_with("MAP")) {
            println!("  {line}");
        }
    }
    session.service.cancel().await.expect("close the session");
}
