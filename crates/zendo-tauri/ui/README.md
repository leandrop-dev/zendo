# zendo-tauri/ui

Zendo's frontend: SvelteKit 3 in SPA mode (`ssr = false`, `adapter-static`
with `fallback: 'index.html'`), Svelte 5 runes, TypeScript, Tailwind CSS 4 and
shadcn-svelte components. The Rust side is `../` (see its README).

## Running it

The real target is the Tauri window (WebKitGTK), so run it from the crate:

```sh
ui/node_modules/.bin/tauri dev     # from crates/zendo-tauri
```

The frontend alone, in a browser, is enough for layout work; the pages skip
the IPC there and say so (`src/lib/tauri.ts`):

```sh
npm run dev
```

## Checks

```sh
npm run check   # svelte-check
npm run lint    # prettier --check + eslint
npm test        # vitest, jsdom
npm run build   # build/, what tauri.conf.json's frontendDist points at
```

## Where things are

- `src/routes/+layout.ts`: the SPA switch (`ssr = false`).
- `src/routes/+page.svelte`: slice 0's placeholder page, which reads the app
  version over IPC.
- `src/lib/tauri.ts`: the typed boundary; commands wrap one Rust function each
  and land slice by slice (`docs/tauri-port-plan.md`).
- `src/lib/components/ui/`: shadcn-svelte's copies (`npx shadcn-svelte add`).
  Their navigation-link rule is off in `eslint.config.js`; they take any href.
- `components.json`: the shadcn-svelte setup (`vega`, neutral, Lucide). The
  monospace type of the design is a slice 1 decision, not this preset's font.
