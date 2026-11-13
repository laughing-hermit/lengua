# Purpose

The screen shows one note, and a summary of it that the AI part of the screen draws from the
answer of the server.

# Behaviours

## Show a note

Given the note with the number 1,
When the screen opens,
Then it shows the title and the text of the note.

### Example: the first note

- State `id`: "1"
- Expect text: "Buy two bottles."
- Expect state `title`: "Milk"
- Expect state `explained`: false

## Show an error for a note that is not there

Given no note with the number 9,
When the screen opens,
Then it shows the error of the server.

### Example: no note

- State `id`: "9"
- Expect text: "No note has this number."

## Show the summary

Given the note with the number 1,
When the user clicks `Explain`,
Then the screen draws the card of the summary that the server describes.

### Example: the summary

- State `id`: "1"
- Click: `Explain`
- Expect text: "Milk: Buy two bottles."
- Expect state `explained`: true

# Constraints

- Uses: `notes`

# Structure

- `NoteScreen`: shows a note, its error, and the summary that the AI part draws.
- `loadNote`: asks the server for a note and puts it, or the error, in the state.
- `explain`: asks the server for the summary of a note and puts it in the state.
- `toSpec`: changes the answer of the server into the description that the renderer draws.
