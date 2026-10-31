---
name: compile
description: Compiles a lengua program's spec.md into main.s, x86-64 assembly for this computer (integer, vector and floating-point instructions) that passes every test in the spec, in lengua's dialect checked against the lengua Power of 10 (NASA's rules adapted for assembly). Use after check records the spec, when the lengua check tool says to write main.s or reports problems in it, when someone shares a spec to build on this computer, or when the user asks for a faster program. Ends when check says done, with a short report to the user.
license: Apache-2.0
compatibility: Needs the lengua MCP server from the lengua plugin, on Linux x86-64 with LLVM (llvm-mc and ld.lld).
---

# Compile a lengua spec

`main.s` is the program: x86-64 assembly for Linux that you compile from `spec.md`. You are the
compiler. lengua checks your translation: it assembles `main.s`, reads the machine code back,
matches it to `main.s` line by line, checks the lengua Power of 10, then runs every test in the
spec and proves what it can. Nothing runs before every rule passes. Once a check passes, lengua
writes main.s's second line itself, naming the spec it was compiled from, this computer's target
and the CPU features the code needs; never write that line yourself.

## Steps

1. Read `<path>/spec.md`: its behaviors, and the generated tests under them. The last `check`
   reply's `machine` lists the CPU extensions main.s may use, such as BMI, POPCNT, LZCNT, ADX,
   MOVBE, AVX2 or AVX-512.
2. Plan the functions before writing them: at most 60 instructions each (rule 4), so `_start`
   checks the arguments and calls the functions that do the work.
3. Write `<path>/main.s` in the shape of [the template](assets/main-template.s): its first line
   is `.intel_syntax noprefix`, and `assert_fail` and `assert_message` are copied word for word;
   the rest is the program. The template is a complete, checked program that prints the negative
   of a number ([its spec](assets/template-spec.md)); its `parse_i64`, `write_i64` and
   `write_all` can be reused as they are. To read a whole input, a reply, or a request up to the
   byte that ends it (such as a newline), call `fill` from [fill.s](assets/fill.s), copied word for
   word with its three constants: the proof then covers every way the input can arrive, cut into
   any pieces. Write your own read loop only to stream input larger than a buffer. A server whose
   spec serves several clients at once starts from
   [the event-loop server](assets/server-template.s) ([its spec](assets/server-template-spec.md)):
   `poll` over the listening socket and a fixed table of connections, a line buffer for each,
   and answering each client as its request completes. A program for the terminal starts from
   [the key reader](assets/terminal-template.s) ([its spec](assets/terminal-template-spec.md)):
   keys one at a time with no echo, a wait for the next key, and the terminal set back at the
   end. A choice made for each input byte with `cmov` or `setcc`, not a branch, keeps the proof
   to one path; a branch per byte doubles the paths with each byte, and the proof stops early.
4. Write the code by [the rules](references/power-of-10.md), with
   [the Linux x86-64 reference](references/linux-x86_64.md) for system calls, the program's start
   and the calling convention. Write it correct first; speed comes in step 6.
5. Call `check` with `path`. It checks the rules first and lists each violation as
   `rule N (name) main.s:LINE: problem. Fix: …`. Then it runs every test: each behavior's example,
   and the generated tests with fresh random inputs (boundary values first). A failure shows the
   inputs, the seed, what was expected and what happened, and `assertion N failed at main.s:LINE`
   when an assertion fires. When a test fails, decide from `spec.md` whether `main.s` or the test
   is wrong: fix `main.s`, or fix the test in `spec.md` by the write-spec skill's rules, and call
   `check` again. Keep going until every test passes; never stop to ask the user.
6. **Make it fast.** lengua aims for programs faster than C, C++ and Rust built with every
   optimization. When the program loops over its input (text, numbers, files, a stream), make one
   pass for speed as soon as `check` says every test passes, then call `check` again: the tests
   must stay green, and the `cycles` in `performance` of the tests with the largest inputs should
   drop. Editing main.s starts its proof over, so make this pass before waiting for the proof.
   - **Vectors as wide as the CPU allows:** a loop over bytes takes 64 at a time with AVX-512
     when `machine` lists it (32 with AVX2), and the last partial vector with a masked load
     (`vmovdqu8 zmm0 {k1}{z}`), never byte by byte. `gcc -O3 -march=native` vectorizes too, so
     only the widest loop beats it.
   - **Break dependency chains:** when each step waits on the one before, as in a checksum or a
     running sum, run two to four independent chains side by side and join them at the end.
   - **Large pieces:** read and write a megabyte at a time, never a line or a byte per call.
   - **Input of any length streams:** read a piece, process it, keep only what crosses into the
     next piece (such as whether the last byte ended a word), and repeat until `read` returns 0.
   - **Threads** only when each gets at least a few hundred kilobytes of work (see below).

   `assets/fast` holds three checked programs to learn from and copy from, each with its spec:
   [wc](assets/fast/wc/main.s) counts lines and words 64 bytes a step with AVX-512 masks;
   [crc32c](assets/fast/crc32c/main.s) runs three `crc32` chains side by side and joins them with
   `pclmulqdq`; [histogram](assets/fast/histogram/main.s) spreads its counts over four tables.
   Their `digits` (write a number's decimal digits) can be copied as it is.
7. Once every test passes, `check` proves the program in the background, and within about 20 seconds
   it says `done`. Each line of `proofs` says what's proven, how far, or why not, and a proof still
   running says how far it has got: the proofs go on for up to five minutes, lengua records each in
   lengua.lock as it ends, and any later `check` shows them. Don't wait for them, but call `check`
   again before the user relies on the program: a proof that finds it wrong shows there, and `run`
   refuses that build from then on, even after lengua restarts. For each generated
   test it can, it proves for every input rather than a sample that the program prints what the
   formulas say: whole numbers, decimals, each value of a `choice(…)`, texts and text formulas such
   as `upper` and `count`, checksums from `crc32` instructions, and files. Long texts are proven
   up to a length, such as 4096 bytes, and tested beyond it: each line of `proofs` says how far,
   and names any function the proof never ran, such as a fast path for longer inputs. Tell the
   user that code isn't proven. Every run a proof covers is also held to
   the memory rules: each access stays inside its own buffer, frame or argument, no return address
   is written, and no stack is read before it's written. If the proof finds inputs where the
   program is wrong, or breaks a memory rule, `check` reports them like a failing test, and `run`
   won't run that build: fix `main.s` and call `check` again. "Tested, not proven" is fine: say
   why in one line. Vector code proves too, from SSE2 to AVX-512 with masks, and so do clients and
   servers on the network: what the test's peer sends arrives whole or cut, and in any pieces at
   all where the program reads it with `fill`. Programs that read and print numbers with the
   template's `parse_i64` and `write_i64` prove best.
8. Tell the user, from the `done` reply, in a few short lines and plain words: where the program
   is; what its spec promises, in two or three lines (its behaviors, its limits such as input
   sizes and ranges, and what it may touch: files, ports); that it follows every rule and passed
   all its test runs; which behaviors are proven for every input, and which up to a length, such
   as "proven for every input up to 4096 bytes, tested beyond" (and, if the proof says one solver
   answered alone, which one to install for a second check), and which proofs are still running,
   which checking again later shows; what `memory` says, in one line; what the spec's first
   example did
   (`example`); that they can ask you to run it on their own inputs or files with `run`, or to
   change what it does; and the command that runs it from their own terminal, scripts and
   services, `shell`, as it is, followed by the program's arguments. A server isn't running after
   `check`: offer to start it with `run`, which returns its address. If the reply has `changes`, say in a line each what changed after their
   request and why, and name every change that tests less; if it lists more than one request,
   every one must be the user's own words. If the code handles a case the spec doesn't mention,
   such as input longer than a buffer, say so in one line and offer to add it to the spec. When
   `tests` says a generated test tried every value in its range, say that behavior was checked
   for every input in its range. Leave out hashes and seeds.

### Running it for the user

`run` runs the checked program with `path`, plus `args` and `stdin` when it reads them, within 40
seconds. A program that works on files runs in a folder of the user's: pass it as `folder` and
name the files in `args` relative to it; the program can't touch anything outside it. A server
starts in the background on its spec's port and `run` returns its address: try it with your own
tools, such as `curl` or `nc`, then call `run` with `stop: true` to stop it and see what it
printed. A program that connects reaches only the ports its spec names.

### Shared specs

People share specs, not code: the spec is the source of truth, and main.s is disposable. A spec
someone shares compiles here like any other: write `main.s` for this computer. To share a program,
the user shares `spec.md`, and anyone's agent compiles it for their own computer. Each compile is
a new program that passes the same checks, so whatever must be the same everywhere belongs in the
spec. A `main.s` that comes along in a copied folder is checked from scratch before `run` runs it;
when its second line says it was compiled for another computer or a CPU feature this one lacks,
`check` says so, and you compile the spec again for this one. lengua's line names the code, it
doesn't protect it: anyone can build main.s outside lengua, so its guarantees hold only for
programs that `run` runs after `check`.

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
[its spec](assets/vector-template-spec.md)) is a checked SSE2 program that finds a newline and
capitalizes a line 16 bytes at a time. Use AVX2 or AVX-512 when `machine` lists them, and keep
every vector access inside its buffer: see "Vectors" in [the rules](references/power-of-10.md).
With AVX-512BW, a byte test is one compare into a mask register and the change one masked
instruction: `vpsubb zmm4, zmm0, zmm1` (subtract `'a'`), `vpcmpub k1, zmm4, zmm2, 1` (below 26),
`vpsubb zmm0 {k1}, zmm0, zmm3` (subtract the case bit). Measured against Rust and C on a 1 MiB
file, that loop takes 4 times fewer cycles than `gcc -O3 -march=native`, where the 16-byte one
takes more instructions than C. Memory the program touches costs kernel time too, a page fault
per 4 KiB: work in place where the rules allow it. For floating point, packed instructions do 2
to 8 doubles at once, or 4 to 16 singles, as long as each element's steps stay in the formula's
order.

When the spec allows threads (`- Threads: up to N`), split heavy work across cores with
`parallel_for`: [the parallel template](assets/parallel-template.s) (with
[its spec](assets/parallel-template-spec.md)) capitalizes up to a megabyte on 16 threads, each
worker filling its own slot. Copy its thread code word for word, follow "Threads" in
[the rules](references/power-of-10.md), and compare the time on the clock, which threads
shorten. Starting a thread costs tens of microseconds, so give each one at least a few hundred
kilobytes of vector work: on a megabyte, one 64-byte loop is as fast as sixteen threads.
