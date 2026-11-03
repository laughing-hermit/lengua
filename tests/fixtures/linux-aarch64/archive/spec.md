---
type: lengua/program-spec
title: archive
description: Moves a file into the archive folder, making the folder when it's missing.
---
# Purpose
Moves the file its one argument names, `1` to `64` characters with no `/`, into the folder
`archive` in the working folder, making `archive` when it isn't there, to tidy a folder of notes.
When the file isn't there it makes nothing.
A file of `0` to `5000` bytes keeps what it holds. Without exactly one argument, or with a name it
can't take, it prints how to use it and exits with code 1. When the file isn't there it exits with
code 2, and when moving fails otherwise with code 3.

# Behaviors
## B1: Moves the file into the archive
- Given the argument `notes.txt`
- And a file `notes.txt` holding `call Ada`
- When the program runs
- Then it prints `archived notes.txt` followed by a newline
- And the file `archive/notes.txt` holds `call Ada`
- And there is no file `notes.txt`
- And it exits with code `0`

### For any `text`
- Given the argument `notes.txt`
- And a file `notes.txt` holding `{text}`
- When the program runs
- Then it prints `archived notes.txt` followed by a newline
- And the file `archive/notes.txt` holds `{text}`
- And there is no file `notes.txt`
- And it exits with code `0`

| text |
|---|
| `string(0, 5000, "Aacdl \n")` |

## B2: Uses the archive that's already there
- Given the argument `todo.txt`
- And an empty folder `archive`
- And a file `todo.txt` holding `milk`
- When the program runs
- Then it prints `archived todo.txt` followed by a newline
- And the file `archive/todo.txt` holds `milk`
- And there is no file `todo.txt`
- And it exits with code `0`

### For any `text`
- Given the argument `todo.txt`
- And an empty folder `archive`
- And a file `todo.txt` holding `{text}`
- When the program runs
- Then it prints `archived todo.txt` followed by a newline
- And the file `archive/todo.txt` holds `{text}`
- And there is no file `todo.txt`
- And it exits with code `0`

| text |
|---|
| `string(0, 5000, "iklm \n")` |

## B3: Says when the file isn't there
- Given the argument `gone`
- When the program runs
- Then it prints `archive: no file gone` followed by a newline to standard error
- And there is no folder `archive`
- And it exits with code `2`

### For any `name`
- Given the argument `{name}`
- When the program runs
- Then it prints `archive: no file {name}` followed by a newline to standard error
- And there is no folder `archive`
- And it exits with code `2`

| name |
|---|
| `string(1, 64, "egno")` |

## B4: Shows usage without one plain name
- Given the argument `a/b`
- When the program runs
- Then it prints `usage: archive <file>` followed by a newline to standard error
- And it exits with code `1`

### For any `first` and `second`
- Given the argument `{first}/{second}`
- When the program runs
- Then it prints `usage: archive <file>` followed by a newline to standard error
- And it exits with code `1`

| first | second |
|---|---|
| `string(1, 64, "ab")` | `string(1, 64, "ab")` |

# Constraints
- Uses: files
