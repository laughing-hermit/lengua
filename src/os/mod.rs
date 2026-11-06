//! What differs from one operating system to the next in how the server runs a program: how it is
//! started, confined and counted. Each system has a module of its own with the same four things,
//! which [`crate::runner`] re-exports: [`Started`], a program that is running, and `start`,
//! `attached` and `launch`, which begin a run, run one from a shell, and are the launcher itself.
//!
//! Linux has the kernel's confinement, and is the only system lengua runs programs on: Windows
//! reaches it through WSL 2 and a Mac through a Linux VM or container. Any other system says so
//! when asked to run a program.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod unsupported;

#[cfg(target_os = "linux")]
pub use linux::{Started, attached, launch, start};
#[cfg(not(target_os = "linux"))]
pub use unsupported::{Started, attached, launch, start};
