# The lengua Power of 10 (WebAssembly dialect 1)

`main.wat` is WebAssembly text, one program for every computer. `check` parses it, merges lengua's
library into it, validates the module with the engine's own features, and checks the rules below.
Nothing runs before every rule passes. Each violation names its rule, its line in `main.wat` and a
fix.

These are NASA's "Power of 10" rules for safety-critical code (Holzmann, 2006), adapted for
WebAssembly and joined with Rust's promise of memory safety. A program that passes them, and its
proofs, has the three guarantees Rust gives, proven rather than checked while it runs:
- **no access outside its object:** every load, store and copy the rules follow stays inside the
  region it names, for every input (rule 9 and the memory rules M1 to M8). An access they leave to
  the proof stays inside the program's memory on every run the proof covers, and inside its region
  only as far as the tests show: `check`'s memory line lists each such access;
- **no use after free:** nothing is allocated or freed while the program runs (rule 3), so no
  address outlives what it points to;
- **no data race:** the program is one thread.

And one Rust doesn't: WebAssembly's isolation. Even a wrong program can't reach anything outside
its own memory, and it reaches the computer only through the calls its idea allows.

## The module

```wat
(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 2 2)

  ;; Constants (rule 8).
  (global $STDOUT i32 (i32.const 1))
  (global $EXIT_OK i32 (i32.const 0))

  ;; Regions (rule 3, M1): a read-only one holding the greeting, a writable one for the iovec.
  (global $greeting i32 (i32.const 1024))
  (global $greeting.rosize i32 (i32.const 6))
  (data (i32.const 1024) "hello\0a")
  (global $iovec i32 (i32.const 2048))
  (global $iovec.size i32 (i32.const 16))

  (func $main (export "_start")
    ...
    (call $proc_exit (global.get $EXIT_OK))
    unreachable))
```

- **Imports** come only from `wasi_snapshot_preview1`, and only the functions the idea allows:
  [the host](host.md) says which each `- Uses:` and `- Network:` line grants. `check` refuses any
  other import, so a program can't even name a call it isn't allowed.
- **Exports** are exactly the function `_start` and the memory `memory`.
- **One memory**, declared once with its minimum equal to its maximum (rule 3), of at most 16,384
  pages, 1 GiB. No `memory.grow`.
- **No start function, no tables, no element segments**, so no `call_indirect` (rule 9).
- **lengua's library** is merged into the module by `check`: `$assert_fail`, `$fill` and
  `$write_all`. Call them; never define them. Names beginning `$lengua.` are lengua's. Name every
  function you call or export (`$name`), never its number: the library's imports come first, so
  the merge changes every number, and `check` refuses one.
- **The language:** WebAssembly 2.0 with SIMD128: integers, floats, bulk memory (`memory.copy`,
  `memory.fill`), multiple results, sign extension, saturating conversions, and the
  deterministic 128-bit vector instructions. Not threads or atomics, relaxed SIMD, wide
  arithmetic, 64-bit memory, several memories, tail calls, exceptions, GC or reference types.

## The rules

### 1. Simple control flow

WebAssembly's control is already structured: `block`, `loop` and `if` nest, and a branch (`br`,
`br_if`, `br_table`) leaves to a label around it in the same function. lengua adds:
- **No recursion.** The call graph has no cycle: no function calls itself, directly or through
  others, so the deepest call path is known.
- **Every path ends** at the function's `end` with its results, or in a call that never returns
  (`$proc_exit`, `$assert_fail`) followed by `unreachable`. `unreachable` appears nowhere else.
- **A `br_table`'s default** leads to an assertion or an error path, so an unexpected value can't
  slip through.

### 2. Bounded loops

```wat
    (local.set $n (global.get $MAX_WRITES))
    (loop $write_all_loop  ;; bound: 64
      ...
      (local.set $n (i32.sub (local.get $n) (i32.const 1)))
      (br_if $write_all_loop (local.get $n)))
```

