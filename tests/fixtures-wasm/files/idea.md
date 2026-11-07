---
type: Attested Computation
title: files
description: Reads every file in the current folder and prints how many files there are and how many bytes they hold.
runtime: webassembly
computation: main.wat
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
A benchmark uses this program to measure the cost of many small files. The program reads each file
that is directly in the current folder, from its start to its end. Then it prints the number of
files, a space, the number of bytes in all of them and a newline. It does not count folders, and it
does not look in them. It does not change a file.

The folder has up to `65536` entries, and each name has up to `255` bytes. The tests give it three
files of up to `1048576`, `65536` and `255` bytes. If the program cannot list the folder or read a
file, it exits with code 3. If the program cannot write its output, it exits with code 2.

# Behaviors
## B1: Counts the files and their bytes
- Given a file `a.txt` that contains `hello`
- And a file `b.txt` that contains `world!`
- And a file `c.txt` that contains `q`
- And an empty folder `sub`
- When the program runs
- Then it prints `3 12` and a newline
- And it exits with code `0`

### For any `a`, `b` and `c`
- Given a file `a.txt` that contains `{a}`
- And a file `b.txt` that contains `{b}`
- And a file `c.txt` that contains `{c}`
- And an empty folder `sub`
- When the program runs
- Then it prints `3 {len(a) + len(b) + len(c)}` and a newline
- And it exits with code `0`

| a | b | c | runs |
|---|---|---|---|
| `string(0, 1048576, "ab\n")` | `string(0, 65536, "xyz")` | `string(0, 255, "q")` | 30 |

## B2: Prints zeros for an empty folder
- Given no arguments
- When the program runs
- Then it prints `0 0` and a newline
- And it exits with code `0`

## B3: Exits with code 2 when it cannot write its output
- Given a file `a.txt` that contains `hello`
- And standard output is closed
- When the program runs
- Then it exits with code `2`

### For any `a`
- Given a file `a.txt` that contains `{a}`
- And standard output is closed
- When the program runs
- Then it exits with code `2`

| a | runs |
|---|---|
| `string(0, 255, "q")` | 20 |

# Constraints
- Uses: files
