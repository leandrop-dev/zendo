# Zendo

A game launcher for Linux: every game in one library, in a normal window with
a full-screen mode for the couch, launched with the right tools, and mods
later. A sister project of [hyprmonk](../hyprmonk), MIT like it.

Status: planned on 2026-11-04 from a hyprmonk session.
2026-11-05: window default, Rust + Iced and optional tools decided; first
session: the repo, the package checks and phase 1 built (see Phases).
2026-11-06: phase 1 checked against a real game from each store; phase 2
built. Then the direction changed: Zendo follows Hikari's functionality
and UI/UX [firm]; omarchy-games-menu was only ever a source of ideas for a
Rust launcher. The phases below are reordered for that. Phase 3, the
Hikari-layout shell, built the same day, then all of phase 4 (Accounts,
Store, Downloads); real sign-ins and installs wait for the user.
Later on 2026-11-06 the user tested phase 4 and listed what to change; the
order agreed [firm]: 4d (fixes to downloads, update status, sizes), then a
sign-in window, then phase 5 with Running/Stop and the ProtonDB rating. The
user checked 4d in the window; 4d and 4e (the sign-in window) were built
the same day, and the user checked both. Phase 5 started the same day,
split into 5a to 5e (below); 5a and 5b are built and the user checked
them; 5c is built, and its options fixed both quirks the user found. The user asked to leave in-game quirks (controllers, full
screen) for the settings sheet, and to get most features working first
[firm].

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
- SteamGridDB (`https://www.steamgriddb.com/api/v2`, `Authorization:
  Bearer <key>`), tried with a real key on 2026-11-06:
  `search/autocomplete/{name}`, `grids/game/{id}` and `grids/steam/{app}`
  with `dimensions=600x900,342x482,660x930` (a size it does not know gets a
  400, "Invalid asset dimensions specified"; Zendo then falls back to the
  first image taller than wide), and `heroes/game/{id}` and
  `heroes/steam/{app}` for the wide images (1920x620). A bad key gets a
  401, "Invalid key format"; no key a 401, "Authentication Required".
  Settings checks a key with one `search/autocomplete` call.
- Wide images (phase 3). Steam's `library_hero.jpg` beside the cover;
  Heroic's store caches give `art_cover`, a landscape image, and
  `developer`. Install details: legendary's `installed.json` has
  `install_path`, `install_size` (bytes), `platform` and `version`; Heroic's
  GOG `installed.json` the same with `install_size` as a label
  ("75.85 GiB"); nile's `path`, `size` (bytes) and `version`; Steam's
  manifest `installdir`, `SizeOnDisk` and `buildid`.
- Play records. Steam: `LastPlayed` in each `appmanifest_*.acf`, and
  `LastPlayed` and `Playtime` (minutes) under
  `UserLocalConfigStore/Software/Valve/Steam/apps/<app>` in
  `userdata/<account>/config/localconfig.vdf`. Heroic:
  `store/timestamp.json`, `lastPlayed` (ISO-8601, may be empty) and
  `totalPlayed` (minutes) by app name; its store splits keys on dots, so
  Amazon ids (`amzn1.adg.product.…`) are nested objects to join back.

Zendo's own files [tentative]: `~/.config/zendo/config.toml` for what the
user sets (`steamgriddb_key`, `download_covers`), written by the Settings
page with mode 0600 because it holds the key (comments added by hand are not
kept; Hikari uses the system keyring first, which Zendo could add later
[tentative]); `~/.local/state/zendo/state.json`
for what Zendo remembers (launches, hidden and pinned games, the sort),
written by the window and the command alike; and `~/.cache/zendo/covers/`
with `<id>.<ext>` covers, `<id>.hero.<ext>` wide images and a
`misses.json` (keys `<id>` and `<id>#hero`) so a game without one is asked
about again only after a week. One state file, not omarchy-games-menu's split by owner, since both
sides write the same few things; each write reads, changes and renames the
file into place.

Stores (phase 4), read from legendary, gogdl and nile's source and Heroic
2.22.4's on 2026-11-06:

