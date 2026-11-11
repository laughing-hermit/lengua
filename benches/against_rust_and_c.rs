//! The gate: lengua's programs against the same programs in C, C++ and Rust, on the runner it
//! runs on (todo.md's finish line, point 4): `cargo bench --bench against_rust_and_c`.
//!
//! A workload has lengua's program, its folder in `PROGRAMS`, which the bench takes from its
//! idea to done as an agent does, and six other versions from benches/programs/<workload>: C,
//! C++ and Rust compiled to WebAssembly by `cargo xtask bench-wasm` (once, on linux-x86_64, for
//! every runner), and the same three built natively here with this system's own compilers. Every
//! version gets the same arguments, input and files and must give the same output, which is
//! checked before anything is measured. Each runs as people run it, in a process of its own,
//! measured by `lengua-mcp measure`: lengua's through `lengua-mcp run`, with every check it
//! makes; the other modules in lengua's host, in the same engine with the same cache; the native
//! builds as a shell starts them.
//!
//! The runs alternate: a round runs every version once, in an order shuffled from round to round.
//! On each row, lengua against one other version, `lengua::gate` decides CPU and wall time with
//! a one-sided Mann-Whitney U test, adding runs until both decide, and peak memory and size as
//! measured; a measure within the gate's noise margin, or one the test doesn't decide, is level,
//! neither ahead nor behind (`gate::MARGIN`).
//!
//! Everything the run measured goes to target/bench/results-<target>.json; the report,
//! BENCHMARKS-<target>.md, is written from that file alone; the lines of KNOWN_BEHIND the run
//! passes with go to target/bench/KNOWN_BEHIND-<target>.txt. The bench fails where the run and
//! KNOWN_BEHIND disagree, where a change in KNOWN_BEHIND's history named a row the list before it
//! didn't (the list may only shrink, so the bench reads the repository's whole history), and
//! where a version can't be measured.
//!
//! Apart from the gate's rounds, and after them, lengua's program runs against lengua-module:
//! the very module lengua's build makes from the program's main.wat, run by `lengua-mcp measure`
//! as the other modules are. The two run alone, in rounds of their own, so the gate's rounds run
//! as they would without them, and the report says how far apart they are: what lengua's run
//! does before and around running its module. lengua-module is no row, and the gate and
//! KNOWN_BEHIND never see it; like every version, it must give the same output, or the bench
//! fails.
//!
//! What the environment can say:
//! - `LENGUA_BENCH_WASM`: the folder of the WebAssembly builds, target/bench-wasm when unset;
//! - `LENGUA_BENCH_ONLY`: the workloads to measure, between commas, instead of every one;
//! - `LENGUA_BENCH_ROW_SECONDS`: the longest a workload's rows run, 600 seconds when unset;
//! - `LENGUA_BENCH_AGAINST`: a commit whose lengua programs are measured against today's;
//! - `LENGUA_BENCH_REPORT`: a results file, from a run's artifacts, to write the report from,
//!   measuring nothing.

#[path = "../tests/common/mod.rs"]
mod common;

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use common::{Program, Protocol, Session};
use lengua::gate::{self, Found, KnownBehind, Measure, Verdict};
use lengua::hash::Sha256Hex;
use lengua::measure::Measures;
use serde::{Deserialize, Serialize};

/// The rounds after each version's checked run that aren't counted either: the system's cache of
/// the files, and the engine's of each module compiled, are warm when the counted rounds begin.
const WARM_UP: usize = 2;

/// The runs of each version with the CPU's counters attached, apart from the counted ones.
const COUNTER_RUNS: usize = 5;

/// The version that runs lengua's own module the way the other modules run: built from the
/// checked program's main.wat by lengua's build, the module `lengua-mcp run` runs, and run by
/// `lengua-mcp measure` in lengua's host. It runs only in rounds of its own with lengua's
/// program, and is no row of the gate's.
const MODULE_VERSION: &str = "lengua-module";

/// lengua-module's file, beside the workload's working folder and never in it.
const MODULE_FILE: &str = "lengua-module.wasm";

/// The counted runs each of lengua and lengua-module takes in their rounds of their own.
const APART_RUNS: usize = 160;

/// The fewest counted runs each of them takes before their rounds may stop on time.
const APART_FEWEST: usize = 20;

/// How long their counted rounds go on, once each has [`APART_FEWEST`] runs.
const APART_TIME: Duration = Duration::from_secs(60);

/// The large workloads' input: 64 MiB, so that the work outweighs starting and ending a process.
const LARGE: usize = 64 * 1024 * 1024;

/// The input of the workloads whose lengua program takes it whole: 1 MiB, the most it takes.
const MEBIBYTE: usize = 1024 * 1024;

/// How many small files the files workload reads.
const FILES: usize = 4096;

/// The most bytes in each of them.
const FILE_BYTES: usize = 4096;

/// The port lengua's server listens on, as its idea says, which every server version is given.
const PORT: u16 = 8080;

/// How many clients the server has at once.
const CLIENTS: usize = 64;

/// How many requests each client sends, one at a time.
const REQUESTS: usize = 1000;

/// The other versions' languages, by the gate's names, and their source files in
/// benches/programs/<workload>: each is built natively, and compiled to `<language>.wasm`.
const LANGUAGES: [(&str, &str); 3] = [("c", "main.c"), ("cpp", "main.cpp"), ("rust", "main.rs")];

/// The other versions built natively, by the gate's names.
const NATIVE: [&str; 3] = ["c", "cpp", "rust"];

/// Each workload's lengua program: its folder, from the repository's root.
const PROGRAMS: [(&str, &str); 14] = [
    ("empty", "tests/fixtures/empty"),
    ("add", "tests/fixtures/add"),
    ("hypotenuse", "examples/hypotenuse"),
    ("upcase", "examples/upcase"),
    ("upcase-stdin", "tests/fixtures/upcase-stdin"),
    ("shout", "tests/fixtures/shout"),
    ("wc", "examples/count"),
    ("histogram", "tests/fixtures/histogram"),
    ("crc32c", "tests/fixtures/crc32c"),
    ("sort", "tests/fixtures/sort-large"),
    ("tally", "tests/fixtures/tally"),
    ("server", "tests/fixtures/shout-server"),
    ("files", "tests/fixtures/files"),
    ("readings", "tests/fixtures/readings"),
];

/// lengua's program for workload `name`: its folder, from the repository's root.
fn program_of(name: &str) -> &'static str {
    PROGRAMS
        .iter()
        .find(|(workload, _)| *workload == name)
        .map_or("", |(_, program)| *program)
}

/// One workload: what every version of it runs on.
struct Workload {
    /// Its name, as the gate and KNOWN_BEHIND know it, and its folder in benches/programs.
    name: &'static str,
    /// What it does, for the report.
    about: &'static str,
    /// Its arguments.
    args: Vec<String>,
    /// Its standard input, when it reads one.
    stdin: Option<Vec<u8>>,
    /// The files of its working folder, by name.
    files: Vec<(String, Vec<u8>)>,
    /// The file it writes, when its output is a file and not standard output.
    output_file: Option<&'static str>,
    /// Whether it is a server, measured under load.
    server: bool,
}

impl Workload {
    /// The workload `name`, with no arguments, input or files yet.
    fn new(name: &'static str, about: &'static str) -> Self {
        Self {
            name,
            about,
            args: Vec::new(),
            stdin: None,
            files: Vec::new(),
            output_file: None,
            server: false,
        }
    }
}

/// `words` as owned strings.
fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

/// The workload `name`, its input made from fixed seeds: the same bytes on every run.
fn workload(name: &str) -> Option<Workload> {
    let workload = match name {
        "empty" => Workload::new(
            "empty",
            "Starts and exits with code 0: all of it is starting and ending a process, which no \
             program avoids, the floor under every workload that starts one.",
        ),
        "add" => Workload {
            args: strings(&["123456789", "-987654"]),
            ..Workload::new(
                "add",
                "Parses two whole numbers from its arguments and prints their sum: almost all of \
                 it is starting up and exiting.",
            )
        },
        "hypotenuse" => Workload {
            args: strings(&["123456.789", "98765.432"]),
            ..Workload::new(
                "hypotenuse",
                "Prints the hypotenuse of two decimal sides from its arguments, rounded to three \
                 decimals: a square root, and almost all of it starting up and exiting.",
            )
        },
        "upcase" => Workload {
            args: strings(&["in.txt", "out.txt"]),
            files: vec![("in.txt".to_owned(), text(MEBIBYTE))],
            output_file: Some("out.txt"),
            ..Workload::new(
                "upcase",
                "Copies a 1 MiB text file to another with its ASCII letters in capitals: a file \
                 opened, read, made and written, and one pass over the bytes. lengua's program \
                 changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store \
                 every byte in a plain loop, for their compilers to vectorize.",
            )
        },
        "upcase-stdin" => Workload {
            stdin: Some(text(LARGE)),
            ..Workload::new(
                "upcase-stdin",
                "Copies 64 MiB of standard input to standard output with its ASCII letters in \
                 capitals, a piece at a time. lengua's program changes 64 bytes a step with \
                 SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain \
                 loop, for their compilers to vectorize.",
            )
        },
        "shout" => Workload {
            stdin: Some(text(MEBIBYTE)),
            ..Workload::new(
                "shout",
                "Reads 1 MiB of standard input whole, then prints it with its ASCII letters in \
                 capitals. lengua's program changes 64 bytes a step with SIMD128, written by \
                 hand, then 16; C, C++ and Rust store every byte in a plain loop, for their \
                 compilers to vectorize.",
            )
        },
        "wc" => Workload {
            stdin: Some(text(LARGE)),
            ..Workload::new(
                "wc",
                "Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. \
                 lengua's program compares 16 bytes a step with SIMD128, written by hand, and \
                 adds each comparison's lanes to counters of a byte, widened every 255 steps; C, \
                 C++ and Rust compare each byte and the byte before it in a plain loop, counted \
                 255 bytes at a time in counters of a byte, for their compilers to vectorize.",
            )
        },
        "histogram" => Workload {
            stdin: Some(text(LARGE)),
            ..Workload::new(
                "histogram",
                "Counts each byte value of 64 MiB of standard input. Every version counts in \
                 four tables, so that neighboring bytes don't wait on one counter: C, C++ and \
                 Rust four bytes a step, lengua's program eight, each byte loaded on its own.",
            )
        },
        "crc32c" => Workload {
            stdin: Some(text(LARGE)),
            ..Workload::new(
                "crc32c",
                "The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the \
                 CPU's CRC instruction; WebAssembly has none, so the modules take the bytes \
                 through tables: C, C++ and Rust eight bytes a step through eight tables, \
                 lengua's program sixteen through sixteen.",
            )
        },
        "sort" => Workload {
            stdin: Some(text(LARGE)),
            ..Workload::new(
                "sort",
                "Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` \
                 does. Each version sorts with its own language's sort: lengua's program finds \
                 the lines 16 bytes a step from a bitmask of the newlines, gives each a record of \
                 8 bytes, its start below 4 bytes of the line as a key that compares as one \
                 number, and orders the records in place with a multikey quicksort without \
                 recursion, whose two splits around a median of three keys take no branch on a \
                 record, whose lines that are the same as the pivot go on with their next 4 \
                 bytes, and which sorts a part with a heap sort once it has been split below or \
                 above a pivot more than twice as many times as the lines' count has bits, as \
                 introsort does; it prints the lines 16 bytes a step with SIMD128, written by \
                 hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ \
                 with std::sort and Rust with sort_unstable, each comparing lines with its \
                 library's memcmp or comparison of slices. Every version prints through a 64 KiB \
                 buffer.",
            )
        },
        "tally" => Workload {
            stdin: Some(prose(LARGE)),
            ..Workload::new(
                "tally",
                "Counts each different word of 64 MiB of standard input, 4,096 different words \
                 from common to rare, and prints them from the most frequent. C and C++ count in \
                 a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each \
                 passing over the bytes and hashing them a byte at a time. lengua's program finds \
                 where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written \
                 by hand, and counts in a table with linear probing whose entries hold a word's \
                 first 8 bytes as its key: a word of up to 8 bytes is hashed with one \
                 multiplication and matched as one number.",
            )
        },
        "server" => Workload {
            args: vec![PORT.to_string()],
            server: true,
            ..Workload::new(
                "server",
                "A server: 64 clients at once each send 1,000 lines of 64 bytes, one at a time, \
                 and each line comes back in capitals before the next goes. Its wall time is \
                 from the first connection to the last answer.",
            )
        },
        "files" => Workload {
            files: small_files(),
            ..Workload::new(
                "files",
                "Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its \
                 end, and prints how many files and bytes it read. A read of a file gives every \
                 byte it asks for up to the file's end, so lengua's program takes a read that \
                 gives less than its 64 KiB buffer as a file's last, one read for each of these \
                 files; C, C++ and Rust read until a read gives nothing, two reads a file.",
            )
        },
        "readings" => Workload {
            args: strings(&["1000000"]),
            ..Workload::new(
                "readings",
                "Reads the clock that only goes forward a million times.",
            )
        },
        _ => return None,
    };
    Some(workload)
}

