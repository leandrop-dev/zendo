# zendo-tauri

Zendo's window in Tauri 2, beside the Slint window in `crates/zendo` until the
port's parity checklist passes (`docs/tauri-port-plan.md`).

- `src/main.rs`: the Tauri shell. Slice 0 has no commands yet; they land slice
  by slice as thin wrappers over `zendo-core` and `zendo-shell`.
- `tauri.conf.json`: the window (title `Zendo`, 1360x860, minimum 1100x560),
  the dev loop and the build. The product name is `zendo`, which is the window
  class the compositors' blur rules key on.
- `capabilities/default.json`: the main window's permissions. `core:default`
  for now; each Zendo command gets its own capability as it lands.
- `ui/`: the SvelteKit frontend (`ui/README.md` has its own commands).

## The dev loop

From this directory:

```sh
ui/node_modules/.bin/tauri dev
```

It runs `npm --prefix ui run dev` first (Vite on `localhost:1420`, strict
port), then builds and starts the debug shell pointed at the dev server. Run
the frontend alone with `npm --prefix ui run dev`; in a plain browser the
pages skip the IPC and say so (`ui/src/lib/tauri.ts`).

A production build without a bundle (the bundler is off until the release
pipeline, per the plan):

```sh
ui/node_modules/.bin/tauri build
```

## Checks

```sh
npm --prefix ui run check   # svelte-check
npm --prefix ui run lint    # prettier + eslint
npm --prefix ui test        # vitest, jsdom
cargo clippy -p zendo-tauri --all-targets -- -D warnings
```

`tauri-build` writes `gen/schemas` at build time; it is git-ignored.
