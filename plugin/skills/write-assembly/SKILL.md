---
name: write-assembly
description: Writes a lengua program's main.s, the x86-64 assembly (integer and vector instructions) that implements its approved spec and passes its frozen tests, in lengua's dialect checked against the lengua Power of 10 (NASA's rules adapted for assembly). Use after check_tests passes, when the lengua status tool says to write main.s, when check_code reports violations, or when the user asks for a faster program. Ends with a first run and a short report to the user.
license: Apache-2.0
compatibility: Needs the lengua MCP server from the lengua plugin, on Linux x86-64 with LLVM (llvm-mc and ld.lld).
---

# Write lengua assembly

`main.s` is the program: x86-64 assembly for Linux that you write from the approved spec and its
frozen tests. You are the compiler. lengua checks your translation: it assembles `main.s`, reads
the machine code back, matches it to `main.s` line by line, and checks the lengua Power of 10.
Nothing runs before every rule passes.

## Steps

1. Take the spec label (`sha256:…`) from the last `next_step`, which names it. If you don't have
   it, or you want the CPU's features (`machine.features`) before using BMI, POPCNT, LZCNT, ADX
   or MOVBE instructions, call the lengua `status` tool with `path`.
2. Read `<path>/spec.md` and `<path>/tests.toml`.
3. Write `<path>/main.s` in the shape of [the template](assets/main-template.s): line 1 is
   `# lengua: spec <spec_label>`, line 2 is `.intel_syntax noprefix`, and `assert_fail` and
   `assert_message` are copied word for word; the rest is the program. The template is a
   complete, checked program that prints the negative of a number (its spec and tests are
   [template-spec.md](assets/template-spec.md) and [template-tests.toml](assets/template-tests.toml));
   its `parse_i64`, `write_i64` and `write_all` can be reused as they are.
4. Write the code by [the rules](references/power-of-10.md), with
   [the Linux x86-64 reference](references/linux-x86_64.md) for system calls, the program's start
   and the calling convention.
5. Call `check_code` with `path`. It checks the rules first and lists each violation as
   `rule N (name) main.s:LINE: problem. Fix: …`. Then it runs every test: the examples, and the
   generated tests with fresh random inputs (boundary values first). A failure shows the inputs,
   the seed, what was expected and what happened, and `assertion N failed at main.s:LINE` when an
   assertion fires. Fix everything and call it again until it passes; then `lengua.lock` records
   the code.
6. Run the program once with `run`, using the spec's first example: `path`, plus `args` and
   `stdin` when it reads them.
7. Tell the user in a few short lines and plain words: where the program is, that it follows
   every rule and passed all its test runs, what `run` printed, and that they can ask you to run
   it with other inputs or to change what it does (which starts with the spec). If the code
   handles a case the spec doesn't mention, such as input longer than a buffer, say so in one line
   and offer to add it to the spec. When `check_code` reports that a generated test tried every
   value in its range, say that behavior was checked for every input in its range. Leave out
   hashes, seeds and fingerprints.

## Making it faster

Only when the user asks for a faster program: read `performance` in the `check_code` report. For
each test it gives the instructions executed (the steadiest measure of work), the CPU cycles in
the program's own code (its time, where slow instructions such as division weigh more), and the
CPU time in microseconds (which also counts the kernel's work for system calls, so many small
writes show up there; for a short run it's mostly the kernel starting and ending the process).
Change `main.s` and call `check_code` again: the tests must stay green, and the median cycles and
CPU time should drop. Every measure varies a little between runs, so compare medians. Starting and
ending a process costs about 100 microseconds whatever the program does, so speed work shows on
large inputs: compare cycles on the tests with the longest input (generated tests try the longest
text first).

The biggest win on text and arrays is usually vector instructions: one `pcmpeqb` compares 16
bytes, and AVX2 and AVX-512 do 32 and 64. [The vector template](assets/vector-template.s) (with
[its spec](assets/vector-template-spec.md) and [tests](assets/vector-template-tests.toml)) is a
checked SSE2 program that finds a newline and capitalizes a line 16 bytes at a time. Use AVX2 or
AVX-512 only when `status` lists them, and keep every vector access inside its buffer: see
"Vectors" in [the rules](references/power-of-10.md).

## The rules in brief

1. **Simple control flow:** jumps stay in their function, no recursion, every path ends, and a
   chain of `cmp` + `je` ends in an assertion.
2. **Bounded loops:** `mov R, NAME` before a head label with `# bound: N`, and `dec R` + `jnz`
   back; nothing else changes `R`.
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
10. **Checked by tools:** warnings are errors, and `check_code` runs on every change.

## Never

- `;`, `/* */`, a label and an instruction on one line, `rep` or `lock`
- a constant defined after its use, `. - name` outside `.size`, `[name]` without `rip`, or `offset`
- `leave`, `enter`, `loop`, string instructions, x87 or floating point
- `movdqa` or `vmovdqa` (use `movdqu`), an SSE instruction other than `movdqu` reading 16 bytes of
  memory, rounding overrides (`{rn-sae}`), gathers or scatters
- `and rsp`, `mov rsp`, or memory below `rsp`
- a loop counter in `rax`, `rcx` or `r11` across a `syscall`, or in any caller-saved register
  across a `call`
- changes to `assert_fail` or `assert_message`
