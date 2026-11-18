# zendo-tauri

Zendo's window in Tauri 2, beside the Slint window in `crates/zendo` until the
port's parity checklist passes (`docs/tauri-port-plan.md`).

Slices 0, 1, 2 and 3a are in place: the scaffold, the shell, the Library
(grid, chips, search, sort, cursor, marks, bulk edits, cover downloads and the
file watch), and the game page's header and actions. The rest of that page and
the other pages follow.

- `src/main.rs`: the command line, `zendo_shell::cli`'s, and the Tauri app.
  `--game-mode`/`--fullscreen` opens full screen.
- `src/platform.rs`: the tray (`ksni`, through `zendo-shell`), the controller
  (`gilrs`, through `zendo-shell`) and the KDE colour scheme, watched every 2
  seconds. Closing hides to the tray when `minimize_to_tray` is on and the
  tray could be made; otherwise it quits.
- `src/commands.rs`: `bootstrap` (words, languages, palette, full screen and
  tray state), `tr` (a message with values) and `window_fullscreen_toggle`.
- `src/game.rs`: the game page's commands: `game_get`, `game_launch`,
  `game_stop`, `game_set_pinned`, `game_set_hidden` and `game_set_status`,
  over `zendo_shell::running` for the launch.
- `src/library.rs`: the Library's commands over `zendo_shell::library`: the
  list (every card once), the chips, the search, the sort, the cursor, the
  marks, and the bulk edits. It scans on a thread, watches the launchers'
  files every 10 seconds, and fetches the covers that are missing. Covers are
  files in `$CACHE/zendo/covers`, read through the asset protocol (enabled in
  `tauri.conf.json` with that scope).
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
- `library_loaded`: a scan is in; the page asks for the list again.
- `cover_changed`: a downloaded cover, with the game's id and its path.
- `library_status`: a line the page shows, such as the count of images that
  could not be downloaded.
- `library_changed`: a mark or a count changed on a game's page; the Library
  asks for its list again.
- `game_run` and `game_line`: the running game's state (idle, starting,
  running, stopping) and the line under its actions.
- `pad`: a controller move (`left`, `right`, `up`, `down`, `activate`, `back`,
  `sort`, `hide`, `show-hidden`, `toggle-game-mode`, `prev-shelf`,
  `next-shelf`). The frontend decides what each does: the grid moves the
  cursor, Sort cycles the order, Show hidden toggles the view, Select toggles
  full screen until Game Mode.

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

The FPS harness, which needs no input: `ZENDO_FPS=1` makes the grid scroll
itself through the whole list and show the frame rate and the worst half
second. `scripts/seed-library.py --home <dir> --count 2700` writes a large
throwaway library and its covers (it needs Pillow):

```sh
scripts/seed-library.py --home /tmp/zendo-fps --count 2700
env -i PATH="$PATH" HOME=/tmp/zendo-fps \
    XDG_DATA_HOME=/tmp/zendo-fps/.local/share \
    XDG_CACHE_HOME=/tmp/zendo-fps/.cache ZENDO_FPS=1 target/debug/zendo-tauri
```

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
