---
type: lengua Program
description: "The program waits for 30 seconds in a call to the host, and then it writes `awake`."
module: main
source: src/main.rs
uses: []
used_by: []
label: tested
idea_sha256: 31dc4f05bfe0015d00134d877996229503c8761f8eea0444c6a1c82cd99559ac
source_sha256: 039de563daabc89903b33f74dffea24bf15e09126f26f83e5f8ff3aa5fbe75d2
cargo_sha256: b5bf902e42a9b019b85231565a000d6b2b46993712ae89e3592ed736f39e83c1
world_sha256: 0a7313a3dd0aa5c41e90b244f039886104d47ce35544350dbfa738da23ba0d2c
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T00:35:04Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T00:35:04Z }
title: sleepy
status: draft
---

# Purpose

The program waits for 30 seconds in a call to the host, and then it writes `awake`. The time of
its idea is 1 second, so the host stops the program before it writes the word.

# Behaviours

## Stop a program that waits in the host

Given no input,
When the program waits in the host for longer than the time of its idea,
Then the host stops it at the time limit, and the standard output is empty.

### Example: no input

- Expect end: out of time

# Constraints

- Memory: 16 MiB
- Time: 1 s

# Structure

- `main`: waits for 30 seconds, and then writes `awake` and a line end.

# Links

* Index: [sleepy](index.md).
