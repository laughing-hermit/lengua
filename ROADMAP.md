# lengua's roadmap: six targets, one product

This is how lengua gets from two targets (Linux on x86-64 and ARM64) to six (macOS and Windows on
each CPU too), and to the finish line set on 2026-11-05: lengua *perfected*, a replacement for
and an improvement on C, C++ and Rust, faster and leaner than them, the same product on every
target. The finish line itself, and what is left, are in [todo.md](todo.md). This file holds:
- the decisions the rounds rest on;
- the design: the OS edge, the broker, the server's OS seams, the toolchains, the sandbox on
  each OS, CI and the performance gate;
- each round's steps and the runs that verify it.

As a round lands, its part moves into [DESIGN.md](DESIGN.md) and leaves this file.

Decided with the owner (2026-11-04):
- **A shared edge per OS.** A program's text is written once per CPU and runs on all three OSes.
  Every operation that reaches the OS (read, write, open, the clock, threads...) is a call into a
  small canonical edge, one per target, which lengua supplies and checks word for word, as it
  does `assert_fail` and `fill` today. Linux programs move to it too.
  - So six targets need two program texts per program, not six, and the OS layer is written once
    per target.
  - Each edge is tailored for speed: raw system calls on Linux, libSystem's thinnest wrappers on
    macOS, kernel32 on Windows. CPU code stays tailored per CPU (SSE2/AVX-512, NEON/SVE).
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
reverse.

1. **A broker replaces the ptrace supervisor on every OS.** One Rust service holds the name
   rules, plus the clock, random bytes, sleeps and waits in tests. Under lengua (tests and `run`),
   a name operation costs one round trip, about what a ptrace stop costs today. A program run on
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
    - The macOS test jobs run on the newest images GitHub has made generally available (macos-26
      and macos-26-intel). A change Apple makes then stops runs at once, rather than quietly
      weakening them.

## Design

### 1. The edge

**One definition, `src/edge/mod.rs`.** It holds `enum Op`, 21 operations. Each one gives:
- its symbol;
- what allows it: always, a `Use`, a `- Network:` form, or threads;
- its arguments;
- the buffers it lends: which argument, its length, read or written;
- its result;
- its subset of the error set;
- Linux's kernel pin.

The module also holds the closed `Error` set, the open modes, the `Info` and listing-entry
layouts, and `PIPE_WRITE`. Each of these facts is then written once. The module replaces:
- `spec::Syscall`, `Use::syscalls` and its inverse `Use::of` (spec.rs:424-638), and `Network`'s
  call lists (spec.rs:1766-1779);
- `Profile::allowed_syscalls`, `syscall_name`, `syscall_number` and their pins
  (platform.rs:89-219);
- memory.rs's per-call buffers (156-200), and the prover's `lent` calls that repeat them;
- calls.rs's `Call` and its two tables, and `syscalls::connects`;
- the constants written twice (EBADF/EPIPE, EINVAL/EEXIST, O_CREAT, MSG_NOSIGNAL, getrandom's
  flags), and the file-call list (runner.rs:1133, supervise.rs:257-270).

| op | arguments → result | allowed by |
|---|---|---|
| `os_read`, `os_write` | fd, buffer, length → count | input, files, network, terminal; write always |
| `os_open`, `os_close` | name, mode → fd; fd → 0 | files (close: network too) |
| `os_seek`, `os_info` | fd, offset, whence → offset; fd, out[16] → 0 | files |
| `os_list` | folder fd, buffer, length → bytes (0 at the end) | files |
| `os_mkdir`, `os_remove`, `os_rename` | name; name, FILE/FOLDER; from, to → 0 | files |
| `os_clock`, `os_sleep` | REALTIME 0/MONOTONIC 1, out[16] → 0; clock, absolute?, time[16] → 0 | the clock |
| `os_random` | buffer, length ≤ 256 → count | random bytes |
| `os_accept`, `os_shutdown` | fd 3 → fd; fd, how 0/1/2 → 0 | a server; network |
| `os_poll` | table, count, ms (−1 forever) → ready | several clients, the terminal |
| `os_terminal`, `os_window` | RAW 1/RESTORE 0 → 0 or −ENOTTY; out[8] rows, cols → 0 or −ENOTTY | the terminal |
| `os_thread_start`, `os_thread_join` | index, slot → 0; index → the worker's result | threads (lengua's library only) |
| `os_exit` | status → never returns | always |

**Errors.** There is one closed set: Linux's numbers, negated. They are 2, 5, 9, 11, 13, 16, 17,
20, 21, 22, 24, 25, 27, 28, 29, 32, 36, 39, 84, 104 and 107.
- Each op lists its subset, and the prover returns exactly that subset.
- Every edge, Linux's included, turns an error outside the op's list into EIO. That happens on
  the error path only.
- macOS's `errno` and Windows's `GetLastError`/`WSAGetLastError` map through tables in their edge
  files. A test requires each table to cover the whole set.
- The broker returns EACCES for ELOOP and EXDEV, as Landlock answers today.

