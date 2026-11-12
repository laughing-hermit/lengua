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
