---
type: lengua Screen
description: "The screen shows the list of the notes, with a link to each note and a link to write a new note."
module: ui/home
source: web/screens/home.tsx
uses: [ideas/notes]
used_by: [idea]
label: checked
idea_sha256: de5feaa51145148e9a2a2d2e06c0c60bc054d214a10b6f271a67bf365fa6b776
source_sha256: dde75252c4a1f81a078b4397b8ee30247ef8536f74cf22e6eb875e916b65f870
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T15:31:10Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T15:31:10Z }
title: ui/home
status: draft
---

# Purpose

The screen shows the list of the notes, with a link to each note and a link to write a new
note.

# Behaviours

## Show the notes

Given the two notes of the server,
When the screen opens,
Then it shows the title of each note and a link to write a note.

### Example: two notes

- Expect text: "Call"
- Expect label: `Write a note`

## Show the number of the notes

Given the two notes of the server,
When the screen opens,
Then it shows the text `2 notes`.

### Example: the count

- Expect text: "2 notes"

# Constraints

- Uses: `notes`

# Structure

- `HomeScreen`: shows the notes, their number and a link to the screen `write`.
- `NoteLink`: shows the title of one note as a link to its screen.
- `countText`: gives the text for a number of notes.

# Links

* Uses: [notes](../notes.md).
* Used by: [notes](../../idea.md).
* Calls: [`GET /api/notes`](../../api.md).
* Index: [notes](../../index.md).
