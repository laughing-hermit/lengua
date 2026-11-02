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
use super::vector::Expand;

/// Where every word is taken to be: branch targets are relative to it.
const ADDRESS: u64 = 0x40_1000;

/// The random words drawn in each class.
const PER_CLASS: usize = 3000;

/// Each encoding class the dialect has: its name, the bits that place a word in it, and their
/// value. The classes whose fields are mostly unallocated fix more bits, so random words land on
/// instructions.
const CLASSES: [(&str, u32, u32); 79] = [
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
    ("vector three same", 0x9f20_0400, 0x0e20_0400),
    ("vector two-register misc", 0x9f3e_0c00, 0x0e20_0800),
    ("vector across lanes", 0x9f3e_0c00, 0x0e30_0800),
    ("vector copy", 0x9fe0_8400, 0x0e00_0400),
    ("vector permute", 0xbf20_8c00, 0x0e00_0800),
    ("vector extract", 0xbfe0_8400, 0x2e00_0000),
    ("vector table", 0xbfe0_8c00, 0x0e00_0000),
    ("vector modified immediate", 0x9ff8_0400, 0x0f00_0400),
    ("vector shift by immediate", 0x9f80_0400, 0x0f00_0400),
    ("ld1 and st1", 0xbfbf_0000, 0x0c00_0000),
    ("ld1 and st1, moving", 0xbfa0_0000, 0x0c80_0000),
    (
        "q loads and stores, unsigned offset",
        0xff80_0000,
        0x3d80_0000,
    ),
    (
        "q loads and stores, unscaled and moving",
        0xffa0_0000,
        0x3c80_0000,
    ),
    (
        "q loads and stores, register offset",
        0xffa0_0c00,
        0x3ca0_0800,
    ),
    ("q pairs", 0xfe00_0000, 0xac00_0000),
    ("vector fp three same", 0x9f20_c400, 0x0e20_c400),
    ("vector fp two-register misc", 0x9f3f_8c00, 0x0e21_8800),
    ("vector fp compares with zero", 0x9f3f_cc00, 0x0e20_c800),
    ("fcvtl and fcvtn", 0x9fbf_ec00, 0x0e21_6800),
    ("vector fp across lanes", 0xbf7f_cc00, 0x2e30_c800),
    ("fp pairwise into a scalar", 0xff3f_cc00, 0x7e30_c800),
    ("vector fmov", 0x9ff8_fc00, 0x0f00_f400),
    ("ldar", 0xbfff_fc00, 0x88df_fc00),
    ("crc32 and crc32c", 0x7fe0_e000, 0x1ac0_4000),
    ("pmull and pmull2", 0xbf20_fc00, 0x0e20_e000),
    // SVE's: the fields an element size or a pattern takes are left to vary where the dialect
    // has one of them, so most words of a class are in it.
    ("sve ptrue", 0xff3e_fff0, 0x2518_e3e0),
    ("sve whilelo", 0xff20_ec10, 0x2520_0c00),
    ("sve predicate logic", 0xff30_c000, 0x2500_4000),
    ("sve cntp", 0xff3f_c200, 0x2520_8000),
    ("sve ld1b", 0xffe0_e000, 0xa400_4000),
    ("sve st1b", 0xffe0_e000, 0xe400_4000),
    ("sve compares of vectors", 0xff20_4000, 0x2400_0000),
    (
        "sve compares with unsigned numbers",
        0xff20_0000,
        0x2420_0000,
    ),
    ("sve compares with signed numbers", 0xff20_4000, 0x2500_0000),
    ("sve add and sub", 0xff20_e000, 0x0420_0000),
    ("sve bitwise", 0xff20_fc00, 0x0420_3000),
    ("sve cnt", 0xff3f_e000, 0x041a_a000),
    ("sve sel", 0xff20_c000, 0x0520_c000),
    ("sve dup of a number", 0xff3f_c000, 0x2538_c000),
    ("sve dup of a register", 0xff3f_fc00, 0x0520_3800),
    ("sve cpy of a number", 0xff30_8000, 0x0510_0000),
    ("sve uaddv", 0xff3f_e000, 0x0401_2000),
    ("sve incb", 0xff30_fbe0, 0x0430_e3e0),
    ("sve cntb", 0xff30_ffe0, 0x0420_e3e0),
    ("sve insr", 0xff3f_fc00, 0x0524_3800),
    ("sve lastb", 0xff3f_e000, 0x0521_a000),
];

