# Linux AArch64 reference

For a computer whose `machine` line starts `linux-aarch64`: an ARM64 computer running Linux, or
an x86-64 one testing AArch64 code through QEMU. The rules are the same as x86-64's
([the rules](power-of-10.md)), written in AArch64's instructions; start from
[the AArch64 template](../assets/linux-aarch64/main-template.s), and read standard input with
[AArch64's fill](../assets/linux-aarch64/fill.s), copied word for word with its constants.

What AArch64 code covers so far, checked, tested and proven as x86-64's is: arguments, whole
numbers, floating point in singles and doubles, text, standard input of any length through
`fill`, and files and folders. Not yet, so a spec that needs them is compiled for x86-64 for now:
vectors (NEON), threads, servers and several clients at once, the terminal, and the checksum and
crypto instructions.

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
`clock_gettime` and `clock_nanosleep` for `the clock`; `getrandom` for `random bytes`. The kernel
refuses any other.

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
| `exit` | 93 | `w0` status | doesn't return |

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
