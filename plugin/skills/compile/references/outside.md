# An outside module: a program a compiler made from another language

lengua's own way is `main.wat`, which you compile from the idea by hand, and which `check` holds to
every rule and proves. That stays the default, because only that program is checked and proven.

An outside module is the other way: `main.wasm`, a WebAssembly module that a compiler made from
Rust, C or another language. lengua runs it as the idea's program, in the same host, under the
same grants, and against the idea's tests. It holds the module to none of main.wat's rules and
proves nothing of it. Every reply and every run says so: **trusted, not proven**.

## When to use one

Use an outside module only when the user asks for it:

- the user has code in another language and wants to run that code, not a new program;
- the user names the language the program must be in ("write it in Rust", "it must be C");
- the program needs a library that exists in that language and that you would not write again,
  and the user accepts what is lost.

Otherwise compile the idea to `main.wat`. Do not choose an outside module because it is faster to
write, and never to get past a rule or a proof that `main.wat` fails: say what failed instead.

Before you build one, tell the user in one or two lines what they get and what they give up (the
table below), and go on only when they still want it.

## What holds, and what doesn't

| | `main.wat` | `main.wasm`, an outside module |
|---|---|---|
| Reaches only what the idea grants: files, network, programs, terminal, environment, clock | yes: the checker refuses the import, and the host refuses the call | yes: the host refuses the call, and `check` refuses an import of lengua's own modules that the idea doesn't grant |
| Its memory stays its own, and within its declared size | yes | yes: WebAssembly and the host hold it, to at most 1 GiB |
| Stops at the deadline of a test and of the `run` tool | yes | yes: the engine stops it |
| Ends by itself (bounded loops, no recursion) | yes, by rule | no: a run from a shell has no deadline, so there the user stops it |
| Memory rules M1 to M8: no access outside its object, no use after free | yes | no: it can corrupt its own memory, as a C program can |
| The secret rule (`- Secret:`) | yes | no: `check` refuses an idea with a `- Secret:` line |
| Assertions, checked results, the rest of the Power of 10 | yes | no |
| Proofs for every input | where the prover reaches | none: tests only |
| lengua's entry in `verified`, and `lengua-mcp check` attests it | yes | no |
| Parts (`- Builds on:`) | yes | no: `check` refuses the line |
| `check`'s last state | `done` | `trusted_not_proven` |

So an outside module is safe to run in this sense: it can't read the user's files, open a
connection or start a program unless the idea grants it, and it can't leave its own memory. It is
not shown to be correct beyond the tests' own runs.

## The idea

The idea is written as for any program, with the write-idea skill: its behaviors, its tests and
its grants. Two things say that its program is an outside module, and `check` holds them to each
other:

- the frontmatter says `computation: main.wasm`;
- `# Constraints` has one line `- Module: main.wasm, built from …`, which says in up to 200
  characters what the module was built from: the language, the source and the compiler.
  lengua records these words and shows them to the user. It can't check them.

````markdown
---
type: Attested Computation
title: count
description: Prints the number of bytes in the file notes.txt of the current folder.
runtime: webassembly
computation: main.wasm
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
The program reads the file `notes.txt` in the current folder. Then it prints the number of bytes
in the file and a newline.

The file has up to `4096` bytes. If the program cannot read the file, it prints `no notes.txt` and
a newline to standard error. Then it exits with code 3.

# Behaviors
## B1: Prints the number of bytes in the file
- Given a file `notes.txt` that contains `hello`
- When the program runs
- Then it prints `5` and a newline
- And it exits with code `0`

### For any `text`
- Given a file `notes.txt` that contains `{text}`
- When the program runs
- Then it prints `{len(text)}` and a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 4096, "abc \n")` | 20 |

## B2: Says that the file is not there
- Given no arguments
- When the program runs
- Then it prints `no notes.txt` and a newline to standard error
- And it exits with code `3`

# Constraints
- Uses: files
- Module: main.wasm, built from Rust (src/main.rs) with rustc for wasm32-wasip1
````

Write the tests as carefully as for `main.wat`: they are all that shows the program does what the
idea says. An idea with a `- Secret:` line or a `- Builds on:` line can't have an outside module.

## Building the module

Build a command (a program with a `main`) for the target `wasm32-wasip1`, WASI preview 1, and put
the result in the program's folder as `main.wasm`. Keep the source beside it or where the user
wants it: lengua reads only `main.wasm`. The folder holds no `main.wat`.

Rust:

```
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1
```

with, in `.cargo/config.toml` or `RUSTFLAGS`, the memory's maximum, and in the release profile
`panic = "abort"` and `strip = true`:

```
[target.wasm32-wasip1]
rustflags = ["-C", "link-arg=--max-memory=67108864"]
```

One file with no dependencies builds with rustc alone:

```
rustc --edition 2024 --target wasm32-wasip1 -C opt-level=2 -C panic=abort -C strip=symbols \
  -C link-arg=--max-memory=67108864 -o main.wasm src/main.rs
