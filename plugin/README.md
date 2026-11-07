# lengua

An AI language: the agent writes a program's idea in plain words, its behaviors and tests
inside, then compiles it into code that lengua checks against an adapted NASA Power of 10 and
memory rules like Rust's, tests against every test in the idea, proves for every input it can,
and runs confined to what its idea names. An idea of the type Attested Computation becomes
WebAssembly, `main.wat`, which runs the same on every system lengua is built for; any other idea
becomes, for now, assembly for Linux on x86-64 or on ARM64, `main.s`.

- Skills: `write-idea` writes the idea, and `compile` turns it into `main.wat` or `main.s`.
- Tools: `check` checks and tests, and `run` runs a checked program. From a shell, `lengua-mcp
  run <program folder> -- <arguments>` runs it too.

lengua's release archive for each system and CPU it is built for (Linux on x86-64 or ARM64,
macOS on ARM64, Windows on x86-64 or ARM64) holds this plugin with its server,
`server/lengua-mcp`, and the z3 and cvc5 solvers its proofs need, in `solvers/`: a `main.wat`
program needs nothing more, but on Linux a glibc of 2.39 or later. `lengua-mcp doctor` says
whether lengua has what it needs here. A `main.s` program needs Linux with Landlock on (Linux
5.13 or newer; 6.12 for a program that uses the network) and LLVM's `llvm-mc` and `ld.lld`.