/// The next state of a xorshift generator, from a state that isn't 0.
fn next(mut state: u64) -> u64 {
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    state
}

/// Text like what people write: letters in both cases, digits, punctuation, newlines and some
/// letters beyond ASCII, from a fixed seed, cut at a whole character to at most `bytes`.
fn text(bytes: usize) -> Vec<u8> {
    const WORDS: [&str; 12] = [
        "lengua", "Assembly", "proves", "every", "input", "café", "fast", "Rust", "C", "42",
        "naïve", "x86-64",
    ];
    let mut state: u64 = 0x2545_f491_4f6c_dd1d;
    let mut text = Vec::with_capacity(bytes + 16);
    while text.len() < bytes {
        state = next(state);
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

/// Words between spaces and newlines, for the tally: 4,096 different words of one to three
/// syllables, a few common and most rare, as in prose, from a fixed seed, `bytes` of them.
fn prose(bytes: usize) -> Vec<u8> {
    const SYLLABLES: [&str; 16] = [
        "ka", "lo", "mi", "ne", "su", "ta", "ri", "po", "an", "el", "is", "or", "un", "ve", "zu",
        "qi",
    ];
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut text = Vec::with_capacity(bytes + 16);
    while text.len() < bytes {
        state = next(state);
        // A word's number has from 0 to 12 bits, each count of bits as likely, so the small
        // numbers are common and most of the 4,096 rare. Its digits in base 16 are its syllables.
        let bits = state % 13;
        let mut number = (state >> 16) & ((1_u64 << bits) - 1);
        loop {
            let syllable = SYLLABLES[usize::try_from(number % 16).unwrap_or(0)];
            text.extend_from_slice(syllable.as_bytes());
            number /= 16;
            if number == 0 {
                break;
            }
        }
        text.push(if state >> 60 == 0 { b'\n' } else { b' ' });
    }
    text.truncate(bytes);
    text
}

/// The files workload's folder: 4,096 files of 1 to 4,096 bytes of text.
fn small_files() -> Vec<(String, Vec<u8>)> {
    let source = text(FILES * FILE_BYTES);
    let room = source.len() - FILE_BYTES;
    let mut state: u64 = 0x5851_f42d_4c95_7f2d;
    let mut at = 0;
    let mut files = Vec::with_capacity(FILES);
    for number in 0..FILES {
        state = next(state);
        let size = 1 + usize::try_from(state).unwrap_or(0) % FILE_BYTES;
        let start = at % room;
        at += size;
        let contents = source[start..start + size].to_vec();
        files.push((format!("file-{number:04}.txt"), contents));
    }
    files
}

/// Whether `byte` is whitespace, as wc and the tally take it: a space, a tab, a newline, a
/// vertical tab, a form feed or a carriage return.
fn whitespace(byte: &u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// The output every version of `workload` must give, worked out here: what it prints, or what
/// it writes in its file. A server's answers are checked as they come.
fn expected(workload: &Workload) -> Vec<u8> {
    let input = workload.stdin.as_deref().unwrap_or_default();
    match workload.name {
        "add" => b"122469135\n".to_vec(),
        // fixed(sqrt(a * a + b * b), 3), as the idea computes it, from sides in thousandths.
        "hypotenuse" => {
            let (a, b) = (123_456_789.0_f64 / 1000.0, 98_765_432.0_f64 / 1000.0);
            let thousandths = ((a * a + b * b).sqrt() * 1000.0).round_ties_even() as i64;
            format!("{}.{:03}\n", thousandths / 1000, thousandths % 1000).into_bytes()
        }
        "upcase" => {
            let file = workload.files.first();
            file.map(|(_, contents)| contents.to_ascii_uppercase())
                .unwrap_or_default()
        }
        "upcase-stdin" | "shout" => input.to_ascii_uppercase(),
        "wc" => {
            let lines = input.iter().filter(|byte| **byte == b'\n').count();
            let words = input.split(whitespace).filter(|word| !word.is_empty());
            format!("{lines} {}\n", words.count()).into_bytes()
        }
        "histogram" => {
            let mut counts = [0_u64; 256];
            for byte in input {
                counts[usize::from(*byte)] += 1;
            }
            let mut out = String::new();
            for (value, count) in counts.iter().enumerate().filter(|(_, count)| **count > 0) {
                let _ = writeln!(out, "{value} {count}");
            }
            out.into_bytes()
        }
        "crc32c" => format!("{}\n", crc32c(input)).into_bytes(),
        "sort" => sorted_lines(input),
        "tally" => tally(input),
        "files" => {
            let bytes: usize = workload.files.iter().map(|file| file.1.len()).sum();
            format!("{} {bytes}\n", workload.files.len()).into_bytes()
        }
        "readings" => b"1000000\n".to_vec(),
        // empty prints nothing, and a server's answers are checked as they come.
        _ => Vec::new(),
    }
}

/// The CRC-32C of `bytes`, a byte at a time through a table.
fn crc32c(bytes: &[u8]) -> u32 {
    let mut table = [0_u32; 256];
    for (index, entry) in (0_u32..).zip(table.iter_mut()) {
        let mut crc = index;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0x82F6_3B78 & 0_u32.wrapping_sub(crc & 1));
        }
        *entry = crc;
    }
    let mut crc = !0_u32;
    for byte in bytes {
        crc = table[usize::from((crc as u8) ^ *byte)] ^ (crc >> 8);
    }
    !crc
}

/// The lines of `input` in byte order, each with a newline, as `LC_ALL=C sort` prints them: a
/// last line without a newline is a line too.
fn sorted_lines(input: &[u8]) -> Vec<u8> {
    let mut lines: Vec<&[u8]> = input.split(|byte| *byte == b'\n').collect();
    // What follows the last newline is a line only when it isn't empty.
    if lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.sort_unstable();
    let mut out = Vec::with_capacity(input.len() + 1);
    for line in lines {
        out.extend_from_slice(line);
        out.push(b'\n');
    }
    out
}

/// The tally of `input`'s words: a line for each different word, its count, a space and the
/// word, the most frequent first and words of the same count in byte order.
fn tally(input: &[u8]) -> Vec<u8> {
    let mut counts: HashMap<&[u8], u64> = HashMap::new();
    for word in input.split(whitespace).filter(|word| !word.is_empty()) {
        *counts.entry(word).or_default() += 1;
    }
    let mut words: Vec<(&[u8], u64)> = counts.into_iter().collect();
    words.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let mut out = Vec::new();
    for (word, count) in words {
        out.extend_from_slice(count.to_string().as_bytes());
        out.push(b' ');
        out.extend_from_slice(word);
        out.push(b'\n');
    }
    out
}

/// The words after `label` on the line of `idea` that starts with it, such as its `- Uses:`
/// line's, or nothing.
fn constraint(idea: &str, label: &str) -> String {
    let line = idea.lines().find_map(|line| line.strip_prefix(label));
    line.map(|words| words.trim().to_owned())
        .unwrap_or_default()
}

/// `error`, after what it happened to.
fn failed(what: impl std::fmt::Display, error: impl std::fmt::Display) -> String {
    format!("{what}: {error}")
}

/// A workload ready to run: its folder, its input and the output to check.
struct Prepared {
    /// Its arguments.
    args: Vec<String>,
    /// The folder every version runs in, which holds the workload's files and nothing else.
    folder: PathBuf,
    /// The file its standard input comes from, beside the folder.
    stdin: Option<PathBuf>,
    /// Where a checked run's standard output goes, beside the folder.
    stdout: PathBuf,
    /// Where each run's standard error goes.
    stderr: PathBuf,
    /// The file of the folder it writes, when its output is a file.
    output_file: Option<&'static str>,
    /// The output every version must give: its SHA-256, its length and its first bytes.
    expected: (Sha256Hex, usize, String),
    /// What a module of another toolchain may reach: the words of lengua's program's idea's
    /// `- Uses:` line.
    uses: String,
    /// Its idea's `- Network:` line, after the colon.
    network: String,
    /// Whether it is a server.
    server: bool,
}

/// Writes `workload`'s files and input into `place`, and works out the output to check, with
/// what a module may reach from `idea`, lengua's program's.
fn prepare(workload: Workload, place: &Path, idea: &str) -> Result<Prepared, String> {
    let folder = place.join("folder");
    std::fs::create_dir_all(&folder).map_err(|error| failed(folder.display(), error))?;
    for (name, contents) in &workload.files {
        let file = folder.join(name);
        std::fs::write(&file, contents).map_err(|error| failed(file.display(), error))?;
    }
    let output = expected(&workload);
    let head = String::from_utf8_lossy(&output[..output.len().min(80)]).into_owned();
    let stdin = match &workload.stdin {
        Some(bytes) => {
            let file = place.join("stdin");
            std::fs::write(&file, bytes).map_err(|error| failed(file.display(), error))?;
            Some(file)
        }
        None => None,
    };
    Ok(Prepared {
        args: workload.args,
        folder,
        stdin,
        stdout: place.join("stdout"),
        stderr: place.join("stderr"),
        output_file: workload.output_file,
        expected: (Sha256Hex::of(&output), output.len(), head),
        uses: constraint(idea, "- Uses:"),
        network: constraint(idea, "- Network:"),
        server: workload.server,
    })
}

/// One version of a workload, ready to run.
#[derive(Debug, Clone)]
struct Version {
    /// `lengua`, or the other version's name in the gate.
    name: String,
    /// What runs: lengua's checked program folder, a module, or an executable.
    program: PathBuf,
    /// How it was made, for the report.
    made: String,
}

/// A version of a workload as the bench made it: ready to run, or why it isn't.
struct Made {
    /// Its name: `lengua`, or the other version's in the gate.
    name: String,
    /// The version, or why it couldn't be made.
    version: Result<Version, String>,
}

/// What measures every run: lengua-mcp's `measure`, with the data folder whose key signs
/// lengua's locks and whose cache keeps every module compiled, lengua's and the others' alike.
struct Runner {
    /// lengua-mcp, as Cargo built it for the bench.
    lengua: PathBuf,
    /// The data folder.
    data_dir: PathBuf,
}

impl Runner {
    /// One run of `version` on `prepared`, measured, with its standard output kept in
    /// `prepared.stdout` when `keep` and the CPU's counters attached when `counters`.
    ///
    /// # Errors
    ///
    /// When it can't be measured, ends other than with exit code 0, or a server answers wrong.
    fn run(
        &self,
        prepared: &Prepared,
        version: &Version,
        keep: bool,
        counters: bool,
    ) -> Result<Measures, String> {
        if prepared.server {
            wait_for_port();
        }
        let mut command = Command::new(&self.lengua);
        command
            .arg("--data-dir")
            .arg(&self.data_dir)
            .arg("measure")
            .arg("--in")
            .arg(&prepared.folder)
            .arg("--stderr")
            .arg(&prepared.stderr);
        if keep {
            command.arg("--stdout").arg(&prepared.stdout);
        }
        if let Some(stdin) = &prepared.stdin {
            command.arg("--stdin").arg(stdin);
        }
        if !prepared.uses.is_empty() {
            command.arg("--uses").arg(&prepared.uses);
        }
        if !prepared.network.is_empty() {
            command.arg("--network").arg(&prepared.network);
        }
        if prepared.server {
            let load = [
                ("--serve", PORT.to_string()),
                ("--clients", CLIENTS.to_string()),
                ("--requests", REQUESTS.to_string()),
            ];
            for (option, value) in load {
                command.arg(option).arg(value);
            }
        }
        if counters {
            command.arg("--counters");
        }
        command.arg(&version.program).arg("--");
        command.args(&prepared.args).stdin(Stdio::null());
        let name = &version.name;
        let output = command
            .output()
            .map_err(|error| format!("{name}: lengua-mcp didn't start: {error}"))?;
        let said = || std::fs::read_to_string(&prepared.stderr).unwrap_or_default();
        if !output.status.success() {
            let (measure, said) = (String::from_utf8_lossy(&output.stderr), said());
            let (measure, said) = (measure.trim(), said.trim());
            return Err(format!("{name}: measure failed: {measure} {said}"));
        }
        let measures: Measures = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("{name}: measure's output: {error}"))?;
        if prepared.server {
            if let Some(problem) = &measures.problem {
                return Err(format!("{name}: {problem}"));
            }
            let (answered, asked) = (measures.answered.unwrap_or(0), CLIENTS * REQUESTS);
            if answered != asked as u64 {
                return Err(format!("{name} answered {answered} of {asked} requests"));
            }
        } else if measures.exit != Some(0) {
            let (exit, said) = (measures.exit, said());
            let said = said.trim();
            return Err(format!("{name} ended with {exit:?}, not 0: {said}"));
        }
        Ok(measures)
    }
}

