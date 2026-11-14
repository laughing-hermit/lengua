---
type: lengua Program
description: "The program waits for one client on a TCP port and answers it."
module: main
source: src/main.rs
uses: []
used_by: []
label: checked
idea_sha256: dc33d9eacf4ef8ac20589cfaafbfd750d7f9a78259b8608fa3b8b7c38259a8b9
source_sha256: 77678618da86c5b558cf95993acfe42473727eeebb547890cf73b97282375c12
cargo_sha256: e790a7e1a62a33f2a9a4089abe91bcfde72390dc3b6c81944dc7272ee61f8b09
world_sha256: c6f394502f55f9393f5efe11d08200f37d69d3dfdb32354fbbe65fa901a6b07f
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T15:22:53Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T15:22:53Z }
title: upper
status: draft
---

# Purpose

The program waits for one client on a TCP port and answers it. It reads the port from its first
argument. It reads the text that the client sends, and it sends the text back in upper case.
The idea grants the port 7803 only, so the host refuses any other port. When the host refuses
the port, the program writes `refused` and stops with the exit code 1.

# Behaviours

## Answer one client in upper case

Given a client that connects to the port 7803 and sends `hello`,
When the program listens on the port 7803,
Then the client gets the answer `HELLO`.

### Example: one client

- Client `7803`: "hello\n"
- Arguments: "7803"
- Expect answer: "HELLO\n"
- Expect exit code: 0

## Refuse a port that the idea does not grant

Given the port 7804, which the idea does not grant,
When the program tries to listen on it,
Then the host refuses the port, and the program writes `refused`.

### Example: a port with no grant

- Arguments: "7804"
- Expect standard output: "refused\n"
- Expect exit code: 1

# Constraints

- Reads: arguments
- Listens on: TCP port 7803
- Memory: 16 MiB
- Time: 5 s

# Structure

- `main`: reads the port, answers one client on it, or writes `refused`.
- `answer_one`: listens on a port, reads the text of one client and sends it back in upper
  case, or gives nothing when the host refuses.

# Links

* Index: [upper](index.md).
