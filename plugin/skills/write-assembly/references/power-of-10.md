# The lengua Power of 10 (dialect 2)

`main.s` is x86-64 assembly for Linux in Intel syntax. `check` builds it, reads the machine
code back, matches it to `main.s` line by line (operands included), and checks the rules below.
Nothing runs before every rule passes. Each violation names its rule, its line and a fix.

These are NASA's "Power of 10" rules for safety-critical code (Holzmann, 2006), adapted for
assembly and combined with ideas from C, C++ and Rust.

## Layout

- **The first line** is `.intel_syntax noprefix`; only comments may come before it.
- **Constants come next**, as `.equ NAME, value` lines with comments between them. Names are
  `UPPER_CASE`. A value is a number (decimal, `0x` hex, or a character such as `'0'`) or earlier
  constants combined with `+ - * /` and parentheses. Constants never hold addresses (`.`) or text.
  The assembler silently reads a constant defined after its use as a memory address, so all of
  them live in this block.
- **Then the sections:** `.section .rodata`, `.section .data`, `.section .bss` and `.text`.
- **A data symbol:**

  ```
  .type usage_msg, @object
  usage_msg:
      .ascii "usage: negate <n>\n"
  .size usage_msg, . - usage_msg
  ```

  Data directives: `.ascii`, `.asciz`, `.byte`, `.short`, `.long`, `.quad` (numbers only),
  `.double` and `.float` (plain decimals, such as `100.0`),
  `.zero NAME` and `.skip NAME` (sizes are named constants), and `.balign N`. `.bss` holds only
  `.zero` and `.skip`. `.balign N` (1 to 64) between `.type name, @object` and `name:` makes the
  symbol start at a multiple of N.
- **A function:**

  ```
  # returns: result
  .type parse_i64, @function
  parse_i64:
      ... one instruction per line, local labels (.Lname:) on their own lines ...
  .size parse_i64, . - parse_i64
  ```

  `# returns: result` (optional, right above `.type`) marks a function whose `rax` is negative on
  error.
- **`.globl _start`** in `.text`; nothing else is exported.
- **One instruction per line.** Not allowed: `;`, `/* */`, a label and an instruction on one
  line, numbered labels (`1:`), prefixes (`rep`, `lock`), `offset`, segment registers, macros and
  their kin (`.macro`, `.rept`, `.irp`, `.if`, `.include`, `.set`), and anything but instructions
  in `.text`.

## Operands

| Kind | Examples |
|---|---|
| Register | `rax`, `eax`, `ax`, `al`, `r8`, `r8d`, `r8w`, `r8b`; vectors `xmm0`–`xmm31`, `ymm0`–`ymm31`, `zmm0`–`zmm31`; masks `k0`–`k7` |
| Number | `10`, `0x1f`, `'0'`, `MAX_DIGITS`, `OUT_BUF_LEN - 1` |
| Memory | `qword ptr [rsp + 16]`, `byte ptr [rdi + rcx]`, `[rbx + rcx*8 + 8]`, `qword ptr [rip + out_buf]`, `xmmword ptr [rsi + rcx]` |
| Target | `.Lloop` (jumps), `parse_i64` (calls) |

Memory needs a base register. Data symbols are reached only as `[rip + name]`, plus an optional
constant, and `lea reg, [rip + name]` loads an address. Write the size (`byte ptr`, `word ptr`,
`dword ptr`, `qword ptr`, and for vectors `xmmword ptr` (16 bytes), `ymmword ptr` (32) and
`zmmword ptr` (64)) when no register operand gives it.

## Instruction set

Dialect 2 is integer instructions, `syscall`, vector instructions and floating point in double
and single precision:

- `mov movzx movsx movsxd lea push pop xchg bswap` and every `cmovCC`
- `add adc sub sbb neg inc dec mul imul div idiv cbw cwde cdqe cwd cdq cqo`
- `and or xor not test cmp shl sal shr sar rol ror shld shrd bt bts btr btc bsf bsr` and every
  `setCC`
- `jmp`, every `jCC`, `call ret syscall nop`

When the CPU has them (the `machine` line of a `check` reply lists its features): BMI1 (`andn bextr blsi blsmsk blsr
tzcnt`), BMI2 (`bzhi mulx pdep pext rorx sarx shlx shrx`), POPCNT (`popcnt`), LZCNT (`lzcnt`),
ADX (`adcx adox`) and MOVBE (`movbe`).

### Vectors

