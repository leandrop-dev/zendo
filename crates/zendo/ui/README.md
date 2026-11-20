# zendo/ui

Zendo's frontend: SvelteKit 3 in SPA mode (`ssr = false`, `adapter-static`
with `fallback: 'index.html'`), Svelte 5 runes, TypeScript, Tailwind CSS 4 and
shadcn-svelte components. The Rust side is `../` (see its README).

## Running it

The real target is the Tauri window (WebKitGTK), so run it from the crate:

```sh
ui/node_modules/.bin/tauri dev     # from crates/zendo
```

The frontend alone, in a browser, is enough for layout work; the IPC is not
there, so the pages keep Zendo's own colours and show message ids instead of
words:

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
- `src/routes/+layout.svelte`: the shell; starts the store, listens for F11 and
  Escape, and draws the top bar around the page.
- `src/routes/`: one route per page: the Library with the grid, `/game/[id]`
  the game page, `/store` and `/store/[id]`, `/downloads`, `/stats`,
  `/settings`, `/accounts` and `/about`.
- `src/routes/+page.svelte`: the Library page: the source and sort chips, the
  organize and bulk chips, the grid, and the keyboard's moves.
- `src/lib/components/library/`: the card (with its mark button), the
  virtualized grid (and its FPS harness) and the chip rows.
- `src/lib/components/top-bar.svelte`: the tabs, the view chips and search on
  the Library, the Downloads and Settings buttons and the Settings row.
- `src/lib/library.svelte.ts`: the Library's store; the cards by id, the
  shown order, the chips and the cursor, all from Rust, plus the covers and
  statuses it hears about.
- `src/lib/grid.ts`: the grid's arithmetic (cards across, rows on screen),
  with its tests.
- `src/lib/state.svelte.ts`: the shell's store; the words, the palette and the
  events Rust pushes.
- `src/lib/theme.ts`: the palette as CSS variables, and Zendo's own for a
  browser preview.
- `src/lib/api.ts`: the typed boundary; each function wraps one command.
- `src/lib/components/ui/`: shadcn-svelte's copies (`npx shadcn-svelte add`).
  Their navigation-link rule is off in `eslint.config.js`; they take any href.
- `components.json`: the shadcn-svelte setup (`vega`, neutral, Lucide). The
  font is JetBrains Mono in `static/fonts` (OFL-1.1); the design's type is
  mono throughout.
- `src/routes/layout.css`: Tailwind, the shadcn variables, the font faces, and
  Zendo's own colours as the first paint before `bootstrap` answers.
