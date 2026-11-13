# Purpose

The screen is a form for a new note, with a title and a text. It checks the title before it
sends the note to the server. A title must have one character or more, and 40 characters at
most. The screen shows the error of a bad title.

# Behaviours

## Refuse an empty title

Given an empty form,
When the user clicks the button `Save`,
Then the form shows an error and does not send the note.

### Example: no title

- Click: `save`
- Expect state `error`: "The title is empty."
- Expect state `sent`: false
- Expect text: "The title is empty."

## Refuse a title that is too long

Given a title of 41 to 60 characters,
When the user clicks the button `Save`,
Then the form shows an error and does not send the note.

### Cases: long titles

- Range `t`: text of 41 to 60 characters from "ab"
- Type into `title`: "{t}"
- Click: `save`
- Expect state `error`: "The title is too long."
- Expect state `sent`: false

## Send a good note

Given a title of 1 to 40 characters and a text,
When the user clicks the button `Save`,
Then the form sends the note and shows no error.

### Cases: good titles

- Range `t`: text of 1 to 40 characters from "ab"
- Type into `title`: "{t}"
- Type into `text`: "Green tea."
- Click: `save`
- Expect state `sent`: true
- Expect state `error`: ""
- Expect state `title`: "{t}"

## Cancel the form

Given a form with a title,
When the user clicks the button `Cancel`,
Then the form tells the list to open and does not send the note.

### Example: cancel

- Type into `title`: "Tea"
- Click: `cancel`
- Expect state `left`: true
- Expect state `sent`: false

# Structure

- `check`: gives the error of a title, or nothing for a good title.
- `Edit::make`: makes the form with two empty text fields that keep the title and the text.
- `Edit::save`: checks the title, and sends the note to the server when the title is good.
- `Edit::cancel`: marks that the user left, and tells the list.
- `Edit::render`: shows the fields, the error and the buttons `Save` and `Cancel`.
