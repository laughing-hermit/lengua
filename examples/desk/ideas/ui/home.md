# Purpose

The screen is the window of the desktop app. It has a form for a prompt, and it shows the
letters that the model gives after the prompt.

# Behaviours

## Write the next letters

Given the prompt `ab`,
When the user clicks `Write`,
Then the screen shows the text of the model.

### Example: two letters

- Type into `Prompt`: "ab"
- Click: `Write`
- Expect text: "abfbedbfbe"
- Expect state `written`: true

## Show the form first

Given no prompt,
When the screen opens,
Then it shows the form and no text.

### Example: the form

- Expect label: `Prompt`
- Expect text: "Write with the local model"
- Expect state `written`: false

# Structure

- `HomeScreen`: shows the form, the text of the model and the error.
- `write`: sends the prompt to the server and keeps the text or the error.
