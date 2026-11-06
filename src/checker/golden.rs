//! Round 9's guard G1: the checker's report on every program in the repository, for each of the
//! two CPUs, x86-64's and ARM64's, and on thousands of small mutants of them, recorded before the
//! checker moves onto architecture-neutral instruction facts, so that each step of the move can
//! be shown to change no report at all.
//!
//! `LENGUA_GOLDEN=write cargo test --release --lib checker::golden -- --ignored` records the
//! reports under `target/golden/`, a recording for each CPU; without the variable the test
//! compares and, when anything differs, writes what it found beside the recording and names the
//! first differences. The recording isn't committed: it's made again from the commit before the
//! refactor if it's lost. A run that finds none, such as a fresh checkout's on a runner, makes
//! it and says in its summary ([`crate::skiplog`]) that it compared nothing. Builds are cached by
//! their source's hash under `target/golden-cache/`, so a comparison takes seconds.
//!
//! What differs between the CPUs is one table, [`PLACES`]: where the programs are, what a comment
//! starts with, and the recording's name. The mutants of a CPU's program swap its own registers,
//! flip its own conditions and move its own addressing; the program lists are what
//! `tests/baseline.rs` holds to the baseline.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{Context, Report, check_layout, check_program, lengua_source};
use crate::hash::Sha256Hex;
use crate::platform::{CpuFeatures, Profile};
use crate::targets::PerCpu;
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

/// The annotations a mutant drops, each written after the dialect's comment marker.
const ANNOTATIONS: [&str; 6] = [
    "bound:",
    "returns:",
    "unsafe:",
    "end unsafe",
    "from:",
    "shared:",
];

/// Condition codes and their opposites in AArch64's `b.cond` and the instructions that select by
/// one.
const A64_CONDITIONS: [(&str, &str); 16] = [
    ("eq", "ne"),
    ("ne", "eq"),
    ("hs", "lo"),
    ("lo", "hs"),
    ("cs", "cc"),
    ("cc", "cs"),
    ("hi", "ls"),
    ("ls", "hi"),
    ("mi", "pl"),
    ("pl", "mi"),
    ("vs", "vc"),
    ("vc", "vs"),
    ("ge", "lt"),
    ("lt", "ge"),
    ("gt", "le"),
    ("le", "gt"),
];

/// AArch64's instructions that take a condition as their last operand.
const A64_SELECTS: [&str; 9] = [
    "csel", "csinc", "csinv", "csneg", "cset", "csetm", "cinc", "cinv", "cneg",
];

/// AArch64's instructions a mutant swaps for a close relative.
const A64_FAMILIES: [(&str, &str); 20] = [
    ("add", "sub"),
    ("sub", "add"),
    ("adds", "subs"),
    ("subs", "adds"),
    ("cmp", "cmn"),
    ("cmn", "cmp"),
    ("and", "orr"),
    ("orr", "and"),
    ("eor", "orr"),
    ("lsl", "lsr"),
    ("lsr", "lsl"),
    ("tbz", "tbnz"),
    ("tbnz", "tbz"),
    ("cbz", "cbnz"),
    ("cbnz", "cbz"),
    ("udiv", "sdiv"),
    ("sdiv", "udiv"),
    ("ldrb", "ldrsb"),
    ("ldrsb", "ldrb"),
    ("mul", "add"),
];

/// What differs between the targets: where the programs are, what a comment starts with, and the
/// recording's name.
pub(crate) struct Places {
    /// What its recordings are called: `checker-<label>.txt` and `prover-<label>.txt`.
    pub(crate) label: &'static str,
    /// Folders of programs, each holding a spec.md and a main.s, from the repository's root.
    folders: &'static [&'static str],
    /// The skill's assets for the target, where its templates and its fast programs are.
    assets: &'static str,
    /// The benchmark's variants for the target: the spec each meets, and its main.s.
    benches: &'static [(&'static str, &'static str)],
    /// What starts a comment in its dialect.
    comment: &'static str,
}

/// The skill's assets that the targets share: the specs of the templates and of the fast
/// programs that have none of their own.
const SHARED_ASSETS: &str = "plugin/skills/compile/assets";

