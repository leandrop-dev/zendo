# Zendo: the backlog

The work left. Items 1 to 20, 27 to 38, 41 to 54 are built; the detail of each is in
`docs/history.md` under its item number, and the look in `docs/design.md`.

The work next is the port from Slint to Tauri 2 and SvelteKit, on branch
`f/ui-revamp` and in slices, in `docs/tauri-port-plan.md` (slice 0, the
scaffold; slice 1, the shell; slice 2, the Library; slices 3a, 3b, 3c, 3d and
3e-1, the game page's header, sessions, details, local games, store cards and
Wine prefix card, all built 2026-11-18).
The items below stay as the reference for what the port has to reach.

## The port from Iced to Slint (built 2026-11-14 to 2026-11-15)

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
  Diagnostics groups, built 2026-11-14), **50b** (built 2026-11-14: the Engines
  and Proton groups with engines, Protons, runtimes and the Epic overlay, the
  Theme choices and the Discord switch with presence) and **50c** (built
  2026-11-14: the Accounts page for Epic, GOG and Amazon, About, and the
  Collections sheet with the Library's COLLECTIONS and ADD TO COLLECTION chips
  and the game page's collections row). Left in 50b for 51: an engine is also
  busy while a store job runs (Iced's `engines_busy`), done in 51b. What 50c left for 51 (asking a store for its games after a sign-in
  or check, the Store forgetting a list on sign-out, Steam's card whole with
  its achievements and the "n of 4" count) was done in 51a.
- **51** Sliced with the user into three. **51a built 2026-11-14**: the Store
  page, a store game's page, Steam's card on Accounts, and the hooks 50c left.
  **51b built 2026-11-14**: the Downloads page and the queue (shared with Iced
  as `zendo_shell::queue`), Install, Import, Resume, See downloads and "Also
  install its DLC" on the store game's page, the question about closing while
  a job runs, the engines busy while a job runs, the updates by themselves and
  the update checks after a scan. The "installer sheet" that was listed here is
  the FOMOD mod installer: it moved to 52 [firm, decided with the user]. **51c
  built 2026-11-15**: Steam installs (Install with Steam, Ask Steam again, Stop
  waiting, its progress and the Steam card on Downloads, shared with Iced as
  `zendo_shell::steam_installs`), and the installed game's page: Update, the
  job line with See downloads, More with Check for updates, Repair, Move… and
  Uninstall (pressed twice), Verify and Uninstall with Steam, the Updates row,
  and the DLC card's Install, Install all and Uninstall (pressed twice). The
  jobs held while a game runs were 51b's. Item 54's parity checklist must go
  through all of 51.
- **52** Sliced with the user into three [firm, 2026-11-15]. **52a built
  2026-11-15**: launch options with gamescope (the form is
  `zendo_shell::launch_options`, shared with Iced; the Slint sheet is
  `ui/launch_options.slint` and `options_ui.rs`; the lists are chips, not
  dropdowns). **52b built 2026-11-15**: the artwork chooser (Change cover and
  Change backdrop; the picker is `zendo_shell::artwork`, shared with Iced; the
  Slint sheet is `ui/artwork.slint` and `artwork_ui.rs`; Iced's `backdrops.rs`
  needed no port, Slint already drew the backdrop). **52c** is sliced [firm]:
  **52c1 built 2026-11-15**: the Mods card on a game's page (the state, the
  jobs and the words are `zendo_shell::mods`, shared with Iced; the Slint card
  is `ModsCard` in `game.slint`, `ui/mods.slint` and `mods_ui.rs`).
  **52c2** is sliced again [firm, 2026-11-15]: **52c2a built 2026-11-15**:
  Nexus (the key and the link handler in Settings; `nxm://` links, the game
  chooser, downloads with progress and Stop, Update on a mod and the update
  check after a read; the state, transitions, workers and words are
  `zendo_shell::nexus`, shared with Iced; the Slint side is `ui/nexus.slint`
  and `nexus_ui.rs`). **52c2b built 2026-11-15**: Search Nexus (the shared
  state and words are `zendo_shell::nexus_search`, with Iced switched first;
  the Slint sheet is `ui/nexus_search.slint` and `search_ui.rs`; Page on
  Nexus, Get on Nexus and a Premium Download were not seen on screen).
  **52c3 built 2026-11-15**: the
  FOMOD installer sheet (the state, the rules and the words are
  `zendo_shell::installer`, shared with Iced, with a `Work::Install` in
  `zendo_shell::mods`; the Slint sheet is `ui/installer.slint` and
  `installer_ui.rs`), Options… on a mod, the slot for a waiting installer, and
  the card of mods coming in from Nexus on Downloads. Not seen on screen: the
  waiting slot, the Downloads card, the pad, pt-PT (tests cover the first
  three). Item 54's parity checklist must go through all of 52; for 52a: a failed save on a field that is scrolled
  out of sight says nothing at the footer (Iced's sheet is the same); for
  52c1: Iced shows "Reading mods…" while Slint draws no card until they are
  read, and on Stardew Valley's page the bottom of the Mods card could not be
  scrolled to once (`docs/history.md`, item 52c1; the pages' content height
  flips on every rebuild, with or without the card).
- **53** Stats tab and Game Mode, sliced in two [firm]. **53a built
  2026-11-15**: the Stats page (`ui/stats.slint`, `stats_ui.rs`; the sums and
  words were already shared). **53b built 2026-11-15**: Game Mode (F11 and
  Select, the hero and the carousel, shelves with Tab and LB/RB, the Start menu,
  Details, the top bar's GAME MODE chip, `--game-mode`; the rules are
  `zendo_shell::game_mode` (Iced's `app/game_mode.rs` and `hero.rs` were not
  switched to it and go at the cut-over), the Slint side is `ui/game_mode.slint`
  and `game_mode_ui.rs`). Not seen: a real controller, a hero with a Steam
  summary, a light scheme. Item 54's parity checklist must go through 53.
- **54 built 2026-11-15** Cut-over, sliced in two with the user. **54a**: the
  parity checklist (`docs/parity-checklist.md`: the 11 messages only Iced used
  and what the Slint side did for each; the update badge, the "game gone"
  notes, the per-source library problems, "No images to download", the panic
  text and the Stats hint were ported, "Reading mods…" left out on purpose;
  seen on screen except the update badge). **54b** (signed off by the user): the
  Slint crate became `crates/zendo` (the old `zendo-slint`), the Iced
  `app.rs` and `app/` were deleted with `iced` from the workspace, the four
  messages nothing used were dropped from both `.ftl` files, the spec's
  licence became `MIT AND OFL-1.1 AND GPL-3.0-only` and its run-time libraries
  and the PKGBUILD's became Slint's (EGL, GL, fontconfig, Wayland, xkbcommon,
  X11; no Vulkan), and the README, PLAN.md and the module comments were
  brought up to date. The RPM mock build passes (it needed `rustflags_debuginfo 1` and 16 codegen units in the spec: with Fedora's defaults rustc was OOM-killed at 17 GB). Left: the COPR resubmit (the user's, below).

The full plan, with files and risks, is `~/.claude/plans/drifting-hugging-scone.md`
(outside the repo).

Steam games have no "Launch options" button by design: Steam starts them and
applies its own options ("Manage in Steam" opens its page). `docs/history.md`
item 32.

Built but not yet tried by the user with `cargo build`: items 33 to 36, and
items 28 and 29. Not looked at on screen: a light KDE colour scheme (item
31), pt-PT for item 38, the Store grid, signed-in Accounts, mods and Nexus
(item 33), and a real controller (item 34).

**For the user to try, the Slint window's items 49a to 49c** (checked by Claude only
in a throwaway home; what was not checked is in `docs/history.md`). Run
`cargo run -p zendo` with your real library and tick off:

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
- **Settings (50a)**, with `cargo run -p zendo` (it writes your real
  `config.toml`). Claude already clicked through language, Opaque, a rejected
  key, the Library switches and Copy report (`docs/history.md`, item 50a).
  What needs you: the tray switch (needs the session bus: close the window
  with it on: hides; with it off: quits; turning it off while hidden brings
  the window back); a good SteamGridDB key (Check, then Save keeps it, and
  it is in the keyring), removing a key with an empty field plus Save, "Get
  a key" opening the site; Download now with a key; Open folder and Open
  logs on each row; the switches surviving a restart.
