---
type: lengua Interface
description: "The routes of the back end that the front end calls, with the shapes of their requests and answers."
label: checked
client_sha256: 3f76456a011eb35135d9b90d15e2f99ac0f16c5f9d4ab3d7f3927327c3ea45ad
---

# Routes

## `GET /api/notes`

* Function: `getNotes` in `web/lib/lengua.ts`.
* Request body: none.
* Answer: `{ "id": number; "title": string; "text": string }[]`.
* Failure: `unknown`.
* Statuses: 200.
* Served by: [notes](idea.md).
* Called by: [ui/home](ideas/ui/home.md).

## `GET /api/notes/{id}`

* Function: `getNotesById` in `web/lib/lengua.ts`.
* Request body: none.
* Answer: `{ "id": number; "title": string; "text": string }`.
* Failure: `{ "error": string }`.
* Statuses: 200, 404.
* Served by: [notes](idea.md), [notes](ideas/notes.md).
* Called by: [ui/note](ideas/ui/note.md).

## `POST /api/notes`

* Function: `postNotes` in `web/lib/lengua.ts`.
* Request body: `{ "title": string; "text": string }`.
* Answer: `{ "id": number; "title": string; "text": string }`.
* Failure: `{ "error": string }`.
* Statuses: 201, 400.
* Served by: [notes](idea.md), [notes](ideas/notes.md).
* Called by: [ui/write](ideas/ui/write.md).

## `POST /api/ui`

* Function: `postUi` in `web/lib/lengua.ts`.
* Request body: `{ "id"?: number }`.
* Answer: `{ "root": string; "elements": { "card": { "type": string; "props": { "title": string }; "children": string[] }; "text": { "type": string; "props": { "content": string }; "children": unknown[] } } }`.
* Failure: `{ "error": string }`.
* Statuses: 200, 404, 503.
* Served by: [notes](idea.md), [notes](ideas/notes.md), [summary](ideas/summary.md).
* Called by: [ui/note](ideas/ui/note.md).

## `POST /api/register`

* Function: `postRegister` in `web/lib/lengua.ts`.
* Request body: `{ "name": string; "password": string }`.
* Answer: `{ "name": string }`.
* Failure: `{ "error": string }`.
* Statuses: 201, 400, 409.
* Served by: [accounts](ideas/accounts.md).
* Called by: no screen.

## `POST /api/login`

* Function: `postLogin` in `web/lib/lengua.ts`.
* Request body: `{ "name": string; "password": string }`.
* Answer: `{ "token": string }`.
* Failure: `{ "error": string }`.
* Statuses: 200, 401.
* Served by: [accounts](ideas/accounts.md).
* Called by: [ui/login](ideas/ui/login.md).

## `GET /api/me`

* Function: `getMe` in `web/lib/lengua.ts`.
* Request body: none.
* Answer: `{ "name": string }`.
* Failure: `{ "error": string }`.
* Statuses: 200, 401.
* Served by: [accounts](ideas/accounts.md).
* Called by: [ui/login](ideas/ui/login.md).

# Links

* Index: [index](index.md).
