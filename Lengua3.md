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
  (`lengua Program`, `lengua Module` or `lengua Screen`), `description` (the first sentence of
  `# Purpose`), `module` (`main` for the top idea), `source` (`src/main.rs`, `src/<name>.rs`,
  `src/ui/<name>.rs`), `uses` and `used_by` (OKF concept ids, the file without `.md`: `idea`,
  `ideas/cart`, `ideas/ui/basket`), `label` (`unchecked` until the tests run at L5),
  `idea_sha256` (the SHA-256 of the idea's text after the frontmatter, blank lines at its
  start left out) and `source_sha256` (of the Rust file, or `none`); on the top idea only,
  `cargo_sha256` and `world_sha256`, the SHA-256 of the `Cargo.toml` and `wit/world.wit` that
  check wrote (since L2, see "The generated files" below).
- **The agent may write any other key**, and lengua keeps it byte for byte: OKF's `title`,
  `tags`, `status`, `sources`, `generated`, `verified`, `stale_after`, `resource`, or a key of
  its own. lengua does not check their values.

**The sections** are level-1 headings, in this order, each one time: `# Purpose`,
`# Behaviours`, `# Constraints`, `# Structure`. Every idea has Purpose, Behaviours and
Structure; the top idea has Constraints too, and a module or screen idea may have them. Only
blank lines come before `# Purpose`. A heading inside a fenced block is not a section.

**`# Purpose`**: one paragraph or more of description in STE, with no headings and no fenced
blocks. A list item (`- ` or `* `) counts as its own paragraph. Its first sentence is the
idea's `description` in the frontmatter and in `index.md`.

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
| `Expect standard output` | a string | standard output, compared exactly |
| `Expect standard error` | a string | standard error, compared exactly; not compared when absent |
| `Expect exit code` | a number from 0 to 255 | the exit code; every example has it |
| ``Expect file `<path>` `` | a string | a file's content after the run, compared exactly |

The keys of a screen example (state and text are compared; pixels are not, section 7):

| Key | Value | Means |
|---|---|---|
| ``State `<field>` `` | a string, an integer, `true` or `false` | the screen's state before |
| `Click` | `` `<element>` `` | the user clicks the element |
| ``Type into `<element>` `` | a string | the user types the text into the element |
| ``Expect state `<field>` `` | a string, an integer, `true` or `false` | the state after |
| `Expect text` | a string | text the screen shows |

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
draws boundary values first, then values from a fixed seed (at L5).

**`# Constraints`**: only list items, one line each, `- <Key>: <value>`:

| Key | Value | Where |
|---|---|---|
| `Reads` | `standard input`, `arguments`, `the clock`, `random numbers` or ``files in `<folder>` `` | top idea |
| `Writes` | ``files in `<folder>` `` | top idea |
| `Connects to` | `` `<host>:<port>` `` | top idea |
| `Memory` | a number and `KiB`, `MiB` or `GiB`, up to 4 GiB; required | top idea |
| `Time` | a number and `ms` or `s`, up to 3600 s; required | top idea |
| `Modules` | module names in backticks, commas between them | top idea |
| `Exports` | function names in backticks, commas between them | top idea |
| `Uses` | module names in backticks, commas between them | any idea |
| `Uses crate` | `` `<name>` <major.minor.patch>, for <reason> `` | any idea |

Standard output and standard error are always granted. Every file in `ideas/` must be in
`- Modules:`, and every name there must have its file. A `Uses` name must be a module of the
program (lengua libraries come at L7). The same line two times is a finding.

**`# Structure`**: only list items, ``- `<name>`: <one sentence>.``, where the name is a
function, `snake_case`, or a method, `Type::snake_case` with the type in `UpperCamelCase`. The
sentence may continue on indented lines; it is one STE sentence and ends with a full stop. A
name is there one time only. The top idea's structure has `main`. At L4 `check` holds
these names to the functions of the Rust file (rule 12).

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

