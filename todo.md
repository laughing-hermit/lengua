# lengua 3: todo

The design is [Lengua3.md](Lengua3.md). Each milestone is green on all five runners before the
next starts (Lengua3.md, section 8).

## Milestones

| # | Milestone | State |
|---|---|---|
| L0 | Tag `lengua2-final`; empty main; a new Cargo workspace; CI on five runners | Landed on main at bd29290 (run 43371160450, green on five) |
| L1 | The idea file: reader, STE check, test format, `# Structure`, OKF, the wiki | Landed on main at c3d1db1 (run 43376901162, green on five) |
| L2 | Generate `Cargo.toml` and the world; build in the background; `doctor` | Landed on main at a9aad3e (run 43380465318, green on five) |
| L3 | The host on the component model: standard input and output, arguments, files, clock, random | Landed on main at 05b8b14 (run 43388384600, green on five) |
| L4 | The Power of 10 lint and the component checks | Done on `wip/l4`; the five-runner landing run is named in the landing report |
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

- **2026-11-13, L2: `check` generates `Cargo.toml` and `wit/world.wit` and holds them.** Their
  exact shapes, the grant-to-interface table, the build job, its states and `doctor` are in
  Lengua3.md, section 4, "The generated files, the build job and `doctor`".
- **2026-11-13, L2: a hand edit is told from an idea change by a stamp.** The top idea's
  frontmatter gets two lengua keys, `cargo_sha256` and `world_sha256`, the hashes of what the
  last check wrote. A file with that hash, or with the hash of the new text, is lengua's; any
  other text is `generated-edited`. Without the stamp, a change of the ideas or a newer lengua
  would look like a hand edit. At L6 the lock holds the same hashes.
- **2026-11-13, L2: the release profile** is `opt-level = 3`, `panic = "abort"`, `lto = true`,
  `codegen-units = 1`: speed first (the performance vision), whole-program optimization for
  smaller and faster code; the longer build is hidden by the background job. Not `opt-level =
  "z"` or `"s"`: size is second to speed.