**Normalized data**, the same bytes on every OS. Each OS's own structures (`stat`, `O_*`,
`linux_dirent64`, termios, Windows's) live only in its edge file.
- `Info` is {size, kind}: kind 0 is a file, 1 a folder, 2 anything else. The size is 0 unless the
  kind is a file.
- A listing entry is {u16 length (a multiple of 8), u8 kind, u8 name length, name, NUL,
  padding}. `.` and `..` are left out.
- Open modes are READ 0, WRITE 1, APPEND 2, NEW 3 and FOLDER 4. Names are relative to the working
  folder and use `/` on every OS.
- `os_poll` takes Linux's 8-byte pollfd entries, with POLLIN 1, POLLERR 8, POLLHUP 16 and
  POLLNVAL 32. Windows translates them to WSAPOLLFD and back.
- RAW is the template's mode: no echo and no line editing, with signals and output processing
  kept. Windows translates console input as a POSIX terminal gives it: CR becomes LF. In line
  mode CRLF becomes LF, and a Ctrl+Z line is the end of input.
- On macOS and Windows, an absolute `os_sleep` is a repeated relative wait.

**Sources.**
- There are six edge files, `src/edge/<target>.s`, and two OS-neutral CPU libraries,
  `src/edge/x86_64.s` and `src/edge/aarch64.s`. The libraries hold assert_fail and
  assert_message, fill, and `parallel_for(count, slots, slot_size, results)`, which runs
  `worker(index, slot)`.
- All of them are embedded with `include_str!`, never copied into main.s.
- Each build writes lengua.s (the library, plus the edge functions main.s's calls reach), appends
  it after main.s's last line, and assembles one object. main.s's line numbers stay the same, and
  a name clash becomes an assembler error.
- Only reached ops are linked, so a program imports only what it calls.
- Each OS fact is written once, in its edge file:
  - Linux's `SYS_*` numbers; seccomp's allow-list is read from the linked code with
    `cfg::constant_before`;
  - macOS's errno table;
  - Windows's offsets and each import's DLL; the `.tbd` and `.def` import lists are read from the
    edge's undefined symbols.
- main.s declares the error and mode constants it compares against as `.equ` lines. The checker
  holds them to edge's values, as it holds `SYS_` constants today.

**The checker.**
- An executable's functions form two units:
  - main.s's functions, matched line by line to main.s under every rule;
  - lengua's functions, matched line by line to the lengua.s that lengua assembled, and exempt
    from the program rules.
- Only lengua's unit makes system calls or library calls: through the GOT on macOS and the IAT on
  Windows, with no stubs or thunks.
- It refuses:
  - `syscall`/`svc` in main.s;
  - main.s defining a reserved name (`_start`, `os_*`, assert_fail, fill, parallel_for);
  - main.s touching lengua's data, or entering lengua anywhere but an op's or the library's entry;
  - a call to an op the spec doesn't allow;
  - any import beyond the linked ops'. On Mach-O that means only libSystem, every GOT slot an
    allowed import, and no `__stubs`; on PE, only the linked ops' DLLs and functions.
- A test holds the edge files and the libraries to the dialect's rules, apart from their OS
  calls.

**One calling convention per CPU, shared by its three OSes.** `Abi.edge_arguments` and
`edge_clobbers` replace the system-call pair.
- Arguments go in rdi, rsi, rdx, r10, r8, r9, or x0–x5. The result comes back in rax or x0.
- What a call changes:
  - x86-64: rax, rcx, r11, the flags, every xmm/ymm/zmm register, and k0–k7;
  - AArch64: x0, x8, x16, x17, x30, NZCV, v0–v7 and v16–v31, the upper halves of v8–v15, and all
    of z, p and ffr. d8–d15 are kept, which costs nothing on any of the three OSes.
- On Linux an op is `mov eax, SYS_X; syscall; ret`, or `mov x8, #SYS_X; svc #0; ret`.
- The macOS and Windows edges:
  - save what their C ABI may change but the convention keeps: on x86-64, rdi, rsi, rdx and
    r8–r10 on macOS, and rdx and r8–r10 on Windows; on AArch64, x1–x7 and x9–x15;
  - realign the stack and add Windows's 32-byte shadow space;
  - run `vzeroupper` before calling the library, on x86-64 with AVX.
- New rules:
  - after a call that reaches the OS (directly, or through fill, parallel_for or the program's own
    functions), a changed register can't be read until it's written. The prover gives those
    registers fresh values;
  - every function's frame is under 4,096 bytes. `MAX_STACK_BYTES` (1 MiB per thread) stays.

**Start, exit, threads, terminal, descriptors, pipes.**
- *Start.* `_start` calls `main(argc, argv, envp)`, with the three in the first argument
  registers.
  - On Linux they come from the start stack.
  - On macOS, dyld calls LC_MAIN's `_start` with them already in place.
  - On Windows:
    - the edge splits `GetCommandLineW` by the MSVC rules, which `runner::arguments_length`
      (runner.rs:72-95) already models;
    - it converts each argument with `WideCharToMultiByte(CP_UTF8)`;
    - it takes the environment from `GetEnvironmentStringsW`, leaving out the `=X:` entries and
      SystemRoot (which the launcher adds for Winsock);
    - it holds them in static buffers, at most 98,301 bytes;
    - it calls `WSAStartup` when a network op is linked.
  - This replaces the start-stack rules (cfg.rs's entry depth, memory.rs:262-268 and 839-856,
    rules.rs:208-214) and the prover's start stack (exec.rs:940-965).
- *Exit.* exit_group on Linux, `_exit` on macOS, and `TerminateProcess(GetCurrentProcess(),
  status)` on Windows, because `ExitProcess` would run DLL detach code while the workers die.
- *Threads.*
  - Linux uses `clone` on the edge's static stacks (64 × 1 MiB in `.bss`), joined by waiting on a
    futex.
  - macOS uses `pthread_create`/`pthread_join` with 1 MiB stacks.
  - Windows uses `CreateThread` with 1 MiB and joins with `WaitForSingleObject`.
  - This replaces three things: the parallel template's trio, threads.rs's word-for-word check
    and constants (threads.rs:36-44), and the prover's clone/futex model (exec.rs:3503-3589,
    3913-3928).
  - The new model runs the worker at start and returns its result at join.
