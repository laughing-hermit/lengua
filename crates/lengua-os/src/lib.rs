//! lengua's calls that need unsafe code, kept apart from the rest so that the lengua crate can
//! forbid unsafe code outright: each is wrapped here in a safe function, with a test, those into
//! the operating system on the runner of the system they are for. On Linux, a program run in a
//! process forked from this one, with what the system counted of its cost; on Windows, how the
//! system describes the CPU's caches, and what a child process cost; on Windows and macOS, the
//! memory the computer has free and in all, and what a process holds of it; on every system, the
//! terminal a program runs in, its size and its mode made raw and put back, by a handler too
//! when a signal or the console's closing ends the process; and on Windows, a job that ends a
//! started program with every process it started. And on every system,
//! a module the engine compiled ahead of its runs, loaded into the engine only when it carries
//! the seal of the user's key ([`load_sealed`]).

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

        /// `GetFileInformationByHandle`: what Windows knows of the open file `file`, written to
        /// `information`, a `BY_HANDLE_FILE_INFORMATION`, which is thirteen 32-bit integers.
        #[link_name = "GetFileInformationByHandle"]
        pub(super) fn file_information(file: *mut core::ffi::c_void, information: *mut u32) -> i32;

        /// `GetConsoleMode`: the mode of the console `console` is a handle of, written to
        /// `mode`; 0 when it is no console.
        #[link_name = "GetConsoleMode"]
        pub(super) fn console_mode(console: *mut core::ffi::c_void, mode: *mut u32) -> i32;

        /// `SetConsoleMode`: sets the mode of the console `console` is a handle of.
        #[link_name = "SetConsoleMode"]
        pub(super) fn set_console_mode(console: *mut core::ffi::c_void, mode: u32) -> i32;

        /// `GetConsoleScreenBufferInfo`: what Windows knows of the console's screen, written to
        /// `information`, a `CONSOLE_SCREEN_BUFFER_INFO`, which is eleven 16-bit integers.
        #[link_name = "GetConsoleScreenBufferInfo"]
        pub(super) fn screen_information(
            console: *mut core::ffi::c_void,
            information: *mut i16,
        ) -> i32;

        /// `SetConsoleCtrlHandler`: adds `handler` to what Windows calls, on a thread of its
        /// own, when the console gets Ctrl-C or Ctrl-Break, or is closed.
        #[link_name = "SetConsoleCtrlHandler"]
        pub(super) fn set_control_handler(
            handler: Option<unsafe extern "system" fn(u32) -> i32>,
            add: i32,
        ) -> i32;

        /// `CreateJobObjectW`: a new job with no name; null when Windows makes none.
        #[link_name = "CreateJobObjectW"]
        pub(super) fn create_job(
            security: *mut core::ffi::c_void,
            name: *const u16,
        ) -> *mut core::ffi::c_void;

        /// `SetInformationJobObject`: sets the limits of `job` of the class `class` from the
        /// `length` bytes at `information`.
        #[link_name = "SetInformationJobObject"]
        pub(super) fn set_job_information(
            job: *mut core::ffi::c_void,
            class: i32,
            information: *const core::ffi::c_void,
            length: u32,
        ) -> i32;

        /// `AssignProcessToJobObject`: puts `process` in `job`, and with it every process it
        /// starts from then on.
        #[link_name = "AssignProcessToJobObject"]
        pub(super) fn assign_to_job(
            job: *mut core::ffi::c_void,
            process: *mut core::ffi::c_void,
        ) -> i32;

        /// `TerminateJobObject`: ends every process of `job` with the exit code `code`.
        #[link_name = "TerminateJobObject"]
        pub(super) fn terminate_job(job: *mut core::ffi::c_void, code: u32) -> i32;

        /// `CloseHandle`: closes a handle this process holds.
        #[link_name = "CloseHandle"]
        pub(super) fn close_handle(handle: *mut core::ffi::c_void) -> i32;
    }
}

/// A Windows job: the processes put in it, and every process they start, end together when
/// lengua ends the job or drops it. lengua puts a program it starts for a lengua program in
/// one, so that nothing the started program left behind outlives the call.
#[cfg(windows)]
#[derive(Debug)]
pub struct Job(*mut core::ffi::c_void);

// SAFETY: a job's handle is a number Windows takes from any thread, and `Job` has no other
// state.
#[cfg(windows)]
unsafe impl Send for Job {}

/// The class of a job's limits that holds `KILL_ON_JOB_CLOSE`:
/// `JobObjectExtendedLimitInformation`.
#[cfg(windows)]
const JOB_EXTENDED_LIMITS: i32 = 9;

/// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`: the job's processes end when its last handle closes.
#[cfg(windows)]
const JOB_KILL_ON_CLOSE: u64 = 0x2000;

/// How many 64-bit words a `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` takes on 64-bit Windows: 64
/// bytes of basic limits, 48 of counters and 32 of memory limits.
#[cfg(windows)]
const JOB_LIMIT_WORDS: usize = 18;

/// Which of those words holds `LimitFlags`, in its low half: the two time limits come first.
#[cfg(windows)]
const JOB_LIMIT_FLAGS: usize = 2;