- **2026-11-13, L2: `[workspace]` in every generated `Cargo.toml`**, so a program folder
  inside another Cargo workspace (as `examples/hello` is in lengua's) is its own root.
- **2026-11-13, L2: crates are pinned exactly** (`name = "=1.2.3"`), the version the idea
  names; the same crate at two versions in two ideas is the finding `crate-version`.
- **2026-11-13, L2: `include` lists the ideas' Rust files** (the "module files from
  `- Modules:`"). It is the package's list of files; Rust still finds modules by `mod`.
- **2026-11-13, L2: a program with `- Exports:` is a `[lib]`** with `path = "src/main.rs"`;
  libraries come at L7 and may change this.
- **2026-11-13, L2: no `wit-bindgen`.** A `wasm32-wasip2` program on the standard library
  reaches WASI 0.2 through `std`, and the toolchain makes the component; the world is the
  statement the component is held to at L3. `wit-bindgen` or `wasi` comes with the HTTP world
  (L7), declared with `- Uses crate:`.
- **2026-11-13, L2: the world names WASI 0.2.12** (the current 0.2 release, read from
  `WebAssembly/WASI` at the tag `v0.2.12`); L3 compares by interface name with any 0.2
  version. An interface's type dependencies come with it: files bring `wasi:clocks/wall-clock`
  (`datetime`), sockets bring `wasi:clocks/monotonic-clock` (`duration`). UDP is in no grant.
- **2026-11-13, L2: the build runs only when `src/main.rs` is there**; without it the state is
  `no_code`, so an agent can check an idea before it writes the code.
- **2026-11-13, L2: the wiki and the generated files are written before the build**, when the
  ideas pass; a failed build does not take them back (the label stays `unchecked` until L5).
- **2026-11-13, L2: a failed build with the same key is not run again**; the reply repeats the
  failure until a file changes. A built component that was removed is built again; a missing
  toolchain is checked again on every `check`.
- **2026-11-13, L2: the compiler's lines are read in `--message-format=short`**, one line per
  error with `file:line:column:`, so no JSON reader is needed; the column goes in the message.
- **2026-11-13, L2: `doctor` needs `rustc` 1.85 or later** (edition 2024) and is run in the
  current folder, so a `rust-toolchain.toml` there counts. The commands it gives: the rustup
  installer (`winget install --id Rustlang.Rustup` on Windows), `rustup update stable`,
  `rustup target add <target>`.
- **2026-11-13, L2: no new crates.** The build job is a standard-library thread and child
  process; the generators are on the standard library.
- **2026-11-13, L2: rule 12 waits for L4.** Lengua3.md said "at L2 and L4"; holding the
  structure to the Rust functions needs a reader of Rust source, which is lengua's lint (L4).
- **2026-11-13, L2: CI lists the imports of the built `hello.wasm`** (the `wasi:` names in the
  binary) after the tests, as evidence for L3's import check.

- **2026-11-13, L3: a grant controls what a program can reach, not what `std` imports.** The
  world's always set holds what Rust 1.100's `std` imports with no grant (`wasi:cli/environment`,
  `stdin`, `terminal-input`, `terminal-stdin`, `wasi:clocks/monotonic-clock`), and the host
  links them with nothing behind them unless granted: no arguments, no environment variables,
  an empty standard input, no terminal, a monotonic clock that moves 1 ms at each read.
  `- Reads: standard input` and `- Reads: arguments` now add no interface; `- Reads: the
  clock` adds `wasi:clocks/wall-clock`. Every other interface is refused at check
  (`import-ungranted`) and not linked at run. Details: Lengua3.md, section 4.
- **2026-11-13, L3: Wasmtime 50.0.1, pinned `=50.0.1`** (still the latest 50.x; 51.0.0 is at
  rc.1). Lengua's dependencies grow by three, each with its reason:
  - `wasmtime` 50.0.1, default features off, `cranelift`, `runtime`, `std`,
    `component-model`: the host, the component model and the compiler. No cache, Winch,
    parallel compilation (`rayon`), GC, `wat` or profiling.
  - `wasmtime-wasi` 50.0.1, default features off, `p2`: WASI 0.2 (no `p1`, no `p3`); it brings
    `tokio` (with `rt-multi-thread`, `time`, `net`, `io-util`, `io-std`, `sync`), `bytes`,
    `futures`, `async-trait`, `tracing`, `bitflags`, `url`, `rand`, `thiserror`, `rustix` and,
    on Windows, `windows-sys`.
  - `wasmtime-wasi-io` 50.0.1, `std`: already brought by `wasmtime-wasi`; named because
    `wasi:io/error`'s `add_to_linker` is only there.
  - What they bring, counted in the `Cargo.lock` that the lockfile job resolved (run
    43383628628): the lock grows from 93 packages to 208, 115 new, for every target together.
    Wasmtime's own (`wasmtime-environ`, `wasmtime-internal-*` for the core, Cranelift, fibers,
    the unwinder, the component macros and `wit-bindgen`), Cranelift's 12 crates and
    `regalloc2`, `pulley-interpreter`, `wasmparser`, `wasm-encoder`, `wit-parser`,
    `wit-component`, `object`, `gimli`, `addr2line`, `postcard`, `semver`, `encoding_rs`,
    `rustix`, `memfd`, `mach2`, `rand` with `chacha20`, `url` with the `icu_*` and `zero*`
    crates of `idna`, `sha2`, `anyhow`, and smaller ones.
- **2026-11-13, L3: the host is a module of `lengua-mcp`** (`host.rs`, `runs.rs`): one engine
  with epoch interruption and a 10 ms ticker thread; a linker per run with only the world;
  a `ResourceLimiter` of lengua's own for `- Memory:`; the epoch deadline for `- Time:`.
  No `unsafe`.
- **2026-11-13, L3: `run {folder, arguments?, stdin?, fixed?}`.** `fixed` is a fourth input,
  not in the plan, so the determinism the tests need is there and tested now. Input that the
  idea does not grant is refused, not dropped. A run waits up to 30 s, then replies `running`;
  the same input waits for the same run.
- **2026-11-13, L3: files are preopened under the folder's own name**, read-only for
  `- Reads:`, read-write for `- Writes:` (made when missing); a read folder that is not there
  is a refusal.
- **2026-11-13, L3: the component check is in the build job** (Wasmtime compiles the
  component once, by SHA-256, and `run` reuses it); a library (`.rlib`, L7) is not checked.
