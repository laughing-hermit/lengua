---
type: lengua Server
description: "The program is an HTTP server."
module: main
source: src/main.rs
uses: []
used_by: []
label: checked
idea_sha256: 9a517252c8d1cdbe8ac71397406306bc095f51effb4816df196d09074176ce98
source_sha256: a9c40126a927642ba422a94ecf445b455885054af29900adae682a1436f04903
cargo_sha256: 687df52b60c4767198dfb82695c2c921a86b2ededdcaa6da70a33c9fdebf1ce8
world_sha256: 6aa2f03d29a6dae284b24b3801913f715788aad0e8f74739ae1fde5efab7af5d
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T04:34:22Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T04:34:22Z }
title: greet
status: draft
---

# Purpose

The program is an HTTP server. It answers `GET /hello?name=<name>` with the text
`Hello, <name>!`, and any other path with the status 404. The host serves it on the TCP port
7810, and the program gets no other grant.

# Behaviours

## Answer a request with a name

Given a request for `/hello?name=Ada`,
When the server answers,
Then the status is 200 and the body is `Hello, Ada!`.

### Example: the name `Ada`

- Request: `GET /hello?name=Ada`
- Expect status: 200
- Expect header `content-type`: "text/plain"
- Expect body: "Hello, Ada!"

## Answer 404 for an unknown path

Given a request for `/other`,
When the server answers,
Then the status is 404 and the body is `not found`.

### Example: another path

- Request: `GET /other`
- Expect status: 404
- Expect body: "not found"

## Answer 404 for an empty name

Given a request for `/hello?name=` with no name,
When the server answers,
Then the status is 404.

### Example: no name

- Request: `GET /hello?name=`
- Expect status: 404
- Expect body: "not found"

# Constraints

- Serves: HTTP on TCP port 7810
- Memory: 32 MiB
- Time: 5 s
- Uses crate: `wasip2` 2.0.1, for the interface of the server to its host, the HTTP world of `WASI`

# Structure

- `Greet::handle`: answers a request with the status and the body that `answer` gives.
- `answer`: gives the status and the body for the path of a request.

# Links

* Index: [greet](index.md).
