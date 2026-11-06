//! The templates built and run alone, with no server, on every target: LLVM assembles and links
//! each target's main template and a program that drives lengua's fill and assert_fail, each with
//! lengua.s after it, and these tests read the machine code and run the programs themselves. What
//! they hold is written once, with a cell for each target (`PerCpu`):
//!
//! - one machine instruction per source line, function by function, as `llvm-objdump` prints
//!   them, in main.s and in lengua.s: the step from the text to machine code that `check` relies
//!   on;
//! - the template against its spec's examples, run as a program of its own;
//! - fill and assert_fail as a program of their own: the same bytes however they arrive, and an
//!   assertion that fails as lengua's handler says;
//! - every word of both programs read by lengua's decoder as LLVM's disassembler reads it:
//!   AArch64's through lengua's own decoder and parser, x86-64's through iced-x86, the decoder
//!   `check` reads machine code with, held to LLVM's disassembly instruction by instruction.
//!
//! Every program is built for every target on any computer (LLVM builds for all of them) and its
//! machine code is read on any computer; it runs where this computer runs the target's programs,
//! natively or through QEMU's user mode, and notes in the run's summary where it cannot.

mod common;
mod per_target;

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use lengua::isa::Isa;
use lengua::platform::Profile;
use lengua::targets::PerCpu;

/// Functions by name, each with the mnemonics of its instructions in order.
type Functions = Vec<(String, Vec<String>)>;

/// Each target's main template, in the compile skill's assets.
const TEMPLATES: PerCpu<&str> = PerCpu {
    x86_64: include_str!("../plugin/skills/compile/assets/main-template.s"),
    aarch64: include_str!("../plugin/skills/compile/assets/linux-aarch64/main-template.s"),
};

/// The first line of each target's programs, which names the CPU's syntax.
const FIRST_LINES: PerCpu<&str> = PerCpu {
    x86_64: ".intel_syntax noprefix\n",
    aarch64: ".arch armv8-a\n",
};

/// What a target's program of fill and assert_fail is made of besides lengua's text: the `main`
/// that drives them. With no arguments it copies standard input up to a newline that a read ends
/// with into a buffer of 64 bytes and writes what it read; with one it fails assertion 123.
const DRIVERS: PerCpu<&str> = PerCpu {
    x86_64: concat!(
        ".text\n",
        ".type main, @function\n",
        "main:\n",
        "    sub rsp, 8\n",
        "    cmp rdi, 2\n",
        "    je .Lassert\n",
        "    xor edi, edi\n",
        "    lea rsi, [rip + buf]\n",
        "    mov edx, 64\n",
        "    mov ecx, 10\n",
        "    call fill\n",
        "    test rax, rax\n",
        "    js .Lout\n",
        "    mov rdx, rax\n",
        "    mov edi, 1\n",
        "    lea rsi, [rip + buf]\n",
        "    call os_write\n",
        ".Lout:\n",
        "    xor edi, edi\n",
        "    call os_exit\n",
        ".Lassert:\n",
        "    mov edi, 123\n",
        "    call assert_fail\n",
        ".size main, . - main\n"
    ),
    aarch64: concat!(
        ".text\n",
        ".type main, @function\n",
        "main:\n",
        "    cmp x0, #2\n",
        "    b.eq .Lassert\n",
        "    mov x0, #0\n",
        "    adrp x1, buf\n",
        "    add x1, x1, :lo12:buf\n",
        "    mov x2, #64\n",
        "    mov w3, #10\n",
        "    bl fill\n",
        "    tbnz x0, #63, .Lout\n",
        "    mov x2, x0\n",
        "    mov x0, #1\n",
        "    adrp x1, buf\n",
        "    add x1, x1, :lo12:buf\n",
        "    bl os_write\n",
        ".Lout:\n",
        "    mov x0, #0\n",
        "    bl os_exit\n",
        ".Lassert:\n",
        "    mov w0, #123\n",
        "    bl assert_fail\n",
        ".size main, . - main\n"
    ),
};

/// `target`'s program of fill and assert_fail: a buffer and the driver.
fn fill_program(target: Profile) -> String {
    format!(
        "{first}.section .bss\n.type buf, @object\nbuf:\n    .zero 64\n.size buf, . - buf\n\n\
         {driver}",
        first = FIRST_LINES.get(target),
        driver = DRIVERS.get(target),
    )
}

