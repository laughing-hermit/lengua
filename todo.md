# lengua 3: todo

The design is [Lengua3.md](Lengua3.md). Each milestone is green on all five runners before the
next starts (Lengua3.md, section 8).

## Milestones

| # | Milestone | State |
|---|---|---|
| L0 | Tag `lengua2-final`; empty main; a new Cargo workspace; CI on five runners | In progress on `wip/l0` |
| L1 | The idea file: reader, STE check, test format, `# Structure`, OKF, the wiki | Not started |
| L2 | Generate `Cargo.toml` and the world; build in the background; `doctor` | Not started |
| L3 | The host on the component model: standard input and output, arguments, files, clock, random | Not started |
| L4 | The Power of 10 lint and the component checks | Not started |
| L5 | Tests from the idea, generated cases, mutation score, trust labels | Not started |
| L6 | The lock, sealed precompiled code, `lengua-run`, the `run` tool | Not started |
| L7 | Applications of several modules; network and HTTP grants; lengua libraries and the shared libraries; a component from another language | Not started |
| L8 | The GPUI front end, embedded in the one file, its behaviours tested | Not started |
| L9 | The skills rewritten for Rust and GPUI; a product test in an interactive session | Not started |
| L10 | The scale test | Not started |
| L11 | Trace and profile; a bench of lengua's overhead; documents; the finish line | Not started |

## Decisions

- **2026-11-13, WASI 0.2 (`wasm32-wasip2`), not 0.3.** WASI 0.3.0 shipped on 2026-06-11, but
  `wasmtime-wasi` still calls its 0.3 module experimental, and Rust's `wasm32-wasip3` reached
  stable only in 1.100.0, with threads not working and LLVM 23 and wasi-sdk 34 named as build
  requirements. 0.2 is stable on both sides and needs only `rustup target add`. Look again at
  L3 and L7. Sources: Lengua3.md, section 10.
- **2026-11-13, Rust 1.100.0 pinned** in `rust-toolchain.toml`, with `rustfmt`, `clippy`,
  `wasm32-wasip2` and `wasm32-unknown-unknown`. It is the current stable (released
  2026-11-12) and the first with `wasm32-wasip3` on stable, so a later move to 0.3 needs no
  toolchain jump.
- **2026-11-13, Wasmtime 50.0.1 is the version to pin at L3** (the current release,
  2026-10-29). Not a dependency yet.
- **2026-11-13, Tokio stays out.** It builds for `wasm32-wasip2` since 1.51.0, but only with
  `--cfg tokio_unstable`; Lengua3.md already says it is not an exception.
- **2026-11-13, lengua's own code follows rule 11: no comments, no doc comments.** The
  documentation is Lengua3.md and each crate's idea-like `README.md`. Reason: a language that
  says comments drift should not ship its own.
- **2026-11-13, workspace lints:** `warnings` denied, clippy `pedantic` denied, `unsafe_code`
  forbidden; CI also sets `RUSTFLAGS=-D warnings`. The rest of the Power of 10 comes at L4.
- **2026-11-13, edition 2024, resolver 3, `Cargo.lock` committed, every cargo step `--locked`.**
- **2026-11-13, Agent Plugins 1.0.0.** agent-plugins.org publishes 1.0.0 as current (no 1.1.0
  schema exists). `plugin/plugin.json` and `plugin/mcp.json` use the 1.0.0 schemas; `mcp.json`
  has no servers until `lengua-mcp` is an MCP server.
- **2026-11-13, CI:** dispatch (`mode` compile or test; `targets` all or a comma list), a
  weekly run on Monday, and a push to main runs `test` on all five; a push to `wip/*` runs
  nothing. On `windows-11-arm`, rustup is installed by the workflow if the image lacks it.

## Runs

The five-runner run a landing rests on is at the head it tests, so it is named in the landing report and added here at the next milestone.

| Date | Run | Ref and sha | Mode, targets | Result |
|---|---|---|---|---|
| 2026-11-13 | [43370086686](https://github.com/laughing-hermit/lengua/actions/runs/43370086686) | wip/l0 8b596f9 | test, linux-x86_64 | Failed: clippy `single_match_else` (pedantic) in `main` |
| 2026-11-13 | [43370623006](https://github.com/laughing-hermit/lengua/actions/runs/43370623006) | wip/l0 3db4778 | test, linux-x86_64 | Green: format, clippy, 1 test passed, binary built and printed `lengua-mcp 0.1.0` |

## What the permission checks refused

Nothing yet.
