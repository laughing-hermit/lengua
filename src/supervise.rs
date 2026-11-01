//! The test supervisor. When a test supplies random bytes or clock readings, a second process of
//! this binary traces the launcher, and so the program it becomes, and answers each `getrandom`
//! and `clock_gettime` call with the test's values instead of the kernel's. The launcher's
//! seccomp filter stops only those two calls; everything else runs as in any test, including
//! the counters, the exit status and the time limit.
//!
//! Bytes asked for past the end of the supply are zeros, so a program may draw in batches of any
//! size: only the bytes it uses decide its output. The last clock reading repeats.
//!
//! For a program that may reach only this computer, it also reads where each `connect` goes and
//! refuses any other computer with `EACCES`, as the kernel refuses a port the spec doesn't name:
//! the kernel's network rules know ports, not computers.
//!
//! It reads the launcher's process id, `local` or `any`, and the supply on standard input, says
//! `attached` once it traces the launcher, and when the program ends says `ok` and how many
//! random bytes the program asked for, or what went wrong.

use std::io::Write as _;
use std::process::ExitCode;

/// The command-line flag that starts the binary as a supervisor.
pub const SUPERVISE_FLAG: &str = "--supervise";

/// Whether the supervisor runs on this computer's CPU: it reads and rewrites a stopped call's
/// registers, x86-64's so far. Where it doesn't, tests that need it aren't run, and a program
/// that may connect only to this computer doesn't run either.
pub const AVAILABLE: bool = cfg!(target_arch = "x86_64");

fn say(line: &str) {
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{line}");
    let _ = stdout.flush();
}

/// Supervisor mode: traces the launcher named on standard input until its program ends.
#[must_use]
pub fn supervise() -> ExitCode {
    #[cfg(target_arch = "x86_64")]
    {
        traced::supervise()
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        say(&format!(
            "error: lengua's supervisor doesn't run on {} yet",
            std::env::consts::ARCH
        ));
        ExitCode::FAILURE
    }
}

/// The supervisor itself, for a CPU whose registers it reads.
#[cfg(target_arch = "x86_64")]
mod traced {
    use std::io::{IoSlice, Read as _};
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
                let (reach, supply) = rest.split_once('\n').ok_or("no reach")?;
                let local = match reach {
                    "local" => true,
                    "any" => false,
                    other => return Err(format!("{other:?} isn't a reach")),
                };
                Ok((Pid::from_raw(pid), local, Supply::decode(supply)?))
            });
        let (leader, local, supply) = match parsed {
            Ok(parsed) => parsed,
            Err(problem) => {
                say(&format!("error: {problem}"));
                return ExitCode::FAILURE;
            }
        };
        let options = Options::PTRACE_O_TRACESECCOMP
            | Options::PTRACE_O_TRACECLONE
            | Options::PTRACE_O_TRACEEXEC
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
            local,
            supply,
            random_used: 0,
            readings_used: 0,
            started: false,
        };
        say(&supervisor.follow());
        ExitCode::SUCCESS
    }

    struct Supervisor {
        leader: Pid,
        /// Whether the program may connect only to this computer.
        local: bool,
        supply: Supply,
        /// Random bytes the program has asked for, supplied or past the supply.
        random_used: usize,
        readings_used: usize,
        /// Whether the launcher has become the program: its own calls before then run for real.
        started: bool,
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
                        if self.started {
                            self.answer(task);
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
            format!("ok {}", self.random_used)
        }

        /// Answers the call `task` is stopped at, skipping the kernel's, or lets the kernel make it.
        fn answer(&mut self, task: Pid) {
            let Ok(mut registers) = ptrace::getregs(task) else {
                return;
            };
            let result = match i64::try_from(registers.orig_rax) {
                Ok(syscalls::GETRANDOM) => {
                    self.random(task, registers.rdi, registers.rsi, registers.rdx)
                }
                Ok(syscalls::CLOCK_GETTIME) => self.clock(task, registers.rsi),
                Ok(syscalls::CLOCK_NANOSLEEP) => 0,
                Ok(syscalls::CONNECT) if self.local => {
                    match reaches_this_computer(task, registers.rsi) {
                        true => return,
                        false => -(Errno::EACCES as i64),
                    }
                }
                _ => -(Errno::ENOSYS as i64),
            };
            // A call number of -1 skips the call; the program sees `result` as what it returned.
            registers.orig_rax = u64::MAX;
            registers.rax = result.cast_unsigned();
            let _ = ptrace::setregs(task, registers);
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

    /// Whether the socket address at `address` in `task`'s memory is this computer: 127.0.0.0/8 in
    /// IPv4, and `::1` or an IPv4 one mapped into IPv6. An address that can't be read isn't.
    fn reaches_this_computer(task: Pid, address: u64) -> bool {
        let mut bytes = [0_u8; 24];
        let Ok(base) = usize::try_from(address) else {
            return false;
        };
        let remote = [RemoteIoVec {
            base,
            len: bytes.len(),
        }];
        // An IPv4 address is 16 bytes, an IPv6 one 28; the first 24 hold every byte checked.
        let read = process_vm_readv(task, &mut [std::io::IoSliceMut::new(&mut bytes)], &remote)
            .unwrap_or(0);
        let family = u16::from_le_bytes([bytes[0], bytes[1]]);
        match family {
            2 if read >= 8 => bytes[4] == 127,
            10 if read >= 24 => {
                let host = &bytes[8..24];
                let loopback = host[..15].iter().all(|byte| *byte == 0) && host[15] == 1;
                let mapped = host[..10].iter().all(|byte| *byte == 0)
                    && host[10..12] == [0xff, 0xff]
                    && host[12] == 127;
                loopback || mapped
            }
            _ => false,
        }
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
}
