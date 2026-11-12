//! lengua: an AI language. A person's idea, in behavior-driven natural language with its tests
//! inside, is compiled by the AI agent into checked WebAssembly, which lengua runs the same way on
//! every computer.
//!
//! A lengua program is a folder with two files, `idea.md` (the source of truth, tests included,
//! an Open Knowledge Format Attested Computation) and `main.wat` (the WebAssembly text compiled
//! from it), plus `lengua.lock`, this computer's signed record of what it checked. This crate is
//! the MCP server that checks, stamps and runs them. See `DESIGN.md` at the repository root for
//! every design decision.

// What only Linux uses, such as a proof worker's lowered priority, is dead on the other systems.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

pub mod aes;
pub mod call;
pub mod checker;
pub mod connect;
pub mod data_dir;
pub mod doctor;
pub mod expect;
pub mod expr;
#[cfg(test)]
mod fuzz;
pub mod gate;
pub mod graph;
pub mod hash;
pub mod idea;
pub mod install;
pub mod lock;
pub mod lsp;
pub mod measure;
pub mod named;
pub mod names;
pub mod platform;
pub mod prove;
pub mod reach;
pub mod runner;
pub mod server;
pub mod skiplog;
pub mod split;
pub mod starts;
pub mod state;
pub mod ste;
pub mod supply;
pub mod testdiff;
pub mod testfile;
pub mod testrun;
pub mod trust;
pub mod wasm;
