# lengua-mcp

## Purpose

`lengua-mcp` is the MCP server of the lengua plugin. It serves MCP over standard input and
output with the official Rust SDK, `rmcp`. At L1 it has one tool, `check`, which checks the
ideas of a program's folder with `lengua-idea` and replies with every problem as structured
JSON. Later milestones add the build, the tests, the lock and the tool `run` (see
[Lengua3.md](../../Lengua3.md), section 4).

## Behaviours

- Given no arguments, when `lengua-mcp` runs, then it serves MCP on standard input and output
  until its standard input closes.
- Given the argument `--version`, when `lengua-mcp` runs, then it writes `lengua-mcp
  <version>` and a line end to standard output and stops with exit code 0.
- Given any other arguments, when `lengua-mcp` runs, then it writes `usage: lengua-mcp
  [--version]` to standard error and stops with exit code 2.
- Given a client that lists the tools, when the server replies, then the list has one tool,
  `check`, with an input schema (`folder`) and an output schema.
- Given a call of `check` with a folder, when the server replies, then the structured content
  has `ok`, `checked_by`, `findings` (each with `file`, `line`, `code` and `message`), `notes`
  and `written`.
- Given a call of `check` with a path that is not a folder, when the server replies, then the
  result is a tool error.

## Structure

- `main`: reads the arguments and writes the version line or the usage line, or serves.
- `serve`: starts the Tokio runtime on one thread and serves `Lengua` on standard input and
  output.
- `Lengua::check`: the tool; checks the folder and turns the report into the reply.
- `reply`: turns a `lengua_idea::Report` into a `CheckReply`.
- `now`: the seconds since 1970, for the log's date.
- `CheckParams`, `CheckReply`, `FindingReply`, `NoteReply`: the tool's input and output, with
  their JSON schemas.
- `VERSION`: the name of the package and its version, joined by a space.
- `tests/version.rs`: runs the binary with `--version` and with other arguments.
- `tests/check.rs`: starts the server, sends `initialize`, lists the tools and calls `check`
  on the fixtures of `lengua-idea`.
