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

- `route`: gives the status and the JSON of the answer for a method and a path under `/api/`.
- `all`: gives every note as a JSON list.
- `one`: gives the note with a number, or an error.
- `add`: checks a new note and gives it with its new number, or an error.
- `summary`: gives the description of a card with the summary of a note.
- `note_json`: writes one note as JSON.
- `quoted`: writes a text as a JSON string.
- `field`: finds the value of a text field in a JSON object.
- `number`: finds the value of a number field in a JSON object.
- `error`: writes an error as JSON.
- `find`: finds the note with a number.
