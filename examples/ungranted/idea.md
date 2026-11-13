# Purpose

The program writes a random number to the file `note.txt` and to the standard output. Its idea
grants no files and no random numbers, so the check refuses the program, and the host does not
start it.

# Behaviours

## Write a random number

Given no input,
When the program runs,
Then it writes a random number to the file `note.txt` and to the standard output.

### Example: no input

- Expect exit code: 0

# Constraints

- Memory: 16 MiB
- Time: 2 s

# Structure

- `main`: writes a random number to the file `note.txt` and to the standard output.