- **Settings, Engines, Proton and Theme (50b)**, same run, also writes your
  real `config.toml` and downloads into your real folders. Claude already
  clicked through Theme (Dusk, a broken theme file, System following a
  `kdeglobals`), Opaque and Discord's switch, the engine list and a real
  legendary download and Remove, Anti-cheat's list, the default Proton, a
  seeded Proton's Remove and the release lists (`docs/history.md`, item 50b).
  What needs you: Discord presence with Discord open (the status shows a
  game while it runs and clears when it ends); the Update button (an older
  engine); Anti-cheat Download, Update and Remove; the Epic overlay's Install
  and Remove (a large download through legendary, signed in or not); Stop on a
  download (catch one on a slower link); the Engines buttons greyed while a
  game runs; Settings in pt-PT; your own KDE scheme as System, light and dark.
- **Accounts, About and Collections (50c)**, same run. Claude already clicked
  through every new control against fake engines (`docs/history.md`, item 50c);
  not tried: Add game's Cancel keeping the keys after it closes (fixed like the
  others), and a Collections sheet with a controller.
  What needs you, with `cargo run -p zendo` and your real engines: a real
  sign-in to Epic, GOG and Amazon (the window loads the store's page and hands
  the code back by itself; Use my browser instead, then paste the page or
  address; Amazon's page is made by nile), a code pasted with Ctrl+V anywhere on
  the page, Sign out of each, the two GOG switches with a real account (time
  played goes to GOG; Galaxy starts GOG games), the sheet with a controller
  (only B closes it), Accounts, About and the Collections sheet in pt-PT, and
  F1 and Ctrl+4 / Ctrl+6.
- **The Store, a store game's page and Steam's card (51a)**, same run. Claude
  already clicked through the Store's chips, sorts, keys, search field,
  Refresh and errors card, a store game's page with its pills and trail,
  Open in library, the sign-in-first state, and Steam's card (Sign out, Sign
  in's window, Cancel, Refresh with a token Steam refuses) against a seeded home
  and fake engines
  (`docs/history.md`, item 51a). What needs you, with your real accounts: the
  Store after a real sign-in to Epic, GOG and Amazon (the lists, their covers,
  Refresh); a real Steam sign-in on Steam's page (and Steam Guard), the list
  that follows with your family's games marked "Shared by your family", the
  family line on the card, and a restart that keeps you signed in; a store
  game's page with a real engine (sizes, "needs …", the DLC you own, the
  latest version); the Store in pt-PT; the pad on the grid (A opens, Y sorts);
  Ctrl+F, Ctrl+V and Ctrl+R on the Store.
