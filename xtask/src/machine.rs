//! `cargo xtask machine <target>`: the runner's facts, in the job's summary and its log: its CPU,
//! cores and system, and the CPU features the bench's native C, C++ and Rust builds compile for
//! (`-march=native` and `target-cpu=native`).

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::process::Command;

use crate::ci;

/// The features of an x86-64 CPU the summary says it has or lacks, as /proc/cpuinfo names them.
const X86_64_FLAGS: [&str; 9] = [
    "sse4_2",
    "popcnt",
    "pclmulqdq",
    "aes",
    "sha_ni",
    "avx2",
    "bmi1",
    "bmi2",
    "avx512bw",
];

/// The features of an ARM64 CPU, likewise.
const AARCH64_FEATURES: [&str; 9] = [
    "asimd", "crc32", "pmull", "aes", "sha1", "sha2", "atomics", "sve", "sve2",
];

/// The settings of macOS's `sysctl -a` the summary lists: `hw.optional.` and then one of these.
const MACOS_SETTINGS: [&str; 12] = [
    "arm.FEAT_CRC32",
    "arm.FEAT_PMULL",
    "arm.FEAT_AES",
    "arm.FEAT_SHA256",
    "arm.FEAT_SHA3",
    "arm.FEAT_SVE",
    "arm.FEAT_SME",
    "avx",
    "sse4",
    "aes",
    "bmi",
    "popcnt",
];

/// `cargo xtask machine <target>`.
pub fn task(arguments: &[String]) -> Result<u8, String> {
    let target = arguments.first().map_or("unknown", String::as_str);
    let mut text = format!("### The machine ({target})\n\n");
    match target {
        "linux-x86_64" => {
            let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
            let model = cpuinfo
                .lines()
                .find(|line| line.contains("model name"))
                .and_then(|line| line.split_once(':'))
                .map_or("", |(_, model)| model.strip_prefix(' ').unwrap_or(model));
            let _ = writeln!(text, "{model}, {} vCPUs, kernel {}\n", cores(), kernel());
            text.push_str(&features(starting(&cpuinfo, "flags"), &X86_64_FLAGS));
            let _ = writeln!(text, "- AVX-512: {}", avx512(&cpuinfo));
        }
        "linux-aarch64" => {
            let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
            let _ = writeln!(text, "ARM64, {} vCPUs, kernel {}\n", cores(), kernel());
            text.push_str(&features(starting(&cpuinfo, "Features"), &AARCH64_FEATURES));
        }
        _ if target.starts_with("macos-") => macos(&mut text),
        _ if target.starts_with("windows-") => windows(&mut text),
        _ => text.push_str(&ci::printed("uname", &["-a"])),
    }
    ci::print_and_summarize(&text)?;
    Ok(0)
}

/// The threads this process may run at once, as `nproc` counts them.
fn cores() -> String {
    std::thread::available_parallelism().map_or_else(|_| "?".to_owned(), |n| n.to_string())
}

/// The Linux kernel's release, as `uname -r` prints it.
fn kernel() -> String {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    release.trim().to_owned()
}

/// The first line of `text` that starts with `start`, or nothing.
fn starting<'a>(text: &'a str, start: &str) -> &'a str {
    text.lines()
        .find(|line| line.starts_with(start))
        .unwrap_or_default()
}

/// One line for each of `names`, which says whether `line`, a CPU's list of features, has it as
/// a word.
fn features(line: &str, names: &[&str]) -> String {
    let mut said = String::new();
    for name in names {
        let has = line.split_whitespace().any(|word| word == *name);
        let _ = writeln!(said, "- {name}: {}", if has { "yes" } else { "no" });
    }
    said
}

/// Every AVX-512 feature /proc/cpuinfo names, each once and followed by a space: `avx512` and the
/// letters after it, as `grep -o 'avx512[a-z]*' | sort -u` finds them.
fn avx512(cpuinfo: &str) -> String {
    let mut found = BTreeSet::new();
    for (at, _) in cpuinfo.match_indices("avx512") {
        let after = cpuinfo[at + 6..].trim_start_matches(|c: char| c.is_ascii_lowercase());
        found.insert(&cpuinfo[at..cpuinfo.len() - after.len()]);
    }
    let mut list = String::new();
    for feature in found {
        list.push_str(feature);
        list.push(' ');
    }
    list
}

/// What `sysctl -n <name>` prints, when it knows the name.
fn sysctl(name: &str) -> Option<String> {
    let printed = ci::output_of(Command::new("sysctl").args(["-n", name]))?;
    Some(printed.trim().to_owned())
}

/// macOS: the CPU, its cores, the memory and the system's version, then the CPU's features.
fn macos(text: &mut String) {
    let brand = sysctl("machdep.cpu.brand_string")
        .unwrap_or_else(|| ci::printed("uname", &["-m"]).trim().to_owned());
    let cores = sysctl("hw.ncpu").unwrap_or_default();
    let memory = sysctl("hw.memsize")
        .and_then(|bytes| bytes.parse::<u64>().ok())
        .map_or(0, |bytes| bytes / 1_073_741_824);
    let version = ci::printed("sw_vers", &["-productVersion"]);
    let build = ci::printed("sw_vers", &["-buildVersion"]);
    let _ = writeln!(
        text,
        "{brand}, {cores} vCPUs, {memory} GiB, macOS {} ({})\n",
        version.trim(),
        build.trim()
    );
    let settings = ci::printed("sysctl", &["-a"]);
    let mut listed: Vec<&str> = settings
        .lines()
        .filter(|line| {
            line.strip_prefix("hw.optional.")
                .is_some_and(|rest| MACOS_SETTINGS.iter().any(|name| rest.starts_with(name)))
        })
        .collect();
    listed.sort_unstable();
    text.push_str("```\n");
    for line in listed {
        text.push_str(line);
        text.push('\n');
    }
    text.push_str("```\n");
}

/// Windows: the CPU as the system names it, its cores and the system's version.
fn windows(text: &mut String) {
    let named = |name: &str, otherwise: &str| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| otherwise.to_owned())
    };
    let printed = ci::printed("cmd", &["/c", "ver"]).replace('\r', "");
    let version: Vec<&str> = printed.lines().filter(|line| !line.is_empty()).collect();
    let _ = writeln!(
        text,
        "{}, {} vCPUs, {}",
        named("PROCESSOR_IDENTIFIER", "unknown"),
        named("NUMBER_OF_PROCESSORS", "?"),
        version.join("\n")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A feature is a whole word of the CPU's list, and the summary says yes or no for each.
    #[test]
    fn each_feature_is_a_whole_word_of_the_list() {
        let line = "flags\t\t: fpu vaes avx2 sse4_2";
        assert_eq!(
            features(line, &["aes", "avx2", "sse4_2"]),
            "- aes: no\n- avx2: yes\n- sse4_2: yes\n"
        );
        assert_eq!(starting("model\nflags : a\nflags : b\n", "flags"), "flags : a");
        assert_eq!(starting("model\n", "flags"), "");
    }

    /// AVX-512's features are listed once each, sorted, with the letters after `avx512` and no
    /// more.
    #[test]
    fn avx512_s_features_are_listed_once_each() {
        let cpuinfo = "flags : avx512f avx512bw avx512_vnni avx512vbmi2\nflags : avx512f\n";
        assert_eq!(avx512(cpuinfo), "avx512 avx512bw avx512f avx512vbmi ");
        assert_eq!(avx512("flags : sse2\n"), "");
    }
}