/// Waits until the server's port is free, as a server just stopped can hold it a moment.
fn wait_for_port() {
    let deadline = Instant::now() + Duration::from_secs(60);
    while TcpListener::bind((Ipv4Addr::LOCALHOST, PORT)).is_err() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Runs `version` once with its output kept, and checks that it is the output every version
/// must give: gives the size of what ran.
fn check(runner: &Runner, prepared: &Prepared, version: &Version) -> Result<u64, String> {
    if let Some(name) = prepared.output_file {
        let _ = std::fs::remove_file(prepared.folder.join(name));
    }
    let measures = runner.run(prepared, version, true, false)?;
    if prepared.server {
        return Ok(measures.size);
    }
    let file = match prepared.output_file {
        Some(name) => prepared.folder.join(name),
        None => prepared.stdout.clone(),
    };
    let name = &version.name;
    let output = std::fs::read(&file).map_err(|error| failed(file.display(), error))?;
    let (sha, length, head) = &prepared.expected;
    if Sha256Hex::of(&output) != *sha {
        let got = String::from_utf8_lossy(&output[..output.len().min(80)]);
        let gave = output.len();
        return Err(format!(
            "{name} gave {gave} bytes beginning {got:?}, not the {length} bytes every version \
             must give, beginning {head:?}"
        ));
    }
    Ok(measures.size)
}

/// A row of a workload's table in the report: its label, and how each version's cell is shown.
type TableLine = (&'static str, fn(&Measured) -> String);

/// One version's runs as the results keep them, and what it is.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Measured {
    /// `lengua`, or the other version's name.
    name: String,
    /// How it was made.
    made: String,
    /// The size of what ran, in bytes.
    size: u64,
    /// Each counted run's CPU time, in user space and the kernel, in nanoseconds.
    cpu_ns: Vec<u64>,
    /// Each counted run's wall time, in nanoseconds.
    wall_ns: Vec<u64>,
    /// Each counted run's peak resident memory, in bytes, where the system said.
    peak_bytes: Vec<u64>,
    /// The medians of the CPU's counts of its reads that missed the L1 data cache, the last-level
    /// cache and the data TLB, where the system counts them.
    misses: [Option<f64>; 3],
    /// Why it wasn't measured, or not to the end, if it wasn't.
    problem: Option<String>,
}

impl Measured {
    /// Keeps one counted run's measures.
    fn push(&mut self, measures: &Measures) {
        self.cpu_ns.push(measures.cpu_ns());
        self.wall_ns.push(measures.wall_ns);
        if let Some(peak) = measures.peak_bytes {
            self.peak_bytes.push(peak);
        }
    }
}

/// `values` as floats, for the statistics.
fn floats(values: &[u64]) -> Vec<f64> {
    values.iter().map(|value| *value as f64).collect()
}

/// The shift from `before`'s values to `after`'s, as Hodges and Lehmann estimate it: the median
/// of every difference of a value of `after` less a value of `before`, with its 95% interval, the
/// k-th smallest and the k-th largest of those m·n differences, k = floor(m·n/2 − 1.96·
/// sqrt(m·n·(m+n+1)/12)) and at least 1 (Mann-Whitney's normal bound). None when either has no
/// values.
fn hodges_lehmann(after: &[u64], before: &[u64]) -> Option<(f64, f64, f64)> {
    if after.is_empty() || before.is_empty() {
        return None;
    }
    let mut differences = Vec::with_capacity(after.len() * before.len());
    for value in after {
        for other in before {
            differences.push(*value as f64 - *other as f64);
        }
    }
    differences.sort_by(f64::total_cmp);
    let (m, n) = (after.len() as f64, before.len() as f64);
    let bound = (m * n / 2.0 - 1.96 * (m * n * (m + n + 1.0) / 12.0).sqrt()).floor();
    let count = differences.len();
    let k = if bound >= 1.0 {
        (bound as usize).min(count)
    } else {
        1
    };
    let estimate = gate::median(&differences)?;
    Some((estimate, differences[k - 1], differences[count - k]))
}

/// The mean of `values`, or none for none.
fn mean(values: &[u64]) -> Option<f64> {
    (!values.is_empty()).then(|| floats(values).iter().sum::<f64>() / values.len() as f64)
}

/// The difference of `after`'s mean less `before`'s, with its 95% interval: 1.96 standard errors
/// either side, each sample's variance taken on its own (Welch's). None when either has fewer than
/// two values.
fn welch(after: &[u64], before: &[u64]) -> Option<(f64, f64, f64)> {
    let spread = |values: &[u64]| -> Option<(f64, f64)> {
        if values.len() < 2 {
            return None;
        }
        let average = mean(values)?;
        let squares: f64 = floats(values)
            .iter()
            .map(|value| (value - average) * (value - average))
            .sum();
        Some((
            average,
            squares / (values.len() - 1) as f64 / values.len() as f64,
        ))
    };
    let (after_mean, after_share) = spread(after)?;
    let (before_mean, before_share) = spread(before)?;
    let difference = after_mean - before_mean;
    let error = (after_share + before_share).sqrt();
    Some((
        difference,
        difference - 1.96 * error,
        difference + 1.96 * error,
    ))
}

/// The gate's verdict at a look on lengua's values against the other version's, over the runs
/// both have, and its p.
fn tested(lengua: &[u64], other: &[u64]) -> (Verdict, f64) {
    let runs = lengua.len().min(other.len());
    gate::tested_verdict(
        &floats(&lengua[..runs]),
        &floats(&other[..runs]),
        gate::LOOK_ALPHA,
    )
}

/// Whether the gate is done with a row: it has the most runs, or it is at a look and both of
/// its tests decide.
fn decided(lengua: &Measured, other: &Measured) -> bool {
    let runs = other.cpu_ns.len();
    if runs >= gate::MAX_RUNS {
        return true;
    }
    if !gate::looks_at(runs) {
        return false;
    }
    let cpu = tested(&lengua.cpu_ns, &other.cpu_ns).0;
    let wall = tested(&lengua.wall_ns, &other.wall_ns).0;
    cpu != Verdict::Undecided && wall != Verdict::Undecided
}

/// The order a round runs `count` versions in, shuffled from the round's number `round`, so that
/// no version always runs after another and finds the caches as that one left them.
fn order(round: usize, count: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..count).collect();
    let seed = (round as u64 + 1).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let mut state = 0x9e37_79b9_7f4a_7c15_u64 ^ seed;
    for at in (1..count).rev() {
        state = next(state);
        order.swap(at, usize::try_from(state % (at as u64 + 1)).unwrap_or(0));
    }
    order
}

/// Runs the `versions` of `prepared` that are ready in rounds, keeping each one's counted runs
/// in `measured`, in the same order: the first version is lengua's, and each other is a row with
/// it. Each version's output is checked first, and [`WARM_UP`] rounds go uncounted; then every
/// round runs lengua and each version whose row is still open, in an order shuffled from round
/// to round. A row closes when the gate decides both of its tests at a look, or at the most runs;
/// every row closes once `seconds` have gone by.
fn rounds(
    runner: &Runner,
    prepared: &Prepared,
    versions: &[Option<Version>],
    measured: &mut [Measured],
    seconds: u64,
) {
    for (version, one) in versions.iter().zip(measured.iter_mut()) {
        if let Some(version) = version {
            match check(runner, prepared, version) {
                Ok(size) => one.size = size,
                Err(problem) => one.problem = Some(problem),
            }
        }
    }
    let mut open: Vec<bool> = measured.iter().map(|one| one.problem.is_none()).collect();
    let limit = Duration::from_secs(seconds);
    let mut started = Instant::now();
    let mut round = 0;
    while open.first() == Some(&true) && open.iter().skip(1).any(|row| *row) {
        let counted = round >= WARM_UP;
        if round == WARM_UP {
            started = Instant::now();
        }
        if counted && started.elapsed() >= limit {
            break;
        }
        let running: Vec<usize> = (0..versions.len()).filter(|at| open[*at]).collect();
        for at in order(round, running.len()) {
            let index = running[at];
            let Some(version) = &versions[index] else {
                continue;
            };
            match runner.run(prepared, version, false, false) {
                Ok(measures) if counted => measured[index].push(&measures),
                Ok(_) => {}
                Err(problem) => {
                    measured[index].problem = Some(problem);
                    open[index] = false;
                }
            }
        }
        if counted && let Some((lengua, others)) = measured.split_first() {
            for (row, other) in open.iter_mut().skip(1).zip(others) {
                if *row && decided(lengua, other) {
                    *row = false;
                }
            }
        }
        round += 1;
    }
}