The idea's grants generate the WIT world in `wit/world.wit`. The world imports only the WASI
interfaces the grants allow: standard output always; standard input, arguments, the clock,
random numbers, files and sockets only when granted.

Two checks hold it:
- **At check:** the built component's imports must all be in the world. Anything else is
  refused.
- **At run:** the host links only the world's interfaces, with lengua's policy on top: files only
  in the folders the idea names, connections only to the hosts and ports it names, memory and
  time limits from the idea.

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

[workspace]
```

- `name` is the folder's name. It must start with a lowercase letter and have only lowercase
  letters, digits, `-` and `_`, and not be a Rust keyword or a name Cargo keeps (`std`, `test`,
  `build`, `deps`, `examples`, `con`, ...); otherwise the finding `package-name`.
- `include` lists the Rust file of every idea, in the order of the ideas: `src/main.rs`, then
  `src/<module>.rs` and `src/ui/<screen>.rs`. Rust finds a module through its `mod` line, so
  this is the package's list of files, not how it is built.
- A program is a `[[bin]]`. A program with `- Exports:` (a library, L7) is a `[lib]` instead,
  with the name in `snake_case` and `path = "src/main.rs"`.
- `[dependencies]` is there only when an idea has `- Uses crate:` lines: one line per crate,
  ``<name> = "=<version>"``, pinned to the exact version, sorted by name. Every idea's crates
  are the program's; the same crate at two versions is the finding `crate-version`.
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
  import wasi:cli/exit@0.2.12;
  import wasi:cli/stderr@0.2.12;
  import wasi:cli/stdout@0.2.12;
  import wasi:cli/terminal-output@0.2.12;
  import wasi:cli/terminal-stderr@0.2.12;
  import wasi:cli/terminal-stdout@0.2.12;
  import wasi:io/error@0.2.12;
  import wasi:io/poll@0.2.12;
  import wasi:io/streams@0.2.12;

  export wasi:cli/run@0.2.12;
}
```

