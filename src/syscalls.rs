//! Linux's edge with a program, per CPU: the system call numbers and the details of Linux's ABI
//! that differ between CPUs, one module each.
//!
//! The profile ([`crate::platform`]), the prover's model of the machine ([`crate::prove`]) and the
//! runtime (the sandbox [`crate::runner`]) read them here, so each target's table lives once;
//! [`host`] is this computer's, for the runtime.
//! The numbers aren't portable: AArch64's `read` is 63 and x86-64's 0, and AArch64 has no `poll`,
//! only `ppoll`. macOS and Windows take no direct system calls at all. Each table is pinned
//! against the kernel's headers and the C library, where this computer has them. See
//! `plugin/skills/compile/references/porting.md`.

/// How a program waits until one of its descriptors is ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    /// `poll(fds, count, milliseconds)`.
    Poll,
    /// `ppoll(fds, count, timespec, signal mask, mask size)`, the only one AArch64 has.
    Ppoll,
}

/// The details of Linux's ABI that the prover models and that differ between CPUs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxAbi {
    /// The bytes of `struct stat`.
    pub stat_len: u64,
    /// Where in it `st_mode` is: a file's kind and permissions.
    pub stat_mode: u64,
    /// Where in it `st_size` is: a file's size in bytes.
    pub stat_size: u64,
    /// `O_DIRECTORY`: `openat` opens a folder or fails.
    pub o_directory: u64,
    /// The `openat` flags that change nothing the prover's model tracks: no controlling
    /// terminal, large files, closing on exec.
    pub o_harmless: u64,
    /// How a program waits for its descriptors.
    pub wait: Wait,
}

/// Linux on x86-64.
pub mod x86_64 {
    use super::{LinuxAbi, Wait};
    use crate::idea::Syscall;

    /// `read` from a file descriptor.
    pub const READ: i64 = 0;
    /// `write` to a file descriptor.
    pub const WRITE: i64 = 1;
    /// `close` a file descriptor.
    pub const CLOSE: i64 = 3;
    /// `fstat`, an open file's size and kind.
    pub const FSTAT: i64 = 5;
    /// `poll`, a wait until a descriptor is ready.
    pub const POLL: i64 = 7;
    /// `ppoll`, the same wait with a time as seconds and nanoseconds: no x86-64 program's, but
    /// the call QEMU waits in for an AArch64 program's `ppoll` here.
    pub const PPOLL: i64 = 271;
    /// The calls a process waits in for descriptors to be ready.
    pub const WAITS: [i64; 2] = [POLL, PPOLL];
    /// `lseek`, move a file offset.
    pub const LSEEK: i64 = 8;
    /// `rt_sigaction`, which the launcher makes between confining itself and becoming the
    /// program.
    pub const RT_SIGACTION: i64 = 13;
    /// `rt_sigprocmask`, which the launcher makes too.
    pub const RT_SIGPROCMASK: i64 = 14;
    /// `ioctl`, read or set a terminal's mode.
    pub const IOCTL: i64 = 16;
    /// `socket`, open a TCP socket.
    pub const SOCKET: i64 = 41;
    /// `connect` a socket to a port.
    pub const CONNECT: i64 = 42;
    /// `accept`, take the next connection.
    pub const ACCEPT: i64 = 43;
    /// `sendto`, send on a connection.
    pub const SENDTO: i64 = 44;
    /// `recvfrom`, receive on a connection.
    pub const RECVFROM: i64 = 45;
    /// `sendmsg`, send a message with a descriptor: the edge's only way to talk to the broker.
    pub const SENDMSG: i64 = 46;
    /// `recvmsg`, receive a message and the descriptor that came with it.
    pub const RECVMSG: i64 = 47;
    /// `shutdown`, end one direction of a connection.
    pub const SHUTDOWN: i64 = 48;
    /// `clone`, start a thread (only through the parallel template's flags).
    pub const CLONE: i64 = 56;
    /// `execve`, the launcher becoming the program.
    pub const EXECVE: i64 = 59;
    /// `exit`, end the calling thread.
    pub const EXIT: i64 = 60;
    /// `futex`, wait on a word (the join after fork-join).
    pub const FUTEX: i64 = 202;
    /// `getdents64`, read a folder's entries.
    pub const GETDENTS64: i64 = 217;
    /// `clock_gettime`, read a clock.
    pub const CLOCK_GETTIME: i64 = 228;
    /// `clock_nanosleep`, sleep for or until a time.
    pub const CLOCK_NANOSLEEP: i64 = 230;
    /// `exit_group`, end the program (all threads).
    pub const EXIT_GROUP: i64 = 231;
    /// `openat`, open a file relative to a directory.
    pub const OPENAT: i64 = 257;
    /// `mkdirat`, make a folder.
    pub const MKDIRAT: i64 = 258;
    /// `unlinkat`, delete a file or empty folder.
    pub const UNLINKAT: i64 = 263;
    /// `renameat`, rename or move a file or folder.
    pub const RENAMEAT: i64 = 264;
    /// `accept4`, take the next connection with flags.
    pub const ACCEPT4: i64 = 288;
    /// `getrandom`, fill a buffer with random bytes.
    pub const GETRANDOM: i64 = 318;

