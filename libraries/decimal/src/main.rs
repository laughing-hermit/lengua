#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Empty,
    Invalid(usize),
    Overflow,
    TooPrecise,
}

pub fn parse_integer(text: &str) -> Result<i64, Error> {
    let (negative, digits, offset) = signed(text)?;
    let value = accumulate(digits, offset, negative, 0)?;
    debug_assert!(negative || value >= 0);
    debug_assert!(!negative || value <= 0);
    Ok(value)
}

pub fn parse_fixed(text: &str, places: u32) -> Result<i64, Error> {
    let (negative, digits, offset) = signed(text)?;
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let wanted = usize::try_from(places).unwrap_or_default();
    if fraction.len() > wanted {
        return Err(Error::TooPrecise);
    }
    let value = accumulate(whole, offset, negative, 0)?;
    let value = accumulate(fraction, offset + whole.len() + 1, negative, value)?;
    let missing = u32::try_from(wanted - fraction.len()).unwrap_or_default();
    let scaled = 10_i64
        .checked_pow(missing)
        .and_then(|factor| value.checked_mul(factor))
        .ok_or(Error::Overflow)?;
    debug_assert!(negative || scaled >= 0);
    debug_assert!(fraction.len() <= wanted);
    Ok(scaled)
}

fn signed(text: &str) -> Result<(bool, &str, usize), Error> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    if digits.is_empty() || digits == "." {
        return Err(Error::Empty);
    }
    let offset = text.len() - digits.len();
    debug_assert!(offset <= 1);
    debug_assert!(!digits.is_empty());
    Ok((negative, digits, offset))
}

fn accumulate(digits: &str, offset: usize, negative: bool, start: i64) -> Result<i64, Error> {
    let mut value = start;
    for (at, character) in digits.char_indices() {
        let digit = character
            .to_digit(10)
            .map(i64::from)
            .ok_or(Error::Invalid(offset + at))?;
        let shifted = value.checked_mul(10).ok_or(Error::Overflow)?;
        let next = if negative {
            shifted.checked_sub(digit)
        } else {
            shifted.checked_add(digit)
        };
        value = next.ok_or(Error::Overflow)?;
        debug_assert!(digit < 10);
    }
    debug_assert!(negative || value >= start);
    Ok(value)
}

#[must_use]
pub fn format_integer(value: i64) -> String {
    let text = value.to_string();
    debug_assert!(!text.is_empty());
    debug_assert!(text.starts_with('-') == (value < 0));
    text
}

#[must_use]
pub fn format_fixed(value: i64, places: u32) -> String {
    let Some(factor) = 10_u64.checked_pow(places) else {
        return format_integer(value);
    };
    let magnitude = value.unsigned_abs();
    let whole = magnitude / factor;
    let fraction = magnitude % factor;
    let sign = if value < 0 { "-" } else { "" };
    let width = usize::try_from(places).unwrap_or_default();
    let text = if places == 0 {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{fraction:0width$}")
    };
    debug_assert!(fraction < factor);
    debug_assert!(text.len() > width);
    text
}

pub fn add(left: i64, right: i64) -> Result<i64, Error> {
    let sum = left.checked_add(right).ok_or(Error::Overflow)?;
    debug_assert!(right < 0 || sum >= left);
    debug_assert!(right > 0 || sum <= left);
    Ok(sum)
}

pub fn multiply(left: i64, right: i64) -> Result<i64, Error> {
    let product = left.checked_mul(right).ok_or(Error::Overflow)?;
    debug_assert!(left == 0 || product / left == right);
    debug_assert!(right == 0 || product / right == left);
    Ok(product)
}
