# lengua 3

The design of lengua 3: a fresh start, in which the agent writes Rust, Rust is compiled to
WebAssembly, and the idea file is the program's only documentation.

Decided by the owner on 2026-11-13. lengua 2 (hand-written WebAssembly text, `main.wat`) is
finished and kept at the tag `lengua2-final`; main starts over from an empty tree.

## 1. The vision

In the owner's words:

- "The purpose of this is basically using the LLM as a compiler. Modernizing the programming
  language. into a AI language."
- "i want to evolve programming languages into ai languages"
- "EVERYTHING GETS COMPILED TO WEB ASSEMBLY FROM RUST"
- "this need to be able to run anywhere regardless of cpu or os"
- "code file will not have comments, the idea file thats in sync with code will be the
  documentation/code"
- "i want a fresh start dont be biased with past implementation"
- "entire software applications should be compiled to one WebAssembly file from Rust... And
  idea.md is like the documentation of that file"
- "for the frontend we can use gpui/gpui-kit … but remember all has to be in 1 wasm file"
- "ensure documentation doesnt drift, all documentation must be current and in sync thats why
  we have a lock file"

What that means for lengua 3:

- **A person writes what they want; the agent is the compiler.** The person states a request.
  The agent writes an idea file: what the program does, its behaviours as tests, and what it may
  touch. Then the agent writes the Rust that does it. lengua checks the Rust against the idea
  and runs it.
- **The idea file is the source of truth and the only documentation.** Code files have no
  comments. Every function the code has is named and described in the idea, and nothing else
  is. `check` holds the two to each other, and the lock refuses one changed without the other.
- **Runs anywhere.** What lengua builds is one WebAssembly component: the same bytes on every
  CPU and every operating system.
- **One application, one file.** A whole application, however many modules it has, server and
  front end, compiles from Rust into one `.wasm` file, signed by one lock. Its idea files are
  that file's documentation, and the lock keeps them current with it.
- **The machine checks the AI's work.** The AI is fast and tireless but makes mistakes, so
  nothing it writes is trusted because it wrote it. Trust comes from the compiler, the rules,
  the tests and the host, and lengua says plainly how far each check went.

### Why Rust compiled to WebAssembly

- **The AI knows Rust well**, far better than WebAssembly text. lengua 2's scale test (a web
  server of 3,638 lines of WebAssembly text) showed the AI could write it, but the bookkeeping
  of offsets and layout cost cycles that a compiler does for free.
- **The Rust compiler catches a class of mistakes** before lengua sees the code: types, memory,
  lifetimes, unhandled results.
- **WebAssembly keeps the sandbox.** A WebAssembly component can reach nothing the host does not
  hand it, so the idea's grants are enforced exactly, on every system. Native code would need
  each operating system's own sandbox, which differ and leak.
- **One component everywhere.** The same `.wasm` file runs on Linux, macOS and Windows, on x86-64
  and ARM64, and in any runtime that speaks WASI 0.2.
- **Interoperability.** WASI is a standard interface and the component model is a standard way
  to link components. A component built from another language can be linked with a lengua
  program, and a lengua program can be used by other hosts.

## 2. What lengua 3 keeps from lengua 2, as ideas

Kept as ideas, not as code. Old code is read at the tag `lengua2-final` when it helps, and a
piece is ported only when lengua 3 needs it and it passes review on its own.

- **The idea file:** a purpose, behaviours, and grants. Behaviours are written the
  Behaviour-Driven way, as Given/When/Then with their examples, and each one is a test. The
  whole file is written in ASD-STE100 Simplified Technical English. `check` lints both and
  refuses an idea that breaks either.
- **Grants enforced by the host:** a program reaches only what its idea declares.
- **Trust labels:** every result says how far the checks went.
- **The lock:** one file ties the idea, the code and the build together, signed with the user's
  key.
