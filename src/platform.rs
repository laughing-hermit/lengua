//! Detects the machine (OS, CPU, CPU features) and picks its platform profile.
//!
//! A profile holds what differs per OS and CPU: the target triple, the system call table and the
//! details of Linux's ABI ([`crate::syscalls`]), and the linker's flags. The checker's facts of
//! each CPU's instructions and its calling convention are [`crate::isa`]'s.

use serde::Serialize;

use crate::idea::Syscall;
use crate::isa::Isa;
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
    /// Every target lengua exists on. Four more are planned (macOS and Windows, each on x86-64
    /// and ARM64); a new profile is added here, to [`crate::targets::PerCpu`] and to every
    /// `match` on a profile, none of which compiles until it is.
    pub const ALL: [Self; 2] = [Self::LinuxX86_64, Self::LinuxAarch64];

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
        Self::ALL.into_iter().find(|profile| profile.name() == name)
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

    /// The system calls the kernel lets a program make: those its idea allows; `write` and
    /// `exit_group`, which `assert_fail` may always make; for a program whose idea allows
    /// threads, `clone` with exactly [`Self::CLONE_FLAGS`], `futex` and the thread's own `exit`;
    /// and what the launcher needs between confining itself and becoming the program
    /// (`rt_sigprocmask`, `rt_sigaction`, `execve`).
    #[must_use]
    pub fn allowed_syscalls(self, idea: &[Syscall], threads: bool) -> Vec<Allowed> {
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
        let mut allowed: Vec<Allowed> = idea
            .iter()
            .chain(&[Syscall::Write, Syscall::ExitGroup])
            .chain(if threads { &[Syscall::Exit][..] } else { &[] })
            .map(|syscall| Allowed {
                number: self.syscall_number(*syscall),
                pin: match syscall {
                    Syscall::Accept4 => Pin::ListeningSocket,
                    Syscall::Sendto => Pin::NoAddress,
                    Syscall::Ioctl => Pin::Terminal,
                    // The calls that take a descriptor first never take the channel, which the
                    // edge refuses too: a program past the checker can't talk to the broker.
                    Syscall::Read
                    | Syscall::Write
                    | Syscall::Close
                    | Syscall::Lseek
                    | Syscall::Fstat
                    | Syscall::Getdents64
                    | Syscall::Shutdown => Pin::NotChannel,
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
    /// `accept4` on file descriptor 3, the listening socket lengua bound for the program.
    ListeningSocket,
    /// `sendto` on a connection: no address, and no flag but `MSG_NOSIGNAL`, so it can't open a
    /// connection of its own.
    NoAddress,
    /// `ioctl` on standard input, output or error, to read or set the terminal's mode or read
    /// its window's size, and nothing else.
    Terminal,
    /// `sendmsg` or `recvmsg` on descriptor 4,352 alone, lengua's channel to its broker.
    Channel,
    /// Any descriptor but 4,352, which is lengua's: the calls that take one first and are not
    /// pinned to others.
    NotChannel,
}

impl Pin {
    /// How the launcher's argument writes it after the call's number: nothing, `:N`, `:tcp`,
    /// `:listening`, `:noaddress`, `:terminal`, `:channel` or `:notchannel`.
    #[must_use]
    pub fn encode(self) -> String {
        match self {
            Self::Any => String::new(),
            Self::First(argument) => format!(":{argument}"),
            Self::ListeningSocket => ":listening".into(),
            Self::NoAddress => ":noaddress".into(),
            Self::Terminal => ":terminal".into(),
            Self::Channel => ":channel".into(),
            Self::NotChannel => ":notchannel".into(),
        }
    }

    /// Reads what [`Self::encode`] wrote after the colon.
    #[must_use]
    pub fn decode(text: &str) -> Option<Self> {
        match text {
            "listening" => Some(Self::ListeningSocket),
            "noaddress" => Some(Self::NoAddress),
            "terminal" => Some(Self::Terminal),
            "channel" => Some(Self::Channel),
            "notchannel" => Some(Self::NotChannel),
            argument => argument.parse().ok().map(Self::First),
        }
    }
}

/// CPU instruction-set extensions beyond what every x86-64 CPU has, SSE2. Programs may use an
/// extension's instructions only when this CPU has it. What a target's programs may count on
/// every machine having is its baseline, [`CpuFeatures::baseline`].
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
    /// AES rounds on 128-bit lanes (`aesenc`, `aesdec`, …), and on AArch64 its AES extension
    /// (`aese`, `aesd`, `aesmc`, `aesimc`).
    pub aes: bool,
    /// AES rounds on 256-bit and 512-bit registers (`vaesenc ymm0, …`).
    pub vaes: bool,
    /// SHA-1 and SHA-256 rounds (`sha256rnds2`, …), and on AArch64 its SHA2 extension
    /// (`sha1c`, `sha256h` and the rest).
    pub sha: bool,
    /// Carry-less multiplication (`pclmulqdq`), for CRCs and hashes.
    pub pclmulqdq: bool,
    /// Carry-less multiplication on 256-bit and 512-bit registers.
    pub vpclmulqdq: bool,
    /// Galois-field byte operations (`gf2p8affineqb`, `gf2p8mulb`).
    pub gfni: bool,
    /// AArch64's CRC-32 instructions (`crc32cx` and the rest), optional before ARMv8.1.
    pub crc: bool,
    /// AArch64's carry-less multiply of doublewords (`pmull v0.1q, v1.1d, v2.1d`), part of its
    /// crypto extension; the byte form is in every ARM64 CPU.
    pub pmull: bool,
    /// AArch64's scalable vectors, SVE: their length in bytes, 16 to 256, or 0 where the CPU
    /// has none. lengua's model holds up to [`SVE_MAX_BYTES`], so a longer one is none to it.
    pub sve: u16,
}

/// The longest SVE vector lengua's model holds, in bytes: 512 bits, as an x86-64 zmm register
/// is. Its rules hold every SVE access to this width, so a program fits every CPU's length.
pub const SVE_MAX_BYTES: u16 = 64;

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
            crc: true,
            pmull: true,
            sve: SVE_MAX_BYTES,
        }
    }

    /// The baseline of a CPU, the same on every system it runs on: the extensions every machine of
    /// `isa` has, and so all that a program lengua ships for it may rely on. An instruction
    /// beyond it belongs to a fast tier, a program of its own beside a baseline sibling that meets
    /// the very same idea, which an agent picks only where the machine line lists the extension
    /// (DESIGN.md, "One product on every target"). `tests/baseline.rs` holds every shipped
    /// program to it.
    ///
    /// - Linux on x86-64: SSE2, which every x86-64 CPU has, with SSE3, SSSE3, SSE4.1, SSE4.2 and
    ///   POPCNT. Windows 11 24H2 requires a CPU with SSE4.2 and POPCNT, and every GitHub runner
    ///   has them. BMI1, BMI2, LZCNT, AVX and what follows are not in it.
    /// - Linux on AArch64: ARMv8.0-A with NEON and floating point, and nothing optional: no
    ///   CRC-32, PMULL, AES, SHA-1 or SHA-2, atomics (LSE) or SVE.
    ///
    /// The owner decided both sets and may change them here. A machine that lacks its target's
    /// baseline is outside what lengua is written for.
    #[must_use]
    pub fn baseline(isa: Isa) -> Self {
        match isa {
            Isa::X86_64 => Self {
                popcnt: true,
                sse3: true,
                ssse3: true,
                sse4_1: true,
                sse4_2: true,
                ..Self::default()
            },
            Isa::Aarch64 => Self::default(),
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
    /// The CPU features main.s may use on this target: this computer's, or under QEMU the
    /// emulated CPU's, which has AArch64's CRC-32 instructions and its carry-less multiply.
    #[must_use]
    pub fn features(&self, machine: &Machine) -> CpuFeatures {
        match self.emulator {
            None => machine.features,
            Some(_) => CpuFeatures {
                crc: true,
                pmull: true,
                aes: true,
                sha: true,
                sve: emulated_sve_bytes(),
                ..CpuFeatures::default()
            },
        }
    }

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

/// The CPU's model, as the system names it, read once: a process's CPU doesn't change, and on
/// macOS and Windows reading it starts a program.
fn cpu_model() -> Option<String> {
    static MODEL: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    MODEL.get_or_init(read_cpu_model).clone()
}

/// Linux names the CPU in /proc/cpuinfo.
#[cfg(target_os = "linux")]
fn read_cpu_model() -> Option<String> {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    model_name_from_cpuinfo(&cpuinfo)
}

/// macOS names the CPU in sysctl's `machdep.cpu.brand_string`, such as `Apple M2 Pro`.
#[cfg(target_os = "macos")]
fn read_cpu_model() -> Option<String> {
    let output = std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let name = String::from_utf8(output.stdout).ok()?;
    let name = name.trim();
    (output.status.success() && !name.is_empty()).then(|| name.to_owned())
}

/// Windows names the CPU in its registry's `ProcessorNameString`, which `reg query` prints.
#[cfg(windows)]
fn read_cpu_model() -> Option<String> {
    let output = std::process::Command::new("reg")
        .args([
            "query",
            r"HKLM\HARDWARE\DESCRIPTION\System\CentralProcessor\0",
            "/v",
            "ProcessorNameString",
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    processor_name_from_reg(&String::from_utf8_lossy(&output.stdout))
}

/// Another system's CPU goes unnamed.
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
const fn read_cpu_model() -> Option<String> {
    None
}

/// The name `reg query … /v ProcessorNameString` prints after the value's type, on a line such as
/// `    ProcessorNameString    REG_SZ    AMD EPYC 7763 64-Core Processor`.
#[cfg(any(windows, test))]
fn processor_name_from_reg(printed: &str) -> Option<String> {
    printed.lines().find_map(|line| {
        let (_, value) = line.split_once("REG_SZ")?;
        let name = value.trim();
        (!name.is_empty()).then(|| name.to_owned())
    })
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
        crc: false,
        pmull: false,
        sve: 0,
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn cpu_features() -> CpuFeatures {
    CpuFeatures {
        crc: has_crc(),
        pmull: has_pmull(),
        aes: has_aes(),
        sha: has_sha2(),
        sve: sve_bytes(),
        ..CpuFeatures::default()
    }
}

/// This ARM64 CPU's SVE vector length in bytes, as the kernel gives this process and its
/// children, or 0 where it has none or a length past the model's.
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn sve_bytes() -> u16 {
    if !std::arch::is_aarch64_feature_detected!("sve") {
        return 0;
    }
    rustix::thread::sve_vector_length_configuration()
        .ok()
        .and_then(|config| u16::try_from(config.vector_length_in_bytes).ok())
        .filter(|bytes| *bytes <= SVE_MAX_BYTES)
        .unwrap_or(0)
}

/// No other CPU has SVE, and no other system tells its length yet: Apple's CPUs have none, and
/// Windows's length is a todo item for round 6.
#[cfg(not(any(
    target_arch = "x86_64",
    all(target_os = "linux", target_arch = "aarch64")
)))]
const fn sve_bytes() -> u16 {
    0
}

/// The SVE vector length QEMU gives the AArch64 programs it runs here, in bytes, or 0 where it
/// can't be told or is past the model's: measured once by running a program of four
/// instructions, `cntb x0`, `lsr x0, x0, #4`, and an exit with x0, which says it in 16 bytes.
#[cfg(target_os = "linux")]
fn emulated_sve_bytes() -> u16 {
    static FOUND: std::sync::OnceLock<u16> = std::sync::OnceLock::new();
    *FOUND.get_or_init(|| {
        let Ok(folder) = tempfile::tempdir() else {
            return 0;
        };
        let path = folder.path().join("vector-length");
        let written = std::fs::write(&path, sve_probe()).and_then(|()| {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
        });
        if written.is_err() {
            return 0;
        }
        std::process::Command::new(&path)
            .env_clear()
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok()
            .and_then(|status| status.code())
            .and_then(|sixteens| u16::try_from(sixteens).ok())
            .map(|sixteens| sixteens * 16)
            .filter(|bytes| (16..=SVE_MAX_BYTES).contains(bytes))
            .unwrap_or(0)
    })
}

/// QEMU's user mode, which runs another CPU's programs, is Linux's alone.
#[cfg(not(target_os = "linux"))]
const fn emulated_sve_bytes() -> u16 {
    0
}

/// A static AArch64 Linux executable that exits with its SVE vector length over 16: an ELF
/// header, one segment, and the four instructions.
fn sve_probe() -> Vec<u8> {
    const BASE: u64 = 0x40_0000;
    const CODE_AT: u64 = 64 + 56;
    let code: [u32; 4] = [0x0420_e3e0, 0xd344_fc00, 0xd280_0ba8, 0xd400_0001];
    let size = CODE_AT + 16;
    let mut elf = Vec::with_capacity(136);
    elf.extend_from_slice(b"\x7fELF");
    elf.extend_from_slice(&[2, 1, 1, 0]);
    elf.extend_from_slice(&[0; 8]);
    elf.extend_from_slice(&2_u16.to_le_bytes());
    elf.extend_from_slice(&183_u16.to_le_bytes());
    elf.extend_from_slice(&1_u32.to_le_bytes());
    elf.extend_from_slice(&(BASE + CODE_AT).to_le_bytes());
    elf.extend_from_slice(&64_u64.to_le_bytes());
    elf.extend_from_slice(&0_u64.to_le_bytes());
    elf.extend_from_slice(&0_u32.to_le_bytes());
    for half in [64_u16, 56, 1, 0, 0, 0] {
        elf.extend_from_slice(&half.to_le_bytes());
    }
    elf.extend_from_slice(&1_u32.to_le_bytes());
    elf.extend_from_slice(&5_u32.to_le_bytes());
    for word in [0, BASE, BASE, size, size, 0x1000_u64] {
        elf.extend_from_slice(&word.to_le_bytes());
    }
    for instruction in code {
        elf.extend_from_slice(&instruction.to_le_bytes());
    }
    elf
}

/// Whether this ARM64 CPU has the CRC-32 instructions.
#[cfg(target_arch = "aarch64")]
fn has_crc() -> bool {
    std::arch::is_aarch64_feature_detected!("crc")
}

/// No other CPU has AArch64's CRC-32 instructions.
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
const fn has_crc() -> bool {
    false
}

/// Whether this ARM64 CPU multiplies doublewords carry-less, `pmull`'s 1Q form.
#[cfg(target_arch = "aarch64")]
fn has_pmull() -> bool {
    std::arch::is_aarch64_feature_detected!("pmull")
}

/// No other CPU has AArch64's `pmull`.
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
const fn has_pmull() -> bool {
    false
}

/// Whether this ARM64 CPU has the AES instructions (`aese` and the rest).
#[cfg(target_arch = "aarch64")]
fn has_aes() -> bool {
    std::arch::is_aarch64_feature_detected!("aes")
}

/// No other CPU has AArch64's AES instructions.
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
const fn has_aes() -> bool {
    false
}

/// Whether this ARM64 CPU has the SHA-1 and SHA-256 instructions (`sha1c`, `sha256h` and the
/// rest), the SHA2 extension.
#[cfg(target_arch = "aarch64")]
fn has_sha2() -> bool {
    std::arch::is_aarch64_feature_detected!("sha2")
}

/// No other CPU has AArch64's SHA instructions.
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
const fn has_sha2() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four-instruction program tells SVE's vector length where AArch64 programs run, under
    /// QEMU or natively: a multiple of 16 bytes the model holds, or 0 where there's no SVE.
    #[test]
    fn the_probe_tells_sves_vector_length() {
        let bytes = emulated_sve_bytes();
        assert!(
            bytes == 0 || (bytes.is_multiple_of(16) && bytes <= SVE_MAX_BYTES),
            "{bytes}"
        );
        let runs_aarch64 = cfg!(target_arch = "aarch64")
            || std::fs::read_to_string("/proc/sys/fs/binfmt_misc/qemu-aarch64")
                .is_ok_and(|entry| entry.starts_with("enabled"));
        if runs_aarch64 && cfg!(target_arch = "x86_64") {
            // QEMU's CPU has SVE.
            assert!(bytes >= 16, "{bytes}");
        }
        assert_eq!(sve_probe().len(), 136);
    }

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

    /// Windows's registry names the CPU after the value's type, padded with spaces.
    #[test]
    fn reads_the_registry_s_processor_name() {
        let printed = "\r\nHKEY_LOCAL_MACHINE\\HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0\
                       \r\n    ProcessorNameString    REG_SZ    Snapdragon 8cx Gen 3  \r\n\r\n";
        assert_eq!(
            processor_name_from_reg(printed).as_deref(),
            Some("Snapdragon 8cx Gen 3")
        );
        assert_eq!(processor_name_from_reg("ERROR: not found\r\n"), None);
        assert_eq!(
            processor_name_from_reg("    ProcessorNameString    REG_SZ    \r\n"),
            None
        );
    }

    /// The baseline in the checker's own names for extensions, which `tests/baseline.rs` asks
    /// about: x86-64's has SSE3 to SSE4.2 and POPCNT and none of what the shipped programs use
    /// beyond them, ARM64's has nothing optional.
    #[test]
    fn the_baseline_is_named_in_the_checkers_extensions() {
        use crate::checker::{aarch64, x86_64};
        let x86 = CpuFeatures::baseline(Isa::X86_64);
        for name in ["SSE3", "SSSE3", "SSE4.1", "SSE4.2", "POPCNT"] {
            assert_eq!(x86_64::cpu_has(name, &x86), Some(true), "{name}");
        }
        for name in [
            "BMI1",
            "BMI2",
            "LZCNT",
            "ADX",
            "MOVBE",
            "AVX",
            "AVX2",
            "FMA",
            "AVX-512F",
            "AVX-512BW",
            "AES-NI",
            "SHA",
            "PCLMULQDQ",
            "GFNI",
        ] {
            assert_eq!(x86_64::cpu_has(name, &x86), Some(false), "{name}");
        }
        let arm = CpuFeatures::baseline(Isa::Aarch64);
        for name in [
            "crc", "pmull", "aes", "sha", "sve16", "sve32", "sve48", "sve64",
        ] {
            assert_eq!(aarch64::cpu_has(name, &arm), Some(false), "{name}");
        }
        assert_eq!(arm, CpuFeatures::default());
    }

    /// This computer has everything its target's baseline names, as every machine of the target
    /// must: the claim the baseline rests on, checked on the hardware each run is on.
    #[test]
    fn this_computer_has_its_targets_baseline() {
        let machine = Machine::detect();
        let Some(profile) = machine.profile else {
            crate::skiplog::note("this computer isn't a target lengua builds for");
            return;
        };
        let json = |features: CpuFeatures| {
            serde_json::to_value(features).expect("features are plain data")
        };
        let have = json(machine.features);
        let floor = json(CpuFeatures::baseline(profile.isa()));
        for (name, needed) in floor.as_object().expect("features are an object") {
            if *needed == serde_json::Value::Bool(true) {
                assert_eq!(have[name], serde_json::Value::Bool(true), "{name}");
            }
        }
    }
}