    /// The ABI's details: x86-64 has its own `struct stat` and `O_DIRECTORY`.
    pub const ABI: LinuxAbi = LinuxAbi {
        stat_len: 144,
        stat_mode: 24,
        stat_size: 48,
        o_directory: 0o200_000,
        o_harmless: 0o400 | 0o100_000 | 0o2_000_000,
        wait: Wait::Poll,
    };

    /// The number a program makes `syscall` with.
    #[must_use]
    pub const fn number(syscall: Syscall) -> i64 {
        match syscall {
            Syscall::Read => READ,
            Syscall::Write => WRITE,
            Syscall::Openat => OPENAT,
            Syscall::Close => CLOSE,
            Syscall::Lseek => LSEEK,
            Syscall::Fstat => FSTAT,
            Syscall::Getdents64 => GETDENTS64,
            Syscall::Mkdirat => MKDIRAT,
            Syscall::Unlinkat => UNLINKAT,
            Syscall::Renameat => RENAMEAT,
            Syscall::Exit => EXIT,
            Syscall::ExitGroup => EXIT_GROUP,
            Syscall::ClockGettime => CLOCK_GETTIME,
            Syscall::ClockNanosleep => CLOCK_NANOSLEEP,
            Syscall::Getrandom => GETRANDOM,
            Syscall::Accept4 => ACCEPT4,
            Syscall::Shutdown => SHUTDOWN,
            Syscall::Sendto => SENDTO,
            Syscall::Poll => POLL,
            Syscall::Ioctl => IOCTL,
        }
    }
}

/// Linux on AArch64: the kernel's generic table, with `renameat` and the older `fstat` kept.
pub mod aarch64 {
    use super::{LinuxAbi, Wait};
    use crate::idea::Syscall;