- **Downloads, Install and Import (51b)**, same run. **Careful: a real
  `cargo run -p zendo` now resumes the queue left in
  `~/.local/state/zendo/downloads.json` (jobs not paused start at once), asks
  your stores about the games Zendo installed, and, with "Update games by
  themselves" on (the default), queues and runs the updates it finds.** Claude
  already clicked every control of the Downloads page and of a store game's
  page against a fake `legendary`, in two runs, and the same Resume, a finished
  job and the saved queue in the Iced window (`docs/history.md`, item 51b).
  What needs you, with a real engine: a real install of a game from your Epic
  list, one you do not mind downloading (Zendo's page says "needs …" before you
  press Install; Settings → Library says where it goes), and one that has
  installable DLC, which the cache cannot tell, only the engine's answer when
  the page opens: "Also install its DLC (n)" shows only then, ticked and
  unticked; Pause and Resume mid-download, Cancel (the folder should be gone),
  a failure (unplug the network), Close anyway and the queue still there next
  time; Import with a game you already have on disk; the same for GOG and
  Amazon (not checked at all on screen); the pad (A, B) on the question about
  closing, and the tray's Quit with a download running (needs the session bus);
  a game's update while it plays (the job waits, "Waits until the game is
  closed"); Downloads in pt-PT and with a light KDE scheme. The game page's
  Update, Check, Move and Uninstall are 51c's, below.
- **Steam installs and the game page's jobs (51c)**, same run. **Careful: a
  real `cargo run -p zendo` asks your real Steam client** (Install with
  Steam opens Steam's own dialog, Verify and Uninstall with Steam ask Steam;
  Uninstall with Steam and a game's Uninstall, Repair and Move touch real
  files). Claude already clicked every new control against fakes of `steam`,
  `xdg-open` and `legendary` and seeded Steam manifests (`docs/history.md`,
  item 51c). What needs you, with the real clients: Install with Steam on a
  game you do not own yet on this PC (Steam's dialog opens; cancel it and Ask
  Steam again or Stop waiting; install it and watch the page and Downloads
  follow Steam's own bar, pause and resume it in Steam, and a download you start
  in Steam itself shows too); Verify with Steam and Uninstall with Steam on a
  game you can spare (the footer says so when Steam finishes); on an Epic or
  GOG game Zendo installed: Update (when one is found), Check for updates,
  Repair, Move… to another disk (the files and the record move; check that
  the game starts), Uninstall (the second press), and the DLC's Install, Install
  all and Uninstall on a game that has some; the buttons off while a job runs;
  the same in pt-PT and with a light KDE scheme. One thing from the check to
  look at: `move_game` (`zendo-core` `downloads.rs`), shared with Iced, deletes
  the old folder when the record already names the new one without checking
  that the new folder exists; I could only reach it with a broken fake.
- **Launch options (52a)**, same run. It writes your real `games.toml` for the
  games you save. Claude already clicked every control against four seeded
  games and read the options file (`docs/history.md`, item 52a), and opened and
  saved Iced's sheet once. What needs you: a game started with the options
  (a real Proton run with MangoHud, GameMode, gamescope with a size and a
  filter, FSR, a frame cap, arguments, environment and scripts before and
  after: does each do what it says); a real sync with Sync now, Upload and
  Download on an Epic or GOG game with cloud saves; Browse with the real file
  dialog on a game of yours; a Steam Linux Runtime and a Proton you installed
  in the lists; the sheet in pt-PT and with a light KDE scheme and with a
  controller (the pad only closes it). One thing from the check: a Save that
  fails on a field out of sight (the gamescope sizes, with the form scrolled to
  the bottom) says nothing at the footer, as in Iced.
- pt-PT and a light KDE scheme on the game page; a window narrower than the
  page needs; arrow keys and a controller on the Library into a game and back.

