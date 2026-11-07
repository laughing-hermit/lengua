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

1. builds `main.wat` into a standard WebAssembly module, with lengua's small library merged in,
   and runs it only when the module is byte for byte the one `check` tested, so a module that
   breaks a rule never runs;
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