/// lengua's run apart from its module on `prepared`: `lengua`, lengua's version, whose checked
/// run gave the size it holds, and `module`, lengua-module, run alone in rounds of their own, in
/// an order shuffled from round to round. lengua-module's output is checked first, as every
/// version's is; then [`WARM_UP`] rounds go uncounted, and counted rounds follow until each has
/// [`APART_RUNS`] runs, or until they have taken [`APART_TIME`] once each has [`APART_FEWEST`]. A
/// version that can't be measured, or gives another output, ends them, with why.
fn apart_rounds(
    runner: &Runner,
    prepared: &Prepared,
    (lengua, size): (&Version, u64),
    module: Result<Version, String>,
) -> Apart {
    let mut apart = Apart {
        lengua: Measured {
            name: lengua.name.clone(),
            made: lengua.made.clone(),
            size,
            ..Measured::default()
        },
        module: Measured {
            name: MODULE_VERSION.to_owned(),
            ..Measured::default()
        },
        seconds: 0.0,
    };
    let module = match module {
        Ok(module) => module,
        Err(problem) => {
            apart.module.problem = Some(problem);
            return apart;
        }
    };
    apart.module.made.clone_from(&module.made);
    match check(runner, prepared, &module) {
        Ok(checked) => apart.module.size = checked,
        Err(problem) => {
            apart.module.problem = Some(problem);
            return apart;
        }
    }
    let versions = [lengua, &module];
    let mut started = Instant::now();
    let mut round = 0;
    loop {
        let counted = round >= WARM_UP;
        if round == WARM_UP {
            started = Instant::now();
        }
        let runs = apart.lengua.cpu_ns.len().min(apart.module.cpu_ns.len());
        let timed_out = runs >= APART_FEWEST && started.elapsed() >= APART_TIME;
        if counted && (runs >= APART_RUNS || timed_out) {
            break;
        }
        for at in order(round, versions.len()) {
            let one = if at == 0 {
                &mut apart.lengua
            } else {
                &mut apart.module
            };
            match runner.run(prepared, versions[at], false, false) {
                Ok(measures) if counted => one.push(&measures),
                Ok(_) => {}
                Err(problem) => one.problem = Some(problem),
            }
        }
        if apart.lengua.problem.is_some() || apart.module.problem.is_some() {
            break;
        }
        round += 1;
    }
    apart.seconds = started.elapsed().as_secs_f64();
    apart
}

/// The medians of the CPU's counts of `version`'s reads that missed the L1 data cache, the
/// last-level cache and the data TLB, over [`COUNTER_RUNS`] runs of their own, on Linux, the one
/// system whose counters lengua reads.
fn misses(runner: &Runner, prepared: &Prepared, version: &Version) -> [Option<f64>; 3] {
    let mut counted: [Vec<f64>; 3] = Default::default();
    if cfg!(target_os = "linux") {
        for _ in 0..COUNTER_RUNS {
            let Ok(measures) = runner.run(prepared, version, false, true) else {
                continue;
            };
            let counts = [
                measures.l1d_misses,
                measures.llc_misses,
                measures.dtlb_misses,
            ];
            for (all, count) in counted.iter_mut().zip(counts) {
                all.extend(count.map(|count| count as f64));
            }
        }
    }
    counted.map(|all| gate::median(&all))
}

/// What the gate found on a row's measure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Decided {
    /// `ahead`, `level`, `behind` or `not measured`, as [`Verdict::name`] writes them.
    verdict: String,
    /// lengua's median, or its value.
    lengua: Option<f64>,
    /// The other version's.
    other: Option<f64>,
    /// The test's p-value, for CPU and wall time.
    p: Option<f64>,
}

impl Decided {
    /// A measure the run has no values for.
    fn unmeasured() -> Self {
        Self {
            verdict: Verdict::Unmeasured.name().to_owned(),
            ..Self::default()
        }
    }

    /// `measure`, one taken as measured, peak memory or size: lengua's value against the
    /// other's, with the gate's margin where the measure has it.
    fn measured(measure: Measure, lengua: f64, other: f64) -> Self {
        let verdict = gate::measured_verdict(measure, lengua, other);
        Self {
            verdict: verdict.name().to_owned(),
            lengua: Some(lengua),
            other: Some(other),
            p: None,
        }
    }

    /// A measure the test decides, over the first `runs` values of each: what the test found,
    /// held to the gate's margin by the two medians, and the test's own p.
    fn tested(lengua: &[u64], other: &[u64], runs: usize) -> Self {
        let (test, p) = tested(&lengua[..runs], &other[..runs]);
        let ours = gate::median(&floats(&lengua[..runs]));
        let theirs = gate::median(&floats(&other[..runs]));
        let verdict = match (ours, theirs) {
            (Some(ours), Some(theirs)) => gate::with_margin(test, ours, theirs),
            _ => Verdict::Unmeasured,
        };
        Self {
            verdict: verdict.name().to_owned(),
            lengua: ours,
            other: theirs,
            p: Some(p),
        }
    }
}

/// A row: lengua against one other version of a workload, on each measure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Row {
    /// The other version's name.
    versus: String,
    /// The runs of each version its tests took.
    runs: usize,
    /// What the gate found on CPU time, wall time, peak memory and size, in that order.
    measures: Vec<Decided>,
    /// Why the row wasn't measured, if it wasn't.
    problem: Option<String>,
}

/// The row of `other` against `lengua`, over the runs both have: CPU and wall time decided by
/// the test at a look's level and held to the gate's margin, peak memory by the median of each
/// one's peaks with the same margin, and size as it is, to the byte.
/// Against a native build, lengua's size is its module's and `runtime`'s, the bytes of the
/// lengua-mcp that runs it, as a native build needs nothing of lengua's to run; against another
/// module, which runs in the same lengua-mcp, it is the module's alone.
fn row(lengua: &Measured, other: &Measured, runtime: u64) -> Row {
    let runs = lengua.cpu_ns.len().min(other.cpu_ns.len());
    let problem = other.problem.clone().or_else(|| lengua.problem.clone());
    if problem.is_some() || runs == 0 {
        return Row {
            versus: other.name.clone(),
            runs,
            measures: vec![Decided::unmeasured(); Measure::ALL.len()],
            problem: problem.or_else(|| Some("no runs were counted".to_owned())),
        };
    }
    let peak = |measured: &Measured| {
        let peaks = &measured.peak_bytes;
        gate::median(&floats(&peaks[..runs.min(peaks.len())]))
    };
    let memory = match (peak(lengua), peak(other)) {
        (Some(ours), Some(theirs)) => Decided::measured(Measure::Memory, ours, theirs),
        _ => Decided::unmeasured(),
    };
    let size = if NATIVE.contains(&other.name.as_str()) {
        lengua.size.saturating_add(runtime)
    } else {
        lengua.size
    };
    Row {
        versus: other.name.clone(),
        runs,
        measures: vec![
            Decided::tested(&lengua.cpu_ns, &other.cpu_ns, runs),
            Decided::tested(&lengua.wall_ns, &other.wall_ns, runs),
            memory,
            Decided::measured(Measure::Size, size as f64, other.size as f64),
        ],
        problem: None,
    }
}

/// The rows of a workload's `versions`, lengua's first: one for each other version, with
/// `runtime`, lengua-mcp's bytes, in lengua's size against a native build.
fn rows_of(versions: &[Measured], runtime: u64) -> Vec<Row> {
    match versions.split_first() {
        Some((lengua, others)) => others
            .iter()
            .map(|other| row(lengua, other, runtime))
            .collect(),
        None => Vec::new(),
    }
}

/// What the run found on one workload.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Outcome {
    /// The workload's name.
    name: String,
    /// lengua's program, in the repository.
    program: String,
    /// What it does.
    about: String,
    /// How long its rounds took, in seconds.
    seconds: f64,
    /// Each version's runs, lengua's first.
    versions: Vec<Measured>,
    /// Each row, in the order of the versions.
    rows: Vec<Row>,
    /// lengua's run apart from its module, in rounds of their own after the gate's: none for a
    /// workload measured against an earlier commit, or in a results file from before they ran.
    #[serde(default)]
    apart: Option<Apart>,
}

/// lengua's run apart from its module, as a workload's rounds of their own measured it: lengua's
/// program, run as `lengua-mcp run` runs it, and lengua-module, the very module that run runs,
/// run as the other modules are.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Apart {
    /// lengua's runs in these rounds.
    lengua: Measured,
    /// lengua-module's.
    module: Measured,
    /// How long their counted rounds took, in seconds.
    seconds: f64,
}

/// lengua's programs today against the same programs at an earlier commit.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Against {
    /// The commit.
    commit: String,
    /// Each workload's: today's lengua program and then's, and one row, then's.
    workloads: Vec<Outcome>,
}

/// Everything a run measured and found, as target/bench/results-<target>.json keeps it: the
/// report is written from it alone.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Results {
    /// The runner, as xtask/targets.json names it.
    target: String,
    /// The commit measured.
    commit: String,
    /// Where the run is.
    run: String,
    /// The CPU, as the system names it.
    cpu: String,
    /// How many threads it runs at once.
    cores: usize,
    /// The WebAssembly engine.
    engine: String,
    /// Each other version's compilers and flags, by its name.
    toolchains: BTreeMap<String, String>,
    /// The bytes of the lengua-mcp executable that ran lengua's programs, which lengua's size
    /// counts against a native build.
    #[serde(default)]
    runtime: u64,
    /// The counts of runs at which the gate looks at a row.
    looks: Vec<usize>,
    /// The level each row's measure is decided at, over all its looks.
    alpha: f64,
    /// The level of each look.
    look_alpha: f64,
    /// The most runs of each version of a workload.
    max_runs: usize,
    /// The gate's noise margin, as a share of the other version's value; 0 in the results of a
    /// run from before the gate had one.
    #[serde(default)]
    margin: f64,
    /// The longest a workload's rows ran, in seconds.
    row_seconds: u64,
    /// The rounds after each version's checked run that weren't counted.
    warm_up: usize,
    /// The most counted runs each of lengua and lengua-module took in their rounds of their own.
    #[serde(default)]
    apart_runs: usize,
    /// The fewest counted runs each of them took before those rounds could stop on time.
    #[serde(default)]
    apart_fewest: usize,
    /// How long those counted rounds could go on once each had the fewest, in seconds.
    #[serde(default)]
    apart_seconds: u64,
    /// Whether the run measured every workload of the gate.
    every_workload: bool,
    /// Each workload's outcome.
    workloads: Vec<Outcome>,
    /// lengua's programs against an earlier commit's, when the run was asked for it.
    against: Option<Against>,
    /// Where the run and KNOWN_BEHIND disagree.
    ratchet: Vec<String>,
    /// Where KNOWN_BEHIND grew in the history of the commit measured, which it may not.
    #[serde(default)]
    grown: Vec<String>,
}

/// What measuring every workload shares.
struct Context {
    /// The repository.
    root: PathBuf,
    /// The folder the bench writes in, target/bench.
    out: PathBuf,
    /// The folder the native versions are built in.
    build: PathBuf,
    /// What runs and measures each run.
    runner: Runner,
    /// The folder of the WebAssembly builds.
    wasm: PathBuf,
    /// The longest a workload's rows run, in seconds.
    seconds: u64,
    /// The bytes of lengua-mcp's executable, which lengua's size counts against a native build.
    runtime: u64,
}

/// lengua's programs, each checked from its idea to done, or why it isn't, by workload.
type Checked = BTreeMap<&'static str, Result<Program, String>>;

/// The compiler of `language` on this system: GCC on Linux, Apple's clang on macOS, LLVM's clang
/// on Windows, and rustc everywhere.
fn compiler(language: &str) -> &'static str {
    match (language, std::env::consts::OS) {
        ("rust", _) => "rustc",
        ("c", "linux") => "gcc",
        ("cpp", "linux") => "g++",
        ("c", "macos") => "cc",
        ("cpp", "macos") => "c++",
        ("c", _) => "clang",
        _ => "clang++",
    }
}