#[cfg(windows)]
impl Job {
    /// A new job that holds `process`, and ends its processes when it is dropped.
    ///
    /// # Errors
    ///
    /// What Windows says, when it makes no job or won't put the process in it.
    pub fn holding(process: &impl std::os::windows::io::AsRawHandle) -> std::io::Result<Self> {
        // SAFETY: both arguments may be null: a job with the default security and no name.
        let handle = unsafe { windows::create_job(std::ptr::null_mut(), std::ptr::null()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let job = Self(handle);
        let mut limits = [0_u64; JOB_LIMIT_WORDS];
        limits[JOB_LIMIT_FLAGS] = JOB_KILL_ON_CLOSE;
        let length = u32::try_from(std::mem::size_of_val(&limits)).unwrap_or(u32::MAX);
        // SAFETY: `limits` holds the 144 bytes of a JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        // aligned to 8 as its fields need, and Windows reads `length` bytes of it.
        let set = unsafe {
            windows::set_job_information(
                job.0,
                JOB_EXTENDED_LIMITS,
                limits.as_ptr().cast::<core::ffi::c_void>(),
                length,
            )
        };
        if set == 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: the job's handle is open, and the process's is for as long as `process` is
        // borrowed.
        let assigned = unsafe { windows::assign_to_job(job.0, process.as_raw_handle()) };
        if assigned == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(job)
    }

    /// Ends every process of the job now.
    pub fn end(&self) {
        // SAFETY: the job's handle is open until `self` is dropped.
        unsafe {
            windows::terminate_job(self.0, 1);
        }
    }
}

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: the handle is the job's own, open until now, and closed once.
        unsafe {
            windows::close_handle(self.0);
        }
    }
}

