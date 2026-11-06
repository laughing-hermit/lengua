# lengua's roadmap: two targets, one product

lengua has two targets, Linux on x86-64 and Linux on ARM64. This roadmap began as the plan for six
(macOS and Windows on each CPU too). On 2026-11-06 the owner dropped macOS on x86-64, since Apple
no longer makes Intel Macs, and then decided that the native macOS and Windows ports wait: Windows
and Mac reach lengua through its image, run by Docker or Podman (todo.md, "Round 3" and
"Deferred"). What this file holds is the way to the finish line set on 2026-11-05: lengua
*perfected*, a replacement for and an improvement on C, C++ and Rust, faster and leaner than them,
the same product on both targets. The finish line itself, and what is left, are in
[todo.md](todo.md). This file holds:
- the decisions the rounds rest on;
- the design: the OS edge, the broker, the server's OS seams, the toolchains, the sandbox, CI and
  the performance gate, with the macOS and Windows parts kept as the design of the deferred ports;
- each round's steps and the runs that verify it.

As a round lands, its part moves into [DESIGN.md](DESIGN.md) and leaves this file.

Decided with the owner (2026-11-04):
- **A shared edge per OS.** A program's text is written once per CPU, whatever the OS.
  Every operation that reaches the OS (read, write, open, the clock, threads...) is a call into a
  small canonical edge, one per target, which lengua supplies, assembles after main.s and checks
  in the executable line by line, as it does its own `assert_fail`, `fill` and `parallel_for`.
  Linux programs moved to it too (round 2).
  - So a new OS costs one edge file per CPU, not a new text for each program.
  - Each edge is tailored for speed: raw system calls on Linux, and, if the ports return,
    libSystem's thinnest wrappers on macOS and kernel32 on Windows. CPU code stays tailored per
    CPU (SSE2/AVX-512, NEON/SVE).
- **Unsafe code.** lengua's crate keeps `unsafe_code = "forbid"`. OS facilities come from
  maintained crates with safe APIs; where none exists, one small separate binding crate in the
  workspace holds only those calls, reviewed and tested.
- **Session suites.** The session suites for the new targets stay with the owner, because real
  Claude Code sessions need a logged-in Claude and cost usage.
- **Per-target speed.** "Use whichever is best to maintain speed and efficiency per target":
  instructions tailored to each target are accepted.

A design agent drafted this roadmap and an adversarial reviewer attacked it (2026-11-05). The
reviewer's 18 problems are fixed in place, and the claims they rest on were checked against the
code.

## Ground rules for every round

- **Runners only.** Every `cargo fmt`, clippy, build, test, proof, mutation run and benchmark
  runs in GitHub Actions. Work goes on a `wip/<round>` branch (no push runs there by design);
  runs are dispatched with filters; formatting comes from the format job's patch; a round merges
  to main only when its runs are green, and each run's id is the evidence in todo.md.
- **One product, no redundancy.** Each operation, rule, limit, test case and doc paragraph is
  written once. A target supplies only its cells, and a missing cell breaks the build. Tables are
  keyed by what a cell depends on: the CPU (`PerCpu`), the OS (`PerOs`) or both (`PerTarget`).
  References and assets are split the same way, never copied.
- **Product shape.** Skills (markdown) plus `lengua-mcp`; no hooks, no subagents in the
  product; on every host the server is the same binary's code built natively.
- **Commits** per part, noreply email, the session trailer; pushes publish (the repo is public).
- **Safety.** Stopped topics stay out; a new stop ends that item and is recorded in todo.md,
  never routed around through other models or subagents.

## Decisions taken under the owner's rule

These were taken on 2026-11-05 under the owner's rule ("whichever is best for speed and
efficiency per target"), and the owner approved them with the plan. Each is the owner's to
reverse. Since 2026-11-06 only Linux is built, so the decisions about macOS and Windows wait with
the ports.

1. **A broker replaces the ptrace supervisor on every OS.** One Rust service holds the name
   rules, plus the clock, random bytes, sleeps and waits in tests. Under lengua (tests and `run`),
   a name operation costs one round trip, about what a ptrace stop cost before round 2. A program run on
   its own calls the OS directly, as today.
2. **Names match byte for byte on every OS**, including macOS's and Windows's case-insensitive
   disks. A name that matches a file only when case or Unicode normalization is ignored gets
   ENOENT.
3. **Listing order stays unpromised.** Tests turn each listing round, as today. Listings leave
   out `.` and `..`.
4. **One closed error set**, Linux's numbers, on every OS. An error outside an operation's list
   is EIO.
5. **A write past a test's file limit fails with EFBIG** on every OS, because Windows can't kill
   a process for a file's size the way Linux's SIGXFSZ does.
6. **Listening ports start at 1024** on every target, because Linux refuses lower ones to users.
7. **The environment is at most 32,767 UTF-16 units** on every target. That is Windows's limit,
   and it gives the edge's buffer a size.
8. **A function's frame is under 4,096 bytes** on every target, because Windows grows a stack a
   page at a time. The shipped programs' largest frame is 40 bytes, and large memory is static,
   sized from the spec.
9. **After a call that reaches the OS**, the vector registers and each CPU's few scratch
   registers must be written before they are read again. Programs reload vector constants after
   a read, a few loads per 64 KiB, which is cheaper than saving AVX-512 or SVE state on every
   call.
10. **assert_fail, fill and parallel_for leave main.s** for lengua's linked library, so the ~95
    copies go.
11. **Names and minimum versions:** the targets are named `macos-aarch64` and `windows-aarch64`.
    Programs are built for macOS 11 and Windows 10 1809 or later. The documents say which
    versions the runners actually test: macOS 26 and 15, Windows Server 2025 and Windows 11.
12. **Test input of up to 4,096 bytes is in its pipe before the program starts.** POSIX's read
    rule then makes "one read returns it whole" true on every OS, so proofs keep 4,096 everywhere.
    If the Windows spike shows Windows splits it, every target uses the size Windows keeps whole.
13. **The QEMU path stays**, so an x86-64 Linux computer can test an ARM64 build, and it gets a CI
    job of its own.
14. **Release archives are unsigned**, which costs nothing. Apple ARM64 executables get the
    ad-hoc signature macOS requires.
15. **Fail closed, and gate macOS's sandbox on the newest macOS.** Seatbelt, macOS's only sandbox
    for command-line programs, is deprecated, and its rules aren't documented.
    - On every OS, the launcher confirms the confinement is in force before it starts a program,
      and otherwise refuses with a reason.
    - The macOS test job runs on the newest image GitHub has made generally available (macos-26).
      A change Apple makes then stops runs at once, rather than quietly weakening them.

## Design

Only Linux is built. What follows about macOS and Windows is the design of the native ports the
owner deferred on 2026-11-06, kept so that they can come back (todo.md, "Deferred"); what Linux
has is in [DESIGN.md](DESIGN.md).

### 1. The edge

