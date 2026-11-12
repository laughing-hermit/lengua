---
type: Attested Computation
title: count
description: Prints the number of bytes in the file notes.txt of the current folder.
runtime: webassembly
computation: main.wasm
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
A test uses this program to run a module that a Rust compiler made. The program reads the file
`notes.txt` in the current folder. Then it prints the number of bytes in the file and a newline.

The file has up to `4096` bytes. If the program cannot read the file, it prints `no notes.txt` and
a newline to standard error. Then it exits with code 3.

# Behaviors
## B1: Prints the number of bytes in the file
- Given a file `notes.txt` that contains `hello`
- When the program runs
- Then it prints `5` and a newline
- And it exits with code `0`

### For any `text`
- Given a file `notes.txt` that contains `{text}`
- When the program runs
- Then it prints `{len(text)}` and a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 4096, "ehlo abc\n")` | 20 |

## B2: Says that the file is not there
- Given no arguments
- When the program runs
- Then it prints `no notes.txt` and a newline to standard error
- And it exits with code `3`

# Constraints
- Uses: files
- Module: main.wasm, built from Rust (tests/outside/count/main.rs) with rustc for wasm32-wasip1
