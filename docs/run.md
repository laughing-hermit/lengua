# Running a lengua program

A lengua program is a folder with two files: `idea.md`, an [Open Knowledge Format](https://github.com/GoogleCloudPlatform/open-knowledge-format)
Attested Computation that says what the program does and holds its tests, and `main.wat`, the
computation, in WebAssembly text. Every lengua idea names this page as its `executor.resource`:
this is how a lengua computation runs.

## Run it

```sh
lengua-mcp run <folder> [arguments…]
```

`lengua-mcp` is lengua's MCP server, which the lengua plugin installs. It runs a folder that
lengua's `check` took to done on this computer: `lengua.lock`, signed with this user's key,
records `idea.md` and `main.wat` as they are, and `check` built `main.wat`, held it to every rule
of lengua's WebAssembly dialect and ran every test, and none of its proofs found inputs where
`main.wat` is wrong. Given such a folder, it:

1. takes the standard WebAssembly module `check` tested, `main.wat` with lengua's small library
   merged in, which `check` kept in lengua's data folder, and runs it only when it is byte for
   byte the module `lengua.lock` records, so a module that breaks a rule never runs; where the
   kept module is missing or another, it builds `main.wat` and runs the build only when it is
   that same module;
2. runs it on Wasmtime, in lengua's own WASI host, with the current folder as the working folder
   (or the one `--in <folder>` names) and the arguments after the folder as the program's
   arguments. An argument that starts with `-` goes after `--`.

Any other folder it refuses, saying why, and exits with 125. To check a folder that came from
another computer, use [`lengua-mcp check`](check.md), or have the agent call `check` on it.

The program reaches only what its idea's `- Uses:` and `- Network:` lines name: its arguments and
standard streams always; standard input, files in the working folder, the environment (only the
variables each `--env NAME=value` gives), the clock, random bytes, and the connections lengua
makes for it only when the idea says so. A path that would leave the working folder is refused.
Memory and files are limited; a run from a shell has no time limit.

`lengua-mcp run` exits with the program's exit code. A program that traps (the engine stops it at
an instruction that can't go on) is named on standard error with its line in `main.wat`, and
`lengua-mcp run` exits with 134, as an abort does. In an agent, the MCP tool `run` runs the
program in a process of its own, for 10 seconds by default and with at most 8 MiB of output, and a
server keeps serving in the background until `run` stops it.

## The receipt

What a run gives back, the fields an idea's `executor.receipt` lists:

| Field | What it is |
|---|---|
| `stdout` | everything the program wrote to standard output |
| `stderr` | everything it wrote to standard error |
| `exit` | its exit code |
| `files` | the files it left in the working folder |

To check a run against its idea, see [check.md](check.md).

## Measure it

```sh
lengua-mcp measure [--in <folder>] [--stdin <file>] [--stdout <file>] <folder> [-- arguments…]
```

`lengua-mcp measure` runs the program once, in a process of its own, exactly as `lengua-mcp run`
runs it, and prints one line of JSON: its CPU time in user space and in the kernel, its time on
the clock, its peak resident memory and the size of the module it ran, as the system counts them
for that process (`getrusage` on Linux and macOS, the process's counters on Windows). Standard
input comes from the file `--stdin` names, standard output goes to the file `--stdout` names.

It measures other programs the same way, so they can be set side by side: any WebAssembly module
of WASI preview 1, such as a C, C++ or Rust toolchain makes, runs in lengua's own host, in the
same configuration of the engine, with what `--uses` and `--network` allow, written as an idea's
`- Uses:` and `- Network:` lines are; and a native executable runs as a shell starts it. With
`--serve <port>`, the program is a server: 64 clients each send 1,000 lines of 64 bytes, one at a
time, and must get each back in capitals, and its time on the clock runs from its first
connection to its last answer. The gate that holds lengua to C, C++ and Rust,
`benches/against_rust_and_c.rs`, measures every program this way.