/// The terminal a program runs in, on Linux and macOS: its size, and its mode made raw and put
/// back. Raw here is keys as they're pressed: no line editing and no echo, Enter as the byte 13
/// (as Windows gives it), Ctrl-S and Ctrl-Q as keys. The interrupt key stays, so Ctrl-C still
/// ends the program, and output is as before.
#[cfg(unix)]
pub mod terminal {
    use std::io;
    use std::os::fd::{AsFd as _, AsRawFd as _, BorrowedFd};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Once, OnceLock};

    use nix::libc;
    use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};
    use nix::sys::termios::{
        InputFlags, LocalFlags, SetArg, SpecialCharacterIndices, Termios, tcgetattr, tcsetattr,
    };

    nix::ioctl_read_bad!(
        /// `TIOCGWINSZ`: the size of the terminal a descriptor is, written to a `winsize`.
        window_size,
        libc::TIOCGWINSZ,
        libc::winsize
    );

    /// The mode a terminal had before lengua made it raw.
    #[derive(Clone, Copy)]
    pub struct Saved(libc::termios);

    impl std::fmt::Debug for Saved {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("Saved")
        }
    }

    /// The mode standard input's terminal had before lengua first made it raw.
    static BEFORE: OnceLock<Saved> = OnceLock::new();

    /// Whether standard input's terminal is raw now, by lengua's doing.
    static RAW: AtomicBool = AtomicBool::new(false);

    /// The handlers that put the mode back are set once.
    static GUARDED: Once = Once::new();

    /// The system's error as a `std::io` one.
    fn failed(error: nix::errno::Errno) -> io::Error {
        io::Error::from(error)
    }

    /// A number of any width as 64 bits: the system's flags are 32 bits wide on Linux and 64 on
    /// macOS.
    fn wide<T: Into<u64>>(number: T) -> u64 {
        number.into()
    }

    /// The columns and rows of the terminal `terminal` is.
    ///
    /// # Errors
    ///
    /// What the system says: `terminal` is no terminal.
    pub fn size_of(terminal: BorrowedFd<'_>) -> io::Result<(u16, u16)> {
        let mut size = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: the descriptor is open for as long as `terminal` is borrowed, and the system
        // writes one `winsize` to `size`, of this frame.
        unsafe { window_size(terminal.as_raw_fd(), &raw mut size) }.map_err(failed)?;
        Ok((size.ws_col, size.ws_row))
    }

    /// The mode of the terminal `terminal` is, as numbers: equal when the mode is the same. The
    /// flag `PENDIN` is left out: it is no mode but the system's own note that typed input waits
    /// to be read again, which macOS sets when a terminal goes back to lines.
    ///
    /// # Errors
    ///
    /// What the system says: `terminal` is no terminal.
    pub fn mode_of(terminal: BorrowedFd<'_>) -> io::Result<Vec<u64>> {
        let now: libc::termios = tcgetattr(terminal).map_err(failed)?.into();
        let mut mode = vec![
            wide(now.c_iflag),
            wide(now.c_oflag),
            wide(now.c_cflag),
            wide(now.c_lflag & !libc::PENDIN),
        ];
        mode.extend(now.c_cc.iter().map(|code| u64::from(*code)));
        Ok(mode)
    }

    /// Makes the terminal `terminal` is raw, and gives the mode it had.
    ///
    /// # Errors
    ///
    /// What the system says: `terminal` is no terminal.
    pub fn set_raw(terminal: BorrowedFd<'_>) -> io::Result<Saved> {
        let before: Termios = tcgetattr(terminal).map_err(failed)?;
        let mut raw = before.clone();
        raw.local_flags
            .remove(LocalFlags::ICANON | LocalFlags::ECHO);
        raw.input_flags.remove(InputFlags::ICRNL | InputFlags::IXON);
        raw.control_chars[SpecialCharacterIndices::VMIN as usize] = 1;
        raw.control_chars[SpecialCharacterIndices::VTIME as usize] = 0;
        tcsetattr(terminal, SetArg::TCSANOW, &raw).map_err(failed)?;
        Ok(Saved(before.into()))
    }

    /// Puts the mode `saved` back on the terminal `terminal` is.
    ///
    /// # Errors
    ///
    /// What the system says.
    pub fn put_back(terminal: BorrowedFd<'_>, saved: &Saved) -> io::Result<()> {
        tcsetattr(terminal, SetArg::TCSANOW, &Termios::from(saved.0)).map_err(failed)
    }

    /// A pseudo-terminal of the caller's own, for a test of a program at a terminal: the end a
    /// program gets as its terminal, then the end that types its keys and reads what it shows.
    ///
    /// # Errors
    ///
    /// What the system says, when it gives none.
    pub fn pseudo() -> io::Result<(std::os::fd::OwnedFd, std::os::fd::OwnedFd)> {
        let pair = nix::pty::openpty(None, None).map_err(failed)?;
        Ok((pair.slave, pair.master))
    }

    /// What a signal that ends the process does first: the terminal's mode goes back, then the
    /// signal does what it does by default.
    extern "C" fn on_signal(number: libc::c_int) {
        if RAW.load(Ordering::SeqCst)
            && let Some(before) = BEFORE.get()
        {
            // SAFETY: `tcsetattr` may be called from a signal's handler, and `before` is never
            // written once it is set.
            unsafe {
                libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw const before.0);
            }
        }
        // SAFETY: both may be called from a signal's handler: the signal's default action is
        // set again, and the signal sent to this process once more, which ends it as the
        // signal would have.
        unsafe {
            libc::signal(number, libc::SIG_DFL);
            libc::raise(number);
        }
    }

    /// Sets the handlers that put the mode back when a signal ends the process.
    fn guard() {
        GUARDED.call_once(|| {
            let action = SigAction::new(
                SigHandler::Handler(on_signal),
                SaFlags::empty(),
                SigSet::empty(),
            );
            for signal in [
                Signal::SIGINT,
                Signal::SIGTERM,
                Signal::SIGHUP,
                Signal::SIGQUIT,
            ] {
                // SAFETY: the handler calls only what a signal's handler may call, and reads
                // only values that are never written while it can run.
                let _ = unsafe { sigaction(signal, &action) };
            }
        });
    }

    /// The columns and rows of the terminal this process runs in: standard output's, or
    /// standard error's, or standard input's, the first that is one.
    #[must_use]
    pub fn size() -> Option<(u16, u16)> {
        size_of(io::stdout().as_fd())
            .or_else(|_| size_of(io::stderr().as_fd()))
            .or_else(|_| size_of(io::stdin().as_fd()))
            .ok()
    }

    /// Makes standard input's terminal raw. The mode it had the first time is kept, and put
    /// back by [`restore`], and by a handler when `SIGINT`, `SIGTERM`, `SIGHUP` or `SIGQUIT`
    /// ends the process.
    ///
    /// # Errors
    ///
    /// What the system says: standard input is no terminal.
    pub fn raw() -> io::Result<()> {
        let stdin = io::stdin();
        let saved = set_raw(stdin.as_fd())?;
        BEFORE.get_or_init(|| saved);
        RAW.store(true, Ordering::SeqCst);
        guard();
        Ok(())
    }

    /// Puts standard input's terminal back in the mode it had before [`raw`]; nothing when it
    /// isn't raw.
    ///
    /// # Errors
    ///
    /// What the system says.
    pub fn restore() -> io::Result<()> {
        if !RAW.swap(false, Ordering::SeqCst) {
            return Ok(());
        }
        match BEFORE.get() {
            Some(before) => put_back(io::stdin().as_fd(), before),
            None => Ok(()),
        }
    }

    #[cfg(test)]
    mod tests {
        use std::os::fd::AsFd as _;
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        use super::{mode_of, put_back, set_raw, size_of};

        /// What the test's own process is started again with, to be the child of a test.
        const CHILD: &str = "lengua-os-child";

        /// A terminal of the test's own goes raw and back: its mode changes while it is raw,
        /// and afterwards every flag and key is as before.
        #[test]
        fn a_terminal_goes_raw_and_back() {
            let pair = nix::pty::openpty(None, None).expect("a pseudo-terminal");
            let terminal = pair.slave.as_fd();
            let before = mode_of(terminal).expect("its mode");
            let saved = set_raw(terminal).expect("it goes raw");
            let raw = mode_of(terminal).expect("its mode");
            assert_ne!(raw, before);
            put_back(terminal, &saved).expect("it goes back");
            assert_eq!(mode_of(terminal).expect("its mode"), before);
            // A pseudo-terminal has a size, which a pipe doesn't.
            assert!(size_of(terminal).is_ok());
            let (read, _write) = std::io::pipe().expect("a pipe");
            assert!(size_of(read.as_fd()).is_err());
            assert!(set_raw(read.as_fd()).is_err());
        }

        /// The child of the next test: makes its standard input raw, and waits to be ended.
        /// Run by itself it does nothing.
        #[test]
        #[ignore = "the child of a_signal_puts_the_mode_back"]
        fn child_goes_raw_and_waits() {
            if !std::env::args().any(|argument| argument == CHILD) {
                return;
            }
            super::raw().expect("standard input is the test's terminal");
            std::thread::sleep(Duration::from_secs(60));
        }

        /// A process whose terminal lengua made raw, and that a signal ends, leaves the
        /// terminal as it found it.
        #[test]
        fn a_signal_puts_the_mode_back() {
            let pair = nix::pty::openpty(None, None).expect("a pseudo-terminal");
            let before = mode_of(pair.slave.as_fd()).expect("its mode");
            let input = pair.slave.try_clone().expect("a second descriptor");
            let mut child = Command::new(std::env::current_exe().expect("this test"))
                .args([
                    "--exact",
                    "terminal::tests::child_goes_raw_and_waits",
                    CHILD,
                    "--ignored",
                    "--nocapture",
                ])
                .stdin(Stdio::from(input))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("the child starts");
            let started = Instant::now();
            while mode_of(pair.slave.as_fd()).expect("its mode") == before {
                assert!(
                    started.elapsed() < Duration::from_secs(30),
                    "the child never made the terminal raw"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
            let pid = nix::unistd::Pid::from_raw(i32::try_from(child.id()).expect("a pid"));
            nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM).expect("a signal");
            let status = child.wait().expect("the child ends");
            assert!(!status.success());
            assert_eq!(mode_of(pair.slave.as_fd()).expect("its mode"), before);
        }
    }
}

