---
type: Attested Computation
title: bytes
description: A part that compares two runs of bytes, and says if the first two lines of standard input are the same.
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
This program is a part of the library of lengua. Other programs take its function, which says if
two runs of bytes are the same.

The program itself compares the first two lines of standard input. It prints `same` when they
hold the same bytes, and `different` when they do not. A line ends at a newline or at the end of
the input, and a line that is not there is an empty line. The input has at most `4096` bytes, and
input of `4097` to `5000` bytes is too long. The tests give each line `0` to `40` bytes. They
also give short lines of up to `3` bytes, so that many pairs of lines are the same.

If the program cannot read its input, it exits with code 3. If the program cannot write its
output, it exits with code 4.

# Behaviors
## B1: Compares two lines
- Given standard input that contains `ab\nab\n`
- When the program runs
- Then it prints `same` and a newline
- And it exits with code `0`

### For any two lines
The short lines try every pair, so the test meets pairs that are the same.

- Given standard input that contains `{a}\n{b}\n`
- When the program runs
- Then it prints `{if(a == b, "same", "different")}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `string(0, 3, "ab")` | `string(0, 3, "ab")` | 100 |
| `string(0, 40, "abcdefghijklmnopqrstuvwxyz ")` | `string(0, 40, "abcdefghijklmnopqrstuvwxyz ")` | 100 |

## B2: Takes a last line that has no newline
- Given standard input that contains `ab\nba`
- When the program runs
- Then it prints `different` and a newline
- And it exits with code `0`

### For any two lines
- Given standard input that contains `{a}\n{b}`
- When the program runs
- Then it prints `{if(a == b, "same", "different")}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `string(0, 3, "ab")` | `string(0, 3, "ab")` | 100 |

## B3: Takes empty input as two empty lines
- Given empty standard input
- When the program runs
- Then it prints `same` and a newline
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
- Given standard input that contains `ab\nab\n`
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
| `string(0, 3, "ab")` | `string(0, 3, "ab")` | 20 |

# Constraints
- Uses: standard input
- Builds on: [lines](../lines/idea.md)

# Contract
A program that builds on this part names it on the `- Builds on:` line of its idea. It copies the
function from main.wat, word for word, under a `;; from: ../bytes` line, with the `$BYTES_`
constant that the function reads.

- `$bytes_equal`: reads 4096 bytes at `$a`; reads 4096 bytes at `$b`. Its arguments are `$a`,
  `$b` and `$length`. It gives 1 when the `$length` bytes at `$a` and the `$length` bytes at
  `$b` are the same, and 0 when they are not. It stops at the first byte that differs, so it is
  not for bytes that are secret. `$length` is at most 4096: the function stops the program with
  assertion 841 when it is not. The caller lends `$a` and `$b` with room for 4096 bytes each,
  whatever `$length` is, and the two may be the same bytes.
