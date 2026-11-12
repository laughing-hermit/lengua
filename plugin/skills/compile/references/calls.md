# Calls between modules: a library in another language, and a lengua program as a library

A `main.wat` program is one module, and that stays the default: only what is in `main.wat` is
held to the rules and proven. Two lines of `# Constraints` let a program work with other code.

- `- Calls:` names a module the program calls: an outside module, which a compiler made from
  Rust, C or another language, or another lengua program of the same bundle.
- `- Exports:` lists the functions a program offers to the programs that call it.

A called module runs beside the program as an instance of its own. It has a memory of its own,
and it gets **no function of lengua's host**: it reads no file, opens no connection, prints
nothing and reads no clock, whatever the program's idea grants the program. Numbers pass in a
call. Bytes pass by copy, through lengua's `give` and `take`.

## When to use a call

Use a `- Calls:` line only when the user asks for it, or when the user's code is the reason:

- the user has a library in another language and wants this program to use that code;
- the user names the language a part of the work must be in;
- the work is in another lengua program already, and the user wants one copy of it.

Otherwise write the function in `main.wat`, or take it from a part ([parts](parts.md)): a
function in `main.wat` is checked and proven with the program, and a called module is not. Never
move code into a called module to get past a rule or a proof that `main.wat` fails.

## What holds, and what doesn't

| | A function in `main.wat` | A called outside module | A called lengua program |
|---|---|---|---|
| lengua says of it | checked, and proven where the prover reaches | **trusted, not proven** | **checked** |
| Reaches the files, the network, the clock | only what the idea grants | nothing | nothing |
| Its memory | the program's | its own, at most 1 GiB, as its module declares | its own |
| The rules of the dialect | yes | no | yes, with its own idea |
| Proofs | of the program's tests | none | of its own tests, not of this program's calls |
| The deadline of a test and of `run`, the engine's stack | yes | yes, with the program | yes |
| The program's proofs | reach it | stop at the call | stop at the call |

A generated test whose runs reach a call is **tested, not proven**: the proof line says
`a call of <module>.<function>, a function of a module it calls`. A generated test that ends
before any call, such as one that refuses its input first, is proven as before.

lengua writes no entry in `verified` for a program that calls a module, and `lengua-mcp check`
does not attest it: the entry names an idea and its `main.wat`, and such a program rests on a
third file.

## The idea's line

```markdown
# Constraints
- Uses: standard input
- Calls: crc32.wasm, built from Rust (crc32.rs) with rustc for wasm32-wasip1; its functions: buffer() -> i32, crc32(i32) -> i32
```

- The module's file is `<name>.wasm`, beside `main.wat`. The name is lowercase letters, digits,
  `-` and `_`.
- `built from …` says the language, the source and the compiler, in up to 200 characters.
  lengua records the words and can't check them.
- `its functions:` lists each function the program calls, with its numbers: `i32`, `i64`,
  `f32` or `f64`, up to 8, and the one it gives, or none. `check` holds the module's exports
  and `main.wat`'s imports to this list.
- For a lengua program, link to its idea in place of the file and the words:
  `- Calls: [crc32-library](../crc32-library/idea.md); its functions: buffer() -> i32, crc32(i32) -> i32`.
- An idea calls at most 4 modules, each on its own line. It has no `- Secret:` line and no
  `- Splits:` line: `check` refuses either beside `- Calls:`.

## The imports in `main.wat`

```wat
(import "lengua_call_1" "give" (func $give (param i32 i32 i32 i32) (result i32)))
(import "lengua_call_1" "take" (func $take (param i32 i32 i32 i32) (result i32)))
(import "crc32" "buffer" (func $buffer (result i32)))
(import "crc32" "crc32" (func $crc32 (param i32) (result i32)))
;; The called module's number for `give` and `take`: the idea's first `- Calls:` line is 0.
(global $CRC32_MODULE i32 (i32.const 0))
…
    ;; The bytes go to the module by copy, to the address it gives; then a number goes in.
    (br_if $call_failed
      (call $give (global.get $CRC32_MODULE) (call $buffer) (global.get $text)
        (local.get $got)))
    (local.set $crc (call $crc32 (local.get $got)))
```

- Import each function from the module named as the called module is, with the type the
  idea's line gives it.
- What a called function gives back is a number of the called module's: the rules treat it as
  any number, never as an address in this program's memory and never as an error number. Check
  it as you check an input.
