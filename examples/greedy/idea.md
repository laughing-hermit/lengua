# Purpose

The program keeps blocks of memory without end. The memory limit of its idea stops it, and the
host gives the state `out_of_memory`.

# Behaviours

## Stop at the memory limit

Given no input,
When the program keeps more than 4 MiB of memory,
Then the host stops it.

### Example: no input

- Expect exit code: 1

# Constraints

- Memory: 4 MiB
- Time: 2 s

# Structure

- `main`: keeps blocks of 1 MiB of memory without end.
