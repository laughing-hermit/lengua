//! Random-value tests of the prover's model of each instruction against this computer's CPU. One
//! harness program holds every probe, a few instructions each; it runs them all on many random
//! register, flag and memory values and prints where they ended. The model must end the same
//! wherever it says it knows the answer: a flag it calls undefined is the only thing it may skip.

use std::io::Write as _;
use std::process::{Command, Stdio};

use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

use super::{Image, Last, State, Step, step};
use crate::checker::decode::Machine;
use crate::platform::Profile;
use crate::prove::term::Pool;
use crate::toolchain::Toolchain;

/// Registers by width and number, in the CPU's own order (rax, rcx, rdx, rbx, rsp, …).
const NAMES: [[&str; 16]; 4] = [
    [
        "al", "cl", "dl", "bl", "spl", "bpl", "sil", "dil", "r8b", "r9b", "r10b", "r11b", "r12b",
        "r13b", "r14b", "r15b",
    ],
    [
        "ax", "cx", "dx", "bx", "sp", "bp", "si", "di", "r8w", "r9w", "r10w", "r11w", "r12w",
        "r13w", "r14w", "r15w",
    ],
    [
        "eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi", "r8d", "r9d", "r10d", "r11d",
        "r12d", "r13d", "r14d", "r15d",
    ],
    [
        "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12",
        "r13", "r14", "r15",
    ],
];

/// The registers that can sit next to ah, ch, dh and bh in one instruction.
const LEGACY_BYTES: [&str; 8] = ["al", "cl", "dl", "bl", "ah", "ch", "dh", "bh"];

/// Condition codes, as instruction suffixes.
const CONDITIONS: [&str; 16] = [
    "o", "no", "b", "ae", "e", "ne", "be", "a", "s", "ns", "p", "np", "l", "ge", "le", "g",
];

/// Bytes per vector: 16 registers, the flags, the memory cell and the probe's address.
const VECTOR: usize = 19 * 8;

/// Bytes per result: 16 registers, the flags and the memory cell.
const RESULT: usize = 18 * 8;

/// The flags the model keeps: carry, parity, zero, sign and overflow.
const FLAGS: [(u64, &str); 5] = [
    (1, "CF"),
    (1 << 2, "PF"),
    (1 << 6, "ZF"),
    (1 << 7, "SF"),
    (1 << 11, "OF"),
];

/// How a probe's values are chosen.
#[derive(Debug, Clone, Copy)]
enum Values {
    /// Anything, with rsp just above the memory cell.
    Any,
    /// Anything, with rsp at the memory cell, for `pop`.
    Pop,
    /// A division by the register numbered `divisor` (or the memory cell when it's 16) that
    /// doesn't fault.
    Divide {
        width: u32,
        signed: bool,
        divisor: usize,
    },
}

/// Instructions to run, and how many of them the model steps through.
struct Probe {
    code: Vec<String>,
    values: Values,
}

fn one(code: String) -> Probe {
    Probe {
        code: vec![code],
        values: Values::Any,
    }
}

fn index(width: u32) -> usize {
    match width {
        8 => 0,
        16 => 1,
        32 => 2,
        _ => 3,
    }
}

/// A random register other than rsp, `width` bits wide.
fn register(rng: &mut ChaCha8Rng, width: u32) -> String {
    let mut number = rng.random_range(0..15);
    if number >= 4 {
        number += 1;
    }
    NAMES[index(width)][number].to_owned()
}

/// A random register among al, cl, dl, bl, ah, ch, dh and bh.
fn legacy_byte(rng: &mut ChaCha8Rng) -> String {
    LEGACY_BYTES[rng.random_range(0..LEGACY_BYTES.len())].to_owned()
}

fn memory(width: u32) -> String {
    let size = match width {
        8 => "byte",
        16 => "word",
        32 => "dword",
        _ => "qword",
    };
    format!("{size} ptr [rip + cell]")
}

