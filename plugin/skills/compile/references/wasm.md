# Writing main.wat

`main.wat` is WebAssembly text that lengua builds, checks against [the WebAssembly
dialect](power-of-10-webassembly.md) and runs through [lengua's host](host.md). Those two pages
are the rules and the calls; this page is how to write a program that meets them on the first
check, from the module's layout to vectors, with the idioms the templates use. Every snippet here
is from a template in `assets/`, each a complete program that passes every rule and every test
of its idea.

## The templates

| Template | Its idea | What it shows |
|---|---|---|
| `main-template.wat` | `main-template-idea.md` | Arguments, parsing and printing a number, an error for each bad input |
| `stream-template.wat` | `stream-template-wat-idea.md` | Standard input of any length, a 64 KiB buffer-full at a time, in constant memory |
| `vector-template.wat` | `vector-template-wat-idea.md` | A line read up to its newline, found and changed 16 bytes at a time, with a scalar tail |
| `server-template.wat` | `server-template-wat-idea.md` | A server for several clients at once: `poll_oneoff`, `sock_accept`, `sock_recv`, `sock_send` |
| `tally-template.wat` | `tally-template-wat-idea.md` | A hash table, a merge sort and buffered output, in regions of several megabytes |

Start from the template closest to the idea, and keep its shape. Two kinds of program have no
template: one that reads keys from the terminal waits for lengua's host to give the terminal's
two imports, raw mode and the window's size; one that needs threads has none, since a
WebAssembly program has no threads.

## The module's layout

In this order, as every template has it:

1. **Imports**, only those the idea allows ([the host](host.md) lists which line allows which),
   each with its name and WASI's own type: `(func $fd_close (param i32) (result i32))`. Import
   only what you call: an import never called is refused.
2. **The memory**, one, its minimum equal to its maximum, exported as `memory`:
   `(memory (export "memory") 2 2)` for 128 KiB. Pages are 64 KiB; count them from the regions'
   end.
3. **Named constants**, immutable globals, grouped under comments: descriptors, exit codes,
   error numbers, sizes and limits, characters. Every number a call, a comparison or a loop needs
   is one of these.
4. **Regions**, from address 1024 up (lower addresses are lengua's): each an address global
   `$x` and a size global `$x.size`, or `$x.rosize` for read-only data placed with `data`.
5. **Functions**, `_start` first as `$main`, then what it calls. Name every function, global,
   local and label with `$`: the build refuses a function named by its number, as lengua's
   library merges in before your code.

Comments on a line carry the markers the rules read: `;; bound: N` on a loop's line,
`;; shared: <reason>` on a region's or a changing global's line, `;; returns: result` on a
function's line, and `;; unsafe: <reason>` … `;; end unsafe` around accesses whose checks are
provably unneeded. A template uses no unsafe block; neither should a program that can check.

## Regions and their sizes

```wat
  ;; READ_MAX bytes of input, rounded up to whole vectors so that every vector load stays inside.
  (global $line i32 (i32.const 1088))
  (global $line.size i32 (i32.const 80))
```

- **Size each region from the idea's limits**: the longest input it reads, the most entries it
  keeps, the longest line it prints. One byte past a limit is how a program sees input that is
  too long: the tally's `$input` holds `1048577` bytes for a limit of `1048576`.
- **Place them apart**, each starting at or after the end of the one before, and align large ones
  to 64 bytes ([caches.md](caches.md) says more of laying them out for the CPU's caches). Write
  the addresses out: a global's value is a constant, never an expression.
- **Writable memory starts at zero**, and a writable region can't hold data. So choose 0 as the
  mark of "nothing yet": the server's free slot is descriptor 0, which no connection is.
- **One owner each.** A writable region that one function uses (`global.get $x`) needs nothing
  more. Where another function needs it, lend it, as an address and a length in parameters, or
  mark the region `;; shared: <reason>` on its line. Lending makes a function reusable; the
  rules then check, at each call, that the farthest the callee reaches through the address fits
  the caller's region.
- **Read-only text** goes in a region of its own with `$x.rosize` equal to its bytes:
  `(data (i32.const 1024) "error: no input\0a")` with `(global $no_input.rosize i32 (i32.const
  16))`.

## Reading with `$fill`

```wat
            (local.set $got
              (call $fill (global.get $STDIN) (global.get $buffer) (global.get $buffer.size)
                (global.get $READ_TO_END)))
            (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
            ;; The input has ended.
            (br_if $done (i32.eqz (local.get $got)))
            (if (i32.gt_u (local.get $got) (global.get $buffer.size))
              (then (call $assert_fail (i32.const 1)) unreachable))
```

`$fill (fd, at, room, stop)` reads until `room` bytes are in, the input ends, or a read ends with
the byte `stop` (-1 for none). It gives the count, or a negative error. Read through it rather
than calling `fd_read` yourself. With `stop` -1 it reads the same however the input arrives, cut
into any pieces. With a stop byte it can read past the first one, when a piece goes on after it:
only the bytes up to the first stop byte are certain, so find that byte yourself and keep what
follows it for the next line.

- **Test it at once**, `i32.lt_s` against 0, then bound the count before using it: the rules know
  nothing of what the library gives.
- **A line**: `stop` is `$NEWLINE` and `room` the longest line and its newline. Then find the
  first newline in what came, as `$find_newline` does: a newline may come before the last byte.
- **Any length**: `stop` is -1 and `room` a whole buffer, 64 KiB. A count below the buffer's size
  is the input's last piece; a loop of buffer-fulls keeps only what crosses from one to the next.
- **A file**: on a local disk, a read of a regular file gives every byte it asks for up to the
  file's end ([the host](host.md)), so `fd_read` into the whole buffer, its iovec written just
  before the call, needs no second read to find the end: a count below the buffer's size is the
  file's last. `$fill` reads a file the same way, with one more read that gives 0 at the end,
  which costs a call for each file where a program reads many small ones. On a network or FUSE
  mount a read may give fewer bytes before the end, so a program that must be right there too
  reads through `$fill`.

## Writing with `$write_all`

```wat
            (br_if $write_failed
              (call $write_all (global.get $STDOUT) (global.get $line)
                (i32.add (local.get $len) (i32.const 1))))
```

`$write_all (fd, at, len)` writes every byte, however many writes it takes, and gives 0 or a
negative error, so the branch tests it in place. Write large pieces: the tally fills `$out` and
writes it when the next line doesn't fit. Each exit code is a named constant from 0 to 255, never
134, which is a failed assertion's.

## Checking every error number

Every host call gives 0 or an error number, and the first thing after it is a branch that tests
it, before anything the call wrote is read:

```wat
        (local.set $err
          (call $sock_recv (local.get $fd) (global.get $iovec) (global.get $ONE_BUFFER)
            (global.get $NO_FLAGS) (global.get $received) (global.get $received_flags)))
        (br_if $whole (local.get $err))
```

Keep the result in a local and read it back at once, `local.set` then `local.get`, or test the
call's value directly. A block that the branch leaves must open before the call, not between the
call and its test. A function of your own that can fail is marked `;; returns: result` on its
line, gives 0 or an error number, and its callers test it the same way. Compare an error number
only with 0 or with a named constant, as host.md names them.

## Counted loops and their bounds

```wat
        (block $done
          (local.set $left (global.get $buffer.size))
          (loop $byte  ;; bound: 65536
            (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
            …
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $byte (local.get $left))))
```

- The counter is an `i32` local, set from a named constant in the two instructions right before
  `loop`, its value from 1 to the `;; bound: N` on the loop's line, so at most 2147483647.
- The loop ends with two instructions of its own, directly inside it, not in a block within it:
  `(local.set $left (i32.sub (local.get $left) (i32.const 1)))`, then
  `(br_if $byte (local.get $left))`. Nothing else in the loop writes the counter.
- Leave early with `br` or `br_if` to a block around the loop. A loop whose counter runs out
  either has done its work (the counter was the most it could do) or meets an assertion right
  after its end, when running out can't happen: `(call $assert_fail (i32.const 2)) unreachable`.
- Nest a loop inside another when you need more passes; reuse a counter local in a loop that
  follows, set again before it.
- A server's one loop that takes connections says `;; bound: forever` and ends with `(br $serve)`;
  something in it, or in a function it calls, calls `sock_accept`.

## Indexes and their checks

The rules follow each value: a region's address plus a number, a number with the range the code's
checks give it, or a value loaded from memory, which has no range at all. An access is safe when
every byte of it lies inside its region for every value the number can have.

- **Check the local you index with**, after its last write and before the access, unsigned:
  `i32.ge_u` against the region's size less the access's width, plus one, as `(i32.ge_u
  (local.get $i) (global.get $out.size))` before a byte; or `i32.gt_u` against the size less the
  width, as `(i32.gt_u (local.get $i) (global.get $LAST_CHUNK))` before a vector, where
  `$LAST_CHUNK`, the size less 16, is the last place a vector may start.
- **Inside a loop**, check each time round: what a loop changes has no bound where the loop
  starts, so a check before the loop says nothing inside it.
- **A comparison bounds the local it reads directly.** `(i32.gt_u (i32.add (local.get $i)
  (global.get $CHUNK)) (local.get $len))` says something of `$len`, not of `$i`: check `$i`
  itself as well.
- **Bound a value from memory first.** A count a call wrote, or a slot's length kept in memory,
  becomes a number the rules follow at its first bounds check; tests of it before that, such as
  "is it zero", are lost. The server checks `$got` against its room, and only then for 0.
- **A start and a length are checked apart** unless one expression makes both from the same
  local, as `(i32.add (global.get $out) (local.get $at))` and `(i32.sub (global.get $out.size)
  (local.get $at))` in one call do: the rules see those add up to the region's size. A buffer
  whose start and length both vary in other ways can't be shown to fit, so give it a fixed start.
  The server receives into `$chunk` at its region's start, and then copies the bytes into the
  slot's line one at a time, each place checked; it sends from the start of the slot's line, where
  `$shift` moves what a send leaves.
- **iovecs**: write the array with `i32.store` at fixed offsets of its region, in the function
  that makes the call, so the rules follow each buffer it points to.
- **Lent addresses**: a callee checks its own indexes against its own limits, and the rules sum
  the farthest it goes; the caller's region must have that room.

## Assertions

```wat
    (if (i32.gt_u (local.get $len) (global.get $MAX_LINE))
      (then (call $assert_fail (i32.const 4)) unreachable))
```

An assertion states what must be true, and fails only when it isn't: in the `then` of an `if`
whose condition is the check, right after a `br_if`, or right after a counted loop that can't run
out. Number them from 1 to 899, each once, at least two per function on average; checks on a
callee's parameters, and the bounds checks above, are the usual ones. `$assert_fail` prints
`lengua: assertion NNN failed` and exits with 134, and `check` names the line.

## SIMD128, and its scalar tail

```wat
    (local.set $small_a (i8x16.splat (global.get $SMALL_A)))
    …
        (local.set $chunk (v128.load (i32.add (local.get $at) (local.get $i))))
        (v128.store (i32.add (local.get $at) (local.get $i))
          (i8x16.sub (local.get $chunk)
            (v128.and (local.get $case_bit)
              (i8x16.lt_u (i8x16.sub (local.get $chunk) (local.get $small_a))
                (local.get $letters)))))
```

- **Spread constants over the lanes once,** before the loop, into `v128` locals: `i8x16.splat`
  of a named constant.
- **A range test is one compare**: subtract the range's start and compare unsigned with its
  length, `i8x16.lt_u`, as below the start wraps to a large number. A compare gives each lane all
  ones or all zeros: a mask.
- **Masks select**: `v128.and` of a mask and a constant is the change for each lane, then
  `i8x16.sub`, `i8x16.add` or `v128.xor` applies it; `v128.bitselect` picks between two vectors.
- **Find a byte** with `i8x16.eq` against a splat of it, `i8x16.bitmask` (a bit for each lane),
  and `i32.ctz` of a mask that isn't 0: the first match's place in the vector.
- **Count matches** in the lanes, not through a bitmask, which ARM64 has no one instruction
  for: a compare's all ones moved down by 7 (`i8x16.shr_u`) is 1 where it holds, which
  `i8x16.add` adds to a counter of a byte in each lane. Before 255 steps, widen the bytes in
  pairs into lanes of 16 bits, `i16x8.shl` then `i16x8.shr_u` by 8 for the first of each pair and
  `i16x8.shr_u` by 8 for the second, and the same at 16 bits into lanes of 32 bits at the end,
  then add the lanes with `i32x4.extract_lane`. Shifts keep each count a sum of the compares'
  conditions, which the proofs follow; `extadd_pairwise` does the same in one instruction, but
  the proofs don't follow it yet.
- **The byte before each lane** is `i8x16.shuffle` of the step before's vector and this one's,
  lanes 15 to 30: the last lane of the one before, then this one's first fifteen.
- **Bound every vector access** to the region's size less 16, so a 16-byte access never crosses
  the end; round a region up to whole vectors where a loop reads them all, as `$line`'s 80 bytes
  hold `READ_MAX`, 65.
- **The scalar tail**: the vector loop runs while 16 bytes are left; a second counted loop, its
  bound 15, does the rest a byte at a time with the same change in `i32` arithmetic. Then no
  access touches a byte past the data.
- **Only the deterministic instructions**: no relaxed SIMD, which gives different bits on
  different computers. Integer lanes compute the same everywhere; float lanes do too, as the
  engine makes every NaN the same.

## The network

A server's listener is descriptor 4, and lengua numbers each connection it accepts from 5:

- **One loop that never ends** builds its waits, calls `poll_oneoff`, and handles each event.
  A subscription is 48 bytes: its own number (`i64` at 0), its kind (`i32.store8` at 8, 1 to wait
  to read) and its descriptor (`i32` at 16). An event is 32 bytes and starts with its
  subscription's number, so give each subscription its slot's number, and the listener one past
  them all.
- **Wait on the listener only while a slot is free**, so further clients wait their turn in its
  queue and `sock_accept` always finds a slot.
- **Read what has come** with one `sock_recv` an event, never in a loop of its own: a client that
  holds the rest of its request must not stop the others. Keep each slot's line and its length
  in regions, and answer once the request is whole, by its newline, blank line or length.
- **Answer and close**: `sock_send` until the whole answer has gone, then `fd_close`, then free
  the slot. A send takes what goes at once, which may be only the first part of a buffer: bound
  the count it wrote and send what is left, as `$finish` does once `$shift` has moved it to the
  line's start. Test each call's error number, even where every path goes on the same way.

## What the build and the checker refuse

The build refuses text that doesn't parse or validate with the engine's features, a function
named by its number, and any name starting with `$lengua.`. The checker refuses, each on its line
with a fix:

- imports the idea doesn't allow, or with another type; a call of `$fill` when the idea allows
  no `fd_read` ([the host](host.md) lists the lines that do); exports but `_start` and `memory`;
  a start function, a table, an element segment, `memory.grow`, `memory.init`, a second memory,
  or a memory whose sizes differ;
- regions below 1024, overlapping, past the memory, or without their size; data outside a
  read-only region; a `$x.rosize` that isn't its data's length;
- recursion; `return`, or a branch to the function's own label; `unreachable` anywhere but right
  after `proc_exit` or `$assert_fail`, and either of those without it after; a `br_table` whose
  default doesn't lead to an assertion or an error; `_start` reaching its end;
- a loop without its counter or its bound, or another backward branch; a second loop that runs
  forever, or one in a program that doesn't serve;
- more than 180 operators in a function; fewer than two assertions per function on average, an
  assertion number twice or outside 1 to 899, or an assertion that is reached without a failed
  check;
- an untested error number; a bare number as a descriptor, a length, a mode, an exit status, a
  bound against a local, or an error number; a changing global without `;; shared:`, or a
  writable region two functions use without it;
- an address not made from a region's global plus offsets (M1): a plain number, as in
  `(i32.load (i32.const 2048))`, two regions' addresses added, or one that is a plain number on
  some path;
- an access with a computed index that no check keeps inside its region, or a constant one past
  it; a write to a read-only region; a call that writes a buffer while another argument names the
  same bytes; a call path too deep for the engine's stack;
- the trapping float-to-integer conversions (use `trunc_sat`), `reinterpret` of a float that may
  be NaN, a division whose divisor may be 0, relaxed SIMD, atomics, reference types, annotations;
- a function never called, a local or a constant never read, and code that never runs;
- for an idea with a `- Secret:` line, a secret that decides a branch, a `select`, an address, a
  length, a loop's count, a division or a float operation, or reaches a host call but as the bytes
  it writes out: [constant-time.md](constant-time.md) says what to write instead.

What the rules can't settle, such as an address loaded from memory or what `args_get` writes, is
left to the proof, which `check` reports with the rest.

## lengua's library

`check` merges these into the module; call them, never define them:

- `$assert_fail (param i32)`: an assertion's number; never returns.
- `$fill (param $fd i32) (param $at i32) (param $room i32) (param $stop i32) (result i32)`.
- `$write_all (param $fd i32) (param $at i32) (param $len i32) (result i32)`.

Addresses 0 to 1023 are lengua's: the library keeps its iovec and counts there while a call
lasts, and a buffer given to it starts at 1024 or above.
