# The front end: Next.js screens in TSX

`check` writes every file of `web/` but the screens: `package.json` (the base and the declared
packages, pinned), `next.config.mjs` (a static export), `tsconfig.json` (at its strictest),
the routes in `web/app/`, the shadcn/ui components that `- Uses shadcn:` names, and
`web/lib/lengua.ts`. You write only `web/screens/<name>.tsx`. pnpm downloads Next.js the first
time the app is built; a hand edit of a generated file is a finding.

## A screen

```tsx
import { Button } from "@/components/ui/button";
import { assert, getNotes, useScreen } from "@/lib/lengua";

interface HomeState {
  readonly count: number;
  readonly error: string;
}

export async function load(state: HomeState, set: (next: HomeState) => void): Promise<void> {
  assert(state.count >= 0, "a count is not negative");
  const answer = await getNotes();
  assert(answer.status >= 100, "the server answered");
  set(answer.ok ? { ...state, count: answer.value.length } : { ...state, error: "No notes." });
}

export function HomeScreen() {
  const [state, setState] = useScreen<HomeState>({ count: 0, error: "" });
  return (
    <main className="mx-auto max-w-xl p-6">
      <p>{state.count} notes</p>
      <Button
        onClick={() => {
          load(state, setState).catch(() => {
            setState({ ...state, error: "No answer." });
          });
        }}
      >
        Load
      </Button>
    </main>
  );
}
```

- The main component is `<Name>Screen`; it and every other function are in the idea's
  `# Structure`.
- `useScreen(first)` keeps the screen's state; its fields are the `State` keys of the
  examples.
- `assert(condition, message)` from `@/lib/lengua` throws when the condition is false.
- `queryParam(name)` reads the query string: a screen for one item is `/note/?id=3`, since
  each path is a file of the export.
- Link to another screen with `next/link`: `/` for `home`, `/<name>/` for the others.

## The back end, typed

`web/lib/lengua.ts` has one function for each `- Route:`, named from the method and the path:
`GET /api/notes` is `getNotes()`, `GET /api/notes/{id}` is `getNotesById(id)`, `POST /api/ui`
is `postUi(body)`. Each takes the session's token last (`token = ""`, sent as `authorization: Bearer <token>`) and gives an
`Answer`: `{ ok: true, status, value }` or `{ ok: false, status, error }`, with the types of
the examples. A screen calls the back end only through these (`ts-fetch` otherwise).

## A lengua library in the browser

A library the program uses (`- Uses:`) is also built for the browser: import it as
`@/lib/wasm/<library>` and call its functions with strings, booleans, integers up to 32 bits
and `f64`. The server runs the same code.

## AI on the screen

json-render (`@json-render/core`, `@json-render/react`, with `zod`) draws a JSON description
of a screen that the back end got from a model, only with the components of a catalog the
screen defines. The browser never holds a key and never calls a model; a small model in the
browser on WebGPU needs no key and no grant.

## What a static export can't do

No Route Handlers, Server Actions, middleware, `cookies()`, ISR, `next/image` optimization or
dynamic routes: the Rust back end does that work.

## A desktop app

The same screens and back end in Electron: the idea says `- Native: a desktop window, for the
Electron shell of a desktop app`, `check` writes `web/desktop/main.mjs`, and
`lengua-run --desktop <folder>` opens the window.
