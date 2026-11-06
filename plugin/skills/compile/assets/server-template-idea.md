---
type: lengua/program-idea
title: echo-at-once
description: A line-echo server that serves up to 8 clients at once.
---
# Purpose
A network service that serves several clients at once: on each connection it reads a line of at
most `1023` characters followed by a newline, sends it back, and closes the connection, answering
each client as soon as its line is complete, whatever the others are doing. It reads until what
it has ends with a newline, the client closes its side, or it holds 1024 bytes, and sends back
exactly what it read. It serves up to 8 clients at once on TCP port 7007 of this computer; more
wait their turn.

# Behaviors
## B1: Echoes a line
- Given a client that sends `hello` followed by a newline
- When the program runs
- Then the client gets back `hello` followed by a newline
- And it keeps serving

### For any `line`
- Given a client that sends `{line}` followed by a newline
- When the program runs
- Then the client gets back `{line}` followed by a newline
- And it keeps serving

| line |
|---|
| `string(0, 1023, "abcdefghijklmnopqrstuvwxyz ")` |

## B2: Answers clients at once
- Given a client that sends `first` followed by a newline
- And a client that sends `second` followed by a newline
- And the clients connect at once
- When the program runs
- Then the client gets back `first` followed by a newline
- And the client gets back `second` followed by a newline
- And it keeps serving

### For any `a` and `b`
- Given a client that sends `{a}` followed by a newline
- And a client that sends `{b}` followed by a newline
- And the clients connect at once
- When the program runs
- Then the client gets back `{a}` followed by a newline
- And the client gets back `{b}` followed by a newline
- And it keeps serving

| a | b |
|---|---|
| `string(0, 1023, "abcdefghijklmnopqrstuvwxyz ")` | `string(0, 1023, "abcdefghijklmnopqrstuvwxyz ")` |

# Constraints
- Network: listens on TCP port 7007, up to 8 clients at once
