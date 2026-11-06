//! lengua: an AI language. A person's idea, in behavior-driven natural language with its tests
//! inside, is compiled by the AI agent into checked assembly for the target's CPU, x86-64 or
//! AArch64.
//!
//! A lengua program is a folder with two files, `idea.md` (the source of truth, tests included)
//! and `main.s` (the code compiled from it for this computer), plus `lengua.lock`, this
//! computer's signed record of what it checked. This crate is the MCP server that checks, stamps
//! and runs them. See `DESIGN.md` at the repository root for every design decision.

// A program's launch, confinement and counters exist only on Linux, the only system lengua runs
// programs on: elsewhere, what only they use is dead.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

pub mod aes;
pub mod broker;
pub mod checker;
pub mod connect;
pub mod data_dir;
pub mod edge;
pub mod expect;
pub mod expr;
#[cfg(test)]
mod fuzz;
pub mod hash;
pub mod header;
pub mod idea;
pub mod isa;
pub mod lock;
pub mod names;
mod os;
pub mod platform;
pub mod prove;
pub mod runner;
pub mod server;
pub mod skiplog;
pub mod state;
pub mod supply;
pub mod syscalls;
pub mod targets;
pub mod testdiff;
pub mod testfile;
pub mod testrun;
pub mod toolchain;