- **2026-11-13, L3: seven examples**, each with its own idea: `echo` (arguments), `count`
  (standard input), `files` (a read and a write folder), `clock` (clock and random numbers),
  `ungranted` (a file and random numbers with no grant), `forever` (the time limit), `greedy`
  (the memory limit).

- **2026-11-13, L4: lengua reads Rust itself, on the standard library; no `syn`.** A new crate,
  `crates/lengua-lint` (a tokenizer of Rust's lexical grammar, a reader of items and bodies,
  the call graph, the rules), with no dependencies; `lengua-idea` uses it. `syn` does not merit
  it (Lengua3.md, section 4: convenience never does): the rules need items, bodies and a few
  shapes, not all of Rust, and what the reader can't read is the finding `rust-read`. No new
  external crate; `Cargo.lock` gains only the path crate.
- **2026-11-13, L4: the lint runs with the idea checks**, when `src/main.rs` is there, so its
  findings come in the same reply as the ideas' and stop the writing and the build; `run`
  refuses a program the lint refuses. With no `src/main.rs`, nothing is linted (an idea can be
  checked before its code).
- **2026-11-13, L4: what each rule accepts and refuses** is in Lengua3.md, section 5, "What
  each rule accepts and refuses", one code a rule part (`rule-1-recursion`, `rule-2-loop`, ...).
- **2026-11-13, L4: rule 5 does not count `main`**, neither as a function nor its assertions:
  `main` is where a program reads its input and writes its output, and a program that is only
  `main` (hello world) has no function to hold to an average. Rule 4 still holds `main` to 60
  lines. To reverse: count it, and every example then needs two assertions in `main`.
- **2026-11-13, L4: rule 2's accepted shapes** are a counter that steps by exactly 1 at the
  loop's top level, against an integer literal or an `UPPER_CASE` constant, with no other
  change and no `continue` before the step; a `for` is bounded unless it goes over an open
  range or one of std's endless iterators with no `.take(`/`.zip(`. A step of 1 is what keeps
  a wrapping counter from jumping its constant.
