//! lengua's calls into the operating system, kept apart from the rest so that the lengua crate can
//! forbid unsafe code outright: any call that needs it is wrapped here in a safe function, with a
//! test on the runner of the system it is for. On Unix, the signal a write past the file size
//! limit sends; on Windows, how the system describes the CPU's caches.
//!
//! The spikes in `tests/spikes.rs` ask the real macOS and Windows runners what the toolchain and
//! the systems do (round 1 of the roadmap in todo.md); their answers are recorded in ROADMAP.md.

/// Makes this process, and the program it becomes, ignore SIGXFSZ, the signal a write past the
/// file size limit would otherwise end it with. Ignored, the system fails the write with `EFBIG`
/// instead, after a short write up to the limit: a program sees a full file as an error it can
/// handle, as it does on every system lengua runs on, not as its own end.
///
/// An ignored signal stays ignored when the process becomes another program.
///
/// # Errors
///
/// The system's reason, when it won't change what the signal does.
#[cfg(unix)]
pub fn ignore_file_size_signal() -> Result<(), String> {
    use nix::sys::signal::{SigHandler, Signal, signal};
    // SAFETY: `SigIgn` installs no handler, so no code of ours runs in a signal's context, and
    // nothing else in this process depends on what SIGXFSZ did before.
    unsafe { signal(Signal::SIGXFSZ, SigHandler::SigIgn) }
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// One of the CPU's caches, as Windows describes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cache {
    /// Its level, from 1, the nearest.
    pub level: u8,
    /// What it holds, as Windows numbers it: 0 data and instructions, 1 instructions, 2 data, 3
    /// a trace.
    pub kind: u32,
    /// Its size, in bytes.
    pub bytes: u32,
    /// The size of its lines, in bytes.
    pub line: u16,
}

/// The relationship of the records `GetLogicalProcessorInformationEx` writes for caches.
const RELATION_CACHE: u32 = 2;

/// The caches `records` describes, as `GetLogicalProcessorInformationEx` writes them for its
/// caches (`SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX`, each `Size` bytes, with a
/// `CACHE_RELATIONSHIP` at byte 8 and its first `GROUP_AFFINITY` at byte 40): those of the first
/// logical processor, each level and kind once, the nearest first. When no record names that
/// processor, the first of each level and kind. Read by bytes, so on every system.
#[must_use]
pub fn processor_caches(records: &[u8]) -> Vec<Cache> {
    let mut found: Vec<(Cache, bool)> = Vec::new();
    let mut at = 0_usize;
    while let Some(header) = at.checked_add(8).and_then(|end| records.get(at..end)) {
        let relationship = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        let Ok(size) = usize::try_from(size) else {
            break;
        };
        if size < 8 {
            break;
        }
        if relationship == RELATION_CACHE
            && let Some(record) = at.checked_add(size).and_then(|end| records.get(at..end))
            && let Some(cache) = cache_of(record)
        {
            found.push(cache);
        }
        at = at.saturating_add(size);
    }
    let first = found.iter().any(|(_, first)| *first);
    let mut caches: Vec<Cache> = Vec::new();
    for (cache, of_first) in found {
        let known = caches
            .iter()
            .any(|kept| kept.level == cache.level && kept.kind == cache.kind);
        if (of_first || !first) && !known {
            caches.push(cache);
        }
    }
    caches.sort_by_key(|cache| (cache.level, cache.kind));
    caches
}

/// The cache one record describes, and whether its first group affinity names the first logical
/// processor: group 0, the mask's lowest bit.
fn cache_of(record: &[u8]) -> Option<(Cache, bool)> {
    let byte = |at: usize| record.get(at).copied();
    let word = |at: usize| Some(u16::from_le_bytes([byte(at)?, byte(at + 1)?]));
    let dword = |at: usize| {
        Some(u32::from_le_bytes([
            byte(at)?,
            byte(at + 1)?,
            byte(at + 2)?,
            byte(at + 3)?,
        ]))
    };
    let cache = Cache {
        level: byte(8)?,
        kind: dword(16)?,
        bytes: dword(12)?,
        line: word(10)?,
    };
    let mask = u64::from_le_bytes(record.get(40..48)?.try_into().ok()?);
    let group = word(48)?;
    Some((cache, group == 0 && mask & 1 == 1))
}

/// Windows's own calls, which only this crate makes.
#[cfg(windows)]
mod windows {
    #[link(name = "kernel32", kind = "raw-dylib")]
    unsafe extern "system" {
        /// `GetLogicalProcessorInformationEx`: the records of one relationship, written to
        /// `buffer`, which holds `length` bytes; `length` becomes how many the records take.
        #[link_name = "GetLogicalProcessorInformationEx"]
        pub(super) fn logical_processor_information(
            relationship: i32,
            buffer: *mut u8,
            length: *mut u32,
        ) -> i32;
    }
}