The world is a subset of WASI 0.2's `wasi:cli/command`
([command.wit](https://github.com/WebAssembly/WASI/blob/v0.2.12/proposals/cli/wit/command.wit),
[imports.wit](https://github.com/WebAssembly/WASI/blob/v0.2.12/proposals/cli/wit/imports.wit)):
one `import` line per interface, sorted, each once, and `export wasi:cli/run` for a program (not
for a library). The interfaces of each grant, with the interfaces their types come from:

| Grant | Interfaces |
|---|---|
| always (standard output, standard error, the exit code) | `wasi:cli/exit`, `stdout`, `stderr`, `terminal-output`, `terminal-stdout`, `terminal-stderr`; `wasi:io/error`, `poll`, `streams` |
| `- Reads: standard input` | `wasi:cli/stdin`, `terminal-input`, `terminal-stdin` |
| `- Reads: arguments` | `wasi:cli/environment` (the arguments and the environment variables are in one interface; at L3 the host is to give no variables) |
| `- Reads: the clock` | `wasi:clocks/monotonic-clock`, `wall-clock` |
| `- Reads: random numbers` | `wasi:random/random`, `insecure`, `insecure-seed` |
| `- Reads:` or `- Writes: files in ...` | `wasi:filesystem/types`, `preopens`; `wasi:clocks/wall-clock` (for the type `datetime`) |
| `- Connects to: ...` | `wasi:sockets/network`, `instance-network`, `tcp`, `tcp-create-socket`, `ip-name-lookup`; `wasi:clocks/monotonic-clock` (for the type `duration`) |

UDP is in no grant. The version is `0.2.12`, the current WASI 0.2 release; at L3 the
component's imports are held to the world by package and interface name, with any `0.2`
version, since WASI 0.2 releases only add to the interfaces. The folders, hosts, memory and time
are not in the world: the host reads them from the idea at L3.

The world is not given to a binding generator. A `wasm32-wasip2` program that uses only the
standard library needs no `wit-bindgen`: `std` reaches WASI 0.2 itself, and the Rust toolchain
makes the component. So the world needs no `wit/deps/` folder until a program uses WASI's HTTP
world (L7), and it is the statement the component is held to at L3.

**The build job.** With no findings and a `src/main.rs`, `check` starts a build in the
background, in the program's folder, on a thread with one child process:

- `cargo fetch --target wasm32-wasip2`, only when the ideas use crates and `Cargo.toml` changed
  since the last fetch (the folder `target/lengua/` keeps the SHA-256 of the last one);
- then `cargo build --release --target wasm32-wasip2 --offline --message-format=short`.

The job's key is the SHA-256 of `Cargo.toml`, `wit/world.wit` and every file in `src/`. A
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
| `built` | the build passed | `seconds` it took, `target`, `component` (its absolute path), `size` (bytes), `sha256` |
| `build_failed` | the build failed | `seconds`, `target`; the errors are in `findings` |

The compiler's errors and warnings become findings, all of them, in the format of L1: the
file relative to the folder (with `/`), the line, the code `rust-error` or `rust-warning`, and
the message `column <n>: <the compiler's line>`, for example ``column 22: error[E0308]:
mismatched types``. An error with no place (Cargo's own) is `build-error` on `Cargo.toml`, line
1. `ok` is true when there are no findings, so it is also true while the build runs.

**`lengua-mcp doctor`** checks, in the current folder (so a `rust-toolchain.toml` there
counts), that `cargo` and `rustc` run, that `rustc` is 1.85 or later (edition 2024), and that
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

### Libraries and other languages

- **A lengua library** is another program of the bundle that declares `- Exports:`. A program
  that uses it names it in `- Uses:`, and `check` links it as a Rust path crate. Its own checks
  and tests stand.
- **A component built from another language** (C, Go, anything with a WASI 0.2 target) is linked
  through the component model. lengua did not build or check it, so it is labelled `trusted`.

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

#### The wiki's formats

Decided at L1 (2026-11-13). The folder is an OKF v0.2 bundle: every idea is a concept, and
`index.md` and `log.md` at the folder's root are OKF's reserved files (OKF sections 8 and 9).

- **When `check` writes it:** only when the check finds no problems, in the same step for all
  three (the frontmatter of every idea, `index.md`, `log.md`), so the wiki never shows a state
  the checks refused. A file whose text did not change is not written again. At L6 the lock is
  written in the same step.
- **The frontmatter** of each idea: the keys lengua writes come first, in the order of "The
  idea file's grammar", then the agent's keys as they were. An example:

  ```
  ---
  type: lengua Module
  description: "The module finds the words in a text."
  module: words
  source: src/words.rs
  uses: []
  used_by: [idea, ideas/ui/summary]
  label: unchecked
  idea_sha256: 3b0f…
  source_sha256: none
  title: Words
  ---
  ```

- **`index.md`** is written new by every check. It has `okf_version: "0.2"` as its only
  frontmatter (OKF allows this key in the root index), then up to three sections, `# Program`,
  `# Modules` and `# Screens`, with one line per idea: ``* [<title>](<file>) - <description>
  Label: `<label>`.`` The program's title is the folder's name; a module's or a screen's is its
  name.
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
  hash differs, and *removed* when its code file is gone. A check that changed nothing ends
  its first line with `; nothing changed`. Each *Note* is also in the reply's `notes`.

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

## Sources

- GPUI: [gpui.rs](https://gpui.rs/), [docs.rs/gpui](https://docs.rs/gpui/latest/gpui/),
  [examples](https://gpui.rs/examples/); its web target: Zed's
  [`crates/gpui_web`](https://github.com/zed-industries/zed/tree/main/crates/gpui_web) and
  pull request [#50228, "GPUI on the web"](https://github.com/zed-industries/zed/pull/50228),
  merged on 2026-02-26.
- GPUI Kit: [gpui-kit.com](https://gpui-kit.com/).
- The standards and the Power of 10: the links in sections 4 and 5.