The edge's OS-neutral design and its Linux build are in [DESIGN.md](DESIGN.md) ("The edge",
"Running programs" and "Porting a target"): 21 operations defined once, one closed error set,
the same data on every OS, one calling convention per CPU, lengua.s assembled after main.s, and
the checker's two units. This section keeps what macOS (round 3) and Windows (round 4) will do
differently or add.

**Sources.**
- Two edge files exist, `src/edge/linux-x86_64.s` and `src/edge/linux-aarch64.s`, beside the two
  OS-neutral CPU libraries, `src/edge/x86_64.s` and `src/edge/aarch64.s`. Three more come:
  `src/edge/macos-aarch64.s` in round 3 and `src/edge/windows-{x86_64,aarch64}.s` in round 4.
  Each is embedded with `include_str!` and gets its arm in `edge::files`.
- Each OS fact is written once, in its edge file: macOS's errno table, and Windows's offsets and
  each import's DLL. The `.tbd` and `.def` import lists are read from the edge's undefined
  symbols.

**Calling the library.** A program on macOS or Windows reaches the OS only through the system
library, so each operation of their edges calls it, through the GOT on macOS and the IAT on
Windows, with no stubs or thunks, and turns what comes back into the closed error set.
- macOS's `errno` and Windows's `GetLastError`/`WSAGetLastError` map through tables in their edge
  files. A test requires each table to cover the whole set.
- The edges save what their C ABI may change but the calling convention keeps:
  - on x86-64 (Windows alone), rdx and r8–r10;
  - on AArch64, x1–x7 and x9–x15 (d8–d15 need no saving: every OS's C ABI keeps them).
- They realign the stack, add Windows's 32-byte shadow space, and run `vzeroupper` before calling
  the library, on x86-64 with AVX.

**Data.** Each OS's own structures (`stat`, `O_*`, directory entries, termios, Windows's) live only
in its edge file, and a program sees the same bytes on every OS.
- Windows translates `os_poll`'s entries to WSAPOLLFD and back.
- Windows translates console input as a POSIX terminal gives it: CR becomes LF. In line mode CRLF
  becomes LF, and a Ctrl+Z line is the end of input.
- On macOS and Windows, an absolute `os_sleep` is a repeated relative wait.

