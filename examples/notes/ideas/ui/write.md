---
type: lengua Screen
description: "The screen is a form to write a new note, with a title and a text."
module: ui/write
source: web/screens/write.tsx
uses: [ideas/notes]
used_by: [idea]
label: checked
idea_sha256: 0e79f3cc48ed008412f8cc01cce6d329956e9f16537ebbac63b59560e8792013
source_sha256: f248fc2792ba530e7b2ee1772fc7870122c421ac9c8e97cee42243c4e8f22751
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T15:31:10Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T15:31:10Z }
title: ui/write
status: draft
---

# Purpose

The screen is a form to write a new note, with a title and a text. It checks the title in the
browser with the library `titles` before it sends the note. The server runs the same code.

# Behaviours

## Save a good note

Given the title `Tea` and the text `Buy green tea.`,
When the user clicks `Save`,
Then the screen sends the note and shows that it is saved.

### Example: a good note

- Type into `Title`: "Tea"
- Type into `Text`: "Buy green tea."
- Click: `Save`
- Expect text: "Saved note 3."
- Expect state `saved`: true

## Refuse a note with no title

Given no title,
When the user clicks `Save`,
Then the screen shows an error and sends nothing.

### Example: no title

- Type into `Text`: "Buy green tea."
- Click: `Save`
- Expect text: "A note has a title."
- Expect state `saved`: false

## Refuse a long title in the browser

Given a title of 41 characters,
When the user clicks `Save`,
Then the screen shows the error of the library and sends nothing.

### Example: a long title

- Type into `Title`: "abcdefghijabcdefghijabcdefghijabcdefghijk"
- Click: `Save`
- Expect text: "A title has 40 characters or fewer."
- Expect state `saved`: false

# Constraints

- Uses: `notes`

# Structure

- `WriteScreen`: shows the form, the error and the result.
- `save`: sends a note with a good title to the server, with the result in the state.

# Links

* Uses: [notes](../notes.md).
* Used by: [notes](../../idea.md).
* Calls: [`POST /api/notes`](../../api.md).
* Index: [notes](../../index.md).
