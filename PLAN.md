# Zendo

A game launcher for Linux: every game in one library, in a normal window with
a full-screen mode for the couch, launched with the right tools, and mods
later. Source files MIT, and so is the binary. Developed on CachyOS
(Arch) with Hyprland and on Fedora with KDE Plasma, next to
[plasmonk-kde](../plasmonk-kde); it began as a sister project of hyprmonk.

## Documents

Read this file first. The rest is read on demand, not up front.

| File | Holds | Read it when |
|---|---|---|
| `PLAN.md` | the direction, the stack, the current state | always, first |
| `docs/backlog.md` | what is next, what is on hold, and a table of the built items 27 to 65 | starting new work |
| `docs/design.md` | the redesign, from the omakade analysis | working on the window's look or pages |
| `docs/history.md` | what was built, when, and what was checked, by item | you need why, or are changing an area whose history matters |
| `docs/reference.md` | verified facts about the stores, the engines and their files | you drive an engine, read a launcher's files or call a store's API |

## Where it stands (2026-11-18)

Phases 1 to 7 and items 1 to 20 are built and mostly tried by the user; the
whole-workspace review of 2026-11-10 is closed. On 2026-11-12 the project
moved to Fedora 45 + KDE Plasma and was redesigned (see "Direction"): items 27
to 36, 38, 41 to 54 and 55a to 60e are built, with tests green
(`docs/backlog.md` has the table, `docs/history.md` the detail and what was
checked). On 2026-11-16 the
development machine moved to CachyOS (Arch) with Hyprland; the window is
distro-agnostic and is supported on both CachyOS/Hyprland and Fedora/KDE
Plasma. Item 37 (emulation), the list and details views, OptiScaler and
recompiled games are on hold.

On 2026-11-17 Battle.net was taken out at the user's request and kept as a
possible future source beside EA app, Ubisoft Connect and others, to look at
again after the first stable release: its client never showed a usable window
under Proton, so no game was read (items 57 and 61 in `docs/backlog.md`).
Zendo's sources are Steam, Epic, GOG and Amazon, Game Pass
and Xbox through Nexus Launcher, Greenlight for the console's screen, Heroic's
sideloaded games, `.desktop` files and games added by hand.

Game Pass installs are Zendo's own since item 62: a game is streamed into
`<games folder>/GamePass`, recorded with a `.zendo-xbox.json` marker and
registered in Nexus Launcher's `launcher-state.json`, so the launcher lists it
and its prepared runner plays it; Zendo uninstalls the ones it installed. The
record write is ADR 0002. Updates stay with the launcher (item 60c).

