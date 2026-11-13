use std::fmt::Write as _;

const ROUNDS: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

const START: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

const POLYNOMIAL: u32 = 0xedb8_8320;

#[must_use]
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let message = padded(data);
    debug_assert_eq!(message.len() % 64, 0);
    let mut state = START;
    for block in message.chunks_exact(64) {
        compress(&mut state, block);
    }
    let mut out = [0u8; 32];
    for (slot, word) in out.chunks_exact_mut(4).zip(state) {
        slot.copy_from_slice(&word.to_be_bytes());
    }
    debug_assert_eq!(out.len(), 32);
    out
}

#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    let mut text = String::new();
    for byte in sha256(data) {
        let _ = write!(text, "{byte:02x}");
    }
    debug_assert_eq!(text.len(), 64);
    debug_assert!(text.chars().all(|c| c.is_ascii_hexdigit()));
    text
}

fn padded(data: &[u8]) -> Vec<u8> {
    let mut message = data.to_vec();
    message.push(0x80);
    let zeros = (119 - data.len() % 64) % 64;
    message.resize(message.len() + zeros, 0);
    let bits = u64::try_from(data.len())
        .unwrap_or(u64::MAX)
        .wrapping_mul(8);
    message.extend_from_slice(&bits.to_be_bytes());
    debug_assert_eq!(message.len() % 64, 0);
    debug_assert!(message.len() >= data.len() + 9);
    message
}

fn schedule(block: &[u8]) -> [u32; 64] {
    let mut words = [0u32; 64];
    for (slot, bytes) in words.iter_mut().zip(block.chunks_exact(4)) {
        *slot = <[u8; 4]>::try_from(bytes)
            .map(u32::from_be_bytes)
            .unwrap_or_default();
    }
    for at in 16..words.len() {
        let far = words.get(at - 15).copied().unwrap_or_default();
        let near = words.get(at - 2).copied().unwrap_or_default();
        let oldest = words.get(at - 16).copied().unwrap_or_default();
        let middle = words.get(at - 7).copied().unwrap_or_default();
        let low = far.rotate_right(7) ^ far.rotate_right(18) ^ (far >> 3);
        let high = near.rotate_right(17) ^ near.rotate_right(19) ^ (near >> 10);
        if let Some(slot) = words.get_mut(at) {
            *slot = oldest
                .wrapping_add(low)
                .wrapping_add(middle)
                .wrapping_add(high);
        }
    }
    debug_assert_eq!(block.len(), 64);
    debug_assert_eq!(words.len(), 64);
    words
}

fn compress(state: &mut [u32; 8], block: &[u8]) {
    let mut work = *state;
    for (round, word) in ROUNDS.iter().zip(schedule(block)) {
        let [first, second, third, fourth, fifth, sixth, seventh, eighth] = work;
        let mixed = fifth.rotate_right(6) ^ fifth.rotate_right(11) ^ fifth.rotate_right(25);
        let choice = (fifth & sixth) ^ (!fifth & seventh);
        let one = eighth
            .wrapping_add(mixed)
            .wrapping_add(choice)
            .wrapping_add(*round)
            .wrapping_add(word);
        let spread = first.rotate_right(2) ^ first.rotate_right(13) ^ first.rotate_right(22);
        let majority = (first & second) ^ (first & third) ^ (second & third);
        let two = spread.wrapping_add(majority);
        work = [
            one.wrapping_add(two),
            first,
            second,
            third,
            fourth.wrapping_add(one),
            fifth,
            sixth,
            seventh,
        ];
    }
    for (slot, add) in state.iter_mut().zip(work) {
        *slot = slot.wrapping_add(add);
    }
    debug_assert_eq!(block.len(), 64);
    debug_assert_eq!(ROUNDS.len(), 64);
}

#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            debug_assert!(mask == 0 || mask == u32::MAX);
            crc = (crc >> 1) ^ (POLYNOMIAL & mask);
        }
    }
    debug_assert!(data.len() < usize::MAX);
    !crc
}
