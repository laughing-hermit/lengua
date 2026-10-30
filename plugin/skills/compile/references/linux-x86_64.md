# Linux x86-64 reference

## System calls

What the spec names under `# Constraints` decides the calls a program may make on this computer:
`write`, `exit` and `exit_group` always; `read` for `standard input`; `openat`, `read`, `write`,
`lseek` and `close` for `files`; `clock_gettime` and `clock_nanosleep` (to wait) for `the
clock`; `getrandom` for `random bytes`; none for `the environment`, which is on the stack at the
start; `socket` and `connect` for `- Network: connects to …`, `accept4` for `- Network: listens
on …`, and `read`, `close`, `shutdown` and `sendto` for either. The kernel refuses any other.

`mov eax, SYS_NAME` picks the call, the arguments go in `rdi`, `rsi`, `rdx`, `r10`, `r8` and `r9`
(`r10`, not `rcx`), and `syscall` runs it. It returns in `rax` and changes `rcx` and `r11`; every
other register keeps its value. A result from -4095 to -1 is an error (the negative errno), so
test it right away with `test rax, rax` and `js`.

| Call | `SYS_` number | Arguments | Result |
|---|---|---|---|
| `read` | 0 | `edi` fd, `rsi` buffer, `rdx` count | bytes read, 0 at the end of input, or an error |
| `write` | 1 | `edi` fd, `rsi` bytes, `rdx` count | bytes written, possibly fewer than asked, or an error |
| `close` | 3 | `edi` fd | 0 or an error |
| `lseek` | 8 | `edi` fd, `rsi` offset, `edx` whence (0 start, 1 current, 2 end) | the new offset or an error |
| `exit` | 60 | `edi` status | doesn't return |
| `clock_gettime` | 228 | `edi` clock (0 the time of day, 1 a clock that only moves forward), `rsi` 16 bytes for seconds then nanoseconds | 0 or an error |
| `clock_nanosleep` | 230 | `edi` clock (1, the one that only moves forward), `esi` 0, `rdx` 16 bytes of seconds then nanoseconds to wait, `r10` 0 | 0 or an error |
| `exit_group` | 231 | `edi` status | doesn't return |
| `openat` | 257 | `edi` -100 (the current folder), `rsi` NUL-terminated path, `edx` flags, `r10d` mode | a file descriptor or an error |
| `getrandom` | 318 | `rdi` buffer, `rsi` count, `edx` flags (0) | bytes filled, all of them for up to 256, or an error |
| `socket` | 41 | `edi` 2 (IPv4) or 10 (IPv6), `esi` 1 (a stream), `edx` 0 | a file descriptor or an error |
| `connect` | 42 | `edi` the socket, `rsi` the address, `edx` its length (16 for IPv4) | 0 or an error; `EACCES` (-13) for a port the spec doesn't name |
| `sendto` | 44 | `edi` the connection, `rsi` bytes, `rdx` count, `r10d` 0x4000 (`MSG_NOSIGNAL`), `r8` 0, `r9` 0 | bytes sent or an error |
| `shutdown` | 48 | `edi` the connection, `esi` 1 to stop sending | 0 or an error |
| `accept4` | 288 | `edi` 3, `rsi` 0, `rdx` 0, `r10d` 0 | the next connection's file descriptor, or an error |

File descriptors 0, 1 and 2 are standard input, output and error. `openat` flags: `O_RDONLY` 0,
`O_WRONLY` 1, `O_RDWR` 2, `O_CREAT` 0x40, `O_TRUNC` 0x200, `O_APPEND` 0x400; a mode such as 0o644
is 420 in decimal. `write` and `read` may move fewer bytes than asked, so loop until done, as the
template's `write_all` does.

