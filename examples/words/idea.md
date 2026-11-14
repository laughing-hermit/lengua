---
type: lengua Program
description: "The program counts the different words in a text."
module: main
source: src/main.rs
uses: []
used_by: []
label: checked
idea_sha256: 4f972326f63078e1219959f9da1589fad50eb4705013ec938075947ef1952ce4
source_sha256: 3d8b6ede53e56abe4b10076078ca396f5e39044f501794609f160f3043cf0471
cargo_sha256: 42a131df898ebdc9ed8b1cb1b86377c1e4efbe6ca38a3614f265df69249c485a
world_sha256: 0a7313a3dd0aa5c41e90b244f039886104d47ce35544350dbfa738da23ba0d2c
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T10:07:32Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T10:07:32Z }
title: words
status: draft
approved: { sha256: f4e6a38d22b1ecff13c3b0881c722ce588778573d16086e6c729a4c2d8320238, by: human:owner, at: 2026-11-14T12:00:00Z }
---

# Purpose

The program counts the different words in a text. It reads the text from the standard input.
It writes the number of different words and a line end to the standard output. It keeps the
words in a `HashMap`, and its idea does not grant random numbers. The host gives the `HashMap`
a fixed value for its keys, because the program does not have random numbers.

# Behaviours

## Count the different words in a text

Given the text `a b a` on the standard input,
When the program runs,
Then it writes 2 and a line end to the standard output.

### Example: a word two times

- Standard input: "a b a\n"
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

- `main`: reads the standard input and writes the number of different words.

# Links

* Index: [words](index.md).
