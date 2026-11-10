# Review: soundness of lengua's secret rule

main at b14ff2b. Its five findings were then confirmed by tests and fixed (runs 42877251634, 42881405730 and 42884257361); what stays open is in todo.md.

Method: read-only. I read all of src/checker/wasm/secret.rs, the parts of memory.rs it depends on (values, ranges, facts, records, iovecs, judge), model.rs helpers, wasi.rs, host/fds.rs, the stdin/fd_read host code, library.wat, constant-time.md, secret-equal, and the secret tests. Nothing was built or run, so every "the rule accepts this" below is from reading the code, not from a run. Each finding says what would confirm it.

Verdict: the rule is not sound for its stated property. There are four ways a secret reaches a branch unreported, plus one declared gap ("lengths aren't secret") that leaks content in one case. The control-flow, call, global, operator and limit handling all checked out.

Short names used below: secret.rs = src/checker/wasm/secret.rs, memory.rs = same folder. The fragments marked "plant" go where `;; plant` is in the `SECRET` module of src/checker/wasm/tests.rs:1146 (regions $text 1024/64, $out 2048/8, $iovec 3072/16; $got = $fill's count, $byte = first secret byte). Each should give a `Rule::Secrets` violation on its last `if`, and by my reading gives none.

## Findings, most severe first

### 1. HIGH. A memory.fill / memory.copy / memory.init of a variable public length "cleans" bytes it may not write.

Where: secret.rs:1426 (`exact: offset.constant().is_some() && matches!(reach, Reach::Bytes(_))`), fed by `reach_of` at secret.rs:1432-1439 (`Reach::Bytes(range.high)` for any bounded length, constant or not), used by `copy` (1120-1125) and `fill` (1139-1142), and acted on by `write` at 976-983 (one exact place and a public value clears `start..end`).

Why unsound: "exact" means these bytes are written on every run. For a load or store the width is fixed, so it holds. For copy/fill/init the length is a run-time value; the rule takes the top of its range as the bytes written. A public length in 0..=N at a constant offset clears N bytes of marks while the run may write fewer, even none. The memory rules accept exactly this shape (memory.rs:2854-2900: computed length, `offset.high + range.high <= size`, no verdict).

Plant (minimal):

```wat
(memory.fill (global.get $text) (i32.const 0) (i32.and (local.get $got) (i32.const 1)))
(if (i32.load8_u (global.get $text)) (then (nop)))
```

`$got` is public (secret.rs:1238, read to end). The memory rules give `$fill`'s result `Number(full)` (memory.rs:2108) and `i32.and` with 1 the range 0..=1 (memory.rs:585-591). So `fill` gets `Reach::Bytes(1)`, an exact place $text[0,1), and clears it. With an input of even length the fill writes 0 bytes and the `if` branches on the first secret byte. Same with `memory.copy (global.get $text) (global.get $out) <that length>`. A realistic form: "zero the n bytes I used" or "copy n public bytes over the key buffer" with n clamped by a bounds check.

Confidence: high (about 95%) that the rule reports nothing. Confirm with the planted test. Fix: exact only when the length's range is a constant (`values.get(2).range().constant().is_some()`); otherwise a public copy/fill changes nothing and a secret one marks the whole span.

Test coverage: none. The only strong-update test is a fixed-width `i32.store8` (tests.rs:1320).

### 2. HIGH. Sources are incomplete: a secret written out can be read back as public (file round trip; socket echo).

Where: secret.rs:1243-1249 (`secret_read`) and 1277 (only `fd_read` on a descriptor that may be 0, or 5 and up when a file is secret, is a source); `sock_recv` is never a source (1268-1295); secret data to `fd_write`/`$write_all`/`sock_send` on any descriptor is allowed by design (module comment lines 12-14).

Why unsound: with `- Secret: standard input` and `- Uses: standard input and files`, the program may write the secret bytes to a file (allowed: data written out), reopen it, and read it back. If the descriptor's range excludes 0, the read is public. A descriptor loaded from memory is `Loaded` (range 0..max, so tainted by accident), but two ordinary assertions turn it into `Number(5..=63)`: `Fact::Below` on a `Loaded` local makes it `Number(0..limit-1)` (memory.rs:1131-1140), then `Fact::AtLeast` raises `low` (1141-1145). Then `range.low == 0` is false and `sources.files` is false, so nothing is marked.

Module (modelled on FILE_READ, tests.rs:1386; idea: Uses standard input and files, Secret standard input):

```wat
(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $fd_close (param i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (global $STDIN i32 (i32.const 0))
  (global $FOLDER i32 (i32.const 3))
  (global $FIRST_OPENED i32 (i32.const 5))
  (global $MAX_FD i32 (i32.const 64))
  (global $CREATE_EMPTY i32 (i32.const 9))
  (global $EXIT_OK i32 (i32.const 0))
  (global $READ_TO_END i32 (i32.const -1))
  (global $name i32 (i32.const 1024))
  (global $name.rosize i32 (i32.const 7))
  (data (i32.const 1024) "tmp.txt")
  (global $opened i32 (i32.const 2048))
  (global $opened.size i32 (i32.const 4))
  (global $key i32 (i32.const 3072))
  (global $key.size i32 (i32.const 32))
  (global $back i32 (i32.const 4096))
  (global $back.size i32 (i32.const 32))
  (func $main (export "_start")
    (local $fd i32)
    (drop (call $fill (global.get $STDIN) (global.get $key) (global.get $key.size)
      (global.get $READ_TO_END)))
    (drop (call $path_open (global.get $FOLDER) (i32.const 0) (global.get $name)
      (global.get $name.rosize) (global.get $CREATE_EMPTY) (i64.const 64) (i64.const 0)
      (i32.const 0) (global.get $opened)))
    (local.set $fd (i32.load (global.get $opened)))
    (drop (call $write_all (local.get $fd) (global.get $key) (global.get $key.size)))
    (drop (call $fd_close (local.get $fd)))
    (drop (call $path_open (global.get $FOLDER) (i32.const 0) (global.get $name)
      (global.get $name.rosize) (i32.const 0) (i64.const 2) (i64.const 0)
      (i32.const 0) (global.get $opened)))
    (local.set $fd (i32.load (global.get $opened)))
    (if (i32.ge_u (local.get $fd) (global.get $MAX_FD))
      (then (call $assert_fail (i32.const 1)) unreachable))
    (if (i32.lt_u (local.get $fd) (global.get $FIRST_OPENED))
      (then (call $assert_fail (i32.const 2)) unreachable))
    (drop (call $fill (local.get $fd) (global.get $back) (global.get $back.size)
      (global.get $READ_TO_END)))
    (if (i32.load8_u (global.get $back)) (then (nop)))  ;; the secret's first byte: not reported
    (call $proc_exit (global.get $EXIT_OK))
    unreachable))
```

The same shape works with a connection: `sock_send` the secret to a peer that echoes, `sock_recv` it back (never a source, whatever is secret).

Confidence: high (about 85%) for the rule; other dialect rules may want cosmetic changes to the module. Confirm: check it with `secrets: [Source::StandardInput]`, uses standard input and files, and assert a Secrets violation on the last `if`. Fix options: when anything is secret, treat every read from a file or connection as secret; or refuse secret data to any descriptor that can be read back (anything but 1 and 2) unless reads from files/sockets are then secret.

Test coverage: none.

### 3. HIGH/MEDIUM. The rule assumes an access stays inside the region its address is made from; the memory rules do not hold that for accesses they leave to the proof.

Where: secret.rs:1412-1428 (`inside`: an unbounded low is 0, an unbounded high is the region's size) and the module comment lines 30-31. Against memory.rs:2901-2909: an offset with no bound but `checked` ("checked against a limit the rules don't follow"), and any unbounded index in a `;; unsafe:` block, is only `Verdict::Unsettled`, an obligation. src/checker/wasm/mod.rs:62-63 says the proof holds such an access "inside the program's memory but not inside its region".

Why unsound: a store of a secret at `$out + i`, with `i` checked against a limit loaded from memory, marks only $out, but may land in any other region; a later load of that region is public. The reverse (a load that leaves its region into secret bytes) is public too.

Plant ($out + 1024 is $iovec's first byte):

```wat
(i32.store (global.get $iovec) (i32.const 1024))
(i32.store offset=4 (global.get $iovec) (i32.const 1025))
(local.set $word (i32.load (global.get $iovec)))
(if (i32.lt_u (local.get $word) (i32.load offset=4 (global.get $iovec)))
  (then (i32.store8 (i32.add (global.get $out) (local.get $word)) (local.get $byte))))
(if (i32.load8_u (global.get $iovec)) (then (nop)))
```

Trace: `$word` is `Loaded`; the comparison gives `Below{limit: None}`; in the `then` arm it becomes `Number(full, checked)` (memory.rs:1131-1140); `offset_plus` gives low 0, high none, checked (653-703); `judge` gives Unsettled, no violation. The secret rule marks $out[0,8) only; $iovec[0] is unmarked and the last `if` passes.

The same root cause has two more faces:

- `args_get`/`environ_get` (`Bytes::Sized`, `Reach::ToEnd`, secret.rs:1452): arguments longer than the buffer's region spill secret text past the region, unmarked. This depends on what the proof demands of the sizes; I did not read the prover.
- iovec tracking (memory.rs:2059-2075) only forgets the entries of the region an address names, so an out-of-region store can change an iovec the rules think they know, and `fd_read` then fills a buffer the secret rule never marks.

Confidence: about 80% that `check` accepts the plant with no Secrets violation; lower (about 60%) that the proof step also passes it, which the module comment suggests but I did not verify. Confirm: the planted test, then run the proof on it. Fix: in `inside`, give `Place::Anywhere` whenever `offset.low` or `offset.high` is none or the access is in an unsafe block (i.e. whenever `judge` would not have bounded it itself).

Test coverage: none.

### 4. MEDIUM. Memory outside every region is invisible to the marks, and public stores can wipe the only trace of a secret put there.

Where: secret.rs:1319-1323 (`everywhere` marks regions only), 1313 (`Place::Anywhere => memory.any()`), with `Marks::clear` at 125-147.

Why unsound: a store of a secret through an address the rules don't follow marks every region. Exact public stores then clear every region. The secret, if the address pointed outside all regions (bytes below 1024 or the gaps between regions, which the proof allows: "inside the program's memory"), is still there, and a load through the same loaded address sees `memory.any() == false`.

Plant (2560 = $out + 512 lies in no region):

```wat
(i32.store (global.get $iovec) (i32.add (global.get $out) (i32.const 512)))
(i32.store8 (i32.load (global.get $iovec)) (local.get $byte))
(memory.fill (global.get $text) (i32.const 0) (global.get $text.size))
(memory.fill (global.get $out) (i32.const 0) (global.get $out.size))
(memory.fill (global.get $iovec) (i32.const 0) (global.get $iovec.size))
(i32.store (global.get $iovec) (i32.add (global.get $out) (i32.const 512)))
(if (i32.load8_u (i32.load (global.get $iovec))) (then (nop)))
```

The fills have constant lengths, so their clears are legitimate; this is independent of finding 1. It needs a module with no read-only region (those can never be cleared, see over-approximations).

Confidence: about 85% for the secret rule; about 65% that nothing else refuses a stored out-of-region address value (only accesses are judged, memory.rs:2553-2672). Fix: a sticky "outside the regions" mark in `Marks` that `everywhere` sets, nothing clears, and an `Anywhere` read tests.

Test coverage: none. No test loads or stores through a loaded address at all.

### 5. MEDIUM/LOW (declared, but leaks content). "Lengths aren't secret" lets a direct `fd_read` on a secret standard input reveal where its lines end.

Where: secret.rs:1297-1298 (every host result and every written lend is public, including `fd_read`'s count); host: src/wasm/host/mod.rs:1088-1094 and src/wasm/host/stdio.rs:76 (`Self::Here` passes on whatever one OS read gives).

Why: on a terminal one read returns one line, and a line-buffered writer on a pipe does the same, so the count is the position of a newline in the secret. The rule refuses exactly this for `$fill` with a stop byte (secret.rs:1230-1232) and constant-time.md line 102 tells the agent never to find a secret's end by a byte, yet a loop of raw `fd_read`s branching on each count passes. For tests (`Input::Given`) the pieces are fixed by the test, so it won't show there.

This is consistent with the rule's own words, so it is a gap in the claim rather than a bug in the code. Also public by the same decree: a secret file's size (`fd_filestat_get`, `fd_seek` from the end, `poll_oneoff`'s byte count) and each argument's and variable's length and position. Confirm by decision, not by test: either say in the comment and the guide that chunk boundaries of a direct `fd_read` are public, or make `fd_read`'s count on a secret descriptor secret unless the read is through `$fill` to the end.

## Checked and found sound

- Limits. MAX_ROUNDS and BUDGET inside a loop: everything the loop may write becomes secret (`havoc`, secret.rs:674-682, 750-753), and the head only grows, so it settles. BUDGET, MAX_DEPTH and recursion at a call: reported as `Unfollowed` and everything secret (451-459, 478-494). MAX_DEPTH at a block: reported and havocked (665-670). Nothing just stops following.
- No public catch-all operator. `generic` (1147-1157) makes every result secret when any operand is, by wasmparser's own arity (model.rs:1128-1140); the (0,0) fallback only applies to proposals the dialect bans. SIMD lane extracts, bitmask, any_true/all_true, swizzle, popcnt/clz/ctz, wraps and extends all propagate.
- Float and division classification: `divides` lists all eight (1457-1469); `floats` matches any variant with F32/F64 in its name, so f32x4/f64x2 and conversions both ways are refused (1473-1487).
- Locals across back-edges: head is the join of the entry and every branch back (735-740). Block results, multi-value, `br` carrying values, `br_table` (every target gets the state, 1011-1022), `if` without `else` (793-798), `return` (1023-1027).
- `select`: secret condition refused, result secret when any of the three is (1032-1040).
- Globals: strong updates are right, one global is one place (1066-1072); a call takes and gives them (1183, 1190).
- Calls: one outcome per context of (secret arguments, bindings, marks, globals) (438-445); a callee's memory and globals replace the caller's (1189-1190); a callee that never returns ends the path (1192).
- Stores and loads of fixed width: partial overlaps and unaligned widths are right (`holds` is any overlap, 150-154; `clear` cuts exactly the written bytes, 130-141). A public store through a non-constant or one-of-several address cleans nothing (984-985).
- Records: the memory rules overwrite each operator's record on every visit (memory.rs:1720-1727), but a loop's last pass is its settled one and heads only grow, so the kept record covers every run. Unreached operators have no record and become `Anywhere` (secret.rs:954).
- iovec tracking inside the memory rules (stores through other addresses, copies, fills, own calls and host writes all forget what they may change, memory.rs:2059-2075, 2143-2160, 2270-2278), apart from finding 3.
- `fd_read` with iovecs the rules can't follow marks every region (1293); an unknown descriptor counts as standard input and as a file (1245).
- No start function, only `_start` exported, WASI imports only, no `memory.grow`, no tables (structure.rs:290-360, 427; 123-170), so `_start` with empty marks is the only entry.
- `fd_pread`, `fd_renumber`, `path_readlink` are not given (wasi.rs:457, 492, 580), so no second way to read a file or move descriptor 0. Files open at 5 or above (host/mod.rs:360-363).
- Exit code and `$assert_fail`'s number are arguments, refused when secret (1198-1204). Traps: a conditional `unreachable` needs a branch, division and float truncation are refused outright.
- A secret operand of shifts, rotates and multiplication passes: outside the stated property, and said so in constant-time.md line 66.

## Over-approximations worth knowing

- With standard input secret, every read from a descriptor loaded from memory (every opened file) is secret, since `Loaded` has low 0 (1247). With a file secret, `fd_read` on a connection (5 and up) is secret too, while `sock_recv` is not: inconsistent.
- One secret argument or variable makes all of them secret (said in the comment).
- After any secret store through an unfollowed address, read-only regions are marked and can never be cleaned, so even constants loaded from them are secret.
- A loop that zeroes a secret buffer byte by byte never cleans it; only constant-offset stores and constant-length fills do.
- Host writes never clean (a public `fd_read` over a secret buffer leaves it secret).
- Bit-only float operators are refused: reinterpret, abs, neg, copysign, splat, lane extract.
- `select` on a secret condition is refused though the engines emit a conditional move; division by a public constant is refused.
- When a loop hits MAX_ROUNDS the program is refused for ordinary leaks on "everything secret" with no message that a limit was hit (the havoc is silent, 750-753).

## Comment and guide against the code

Claims the code doesn't keep:

- "exact where an access's offset is a constant, so a store of a value that isn't secret over secret bytes makes them public again" (lines 22-23): the code also does it for copy/fill/init of non-constant length (finding 1).
- "An access is taken to stay inside the region its address is made from, as the memory rules hold it to" (30-31): they don't, for what they leave to the proof (finding 3).
- "one through an address they don't follow may touch any region" (31): it may also touch memory in no region, which the marks can't hold (finding 4).
- "Nothing the idea's `- Secret:` line names … decides how main.wat runs" (1-2): not across a write-out and read-back (finding 2).
- MAX_ROUNDS: "everything it writes is taken as secret" (51): it is every byte of memory and every mutable global, plus the locals the loop writes; more than the words say, and sound.

What the code does and the comment doesn't say:

- Refuses what it can't follow: recursion, nesting past 256, more than 4,000,000 operators (`Leak::Unfollowed`).
- Handles `memory.init` (segment offset as an address, length).
- Treats a descriptor it can't bound as standard input and as a file; the guide's table says "descriptor 0" and "any file the program opens".
- `fd_read` with unfollowed iovecs marks every region.
- Results and error numbers of every host call are public.
- The data buffers of any host call are never looked at, not only `fd_write`/`sock_send`.

## The tests

Covered, would catch a regression: each refusal kind once (`if`, `br_if`, `br_table`, loop count, `select`, load and store address, copy length, fill address, `div_u`, `rem_s`, one float conversion, library argument, host buffer, `$fill` stop byte, recursion) in tests.rs:1212; public count, mask, written-out secret, one fixed-width strong update, per-context calls (1313); flow through a call's argument and result, a lent address, a global, a store then load (1337); argument, variable and file sources (1424); secret-equal and ten edits incl. exit code (tests/wasm_red_team.rs:720-860).

No test at all:

- Findings 1 to 5.
- Weak updates: a public store at a computed index or through one of several addresses must not clean.
- `memory.copy` carrying a secret from one region to another; `memory.fill` with a secret value; `memory.init`; the copy's source address; fill and init lengths.
- Any access through a loaded address; `fd_read` with unfollowed iovecs; `fd_read` straight from standard input; `$fill` on a file; an unknown or bounded descriptor.
- A secret that arrives through a back-edge on the second round; MAX_ROUNDS, BUDGET, MAX_DEPTH.
- Block results, multi-value, `br` with a secret value, `br_table` into a loop, an `if` without `else` joining a secret local.
- A global made public again; a callee that never returns.
- Partial overlap (store at offset 1, load at 0); 64-bit and vector widths; lane loads.
- SIMD of any kind; i64 division and remainder; the float load/store/const exemption; f32x4/f64x2 operators.
- `sock_send`/`sock_recv`; a secret argument of a host import other than `proc_exit`.

Not examined: the prover (whether it passes findings 3 and 4's obligations), the range arithmetic of memory.rs beyond a spot check (a wrong `low` there would also break `secret_read`), and `Builds on:` composition.
