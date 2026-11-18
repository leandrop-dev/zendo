# zendo-tauri

Zendo's window in Tauri 2, beside the Slint window in `crates/zendo` until the
port's parity checklist passes (`docs/tauri-port-plan.md`).

Slices 0 and 1 are in place: the scaffold and the shell. The Library grid and
the pages follow.

- `src/main.rs`: the command line, `zendo_shell::cli`'s, and the Tauri app.
  `--game-mode`/`--fullscreen` opens full screen.
- `src/platform.rs`: the tray (`ksni`, through `zendo-shell`), the controller
  (`gilrs`, through `zendo-shell`) and the KDE colour scheme, watched every 2
  seconds. Closing hides to the tray when `minimize_to_tray` is on and the
  tray could be made; otherwise it quits.
- `src/commands.rs`: `bootstrap` (words, languages, palette, full screen and
  tray state), `tr` (a message with values) and `window_fullscreen_toggle`.
- `tauri.conf.json`: the window (title `Zendo`, 1360x860, minimum 1100x560),
  the dev loop and the build. The product name is `zendo`, which is the window
  class the compositors' rules key on; `main` sets GTK's program name to it,
  since the product name alone does not.
- `capabilities/default.json`: the main window's permissions, `core:default`
  for now; each Zendo command gets its own capability as it lands.
- `ui/`: the SvelteKit frontend (`ui/README.md` has its own commands).

## Events

- `theme_changed`: the palette, when `kdeglobals` changed while the theme is
  the KDE scheme.
- `pad`: a controller move (`left`, `right`, `up`, `down`, `activate`, `back`,
  `sort`, `hide`, `show-hidden`, `toggle-game-mode`, `prev-shelf`,
  `next-shelf`). The frontend decides what each does; the grid's moves arrive
  with the Library.

## The dev loop

From this directory:

```sh
ui/node_modules/.bin/tauri dev
```

It runs `npm run dev` itself (Vite on `localhost:1420`, strict port), from the
frontend directory it derives from `frontendDist`, then builds and starts the
debug shell pointed at the dev server. The frontend alone is enough for layout
work (`npm --prefix ui run dev`): in a plain browser the IPC is not there, so
the pages keep Zendo's own colours and show message ids.

A production build without a bundle (the bundler is off until the release
pipeline):

```sh
ui/node_modules/.bin/tauri build [--debug]
```

## Checks

```sh
npm --prefix ui run check   # svelte-check
npm --prefix ui run lint    # prettier + eslint
npm --prefix ui test        # vitest, jsdom
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`tauri-build` writes `gen/schemas` at build time; it is git-ignored. The font
links in `ui/static/fonts` point at `crates/zendo/fonts`, and the adapter's
build copies the files, so the binary carries them.
