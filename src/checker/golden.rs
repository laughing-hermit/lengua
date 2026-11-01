//! Round 9's guard G1: the checker's report on every x86-64 program in the repository and on
//! thousands of small mutants of them, recorded before the checker moves onto
//! architecture-neutral instruction facts, so that each step of the move can be shown to change
//! no report at all.
//!
//! `LENGUA_GOLDEN=write cargo test --release --lib checker::golden -- --ignored` records the
//! reports under `target/golden/`; without the variable the test compares and, when anything
//! differs, writes what it found beside the recording and names the first differences. The
//! recording isn't committed: it's made again from the commit before the refactor if it's lost.
//! Builds are cached by their source's hash under `target/golden-cache/`, so a comparison takes
//! seconds.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use super::{Context, Report, check_layout, check_program};
use crate::hash::Sha256Hex;
use crate::platform::{CpuFeatures, Profile};
use crate::spec::Syscall;
use crate::toolchain::Toolchain;

/// The most mutants made of one program, taken evenly from all of its candidates.
const MAX_MUTANTS: usize = 300;

/// Builds and checks run this many at a time.
const AT_ONCE: usize = 16;

/// General-purpose registers by width; a mutant swaps one for the next in its row.
const REGISTERS: [&[&str]; 4] = [
    &[
        "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11", "r12",
        "r13", "r14", "r15",
    ],
    &[
        "eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp", "r8d", "r9d", "r10d", "r11d",
        "r12d", "r13d", "r14d", "r15d",
    ],
    &[
        "ax", "bx", "cx", "dx", "si", "di", "bp", "sp", "r8w", "r9w", "r10w", "r11w", "r12w",
        "r13w", "r14w", "r15w",
    ],
    &[
        "al", "bl", "cl", "dl", "sil", "dil", "bpl", "spl", "r8b", "r9b", "r10b", "r11b", "r12b",
        "r13b", "r14b", "r15b",
    ],
];

/// Condition codes and their opposites, for `jcc`, `setcc` and `cmovcc`.
const CONDITIONS: [(&str, &str); 20] = [
    ("e", "ne"),
    ("ne", "e"),
    ("z", "nz"),
    ("nz", "z"),
    ("b", "ae"),
    ("ae", "b"),
    ("a", "be"),
    ("be", "a"),
    ("c", "nc"),
    ("nc", "c"),
    ("l", "ge"),
    ("ge", "l"),
    ("g", "le"),
    ("le", "g"),
    ("s", "ns"),
    ("ns", "s"),
    ("o", "no"),
    ("no", "o"),
    ("p", "np"),
    ("np", "p"),
];

/// Instructions a mutant swaps for a close relative.
const FAMILIES: [(&str, &str); 18] = [
    ("add", "sub"),
    ("sub", "add"),
    ("adc", "sbb"),
    ("sbb", "adc"),
    ("inc", "dec"),
    ("dec", "inc"),
    ("and", "or"),
    ("or", "and"),
    ("test", "cmp"),
    ("cmp", "test"),
    ("push", "pop"),
    ("pop", "push"),
    ("shl", "shr"),
    ("shr", "shl"),
    ("movzx", "movsx"),
    ("movsx", "movzx"),
    ("rol", "ror"),
    ("ror", "rol"),
];

/// The annotations a mutant drops.
const ANNOTATIONS: [&str; 6] = [
    "# bound:",
    "# returns:",
    "# unsafe:",
    "# end unsafe",
    "# from:",
    "# shared:",
];

