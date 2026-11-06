//! Every program lengua ships, on every target, goes through the server: its rules, its tests and,
//! with no time for proofs, nothing more. Each target ships a twin of every program, compiled from
//! the very same idea, so neither CPU has a program the other CPU's suite doesn't check: one
//! product on every target, only the hardware differing.
//!
//! The test is written once, with a cell for each target (`PerCpu`) saying where its programs
//! are and what its CPU can lack; a target added to lengua breaks it until its cells are filled.
//! It takes the programs of every target this computer runs, natively or through QEMU's user
//! mode; the others' are taken on their own runners.

mod common;
mod per_target;

use std::path::{Path, PathBuf};

use common::{Program, Protocol};
use lengua::platform::{CpuFeatures, Emulator, Machine, Profile, Target};
use lengua::targets::PerCpu;
use per_target::Way;
use serde_json::Value;

/// The folders of each target's programs, relative to the repository: a program is a folder with
/// a idea.md and a main.s.
const FOLDERS: PerCpu<&[&str]> = PerCpu {
    x86_64: &["examples", "tests/fixtures"],
    aarch64: &["tests/fixtures/linux-aarch64"],
};

/// An optional extension of a CPU: a program that uses it can't run on a CPU without it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Extension {
    Avx512bw,
    Aes,
    Sha,
    Pclmulqdq,
    Crc,
}

impl Extension {
    /// Its name, as the run's summary says it.
    fn name(self) -> &'static str {
        match self {
            Self::Avx512bw => "AVX-512BW",
            Self::Aes => "AES",
            Self::Sha => "SHA",
            Self::Pclmulqdq => "PCLMULQDQ",
            Self::Crc => "CRC-32",
        }
    }

    /// Whether `features` has it.
    fn present(self, features: &CpuFeatures) -> bool {
        match self {
            Self::Avx512bw => features.avx512bw,
            Self::Aes => features.aes,
            Self::Sha => features.sha,
            Self::Pclmulqdq => features.pclmulqdq,
            Self::Crc => features.crc,
        }
    }
}

/// The optional extensions of each target's CPU, each with words of a program that show it uses
/// the extension. The baseline (x86-64's SSE2 to SSE4.2 and POPCNT, ARM64's NEON) isn't here:
/// every machine of the target has it.
const EXTENSIONS: PerCpu<&[(Extension, &[&str])]> = PerCpu {
    x86_64: &[
        (Extension::Avx512bw, &["vpcmpub", "zmm"]),
        (Extension::Aes, &["aesenc"]),
        (Extension::Sha, &["sha256rnds2"]),
        (Extension::Pclmulqdq, &["pclmulqdq"]),
    ],
    aarch64: &[
        (Extension::Crc, &["crc32"]),
        (Extension::Aes, &["aese", "aesd"]),
        (Extension::Sha, &["sha256h", "sha1c"]),
    ],
};

/// The extension `code` uses that this computer's CPU for `target` lacks, if any: a program for
/// it can't run here, and the rules test of the whole repository checks it for every extension.
fn lacking(target: Profile, code: &str, features: &CpuFeatures) -> Option<Extension> {
    EXTENSIONS
        .get(target)
        .iter()
        .find(|(extension, words)| {
            words.iter().any(|word| code.contains(word)) && !extension.present(features)
        })
        .map(|(extension, _)| *extension)
}

/// The CPU features a session for `target` runs programs with: this computer's, or under QEMU
/// the emulated CPU's.
fn features(target: Profile, way: Way) -> CpuFeatures {
    Target {
        profile: target,
        emulator: (way == Way::Emulated).then(|| Emulator {
            name: "qemu".into(),
        }),
    }
    .features(&Machine::detect())
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// Whether any of `lines` contains `needle`.
fn any_line(lines: &Value, needle: &str) -> bool {
    lines
        .as_array()
        .is_some_and(|lines| lines.iter().any(|line| text(line).contains(needle)))
}

/// Every program folder of `root`'s `folders`, each with a main.s: its name and path.
fn programs(root: &Path, folders: &[&str]) -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    for folder in folders {
        for entry in std::fs::read_dir(root.join(folder)).expect("a folder of programs") {
            let path = entry.expect("an entry").path();
            if path.join("main.s").is_file()
                && let Some(name) = path.file_name().and_then(|name| name.to_str())
            {
                found.push((name.to_owned(), path.clone()));
            }
        }
    }
    found.sort();
    found
}

/// Every program of every target follows every rule and passes its tests through the server.
#[tokio::test]
async fn every_program_passes_its_checks_on_every_target() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for target in Profile::ALL {
        let Some((session, way)) = per_target::session_with_way(target, Protocol::Discover).await
        else {
            continue;
        };
        let machine_features = features(target, way);
        let found = programs(root, FOLDERS.get(target));
        assert!(!found.is_empty(), "no programs for {}", target.name());
        for (name, folder) in found {
            let read = |file: &str| {
                std::fs::read_to_string(folder.join(file))
                    .unwrap_or_else(|error| panic!("{name}/{file}: {error}"))
            };
            // A program for an extension this CPU lacks can't run here (GitHub's x86-64 runners
            // are a mixed pool, about half without AVX-512): it is left out, and noted. The
            // rules test of the whole repository checks its rules for every extension.
            if let Some(extension) = lacking(target, &read("main.s"), &machine_features) {
                lengua::skiplog::note(&format!("this CPU has no {}", extension.name()));
                continue;
            }
            let program = Program::new();
            for file in ["idea.md", "main.s"] {
                program.write(file, &read(file));
            }
            session.record(&program).await;
            let report = session.settle(&program).await;
            let what = format!("{name} on {}", target.name());
            if name == "second" {
                // Wrong by design on every target: one fill reads both lines only when no read
                // ends right after the first, and the tests' own pieces of standard input find
                // it.
                assert_eq!(report["state"], "tests_failed", "{what}: {report}");
                continue;
            }
            assert_eq!(report["state"], "done", "{what}: {report}");
            assert!(
                any_line(&report["tests"], "follows every rule and passed all"),
                "{what}: {report}"
            );
        }
        session.service.cancel().await.expect("close the session");
    }
}

/// Every program has its twin on every other target, compiled from the very same idea, and no
/// target has a program the others lack: one product on every target, only the hardware
/// differing.
#[test]
fn every_program_has_its_twin_on_the_other_cpu() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let shipped: Vec<(Profile, Vec<(String, PathBuf)>)> = Profile::ALL
        .into_iter()
        .map(|target| (target, programs(root, FOLDERS.get(target))))
        .collect();
    let idea = |folder: &Path| std::fs::read(folder.join("idea.md")).ok();
    let mut problems = Vec::new();
    for (target, found) in &shipped {
        for (name, folder) in found {
            for (other, others) in &shipped {
                if other == target {
                    continue;
                }
                match others.iter().find(|(known, _)| known == name) {
                    None => problems.push(format!(
                        "{name}: on {}, with no twin on {}",
                        target.name(),
                        other.name()
                    )),
                    Some((_, twin)) if idea(folder) != idea(twin) => problems.push(format!(
                        "{name}: its {} twin's idea differs from {}'s",
                        other.name(),
                        target.name()
                    )),
                    Some(_) => {}
                }
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
    assert!(
        shipped.iter().all(|(_, found)| found.len() >= 29),
        "29 programs on each target: {:?}",
        shipped
            .iter()
            .map(|(target, found)| (target.name(), found.len()))
            .collect::<Vec<_>>()
    );
}
