# Linux AArch64 reference

For a computer whose `machine` line starts `linux-aarch64`: an ARM64 computer running Linux, or
an x86-64 one testing AArch64 code through QEMU. The rules are the same as x86-64's
([the rules](power-of-10.md)), written in AArch64's instructions; start from
[the AArch64 template](../assets/linux-aarch64/main-template.s), and read standard input with
[AArch64's fill](../assets/linux-aarch64/fill.s), copied word for word with its constants.

What AArch64 code covers so far, checked, tested and proven as x86-64's is: arguments, whole
numbers, floating point in singles and doubles, vectors of whole numbers and of floats (NEON),
text, standard input of any length through `fill`, files and folders, hash tables and word
tallies, sorting, checked jump tables, servers (one client at a time or several at once), clients
of a server on this computer, the terminal, threads, and CRC-32 checksums with the CPU's
instructions. Not yet, so a spec that needs them is compiled for x86-64 for now: the CPU's AES,
SHA and carry-less multiply instructions.

## Templates

Each is x86-64's template compiled for AArch64 from the same spec, checked, tested and proven as
x86-64's is.
- [The template](../assets/linux-aarch64/main-template.s) prints the negative of a number.
- [The vector template](../assets/linux-aarch64/vector-template.s) shouts a line 16 bytes at a
  time with NEON.
- [The streaming template](../assets/linux-aarch64/stream-template.s) capitalizes standard input
  of any length a 4 KiB buffer-full at a time through `fill`, from
  [its spec](../assets/stream-template-spec.md), and is proven for every length.
- [The word tally](../assets/linux-aarch64/tally-template.s) counts words through a hash table
  of a million slots and sorts them with a merge sort, from
  [its spec](../assets/tally-template-spec.md): values the x86-64 form keeps on the stack stay in
  `x19` to `x28`, and constants too wide for `cmp`, such as `MAX_WORDS`, sit in a register built
  with `mov` and `movk`.
- [The event-loop server](../assets/linux-aarch64/server-template.s) serves up to 8 clients at
  once with `ppoll`, from [its spec](../assets/server-template-spec.md), proven in every order
  its clients' steps can come in.
- [The key reader](../assets/linux-aarch64/terminal-template.s) reads keys one at a time with no
  echo, from [its spec](../assets/terminal-template-spec.md).
- [The parallel template](../assets/linux-aarch64/parallel-template.s) capitalizes up to a
  megabyte on up to 16 threads, from [its spec](../assets/parallel-template-spec.md). Copy its
  `parallel_for`, `start_worker` and `join_worker`, their data and constants word for word, as on
  x86-64; `worker` keeps its slot in `x19`, set by `mov x19, x1` after saving x19, and writes
  only its stack and its slot. AArch64's `clone` takes where to clear the thread's id in `x4`, not
  x86-64's fourth argument, and `join_worker` reads the id with `ldar`, a load-acquire, so the
  worker's result can't be read before the id shows it ended: loads on ARM64 can pass each
  other. `ldar` of a whole `w` or `x` register from a base with no offset is the dialect's only
  ordered access.
- [The fast crc32c](../assets/linux-aarch64/fast/crc32c/main.s) prints standard input's CRC-32C
  from [its spec](../assets/fast/crc32c/spec.md), 8 bytes a step with `crc32cx` and the rest with
  `crc32cb`.
- [The fast `wc`](../assets/linux-aarch64/fast/wc/main.s) counts lines and words 16 bytes a step,
  each step masked to the bytes left, proven for every length with a 64 KiB buffer.

## The dialect's form

- Line 1 is exactly `.arch armv8-a`, and comments start with `//`, annotations too:
  `// bound: N` after a loop's label, `// returns: result` in a function's header comment.
- Registers `x0` to `x30`, their low halves `w0` to `w30`, `sp`, and `xzr`; never `x18`, the
  platform's register on Apple's and Windows' ARM64. Floating point is in `d0` to `d31` and
  `s0` to `s31`.
- An address only as `adrp xD, name` and, on the next line, `add xD, xD, :lo12:name`; no `adr`,
  no literal pool (`ldr x0, =value`), and no `:lo12:` inside a load or store.
- A number too wide for one `mov` is a `mov` and `movk`s that each name the same constant:
  `mov x2, #BUF_LEN & 0xffff` then `movk x2, #BUF_LEN >> 16, lsl #16`.
- Indexes are whole `x` registers: `ldrb w3, [x1, x2]`.

## System calls

What the spec names under `# Constraints` decides the calls a program may make, as on x86-64:
`write`, `exit` and `exit_group` always; `read` for `standard input`; `openat`, `read`, `write`,
`lseek`, `close`, `fstat`, `getdents64`, `mkdirat`, `unlinkat` and `renameat` for `files`;
`clock_gettime` and `clock_nanosleep` for `the clock`; `getrandom` for `random bytes`; `socket`,
`connect`, `sendto`, `close` and `read` for the network, `accept4` on the listening socket lengua
hands a server as descriptor 3, and `ppoll` for a server of several clients at once; `ioctl` and
`ppoll` for the terminal; `clone`, `futex` and `exit` for threads, only through the parallel
template's `parallel_for`. The kernel refuses any other.

`mov x8, #SYS_NAME` picks the call, the arguments go in `x0` to `x5`, and `svc #0` runs it. It
returns in `x0` and changes nothing else. A result from -4095 to -1 is an error (the negative
errno), so test it right away with `tbnz x0, #63, error`.

| Call | `SYS_` number | Arguments | Result |
|---|---|---|---|
| `read` | 63 | `x0` fd, `x1` buffer, `x2` count | bytes read, 0 at the end of input, or an error |
| `write` | 64 | `x0` fd, `x1` bytes, `x2` count | bytes written, possibly fewer than asked, or an error |
| `openat` | 56 | `x0` `AT_FDCWD` (-100), `x1` the name, `x2` flags (0 to read; 0x241 to write a new or emptied file), `x3` the new file's mode, such as 420 (`0644`) | the fd or an error |
| `close` | 57 | `x0` fd | 0 or an error |
| `lseek` | 62 | `x0` fd, `x1` offset, `x2` whence (0 start, 1 current, 2 end) | the new offset or an error |
| `fstat` | 80 | `x0` an open file, `x1` 128 bytes for its details: its size is the 8 bytes at 48 | 0 or an error |
| `getdents64` | 61 | `x0` a folder opened with `O_DIRECTORY` (0x4000, not x86-64's 0x10000), `x1` a buffer, `x2` its size | the bytes of entries it filled, as on x86-64 |
| `mkdirat` | 34 | `x0` `AT_FDCWD`, `x1` the folder's name, `x2` its mode | 0, -17 (`EEXIST`) when it's there, or an error |
| `unlinkat` | 35 | `x0` `AT_FDCWD`, `x1` the name, `x2` 0 for a file or 0x200 for an empty folder | 0 or an error |
| `renameat` | 38 | `x0` `AT_FDCWD`, `x1` the old name, `x2` `AT_FDCWD`, `x3` the new name | 0 or an error |
| `clock_gettime` | 113 | `x0` clock (0 the time of day, 1 a clock that only moves forward), `x1` 16 bytes for seconds then nanoseconds | 0 or an error |
| `clock_nanosleep` | 115 | `x0` clock, `x1` 0, `x2` 16 bytes of how long | 0 or an error |
| `getrandom` | 278 | `x0` buffer, `x1` count, `x2` 0 | bytes filled or an error |
| `socket` | 198 | `x0` 2 (`AF_INET`), `x1` 1 (`SOCK_STREAM`), `x2` 0 | the socket or an error |
| `connect` | 203 | `x0` the socket, `x1` 16 bytes of address (family, port high byte first, 127.0.0.1), `x2` 16 | 0 or an error |
| `accept4` | 242 | `x0` 3, `x1` to `x3` 0 | the connection or an error |
| `sendto` | 206 | `x0` the connection, `x1` bytes, `x2` count, `x3` 0x4000 (`MSG_NOSIGNAL`), `x4` and `x5` 0 | bytes sent or an error |
| `ppoll` | 73 | `x0` a table of 8-byte entries (descriptor, events, events that came), `x1` how many, `x2` 0 to wait until one is ready, or 16 bytes of how long, `x3` 0 | how many are ready, 0 when the time ran out, or an error |
| `ioctl` | 29 | `x0` 0, `x1` `TCGETS` (0x5401) or `TCSETS` (0x5402), `x2` 36 bytes of the terminal's mode | 0, or an error when standard input isn't a terminal |
| `exit` | 93 | `w0` status | doesn't return |

AArch64 has no `poll`: `ppoll` is the wait, with x86-64's `poll` table. Its time is a pointer, so
"until one is ready" is `x2` 0, not -1, and the kernel writes the time left back into the 16
bytes it read: keep them in `.bss` and set them again before each wait, as
[the terminal template](../assets/linux-aarch64/terminal-template.s) does. A native kernel
skips the write to `.rodata`, but QEMU refuses the wait. The port in `connect`'s address goes high byte first: `rev16 w12, w10`
turns a port in `w10` around.

## Program start

At `_start` the stack holds the arguments, and `sp` is 16-byte aligned:

| Address | Holds |
|---|---|
| `[sp]` | `argc`, the number of arguments including the program's path |
| `[sp, #8]` | `argv[0]`, the program's path |
| `[sp, #16]` | `argv[1]`, the first argument, and so on |

Each argument is a NUL-terminated string, read-only. `_start` has no return address: it ends
with `exit`, and keeps what it needs across calls in `x19` to `x28` without saving them.

## Calling convention (AAPCS64)

- Arguments go in `x0` to `x7`; results come back in `x0`, and in `x1` for a second value.
- `bl` puts the return address in `x30`. A function that calls anything but `assert_fail`
  opens with `stp x29, x30, [sp, #-16]!` and closes with `ldp x29, x30, [sp], #16` right before
  its one `ret`.
- A function must give back `x19` to `x28`, `d8` to `d15` (and `sp`) unchanged: save the ones it
  uses in pairs, `stp x19, x20, [sp, #-16]!` after the opening and `ldp x19, x20, [sp], #16`
  before the closing, in mirrored order, and `d8` to `d15` the same way after the `x` pairs. It
  may change `x0` to `x17` and the other `d` registers.
- `sp` moves only by multiples of 16. `ret` returns through `x30` only; no `blr`, and `br` only
  as a checked jump table's jump.

## Idioms the rules read

- **Loops:** `mov xR, #NAME` just before the head, `.Lhead:  // bound: N` with `N` the constant's
  value, and the loop closed by `subs xR, xR, #1` and `b.ne .Lhead` (or `sub` and `cbnz`; never
  `sub` and `b.ne`, which test stale flags).
- **Bounds:** `cmp xI, #LIMIT` and `b.hs` to an assertion before `[xB, xI]`, `LIMIT` an encodable
  number or a register holding a named constant.
- **Results:** the first instruction to touch `x0` after a call to a `// returns: result`
  function, or after `svc`, is `tbnz x0, #63, …`, `cmp x0, #0` with `b.lt`, or `cbz`/`cbnz x0`.
- **Assertions:** `mov w0, #N` and `bl assert_fail`, each number reached from one place, at least
  two a function on average, and below 990, fill's own.
- **Exit codes:** `mov w0, #EXIT_NAME`.

## CRC-32 facts

- The CRC-32 instructions are an extension: write `.arch_extension crc` on the line after
  `.arch armv8-a`, and check holds them to a CPU that has them, as the machine line lists (`crc`;
  QEMU's CPU has them).
- `crc32cx w0, w0, x13` takes 8 bytes of x13 into the CRC-32C state in w0, lowest byte first, as
  x86-64's `crc32 rax, qword ptr […]` does; `crc32cb`, `crc32ch` and `crc32cw` take 1, 2 and 4
  bytes from a `w` register. `crc32b` to `crc32x` are the same in the IEEE polynomial, zip's and
  PNG's. The state starts as all ones and ends inverted (`mvn w0, w20`).
- A step's 8-byte load is checked on its start: give the buffer 8 bytes past its last, so
  `cmp x10, #BUFFER_FULL` and `b.hi` hold it inside, as the fast crc32c does.

## Integer facts

- Writing a `w` register clears the upper half: `mov w0, #1` sets `x0` to 1.
- `udiv` and `sdiv` give the quotient only; the remainder is `msub` of it (`msub x13, x12, x11, x0`
  is `x0 - x12 × x11`), and the proof reads that as `%`. Dividing by zero gives 0; nothing traps.
- `adds` and `subs` set the flags; `b.vs` catches signed overflow, `b.hs`/`b.lo` compare unsigned
  (the carry is "no borrow" after a subtraction), `b.ge`, `b.lt`, `b.gt` and `b.le` signed.
- A product's overflow: `mul` for the low half, `smulh` for the high, and the product fits where
  the high half equals the low half's sign, `cmp x13, x12, asr #63`.
- `cset w5, lo` makes a condition 0 or 1, and `csel` chooses without a branch: a byte's capital is
  `sub w4, w4, w5, lsl #5` after `cset` of whether it's a letter, with no branch on the byte, which
  keeps a proof's paths to one.
- Exit codes go from 0 to 255; 134 is lengua's code for a failed assertion.

## Vector facts

- NEON's 32 vector registers are 16 bytes each: `v0.16b` names v0's 16 bytes, `v0.8h` its 8
  halfwords, `v0.4s` its 4 words and `v0.2d` its 2 doublewords; `8b`, `4h` and `2s` name the low
  8 bytes alone. A write of any arrangement clears the rest of the register. `q0` names a whole
  register in loads and stores, and `v1.b[3]` one lane.
- `d8` to `d15`, the low halves of `v8` to `v15`, are all a call keeps: a function that changes
  `v8` to `v15` saves `d8` to `d15` (see floating point below), and after a `bl` every other
  vector register holds anything. System calls keep them all.
- Load and store 16 bytes with `ldr q0, [x1, x2]` and `str q0, [x1, x2]`, after `cmp x2, LIMIT`
  and `b.hs` away with `LIMIT` the buffer's size less 15, so the whole vector stays inside; a
  limit too wide for `cmp`'s number is a named constant in a register. A pointer moved by `add`,
  `sub` or an access's writeback (`ldrb w3, [x1], #1`) still points into its buffer, as the rules
  follow it.
- A number in every lane: `movi v1.16b, #NEWLINE`; a register's low byte in every lane: `dup
  v1.16b, w4`. A table of bytes, such as each lane's place, comes from `.byte` in `.rodata`,
  `ldr q16, [x3]`.
- A comparison sets each lane to all ones where it holds, else zero: `cmeq` (equal), `cmhi` and
  `cmhs` (higher, and higher or the same, without a sign), `cmgt` and `cmge` (with one), and
  their forms with `#0`. `and`, `orr`, `bic` and `eor` join them.
- Count a comparison's lanes: `ushr v1.16b, v1.16b, #7` makes each 1 or 0, `addv b1, v1.16b`
  adds them, and `umov w6, v1.b[0]` takes the sum.
- Find the first matching byte: `shrn v0.8b, v0.8h, #4` packs each byte's comparison into 4 bits
  of a doubleword, `fmov x0, d0` takes it, and `rbit` then `clz` give 4 times the byte's place.
- Mask a loop's last step to the bytes left, so it's the same code: `dup` the bytes left,
  `min(left, 16)`, over a vector and `cmhi` it against each lane's place, then `and` each
  comparison with that mask, as [the fast wc](../assets/linux-aarch64/fast/wc/main.s) does.
- The byte before each of a step's bytes, the last step's carried in: `ext v3.16b, v21.16b,
  v2.16b, #15`, with `v21` the step before. `tbl v4.16b, { v2.16b }, v4.16b` picks the lanes
  `v4` names.
- [The vector template](../assets/linux-aarch64/vector-template.s) finds a newline and
  capitalizes a line 16 bytes at a time, from [its spec](../assets/vector-template-spec.md).
- Floats on vectors are singles two or four at a time (`v0.2s`, `v0.4s`) or doubles two at a
  time (`v0.2d`), each lane computed as the scalar instruction computes it, NaNs and zeros' signs
  included: `fadd`, `fsub`, `fmul`, `fdiv`, `fmax`, `fmin`, `fmaxnm`, `fminnm`, `fabd`, `fmla`
  and `fmls` (fused, the destination the addend), `fabs`, `fneg`, `fsqrt`, `frintn`, `frinta`,
  `frintp`, `frintm`, `frintz`. Comparisons give all ones or zero, false for a NaN: `fcmeq`,
  `fcmge`, `fcmgt`, the magnitudes' `facge` and `facgt`, and each with `#0.0`. Lanes convert to
  and from whole numbers as wide (`fcvtzs`, `fcvtzu`, `scvtf`, `ucvtf`), and between singles and
  doubles (`fcvtl`, `fcvtl2`, `fcvtn`, `fcvtn2`). `faddp`, `fmaxp`, `fminp`, `fmaxnmp` and
  `fminnmp` pair neighbouring lanes, or two lanes into a scalar (`faddp d0, v0.2d`); `fmaxv`,
  `fminv`, `fmaxnmv` and `fminnmv` reduce four singles. `fmov v0.4s, #1.0` puts a number `fmov`
  can hold in every lane.
- Two numbers as one vector of doubles: `fmov d0, x0`, then `mov v0.d[1], x1`, then `scvtf
  v0.2d, v0.2d`; a scalar in every lane: `dup v1.2d, v2.d[0]`.
- Not in the dialect: half precision (`v0.8h` of floats), the estimates (`frecpe`, `frsqrte`,
  `frecps`, `frsqrts`, `fmulx`), the conversions that round otherwise than toward zero (round
  first with an `frint`), the interleaving `ld2` to `ld4`, the single-lane loads, and SVE.

## Floating-point facts

- Doubles are `d0` to `d31` and singles `s0` to `s31`: the low 64 and 32 bits of the vector
  registers `v0` to `v31`, and writing one clears the rest of its vector register. Arguments go
  in `d0` to `d7`, the result in `d0`.
- `d8` to `d15` are callee-saved: a function that changes one saves it in a pair after its frame
  record, `stp d8, d9, [sp, #-16]!`, and restores it with `ldp d8, d9, [sp], #16` before
  `ldp x29, x30, [sp], #16`. A call changes every other one.
- `scvtf d0, x1` turns a whole number into a double, exactly up to 2^53; `ucvtf` reads it
  without a sign. `fcvtzs x0, d0` rounds toward zero; for the nearest, ties to even, as `fixed`
  and `round` round, `frintn d0, d0` first. Out of range `fcvtzs` gives the nearest end, and a
  NaN gives 0.
- `fmov d0, #1.5` holds ±16 to 31 sixteenths times 2^-3 to 2^4; any other number comes from a
  whole one, `mov x1, #100` and `scvtf d1, x1`, or from `.double 100.0` in `.rodata`, read with
  `adrp`, `add :lo12:` and `ldr d1, [x1]`.
- After `fcmp d0, d1`, branch with `b.mi` (less), `b.ls` (less or equal), `b.eq` (equal),
  `b.ge` (greater or equal), `b.gt` (greater), and `b.vs` when either is a NaN: each of these is
  false for a NaN but `b.vs`. `b.lt`, `b.le`, `b.hi`, `b.hs` and `b.ne` are true for one.
- "12.34" is 1234 / 100.0: `scvtf` both numbers and `fdiv` once.
- `fmadd d0, d1, d2, d3` is `d1 × d2 + d3` with one rounding: use it exactly where the tests'
  formula says `fma(a, b, c)`, and `fmul` then `fadd` for `a * b + c`.
- Singles work the same way with `s` registers; `fcvt d0, s0` widens a single exactly, and
  `fcvt s0, d0` rounds a double to the nearest single.
- Also there: `fabs`, `fneg`, `fsqrt`, `fmax` and `fmin` (a NaN comes through), `fmaxnm` and
  `fminnm` (a quiet NaN loses to a number), `fnmul`, `fmsub`, `fnmadd`, `fnmsub`, `fcsel`, `fmov`
  between `d` and `x` registers or `s` and `w`, `frintp`, `frintm`, `frintz` and `frinta`, and
  `ldr`, `str`, `ldp` and `stp` of `s` and `d` registers.
- Left out: half precision (`h` registers), whole vector registers (`q` and `v`, which NEON
  brings later), changes to FPCR (rounding stays nearest, ties to even) and the estimates
  (`frecpe`, `frsqrte`).
