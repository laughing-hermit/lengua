use std::fmt::Write as _;

const BLOCK: usize = 64;
const SCHEME: &str = "pbkdf2-sha256";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Form,
    Signature,
    Expired,
}

fn hmac(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut block = [0u8; BLOCK];
    let short;
    let key = if key.len() > BLOCK {
        short = hashes::sha256(key);
        short.as_slice()
    } else {
        key
    };
    for (place, byte) in block.iter_mut().zip(key) {
        *place = *byte;
    }
    let mut inner: Vec<u8> = block.iter().map(|b| b ^ 0x36).collect();
    inner.extend_from_slice(message);
    let mut outer: Vec<u8> = block.iter().map(|b| b ^ 0x5c).collect();
    outer.extend_from_slice(&hashes::sha256(&inner));
    debug_assert_eq!(outer.len(), BLOCK + 32);
    debug_assert!(inner.len() >= BLOCK);
    hashes::sha256(&outer)
}

#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    debug_assert_eq!(out.len(), bytes.len() * 2);
    debug_assert!(out.chars().all(|c| c.is_ascii_hexdigit()));
    out
}

#[must_use]
pub fn hmac_sha256_hex(key: &[u8], message: &[u8]) -> String {
    let text = to_hex(&hmac(key, message));
    debug_assert_eq!(text.len(), 64);
    debug_assert!(text.chars().all(|c| c.is_ascii_hexdigit()));
    text
}

fn pbkdf2(password: &[u8], salt: &[u8], rounds: u32) -> [u8; 32] {
    let mut first = salt.to_vec();
    first.extend_from_slice(&1u32.to_be_bytes());
    let mut step = hmac(password, &first);
    let mut out = step;
    for _ in 1..rounds {
        step = hmac(password, &step);
        for (place, byte) in out.iter_mut().zip(step.iter()) {
            *place ^= *byte;
        }
    }
    debug_assert_eq!(out.len(), 32);
    debug_assert_eq!(first.len(), salt.len() + 4);
    out
}

#[must_use]
pub fn hash_password(password: &str, salt: &str, rounds: u32) -> String {
    let rounds = rounds.max(1);
    let hash = to_hex(&pbkdf2(password.as_bytes(), salt.as_bytes(), rounds));
    let text = format!("{SCHEME}${rounds}${salt}${hash}");
    debug_assert!(text.starts_with(SCHEME));
    debug_assert!(text.ends_with(&hash));
    text
}

#[must_use]
pub fn same(a: &str, b: &str) -> bool {
    let mut difference = u8::from(a.len() != b.len());
    for (x, y) in a.bytes().zip(b.bytes()) {
        difference |= x ^ y;
    }
    debug_assert!(a.len() != b.len() || difference == 0 || a != b);
    debug_assert!(difference != 0 || a == b);
    difference == 0
}

#[must_use]
pub fn verify_password(password: &str, stored: &str) -> bool {
    let mut parts = stored.split('$');
    let (Some(scheme), Some(rounds), Some(salt), Some(hash), None) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return false;
    };
    let Ok(rounds) = rounds.parse::<u32>() else {
        return false;
    };
    debug_assert!(stored.len() >= scheme.len());
    debug_assert!(hash.len() <= stored.len());
    scheme == SCHEME && same(&hash_password(password, salt, rounds), stored)
}

#[must_use]
pub fn sign_token(subject: &str, expires: u64, key: &str) -> String {
    let body = format!("{subject}.{expires}");
    let signature = hmac_sha256_hex(key.as_bytes(), body.as_bytes());
    debug_assert_eq!(signature.len(), 64);
    debug_assert!(body.ends_with(&expires.to_string()));
    format!("{body}.{signature}")
}

pub fn check_token(token: &str, now: u64, key: &str) -> Result<String, Error> {
    let (body, signature) = token.rsplit_once('.').ok_or(Error::Form)?;
    let (subject, expires) = body.rsplit_once('.').ok_or(Error::Form)?;
    let expires = expires.parse::<u64>().map_err(|_| Error::Form)?;
    if subject.is_empty() {
        return Err(Error::Form);
    }
    let wanted = hmac_sha256_hex(key.as_bytes(), body.as_bytes());
    debug_assert_eq!(wanted.len(), 64);
    debug_assert!(body.len() < token.len());
    if !same(&wanted, signature) {
        return Err(Error::Signature);
    }
    if now >= expires {
        return Err(Error::Expired);
    }
    Ok(subject.to_string())
}
