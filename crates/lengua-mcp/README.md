# lengua-mcp

## Purpose

`lengua-mcp` is the MCP server of the lengua plugin. It serves MCP over standard input and
output with the official Rust SDK, `rmcp`. It has one tool, `check`, which checks the ideas of
a program's folder with `lengua-idea`, writes the generated files, and builds the program to a
WebAssembly component in the background, replying with every problem and the build's state as
structured JSON. Its command `doctor` checks that the Rust toolchain can build. Later
milestones add the host, the tests, the lock and the tool `run` (see
[Lengua3.md](../../Lengua3.md), section 4, "The generated files, the build job and `doctor`").

## Behaviours

- Given no arguments, when `lengua-mcp` runs, then it serves MCP on standard input and output
  until its standard input closes.
- Given the argument `--version`, when `lengua-mcp` runs, then it writes `lengua-mcp
  <version>` and a line end to standard output and stops with exit code 0.
- Given the argument `doctor`, when `lengua-mcp` runs, then it writes one line for `cargo`, for
  `rustc` and for each of the targets `wasm32-wasip2` and `wasm32-unknown-unknown`, with the
  command that installs each missing part, and stops with exit code 0, or 1 when a part is
  missing.
- Given any other arguments, when `lengua-mcp` runs, then it writes `usage: lengua-mcp
  [--version | doctor]` to standard error and stops with exit code 2.
- Given a client that lists the tools, when the server replies, then the list has one tool,
  `check`, with an input schema (`folder`) and an output schema.
- Given a call of `check` with a folder, when the server replies, then the structured content
  has `ok`, `checked_by`, `findings` (each with `file`, `line`, `code` and `message`), `notes`,
  `written` and, when the ideas have no problems, `build`.
- Given a folder with ideas and code with no problems, when `check` is called until the
  build's state is not `building`, then the state is `built`, with the component's path, size
  and SHA-256.
- Given code that does not compile, when the build ends, then its state is `build_failed` and
  every error of the compiler is a finding with its file and line.
- Given a change to the code after a build, when `check` is called, then a new build starts.
- Given no Rust toolchain on the path, when `check` would build, then the state is
  `build_failed` with one finding, `toolchain-missing`, whose message is `doctor`'s.
- Given a call of `check` with a path that is not a folder, when the server replies, then the
  result is a tool error.

## Structure

- `main`: reads the arguments and writes the version line or the usage line, runs `doctor`, or
  serves.
- `serve`: starts the Tokio runtime on one thread and serves `Lengua` on standard input and
  output.
- `run_doctor`: runs the doctor's checks in the current folder and writes their lines.
- `Lengua::check`: the tool; checks the folder and turns the report into the reply.
- `Lengua::reply`, `build_reply`: turn a `lengua_idea::Report` and the build's state into a
  `CheckReply`.
- `now`: the seconds since 1970, for the log's date.
- `CheckParams`, `CheckReply`, `FindingReply`, `NoteReply`, `BuildReply`: the tool's input and
  output, with their JSON schemas.
- `VERSION`: the name of the package and its version, joined by a space.
- `build.rs`: `Builds::status` gives the state of a folder's build and starts a new job when the
  build's key changed (`Job::start`, `Job::current`, `Job::status`, `Job::stop`); `run`, `fetch`
  (with `fetch_stamp`), `cargo` and `built` run the job on its thread; `problems`, `located`
  and `relative` turn the compiler's lines into findings; `lock` and `problem` are helpers.
- `doctor.rs`: `examine` checks `cargo`, `rustc` (`version`) and the targets
  (`installed_targets`), with `output` and `install_rust`; `Doctor::ok`, `Doctor::text`,
  `Doctor::problems` and `Doctor::line` give the result.
- `tests/version.rs`: runs the binary with `--version` and with other arguments.
- `tests/doctor.rs`: runs `doctor` with the toolchain and with an empty path.
- `tests/check.rs`: starts the server, sends `initialize`, lists the tools, calls `check` on
  the fixtures of `lengua-idea`, and builds copies of `examples/hello`: as it is, with a
  compile error, after a change, with a hand-edited `Cargo.toml`, and with no toolchain.