/// Each target's places.
pub(crate) static PLACES: PerCpu<Places> = PerCpu {
    x86_64: Places {
        label: "x86",
        folders: &["examples", "tests/fixtures"],
        assets: SHARED_ASSETS,
        benches: &[
            (
                "examples/upcase/spec.md",
                "benches/programs/upcase/avx512.s",
            ),
            ("examples/count/spec.md", "benches/programs/count/avx512.s"),
            (
                "plugin/skills/compile/assets/parallel-template-spec.md",
                "benches/programs/shout/avx512.s",
            ),
            (
                "plugin/skills/compile/assets/parallel-template-spec.md",
                "benches/programs/shout/avx512-one.s",
            ),
        ],
        comment: "#",
    },
    aarch64: Places {
        label: "aarch64",
        folders: &["tests/fixtures/linux-aarch64"],
        assets: "plugin/skills/compile/assets/linux-aarch64",
        benches: &[
            ("examples/upcase/spec.md", "benches/programs/upcase/neon.s"),
            (
                "plugin/skills/compile/assets/parallel-template-spec.md",
                "benches/programs/shout/neon-one.s",
            ),
        ],
        comment: "//",
    },
};

/// A program and the spec that says which system calls and threads it may use.
pub(crate) struct Program {
    /// Its path from the repository's root.
    pub(crate) name: String,
    pub(crate) text: String,
    pub(crate) spec: Arc<crate::spec::Spec>,
    ops: Arc<Vec<crate::edge::Op>>,
    threads: Option<u32>,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The names of what `folder` holds that end with `suffix`, sorted.
fn names_ending(folder: &Path, suffix: &str) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(folder)
        .unwrap_or_else(|error| panic!("{}: {error}", folder.display()))
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(suffix))
        .collect();
    found.sort();
    found
}

/// The spec of the program in `folder`, a folder of a target's fast programs: its own spec.md or,
/// for a program named for the extension it uses after the program it speeds up (`wc-sve`), that
/// program's, looked for in the target's folder first and then in the shared one.
fn spec_of(folder: &Path) -> PathBuf {
    let own = folder.join("spec.md");
    if own.is_file() {
        return own;
    }
    let shared = root().join(SHARED_ASSETS).join("fast");
    let beside = folder.parent().expect("a folder of programs").to_owned();
    let mut name = folder
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    loop {
        for fast in [&beside, &shared] {
            let spec = fast.join(&name).join("spec.md");
            if spec.is_file() {
                return spec;
            }
        }
        match name.rsplit_once('-') {
            Some((speeds_up, _)) => name = speeds_up.to_owned(),
            None => panic!("{} has no spec", folder.display()),
        }
    }
}

/// Every program of `profile`'s in the repository, with its spec: the examples and fixtures, the
/// fast programs, the skill's templates, and the benchmark's variants.
pub(crate) fn programs(profile: Profile) -> Vec<Program> {
    let root = root();
    let places = PLACES.get(profile);
    let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut folders: Vec<PathBuf> = places
        .folders
        .iter()
        .map(|folder| root.join(folder))
        .collect();
    folders.push(root.join(places.assets).join("fast"));
    for folder in folders {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&folder)
            .unwrap_or_else(|error| panic!("{}: {error}", folder.display()))
            .map(|entry| entry.expect("an entry").path())
            .filter(|path| path.join("main.s").is_file())
            .collect();
        entries.sort();
        pairs.extend(
            entries
                .into_iter()
                .map(|path| (spec_of(&path), path.join("main.s"))),
        );
    }
    // A template is made from the spec of its name: main-template.s from template-spec.md.
    let assets = root.join(places.assets);
    for name in names_ending(&assets, "-template.s") {
        let stem = name.strip_suffix("-template.s").unwrap_or_default();
        let spec = if stem == "main" {
            "template-spec.md".to_owned()
        } else {
            format!("{stem}-template-spec.md")
        };
        pairs.push((root.join(SHARED_ASSETS).join(spec), assets.join(name)));
    }
    for (spec, code) in places.benches {
        pairs.push((root.join(spec), root.join(code)));
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
                ops: Arc::new(spec.ops.clone()),
                threads: spec.threads,
                spec: Arc::new(spec),
            }
        })
        .collect()
}

