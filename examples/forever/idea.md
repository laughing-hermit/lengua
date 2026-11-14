---
type: lengua Program
description: "The program counts to the largest 64 - bit number, so it does not stop in the time of its idea."
module: main
source: src/main.rs
uses: []
used_by: []
label: tested
idea_sha256: 35f052e24b702818e5f02f2f31c7692bdbfe466d72f0517dd8dd78ef71827537
source_sha256: e31b5bf06e927b7cab213d51f5f126790abcc2ff6c99ae28d4cf25c007627617
cargo_sha256: 2afa06e92e6bda7e6918a5167c2582912dc58229b8ecfe79dc8c6310f5e96266
world_sha256: 0a7313a3dd0aa5c41e90b244f039886104d47ce35544350dbfa738da23ba0d2c
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T04:34:07Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T04:34:07Z }
title: forever
status: draft
---

# Purpose

The program counts to the largest 64-bit number, so it does not stop in the time of its idea.
The time limit of its idea stops it, and the host gives the state `out_of_time`.

# Behaviours

## Stop at the time limit

Given no input,
When the program runs for longer than the time of its idea,
Then the host stops it.

### Example: no input

- Expect end: out of time

# Constraints

- Memory: 16 MiB
- Time: 200 ms

# Structure

- `main`: counts to the largest 64-bit number.

# Links

* Index: [forever](index.md).