/// The terminal a program runs in, on Windows, which calls it the console: its size, and its
/// mode made raw and put back. Raw here is keys as they're pressed: no line input and no echo,
/// with the keys as a terminal's sequences (`ENABLE_VIRTUAL_TERMINAL_INPUT`), as Linux and macOS
/// give them. Ctrl-C still ends the program.
#[cfg(windows)]
pub mod terminal {
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use std::sync::Once;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    use super::windows;

    /// `ENABLE_LINE_INPUT`: a read gives a whole line.
    const LINE_INPUT: u32 = 0x2;
    /// `ENABLE_ECHO_INPUT`: what is typed is shown.
    const ECHO_INPUT: u32 = 0x4;
    /// `ENABLE_VIRTUAL_TERMINAL_INPUT`: keys come as a terminal's sequences.
    const TERMINAL_INPUT: u32 = 0x200;
    /// `ENABLE_VIRTUAL_TERMINAL_PROCESSING`: output's sequences are obeyed.
    const TERMINAL_OUTPUT: u32 = 0x4;

    /// How many 16-bit integers a `CONSOLE_SCREEN_BUFFER_INFO` takes: the buffer's size, the
    /// cursor, the attributes, the window's four edges and the largest window.
    const SCREEN_INTEGERS: usize = 11;
    /// Which of them are the window's left, top, right and bottom edges.
    const WINDOW: [usize; 4] = [5, 6, 7, 8];

    /// The mode standard input's console had before lengua first made it raw.
    static INPUT_BEFORE: AtomicU32 = AtomicU32::new(0);
    /// The mode standard output's console had then, when it is one.
    static OUTPUT_BEFORE: AtomicU32 = AtomicU32::new(0);
    /// Whether standard output's mode was kept.
    static OUTPUT_KEPT: AtomicBool = AtomicBool::new(false);
    /// Whether the modes were kept.
    static KEPT: AtomicBool = AtomicBool::new(false);
    /// Whether standard input's console is raw now, by lengua's doing.
    static RAW: AtomicBool = AtomicBool::new(false);
    /// The handler that puts the mode back is set once.
    static GUARDED: Once = Once::new();

    /// The mode of the console `console` is a handle of.
    ///
    /// # Errors
    ///
    /// What Windows says: `console` is no console.
    pub fn mode_of(console: &impl AsRawHandle) -> io::Result<u32> {
        let mut mode: u32 = 0;
        // SAFETY: the handle is open for as long as `console` is borrowed, and Windows writes
        // 32 bits to `mode`, of this frame.
        let done = unsafe { windows::console_mode(console.as_raw_handle(), &raw mut mode) };
        if done == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(mode)
    }

