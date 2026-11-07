---
type: Attested Computation
title: count
description: Counts the lines and words of standard input and prints the two counts, as wc -lw does.
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

This program is a fast replacement for `wc -lw` on very large files. It reads standard input of
any length, one piece at a time. It prints the number of lines and the number of words in the
input. It takes no arguments.

The program counts one line for each newline byte. A word is a run of one or more bytes that are
not whitespace. Whitespace is the space, tab, newline, vertical tab, form feed and carriage return
bytes. Every other byte is part of a word, and this includes the bytes of UTF-8 characters.

The program prints the line count, one space, the word count and a newline. It prints each count
as a whole number in decimal.

The tests give it standard input of up to `1048576` bytes. The program also reads larger inputs,
one piece at a time. If the program cannot read standard input, it exits with code 2. If the
program cannot write its output, it exits with code 3.

# Behaviors

## B1: Counts lines and words
- Given standard input that contains `hello world` and a newline then `foo` and a newline
- When the program runs
- Then it prints `2 3` and a newline
- And it exits with code `0`

### For any `text`
The first row draws text with little whitespace, as in prose. The second row draws text with much
whitespace of each kind.

- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{count(text, "\n")} {words(text)}` and a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "abcdefghijklmnopqrstuvwxyz0123456789 \n")` | 300 |
| `string(0, 1048576, "x. \t\n\r")` | 300 |

## B2: Separates words at any run of whitespace
- Given standard input that contains `a\t\tb   \rc`
- When the program runs
- Then it prints `0 3` and a newline
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{count(text, "\n")} {words(text)}` and a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "abc \t\r")` | 300 |

## B3: Counts zero lines and zero words in empty input
- Given empty standard input
- When the program runs
- Then it prints `0 0` and a newline
- And it exits with code `0`

## B4: Shows the usage line when it gets arguments
- Given the argument `file.txt`
- When the program runs
- Then it prints `usage: count < input` and a newline to standard error
- And it exits with code `1`

### For any `a`
- Given the argument `{a}`
- When the program runs
- Then it prints `usage: count < input` and a newline to standard error
- And it exits with code `1`

| a | runs |
|---|---|
| `choice("file.txt", "-l", "-", "", "--help")` | 20 |

# Constraints

- Uses: standard input
