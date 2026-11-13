# Purpose

The module keeps the notes of the server and makes a new note from a request. A title must
have one character or more, and 40 characters at most. The answers and the errors are JSON.

# Behaviours

## Make a new note

Given a request with the title `Tea` and a text,
When the screens send `POST /api/notes`,
Then the answer is 201 with the new note and its number.

### Example: a good note

- Request: `POST /api/notes`
- Request body: "{\"title\":\"Tea\",\"text\":\"Green tea.\"}"
- Expect status: 201
- Expect body: "{\"id\":\"3\",\"title\":\"Tea\",\"text\":\"Green tea.\"}"

## Refuse a note with no title

Given a request with an empty title,
When the screens send `POST /api/notes`,
Then the answer is 422 with the error.

### Example: an empty title

- Request: `POST /api/notes`
- Request body: "{\"title\":\" \",\"text\":\"x\"}"
- Expect status: 422
- Expect body: "{\"error\":\"The title is empty.\"}"

## Refuse a title that is too long

Given a request with a title of 41 to 60 characters,
When the screens send `POST /api/notes`,
Then the answer is 422 with the error.

### Cases: long titles

- Range `t`: text of 41 to 60 characters from "ab"
- Request: `POST /api/notes`
- Request body: "{{\"title\":\"{t}\",\"text\":\"x\"}}"
- Expect status: 422
- Expect body: "{{\"error\":\"The title is too long.\"}}"

## Accept a title of 40 characters at most

Given a request with a title of 1 to 40 characters,
When the screens send `POST /api/notes`,
Then the answer is 201.

### Cases: short titles

- Range `t`: text of 1 to 40 characters from "ab"
- Request: `POST /api/notes`
- Request body: "{{\"title\":\"{t}\",\"text\":\"\"}}"
- Expect status: 201

## Refuse a body that is not JSON

Given a request body that is not JSON,
When the screens send `POST /api/notes`,
Then the answer is 400 with the error.

### Example: not JSON

- Request: `POST /api/notes`
- Request body: "title"
- Expect status: 400
- Expect body: "{\"error\":\"The body is not JSON.\"}"

# Structure

- `listing`: gives the two notes of the server as a JSON list.
- `route`: gives the status and the JSON body of an `api` path.
- `create`: makes a new note from the body of a request, or gives an error.
- `check`: gives the error of a title, or nothing for a good title.
- `text_of`: gives the text of a field of a JSON object, or an empty text.
- `error`: gives a status and a JSON object with the error.
