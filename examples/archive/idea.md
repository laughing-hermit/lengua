---
type: lengua/program-idea
title: archive
description: Moves a file into the archive folder and makes the folder if it does not exist.
---
# Purpose
The one argument of the program is the name of a file, `1` to `64` characters with no `/`. The
program moves this file into the folder `archive` in the current folder, to keep a folder of notes
tidy. A file of `0` to `5000` bytes keeps its contents.

If the folder `archive` does not exist, the program makes it. If the file does not exist, the
program makes nothing.

Without exactly one argument, or with a name that it cannot accept, the program prints how to use
it. Then it exits with code 1. If the file does not exist, the program exits with code 2. If the
move fails for a different reason, the program exits with code 3.

# Behaviors
## B1: Moves the file into the archive
- Given the argument `notes.txt`
- And a file `notes.txt` that contains `call Ada`
- When the program runs
- Then it prints `archived notes.txt` and a newline
- And the file `archive/notes.txt` contains `call Ada`
- And there is no file `notes.txt`
- And it exits with code `0`

### For any `text`
- Given the argument `notes.txt`
- And a file `notes.txt` that contains `{text}`
- When the program runs
- Then it prints `archived notes.txt` and a newline
- And the file `archive/notes.txt` contains `{text}`
- And there is no file `notes.txt`
- And it exits with code `0`

| text |
|---|
| `string(0, 5000, "Aacdl \n")` |

## B2: Uses an archive folder that already exists
- Given the argument `todo.txt`
- And an empty folder `archive`
- And a file `todo.txt` that contains `milk`
- When the program runs
- Then it prints `archived todo.txt` and a newline
- And the file `archive/todo.txt` contains `milk`
- And there is no file `todo.txt`
- And it exits with code `0`

### For any `text`
- Given the argument `todo.txt`
- And an empty folder `archive`
- And a file `todo.txt` that contains `{text}`
- When the program runs
- Then it prints `archived todo.txt` and a newline
- And the file `archive/todo.txt` contains `{text}`
- And there is no file `todo.txt`
- And it exits with code `0`

| text |
|---|
| `string(0, 5000, "iklm \n")` |

## B3: Says when the file does not exist
- Given the argument `gone`
- When the program runs
- Then it prints `archive: no file gone` and a newline to standard error
- And there is no folder `archive`
- And it exits with code `2`

### For any `name`
- Given the argument `{name}`
- When the program runs
- Then it prints `archive: no file {name}` and a newline to standard error
- And there is no folder `archive`
- And it exits with code `2`

| name |
|---|
| `string(1, 64, "egno")` |

## B4: Shows the usage line without one plain name
- Given the argument `a/b`
- When the program runs
- Then it prints `usage: archive <file>` and a newline to standard error
- And it exits with code `1`

### For any `first` and `second`
- Given the argument `{first}/{second}`
- When the program runs
- Then it prints `usage: archive <file>` and a newline to standard error
- And it exits with code `1`

| first | second |
|---|---|
| `string(1, 64, "ab")` | `string(1, 64, "ab")` |

# Constraints
- Uses: files
