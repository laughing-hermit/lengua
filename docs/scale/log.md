---
type: note
title: W22, the log
description: Every check cycle of the scale test, as it happened.
---
# W22: the log of the scale test

One agent (Claude, model Fable 5.2, in one interactive session on 2026-11-12) wrote one program
of several thousand lines of `main.wat` from an idea, and then changed it three times. This
file is what happened, in order. [README.md](README.md) has the summary and the verdict.

The agent's computer ran no build and no lengua: it edited files and ran git and gh. Each
`check` was a dispatch of `ci.yml` in its `ignored` mode on Linux on x86-64, with the test
`check_one_folder` and the folder in `check_folder`, which checks each part the folder builds on
first, and gives each folder 60 seconds for all its proofs. So a check cycle cost a build of
lengua on a runner before `check` said anything, and the agent put as much as it could into
each cycle. In the product, `check` answers in seconds: the count of cycles here is what
carries over, and the minutes are the runner's.

## What the agent read first

The two skills as shipped, and nothing of lengua's source for how to write code:
`plugin/skills/write-idea` (SKILL.md, references/test-format.md, parts.md) and
`plugin/skills/compile` (SKILL.md, references/wasm.md, power-of-10-webassembly.md, host.md,
parts.md, the server template and its idea, the parts decimal, bytes and crc32). Where the
skills did not answer a question that a program of this size raises, the agent read lengua's
source for the answer, and each such question is listed under "Where the skills were silent"
in the README: each is a place where an agent with the plugin alone would have guessed.

## The program, and why this one

A web server, `examples/httpd`, built on four parts. lengua has a grant to serve (`- Network:
listens on TCP port N, up to M clients at once`), so the server is a real one: it takes its
connections with `poll_oneoff` and `sock_accept`, as the compile skill's event-loop template
does.

| Folder | What it is | Written for W22 |
|---|---|---|
| `examples/json` | A part: checks one JSON value and writes it with no space between its tokens | yes |
| `examples/http` | A part: the head of an HTTP/1.1 request: its end, its request line, its fields, its path decoded | yes |
| `examples/decimal` | The library's part: numbers read and written | no |
| `examples/crc32` | The library's part: the CRC-32, which is the server's ETag | no |
| `examples/httpd` | The server: health, files with types, ETags, parts of files and lists of folders, a store of JSON values in a file, the errors, the log | yes |

Decisions taken before the first line of code, each because of something the skills or
lengua's tests fix:

- **One request a connection, then close.** A test's client reads until the server closes.
- **The log goes to standard output, and every behavior states its lines.** A test compares
  standard output even when the behavior ends with `it does not stop`.
- **The ETag is the CRC-32 in decimal.** The tests' formulas have `crc32(s)`, and they have no
  date and no substring; `fd_filestat_get` gives no time. So `Last-Modified` was left out.
- **A part of a file is tested with a file of three pieces of five bytes.** A formula can't cut
  a string, so the test builds the file from the pieces it expects back.
- **`Content-Length` in a test's request is a fixed number.** A client's message holds bare
  variables only, so a PUT's body has a fixed length in every generated run.
- **No waiting close.** The server closes after its answer. A request that is refused before
  its body has come (413) is tested with a stated length and no body, so that the test's client
  has nothing left to send when the server closes.
- **All literal text in one read-only region, each text ended by a NUL.** A function lent an
  address reaches a fixed number of bytes from it, whatever length it is given, and a read-only
  region holds exactly its data. So a copy function can't be lent one small text after
  another: each would need the copy's whole reach after it. The server keeps its 54 texts in
  one region and passes their places.
- **Each part's function takes the text's start and a place in it**, never the address of the
  place, for the same reason.

## Before the first check

The agent wrote, in this order: json's code and idea; http's code and idea; httpd's idea (631
lines, 23 behaviors); httpd's code in four pieces (the constants and regions; the loop and the
request; the files; the store), 2,570 lines of its own; then the 24 functions it takes from
the four parts, copied with `sed` by their line numbers, each under its `;; from:` line.

`check` was 20 to 30 minutes away each time, so before each dispatch the agent ran checks of
its own with `grep` and `awk`, which stand in here for what `check` says in seconds:

| Check | What it found before cycle 1 |
|---|---|
| Parentheses that balance | nothing |
| A global read that isn't defined, a function called that isn't defined, a local or a label used that isn't declared | nothing, in 4,862 lines |
| A constant never read, a local never read | nothing |
| Operators a function, counted from the text | `$json_minify` as first written came to about 260, counted by hand before any tool: split in two (`$json_structure`). http's `$main` came to about 225: split in two (`$report`) |
| An assertion number used twice | nothing |
| The places of the 54 texts in `$texts` | computed with `awk` from the texts' lengths, not by hand |

The 180 operators a function are the rule the agent met first, twice, before any check ran.
