---
type: lengua/program-idea
title: echo
description: A line-echo server that sends each connection's line back.
---
# Purpose
A small network service for trying out clients: on each connection it reads a line of at most
`1023` characters followed by a newline, sends it back, and closes the connection, then waits for
the next one. It reads until what it has ends with a newline, the client closes its side, or it
holds 1024 bytes, and sends back exactly what it read. It serves on TCP port 7007 of this
computer.

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

# Constraints
- Network: listens on TCP port 7007
