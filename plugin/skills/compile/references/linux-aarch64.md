# Linux AArch64 reference

For a computer whose `machine` line starts `linux-aarch64`: an ARM64 computer running Linux, or
an x86-64 one testing AArch64 code through QEMU. The rules are the same as x86-64's
([the rules](power-of-10.md)), written in AArch64's instructions, and a program calls the same
operations and library functions ([the edge](edge.md)); this page is what is AArch64's. Start from
[the AArch64 template](../assets/linux-aarch64/main-template.s), and read standard input with
`fill`.

What AArch64 code covers so far, checked, tested and proven as x86-64's is: arguments, whole
numbers, floating point in singles and doubles, vectors of whole numbers and of floats (NEON),
text, standard input of any length through `fill`, files and folders, hash tables and word
tallies, sorting, checked jump tables, servers (one client at a time or several at once), clients
of a server on this computer, the terminal, threads, CRC-32 checksums with the CPU's
instructions, carry-less multiplication (`pmull`), the CPU's AES and SHA instructions, and SVE's
scalable vectors of bytes.

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
  once with `os_poll`, from [its spec](../assets/server-template-spec.md), proven in every order
  its clients' steps can come in.
- [The key reader](../assets/linux-aarch64/terminal-template.s) reads keys one at a time with no
  echo, from [its spec](../assets/terminal-template-spec.md).
- [The parallel template](../assets/linux-aarch64/parallel-template.s) capitalizes up to a
  megabyte on up to 16 threads, from [its spec](../assets/parallel-template-spec.md). It calls
  `parallel_for`, which is lengua's, as on x86-64; `worker` keeps its slot in `x19`, set by
  `mov x19, x1` after saving x19, and writes only its stack and its slot. lengua's code does the
  threads' part that is AArch64's: `clone` takes where to clear the thread's id in `x4`, and the
  join reads the id with `ldar`, a load-acquire, so a worker's result can't be read before the id
  shows it ended: loads on ARM64 can pass each other. `ldar` of a whole `w` or `x` register from a
  base with no offset is the dialect's only ordered access.
- [The crc32c](../assets/linux-aarch64/fast/crc32c/main.s) prints standard input's CRC-32C from
  [its spec](../assets/fast/crc32c/spec.md) with no CRC32 instruction, which only ARMv8.1 and
  later have, so it runs on every ARM64 computer: a byte at a time through a table of 256 words,
  a 64 KiB piece of standard input at a time through `fill`.
- [The crc32c-crc tier](../assets/linux-aarch64/fast/crc32c-crc/main.s), beside a `spec.md` that
  is the same bytes, is the form for a CPU that lists `crc` and `pmull` on the machine line, each
  read's bytes as they come: 64 bytes a step with `ldp` and `crc32cx` while that many are left,
  then 8 bytes a step, and the rest with `crc32cb`.
- [The sha256-sha2](../assets/linux-aarch64/fast/sha256-sha2/main.s) and
  [aes-aes](../assets/linux-aarch64/fast/aes-aes/main.s) tiers are the SHA-256 and AES-128
  programs with the SHA2 and AES instructions, for a CPU that lists `sha` or `aes` on the machine
  line, each beside a `spec.md` of the same bytes as its baseline's. The baselines, plain integer
  code that runs everywhere (`tests/fixtures/linux-aarch64/sha256` and `aes` in the repository),
  are `ror` and `eor` with a rotated register, `ubfx` for a byte and, for AES, a table of 256
  words.
- [The fast `wc`](../assets/linux-aarch64/fast/wc/main.s) counts lines and words 16 bytes a step,
  each step masked to the bytes left, proven for every length with a 64 KiB buffer;
  [its SVE form](../assets/linux-aarch64/fast/wc-sve/main.s) does it in the scalable vectors the
  CPU has.
- [The fast histogram](../assets/linux-aarch64/fast/histogram/main.s) counts the bytes of
  standard input into eight tables of 256 counts, eight bytes a step, the k-th byte of each step
  into table k, so a step's bytes never wait on one counter.

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

