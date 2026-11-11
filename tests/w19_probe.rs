//! Temporary, for W19's measurements: what a shell run costs with lengua-run, built with no
//! compiler, against lengua-mcp's own run and the smallest programs, and what each of its parts
//! buys, measured by `lengua-mcp measure` on this runner.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{Program, Protocol, Session};

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
    /// The lengua-mcp that measures it, which runs it with the lengua-run beside it, if any.
    measurer: PathBuf,
    /// What runs.
    target: Target,
}

/// Runs `case` once through `lengua-mcp measure`: what it printed.
fn measured(case: &Case, data: &Path, stages: Option<&Path>) -> serde_json::Value {
    let mut command = Command::new(&case.measurer);
    command.arg("--data-dir").arg(data).arg("measure");
    if let Some(file) = stages {
        command.arg("--stderr").arg(file);
        command.env("LENGUA_STAGES", "maps");
    }
    command.env("LENGUA_VARIANT", case.variant);
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

/// Copies the programs `names` from `from` into the new folder `to`.
fn kit(from: &[&PathBuf], to: &Path) {
    std::fs::create_dir_all(to).expect("a folder");
    for program in from {
        let name = program.file_name().expect("a name");
        std::fs::copy(program, to.join(name)).expect("copied");
    }
}

#[tokio::test]
#[ignore = "temporary: W19's measurements"]
async fn w19_probe() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let exe = std::env::consts::EXE_SUFFIX;
    let server = PathBuf::from(env!("CARGO_BIN_EXE_lengua-mcp"));
    let spawner = PathBuf::from(env!("CARGO_BIN_EXE_lengua-spawn"));
    let whole = server.with_file_name(format!("lengua-run{exe}"));

    // lengua-run built alone, so that it has no compiler.
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let built = Command::new(cargo)
        .current_dir(root)
        .args(["build", "--release", "--locked", "-p", "lengua-run"])
        .args(["--target-dir", "target/lean"])
        .status()
        .expect("cargo starts");
    assert!(built.success(), "lengua-run didn't build alone");
    let lean = root
        .join("target/lean/release")
        .join(format!("lengua-run{exe}"));
    for file in [&server, &spawner, &lean, &whole] {
        let size = std::fs::metadata(file).map_or(0, |file| file.len());
        println!("SIZE {} {size}", file.display());
    }

    let folder = tempfile::tempdir().expect("a folder");
    let with_lean = folder.path().join("lean");
    let with_whole = folder.path().join("whole");
    let without = folder.path().join("without");
    kit(&[&server, &spawner, &lean], &with_lean);
    kit(&[&server, &spawner, &whole], &with_whole);
    kit(&[&server, &spawner], &without);
    let measurer = |kit: &Path| kit.join(format!("lengua-mcp{exe}"));

    // add, checked to done.
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    program.write("idea.md", IDEA);
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    program.write("main.wat", CODE);
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    println!("SHELL {}", done["shell"]);
    let data = session.data_dir.path().to_path_buf();

    let tiny = lengua::wasm::text::build(TINY).expect("it builds").wasm;
    let tiny_wasm = folder.path().join("tiny.wasm");
    std::fs::write(&tiny_wasm, &tiny).expect("written");

    let words = |list: &[&str]| list.iter().map(|word| (*word).to_owned()).collect::<Vec<_>>();
    let module = |name, variant, kit: &Path| Case {
        name,
        variant,
        measurer: measurer(kit),
        target: Target::Module(tiny_wasm.clone()),
    };
    let run = |name, variant, kit: &Path| Case {
        name,
        variant,
        measurer: measurer(kit),
        target: Target::Folder(program.path().to_path_buf(), words(&["2", "3"])),
    };
    let native = |name, program: &Path, args: &[&str]| Case {
        name,
        variant: "",
        measurer: measurer(&without),
        target: Target::Native(program.to_path_buf(), words(args)),
    };
    let cases = [
        native("spawn (a small Rust program)", &spawner, &[]),
        native("lengua-mcp --version", &server, &["--version"]),
        native("lengua-run --version, whole workspace", &whole, &["--version"]),
        native("lengua-run --version, alone", &lean, &["--version"]),
        run("run: lengua-mcp run", "", &without),
        run("run: lengua-run, whole workspace", "", &with_whole),
        run("run: lengua-run", "", &with_lean),
        run("run: lengua-run, a thread", "thread", &with_lean),
        run("run: lengua-run, every function", "linkall", &with_lean),
        run("run: lengua-run, the sha2 crate", "sha2", &with_lean),
        module("module: lengua-mcp", "", &without),
        module("module: lengua-run", "", &with_lean),
        module("module: lengua-run, a thread", "thread", &with_lean),
        module("module: lengua-run, every function", "linkall", &with_lean),
    ];
    let mut results: Vec<[Vec<u64>; 3]> = cases.iter().map(|_| Default::default()).collect();
    let rounds = 160;
    for round in 0..rounds + 3 {
        for (at, case) in cases.iter().enumerate() {
            let measures = measured(case, &data, None);
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
            let _ = measured(case, &data, Some(&stages));
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