**The checker** holds the two units and the rules on main.s as it does for Linux (DESIGN, "What
the checker holds"). For the new formats it also:
- lets only lengua's unit call the library, and refuses any import beyond the linked operations'.
  On Mach-O that means only libSystem, every GOT slot an allowed import, and no `__stubs`; on PE,
  only the linked operations' DLLs and functions;
- holds the new edge files to the two tests that keep lengua's text to the dialect, with calls
  into the library added to what only lengua's text may do.

**Start, exit, threads, terminal, descriptors, pipes.**
- *Start.* `_start` calls `main(argc, argv, envp)` with the three in the first argument
  registers, as on Linux.
  - On macOS, dyld calls LC_MAIN's `_start` with them already in place.
  - On Windows:
    - the edge splits `GetCommandLineW` by the MSVC rules, which `runner::arguments_length`
      already models;
    - it converts each argument with `WideCharToMultiByte(CP_UTF8)`;
    - it takes the environment from `GetEnvironmentStringsW`, leaving out the `=X:` entries and
      SystemRoot (which the launcher adds for Winsock);
    - it holds them in static buffers, at most 98,301 bytes;
    - it calls `WSAStartup` when a network op is linked.
- *Exit.* `_exit` on macOS, and `TerminateProcess(GetCurrentProcess(), status)` on Windows,
  because `ExitProcess` would run DLL detach code while the workers die.
- *Threads.*
  - macOS uses `pthread_create`/`pthread_join` with 1 MiB stacks.
  - Windows uses `CreateThread` with 1 MiB and joins with `WaitForSingleObject`.
- *Terminal.*
  - macOS: tcgetattr, tcsetattr and ioctl. A key wait uses `select`, because `poll` fails on
    terminals there.
  - Windows: Get/SetConsoleMode with VT enabled, GetConsoleScreenBufferInfo, and a wait on the
    console handle.
  - `run` restores the mode on every OS.
- *Descriptors.* On Windows the edge keeps a table of 4,352 slots {kind, read handle, write
  handle}, filled lowest free first.
  - Slots 0–2 come from `GetStdHandle`.
  - The listener (3), the connections (4 and up) and the channel (4,352) come from STARTUPINFO's
    `lpReserved2`, which the launcher writes in the MSVC CRT's format.
- *Pipes.* The runner writes a test's standard input into its pipe before the program starts, as
  on Linux (decision 12). On Windows the pipe is a byte-mode pipe, sized larger than 4,096 bytes,
  and the R1 spike found that one read returns a 4,096-byte write whole.

**The prover** needs no model of its own for an OS: `Cpu::edge_call` answers each operation by the
contract every edge keeps. The thread operations are the one exception. On Linux the prover runs
`os_thread_start` and `os_thread_join` as the code they are, with its clone and futex model. On
macOS and Windows that code calls `pthread_create` or `CreateThread`, which the prover can't step
into, so it will model the two operations whole: `os_thread_start` runs the worker, and
`os_thread_join` returns its result.

### 2. The broker, in place of the ptrace supervisor

The broker's messages and parser (`src/broker.rs`) are OS-neutral, and the Linux service
(`src/broker/linux.rs`) is built; both are in [DESIGN.md](DESIGN.md) ("Running programs"). macOS
and Windows each add a service of their own beside it, on the same messages.

- **The channel.** A program inherits it at descriptor 4,352 on every OS: a socketpair end on
  macOS, and on Windows an inherited duplex named pipe, whose handle the launcher puts in
  `lpReserved2` (§1). An edge that finds no first message from the broker there calls the
  library itself, as a program outside lengua does.
- **Names.** Each service opens beneath the program's folder without following links, and a link
  or a way out is `EACCES` on every OS, as on Linux:
  - macOS: a walk with `O_NOFOLLOW`;
  - Windows: reparse-point checks, POSIX-semantics delete and rename, `FILE_SHARE_DELETE` and
    verbatim paths. Another process holding a file there gives an environmental EBUSY.
- **Handing a file over.** `SCM_RIGHTS` over the socketpair on macOS, and `DuplicateHandle` into
  the program on Windows.
- **The file limit.** On macOS the kernel enforces it as on Linux (round 2, below). Windows has no
  RLIMIT_FSIZE, so its edge counts the bytes it writes against the limit in the broker's first
  message, and gives the same two answers: a short write, then EFBIG.
- **No emulator.** Neither macOS nor Windows has Linux's QEMU case (testing another CPU's build
  under Rosetta 2 or Windows's x64 emulation is deferred in todo.md), so every program that has
  a channel gets no path from the sandbox (§5).

### 3. The server's OS seams

- **`src/os/`** has four parts:
  - `unix/`, shared by Linux and macOS: descriptor placement, rlimits, execve, wait4, restoring
    the terminal, forwarding signals, socketpairs and SCM_RIGHTS;
  - `linux/`: Landlock, seccomp and perf counters, in `src/os/linux.rs` since round 1
    (`launch_job`, `confine` and the pins);
  - `macos/`: Seatbelt and sysctl;
  - `windows/`, built on `crates/lengua-os`.
- **Host facts.**
  - CPU time and peak memory come from rusage on Unix. On Windows they come from
    `QueryProcessCycleTime` and `PeakWorkingSetSize`, because the job's accounting only advances
    every ~15.6 ms.
  - `cpu_model` comes from /proc/cpuinfo, `sysctl machdep.cpu.brand_string`, or the registry.
  - The memory guard in prove/watch.rs reads each OS's own: /proc on Linux today, `vm_stat` and
    `sysctl` on macOS (round 3), `GlobalMemoryStatusEx` on Windows (round 4).
  - The data folder is XDG on Linux, `~/Library/Application Support/lengua` on macOS, and
    `%LOCALAPPDATA%\lengua` on Windows.
  - The key folder has mode 0700 on Unix, and an ACL for the user alone on Windows.
- **Keyed by target, never silently by CPU.**
  - `Profile::os()` and `Profile::isa()` are the only exhaustive matches, besides the name and
    the triple.
  - `Isa::abi()` stays per CPU.
  - `decode::read(bytes, profile)` reads the profile's format and refuses any other.
  - `supervise::AVAILABLE` went with supervise.rs. `Image::linux` and `syscalls::host` stay, as
    Linux's own.
  - `CpuFeatures::baseline` takes an `Isa`.
- **Profiles.** LinuxX86_64, LinuxAarch64, MacosAarch64, WindowsX86_64 and WindowsAarch64. The
  ARM64 ones are named `macos-aarch64` and `windows-aarch64`.
- **Tables.**
  - `PerCpu<T>` and `PerOs<T>` join `PerTarget<T>`. `PerTarget` keeps only what differs per
    CPU-and-OS pair: today `ELSEWHERE` in tests/sessions.rs.
  - The 288 cells of round 1 are each a CPU's text or count, so they are `PerCpu`. macOS and
    Windows then inherit every case.
  - Check-only tables run every target of the cell's CPU.
  - Prover tables run the host's CPU built for the host's OS; the other CPU's run only on the
    Linux runners.
  - Run tables run the host's target.
- **Crates.**
  - Linux only: landlock, seccompiler, perf-event2.
  - Unix: rustix, and nix (process, signal).
  - Everywhere: object (elf today; macho and pe come with rounds 3 and 4). cap-std too, if it
    opens files beneath a folder on all three OSes.
- **`crates/lengua-os`** is the workspace's only unsafe code, about 1,500 lines. Each call is
  wrapped in a safe function with a test on its OS's runner:
  - macOS: `sandbox_init_with_parameters`, with sandbox-exec as the fallback;
  - Unix: wait4 with rusage, unless nix or rustix offer it safely;
  - Windows:
    - CreateProcessW with STARTUPINFOEXW (handle list, security capabilities, lpReserved2),
      started suspended, put in the job, then resumed;
    - the LPAC AppContainer profile and SID;
    - job limits and accounting;
    - DuplicateHandle;
    - inheritable, non-overlapped sockets;
    - console modes and the code page;
    - CreatePseudoConsole, for tests;
    - QueryProcessCycleTime and GetProcessMemoryInfo;
    - SetErrorMode.
- **CPU features.** std's detection works on all three OSes.
  - SVE's vector length is read only on Linux.
  - Windows ARM64 reports no SVE for now (a todo item).
  - Performance counters exist only on Linux; elsewhere, reports say "not counted on this
    computer".

### 4. Toolchains and reading executables

| | link | libraries | a user needs |
|---|---|---|---|
| Linux | `ld.lld --fatal-warnings [--no-relax] -e _start`, static ELF | none | llvm and lld |
| macOS | `ld64.lld -arch arm64 -platform_version macos 11.0 11.0 -e __start -fatal_warnings`; dynamic, LC_MAIN, ad-hoc signed | a minimal libSystem.tbd lengua writes from the edge's imports | LLVM only (llvm-mc and lld, from conda-forge, Homebrew, or the release archive); no Xcode, no SDK |
| Windows | `lld-link /entry:_start /subsystem:console /nodefaultlib /dynamicbase /nxcompat /highentropyva /stack:0x800000 /Brepro /WX /debug:symtab` | kernel32 (+ ws2_32, bcryptprimitives when used), import libraries built by llvm-dlltool from `.def` files lengua writes, cached in the data folder | LLVM's Windows release archive (llvm-mc, lld-link, llvm-dlltool); no Windows SDK |

Triples: `{x86_64,aarch64}-unknown-linux-gnu`, `arm64-apple-macos11.0`,
`{x86_64,aarch64}-pc-windows-msvc`.

- **Lowering** (`src/toolchain/lower.rs`) goes line for line, so reports still name main.s's
  lines.
  - Mach-O:
    - a leading `_` on every symbol, with imports written under their C names;
    - `.type` and `.size` dropped, with each symbol's extent taken from the source;
    - `.Lx` becomes `Lx`;
    - `.rodata` goes to `__TEXT,__const`, and `.data` and `.bss` to `__DATA`;
    - `@PAGE`/`@PAGEOFF` on AArch64.
  - COFF: `.type` and `.size` dropped, and `.rodata` becomes `.rdata`.
  - The x86-64 dialect admits only RIP-relative symbol addresses, which is all the shipped
    programs use.
- **Reading back** uses checker/decode.rs with `decode/{elf,macho,pe}.rs`.
  - Functions and data come from the symbols: PE's from `/debug:symtab`, Mach-O's with their
    leading `_` removed. Where the format records no sizes, the sizes come from the source.
  - Memory that is both writable and executable is checked for in each format.
  - Read-only data, jump tables, the GOT and the IAT must not be writable at run time: Mach-O's
    maxprot or SG_READ_ONLY, and PE's section flags.
  - Every byte of the text section belongs to a function.
  - Imports are checked as described in §1.
- **Solvers.** z3 4.16.0 and cvc5 1.3.4 both publish native builds for Linux, macOS and Windows
  on both CPUs (checked 2026-11-05; cvc5's include `cvc5-Win64-arm64-static.zip`). Both are
  pinned by SHA-256 in `scripts/ci/tools.tsv`, which holds the Linux rows alone since
  2026-11-06.
- **LLVM on macOS and Windows in CI.** conda-forge publishes `lld` and `llvm-tools` 22.1.8 for
  osx-arm64, win-64 and win-arm64. So setup installs them there with micromamba (pinned by
  SHA-256), the same LLVM version the Linux jobs take from apt.llvm.org. Users' installs follow
  in R6, where shipping llvm-mc and lld in the release archive is weighed.

### 5. The sandbox on each OS

| limit | Linux | macOS | Windows |
|---|---|---|---|
| files only through the broker | Landlock, no path for a program that has a channel | Seatbelt deny-default, system libraries readable | LPAC AppContainer, no ACL |
| no network of its own | Landlock net + seccomp | Seatbelt `deny network*`, an inherited listener allowed | no capability |
| connections handed over (3, 4…) | dup2 + SCM_RIGHTS | same | handle list + the CRT table: sockets, or relayed pipes if the spike says so |
| 4,352 open files | RLIMIT_NOFILE 4,352 | same | the edge's table → EMFILE |
| time and output limits | runner | runner | runner + TerminateJobObject |
| no processes, no signals to others | seccomp, Landlock scopes | Seatbelt `deny process-fork`, `signal` | job: one active process |
| no core files, no dialogs | RLIMIT_CORE 0 | same | SetErrorMode, no WER, die on an unhandled exception, CREATE_NO_WINDOW |
| crashes as signal numbers | native | SIGBUS → 11 | access violation and stack overflow → 11, illegal instruction → 4, divide → 8, Ctrl+C → 2, ended by lengua → 9 |

Some behavior is reshaped on every target, the deferred ones too, so all keep one promise:
- EFBIG replaces SIGXFSZ.
- The terminal is two operations. On Linux they make the ioctls that `Pin::Terminal` allows, and
  the pin stays.
- `sendto` with MSG_NOSIGNAL folds into `os_write`; SIGPIPE is ignored.
- nanosleep's remaining time and accept's peer address are dropped.
- poll and ppoll become one `os_poll`, in milliseconds.
- Listening ports start at 1024; Windows binds them with SO_EXCLUSIVEADDRUSE.
- Names match exactly.

Two OS differences remain:
- **A per-call kernel filter exists only on Linux.** On macOS and Windows two things keep the same
  promises with one layer fewer: the checker guarantees that main.s reaches the OS only through
  the linked edge, and the OS sandbox does the rest (DESIGN, "Porting a target", item 6: macOS and
  Windows have neither seccomp nor Landlock).
- **Windows's socket handover depends on the R1 spike.** If an LPAC child can use inherited
  sockets, Windows hands them over as POSIX does. If not, the listener stays in lengua and
  connections become relayed pipes. The limits stay the same and the server is slower, which R5
  measures.

**Fail closed.** On every OS, the launcher confirms the confinement is in force before it starts
a program. If it isn't, `run` and the tests refuse, and say why.
- Linux: Landlock's ruleset status and seccomp's filter, as today.
- macOS: `sandbox_init`'s result.
- Windows: the token's AppContainer and LPAC flags, and the process's assignment to the job.

Seatbelt is deprecated and its rules undocumented, so macOS's sandbox tests gate on the newest
macOS image GitHub offers (§6). If Apple ever removes Seatbelt, the fallback to weigh is the App
Sandbox: entitlements on the ad-hoc-signed program. That fallback is in todo.md.

The tests that try to get past the sandbox on macOS and Windows are left for the owner (todo.md's
stopped list, 2026-11-05).

### 6. CI on the free runners

- **One workflow, `.github/workflows/ci.yml`.** targets.yml, arm64.yml and unported.yml are
  removed. Its jobs:
  - **format** (ubuntu-24.04): `cargo fmt --check`, with `fmt.patch` as an artifact.
  - **test**, on two runners: ubuntu-24.04 and ubuntu-24.04-arm. Each runs the whole suite in one
    job, which took 35 to 62 minutes on the six runners of run 42085247387. Shards were planned
    and left out: a shard split by test would break the proving lock, which is per process.
    Until 2026-11-06 the test job also ran on macOS and Windows; the ports' CI design is in the
    deferred round, R4.

    Each job:
    - runs `scripts/ci/setup.sh`: Rust 1.99.1, LLVM 22 and the solvers, each pinned in
      `scripts/ci/tools.tsv`;
    - writes the machine's facts into the run summary (`scripts/ci/machine.sh`);
    - runs clippy with `-D warnings`;
    - runs `cargo test --workspace`;
    - writes the skip summary, and keeps the logs, skip logs and benchmark results as artifacts.
  - **qemu** (ubuntu-24.04): the suite again with QEMU's user mode, so the ARM64 cells run on
    an x86-64 computer as they do on a developer's. It reports without failing the run until its
    failing cases are fixed (todo.md, round 2).
  - **golden-diff** (ubuntu-24.04, `golden` mode only): compares the head's and the base's
    recordings, raw or as the proofs' verdicts alone (`scripts/ci/golden-diff.sh`).
  - **gate**: depends on plan, format, test and golden-diff.
- **Runner facts** (2026-11-04). 4 vCPUs on Linux, kernel 7.0 (azure), Landlock and yama in the
  LSM list, `ptrace_scope` 1. The x86-64 pool is mixed, about half of it with AVX-512, and the
  ARM64 runner has SVE. Not recommended: lowering the proof budgets (tests assert verdicts);
  larger or self-hosted runners (outside the $0 rule); emulating AVX-512 (QEMU's TCG has
  none).
- **When it runs.**
  - Pushes to main run it, path-filtered; `wip/**` branches get no push runs.
  - Dispatch inputs: targets (both by default); mode (`compile` is clippy plus `cargo test
    --no-run`, about 10 minutes; also `test`, `ignored`, `golden`, `bench` and `release`);
    filter; test_files; check_folder; check_target;
    mutate_only; stall_seconds; seed; and, for `golden` and `bench`, golden_base,
    golden_base_run, golden_view and bench_against.
  - Every week: `test` and `ignored` on both. From round 3 the same run also covers the suite
    inside a default Docker container (the lengua image) on both runners.
- **Caches and actions.**
  - Caches hold the registry and a pruned target/ per runner, with LLVM and the solvers cached
    separately, all within 10 GB.
  - Actions use their Node 24 majors.
  - Runs cancel in progress only off main.
- **Line endings.** `.gitattributes` (`* text=auto eol=lf`) lands before the first Windows test,
  because GitHub's Windows images would otherwise check out CRLF. A spec's hash normalizes CRLF.
- **parity_inventory.**
  - R1 adds `scripts/ci/targets.json`: a runner label for each target, each needing a native
    `cargo test` job. parity_inventory and ci.yml's plan job read it.
  - DEBT is empty after R1.
- **How a round iterates.**
  1. Push `wip/<round>`.
  2. Dispatch `compile` on every target the change touches.
  3. Run the filtered tests, then the full suite.
  4. Take formatting only from `fmt.patch`, with `gh run download` and `git apply`.
  5. Merge when the runs are green, and put their run ids in todo.md.

### 7. The performance finish line

- **The bench** (benches/against_rust_and_c.rs) becomes portable.
  - lengua's versions become `PerCpu`, built by lengua's own build, with the edge.
  - C and C++ are built the way each OS's developers build them:
    - Linux: gcc/g++ `-O3 -march=native -flto`;
    - macOS: Apple clang `-O3 -mcpu=native`;
    - Windows: LLVM clang `-O3 -march=native` with the UCRT. A shared `benches/programs/io.h`
      gives the C programs a Windows path.
  - Rust is built as now.
  - A hidden `lengua-mcp --measure` does the measuring, replacing GNU time and the /bin/sh FIFO.
  - Each runner writes its own `BENCHMARKS-<target>.md`.
- **The empty program** is a workload like the others: lengua's main calls `os_exit(0)` and C's
  main returns 0, and lengua must start faster than C. Each runner also reports it as that OS's
  floor, the part of a startup-bound workload that no program avoids.
- **The gate** fails on any workload and measure where the one lengua program for the machine
  isn't ahead of the best C, C++ and Rust on that measure:
  - *CPU and wall time.* The runs alternate between the languages, so drift hits all alike.
    lengua is ahead when its median is lower and a one-sided Mann–Whitney U test gives p < 0.01.
    Until the test decides, the bench adds runs, up to 2,000 per language or ten minutes a row.
    Still undecided at the cap counts as behind.
  - *Peak memory and executable size.* lengua's median must be smaller, as measured.
  - *One program against the best of the others.* lengua's side is one program per workload and
    machine: the fastest tier the CPU supports, as the compile skill picks it, named in the bench's
    table. It must be ahead on all four measures at once. The others are taken at their best on
    each measure, so a different program for each: vector code, and threads wherever any
    language's best is threaded (C with pthreads or Windows threads, C++ with `std::thread`,
    Rust with scoped threads) for time, the single-thread version for memory. A row where no tier
    wins all four is behind, and `KNOWN_BEHIND` is the work of making one do it (threads with lean
    stacks, tables that stay small).

  `KNOWN_BEHIND` (target, workload, measure, why) is round 5's work list. It may only shrink, like
  tests/baseline.rs's ratchet, and it is empty at the finish line.
- **A risk, stated plainly.** Some costs every language pays alike: macOS's dyld and libSystem
  start, a pipe's copy, an open, libSystem's clock, and executable sizes that round to a page.
  Where one dominates a row, lengua's lead is only its own few instructions, and the row needs
  the most runs or may tie. A row that stays undecided keeps the finish line open and goes to
  todo.md for the owner; it is never relabelled as level.
- **New workloads.**
  - empty;
  - sort: 64 MiB of lines, in byte order;
  - word tally: 64 MiB, printing the ten most common words;
  - a server: 64 clients × 1,000 requests of 64 bytes, and again with 4,096 clients;
  - many files: 10,000 opens, run on its own and under `run`;
  - the clock: a million readings.
- **Known gaps**, each closed by an A/B comparison within one run.
  - crc32c's wall time on both CPUs: process each read as it comes, instead of waiting for fill's
    1 MiB. ARM64's twin uses 64 KiB.
  - x86-64's histogram: more tables, unrolling.
  - The SSE2 16-thread shout: a one-thread SSE2 shout, to match neon-one.s.
  - `move_front` copying 16 bytes a step.
  - Measure count/sve.s, and a threaded ARM64 shout.
  - Read the Linux clock through the vDSO, as C's `clock_gettime` does.
  - An event-queue op (epoll, kqueue, IOCP) for the 4,096-client server: C servers use them, so
    the gate needs lengua to have them too.
  - Every row the first gate run shows level or behind joins this list.

## Rounds

### R0 — documents (done 2026-11-05)
- todo.md is rewritten: the finish line, the roadmap, every open item under the round that takes
  it, the owner's items, the deferred ones, and the stopped list.
- README's benchmark claim says what BENCHMARKS.md shows.
- DESIGN's performance bar and workload count match the finish line.
- This file is added.

Nothing ran, since R0 changed documents only; the commit is the record.

### R1 — six hosts, one workflow, the spikes, the soundness items

Round 1 built and tested all six hosts. On 2026-11-06 macOS on x86-64 was dropped and the native
macOS and Windows ports were deferred, so its answers below are the record of what was found, and
two hosts remain.

**Goal.**
- lengua-mcp builds, passes clippy and runs `cargo test --workspace` natively on all six
  runners.
- Linux's results are unchanged.
- On macOS and Windows, every test that runs no program passes; the rest skip with a note.
- The spikes have answered.
- The Linux soundness items are fixed before R2 takes its ledger.

**Steps.**
1. **The first dispatch runs the spikes.** They are `crates/lengua-os` tests (ignored; dispatched
   with mode=ignored, filter=spike), so they run before lengua's crate builds on macOS and
   Windows. Their answers go into §1 and §5 of this file, and into todo.md, before R2.
   - **macOS.** The spike program is a ten-line `_start` → main → write/exit program:
     - built with llvm-mc, ld64.lld and a generated tbd;
     - calling through the GOT;
     - with a lowered `.rodata`, a `.bss`, a relative table, and symbols that start with `l` and
       `L`.

     It checks:
     - a socketpair end at fd 4, and a descriptor received by SCM_RIGHTS;
     - a crashing child: ReportCrash's delay, and SIGBUS;
     - a pseudo-terminal;
     - whether a stored name's bytes can be read back.
   - **Windows.** The same program, through lld-link with `/debug:symtab` and a dlltool-built
     kernel32.lib. It checks:
     - a handed-over pipe, connected loopback socket, listener and DuplicateHandle'd file, and
       which of ReadFile, WriteFile, recv, send, accept, WSAPoll and shutdown work on each;
     - that GetCommandLineW and the lpReserved2 table arrive;
     - a chain of frames under 4 KiB on a guard-paged stack;
     - an empty environment with WSAStartup;
     - tick-based CPU time against cycle time;
     - commit and working set for a large `.bss`;
     - whether a 4,096-byte write is read whole from a byte-mode pipe.
   - The spikes' checks of what the sandbox refuses are left for the owner (todo.md's stopped
     list, 2026-11-05).

   **First answers (2026-11-05, run 42068120455, all six runners).**
   - *The toolchain makes a runnable executable on each system with LLVM's tools alone, no SDK.*
     - macOS, both CPUs: llvm-mc 22.1.8, `ld64.lld -platform_version macos 11.0 11.0 -e _main`
       and a hand-written `libSystem.tbd` (`_write`, `__exit`) link a Mach-O that dyld runs, which
       writes "hello" through the GOT and exits 0. `ld64.lld` signs ARM64 output ad hoc itself:
       no `codesign` step was needed.
     - Windows, both CPUs: `llvm-dlltool -d kernel32.def` makes the import library, and `lld-link
       /entry:_start /subsystem:console /nodefaultlib /debug:symtab` links a PE that runs and
       calls GetStdHandle, WriteFile and ExitProcess through the IAT.
   - *What the macOS linker lays down* (ARM64; x86-64 is the same with 4 KiB pages): `__PAGEZERO`,
     `__TEXT` (`__text`, `__cstring`), `__DATA_CONST` holding `__got` with the read-only flag
     `SG_READ_ONLY` set, `__LINKEDIT`; load commands `LC_MAIN`, `LC_LOAD_DYLIB
     /usr/lib/libSystem.B.dylib`, `LC_FUNCTION_STARTS`, `LC_DATA_IN_CODE`, `LC_CODE_SIGNATURE`. So
     the checker can hold the GOT read-only by its segment flag, as §4 plans. The executable is
     33,408 bytes on ARM64 (two 16 KiB segments and 640 bytes of `__LINKEDIT`) and about 8.5 KiB
     on x86-64: a floor a C program shares, so a size lead there comes from `__LINKEDIT` alone.
   - *A fault ends the program by SIGSEGV (11), at once.* A write to address 0 ended by 11 on both
     CPUs in 3 to 4.5 ms, shorter than a program that exits (6.8 ms, its first run): no crash
     reporter delay shows. (The SIGBUS to 11 mapping stays in the table for the faults that raise it.)
   - *Names.* A directory listing gives back the bytes a name was stored with, on macOS and
     Windows (composed `é`, c3 a9, listed as c3 a9), so the broker can compare names byte for byte
     (decision 2). What a lookup folds differs: both systems find `README` for a file made as
     `Readme`, and creating `readme` over it leaves one entry; macOS finds the composed file by
     its decomposed spelling, Windows doesn't. So a lookup alone can't say whether a name was
     spelt exactly: the broker reads the stored name from the listing.
   - *A pipe.* On macOS and Windows alike, one read of a 64 KiB buffer after one 4,096-byte write
     gets all 4,096 bytes, and two 1,000-byte writes come back as 2,000 in one read: decision 12
     holds without the fallback size.
   - *Still to answer:* a socketpair end at fd 4 and a descriptor by SCM_RIGHTS, and a
     pseudo-terminal, on macOS; on Windows the handed-over handles, `lpReserved2`, a deep frame
     chain, an empty environment with WSAStartup, cycle time against ticks, and a large `.bss`'s
     commit. They need the program to run with descriptors and handles placed for it, which
     round 3 and round 4 do; the answers are asked for again there.
