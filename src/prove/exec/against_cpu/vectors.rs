//! The vector instructions against this CPU: the same kind of harness as the scalar one, with
//! zmm0 to zmm7, k0 to k7 and a 128-byte memory cell in each run's state. Legacy SSE, VEX and
//! EVEX forms, register and memory operands, and merge and zero masks are all probed.

use std::io::Write as _;
use std::process::{Command, Stdio};
use std::rc::Rc;

use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

use super::super::{Image, Last, State, Step, VECTOR_BYTES, step};
use super::{CONDITIONS, FLAGS, NAMES, allows, build, possible, register};
use crate::prove::term::{Pool, Term};

mod crypto;
mod floats;

/// Where each part of a run's state sits in its input and in its result.
const GPRS: usize = 0;
const FLAGS_AT: usize = 128;
const ZMMS: usize = 136;
const KS: usize = ZMMS + 8 * 64;
const CELL: usize = KS + 8 * 8;
const CELL_BYTES: usize = 128;
const ADDRESS: usize = CELL + CELL_BYTES;
/// The bytes of one run's input: the state, then the probe's address.
const INPUT: usize = ADDRESS + 8;
/// The bytes of one run's result: the state.
const RESULT: usize = ADDRESS;

/// Bytes that vector code tests for: controls, blanks, digits, letter bounds and the top bit.
const EDGES: [u8; 16] = [
    0x00, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x20, b'0', b'9', b'A', b'Z', b'a', b'z', 0x7f, 0x80, 0xff,
];

/// A random vector register of `bytes` bytes among the eight the harness records.
fn vector_register(rng: &mut ChaCha8Rng, bytes: usize) -> String {
    let number = rng.random_range(0..8);
    match bytes {
        16 => format!("xmm{number}"),
        32 => format!("ymm{number}"),
        _ => format!("zmm{number}"),
    }
}

/// The memory cell as an operand of `bytes` bytes.
fn cell(bytes: usize) -> &'static str {
    match bytes {
        1 => "byte ptr [rip + cell]",
        2 => "word ptr [rip + cell]",
        4 => "dword ptr [rip + cell]",
        8 => "qword ptr [rip + cell]",
        16 => "xmmword ptr [rip + cell]",
        32 => "ymmword ptr [rip + cell]",
        _ => "zmmword ptr [rip + cell]",
    }
}

/// A random write mask, k1 to k7.
fn mask_register(rng: &mut ChaCha8Rng) -> String {
    format!("k{}", rng.random_range(1..8))
}

/// A write mask on a destination, merging or zeroing, or none.
fn with_mask(rng: &mut ChaCha8Rng, destination: &str) -> String {
    match rng.random_range(0..3) {
        0 => destination.to_owned(),
        1 => format!("{destination} {{{}}}", mask_register(rng)),
        _ => format!("{destination} {{{}}}{{z}}", mask_register(rng)),
    }
}

