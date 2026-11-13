---
type: lengua Program
description: "The program writes a random number to the file `note.txt` and to the standard output."
module: main
source: src/main.rs
uses: []
used_by: []
label: unchecked
idea_sha256: 7150cd6535e15d028df296fa782bdddbc01245ccdeae1c5b20a40790535f62d3
source_sha256: e9f327f6e87f719c28c720b9dadb77b573f422b1acf6282aac980ae714eeabcc
cargo_sha256: b4ccde3e8ed565e77c2af11d3e1dfc96a2d8f4bfe7cc6d68c5cee58dc8fd7575
world_sha256: 0a7313a3dd0aa5c41e90b244f039886104d47ce35544350dbfa738da23ba0d2c
generated: { by: lengua-mcp/0.1.0, at: 2026-11-13T23:03:20Z }
title: ungranted
status: draft
---

# Purpose

The program writes a random number to the file `note.txt` and to the standard output. Its idea
grants no files, so the check refuses the program, and the host does not start it.

# Behaviours

## Write a random number

Given no input,
When the program runs,
Then it writes a random number to the file `note.txt` and to the standard output.

### Example: no input

- Expect exit code: 0

# Constraints

- Memory: 16 MiB
- Time: 2 s

# Structure

- `main`: writes a random number to the file `note.txt` and to the standard output.

# Links

* Index: [ungranted](index.md).
