# Purpose

The module asks an AI model for the summary of a note, through the host, and describes a card
with the summary for the screen `note`. The front end draws the card with `json-render`. The
host holds the key of the model, so the browser never gets the key.

# Behaviours

## Refuse a summary of no note

Given a request with no number,
When a client sends `POST /api/ui`,
Then the answer is 404 with an error, and the server does not ask the model.

### Example: no number

- Request: `POST /api/ui`
- Request body: "{}"
- Expect status: 404
- Expect body: "{\"error\":\"No note has this number.\"}"

## Give an empty summary

Given an empty answer of the model,
When a client sends `POST /api/ui` with a number,
Then the answer is a card with no text.

### Example: an empty answer

- Request: `POST /api/ui`
- Request body: "{\"id\": 1}"
- Model answers: ""
- Expect status: 200
- Expect body: "{\"root\":\"card\",\"elements\":{\"card\":{\"type\":\"Card\",\"props\":{\"title\":\"Summary\"},\"children\":[\"text\"]},\"text\":{\"type\":\"Text\",\"props\":{\"content\":\"\"},\"children\":[]}}}"

# Constraints

- Uses: `notes`

# Structure

- `summary`: asks the model for the summary of a note, and gives the card or an error.
- `spec`: describes a card with the summary, in the JSON of `json-render`.
- `ask`: sends a prompt to the model through the host, and gives its answer.
