//! Linux x86-64 system call numbers, in one place.
//!
//! The profile ([`crate::platform`]), the prover's model of the machine ([`crate::prove`]) and the
//! runtime (the ptrace supervisor [`crate::supervise`] and the sandbox [`crate::runner`]) all read
//! the numbers here, so this target's table lives once. The numbers are **not** portable: another
//! Linux CPU numbers the same calls differently (AArch64's `read` is 63, not 0), and macOS and
//! Windows don't take direct system calls at all. Each new target brings its own table — see
//! `plugin/skills/compile/references/porting.md`.

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
/// `lseek`, move a file offset.
pub const LSEEK: i64 = 8;
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
/// `shutdown`, end one direction of a connection.
pub const SHUTDOWN: i64 = 48;
/// `clone`, start a thread (only through the parallel template's flags).
pub const CLONE: i64 = 56;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers are the Linux x86-64 ABI's; pin them so any change is deliberate.
    #[test]
    fn the_numbers_are_the_linux_x86_64_abi() {
        assert_eq!(
            [
                READ,
                WRITE,
                CLOSE,
                FSTAT,
                POLL,
                LSEEK,
                IOCTL,
                SOCKET,
                CONNECT,
                ACCEPT,
                SENDTO,
                RECVFROM,
                SHUTDOWN,
                CLONE,
                EXIT,
                FUTEX,
                GETDENTS64,
                CLOCK_GETTIME,
                CLOCK_NANOSLEEP,
                EXIT_GROUP,
                OPENAT,
                MKDIRAT,
                UNLINKAT,
                RENAMEAT,
                ACCEPT4,
                GETRANDOM,
            ],
            [
                0, 1, 3, 5, 7, 8, 16, 41, 42, 43, 44, 45, 48, 56, 60, 202, 217, 228, 230, 231, 257,
                258, 263, 264, 288, 318,
            ],
        );
    }
}
