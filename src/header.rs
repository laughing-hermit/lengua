//! The line lengua writes into main.s once its code has passed a check: the idea it was compiled
//! from, the target it was compiled for, and the CPU features its instructions need beyond that
//! target's baseline:
//!
//! `# lengua: compiled from idea.md sha256:… for linux-x86_64; needs AVX2, BMI2`
//!
//! It names the code; it doesn't protect it. main.s is plain text anyone can build outside
//! lengua, so code someone shares is checked again on each computer before lengua runs it. The
//! line sits second, after `.intel_syntax noprefix` or `.arch armv8-a`, as a comment of the
//! target's CPU (`// lengua: …` on AArch64), and lengua's hash of main.s leaves it out, so
//! writing it changes nothing that was checked.

use crate::checker::x86_64;
use crate::hash::Sha256Hex;
use crate::isa::Isa;
use crate::platform::{CpuFeatures, Profile};

/// The line after its comment marker.
const BODY: &str = " lengua: compiled from idea.md sha256:";

/// What lengua's line in main.s says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// lengua's hash of the idea.md the code was compiled from, which leaves its trust record out
    /// ([`crate::trust::idea_hash`]).
    pub idea: String,
    /// The target it was compiled for, such as `linux-x86_64`.
    pub target: String,
    /// The CPU features its instructions need beyond the target's baseline.
    pub features: Vec<String>,
}

impl Header {
    /// The line itself, a comment of the target's CPU.
    #[must_use]
    pub fn line(&self) -> String {
        let comment = Profile::from_name(&self.target)
            .map_or(Isa::X86_64, Profile::isa)
            .comment();
        let mut line = format!("{comment}{BODY}{} for {}", self.idea, self.target);
        if !self.features.is_empty() {
            line.push_str("; needs ");
            line.push_str(&self.features.join(", "));
        }
        line
    }

    /// The line `code` carries, when it has one lengua can read.
    #[must_use]
    pub fn of(code: &str) -> Option<Self> {
        let line = code.lines().find(|line| is_header(line))?;
        let (idea, rest) = body(line.trim_end())?.split_once(" for ")?;
        let (target, features) = match rest.split_once("; needs ") {
            Some((target, features)) => (target, features.split(", ").map(str::to_owned).collect()),
            None => (rest, Vec::new()),
        };
        Some(Self {
            idea: idea.to_owned(),
            target: target.to_owned(),
            features,
        })
    }

    /// Why code with this line can't run here, when it can't: another target, or features this
    /// CPU lacks.
    #[must_use]
    pub fn elsewhere(&self, profile: Profile, cpu: &CpuFeatures) -> Option<String> {
        if self.target != profile.name() {
            return Some(format!(
                "main.s was compiled for {}, and this computer is {}",
                self.target,
                profile.name()
            ));
        }
        let has = |name: &str| match profile.isa() {
            Isa::X86_64 => x86_64::cpu_has(name, cpu),
            Isa::Aarch64 => crate::checker::aarch64::cpu_has(name, cpu),
        };
        let lacking: Vec<&str> = self
            .features
            .iter()
            .filter(|name| has(name) == Some(false))
            .map(String::as_str)
            .collect();
        (!lacking.is_empty()).then(|| {
            let sve = lacking.iter().find_map(|name| name.strip_prefix("sve"));
            match sve {
                // A proof of SVE code holds at the vector length it ran at.
                Some(bytes) => format!(
                    "main.s was proven with SVE vectors of {bytes} bytes, and this computer's CPU \
                     {}",
                    if cpu.sve == 0 {
                        "has no SVE".to_owned()
                    } else {
                        format!("has {}-byte vectors", cpu.sve)
                    }
                ),
                None => format!(
                    "main.s was compiled for a CPU with {}, which this computer's CPU lacks",
                    lacking.join(", ")
                ),
            }
        })
    }
}

/// What follows the header's prefix, when `line` is a header: lengua's words after either
/// CPU's comment marker.
fn body(line: &str) -> Option<&str> {
    [Isa::X86_64, Isa::Aarch64]
        .into_iter()
        .find_map(|isa| line.strip_prefix(isa.comment())?.strip_prefix(BODY))
}

fn is_header(line: &str) -> bool {
    body(line).is_some()
}

/// main.s as lengua hashes it: without lengua's own line, so writing that line changes nothing
/// that was checked.
#[must_use]
pub fn code_hash(code: &[u8]) -> Sha256Hex {
    match std::str::from_utf8(code) {
        Ok(text) if text.lines().any(is_header) => {
            let kept: String = text
                .split_inclusive('\n')
                .filter(|line| !is_header(line))
                .collect();
            Sha256Hex::of(kept.as_bytes())
        }
        _ => Sha256Hex::of(code),
    }
}

