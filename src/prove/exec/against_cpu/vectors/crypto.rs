//! The cipher, hash and carry-less multiply probes: every form the model runs, where this CPU
//! has it, on the harness's random registers and memory.

use rand::RngExt;
use rand::rngs::ChaCha8Rng;

use super::{cell, vector_register};

/// Every crypto probe.
pub(super) fn probes(rng: &mut ChaCha8Rng) -> Vec<Vec<String>> {
    let features = crate::platform::Machine::detect().features;
    let mut list: Vec<Vec<String>> = Vec::new();
    let one = |code: String| vec![code];
    let x = |rng: &mut ChaCha8Rng| vector_register(rng, 16);
    if features.aes {
        for op in ["aesenc", "aesenclast", "aesdec", "aesdeclast", "aesimc"] {
            for _ in 0..3 {
                list.push(one(format!("{op} {}, {}", x(rng), x(rng))));
            }
            list.push(one(format!("{op} {}, {}", x(rng), cell(16))));
        }
        for _ in 0..4 {
            let immediate = rng.random::<u8>();
            list.push(one(format!(
                "aeskeygenassist {}, {}, {immediate}",
                x(rng),
                x(rng)
            )));
        }
        for op in ["vaesenc", "vaesenclast", "vaesdec", "vaesdeclast"] {
            list.push(one(format!("{op} {}, {}, {}", x(rng), x(rng), x(rng))));
        }
        list.push(one(format!("vaesimc {}, {}", x(rng), x(rng))));
        list.push(one(format!(
            "vaeskeygenassist {}, {}, {}",
            x(rng),
            x(rng),
            rng.random::<u8>()
        )));
    }
    if features.vaes {
        for bytes in [32, 64] {
            let v = |rng: &mut ChaCha8Rng| vector_register(rng, bytes);
            for op in ["vaesenc", "vaesenclast", "vaesdec", "vaesdeclast"] {
                list.push(one(format!("{op} {}, {}, {}", v(rng), v(rng), v(rng))));
            }
        }
    }
    if features.sha {
        for _ in 0..3 {
            list.push(one(format!(
                "sha1rnds4 {}, {}, {}",
                x(rng),
                x(rng),
                rng.random_range(0..4)
            )));
        }
        for op in [
            "sha1nexte",
            "sha1msg1",
            "sha1msg2",
            "sha256rnds2",
            "sha256msg1",
            "sha256msg2",
        ] {
            for _ in 0..3 {
                list.push(one(format!("{op} {}, {}", x(rng), x(rng))));
            }
            list.push(one(format!("{op} {}, {}", x(rng), cell(16))));
        }
    }
    if features.pclmulqdq {
        for immediate in [0x00, 0x01, 0x10, 0x11] {
            list.push(one(format!(
                "pclmulqdq {}, {}, {immediate}",
                x(rng),
                x(rng)
            )));
            list.push(one(format!(
                "vpclmulqdq {}, {}, {}, {immediate}",
                x(rng),
                x(rng),
                x(rng)
            )));
        }
        list.push(one(format!("pclmulqdq {}, {}, 1", x(rng), cell(16))));
    }
    if features.vpclmulqdq {
        for bytes in [32, 64] {
            let v = |rng: &mut ChaCha8Rng| vector_register(rng, bytes);
            list.push(one(format!(
                "vpclmulqdq {}, {}, {}, {}",
                v(rng),
                v(rng),
                v(rng),
                [0x00, 0x01, 0x10, 0x11][rng.random_range(0..4)]
            )));
        }
    }
    list
}
