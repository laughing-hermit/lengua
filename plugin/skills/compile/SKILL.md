---
name: compile
description: Compiles a lengua program's idea.md into main.s, assembly for this computer's target, Linux on x86-64 or AArch64 (integer, vector and floating-point instructions) that passes every test in the idea, in lengua's dialect checked against the lengua Power of 10 (NASA's rules adapted for assembly). Use after check records the idea, when the lengua check tool says to write main.s or reports problems in it, when someone shares an idea to build on this computer, or when the user asks for a faster program. Ends when check says done, with a short report to the user.
license: Apache-2.0
compatibility: Needs the lengua MCP server from the lengua plugin, on Linux x86-64 or ARM64 with LLVM (llvm-mc and ld.lld).
---

# Compile a lengua idea

`main.s` is the program: assembly for this computer's target, Linux on x86-64 or on AArch64,
that you compile from `idea.md`; the first word of `check`'s `machine` line names the target,
`linux-x86_64` or `linux-aarch64`. You are the compiler. You write `main`, and reach the system
only by calling lengua's operations (`os_read`, `os_write`, `os_open`, …) and its library
(`assert_fail`, `fill`, `parallel_for`), which are the same on every computer:
[the edge](references/edge.md) lists them. lengua checks your translation: it assembles `main.s`
with its own file after it (`_start` and what `main` calls), reads the machine code back,
matches it to `main.s` line by line, checks the lengua Power of 10, then runs every test in the
idea and proves what it can. Nothing runs before every rule passes. Once a check passes, lengua
writes main.s's second line itself, naming the idea it was compiled from, this computer's target
and the CPU features the code needs; never write that line yourself.

## Steps

1. Read `<path>/idea.md`: its behaviors, and the generated tests under them. The last `check`
   reply's `machine` lists the CPU extensions main.s may use, such as BMI, POPCNT, LZCNT, ADX,
   MOVBE, AVX2 or AVX-512. **Write for the baseline first**, the instructions every machine of
   the target has: SSE2 with SSE3, SSSE3, SSE4.1, SSE4.2 and POPCNT on x86-64, and ARMv8.0-A with
   NEON and floating point on AArch64, with nothing optional (no CRC, PMULL, AES, SHA or SVE).
   Take an extension `machine` lists only in step 6, for speed. lengua's own programs are held
   to the same rule: a program that needs an extension is a fast tier, named for it (`wc-sve`),
   beside a baseline form from the very same idea, and the tier is the one to start from only
   where `machine` lists its extension.
2. Plan the functions before writing them: at most 60 instructions each (rule 4), so `main`
   checks the arguments and calls the functions that do the work. When the idea builds on parts
   (`- Builds on:`), each part must be done first: call `check` on its folder until it is. Then
   take the functions you use from the part's main.s word for word, each with a
   `# from: ../sort` line in the comment above its `.type`, with the data they use: `check` holds
   each to the part's current code, and says which to copy again when the part changes. Never
   rewrite what a part already does.