The COPR resubmit of `main` is done: the user ran `copr-cli buildscm
leandrop/zendo --clone-url https://github.com/leandrop-dev/zendo.git --commit
main --method make_srpm --nowait` on 2026-11-15 (commit `90da88a`), and build
11332151 succeeded (about 40 minutes). The RPM was not installed: on
2026-11-16 the machine moved to CachyOS, so the Arch PKGBUILD is the package
used there (`makepkg -si`, README). The COPR build stays for Fedora.

## From the user's testing of 2026-11-16

The user ran the Slint window against their real library and stores and sent a
list of bugs, improvements and new sources. The order is their choice [firm]:
the bugs first, then the improvements, then the new sources. Numbers 55 to 59,
so they never collide with items 1 to 54.

### Bugs (55a to 55c)

55a to 55c are built; 56a to 56c are built; 57a and 57b are built; 58a to
58c are built; 59 is built; 60a to 60e are built; 61 and 62 are next.

- **55a built 2026-11-16, the layout fixes** (five, all small, all in the
  Slint window):
  1. The footer floats under the empty Library and Store instead of sitting at
     the bottom: the empty-state `Rectangle` in `ui/library.slint` and
     `ui/store.slint` is only as tall as its text. Give it
     `vertical-stretch: 1`.
  2. The view chips (All, Favorites, Recent, Hidden) sit beside the tabs in
     `ui/top_bar.slint`; `docs/design.md` puts them beside the search. Move
     them to the right cluster, left of the search field.
  3. The `sort` label is not centred against the chips, and neither is the
     `organize` label: both are bare `Text` in a `FlexboxLayout` with
     `cross-axis-alignment: start` (`ui/library.slint`, `ui/store.slint`).
     Give each the chip's height and centre it.
  4. The Store's sort chips are on a row of their own; the Library's share
     their row with the source chips. Merge them in `ui/store.slint`.
  5. The Technical card's `+`/`−` button is under the heading, not on it
     (`ui/game.slint`). Put it on the heading row, right-aligned, and make the
     heading press it.
- **55b built 2026-11-16, the tray** (two, in `zendo-shell` and the window):
  6. "Only minimize to tray when the game window opens": for a game Zendo
     starts itself (`Watch::Group`), `HideOnLaunch` fires on `Started`, before
     the game's window is up (`zendo-shell/src/running.rs`). Hide when the
     window is actually mapped instead, reusing the window detection Stop
     already uses. Today the Play button's change and a failure are hidden too.
  7. "When the game closes the Zendo window should unminimize": there is no
     such path. Add an effect (`ShowOnClose` or `ShowWhenIdle`) in
     `running.rs` and handle it in `game_ui.rs`, beside `HideOnLaunch`.
     (On Wayland the window is hidden, not minimised, so this is a show.)
- **55c built 2026-11-17, the game page's scroll**: the known item 52c1 bug
  (`docs/history.md`): the page's content height flipped, the Screenshots card
  especially, and the `ScrollView` kept a stale height, so the bottom could not
  be reached. The user: "sometimes we have too much scroll, sometimes too
  less ... aren't we able to make the scroll fit content?" The cause was
  found on 2026-11-17: a card's wrapped text is measured at the width the
  layout hands it during the layout pass, not at the width it is drawn at, and
  a width assigned in that pass does not re-measure the page above it. Two
  changes in `ui/game.slint` fix it: each card in the two columns takes its
  column's width as a binding, and the scroll's `content-height` is the page's
  preferred height. The user ran `cargo run --release` and went through the
  pages: better. Not tried: a narrow window, pt-PT, a light scheme.

### Improvements (56a to 56c)

- **56a built 2026-11-16, the screenshot lightbox**: a screenshot on a game
  page opened its full image in the browser (`GameData.open-path(shot.full)`
  in `ui/game.slint`). It now opens in the app: `ui/shots.slint`
  (`ShotSheet`), one full picture over the page with previous, a count and
  next. The full-size files are downloaded beside the thumbnails.
- **56b built 2026-11-16, the Store's platform badge**: a badge in the
  top-left of a Store card with the store the game belongs to (the mark box
  is only on the Library's cards). `CardData.platform` carries it.
- **56c built 2026-11-16, the wide art's fade**: the game page's backdrop
  was a flat `opacity: 0.22` (`ui/game.slint`). It now fades in at the top
  edge and out towards the bottom, as omakade does.

### New sources (57 to 59)

Read from omakade's docs and source on `main` on 2026-11-16 (`docs/GUIDE.md`,
`docs/COMPATIBILITY.md`, `src/sources/battlenet/`, `src/sources/xodus/`,
`src/launch/GameLauncher.cpp`). Ideas and formats only, as everywhere else.
The verified contracts are in `docs/reference.md`, "New sources: Battle.net,
Nexus Launcher, Xbox streaming". Sliced with the user on 2026-11-16.

