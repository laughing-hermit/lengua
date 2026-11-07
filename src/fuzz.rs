//! Fuzzing every reader lengua has: what a person or an agent writes (idea.md and its tests, and
//! main.s for each CPU), what lengua reads back (main.s's header, its lock, and the executable
//! it built), the AArch64 decoder's machine words, and the test language's formulas. No reader may panic, or take long,
//! on any input: random bytes, and the repository's own ideas and programs mutated (bytes
//! changed, lines dropped, doubled and swapped, tricky pieces put in, cut short, and spliced with
//! each other). The runs are seeded, so a failure comes back the same; each one that fails is
//! kept under `target/fuzz` to replay.
//!
//! A short pass runs with every test; the long one, `long_fuzzing_finds_nothing`, is ignored and
//! runs by hand or on CI's dispatch.

use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

/// The longest any one input may take a reader.
const SLOW: Duration = Duration::from_secs(5);

/// Pieces that tend to break readers: the format's own markers, numbers past every width,
/// unclosed quotes and braces, and bytes text rarely holds.
const TRICKY: &[&str] = &[
    "`",
    "``",
    "{",
    "}",
    "{{",
    "}}",
    "|",
    "\\",
    "\n",
    "\r\n",
    "\t",
    "#",
    "##",
    "### For any `a`",
    "- Given ",
    "- When the program runs",
    "- Then it prints `",
    "- And it exits with code `",
    "## B1: ",
    "| a |\n|---|\n",
    "`int(",
    "`string(0, ",
    "`choice(",
    ")",
    "(",
    ",",
    "..",
    ".equ ",
    "0x",
    "-",
    "999999999999999999999999999999",
    "-9223372036854775808",
    "18446744073709551616",
    "\u{0}",
    "\u{feff}",
    "\u{1f600}",
    "é",
    ":lo12:",
    "[",
    "]",
    "#-1",
    "# bound: ",
    "// bound: ",
    "lsl #",
    "qword ptr [",
    "rip + ",
    ".section .data\n",
    ".type x, @object\n",
    "x:\n",
    "---\n",
    "type: lengua/program-idea\n",
    "- Uses: ",
    "- Network: connects to TCP port ",
    " over TLS",
    "- Threads: up to ",
    "=",
    "\"",
    "'",
    "%",
    "*",
    "/",
];

/// Every idea and program in the repository, with its path.
fn corpus() -> Vec<(PathBuf, Vec<u8>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    let mut pending = vec![
        root.join("examples"),
        root.join("tests/fixtures"),
        root.join("plugin/skills/compile/assets"),
    ];
    while let Some(folder) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "md" || extension == "s")
                && let Ok(bytes) = std::fs::read(&path)
            {
                found.push((path, bytes));
            }
        }
    }
    found.sort();
    found
}

/// An idea's text with every number past 4,096 brought down to it, so its generated tests' dry
/// runs stay quick: the fuzzer tries the readers, not the formulas' speed on megabytes. A huge
/// number a mutation brings in stays as it is.
fn shrunk(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at].is_ascii_digit() {
            let start = at;
            while at < bytes.len() && bytes[at].is_ascii_digit() {
                at += 1;
            }
            let digits = &bytes[start..at];
            let big = digits.len() > 4
                || std::str::from_utf8(digits)
                    .ok()
                    .and_then(|text| text.parse::<u32>().ok())
                    .is_some_and(|number| number > 4096);
            out.extend_from_slice(if big { b"4096" } else { digits });
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    out
}