/// The sets of flags to build `language`'s native version with, in the order to try them, each
/// with the libraries to link after the source: for speed on this CPU, as the system's
/// developers build. Where the compiler doesn't take the CPU's own name, a CPU flag that every
/// runner of its kind takes (AArch64's CRC instructions or x86-64's SSE4.2, which crc32c's
/// native versions need), and last, without link-time optimization. Every executable ships
/// without its symbols, on every system: C and C++ with `-s` on Linux and `-Wl,-x` on macOS
/// (Windows's linker keeps none in the executable), Rust with `strip=symbols`, which rustc runs
/// its system's tool for.
fn flag_sets(language: &str) -> Vec<(Vec<&'static str>, Vec<&'static str>)> {
    if language == "rust" {
        let flags = vec![
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
            "-C",
            "strip=symbols",
        ];
        return vec![(flags, Vec::new())];
    }
    let cpus: &[&str] = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", _) => &["-mcpu=native", "-mcpu=apple-m1"],
        (_, "aarch64") => &["-march=native", "-mcpu=native", "-march=armv8-a+crc"],
        _ => &["-march=native", "-msse4.2"],
    };
    let (fast, libraries): (&[&str], &[&str]) = match std::env::consts::OS {
        "linux" => (&["-O3", "-flto", "-s"], &["-lm"]),
        "macos" => (&["-O3", "-flto", "-Wl,-x"], &[]),
        _ => (&["-O3", "-flto", "-fuse-ld=lld"], &["-lws2_32"]),
    };
    let plain: &[&str] = &["-O3"];
    let mut sets = Vec::new();
    for flags in [fast, plain] {
        for cpu in cpus {
            let mut set = flags.to_vec();
            set.push(*cpu);
            sets.push((set, libraries.to_vec()));
        }
    }
    sets
}

/// The first line `program` prints with `args`, or that it says nothing.
fn first_line(program: &str, args: &[&str]) -> String {
    let Ok(output) = Command::new(program).args(args).output() else {
        return format!("{program}, which doesn't start here");
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next().map(str::to_owned);
    line.unwrap_or_else(|| format!("{program}, whose version it doesn't say"))
}

/// Builds `source` into `executable` with `compiler`, trying each of `language`'s sets of flags
/// in turn: gives the flags that built it, or what the compiler said of the last.
fn build(
    compiler: &str,
    language: &str,
    source: &Path,
    executable: &Path,
) -> Result<String, String> {
    let mut said = String::new();
    for (flags, libraries) in flag_sets(language) {
        let mut command = Command::new(compiler);
        command.args(&flags).arg("-o").arg(executable);
        command.arg(source).args(&libraries);
        let output = command.output().map_err(|error| failed(compiler, error))?;
        if output.status.success() {
            let used: Vec<&str> = flags.iter().chain(&libraries).copied().collect();
            return Ok(used.join(" "));
        }
        said = String::from_utf8_lossy(&output.stderr).into_owned();
    }
    let (source, said) = (source.display(), said.trim());
    Err(format!("{compiler} couldn't build {source}: {said}"))
}

/// The native version of workload `name` in `language`, from `file`, built in `folder` with
/// this system's compiler, whose version `toolchains` keeps.
fn native(
    root: &Path,
    name: &str,
    (language, file): (&str, &str),
    folder: &Path,
    toolchains: &mut BTreeMap<String, String>,
) -> Result<Version, String> {
    let compiler = compiler(language);
    let source = root.join("benches").join("programs").join(name).join(file);
    let suffix = std::env::consts::EXE_SUFFIX;
    let executable = folder.join(format!("{name}-{language}{suffix}"));
    let flags = build(compiler, language, &source, &executable)?;
    let version = first_line(compiler, &["--version"]);
    let made = format!("{version}; {compiler} {flags}");
    toolchains
        .entry(language.to_owned())
        .or_insert_with(|| made.clone());
    Ok(Version {
        name: language.to_owned(),
        program: executable,
        made,
    })
}

/// The WebAssembly builds of workload `name`, from `folder`, as `cargo xtask bench-wasm` made them.
fn modules(folder: &Path, name: &str, toolchains: &BTreeMap<String, String>) -> Vec<Made> {
    let mut made = Vec::new();
    for (language, _) in LANGUAGES {
        let version = format!("{language}-wasm");
        let module = folder.join(name).join(format!("{language}.wasm"));
        let ready = if module.is_file() {
            Ok(Version {
                name: version.clone(),
                program: module,
                made: toolchains.get(&version).cloned().unwrap_or_default(),
            })
        } else {
            Err(format!(
                "no module {}: cargo xtask bench-wasm builds it, in the folder that \
                 LENGUA_BENCH_WASM names",
                module.display()
            ))
        };
        made.push(Made {
            name: version,
            version: ready,
        });
    }
    made
}

/// lengua's version `name` of workload `program`, from its checked folder, or why there is
/// none; `commit` says where its files are from, when not from today's.
fn lengua_made(
    name: &str,
    checked: Option<&Result<Program, String>>,
    program: &str,
    commit: &str,
) -> Made {
    let at = if commit.is_empty() {
        String::new()
    } else {
        format!(" at {commit}")
    };
    let (release, engine) = (env!("CARGO_PKG_VERSION"), lengua::wasm::engine::ENGINE);
    let version = match checked {
        Some(Ok(checked)) => Ok(Version {
            name: name.to_owned(),
            program: checked.path().to_path_buf(),
            made: format!("lengua-mcp {release} run, {engine}, {program}{at}"),
        }),
        Some(Err(problem)) => Err(format!("lengua's program{at} isn't done: {problem}")),
        None => Err(format!("lengua's program{at} wasn't checked")),
    };
    Made {
        name: name.to_owned(),
        version,
    }
}

/// lengua-module's bytes for lengua's checked program `checked`: the module lengua's build makes
/// from its main.wat, which `lengua-mcp run` runs, or why there are none.
fn lengua_module(checked: Option<&Result<Program, String>>) -> Result<Vec<u8>, String> {
    let program = match checked {
        Some(Ok(program)) => program,
        Some(Err(problem)) => return Err(format!("lengua's program isn't done: {problem}")),
        None => return Err("lengua's program wasn't checked".to_owned()),
    };
    let file = program.file("main.wat");
    let code = std::fs::read_to_string(&file).map_err(|error| failed(file.display(), error))?;
    lengua::wasm::text::build(&code)
        .map(|built| built.wasm)
        .map_err(|problem| failed(file.display(), problem))
}

/// lengua-module, ready to run: `bytes`, its module, written to a file in `place`, beside the
/// workload's working folder and never in it, or why it couldn't be.
fn module_version(place: &Path, bytes: Result<Vec<u8>, String>) -> Result<Version, String> {
    let file = place.join(MODULE_FILE);
    std::fs::write(&file, bytes?).map_err(|error| failed(file.display(), error))?;
    let (release, engine) = (env!("CARGO_PKG_VERSION"), lengua::wasm::engine::ENGINE);
    Ok(Version {
        name: MODULE_VERSION.to_owned(),
        program: file,
        made: format!(
            "the module lengua-mcp {release} builds from lengua's main.wat, run in {engine} as \
             the other modules are"
        ),
    })
}

/// Measures the `made` versions of `prepared` that are ready, lengua's first, and gives each
/// one's runs in the same order, with how long their rounds took: a version that isn't ready
/// gets only why, and when lengua's isn't, no version runs. Given `module`, lengua-module, once
/// the gate's rounds are over and when lengua's version ran in them to the end, lengua and
/// lengua-module run in rounds of their own, which the third value gives.
fn measure_versions(
    context: &Context,
    prepared: &Prepared,
    made: Vec<Made>,
    module: Option<Result<Version, String>>,
) -> (Vec<Measured>, f64, Option<Apart>) {
    let mut measured = Vec::new();
    let mut versions = Vec::new();
    for made in made {
        let mut one = Measured {
            name: made.name,
            ..Measured::default()
        };
        match made.version {
            Ok(version) => {
                one.made.clone_from(&version.made);
                versions.push(Some(version));
            }
            Err(problem) => {
                one.problem = Some(problem);
                versions.push(None);
            }
        }
        measured.push(one);
    }
    let started = Instant::now();
    if versions.first().is_some_and(Option::is_some) {
        let runner = &context.runner;
        rounds(runner, prepared, &versions, &mut measured, context.seconds);
    }
    let seconds = started.elapsed().as_secs_f64();
    // lengua's run apart from its module, after the gate's rounds, which run as they would
    // without it.
    let lengua = versions.first().and_then(Option::as_ref);
    let ran = measured
        .first()
        .filter(|one| one.problem.is_none())
        .map(|one| one.size);
    let apart = match (module, lengua, ran) {
        (Some(module), Some(lengua), Some(size)) => Some(apart_rounds(
            &context.runner,
            prepared,
            (lengua, size),
            module,
        )),
        _ => None,
    };
    for (version, one) in versions.iter().zip(&mut measured) {
        if let Some(version) = version
            && one.problem.is_none()
        {
            one.misses = misses(&context.runner, prepared, version);
        }
    }
    (measured, seconds, apart)
}

/// Measures the `made` versions of workload `name` on its input, made in a folder of its own, and
/// with `module`, lengua-module's bytes, lengua's run apart from its module.
fn outcome_of(
    context: &Context,
    name: &str,
    made: Vec<Made>,
    module: Option<Result<Vec<u8>, String>>,
) -> Outcome {
    let workload = workload(name).expect("each of the gate's workloads has its input here");
    let program = program_of(name).to_owned();
    let idea = std::fs::read_to_string(context.root.join(&program).join("idea.md"));
    let mut outcome = Outcome {
        name: name.to_owned(),
        program,
        about: workload.about.to_owned(),
        ..Outcome::default()
    };
    let place = tempfile::tempdir_in(&context.out).expect("a folder for the workload");
    match prepare(workload, place.path(), &idea.unwrap_or_default()) {
        Ok(prepared) => {
            let module = module.map(|bytes| module_version(place.path(), bytes));
            let (versions, seconds, apart) = measure_versions(context, &prepared, made, module);
            outcome.versions = versions;
            outcome.seconds = seconds;
            outcome.apart = apart;
        }
        Err(problem) => {
            for made in made {
                outcome.versions.push(Measured {
                    name: made.name,
                    problem: Some(problem.clone()),
                    ..Measured::default()
                });
            }
        }
    }
    outcome.rows = rows_of(&outcome.versions, context.runtime);
    outcome
}

/// Measures workload `name` on this runner: lengua's program, `checked`, against the WebAssembly
/// builds and the native versions, built here; then lengua's run apart from its module.
fn measure_workload(
    context: &Context,
    name: &str,
    checked: Option<&Result<Program, String>>,
    toolchains: &mut BTreeMap<String, String>,
) -> Outcome {
    let mut made = vec![lengua_made("lengua", checked, program_of(name), "")];
    made.extend(modules(&context.wasm, name, toolchains));
    for language in LANGUAGES {
        let version = native(&context.root, name, language, &context.build, toolchains);
        made.push(Made {
            name: language.0.to_owned(),
            version,
        });
    }
    outcome_of(context, name, made, Some(lengua_module(checked)))
}

/// lengua's programs today against the same programs at `commit`, each workload's two in rounds
/// of their own: on that row, ahead is today's lower.
fn measure_against(
    context: &Context,
    commit: &str,
    names: &[&'static str],
    (now, then): (&Checked, &Checked),
) -> Against {
    let mut workloads = Vec::new();
    for name in names {
        let program = program_of(name);
        let made = vec![
            lengua_made("lengua", now.get(name), program, ""),
            lengua_made("then", then.get(name), program, commit),
        ];
        workloads.push(outcome_of(context, name, made, None));
    }
    Against {
        commit: commit.to_owned(),
        workloads,
    }
}

/// lengua's program from `idea` and `code`, taken from the user's request to done in `session`
/// as an agent takes it: its idea recorded, then its main.wat checked until its tests end. Gives
/// its folder, which `run` then runs, or why it isn't done.
async fn checked(session: &Session, idea: &str, code: &str) -> Result<Program, String> {
    let program = Program::new();
    program.write("idea.md", idea);
    let recorded = session.record(&program).await;
    if recorded["state"] != "write_code" {
        return Err(format!("its idea wasn't recorded: {recorded}"));
    }
    program.write("main.wat", code);
    let done = session.settle(&program).await;
    if done["state"] != "done" {
        return Err(format!("check didn't say done: {done}"));
    }
    Ok(program)
}

/// lengua's program in the repository's folder `path`, taken from its idea to done.
async fn checked_here(session: &Session, root: &Path, path: &str) -> Result<Program, String> {
    let read = |file: &str| {
        let file = root.join(path).join(file);
        std::fs::read_to_string(&file).map_err(|error| failed(file.display(), error))
    };
    checked(session, &read("idea.md")?, &read("main.wat")?).await
}

/// What git prints when it runs with `args` in the repository `root`, or nothing when it fails.
fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    output.status.success().then_some(text)
}

/// The file `path` of the repository as commit `commit` had it, if it had it.
fn at_commit(root: &Path, commit: &str, path: &str) -> Option<String> {
    let object = format!("{commit}:{path}");
    git(root, &["show", object.as_str()])
}

/// Where `after`, KNOWN_BEHIND as a change left it, names a row that `before`, the list the
/// change replaced, didn't, with the names the gate had before the change: those `code`, its
/// src/gate.rs then, quotes.
fn grown_by(before: &str, after: &str, code: &str) -> Vec<String> {
    let existed = |name: &str| code.contains(&format!("\"{name}\""));
    let (before, after) = (KnownBehind::read(before), KnownBehind::read(after));
    gate::grown(&before, &after, existed)
}

/// Where KNOWN_BEHIND grew, which it may not: each change to the list on the first parents of
/// the commit measured, held to the list it replaced, and the list as it is now, `now`, held to
/// HEAD's. A clone without the history can't be checked, which fails the run.
fn grown_in_history(root: &Path, now: &str) -> Vec<String> {
    let shallow = git(root, &["rev-parse", "--is-shallow-repository"]);
    if shallow.as_deref().map(str::trim) != Some("false") {
        let problem = "KNOWN_BEHIND's history can't be read: the bench needs git and the whole \
                       history, not a shallow clone";
        return vec![problem.to_owned()];
    }
    let log = ["log", "--first-parent", "--format=%H", "--", "KNOWN_BEHIND"];
    let Some(changes) = git(root, &log) else {
        return vec!["git can't list the changes to KNOWN_BEHIND".to_owned()];
    };
    let mut grown = Vec::new();
    if let Some(head) = at_commit(root, "HEAD", "KNOWN_BEHIND")
        && head != now
    {
        let code = at_commit(root, "HEAD", "src/gate.rs").unwrap_or_default();
        for problem in grown_by(&head, now, &code) {
            grown.push(format!("not committed: {problem}"));
        }
    }
    // Each commit's hash, newest first.
    for commit in changes.split_whitespace() {
        let parent = format!("{commit}^");
        let before = at_commit(root, &parent, "KNOWN_BEHIND");
        let after = at_commit(root, commit, "KNOWN_BEHIND");
        // The commit that made the list, or one that took it away, replaced no list.
        let (Some(before), Some(after)) = (before, after) else {
            continue;
        };
        let code = at_commit(root, &parent, "src/gate.rs").unwrap_or_default();
        for problem in grown_by(&before, &after, &code) {
            grown.push(format!("{commit}: {problem}"));
        }
    }
    grown
}

/// lengua's program in the folder `path` as commit `commit` had it, taken from its idea to done.
/// Before W11 each was in tests/fixtures-wasm, under the last name of its folder now, and `path`
/// held the native program or nothing: a commit with no main.wat in `path` has it there.
async fn checked_at(
    session: &Session,
    root: &Path,
    commit: &str,
    path: &str,
) -> Result<Program, String> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let before = format!("tests/fixtures-wasm/{name}");
    let has = |folder: &str| at_commit(root, commit, &format!("{folder}/main.wat")).is_some();
    let folder = if !has(path) && has(&before) {
        before.as_str()
    } else {
        path
    };
    let read = |file: &str| {
        let shown = at_commit(root, commit, &format!("{folder}/{file}"));
        shown.ok_or_else(|| format!("{commit} has no {folder}/{file}"))
    };
    checked(session, &read("idea.md")?, &read("main.wat")?).await
}

