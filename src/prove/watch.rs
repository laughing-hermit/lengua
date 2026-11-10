//! Keeps the computer usable while proofs run. lengua starts fewer proof workers where less
//! memory is free, and a watchdog stops the solver using the most when free memory runs low, so
//! a hard proof ends as unknown, saying why, instead of the kernel's out-of-memory killer ending
//! whatever it picks, such as the session the agent runs in. And each proof worker, with the
//! solvers it starts, runs below the user's own work, so a proof takes only the cores nothing
//! else wants.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::Duration;

/// The memory a proof worker may come to use, with its solvers, before it's one too many.
const PER_WORKER: u64 = 1 << 30;

/// How often the watchdog looks.
const TICK: Duration = Duration::from_millis(250);

/// The least a solver must hold for stopping it to be worth it.
const WORTH_STOPPING: u64 = 64 << 20;

/// Why a solver the watchdog stopped stopped.
pub const LOW_MEMORY: &str = "the computer ran low on memory, so lengua stopped this proof's \
                              solver";

/// Why a proof whose own terms grew too large for the free memory stopped: there may be
/// gigabytes free still, but by the pool's estimated size not enough for its tables to double.
pub const OUT_OF_MEMORY: &str =
    "the proof's terms grew too large for the memory the computer has free, so lengua stopped it";

/// The solvers running, by process id, and those the watchdog stopped.
struct Solvers {
    running: BTreeSet<u32>,
    stopped: BTreeSet<u32>,
    watching: bool,
}

static SOLVERS: Mutex<Solvers> = Mutex::new(Solvers {
    running: BTreeSet::new(),
    stopped: BTreeSet::new(),
    watching: false,
});

/// A number of kibibytes from a `/proc` line such as `MemAvailable:  1234 kB`, as bytes.
#[cfg(target_os = "linux")]
fn field(text: &str, name: &str) -> Option<u64> {
    let line = text.lines().find(|line| line.starts_with(name))?;
    let kibibytes: u64 = line[name.len()..]
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .ok()?;
    Some(kibibytes * 1024)
}

/// The memory free for new work, and all there is, in bytes, as Linux counts them in
/// `/proc/meminfo`.
#[cfg(target_os = "linux")]
fn memory() -> Option<(u64, u64)> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    Some((field(&text, "MemAvailable:")?, field(&text, "MemTotal:")?))
}

/// The memory free for new work, and all there is, in bytes, as the system counts them: on
/// Windows the lesser of the physical memory free and the room under the commit limit, on macOS
/// the pages free and inactive.
#[cfg(not(target_os = "linux"))]
fn memory() -> Option<(u64, u64)> {
    let memory = lengua_os::memory().ok()?;
    Some((memory.available, memory.total))
}

/// The least free memory the watchdog lets proofs leave: a sixteenth of all there is, and at
/// least 512 MiB.
const fn floor(total: u64) -> u64 {
    let sixteenth = total / 16;
    if sixteenth > 512 << 20 {
        sixteenth
    } else {
        512 << 20
    }
}

/// Whether free memory is below the floor proofs leave. A proof's own terms, not only its
/// solvers, can fill the memory: a hash of the input makes them by the million, and by then the
/// kernel's out-of-memory killer would end whatever it picks.
#[must_use]
pub fn is_low() -> bool {
    is_short_for(0)
}

/// What a term takes with the entries kept for it, in bytes, as GitHub's ARM64 runner measured it:
/// 15.6 million came to about 4 GB.
pub const BYTES_PER_TERM: u64 = 256;

/// Whether free memory is below the floor proofs leave, plus twice what a pool of `bytes` holds
/// already. A pool's tables double when they fill, which takes about their size again for a
/// moment, and the proof's other workers grow theirs the same way at the same time: pools that
/// waited for the floor itself took a runner from 4.8 GB free to 0.2 GB between two samples ten
/// seconds apart, probably in such doublings. Where this system's free memory isn't read yet, no
/// pool is short, but in a test that holds pools to a worker's share (`WorkerShareForTest`).
#[must_use]
pub fn is_short_for(bytes: u64) -> bool {
    pretending()
        || memory().map_or_else(
            || past_a_worker_s_share(bytes),
            |(available, total)| available < floor(total).saturating_add(bytes.saturating_mul(2)),
        )
}

/// What a pool may hold in a test that asks for it ([`WorkerShareForTest`]), where this system's
/// free memory isn't read yet: a proof worker's share of the memory, [`PER_WORKER`]; 0 for no
/// such limit.
#[cfg(test)]
static SHARE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Whether a pool of `bytes` has no room to double within the share a test holds pools to, where
/// this system's free memory isn't read yet.
#[cfg(test)]
fn past_a_worker_s_share(bytes: u64) -> bool {
    let share = SHARE.load(std::sync::atomic::Ordering::Relaxed);
    share > 0 && bytes.saturating_mul(2) > share
}

/// Only a test holds pools to a share: elsewhere, a system whose free memory isn't read sets a
/// pool no limit.
#[cfg(not(test))]
const fn past_a_worker_s_share(_bytes: u64) -> bool {
    false
}