- Engines [tentative]: each is looked for on `PATH` first, then in Heroic's
  bundled copies (`/usr/lib/heroic-games-launcher/...` or `/opt/Heroic/...`).
  Zendo signs in with accounts of its own, in
  `~/.local/share/zendo/stores/{epic,gog,amazon}` (mode 0700), never in
  Heroic's: `LEGENDARY_CONFIG_PATH=<dir>`; `GOGDL_CONFIG_PATH=<dir>` (gogdl
  adds `heroic_gogdl`) with `--auth-config-path <dir>/auth.json`;
  `NILE_CONFIG_PATH=<dir>` (nile adds `nile/`).
- Sign-in [tentative]: paste the code, Hikari's fallback, rather than a
  browser window of Zendo's own. Zendo opens the store's login page in the
  user's browser; the user pastes back what the page shows (Epic's JSON,
  GOG's final address, Amazon's final address) and Zendo finds the code in
  it. Epic: `legendary.gl/epiclogin`, then `legendary auth --code`; the name
  is `displayName` in `user.json`. GOG: `auth.gog.com/auth` with Galaxy's
  client id, ending on `embed.gog.com/on_login_success?code=`, then
  `gogdl auth --code`; the name from `users.gog.com/users/{id}`. Amazon:
  `nile auth --login --non-interactive` gives the URL and the PKCE data
  (kept in memory only), the final address holds
  `openid.oa2.authorization_code`, then `nile register`; `nile auth
  --status` gives the name. Tokens and codes are never logged or shown.
- Owned games: `legendary list --json` (DLC have `metadata.mainGameItem`;
  covers from `keyImages` `DieselGameBoxTall` and `DieselGameBox`); `nile
  library sync`, then `nile/library.json`; GOG has no list command, so
  Zendo asks `galaxy-library.gog.com/users/{id}/releases` (paged), keeps
  `platform_id == "gog"`, and asks `gamesdb.gog.com` for each game with
  the release's certificate, caching each answer under
  `~/.cache/zendo/store/gogdb/` with its `ETag`. Games that are not
  `game` or `mod`, or not `visible_in_library`, are left out. Against the
  user's real Heroic caches: 156 Epic games (16 DLC left out), 135 Amazon,
  113 GOG (86 hidden entries and 3 DLC left out), every one with a cover
  and a developer.
- Each store's list is kept in `~/.cache/zendo/store/<store>.json` and
  shown at once on the next start; it is asked for again only on Refresh,
  at sign-in, or when none is kept. Signing out forgets it. A store game
  counts as installed when the library has a game with the same id
  (`epic:<app name>`, `gog:<product id>`, `amazon:<product id>`, as
  Heroic's import names them).

Covers are kept at the size the store serves them. Iced decodes each to
full-size RGBA (the Amazon one is about 6 MB in memory), which is fine for a
few dozen games; for large libraries, scale them down when saving [re-evaluate
once someone has a few hundred games].

## What the user needs installed [tentative]

Nothing is required. Zendo detects each tool and shows what is missing.

| Tool | Needed for |
|---|---|
| Steam | Steam games. The library is read from files, but launching goes through the client. |
| Heroic | Its installed games are imported. From phase 4 also a source of the store engines when they are not installed alone. |
| legendary, gogdl, nile | Phase 4: Epic, GOG and Amazon accounts, owned games and installs. |
| umu-launcher | Windows games installed by hand. Check whether it fetches Proton itself. |
| MangoHud, GameMode, gamescope | Only games where they are switched on. |

## Ideas from (ideas only, no code copied)

- [Hikari](https://github.com/dodo986/hikari) (GPL-3.0, Go daemon + React
  window in Wails/WebKitGTK), the model for Zendo's functionality and UI/UX.
  Pages (from its README, CHANGELOG and `frontend/web/src/pages`, read
  2026-11-06): Library and Store (installed vs owned, a cover grid, search,
  store chips with counts, "A-Z" and "Playing first" sort chips, pins that
  lead the library, "+ Add game", "Refresh"); a game page ("← Library" and a
  breadcrumb, a landscape hero with store/installed badges, title,
  developer and facts, an action bar "Play now / Launch command / Settings
  / Log / Pin", a status line, a "Game details" card, an OptiScaler card,
  and sheets for game settings, the launch command and the launch log);
  Downloads; Accounts (Epic, GOG and Amazon sign-in in a private browser
  window, falling back to pasting a code); Settings (the daemon, a
  Wine/Proton manager with GE-Proton and Proton-CachyOS downloads, engine
  readiness, API keys kept in the system keyring with a 0600 file as
  fallback, Diagnostics with "Copy report"); About; Mods. It drives legendary/gogdl/nile with credentials of its own; imports
  Heroic's installed games and reads Steam libraries (native, Flatpak, Snap)
  without writing to them; umu/Proton or Wine with a prefix per game;
  DXVK/VKD3D, EAC/BattlEye runtimes; MangoHud, GameMode, gamescope and custom
  wrappers; mods through Amethyst Mod Manager's backend (Nexus, FOMOD,
  `nxm://`); OptiScaler; `logs` and `report` commands and a diagnostics page.
  GPL: read for ideas, never copy.