/// The value of the environment variable `name`, when it says something.
fn setting(name: &str) -> Option<String> {
    let value = std::env::var(name).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// The workloads to measure: those `LENGUA_BENCH_ONLY` names, or every one.
fn chosen() -> Result<Vec<&'static str>, String> {
    let Some(only) = setting("LENGUA_BENCH_ONLY") else {
        return Ok(gate::WORKLOADS.to_vec());
    };
    let mut names = Vec::new();
    let listed = only.split(',').map(str::trim);
    for name in listed.filter(|name| !name.is_empty()) {
        let Some(known) = gate::WORKLOADS.iter().find(|known| **known == name) else {
            let all = gate::WORKLOADS.join(", ");
            return Err(format!("LENGUA_BENCH_ONLY: {name:?} isn't one of {all}"));
        };
        names.push(*known);
    }
    Ok(names)
}

/// The commit measured, and where the run is: from GitHub's variables in CI, else from git.
fn provenance(root: &Path) -> (String, String) {
    let commit = setting("GITHUB_SHA").unwrap_or_else(|| {
        let shown = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output();
        let text = shown.map(|shown| String::from_utf8_lossy(&shown.stdout).trim().to_owned());
        text.unwrap_or_default()
    });
    let place = [
        setting("GITHUB_SERVER_URL"),
        setting("GITHUB_REPOSITORY"),
        setting("GITHUB_RUN_ID"),
    ];
    let run = match place {
        [Some(server), Some(repository), Some(id)] => {
            format!("{server}/{repository}/actions/runs/{id}")
        }
        _ => "a run outside CI".to_owned(),
    };
    (commit, run)
}

/// The CPU's model, as the system names it.
fn cpu_model() -> String {
    if let Some(model) = lengua::platform::Machine::detect().cpu_model {
        return model;
    }
    if cfg!(target_os = "macos") {
        return first_line("sysctl", &["-n", "machdep.cpu.brand_string"]);
    }
    setting("PROCESSOR_IDENTIFIER").unwrap_or_else(|| "a CPU the system doesn't name".to_owned())
}

/// The WebAssembly builds' compilers and flags, from toolchains.tsv in `folder` (which
/// `cargo xtask bench-wasm` writes).
fn wasm_toolchains(folder: &Path) -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(folder.join("toolchains.tsv")).unwrap_or_default();
    let mut toolchains = BTreeMap::new();
    for line in text.lines() {
        if let Some((name, rest)) = line.split_once('\t') {
            toolchains.insert(name.to_owned(), rest.replace('\t', "; "));
        }
    }
    toolchains
}

/// The verdict `name` names; a word that names none is a measure not measured.
fn verdict_named(name: &str) -> Verdict {
    Verdict::from_name(name).unwrap_or(Verdict::Unmeasured)
}

/// What the run found on each row and measure of `results`, as the ratchet takes it.
fn found(results: &Results) -> Vec<Found> {
    let mut found = Vec::new();
    for outcome in &results.workloads {
        for row in &outcome.rows {
            for (measure, decided) in Measure::ALL.into_iter().zip(&row.measures) {
                found.push(Found {
                    workload: outcome.name.clone(),
                    versus: row.versus.clone(),
                    measure,
                    verdict: verdict_named(&decided.verdict),
                });
            }
        }
    }
    found
}

/// The lines of KNOWN_BEHIND that `results`, which found `found`, pass with, and what they are:
/// the rows behind by more than the margin, and the rows not measured that `known`, the list as
/// it is, names.
fn proposal(known: &KnownBehind, results: &Results, found: &[Found]) -> String {
    let Results {
        target,
        commit,
        run,
        ..
    } = results;
    let margin = percent(results.margin);
    let mut text = format!(
        "# The lines of KNOWN_BEHIND for {target} that the run passes with ({run}, at {commit}): \
         a line for\n# each row and measure where lengua is behind by more than the gate's \
         margin of {margin}, and the\n# line the list has for each row the run couldn't measure. \
         A level row has no line. They take\n# the place of KNOWN_BEHIND's lines for {target}.\n"
    );
    if !results.every_workload {
        let workloads = &results.workloads;
        let measured: Vec<&str> = workloads.iter().map(|one| one.name.as_str()).collect();
        let measured = measured.join(", ");
        let _ = writeln!(text, "# Only these workloads were measured: {measured}.");
    }
    text.push_str(&gate::proposal(known, target, found));
    text
}

/// A share as a percentage: 0.05 is `5%`.
fn percent(share: f64) -> String {
    format!("{:.0}%", share * 100.0)
}

/// A number to three figures.
fn figures(value: f64) -> String {
    if value >= 100.0 {
        format!("{value:.0}")
    } else if value >= 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    }
}

/// A time in nanoseconds, to three figures in the unit that suits it.
fn duration(nanoseconds: f64) -> String {
    let (value, unit) = if nanoseconds >= 1e9 {
        (nanoseconds / 1e9, "s")
    } else if nanoseconds >= 1e6 {
        (nanoseconds / 1e6, "ms")
    } else if nanoseconds >= 1e3 {
        (nanoseconds / 1e3, "µs")
    } else {
        (nanoseconds, "ns")
    };
    format!("{} {unit}", figures(value))
}

/// A count of bytes, to three figures in the unit that suits it.
fn bytes(count: f64) -> String {
    if count >= 1_048_576.0 {
        format!("{} MiB", figures(count / 1_048_576.0))
    } else if count >= 1024.0 {
        format!("{} KiB", figures(count / 1024.0))
    } else {
        format!("{count:.0} B")
    }
}

/// A whole number with a comma between each three digits.
fn thousands(value: f64) -> String {
    let digits = format!("{value:.0}");
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A p-value, to two figures.
fn p_value(p: f64) -> String {
    if p < 0.001 {
        format!("{p:.1e}")
    } else {
        format!("{p:.3}")
    }
}

/// What the report shows where there is no number.
fn dash() -> String {
    "–".to_owned()
}

/// The median of `values`, shown by `show`, or a dash when there are none.
fn median_shown(values: &[u64], show: fn(f64) -> String) -> String {
    gate::median(&floats(values)).map_or_else(dash, show)
}

/// A verdict's mark in the gate's table.
fn mark(verdict: &str) -> &'static str {
    match verdict {
        "ahead" => "✓",
        "behind" => "✗",
        "level" => "≈",
        // The results of a run from before the gate had its margin.
        "undecided" => "?",
        _ => "–",
    }
}

/// A row's marks in the gate's table: CPU time, wall time, peak memory and size, in that order.
fn marks(row: Option<&Row>) -> String {
    let Some(row) = row else {
        return dash();
    };
    let measures = &row.measures;
    let marks: Vec<&str> = measures.iter().map(|one| mark(&one.verdict)).collect();
    marks.join(" ")
}

