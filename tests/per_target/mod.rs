//! How this computer reaches each target, for the tests written once for every target: on its own
//! CPU, through QEMU's user mode where that is set up for the other CPU, or not at all. A test
//! asks here, runs what it can, and notes (lengua::skiplog) what it could not cover.
//!
//! Every test that holds a table of cells, one per target (`lengua::targets::PerCpu`), runs
//! the cells of the target this computer is natively, on that target's runner in CI; a cell for
//! the other target runs only where QEMU does that CPU's programs, and every cell is at least
//! built (LLVM assembles and links for every target on any computer).

// Each test file uses a different subset of these helpers.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use lengua::platform::Profile;
use lengua::toolchain::Toolchain;

use crate::common::{Protocol, Session};

/// How this computer runs a target's programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// On its own CPU: this computer is the target.
    Native,
    /// Through QEMU's user mode, registered with binfmt_misc's flag F, as a developer's
    /// `LENGUA_TARGET` asks. The kernel sees QEMU's system calls, not the program's.
    Emulated,
}

/// The native target this computer is, when lengua has one for it: macOS and Windows have none;
/// their programs are WebAssembly (todo.md, the pivot).
pub fn this_target() -> Option<Profile> {
    Profile::for_target(std::env::consts::OS, std::env::consts::ARCH)
}

/// The target this computer is, for a test that runs programs and so has checked
/// [`runs_programs`] first.
pub fn host() -> Profile {
    this_target().expect("a test that runs programs checks runs_programs first")
}

/// Whether lengua runs programs on this computer. Where it doesn't, a test that runs one says why
/// in the run's summary and ends.
pub fn runs_programs() -> bool {
    let runs = this_target().is_some();
    if !runs {
        lengua::skiplog::note("native programs run on Linux only");
    }
    runs
}

/// The CPU's name as the run's summary says it.
fn cpu_name(target: Profile) -> &'static str {
    match target {
        Profile::LinuxX86_64 => "x86-64",
        Profile::LinuxAarch64 => "AArch64",
    }
}

/// Whether QEMU's user mode runs `target`'s programs here, registered with binfmt_misc's flag F:
/// the kernel opens QEMU when it is registered, so a program confined to its folder still runs.
pub fn qemu_runs(target: Profile) -> bool {
    let entry = std::fs::read_to_string(format!("/proc/sys/fs/binfmt_misc/qemu-{}", target.cpu()))
        .unwrap_or_default();
    entry.lines().next() == Some("enabled")
        && entry.lines().any(|line| {
            line.strip_prefix("flags: ")
                .is_some_and(|flags| flags.contains('F'))
        })
}

/// How this computer runs `target`'s programs, or `None`, with a note in the run's summary, where
/// it cannot.
pub fn way(target: Profile) -> Option<Way> {
    if !runs_programs() {
        None
    } else if target == host() {
        Some(Way::Native)
    } else if qemu_runs(target) {
        Some(Way::Emulated)
    } else {
        lengua::skiplog::note(&format!(
            "QEMU doesn't run {} programs here (install qemu-user-static)",
            cpu_name(target)
        ));
        None
    }
}

/// A session for `target`, where this computer runs its programs, and the way it runs them; its
/// proofs get no time.
pub async fn session_with_way(target: Profile, protocol: Protocol) -> Option<(Session, Way)> {
    let how = way(target)?;
    let session = match how {
        Way::Native => Session::start(protocol).await,
        Way::Emulated => Session::emulating(protocol, target.name()).await,
    };
    Some((session, how))
}

/// A session for `target`, where this computer runs its programs; its proofs get no time.
pub async fn session(target: Profile, protocol: Protocol) -> Option<Session> {
    session_with_way(target, protocol)
        .await
        .map(|(session, _)| session)
}

/// A session for `target` with time for proofs, once no other session in the test binary is
/// proving: its full time natively, `emulated_seconds` through QEMU.
pub async fn proving_session(
    target: Profile,
    protocol: Protocol,
    emulated_seconds: u64,
) -> Option<Session> {
    Some(match way(target)? {
        Way::Native => Session::proving_alone(protocol).await,
        Way::Emulated => Session::emulating_for(protocol, target.name(), emulated_seconds).await,
    })
}

/// Assembles and links `source` for `target` into `folder` and returns the executable. LLVM
/// builds for every target on any computer, so a program for the other CPU is built (and its
/// typos found) even where it cannot run.
pub async fn build(target: Profile, folder: &Path, name: &str, source: &str) -> PathBuf {
    let toolchain = Toolchain::discover()
        .await
        .expect("llvm-mc and ld.lld are installed");
    std::fs::create_dir_all(folder).expect("a folder for the build");
    let assembly = folder.join(format!("{name}.s"));
    let object = folder.join(format!("{name}.o"));
    let executable = folder.join(name);
    std::fs::write(&assembly, source).expect("write the source");
    toolchain
        .assemble(target, &assembly, &object)
        .await
        .unwrap_or_else(|error| panic!("{name} for {}: {error}", target.name()));
    toolchain
        .link(target, &object, &executable)
        .await
        .unwrap_or_else(|error| panic!("{name} for {}: {error}", target.name()));
    executable
}
