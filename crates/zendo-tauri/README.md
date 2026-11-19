# zendo-tauri

Zendo's window in Tauri 2, beside the Slint window in `crates/zendo` until the
port's parity checklist passes (`docs/tauri-port-plan.md`).

Slices 0, 1, 2, 3a, 3b, 3c, 3d, 3e-1, 3e-2 and 3e-3 are in place: the
scaffold, the shell, the Library (grid, chips, search, sort, cursor, marks,
bulk edits, cover downloads and the file watch), the game page's header and
actions, its sessions and details, local games, its store cards, the Wine
prefix card with its tools, GOG's changelog and the cloud saves row. The Mods
card and the rest of the other pages follow.

- `src/main.rs`: the command line, `zendo_shell::cli`'s, and the Tauri app.
  `--game-mode`/`--fullscreen` opens full screen.
- `src/platform.rs`: the tray (`ksni`, through `zendo-shell`), the controller
  (`gilrs`, through `zendo-shell`) and the KDE colour scheme, watched every 2
  seconds. Closing hides to the tray when `minimize_to_tray` is on and the
  tray could be made; otherwise it quits.
- `src/commands.rs`: `bootstrap` (words, languages, palette, full screen and
  tray state), `tr` (a message with values) and `window_fullscreen_toggle`.
- `src/prefix.rs`: the Wine prefix card for a Windows game Zendo starts: the
  folder, Open folder, the known fixes, and the tools (`prefix_run`,
  `prefix_install`, `prefix_pick_program`, `prefix_want`) with their state.
  One tool runs per game at a time and none while the game does.
- `src/local.rs`: games added by hand: the form's get, its about line, the
  file dialog, and save and remove, each reading the library again after it
  writes.
- `src/game.rs`: the game page's commands: `game_get`, `game_launch`,
  `game_stop`, `game_set_pinned`, `game_set_hidden` and `game_set_status`,
  over `zendo_shell::running` for the launch.
- `src/cards.rs`: the page's store cards, asked once a session:
  `cards_get`, `cards_retry` (for a card that failed), `cards_more_achievements`,
  `cards_more_changelog` and `steam_app_set`. ProtonDB's rating, Steam's
  store, the requirements, the achievements and GOG's changelog are asked on
  threads of their own; a failure is kept and shown with Try again. The words
  are `zendo_shell::cards` and `zendo_shell::gog`, shared with the Slint
  window.
- `src/cloud.rs`: the Details card's cloud saves row (off, syncing, waiting
  for the first start, in sync when, none kept, or failed), from
  `zendo_core::cloud_saves` and the launch options, and whether a sync is
  running now, which the launch's effects set.
- `src/library.rs`: the Library's commands over `zendo_shell::library`: the
  list (every card once), the chips, the search, the sort, the cursor, the
  marks, and the bulk edits. It scans on a thread, watches the launchers'
  files every 10 seconds, and fetches the covers that are missing. Covers are
  files in `$CACHE/zendo/covers`, read through the asset protocol (enabled in
  `tauri.conf.json` with `$CACHE/zendo/**`: the store cards' screenshots and
  achievement icons are in that folder's neighbours).
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
- `game_cards`: a store card's answer is in; the payload is the game's id, and
  the page asks for its cards again.
- `game_changed`: something about a game's data changed (a prefix tool ended,
  the known fixes were read again, a mark, the saves' state); the payload is
  the game's id, and the page asks for its game again.
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

The check needs this one, or `tauri dev`: a plain `cargo build` opens the
window on `devUrl` and shows "Could not connect to localhost" with no dev
server behind it.

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