/// Every vector probe.
#[expect(clippy::too_many_lines, reason = "a flat list of instruction forms")]
fn probes(rng: &mut ChaCha8Rng) -> Vec<Vec<String>> {
    let mut list: Vec<Vec<String>> = Vec::new();
    let one = |code: String| vec![code];
    let x = |rng: &mut ChaCha8Rng| vector_register(rng, 16);
    // Legacy SSE, on xmm registers: the upper bytes stay.
    for op in [
        "movdqu", "movdqa", "movups", "movaps", "pcmpeqb", "pcmpeqw", "pcmpeqd", "pcmpeqq",
        "pcmpgtb", "pcmpgtw", "pcmpgtd", "pcmpgtq", "pand", "pandn", "por", "pxor", "paddb",
        "paddw", "paddd", "paddq", "psubb", "psubw", "psubd", "psubq", "paddusb", "psubusb",
        "pminub", "pmaxub", "pshufb", "andps", "orps", "xorps", "andnps",
    ] {
        for _ in 0..2 {
            let (a, b) = (x(rng), x(rng));
            list.push(one(format!("{op} {a}, {b}")));
        }
        let a = x(rng);
        list.push(one(format!("{op} {a}, {}", cell(16))));
    }
    for op in ["movdqu", "movdqa", "movups", "movntdq"] {
        let a = x(rng);
        list.push(one(format!("{op} {}, {a}", cell(16))));
    }
    for _ in 0..2 {
        let (a, b) = (x(rng), x(rng));
        list.push(one(format!("pshufd {a}, {b}, {}", rng.random::<u8>())));
        list.push(one(format!("ptest {a}, {b}")));
        list.push(one(format!("pmovmskb {}, {a}", register(rng, 32))));
        list.push(one(format!("movd {a}, {}", register(rng, 32))));
        list.push(one(format!("movd {}, {a}", register(rng, 32))));
        list.push(one(format!("movq {a}, {}", register(rng, 64))));
        list.push(one(format!("movq {}, {a}", register(rng, 64))));
        list.push(one(format!("movq {a}, {b}")));
    }
    let a = x(rng);
    list.push(one(format!(
        "pshufd {a}, {}, {}",
        cell(16),
        rng.random::<u8>()
    )));
    list.push(one(format!("ptest {a}, {}", cell(16))));
    list.push(one(format!("movd {a}, {}", cell(4))));
    list.push(one(format!("movd {}, {a}", cell(4))));
    list.push(one(format!("movq {a}, {}", cell(8))));
    list.push(one(format!("movq {}, {a}", cell(8))));
    for op in [
        "psllw", "pslld", "psllq", "psrlw", "psrld", "psrlq", "psraw", "psrad",
    ] {
        for _ in 0..2 {
            list.push(one(format!("{op} {}, {}", x(rng), rng.random_range(0..70))));
        }
    }
    for op in ["pslldq", "psrldq"] {
        for _ in 0..2 {
            list.push(one(format!("{op} {}, {}", x(rng), rng.random_range(0..18))));
        }
    }
    // ptest's flags, read after a comparison that set them otherwise.
    for condition in ["e", "ne", "b", "ae", "a", "be"] {
        let (a, b) = (x(rng), x(rng));
        list.push(vec![
            format!("cmp {}, {}", register(rng, 64), register(rng, 64)),
            format!("ptest {a}, {b}"),
            format!("set{condition} {}", register(rng, 8)),
        ]);
    }
    // VEX, on xmm and ymm registers: the bytes above clear.
    for bytes in [16, 32] {
        let v = |rng: &mut ChaCha8Rng| vector_register(rng, bytes);
        for op in [
            "vpcmpeqb", "vpcmpeqw", "vpcmpeqd", "vpcmpeqq", "vpcmpgtb", "vpcmpgtd", "vpand",
            "vpandn", "vpor", "vpxor", "vpaddb", "vpaddd", "vpsubb", "vpsubq", "vpaddusb",
            "vpsubusb", "vpminub", "vpmaxub", "vpshufb",
        ] {
            let (a, b, c) = (v(rng), v(rng), v(rng));
            list.push(one(format!("{op} {a}, {b}, {c}")));
            list.push(one(format!("{op} {a}, {b}, {}", cell(bytes))));
        }
        let (a, b) = (v(rng), v(rng));
        list.push(one(format!("vmovdqu {a}, {b}")));
        list.push(one(format!("vmovdqu {a}, {}", cell(bytes))));
        list.push(one(format!("vmovdqu {}, {a}", cell(bytes))));
        list.push(one(format!("vmovdqa {a}, {}", cell(bytes))));
        list.push(one(format!("vpshufd {a}, {b}, {}", rng.random::<u8>())));
        list.push(one(format!("vptest {a}, {b}")));
        list.push(one(format!("vpmovmskb {}, {a}", register(rng, 32))));
        list.push(one(format!("vpsrlw {a}, {b}, {}", rng.random_range(0..20))));
        list.push(one(format!(
            "vpslldq {a}, {b}, {}",
            rng.random_range(0..18)
        )));
        for op in [
            "vpbroadcastb",
            "vpbroadcastw",
            "vpbroadcastd",
            "vpbroadcastq",
        ] {
            list.push(one(format!("{op} {a}, {}", x(rng))));
        }
        list.push(one(format!("vpbroadcastb {a}, {}", cell(1))));
        list.push(one(format!("vpbroadcastd {a}, {}", cell(4))));
    }
    list.push(one("vzeroupper".to_owned()));
    list.push(one(format!("vmovd {}, {}", x(rng), register(rng, 32))));
    list.push(one(format!("vmovq {}, {}", x(rng), register(rng, 64))));
    // EVEX, on zmm registers, with and without masks.
    let z = |rng: &mut ChaCha8Rng| vector_register(rng, 64);
    for op in ["vmovdqu8", "vmovdqu16", "vmovdqu32", "vmovdqu64"] {
        for _ in 0..2 {
            let (a, b) = (z(rng), z(rng));
            let destination = with_mask(rng, &a);
            list.push(one(format!("{op} {destination}, {b}")));
            let destination = with_mask(rng, &a);
            list.push(one(format!("{op} {destination}, {}", cell(64))));
        }
        let a = z(rng);
        list.push(one(format!(
            "{op} {} {{{}}}, {a}",
            cell(64),
            mask_register(rng)
        )));
        list.push(one(format!("{op} {}, {a}", cell(64))));
    }
    for op in [
        "vpaddb", "vpaddw", "vpaddd", "vpaddq", "vpsubb", "vpsubd", "vpandd", "vpandq", "vpandnd",
        "vpandnq", "vpord", "vporq", "vpxord", "vpxorq", "vpminub", "vpmaxub", "vpshufb",
    ] {
        for _ in 0..2 {
            let (a, b, c) = (z(rng), z(rng), z(rng));
            let destination = with_mask(rng, &a);
            list.push(one(format!("{op} {destination}, {b}, {c}")));
        }
        let (a, b) = (z(rng), z(rng));
        let destination = with_mask(rng, &a);
        list.push(one(format!("{op} {destination}, {b}, {}", cell(64))));
    }
    for op in ["vpternlogd", "vpternlogq"] {
        for _ in 0..3 {
            let (a, b, c) = (z(rng), z(rng), z(rng));
            let destination = with_mask(rng, &a);
            list.push(one(format!(
                "{op} {destination}, {b}, {c}, {}",
                rng.random::<u8>()
            )));
        }
    }
    for _ in 0..2 {
        let (a, b) = (z(rng), z(rng));
        let destination = with_mask(rng, &a);
        list.push(one(format!(
            "vpshufd {destination}, {b}, {}",
            rng.random::<u8>()
        )));
        let destination = with_mask(rng, &a);
        list.push(one(format!(
            "vpbroadcastb {destination}, {}",
            register(rng, 32)
        )));
        list.push(one(format!("vpbroadcastd {a}, {}", register(rng, 32))));
        list.push(one(format!("vpbroadcastq {a}, {}", register(rng, 64))));
        list.push(one(format!("vpbroadcastb {a}, {}", x(rng))));
    }
    // Across lanes: permutes by indices or an immediate, one table or two, and compress and
    // expand, each where this CPU has it.
    let features = crate::platform::Machine::detect().features;
    let mut permutes = vec![
        "vpermd", "vpermw", "vpermq", "vpermt2w", "vpermt2d", "vpermt2q",
    ];
    permutes.extend(["vpermi2w", "vpermi2d", "vpermi2q"]);
    if features.avx512vbmi {
        permutes.extend(["vpermb", "vpermt2b", "vpermi2b"]);
    }
    for op in permutes {
        for _ in 0..2 {
            let (a, b, c) = (z(rng), z(rng), z(rng));
            let destination = with_mask(rng, &a);
            list.push(one(format!("{op} {destination}, {b}, {c}")));
        }
        let (a, b) = (z(rng), z(rng));
        let destination = with_mask(rng, &a);
        list.push(one(format!("{op} {destination}, {b}, {}", cell(64))));
    }
    for _ in 0..3 {
        let (a, b) = (z(rng), z(rng));
        let destination = with_mask(rng, &a);
        list.push(one(format!(
            "vpermq {destination}, {b}, {}",
            rng.random::<u8>()
        )));
        let y = |rng: &mut ChaCha8Rng| vector_register(rng, 32);
        let (a, b, c) = (y(rng), y(rng), y(rng));
        list.push(one(format!(
            "vperm2i128 {a}, {b}, {c}, {}",
            rng.random::<u8>()
        )));
        list.push(one(format!("vpermd {a}, {b}, {c}")));
        list.push(one(format!("vpermq {a}, {b}, {}", rng.random::<u8>())));
    }
    let mut packs = vec!["vpcompressd", "vpcompressq", "vpexpandd", "vpexpandq"];
    if features.avx512vbmi2 {
        packs.extend(["vpcompressb", "vpcompressw", "vpexpandb", "vpexpandw"]);
    }
    for op in packs {
        for _ in 0..2 {
            let (a, b) = (z(rng), z(rng));
            let destination = with_mask(rng, &a);
            list.push(one(format!("{op} {destination}, {b}")));
        }
        if op.contains("compress") {
            let a = z(rng);
            list.push(one(format!(
                "{op} {} {{{}}}, {a}",
                cell(64),
                mask_register(rng)
            )));
        }
    }
    // Compares and tests into masks, under a mask or not.
    let k = |rng: &mut ChaCha8Rng| format!("k{}", rng.random_range(0..8));
    for op in [
        "vpcmpeqb",
        "vpcmpeqw",
        "vpcmpeqd",
        "vpcmpeqq",
        "vpcmpgtb",
        "vpcmpgtq",
        "vptestmb",
        "vptestnmb",
        "vptestmd",
        "vptestnmq",
    ] {
        for _ in 0..2 {
            let (a, b) = (z(rng), z(rng));
            let destination = match rng.random_range(0..2) {
                0 => k(rng),
                _ => format!("{} {{{}}}", k(rng), mask_register(rng)),
            };
            list.push(one(format!("{op} {destination}, {a}, {b}")));
        }
        let a = z(rng);
        list.push(one(format!("{op} {}, {a}, {}", k(rng), cell(64))));
    }
    for op in [
        "vpcmpb", "vpcmpub", "vpcmpw", "vpcmpuw", "vpcmpd", "vpcmpud", "vpcmpq", "vpcmpuq",
    ] {
        for predicate in 0..8 {
            let (a, b) = (z(rng), z(rng));
            let destination = match rng.random_range(0..2) {
                0 => k(rng),
                _ => format!("{} {{{}}}", k(rng), mask_register(rng)),
            };
            list.push(one(format!("{op} {destination}, {a}, {b}, {predicate}")));
        }
        let a = z(rng);
        list.push(one(format!(
            "{op} {}, {a}, {}, {}",
            k(rng),
            cell(64),
            rng.random_range(0..8)
        )));
    }
    for op in ["vpmovm2b", "vpmovm2w", "vpmovm2d", "vpmovm2q"] {
        list.push(one(format!("{op} {}, {}", z(rng), k(rng))));
    }
    for op in ["vpmovb2m", "vpmovw2m", "vpmovd2m", "vpmovq2m"] {
        list.push(one(format!("{op} {}, {}", k(rng), z(rng))));
    }
    // The mask registers' own instructions.
    for suffix in ["b", "w", "d", "q"] {
        let general = if suffix == "q" {
            register(rng, 64)
        } else {
            register(rng, 32)
        };
        list.push(one(format!("kmov{suffix} {}, {general}", k(rng))));
        list.push(one(format!("kmov{suffix} {general}, {}", k(rng))));
        list.push(one(format!("kmov{suffix} {}, {}", k(rng), k(rng))));
        let bytes = match suffix {
            "b" => 1,
            "w" => 2,
            "d" => 4,
            _ => 8,
        };
        list.push(one(format!("kmov{suffix} {}, {}", k(rng), cell(bytes))));
        list.push(one(format!("kmov{suffix} {}, {}", cell(bytes), k(rng))));
        for op in ["kor", "kand", "kandn", "kxor", "kxnor"] {
            list.push(one(format!(
                "{op}{suffix} {}, {}, {}",
                k(rng),
                k(rng),
                k(rng)
            )));
        }
        list.push(one(format!("knot{suffix} {}, {}", k(rng), k(rng))));
        for op in ["kshiftl", "kshiftr"] {
            list.push(one(format!(
                "{op}{suffix} {}, {}, {}",
                k(rng),
                k(rng),
                rng.random_range(0..70)
            )));
        }
        for op in ["kortest", "ktest"] {
            list.push(one(format!("{op}{suffix} {}, {}", k(rng), k(rng))));
            let condition = CONDITIONS[rng.random_range(0..CONDITIONS.len())];
            list.push(vec![
                format!("cmp {}, {}", register(rng, 64), register(rng, 64)),
                format!("{op}{suffix} {}, {}", k(rng), k(rng)),
                format!("set{condition} {}", register(rng, 8)),
            ]);
        }
    }
    list.extend(floats::probes(rng));
    list.extend(crypto::probes(rng));
    list
}

