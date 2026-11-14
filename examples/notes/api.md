---
type: lengua Interface
description: "The routes of the back end that the front end calls, with the shapes of their requests and answers."
label: checked
client_sha256: c5bd5446a42ec31b6813b7741f399fa3f184fa06179980235ed7ee7afb03b096
---

# Routes

## `GET /api/notes`

* Function: `getNotes` in `web/lib/lengua.ts`.
* Request body: none.
* Answer: `{ "id": number; "title": string; "text": string }[]`.
* Failure: `unknown`.
* Served by: [notes](idea.md).
* Called by: [ui/home](ideas/ui/home.md).

## `GET /api/notes/{id}`

* Function: `getNotesById` in `web/lib/lengua.ts`.
* Request body: none.
* Answer: `{ "id": number; "title": string; "text": string }`.
* Failure: `{ "error": string }`.
* Served by: [notes](idea.md).
* Called by: [ui/note](ideas/ui/note.md).

## `POST /api/notes`

* Function: `postNotes` in `web/lib/lengua.ts`.
* Request body: `{ "title": string; "text": string }`.
* Answer: `{ "id": number; "title": string; "text": string }`.
* Failure: `{ "error": string }`.
* Served by: [notes](idea.md), [notes](ideas/notes.md).
* Called by: [ui/write](ideas/ui/write.md).

## `POST /api/ui`

* Function: `postUi` in `web/lib/lengua.ts`.
* Request body: `{ "id": number }`.
* Answer: `{ "root": string; "elements": { "card": { "type": string; "props": { "title": string }; "children": string[] }; "text": { "type": string; "props": { "content": string }; "children": unknown[] } } }`.
* Failure: `{ "error": string }`.
* Served by: [notes](idea.md), [notes](ideas/notes.md).
* Called by: [ui/note](ideas/ui/note.md).

# Links

* Index: [index](index.md).