/// A row's measure as the report shows it: the verdict, lengua's value over the other's, and
/// the test's p where a test decided it.
fn cell(decided: &Decided) -> String {
    let ratio = match (decided.lengua, decided.other) {
        (Some(lengua), Some(other)) if other > 0.0 => format!(", {:.2}×", lengua / other),
        _ => String::new(),
    };
    let p = decided.p.map(p_value);
    let p = p.map_or_else(String::new, |p| format!(", p {p}"));
    format!("{}{ratio}{p}", decided.verdict)
}

/// lengua's value over the best native build's on the measure at `at` of `outcome`, or a dash.
fn gap(outcome: &Outcome, at: usize) -> String {
    let mut best: Option<(f64, f64)> = None;
    for row in &outcome.rows {
        let Some(decided) = row.measures.get(at) else {
            continue;
        };
        let (Some(lengua), Some(other)) = (decided.lengua, decided.other) else {
            continue;
        };
        let native = NATIVE.contains(&row.versus.as_str());
        if native && best.is_none_or(|(_, least)| other < least) {
            best = Some((lengua, other));
        }
    }
    match best {
        Some((lengua, other)) if other > 0.0 => format!("{:.2}×", lengua / other),
        _ => dash(),
    }
}

/// The rows of a table of `outcomes`, a row and its measures to a line: each outcome's rows
/// under its name.
fn row_lines(out: &mut String, outcomes: &[Outcome]) {
    out.push_str("| workload | against | runs | CPU time | wall time | peak memory | size |\n");
    out.push_str("|---|---|---|---|---|---|---|\n");
    for outcome in outcomes {
        for row in &outcome.rows {
            let cells: Vec<String> = row.measures.iter().map(cell).collect();
            let (name, versus, runs) = (&outcome.name, &row.versus, row.runs);
            let cells = cells.join(" | ");
            let _ = writeln!(out, "| {name} | {versus} | {runs} | {cells} |");
        }
    }
}

/// A workload's section of the report: what it is, each version's medians, and each row.
fn workload_section(out: &mut String, outcome: &Outcome) {
    let (name, program, about) = (&outcome.name, &outcome.program, &outcome.about);
    let seconds = outcome.seconds.round();
    let _ = writeln!(out, "\n## {name}\n");
    let _ = writeln!(out, "{about} lengua's program: {program}.\n");
    let versions = &outcome.versions;
    let names: Vec<&str> = versions.iter().map(|one| one.name.as_str()).collect();
    let _ = writeln!(out, "| | {} |", names.join(" | "));
    let _ = writeln!(out, "|---|{}", "---|".repeat(names.len()));
    let lines: [TableLine; 5] = [
        ("counted runs", |one| one.cpu_ns.len().to_string()),
        ("CPU time", |one| median_shown(&one.cpu_ns, duration)),
        ("wall time", |one| median_shown(&one.wall_ns, duration)),
        ("peak memory", |one| median_shown(&one.peak_bytes, bytes)),
        ("size", |one| bytes(one.size as f64)),
    ];
    for (label, show) in lines {
        let cells: Vec<String> = versions.iter().map(show).collect();
        let _ = writeln!(out, "| {label} | {} |", cells.join(" | "));
    }
    let counters = ["L1 data misses", "last-level misses", "data TLB misses"];
    for (at, label) in counters.iter().enumerate() {
        let counts: Vec<Option<f64>> = versions.iter().map(|one| one.misses[at]).collect();
        if counts.iter().all(Option::is_none) {
            continue;
        }
        let cells: Vec<String> = counts
            .iter()
            .map(|count| count.map_or_else(dash, thousands))
            .collect();
        let _ = writeln!(out, "| {label} | {} |", cells.join(" | "));
    }
    let _ = writeln!(out, "\nIts rounds took {seconds} seconds.");
    for measured in versions {
        let (name, made) = (&measured.name, &measured.made);
        if !made.is_empty() {
            let _ = writeln!(out, "\n- {name}: {made}");
        }
        if let Some(problem) = &measured.problem {
            let _ = writeln!(out, "\n- {name} wasn't measured to the end: {problem}");
        }
    }
}

/// `value` shown by `show`, with its sign.
fn signed(value: f64, show: fn(f64) -> String) -> String {
    if value < 0.0 {
        format!("-{}", show(-value))
    } else {
        format!("+{}", show(value))
    }
}

/// A cell of the section on lengua's run apart from its module: lengua's median against
/// lengua-module's, then the shift from lengua-module's runs to lengua's with its 95% interval
/// ([`hodges_lehmann`]), each shown by `show`; a dash where either has no values.
fn shift_cell(lengua: &[u64], module: &[u64], show: fn(f64) -> String) -> String {
    let medians = (gate::median(&floats(lengua)), gate::median(&floats(module)));
    let (Some(ours), Some(theirs)) = medians else {
        return dash();
    };
    let Some((shift, low, high)) = hodges_lehmann(lengua, module) else {
        return dash();
    };
    let (ours, theirs) = (show(ours), show(theirs));
    let (shift, low, high) = (signed(shift, show), signed(low, show), signed(high, show));
    format!("{ours} against {theirs}: {shift} ({low} to {high})")
}

/// A cell of that section from means, for CPU time counted in a clock's ticks: lengua's mean
/// against lengua-module's, then the difference of the means with its 95% interval ([`welch`]),
/// each shown by `show`; a dash where either has fewer than two values.
fn means_cell(lengua: &[u64], module: &[u64], show: fn(f64) -> String) -> String {
    let (Some(ours), Some(theirs)) = (mean(lengua), mean(module)) else {
        return dash();
    };
    let Some((shift, low, high)) = welch(lengua, module) else {
        return dash();
    };
    let (ours, theirs) = (show(ours), show(theirs));
    let (shift, low, high) = (signed(shift, show), signed(low, show), signed(high, show));
    format!("means {ours} against {theirs}: {shift} ({low} to {high})")
}

/// The report's section on lengua's run apart from its module, when the run measured it: on each
/// workload, lengua's program and lengua-module from their rounds of their own, and the shift
/// from lengua-module's runs to lengua's on CPU time, wall time and peak memory.
fn apart_section(out: &mut String, results: &Results) {
    let workloads: Vec<(&str, &Apart)> = results
        .workloads
        .iter()
        .filter_map(|outcome| Some((outcome.name.as_str(), outcome.apart.as_ref()?)))
        .collect();
    if workloads.is_empty() {
        return;
    }
    let Results {
        warm_up,
        apart_runs,
        apart_fewest,
        apart_seconds,
        ..
    } = results;
    let _ = writeln!(out, "\n## lengua's run apart from its module\n");
    let _ = writeln!(
        out,
        "After the gate's rounds, each workload's lengua program ran against lengua-module: the \
         very module that `lengua-mcp run` runs, built by lengua from the same main.wat and run \
         by `lengua-mcp measure` as the other modules are, in the same engine with the same \
         cache of compiled modules and with the words of the program's idea. The two ran alone, \
         in rounds of their own, in an order shuffled from round to round, after {warm_up} \
         rounds that weren't counted, until each had {apart_runs} runs, or for {apart_seconds} \
         seconds once each had {apart_fewest}. Each cell gives lengua's median against \
         lengua-module's, then the shift from lengua-module's runs to lengua's: the median of \
         every difference of a lengua run less a lengua-module run (Hodges and Lehmann's \
         estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what \
         lengua's run does before and around running its module.\n"
    );
    let ticks = results.target.starts_with("windows");
    if ticks {
        let _ = writeln!(
            out,
            "Windows counts CPU time in ticks of 15.6 ms, so on a short run each version's \
             median is a whole number of ticks, and a shift of medians is none or a whole tick. \
             The CPU time cells give means instead: each run's ticks sample its CPU time, so a \
             mean converges on it. A cell gives lengua's mean against lengua-module's, then the \
             difference of the means, with its 95% interval (1.96 standard errors either side, \
             Welch's). Wall time and peak memory give medians, as on every runner.\n"
        );
    }
    out.push_str("| workload | runs | CPU time | wall time | peak memory |\n");
    out.push_str("|---|---|---|---|---|\n");
    for (name, apart) in &workloads {
        let (lengua, module) = (&apart.lengua, &apart.module);
        let (counted, seconds) = (lengua.cpu_ns.len(), apart.seconds.round());
        let runs = format!("{counted}, {} in {seconds} s", module.cpu_ns.len());
        let cpu = if ticks {
            means_cell(&lengua.cpu_ns, &module.cpu_ns, duration)
        } else {
            shift_cell(&lengua.cpu_ns, &module.cpu_ns, duration)
        };
        let wall = shift_cell(&lengua.wall_ns, &module.wall_ns, duration);
        let peak = shift_cell(&lengua.peak_bytes, &module.peak_bytes, bytes);
        let _ = writeln!(out, "| {name} | {runs} | {cpu} | {wall} | {peak} |");
    }
    let _ = writeln!(
        out,
        "\nThe runs are lengua's and lengua-module's counted runs, in that order, and how long \
         their counted rounds took."
    );
    for (name, apart) in &workloads {
        for measured in [&apart.lengua, &apart.module] {
            if let Some(problem) = &measured.problem {
                let version = &measured.name;
                let _ = writeln!(
                    out,
                    "\n- {name}, {version}, wasn't measured to the end: {problem}"
                );
            }
        }
    }
}