/// The mutants of `text`, each with its name: for every instruction, the line deleted,
/// doubled, swapped with the next, a register, a number, a condition or the instruction
/// changed for a neighbour, or its memory operand moved; for every annotation, the annotation
/// dropped. At most [`MAX_MUTANTS`], taken evenly. The registers, conditions, instructions and
/// addressing a mutant changes, and the comment it keeps, are `profile`'s.
fn mutants(profile: Profile, text: &str) -> Vec<(String, String)> {
    let marker = PLACES.get(profile).comment;
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
        let (code, comment) = match line.find(marker) {
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
        if let Some(swapped) = swap_register(profile, operands) {
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
        if let Some((flipped, operands)) = flip_condition(profile, mnemonic, operands) {
            found.push((id("cond"), with(index, Some(rebuild(&flipped, &operands)))));
        }
        let families: &[(&str, &str)] = match profile {
            Profile::LinuxX86_64 => &FAMILIES,
            Profile::LinuxAarch64 => &A64_FAMILIES,
        };
        if let Some((_, other)) = families.iter().find(|(name, _)| *name == mnemonic) {
            found.push((id("family"), with(index, Some(rebuild(other, operands)))));
        }
        if let Some(moved) = move_memory(profile, operands) {
            found.push((id("mem"), with(index, Some(rebuild(mnemonic, &moved)))));
        }
    }
    for (index, line) in lines.iter().enumerate() {
        if let Some(at) = ANNOTATIONS
            .iter()
            .find_map(|annotation| line.find(&format!("{marker} {annotation}")))
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

/// `register` as the next register of its width and kind, on `profile`'s CPU: x86-64's rax to
/// rbx and on through its row, xmm0 to xmm1; AArch64's x0 to x1 (x0 to x28), w0 to w1, v0.16b to
/// v1.16b.
fn next_register(profile: Profile, register: &str) -> Option<String> {
    match profile {
        Profile::LinuxX86_64 => {
            let next = REGISTERS.iter().find_map(|row| {
                let at = row.iter().position(|name| *name == register)?;
                Some(row[(at + 1) % row.len()].to_owned())
            });
            let vector = ["xmm", "ymm", "zmm"].iter().find_map(|prefix| {
                let number: u32 = register.strip_prefix(prefix)?.parse().ok()?;
                Some(format!("{prefix}{}", (number + 1) % 16))
            });
            next.or(vector)
        }
        Profile::LinuxAarch64 => {
            let general = ['x', 'w'].iter().find_map(|prefix| {
                let number: u32 = register.strip_prefix(*prefix)?.parse().ok()?;
                (number <= 28).then(|| format!("{prefix}{}", (number + 1) % 29))
            });
            // `v1.16b`: the register's number, then its arrangement.
            let vector = register.strip_prefix('v').and_then(|rest| {
                let digits = rest
                    .find(|character: char| !character.is_ascii_digit())
                    .unwrap_or(rest.len());
                let number: u32 = rest[..digits].parse().ok()?;
                Some(format!("v{}{}", (number + 1) % 32, &rest[digits..]))
            });
            general.or(vector)
        }
    }
}

/// `operands` with its first register swapped for the next of its width.
fn swap_register(profile: Profile, operands: &str) -> Option<String> {
    for (start, end) in words(operands) {
        if let Some(new) = next_register(profile, &operands[start..end]) {
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
fn flip_x86_condition(mnemonic: &str) -> Option<String> {
    ["cmov", "set", "j"].iter().find_map(|prefix| {
        let condition = mnemonic.strip_prefix(prefix)?;
        let (_, opposite) = CONDITIONS.iter().find(|(code, _)| *code == condition)?;
        Some(format!("{prefix}{opposite}"))
    })
}

/// The opposite of AArch64's condition `code`.
fn opposite_a64_condition(code: &str) -> Option<&'static str> {
    A64_CONDITIONS
        .iter()
        .find(|(name, _)| *name == code)
        .map(|(_, opposite)| *opposite)
}

/// The instruction `mnemonic` with `operands` as `profile`'s CPU has it with the condition it
/// tests reversed: x86-64's `jcc`, `setcc` and `cmovcc` by their names; AArch64's `b.cond` by its
/// name, and `csel`, `cset` and their kin by their last operand.
fn flip_condition(profile: Profile, mnemonic: &str, operands: &str) -> Option<(String, String)> {
    match profile {
        Profile::LinuxX86_64 => {
            flip_x86_condition(mnemonic).map(|flipped| (flipped, operands.to_owned()))
        }
        Profile::LinuxAarch64 => {
            if let Some(code) = mnemonic.strip_prefix("b.") {
                let opposite = opposite_a64_condition(code)?;
                return Some((format!("b.{opposite}"), operands.to_owned()));
            }
            if !A64_SELECTS.contains(&mnemonic) {
                return None;
            }
            let (before, code) = operands.rsplit_once(", ")?;
            let opposite = opposite_a64_condition(code)?;
            Some((mnemonic.to_owned(), format!("{before}, {opposite}")))
        }
    }
}

/// `operands` with its memory operand's scale changed, or 8 bytes further on: x86-64's `*8` to
/// `*4`; AArch64's `lsl #3` to `lsl #2`, or its immediate offset 8 further on.
fn move_memory(profile: Profile, operands: &str) -> Option<String> {
    match profile {
        Profile::LinuxX86_64 => {
            for (from, to) in [("*8", "*4"), ("*4", "*8"), ("*2", "*4"), ("*1", "*2")] {
                if operands.contains(from) {
                    return Some(operands.replacen(from, to, 1));
                }
            }
            let end = operands.find(']')?;
            Some(format!("{} + 8{}", &operands[..end], &operands[end..]))
        }
        Profile::LinuxAarch64 => {
            let end = operands.find(']')?;
            let open = operands[..end].rfind('[')?;
            // A post-indexed access, `[sp], #16`, has its offset after the bracket: left alone.
            if operands[end + 1..].trim_start().starts_with(',') {
                return None;
            }
            let bracket = &operands[open + 1..end];
            for (from, to) in [
                ("lsl #3", "lsl #2"),
                ("lsl #2", "lsl #3"),
                ("lsl #1", "lsl #2"),
            ] {
                if bracket.contains(from) {
                    return Some(operands.replacen(from, to, 1));
                }
            }
            // An offset `[x1, #4]` becomes `[x1, #12]`; a base alone `[x1]` gets `#8`.
            let (head, tail) = (&operands[..=open], &operands[end..]);
            match bracket.rsplit_once(", #") {
                Some((base, offset)) => {
                    let number: i64 = offset.parse().ok()?;
                    Some(format!("{head}{base}, #{}{tail}", number + 8))
                }
                None if !bracket.contains(',') => Some(format!("{head}{bracket}, #8{tail}")),
                None => None,
            }
        }
    }
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

/// `text` built for `profile`'s CPU, from the cache when it was built before: the executable, or
/// why it didn't build.
pub(crate) async fn built(
    profile: Profile,
    toolchain: &Toolchain,
    text: &str,
) -> Result<Vec<u8>, String> {
    // A program in the edge form is built with lengua.s after it, which changes when lengua does:
    // the cache keys on both, since target/ is kept between runs.
    let lengua = crate::edge::lengua_text(profile, text).unwrap_or_default();
    let key = Sha256Hex::of(format!("{text}\0{lengua}").as_bytes());
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
    let made = match toolchain.assemble(profile, &assembly, &object).await {
        Ok(()) => toolchain.link(profile, &object, &executable).await,
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

/// The checker's report on `text` for `profile`'s CPU, as [`describe`] writes it, or why it
/// didn't build.
async fn report(
    profile: Profile,
    toolchain: &Toolchain,
    text: &str,
    ops: &[crate::edge::Op],
    threads: Option<u32>,
) -> String {
    let (source, layout) = check_layout(text);
    if !layout.passed() {
        return format!("layout\n{}", describe(&layout));
    }
    let lengua = lengua_source(profile, text);
    match built(profile, toolchain, text).await {
        Ok(bytes) => describe(&check_program(
            &source,
            &bytes,
            Context {
                ops,
                threads,
                lengua: lengua.as_ref(),
                features: CpuFeatures::all(),
                profile,
            },
        )),
        Err(message) => format!("build failed: {message}\n"),
    }
}

/// The recording of `profile`'s programs: each program's report in full, then a hash of each
/// mutant's.
async fn record(profile: Profile) -> String {
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
    for program in programs(profile) {
        let mut ids = Vec::new();
        let all = std::iter::once(("base".to_owned(), program.text.clone()))
            .chain(mutants(profile, &program.text));
        for (id, text) in all {
            let key = Sha256Hex::of(text.as_bytes()).as_str().to_owned();
            ids.push((id, key.clone()));
            if !started.insert(key.clone()) {
                continue;
            }
            let (toolchain, permits) = (Arc::clone(&toolchain), Arc::clone(&permits));
            let (ops, threads) = (Arc::clone(&program.ops), program.threads);
            tasks.spawn(async move {
                let _permit = permits.acquire_owned().await.expect("never closed");
                let found = report(profile, &toolchain, &text, &ops, threads).await;
                (key, found)
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

/// Writes `found` as the recording at `path`.
fn record_at(path: &Path, found: &str) {
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("make target/golden");
    std::fs::write(path, found).expect("record");
    eprintln!(
        "recorded {} ({} lines)",
        path.display(),
        found.lines().count()
    );
}

/// Compares `found` with the recording called `name`, or records it under
/// `LENGUA_GOLDEN=write`. Where there is no recording, as on a fresh checkout, it makes one and
/// notes in the run's summary that it compared nothing: the run is a smoke test of the guard's
/// machinery, and the guard around a refactor needs the recording of the commit before it.
pub(crate) fn compare_or_record(name: &str, found: &str) {
    let path = recording_path(name);
    if std::env::var("LENGUA_GOLDEN").as_deref() == Ok("write") {
        record_at(&path, found);
        return;
    }
    let Ok(recorded) = std::fs::read_to_string(&path) else {
        record_at(&path, found);
        crate::skiplog::note(&format!(
            "{name} had no recording to compare with (LENGUA_GOLDEN=write makes one at the commit \
             before a refactor): this run recorded it and compared nothing"
        ));
        return;
    };
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

/// The guard on `profile`'s programs: their reports, and those of their mutants, are as recorded.
/// Where there is no recording to compare with, the reports are made twice (the second time from
/// the cached builds) and must be the same, which at least shows the guard is deterministic.
async fn reports_are_as_recorded(profile: Profile) {
    let name = format!("checker-{}.txt", PLACES.get(profile).label);
    let writing = std::env::var("LENGUA_GOLDEN").as_deref() == Ok("write");
    let fresh = !writing && !recording_path(&name).is_file();
    let found = record(profile).await;
    compare_or_record(&name, &found);
    if fresh {
        let again = record(profile).await;
        assert!(
            again == found,
            "{name}: two runs of the checker on the same programs gave different reports"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Round 9's guard: run with --ignored around the checker's refactor"]
async fn every_report_is_as_recorded() {
    reports_are_as_recorded(Profile::LinuxX86_64).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Round 9's guard: run with --ignored around the checker's refactor"]
async fn every_report_is_as_recorded_on_aarch64() {
    reports_are_as_recorded(Profile::LinuxAarch64).await;
}

/// The template of each target, whose mutants the next test looks at.
const TEMPLATES: PerCpu<&str> = PerCpu {
    x86_64: include_str!("../../plugin/skills/compile/assets/main-template.s"),
    aarch64: include_str!("../../plugin/skills/compile/assets/linux-aarch64/main-template.s"),
};

#[test]
fn mutants_are_made_the_same_way_every_time() {
    for profile in Profile::ALL {
        let text = *TEMPLATES.get(profile);
        let name = profile.name();
        let (first, second) = (mutants(profile, text), mutants(profile, text));
        assert_eq!(first, second, "{name}");
        assert!(
            first.len() > 100 && first.len() <= MAX_MUTANTS,
            "{name}: {}",
            first.len()
        );
        let kinds: BTreeSet<&str> = first
            .iter()
            .filter_map(|(id, _)| id.split_once(':').map(|(_, kind)| kind))
            .collect();
        for kind in [
            "del", "dup", "swap", "reg", "imm0", "cond", "family", "annot",
        ] {
            assert!(kinds.contains(kind), "{name}: {kind} in {kinds:?}");
        }
        // Each mutant changes the program.
        assert!(first.iter().all(|(_, mutant)| mutant != text), "{name}");
    }
}

/// A mutant of each target's program moves what that CPU's instructions have: registers by their
/// names, the condition a jump or a select tests, and the addressing of a load.
#[test]
fn mutants_move_each_cpus_own_instructions() {
    assert_eq!(
        swap_register(Profile::LinuxX86_64, "rax, qword ptr [rsi + rcx*8]").as_deref(),
        Some("rbx, qword ptr [rsi + rcx*8]")
    );
    assert_eq!(
        swap_register(Profile::LinuxAarch64, "x0, [x1, x2, lsl #3]").as_deref(),
        Some("x1, [x1, x2, lsl #3]")
    );
    assert_eq!(
        swap_register(Profile::LinuxAarch64, "v3.16b, v4.16b").as_deref(),
        Some("v4.16b, v4.16b")
    );
    // x30 and sp are not swapped.
    assert_eq!(swap_register(Profile::LinuxAarch64, "x30, sp"), None);
    assert_eq!(
        flip_condition(Profile::LinuxX86_64, "jne", ".Lagain"),
        Some(("je".to_owned(), ".Lagain".to_owned()))
    );
    assert_eq!(
        flip_condition(Profile::LinuxAarch64, "b.ne", ".Lagain"),
        Some(("b.eq".to_owned(), ".Lagain".to_owned()))
    );
    assert_eq!(
        flip_condition(Profile::LinuxAarch64, "csel", "x0, x7, x0, lo"),
        Some(("csel".to_owned(), "x0, x7, x0, hs".to_owned()))
    );
    assert_eq!(
        flip_condition(Profile::LinuxAarch64, "cset", "w7, lo"),
        Some(("cset".to_owned(), "w7, hs".to_owned()))
    );
    assert_eq!(
        flip_condition(Profile::LinuxAarch64, "add", "x0, x1, x2"),
        None
    );
    assert_eq!(
        move_memory(Profile::LinuxAarch64, "w5, [x1, x4, lsl #2]").as_deref(),
        Some("w5, [x1, x4, lsl #3]")
    );
    assert_eq!(
        move_memory(Profile::LinuxAarch64, "x2, [sp, #16]").as_deref(),
        Some("x2, [sp, #24]")
    );
    assert_eq!(
        move_memory(Profile::LinuxAarch64, "w5, [x1]").as_deref(),
        Some("w5, [x1, #8]")
    );
    // A post-indexed pair keeps its writeback.
    assert_eq!(
        move_memory(Profile::LinuxAarch64, "x29, x30, [sp], #16"),
        None
    );
}

/// Prints the report on one program or mutant, for a golden that differs:
/// `LENGUA_GOLDEN_MUTANT="examples/sort/main.s L432:family"` (or `base`).
#[tokio::test]
#[ignore = "explains a golden's difference"]
async fn explains_a_mutant() {
    let wanted = std::env::var("LENGUA_GOLDEN_MUTANT").expect("LENGUA_GOLDEN_MUTANT names one");
    let (name, id) = wanted.split_once(' ').expect("a program and a mutant");
    let (profile, program) = Profile::ALL
        .into_iter()
        .flat_map(|profile| {
            programs(profile)
                .into_iter()
                .map(move |found| (profile, found))
        })
        .find(|(_, program)| program.name == name)
        .expect("a program in the golden");
    let text = if id == "base" {
        program.text.clone()
    } else {
        mutants(profile, &program.text)
            .into_iter()
            .find(|(found, _)| found == id)
            .expect("a mutant of it")
            .1
    };
    let toolchain = Toolchain::discover().await.expect("llvm-mc and ld.lld");
    eprintln!(
        "{}",
        report(profile, &toolchain, &text, &program.ops, program.threads).await
    );
}
