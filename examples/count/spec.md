---
type: lengua/program-spec
title: count
description: Counts the lines and words of standard input and prints both, like wc -lw.
---
# Purpose

A fast replacement for `wc -lw` on very large files: it reads standard input of any length, a piece
at a time, and prints how many lines and how many words it holds. A line is counted for each
newline byte. A word is a run of one or more bytes that are not whitespace, where whitespace is the
space, tab, newline, vertical tab, form feed and carriage return bytes; every other byte, including
bytes of UTF-8 characters, belongs to a word. It prints the line count, one space, the word count
and a newline, both as whole numbers in decimal. It takes no arguments. The tests give it standard
input of up to `1048576` bytes; the program streams inputs of any size beyond that. If reading
standard input fails, the program exits with code 2. If writing the output fails, the program
exits with code 3.

# Behaviors

## B1: Counts lines and words
- Given standard input holding the line `hello world` followed by a newline and the line `foo` followed by a newline
- When the program runs
- Then it prints `2 3` followed by a newline to standard output
- And it exits with code `0`

## B2: Any run of whitespace separates words
- Given standard input holding `a`, two tabs, `b`, three spaces, a carriage return and `c`, with no newline
- When the program runs
- Then it prints `0 3` followed by a newline to standard output
- And it exits with code `0`

## B3: Counts empty input as nothing
- Given empty standard input
- When the program runs
- Then it prints `0 0` followed by a newline to standard output
- And it exits with code `0`

## B4: Shows usage when given arguments
- Given the argument `file.txt`
- When the program runs
- Then it prints `usage: count < input` followed by a newline to standard error
- And it exits with code `1`

# Constraints

- Allowed system calls: read, write, exit
