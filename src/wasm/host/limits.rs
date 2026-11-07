//! How far a WebAssembly program may go: its memory up to the size its module declares, its table
//! up to [`TABLE_ELEMENTS`], its time up to a deadline, which also bounds every call of the host
//! that waits, its output up to a cap per stream, each file up to a size and all it writes to
//! files up to a total.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use wasmtime::{ExternType, Module, StoreLimits, StoreLimitsBuilder};

use super::{Fail, Stop};
use crate::wasm::wasi::errno;

/// The most a host call waits between two looks at the deadline and at lengua's stop.
pub const SLICE: Duration = Duration::from_millis(20);

/// The most bytes a test's program writes to files in all, every write counted: past it a write
/// is cut short, and the next fails with `FBIG`, as past one file's limit.
pub const TEST_WRITTEN: u64 = 256 * 1024 * 1024;

/// The most elements a module's table may hold, whatever the module declares or grows it to: room
/// for every function a C, C++ or Rust toolchain's module calls through its table, and at 8
/// bytes an element, 8 MiB of the worker's own memory. A module that declares more never starts,
/// and a `table.grow` past it gives -1, as a `memory.grow` past the memory's limit does.
pub const TABLE_ELEMENTS: usize = 1 << 20;

/// A WebAssembly page.
const PAGE: u64 = 64 * 1024;

/// The limits of one run, and how far it has gone.
#[derive(Debug)]
pub struct Limits {
    /// When it must have ended, if it must.
    pub deadline: Option<Instant>,
    /// Set when lengua stops it, as at the end of a server's test.
    pub stop: Arc<AtomicBool>,
    /// The most bytes standard output, and standard error, may hold.
    pub max_output: usize,
    /// The most bytes a file may hold, if there's a limit.
    pub max_file: Option<u64>,
    /// The most bytes it may write to files in all, if there's a limit.
    pub max_written: Option<u64>,
    /// How many bytes it has written to files.
    pub written: u64,
    /// What the engine holds its memories and tables to.
    pub store: StoreLimits,
}

impl Limits {
    /// Limits with a deadline `timeout` from now, if any, holding memory to `memory` bytes.
    #[must_use]
    pub fn new(
        timeout: Option<Duration>,
        stop: Arc<AtomicBool>,
        max_output: usize,
        max_file: Option<u64>,
        max_written: Option<u64>,
        memory: usize,
    ) -> Self {
        Self {
            deadline: timeout.map(|timeout| Instant::now() + timeout),
            stop,
            max_output,
            max_file,
            max_written,
            written: 0,
            store: store_limits(memory),
        }
    }

    /// Whether the program may go on: not past its deadline, and not stopped.
    ///
    /// # Errors
    ///
    /// Why it may not.
    pub fn check(&self) -> Result<(), Stop> {
        if self.stop.load(Ordering::SeqCst) {
            return Err(Stop::Stopped);
        }
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(Stop::TimedOut);
        }
        Ok(())
    }

    /// How long a call that waits may wait now before it looks again: at most `most`, and never
    /// past the deadline.
    ///
    /// # Errors
    ///
    /// When the program may not go on.
    pub fn slice(&self, most: Duration) -> Result<Duration, Stop> {
        self.check()?;
        let most = most.max(Duration::from_millis(1));
        Ok(match self.deadline {
            Some(deadline) => deadline
                .saturating_duration_since(Instant::now())
                .clamp(Duration::from_millis(1), most),
            None => most,
        })
    }

    /// How many of `wanted` bytes a write at `position` of a file may write: all, fewer when it
    /// would cross the file's limit or the total, none when it is there already.
    ///
    /// # Errors
    ///
    /// `FBIG` for a write of something at a limit already.
    pub fn file_write(&self, position: u64, wanted: usize) -> Result<usize, Fail> {
        let wanted_bytes = u64::try_from(wanted).unwrap_or(u64::MAX);
        let mut room = wanted_bytes;
        if let Some(limit) = self.max_file {
            room = room.min(limit.saturating_sub(position));
        }
        if let Some(limit) = self.max_written {
            room = room.min(limit.saturating_sub(self.written));
        }
        if room == 0 && wanted > 0 {
            return Err(Fail::Errno(errno::FBIG));
        }
        Ok(usize::try_from(room).unwrap_or(wanted))
    }
}

/// What the engine holds a store to: one instance, one memory of at most `memory` bytes, one
/// table of at most [`TABLE_ELEMENTS`], and a memory or table that fails to grow past that rather
/// than trap.
#[must_use]
pub fn store_limits(memory: usize) -> StoreLimits {
    StoreLimitsBuilder::new()
        .memory_size(memory)
        .table_elements(TABLE_ELEMENTS)
        .instances(1)
        .memories(1)
        .tables(1)
        .trap_on_grow_failure(false)
        .build()
}

/// The most bytes `module`'s memory may hold: what it declares as its maximum, or its size when
/// it declares none, so a program can't take memory its module doesn't name.
#[must_use]
pub fn memory_limit(module: &Module) -> usize {
    let declared = module.exports().find_map(|export| match export.ty() {
        ExternType::Memory(memory) if export.name() == "memory" => {
            Some(memory.maximum().unwrap_or_else(|| memory.minimum()))
        }
        _ => None,
    });
    // A module without an exported memory can't reach the host; whatever memory it keeps to
    // itself is its own, up to the engine's 4 GiB.
    let pages = declared.unwrap_or(65_536);
    usize::try_from(pages.saturating_mul(PAGE)).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(max_file: Option<u64>, max_written: Option<u64>) -> Limits {
        Limits::new(
            Some(Duration::from_secs(60)),
            Arc::new(AtomicBool::new(false)),
            100,
            max_file,
            max_written,
            0,
        )
    }

    #[test]
    fn a_write_that_crosses_a_files_limit_is_cut_short_and_the_next_fails() {
        let limits = limits(Some(10), None);
        assert_eq!(limits.file_write(0, 4).ok(), Some(4));
        assert_eq!(limits.file_write(8, 4).ok(), Some(2));
        assert!(matches!(
            limits.file_write(10, 1),
            Err(Fail::Errno(errno::FBIG))
        ));
        // Nothing to write is never past a limit.
        assert_eq!(limits.file_write(10, 0).ok(), Some(0));
    }

    #[test]
    fn the_total_written_holds_across_files() {
        let mut limits = limits(Some(100), Some(10));
        limits.written = 7;
        assert_eq!(limits.file_write(0, 5).ok(), Some(3));
        limits.written = 10;
        assert!(matches!(
            limits.file_write(0, 1),
            Err(Fail::Errno(errno::FBIG))
        ));
    }

    #[test]
    fn a_stop_or_a_deadline_ends_the_run() {
        let running = limits(None, None);
        assert_eq!(running.check(), Ok(()));
        assert!(running.slice(SLICE).is_ok_and(|slice| slice <= SLICE));
        running.stop.store(true, Ordering::SeqCst);
        assert_eq!(running.check(), Err(Stop::Stopped));
        let late = Limits {
            deadline: Some(Instant::now()),
            ..limits(None, None)
        };
        assert_eq!(late.check(), Err(Stop::TimedOut));
    }
}
