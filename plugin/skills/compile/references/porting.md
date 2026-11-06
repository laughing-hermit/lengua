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
  confine a run; the spec-exact system-call filter and the cycle counts need the target natively.
  The tests' supplied random bytes and clock come from lengua's broker, so they work under
  emulation too. So the per-host part of the server is the runner, and a target is fully tested
  only on its own CPU (a CI runner counts).
- **The plugin decides per target.** The server detects its host at startup
  (`Profile::for_target` over `std::env::consts`) and uses that target; the user installs one
  plugin and it picks what it needs. On an unsupported host it says so and points here. Ship one
  server build per host OS (its runner + the shared backends); a selector picks the host's build;
  the skills (markdown) are the same everywhere.
- **No hardware needed to build or test a target.** Rust cross-compiles the server (bar the
  Linux-only confinement); CI runners are real Linux, macOS (Apple silicon included) and Windows
  machines; QEMU runs Linux ARM64 locally.

## What a new target profile must supply

A profile lives in `src/platform.rs` (today two variants, `LinuxX86_64` and `LinuxAarch64`, each
holding the triple, the short name, the syscall-number table and the baseline feature set, below).
Each new target needs:

1. **Toolchain** — the LLVM triple, the linker (`ld.lld` for ELF/Linux, `ld64.lld` for
   Mach-O/macOS, `lld-link` for COFF/Windows), its flags and the object format. `toolchain.rs`
   `link()` hardcodes `ld.lld` and ELF today, so it must learn to select these. Every
   instruction must link as written: on AArch64, `ld.lld` rewrites `adrp` + `add :lo12:` pairs
   into `nop` + `adr` unless it's given `--no-relax`, which would break the line-for-line match.
2. **Decoder** — a decoder for the CPU's machine code (today iced-x86 for x86-64 and lengua's
   own for AArch64, both reading ELF).
   For a new CPU, lengua decodes exactly its dialect's subset itself, so anything outside it is
   refused by construction, and tests that decoder against LLVM's disassembler.
3. **Prover model** — the CPU's instructions in `src/prove/exec`, and CPU-feature detection.
4. **Dialect** — the Power of 10 in that CPU's mnemonics and registers, with its own reference
   beside `x86_64.md`.
5. **The OS edge** — a file for the OS and CPU, `src/edge/<os>-<cpu>.s`, holding `_start` and the
   21 operations of [the edge](edge.md), `os_read` to `os_exit`, written for the OS. On Linux each
   is a system call with a few instructions around it: the numbers, the instruction and registers
   that make it, and the ABI's details, which differ between CPUs even on one OS (on AArch64:
   no `poll`, only `ppoll`; `clone`'s child-tid argument in another place; a 128-byte
   `struct stat` with `st_mode` at 16 rather than 144 and 24; `O_DIRECTORY` = `0o40000`, which is
   x86-64's value for `O_DIRECT`). **macOS and Windows don't allow direct syscalls:** their edges
   call the system library (libSystem; kernel32 and ntdll) instead. The operations' names,
   arguments, errors and results are `src/edge/mod.rs`'s and the same everywhere, so no program
   changes; `assert_fail`, `fill` and `parallel_for` are written once per CPU on top of them, and
   `tests/edge.rs` holds every operation to one table of cases on every target.
6. **The runner, the broker and the sandbox** (the per-host part) — the launcher that confines a
   program and starts it (on Linux seccomp, Landlock and resource limits, `os/linux.rs`), and
   the broker (`src/broker.rs`) that serves the program what a sandbox with no path leaves it:
   every name opened beneath its folder without following a link, held to what every system takes;
   the tests' clock and random bytes; and notes of its waits and listings. It talks to the
   program's edge over a channel the program inherits at descriptor 4,352. macOS and Windows each
   need their own confinement and their own channel. The network needs none of a sandbox's own
   policing: lengua's runner makes each connection a spec names and hands the program its end of
   a local stream (a Unix socket pair on Linux, from descriptor 4; a pipe or socket handle on
   Windows), as it binds a server's port, so a target's sandbox only has to keep the program off
   the network altogether. macOS also needs the binary code-signed (ad-hoc via `ld64.lld`) before
   it will run.

## The baseline