/// `code` with `header` as lengua's line, in place of any it had: second, after
/// `.intel_syntax noprefix` or `.arch armv8-a`.
#[must_use]
pub fn with(code: &str, header: &Header) -> String {
    let mut lines = code.split_inclusive('\n').filter(|line| !is_header(line));
    let first = lines.next().unwrap_or_default();
    let newline = if first.ends_with('\n') { "" } else { "\n" };
    format!(
        "{first}{newline}{}\n{}",
        header.line(),
        lines.collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE: &str = ".intel_syntax noprefix\n.globl _start\n_start:\n    ret\n";

    fn header(features: &[&str]) -> Header {
        Header {
            idea: Sha256Hex::of(b"idea").to_string(),
            target: "linux-x86_64".into(),
            features: features.iter().map(|&name| name.to_owned()).collect(),
        }
    }

    #[test]
    fn writes_the_line_second_and_reads_it_back() {
        let written = with(CODE, &header(&["AVX2", "BMI2"]));
        let lines: Vec<&str> = written.lines().collect();
        assert_eq!(lines[0], ".intel_syntax noprefix");
        assert_eq!(
            lines[1],
            format!(
                "# lengua: compiled from idea.md sha256:{} for linux-x86_64; needs AVX2, BMI2",
                Sha256Hex::of(b"idea")
            )
        );
        assert_eq!(&lines[2..], &CODE.lines().collect::<Vec<_>>()[1..]);
        assert_eq!(Header::of(&written), Some(header(&["AVX2", "BMI2"])));
        // Writing it again replaces it.
        let again = with(&written, &header(&[]));
        assert_eq!(again.matches("# lengua:").count(), 1, "{again}");
        assert_eq!(Header::of(&again), Some(header(&[])));
        assert!(!again.contains("needs"), "{again}");
        assert_eq!(Header::of(CODE), None);
    }

    /// On AArch64 the line is a `//` comment, since `#` starts a number there.
    #[test]
    fn an_aarch64_line_is_a_comment_there() {
        let code = ".arch armv8-a\n.globl _start\n_start:\n    ret\n";
        let aarch64 = Header {
            target: "linux-aarch64".into(),
            ..header(&[])
        };
        let written = with(code, &aarch64);
        let lines: Vec<&str> = written.lines().collect();
        assert_eq!(lines[0], ".arch armv8-a");
        assert_eq!(
            lines[1],
            format!(
                "// lengua: compiled from idea.md sha256:{} for linux-aarch64",
                Sha256Hex::of(b"idea")
            )
        );
        assert_eq!(Header::of(&written), Some(aarch64.clone()));
        assert_eq!(code_hash(written.as_bytes()), code_hash(code.as_bytes()));
        assert_eq!(with(&written, &aarch64), written);
    }

    #[test]
    fn the_line_leaves_the_code_hash_alone() {
        let written = with(CODE, &header(&["AVX2"]));
        assert_ne!(written, CODE);
        assert_eq!(code_hash(written.as_bytes()), code_hash(CODE.as_bytes()));
        assert_eq!(code_hash(CODE.as_bytes()), Sha256Hex::of(CODE.as_bytes()));
        // Any other edit changes it.
        let edited = written.replace("ret", "ud2");
        assert_ne!(code_hash(edited.as_bytes()), code_hash(CODE.as_bytes()));
    }

    #[test]
    fn says_why_code_for_elsewhere_cant_run_here() {
        let cpu = CpuFeatures {
            avx2: true,
            ..CpuFeatures::default()
        };
        let here = Profile::LinuxX86_64;
        assert_eq!(header(&["AVX2"]).elsewhere(here, &cpu), None);
        let lacking = header(&["AVX2", "AVX-512BW"])
            .elsewhere(here, &cpu)
            .expect("a feature it lacks");
        assert!(lacking.contains("AVX-512BW"), "{lacking}");
        assert!(!lacking.contains("AVX2,"), "{lacking}");
        let other = Header {
            target: "macos-aarch64".into(),
            ..header(&[])
        };
        let elsewhere = other.elsewhere(here, &cpu).expect("another target");
        assert!(elsewhere.contains("macos-aarch64"), "{elsewhere}");
        // Names lengua doesn't know don't stop it.
        assert_eq!(header(&["QUANTUM"]).elsewhere(here, &cpu), None);
    }
}
