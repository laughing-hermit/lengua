//! Detects the machine (OS, CPU, CPU features) and picks its platform profile.
//!
//! A profile holds what differs per OS and CPU: the target triple, the system call table and the
//! details of Linux's ABI ([`crate::syscalls`]), and the linker's flags. The checker's facts of
//! each CPU's instructions and its calling convention are [`crate::isa`]'s.

use serde::Serialize;

use crate::isa::Isa;
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
            ("linux", "aarch64") => Some(Self::LinuxAarch64),
            _ => None,
        }
    }

    /// The profile a short name, such as `linux-aarch64`, names.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::LinuxX86_64, Self::LinuxAarch64]
            .into_iter()
            .find(|profile| profile.name() == name)
    }

    /// The CPU whose instructions its programs are.
    #[must_use]
    pub const fn isa(self) -> Isa {
        match self {
            Self::LinuxX86_64 => Isa::X86_64,
            Self::LinuxAarch64 => Isa::Aarch64,
        }
    }

    /// The CPU's name, as `std::env::consts::ARCH` and QEMU name it.
    #[must_use]
    pub const fn cpu(self) -> &'static str {
        match self {
            Self::LinuxX86_64 => "x86_64",
            Self::LinuxAarch64 => "aarch64",
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

/// QEMU's user mode, which runs another CPU's Linux programs on this computer: the kernel hands
/// it each such program it's asked to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Emulator {
    /// What runs them, with its version, such as `qemu-aarch64 10.2.2`.
    pub name: String,
}

/// What a session compiles, checks and runs programs for: this computer's own target, or, at a
/// developer's request, another that QEMU's user mode runs here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The target.
    pub profile: Profile,
    /// QEMU, when this computer runs the target's programs only through it.
    pub emulator: Option<Emulator>,
}

impl Target {
    /// The session's target: this computer's own, unless `requested`, the developer's
    /// `LENGUA_TARGET`, names another that QEMU's user mode runs here. Emulation is for
    /// developing lengua; the target of a session that isn't asked for one is always this
    /// computer's.
    ///
    /// # Errors
    ///
    /// Why the session can't have that target: this computer isn't one lengua builds on, the
    /// name isn't a target, or QEMU isn't set up to run the target's programs.
    pub fn session(machine: &Machine, requested: Option<&str>) -> Result<Self, String> {
        let host = machine.profile.ok_or_else(|| {
            format!(
                "{}-{} isn't supported yet. lengua builds programs for linux-x86_64 and \
                 linux-aarch64.",
                machine.os, machine.arch
            )
        })?;
        let Some(name) = requested.map(str::trim).filter(|name| !name.is_empty()) else {
            return Ok(Self {
                profile: host,
                emulator: None,
            });
        };
        let profile = Profile::from_name(name).ok_or_else(|| {
            format!(
                "LENGUA_TARGET is {name}, which isn't a target lengua knows: linux-x86_64 or \
                 linux-aarch64"
            )
        })?;
        if profile == host {
            return Ok(Self {
                profile,
                emulator: None,
            });
        }
        let cpu = profile.cpu();
        let entry = std::fs::read_to_string(format!("/proc/sys/fs/binfmt_misc/qemu-{cpu}"))
            .unwrap_or_default();
        let interpreter = registered_interpreter(&entry).ok_or_else(|| {
            format!(
                "LENGUA_TARGET is {name}, which this computer runs only through QEMU's user \
                 mode: install qemu-user-static, whose binfmt_misc entry qemu-{cpu} must be \
                 enabled with flag F"
            )
        })?;
        let version = std::process::Command::new(interpreter)
            .arg("--version")
            .env_clear()
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|text| qemu_version(&text));
        Ok(Self {
            profile,
            emulator: Some(Emulator {
                name: version.map_or_else(
                    || format!("qemu-{cpu}"),
                    |version| format!("qemu-{cpu} {version}"),
                ),
            }),
        })
    }

    /// The target as a machine line names it: `linux-aarch64`, or
    /// `linux-aarch64 (emulated by qemu-aarch64 10.2.2 on linux-x86_64)`.
    #[must_use]
    pub fn describe(&self, machine: &Machine) -> String {
        match &self.emulator {
            None => self.profile.name().to_owned(),
            Some(emulator) => format!(
                "{} (emulated by {} on {}-{})",
                self.profile.name(),
                emulator.name,
                machine.os,
                machine.arch
            ),
        }
    }
}

/// The interpreter a binfmt_misc entry runs, when the entry is enabled and its flags include F:
/// the kernel opened the interpreter when it was registered, so a program confined to its own
/// folder still runs through it.
fn registered_interpreter(entry: &str) -> Option<&str> {
    let mut lines = entry.lines().map(str::trim);
    if lines.next() != Some("enabled") {
        return None;
    }
    let field = |name: &str| {
        entry
            .lines()
            .find_map(|line| line.trim().strip_prefix(name).map(str::trim))
    };
    let interpreter = field("interpreter ")?;
    field("flags:")?
        .contains('F')
        .then_some(interpreter)
        .filter(|interpreter| !interpreter.is_empty())
}

