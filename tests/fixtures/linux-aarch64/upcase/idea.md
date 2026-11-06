---
type: lengua/program-idea
title: upcase
description: Copies a text file to another file with its ASCII letters in capitals.
---
# Purpose

A person can use this program to shout a text file. The arguments are the name of an input file
and the name of an output file. The program writes the contents of the input file to the output
file. It changes each ASCII letter from a to z to its capital. It keeps all other bytes as they
are, also the bytes that are not ASCII.

The input file has up to `1048576` bytes, and each file name has up to `255` bytes. If the output
file does not exist, the program makes it. If it exists, the program replaces it. If the program
cannot read the input or cannot make or write the output, it exits with code 4.

# Behaviors

## B1: Capitalizes the letters of the input file
- Given the arguments `in.txt` and `out.txt`
- And a file `in.txt` that contains `Hello, World 42!` and a newline
- When the program runs
- Then the file `out.txt` contains `HELLO, WORLD 42!` and a newline
- And it exits with code `0`

### For any `text`
- Given the arguments `in.txt` and `out.txt`
- And a file `in.txt` that contains `{text}`
- When the program runs
- Then the file `out.txt` contains `{upper(text)}`
- And it exits with code `0`

| text | runs |
|---|---|
| `` string(0, 1048576, "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 !,.@[`{~\t\n") `` | 100 |

## B2: Shows the usage line when the arguments are not two file names
- Given the argument `in.txt`
- When the program runs
- Then it prints `usage: upcase <input> <output>` and a newline to standard error
- And it exits with code `1`

### For any `a`
- Given the argument `{a}`
- When the program runs
- Then it prints `usage: upcase <input> <output>` and a newline to standard error
- And it exits with code `1`

| a |
|---|
| `choice("in.txt", "out.txt", "x", "")` |

## B3: Rejects an input file that does not exist
- Given the arguments `missing.txt` and `out.txt`
- And no file `missing.txt`
- When the program runs
- Then it prints `upcase: cannot open missing.txt` and a newline to standard error
- And it exits with code `2`

### For any `name`
- Given the arguments `{name}` and `out.txt`
- When the program runs
- Then it prints `upcase: cannot open {name}` and a newline to standard error
- And it exits with code `2`

| name |
|---|
| `choice("missing.txt", "a", "no such file.txt", "in.txt.bak")` |

## B4: Rejects an input file that is too large
- Given the arguments `big.txt` and `out.txt`
- And a file `big.txt` that contains `a` `1048576` times then `x`
- When the program runs
- Then it prints `upcase: big.txt is larger than 1048576 bytes` and a newline to standard error
- And it exits with code `3`

### For any `a`
- Given the arguments `big.txt` and `out.txt`
- And a file `big.txt` that contains `{a}x`
- When the program runs
- Then it prints `upcase: big.txt is larger than 1048576 bytes` and a newline to standard error
- And it exits with code `3`

| a | runs |
|---|---|
| `string(1048576, 1048576, "abcXYZ\n")` | 10 |

# Constraints

- Uses: files
