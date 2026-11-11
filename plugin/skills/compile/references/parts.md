# Parts: functions a program takes, already checked

A part is a lengua program whose idea offers some of its functions to other programs. Its
`idea.md` has a `# Contract` section that lists each function, with what the function reads and
writes through the addresses it is lent. `check` holds that section to the part's `main.wat`, and
the part's own tests and proofs go through the functions. A program that needs what a part does
takes the function instead of writing it again: it copies the function into its own `main.wat`,
word for word, and `check` holds the copy to the part at every check.

`main.wat` stays the one text lengua builds, checks, tests, proves and runs. Nothing is linked
in: a taken function is in the program's own module, so every rule, the memory rules and the
secret rule among them, goes through it as through any other function, and the program's own
tests and proofs run it again as the program calls it.

Look for a part before you write a routine. The library's parts are in this skill's
[assets/parts](../assets/parts/decimal/idea.md) folder, each a folder with its `idea.md` and its
`main.wat`.

## The library

| Part | Functions it offers | What they do | Its limits |
|---|---|---|---|
| [decimal](../assets/parts/decimal/idea.md) | `$decimal_parse (at, length, into)`, `$decimal_format (value, into)` | Read a whole number from decimal text, and write one as decimal text | A number from -9223372036854775807 to 9223372036854775807; a text of at most 20 bytes |
| [lines](../assets/parts/lines/idea.md) | `$lines_read (fd, text)`, `$lines_end (text, from, length)` | Read all of an input into memory, and find where each line ends | A text of at most 4096 bytes; one byte more says the input is longer |
| [crc32](../assets/parts/crc32/idea.md) | `$crc32_update (crc, at, length)` | The CRC-32 of zlib, gzip and PNG, a piece at a time | At most 4096 bytes a call; a longer text takes a call for each piece |
| [bytes](../assets/parts/bytes/idea.md) | `$bytes_equal (a, b, length)` | Say if two runs of bytes are the same | At most 4096 bytes a call; it stops at the first byte that differs, so it is not for secrets |
| [sort64](../assets/parts/sort64/idea.md) | `$sort64 (at, count)` | Sort whole numbers of 64 bits where they are, from the smallest to the largest | At most 256 numbers a call; about 33000 steps for 256; its branches follow the numbers, so it is not for secrets |

Each part's `idea.md` has the whole contract: what each argument is, what the function gives,
and what stops the program. A part's limits are fixed: a function reaches a fixed number of
bytes through each address, whatever length it is given, and a program can't change them. A
program that needs more than a part gives writes its own function.

`examples/sum` and `examples/checksums` in lengua's repository are programs built on these
parts: the first adds the numbers on the lines of its input, with lines and decimal; the second
prints the CRC-32 of each line, with lines, crc32 and decimal.

## Take a function from a part

1. **Put the part beside the program.** Copy the part's folder into the program's bundle, as a
   sibling of the program's folder: `decimal/` beside `sum/`. Copy also each part that part
   builds on: lines and crc32 build on decimal, bytes on lines, and sort64 on lines and decimal.
   Never edit a part you copied from the library: `check` then no longer names it as the
   library's.
2. **Name it in the idea**, under `# Constraints`, with the write-idea skill:
   `- Builds on: [lines](../lines/idea.md), [decimal](../decimal/idea.md)`.
3. **Check the part until it is done**: call `check` with the part's folder as `path`, the first
   time with the user's words as `request`, as for any program. A part that builds on another
   comes after it. `check` refuses a program whose part isn't checked on this computer.
4. **Copy each function you use into main.wat, word for word**, from the part's `main.wat`,
   with its comments, and write `;; from: ../decimal` on a line of its own above its `(func`.
   The path is the part's folder, as the idea's link names it.
5. **Copy each constant the function reads**, the `(global $DECIMAL_… …)` lines, exactly as the
   part writes them. Copy also each function of the part that it calls, the same way, and the
   import of each host function it calls. Copy only the constants your copies read: an unused
   global breaks rule 10.
6. **Lend each address with the room the contract states.** A function that `reads 20 bytes at
   $at` is lent an address with 20 bytes of its region after it, whatever length you pass: size
   the region for the last place you lend. The memory rules check each call. A function that
   `writes` is lent a writable region, and never the same bytes in two arguments.
7. **Keep the function's other arguments inside its contract.** Where the contract says a
   length is at most 4096, check it before the call. A function stops the program with one of
   its own assertions when an argument is outside its contract.
