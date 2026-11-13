# Purpose

The program counts to the largest 64-bit number, so it does not stop in the time of its idea.
The time limit of its idea stops it, and the host gives the state `out_of_time`.

# Behaviours

## Stop at the time limit

Given no input,
When the program runs for longer than the time of its idea,
Then the host stops it.

### Example: no input

- Expect exit code: 1

# Constraints

- Memory: 16 MiB
- Time: 200 ms

# Structure

- `main`: counts to the largest 64-bit number.
