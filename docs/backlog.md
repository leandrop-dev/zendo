# Zendo: the backlog

The work left. Items 1 to 20, 27 to 38 and 41 to 49 are built; the detail of each is in
`docs/history.md` under its item number, and the look in `docs/design.md`.

## Next: the port from Iced to Slint

Decided by the user on 2026-11-14 after item 45's spike (`docs/history.md`,
"Item 45, the Slint spike"): a **full port**, Slint under **GPLv3** (COPR
forbids sources under a licence outside Fedora's allowed list, and Slint's
Royalty-free licence is on neither list; item 46), in a **new crate
`crates/zendo-slint` beside the Iced `crates/zendo`**, so `main` stays
releasable. Strings stay in Fluent, handed to Slint through a `Tr` global with
pure callbacks (verified in item 47; a message with two values needs a second
lookup shape, to come with the first page that uses one). One commit per item,
when asked. Keep
logic in `zendo-core`; where `app/*.rs` mixes logic with views, move the logic
to `zendo-core` when that page is ported. Ask before building any other page
`docs/design.md` lists (a Home page was built and removed at the user's wish).

- **49 built 2026-11-14** Game page and what hangs off it, in three slices:
  **49a** (shell, backdrop, Play/Stop and the running badge, status, favorite,
  hide, tiles, sessions, details, local games and Add game), **49b** (Steam
  facts, requirements, DLC, ProtonDB, achievements; the DLC's owned list and
  Install/Uninstall wait for 51) and **49c** (Anti-cheat, the Deck pill, the
  cloud saves row and the syncs' effects, the Wine prefix card, GOG's
  changelog and total time played). Left out on purpose, to land with their
  item: Edit categories (50), Update, the job line, Check/Repair/Move/Uninstall
  and Steam Verify/Uninstall (51), Launch options and Change cover/backdrop
  (52), Discord presence (50), jobs held while a game runs (51). The EOS
  overlay was listed under 49 but is a Settings section in Iced, so it is part
  of 50. Item 54's parity checklist must go through this list.
- **50** Sliced into **50a** (the Settings page: General, Keys, Library and
  Diagnostics groups, built 2026-11-14), **50b** (Engines and Proton groups:
  engines, Protons, runtimes, the Epic overlay section, the Theme choices, the
  Discord switch with presence) and **50c** (Accounts, About, Collections).
  Settings, engines, Protons, runtimes, themes, Accounts, About, the
  Epic overlay section (`app/eos_overlay.rs`), Collections sheet (the
  Library's COLLECTIONS and ADD TO COLLECTION chips come with it; the
  collection and saved-view chips already filter). The language switch there
  must call `tr::show`; the Library re-labels itself.
- **51** Store, store game page, Downloads, installer, Steam installs.
- **52** Mods, Nexus, Nexus search, launch options (and gamescope), artwork.
- **53** Stats tab and Game Mode.
- **54** Cut-over: parity checklist, `zendo-slint` becomes `zendo`, the Iced
  crate goes, packaging (the spec's licence becomes `MIT AND OFL-1.1 AND
  GPL-3.0-only`), README, docs, COPR resubmit.

The full plan, with files and risks, is `~/.claude/plans/drifting-hugging-scone.md`
(outside the repo).

Steam games have no "Launch options" button by design: Steam starts them and
applies its own options ("Manage in Steam" opens its page). `docs/history.md`
item 32.

Built but not yet tried by the user with `cargo build`: items 33 to 36, and
items 28 and 29. Not looked at on screen: a light KDE colour scheme (item
31), pt-PT for item 38, the Store grid, signed-in Accounts, mods and Nexus
(item 33), and a real controller (item 34).

**For the user to try, `zendo-slint` items 49a to 49c** (checked by Claude only
in a throwaway home; what was not checked is in `docs/history.md`). Run
`cargo run -p zendo-slint` with your real library and tick off:

- Game page: Play and Stop on a real game, the running badge, a status chip,
  Favorite and Hide, Add game with the real file dialog (an `.exe` and a
  native program), Edit and Remove on a local game.
- Cards from the network on a Steam game: the ProtonDB pill, About, Details
  rows, screenshots (click one), requirements, achievements with Show more
  (needs a game that has some), a click on the Metacritic row and the pill.
- Anti-cheat: a game on AreWeAntiCheatYet (Rust, 252490) shows the card; its
  site button and a note's link open the browser; a game that is not listed
  has none. Deck pill: Verified, Playable and Unsupported games, and its click.
- Offline: unplug the network, open a game: failed cards say why and Try again
  asks again.
- Wine prefix card on a Windows game Zendo starts (needs umu from Settings →
  Engines): Wine settings, winetricks, Run a program, installing a component
  (the typed text is saved and cleared after success), known fixes list and
  its button, one tool at a time, none while the game runs.
- GOG (signed in): the Changelog card and Show earlier updates; GOG's total
  time played after a session; the Cloud saves row before, during and after a
  real sync (Epic too).
- **Settings (50a)**, with `cargo run -p zendo-slint` (it writes your real
  `config.toml`). Claude already clicked through language, Opaque, a rejected
  key, the Library switches and Copy report (`docs/history.md`, item 50a).
  What needs you: the tray switch (needs the session bus: close the window
  with it on: hides; with it off: quits; turning it off while hidden brings
  the window back); a good SteamGridDB key (Check, then Save keeps it, and
  it is in the keyring), removing a key with an empty field plus Save, "Get
  a key" opening the site; Download now with a key; Open folder and Open
  logs on each row; the switches surviving a restart.
- pt-PT and a light KDE scheme on the game page; a window narrower than the
  page needs; arrow keys and a controller on the Library into a game and back.

Waiting on the user: the COPR resubmit of `main` (item 30). They run
`copr-cli buildscm zendo --clone-url https://github.com/leandrop-dev/zendo.git
--commit main --method make_srpm --nowait` once the redesign is done; the
installed RPM still ships the old ensō icon until then.

## On hold

All decided by the user on 2026-11-13.

- Library views beyond the grid: a list and a details view (was 25, then
  39). Only wanted if items 32 and 34 do not already cover it.
- OptiScaler (was 23, then 40): the user's GPUs (GTX 1070 Ti and 1060) gain
  nothing from it.
- Emulation (was 26, then 37). Decided in that planning [tentative]:
  presets for common emulators plus custom profiles (a program and an
  argument template with `{rom}`); ROMs from folders the user adds, one
  console and one emulator per folder; a fifth tab, Emulation, with a card
  per console, then that console's ROM grid, then the normal game page, with
  the setup (emulators and ROM folders) on the tab; ROMs also under an
  Emulation chip in the Library; the user installs the emulators, as in
  omakade; checked with the RetroArch Flatpak. Ideas from omakade, not yet
  chosen (ideas only, it is GPL): RetroArch playlists as a source, a scan of
  the EmuDeck `~/Emulation/roms/<system>` layout, and covers from Libretro's
  thumbnails.
- Recompiled games (N64 recomps and the like) and their mods (was 21), with
  ideas from Quiver Launcher (checked 2026-11-07: it installs apps from
  GitHub and GitLab releases with community catalogs; Business Source
  License 1.1 from version 4, so ideas only, no code).

## Built since the reformulation of 2026-11-12

Numbers 27 onwards, so they never collide with items 1 to 20 cited in
history.md and the commits.

| # | What | Built |
|---|---|---|
| 27 | Generic platform: tray restore without `hyprctl`, GOG Galaxy without Heroic | 2026-11-12 |
| 28 | The KDE colour scheme as a theme ("System" reads `~/.config/kdeglobals`) | 2026-11-12 |
| 29 | Settings → Engines downloads legendary, gogdl, nile, comet, umu-launcher; Proton-CachyOS beside GE-Proton | 2026-11-12 |
| 30 | Fedora RPM and COPR project `leandrop/zendo` (the RPM bundles no engines) | 2026-11-12 |
| 31 | Redesign shell: top bar, translucent window, JetBrains Mono, icons | 2026-11-12 |
| 32 | Library and game page | 2026-11-12 |
| 33 | Remaining pages: Store, Downloads, Accounts, Settings, sheets, mods, Nexus | 2026-11-12 |
| 34 | Game Mode on F11 / Select | 2026-11-12 |
| 35 | Play status, saved filters, bulk edits | 2026-11-12 |
| 36 | Play sessions and a Stats tab | 2026-11-13 |
| 38 | Steam's description and facts for every game, and a user-given Steam app id | 2026-11-13 |
| 41 | Lucide icons, Downloads and Settings as icon buttons, view chips in the top bar, one title per card | 2026-11-13 |
| 42 | Up Next removed | 2026-11-13 |
| 43 | Collections: categories and saved filters as one list with one set of names | 2026-11-13 |
| 44 | Dependencies optimised in debug builds; 960-pixel copies of heroes | 2026-11-13 |
| 45 | Slint spike of the Library grid: a verdict on eight points, no port | 2026-11-14 |
| 46 | Slint licence checked against COPR (GPLv3 chosen); the toolkit-free modules moved to `zendo-shell` | 2026-11-14 |
| 47 | `zendo-slint` skeleton: window, themes, `Tr`, top bar, tray with size kept, `--game-mode`; the command line moved to `zendo-shell` | 2026-11-14 |
| 48 | The Library in Slint: grid, chips, search, sort, marks, bulk actions, keyboard and pad cursor, cover downloads, file watch; the grid's logic in `zendo_core::browse` | 2026-11-14 |

Item 37 (emulation) is on hold above.
