# Purpose

The screen shows the list of the notes, with a link to each note and a link to write a new
note.

# Behaviours

## Show the notes

Given the two notes of the server,
When the screen opens,
Then it shows the title of each note and a link to write a note.

### Example: two notes

- Expect text: "Call"
- Expect label: `Write a note`

## Show the number of the notes

Given the two notes of the server,
When the screen opens,
Then it shows the text `2 notes`.

### Example: the count

- Expect text: "2 notes"

# Constraints

- Uses: `notes`

# Structure

- `HomeScreen`: shows the notes, the number of notes and the link to write a note.
- `NoteLink`: shows the title of one note as a link to its screen.
- `countText`: gives the text for a number of notes.
