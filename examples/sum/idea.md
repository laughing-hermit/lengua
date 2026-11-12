---
type: Attested Computation
title: sum
description: Adds the whole numbers on the lines of standard input and prints the sum.
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
A shell script can use this program to add a list of numbers. The program reads standard input,
with one whole number on each line, and prints the sum of the numbers. Each number is between
`-1000000000` and `1000000000`, in decimal, with an optional minus sign at the start. A line ends
at a newline or at the end of the input. The sum of no numbers is `0`.

The input has at most `4096` bytes, and input of `4097` to `5000` bytes is too long. If a line is
not such a number, the program prints an error and exits with code 2. If the program cannot read
its input, it exits with code 4. If the program cannot write its output, it exits with code 5.

The program is made from two parts of the library of lengua. It finds the lines with the part
lines, and it reads and writes the numbers with the part decimal.

# Behaviors
## B1: Adds the numbers
- Given standard input that contains `2\n3\n`
- When the program runs
- Then it prints `5` and a newline
- And it exits with code `0`

### For any two numbers
- Given standard input that contains `{a}\n{b}\n`
- When the program runs
- Then it prints `{a + b}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 200 |

## B2: Takes a last line that has no newline
- Given standard input that contains `2\n3\n-4`
- When the program runs
- Then it prints `1` and a newline
- And it exits with code `0`

### For any three numbers
- Given standard input that contains `{a}\n{b}\n{c}`
- When the program runs
- Then it prints `{a + b + c}` and a newline
- And it exits with code `0`

| a | b | c | runs |
|---|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 100 |

## B3: Prints 0 for empty input
- Given empty standard input
- When the program runs
- Then it prints `0` and a newline
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
| `int(-1000000000, 1000000000)` | `choice("x", "", "1.5", "-", "+5", "12 ", " 12", "1000000001", "-1000000001", "123456789012345678901")` | 60 |

## B5: Rejects input that is too long
- Given standard input that contains `1\n` `2049` times
- When the program runs
- Then it prints `error: input too long` and a newline to standard error
- And it exits with code `3`

### For any long `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `error: input too long` and a newline to standard error
- And it exits with code `3`

| text | runs |
|---|---|
| `string(4097, 5000, "12\n")` | 20 |

## B6: Exits with code 5 when it cannot write its output
- Given standard input that contains `2\n3\n`
- And standard output is closed
- When the program runs
- Then it exits with code `5`

### For any two numbers
- Given standard input that contains `{a}\n{b}\n`
- And standard output is closed
- When the program runs
- Then it exits with code `5`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 20 |

# Constraints
- Uses: standard input
- Builds on: [lines](../lines/idea.md), [decimal](../decimal/idea.md)