```

C, with [wasi-sdk](https://github.com/WebAssembly/wasi-sdk)'s clang:

```
$WASI_SDK/bin/clang --target=wasm32-wasip1 --sysroot=$WASI_SDK/share/wasi-sysroot -O2 \
  -Wl,--max-memory=67108864 -Wl,--strip-all -o main.wasm main.c
```

What `check` needs of the module, and refuses it without, each with what to change:

- **A stated maximum for its memory**, of at most 1 GiB (16,384 pages): `--max-memory=` in bytes,
  a multiple of 65,536. The examples give 64 MiB. lengua holds a program to the memory its module
  declares, so a module with no maximum is refused. The memory is exported as `memory`, which
  these toolchains do by themselves.
- **A `_start` that takes and gives nothing**: a command, which these toolchains make from
  `main`. A library (a reactor) has none.
- **Imports of lengua's host alone**: the functions of `wasi_snapshot_preview1`, which the
  toolchain's standard library imports, and lengua's own modules for what an idea grants beyond
  WASI ([the host](host.md) lists them). A function of another host (`env`, WASI preview 2, a
  JavaScript glue module) is refused: build for `wasm32-wasip1`, not `wasm32-unknown-unknown`,
  `wasm32-wasip2` or Emscripten.
- **The engine's features alone**: WebAssembly 1.0 with sign extension, saturating
  float-to-int conversions, multiple values, reference types (a toolchain's table of functions),
  bulk memory and SIMD128. No threads, tail calls, exceptions, 64-bit or multiple memories, GC,
  relaxed SIMD or components. A Rust or C release build for `wasm32-wasip1` uses none of them;
  for C++, build with `-fno-exceptions`.
- **At most 16 MiB** of module. Strip the names and the debug sections.

A WASI function the idea doesn't grant still links, since a toolchain's startup imports several
whatever the program does. The host answers each call of it with the error "not capable" (76),
or harmlessly: no environment, no folder, zeros for random bytes. `check`'s `outside` lines name
the ones the module imports.

A program reaches the files of the folder it runs in with `- Uses: files`, through its
language's own file functions. For what lengua gives beyond WASI (a connection the program opens
itself, a file an argument names, a program it starts, the terminal), the idea needs the grant's
line, and the program imports the function from lengua's module, with the types
[the host](host.md) gives. In C:

```c
__attribute__((import_module("lengua_net_1"), import_name("tcp_open")))
extern int32_t tcp_open(const char *name, int32_t name_len, int32_t port, int32_t tls, int32_t *fd);
```

In Rust:

```rust
#[link(wasm_import_module = "lengua_net_1")]
unsafe extern "C" {
    fn tcp_open(name: *const u8, name_len: u32, port: u32, tls: u32, fd: *mut u32) -> u32;
}
```

`check` refuses such an import when the idea doesn't grant it, and the host holds every call to
the grant's own limits: the computers, the ports, the arguments.

## Checking and running it

Call `check` with `path` as for any program. Its states are the same up to the last:

| `state` | What it means for an outside module |
|---|---|
| `write_code` | The idea is recorded and there's no `main.wasm` yet: build it. |
| `code_problems` | `check` refuses the module, and each problem says what to build differently; or the folder holds `main.wat` too, which is one code file too many. |
| `testing`, `tests_failed` | As for `main.wat`. Fix the source and build again, or fix the test. A failure has no line of code. |
| `trusted_not_proven` | Every test passed. This is the last state: there is no `proving` and no `done`. |

Every reply about an outside module has `outside`: its first line says `trusted, not proven` and
what the idea says the module was built from, then one line for each guarantee that does not
hold (`not held`, `not proven`, `not attested`), then what lengua's host holds (`held`). The
`proofs` lines all say `not proven`, there is no `memory` line, and lengua writes no entry in
idea.md's `verified`.

`run` and the reply's `shell` command run it as any program: `run`'s reply has `outside` too, and
a run from a shell says the same words on standard error before the program starts. A run from a
shell has no deadline, for any program; an outside module has no bounded loops either, so say
that to a user who will run it from a script or a service.

Every build is a new module: after any change to the source, build `main.wasm` again and call
`check`. `run` runs only the very bytes `check` tested.

## What to tell the user

At `trusted_not_proven`, tell the user in plain words, and never say the program is checked,
proven or done:

- where the program is, what its idea promises, and that it passed all its test runs;
- that it is an outside module, built from what the idea says, and that lengua can't check that;
- that lengua holds it to its idea's grants (say them) and to its own memory, so it can't touch
  anything else of the computer;
- that lengua does not hold it to its rules and proved nothing: it is tested, and it may still be
  wrong or never end on an input the tests didn't try;
- the `shell` command, and that a run from a shell has no time limit.

If the user wants the guarantees, offer to compile the same idea to `main.wat`: remove the
`- Module:` line, write `computation: main.wat`, and follow the compile skill's steps.