/// A random immediate that fits an instruction on `width` bits.
fn immediate(rng: &mut ChaCha8Rng, width: u32) -> i64 {
    match (width, rng.random_range(0..3)) {
        (_, 0) => i64::from(rng.random_range(-128_i32..128)),
        (8, _) => i64::from(rng.random::<i8>()),
        (16, _) => i64::from(rng.random::<i16>()),
        _ => i64::from(rng.random::<i32>()),
    }
}

/// Every probe: each instruction the model runs, in each width and operand kind.
#[expect(clippy::too_many_lines, reason = "a flat list of instruction forms")]
fn probes(rng: &mut ChaCha8Rng) -> Vec<Probe> {
    let mut list = Vec::new();
    for width in [8_u32, 16, 32, 64] {
        for op in [
            "add", "adc", "sub", "sbb", "cmp", "and", "or", "xor", "test",
        ] {
            for _ in 0..3 {
                let (a, b) = (register(rng, width), register(rng, width));
                list.push(one(format!("{op} {a}, {b}")));
            }
            let a = register(rng, width);
            let value = immediate(rng, width);
            list.push(one(format!("{op} {a}, {value}")));
            list.push(one(format!("{op} {}, {a}", memory(width))));
            list.push(one(format!("{op} {}, {value}", memory(width))));
            if op != "test" {
                list.push(one(format!("{op} {a}, {}", memory(width))));
            }
            if width == 8 {
                let (a, b) = (legacy_byte(rng), legacy_byte(rng));
                list.push(one(format!("{op} {a}, {b}")));
            }
        }
        for op in ["inc", "dec", "neg", "not"] {
            list.push(one(format!("{op} {}", register(rng, width))));
            list.push(one(format!("{op} {}", memory(width))));
            if width == 8 {
                list.push(one(format!("{op} {}", legacy_byte(rng))));
            }
        }
        for op in ["shl", "shr", "sar", "rol", "ror"] {
            let a = register(rng, width);
            list.push(one(format!("{op} {a}, 1")));
            list.push(one(format!("{op} {a}, cl")));
            for _ in 0..3 {
                let count = rng.random_range(0..2 * width);
                list.push(one(format!("{op} {}, {count}", register(rng, width))));
            }
            list.push(one(format!(
                "{op} {}, {}",
                memory(width),
                rng.random_range(0..width)
            )));
        }
        let (a, b) = (register(rng, width), register(rng, width));
        list.push(one(format!("mov {a}, {b}")));
        list.push(one(format!("mov {a}, {}", immediate(rng, width))));
        list.push(one(format!("mov {a}, {}", memory(width))));
        list.push(one(format!("mov {}, {b}", memory(width))));
        list.push(one(format!(
            "mov {}, {}",
            memory(width),
            immediate(rng, width)
        )));
        list.push(one(format!("xchg {a}, {b}")));
        list.push(one(format!("xchg {a}, {}", memory(width))));
        if width == 8 {
            list.push(one(format!(
                "mov {}, {}",
                legacy_byte(rng),
                legacy_byte(rng)
            )));
            list.push(one(format!(
                "xchg {}, {}",
                legacy_byte(rng),
                legacy_byte(rng)
            )));
        }
        if width >= 32 {
            list.push(one(format!("bswap {}", register(rng, width))));
        }
        if width >= 16 {
            for from in [8_u32, 16, 32] {
                if from >= width {
                    continue;
                }
                let to = register(rng, width);
                let source = if from == 8 && width < 64 && rng.random_bool(0.3) {
                    // ah and the rest can't share an instruction with a REX prefix, which a
                    // 64-bit destination or r8 and above would need.
                    let to = NAMES[index(width)][rng.random_range(0..4)];
                    list.push(one(format!("movzx {to}, {}", legacy_byte(rng))));
                    list.push(one(format!("movsx {to}, {}", legacy_byte(rng))));
                    continue;
                } else {
                    register(rng, from)
                };
                let (zero, sign) = if from == 32 {
                    (None, "movsxd")
                } else {
                    (Some("movzx"), "movsx")
                };
                if let Some(zero) = zero {
                    list.push(one(format!("{zero} {to}, {source}")));
                    list.push(one(format!("{zero} {to}, {}", memory(from))));
                }
                list.push(one(format!("{sign} {to}, {source}")));
                list.push(one(format!("{sign} {to}, {}", memory(from))));
            }
            for _ in 0..3 {
                let (base, scaled) = (register(rng, 64), register(rng, 64));
                let scale = [1, 2, 4, 8][rng.random_range(0..4)];
                let offset = immediate(rng, 32);
                list.push(one(format!(
                    "lea {}, [{base} + {scaled}*{scale} + {offset}]",
                    register(rng, width)
                )));
            }
            list.push(one(format!("lea {}, [rip + cell]", register(rng, width))));
            list.push(one(format!("lea {}, [rsp + 8]", register(rng, width))));
            let (a, b) = (register(rng, width), register(rng, width));
            list.push(one(format!("imul {a}, {b}")));
            list.push(one(format!("imul {a}, {}", memory(width))));
            list.push(one(format!("imul {a}, {b}, {}", immediate(rng, width))));
            list.push(one(format!(
                "imul {a}, {b}, {}",
                rng.random_range(-128..128)
            )));
            for condition in CONDITIONS {
                let (a, b) = (register(rng, width), register(rng, width));
                list.push(one(format!("cmov{condition} {a}, {b}")));
            }
            list.push(one(format!("cmovl {a}, {}", memory(width))));
        }
        for op in ["mul", "imul"] {
            list.push(one(format!("{op} {}", register(rng, width))));
            list.push(one(format!("{op} {}", memory(width))));
        }
        for signed in [false, true] {
            let op = if signed { "idiv" } else { "div" };
            for _ in 0..3 {
                // Neither half of the dividend: those would leave no room to avoid a fault.
                let divisor = loop {
                    let number = rng.random_range(0..16);
                    if ![0, 2, 4].contains(&number) {
                        break number;
                    }
                };
                list.push(Probe {
                    code: vec![format!("{op} {}", NAMES[index(width)][divisor])],
                    values: Values::Divide {
                        width,
                        signed,
                        divisor,
                    },
                });
            }
            list.push(Probe {
                code: vec![format!("{op} {}", memory(width))],
                values: Values::Divide {
                    width,
                    signed,
                    divisor: 16,
                },
            });
        }
        // A flag-setting instruction, then each condition read from it: after cmp, sub and the
        // logic operations the model reads the condition from the operands, not the flags.
        let mut setters = Vec::new();
        for op in [
            "cmp", "sub", "test", "and", "or", "xor", "add", "adc", "sbb", "imul",
        ] {
            if width >= 16 || op != "imul" {
                let (a, b) = (register(rng, width), register(rng, width));
                setters.push(format!("{op} {a}, {b}"));
            }
        }
        for op in ["inc", "dec", "neg"] {
            setters.push(format!("{op} {}", register(rng, width)));
        }
        setters.push(format!("shl {}, 1", register(rng, width)));
        setters.push(format!("sar {}, 3", register(rng, width)));
        let (a, value) = (register(rng, width), immediate(rng, width));
        setters.push(format!("cmp {a}, {value}"));
        let (a, value) = (register(rng, width), immediate(rng, width));
        setters.push(format!("test {a}, {value}"));
        for (number, setter) in setters.iter().enumerate() {
            let every = number < 4 || setter.starts_with("cmp") || setter.starts_with("test");
            for condition in CONDITIONS {
                if !every && rng.random_range(0..4) != 0 {
                    continue;
                }
                let reader = if width >= 16 && rng.random_bool(0.5) {
                    let (a, b) = (register(rng, width), register(rng, width));
                    format!("cmov{condition} {a}, {b}")
                } else {
                    format!("set{condition} {}", register(rng, 8))
                };
                list.push(Probe {
                    code: vec![setter.clone(), reader],
                    values: Values::Any,
                });
            }
        }
    }
    for code in ["cbw", "cwde", "cdqe", "cwd", "cdq", "cqo", "nop"] {
        list.push(one(code.to_owned()));
    }
    for _ in 0..4 {
        list.push(one(format!("push {}", register(rng, 64))));
        list.push(Probe {
            code: vec![format!("pop {}", register(rng, 64))],
            values: Values::Pop,
        });
    }
    list.push(one(format!("push {}", immediate(rng, 32))));
    list.push(one("push -1".to_owned()));
    list.push(one("push qword ptr [rip + cell]".to_owned()));
    list.push(Probe {
        code: vec!["pop qword ptr [rip + cell]".to_owned()],
        values: Values::Pop,
    });
    list.push(one(format!(
        "movabs {}, {}",
        register(rng, 64),
        rng.random::<i64>()
    )));
    list.push(one(format!("mov {}, rsp", register(rng, 64))));
    list.push(one("add rsp, 16".to_owned()));
    list
}

