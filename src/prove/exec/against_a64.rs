//! Random-value tests of the prover's model of AArch64's instructions against an AArch64 CPU: this
//! computer's own, or QEMU's, which the kernel hands AArch64 programs to on an x86-64 one. One
//! harness program holds every probe, a few instructions each; it runs them all on many random
//! register, flag and memory values and prints where they ended. The model must end the same
//! every time: AArch64 leaves nothing these instructions do undefined.
//!
//! The harness's own code sets the flags with `msr nzcv`, moves whole V registers with `ldp q`
//! and `stp q`, and jumps into each probe through a register, which the dialect doesn't have;
//! only the probes, all in the dialect, run in the model. Floating-point probes start from a mix
//! of NaNs of both kinds, zeros of both signs, infinities, subnormals, ties and the limits of
//! each conversion, and the whole of each V register is compared, so a scalar write must clear
//! what's above it.

mod sve;

use std::io::Write as _;
use std::process::{Command, Stdio};

use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

use super::{Image, Last, State, Step, step};
use crate::checker::decode::Machine;
use crate::isa::aarch64::fp_immediate;
use crate::platform::Profile;
use crate::prove::term::Pool;
use crate::toolchain::Toolchain;

/// The registers a probe works with, x0 to x15; x15 holds the memory cell's address in a probe
/// that reaches memory.
const REGISTERS: usize = 16;

/// The V registers a probe works with, v0 to v7, 16 bytes each.
const FP_REGISTERS: usize = 8;

/// The memory cell's words: 64 bytes, as much as `ld1` of four registers moves.
const CELL_WORDS: usize = 8;

/// Where the V registers are in a vector and a result: after the registers, NZCV, the cell's
/// words and the probe's address (a result's last word unused), at a multiple of 16, as `ldp q`
/// and `stp q` reach.
const V_AT: usize = (REGISTERS + CELL_WORDS + 2) * 8;

/// Bytes per vector: the registers, NZCV, the cell's words, the probe's address and the V
/// registers.
const VECTOR: usize = V_AT + FP_REGISTERS * 16;

/// Bytes per result: the registers, NZCV, the cell's words, a word unused and the V registers.
const RESULT: usize = V_AT + FP_REGISTERS * 16;

/// The conditions, as `b.cond` and `csel` name them.
const CONDITIONS: [&str; 16] = [
    "eq", "ne", "hs", "lo", "mi", "pl", "vs", "vc", "hi", "ls", "ge", "lt", "gt", "le", "al", "nv",
];

/// The flags, by their bit in NZCV.
const FLAGS: [(u64, &str); 4] = [
    (1 << 31, "N"),
    (1 << 30, "Z"),
    (1 << 29, "C"),
    (1 << 28, "V"),
];

/// A few instructions, and whether they reach the memory cell through x15.
struct Probe {
    code: Vec<String>,
    memory: bool,
}

fn probe(code: &[String]) -> Probe {
    Probe {
        code: code.to_vec(),
        memory: false,
    }
}

fn memory(code: &[String]) -> Probe {
    Probe {
        code: code.to_vec(),
        memory: true,
    }
}

/// A register a probe works with: x0 to x14, `x` or `w`.
fn register(rng: &mut ChaCha8Rng, wide: bool) -> String {
    let number = rng.random_range(0..15);
    format!("{}{number}", if wide { "x" } else { "w" })
}

/// An s or d register a probe works with, s0 to s7 or d0 to d7.
fn fp_register(rng: &mut ChaCha8Rng, double: bool) -> String {
    let number = rng.random_range(0..FP_REGISTERS);
    format!("{}{number}", if double { "d" } else { "s" })
}

/// The floating-point probes, of singles and of doubles.
#[allow(clippy::too_many_lines)]
fn fp_probes(rng: &mut ChaCha8Rng, list: &mut Vec<Probe>) {
    for double in [true, false] {
        let f = |rng: &mut ChaCha8Rng| fp_register(rng, double);
        for op in [
            "fadd", "fsub", "fmul", "fdiv", "fmax", "fmin", "fmaxnm", "fminnm", "fnmul",
        ] {
            let (d, n, m) = (f(rng), f(rng), f(rng));
            list.push(probe(&[format!("{op} {d}, {n}, {m}")]));
        }
        for op in ["fmadd", "fmsub", "fnmadd", "fnmsub"] {
            let (d, n, m, a) = (f(rng), f(rng), f(rng), f(rng));
            list.push(probe(&[format!("{op} {d}, {n}, {m}, {a}")]));
        }
        for op in [
            "fmov", "fabs", "fneg", "fsqrt", "frintn", "frintp", "frintm", "frintz", "frinta",
        ] {
            let (d, n) = (f(rng), f(rng));
            list.push(probe(&[format!("{op} {d}, {n}")]));
        }
        let (d, n) = (fp_register(rng, !double), f(rng));
        list.push(probe(&[format!("fcvt {d}, {n}")]));
        for op in ["fcmp", "fcmpe"] {
            let (n, m) = (f(rng), f(rng));
            list.push(probe(&[format!("{op} {n}, {m}")]));
            let n = f(rng);
            list.push(probe(&[format!("{op} {n}, #0.0")]));
        }
        for condition in &CONDITIONS[..14] {
            let (d, n, m) = (f(rng), f(rng), f(rng));
            list.push(probe(&[format!("fcsel {d}, {n}, {m}, {condition}")]));
        }
        let number = fp_immediate(rng.random());
        let d = f(rng);
        list.push(probe(&[format!("fmov {d}, #{number:?}")]));
        // The bits to and from a general register as wide.
        let (g, n) = (register(rng, double), f(rng));
        list.push(probe(&[format!("fmov {g}, {n}")]));
        let (d, g) = (f(rng), register(rng, double));
        list.push(probe(&[format!("fmov {d}, {g}")]));
        for wide in [true, false] {
            for op in ["scvtf", "ucvtf"] {
                let (d, g) = (f(rng), register(rng, wide));
                list.push(probe(&[format!("{op} {d}, {g}")]));
            }
            for op in ["fcvtzs", "fcvtzu"] {
                let (g, n) = (register(rng, wide), f(rng));
                list.push(probe(&[format!("{op} {g}, {n}")]));
            }
        }
        // Loads and stores in the cell at x15, the pairs filling it or half of it.
        let size = if double { 8 } else { 4 };
        let shift = if double { 3 } else { 2 };
        list.push(memory(&[format!("str {}, [x15]", f(rng))]));
        list.push(memory(&[format!("str {}, [x15, #{size}]", f(rng))]));
        list.push(memory(&[format!("ldr {}, [x15]", f(rng))]));
        list.push(memory(&[format!("ldr {}, [x15, #{size}]", f(rng))]));
        list.push(memory(&[format!("str {}, [x15, #{size}]!", f(rng))]));
        list.push(memory(&[format!("ldr {}, [x15], #{size}", f(rng))]));
        list.push(memory(&[
            "mov x14, #1".to_owned(),
            format!("ldr {}, [x15, x14, lsl #{shift}]", f(rng)),
        ]));
        list.push(memory(&[format!("stp {}, {}, [x15]", f(rng), f(rng))]));
        // Two registers loaded at once must differ: the same one twice is unpredictable.
        let first = rng.random_range(0..FP_REGISTERS);
        let second = (first + rng.random_range(1..FP_REGISTERS)) % FP_REGISTERS;
        let kind = if double { "d" } else { "s" };
        list.push(memory(&[format!(
            "ldp {kind}{first}, {kind}{second}, [x15]"
        )]));
    }
    // A comparison's flags read by a branch-free choice, and whole numbers through doubles and
    // back.
    list.push(probe(&[
        "fcmp d1, d2".to_owned(),
        "cset w0, mi".to_owned(),
        "cset w1, lt".to_owned(),
        "cset w2, vs".to_owned(),
    ]));
    list.push(probe(&[
        "scvtf d0, x1".to_owned(),
        "fcvtzs x2, d0".to_owned(),
        "ucvtf s1, w3".to_owned(),
        "fcvtzu w4, s1".to_owned(),
    ]));
}