- **57, Battle.net** [removed 2026-11-17; a possible future source]. Built as
  a read-only import (57a), the installer in Settings and the sign-in card
  (57b), and Zendo's own prefix moved to Proton after the client crashed in
  stock Wine (58c), then taken out whole at the user's request: the client
  started under Proton but its window stayed blank (item 61), so no game was
  ever imported from a real installation. Gone: `zendo-core/src/battlenet.rs`,
  `Source::BattleNet`, `Launch::Prefix` and `WineRunner` (nothing else used
  them), `launch::in_prefix`, the Settings → Launchers row, the Accounts card,
  the Settings → Sources row, the report's count and the `battlenet-*`
  messages in both `.ftl` files (`docs/history.md`, "Battle.net taken out").
  The code is in git history up to the commit before the removal; the verified
  formats and commands stay in `docs/reference.md`, and the window problem and
  its candidate fixes stay in item 61.
- **58, Game Pass through Nexus Launcher** [firm, sliced 2026-11-16].
  `domsvoboda/nexus-launcher` (MIT, formerly `xodus-nexus`). Read-only:
  `launcher-state.json` games with `installState == "ready"`, launched through
  Nexus's own `shortcuts/launch-game <id>`. Sliced in two:
  - **58a built 2026-11-17**: the import and the launch.
    `zendo-core/src/xodus.rs`, `Source::Xbox` (nine sources then, eight
    now), the script's
    `# X-Xodus-Nexus-Profile=` marker checked before anything starts, and the
    chip (from `Source::ALL`), the report and Settings rows. **The art is the
    launcher's own**: the record's inline `coverDataUrl` (a `data:` URL, which
    `covers::download` now reads) and its `heroUrl`. Checked in a throwaway
    home with a fake `launch-game` through `zendo list` and `zendo launch`;
    not seen: the window, a real launcher and game, pt-PT.
    **Recorded for later (option 2)**: Microsoft Store art from
    `displaycatalog.mp.microsoft.com` by `storeId` (`Poster` at `w=600`,
    `SuperHeroArt` else `TitledHeroArt` at `w=1920`), if the inline picture
    proves not enough or of a lesser quality. It needs one request per game,
    so the scan does not do it.
  - **58b built 2026-11-17**: the download and the sign-in.
    `xodus::release()`/`install()` bring the AppImage for this machine from
    GitHub (its `sha256` digest checked, `engines::latest_asset`), into
    `$XDG_DATA_HOME/zendo/launchers/Nexus.AppImage`, executable;
    `xodus::open()` starts it. Settings → Launchers has its row (Download
    launcher → Open Nexus Launcher, with the bar and Stop), and Accounts has
    an XBOX card that opens it and shows the games its profile lists. Checked
    in a scratch home with the real v0.5.0 AppImage (download, digest,
    0755, `--appimage-version` runs); the user's first look found one note
    drawn three times, fixed (each row keeps its own note now); not seen: the
    rows and the card on screen after that, the bar, and the launcher's own
    window with its runtime and sign-in.
- **59 built 2026-11-17, Xbox Series S**. Cloud gaming is 58. Remote play is a
  streaming client, not a library source: **Greenlight** (MIT,
  `unknownskl/greenlight`) streams the console's own screen and lists no
  games. The slice, as recorded, is the thin entry that opens it, and Zendo
  **brings its AppImage** (the user chose it over the Flatpak): 
  `zendo-core/src/greenlight.rs` takes `Greenlight-<version>.AppImage` from the
  latest GitHub release, checks its digest and puts it in Zendo's launchers
  folder (`engines::bring`, the machinery Nexus Launcher shares); a
  Flatpak or a `greenlight` on `PATH` is used just as well. Settings →
  Launchers has the row: Download Greenlight (bar and Stop) → Open Greenlight.
  No `--connect`: the console id is Greenlight's to know (its README says a
  console's is a 16-hex id), so its window carries the list. **xbPlay is
  Flathub-archived and end-of-life, with a copyright dispute, so it is not
  used.** Checked in a scratch home with the real v2.4.2 AppImage (digest,
  0755, `--appimage-version`); not seen: the row on screen, Greenlight's own
  window and a console streamed.