/// The harness: reads vectors from standard input, runs each one's probe with its registers,
/// flags and memory cell, and writes where each ended to standard output.
fn harness(probes: &[Probe], capacity: usize) -> String {
    let mut source = String::from(
        ".intel_syntax noprefix\n.text\n.globl _start\n.type _start, @function\n_start:\n",
    );
    source.push_str(&format!(
        "    xor r12d, r12d
.Lread:
    xor eax, eax
    xor edi, edi
    lea rsi, [rip + vectors]
    add rsi, r12
    mov rdx, {}
    sub rdx, r12
    syscall
    test rax, rax
    jle .Lread_done
    add r12, rax
    jmp .Lread
.Lread_done:
    mov rax, r12
    xor edx, edx
    mov ecx, {VECTOR}
    div rcx
    mov [rip + count], rax
    mov qword ptr [rip + position], 0
.Lnext:
    mov rcx, [rip + position]
    cmp rcx, [rip + count]
    jae .Lfinish
    imul rsi, rcx, {VECTOR}
    lea rdi, [rip + vectors]
    add rsi, rdi
    lea rdi, [rip + current]
    mov ecx, 19
.Lcopy_in:
    mov rax, [rsi]
    mov [rdi], rax
    add rsi, 8
    add rdi, 8
    dec ecx
    jnz .Lcopy_in
    mov rax, [rip + current + 136]
    mov [rip + cell], rax
    lea rsp, [rip + scratch + 4096]
    push qword ptr [rip + current + 128]
    popfq
",
        capacity * VECTOR
    ));
    for (number, name) in NAMES[3].iter().enumerate() {
        source.push_str(&format!(
            "    mov {name}, [rip + current + {}]\n",
            number * 8
        ));
    }
    source.push_str("    jmp qword ptr [rip + current + 144]\n.Lafter:\n");
    for (number, name) in NAMES[3].iter().enumerate() {
        source.push_str(&format!(
            "    mov [rip + result + {}], {name}\n",
            number * 8
        ));
    }
    source.push_str(&format!(
        "    lea rsp, [rip + scratch + 4096]
    pushfq
    pop qword ptr [rip + result + 128]
    mov rax, [rip + cell]
    mov [rip + result + 136], rax
    mov rcx, [rip + position]
    imul rdi, rcx, {RESULT}
    lea rsi, [rip + results]
    add rdi, rsi
    lea rsi, [rip + result]
    mov ecx, 18
.Lcopy_out:
    mov rax, [rsi]
    mov [rdi], rax
    add rsi, 8
    add rdi, 8
    dec ecx
    jnz .Lcopy_out
    inc qword ptr [rip + position]
    jmp .Lnext
.Lfinish:
    mov rax, [rip + count]
    imul r13, rax, {RESULT}
    xor r12d, r12d
.Lwrite:
    cmp r12, r13
    jae .Lexit
    mov eax, 1
    mov edi, 1
    lea rsi, [rip + results]
    add rsi, r12
    mov rdx, r13
    sub rdx, r12
    syscall
    test rax, rax
    jle .Lexit
    add r12, rax
    jmp .Lwrite
.Lexit:
    mov eax, 60
    xor edi, edi
    syscall
.size _start, .-_start
"
    ));
    for (number, probe) in probes.iter().enumerate() {
        source.push_str(&format!(".type probe{number}, @function\nprobe{number}:\n"));
        for line in &probe.code {
            source.push_str(&format!("    {line}\n"));
        }
        source.push_str(&format!(
            "    jmp .Lafter\n.size probe{number}, .-probe{number}\n"
        ));
    }
    source.push_str(".bss\n.balign 8\n");
    for (name, size) in [
        ("vectors", capacity * VECTOR),
        ("results", capacity * RESULT),
        ("current", VECTOR),
        ("result", RESULT),
        ("cell", 8),
        ("position", 8),
        ("count", 8),
        ("scratch", 4096),
    ] {
        source.push_str(&format!(
            ".type {name}, @object\n{name}:\n    .zero {size}\n.size {name}, {size}\n"
        ));
    }
    source
}

