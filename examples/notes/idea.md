---
type: lengua Server
description: "The program is a small web application for notes."
module: main
source: src/main.rs
uses: [ideas/notes, ideas/accounts, ideas/summary, ideas/ui/home, ideas/ui/login, ideas/ui/note, ideas/ui/write, ../titles/idea, ../../libraries/logins/idea]
used_by: []
label: checked
idea_sha256: 21d14e833a6203724731c682f621b90519a9f4eb96f4f6cf5526118c9a262df8
source_sha256: 6749f59b9dcf23e799e9387e2ac6d03160ff9489971cf347d1872a40f04f858d
cargo_sha256: a71e617ef16a3533e8b7ccc811be90a821436cd6b0c75e52e9bc9f8971760d96
world_sha256: 432ab60b895088c9bc0206a29ba4e132e6b86715dbabb9ceab8b2b6adf271f30
web_sha256: 3c10a01c59ede2c82f1f315ee5d9a354e90bfc8d7bae13c065edeeefdcfb5a44
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T12:52:52Z }
verified:
  - { by: human:owner, at: 2026-11-14T12:00:00Z }
  - { by: process:lengua-check, at: 2026-11-14T12:52:52Z }
title: notes
status: stable
approved: { sha256: 7048ff4a72c232f317ac7cd746074574215570a3768d84901ed643f3e88fd768, by: human:owner, at: 2026-11-14T12:00:00Z }
---

# Purpose

The program is a small web application for notes. Its back end is an HTTP server in Rust that
keeps the notes in the folder `data` and gives them as JSON. The server adds a note when the
library `titles` accepts its title. It keeps the accounts of its users with the library
`logins`, and it asks an AI model for the summary of a note. The front end has four screens in
TypeScript with `Next.js`: `home`, `login`, `note` and `write`. The server carries the front end
and serves it, so the application is one WebAssembly file.

The screen `write` checks a title in the browser with the same library, so the browser and the
server give the same answer. The module `notes` keeps the notes, the module `accounts` keeps the
accounts, and the module `summary` asks the model. Before the first note is added, the server
gives two notes.

# Behaviours

## List the notes

Given the two notes of the server,
When a client sends `GET /api/notes`,
Then the answer is 200 with the two notes as JSON.

### Example: the notes

- Request: `GET /api/notes`
- Expect status: 200
- Expect header `content-type`: "application/json"
- Expect body: "[{\"id\":1,\"title\":\"Milk\",\"text\":\"Buy two bottles.\"},{\"id\":2,\"title\":\"Call\",\"text\":\"Call the shop at noon.\"}]"

## Give one note

Given the note with the number 1,
When a client sends `GET /api/notes/1`,
Then the answer is 200 with the note as JSON.

### Example: the first note

- Request: `GET /api/notes/1`
- Expect status: 200
- Expect body: "{\"id\":1,\"title\":\"Milk\",\"text\":\"Buy two bottles.\"}"

### Example: a note that is not there

- Request: `GET /api/notes/9`
- Expect status: 404
- Expect body: "{\"error\":\"No note has this number.\"}"

## Add a note

Given a note with a title and a text,
When a client sends it with `POST /api/notes`,
Then the answer is 201 with the note and its new number, and the server keeps it.

### Example: a good note

- Request: `POST /api/notes`
- Request body: "{\"title\":\"Tea\",\"text\":\"Buy green tea.\"}"
- Expect status: 201
- Expect body: "{\"id\":3,\"title\":\"Tea\",\"text\":\"Buy green tea.\"}"
- Expect file `data/notes.txt`: "1\tMilk\tBuy two bottles.\n2\tCall\tCall the shop at noon.\n3\tTea\tBuy green tea.\n"

### Example: a note with no title

- Request: `POST /api/notes`
- Request body: "{\"title\":\"\",\"text\":\"Buy green tea.\"}"
- Expect status: 400
- Expect body: "{\"error\":\"A note has a title.\"}"

## Give a summary for the screen

Given the note with the number 1 and the answer of the model,
When a client sends `POST /api/ui` with the number,
Then the answer is a description of a card with the summary, for the AI part of the screen.

