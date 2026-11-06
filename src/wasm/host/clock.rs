//! The clock: the time of day and a clock that only moves forward, each rounded down to a
//! microsecond, so the clock can't carry what finer timing would (OpenAI's 2026 report of agents
//! signalling through the CPU cache's timing). In a test that supplies readings, every call reads
//! the next, the last repeating.

use std::time::{Instant, SystemTime};

use super::{Fail, Host, memory};
use crate::supply::Time;
use crate::wasm::wasi::{clock, errno};

/// The clocks' resolution, in nanoseconds: one microsecond.
pub const RESOLUTION: u64 = 1000;

/// A run's clocks.
#[derive(Debug)]
pub struct Clock {
    /// When the run started: the forward clock's zero.
    started: Instant,
    /// The readings a test supplies, if it does.
    readings: Vec<Time>,
    /// Which reading the next call gets.
    next: usize,
}

impl Clock {
    /// The clocks of a run, reading `readings` when a test supplies them.
    #[must_use]
    pub fn new(readings: Vec<Time>) -> Self {
        Self {
            started: Instant::now(),
            readings,
            next: 0,
        }
    }

    /// The clock `id`'s reading now, in nanoseconds, rounded down to a microsecond: a test's next
    /// supplied reading, when it supplies them.
    ///
    /// # Errors
    ///
    /// `INVAL` for a clock WASI doesn't have or lengua doesn't give.
    pub fn now(&mut self, id: u32) -> Result<u64, u16> {
        let reading = self.peek(id)?;
        if !self.readings.is_empty() {
            self.next = self.next.saturating_add(1);
        }
        Ok(reading)
    }

    /// What [`Self::now`] would read, without moving on to a test's next reading.
    ///
    /// # Errors
    ///
    /// `INVAL` for a clock WASI doesn't have or lengua doesn't give.
    pub fn peek(&self, id: u32) -> Result<u64, u16> {
        if id != clock::REALTIME && id != clock::MONOTONIC {
            return Err(errno::INVAL);
        }
        let nanoseconds = if let Some(last) = self.readings.len().checked_sub(1) {
            let time = self.readings[self.next.min(last)];
            u64::try_from(time.seconds)
                .unwrap_or(0)
                .saturating_mul(1_000_000_000)
                .saturating_add(u64::from(time.nanoseconds))
        } else if id == clock::REALTIME {
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |since| {
                    u64::try_from(since.as_nanos()).unwrap_or(u64::MAX)
                })
        } else {
            u64::try_from(self.started.elapsed().as_nanos()).unwrap_or(u64::MAX)
        };
        Ok(nanoseconds - nanoseconds % RESOLUTION)
    }
}

/// `clock_time_get(id, precision, out)`.
///
/// # Errors
///
/// A buffer outside memory.
pub fn clock_time_get(host: &mut Host, memory: &mut [u8], id: u32, out: u32) -> Result<(), Fail> {
    memory::check(memory, out, 8)?;
    let now = host.clock.now(id).map_err(Fail::Errno)?;
    memory::write_u64(memory, out, now)
}

/// `clock_res_get(id, out)`.
///
/// # Errors
///
/// A buffer outside memory.
pub fn clock_res_get(memory: &mut [u8], id: u32, out: u32) -> Result<(), Fail> {
    if id != clock::REALTIME && id != clock::MONOTONIC {
        return Err(Fail::Errno(errno::INVAL));
    }
    memory::write_u64(memory, out, RESOLUTION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readings_are_rounded_down_to_a_microsecond() {
        let mut clock = Clock::new(Vec::new());
        let now = clock.now(clock::REALTIME).expect("the time of day");
        assert_eq!(now % RESOLUTION, 0);
        let forward = clock.now(clock::MONOTONIC).expect("the forward clock");
        assert_eq!(forward % RESOLUTION, 0);
        assert_eq!(clock.now(2), Err(errno::INVAL));
    }

    #[test]
    fn a_test_supplies_the_readings_the_last_repeating() {
        let mut clock = Clock::new(vec![
            Time {
                seconds: 1,
                nanoseconds: 999_999_999,
            },
            Time {
                seconds: 2,
                nanoseconds: 0,
            },
        ]);
        assert_eq!(clock.now(clock::REALTIME), Ok(1_999_999_000));
        assert_eq!(clock.now(clock::MONOTONIC), Ok(2_000_000_000));
        assert_eq!(clock.now(clock::REALTIME), Ok(2_000_000_000));
    }
}
