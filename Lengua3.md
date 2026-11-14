---
type: lengua Design
title: lengua 3
description: "The design of lengua 3, the source that every milestone is built from."
---

# lengua 3

The design of lengua 3: a fresh start, in which the agent writes Rust, Rust is compiled to
WebAssembly, and the idea file is the program's only documentation.

Decided by the owner on 2026-11-13. lengua 2 (hand-written WebAssembly text, `main.wat`) is
finished and kept at the tag `lengua2-final`; main starts over from an empty tree.

### A fresh start

lengua 3 is designed and written from scratch, so that it is not biased by lengua 2 ("i want a
fresh start dont be biased with past implementation").

- **This document is the source.** lengua 3 is built from what this document says, not from
  what lengua 2 did. Where the two differ, this document wins.
- **No code is carried over.** lengua 2's source is not copied, ported or used as a template.
  Every file of lengua 3 is written new.
- **Don't read lengua 2's code to build lengua 3.** Whoever builds a milestone, a person or an
  agent, works from this document and the standards it links. Lengua 2's reports may be read
  for lessons (for example its scale test), never its code for answers.
- **Main starts empty.** At L0 every file of lengua 2 is removed from main. lengua 2 stays only
  at the tag `lengua2-final`, as history.
- **Questions go back to the design.** When this document doesn't answer something, the answer
  is decided for lengua 3 and written here, not taken from lengua 2.

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
- "lengua must be the evolution of a programming language, it needs to be an artificial
  intelligence/ super intelligence language"
- "the idea is maximum interoperability regardless of os and cpu"
- "lengua should also compile rust to webassembly as much as possible but i know that there
  will be exeptions"
- "it should be able to do a full stack way of doing things"

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
- **Maximum compatibility, few dependencies.** One WebAssembly file runs on any computer,
  whatever its operating system and CPU. Always minimise external dependencies: the standard
  library first, and an external crate only when it merits it (section 4).
- **The machine checks the AI's work.** The AI is fast and tireless but makes mistakes, so
  nothing it writes is trusted because it wrote it. Trust comes from the compiler, the rules,
  the tests and the host, and lengua says plainly how far each check went.
- **Full stack.** One lengua application can hold a GPUI front end, a server, storage,
  logins, calls to other services and to AI models, all in the one file. Full-stack web
  development is what lengua does first and best.

### WebAssembly first; native only when necessary

All Rust that lengua builds compiles to WebAssembly unless WebAssembly truly can't do the job.

| Kind of application or part | Builds to | Why |
|---|---|---|
| Web apps: front end and server | WebAssembly | The browser and WASI servers are where WebAssembly is strongest |
| Command-line tools, servers, background services | WebAssembly | No window is needed, so one file runs on any computer |
| GPUI apps in a browser window | WebAssembly | They look like desktop apps and run in the browser |
| Calling AI models over the network, embeddings, search | WebAssembly | Outgoing HTTP is enough |
| Desktop apps with their own window | Native | A real window and the GPU outside the browser are not in WebAssembly yet |
| Running or training AI models locally | Native | Needs the GPU directly |
| Threads that share memory | Native | Not in WebAssembly yet |
| Hardware devices (serial, USB, Bluetooth) | Native, or a host grant | Not in WebAssembly; a specific device can instead be a grant the host provides |

The rule for a native part:
1. The idea declares it with `- Native: <part>, for <reason>`, and the reason must be one of the
   kinds marked Native above. Any other reason is refused.
2. Only the part that needs it is native; the rest of the application stays WebAssembly.
3. The same checks apply: the Power of 10, the idea's tests, the mutation score, the lock.
4. Its label is `native`, shown in every reply and in the wiki, because it is not sandboxed: it
   can do what the user's account can.
5. When a standard lifts the limit (see the watch list in section 10), the part moves back to
   WebAssembly.

The two native programs that lengua itself is made of, `lengua-mcp` and `lengua-run`, are the
standing exception: something native must run the WebAssembly. lengua's own std-only
libraries are also built for WebAssembly, so the checker could one day run as WebAssembly too:
since L8, CI builds `lengua-idea` and `lengua-lint` for `wasm32-wasip2` on every runner, and a
failure fails the run.

**Where it runs, said plainly.** The user is never surprised:
- before any code is written, the `write-idea` skill says whether the request will be one
  WebAssembly file or needs a native part, and why, and offers a WebAssembly alternative when
  one exists;
- the idea file says it (WebAssembly is the default; native parts are declared);
- `check` refuses code that needs native without the idea declaring it, and says what needs it;
- every reply and `index.md` start with a "Where it runs" line, for example "One WebAssembly
  file. Runs on any OS and CPU. Sandboxed: it reaches only what the idea grants."

### The vision: the best lengua solution, before the idea

Before any idea is written, the `vision` skill works out with the person the best way to build
what they want with lengua, and writes it to `vision.md` in the application's folder, in plain
words for the person:

- whether it fits lengua, and where it runs (one WebAssembly file, or which native part and why);
- its shape: modules, screens, what is on the server and what in the GPUI front end, storage,
  logins, calls to other services and AI models;
- what it may reach: files, network, secrets, storage;
- what it is built from: the standard library, lengua's shared libraries, the agent's own code,
  and any crate that merits it, pure Rust first;
- what lengua can't do yet for it, and the alternatives;
- for each principle below, whether the plan meets it easily or pushes against it, and why.

The person reads `vision.md` and approves it in their own words, like the idea. Then
`write-idea` turns it into the ideas. `vision.md` is the top page of the application's wiki,
with OKF frontmatter, and the lock signs it with everything else. If the ideas later grow past
the vision (a new grant, a native part, a new crate), `check` says so, and the vision is
updated and approved again.

The principles and their do's and don'ts live in one page, the skill's
`references/principles.md`, which this document also points to; `check` holds the page and this
section to the same list, so they can't drift apart.

| # | Principle | Do | Don't |
|---|---|---|---|
| 1 | An AI language | Write the idea as the source and the only documentation; let the AI compile it to Rust; let the machine verify it | Trust code because an AI wrote it; document anything outside the idea |
| 2 | Maximum interoperability | Build one file that runs on any OS and CPU | Build per system when WebAssembly can do the job |
| 3 | WebAssembly first | Make every part WebAssembly; declare a native part only for a named limit, with its reason | Go native for speed, convenience, or a crate that only builds natively when a WebAssembly path exists |
| 4 | Nothing unless absolutely necessary | Use the standard library, then lengua's shared libraries, then the agent's own code; take a pure-Rust crate only when it merits it, declared with its reason | Add a crate to save a few hundred lines; take a crate that wraps C when a pure-Rust one exists; grant access the program doesn't need |
| 5 | Full stack, web first | Put the GPUI front end, the server, storage, logins and AI calls in the one file | Split one application into separate deployables |
| 6 | The Power of 10 | Plan small functions, bounded loops, plain data, every error handled | Plan recursion, unbounded loops, global state, `unwrap`, `unsafe` |
| 7 | Trust you can see | Say up front what will be `checked`, `tested`, `trusted` or `native` | Hide a trusted component or a native part |
| 8 | Documentation that can't drift | Keep the vision, the ideas and the code signed together; name every function in its idea | Write comments in code; change code without its idea, or an idea without a check |
| 9 | The person decides | Ask the person to read and approve the vision and the idea in their own words | Write code before approval; decide for the person what only they can |

### The person approves the idea before any code

The idea file is what the person asked for, so the person approves it before the agent writes
code, and again whenever a change alters what the program does or may reach. It is natural:

1. The `write-idea` skill writes the idea and runs `check` until it has no problems.
2. It tells the person the idea is ready and where it is. The person opens the file in
   whatever they use to read files and reads it. The idea file is written to be read.
3. The person says so in their own words, for example "I approve", or says what to change.
   The agent changes the idea and asks again. No code is written before the person approves.
4. When the person approves, the agent tells `check`, and `check` writes the hash of the
   approved ideas into the lock. Nothing else is asked of the person.
5. The `build` skill starts only from an approved idea. If an idea's behaviours, grants, native
   parts or crates change after approval, `check` refuses to build until the person approves
   again; a change to wording or `# Structure` alone does not need it.

What lengua can't do: it can't prove a person, and not the agent, said yes. The skill asks,
and the lock and the log record when and what was approved.

### Programs that ask AI models

An AI language's programs can ask an AI model as part of their own work: classify this,
summarise that, extract these fields, then carry on with the answer, the way LangChain does.
In lengua it is a grant like any other.

- **The idea declares it:** `- Asks: an AI model, for <what>`, with limits:
  `up to N calls, M tokens` per run. Without the grant, the program can't ask.
- **One interface, provided by the host:** send a prompt, get a reply; or ask for a structured
  reply that must match a shape, which the host checks. Streaming replies too. The program
  never holds an API key.
- **The person running it chooses the model**, outside the program:
  - a cloud API (such as Anthropic's), with its key held by the host;
  - a local model (such as Ollama or llama.cpp), so nothing leaves the machine;
  - the agent's own model through MCP sampling, when the program runs from an agent that
    supports it. Claude Code does not support sampling yet
    ([issue #1785](https://github.com/anthropics/claude-code/issues/1785),
    [MCP sampling](https://modelcontextprotocol.io/specification/2025-11-25/client/sampling)),
    so this path waits; cloud and local work now.
- **Tests stay exact and free.** An example gives the model's answer:
  ``Given the model answers `urgent` ``. A separate, optional live run asks the real model and
  is never part of the label.
- **AI output is untrusted input.** The program checks every answer (parses it into a shape,
  refuses anything else); the Power of 10's "every result handled" applies.
- **Still one WebAssembly file.** The interface is a WIT interface the host links only with
  the grant. (`wasi-nn` is for running a model inside the program; it is on the watch list.)
- **A shared library, `ai`,** on the standard library alone: prompt templates, structured
  replies, retries, and steps chained one after another.
- **Where it runs and what it costs are said plainly:** the "Where it runs" line names the
  model's kind (cloud, local), and the reply shows the call and token limits.

### Nothing unless absolutely necessary

One principle runs through lengua:
1. No external dependencies unless absolutely necessary; pure-Rust crates first.
2. No native builds unless absolutely necessary.
3. No access unless the idea grants it.
4. No `unsafe` code: lengua's single `unsafe` call runs only after verification.
5. No randomness or real time unless granted.
6. No memory or time beyond what the idea declares.
7. No unchecked code unless it is labelled `trusted`.
8. No new grammar in the idea file unless something can't be said without it.
9. No human steps unless only a person can take them.

And three with no exceptions: no comments in code; no code without its idea and no idea without
its code; no change without a new check.

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

Kept as ideas only, never as code. lengua 3 is written new, from this document (see "A fresh
start" above).

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
| Skill `vision` | Turns what a person wants to build into `vision.md`: the best lengua solution for it, judged against lengua's principles and their do's and don'ts |
| Skill `write-idea` | Turns an approved `vision.md` (or a small request) into `idea.md` |
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

### The idea file's grammar

Decided at L1 (2026-11-13). `check` reads every idea to this grammar and reports every place
that breaks it, all in one reply (see "What `check` replies" below).

**Files.** An application folder has one top idea, `idea.md` (kind *program*), and may have
module ideas, `ideas/<name>.md` (kind *module*), and screen ideas, `ideas/ui/<name>.md` (kind
*screen*). A module's name is `snake_case` and not `main`, `ui`, `index` or `log`; a screen's
name is `ui/<name>`. Any other folder under `ideas/` is a finding. Every file is UTF-8; a
carriage return before a line end is read as a line end.

**The frontmatter** is optional for the agent: an idea can start at `# Purpose`. When it is
there, it opens with `---` on line 1 and closes with `---`; each line between is `key: value`
(a key of letters, digits, `_` and `-`), a line that continues the key above (it starts with a
space or `-`), a blank line or a `#` comment. A key may be there one time only.

- **lengua writes these keys**, and overwrites any value the agent gave them: `type`
  (`lengua Program`, `lengua Server` or `lengua Library` for the top idea, by its shape, since
  L7; `lengua Module` or `lengua Screen`; a component page, not an idea, is `lengua Component`,
  since L9), `description` (the first sentence of
  `# Purpose`), `module` (`main` for the top idea), `source` (`src/main.rs`, `src/<name>.rs`,
  `src/ui/<name>.rs`), `uses` and `used_by` (OKF concept ids, the file without `.md`: `idea`,
  `ideas/cart`, `ideas/ui/basket`; since L9 a library's id too, its folder and `/idea`:
  `../../libraries/json/idea`), `label` (`unchecked`, `tested` or `checked`, section 6;
  since L5 the program's label, on every idea of it),
  `idea_sha256` (the SHA-256 of the idea's text after the frontmatter, blank lines at its
  start and the `# Links` section left out, the text ending with one line end) and
  `source_sha256` (of the Rust file, or `none`); on the top idea only,
  `cargo_sha256` and `world_sha256`, the SHA-256 of the `Cargo.toml` and `wit/world.wit` that
  check wrote (since L2, see "The generated files" below); since L9, OKF's trust fields
  `generated` (`{ by: lengua-mcp/<version>, at: <time> }`) and `verified` (below).
- **The agent may write any other key**, and lengua keeps it byte for byte: OKF's `title`,
  `tags`, `status`, `sources`, `stale_after`, `resource`, or a key of its own. Since L9 lengua
  writes `title` (the folder's name for the top idea, the module's name for the others) and
  `status: draft` when they are absent, after the agent's keys, and from then on keeps them as
  the agent's. It checks the values OKF defines (OKF sections 5 and 7): `status` is `draft` or
  `deprecated` (`stable` only after the person's approval, L11; `okf-status` otherwise);
  `stale_after`, and each `at` and `last_modified`, an ISO 8601 time with its offset
  (`okf-time`); each `by` and each source's `author` an OKF actor, `human:<id>`,
  `process:<id>` or `<producer>/<version>` (`okf-actor`); each source has a `resource`
  (`okf-sources`); `verified` is a `{ by, at }` mapping or a list of them (`okf-trust`).
- **`verified`** (since L9) keeps the entries of other actors (a person's, at L11) and holds
  one entry of lengua's own, `{ by: process:lengua-check, at: <time> }`, while the label is
  `checked` or `tested`: the time of the first check that confirmed the files as they are.
  When the label goes back to `unchecked`, lengua's entry is removed.
- **The times.** `generated.at` and lengua's `verified` `at` change only when the page's text
  changes; a check that changes nothing keeps them, so a page is not rewritten for nothing.
  Every time is UTC, `YYYY-MM-DDTHH:MM:SSZ`.

**`# Links`** (since L9) is the last section of every idea, and `check` writes it: real
markdown links, relative to the page, to the ideas and libraries the idea uses (`* Uses:`),
the ideas that use it (`* Used by:`), on the top idea the component pages
(`* Components:`), `vision.md` when the folder has one (`* Vision:`), and the folder's
`index.md` (`* Index:`). It is not part of the idea: the grammar below never sees it, and the
idea's hash leaves it out, as it leaves out the frontmatter. A `# Links` that is not the last
section is `section-unknown`.

**The sections** are level-1 headings, in this order, each one time: `# Purpose`,
`# Behaviours`, `# Constraints`, `# Structure`. Every idea has Purpose, Behaviours and
Structure; the top idea has Constraints too, and a module or screen idea may have them. Only
blank lines come before `# Purpose`. A heading inside a fenced block is not a section.

**`# Purpose`**: one paragraph or more of description in STE, with no headings and no fenced
blocks. A list item (`- ` or `* `) counts as its own paragraph. Its first sentence is the
idea's `description` in the frontmatter and in `index.md`. Since L9, a name in backticks that
is a module of the program or a lengua library must be one the idea lists
(`purpose-unlisted`): the top idea lists its modules in `- Modules:`, a module the modules it
uses in `- Uses:`, and a library is listed in the top idea's `- Uses:`. The libraries known are
those the top idea uses and the folders of every `libraries/` folder above the program's folder;
a library's own idea may name other libraries, since it can't use them. The check is
conservative: a name in plain words is not held.

**`# Behaviours`**: one behaviour or more. Each is:

```
## <name>                         a name, different from the other behaviours' names

Given <clause>                    one line, or more lines that continue it
And <clause>                      optional, after any of the three
When <clause>
Then <clause>

### Example: <name>               one block or more: Example or Cases
- <Key>: <value>
```

`Given`, `When` and `Then` are each there one time, in that order; `And` adds to the one above
it. Each clause is checked as an STE description. A behaviour has one `### Example:` or
`### Cases:` block or more. A block has only list items, each `- <Key>: <value>`; a value can
continue on lines indented under its item.

The keys of a program or module example (the program runs once, with nothing else on its
standard input, no arguments and no files unless the example gives them):

| Key | Value | Means |
|---|---|---|
| `Standard input` | a string | the bytes on standard input |
| `Arguments` | strings with spaces between them | the arguments after the program's name |
| ``File `<path>` `` | a string | a file that is there before the run |
| `Expect standard output` | a string | standard output, compared exactly; when absent, the standard output is empty |
| `Expect standard error` | a string | standard error, compared exactly; not compared when absent |
| `Expect end` | `exited`, `out of time`, `out of memory` or `trapped` | how the run ends (since L5); `exited` when absent |
| `Expect exit code` | a number from 0 to 255 | the exit code; every example that ends `exited` has it, and no other example has it |
| ``Expect file `<path>` `` | a string | a file's content after the run, compared exactly |

The keys of a screen example (state and text are compared; pixels are not, section 7):

| Key | Value | Means |
|---|---|---|
| ``State `<field>` `` | a string, an integer, `true` or `false` | the screen's state before |
| `Click` | `` `<element>` `` | the user clicks the element |
| ``Type into `<element>` `` | a string | the user types the text into the element |
| ``Expect state `<field>` `` | a string, an integer, `true` or `false` | the state after |
| `Expect text` | a string | text the screen shows |

The keys of a network fixture, a library call and a server request (since L7):

| Key | Value | Means |
|---|---|---|
| ``Server `<host>:<port>` `` | a string | lengua listens at the address for the run, sends the string to each client and reads what it sends to the end |
| ``Client `<port>` `` | a string | lengua connects to `127.0.0.1:<port>` while the program runs, sends the string and reads the answer |
| `Expect answer` | a string | what the one client got, compared exactly |
| `Call` | a Rust expression in backticks | a library's example: the harness evaluates it with the library's exports in scope |
| `Expect value` | Rust's `{:?}` of the value, in backticks or a string | compared exactly; required with `Call` |
| `Request` | `` `<METHOD> <path>` ``: `GET`, `HEAD`, `POST`, `PUT`, `DELETE`, `PATCH` or `OPTIONS`, and a path from `/` | a server's example: the request |
| ``Request header `<name>` `` | a string | a header of the request |
| `Request body` | a string | the body of the request, with its `content-length` |
| `Expect status` | a number from 100 to 599 | the answer's status; required with `Request` |
| ``Expect header `<name>` `` | a string | a header of the answer, the name in any case |
| `Expect body` | a string | the answer's body, compared exactly; not compared when absent |

A call or a request has no exit code (`example-exit-code-unused`); `Expect value` goes with
`Call` (`example-call-missing`, `example-value-missing`), and the keys of a request with
`Request` (`example-request-missing`, `example-status-missing`). A block is one of the three
kinds (`example-mixed`). A server's address is `host:port`, a client's a port
(`example-address`); `Expect answer` goes with one `Client` (`example-client-missing`).

A screen example has one `Expect` item or more. A key is in a block one time only. A path is
relative, with `/` between names, and stays inside the folder (no `..`, no `:`, no `\`).

A **string** is in double quotes, with the escapes `\n`, `\t`, `\r`, `\0`, `\\`, `\"` and
`\u{<hex>}`. A long string can instead be a fenced block on the lines below an item whose value
is empty; its lines, less the fence's indent, each with a line end, are the string:

~~~
- Expect standard output:
  ```
  2
  ```
~~~

A **`### Cases:` block** is an example that lengua draws many runs from (section 7). It has one
range or more:

- ``- Range `<name>`: integers from <low> to <high>``, with low not greater than high;
- ``- Range `<name>`: text of <low> to <high> characters from "<characters>"``, lengths from 0 to
  65536, with one character or more.

Its other items are the example keys, and a string there can name a range as `{<name>}`
(`{{` and `}}` are braces). Every name must be a range of the block, and every range must be
used. Ranges and `{...}` are only in Cases blocks; in an Example, a brace is a brace. lengua
draws boundary values first, then 20 values from a fixed seed (section 7, "How the tests
run").

**`# Constraints`**: only list items, one line each, `- <Key>: <value>`:

| Key | Value | Where |
|---|---|---|
| `Reads` | `standard input`, `arguments`, `the clock`, `random numbers` or ``files in `<folder>` `` | top idea |
| `Writes` | ``files in `<folder>` `` | top idea |
| `Connects to` | `` `<host>:<port>` ``, a name or an IP address and a port (since L7) | top idea |
| `Listens on` | `TCP port <number>`, 1 to 65535 (since L7) | top idea |
| `Serves` | `HTTP on TCP port <number>`: the idea is a server (since L7) | top idea |
| `Memory` | a number and `KiB`, `MiB` or `GiB`, up to 4 GiB; required | top idea |
| `Time` | a number and `ms` or `s`, up to 3600 s; required | top idea |
| `Modules` | module names in backticks, commas between them | top idea |
| `Exports` | function and type names in backticks, commas between them: the idea is a library | top idea |
| `Uses` | module names, or folders of lengua libraries (a name with a `/`, relative to the folder: `` `../../libraries/json` ``), in backticks, commas between them | any idea; a library only in the top idea |
| `Uses crate` | `` `<name>` <major.minor.patch>, for <reason> `` | any idea |
| `Uses component` | `` `<file>.wasm`, built from <words> ``: a component from another language, a file inside the folder (since L7) | top idea |

Standard output and standard error are always granted. Every file in `ideas/` must be in
`- Modules:`, and every name there must have its file. A `Uses` name with no `/` must be a module
of the program; one with a `/` is a lengua library (L7). The same line two times is a finding.

**The shape of the top idea** (since L7). An idea is a *program* (it exports `wasi:cli/run`), a
*server* (`- Serves:`; it exports `wasi:http/incoming-handler`) or a *library* (`- Exports:`; a
Rust `[lib]`). Two of them at once is `constraint-kind`. A library has no grants of its own, no
libraries and no components (`library-grant`): the program that uses it grants what it reaches,
and its examples are calls (`example-library-call` otherwise). A server's examples are requests
(`example-server-request`), and a server reads no standard input and no arguments
(`server-input`). A server and a library need no `main` in `# Structure`. Each module must have
its `mod <name>;` line in `src/main.rs` (`module-undeclared`), and each name of `- Exports:` a
`pub` item or a `pub use` there (`export-missing`).

**`# Structure`**: only list items, ``- `<name>`: <one sentence>.``, where the name is a
function, `snake_case`, or a method, `Type::snake_case` with the type in `UpperCamelCase`. The
sentence may continue on indented lines; it is one STE sentence and ends with a full stop. A
name is there one time only. The top idea's structure has `main`. Since L4 `check` holds
these names to the functions of the Rust file (rule 12, section 5).

**What `check` replies.** One pass reads every idea file and collects every finding of every
kind: the frontmatter, the sections, the behaviours and their blocks, the constraints, the
structure, the modules, and STE. Each finding has the file (relative to the folder, `/`
between names), the line (from 1), a code (for example `ste-word` or
`example-exit-code-missing`) and a plain message. The findings are sorted by file, then line,
then code. With no findings, the reply also lists the files `check` wrote and its notes.

### What the STE check holds

[ASD-STE100](https://www.asd-ste100.org/) Simplified Technical English is a controlled
language: a dictionary of approved words, each with one meaning and one part of speech, and
writing rules. `check` holds the rules a machine can check, with its own code and its own word
list, no external crate:

| Rule | How `check` holds it | Code |
|---|---|---|
| Approved words only | every word must be in lengua's dictionary (`crates/lengua-idea/src/ste-words.txt`), as itself or as a regular form: `-s`, `-es`, `-ies`, `-ed`, `-d`, a doubled last letter and `-ed` | `ste-word` |
| Unapproved words have approved ones | a short list (`ste-replace.txt`) names the word to use: `utilize` gives `use` | `ste-word-unapproved` |
| No `-ing` verbs | a word that is a dictionary verb plus `-ing` is refused, unless it is listed as a noun (`string`, `warning`) | `ste-word-ing` |
| Technical names are allowed | words in backticks, strings in quotes, numbers, words with a digit or `_`, words with a capital after the first letter (`WebAssembly`), and capitalized words that do not start the sentence (`Rust`) | — |
| A procedure has 20 words or fewer | a sentence that starts with a dictionary verb is an instruction | `ste-sentence-length` |
| A description has 25 words or fewer | every other sentence; a word in backticks, a string and a number count as one word | `ste-sentence-length` |
| A paragraph has six sentences or fewer | in `# Purpose` | `ste-paragraph-length` |
| One instruction in a sentence | an instruction with `and` before a second verb | `ste-one-instruction` |
| The active voice | in an instruction, a form of *be* and a past participle; in a description, the same followed by `by` | `ste-passive` |
| Simple tenses only | *has*, *have* or *had* and a past participle | `ste-tense` |
| Noun clusters of three words or fewer | four words in a row that the dictionary knows only as nouns | `ste-noun-cluster` |

What it does not check: that a word has its one approved meaning and part of speech; the full
official dictionary (it is the ASD's, so lengua's list is its own, of the general words STE
approves that lengua knows, the words of programs, and their forms); missing articles; vague
words and the rules of style. The agent writes to those; `check` holds the rest. The texts
checked are the purpose, the clauses of the behaviours, the names of behaviours and blocks
(words only), the structure's sentences and the reasons of `Uses crate` (words only).
Constraints, keys and values are not prose and are not checked.

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
`wasm32-unknown-unknown` first when the application has a front end (L8), then for
`wasm32-wasip2` with the front end embedded. It replies `building` until the build ends, then
goes on. The result is one WebAssembly component, the one file. The job, its states and the
reply are in "The generated files, the build job and `doctor`" below.

The user needs Rust and its `wasm32-wasip2` and `wasm32-unknown-unknown` targets installed.
`lengua-mcp doctor` checks both and says the one command to run if either is missing.

### Grants become the component's world

The idea's grants generate the WIT world in `wit/world.wit`. **A grant controls what a program
can reach, not what the standard library imports** (decided at L3, 2026-11-13). Rust's `std`
imports `wasi:cli/environment`, `stdin`, `terminal-input`, `terminal-stdin` and
`wasi:clocks/monotonic-clock` even for hello world (section 10), so the world has an *always*
set that holds them, and the host links them with nothing behind them unless the idea grants
them: no arguments and no environment variables, an empty standard input, no terminal, and a
monotonic clock that only moves forward, 1 ms at each read. Since L5 the always set also holds
`wasi:random/insecure-seed`, from which `std`'s `RandomState` takes the keys of every `HashMap`
and `HashSet` (section 10): without `- Reads: random numbers` the host gives it a fixed value,
so a `HashMap` works with no grant and gives the same order on every run, while random numbers
themselves (`wasi:random/random` and `insecure`) still need the grant. Every other interface
(files, sockets, random numbers, the wall clock, HTTP) is in the world only when granted.

Two checks hold it:
- **At check:** when the build ends, `check` compiles the component with Wasmtime and reads its
  imports (`Component::component_type().imports()`). Each import must be an interface of the
  world, at any WASI `0.2` version; anything else is the finding `import-ungranted` on
  `idea.md`, with the grant that would allow it, and the build is `build_failed`.
- **At run:** the host links only the world's interfaces, so a component that imports anything
  else fails to instantiate, and `run` replies `refused` with Wasmtime's message. On top of the
  world, lengua's policy: files only in the folders the idea names, memory and time limits from
  the idea; since L7, connections only to the addresses it names and binds only on the ports it
  names (section 4, "The host").

### The generated files, the build job and `doctor`

Decided at L2 (2026-11-13).

**When `check` writes them.** After the idea checks of L1 pass, `check` makes the text of
`Cargo.toml` and `wit/world.wit` from the ideas, holds the files already there to it (below),
and, with no findings, writes them in the same step as the wiki. A file whose text did not
change is not written again. The top idea's frontmatter records the SHA-256 of each
(`cargo_sha256`, `world_sha256`), and the log has a line for each one that is new or changed:
``* Generated `Cargo.toml`, changed: `<sha256>`.``

**How they are held.** A generated file that is there is accepted when its SHA-256 is that of
the text `check` makes now, or the one in the frontmatter (the text the last check wrote, so a
change of the ideas, or a newer lengua, simply writes it again). Any other text is a hand edit:
the finding `generated-edited`, at the first line that differs, and nothing is written or
built. To fix it, remove the file (or undo the edit) and change the ideas instead.

**`Cargo.toml`**, for the folder `hello` with no crates:

```toml
[package]
name = "hello"
version = "0.1.0"
edition = "2024"
publish = false
include = ["src/main.rs"]

[[bin]]
name = "hello"
path = "src/main.rs"

[lints.rust]
warnings = "deny"
unsafe_code = "forbid"
unused_must_use = "deny"

[lints.clippy]
pedantic = { level = "deny", priority = -1 }
unwrap_used = "deny"
expect_used = "deny"
indexing_slicing = "deny"

[profile.release]
opt-level = 3
panic = "abort"
lto = true
codegen-units = 1
strip = true

[workspace]
```

- `name` is the folder's name. It must start with a lowercase letter and have only lowercase
  letters, digits, `-` and `_`, and not be a Rust keyword or a name Cargo keeps (`std`, `test`,
  `build`, `deps`, `examples`, `con`, ...); otherwise the finding `package-name`.
- `include` lists the Rust file of every idea, in the order of the ideas: `src/main.rs`, then
  `src/<module>.rs` and `src/ui/<screen>.rs`. Rust finds a module through its `mod` line, so
  this is the package's list of files, not how it is built.
- A program is a `[[bin]]`. A library (`- Exports:`, L7) is a `[lib]` instead, with the name in
  `snake_case` and `path = "src/main.rs"`, and its `[lints.clippy]` also allow
  `missing_errors_doc` and `missing_panics_doc` (rule 11 forbids the doc comments they ask for).
  A server (`- Serves:`, L7) is a `[lib]` with `crate-type = ["cdylib"]`, which the Rust
  toolchain makes into a component: `target/wasm32-wasip2/release/<snake_case name>.wasm`.
- `[dependencies]` is there only when an idea has `- Uses crate:` or library lines: one line per
  crate, ``<name> = "=<version>"``, pinned to the exact version, and since L7 one line per
  library, ``<name> = { path = "<folder>" }``, all sorted by name. Every idea's crates are the
  program's; the same crate at two versions is the finding `crate-version`.
- `[lints]` holds the rules of section 5 that Cargo's `[lints]` can hold (rules 6, 7, 9, 10).
  `cargo build` holds the `rust` lints; the `clippy` lints take effect when clippy runs, at L4.
- `[profile.release]`: `panic = "abort"` (a panic stops the component, with no unwinding code);
  `opt-level = 3`, the speed setting, since lengua is to be faster than C, C++ and Rust, not the
  smallest; `lto = true` and `codegen-units = 1`, so the whole program is optimized as one unit,
  which gives smaller and faster code at a longer build, which the background job hides.
- `[workspace]`, empty, makes the folder its own Cargo workspace, so a program inside another
  workspace (as `examples/hello` is inside lengua's) builds alone.

**`wit/world.wit`**, for hello world:

```wit
package lengua:program;

world program {
  import wasi:cli/environment@0.2.12;
  import wasi:cli/exit@0.2.12;
  import wasi:cli/stderr@0.2.12;
  import wasi:cli/stdin@0.2.12;
  import wasi:cli/stdout@0.2.12;
  import wasi:cli/terminal-input@0.2.12;
  import wasi:cli/terminal-output@0.2.12;
  import wasi:cli/terminal-stderr@0.2.12;
  import wasi:cli/terminal-stdin@0.2.12;
  import wasi:cli/terminal-stdout@0.2.12;
  import wasi:clocks/monotonic-clock@0.2.12;
  import wasi:io/error@0.2.12;
  import wasi:io/poll@0.2.12;
  import wasi:io/streams@0.2.12;
  import wasi:random/insecure-seed@0.2.12;

  export wasi:cli/run@0.2.12;
}
```

The world is a subset of WASI 0.2's `wasi:cli/command`
([command.wit](https://github.com/WebAssembly/WASI/blob/v0.2.12/proposals/cli/wit/command.wit),
[imports.wit](https://github.com/WebAssembly/WASI/blob/v0.2.12/proposals/cli/wit/imports.wit)):
one `import` line per interface, sorted, each once, and `export wasi:cli/run` for a program,
`export wasi:http/incoming-handler` for a server (L7), and no export for a library. The interfaces
of each grant, with the interfaces their types come from:

| Grant | Interfaces |
|---|---|
| always (standard output, standard error, the exit code, and what `std` imports with no grant, since L3) | `wasi:cli/environment`, `exit`, `stdin`, `stdout`, `stderr`, `terminal-input`, `terminal-output`, `terminal-stdin`, `terminal-stdout`, `terminal-stderr`; `wasi:clocks/monotonic-clock`; `wasi:io/error`, `poll`, `streams`; since L5 `wasi:random/insecure-seed` (the keys of `HashMap`) |
| `- Reads: standard input` | none more: the host gives the input (below) |
| `- Reads: arguments` | none more: the host gives the arguments, and never environment variables |
| `- Reads: the clock` | `wasi:clocks/wall-clock`; the host gives its real clocks |
| `- Reads: random numbers` | `wasi:random/random`, `insecure` (and `insecure-seed`, already in the always set) |
| `- Reads:` or `- Writes: files in ...` | `wasi:filesystem/types`, `preopens`; `wasi:clocks/wall-clock` (for the type `datetime`) |
| `- Connects to: ...` or `- Listens on: ...` | `wasi:sockets/network`, `instance-network`, `tcp`, `tcp-create-socket`, `ip-name-lookup`, and since L7 `udp` and `udp-create-socket`; `wasi:clocks/monotonic-clock` (for the type `duration`), and since L7 `wasi:clocks/wall-clock`, which `std`'s sockets import (run 43464215484) |
| `- Serves: HTTP on TCP port N` (L7) | `wasi:http/types` |

UDP is in no grant: since L7 the world of a network grant names `wasi:sockets/udp` and
`udp-create-socket`, because `std`'s sockets (through `wasi-libc`) can import them with TCP, and
the host links them with UDP off, so every UDP call fails. The version is `0.2.12`, the current WASI 0.2 release; at L3 the
component's imports are held to the world by package and interface name, with any `0.2`
version, since WASI 0.2 releases only add to the interfaces. The folders, hosts, memory and time
are not in the world: the host reads them from the idea at L3.

The world is not given to a binding generator. A `wasm32-wasip2` program that uses only the
standard library needs no `wit-bindgen`: `std` reaches WASI 0.2 itself, and the Rust toolchain
makes the component. So the world needs no `wit/deps/` folder, and it is the statement the
component is held to at L3. Since L7 a server takes its bindings from the crate `wasip2`
(`wasip2::http::proxy::export!`), which carries WASI 0.2.12's own WIT, and a program that uses a
component from another language takes the bindings of its interface from `wit-bindgen`'s
`generate!` with the interface written inline and `path: []`, so that it does not read `wit/`; neither needs `wit/deps/`. The interfaces a
trusted component exports are not in `world.wit`: `check` reads them from the component and
holds the program's imports to the world and to them.

**The build job.** With no findings and a `src/main.rs`, `check` starts a build in the
background, in the program's folder, on a thread with one child process:

- `cargo fetch --target wasm32-wasip2`, only when the ideas use crates and `Cargo.toml` changed
  since the last fetch (the folder `target/lengua/` keeps the SHA-256 of the last one);
- since L4, `cargo fmt --check`, whose differences are `rust-format` findings, then `cargo
  clippy --release --target wasm32-wasip2 --offline --message-format=short -- -D warnings`
  (section 5);
- then `cargo build --release --target wasm32-wasip2 --offline --message-format=short`;
- since L5, when the component checks pass, the tests (`testing`), then the mutants
  (`mutating`), on the same thread (section 7).

The job's key is the SHA-256 of `Cargo.toml`, `wit/world.wit`, every file in `src/` and,
since L5, the tests of every idea with the grants (so a change of an example runs the tests
again, and a change of the purpose alone does not). A
`check` with the same key replies with the job's state and starts nothing; a `check` with a
new key stops the job that runs (if one does) and starts a new one. A built component that is
no longer there is built again. Before a new job, `check` runs `doctor`'s checks; when a part
is missing, the build is `build_failed` with one finding, `toolchain-missing`, whose message is
`doctor`'s lines for the missing parts.

**The reply.** With no idea findings, `check`'s reply has `build`:

| `state` | Means | Also in `build` |
|---|---|---|
| `no_code` | there is no `src/main.rs` yet; nothing is built | `target` |
| `building` | the build runs; call `check` again | `seconds` so far, `target` |
| `build_failed` | the build failed | `seconds`, `target`; the errors are in `findings` |
| `testing` (since L5) | the component is built and the tests run; call `check` again | `component` (its absolute path), `size` (bytes), `sha256`; `tests` with `total`, `run` and `failed` |
| `tests_failed` | a test failed | as `testing`; each failed run is a finding (section 7) |
| `mutating` | every test passed and the mutants run; call `check` again | as `testing`; `mutants` with `total`, `done`, `caught`, `survived`, `unbuilt`, `same` and `cached` |
| `done` | the job ended: built, tested and measured | as `mutating`; `seconds` it took |

L4's state `built` is gone: a built component goes on to `testing`. Next to `build`, the reply
has `label` (`unchecked` until `done`, then `checked` or `tested`, section 6) and `score` (the
mutation score in percent, or none when no mutant counted), and each mutant that survived is a
note with its file and line. No tool call waits for the job: each `check` replies at once.

The compiler's errors and warnings become findings, all of them, in the format of L1: the
file relative to the folder (with `/`), the line, the code `rust-error` or `rust-warning`, and
the message `column <n>: <the compiler's line>`, for example ``column 22: error[E0308]:
mismatched types``. An error with no place (Cargo's own) is `build-error` on `Cargo.toml`, line
1. `ok` is true when there are no findings, so it is also true while the build runs.

**`lengua-mcp doctor`** checks, in the current folder (so a `rust-toolchain.toml` there
counts), that `cargo` and `rustc` run, that `rustc` is 1.85 or later (edition 2024), that
`cargo fmt` and `cargo clippy` run (since L4; the command to install one is `rustup component
add rustfmt` or `clippy`), and that
the targets `wasm32-wasip2` and `wasm32-unknown-unknown` are installed: by `rustup target list
--installed` when `rustup` is there, otherwise by `rustc --print sysroot` and the target's
folder under `lib/rustlib/`. It writes one line per part, `ok       <part>: <version>` or
`missing  <part>: not found; to install it, run: <command>`, then one line that says whether
lengua can build, and stops with exit code 0, or 1 when a part is missing. The commands:
`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh` (on Windows `winget install
--id Rustlang.Rustup`) for Rust, `rustup update stable` for an old `rustc`, and `rustup target
add <target>` for a target.

### The host

Wasmtime with the component model and its WASI 0.2 implementation. In tests the clock is fixed
and random numbers come from a seed, so every run gives the same output on every system.

Built at L3 (2026-11-13), in `crates/lengua-mcp/src/host.rs`, on Wasmtime 50.0.1 (`wasmtime`
with only `cranelift`, `runtime`, `std` and `component-model`; `wasmtime-wasi` with only `p2`;
`wasmtime-wasi-io`), with no `unsafe` in lengua's code. One engine serves the server, with
epoch interruption on and a thread that moves the epoch every 10 ms. Each run has its own store
and a linker that holds the world's interfaces and nothing else, each added by its own
`add_to_linker` (the synchronous bindings of `wasmtime_wasi::p2`).

Since L6 the host is its own crate, `crates/lengua-host`, on `wasmtime` with only `runtime`,
`std` and `component-model` (no compiler), `wasmtime-wasi` with `p2` and `wasmtime-wasi-io`,
shared by `lengua-mcp` and `lengua-run`. `lengua-mcp` adds `cranelift` to its own `wasmtime`, to
compile and seal; every run, of `run` and of the tests, loads sealed precompiled code in a
process of its own (section 4, "The lock").

How each grant maps to the host:

| Grant | The host |
|---|---|
| always | standard output and standard error to memory (16 MiB each, returned by `run`); standard input empty; no terminal; the exit code; since L5, `insecure-seed` with a fixed value unless random numbers are granted |
| none for the environment | no environment variables, ever; no arguments unless granted |
| `- Reads: standard input` | the `stdin` of the `run` call as the standard input; without the grant, `run` with `stdin` is refused |
| `- Reads: arguments` | the folder's name and the `arguments` of the `run` call; without the grant, `run` with arguments is refused |
| `- Reads: the clock` | the real wall clock and monotonic clock; without it, the monotonic clock moves 1 ms at each read, and the wall clock (there only for the files' type `datetime`) reads 2026-01-01T00:00:00Z |
| `- Reads: random numbers` | Wasmtime's random number generators, and its real `insecure-seed` |
| ``- Reads: files in `<folder>` `` | the folder, preopened under its own name (`data/...` in the program), read-only; a folder that is not there is a refusal |
| ``- Writes: files in `<folder>` `` | the folder, preopened read-write, made when it is not there |
| `- Memory:` | a `ResourceLimiter` refuses to grow a memory past it; the program stops and `run` replies `out_of_memory` |
| `- Time:` | the store's epoch deadline (the time over 10 ms, plus one); the program stops and `run` replies `out_of_time` |
| `- Connects to:` (L7) | TCP on and UDP off; each granted `host:port` resolved when the run starts; the socket address check of `wasmtime-wasi` permits a connection only to those addresses, and the bind of the wildcard address that a connection makes; name lookup only when a granted host is a name |
| `- Listens on:` (L7) | TCP on; a bind and a listen only on the granted ports, of `127.0.0.1` or of any local address; every client accepted |
| `- Serves: HTTP on TCP port N` (L7) | the server is a component that exports `wasi:http/incoming-handler`; `wasmtime-wasi-http`'s synchronous bindings give it each request, with hooks of lengua's own that refuse every outgoing request; `run` and the tests give one request on the standard input and get the answer as HTTP/1.1 bytes; `lengua-run` serves `127.0.0.1:N`, one request at a time, each under the idea's time and memory |
| `- Uses component:` (L7) | the host instantiates the component first, in the same store and under the same grants, and links each function of each interface it exports into the program's imports |

Wasmtime keeps a preopened folder's paths inside it: `..` out of it, an absolute path, and a
symbolic link that points out of it are refused (tested at L3).

**Fixed runs.** `run` with `fixed: true`, as the tests run since L5, gives the wall clock
2026-01-01T00:00:00Z, the stepped monotonic clock, and random numbers from a fixed seed
(Wasmtime's `Deterministic` over 4,096 bytes made by SplitMix64), so two runs give the same
output on every system.

**The `run` tool.** `run {folder, arguments?, stdin?, fixed?}` verifies `lengua.lock` first
(section 4, "The lock"), and refuses a program whose files are not as the lock signed them;
then it reads the ideas without writing anything (`lengua_idea::program`), refuses a folder
whose ideas have findings and input the idea does not grant, and runs the program in a process
of its own: `lengua-mcp worker`, which verifies the lock again, loads the sealed code and runs
it with its output in memory (since L6; before, the built component from `target/` ran on a
thread of the server). It waits up
to 30 seconds; a run that is still going then replies `running`, and a `run` with the same
folder, component and input waits for the same run. The reply:

| `state` | Means | Also |
|---|---|---|
| `running` | the run goes on; call `run` again with the same input | `milliseconds` |
| `exited` | the program ended | `exit_code`, `stdout`, `stderr` |
| `out_of_time` | the program ran past its idea's `- Time:` | `message`, `stdout`, `stderr` |
| `out_of_memory` | the program grew its memory past its idea's `- Memory:` | `message`, `stdout`, `stderr` |
| `trapped` | the program stopped with a trap (a panic aborts with one) | `message`, the trap's text |
| `refused` | the host did not start it: no lock, a file not as the lock signed it, a lock signed with another key, ideas with findings, input not granted, an import the world lacks | `message` |

A host call that blocks (a long sleep, a blocking read) is not stopped by the epoch, so since
L6 every run is a process of its own, and lengua stops the process at the idea's time and two
seconds (`Child::kill`, on every system); the reply is then `out_of_time` with no output. This
closed the finding of L3 (section 10).

### Libraries and other languages

- **A lengua library** is another program of the bundle that declares `- Exports:`. A program
  that uses it names it in `- Uses:`, and `check` links it as a Rust path crate. Its own checks
  and tests stand.
- **A component built from another language** (C, Go, anything with a WASI 0.2 target) is linked
  through the component model. lengua did not build or check it, so it is labelled `trusted`.

#### As built at L7

**Applications of several modules.** A module is an idea in `ideas/<name>.md`, named in
`- Modules:`, with its Rust file `src/<name>.rs` and its `mod <name>;` line in `src/main.rs`.
Every check of L1 to L6 holds per module: STE, `# Structure` (rule 12) and the lint on each file,
the call graph across all files, the tests of every idea, the mutants of every file (40 at most,
spread over the functions of all of them), and one lock over every idea and file. **A module's
examples run through the program's own entry** (its standard input, arguments, files, requests),
against the one component: a module of a program has no entry of its own, and a Rust binary
can't be linked into a test harness. The example `tally` is a program of three modules.

**Libraries and their harness.** A library's examples are calls (`- Call:` and `- Expect
value:`). When the library builds, `check` writes a harness package in
`target/lengua/harness`: a binary that depends on the library as a path crate, with `use
<library>::*;` and one line per example that prints the call's value with `{:?}`; it builds it
for `wasm32-wasip2` with the library's target folder, holds its imports to the always set, seals
it, and runs each example as the harness with the example's number as its argument (granted to
the harness only). A call that does not build is the finding `harness-error` at its example.
The mutants of a library are built into a harness of their own (`target/lengua/mutant-harness`,
the package `<name>-mutant` under the crate name `<name>`), so the score of a library is the
score of its examples. A library's label is its own, and its lock is of kind `library`.

**A program that uses a library** names its folder in `- Uses:`. `check` reads the library's
`idea.md`, verifies its lock under the user's key (`library-lock` when it is missing or when a
file changed, `library-missing` when there is no `idea.md`), links it as a path crate, adds the
SHA-256 of its lock to the job's key (so a library checked again builds the program again), and
names the crate in the lint's list of declared crates. The reply's `libraries` lists each one
with its own label, `index.md` has a `# Libraries` section, and the lock a `library` line. The
mutants of the program use the library at its absolute path.

**Network grants and fixtures.** `- Connects to:` and `- Listens on:` give `wasi:sockets` and
lengua's socket address check (section 4, "The host"). An example gives a test server with
``- Server `<host>:<port>`: "<answer>"`` and a test client with ``- Client `<port>`: "<text>"``
and `- Expect answer:`; the ports of the examples in the repository are fixed and different
(7801 to 7811), and a test server waits up to 10 s for its port. The examples `fetch` and
`upper` show a connection and a listen, each granted and refused.

**The HTTP world.** A server's idea has `- Serves: HTTP on TCP port N`, its Rust implements
`wasip2::exports::http::incoming_handler::Guest` and calls `wasip2::http::proxy::export!`, with
`- Uses crate: `wasip2` 2.0.1` (the current WASI 0.2 bindings, 2.0.1+wasi-0.2.12; the older
`wasi` crate stopped at 0.14.7 for WASI 0.2.4). The host serves it with `wasmtime-wasi-http`
(section 4, "The host"). The examples `greet` and `shop` are servers.

**A component from another language.** ``- Uses component: `components/tax.wasm`, built from
...`` names a component file in the folder. `check` compiles it with lengua's engine, holds its
imports to the program's world (`component-import-ungranted`), reads the interfaces it exports
and holds the program's imports to the world and to them, and seals it next to the program
(`sealed-trusted` in the locks). The program imports the interface with `wit-bindgen`'s
`generate!` and the interface's WIT inline (`- Uses crate: `wit-bindgen` 0.62.0`, the version
`wasip2` 2.0.1 uses, so one `wit-bindgen` is in the graph). The host instantiates the component
first and links its exports into the program's imports (no `wac`: the host's own linker does
it). The reply's `components` lists it with the label `trusted`, a note says the program uses a
trusted component, and `index.md` has a `# Components` section. The example `shop` uses
`components/tax.wasm`, which CI builds from `rates.c` and `tax.wit` with wasi-sdk 34 and the
`wit-bindgen` 0.62.0 command line, both pinned by SHA-256 and downloaded in CI only; no binary
is in the repository.

### Crates: few, and only ones that build for WebAssembly

**Use the Rust standard library; take an external crate only when it is absolutely necessary.**
In the age of AI, an agent can write its own code, checked and tested to lengua's rules, where
before a team would have taken a crate. Every external crate is code lengua didn't write and
can't hold to its rules. An exception has to merit it, as GPUI does.

A crate merits it only when all of these hold, and the idea's `- Uses crate:` line says which:
- writing it would be a large project of its own (a UI framework, a GPU renderer), or it is the
  component's own interface to its host, which can't be written any other way;
- the standard library, lengua's shared libraries and the agent's own code can't do the job;
- it builds for every target the application needs.

Convenience never merits it: saving a few hundred lines is what the agent is for.

The exceptions that merit it, each still declared in the idea with its reason:
- **GPUI, its web crate and GPUI Kit**, for the front end: a GPU-drawn UI framework is not
  something to write again.
- **The WASI bindings** for what the standard library does not cover, such as WASI's HTTP world
  for a server: the generated interface code (`wit-bindgen` or the `wasi` crate). It is the
  component's own interface, not a library of features.

Not exceptions: **Axum** and **Tokio**. A server's routing and its requests are written on the
standard library and lengua's shared HTTP library, over the WASI bindings. A WASI component has
one thread, so an async runtime brings little. (Axum can run in the WASI HTTP world through
`wstd-axum`, [docs](https://docs.rs/wstd-axum), if an application ever truly needs it.)

The `build` skill chooses in this order:

1. **The Rust standard library** ([std](https://doc.rust-lang.org/std/)), as far as it goes.
   It is part of the toolchain, so it is no dependency at all, and most of it works in a WASI
   component: collections, strings, formatting, files, the clock, standard input and output.
   What it lacks for WebAssembly (threads, for one) the application does without.
2. **A lengua library** already in the bundle, or in lengua's own shared libraries (section
   below), themselves built on the standard library alone. Written to the same rules, checked
   and tested like the application.
3. **The agent writes its own** as a lengua library, on the standard library: its own idea, its
   own tests, no comments, the Power of 10. It then serves every later application too.
4. **A crate from crates.io**, only as an exception (above), and only if it builds for both
   targets, `wasm32-wasip2` and, for the front end, `wasm32-unknown-unknown`.

What `check` does with each external crate:
- It must be declared in the idea with `- Uses crate: <name> <version>, for <what>`. The reason
  is part of the documentation.
- It must build for each target the application needs. A crate that needs threads, a C library
  or one operating system's own calls fails, and the reply names the crate and the reason.
- It is pinned by checksum in `Cargo.lock` and signed in the lock.
- The reply counts the external crates, the crates they bring in with them, and which of them
  use `unsafe`. That count is in `index.md`, so it can be watched as the application grows.

### lengua's shared libraries

lengua ships a small set of its own libraries, written as lengua programs on the standard library alone, so the common needs
of an application don't send it to crates.io: reading and writing JSON, parsing and building
HTTP messages, decimal numbers, text and lines, sorting, CRC and hashes, dates and times. Each
has its own idea, tests and lock, and builds for both targets. GPUI and GPUI Kit stay external:
writing a UI framework is not lengua's work.

Built at L7, in `libraries/` at the repository's root. Each is a lengua library on the standard
library alone, with no recursion, no `unsafe` and every rule of section 5, checked to the label
`checked` by the tests, and built for `wasm32-unknown-unknown` too by CI. A program names one
with ``- Uses: `<path>/libraries/<name>` ``; the `build` skill (L9) will point to them.

| Library | Exports |
|---|---|
| `json` (modules `read`, `write`) | `Value` (`Null`, `Bool`, `Number(String)`: the number's text, `Text`, `List`, `Object(Vec<(String, Value)>)`: fields in order), `Error` (`End`, `Unexpected(position)`, `Escape(position)`, `Number(position)`, `Deep`), `parse(&str) -> Result<Value, Error>` (RFC 8259, a stack of at most 512 frames), `write(&Value) -> String` (no spaces), `get(&Value, &str) -> Option<&Value>` |
| `http` | `Request` (`method`, `target`, `headers`, `body`), `Response` (`status`, `reason`, `headers`, `body`), `Error`, `parse_request(&[u8])`, `parse_response(&[u8])`, `write_request(&Request) -> Vec<u8>`, `write_response(&Response) -> Vec<u8>`, `header(&[(String, String)], &str) -> Option<&str>` (any case), `decode_chunked(&[u8]) -> Result<(Vec<u8>, usize), Error>`, `encode_chunked(&[u8], usize) -> Vec<u8>` (RFC 9112) |
| `decimal` | `Error` (`Empty`, `Invalid(position)`, `Overflow`, `TooPrecise`), `parse_integer(&str) -> Result<i64, Error>`, `parse_fixed(&str, places) -> Result<i64, Error>` (a number of the smallest units), `format_integer(i64)`, `format_fixed(i64, places)`, `add` and `multiply` (`Overflow` past `i64`) |
| `text` | `lines(&str) -> Vec<String>` (with `\n` or `\r\n`), `split(&str, char)`, `words(&str)`, `trim(&str) -> String` (space, tab, line end, carriage return), `check_utf8(&[u8]) -> Result<(), position>`, `is_utf8(&[u8]) -> bool` (lengua's own check, held to `std`'s) |
| `sort` | `stable_sort(&mut [T])`, `stable_sort_by_key(&mut [T], key)` (a bottom-up merge sort with one buffer), `binary_search(&[T], &T) -> Option<usize>` (the first equal item) |
| `hashes` | `sha256(&[u8]) -> [u8; 32]`, `sha256_hex(&[u8]) -> String` (FIPS 180-4), `crc32(&[u8]) -> u32` (ISO 3309, `zlib`) |
| `dates` | `Date` (`year`, `month`, `day`), `Error` (`Form(position)`, `Month`, `Day`, `Time`), `is_leap`, `days_in_month`, `days_from_civil(Date) -> i64` and `civil_from_days(i64) -> Date` (day 0 is 1970-01-01, the Gregorian calendar), `days_between`, `parse_date` and `format_date` (`YYYY-MM-DD`), `parse_timestamp` and `format_timestamp` (`YYYY-MM-DDTHH:MM:SSZ`, seconds from 1970) |

### The lock

The lock is what keeps every document current. `lengua.lock` holds, signed together with an
HMAC under the user's key, the hashes of:
- every idea, the top one and each module's and screen's;
- `index.md` and `log.md`, and the component pages (since L9);
- every file in `src/` and the generated files;
- `Cargo.lock`, which pins every dependency by checksum, and the toolchain's version;
- both compiled parts and the one file.

It also names the sealed, precompiled form of the component that `run` and `lengua-run` load.

`check` writes the wiki (the frontmatter, `index.md` and `log.md`) in the same step as the lock,
and only when every check passed. So there is no moment when the lock is current and the
documentation is not. A change to any idea, any code or any document without a new check means
the lock no longer matches, and `run` refuses the program.

#### The lock as built

Decided at L6 (2026-11-13), in `crates/lengua-idea/src/lock.rs` (the key, the format, the
check), `crates/lengua-seal` (loading sealed code), `crates/lengua-host` (the host, shared),
`crates/lengua-run` (the lean runner) and `crates/lengua-mcp` (`check` writes it, `run` and the
worker check it).

**The user's key.** 32 random bytes, made by the first `check` (`getrandom`, already in the
dependency graph through Wasmtime's `rand`; the standard library has no source of random bytes
on every system) and kept as 64 hexadecimal digits and a line end in the file `key` of lengua's
data folder:

| System | The data folder |
|---|---|
| any, for tests and for a person who wants another | `$LENGUA_HOME` |
| Linux | `$XDG_DATA_HOME/lengua` (when it is an absolute path), otherwise `~/.local/share/lengua` |
| macOS | `~/Library/Application Support/lengua` |
| Windows | `%APPDATA%\lengua` |

On Linux and macOS the folder is made owner-only (`0700`) and the key `0600`; on Windows
`%APPDATA%` is already the user's own. The key is written to a new file and then linked into
place, so two servers that start at once keep one key. Nothing else reads it: it signs, it is
never sent anywhere, and a lock made on another computer or by another user is refused there,
so a lock is this user's statement that `check` passed here.

**The format of `lengua.lock`.** Text, one entry a line, each line ending with a line end
(`\n`, on every system), in this order:

```
lengua-lock 1
kind program
lengua 0.1.0
toolchain rustc 1.100.0 (…)
label checked
score 6 of 6
file 3b0f… idea.md
file 9c1e… ideas/words.md
file 77aa… index.md
file 0d4c… log.md
file 51f2… src/main.rs
file a903… src/words.rs
file e1b0… Cargo.toml
file 4c88… wit/world.wit
file 0f6d… Cargo.lock
component 8a2b… target/wasm32-wasip2/release/words.wasm
sealed 6e19… target/lengua/sealed/8a2b….cwasm
hmac-sha256 2c5d…
```

- `lengua-lock 1` is the format's version; `kind` is `program` (the lock `check` writes) or
  `test` (the locks the tests use, below); `lengua` is lengua's version.
- `toolchain` is the first line of `rustc --version` in the program's folder, as `doctor`
  read it when the job started; `label` and `score` are the program's (section 6), `score`
  as `<caught> of <counted>`, or `none`.
- Each `file`, `component` and `sealed` line is a tag, the SHA-256 of the file in lowercase
  hexadecimal, and its path relative to the folder with `/` between names. The files are every
  idea (`idea.md`, then `ideas/*.md` and `ideas/ui/*.md`, each sorted), `index.md`, `log.md`,
  the component pages (`components/*.md`, sorted, since L9),
  every file under `src/` (sorted), the generated `Cargo.toml` and `wit/world.wit`, and
  `Cargo.lock` when the folder has one (Cargo writes it at the first build). Then the
  component that `cargo build` made, and the sealed precompiled code (a library, L7, has no
  `sealed` line).
- The last line is `hmac-sha256` and the HMAC-SHA256 (RFC 2104) under the user's key of every
  byte before that line. lengua's own SHA-256 and HMAC, on the standard library, are tested
  with the vectors of RFC 4231.

The lock holds the front end's compiled part and the one file when there is a front end (L8);
until then the component is the one file.

**Since L7:** `kind` is also `library`, the lock of a lengua library: its files, its `component`
(the `.rlib` Cargo made) and no `sealed` line. A program's lock has, after its files, one
``library <sha256> <folder>/lengua.lock`` line for each library it uses, one
``trusted <sha256> <file>`` line for each component from another language, and after its own
`sealed` line one `sealed-trusted` line for the sealed code of each such component. `verify`
checks each `library` line's hash and then the library's own lock (kind `library`, under the same
key), so a library whose files changed after its check refuses the program at `check` and at
`run`; and it gives the sealed code of the trusted components with the program's. The tests' locks
(`kind test`) carry the `sealed-trusted` lines too.

**When `check` writes it.** In the same `check` call as the wiki, right after it, and only
when the build job is `done` for the same key (the label in the wiki and in the lock is the one
of that job). A check whose job is `building`, `testing`, `mutating`, `build_failed` or
`tests_failed` writes no lock. Until L9 every check added a log entry, so the lock that was
there no longer matched; since L9 a check that changes nothing writes nothing ("The wiki's
formats"), and the lock that was there still matches only when every idea, code file,
generated file and wiki file is exactly as it signed them: a changed file, or a job that ends
`build_failed` or `tests_failed` (the label goes to `unchecked` in the wiki), makes `run`
refuse until a check is `done` again. Every `done` check writes the lock again.

**Sealed precompiled code.** When the build ends and the component checks pass, the job
precompiles the component for this computer (`Component::serialize` of the component lengua's
engine compiled, the same bytes Cranelift made for the import check) and writes it to
`target/lengua/sealed/<the component's SHA-256>.cwasm`, with a test lock next to it
(`<SHA-256>.lock`: `kind test` and one `sealed` line, signed). The tests and the mutants run
from sealed code too, each mutant from its own, removed after its tests. The lock's `sealed`
line names the program's.

**Loading it, the one `unsafe` call.** Wasmtime loads precompiled code only through `unsafe`
functions (`Component::deserialize`, `deserialize_raw`, `deserialize_file`): the bytes are run
as machine code with no check of what they do. docs.wasmtime.dev (Wasmtime 50.0.1) has no safe
way: the safe constructors (`Component::new`, `from_binary`, `from_file`) compile, which needs
Cranelift, and Wasmtime's on-disk cache is only for those. So lengua's own code, which forbids
`unsafe` everywhere, has one exception, decided at L6: the crate `crates/lengua-seal`, the only
manifest without the workspace's lints (it denies `unsafe_code` and allows it on one
function), with exactly one `unsafe` call, `Component::deserialize`, in `load`. `load` takes a
`lengua_idea::lock::Sealed`, which only `lock::verify` makes, after it checked the lock's HMAC
under the user's key and the precompiled file's SHA-256; the bytes loaded are the bytes hashed,
read into memory once. A test of `lengua-seal` counts the `unsafe` blocks of every crate and
fails if there is any other. Wasmtime itself refuses code made by another Wasmtime version, for
another CPU or with other settings, and `run` then says to run `check`.

**What `run` and `lengua-run` refuse.** Before they run anything, both verify the lock, in this
order, and refuse with a message that ends "run `check`":
1. no `lengua.lock`;
2. a signature that is not the HMAC under this user's key (another key, or the lock changed by
   hand);
3. a lock of the wrong kind or format;
4. every file of the lock whose SHA-256 differs (``` `src/main.rs` changed ```) or that is not
   there, and every idea or file under `src/` that is not in the lock (``` `src/extra.rs` is
   new ```), all in one message.

Then the ideas are read (`lengua_idea::program`) for the grants, the input is held to them,
and the sealed code is loaded. `run` verifies in the server first and again in the worker that
loads the code.

**Every run in a process of its own.** A host call that blocks (a sleep, a blocking read) is
not stopped by Wasmtime's epoch, so each run is a process: `run` and each test start
`lengua-mcp worker <kind> <lock> <folder> <root> <fixed | live> [arguments...]`, give it the
standard input, and kill it with `Child::kill` (on every system; no job object is needed, as
the worker starts no process of its own) when the idea's time and two seconds have passed; the
run is then `out_of_time` with no output. The worker writes its outcome as one frame on its
standard output: a line `<ending> <exit code> <message bytes> <stdout bytes> <stderr bytes>`,
then the message, standard output and standard error. `lengua-run` is a process of its own
already: a thread ends it at the same time. The example `sleepy` (a 30 s sleep, `- Time: 1 s`)
tests it.

**`lengua-run`.** `lengua-run <folder> [arguments...]` runs a checked folder from a shell, on
`lengua-host` and `lengua-seal` alone: Wasmtime with no compiler (`runtime`, `std` and
`component-model`; CI fails if `cargo tree -p lengua-run` has a code generator (`cranelift-codegen`,
`cranelift-frontend`, `cranelift-native`, `wasmtime-internal-cranelift`, Winch), and builds it
alone so
that the features of `lengua-mcp` do not join it). It verifies the lock as `run` does, grants
only what the idea grants (arguments only with `- Reads: arguments`; standard input passed
through only with `- Reads: standard input`, otherwise empty; standard output and standard error
passed through; the real clocks only when granted), and stops with the program's exit code, or
its own: 124 out of time, 125 refused, 126 out of memory, 134 trapped, each with a line
`lengua-run: <what>: <message>` on standard error.
Measured on Linux x86-64 (run 43448303904, release builds): `lengua-run` is 6,816,864 bytes and
`lengua-mcp` 27,970,944 (4.1 times larger, with Cranelift, `rmcp` and Tokio); hello world from
its sealed code takes 5.3 ms a run with `lengua-run` and 5.5 ms with `lengua-mcp worker` (20 runs
each: 0.105 s and 0.109 s), and `--version` 1.4 ms and 1.6 ms, so start-up is mostly the
process, not the binary. The release `lengua-run`, with no compiler, loaded the code that the
tests' `lengua-mcp` sealed. The other four systems: todo.md, the landing run.

### The wiki and the log: documentation that can't drift

An application's ideas form a wiki, as in Karpathy's LLM wiki, and `check` keeps it.

- **Every idea is a wiki page** with Open Knowledge Format frontmatter: the top `idea.md` and
  each `ideas/<module>.md`. The frontmatter gives its kind, its module, links to the ideas it
  uses and the ideas that use it, its trust label, OKF's trust fields and the hash of its Rust
  file; its `# Links` section links the same pages as markdown links (since L9). `check`
  writes the frontmatter and the links; the agent doesn't.
- **`index.md`** has one line per idea and module, with its purpose and its label, and the
  libraries, the component pages and the crate count. It is the map an agent, or a person,
  reads to find the right page before reading any code.
- **`log.md`** is append-only. Each check that changes a file adds one entry: the date, which
  ideas and which Rust files changed, their new hashes, the result and the label. When an entry shows a Rust file
  changed with no change to its idea, or an idea changed with no change to its code, `check`'s
  reply says so.

Three things stop the documentation drifting from the code:
1. **Rule 12:** every function is named in its module's `# Structure`, and nothing else is.
2. **The lock:** the ideas', the documents' and the code's hashes are signed together, and
   `run` refuses a program whose files no longer match.
3. **The log:** the history of when each idea and each file changed, so anyone can see they
   moved together.

#### The wiki's formats

Decided at L1 (2026-11-13), and completed at L9 for OKF's trust fields and the LLM wiki's
links. The folder is an OKF v0.2 bundle: every idea is a concept, and `index.md` and `log.md` at
the folder's root are OKF's reserved files (OKF sections 8 and 9).

- **When `check` writes it:** only when the check finds no problems, in the same step for all
  of it (the frontmatter and links of every idea, the component pages, `index.md`, `log.md`),
  so the wiki never shows a state the checks refused. A file whose text did not change is not
  written again, and a check that would change no file writes nothing, not even a log entry
  (since L9). At L6 the lock is written in the same step.
- **The label while the job runs** (since L9). A check whose job has no result yet (`building`,
  `testing`, `mutating`, or no code) writes the label the wiki already has when that leaves
  every file as it is: the same ideas, code and generated files keep the label they were
  checked to, so a new session or a new clone does not write `unchecked` and then `checked`
  again. When anything changed, the label is `unchecked` until the job ends. A job that ends
  `build_failed` or `tests_failed` writes `unchecked`, and `done` writes its label. The job's
  inputs count an example's line from the start of its idea's text, not of the file, so the
  lines `verified` adds to the frontmatter do not start the job again.
- **The frontmatter** of each idea: the keys lengua writes come first, in the order of "The
  idea file's grammar", then the agent's keys as they were, then `title` and `status` when
  they were absent. An example:

  ```
  ---
  type: lengua Module
  description: "The module finds the words in a text."
  module: words
  source: src/words.rs
  uses: []
  used_by: [idea, ideas/ui/summary]
  label: checked
  idea_sha256: 3b0f…
  source_sha256: 9c1e…
  generated: { by: lengua-mcp/0.1.0, at: 2026-11-13T14:02:11Z }
  verified:
    - { by: process:lengua-check, at: 2026-11-13T14:02:11Z }
  title: words
  status: draft
  ---
  ```

- **`# Links`**, at the end of each idea (since L9, "The idea file's grammar"). For the module
  above:

  ```
  # Links

  * Used by: [tally](../idea.md), [ui/summary](ui/summary.md).
  * Index: [tally](../index.md).
  ```

- **Component pages** (since L9). Each component from another language has a concept page,
  `components/<the file's name without .wasm>.md`: `type: lengua Component`, a `description`
  ("A component built from …"), `resource` (the `.wasm` file, from the folder's root, `/components/tax.wasm`),
  `label: trusted`, `generated`, `title` and `status`, a short body that says lengua did not
  build it or check its code, and its `# Links` (used by the top idea, the index). The index
  and the top idea link to the page, not to the `.wasm` file. The pages are in the lock.
- **`index.md`** is written new by every check that writes. It has `okf_version: "0.2"` as its only
  frontmatter (OKF allows this key in a bundle's root index), then the sections that have lines:
  `# Program`, `# Modules` and `# Screens`, with one line per idea,
  ``* [<title>](<file>) - <description> Label: `<label>`.``; `# Libraries`, one line per lengua library with its own
  label; `# Components`, one line per component page, label `trusted`; and `# Crates` (since
  L9), the crate count of section 4, "Crates": the crates the ideas declare, the crates they
  bring in (the external packages of the folder's `Cargo.lock`, so "not known until the first
  build" before it), which of them have `unsafe` in their Rust code (the word, outside line
  comments, in their sources in Cargo's folder), which are not pure Rust (a `links` key with a build script that links a native library, or
  `cc`, `cmake`, `pkg-config`, `bindgen` or `vcpkg` among their build dependencies), and any
  whose source lengua could not read. The count is kept in `target/lengua/crates.txt` under the
  `Cargo.lock`'s SHA-256 once every source was read (before the first build ends, Cargo has
  not downloaded them all; run 43519544263 kept a count made too early). An idea's title is its
  `title` (above).
- **Every page in the folder is a concept** (since L9): each `.md` file of the folder other than
  `index.md` and `log.md` (the ideas, the component pages, and any page the agent adds, such
  as `vision.md` at L11) has frontmatter with a non-empty `type` (`okf-type` otherwise), and
  its OKF values are checked as an idea's are. `target/`, hidden folders and a folder with an
  `idea.md` of its own are not part of the folder.
- **Secrets** (the rule for L9's secrets grant, written now because there is no grant of a
  secret yet, so there is nothing to test). A secret's value is given by the host at run time
  and never read by `check`: it is never written to the frontmatter, `index.md`, `log.md`, a
  component page, the lock or a reply. The idea names a secret, never its value. When the grant
  is built, a test gives a secret whose value is a marked string and asserts that no file
  `check` or `run` writes, and no reply, holds it.
- **`log.md`** starts with `# Log`. OKF orders a log newest first, under `## YYYY-MM-DD`
  headings, so a check puts its entry at the top of today's heading, or adds today's heading
  above the older ones. "Append-only" means that no entry already there is changed or removed.
  An entry is:

  ```
  * **Check** at 14:02:11Z: no problems; label `unchecked`.
    * Idea [words](ideas/words.md), changed: `<sha256>`.
    * Code `src/words.rs`, new: `<sha256>`.
    * Note: the code `src/words.rs` changed and its idea `ideas/words.md` did not.
  ```

  An idea or a code file is *new* when the frontmatter had no hash for it, *changed* when the
  hash differs, and *removed* when its code file is gone. A check that changes only the label
  or the index has an entry with its first line only. Each *Note* is also in the reply's
  `notes`. Since L5 the label is the program's (section 6), and when it is `checked` or
  `tested` the first line also has the score: ``label `checked`, mutation score 6 of 6
  (100%)``, or ``label `checked`, no mutant counted``.

#### One wiki across the repository

Decided at L9 for L13's part (2026-11-13). The repository is an OKF bundle whose folders under
`libraries/` and `examples/` are bundles of their own, each with its root `index.md`:

- **Committed:** the frontmatter and `# Links` of every idea, the component pages, `index.md`
  and `log.md` of each folder in `libraries/` and `examples/`, as `check` wrote them on a
  runner, never by hand. Not committed: `lengua.lock` (signed with one user's key),
  `Cargo.toml`, `wit/`, `Cargo.lock` and `target/`.
- **Written on a runner:** the CI mode `wiki` (`gh workflow run ci.yml -f mode=wiki -f
  targets=linux-x86_64`) runs the ignored test `crates/lengua-mcp/tests/wiki.rs`, which calls
  `check` on every library and then every example, in place, until each job ends, then runs
  `lengua-mcp wiki <repository>`; the files are its artifact `wiki-<run>-linux-x86_64`, which
  is downloaded and committed.
- **The root indexes:** `lengua-mcp wiki <root>` writes them from the folders' frontmatter, so
  `check` stays about one folder: the repository's `index.md` (`okf_version: "0.2"`; `#
  Documents`, the pages at the root; `# Folders`, `libraries/` and `examples/` with their
  counts) and one `index.md` per folder of lengua folders (`# Programs`, `# Servers`,
  `# Libraries`), a line per folder with its description and label, linking to its
  `index.md`. Then it prints every finding of the tree, below, and fails when there is one.
- **Who uses a library.** A library learns its users without its own files changing: `used_by`
  is never written into a library's frontmatter, which is in its lock, so a new user does not
  make every library's lock stale. The program's top idea has the library in `uses` and links
  to it, and the folder index of `libraries/` says ``Used by: [shop](../examples/shop/index.md)``
  from the programs' `uses`.
- **The committed wiki stays as it is:** every test and every check of CI's mode `test` runs on
  copies of the folders (`target/tmp`, the system's temporary folder), never on the committed
  files, and a check that changes nothing writes nothing, so the committed `log.md` grows only
  when the mode `wiki` runs after a change.
- **The test of the tree** (`crates/lengua-idea/tests/wiki.rs`, every runner): every `.md` in
  the repository but `index.md` and `log.md` has frontmatter with a non-empty `type` and valid
  OKF values (OKF section 11); every `index.md` has only headings and list lines, and
  frontmatter (`okf_version`) only at a bundle's root; every `log.md` has `## YYYY-MM-DD`
  headings, newest first; every relative link, outside code, leads to a file or a folder; every
  committed idea's `idea_sha256` and `source_sha256` are those of its files now
  (`wiki-stale`: change an idea or its code, and dispatch the mode `wiki`); and the root
  indexes are the ones `lengua-mcp wiki` writes. Not walked: hidden folders, `target/`, any
  `fixtures/` folder (the tests' deliberately broken ideas) and a folder with a `SKILL.md`
  (the plugin's skills follow the Agent Skills standard, whose frontmatter has no `type`). The
  design documents (this file, `todo.md`, `README.md` and each crate's `README.md`) have a
  `type` of their own: `lengua Design`, `lengua Plan`, `lengua Overview`, `lengua Crate`.

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
| 13 | The standard library first; an external crate only when absolutely necessary (GPUI, its web crate, GPUI Kit, the WASI bindings), declared in the idea (`- Uses crate: name version, for what`), building for every target the application needs, pinned by checksum, signed in the lock, and counted in the reply with whether it uses `unsafe`. Otherwise the agent writes its own, on the standard library. | The generated `Cargo.toml`, `Cargo.lock`, the lock and the reply |

And on the built component: imports only from the world; no threads, no relaxed SIMD and no other
feature whose result can differ between machines; a size limit.

### What each rule accepts and refuses

Decided at L4 (2026-11-13). `check` holds the rules in three places, and all their findings
come in one reply:

1. **lengua's lint** (`crates/lengua-lint`), on the ideas' pass, when `src/main.rs` is there:
   rules 1, 2, 4, 5, 6, 8, 10 (the attributes that turn a warning off), 11 and 12. A finding
   here means nothing is written and nothing is built, as with an idea's finding.
2. **The build job**, in this order: `cargo fmt --check` (a difference is `rust-format`, at the
   file and line `rustfmt` names), then `cargo clippy --release --target wasm32-wasip2
   --offline --message-format=short -- -D warnings` (rules 7, 9, 10 and the `[lints]` of the
   generated `Cargo.toml`; its errors are `rust-error` and `rust-warning`, as the compiler's),
   then `cargo build`.
3. **The component checks**, when the build ends (below).

Each finding of the lint has a code that names its rule:

| Rule | Accepts | Refuses, with the code |
|---|---|---|
| 1 | calls that make no cycle | a function that can reach itself through the call graph (`rule-1-recursion`, one finding on each function of the cycle, with the path); any `dyn` (`rule-1-dyn`); any `fn(` type, a function pointer (`rule-1-fn-pointer`) |
| 2 | `for` over a range with an end, a slice, an array, a collection or an iterator; `while <counter> < <constant>` (or `<=`, or `>`/`>=` counting down, or the constant first), `&&` with other conditions allowed, with `<counter> += 1;` (or `-= 1;`) at the loop's top level; `loop` with a top-level `if <counter> >= <constant> { break ... }` (or `return`; `\|\|` with other conditions allowed) and the step the same way | `while let`; a condition with `\|\|` at its top level; a `while` or `loop` with no counter compared with a constant (an integer literal, or a path whose last name is `UPPER_CASE`); a step that is not 1, not at the top level, or the wrong way; any other change of the counter in the body (`=`, a compound assignment, `&mut`, a `let` of the same name); a `continue` for this loop before the step or the check; `<= ..MAX` and `>= ..MIN`; a `for` over `a..`, `repeat`, `repeat_with`, `from_fn`, `successors` or `cycle`, unless `.take(` or `.zip(` follows (all `rule-2-loop`) |
| 4 | a function of 60 lines or fewer, from the line of `fn` to the line of its `}`, as written (the build's `cargo fmt --check` holds that what is written is what `rustfmt` writes) | a longer function (`rule-4-length`) |
| 5 | at least two `assert!`, `assert_eq!`, `assert_ne!`, `debug_assert!`, `debug_assert_eq!` or `debug_assert_ne!` per function on average, over every function of the program but `main` (`main` and its assertions are not counted, so a program that is only `main` passes) | fewer (`rule-5-assertions`, on the first function with fewer than two) |
| 6 | a `static` whose type is made of numbers, `bool`, `char`, `str`, `Option`, references, arrays, tuples, and the program's own structs and enums of them | `static mut`; a `static` of any other type (`Cell`, `RefCell`, `Mutex`, atomics, `OnceLock`, `String`, a type lengua can't see into); `thread_local!` (all `rule-6-static`) |
| 8 | the standard library's macros (`println!`, `vec!`, `assert!`, `format!`, `write!`, `matches!`, `include_str!`, ... a list of 33); a `macro_rules!` whose body writes no item and uses no macro of the program; macros and derives of a crate that the idea declares, by path or by `use`; the derives `Clone`, `Copy`, `Debug`, `Default`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash`; the attributes `derive`, `cfg`, `must_use`, `inline`, `cold`, `repr`, `non_exhaustive`, `track_caller`, `forbid`, `deny`; `cfg` and `cfg!` with only `all`, `any`, `not`, `unix`, `windows` and the `target_*` names | a `macro_rules!` that writes `fn`, `struct`, `enum`, `union`, `impl`, `trait`, `mod`, `static`, `const`, `type`, `use`, `extern` or `macro_rules`, or uses a macro of the program (`rule-8-macro-rules`); any other macro, `include!` among them (`rule-8-macro`); any other derive (`rule-8-derive`); any other attribute, `cfg_attr`, `test` and `path` among them (`rule-8-attribute`); a `cfg` of anything but the target, `test`, `debug_assertions` and `feature` among them (`rule-8-cfg`); a `build.rs` (`rule-8-build-script`) |
| 10 | — | `#[allow]`, `#[warn]` and `#[expect]`, outer or inner, since they turn the `[lints]` off (`rule-10-allow`); clippy holds the rest |
| 11 | — | every comment, `//`, `/* */`, `///`, `//!`, `/** */` and `/*! */`; a `#[doc]` attribute; a shebang line (`rule-11-comment`) |
| 12 | every function with a body named in its module's `# Structure`, a free function as `name` and a method as `Type::name` (the type of the `impl`, or the trait for a trait's own default method), and nothing else there | a function that is not named (`rule-12-unnamed`, on the Rust file); a name with no function (`rule-12-unknown`, on the idea); a `fn`, `impl`, `trait`, `mod` or `macro_rules!` inside a function (`rule-12-nested`); a `mod` with a body (`rule-12-module`); an `impl` for a type that has no `Type::` name: a primitive, a reference to one, a tuple, a slice, or a type parameter (`rule-12-impl`); a Rust file under `src/` that is no idea's file (`rule-12-source-unlisted`); an idea with no Rust file (`rule-12-source-missing`) |

Two methods of one type with the same name (`fmt` of `Display` and of `Debug`) are one name in
`# Structure`. A trait's method with no body is not a function; a trait's default method is
`Trait::name`.

**The reader** reads Rust on the standard library alone (decided at L4: `syn` does not merit
it; it would be the lint's only dependency and a parser of all of Rust, where the rules need
items, function bodies and a few shapes). It has a tokenizer of Rust's lexical grammar
(identifiers and raw identifiers, lifetimes and character literals told apart, strings, byte,
C and raw strings with any number of `#`, nested block comments, numbers with their suffixes,
`1..2` and `1.max(2)` read as Rust reads them, a shebang, a byte-order mark), pairs the
delimiters, and reads the items: `fn` (free, in `impl` blocks and in traits; with `const`,
`async`, `unsafe`, `extern "abi"` and `pub(...)`), `struct`, `enum` and `union` with their
fields, `trait`, `impl` (its generics, its trait and its type), `mod name;`, `use`, `static`,
`const`, `type`, `macro_rules!`, macro invocations, and attributes. Whatever else it meets is
the finding `rust-read`, with the line, and the rules are not guessed there: an `extern` block,
`extern crate`, a negative `impl`, an item that is not one of these, a string or comment with
no end, a delimiter that does not close.

**What the call graph resolves.** Every function with a body is a node. Its edges come from its
body, closures included, and from the bodies of the program's macros it uses:
- a call `name(...)` or `name::<T>(...)` is an edge to every free function of that name, in any
  module; `module::name(...)`, `crate::...` and `super::...` too, and `std::`, `core::` and
  `alloc::` paths are not the program's;
- a function's name used as a value (`map(step)`) is an edge, unless the body binds that name
  (`let`, a parameter, a closure's parameter, `for`, a `match` arm, `if let`);
- `Self::m`, `Type::m`, `Trait::m` and a type alias's `Alias::m` are edges to that method, and to
  the trait's default `m` for each trait the program implements for the type; a trait's `m` is
  an edge to the `m` of every type that implements it; a type parameter's `T::m` is an edge to
  every method `m`;
- a method call `x.m(...)` is resolved by the receiver: on `self`, to the type of the `impl`
  (or every implementor, in a trait); on a field of `self`, to the types of that field; on a
  parameter, to the types of the parameter; any other receiver, a local variable, a call's
  result, a literal, `impl Trait` or a type parameter, to every method `m` of the program.

What it does not resolve: calls that Rust makes on its own, through operators (`+` calls `add`),
format macros (`{}` calls `fmt`), `Drop`, `Deref`, `for` (`next`), `?` (`from`), and the
standard library's generic code that calls a trait method of the program (`sort` calls `cmp`);
the code a declared crate's macro writes and the calls a declared crate makes back into the
program's trait impls; and the drop glue and derived code of a recursive type. A recursion
through these is not refused at check; at run, the stack overflows and the program traps, or
the time limit stops it.

**The component checks**, when the build ends: the size, at most 64 MiB (`component-size`),
since the one file holds a GPUI front end with its fonts and images (section 4), and Cranelift's
time to compile a larger file at the first run passes the 40 seconds of a tool call; the
imports, only from the world (`import-ungranted`, since L3); and Wasmtime compiling the
component with lengua's engine, whose `wasmtime::Config` refuses what can give a different
result on a different machine (`component-refused`): `wasm_relaxed_simd(false)`,
`wasm_shared_everything_threads(false)`, and no threads or shared memory at all, since Wasmtime
is built without its `threads` feature; and `cranelift_nan_canonicalization(true)`, so that a
NaN has the same bits on every CPU. Plain SIMD stays: its results are the same everywhere once
NaNs are canonical.

## 6. Trust labels

| Label | Means |
|---|---|
| `unchecked` | Not yet through every check: the job runs, a test failed, the build failed, or there is no code |
| `checked` | Built by lengua, every rule holds, every test passes, and the mutation score is at or above the threshold, 80% |
| `tested` | Built by lengua, every rule holds and every test passes, but the tests are weak: the mutation score is under 80% |
| `trusted` | A component lengua did not build; it runs under the grants, and its tests run, but nothing about its code is checked (L7) |

`check`'s reply and the wiki show the label of every program. A program that uses a `trusted`
component says so.

Decided at L5 (2026-11-13):

- **The threshold is 80%**: `checked` when caught × 100 ≥ 80 × counted. Some mutants can't be
  caught by any test (a change the program's output never shows, such as the start of a loop
  that runs past its time limit), so 100% would ask for the impossible; 80% is the bar that
  mutation tools commonly set as "high" (Stryker's default `high` is 80), and it still asks for
  a test of every boundary and every branch in a small program. Under it, the surviving
  mutants are notes, so the agent knows which tests to add.
- **The label is the program's**: every idea of the program, `index.md`'s lines and the log
  entry carry the same label, since the tests and the score are the program's as a whole.
- **No mutant counted is `checked`**: a program whose code has no place to mutate (hello world
  is one `println!`), or whose mutants all fail to build or build to the same component, has
  nothing the tests could miss; its score is none, and the log says `no mutant counted`.
- **When the label is written:** `check` writes the wiki before it starts or asks about the
  job, so the label in the wiki is the one of the job that was `done` at that call; the next
  `check` after the job ends writes `checked` or `tested`. A failed test, a failed build and a
  running job all give `unchecked`.

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

### How the tests run

Decided at L5 (2026-11-13), in `crates/lengua-mcp/src/testing.rs` and `mutation.rs`, with the
drawing of cases in `crates/lengua-idea/src/cases.rs` and the mutator in
`crates/lengua-lint/src/mutate.rs`.

**Which tests.** Every `### Example:` and `### Cases:` block of every idea, the top one and
each module's, in the order of the ideas and then of the file, runs against the program's one
component: a module's examples are examples of the program. A screen's examples
(`ideas/ui/`) run at L8 with GPUI's test context; until then each is a note, "it was not run".

**One run.** For each run, lengua makes a new empty folder under the system's temporary folder,
makes in it every folder the idea grants (read and write), writes each ``File `<path>` `` of
the example, and runs the component through the host with that folder as the program's
folder, the example's standard input and arguments, and `fixed` on (section 4, "Fixed runs").
The run has the idea's memory and time, and since L6 it is a process of its own (`lengua-mcp
worker`, from the component's sealed code and test lock), stopped at the time and two seconds, as
`run` does.
An example that gives arguments or a standard input that the idea
does not grant fails, with the grant named. Then lengua compares, and every difference is in
the one finding:
- how the run ended (`Expect end`, `exited` when absent; a trap's or the limit's message
  follows the difference);
- the exit code, when the run exited;
- standard output, always (an example without `Expect standard output` expects none);
- standard error, when the example gives it;
- each ``Expect file `<path>` `` in the run's folder after the run.

**The finding.** A failed example is `test-failed`, a failed drawn case `case-failed`, at the
idea file and the line of its `### Example:` or `### Cases:` heading. The message names the
example and its behaviour, a case's drawn values (``the case with `n` = 9 of `numbers of one
digit` ``), and for each difference the expected text and the text that came, from 20
characters before the first character that differs, at most 60 characters each, as Rust
writes a string: ``standard output: expected "small\n" and got "large\n" (they differ at
character 1)``. Every run runs, so one reply has every failure.

**Drawn cases.** For each `### Cases:` block lengua draws, with SplitMix64 from a seed that is
the SHA-256 of the behaviour's name, the block's name and its ranges (so a block always draws
the same cases, wherever it is in the file, on every system):
1. the boundary values first: each range's low end, the value above it, the value below the
   high end, and the high end (for a text range, those are its lengths; the characters are
   drawn), the first case taking each range's first boundary, the second each range's
   second, and so on;
2. then 20 cases from the seed, each value anywhere in its range;
3. a case drawn before (the same values) is not run again, so a block has at most 24 cases (a
   range of ten integers has at most ten).

Each `{name}` of the block's strings becomes the drawn value, `{{` and `}}` a brace.

**Mutants.** When every test passes, the job measures the tests with mutants of the program's
own code: lengua's mutator reads each Rust file with `lengua-lint`'s tokenizer and reader, and
in each function's body (`main` among them) finds the places to change:

| Mutation | From | To |
|---|---|---|
| an arithmetic operator | `+`, `-`, `*`, `/`, `%`, `+=`, `-=`, `*=`, `/=` | `-`, `+`, `/`, `*`, `*`, `-=`, `+=`, `/=`, `*=` |
| a comparison | `<`, `<=`, `>`, `>=`, `==`, `!=` | `<=`, `<`, `>=`, `>`, `!=`, `==` |
| a logical operator | `&&`, `\|\|` | `\|\|`, `&&` |
| a constant | a decimal integer `n` (with its suffix) | `n + 1` |
| a boolean | `true`, `false` | `false`, `true` |
| a condition | `if <condition> {` (not `if let`) | `if !(<condition>) {` |
| a return value | the body of a function that returns `bool`, `char`, an integer, a float, `String`, `&str`, `Option<...>` or `Vec<...>` | `{ let _ = { <the body> }; Default::default() }` |

An operator is one with a space on both sides, as `rustfmt` writes a binary operator (so a
generic's `<`, a unary `-` and a path's `::` are not), between an operand and an operand. A
tuple field (`t.0`) and a hexadecimal, octal or binary number are not constants to change. The
arguments of `assert!`, `assert_eq!`, `assert_ne!` and their `debug_` forms are not mutated:
a weaker assertion is a mutant no example can catch (and `debug_assert!` is not in a release
build at all).

**At most 40 mutants**, spread over the functions: the first place of each function, then the
second of each, and so on, until 40. Each mutant is built in its own package,
`target/lengua/mutant/` (a copy of `Cargo.toml` with the package named `<name>-mutant`, and of
`src/`, with the one change), with `cargo build --release --target wasm32-wasip2 --offline
--target-dir <the program's target>`, so it reuses the program's build folder and its built
crates; then the tests run on it, in order, until one fails:

| The mutant | Counts | Is |
|---|---|---|
| a test failed on it | yes | `caught` |
| every test passed | yes | `survived`; a note at its file and line: ``a mutant survived: in `size`, `<=` became `<`, and every test still passed; add a test whose result tells the two apart`` |
| it did not build | no | `unbuilt` |
| it built to the same bytes as the program | no | `same`: the compiler made the change disappear, so no test could catch it |

The score is caught / counted. 40 keeps a check of a small program to about a minute on the
runners (each mutant is one build of about a second and its tests), and gives the score a
step of 2.5% at most; a larger program has its 40 spread over all its functions.

**The cache.** `target/lengua/mutants` keeps the result of each mutant by a key: the SHA-256 of
lengua's version, the tests (without their lines, so moving an example does not count) and the
grants, `Cargo.toml`, every Rust file with the bodies of its functions left out, and then the
mutant's function (its whole text), its place in the function and its change. So when only
the bodies of some functions change, a later check builds the mutants of those functions only
and takes the others from the cache (the reply's `cached`); a change outside the bodies (a
type, a `use`, a constant), of the tests, of the grants or of lengua measures them all again.
The file keeps only the mutants of the last check.

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
| L8 | The GPUI front end: screens built for the browser, embedded in the one file; several screens with navigation and forms; their behaviours tested; what a screen reader can read, tested and written down; one Zed commit pinned; lengua's own std-only crates also built for WebAssembly in CI | A web application with GPUI screens ships as one file and opens in a browser |
| L9 | Full stack: programs that ask AI models (`- Asks: an AI model`, with call and token limits; the host's interface to a cloud or local model, structured and streamed replies; model answers given in the tests; the `ai` shared library); storage granted by the idea; outgoing HTTP calls (other services and AI models, with streaming replies); secrets given by the host and never written to the lock or logs; logins (sessions, password hashing, tokens) as a shared library; the front end calling its own server with shared Rust types; logs and errors of a running app; an embeddings and vector-search shared library; a tested way to deploy | A full-stack application (front end, server, storage, logins, an AI call) is `checked` and runs as one file |
| L10 | Native parts: `- Native:` with a named reason, built for the user's system and on five runners, the same checks, the `native` label, the "Where it runs" line; an example desktop app with its own GPUI window whose logic stays WebAssembly; what a native part can reach, written down | A native part is built, checked and labelled; an undeclared native need is refused |
| L11 | The skills: a new `vision` skill (the best lengua solution for an idea, `vision.md`, the principles with their do's and don'ts in `references/principles.md`, approved by the person) and `write-idea` and `build` rewritten for Rust, GPUI, full stack and native parts; the person reads the idea and approves it in their own words before any code, and again for changes (the approved hash in the lock, `build` refused without it); "Where it runs" before any code, pure Rust first, the merit test for crates, crates that aren't pure Rust flagged; the Agent Plugins version settled; a product test in an interactive session | An agent goes from a request to a `checked` full-stack application with the plugin alone |
| L12 | The scale test: a full-stack application (GPUI front end, server, storage, logins, an AI call) in one file, built and then changed three times | A report of cycles and breakages, next to lengua 2's 3,638-line record |
| L13 | The finish line: trace and profile; a bench of lengua's overhead; release archives for the five systems with install steps; every path a milestone listed as never exercised, now exercised or its reason written; WASI 0.3 checked again; every document current; a short guide to the owner's own steps (publishing, loading the plugin, branch protection) | The finish-line run green on five; only main and tags remain |

The watch list (section 10) is not a milestone: each item waits on a standard.

### The LLM wiki and OKF, by milestone

A review on 2026-11-13 against
[OKF v0.2](https://github.com/GoogleCloudPlatform/open-knowledge-format/blob/main/SPEC.md) and
[Karpathy's LLM wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) found the
structure sound (concept ids, `index.md`, `log.md`) and these gaps. Each is part of the
milestone named:

- **L9, in `check`:** all done ahead of L9, on 2026-11-13, on `wip/wiki` (the five-runner
  landing run is named in the landing report); "The idea file's grammar" and "The wiki's formats"
  say how each works.
  - Done: OKF's trust fields on every page: `generated` (by `lengua-mcp/<version>`, at),
    `verified` by `process:lengua-check` while the label is `checked` or `tested`, and `status`
    (`draft` until approved); agent-written values of these checked, not only kept
    (`okf-status`, `okf-time`, `okf-actor`, `okf-trust`, `okf-sources`).
  - Done: `title` written when absent, and used by `index.md`.
  - Done: a `# Links` section that `check` writes at the end of each idea, with real markdown
    links (uses, used by, libraries, components, vision when it exists, the index), kept out of
    the idea's hash like the frontmatter.
  - Done: libraries in `uses`; `used_by` of a library written only in the root index of
    `libraries/`, not in the library's own files (its lock would change with every new user);
    a concept page for each component (`components/<name>.md`) in place of a link to a `.wasm`
    file.
  - Done: the crate count in `index.md`.
  - Done: a conformance pass: every `.md` in the folder other than `index.md` and `log.md` has
    frontmatter with a `type` (`okf-type`).
  - Done: no log entry, and no file written, when a check changes nothing; a check whose job
    has not ended keeps the label of the same files.
  - Done: the type list in this document matches the code (`lengua Server`, `lengua Library`,
    `lengua Component` added).
  - Done: a finding when an idea's Purpose names a module or library its Constraints don't list
    (`purpose-unlisted`, names in backticks only).
  - Secrets never written to the frontmatter, the index or the log: the rule is written ("The
    wiki's formats", "Secrets"); there is no grant of a secret yet, so its test comes with L9's
    secrets grant.
- **L10:** the "Where it runs" line first in `index.md`.
- **L11, the query path and the person's part:**
  - `vision.md` as an OKF page (`type: lengua Vision`), first in `index.md`, quoting the
    person's request, linked from every idea's `sources`, and in the lock's file list.
  - Every skill's first step: read `index.md`, then the pages it names, before any code; a
    design answer goes back into `vision.md`.
  - On approval, `verified` gains `human:<id>` and `status` becomes `stable`; `log.md` gets an
    `**Approval**` entry with the person's words, plus entries for the vision and for lint.
  - `check` holds the ideas to `vision.md` and says when they grow past it.
  - The idea grammar ships in the skill's references, so an agent needs nothing outside the
    plugin.
- **L13, one wiki across the repository:** done ahead of L13, on 2026-11-13 (the five-runner
  landing run is named in the landing report), as "One wiki across the repository" says.
  - Done: the generated frontmatter, `# Links`, component pages, `index.md` and `log.md` of
    `libraries/*` and `examples/*` committed from the CI mode `wiki` (run 43522428412), not the
    per-user lock, so the shared libraries are a browsable wiki.
  - Done: root `index.md` files for the repository, `libraries/` and `examples/`, written by
    `lengua-mcp wiki <root>`.
  - Done: a CI test that checks the whole tree against OKF (section 11), its links, and that
    the committed wiki is current.
  - A decision on OKF's Attested Computation, which fits a locked program: a recommendation is
    in section 10; the decision stays with L13.

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

Decided for lengua 3 at L0 (2026-11-13):

- **lengua's own code follows rule 11.** Its Rust files have no comments and no doc comments,
  as lengua asks of the programs it builds. Its documentation is this document and each
  crate's `README.md`, written like an idea (purpose, behaviours, structure). A language that
  says comments drift should not ship comments of its own.
- **lengua's own lints** are in the workspace's `[lints]`: every warning denied, clippy's
  `pedantic` group denied, `unsafe_code` forbidden. The other rules of section 5 come with
  lengua's lint at L4.
- **Agent Plugins 1.0.0** is the plugin format: agent-plugins.org publishes 1.0.0 as its
  current version, with its schemas under `schemas/1.0.0/` (there is no 1.1.0 there).

Decided for lengua 3 at L1 (2026-11-13):

- **lengua's own code has two crates so far.** `crates/lengua-idea` is the idea reader, the
  STE check and the wiki writer, on the standard library alone (with its own SHA-256).
  `crates/lengua-mcp` is the server; its only dependencies are `rmcp` (the MCP Rust SDK,
  features `server`, `macros` and `transport-io`) and `tokio` (`macros` and `rt`, which
  `rmcp` already brings), and it reaches `serde` and `schemars` through `rmcp`'s re-exports.
  Each dependency and its reason is in `todo.md`.

Decided for lengua 3 at L2 (2026-11-13):

- **The generators are in `lengua-idea`** (`generate.rs`, the standard library only), since
  they read the ideas; **the build job and `doctor` are in `lengua-mcp`** (`build.rs`,
  `doctor.rs`), on a standard-library thread and child process. No new crates.
- **No `wit-bindgen` for a standard-library program**; the world is a statement, held at L3.
- **Rule 12 (the structure held to the Rust functions) comes at L4** with lengua's lint, which
  reads the Rust source; L2 does not read the Rust.
- **`examples/` holds example programs** as the agent writes them: `idea.md` and `src/` only.
  The tests copy them to Cargo's test folder and check the copy, so the generated files, the
  wiki and `target/` are never in the repository.

Decided for lengua 3 at L3 (2026-11-13):

- **Wasmtime 50.0.1 is the host**, pinned exactly (`=50.0.1`; 50.0.1 is still the latest 50.x
  on its releases page, with 51.0.0 only at rc.1). Default features off: `cranelift`,
  `runtime`, `std` and `component-model` for `wasmtime`, `p2` for `wasmtime-wasi`, and
  `wasmtime-wasi-io` for `wasi:io/error`, whose `add_to_linker` `wasmtime-wasi` does not
  export. No cache, no Winch, no parallel compilation, no GC. It merits being a dependency:
  it is the host. The crates it brings are counted in `todo.md`.
- **The host is in `lengua-mcp`** (`host.rs`, `runs.rs`), not a crate of its own yet; at L6
  the lean runner `lengua-run` takes it with the sealed code.
- **A grant controls what a program can reach, not what `std` imports**: the always set
  (section 4, "Grants become the component's world").
- **`run` has a fourth input, `fixed`**, so that the determinism the tests of L5 need is
  there and tested now.
- **WASI 0.3 looked at again**: `wasmtime-wasi` 50.0.1 still has `p3` as a feature of its own
  with the 0.2 module as its stable side, so lengua stays on 0.2.

Decided for lengua 3 at L4 (2026-11-13):

- **lengua reads Rust itself**: `crates/lengua-lint`, on the standard library alone, used by
  `lengua-idea`; no `syn` (section 5, "What each rule accepts and refuses").
- **The lint runs with the idea checks**, when `src/main.rs` is there; the build job runs
  `cargo fmt --check`, then clippy, then `cargo build`; the component checks follow it.
- **Rule 5 does not count `main`**; rule 10 refuses `#[allow]`, `#[warn]` and `#[expect]`;
  the call graph errs towards a false recursion, never a missed one, for the calls it reads,
  and does not read the calls Rust makes on its own (section 5).
- **The size limit is 64 MiB**, and lengua's engine refuses relaxed SIMD and threads and
  canonicalizes NaNs.

Decided for lengua 3 at L5 (2026-11-13):

- **The tests run in the build job**, after the component checks, then the mutants; the
  states are `building`, `testing`, `mutating` and `done` (or `build_failed`,
  `tests_failed`), each with its progress, and `check` never waits for them (section 4, "The
  reply").
- **`Expect end`** (`exited`, `out of time`, `out of memory`, `trapped`; `exited` when absent)
  is in the test format, and `Expect exit code` goes only with `exited` (L3's finding).
  `forever` and `greedy` use it.
- **An example with no `Expect standard output` expects an empty standard output**, as the
  grammar's "compared exactly" says; only standard error is not compared when absent.
- **`insecure-seed` is in the always set**, with a fixed value unless the idea grants random
  numbers (L4's finding), so a `HashMap` needs no grant; `wasi:random/random` and `insecure`
  still need `- Reads: random numbers`.
- **lengua's own mutator, at most 40 mutants, the threshold 80%, a cache by function** (section
  7, "How the tests run"; section 6). No new crate: the mutator is in `lengua-lint`, the runs
  and the mutants in `lengua-mcp`.
- **The label is the program's** and is written by the next `check` after the job ends; a
  program with no mutant counted is `checked` (section 6).

Decided for lengua 3 at L6 (2026-11-13):

- **The lock** is text, signed with HMAC-SHA256 under the user's key, written by `check` only
  when its job is `done`, right after the wiki in the same call (section 4, "The lock as
  built", with its exact format).
- **The user's key** is 32 bytes from `getrandom` (in the graph through Wasmtime), in lengua's
  data folder (`$LENGUA_HOME`, else the system's own: XDG on Linux, Application Support on
  macOS, `%APPDATA%` on Windows), owner-only where the system allows.
- **One crate may use `unsafe`: `crates/lengua-seal`**, with one `unsafe` call,
  `Component::deserialize`, run only on bytes that `lock::verify` checked. Wasmtime has no safe
  way to load precompiled code without its compiler.
- **The host is the crate `lengua-host`**, with no compiler; `lengua-mcp` compiles and seals,
  `lengua-run` only loads.
- **Every run is a process of its own**, killed at the idea's time and two seconds: `run`'s and
  every test's (`lengua-mcp worker`), and `lengua-run` itself. The tests and mutants run from
  sealed code with test locks, so the one `unsafe` call always follows a verified signature.
- **`lengua-run`** is a new binary crate; its exit codes are the program's, or 124, 125, 126 and
  134 for lengua-run's own endings.

Decided for lengua 3 at L7 (2026-11-13):

- **A module's examples run through the program's entry**; a library's through a generated
  harness of calls (section 4, "As built at L7"). Each module has its `mod` line.
- **An idea has a shape**: a program, a server (`- Serves:`) or a library (`- Exports:`); a
  library reaches nothing of its own, and its lock is of kind `library`.
- **A program names a library by its folder** in `- Uses:`; its lock is verified at `check` and
  at `run`, and recorded in the program's lock.
- **Network grants are exact addresses and ports**, held by `wasmtime-wasi`'s socket address
  check; UDP is linked and off. Tests give a `Server` or a `Client`.
- **The HTTP world**: `wasmtime-wasi-http` (synchronous bindings, outgoing requests refused) in
  the host; `wasip2` 2.0.1 in the program; requests in memory for `run` and the tests, a TCP port
  for `lengua-run`.
- **A component from another language** is compiled, held to the world, sealed and linked by
  lengua's host; labelled `trusted`. The first one is in C, built by wasi-sdk 34 in CI.
- **lengua's shared libraries** are seven lengua libraries in `libraries/`, each `checked`.
- **New dependencies of lengua's own**: `wasmtime-wasi-http` 50.0.1 (`p2`) and `hyper` 1 (no
  features, for its re-exported types; already in the graph), both in `lengua-host`.

Decided for lengua 3 at L8 (2026-11-13), the parts that do not depend on the front end:

- **No path remapping.** `--remap-path-prefix` for the Cargo home, the folder and the path
  dependencies was tried and taken back: the shop still differed between systems with it (run
  43524778056), since its differences come from Cargo's crate hashes (section 10), and `hello`
  holds no paths. Cargo's network retries are 10 (`CARGO_NET_RETRY`) in every cargo call of the
  job.
- **`strip = true` in the generated release profile**: the component has no `name` section.
  Its mangled names carry Cargo's crate hashes, which differ between computers, so the same
  program built on Linux x86-64 and on Linux ARM64 differed there and nowhere else (runs
  43524778056 and 43528549629). With it, `hello` has 54,690 bytes and one SHA-256 on all five
  runners (run 43536298022).
- **CI holds `hello`, `tally` and `greet` to one SHA-256 on every runner** (the job `same-bytes`,
  from the components each check job uploads, with the difference printed by `wasm-tools`
  1.263.0 when there is one); **the shop is compared and reported, not held**, since a program
  that uses a lengua library cannot have one SHA-256 on every system with Rust 1.100.0 (section
  10, "Found at L8: why the shop is not the same on every system").
- **CI builds `lengua-idea` and `lengua-lint` for `wasm32-wasip2`** on every runner, and a
  failure fails the run (section 1).

## 10. Risks and open questions

- **WASI 0.2, decided at L0 (2026-11-13): lengua 3 targets WASI 0.2, `wasm32-wasip2`.**
  Checked against the official sources on 2026-11-13:
  - WASI 0.3.0 shipped on 2026-06-11 and 0.3.1 on 2026-08-11 (not February, as the third-party
    report said); 0.2 went on to 0.2.12 on the same train, and runtimes may keep supporting
    0.2 next to 0.3 ([wasi.dev roadmap](https://wasi.dev/roadmap)).
  - Rust 1.100.0 (2026-11-12) is the current stable
    ([announcement](https://blog.rust-lang.org/2026/11/12/Rust-1.100.0/)). `wasm32-wasip2`,
    `wasm32-wasip3` and `wasm32-unknown-unknown` are all tier 2 with full `std`
    ([platform support](https://doc.rust-lang.org/stable/rustc/platform-support.html)).
    `wasm32-wasip3` first reached stable in 1.100.0, its threads still fail to spawn, it is
    not tested in Rust's CI, and its page names LLVM 23 and wasi-sdk 34 as build requirements
    ([wasm32-wasip3](https://doc.rust-lang.org/nightly/rustc/platform-support/wasm32-wasip3.html));
    `wasm32-wasip2` has been tier 2 since 1.82 and runs on Wasmtime 17 and later
    ([wasm32-wasip2](https://doc.rust-lang.org/nightly/rustc/platform-support/wasm32-wasip2.html),
    [tier 2 post](https://blog.rust-lang.org/2024/11/26/wasip2-tier-2/)).
  - Wasmtime's current release is 50.0.1 (2026-10-29)
    ([releases](https://github.com/bytecodealliance/wasmtime/releases/tag/v50.0.1)). The
    component model and its async are tier 1
    ([stability tiers](https://docs.wasmtime.dev/stability-tiers.html)), and wasi.dev says
    WASI 0.3 is on by default since Wasmtime 46, but `wasmtime-wasi`'s API documents still
    call its WASI 0.3 module "experimental, unstable and incomplete", while its WASI 0.2 module
    carries no such label ([wasmtime_wasi](https://docs.wasmtime.dev/api/wasmtime_wasi/)).
  - Tokio builds for `wasm32-wasip2`, networking included, since 1.51.0 (2026-04-03), but only
    with `RUSTFLAGS="--cfg tokio_unstable"`
    ([tokio#7933](https://github.com/tokio-rs/tokio/pull/7933)). That changes nothing here:
    Tokio is not an exception (section 4).

  Why 0.2: the user needs only `rustup target add wasm32-wasip2`; WASI 0.2 is the stable side
  of Wasmtime's WASI; lengua's components have one thread, so 0.3's async adds little yet. The
  pins: Rust 1.100.0 in `rust-toolchain.toml` with both WebAssembly targets, and Wasmtime
  50.0.1, added as a dependency at L3. Look at 0.3 again at L3 and at L7 (HTTP): take it when
  `wasmtime-wasi` calls it stable and its Rust target builds with `rustup` alone.
- **The standard library imports more than hello world's grants (found at L2, for L3).** The
  `hello.wasm` that Rust 1.100.0 builds for `wasm32-wasip2` (66,121 bytes, CI run 43379311761)
  imports, all at `@0.2.12`: `wasi:cli/environment`, `exit`, `stderr`, `stdin`, `stdout`,
  `terminal-input`, `terminal-output`, `terminal-stderr`, `terminal-stdin`,
  `terminal-stdout`; `wasi:clocks/monotonic-clock`; `wasi:io/error`, `poll`, `streams`; and it
  exports `wasi:cli/run`. So `std` imports `environment`, `stdin` (with its terminal
  interfaces) and `monotonic-clock` even when the program does not use them. **Decided at
  L3:** both halves: the world's always set grows to them, and the host holds the grant's
  absence by linking them with nothing behind them (section 4). Files, random numbers, the
  wall clock and sockets were not imported by hello world, so for those the world's rule holds
  as written.
- **Found at L3: a host call that blocks is not stopped by the time limit.** The epoch stops
  WebAssembly code, not Wasmtime's host code: a program that sleeps (`wasi:clocks` subscribe and
  `wasi:io/poll`) sleeps for real, even with the stepped clock. `run` stops waiting two
  seconds past the time and replies `out_of_time`, but the run's thread goes on until the
  sleep ends. To close at L6 with the lean runner (a pollable of lengua's own, or the
  runner's process stopped). **Closed at L6:** every run is a process of its own (`lengua-mcp
  worker` for `run` and the tests, `lengua-run` from a shell), stopped at the time and two
  seconds with `Child::kill` or by its own end, on all five systems; the example `sleepy` (a
  sleep of 30 s, `- Time: 1 s`) is stopped at about 3 s, in its test, by `run` and by
  `lengua-run` (section 4, "The lock as built").
- **Found at L3: the test format has no key for how a run ends.** An example must give `Expect
  exit code`, but a program stopped at its time or memory limit has none. The examples
  `forever` and `greedy` write `Expect exit code: 1` for now; L5 needs a key such as `Expect
  end: out of time`. **Closed at L5:** `Expect end` is in the grammar, and `forever` and `greedy`
  use it.
- **Open at L3: what more `std` imports.** The CI step "the imports of the built examples"
  lists the `wasi:` names in each example's bytes (run 43387181531). That is not the list of
  imports: `files.wasm` has the name `wasi:random/insecure-seed`, and `clock.wasm` the names of
  `wasi:filesystem`, yet both passed the import check, which reads the real imports through
  Wasmtime. So the names are also in the component's type information, and the check is the
  evidence. If a common part of `std` (`HashMap`'s random keys, for one) does import
  `wasi:random`, a program that uses it needs `- Reads: random numbers`; to decide then whether
  such an import joins the always set with a fixed seed behind it. To make the evidence exact,
  L4's component checks can print the imports that Wasmtime reads.
- **Found at L4: what `std` imports, as Wasmtime reads it** (`lengua-mcp imports`, CI run
  43396401311, Rust 1.100.0). `hello`, `echo` and `count` import the always set only. `files`
  adds `wasi:clocks/wall-clock`, `wasi:filesystem/preopens` and `types`, and no `wasi:random`.
  `clock` adds `wasi:clocks/wall-clock` and `wasi:random/insecure-seed`, and not
  `wasi:random/random` or `insecure`: `std`'s `RandomState`, the hasher of every `HashMap` and
  `HashSet`, takes its keys from `insecure-seed`. So a program with a `HashMap` needs `- Reads:
  random numbers` today. Still open (for L5, whose tests need it): to put `insecure-seed` in the
  always set with a fixed seed behind it unless random numbers are granted, as the clock is.
  **Decided at L5:** it is in the always set, as that says (section 4); the example `words`
  uses a `HashMap` with no grant.
- **Build time:** a cold `cargo build` can take longer than one tool call; the background job
  and a kept build folder per program must hide it.
- **The user must have Rust installed**, with its two WebAssembly targets. lengua 2 needed nothing
  but lengua itself. Accepted by the owner on 2026-11-13: "I acknowledge that having Rust
  installed is a dependency now." `lengua-mcp doctor` checks it and says how to install it.
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

- **The watch list: what would lift each native limit.** Each waits on a standard, not on
  lengua; when one matures, the native kind it covers moves to WebAssembly.
  - Desktop windows and the GPU outside the browser: **wasi-gfx** (WebGPU for WASI). Then
    `lengua-run` could open a real window for the same one file.
  - Running AI models locally: **wasi-nn**.
  - Threads that share memory: WebAssembly's **shared-everything-threads** proposal.
  - WebSockets: not in WASI 0.2's HTTP world; watch WASI 0.3 and later.
  - Timed jobs ("every night at 2 a.m."): no WASI standard yet.
- **Decided 2026-11-13, the front end is GPUI and GPUI Kit,** after comparing Leptos and Native
  Shell: the owner chose to keep GPUI. Known risks: GPUI's web target is a basic pre-1.0 port,
  and GPUI Kit's own site says it is made for the desktop; L8 tests it first and pins one
  commit.
- **Crates that aren't pure Rust** (they wrap a C library) need a C compiler, build differently
  on each system and usually can't go into WebAssembly, so the `build` skill takes them last,
  and `check` flags them.
- **Looked at again at L7: WASI 0.3 for HTTP.** `wasmtime-wasi-http` 50.0.1 has `p3` as a
  feature of its own and its documents call the 0.3 module "experimental, incomplete", while its
  `p2` module, with synchronous bindings, carries no such label; `wasip2` 2.0.1 binds WASI
  0.2.12. So servers stay on WASI 0.2 (`wasi:http/proxy` 0.2.12). Look again at L13.
- **Found at L7: what `std`'s sockets import** (run 43478821353, `lengua-mcp imports`): `fetch`
  and `upper` import `wasi:sockets/tcp`, `tcp-create-socket`, `network`, `instance-network`,
  `ip-name-lookup`, `udp` and `udp-create-socket`, and `wasi:clocks/wall-clock`, though they use
  TCP only. So a network grant's world names them all, and the host keeps UDP off.
- **Found at L7: the bindings' macros and `unsafe_code = "forbid"`.** `wasip2::http::proxy::export!`
  (a `macro_rules!` of the crate) and `wit_bindgen::generate!` (a procedural macro) write
  `unsafe` code and `#[unsafe(export_name)]` into the program, and the program's own lints still
  forbid `unsafe_code`: rustc does not report a lint in code from another crate's macro. The
  program's own source has no `unsafe` (lengua's lint and rule 9 hold that); the macros' code is
  the declared crate's, as rule 9 allows. Measured on Linux x86-64 only so far; the landing run
  holds it on five.
- **Open at L7: the ports of the tests are fixed** (7801 to 7811 in the examples); a test server
  waits up to 10 s for its port, and `lengua-run`'s test of a server needs its port free. A
  program's own tests do not use the network unless the example gives a `Server` or a `Client`.
- **Open at L7: `generate!` and the folder `wit/`.** `wit-bindgen`'s `generate!` reads `wit/` by
  default, and lengua's `wit/world.wit` is a statement for the host, not WIT for a binding
  generator (it imports WASI with no `wit/deps/`). A program writes `path: []` with `inline`; the
  `build` skill (L9) says so.
- **Found at L8: why the shop is not the same on every system.** Run 43543315033 (`strip = true`,
  Rust 1.100.0): `hello` 54,690 bytes on all five runners with one SHA-256; the shop 81,082 bytes
  everywhere, in two versions, one on Linux x86-64, Linux ARM64 and Windows ARM64 and one on macOS
  ARM64 and Windows x86-64, 183 bytes apart. `wasm-tools print` shows the cause: one function
  (`(type 13)`, no parameters) stands at index 172 in one and 167 in the other, so the function
  numbers between, the table of `call_indirect` (`elem`) and two data addresses move. Four builds
  from clean on one runner gave one SHA-256 (run 43541237409), so it is not chance. The shop uses
  two lengua libraries, `json` and `decimal`, which are path dependencies outside its folder, and
  Cargo puts the absolute path of such a dependency into its crate hash (`-C metadata`, through
  the package id's source; its source, `compilation_files.rs`, leaves the path out only with the
  profile setting `trim-paths`). The crate hashes order the functions that rustc writes, and the
  repository's path differs between the systems (`/home/runner/work/...` on Linux,
  `/Users/runner/work/...` on macOS, a drive letter on Windows). Before `strip = true` the hashes
  were also in the names, so even Linux x86-64 and
  ARM64 differed (run 43528549629). What would fix it: Cargo's `trim-paths`, stable from Rust
  1.101 (Cargo's documentation), not 1.100.0; or the libraries inside the program's folder, which
  lengua's design does not do. Until then `hello`, `tally` and `greet` (no path dependency) are
  held to one SHA-256 and the shop is reported. L7's 3-byte difference on Windows ARM64 most likely
  had the same cause, with the names still in (L7 compared sizes only).

- **History: GPUI's web target, tried at L8 and dropped on 2026-11-13.** The owner then chose
  Leptos for the front end and Tauri for the desktop; GPUI, GPUI Kit, Next.js and Electron are
  dropped. The work is kept at the tag `archive/l8-gpui`. What was found, in CI on Linux x86-64:
  - **The pin:** GPUI Kit (`gpui-component` 0.9.1, 2026-11-09) requires `gpui-pre` `=0.3.12`, the
    crates.io snapshot of Zed commit `d2a4fb6ded7c470be25d1541281da3882495b65b`, so one Zed commit
    could be pinned only through that exact version, not through a `git` dependency with
    `rev =`, which would put a second, incompatible `gpui` in the graph.
  - **What built:** GPUI and GPUI Kit built for `wasm32-unknown-unknown` on stable Rust 1.100.0
    with the web platform single-threaded (default features off); the default `multithreaded`
    needs a nightly toolchain and a cross-origin-isolated page. A cold release build (LTO, one
    codegen unit) took 3 min 18 s; the `.wasm` had 23,051,428 bytes, 20,977,185 after
    `wasm-bindgen` 0.2.130 (about 21 MB), and the loader 161,147 bytes; 466 crates in the
    browser's graph, 801 in the lock with the native tests' (runs 43490423487, 43491793393).
  - **The GPU:** headless Chrome 157 on the runner has no WebGPU adapter; GPUI fell back to WebGL2
    through SwiftShader and drew a GPUI Kit button and text field. WebGPU on a real GPU was not
    exercised.
  - **Fonts were needed:** with no fonts, GPUI panicked at its first text
    (`gpui-pre-0.3.12/src/text_system.rs:620`); its web text system has none of its own and falls
    back to IBM Plex Sans, which then had to be shipped in the file.
  - **Accessibility:** GPUI builds its AccessKit tree only when not compiled for wasm
    (`gpui/src/window.rs`, `cfg(not(target_family = "wasm"))`), and `gpui_web` has no
    accessibility code, so a screen reader could reach only GPUI's canvas and its hidden
    `textarea`, never its text. Read from the source; the DevTools check of the accessibility
    tree never ran.
  - **Tests:** natively, GPUI's `TestAppContext` ran a screen: a click at a debug selector,
    typing into a text field, and the text of the last frame through a recording text system
    (2 min 16 s to build, 0.03 s to run).
  - **Paths:** the browser's `.wasm` held the absolute paths of the Cargo home in its panic
    messages, so two computers build different files; remapping them was tried at L8 and taken
    back (section 9).
  - **How far the whole application got** (an example of three screens, runs 43496636597 to
    43508359575): the front end built, `wasm-bindgen` ran and the one file was made; the screens'
    tests, their mutants, the serving by `lengua-run` and the browser test never passed before
    the change of direction.

- **Recommended on 2026-11-13, for L13 to decide: OKF's Attested Computation (OKF section 10).**
  A checked lengua program is close to what OKF calls an Attested Computation: the idea is the
  contract, the behaviours say what the computation does, the grants are the only inputs it can
  reach, and the lock with its sealed code makes "did the sanctioned code run" a comparison of
  hashes, not a judgement. The recommendation:
  - Not to make the ideas themselves Attested Computations. An idea describes a program; it is
    an OKF concept of its own type, as it is now.
  - At L13, `check` writes one more page for a program that is `checked`, `attestation.md`,
    `type: Attested Computation`: `runtime: lengua` (a WASI 0.2 component run by `lengua-run`),
    `parameters` from the grants (`arguments` and `standard input` as strings, a server's
    request), `computation` naming the component file, `executor` naming `lengua-run` with a
    `receipt` of the component's SHA-256, the lock's SHA-256, the SHA-256 of the input, the
    exit code and the SHA-256 of the output, and an `attester` that is lengua's own code with no
    model in it: it checks the receipt's hashes against the lock and, as the run is
    deterministic under the fixed clock and seed, can run the sealed code again and compare.
  - Two things to wait for first. The lock is signed with an HMAC under the user's key, so it
    proves a check only to that user; a receipt for others needs a public-key signature
    (Ed25519, written on the standard library and tested with RFC 8032's vectors). And OKF v0.2
    leaves the receipt and verdict formats and the attester's interface to a later version
    (OKF section 12, "Considered and deferred"); lengua should follow them when they come
    rather than invent its own.
  - Until then, `verified` by `process:lengua-check` is the machine-confirmed tier OKF already
    defines (OKF section 5.3), and it is on every checked page.

## Sources

- GPUI: [gpui.rs](https://gpui.rs/), [docs.rs/gpui](https://docs.rs/gpui/latest/gpui/),
  [examples](https://gpui.rs/examples/); its web target: Zed's
  [`crates/gpui_web`](https://github.com/zed-industries/zed/tree/main/crates/gpui_web) and
  pull request [#50228, "GPUI on the web"](https://github.com/zed-industries/zed/pull/50228),
  merged on 2026-02-26.
- GPUI Kit: [gpui-kit.com](https://gpui-kit.com/).
- The standards and the Power of 10: the links in sections 4 and 5.
