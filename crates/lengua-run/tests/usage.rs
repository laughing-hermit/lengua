use std::process::Command;

fn lengua_run(args: &[&str]) -> std::process::Output {
    let home = std::env::temp_dir().join(format!("lengua-run-usage-{}", std::process::id()));
    Command::new(env!("CARGO_BIN_EXE_lengua-run"))
        .args(args)
        .env("LENGUA_HOME", home)
        .output()
        .expect("lengua-run starts")
}

#[test]
fn the_version_and_the_usage_line() {
    let version = lengua_run(&["--version"]);
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8_lossy(&version.stdout),
        format!("lengua-run {}\n", env!("CARGO_PKG_VERSION"))
    );
    let usage = lengua_run(&[]);
    assert_eq!(usage.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&usage.stderr).starts_with("usage: lengua-run <folder>"));
}

#[test]
fn a_folder_with_no_key_or_no_lock_is_refused() {
    let folder = std::env::temp_dir().join(format!("lengua-run-empty-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("a folder");
    let refused = lengua_run(&[folder.to_string_lossy().as_ref()]);
    assert_eq!(refused.status.code(), Some(125));
    let message = String::from_utf8_lossy(&refused.stderr);
    assert!(message.starts_with("lengua-run: refused: "), "{message}");
    assert!(message.contains("call check"), "{message}");
}
