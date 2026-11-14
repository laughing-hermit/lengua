---
name: vision
description: Works out with a person the best lengua solution for what they want to build, before any idea or code, and writes it to vision.md in the application's folder for the person to approve. Says where it runs (one WebAssembly file, a web app, a desktop app, or which native part and why), the GPU case, the shape, what it reaches, where it deploys and what it is built from, judged against lengua's principles. Use when a person asks for a new application, or a change that adds a grant, a native part, a crate or a package.
license: Apache-2.0
---

# vision

The vision is the person's plan in plain words. It comes before the ideas, and nothing is
written from it until the person approves it.

## Steps

1. If the folder has an `index.md`, read it, then `vision.md` and the pages it names.
2. Read the request. Ask the person only what you can't decide: who uses it, what it must keep,
   what it must reach. Decide the rest from [the principles](references/principles.md).
3. Put each part of the request in one row of "Which part does what". Say the GPU case first if
   there is one. Offer a WebAssembly way when the person asks for something native.
4. Write `vision.md` in the application's folder, in the form below.
5. Tell the person that the vision is ready and where it is. Ask them to read it and to say,
   in their own words, that they approve it or what to change.
6. Change it until they approve. Then call the MCP tool `approve` with the folder, the
   person's name and their exact words. Never call `approve` on your own judgement.
7. Go on with the skill `write-idea`.

## vision.md

```
---
type: lengua Vision
title: <the application's name>
description: "<one sentence: what it is>"
status: draft
---

# Request

> <the person's request, in their own words>

# Where it runs

<one line: one WebAssembly file / a web app / a desktop app / a native part, and why>

# GPU

<the case, or "None.">

# Shape

<the modules, the screens, what is on the server and what in the front end, storage, logins,
calls to services and to AI models; each part with its row of "Which part does what">

# Reaches

<files, network, secrets, storage, AI calls: every grant it needs>

# Deployment

<Vercel and a lengua-run container, any server, or the desktop through Electron>

# Built from

<the standard library, lengua's libraries, the agent's own code, and each crate or npm package
with its reason; say when a crate is not pure Rust>

# Limits

<what lengua can't do yet for it, and the alternatives>

# Principles

<one line for each principle: meets it easily, or pushes against it, and why>
```

`check` holds `type: lengua Vision` and `# Where it runs`, and writes `status`. The tool
`approve` writes `approved` in the frontmatter; don't write it yourself.

## When the vision changes

`check` builds nothing while `vision.md` differs from what the person approved. When the ideas
grow past it (a new grant, a native part, a new crate or package), change the vision first and
ask the person again.

## What lengua can't know

lengua can't tell whether the person or the agent called `approve`. The record in the
frontmatter, the lock and `log.md` is only as true as the agent that wrote it. So call
`approve` only after the person's own words, and pass them exactly.
