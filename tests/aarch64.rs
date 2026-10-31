//! The Linux AArch64 templates (Round 9, R0): they assemble and link one machine instruction per
//! source line, and do what their x86-64 counterparts do, run natively on an ARM64 computer or
//! under QEMU on another (when binfmt hands AArch64 programs to qemu-aarch64 with flag F, which
//! opens QEMU once, so Landlock later can't keep it from starting).

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const TRIPLE: &str = "aarch64-unknown-linux-gnu";

fn asset(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugin/skills/compile/assets/linux-aarch64")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Builds `source` into an AArch64 executable in `folder`. The link keeps every instruction as
/// written: by default ld.lld rewrites some `adrp` + `add :lo12:` pairs into `nop` + `adr`, which
/// would break lengua's line-for-line match of main.s to the machine code.
fn build(folder: &Path, source: &str) -> PathBuf {
    let (assembly, object, executable) = (
        folder.join("main.s"),
        folder.join("main.o"),
        folder.join("main"),
    );
    std::fs::write(&assembly, source).expect("write main.s");
    let assembled = Command::new("llvm-mc")
        .arg(format!("--triple={TRIPLE}"))
        .args(["--filetype=obj", "--fatal-warnings"])
        .arg(&assembly)
        .arg("-o")
        .arg(&object)
        .output()
        .expect("llvm-mc");
    assert!(
        assembled.status.success(),
        "{}",
        String::from_utf8_lossy(&assembled.stderr)
    );
    let linked = Command::new("ld.lld")
        .args(["--fatal-warnings", "--no-relax", "-o"])
        .arg(&executable)
        .arg(&object)
        .output()
        .expect("ld.lld");
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    executable
}

/// Whether AArch64 programs run here: natively, or through binfmt's qemu-aarch64 with flag F.
fn runs_aarch64() -> bool {
    if cfg!(target_arch = "aarch64") {
        return true;
    }
    let entry =
        std::fs::read_to_string("/proc/sys/fs/binfmt_misc/qemu-aarch64").unwrap_or_default();
    let ready = entry.lines().next() == Some("enabled")
        && entry.lines().any(|line| {
            line.strip_prefix("flags: ")
                .is_some_and(|flags| flags.contains('F'))
        });
    if !ready {
        eprintln!("skipped: AArch64 programs don't run here (install qemu-user-static)");
    }
    ready
}

/// The source's instructions, function by function: each function's name and its lines'
/// mnemonics, leaving out labels, comments, directives and data.
fn source_functions(source: &str) -> Vec<(String, Vec<String>)> {
    let mut functions = Vec::new();
    let mut current: Option<(String, Vec<String>)> = None;
    for line in source.lines() {
        let code = line.split("//").next().unwrap_or_default().trim();
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
            mnemonics.push(mnemonic.to_owned());
        }
    }
    functions
}

/// The executable's instructions, function by function, as llvm-objdump prints them.
fn machine_functions(executable: &Path) -> Vec<(String, Vec<String>)> {
    let dumped = Command::new("llvm-objdump")
        .args(["-d", "--no-show-raw-insn"])
        .arg(executable)
        .output()
        .expect("llvm-objdump");
    assert!(dumped.status.success());
    let mut functions: Vec<(String, Vec<String>)> = Vec::new();
    for line in String::from_utf8_lossy(&dumped.stdout).lines() {
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
            mnemonics.push(mnemonic.to_owned());
        }
    }
    functions
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

#[test]
fn the_aarch64_template_links_one_instruction_per_line() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let source = asset("main-template.s");
    assert!(
        source.starts_with(".arch armv8-a\n"),
        "line 1 names the CPU"
    );
    let executable = build(folder.path(), &source);
    let written = source_functions(&source);
    let built = machine_functions(&executable);
    assert_eq!(
        written.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        built.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        "the functions, in order"
    );
    for ((name, source), (_, machine)) in written.iter().zip(&built) {
        assert_eq!(
            source, machine,
            "{name}: one instruction per line, as written"
        );
    }
}