2. **The build.**
   - Add `.gitattributes`.
   - In Cargo.toml: the Linux-only crates go under `cfg(target_os = "linux")`, rustix and nix
     under `cfg(unix)`. object's macho and pe features come with `decode/macho.rs` (round 3) and
     `decode/pe.rs` (round 4); sysinfo is not added.
   - Make a workspace with `crates/lengua-os`.
3. **`src/os/`** (§3). Done, with these differences from the plan (2026-11-05):
   - runner.rs's launch half moved whole into `src/os/linux.rs`, and `src/os/unsupported.rs`
     gives every other system the same four things, saying programs don't run there yet. The
     `unix/` half macOS shares is split out in round 3, when macOS shows what it needs.
   - supervise.rs is gone (round 2). The /proc probe in testrun.rs stays, for a program that has
     no broker.
   - connect.rs: the Unix socketpairs and the relay are Unix's; `Peer` is a type with no values
     on Windows.
   - prove/watch.rs did **not** move onto sysinfo: its memory guard still reads Linux's /proc,
     and its tests say so where it reads nothing. Round 3 reads macOS's (`vm_stat`, `sysctl`) and
     round 4 Windows's (`GlobalMemoryStatusEx`), without a new dependency; a solver is stopped on
     Windows by `taskkill`.
   - The toolchain's PATH search honours `.exe`, with an install hint per OS; the data folder is
     each system's own.
   - smt.rs's `#!/bin/sh` fake solver is still a script: its two tests skip with a note where
     there is no shell (making it the test binary is left over).