- *Terminal.*
  - Linux: ioctl TCGETS/TCSETS/TIOCGWINSZ on descriptors 0–2.
  - macOS: tcgetattr, tcsetattr and ioctl. A key wait uses `select`, because `poll` fails on
    terminals there.
  - Windows: Get/SetConsoleMode with VT enabled, GetConsoleScreenBufferInfo, and a wait on the
    console handle.
  - `run` restores the mode on every OS.
- *Descriptors.*
  - POSIX: a program has 4,352 descriptors, and RLIMIT_NOFILE is 4,353. The broker's channel is
    descriptor 4,352, which every op refuses when main.s passes it.
  - Windows: the edge keeps a table of 4,352 slots {kind, read handle, write handle}, filled
    lowest free first.
    - Slots 0–2 come from `GetStdHandle`.
    - The listener (3), the connections (4 and up) and the channel (4,352) come from
      STARTUPINFO's `lpReserved2`, which the launcher writes in the MSVC CRT's format.
- *Pipes.*
  - The runner writes a test's standard input of up to 4,096 bytes into its pipe, sized larger,
    before the program starts.
  - One read then returns all of it: by POSIX's read rule, and by Windows's byte-mode pipes, which
    the R1 spike confirms.
  - `PIPE_WRITE` moves into edge and stays 4,096 on every target. DESIGN states a proof's
    condition as "input that one read returns whole".

**The prover.**
- A call to an `os_*` function runs `Cpu::edge_call(op)`, in the new prove/exec/edge.rs.
  - There is one model per op, moved from `Cpu::syscall` (exec.rs:3591-4003) and net.rs.
  - It reads the arguments through `edge_arguments`, writes the result, and applies
    `edge_clobbers`.
  - The clobbers include SVE state. Linux clears it, but today's model keeps it
    (exec/aarch64.rs:219).
  - It never steps into edge code.
- fill, assert_fail and parallel_for are stepped as CPU code.
- Removed:
  - `Call`, `Call::of` and `Convention` (calls.rs:124-167);
  - `Image::linux` and `LinuxAbi`;
  - the rcx/r11 lines in State::returns (exec.rs:1404-1410);
  - poll and ppoll as two separate cases;
  - "a system call chosen by the input";
  - the models of calls no spec allows;
  - AT_FDCWD and the O_ constants.

### 2. The broker, in place of the ptrace supervisor

- `src/broker.rs` is one OS-neutral service. It runs in the process that started the program
  (the server, or `lengua-mcp run`), with one thread per program. Every message is treated as
  hostile input, and the parser has a fuzz target.
- **Names.** `os_open`, `os_mkdir`, `os_remove` and `os_rename` are requests to the broker. The
  broker:
  - applies `names::broken` and `names::clashes`, with a cache per folder;
  - matches each name byte for byte against the folder's entries;
  - answers EBUSY from its own record of what is open, kept current by a one-way note the edge
    sends on each close;
  - opens beneath the folder without following links:
    - Linux: openat2 with `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS`;
    - macOS: a walk with `O_NOFOLLOW`;
    - Windows: reparse-point checks, POSIX-semantics delete and rename, `FILE_SHARE_DELETE` and
      verbatim paths. Another process holding a file there gives an environmental EBUSY;
  - hands the descriptor back by SCM_RIGHTS on POSIX, or with DuplicateHandle into the program on
    Windows.