- **60, Game Pass from Zendo** [firm, 2026-11-17]. The user's question after
  58b: can Zendo show the Game Pass games on offer and install them, rather
  than only read what Nexus Launcher installed? It can, on the launcher's own
  backend, which is signed in already (checked 2026-11-17, the same machine):
  `~/.config/xodus-nexus/xodus-backends/current.json` names `xodus-cli`
  (v0.2.15), whose `library --json` answers the account's entitlements
  (`productId`, status, dates) and whose `streaming <source> <dest>` downloads
  and extracts a game with `--progress-json` and pause/resume over stdin; the
  `productId` is the Microsoft Store id that option 2's art is keyed by
  (`docs/reference.md`, "Nexus Launcher"). Sliced, to do with the user:
  **60a built 2026-11-17**, the owned list: `xodus::owned()` reads the
  backend's entitlements and the Store's catalogue (titles, publishers, short
  descriptions, `Poster` at `w=600` and `SuperHeroArt` at `w=1920`), keeps the
  games and leaves the add-ons out, and the Store page lists them under an
  Xbox chip with the stores' cards, sorts, search and cover fetch. Checked
  against the user's account: 22 games of 36 entitlements, in 1.9 s; not seen
  on screen.
  **60b built 2026-11-17**: the page's facts and the hand-off.
  `xodus::package` runs `resolve-package` (the desktop build's `supported`,
  `status`, `downloadBytes` and format), and a Game Pass game's page says
  where it goes ("Install it in Nexus Launcher; it needs 2.1 GB…", or "The
  Store has no install of it for this PC"), with an **Open Nexus Launcher**
  button. Checked against the real backend (Minecraft installable, Cuphead
  not); not seen on screen. **60c closed 2026-11-17: the CLI allows neither.**
  It has no uninstall or update command, `resolve-package` gives no version,
  and the Store catalogue's package `Version` is `"0"`; the launcher's
  `launcher-state.json` (with `installedVersion` and
  `downloads[].selectedContentId`) is already watched, so what the launcher
  installs, updates or removes is rescanned. The facts are in
  `docs/reference.md`, "Nexus Launcher". **The open question, for a piece of
  its own**: installing *through* Zendo. `xodus-cli streaming <product> <folder>` exists (progress JSON,
  pause/resume over stdin) but needs the launcher's runtime, and the CLI
  expects the launcher's own package selection (`download` refuses without
  it); an install made behind the launcher's back is not in
  `launcher-state.json`, so Zendo could neither list it nor start it with
  `launch-game <id>` (`run <path> <wine>` is a second path to design). One
  small title installed with `streaming` would settle whether the state gains
  it. **Settled on 2026-11-17** (`docs/history.md`, "The Game Pass install
  experiment"): `streaming` works and its progress protocol is clear, but
  `launcher-state.json` is not touched, and without that record Zendo can
  neither list the game nor start it through `launch-game`; `xodus-cli run
  <folder> <wine>` is the other way, and needs the runtime's own Wine (the
  system Wine 11.22 page-faulted). See item 62.
