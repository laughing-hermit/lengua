---
type: lengua/program-idea
title: echo-at-once
description: Sends each line back to its client, for up to 8 clients at once.
---
# Purpose
This program is a network service for several clients at once. On each connection, it reads a
line of at most `1023` characters and a newline. It sends the line back and closes the connection.
It answers each client when the line of that client is complete, and it does not wait for the
other clients.

It reads until the data ends with a newline, until the client closes its side, or until it has
1024 bytes. Then it sends back exactly the data that it read. It serves up to 8 clients at once on
TCP port 7007 of this computer. More clients wait for their turn.

# Behaviors
## B1: Echoes a line
- Given a client that sends `hello` and a newline
- When the program runs
- Then the client receives `hello` and a newline
- And it does not stop

### For any `line`
- Given a client that sends `{line}` and a newline
- When the program runs
- Then the client receives `{line}` and a newline
- And it does not stop

| line |
|---|
| `string(0, 1023, "abcdefghijklmnopqrstuvwxyz ")` |

## B2: Answers clients at once
- Given a client that sends `first` and a newline
- And a client that sends `second` and a newline
- And the clients connect at once
- When the program runs
- Then the client receives `first` and a newline
- And the client receives `second` and a newline
- And it does not stop

### For any `a` and `b`
- Given a client that sends `{a}` and a newline
- And a client that sends `{b}` and a newline
- And the clients connect at once
- When the program runs
- Then the client receives `{a}` and a newline
- And the client receives `{b}` and a newline
- And it does not stop

| a | b |
|---|---|
| `string(0, 1023, "abcdefghijklmnopqrstuvwxyz ")` | `string(0, 1023, "abcdefghijklmnopqrstuvwxyz ")` |

# Constraints
- Network: listens on TCP port 7007, up to 8 clients at once
