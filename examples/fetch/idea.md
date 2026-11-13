# Purpose

The program sends `ping` to a server and writes the answer of the server. It reads the address
of the server from its first argument. The idea grants one address only, `127.0.0.1:7801`, so
the host refuses a connection to any other address. When the connection fails, the program
writes `refused` and stops with the exit code 1.

# Behaviours

## Write the answer of a granted server

Given a server at `127.0.0.1:7801` that answers `pong`,
When the program connects to `127.0.0.1:7801`,
Then it writes the answer `pong` and a line end.

### Example: the granted server

- Server `127.0.0.1:7801`: "pong\n"
- Arguments: "127.0.0.1:7801"
- Expect standard output: "pong\n"
- Expect exit code: 0

## Refuse a server that the idea does not grant

Given a server at `127.0.0.1:7802` that answers `secret`,
When the program connects to `127.0.0.1:7802`,
Then the host refuses the connection, and the program writes `refused`.

### Example: an address with no grant

- Server `127.0.0.1:7802`: "secret\n"
- Arguments: "127.0.0.1:7802"
- Expect standard output: "refused\n"
- Expect exit code: 1

# Constraints

- Reads: arguments
- Connects to: `127.0.0.1:7801`
- Memory: 16 MiB
- Time: 5 s

# Structure

- `main`: reads the address, sends `ping` to it and writes the answer, or `refused`.
- `fetch`: connects to an address, sends `ping` and gives the answer, or nothing when the
  connection fails.