/// What `per_target::build` assembles for `source`, a program of the edge form: main.s with
/// lengua's text after it, whose `_start` calls `main` and which holds the operations and the
/// library functions it calls.
fn assembled(target: Profile, source: &str) -> String {
    let edge = lengua::edge::lengua_text(target, source).expect("a program of the edge form");
    format!("{source}\n{edge}")
}

/// The condition codes an assembler takes under several names, each with the name LLVM's
/// disassembler prints.
const CONDITIONS: &[(&str, &str)] = &[
    ("z", "e"),
    ("nz", "ne"),
    ("c", "b"),
    ("nae", "b"),
    ("nc", "ae"),
    ("nb", "ae"),
    ("na", "be"),
    ("nbe", "a"),
    ("nge", "l"),
    ("nl", "ge"),
    ("ng", "le"),
    ("nle", "g"),
    ("pe", "p"),
    ("po", "np"),
];

/// A mnemonic as LLVM's disassembler prints it. On x86-64 the alias of a condition code is the
/// code's first name (`jz` is printed `je`) and `movabs` is the `mov` of a 64-bit number; AArch64's
/// are printed as written.
fn printed_as(isa: Isa, mnemonic: &str) -> String {
    if isa != Isa::X86_64 {
        return mnemonic.to_owned();
    }
    if mnemonic == "movabs" {
        return "mov".to_owned();
    }
    for prefix in ["j", "cmov", "set"] {
        if let Some(condition) = mnemonic.strip_prefix(prefix)
            && let Some((_, printed)) = CONDITIONS.iter().find(|(alias, _)| *alias == condition)
        {
            return format!("{prefix}{printed}");
        }
    }
    mnemonic.to_owned()
}

/// The source's instructions, function by function: each function's name and its lines'
/// mnemonics, leaving out labels, comments, directives and data.
fn source_functions(isa: Isa, source: &str) -> Functions {
    let mut functions = Vec::new();
    let mut current: Option<(String, Vec<String>)> = None;
    for line in source.lines() {
        let code = line.split(isa.comment()).next().unwrap_or_default().trim();
        if let Some(rest) = code.strip_prefix(".type ")
            && let Some(name) = rest.strip_suffix(", @function")
        {
            current = Some((name.trim().to_owned(), Vec::new()));
        } else if code.starts_with(".size ") {
            if let Some(function) = current.take() {
                functions.push(function);
            }
        } else if let Some((_, mnemonics)) = current.as_mut()
            && !code.is_empty()
            && !code.ends_with(':')
            && !code.starts_with('.')
        {
            let mnemonic = code.split_whitespace().next().unwrap_or_default();
            mnemonics.push(printed_as(isa, mnemonic));
        }
    }
    functions
}

/// What `llvm-objdump` prints of the executable's code. It prints x86-64 in AT&T syntax, whose
/// mnemonics carry size suffixes, unless asked for Intel's, which it takes as an option of its own
/// or as a disassembler option, depending on its release.
fn disassembled(isa: Isa, executable: &Path) -> String {
    let attempts: &[&[&str]] = match isa {
        Isa::X86_64 => &[&["--x86-asm-syntax=intel"], &["-M", "intel"]],
        Isa::Aarch64 => &[&[]],
    };
    let mut failures = Vec::new();
    for options in attempts {
        let dumped = Command::new("llvm-objdump")
            .args(["-d", "--no-show-raw-insn"])
            .args(*options)
            .arg(executable)
            .output()
            .expect("llvm-objdump");
        let text = String::from_utf8_lossy(&dumped.stdout).into_owned();
        // AT&T syntax names its registers with a percent sign.
        if dumped.status.success() && !text.contains('%') {
            return text;
        }
        failures.push(format!(
            "{options:?}: {} {}",
            dumped.status,
            String::from_utf8_lossy(&dumped.stderr)
        ));
    }
    panic!("llvm-objdump didn't print {isa:?} as asked: {failures:#?}");
}