/// The harness: reads runs from standard input, gives each probe its registers, flags, vector
/// registers, masks and memory cell, and writes where each ended.
fn harness(probes: &[Vec<String>], capacity: usize) -> String {
    let mut source = String::from(
        ".intel_syntax noprefix\n.text\n.globl _start\n.type _start, @function\n_start:\n",
    );
    source.push_str(&format!(
        "    xor r12d, r12d
.Lread:
    xor eax, eax
    xor edi, edi
    lea rsi, [rip + runs]
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
    mov ecx, {INPUT}
    div rcx
    mov [rip + count], rax
    mov qword ptr [rip + position], 0
.Lnext:
    mov rcx, [rip + position]
    cmp rcx, [rip + count]
    jae .Lfinish
    imul rsi, rcx, {INPUT}
    lea rdi, [rip + runs]
    add rsi, rdi
    lea rdi, [rip + current]
    mov ecx, {}
.Lcopy_in:
    mov rax, [rsi]
    mov [rdi], rax
    add rsi, 8
    add rdi, 8
    dec ecx
    jnz .Lcopy_in
    lea rsi, [rip + current + {CELL}]
    lea rdi, [rip + cell]
    mov ecx, {}
.Lcell_in:
    mov rax, [rsi]
    mov [rdi], rax
    add rsi, 8
    add rdi, 8
    dec ecx
    jnz .Lcell_in
    lea rsp, [rip + scratch + 4096]
    push qword ptr [rip + current + {FLAGS_AT}]
    popfq
",
        capacity * INPUT,
        INPUT / 8,
        CELL_BYTES / 8
    ));
    for number in 0..8 {
        source.push_str(&format!(
            "    vmovdqu64 zmm{number}, zmmword ptr [rip + current + {}]\n    kmovq k{number}, qword ptr [rip + current + {}]\n",
            ZMMS + number * 64,
            KS + number * 8
        ));
    }
    for (number, name) in NAMES[3].iter().enumerate() {
        source.push_str(&format!(
            "    mov {name}, [rip + current + {}]\n",
            GPRS + number * 8
        ));
    }
    source.push_str(&format!(
        "    jmp qword ptr [rip + current + {ADDRESS}]\n.Lafter:\n"
    ));
    for (number, name) in NAMES[3].iter().enumerate() {
        source.push_str(&format!(
            "    mov [rip + result + {}], {name}\n",
            GPRS + number * 8
        ));
    }
    source.push_str(&format!(
        "    lea rsp, [rip + scratch + 4096]\n    pushfq\n    pop qword ptr [rip + result + {FLAGS_AT}]\n"
    ));
    for number in 0..8 {
        source.push_str(&format!(
            "    vmovdqu64 zmmword ptr [rip + result + {}], zmm{number}\n    kmovq qword ptr [rip + result + {}], k{number}\n",
            ZMMS + number * 64,
            KS + number * 8
        ));
    }
    source.push_str(&format!(
        "    lea rsi, [rip + cell]
    lea rdi, [rip + result + {CELL}]
    mov ecx, {}
.Lcell_out:
    mov rax, [rsi]
    mov [rdi], rax
    add rsi, 8
    add rdi, 8
    dec ecx
    jnz .Lcell_out
    mov rcx, [rip + position]
    imul rdi, rcx, {RESULT}
    lea rsi, [rip + results]
    add rdi, rsi
    lea rsi, [rip + result]
    mov ecx, {}
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
",
        CELL_BYTES / 8,
        RESULT / 8
    ));
    for (number, code) in probes.iter().enumerate() {
        source.push_str(&format!(".type probe{number}, @function\nprobe{number}:\n"));
        for line in code {
            source.push_str(&format!("    {line}\n"));
        }
        source.push_str(&format!(
            "    jmp .Lafter\n.size probe{number}, .-probe{number}\n"
        ));
    }
    source.push_str(".bss\n.balign 64\n");
    for (name, size) in [
        ("cell", CELL_BYTES),
        ("runs", capacity * INPUT),
        ("results", capacity * RESULT),
        ("current", INPUT),
        ("result", RESULT),
        ("position", 8),
        ("count", 8),
        ("scratch", 4096),
    ] {
        source.push_str(&format!(
            ".balign 64\n.type {name}, @object\n{name}:\n    .zero {size}\n.size {name}, {size}\n"
        ));
    }
    source
}

