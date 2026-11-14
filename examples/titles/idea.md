# Purpose

The library checks the title of a note. The web application `notes` uses it on its server and
in the browser, so the two give the same answer for each title.

# Behaviours

## Accept a good title

Given the title `Tea`,
When a program calls `title_error`,
Then the value is an empty text.

### Example: a short title

- Call: `title_error("Tea")`
- Expect value: `""`

### Example: a title of 40 characters

- Call: `title_error("abcdefghijabcdefghijabcdefghijabcdefghij")`
- Expect value: `""`

## Refuse an empty title

Given a title of spaces,
When a program calls `title_error`,
Then the value is the error for no title.

### Example: spaces

- Call: `title_error("   ")`
- Expect value: `"A note has a title."`

## Refuse a long title

Given a title of 41 characters,
When a program calls `title_error`,
Then the value is the error for a long title.

### Example: a title of 41 characters

- Call: `title_error("abcdefghijabcdefghijabcdefghijabcdefghijk")`
- Expect value: `"A title has 40 characters or fewer."`

# Constraints

- Memory: 16 MiB
- Time: 5 s
- Exports: `title_error`

# Structure

- `title_error`: gives the error of a title, or an empty text when the title is good.
- `length`: gives the number of characters of a title with no spaces at its ends.