    /// `ioctl`, read or set a terminal's mode.
    pub const IOCTL: i64 = 29;
    /// `mkdirat`, make a folder.
    pub const MKDIRAT: i64 = 34;
    /// `unlinkat`, delete a file or empty folder.
    pub const UNLINKAT: i64 = 35;
    /// `renameat`, rename or move a file or folder.
    pub const RENAMEAT: i64 = 38;
    /// `openat`, open a file relative to a directory.
    pub const OPENAT: i64 = 56;
    /// `close` a file descriptor.
    pub const CLOSE: i64 = 57;
    /// `getdents64`, read a folder's entries.
    pub const GETDENTS64: i64 = 61;
    /// `lseek`, move a file offset.
    pub const LSEEK: i64 = 62;
    /// `read` from a file descriptor.
    pub const READ: i64 = 63;
    /// `write` to a file descriptor.
    pub const WRITE: i64 = 64;
    /// `ppoll`, a wait until a descriptor is ready, for a time a `timespec` gives.
    pub const PPOLL: i64 = 73;
    /// The calls a process waits in for descriptors to be ready: `ppoll` alone.
    pub const WAITS: [i64; 1] = [PPOLL];
    /// `fstat`, an open file's size and kind.
    pub const FSTAT: i64 = 80;
    /// `exit`, end the calling thread.
    pub const EXIT: i64 = 93;
    /// `exit_group`, end the program (all threads).
    pub const EXIT_GROUP: i64 = 94;
    /// `futex`, wait on a word (the join after fork-join).
    pub const FUTEX: i64 = 98;
    /// `clock_gettime`, read a clock.
    pub const CLOCK_GETTIME: i64 = 113;
    /// `clock_nanosleep`, sleep for or until a time.
    pub const CLOCK_NANOSLEEP: i64 = 115;
    /// `rt_sigaction`, which the launcher makes between confining itself and becoming the
    /// program.
    pub const RT_SIGACTION: i64 = 134;
    /// `rt_sigprocmask`, which the launcher makes too.
    pub const RT_SIGPROCMASK: i64 = 135;
    /// `socket`, open a TCP socket.
    pub const SOCKET: i64 = 198;
    /// `accept`, take the next connection.
    pub const ACCEPT: i64 = 202;
    /// `connect` a socket to a port.
    pub const CONNECT: i64 = 203;
    /// `sendto`, send on a connection.
    pub const SENDTO: i64 = 206;
    /// `recvfrom`, receive on a connection.
    pub const RECVFROM: i64 = 207;
    /// `sendmsg`, send a message with a descriptor: the edge's only way to talk to the broker.
    pub const SENDMSG: i64 = 211;
    /// `recvmsg`, receive a message and the descriptor that came with it.
    pub const RECVMSG: i64 = 212;
    /// `shutdown`, end one direction of a connection.
    pub const SHUTDOWN: i64 = 210;
    /// `clone`, start a thread.
    pub const CLONE: i64 = 220;
    /// `execve`, the launcher becoming the program.
    pub const EXECVE: i64 = 221;
    /// `accept4`, take the next connection with flags.
    pub const ACCEPT4: i64 = 242;
    /// `getrandom`, fill a buffer with random bytes.
    pub const GETRANDOM: i64 = 278;

    /// The ABI's details: the generic `struct stat` and `O_*` flags, and only `ppoll`.
    pub const ABI: LinuxAbi = LinuxAbi {
        stat_len: 128,
        stat_mode: 16,
        stat_size: 48,
        o_directory: 0o40_000,
        o_harmless: 0o400 | 0o400_000 | 0o2_000_000,
        wait: Wait::Ppoll,
    };

    /// The number a program makes `syscall` with: `ppoll` for a wait.
    #[must_use]
    pub const fn number(syscall: Syscall) -> i64 {
        match syscall {
            Syscall::Read => READ,
            Syscall::Write => WRITE,
            Syscall::Openat => OPENAT,
            Syscall::Close => CLOSE,
            Syscall::Lseek => LSEEK,
            Syscall::Fstat => FSTAT,
            Syscall::Getdents64 => GETDENTS64,
            Syscall::Mkdirat => MKDIRAT,
            Syscall::Unlinkat => UNLINKAT,
            Syscall::Renameat => RENAMEAT,
            Syscall::Exit => EXIT,
            Syscall::ExitGroup => EXIT_GROUP,
            Syscall::ClockGettime => CLOCK_GETTIME,
            Syscall::ClockNanosleep => CLOCK_NANOSLEEP,
            Syscall::Getrandom => GETRANDOM,
            Syscall::Accept4 => ACCEPT4,
            Syscall::Shutdown => SHUTDOWN,
            Syscall::Sendto => SENDTO,
            Syscall::Poll => PPOLL,
            Syscall::Ioctl => IOCTL,
        }
    }
}

/// Whether `number` is `socket` or `connect` on `profile`'s CPU: the calls a program never makes
/// itself, since lengua makes its connections and hands them over.
#[must_use]
pub fn connects(profile: crate::platform::Profile, number: i64) -> bool {
    let [socket, connect] = match profile {
        crate::platform::Profile::LinuxX86_64 => [x86_64::SOCKET, x86_64::CONNECT],
        crate::platform::Profile::LinuxAarch64 => [aarch64::SOCKET, aarch64::CONNECT],
    };
    number == socket || number == connect
}

