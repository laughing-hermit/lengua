---
type: Attested Computation
title: empty
description: Does nothing and exits with code 0, so that a benchmark can measure the start and the end of a program.
runtime: webassembly
computation: main.wat
parameters: []
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
This program does no work. It starts, and then it exits with code 0. It reads nothing and prints
nothing. A benchmark uses it to measure the time that a program takes to start and to end, which
no program can make shorter.

# Behaviors
## B1: Exits with code 0
- Given no arguments
- When the program runs
- Then it prints nothing
- And it exits with code `0`

# Constraints
- Uses: nothing