/// A vector register a probe works with, v0 to v7, as `arrangement` names its lanes.
fn v_register(rng: &mut ChaCha8Rng, arrangement: &str) -> String {
    format!("v{}.{arrangement}", rng.random_range(0..FP_REGISTERS))
}

/// The probes of floats on vectors: singles two or four at a time and doubles two at a time,
/// each instruction in every arrangement it takes.
fn vector_float_probes(rng: &mut ChaCha8Rng, list: &mut Vec<Probe>) {
    for arrangement in ["2s", "4s", "2d"] {
        let v = |rng: &mut ChaCha8Rng| v_register(rng, arrangement);
        for op in [
            "fadd", "fsub", "fmul", "fdiv", "fmax", "fmin", "fmaxnm", "fminnm", "fabd", "faddp",
            "fmaxp", "fminp", "fmaxnmp", "fminnmp", "fmla", "fmls", "fcmeq", "fcmge", "fcmgt",
            "facge", "facgt",
        ] {
            let (d, n, m) = (v(rng), v(rng), v(rng));
            list.push(probe(&[format!("{op} {d}, {n}, {m}")]));
        }
        for op in [
            "fabs", "fneg", "fsqrt", "frintn", "frinta", "frintp", "frintm", "frintz", "fcvtzs",
            "fcvtzu", "scvtf", "ucvtf",
        ] {
            let (d, n) = (v(rng), v(rng));
            list.push(probe(&[format!("{op} {d}, {n}")]));
        }
        for op in ["fcmeq", "fcmge", "fcmgt", "fcmle", "fcmlt"] {
            let (d, n) = (v(rng), v(rng));
            list.push(probe(&[format!("{op} {d}, {n}, #0.0")]));
        }
        let number = fp_immediate(rng.random());
        let d = v(rng);
        list.push(probe(&[format!("fmov {d}, #{number:?}")]));
    }
    // Between singles and doubles, each half.
    for (op, to, from) in [
        ("fcvtl", "2d", "2s"),
        ("fcvtl2", "2d", "4s"),
        ("fcvtn", "2s", "2d"),
        ("fcvtn2", "4s", "2d"),
    ] {
        let (d, n) = (v_register(rng, to), v_register(rng, from));
        list.push(probe(&[format!("{op} {d}, {n}")]));
    }
    // Four singles into one, and two floats into a scalar.
    for op in ["fmaxv", "fminv", "fmaxnmv", "fminnmv"] {
        let (d, n) = (rng.random_range(0..FP_REGISTERS), v_register(rng, "4s"));
        list.push(probe(&[format!("{op} s{d}, {n}")]));
    }
    for op in ["faddp", "fmaxp", "fminp", "fmaxnmp", "fminnmp"] {
        let (d, n) = (rng.random_range(0..FP_REGISTERS), v_register(rng, "2s"));
        list.push(probe(&[format!("{op} s{d}, {n}")]));
        let (d, n) = (rng.random_range(0..FP_REGISTERS), v_register(rng, "2d"));
        list.push(probe(&[format!("{op} d{d}, {n}")]));
    }
}

