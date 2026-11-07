---
type: Attested Computation
title: shout
description: Reads a line from standard input and prints it in capital letters.
runtime: webassembly
computation: main.wat
parameters:
  - { name: line, type: text, required: true, from: standard input }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
A shell script can use this program to shout a line of text. The program reads one line of `1` to
`64` lowercase letters and spaces from standard input. It prints the line in capital letters. The
line ends at a newline or at the end of the input. A line of `65` to `4096` characters is too long.

If the program cannot read its input, it exits with code 3. If the program cannot write its output,
it exits with code 4.

# Behaviors
## B1: Shouts a line
- Given standard input that contains `hello world` and a newline
- When the program runs
- Then it prints `HELLO WORLD` and a newline
- And it exits with code `0`

### For any `line`
- Given standard input that contains `{line}` and a newline
- When the program runs
- Then it prints `{upper(line)}` and a newline
- And it exits with code `0`

| line | runs |
|---|---|
| `string(1, 64, "abcdefghijklmnopqrstuvwxyz ")` | 200 |

## B2: Rejects empty input
- Given no arguments
- When the program runs
- Then it prints `error: no input` and a newline to standard error
- And it exits with code `1`

## B3: Rejects a line that is too long
- Given standard input that contains `aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa` and a newline
- When the program runs
- Then it prints `error: line too long` and a newline to standard error
- And it exits with code `2`

### For any `line`
- Given standard input that contains `{line}` and a newline
- When the program runs
- Then it prints `error: line too long` and a newline to standard error
- And it exits with code `2`

| line | runs |
|---|---|
| `string(65, 4096, "abcdefghijklmnopqrstuvwxyz ")` | 50 |

## B4: Exits with code 4 when it cannot write its output
- Given standard input that contains `hello world` and a newline
- And standard output is closed
- When the program runs
- Then it exits with code `4`

### For any `line`
- Given standard input that contains `{line}` and a newline
- And standard output is closed
- When the program runs
- Then it exits with code `4`

| line | runs |
|---|---|
| `string(1, 64, "abcdefghijklmnopqrstuvwxyz ")` | 20 |

# Constraints
- Uses: standard input