The network: an IPv4 address for `connect` is 16 bytes, a `.short` 2, the port in network byte
order (`rol ax, 8` swaps a port's two bytes), the four address bytes (`127, 0, 0, 1`), then 8
zero bytes. A server never binds or listens: lengua gives it the listening socket as file
descriptor 3, and it takes each connection with `accept4`, reads until it has the request, answers
with `sendto` and `MSG_NOSIGNAL` (with `write`, a client that hangs up early would kill it with
SIGPIPE), and closes the connection. A client reads until `read` returns 0, when the server has
closed its side. Data can arrive in pieces, so never assume one `read` gets it all.

Random bytes and the clock: tests usually supply them in place of the kernel's, so output that
depends on them can be checked exactly. Draw the random bytes the spec's Purpose says how to use,
in any batch size: bytes asked for past what a test supplies are zeros, so only the bytes the
program uses decide its output. Each `clock_gettime` call reads the test's next reading, and the
last one repeats. Check both results like any call's.

Files: open an input with `openat` (-100, name, `O_RDONLY`), and create or replace an output with
`O_WRONLY | O_CREAT | O_TRUNC` (0x241) and mode 420; `close` each when done. Name files relative to
the working folder. The kernel lets a program open files only inside that folder, so any other
path fails with `EACCES` (-13); a missing file fails with `ENOENT` (-2). Each error is a behavior
the spec names, with its own exit code.

## Program start

At `_start` the stack holds the arguments, and `rsp` is 16-byte aligned:

| Address | Holds |
|---|---|
| `[rsp]` | `argc`, the number of arguments including the program's path |
| `[rsp + 8]` | `argv[0]`, the program's path |
| `[rsp + 16]` | `argv[1]`, the first argument, and so on |
| `[rsp + 8 * (argc + 1)]` | 0, the end of the list |
| `[rsp + 8 * (argc + 2)]` | the environment: a pointer to each variable's `NAME=value`, then 0 |

Each argument and variable is a NUL-terminated string, read-only. A program gets environment
variables only when its spec uses `the environment`, and then only those its test or the user's
run gives; otherwise the list is empty. lengua runs programs in a working folder of their own: an empty temporary folder, or the user's folder when they ask to run
the program on their files. `_start` has no return address: it ends with `exit`.

## Calling convention (System V)

- Arguments go in `rdi`, `rsi`, `rdx`, `rcx`, `r8` and `r9`; results come back in `rax`, and in
  `rdx` for a second value.
- A function must give back `rbx`, `rbp`, `r12` to `r15` (and `rsp`) unchanged; it may change
  `rax`, `rcx`, `rdx`, `rsi`, `rdi` and `r8` to `r11`.
- `rsp` is a multiple of 16 at every `call`, so a function starts 8 bytes off (the return
  address).

## Integer facts

- Writing a 32-bit register clears the upper half: `mov eax, 1` sets `rax` to 1, and
  `xor eax, eax` clears `rax`. Writing 8 or 16 bits leaves the rest unchanged, so load bytes with
  `movzx`.
- Signed division: `cqo`, then `idiv` (quotient in `rax`, remainder in `rdx`, truncated toward
  zero). Dividing by zero, or the most negative number by -1, kills the program with SIGFPE.
- Unsigned division: `xor edx, edx`, then `div`.
- `jo` catches signed overflow after `add`, `sub`, `imul` and `neg`; `jc` catches unsigned carry.
- Signed comparisons jump with `jl`, `jle`, `jg` and `jge`; unsigned ones with `jb`, `jbe`, `ja` and
  `jae`. Bounds checks use the unsigned ones, which also catch negative indexes.
- `imul rax, rbx, 10` multiplies by a constant. Characters are numbers: `sub eax, '0'` turns a
  digit's character into its value.
- Exit codes go from 0 to 255; 134 is lengua's code for a failed assertion.

## Vector facts

- Every x86-64 CPU has SSE2: 16 `xmm` registers of 16 bytes. AVX2 adds 32-byte `ymm` forms
  (`vpaddb ymm0, ymm1, ymm2`) and AVX-512 64-byte `zmm` forms, 32 vector registers and the mask
  registers `k0` to `k7`; a `check` reply's `machine` lists which this CPU has.
- No vector or mask register is callee-saved: after a `call` they hold anything. System calls keep
  them.
- Spread a byte over a register without data: `mov eax, 0x0a0a0a0a`, `movd xmm1, eax`,
  `pshufd xmm1, xmm1, 0`. With AVX2: `vpbroadcastb ymm1, xmm1`.
- Find bytes 16 at a time: `pcmpeqb xmm0, xmm1` sets each matching byte to `0xff`, `pmovmskb
  eax, xmm0` gathers one bit per byte, and `bsf` or `tzcnt` gives the first match's index.
- Byte compares are signed (`pcmpgtb`): ASCII letters are below `0x80`, so they compare as
  expected.
- SSE instructions that read 16 bytes of memory need it aligned, except `movdqu` and `lddqu`; the
  `v` (VEX) forms don't. lengua allows only unaligned loads and stores.
- After using 256-bit or 512-bit registers, `vzeroupper` before SSE code avoids a slowdown on
  some CPUs.

## Floating-point facts

- Doubles travel in `xmm` registers: arguments in `xmm0` to `xmm7`, the result in `xmm0`. No
  `xmm` register is callee-saved.
- `cvtsi2sd xmm0, rax` turns a whole number into a double, exactly up to 2^53. `cvtsd2si rax,
  xmm0` rounds to the nearest whole number, ties to even; `cvttsd2si` rounds toward zero.
- After `ucomisd xmm0, xmm1`, jump with the unsigned conditions: `jb` (less), `ja` (greater),
  `je` (equal), and `jp` when either is NaN.
- Constants: `.double 100.0` in `.rodata`, read with `movsd xmm1, qword ptr [rip + hundred]`, or
  `mov eax, 100` and `cvtsi2sd xmm1, eax`.
- "12.34" is 1234 / 100.0: one `divsd` of two exact numbers gives the double the tests use.
- Singles work the same way with `ss` instructions: `cvtsi2ss` is exact up to 2^24, `.float 2.5`
  makes a constant, and `cvtss2sd` widens a single to a double exactly. The low 4 bytes of an
  `xmm` register hold a single.
- `vfmadd231sd xmm0, xmm1, xmm2` sets `xmm0 = xmm1 × xmm2 + xmm0` with one rounding; `132` and
  `213` choose other operand orders.
