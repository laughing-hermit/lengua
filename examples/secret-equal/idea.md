---
type: Attested Computation
title: secret-equal
description: Prints same if the two lines of its input are the same and diff if not, with no branch on their bytes.
runtime: webassembly
computation: main.wat
parameters:
  - { name: lines, type: text, required: true, from: standard input }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
The program compares two lines of text that are secret. Standard input holds the two lines, each
with a newline at its end. Each line has `1` to `64` characters, and the two lines have the same
length. For example, `aba` and `abb` each have `3` characters.

The program prints `same` if the two lines are the same, and `diff` if they are not. It compares
all the bytes of the two lines on every run. No byte of the lines decides a branch, an address in
memory or the count of a loop. The `- Secret:` line names standard input, and `check` refuses code
that lets a byte of it decide one of these.

The program finds the two lines from the length of its input, not from their bytes. Input that is
empty, input of an odd length and input of more than `130` bytes are not two such lines. For them,
the program prints its usage line to standard error and exits with code 1. If the program cannot
read its input, it exits with code 2. If it cannot write its output, it exits with code 3.

# Behaviors
## B1: Prints same for two lines that are the same
- Given standard input that contains `aba\naba` and a newline
- When the program runs
- Then it prints `same` and a newline
- And it exits with code `0`

### For any `line`
- Given standard input that contains `{line}\n{line}` and a newline
- When the program runs
- Then it prints `same` and a newline
- And it exits with code `0`

| line |
|---|
| `string(1, 64, "abc")` |

## B2: Prints diff for two lines that differ
- Given standard input that contains `aba\nabb` and a newline
- When the program runs
- Then it prints `diff` and a newline
- And it exits with code `0`

### For any `first` and `second`
- Given standard input that contains `{first}\n{second}` and a newline
- When the program runs
- Then it prints `{if(first == second, "same", "diff")}` and a newline
- And it exits with code `0`

| first | second |
|---|---|
| `string(1, 1, "ab")` | `string(1, 1, "ab")` |
| `string(3, 3, "ab")` | `string(3, 3, "ab")` |
| `string(64, 64, "ab")` | `string(64, 64, "ab")` |

## B3: Shows the usage line for input of an odd length
- Given standard input that contains `aba`
- When the program runs
- Then it prints `usage: two lines of the same length` and a newline to standard error
- And it exits with code `1`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `usage: two lines of the same length` and a newline to standard error
- And it exits with code `1`

| text |
|---|
| `string(1, 1, "abc")` |
| `string(3, 3, "ab")` |

## B4: Shows the usage line without input
- Given no standard input
- When the program runs
- Then it prints `usage: two lines of the same length` and a newline to standard error
- And it exits with code `1`

# Constraints
- Uses: standard input
- Secret: standard input
