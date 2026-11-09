---
type: Attested Computation
title: sort
description: Sorts the lines of standard input in byte order and can drop duplicate lines.
runtime: webassembly
computation: main.wat
parameters:
  - { name: text, type: text, required: true, from: standard input }
  - { name: option, type: text, required: false, from: argument 1 }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
The program sorts text as `LC_ALL=C sort` does, for shell pipelines. It reads standard input of
up to `1048576` bytes and splits it into lines. It prints the lines in byte order, each with a
newline at its end. A last line without a newline is also a line, and the program adds a newline
when it prints it. For empty input, the program prints nothing.

With the option `-u`, it prints each different line only once, as `sort -u` does. Any other
argument, or more than one argument, is a usage error.

The tests also try short inputs of up to `64` bytes with a wider mix of characters. The program
rejects input that is longer than the limit, and it does not sort a part of it. If the program
cannot read its input, it exits with code 3. If the program cannot write its output, it exits
with code 2.

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
| `string(0, 1048576, "aefgilpr\n")` | 50 |
| `string(0, 64, "aefgilprBz \n\t")` | 300 |

## B2: Drops duplicate lines with -u
- Given the argument `-u`
- And standard input that contains `pear\napple\nfig\napple\n`
- When the program runs
- Then it prints `apple\nfig\npear\n`
- And it exits with code `0`

### For any text, uniquely
- Given the argument `-u`
- And standard input that contains `{text}`
- When the program runs
- Then it prints `{uniq(sort(text))}`
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "aefgilpr\n")` | 50 |
| `string(0, 64, "aefgilprBz \n\t")` | 300 |

## B3: Adds a newline to a last line without one
- Given standard input that contains `b\na`
- When the program runs
- Then it prints `a\nb\n`
- And it exits with code `0`

### For any lines with an unfinished last line
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{sort(text)}`
- And it exits with code `0`

| text | runs |
|---|---|
| `choice("b\na", "x", "\n\nz", "a\nb\nc", "zz\n\nzz")` | 5 |

## B4: Rejects an unknown option
- Given the argument `-r`
- And standard input that contains `b\na\n`
- When the program runs
- Then it prints `usage: sort [-u]` and a newline to standard error
- And it exits with code `1`

### For any unknown argument
- Given the argument `{arg}`
- And standard input that contains `b\na\n`
- When the program runs
- Then it prints `usage: sort [-u]` and a newline to standard error
- And it exits with code `1`

| arg |
|---|
| `choice("-r", "-uu", "u", "--unique", "-", "", "-U")` |

## B5: Rejects extra arguments
- Given the arguments `-u` and `-u`
- And standard input that contains `b\na\n`
- When the program runs
- Then it prints `usage: sort [-u]` and a newline to standard error
- And it exits with code `1`

### For any two or more arguments
- Given the arguments `{a}` and `{b}`
- And standard input that contains `{text}`
- When the program runs
- Then it prints `usage: sort [-u]` and a newline to standard error
- And it exits with code `1`

| a | b | text |
|---|---|---|
| `choice("-u", "x", "")` | `choice("-u", "-r", "")` | `string(0, 64, "ab\n")` |

## B6: Rejects input that is too long
- Given standard input that contains `a` `1048576` times then `\n`
- When the program runs
- Then it prints `error: input longer than 1048576 bytes` and a newline to standard error
- And it exits with code `4`

### For any input past the limit
- Given standard input that contains `{head}` then `\n`
- When the program runs
- Then it prints `error: input longer than 1048576 bytes` and a newline to standard error
- And it exits with code `4`

| head | runs |
|---|---|
| `string(1048576, 1048576, "az\n")` | 20 |

# Constraints
- Uses: standard input