    /// Sets the mode of the console `console` is a handle of.
    ///
    /// # Errors
    ///
    /// What Windows says.
    pub fn set_mode(console: &impl AsRawHandle, mode: u32) -> io::Result<()> {
        // SAFETY: the handle is open for as long as `console` is borrowed.
        let done = unsafe { windows::set_console_mode(console.as_raw_handle(), mode) };
        if done == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// An input mode made raw: no line input, no echo, and keys as a terminal's sequences.
    #[must_use]
    pub const fn raw_input(mode: u32) -> u32 {
        (mode & !(LINE_INPUT | ECHO_INPUT)) | TERMINAL_INPUT
    }

    /// Makes the console `input` is a handle of raw, and gives the mode it had. A console that
    /// knows no terminal's sequences is made raw without them.
    ///
    /// # Errors
    ///
    /// What Windows says: `input` is no console's input.
    pub fn set_raw(input: &impl AsRawHandle) -> io::Result<u32> {
        let before = mode_of(input)?;
        let raw = raw_input(before);
        set_mode(input, raw).or_else(|_| set_mode(input, raw & !TERMINAL_INPUT))?;
        Ok(before)
    }

    /// The columns and rows of the window of the console `output` is a handle of.
    ///
    /// # Errors
    ///
    /// What Windows says: `output` is no console's output.
    pub fn size_of(output: &impl AsRawHandle) -> io::Result<(u16, u16)> {
        let mut information = [0_i16; SCREEN_INTEGERS];
        // SAFETY: the handle is open for as long as `output` is borrowed, and Windows writes
        // one record of eleven 16-bit integers to `information`, of this frame.
        let done = unsafe {
            windows::screen_information(output.as_raw_handle(), information.as_mut_ptr())
        };
        if done == 0 {
            return Err(io::Error::last_os_error());
        }
        let [left, top, right, bottom] = WINDOW.map(|index| i32::from(information[index]));
        let columns = u16::try_from(right - left + 1).unwrap_or(0);
        let rows = u16::try_from(bottom - top + 1).unwrap_or(0);
        Ok((columns, rows))
    }

    /// Puts the kept modes back, when the console is raw.
    fn put_back() -> io::Result<()> {
        if !RAW.swap(false, Ordering::SeqCst) || !KEPT.load(Ordering::SeqCst) {
            return Ok(());
        }
        if OUTPUT_KEPT.load(Ordering::SeqCst) {
            let _ = set_mode(&io::stdout(), OUTPUT_BEFORE.load(Ordering::SeqCst));
        }
        set_mode(&io::stdin(), INPUT_BEFORE.load(Ordering::SeqCst))
    }

    /// What Windows calls when the console gets Ctrl-C or Ctrl-Break, or is closed: the mode
    /// goes back, and the next handler, Windows's own, ends the process.
    unsafe extern "system" fn on_control(_: u32) -> i32 {
        let _ = put_back();
        0
    }

    /// The columns and rows of the console this process runs in: standard output's, or
    /// standard error's.
    #[must_use]
    pub fn size() -> Option<(u16, u16)> {
        size_of(&io::stdout())
            .or_else(|_| size_of(&io::stderr()))
            .ok()
    }

    /// Makes standard input's console raw, and has standard output's obey a terminal's
    /// sequences. The modes they had the first time are kept, and put back by [`restore`], and
    /// by a handler when Ctrl-C or Ctrl-Break ends the process or the console is closed.
    ///
    /// # Errors
    ///
    /// What Windows says: standard input is no console.
    pub fn raw() -> io::Result<()> {
        let before = set_raw(&io::stdin())?;
        if !KEPT.swap(true, Ordering::SeqCst) {
            INPUT_BEFORE.store(before, Ordering::SeqCst);
            if let Ok(output) = mode_of(&io::stdout()) {
                OUTPUT_BEFORE.store(output, Ordering::SeqCst);
                OUTPUT_KEPT.store(true, Ordering::SeqCst);
            }
        }
        if OUTPUT_KEPT.load(Ordering::SeqCst) {
            let output = OUTPUT_BEFORE.load(Ordering::SeqCst);
            let _ = set_mode(&io::stdout(), output | TERMINAL_OUTPUT);
        }
        RAW.store(true, Ordering::SeqCst);
        GUARDED.call_once(|| {
            // SAFETY: the handler is a function of this program, which lives as long as it
            // does.
            unsafe {
                windows::set_control_handler(Some(on_control), 1);
            }
        });
        Ok(())
    }

    /// Puts standard input's console, and standard output's, back in the modes they had before
    /// [`raw`]; nothing when the console isn't raw.
    ///
    /// # Errors
    ///
    /// What Windows says.
    pub fn restore() -> io::Result<()> {
        put_back()
    }

    #[cfg(test)]
    mod tests {
        use std::os::windows::process::CommandExt as _;
        use std::process::{Command, Stdio};

        use super::{mode_of, raw_input, set_mode, set_raw, size_of};

        /// What the test's own process is started again with, to be the child of a test.
        const CHILD: &str = "lengua-os-child";

        /// `CREATE_NEW_CONSOLE`: the started process gets a console of its own.
        const NEW_CONSOLE: u32 = 0x10;

        /// The test's own process, started again to run the ignored test `name` as a child.
        fn child(name: &str) -> Command {
            let mut command = Command::new(std::env::current_exe().expect("this test"));
            command
                .args(["--exact", name, CHILD, "--ignored", "--nocapture"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            command
        }

        /// The child of the next test, in a console of its own: the console goes raw and back,
        /// and has a size. Run by itself it does nothing.
        #[test]
        #[ignore = "the child of a_console_goes_raw_and_back"]
        fn child_in_a_console() {
            if !std::env::args().any(|argument| argument == CHILD) {
                return;
            }
            let open = |name: &str| {
                std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(name)
                    .expect("the console")
            };
            let input = open("CONIN$");
            let before = mode_of(&input).expect("its mode");
            assert_eq!(set_raw(&input).expect("it goes raw"), before);
            let raw = mode_of(&input).expect("its mode");
            assert_eq!(raw & 0x6, 0, "line input and echo are off: {raw:#x}");
            assert_eq!(raw | 0x200, raw_input(before));
            set_mode(&input, before).expect("it goes back");
            assert_eq!(mode_of(&input).expect("its mode"), before);
            let (columns, rows) = size_of(&open("CONOUT$")).expect("its size");
            assert!(columns > 0 && rows > 0, "{columns} by {rows}");
            // A file is no console.
            let file =
                std::fs::File::open(std::env::current_exe().expect("this test")).expect("it opens");
            assert!(mode_of(&file).is_err());
            assert!(size_of(&file).is_err());
        }

        /// A console goes raw and back, and has a size: shown in a console of the child's own,
        /// since a test's own streams are pipes.
        #[test]
        fn a_console_goes_raw_and_back() {
            let status = child("terminal::tests::child_in_a_console")
                .creation_flags(NEW_CONSOLE)
                .status()
                .expect("the child starts");
            assert!(status.success(), "{status:?}");
        }

        /// The child of the next test: waits to be ended. Run by itself it does nothing.
        #[test]
        #[ignore = "the child of a_job_ends_its_process"]
        fn child_waits() {
            if !std::env::args().any(|argument| argument == CHILD) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_secs(60));
        }

        /// A job ends the process it holds when it is told to.
        #[test]
        fn a_job_ends_its_process() {
            let mut waiting = child("terminal::tests::child_waits")
                .spawn()
                .expect("the child starts");
            let job = crate::Job::holding(&waiting).expect("a job");
            job.end();
            let status = waiting.wait().expect("the child ends");
            assert!(!status.success(), "{status:?}");
        }
    }
}

/// How many 32-bit integers a `BY_HANDLE_FILE_INFORMATION` takes: the attributes, three times of
/// two each, the volume's serial number, the size in two, the number of links, and the file's
/// index in two.
#[cfg(windows)]
const FILE_INFORMATION_INTEGERS: usize = 13;

/// Which of them is the number of links, `nNumberOfLinks`.
#[cfg(windows)]
const NUMBER_OF_LINKS: usize = 10;

/// How many names the open file `file` has on its volume, as Windows counts them: more than one
/// when it has a hard link.
///
/// # Errors
///
/// What Windows says, when it won't describe the file.
#[cfg(windows)]
pub fn link_count(file: &impl std::os::windows::io::AsRawHandle) -> std::io::Result<u64> {
    let mut information = [0_u32; FILE_INFORMATION_INTEGERS];
    // SAFETY: the handle is an open file's for as long as `file` is borrowed, and Windows writes
    // one record of thirteen 32-bit integers to `information`, of this frame, which holds that
    // many, aligned as they need.
    let done = unsafe { windows::file_information(file.as_raw_handle(), information.as_mut_ptr()) };
    if done == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(u64::from(information[NUMBER_OF_LINKS]))
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

/// A child process whose life is tied to this process's on Windows: while this value lives, the
/// child lives no longer than this process, however this process ends ([`tie`]). Dropping it
/// ends a child that still runs.
#[cfg(windows)]
#[derive(Debug)]
pub struct Tied(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for Tied {
    fn drop(&mut self) {
        // SAFETY: the handle is the job's, which `tie` made and nothing else closes.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

/// Ties `child`'s life to this process's: it puts `child` in a job of its own that Windows ends
/// when the job's last handle closes, and this process holds the only one, in what this gives.
/// So when this process is ended from outside, as a service manager or `lengua-mcp measure` ends
/// lengua-run, the lengua-mcp it left a run to ends with it, where a Unix has one process for
/// both.
///
/// # Errors
///
/// The system's reason, when it won't make the job or take the child into it.
#[cfg(windows)]
pub fn tie(child: &std::process::Child) -> Result<Tied, String> {
    use std::os::windows::io::AsRawHandle as _;

    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_BASIC_LIMIT_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    };

    let failed = |call: &str| format!("{call}: {}", std::io::Error::last_os_error());
    // SAFETY: the job has no security attributes and no name, and the call takes a null pointer
    // for each.
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
        return Err(failed("CreateJobObjectW"));
    }
    let tied = Tied(job);
    let limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
            LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            ..JOBOBJECT_BASIC_LIMIT_INFORMATION::default()
        },
        ..JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default()
    };
    let size = u32::try_from(std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
        .unwrap_or(u32::MAX);
    // SAFETY: `job` is the job just made, open in `tied`; `limits` is a
    // JOBOBJECT_EXTENDED_LIMIT_INFORMATION of this frame, `size` bytes, which the call reads.
    let set = unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast(),
            size,
        )
    };
    if set == 0 {
        return Err(failed("SetInformationJobObject"));
    }
    // SAFETY: `job` as above, and the other handle is the process handle `child` owns, open
    // while `child` is borrowed, with the full access CreateProcess gives it.
    let assigned = unsafe { AssignProcessToJobObject(job, child.as_raw_handle()) };
    if assigned == 0 {
        return Err(failed("AssignProcessToJobObject"));
    }
    Ok(tied)
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

/// The first bytes of what a seal authenticates: no other use of the user's key begins with
/// them, so a lock's signature is no seal and a seal no lock's signature.
const SEAL_DOMAIN: &[u8] = b"lengua-compiled-module-v1\n";

/// How many bytes a seal is: an HMAC-SHA256 tag, the last bytes of a sealed module.
pub const SEAL_BYTES: usize = 32;

/// How many bytes the key that seals a module is.
pub const SEAL_KEY_BYTES: usize = 32;

/// Why a sealed module wasn't loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsealed {
    /// The bytes don't carry this key's seal for what they are said to be: they were never
    /// handed to the engine.
    NoSeal,
    /// The seal is this key's, and the engine doesn't take the module: it was compiled by
    /// another version or configuration of the engine, or for a CPU this one isn't. It says
    /// why.
    Engine(String),
}

impl std::fmt::Display for Unsealed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSeal => f.write_str("the compiled module doesn't carry this user's seal"),
            Self::Engine(why) => write!(f, "the engine doesn't take the compiled module: {why}"),
        }
    }
}

