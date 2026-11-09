//! Detects the machine lengua runs on: its system and CPU, the CPU's model, its cores and the
//! vector extensions Cranelift compiles WebAssembly's 128-bit SIMD to there, and its caches, which
//! the `machine` line and `lengua-mcp doctor` name.

/// The CPU's vector extensions beyond what every machine of its kind has, which say how wide the
/// vectors go that Cranelift compiles WebAssembly's SIMD128 to here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CpuFeatures {
    /// SSE3.
    pub sse3: bool,
    /// SSSE3.
    pub ssse3: bool,
    /// SSE4.1.
    pub sse4_1: bool,
    /// SSE4.2.
    pub sse4_2: bool,
    /// AVX: VEX-encoded 128-bit forms of the SSE instructions.
    pub avx: bool,
    /// AVX2: 256-bit integer vectors.
    pub avx2: bool,
    /// AVX-512 Foundation: 512-bit vectors and mask registers.
    pub avx512f: bool,
    /// AArch64's scalable vectors, SVE: their length in bytes, 16 to [`SVE_MAX_BYTES`], or 0
    /// where the CPU has none or a longer one.
    pub sve: u16,
}

/// The longest SVE vector the machine line names, in bytes: 512 bits, as an x86-64 zmm register
/// is.
pub const SVE_MAX_BYTES: u16 = 64;

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
    /// Detected vector extensions.
    pub features: CpuFeatures,
    /// The CPU's caches as the system describes them, for the first CPU it numbers (on macOS,
    /// its performance cores), the nearest first: a level the system doesn't describe isn't
    /// here, never guessed.
    pub caches: Vec<Cache>,
}

impl Machine {
    /// Detects the current machine.
    #[must_use]
    pub fn detect() -> Self {
        Self {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            cpu_model: cpu_model(),
            cores: std::thread::available_parallelism().map_or(1, std::num::NonZero::get),
            features: cpu_features(),
            caches: caches(),
        }
    }
}

/// What one of the CPU's caches holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CacheHolds {
    /// Instructions.
    Instructions,
    /// Data.
    Data,
    /// Both.
    Both,
}

/// One of the CPU's caches, as the system describes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cache {
    /// Its level: 1 for the nearest.
    pub level: u8,
    /// What it holds.
    pub holds: CacheHolds,
    /// Its size, in bytes.
    pub bytes: u64,
    /// The size of its lines, in bytes, when the system says it.
    pub line: Option<u64>,
}

/// The caches this computer's system describes, read once.
fn caches() -> Vec<Cache> {
    static FOUND: std::sync::OnceLock<Vec<Cache>> = std::sync::OnceLock::new();
    FOUND.get_or_init(system_caches).clone()
}

/// Linux describes each of a CPU's caches in a folder of its own under `/sys`.
#[cfg(target_os = "linux")]
fn system_caches() -> Vec<Cache> {
    linux_caches(std::path::Path::new("/sys/devices/system/cpu/cpu0/cache"))
}

/// macOS names them with `sysctl`.
#[cfg(target_os = "macos")]
fn system_caches() -> Vec<Cache> {
    mac_caches(sysctl)
}

/// Windows describes them through `GetLogicalProcessorInformationEx`, which lengua-os calls.
#[cfg(windows)]
fn system_caches() -> Vec<Cache> {
    let Ok(described) = lengua_os::caches() else {
        return Vec::new();
    };
    described
        .into_iter()
        .filter_map(|cache| {
            let holds = match cache.kind {
                0 => CacheHolds::Both,
                1 => CacheHolds::Instructions,
                2 => CacheHolds::Data,
                _ => return None,
            };
            Some(Cache {
                level: cache.level,
                holds,
                bytes: u64::from(cache.bytes),
                line: (cache.line > 0).then_some(u64::from(cache.line)),
            })
        })
        .filter(|cache| cache.level > 0 && cache.bytes > 0)
        .collect()
}

/// Another system describes none that lengua reads.
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn system_caches() -> Vec<Cache> {
    Vec::new()
}