3. Write `<path>/main.s` in the shape of [the template](assets/main-template.s): its first line
   is `.intel_syntax noprefix` (on `linux-aarch64`, [the AArch64 template](assets/linux-aarch64/main-template.s),
   whose first line is `.arch armv8-a`), `main` takes the argument count, the arguments and the
   environment in its first three argument registers and ends with `call os_exit`, and
   `assert_fail` is lengua's: call it with the assertion's number, never define it; the rest is
   the program. The template is a complete, checked program that prints the negative
   of a number ([its idea](assets/template-idea.md)); its `parse_i64`, `write_i64` and
   `write_all` can be reused as they are. To read a whole input, a reply, or a request up to the
   byte that ends it (such as a newline), call `fill`, which is lengua's: the proof then covers
   every way the input can arrive, cut into any pieces. To stream input larger than a buffer,
   call `fill` again for each buffer-full, with its stop byte -1, as
   [the streaming filter](assets/stream-template.s) does
   ([its idea](assets/stream-template-idea.md); on AArch64
   [its NEON form](assets/linux-aarch64/stream-template.s)): where each buffer-full's
   output is its bytes
   mapped one by one (`upper`, `lower`, `tr`), or counts kept in registers and printed at the
   end (`len`, `words`, `count` of a part such as `"\n"` or `"\r\n"`), the proof covers every
   length by induction over the loop, whatever the program keeps of the last buffer-full (such
   as whether it ended inside a word); a `count` of a part that can overlap itself, such as
   `"aa"`, is covered too where a register of its own carries how much of the part the text ends
   with from one buffer-full to the next (for `"aa"`, whether the last byte is an `a` that starts
   no pair yet), kept as 0 or 1 with `setcc`, `and` and `xor`, without branches; otherwise it
   goes on through longer input as far as its time reaches, where a read loop of your own is proven
   only up to the 4096 bytes one pipe write holds. For a buffer as large as 64 KiB, the proof takes
   the last buffer-full at every length at once, which needs each loop over a buffer-full to leave
   only where its index reaches the bytes `fill` read, and to mask each step to the bytes left,
   so the last partial step is the same code: on x86-64, `min(left, 16)` spread over a vector
   with `pshufb`, then `pminub` and `pcmpeqb` against each lane's place plus one (an unsigned
   compare: the proof doesn't follow a signed `pcmpgtb` one), as
   [the fast wc](assets/fast/wc/main.s) does (`bzhi` of `min(left, 64)` into `k1` in
   [its AVX-512 tier](assets/fast/wc-avx512/main.s)); on AArch64, `cmhi` of `min(left, 16)`,
   spread over a vector with `dup`, against each lane's place, as
   [its NEON form](assets/linux-aarch64/fast/wc/main.s) does. A separate routine for the last
   partial vector, at a
   place the length chooses, is proven only one final length at a time, which a large buffer's
   proof doesn't finish. A server whose
   idea serves several clients at once starts from
   [the event-loop server](assets/server-template.s) ([its idea](assets/server-template-idea.md)):
   `os_poll` over the listening socket and a fixed table of connections, a line buffer for each,
   and answering each client as its request completes; its proof follows the clients' steps in
   every order between polls; on AArch64 [its form](assets/linux-aarch64/server-template.s) is
   the same program. A program for the terminal starts from
   [the key reader](assets/terminal-template.s) ([its idea](assets/terminal-template-idea.md); on
   AArch64 [its form](assets/linux-aarch64/terminal-template.s)):
   keys one at a time with no echo, a wait for the next key, and the terminal set back at the
   end. A program that counts or looks things up by key starts from
   [the word tally](assets/tally-template.s) ([its idea](assets/tally-template-idea.md); on
   AArch64 [its form](assets/linux-aarch64/tally-template.s)): a hash
   table of a million slots by FNV-1a with linear probing, its entries in the order the keys first
   appear, sorted by a merge sort to print, so printing walks the entries, never the slots. A
   choice made for each input byte with `cmov` or `setcc`, not a branch, keeps the proof
   to one path; a branch per byte doubles the paths with each byte, and the proof stops early.
4. Write the code by [the rules](references/power-of-10.md), with
   [the edge](references/edge.md) for the operations, the errors and the library, and
   [the x86-64 reference](references/x86_64.md), or when the machine line names AArch64
   [the AArch64 reference](references/aarch64.md), for the calling convention and the dialect's
   form on the CPU. Write it correct first; speed comes in step 6.
5. Call `check` with `path`. It checks the rules first and lists each violation as
   `rule N (name) main.s:LINE: problem. Fix: …`. Then it runs every test: each behavior's example,
   and the generated tests with fresh random inputs (boundary values first). A failure shows the
   inputs, the seed, what was expected and what happened, and `assertion N failed at main.s:LINE`
   when an assertion fires. When a test fails, decide from `idea.md` whether `main.s` or the test
   is wrong: fix `main.s`, or fix the test in `idea.md` by the write-idea skill's rules, and call
   `check` again. Keep going until every test passes; never stop to ask the user.
