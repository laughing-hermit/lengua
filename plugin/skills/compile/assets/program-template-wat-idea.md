---
type: Attested Computation
title: computer
description: Prints the name of the computer and the name of its user, which two programs of the computer give.
runtime: webassembly
computation: main.wat
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose

A shell script can use this program to learn where it runs. The program starts two programs of
the computer, `hostname` and then `whoami`, each with no arguments. It prints the output of each
program as it is: the name of the computer, and then the name of its user.

The program starts only these two programs, and it gives them no arguments and no input. Each
program has 10 seconds, and its output has up to `256` bytes.

If the computer does not have one of the programs, the program prints its name and `: not found`
in the place of its output. If one of the programs fails, the program prints its name and
`: failed`. Then it continues with the next program.

If the program cannot write its output, it exits with code 4.

# Behaviors

## B1: Prints the output of a program and says when the computer does not have a program
- Given the program `hostname` prints `box\n` and exits with code `0`
- When the program runs
- Then it prints `box\nwhoami: not found\n`
- And it exits with code `0`

## B2: Exits with code 4 when it cannot write its output
- Given the program `hostname` prints `box\n` and exits with code `0`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

# Constraints

- Runs: `hostname` with no arguments
- Runs: `whoami` with no arguments