4. **Keying by target.** `Profile::isa()` and the rest of §3's keying (`Profile::os()` waits for
   round 3, step 5); `CpuFeatures::baseline` taking an `Isa`; the `-aarch64` names.
5. **Tables.**
   - `PerCpu` goes in src/targets.rs, with `PerTarget` kept for what differs by system too.
     `PerOs` and `Profile::os()` wait for round 3, when a second system's profile makes them
     needed.
   - The 288 cells are converted mechanically.
   - `every_prover_test_runs_on_every_target` (src/prove/tests.rs) takes `PerCpu`.
   - Host-bound tests use `targets::host()` and `skiplog::note`.
6. **One folder check.** `check_one_aarch64_folder` folds into `check_one_folder(target)`, which
   keeps the QEMU path. DEBT is then empty.
7. **CI.** ci.yml and `scripts/ci/{setup.sh,tools.tsv,targets.json}` (§6); the three old
   workflows are deleted; parity_inventory reads targets.json.
8. **Soundness items on Linux**, so that R2's ledger rests on a sound prover:
   - the AArch64 vector model's wildcard: an unmodelled op says "doesn't model yet", as x86-64's
     does, instead of running as zeros;
   - `cbz`/`tbz` on xzr/wzr no longer get past the dead-code and "assertion always fails" checks;
   - SVE state across `svc` is cleared, as Linux clears it;
   - `prove_all`'s done-before-lock window: it writes `[proofs]` before the last line goes final;
   - the prover's time budgets are scaled by a measured speed probe instead of the vCPU count, so
     runners of different speeds reach the same verdicts.

