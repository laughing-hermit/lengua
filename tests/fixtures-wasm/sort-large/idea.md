---
type: Attested Computation
title: sort-large
description: Sorts the lines of standard input of up to 64 MiB in byte order, as LC_ALL=C sort does.
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
The program sorts text as `LC_ALL=C sort` does, for large files in shell pipelines. It reads
standard input of up to `67108864` bytes, in up to `4194304` lines. It prints the lines in byte
order, each with a newline at its end. A line that starts another line comes first. A last line
without a newline is also a line, and the program adds a newline when it prints it. For empty
input, the program prints nothing.

The tests give it up to `1048576` bytes. They also try short inputs of up to `64` bytes with a
wider mix of characters. The program rejects input that is longer than a limit, and it sorts no
part of it. If the program cannot read its input, it exits with code 3. If the program cannot write
its output, it exits with code 2. The test of a write that fails gives it `1` to `64` letters.

# Behaviors
## B1: Sorts lines
- Given standard input that contains `pear\napple\nfig\napple\n`
- When the program runs
- Then it prints `apple\napple\nfig\npear\n`
- And it exits with code `0`

### For any text
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{sort(text)}`
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "ab \n")` | 30 |
| `string(0, 1048576, "abcdefghijklmnopqrstuvwxyz\n")` | 10 |
| `string(0, 64, "abcAB \n\t~é")` | 300 |

## B2: Rejects input of too many lines
- Given standard input that contains `\n` `4194304` times then `x\n`
- When the program runs
- Then it prints `error: more than 4194304 lines` and a newline to standard error
- And it exits with code `4`

### For any input of too many lines
- Given standard input that contains `\n` `4194304` times then `{tail}`
- When the program runs
- Then it prints `error: more than 4194304 lines` and a newline to standard error
- And it exits with code `4`

| tail | runs |
|---|---|
| `string(1, 64, "ab\n")` | 10 |

## B3: Exits with code 2 when it cannot write its output
- Given standard input that contains `pear\napple\n`
- And standard output is closed
- When the program runs
- Then it exits with code `2`

### For any `text`
- Given standard input that contains `{text}`
- And standard output is closed
- When the program runs
- Then it exits with code `2`

| text | runs |
|---|---|
| `string(1, 64, "abcdeghnot")` | 20 |

# Constraints
- Uses: standard input
