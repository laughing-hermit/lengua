//! What differs from one operating system to the next in how the server runs a program: how it is
//! started, confined and counted. Each system has a module of its own with the same four things,
//! which [`crate::runner`] re-exports: [`Started`], a program that is running, and `start`,
//! `attached` and `launch`, which begin a run, run one from a shell, and are the launcher itself.
//!
//! Linux has the kernel's confinement for native programs. macOS and Windows run no native
//! program, and say so when asked to; their programs are WebAssembly, which the pivot in todo.md
//! brings to every system. Checking and proving a program don't run it, and work everywhere.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod unsupported;

#[cfg(target_os = "linux")]
pub use linux::{Started, attached, launch, start};
#[cfg(not(target_os = "linux"))]
pub use unsupported::{Started, attached, launch, start};