/// The caches `folder`, one CPU's `cache` folder in Linux's `/sys`, describes: each `index*` in
/// it with its `level`, `type` and `size`, and `coherency_line_size` for its lines, the nearest
/// first. One that doesn't say its level, kind or size isn't there.
fn linux_caches(folder: &std::path::Path) -> Vec<Cache> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut caches: Vec<Cache> = entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("index"))
        .filter_map(|entry| {
            let path = entry.path();
            let read = |name: &str| {
                std::fs::read_to_string(path.join(name))
                    .ok()
                    .map(|text| text.trim().to_owned())
            };
            let level = read("level")?
                .parse::<u8>()
                .ok()
                .filter(|level| *level > 0)?;
            let holds = match read("type")?.as_str() {
                "Data" => CacheHolds::Data,
                "Instruction" => CacheHolds::Instructions,
                "Unified" => CacheHolds::Both,
                _ => return None,
            };
            let bytes = cache_size(&read("size")?)?;
            let line = read("coherency_line_size")
                .and_then(|text| text.parse::<u64>().ok())
                .filter(|line| *line > 0);
            Some(Cache {
                level,
                holds,
                bytes,
                line,
            })
        })
        .collect();
    caches.sort_by_key(|cache| (cache.level, cache.holds));
    caches.dedup_by_key(|cache| (cache.level, cache.holds));
    caches
}

/// A cache's size as Linux writes it: `48K`, `2048K`, `32M`, or a number of bytes.
fn cache_size(text: &str) -> Option<u64> {
    let text = text.trim();
    let (digits, scale) = match text.chars().next_back()? {
        'K' | 'k' => (&text[..text.len() - 1], 1_u64 << 10),
        'M' | 'm' => (&text[..text.len() - 1], 1_u64 << 20),
        'G' | 'g' => (&text[..text.len() - 1], 1_u64 << 30),
        _ => (text, 1),
    };
    digits
        .trim()
        .parse::<u64>()
        .ok()?
        .checked_mul(scale)
        .filter(|bytes| *bytes > 0)
}

/// The caches macOS's `sysctl` names, `read` giving the number of each name it has: the
/// performance cores' (`hw.perflevel0.*`) where the CPU has kinds of cores, else the CPU's own
/// (`hw.l1dcachesize` and the rest), each with `hw.cachelinesize` for its lines.
#[cfg(any(target_os = "macos", test))]
fn mac_caches(read: impl Fn(&str) -> Option<u64>) -> Vec<Cache> {
    let line = read("hw.cachelinesize").filter(|line| *line > 0);
    [
        (1, CacheHolds::Data, "l1dcachesize"),
        (2, CacheHolds::Both, "l2cachesize"),
        (3, CacheHolds::Both, "l3cachesize"),
    ]
    .into_iter()
    .filter_map(|(level, holds, name)| {
        let bytes = read(&format!("hw.perflevel0.{name}"))
            .or_else(|| read(&format!("hw.{name}")))
            .filter(|bytes| *bytes > 0)?;
        Some(Cache {
            level,
            holds,
            bytes,
            line,
        })
    })
    .collect()
}

