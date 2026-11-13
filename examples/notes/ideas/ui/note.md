# Purpose

The screen shows one note: its title and its text. A click on the button `Back` tells the
list to open again.

# Behaviours

## Show a note

Given a note with the title `Milk`,
When the screen shows it,
Then its text is on the screen.

### Example: the note `Milk`

- State `title`: "Milk"
- State `text`: "Buy two bottles."
- Expect state `left`: false
- Expect text: "Buy two bottles."

## Go back

Given a note,
When the user clicks the button `Back`,
Then the screen tells the list to open.

### Example: back

- Click: `back`
- Expect state `left`: true

# Structure

- `Note::make`: makes the screen with no note.
- `Note::show`: shows a note with its title and its text.
- `Note::back`: marks that the user left, and tells the list.
- `Note::render`: shows the title, the text and the button `Back`.
