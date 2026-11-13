# Purpose

The screen shows the number of words in the text.

# Behaviours

## Show the count

Given a text with two words,
When the user clicks the count button,
Then the screen shows the number 2.

### Example: two words

- State `text`: "a b"
- Click: `count`
- Expect text: "2 words"
- Expect state `count`: 2

# Constraints

- Uses: `words`

# Structure

- `show_count`: shows the number of words.
