---
type: lengua Library
description: "The library reads and writes JSON text, as RFC 8259 gives it, with a value type of its own."
module: main
source: src/main.rs
uses: [ideas/read, ideas/write]
used_by: []
label: checked
idea_sha256: 6b21f83390618f5c3257843ed5c5f188f47ab3fe17dbfe427fb408fd0a681491
source_sha256: 85fd1f1c20905b9ed5895291775b46eab3b1fd3636befdca1bc28c744ad6a637
cargo_sha256: 5ef3ef8a7df65842f2a9e661ae55ad8cb2109c861f1fa9a565aab76a3786573f
world_sha256: 43e5e032ac9bee5f73ecde94bf9f71093003bcc169f5f7d6e9e2310be5b0b626
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T04:30:39Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T04:30:39Z }
title: json
status: draft
---

# Purpose

The library reads and writes JSON text, as RFC 8259 gives it, with a value type of its own. A
number keeps its text, so no digit is lost; the `decimal` library reads it as a number. The two
modules use no recursion: a list in a list is a frame on a stack, up to 512 frames.
It is a `lengua` library on the standard library only, in two modules, `read` and `write`.

# Behaviours

## Find a field of an object

Given an object with the field `a`,
When a program calls `get` with the name `a`,
Then it gives the value of the field.

### Example: a field

- Call: `get(&parse("{\"a\": 1, \"b\": true}").unwrap(), "b")`
- Expect value: `Some(Bool(true))`

### Example: a name that is not there

- Call: `get(&parse("{\"a\": 1}").unwrap(), "c")`
- Expect value: `None`

### Example: a list has no fields

- Call: `get(&parse("[1]").unwrap(), "a")`
- Expect value: `None`

# Constraints

- Memory: 32 MiB
- Time: 5 s
- Modules: `read`, `write`
- Exports: `Value`, `Error`, `parse`, `write`, `get`

# Structure

- `get`: gives the value of a field of an object, or nothing.

# Links

* Uses: [read](ideas/read.md), [write](ideas/write.md).
* Index: [json](index.md).