- **The wiki and the log** (Karpathy's LLM wiki): every idea, the top one and each module's, is a
  page; `check` keeps an `index.md` and an append-only `log.md`, so the documentation can't
  drift from the code.
- **Open Knowledge Format** frontmatter on every idea, module ideas included, so programs and
  their parts can be indexed and found.
- **A lean runner** that loads sealed, precompiled code with no compiler in it.
- **All of an idea's problems in one reply**, not one kind at a time.
- **Run tools:** a trace of host calls and a profile.
- **Five runners, same bits:** built and tested on GitHub's free runners for Linux, macOS and
  Windows, x86-64 and ARM64.
- **No tool call past about 40 seconds:** long work runs as a background job and `check`
  replies with its state.

## 3. What lengua 3 drops

- Hand-written WebAssembly text (`main.wat`) and its checker.
- The SMT prover (z3 and cvc5), with no replacement planned. It was lengua 2's most expensive
  part and reached little: of the web server's 25 tests, 8 were proven for every input. Heavy
  verification tools are not part of the vision; trust comes from the compiler, the rules, the
  tests and the mutation score.
- Parts by verbatim copy, and named-data offsets.
- Work split into pieces (it did not pay at common sizes).
- The "outside module" side door: every program is a component now, and a component built by
  something else is simply labelled trusted.
- The gate's list of rows behind C, C++ and Rust. lengua 3 measures its own overhead instead:
  a component under lengua's host against the same component under plain Wasmtime.

## 4. Architecture

### The plugin

lengua is an Agent Plugin: skills and one Rust MCP server, nothing else.

| Part | What it does |
|---|---|
| Skill `write-idea` | Turns a request into `idea.md` |
| Skill `build` | Turns `idea.md` into Rust in `src/` |
| MCP tool `check` | Checks the idea, generates the build files, builds, checks the rules, runs the tests, writes the lock and the wiki |
| MCP tool `run` | Runs a checked program under its grants |
| `lengua-run` | The lean runner, for running a checked program from a shell |

Standards followed:
[Agent Plugins](https://agent-plugins.org/),
[Agent Skills](https://agentskills.io/home),
[MCP Rust SDK](https://github.com/modelcontextprotocol/rust-sdk),
[Open Knowledge Format](https://github.com/GoogleCloudPlatform/open-knowledge-format),
[Karpathy's LLM wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f).

### An application folder

```
shop/
  idea.md            written by the agent: the application, its behaviours and grants
  ideas/cart.md      written by the agent: one idea per module (small programs need none)
  ideas/pages.md
  ideas/ui/basket.md written by the agent: one idea per screen of the front end
  src/main.rs        written by the agent: Rust, no comments
  src/cart.rs        one Rust file per module idea
  src/pages.rs
  src/ui/basket.rs   one Rust file per screen idea, written with GPUI
  Cargo.toml         generated by check from the ideas; not edited by hand
  wit/world.wit      generated by check from the grants
  lengua.lock        written by check
```

All of it compiles into one file, `shop.wasm`. One idea file can't hold a large application
(lengua 2's 3,638-line server needed an idea of 594 lines), so each module has its own idea,
with its own behaviours and its own `# Structure`, and the top `idea.md` names the modules and
holds the grants.

The agent writes only the ideas and `src/`. `check` generates `Cargo.toml` (name, edition, the
lints of section 5, a release profile with `panic = "abort"`, only the crates the idea declares, as rule 13
allows) and `wit/world.wit`, and holds them unchanged: a hand edit is a finding.

### The idea file

```
---
okf: ...                      Open Knowledge Format frontmatter
---
# Purpose                     what the program is for, in STE
# Behaviours                  Given / When / Then, each with its examples; each is a test
# Constraints                 grants (- Reads: files in ..., - Connects to: ...),
                              memory, time, exports, modules
# Structure                   every function of the module, one sentence each
```

`# Structure` is new. It is how the ideas become the documentation: `check` reads every
function in a module's Rust file and every name in that module's `# Structure`, and each must
have the other.

### Web applications: the server and the GPUI front end

A web application has two halves, and both are in the one file:

- **The server side** receives requests and sends back pages, data and images. It is a WASI
  component that implements WASI's HTTP world, built for `wasm32-wasip2`. lengua's host runs it
  under the idea's grants.
- **The front end** runs inside the visitor's browser and reacts at once to clicks and typing.
  It is written with [GPUI](https://gpui.rs/) and [GPUI Kit](https://gpui-kit.com/), Zed's
  GPU-drawn Rust UI framework and a library of components on it. GPUI runs in the browser
  through Zed's `gpui_web` crate (merged upstream in February 2026), built for
  `wasm32-unknown-unknown`, drawing on one canvas with WebGPU and WebGL2 as the fallback.

A browser can't run a WASI component, and a WASI host can't run a browser module, so no single
WebAssembly binary can be both. The one file holds both: the server component carries the
front end's compiled `.wasm`, the small loader a browser needs to start it, its fonts and its
images, and serves them itself. One file to ship, one lock, two compiled parts inside.

- Each screen of the front end has its own idea in `ideas/ui/`, in Given/When/Then and STE like
  every other idea, and its own Rust file in `src/ui/`. Rules 11 and 12 hold for them too.
- The front end reaches only its own server. lengua's grants hold the server; the browser's
  sandbox holds the front end.
- Limits, from GPUI's web target: one window per page; WebGPU preferred, WebGL2 when not; the
  fonts are shipped in the file, since a browser gives a canvas no fonts. To verify at the
  milestone.

### The build

`check` runs `cargo build --offline --release` on the user's machine as a background job: for
`wasm32-unknown-unknown` first when the application has a front end, then for `wasm32-wasip2`
with the front end embedded. It replies `building` until the build ends, then goes on. The
result is one WebAssembly component, the one file.

The user needs Rust and its `wasm32-wasip2` and `wasm32-unknown-unknown` targets installed. `lengua-mcp doctor` checks both
and says the one command to run if either is missing.

### Grants become the component's world

The idea's grants generate the WIT world in `wit/world.wit`. The world imports only the WASI
interfaces the grants allow: standard output always; standard input, arguments, the clock,
random numbers, files and sockets only when granted.

Two checks hold it:
- **At check:** the built component's imports must all be in the world. Anything else is
  refused.
- **At run:** the host links only the world's interfaces, with lengua's policy on top: files only
  in the folders the idea names, connections only to the hosts and ports it names, memory and
  time limits from the idea.

### The host

Wasmtime with the component model and its WASI 0.2 implementation. In tests the clock is fixed
and random numbers come from a seed, so every run gives the same output on every system.

### Libraries and other languages

- **A lengua library** is another program of the bundle that declares `- Exports:`. A program
  that uses it names it in `- Uses:`, and `check` links it as a Rust path crate. Its own checks
  and tests stand.
- **A component built from another language** (C, Go, anything with a WASI 0.2 target) is linked
  through the component model. lengua did not build or check it, so it is labelled `trusted`.

### Crates: few, and only ones that build for WebAssembly

The whole Rust ecosystem is open, but every external crate is code lengua didn't write and
can't hold to its rules, so lengua keeps them few. The `build` skill chooses in this order:

1. **A lengua library** already in the bundle, or in lengua's own shared libraries (section
   below). Written to the same rules, checked and tested like the application.
2. **A crate from crates.io that builds for both targets**, `wasm32-wasip2` and, for the front
   end, `wasm32-unknown-unknown`. Pure-Rust crates usually do.
3. **If no such crate exists**, or the one that exists pulls in much more than it is worth, the
   agent writes its own as a lengua library: its own idea, its own tests, no comments, the
   Power of 10. It then serves every later application too.

What `check` does with each external crate:
- It must be declared in the idea with `- Uses crate: <name> <version>, for <what>`. The reason
  is part of the documentation.
- It must build for each target the application needs. A crate that needs threads, a C library
  or one operating system's own calls fails, and the reply names the crate and the reason.
- It is pinned by checksum in `Cargo.lock` and signed in the lock.
- The reply counts the external crates, the crates they bring in with them, and which of them
  use `unsafe`. That count is in `index.md`, so it can be watched as the application grows.

### lengua's shared libraries

lengua ships a small set of its own libraries, written as lengua programs, so the common needs
of an application don't send it to crates.io: reading and writing JSON, parsing and building
HTTP messages, decimal numbers, text and lines, sorting, CRC and hashes, dates and times. Each
has its own idea, tests and lock, and builds for both targets. GPUI and GPUI Kit stay external:
writing a UI framework is not lengua's work.

### The lock

The lock is what keeps every document current. `lengua.lock` holds, signed together with an
HMAC under the user's key, the hashes of:
- every idea, the top one and each module's and screen's;
- `index.md` and `log.md`;
- every file in `src/` and the generated files;
- `Cargo.lock`, which pins every dependency by checksum, and the toolchain's version;
- both compiled parts and the one file.

It also names the sealed, precompiled form of the component that `run` and `lengua-run` load.

`check` writes the wiki (the frontmatter, `index.md` and `log.md`) in the same step as the lock,
and only when every check passed. So there is no moment when the lock is current and the
documentation is not. A change to any idea, any code or any document without a new check means
the lock no longer matches, and `run` refuses the program.

### The wiki and the log: documentation that can't drift

An application's ideas form a wiki, as in Karpathy's LLM wiki, and `check` keeps it.

- **Every idea is a wiki page** with Open Knowledge Format frontmatter: the top `idea.md` and
  each `ideas/<module>.md`. The frontmatter gives its kind, its module, links to the ideas it
  uses and the ideas that use it, its trust label, and the hash of its Rust file. `check`
  writes the frontmatter; the agent doesn't.
- **`index.md`** has one line per idea and module, with its purpose and its label. It is the
  map an agent, or a person, reads to find the right page before reading any code.
- **`log.md`** is append-only. Each check adds one entry: the date, which ideas and which Rust
  files changed, their new hashes, the result and the label. When an entry shows a Rust file
  changed with no change to its idea, or an idea changed with no change to its code, `check`'s
  reply says so.

Three things stop the documentation drifting from the code:
1. **Rule 12:** every function is named in its module's `# Structure`, and nothing else is.
2. **The lock:** the ideas', the documents' and the code's hashes are signed together, and
   `run` refuses a program whose files no longer match.
3. **The log:** the history of when each idea and each file changed, so anyone can see they
   moved together.

## 5. The Power of 10, for Rust compiled to WebAssembly

NASA's [Power of 10](https://en.wikipedia.org/wiki/The_Power_of_10:_Rules_for_Developing_Safety-Critical_Code),
adapted for Rust that runs as a WebAssembly component and is written by an AI.

| # | NASA's rule | lengua 3's rule | Held by |
|---|---|---|---|
| 1 | Simple control flow, no recursion | No recursion, direct or through other functions. No `dyn` trait objects and no function pointers in program code. | lengua's lint (the call graph from the source) |
| 2 | Every loop has a fixed upper bound | `for` over a range, a slice or a bounded iterator. `while` and `loop` only with a counter checked against a constant. | lengua's lint; the host's time limit as a backstop |
| 3 | No dynamic memory after start-up | The idea declares a memory maximum. The component's memory can never grow past it. | The component check and the host's limit |
| 4 | A function fits on a page | 60 lines or fewer, after `rustfmt`. | lengua's lint |
| 5 | Two assertions a function, on average | At least two `assert!` or `debug_assert!` per function, on average. | lengua's lint |
| 6 | Data in the smallest scope | No `static mut`, no global with interior mutability. | `[lints]` and lengua's lint |
| 7 | Check every return value and parameter | Every `Result` and `#[must_use]` value is used. No `unwrap`, `expect` or indexing that can panic in program code. | `[lints]`: `unused_must_use`, `clippy::unwrap_used`, `clippy::expect_used`, `clippy::indexing_slicing` |
| 8 | Limit the preprocessor | No `macro_rules!` that define items, no `build.rs`, no `cfg` beyond the target. Macros and derives only from the crates the idea declares (rule 13). | lengua's lint and the generated `Cargo.toml` |
| 9 | Restrict pointers | No `unsafe` at all in the application's own code, so no raw pointers. A declared crate may use it, and the reply says which do. | `#![forbid(unsafe_code)]` through `[lints]` |
| 10 | All warnings on, from day one | Every warning is an error; clippy's `pedantic` group on; from the first build. | `[lints]` |

Added for an AI language:

| # | Rule | Held by |
|---|---|---|
| 11 | No comments and no doc comments in code files. The idea is the documentation. | lengua's lint |
| 12 | Every function is named in its module's `# Structure`, and nothing else is. | `check` |
| 13 | Few external crates: a lengua library first; a crate from the ecosystem only if the idea declares it (`- Uses crate: name version, for what`) and it builds for every target the application needs; otherwise the agent writes its own as a lengua library. Each is pinned by checksum, signed in the lock, and counted in the reply with whether it uses `unsafe`. | The generated `Cargo.toml`, `Cargo.lock`, the lock and the reply |

And on the built component: imports only from the world; no threads, no relaxed SIMD and no other
feature whose result can differ between machines; a size limit.

## 6. Trust labels

| Label | Means |
|---|---|
| `checked` | Built by lengua, every rule holds, every test passes, and the mutation score is above the threshold |
| `tested` | Built by lengua, every rule holds and every test passes, but the tests are weak: the mutation score is under the threshold |
| `trusted` | A component lengua did not build; it runs under the grants, and its tests run, but nothing about its code is checked |

`check`'s reply and the wiki show the label of every program. A program that uses a `trusted`
component says so.

## 7. Tests

- **The idea's behaviours** run against the built component: given its input (standard input,
  arguments, files, a local network fixture), then its output, compared exactly.
- **Generated cases:** a behaviour can give ranges, and lengua draws cases from them, boundary
  values first.
- **Mutation score:** lengua changes the Rust source in small ways (an operator, a constant, a
  comparison), builds each mutant, and runs the tests. The share of mutants the tests catch is
  the score. It says how strong the tests are, and decides between `checked` and `tested`.
- **The front end:** each screen's behaviours run against the screen's state through GPUI's test
  context: given the state, when the user clicks or types, then the state and what is shown.
  The screen is not compared pixel by pixel; the tests compare state and text, not images.
- **Deterministic:** the same input gives the same output on every system, every time.

## 8. Milestones

Each one is green on all five runners before the next starts.

| # | Milestone | Done when |
|---|---|---|
| L0 | Tag `lengua2-final`; empty main; a new Cargo workspace; CI on five runners | An empty `lengua-mcp` builds and its one test passes on five |
| L1 | The idea file: reader, STE check, test format, `# Structure`, OKF, the wiki | `check` reports every problem of an idea at once |
| L2 | Generate `Cargo.toml` and the world; build in the background; `doctor` | A hello-world idea builds to a component on five |
| L3 | The host on the component model: standard input and output, arguments, files, clock, random | A program runs under exactly its grants; an ungranted import is refused at check and at run |
| L4 | The Power of 10 lint and the component checks | Each rule has a test that a program breaking it is refused |
| L5 | Tests from the idea, generated cases, mutation score, trust labels | A program goes from idea to `checked` |
| L6 | The lock, sealed precompiled code, `lengua-run`, the `run` tool | A changed idea or file is refused at run |
| L7 | Applications of several modules; network and HTTP grants; lengua libraries and the shared libraries (JSON, HTTP, decimal, text, sorting, hashes, dates); a component from another language | An application of several module ideas builds to one file; it uses a library and a C component, each labelled |
| L8 | The GPUI front end: screens built for the browser, embedded in the one file, their behaviours tested | A web application with a GPUI screen ships as one file and opens in a browser |
| L9 | The skills rewritten for Rust and GPUI; a product test in an interactive session | An agent goes from a request to a `checked` web application with the plugin alone |
| L10 | The scale test: a web application, server and GPUI front end, in one file, built and then changed three times | A report of cycles and breakages, next to lengua 2's 3,638-line record |
| L11 | Trace and profile; a bench of lengua's overhead; documents; the finish line | The finish-line run green on five; only main and tags remain |

## 9. How the work is done

Unchanged from lengua 2:

- This computer only edits files and runs git and gh. Everything is built, tested and measured
  on GitHub's free runners.
- Lean CI: iterate on Linux on x86-64; all five runners only for the run a landing rests on; a
  compile check for documents-only changes.
- Each milestone on a `wip/` branch, landed on main by fast-forward after a green five-runner
  run, then deleted. At the end only main remains, with tags.
- Nothing may cost money.
- Agent-level product tests only in the live interactive session, never headless.

## 10. Risks and open questions

- **WASI 0.2 or 0.3:** check the state of WASI 0.3 and of the `wasm32-wasip2` target and
  Wasmtime's versions at L0, and pin them.
- **Build time:** a cold `cargo build` can take longer than one tool call; the background job
  and a kept build folder per program must hide it.
- **The user must have Rust installed.** lengua 2 needed nothing but lengua itself.
- **Component size and start-up time** against lengua 2's hand-written modules: measured at L11.
  GPUI makes the file much larger.
- **GPUI's web target is new** and lives in Zed's repository: pin one commit, and expect to
  follow it as it changes.
- **Crates:** the whole Rust ecosystem is open, but not every crate builds for WebAssembly: a
  crate that needs threads, a C library or one operating system's own calls fails the build,
  and `check` says which. Crates bring code lengua did not write, some with `unsafe`; each is
  declared in the idea, pinned by checksum and named in the reply. They are downloaded once,
  at the first build, and kept.
- **Browser support:** WebGPU is in current Chrome, Edge, Firefox and Safari; older browsers get
  WebGL2.
- **GPUI's test context** must run on all five systems for the front end's tests.
- **What other runtimes allow:** a lengua component runs in any WASI 0.2 runtime, but outside
  lengua the grants are that runtime's to enforce, not lengua's.

## Sources

- GPUI: [gpui.rs](https://gpui.rs/), [docs.rs/gpui](https://docs.rs/gpui/latest/gpui/),
  [examples](https://gpui.rs/examples/); its web target: Zed's
  [`crates/gpui_web`](https://github.com/zed-industries/zed/tree/main/crates/gpui_web) and
  pull request [#50228, "GPUI on the web"](https://github.com/zed-industries/zed/pull/50228),
  merged on 2026-02-26.
- GPUI Kit: [gpui-kit.com](https://gpui-kit.com/).
- The standards and the Power of 10: the links in sections 4 and 5.