## Calling an operation

An operation's arguments go in `x0` to `x5` (a 32-bit argument such as a descriptor in the low
half, `w0`), and its result comes back in `x0`. It changes `x0`, `x8`, `x16`, `x17`, `x30`, the
flags, the vector registers `v0` to `v7` and `v16` to `v31` (and the upper halves of `v8` to
`v15`), and SVE's `z`, `p` and `ffr` registers, and no other register: a counter in `x19` or a
pointer in `x1` is as it was after the call. A result below zero is an error, so test it right
away with `tbnz x0, #63, error`:

```
    mov x0, #STDOUT
    adrp x1, greeting
    add x1, x1, :lo12:greeting
    mov x2, #GREETING_LEN
    bl os_write
    tbnz x0, #63, .Lmain_write_failed
```

`bl` puts the return address in `x30`, which an operation changes: a function that calls one saves
`x30` as any caller does (`stp x29, x30, [sp, #-16]!`, below). The registers a call changes can't be
read after it until something sets them. A vector constant kept across a call is loaded again
after it. The library's functions (`fill`, `assert_fail`, `parallel_for`) take their arguments by
the calling convention below.

AArch64 has no `poll`, and the operation hides what that means: `os_poll` takes the same table
as x86-64's and a time in milliseconds, -1 for no limit, whatever the kernel's `ppoll` wants.

## Program start

lengua's `_start` calls `main`, so `main` is entered as a function is: `x0` holds `argc`, the
number of arguments including the program's path; `x1` the address of an array of pointers to
them, `argv[0]` the program's path, ending in 0; and `x2` the address of an array of pointers to
the environment, each variable's `NAME=value`, ending in 0. Each is a NUL-terminated string,
read-only. `sp` is 16-byte aligned. A program gets environment variables only when its spec uses
`the environment`, and then only those its test or the user's run gives; otherwise the list is
empty. lengua runs programs in a working folder of their own: an empty temporary folder, or the
user's folder when they ask to run the program on their files. `main` has nothing to return to:
it ends with `bl os_exit`, and keeps what it needs across calls in `x19` to `x28` without saving
them.

## Calling convention (AAPCS64)

- Arguments go in `x0` to `x7`; results come back in `x0`, and in `x1` for a second value.
- `bl` puts the return address in `x30`. A function that calls anything but `assert_fail`, an
  operation included, opens with `stp x29, x30, [sp, #-16]!` and closes with `ldp x29, x30, [sp], #16` right before
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
  function or an operation (`bl os_…`), is `tbnz x0, #63, …`, `cmp x0, #0` with `b.lt`, or `cbz`/`cbnz x0`.
- **Assertions:** `mov w0, #N` and `bl assert_fail`, each number reached from one place, at least
  two a function on average, numbered 1 to 999 as on x86-64, each once; lengua's own, in `fill` and
  `parallel_for`, are 990 and up, so keep a program's below them.