/// A test's limit on every pool, where this system's free memory isn't read yet (Windows and
/// macOS so far): a pool may grow while it has room to double within a proof worker's share
/// ([`PER_WORKER`]), and past that finds the memory short, as a pool finds it on Linux when free
/// memory runs low, so that a proof whose terms would fill the computer's memory stops as it
/// does there rather than end the test's process when an allocation fails. The prover's golden
/// runs under it on every runner. It holds for every thread as long as the value lasts; where
/// free memory is read, it changes nothing.
#[cfg(test)]
pub struct WorkerShareForTest;

#[cfg(test)]
impl WorkerShareForTest {
    /// Starts the limit; it lasts as long as the value returned.
    #[must_use]
    pub fn hold() -> Self {
        SHARE.store(PER_WORKER, std::sync::atomic::Ordering::Relaxed);
        Self
    }
}

#[cfg(test)]
impl Drop for WorkerShareForTest {
    fn drop(&mut self) {
        SHARE.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
thread_local! {
    /// Whether a test has made this thread see a computer with no memory left.
    static NO_MEMORY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether a test has made this thread see a computer with no memory left.
#[cfg(test)]
fn pretending() -> bool {
    NO_MEMORY.get()
}

/// Only a test pretends: a proof always looks at the computer it runs on.
#[cfg(not(test))]
const fn pretending() -> bool {
    false
}

/// A test's pretence that the computer has no memory left, for the proofs on its thread until
/// this is dropped.
#[cfg(test)]
pub struct NoMemoryForTest;

#[cfg(test)]
impl NoMemoryForTest {
    /// Starts the pretence; it lasts as long as the value returned.
    #[must_use]
    pub fn pretend() -> Self {
        NO_MEMORY.set(true);
        Self
    }
}

#[cfg(test)]
impl Drop for NoMemoryForTest {
    fn drop(&mut self) {
        NO_MEMORY.set(false);
    }
}

/// The memory a process holds, in bytes, as Linux counts it in the process's `status`.
#[cfg(target_os = "linux")]
fn resident(pid: u32) -> Option<u64> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    field(&text, "VmRSS:")
}

/// The memory a process holds, in bytes, as the system counts it: its working set on Windows,
/// its resident size on macOS.
#[cfg(not(target_os = "linux"))]
fn resident(pid: u32) -> Option<u64> {
    lengua_os::resident_bytes(pid).ok()
}

/// How far below normal a proof worker's priority goes: other work always comes first.
const PROOF_NICENESS: i32 = 10;

/// Lowers the calling thread's priority below the user's own work, and with it the solvers it
/// starts, which inherit it. Linux keeps a priority for each thread, so the server's other
/// threads, which answer the agent, keep theirs. Elsewhere a priority is the whole process's,
/// which would slow the threads that answer the agent too, so nothing is lowered there yet.
pub fn yield_to_others() {
    #[cfg(target_os = "linux")]
    let _ = rustix::process::setpriority_process(None, PROOF_NICENESS);
}

/// How many proof workers the free memory leaves room for, at most `cores`, and at least one.
#[must_use]
pub fn workers(cores: usize) -> usize {
    let room = memory().map_or(cores, |(available, _)| {
        usize::try_from(available / PER_WORKER).unwrap_or(usize::MAX)
    });
    cores.min(room).max(1)
}

/// Notes a solver that started, and starts watching if nothing is.
pub fn started(pid: u32) {
    let Ok(mut solvers) = SOLVERS.lock() else {
        return;
    };
    solvers.running.insert(pid);
    if !solvers.watching {
        solvers.watching = true;
        std::thread::spawn(watch);
    }
}

/// Notes a solver that ended, and says whether the watchdog stopped it.
pub fn ended(pid: u32) -> bool {
    SOLVERS.lock().is_ok_and(|mut solvers| {
        solvers.running.remove(&pid);
        solvers.stopped.remove(&pid)
    })
}

/// Whether the watchdog stopped the solver with this process id.
#[must_use]
pub fn was_stopped(pid: u32) -> bool {
    SOLVERS
        .lock()
        .is_ok_and(|solvers| solvers.stopped.contains(&pid))
}

/// Stops the running solver that holds the most memory, if it holds enough to matter; whether
/// one was stopped.
fn stop_largest() -> bool {
    let Ok(mut solvers) = SOLVERS.lock() else {
        return false;
    };
    let largest = solvers
        .running
        .iter()
        .filter(|pid| !solvers.stopped.contains(pid))
        .filter_map(|pid| Some((resident(*pid)?, *pid)))
        .max();
    match largest {
        Some((held, pid)) if held >= WORTH_STOPPING => {
            tracing::warn!(
                pid,
                held,
                "free memory ran low: stopping the solver using the most"
            );
            stop(&mut solvers, pid)
        }
        _ => false,
    }
}

/// Stops a running solver, marking it stopped; whether it was.
fn stop(solvers: &mut Solvers, pid: u32) -> bool {
    let killed = kill(pid);
    if killed {
        solvers.stopped.insert(pid);
    }
    killed
}

/// Kills the process with this number; whether it did.
#[cfg(unix)]
fn kill(pid: u32) -> bool {
    let Ok(raw) = i32::try_from(pid) else {
        return false;
    };
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(raw), nix::sys::signal::SIGKILL).is_ok()
}

/// Kills the process with this number; whether it did. Windows has no signal to send by number, so
/// `taskkill` ends it at once.
#[cfg(not(unix))]
fn kill(pid: u32) -> bool {
    std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Stops the solver with this process id as the watchdog would, for tests.
#[cfg(test)]
pub fn stop_for_test(pid: u32) -> bool {
    SOLVERS
        .lock()
        .is_ok_and(|mut solvers| stop(&mut solvers, pid))
}

/// The watchdog: while solvers run, stops the largest whenever free memory is below the floor.
fn watch() {
    loop {
        std::thread::sleep(TICK);
        if let Some((available, total)) = memory()
            && available < floor(total)
        {
            stop_largest();
        }
        let Ok(mut solvers) = SOLVERS.lock() else {
            return;
        };
        if solvers.running.is_empty() {
            solvers.watching = false;
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn reads_memory_from_proc() {
        let text = "MemTotal:       28350444 kB\nMemFree:  1 kB\nMemAvailable:   24855000 kB\n";
        assert_eq!(field(text, "MemAvailable:"), Some(24_855_000 * 1024));
        assert_eq!(field(text, "MemTotal:"), Some(28_350_444 * 1024));
        assert_eq!(field(text, "Swap:"), None);
    }

    /// Every system lengua runs on says what memory it has and has free, so the floor and the
    /// count of workers hold on each.
    #[test]
    fn reads_this_computers_memory() {
        let (available, total) = memory().expect("this computer's memory");
        assert!(
            available > 0 && available <= total,
            "{available} of {total}"
        );
        assert_eq!(floor(8 << 30), 512 << 20);
        assert_eq!(floor(32 << 30), 2 << 30);
        assert!(workers(16) >= 1);
        assert_eq!(workers(1), 1);
    }

    /// Where this system's free memory isn't read, a test that holds pools to a worker's share
    /// finds a pool that couldn't double within it short, and one that could not; without the
    /// limit, no pool is short there. (Where free memory is read, the free memory decides, as
    /// the next test shows.)
    #[test]
    fn a_test_holds_pools_to_a_worker_s_share_where_memory_is_unread() {
        if memory().is_some() {
            return;
        }
        let (small, large) = (PER_WORKER / 4, PER_WORKER);
        assert!(!is_short_for(large));
        let share = WorkerShareForTest::hold();
        assert!(is_short_for(large) && !is_short_for(small));
        assert!(!is_low());
        drop(share);
        assert!(!is_short_for(large));
    }

    /// A pool needs the floor and room to double twice over: none has room for a pool the size of
    /// the address space, and a test can make a thread see a computer with no memory at all.
    #[test]
    fn a_pool_that_may_double_needs_room_for_it() {
        assert!(memory().is_some(), "this computer's memory");
        assert!(is_short_for(u64::MAX));
        let pretence = NoMemoryForTest::pretend();
        assert!(is_low() && is_short_for(0));
        drop(pretence);
    }

    /// A proof worker yields: its priority drops below normal, and a solver it starts inherits it.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_proof_worker_yields_to_other_work() {
        std::thread::spawn(|| {
            yield_to_others();
            assert_eq!(
                rustix::process::getpriority_process(None).expect("a priority"),
                PROOF_NICENESS
            );
            let child = std::process::Command::new("sleep")
                .arg("5")
                .spawn()
                .expect("a process");
            let pid = rustix::process::Pid::from_raw(i32::try_from(child.id()).expect("a pid"));
            let niceness = rustix::process::getpriority_process(pid).expect("its priority");
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            assert_eq!(niceness, PROOF_NICENESS);
        })
        .join()
        .expect("the thread");
    }

    /// A solver the watchdog stops ends at once (on Linux and macOS by SIGKILL), and is known to
    /// have been stopped; while it runs, the system says what memory it holds. This stops its own
    /// process by number, never "the largest", which another test's solver could be.
    #[test]
    fn a_stopped_solver_is_known_to_be() {
        // A program that waits for input it never gets, so it runs until it is stopped.
        let waits = if cfg!(windows) { "cmd" } else { "cat" };
        let mut child = std::process::Command::new(waits)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("a process");
        let pid = child.id();
        started(pid);
        assert!(!was_stopped(pid));
        assert!(resident(pid).is_some_and(|held| held > 0));
        assert!(
            SOLVERS
                .lock()
                .is_ok_and(|mut solvers| stop(&mut solvers, pid))
        );
        assert!(was_stopped(pid));
        let status = child.wait().expect("it ends");
        assert!(!status.success(), "{status:?}");
        #[cfg(unix)]
        assert_eq!(
            std::os::unix::process::ExitStatusExt::signal(&status),
            Some(9)
        );
        assert!(ended(pid), "the watchdog stopped it");
        assert!(!was_stopped(pid));
    }
}
