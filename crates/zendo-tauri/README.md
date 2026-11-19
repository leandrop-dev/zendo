# zendo-tauri

Zendo's window in Tauri 2, beside the Slint window in `crates/zendo` until the
port's parity checklist passes (`docs/tauri-port-plan.md`).

Slices 0, 1, 2 and 3 are in place: the scaffold, the shell, the Library
(grid, chips, search, sort, cursor, marks, bulk edits, cover downloads and
the file watch), and the whole game page (its header and actions, sessions,
details, local games, its store cards, the Wine prefix card with its tools,
GOG's changelog, the cloud saves row and the Mods card). The other pages
follow.

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
- `src/stats.rs`: the Stats page over `zendo_shell::stats`: the period chips,
  the tiles, the most played, the months and weekdays charts, the highlights
  and the library by status (`stats_get`, `stats_period`).
- `src/game_mode.rs`: Game Mode over `zendo_shell::game_mode`: the shelves, the
  hero, the carousel for the window's width and the Start menu
  (`game_mode_*`: get, sized, toggle, start, shelf, pick, step,
  cycle_shelf, open/close menu, back, menu_step, choose, play, details,
  browse, show). F11, Select and `--game-mode` enter it full screen.
- `src/jobs.rs`: the installed game page's jobs over `zendo_shell::queue` and
  `zendo_shell::dlc`: `game_open` (the page opened: More folded, questions
  forgotten), `game_update`, `game_repair`, `game_check_updates`,
  `game_uninstall` (twice), `game_move` (the folder dialog), `game_toggle_more`,
  `game_dlc` and `game_dlc_all`, and Steam's `steam_verify_game` and
  `steam_uninstall_game`. The page's DTO carries `update`, `manage`, `job` and
  the DLC card.
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
- `src/settings.rs`: Settings' General, Library, Keys and Diagnostics groups
  (`settings_get`, `settings_set_language`, `settings_set_theme`,
  `settings_set_tray`, `settings_set_discord`, `settings_set_covers`,
  `settings_set_auto_update`, `settings_set_workers`, `settings_set_path_first`,
  `settings_default_chosen`, the SteamGridDB and Nexus key fields,
  `settings_download_covers`, `settings_copy_report`, `settings_open`). A
  change shows at once and is written behind one writer; the language and the
  theme follow a successful write. Discord's presence, the Launchers, Engines
  and Proton groups live in `src/presence.rs`, `src/launchers.rs`,
  `src/engines.rs` and `src/protons.rs`.
- `src/accounts.rs`: the Accounts page: the three stores' cards (Epic, GOG,
  Amazon) and their sign-in in the shared `zendo_shell::sign_in_window`
  subprocess or the browser, Steam's card (its token in the keyring) and
  Nexus Launcher's card, plus GOG's two switches. What a sign-in means for the
  Store's lists is slice 5's.
- `src/collections.rs`: the Collections sheet over `zendo_shell::collections`
  and `zendo_core::State` (hand-picked collections and saved views made,
  renamed and deleted; the marked games ticked), opened by the Library's
  COLLECTIONS chip. The sheet is `ui/src/lib/components/collections-sheet.svelte`. About is
  `ui/src/routes/about/+page.svelte`, fed by `bootstrap`'s `version_line` and
  `repository`.
- `src/store.rs`: the Store page over `zendo_shell::store::StoreBrowser`
  (moved there for both windows): the lists from the cache and each signed-in
  store, the chips, search, sorts, Refresh, the cards, and a store game's page
  with its facts and the size, version and DLC its engine says.
- `src/steam.rs`: Steam's installs over `zendo_shell::steam_installs`, read
  on a thread every 15 s (2 s while Steam is busy): its card on Downloads, the
  Install with Steam states of a Steam store game's page, Stop waiting, and
  Open Steam's downloads. Closing the window with a store job or Steam busy
  asks first (`downloads.rs`).
- `src/downloads.rs`: the queue over `zendo_shell::queue`: the queue left from
  last time read at start and begun, the job's thread, a cancelled install's
  files cleared, the library read again after a job, and `downloads_get` /
  `downloads_pause` / `downloads_resume` / `downloads_cancel` /
  `downloads_clear` / `downloads_open`. A store game's Install, Import, Resume
  and "Also install its DLC" are here too (`owned_install`, `owned_import`,
  `owned_resume`, `owned_toggle_dlc`, `owned_forget`), and the page asks for
  it again by `store_changed`, `downloads_changed` and `steam_changed`. The
  updates of the games Zendo installed are asked after each scan, and queued
  by themselves with Settings' switch on; a close while a job runs is held
  back with a question (`downloads_close_anyway`, `downloads_stay_open`). The page follows a running job's
  progress by asking every 500 ms while it runs
  (`ui/src/lib/downloads.svelte.ts`); the top bar counts the queue and the
  window's foot says what it is doing.
- `src/mods.rs`: the Mods card over `zendo_shell::mods`'s state and words:
  `mods_get` and `mods_begin` read it, and each press is a command
  (`mods_toggle_mod`, `mods_move_mod`, `mods_remove`, the plugins, the folder,
  the profiles, `mods_apply`, `mods_add`, `mods_find_nexus`). A change runs
  on a thread and says `game_changed`. Update from Nexus, Search Nexus and
  the FOMOD installer are slice 7's.
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
- `store_changed`: a Store list arrived, a game's size or version was asked,
  or the library changed what is installed; the Store page and a store game's
  page ask again.
- `downloads_changed`: the queue changed (a job queued, started, paused,
  cancelled or ended); the Downloads page and a store game's page ask again.
- `steam_changed`: what Steam is doing changed; the Downloads page, a store
  game's page and an installed game's page ask again.
- `downloads_changed` is also what an installed game's page follows: the job
  line and the buttons (Update, More, the DLC card) change with the queue.
- `downloads_asked`: closing was asked for while a job runs; the frontend
  opens the Downloads page with the question.
- `store_changed` is also sent when the accounts change whose games are
  listed.
- `game_run` and `game_line`: the running game's state (idle, starting,
  running, stopping) and the line under its actions.
- `game_cards`: a store card's answer is in; the payload is the game's id, and
  the page asks for its cards again.
- `settings_changed`: a setting was written (or failed), the covers' fetch
  ended, or the report was copied; the Settings page asks again.
- `accounts_changed`: a store's account was checked, a sign-in or sign-out
  ended, or Steam's sign-in did; the Accounts page asks again.
- `collections_changed`: the Collections sheet opened or closed; the Library
  page asks for it again.
- `language_changed`: the window's language and its messages.
- `game_changed`: something about a game's data changed (a prefix tool ended,
  the known fixes were read again, a mark, the saves' state); the payload is
  the game's id, and the page asks for its game again.
- `game_mode_changed`: Game Mode was entered, left or changed its shelf; the
  frontend asks for the screen again.
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
