//! The decoder held to LLVM's disassembler. For random words in each encoding class the dialect
//! has, and fully random ones: where LLVM reads a word, lengua's decoding of it is what lengua's
//! own parser reads from LLVM's text for it, or both say it's outside the dialect; where LLVM
//! refuses a word, or warns that its result isn't defined, lengua refuses it too.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::process::{Command, Stdio};

use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

use super::decode::decode;
use super::text::parse;

/// Where every word is taken to be: branch targets are relative to it.
const ADDRESS: u64 = 0x40_1000;

/// The random words drawn in each class.
const PER_CLASS: usize = 3000;

/// Each encoding class the dialect has: its name, the bits that place a word in it, and their
/// value. The classes whose fields are mostly unallocated fix more bits, so random words land on
/// instructions.
const CLASSES: [(&str, u32, u32); 33] = [
    ("adr and adrp", 0x1f00_0000, 0x1000_0000),
    ("add and sub with a number", 0x1f80_0000, 0x1100_0000),
    ("logical with a bitmask", 0x1f80_0000, 0x1200_0000),
    ("move wide", 0x1f80_0000, 0x1280_0000),
    ("bitfield", 0x1f80_0000, 0x1300_0000),
    ("extract", 0x7fa0_0000, 0x1380_0000),
    ("b and bl", 0x7c00_0000, 0x1400_0000),
    ("cbz and cbnz", 0x7e00_0000, 0x3400_0000),
    ("tbz and tbnz", 0x7e00_0000, 0x3600_0000),
    ("b.cond", 0xff00_0010, 0x5400_0000),
    ("svc", 0xffe0_001f, 0xd400_0001),
    ("br, blr and ret", 0xff9f_fc1f, 0xd61f_0000),
    (
        "loads and stores, unsigned offset",
        0x3b00_0000,
        0x3900_0000,
    ),
    (
        "loads and stores, unscaled and moving",
        0x3b20_0000,
        0x3800_0000,
    ),
    (
        "loads and stores, register offset",
        0x3b20_0c00,
        0x3820_0800,
    ),
    ("pairs", 0x3a00_0000, 0x2800_0000),
    ("logical with a shifted register", 0x1f00_0000, 0x0a00_0000),
    (
        "add and sub with a shifted register",
        0x1f20_0000,
        0x0b00_0000,
    ),
    ("conditional select", 0x1fe0_0000, 0x1a80_0000),
    ("two sources", 0x7fe0_c000, 0x1ac0_0000),
    ("one source", 0x7fff_e000, 0x5ac0_0000),
    ("three sources", 0x1f00_0000, 0x1b00_0000),
    ("fp one source", 0xff20_7c00, 0x1e20_4000),
    ("fp two sources", 0xff20_0c00, 0x1e20_0800),
    ("fp three sources", 0xff00_0000, 0x1f00_0000),
    ("fp compare", 0xff20_fc07, 0x1e20_2000),
    ("fp conditional select", 0xff20_0c00, 0x1e20_0c00),
    ("fp immediate", 0xff20_1fe0, 0x1e20_1000),
    ("fp and general register moves", 0x7f20_fc00, 0x1e20_0000),
    (
        "fp loads and stores, unsigned offset",
        0x3f00_0000,
        0x3d00_0000,
    ),
    (
        "fp loads and stores, unscaled and moving",
        0x3f20_0000,
        0x3c00_0000,
    ),
    (
        "fp loads and stores, register offset",
        0x3f20_0c00,
        0x3c20_0800,
    ),
    ("fp pairs", 0x3c00_0000, 0x2c00_0000),
];