6. **Make it fast.** lengua aims for programs faster than C, C++ and Rust built with every
   optimization. When the program loops over its input (text, numbers, files, a stream), make one
   pass for speed as soon as `check` says every test passes, then call `check` again: the tests
   must stay green, and the `cycles` in `performance` of the tests with the largest inputs should
   drop. Editing main.s starts its proof over, so make this pass before waiting for the proof.
   - **A baseline form that runs on every computer of the target, and vectors as wide as the
     CPU allows beside it:** a loop over bytes takes 16 at a time, never byte by byte: SSE2 on
     x86-64 (with `popcnt`), which every x86-64 computer has, and NEON on AArch64, each step
     masked to the bytes left with lanes compared against their places, so the last partial
     vector is the same code. That is the program's baseline form, and it meets the idea on
     every computer of its target. When `machine` lists a wider extension, a faster tier goes
     beside the baseline, never in its place: 64 bytes at a time with AVX-512BW (32 with AVX2;
     a `k1` mask with masked compares, or a masked load, `vmovdqu8 zmm0 {k1}{z}`), and SVE on
     AArch64. Both forms meet the very same idea.md, so what a program does never depends on an
     optional extension. `gcc -O3 -march=native` vectorizes too, so only the widest loop beats
     it.
   - **Break dependency chains:** when each step waits on the one before, as in a checksum or a
     running sum, run two to four independent chains side by side and join them at the end.
   - **Large pieces:** read through `fill` into a 64 KiB buffer, the most a pipe hands over at
     a time, and write in large pieces, never a line or a byte per call: each operation is a call
     and a system call.
   - **Input of any length streams:** fill a buffer, process it, keep only what crosses into the
     next buffer-full (such as whether the last byte ended a word), and repeat until `fill`
     returns less than the buffer.
   - **Threads** only when each gets at least a few hundred kilobytes of work (see below).
   - **Many cases on one small number** (an operator's character, a state machine's state): one
     checked jump table, not a chain of compares (references/power-of-10.md, rule 9).

   `assets/fast` holds checked programs to learn from and copy from, each with its idea (and a
   faster tier beside the baseline where the CPU has more):
   [wc](assets/fast/wc/main.s) counts lines and words 16 bytes a step with SSE2, the baseline
   form that runs on every x86-64 computer, each step masked to the bytes left, and is proven
   for every length, as are [its AVX-512 tier](assets/fast/wc-avx512/main.s) (64 bytes a step
   with masks; on the runners it saved CPU time on some machines and never wall time through a
   pipe, so start from `wc` and take the tier only when a measurement on this machine favors
   it), [its NEON form](assets/linux-aarch64/fast/wc/main.s) for AArch64 and
   [its SVE tier](assets/linux-aarch64/fast/wc-sve/main.s);
   [crc32c-pclmul](assets/fast/crc32c-pclmul/main.s) runs three `crc32` chains side by side and
   joins them with `pclmulqdq`, and [its AArch64 form](assets/linux-aarch64/fast/crc32c-crc/main.s)
   one `crc32cx` chain beside `pmull` folds on the vector units; [histogram](assets/fast/histogram/main.s) spreads its counts over four tables,
   and [its AArch64 form](assets/linux-aarch64/fast/histogram/main.s) over eight.
   Their `digits` (write a number's decimal digits) can be copied as it is.

   **A baseline form, and fast tiers beside it.** Every program has a form that runs on every
   computer of its target, and where a CPU's optional instructions make it faster, a tier named
   for the extension it needs, from the very same idea; pick a tier when `machine` lists the
   extension, never as the only way to get the behavior. The baseline of x86-64 is SSE2 to SSE4.2
   and POPCNT, of AArch64 ARMv8.0 with NEON: no SHA, AES, PCLMUL, CRC32 or PMULL instructions,
   no AVX, no SVE. For the checksums and ciphers: [crc32c](assets/fast/crc32c/main.s) is one
   `crc32` chain of SSE4.2, 8 bytes a step, proven for every text as far as the proof's time
   reaches (each line of `proofs` says how far), and on AArch64
   [its baseline](assets/linux-aarch64/fast/crc32c/main.s) is a table of 256 words a byte at a
   time, which the tests check, and `proofs` says how far the prover reads it; SHA-256 and
   AES-128 are plain integer code, rotates and adds, with a table of 256 words for AES (the
   repository's `examples/sha256` and `examples/aes`, and their AArch64 twins in
   `tests/fixtures/linux-aarch64`), and their tiers are
   [sha256-ni](assets/fast/sha256-ni/main.s) and [aes-ni](assets/fast/aes-ni/main.s), on AArch64
   [sha256-sha2](assets/linux-aarch64/fast/sha256-sha2/main.s) and
   [aes-aes](assets/linux-aarch64/fast/aes-aes/main.s), each beside a `idea.md` that is its
   baseline's, byte for byte. Their programs are tested, not proven: the prover doesn't model the
   test functions `sha256` and `aes128`.
7. Once every test passes, `check` proves the program in the background, and within about 20 seconds
   it says `done`. Each line of `proofs` says what's proven, how far, or why not, and a proof still
   running says how far it has got: the proofs go on for up to five minutes, lengua records each in
   lengua.lock as it ends, and any later `check` shows them. Don't wait for them, but call `check`
   again before the user relies on the program: a proof that finds it wrong shows there, and `run`
   refuses that build from then on, even after lengua restarts. For each generated
   test it can, it proves for every input rather than a sample that the program prints what the
   formulas say: whole numbers, decimals, each value of a `choice(…)`, texts and text formulas such
   as `upper` and `count`, checksums from `crc32` instructions, floats, and files. Compute floats
   in the formula's order, operation for operation: the proof then follows them term for term, in
   milliseconds, where another method, even a more accurate one, is at best tested, and can print
   a different last digit than the formula. A text is
   proven up to the length its idea gives it, shortest first, as far as the proof's five minutes
   reach, and tested beyond: each line of `proofs` says how far ("up to 4889 of 1048576 bytes"),
   and names any function the proof never ran, such as a fast path for longer inputs. Tell the
   user that code isn't proven. A line that says to check with `prove_further` can go on: when
   the user wants more of it proven, call `check` with `prove_further: true`, and each such
   check proves on from where the last stopped, for another stretch in the background. Every run a proof covers is also held to
   the memory rules: each access stays inside its own buffer, frame or argument, no return address
   is written, and no stack is read before it's written. If the proof finds inputs where the
   program is wrong, or breaks a memory rule, `check` reports them like a failing test, and `run`
   won't run that build: fix `main.s` and call `check` again. "Tested, not proven" is fine: say
   why in one line. Vector code proves too, from SSE2 to AVX-512 with masks, and so do programs on
   several threads, and clients and servers on the network: what the test's peer sends arrives whole or cut, and in any pieces at
   all where the program reads it with `fill`. Programs that read and print numbers with the
   template's `parse_i64` and `write_i64` prove best.
8. Tell the user, from the `done` reply, in a few short lines and plain words: where the program
   is; what its idea promises, in two or three lines (its behaviors, its limits such as input
   sizes and ranges, and what it may touch: files, ports); that it follows every rule and passed
   all its test runs; which behaviors are proven for every input, and which up to a length, such
   as "proven for every input up to 4889 of 1048576 bytes, tested beyond; I can prove on" (and,
   if the proof says one solver
   answered alone, which one to install for a second check), and which proofs are still running,
   which checking again later shows; what `memory` says, in one line; what the idea's first
   example did
   (`example`); that they can ask you to run it on their own inputs or files with `run`, or to
   change what it does; and the command that runs it from their own terminal, scripts and
   services, `shell`, as it is, followed by the program's arguments. A server isn't running after
   `check`: offer to start it with `run`, which returns its address. If the reply has `changes`, say in a line each what changed after their
   request and why, and name every change that tests less; if it lists more than one request,
   every one must be the user's own words. If the code handles a case the idea doesn't mention,
   such as input longer than a buffer, say so in one line and offer to add it to the idea. When
   `tests` says a generated test tried every value in its range, say that behavior was checked
   for every input in its range. Leave out hashes and seeds.

### Running it for the user

`run` runs the checked program with `path`, plus `args` and `stdin` when it reads them, within 10
seconds, or within the `timeout_seconds` you pass, up to 40. A program that works on files runs in
a folder of the user's: pass it as `folder` and name the files in `args` relative to it; the
program can't touch anything outside it, and lengua opens every name it asks for. A server
starts in the background on its idea's port and `run` returns its address: try it with your own
tools, such as `curl` or `nc`, then call `run` with `stop: true` to stop it and see what it
printed. A program that connects gets its connections from lengua, made before it starts to the
places its idea names, and reaches nothing else; `run`'s report says which it made, and why one
couldn't be.

### Shared ideas

People share ideas, not code: the idea is the source of truth, and main.s is disposable. An idea
someone shares compiles here like any other: write `main.s` for this computer. To share a program,
the user shares `idea.md`, and anyone's agent compiles it for their own computer. Each compile is
a new program that passes the same checks, so whatever must be the same everywhere belongs in the
idea. A `main.s` that comes along in a copied folder is checked from scratch before `run` runs it;
when its second line says it was compiled for another computer or a CPU feature this one lacks,
`check` says so, and you compile the idea again for this one. lengua's line names the code, it
doesn't protect it: `lengua-mcp link main.s` prints main.s followed by lengua's file, so anyone
can build it outside lengua, and its guarantees hold only for programs that `run` runs after
`check`. lengua supports two targets so far, Linux on x86-64 and on
AArch64, the same lengua on each: every rule, limit and capability of one is the other's, and each
example and fixture has its twin on the other CPU, compiled from the very same idea. Every
program has a baseline form that runs on every computer of its target, and an optional extension
(AVX-512, SVE) is only ever a faster tier beside it, chosen from the `machine` line. Adding a CPU
or OS is set out in [porting.md](references/porting.md).

## Making it faster

Only when the user asks for a faster program: read `performance` in the `done` reply. For each
test it gives, in one line, the instructions each run executed over all its runs (the steadiest
measure of work), and, from the test's largest input run alone five times, the median CPU cycles
in the program's own code (its time, where slow instructions such as division weigh more), the CPU
time in microseconds (which also counts the kernel's work for system calls, so many small writes
show up there; for a short run it's mostly the kernel starting and ending the process), and the
time on the clock. Change `main.s` and call `check` again: the tests must stay green, and the
cycles and CPU time should drop. Every measure varies a little between runs. Starting and ending a
process costs about 100 microseconds whatever the program does, so speed work shows on large
inputs: compare cycles on the tests with the largest inputs. For a program with threads, the
measures cover every thread, and the time on the clock says how long it took.

The biggest win on text and arrays is usually vector instructions: one `pcmpeqb` compares 16 bytes,
and AVX2 and AVX-512 do 32 and 64. [The vector template](assets/vector-template.s) (with
[its idea](assets/vector-template-idea.md)) is a checked SSE2 program that finds a newline and
capitalizes a line 16 bytes at a time, and on AArch64
[its NEON form](assets/linux-aarch64/vector-template.s) does the same from the same idea. Keep
the 16-byte form as the program's baseline, which runs on every computer of its target, and
use AVX2 or AVX-512 as a faster tier beside it when `machine` lists them (the fast `wc` is
both: [`wc`](assets/fast/wc/main.s) and [`wc-avx512`](assets/fast/wc-avx512/main.s), one idea).
Keep every vector access inside its buffer: see "Vectors" in
[the rules](references/power-of-10.md).
With AVX-512BW, a byte test is one compare into a mask register and the change one masked
instruction: `vpsubb zmm4, zmm0, zmm1` (subtract `'a'`), `vpcmpub k1, zmm4, zmm2, 1` (below 26),
`vpsubb zmm0 {k1}, zmm0, zmm3` (subtract the case bit). Measured against Rust and C on a 1 MiB
file, that loop takes 5 times fewer cycles than `gcc -O3 -march=native`, where the 16-byte one
takes more instructions than C. Memory the program touches costs kernel time too, a page fault
per 4 KiB: work in place where the rules allow it. For floating point, packed instructions do 2
to 8 doubles at once, or 4 to 16 singles, as long as each element's steps stay in the formula's
order; on AArch64, NEON's do 2 doubles or 4 singles (`fmul v0.2d, v1.2d, v2.2d`).

When the idea allows threads (`- Threads: up to N`), split heavy work across cores with
`parallel_for`: [the parallel template](assets/parallel-template.s) (with
[its idea](assets/parallel-template-idea.md); on AArch64
[its form](assets/linux-aarch64/parallel-template.s)) capitalizes up to a megabyte on 16
threads, each worker filling its own slot. Call `parallel_for` as it does, follow "Threads" in
[the rules](references/power-of-10.md), and compare the time on the clock, which threads
shorten. Starting a thread costs tens of microseconds, so give each one at least a few hundred
kilobytes of vector work: on a megabyte, one 64-byte loop is as fast as sixteen threads.
