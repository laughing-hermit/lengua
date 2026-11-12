---
type: Attested Computation
title: reach
description: Asks for a connection to a computer that its idea does not grant and prints the answer.
runtime: webassembly
computation: main.wasm
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
A test uses this program to run a module that a C compiler made. The program asks for a connection
to TCP port 443 of example.org over TLS. Its idea grants TCP port 443 of example.com only.

If lengua refuses the connection, the program prints `refused` and a newline. If lengua opens the
connection, the program prints `opened` and a newline.

# Behaviors
## B1: Prints that lengua refused the connection
- Given no arguments
- When the program runs
- Then it prints `refused` and a newline
- And it exits with code `0`

# Constraints
- Uses: nothing
- Network: opens TCP connections to port 443 on example.com over TLS
- Module: main.wasm, built from C (tests/outside/reach/main.c) with clang of wasi-sdk for wasm32-wasip1
