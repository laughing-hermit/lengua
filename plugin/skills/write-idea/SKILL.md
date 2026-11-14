---
name: write-idea
description: Turns a person's request into a lengua idea file, idea.md (and ideas/<module>.md, ideas/ui/<screen>.md for a larger program), with its purpose, its behaviours as Given/When/Then tests with examples, its constraints (the grants) and its structure, in ASD-STE100 Simplified Technical English, then calls the check tool until it reports no problems. Use when a person asks for a new lengua program or a change to one.
license: Apache-2.0
---

# write-idea

An idea file is the program's only documentation and the source of its tests. Write it so
that the MCP tool `check` reports no problems. The full grammar is in `Lengua3.md`, section 4,
"The idea file's grammar".

## Steps

1. Read the request. Decide what the program reads, what it writes, and what it may touch.
2. Make the folder. Write `idea.md` with these four sections, in this order:
   - `# Purpose`: what the program is for, in a few short sentences.
   - `# Behaviours`: one `## name` per behaviour, with a `Given`, a `When` and a `Then` line,
     then one `### Example: name` block or more. See
     [the test format](references/test-format.md).
   - `# Constraints`: one `- Key: value` line each. The top idea always has `- Memory:` and
     `- Time:`. Grant only what the program needs: `- Reads: standard input`,
     ``- Reads: files in `data/` ``, ``- Writes: files in `out/` ``,
     ``- Connects to: `host:443` ``. Standard output and standard error are always granted.
   - `# Structure`: one ``- `name`: one sentence.`` per function, `main` first.
3. For a program of more than one module, write one `ideas/<name>.md` per module and one
   `ideas/ui/<name>.md` per screen, each with Purpose, Behaviours and Structure. Name each one
   in the top idea's ``- Modules: `cart`, `ui/basket` ``. A module that uses another says
   ``- Uses: `cart` `` in its own `# Constraints`.
4. Write every sentence in STE. See [the STE rules](references/ste.md).
5. Do not write the frontmatter keys that lengua writes (`type`, `description`, `module`,
   `source`, `uses`, `used_by`, `label`, `idea_sha256`, `source_sha256`, `generated`,
   `verified`). You may write OKF's `title`, `tags`, `sources` and `status` (`draft` or
   `deprecated`; an idea becomes `stable` only when the person approves it); `check` keeps them
   and checks their values.
6. Call `check` with the folder's absolute path. It returns every problem at once, each with
   its file, line, code and message. Fix all of them, then call `check` again. Repeat until
   `ok` is true.
7. Do not edit `index.md`, `log.md`, the frontmatter or the `# Links` section at the end of
   each idea by hand; `check` writes them.

## A small idea

````
# Purpose

The program counts the words in a text. It reads the text from the standard input.

# Behaviours

## Count the words in a text

Given a text with two words on the standard input,
When the program runs,
Then it writes 2 and a line end to the standard output.

### Example: two words

- Standard input: "hello world\n"
- Expect standard output: "2\n"
- Expect exit code: 0

# Constraints

- Reads: standard input
- Memory: 16 MiB
- Time: 2 s

# Structure

- `main`: reads the standard input and writes the number of words.
````
