# lengua 3: todo

The design is [Lengua3.md](Lengua3.md). Each milestone is green on all five runners before the
next starts (Lengua3.md, section 8).

## Milestones

| # | Milestone | State |
|---|---|---|
| L0 | Tag `lengua2-final`; empty main; a new Cargo workspace; CI on five runners | Landed on main at bd29290 (run 43371160450, green on five) |
| L1 | The idea file: reader, STE check, test format, `# Structure`, OKF, the wiki | Done on `wip/l1`, green on linux-x86_64; the five-runner landing run is named in the landing report |
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

- **2026-11-13, L1: two crates.** `crates/lengua-idea`, a library on the standard library
  alone (the reader, the STE check, the wiki writer, SHA-256), and `crates/lengua-mcp`, the
  server, which uses it. The grammar, the STE rules held and the wiki's formats are in
  Lengua3.md, section 4.
- **2026-11-13, L1: lengua's own dependencies**, each with its reason:
  - `rmcp` 3.10.0 (the current release, 2026-11-12), features `server`, `macros`,
    `transport-io`, default features off: the official MCP Rust SDK, the owner's chosen
    standard; it is the server's interface to its client.
  - `tokio` 1, features `macros` and `rt` only: `rmcp` runs on it and already brings it; the
    server names it only for `#[tokio::main(flavor = "current_thread")]`.
  - `serde`, `serde_json` and `schemars` come with `rmcp`, which re-exports them; `lengua-mcp`
    uses them through `rmcp::serde` and `rmcp::schemars`, so they are not its own
    dependencies. `serde_json` is a dev-dependency of the server's integration test (already
    in the graph through `rmcp`), to read the server's JSON replies.
  - Nothing else. `rmcp` brings `sha2`, but the idea hashes use lengua's own SHA-256, so the
    library stays on the standard library.
- **2026-11-13, L1: `check` writes the wiki only when it finds no problems**, the frontmatter,
  `index.md` and `log.md` in one step, as Lengua3.md says of the lock. A refused check adds no
  log entry.
- **2026-11-13, L1: the log is newest first**, as OKF section 9 orders it. "Append-only" is
  kept as: no entry already there is ever changed or removed.
- **2026-11-13, L1: lengua owns the frontmatter keys `type`, `description`, `module`,
  `source`, `uses`, `used_by`, `label`, `idea_sha256`, `source_sha256`**; every other key
  (OKF's `title`, `tags`, `sources`, `generated`, ...) is the agent's and kept byte for byte.
- **2026-11-13, L1: the passive voice is a finding in a description only when `by` follows
  the participle**; in an instruction, any passive is. STE asks for the active voice "as much
  as possible" in descriptions, and an adjective such as "closed" can't be told from a
  participle without a parser.
- **2026-11-13, L1: lengua's STE dictionary is its own** (`ste-words.txt`, about 770 words
  with parts of speech, and `ste-replace.txt`): the official ASD-STE100 dictionary is the
  ASD's to give, so lengua does not copy it. Technical names go in backticks.
- **2026-11-13, L1: `.gitattributes` sets `eol=lf`**, so the fixtures and their hashes are the
  same bytes on the Windows runners.
- **2026-11-13, L1: CI has a `lockfile` job.** When `cargo metadata --locked` fails, it
  resolves the lock on the runner and uploads `Cargo.lock` as an artifact, which is then
  committed; the lock is never written by hand.
- **2026-11-13, L1: `lengua-mcp` with no arguments serves MCP** (L0 printed the version);
  `--version` still prints it. `plugin/mcp.json` starts `./bin/lengua-mcp` (Agent Plugins
  1.0.0: a `./` command is resolved against the plugin root). Putting the binary in
  `plugin/bin/` is for a later milestone (the release and `doctor`).
- **2026-11-13, L1: one lint allowed, `clippy::unused_async_trait_impl`, on `impl ServerHandler
  for Lengua`** only: `rmcp`'s `#[tool_handler]` macro generates async trait methods with no
  `.await`, which lengua can't change; writing the handler by hand would copy the macro.

## Runs

The five-runner run a landing rests on is at the head it tests, so it is named in the landing report and added here at the next milestone.

| Date | Run | Ref and sha | Mode, targets | Result |
|---|---|---|---|---|
| 2026-11-13 | [43370086686](https://github.com/laughing-hermit/lengua/actions/runs/43370086686) | wip/l0 8b596f9 | test, linux-x86_64 | Failed: clippy `single_match_else` (pedantic) in `main` |
| 2026-11-13 | [43370623006](https://github.com/laughing-hermit/lengua/actions/runs/43370623006) | wip/l0 3db4778 | test, linux-x86_64 | Green: format, clippy, 1 test passed, binary built and printed `lengua-mcp 0.1.0` |
| 2026-11-13 | [43371160450](https://github.com/laughing-hermit/lengua/actions/runs/43371160450) | wip/l0 bd29290 | test, all five | Green on five: the L0 landing run |
| 2026-11-13 | [43374442504](https://github.com/laughing-hermit/lengua/actions/runs/43374442504) | wip/l1 b26624a | test, linux-x86_64 | Failed as planned: `Cargo.lock` out of date (the lockfile job uploaded the resolved lock) and a format patch |
| 2026-11-13 | [43374878743](https://github.com/laughing-hermit/lengua/actions/runs/43374878743) | wip/l1 9293f97 | test, linux-x86_64 | Failed: 12 clippy findings in `lengua-idea` (`chunks_exact`, `format_collect`, `option_option`, `format_push_string`, ...) |
| 2026-11-13 | [43375624971](https://github.com/laughing-hermit/lengua/actions/runs/43375624971) | wip/l1 c62a402 | test, linux-x86_64 | Failed: 2 clippy findings in `lengua-mcp` (`needless_pass_by_value`, `unused_async_trait_impl` from `rmcp`'s macro) and a format patch |
| 2026-11-13 | [43376326657](https://github.com/laughing-hermit/lengua/actions/runs/43376326657) | wip/l1 79fbe14 | test, linux-x86_64 | Green: format, lockfile, clippy, 63 tests passed (51 unit, 6 + 4 integration, 2 version), binary built |

## What the permission checks refused

Nothing yet.
