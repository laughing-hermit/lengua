# Purpose

The program is a small web application for notes. Its back end is an HTTP server in Rust that
gives the notes as JSON, adds a note when its title is good, and makes a summary of a note for
the AI part of the screen. Its front end has three screens, `home`, `note` and `write`, in
TypeScript with `Next.js`. The server carries the front end and serves it, so the application is
one WebAssembly file.

The summary is a fixed answer of the server until the grant for an AI model is in `lengua`. The
notes are fixed too until the grant for storage is in `lengua`, so a new note is checked and
given back, but it is not kept.

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
Then the answer is 201 with the note and its new number.

### Example: a good note

- Request: `POST /api/notes`
- Request body: "{\"title\":\"Tea\",\"text\":\"Buy green tea.\"}"
- Expect status: 201
- Expect body: "{\"id\":3,\"title\":\"Tea\",\"text\":\"Buy green tea.\"}"

### Example: a note with no title

- Request: `POST /api/notes`
- Request body: "{\"title\":\"\",\"text\":\"Buy green tea.\"}"
- Expect status: 400
- Expect body: "{\"error\":\"A note has a title.\"}"

## Give a summary for the screen

Given the note with the number 1,
When a client sends `POST /api/ui` with the number,
Then the answer is a description of a card with the summary, for the AI part of the screen.

### Example: the summary of the first note

- Request: `POST /api/ui`
- Request body: "{\"id\":1}"
- Expect status: 200
- Expect body: "{\"root\":\"card\",\"elements\":{\"card\":{\"type\":\"Card\",\"props\":{\"title\":\"Summary\"},\"children\":[\"text\"]},\"text\":{\"type\":\"Text\",\"props\":{\"content\":\"Milk: Buy two bottles.\"},\"children\":[]}}}"

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
- Memory: 64 MiB
- Time: 5 s
- Modules: `notes`, `ui/home`, `ui/note`, `ui/write`
- Uses crate: `wasip2` 2.0.1, for the interface of the server to its host, the HTTP world of `WASI`
- Route: `GET /api/notes`
- Route: `GET /api/notes/{id}`
- Route: `POST /api/notes`
- Route: `POST /api/ui`
- Uses shadcn: `button`, `card`, `input`, `label`, `textarea`
- Uses package: `@json-render/core` 0.22.0, for the catalog of the parts that an AI model can put on the screen
- Uses package: `@json-render/react` 0.22.0, for the parts of the screen that an AI model describes
- Uses package: `zod` 4.6.7, for the shapes of the parts in the catalog

# Structure

- `Notes::handle`: answers a request with the status, the type and the body that `answer` gives.
- `answer`: gives the answer of the notes for a path under `/api/`, and a file of the front end for any other path.
- `method_name`: gives the name of the method of a request.
- `body_of`: reads the body of a request as text.
- `respond`: writes the status, the type and the body of the answer.
