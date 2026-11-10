//! lengua's calls into the operating system, kept apart from the rest so that the lengua crate can
//! forbid unsafe code outright: any call that needs it is wrapped here in a safe function, with a
//! test on the runner of the system it is for. On Linux, a program run in a process forked from
//! this one, with what the system counted of its cost; on Windows, how the system describes the
//! CPU's caches, and what a child process cost; on Windows and macOS, the memory the computer has
//! free and in all, and what a process holds of it.

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
    Some((cache, group == 0 && (mask & 1) == 1))
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

/// The computer's memory, as its system counts it.
#[cfg(any(windows, target_os = "macos"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Memory {
    /// What new work can still take, in bytes. On Windows, the physical memory free or the
    /// room left under the commit limit, whichever is less: an allocation fails at either. On
    /// macOS, the pages free and those inactive, which the system gives back when asked.
    pub available: u64,
    /// The physical memory the computer has, in bytes.
    pub total: u64,
}

/// The computer's memory, free and in all, from `GlobalMemoryStatusEx`.
///
/// # Errors
///
/// The system's reason, when it won't say.
#[cfg(windows)]
pub fn memory() -> Result<Memory, String> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let size = u32::try_from(std::mem::size_of::<MEMORYSTATUSEX>()).unwrap_or(u32::MAX);
    let mut status = MEMORYSTATUSEX {
        dwLength: size,
        ..MEMORYSTATUSEX::default()
    };
    // SAFETY: `status` is a MEMORYSTATUSEX of this frame, which outlives the call, and its
    // `dwLength` says its size, as the call needs; the call fills it and nothing else.
    let read = unsafe { GlobalMemoryStatusEx(&raw mut status) };
    if read == 0 {
        return Err(format!(
            "GlobalMemoryStatusEx: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(Memory {
        available: status.ullAvailPhys.min(status.ullAvailPageFile),
        total: status.ullTotalPhys,
    })
}

/// The memory the process with this number holds resident, its working set, in bytes, from
/// `K32GetProcessMemoryInfo`.
///
/// # Errors
///
/// The system's reason, when it won't open the process or say: no process has the number, or it
/// is another user's.
#[cfg(windows)]
pub fn resident_bytes(pid: u32) -> Result<u64, String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
    };

    // SAFETY: the call takes no pointer; it gives a new handle, or null.
    let handle =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, 0, pid) };
    if handle.is_null() {
        return Err(format!("OpenProcess: {}", std::io::Error::last_os_error()));
    }
    let size = u32::try_from(std::mem::size_of::<PROCESS_MEMORY_COUNTERS>()).unwrap_or(u32::MAX);
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: size,
        ..PROCESS_MEMORY_COUNTERS::default()
    };
    // SAFETY: `handle` is the open handle above, with the access the call needs, and `counters`
    // is a PROCESS_MEMORY_COUNTERS of this frame, `size` bytes, which the call fills.
    let read = unsafe { K32GetProcessMemoryInfo(handle, &raw mut counters, size) };
    let problem = std::io::Error::last_os_error();
    // SAFETY: `handle` is the handle opened above, which nothing else holds, closed once.
    unsafe {
        CloseHandle(handle);
    }
    if read == 0 {
        return Err(format!("K32GetProcessMemoryInfo: {problem}"));
    }
    Ok(u64::try_from(counters.WorkingSetSize).unwrap_or(u64::MAX))
}

/// macOS's own calls, which only this crate makes.
#[cfg(target_os = "macos")]
mod macos {
    unsafe extern "C" {
        /// `mach_host_self`: a port that names this computer to the kernel's counters.
        pub(super) fn mach_host_self() -> u32;
        /// `host_statistics64`: the counters of one flavor, written to `info` as 32-bit
        /// integers, at most `count` of them; `count` becomes how many it wrote.
        pub(super) fn host_statistics64(
            host: u32,
            flavor: i32,
            info: *mut u32,
            count: *mut u32,
        ) -> i32;
    }
}

/// The flavor of `host_statistics64` that counts the memory's pages (`HOST_VM_INFO64`).
#[cfg(target_os = "macos")]
const HOST_VM_INFO64: i32 = 4;

/// How many 32-bit integers the page counts take (`vm_statistics64`, as macOS has written it
/// since 10.9: `HOST_VM_INFO64_REV1_COUNT`). Its first is the pages free and its third the
/// pages inactive.
#[cfg(target_os = "macos")]
const VM_INFO_INTEGERS: usize = 38;

