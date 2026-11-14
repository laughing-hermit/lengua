---
type: lengua Program
description: "The program shows the text of files."
module: main
source: src/main.rs
uses: []
used_by: []
label: checked
idea_sha256: 7c82c772c7ac7710ca211f5dd854639d078038c3606d84db0784ee5cfda8eae2
source_sha256: ab9840bfc067a7fabaf52d7e2dd5d1ee08ded2741956c7264263ba7545a860d7
cargo_sha256: d919a99ed857d895887deb51378de268f355145ba0821ba5f89966e73711b0f7
world_sha256: cbdc5c096384a41f695cc82d201f1ef28c2d1270d259ddd0014340d04e0b359e
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T12:47:05Z }
verified:
  - { by: human:owner, at: 2026-11-14T12:00:00Z }
  - { by: process:lengua-check, at: 2026-11-14T12:47:05Z }
title: files
status: stable
approved: { sha256: 32f5d3fcb57e206be5ae7f9c92da104389c1961856193ad06963ff2a6435520f, by: human:owner, at: 2026-11-14T12:00:00Z }
---

# Purpose

The program shows the text of files. Each argument is the path of a file. For each path, it
writes the text of the file to the standard output. When it cannot read the file, it writes
`cannot read`, the path and a line end.

The program can read files in the folder `data`, and it can write files in the folder `out`.
It tries to write the file `data/new.txt`. When it cannot, it writes `write in data: no` and a
line end. Then it writes the number of files that it read to the file `out/count.txt`.

# Behaviours

## Show a file in the data folder

Given the file `data/note.txt` with the text `a note`,
When the program runs with the argument `data/note.txt`,
Then it writes the text of the file, `write in data: no` and a line end to the standard output.
And it writes 1 and a line end to the file `out/count.txt`.

### Example: a note

- Arguments: "data/note.txt"
- File `data/note.txt`: "a note\n"
- Expect standard output: "a note\nwrite in data: no\n"
- Expect file `out/count.txt`: "1\n"
- Expect exit code: 0

## Show no file outside the folders

Given the argument `../idea.md`,
When the program runs,
Then it writes `cannot read ../idea.md`, a line end, `write in data: no` and a line end to the
standard output.

### Example: a path up

- Arguments: "../idea.md"
- Expect standard output: "cannot read ../idea.md\nwrite in data: no\n"
- Expect file `out/count.txt`: "0\n"
- Expect exit code: 0

# Constraints

- Reads: arguments
- Reads: files in `data`
- Writes: files in `out`
- Memory: 16 MiB
- Time: 2 s

# Structure

- `main`: writes the text of each file, tries to write in `data`, and writes the count to `out`.

# Links

* Index: [files](index.md).