/// One run's input bytes: random registers and flags, vector bytes from the edges vector code
/// tests and plain random ones, masks, and the cell.
fn input(rng: &mut ChaCha8Rng, address: u64, cell_address: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; INPUT];
    for number in 0..16 {
        let value = if number == 4 {
            cell_address + CELL_BYTES as u64
        } else {
            super::value(rng)
        };
        bytes[GPRS + number * 8..GPRS + number * 8 + 8].copy_from_slice(&value.to_le_bytes());
    }
    let mut flags = 0x202_u64;
    for (bit, _) in FLAGS {
        if rng.random_bool(0.5) {
            flags |= bit;
        }
    }
    bytes[FLAGS_AT..FLAGS_AT + 8].copy_from_slice(&flags.to_le_bytes());
    let byte = |rng: &mut ChaCha8Rng| match rng.random_range(0..3) {
        0 => rng.random::<u8>(),
        1 => EDGES[rng.random_range(0..EDGES.len())],
        _ => b'a' + rng.random_range(0..26),
    };
    // Each 8 bytes a double, two singles or a mix of bytes, so float code meets its edges.
    let fill = |rng: &mut ChaCha8Rng, bytes: &mut [u8]| {
        for chunk in bytes.chunks_mut(8) {
            match floats::lane(rng) {
                Some(lane) => chunk.copy_from_slice(&lane[..chunk.len()]),
                None => {
                    for slot in chunk {
                        *slot = byte(rng);
                    }
                }
            }
        }
    };
    fill(rng, &mut bytes[ZMMS..KS]);
    for number in 0..8 {
        let mask: u64 = match rng.random_range(0..4) {
            0 => u64::MAX,
            1 => 0,
            _ => rng.random(),
        };
        bytes[KS + number * 8..KS + number * 8 + 8].copy_from_slice(&mask.to_le_bytes());
    }
    fill(rng, &mut bytes[CELL..CELL + CELL_BYTES]);
    bytes[ADDRESS..ADDRESS + 8].copy_from_slice(&address.to_le_bytes());
    bytes
}

