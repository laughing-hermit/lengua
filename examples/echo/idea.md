---
type: lengua Program
description: "The program writes its arguments to the standard output."
module: main
source: src/main.rs
uses: []
used_by: []
label: checked
idea_sha256: dd9a3fb62d7a33e99a23bab51ae130a51744650554979b489d4552471ac64059
source_sha256: dce1236eef08ecbfb876316b7cb984d2180b95abecd600dc4b76bec8df2d853e
cargo_sha256: d1015ddd599fc4319f42fe64d424e7e61a06d314468faf3ab04f1e844798cbfb
world_sha256: 0a7313a3dd0aa5c41e90b244f039886104d47ce35544350dbfa738da23ba0d2c
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T10:06:16Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T10:06:16Z }
title: echo
status: draft
approved: { sha256: 9e98b4e3dc21fa4d7ca5cd97104649cdc9bdaae7cdd5828d8000f7c86b42f151, by: human:owner, at: 2026-11-14T12:00:00Z }
---

# Purpose

The program writes its arguments to the standard output. It writes each argument on a line of
its own, in the order of the arguments.

# Behaviours

## Write each argument on a line

Given the arguments `one` and `two`,
When the program runs,
Then it writes `one`, a line end, `two` and a line end to the standard output.

### Example: two arguments

- Arguments: "one" "two"
- Expect standard output: "one\ntwo\n"
- Expect exit code: 0

## Write nothing with no arguments

Given no arguments,
When the program runs,
Then it writes nothing to the standard output.

### Example: no arguments

- Expect standard output: ""
- Expect exit code: 0

# Constraints

- Reads: arguments
- Memory: 16 MiB
- Time: 2 s

# Structure

- `main`: writes each argument and a line end to the standard output.

# Links

* Index: [echo](index.md).
