---
type: lengua Crate
title: lengua-mcp
description: "The MCP server of the lengua plugin, with the tools check and run."
---

# lengua-mcp

## Purpose

`lengua-mcp` is the MCP server of the lengua plugin. It serves MCP over standard input and
output with the official Rust SDK, `rmcp`. It has two tools. `check` checks the ideas of a
program's folder with `lengua-idea`, writes the generated files, builds the program to a
WebAssembly component in the background, and holds the component's imports to the world of its
grants; then, in the same background job, it runs the examples of the ideas as tests and measures
them with mutants of the code, and gives the trust label, replying with every problem and the
job's state as structured JSON. `run` runs the
built component on Wasmtime under exactly the grants of its idea, with the idea's memory and
time as limits, in a process of its own that lengua stops at the time. When the job is done,
`check` writes `lengua.lock`, the signed hashes of the ideas, the wiki, the code, the generated
files, the component and its sealed precompiled code, and `run` refuses a program whose files
are not as the lock signed them. Its command `doctor` checks that the Rust toolchain can build.
See [Lengua3.md](../../Lengua3.md), section 4, "The lock", "The generated files, the build
job and `doctor`", "Grants become the component's world" and "The host", and sections 6
and 7.

## Behaviours

- Given no arguments, when `lengua-mcp` runs, then it serves MCP on standard input and output
  until its standard input closes.
- Given the argument `--version`, when `lengua-mcp` runs, then it writes `lengua-mcp
  <version>` and a line end to standard output and stops with exit code 0.
- Given the argument `doctor`, when `lengua-mcp` runs, then it writes one line for `cargo`, for
  `rustc`, `rustfmt`, `clippy` and each of the targets `wasm32-wasip2` and
  `wasm32-unknown-unknown`, with the
  command that installs each missing part, and stops with exit code 0, or 1 when a part is
  missing.
- Given any other arguments, when `lengua-mcp` runs, then it writes `usage: lengua-mcp
  [--version | doctor]` to standard error and stops with exit code 2.
- Given a client that lists the tools, when the server replies, then the list has two tools,
  `check` (input `folder`) and `run` (inputs `folder`, `arguments`, `stdin` and `fixed`), each
  with an output schema.
- Given a call of `check` with a folder, when the server replies, then the structured content
  has `ok`, `checked_by`, `findings` (each with `file`, `line`, `code` and `message`), `notes`
  (each with `file`, `line` when it has one, and `message`), `label`, `score`,
  `written` and, when the ideas have no problems, `build`.
- Given a folder with ideas and code with no problems, when `check` is called until the job's
  state is not `building`, `testing` or `mutating`, then the state is `done`, with the
  component's path, size
  and SHA-256, the tests that ran, the mutants, and the label `checked` or `tested`.
- Given an example whose output, exit code, end or files differ from what the program gives,
  when the tests run, then the state is `tests_failed` and the finding `test-failed` (or
  `case-failed`, with the drawn values) is at the idea and the line of the block, with the
  expected text and the text that came.
- Given a program whose tests pass, when the mutants run, then each mutant the tests do not
  catch is a note at its file and line, the score is caught of counted, and the label is
  `checked` at 80% or more and `tested` under it; a later check builds only the mutants of the
  functions that changed.
- Given a program with a `HashMap` and no grant of random numbers, when it runs, then it works:
  `insecure-seed` is in the always set, with a fixed value.
- Given code that `rustfmt` would change, when the build ends, then its state is `build_failed`
  with a `rust-format` finding for each place, and nothing else is built.
- Given code that clippy refuses (an `unwrap`, for one), when the build ends, then its state is
  `build_failed` and each of clippy's errors is a finding with its file and line.
- Given a component larger than 64 MiB, or one that lengua's engine does not take (threads,
  shared memory, relaxed SIMD), when the build ends, then its state is `build_failed` with the
  finding `component-size` or `component-refused`.
- Given the arguments `imports <file>`, when `lengua-mcp` runs, then it writes the imports of
  the component in the file, as Wasmtime reads them, one a line, sorted.
- Given code that does not compile, when the build ends, then its state is `build_failed` and
  every error of the compiler is a finding with its file and line.
- Given a component that imports an interface outside the world of its grants, when the build
  ends, then its state is `build_failed` with one finding `import-ungranted` for each such
  import, and no lock is written, so `run` refuses it; were it run, the host would not link the
  import, since it links only the world.
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
- Given a check whose job is `done`, when it replies, then it has written `lengua.lock` (in
  `written`), in the same call as the wiki, signed with the user's key, which the first `check`
  makes in lengua's data folder; a check whose job is not `done` writes no lock.
