# lengua-mcp

## Purpose

`lengua-mcp` is the MCP server of the lengua plugin. It serves MCP over standard input and
output with the official Rust SDK, `rmcp`. It has two tools. `check` checks the ideas of a
program's folder with `lengua-idea`, writes the generated files, builds the program to a
WebAssembly component in the background, and holds the component's imports to the world of its
grants, replying with every problem and the build's state as structured JSON. `run` runs the
built component on Wasmtime under exactly the grants of its idea, with the idea's memory and
time as limits. Its command `doctor` checks that the Rust toolchain can build. Later milestones
add the tests, the lock and the lean runner (see [Lengua3.md](../../Lengua3.md), section 4,
"The generated files, the build job and `doctor`", "Grants become the component's world" and
"The host").

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
- Given a client that lists the tools, when the server replies, then the list has two tools,
  `check` (input `folder`) and `run` (inputs `folder`, `arguments`, `stdin` and `fixed`), each
  with an output schema.
- Given a call of `check` with a folder, when the server replies, then the structured content
  has `ok`, `checked_by`, `findings` (each with `file`, `line`, `code` and `message`), `notes`,
  `written` and, when the ideas have no problems, `build`.
- Given a folder with ideas and code with no problems, when `check` is called until the
  build's state is not `building`, then the state is `built`, with the component's path, size
  and SHA-256.
- Given code that does not compile, when the build ends, then its state is `build_failed` and
  every error of the compiler is a finding with its file and line.
- Given a component that imports an interface outside the world of its grants, when the build
  ends, then its state is `build_failed` with one finding `import-ungranted` for each such
  import, and `run` replies `refused`, since the host links only the world.
- Given a change to the code after a build, when `check` is called, then a new build starts.
- Given no Rust toolchain on the path, when `check` would build, then the state is
  `build_failed` with one finding, `toolchain-missing`, whose message is `doctor`'s.
- Given a built program, when `run` is called, then the reply has the state (`exited`,
  `out_of_time`, `out_of_memory`, `trapped` or `refused`), the exit code, standard output and
  standard error; a run longer than 30 seconds replies `running` until a later call with the
  same input gets its end.
- Given arguments or standard input that the idea does not grant, when `run` is called, then
  the state is `refused` and the message names the grant.
- Given a folder that the idea grants, when the program reads or writes in it, then it can, as
  the grant says; given `..`, an absolute path or a symbolic link out of the folder, it cannot.
- Given `fixed: true`, when `run` is called two times, then the output is the same: a fixed
  wall clock, a stepped monotonic clock and random numbers from a fixed seed.
- Given a call of `check` or `run` with a path that is not a folder, when the server replies,
  then the result is a tool error.

## Structure

- `main`: reads the arguments and writes the version line or the usage line, runs `doctor`, or
  serves.
- `serve`: starts the host and the Tokio runtime on one thread, and serves `Lengua` on standard
  input and output.
- `run_doctor`: runs the doctor's checks in the current folder and writes their lines.
- `Lengua::check`: the tool; checks the folder and turns the report into the reply.
- `Lengua::reply`, `build_reply`: turn a `lengua_idea::Report` and the build's state into a
  `CheckReply`.
- `Lengua::run`: the tool; reads the ideas with `lengua_idea::program`, refuses input that is
  not granted (`ungranted_input`), and turns the run's progress into a `RunReply` (`run_reply`,
  `plain`, `millis`).
- `now`: the seconds since 1970, for the log's date.
- `CheckParams`, `CheckReply`, `FindingReply`, `NoteReply`, `BuildReply`, `RunParams`,
  `RunReply`: the tools' input and output, with their JSON schemas.
- `VERSION`: the name of the package and its version, joined by a space.
- `build.rs`: `Builds::new` and `Builds::status` give the state of a folder's build and start a
  new job when the build's key changed (`Job::start`, `Job::current`, `Job::status`,
  `Job::stop`); `run`, `fetch` (with `fetch_stamp`), `cargo` and `built` run the job on its
  thread, and `ungranted` holds the component's imports to the world; `problems`, `located` and
  `relative` turn the compiler's lines into findings; `lock` and `problem` are helpers.
- `host.rs`: `Host::new` makes the engine and its epoch thread; `Host::component` compiles a
  component once per SHA-256; `Host::imports` lists its imports, and `outside` and `grant_for`
  hold them to the world; `Host::run` and `Host::execute` run a component: `link` adds one
  interface of the world to the linker, `context` makes the WASI context from the grants and
  the input, `Limits` (with `limiter`) holds the memory, `Steps` and `Fixed` are the stepped
  and the fixed clock, `seeded` the fixed random bytes, `Guest` and `Table` the store's data,
  and `ending` reads how the run ended; `Input`, `Ending` and `Outcome` are its input and
  result.
- `runs.rs`: `Runs::new` and `Runs::run` start a run on its own thread or wait for the one with
  the same `key`, up to 30 seconds (`Job::start`, `Job::wait`), and give its `Progress`.
- `doctor.rs`: `examine` checks `cargo`, `rustc` (`version`) and the targets
  (`installed_targets`), with `output` and `install_rust`; `Doctor::ok`, `Doctor::text`,
  `Doctor::problems` and `Doctor::line` give the result.
- `tests/version.rs`: runs the binary with `--version` and with other arguments.
- `tests/doctor.rs`: runs `doctor` with the toolchain and with an empty path.
- `tests/check.rs`: starts the server, sends `initialize`, lists the tools, calls `check` on
  the fixtures of `lengua-idea`, and builds copies of the examples: `hello` as it is, with a
  compile error, after a change, with a hand-edited `Cargo.toml`, and with no toolchain; then
  runs `hello`, `echo` (arguments), `count` (standard input), `files` (a read folder, a write
  folder, and paths out of them), `ungranted` (refused at check and at run), `forever` (the
  time limit), `greedy` (the memory limit) and `clock` (fixed and real clock and random
  numbers).
