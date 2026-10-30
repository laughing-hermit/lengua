---
name: write-assembly
description: Writes a lengua program's main.s, the x86-64 assembly (integer, vector and floating-point instructions) that implements its recorded spec and passes its tests, in lengua's dialect checked against the lengua Power of 10 (NASA's rules adapted for assembly). Use after check accepts the tests, when the lengua check tool says to write main.s or reports problems in it, or when the user asks for a faster program. Ends when check says done, with a short report to the user.
license: Apache-2.0
compatibility: Needs the lengua MCP server from the lengua plugin, on Linux x86-64 with LLVM (llvm-mc and ld.lld).
---

# Write lengua assembly

`main.s` is the program: x86-64 assembly for Linux that you write from the recorded spec and its
tests. You are the compiler. lengua checks your translation: it assembles `main.s`, reads
the machine code back, matches it to `main.s` line by line, and checks the lengua Power of 10.
Nothing runs before every rule passes.

## Steps

1. Read `<path>/spec.md` and `<path>/tests.toml`. The last `check` reply's `machine` lists the
   CPU extensions main.s may use, such as BMI, POPCNT, LZCNT, ADX, MOVBE, AVX2 or AVX-512.
2. Plan the functions before writing them: at most 60 instructions each (rule 4), so `_start`
   checks the arguments and calls the functions that do the work.
3. Write `<path>/main.s` in the shape of [the template](assets/main-template.s): its first line
   is `.intel_syntax noprefix`, and `assert_fail` and `assert_message` are copied word for word;
   the rest is the program. The template is a complete, checked program that prints the negative
   of a number (its spec and tests are [template-spec.md](assets/template-spec.md) and
   [template-tests.toml](assets/template-tests.toml)); its `parse_i64`, `write_i64` and
   `write_all` can be reused as they are. To read a whole input, a reply, or a request up to the
   byte that ends it (such as a newline), call `fill` from [fill.s](assets/fill.s), copied word for
   word with its three constants: the proof then covers every way the input can arrive, cut into
   any pieces. Write your own read loop only to stream input larger than a buffer.
4. Write the code by [the rules](references/power-of-10.md), with
   [the Linux x86-64 reference](references/linux-x86_64.md) for system calls, the program's start
   and the calling convention. Write it correct first; speed comes in step 6.
5. Call `check` with `path`. It checks the rules first and lists each violation as
   `rule N (name) main.s:LINE: problem. Fix: …`. Then it runs every test: the examples, and the
   generated tests with fresh random inputs (boundary values first). A failure shows the inputs,
   the seed, what was expected and what happened, and `assertion N failed at main.s:LINE` when an
   assertion fires. When a test fails, decide from `spec.md` whether `main.s` or the test is wrong:
   fix `main.s`, or fix `tests.toml` with the write-tests skill, and call `check` again. Keep going
   until every test passes; never stop to ask the user.
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

   `assets/fast` holds three checked programs to learn from and copy from, each with its spec and
   tests: [wc](assets/fast/wc/main.s) counts lines and words 64 bytes a step with
   AVX-512 masks; [crc32c](assets/fast/crc32c/main.s) runs three `crc32` chains side by side and
   joins them with `pclmulqdq`; [histogram](assets/fast/histogram/main.s) spreads its counts over
   four tables. Their `digits` (write a number's decimal digits) can be copied as it is.
7. Once every test passes, `check` proves the program in the background and says `proving`: call
   it again, with nothing to change, until it says `done`. Each call returns within about 40
   seconds, and a proof takes at most five minutes. For each generated test it can, it proves for
   every input rather than a sample that the program prints what the formulas say: whole numbers,
   decimals, each value of a `choice(…)`, texts and text formulas such as `upper` and `count`, and
   files. Long texts are proven up to a length, such as 4096 bytes, and tested beyond it: each line
   of `proofs` says how far. If the proof finds inputs where the program is wrong, `check` reports
   them like a failing test: fix `main.s` and call `check` again. "Tested, not proven" is fine: say
   why in one line. Vector code proves too, from SSE2 to AVX-512 with masks, and so do clients and
   servers on the network: what the test's peer sends arrives whole or cut, and in any pieces at
   all where the program reads it with `fill`. Programs that read and print numbers with the
   template's `parse_i64` and `write_i64` prove best.
8. Tell the user, from the `done` reply, in a few short lines and plain words: where the program
   is; what its spec promises, in two or three lines (its behaviors, its limits such as input
   sizes and ranges, and what it may touch: files, ports); that it follows every rule and passed
   all its test runs; which behaviors are proven for every input, and which up to a length, such
   as "proven for every input up to 4096 bytes, tested beyond" (and, if the proof says one solver
   answered alone, which one to install for a second check); what the spec's first example did
   (`example`); and that they can ask you to run it on their own inputs or files with `run`, or to
   change what it does. A server isn't running after `check`: offer to start it with `run`, which
   returns its address. If the reply has `changes`, say in a line each what changed after their
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
[its spec](assets/vector-template-spec.md) and [tests](assets/vector-template-tests.toml)) is a
checked SSE2 program that finds a newline and capitalizes a line 16 bytes at a time. Use AVX2 or
AVX-512 when `machine` lists them, and keep every vector access inside its buffer: see "Vectors" in
[the rules](references/power-of-10.md). With AVX-512BW, a byte test is one compare into a mask
register and the change one masked instruction: `vpsubb zmm4, zmm0, zmm1` (subtract `'a'`),
`vpcmpub k1, zmm4, zmm2, 1` (below 26), `vpsubb zmm0 {k1}, zmm0, zmm3` (subtract the case bit).
Measured against Rust and C on a 1 MiB file, that loop takes 4 times fewer cycles than
`gcc -O3 -march=native`, where the 16-byte one takes more instructions than C. Memory the program
touches costs kernel time too, a page fault per 4 KiB: work in place where the rules allow it. For
floating point, packed instructions do 2 to 8 doubles at once, or 4 to 16 singles, as long as each
element's steps stay in the formula's order.

When the spec allows threads (`- Threads: up to N`), split heavy work across cores with
`parallel_for`: [the parallel template](assets/parallel-template.s) (with
[its spec](assets/parallel-template-spec.md) and [tests](assets/parallel-template-tests.toml))
capitalizes up to a megabyte on 16 threads, each worker filling its own slot. Copy its thread code
word for word, follow "Threads" in [the rules](references/power-of-10.md), and compare
the time on the clock, which threads shorten. Starting a thread costs tens of
microseconds, so give each one at least a few hundred kilobytes of vector work: on a megabyte,
one 64-byte loop is as fast as sixteen threads.

## The rules in brief

1. **Simple control flow:** jumps stay in their function, no recursion, every path ends, and a
   chain of `cmp` + `je` ends in an assertion.
2. **Bounded loops:** `mov R, NAME` before a head label with `# bound: N`, and `dec R` + `jnz`
   back; nothing else changes `R`. A server's loop that takes connections may say
   `# bound: forever` and close with `jmp`.
3. **Fixed resources:** only the spec's system calls, each set up by `mov eax, SYS_NAME`; the stack
   moves only by push, pop, call, ret and `add`/`sub rsp`; nothing below `rsp`.
4. **Small functions:** at most 60 instructions, one `ret`, calls 16-byte aligned, and `rbx`,
   `rbp`, `r12`–`r15` saved when changed.
5. **Assertions:** `label: mov edi, <1-999>` + `call assert_fail`, reached by one conditional
   jump; at least 2 per function on average.
6. **Smallest scope:** only `_start` exported; writable data belongs to one function.
7. **Checked results:** `test rax, rax` + a conditional jump right after every system call and
   every call to a `# returns: result` function.
8. **Named constants:** for system calls, exit codes, sizes and loop counts, all defined at the
   top; `NAME_LEN` equals the size of `name`.
9. **No indirection:** no jumps through registers; indexed accesses bounds-checked first over
   their whole width (16 to 64 bytes for vectors), or inside `# unsafe: <reason>` … `# end unsafe`.
10. **Checked by tools:** warnings are errors, and `check` runs on every change.

## Never

- `;`, `/* */`, a label and an instruction on one line, `rep` or `lock`
- a constant defined after its use, `. - name` outside `.size`, `[name]` without `rip`, or `offset`
- `leave`, `enter`, `loop`, string instructions, x87, half precision or `rcp`/`rsqrt`
- floating-point steps in another order than the formula's, `* 0.1` for `/ 10.0`, or a fused
  multiply-add where the formula doesn't say `fma`
- `movdqa` or `vmovdqa` (use `movdqu`), an SSE instruction other than `movdqu` reading 16 bytes of
  memory, rounding overrides (`{rn-sae}`), gathers or scatters
- `and rsp`, `mov rsp`, or memory below `rsp`
- a loop counter in `rax`, `rcx` or `r11` across a `syscall`, or in any caller-saved register
  across a `call`
- changes to `assert_fail` or `assert_message`