Integer vector instructions on `xmm` (16 bytes), `ymm` (32) and `zmm` (64) registers. SSE2 is in
every x86-64 CPU; the rest only when `check`'s `machine` lists them: SSE3, SSSE3, SSE4.1, SSE4.2, AVX, AVX2
and AVX-512 (F, BW, CD, DQ, VL, VBMI, VBMI2, VNNI, BITALG, VPOPCNTDQ):

- loads and stores: `movdqu`, `vmovdqu` (and `vmovdqu8` … `vmovdqu64`), `lddqu`; between
  general-purpose and vector registers: `movd`, `movq`, `pextrb`…`pextrq`, `pinsrb`…`pinsrq`,
  `pmovmskb`
- arithmetic: `padd*`, `psub*` (also saturating), `pmull*`, `pmulh*`, `pmuludq`, `pmaddwd`,
  `pmaddubsw`, `psadbw`, `pavg*`, `pabs*`, `pmin*`, `pmax*`, `phadd*`, `phsub*`
- logic and compares: `pand pandn por pxor`, `vpternlog*`, `pcmpeq*`, `pcmpgt*`, `vpcmp*`
  (into mask registers), `vptestm*`, `ptest`, and the SSE4.2 string compares `pcmpistri` and
  friends
- shifts and shuffles: `psll*`, `psrl*`, `psra*`, `pslldq`, `psrldq`, `vpsllv*`, `vprol*`,
  `pshufb`, `pshufd`, `palignr`, `punpck*`, `pack*`, `pblendw`, `pblendvb`, `vpblendd`,
  `vperm*`, `vinserti*`, `vextracti*`, `vpbroadcast*`, `vbroadcasti*`
- widening and narrowing: `pmovzx*`, `pmovsx*`, `vpmov*`
- AVX-512 mask registers: `kmov*`, `kand*`, `kor*`, `kxor*`, `knot*`, `kortest*`, `ktest*`,
  `kshift*`, `kunpck*`, `vpmovm2*`, `vpmov*2m`; masked blends, compress and expand:
  `vpblendm*`, `vpcompress*`, `vpexpand*`
- bit counts and dot products: `vpopcnt*`, `vplzcnt*`, `vpconflict*`, `vpdpbusd`, `vpdpwssd`
- `vzeroupper`, `vzeroall`, and `crc32`
- hashing, checksums and ciphers, when the CPU has them: AES-NI and VAES (`aesenc`, `aesdec`,
  `aesimc`, `aeskeygenassist` and their `v` forms), SHA (`sha1*`, `sha256*`), PCLMULQDQ and
  VPCLMULQDQ (`pclmulqdq`), GFNI (`gf2p8affineqb`, `gf2p8affineinvqb`, `gf2p8mulb`). The test
  language has `sha256`, `crc32` and `crc32c` to check such programs.

Rules for vectors:

- **Unaligned only.** Load and store with `movdqu`/`vmovdqu`; aligned moves (`movdqa`,
  `vmovdqa`) fault on an unaligned address and aren't in the dialect. An SSE instruction (no `v`)
  may read 16 bytes of memory only as `movdqu` or `lddqu`, because the others need alignment:
  load into a register first, or use the `v` form when the CPU has AVX.
- **Masks and broadcasts.** AVX-512 write masks (`vpaddd zmm0 {k1}, zmm1, zmm2`, with `{z}` to
  zero the lanes the mask leaves out) and broadcasts (`dword ptr [rax]{1to16}`) are in the
  dialect, and so are `vpblendm*`, `vpcompress*` and `vpexpand*`. A masked load or store still
  counts its full width for bounds checks, so size buffers in whole vectors. Rounding overrides
  (`{rn-sae}`) aren't in the dialect.
