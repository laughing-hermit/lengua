//! Detects the machine (OS, CPU, CPU features) and picks its platform profile.
//!
//! A profile holds everything that differs per OS and CPU: the target triple today, and later the
//! system call table, calling convention and linker details. The MVP has one profile.

use serde::Serialize;

/// The OS + CPU combinations lengua knows how to build for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Linux on x86-64: ELF executables, direct system calls, System V calling convention.
    LinuxX86_64,
}

impl Profile {
    /// The profile for an OS/arch pair as reported by `std::env::consts`, if supported.
    #[must_use]
    pub fn for_target(os: &str, arch: &str) -> Option<Self> {
        match (os, arch) {
            ("linux", "x86_64") => Some(Self::LinuxX86_64),
            _ => None,
        }
    }

    /// Short name, e.g. `linux-x86_64`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::LinuxX86_64 => "linux-x86_64",
        }
    }

    /// The LLVM target triple passed to the assembler.
    #[must_use]
    pub fn llvm_triple(self) -> &'static str {
        match self {
            Self::LinuxX86_64 => "x86_64-unknown-linux-gnu",
        }
    }
}

/// CPU instruction-set extensions the optimize step may use when present.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct CpuFeatures {
    /// AVX2 vector instructions.
    pub avx2: bool,
    /// AVX-512 Foundation.
    pub avx512f: bool,
    /// BMI2 bit-manipulation instructions.
    pub bmi2: bool,
}

/// What lengua knows about the machine it runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    /// Operating system, as in `std::env::consts::OS` (e.g. `linux`).
    pub os: &'static str,
    /// CPU architecture, as in `std::env::consts::ARCH` (e.g. `x86_64`).
    pub arch: &'static str,
    /// Human-readable CPU model, when the OS reports one.
    pub cpu_model: Option<String>,
    /// Detected CPU extensions.
    pub features: CpuFeatures,
    /// The matching profile, or `None` when this platform isn't supported yet.
    pub profile: Option<Profile>,
}

impl Machine {
    /// Detects the current machine.
    #[must_use]
    pub fn detect() -> Self {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        Self {
            os,
            arch,
            cpu_model: cpu_model(),
            features: cpu_features(),
            profile: Profile::for_target(os, arch),
        }
    }
}

fn cpu_model() -> Option<String> {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    model_name_from_cpuinfo(&cpuinfo)
}

fn model_name_from_cpuinfo(cpuinfo: &str) -> Option<String> {
    cpuinfo.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == "model name").then(|| value.trim().to_owned())
    })
}

#[cfg(target_arch = "x86_64")]
fn cpu_features() -> CpuFeatures {
    CpuFeatures {
        avx2: std::arch::is_x86_feature_detected!("avx2"),
        avx512f: std::arch::is_x86_feature_detected!("avx512f"),
        bmi2: std::arch::is_x86_feature_detected!("bmi2"),
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn cpu_features() -> CpuFeatures {
    CpuFeatures::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_linux_x86_64_is_supported() {
        assert_eq!(
            Profile::for_target("linux", "x86_64"),
            Some(Profile::LinuxX86_64)
        );
        assert_eq!(Profile::for_target("linux", "aarch64"), None);
        assert_eq!(Profile::for_target("macos", "aarch64"), None);
        assert_eq!(Profile::for_target("windows", "x86_64"), None);
    }

    #[test]
    fn reads_model_name() {
        let cpuinfo = "processor\t: 0\nmodel name\t: Example CPU 9000\nflags\t: fpu\n";
        assert_eq!(
            model_name_from_cpuinfo(cpuinfo).as_deref(),
            Some("Example CPU 9000")
        );
        assert_eq!(model_name_from_cpuinfo("processor\t: 0\n"), None);
    }
}
