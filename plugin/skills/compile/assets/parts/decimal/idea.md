---
type: Attested Computation
title: decimal
description: A part that reads and writes whole numbers in decimal, and prints the negative of the number in its argument.
runtime: webassembly
computation: main.wat
parameters:
  - { name: n, type: integer, required: true, from: argument 1 }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
This program is a part of the library of lengua. Other programs take its two functions: one reads
a whole number from decimal text, and one writes a whole number as decimal text.

The program itself shows that the two functions agree. It reads one whole number between
`-9223372036854775807` and `9223372036854775807` from its argument and prints its negative. The
number is in decimal, with an optional minus sign at the start and at most 19 digits. If the
program cannot write its output, it exits with code 3.

# Behaviors
## B1: Negates a number
- Given the argument `7`
- When the program runs
- Then it prints `-7` and a newline
- And it exits with code `0`

### For any `n`
- Given the argument `{n}`
- When the program runs
- Then it prints `{-n}` and a newline
- And it exits with code `0`

| n | runs |
|---|---|
| `int(-9223372036854775807, 9223372036854775807)` | 200 |

## B2: Shows the usage line when it does not get one argument
- Given no arguments
- When the program runs
- Then it prints `usage: decimal <n>` and a newline to standard error
- And it exits with code `1`

## B3: Rejects text that is not a number
- Given the argument `12x`
- When the program runs
- Then it prints `error: not a number` and a newline to standard error
- And it exits with code `2`

### For any `s`
- Given the argument `{s}`
- When the program runs
- Then it prints `error: not a number` and a newline to standard error
- And it exits with code `2`

| s | runs |
|---|---|
| `choice("12x", "abc", "", "-", "--5", "+5", "1-2", "1:", "9223372036854775808", "-9223372036854775808", "00000000000000000001", "123456789012345678901")` | 60 |

## B4: Exits with code 3 when it cannot write its output
- Given the argument `7`
- And standard output is closed
- When the program runs
- Then it exits with code `3`

### For any `n`
- Given the argument `{n}`
- And standard output is closed
- When the program runs
- Then it exits with code `3`

| n | runs |
|---|---|
| `int(-9223372036854775807, 9223372036854775807)` | 20 |

# Constraints
- Uses: nothing

# Contract
A program that builds on this part names it on the `- Builds on:` line of its idea. It copies
each function it takes from main.wat, word for word, under a `;; from: ../decimal` line, with
the `$DECIMAL_` constants that the function reads.

- `$decimal_parse`: reads 20 bytes at `$at`; writes 8 bytes at `$into`. Its arguments are `$at`,
  `$length` and `$into`. It reads the whole number that the first `$length` bytes at `$at` write
  in decimal: an optional minus sign, then 1 to 19 digits, and nothing else. It writes the number
  at `$into`, as 64 bits, and gives 0. It gives 28, and writes nothing, when the text is not such
  a number, or when the number is not between -9223372036854775807 and 9223372036854775807.
  The caller lends `$at` with room for 20 bytes, whatever `$length` is.
- `$decimal_format`: writes 20 bytes at `$into`. Its arguments are `$value`, as 64 bits, and
  `$into`. It writes `$value` in decimal at `$into`: a minus sign when it is negative, then its
  digits, with no zero at the start but for 0 itself. It gives the number of bytes it wrote, 1 to
  20. `$value` is never -9223372036854775808: the function stops the program with assertion 802
  for that number.