- `give(module, at, from, len)` copies `len` bytes at `from` in this program's memory to `at`
  in the called module's memory. `take(module, at, into, len)` copies the other way. `module`
  counts the idea's `- Calls:` lines from 0. Each answers an error number, which rule 7 has you
  test: `INVAL` (28) when the idea calls no such module, the module has no memory, or the
  bytes at `at` are not inside it. The buffer in this program's memory is held to its region
  as for every call of the host.
- The address `at` is a number the called module gives, such as the address of a buffer it
  keeps. Ask the module for it with a function; never write it as a constant.
- A trap in the called module, or a call it makes to the host, ends the run. lengua places
  the trap at the line of `main.wat` that made the call.

`examples/checksum-rust` is a whole program that calls a Rust library, and
`examples/checksum-library` one that calls a lengua program.

## Build a library in Rust or in C

A called module is a library: it has no `main`, it imports nothing, and it exports the functions
and its memory. `check` refuses a module with any import, so use no standard library that needs
the system.

Rust, a `no_std` crate built as a `cdylib`:

```sh
rustc --edition 2024 --target wasm32-wasip1 --crate-type cdylib \
  -C opt-level=2 -C panic=abort -C strip=symbols \
  -C link-arg=--max-memory=16777216 -o crc32.wasm crc32.rs
```

```rust
#![no_std]

/// The address of the buffer the caller copies its bytes to.
#[unsafe(no_mangle)]
pub extern "C" fn buffer() -> *mut u8 { … }

#[unsafe(no_mangle)]
pub extern "C" fn crc32(length: u32) -> u32 { … }

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
```

C, with no standard library and no entry:

```sh
clang --target=wasm32-wasip1 --sysroot=$WASI_SDK/share/wasi-sysroot -O2 \
  -nostdlib -Wl,--no-entry -Wl,--max-memory=16777216 -Wl,--strip-all -o crc32.wasm crc32.c
```

```c
__attribute__((export_name("crc32")))
uint32_t crc32(uint32_t length) { … }
```

- State the memory's maximum (`--max-memory`), at most 1 GiB. `check` refuses a module whose
  memory declares none.
- Keep the buffers the caller fills as static arrays, and give their addresses with a
  function. Check every length the caller passes: lengua holds the module inside its own
  memory, and to nothing inside it.
- A module that exports `_initialize`, as a library built for WASI does, has it called once,
  before the program starts.

## What `check` refuses of a called outside module

- A file past 16 MiB, or a module the engine doesn't take (threads, tail calls, exceptions,
  64-bit or multiple memories, GC, relaxed SIMD, wide arithmetic, a component).
- Any import. Build the library with nothing left for a host to give.
- A memory that isn't exported as `memory`, that declares no maximum, or that may hold more than
  16,384 pages.
- A function of the idea's line that the module doesn't export, or exports with another type.

## A lengua program as a library

A program offers functions to the programs that call it with one line of its idea, and exports
them from its `main.wat`:

```markdown
- Exports: buffer() -> i32, crc32(i32) -> i32
```

```wat
(func $buffer (export "buffer") (result i32)
  (global.get $text))
(func $crc32 (export "crc32") (param $length i32) (result i32)
  …)
```

- It is a program like any other: it has its `_start`, its behaviors, its tests and its proofs,
  and `check` takes it to `done`. Have `_start` call the functions it offers, so that its own
  tests and proofs go through them.
- `check` holds the exports to the line: each listed function is exported, its own, with the
  listed type, and nothing else is exported but `_start` and `memory`.
- **An exported function takes numbers, never an address.** The caller's memory is another
  memory, so a number it passes names nothing here: `check` refuses an exported function that
  uses a parameter as an address. Keep the bytes in a region, give the region's address with a
  function, and take a length or an index, which the function checks with an assertion.
- A called program gets no function of the host. An exported function that reaches the host,
  as a failed assertion does when it prints its number, ends the caller's run there.
- An idea with `- Exports:` has no `- Secret:` line and no `- Calls:` line.
- The caller's `check` needs the library checked on this computer, to `done`. When the library
  changes, check it again, then check each program that calls it: a run refuses a program
  whose called module is no longer the one its tests ran with.

## The report to the user

Say which module the program calls and what lengua says of it, from the reply's `calls`:

- for an outside module: that it is **trusted, not proven**, what the idea says it was built
  from, that it reaches nothing of the computer and has a memory of its own, and that what
  depends on its answers is tested, not proven;
- for a lengua program: that lengua **checked** it on this computer, and what its own proofs
  cover;
- for either: which generated tests are proven and which are tested, from `proofs`, and that
  the program has no entry in `verified`.
