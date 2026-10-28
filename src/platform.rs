//! Detects the machine (OS, CPU, CPU features) and picks its platform profile.
//!
//! A profile holds everything that differs per OS and CPU: the target triple today, and later the
//! system call table, calling convention and linker details. The MVP has one profile.

use serde::Serialize;

use crate::spec::Syscall;

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

    /// The number a program loads into `eax` to make this system call.
    #[must_use]
    pub fn syscall_number(self, syscall: Syscall) -> i64 {
        match (self, syscall) {
            (Self::LinuxX86_64, Syscall::Read) => 0,
            (Self::LinuxX86_64, Syscall::Write) => 1,
            (Self::LinuxX86_64, Syscall::Close) => 3,
            (Self::LinuxX86_64, Syscall::Lseek) => 8,
            (Self::LinuxX86_64, Syscall::Exit) => 60,
            (Self::LinuxX86_64, Syscall::ExitGroup) => 231,
            (Self::LinuxX86_64, Syscall::Openat) => 257,
        }
    }

    /// The flags `parallel_for` starts threads with: `CLONE_VM`, `CLONE_FS`, `CLONE_FILES`,
    /// `CLONE_SIGHAND`, `CLONE_THREAD`, `CLONE_SYSVSEM`, `CLONE_PARENT_SETTID` and
    /// `CLONE_CHILD_CLEARTID`.
    pub const CLONE_FLAGS: u64 = 0x0035_0F00;

    /// The system calls the kernel lets a program make: those its spec allows; `write` and
    /// `exit_group`, which `assert_fail` may always make; for a program whose spec allows
    /// threads, `clone` with exactly [`Self::CLONE_FLAGS`], `futex` and the thread's own `exit`;
    /// and what the launcher needs between confining itself and becoming the program
    /// (`rt_sigprocmask`, `rt_sigaction`, `execve`).
    #[must_use]
    pub fn allowed_syscalls(self, spec: &[Syscall], threads: bool) -> Vec<Allowed> {
        let (launch, clone, futex): (&[i64], i64, i64) = match self {
            Self::LinuxX86_64 => (&[14, 13, 59], 56, 202),
        };
        let mut allowed: Vec<Allowed> = spec
            .iter()
            .chain(&[Syscall::Write, Syscall::ExitGroup])
            .chain(if threads { &[Syscall::Exit][..] } else { &[] })
            .map(|syscall| Allowed::any(self.syscall_number(*syscall)))
            .chain(launch.iter().copied().map(Allowed::any))
            .collect();
        if threads {
            allowed.push(Allowed {
                number: clone,
                first_argument: Some(Self::CLONE_FLAGS),
            });
            allowed.push(Allowed::any(futex));
        }
        allowed.sort_unstable();
        allowed.dedup();
        allowed
    }
}

/// A system call the kernel lets a program make, by number, and the only first argument it may
/// pass, if just one is allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Allowed {
    /// The system call's number.
    pub number: i64,
    /// The one first argument allowed, if any.
    pub first_argument: Option<u64>,
}

impl Allowed {
    /// The system call with any arguments.
    #[must_use]
    pub fn any(number: i64) -> Self {
        Self {
            number,
            first_argument: None,
        }
    }
}

/// CPU instruction-set extensions beyond the x86-64 baseline (which includes SSE2). Programs may
/// use an extension's instructions only when this CPU has it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct CpuFeatures {
    /// BMI1 bit manipulation (`andn`, `bextr`, `blsi`, `blsmsk`, `blsr`, `tzcnt`).
    pub bmi1: bool,
    /// BMI2 bit manipulation (`bzhi`, `mulx`, `pdep`, `pext`, `rorx`, `sarx`, `shlx`, `shrx`).
    pub bmi2: bool,
    /// `popcnt`.
    pub popcnt: bool,
    /// `lzcnt`.
    pub lzcnt: bool,
    /// Multi-precision add-carry (`adcx`, `adox`).
    pub adx: bool,
    /// Byte-swapping moves (`movbe`).
    pub movbe: bool,
    /// SSE3 vector instructions (`lddqu`).
    pub sse3: bool,
    /// SSSE3 vector instructions (`pshufb`, `pabsb`, `palignr`, `pmaddubsw`, …).
    pub ssse3: bool,
    /// SSE4.1 vector instructions (`pminud`, `pmulld`, `pmovzxbw`, `pextrb`, `ptest`, …).
    pub sse4_1: bool,
    /// SSE4.2 (`pcmpgtq`, the string compares and `crc32`).
    pub sse4_2: bool,
    /// AVX: VEX-encoded 128-bit forms of the SSE instructions, without alignment rules.
    pub avx: bool,
    /// AVX2: 256-bit integer vectors (`vpaddb ymm0, …`).
    pub avx2: bool,
    /// Fused multiply-add (`vfmadd231sd`, …): `a × b + c` rounded once.
    pub fma: bool,
    /// AVX-512 Foundation: 512-bit vectors and mask registers.
    pub avx512f: bool,
    /// AVX-512 byte and word instructions.
    pub avx512bw: bool,
    /// AVX-512 conflict detection (`vpconflictd`, `vplzcntd`).
    pub avx512cd: bool,
    /// AVX-512 doubleword and quadword instructions.
    pub avx512dq: bool,
    /// AVX-512 on 128-bit and 256-bit registers.
    pub avx512vl: bool,
    /// AVX-512 byte permutes (`vpermb`, `vpmultishiftqb`).
    pub avx512vbmi: bool,
    /// AVX-512 byte shifts, compress and expand (`vpshldw`, …).
    pub avx512vbmi2: bool,
    /// AVX-512 dot products (`vpdpbusd`, `vpdpwssd`).
    pub avx512vnni: bool,
    /// AVX-512 bit algorithms (`vpopcntb`, `vpshufbitqmb`).
    pub avx512bitalg: bool,
    /// AVX-512 population counts on doublewords and quadwords (`vpopcntd`).
    pub avx512vpopcntdq: bool,
    /// VEX-encoded dot products (`vpdpbusd` on AVX registers).
    pub avxvnni: bool,
    /// AES rounds on 128-bit lanes (`aesenc`, `aesdec`, …).
    pub aes: bool,
    /// AES rounds on 256-bit and 512-bit registers (`vaesenc ymm0, …`).
    pub vaes: bool,
    /// SHA-1 and SHA-256 rounds (`sha256rnds2`, …).
    pub sha: bool,
    /// Carry-less multiplication (`pclmulqdq`), for CRCs and hashes.
    pub pclmulqdq: bool,
    /// Carry-less multiplication on 256-bit and 512-bit registers.
    pub vpclmulqdq: bool,
    /// Galois-field byte operations (`gf2p8affineqb`, `gf2p8mulb`).
    pub gfni: bool,
}