impl std::error::Error for Unsealed {}

/// The HMAC-SHA256, keyed with `key`, of a compiled module and what it is said to be, `about`:
/// the domain, `about`'s length, `about`, then the module, so no two pairs give the same bytes.
/// It is ring's, whose SHA-256 uses the CPU's own instructions for it on x86-64 and on ARM64:
/// every run computes it over the whole compiled module, which for a small program is 12 KiB on
/// x86-64 and 135 KiB on ARM64, where the engine lays code out for 64 KiB pages. There ring took
/// 0.06 ms for it and the sha2 crate, which has those instructions on x86-64 alone, 0.32 ms (W19).
fn seal(key: &[u8; SEAL_KEY_BYTES], about: &[u8], compiled: &[u8]) -> ring::hmac::Tag {
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, key);
    let mut mac = ring::hmac::Context::with_key(&key);
    mac.update(SEAL_DOMAIN);
    mac.update(&u64::try_from(about.len()).unwrap_or(u64::MAX).to_le_bytes());
    mac.update(about);
    mac.update(compiled);
    mac.sign()
}

/// Compiles the WebAssembly module `wasm` with `engine` for this computer, ahead of its runs,
/// and seals what the engine made with `key` as `about`, which says what the module is, such as
/// its SHA-256: the compiled module, then its seal. It is the only function that makes a seal,
/// and it seals nothing but what the engine's own compiler gave it in this very call, so
/// whatever carries a seal of `key` is a module this engine's code compiled, for whoever keeps
/// `key` to themselves.
///
/// # Errors
///
/// When the engine doesn't compile the module, as it says.
#[cfg(feature = "compiler")]
pub fn compile_sealed(
    engine: &wasmtime::Engine,
    key: &[u8; SEAL_KEY_BYTES],
    about: &[u8],
    wasm: &[u8],
) -> Result<Vec<u8>, String> {
    let mut compiled = engine
        .precompile_module(wasm)
        .map_err(|error| format!("{error:#}"))?;
    let tag = seal(key, about, &compiled);
    compiled.extend_from_slice(tag.as_ref());
    Ok(compiled)
}