- **Left out:** gathers and scatters (their addresses can't be bounds-checked), non-temporal
  moves, the AVX2 masked moves (`vpmaskmovd`) and MMX.
- **Every byte of an access counts.** A 16-byte load at index `rcx` reads bytes `rcx` to
  `rcx + 15`, and rule 9's bounds check must keep all of them inside the buffer (see below).
  Size buffers in whole chunks so the last chunk stays inside.
- **Calls clobber vectors.** No vector register is callee-saved, so a function never saves one,
  and a value in a vector register is gone after a `call`. System calls keep them.

[assets/vector-template.s](../assets/vector-template.s) is a complete, checked SSE2 program
(capitalizing a line 16 bytes at a time) with its spec and tests.

### Floating point

Double precision, scalar and packed (`sd` and `pd`), from SSE2 to AVX-512:

- moves: `movsd`, `movupd`, `movhpd`, `movlpd`, `movddup`, `movmskpd`, `vbroadcastsd`, and the
  `movq`/`movd` moves between general-purpose and vector registers
- arithmetic: `addsd subsd mulsd divsd sqrtsd minsd maxsd`, the same with `pd`, `haddpd`,
  `hsubpd`, `addsubpd`; logic: `andpd andnpd orpd xorpd`
- compares: `ucomisd`/`comisd` (then `jb`, `jbe`, `ja`, `jae`, `je`, and `jp` for NaN), `cmpsd`,
  `cmppd`, `vcmppd` into a mask register
- conversions: `cvtsi2sd`, `cvtsd2si` (nearest, ties to even), `cvttsd2si` (toward zero),
  `cvtdq2pd`, `cvtpd2dq`, `cvttpd2dq`, and AVX-512's 64-bit and unsigned forms
- rounding: `roundsd`, `roundpd`, `vrndscalesd`, `vrndscalepd`; shuffles: `unpcklpd`, `unpckhpd`,
  `shufpd`, `blendpd`, `blendvpd`, `vpermilpd`, `vpermpd`, `vperm2f128`, `vinsertf*`,
  `vextractf*`, `vshuff64x2`, `vpermi2pd`, `vpermt2pd`, `vblendmpd`, `vcompresspd`, `vexpandpd`
- the `v` forms of all of these, and `.double 100.0` constants in `.rodata`

Single precision (`ss` and `ps`) has the same operations on 32-bit numbers: `movss`, `movups`,
`addss` … `sqrtps`, `ucomiss`, `cmpss`, `cvtsi2ss`, `cvtss2si`, `cvttss2si`, `roundss`,
`shufps`, `insertps`, `extractps`, `vpermps` and the rest, with `.float 2.5` constants. Convert
between the precisions with `cvtss2sd` (exact) and `cvtsd2ss` (to the nearest single), and
`cvtps2pd`/`cvtpd2ps` for vectors. A `ymm` register holds 8 singles and a `zmm` register 16, twice
as many as doubles.

Fused multiply-add (`vfmadd*`, `vfmsub*`, `vfnmadd*`, `vfnmsub*`, `vfmaddsub*`, `vfmsubadd*`, in
`132`, `213` and `231` forms, for `ss`, `sd`, `ps` and `pd`) needs the FMA extension. It computes
`a × b + c` with one rounding instead of two, so use it exactly where the tests' formula says
`fma(a, b, c)`.

Exact decimals such as money don't need floating point: parse "12.5" into 1250 cents, compute
with whole-number instructions, and print the cents with the point two digits from the right, as
the tests' `scaled` and `places` do. Use floating point for measured values, in the precision the
tests use: doubles, or singles where the formula says `single(x)`.

The tests compute exactly what these instructions compute, so:

- **Same operations, same order.** `{a + b + c}` is `(a + b) + c`: add in that order, never in
  pairs across lanes. Don't turn `/ 10.0` into `* 0.1`, which rounds differently.
- **Read decimals as one division.** Parse "12.34" into the whole number 1234 and its 2 places,
  then `cvtsi2sd` both 1234 and 100 and `divsd` once: that's the value the tests use. For a
  single, `cvtsi2ss` and `divss` the same way.
- **Print with `fixed`'s recipe.** `fixed(x, 2)` is `mulsd` by 100.0 (one constant), `cvtsd2si`
  (nearest, ties to even), then the whole number printed with the point two digits from the
  right ("0.05", "-0.05"). Take the sign from that whole number, so −0.004 prints "0.00". Print a
  single by widening it with `cvtss2sd` first, then the same recipe.
- **Fuse only where the formula does.** `a * b + c` is `mulsd` then `addsd`; `fma(a, b, c)` is
  one `vfmadd231sd`. Swapping one for the other changes the last bit.
- **Left out:** half precision, the `rcp`/`rsqrt` approximations (their results differ between
  CPU makers), `dpps`/`dppd`, `ldmxcsr` (rounding stays nearest, ties to even) and x87.

Everything else is out: x87, string instructions (`movs`, `stos`, …), `loop`, `jrcxz`, `enter`,
`leave`, `int`, `pushf` and `popf`.

### Threads

When the spec allows threads (`- Threads: up to N` under Constraints), a program can split its
work across cores with fork-join: `parallel_for` starts the threads and waits for all of them,
like Rust's scoped threads. Copy `parallel_for`, `start_worker`, `join_worker`, the constants
from `SYS_CLONE` to `MAX_WAITS`, and the data `stacks`, `tids`, `results` and `slots` word for
word from [the parallel template](../assets/parallel-template.s); `check` compares them. Then
choose three constants:
- `THREADS`, at most the spec's N;
- `STACK_SIZE`, each thread's stack: a multiple of 64 that holds the worker's deepest call path,
  which `check` computes;
- `SLOT_SIZE`, each worker's output: a multiple of 64.

Write a function named `worker`. `call parallel_for` with `rdi` = how many workers to run, from 1
to `THREADS`, and it calls `worker` once for each index, worker 0 on the calling thread:
- `worker` gets its index in `rdi` and its slot, `slots + index * SLOT_SIZE`, in `rsi`. It starts
  with `push rbx` and `mov rbx, rsi` (before any call), and leaves `rbx` alone after that.
- It returns a 64-bit result in `rax`, which lands in `results[index]`.
- It and every function it calls write only their own stack frame (`[rsp + …]`, inside what they
  pushed or reserved) and the slot (`[rbx + …]`, inside `SLOT_SIZE`, with the usual bounds
  checks). They read anything, including data the main thread wrote before `parallel_for`.
- They make no system calls, and only `worker` and its callees call them. Read and write on the
  main thread, before and after.

No two threads ever write the same byte, so results are the same on every run: combine `results`
in index order for a reduction (a count, a sum), or lay the slots end to end for a map (each
worker writes its part of the output, and one `write` prints them all). Assertions 901 to 999 are
the thread code's; number yours from 1 to 899. The kernel lets a threaded program call `clone`
only with `parallel_for`'s flags.

Threads pay off when each worker has real work: starting one costs about 10 to 20 microseconds,
and splitting a fast vector loop over a megabyte saves little. Pick the worker count from the
input's size, as the template does, and compare the time on the clock in the
`check` report: threads lower it while the CPU time, the total over all threads, stays or
rises.

## The rules

### 1. Simple control flow

Jumps land on labels in the same function; calls name functions. No recursion. Every path ends
in `ret`, `jmp`, an exit system call or an assertion; no line is unreachable, and control never
runs off a function's end. A chain of two or more `cmp R, constant` + `je` on one register ends in
an assertion, so an unexpected value can't slip through:

```
    cmp eax, KIND_ADD
    je .Ladd
    cmp eax, KIND_SUB
    je .Lsub
    jmp .Lassert_unknown_kind
```

### 2. Bounded loops

```
    mov r8, MAX_WRITES
.Lwrite_all_loop:  # bound: 64
    ... body ...
    dec r8
    jnz .Lwrite_all_loop
```

The counter is set by `mov R, NAME` right before the head label; `NAME`'s value is from 1 to the
`# bound: N` written after the head label. The loop ends with `dec R` (or `sub R, 1`) right before
`jnz` (or `jne`) to the head. Enter only through the head, and leave early with forward jumps.
Nothing in the body changes `R`: `syscall` changes `rax`, `rcx` and `r11`, and `call` changes
`rax`, `rcx`, `rdx`, `rsi`, `rdi` and `r8` to `r11`, so a loop that calls counts in `rbx` or `r12`
to `r15`. Every backward jump must be such a loop's end.

One loop may run without a bound, as NASA's rule allows for loops meant never to end: a server's
loop that takes connections, in a program whose spec says `- Network: listens on TCP port N`. Its
head label says `# bound: forever` and it closes with an unconditional `jmp` to the head, so it
provably never ends by itself; leaving early (to exit) uses forward jumps as usual:

```
.Lstart_serve:  # bound: forever
    mov eax, SYS_ACCEPT4
    ... take one connection, answer it, close it ...
    jmp .Lstart_serve
```

### 3. Resources reserved up front

NASA's rule is "no dynamic memory allocation after initialization". In lengua all memory is
reserved before the program runs: data in `.rodata`, `.data` and `.bss`, plus the stack. Size
every buffer from the spec's limits (the longest input it allows, the most items it holds). A
large `.bss` buffer costs nothing until it's used, because Linux hands out zeroed pages as the
program touches them, so a 64 MiB buffer for a program that reads up to 64 MiB is fine. Nothing
is allocated or freed while the program runs, so there's no leak, double free or use after free.

The program runs as one thread unless its spec allows threads (`- Threads: up to N`); then it
starts them only through the canonical `parallel_for` (see "Threads" below). It uses only the
system calls its spec allows, which are the capabilities the program is granted, and each `syscall`
is chosen by `mov eax, SYS_NAME` in the same straight-line code (no label or call between). The stack changes only through 64-bit `push` and
`pop`, `call`, `ret`, and `sub rsp, N` / `add rsp, N`; every path into a line has the same depth,
and nothing reads or writes below `rsp` (no red zone). `check` checks the deepest stack
over all call paths.

### 4. Small functions

At most 60 instructions per function (`assert_fail` excluded). One `ret`, reached with the stack
as it was on entry; `_start` has no `ret` and ends with an exit system call. Calls happen with the
stack 16-byte aligned: `_start` starts aligned, and other functions start 8 bytes deep (the return
address), so they move `rsp` by 8 more (`sub rsp, 8`, or one extra `push`) before calling. Calls
to `assert_fail` don't need alignment. A function that changes `rbx`, `rbp` or `r12` to `r15`
pushes them first and pops them in reverse order right before its `ret`.

### 5. Assertions

```
    cmp rcx, MAX_CHARS
    jae .Lparse_assert_index
    ...
.Lparse_assert_index:
    mov edi, 4
    call assert_fail
```

An assertion is exactly a label, `mov edi, <number from 1 to 999>` and `call assert_fail`. It's
reached by one jump only: a conditional jump, or a `jmp` right after one (the end of a comparison
chain, or a loop that ran out). Numbers are unique in the program, and functions average at least
2 assertions. `assert_fail` prints `lengua: assertion NNN failed` to standard error and exits with
code 134, which `check` reports as the assertion's line. Copy `assert_fail` and
`assert_message` from `assets/main-template.s` word for word.

### 6. Smallest scope

Only `_start` is exported. `[rip + name]` operands name data symbols, and read-only data is never
written. Writable data (`.data`, `.bss`) belongs to one function; others receive a pointer and a
length, unless the data is marked `# shared: <reason>` right above its `.type` line.

### 7. Checked results

After a system call that returns (not exit), or a call to a function marked `# returns: result`,
the first instruction that touches `rax` is `test` or `cmp`, directly followed by a conditional
jump, with no label or jump before it:

```
    call parse_i64
    test rax, rax
    js .Lstart_not_a_number
```

Results follow Linux: `rax` of 0 or more means success, a negative `rax` is an error code. A
result function returns its value in another register, such as `rdx`.

### 8. No macros, named constants

No macros, repeats, conditional assembly or includes. Named constants for system call numbers
(`mov eax, SYS_WRITE` with `.equ SYS_WRITE, 1`), exit statuses (`mov edi, EXIT_USAGE` on every path
to the exit; 0 to 255, never 134), `.zero` and `.skip` sizes, and loop counts. A constant named
`NAME_LEN` must equal the size of the data symbol `name`: `USAGE_MSG_LEN` is `usage_msg`'s length.

### 9. No indirection, checked indexes

No jumps or calls through registers or memory, and data holds numbers, never addresses. A memory
access with an index register needs, since the last label, `cmp index, LIMIT` followed by `jae`
(or `ja`) away from the access, and no change to the index before the access. `jae` compares
without sign, so a negative index is caught too. `lea` doesn't access memory, so `lea rdi,
[rbx + rdx]` needs no check:

```
.Lparse_loop:  # bound: 20
    cmp rcx, MAX_CHARS
    jae .Lparse_assert_index
    movzx edx, byte ptr [rdi + rcx]
```

When the base register holds the address of one of the program's data symbols (every path sets
it with `lea base, [rip + name]`), the limit must keep every byte of every access inside that
symbol. For a 16-byte load from an 80-byte buffer, the largest index is 64:

```
    cmp rcx, LINE_BUF_LEN - CHUNK + 1
    jae .Lshout_assert_index
    movdqu xmm0, xmmword ptr [rsi + rcx]
```

An access through `[rip + name]` must fit inside `name` in the same way. Where a
check is provably unnecessary, wrap the access in `# unsafe: <reason>` … `# end unsafe`. Only
bounds checks can be skipped this way, and `check` finds every unsafe block.

This rule checks the shape of each bounds check, and the limit against data symbols. It doesn't
check moving a pointer (`add rsi, rcx`) instead of indexing: the tests guard that until the proof
step after the MVP.

### 10. Checked by tools

The assembler and linker treat warnings as errors. `check` runs on every change, and nothing
runs or is stamped before every rule passes. The lock records `dialect = 2`.