- Given a locked program, when one byte of an idea, `index.md`, `log.md`, a file in `src/`,
  `Cargo.toml`, `wit/world.wit`, `Cargo.lock`, the component or the sealed code changes, or a
  new idea or Rust file comes, or the lock was signed with another key, when `run` is called,
  then the state is `refused` and the message names the file and says "run `check`".
- Given a program that waits in a host call (a sleep) past its time, when `run` is called or
  the tests run, then its process is stopped at the time and two seconds, and the run ends
  `out_of_time`.
- Given the arguments `worker <kind> <lock> <folder> <root> <fixed | live> [arguments...]`,
  when `lengua-mcp` runs, then it reads the program's standard input from its own, verifies
  the lock, loads the sealed code, runs the program with its output in memory, and writes the
  outcome in one frame to standard output; `run` and the tests start it, one process a run.
- Given a call of `check` or `run` with a path that is not a folder, when the server replies,
  then the result is a tool error.

## Structure

- `main`: reads the arguments and writes the version line or the usage line, runs `doctor`,
  prints the imports of a component (`print_imports`), or
  serves.
- `serve`: starts the host and the Tokio runtime on one thread, and serves `Lengua` on standard
  input and output.
- `run_doctor`: runs the doctor's checks in the current folder and writes their lines.
- `Lengua::check`: the tool; makes the user's key on the first call
  (`lengua_idea::lock::user_key_or_create`, 32 bytes from `getrandom`), checks the folder,
  writes the lock when the job is `done` (`seal_program`), and turns the report into the reply.
- `Lengua::reply`, `build_reply`: turn a `lengua_idea::Report` and the job's state (the
  snapshot taken while the wiki is written, or a new one) into a
  `CheckReply`.
- `Lengua::run`: the tool; verifies `lengua.lock` with `lengua_idea::lock::verify`, reads the
  ideas with `lengua_idea::program`, refuses input that is not granted (`ungranted_input`), and
  turns the run's progress into a `RunReply` (`run_reply`, `plain`, `millis`).
- `seal_program`: writes `lengua.lock` with the files of `lengua_idea::lock::program_files`,
  the component and the sealed code, when the snapshot's job is `done` for the same key.
- `now`: the seconds since 1970, for the log's date.
- `CheckParams`, `CheckReply`, `FindingReply`, `NoteReply`, `BuildReply`, `TestsReply`,
  `MutantsReply`, `RunParams`,
  `RunReply`: the tools' input and output, with their JSON schemas.
- `VERSION`: the name of the package and its version, joined by a space.
- `build.rs`: `Builds::new`, `Builds::snapshot` (the state of the job of the same key, or
  none) and `Builds::status` give the state of a folder's job and start a new one when the
  key changed, or when the component, its sealed code or its test lock is gone (`Job::start`,
  `Job::current`, `Job::status`, `Job::stop`), and keep the toolchain's version from
  `doctor`; the `Worker` runs it on its thread (`Worker::work`, with `update`, `stopped` and
  `finish`): `compile` (`fetch` with `fetch_stamp`, `step`, `cargo`; `cargo fmt --check` with
  `format_problems`, clippy, the build, and `built`, which seals the component), then the
  tests, then the mutants; `ungranted` holds the component's size (`too_large`, `SIZE_LIMIT`)
  and imports to the world; `problems`, `located` and `relative` turn the compiler's lines into
  findings; `Status` (with `Status::new`, `Status::score` and `Status::trust`, whose
  `THRESHOLD` is 80), `Stage` (with `Stage::name` and `Stage::running`), `Tests`, `Mutants`,
  `Component`, `Problem` and `Remark` are the job's state; `lock` and `problem` are helpers.
- `testing.rs`: `cases` expands the tests of the ideas (screens left out) into runs;
  `run_case` runs one in a new folder (`place`) in a worker process (`worker::run`, with the
  component's test lock), refuses input the idea does not grant (`ungranted`), and compares
  the end, the exit code, the output and the files (`compare`, with `diff` and `shown`);
  `problem` makes the finding, with `values_text` for a case's drawn values.
- `mutation.rs`: `run` gets the mutants from `lengua_idea::mutants`, at most `MUTANTS` (40),
  takes each result from the cache (`read_cache`, `write_cache`, keyed by `context` and
  `key`) or builds the mutant in its package (`workspace`, `mutant_name`), seals it, runs the
  tests on it (`verdict`, `judge`) and removes its sealed code, and makes a note of each
  survivor (`remark`); `Verdict` (with `name` and `parse`) and `Workspace` are its data.
- `host.rs`: `Host::new` makes `lengua_host`'s engine with `config` (`lengua_host::config`
  and NaNs canonical); `Host::engine` gives it; `Host::component` compiles a component once
  per SHA-256; `Host::imports` lists its imports, and `outside` and `grant_for` hold them to
  the world; `Host::seal` precompiles a component (`Component::serialize`) to
  `target/lengua/sealed/<sha256>.cwasm` (`SEALED`) and signs a test lock for it; `clear` and
  `unseal` remove sealed code; `Seal` names the two files.
