# Porting lengua to another target

lengua is hardware-agnostic where it matters: the **spec** names no CPU or OS (`check` rejects a
CPU or OS term in a spec's prose), so the thing people share is the same on every target. What is
per-target is the compiled `main.s` and lengua's checking of it — and that is per-target by nature:
you can't check native code without modelling the native machine. This file is the contract for
adding a target, so "add a target" is a bounded task, not a rewrite.

lengua's design is **native per target** (the AI writes the target's assembly; lengua checks and
proves the actual machine code), chosen over compiling to a portable IR because it keeps the
hand-tuned speed that beats C/C++/Rust and proves the literal bytes that run. LLVM is used only as
a **non-optimizing assembler and linker** (`llvm-mc`, `lld`), never as a codegen backend — an
optimizer would rewrite the instructions the prover just checked.

## What needs the host, and what doesn't

- **Checking, proving and compiling are host-independent.** `llvm-mc` cross-assembles for any
  target and a proof is math, so one server build can carry every target's backend and compile,
  check and prove for any of them, whatever host it runs on.
- **Only `run` needs the matching host** — its sandbox is OS-specific, and executing non-host code
  needs that host or an emulator. So the one truly per-host part of the server is the runner.
- **The plugin decides per target.** The server detects its host at startup
  (`Profile::for_target` over `std::env::consts`) and uses that target; the user installs one
  plugin and it picks what it needs. On an unsupported host it says so and points here. Ship one
  server build per host OS (its runner + the shared backends); a selector picks the host's build;
  the skills (markdown) are the same everywhere.
- **No hardware needed to build or test a target.** Rust cross-compiles the server (bar the
  Linux-only confinement); CI runners are real Linux, macOS (Apple silicon included) and Windows
  machines; QEMU runs Linux ARM64 locally.

## What a new target profile must supply

A profile lives in `src/platform.rs` (today one variant, `LinuxX86_64`, holding the triple, the
short name and the syscall-number table). Each new target needs:

1. **Toolchain** — the LLVM triple, the linker (`ld.lld` for ELF/Linux, `ld64.lld` for
   Mach-O/macOS, `lld-link` for COFF/Windows) and the object format. `toolchain.rs` `link()`
   hardcodes `ld.lld` and ELF today, so it must learn to select these.
2. **Decoder** — a decoder for the CPU's machine code (today iced-x86, x86 only, reading ELF).
   LLVM's disassembler, or Capstone, could make this step multi-architecture.
3. **Prover model** — the CPU's instructions in `src/prove/exec`, and CPU-feature detection.
4. **Dialect** — the Power of 10 in that CPU's mnemonics and registers, with its own reference
   beside `linux-x86_64.md`.
5. **The OS edge** — on Linux, the syscall-number table (in the profile) and direct `syscall`
   instructions. **macOS and Windows don't allow direct syscalls:** a program must call the system
   library (libSystem, ntdll), so those model the system-library calling convention instead.
6. **The runner and sandbox** (the per-host part) — on Linux, seccomp and Landlock
   (`runner.rs`, `supervise.rs`); macOS and Windows each need their own confinement. macOS also
   needs the binary code-signed (ad-hoc via `ld64.lld`) before it will run.

## Order of targets

- **Linux ARM64 is nearest:** same OS, so it reuses the OS edge (5) and the runner (6) unchanged,
  and keeps ELF; only the decoder, prover model, dialect and a new triple with ARM64 features (2,
  3, 4) are new. Testable here under QEMU.
- **macOS (ARM64, x86-64)** and **Windows (x86-64, ARM64)** need all six, the OS edge and the
  sandbox most of all, because they forbid the direct syscalls lengua's Linux programs use.