fn word(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes"))
}

/// Runs the probe at `address` in the model on one run's input, and compares where it ended
/// with the CPU's `real` result; the differences, or why the model couldn't run it.
fn compare(
    image: &Image,
    code: &[String],
    input: &[u8],
    real: &[u8],
    cell_address: u64,
) -> Result<Vec<String>, String> {
    let mut pool = Pool::default();
    let mut state = State::start(image, &mut pool, &[], &[], Vec::new());
    for number in 0..16 {
        state.registers[number] = pool.bv(64, u128::from(word(input, GPRS + number * 8)));
    }
    let flags = word(input, FLAGS_AT);
    let bit = |pool: &mut Pool, mask: u64| pool.boolean(flags & mask != 0);
    state.carry = bit(&mut pool, 1);
    state.parity = bit(&mut pool, 1 << 2);
    state.zero = bit(&mut pool, 1 << 6);
    state.sign = bit(&mut pool, 1 << 7);
    state.overflow = bit(&mut pool, 1 << 11);
    state.last = Last::Other;
    for number in 0..8 {
        let bytes: [Term; VECTOR_BYTES] =
            std::array::from_fn(|at| pool.bv(8, u128::from(input[ZMMS + number * 64 + at])));
        state.vectors[number] = Rc::new(bytes);
        state.masks[number] = pool.bv(64, u128::from(word(input, KS + number * 8)));
    }
    for at in 0..CELL_BYTES {
        let byte = pool.bv(8, u128::from(input[CELL + at]));
        state.store(&mut pool, cell_address + at as u64, byte);
    }
    state.rip = word(input, ADDRESS);
    for _ in code {
        match step(image, &mut state, &mut pool) {
            Ok(Step::Next) => {}
            Ok(Step::Split(..) | Step::Choose(_)) => {
                return Err("the model split a run with known values".into());
            }
            Err(end) => return Err(format!("the model stopped: {end:?}")),
        }
    }
    let mut problems = Vec::new();
    let mut check = |what: String, term: Term, expected: u128| match possible(&pool, term) {
        Ok(values) if allows(values.as_ref(), expected) => {}
        Ok(values) => problems.push(format!("{what} is {values:x?}, the CPU says {expected:#x}")),
        Err(problem) => problems.push(format!("{what} {problem}")),
    };
    for (number, name) in NAMES[3].iter().enumerate() {
        let expected = u128::from(word(real, GPRS + number * 8));
        check((*name).to_owned(), state.registers[number], expected);
    }
    let real_flags = word(real, FLAGS_AT);
    for ((mask, name), flag) in FLAGS.iter().zip([
        state.carry,
        state.parity,
        state.zero,
        state.sign,
        state.overflow,
    ]) {
        check((*name).to_owned(), flag, u128::from(real_flags & mask != 0));
    }
    for number in 0..8 {
        for at in 0..VECTOR_BYTES {
            let expected = u128::from(real[ZMMS + number * 64 + at]);
            check(
                format!("zmm{number} byte {at}"),
                state.vectors[number][at],
                expected,
            );
        }
        let expected = u128::from(word(real, KS + number * 8));
        check(format!("k{number}"), state.masks[number], expected);
    }
    for at in 0..CELL_BYTES {
        let term = state
            .memory
            .get(cell_address + at as u64)
            .ok_or("the cell is gone")?;
        check(format!("cell byte {at}"), term, u128::from(real[CELL + at]));
    }
    Ok(problems)
}