/// The executable's instructions, function by function, as llvm-objdump prints them.
fn machine_functions(isa: Isa, executable: &Path) -> Functions {
    let mut functions: Functions = Vec::new();
    for line in disassembled(isa, executable).lines() {
        // `0000000000210120 <_start>:` starts a function; `  210120:      \tmov\tx0, #1` is an
        // instruction.
        if let Some(name) = line
            .split_once(" <")
            .filter(|(address, _)| address.chars().all(|c| c.is_ascii_hexdigit()))
            .and_then(|(_, rest)| rest.strip_suffix(">:"))
        {
            functions.push((name.to_owned(), Vec::new()));
        } else if line.starts_with(' ')
            && let Some((_, instruction)) = line.trim_start().split_once(':')
            && let Some((_, mnemonics)) = functions.last_mut()
        {
            let mnemonic = instruction.split_whitespace().next().unwrap_or_default();
            mnemonics.push(printed_as(isa, mnemonic));
        }
    }
    functions
}

/// The program's source has as many instructions as its machine code, function by function, and
/// each is the same instruction.
fn one_instruction_per_line(target: Profile, what: &str, source: &str, executable: &Path) {
    let isa = target.isa();
    let written = source_functions(isa, source);
    let built = machine_functions(isa, executable);
    let cpu = target.name();
    assert_eq!(
        written.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        built.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        "{what} on {cpu}: the functions, in order"
    );
    for ((name, source), (_, machine)) in written.iter().zip(&built) {
        assert_eq!(
            source, machine,
            "{what} on {cpu}, {name}: one instruction per line, as written"
        );
    }
}