- **Checked jump tables:** a `switch` over a dense range of small numbers jumps through a table
  of the function's own labels in `.rodata` (`.long .Lcase - table` entries), the index bounds-
  checked as any index is, as the calculator fixture does:
  `sub w9, w9, #FIRST`, `cmp x9, #COUNT`, `b.hs .Lunknown`, `adrp x10, table`,
  `add x10, x10, :lo12:table`, `ldrsw x11, [x10, x9, lsl #2]`, `add x11, x11, x10`, `br x11`
  ([x86-64's form](power-of-10.md) in AArch64's instructions).
- **Exit codes:** `mov w0, #EXIT_NAME`.

## The baseline

The baseline is what every ARM64 machine lengua is written for has: ARMv8.0-A with NEON and
floating point, and nothing optional. Write for it first: NEON's 16-byte vectors, the integer and
floating-point instructions above, and `.arch armv8-a` alone. Everything else the `machine` line
lists is an extension: the CRC-32 instructions (`crc`), the carry-less multiply of doublewords
(`pmull`), the AES instructions (`aes`), the SHA-1 and SHA-2 instructions (`sha`) and SVE (`sve`,
with the vector length). A program that needs one writes `.arch_extension` for it on the line
after `.arch armv8-a` and is a fast tier: a program of its own, named for the extension (the SVE
`wc` is `wc-sve`), beside the baseline form and from the very same spec, which you start from
only where `machine` lists the extension. `check` refuses an instruction the machine lacks
(`needs CRC, which this CPU doesn't have`), and lengua's own programs are held to the baseline by
`tests/baseline.rs`.

## CRC-32 facts

- The CRC-32 instructions are an extension: write `.arch_extension crc` on the line after
  `.arch armv8-a`, and check holds them to a CPU that has them, as the machine line lists (`crc`;
  QEMU's CPU has them).
- `crc32cx w0, w0, x13` takes 8 bytes of x13 into the CRC-32C state in w0, lowest byte first, as
  x86-64's `crc32 rax, qword ptr […]` does; `crc32cb`, `crc32ch` and `crc32cw` take 1, 2 and 4
  bytes from a `w` register. `crc32b` to `crc32x` are the same in the IEEE polynomial, zip's and
  PNG's. The state starts as all ones and ends inverted (`mvn w0, w20`).
- A step's 8-byte load is checked on its start: give the buffer 8 bytes past its last, so
  `cmp x10, #BUFFER_FULL` and `b.hi` hold it inside, as the crc32c-crc tier does.
- One chain of `crc32cx` ran at one a cycle on GitHub's ARM64 runners (Neoverse N2), whose CRC
  unit hands each result straight to the next, so there the loop's own instructions are what slow
  it: take 64 bytes a step, `ldp x14, x15, [x13, #16]` through a pointer `add` moved into the
  buffer, as the crc32c-crc tier does. A load through a moved pointer has no index for `cmp` to
  check: its bounds are the proof's to show.
- `pmull v0.1q, v1.1d, v2.1d` multiplies v1's and v2's low doublewords as polynomials over GF(2),
  carries dropped, into all 16 bytes of v0, as x86-64's `pclmulqdq` does; `pmull2 v0.1q, v1.2d,
  v2.2d` multiplies their high ones. That form is the PMULL extension, which LLVM files under AES:
  write `.arch_extension aes` after `.arch_extension crc`, and check holds it to a CPU that has it
  (`pmull` on the machine line). The bytes' form, `pmull v0.8h, v1.8b, v2.8b` and `pmull2 v0.8h,
  v1.16b, v2.16b`, eight products of bytes into halfwords, is in every ARM64 CPU.
- Past the CRC unit's one a cycle, the vector units take a share: the crc32c-crc tier runs its chain
  over the first half of each 32 KiB block while `pmull` and `pmull2` fold the second half into
  four vectors, 64 bytes a step, then folds the four into one, takes its CRC with two `crc32cx`
  from a state of 0, and joins the chain's state to it, moved past the half by one more `pmull`
  and a `crc32cx`. What each fold multiplies by is a power of x mod the polynomial, bit-reflected,
  in `.rodata`.

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
  vector register holds anything. So does the upper half of `v8` to `v15` after a call of an
  operation (`os_*`, or `fill` and `parallel_for`, which call them), as do `v0` to `v7` and `v16`
  to `v31`: reload what you need from memory.
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
  first with an `frint`), the interleaving `ld2` to `ld4`, and the single-lane loads. SVE has its
  own section below.

## AES and SHA facts

- AArch64's AES and SHA instructions are extensions: write `.arch_extension aes` and
  `.arch_extension sha2` with the others, and check holds them to a CPU that has them (`aes` and
  `sha` on the machine line).
- AES, a round at a time on a vector of 16 bytes: `aese v0.16b, v1.16b` XORs the key in v1 into
  the state in v0, then SubBytes and ShiftRows; `aesmc v0.16b, v0.16b` is MixColumns, so a round
  is `aese` then `aesmc`, and the last `aese` then an `eor` of the last key. `aesd` and `aesimc`
  are the inverse.
- SHA-1 and SHA-256, four rounds at a time: `sha1c q0, s1, v2.4s` (and `sha1p`, `sha1m`, each
  with its round function) takes the state's a to d in q0, e in s1, and the message words with
  their constants added in v2; `sha1h s1, s0` rotates a word; `sha1su0` and `sha1su1` step the
  message schedule. `sha256h q0, q1, v2.4s` and `sha256h2 q1, q2, v2.4s` take the state's two
  halves, and `sha256su0` and `sha256su1` its schedule. The test language's `sha256` checks
  what a program computes with them.

## SVE facts

- SVE's vectors are as long as the CPU makes them, 16 to 256 bytes: 16 on GitHub's ARM64
  runners (Neoverse N2), 64 under QEMU here; lengua's model holds up to 64. When the CPU has SVE
  the machine line says so, with the length. Write `.arch_extension sve` with the other extension
  lines. A proof holds at the length it ran at: the lock records it, and `run` refuses a CPU with
  another until check runs there.
- The dialect's SVE is the integer subset of bytes a streaming loop needs: every element is a
  byte (`z0.b`), but for the bitwise operations of whole registers, written as doublewords (`and
  z0.d, z1.d, z2.d`, `orr`, `eor`, `bic`, and `mov z0.d, z1.d`). A NEON write clears the Z
  register above its 16 bytes, as the CPU does; z8 to z15's low halves are d8 to d15, which a call
  keeps.
- Predicates p0 to p15 hold a bit for each byte. A load, a store, a compare, `cnt`, `uaddv` and
  `lastb` take a governing predicate of p0 to p7: `p0/z` zeros the inactive lanes, `p0/m` keeps
  them.
- A loop over a piece: `whilelo p0.b, x10, x1` makes the lanes the bytes left and sets the
  flags, so `b.eq` leaves when none is left; `incb x10` steps the index a vector. `ld1b { z0.b },
  p0/z, [x0, x10]` loads the active lanes and zeros the rest; `st1b { z0.b }, p0, [x0, x10]`
  stores them. The rules hold every SVE access to 64 bytes, the longest vector, so it fits every
  CPU: give the buffer 64 bytes of room past its last, and `cmp x10, #BUF_LEN` with `b.hs` to an
  assertion before the access, as [the SVE wc](../assets/linux-aarch64/fast/wc-sve/main.s) does.
- A compare sets a predicate's lanes where `p0` is active: `cmpeq p1.b, p0/z, z0.b, #10`, with
  numbers from -16 to 15 for `eq`, `ne`, `ge`, `gt`, `lt` and `le` and from 0 to 127 for `hs`,
  `hi`, `lo` and `ls`, or against a vector's lanes (`mov z18.b, #32` first). Its flags say of
  its result what `whilelo`'s do. Predicates join with `and`, `orr`, `eor` and `bic p0.b, p1/z,
  p2.b, p3.b`, and `not p0.b, p1/z, p2.b`; `cntp x5, p0, p1.b` counts the lanes active in both.
- A vector's bytes: `mov z1.b, #10`, `mov z1.b, w3`, `mov z1.b, p2/z, #-1` (a number in the active
  lanes, zero elsewhere), `sel z0.b, p0, z1.b, z2.b`, `add` and `sub`, `cnt z0.b, p0/m, z1.b`
  (each byte's set bits), `uaddv d0, p0, z1.b` (the active bytes' sum, in d0), `insr z1.b, w12`
  (each lane moved up one, w12's byte in lane 0, as the byte before a step), `lastb w13, p0,
  z1.b` (the last active lane's byte), and `cntb x0` (the vector's length).
- Not in the dialect: elements wider than a byte, first-fault and gathering loads, the
  first-fault register, the flag-setting forms of predicate logic (`ands`, `ptrues`), and SVE's
  other instructions.
- [The SVE wc](../assets/linux-aarch64/fast/wc-sve/main.s) counts lines and words a vector a
  step, from [the wc's spec](../assets/fast/wc/spec.md), proven for every length.

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
