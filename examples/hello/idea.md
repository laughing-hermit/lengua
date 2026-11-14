---
type: lengua Program
description: "The program writes a short message to the standard output."
module: main
source: src/main.rs
uses: []
used_by: []
label: checked
idea_sha256: 9ef82a0425640e18e79a43ca1ff23d71e05fe6c1d77af21b1616d8658c85f055
source_sha256: c8e0583694bb1e0188dbe28fe0d65ac1130723c55f968b6262b906c147f72549
cargo_sha256: ab5db4d335a66fd2179a373285a93b022444cfabdacd4928b0365a06bc6f015f
world_sha256: 0a7313a3dd0aa5c41e90b244f039886104d47ce35544350dbfa738da23ba0d2c
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T10:06:51Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T10:06:51Z }
title: hello
status: draft
---

# Purpose

The program writes a short message to the standard output. The message is `Hello, world!`
and a line end.

# Behaviours

## Write the message

Given no standard input and no arguments,
When the program runs,
Then it writes `Hello, world!` and a line end to the standard output.

### Example: no input

- Expect standard output: "Hello, world!\n"
- Expect exit code: 0

# Constraints

- Memory: 16 MiB
- Time: 2 s

# Structure

- `main`: writes the message and a line end to the standard output.

# Links

* Index: [hello](index.md).
