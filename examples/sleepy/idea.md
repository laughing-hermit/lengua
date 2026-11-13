# Purpose

The program waits for 30 seconds in a call to the host, and then it writes `awake`. The time of
its idea is 1 second, so the host stops the program before it writes the word.

# Behaviours

## Stop a program that waits in the host

Given no input,
When the program waits in the host for longer than the time of its idea,
Then the host stops it at the time limit, and the standard output is empty.

### Example: no input

- Expect end: out of time

# Constraints

- Memory: 16 MiB
- Time: 1 s

# Structure

- `main`: waits for 30 seconds, and then writes `awake` and a line end.
