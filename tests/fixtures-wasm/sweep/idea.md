---
type: Attested Computation
title: sweep
description: Deletes the empty files in the current folder and says how many it deleted.
runtime: webassembly
computation: main.wat
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit, files]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
The program deletes each empty file that is directly in the current folder, to tidy a folder after
a script. It keeps each file that has contents, from `1` byte to `100` and more. It also keeps each
folder. Then it prints the number of files that it deleted, `0` to `16`. If it cannot list, examine
or delete the files, it exits with code 3.

# Behaviors
## B1: Deletes the empty files
- Given an empty file `a.txt`
- And a file `b.txt` that contains `keep`
- When the program runs
- Then it prints `deleted 1` and a newline
- And there is no file `a.txt`
- And it exits with code `0`

### For any `text`
- Given an empty file `a.txt`
- And a file `b.txt` that contains `{text}`
- When the program runs
- Then it prints `deleted 1` and a newline
- And there is no file `a.txt`
- And it exits with code `0`

| text |
|---|
| `string(1, 100, "ekp")` |

## B2: Keeps a folder that has no empty files
- Given a file `b.txt` that contains `keep`
- And an empty folder `old`
- When the program runs
- Then it prints `deleted 0` and a newline
- And it exits with code `0`

### For any `text`
- Given a file `b.txt` that contains `{text}`
- And an empty folder `old`
- When the program runs
- Then it prints `deleted 0` and a newline
- And it exits with code `0`

| text |
|---|
| `string(1, 100, "ekp")` |

# Constraints
- Uses: files