/// The computer's memory, free and in all: the kernel's page counts from `host_statistics64`,
/// and the page's size and the count of physical pages from `sysconf`.
///
/// # Errors
///
/// When the system won't say.
#[cfg(target_os = "macos")]
pub fn memory() -> Result<Memory, String> {
    use nix::libc;

    /// The port that names this computer, asked for once: each ask takes a reference to it.
    static HOST: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    let host = *HOST.get_or_init(|| {
        // SAFETY: the call takes nothing and gives a port's name, a number.
        unsafe { macos::mach_host_self() }
    });
    let mut counts = [0_u32; VM_INFO_INTEGERS];
    let mut written = u32::try_from(VM_INFO_INTEGERS).unwrap_or(0);
    // SAFETY: `counts` holds `written` 32-bit integers, and the kernel writes at most that many
    // there, then how many it wrote to `written`; both are of this frame and outlive the call.
    let status = unsafe {
        macos::host_statistics64(host, HOST_VM_INFO64, counts.as_mut_ptr(), &raw mut written)
    };
    if status != 0 {
        return Err(format!("host_statistics64: kern_return_t {status}"));
    }
    // SAFETY: `sysconf` takes a number and gives one; it reads no memory of ours.
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    // SAFETY: as above.
    let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
    let (Ok(page), Ok(pages)) = (u64::try_from(page), u64::try_from(pages)) else {
        return Err(format!("sysconf: {}", std::io::Error::last_os_error()));
    };
    // The kernel counts pages in unsigned 32-bit integers.
    let count = |at: usize| u64::from(counts[at]);
    Ok(Memory {
        available: count(0).saturating_add(count(2)).saturating_mul(page),
        total: pages.saturating_mul(page),
    })
}

/// The flavor of `proc_pid_rusage` that gives the first of its records (`RUSAGE_INFO_V0`).
#[cfg(target_os = "macos")]
const RUSAGE_INFO_V0: i32 = 0;

/// The memory the process with this number holds resident, in bytes, from `proc_pid_rusage`.
///
/// # Errors
///
/// The system's reason, when it won't say: no process has the number, or it is another user's.
#[cfg(target_os = "macos")]
pub fn resident_bytes(pid: u32) -> Result<u64, String> {
    use nix::libc;

    let pid = i32::try_from(pid).map_err(|error| error.to_string())?;
    // `rusage_info_v0` is 96 bytes: 16 of an identifier, then ten 64-bit counts, of which the
    // seventh is the resident size.
    let mut record = [0_u64; 12];
    // SAFETY: `record` is 96 bytes aligned to 8, of this frame, which is what the first flavor
    // writes; the call writes nothing else.
    let status = unsafe { libc::proc_pid_rusage(pid, RUSAGE_INFO_V0, record.as_mut_ptr().cast()) };
    if status != 0 {
        return Err(format!(
            "proc_pid_rusage: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(record[8])
}

#[cfg(all(test, any(windows, target_os = "macos")))]
mod memory_tests {
    use super::{memory, resident_bytes};

    /// The system says how much memory the computer has, and that some of it, not more than all,
    /// is free for new work.
    #[test]
    fn the_computer_has_memory_and_some_of_it_free() {
        let memory = memory().expect("this computer's memory");
        eprintln!("{memory:?}");
        assert!(memory.total >= 1 << 30, "{memory:?}");
        assert!(memory.available > 0, "{memory:?}");
        assert!(memory.available <= memory.total, "{memory:?}");
    }

    /// This process holds some memory, a child it started and still runs holds some too, and no
    /// process has the largest number there is.
    #[test]
    fn a_running_process_holds_memory() {
        let own = resident_bytes(std::process::id()).expect("this process's memory");
        assert!(own > 0);
        // A program that waits for input it never gets, so it runs until it is killed.
        let waits = if cfg!(windows) { "cmd" } else { "cat" };
        let mut child = std::process::Command::new(waits)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("it starts");
        let held = resident_bytes(child.id());
        let _ = child.kill();
        let _ = child.wait();
        assert!(held.expect("the child's memory") > 0);
        assert!(resident_bytes(u32::MAX).is_err());
    }
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

    /// Windows describes this computer's caches: on x86-64, where Windows always describes them,
    /// an L1 data cache; on ARM64 the call answers, with what it describes.
    #[cfg(windows)]
    #[test]
    fn windows_describes_its_caches() {
        let described = super::caches();
        if cfg!(target_arch = "x86_64") {
            let caches = described.expect("Windows describes its caches");
            assert!(
                caches
                    .iter()
                    .any(|cache| cache.level == 1 && cache.kind == 2 && cache.bytes > 0),
                "{caches:?}"
            );
        } else if let Ok(caches) = described {
            assert!(caches.iter().all(|cache| cache.level > 0), "{caches:?}");
        }
    }
}
