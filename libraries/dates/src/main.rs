#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Form(usize),
    Month,
    Day,
    Time,
}

const DAY_SECONDS: i64 = 86_400;

#[must_use]
pub fn is_leap(year: i32) -> bool {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    debug_assert!(!leap || year % 4 == 0);
    debug_assert!(year % 400 != 0 || leap);
    leap
}

#[must_use]
pub fn days_in_month(year: i32, month: u8) -> u8 {
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    };
    debug_assert!(days <= 31);
    debug_assert!(days == 0 || days >= 28);
    days
}

#[must_use]
pub fn days_from_civil(date: Date) -> i64 {
    let year = i64::from(date.year) - i64::from(date.month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(date.month);
    let shifted = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted + 2) / 5 + i64::from(date.day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    debug_assert!((0..400).contains(&year_of_era));
    debug_assert!((0..146_097).contains(&day_of_era));
    era * 146_097 + day_of_era - 719_468
}

#[must_use]
pub fn civil_from_days(days: i64) -> Date {
    let shifted_days = days + 719_468;
    let era = shifted_days.div_euclid(146_097);
    let day_of_era = shifted_days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted + 2) / 5 + 1;
    let month = if shifted < 10 { shifted + 3 } else { shifted - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    debug_assert!((1..=12).contains(&month));
    debug_assert!((1..=31).contains(&day));
    Date {
        year: i32::try_from(year).unwrap_or_default(),
        month: u8::try_from(month).unwrap_or_default(),
        day: u8::try_from(day).unwrap_or_default(),
    }
}

#[must_use]
pub fn days_between(from: Date, to: Date) -> i64 {
    let days = days_from_civil(to) - days_from_civil(from);
    debug_assert_eq!(days >= 0, to >= from);
    debug_assert!(days.abs() < i64::from(i32::MAX) * 400);
    days
}

fn number(text: &str, from: usize, to: usize) -> u16 {
    let found = text
        .get(from..to)
        .and_then(|part| part.parse::<u16>().ok())
        .unwrap_or_default();
    debug_assert!(to > from);
    debug_assert!(to - from <= 4);
    found
}

fn form(text: &str, length: usize, separators: &[(usize, u8)]) -> Result<(), Error> {
    for (at, byte) in text.bytes().enumerate() {
        let wanted = separators.iter().find(|(place, _)| *place == at).map(|(_, b)| *b);
        let fits = match wanted {
            Some(separator) => byte == separator,
            None => byte.is_ascii_digit(),
        };
        if !fits || at >= length {
            return Err(Error::Form(at));
        }
    }
    debug_assert!(separators.iter().all(|(place, _)| *place < length));
    debug_assert!(length > 0);
    if text.len() < length {
        return Err(Error::Form(text.len()));
    }
    Ok(())
}

pub fn parse_date(text: &str) -> Result<Date, Error> {
    form(text, 10, &[(4, b'-'), (7, b'-')])?;
    let year = i32::from(number(text, 0, 4));
    let month = u8::try_from(number(text, 5, 7)).unwrap_or_default();
    let day = u8::try_from(number(text, 8, 10)).unwrap_or_default();
    if !(1..=12).contains(&month) {
        return Err(Error::Month);
    }
    if day == 0 || day > days_in_month(year, month) {
        return Err(Error::Day);
    }
    debug_assert!(year <= 9999);
    debug_assert!(day <= 31);
    Ok(Date { year, month, day })
}

#[must_use]
pub fn format_date(date: Date) -> String {
    let text = format!("{:04}-{:02}-{:02}", date.year, date.month, date.day);
    debug_assert!(text.len() >= 10);
    debug_assert!(text.contains('-'));
    text
}

pub fn parse_timestamp(text: &str) -> Result<i64, Error> {
    let date = parse_date(text.get(..10).unwrap_or(text))?;
    let clock = text.get(10..).unwrap_or_default();
    form(clock, 10, &[(0, b'T'), (3, b':'), (6, b':'), (9, b'Z')]).map_err(|error| match error {
        Error::Form(at) => Error::Form(at + 10),
        other => other,
    })?;
    let hour = i64::from(number(clock, 1, 3));
    let minute = i64::from(number(clock, 4, 6));
    let second = i64::from(number(clock, 7, 9));
    if hour > 23 || minute > 59 || second > 59 {
        return Err(Error::Time);
    }
    debug_assert_eq!(clock.len(), 10);
    debug_assert!(hour < 24);
    Ok(days_from_civil(date) * DAY_SECONDS + hour * 3600 + minute * 60 + second)
}

#[must_use]
pub fn format_timestamp(seconds: i64) -> String {
    let date = civil_from_days(seconds.div_euclid(DAY_SECONDS));
    let rest = seconds.rem_euclid(DAY_SECONDS);
    let text = format!(
        "{}T{:02}:{:02}:{:02}Z",
        format_date(date),
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    );
    debug_assert!(rest < DAY_SECONDS);
    debug_assert!(text.ends_with('Z'));
    text
}
