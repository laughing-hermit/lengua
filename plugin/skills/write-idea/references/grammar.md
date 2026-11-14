# The idea file's grammar

`check` reads every idea to this grammar and returns every problem at once, each with its
file, line, code and message.

## Files

| File | Kind | Code |
|---|---|---|
| `idea.md` | the top idea: a program, a server or a library | `src/main.rs` |
| `ideas/<name>.md` | a module, `snake_case`, not `main`, `ui`, `index` or `log` | `src/<name>.rs` |
| `ideas/ui/<name>.md` | a screen of the front end, the module `ui/<name>` | `web/screens/<name>.tsx` |
| `vision.md` | the vision (skill `vision`) | none |

Any other folder under `ideas/` is a finding.

## The frontmatter

Optional: an idea can start at `# Purpose`. `check` writes `type`, `description`, `module`,
`source`, `uses`, `used_by`, `label`, `idea_sha256`, `source_sha256`, `cargo_sha256`,
`world_sha256`, `web_sha256`, `generated` and `verified`, and `status` (`draft`, or `stable`
while the person's approval holds). The tool `approve` writes `approved`. You may write
`title`, `tags`, `sources`, `stale_after`, `status: deprecated` and keys of your own; `check`
keeps them and checks OKF's values.

## The sections

Level-1 headings, in this order, each one time: `# Purpose`, `# Behaviours`, `# Constraints`,
`# Structure`. The top idea has all four; a module or a screen may leave out `# Constraints`.
`check` writes a last section, `# Links`; don't edit it.

- **`# Purpose`**: paragraphs in STE, no headings, no fenced blocks. The first sentence is the
  description. A module or library named in backticks must be listed in `- Modules:` or
  `- Uses:` (`purpose-unlisted`).
- **`# Behaviours`**: one `## <name>` or more, each with `Given`, `When` and `Then` one time
  each, in order, `And` after any of them, then one `### Example:` or `### Cases:` block or
  more. See [the test format](test-format.md).
- **`# Constraints`**: only `- <Key>: <value>` lines, below.
- **`# Structure`**: ``- `<name>`: <one STE sentence>.`` for every function: `snake_case`, or
  `Type::snake_case` for a method; in a screen, `lowerCamelCase` functions and `UpperCamelCase`
  components, the main one `<Name>Screen` (`HomeScreen` for `ui/home`). The top idea of a
  program has `main`. Every function of the code is named, and nothing else is.

## Constraints

| Key | Value | Where |
|---|---|---|
| `Reads` | `standard input`, `arguments`, `the clock`, `random numbers`, ``files in `<folder>` ``, ``the secret `<NAME>` `` | top idea |
| `Writes` | ``files in `<folder>` `` | top idea |
| `Connects to` | `` `<host>:<port>` `` | top idea |
| `Listens on` | `TCP port <number>` | top idea |
| `Serves` | `HTTP on TCP port <number>`: the idea is a server | top idea |
| `Memory` | a number and `KiB`, `MiB` or `GiB`, up to 4 GiB; required | top idea |
| `Time` | a number and `ms` or `s`, up to 3600 s; required | top idea |
| `Modules` | module names in backticks, commas between them (`` `cart`, `ui/basket` ``) | top idea |
| `Exports` | function and type names in backticks: the idea is a library | top idea |
| `Uses` | modules, or folders of lengua libraries (`` `../../libraries/json` ``) | any idea; a library only in the top idea |
| `Uses crate` | `` `<name>` <major.minor.patch>, for <reason> `` | any idea |
| `Uses package` | `` `<name>` <major.minor.patch>, for <reason> ``: an npm package, pinned | any idea |
| `Uses shadcn` | `button`, `card`, `input`, `label`, `textarea`, in backticks | top idea |
| `Uses component` | `` `<file>.wasm`, built from <words> ``: a component from another language | top idea |
| `Route` | `` `<METHOD> <path>` ``, parts may be `{<name>}`: a route the front end calls | top idea of a server |
| `Asks` | `an AI model, for <what>, up to <N> calls and <M> tokens` | top idea |
| `Calls` | ``HTTP at `<host>:<port>` ``, or ``HTTPS at `<host>:<port>` `` (L10) | top idea |
| `Native` | `<what it is>, for <reason>` (this idea is a native part, or a desktop app), or `` `<folder>`, for <reason> `` (a native part it starts), L10 | top idea |

Standard output and standard error are always granted. Grant only what the program needs.
The reasons of `- Native:` are `large AI models`, `the fastest local inference`, `training or
fine-tuning`, `heavy compute with shared-memory threads`, `a device with no host grant` and
`the Electron shell of a desktop app`.

## Shapes

- A **program** runs once, from standard input and arguments to standard output.
- A **server** has `- Serves:`; its examples are requests; it reads no standard input or
  arguments. An idea with screens is a server: its back end serves the front end. A screen's
  route is `/` for `home` and `/<name>/` for any other.
- A **library** has `- Exports:` and no grants; its examples are calls.
- A **native part** (L10) has `- Native: <what>, for <reason>`, reads only standard input and
  arguments, and is built for the computer, not for WebAssembly; label `native`.

A module needs its `mod <name>;` line in `src/main.rs`, and each `- Exports:` name a `pub` item.
