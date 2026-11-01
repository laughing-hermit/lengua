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

- **Checking the rules and proving are host-independent.** `llvm-mc` cross-assembles for any
  target and a proof is math, so one server build can carry every target's backend and check and
  prove for any of them, whatever host it runs on.
- **Testing and running need the target's CPU.** `check` runs the spec's tests, and `run` runs
  the program, so both need that CPU: natively, or under an emulator for development (QEMU's user
  mode runs Linux ARM64 programs on an x86-64 Linux host). Under emulation the kernel sees the
  emulator's own system calls, not the program's, so only Landlock and resource limits still
  confine a run; the spec-exact system-call filter, the tests' supplied random bytes and clock,
  and the cycle counts need the target natively. So the per-host part of the server is the
  runner, and a target is fully tested only on its own CPU (a CI runner counts).
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
   Mach-O/macOS, `lld-link` for COFF/Windows), its flags and the object format. `toolchain.rs`
   `link()` hardcodes `ld.lld` and ELF today, so it must learn to select these. Every
   instruction must link as written: on AArch64, `ld.lld` rewrites `adrp` + `add :lo12:` pairs
   into `nop` + `adr` unless it's given `--no-relax`, which would break the line-for-line match.
2. **Decoder** — a decoder for the CPU's machine code (today iced-x86, x86 only, reading ELF).
   For a new CPU, lengua decodes exactly its dialect's subset itself, so anything outside it is
   refused by construction, and tests that decoder against LLVM's disassembler.
3. **Prover model** — the CPU's instructions in `src/prove/exec`, and CPU-feature detection.
4. **Dialect** — the Power of 10 in that CPU's mnemonics and registers, with its own reference
   beside `linux-x86_64.md`.
5. **The OS edge** — on Linux, the system calls' numbers, the instruction and registers that make
   them, and the ABI's details, which differ between CPUs even on one OS (on AArch64: no `poll`,
   only `ppoll`; `clone`'s child-tid argument in another place; a 128-byte `struct stat` with
   `st_mode` at 16 rather than 144 and 24; `O_DIRECTORY` = `0o40000`, which is x86-64's value
   for `O_DIRECT`). **macOS and Windows don't allow direct syscalls:** a program must call the
   system library (libSystem, ntdll), so those model the system-library calling convention
   instead.
6. **The runner and sandbox** (the per-host part) — on Linux, seccomp and Landlock
   (`runner.rs`) and the supervisor that supplies tests' random bytes and clock (`supervise.rs`,
   which reads the CPU's registers); macOS and Windows each need their own confinement. macOS
   also needs the binary code-signed (ad-hoc via `ld64.lld`) before it will run.

## One product on every target

A port adds a CPU's or an OS's means, never a different lengua. The same spec means the same on
every target: the same skills, tools, reports and messages; the same rules, memory safety, tests
and proof engine; and the same limits on every OS. The capabilities a spec can name, and what the
sandbox allows of each, are one definition that each OS enforces with its own means; where an OS
can't provide or enforce part of it, that part is cut to what every OS can do, on Linux too. Only
the hardware may differ: its instructions, its vectors' width, its speed. A target is done when
it covers what x86-64 covers, each item with its own hardware's means.

## Order of targets

- **Linux ARM64 is nearest, and under way:** the same OS, so the same kinds of system calls, the
  same sandbox (seccomp, Landlock) and ELF; but its decoder, prover model, dialect and triple
  (2, 3, 4) are new, and the OS edge (5) and the supervisor's registers (6) change in detail.
  Tested under QEMU on an x86-64 Linux computer for everything but the sandbox's exact filter and
  supplies, and on an ARM64 computer or CI runner for those. Checked, tested and proven, as
  x86-64's code is, for the template's kind of program: arguments, whole numbers, floating point,
  vectors (NEON), text, standard input of any length through `fill`, and files and folders
  ([its reference](linux-aarch64.md)); its fast `wc` is proven for every length with a 64 KiB
  buffer, as x86-64's is. Floating point on vectors, threads, servers, the terminal and the
  crypto instructions come next.
- **macOS (ARM64, x86-64)** and **Windows (x86-64, ARM64)** need all six, the OS edge and the
  sandbox most of all, because they forbid the direct syscalls lengua's Linux programs use.
