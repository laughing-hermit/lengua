//! `lengua-mcp measure` on Linux counts a program's peak memory as the program's own. Linux begins
//! a process's count of its peak at what the process it was started from held, so the program
//! starts from lengua-spawn, a small process beside lengua-mcp, and not from lengua-mcp, whose
//! megabytes a small program's count would begin at.
#![cfg(target_os = "linux")]

use std::path::Path;
use std::process::Command;

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
/// lengua-mcp would begin its count at, and which `measure` would then leave out. Cargo builds
/// lengua-spawn beside lengua-mcp.
#[test]
fn a_small_programs_peak_is_its_own() {
    let spawner = Path::new(env!("CARGO_BIN_EXE_lengua-spawn"));
    let beside = Path::new(env!("CARGO_BIN_EXE_lengua-mcp")).with_file_name("lengua-spawn");
    assert_eq!(spawner, beside);
    let small = measured("/bin/true", &[]);
    assert_eq!(small["exit"], 0, "{small}");
    let large = measured(env!("CARGO_BIN_EXE_lengua-mcp"), &["--version"]);
    assert_eq!(large["exit"], 0, "{large}");
    let peak = |measures: &serde_json::Value| measures["peak_bytes"].as_u64();
    let (Some(small_peak), Some(large_peak)) = (peak(&small), peak(&large)) else {
        panic!("a peak for each: /bin/true's {small}, lengua-mcp --version's {large}");
    };
    assert!(
        small_peak < large_peak,
        "/bin/true held {small_peak} bytes at most, lengua-mcp --version {large_peak}"
    );
}
