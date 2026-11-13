---
type: lengua Program
description: "The program writes the time and a random number to the standard output."
module: main
source: src/main.rs
uses: []
used_by: []
label: tested
idea_sha256: b8f02ba5d9b46ff8de968eeb9e7dfc6ffb6a2b7c3b854acebc02f0d8e211c81b
source_sha256: 877c938a3b33ab2d737b222a10d53f3e8748a1662aac0ae1a72e041c94207430
cargo_sha256: af6899f6f1f111b5641d009e92a339633faf9bbc9f354d52c2cddadced3d0a37
world_sha256: 1dcb1f39bae916e91c7370da7a68bfdf772df1c0ecd52d570128a0a7cead5ab1
generated: { by: lengua-mcp/0.1.0, at: 2026-11-13T23:01:23Z }
verified:
  - { by: process:lengua-check, at: 2026-11-13T23:01:23Z }
title: clock
status: draft
---

# Purpose

The program writes the time and a random number to the standard output. The time is the
number of seconds after the start of 1970 on the clock of the host. With the fixed clock and
the fixed random numbers of the tests, the program writes the same two numbers on each run.

# Behaviours

## Write the time and a random number

Given the fixed clock and the fixed random numbers of the tests,
When the program runs,
Then it writes 1767225600, a line end, a random number and a line end to the standard output.

### Example: the fixed clock

- Expect standard output: "1767225600\n1859844318930836693\n"
- Expect exit code: 0

# Constraints

- Reads: the clock
- Reads: random numbers
- Memory: 16 MiB
- Time: 2 s

# Structure

- `main`: writes the seconds of the clock and a random number to the standard output.

# Links

* Index: [clock](index.md).