/// The vector probes, over every arrangement each instruction takes.
#[allow(clippy::too_many_lines)]
fn vector_probes(rng: &mut ChaCha8Rng, list: &mut Vec<Probe>) {
    // Arrangement, lane bits, and the name of a lane of that size.
    const ARRANGEMENTS: [(&str, u32, &str); 7] = [
        ("8b", 8, "b"),
        ("16b", 8, "b"),
        ("4h", 16, "h"),
        ("8h", 16, "h"),
        ("2s", 32, "s"),
        ("4s", 32, "s"),
        ("2d", 64, "d"),
    ];
    for (arrangement, bits, lane) in ARRANGEMENTS {
        let v = |rng: &mut ChaCha8Rng| v_register(rng, arrangement);
        let mut three = vec![
            "add", "sub", "cmeq", "cmtst", "cmhi", "cmhs", "cmgt", "cmge", "uqadd", "uqsub",
            "sqadd", "sqsub", "addp", "zip1", "zip2", "uzp1", "uzp2", "trn1", "trn2",
        ];
        if bits < 64 {
            three.extend([
                "mul", "umax", "umin", "smax", "smin", "umaxp", "uminp", "smaxp", "sminp",
            ]);
        }
        for op in three {
            let (d, n, m) = (v(rng), v(rng), v(rng));
            list.push(probe(&[format!("{op} {d}, {n}, {m}")]));
        }
        for op in ["cmeq", "cmgt", "cmge", "cmle", "cmlt"] {
            let (d, n) = (v(rng), v(rng));
            list.push(probe(&[format!("{op} {d}, {n}, #0")]));
        }
        let mut two = vec!["abs", "neg"];
        if bits < 64 {
            two.extend(["cls", "clz", "rev64"]);
        }
        if bits <= 16 {
            two.push("rev32");
        }
        for op in two {
            let (d, n) = (v(rng), v(rng));
            list.push(probe(&[format!("{op} {d}, {n}")]));
        }
        // Shifts by every distance a lane takes, the ends included.
        for op in ["ushr", "sshr", "usra", "ssra", "sri"] {
            let (d, n) = (v(rng), v(rng));
            let amount = rng.random_range(1..=bits);
            list.push(probe(&[format!("{op} {d}, {n}, #{amount}")]));
        }
        for op in ["shl", "sli"] {
            let (d, n) = (v(rng), v(rng));
            let amount = rng.random_range(0..bits);
            list.push(probe(&[format!("{op} {d}, {n}, #{amount}")]));
        }
        // A lane, or a general register, in every lane.
        let lanes = 128 / bits;
        let (d, n) = (v(rng), rng.random_range(0..FP_REGISTERS));
        let index = rng.random_range(0..lanes);
        list.push(probe(&[format!("dup {d}, v{n}.{lane}[{index}]")]));
        let general = register(rng, bits == 64);
        let d = v(rng);
        list.push(probe(&[format!("dup {d}, {general}")]));
        // Lanes into general registers, and general registers and lanes into lanes.
        let (n, index) = (
            rng.random_range(0..FP_REGISTERS),
            rng.random_range(0..lanes),
        );
        let into = register(rng, bits == 64);
        list.push(probe(&[format!("umov {into}, v{n}.{lane}[{index}]")]));
        if bits < 64 {
            let wide = bits == 32 || rng.random();
            let into = register(rng, wide);
            list.push(probe(&[format!("smov {into}, v{n}.{lane}[{index}]")]));
        }
        let (d, index) = (
            rng.random_range(0..FP_REGISTERS),
            rng.random_range(0..lanes),
        );
        let from = register(rng, bits == 64);
        list.push(probe(&[format!("mov v{d}.{lane}[{index}], {from}")]));
        let (n, from_index) = (
            rng.random_range(0..FP_REGISTERS),
            rng.random_range(0..lanes),
        );
        list.push(probe(&[format!(
            "mov v{d}.{lane}[{index}], v{n}.{lane}[{from_index}]"
        )]));
    }
    // The bitwise operations, on bytes.
    for arrangement in ["8b", "16b"] {
        let v = |rng: &mut ChaCha8Rng| v_register(rng, arrangement);
        for op in ["and", "bic", "orr", "orn", "eor", "bsl", "bit", "bif"] {
            let (d, n, m) = (v(rng), v(rng), v(rng));
            list.push(probe(&[format!("{op} {d}, {n}, {m}")]));
        }
        for op in ["cnt", "not", "rbit", "rev16", "mov"] {
            let (d, n) = (v(rng), v(rng));
            list.push(probe(&[format!("{op} {d}, {n}")]));
        }
        let (d, n, m) = (v(rng), v(rng), v(rng));
        let length = if arrangement == "8b" { 8 } else { 16 };
        list.push(probe(&[format!(
            "ext {d}, {n}, {m}, #{}",
            rng.random_range(0..length)
        )]));
        // Tables of 1 to 4 registers, indexes from a register of small numbers so some land
        // inside them.
        // A list's registers follow each other, kept within v0 to v7.
        for (count, op) in [(1, "tbl"), (2, "tbx"), (3, "tbl"), (4, "tbx")] {
            let first = rng.random_range(0..=FP_REGISTERS - count);
            let table: Vec<String> = (0..count)
                .map(|at| format!("v{}.16b", first + at))
                .collect();
            let (d, m) = (v(rng), rng.random_range(0..FP_REGISTERS));
            list.push(probe(&[
                format!("movi v{m}.16b, #{}", rng.random_range(0..80_u32)),
                format!("{op} {d}, {{ {} }}, v{m}.{arrangement}", table.join(", ")),
            ]));
            let (d, m) = (v(rng), v(rng));
            list.push(probe(&[format!(
                "{op} {d}, {{ {} }}, {m}",
                table.join(", ")
            )]));
        }
    }
    // Widening and narrowing: (narrow, wide, lane bits of the narrow).
    for (narrow, wide, bits) in [
        ("8b", "8h", 8),
        ("16b", "8h", 8),
        ("4h", "4s", 16),
        ("8h", "4s", 16),
        ("2s", "2d", 32),
        ("4s", "2d", 32),
    ] {
        let high = narrow == "16b" || narrow == "8h" || narrow == "4s";
        let two = if high { "2" } else { "" };
        let (n, m) = (
            rng.random_range(0..FP_REGISTERS),
            rng.random_range(0..FP_REGISTERS),
        );
        list.push(probe(&[format!("xtn{two} v{n}.{narrow}, v{m}.{wide}")]));
        let amount = rng.random_range(1..=bits);
        list.push(probe(&[format!(
            "shrn{two} v{n}.{narrow}, v{m}.{wide}, #{amount}"
        )]));
        for op in ["ushll", "sshll"] {
            let amount = rng.random_range(0..bits);
            list.push(probe(&[format!(
                "{op}{two} v{m}.{wide}, v{n}.{narrow}, #{amount}"
            )]));
        }
        // Pairwise sums into lanes twice as wide.
        let pairs = match narrow {
            "8b" => "4h",
            "16b" => "8h",
            "4h" => "2s",
            "8h" => "4s",
            "2s" => "1d",
            _ => "2d",
        };
        for op in ["uaddlp", "saddlp"] {
            list.push(probe(&[format!("{op} v{m}.{pairs}, v{n}.{narrow}")]));
        }
    }
    // Across lanes: (arrangement, scalar, long scalar).
    for (arrangement, scalar, long) in [
        ("8b", "b", "h"),
        ("16b", "b", "h"),
        ("4h", "h", "s"),
        ("8h", "h", "s"),
        ("4s", "s", "d"),
    ] {
        let (d, n) = (
            rng.random_range(0..FP_REGISTERS),
            rng.random_range(0..FP_REGISTERS),
        );
        for op in ["addv", "umaxv", "uminv", "smaxv", "sminv"] {
            list.push(probe(&[format!("{op} {scalar}{d}, v{n}.{arrangement}")]));
        }
        for op in ["uaddlv", "saddlv"] {
            list.push(probe(&[format!("{op} {long}{d}, v{n}.{arrangement}")]));
        }
    }
    // Numbers into every lane, in each of their forms.
    let imm = |rng: &mut ChaCha8Rng| rng.random::<u8>();
    let d = |rng: &mut ChaCha8Rng| rng.random_range(0..FP_REGISTERS);
    for arrangement in ["8b", "16b"] {
        list.push(probe(&[format!(
            "movi v{}.{arrangement}, #{}",
            d(rng),
            imm(rng)
        )]));
    }
    for arrangement in ["4h", "8h"] {
        for shift in [0, 8] {
            for op in ["movi", "mvni", "orr", "bic"] {
                list.push(probe(&[format!(
                    "{op} v{}.{arrangement}, #{}, lsl #{shift}",
                    d(rng),
                    imm(rng)
                )]));
            }
        }
    }
    for arrangement in ["2s", "4s"] {
        for shift in [0, 8, 16, 24] {
            for op in ["movi", "mvni", "orr", "bic"] {
                list.push(probe(&[format!(
                    "{op} v{}.{arrangement}, #{}, lsl #{shift}",
                    d(rng),
                    imm(rng)
                )]));
            }
        }
        for shift in [8, 16] {
            for op in ["movi", "mvni"] {
                list.push(probe(&[format!(
                    "{op} v{}.{arrangement}, #{}, msl #{shift}",
                    d(rng),
                    imm(rng)
                )]));
            }
        }
    }
    let mask: u64 = (0..8)
        .map(|byte| if rng.random() { 0xff << (8 * byte) } else { 0 })
        .sum();
    list.push(probe(&[format!("movi v{}.2d, #{mask:#x}", d(rng))]));
    list.push(probe(&[format!("movi d{}, #{mask:#x}", d(rng))]));
    // Loads and stores of whole registers in the cell at x15, 64 bytes.
    let q = |rng: &mut ChaCha8Rng| rng.random_range(0..FP_REGISTERS);
    list.push(memory(&[format!("ldr q{}, [x15]", q(rng))]));
    list.push(memory(&[format!("str q{}, [x15, #48]", q(rng))]));
    list.push(memory(&[format!("ldur q{}, [x15, #7]", q(rng))]));
    list.push(memory(&[format!("str q{}, [x15, #16]!", q(rng))]));
    list.push(memory(&[format!("ldr q{}, [x15], #32", q(rng))]));
    list.push(memory(&[
        "mov x14, #2".to_owned(),
        format!("ldr q{}, [x15, x14, lsl #4]", q(rng)),
    ]));
    let first = q(rng);
    let second = (first + rng.random_range(1..FP_REGISTERS)) % FP_REGISTERS;
    list.push(memory(&[format!("ldp q{first}, q{second}, [x15, #32]")]));
    list.push(memory(&[format!("stp q{}, q{}, [x15]", q(rng), q(rng))]));
    for (count, arrangement) in [(1, "16b"), (2, "8b"), (3, "4s"), (4, "16b"), (4, "1d")] {
        let first = rng.random_range(0..=FP_REGISTERS - count);
        let list_text: Vec<String> = (0..count)
            .map(|at| format!("v{}.{arrangement}", first + at))
            .collect();
        let bytes = count
            * if arrangement == "16b" || arrangement == "4s" {
                16
            } else {
                8
            };
        for op in ["ld1", "st1"] {
            list.push(memory(&[format!(
                "{op} {{ {} }}, [x15]",
                list_text.join(", ")
            )]));
            list.push(memory(&[format!(
                "{op} {{ {} }}, [x15], #{bytes}",
                list_text.join(", ")
            )]));
        }
        list.push(memory(&[
            "mov x14, #5".to_owned(),
            format!("ld1 {{ {} }}, [x15], x14", list_text.join(", ")),
        ]));
    }
    // A comparison's lanes counted, as a counting loop does it.
    list.push(probe(&[
        "cmeq v0.16b, v1.16b, v2.16b".to_owned(),
        "ushr v0.16b, v0.16b, #7".to_owned(),
        "addv b3, v0.16b".to_owned(),
        "umov w4, v3.b[0]".to_owned(),
    ]));
    list.push(probe(&[
        "cmeq v0.16b, v1.16b, v2.16b".to_owned(),
        "shrn v0.8b, v0.8h, #4".to_owned(),
        "fmov x5, d0".to_owned(),
        "rbit x5, x5".to_owned(),
        "clz x5, x5".to_owned(),
    ]));
}

