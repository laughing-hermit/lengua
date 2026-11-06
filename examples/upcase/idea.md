---
type: lengua/program-idea
title: upcase
description: Copies a text file to another file with its ASCII letters in capitals.
---
# Purpose

Lets someone shout a text file: given the name of an input file and the name of an output file,
it writes the input's contents to the output file with every ASCII letter from a to z turned into
its capital, and every other byte, including non-ASCII bytes, left as it is. The input file holds
up to `1048576` bytes, and each file name is up to `255` bytes long. The output file is created if
it doesn't exist and replaced if it does. If reading the input or creating or writing the output
fails, the program exits with code 4.

# Behaviors

## B1: Capitalizes the letters of the input file
- Given the arguments `in.txt` and `out.txt`
- And a file `in.txt` holding `Hello, World 42!` followed by a newline
- When the program runs
- Then the file `out.txt` holds `HELLO, WORLD 42!` followed by a newline
- And it exits with code `0`

### For any `text`
- Given the arguments `in.txt` and `out.txt`
- And a file `in.txt` holding `{text}`
- When the program runs
- Then the file `out.txt` holds `{upper(text)}`
- And it exits with code `0`

| text | runs |
|---|---|
| `` string(0, 1048576, "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 !,.@[`{~\t\n") `` | 100 |

## B2: Shows usage when the arguments are not two file names
- Given the argument `in.txt`
- When the program runs
- Then it prints `usage: upcase <input> <output>` followed by a newline to standard error
- And it exits with code `1`

### For any `a`
- Given the argument `{a}`
- When the program runs
- Then it prints `usage: upcase <input> <output>` followed by a newline to standard error
- And it exits with code `1`

| a |
|---|
| `choice("in.txt", "out.txt", "x", "")` |

## B3: Rejects a missing input file
- Given the arguments `missing.txt` and `out.txt`
- And no file `missing.txt`
- When the program runs
- Then it prints `upcase: cannot open missing.txt` followed by a newline to standard error
- And it exits with code `2`

### For any `name`
- Given the arguments `{name}` and `out.txt`
- When the program runs
- Then it prints `upcase: cannot open {name}` followed by a newline to standard error
- And it exits with code `2`

| name |
|---|
| `choice("missing.txt", "a", "no such file.txt", "in.txt.bak")` |

## B4: Rejects an input file that is too large
- Given the arguments `big.txt` and `out.txt`
- And a file `big.txt` holding `a` repeated `1048576` times followed by `x`
- When the program runs
- Then it prints `upcase: big.txt is larger than 1048576 bytes` followed by a newline to standard error
- And it exits with code `3`

### For any `a`
- Given the arguments `big.txt` and `out.txt`
- And a file `big.txt` holding `{a}x`
- When the program runs
- Then it prints `upcase: big.txt is larger than 1048576 bytes` followed by a newline to standard error
- And it exits with code `3`

| a | runs |
|---|---|
| `string(1048576, 1048576, "abcXYZ\n")` | 10 |

# Constraints

- Uses: files