/// A value from a mix of edge cases and plain random numbers.
fn value(rng: &mut ChaCha8Rng) -> u64 {
    const EDGES: [u64; 18] = [
        0,
        1,
        2,
        0x7f,
        0x80,
        0xff,
        0x100,
        0x7fff,
        0x8000,
        0xffff,
        0x7fff_ffff,
        0x8000_0000,
        0xffff_ffff,
        0x1_0000_0000,
        0x7fff_ffff_ffff_ffff,
        0x8000_0000_0000_0000,
        u64::MAX,
        u64::MAX - 1,
    ];
    match rng.random_range(0..4) {
        0 => rng.random(),
        1 => EDGES[rng.random_range(0..EDGES.len())],
        // An edge in the low bits, with random bits above it.
        2 => {
            let bits = [8, 16, 32][rng.random_range(0..3)];
            let low = EDGES[rng.random_range(0..EDGES.len())] & ((1 << bits) - 1);
            (rng.random::<u64>() << bits) | low
        }
        _ => rng.random_range(0..64),
    }
}

/// The value of `width` bits at the bottom of `value`, or the sign-extended one.
const fn low(value: u64, width: u32, signed: bool) -> i128 {
    let mask = if width == 64 {
        u64::MAX
    } else {
        (1 << width) - 1
    };
    let bits = value & mask;
    if signed && width < 64 && bits >> (width - 1) & 1 == 1 {
        bits as i128 - (1_i128 << width)
    } else if signed && width == 64 {
        bits as i64 as i128
    } else {
        bits as i128
    }
}

