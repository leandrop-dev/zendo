# Zendo

A game launcher for Linux: every game in one library, in a normal window with
a full-screen mode for the couch, launched with the right tools, and mods
later. MIT. Developed on Fedora with KDE Plasma next to
[plasmonk-kde](../plasmonk-kde); it began as a sister project of hyprmonk.

## Documents

Read this file first. The rest is read on demand, not up front.

| File | Holds | Read it when |
|---|---|---|
| `PLAN.md` | the direction, the stack, the current state | always, first |
| `docs/backlog.md` | what is next, what is on hold, and a table of items 27 to 45 | starting new work |
| `docs/design.md` | the redesign, from the omakade analysis | working on the window's look or pages |
| `docs/history.md` | what was built, when, and what was checked, by item | you need why, or are changing an area whose history matters |
| `docs/reference.md` | verified facts about the stores, the engines and their files | you drive an engine, read a launcher's files or call a store's API |

## Where it stands (2026-11-13)

Phases 1 to 7 and items 1 to 20 are built and mostly tried by the user; the
whole-workspace review of 2026-11-10 is closed. On 2026-11-12 the project
moved to Fedora 45 + KDE Plasma and was redesigned (see "Direction"): items 27
to 36, 38, 41 to 53 are built, with tests green (`docs/backlog.md` has the table,
`docs/history.md` the detail and what was checked). Item 37 (emulation), the
list and details views, OptiScaler and recompiled games are on hold.

- Tray: hiding closes the window and the tray opens a new one
  (`iced::daemon`), because on Wayland an app cannot unminimise itself.
- The Fedora RPM builds on COPR `leandrop/zendo`; the user resubmits `main`
  after the redesign is done.
- Not yet tried by the user: items 28, 29, 33 to 36 and 41 to 44 (41 to 44 were
  checked together on screen with the user at the PC; `docs/history.md` says
  what was not).
