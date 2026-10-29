//! Values a test gives a program in place of the kernel's: the bytes its `getrandom` calls
//! receive and what its `clock_gettime` calls read. They make output that depends on chance or
//! the time exact, so tests can compare it and proofs can cover it.

use std::fmt::Write as _;

/// The most random bytes one test may supply.
pub const MAX_RANDOM_BYTES: usize = 65_536;

/// The most clock readings one test may supply.
pub const MAX_CLOCK_READINGS: usize = 1024;

/// A clock reading: whole seconds since 1970 and nanoseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Time {
    /// Seconds since 1970-01-01 00:00:00 UTC.
    pub seconds: i64,
    /// Nanoseconds past the second, below 1,000,000,000.
    pub nanoseconds: u32,
}

impl Time {
    /// Reads a reading written in seconds, such as "1735689600" or "1735689600.25", with up to
    /// nine decimal places and never before 1970.
    ///
    /// # Errors
    ///
    /// Says how to write it.
    pub fn parse(text: &str) -> Result<Self, String> {
        let bad = || {
            format!(
                "clock reading {text:?} must be seconds since 1970, such as \"1735689600\" or \
                 \"1735689600.25\" (up to 9 decimal places)"
            )
        };
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        let digits =
            |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
        if !digits(whole)
            || (text.contains('.') && !digits(fraction))
            || fraction.len() > 9
            || (whole.len() > 1 && whole.starts_with('0'))
        {
            return Err(bad());
        }
        let seconds = whole.parse().map_err(|_| bad())?;
        let nanoseconds = if fraction.is_empty() {
            0
        } else {
            format!("{fraction:0<9}").parse().map_err(|_| bad())?
        };
        Ok(Self {
            seconds,
            nanoseconds,
        })
    }
}

/// Reads bytes written as pairs of hex digits, such as "2a00ff".
///
/// # Errors
///
/// Says how to write them.
pub fn hex(text: &str) -> Result<Vec<u8>, String> {
    let bad = || {
        format!(
            "random = {text:?} must be bytes written as pairs of hex digits, such as \"2a00ff\""
        )
    };
    if !text.len().is_multiple_of(2) || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(bad());
    }
    let bytes: Vec<u8> = (0..text.len())
        .step_by(2)
        .map(|start| u8::from_str_radix(&text[start..start + 2], 16).map_err(|_| bad()))
        .collect::<Result<_, _>>()?;
    if bytes.len() > MAX_RANDOM_BYTES {
        return Err(format!(
            "random holds {} bytes; a test supplies at most {MAX_RANDOM_BYTES}",
            bytes.len()
        ));
    }
    Ok(bytes)
}

/// What a test supplies in place of the kernel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Supply {
    /// The bytes `getrandom` calls receive, in order, or `None` when the kernel draws them.
    pub random: Option<Vec<u8>>,
    /// What successive `clock_gettime` calls read, the last one repeating; empty when the
    /// kernel's clock answers.
    pub clock: Vec<Time>,
}

impl Supply {
    /// The supply a test writes as `random` hex and `clock` readings.
    ///
    /// # Errors
    ///
    /// Says what's wrong with either.
    pub fn parse(random: Option<&str>, clock: &[String]) -> Result<Self, String> {
        if clock.len() > MAX_CLOCK_READINGS {
            return Err(format!(
                "clock holds {} readings; a test supplies at most {MAX_CLOCK_READINGS}",
                clock.len()
            ));
        }
        Ok(Self {
            random: random.map(hex).transpose()?,
            clock: clock
                .iter()
                .map(|text| Time::parse(text))
                .collect::<Result<_, _>>()?,
        })
    }

    /// Whether the test supplies anything, so the run needs the supervisor.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.random.is_none() && self.clock.is_empty()
    }

    /// The supply as the supervisor reads it on its standard input: a line with the random
    /// bytes in hex (or `-` for none), then one `seconds nanoseconds` line per reading.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut text = match &self.random {
            None => "-".to_owned(),
            Some(bytes) => bytes.iter().fold(String::new(), |mut text, byte| {
                let _ = write!(text, "{byte:02x}");
                text
            }),
        };
        text.push('\n');
        for time in &self.clock {
            let _ = writeln!(text, "{} {}", time.seconds, time.nanoseconds);
        }
        text
    }

    /// Reads [`Self::encode`]'s text.
    ///
    /// # Errors
    ///
    /// When the text isn't in that form.
    pub fn decode(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let random = match lines.next() {
            None => return Err("no random line".into()),
            Some("-") => None,
            Some(bytes) => Some(hex(bytes)?),
        };
        let clock = lines
            .map(|line| {
                let (seconds, nanoseconds) = line
                    .split_once(' ')
                    .ok_or_else(|| format!("{line:?} isn't a clock reading"))?;
                Ok(Time {
                    seconds: seconds.parse().map_err(|_| format!("{line:?}"))?,
                    nanoseconds: nanoseconds
                        .parse()
                        .ok()
                        .filter(|nanoseconds| *nanoseconds < 1_000_000_000)
                        .ok_or_else(|| format!("{line:?}"))?,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self { random, clock })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_clock_readings_in_seconds() {
        assert_eq!(
            Time::parse("1735689600.25"),
            Ok(Time {
                seconds: 1_735_689_600,
                nanoseconds: 250_000_000
            })
        );
        assert_eq!(
            Time::parse("0"),
            Ok(Time {
                seconds: 0,
                nanoseconds: 0
            })
        );
        for bad in ["", "-1", "1.", ".5", "01", "1.0000000001", "1e9", " 1"] {
            assert!(Time::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn reads_hex_bytes() {
        assert_eq!(hex("2a00FF"), Ok(vec![0x2a, 0, 0xff]));
        assert_eq!(hex(""), Ok(Vec::new()));
        for bad in ["2", "2g", "0x2a", "2a "] {
            assert!(hex(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn travels_to_the_supervisor_and_back() {
        let supply =
            Supply::parse(Some("2a00"), &["5".into(), "7.000000001".into()]).expect("a supply");
        assert_eq!(supply.encode(), "2a00\n5 0\n7 1\n");
        assert_eq!(Supply::decode(&supply.encode()), Ok(supply));
        let clock_only = Supply::parse(None, &["5".into()]).expect("a supply");
        assert_eq!(Supply::decode(&clock_only.encode()), Ok(clock_only));
        assert!(Supply::default().is_empty());
    }
}
