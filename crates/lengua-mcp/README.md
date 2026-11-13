# lengua-mcp

## Purpose

`lengua-mcp` is the MCP server of the lengua plugin. Later milestones give it the tools `check`
and `run` (see [Lengua3.md](../../Lengua3.md), section 4). At L0 it only shows its version.

## Behaviours

- Given no arguments, when `lengua-mcp` runs, then it writes `lengua-mcp <version>` and a line
  end to standard output and stops with exit code 0.
- Given the argument `--version`, when `lengua-mcp` runs, then it writes the same line and stops
  with exit code 0.
- Given any other arguments, when `lengua-mcp` runs, then it writes `usage: lengua-mcp
  [--version]` to standard error and stops with exit code 2.

## Structure

- `main`: reads the arguments and writes the version line or the usage line.
- `VERSION`: the name of the package and its version, joined by a space.
- `version_flag_prints_the_name_and_the_version` (test): runs the binary with `--version` and
  compares its output.