- **62 built 2026-11-17, installing Game Pass games from Zendo.** The user's
  call, after 60c. Zendo streams a Store package with
  `xodus-cli streaming <product> <folder> --progress-json` into
  `<games folder>/GamePass/<title>` through the one download queue (a new
  `Shop::Xbox` job; the queue's `Job` now names its `Shop`, with a serde alias
  for old `downloads.json` files), checks free space first, and writes a
  `.zendo-xbox.json` marker (product, title, publisher, package, version,
  executable, bytes, cover and hero URLs). Launch and compatibility are the
  launcher's: the marker is registered as a record in
  `launcher-state.json` (ADR 0002), so Nexus Launcher lists the game, prepares
  `compatdata/<id>/xodus-proton-game` and plays it through `launch-game <id>`;
  writing the state requires the launcher closed (`SingletonLock`), and
  Zendo uninstalls the games it made by removing the folder, the record and
  that `compatdata` folder. **Seen live with the user**: Retro Classics and
  Roblox installed, listed by the launcher and uninstalled through Zendo;
  Minecraft Legends, installed by Zendo after the launcher's copy was
  removed, played through Zendo. Retro Classics and Roblox crash inside
  vkd3d-proton under the launcher itself (`vkEndCommandBuffer Exception
  0xc0000005`, exit 3), so those titles do not play on this machine yet.
  Out of scope: updates (60c), DLC, MangoHud/gamescope wrappers, and a game
  installed by hand into `GamePass` without a marker (not Zendo's to remove).
- **60d built 2026-11-17, the packages on the cards**: when the Game Pass list
  arrives a thread asks `resolve-package` for each game, so a card shows an
  UNSUPPORTED badge when this PC cannot install it, without the page being
  opened (and the page's size is instant then). Not seen on screen.
- **60e built 2026-11-17, Game Pass achievements**: `xodus::title_id` reads
  `<TitleId>` from an installed game's `MicrosoftGame.config` (hex, asked in
  decimal) and `xodus::achievements` maps the backend's items into the page's
  Achievements card, unlocked first, secrets kept back, "n of m" right, with
  their pictures downloaded into the achievement icons' cache. Checked with
  Minecraft Legends (40 achievements, 40 icons); not seen on screen. `game-activity` (time played and last activity) is still unread and
  could fill a Game Pass game's time played.
- **61, Battle.net's empty window under Proton** [withdrawn 2026-11-17 with
  item 57; kept for when Battle.net returns]. From the user's testing,
  2026-11-17: the client started (58c) and showed its window, but the window
  was blank. The log
  (`$XDG_STATE_HOME/zendo/logs/battlenet.log`) says Proton upgraded the
  plain-Wine prefix to CachyOS-11.0-100, that ProtonFixes found no title for
  `umu-battlenet` ("Game title not found in CSV", "No main stage global
  protonfix"), that the game drive's parent (/home) could not be used, and
  ends with "Executable is a unix path, launching with 'umu.exe'" and "ntsync:
  up and running" with no error. What to try, one at a time, each a small
  change to the environment such a launch builds (`in_prefix` before item 57
  removed it): `PROTON_USE_WINED3D=1` (the client's
  Chromium window often needs software D3D), `PROTON_ENABLE_CEF_SANDBOX=0`-ish
  flags to the executable (`--no-cef-sandbox`), a GE-Proton instead of
  CachyOS's, and `Battle.net.exe` started with `-d3d9`/`-d3d11`. The client's
  own window needs a real check each time (58c's "not seen").

## Possible future sources (after the first stable release)

Decided by the user on 2026-11-17: Zendo keeps to the sources it has (Steam,
Epic, GOG and Amazon through Heroic, Game Pass and Xbox through Nexus
Launcher, Greenlight, Heroic's sideloaded games, `.desktop` files and games
added by hand) and their work. Battle.net, EA app, Ubisoft Connect and others
are looked at again after the first stable release, once the user has
investigated them first.

- **Battle.net**: built, then taken out on 2026-11-17 (item 57). Its client
  started under Proton but showed a blank window, so no game was ever read
  from a real installation; the candidate fixes are in item 61. The verified
  facts about the Agent's `product.db`, the client's files, the launch codes
  and the installer are kept in `docs/reference.md`, and the code is in git
  history up to the commit before the removal.
- **EA app**: no Linux client; EA games arrive through Steam or Heroic, which
  Zendo already imports. Not investigated beyond that.
- **Ubisoft Connect**: the same: no Linux client, and Ubisoft games come
  through Steam or Heroic. Not investigated.
- **Others**: the user brings them, and their names are not recorded here
  until they are looked at. What makes one worth a slice: a launcher that runs
  on Linux or under Wine/Proton, a library file Zendo can read without
  writing, and a launch command; a launcher with none of those is no source at
  all.

Emulation and RetroAchievements stay on hold (item 37).

## Release packaging (built 2026-11-17)

- **63, the release system** [built 2026-11-17]. `.github/workflows/ci.yml`
  runs fmt, clippy with `-D warnings` and the workspace tests on every push
  and pull request. `.github/workflows/release.yml`, on a `v*` tag, builds
  the Arch package `zendo` from the tag tarball, `repo-add`s it into a
  pacman repository pushed to `gh-pages`
  (`https://leandrop-dev.github.io/zendo/x86_64`), makes the tag's SRPM with
  a vendored crate tree and builds it on COPR, then publishes a GitHub
  Release with the source tarball, `SHA256SUMS` and both packages. The
  dry run was `v1.0.0-rc.1` (release 2026-11-17, COPR build 11342037); the
  Arch repository and the COPR repository both serve the package. ADR 0003.
  Not done: signing the Arch repository (item 65), a cleanup policy for old
  packages on Pages (item 66), and the AUR package (on hold by the user on
  2026-11-17 while the Pages repository works; registration is closed too).
- **65, sign the Arch repository** [tentative]. Currently unsigned with
  `SigLevel = Optional TrustAll`, so anything able to write the `gh-pages`
  branch can publish a package every machine installs. Steps, checked
  against `repo-add --help`, `makepkg --help` and `pacman-key --help` on
  2026-11-17:
  - A dedicated key (`zendo-repo@…`, expiry 2 to 3 years, not the personal
    key). Public key distributed; private key in a repository secret, with a
    passphrase secret if it has one (`gpg --batch --pinentry-mode loopback
    --passphrase`).
  - Each release: `makepkg --sign` (or `gpg --detach-sign`) for
    `zendo-<version>-1-x86_64.pkg.tar.zst.sig`, and
    `repo-add --sign --key <fingerprint>` for the database and `.files`
    signatures. The `arch` job imports the key into a temporary `GNUPGHOME`
    and publishes the `.sig` files beside the existing assets.
  - Trust on a user's machine, one time: download the public key and
    `pacman-key --add zendo.gpg` + `pacman-key --lsign-key <fingerprint>`,
    then `SigLevel = Required DatabaseOptional`; or ship a `zendo-keyring`
    package (key under `/usr/share/pacman/keyrings/`, install script running
    `pacman-key --populate zendo`), installed once from the release by
    fingerprint. The keyring package is the bootstrap that must be trusted
    out of band.
  - Risks to plan for: a lost or expired key needs a new keyring package
    signed with the old key; a leaked key needs revocation and a keyring
    update; signing protects the files on Pages, not a rewrite of both the
    keyring package and the repository.
  - README changes: fingerprint, the one-time commands or the keyring
    package, and `Required DatabaseOptional` in place of `Optional TrustAll`.
- **66, clean up old packages on GitHub Pages** [tentative]. `gh-pages` keeps
  every release (`keep_files: true`), about 18 MB each (`zendo-1.0.0` package
  ~18 MB; the RC's debug package went with `!debug`). Pages has a 1 GB site
  limit, so roughly 50 releases. A workflow step after `repo-add` could keep
  the newest two or three versions (delete packages whose names the database
  no longer lists, keep their `.sig` files in step), and say what was
  removed; users pinned to an older version would lose it, so state that in
  the release notes. Also watch the site size and report it in the job.
- **64, the first stable release** [built 2026-11-17]: `v1.0.0`.

### What was there before

- `v0.1.0` is an annotated pre-release tag at `af26913` (2026-11-16).
- The Arch `zendo-git` PKGBUILD builds `main`; the Fedora snapshot spec is
  still in `packaging/fedora/zendo.spec` for that path.

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
- Xbox streaming inside Zendo (asked 2026-11-17, after item 59): a stream of
  the console's own screen, in Zendo's window, instead of Greenlight. It is
  possible in principle, since Greenlight's whole streaming half is the MIT
  `xbox-xcloud-player` (TypeScript, on `xbox-webapi`): Xbox Live sign-in, then
  WebRTC with the console (or xCloud), H.264/HEVC video, Opus audio, and the
  controller's input over the same channel. In Rust that is four pieces Zendo
  has none of: the Xbox auth chain (device-code sign-in is already understood
  in `zendo-shell`'s sign-in window, the rest would be new), a WebRTC stack
  (`webrtc-rs`), a video decoder and an audio sink (gstreamer or ffmpeg +
  cpal), and a way to put decoded frames on a Slint surface at 1080p60. It is
  its own project, not a slice, and it would also be the one thing in Zendo
  that needs a GPU video path and a network protocol Zendo cannot test
  offline. On hold; revisit if `xbox-xcloud-player` gains a Rust binding or if
  the user wants to spend a phase on it.
- Recompiled games (N64 recomps and the like) and their mods (was 21), with
  ideas from Quiver Launcher (checked 2026-11-07: it installs apps from
  GitHub and GitLab releases with community catalogs; Business Source
  License 1.1 from version 4, so ideas only, no code).

## After the whole-workspace review (2026-11-18)

What the review left for a decision rather than a mechanical fix. The rest
of it is recorded in `docs/history.md` under the same date.

- **Contrast at the default translucency** (`ui/main.slint`, the background
  at `Colors.background.with-alpha(translucency)`): over a bright, blurred
  desktop `Colors.dim` falls near 1.6:1, and the footer and captions use it.
  Raising the default to about 0.85 keeps the blur without the wash-out, but
  it changes the look, so it waits for the user.
- **A motion and type pass**: no `animate` anywhere (150 to 200 ms on the
  background of chips, tabs and buttons would cover most of the window), and
  every font size is in `px` with `default-font-size: 14px`; `rem` would let
  the window follow the user's scale.
- **The no-match card is headed "About"** (`ui/game.slint`, `game-about`
  over `steam-app-no-match`). It stands in for the About card, but the
  heading describes what is missing; a new Fluent id is needed in every
  language, and the i18n tests enforce the same ids everywhere.
- **`Tr.count` / `lookup-count` is declared and never used** (`ui/style.slint`,
  `tr.rs`). Remove it, or use it for the plural messages that are formatted
  on the Rust side today.
- **`covers::Queue.wide`** grows for the life of the process (every hero,
  backdrop and screenshot ever asked for). It is bounded by the library size;
  pruning it on eviction races a worker that reads the set while decoding.
- **No size cap on a checksum-less download** (`http::download` uses
  `u64::MAX`; the Lutris-provided runtimes carry no digest). A cap is easy,
  but game and mod downloads share the same function and have no natural
  ceiling.
- **Two perf items, not faults**: `Sort::compare` lowercases both titles per
  comparison (`zendo list` over the whole library), and `owned::cached`
  derives images, sizes and totals for a whole store list on every library
  scan to look up a few installed ids.
- **The installer's accessible role is `checkbox` for every row**
  (`ui/installer.slint`): Slint wants a constant role and the list mixes
  radio rows and boxes. A per-row role would need the rows split into two
  components.

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
| 49 to 54 | The rest of the Slint window (game page, Settings, Accounts, Store, Downloads, mods, Stats, Game Mode), and the cut-over: `zendo-slint` became `zendo`, the Iced crate deleted, licence and packaging updated | 2026-11-14 to 2026-11-15 |
| 62 | Game Pass installs, registration with Nexus Launcher and uninstall | 2026-11-17 |
| 63 | CI (fmt, clippy, tests) and the tag release: GitHub Release, Arch pacman repository on Pages, COPR | 2026-11-17 |
| 64 | First stable release `v1.0.0` | 2026-11-17 |
| 65 | Whole-workspace review: security, data-safety and UI fixes; `v1.0.1` | 2026-11-18 |

Item 37 (emulation) is on hold above.