#[test]
fn the_aarch64_template_does_what_its_spec_says() {
    if !runs_aarch64() {
        return;
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let executable = build(folder.path(), &asset("main-template.s"));
    // template-spec.md's examples, its bounds, and the texts its B3 tries.
    assert_eq!(
        run(&executable, &["7"], &[]),
        ("-7\n".into(), String::new(), Some(0))
    );
    for (n, negated) in [
        ("0", "0"),
        ("-0", "0"),
        ("9223372036854775807", "-9223372036854775807"),
        ("-9223372036854775807", "9223372036854775807"),
        ("-1234567", "1234567"),
    ] {
        assert_eq!(
            run(&executable, &[n], &[]),
            (format!("{negated}\n"), String::new(), Some(0)),
            "{n}"
        );
    }
    let usage = (String::new(), "usage: negate <n>\n".to_owned(), Some(1));
    assert_eq!(run(&executable, &[], &[]), usage);
    assert_eq!(run(&executable, &["1", "2"], &[]), usage);
    for text in [
        "12x",
        "abc",
        "",
        "-",
        "--5",
        "+5",
        "1-2",
        "9223372036854775808",
        "-9223372036854775808",
    ] {
        assert_eq!(
            run(&executable, &[text], &[]),
            (String::new(), "error: not a number\n".into(), Some(2)),
            "{text:?}"
        );
    }
}

#[test]
fn aarch64_fill_and_assert_fail_behave_as_on_x86_64() {
    let template = asset("main-template.s");
    let fill = asset("fill.s");
    let between = |text: &str, start: &str, end: &str| -> String {
        let from = text.find(start).expect("the start");
        let to = from + text[from..].find(end).expect("the end") + end.len();
        format!("{}\n", &text[from..to])
    };
    // fill's constants and function, assert_fail and its message, and a driver: with no
    // arguments it copies standard input up to a newline a read ends with; with one, it fails
    // assertion 123.
    let source = format!(
        ".arch armv8-a\n{constants}\n.section .rodata\n{message}\n.section .bss\n\
         .type buf, @object\nbuf:\n    .zero 64\n.size buf, . - buf\n\n.text\n.globl _start\n\
         .type _start, @function\n_start:\n    ldr x0, [sp]\n    cmp x0, #2\n    b.eq .Lassert\n\
         \x20   mov x0, #0\n    adrp x1, buf\n    add x1, x1, :lo12:buf\n    mov x2, #64\n\
         \x20   mov w3, #10\n    bl fill\n    tbnz x0, #63, .Lout\n    mov x2, x0\n\
         \x20   mov x0, #1\n    adrp x1, buf\n    add x1, x1, :lo12:buf\n    mov x8, #64\n\
         \x20   svc #0\n.Lout:\n    mov x0, #0\n    mov x8, #93\n    svc #0\n.Lassert:\n\
         \x20   mov w0, #123\n    bl assert_fail\n.size _start, . - _start\n\n{fill}\n{assert}",
        constants = between(&fill, ".equ SYS_READ", "MAX_FILL_READS, 1099511627776"),
        message = between(
            &template,
            ".type assert_message",
            ".size assert_message, . - assert_message"
        ),
        fill = between(&fill, ".type fill, @function", ".size fill, . - fill"),
        assert = between(
            &template,
            ".type assert_fail, @function",
            ".size assert_fail, . - assert_fail"
        ),
    );
    let folder = tempfile::tempdir().expect("temporary folder");
    let executable = build(folder.path(), &source);
    for ((name, written), (_, built)) in source_functions(&source)
        .iter()
        .zip(&machine_functions(&executable))
    {
        assert_eq!(
            written, built,
            "{name}: one instruction per line, as written"
        );
    }
    if !runs_aarch64() {
        return;
    }
    assert_eq!(
        run(&executable, &["x"], &[]),
        (
            String::new(),
            "lengua: assertion 123 failed\n".into(),
            Some(134)
        )
    );
    // The same bytes however they arrive: in one piece, or cut before the newline.
    let cuts: [&[&[u8]]; 3] = [&[b"abc\n"], &[b"ab", b"c\n"], &[b"a", b"b", b"c", b"\n"]];
    for pieces in cuts {
        assert_eq!(
            run(&executable, &[], pieces),
            ("abc\n".into(), String::new(), Some(0)),
            "{pieces:?}"
        );
    }
    // Input with no newline, read to its end; and none at all.
    assert_eq!(
        run(&executable, &[], &[b"no newline"]),
        ("no newline".into(), String::new(), Some(0))
    );
    assert_eq!(
        run(&executable, &[], &[]),
        (String::new(), String::new(), Some(0))
    );
}
