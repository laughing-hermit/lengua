//! The system calls the prover follows, by what they do, and where each CPU keeps a call's
//! number and arguments. The numbers come from each CPU's table in [`crate::syscalls`].

use crate::isa::Isa;
use crate::syscalls::{aarch64, x86_64};

/// What a system call does, whatever its number on the CPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Call {
    /// `read`.
    Read,
    /// `write`.
    Write,
    /// `close`.
    Close,
    /// `fstat`.
    Fstat,
    /// `poll`, which AArch64 doesn't have.
    Poll,
    /// `ppoll`, AArch64's only wait: `poll`'s table and count, then how long as a pointer to
    /// seconds and nanoseconds, none meaning until one is ready, and a signal mask.
    Ppoll,
    /// `ioctl`.
    Ioctl,
    /// `socket`.
    Socket,
    /// `connect`.
    Connect,
    /// `accept`.
    Accept,
    /// `accept4`.
    Accept4,
    /// `sendto`.
    Sendto,
    /// `recvfrom`.
    Recvfrom,
    /// `shutdown`.
    Shutdown,
    /// `clone`.
    Clone,
    /// `exit`.
    Exit,
    /// `exit_group`.
    ExitGroup,
    /// `futex`.
    Futex,
    /// `getdents64`.
    Getdents64,
    /// `clock_gettime`.
    ClockGettime,
    /// `clock_nanosleep`.
    ClockNanosleep,
    /// `openat`.
    Openat,
    /// `mkdirat`.
    Mkdirat,
    /// `unlinkat`.
    Unlinkat,
    /// `renameat`.
    Renameat,
    /// `getrandom`.
    Getrandom,
}

impl Call {
    /// The call `number` makes on `isa`, when the prover follows it.
    pub(super) fn of(isa: Isa, number: i64) -> Option<Self> {
        let table: &[(i64, Self)] = match isa {
            Isa::X86_64 => &[
                (x86_64::READ, Self::Read),
                (x86_64::WRITE, Self::Write),
                (x86_64::CLOSE, Self::Close),
                (x86_64::FSTAT, Self::Fstat),
                (x86_64::POLL, Self::Poll),
                (x86_64::IOCTL, Self::Ioctl),
                (x86_64::SOCKET, Self::Socket),
                (x86_64::CONNECT, Self::Connect),
                (x86_64::ACCEPT, Self::Accept),
                (x86_64::ACCEPT4, Self::Accept4),
                (x86_64::SENDTO, Self::Sendto),
                (x86_64::RECVFROM, Self::Recvfrom),
                (x86_64::SHUTDOWN, Self::Shutdown),
                (x86_64::CLONE, Self::Clone),
                (x86_64::EXIT, Self::Exit),
                (x86_64::EXIT_GROUP, Self::ExitGroup),
                (x86_64::FUTEX, Self::Futex),
                (x86_64::GETDENTS64, Self::Getdents64),
                (x86_64::CLOCK_GETTIME, Self::ClockGettime),
                (x86_64::CLOCK_NANOSLEEP, Self::ClockNanosleep),
                (x86_64::OPENAT, Self::Openat),
                (x86_64::MKDIRAT, Self::Mkdirat),
                (x86_64::UNLINKAT, Self::Unlinkat),
                (x86_64::RENAMEAT, Self::Renameat),
                (x86_64::GETRANDOM, Self::Getrandom),
            ],
            // No `poll`: AArch64 waits with `ppoll`.
            Isa::Aarch64 => &[
                (aarch64::PPOLL, Self::Ppoll),
                (aarch64::READ, Self::Read),
                (aarch64::WRITE, Self::Write),
                (aarch64::CLOSE, Self::Close),
                (aarch64::FSTAT, Self::Fstat),
                (aarch64::IOCTL, Self::Ioctl),
                (aarch64::SOCKET, Self::Socket),
                (aarch64::CONNECT, Self::Connect),
                (aarch64::ACCEPT, Self::Accept),
                (aarch64::ACCEPT4, Self::Accept4),
                (aarch64::SENDTO, Self::Sendto),
                (aarch64::RECVFROM, Self::Recvfrom),
                (aarch64::SHUTDOWN, Self::Shutdown),
                (aarch64::CLONE, Self::Clone),
                (aarch64::EXIT, Self::Exit),
                (aarch64::EXIT_GROUP, Self::ExitGroup),
                (aarch64::FUTEX, Self::Futex),
                (aarch64::GETDENTS64, Self::Getdents64),
                (aarch64::CLOCK_GETTIME, Self::ClockGettime),
                (aarch64::CLOCK_NANOSLEEP, Self::ClockNanosleep),
                (aarch64::OPENAT, Self::Openat),
                (aarch64::MKDIRAT, Self::Mkdirat),
                (aarch64::UNLINKAT, Self::Unlinkat),
                (aarch64::RENAMEAT, Self::Renameat),
                (aarch64::GETRANDOM, Self::Getrandom),
            ],
        };
        table
            .iter()
            .find(|(known, _)| *known == number)
            .map(|(_, call)| *call)
    }
}

/// Where a CPU keeps a system call's number and arguments, and its stack pointer, as the slots
/// of the prover's registers.
#[derive(Debug)]
pub(super) struct Convention {
    /// The call's number.
    pub(super) number: usize,
    /// Its six arguments, in order.
    pub(super) arguments: [usize; 6],
    /// The stack pointer, whose owner is the function's own frame.
    pub(super) stack: usize,
    /// Which argument of `clone` is where the kernel clears the thread's id at its end.
    pub(super) child_tid: usize,
    /// The registers the call instruction itself changes, besides the result: x86-64's `syscall`
    /// leaves the return address in rcx and the flags in r11.
    pub(super) clobbered: Option<(usize, usize)>,
}

impl Convention {
    /// `isa`'s.
    pub(super) const fn of(isa: Isa) -> &'static Self {
        match isa {
            Isa::X86_64 => &X86_64,
            Isa::Aarch64 => &AARCH64,
        }
    }
}

/// x86-64: the number in rax, the arguments in rdi, rsi, rdx, r10, r8 and r9.
const X86_64: Convention = Convention {
    number: 0,
    arguments: [7, 6, 2, 10, 8, 9],
    stack: 4,
    child_tid: 3,
    clobbered: Some((1, 11)),
};

/// AArch64: the number in x8, the arguments in x0 to x5; sp is slot 31.
const AARCH64: Convention = Convention {
    number: 8,
    arguments: [0, 1, 2, 3, 4, 5],
    stack: 31,
    child_tid: 4,
    clobbered: None,
};
