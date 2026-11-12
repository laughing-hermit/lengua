---
type: Attested Computation
title: copy
description: Copies the file that its first argument names to the file that its second argument names.
runtime: webassembly
computation: main.wat
parameters:
  - { name: input, type: text, required: true, from: argument 1 }
  - { name: output, type: text, required: true, from: argument 2 }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit, files]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose

A person can use this program to copy one file. The arguments are the path of an input file and
the path of an output file. The person gives each path when the program runs, and each path can
lead to any folder of the computer. The program reads the input file and writes the same bytes to
the output file.

The program opens only these two files. It can read the first file and it cannot change it. It
can write the second file and it cannot read it. It does not see the paths: it tells the computer
which argument to open.

The input file has `0` to `65536` bytes. If the output file does not exist, the program makes it.
If it exists, the program replaces its contents.

The exit codes for errors are these:
- If the program does not get two arguments, it exits with code 1.
- If the program cannot open one of the files, it exits with code 2.
- If the input file has more than 65536 bytes, it exits with code 3.
- If the program cannot read the input or write the output, it exits with code 4.

# Behaviors

## B1: Copies the input file to the output file
- Given the arguments `in.txt` and `out.txt`
- And a file `in.txt` that contains `hello` and a newline
- When the program runs
- Then the file `out.txt` contains `hello` and a newline
- And it exits with code `0`

### For any `text`
- Given the arguments `in.txt` and `out.txt`
- And a file `in.txt` that contains `{text}`
- When the program runs
- Then the file `out.txt` contains `{text}`
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 65536, "abcdefghijklmnopqrstuvwxyz0123456789 .,\t\n")` | 100 |

## B2: Shows the usage line when it does not get two arguments
- Given the argument `in.txt`
- When the program runs
- Then it prints `usage: copy <input> <output>` and a newline to standard error
- And it exits with code `1`

### For any `a`
- Given the argument `{a}`
- When the program runs
- Then it prints `usage: copy <input> <output>` and a newline to standard error
- And it exits with code `1`

| a |
|---|
| `choice("in.txt", "out.txt", "x", "")` |

## B3: Rejects an input file that does not exist
- Given the arguments `missing.txt` and `out.txt`
- And no file `missing.txt`
- When the program runs
- Then it prints `copy: cannot open the input file` and a newline to standard error
- And it exits with code `2`

### For any `name`
- Given the arguments `{name}` and `out.txt`
- When the program runs
- Then it prints `copy: cannot open the input file` and a newline to standard error
- And it exits with code `2`

| name |
|---|
| `choice("missing.txt", "a", "no such file.txt", "in.txt.bak")` |

## B4: Rejects an input file that is too large
- Given the arguments `big.txt` and `out.txt`
- And a file `big.txt` that contains `a` `65536` times then `x`
- When the program runs
- Then it prints `copy: the input file is larger than 65536 bytes` and a newline to standard error
- And it exits with code `3`

### For any `a`
- Given the arguments `big.txt` and `out.txt`
- And a file `big.txt` that contains `{a}x`
- When the program runs
- Then it prints `copy: the input file is larger than 65536 bytes` and a newline to standard error
- And it exits with code `3`

| a | runs |
|---|---|
| `string(65536, 65536, "abcXYZ\n")` | 10 |

# Constraints

- Files: reads the file its first argument names
- Files: writes the file its second argument names
