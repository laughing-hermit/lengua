//! CI's format job, which checks formatting once, beside the tests rather than in front of them,
//! so that a difference never hides a test result; and makes the lock, since nothing builds on
//! the owner's computer. Its two steps run the task crate without the alias's `--locked`, which
//! would stop them where they are needed, when Cargo.lock is behind Cargo.toml: cargo then makes
//! the lock as it runs them.
//!
//! - `cargo xtask format`: `cargo fmt --check`, and when it finds a difference, the patch that
//!   formats it, fmt.patch, to apply with git, and the job fails.
//! - `cargo xtask lock`: when Cargo.lock was behind Cargo.toml, the lock cargo made of it,
//!   lock/Cargo.lock, to copy into the repository, and the job fails.

use std::process::{Command, Stdio};

use crate::{ci, files};

/// What a lock behind Cargo.toml says.
const BEHIND: &str = "Cargo.lock is behind Cargo.toml: lock/Cargo.lock is the lock cargo made, \
                      to copy into the repository";

/// `cargo xtask format`.
pub fn format(arguments: &[String]) -> Result<u8, String> {
    if !arguments.is_empty() {
        return Err("usage: cargo xtask format".to_owned());
    }
    let root = ci::root()?;
    let checked = ci::cargo()
        .current_dir(&root)
        .args(["fmt", "--check"])
        .status()
        .map_err(|error| format!("cargo fmt didn't start: {error}"))?;
    if checked.success() {
        return Ok(0);
    }
    ci::must(ci::cargo().current_dir(&root).arg("fmt"))?;
    // rustfmt's changes alone: cargo, which started this task, may have made the lock anew.
    let diff = Command::new("git")
        .current_dir(&root)
        .args(["diff", "--", ".", ":(exclude)Cargo.lock"])
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("git didn't start: {error}"))?;
    std::fs::write(root.join("fmt.patch"), diff.stdout)
        .map_err(|error| format!("fmt.patch: {error}"))?;
    Ok(1)
}

/// `cargo xtask lock`.
pub fn lock(arguments: &[String]) -> Result<u8, String> {
    if !arguments.is_empty() {
        return Err("usage: cargo xtask lock".to_owned());
    }
    let root = ci::root()?;
    // cargo, which started this task, resolved the workspace as `cargo metadata` does, and wrote
    // the lock it made of a Cargo.lock behind Cargo.toml: the commit's lock and the file differ.
    ci::must(
        ci::cargo()
            .current_dir(&root)
            .args(["metadata", "--format-version", "1"])
            .stdout(Stdio::null()),
    )?;
    let unchanged = Command::new("git")
        .current_dir(&root)
        .args(["diff", "--quiet", "--", "Cargo.lock"])
        .status()
        .map_err(|error| format!("git didn't start: {error}"))?;
    if unchanged.success() {
        return Ok(0);
    }
    let kept = root.join("lock");
    files::folder(&kept)?;
    files::copy(&root.join("Cargo.lock"), &kept.join("Cargo.lock"))?;
    Err(BEHIND.to_owned())
}