#[tokio::test]
async fn every_vector_instruction_matches_this_cpu() {
    if !crate::platform::Machine::detect().features.avx512bw {
        eprintln!("this CPU has no AVX-512BW");
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
    let per_probe = 12;
    let capacity = probes.len() * per_probe;
    let folder = tempfile::tempdir().expect("a folder");
    let (executable, machine) = build(&harness(&probes, capacity), folder.path()).await;
    let image = Image::new(&machine).expect("the harness has _start");
    let cell_address = machine.data["cell"].address;
    let mut inputs = Vec::with_capacity(capacity);
    let mut stream = Vec::with_capacity(capacity * INPUT);
    for number in 0..probes.len() {
        let address = machine.functions[&format!("probe{number}")].address;
        for _ in 0..per_probe {
            let bytes = input(&mut rng, address, cell_address);
            stream.extend_from_slice(&bytes);
            inputs.push((number, bytes));
        }
    }
    let mut child = Command::new(&executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run the harness");
    let mut stdin = child.stdin.take().expect("its input");
    let writer = std::thread::spawn(move || stdin.write_all(&stream));
    let output = child.wait_with_output().expect("its results");
    writer.join().expect("the writer").expect("write the runs");
    assert!(output.status.success(), "the harness failed: {output:?}");
    assert_eq!(output.stdout.len(), capacity * RESULT, "every result");
    let mut problems = Vec::new();
    for (position, (number, bytes)) in inputs.iter().enumerate() {
        let real = &output.stdout[position * RESULT..(position + 1) * RESULT];
        let code = &probes[*number];
        match compare(&image, code, bytes, real, cell_address) {
            Ok(found) => problems.extend(
                found
                    .into_iter()
                    .map(|problem| format!("`{}`: {problem}", code.join("; "))),
            ),
            Err(problem) => problems.push(format!("`{}`: {problem}", code.join("; "))),
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
