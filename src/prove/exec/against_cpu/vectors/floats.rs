//! The floating-point probes: every float instruction the model runs, in each encoding and
//! operand kind, with lanes of float values that float code meets.

use rand::RngExt;
use rand::rngs::ChaCha8Rng;

use super::super::register;
use super::{cell, mask_register, vector_register, with_mask};

/// Doubles at the edges: zeros of both signs, ties for rounding, the ends of the 32- and 64-bit
/// whole numbers, the largest and smallest, infinities, and NaNs quiet, signaling and default.
const DOUBLES: [u64; 24] = [
    0x0000_0000_0000_0000,
    0x8000_0000_0000_0000,
    0x3FF0_0000_0000_0000,
    0xBFF0_0000_0000_0000,
    0x3FE0_0000_0000_0000,
    0x3FF8_0000_0000_0000,
    0x4004_0000_0000_0000,
    0xC004_0000_0000_0000,
    0x4023_0000_0000_0000,
    0x41DF_FFFF_FFE0_0000,
    0x41E0_0000_0000_0000,
    0xC1E0_0000_0000_0000,
    0x43E0_0000_0000_0000,
    0xC3E0_0000_0000_0000,
    0x7FEF_FFFF_FFFF_FFFF,
    0x0010_0000_0000_0000,
    0x0000_0000_0000_0001,
    0x7FF0_0000_0000_0000,
    0xFFF0_0000_0000_0000,
    0x7FF8_0000_0000_0000,
    0xFFF8_0000_0000_0000,
    0x7FF0_0000_0000_0001,
    0x7FF4_0000_0000_00AB,
    0x4415_AF1D_78B5_8C40,
];

/// The same edges for singles.
const SINGLES: [u32; 22] = [
    0x0000_0000,
    0x8000_0000,
    0x3F80_0000,
    0xBF80_0000,
    0x3F00_0000,
    0x3FC0_0000,
    0x4020_0000,
    0xC020_0000,
    0x4F00_0000,
    0xCF00_0000,
    0x4EFF_FFFF,
    0x5F00_0000,
    0x7F7F_FFFF,
    0x0080_0000,
    0x0000_0001,
    0x7F80_0000,
    0xFF80_0000,
    0x7FC0_0000,
    0xFFC0_0000,
    0x7F80_0001,
    0x7FA0_00AB,
    0x4B00_0001,
];

/// A double that's ordinary: a small whole number, a number with a fraction, or any magnitude.
fn ordinary_double(rng: &mut ChaCha8Rng) -> u64 {
    match rng.random_range(0..3) {
        0 => f64::from(rng.random_range(-1000_i32..1000)).to_bits(),
        1 => (f64::from(rng.random_range(-4000_i32..4000)) / 8.0).to_bits(),
        _ => {
            let sign = u64::from(rng.random_bool(0.5)) << 63;
            let exponent = rng.random_range(1023_u64 - 70..1023 + 70) << 52;
            sign | exponent | (rng.random::<u64>() >> 12)
        }
    }
}

fn ordinary_single(rng: &mut ChaCha8Rng) -> u32 {
    #[expect(clippy::cast_possible_truncation, reason = "a single's bits")]
    let narrow = |double: f64| (double as f32).to_bits();
    match rng.random_range(0..3) {
        0 => narrow(f64::from(rng.random_range(-1000_i32..1000))),
        1 => narrow(f64::from(rng.random_range(-4000_i32..4000)) / 8.0),
        _ => {
            let sign = u32::from(rng.random_bool(0.5)) << 31;
            let exponent = rng.random_range(127_u32 - 40..127 + 40) << 23;
            sign | exponent | (rng.random::<u32>() >> 9)
        }
    }
}

/// Eight bytes of a vector: a double, two singles, or `None` to leave them to the byte mix.
pub(super) fn lane(rng: &mut ChaCha8Rng) -> Option<[u8; 8]> {
    let single = |rng: &mut ChaCha8Rng| {
        if rng.random_bool(0.4) {
            SINGLES[rng.random_range(0..SINGLES.len())]
        } else {
            ordinary_single(rng)
        }
    };
    match rng.random_range(0..4) {
        0 => {
            let bits = if rng.random_bool(0.4) {
                DOUBLES[rng.random_range(0..DOUBLES.len())]
            } else {
                ordinary_double(rng)
            };
            Some(bits.to_le_bytes())
        }
        1 => {
            let (low, high) = (single(rng), single(rng));
            Some((u64::from(high) << 32 | u64::from(low)).to_le_bytes())
        }
        _ => None,
    }
}