/// `bytes` changed one way, chosen by `rng`, `other` the bytes of another file to splice in.
fn mutate(rng: &mut ChaCha8Rng, bytes: &[u8], other: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    let lines = |bytes: &[u8]| -> Vec<Vec<u8>> {
        bytes
            .split_inclusive(|byte| *byte == b'\n')
            .map(<[u8]>::to_vec)
            .collect()
    };
    let rounds = rng.random_range(1..=4);
    for _ in 0..rounds {
        match rng.random_range(0..7) {
            0 if !out.is_empty() => {
                let at = rng.random_range(0..out.len());
                out[at] = rng.random();
            }
            1 => {
                let mut all = lines(&out);
                if !all.is_empty() {
                    all.remove(rng.random_range(0..all.len()));
                }
                out = all.concat();
            }
            2 => {
                let mut all = lines(&out);
                if !all.is_empty() {
                    let at = rng.random_range(0..all.len());
                    let line = all[at].clone();
                    all.insert(at, line);
                }
                out = all.concat();
            }
            3 => {
                let mut all = lines(&out);
                if all.len() > 1 {
                    let (a, b) = (
                        rng.random_range(0..all.len()),
                        rng.random_range(0..all.len()),
                    );
                    all.swap(a, b);
                }
                out = all.concat();
            }
            4 => {
                let piece = TRICKY[rng.random_range(0..TRICKY.len())].as_bytes();
                let at = rng.random_range(0..=out.len());
                out.splice(at..at, piece.iter().copied());
            }
            5 if !out.is_empty() => out.truncate(rng.random_range(0..out.len())),
            _ if !other.is_empty() => {
                let start = rng.random_range(0..other.len());
                let end = rng.random_range(start..=other.len().min(start + 400));
                let at = rng.random_range(0..=out.len());
                let cut = rng.random_range(at..=out.len().min(at + 400));
                out.splice(at..cut, other[start..end].iter().copied());
            }
            _ => {}
        }
    }
    out
}

/// Runs `read` on `input`, and fails, keeping the input under `target/fuzz`, when it panics or
/// takes past [`SLOW`].
fn survives(reader: &str, index: usize, input: &[u8], read: &dyn Fn(&[u8])) {
    let started = Instant::now();
    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| read(input)));
    let took = started.elapsed();
    if outcome.is_ok() && took < SLOW {
        return;
    }
    let kept = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/fuzz")
        .join(format!("{reader}-{index}.txt"));
    let _ = std::fs::create_dir_all(kept.parent().unwrap_or(Path::new(".")));
    let _ = std::fs::write(&kept, input);
    if outcome.is_err() {
        panic!(
            "{reader} panicked on input {index}, kept in {}",
            kept.display()
        );
    }
    panic!(
        "{reader} took {took:?} on input {index}, kept in {}",
        kept.display()
    );
}

