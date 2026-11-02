//! lengua: an AI language. A person's spec, in behavior-driven natural language with its tests
//! inside, is compiled by the AI agent into checked x86-64 assembly.
//!
//! A lengua program is a folder with two files, `spec.md` (the source of truth, tests included)
//! and `main.s` (the code compiled from it for this computer), plus `lengua.lock`, this
//! computer's signed record of what it checked. This crate is the MCP server that checks, stamps
//! and runs them. See `DESIGN.md` at the repository root for every design decision.

pub mod checker;
pub mod connect;
pub mod data_dir;
pub mod expect;
pub mod expr;
pub mod hash;
pub mod header;
pub mod isa;
pub mod lock;
pub mod names;
pub mod platform;
pub mod prove;
pub mod runner;
pub mod server;
pub mod spec;
pub mod state;
pub mod supervise;
pub mod supply;
pub mod syscalls;
pub mod testdiff;
pub mod testfile;
pub mod testrun;
pub mod toolchain;
