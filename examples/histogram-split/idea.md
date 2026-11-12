---
type: Attested Computation
title: histogram-split
description: Counts how many times each byte value occurs in standard input, with the work split into pieces that run at once.
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
The program counts how many times each byte value occurs in standard input of any length. It reads
one part of 8 MiB at a time, so it can read input that is larger than its memory. For each byte
value from `0` to `255` that occurs, it prints a line, in the order of the values. The line is the
value in decimal, a space, the count in decimal and a newline. For empty input, it prints nothing.

The program splits each part of the input into `8` pieces. A piece is one more run of this
program, and it gets its share of the bytes as its standard input. The pieces run at the same time
when the computer has more than one core. A piece counts the bytes of its share and prints the
lines for them. The program adds the counts of the pieces, so the result is the same on every
computer.

The tests give it up to `1048576` bytes. If the program cannot read its input, it exits with code 2.
If the program cannot write its output, it exits with code 3. The test of a write that fails gives
it `1` to `64` bytes.

# Behaviors
## B1: Counts each byte value
- Given standard input that contains `\n aa a\n`
- When the program runs
- Then it prints `10 2\n32 2\n97 3\n`
- And it exits with code `0`

### For any `text`
The input starts with a newline, a space and `a`, so each of the three values occurs.

- Given standard input that contains `\n a{text}`
- When the program runs
- Then it prints `10 {count(text, "\n") + 1}\n32 {count(text, " ") + 1}\n97 {count(text, "a") + 1}\n`
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, " a\n")` | 30 |
| `string(0, 64, " a\n")` | 200 |

## B2: Counts a run of one byte
- Given standard input that contains `x` `5` times
- When the program runs
- Then it prints `120 5` and a newline
- And it exits with code `0`

### For any `run`
- Given standard input that contains `{run}`
- When the program runs
- Then it prints `120 {len(run)}` and a newline
- And it exits with code `0`

| run | runs |
|---|---|
| `string(1, 1048576, "x")` | 30 |

## B3: Prints nothing for empty input
- Given empty standard input
- When the program runs
- Then it prints nothing
- And it exits with code `0`

## B4: Exits with code 3 when it cannot write its output
- Given standard input that contains `banana`
- And standard output is closed
- When the program runs
- Then it exits with code `3`

### For any `text`
- Given standard input that contains `{text}`
- And standard output is closed
- When the program runs
- Then it exits with code `3`

| text | runs |
|---|---|
| `string(1, 64, "abn \n")` | 20 |

## B5: Counts the bytes of its share as a piece
- Given standard input that contains `\n aa a\n`
- And the program is a piece
- When the program runs
- Then it prints `10 2\n32 2\n97 3\n`
- And it exits with code `0`

### For any `share`
The share starts with a newline, a space and `a`, so each of the three values occurs.

- Given standard input that contains `\n a{share}`
- And the program is a piece
- When the program runs
- Then it prints `10 {count(share, "\n") + 1}\n32 {count(share, " ") + 1}\n97 {count(share, "a") + 1}\n`
- And it exits with code `0`

| share | runs |
|---|---|
| `string(0, 1048576, " a\n")` | 30 |
| `string(0, 64, " a\n")` | 100 |

# Constraints
- Uses: standard input
- Splits: its work into up to 8 pieces, each of up to 1048576 bytes, each gives back 8192 bytes
