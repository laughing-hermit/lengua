---
type: lengua/program-spec
title: wc
description: Counts the lines and words on standard input like `wc -lw`, as fast as possible using AVX-512.
---
# Purpose
A drop-in for `wc -lw` on very large files, for people who count lines and words in huge logs and
datasets and want the answer as fast as the machine can give it. It reads standard input of any
length, a piece at a time, so a file is counted with `./wc < big.txt`; the tests use standard input
of up to `1048576` bytes, and short texts of up to `64` characters that mix in letters such as `é` that take two bytes.

A line is counted for every newline byte. A word is a run of one or more bytes that are not
whitespace, where whitespace is the space, tab, newline, vertical tab, form feed and carriage
return bytes; every other byte, including control bytes and bytes above 127, is part of a word.

It prints the line count, a space, then the word count, followed by a newline, each count
right-aligned in a field of 7 characters padded with spaces on the left; a count longer than 7
digits is printed in full. This is exactly what GNU `wc -lw` prints for standard input.

If reading the input fails, the program exits with code 1. If writing the output fails, the
program exits with code 2.

# Behaviors
## B1: Counts lines and words
- Given the standard input `hello world`, a newline, `foo bar baz` and a newline
- When the program runs
- Then it prints `      2       5` followed by a newline
- And it exits with code `0`

## B2: Treats any whitespace as a word break
- Given the standard input `a`, a tab, `b`, a carriage return, a newline, `c`, a vertical tab, `d`, a form feed, `e`, two spaces and `f`
- When the program runs
- Then it prints `      1       6` followed by a newline
- And it exits with code `0`

## B3: Counts every other byte as part of a word
- Given the standard input `café`, a byte of value 1, a space and `déjà`
- When the program runs
- Then it prints `      0       2` followed by a newline
- And it exits with code `0`

## B4: Prints zeros for empty input
- Given empty standard input
- When the program runs
- Then it prints `      0       0` followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: read, write, exit