/// A reader of some text lengua takes in, named.
type Reader = (&'static str, Box<dyn Fn(&[u8])>);

/// Each reader, over its own text.
fn readers() -> Vec<Reader> {
    vec![
        (
            "idea",
            Box::new(|input: &[u8]| {
                let text = String::from_utf8_lossy(input);
                if let Ok(idea) = crate::idea::parse(&text) {
                    let _ = crate::testfile::check(&idea);
                }
                // lengua's entry in `verified`, written and taken away, leaves the idea's hash.
                let hash = crate::trust::idea_hash(text.as_bytes());
                let at = "2026-11-06T23:00:00Z";
                let code = crate::hash::Sha256Hex::of(b"code");
                let entry = crate::trust::Verified::new(at, hash.clone(), code);
                let written = crate::trust::with_lengua_verified(&text, &entry);
                assert_eq!(crate::trust::idea_hash(written.as_bytes()), hash);
                if let Some(removed) = crate::trust::without_lengua_verified(&written) {
                    assert_eq!(crate::trust::idea_hash(removed.as_bytes()), hash);
                }
            }),
        ),
        (
            "main.s",
            Box::new(|input: &[u8]| {
                let text = String::from_utf8_lossy(input);
                let _ = crate::checker::source::parse(&text);
                let _ = crate::checker::check_layout(&text);
                let _ = crate::header::Header::of(&text);
            }),
        ),
        (
            "formula",
            Box::new(|input: &[u8]| {
                let text = String::from_utf8_lossy(input);
                // Each braced piece, as a test's formulas are written.
                for piece in text.split('{').skip(1).take(8) {
                    let formula = piece.split('}').next().unwrap_or(piece);
                    if let Ok(parsed) = crate::expr::parse(formula) {
                        let _ = crate::expr::eval(&parsed, &crate::expr::Env::new());
                    }
                }
            }),
        ),
        (
            "okf",
            Box::new(|input: &[u8]| {
                // The graph reads every page of a bundle, and lengua's index.md and log.md.
                let text = String::from_utf8_lossy(input);
                let _ = crate::graph::okf::frontmatter(&text);
                let _ = crate::graph::okf::idea_links(&text);
                for line in text.lines().take(64) {
                    let _ = crate::graph::okf::entry_target(line);
                }
                let day = crate::graph::Day::from_unix_days(20_763);
                let entry = "* **Update**: [add](add/idea.md) recorded.";
                let _ = crate::graph::okf::logged(Some(&*text), day, entry);
            }),
        ),
        (
            "lock",
            Box::new(|input: &[u8]| {
                let _ = crate::lock::parse_body(&String::from_utf8_lossy(input));
            }),
        ),
        (
            "executable",
            Box::new(|input: &[u8]| {
                let _ = crate::checker::decode::read(input);
            }),
        ),
        (
            "broker request",
            Box::new(|input: &[u8]| {
                // A request whole, and its fixed part and first message on their own.
                if let Ok(request) = crate::broker::parse_request(input) {
                    // What the parser accepts is exactly what a client sends for it.
                    assert_eq!(request.encode(), input, "an accepted request re-encodes");
                }
                let _ = crate::broker::parse_header(input);
                let _ = crate::broker::Hello::decode(input);
                let _ = crate::broker::parse_reply(input);
            }),
        ),
    ]
}

/// Executables lengua built, for the ELF reader to start from: each CPU's template, where the
/// toolchain is installed.
fn executables() -> Vec<Vec<u8>> {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return Vec::new();
    };
    runtime.block_on(async {
        let Ok(toolchain) = crate::toolchain::Toolchain::discover().await else {
            return Vec::new();
        };
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("plugin/skills/compile/assets");
        let mut found = Vec::new();
        for (path, profile) in [
            ("main-template.s", crate::platform::Profile::LinuxX86_64),
            (
                "linux-aarch64/main-template.s",
                crate::platform::Profile::LinuxAarch64,
            ),
        ] {
            let Ok(folder) = tempfile::tempdir() else {
                continue;
            };
            let (object, executable) = (folder.path().join("main.o"), folder.path().join("main"));
            if toolchain
                .assemble(profile, &assets.join(path), &object)
                .await
                .is_ok()
                && toolchain.link(profile, &object, &executable).await.is_ok()
                && let Ok(bytes) = std::fs::read(&executable)
            {
                found.push(bytes);
            }
        }
        found
    })
}