/// The number `sysctl` gives for `name`, when it has one.
#[cfg(target_os = "macos")]
fn sysctl(name: &str) -> Option<u64> {
    let output = std::process::Command::new("/usr/sbin/sysctl")
        .arg("-n")
        .arg(name)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()?.trim().parse().ok()
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
        sse3: std::arch::is_x86_feature_detected!("sse3"),
        ssse3: std::arch::is_x86_feature_detected!("ssse3"),
        sse4_1: std::arch::is_x86_feature_detected!("sse4.1"),
        sse4_2: std::arch::is_x86_feature_detected!("sse4.2"),
        avx: std::arch::is_x86_feature_detected!("avx"),
        avx2: std::arch::is_x86_feature_detected!("avx2"),
        avx512f: std::arch::is_x86_feature_detected!("avx512f"),
        sve: 0,
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn cpu_features() -> CpuFeatures {
    CpuFeatures {
        sve: sve_bytes(),
        ..CpuFeatures::default()
    }
}

/// This ARM64 CPU's SVE vector length in bytes, as the kernel gives this process and its
/// children, or 0 where it has none or a length past [`SVE_MAX_BYTES`].
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
/// Windows doesn't say it.
#[cfg(not(any(
    target_arch = "x86_64",
    all(target_os = "linux", target_arch = "aarch64")
)))]
const fn sve_bytes() -> u16 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Linux's folder for each cache is read for its level, kind, size and line size, the nearest
    /// first; one that doesn't say its level, kind or size is left out, and so is a folder that
    /// isn't a cache's.
    #[test]
    fn linux_describes_each_cache_in_a_folder_of_its_own() {
        let root = tempfile::tempdir().expect("a folder");
        let write = |index: &str, files: &[(&str, &str)]| {
            let folder = root.path().join(index);
            std::fs::create_dir_all(&folder).expect("a cache's folder");
            for (name, text) in files {
                std::fs::write(folder.join(name), format!("{text}\n")).expect("a cache's file");
            }
        };
        let line = ("coherency_line_size", "64");
        write(
            "index0",
            &[("level", "1"), ("type", "Data"), ("size", "48K"), line],
        );
        write(
            "index1",
            &[
                ("level", "1"),
                ("type", "Instruction"),
                ("size", "32K"),
                line,
            ],
        );
        write(
            "index2",
            &[("level", "2"), ("type", "Unified"), ("size", "2048K"), line],
        );
        write(
            "index3",
            &[("level", "3"), ("type", "Unified"), ("size", "32M")],
        );
        write("index4", &[("level", "4"), ("type", "Unified")]);
        write("power", &[("level", "9"), ("type", "Data"), ("size", "1K")]);
        let cache = |level, holds, bytes, line| Cache {
            level,
            holds,
            bytes,
            line,
        };
        assert_eq!(
            linux_caches(root.path()),
            [
                cache(1, CacheHolds::Instructions, 32 << 10, Some(64)),
                cache(1, CacheHolds::Data, 48 << 10, Some(64)),
                cache(2, CacheHolds::Both, 2 << 20, Some(64)),
                cache(3, CacheHolds::Both, 32 << 20, None),
            ]
        );
        assert!(linux_caches(&root.path().join("missing")).is_empty());
        assert_eq!(cache_size("48K"), Some(48 << 10));
        assert_eq!(cache_size("512"), Some(512));
        assert_eq!(cache_size("0K"), None);
        assert_eq!(cache_size("K"), None);
    }

    /// macOS's names are read for the performance cores where it has them, for the CPU's own
    /// otherwise, each level with the one line size; a level it doesn't name is left out.
    #[test]
    fn macos_names_its_caches_with_sysctl() {
        fn named(pairs: &[(&str, u64)], name: &str) -> Option<u64> {
            pairs
                .iter()
                .find(|(known, _)| *known == name)
                .map(|(_, value)| *value)
        }
        let apple = [
            ("hw.cachelinesize", 128),
            ("hw.perflevel0.l1dcachesize", 131_072),
            ("hw.l1dcachesize", 65_536),
            ("hw.perflevel0.l2cachesize", 16_777_216),
            ("hw.l2cachesize", 4_194_304),
        ];
        let cache = |level, holds, bytes, line| Cache {
            level,
            holds,
            bytes,
            line,
        };
        assert_eq!(
            mac_caches(|name| named(&apple, name)),
            [
                cache(1, CacheHolds::Data, 131_072, Some(128)),
                cache(2, CacheHolds::Both, 16_777_216, Some(128)),
            ]
        );
        let intel = [
            ("hw.cachelinesize", 64),
            ("hw.l1dcachesize", 32_768),
            ("hw.l2cachesize", 262_144),
            ("hw.l3cachesize", 8_388_608),
        ];
        assert_eq!(
            mac_caches(|name| named(&intel, name)),
            [
                cache(1, CacheHolds::Data, 32_768, Some(64)),
                cache(2, CacheHolds::Both, 262_144, Some(64)),
                cache(3, CacheHolds::Both, 8_388_608, Some(64)),
            ]
        );
        assert!(mac_caches(|_| None).is_empty());
    }

    /// This computer's caches are as its system describes them: macOS, and Linux and Windows on
    /// x86-64, always describe an L1 data cache, which detection finds; elsewhere the test notes
    /// when the system describes none, which is then left out, never guessed.
    #[test]
    fn this_computer_s_caches_are_what_its_system_describes() {
        let machine = Machine::detect();
        let data = machine
            .caches
            .iter()
            .find(|cache| cache.level == 1 && cache.holds == CacheHolds::Data);
        let described = cfg!(any(target_os = "macos", target_arch = "x86_64"));
        match data {
            Some(cache) => assert!(cache.bytes >= 1024, "{:?}", machine.caches),
            None if described => panic!("no L1 data cache in {:?}", machine.caches),
            None => crate::skiplog::note("the system describes no L1 data cache"),
        }
        assert_eq!(Machine::detect().caches, machine.caches);
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
}
