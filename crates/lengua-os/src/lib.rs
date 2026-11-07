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