- **The window is moving from Iced to Slint** [firm, decided 2026-11-14]. The
  spike (item 45) found translucency (any alpha), blur (by window class, as
  in Iced), a bundled font, SVG icons, the KDE scheme through `zendo-core`,
  arrow keys and a real DualSense through gilrs all working; scrolling needs a
  hand-built row model. Licence: Slint under **GPLv3**, because COPR forbids
  sources under a licence outside Fedora's allowed list and Slint's
  Royalty-free licence is on neither list (item 46); the binary is GPLv3, the
  source files stay MIT. Staging: a new crate `crates/zendo-slint` beside the
  Iced `crates/zendo`, shared toolkit-free code in `crates/zendo-shell` (words,
  tray, controller, links, sign-in window, file dialog and the command line),
  the Iced crate deleted at parity (items 47 to 54). Keep logic in
  `zendo-core`. `zendo-slint` exists since item 47: it opens, follows themes
  and the KDE scheme, switches pages and hides to the tray; since item 48 its
  Library page is real (grid, chips, search, sort, marks, bulk actions,
  keyboard and pad cursor, cover downloads), the grid's logic is
  `zendo_core::browse` and the labels are `zendo_shell::format`, both shared
  with Iced. On 2,700 distinct covers it scrolls at 61 to 75 fps where Iced
  holds 3 to 9 (`docs/history.md`, item 48). Item 49, the game page, is built:
  49a (a card opens the game page with Play and Stop, the running badge,
  status, favorite, hide, tiles, sessions and details, local games with the
  Add game sheet; the run logic is shared in `zendo_shell::running`), 49b (the
  ProtonDB pill, Steam's description and facts, screenshots, requirements,
  achievements, DLC installed, the Steam app id) and 49c (Anti-cheat, the Steam
  Deck pill, the cloud saves row, the Wine prefix card, GOG's changelog and
  total time played). Item 50 is built: 50a (the Settings page's General,
  Keys, Library and Diagnostics groups), 50b (the Engines and Proton groups,
  the Theme choices and the Discord switch) and 50c (the Accounts page for Epic,
  GOG and Amazon, About, and the Collections sheet with its chips and the game
  page's collections row). Item 51 is sliced: 51a is built (the Store page
  with its chips, sorts, keys, Refresh and errors; a store game's page, which
  is the game page in an owned mode; Steam's sign-in card on Accounts, with
  its list and achievements; the shared rules moved to `zendo-core` and
  `zendo-shell`), and 51b is built (the Downloads page and the job queue, whose
  rules are `zendo_shell::queue`, shared with Iced; Install, Import, "Also
  install its DLC" and See downloads on a store game's page; the question
  about closing while a job runs; the update checks and the updates by
  themselves). The FOMOD installer sheet, listed under 51b, is a mod installer
  and moved to 52. 51c is built (Steam's installs, followed from Zendo and
  shared with Iced as `zendo_shell::steam_installs`: Install with Steam, its
  progress and the card on Downloads, Verify and Uninstall with Steam; and the
  installed game's page: Update, the job line, More with Check for updates,
  Repair, Move and Uninstall, and the DLC card's Install and Uninstall).
  Item 52 is sliced [firm]: 52a is built (the Launch options sheet with
  gamescope, whose form is `zendo_shell::launch_options`, shared with Iced),
  52b is built (the artwork chooser: Change cover and Change backdrop, whose
  picker is `zendo_shell::artwork`, shared with Iced), and 52c is sliced
  [firm]: 52c1 is built (the Mods card on a game's page: profiles, mods in
  order, plugins, the folder, Apply and Back to vanilla, Add mod and Find on
  Nexus; its state and words are `zendo_shell::mods`, shared with Iced), 52c2
  is sliced again [firm]: 52c2a is built (the Nexus key and link handler in
  Settings, `nxm://` links and the game chooser, downloads with Stop and
  Update on the Mods card, the update check; shared as `zendo_shell::nexus`)
  and 52c2b is built too (Search Nexus: the sheet with results, pictures,
  file lists, Page on Nexus and Get on Nexus; shared as
  `zendo_shell::nexus_search`) and 52c3 is built too (the FOMOD installer sheet,
  whose state is `zendo_shell::installer`, shared with Iced; Options… on a mod;
  the card of mods coming in from Nexus on Downloads), which closes item 52.
  Item 53 is built [firm, sliced in two]: 53a (the Stats page: period chips,
  tiles, most played, charts by month and weekday, highlights and the library
  by status, reading the sessions the game page keeps; arrows change the
  period) and 53b (Game Mode: F11 and Select, the hero and carousel, shelves,
  the Start menu, `--game-mode`; its rules are `zendo_shell::game_mode`). Run
  it as `zendo-slint`.

## Next

Item 54, the cut-over: `docs/backlog.md` has it, and what is on hold.

## Direction [firm]

Chosen on 2026-11-06:

- Hikari's functionality: Zendo signs in to Epic, GOG and Amazon itself and
  installs, updates and uninstalls through legendary, gogdl and nile (the
  copies Heroic bundles when they are not installed alone), with a Store
  page of owned games and a Downloads page. Steam's and Heroic's installed
  games keep being imported read-only.
- (Superseded on 2026-11-12 by the top bar and KDE colours below.) Hikari's
  layout, hyprmonk's colours: a sidebar (Library, Store,
  Downloads; Accounts, Settings, About), a top bar with search, store chips
  and sort chips, a game page with a wide hero image, an action bar and a
  details card, and a Settings page (keys, engines, Wine/Proton,
  diagnostics). The palette is Zendo's own until hyprmonk writes
  `tone.json`.
- The controller and F11 full-screen stay, as extras Hikari does not have.
- Order: the new shell first, with the SteamGridDB key entered in Settings;
  then accounts, Store and Downloads; then launch options; then local games
  and mods.

Reformulated on 2026-11-12 [tentative until the user confirms the order in
docs/backlog.md]:

