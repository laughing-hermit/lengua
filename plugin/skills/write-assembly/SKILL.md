---
name: write-assembly
description: Writes a lengua program's main.s, the x86-64 assembly that implements its approved spec and passes its frozen tests, in lengua's dialect checked against the lengua Power of 10 (NASA's rules adapted for assembly). Use after check_tests passes, when the lengua status tool says to write main.s, when check_code reports violations, or to make a checked program faster. Ends with check_code.
license: Apache-2.0
compatibility: Needs the lengua MCP server from the lengua plugin, on Linux x86-64 with LLVM (llvm-mc and ld.lld).
---

# Write lengua assembly

`main.s` is the program: x86-64 assembly for Linux that you write from the approved spec and its
frozen tests. You are the compiler. lengua checks your translation: it assembles `main.s`, reads
the machine code back, matches it to `main.s` line by line, and checks the lengua Power of 10.
Nothing runs before every rule passes.

## Steps

1. Call the lengua `status` tool with `path` set to the program folder's absolute path. Note
   `program.spec_label` and the CPU features under `machine.features`.
2. Read `<path>/spec.md` and `<path>/tests.toml`.
3. Copy [the template](assets/main-template.s) to `<path>/main.s` and set line 1 to
   `# lengua: spec <spec_label>`. Keep `.intel_syntax noprefix`, `assert_fail` and
   `assert_message` exactly; replace the rest with the program. The template is a complete,
   checked program that prints the negative of a number (its spec and tests are
   [template-spec.md](assets/template-spec.md) and [template-tests.toml](assets/template-tests.toml));
   its `parse_i64`, `write_i64` and `write_all` can be reused as they are.
4. Write the code by [the rules](references/power-of-10.md).
5. Call `check_code` with `path`. It lists each violation as `rule N (name) main.s:LINE: problem.
   Fix: …`. Fix them all and call it again until everything passes.
6. Follow the `next_step` it returns.

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
9. **No indirection:** no jumps through registers; indexed accesses bounds-checked first, or
   inside `# unsafe: <reason>` … `# end unsafe`.
10. **Checked by tools:** warnings are errors, and `check_code` runs on every change.

## Never

- `;`, `/* */`, a label and an instruction on one line, `rep` or `lock`
- a constant defined after its use, `. - name` outside `.size`, `[name]` without `rip`, or `offset`
- `leave`, `enter`, `loop`, string instructions, x87, SSE or AVX
- `and rsp`, `mov rsp`, or memory below `rsp`
- a loop counter in `rax`, `rcx` or `r11` across a `syscall`, or in any caller-saved register
  across a `call`
- changes to `assert_fail` or `assert_message`