/// The version `qemu-<cpu> --version` says, as `10.2.2`.
fn qemu_version(text: &str) -> Option<String> {
    let line = text.lines().next()?;
    let (_, rest) = line.split_once(" version ")?;
    rest.split_whitespace().next().map(str::to_owned)
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

/// The CPU's model: its `model name`, which x86-64 Linux gives, or else, as on ARM64 Linux,
/// which gives none, the implementer and part numbers that name the design.
fn model_name_from_cpuinfo(cpuinfo: &str) -> Option<String> {
    let field = |name: &str| {
        cpuinfo.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key.trim() == name).then(|| value.trim().to_owned())
        })
    };
    field("model name").or_else(|| {
        let implementer = field("CPU implementer")?;
        let part = field("CPU part")?;
        Some(format!("CPU implementer {implementer}, part {part}"))
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
    fn linux_on_x86_64_and_aarch64_are_hosts() {
        assert_eq!(
            Profile::for_target("linux", "x86_64"),
            Some(Profile::LinuxX86_64)
        );
        assert_eq!(
            Profile::for_target("linux", "aarch64"),
            Some(Profile::LinuxAarch64)
        );
        assert_eq!(Profile::for_target("macos", "aarch64"), None);
        assert_eq!(Profile::for_target("windows", "x86_64"), None);
        for profile in [Profile::LinuxX86_64, Profile::LinuxAarch64] {
            assert_eq!(Profile::from_name(profile.name()), Some(profile));
        }
        assert_eq!(Profile::from_name("macos-aarch64"), None);
    }

    /// The session's target is this computer's unless a developer asks for another, and then
    /// only through a QEMU registered to run it.
    #[test]
    fn a_session_targets_this_computer_unless_asked_otherwise() {
        let machine = |arch: &'static str| Machine {
            os: "linux",
            arch,
            cpu_model: None,
            cores: 1,
            features: CpuFeatures::default(),
            profile: Profile::for_target("linux", arch),
        };
        let x86 = machine("x86_64");
        let native = |profile| Target {
            profile,
            emulator: None,
        };
        assert_eq!(
            Target::session(&x86, None),
            Ok(native(Profile::LinuxX86_64))
        );
        assert_eq!(
            Target::session(&x86, Some(" ")),
            Ok(native(Profile::LinuxX86_64))
        );
        assert_eq!(
            Target::session(&x86, Some("linux-x86_64")),
            Ok(native(Profile::LinuxX86_64))
        );
        assert_eq!(
            Target::session(&machine("aarch64"), Some("linux-aarch64")),
            Ok(native(Profile::LinuxAarch64))
        );
        let unknown = Target::session(&x86, Some("windows-x86_64")).expect_err("not a target");
        assert!(unknown.contains("isn't a target lengua knows"), "{unknown}");
        let riscv = Machine {
            profile: None,
            ..machine("riscv64")
        };
        let unsupported = Target::session(&riscv, None).expect_err("not a host");
        assert!(
            unsupported.contains("linux-riscv64 isn't supported yet"),
            "{unsupported}"
        );
    }

    #[test]
    fn reads_qemus_registration_and_version() {
        let entry = "enabled\ninterpreter /usr/bin/qemu-aarch64-static\nflags: F\noffset 0\n";
        assert_eq!(
            registered_interpreter(entry),
            Some("/usr/bin/qemu-aarch64-static")
        );
        assert_eq!(
            registered_interpreter(&entry.replace("flags: F", "flags: PC")),
            None
        );
        assert_eq!(
            registered_interpreter(&entry.replace("enabled", "disabled")),
            None
        );
        assert_eq!(registered_interpreter(""), None);
        assert_eq!(
            qemu_version("qemu-aarch64 version 10.2.2 (qemu-10.2.2-1.fc44)\nCopyright\n"),
            Some("10.2.2".to_owned())
        );
        assert_eq!(qemu_version("nonsense"), None);
    }

    #[test]
    fn reads_model_name() {
        let cpuinfo = "processor\t: 0\nmodel name\t: Example CPU 9000\nflags\t: fpu\n";
        assert_eq!(
            model_name_from_cpuinfo(cpuinfo).as_deref(),
            Some("Example CPU 9000")
        );
        assert_eq!(model_name_from_cpuinfo("processor\t: 0\n"), None);
        // ARM64 Linux names no model, only the design's numbers.
        let arm = "processor\t: 0\nBogoMIPS\t: 50.00\nCPU implementer\t: 0x41\nCPU \
                   architecture: 8\nCPU variant\t: 0x3\nCPU part\t: 0xd0c\n";
        assert_eq!(
            model_name_from_cpuinfo(arm).as_deref(),
            Some("CPU implementer 0x41, part 0xd0c")
        );
    }
}
