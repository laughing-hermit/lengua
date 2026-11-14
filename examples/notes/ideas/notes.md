---
type: lengua Module
description: "The module keeps the notes in the file `data/notes.txt`, one note on a line, and answers the routes under `/api/`."
module: notes
source: src/notes.rs
uses: [ideas/accounts, ideas/summary]
used_by: [idea, ideas/accounts, ideas/summary, ideas/ui/home, ideas/ui/login, ideas/ui/note, ideas/ui/write]
label: checked
idea_sha256: 3f5bddf294d1f7259507c8c0a822d246be2f264b6288a0bfa292e5f40151696d
source_sha256: 9ce5879a6258013dc4795624d8fa1baf387715a4807b2eb5a9e4915f1aaf31ee
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T12:52:52Z }
verified:
  - { by: human:owner, at: 2026-11-14T12:00:00Z }
  - { by: process:lengua-check, at: 2026-11-14T12:52:52Z }
title: notes
status: stable
---

# Purpose

The module keeps the notes in the file `data/notes.txt`, one note on a line, and answers the
routes under `/api/`. It checks a new note, and it gives the routes of the accounts and of the
summary to the modules `accounts` and `summary`.

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

## Give a kept note

Given the file of the notes with a note of the number 3,
When a client sends `GET /api/notes/3`,
Then the answer is 200 with that note.

### Example: the note of the number 3

- File `data/notes.txt`: "1\tMilk\tBuy two bottles.\n2\tCall\tCall the shop at noon.\n3\tTea\tBuy green tea.\n"
- Request: `GET /api/notes/3`
- Expect status: 200
- Expect body: "{\"id\":3,\"title\":\"Tea\",\"text\":\"Buy green tea.\"}"

## Add a note after the kept notes

Given the file of the notes with a note of the number 7,
When a client sends a note with `POST /api/notes`,
Then the new note gets the number 8, and the server writes its line with no tab in it.

### Example: the next number

- File `data/notes.txt`: "7\tTea\tBuy green tea.\n"
- Request: `POST /api/notes`
- Request body: "{\"title\":\"Rice\",\"text\":\"Two\tbags.\"}"
- Expect status: 201
- Expect body: "{\"id\":8,\"title\":\"Rice\",\"text\":\"Two bags.\"}"
- Expect file `data/notes.txt`: "7\tTea\tBuy green tea.\n8\tRice\tTwo bags.\n"

## Refuse a summary of a note that is not there

Given no note with the number 7,
When a client sends `POST /api/ui` with the number 7,
Then the answer is 404 with an error.

### Example: no note

- Request: `POST /api/ui`
- Request body: "{\"id\":7}"
- Expect status: 404
- Expect body: "{\"error\":\"No note has this number.\"}"

# Constraints

- Uses: `accounts`, `summary`

# Structure

- `route`: gives the status and the JSON for a method and a path under `/api/`.
- `load`: gives the notes of the file, or the two first notes when there is no file.
- `note_line`: reads one line of the file of the notes.
- `save`: writes the notes to the file, one note on a line.
- `all`: gives every note as a JSON list.
- `one`: gives the note with a number, or an error.
- `clean`: gives a text with a space in place of each tab and line end.
- `add`: keeps a new note with its new number, or gives an error for a bad title.
- `note_json`: writes one note as JSON.
- `quoted`: writes a text as a JSON string.
- `field`: finds the value of a text field in a JSON object.
- `number`: finds the value of a number field in a JSON object.
- `error`: writes an error as JSON.
- `find`: finds the note with a number.

# Links

* Uses: [accounts](accounts.md), [summary](summary.md).
* Used by: [notes](../idea.md), [accounts](accounts.md), [summary](summary.md), [ui/home](ui/home.md), [ui/login](ui/login.md), [ui/note](ui/note.md), [ui/write](ui/write.md).
* Serves: [`GET /api/notes/{id}`](../api.md), [`POST /api/notes`](../api.md), [`POST /api/ui`](../api.md).
* Called by: [ui/note](ui/note.md), [ui/write](ui/write.md).
* Index: [notes](../index.md).
