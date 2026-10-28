//! SHA-256 digests as the hex strings that appear in locks and labels.

use std::fmt;

use sha2::{Digest, Sha256};

/// A SHA-256 digest written as 64 lowercase hex characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Sha256Hex(String);

impl Sha256Hex {
    /// Hashes `bytes`.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(hex(&Sha256::digest(bytes)))
    }

    /// Accepts exactly 64 lowercase hex characters.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let valid = text.len() == 64
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        valid.then(|| Self(text.to_owned()))
    }

    /// The 64 hex characters.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The label form used in tests.toml and main.s: `sha256:<hex>`.
    #[must_use]
    pub fn label(&self) -> String {
        format!("sha256:{}", self.0)
    }

    /// The first 12 hex characters, enough for a person to compare by eye.
    #[must_use]
    pub fn short(&self) -> &str {
        &self.0[..12]
    }
}

impl fmt::Display for Sha256Hex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Lowercase hex encoding.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_known_input() {
        assert_eq!(
            Sha256Hex::of(b"abc").as_str(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn parses_only_lowercase_hex_of_the_right_length() {
        let digest = Sha256Hex::of(b"abc");
        assert_eq!(Sha256Hex::parse(digest.as_str()), Some(digest.clone()));
        assert_eq!(Sha256Hex::parse(&digest.as_str().to_uppercase()), None);
        assert_eq!(Sha256Hex::parse(&digest.as_str()[1..]), None);
        assert_eq!(Sha256Hex::parse(&format!("{digest}0")), None);
    }

    #[test]
    fn formats_label_and_short_form() {
        let digest = Sha256Hex::of(b"abc");
        assert_eq!(digest.label(), format!("sha256:{digest}"));
        assert_eq!(digest.short(), "ba7816bf8f01");
    }
}