/// What LLVM's disassembler says of each word: its text, or none where it refuses the word or
/// warns that its result isn't defined.
fn llvm(words: &[u32]) -> Option<Vec<Option<String>>> {
    let mut child = Command::new("llvm-mc")
        .args([
            "--disassemble",
            "--triple=aarch64-unknown-linux-gnu",
            // AES brings the doublewords' pmull, which LLVM puts under it.
            "--mattr=+crc,+aes,+sve",
        ])
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

    // Vectors: a V register whole, named by its low double, which is what a call keeps of v8 to
    // v15.
    let d = |number| {
        Reg::A64Fp(super::FpReg {
            number,
            double: true,
        })
    };
    let push = facts_of("stp q8, q9, [sp, #-32]!");
    assert_eq!(
        (push.op, push.stack),
        (Op::Push([Some(d(8)), Some(d(9))]), Stack::Moves(32))
    );
    let select = facts_of("bsl v0.16b, v1.16b, v2.16b");
    assert!(select.uses(d(0)) && select.uses(d(1)) && select.changes(d(0), &ABI));
    // A fused multiply-add's destination is its addend; fcvtn2 keeps the low half.
    let fused = facts_of("fmla v0.2d, v1.2d, v2.2d");
    assert!(fused.uses(d(0)) && fused.uses(d(2)) && fused.changes(d(0), &ABI));
    assert!(!facts_of("fmul v0.2d, v1.2d, v2.2d").uses(d(0)));
    assert!(facts_of("fcvtn2 v0.4s, v1.2d").uses(d(0)));
    assert!(!facts_of("fcvtn v0.2s, v1.2d").uses(d(0)));
    assert!(!facts_of("orr v0.16b, v1.16b, v2.16b").uses(d(0)));
    let loads = facts_of("ld1 {v30.16b-v1.16b}, [x1], #64");
    assert!(
        [30, 31, 0, 1]
            .into_iter()
            .all(|number| loads.changes(d(number), &ABI))
    );
    assert!(loads.changes(x(1), &ABI) && !loads.changes(d(2), &ABI));
    assert_eq!(loads.memory.expect("memory").width, 64);
    let lane = facts_of("umov w3, v1.b[7]");
    assert_eq!(lane.dst, Some(w(3)));
    assert!(lane.uses(d(1)) && lane.changes(x(3), &ABI));
    let insert = facts_of("mov v2.s[1], w4");
    assert!(insert.uses(d(2)) && insert.uses(w(4)) && insert.changes(d(2), &ABI));
    assert_eq!(
        facts_of("ldr q0, [x1, x2, lsl #4]")
            .memory
            .map(|memory| (memory.scale, memory.width)),
        Some((16, 16))
    );
    // A call changes the vectors but v8 to v15's low halves.
    let call = facts_of("bl label");
    assert!(call.changes(d(16), &ABI) && !call.changes(d(8), &ABI));
}

