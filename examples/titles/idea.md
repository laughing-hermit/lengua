---
type: lengua Library
description: "The library checks the title of a note."
module: main
source: src/main.rs
uses: []
used_by: []
label: checked
idea_sha256: 81e4c2ab80c7e97f9d62310e96bef6fb92824b3b6bee39a8050bfaf1186cdb60
source_sha256: bf46bd6d8d0716650ab08285b0b411b7ad2f3eb35619167abbc2910274b13ffd
cargo_sha256: 64b35f6754bf56f03749cb8689473e0c62a524a274bda2aae0f2723d0b9a2e34
world_sha256: 43e5e032ac9bee5f73ecde94bf9f71093003bcc169f5f7d6e9e2310be5b0b626
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T07:48:21Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T07:48:21Z }
title: titles
status: draft
---

# Purpose

The library checks the title of a note. The web application `notes` uses it on its server and
in the browser, so the two give the same answer for each title.

# Behaviours

## Accept a good title

Given the title `Tea`,
When a program calls `title_error`,
Then the value is an empty text.

### Example: a short title

- Call: `title_error("Tea")`
- Expect value: `""`

### Example: a title of 40 characters

- Call: `title_error("abcdefghijabcdefghijabcdefghijabcdefghij")`
- Expect value: `""`

## Refuse an empty title

Given a title of spaces,
When a program calls `title_error`,
Then the value is the error for no title.

### Example: spaces

- Call: `title_error("   ")`
- Expect value: `"A note has a title."`

## Refuse a long title

Given a title of 41 characters,
When a program calls `title_error`,
Then the value is the error for a long title.

### Example: a title of 41 characters

- Call: `title_error("abcdefghijabcdefghijabcdefghijabcdefghijk")`
- Expect value: `"A title has 40 characters or fewer."`

# Constraints

- Memory: 16 MiB
- Time: 5 s
- Exports: `title_error`

# Structure

- `title_error`: gives the error of a title, or an empty text when the title is good.
- `length`: gives the number of characters of a title with no spaces at its ends.

# Links

* Index: [titles](index.md).