/// Whether dividing as the probe does faults on these registers and memory cell.
fn faults(registers: &[u64; 16], cell: u64, width: u32, signed: bool, divisor: usize) -> bool {
    let source = if divisor == 16 {
        cell
    } else {
        registers[divisor]
    };
    let divisor = low(source, width, signed);
    if divisor == 0 {
        return true;
    }
    let (rax, rdx) = (registers[0], registers[2]);
    let dividend: i128 = if width == 8 {
        low(rax, 16, signed)
    } else {
        let high = low(rdx, width, signed);
        let low_half = low(rax, width, false);
        (high << width) | low_half
    };
    // At 64 bits the dividend may not fit an i128 when unsigned.
    let quotient = if !signed && width == 64 {
        let dividend = (u128::from(rdx) << 64) | u128::from(rax);
        let divisor = u128::from(source);
        return dividend / divisor > u128::from(u64::MAX);
    } else {
        dividend / divisor
    };
    let (smallest, largest) = if signed {
        (-(1_i128 << (width - 1)), (1_i128 << (width - 1)) - 1)
    } else {
        (0, (1_i128 << width) - 1)
    };
    quotient < smallest || quotient > largest
}

/// One vector of values for `probe`.
fn vector(rng: &mut ChaCha8Rng, probe: &Probe, cell_address: u64) -> ([u64; 16], u64, u64) {
    loop {
        let mut registers = [0_u64; 16];
        for register in &mut registers {
            *register = value(rng);
        }
        let cell = value(rng);
        let mut flags = 0x202;
        for (bit, _) in FLAGS {
            if rng.random_bool(0.5) {
                flags |= bit;
            }
        }
        if rng.random_bool(0.5) {
            flags |= 1 << 4;
        }
        registers[4] = match probe.values {
            Values::Pop => cell_address,
            _ => cell_address + 8,
        };
        if let Values::Divide {
            width,
            signed,
            divisor,
        } = probe.values
        {
            if rng.random_bool(0.5) {
                // A dividend whose high half is just the low half's sign, which most divisors
                // divide without a fault.
                let negative = signed && low(registers[0], width, true) < 0;
                if width == 8 {
                    registers[0] = (registers[0] & !0xff00) | if negative { 0xff00 } else { 0 };
                } else {
                    registers[2] = if negative { u64::MAX } else { 0 };
                }
            }
            if faults(&registers, cell, width, signed, divisor) {
                continue;
            }
        }
        return (registers, flags, cell);
    }
}

