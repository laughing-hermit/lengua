---
type: lengua/program-spec
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
- Given a file named `in.txt` holding `Hello, World 42!` followed by a newline
- And the arguments `in.txt` and `out.txt`
- When the program runs
- Then `out.txt` holds `HELLO, WORLD 42!` followed by a newline
- And it prints nothing
- And it exits with code `0`

## B2: Shows usage when the arguments are not two file names
- Given the single argument `in.txt`
- When the program runs
- Then it prints `usage: upcase <input> <output>` followed by a newline to standard error
- And it exits with code `1`

## B3: Rejects a missing input file
- Given no file named `missing.txt`
- And the arguments `missing.txt` and `out.txt`
- When the program runs
- Then it prints `upcase: cannot open missing.txt` followed by a newline to standard error
- And it exits with code `2`

## B4: Rejects an input file that is too large
- Given a file named `big.txt` holding 1048577 bytes
- And the arguments `big.txt` and `out.txt`
- When the program runs
- Then it prints `upcase: big.txt is larger than 1048576 bytes` followed by a newline to standard error
- And it exits with code `3`

# Constraints

- Allowed system calls: read, write, openat, close, exit
