//! Temporary, for W19's measurements: where a shell run's time and memory go. With
//! `LENGUA_STAGES` set, each [`mark`] notes the time on the clock, the process's CPU time, its
//! page faults, its peak and its resident memory, and [`dump`] writes them to standard error,
//! with what each mapping of the process holds. Without it, a mark does nothing.

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// One mark.
struct Mark {
    /// Its name.
    name: &'static str,
    /// The thread that made it.
    thread: String,
    /// Nanoseconds since the first mark.
    wall_ns: u128,
    /// The process's CPU time, in microseconds.
    cpu_us: i64,
    /// Its minor page faults.
    faults: i64,
    /// Its peak resident memory, in KiB.
    peak_kib: i64,
    /// Its resident memory, in KiB.
    resident_kib: u64,
}

/// Whether marks are kept, the first mark's time, and the marks.
fn kept() -> Option<&'static (Instant, Mutex<Vec<Mark>>)> {
    static KEPT: OnceLock<Option<(Instant, Mutex<Vec<Mark>>)>> = OnceLock::new();
    KEPT.get_or_init(|| {
        std::env::var_os("LENGUA_STAGES").map(|_| (Instant::now(), Mutex::new(Vec::new())))
    })
    .as_ref()
}

/// Whether the variant `name` is asked for in `LENGUA_VARIANT`, a list with commas between.
#[must_use]
pub fn variant(name: &str) -> bool {
    static ASKED: OnceLock<Vec<String>> = OnceLock::new();
    ASKED
        .get_or_init(|| {
            std::env::var("LENGUA_VARIANT")
                .map(|list| list.split(',').map(str::to_owned).collect())
                .unwrap_or_default()
        })
        .iter()
        .any(|asked| asked == name)
}

/// The process's resident memory now, in KiB, on Linux; 0 elsewhere.
fn resident_kib() -> u64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|text| {
            text.split_whitespace()
                .nth(1)
                .and_then(|pages| pages.parse::<u64>().ok())
        })
        .map_or(0, |pages| pages * 4)
}

/// Notes where the process stands, as `name`.
#[allow(clippy::useless_conversion)]
pub fn mark(name: &'static str) {
    let Some((start, marks)) = kept() else {
        return;
    };
    let wall_ns = start.elapsed().as_nanos();
    #[cfg(unix)]
    let (cpu_us, faults, peak_kib) = {
        use nix::sys::resource::{UsageWho, getrusage};
        getrusage(UsageWho::RUSAGE_SELF).map_or((0, 0, 0), |usage| {
            let user = usage.user_time();
            let system = usage.system_time();
            (
                (user.tv_sec() + system.tv_sec()) * 1_000_000
                    + i64::from(user.tv_usec())
                    + i64::from(system.tv_usec()),
                usage.minor_page_faults(),
                usage.max_rss(),
            )
        })
    };
    #[cfg(not(unix))]
    let (cpu_us, faults, peak_kib) = (0, 0, 0);
    let made = Mark {
        name,
        thread: std::thread::current().name().unwrap_or("?").to_owned(),
        wall_ns,
        cpu_us,
        faults,
        peak_kib,
        resident_kib: resident_kib(),
    };
    if let Ok(mut marks) = marks.lock() {
        marks.push(made);
    }
}

/// Writes every mark to standard error, then what each mapping holds.
pub fn dump() {
    let Some((_, marks)) = kept() else {
        return;
    };
    mark("dump");
    if let Ok(marks) = marks.lock() {
        for mark in marks.iter() {
            eprintln!(
                "STAGE {} thread={} wall_ns={} cpu_us={} faults={} peak_kib={} rss_kib={}",
                mark.name,
                mark.thread,
                mark.wall_ns,
                mark.cpu_us,
                mark.faults,
                mark.peak_kib,
                mark.resident_kib
            );
        }
    }
    if std::env::var("LENGUA_STAGES").is_ok_and(|value| value == "maps")
        && let Ok(maps) = std::fs::read_to_string("/proc/self/smaps")
    {
        let mut name = String::new();
        let mut size = String::new();
        for line in maps.lines() {
            let mut words = line.split_whitespace();
            let first = words.next().unwrap_or_default();
            if first.contains('-') && !first.ends_with(':') {
                let perms = words.next().unwrap_or_default();
                let path = words.nth(3).unwrap_or("[anon]");
                name = format!("{perms} {path}");
            } else if first == "Size:" {
                size = words.next().unwrap_or_default().to_owned();
            } else if first == "Rss:" {
                let rss = words.next().unwrap_or_default();
                if rss != "0" {
                    eprintln!("MAP rss_kib={rss} size_kib={size} {name}");
                }
            }
        }
    }
}
