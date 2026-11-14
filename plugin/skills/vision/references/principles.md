# lengua's principles

Judge every plan against these principles before any idea is written. For each one, say in
`vision.md` whether the plan meets it easily or pushes against it, and why. The table is the
same as the design's (`Lengua3.md`, "The vision"); a test holds the two to the same rows.

## The principles

| # | Principle | Do | Don't |
|---|---|---|---|
| 1 | An AI language | Write the idea as the source and the only documentation; let the AI compile it to Rust for the back end and TypeScript for the front end; let the machine verify both | Trust code because an AI wrote it; document anything outside the idea |
| 2 | Maximum interoperability | Build one file that runs on any OS and CPU, whose front end opens in any current browser and in Electron; deploy the web front end on Vercel natively and the back end as a `lengua-run` container serving the one .wasm; or `lengua-run` on any server; Electron for desktop | Build per system when WebAssembly can do the job; rewrite the back end natively or through a JavaScript shim to fit a host |
| 3 | WebAssembly first | Rust to WebAssembly for anything that can be; native Rust only for the specific parts that truly need it (training or fine-tuning models, the heaviest GPU work, shared-memory threads, hardware without a host grant). Put shared pure logic in a lengua library that runs in the browser and on the server; do GPU work for the screen (3D, charts, effects, small to medium AI models with WebGPU, for example WebLLM) in the front end; call a cloud model or a local model server through `- Asks:` from the WebAssembly back end; say for each part which row of "Which part does what" it is, and why WebGPU stops short when it does | Go native for convenience, speed alone, or to fit one host |
| 4 | Nothing unless absolutely necessary | Use the standard library, then lengua's shared libraries, then the agent's own code; take a pure-Rust crate or an npm package only when it merits it, declared with its reason and pinned, and say when a crate is not pure Rust; download front-end tools only when the app needs them | Add a crate or a package to save a few hundred lines; take a crate that wraps C when a pure-Rust one exists; bundle tools an app does not use; grant access the program doesn't need |
| 5 | Full stack, web first | Put the Next.js front end, the server, storage, logins and AI calls in the one file; keep AI keys on the back end | Split one application into separate deployables; use a Next.js feature that needs a Node server; call a model from the browser with a key |
| 6 | The Power of 10 | Plan small functions, bounded loops, plain data, every error and promise handled, in Rust and in TypeScript | Plan recursion, unbounded loops, global state, `unwrap`, `unsafe`, `any`, casts, a floating promise |
| 7 | Trust you can see | Say "Where it runs" before any code, and what will be `checked`, `tested`, `trusted` or `native` | Hide a trusted component, a native part or a crate that is not pure Rust |
| 8 | Documentation that can't drift | Keep the vision, the ideas and the code signed together; name every function in its idea | Write comments in code; change code without its idea, or an idea without a check |
| 9 | The person decides | Ask the person to read and approve the vision and the idea in their own words | Write code before approval; decide for the person what only they can |

## Which part does what

The line: Rust to WebAssembly for anything that can be; native Rust only for the specific
parts that can't. Put each part of the request in one row, and name the row in `vision.md`.

| Work | Where | Built with | Native? |
|---|---|---|---|
| Screens, forms, navigation, layout, visualisations, AI chat UI | Front end | Next.js (TSX), shadcn/ui, Vercel's AI-UI tools | No |
| 3D, charts, effects, image and video processing | Front end | WebGPU in the browser or Electron's Chromium | No |
| Small to medium AI models on the user's machine | Front end | WebGPU (for example WebLLM) | No |
| Data, business rules, storage, logins, secrets, APIs, calls to other services | Back end | Rust compiled to WebAssembly (one file) | No |
| Calling a cloud model or a local model server (for example Ollama on the same machine) | Back end | WebAssembly, through the `- Asks:` grant (L9) | No |
| Shared pure logic (validation, calculations, formatting) | Front end and back end | A lengua library, imported by Next.js as WebAssembly and linked into the back end | No |
| Large AI models, the fastest local inference, training or fine-tuning | Back end | A native Rust part (Candle or Burn, on CUDA or Metal) | Yes, declared and labelled |
| Heavy compute that needs threads sharing memory | Back end | A native Rust part | Yes, declared and labelled |
| A hardware device with no host grant | Back end | A native Rust part | Yes, declared and labelled |
| Desktop window and shell | Desktop | Electron, running the same front end and back end | Yes (the shell), declared |
| Shared types between the front end and the back end | Interface | Generated from the back end's routes (`api.md`, `web/lib/lengua.ts`) | No |

