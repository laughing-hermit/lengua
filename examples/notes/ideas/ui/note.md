---
type: lengua Screen
description: "The screen shows one note, and a summary of it that the AI part of the screen draws from the answer of the server."
module: ui/note
source: web/screens/note.tsx
uses: [ideas/notes]
used_by: [idea]
label: checked
idea_sha256: cbe457f2066349c8b7d4d2d9adec60a049be5cff5d5c0bb1ce50be58805776e5
source_sha256: c3f0cf72fd1116f29c691c3fd083d5fa9991ec613a1525a8024c61b0a2569f65
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T07:56:57Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T07:56:57Z }
title: ui/note
status: draft
---

# Purpose

The screen shows one note, and a summary of it that the AI part of the screen draws from the
answer of the server.

# Behaviours

## Show a note

Given the note with the number 1,
When the screen opens,
Then it shows the title and the text of the note.

### Example: the first note

- State `id`: "1"
- Expect text: "Buy two bottles."
- Expect state `title`: "Milk"
- Expect state `explained`: false

## Show an error for a note that is not there

Given no note with the number 9,
When the screen opens,
Then it shows the error of the server.

### Example: no note

- State `id`: "9"
- Expect text: "No note has this number."

## Show the summary

Given the note with the number 1,
When the user clicks `Explain`,
Then the screen draws the card of the summary that the server describes.

### Example: the summary

- State `id`: "1"
- Click: `Explain`
- Expect text: "Milk: Buy two bottles."
- Expect state `explained`: true

# Constraints

- Uses: `notes`

# Structure

- `NoteScreen`: shows a note, its error and the card of its summary.
- `loadNote`: puts the note from the server, or its error, in the state.
- `explain`: puts the summary of the note from the server in the state.
- `toSpec`: changes the answer of the server into the description that the renderer draws.

# Links

* Uses: [notes](../notes.md).
* Used by: [notes](../../idea.md).
* Calls: [`GET /api/notes/{id}`](../../api.md), [`POST /api/ui`](../../api.md).
* Index: [notes](../../index.md).
