# Purpose

The library reads and writes JSON text, as RFC 8259 gives it, with a value type of its own. A
number keeps its text, so no digit is lost; the `decimal` library reads it as a number. The two
modules use no recursion: a list in a list is a frame on a stack, up to 512 frames.
It is a `lengua` library on the standard library only, in two modules, `read` and `write`.

# Behaviours

## Find a field of an object

Given an object with the field `a`,
When a program calls `get` with the name `a`,
Then it gives the value of the field.

### Example: a field

- Call: `get(&parse("{\"a\": 1, \"b\": true}").unwrap(), "b")`
- Expect value: `Some(Bool(true))`

### Example: a name that is not there

- Call: `get(&parse("{\"a\": 1}").unwrap(), "c")`
- Expect value: `None`

### Example: a list has no fields

- Call: `get(&parse("[1]").unwrap(), "a")`
- Expect value: `None`

# Constraints

- Memory: 32 MiB
- Time: 5 s
- Modules: `read`, `write`
- Exports: `Value`, `Error`, `parse`, `write`, `get`

# Structure

- `get`: gives the value of a field of an object, or nothing.
