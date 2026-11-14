---
type: lengua Module
description: "The module keeps the notes and answers the routes under `/api/`."
module: notes
source: src/notes.rs
uses: []
used_by: [idea, ideas/ui/home, ideas/ui/note, ideas/ui/write]
label: checked
idea_sha256: 55de08672f7dab83fbe81d04c5497ddba7334127006594673e7d7c1fc29dbaee
source_sha256: e7b7d86325163115c1a9b0b82a55296ae14299178ff3c5069fe711fa7de96b04
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T04:38:07Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T04:38:07Z }
title: notes
status: draft
---

# Purpose

The module keeps the notes and answers the routes under `/api/`. It checks a new note and
makes the summary of a note.

# Behaviours

## Refuse a title that is too long

Given a note with a title of more than 40 characters,
When a client sends it with `POST /api/notes`,
Then the answer is 400 with an error.

### Example: a long title

- Request: `POST /api/notes`
- Request body: "{\"title\":\"This title has many more words than a title has.\",\"text\":\"x\"}"
- Expect status: 400
- Expect body: "{\"error\":\"A title has 40 characters or fewer.\"}"

## Refuse a summary of a note that is not there

Given no note with the number 7,
When a client sends `POST /api/ui` with the number 7,
Then the answer is 404 with an error.

### Example: no note

- Request: `POST /api/ui`
- Request body: "{\"id\":7}"
- Expect status: 404
- Expect body: "{\"error\":\"No note has this number.\"}"

# Structure

- `route`: gives the status and the JSON for a method and a path under `/api/`.
- `all`: gives every note as a JSON list.
- `one`: gives the note with a number, or an error.
- `add`: gives a new note with its new number, or an error for a bad title.
- `summary`: gives the description of a card with the summary of a note.
- `note_json`: writes one note as JSON.
- `quoted`: writes a text as a JSON string.
- `field`: finds the value of a text field in a JSON object.
- `number`: finds the value of a number field in a JSON object.
- `error`: writes an error as JSON.
- `find`: finds the note with a number.

# Links

* Used by: [notes](../idea.md), [ui/home](ui/home.md), [ui/note](ui/note.md), [ui/write](ui/write.md).
* Serves: [`POST /api/notes`](../api.md), [`POST /api/ui`](../api.md).
* Called by: [ui/note](ui/note.md), [ui/write](ui/write.md).
* Index: [notes](../index.md).