- [omarchy-games-menu](https://github.com/akitaonrails/omarchy-games-menu)
  (MIT, Rust `ogm` + Quickshell), ideas for a Rust launcher only, not the
  model: full-screen grid of covers in sections;
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
   games by last played. Checked later that day: SteamGridDB with a real key
   (`dimensions=600x900,342x482,660x930` accepted, a bad size gets a 400 with
   "Invalid asset dimensions specified"; search and grids by Steam app or
   game id work), and every key in the live window through `wtype`, F11
   full-screen included.
3. The shell, Hikari's layout: a sidebar (Library, Settings, About for now;
   Store, Downloads and Accounts join with phase 4), the Library page (title
   and counts, search box, Refresh, store chips with counts, sort chips,
   showing hidden games, pins that lead the grid, cards with a store badge),
   the game page (back and breadcrumb, a hero from Steam's
   `library_hero.jpg`, Heroic's `art_cover` or SteamGridDB's heroes, badges,
   developer, an action bar with Play, Pin and Hide, a status line, a
   details card with store, launcher, install path, size, version and play
   time), Settings (the SteamGridDB key typed in and checked, covers on or
   off, the sources found, Zendo's files) and About. A click or Enter opens
   a game; Play starts it. Built 2026-11-06 (`crates/zendo/src/app/`, one
   module per page). Checked live through `wtype` on the four test games:
   every page, pinning (the pinned game leads), the search, the Heroic wide
   images downloaded by themselves, Steam's from its cache; and on scratch
   folders, a wrong key turned down and not saved, the real one accepted
   and written with mode 0600. Save keeps a key only once SteamGridDB takes
   it, or when the site cannot be reached. Keys added: F1 About, Ctrl+,
   Settings, Ctrl+F search, Ctrl+P pin, F5 or Ctrl+R refresh; on a game's
   page Y pins and X hides. Not yet tried: Play from the game page on a
   real game, a real controller.
4. Accounts, Store and Downloads, in three steps.
   - 4a Accounts, built 2026-11-06: a page with a card per store (who is
     signed in, the engine found and where), sign in by pasting the code
     (Ctrl+V pastes from anywhere on the page), sign out. Checked on
     scratch folders: the three cards, the engines found in Heroic's copies,
     nile's folder made with mode 0700. Not yet tried: a real sign-in to
     each store.
   - 4b Store, built 2026-11-06: every game the signed-in stores say the
     user owns, by title, with the installed ones marked; search, store
     chips, "Not installed only", Refresh, and a page per game (hero,
     details, "Open in library" when installed; Install waits for 4c). Its
     own cursor and scroll, kept when going back. Checked on scratch
     folders with a stand-in `legendary` that printed Heroic's legendary
     metadata: 156 games, all 312 images downloaded, Absolute Drift marked
     installed and opening its library page. Two fixes from that: image
     URLs with bare spaces (Epic's GTA V) are encoded, and a list that
     arrives while images download is fetched for afterwards. Not yet
     tried: the real engines' lists through Zendo's own sign-ins, GOG's
     and Amazon's pages.
   - 4c Downloads, built 2026-11-06: install, update, repair and uninstall
     with a queue, a Downloads page (the running job with a progress bar,
     percentage, size, speed and time left, Stop; up next with Remove;
     finished this session), Install on a store game's page (Enter too),
     and Update, Repair and Uninstall (pressed twice) on the page of a game
     Zendo installed.
     No background process [firm, chosen 2026-11-06]: unlike Hikari's
     daemon, the window runs legendary, gogdl and nile as its own child
     processes and reads their progress; the queue is kept in
     `~/.local/state/zendo/downloads.json` and picks up on the next start,
     and closing the window while something downloads asks first (Iced's
     `exit_on_close_request: false`, set in `window::Settings` itself,
     since `.window()` replaces what the builder set before it). A
     detached worker can come later if it is missed.
     How each engine is driven, from their source and Heroic 2.22.4's:
     legendary `install <app> --base-path <dir> --platform Windows
     --skip-sdl --skip-dlcs -y`, `install <app> --update-only`, `repair
     <app> -y`, `uninstall <app> -y`; nile `install --base-path <dir>
     <id>`, `update`, `verify`, `uninstall`; gogdl `info <id> --platform
     <p> --lang en-US` (JSON with `folder_name`, `versionName`, `buildId`
     and `size` per language), then `download|update|repair <id>
     --platform <p> --path <dir> --support <stores/gog>/support/<id>
     --skip-dlcs --lang en-US` (`download` adds the game's folder to the
     path, the others take the game's folder). All three log `= Progress:
     12.34[%] …, ETA: hh:mm:ss`, `= Downloaded: N MiB` and `+ Download\t- N
     MiB/s`. Each engine runs in a process group of its own; Stop sends
     SIGINT to the group, as Ctrl+C would, then kills it after 15 s.
     Choices [tentative]: games go into `~/Games` (`install_dir` in
     config.toml); GOG's Linux build when there is one, else Windows;
     English for GOG; no DLC; a game both Heroic and Zendo installed keeps
     Heroic's launch. gogdl keeps no list of what it installed, so Zendo
     writes `stores/gog/installed.json` in Heroic's shape and reads it
     with the same code as Heroic's; uninstalling a GOG game deletes the
     folder recorded there, refusing the home folder, anything right in
     it, the games folder itself, a folder holding another game, links
     and paths fewer than three levels deep. Games Zendo installed show in
     the library through Heroic's readers pointed at Zendo's folders, with
     titles and images from the Store's lists; Play on them says launching
     comes with phase 5.
     Checked on scratch folders with stand-in legendary and gogdl scripts
     (progress in each engine's format): two installs, the library listing
     both with size and version, Stop reaching the engine's process group,
     a GOG update, both uninstalls leaving nothing behind; in the window,
     the queue starting by itself, the footer's progress, the finished
     games joining the library, and a close request opening Downloads
     with the confirm. When Zendo was killed outright, the stand-in engine
     died at its next log line (the pipe closed); whether the real engines
     do the same is not checked. Not yet tried: any real engine install,
     update, repair or uninstall.
   - 4d Fixes from the user's tests, built 2026-11-06:
     - GOG reinstall: gogdl compares against the manifest of the last
       install it finished (`stores/gog/heroic_gogdl/manifests/<id>`), and
       uninstalling left it, so reinstalling found "Nothing to do" and
       failed with "the game's folder is not there". Reproduced with the
       real gogdl; the manifest now goes on uninstall, on cancel, and
       before an install Zendo has no record of.
     - Pause, Resume and Cancel replace Stop [firm, the user's choice].
       Pause stops the engine and keeps what it wrote; Resume carries on;
       Cancel stops and, for an install, deletes the game's folder and what
       the engine kept to resume. Each install gets a folder of its own,
       picked when it starts and kept with the job (legendary
       `--base-path <dir> --game-folder <name>`, nile `--path <folder>`,
       gogdl's own `folder_name`), so Cancel knows what to delete. A job
       that fails stays queued, paused with the reason, to try again;
       closing the window mid-download pauses the whole queue, and nothing
       starts by itself next time. A stuck engine's whole process group is
       killed after the 15 s grace (gogdl was seen to hang on a thread
       after Ctrl+C).
     - Resuming, gogdl counts only the bytes left (`= Progress: 5.49
       n/total`, total = what is left to write), so a resumed GOG install
       is shown against the game's whole size and carries on from where
       the bar was. legendary and nile resume too (legendary from
       `tmp/<app>.resume`, nile file by file, re-checking what is there);
       how their percentages behave on a resume is not checked.
     - Update status: the page of a game Zendo installed says "Up to
       date" or "Update available: <version>", with an UPDATE AVAILABLE
       badge and Update highlighted; asked once a session and after each
       job, or with "Check for updates". legendary `info <app> --json`
       (`game.version` against `install.version`), gogdl `info` (`buildId`
       against the one Zendo recorded), nile `list-updates --json` (a list
       of ids).
     - Sizes and store facts on both game pages: download size, size on
       disk, latest version and GOG's latest build date, asked of the
       engine when the page opens (legendary `info --json`'s
       `manifest.download_size`/`disk_size`, gogdl `info`, nile `install
       --info --json`'s `download_size`, the files' total); publisher,
       release date, genres and the store's description from the lists
       (GOG's games database; nile's `productDetail.details` and Epic's
       `metadata.description` field names are unverified, so they may
       stay empty). The install note says how much room it needs.
     Checked with the real gogdl and the user's GOG sign-in, installing
     Bō: Path of the Teal Lotus into a scratch folder with a stale
     manifest planted: the manifest cleared, the install paused at 5%,
     `.gogdl-resume` kept, resumed from 4.5%, cancelled leaving nothing.
     Not checked: legendary and nile (not signed in). The user tried the
     window and it worked.
   - 4e Sign-in window [firm], built 2026-11-06: Sign in opens the store's
     login page in a small window of Zendo's own and takes the code from
     the page the store's sign-in ends on, as Heroic does, with nothing to
     copy; for children and people who find codes confusing. The window is
     a second process, `zendo sign-in-window <store> <url>` (hidden from
     the help), since the web view needs GTK's own loop apart from Iced's:
     `wry` 0.57.1 on `webkit2gtk-4.1`, built with `build_gtk` (no X11
     needed, so it runs on Wayland), incognito so no cookies or caches
     are written (without it WebKit wrote `storage`, `localstorage`,
     `mediakeys` and `hsts-storage.sqlite` into `~/.local/share/zendo` and
     two caches into `~/.cache/zendo`). It watches each navigation and
     page load, and a script on the last page posts its address (and on
     Epic's, its JSON text); `stores::code_at` takes the code only from
     `embed.gog.com/on_login_success`, a `www.amazon.*` address with
     `openid.oa2.authorization_code`, or Epic's `/id/api/redirect`. The
     code goes to stdout, closing gives exit code 3, and the window ends
     when Zendo closes its stdin (Cancel, Zendo exiting). Zendo sets
     `WEBKIT_DISABLE_DMABUF_RENDERER=1` for it unless already set, as
     WebKitGTK's DMA-BUF renderer draws blank on NVIDIA's driver on
     Wayland [re-evaluate when WebKitGTK fixes it]. When the window cannot
     open, or with "Use my browser instead", the old paste steps remain.
     Checked: a local page redirecting to a made-up GOG success address
     gave its code without loading it; closing stdin gave exit 3; GOG's and
     Epic's real login pages rendered on this PC (GTX 1070 Ti, Hyprland);
     nothing written to disk in incognito.
     Pop-ups, for "Sign in with Google" and the like, which the user and
     others use on Epic and GOG: each opens in a window of its own, a web
     view related to its opener (so `window.opener` works and it shares
     the opener's in-memory session), with the same watch for the code,
     closed when its page calls `window.close()`. Pages may open them
     outside a click (`javascript-can-open-windows-automatically`). wry's
     own default for pop-ups needs a `gtk::ApplicationWindow`, and without
     a handler WebKit opens none. Checked with a local opener and pop-up:
     two windows, the pop-up's message reaching its opener, the pop-up
     closing itself, the code coming back. The user then signed in
     through the window and it worked.
5. Launch options per game (a game settings sheet): Proton through umu with
   a prefix per game, a Wine/Proton manager (GE-Proton downloads), MangoHud,
   GameMode, gamescope, arguments and environment, a log of each launch.
   With it [firm]: Play turns into Running while the game runs, with a Stop
   that kills the game when it freezes (exact for games Zendo starts; for
   Steam and Heroic games, found by their install folder); and a ProtonDB
   rating pill on the game's page (Borked to Platinum, by Steam app id,
   found by title for the other stores), which opens the game's ProtonDB
   page in the browser when clicked. Games are started from the home
   folder [tentative, offered, not yet agreed; built in 5a], so a relative
   path in a launcher's settings (Heroic once had `winePrefix:
   "undefined/<title>"`) does not depend on where Zendo was started.
   Steps [tentative]: 5a starting the games Zendo installed, with Running
   and Stop for them; 5b Running and Stop for Steam and Heroic games, by
   their install folder; 5c the settings sheet; 5d the ProtonDB pill; 5e
   GE-Proton downloads.
   - 5a Starting the games Zendo installed, built 2026-11-06: through the
     engine's own `launch` (it knows the store's arguments, such as Epic's
     sign-in token): legendary and nile `launch <app>`, gogdl `launch
     <path> <id> --platform <p>`. A Windows build gets `--no-wine
     --wrapper <umu-run>` (all three split the wrapper as a shell would)
     with `WINEPREFIX`, `GAMEID`, `STORE` (`egs`, `gog`, `amazon`) and
     `PROTONPATH` set. The prefix [tentative] is
     `<install_dir>/Prefixes/<game's folder name>`, made on the first
     launch. The Proton [tentative, until the settings sheet] is the first
     found in Steam's compatibility tools folders (the user's, `~/.steam`,
     `/usr/share/steam`, `/usr/local/share/steam`) and Steam's own
     Protons: GE-Proton or UMU-Proton (which carry the fixes umu applies)
     before a distribution's (cachyos, tkg), then Valve's, Experimental
     last, newest first; with none, umu downloads UMU-Proton itself.
     `GAMEID` comes from umu's database (`umu_api.php?store=gog&codename=
     <id>`, a 5 s timeout), kept in `~/.cache/zendo/umu.json`; a game it
     does not know runs as `umu-default` and is asked about again after a
     week. Each launch's output goes to
     `~/.local/state/zendo/logs/<id>.log`, headed with the command and its
     environment.
     Running and Stop: every game Zendo starts (engine games and
     `.desktop` ones) runs in a process group of its own, and counts as
     running while any process of the group is alive (polled every 2 s),
     since legendary starts the game and exits. Play turns into Running
     with Stop beside it, the card shows RUNNING; Stop sends SIGTERM to
     the group, and turns into Force stop, which sends SIGKILL. Signals
     never go to group 0, 1 or Zendo's own. When the game ends, its play
     time is added. A game handed to Steam or Heroic is not followed yet
     (5b). The page of a Zendo-installed game has a Launch card with the
     prefix and the last log, each with Open.
     Checked: clippy and tests (a group seen running, then gone after a
     kill; Running and closed in the window's state; a failed start
     leaving Play). The user then installed and played two games through
     Zendo, and it worked: 10 Second Ninja X from Amazon (nile, umu's
     database knew it as `umu-435790`) and Gunslugs 3 Rogue Tactics from
     GOG (gogdl, `umu-default`), both on proton-cachyos-slr, the only
     Proton here. That was also the first real Amazon install. Not
     checked: an Epic game through legendary, and how long umu's first
     run takes to fetch the Steam Linux Runtime on a fresh PC, with
     nothing shown meanwhile.
     Quirks the user found, left for 5c [firm]: Gunslugs 3 opened in a
     window, not full screen (a Java game keeping its own window mode,
     `storedWindowedModeID` in its preferences in the prefix; gamescope's
     `-f` is the general answer); the DualSense did nothing in 10 Second
     Ninja X, though the game's options had the controller on (a
     GameMaker game, which reads controllers through XInput; Proton hands
     a DualSense to the game as a raw HID device, and Steam Input's
     virtual Xbox pad is not there outside Steam). Proton's
     `PROTON_PREFER_SDL=1` (it sets `PROTON_DISABLE_HIDRAW=1` and
     `PROTON_NO_STEAMINPUT=1`) is the likely switch (confirmed in 5c); the user
     wants it a per-game choice in the settings sheet, not a default, so
     games that use a DualSense or an Xbox pad natively keep it, and will
     test it with both (a PS5 DualSense and an Xbox Series S pad).
   - 5b Running and Stop for Steam and Heroic games, built 2026-11-06:
     after the link is handed over, Play shows Starting ("Waiting for
     Steam to start …") until a process turns up whose program is in the
     game's install folder or whose arguments name it (as a whole path,
     alone or after `=`: `/games/Foo Bar` is not `/games/Foo`); with it,
     every process those started. For Steam that is its `reaper` and
     Proton's script (`waitforexitandrun`), which last as long as the
     game; for Heroic its `umu-run`, `wine` or the native program. Then
     Running with Stop (SIGTERM to those processes, then Force stop with
     SIGKILL), and closed once three looks in a row (6 s) find none, as a
     game's own launcher may hand over to the game with a gap. Zendo waits
     5 minutes [tentative] for the game to turn up (Steam may update it
     first), then says it did not see it start. Play time is left to
     Steam and Heroic, which count their own. A game with no install
     folder known is handed over unfollowed, as before.
     Checked: clippy and tests (a process naming a folder and its child
     found, then gone after a kill; whole-path matching; Starting, Seen,
     Running and closed in the window's state). The user then tried it
     with Hexologic from Steam, and it worked. Not checked with a Heroic
     game. Known edges: anything else naming the folder
     counts too (an editor open on a file in it, Heroic's engine updating
     the game); a game started outside Zendo shows Play until it is
     started from Zendo.
   - 5c Launch options sheet, built 2026-11-06: "Launch options" beside
     Play on the page of a game Zendo starts itself (installed by Zendo,
     or a `.desktop` game) opens a sheet over the page; Steam and Heroic
     games keep their launcher's own settings. Kept per game in
     `~/.config/zendo/games.toml` [tentative] (a table per id, games left
     as they come not written, readable by the user only since an
     environment can hold a token):
     - Proton [Windows games Zendo installed]: "Default (<first found>)"
       or any one installed; one chosen and then removed shows as "not
       installed" and the default runs.
     - MangoHud, GameMode, gamescope with full screen and a size (`-f`,
       `-W`/`-H`, both or neither). Wrappers outermost first:
       `gamemoderun`, `gamescope … --` (with `--mangoapp` when MangoHud
       is on, as MangoHud does not draw inside gamescope), `mangohud`,
       then umu-run for a Windows game; passed to the engine as one
       `--wrapper`, or put before a `.desktop` game's program. A switch
       whose program is missing is greyed out with "Not installed"
       (GameMode is not installed here); a game whose options turn on a
       missing program does not start, and says which.
     - Controllers as Xbox pads [Windows games]: `PROTON_PREFER_SDL=1`,
       off by default [firm, the user's choice], for 10 Second Ninja X's
       kind of game.
     - Arguments and environment, split as a shell would (`shlex`):
       environment words must be `NAME=value`. All three engines pass the
       arguments they do not know on to the game (legendary says so;
       gogdl's and nile's source pass their unknown arguments on), so
       they go after Zendo's own; an argument an engine knows itself is
       taken by it (argparse also takes abbreviations, as `--win` for
       legendary's `--wine`).
     While the sheet is open, Enter saves, Escape closes, and the page's
     keys and the controller do nothing. Checked: clippy and tests (the
     options file leaving defaults out, shell splitting and bad
     environment words, wrapper order, the command a native GOG game gets
     with options on, the sheet opening, refusing a bad environment or a
     width without a height, saving, Escape, no sheet for Steam games).
     The user then tried the sheet, and both quirks from 5a were gone:
     Gunslugs 3 went full screen with gamescope and full screen on, and
     the controller worked in 10 Second Ninja X with controllers as Xbox
     pads on (`PROTON_PREFER_SDL=1`, confirming the cause above).
6. Local games: "Add game", with a cover search on SteamGridDB.
7. Mods: Nexus, `nxm://`, profiles, back to vanilla.

## On hyprmonk's side

- Install it (package list or the installer) and bind a key (Super+G).
- The tone: hyprmonk writes no file with the active tone's colours yet
  (`~/.local/state/hyprmonk/theme/colors.json` is matugen's wallpaper palette,
  `tone.lua` only the border accent). Add a `tone.json` there for Zendo to
  read into its Iced theme, with its own colours when the file is missing.
- hyprmonk's roadmap points here; this file is the plan.

## Next session

- 5d (ProtonDB pill), then 5e (GE-Proton downloads).
  An Epic install and launch is still untried.

## Conventions

As in hyprmonk: commit on main, no attribution lines, commit only when asked.
