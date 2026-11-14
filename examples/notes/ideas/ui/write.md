# Purpose

The screen is a form to write a new note, with a title and a text. It checks the title before
it sends the note to the server.

# Behaviours

## Save a good note

Given the title `Tea` and the text `Buy green tea.`,
When the user clicks `Save`,
Then the screen sends the note and shows that it is saved.

### Example: a good note

- Type into `Title`: "Tea"
- Type into `Text`: "Buy green tea."
- Click: `Save`
- Expect text: "Saved note 3."
- Expect state `saved`: true

## Refuse a note with no title

Given no title,
When the user clicks `Save`,
Then the screen shows an error and sends nothing.

### Example: no title

- Type into `Text`: "Buy green tea."
- Click: `Save`
- Expect text: "A note has a title."
- Expect state `saved`: false

# Constraints

- Uses: `notes`

# Structure

- `WriteScreen`: shows the form, the error and the result.
- `titleError`: gives the error of a title, or an empty text when the title is good.
- `save`: sends a note with a good title to the server, with the result in the state.
