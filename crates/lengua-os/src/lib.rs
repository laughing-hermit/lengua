//! lengua's calls into the operating system, kept apart from the rest so that the lengua crate can
//! forbid unsafe code outright: any call that needs it is wrapped here in a safe function, with a
//! test on the runner of the system it is for.
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

/// What a program cost that ran in a process forked from this one, as Linux counted it for that
/// process, and for any it waited for.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Forked {
    /// Its exit code, or `None` when a signal ended it.
    pub exit: Option<i32>,
    /// Its CPU time in user space, in nanoseconds.
    pub user_ns: u64,
    /// The CPU time the kernel spent for it, in nanoseconds.
    pub system_ns: u64,
    /// Its time on the clock, from just before the fork to just after it ended, in nanoseconds.
    pub wall_ns: u64,
    /// The most memory it held resident at once, in bytes. Linux begins the count at what the
    /// copy of this process that became the program held, so the count is the program's own
    /// only when this process holds less than the program does.
    pub peak_bytes: u64,
}

/// Runs the program that `words` names, its path first and then its arguments (the path is its
/// own name too), in a process forked from this one, with this process's streams, folder and
/// environment; waits for it; and gives what it cost, as the system counted it for that process.
/// SIGTERM, while it waits, stops the program with SIGKILL, and this process's end, should it come
/// first, stops it as well. The program starts with SIGPIPE as a shell gives it, and with this
/// process's signal mask.
///
/// The counts are of every child this process has waited for, and SIGCHLD and SIGTERM, which it
/// waits for, are held back in this thread alone: it runs a program only in a process of one
/// thread that has waited for no other child, as `lengua-spawn` is.
///
/// # Errors
///
/// When this process has another thread, or the system's reason, when it can't fork, wait or
/// count.
#[cfg(target_os = "linux")]
pub fn run_forked(words: &[std::ffi::CString]) -> Result<Forked, String> {
    use nix::libc;
    use nix::sys::resource::{UsageWho, getrusage};
    use nix::sys::signal::{SigHandler, SigSet, SigmaskHow, Signal, kill, signal, sigprocmask};
    use nix::sys::time::TimeVal;
    use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
    use nix::unistd::{ForkResult, fork};

    let Some(program) = words.first() else {
        return Err("no program to run".to_owned());
    };
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|error| format!("/proc/self/status: {error}"))?;
    let mut lines = status.lines();
    let threads = lines.find_map(|line| line.strip_prefix("Threads:"));
    let threads = threads.map(str::trim).unwrap_or_default();
    if threads != "1" {
        return Err(format!(
            "a program runs forked only from a process of one thread; this one has {threads:?}"
        ));
    }
    // The arguments as execv takes them, made before the fork, so that the child allocates
    // nothing: a pointer to each, then a null one.
    let mut argv: Vec<*const libc::c_char> = words.iter().map(|word| word.as_ptr()).collect();
    argv.push(std::ptr::null());
    // SIGCHLD and SIGTERM are held back until this process waits for them, so that neither is
    // lost between the fork and the wait.
    let mut waited = SigSet::empty();
    waited.add(Signal::SIGCHLD);
    waited.add(Signal::SIGTERM);
    let mut mask = SigSet::empty();
    sigprocmask(SigmaskHow::SIG_BLOCK, Some(&waited), Some(&mut mask))
        .map_err(|error| format!("sigprocmask: {error}"))?;
    let started = std::time::Instant::now();
    // SAFETY: the child, a copy of this process of one thread, holds no lock another thread took,
    // and calls only what is safe in a forked child before it becomes the program or ends: it
    // sets its signal mask, SIGPIPE and the signal its parent's end sends it, and allocates
    // nothing.
    let forked = unsafe { fork() }.map_err(|error| format!("fork: {error}"))?;
    let child = match forked {
        ForkResult::Child => {
            let _ = sigprocmask(SigmaskHow::SIG_SETMASK, Some(&mask), None);
            // SAFETY: `SigDfl` installs no handler: no code of ours runs in a signal's context.
            let _ = unsafe { signal(Signal::SIGPIPE, SigHandler::SigDfl) };
            let _ = nix::sys::prctl::set_pdeathsig(Signal::SIGKILL);
            // SAFETY: `program` and each pointer of `argv` but the last point to NUL-terminated
            // strings that `words` holds, and `argv` ends with a null pointer; `_exit` ends the
            // child at once, running nothing of this process's.
            unsafe {
                libc::execv(program.as_ptr(), argv.as_ptr());
                libc::_exit(127)
            }
        }
        ForkResult::Parent { child } => child,
    };
    let exit = loop {
        let caught = match waited.wait() {
            Ok(caught) => caught,
            Err(error) => return Err(format!("sigwait: {error}")),
        };
        if caught == Signal::SIGTERM {
            let _ = kill(child, Signal::SIGKILL);
        }
        let ended = waitpid(child, Some(WaitPidFlag::WNOHANG));
        match ended.map_err(|error| format!("waitpid: {error}"))? {
            WaitStatus::Exited(_, code) => break Some(code),
            WaitStatus::Signaled(..) => break None,
            _ => {}
        }
    };
    let wall = started.elapsed();
    let _ = sigprocmask(SigmaskHow::SIG_SETMASK, Some(&mask), None);
    let usage =
        getrusage(UsageWho::RUSAGE_CHILDREN).map_err(|error| format!("getrusage: {error}"))?;
    let nanoseconds = |time: TimeVal| {
        let seconds = u64::try_from(time.tv_sec()).unwrap_or(0);
        let microseconds = u64::try_from(time.tv_usec()).unwrap_or(0);
        let whole = seconds.saturating_mul(1_000_000_000);
        whole.saturating_add(microseconds.saturating_mul(1000))
    };
    // Linux counts the peak in KiB.
    let kib = u64::try_from(usage.max_rss()).unwrap_or(0);
    Ok(Forked {
        exit,
        user_ns: nanoseconds(usage.user_time()),
        system_ns: nanoseconds(usage.system_time()),
        wall_ns: u64::try_from(wall.as_nanos()).unwrap_or(u64::MAX),
        peak_bytes: kib.saturating_mul(1024),
    })
}

