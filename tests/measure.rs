//! `lengua-mcp measure` counts a program's peak memory as the program's own, on every system.
//! Linux begins a process's count of its peak at what the process it was started from held, so
//! there the program starts from lengua-spawn, a small process beside lengua-mcp, and not from
//! lengua-mcp, whose megabytes a small program's count would begin at. macOS and Windows start a
//! process with a count of its own, and there `measure` starts the program straight from
//! lengua-mcp. The case is the same on each system: a small program holds less than lengua-mcp.

use std::env::consts::EXE_SUFFIX;
use std::path::Path;
use std::process::Command;

use lengua::measure::{REFUSED, SPAWNER};

/// What `lengua-mcp measure` prints for `program` run with `args`.
fn measured(program: &str, args: &[&str]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .arg("measure")
        .arg(program)
        .arg("--")
        .args(args)
        .output()
        .expect("lengua-mcp starts");
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "measure failed: {said}");
    serde_json::from_slice(&output.stdout).expect("measure's JSON")
}

/// A small program's peak is its own: below lengua-mcp's, which a program started straight from
/// lengua-mcp would begin its count at on Linux, and which `measure` would then leave out. The
/// small program is lengua-spawn itself, given no program to run: on every system it says so and
/// exits with 125, having done next to nothing, and every system has it, as Cargo builds it
/// beside lengua-mcp, where `measure` looks for it on Linux.
#[test]
fn a_small_programs_peak_is_its_own() {
    let server = env!("CARGO_BIN_EXE_lengua-mcp");
    let spawner = env!("CARGO_BIN_EXE_lengua-spawn");
    let name = format!("{SPAWNER}{EXE_SUFFIX}");
    assert_eq!(Path::new(spawner), Path::new(server).with_file_name(name));
    let small = measured(spawner, &[]);
    assert_eq!(small["exit"], REFUSED, "{small}");
    let large = measured(server, &["--version"]);
    assert_eq!(large["exit"], 0, "{large}");
    let peak = |measures: &serde_json::Value| measures["peak_bytes"].as_u64();
    let (Some(small_peak), Some(large_peak)) = (peak(&small), peak(&large)) else {
        panic!("a peak for each: lengua-spawn's {small}, lengua-mcp --version's {large}");
    };
    assert!(
        small_peak < large_peak,
        "lengua-spawn held {small_peak} bytes at most, lengua-mcp --version {large_peak}"
    );
}

/// A module of another toolchain measured with an idea runs as a checked program does.
/// `measure` keeps a folder for it in the data folder, with the idea, the module as main.wasm
/// and a lock the user's key signs, which records the module's hash; the measured run reads
/// and checks all three, so the folder runs by `lengua-mcp run --module` too, and no longer
/// once its module is changed. Without an idea the module runs with no lock, as before.
#[test]
fn a_module_measured_with_an_idea_runs_as_a_checked_program_does() {
    let server = env!("CARGO_BIN_EXE_lengua-mcp");
    let root = tempfile::tempdir().expect("a folder");
    let (data, work) = (root.path().join("data"), root.path().join("work"));
    std::fs::create_dir_all(&work).expect("a working folder");
    let idea = root.path().join("idea.md");
    std::fs::write(&idea, include_str!("fixtures/add/idea.md")).expect("the idea");
    let wasm = lengua::wasm::text::build(include_str!("fixtures/add/main.wat"))
        .expect("it builds")
        .wasm;
    let module = root.path().join("add.wasm");
    std::fs::write(&module, &wasm).expect("the module");
    let out = root.path().join("out");
    let measure = |with_idea: bool| {
        let mut command = Command::new(server);
        command.arg("--data-dir").arg(&data).arg("measure");
        command.arg("--in").arg(&work).arg("--stdout").arg(&out);
        if with_idea {
            command.arg("--idea").arg(&idea);
        }
        let output = command
            .arg(&module)
            .args(["--", "2", "3"])
            .output()
            .expect("lengua-mcp starts");
        let said = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "measure failed: {said}");
        let measures: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("measure's JSON");
        assert_eq!(measures["exit"], 0, "{measures}");
        assert_eq!(std::fs::read(&out).expect("its output"), b"5\n");
    };
    measure(false);
    assert!(
        !data.join("measured").exists(),
        "a run with no idea kept a folder"
    );
    measure(true);
    measure(true);
    let kept: Vec<_> = std::fs::read_dir(data.join("measured"))
        .expect("the folders measure keeps")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(kept.len(), 1, "{kept:?}");
    let folder = &kept[0];
    assert_eq!(
        std::fs::read(folder.join("main.wasm")).expect("the module"),
        wasm
    );
    let lock = std::fs::read_to_string(folder.join("lengua.lock")).expect("its lock");
    let hash = lengua::hash::Sha256Hex::of(&wasm);
    assert!(
        lock.contains(&format!("module_sha256 = \"{hash}\"")),
        "{lock}"
    );
    let run = || {
        Command::new(server)
            .arg("--data-dir")
            .arg(&data)
            .arg("run")
            .arg(folder)
            .args(["--module", "--in"])
            .arg(&work)
            .args(["2", "3"])
            .output()
            .expect("lengua-mcp starts")
    };
    let sum = run();
    assert_eq!(sum.stdout, b"5\n", "{sum:?}");
    assert_eq!(sum.status.code(), Some(0), "{sum:?}");
    let mut changed = wasm.clone();
    changed.push(0);
    std::fs::write(folder.join("main.wasm"), changed).expect("a changed module");
    let refused = run();
    assert_eq!(refused.status.code(), Some(125), "{refused:?}");
    assert!(refused.stdout.is_empty(), "{refused:?}");
}