/// Every float probe.
#[expect(clippy::too_many_lines, reason = "a flat list of instruction forms")]
pub(super) fn probes(rng: &mut ChaCha8Rng) -> Vec<Vec<String>> {
    let mut list: Vec<Vec<String>> = Vec::new();
    let one = |code: String| vec![code];
    let x = |rng: &mut ChaCha8Rng| vector_register(rng, 16);
    // Legacy SSE: the upper bytes stay.
    for (op, element) in [
        ("addsd", 8),
        ("subsd", 8),
        ("mulsd", 8),
        ("divsd", 8),
        ("minsd", 8),
        ("maxsd", 8),
        ("sqrtsd", 8),
        ("addss", 4),
        ("subss", 4),
        ("mulss", 4),
        ("divss", 4),
        ("minss", 4),
        ("maxss", 4),
        ("sqrtss", 4),
        ("ucomisd", 8),
        ("comisd", 8),
        ("ucomiss", 4),
        ("comiss", 4),
        ("cvtsd2ss", 8),
        ("cvtss2sd", 4),
        ("movsd", 8),
        ("movss", 4),
    ] {
        for _ in 0..2 {
            list.push(one(format!("{op} {}, {}", x(rng), x(rng))));
        }
        list.push(one(format!("{op} {}, {}", x(rng), cell(element))));
    }
    for op in [
        "addpd",
        "subpd",
        "mulpd",
        "divpd",
        "minpd",
        "maxpd",
        "sqrtpd",
        "addps",
        "subps",
        "mulps",
        "divps",
        "minps",
        "maxps",
        "sqrtps",
        "andpd",
        "orpd",
        "xorpd",
        "andnpd",
        "cvtpd2dq",
        "cvttpd2dq",
        "cvtdq2ps",
        "cvtps2dq",
        "cvttps2dq",
        "cvtpd2ps",
        "unpcklpd",
        "unpckhpd",
        "unpcklps",
        "unpckhps",
        "haddpd",
        "haddps",
        "hsubpd",
        "hsubps",
    ] {
        for _ in 0..2 {
            list.push(one(format!("{op} {}, {}", x(rng), x(rng))));
        }
        list.push(one(format!("{op} {}, {}", x(rng), cell(16))));
    }
    for op in ["cvtdq2pd", "cvtps2pd"] {
        list.push(one(format!("{op} {}, {}", x(rng), x(rng))));
        list.push(one(format!("{op} {}, {}", x(rng), cell(8))));
    }
    for op in ["movhlps", "movlhps"] {
        list.push(one(format!("{op} {}, {}", x(rng), x(rng))));
    }
    for (op, element) in [
        ("roundsd", 8),
        ("roundss", 4),
        ("roundpd", 16),
        ("roundps", 16),
    ] {
        for _ in 0..3 {
            let immediate = rng.random_range(0..16);
            list.push(one(format!("{op} {}, {}, {immediate}", x(rng), x(rng))));
        }
        let immediate = rng.random_range(0..16);
        list.push(one(format!(
            "{op} {}, {}, {immediate}",
            x(rng),
            cell(element)
        )));
    }
    for (op, element) in [("cmpsd", 8), ("cmpss", 4), ("cmppd", 16), ("cmpps", 16)] {
        for predicate in 0..8 {
            list.push(one(format!("{op} {}, {}, {predicate}", x(rng), x(rng))));
        }
        let predicate = rng.random_range(0..8);
        list.push(one(format!(
            "{op} {}, {}, {predicate}",
            x(rng),
            cell(element)
        )));
    }
    for op in ["shufpd", "shufps"] {
        for _ in 0..2 {
            list.push(one(format!(
                "{op} {}, {}, {}",
                x(rng),
                x(rng),
                rng.random::<u8>()
            )));
        }
    }
    for op in ["cvtsi2sd", "cvtsi2ss"] {
        for width in [32, 64] {
            list.push(one(format!("{op} {}, {}", x(rng), register(rng, width))));
        }
        list.push(one(format!("{op} {}, {}", x(rng), cell(4))));
        list.push(one(format!("{op} {}, {}", x(rng), cell(8))));
    }
    for (op, element) in [
        ("cvtsd2si", 8),
        ("cvttsd2si", 8),
        ("cvtss2si", 4),
        ("cvttss2si", 4),
    ] {
        for width in [32, 64] {
            list.push(one(format!("{op} {}, {}", register(rng, width), x(rng))));
            list.push(one(format!(
                "{op} {}, {}",
                register(rng, width),
                cell(element)
            )));
        }
    }
    for op in ["movsd", "movss"] {
        let element = if op == "movsd" { 8 } else { 4 };
        list.push(one(format!("{op} {}, {}", cell(element), x(rng))));
    }
    for op in ["movmskpd", "movmskps"] {
        list.push(one(format!("{op} {}, {}", register(rng, 32), x(rng))));
    }
    // ucomisd's flags, read after a comparison that set them otherwise.
    for condition in ["e", "ne", "b", "ae", "a", "be", "p", "np"] {
        list.push(vec![
            format!("cmp {}, {}", register(rng, 64), register(rng, 64)),
            format!("ucomisd {}, {}", x(rng), x(rng)),
            format!("set{condition} {}", register(rng, 8)),
        ]);
    }
    // VEX scalar, on xmm registers: the rest of the low 16 bytes from the middle operand, and
    // the bytes above clear.
    for (op, element) in [
        ("vaddsd", 8),
        ("vsubsd", 8),
        ("vmulsd", 8),
        ("vdivsd", 8),
        ("vminsd", 8),
        ("vmaxsd", 8),
        ("vsqrtsd", 8),
        ("vaddss", 4),
        ("vmulss", 4),
        ("vdivss", 4),
        ("vmaxss", 4),
        ("vsqrtss", 4),
        ("vcvtsd2ss", 8),
        ("vcvtss2sd", 4),
        ("vmovsd", 8),
        ("vmovss", 4),
        ("vfmadd132sd", 8),
        ("vfmadd213sd", 8),
        ("vfmadd231sd", 8),
        ("vfmadd132ss", 4),
        ("vfmadd213ss", 4),
        ("vfmadd231ss", 4),
    ] {
        for _ in 0..2 {
            list.push(one(format!("{op} {}, {}, {}", x(rng), x(rng), x(rng))));
        }
        if !op.starts_with("vmov") {
            list.push(one(format!(
                "{op} {}, {}, {}",
                x(rng),
                x(rng),
                cell(element)
            )));
        }
    }
    for op in ["vmovsd", "vmovss"] {
        let element = if op == "vmovsd" { 8 } else { 4 };
        list.push(one(format!("{op} {}, {}", x(rng), cell(element))));
        list.push(one(format!("{op} {}, {}", cell(element), x(rng))));
    }
    for op in ["vucomisd", "vcomiss"] {
        list.push(one(format!("{op} {}, {}", x(rng), x(rng))));
    }
    for op in ["vroundsd", "vroundss"] {
        list.push(one(format!(
            "{op} {}, {}, {}, {}",
            x(rng),
            x(rng),
            x(rng),
            rng.random_range(0..16)
        )));
    }
    for op in ["vcmpsd", "vcmpss"] {
        for _ in 0..4 {
            list.push(one(format!(
                "{op} {}, {}, {}, {}",
                x(rng),
                x(rng),
                x(rng),
                rng.random_range(0..32)
            )));
        }
    }
    for op in ["vcvtsi2sd", "vcvtsi2ss"] {
        list.push(one(format!(
            "{op} {}, {}, {}",
            x(rng),
            x(rng),
            register(rng, 64)
        )));
        list.push(one(format!(
            "{op} {}, {}, {}",
            x(rng),
            x(rng),
            register(rng, 32)
        )));
    }
    for op in ["vcvtsd2si", "vcvttss2si"] {
        list.push(one(format!("{op} {}, {}", register(rng, 64), x(rng))));
    }
    // VEX packed, on xmm and ymm registers.
    for bytes in [16, 32] {
        let v = |rng: &mut ChaCha8Rng| vector_register(rng, bytes);
        for op in [
            "vaddpd",
            "vsubpd",
            "vmulpd",
            "vdivpd",
            "vminpd",
            "vmaxpd",
            "vaddps",
            "vmulps",
            "vdivps",
            "vminps",
            "vandpd",
            "vxorpd",
            "vandnpd",
            "vorps",
            "vunpcklpd",
            "vunpckhps",
            "vhaddpd",
            "vhaddps",
            "vhsubpd",
            "vfmadd132pd",
            "vfmadd213pd",
            "vfmadd231pd",
            "vfmadd132ps",
            "vfmadd213ps",
            "vfmadd231ps",
        ] {
            list.push(one(format!("{op} {}, {}, {}", v(rng), v(rng), v(rng))));
            list.push(one(format!("{op} {}, {}, {}", v(rng), v(rng), cell(bytes))));
        }
        for op in ["vsqrtpd", "vsqrtps", "vcvtdq2ps", "vcvtps2dq", "vcvttps2dq"] {
            list.push(one(format!("{op} {}, {}", v(rng), v(rng))));
            list.push(one(format!("{op} {}, {}", v(rng), cell(bytes))));
        }
        for op in ["vroundpd", "vroundps"] {
            list.push(one(format!(
                "{op} {}, {}, {}",
                v(rng),
                v(rng),
                rng.random_range(0..16)
            )));
        }
        for op in ["vcmppd", "vcmpps"] {
            for _ in 0..3 {
                list.push(one(format!(
                    "{op} {}, {}, {}, {}",
                    v(rng),
                    v(rng),
                    v(rng),
                    rng.random_range(0..32)
                )));
            }
        }
        for op in ["vshufpd", "vshufps"] {
            list.push(one(format!(
                "{op} {}, {}, {}, {}",
                v(rng),
                v(rng),
                v(rng),
                rng.random::<u8>()
            )));
        }
        // Widening takes half the destination's bytes, narrowing gives half the source's.
        if bytes == 32 {
            let (half, wide) = (x(rng), v(rng));
            list.push(one(format!("vcvtdq2pd {wide}, {half}")));
            list.push(one(format!("vcvtps2pd {wide}, {half}")));
            list.push(one(format!("vcvtpd2dq {}, {wide}", x(rng))));
            list.push(one(format!("vcvttpd2dq {}, {wide}", x(rng))));
            list.push(one(format!("vcvtpd2ps {}, {wide}", x(rng))));
            list.push(one(format!("vbroadcastsd {wide}, {}", x(rng))));
            list.push(one(format!("vbroadcastsd {wide}, {}", cell(8))));
        }
        list.push(one(format!("vbroadcastss {}, {}", v(rng), x(rng))));
        list.push(one(format!("vbroadcastss {}, {}", v(rng), cell(4))));
        list.push(one(format!("vmovmskpd {}, {}", register(rng, 32), v(rng))));
        list.push(one(format!("vmovmskps {}, {}", register(rng, 32), v(rng))));
    }
    list.push(one(format!("vmovhlps {}, {}, {}", x(rng), x(rng), x(rng))));
    list.push(one(format!("vmovlhps {}, {}, {}", x(rng), x(rng), x(rng))));
    // EVEX, on zmm registers and masked xmm ones.
    let z = |rng: &mut ChaCha8Rng| vector_register(rng, 64);
    for op in [
        "vaddpd",
        "vmulps",
        "vdivpd",
        "vmaxps",
        "vandpd",
        "vxorps",
        "vandnps",
        "vunpcklps",
        "vshufps",
        "vfmadd231pd",
        "vfmadd132ps",
    ] {
        for _ in 0..2 {
            let (a, b, c) = (z(rng), z(rng), z(rng));
            let destination = with_mask(rng, &a);
            let immediate = if op == "vshufps" {
                format!(", {}", rng.random::<u8>())
            } else {
                String::new()
            };
            list.push(one(format!("{op} {destination}, {b}, {c}{immediate}")));
        }
        if op != "vshufps" {
            let (a, b) = (z(rng), z(rng));
            let destination = with_mask(rng, &a);
            list.push(one(format!("{op} {destination}, {b}, {}", cell(64))));
        }
    }
    for op in ["vsqrtpd", "vcvtdq2ps", "vcvttps2dq"] {
        let (a, b) = (z(rng), z(rng));
        let destination = with_mask(rng, &a);
        list.push(one(format!("{op} {destination}, {b}")));
    }
    let y = |rng: &mut ChaCha8Rng| vector_register(rng, 32);
    for _ in 0..2 {
        let register = z(rng);
        let destination = with_mask(rng, &register);
        list.push(one(format!("vcvtdq2pd {destination}, {}", y(rng))));
        let register = y(rng);
        let destination = with_mask(rng, &register);
        list.push(one(format!("vcvtpd2ps {destination}, {}", z(rng))));
        let register = x(rng);
        let destination = with_mask(rng, &register);
        list.push(one(format!("vaddsd {destination}, {}, {}", x(rng), x(rng))));
        let register = x(rng);
        let destination = with_mask(rng, &register);
        list.push(one(format!(
            "vfmadd213ss {destination}, {}, {}",
            x(rng),
            x(rng)
        )));
        let register = x(rng);
        let destination = with_mask(rng, &register);
        list.push(one(format!("vmovsd {destination}, {}", cell(8))));
        list.push(one(format!(
            "vmovsd {} {{{}}}, {}",
            cell(8),
            mask_register(rng),
            x(rng)
        )));
    }
    let k = |rng: &mut ChaCha8Rng| format!("k{}", rng.random_range(0..8));
    for op in ["vcmppd", "vcmpps"] {
        for _ in 0..3 {
            let destination = match rng.random_range(0..2) {
                0 => k(rng),
                _ => format!("{} {{{}}}", k(rng), mask_register(rng)),
            };
            list.push(one(format!(
                "{op} {destination}, {}, {}, {}",
                z(rng),
                z(rng),
                rng.random_range(0..32)
            )));
        }
    }
    list.push(one(format!(
        "vcmpsd {}, {}, {}, {}",
        k(rng),
        x(rng),
        x(rng),
        rng.random_range(0..32)
    )));
    list
}