/// BENCHMARKS-<target>.md, written from `results` alone: every number in it is the run's.
fn report(results: &Results) -> String {
    let Results {
        target,
        commit,
        run,
        cpu,
        cores,
        engine,
        alpha,
        look_alpha,
        max_runs,
        row_seconds,
        warm_up,
        ..
    } = results;
    let margin = percent(results.margin);
    let (low, high) = (1.0 - results.margin, 1.0 + results.margin);
    let looks: Vec<String> = results.looks.iter().map(ToString::to_string).collect();
    let looks = looks.join(", ");
    let mut out = String::new();
    let _ = writeln!(out, "# lengua against C, C++ and Rust on {target}\n");
    let _ = writeln!(
        out,
        "Measured on {target} ({cpu}, {cores} threads at once) by `cargo bench --bench \
         against_rust_and_c`, at commit {commit}, in {run}. Every number here is that run's: \
         the bench writes this file from the run's results alone, \
         target/bench/results-{target}.json in its artifacts.\n"
    );
    let _ = writeln!(
        out,
        "Every version of a workload does the same work in its own language's way: lengua's \
         program in the fastest form lengua's dialect allows, with SIMD128 written by hand where \
         the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, \
         written for their compilers to vectorize, with no SIMD written by hand. Each workload \
         below says how each version does its work.\n"
    );
    let _ = writeln!(
        out,
        "Every version of a workload gets the same arguments, input and files, and gives the \
         same output, checked before anything is measured. Each runs as people run it, in a \
         process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and \
         in the kernel) and its peak resident memory as the system counts them for the process, \
         its wall time on the clock, and the size of what ran. lengua's program runs through \
         `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly \
         run in lengua's host, in the same engine ({engine}) with the same cache of compiled \
         modules; their native builds, made on this runner, run as a shell starts them. Linux \
         begins a process's count of its peak memory at what the process it was started from \
         held, so there every version starts from lengua-spawn, a small process of lengua's \
         that forks it and counts it, and writes the counts and the version's time on the clock \
         for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at \
         lengua-mcp's megabytes.\n"
    );
    let runtime = bytes(results.runtime as f64);
    let _ = writeln!(
        out,
        "Size is a module's bytes on the rows against other modules, which run in the same \
         lengua-mcp. Against a native build, lengua's size is its module's and that of the \
         lengua-mcp it needs to run ({runtime}), and the native build's is its executable's \
         alone: what it loads from the system's libraries, C's C library and C++'s where they \
         are linked dynamically, isn't counted, and Rust's standard library, linked in, is.\n"
    );
    let _ = writeln!(
        out,
        "The runs alternate: each round runs every version once, in an order shuffled from \
         round to round, after each version's checked run and {warm_up} rounds that aren't \
         counted. On each row, lengua against one other version, the gate decides CPU and wall \
         time with a one-sided Mann-Whitney U test at p < {alpha}: it looks at the row after \
         {looks} runs of each version, each look deciding at p < {look_alpha} ({alpha} split \
         over the looks), until both of the row's tests decide, up to {max_runs} runs or \
         {row_seconds} seconds. Peak memory, the median of the runs' peaks, and size are \
         compared as measured.\n"
    );
    let _ = writeln!(
        out,
        "The gate has a noise margin of {margin}, as the runners are shared machines and a row \
         this close changes sides from one run to the next. A measure is level when the test \
         doesn't decide it (its p isn't below {look_alpha}), or when lengua's value is from \
         {low:.2} to {high:.2} times the other version's, whatever the test decides; for peak \
         memory, when the two medians are that close. It is ahead or behind only past the \
         margin, and for CPU and wall time only when the test decides it too. Size is the same \
         on every run and has no margin: a byte more is behind. KNOWN_BEHIND names the rows \
         where lengua is behind; a level row needs no line there and doesn't fail the gate. \
         Every ratio and every p is printed below, whatever the verdict.\n"
    );
    let _ = writeln!(out, "The versions:\n");
    let _ = writeln!(out, "- lengua: lengua-mcp, run in {engine}");
    for (name, toolchain) in &results.toolchains {
        let _ = writeln!(out, "- {name}: {toolchain}");
    }
    let _ = writeln!(out, "\n## The gate\n");
    let _ = writeln!(
        out,
        "lengua against each version, on CPU time, wall time, peak memory and size, in that \
         order: ✓ lengua is ahead by more than {margin}, ✗ behind by more than {margin}, ≈ level \
         (within {margin}, or not decided), – not measured.\n"
    );
    let _ = writeln!(out, "| workload | {} |", gate::VERSUS.join(" | "));
    let _ = writeln!(out, "|---|{}", "---|".repeat(gate::VERSUS.len()));
    for outcome in &results.workloads {
        let row = |versus: &str| outcome.rows.iter().find(|one| one.versus == versus);
        let cells = gate::VERSUS.map(|versus| marks(row(versus)));
        let _ = writeln!(out, "| {} | {} |", outcome.name, cells.join(" | "));
    }
    if results.ratchet.is_empty() {
        let _ = writeln!(out, "\nThe run agrees with KNOWN_BEHIND.");
    } else {
        let _ = writeln!(out, "\nWhere the run and KNOWN_BEHIND disagree:\n");
        for problem in &results.ratchet {
            let _ = writeln!(out, "- {problem}");
        }
    }
    if !results.grown.is_empty() {
        let _ = writeln!(out, "\nWhere KNOWN_BEHIND grew, which it may not:\n");
        for problem in &results.grown {
            let _ = writeln!(out, "- {problem}");
        }
    }
    let _ = writeln!(out, "\n## The native gaps\n");
    let _ = writeln!(
        out,
        "lengua's median over that of the best native build, C's, C++'s or Rust's, on each \
         measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.\n"
    );
    out.push_str("| workload | CPU time | wall time | peak memory | size |\n");
    out.push_str("|---|---|---|---|---|\n");
    let measures = Measure::ALL.len();
    for outcome in &results.workloads {
        let gaps: Vec<String> = (0..measures).map(|at| gap(outcome, at)).collect();
        let _ = writeln!(out, "| {} | {} |", outcome.name, gaps.join(" | "));
    }
    let _ = writeln!(out, "\n## Every row\n");
    let _ = writeln!(
        out,
        "Each measure's verdict (ahead, level, behind, or not measured), lengua's median over \
         the other version's, and the test's p.\n"
    );
    row_lines(&mut out, &results.workloads);
    for outcome in &results.workloads {
        workload_section(&mut out, outcome);
    }
    apart_section(&mut out, results);
    if let Some(against) = &results.against {
        let commit = &against.commit;
        let _ = writeln!(out, "\n## lengua now against lengua at {commit}\n");
        let _ = writeln!(
            out,
            "Each workload's lengua program as it is now and as commit {commit} had it, both \
             checked and run here, in rounds as above: on the row `then`, ahead is today's \
             lower.\n"
        );
        row_lines(&mut out, &against.workloads);
        for outcome in &against.workloads {
            for measured in &outcome.versions {
                if let Some(problem) = &measured.problem {
                    let (name, version) = (&outcome.name, &measured.name);
                    let _ = writeln!(out, "\n- {name}, {version}: {problem}");
                }
            }
        }
    }
    out
}

/// Writes `results`' report, BENCHMARKS-<target>.md, at the repository's root `root`.
fn write_report(root: &Path, results: &Results) -> Result<(), String> {
    let path = root.join(format!("BENCHMARKS-{}.md", results.target));
    std::fs::write(&path, report(results)).map_err(|error| failed(path.display(), error))?;
    eprintln!("wrote {}", path.display());
    Ok(())
}

/// Writes BENCHMARKS-<target>.md from the results file `file`, as a run's artifacts keep it.
fn report_from(root: &Path, file: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|error| failed(file.display(), error))?;
    let results: Results =
        serde_json::from_str(&text).map_err(|error| failed(file.display(), error))?;
    write_report(root, &results)
}

/// The gate on this runner: lengua's programs checked, every workload measured, the results,
/// the report and the list that would pass written, and the run held to KNOWN_BEHIND. Gives
/// whether it passed.
async fn gate_run(root: &Path) -> Result<bool, String> {
    let target = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let names = chosen()?;
    let seconds = match setting("LENGUA_BENCH_ROW_SECONDS") {
        Some(text) => text
            .parse::<u64>()
            .map_err(|_| format!("LENGUA_BENCH_ROW_SECONDS is {text:?}, not seconds"))?,
        None => gate::MAX_ROW.as_secs(),
    };
    let list = root.join("KNOWN_BEHIND");
    let list = std::fs::read_to_string(&list).map_err(|error| failed(list.display(), error))?;
    let known = KnownBehind::parse(&list).map_err(|problems| problems.join("\n"))?;
    // The list may only shrink: each change to it, held to the list it replaced, said at once.
    let grown = grown_in_history(root, &list);
    for problem in &grown {
        eprintln!("{problem}");
    }
    let out = root.join("target").join("bench");
    std::fs::create_dir_all(&out).map_err(|error| failed(out.display(), error))?;
    let wasm = setting("LENGUA_BENCH_WASM").map(PathBuf::from);
    let wasm = wasm.unwrap_or_else(|| root.join("target").join("bench-wasm"));
    let against = setting("LENGUA_BENCH_AGAINST");

    // lengua's programs, each from its idea to done in one session, as an agent takes them: the
    // session's data folder then holds the key their locks are signed with, and the engine's
    // cache of compiled modules, which every module's runs share.
    let session = Session::start(Protocol::Discover).await;
    let (mut now, mut then) = (Checked::new(), Checked::new());
    for &name in &names {
        let path = program_of(name);
        eprintln!("checking {path}");
        now.insert(name, checked_here(&session, root, path).await);
        if let Some(commit) = &against {
            then.insert(name, checked_at(&session, root, commit, path).await);
        }
    }
    let data_dir = session.data_dir.path().to_path_buf();
    let _ = session.service.cancel().await;
    let build = tempfile::tempdir_in(&out).map_err(|error| failed(out.display(), error))?;
    let server = PathBuf::from(env!("CARGO_BIN_EXE_lengua-mcp"));
    let runtime = std::fs::metadata(&server).map_err(|error| failed(server.display(), error))?;
    let context = Context {
        root: root.to_path_buf(),
        out: out.clone(),
        build: build.path().to_path_buf(),
        runner: Runner {
            lengua: server,
            data_dir,
        },
        wasm,
        seconds,
        runtime: runtime.len(),
    };
    let (commit, run) = provenance(root);
    let mut results = Results {
        target: target.clone(),
        commit,
        run,
        cpu: cpu_model(),
        cores: std::thread::available_parallelism().map_or(1, std::num::NonZero::get),
        engine: lengua::wasm::engine::ENGINE.to_owned(),
        toolchains: wasm_toolchains(&context.wasm),
        runtime: context.runtime,
        looks: gate::LOOKS.to_vec(),
        alpha: gate::ALPHA,
        look_alpha: gate::LOOK_ALPHA,
        max_runs: gate::MAX_RUNS,
        margin: gate::MARGIN,
        row_seconds: seconds,
        warm_up: WARM_UP,
        apart_runs: APART_RUNS,
        apart_fewest: APART_FEWEST,
        apart_seconds: APART_TIME.as_secs(),
        every_workload: names.len() == gate::WORKLOADS.len(),
        grown,
        ..Results::default()
    };
    for &name in &names {
        eprintln!("measuring {name}");
        let checked = now.get(name);
        let outcome = measure_workload(&context, name, checked, &mut results.toolchains);
        results.workloads.push(outcome);
    }
    if let Some(commit) = &against {
        eprintln!("measuring lengua's programs against {commit}");
        let workloads = (&now, &then);
        results.against = Some(measure_against(&context, commit, &names, workloads));
    }
    let found = found(&results);
    results.ratchet = if gate::TARGETS.contains(&target.as_str()) {
        gate::ratchet(&known, &target, &found)
    } else {
        let alone = format!("{target} is no runner of the gate's");
        vec![alone]
    };
    let json = serde_json::to_string(&results).map_err(|error| failed("the results", error))?;
    let file = out.join(format!("results-{target}.json"));
    std::fs::write(&file, json).map_err(|error| failed(file.display(), error))?;
    write_report(root, &results)?;
    let file = out.join(format!("KNOWN_BEHIND-{target}.txt"));
    let lines = proposal(&known, &results, &found);
    std::fs::write(&file, lines).map_err(|error| failed(file.display(), error))?;
    let mut failures = results.ratchet.clone();
    failures.extend(results.grown.iter().cloned());
    for outcome in &results.workloads {
        for measured in &outcome.versions {
            if let Some(problem) = &measured.problem {
                let (name, version) = (&outcome.name, &measured.name);
                failures.push(format!("{name}, {version}: {problem}"));
            }
        }
        // lengua-module tests itself as every version does: one that can't be measured, or
        // gives another output, fails the bench, and so does lengua's run in its rounds.
        let apart = outcome
            .apart
            .iter()
            .flat_map(|apart| [&apart.lengua, &apart.module]);
        for measured in apart {
            if let Some(problem) = &measured.problem {
                let (name, version) = (&outcome.name, &measured.name);
                failures.push(format!(
                    "{name}, {version}, in the rounds of lengua's run apart from its module: \
                     {problem}"
                ));
            }
        }
    }
    for failure in &failures {
        eprintln!("{failure}");
    }
    let verdict = if failures.is_empty() {
        "passed"
    } else {
        "failed"
    };
    let count = failures.len();
    eprintln!("the gate {verdict} on {target}, with {count} problems");
    Ok(failures.is_empty())
}

#[tokio::main]
async fn main() -> ExitCode {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ran = match setting("LENGUA_BENCH_REPORT") {
        Some(file) => report_from(&root, Path::new(&file)).map(|()| true),
        None => gate_run(&root).await,
    };
    match ran {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(problem) => {
            eprintln!("the bench couldn't run: {problem}");
            ExitCode::FAILURE
        }
    }
}
