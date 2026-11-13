# Purpose

The program keeps many blocks of memory, more than the memory of its idea. The memory limit of
its idea stops it, and the host gives the state `out_of_memory`.

# Behaviours

## Stop at the memory limit

Given no input,
When the program keeps more than 4 MiB of memory,
Then the host stops it.

### Example: no input

- Expect end: out of memory

# Constraints

- Memory: 4 MiB
- Time: 2 s

# Structure

- `main`: keeps many blocks of 1 MiB of memory.