- **Tests.** The broker's first message is waiting before the program starts. It says whether
  this is a test or a run, and carries the Supply (supply.rs) and the file limit. In a test:
  - clock readings and random bytes come from the broker, and sleeps end at once;
  - listings come in the OS's order, then turned round in a second run (`Job::reversed`, as
    today);
  - `os_accept` and `os_poll` send a one-way "waiting" note, which replaces the
    `/proc/<pid>/syscall` probe (testrun.rs:1129-1152);
  - the edge fails a write past the file limit with EFBIG. RLIMIT_FSIZE stays set above that
    limit, as a backstop.
- **`run`.** The broker handles names only. Reads, writes, seeks, polls, listings of a handed
  folder, the clock, random bytes and sleeps go straight to the OS. A spec that uses no files gets
  no channel.
- **On its own.** A program that finds no channel calls the OS directly, as programs outside
  lengua do today. lengua's guarantees hold for what `run` runs after `check`, as DESIGN's threat
  model already says.
- **The sandbox** grants no path on any OS.
- **Deleted:** supervise.rs, `syscalls::host`, the per-CPU register code, the /proc probe, and
  nix's ptrace and uio. Under QEMU, names and the supplied values start working: a socket works
  there, where ptrace didn't.

### 3. The server's OS seams

- **`src/os/`** has four parts:
  - `unix/`, shared by Linux and macOS: descriptor placement, rlimits, execve, wait4, restoring
    the terminal, forwarding signals, socketpairs and SCM_RIGHTS;
  - `linux/`: Landlock, seccomp and perf counters, moved from runner.rs:1254-1700;
  - `macos/`: Seatbelt and sysctl;
  - `windows/`, built on `crates/lengua-os`.
- **Host facts.**
  - CPU time and peak memory come from rusage on Unix. On Windows they come from
    `QueryProcessCycleTime` and `PeakWorkingSetSize`, because the job's accounting only advances
    every ~15.6 ms.
  - `cpu_model` comes from /proc/cpuinfo, `sysctl machdep.cpu.brand_string`, or the registry.
  - The memory guard uses `sysinfo` on all three OSes, in place of prove/watch.rs's /proc reads.
  - The data folder is XDG on Linux, `~/Library/Application Support/lengua` on macOS, and
    `%LOCALAPPDATA%\lengua` on Windows.
  - The key folder has mode 0700 on Unix, and an ACL for the user alone on Windows.
- **Keyed by target, never silently by CPU.**
  - `Profile::os()` and `Profile::isa()` are the only exhaustive matches, besides the name and
    the triple.
  - `Isa::abi()` stays per CPU.
  - `decode::read(bytes, profile)` reads the profile's format and refuses any other.
  - `Image::linux`, `syscalls::host` and `supervise::AVAILABLE` are removed.
  - `CpuFeatures::baseline` takes an `Isa`.
- **Profiles.** LinuxX86_64, LinuxAarch64, MacosX86_64, MacosAarch64, WindowsX86_64 and
  WindowsAarch64. The ARM64 ones are named `macos-aarch64` and `windows-aarch64`; header.rs:232
  and tests/sessions.rs:102 say `macos-arm64` today.
- **Tables.**
  - `PerCpu<T>` and `PerOs<T>` join `PerTarget<T>`. `PerTarget` keeps only what differs per
    CPU-and-OS pair: HOSTS/RUNNERS, and limits.rs's sandbox-bypass sources.
  - Today's 289 cells are each a CPU's text or count, so they become `PerCpu`. macOS and Windows
    then inherit every case.
  - Check-only tables run every target of the cell's CPU.
  - Prover tables run the host's CPU built for the host's OS; the other CPU's run only on the
    Linux runners.
  - Run tables run the host's target.
- **Crates.**
  - Linux only: landlock, seccompiler, perf-event2.
  - Unix: rustix, and nix (process, signal).
  - Everywhere: sysinfo and object (elf, macho, pe). cap-std too, if it opens files beneath a
    folder on all three OSes.
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
| macOS | `ld64.lld -arch x86_64\|arm64 -platform_version macos 11.0 11.0 -e __start -fatal_warnings`; dynamic, LC_MAIN, ad-hoc signed on ARM64 | a minimal libSystem.tbd lengua writes from the edge's imports | LLVM only (llvm-mc and lld, from conda-forge, Homebrew on ARM64, or the release archive); no Xcode, no SDK |
| Windows | `lld-link /entry:_start /subsystem:console /nodefaultlib /dynamicbase /nxcompat /highentropyva /stack:0x800000 /Brepro /WX /debug:symtab` | kernel32 (+ ws2_32, bcryptprimitives when used), import libraries built by llvm-dlltool from `.def` files lengua writes, cached in the data folder | LLVM's Windows release archive (llvm-mc, lld-link, llvm-dlltool); no Windows SDK |

