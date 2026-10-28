//! lengua: spec-driven programming in checked x86-64 assembly.
//!
//! A lengua program is a folder with four files: `spec.md` (the source of truth), `tests.toml`,
//! `main.s` and `lengua.lock`. This crate is the MCP server that checks, stamps and runs them.
//! See `DESIGN.md` at the repository root for every design decision.

pub mod data_dir;
pub mod platform;
pub mod server;
pub mod toolchain;