- `worker.rs`: `run` starts `lengua-mcp worker` for one run (`Request`), gives it the standard
  input, reads its frame, and kills it at the time and two seconds (`stopped`); `serve` is the
  worker: `outcome` verifies the lock, reads the ideas, loads the sealed code with
  `lengua_seal::load` and runs it with `lengua_host`; `encode` and `decode` are the frame.
- `runs.rs`: `Runs::run` starts a run in a worker process on its own thread, or waits for the
  one with the same `key`, up to 30 seconds (`Job::start`, `Job::wait`), and gives its
  `Progress`; `GRACE` is the two seconds.
- `front.rs` (since L8): `build` copies `web/` to `target/lengua/web/`, runs `pnpm install`
  (frozen when `web/pnpm-lock.yaml` is there, else it keeps pnpm's lock), `tsc --noEmit`
  (`type_problems`), Vitest with its JSON report (`test_problems`) and `next build`, and writes the
  crate `lengua_web` of the export (`assets_text`, `media_type`); `judge` runs a screen's mutant.
- `doctor.rs`: `examine` checks `cargo`, `rustc` (`version`), `rustfmt`, `clippy` and the targets
  (`installed_targets`), with `output` and `install_rust`; `Doctor::ok`, `Doctor::text`,
  `Doctor::problems`, `Doctor::rustc` and `Doctor::line` give the result. Since L8 `front_items` checks `node` and `pnpm` (`find` on the `PATH`, `numbers`), and `Doctor::front_ok` and `Doctor::front_problems` give theirs.
- `tests/version.rs`: runs the binary with `--version` and with other arguments.
- `tests/doctor.rs`: runs `doctor` with the toolchain and with an empty path.
- `tests/check.rs`: starts the server (with `LENGUA_HOME` in Cargo's test folder), sends
  `initialize`, lists the tools, calls `check` on
  the fixtures of `lengua-idea`, and builds copies of the examples: `hello` as it is (no lock
  before `done`, a lock at `done`), with a
  compile error, after a change, with a hand-edited `Cargo.toml`, with no toolchain, with a
  comment (refused by the lint), unformatted and with an `unwrap` (refused by clippy); then
  runs `hello`, `echo` (arguments), `count` (standard input), `files` (a read folder, a write
  folder, and paths out of them), `ungranted` (refused at check and at run), `forever` (the
  time limit), `greedy` (the memory limit) and `clock` (fixed and real clock and random
  numbers); then the tests and mutants: a wrong output (`test-failed` with the diff, and no
  lock), a failing
  boundary of `small` (`case-failed`), `small` checked with its label in the wiki and the cache
  after a change of `main`, a weak idea of `small` tested with a surviving mutant as a note,
  and `words` (a `HashMap` with no grant); then the lock: a changed byte in each kind of locked
  file and a new Rust file refused at run and by `lengua-run`, a lock signed with another key
  refused, `lengua-run` on `hello`, `files` (arguments and folders) and `count` (standard
  input), and `sleepy` (a sleep past its time, stopped by `run` and by `lengua-run`);
  `until_built` writes how long each job took to `target/tmp/lengua-timings.txt`.
- `harness.rs` (since L7): `write` makes the harness of a library in `target/lengua/harness`
  (or, for a mutant, `target/lengua/mutant-harness`): a binary package whose `main.rs` calls the
  `Call` of each example on its own line, with the library's exports in scope, and prints the
  value with `{:?}`; `manifest` and `main_text` are its two files, and `mapped` turns an error
  of the harness's build into the finding `harness-error` at the example.
- `fixtures.rs` (since L7): `Fixtures::start` binds each `Server` of an example (waiting up to
  10 s for the port) and starts each `Client`, which connects to the program's port; `finish`
  stops them and gives what each client got.
- `wrapper.rs` (since L8): with `LENGUA_RUSTC_WRAPPER` set (`MARK`), `lengua-mcp <rustc>
  <arguments>` is the rustc wrapper that the build job gives Cargo (`RUSTC_WRAPPER`): `run`
  starts the compiler with the arguments `rewritten`, where a crate built for a `wasm32` target
  gets a `-C metadata` from `metadata` (its name, crate types, edition, target, `--cfg` values,
  the names of its `--extern` crates and its version, read by `value`), so that a component has
  the same bytes on every system; a crate built for the host keeps Cargo's.
