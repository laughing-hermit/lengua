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