/// Runs `executable` with `arguments` and `stdin`; its stdout, stderr and exit code.
fn run(executable: &Path, arguments: &[&str], stdin: &[&[u8]]) -> (String, String, Option<i32>) {
    let mut child = Command::new(executable)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the program");
    let mut input = child.stdin.take().expect("its standard input");
    for (index, piece) in stdin.iter().enumerate() {
        if index > 0 {
            // A pause, so the program reads each piece apart.
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        input.write_all(piece).expect("write a piece");
        input.flush().expect("flush");
    }
    drop(input);
    let output = child.wait_with_output().expect("the program ends");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

#[tokio::test]
async fn the_template_links_one_instruction_per_line() {
    let folder = tempfile::tempdir().expect("temporary folder");
    for target in Profile::ALL {
        let source = *TEMPLATES.get(target);
        assert!(
            source.starts_with(*FIRST_LINES.get(target)),
            "line 1 of the {} template names the CPU",
            target.name()
        );
        let place = folder.path().join(target.name());
        let executable = per_target::build(target, &place, "main", source).await;
        let program = assembled(target, source);
        one_instruction_per_line(target, "the template", &program, &executable);
    }
}

/// A run of a program: its arguments, then what it prints to standard output and to standard
/// error and the code it exits with.
type Case = (&'static [&'static str], &'static str, &'static str, i32);

/// template-spec.md's examples, its bounds, and the texts its B3 tries: the same for every
/// target, as the spec is.
const NEGATE_CASES: &[Case] = &[
    (&["7"], "-7\n", "", 0),
    (&["0"], "0\n", "", 0),
    (&["-0"], "0\n", "", 0),
    (&["9223372036854775807"], "-9223372036854775807\n", "", 0),
    (&["-9223372036854775807"], "9223372036854775807\n", "", 0),
    (&["-1234567"], "1234567\n", "", 0),
    (&[], "", "usage: negate <n>\n", 1),
    (&["1", "2"], "", "usage: negate <n>\n", 1),
    (&["12x"], "", "error: not a number\n", 2),
    (&["abc"], "", "error: not a number\n", 2),
    (&[""], "", "error: not a number\n", 2),
    (&["-"], "", "error: not a number\n", 2),
    (&["--5"], "", "error: not a number\n", 2),
    (&["+5"], "", "error: not a number\n", 2),
    (&["1-2"], "", "error: not a number\n", 2),
    (&["9223372036854775808"], "", "error: not a number\n", 2),
    (&["-9223372036854775808"], "", "error: not a number\n", 2),
];

#[tokio::test]
async fn the_template_does_what_its_spec_says() {
    let folder = tempfile::tempdir().expect("temporary folder");
    for target in Profile::ALL {
        let place = folder.path().join(target.name());
        let executable = per_target::build(target, &place, "main", TEMPLATES.get(target)).await;
        if per_target::way(target).is_none() {
            continue;
        }
        for (arguments, stdout, stderr, code) in NEGATE_CASES {
            assert_eq!(
                run(&executable, arguments, &[]),
                ((*stdout).to_owned(), (*stderr).to_owned(), Some(*code)),
                "{} with {arguments:?}",
                target.name()
            );
        }
    }
}

#[tokio::test]
async fn fill_and_assert_fail_behave_as_a_program_of_their_own() {
    let folder = tempfile::tempdir().expect("temporary folder");
    for target in Profile::ALL {
        let source = fill_program(target);
        let place = folder.path().join(target.name());
        let executable = per_target::build(target, &place, "main", &source).await;
        let program = assembled(target, &source);
        one_instruction_per_line(target, "the fill program", &program, &executable);
        if per_target::way(target).is_none() {
            continue;
        }
        let name = target.name();
        assert_eq!(
            run(&executable, &["x"], &[]),
            (
                String::new(),
                "lengua: assertion 123 failed\n".into(),
                Some(134)
            ),
            "{name}: assertion 123"
        );
        // The same bytes however they arrive: in one piece, or cut before the newline.
        let cuts: [&[&[u8]]; 3] = [&[b"abc\n"], &[b"ab", b"c\n"], &[b"a", b"b", b"c", b"\n"]];
        for pieces in cuts {
            assert_eq!(
                run(&executable, &[], pieces),
                ("abc\n".into(), String::new(), Some(0)),
                "{name}: {pieces:?}"
            );
        }
        // Input with no newline, read to its end; and none at all.
        assert_eq!(
            run(&executable, &[], &[b"no newline"]),
            ("no newline".into(), String::new(), Some(0)),
            "{name}: no newline"
        );
        assert_eq!(
            run(&executable, &[], &[]),
            (String::new(), String::new(), Some(0)),
            "{name}: no input"
        );
    }
}

/// The address of an executable's `.text` and its bytes.
fn text_section(executable: &Path) -> (u64, Vec<u8>) {
    use object::{Object, ObjectSection};
    let bytes = std::fs::read(executable).expect("the executable");
    let file = object::File::parse(&*bytes).expect("an ELF file");
    let text = file.section_by_name(".text").expect("a .text section");
    (text.address(), text.data().expect("its bytes").to_vec())
}

/// What LLVM's disassembler makes of `bytes`, which are one instruction: its mnemonic and its
/// operands. LLVM prints x86-64 in AT&T syntax unless asked for Intel's, output variant 1, which
/// releases take under two names.
fn llvm_reads(target: Profile, bytes: &[u8]) -> (String, String) {
    let attempts: &[&[&str]] = match target.isa() {
        Isa::X86_64 => &[&["--output-asm-variant=1"], &["--x86-asm-syntax=intel"]],
        Isa::Aarch64 => &[&[]],
    };
    let input: String = bytes.iter().map(|byte| format!("{byte:#04x} ")).collect();
    let mut failures = Vec::new();
    for options in attempts {
        let mut child = Command::new("llvm-mc")
            .arg("--disassemble")
            .arg(format!("--triple={}", target.llvm_triple()))
            .args(*options)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("llvm-mc");
        if let Some(mut stdin) = child.stdin.take() {
            // llvm-mc may refuse an option and end before it reads: its answer is read below.
            let _ = stdin.write_all(input.as_bytes());
        }
        let output = child.wait_with_output().expect("llvm-mc ends");
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        let instructions: Vec<&str> = text.lines().filter(|row| row.starts_with('\t')).collect();
        if output.status.success() && instructions.len() == 1 && !text.contains('%') {
            let code = instructions[0]
                .split(target.isa().comment())
                .next()
                .unwrap_or_default()
                .trim();
            let (mnemonic, operands) = code.split_once('\t').unwrap_or((code, ""));
            return (mnemonic.trim().to_owned(), operands.trim().to_owned());
        }
        failures.push(format!(
            "{options:?}: {} {text:?} {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    panic!("llvm-mc didn't read {bytes:02x?} as one instruction of {target:?}: {failures:#?}");
}

/// The words one disassembler writes where the other leaves them out: the sizes before an
/// address (`qword ptr`) and the kinds of a branch (iced-x86 writes `jne short`).
const SIZES: &[&str] = &[
    "byte", "word", "dword", "qword", "tbyte", "fword", "oword", "xmmword", "ymmword", "zmmword",
    "ptr", "short", "near", "far",
];

/// Whether `word` is a hexadecimal number as iced-x86 writes it, with an `h` after it.
fn is_hexadecimal(word: &str) -> bool {
    let Some(digits) = word.strip_suffix('h') else {
        return false;
    };
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_hexdigit())
}

/// The register names among an instruction's operands, each once and in lowercase: the words that
/// start with a letter, less the sizes and the numbers written with a letter. iced-x86 writes
/// `imul rax, 0Ah` where LLVM writes `imul rax, rax, 10`, and the address of an operand relative
/// to `rip` where LLVM writes `[rip - 4231]`, so a register is held to be among them, never how
/// many times it is named, and `rip` is left out.
fn registers(operands: &str) -> BTreeSet<String> {
    operands
        .split(|c: char| !c.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|word| word.starts_with(|c: char| c.is_ascii_alphabetic()))
        .filter(|word| !SIZES.contains(&word.as_str()) && !is_hexadecimal(word) && word != "rip")
        .collect()
}

/// Every word of an AArch64 program is in the dialect: lengua's decoder reads each, and reads it
/// as LLVM's disassembler does, through lengua's parser. The words it doesn't, each with why.
fn aarch64_words_that_disagree(what: &str, executable: &Path) -> Vec<String> {
    use lengua::isa::aarch64::{decode::decode, text::parse};
    let (base, bytes) = text_section(executable);
    let (words, _) = bytes.as_chunks::<4>();
    assert!(words.len() > 50, "{what}: {} words", words.len());
    let mut problems = Vec::new();
    for (word, address) in words.iter().zip((base..).step_by(4)) {
        let word = u32::from_le_bytes(*word);
        let (mnemonic, operands) = llvm_reads(Profile::LinuxAarch64, &word.to_le_bytes());
        let here = format!("{what} at {address:#x}: `{mnemonic} {operands}`");
        match (
            decode(word, address),
            parse(&mnemonic, &operands, address, &|_| None),
        ) {
            (Ok(decoded), Ok(parsed)) if decoded == parsed => {}
            (decoded, parsed) => problems.push(format!("{here}: {decoded:?} against {parsed:?}")),
        }
    }
    problems
}

/// Every instruction of an x86-64 program is read by iced-x86, the decoder `check` reads machine
/// code with, as LLVM's disassembler reads the same bytes: one instruction, the same mnemonic and
/// the same registers. The instructions it doesn't, each with why.
fn x86_words_that_disagree(what: &str, executable: &Path) -> Vec<String> {
    use lengua::checker::{decode, x86_64};
    let (base, text) = text_section(executable);
    let bytes = std::fs::read(executable).expect("the executable");
    let machine = decode::read(&bytes).expect("lengua reads the executable");
    let mut counted = 0;
    let mut problems = Vec::new();
    for (function, code) in &machine.functions {
        for instruction in code.x86() {
            let at = usize::try_from(instruction.ip() - base).expect("an offset in .text");
            let encoded = &text[at..at + instruction.len()];
            let (mnemonic, operands) = llvm_reads(Profile::LinuxX86_64, encoded);
            let shown = x86_64::display(instruction);
            let here = format!(
                "{what}, {function} at {:#x}: `{shown}` is `{mnemonic} {operands}` to LLVM",
                instruction.ip()
            );
            if x86_64::mnemonic(instruction) != printed_as(Isa::X86_64, &mnemonic) {
                problems.push(format!("{here}: the mnemonics differ"));
            }
            let ours = shown.split_once(' ').map_or("", |(_, operands)| operands);
            let (iced, llvm) = (registers(ours), registers(&operands));
            if iced != llvm {
                problems.push(format!("{here}: registers {iced:?} against {llvm:?}"));
            }
            counted += 1;
        }
    }
    assert!(counted > 50, "{what}: {counted} instructions");
    problems
}

#[tokio::test]
async fn every_template_word_is_in_the_dialect() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let mut problems = Vec::new();
    for target in Profile::ALL {
        for (what, source) in [
            ("the template", (*TEMPLATES.get(target)).to_owned()),
            ("the fill program", fill_program(target)),
        ] {
            let place = folder
                .path()
                .join(target.name())
                .join(what.replace(' ', "-"));
            let executable = per_target::build(target, &place, "main", &source).await;
            problems.extend(match target.isa() {
                Isa::Aarch64 => aarch64_words_that_disagree(what, &executable),
                Isa::X86_64 => x86_words_that_disagree(what, &executable),
            });
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}
