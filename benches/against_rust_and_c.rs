//! Measures lengua programs against the same programs written in C, C++ and Rust, on this
//! machine, and writes BENCHMARKS.md: `cargo bench --bench against_rust_and_c`.
//!
//! Every version of a program gets the same arguments, input and files, must give the same output,
//! and is measured the same way: user-space instructions and cycles and CPU time from the CPU's
//! counters, started at the program's own exec, plus wall-clock time, peak memory and file size.
//! C, C++ and Rust are built with every optimization for this CPU, and linked the way they ship.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use lengua::platform::{Machine, Profile};
use lengua::toolchain::Toolchain;
use perf_event::events::{Event, Hardware, Software};
use perf_event::{Builder, Counter, ReadFormat};

/// Runs per version, after warming up; the report gives each measure's median.
const RUNS: usize = 41;
const WARM_UP: usize = 3;

/// The flags each compiler gets: everything for speed on this CPU.
const RUSTC_FLAGS: [&str; 12] = [
    "--edition",
    "2024",
    "-C",
    "opt-level=3",
    "-C",
    "target-cpu=native",
    "-C",
    "codegen-units=1",
    "-C",
    "lto=fat",
    "-C",
    "panic=abort",
];
const GCC_FLAGS: [&str; 4] = ["-O3", "-march=native", "-flto", "-s"];

/// The large workloads' input: 64 MiB, so the work outweighs starting and ending a process.
const LARGE: usize = 64 * 1024 * 1024;

