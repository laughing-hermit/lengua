---
type: Attested Computation
title: lines
description: A part that reads standard input and finds its lines, and prints the length of each line.
runtime: webassembly
computation: main.wat
parameters:
  - { name: text, type: text, required: true, from: standard input }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
This program is a part of the library of lengua. Other programs take its two functions: one reads
all of standard input into memory, and one finds the end of a line.

The program itself prints the length of each line of standard input, in bytes. It prints one
number on each line of its output. A line ends at a newline or at the end of the input. The input
has at most `4096` bytes, and input of `4097` to `5000` bytes is too long. The tests give each
line `0` to `40` bytes, and a last line that has no newline `1` to `40` bytes.

If the program cannot read its input, it exits with code 3. If the program cannot write its
output, it exits with code 4.

# Behaviors
## B1: Prints the length of each line
- Given standard input that contains `one\nthree\n`
- When the program runs
- Then it prints `3\n5\n`
- And it exits with code `0`

### For any two lines
- Given standard input that contains `{a}\n{b}\n`
- When the program runs
- Then it prints `{len(a)}\n{len(b)}\n`
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `string(0, 40, "abcdefghijklmnopqrstuvwxyz ")` | `string(0, 40, "abcdefghijklmnopqrstuvwxyz ")` | 200 |

## B2: Takes a last line that has no newline
- Given standard input that contains `one\nthree`
- When the program runs
- Then it prints `3\n5\n`
- And it exits with code `0`

### For any two lines
- Given standard input that contains `{a}\n{b}`
- When the program runs
- Then it prints `{len(a)}\n{len(b)}\n`
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `string(0, 40, "abcdefghijklmnopqrstuvwxyz ")` | `string(1, 40, "abcdefghijklmnopqrstuvwxyz ")` | 100 |

## B3: Prints nothing for empty input
- Given empty standard input
- When the program runs
- Then it prints nothing
- And it exits with code `0`

## B4: Rejects input that is too long
- Given standard input that contains `a` `4097` times
- When the program runs
- Then it prints `error: input too long` and a newline to standard error
- And it exits with code `2`

### For any long `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `error: input too long` and a newline to standard error
- And it exits with code `2`

| text | runs |
|---|---|
| `string(4097, 5000, "ab\n")` | 20 |

## B5: Exits with code 4 when it cannot write its output
- Given standard input that contains `one\nthree\n`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

### For any two lines
- Given standard input that contains `{a}\n{b}\n`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

| a | b | runs |
|---|---|---|
| `string(0, 40, "abcdefghijklmnopqrstuvwxyz ")` | `string(0, 40, "abcdefghijklmnopqrstuvwxyz ")` | 20 |

# Constraints
- Uses: standard input
- Builds on: [decimal](../decimal/idea.md)

# Contract
A program that builds on this part names it on the `- Builds on:` line of its idea. It copies
each function it takes from main.wat, word for word, under a `;; from: ../lines` line, with the
`$LINES_` constants that the function reads. Its idea uses standard input, since `$lines_read`
reads.

- `$lines_read`: writes 4097 bytes at `$text`. Its arguments are `$fd`, the input to read, and
  `$text`. It reads the input into the bytes at `$text` until the input ends or 4097 bytes are
  in. It gives the number of bytes it read, 0 to 4097, or a negative number when the read fails.
  4097 bytes say that the input is longer than the 4096 bytes that `$lines_end` takes.
- `$lines_end`: reads 4096 bytes at `$text`. Its arguments are `$text`, `$from` and `$length`.
  It gives the place where the line that starts at byte `$from` of the `$length` bytes at `$text`
  ends: the place of its newline, or `$length` when no newline comes after `$from`. The next
  line starts one byte after that place. `$length` is at most 4096 and `$from` is at most
  `$length`: the function stops the program with assertion 821 or 822 when they are not.
