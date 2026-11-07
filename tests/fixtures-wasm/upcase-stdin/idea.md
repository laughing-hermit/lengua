---
type: Attested Computation
title: upcase-stdin
description: Copies standard input of any length to standard output with every ASCII letter in capitals, 64 bytes at a time.
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
This program is a fast filter for shell pipelines. It copies standard input of any length to
standard output, with each ASCII letter `a` to `z` in capitals. It does not change the other
bytes, and this includes the bytes of UTF-8 characters. It copies one piece of 64 KiB at a time,
so it can copy input that is larger than its memory.

The tests give it up to `1048576` bytes. They also try short inputs of up to `64` bytes with the
characters next to the letters. If the program cannot read its input, it exits with code 3. If the
program cannot write its output, it exits with code 4. The test of a write that fails gives it `1`
to `65536` bytes.

# Behaviors
## B1: Shouts its input
- Given standard input that contains `hello, world`
- When the program runs
- Then it prints `HELLO, WORLD`
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{upper(text)}`
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "abcdehlorwxyzABZ019 ,.\n")` | 40 |
| `string(0, 64, "az{}@[AZ09 \n~é")` | 300 |

## B2: Exits with code 4 when it cannot write its output
- Given standard input that contains `hello`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

### For any `text`
- Given standard input that contains `{text}`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

| text | runs |
|---|---|
| `string(1, 65536, "abcdehlorwxyz019 ,.")` | 20 |

# Constraints
- Uses: standard input