**Reuse.** `PerTarget::cells` (src/targets.rs), `skiplog::note`, scripts/skip-summary.sh and
stall-report.sh, and `Toolchain::assemble/link` (src/toolchain.rs).

**Verified by.**
- `compile` is green on six runners.
- `test` is green on six. Linux passes at least main's last counts (530 and 526). macOS and
  Windows note every skip.
- qemu and parity_inventory are green.
- The spike logs are recorded.

**Size and risks.** Size L, about 3k lines, mostly mechanical.
- Windows will take 2–4 compile round trips (std::os::unix, tokio's unix APIs).
- Tests that assume Linux will need skips.
- The ARM64 macOS runner has only 3 vCPUs and 7 GB.

### R2 — the edge and the broker, on Linux (both CPUs): built 2026-11-06

What was built is in [DESIGN.md](DESIGN.md): "The edge", "Running programs" (the sandbox and the
broker), "Platforms" and "Porting a target". The runs that show it, and what the round still has
to verify, are in [todo.md](todo.md). The plan's OS-neutral parts left §1 and §2 when it was built.
The build differs from the plan in these ways:
- **The system-call models stay.** The prover keeps `Cpu::syscall`, `Call`, `Convention`,
  `Image::linux` and its raw `_start` entry, and the spec keeps `Spec.syscalls` and
  `Use::syscalls`, which still give the sandbox's allow-list through `Profile::allowed_syscalls`.
  The plan had them deleted, `syscalls.rs` with them; it stays too, with its header-pinning
  tests, which the plan had become one test. Tests hold the spec's
  call lists and memory.rs's per-call buffers to the operations' own (`Op::allowances`,
  `Op::syscalls`, `Op::lends`), and round 6's prover parity restructures the models (todo.md,
  "Decided while building").
- **A write past a test's file limit** is cut short and then fails with EFBIG by the kernel
  itself: the launcher sets RLIMIT_FSIZE and ignores SIGXFSZ, through `lengua-os`'s first
  function. macOS will do the same, and Windows's edge will count the bytes instead (§2).
- **The thread operations are `parallel_for`'s alone.** The checker refuses a program's own call
  of `os_thread_start` or `os_thread_join`.