/// Builds `source` into an executable in `folder`, and decodes it.
async fn build(source: &str, folder: &std::path::Path) -> (std::path::PathBuf, Machine) {
    let toolchain = Toolchain::discover().await.expect("llvm-mc and ld.lld");
    let (assembly, object, executable) = (
        folder.join("harness.s"),
        folder.join("harness.o"),
        folder.join("harness"),
    );
    std::fs::write(&assembly, source).expect("write the harness");
    toolchain
        .assemble(Profile::LinuxX86_64, &assembly, &object)
        .await
        .expect("assemble the harness");
    toolchain
        .link(&object, &executable)
        .await
        .expect("link the harness");
    let machine = crate::checker::decode::read(&std::fs::read(&executable).expect("read"))
        .expect("decode the harness");
    (executable, machine)
}

/// The values `term` can take: one when it's known, and every value the flags the CPU leaves
/// undefined allow when it depends on them. Anything else it depends on is a mistake.
fn possible(pool: &Pool, term: crate::prove::term::Term) -> Result<Vec<u128>, String> {
    if let Some(value) = pool.constant(term) {
        return Ok(vec![value]);
    }
    let variables = pool.variables(&[term]);
    let names: Vec<String> = variables
        .iter()
        .filter_map(|variable| pool.name(*variable).map(str::to_owned))
        .collect();
    if let Some(other) = names.iter().find(|name| !name.starts_with("undefined")) {
        return Err(format!("depends on {other}"));
    }
    if names.len() > 6 {
        return Err("depends on too many undefined flags".into());
    }
    let mut values = Vec::new();
    for assignment in 0..1_u32 << names.len() {
        let chosen: std::collections::HashMap<String, u128> = names
            .iter()
            .enumerate()
            .map(|(bit, name)| (name.clone(), u128::from(assignment >> bit & 1)))
            .collect();
        let value = pool.evaluate(term, &chosen).ok_or("can't be evaluated")?;
        if !values.contains(&value) {
            values.push(value);
        }
    }
    Ok(values)
}

/// Where the model ends: the values each register, flag and the memory cell can take.
struct Ended {
    registers: Vec<Vec<u128>>,
    flags: Vec<Vec<u128>>,
    cell: Vec<u128>,
}

