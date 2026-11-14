---
type: lengua Program
description: "The program keeps many blocks of memory, more than the memory of its idea."
module: main
source: src/main.rs
uses: []
used_by: []
label: tested
idea_sha256: 4eb8c3deb062213142c6d0889f86ffb10f66f1854be18f49c85aab6a632c272e
source_sha256: 7c6f4e0945857852286e1af6c1c451cdd1931188a2693a7399c8f324670499c1
cargo_sha256: b49c671100765110d894dae7a4f6de7666fc4c5405230839ce0560d0164bbad3
world_sha256: 0a7313a3dd0aa5c41e90b244f039886104d47ce35544350dbfa738da23ba0d2c
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T13:27:58Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T13:27:58Z }
title: greedy
status: draft
---

# Purpose

The program keeps many blocks of memory, more than the memory of its idea. The memory limit of
its idea stops it, and the host gives the state `out_of_memory`.

# Behaviours

## Stop at the memory limit

Given no input,
When the program keeps more than 4 MiB of memory,
Then the host stops it.

### Example: no input

- Expect end: out of memory

# Constraints

- Memory: 4 MiB
- Time: 2 s

# Structure

- `main`: keeps many blocks of 1 MiB of memory.

# Links

* Index: [greedy](index.md).