/// One program and what to run it on.
struct Workload {
    name: &'static str,
    about: &'static str,
    /// lengua's versions: a label and a main.s, from the repository root.
    lengua: &'static [(&'static str, &'static str)],
    /// Versions in other languages: a label and a source file in benches/programs/<name>/.
    others: &'static [(&'static str, &'static str)],
    args: Vec<String>,
    stdin: Vec<u8>,
    files: Vec<(&'static str, Vec<u8>)>,
    /// The file it writes, when its output is a file.
    output_file: Option<&'static str>,
}

/// One built version of a workload.
struct Version {
    label: String,
    executable: PathBuf,
    lengua: bool,
}

/// The medians of one version's runs.
#[derive(Debug, Default)]
struct Measures {
    instructions: Option<u64>,
    cycles: Option<u64>,
    cpu_time: Option<Duration>,
    wall_time: Duration,
    peak_kib: Option<u64>,
    size: u64,
}

/// Text like what people shout: letters in both cases, digits, punctuation, newlines and some
/// non-ASCII letters, from a fixed seed.
fn text(bytes: usize) -> Vec<u8> {
    const WORDS: [&str; 12] = [
        "lengua", "Assembly", "proves", "every", "input", "café", "fast", "Rust", "C", "42",
        "naïve", "x86-64",
    ];
    let mut state: u64 = 0x2545_f491_4f6c_dd1d;
    let mut text = Vec::with_capacity(bytes + 16);
    while text.len() < bytes {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let word = WORDS[usize::try_from(state % 12).unwrap_or(0)];
        text.extend_from_slice(word.as_bytes());
        text.push(match state >> 60 {
            0 => b'\n',
            1 => b',',
            2 => b'.',
            _ => b' ',
        });
    }
    // Whole characters only: cut at a byte below 0x80.
    let mut end = bytes;
    while end > 0 && text[end - 1] >= 0x80 {
        end -= 1;
    }
    text.truncate(end);
    text
}

fn workloads() -> Vec<Workload> {
    let mebibyte = text(1_048_576);
    let large = text(LARGE);
    vec![
        Workload {
            name: "add",
            about: "Parses two numbers from its arguments and prints their sum: almost all of \
                    it is starting up and exiting.",
            lengua: &[("lengua", "examples/add/main.s")],
            others: &[("Rust", "main.rs"), ("C", "main.c"), ("C++", "main.cpp")],
            args: vec!["123456789".into(), "-987654".into()],
            stdin: Vec::new(),
            files: Vec::new(),
            output_file: None,
        },
        Workload {
            name: "hypotenuse",
            about: "Prints the hypotenuse of two decimal sides given as arguments, with three \
                    decimals: each side divided into a double, a square root, and the result \
                    rounded to thousandths. Like add, it's mostly starting up and exiting: here \
                    C and C++ load the math library too.",
            lengua: &[("lengua", "examples/hypotenuse/main.s")],
            others: &[("Rust", "main.rs"), ("C", "main.c"), ("C++", "main.cpp")],
            args: vec!["123456.789".into(), "98765.432".into()],
            stdin: Vec::new(),
            files: Vec::new(),
            output_file: None,
        },
        Workload {
            name: "upcase",
            about: "Copies a 1 MiB text file to another with its ASCII letters capitalized: \
                    file reads and writes, and one pass over the bytes. lengua's versions are \
                    the example, 16 bytes at a time, and the same program 64 bytes at a time \
                    with AVX-512.",
            lengua: &[
                ("lengua", "examples/upcase/main.s"),
                ("lengua, AVX-512", "benches/programs/upcase/avx512.s"),
            ],
            others: &[("Rust", "main.rs"), ("C", "main.c"), ("C++", "main.cpp")],
            args: vec!["in.txt".into(), "out.txt".into()],
            stdin: Vec::new(),
            files: vec![("in.txt", mebibyte.clone())],
            output_file: Some("out.txt"),
        },
        Workload {
            name: "shout",
            about: "Capitalizes 1 MiB of standard input to standard output. lengua's versions \
                    are the fork-join template, 16 bytes at a time on 16 threads, and the same \
                    program 64 bytes at a time with AVX-512, on 16 threads and on one; Rust is \
                    shown both plain and with scoped threads split the same way.",
            lengua: &[
                ("lengua", "plugin/skills/compile/assets/parallel-template.s"),
                ("lengua, AVX-512", "benches/programs/shout/avx512.s"),
                (
                    "lengua, AVX-512, one thread",
                    "benches/programs/shout/avx512-one.s",
                ),
            ],
            others: &[
                ("Rust", "main.rs"),
                ("Rust, threads", "threads.rs"),
                ("C", "main.c"),
                ("C++", "main.cpp"),
            ],
            args: Vec::new(),
            stdin: mebibyte,
            files: Vec::new(),
            output_file: None,
        },
        Workload {
            name: "wc",
            about: "Counts the lines and words of 64 MiB on standard input, as `wc -lw` does. \
                    C, C++ and Rust compare each byte with the one before it, which compilers \
                    can vectorize. lengua's versions take 64 bytes a step with AVX-512 masks: \
                    the skill's example, and the program Claude Code wrote through lengua from \
                    a plain request in an interactive session.",
            lengua: &[
                ("lengua", "plugin/skills/compile/assets/fast/wc/main.s"),
                ("lengua, by Claude Code", "examples/count/main.s"),
            ],
            others: &[("Rust", "main.rs"), ("C", "main.c"), ("C++", "main.cpp")],
            args: Vec::new(),
            stdin: large.clone(),
            files: Vec::new(),
            output_file: None,
        },
        Workload {
            name: "histogram",
            about: "Counts each byte value of 64 MiB on standard input. Every version spreads \
                    neighboring bytes over four tables.",
            lengua: &[(
                "lengua",
                "plugin/skills/compile/assets/fast/histogram/main.s",
            )],
            others: &[("Rust", "main.rs"), ("C", "main.c"), ("C++", "main.cpp")],
            args: Vec::new(),
            stdin: large.clone(),
            files: Vec::new(),
            output_file: None,
        },
        Workload {
            name: "crc32c",
            about: "The CRC-32C of 64 MiB on standard input. C, C++ and Rust use the CPU's \
                    `crc32` instruction 8 bytes at a time, one chain that waits on itself; \
                    lengua's version runs three chains side by side and joins them with \
                    carry-less multiplies.",
            lengua: &[("lengua", "plugin/skills/compile/assets/fast/crc32c/main.s")],
            others: &[("Rust", "main.rs"), ("C", "main.c"), ("C++", "main.cpp")],
            args: Vec::new(),
            stdin: large,
            files: Vec::new(),
            output_file: None,
        },
    ]
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn first_line(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .and_then(|output| {
            String::from_utf8(output.stdout)
                .ok()?
                .lines()
                .next()
                .map(str::to_owned)
        })
        .unwrap_or_else(|| format!("{program} (version unknown)"))
}

fn compile(program: &str, args: &[&str], source: &Path, output: &Path) {
    // C and C++ link the math library, which a program that takes square roots needs.
    let libraries: &[&str] = if program == "rustc" { &[] } else { &["-lm"] };
    let status = Command::new(program)
        .args(args)
        .arg("-o")
        .arg(output)
        .arg(source)
        .args(libraries)
        .status()
        .unwrap_or_else(|error| panic!("{program} didn't run: {error}"));
    assert!(status.success(), "{program} failed on {}", source.display());
}

async fn build(workload: &Workload, toolchain: &Toolchain, folder: &Path) -> Vec<Version> {
    let mut versions = Vec::new();
    for (number, (label, source)) in workload.lengua.iter().enumerate() {
        let (object, executable) = (
            folder.join(format!("{}-{number}.o", workload.name)),
            folder.join(format!("{}-lengua-{number}", workload.name)),
        );
        toolchain
            .assemble(Profile::LinuxX86_64, &root().join(source), &object)
            .await
            .expect("assemble a lengua version");
        toolchain
            .link(Profile::LinuxX86_64, &object, &executable)
            .await
            .expect("link a lengua version");
        versions.push(Version {
            label: (*label).to_owned(),
            executable,
            lengua: true,
        });
    }
    for (label, file) in workload.others {
        let source = root()
            .join("benches/programs")
            .join(workload.name)
            .join(file);
        let executable = folder.join(format!("{}-{}", workload.name, file.replace('.', "-")));
        if file.ends_with(".rs") {
            compile("rustc", &RUSTC_FLAGS, &source, &executable);
        } else if file.ends_with(".cpp") {
            compile("g++", &GCC_FLAGS, &source, &executable);
        } else {
            compile("gcc", &GCC_FLAGS, &source, &executable);
        }
        versions.push(Version {
            label: (*label).to_owned(),
            executable,
            lengua: false,
        });
    }
    versions
}

/// A counter on `pid` and every thread it starts, from its next exec.
fn attach<E: Event>(event: E, pid: i32, with_kernel: bool) -> Option<Counter> {
    let mut builder = Builder::new(event);
    builder
        .observe_pid(pid)
        .inherit(true)
        .enable_on_exec(true)
        .exclude_kernel(!with_kernel)
        .read_format(ReadFormat::TOTAL_TIME_ENABLED | ReadFormat::TOTAL_TIME_RUNNING);
    builder.build().ok()
}

fn exact(counter: &mut Counter) -> Option<u64> {
    let data = counter.read_full().ok()?;
    (data.time_running() == data.time_enabled()).then(|| data.count())
}

/// One run's measures and output.
struct Sample {
    instructions: Option<u64>,
    cycles: Option<u64>,
    cpu_time: Option<Duration>,
    wall_time: Duration,
    stdout: Vec<u8>,
    status: Option<i32>,
}

/// Runs `version` once in `folder`. A shell waits on a pipe so the counters can be attached
/// before it execs the program; they count from that exec on.
fn run(version: &Version, workload: &Workload, folder: &Path) -> Sample {
    let go = folder.join("go");
    if !go.exists() {
        let made = Command::new("mkfifo").arg(&go).status().expect("mkfifo");
        assert!(made.success(), "mkfifo failed");
    }
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(r#"read _ < "$LENGUA_GO" && exec "$0" "$@""#)
        .arg(&version.executable)
        .args(&workload.args)
        .env_clear()
        .env("LENGUA_GO", &go)
        .current_dir(folder)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start the shell");
    let pid = i32::try_from(child.id()).expect("a process id");
    let mut counters = [
        attach(Hardware::INSTRUCTIONS, pid, false),
        attach(Hardware::CPU_CYCLES, pid, false),
        attach(Software::TASK_CLOCK, pid, true)
            .or_else(|| attach(Software::TASK_CLOCK, pid, false)),
    ];
    let mut input = child.stdin.take().expect("its input");
    let bytes = workload.stdin.clone();
    let writer = std::thread::spawn(move || {
        let _ = input.write_all(&bytes);
    });
    // Opening the pipe waits for the shell to read it.
    let mut signal = std::fs::OpenOptions::new()
        .write(true)
        .open(&go)
        .expect("open the pipe");
    let started = Instant::now();
    signal.write_all(b"go\n").expect("signal the shell");
    drop(signal);
    let output = child.wait_with_output().expect("the program ends");
    let wall_time = started.elapsed();
    let _ = writer.join();
    let [instructions, cycles, cpu_time] = counters
        .each_mut()
        .map(|counter| counter.as_mut().and_then(exact));
    Sample {
        instructions,
        cycles,
        cpu_time: cpu_time.map(Duration::from_nanos),
        wall_time,
        stdout: output.stdout,
        status: output.status.code(),
    }
}

/// Peak memory in KiB, from GNU time.
fn peak_kib(version: &Version, workload: &Workload, folder: &Path) -> Option<u64> {
    let report = folder.join("peak");
    let mut child = Command::new("/usr/bin/time")
        .arg("-f")
        .arg("%M")
        .arg("-o")
        .arg(&report)
        .arg(&version.executable)
        .args(&workload.args)
        .env_clear()
        .current_dir(folder)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut input = child.stdin.take()?;
    let bytes = workload.stdin.clone();
    let writer = std::thread::spawn(move || {
        let _ = input.write_all(&bytes);
    });
    child.wait().ok()?;
    let _ = writer.join();
    std::fs::read_to_string(&report).ok()?.trim().parse().ok()
}

fn median<T: Ord + Copy>(mut values: Vec<T>) -> Option<T> {
    values.sort_unstable();
    values.get(values.len() / 2).copied()
}

fn measure(version: &Version, workload: &Workload, folder: &Path, expected: &[u8]) -> Measures {
    for (name, contents) in &workload.files {
        std::fs::write(folder.join(name), contents).expect("write an input file");
    }
    let mut samples = Vec::new();
    for round in 0..WARM_UP + RUNS {
        let sample = run(version, workload, folder);
        assert_eq!(
            sample.status,
            Some(0),
            "{} {} failed",
            workload.name,
            version.label
        );
        let output = match workload.output_file {
            Some(name) => std::fs::read(folder.join(name)).expect("the output file"),
            None => sample.stdout.clone(),
        };
        assert!(
            output == expected,
            "{} {} printed something else",
            workload.name,
            version.label
        );
        if round >= WARM_UP {
            samples.push(sample);
        }
    }
    let peaks: Vec<u64> = (0..5)
        .filter_map(|_| peak_kib(version, workload, folder))
        .collect();
    Measures {
        instructions: median(samples.iter().filter_map(|s| s.instructions).collect()),
        cycles: median(samples.iter().filter_map(|s| s.cycles).collect()),
        cpu_time: median(samples.iter().filter_map(|s| s.cpu_time).collect()),
        wall_time: median(samples.iter().map(|s| s.wall_time).collect()).unwrap_or_default(),
        peak_kib: median(peaks),
        size: std::fs::metadata(&version.executable).map_or(0, |metadata| metadata.len()),
    }
}

/// The output every version must give, computed here.
fn expected_output(workload: &Workload) -> Vec<u8> {
    let space = |byte: &u8| matches!(byte, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r');
    match workload.name {
        "add" => b"122469135\n".to_vec(),
        // fixed(sqrt(a * a + b * b), 3), as the spec computes it.
        "hypotenuse" => {
            let (a, b) = (123_456_789.0_f64 / 1000.0, 98_765_432.0_f64 / 1000.0);
            #[allow(clippy::cast_possible_truncation)]
            let thousandths = ((a * a + b * b).sqrt() * 1000.0).round_ties_even() as i64;
            format!("{}.{:03}\n", thousandths / 1000, thousandths % 1000).into_bytes()
        }
        "wc" => {
            let lines = workload.stdin.iter().filter(|byte| **byte == b'\n').count();
            let words = workload
                .stdin
                .split(space)
                .filter(|word| !word.is_empty())
                .count();
            format!("{lines} {words}\n").into_bytes()
        }
        "histogram" => {
            let mut counts = [0_u64; 256];
            for byte in &workload.stdin {
                counts[usize::from(*byte)] += 1;
            }
            let mut out = String::new();
            for (value, count) in counts.iter().enumerate().filter(|(_, count)| **count > 0) {
                let _ = writeln!(out, "{value} {count}");
            }
            out.into_bytes()
        }
        "crc32c" => {
            let mut crc: u32 = 0xFFFF_FFFF;
            for byte in &workload.stdin {
                crc ^= u32::from(*byte);
                for _ in 0..8 {
                    crc = (crc >> 1) ^ (0x82F6_3B78 & 0_u32.wrapping_sub(crc & 1));
                }
            }
            format!("{}\n", !crc).into_bytes()
        }
        _ => {
            let input = workload
                .files
                .first()
                .map_or(&workload.stdin, |(_, contents)| contents);
            input.to_ascii_uppercase()
        }
    }
}

fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

fn micros(duration: Duration) -> String {
    format!("{:.0} µs", duration.as_secs_f64() * 1e6)
}

fn size(bytes: u64) -> String {
    if bytes < 10 * 1024 {
        format!("{} B", thousands(bytes))
    } else {
        format!("{:.0} KiB", bytes as f64 / 1024.0)
    }
}

/// How many times more the best other version takes than the best lengua version.
fn ratio(lengua: &[Option<f64>], others: &[Option<f64>]) -> String {
    let best = |values: &[Option<f64>]| {
        values
            .iter()
            .flatten()
            .copied()
            .fold(f64::INFINITY, f64::min)
    };
    let (lengua, other) = (best(lengua), best(others));
    if lengua > 0.0 && lengua.is_finite() && other.is_finite() {
        format!("{:.1}×", other / lengua)
    } else {
        "–".into()
    }
}

#[tokio::main]
async fn main() {
    let toolchain = Toolchain::discover().await.expect("llvm-mc and ld.lld");
    let machine = Machine::detect();
    let folder = tempfile::tempdir().expect("a folder");
    let mut report = String::from(
        "# lengua against C, C++ and Rust\n\nMeasured by `cargo bench --bench against_rust_and_c`, \
         which writes this file. Every version of a program gets the same arguments, input and \
         files and must give the same output. Instructions and cycles are user-space counts \
         from the CPU's counters, starting at the program's own exec, so a dynamically linked \
         program's loader counts; CPU time includes the kernel's work for it. Each figure is \
         the median of ",
    );
    let _ = write!(
        report,
        "{RUNS} runs after {WARM_UP} to warm up; peak memory is the median of 5 runs of GNU \
         time. \"Best other ÷ best lengua\" is how many times more the best C, C++ or Rust \
         version takes than the best lengua version.\n\n\
         - CPU: {} ({} cores)\n- lengua: `llvm-mc {}, ld.lld {}`\n- Rust: `{}`, `rustc {}`\n\
         - C and C++: `{}`, `gcc {}` and `g++ {}`\n\
         - C, C++ and Rust link the system's C library dynamically, as they ship by default; \
         lengua's programs are static and use no library.\n",
        machine.cpu_model.as_deref().unwrap_or("unknown"),
        machine.cores,
        toolchain.assembler.version,
        toolchain.linker.version,
        first_line("rustc", &["--version"]),
        RUSTC_FLAGS.join(" "),
        first_line("gcc", &["--version"]),
        GCC_FLAGS.join(" "),
        GCC_FLAGS.join(" "),
    );
    for workload in workloads() {
        eprintln!("{}…", workload.name);
        let versions = build(&workload, &toolchain, folder.path()).await;
        let expected = expected_output(&workload);
        let mut measured = Vec::new();
        for version in &versions {
            let work = folder
                .path()
                .join(format!("{}-{}", workload.name, measured.len()));
            std::fs::create_dir(&work).expect("a working folder");
            measured.push(measure(version, &workload, &work, &expected));
        }
        let _ = write!(
            report,
            "\n## {}\n\n{}\n\n| | {} | best other ÷ best lengua |\n|---|{}---|\n",
            workload.name,
            workload.about,
            versions
                .iter()
                .map(|version| version.label.as_str())
                .collect::<Vec<_>>()
                .join(" | "),
            "---|".repeat(versions.len())
        );
        let row = |name: &str,
                   cell: &dyn Fn(&Measures) -> String,
                   value: &dyn Fn(&Measures) -> Option<f64>| {
            let cells: Vec<String> = measured.iter().map(cell).collect();
            let (mut lengua, mut others) = (Vec::new(), Vec::new());
            for (version, measures) in versions.iter().zip(&measured) {
                if version.lengua {
                    lengua.push(value(measures));
                } else {
                    others.push(value(measures));
                }
            }
            format!(
                "| {name} | {} | {} |\n",
                cells.join(" | "),
                ratio(&lengua, &others)
            )
        };
        let missing = || "–".to_owned();
        report.push_str(&row(
            "instructions",
            &|m| m.instructions.map_or_else(missing, thousands),
            &|m| m.instructions.map(|value| value as f64),
        ));
        report.push_str(&row(
            "cycles",
            &|m| m.cycles.map_or_else(missing, thousands),
            &|m| m.cycles.map(|value| value as f64),
        ));
        report.push_str(&row(
            "CPU time",
            &|m| m.cpu_time.map_or_else(missing, micros),
            &|m| m.cpu_time.map(|value| value.as_secs_f64()),
        ));
        report.push_str(&row("wall time", &|m| micros(m.wall_time), &|m| {
            Some(m.wall_time.as_secs_f64())
        }));
        report.push_str(&row(
            "peak memory",
            &|m| {
                m.peak_kib
                    .map_or_else(missing, |kib| format!("{} KiB", thousands(kib)))
            },
            &|m| m.peak_kib.map(|value| value as f64),
        ));
        report.push_str(&row("file size", &|m| size(m.size), &|m| {
            Some(m.size as f64)
        }));
    }
    let path = root().join("BENCHMARKS.md");
    std::fs::write(&path, &report).expect("write BENCHMARKS.md");
    println!("{report}");
    eprintln!("wrote {}", path.display());
}