impl CpuFeatures {
    /// Every feature present, for tests and documentation.
    #[must_use]
    pub fn all() -> Self {
        Self {
            bmi1: true,
            bmi2: true,
            popcnt: true,
            lzcnt: true,
            adx: true,
            movbe: true,
            sse3: true,
            ssse3: true,
            sse4_1: true,
            sse4_2: true,
            avx: true,
            avx2: true,
            fma: true,
            avx512f: true,
            avx512bw: true,
            avx512cd: true,
            avx512dq: true,
            avx512vl: true,
            avx512vbmi: true,
            avx512vbmi2: true,
            avx512vnni: true,
            avx512bitalg: true,
            avx512vpopcntdq: true,
            avxvnni: true,
            aes: true,
            vaes: true,
            sha: true,
            pclmulqdq: true,
            vpclmulqdq: true,
            gfni: true,
        }
    }
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
    /// How many threads can run at once: the cores this process may use.
    pub cores: usize,
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
            cores: std::thread::available_parallelism().map_or(1, std::num::NonZero::get),
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
        bmi1: std::arch::is_x86_feature_detected!("bmi1"),
        bmi2: std::arch::is_x86_feature_detected!("bmi2"),
        popcnt: std::arch::is_x86_feature_detected!("popcnt"),
        lzcnt: std::arch::is_x86_feature_detected!("lzcnt"),
        adx: std::arch::is_x86_feature_detected!("adx"),
        movbe: std::arch::is_x86_feature_detected!("movbe"),
        sse3: std::arch::is_x86_feature_detected!("sse3"),
        ssse3: std::arch::is_x86_feature_detected!("ssse3"),
        sse4_1: std::arch::is_x86_feature_detected!("sse4.1"),
        sse4_2: std::arch::is_x86_feature_detected!("sse4.2"),
        avx: std::arch::is_x86_feature_detected!("avx"),
        avx2: std::arch::is_x86_feature_detected!("avx2"),
        fma: std::arch::is_x86_feature_detected!("fma"),
        avx512f: std::arch::is_x86_feature_detected!("avx512f"),
        avx512bw: std::arch::is_x86_feature_detected!("avx512bw"),
        avx512cd: std::arch::is_x86_feature_detected!("avx512cd"),
        avx512dq: std::arch::is_x86_feature_detected!("avx512dq"),
        avx512vl: std::arch::is_x86_feature_detected!("avx512vl"),
        avx512vbmi: std::arch::is_x86_feature_detected!("avx512vbmi"),
        avx512vbmi2: std::arch::is_x86_feature_detected!("avx512vbmi2"),
        avx512vnni: std::arch::is_x86_feature_detected!("avx512vnni"),
        avx512bitalg: std::arch::is_x86_feature_detected!("avx512bitalg"),
        avx512vpopcntdq: std::arch::is_x86_feature_detected!("avx512vpopcntdq"),
        avxvnni: std::arch::is_x86_feature_detected!("avxvnni"),
        aes: std::arch::is_x86_feature_detected!("aes"),
        vaes: std::arch::is_x86_feature_detected!("vaes"),
        sha: std::arch::is_x86_feature_detected!("sha"),
        pclmulqdq: std::arch::is_x86_feature_detected!("pclmulqdq"),
        vpclmulqdq: std::arch::is_x86_feature_detected!("vpclmulqdq"),
        gfni: std::arch::is_x86_feature_detected!("gfni"),
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
