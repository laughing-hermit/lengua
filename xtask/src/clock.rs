//! Dates in UTC, from the system's clock, with no crate: the days since 1970 turned into the
//! calendar's year, month and day by Howard Hinnant's `civil_from_days`.

use std::time::{SystemTime, UNIX_EPOCH};

/// The days of the week, from Sunday, as `date` names them.
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// The months, as `date` names them.
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// A moment in UTC, to the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Civil {
    /// The year.
    pub year: u64,
    /// The month, 1 to 12.
    pub month: u64,
    /// The day of the month, 1 to 31.
    pub day: u64,
    /// The hour, 0 to 23.
    pub hour: u64,
    /// The minute, 0 to 59.
    pub minute: u64,
    /// The second, 0 to 59.
    pub second: u64,
    /// The day of the week, 0 for Sunday.
    pub weekday: u64,
}

impl Civil {
    /// The moment `seconds` after 1970-01-01 00:00:00 UTC.
    pub fn from_unix(seconds: u64) -> Self {
        let days = seconds / 86_400;
        let time = seconds % 86_400;
        // civil_from_days, for days since 1970, none of which comes before its era.
        let shifted = days + 719_468;
        let era = shifted / 146_097;
        let of_era = shifted - era * 146_097;
        let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
        let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let from_march = (5 * of_year + 2) / 153;
        let day = of_year - (153 * from_march + 2) / 5 + 1;
        let month = if from_march < 10 {
            from_march + 3
        } else {
            from_march - 9
        };
        Self {
            year: year_of_era + era * 400 + u64::from(month <= 2),
            month,
            day,
            hour: time / 3600,
            minute: time % 3600 / 60,
            second: time % 60,
            // 1970-01-01 was a Thursday.
            weekday: (days + 4) % 7,
        }
    }

    /// Now, by the system's clock.
    pub fn now() -> Self {
        let since = SystemTime::now().duration_since(UNIX_EPOCH);
        Self::from_unix(since.map_or(0, |since| since.as_secs()))
    }

    /// The moment as `date -u` prints it: `Mon Nov  9 20:26:00 UTC 2026`.
    pub fn date(&self) -> String {
        format!(
            "{} {} {:>2} {:02}:{:02}:{:02} UTC {}",
            WEEKDAYS[self.weekday as usize % 7],
            MONTHS[(self.month as usize + 11) % 12],
            self.day,
            self.hour,
            self.minute,
            self.second,
            self.year
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Moments with known dates: the epoch, a leap day, and a Monday in November.
    #[test]
    fn known_moments_have_their_dates() {
        assert_eq!(Civil::from_unix(0).date(), "Thu Jan  1 00:00:00 UTC 1970");
        assert_eq!(
            Civil::from_unix(951_782_400).date(),
            "Tue Feb 29 00:00:00 UTC 2000"
        );
        let monday = Civil::from_unix(1_794_255_960);
        assert_eq!(monday.date(), "Mon Nov  9 20:26:00 UTC 2026");
        assert_eq!((monday.year, monday.month, monday.day), (2026, 11, 9));
        assert_eq!(
            Civil::from_unix(1_798_761_599).date(),
            "Thu Dec 31 23:59:59 UTC 2026"
        );
    }
}
