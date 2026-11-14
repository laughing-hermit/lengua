---
type: lengua Program
description: "The program counts the words in a text."
module: main
source: src/main.rs
uses: []
used_by: []
label: tested
idea_sha256: 68a7bd92c70d162a0d669ada394268f49178cc0137c512e0d1a59949d8906d95
source_sha256: 08db60f2d9b371b1c75d55e6986cfa343507be161898a9750bd9b743a09260d4
cargo_sha256: 033e6e73781015b43c9ac641996ecad684928a9b128e67f84559a2615a46e26f
world_sha256: 0a7313a3dd0aa5c41e90b244f039886104d47ce35544350dbfa738da23ba0d2c
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T06:10:32Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T06:10:32Z }
title: count
status: draft
---

# Purpose

The program counts the words in a text. It reads the text from the standard input. It
writes the number of words and a line end to the standard output.

# Behaviours

## Count the words in a text

Given a text with two words on the standard input,
When the program runs,
Then it writes 2 and a line end to the standard output.

### Example: two words

- Standard input: "hello world\n"
- Expect standard output: "2\n"
- Expect exit code: 0

## Count no words in an empty text

Given an empty standard input,
When the program runs,
Then it writes 0 and a line end to the standard output.

### Example: empty input

- Standard input: ""
- Expect standard output: "0\n"
- Expect exit code: 0

# Constraints

- Reads: standard input
- Memory: 16 MiB
- Time: 2 s

# Structure

- `main`: reads the standard input and writes the number of words.

# Links

* Index: [count](index.md).