/// This computer's table, for the runtime, which runs programs here.
#[cfg(target_arch = "x86_64")]
pub use x86_64 as host;

/// This computer's table, for the runtime, which runs programs here.
#[cfg(target_arch = "aarch64")]
pub use aarch64 as host;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::idea::Syscall;

    /// `#define __NR_name number` (and the generic table's `__NR3264_name`) in a kernel header,
    /// when this computer has it.
    fn header_numbers(path: &str) -> Option<BTreeMap<String, i64>> {
        let text = std::fs::read_to_string(path).ok()?;
        Some(
            text.lines()
                .filter_map(|line| {
                    let mut words = line.split_whitespace();
                    if words.next()? != "#define" {
                        return None;
                    }
                    let name = words.next()?;
                    let name = name
                        .strip_prefix("__NR3264_")
                        .or_else(|| name.strip_prefix("__NR_"))?;
                    let number = words.next()?.parse().ok()?;
                    Some((name.to_owned(), number))
                })
                .collect(),
        )
    }

    /// Every call lengua's programs and launcher make, with each table's number.
    fn calls() -> Vec<(&'static str, i64, i64)> {
        vec![
            ("read", x86_64::READ, aarch64::READ),
            ("write", x86_64::WRITE, aarch64::WRITE),
            ("close", x86_64::CLOSE, aarch64::CLOSE),
            ("fstat", x86_64::FSTAT, aarch64::FSTAT),
            ("lseek", x86_64::LSEEK, aarch64::LSEEK),
            ("rt_sigaction", x86_64::RT_SIGACTION, aarch64::RT_SIGACTION),
            (
                "rt_sigprocmask",
                x86_64::RT_SIGPROCMASK,
                aarch64::RT_SIGPROCMASK,
            ),
            ("ioctl", x86_64::IOCTL, aarch64::IOCTL),
            ("socket", x86_64::SOCKET, aarch64::SOCKET),
            ("connect", x86_64::CONNECT, aarch64::CONNECT),
            ("accept", x86_64::ACCEPT, aarch64::ACCEPT),
            ("sendto", x86_64::SENDTO, aarch64::SENDTO),
            ("recvfrom", x86_64::RECVFROM, aarch64::RECVFROM),
            ("sendmsg", x86_64::SENDMSG, aarch64::SENDMSG),
            ("recvmsg", x86_64::RECVMSG, aarch64::RECVMSG),
            ("shutdown", x86_64::SHUTDOWN, aarch64::SHUTDOWN),
            ("clone", x86_64::CLONE, aarch64::CLONE),
            ("execve", x86_64::EXECVE, aarch64::EXECVE),
            ("exit", x86_64::EXIT, aarch64::EXIT),
            ("futex", x86_64::FUTEX, aarch64::FUTEX),
            ("getdents64", x86_64::GETDENTS64, aarch64::GETDENTS64),
            (
                "clock_gettime",
                x86_64::CLOCK_GETTIME,
                aarch64::CLOCK_GETTIME,
            ),
            (
                "clock_nanosleep",
                x86_64::CLOCK_NANOSLEEP,
                aarch64::CLOCK_NANOSLEEP,
            ),
            ("exit_group", x86_64::EXIT_GROUP, aarch64::EXIT_GROUP),
            ("openat", x86_64::OPENAT, aarch64::OPENAT),
            ("mkdirat", x86_64::MKDIRAT, aarch64::MKDIRAT),
            ("unlinkat", x86_64::UNLINKAT, aarch64::UNLINKAT),
            ("renameat", x86_64::RENAMEAT, aarch64::RENAMEAT),
            ("accept4", x86_64::ACCEPT4, aarch64::ACCEPT4),
            ("getrandom", x86_64::GETRANDOM, aarch64::GETRANDOM),
        ]
    }

    /// The x86-64 numbers, pinned so any change is deliberate, and held to the kernel's x86-64
    /// header where this computer has it.
    #[test]
    fn the_x86_64_numbers_are_the_kernels() {
        assert_eq!(
            calls().iter().map(|(.., x86, _)| *x86).collect::<Vec<_>>(),
            [
                0, 1, 3, 5, 8, 13, 14, 16, 41, 42, 43, 44, 45, 46, 47, 48, 56, 59, 60, 202, 217,
                228, 230, 231, 257, 258, 263, 264, 288, 318
            ]
        );
        assert_eq!(x86_64::POLL, 7);
        if let Some(header) = header_numbers("/usr/include/asm/unistd_64.h") {
            for (name, x86, _) in calls() {
                assert_eq!(header.get(name), Some(&x86), "{name}");
            }
            assert_eq!(header.get("poll"), Some(&x86_64::POLL));
        }
    }

    /// The AArch64 numbers, held to the kernel's generic header, which AArch64 uses, where this
    /// computer has it: it has `renameat` and the older `fstat`, and no `poll`.
    #[test]
    fn the_aarch64_numbers_are_the_kernels() {
        assert_eq!(
            calls().iter().map(|(.., arm)| *arm).collect::<Vec<_>>(),
            [
                63, 64, 57, 80, 62, 134, 135, 29, 198, 203, 202, 206, 207, 211, 212, 210, 220, 221,
                93, 98, 61, 113, 115, 94, 56, 34, 35, 38, 242, 278
            ]
        );
        assert_eq!(aarch64::PPOLL, 73);
        if let Some(header) = header_numbers("/usr/include/asm-generic/unistd.h") {
            for (name, _, arm) in calls() {
                assert_eq!(header.get(name), Some(&arm), "{name}");
            }
            assert_eq!(header.get("ppoll"), Some(&aarch64::PPOLL));
            assert_eq!(header.get("poll"), None);
        }
    }

    /// Each table maps every call an idea may allow, to a number of its own.
    #[test]
    fn every_call_a_idea_allows_has_a_number() {
        for number in [x86_64::number, aarch64::number] {
            let mut numbers: Vec<i64> = Syscall::ALL.into_iter().map(number).collect();
            numbers.sort_unstable();
            numbers.dedup();
            assert_eq!(numbers.len(), Syscall::ALL.len());
        }
    }

    /// This computer's table and ABI are the C library's view of this kernel.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_host_table_is_the_c_librarys() {
        use nix::libc;
        let numbers = [
            (host::READ, libc::SYS_read),
            (host::WRITE, libc::SYS_write),
            (host::CLOSE, libc::SYS_close),
            (host::FSTAT, libc::SYS_fstat),
            (host::LSEEK, libc::SYS_lseek),
            (host::OPENAT, libc::SYS_openat),
            (host::GETDENTS64, libc::SYS_getdents64),
            (host::RENAMEAT, libc::SYS_renameat),
            (host::CLONE, libc::SYS_clone),
            (host::EXECVE, libc::SYS_execve),
            (host::SENDMSG, libc::SYS_sendmsg),
            (host::RECVMSG, libc::SYS_recvmsg),
            (host::FUTEX, libc::SYS_futex),
            (host::GETRANDOM, libc::SYS_getrandom),
            (host::CLOCK_NANOSLEEP, libc::SYS_clock_nanosleep),
            (host::ACCEPT4, libc::SYS_accept4),
            (host::PPOLL, libc::SYS_ppoll),
            (host::RT_SIGPROCMASK, libc::SYS_rt_sigprocmask),
        ];
        for (ours, theirs) in numbers {
            assert_eq!(ours, theirs);
        }
        let abi = host::ABI;
        assert_eq!(abi.stat_len, size_of::<libc::stat>() as u64);
        assert_eq!(
            abi.stat_mode,
            std::mem::offset_of!(libc::stat, st_mode) as u64
        );
        assert_eq!(
            abi.stat_size,
            std::mem::offset_of!(libc::stat, st_size) as u64
        );
        assert_eq!(abi.o_directory, libc::O_DIRECTORY as u64);
        assert_eq!(
            abi.o_harmless & (libc::O_NOCTTY | libc::O_CLOEXEC) as u64,
            (libc::O_NOCTTY | libc::O_CLOEXEC) as u64
        );
    }
}