A profile also fixes the target's **baseline feature set**: the CPU extensions every machine of the
target has. `CpuFeatures::baseline(profile)` names it, and the `match` on `Profile` stops compiling
until a new target has one. Choose the lowest set that every machine the target's OS runs on has,
and write down why:
- Linux on x86-64: SSE2, which every x86-64 CPU has, with SSE3, SSSE3, SSE4.1, SSE4.2 and POPCNT.
  Windows 11 24H2 requires SSE4.2 and POPCNT, and every GitHub runner and every Intel Mac has
  them. BMI1, BMI2, LZCNT, AVX, AVX2, AVX-512, AES-NI, SHA and PCLMULQDQ are not in it.
- Linux on AArch64: ARMv8.0-A with NEON and floating point, and nothing optional: no CRC-32, PMULL,
  AES, SHA-1 or SHA-2, LSE atomics or SVE. The ports to macOS and Windows on ARM64 will fix theirs
  from what all of those chips have, with the owner.

Every program lengua ships for a target has a form that runs on its baseline, so no behavior of a
spec depends on an optional extension, and a computer without AVX-512 or SVE has every program to
learn from and copy. An extension beyond the baseline is a **fast tier**: a program of its own,
named for the extension (`wc-sve` beside `wc`), beside its baseline sibling and from the very same
`spec.md`, so the same tests and proofs hold for both. The agent picks the tier from the `machine`
line's features, and the baseline form on any other computer. `tests/baseline.rs` holds a new
target to it from its first program: it rule-checks every shipped `main.s` of every target in
`Profile::ALL` against that target's baseline, and a program that needs more must be listed as a
tier, with its sibling written, or as a debt, with its sibling planned (`DEBT`, and todo.md's
"Baseline debt"); the test fails for a program that needs more and isn't listed, and for a listed
one that now passes, so the debt only shrinks.

## One product on every target

A port adds a CPU's or an OS's means, never a different lengua. The same spec means the same on
every target: the same skills, tools, reports and messages; the same rules, memory safety, tests
and proof engine; and the same limits on every target. The capabilities a spec can name, and what
the sandbox allows of each, are one definition. Every limit one target has, from its OS or its
CPU, is every target's, whatever their assembly or their OS's permissions: a limit being anything
a spec or its user can observe, what a program may do, how it behaves, what it's refused and
when, and what lengua promises of it. No target is excused a limit because something else of its
own, lengua's checks included, could keep the same promise there: where one OS can't keep a
promise the way another does, every target takes on its limit, and the capability is reshaped so
every OS keeps it the same way, or that part is cut on every target, Linux too. Only the hardware
may differ: its instructions, its vectors' width, its speed, and never so that a spec's behavior
depends on an optional extension (the baseline, above). Every program has a baseline form that
runs on every machine of its target (x86-64's SSE2 with SSE4.2 and POPCNT, ARM64's NEON), and an
optional extension (AVX-512, SVE) is only a named tier beside it, picked from the machine line
and meeting the same spec, tests and proofs. A target is done when it covers what x86-64 covers,
each item with its own hardware's means.

## Order of targets

- **Linux ARM64 is done**, and is the pattern for the rest: the same OS, so the same kinds of
  system calls, the same sandbox (seccomp, Landlock) and ELF; its decoder, prover model, dialect
  and triple (2, 3, 4) are its own, and the OS edge (5) changes in detail. Tested under QEMU on an
  x86-64 Linux computer for everything but the sandbox's exact filter, and on an ARM64 computer or
  CI runner for that. It covers what x86-64
  covers, checked, tested and proven as x86-64's code is: arguments, whole numbers, floating
  point, vectors (NEON, and SVE's scalable bytes), text, standard input of any length through
  `fill`, files and folders, hash tables, sorting, servers and clients, the terminal, threads,
  and the CPU's CRC, carry-less multiply, AES and SHA instructions ([its
  reference](aarch64.md)); its fast `wc` is proven for every length with a 64 KiB buffer,
  as x86-64's is. Each example and fixture has its twin on the other CPU, compiled from the very
  same spec (`every_program_has_its_twin_on_the_other_cpu`, `tests/every_program.rs`).
- **macOS (ARM64, x86-64)** and **Windows (x86-64, ARM64)** need all six, the OS edge and the
  sandbox most of all: their edges call the system library where those systems forbid direct
  system calls, and each confines a program by its own means.