The counter is a local, set from a named constant right before the loop; the constant's value is
from 1 to the `;; bound: N` written on the loop's line. The loop's last two instructions take one
from the counter and branch back while it isn't zero. Nothing else in the body writes the counter.
Leave early with `br` to a block around the loop; the only backward branch is the loop's own.

One loop may run without a bound, as NASA's rule allows for loops meant never to end: a server's
loop that takes connections, in a program whose idea says `- Network: listens on TCP port N`.
Its line says `;; bound: forever` and it ends with an unconditional `br` back to itself.

### 3. Resources reserved up front

NASA's rule is "no dynamic memory allocation after initialization". All of a lengua program's
memory is its one memory, declared with a fixed size and cut into regions sized from the idea's
limits (the longest input it allows, the most items it holds). Nothing is allocated or freed while
it runs, so there is no leak, double free or use after free. WebAssembly's memory starts zeroed.

The program runs as one thread, and it reaches the computer only through the calls its idea
allows: those are its capabilities. Its stack is WebAssembly's own (locals and the operand
stack); nothing in the module keeps a stack in its memory. `check` reports the deepest call path,
which rule 1 makes finite.

### 4. Small functions

At most 180 operators a function (WebAssembly needs about three for each instruction of a CPU's,
so this is about 60 of a CPU's instructions, the 60 lines NASA's rule allows a function, to be
calibrated on the templates). A function ends once, at its `end`: no `return` before it; leave a
block with `br` instead.

### 5. Assertions

```wat
    (if (i32.ge_u (local.get $i) (global.get $MAX_CHARS))
      (then (call $assert_fail (i32.const 4)) unreachable))
```

An assertion is a call to `$assert_fail` with a constant from 1 to 899, reached only when a check
fails: in the `then` of an `if` whose condition is the check, or right after a `br_if` that leaves
when it holds. Numbers are unique in the program, and functions average at least 2 assertions.
`$assert_fail` writes `lengua: assertion NNN failed` to standard error and exits with code 134,
which `check` reports as the assertion's line. lengua's own assertions are numbered 901 and up.

### 6. Smallest scope

Values live in locals; a global that changes holds only what the program keeps across calls and
says why with `;; shared: <reason>` on its line. A writable region belongs to one function, which
lends it to others as an address and a length, unless the region is marked `;; shared: <reason>`.
Read-only regions are never written. Only `_start` and the memory are exported.

### 7. Checked results

Every call to the host returns an error number, 0 on success. The first use of it is a test that
branches, before any result the call wrote is read:

```wat
    (local.set $err (call $fd_read (global.get $STDIN) (global.get $iovec)
                                   (i32.const 1) (global.get $nread)))
    (br_if $read_failed (local.get $err))
```

A program's own function that can fail is marked `;; returns: result` on its line and returns 0
for success or an error number; its callers test it the same way. Parameters and the program's
input are validated before they're used.

### 8. No macros, named constants

WebAssembly text has no macros, and lengua keeps it that way. Descriptors, modes, error numbers,
exit statuses (0 to 255, never 134), sizes and loop bounds are named: immutable globals such as
`(global $STDOUT i32 (i32.const 1))`, never bare numbers. A region `$x` has its size in
`$x.size` or `$x.rosize`, and they must match what the region holds.

### 9. No indirection, checked indexes

No tables, so no indirect calls. An address is made only from a region's global plus offsets
(M1). An access with a computed index is preceded by a check that keeps every byte of the access
inside its region, unsigned, so a negative index is caught too:

```wat
    (br_if $parse_bad (i32.ge_u (local.get $i) (global.get $MAX_CHARS)))
    (i32.load8_u (i32.add (global.get $line) (local.get $i)))
```

For a 16-byte load from an 80-byte region the largest index is 64, so the limit is
`$line.size` - 16 + 1. `memory.copy` and `memory.fill` are held to the same: their source,
destination and length stay inside their regions. A `br_table`'s index needs no check (it has a
default), but the default must lead to an assertion or an error (rule 1). Where a check is
provably unnecessary, mark the access `;; unsafe: <reason>` … `;; end unsafe`: only bounds checks
can be skipped this way, and `check` reports every unsafe block.