/// What a process that has ended cost, as Windows counts it for the process.
#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// Its time on the CPU in user mode, in nanoseconds.
    pub user_ns: u64,
    /// Its time on the CPU in kernel mode, in nanoseconds.
    pub kernel_ns: u64,
    /// The most memory it held resident at once, its peak working set, in bytes.
    pub peak_bytes: u64,
}

/// What `child`, a process this one started and has waited for, cost: its CPU time in user and
/// in kernel mode, from `GetProcessTimes`, and its peak working set, from
/// `K32GetProcessMemoryInfo`. Windows keeps both for as long as a handle to the process is open,
/// which `child` holds.
///
/// # Errors
///
/// The system's reason, when it won't say.
#[cfg(windows)]
pub fn process_usage(child: &std::process::Child) -> Result<Usage, String> {
    use std::os::windows::io::AsRawHandle as _;

    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetProcessTimes;

    let handle = child.as_raw_handle();
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: `handle` is the process handle `child` owns, open while `child` is borrowed, with
    // the full access CreateProcess gives it; each pointer is to a FILETIME of this frame, which
    // outlives the call.
    let timed =
        unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) };
    if timed == 0 {
        return Err(format!(
            "GetProcessTimes: {}",
            std::io::Error::last_os_error()
        ));
    }
    let size = u32::try_from(std::mem::size_of::<PROCESS_MEMORY_COUNTERS>()).unwrap_or(u32::MAX);
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: size,
        ..PROCESS_MEMORY_COUNTERS::default()
    };
    // SAFETY: as above, and `counters` is a PROCESS_MEMORY_COUNTERS of this frame, `size` bytes,
    // which the call fills.
    let read = unsafe { K32GetProcessMemoryInfo(handle, &mut counters, size) };
    if read == 0 {
        return Err(format!(
            "K32GetProcessMemoryInfo: {}",
            std::io::Error::last_os_error()
        ));
    }
    // A FILETIME counts 100-nanosecond intervals in two halves.
    let nanoseconds = |time: FILETIME| {
        let intervals = (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
        intervals.saturating_mul(100)
    };
    Ok(Usage {
        user_ns: nanoseconds(user),
        kernel_ns: nanoseconds(kernel),
        peak_bytes: u64::try_from(counters.PeakWorkingSetSize).unwrap_or(u64::MAX),
    })
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::process_usage;

    /// A process that ran and ended has spent some memory, and a CPU time Windows counts.
    #[test]
    fn a_child_that_ended_has_its_time_and_peak_memory() {
        let mut child = std::process::Command::new("cmd")
            .args(["/C", "exit", "0"])
            .spawn()
            .expect("cmd starts");
        child.wait().expect("cmd ends");
        let usage = process_usage(&child).expect("its usage");
        assert!(usage.peak_bytes > 0, "{usage:?}");
    }
}

#[cfg(all(test, target_os = "linux"))]
mod linux_tests {
    use super::run_forked;
    use std::ffi::CString;

    /// A program forked from a process of one thread ends with its own exit code. libtest runs a
    /// test on a thread of its own beside its main one, and from such a process `run_forked`
    /// refuses, saying why; tests/measure.rs runs it through lengua-spawn, of one thread.
    #[test]
    fn a_program_runs_forked_only_from_a_process_of_one_thread() {
        let words = ["/bin/sh", "-c", "exit 3"].map(|word| CString::new(word).expect("no NUL"));
        match run_forked(&words) {
            Ok(forked) => assert_eq!(forked.exit, Some(3), "{forked:?}"),
            Err(problem) => assert!(problem.contains("one thread"), "{problem}"),
        }
        assert!(run_forked(&[]).is_err());
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::ignore_file_size_signal;

    /// A process that has asked to ignore SIGXFSZ survives raising it, which ends it by default.
    #[test]
    fn a_process_that_ignores_the_file_size_signal_survives_it() {
        ignore_file_size_signal().expect("the signal can be ignored");
        nix::sys::signal::raise(nix::sys::signal::Signal::SIGXFSZ).expect("the signal is raised");
    }
}