## WebAssembly first, native only when necessary

- Do: make the back end one WebAssembly component, the server and the front end's static
  export in one file.
- Do: declare a native part with `- Native: <part>, for <reason>`, the reason one of: `large AI
  models`, `the fastest local inference`, `training or fine-tuning`, `heavy compute with
  shared-memory threads`, `a device with no host grant`, `the Electron shell of a desktop app`.
- Do: keep a native part narrow: a program folder of its own, called by the WebAssembly program
  through the host, labelled `native`.
- Don't: go native for convenience, for speed alone, or to fit one host.
- Don't: give front-end GPU work as a reason: the front end has WebGPU.

## The GPU, said up front

Tell the person which case the request is before anything else about it:

| Case | Where | Native? |
|---|---|---|
| Drawing and light compute: 3D, charts, maps, effects, image and video processing | The front end, WebGPU in the browser or in Electron's Chromium | No |
| Small or medium AI models on the user's machine | The front end, WebGPU (WebLLM, Transformers.js) | No |
| A cloud model or a local model server (Ollama) | The back end, through `- Asks:` | No |
| Large models at full speed, training, fine-tuning, the heaviest GPU work | A native Rust part, pure Rust first (Candle or Burn) | Yes |

Why WebGPU stops short: a page gets limited memory, 32-bit WebAssembly reaches 4 GB, it is
slower than CUDA or Metal, weights are downloads of gigabytes, and training is not practical.
The WebAssembly back end has no GPU.

## Crates and npm packages: few, and only ones that merit them

The order: the Rust standard library, then lengua's shared libraries (`json`, `http`,
`decimal`, `text`, `sort`, `hashes`, `dates`, `ai`, `logins`, `vectors`), then the agent's own
code as a lengua library, and only then a crate.

A crate or an npm package merits it only when all of these hold:

1. Writing it would be a large project of its own (a GPU renderer, a model runtime, TLS), or it
   is the component's own interface to its host (`wasip2`).
2. The standard library, lengua's libraries and the agent's own code can't do the job.
3. It builds for every target the application needs; an npm package works in a static export.

- Do: take a pure-Rust crate. Say in `vision.md` when a crate is not pure Rust (it builds C or
  links a native library) and why no pure-Rust one does the job; `check` flags it too.
- Do: pin it to one version and give its reason: ``- Uses crate: `<name>` <version>, for
  <reason>``.
- Don't: take a crate or a package to save a few hundred lines. Axum and Tokio are not needed:
  a WASI component has one thread.

## No comments

- Do: write the idea as the documentation: every function and component named in its idea's
  `# Structure`, with one sentence.
- Don't: write a comment or a doc comment in Rust or TypeScript; `check` refuses it.

## AI calls

- Do: ask a model from the back end with ``- Asks: an AI model, for <what>, up to <N> calls and
  <M> tokens``; the person who runs the program chooses the model (`LENGUA_MODEL_URL`).
- Do: treat every answer as untrusted input, and give the tests a ``- Model answers:``.
- Don't: hold an API key in the program or in the browser.

## Deployment

| Target | How |
|---|---|
| The web front end | Natively on Vercel, as its static export |
| The back end | A container of `lengua-run` serving the one file (Vercel's Container Runtime, or any container host), or `lengua-run --listen 0.0.0.0:<port> <folder>` on any server |
| The desktop | Electron: `- Native: a desktop window, for the Electron shell of a desktop app`, run with `lengua-run --desktop <folder>` |

- Do: say where it will be deployed in `vision.md`.
- Don't: rely on a disk that lasts on Vercel: lasting storage there is a service reached with
  ``- Calls: HTTPS at `<host>:443` `` and a key in a secret.
- Don't: use a Next.js feature that needs a Node server (Route Handlers, Server Actions,
  middleware, ISR, `next/image` optimization, dynamic routes without static params).
