# Zendo

A game launcher for Linux: every game in one library, in a normal window with
a full-screen mode for the couch, launched with the right tools, and mods
later. A sister project of [hyprmonk](../hyprmonk), MIT like it.

Status: planned on 2026-11-04 from a hyprmonk session; the whole of phases 1
to 7 built by 2026-11-07, then items 1 to 19 of the list after them, the last
of those (the library's categories) on 2026-11-10, and the 25 must-fix
faults of that day's review closed the same day.

## Documents

Read this file first. The rest is read on demand, not up front.

| File | Holds | Read it when |
|---|---|---|
| `PLAN.md` | the direction, the stack, the current state | always, first |
| `docs/backlog.md` | items 19 to 26, the work left, in order | starting one of them |
| `docs/history.md` | what was built, when, and what was checked | you need why, or are changing an area whose history matters |
| `docs/reference.md` | verified facts about the stores, the engines and their files | you drive an engine, read a launcher's files or call a store's API |

## Where it stands

Phases 1 to 7 are built and checked, mods (phase 7) last on 2026-11-07 and
tried on Fallout 3 GOTY. Of the list after phase 7, items 1 to 19 are built,
most of them tried by the user. Item 19, the library's categories, was built
on 2026-11-10 and tried by the user the same day: it works [firm]. A
whole-workspace review the same day (Rust and Iced) found 25 must-fix
faults, all of them since fixed, the last one (the settings save) after the
user's first tries the same day, which also fixed the category tick boxes'
colour and Cat Quest II's cloud saves; its reports are in
`.mallet/reviews/2026-11-10/`. On 2026-11-11 two quick passes over the
review's non-blocking findings cleared 58 of them, and a third cleared 26
more of the engineering ones (docs/history.md); the 29 left need a try-out
or a decision, bar a few follow-ups on the cards, the clipboard and `view`.

Next is item 20, the window: minimise when a game starts, Discord presence,
rescanning by itself when a launcher's files change. The phase 5a gap is
closed: an Epic game was installed and played through legendary on
2026-11-10 (docs/history.md, phase 5a). The other "not checked" notes sit
with the work they belong to in docs/history.md.

## Direction [firm]

Chosen on 2026-11-06:

- Hikari's functionality: Zendo signs in to Epic, GOG and Amazon itself and
  installs, updates and uninstalls through legendary, gogdl and nile (the
  copies Heroic bundles when they are not installed alone), with a Store
  page of owned games and a Downloads page. Steam's and Heroic's installed
  games keep being imported read-only.
- Hikari's layout, hyprmonk's colours: a sidebar (Library, Store,
  Downloads; Accounts, Settings, About), a top bar with search, store chips
  and sort chips, a game page with a wide hero image, an action bar and a
  details card, and a Settings page (keys, engines, Wine/Proton,
  diagnostics). The palette is Zendo's own until hyprmonk writes
  `tone.json`.
- The controller and F11 full-screen stay, as extras Hikari does not have.
- Order: the new shell first, with the SteamGridDB key entered in Settings;
  then accounts, Store and Downloads; then launch options; then local games
  and mods.

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
| Heroic | Its installed games are imported. From phase 4 also a source of the store engines when they are not installed alone. |
| legendary, gogdl, nile | Phase 4: Epic, GOG and Amazon accounts, owned games and installs. |
| umu-launcher | Windows games installed by hand. Check whether it fetches Proton itself. |
| MangoHud, GameMode, gamescope | Only games where they are switched on. |

## Next

Item 20, the window, then 21 to 26: docs/backlog.md, in the user's firm
order. Each item is planned in detail when it starts.

## On hyprmonk's side

- Install it (package list or the installer) and bind a key (Super+G).
- The tone: Zendo reads `~/.local/state/hyprmonk/theme/tone.json`; what it
  holds and when to write it is in hyprmonk's `docs/zendo-tone.md`. hyprmonk
  writes it since its commit 9f98c4a (2026-11-07); Zendo follows it when
  Settings' Theme is hyprmonk, and keeps its own colours by default.
- hyprmonk's roadmap points here; this file is the plan.

## Conventions

As in hyprmonk: commit on main, no attribution lines, commit only when asked.
