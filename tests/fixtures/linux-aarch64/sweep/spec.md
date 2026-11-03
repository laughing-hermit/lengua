---
type: lengua/program-spec
title: sweep
description: Deletes the empty files in the working folder, and says how many.
---
# Purpose
Deletes every empty file directly in the working folder, keeping each file that holds anything,
from `1` byte to `100` and more, and every folder, then prints how many files it deleted, `0` to
`16`, to tidy a folder after a script. If listing, checking or deleting fails, it exits with
code 3.

# Behaviors
## B1: Deletes the empty files
- Given an empty file `a.txt`
- And a file `b.txt` holding `keep`
- When the program runs
- Then it prints `deleted 1` followed by a newline
- And there is no file `a.txt`
- And it exits with code `0`

### For any `text`
- Given an empty file `a.txt`
- And a file `b.txt` holding `{text}`
- When the program runs
- Then it prints `deleted 1` followed by a newline
- And there is no file `a.txt`
- And it exits with code `0`

| text |
|---|
| `string(1, 100, "ekp")` |

## B2: Keeps a folder with nothing empty
- Given a file `b.txt` holding `keep`
- And an empty folder `old`
- When the program runs
- Then it prints `deleted 0` followed by a newline
- And it exits with code `0`

### For any `text`
- Given a file `b.txt` holding `{text}`
- And an empty folder `old`
- When the program runs
- Then it prints `deleted 0` followed by a newline
- And it exits with code `0`

| text |
|---|
| `string(1, 100, "ekp")` |

# Constraints
- Uses: files
