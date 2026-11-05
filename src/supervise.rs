//! The test supervisor. When a test supplies random bytes or clock readings, a second process of
//! this binary traces the launcher, and so the program it becomes, and answers each `getrandom`
//! and `clock_gettime` call with the test's values instead of the kernel's. The launcher's
//! seccomp filter stops only those two calls; everything else runs as in any test, including
//! the counters, the exit status and the time limit.
//!
//! Bytes asked for past the end of the supply are zeros, so a program may draw in batches of any
//! size: only the bytes it uses decide its output. The last clock reading repeats.
//!
//! For a program that uses files it also holds each name a call is given to what every system
//! takes (`crate::names`): a name some system refuses, or a new name that differs only in case
//! or normalization from one in its folder, gets the same error on every system, and so does a
//! file renamed or removed while the program holds it open, which Windows refuses. It notes
//! whether the program listed a folder, and on a test's second run turns each listing round, so
//! a result that depends on the order a file system lists in is found.
//!
//! It reads the launcher's process id, its mode (`plain`, `files` or `files reversed`) and the
//! supply on standard input, says
//! `attached` once it traces the launcher, and when the program ends says `ok`, how many random
//! bytes the program asked for and whether it listed a folder (1 or 0), or what went wrong.

use std::io::Write as _;
use std::process::ExitCode;

/// The command-line flag that starts the binary as a supervisor.
pub const SUPERVISE_FLAG: &str = "--supervise";

