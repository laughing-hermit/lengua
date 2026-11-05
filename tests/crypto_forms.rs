//! The programs whose only form once needed the CPU's SHA, AES or CRC instructions each have a
//! baseline form that runs on every computer of its target, in plain integer code (with SSE2 or
//! NEON for copies), and the form that uses the instructions is a named fast tier beside it,
//! compiled from the very same spec. The baseline is the shipped example or fixture; the tier
//! lives with the skill's other fast programs:
//!
//! - SHA-256: `examples/sha256`, tier `fast/sha256-ni`; on ARM64 the twin in
//!   `tests/fixtures/linux-aarch64/sha256`, tier `linux-aarch64/fast/sha256-sha2`.
//! - AES-128: `examples/aes`, tier `fast/aes-ni`; on ARM64 `aes` and `fast/aes-aes`.
//! - CRC-32C: `fast/crc32c` (x86-64's is the SSE4.2 `crc32` instruction in one chain, which its
//!   baseline holds), tier `fast/crc32c-pclmul`; on ARM64 `fast/crc32c` (a table) and
//!   `fast/crc32c-crc`.
//!
//! The baselines are taken through the server on every runner by the suites that check every
//! example and fixture; here the fast tiers are, on the runners that have the feature, and a
//! tier's absence of its feature is noted, never silent.

mod common;
mod per_target;

use std::path::Path;

use common::{Program, Protocol, Session};
use lengua::platform::{CpuFeatures, Machine};
use serde_json::Value;

/// A program with a baseline form and fast tiers: the folders of its forms on each CPU, the
/// baseline first.
struct Forms {
    /// The program.
    name: &'static str,
    /// The forms on x86-64.
    x86_64: &'static [&'static str],
    /// The forms on ARM64.
    aarch64: &'static [&'static str],
}

const FORMS: [Forms; 3] = [
    Forms {
        name: "sha256",
        x86_64: &[
            "examples/sha256",
            "plugin/skills/compile/assets/fast/sha256-ni",
        ],
        aarch64: &[
            "tests/fixtures/linux-aarch64/sha256",
            "plugin/skills/compile/assets/linux-aarch64/fast/sha256-sha2",
        ],
    },
    Forms {
        name: "aes",
        x86_64: &["examples/aes", "plugin/skills/compile/assets/fast/aes-ni"],
        aarch64: &[
            "tests/fixtures/linux-aarch64/aes",
            "plugin/skills/compile/assets/linux-aarch64/fast/aes-aes",
        ],
    },
    Forms {
        name: "crc32c",
        x86_64: &[
            "plugin/skills/compile/assets/fast/crc32c",
            "plugin/skills/compile/assets/fast/crc32c-pclmul",
        ],
        aarch64: &[
            "plugin/skills/compile/assets/linux-aarch64/fast/crc32c",
            "plugin/skills/compile/assets/linux-aarch64/fast/crc32c-crc",
        ],
    },
];

/// A fast tier to take through the server: its folder, whether a CPU has what it uses, and what
/// the note says where it hasn't.
type Tier = (&'static str, fn(&CpuFeatures) -> bool, &'static str);

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
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