- Generic platform: no Hyprland-specific code (the tray restore through
  `hyprctl`, hyprmonk's `tone.json`); Wayland and X11 through standard
  means. Fedora first: the RPM through COPR moves up, Arch's package stays.
- The theme follows the active KDE colour scheme (`~/.config/kdeglobals`),
  which plasmonk sets. Zendo keeps its own colours elsewhere.
- A complete redesign in the style of
  [omakade](https://github.com/btsouth/omakade), built in Iced: a top bar
  with tabs instead of the sidebar, a translucent window, monospace type,
  Home and a controller-first Game Mode. omakade is GPL-3.0-or-later, so
  its ideas and look only, no code and no assets (`docs/design.md`). Qt/QML
  stays rejected.
- The store engines (legendary, gogdl, nile, comet) and umu-launcher are
  used from their own releases, with no need for Heroic.
- Recompiled games are on hold. Emulation moved up, then went on hold too
  on 2026-11-13.

## What it should do

- One library of every game: Steam, Epic, GOG and Amazon, and games installed
  by hand (native or Wine). Emulators may come later.
- Launch each game the way it needs: Steam games through Steam, Windows games
  through Proton (umu-launcher) with a prefix per game, and MangoHud, GameMode
  and gamescope switched on per game.
- Covers from SteamGridDB, kept in a cache.
- Driven by keyboard and controller: type to filter, sort by what is played
  most and lately.
- Mods in a later phase: Nexus links, profiles, back to vanilla.
- A normal app window by default, full-screen as an option [firm]. Follows
  the KDE colour scheme when asked (Settings → Theme → System).

## Stack

Rust, one `zendo` binary with an [Iced](https://iced.rs) window [firm].
Chosen over:

- Quickshell: a toolkit for desktop shells, not apps. Users would need it
  installed, the UI and the command would be two processes talking over a
  JSON file, and Qt 6 has no gamepad module.
- Raw wgpu: a drawing API, not a UI toolkit; text, layout, scrolling, focus
  and input would all be ours to write.
- Qt/QML through cxx-qt: a heavier build and a Qt dependency.

Controller input through the `gilrs` crate; the KDE colour scheme becomes an
Iced theme.

Layout [tentative], built this way: `crates/zendo-core` (library scan,
launching, state, config and covers) and `crates/zendo` (the Iced app, plus
`list`, `launch`, `hide`, `unhide` and `covers` commands for keybinds and scripts).

Iced has not reached 1.0 and its API changes between releases. Pinned at
0.14.0 and gilrs at 0.11.2 (the current releases on 2026-11-05). Iced 0.14
has a `Grid` widget, but the grid is laid out as rows of fixed-size cards so
the arrow keys know the column count; the selection is Zendo's own, not
Iced's widget focus, so keyboard and controller move the same cursor.

Call the tools that already exist instead of writing them again:
`steam` (`steam://rungameid/ID`), Heroic's engines legendary (Epic), gogdl
(GOG) and nile (Amazon), umu-launcher, mangohud, gamemoderun and gamescope.
All are run as separate programs, never linked, so their GPL does not reach
Zendo's MIT. Before depending on a tool, check where it is packaged and its
licence. The table of what each is, where it is packaged and under which
licence, and where Heroic keeps its bundled copies, is in
`docs/reference.md` ("Tools and where they come from"), together with every
file format Zendo reads: Steam's and Heroic's, the stores' APIs, SteamGridDB
and Zendo's own files.

## What the user needs installed [tentative]

Nothing is required. Zendo detects each tool and shows what is missing.

| Tool | Needed for |
|---|---|
| Steam | Steam games. The library is read from files, but launching goes through the client. |
| Heroic | Optional: its installed games are imported, and its bundled engines are a fallback. |
| legendary, gogdl, nile, comet | Epic, GOG and Amazon accounts, owned games and installs, and GOG Galaxy. Settings → Engines downloads each from its GitHub release; a copy on `PATH` or Heroic's also works. |
| umu-launcher | Windows games; the zipapp release, also from Settings → Engines (needs `python3`). |
| MangoHud, GameMode, gamescope | Only games where they are switched on. |

## On plasmonk's side

- Zendo follows the KDE colour scheme plasmonk writes (item 28, built); nothing is
  needed there.
- Needed for the blur: Zendo's window class `zendo` in plasmonk's
  `themes/mystical/better-blur-classes.txt` (and in the user's own Better Blur
  DX `WindowClasses`); under Better Blur DX only listed classes blur, and
  without it Zendo is translucent but sharp (docs/history.md, item 31's spike).

## Conventions

As in hyprmonk: commit on main, no attribution lines, commit only when asked.