/// `count` inputs for each reader: mutations of the corpus, and a few of random bytes.
fn fuzz(count: usize, seed: u64) {
    let corpus = corpus();
    let executables = executables();
    assert!(corpus.len() > 20, "the repository's own ideas and programs");
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    for (reader, read) in readers() {
        // Readers start from their own kind of text; the lock from a lock lengua writes.
        let starts: Vec<Vec<u8>> = match reader {
            "idea" | "formula" | "okf" => corpus
                .iter()
                .filter(|(path, _)| path.extension().is_some_and(|extension| extension == "md"))
                .map(|(_, bytes)| shrunk(bytes))
                .collect(),
            "lock" => vec![LOCK.as_bytes().to_vec()],
            "broker request" => broker_messages(),
            "executable" if executables.is_empty() => {
                eprintln!("llvm-mc or ld.lld isn't installed: the executable reader isn't fuzzed");
                continue;
            }
            "executable" => executables.clone(),
            _ => corpus
                .iter()
                .filter(|(path, _)| path.extension().is_some_and(|extension| extension == "s"))
                .map(|(_, bytes)| bytes.clone())
                .collect(),
        };
        for index in 0..count {
            let input = if index % 16 == 15 {
                let length = rng.random_range(0..512);
                (0..length).map(|_| rng.random::<u8>()).collect()
            } else {
                let start = &starts[rng.random_range(0..starts.len())];
                let other = corpus[rng.random_range(0..corpus.len())].1.as_slice();
                mutate(&mut rng, start, other)
            };
            survives(reader, index, &input, read.as_ref());
        }
    }
    // The AArch64 decoder on any word at all.
    for index in 0..count * 64 {
        let word: u32 = rng.random();
        survives(
            "aarch64 decoder",
            index,
            &word.to_le_bytes(),
            &|bytes: &[u8]| {
                let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                let _ = crate::isa::aarch64::decode::decode(word, 0x40_0000);
            },
        );
    }
}

/// The messages of the broker's protocol, each whole, for the reader of them to start from: a text
/// file never gets past a message's first byte.
fn broker_messages() -> Vec<Vec<u8>> {
    use crate::broker::{Hello, Kind, Mode, Request, encode_reply};
    let request = |kind, argument, value, first: &str, second: &str| {
        Request {
            kind,
            argument,
            value,
            first: first.as_bytes().to_vec(),
            second: second.as_bytes().to_vec(),
        }
        .encode()
    };
    vec![
        request(Kind::Open, 1, 0, "out.txt", ""),
        request(Kind::Open, 4, 0, "a/b/c", ""),
        request(Kind::Mkdir, 0, 0, "folder/sub", ""),
        request(Kind::Remove, 1, 0, "folder", ""),
        request(Kind::Rename, 0, 0, "old.txt", "new.txt"),
        request(Kind::Closed, 0, 7, "", ""),
        request(Kind::Waiting, 0, 0, "", ""),
        request(Kind::Listed, 0, 0, "", ""),
        request(Kind::Resumed, 0, 0, "", ""),
        request(Kind::Clock, 1, 0, "", ""),
        request(Kind::Random, 0, 16, "", ""),
        encode_reply(-2, b"payload"),
        Hello {
            mode: Mode::Test,
            flags: 7,
            file_limit: 1 << 20,
        }
        .encode()
        .to_vec(),
    ]
}

/// A lock as lengua writes one, to start mutating from.
const LOCK: &str = "version = 1\n\
    [idea]\n\
    sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n\
    request = \"add two numbers\"\n\
    [code]\n\
    target = \"linux-x86_64\"\n\
    sha256 = \"1111111111111111111111111111111111111111111111111111111111111111\"\n\
    executable_sha256 = \"2222222222222222222222222222222222222222222222222222222222222222\"\n\
    toolchain = \"llvm-mc 22\"\n";

#[test]
fn fuzzing_finds_nothing() {
    fuzz(400, 0x6c65_6e67_7561);
}

#[test]
#[ignore = "the long pass: a few minutes, by hand or on CI's dispatch"]
fn long_fuzzing_finds_nothing() {
    let seed = std::env::var("LENGUA_FUZZ_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or(0x6c65_6e67_7561_0002);
    fuzz(40_000, seed);
}

#[test]
#[ignore = "a measurement"]
fn how_long_the_corpus_ideas_take() {
    for (path, bytes) in corpus() {
        if path.extension().is_some_and(|extension| extension == "md") {
            let started = Instant::now();
            if let Ok(idea) = crate::idea::parse(&String::from_utf8_lossy(&shrunk(&bytes))) {
                let _ = crate::testfile::check(&idea);
            }
            let took = started.elapsed();
            if took > Duration::from_millis(500) {
                eprintln!("{took:?} {}", path.display());
            }
        }
    }
}