/// A program and the spec that says which system calls and threads it may use.
pub(crate) struct Program {
    /// Its path from the repository's root.
    pub(crate) name: String,
    pub(crate) text: String,
    pub(crate) spec: Arc<crate::spec::Spec>,
    syscalls: Arc<Vec<Syscall>>,
    threads: Option<u32>,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every x86-64 program in the repository, with its spec.
pub(crate) fn programs() -> Vec<Program> {
    let root = root();
    let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
    for folder in [
        "examples",
        "tests/fixtures",
        "plugin/skills/compile/assets/fast",
    ] {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(root.join(folder))
            .expect("a folder of programs")
            .map(|entry| entry.expect("an entry").path())
            .filter(|path| path.join("main.s").is_file())
            .collect();
        entries.sort();
        pairs.extend(
            entries
                .into_iter()
                .map(|path| (path.join("spec.md"), path.join("main.s"))),
        );
    }
    let assets = root.join("plugin/skills/compile/assets");
    for (spec, code) in [
        ("template-spec.md", "main-template.s"),
        ("vector-template-spec.md", "vector-template.s"),
        ("parallel-template-spec.md", "parallel-template.s"),
        ("server-template-spec.md", "server-template.s"),
        ("terminal-template-spec.md", "terminal-template.s"),
        ("stream-template-spec.md", "stream-template.s"),
        ("tally-template-spec.md", "tally-template.s"),
    ] {
        pairs.push((assets.join(spec), assets.join(code)));
    }
    let benches = root.join("benches/programs");
    for (spec, code) in [
        ("examples/upcase/spec.md", "upcase/avx512.s"),
        (
            "plugin/skills/compile/assets/parallel-template-spec.md",
            "shout/avx512.s",
        ),
        (
            "plugin/skills/compile/assets/parallel-template-spec.md",
            "shout/avx512-one.s",
        ),
    ] {
        pairs.push((root.join(spec), benches.join(code)));
    }
    pairs
        .into_iter()
        .map(|(spec, code)| {
            let spec_text = std::fs::read_to_string(&spec).expect("a spec");
            let spec = crate::spec::parse(&spec_text)
                .unwrap_or_else(|problems| panic!("{}: {problems:?}", spec.display()));
            Program {
                name: code
                    .strip_prefix(&root)
                    .expect("inside the repository")
                    .display()
                    .to_string(),
                text: std::fs::read_to_string(&code).expect("a program"),
                syscalls: Arc::new(spec.syscalls.clone()),
                threads: spec.threads,
                spec: Arc::new(spec),
            }
        })
        .collect()
}

/// The mutants of `text`, each with its name: for every instruction, the line deleted,
/// doubled, swapped with the next, a register, a number, a condition or the instruction
/// changed for a neighbour, or its memory operand moved; for every annotation, the annotation
/// dropped. At most [`MAX_MUTANTS`], taken evenly.
fn mutants(text: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let (source, _) = check_layout(text);
    let instructions: BTreeSet<usize> = source
        .functions
        .iter()
        .flat_map(|function| &function.items)
        .filter_map(|item| match item {
            super::source::Item::Instruction(instruction) => Some(instruction.line - 1),
            super::source::Item::Label(_) => None,
        })
        .collect();
    let with = |index: usize, new: Option<String>| -> String {
        let mut out = String::with_capacity(text.len() + 32);
        for (at, line) in lines.iter().enumerate() {
            if at == index {
                if let Some(new) = &new {
                    out.push_str(new);
                    out.push('\n');
                }
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }
        out
    };
    let mut found = Vec::new();
    for &index in &instructions {
        let line = lines[index];
        let id = |kind: &str| format!("L{}:{kind}", index + 1);
        found.push((id("del"), with(index, None)));
        found.push((id("dup"), with(index, Some(format!("{line}\n{line}")))));
        if instructions.contains(&(index + 1)) {
            let mut swapped: Vec<&str> = lines.clone();
            swapped.swap(index, index + 1);
            found.push((id("swap"), swapped.join("\n") + "\n"));
        }
        let (code, comment) = match line.find('#') {
            Some(at) => line.split_at(at),
            None => (line, ""),
        };
        let indent = &code[..code.len() - code.trim_start().len()];
        let code = code.trim_start();
        let (mnemonic, operands) = code.split_once(' ').unwrap_or((code, ""));
        let rebuild = |mnemonic: &str, operands: &str| {
            let spaced = if operands.is_empty() { "" } else { " " };
            format!("{indent}{mnemonic}{spaced}{operands}{comment}")
                .trim_end()
                .to_owned()
        };
        if let Some(swapped) = swap_register(operands) {
            found.push((id("reg"), with(index, Some(rebuild(mnemonic, &swapped)))));
        }
        if let Some((start, end, value)) = last_number(operands) {
            for (kind, new) in [
                ("imm+1", value.wrapping_add(1)),
                ("imm0", 0),
                ("neg", value.wrapping_neg()),
            ] {
                if new != value {
                    let changed = format!("{}{new}{}", &operands[..start], &operands[end..]);
                    found.push((id(kind), with(index, Some(rebuild(mnemonic, &changed)))));
                }
            }
        }
        if let Some(flipped) = flip_condition(mnemonic) {
            found.push((id("cond"), with(index, Some(rebuild(&flipped, operands)))));
        }
        if let Some((_, other)) = FAMILIES.iter().find(|(name, _)| *name == mnemonic) {
            found.push((id("family"), with(index, Some(rebuild(other, operands)))));
        }
        if let Some(moved) = move_memory(operands) {
            found.push((id("mem"), with(index, Some(rebuild(mnemonic, &moved)))));
        }
    }
    for (index, line) in lines.iter().enumerate() {
        if let Some(at) = ANNOTATIONS
            .iter()
            .find_map(|annotation| line.find(annotation))
        {
            let dropped = line[..at].trim_end().to_owned();
            found.push((format!("L{}:annot", index + 1), with(index, Some(dropped))));
        }
    }
    if found.len() > MAX_MUTANTS {
        let step = found.len().div_ceil(MAX_MUTANTS);
        found = found.into_iter().step_by(step).collect();
    }
    found
}

/// The identifier-like words of `text`, with where each starts and ends.
fn words(text: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut start = None;
    for (at, character) in text.char_indices() {
        let inside = character.is_ascii_alphanumeric() || character == '_' || character == '.';
        match (inside, start) {
            (true, None) => start = Some(at),
            (false, Some(from)) => {
                found.push((from, at));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        found.push((from, text.len()));
    }
    found
}

/// `operands` with its first register swapped for the next of its width.
fn swap_register(operands: &str) -> Option<String> {
    for (start, end) in words(operands) {
        let word = &operands[start..end];
        let next = REGISTERS.iter().find_map(|row| {
            let at = row.iter().position(|register| *register == word)?;
            Some(row[(at + 1) % row.len()].to_owned())
        });
        let vector = ["xmm", "ymm", "zmm"].iter().find_map(|prefix| {
            let number: u32 = word.strip_prefix(prefix)?.parse().ok()?;
            Some(format!("{prefix}{}", (number + 1) % 16))
        });
        if let Some(new) = next.or(vector) {
            return Some(format!("{}{new}{}", &operands[..start], &operands[end..]));
        }
    }
    None
}

/// The last number written in `operands`, with where it starts and ends.
fn last_number(operands: &str) -> Option<(usize, usize, i64)> {
    words(operands).into_iter().rev().find_map(|(start, end)| {
        let word = &operands[start..end];
        let value = match word.strip_prefix("0x") {
            Some(hex) => i64::from_str_radix(hex, 16).ok()?,
            None if word.starts_with(|c: char| c.is_ascii_digit()) => word.parse().ok()?,
            None => return None,
        };
        Some((start, end, value))
    })
}

/// `mnemonic` with its condition reversed, for a `jcc`, `setcc` or `cmovcc`.
fn flip_condition(mnemonic: &str) -> Option<String> {
    ["cmov", "set", "j"].iter().find_map(|prefix| {
        let condition = mnemonic.strip_prefix(prefix)?;
        let (_, opposite) = CONDITIONS.iter().find(|(code, _)| *code == condition)?;
        Some(format!("{prefix}{opposite}"))
    })
}

/// `operands` with its memory operand's scale changed, or 8 bytes further on.
fn move_memory(operands: &str) -> Option<String> {
    for (from, to) in [("*8", "*4"), ("*4", "*8"), ("*2", "*4"), ("*1", "*2")] {
        if operands.contains(from) {
            return Some(operands.replacen(from, to, 1));
        }
    }
    let end = operands.find(']')?;
    Some(format!("{} + 8{}", &operands[..end], &operands[end..]))
}

/// What the checker says, line by line.
fn describe(report: &Report) -> String {
    let mut text = String::new();
    for violation in &report.violations {
        let _ = writeln!(text, "{violation}");
    }
    let _ = writeln!(text, "hidden {}", report.hidden);
    let _ = writeln!(text, "facts {:?}", report.facts);
    let _ = writeln!(text, "unsafe {:?}", report.unsafe_blocks);
    text
}

/// `text` built, from the cache when it was built before: the executable, or why it didn't
/// build.
pub(crate) async fn built(toolchain: &Toolchain, text: &str) -> Result<Vec<u8>, String> {
    let key = Sha256Hex::of(text.as_bytes());
    let folder = root().join("target/golden-cache").join(key.as_str());
    let (assembly, object, executable, failure) = (
        folder.join("main.s"),
        folder.join("main.o"),
        folder.join("main"),
        folder.join("failed.txt"),
    );
    if let Ok(bytes) = tokio::fs::read(&executable).await {
        return Ok(bytes);
    }
    if let Ok(message) = tokio::fs::read_to_string(&failure).await {
        return Err(message);
    }
    tokio::fs::create_dir_all(&folder)
        .await
        .expect("make the cache's folder");
    tokio::fs::write(&assembly, text)
        .await
        .expect("write main.s");
    let made = match toolchain
        .assemble(Profile::LinuxX86_64, &assembly, &object)
        .await
    {
        Ok(()) => toolchain.link(&object, &executable).await,
        Err(error) => Err(error),
    };
    match made {
        Ok(()) => Ok(tokio::fs::read(&executable)
            .await
            .expect("read the executable")),
        Err(error) => {
            // The message names the cache's folder; what matters is what follows it.
            let message = error.message.replace(&format!("{}/", folder.display()), "");
            tokio::fs::write(&failure, &message)
                .await
                .expect("write why it failed");
            Err(message)
        }
    }
}

/// The checker's report on `text`, as [`describe`] writes it, or why it didn't build.
async fn report(
    toolchain: &Toolchain,
    text: &str,
    syscalls: &[Syscall],
    threads: Option<u32>,
) -> String {
    let (source, layout) = check_layout(text);
    if !layout.passed() {
        return format!("layout\n{}", describe(&layout));
    }
    match built(toolchain, text).await {
        Ok(bytes) => describe(&check_program(
            &source,
            &bytes,
            Context {
                syscalls,
                threads,
                features: CpuFeatures::all(),
                profile: Profile::LinuxX86_64,
            },
        )),
        Err(message) => format!("build failed: {message}\n"),
    }
}

/// The recording: each program's report in full, then a hash of each mutant's.
async fn record() -> String {
    let toolchain = Arc::new(
        Toolchain::discover()
            .await
            .expect("llvm-mc and ld.lld are installed"),
    );
    let permits = Arc::new(tokio::sync::Semaphore::new(AT_ONCE));
    let mut tasks = tokio::task::JoinSet::new();
    // Every text is checked once, by its hash, whichever programs and mutants share it.
    let mut started: BTreeSet<String> = BTreeSet::new();
    let mut listed: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for program in programs() {
        let mut ids = Vec::new();
        let all = std::iter::once(("base".to_owned(), program.text.clone()))
            .chain(mutants(&program.text));
        for (id, text) in all {
            let key = Sha256Hex::of(text.as_bytes()).as_str().to_owned();
            ids.push((id, key.clone()));
            if !started.insert(key.clone()) {
                continue;
            }
            let (toolchain, permits) = (Arc::clone(&toolchain), Arc::clone(&permits));
            let (syscalls, threads) = (Arc::clone(&program.syscalls), program.threads);
            tasks.spawn(async move {
                let _permit = permits.acquire_owned().await.expect("never closed");
                (key, report(&toolchain, &text, &syscalls, threads).await)
            });
        }
        listed.push((program.name, ids));
    }
    let mut reports: BTreeMap<String, String> = BTreeMap::new();
    while let Some(done) = tasks.join_next().await {
        let (key, found) = done.expect("a check ran");
        reports.insert(key, found);
    }
    let mut recording = String::new();
    for (name, all) in listed {
        let _ = writeln!(recording, "== {name} ({} mutants)", all.len() - 1);
        for (id, key) in all {
            let found = &reports[&key];
            if id == "base" {
                for line in found.lines() {
                    let _ = writeln!(recording, "  {line}");
                }
            } else {
                let hash = Sha256Hex::of(found.as_bytes());
                let _ = writeln!(recording, "{id} {}", &hash.as_str()[..16]);
            }
        }
    }
    recording
}

/// Where the recording lives.
fn recording_path(name: &str) -> PathBuf {
    root().join("target/golden").join(name)
}

/// Compares `found` with the recording called `name`, or records it under
/// `LENGUA_GOLDEN=write`.
pub(crate) fn compare_or_record(name: &str, found: &str) {
    let path = recording_path(name);
    if std::env::var("LENGUA_GOLDEN").as_deref() == Ok("write") {
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("make target/golden");
        std::fs::write(&path, found).expect("record");
        eprintln!(
            "recorded {} ({} lines)",
            path.display(),
            found.lines().count()
        );
        return;
    }
    let recorded = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "{} isn't recorded: record it with LENGUA_GOLDEN=write at the commit before the \
             refactor",
            path.display()
        )
    });
    if recorded == found {
        return;
    }
    let beside = path.with_extension("found.txt");
    std::fs::write(&beside, found).expect("write what was found");
    let differences: Vec<String> = recorded
        .lines()
        .zip(found.lines())
        .filter(|(old, new)| old != new)
        .take(30)
        .map(|(old, new)| format!("- {old}\n+ {new}"))
        .collect();
    panic!(
        "{name} differs from its recording ({} lines then, {} now); what was found is in {}:\n{}",
        recorded.lines().count(),
        found.lines().count(),
        beside.display(),
        differences.join("\n")
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Round 9's guard: run with --ignored around the checker's refactor"]
async fn every_report_is_as_recorded() {
    let found = record().await;
    compare_or_record("checker-x86.txt", &found);
}

#[test]
fn mutants_are_made_the_same_way_every_time() {
    let text = super::TEMPLATE;
    let (first, second) = (mutants(text), mutants(text));
    assert_eq!(first, second);
    assert!(
        first.len() > 100 && first.len() <= MAX_MUTANTS,
        "{}",
        first.len()
    );
    let kinds: BTreeSet<&str> = first
        .iter()
        .filter_map(|(id, _)| id.split_once(':').map(|(_, kind)| kind))
        .collect();
    for kind in [
        "del", "dup", "swap", "reg", "imm0", "cond", "family", "annot",
    ] {
        assert!(kinds.contains(kind), "{kind} in {kinds:?}");
    }
    // Each mutant changes the program.
    assert!(first.iter().all(|(_, mutant)| mutant != text));
}

/// Prints the report on one program or mutant, for a golden that differs:
/// `LENGUA_GOLDEN_MUTANT="examples/sort/main.s L432:family"` (or `base`).
#[tokio::test]
#[ignore = "explains a golden's difference"]
async fn explains_a_mutant() {
    let wanted = std::env::var("LENGUA_GOLDEN_MUTANT").expect("LENGUA_GOLDEN_MUTANT names one");
    let (name, id) = wanted.split_once(' ').expect("a program and a mutant");
    let program = programs()
        .into_iter()
        .find(|program| program.name == name)
        .expect("a program in the golden");
    let text = if id == "base" {
        program.text.clone()
    } else {
        mutants(&program.text)
            .into_iter()
            .find(|(found, _)| found == id)
            .expect("a mutant of it")
            .1
    };
    let toolchain = Toolchain::discover().await.expect("llvm-mc and ld.lld");
    eprintln!(
        "{}",
        report(&toolchain, &text, &program.syscalls, program.threads).await
    );
}
