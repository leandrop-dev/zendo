# Zendo

A game launcher for Linux: every game in one library, in a normal window with
a full-screen mode for the couch, launched with the right tools, and mods
later. A sister project of [hyprmonk](../hyprmonk), MIT like it.

Status: planned on 2026-11-04 from a hyprmonk session.
2026-11-05: window default, Rust + Iced and optional tools decided; first
session: the repo, the package checks and phase 1 built (see Phases).
2026-11-06: phase 1 checked against a real game from each store; phase 2
built.

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
  hyprmonk's tone when it finds it.

## Stack

Rust, one `zendo` binary with an [Iced](https://iced.rs) window [firm].
Chosen over:

- Quickshell: a toolkit for desktop shells, not apps. Users would need it
  installed, the UI and the command would be two processes talking over a
  JSON file, and Qt 6 has no gamepad module.
- Raw wgpu: a drawing API, not a UI toolkit; text, layout, scrolling, focus
  and input would all be ours to write.
- Qt/QML through cxx-qt: a heavier build and a Qt dependency.

Controller input through the `gilrs` crate; hyprmonk's `tone.json` becomes an
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
Before depending on one, check where it is packaged (Arch repos, CachyOS
repos, AUR) and its licence.

Checked 2026-11-05. All are run as separate programs, never linked, so their
GPL does not reach Zendo's MIT:

| Tool | Packaged | Licence |
|---|---|---|
| umu-launcher | multilib 1.4.4, cachyos 1.4.3 | GPL-3.0-only |
| legendary | AUR 0.21.1 | GPL-3.0-or-later |
| gogdl | AUR `heroic-gogdl` 1.1.2 | GPL-3.0-only |
| nile | no package of its own | GPL-3.0 (Heroic's) |
| gamemode | extra 1.8.2 | BSD-3-Clause |
| Heroic | `heroic-games-launcher` 2.22.4 | GPL-3.0-only |

Heroic bundles all three store engines under
`/usr/lib/heroic-games-launcher/app.asar.unpacked/build/bin/x64/linux/`, so
phase 4 can use those when nile (or the others) are not installed alone.

What phase 1 reads from Heroic, checked in Heroic 2.22.4's own code: under
`~/.config/heroic` (or Flatpak's `~/.var/app/com.heroicgameslauncher.hgl/config/heroic`),
`legendaryConfig/legendary/installed.json`, `gog_store/installed.json` with
titles from `store_cache/gog_library.json`, `nile_config/nile/installed.json`
with titles from its `library.json`, and `sideload_apps/library.json`. Games
start with `heroic://launch?appName=<id>&runner=legendary|gog|nile|sideload`.

`.desktop` files: launchers and tools also file themselves under `Game`
alone (Faugus, Goverlay, Heroic, Lutris, Steam), so no category tells them
apart. `desktop.rs` keeps a list of the known ones, and any entry can be
hidden from the window (phase 2).

What phase 2 reads, checked on 2026-11-06 against Heroic 2.22.4 and a
current Steam client with one game from each store installed:

- Covers. Steam's `appcache/librarycache/<app>/library_600x900.jpg`.
  Heroic's `store_cache/legendary_library.json`, `gog_library.json` and
  `nile_library.json` (top key `library` or `games`) give each game's
  `art_square`, a portrait image on the store's CDN (Epic 1200x1600, GOG
  300x450, Amazon 1080x1440). Heroic's own `images-cache` names files by the
  SHA-256 of the URL, but holds only what Heroic has shown, so Zendo
  downloads into a cache of its own instead.
- SteamGridDB: base URL, `Authorization: Bearer <key>`,
  `search/autocomplete/{name}`, `grids/game/{id}` and `grids/steam/{app}`
  are confirmed from the official Node client's source; the docs page is
  rendered in the browser and could not be read. Which `dimensions` values
  it accepts is not confirmed: Zendo asks for `600x900,342x482,660x930` and,
  on a 400, falls back to the first image taller than wide. Not yet tried
  with a real key.
- Play records. Steam: `LastPlayed` in each `appmanifest_*.acf`, and
  `LastPlayed` and `Playtime` (minutes) under
  `UserLocalConfigStore/Software/Valve/Steam/apps/<app>` in
  `userdata/<account>/config/localconfig.vdf`. Heroic:
  `store/timestamp.json`, `lastPlayed` (ISO-8601, may be empty) and
  `totalPlayed` (minutes) by app name; its store splits keys on dots, so
  Amazon ids (`amzn1.adg.product.…`) are nested objects to join back.

Zendo's own files [tentative]: `~/.config/zendo/config.toml` for what the
user sets (`steamgriddb_key`, `download_covers`), `~/.local/state/zendo/state.json`
for what Zendo remembers (launches, hidden games, the sort), written by the
window and the command alike, and `~/.cache/zendo/covers/` with a
`misses.json` so a game without a cover is asked about again only after a
week. One state file, not omarchy-games-menu's split by owner, since both
sides write the same few things; each write reads, changes and renames the
file into place.

Covers are kept at the size the store serves them. Iced decodes each to
full-size RGBA (the Amazon one is about 6 MB in memory), which is fine for a
few dozen games; for large libraries, scale them down when saving [re-evaluate
once someone has a few hundred games].

## What the user needs installed [tentative]

Nothing is required. Zendo detects each tool and shows what is missing.

| Tool | Needed for |
|---|---|
| Steam | Steam games. The library is read from files, but launching goes through the client. |
| Heroic | Only phase 1's import of the games it installed. Optional from phase 4. |
| legendary, gogdl, nile | Phase 4: Epic, GOG and Amazon games. |
| umu-launcher | Windows games installed by hand. Check whether it fetches Proton itself. |
| MangoHud, GameMode, gamescope | Only games where they are switched on. |

## Ideas from (ideas only, no code copied)

- [Hikari](https://github.com/dodo986/hikari) (GPL-3.0, Go daemon + React
  window): drives legendary/gogdl/nile with credentials of its own; imports
  Heroic's installed games and reads Steam libraries (native, Flatpak, Snap)
  without writing to them; umu/Proton or Wine with a prefix per game;
  DXVK/VKD3D, EAC/BattlEye runtimes; MangoHud, GameMode, gamescope and custom
  wrappers; mods through Amethyst Mod Manager's backend (Nexus, FOMOD,
  `nxm://`); OptiScaler; `logs` and `report` commands and a diagnostics page.
  GPL: read for ideas, never copy.
- [omarchy-games-menu](https://github.com/akitaonrails/omarchy-games-menu)
  (MIT, Rust `ogm` + Quickshell): full-screen grid of covers in sections;
  SteamGridDB covers cached under `~/.cache/ogm/covers` with themed
  placeholders when there is no API key; frecency sort; type anywhere to
  filter; controller support; games found from `.desktop` files with
  `X-OGM-*` keys and TOML catalog fragments in `catalog.d`; update checks
  against GitHub releases and web pages; state split by owner (`state.json`
  for the CLI, `prefs.json` for the UI); a preview of the UI against fixture
  data.

## Phases

1. Library, read-only: Steam libraries (`libraryfolders.vdf`,
   `appmanifest_*.acf`), Heroic's installed games, `.desktop` games. The cover
   grid and launching. Built 2026-11-05, with type-to-filter, keyboard and
   controller moves and F11 full-screen; Steam covers come from Steam's own
   `appcache/librarycache`. On 2026-11-06 `zendo list` found the four test
   games (Hexologic on Steam, Absolute Drift on Epic, Cyberpunk 2077 on GOG,
   Metal Slug X on Amazon) with the right titles. Not yet tried: starting a
   game from Zendo, a real controller, the full-screen toggle live.
2. Covers (the stores' portrait art, SteamGridDB with a key) for the games
   without one, sorting by recent, most played or title (Tab, Y), hiding
   entries (Delete, X; Ctrl+H or Start shows them). Built 2026-11-06; the
   window downloaded the three Heroic covers by itself and sorted the four
   games by last played. Not yet tried: SteamGridDB with a key, and the keys
   in the live window (no key-sending tool here; unit tests cover them).
3. Launch options per game: Proton through umu, prefixes, MangoHud, GameMode,
   gamescope.
4. Stores: sign in, install, update and uninstall through legendary, gogdl and
   nile.
5. Mods.

## On hyprmonk's side

- Install it (package list or the installer) and bind a key (Super+G).
- The tone: hyprmonk writes no file with the active tone's colours yet
  (`~/.local/state/hyprmonk/theme/colors.json` is matugen's wallpaper palette,
  `tone.lua` only the border accent). Add a `tone.json` there for Zendo to
  read into its Iced theme, with its own colours when the file is missing.
- hyprmonk's roadmap points here; this file is the plan.

## Next session

- Start each test game from Zendo, and check it shows up first under
  Recent afterwards.
- Try a controller, the full-screen toggle (F11, Select), and the keys
  phase 2 added.
- With a SteamGridDB key in `config.toml`, run `zendo covers` on a
  `.desktop` game to check the `dimensions` filter.
- Start phase 3.

## Conventions

As in hyprmonk: commit on main, no attribution lines, commit only when asked.
