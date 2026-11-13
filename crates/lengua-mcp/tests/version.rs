use std::process::Command;

#[test]
fn version_flag_prints_the_name_and_the_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .arg("--version")
        .output()
        .expect("lengua-mcp runs");
    let expected = format!("{} {}\n", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
}