/// What LLVM's disassembler says of each word: its text, or none where it refuses the word or
/// warns that its result isn't defined.
fn llvm(words: &[u32]) -> Option<Vec<Option<String>>> {
    let mut child = Command::new("llvm-mc")
        .args(["--disassemble", "--triple=aarch64-unknown-linux-gnu"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let mut input = String::new();
    for word in words {
        let [a, b, c, d] = word.to_le_bytes();
        input.push_str(&format!("{a:#04x} {b:#04x} {c:#04x} {d:#04x}\n"));
    }
    let mut stdin = child.stdin.take()?;
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child.wait_with_output().ok()?;
    writer.join().ok()?.ok()?;
    // `<stdin>:N:1: warning: …`, for the Nth word.
    let mut refused = BTreeSet::new();
    let mut undefined = BTreeSet::new();
    for line in String::from_utf8_lossy(&output.stderr).lines() {
        let Some(rest) = line.strip_prefix("<stdin>:") else {
            continue;
        };
        let Some(number) = rest.split(':').next().and_then(|n| n.parse::<usize>().ok()) else {
            continue;
        };
        if line.contains("invalid instruction encoding") {
            refused.insert(number - 1);
        } else if line.contains("warning") {
            undefined.insert(number - 1);
        }
    }
    let mut texts = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.starts_with('\t'))
        .map(str::to_owned)
        .collect::<Vec<_>>()
        .into_iter();
    let found = (0..words.len())
        .map(|index| {
            if refused.contains(&index) {
                return None;
            }
            let text = texts.next();
            text.filter(|_| !undefined.contains(&index))
        })
        .collect();
    Some(found)
}

/// The words to try: random ones in each class, and random ones anywhere.
fn words() -> Vec<u32> {
    let mut rng = ChaCha8Rng::seed_from_u64(64);
    let mut found = Vec::new();
    for (_, mask, value) in CLASSES {
        for _ in 0..PER_CLASS {
            found.push((rng.random::<u32>() & !mask) | value);
        }
    }
    for _ in 0..30_000 {
        found.push(rng.random());
    }
    found
}

#[test]
fn the_decoder_reads_every_word_as_llvm_does() {
    let words = words();
    let Some(texts) = llvm(&words) else {
        eprintln!("llvm-mc isn't installed: not checked");
        return;
    };
    let mut problems = Vec::new();
    let mut agreed = 0;
    // How many words of each class both read alike, by the class's place in [`CLASSES`].
    let mut agreed_by_class = vec![0_usize; CLASSES.len()];
    for (place, (word, text)) in words.iter().zip(&texts).enumerate() {
        let decoded = decode(*word, ADDRESS);
        let Some(text) = text else {
            if let Ok(inst) = decoded {
                problems.push(format!(
                    "{word:#010x}: LLVM refuses it; lengua reads {inst:?}"
                ));
            }
            continue;
        };
        let code = text.split("//").next().unwrap_or_default().trim();
        let (mnemonic, operands) = code.split_once('\t').unwrap_or((code, ""));
        let parsed = parse(mnemonic.trim(), operands.trim(), ADDRESS, &|_| None);
        match (decoded, parsed) {
            (Ok(decoded), Ok(parsed)) if decoded == parsed => {
                agreed += 1;
                if let Some(count) = agreed_by_class.get_mut(place / PER_CLASS) {
                    *count += 1;
                }
            }
            (Ok(decoded), Ok(parsed)) => problems.push(format!(
                "{word:#010x} `{code}`: lengua decodes {decoded:?} but reads {parsed:?}"
            )),
            (Ok(decoded), Err(why)) => problems.push(format!(
                "{word:#010x} `{code}`: lengua decodes {decoded:?} but can't read LLVM's text: \
                 {why}"
            )),
            (Err(why), Ok(parsed)) => problems.push(format!(
                "{word:#010x} `{code}`: lengua reads {parsed:?} but refuses the word: {why}"
            )),
            (Err(_), Err(_)) => {}
        }
    }
    // A few disagreements of each mnemonic, so every kind shows.
    let mut shown: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for problem in &problems {
        let mnemonic = problem
            .split('`')
            .nth(1)
            .and_then(|code| code.split_whitespace().next())
            .unwrap_or("(refused)");
        let these = shown.entry(mnemonic).or_default();
        if these.len() < 3 {
            these.push(problem);
        }
    }
    let sample: Vec<&str> = shown.values().flatten().copied().collect();
    assert!(
        problems.is_empty(),
        "{} of {} words disagree ({agreed} agree):\n{}",
        problems.len(),
        words.len(),
        sample.join("\n")
    );
    assert!(agreed > 30_000, "only {agreed} words agree");
    // Every class has many words in the dialect, so none passes by both sides refusing it all.
    for ((name, ..), count) in CLASSES.iter().zip(&agreed_by_class) {
        eprintln!("{name}: {count} of {PER_CLASS} agree");
        assert!(
            *count >= PER_CLASS / 50,
            "{name}: only {count} of {PER_CLASS} agree"
        );
    }
}

/// The facts of one line of the dialect, at [`ADDRESS`], with `label` at 0x40_2000.
fn facts_of(line: &str) -> crate::isa::Insn {
    let (mnemonic, operands) = line.split_once(' ').unwrap_or((line, ""));
    let names = |name: &str| (name == "label").then_some(0x40_2000);
    let inst = parse(mnemonic, operands, ADDRESS, &names).unwrap_or_else(|why| panic!("{why}"));
    super::facts::facts(&inst, ADDRESS)
}

#[test]
fn the_facts_say_what_each_idiom_does() {
    use super::{A64Reg, ABI};
    use crate::isa::{Cond, Control, Op, Operand, Reg, Stack, Value};
    let x = |number| Reg::A64(A64Reg::new(number, true, false));
    let w = |number| Reg::A64(A64Reg::new(number, false, false));
    let sp = Reg::A64(A64Reg::new(31, true, true));

    let compare = facts_of("cmp x0, #5");
    assert_eq!(
        compare.op,
        Op::Compare {
            left: Operand::Reg(x(0)),
            right: Operand::Imm(5)
        }
    );
    assert!(compare.uses(x(0)) && !compare.changes(x(0), &ABI));
    assert_eq!(
        facts_of("mov x8, #64").op,
        Op::MovImm {
            dst: x(8),
            value: 64
        }
    );
    assert_eq!(
        facts_of("mov w0, #-1").op,
        Op::MovImm {
            dst: w(0),
            value: 0xffff_ffff
        }
    );
    assert_eq!(
        facts_of("subs x3, x3, #1").op,
        Op::Decrement {
            counter: x(3),
            flags: true
        }
    );
    assert_eq!(facts_of("tst x0, #0xf").op, Op::Test);
    let copy = facts_of("mov x1, x2");
    assert_eq!(
        (copy.op, copy.value),
        (Op::Move, Value::Copy(Operand::Reg(x(2))))
    );
    assert_eq!(
        facts_of("neg x0, x1").value,
        Value::Sub(Operand::Imm(0), Operand::Reg(x(1)))
    );

    // The frame record: pushed as sp moves down 16, popped as it moves back.
    let push = facts_of("stp x29, x30, [sp, #-16]!");
    assert_eq!(push.op, Op::Push([Some(x(29)), Some(x(30))]));
    assert_eq!(push.stack, Stack::Moves(16));
    assert!(push.writes_memory && push.changes(sp, &ABI) && push.uses(x(30)));
    let pop = facts_of("ldp x29, x30, [sp], #16");
    assert_eq!(pop.op, Op::Pop([Some(x(29)), Some(x(30))]));
    assert_eq!(pop.stack, Stack::Moves(-16));
    assert!(pop.changes(x(30), &ABI) && pop.reads_memory);
    assert_eq!(facts_of("sub sp, sp, #32").stack, Stack::Moves(32));
    assert_eq!(facts_of("add sp, sp, #32").stack, Stack::Moves(-32));
    assert!(matches!(facts_of("mov sp, x0").stack, Stack::Refused(_)));

    // A call changes the caller-saved registers and x30; a system call only x0.
    let call = facts_of("bl label");
    assert_eq!(call.control, Control::Call(0x40_2000));
    assert!(call.changes(x(30), &ABI) && call.changes(x(9), &ABI) && !call.changes(x(19), &ABI));
    let syscall = facts_of("svc #0");
    assert_eq!(syscall.control, Control::Syscall);
    assert!(syscall.uses(x(8)) && syscall.changes(x(0), &ABI) && !syscall.changes(x(1), &ABI));
    assert_eq!(
        facts_of("tbnz x0, #63, label").control,
        Control::Branch {
            target: 0x40_2000,
            cond: Cond::BitSet(63)
        }
    );
    assert_eq!(
        facts_of("cbz x0, label").control,
        Control::Branch {
            target: 0x40_2000,
            cond: Cond::Zero
        }
    );
    assert_eq!(
        facts_of("b.hs label").control,
        Control::Branch {
            target: 0x40_2000,
            cond: Cond::AboveEqual
        }
    );
    let ret = facts_of("ret");
    assert!(ret.control == Control::Return && ret.uses(x(30)));

    // Memory: where each access is, how wide, and what moves.
    let byte = facts_of("ldrb w10, [x0, x2]");
    let memory = byte.memory.expect("a memory operand");
    assert_eq!(
        (memory.base, memory.index, memory.scale, memory.width),
        (Some(x(0)), Some(x(2)), 1, 1)
    );
    assert_eq!((byte.dst, byte.value), (Some(w(10)), Value::Load));
    let word = facts_of("ldr x0, [x1, x3, lsl #3]").memory.expect("memory");
    assert_eq!((word.scale, word.width), (8, 8));
    let argument = facts_of("ldr x0, [sp, #16]").memory.expect("memory");
    assert_eq!((argument.base, argument.displacement), (Some(sp), 16));
    let post = facts_of("str x0, [x1], #8");
    assert_eq!(post.memory.expect("memory").displacement, 0);
    assert!(post.changes(x(1), &ABI) && post.uses(x(0)) && post.dst.is_none());
    assert!(facts_of("adrp x1, label").changes(x(1), &ABI));
}
