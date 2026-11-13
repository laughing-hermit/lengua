# Purpose

The screen is the start of the notes application in the browser. It loads the notes from the
server as JSON, and it shows the title of each note. A click on a title opens the screen of
the note. A click on the button `New note` opens the form. The screen keeps the notes, and it
adds a note that the form saved.

# Behaviours

## Show the titles of the notes

Given the JSON of two notes,
When the screen shows the list,
Then the title of each note is on the screen.

### Example: two notes

- State `data`: "[{\"id\":\"1\",\"title\":\"Milk\",\"text\":\"Buy two bottles.\"},{\"id\":\"2\",\"title\":\"Call\",\"text\":\"Call the shop.\"}]"
- Expect state `page`: "list"
- Expect text: "Call"

## Show that there are no notes

Given an empty list of notes,
When the screen shows the list,
Then it shows that there are no notes.

### Example: no notes

- State `data`: "[]"
- Expect text: "There are no notes."

## Open a note

Given the JSON of two notes,
When the user clicks the second title,
Then the screen of the second note opens with its text.

### Example: the second note

- State `data`: "[{\"id\":\"1\",\"title\":\"Milk\",\"text\":\"Buy two bottles.\"},{\"id\":\"2\",\"title\":\"Call\",\"text\":\"Call the shop.\"}]"
- Click: `note-1`
- Expect state `page`: "note"
- Expect state `chosen`: 1
- Expect text: "Call the shop."

## Open the form

Given the list,
When the user clicks the button `New note`,
Then the form opens.

### Example: a new note

- Click: `new`
- Expect state `page`: "edit"
- Expect text: "Save"

## Go back to the list

Given the screen of a note,
When the user clicks the button `Back`,
Then the list opens again.

### Example: back

- State `page`: "note"
- Click: `back`
- Expect state `page`: "list"

## Add the note that the form saved

Given an empty list and the form,
When the user types a title and clicks the button `Save`,
Then the list opens and shows the new title.

### Example: a saved note

- State `data`: "[]"
- State `page`: "edit"
- Type into `title`: "Tea"
- Click: `save`
- Expect state `page`: "list"
- Expect text: "Tea"

# Constraints

- Uses crate: `gpui-pre` 0.3.12, for `GPUI`, the user interface that the GPU draws, from the snapshot of the `Zed` commit `d2a4fb6`
- Uses crate: `gpui-pre-platform` 0.3.12, for the platform of `GPUI` in the browser that the start of the screens uses
- Uses crate: `gpui-pre-web` 0.3.12, for the web target of `GPUI`, which draws on one canvas with `WebGPU` or `WebGL2`
- Uses crate: `gpui-component` 0.9.1, for the buttons and the text fields of `GPUI Kit`
- Uses crate: `gpui-pre-http-client` 0.3.12, for the requests of the screens to their server, through the browser
- Uses crate: `futures` 0.3.34, for the read of the body of an answer
- Uses crate: `wasm-bindgen` 0.2.130, for the start of the screens in the browser and their loader

# Structure

- `start`: starts the screens in a window of the browser, with the list first.
- `follow`: makes the list follow the moves of the screen of a note or of the form.
- `field`: gives the text of a field of a note, or an empty text.
- `List::make`: makes the list with no notes, the screen of a note and the form.
- `List::load`: sends the request for the notes, and keeps them when the answer comes.
- `List::loaded`: keeps the notes of an answer, or marks that they did not load.
- `List::notes`: gives the title and the text of each note.
- `List::choose`: opens the screen of the note that the user clicked.
- `List::new_note`: opens a new form.
- `List::moved`: opens the list again, and adds the note that the form saved.
- `List::add`: adds a note with a title and a text to the notes.
- `List::rows`: makes the title of the list, a button for each note and the button `New note`.
- `List::render`: shows the list, the screen of a note or the form.