Triples: `{x86_64,aarch64}-unknown-linux-gnu`, `{x86_64,arm64}-apple-macos11.0`,
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
- **Solvers.** z3 4.16.0 and cvc5 1.3.4 both publish native builds for all six targets
  (checked 2026-11-05; cvc5's include `cvc5-Win64-arm64-static.zip`). Both are pinned by SHA-256
  in `scripts/ci/tools.tsv`.
- **LLVM on macOS and Windows in CI.** LLVM publishes no Intel macOS archive for 22.1.8, and
  Homebrew no longer builds Intel bottles for llvm or lld. conda-forge does publish `lld` and
  `llvm-tools` 22.1.8 for osx-64, osx-arm64, win-64 and win-arm64. So setup installs them there
  with micromamba (pinned by SHA-256), the same LLVM version the Linux jobs take from
  apt.llvm.org. Users' installs follow in R6, where shipping llvm-mc and lld in the release
  archive is weighed.

### 5. The sandbox on each OS

| limit | Linux | macOS | Windows |
|---|---|---|---|
| files only through the broker | Landlock, no path | Seatbelt deny-default, system libraries readable | LPAC AppContainer, no ACL |
| no network of its own | Landlock net + seccomp | Seatbelt `deny network*`, an inherited listener allowed | no capability |
| connections handed over (3, 4…) | dup2 + SCM_RIGHTS | same | handle list + the CRT table: sockets, or relayed pipes if the spike says so |
| 4,352 open files | RLIMIT_NOFILE 4,353 | same | the edge's table → EMFILE |
| time and output limits | runner | runner | runner + TerminateJobObject |
| no processes, no signals to others | seccomp, Landlock scopes | Seatbelt `deny process-fork`, `signal` | job: one active process |
| no core files, no dialogs | RLIMIT_CORE 0 | same | SetErrorMode, no WER, die on an unhandled exception, CREATE_NO_WINDOW |
| crashes as signal numbers | native | SIGBUS → 11 | access violation and stack overflow → 11, illegal instruction → 4, divide → 8, Ctrl+C → 2, ended by lengua → 9 |

Some behavior is reshaped on every target, so all six keep one promise:
- EFBIG replaces SIGXFSZ.
- Terminal ops replace the ioctl pins.
- `sendto` with MSG_NOSIGNAL folds into `os_write`; SIGPIPE is ignored.
- nanosleep's remaining time and accept's peer address are dropped.
- poll and ppoll become one `os_poll`, in milliseconds.
- Listening ports start at 1024; Windows binds them with SO_EXCLUSIVEADDRUSE.
- Names match exactly.

Two OS differences remain:
- **A per-call kernel filter exists only on Linux.** On macOS and Windows two things keep the same
  promises with one layer fewer: the checker guarantees that main.s reaches the OS only through
  the linked edge, and the OS sandbox does the rest. DESIGN's threat model says so.
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
  - **test**, on six runners: ubuntu-24.04, ubuntu-24.04-arm, macos-26-intel, macos-26,
    windows-2025 and windows-11-arm. Each runs the whole suite in one job, which took 35 to 62
    minutes (run 42085247387). Shards were planned and left out: seven test jobs fit the Free
    plan's concurrent jobs, and a shard split by test would break the proving lock, which is
    per process.
    - The macOS jobs use the newest images GitHub has made generally available. They move up a
      version when a newer one does, so macOS's sandbox tests always gate on the newest macOS.

    Each job:
    - runs `scripts/ci/setup.sh`: Rust 1.99.1, LLVM 22 (Windows ARM64's ring build needs clang,
      which the image's LLVM has) and the solvers, each pinned in `scripts/ci/tools.tsv`;
    - writes the machine's facts into the run summary (`scripts/ci/machine.sh`);
    - runs clippy with `-D warnings`;
    - runs `cargo test --workspace`;
    - writes the skip summary, and keeps the logs, skip logs and benchmark results as artifacts.
  - **qemu** (ubuntu-24.04): the suite again with QEMU's user mode, so the ARM64 cells run on
    an x86-64 computer as they do on a developer's. It reports without failing the run until its
    seven failing cases are fixed (todo.md, round 2).
  - **gate**: depends on plan, format and test.
- **Runner facts** (2026-11-04). 4 vCPUs on Linux, kernel 7.0 (azure), Landlock and yama in the
  LSM list, `ptrace_scope` 1. The x86-64 pool is mixed, about half of it with AVX-512, and the
  ARM64 runner has SVE. Not recommended: lowering the proof budgets (tests assert verdicts);
  larger or self-hosted runners (outside the $0 rule); emulating AVX-512 (QEMU's TCG has
  none).
- **When it runs.**
  - Pushes to main run it, path-filtered; `wip/**` branches get no push runs.
  - Dispatch inputs: targets (all six by default); mode (`compile` is clippy plus `cargo test
    --no-run`, about 10 minutes; also `test`, `ignored`, `bench`, `release`); filter;
    check_folder; stall_seconds.
  - Every week: `test` and `ignored` on all six. The same run also covers:
    - the full suite on the previous macOS images (macos-15, macos-15-intel), because programs
      are built for older macOS too;
    - the sandbox tests on any newer macOS image GitHub has in preview. That job only warns,
      and is not part of the gate.
- **Caches and actions.**
  - Caches hold the registry and a pruned target/ per runner, with LLVM and the solvers cached
    separately, all within 10 GB.
  - Actions use their Node 24 majors.
  - Runs cancel in progress only off main.
- **Line endings.** `.gitattributes` (`* text=auto eol=lf`) lands before the first Windows test,
  because GitHub's Windows images would otherwise check out CRLF. A spec's hash normalizes CRLF.
- **parity_inventory.**
  - R1 adds HOSTS: six runner labels, each needing a native `cargo test` job, read from ci.yml's
    matrix.
  - RUNNERS gains macOS's cells in R3 and Windows's in R4.
  - DEBT is empty after R1.
- **How a round iterates.**
  1. Push `wip/<round>`.
  2. Dispatch `compile` on every target the change touches.
  3. Run the filtered tests, then the full suite.
  4. Take formatting only from `fmt.patch`, with `gh run download` and `git apply`.
  5. Merge when the runs are green, and put their run ids in todo.md.

### 7. The performance finish line

- **The bench** (benches/against_rust_and_c.rs) becomes portable.
  - lengua's versions become `PerCpu` (:82-101), built by lengua's own build, with the edge.
  - C and C++ are built the way each OS's developers build them:
    - Linux: gcc/g++ `-O3 -march=native -flto`;
    - macOS: Apple clang `-O3 -mcpu=native`;
    - Windows: LLVM clang `-O3 -march=native` with the UCRT. A shared `benches/programs/io.h`
      gives the C programs a Windows path.
  - Rust is built as now.
  - A hidden `lengua-mcp --measure` does the measuring, replacing GNU time and the /bin/sh FIFO
    (:483-577).
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
  - Every row the first six-runner gate run shows level or behind joins this list.

## Rounds

### R0 — documents (done 2026-11-05)
- todo.md is rewritten: the finish line, the roadmap, every open item under the round that takes
  it, the owner's items, the deferred ones, and the stopped list.
- README's benchmark claim says what BENCHMARKS.md shows.
- DESIGN's performance bar and workload count match the finish line.
- This file is added.

Nothing ran, since R0 changed documents only; the commit is the record.

### R1 — six hosts, one workflow, the spikes, the soundness items

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
     under `cfg(unix)`; add object's macho and pe features, and sysinfo.
   - Make a workspace with `crates/lengua-os`.
3. **`src/os/`** (§3). Done, with these differences from the plan (2026-11-05):
   - runner.rs's launch half moved whole into `src/os/linux.rs`, and `src/os/unsupported.rs`
     gives every other system the same four things, saying programs don't run there yet. The
     `unix/` half macOS shares is split out in round 3, when macOS shows what it needs.
   - supervise.rs is Linux's. The /proc probe in testrun.rs stays until R2 removes it.
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
4. **Keying by target.** `Profile::os()`/`isa()` and the rest of §3's keying;
   `CpuFeatures::baseline` taking an `Isa`; the `-aarch64` names.
5. **Tables.**
   - `PerCpu` goes in src/targets.rs, with `PerTarget` kept for what differs by system too.
     `PerOs` and `Profile::os()` wait for round 3, when a second system's profile makes them
     needed.
   - The 288 cells are converted mechanically.
   - `every_prover_test_runs_on_every_target` (src/prove/tests.rs:5068-5093) takes `PerCpu`.
   - Host-bound tests use `targets::host()` and `skiplog::note`.
6. **One folder check.** `check_one_aarch64_folder` folds into `check_one_folder(target)`, which
   keeps the QEMU path. DEBT is then empty.
7. **CI.** ci.yml and `scripts/ci/{setup.sh,shard.sh,tools.tsv}` (§6); the three old workflows
   are deleted; parity_inventory gets HOSTS.
8. **Soundness items on Linux**, so that R2's ledger rests on a sound prover:
   - the AArch64 vector model's wildcard: an unmodelled op says "doesn't model yet", as x86-64's
     does, instead of running as zeros;
   - `cbz`/`tbz` on xzr/wzr no longer get past the dead-code and "assertion always fails" checks;
   - SVE state across `svc` is cleared, as Linux clears it;
   - `prove_all`'s done-before-lock window: it writes `[proofs]` before the last line goes final;
   - the prover's time budgets are scaled by a measured speed probe instead of the vCPU count, so
     runners of different speeds reach the same verdicts.

**Reuse.** `PerTarget::cells` (src/targets.rs:34), `skiplog::note`, scripts/skip-summary.sh and
stall-report.sh, parity_inventory's `test_jobs_of`/`runners_of`, and `Toolchain::assemble/link`
(src/toolchain.rs:111-149).

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

### R2 — the edge and the broker, on Linux (both CPUs)

**Goal.**
- Every Linux program reaches the OS only through the linked edge.
- The operations are defined once.
- The broker replaces the supervisor.
- The suites, the mutation test and both goldens are green on both runners.
- The verdict ledger matches, except for listed, expected differences.
- The bench is level with the pre-edge build, measured in the same run.

**Batches.** Each batch is a `compile` dispatch on six runners plus filtered tests on both Linux
runners. Batches (a) to (c) merge to main as each goes green, so main never waits for the whole
round.

0. **The ledger, on main, first.** Every proof line's verdict per CPU, from main's full runs,
   kept as an artifact.
1. **(a) The core.**
   - src/edge/mod.rs.
   - The Linux edge files: `_start`, the 21 ops, the broker helper, and `os_info` built from stat.
   - The CPU libraries.
   - The build appends lengua.s, and seccomp's allow-list is read from the linked code.
   - The checker:
     - the two units;
     - `Exit::Edge{op}` (cfg.rs);
     - the allowed-op rule, replacing rules.rs:351-446;
     - rule 7 on edge calls;
     - the exit-status rule at `os_exit`;
     - `Op::lends` feeding memory.rs;
     - the register and frame rules;
     - main taking argv and envp;
     - the reserved names.
   - prove/exec/edge.rs.
   - The broker, over a Unix socket at fd 4,352, with Landlock granting no path.
   - **tests/edge.rs, the op conformance table.** Each op's cases are a program per CPU, run
     natively and compared with the prover's model. Sentinel values check the kept registers, and
     the terminal ops run on a pseudo-terminal.
   - Both forms are accepted, inline system calls and edge calls, with a ratchet list of
     unmigrated programs that may only shrink.

   Filters: `checker::`, `prove::`, `edge`, `broker`.
2. **(b) The programs.**
   - Templates, fast programs, examples, fixtures, twins and benches migrate (about 95 `.s`
     files).
   - Assets split into assets/x86_64/ and assets/aarch64/; x86-64's leave the assets root.
   - One spec.md per program, so the twins' copies go, with the same-bytes test
     (tests/every_program.rs:190-205).
   - tests/fixtures/aarch64.
   - upcase's retries are unified, and ARM64's assertions renumbered.
   - The header names the CPU and leaves the baseline out of `needs`. The lock still names the
     target.
   - MACHINE_WORDS gains Windows's words and AArch64's registers.
   - The model-against-CPU harnesses (against_cpu.rs:578, 846; against_a64.rs:842, 1112) build
     through the edge, so they run on every runner, with one feature gating for both CPUs.
   - The bench builds through the edge, and hosts a broker for its `run` lines.

   Filters: `template`, `standalone`, `baseline`, `crypto_forms`, `every_program`, `programs`.
3. **(c) The snippets.**
   - The assembly embedded in limits, red_team, network, sessions, workflow, PLANTED and LENDINGS.
   - Listing tests go through the broker's turned-round run.
   - The SIGXFSZ test becomes an EFBIG test.
   - The supervisor-failure tests become broker-failure tests.
4. **(d) When the ratchet is empty.**
   - Delete spec::Syscall, the `SYS_` rules, the start-stack rules, calls.rs's tables,
     Convention, Image::linux, syscalls.rs, supervise.rs, nix's ptrace and uio, and the inline
     form.
   - syscalls.rs's header-pinning tests become one test, holding the Linux edge's `.equ SYS_*`
     lines to the kernel headers and libc.
   - The docs, each paragraph rewritten once:
     - references/edge.md: the ops once, CPU-neutral, with each CPU's registers;
     - references/x86_64.md and aarch64.md, renamed from `linux-*`;
     - compile/SKILL.md: nothing is copied any more, and the CPU folder comes from the machine
       line;
     - porting.md;
     - DESIGN's "Running programs", "Resources" and "Porting" items 5–6.
   - Then everything on both Linux runners:
     - the full suites;
     - the mutation test;
     - both goldens, compared with recordings made at the merge-base in the same run: a fresh
       runner has no recording, so alone they record and compare nothing, as round 1's did (run
       42084426990);
     - the bench A/B: `LENGUA_BENCH_AGAINST=<merge-base>` builds the pre-edge programs beside the
       new ones, in one run.

**Reuse.**
- From the checker: `cfg::constant_before` (cfg.rs:74-97), checker/source.rs's parse,
  `decode::map`, `Checker::results` (rules.rs:503-557), and memory.rs's lending summaries
  (502-519).
- From exec.rs: `open_chosen`, `file_details`, `make_folder`, `remove`, `rename`, `name_at`,
  `State::refused`, `start_thread`, `accept`, `send`, `receive`, `shutdown`, `poll_network`.
- Elsewhere: `names::broken`/`clashes`, `Supply::encode/decode` (supply.rs:102-170),
  `connect::pair` and `Relay`, `runner::portable_run`.

**Verified by.**
- The ledger matches, except for listed, expected differences.
- tests/edge.rs, the full suites, the mutation test and both goldens are green on both Linux
  runners.
- In the A/B, CPU time, wall time, peak memory and size are level within the spread. Each op
  costs one call and one return more, and that is reported.
- `run`'s many-files line is no worse than the supervisor's.

**Size and risks.** Size XL: about 95 programs, ~300 snippets, ~4k lines of Rust, ~1.5k lines of
edge assembly.
- The sheer volume.
- Goldens that hold addresses: they must not, or must be recorded per target.
- The register rule may find more sites than expected.
- Broker latency against the 2-second test limit.

### R3 — macOS, x86-64 and ARM64

**Goal.** macos-x86_64 and macos-aarch64 pass the one suite natively, on macos-26-intel and
macos-26, and also every week on macos-15-intel and macos-15.

**Steps.**
1. **Profiles.** The two profiles, their RUNNERS cells, and `for_target("macos", …)`.
2. **Toolchain.** `Os::Macos`'s triple, ld64.lld, the tbd writer, and the Mach-O lowering (§4).
   The executable is named `main`.
3. **decode/macho.rs**, with the read-only checks.
4. **The edge files, `src/edge/macos-{x86_64,aarch64}.s`.**
   - LC_MAIN's `_start`, imports through the GOT, register saving, and the errno table.
   - stat → Info. Its offsets are pinned by a test against the libc crate's stat on the runner,
     as `the_host_table_is_the_c_librarys` (syscalls.rs:400) does today.
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
7. **Coverage.** The model-against-CPU harnesses on Apple's M1 and on the Intel Mac;
   tests/edge.rs's pseudo-terminal rows.
8. **Docs.** DESIGN's "Porting", README, plugin.json's description, and the skills' machine line.

**Verified by.**
- `compile`, then a full `test`, on six runners.
- The gate compares each CPU's verdict tables across Linux and macOS (finish line 2).
- The mutation test on both macOS runners.
- `bench` on both, for the record.

**Size and risks.** Size L, about 5k lines.
- Seatbelt and dyld's shared cache, which has lived under Cryptexes since macOS 13.
- Mach-O corners: `__bss` zero fill, and tables across sections.
- The ARM64 macOS runner's 7 GB against memory-heavy proofs.
- Only 5 macOS jobs at once, and the queue for the Intel runner.
- Seatbelt is deprecated: failing closed and the gate on the newest image are the mitigation.

### R4 — Windows, x86-64 and ARM64

**Goal.** windows-x86_64 and windows-aarch64 pass the one suite natively, on windows-2025 and
windows-11-arm.

**Steps.**
1. **Profiles.** The two profiles and their RUNNERS cells. The executable is named main.exe.
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
- A full `test` on six runners.
- Verdicts compared across the three OSes of each CPU.
- The mutation test on both Windows runners.

**Size and risks.** Size XL.
- AppContainer quirks.
- The console.
- Process creation and Defender adding latency to every test run.
- cvc5's and z3's Windows ARM64 builds, new to lengua.

### R5 — the performance finish line

**Steps.**
- §7's bench, gate and workloads on six runners (the empty program is a workload and also each
  OS's floor). The gate is enforced in `bench` mode and in the weekly run.
- The known gaps, closed by A/B runs.
- BENCHMARKS-<target>.md, generated from the artifacts.
- README, DESIGN and the bench's own text made true to the runs; "threads on AArch64 come later"
  goes.

**Verified by.** The gate is green on six runners and KNOWN_BEHIND is empty.

### R6 — prover parity, the leftovers, documents, release

**Steps.**
- **Prover reach.**
  - x86-64 gains palignr, pblendw, punpck\*, pack\*, saturating and widening arithmetic,
    string compares, pextr\*/pinsr\*, pmovzx\* and gf2p8\*.
  - `plan_a64` follows pointers through csel, and-immediate and shifted add.
  - The model-against-CPU harness gets its baseline (xmm-only) variant.
  - AArch64 gets adc/sbc.
- **Small items.**
  - Refusal wording.
  - A hex number past i64::MAX.
  - An emulated session's features.
  - The signed-compare reading.
  - The one-folder check's 60 s budget against fast/wc's 231 s proof.
  - DESIGN's AArch64 instruction list.
  - test-format's float wording.
- **Release archives for six hosts.**
  - scripts/release.sh and install.sh per OS, packed as tar.gz, or zip on Windows.
  - mcp.json names lengua-mcp.exe on Windows.
  - `release` mode builds, installs and checks each one on its own runner. They are unsigned.
- **A last pass** over DESIGN, the white paper and the plugin README.

**Verified by.** The full suites, the mutation tests, both goldens and `release` mode, green on six
runners.

## Verification

- **Only on GitHub's runners.** Every round's "Verified by" runs are dispatched there, and their
  ids go into todo.md. Nothing is built, tested, proven or measured on the owner's computer.
- **Per round:**
  1. `compile` on every target the change touches;
  2. the filtered tests;
  3. the full suite on the round's runners;
  4. from R2 on, also the mutation test, the goldens and the bench, as each round names them.
- **The finish line** is one dispatched `ci.yml` run on all six runners, in modes test, ignored,
  bench and release, with everything green:
  - the suites, including the edge's conformance table and the mutation test;
  - the gate job's verdict comparison across the three OSes of each CPU;
  - the bench gate, with KNOWN_BEHIND empty;
  - the release archives, installed and checked.

  todo.md then says "perfected", with that run's id.