/// Loads into `engine` the compiled module `sealed` holds, once its last [`SEAL_BYTES`] bytes
/// are `key`'s seal of the bytes before them as `about` ([`compile_sealed`]), compared in
/// constant time. The module's code is copied into memory of the engine's own, so nothing that
/// changes the bytes' file afterwards changes what runs.
///
/// Wasmtime's `Module::deserialize` is unsafe because it trusts its bytes: it runs the machine
/// code in them and follows their tables with no check that a compiler of its own made them, so
/// other bytes can make the process run anything. This function is safe to call with any bytes
/// because it hands the engine only bytes that carry a seal, and a seal is made in one place,
/// [`compile_sealed`], over what `Engine::precompile_module` returned there. Bytes with a seal
/// are therefore a module Wasmtime compiled, unchanged since, or the work of someone who holds
/// `key`. lengua's key is the user's own secret, the one that signs lengua.lock, kept in a file
/// only the user reads: whoever holds it already acts as the user. HMAC-SHA256 makes any other
/// bytes fail, whatever they are and however they were chosen. Wasmtime then holds a module of
/// its own to its version, its configuration and this CPU's features, and refuses one that
/// doesn't suit `engine`, which is an error here and nothing run.
///
/// # Errors
///
/// [`Unsealed::NoSeal`] for bytes without the seal, which the engine never sees;
/// [`Unsealed::Engine`] when the seal holds and the engine refuses the module.
pub fn load_sealed(
    engine: &wasmtime::Engine,
    key: &[u8; SEAL_KEY_BYTES],
    about: &[u8],
    sealed: &[u8],
) -> Result<wasmtime::Module, Unsealed> {
    use subtle::ConstantTimeEq as _;
    let Some(length) = sealed.len().checked_sub(SEAL_BYTES) else {
        return Err(Unsealed::NoSeal);
    };
    let (compiled, tag) = sealed.split_at(length);
    let own = seal(key, about, compiled);
    if !bool::from(own.as_ref().ct_eq(tag)) {
        return Err(Unsealed::NoSeal);
    }
    // SAFETY: `compiled` carries `key`'s seal, checked just above, and a seal is made only by
    // `compile_sealed`, over the bytes `Engine::precompile_module` returned: these are bytes
    // Wasmtime's compiler made, which is what `deserialize` asks of its caller. It copies them,
    // so they are not read from the caller's slice again.
    unsafe { wasmtime::Module::deserialize(engine, compiled) }
        .map_err(|error| Unsealed::Engine(format!("{error:#}")))
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

    /// A child tied to this process ends when the tie goes, as it would when this process
    /// ended: a `ping` that would run for a minute is over within seconds of it.
    #[test]
    fn a_tied_child_ends_with_its_tie() {
        let mut child = std::process::Command::new("ping")
            .args(["-n", "60", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("ping starts");
        let tied = super::tie(&child).expect("the tie");
        let started = std::time::Instant::now();
        assert!(child.try_wait().expect("its state").is_none(), "ping ended");
        drop(tied);
        child.wait().expect("ping ends");
        assert!(started.elapsed() < std::time::Duration::from_secs(30));
    }

    /// A file has one name, and two once it has a hard link, counted from the handle that was
    /// open before the link was made.
    #[test]
    fn an_open_file_counts_its_names() {
        let folder = std::env::temp_dir().join(format!("lengua-os-links-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("a folder");
        let first = folder.join("first");
        std::fs::write(&first, "one").expect("a file");
        let file = std::fs::File::open(&first).expect("it opens");
        assert_eq!(super::link_count(&file).expect("its names"), 1);
        std::fs::hard_link(&first, folder.join("second")).expect("a hard link");
        assert_eq!(super::link_count(&file).expect("its names"), 2);
        drop(file);
        let _ = std::fs::remove_dir_all(&folder);
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

#[cfg(all(test, feature = "compiler"))]
mod sealed_tests {
    use super::{SEAL_BYTES, Unsealed, compile_sealed, load_sealed};

    /// The smallest module: WebAssembly's magic number and version 1.
    const EMPTY: &[u8] = b"\0asm\x01\0\0\0";

    /// A key, and another.
    const KEYS: [[u8; 32]; 2] = [[7; 32], [8; 32]];

    /// A module the engine compiled loads under the key and the words it was sealed with, and
    /// under no other: a changed byte anywhere, in the module or in the seal, another key,
    /// other words, the seal alone, the module without its seal, and bytes no compiler made
    /// each fail the seal, which is checked before the engine sees a byte.
    #[test]
    fn a_compiled_module_loads_only_with_its_seal() {
        let engine = wasmtime::Engine::default();
        let sealed = compile_sealed(&engine, &KEYS[0], b"empty", EMPTY).expect("it compiles");
        assert!(sealed.len() > SEAL_BYTES);
        assert!(load_sealed(&engine, &KEYS[0], b"empty", &sealed).is_ok());
        let refused = |key: &[u8; 32], about: &[u8], bytes: &[u8]| {
            load_sealed(&engine, key, about, bytes).err()
        };
        for at in [
            0,
            sealed.len() / 2,
            sealed.len() - SEAL_BYTES,
            sealed.len() - 1,
        ] {
            let mut changed = sealed.clone();
            changed[at] ^= 1;
            assert_eq!(
                refused(&KEYS[0], b"empty", &changed),
                Some(Unsealed::NoSeal),
                "byte {at}"
            );
        }
        assert_eq!(refused(&KEYS[1], b"empty", &sealed), Some(Unsealed::NoSeal));
        assert_eq!(refused(&KEYS[0], b"other", &sealed), Some(Unsealed::NoSeal));
        assert_eq!(refused(&KEYS[0], b"", &sealed), Some(Unsealed::NoSeal));
        let (compiled, seal) = sealed.split_at(sealed.len() - SEAL_BYTES);
        assert_eq!(
            refused(&KEYS[0], b"empty", compiled),
            Some(Unsealed::NoSeal)
        );
        assert_eq!(refused(&KEYS[0], b"empty", seal), Some(Unsealed::NoSeal));
        assert_eq!(refused(&KEYS[0], b"empty", &[]), Some(Unsealed::NoSeal));
        assert_eq!(refused(&KEYS[0], b"empty", EMPTY), Some(Unsealed::NoSeal));
        // What it is said to be is told apart from the module's first bytes: the words' length
        // goes into the seal.
        let mut moved = b"y".to_vec();
        moved.extend_from_slice(compiled);
        moved.extend_from_slice(seal);
        assert_eq!(refused(&KEYS[0], b"empt", &moved), Some(Unsealed::NoSeal));
    }

    /// The engine holds a sealed module to its own configuration: one compiled to count fuel
    /// doesn't load into an engine that counts none, and the answer says it was the engine's.
    #[test]
    fn the_engine_refuses_a_sealed_module_of_another_configuration() {
        let mut counting = wasmtime::Config::new();
        counting.consume_fuel(true);
        let counting = wasmtime::Engine::new(&counting).expect("an engine");
        let sealed = compile_sealed(&counting, &KEYS[0], b"empty", EMPTY).expect("it compiles");
        assert!(load_sealed(&counting, &KEYS[0], b"empty", &sealed).is_ok());
        let plain = wasmtime::Engine::default();
        let refused = load_sealed(&plain, &KEYS[0], b"empty", &sealed).err();
        assert!(matches!(refused, Some(Unsealed::Engine(_))), "{refused:?}");
    }
}
