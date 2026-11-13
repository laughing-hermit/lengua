use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn doctor_finds_the_toolchain_and_both_targets() {
    let output = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .arg("doctor")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("lengua-mcp runs");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{text}");
    for part in [
        "ok       cargo: cargo ",
        "ok       rustc 1.85 or later: rustc ",
        "ok       the target wasm32-wasip2: installed",
        "ok       the target wasm32-unknown-unknown: installed",
    ] {
        assert!(text.contains(part), "{part} in {text}");
    }
    assert!(text.ends_with("lengua can build programs.\n"), "{text}");
}

#[test]
fn doctor_without_rust_says_how_to_install_it_and_fails() {
    let empty = Path::new(env!("CARGO_TARGET_TMPDIR")).join("lengua-mcp-doctor-empty");
    fs::create_dir_all(&empty).expect("a folder");
    let output = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .arg("doctor")
        .current_dir(&empty)
        .env("PATH", &empty)
        .output()
        .expect("lengua-mcp runs");
    let text = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{text}");
    assert!(
        text.contains("missing  cargo: not found; to install it, run: "),
        "{text}"
    );
    assert!(text.contains("rustup target add wasm32-wasip2"), "{text}");
    assert!(
        text.ends_with("until the missing parts are installed.\n"),
        "{text}"
    );
}
