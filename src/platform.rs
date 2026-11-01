//! Detects the machine (OS, CPU, CPU features) and picks its platform profile.
//!
//! A profile holds what differs per OS and CPU: the target triple, the system call table and the
//! details of Linux's ABI ([`crate::syscalls`]), and the linker's flags. The checker's facts of
//! each CPU's instructions and its calling convention are [`crate::isa`]'s.

use serde::Serialize;

use crate::spec::Syscall;
use crate::syscalls;

/// The OS + CPU combinations lengua knows how to build for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Linux on x86-64: ELF executables, direct system calls, System V calling convention.
    LinuxX86_64,
    /// Linux on AArch64: ELF executables, direct system calls, the AAPCS64 calling convention.
    LinuxAarch64,
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
            Self::LinuxAarch64 => "linux-aarch64",
        }
    }

    /// The LLVM target triple passed to the assembler.
    #[must_use]
    pub fn llvm_triple(self) -> &'static str {
        match self {
            Self::LinuxX86_64 => "x86_64-unknown-linux-gnu",
            Self::LinuxAarch64 => "aarch64-unknown-linux-gnu",
        }
    }

    /// The details of Linux's ABI on this CPU that the prover models.
    #[must_use]
    pub const fn linux_abi(self) -> &'static syscalls::LinuxAbi {
        match self {
            Self::LinuxX86_64 => &syscalls::x86_64::ABI,
            Self::LinuxAarch64 => &syscalls::aarch64::ABI,
        }
    }

    /// The system call's name on this CPU, as main.s names its number (`SYS_PPOLL` for a wait
    /// on AArch64, which has no `poll`).
    #[must_use]
    pub fn syscall_name(self, syscall: Syscall) -> &'static str {
        match (syscall, self.linux_abi().wait) {
            (Syscall::Poll, syscalls::Wait::Ppoll) => "ppoll",
            _ => syscall.name(),
        }
    }

    /// The number a program makes this system call with.
    #[must_use]
    pub const fn syscall_number(self, syscall: Syscall) -> i64 {
        match self {
            Self::LinuxX86_64 => syscalls::x86_64::number(syscall),
            Self::LinuxAarch64 => syscalls::aarch64::number(syscall),
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
            Self::LinuxX86_64 => {
                use syscalls::x86_64::{CLONE, EXECVE, FUTEX, RT_SIGACTION, RT_SIGPROCMASK};
                (&[RT_SIGPROCMASK, RT_SIGACTION, EXECVE], CLONE, FUTEX)
            }
            Self::LinuxAarch64 => {
                use syscalls::aarch64::{CLONE, EXECVE, FUTEX, RT_SIGACTION, RT_SIGPROCMASK};
                (&[RT_SIGPROCMASK, RT_SIGACTION, EXECVE], CLONE, FUTEX)
            }
        };
        let mut allowed: Vec<Allowed> = spec
            .iter()
            .chain(&[Syscall::Write, Syscall::ExitGroup])
            .chain(if threads { &[Syscall::Exit][..] } else { &[] })
            .map(|syscall| Allowed {
                number: self.syscall_number(*syscall),
                pin: match syscall {
                    Syscall::Socket => Pin::TcpSocket,
                    Syscall::Accept4 => Pin::ListeningSocket,
                    Syscall::Sendto => Pin::NoAddress,
                    Syscall::Ioctl => Pin::Terminal,
                    _ => Pin::Any,
                },
            })
            .chain(launch.iter().copied().map(Allowed::any))
            .collect();
        if threads {
            allowed.push(Allowed {
                number: clone,
                pin: Pin::First(Self::CLONE_FLAGS),
            });
            allowed.push(Allowed::any(futex));
        }
        allowed.sort_unstable();
        allowed.dedup();
        allowed
    }
}

/// A system call the kernel lets a program make, by number, and what its arguments may be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Allowed {
    /// The system call's number.
    pub number: i64,
    /// What its arguments must be.
    pub pin: Pin,
}

impl Allowed {
    /// The system call with any arguments.
    #[must_use]
    pub fn any(number: i64) -> Self {
        Self {
            number,
            pin: Pin::Any,
        }
    }
}

/// What a system call's arguments must be for the kernel to allow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pin {
    /// Any arguments.
    Any,
    /// Exactly this first argument: `clone` with `parallel_for`'s flags.
    First(u64),
    /// `socket` for TCP over IPv4 or IPv6, and nothing else: no UDP, raw or local sockets.
    TcpSocket,
    /// `accept4` on file descriptor 3, the listening socket lengua bound for the program.
    ListeningSocket,
    /// `sendto` on a connection: no address, and no flag but `MSG_NOSIGNAL`, so it can't open a
    /// connection of its own.
    NoAddress,
    /// `ioctl` on standard input, output or error, to read or set the terminal's mode or read
    /// its window's size, and nothing else.
    Terminal,
}

impl Pin {
    /// How the launcher's argument writes it after the call's number: nothing, `:N`, `:tcp`,
    /// `:listening`, `:noaddress` or `:terminal`.
    #[must_use]
    pub fn encode(self) -> String {
        match self {
            Self::Any => String::new(),
            Self::First(argument) => format!(":{argument}"),
            Self::TcpSocket => ":tcp".into(),
            Self::ListeningSocket => ":listening".into(),
            Self::NoAddress => ":noaddress".into(),
            Self::Terminal => ":terminal".into(),
        }
    }

    /// Reads what [`Self::encode`] wrote after the colon.
    #[must_use]
    pub fn decode(text: &str) -> Option<Self> {
        match text {
            "tcp" => Some(Self::TcpSocket),
            "listening" => Some(Self::ListeningSocket),
            "noaddress" => Some(Self::NoAddress),
            "terminal" => Some(Self::Terminal),
            argument => argument.parse().ok().map(Self::First),
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
    fn the_aarch64_profile_has_its_own_names_and_calls() {
        let arm = Profile::LinuxAarch64;
        assert_eq!(arm.name(), "linux-aarch64");
        assert_eq!(arm.llvm_triple(), "aarch64-unknown-linux-gnu");
        assert_eq!(arm.syscall_number(Syscall::Read), 63);
        assert_eq!(arm.syscall_name(Syscall::Poll), "ppoll");
        assert_eq!(arm.syscall_number(Syscall::Poll), 73);
        assert_eq!(Profile::LinuxX86_64.syscall_name(Syscall::Poll), "poll");
        let launcher = arm.allowed_syscalls(&[], true);
        for number in [135, 134, 221, 220, 98, 64, 94, 93] {
            assert!(
                launcher.iter().any(|allowed| allowed.number == number),
                "{number}: {launcher:?}"
            );
        }
    }

    #[test]
    fn only_linux_x86_64_is_a_host_yet() {
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