- **Only the edge form is accepted,** with no ratchet list: text that defines `main` and not
  `_start` is a program (`edge::is_program`), and the checker refuses any other. The raw probes in
  `tests/limits.rs` stay raw by purpose; the snippets that tested the inline form's rules went
  with the form.
- **The broker serves only what the program's spec allows:** names, the clock and random bytes.
  It answers them outside the program's confinement, so the kernel can't hold the program to its
  spec for them, and the broker does.
- **The sandbox.** seccomp's allow-list comes from the spec as before, not from the linked code,
  plus `sendmsg` and `recvmsg` on descriptor 4,352 alone. Landlock grants no path to a program
  that has a channel, which is one whose linked code uses names, the clock, random bytes, sleeps,
  listings or waits, in a run too. A program without one keeps its working folder, and so does a
  program QEMU's user mode runs.
- **The layout** (one `spec.md` per program, the assets split per CPU, `tests/fixtures/aarch64`)
  moves to round 6.
- **The programs' batch** is done as far as this: every shipped program, template and benchmark
  is in the edge form, and the bench builds through the edge. Not built, and still open:
  upcase's retries (4,096 writes on x86-64, 64 on ARM64) and the ARM64 template's skipped
  assertion 3; the header's stamp, which still names the target and lists the baseline in
  `needs`; MACHINE_WORDS' Windows words and AArch64's registers; the model-against-CPU harnesses,
  which still build raw `_start` programs; and a broker for the bench's `run` lines.

### R3 — reach: the image first

**Goal.** A person on Windows, on a Mac or on an older Linux can use lengua through one route that
CI tests: the lengua image, run by a working Docker (a `docker` command talking to an engine, from
any provider; Podman is supported), which is a Linux with the same two targets inside. A Linux
that meets the floor needs no container and runs lengua directly. The route is seamless: with
Docker or Podman installed, installing the plugin is the only step. A default container, on any
host, is the floor of the product: what it doesn't give on every host, lengua doesn't need.
todo.md's "Round 3" holds the questions the plan must settle, and an Opus plan makes these
candidate steps executable.

**Steps.**
1. **The lengua image.** Ubuntu 24.04, pinned by digest: the userland CI's native jobs run, so
   the image's install steps are the ones `scripts/ci/setup.sh` already runs on the runners (LLVM
   22 from apt.llvm.org's noble repository, and the solvers pinned in `scripts/ci/tools.tsv`),
   plus lengua-mcp. One multi-arch image for linux/amd64 and linux/arm64, built and published by a
   workflow. Not Ubuntu 22.04 or Debian 12: their glibc (2.35, 2.36) is below the server's 2.39.
   Whether a native install takes the same pinned solvers, so that a proof's verdict doesn't
   depend on the route, is for the plan to settle.
2. **A container job.** `ci.yml` runs the suite inside the image on both Linux runners, under
   Docker's and under Podman's default seccomp profile (the runner has both). It is the test that
   lengua uses nothing a default container lacks: no io_uring, hardware counters optional. The
   image and the native jobs share one userland, so a difference between the two runs is the
   container's.
3. **The kernel floor and the Landlock policy.** Decide what the launcher requires: Landlock for
   every program, or a promise that rests on seccomp and the broker with Landlock as reported
   hardening; and the Landlock ABI the network needs (6 today, Linux 6.12; 4 would be Linux 6.7).
   A sandbox change gets a review and its tests (`tests/limits.rs`, the red team). A Mac is the
   test case: Docker Desktop's VM reportedly has no Landlock, and Colima's, Rancher Desktop's and
   Podman Desktop's are unchecked (todo.md, "Round 3", has the one-line check).
