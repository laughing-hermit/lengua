#[must_use]
pub fn lines(text: &str) -> Vec<String> {
    let mut out: Vec<String> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect();
    if text.ends_with('\n') || text.is_empty() {
        out.pop();
    }
    debug_assert!(out.len() <= text.len() + 1);
    debug_assert!(out.iter().all(|line| !line.contains('\n')));
    out
}

#[must_use]
pub fn split(text: &str, separator: char) -> Vec<String> {
    let out: Vec<String> = text.split(separator).map(str::to_string).collect();
    debug_assert!(!out.is_empty());
    debug_assert!(out.iter().all(|part| !part.contains(separator)));
    out
}

#[must_use]
pub fn words(text: &str) -> Vec<String> {
    let out: Vec<String> = text.split_whitespace().map(str::to_string).collect();
    debug_assert!(out.len() <= text.len());
    debug_assert!(out.iter().all(|word| !word.is_empty()));
    out
}

#[must_use]
pub fn trim(text: &str) -> String {
    let out = text.trim_matches(is_blank).to_string();
    debug_assert!(out.len() <= text.len());
    debug_assert!(!out.starts_with(is_blank));
    out
}

#[must_use]
fn is_blank(character: char) -> bool {
    let blank = matches!(character, ' ' | '\t' | '\n' | '\r');
    debug_assert!(!blank || character.is_ascii_whitespace());
    debug_assert!(character.len_utf8() >= 1);
    blank
}

pub fn check_utf8(bytes: &[u8]) -> Result<(), usize> {
    let mut at = 0;
    for _ in 0..bytes.len() {
        let Some(first) = bytes.get(at).copied() else {
            return Ok(());
        };
        let length = sequence_length(first).ok_or(at)?;
        let rest = bytes.get(at + 1..at + length).ok_or(at)?;
        if !continues(first, rest) {
            return Err(at);
        }
        debug_assert!(length <= 4);
        at += length;
    }
    debug_assert!(at >= bytes.len());
    Ok(())
}

#[must_use]
pub fn is_utf8(bytes: &[u8]) -> bool {
    let valid = check_utf8(bytes).is_ok();
    debug_assert!(valid || !bytes.is_empty());
    debug_assert!(valid == std::str::from_utf8(bytes).is_ok());
    valid
}

fn sequence_length(first: u8) -> Option<usize> {
    let length = match first {
        0x00..=0x7f => Some(1),
        0xc2..=0xdf => Some(2),
        0xe0..=0xef => Some(3),
        0xf0..=0xf4 => Some(4),
        _ => None,
    };
    debug_assert!(length.is_none_or(|n| n <= 4));
    debug_assert!(length.is_none_or(|n| n >= 1));
    length
}

#[must_use]
fn continues(first: u8, rest: &[u8]) -> bool {
    let low = match first {
        0xe0 => 0xa0,
        0xf0 => 0x90,
        _ => 0x80,
    };
    let high = match first {
        0xed => 0x9f,
        0xf4 => 0x8f,
        _ => 0xbf,
    };
    let second = rest.first().is_none_or(|b| (low..=high).contains(b));
    let others = rest.iter().skip(1).all(|b| (0x80..=0xbf).contains(b));
    debug_assert!(rest.len() <= 3);
    debug_assert!(low <= high);
    second && others
}