/// Every probe, its registers and numbers drawn from `rng`.
#[allow(clippy::too_many_lines)]
fn probes(rng: &mut ChaCha8Rng) -> Vec<Probe> {
    let mut list = Vec::new();
    // A load-acquire of a word and of a doubleword from the cell.
    for wide in [true, false] {
        let rt = register(rng, wide);
        list.push(memory(&[format!("ldar {rt}, [x15]")]));
    }
    // The CRC-32 steps of each size, in both polynomials.
    for op in [
        "crc32b", "crc32h", "crc32w", "crc32x", "crc32cb", "crc32ch", "crc32cw", "crc32cx",
    ] {
        let (d, n) = (register(rng, false), register(rng, false));
        let m = register(rng, op.ends_with('x'));
        list.push(probe(&[format!("{op} {d}, {n}, {m}")]));
    }
    // The carry-less multiplies: bytes to halfwords, and doublewords to a quadword, of the lower
    // half and of the upper.
    for (op, from, to) in [
        ("pmull", "8b", "8h"),
        ("pmull2", "16b", "8h"),
        ("pmull", "1d", "1q"),
        ("pmull2", "2d", "1q"),
    ] {
        for _ in 0..4 {
            let (d, n, m) = (
                rng.random_range(0..FP_REGISTERS),
                v_register(rng, from),
                v_register(rng, from),
            );
            list.push(probe(&[format!("{op} v{d}.{to}, {n}, {m}")]));
        }
    }
    // AES's and SHA's instructions, of whole vectors.
    let v = |rng: &mut ChaCha8Rng| rng.random_range(0..FP_REGISTERS);
    for _ in 0..3 {
        for op in ["aese", "aesd", "aesmc", "aesimc"] {
            let (d, n) = (v(rng), v(rng));
            list.push(probe(&[format!("{op} v{d}.16b, v{n}.16b")]));
        }
        for op in ["sha1c", "sha1p", "sha1m"] {
            let (d, n, m) = (v(rng), v(rng), v(rng));
            list.push(probe(&[format!("{op} q{d}, s{n}, v{m}.4s")]));
        }
        let (d, n) = (v(rng), v(rng));
        list.push(probe(&[format!("sha1h s{d}, s{n}")]));
        for op in ["sha1su0", "sha256su1"] {
            let (d, n, m) = (v(rng), v(rng), v(rng));
            list.push(probe(&[format!("{op} v{d}.4s, v{n}.4s, v{m}.4s")]));
        }
        for op in ["sha1su1", "sha256su0"] {
            let (d, n) = (v(rng), v(rng));
            list.push(probe(&[format!("{op} v{d}.4s, v{n}.4s")]));
        }
        for op in ["sha256h", "sha256h2"] {
            let (d, n, m) = (v(rng), v(rng), v(rng));
            list.push(probe(&[format!("{op} q{d}, q{n}, v{m}.4s")]));
        }
    }
    fp_probes(rng, &mut list);
    vector_probes(rng, &mut list);
    vector_float_probes(rng, &mut list);
    for wide in [true, false] {
        let width = if wide { 64 } else { 32 };
        let r = |rng: &mut ChaCha8Rng| register(rng, wide);
        // Arithmetic, with numbers and with shifted registers, with and without the flags.
        for op in ["add", "adds", "sub", "subs"] {
            let (d, n, m) = (r(rng), r(rng), r(rng));
            list.push(probe(&[format!(
                "{op} {d}, {n}, #{}",
                rng.random_range(0..4096)
            )]));
            list.push(probe(&[format!(
                "{op} {d}, {n}, #{}, lsl #12",
                rng.random_range(0..4096)
            )]));
            for shift in ["lsl", "lsr", "asr"] {
                list.push(probe(&[format!(
                    "{op} {d}, {n}, {m}, {shift} #{}",
                    rng.random_range(0..width)
                )]));
            }
            list.push(probe(&[format!("{op} {d}, {n}, {m}")]));
        }
        // Logic, with bitmasks and with shifted registers.
        // Bitmasks each width can hold: a run of ones, repeated across the register.
        let masks = if wide {
            ["0xff", "0xf0f0f0f0f0f0f0f0", "0x3", "0xffff0000ffff0000"]
        } else {
            ["0xff", "0xf0f0f0f0", "0x3", "0xffff0000"]
        };
        for (op, mask) in ["and", "orr", "eor", "ands"].into_iter().zip(masks) {
            let (d, n) = (r(rng), r(rng));
            list.push(probe(&[format!("{op} {d}, {n}, #{mask}")]));
        }
        for op in ["and", "orr", "eor", "ands", "bic", "orn", "eon", "bics"] {
            let (d, n, m) = (r(rng), r(rng), r(rng));
            for shift in ["lsl", "lsr", "asr", "ror"] {
                list.push(probe(&[format!(
                    "{op} {d}, {n}, {m}, {shift} #{}",
                    rng.random_range(0..width)
                )]));
            }
        }
        // Moves of numbers, the pieces of wider ones, and of registers.
        let (d, m) = (r(rng), r(rng));
        list.push(probe(&[format!("mov {d}, {m}")]));
        list.push(probe(&[format!("mvn {d}, {m}")]));
        list.push(probe(&[format!("neg {d}, {m}")]));
        list.push(probe(&[format!(
            "movz {d}, #{}, lsl #16",
            rng.random_range(0..65536)
        )]));
        list.push(probe(&[format!(
            "movn {d}, #{}",
            rng.random_range(0..65536)
        )]));
        list.push(probe(&[format!(
            "movk {d}, #{}, lsl #16",
            rng.random_range(0..65536)
        )]));
        // Bitfields: the shifts by numbers, extracts, inserts and extensions.
        for shift in ["lsl", "lsr", "asr", "ror"] {
            let (d, n) = (r(rng), r(rng));
            list.push(probe(&[format!(
                "{shift} {d}, {n}, #{}",
                rng.random_range(1..width)
            )]));
        }
        let lsb = rng.random_range(0..width - 1);
        let field = rng.random_range(1..=width - lsb);
        for op in ["ubfx", "sbfx", "bfi", "bfxil", "ubfiz", "sbfiz"] {
            let (d, n) = (r(rng), r(rng));
            list.push(probe(&[format!("{op} {d}, {n}, #{lsb}, #{field}")]));
        }
        // uxtb and uxth have only a `w` destination.
        for op in ["sxtb", "sxth", "uxtb", "uxth"] {
            if wide && op.starts_with('u') {
                continue;
            }
            let (d, n) = (r(rng), register(rng, false));
            list.push(probe(&[format!("{op} {d}, {n}")]));
        }
        let (d, n, m) = (r(rng), r(rng), r(rng));
        list.push(probe(&[format!(
            "extr {d}, {n}, {m}, #{}",
            rng.random_range(0..width)
        )]));
        // Division, by zero too, and the shifts by registers.
        for op in ["udiv", "sdiv", "lsl", "lsr", "asr", "ror"] {
            let (d, n, m) = (r(rng), r(rng), r(rng));
            list.push(probe(&[format!("{op} {d}, {n}, {m}")]));
        }
        for op in ["rbit", "rev16", "rev", "clz", "cls"] {
            let (d, n) = (r(rng), r(rng));
            list.push(probe(&[format!("{op} {d}, {n}")]));
        }
        // The multiplies.
        for op in ["madd", "msub"] {
            let (d, n, m, a) = (r(rng), r(rng), r(rng), r(rng));
            list.push(probe(&[format!("{op} {d}, {n}, {m}, {a}")]));
        }
        let (d, n, m) = (r(rng), r(rng), r(rng));
        list.push(probe(&[format!("mul {d}, {n}, {m}")]));
        // A remainder: a quotient and its divisor taken from the dividend with `msub`, which the
        // model gives as the remainder itself where the divisor is a constant past 1.
        for division in ["udiv", "sdiv"] {
            let (q, n, m, d) = (r(rng), r(rng), r(rng), r(rng));
            list.push(probe(&[
                format!("{division} {q}, {n}, {m}"),
                format!("msub {d}, {q}, {m}, {n}"),
            ]));
        }
        // Conditions, after a comparison, a logic test, an addition and a negative comparison.
        for condition in &CONDITIONS[..14] {
            let (n, m, d, a, b) = (r(rng), r(rng), r(rng), r(rng), r(rng));
            list.push(probe(&[
                format!("cmp {n}, {m}"),
                format!("csel {d}, {a}, {b}, {condition}"),
            ]));
            list.push(probe(&[
                format!("tst {n}, {m}"),
                format!("csinc {d}, {a}, {b}, {condition}"),
            ]));
            list.push(probe(&[
                format!("adds {d}, {n}, {m}"),
                format!("csinv {a}, {a}, {b}, {condition}"),
            ]));
            list.push(probe(&[
                format!("cmn {n}, {m}"),
                format!("csneg {d}, {a}, {b}, {condition}"),
            ]));
        }
    }
    // Long multiplies, high halves and rev32, which take x and w registers by their own rules.
    for op in ["smaddl", "umaddl", "smsubl", "umsubl"] {
        let (d, n, m, a) = (
            register(rng, true),
            register(rng, false),
            register(rng, false),
            register(rng, true),
        );
        list.push(probe(&[format!("{op} {d}, {n}, {m}, {a}")]));
    }
    for op in ["smulh", "umulh"] {
        let (d, n, m) = (
            register(rng, true),
            register(rng, true),
            register(rng, true),
        );
        list.push(probe(&[format!("{op} {d}, {n}, {m}")]));
    }
    let (d, n) = (register(rng, true), register(rng, true));
    list.push(probe(&[format!("rev32 {d}, {n}")]));
    let (d, n) = (register(rng, true), register(rng, false));
    list.push(probe(&[format!("sxtw {d}, {n}")]));
    // Branches, taken or not: each skips the instruction after it when it's taken.
    let mut branches = Vec::new();
    for condition in &CONDITIONS[..14] {
        branches.push((vec!["cmp x1, x2".to_owned()], format!("b.{condition}")));
    }
    branches.push((Vec::new(), "cbz x1,".to_owned()));
    branches.push((Vec::new(), "cbnz w1,".to_owned()));
    for bit in [0, 5, 31, 63] {
        branches.push((Vec::new(), format!("tbz x1, #{bit},")));
        branches.push((Vec::new(), format!("tbnz x1, #{bit},")));
    }
    for (number, (before, branch)) in branches.into_iter().enumerate() {
        let label = format!(".Lskip{number}");
        let mut code = before;
        code.push(format!("{branch} {label}"));
        code.push("mov x0, #7".to_owned());
        code.push(format!("{label}:"));
        code.push("nop".to_owned());
        list.push(probe(&code));
    }
    // Loads and stores in the 16-byte cell at x15, every width, sign and addressing mode.
    for (load, offsets) in [
        ("ldr x0", [0, 8]),
        ("ldr w0", [0, 12]),
        ("ldrh w0", [0, 14]),
        ("ldrb w0", [0, 15]),
        ("ldrsw x0", [0, 4]),
        ("ldrsh x0", [2, 6]),
        ("ldrsh w0", [0, 10]),
        ("ldrsb x0", [1, 9]),
        ("ldrsb w0", [3, 7]),
    ] {
        for offset in offsets {
            list.push(memory(&[format!("{load}, [x15, #{offset}]")]));
        }
    }
    for (store, offsets) in [
        ("str x1", [0, 8]),
        ("str w1", [4, 12]),
        ("strh w1", [2, 14]),
        ("strb w1", [5, 15]),
    ] {
        for offset in offsets {
            list.push(memory(&[format!("{store}, [x15, #{offset}]")]));
        }
    }
    list.push(memory(&["ldur x0, [x15, #3]".to_owned()]));
    list.push(memory(&["stur w1, [x15, #9]".to_owned()]));
    list.push(memory(&["ldr x0, [x15, #8]!".to_owned()]));
    list.push(memory(&["ldr x0, [x15], #8".to_owned()]));
    list.push(memory(&["str x1, [x15, #8]!".to_owned()]));
    list.push(memory(&["strb w1, [x15], #3".to_owned()]));
    list.push(memory(&[
        "mov x2, #1".to_owned(),
        "ldr x0, [x15, x2, lsl #3]".to_owned(),
    ]));
    list.push(memory(&[
        "mov x2, #13".to_owned(),
        "ldrb w0, [x15, x2]".to_owned(),
    ]));
    list.push(memory(&[
        "mov x2, #6".to_owned(),
        "strh w1, [x15, x2]".to_owned(),
    ]));
    list.push(memory(&["ldp x0, x1, [x15]".to_owned()]));
    list.push(memory(&["stp x3, x4, [x15]".to_owned()]));
    list.push(memory(&["ldp w0, w1, [x15, #8]".to_owned()]));
    list.push(memory(&["stp w3, w4, [x15, #4]".to_owned()]));
    list.push(memory(&["ldpsw x0, x1, [x15, #8]".to_owned()]));
    list.push(memory(&["ldp x0, x1, [x15], #8".to_owned()]));
    list.push(memory(&["stp w3, w4, [x15, #8]!".to_owned()]));
    list
}