- **2026-11-13, L4: the call graph does not resolve calls Rust makes on its own** (operators,
  format macros, `Drop`, `Deref`, `for`, `?`, std's generic code), nor a declared crate's
  macros and callbacks; recursion through them traps at run (a stack overflow) or meets the
  time limit. Stricter choice not taken: refuse the program's own impls of std's traits.
- **2026-11-13, L4: a method call on an unknown receiver is a call to every method of that
  name**, so the graph errs towards a false recursion, never a missed one; the message says how
  to name the receiver's type.
- **2026-11-13, L4: rule 10 is held by the lint too**: `#[allow]`, `#[warn]` and `#[expect]`
  would turn the generated `[lints]` off, so they are refused (`rule-10-allow`).
- **2026-11-13, L4: rule 12 refuses what it can't name**: functions, impls, traits, modules and
  macros inside a function (`rule-12-nested`), inline modules (`rule-12-module`), impls for a
  primitive, a tuple, a slice or a type parameter (`rule-12-impl`), and Rust files of no idea
  (`rule-12-source-unlisted`).
- **2026-11-13, L4: rule 6 holds a `static` to a list of types** (numbers, `bool`, `char`,
  `str`, `Option`, references, arrays, tuples, the program's own types of them), not to a list
  of interior-mutability types, so a type lengua can't see into is refused.
- **2026-11-13, L4: the build runs `cargo fmt --check`, then clippy, then the build**; a format
  difference is `rust-format`; clippy's errors are `rust-error` like the compiler's.
  `doctor` checks `rustfmt` and `clippy` too.
- **2026-11-13, L4: the component checks**: at most 64 MiB (`component-size`); lengua's engine
  refuses relaxed SIMD and shared-everything threads, has no threads at all (Wasmtime's
  `threads` feature is off), and canonicalizes NaNs (`component-refused` when Wasmtime does not
  take a component). Why 64 MiB and the `Config` lines: Lengua3.md, section 5.
- **2026-11-13, L4: `lengua-mcp imports <file>`**, a command not in the usage line, prints a
  component's imports as Wasmtime reads them; the CI step that grepped `wasi:` names now runs
  it (L3 found the names in the bytes misleading).
- **2026-11-13, L4: `forever` and `greedy` loop with `for` over `0..u64::MAX` and
  `0..u32::MAX`**, bounded as rule 2 asks, and still past their limits; their ideas say so.

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
| 2026-11-13 | [43376901162](https://github.com/laughing-hermit/lengua/actions/runs/43376901162) | wip/l1 c3d1db1 | test, all five | Green on five: the L1 landing run |
| 2026-11-13 | [43379311761](https://github.com/laughing-hermit/lengua/actions/runs/43379311761) | wip/l2 371f8dc | test, linux-x86_64 | Failed only on format (a patch, applied); tests green: 58 + 8 + 4 + 9 + 2 + 2 passed, hello world built in about 2 s; `hello.wasm` imports listed (Lengua3.md, section 10) |
| 2026-11-13 | [43379930462](https://github.com/laughing-hermit/lengua/actions/runs/43379930462) | wip/l2 e340eb0 | test, linux-x86_64 | Green: format, lockfile, clippy, the tests (hello world built, a compile error mapped to `src/main.rs` line 2, a rebuild after a change, a hand-edited `Cargo.toml`, no toolchain, `doctor`), binary built |
| 2026-11-13 | [43380465318](https://github.com/laughing-hermit/lengua/actions/runs/43380465318) | wip/l2 a9aad3e | test, all five | Green on five: the L2 landing run |
| 2026-11-13 | [43383628628](https://github.com/laughing-hermit/lengua/actions/runs/43383628628) | wip/l3 5216b96 | test, linux-x86_64 | Failed as planned: `Cargo.lock` out of date (the lockfile job resolved Wasmtime 50.0.1 and uploaded it) and a format patch |
| 2026-11-13 | [43384106064](https://github.com/laughing-hermit/lengua/actions/runs/43384106064) | wip/l3 b74cda6 | test, linux-x86_64 | Failed: clippy `struct_excessive_bools` on `Grants` (now a list of `Reach`) |
| 2026-11-13 | [43384781543](https://github.com/laughing-hermit/lengua/actions/runs/43384781543) | wip/l3 a26b713 | test, linux-x86_64 | Failed: clippy `case_sensitive_file_extension_comparisons`; the Wasmtime code compiled with no other finding |
| 2026-11-13 | [43385384753](https://github.com/laughing-hermit/lengua/actions/runs/43385384753) | wip/l3 7f77f68 | test, linux-x86_64 | Failed: clippy `nonminimal_bool`, and a format patch |
| 2026-11-13 | [43385974092](https://github.com/laughing-hermit/lengua/actions/runs/43385974092) | wip/l3 198cc9b | test, linux-x86_64 | Cancelled: a stray brace from a hand merge of the format patch |
| 2026-11-13 | [43386023245](https://github.com/laughing-hermit/lengua/actions/runs/43386023245) | wip/l3 710a7f4 | test, linux-x86_64 | Failed: the same merge had removed a function's first line |
| 2026-11-13 | [43386598827](https://github.com/laughing-hermit/lengua/actions/runs/43386598827) | wip/l3 67e8070 | test, linux-x86_64 | Failed: the `lengua-idea` test that said hello world's world has no `stdin` (the always set has it now) |
| 2026-11-13 | [43387181531](https://github.com/laughing-hermit/lengua/actions/runs/43387181531) | wip/l3 cc502b1 | test, linux-x86_64 | Tests green (59 + 8 + 6 + 17 + 2 + 2: hello runs, arguments, standard input, a read and a write folder with `..`, an absolute path and two symbolic links refused, an ungranted import refused at check and at run, the time limit, the memory limit, fixed and real clock and random numbers); failed only on a format patch, applied |
| 2026-11-13 | [43388384600](https://github.com/laughing-hermit/lengua/actions/runs/43388384600) | wip/l3 05b8b14 | test, all five | Green on five: the L3 landing run |

## What the permission checks refused

- **2026-11-13, L2:** a shell command of the builder that ended in `python3 --version` was
  refused. It was a mistake: this computer runs no interpreter, not even for a version. Nothing
  needed it; the work went on with the editor's tools and `sed`.
- **2026-11-13, L3:** a shell command of the builder that began with `python3 --help` (a slip,
  before a `grep`) was refused. Again a mistake, not a need: the edit went on with the editor's
  tools.
- **2026-11-13, L4:** a shell command of the builder that began with `perl -v` (a slip, before a
  `sed` edit) was refused. Again a mistake, not a need: the edit went on with the editor's
  tools.