4. **The host launcher.** The command the plugin's `.mcp.json` runs on Windows and macOS (and on a
   Linux that doesn't meet the floor). On a Linux that does, it runs the native server when that
   is installed and `doctor` passes, and the image otherwise. It tries `docker`, then `podman`,
   pulls the image for the plugin's version, starts or reuses one container, mounts the project,
   maps host paths to container paths, keeps the signing key in a volume, stops the container
   when idle, and explains a failure in a sentence; when the Docker it found has no Landlock
   (Docker Desktop's Mac VM reportedly) the sentence names a VM that should have it. The two
   runtimes differ on a Linux host (a `:Z` label under SELinux; `--userns=keep-id` on rootless
   Podman, `--user` on Docker Engine). No confinement and no OS code, only plumbing; the server
   stays in the image.
5. **`lengua-mcp doctor`.** It prints the route (native, Docker or Podman), the kernel, the
   Landlock ABI, seccomp, glibc, the solvers' versions, the CPU's features and the counters, and
   says in words a person can act on what a route lacks.
6. **The container's own limits.** The proofs' memory guard reads the container's cgroup limit
   instead of the host's `/proc/meminfo`, and nothing needs a hardware counter.
7. **The documents,** and the other routes: WSL 2 with the agent inside it, a Linux VM (Podman
   machine, Colima, Lima), a dev container. Optionally the server on more Linux: static musl
   builds or an older-glibc build for both CPUs, chosen with the LLVM versions lengua needs;
   `scripts/install.sh` and `scripts/release.sh` follow.

**Verified by.**
- `compile`, then a full `test`, on both runners.
- The container job, green on both runners under both runtimes.
- Hand tests: the owner's Fedora, directly and under Podman; and Docker Desktop and Docker Engine
  inside WSL 2 on Windows, Colima, Rancher Desktop and Docker Desktop on a Mac, where someone has
  them, noted in todo.md (no free runner can run those).

**Size and risks.** Size M. The kernels under Docker Desktop, WSL 2 and the Mac VMs differ and some
have no Landlock (todo.md, "Round 3"); who tests the Mac and Windows routes by hand is open.

### R4 — native macOS and Windows ports (deferred 2026-11-06)

The owner deferred these ports when lengua became two targets: Windows and Mac are reached through
the lengua image, run by Docker or Podman (R3). What follows, the plans of the old rounds 3 and 4,
is kept so that they can come back (todo.md, "Deferred").

#### macOS, ARM64 (was R3)

**Goal.** macos-aarch64 passes the one suite natively, on macos-26, and also every week on
macos-15. (macOS on x86-64 was dropped on 2026-11-06: todo.md, "Deferred".)

**Steps.**
1. **Profile.** The profile, and `for_target("macos", …)`.
2. **Toolchain.** `Os::Macos`'s triple, ld64.lld, the tbd writer, and the Mach-O lowering (§4).
   The executable is named `main`.
3. **decode/macho.rs**, with the read-only checks.
4. **The edge file, `src/edge/macos-aarch64.s`.**
   - LC_MAIN's `_start`, imports through the GOT, register saving, and the errno table.
   - stat → Info. Its offsets are pinned by a test against the libc crate's stat on the runner,
     as `the_host_table_is_the_c_librarys` (syscalls.rs) does today.
   - getentropy, nanosleep, clock_gettime, pthreads, poll and select, accept, shutdown, termios,
     and `_exit`.
   - The broker, over sendmsg/recvmsg.
5. **`src/os/macos/`.**
   - Places descriptors 3, 4 and up, and 4,352, and sweeps /dev/fd for anything else.
   - Sets rlimits and ignores SIGPIPE.
   - Applies the Seatbelt profile, written once in os/macos/sandbox.rs, with the inherited
     listener's allowance. Confirms the confinement is in force (§5), then execve.
   - Measures with wait4.
   - Maps SIGBUS to 11.
   - Supplies the machine facts and the data folder.
6. **Left for the owner:** the tests that try to get past the sandbox (todo.md's stopped list,
   2026-11-05).
7. **Coverage.** The model-against-CPU harness on Apple's M1, once it builds through the edge
   (round 2 left the harnesses as raw `_start` programs, which only Linux runs; todo.md,
   round 2); tests/edge.rs's pseudo-terminal rows.
8. **Docs.** DESIGN's "Porting", README, plugin.json's description, and the skills' machine line.

**Verified by.**
- `compile`, then a full `test`, on every runner.
- The gate compares ARM64's verdict tables across Linux and macOS (finish line 2).
- The mutation test on the macOS runner.
- `bench` on it, for the record.

**Size and risks.** Size L, about 3.5k lines, of which the one edge file is about 1.5k, as each
of the Linux edge files is.
- Seatbelt and dyld's shared cache, which has lived under Cryptexes since macOS 13.
- Mach-O corners: `__bss` zero fill, and tables across sections.
- The ARM64 macOS runner's 7 GB against memory-heavy proofs.
- Only 5 macOS jobs at once.
- Seatbelt is deprecated: failing closed and the gate on the newest image are the mitigation.

#### Windows, x86-64 and ARM64 (was R4)

**Goal.** windows-x86_64 and windows-aarch64 pass the one suite natively, on windows-2025 and
windows-11-arm.

**Steps.**
1. **Profiles.** The two profiles. The executable is named main.exe.
2. **Toolchain.** lld-link with `/debug:symtab`, `.def` files and llvm-dlltool, and the COFF
   lowering. decode/pe.rs allows imports only through the IAT, no thunks, and allows base
   relocations.
3. **The edge files, `src/edge/windows-{x86_64,aarch64}.s`.**
   - `_start` (§1) and the handle table.
   - ReadFile/WriteFile: a broken pipe reads 0 and a write to one is EPIPE.
   - SetFilePointerEx and GetFileInformationByHandle.
   - GetSystemTimePreciseAsFileTime and QueryPerformanceCounter.
   - High-resolution waitable timers, and ProcessPrng.
   - CreateThread and WaitForSingleObject.
   - The console ops, with input translation.
   - ws2_32's accept, shutdown and WSAPoll, or the relayed path.
   - TerminateProcess.
   - The EFBIG check.
   - The GetLastError/WSAGetLastError table.
4. **`src/os/windows/`, through lengua-os.**
   - The LPAC profile `lengua.program`, created once per user.
   - A job: the program is killed when the job closes, runs as one process, dies on an unhandled
     exception, and shows no UI.
   - STARTUPINFOEX with exactly the right handles; the process starts suspended, is put in the
     job, then resumed.
   - CREATE_NO_WINDOW, and SystemRoot passed through.
   - A UTF-8 console code page, and `run` restoring the console mode.
   - Exceptions mapped to signals, and timeouts by TerminateJobObject.
   - Cycle time and working set.
   - The broker over an inherited duplex named pipe, with DuplicateHandle.
5. **Coverage.** ConPTY rows in tests/edge.rs, and the harnesses on both Windows runners. The
   tests that try to get past the sandbox are left for the owner (todo.md's stopped list,
   2026-11-05).

**Verified by.**
- A full `test` on every runner.
- Verdicts compared across the OSes of each CPU (two on x86-64, three on ARM64).
- The mutation test on both Windows runners.

**Size and risks.** Size XL.
- AppContainer quirks.
- The console.
- Process creation and Defender adding latency to every test run.
- cvc5's and z3's Windows ARM64 builds, new to lengua.

### R5 — the performance finish line

**Steps.**
- §7's bench, gate and workloads on both runners (the empty program is a workload and also the
  floor). The gate is enforced in `bench` mode and in the weekly run.
- Linux's own fast paths, each measured before and after: the vDSO clock, one `epoll` operation
  for the servers, `splice` for the pipe rows. Not io_uring, which Docker's default profile blocks,
  and nothing else that a default container refuses: a fast path counts only if the R3 container
  job passes with it, so the lead the gate shows is one every user gets, on any host.
- The known gaps, closed by A/B runs.
- BENCHMARKS-<target>.md, generated from the artifacts.
- README, DESIGN and the bench's own text made true to the runs; "threads on AArch64 come later"
  goes.

**Verified by.** The gate is green on both runners and KNOWN_BEHIND is empty.

### R6 — prover parity, the leftovers, documents, release

**Steps.**
- **Prover reach.**
  - x86-64 gains palignr, pblendw, punpck\*, pack\*, saturating and widening arithmetic,
    string compares, pextr\*/pinsr\*, pmovzx\* and gf2p8\*.
  - `plan_a64` follows pointers through csel, and-immediate and shifted add.
  - The model-against-CPU harness gets its baseline (xmm-only) variant.
  - AArch64 gets adc/sbc.
- **The layout** (left over from round 2; todo.md).
  - One `spec.md` per program, so each twin's copy and the same-bytes test
    (`every_program_has_its_twin_on_the_other_cpu`) go.
  - The assets split into `assets/x86_64/` and `assets/aarch64/`; x86-64's leave the assets root.
  - `tests/fixtures/linux-aarch64` becomes `tests/fixtures/aarch64`.
- **Small items.**
  - Refusal wording.
  - A hex number past i64::MAX.
  - An emulated session's features.
  - The signed-compare reading.
  - The one-folder check's 60 s budget against fast/wc's 231 s proof.
  - DESIGN's AArch64 instruction list.
  - test-format's float wording.
- **Release archives for both targets.**
  - scripts/release.sh and install.sh, packed as tar.gz, built as round 3 decides (musl or an
    older glibc) and published with the lengua image.
  - `release` mode builds, installs and checks each one on its own runner. They are unsigned.
- **A last pass** over DESIGN, the white paper and the plugin README.

**Verified by.** The full suites, the mutation tests, both goldens and `release` mode, green on
both runners.

## Verification

- **Only on GitHub's runners.** Every round's "Verified by" runs are dispatched there, and their
  ids go into todo.md. Nothing is built, tested, proven or measured on the owner's computer.
- **Per round:**
  1. `compile` on every target the change touches;
  2. the filtered tests;
  3. the full suite on the round's runners;
  4. from R2 on, also the mutation test, the goldens and the bench, as each round names them.
- **The finish line** is one dispatched `ci.yml` run on both runners, in modes test, ignored,
  bench and release, with everything green:
  - the suites, including the edge's conformance table and the mutation test;
  - the verdict comparison of the two CPUs' proofs;
  - the container job;
  - the bench gate, with KNOWN_BEHIND empty;
  - the release archives, installed and checked.

  todo.md then says "perfected", with that run's id.
