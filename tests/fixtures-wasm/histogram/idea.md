---
type: Attested Computation
title: histogram
description: Counts how many times each byte value occurs in standard input of any length and prints the counts.
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
one piece of 64 KiB at a time, so it can read input that is larger than its memory. For each byte
value from `0` to `255` that occurs, it prints a line, in the order of the values. The line is the
value in decimal, a space, the count in decimal and a newline. For empty input, it prints nothing.

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

# Constraints
- Uses: standard input