On 2026-11-18 the window's port to Tauri 2 and SvelteKit began on branch
`f/ui-revamp`: slices 0 (the crates, the SvelteKit app, the Tauri dev loop and
a placeholder page), 1 (the shell: words, colours, tray, controller, keyboard,
full screen and the command line) and 2 (the Library: the grid, chips, search,
sort, views, cursor, marks, bulk edits, cover downloads and the file watch,
with 62 fps on 2,700 games), 3a (the game page's header and actions), 3b
(its sessions and details), 3c (local games), 3d (the store cards: ProtonDB's
pill, Steam's About, facts and screenshots, the requirements, the
achievements and the Steam app id) are built, so
the game page is whole, and slice 3e (the Wine prefix card with its
tools, GOG's changelog, the cloud saves row and the Mods card), slice 4
(Settings' General, Library, Diagnostics, Keys, Launchers, Engines, Proton,
Theme and Discord's presence; Accounts, About and the Collections sheet) and
slice 5 (the Store page, the queue and the Downloads page, Install and
Import, Steam's installs, the updates, the close question, the Store's lists
following the accounts, and the installed game page's Update, job line, More
row and DLC card) and slice 6 (the Stats page and Game Mode) are built, and
slice 7 (the sheets: the screenshot lightbox, the launch options, the art
picker, Nexus's links, downloads and search, and the FOMOD installer with a
mod's Options…) and slice 8, the cut-over, were done on 2026-11-20: the Slint
crate is deleted, the Tauri crate at `crates/zendo` builds the `zendo`
binary, and the packaging, the licence and the docs follow it
(`docs/tauri-port-plan.md`).

- **The window is Tauri 2 with SvelteKit** [firm, cut over 2026-11-20
  (slice 8)]. It replaced the Slint window, which had replaced Iced after the
  item 45 spike; both are in git history. The port's parity checklist is
  `docs/tauri-port-plan.md`, slice by slice, and what the new window says
  differently is in `docs/history.md` under "The Tauri port, slice 8". The
  bundled JetBrains Mono is still the type, and the KDE scheme still comes
  through `zendo-core`. Licence: **MIT** for the source and the binary; the
  GPLv3 of the Slint build went with Slint (item 46), and the font stays under
  the OFL.
- Crates: `crates/zendo` (the binary and the Tauri window, with the SvelteKit
  app in `ui/`), `crates/zendo-shell`
  (toolkit-free: words, tray, controller, links, the sign-in window, the file
  dialog, the command line, and the state of each page's sheets and jobs) and
  `crates/zendo-core` (everything else: library scan, launching, state,
  stores, mods). Keep logic in `zendo-core` or `zendo-shell`.
- Tray: hiding closes to the tray and the tray brings the window back.
- The checklist that gated the Iced to Slint cut-over is
  `docs/parity-checklist.md` (signed off by the user 2026-11-15).
- Releases are built from a `v*` tag by `.github/workflows/release.yml`: a
  GitHub Release (source tarball, checksums, Arch package, Fedora SRPM), a
  pacman repository on GitHub Pages, and a COPR build (ADR 0003). The first
  stable tag was `v1.0.0`; `v1.0.1` is the current release.
- The Fedora RPM builds on COPR `leandrop/zendo`; the spec's licence is now
  `MIT AND OFL-1.1` and its run-time libraries are WebKitGTK's (GTK 3,
  libsoup 3, fontconfig), not Slint's. The frontend is built beside the
  sources for COPR, which has no network. The user
  has built the Slint release on COPR (build 11332151, 2026-11-15, commit
  `90da88a`); the Tauri package has not been built there yet.
- The user closed the "not yet tried" checklist on 2026-11-17: the app has
  been used through development and works, so items 28, 29, 33 to 36 and 41
  to 44 are taken as seen rather than retested one by one.
- On 2026-11-18 a whole-workspace review (Rust and Slint, `docs/history.md`)
  fixed its must and should items, which `v1.0.1` carries: launch logs are
  the owner's alone, HTTP is HTTPS only, the deletion guard refuses `..`, the
  GOG playtime queue is no longer deleted on a parse error, covers no longer
  recount the chips, Stop leaves the event loop, settings writes are ordered,
  and the grid, Stats and Accounts fixes. The three crates share one
  version from `[workspace.package]`.

## Next

Installing the 1.0.1 packages from the new repositories on the family's
machines (Arch: the `[zendo]` pacman repository; Fedora: COPR), then whatever
the user chooses from `docs/backlog.md` (what is on hold is there, and the
review's deferred items are listed after it). The window is Tauri now: a
package from the new build should be made and tried, and the port's on-screen
checks listed in `.mallet/notes/port-checks.md` are still owed.

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

- Generic platform: no compositor-specific code (the old tray restore
  through `hyprctl`, hyprmonk's `tone.json`); Hyprland and KDE Plasma,
  Wayland and X11, all through standard means. Arch/CachyOS and Fedora are
  both first-class: the PKGBUILD and the COPR RPM ship side by side.
- The theme follows the active KDE colour scheme (`~/.config/kdeglobals`),
  which plasmonk sets. Zendo keeps its own colours elsewhere.
- A complete redesign in the style of
  [omakade](https://github.com/btsouth/omakade), first built in Iced, then
  Slint, now Tauri: a top bar with tabs instead of the sidebar, monospace
  type, Home and a controller-first Game Mode. omakade is GPL-3.0-or-later, so
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

Rust, one `zendo` binary with a [Tauri 2](https://tauri.app) window around a
SvelteKit app in `ui/` [firm, cut over 2026-11-20]. Slint held the window from
2026-11-15 to 2026-11-20, and Iced before it; both are in git history. The
port, slice by slice, is `docs/tauri-port-plan.md`. What led to it: on 2,700
covers the Slint window scrolled at 61 to 75 fps (item 48), but its look and
the port's were drifting apart, and the SvelteKit app gave the pages real
layout, text flow and component reuse the toolkit's widgets could not. Also
rejected:

- Quickshell: a toolkit for desktop shells, not apps. Users would need it
  installed, the UI and the command would be two processes talking over a
  JSON file, and Qt 6 has no gamepad module.
- Raw wgpu: a drawing API, not a UI toolkit; text, layout, scrolling, focus
  and input would all be ours to write.
- Qt/QML through cxx-qt: a heavier build and a Qt dependency.

The frontend is SvelteKit 3 in SPA mode (`adapter-static`, `ssr = false`),
Svelte 5 runes, TypeScript and Tailwind CSS 4, with shadcn-svelte's copies
where they help. Controller input is the `gilrs` crate; the KDE colour
scheme, read by `zendo-core`, becomes the window's colours. The window's
words stay Fluent in `zendo-shell`, sent to the frontend at start and asked
for by id after that. The window is opaque: the translucency and blur the
Slint window had are not ported.

Layout: `crates/zendo-core` (library scan, launching, state, config, covers,
stores, mods), `crates/zendo-shell` (what needs no toolkit: words, tray,
controller, links, the helper windows, the command line, and the state and
words of each page's sheets and jobs) and `crates/zendo` (the Tauri shell,
its commands and events, and the binary, whose `list`, `launch`, `hide`,
`unhide`, `covers` and other commands serve keybinds and scripts; the
SvelteKit app is in its `ui/`).

The Library's grid is virtualized by `zendo_shell::browse`'s arithmetic: rows
of fixed-size cards, and the arrow keys know the column count. The selection
is Zendo's own, not the browser's focus, so keyboard and controller move the
same cursor.

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

## On the desktop's side

- KDE Plasma: Zendo follows the KDE colour scheme plasmonk writes (item 28,
  built); nothing is needed there.
- Hyprland: the window's class is `zendo` and floats, and the colour scheme
  falls back to Zendo's own when there is no `kdeglobals`.
- The window is opaque since the Tauri port, so the blur rules the Slint
  window needed (`windowrulev2 = blur, class:^(zendo)$` under Hyprland, and
  the class in plasmonk's `themes/mystical/better-blur-classes.txt` for KDE)
  no longer draw anything; a user who keeps them loses nothing.

## Conventions

As in hyprmonk: commit on main, no attribution lines, commit only when asked.