/// Vector lines as main.s may write them, aliases LLVM never prints included: each must read as
/// the word LLVM's assembler makes of it.
const VECTOR_LINES: &[&str] = &[
    "add v0.16b, v1.16b, v2.16b",
    "sub v31.2d, v30.2d, v29.2d",
    "mul v0.8h, v1.8h, v2.8h",
    "uqsub v0.16b, v1.16b, v2.16b",
    "sqadd v0.4s, v1.4s, v2.4s",
    "cmeq v0.16b, v1.16b, v2.16b",
    "cmhi v0.16b, v1.16b, v2.16b",
    "cmle v0.8h, v1.8h, v2.8h",
    "cmlt v0.4s, v1.4s, v2.4s",
    "cmlo v0.16b, v1.16b, v2.16b",
    "cmls v0.2d, v1.2d, v2.2d",
    "umaxp v0.16b, v1.16b, v2.16b",
    "addp v0.2d, v1.2d, v2.2d",
    "and v0.16b, v1.16b, v2.16b",
    "bic v0.8b, v1.8b, v2.8b",
    "orn v0.16b, v1.16b, v2.16b",
    "bsl v0.16b, v1.16b, v2.16b",
    "mov v3.16b, v4.16b",
    "mov v3.8b, v4.8b",
    "not v0.16b, v1.16b",
    "mvn v0.8b, v1.8b",
    "cnt v0.8b, v1.8b",
    "rbit v0.16b, v1.16b",
    "rev32 v0.8h, v1.8h",
    "cmeq v0.16b, v1.16b, #0",
    "cmlt v0.2d, v1.2d, #0",
    "neg v0.2d, v1.2d",
    "uaddlp v0.4s, v1.8h",
    "xtn v0.2s, v1.2d",
    "xtn2 v0.16b, v1.8h",
    "addv b0, v1.8b",
    "uaddlv h0, v1.16b",
    "uaddlv s0, v1.8h",
    "umaxv b0, v1.16b",
    "ushr v0.2d, v1.2d, #64",
    "shl v0.2d, v1.2d, #63",
    "sri v0.16b, v1.16b, #8",
    "sli v0.4s, v1.4s, #0",
    "shrn2 v0.16b, v1.8h, #8",
    "ushll2 v0.8h, v1.16b, #7",
    "uxtl v0.8h, v1.8b",
    "uxtl2 v0.4s, v1.8h",
    "sxtl v0.2d, v1.2s",
    "zip2 v0.2d, v1.2d, v2.2d",
    "ext v0.16b, v1.16b, v2.16b, #15",
    "tbl v0.8b, {v31.16b, v0.16b}, v1.8b",
    "tbx v0.16b, {v1.16b-v4.16b}, v5.16b",
    "dup v0.16b, wzr",
    "dup v0.8h, v1.h[7]",
    "dup v0.2d, x30",
    "umov w0, v1.b[15]",
    "umov w0, v1.s[1]",
    "mov w0, v1.s[3]",
    "mov x0, v1.d[1]",
    "smov x0, v1.h[2]",
    "ins v0.b[3], w1",
    "ins v0.s[2], v1.s[0]",
    "mov v0.d[1], x1",
    "mov v0.h[1], v1.h[7]",
    "movi v0.16b, #32",
    "movi v0.8b, #0xff",
    "movi v0.2d, #0",
    "movi d0, #0xff00ff00ff00ff00",
    "movi v0.4s, #32, msl #8",
    "movi v0.4h, #1, lsl #8",
    "mvni v0.2s, #32, lsl #16",
    "orr v0.8h, #32, lsl #8",
    "bic v0.4s, #1, lsl #24",
    "ldr q0, [x1]",
    "ldr q0, [x1, #65520]",
    "ldr q0, [x1, #-16]",
    "ldur q0, [x1, #-3]",
    "ldr q0, [x1, x2, lsl #4]",
    "str q31, [sp, #-32]!",
    "ldr q0, [x1], #16",
    "ldp q0, q1, [sp, #-1024]",
    "stp q30, q31, [x1, #1008]!",
    "ld1 {v0.16b}, [x1]",
    "ld1 {v0.16b-v3.16b}, [x1], #64",
    "ld1 {v0.16b, v1.16b}, [sp], x2",
    "st1 {v31.1d, v0.1d}, [x1], #16",
    // Floats: singles and doubles, the aliases LLVM accepts with operands swapped included.
    "fadd v0.4s, v1.4s, v2.4s",
    "fadd v0.2d, v1.2d, v2.2d",
    "fadd v0.2s, v1.2s, v2.2s",
    "fsub v3.2d, v4.2d, v5.2d",
    "fmul v0.4s, v1.4s, v31.4s",
    "fdiv v0.2d, v1.2d, v2.2d",
    "fmax v0.4s, v1.4s, v2.4s",
    "fmin v0.2d, v1.2d, v2.2d",
    "fmaxnm v0.4s, v1.4s, v2.4s",
    "fminnm v0.2d, v1.2d, v2.2d",
    "fabd v0.4s, v1.4s, v2.4s",
    "faddp v0.4s, v1.4s, v2.4s",
    "fmaxp v0.2d, v1.2d, v2.2d",
    "fminp v0.2s, v1.2s, v2.2s",
    "fmaxnmp v0.4s, v1.4s, v2.4s",
    "fminnmp v0.2d, v1.2d, v2.2d",
    "fmla v0.2d, v1.2d, v2.2d",
    "fmls v0.4s, v1.4s, v2.4s",
    "fcmeq v0.4s, v1.4s, v2.4s",
    "fcmge v0.2d, v1.2d, v2.2d",
    "fcmgt v0.4s, v1.4s, v2.4s",
    "fcmle v0.4s, v1.4s, v2.4s",
    "fcmlt v0.2d, v1.2d, v2.2d",
    "facge v0.4s, v1.4s, v2.4s",
    "facgt v0.2d, v1.2d, v2.2d",
    "facle v0.4s, v1.4s, v2.4s",
    "faclt v0.2d, v1.2d, v2.2d",
    "fabs v0.4s, v1.4s",
    "fneg v0.2d, v1.2d",
    "fsqrt v0.2s, v1.2s",
    "frintn v0.2d, v1.2d",
    "frinta v0.4s, v1.4s",
    "frintp v0.2d, v1.2d",
    "frintm v0.4s, v1.4s",
    "frintz v0.2d, v1.2d",
    "fcvtzs v0.4s, v1.4s",
    "fcvtzu v0.2d, v1.2d",
    "scvtf v0.4s, v1.4s",
    "ucvtf v0.2d, v1.2d",
    "fcmeq v0.4s, v1.4s, #0.0",
    "fcmge v0.2d, v1.2d, #0.0",
    "fcmgt v0.4s, v1.4s, #0.0",
    "fcmle v0.2d, v1.2d, #0.0",
    "fcmlt v0.2s, v1.2s, #0.0",
    "fcvtl v0.2d, v1.2s",
    "fcvtl2 v0.2d, v1.4s",
    "fcvtn v0.2s, v1.2d",
    "fcvtn2 v0.4s, v1.2d",
    "fmaxv s0, v1.4s",
    "fminv s0, v1.4s",
    "fmaxnmv s0, v1.4s",
    "fminnmv s0, v1.4s",
    "faddp s0, v1.2s",
    "faddp d0, v1.2d",
    "fmaxp d0, v1.2d",
    "fminp s0, v1.2s",
    "fmaxnmp d0, v1.2d",
    "fminnmp s0, v1.2s",
    "fmov v0.4s, #1.0",
    "fmov v0.2s, #-2.5",
    "fmov v0.2d, #0.125",
    "pmull v0.8h, v1.8b, v2.8b",
    "pmull2 v31.8h, v30.16b, v29.16b",
    "pmull v0.1q, v1.1d, v2.1d",
    "pmull2 v7.1q, v8.2d, v9.2d",
    "ptrue p0.b",
    "whilelo p1.b, x10, x2",
    "whilelo p2.b, xzr, x3",
    "and p0.b, p1/z, p2.b, p3.b",
    "bic p4.b, p5/z, p6.b, p7.b",
    "eor p8.b, p9/z, p10.b, p11.b",
    "orr p12.b, p13/z, p14.b, p15.b",
    "not p0.b, p1/z, p2.b",
    "mov p0.b, p1/z, p2.b",
    "mov p3.b, p4.b",
    "cntp x5, p1, p2.b",
    "ld1b { z0.b }, p0/z, [x1, x10]",
    "st1b { z31.b }, p7, [x2, x3]",
    "cmpeq p1.b, p0/z, z0.b, z1.b",
    "cmpne p1.b, p0/z, z0.b, #10",
    "cmphs p1.b, p0/z, z0.b, #9",
    "cmplo p2.b, p0/z, z0.b, #14",
    "cmphi p3.b, p1/z, z4.b, z5.b",
    "cmplo p3.b, p1/z, z4.b, z5.b",
    "cmple p3.b, p1/z, z4.b, z5.b",
    "cmpge p1.b, p0/z, z0.b, #-16",
    "cmplt p1.b, p0/z, z0.b, #15",
    "add z0.b, z1.b, z2.b",
    "sub z3.b, z4.b, z5.b",
    "and z0.d, z1.d, z2.d",
    "bic z0.d, z1.d, z2.d",
    "mov z6.d, z7.d",
    "cnt z0.b, p0/m, z1.b",
    "sel z0.b, p8, z1.b, z2.b",
    "mov z0.b, p1/m, z3.b",
    "mov z0.b, #10",
    "dup z1.b, #-1",
    "mov z2.b, w3",
    "dup z2.b, w4",
    "mov z0.b, p0/z, #1",
    "mov z0.b, p0/m, #-128",
    "cpy z0.b, p0/z, #1",
    "uaddv d0, p0, z1.b",
    "incb x10",
    "incb x10, all, mul #4",
    "cntb x0",
    "cntb x0, all, mul #16",
    "insr z0.b, w1",
    "insr z0.b, wzr",
    "lastb w0, p0, z1.b",
];