/// Runs `probe` in the model on one vector's values; an error when it doesn't run it through.
fn model(
    image: &Image,
    probe: &Probe,
    address: u64,
    (registers, flags, cell): ([u64; 16], u64, u64),
    cell_address: u64,
) -> Result<Ended, String> {
    let mut pool = Pool::default();
    let mut state = State::start(image, &mut pool, &[], Vec::new());
    for (slot, value) in registers.iter().enumerate() {
        state.registers[slot] = pool.bv(64, u128::from(*value));
    }
    let bit = |pool: &mut Pool, mask: u64| pool.boolean(flags & mask != 0);
    state.carry = bit(&mut pool, 1);
    state.parity = bit(&mut pool, 1 << 2);
    state.zero = bit(&mut pool, 1 << 6);
    state.sign = bit(&mut pool, 1 << 7);
    state.overflow = bit(&mut pool, 1 << 11);
    state.last = Last::Other;
    let cell_value = pool.bv(64, u128::from(cell));
    state.store(&mut pool, cell_address, cell_value);
    state.rip = address;
    for _ in &probe.code {
        match step(image, &mut state, &mut pool) {
            Ok(Step::Next) => {}
            Ok(Step::Split(..)) => return Err("the model split a run with known values".into()),
            Err(end) => return Err(format!("the model stopped: {end:?}")),
        }
    }
    let registers = state
        .registers
        .iter()
        .enumerate()
        .map(|(slot, register)| {
            possible(&pool, *register).map_err(|problem| format!("{} {problem}", NAMES[3][slot]))
        })
        .collect::<Result<_, _>>()?;
    let flags = [
        state.carry,
        state.parity,
        state.zero,
        state.sign,
        state.overflow,
    ]
    .iter()
    .map(|flag| possible(&pool, *flag))
    .collect::<Result<_, _>>()?;
    let mut bytes = Vec::new();
    for byte in 0..8 {
        let term = state
            .memory
            .get(&(cell_address + byte))
            .ok_or("the memory cell is gone")?;
        bytes.push(*term);
    }
    let mut whole = bytes[0];
    for byte in &bytes[1..] {
        whole = pool.concat(*byte, whole);
    }
    let cell = possible(&pool, whole).map_err(|problem| format!("the cell {problem}"))?;
    Ok(Ended {
        registers,
        flags,
        cell,
    })
}

fn word(bytes: &[u8], index: usize) -> u64 {
    u64::from_le_bytes(
        bytes[index * 8..index * 8 + 8]
            .try_into()
            .expect("eight bytes"),
    )
}

#[tokio::test]
async fn every_instruction_matches_this_cpu() {
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
    let addresses: Vec<u64> = (0..probes.len())
        .map(|number| machine.functions[&format!("probe{number}")].address)
        .collect();
    let mut vectors = Vec::with_capacity(capacity);
    let mut input = Vec::with_capacity(capacity * VECTOR);
    for (number, probe) in probes.iter().enumerate() {
        for _ in 0..per_probe {
            let (registers, flags, cell) = vector(&mut rng, probe, cell_address);
            for value in registers.iter().chain(&[flags, cell, addresses[number]]) {
                input.extend_from_slice(&value.to_le_bytes());
            }
            vectors.push((number, (registers, flags, cell)));
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
    for (position, (number, vector)) in vectors.iter().enumerate() {
        let probe = &probes[*number];
        let real = &output.stdout[position * RESULT..(position + 1) * RESULT];
        let describe = || {
            let (registers, flags, cell) = vector;
            let registers: Vec<String> = registers
                .iter()
                .enumerate()
                .map(|(slot, value)| format!("{}={value:#x}", NAMES[3][slot]))
                .collect();
            format!(
                "`{}` with {} flags={flags:#x} cell={cell:#x}",
                probe.code.join("; "),
                registers.join(" ")
            )
        };
        let ended = match model(&image, probe, addresses[*number], *vector, cell_address) {
            Ok(ended) => ended,
            Err(problem) => {
                problems.push(format!("{}: {problem}", describe()));
                continue;
            }
        };
        for (slot, values) in ended.registers.iter().enumerate() {
            let expected = word(real, slot);
            if !values.contains(&u128::from(expected)) {
                problems.push(format!(
                    "{}: {} is {values:x?}, the CPU says {expected:#x}",
                    describe(),
                    NAMES[3][slot]
                ));
            }
        }
        let real_flags = word(real, 16);
        for ((mask, name), values) in FLAGS.iter().zip(&ended.flags) {
            let expected = u128::from(real_flags & mask != 0);
            if !values.contains(&expected) {
                problems.push(format!(
                    "{}: {name} is {values:?}, the CPU says {expected}",
                    describe()
                ));
            }
        }
        if !ended.cell.contains(&u128::from(word(real, 17))) {
            problems.push(format!(
                "{}: the cell holds {:x?}, the CPU says {:#x}",
                describe(),
                ended.cell,
                word(real, 17)
            ));
        }
    }
    let count = problems.len();
    problems.truncate(25);
    assert!(
        problems.is_empty(),
        "seed {seed}: {count} differences from the CPU, over {} probes and {capacity} runs:\n{}",
        probes.len(),
        problems.join("\n")
    );
}
