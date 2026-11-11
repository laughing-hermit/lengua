---
type: Attested Computation
title: vowels
description: Counts the vowels in standard input and prints the count.
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
A writer can use this program to count the vowels in a text. The program reads standard input of
any length, one piece at a time. It counts each a, e, i, o and u, in small letters and in capital
letters. It prints the count and a newline.

The tests give it text of up to `1048576` bytes, and short texts of up to `64` bytes. If the
program cannot write its output, it exits with code 2. If it cannot read its input, it exits with
code 3.

# Behaviors
## B1: Counts the vowels
- Given standard input that contains `Hello World`
- When the program runs
- Then it prints `3` and a newline
- And it exits with code `0`

### For any text
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{count(lower(text), "a") + count(lower(text), "e") + count(lower(text), "i") + count(lower(text), "o") + count(lower(text), "u")}` and a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "aeiouAEIOUbcdHlrWxyz \n")` | 50 |
| `string(0, 64, "aAeEiIoOuUbcBC")` | 100 |

## B2: Exits with code 2 when it cannot write its output
- Given standard input that contains `abc`
- And standard output is closed
- When the program runs
- Then it exits with code `2`

### For any text, with standard output closed
- Given standard input that contains `{text}`
- And standard output is closed
- When the program runs
- Then it exits with code `2`

| text | runs |
|---|---|
| `string(0, 1048576, "abcdeiouxyz")` | 20 |

# Constraints
- Uses: standard input
