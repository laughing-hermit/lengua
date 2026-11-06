# lengua

An AI language: the agent writes a program's spec in plain words, its behaviors and tests
inside, then compiles it into assembly for this computer, Linux on x86-64 or on ARM64: the same
lengua on each. lengua checks the code against an adapted NASA Power of 10 and memory rules like
Rust's, runs every test, proves the program for every input it can, and runs it confined by the
kernel to the system calls, files and network its spec names.

- Skills: `write-spec` writes the spec, and `compile` turns it into `main.s`.
- Tools: `check` checks and tests, and `run` runs a checked program. From a shell, `lengua-mcp
  run <program folder> -- <arguments>` runs it too.

It needs Linux on x86-64 or on ARM64 with glibc 2.39 or later and Landlock on (Linux 5.13 or
newer; 6.12 for a program that uses the network), LLVM's `llvm-mc` and `ld.lld`, and, for proofs,
the z3 and cvc5 solvers. On Windows use WSL 2 or Docker, and on a Mac a Linux VM or Docker, and run
your agent and lengua inside it (the repository's README, "Windows and Mac").
