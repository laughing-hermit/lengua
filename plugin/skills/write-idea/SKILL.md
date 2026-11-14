---
name: write-idea
description: Turns an approved vision.md, or a small request, into lengua idea files (idea.md, ideas/<module>.md, ideas/ui/<screen>.md) for a Rust back end compiled to WebAssembly and a Next.js front end, with behaviours as Given/When/Then tests, the grants, any native part, crate or npm package with its reason, and the structure, in ASD-STE100 Simplified Technical English; calls check until it reports no problems, then asks the person to approve the ideas in their own words. Use when a person asks for a new lengua program or a change to one.
license: Apache-2.0
---

# write-idea

An idea file is the program's only documentation and the source of its tests. No code is
written before the person approves it.

## Steps

1. If the folder has an `index.md`, read it, then `vision.md`, `api.md` and the ideas it
   names. For a new application, start with the skill `vision`; a small request with no vision
   (one program, no screens, no native part) may start here.
2. Say "Where it runs" to the person before anything else: one WebAssembly file, a web app, a
   desktop app, or a native part and why. Offer a WebAssembly way when one exists.
3. Write `idea.md` with `# Purpose`, `# Behaviours`, `# Constraints` and `# Structure`. See
   [the grammar](references/grammar.md) and [the test format](references/test-format.md).
4. For a larger program, write one `ideas/<name>.md` per module and one `ideas/ui/<name>.md`
   per screen, and list them in ``- Modules: `cart`, `ui/basket` ``.
5. Grant only what the program needs. The top idea always has `- Memory:` and `- Time:`.
6. Take a crate or an npm package only when it merits it (below), with its reason.
7. Write every sentence in STE. See [the STE rules](references/ste.md).
8. Call the MCP tool `check` with the folder's absolute path. Fix every finding, and call it
   again until `ok` is true.
9. Ask the person to approve (below).

## Rust and Next.js, full stack

- The back end is Rust compiled to one WebAssembly component. A server has
  `- Serves: HTTP on TCP port <n>` and a `- Route:` for each route the front end calls.
- The front end is Next.js screens in `ideas/ui/`, exported statically and served by the back
  end, so a web app is still one file. AI keys stay on the back end.
- Pure logic that both sides need (validation, formatting) is a lengua library
  (`- Exports:`), used by the server and imported by the screens.
- An AI call is ``- Asks: an AI model, for <what>, up to <N> calls and <M> tokens``; the
  examples give ``- Model answers:``.
- A desktop app is the same front end and back end in Electron:
  `- Native: a desktop window, for the Electron shell of a desktop app` (L10).
- Only a part that WebAssembly can't do is native: a program folder of its own whose idea says
  `- Native: <what>, for <reason>`, started by the WebAssembly program, which names it with
  ``- Native: `<folder>`, for <reason>`` (L10). The reasons are in the grammar.

## Crates and npm packages: pure Rust first, and only when they merit it

Use the standard library, then lengua's shared libraries (``- Uses:
`<path>/libraries/<name>` ``: `json`, `http`, `decimal`, `text`, `sort`, `hashes`, `dates`,
`ai`, `logins`, `vectors`), then your own code. A crate or a package merits it only when
writing it would be a large project of its own, or it is the component's own interface to its
host (`wasip2` for a server), and nothing else can do the job. Take a pure-Rust crate; `check`
notes a crate that is not pure Rust, and the vision says why. Saving a few hundred lines never
merits it.

## The person approves

1. Tell the person that the ideas are ready, and which files they are. They read them in
   whatever they use to read files.
2. The person says, in their own words, that they approve ("I approve", "looks good, go
   ahead"), or what to change. Change the ideas, call `check`, and ask again.
3. When they approve, call the MCP tool `approve` with the folder, the person's name and
   their exact words. Then go on with the skill `build`.
4. After a change of a behaviour, a grant, a native part, a crate or a package, `check`
   refuses to build (`idea-unapproved`) until the person approves again. A change of the
   purpose or the structure alone needs no new approval.

Never call `approve` because you think the ideas are good, and never put words in the
person's mouth. lengua can't tell who calls the tool: its record in the frontmatter, the lock
and `log.md` is only as true as the agent that wrote it.

## Don't

- Don't write the frontmatter keys that lengua writes, `approved` among them; see the grammar.
- Don't edit `index.md`, `log.md`, `api.md` or the `# Links` section; `check` writes them.

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