/// The harness: reads every vector from standard input, runs each one's probe on its values and
/// writes every result to standard output.
fn harness(probes: &[Probe], capacity: usize) -> String {
    let piece = |value: usize, shift: u32| (value >> shift) & 0xffff;
    let bytes_in = capacity * VECTOR;
    let mut source = format!(
        ".arch armv8-a
.arch_extension crc
.arch_extension aes
.arch_extension sha2
.text
.globl _start
.type _start, @function
_start:
    mov x19, #0
.Lread:
    mov x0, #0
    adrp x1, vectors
    add x1, x1, :lo12:vectors
    add x1, x1, x19
    movz x2, #{}
    movk x2, #{}, lsl #16
    sub x2, x2, x19
    mov x8, #63
    svc #0
    cmp x0, #0
    b.le .Lread_done
    add x19, x19, x0
    b .Lread
.Lread_done:
    mov x20, #{VECTOR}
    udiv x21, x19, x20
    mov x22, #0
.Lnext:
    cmp x22, x21
    b.hs .Lfinish
    adrp x23, vectors
    add x23, x23, :lo12:vectors
    mov x24, #{VECTOR}
    madd x23, x22, x24, x23
    adrp x26, cell
    add x26, x26, :lo12:cell
{}    ldr x0, [x23, #{}]
    msr nzcv, x0
    ldr x25, [x23, #{}]
",
        piece(bytes_in, 0),
        piece(bytes_in, 16),
        (0..CELL_WORDS / 2)
            .map(|pair| format!(
                "    ldp x0, x1, [x23, #{}]\n    stp x0, x1, [x26, #{}]\n",
                (REGISTERS + 1 + 2 * pair) * 8,
                pair * 16
            ))
            .collect::<String>(),
        REGISTERS * 8,
        (REGISTERS + 1 + CELL_WORDS) * 8,
    );
    for pair in 0..FP_REGISTERS / 2 {
        source.push_str(&format!(
            "    ldp q{}, q{}, [x23, #{}]\n",
            pair * 2,
            pair * 2 + 1,
            V_AT + pair * 32
        ));
    }
    for pair in 0..REGISTERS / 2 {
        source.push_str(&format!(
            "    ldp x{}, x{}, [x23, #{}]\n",
            pair * 2,
            pair * 2 + 1,
            pair * 16
        ));
    }
    source.push_str("    br x25\n.Lafter:\n    adrp x23, result\n    add x23, x23, :lo12:result\n");
    for pair in 0..FP_REGISTERS / 2 {
        source.push_str(&format!(
            "    stp q{}, q{}, [x23, #{}]\n",
            pair * 2,
            pair * 2 + 1,
            V_AT + pair * 32
        ));
    }
    for pair in 0..REGISTERS / 2 {
        source.push_str(&format!(
            "    stp x{}, x{}, [x23, #{}]\n",
            pair * 2,
            pair * 2 + 1,
            pair * 16
        ));
    }
    source.push_str(&format!(
        "    mrs x0, nzcv
    str x0, [x23, #{}]
    adrp x26, cell
    add x26, x26, :lo12:cell
{}    adrp x24, results
    add x24, x24, :lo12:results
    mov x26, #{RESULT}
    madd x24, x22, x26, x24
    mov x27, #0
.Lcopy:
    ldr x0, [x23, x27, lsl #3]
    str x0, [x24, x27, lsl #3]
    add x27, x27, #1
    cmp x27, #{}
    b.lo .Lcopy
    add x22, x22, #1
    b .Lnext
.Lfinish:
    mov x26, #{RESULT}
    mul x20, x21, x26
    mov x19, #0
.Lwrite:
    cmp x19, x20
    b.hs .Lexit
    mov x0, #1
    adrp x1, results
    add x1, x1, :lo12:results
    add x1, x1, x19
    sub x2, x20, x19
    mov x8, #64
    svc #0
    cmp x0, #0
    b.le .Lexit
    add x19, x19, x0
    b .Lwrite
.Lexit:
    mov x0, #0
    mov x8, #93
    svc #0
.size _start, .-_start
",
        REGISTERS * 8,
        (0..CELL_WORDS / 2)
            .map(|pair| format!(
                "    ldp x0, x1, [x26, #{}]\n    stp x0, x1, [x23, #{}]\n",
                pair * 16,
                (REGISTERS + 1 + 2 * pair) * 8
            ))
            .collect::<String>(),
        RESULT / 8,
    ));
    for (number, probe) in probes.iter().enumerate() {
        source.push_str(&format!(".type probe{number}, @function\nprobe{number}:\n"));
        for line in &probe.code {
            if line.ends_with(':') {
                source.push_str(&format!("{line}\n"));
            } else {
                source.push_str(&format!("    {line}\n"));
            }
        }
        source.push_str(&format!(
            "    b .Lafter\n.size probe{number}, .-probe{number}\n"
        ));
    }
    source.push_str(".bss\n.balign 16\n");
    for (name, size) in [
        ("vectors", capacity * VECTOR),
        ("results", capacity * RESULT),
        ("result", RESULT),
        ("cell", CELL_WORDS * 8),
    ] {
        source.push_str(&format!(
            ".type {name}, @object\n{name}:\n    .zero {size}\n.size {name}, {size}\n.balign 16\n"
        ));
    }
    source
}

/// A value from a mix of edge cases and plain random numbers.
fn value(rng: &mut ChaCha8Rng) -> u64 {
    const EDGES: [u64; 12] = [
        0,
        1,
        2,
        u64::MAX,
        u64::MAX - 1,
        i64::MAX as u64,
        i64::MIN as u64,
        0x7fff_ffff,
        0x8000_0000,
        0xffff_ffff,
        0x1_0000_0000,
        0x80,
    ];
    match rng.random_range(0..4) {
        0 => EDGES[rng.random_range(0..EDGES.len())],
        1 => rng.random_range(0..64),
        2 => u64::from(rng.random::<u32>()),
        _ => rng.random(),
    }
}

/// A V register's 128 bits: each half a double, two singles, or any bits, from a mix of edge
/// cases and plain random numbers, so a vector's every lane meets them.
fn fp_value(rng: &mut ChaCha8Rng) -> u128 {
    const DOUBLES: [u64; 30] = [
        0,
        0x8000_0000_0000_0000,
        0x7FF0_0000_0000_0000,
        0xFFF0_0000_0000_0000,
        // AArch64's default NaN, x86-64's, signaling NaNs and a quiet one with a payload.
        0x7FF8_0000_0000_0000,
        0xFFF8_0000_0000_0000,
        0x7FF0_0000_0000_0001,
        0xFFF4_0000_0000_0000,
        0x7FFC_0000_0000_0123,
        0x3FF0_0000_0000_0000,
        0xBFF0_0000_0000_0000,
        0x3FE0_0000_0000_0000,
        0x4004_0000_0000_0000,
        0xC004_0000_0000_0000,
        0x3FF8_0000_0000_0000,
        // Subnormals, the smallest normal and the largest finite.
        0x0000_0000_0000_0001,
        0x000F_FFFF_FFFF_FFFF,
        0x0010_0000_0000_0000,
        0x7FEF_FFFF_FFFF_FFFF,
        // The ends of the conversions: 2^63, -2^63, 2^64, just below 2^63, 2^31, 2^32, -2^31,
        // 2^32 - 0.5, -0.5 and -1.
        0x43E0_0000_0000_0000,
        0xC3E0_0000_0000_0000,
        0x43F0_0000_0000_0000,
        0x43DF_FFFF_FFFF_FFFF,
        0x41E0_0000_0000_0000,
        0x41F0_0000_0000_0000,
        0xC1E0_0000_0000_0000,
        0x41EF_FFFF_FFF0_0000,
        0xBFE0_0000_0000_0000,
        0xBFF0_0000_0000_0000,
        0x7E37_E43C_8800_759C,
    ];
    const SINGLES: [u32; 22] = [
        0,
        0x8000_0000,
        0x7F80_0000,
        0xFF80_0000,
        0x7FC0_0000,
        0xFFC0_0000,
        0x7F80_0001,
        0x7FA0_0000,
        0x3F80_0000,
        0xBF80_0000,
        0x3F00_0000,
        0x4020_0000,
        0x0000_0001,
        0x007F_FFFF,
        0x0080_0000,
        0x7F7F_FFFF,
        // 2^31, 2^32, 2^63, 2^64, -2^31 and just below 2^31.
        0x4F00_0000,
        0x4F80_0000,
        0x5F00_0000,
        0x5F80_0000,
        0xCF00_0000,
        0x4EFF_FFFF,
    ];
    let single = |rng: &mut ChaCha8Rng| -> u64 {
        u64::from(match rng.random_range(0..4) {
            0 | 1 => SINGLES[rng.random_range(0..SINGLES.len())],
            2 => rng.random_range(-1.0e6..1.0e6_f32).to_bits(),
            // Quarters, so ties come up.
            _ => (f32::from(rng.random_range(-400_i16..400)) / 4.0).to_bits(),
        })
    };
    let half = |rng: &mut ChaCha8Rng| -> u64 {
        match rng.random_range(0..6) {
            0 | 1 => DOUBLES[rng.random_range(0..DOUBLES.len())],
            2 => rng.random_range(-1.0e6..1.0e6_f64).to_bits(),
            3 => (f64::from(rng.random_range(-400_i32..400)) / 4.0).to_bits(),
            4 => (single(rng) << 32) | single(rng),
            _ => rng.random(),
        }
    };
    let low = half(rng);
    let high = half(rng);
    u128::from(high) << 64 | u128::from(low)
}

/// Builds the harness and reads it back as the checker's decoder does.
async fn build(source: &str, folder: &std::path::Path) -> (std::path::PathBuf, Machine) {
    let toolchain = Toolchain::discover().await.expect("llvm-mc and ld.lld");
    let (assembly, object, executable) = (
        folder.join("harness.s"),
        folder.join("harness.o"),
        folder.join("harness"),
    );
    std::fs::write(&assembly, source).expect("write the harness");
    toolchain
        .assemble(Profile::LinuxAarch64, &assembly, &object)
        .await
        .expect("assemble the harness");
    toolchain
        .link(Profile::LinuxAarch64, &object, &executable)
        .await
        .expect("link the harness");
    let machine = crate::checker::decode::read(&std::fs::read(&executable).expect("read"))
        .expect("decode the harness");
    (executable, machine)
}

/// Where the model ends: each register, NZCV's flags, the cell's words and each V register.
struct Ended {
    registers: Vec<u128>,
    flags: Vec<bool>,
    cell: [u128; CELL_WORDS],
    vectors: Vec<u128>,
}

/// One vector's values: the registers, NZCV, the cell's words and the V registers.
struct Values {
    registers: [u64; REGISTERS],
    nzcv: u64,
    cell: [u64; CELL_WORDS],
    vectors: [u128; FP_REGISTERS],
}

/// Runs a probe in the model, from `start` to `end`, on one vector's values.
fn model(
    image: &Image,
    (start, end): (u64, u64),
    values: &Values,
    cell_address: u64,
) -> Result<Ended, String> {
    let (registers, nzcv, cell) = (&values.registers, values.nzcv, values.cell);
    let mut pool = Pool::default();
    let mut state = State::start(image, &mut pool, &[], &[], Vec::new());
    for (slot, value) in registers.iter().enumerate() {
        state.core.registers[slot] = pool.bv(64, u128::from(*value));
    }
    for (number, value) in values.vectors.iter().enumerate() {
        let vector = std::rc::Rc::make_mut(&mut state.core.vectors[number]);
        for (at, byte) in vector.iter_mut().take(16).enumerate() {
            *byte = pool.bv(8, (value >> (8 * at)) & 0xff);
        }
    }
    let bit = |pool: &mut Pool, mask: u64| pool.boolean(nzcv & mask != 0);
    state.core.sign = bit(&mut pool, 1 << 31);
    state.core.zero = bit(&mut pool, 1 << 30);
    state.core.carry = bit(&mut pool, 1 << 29);
    state.core.overflow = bit(&mut pool, 1 << 28);
    state.core.last = Last::Other;
    for (word, value) in cell.iter().enumerate() {
        let value = pool.bv(64, u128::from(*value));
        state.core.store(&mut pool, cell_address + 8 * word as u64, value);
    }
    state.core.rip = start;
    let mut steps = 0;
    while state.core.rip != end {
        steps += 1;
        if steps > 16 {
            return Err("the model ran past the probe".into());
        }
        match step(image, &mut state, &mut pool) {
            Ok(Step::Next) => {}
            Ok(Step::Split(..) | Step::Choose(_)) => {
                return Err("the model split a run with known values".into());
            }
            Err(end) => return Err(format!("the model stopped: {end:?}")),
        }
    }
    let known = |pool: &Pool, term, what: &str| {
        pool.constant(term).ok_or_else(|| {
            format!(
                "the model doesn't know {what}: {}",
                pool.describe_parts(term)
            )
        })
    };
    let core = &state.core;
    let registers = core.registers[..REGISTERS]
        .iter()
        .enumerate()
        .map(|(slot, term)| known(&pool, *term, &format!("x{slot}")))
        .collect::<Result<_, _>>()?;
    let flags = [core.sign, core.zero, core.carry, core.overflow]
        .iter()
        .zip(FLAGS)
        .map(|(flag, (_, name))| {
            pool.truth(*flag)
                .ok_or_else(|| format!("the model doesn't know {name}"))
        })
        .collect::<Result<_, _>>()?;
    let mut words = [0; CELL_WORDS];
    for (word, slot) in words.iter_mut().enumerate() {
        let mut whole = None;
        for byte in (0..8).rev() {
            let term = core
                .memory
                .get(cell_address + 8 * word as u64 + byte)
                .ok_or("the memory cell is gone")?;
            whole = Some(match whole {
                None => term,
                Some(high) => pool.concat(high, term),
            });
        }
        *slot = known(&pool, whole.ok_or("no cell")?, "the cell")?;
    }
    let vectors = (0..FP_REGISTERS)
        .map(|number| {
            let mut whole = 0;
            for (at, byte) in core.vectors[number].iter().take(16).enumerate() {
                whole |= known(&pool, *byte, &format!("v{number}"))? << (8 * at);
            }
            Ok(whole)
        })
        .collect::<Result<_, String>>()?;
    Ok(Ended {
        registers,
        flags,
        cell: words,
        vectors,
    })
}

fn word(bytes: &[u8], index: usize) -> u64 {
    let mut word = [0; 8];
    word.copy_from_slice(&bytes[index * 8..index * 8 + 8]);
    u64::from_le_bytes(word)
}

/// Whether AArch64 programs run here: natively on Linux, or through QEMU registered with flag F.
fn runs_aarch64() -> bool {
    if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        return true;
    }
    let entry =
        std::fs::read_to_string("/proc/sys/fs/binfmt_misc/qemu-aarch64").unwrap_or_default();
    entry.lines().next() == Some("enabled")
        && entry.lines().any(|line| {
            line.strip_prefix("flags: ")
                .is_some_and(|flags| flags.contains('F'))
        })
}

#[tokio::test]
async fn every_aarch64_instruction_matches_the_cpu() {
    if !runs_aarch64() {
        crate::skiplog::note("QEMU doesn't run AArch64 programs here (install qemu-user-static)");
        return;
    }
    let seed = std::env::var("LENGUA_RANDOM_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or_else(|| {
            let mut bytes = [0; 8];
            getrandom::fill(&mut bytes).expect("a seed");
            u64::from_le_bytes(bytes)
        });
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let probes = probes(&mut rng);
    let per_probe = 24;
    let capacity = probes.len() * per_probe;
    let folder = tempfile::tempdir().expect("a folder");
    let (executable, machine) = build(&harness(&probes, capacity), folder.path()).await;
    let image = Image::new(&machine).expect("the harness has _start");
    let cell_address = machine.data["cell"].address;
    // Each probe runs from its start to its last instruction, the jump back to the harness.
    let spans: Vec<(u64, u64)> = (0..probes.len())
        .map(|number| {
            let function = &machine.functions[&format!("probe{number}")];
            (function.address, function.address + function.size - 4)
        })
        .collect();
    let mut vectors = Vec::with_capacity(capacity);
    let mut input = Vec::with_capacity(capacity * VECTOR);
    for (number, probe) in probes.iter().enumerate() {
        for _ in 0..per_probe {
            let mut registers = [0; REGISTERS];
            for register in &mut registers {
                *register = value(&mut rng);
            }
            if probe.memory {
                registers[15] = cell_address;
            }
            let nzcv = u64::from(rng.random_range(0_u32..16)) << 28;
            let cell: [u64; CELL_WORDS] = std::array::from_fn(|_| value(&mut rng));
            let mut fp = [0; FP_REGISTERS];
            for register in &mut fp {
                *register = fp_value(&mut rng);
            }
            for word in registers
                .iter()
                .chain(&[nzcv])
                .chain(&cell)
                .chain(&[spans[number].0])
            {
                input.extend_from_slice(&word.to_le_bytes());
            }
            for register in &fp {
                input.extend_from_slice(&register.to_le_bytes());
            }
            vectors.push((
                number,
                Values {
                    registers,
                    nzcv,
                    cell,
                    vectors: fp,
                },
            ));
        }
    }
    let mut child = Command::new(&executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run the harness");
    let mut stdin = child.stdin.take().expect("its input");
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let output = child.wait_with_output().expect("its results");
    writer
        .join()
        .expect("the writer")
        .expect("write the vectors");
    assert!(output.status.success(), "the harness failed: {output:?}");
    assert_eq!(output.stdout.len(), capacity * RESULT, "every result");
    let mut problems = Vec::new();
    for (position, (number, values)) in vectors.iter().enumerate() {
        let probe = &probes[*number];
        let real = &output.stdout[position * RESULT..(position + 1) * RESULT];
        let describe = || {
            let registers: Vec<String> = values
                .registers
                .iter()
                .enumerate()
                .map(|(slot, value)| format!("x{slot}={value:#x}"))
                .collect();
            let fp: Vec<String> = values
                .vectors
                .iter()
                .enumerate()
                .map(|(number, value)| format!("v{number}={value:#x}"))
                .collect();
            let cell: Vec<String> = values
                .cell
                .iter()
                .map(|word| format!("{word:#x}"))
                .collect();
            format!(
                "`{}` with {} nzcv={:#x} cell={} {}",
                probe.code.join("; "),
                registers.join(" "),
                values.nzcv,
                cell.join(","),
                fp.join(" ")
            )
        };
        let ended = match model(&image, spans[*number], values, cell_address) {
            Ok(ended) => ended,
            Err(problem) => {
                problems.push(format!("{}: {problem}", describe()));
                continue;
            }
        };
        for (slot, modelled) in ended.registers.iter().enumerate() {
            let expected = u128::from(word(real, slot));
            if *modelled != expected {
                problems.push(format!(
                    "{}: x{slot} is {modelled:#x}, the CPU says {expected:#x}",
                    describe()
                ));
            }
        }
        let real_flags = word(real, REGISTERS);
        for ((mask, name), modelled) in FLAGS.iter().zip(&ended.flags) {
            let expected = real_flags & mask != 0;
            if *modelled != expected {
                problems.push(format!(
                    "{}: {name} is {modelled}, the CPU says {expected}",
                    describe()
                ));
            }
        }
        for (word_number, modelled) in ended.cell.iter().enumerate() {
            let expected = u128::from(word(real, REGISTERS + 1 + word_number));
            if *modelled != expected {
                problems.push(format!(
                    "{}: the cell's word {word_number} is {modelled:#x}, the CPU says \
                     {expected:#x}",
                    describe()
                ));
            }
        }
        for (number, modelled) in ended.vectors.iter().enumerate() {
            let at = V_AT / 8 + 2 * number;
            let expected = u128::from(word(real, at + 1)) << 64 | u128::from(word(real, at));
            if *modelled != expected {
                problems.push(format!(
                    "{}: v{number} is {modelled:#x}, the CPU says {expected:#x}",
                    describe()
                ));
            }
        }
    }
    let shown: Vec<&String> = problems.iter().take(30).collect();
    assert!(
        problems.is_empty(),
        "seed {seed}: {} of {} runs differ:\n{shown:#?}",
        problems.len(),
        vectors.len()
    );
}