8. **Number your own assertions from 1 to 799.** The library's parts use 800 to 899, each part
   its own: decimal 801 to 805, lines 821 to 824, crc32 831 and 832, bytes 841 and 842, sort64
   851 to 854.

A call, from `examples/sum`: the line's start is checked first, so the 20 bytes that
`$decimal_parse` may read from there are inside `$text`, and the result is tested at once.

```wat
                (if (i32.ge_u (local.get $from) (global.get $LINES_TEXT))
                  (then (call $assert_fail (i32.const 2)) unreachable))
                …
                (local.set $err
                  (call $decimal_parse (i32.add (global.get $text) (local.get $from))
                    (local.get $length) (global.get $number)))
                (br_if $not_a_number (local.get $err))
```

## What check says, and what it refuses

Once main.wat holds to its parts, `check`'s reply has `parts`, one line a part: the functions
main.wat takes from it; whether the part is one of lengua's library, unchanged; and what the
part's own lock records of its proofs. lengua.lock records each part with the hashes of its
`idea.md` and its `main.wat`, and the functions taken.

`check` answers `code_problems`, with what to do, when:

- a part isn't checked on this computer, has no `main.wat`, or changed after its last check;
- a part's idea has no `# Contract` section, or a proof found the part wrong;
- a function under a `;; from:` line isn't the part's function word for word (comments and
  spacing aside), or the part's contract doesn't offer it;
- a constant the function reads isn't in main.wat exactly as the part defines it, or a function
  of the part that it calls isn't taken from the part too;
- a `;; from:` line names a part the idea doesn't build on, or the idea builds on a part that
  main.wat takes nothing from;
- a call lends a region with less room than the function reaches, or a read-only region to a
  function that writes: these are the memory rules' own reports, `rule M2 (bounds)` and
  `rule M6 (immutability)`.

When a part changes, check the part again, then copy its functions again.

## What is proven

A part's proofs are of the part's own program, for the inputs its idea gives: `check` on the
part says how far they go, and the `parts` line of a program built on it repeats what the part's
lock records. They are evidence about the function. They are not a proof about your program.
lengua doesn't yet carry a part's proof into the programs that take from it: the prover follows
each taken function again, inside your program, for your program's generated tests. So what
`proofs` says of your program is what holds of it, the taken functions included, and a program
built on parts takes as long to prove as one that wrote the same functions itself.

What the copy check gives is that the function is the one the part's tests and proofs ran: the
same operators, the same constants, the same callees. It names no region and no global that
changes, so what it does depends only on its arguments, the bytes it is lent and the host.

## Write a part of your own

A part is a program like any other, with its own behaviors, tests and `main.wat`. Its own
program must use each function it offers, so that its tests and proofs go through them.

Add a `# Contract` section to its `idea.md`, an item for each function a program may take:

```markdown
# Contract
- `$decimal_parse`: reads 20 bytes at `$at`; writes 8 bytes at `$into`. It reads the whole
  number that the first `$length` bytes at `$at` write in decimal. It gives 0, or 28 when the
  text is not a number.
- `$crc32_start`: uses no memory. It gives the value that a CRC-32 starts from.
```

- **An item** is the function's name in backticks, a colon, its clauses with `;` between them,
  and a full stop. The words after the full stop are yours: what each argument is, what the
  function gives, and which values of its arguments stop the program.
- **A clause** is ``reads N bytes at `$param` `` for an address the function only reads
  through, ``writes N bytes at `$param` `` for one it writes through (it may read there too),
  or `uses no memory` for a function that takes no address. N is the farthest byte the function
  can reach from the address, for any arguments: what the memory rules find. `check` compares
  each clause with main.wat and says the clause to write when they differ.
- **A function a program can take** works only through the addresses in its parameters. It
  names no region (`$buffer`, `$buffer.size`) and no global that changes; it names functions
  and globals by `$name`, never by number; each function of the part that it calls is in the
  contract too; and the part defines it itself, so it isn't one the part took from another
  part. `check` refuses a contract that offers any other function.
- **Bound every index with a named constant** of the part, such as `$DECIMAL_TEXT`, inside the
  function: the rules then know how far it reaches. A function whose reach depends on an
  argument the rules can't bound has no contract.
- **Give its constants and its functions the part's name**, `$DECIMAL_TEN` and
  `$decimal_parse`, so that they meet no name of the program that takes them, and choose
  assertion numbers that the programs you build on it don't use.