#[test]
fn a_vector_fmov_holds_the_float_its_bits_say() {
    for imm8 in 0..=255_u8 {
        let number = super::fp_immediate(imm8);
        let single = Expand::Float.value(imm8, 32);
        let double = Expand::Float.value(imm8, 64);
        #[allow(clippy::cast_possible_truncation)] // Exact: every such number is a single.
        let expected_single = u64::from((number as f32).to_bits());
        assert_eq!(single, expected_single, "{imm8:#04x} as a single");
        assert_eq!(double, number.to_bits(), "{imm8:#04x} as a double");
    }
}

/// The words `llvm-mc` assembles `lines` into, one each, or none where it isn't installed.
fn assembled(lines: &[&str]) -> Option<Vec<u32>> {
    let mut child = Command::new("llvm-mc")
        .args([
            "--triple=aarch64-unknown-linux-gnu",
            "--mattr=+aes,+sve",
            "--show-encoding",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let input = lines.join("\n") + "\n";
    let mut stdin = child.stdin.take()?;
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child.wait_with_output().ok()?;
    writer.join().ok()?.ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.trim().is_empty(), "llvm-mc refused a line: {stderr}");
    let words = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once("encoding: [").map(|(_, bytes)| bytes))
        .map(|bytes| {
            let bytes: Vec<u8> = bytes
                .trim_end_matches(']')
                .split(',')
                .map(|byte| u8::from_str_radix(byte.trim().trim_start_matches("0x"), 16))
                .collect::<Result<_, _>>()
                .expect("an encoding's bytes");
            u32::from_le_bytes(bytes.try_into().expect("four bytes"))
        })
        .collect();
    Some(words)
}

#[test]
fn vector_lines_read_as_the_words_they_assemble_to() {
    let Some(words) = assembled(VECTOR_LINES) else {
        eprintln!("llvm-mc isn't installed: not checked");
        return;
    };
    assert_eq!(words.len(), VECTOR_LINES.len());
    let mut problems = Vec::new();
    for (line, word) in VECTOR_LINES.iter().zip(&words) {
        let (mnemonic, operands) = line.split_once(' ').unwrap_or((line, ""));
        let parsed = parse(mnemonic, operands, ADDRESS, &|_| None);
        let decoded = decode(*word, ADDRESS);
        match (&parsed, &decoded) {
            (Ok(parsed), Ok(decoded)) if parsed == decoded => {
                // And the canonical form reads back as itself.
                let text = decoded.to_string();
                let (mnemonic, operands) = text.split_once(' ').unwrap_or((&text, ""));
                if parse(mnemonic, operands, ADDRESS, &|_| None).as_ref() != Ok(decoded) {
                    problems.push(format!(
                        "`{line}`: its canonical form `{text}` reads otherwise"
                    ));
                }
            }
            _ => problems.push(format!(
                "`{line}` ({word:#010x}): reads {parsed:?}, decodes {decoded:?}"
            )),
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
