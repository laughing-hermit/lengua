//! Fuzzing every reader lengua has: what a person or an agent writes (idea.md and its tests, and
//! main.wat, built and held to the WebAssembly dialect's rules), what lengua reads back (its
//! lock, and the graph's pages), and the test language's formulas. No reader may panic, or take
//! long, on any input: random bytes, and the repository's own ideas and programs mutated (bytes
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

/// Pieces that tend to break readers: the format's own markers, an idea's trust record, numbers
/// past every width, unclosed quotes and braces, and bytes text rarely holds.
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
    "0x",
    "-",
    "999999999999999999999999999999",
    "-9223372036854775808",
    "18446744073709551616",
    "\u{0}",
    "\u{feff}",
    "\u{1f600}",
    "é",
    "[",
    "]",
    "(module",
    "(func $f",
    "(param $a i32)",
    "(result i32)",
    "(local $i i32)",
    "(memory 1)",
    "(data (i32.const 1024) \"",
    "(loop $l",
    "br_if $l",
    "i32.const -1",
    "i64.const 9223372036854775807",
    "local.get $i",
    "i32.load offset=4",
    "call $fill",
    ";; bound: ",
    ";; unsafe: ",
    ";; end unsafe",
    ";; from: ",
    "---\n",
    "type: lengua/program-idea\n",
    "verified:\n",
    "  - { by: human:a, at: 2026-01-01T00:00:00Z }\n",
    "  - { by: lengua/0.0.1 }\n",
    "verified: [",
    "generated: {",
    "- Uses: ",
    "- Network: connects to TCP port ",
    " over TLS",
    "- Threads: up to ",
    "- Secret: a file `",
    " and argument `",
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
                .is_some_and(|extension| extension == "md" || extension == "wat")
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
            "main.wat",
            Box::new(|input: &[u8]| {
                // lengua's text with its library merged in, and a module that builds held to
                // every rule, for an idea that uses nothing.
                let text = String::from_utf8_lossy(input);
                if let Ok(built) = crate::wasm::text::build(&text) {
                    let network = crate::idea::Network::default();
                    let context = crate::checker::wasm::Context {
                        uses: &[],
                        network: &network,
                        secrets: &[],
                    };
                    let _ = crate::checker::wasm::check(&text, &built, &context);
                }
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
                let text = String::from_utf8_lossy(input);
                // What is read with no TOML is the very lock TOML reads from the same text.
                if let Some(lock) = crate::lock::as_written(&text) {
                    assert_eq!(crate::lock::parse_toml(&text), Ok(lock));
                }
                let _ = crate::lock::parse_body(&text);
            }),
        ),
    ]
}

/// `count` inputs for each reader: mutations of the corpus, and a few of random bytes.
fn fuzz(count: usize, seed: u64) {
    let corpus = corpus();
    assert!(corpus.len() > 20, "the repository's own ideas and programs");
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    for (reader, read) in readers() {
        // Readers start from their own kind of text; the lock from a lock exactly as lengua
        // writes it, and from one that isn't quite.
        let starts: Vec<Vec<u8>> = match reader {
            "idea" | "formula" | "okf" => corpus
                .iter()
                .filter(|(path, _)| path.extension().is_some_and(|extension| extension == "md"))
                .map(|(_, bytes)| shrunk(bytes))
                .collect(),
            "lock" => vec![LOCK.as_bytes().to_vec(), written_lock()],
            _ => corpus
                .iter()
                .filter(|(path, _)| path.extension().is_some_and(|extension| extension == "wat"))
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
}

/// A lock as lengua writes one but for its fingerprint, which it leaves out, its signature aside,
/// to start mutating from: TOML's to read.
const LOCK: &str = "# lengua.lock: written only by the lengua MCP server. Do not edit.\n\
    version = 7\n\
    dialect = \"webassembly 1\"\n\
    \n\
    [request]\n\
    words = [\"add two numbers\"]\n\
    idea = \"0000000000000000000000000000000000000000000000000000000000000000\"\n\
    \n\
    [idea]\n\
    sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n\
    \n\
    [code]\n\
    sha256 = \"1111111111111111111111111111111111111111111111111111111111111111\"\n\
    module_sha256 = \"2222222222222222222222222222222222222222222222222222222222222222\"\n\
    engine = \"wasmtime 50.0.1\"\n\
    max_stack_bytes = 64\n\
    unsafe_blocks = 0\n\
    target = \"webassembly\"\n\
    features = [\"bulk-memory\"]\n\
    \n\
    [proofs]\n\
    module_sha256 = \"2222222222222222222222222222222222222222222222222222222222222222\"\n\
    proven = [\"B1\"]\n\
    solvers = \"z3 and cvc5\"\n";

/// A lock exactly as lengua writes one, its signature aside, with proofs of every kind: what the
/// lock's reader with no TOML takes, to start mutating from too.
fn written_lock() -> Vec<u8> {
    use crate::hash::Sha256Hex;
    use crate::lock::{Bounded, CodeStamp, IdeaStamp, Key, Lock, ProofStamp, RequestStamp};
    let lock = Lock {
        request: RequestStamp {
            words: vec!["add two numbers".into(), "and \"negative\" ones".into()],
            idea: Sha256Hex::of(b"first idea"),
        },
        idea: IdeaStamp {
            sha256: Sha256Hex::of(b"idea"),
        },
        code: Some(CodeStamp {
            sha256: Sha256Hex::of(b"main.wat"),
            module_sha256: Sha256Hex::of(b"module"),
            engine: "wasmtime 50.0.1".into(),
            max_stack_bytes: 64,
            unsafe_blocks: 0,
            features: vec!["bulk-memory".into(), "simd".into()],
            parts: Vec::new(),
        }),
        proofs: Some(Box::new(ProofStamp {
            module_sha256: Sha256Hex::of(b"module"),
            proven: vec!["B1".into()],
            bounded: vec![Bounded {
                id: "B2".into(),
                bytes: 64,
                of: 4096,
                more: true,
            }],
            solvers: "z3 and cvc5".into(),
            wrong: vec![("B3".into(), "it printed \"5\"".into())],
            unproven: vec![("B4".into(), "the time ran out".into())],
        })),
    };
    let signed = lock.to_signed_text(&Key::from_bytes([0; 32]));
    let body = signed.split("\n[signature]").next().unwrap_or_default();
    body.as_bytes().to_vec()
}

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
