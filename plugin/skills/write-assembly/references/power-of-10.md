# The lengua Power of 10 (dialect 1)

`main.s` is x86-64 assembly for Linux in Intel syntax. `check_code` builds it, reads the machine
code back, matches it to `main.s` line by line (operands included), and checks the rules below.
Nothing runs before every rule passes. Each violation names its rule, its line and a fix.

These are NASA's "Power of 10" rules for safety-critical code (Holzmann, 2006), adapted for
assembly and combined with ideas from C, C++ and Rust.

## Layout

- **Line 1** is the spec label from the `status` tool: `# lengua: spec sha256:<64 hex digits>`.
- **Line 2** is `.intel_syntax noprefix`.
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
  `.zero NAME` and `.skip NAME` (sizes are named constants), and `.balign N`. `.bss` holds only
  `.zero` and `.skip`.
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
| Register | `rax`, `eax`, `ax`, `al`, `r8`, `r8d`, `r8w`, `r8b` |
| Number | `10`, `0x1f`, `'0'`, `MAX_DIGITS`, `OUT_BUF_LEN - 1` |
| Memory | `qword ptr [rsp + 16]`, `byte ptr [rdi + rcx]`, `[rbx + rcx*8 + 8]`, `qword ptr [rip + out_buf]` |
| Target | `.Lloop` (jumps), `parse_i64` (calls) |

Memory needs a base register. Data symbols are reached only as `[rip + name]`, plus an optional
constant, and `lea reg, [rip + name]` loads an address. Write the size (`byte ptr`, `word ptr`,
`dword ptr`, `qword ptr`) when no register operand gives it.

## Instruction set

Dialect 1 is integer instructions and `syscall`:

- `mov movzx movsx movsxd lea push pop xchg bswap` and every `cmovCC`
- `add adc sub sbb neg inc dec mul imul div idiv cbw cwde cdqe cwd cdq cqo`
- `and or xor not test cmp shl sal shr sar rol ror shld shrd bt bts btr btc bsf bsr` and every
  `setCC`
- `jmp`, every `jCC`, `call ret syscall nop`

When the CPU has them (the `status` tool lists its features): BMI1 (`andn bextr blsi blsmsk blsr
tzcnt`), BMI2 (`bzhi mulx pdep pext rorx sarx shlx shrx`), POPCNT (`popcnt`), LZCNT (`lzcnt`),
ADX (`adcx adox`) and MOVBE (`movbe`).

Everything else is out: x87, MMX, SSE and AVX (vector instructions come in dialect 2), string
instructions (`movs`, `stos`, …), `loop`, `jrcxz`, `enter`, `leave`, `int`, `pushf` and `popf`.

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

### 3. Fixed resources

No memory allocation: a program uses only the system calls its spec allows, and each `syscall`
is chosen by `mov eax, SYS_NAME` in the same straight-line code (no label or call between). The
stack changes only through 64-bit `push` and `pop`, `call`, `ret`, and `sub rsp, N` / `add rsp,
N`; every path into a line has the same depth, and nothing reads or writes below `rsp` (no red
zone). `check_code` reports the deepest stack over all call paths.

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
code 134, which `check_code` reports as the assertion's line. Copy `assert_fail` and
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
operand with an index register needs, since the last label, a `cmp` of that register followed by
a conditional jump, and no change to the register before the access:

```
.Lparse_loop:  # bound: 20
    cmp rcx, MAX_CHARS
    jae .Lparse_assert_index
    movzx edx, byte ptr [rdi + rcx]
```

Where the check is provably unnecessary, wrap the access in `# unsafe: <reason>` …
`# end unsafe`. Only bounds checks can be skipped this way, and `check_code` reports every unsafe
block.

### 10. Checked by tools

The assembler and linker treat warnings as errors. `check_code` runs on every change, and nothing
runs or is stamped before every rule passes. The lock records `dialect = 1`.