/// The caches of the first logical processor, as Windows describes them, each level and kind
/// once, the nearest first.
///
/// # Errors
///
/// When Windows describes no cache.
#[cfg(windows)]
pub fn caches() -> Result<Vec<Cache>, String> {
    let relationship = i32::try_from(RELATION_CACHE).unwrap_or(2);
    let mut length: u32 = 0;
    // SAFETY: a null buffer of 0 bytes only asks how many bytes the records take, which Windows
    // writes to `length`, a local, and to nothing else.
    unsafe {
        windows::logical_processor_information(relationship, std::ptr::null_mut(), &raw mut length);
    }
    if length == 0 {
        return Err("Windows describes no cache".to_owned());
    }
    let words = usize::try_from(length)
        .map_err(|error| error.to_string())?
        .div_ceil(8);
    let mut buffer = vec![0_u64; words];
    let mut written = u32::try_from(words.saturating_mul(8)).unwrap_or(u32::MAX);
    // SAFETY: `buffer` holds `written` bytes, aligned to 8 as the records' fields need, and
    // Windows writes at most `written` bytes there, then how many it wrote to `written`.
    let done = unsafe {
        windows::logical_processor_information(
            relationship,
            buffer.as_mut_ptr().cast::<u8>(),
            &raw mut written,
        )
    };
    if done == 0 {
        return Err("Windows didn't describe its caches".to_owned());
    }
    let bytes: Vec<u8> = buffer.iter().flat_map(|word| word.to_ne_bytes()).collect();
    let used = usize::try_from(written).map_or(bytes.len(), |used| used.min(bytes.len()));
    Ok(processor_caches(&bytes[..used]))
}

#[cfg(test)]
mod tests {
    use super::{Cache, processor_caches};

    /// One record of a cache, as Windows lays it out: its relationship and size, the cache's
    /// level, line size, size and kind, and its first group affinity's mask and group.
    fn record(level: u8, kind: u32, bytes: u32, mask: u64, group: u16) -> Vec<u8> {
        let mut record = vec![0_u8; 56];
        record[0..4].copy_from_slice(&2_u32.to_le_bytes());
        record[4..8].copy_from_slice(&56_u32.to_le_bytes());
        record[8] = level;
        record[9] = 8;
        record[10..12].copy_from_slice(&64_u16.to_le_bytes());
        record[12..16].copy_from_slice(&bytes.to_le_bytes());
        record[16..20].copy_from_slice(&kind.to_le_bytes());
        record[40..48].copy_from_slice(&mask.to_le_bytes());
        record[48..50].copy_from_slice(&group.to_le_bytes());
        record
    }

    /// The first logical processor's caches are kept, each level and kind once and the nearest
    /// first; another processor's are kept only when no record names the first.
    #[test]
    fn the_first_processors_caches_are_read_from_the_records() {
        let mut records = Vec::new();
        for part in [
            record(2, 0, 2 << 20, 0b11, 0),
            record(1, 2, 48 << 10, 0b10, 0),
            record(1, 2, 48 << 10, 0b01, 0),
            record(1, 1, 32 << 10, 0b01, 0),
            record(3, 0, 32 << 20, 0b01, 1),
        ] {
            records.extend(part);
        }
        let cache = |level, kind, bytes| Cache {
            level,
            kind,
            bytes,
            line: 64,
        };
        assert_eq!(
            processor_caches(&records),
            [
                cache(1, 1, 32 << 10),
                cache(1, 2, 48 << 10),
                cache(2, 0, 2 << 20)
            ]
        );
        // A record past the end, or a size below its header, ends the reading.
        let mut cut = records.clone();
        cut.truncate(60);
        assert_eq!(processor_caches(&cut), [cache(2, 0, 2 << 20)]);
        let other = record(3, 0, 32 << 20, 0b100, 1);
        assert_eq!(processor_caches(&other), [cache(3, 0, 32 << 20)]);
        assert!(processor_caches(&[0, 0, 0, 0, 0, 0, 0, 0]).is_empty());
    }

    /// Windows describes this computer's L1 data cache.
    #[cfg(windows)]
    #[test]
    fn windows_describes_its_caches() {
        let caches = super::caches().expect("Windows describes its caches");
        assert!(
            caches
                .iter()
                .any(|cache| cache.level == 1 && cache.kind == 2 && cache.bytes > 0),
            "{caches:?}"
        );
    }

    /// A process that has asked to ignore SIGXFSZ survives raising it, which ends it by default.
    #[cfg(unix)]
    #[test]
    fn a_process_that_ignores_the_file_size_signal_survives_it() {
        super::ignore_file_size_signal().expect("the signal can be ignored");
        nix::sys::signal::raise(nix::sys::signal::Signal::SIGXFSZ).expect("the signal is raised");
    }
}
