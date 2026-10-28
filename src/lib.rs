//! lengua: spec-driven programming in checked x86-64 assembly.
//!
//! A lengua program is a folder with four files: `spec.md` (the source of truth), `tests.toml`,
//! `main.s` and `lengua.lock`. This crate is the MCP server that checks, stamps and runs them.
//! See `DESIGN.md` at the repository root for every design decision.

pub mod checker;
pub mod data_dir;
pub mod expr;
pub mod hash;
pub mod lock;
pub mod platform;
pub mod prove;
pub mod runner;
pub mod server;
pub mod spec;
pub mod state;
pub mod testfile;
pub mod testrun;
pub mod toolchain;
