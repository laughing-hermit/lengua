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