/// Every form of a program, on both CPUs, is compiled from the very same spec: the same bytes.
#[test]
fn every_form_of_a_program_has_the_very_spec_of_the_others() {
    let mut problems = Vec::new();
    for forms in &FORMS {
        let baseline = root().join(forms.x86_64[0]).join("spec.md");
        let spec = std::fs::read(&baseline)
            .unwrap_or_else(|error| panic!("{}: {error}", baseline.display()));
        for folder in forms.x86_64.iter().chain(forms.aarch64) {
            let path = root().join(folder);
            if !path.join("main.s").is_file() {
                problems.push(format!("{}: {folder} has no main.s", forms.name));
            }
            match std::fs::read(path.join("spec.md")) {
                Ok(found) if found == spec => {}
                Ok(_) => problems.push(format!("{}: {folder}/spec.md differs", forms.name)),
                Err(error) => problems.push(format!("{folder}/spec.md: {error}")),
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// The instructions of `code`, one mnemonic for each line of code: comments cut at `marker`,
/// labels and directives left out.
fn mnemonics<'a>(code: &'a str, marker: &str) -> Vec<&'a str> {
    code.lines()
        .map(|line| line.split(marker).next().unwrap_or_default().trim())
        .filter(|line| !line.is_empty() && !line.ends_with(':') && !line.starts_with('.'))
        .filter_map(|line| line.split_whitespace().next())
        .collect()
}

/// A baseline form uses nothing beyond its CPU's baseline: x86-64's SSE2 to SSE4.2 and POPCNT,
/// ARM64's ARMv8.0 with NEON. So no SHA, AES or carry-less multiply, no vector registers wider
/// than xmm (every VEX and EVEX instruction starts with `v`) and no BMI on x86-64, and on ARM64
/// no extension line and no SHA, AES, CRC32 or PMULL instruction.
#[test]
fn a_baseline_form_uses_no_extension_beyond_its_cpus_baseline() {
    // The families of instructions that start alike, and the single ones, of x86-64's extensions;
    // crc32 is SSE4.2's, which the x86-64 baseline holds.
    let x86_64_families = ["sha1", "sha256", "aes", "gf2p8"];
    let x86_64_singles = [
        "pclmulqdq",
        "bzhi",
        "mulx",
        "pdep",
        "pext",
        "rorx",
        "sarx",
        "shlx",
        "shrx",
        "andn",
        "bextr",
        "blsi",
        "blsmsk",
        "blsr",
        "tzcnt",
        "lzcnt",
        "movbe",
        "adcx",
        "adox",
    ];
    let aarch64_beyond = [
        "sha1", "sha256", "sha512", "aes", "crc32", "pmull", "sm3", "sm4",
    ];
    let mut problems = Vec::new();
    for forms in &FORMS {
        let x86_64 = root().join(forms.x86_64[0]).join("main.s");
        let code = std::fs::read_to_string(&x86_64).expect("the x86-64 baseline");
        for mnemonic in mnemonics(&code, "#") {
            let outside = mnemonic.starts_with('v')
                || x86_64_singles.contains(&mnemonic)
                || x86_64_families
                    .iter()
                    .any(|family| mnemonic.starts_with(family));
            if outside {
                problems.push(format!("{}: x86-64 uses {mnemonic}", forms.name));
            }
        }
        let aarch64 = root().join(forms.aarch64[0]).join("main.s");
        let code = std::fs::read_to_string(&aarch64).expect("the ARM64 baseline");
        if code.lines().any(|line| line.starts_with(".arch_extension")) {
            problems.push(format!("{}: ARM64 names an extension", forms.name));
        }
        for mnemonic in mnemonics(&code, "//") {
            if aarch64_beyond
                .iter()
                .any(|beyond| mnemonic.starts_with(beyond))
            {
                problems.push(format!("{}: ARM64 uses {mnemonic}", forms.name));
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// Takes the program in `folder` through the server, with no time for proofs: its rules and
/// every test.
async fn passes_its_checks(session: &Session, folder: &str) {
    let program = Program::new();
    for file in ["spec.md", "main.s"] {
        let path = root().join(folder).join(file);
        let contents = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        program.write(file, &contents);
    }
    session.record(&program).await;
    let report = session.settle(&program).await;
    assert_eq!(report["state"], "done", "{folder}: {report}");
    assert!(
        any_line(&report["tests"], "follows every rule and passed all"),
        "{folder}: {report}"
    );
}

/// The x86-64 baseline of the CRC-32C, which needs only SSE4.2, passes its checks on any
/// runner: it is the form that no AVX-512 draw can skip.
#[cfg(target_arch = "x86_64")]
#[tokio::test]
async fn the_x86_64_crc32c_baseline_passes_its_checks() {
    if !per_target::runs_programs() {
        return;
    }
    if !Machine::detect().features.sse4_2 {
        lengua::skiplog::note("this CPU has no SSE4.2");
        return;
    }
    let session = Session::start(Protocol::Discover).await;
    passes_its_checks(&session, FORMS[2].x86_64[0]).await;
    session.service.cancel().await.expect("close the session");
}

/// Each x86-64 fast tier passes its checks where the CPU has what it uses: the SHA and AES
/// instructions, and for the CRC-32C the carry-less multiply.
#[cfg(target_arch = "x86_64")]
#[tokio::test]
async fn the_x86_64_fast_tiers_pass_their_checks() {
    if !per_target::runs_programs() {
        return;
    }
    let tiers: [Tier; 3] = [
        (
            FORMS[0].x86_64[1],
            |features| features.sha && features.ssse3 && features.sse4_1,
            "this CPU has no SHA extensions",
        ),
        (
            FORMS[1].x86_64[1],
            |features| features.aes && features.ssse3,
            "this CPU has no AES instructions",
        ),
        (
            FORMS[2].x86_64[1],
            |features| features.sse4_2 && features.pclmulqdq,
            "this CPU has no PCLMULQDQ",
        ),
    ];
    let features = Machine::detect().features;
    let session = Session::start(Protocol::Discover).await;
    for (folder, has, missing) in tiers {
        if has(&features) {
            passes_its_checks(&session, folder).await;
        } else {
            lengua::skiplog::note(missing);
        }
    }
    session.service.cancel().await.expect("close the session");
}

/// Each ARM64 fast tier passes its checks natively where the CPU has its extension, and under
/// QEMU, whose CPU has them all, on an x86-64 computer.
#[tokio::test]
async fn the_aarch64_fast_tiers_pass_their_checks() {
    if !per_target::runs_programs() {
        return;
    }
    let native = cfg!(target_arch = "aarch64");
    let session = if native {
        Session::start(Protocol::Discover).await
    } else if common::qemu_runs_aarch64() {
        Session::emulating(Protocol::Discover, "linux-aarch64").await
    } else {
        return;
    };
    let tiers: [Tier; 3] = [
        (
            FORMS[0].aarch64[1],
            |features| features.sha,
            "this CPU has no SHA2 instructions",
        ),
        (
            FORMS[1].aarch64[1],
            |features| features.aes,
            "this CPU has no AES instructions",
        ),
        (
            FORMS[2].aarch64[1],
            |features| features.crc && features.pmull,
            "this CPU has no CRC32 and PMULL instructions",
        ),
    ];
    let features = Machine::detect().features;
    for (folder, has, missing) in tiers {
        if native && !has(&features) {
            lengua::skiplog::note(missing);
        } else {
            passes_its_checks(&session, folder).await;
        }
    }
    session.service.cancel().await.expect("close the session");
}
