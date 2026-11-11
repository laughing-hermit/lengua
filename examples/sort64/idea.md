---
type: Attested Computation
title: sort64
description: A part that sorts whole numbers of 64 bits, and prints the numbers on the lines of standard input in order.
runtime: webassembly
computation: main.wat
parameters:
  - { name: numbers, type: text, required: true, from: standard input }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
This program is a part of the library of lengua. Other programs take its function, which sorts
whole numbers of 64 bits in memory, from the smallest to the largest.

The program itself reads standard input, with one whole number on each line, and prints the
numbers from the smallest to the largest. It prints one number on each line. Each number is
between `-999` and `999`, in decimal, with an optional minus sign at the start. A
line ends at a newline or at the end of the input. The input has at most `256` numbers and at
most `4096` bytes, and input of `4097` to `5000` bytes is too long.

If a line is not a number, the program prints an error and exits with code 2. If the input has
more than 256 numbers, it prints an error and exits with code 3. If the program cannot read its
input, it exits with code 5. If the program cannot write its output, it exits with code 6.

# Behaviors
## B1: Prints the numbers in order
- Given standard input that contains `3\n1\n2\n`
- When the program runs
- Then it prints `1\n2\n3\n`
- And it exits with code `0`

### For any three numbers
- Given standard input that contains `{a}\n{b}\n{c}\n`
- When the program runs
- Then it prints `{min(a, b, c)}\n{a + b + c - min(a, b, c) - max(a, b, c)}\n{max(a, b, c)}\n`
- And it exits with code `0`

| a | b | c | runs |
|---|---|---|---|
| `int(-999, 999)` | `int(-999, 999)` | `int(-999, 999)` | 200 |

## B2: Takes a last line that has no newline
- Given standard input that contains `2\n1`
- When the program runs
- Then it prints `1\n2\n`
- And it exits with code `0`

### For any two numbers
- Given standard input that contains `{a}\n{b}`
- When the program runs
- Then it prints `{min(a, b)}\n{max(a, b)}\n`
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-999, 999)` | `int(-999, 999)` | 100 |

## B3: Prints nothing for empty input
- Given empty standard input
- When the program runs
- Then it prints nothing
- And it exits with code `0`

## B4: Rejects a line that is not a number
- Given standard input that contains `2\nx\n`
- When the program runs
- Then it prints `error: not a number` and a newline to standard error
- And it exits with code `2`

### For any `a` and `s`
- Given standard input that contains `{a}\n{s}\n`
- When the program runs
- Then it prints `error: not a number` and a newline to standard error
- And it exits with code `2`

| a | s | runs |
|---|---|---|
| `int(-999, 999)` | `choice("x", "", "1.5", "-", "+5", "12 ", " 12", "123456789012345678901")` | 60 |

## B5: Rejects more than 256 numbers
- Given standard input that contains `1\n` `256` times then `5\n`
- When the program runs
- Then it prints `error: too many numbers` and a newline to standard error
- And it exits with code `3`

### For any last number
- Given standard input that contains `1\n` `256` times then `{a}\n`
- When the program runs
- Then it prints `error: too many numbers` and a newline to standard error
- And it exits with code `3`

| a | runs |
|---|---|
| `int(-999, 999)` | 20 |

## B6: Rejects input that is too long
- Given standard input that contains `1\n` `2049` times
- When the program runs
- Then it prints `error: input too long` and a newline to standard error
- And it exits with code `4`

### For any long `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `error: input too long` and a newline to standard error
- And it exits with code `4`

| text | runs |
|---|---|
| `string(4097, 5000, "12\n")` | 20 |

## B7: Exits with code 6 when it cannot write its output
- Given standard input that contains `3\n1\n2\n`
- And standard output is closed
- When the program runs
- Then it exits with code `6`

### For any three numbers
- Given standard input that contains `{a}\n{b}\n{c}\n`
- And standard output is closed
- When the program runs
- Then it exits with code `6`

| a | b | c | runs |
|---|---|---|---|
| `int(-999, 999)` | `int(-999, 999)` | `int(-999, 999)` | 20 |

# Constraints
- Uses: standard input
- Builds on: [lines](../lines/idea.md), [decimal](../decimal/idea.md)

# Contract
A program that builds on this part names it on the `- Builds on:` line of its idea. It copies the
function from main.wat, word for word, under a `;; from: ../sort64` line, with the `$SORT64_`
constants that the function reads.

- `$sort64`: writes 2048 bytes at `$at`. Its arguments are `$at` and `$count`. It sorts the
  `$count` numbers at `$at`, 8 bytes each, as 64 bits with a sign, from the smallest to the
  largest, in the same memory. It looks at every pair of numbers, so 256 numbers take about
  33000 steps, and its branches follow the numbers, so it is not for numbers that are secret.
  `$count` is at most 256: the function stops the program with assertion 851 when it is not.
  The caller lends `$at` with room for 2048 bytes, whatever `$count` is.
