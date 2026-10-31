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

/// The memory free for new work, and all there is, in bytes.
fn memory() -> Option<(u64, u64)> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    Some((field(&text, "MemAvailable:")?, field(&text, "MemTotal:")?))
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

/// The memory a process holds, in bytes.
fn resident(pid: u32) -> Option<u64> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    field(&text, "VmRSS:")
}

/// How far below normal a proof worker's priority goes: other work always comes first.
const PROOF_NICENESS: i32 = 10;

/// Lowers the calling thread's priority below the user's own work, and with it the solvers it
/// starts, which inherit it. Linux keeps a priority for each thread, so the server's other
/// threads, which answer the agent, keep theirs.
pub fn yield_to_others() {
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
    let Ok(raw) = i32::try_from(pid) else {
        return false;
    };
    let killed =
        nix::sys::signal::kill(nix::unistd::Pid::from_raw(raw), nix::sys::signal::SIGKILL).is_ok();
    if killed {
        solvers.stopped.insert(pid);
    }
    killed
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

    #[test]
    fn reads_memory_from_proc() {
        let text = "MemTotal:       28350444 kB\nMemFree:  1 kB\nMemAvailable:   24855000 kB\n";
        assert_eq!(field(text, "MemAvailable:"), Some(24_855_000 * 1024));
        assert_eq!(field(text, "MemTotal:"), Some(28_350_444 * 1024));
        assert_eq!(field(text, "Swap:"), None);
        let (available, total) = memory().expect("this computer's memory");
        assert!(available <= total);
        assert_eq!(floor(8 << 30), 512 << 20);
        assert_eq!(floor(32 << 30), 2 << 30);
        assert!(workers(16) >= 1);
        assert_eq!(workers(1), 1);
    }

    /// A proof worker yields: its priority drops below normal, and a solver it starts inherits it.
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

    /// A solver the watchdog stops ends by SIGKILL, and is known to have been stopped. This
    /// stops its own process by number, never "the largest", which another test's solver
    /// could be.
    #[test]
    fn a_stopped_solver_is_known_to_be() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("a process");
        let pid = child.id();
        started(pid);
        assert!(!was_stopped(pid));
        assert!(resident(pid).is_some());
        assert!(
            SOLVERS
                .lock()
                .is_ok_and(|mut solvers| stop(&mut solvers, pid))
        );
        assert!(was_stopped(pid));
        let status = child.wait().expect("it ends");
        assert_eq!(
            std::os::unix::process::ExitStatusExt::signal(&status),
            Some(9)
        );
        assert!(ended(pid), "the watchdog stopped it");
        assert!(!was_stopped(pid));
    }
}
