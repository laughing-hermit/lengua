---
type: Attested Computation
title: shout-server
description: Sends each line back to its client in capitals, for up to 64 clients at once, and keeps each connection open for more lines.
runtime: webassembly
computation: main.wat
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
This program is a network service for many clients at once. A client sends lines of at most `1023`
characters and a newline. The program sends each line back with each ASCII letter `a` to `z` in
capitals, and it keeps the connection open for the next line. When the client closes its side, the
program sends back what remains and closes the connection.

It answers each client when the data of that client ends with a newline, and it does not wait for
the other clients. It sends back at most 1024 bytes at a time. It serves up to `64` clients at once
on TCP port 8080 of this computer. More clients wait for their turn.

# Behaviors
## B1: Shouts a line back
- Given a client that sends `hello` and a newline
- When the program runs
- Then the client receives `HELLO` and a newline
- And it does not stop

### For any `line`
- Given a client that sends `{line}` and a newline
- When the program runs
- Then the client receives `{upper(line)}` and a newline
- And it does not stop

| line |
|---|
| `string(0, 1023, "abcdefghijklmnopqrstuvwxyz ")` |

## B2: Shouts each line of a connection
- Given a client that sends `ab\ncd\n`
- When the program runs
- Then the client receives `AB\nCD\n`
- And it does not stop

### For any `a` and `b`
- Given a client that sends `{a}\n{b}\n`
- When the program runs
- Then the client receives `{upper(a)}\n{upper(b)}\n`
- And it does not stop

| a | b |
|---|---|
| `string(0, 64, "abcxyz AZ")` | `string(0, 64, "abcxyz AZ")` |

## B3: Answers clients at once
- Given a client that sends `first` and a newline
- And a client that sends `second` and a newline
- And the clients connect at once
- When the program runs
- Then the client receives `FIRST` and a newline
- And the client receives `SECOND` and a newline
- And it does not stop

### For any `a` and `b`
- Given a client that sends `{a}` and a newline
- And a client that sends `{b}` and a newline
- And the clients connect at once
- When the program runs
- Then the client receives `{upper(a)}` and a newline
- And the client receives `{upper(b)}` and a newline
- And it does not stop

| a | b |
|---|---|
| `string(0, 1023, "abcdefghijklmnopqrstuvwxyz ")` | `string(0, 1023, "abcdefghijklmnopqrstuvwxyz ")` |

# Constraints
- Network: listens on TCP port 8080, up to 64 clients at once
