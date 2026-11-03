# lengua specs

A small Next.js site where people share lengua specs, never code, and anyone compiles them for
their own computer. It's built and run locally, and isn't deployed yet.

```sh
npm ci --ignore-scripts
npm run build
npm start          # http://localhost:3000
```

- `/specs` lists the shared specs, lengua's own examples (`../examples`) among them, and each
  spec's page shows it with a download of its spec.md.
- `/share` takes a spec.md, uploaded or pasted. The site keeps it under `shared/` (not committed)
  only if it's a lengua spec (front matter of type `lengua/program-spec`, a title, a Purpose and
  Behaviors) of up to 256 KiB, holding no code block and no assembly. Only the site's own form can
  post to it.
- It promises only what lengua enforces: the spec travels and every copy compiled from it passes
  the same checks, never the same program; each computer compiles and checks its own code; main.s
  is plain text anyone can build outside lengua, so the guarantees hold only through lengua's own
  `run`; and a proof covers what its line says.

Its dependencies are pinned to versions at least two weeks old when chosen, and installed with
their install scripts off.