### 10. Checked by tools

The module is validated with exactly the engine's features, and `check`'s lints have no warnings.
`check` runs on every change; nothing runs or is stamped before every rule passes. The lock
records `dialect = "webassembly 1"`.

## Memory safety

Every address has an owner, a region, and every access stays inside it. `check` follows each value
through each function: a region's address (from `global.get $x`), that address plus a number, a
length, a plain number, or a value it doesn't follow (one loaded from memory). What the rules
can't settle, the proof checks on every run it covers: that a division never traps, and that an
access stays inside the program's memory. The proof follows no region, so that such an access
stays inside its own region is tested, not proven, and `check`'s memory line lists them.

- **M1, owners.** An address is never made from a plain number or loaded from memory: only from a
  region's global, plus offsets. Regions start at 1024 or above (lower addresses are lengua's),
  don't overlap, and fit in the memory.
- **M2, bounds.** Every access stays inside its region for every input, through every width from
  1 to 16 bytes, by rule 9's checks or a constant offset.
- **M3, lifetimes.** Locals can't be addressed and nothing is freed, so no address outlives its
  region: this holds by construction.
- **M4, exclusive writes.** A region a function writes through is lent to it once: at a call, no
  other argument names an overlapping part of it, so a write can't change what the callee reads
  through another name. Read-only lending may overlap, as Rust allows many shared borrows but one
  mutable one.
- **M5, initialization.** Memory and locals start at zero, so nothing is read uninitialized; a
  region's bytes the program reads before writing are the zeros it was given, which the proof
  sees.
- **M6, immutability.** A read-only region (`$x.rosize`, filled by `data` segments) is never
  written, through any address. Code is never data: WebAssembly keeps them apart.
- **M7, threads.** There are none, so no data race.
- **M8, stack.** The deepest call path's locals and operands fit within the engine's stack; large
  buffers are regions sized from the idea.
- **Borrowed buffers.** Every buffer a host call reads or writes (an iovec array and the buffers
  it points to, a path's bytes, the place a call writes a count) lies inside a region with room for
  every byte the call may touch: [the host](host.md) lists what each call borrows.

A program whose design needs an address kept in memory (a table of pointers) is "tested, not
proven" for memory, never proven: the memory rules can't follow a loaded address to its owner.

## Numbers

- **Division:** `div_s`, `div_u`, `rem_s` and `rem_u` are preceded by a check that the divisor
  isn't zero and, for `div_s`, that it isn't the most negative number divided by -1, or the proof
  shows neither can happen on every run it covers. With the check, the program never traps there;
  where no proof settles it, that division is tested, not proven, and `check`'s memory line lists
  it.
- **Floats** are allowed (`f32`, `f64`). The engine makes every NaN canonical, so every computer
  computes the same bits, a NaN's too, wherever they go. A program never takes a NaN's bits as a
  number: `reinterpret` of a float that may be NaN is refused. `check` follows that one direct
  view; it doesn't follow a float's bytes through memory, or a float vector's lanes, where the
  engine's canonical NaN keeps the bits the same on every computer. Conversions to integers are
  the saturating ones (`trunc_sat`), never the trapping ones. There is no fused multiply-add in
  core WebAssembly, so an idea's tests don't ask for one.
- **Vectors** (`v128`) are the deterministic SIMD128 instructions; their lane accesses obey M2
  like any other.

## Secrets and caches

- **Secrets.** An idea whose `# Constraints` has a `- Secret:` line adds the secret rule: no value
  computed from an input the line names decides a branch, a `select`, an address, a length, a
  loop's count, a division or a float operation, or reaches a host call but as the bytes it writes
  out. [constant-time.md](constant-time.md) says what `check` refuses and how to write code that
  passes it, with masks. The rule checks the WebAssembly; the engine's machine code is trusted to
  keep its timing.
- **Caches.** The `machine` line names this computer's caches as its system describes them;
  [caches.md](caches.md) says how to lay work out for them, the same way on every computer.
