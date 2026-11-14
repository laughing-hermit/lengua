---
type: lengua Program
description: "The program sends `ping` to a server and writes the answer of the server."
module: main
source: src/main.rs
uses: []
used_by: []
label: checked
idea_sha256: 5039a50bf4e163fc3e0bb17c96bb7a34ebfa8ed4cdbf0691ea76ca22c512d462
source_sha256: f4011392e20621e3e5599a4b12608184a42c2647437626dde3db8429e1dedf5c
cargo_sha256: 21b8172c4970fc685e9dc87199e7ce13bcb1234ca105b37a49889d1dc97425ee
world_sha256: c6f394502f55f9393f5efe11d08200f37d69d3dfdb32354fbbe65fa901a6b07f
generated: { by: lengua-mcp/0.1.0, at: 2026-11-13T23:01:40Z }
verified:
  - { by: process:lengua-check, at: 2026-11-13T23:01:40Z }
title: fetch
status: draft
---

# Purpose

The program sends `ping` to a server and writes the answer of the server. It reads the address
of the server from its first argument. The idea grants one address only, `127.0.0.1:7801`, so
the host refuses a connection to any other address. When the connection fails, the program
writes `refused` and stops with the exit code 1.

# Behaviours

## Write the answer of a granted server

Given a server at `127.0.0.1:7801` that answers `pong`,
When the program connects to `127.0.0.1:7801`,
Then it writes the answer `pong` and a line end.

### Example: the granted server

- Server `127.0.0.1:7801`: "pong\n"
- Arguments: "127.0.0.1:7801"
- Expect standard output: "pong\n"
- Expect exit code: 0

## Refuse a server that the idea does not grant

Given a server at `127.0.0.1:7802` that answers `secret`,
When the program connects to `127.0.0.1:7802`,
Then the host refuses the connection, and the program writes `refused`.

### Example: an address with no grant

- Server `127.0.0.1:7802`: "secret\n"
- Arguments: "127.0.0.1:7802"
- Expect standard output: "refused\n"
- Expect exit code: 1

# Constraints

- Reads: arguments
- Connects to: `127.0.0.1:7801`
- Memory: 16 MiB
- Time: 5 s

# Structure

- `main`: reads the address, sends `ping` to it and writes the answer, or `refused`.
- `fetch`: connects to an address, sends `ping` and gives the answer, or nothing when the
  connection fails.

# Links

* Index: [fetch](index.md).