/// Whether the supervisor runs on this computer: it traces with Linux's ptrace, and reads and
/// rewrites a stopped call's registers, x86-64's and AArch64's. Where it doesn't, tests that need
/// it aren't run.
pub const AVAILABLE: bool = cfg!(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")));

fn say(line: &str) {
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{line}");
    let _ = stdout.flush();
}

/// Supervisor mode: traces the launcher named on standard input until its program ends.
#[must_use]
pub fn supervise() -> ExitCode {
    #[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        traced::supervise()
    }
    #[cfg(not(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64"))))]
    {
        say(&format!(
            "error: lengua's supervisor doesn't run on {} {} yet",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        ExitCode::FAILURE
    }
}

/// The supervisor itself, for Linux on a CPU whose registers it reads.
#[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]
mod traced {
    use std::collections::HashMap;
    use std::io::{IoSlice, Read as _};
    use std::os::unix::fs::MetadataExt as _;
    use std::path::{Path, PathBuf};
    use std::process::ExitCode;

    use nix::errno::Errno;
    use nix::sys::ptrace::{self, Event, Options};
    use nix::sys::uio::{RemoteIoVec, process_vm_readv, process_vm_writev};
    use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
    use nix::unistd::Pid;

    use super::say;
    use crate::supply::Supply;
    use crate::syscalls::host as syscalls;

    /// Traces the launcher named on standard input until its program ends.
    pub(super) fn supervise() -> ExitCode {
        let mut input = String::new();
        if let Err(error) = std::io::stdin().read_to_string(&mut input) {
            say(&format!("error: no supply: {error}"));
            return ExitCode::FAILURE;
        }
        let parsed = input
            .split_once('\n')
            .ok_or_else(|| "no process id".to_owned())
            .and_then(|(pid, rest)| {
                let pid = pid
                    .parse()
                    .map_err(|_| format!("{pid:?} isn't a process id"))?;
                let (mode, supply) = rest.split_once('\n').ok_or("no mode")?;
                let files = match mode {
                    "plain" => None,
                    "files" => Some(FileRules::default()),
                    "files reversed" => Some(FileRules {
                        reversed: true,
                        listed: false,
                    }),
                    other => return Err(format!("{other:?} isn't a mode")),
                };
                Ok((Pid::from_raw(pid), files, Supply::decode(supply)?))
            });
        let (leader, files, supply) = match parsed {
            Ok(parsed) => parsed,
            Err(problem) => {
                say(&format!("error: {problem}"));
                return ExitCode::FAILURE;
            }
        };
        // TRACESYSGOOD tells a stop at a call's exit, where AArch64's answers land, from a
        // signal's.
        let options = Options::PTRACE_O_TRACESECCOMP
            | Options::PTRACE_O_TRACECLONE
            | Options::PTRACE_O_TRACEEXEC
            | Options::PTRACE_O_TRACESYSGOOD
            | Options::PTRACE_O_EXITKILL;
        if let Err(error) = ptrace::seize(leader, options) {
            say(&format!(
                "error: this system doesn't let lengua trace the program to supply its random bytes \
             and clock ({error}); check /proc/sys/kernel/yama/ptrace_scope"
            ));
            return ExitCode::FAILURE;
        }
        say("attached");
        let mut supervisor = Supervisor {
            leader,
            files,
            supply,
            random_used: 0,
            readings_used: 0,
            started: false,
            landing: HashMap::new(),
        };
        say(&supervisor.follow());
        ExitCode::SUCCESS
    }

    struct Supervisor {
        leader: Pid,
        /// For a program that uses files: what it holds their names to.
        files: Option<FileRules>,
        supply: Supply,
        /// Random bytes the program has asked for, supplied or past the supply.
        random_used: usize,
        readings_used: usize,
        /// Whether the launcher has become the program: its own calls before then run for real.
        started: bool,
        /// What to do at each call's exit, by the task making the call.
        landing: HashMap<Pid, Landing>,
    }

    /// What the supervisor does for a program's files.
    #[derive(Debug, Default)]
    struct FileRules {
        /// Whether it turns each folder listing round, for a test's second run.
        reversed: bool,
        /// Whether the program has listed a folder.
        listed: bool,
    }

    /// What the supervisor does at a call's exit.
    enum Landing {
        /// Puts the answer in the result register, the registers as the program left them.
        Answer(i64, Box<cpu::Registers>),
        /// Turns round the entries `getdents64` wrote into the buffer at this address.
        Reverse(u64),
    }

    /// How a call the supervisor answers is kept from doing anything in the kernel, where the CPU
    /// can't skip it: the call made with nothing to do, so it returns at once.
    #[derive(Debug, Clone, Copy)]
    pub(super) enum Harmless {
        /// `getrandom` asking for no bytes.
        ZeroCount,
        /// A clock that doesn't exist, for `clock_gettime` and `clock_nanosleep`.
        MissingClock,
        /// No path at all, for a call on a name lengua refuses: the kernel answers `EFAULT` at
        /// once, and the refusal lands.
        NullPath,
    }

    /// How the stopped task goes on.
    pub(super) enum Resume {
        /// To wherever it goes next.
        On,
        /// To its call's exit, where the answer lands.
        ToExit,
    }

    impl Supervisor {
        /// Follows the program and its threads until it ends, and says how it went.
        fn follow(&mut self) -> String {
            loop {
                let status = match waitpid(None, Some(WaitPidFlag::__WALL)) {
                    Ok(status) => status,
                    Err(Errno::EINTR) => continue,
                    Err(Errno::ECHILD) => break,
                    Err(error) => return format!("error: waiting for the program: {error}"),
                };
                match status {
                    WaitStatus::PtraceEvent(task, _, event)
                        if event == Event::PTRACE_EVENT_SECCOMP as i32 =>
                    {
                        let resume = if self.started {
                            self.answer(task)
                        } else {
                            Resume::On
                        };
                        let _ = match resume {
                            Resume::On => ptrace::cont(task, None),
                            Resume::ToExit => ptrace::syscall(task, None),
                        };
                    }
                    // A call's exit, where its answer lands.
                    WaitStatus::PtraceSyscall(task) => {
                        match self.landing.remove(&task) {
                            Some(Landing::Answer(result, left)) => cpu::land(task, result, &left),
                            Some(Landing::Reverse(buffer)) => {
                                if let Some(filled) = cpu::result(task).filter(|filled| *filled > 0)
                                {
                                    reverse_entries(task, buffer, filled);
                                }
                            }
                            None => {}
                        }
                        let _ = ptrace::cont(task, None);
                    }
                    WaitStatus::PtraceEvent(task, _, event) => {
                        if event == Event::PTRACE_EVENT_EXEC as i32 {
                            self.started = true;
                        }
                        let _ = ptrace::cont(task, None);
                    }
                    // A signal on its way to the program: deliver it, so it ends as it would alone.
                    WaitStatus::Stopped(task, signal) => {
                        let _ = ptrace::cont(task, signal);
                    }
                    WaitStatus::Exited(task, _) | WaitStatus::Signaled(task, _, _)
                        if task == self.leader =>
                    {
                        break;
                    }
                    _ => {}
                }
            }
            let listed = self.files.as_ref().is_some_and(|files| files.listed);
            format!("ok {} {}", self.random_used, u8::from(listed))
        }

        /// Answers the call `task` is stopped at in place of the kernel, or lets the kernel make
        /// it, and says how the task goes on.
        fn answer(&mut self, task: Pid) -> Resume {
            let Ok(registers) = ptrace::getregs(task) else {
                return Resume::On;
            };
            let arguments = cpu::arguments(&registers);
            let [first, second, third, ..] = arguments;
            let call = i64::try_from(cpu::number(&registers));
            if let (Ok(call), Some(files)) = (call, self.files.as_mut()) {
                if call == syscalls::GETDENTS64 {
                    files.listed = true;
                    if files.reversed {
                        self.landing.insert(task, Landing::Reverse(second));
                        return Resume::ToExit;
                    }
                    return Resume::On;
                }
                if [
                    syscalls::OPENAT,
                    syscalls::MKDIRAT,
                    syscalls::UNLINKAT,
                    syscalls::RENAMEAT,
                ]
                .contains(&call)
                {
                    let Some(errno) = check_names(task, self.leader, call, arguments) else {
                        return Resume::On;
                    };
                    return match cpu::answer(task, registers, -errno, Some(Harmless::NullPath)) {
                        Some(left) => {
                            self.landing
                                .insert(task, Landing::Answer(-errno, Box::new(left)));
                            Resume::ToExit
                        }
                        None => Resume::On,
                    };
                }
            }
            let (result, harmless) = match call {
                Ok(syscalls::GETRANDOM) => (
                    self.random(task, first, second, third),
                    Some(Harmless::ZeroCount),
                ),
                Ok(syscalls::CLOCK_GETTIME) => {
                    (self.clock(task, second), Some(Harmless::MissingClock))
                }
                Ok(syscalls::CLOCK_NANOSLEEP) => (0, Some(Harmless::MissingClock)),
                _ => (-(Errno::ENOSYS as i64), None),
            };
            match cpu::answer(task, registers, result, harmless) {
                Some(left) => {
                    self.landing
                        .insert(task, Landing::Answer(result, Box::new(left)));
                    Resume::ToExit
                }
                None => Resume::On,
            }
        }

        /// `getrandom(buffer, count, flags)`: the test's next `count` bytes, zeros past its end.
        fn random(&mut self, task: Pid, buffer: u64, count: u64, flags: u64) -> i64 {
            // GRND_NONBLOCK, GRND_RANDOM and GRND_INSECURE, but not the last two together.
            if flags & !7 != 0 || flags & 6 == 6 {
                return -(Errno::EINVAL as i64);
            }
            // The kernel's own cap on one call.
            let Some(count) = usize::try_from(count)
                .ok()
                .filter(|count| *count <= 33_554_431)
            else {
                return -(Errno::EINVAL as i64);
            };
            let bytes = self.supply.random.as_deref().unwrap_or_default();
            let mut chunk = vec![0_u8; count];
            if let Some(supplied) = bytes.get(self.random_used..) {
                let take = supplied.len().min(count);
                chunk[..take].copy_from_slice(&supplied[..take]);
            }
            match write_to(task, buffer, &chunk) {
                Ok(()) => {
                    self.random_used += count;
                    i64::try_from(count).unwrap_or(i64::MAX)
                }
                Err(errno) => -(errno as i64),
            }
        }

        /// `clock_gettime(clock, time)`: the test's next reading, the last one repeating.
        fn clock(&mut self, task: Pid, buffer: u64) -> i64 {
            let readings = &self.supply.clock;
            let Some(reading) = readings.get(self.readings_used).or_else(|| readings.last()) else {
                return -(Errno::ENOSYS as i64);
            };
            let mut time = [0_u8; 16];
            time[..8].copy_from_slice(&reading.seconds.to_le_bytes());
            time[8..].copy_from_slice(&i64::from(reading.nanoseconds).to_le_bytes());
            match write_to(task, buffer, &time) {
                Ok(()) => {
                    self.readings_used += 1;
                    0
                }
                Err(errno) => -(errno as i64),
            }
        }
    }

    /// The error a file call on a name lengua refuses gets, or `None` when the kernel may make
    /// it: `openat`, `mkdirat` and `unlinkat` give a folder and a path, `renameat` two of each.
    fn check_names(task: Pid, leader: Pid, call: i64, arguments: [u64; 6]) -> Option<i64> {
        const O_CREAT: u64 = 0o100;
        match call {
            syscalls::OPENAT => check_name(
                task,
                leader,
                arguments[0],
                arguments[1],
                arguments[2] & O_CREAT != 0,
                false,
            ),
            syscalls::MKDIRAT => check_name(task, leader, arguments[0], arguments[1], true, false),
            syscalls::UNLINKAT => check_name(task, leader, arguments[0], arguments[1], false, true),
            syscalls::RENAMEAT => check_name(task, leader, arguments[0], arguments[1], false, true)
                .or_else(|| check_name(task, leader, arguments[2], arguments[3], true, true)),
            _ => None,
        }
    }

    /// The error for the path at `address`, in the folder `folder` names: a name some system
    /// refuses; when `creates`, a name that differs only in case or normalization from another
    /// in its folder; when `removes`, a file the program holds open. `None` lets the kernel
    /// decide, as it does for a path it can't read.
    fn check_name(
        task: Pid,
        leader: Pid,
        folder: u64,
        address: u64,
        creates: bool,
        removes: bool,
    ) -> Option<i64> {
        let path = match read_path(task, address)? {
            Some(path) => path,
            None => return Some(crate::names::ENAMETOOLONG),
        };
        if let Some(broken) = crate::names::broken(&path) {
            return Some(broken.errno);
        }
        let text = std::str::from_utf8(&path).ok()?;
        let place = resolve(task, folder, text)?;
        if creates {
            let trimmed = text.trim_end_matches('/');
            let name = trimmed.rsplit('/').next().unwrap_or(trimmed);
            let within = place.parent().map(Path::to_path_buf)?;
            if let Ok(entries) = std::fs::read_dir(within) {
                for entry in entries.flatten() {
                    if let Some(other) = entry.file_name().to_str()
                        && crate::names::clashes(name, other)
                    {
                        return Some(crate::names::EEXIST);
                    }
                }
            }
        }
        if removes && held_open(leader, &place) {
            return Some(crate::names::EBUSY);
        }
        None
    }

    /// The path in `task`'s memory at `address`, up to its NUL: `Some(None)` when it runs past
    /// every system's longest, `None` when it can't be read.
    fn read_path(task: Pid, address: u64) -> Option<Option<Vec<u8>>> {
        let base = usize::try_from(address).ok().filter(|base| *base != 0)?;
        let mut bytes = Vec::new();
        // A page at a time, so a path near the end of the program's memory still reads.
        let mut at = base;
        while bytes.len() <= crate::names::MAX_PATH {
            let room = 4096 - at % 4096;
            let mut chunk = vec![0_u8; room];
            let remote = [RemoteIoVec {
                base: at,
                len: room,
            }];
            let read = process_vm_readv(task, &mut [std::io::IoSliceMut::new(&mut chunk)], &remote)
                .ok()
                .filter(|read| *read > 0)?;
            if let Some(end) = chunk[..read].iter().position(|byte| *byte == 0) {
                bytes.extend_from_slice(&chunk[..end]);
                return Some(Some(bytes));
            }
            bytes.extend_from_slice(&chunk[..read]);
            at += read;
        }
        Some(None)
    }

    /// Where `path` is for `task`, from the folder `folder` names: its working folder for
    /// `AT_FDCWD`, or a folder it holds open.
    fn resolve(task: Pid, folder: u64, path: &str) -> Option<PathBuf> {
        if path.starts_with('/') {
            return Some(PathBuf::from(path));
        }
        // The kernel reads the folder as an int.
        let folder = (folder & 0xffff_ffff) as u32 as i32;
        let base = if folder == -100 {
            PathBuf::from(format!("/proc/{task}/cwd"))
        } else {
            PathBuf::from(format!("/proc/{task}/fd/{folder}"))
        };
        Some(base.join(path))
    }

    /// Whether the program whose leader is `leader` holds the file or folder at `place` open.
    fn held_open(leader: Pid, place: &Path) -> bool {
        let Ok(target) = std::fs::symlink_metadata(place) else {
            return false;
        };
        let Ok(open) = std::fs::read_dir(format!("/proc/{leader}/fd")) else {
            return false;
        };
        open.flatten().any(|entry| {
            std::fs::metadata(entry.path())
                .is_ok_and(|held| held.dev() == target.dev() && held.ino() == target.ino())
        })
    }

    /// Turns round the folder entries `getdents64` wrote: `filled` bytes at `buffer`, each
    /// entry a record whose length is at 16, so the program reads them last to first.
    fn reverse_entries(task: Pid, buffer: u64, filled: i64) {
        let (Ok(base), Ok(length)) = (usize::try_from(buffer), usize::try_from(filled)) else {
            return;
        };
        let mut bytes = vec![0_u8; length];
        let remote = [RemoteIoVec { base, len: length }];
        if process_vm_readv(task, &mut [std::io::IoSliceMut::new(&mut bytes)], &remote).ok()
            != Some(length)
        {
            return;
        }
        let mut records = Vec::new();
        let mut at = 0;
        while at + 19 <= length {
            let size = usize::from(u16::from_le_bytes([bytes[at + 16], bytes[at + 17]]));
            if size == 0 || at + size > length {
                return;
            }
            records.push(&bytes[at..at + size]);
            at += size;
        }
        if at != length {
            return;
        }
        let turned: Vec<u8> = records
            .iter()
            .rev()
            .flat_map(|record| record.iter().copied())
            .collect();
        let _ = write_to(task, buffer, &turned);
    }

    /// Writes `bytes` into `task`'s memory at `address`, as the kernel would: only where the
    /// program may write.
    fn write_to(task: Pid, address: u64, bytes: &[u8]) -> Result<(), Errno> {
        if bytes.is_empty() {
            return Ok(());
        }
        let base = usize::try_from(address).map_err(|_| Errno::EFAULT)?;
        let remote = [RemoteIoVec {
            base,
            len: bytes.len(),
        }];
        match process_vm_writev(task, &[IoSlice::new(bytes)], &remote) {
            Ok(written) if written == bytes.len() => Ok(()),
            _ => Err(Errno::EFAULT),
        }
    }

    /// x86-64: a stopped call's number is in `orig_rax` and its arguments in `rdi`, `rsi`, `rdx`,
    /// `r10`, `r8` and `r9`; a number of -1 skips the call, its result in `rax`.
    #[cfg(target_arch = "x86_64")]
    mod cpu {
        use nix::sys::ptrace;
        use nix::unistd::Pid;

        use super::Harmless;

        pub(super) type Registers = nix::libc::user_regs_struct;

        pub(super) fn number(registers: &Registers) -> u64 {
            registers.orig_rax
        }

        pub(super) fn arguments(registers: &Registers) -> [u64; 6] {
            [
                registers.rdi,
                registers.rsi,
                registers.rdx,
                registers.r10,
                registers.r8,
                registers.r9,
            ]
        }

        /// The program sees `result` as what the call returned, and the kernel never makes it.
        /// Nothing lands later.
        pub(super) fn answer(
            task: Pid,
            mut registers: Registers,
            result: i64,
            _: Option<Harmless>,
        ) -> Option<Registers> {
            registers.orig_rax = u64::MAX;
            registers.rax = result.cast_unsigned();
            let _ = ptrace::setregs(task, registers);
            None
        }

        /// x86-64's answers never wait for a call's exit.
        pub(super) fn land(_: Pid, _: i64, _: &Registers) {}

        /// What a call returned, at its exit.
        pub(super) fn result(task: Pid) -> Option<i64> {
            ptrace::getregs(task)
                .ok()
                .map(|registers| registers.rax.cast_signed())
        }
    }

    /// AArch64: a stopped call's number is in x8 and its arguments in x0 to x5, and its result
    /// goes in x0. The kernel takes a changed number only through `NT_ARM_SYSTEM_CALL`, which
    /// lengua can't write without `unsafe`, so a call it answers goes to the kernel made harmless
    /// and returns at once; at its exit the answer lands in x0, and the one argument made
    /// harmless that isn't x0 (`getrandom`'s count, in x1) is put back. The kernel restores x7,
    /// which it uses to tell a call's entry from its exit, by itself.
    #[cfg(target_arch = "aarch64")]
    mod cpu {
        use nix::sys::ptrace;
        use nix::unistd::Pid;

        use super::Harmless;

        pub(super) type Registers = nix::libc::user_regs_struct;

        /// A clock id past the kernel's last, so `clock_gettime` and `clock_nanosleep` refuse it
        /// at once with `EINVAL`.
        const NO_CLOCK: u64 = 16;

        pub(super) fn number(registers: &Registers) -> u64 {
            registers.regs[8]
        }

        pub(super) fn arguments(registers: &Registers) -> [u64; 6] {
            let x = &registers.regs;
            [x[0], x[1], x[2], x[3], x[4], x[5]]
        }

        /// Makes the call harmless and returns the registers as the program left them, for its
        /// answer to land at the call's exit; or `None` when the call can't be made harmless, so
        /// it's the kernel's to make.
        pub(super) fn answer(
            task: Pid,
            registers: Registers,
            _: i64,
            harmless: Option<Harmless>,
        ) -> Option<Registers> {
            let mut changed = registers;
            match harmless? {
                Harmless::ZeroCount => changed.regs[1] = 0,
                Harmless::MissingClock => changed.regs[0] = NO_CLOCK,
                Harmless::NullPath => changed.regs[1] = 0,
            }
            ptrace::setregs(task, changed).ok()?;
            Some(registers)
        }

        /// What a call returned, at its exit.
        pub(super) fn result(task: Pid) -> Option<i64> {
            ptrace::getregs(task)
                .ok()
                .map(|registers| registers.regs[0].cast_signed())
        }

        /// At the call's exit: `result` in x0, and x1 as the program left it.
        pub(super) fn land(task: Pid, result: i64, left: &Registers) {
            if let Ok(mut registers) = ptrace::getregs(task) {
                registers.regs[0] = result.cast_unsigned();
                registers.regs[1] = left.regs[1];
                let _ = ptrace::setregs(task, registers);
            }
        }
    }
}