### Example: the summary of the first note

- Request: `POST /api/ui`
- Request body: "{\"id\":1}"
- Model answers: " Milk: Buy two bottles.\n"
- Expect status: 200
- Expect body: "{\"root\":\"card\",\"elements\":{\"card\":{\"type\":\"Card\",\"props\":{\"title\":\"Summary\"},\"children\":[\"text\"]},\"text\":{\"type\":\"Text\",\"props\":{\"content\":\"Milk: Buy two bottles.\"},\"children\":[]}}}"

### Example: no answer of the model

- Request: `POST /api/ui`
- Request body: "{\"id\":2}"
- Expect status: 503
- Expect body: "{\"error\":\"The summary is not ready.\"}"

## Answer 404 for a route that is not there

Given a path under `/api/` that is not a route of the server,
When a client sends a request for it,
Then the answer is 404 with an error in JSON.

### Example: another path

- Request: `GET /api/other`
- Expect status: 404
- Expect body: "{\"error\":\"No such route.\"}"

### Example: another method

- Request: `DELETE /api/notes`
- Expect status: 404
- Expect body: "{\"error\":\"No such route.\"}"

## Serve the front end

Given the static files of the front end in the program,
When a client sends `GET /`,
Then the answer is 200 with the page of the screen `home`.

### Example: the first page

- Request: `GET /`
- Expect status: 200
- Expect header `content-type`: "text/html; charset=utf-8"

### Example: the page of a note

- Request: `GET /note/?id=1`
- Expect status: 200
- Expect header `content-type`: "text/html; charset=utf-8"

### Example: a path that is not there

- Request: `GET /nothing`
- Expect status: 404
- Expect body: "not found"

# Constraints

- Serves: HTTP on TCP port 7812
- Reads: random numbers
- Reads: the clock
- Reads: the secret `SESSION_KEY`
- Writes: files in `data`
- Asks: an AI model, for the summary of a note, up to 1 calls and 400 tokens
- Memory: 64 MiB
- Time: 5 s
- Modules: `notes`, `accounts`, `summary`, `ui/home`, `ui/login`, `ui/note`, `ui/write`
- Uses crate: `wasip2` 2.0.1, for the interface of the server to its host, the HTTP world of `WASI`
- Uses: `../titles`, `../../libraries/logins`
- Route: `GET /api/notes`
- Route: `GET /api/notes/{id}`
- Route: `POST /api/notes`
- Route: `POST /api/ui`
- Route: `POST /api/register`
- Route: `POST /api/login`
- Route: `GET /api/me`
- Uses shadcn: `button`, `card`, `input`, `label`, `textarea`
- Uses package: `@json-render/core` 0.22.0, for the catalog of the parts that an AI model can put on the screen
- Uses package: `@json-render/react` 0.22.0, for the parts of the screen that an AI model describes
- Uses package: `zod` 4.6.7, for the shapes of the parts in the catalog

# Structure

- `Notes::handle`: answers a request with what `answer` gives.
- `answer`: answers a path under `/api/` with the notes, and any other path with a file of the front end.
- `method_name`: gives the name of the method of a request.
- `body_of`: reads the body of a request as text.
- `respond`: writes the answer to the client.
- `header`: gives the first value of a header of a request, or no text.

# Links

* Uses: [notes](ideas/notes.md), [accounts](ideas/accounts.md), [summary](ideas/summary.md), [ui/home](ideas/ui/home.md), [ui/login](ideas/ui/login.md), [ui/note](ideas/ui/note.md), [ui/write](ideas/ui/write.md), [titles](../titles/idea.md), [logins](../../libraries/logins/idea.md).
* Serves: [`GET /api/notes`](api.md), [`GET /api/notes/{id}`](api.md), [`POST /api/notes`](api.md), [`POST /api/ui`](api.md).
* Called by: [ui/home](ideas/ui/home.md), [ui/note](ideas/ui/note.md), [ui/write](ideas/ui/write.md).
* Index: [notes](index.md).
