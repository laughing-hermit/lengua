---
type: lengua Crate
title: lengua-run
description: "The lean runner, which runs a checked lengua program from a shell."
---

# lengua-run

## Purpose

`lengua-run` is the lean runner: it runs a checked lengua program from a shell,
`lengua-run [--listen <address:port>] <folder> [arguments...]`, or `lengua-run --desktop <folder>`
for a desktop app. It has Wasmtime without its compiler (no Cranelift; the
`runtime` feature only), so it loads only the sealed precompiled code that `check` made, and only
after it verified the program's `lengua.lock` exactly as the `run` tool does. It grants only
what the idea grants, and passes standard input (when granted), standard output and standard
error through. See [Lengua3.md](../../Lengua3.md), section 4, "The lock" and "`lengua-run`".

## Behaviours

- Given a checked folder whose files are all as its lock signed them, when `lengua-run` runs it,
  then the program runs under its grants with the arguments given, and `lengua-run` stops with
  the program's exit code.
- Given a folder with no lock, a lock signed with another key, or any locked file changed (an
  idea, `index.md`, `log.md`, a file in `src/`, a generated file, `Cargo.lock`, the component,
  the sealed code) or a new idea or source file, when `lengua-run` runs it, then it writes
  `lengua-run: refused: ...`, naming the files and "run `check`", to standard error and stops
  with exit code 125, and runs nothing.
- Given arguments and an idea with no `- Reads: arguments`, then it refuses (125).
- Given a program that runs past the time of its idea, computing or waiting in a host call
  (a sleep, a blocking read), when the time and two seconds have passed, then `lengua-run`
  writes `lengua-run: out of time: ...` and stops with exit code 124; the process's end stops
  the waiting call too.
- Given a program that grows its memory past its idea, then exit code 126; a trap, 134.
- Given a server and `--listen <address:port>`, then it serves that address in place of
  `127.0.0.1:N`, as a container needs (since L9).
- Given `--desktop` and a desktop app, then it serves the one file on its port and starts
  Electron from the app's `target/lengua/web/node_modules` on `web/desktop/main.mjs`, with
  `LENGUA_URL`; it ends with the window, with Electron's exit code (since L10). Given a folder
  that is not a desktop app, or a native part's folder, it refuses (125).
- Given `--version`, then it writes `lengua-run <version>`; given no arguments, the usage line
  and exit code 2.

## Structure

- `main`: reads the arguments; `--version`, the usage line, or `run`.
- `not_this_kind`: refuses a native part, and `--desktop` for an app that is not a desktop one.
- `electron`, `open_window`: find Electron of the app, and start it on the served address once it
  answers, on a thread that ends the process with its exit code.
- `run`: reads the user's key, verifies the lock, reads the ideas with `lengua_idea::program`,
  refuses ungranted arguments, loads the sealed code with `lengua_seal::load`, starts `watch`,
  and runs the program with `lengua_host` and `Streams::Inherit`.
- `watch`: a thread that ends the process at the idea's time and two seconds (`GRACE_MS`).
- `refuse` and `ended`: write lengua-run's own line and give the exit code (`REFUSED`,
  `OUT_OF_TIME`, `OUT_OF_MEMORY`, `TRAPPED`).
- `tests/usage.rs`: the version, the usage line, a folder with no key or lock. The runs of real
  programs are in `lengua-mcp`'s `tests/check.rs`, which checks them first.

Since L7, `lengua-run` links the trusted components that the lock seals, and a server (an idea
with `- Serves: HTTP on TCP port N`) is served on `127.0.0.1:N` until the process is stopped,
each request under the idea's time and memory.
