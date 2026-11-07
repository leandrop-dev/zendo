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
them; 5c is built, and its options fixed both quirks the user found. 5d
is built and checked by the user. 5e is built and the user checked it,
then asked for a default Proton setting and older releases, both built
and checked. Phase 5 is done. The user asked to leave in-game quirks (controllers, full
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
   - 5d ProtonDB pill [firm], built 2026-11-06: on both game pages (the
     library's and the Store's), beside the badges: PROTONDB PLATINUM,
     GOLD, SILVER, BRONZE, BORKED or PENDING in the tier's colour, or
     "NO REPORTS"; a tooltip gives the number of reports, and a click
     opens `https://www.protondb.com/app/<appid>` in the browser. Nothing
     shows for a game ProtonDB cannot know. The rating is ProtonDB's
     summary, `/api/v1/reports/summaries/<appid>.json` (`tier`, `total`;
     a 404 when there are no reports). The Steam app id: a Steam game's
     own; for an Epic, GOG or Amazon game, umu's id when it is
     `umu-<number>` (umu uses Steam's ids); otherwise the Steam store's
     search (`/api/storesearch/?term=<title>`) when an app's name has
     exactly the title's letters and digits, case aside [tentative: no
     fuzzy match, so a title spelt differently finds nothing rather than
     the wrong game]. Kept in `~/.cache/zendo/protondb.json`, asked again
     after a week; a failed lookup is not kept and is asked again when
     the page next opens. A game with a Linux build on Steam shows
     PROTONDB NATIVE in green instead, as ProtonDB's own pages do: the
     summary has no such field, ProtonDB's site takes it from Steam's
     platforms, and Zendo asks Steam the same
     (`/api/appdetails?appids=<id>&filters=platforms`,
     `platforms.linux`); the tooltip still gives the Proton tier. Cache
     entries from before that carry no version and are asked again.
     Checked against the real services: Hexologic (Steam id, Native, with
     Platinum from 9 reports), 10 Second Ninja X (umu's `umu-435790`,
     Platinum, 37), Bō: Path of the Teal Lotus (found by title, 1614440,
     Platinum, 9), Gunslugs 3 Rogue Tactics (none: Steam calls it
     "Gunslugs : Rogue Tactics"). The user checked the pill on screen;
     they noticed Hexologic showing Platinum where ProtonDB says Native,
     which led to the Native check, which they then checked too.
   - 5e GE-Proton downloads, built 2026-11-06: a Proton section on the
     Settings page lists the Protons found (name and folder) and the five
     latest GE-Proton releases from GitHub
     (`api.github.com/repos/GloriousEggroll/proton-ge-custom/releases`,
     asked once a session when Settings opens, or with "Check again";
     prereleases left out), each with its date, size, and Download or
     Installed. Since GE-Proton11 each release has a tarball per
     architecture (`GE-Proton11-12-x86_64.tar.gz`, `…-aarch64…`); before,
     one `GE-Proton10-4.tar.gz`; Zendo takes either for this machine,
     with its `.sha512sum`. One download at a time, with a bar, the bytes
     so far, and Stop: into `~/.cache/zendo/ge-proton/`, checked with
     `sha512sum`, unpacked with `tar --no-same-owner` into a hidden
     `.zendo-<name>.partial` folder in
     `~/.local/share/Steam/compatibilitytools.d` [tentative: the native
     Steam's folder, made when missing; a Flatpak Steam's is not used]
     and moved into place only when complete; the tarball is deleted
     after, and leftovers from a download Zendo was closed in the middle
     of are cleared at the next one. A release without a checksum is
     refused. Remove, for Protons in that folder only (never a
     distribution's or one Steam installed), asks once ("Really
     remove"), naming how many games' launch options chose it (they then
     run with the default). New Protons appear in the launch options
     sheet's list when it next opens.
     Checked: clippy and tests (a tarball made on the spot unpacked into
     place with nothing left beside it, and refused a second time; a
     release known by either name; a system Proton refused for removal;
     one download at a time, Stop and a checksum failure reported, Remove
     asking first); GitHub's real list read (GE-Proton11-12 to 11-3, all
     with checksums). The user then downloaded GE-Proton11-12 (into
     `compatibilitytools.d/GE-Proton11-12-x86_64`), chose it, ran a game
     with it (its log names that `PROTONPATH`) and removed it.
     From that test [firm, the user's asks]:
     - A default Proton in Settings: downloading GE-Proton made it the
       default, as Automatic picks the newest GE-Proton first. The
       section now starts with a Default list, "Automatic: newest
       GE-Proton first (<its pick>)" or any Proton found, kept as
       `default_proton` in `config.toml`; a game whose launch options
       leave Proton as it comes runs with it, and the sheet's "Default
       (…)" names it. One chosen and not found falls back to Automatic
       and says so in the list; removing the default from Settings sets
       it back to Automatic.
     - Older releases: "Show older versions" under the latest five adds
       ten more each press, asking GitHub for its next page of 30 only
       when those read run out (GitHub allows 60 requests an hour
       without an account, and GE-Proton has well over a hundred
       releases). Checked with GitHub's real page 2: GE-Proton10-16 and
       older, with the old tarball names.
     Checked: clippy and tests (pages joined in order, the next page
     asked only past what was read; the default set and Automatic
     first). The user then tried both in the window, and they worked.
   - After phase 5, from the user's tests with Epic games (Cat Quest,
     Costume Quest, Home) and 10 Second Ninja X:
     - In-game settings seemed to revert. The prefixes keep them (each
       game wrote its options file or registry there), but those sessions
       ended with the game killed, not quit: Cat Quest's log ends with
       gamescope's "Parent of gamescopereaper was killed. Killing
       children." (Super+Q closed gamescope's window, and gamescope kills
       the game), and 10 Second Ninja X's save was last written the
       minute it started. Games that save on quit lose the change.
       Zendo's Stop did the same (SIGTERM to every process; a Windows
       game under Proton just dies). Built at the user's ask: Stop now
       first asks the game's windows to close, as their X button would
       (`WM_DELETE_WINDOW`, which Wine turns into the game's own close),
       so the game can save, or ask to; with no such window it sends
       SIGTERM as before (native games take that as a quit); a second
       press is still Force stop (SIGKILL). The windows are found over
       X11 (`x11rb`, already in the build through Iced): every mapped
       top-level window, or one level down, on the X displays the game's
       processes use (their `DISPLAY`, so gamescope's own X server too,
       and Zendo's), whose `_NET_WM_PID` is one of the game's processes
       and which takes `WM_DELETE_WINDOW`. umu's container shares the
       system's process ids (pressure-vessel's `--share-pid`, its
       default), so Wine's pids are the ones Zendo sees. Native Wayland
       windows are not reached this way; their games get SIGTERM.
       Checked on gamescope's headless X server (nothing on screen): of
       two windows, only the one with the game's pid got the request.
       The user then tried Stop on their games and said it seems right.
     - Window classes, asked for by hyprmonk's session for its window
       rules: Zendo's window had an empty class; it is now `zendo`
       (Iced's `platform_specific.application_id`), and so is the
       sign-in window (GTK takes it from the program name, set with
       `glib::set_prgname`). Zendo ships no `.desktop` file yet; when it
       does, it needs `StartupWMClass=zendo`. Games' classes, read with
       `hyprctl clients` while each ran from Zendo: a game umu knows,
       `steam_app_<number>` (10 Second Ninja X, `umu-435790`:
       `steam_app_435790`, XWayland); one it does not,
       `steam_app_default` (Costume Quest); one in gamescope, gamescope's
       window, class `gamescope` with the game's title (Cat Quest, now
       `umu-593280` through the title fallback).
     - Cat Quest stayed cropped in gamescope full screen: Unity had saved
       the tile Hyprland first gave it, windowed at 235x465, and draws at
       that size; gamescope's `-W`/`-H` size its own screen, not the
       game. Unity's own `-screen-fullscreen 1 -screen-width 1920
       -screen-height 1080` in Arguments overrides it. The tiling itself
       is hyprmonk's to fix: a window rule for games, asked for there in
       `docs/zendo-games.md` and its roadmap.
     - umu by title, built: umu's database had none of the Epic games by
       their app names (its Epic entries are keyed by them, as `Catnip`
       for Borderlands 3), so they ran as `umu-default`. When the store's
       id finds nothing, Zendo searches umu by title; that search matches
       part of a title ("Cat" gives four games), so each id it gives (five
       at most) is looked up with `umu_api.php?umu_id=` and taken only
       when one of its titles is the game's, compared as ProtonDB's are.
       Cache entries from before are asked again. The ProtonDB cache is
       asked again too, as its ids for store games come through umu.
       Checked against the real database: Cat Quest finds `umu-593280`,
       10 Second Ninja X `umu-435790`; "Cat", "Home" and Costume Quest
       find none.
     - The fixes umu applies: protonfixes, as for Steam's Proton, Heroic
       and Lutris. An unknown game gets only umu's general defaults (the
       logs show `OPENSSL_ia32cap`, which avoids a crash on newer Intel
       CPUs); game-specific fixes need umu to know the game.
       `PROTONFIXES_DISABLE=1` in a game's environment turns them off.
6. Local games: "Add game", with a cover search on SteamGridDB. Split in
   two:
   - 6a Games added by hand, built 2026-11-06: "Add game" in the
     Library's header opens a sheet: the program (typed, or "Choose…",
     the desktop's own file dialog through GTK's native chooser, which
     goes through the portal; a second process, `zendo pick-file`, as
     with the sign-in window), the title (filled from the program's
     folder when empty: `Cave Story/Doukutsu.exe` gives "Cave Story",
     past folders like `bin` or `x64`) and arguments. A `.exe`, `.bat`
     or `.msi` is a Windows game [tentative: by extension only], run
     through umu-run with Proton in a prefix of its own
     (`<install_dir>/Prefixes/<id's name>`, made on the first launch);
     anything else must be executable and runs as it is. Both start from
     the program's folder, as many games expect. Kept in
     `~/.config/zendo/local.toml` (`[[game]]`: id, title, program,
     arguments); the id is `local:<title as a slug>` from the title
     first given and stays when the title changes, so play time, pins,
     launch options and the prefix stay with the game. umu's id is
     looked up by title (there is no store), so umu's fixes and ProtonDB
     apply to the games it knows. The source chip is "Added"; the page
     has Edit (the same sheet), Remove (asks once; the files and the
     prefix stay), launch options and the launch card; play time is
     counted as for the games Zendo installed. The cover comes from
     SteamGridDB by title like any game without a store's [tentative
     until 6b: a renamed game keeps the cover found under its first
     title].
     Checked: clippy and tests (ids from titles, Windows by extension,
     what is refused: no title, a missing file, a Linux file that is not
     executable, an open quote; the file written and read back; the
     command a Linux game gets with options on, from its folder; the
     title guessed from a folder; the sheet refusing a game with no title,
     Escape closing it, Remove asking first). The user then added a
     Windows `.exe` and a Linux game, which found their covers on
     SteamGridDB, and tried Edit and Remove; all worked. Jet Set Radio
     Future's `jsrf_launcher` started (its log has the command, run from
     its folder) but the game did not come up, which the user is taking
     up with its developer.
   - 6b Art picked by hand, built 2026-11-06: "Change art" at the top
     right of the wide image on both game pages (the library's and the
     Store's) opens a picker over the page: Cover or Wide image, a search
     box with the game's title, the games SteamGridDB finds for it (a
     Steam game's own entry first, through `games/steam/<appid>`; then
     `search/autocomplete/<title>`, eight shown, with their year), and
     the first one's images: `grids/game/<id>` in Steam's portrait sizes,
     or `heroes/game/<id>`, still images only (`types=static`, as
     animated ones may not show), SteamGridDB's default of no NSFW ones.
     Pages of 50 with "Show more"; thumbnails download four at a time and
     show as they come, with the size, style and author in a tooltip. A
     click downloads the full image into
     `~/.cache/zendo/covers/chosen/cover/` or `chosen/hero/`, named
     `<id>.<SteamGridDB image id>.<ext>` (a new name each time, as Iced
     keeps images by path), deleting the one chosen before. A chosen image
     comes before any other, Steam's own included, in the library and the
     Store, and the automatic downloads leave the game alone. "Use the
     automatic cover" (or wide image) deletes it and reads the library
     again, which puts Steam's or the store's back or downloads one. With
     no key the picker says so and offers Settings. A later search or a
     switch of game drops the answers and thumbnails still coming for the
     earlier one. Escape closes it, Enter searches.
     Checked: clippy and tests (a chosen image's name read back; the year
     from SteamGridDB's dates; a page of images read; no key; a late
     search answer dropped; the page's keys doing nothing under the
     picker), and the real SteamGridDB with the key in Settings (an
     ignored test: Jet Set Radio Future found, 2002, portrait covers and
     wide heroes, a thumbnail, Portal 2 by Steam id). The user then tried
     it in the window, and it worked.
7. Mods: Nexus, `nxm://`, profiles, back to vanilla. Decided with the
   user on 2026-11-06 [firm]: Amethyst Mod Manager's model (GPL-3.0, its
   README and wiki read for ideas only): a few games known by their Steam
   id or title with where their mods go, any other game taking mods in
   its own folder or one the user sets; mods kept unpacked and copied in
   on Apply, Back to vanilla putting the folder back as it was. Nexus
   through the user's personal API key while Zendo is in testing, which
   Nexus's API policy allows ("testing stage … or personal use only");
   a public release needs an SSO slug from support@nexusmods.com, the
   user's call. First test: Cat Quest (64-bit Mono Unity, owned) with
   BepInEx 5, whose `BepInEx/LogOutput.log` shows it loaded. Split:
   - 7a Mods from files, built 2026-11-06: a Mods card on the page of
     every game with an install folder. "Add mod…" takes an archive
     (zip, 7z, rar, tar) in the desktop's dialog, from `~/Downloads`,
     unpacked with 7-Zip (bsdtar when it is missing); archives holding
     links are refused. Kept in `~/.local/share/zendo/mods/<game id>/`:
     `mods.toml` (the order, on or off, the folder when set),
     `files/<mod>/`, `applied.json`, `backup/`, `overwrite/`. Each mod
     shows its file count and whose files it replaces or loses to
     (later ones win); ↑ ↓ to reorder, Delete asks once. Known games
     [tentative, a start]: Stardew Valley (`Mods`), Skyrim SE and LE,
     Oblivion and Fallout 3 (added 2026-11-07; Steam 22330, 900883, 22300 and 22370), 4 and New Vegas, Starfield (`Data`, a wrapping `Data`
     folder taken off), Cyberpunk 2077 (its folder; wrappers taken off
     until `archive`, `bin`, `r6`, `red4ext`, `engine` or `mods` shows);
     each with its Nexus name for 7b. Mod folder editable per game, not
     while applied.
     Apply first takes out what was applied, then copies each file from
     the last mod on that has it (copies, not links: a game writing a
     file never changes the kept mod; on btrfs `copy_file_range` shares
     the blocks, checked with `filefrag` here), backing up game files
     it replaces and recording each file and folder it placed. For a
     Windows game, paths take the case of what is there (`data/Textures`
     goes into `Data/textures`), as Wine sees them as one; a Linux game
     keeps case apart. A failed Apply takes out what went in. Back to
     vanilla removes the files placed, puts the backups back, and
     removes the folders it made; what the game wrote in those folders,
     and mod files it changed, go to `overwrite/`, applied after every
     mod next time (a config kept), with a Delete for them. The card
     says whether the list is applied, changed since, or not; Apply and
     Back to vanilla wait while the game runs. Not handled yet: files the
     game writes outside folders Zendo made stay; FOMOD installers and
     Bethesda plugin lists (7d).
     Checked: clippy and tests (apply and back to vanilla exact, with a
     log and a changed file kept and applied again; case for Windows and
     Linux games; a failed apply undone; wrappers found for Cyberpunk,
     Skyrim and Stardew; a zip made with 7z added, an archive with a link
     to `/etc/passwd` refused; games known by Steam id and title), and
     BepInEx_win_x64_5.4.23.6.zip from GitHub added, applied to a game
     folder made for the test and taken out to the two files it held
     (an ignored test). The user tried it in the window on Cat Quest with
     BepInEx 5 (2026-11-06): it works.
   - 7b Nexus, built 2026-11-06: a Nexus Mods section in Settings with
     the personal key (checked with `users/validate` before it is saved,
     which says the account's name and whether it has Premium; kept in
     `config.toml` as `nexus_key`), a link to the "API Access" page
     [tentative: the address, `users/myaccount?tab=api`, could not be
     checked, the site turns curl away], and "Open them with Zendo",
     which writes a hidden `zendo-nxm.desktop` for `x-scheme-handler/nxm`
     pointing at the running binary and sets it with `xdg-mime default`;
     only that button changes the desktop. `zendo nxm://…` hands the link
     to the open window over `$XDG_RUNTIME_DIR/zendo/links.sock` (folder
     0700, one window listens, a stale socket is taken over) or opens one.
     A link's game: the game Zendo knows, the one the user chose before
     (`nexus` in `mods.toml`), or the installed game with its title in
     Nexus's game list (`~/.cache/zendo/nexus-games.json`, asked weekly);
     with no single match, a chooser lists the installed games and
     remembers the one chosen. The file downloads into the game's
     `downloads/` with progress and Stop, in the Mods card and the
     Downloads page (not the store queue: these are plain HTTP), then is
     unpacked and deleted. A file Nexus lists as replacing the one a mod
     came from takes that mod's place (order and on or off kept, Apply
     says changed); another file of the same mod is a mod of its own.
     Each mod from Nexus shows its version; once a session Zendo asks
     Nexus for newer files of the game's mods, and Update downloads with
     Premium or opens the file's page for the Mod manager download button
     otherwise. A Find mods on Nexus button opens the game's mods there.
     Three calls per download, one per mod for updates, and one for the
     key at start, with `Application-Name: Zendo`. Collections are
     refused with a message. One change at a time to any game's mods
     across threads (a lock in `mods.rs`).
     Checked: clippy and tests (links read, collections and bad ones
     refused; updates followed to the newest, a loop ended; games found by
     title; the desktop file's quoting; an update taking its mod's place
     and showing as changed, then applied; the socket passing a link, mode
     0700, a second listener refused, a stale socket taken over; the key
     saved only once accepted; a link before the library waits; the
     chooser), `zendo nxm://` refusing a collection and a bad game name,
     and Nexus answering 401 to a made-up key; an ignored test asks the
     real Nexus with the key in Settings. The user tried it with a real
     key and link in the window (2026-11-06): it works.
   - 7c Profiles, built 2026-11-06: each game's mods have profiles, its
     own order and mods on in each, over the same unpacked mods (as Mod
     Organizer's and Amethyst's do). The list as it was becomes
     "Default". A Profile row on the Mods card: a list to switch, New…
     (a copy of the one in use, switched to), Rename… and Delete (asks
     once; the profile in use goes after a switch to the next, the last
     one stays). Names: one line, 40 characters at most, not another's in
     any case. A mod added while another profile is in use comes last and
     off in the others; a mod deleted leaves them all. Switching while
     mods are applied applies the new profile in one go, unless the game
     runs (then the card says Apply puts it in). In `mods.toml`:
     `active_profile` and a `[[profile]]` per other profile (`order`,
     `enabled`). The folder, overwrite and Nexus name stay per game, not
     per profile [tentative].
     Checked: clippy and tests (each profile's order and mods on kept
     across switches, a later mod last and off, names refused, renames,
     a deleted mod leaving every profile, deleting the one in use and
     refusing the last; switching while applied puts the other profile's
     file in, with the game running only the list changes, not applied
     nothing is applied; the name field, Delete asking first and the
     notes). The user tried it in the window (2026-11-06): it works.
   - 7d FOMOD installers, built 2026-11-07: an archive (added or from
     Nexus) with `fomod/ModuleConfig.xml`, any case, up to three folders
     down, opens an installer sheet instead of going straight in. Read
     with `roxmltree` (MIT or Apache-2.0, already in the tree through
     Iced), UTF-8 or UTF-16, element names in any case. Steps, groups and
     options in the order the file says (`Ascending` by default), steps
     shown or hidden by the flags the options chosen before set, groups'
     rules (exactly one and at most one as radios, at least one, all,
     any), option types (required, recommended, not usable, may not work)
     fixed or depending on flags (those of the same step too, as Vortex's
     test installer expects) and files present; conditional installs;
     priorities. The option last touched is described beside the list
     with its picture. Install copies the chosen files and folders,
     sources found whatever their case, into the mod; a destination
     outside the mod is refused. The unpacked archive is kept in
     `installers/<mod>/` with the choices (`installer`, `chosen` in
     `mods.toml`): Options… on the mod runs it again, and a Nexus update
     of the mod runs the new one with them chosen. A file condition sees
     the mod folder (four levels) and the mods on; "inactive" never
     matches until Zendo keeps a plugin list. An installer with nothing
     to show installs at once; mods coming while one is open wait. Not
     handled: C# script installers (`script.cs`), BAIN.
     Checked: clippy and tests (an installer in UTF-16 and UTF-8 read the
     same, orders, flags hiding a step, a choice making an option of the
     same step required, not usable never on, conditional files, paths
     kept inside; a tar with the installer inside a wrapper built, case
     found, Options… with the choices of last time, then applied, an
     installer climbing out refused, Cancel leaving nothing; the sheet's
     steps, a radio clearing, Escape and the next waiting). Five real
     `ModuleConfig.xml` from GitHub (SmoothCam, IntelEngine, Anomaly
     Magazines Redux, S1DedicatedServers, Vortex's test installer) read
     in an ignored test; a test archive made here (two steps, a picture,
     a conditional file in another case) installed through the real
     path. The user tried it in the window with that archive on Cat
     Quest (2026-11-07): it works.
   - 7e Bethesda plugin order, built 2026-11-07 (`plugins.rs`): for
     Fallout 3, New Vegas, Skyrim LE, Skyrim SE and Fallout 4, the Mods
     card lists the plugins (`.esm`, `.esp`, `.esl`) straight in `Data`
     and in the mods on, in load order, each on or off, with ↑ ↓. The
     game's own load first in Bethesda's order and stay there (the main
     master, and for SSE and FO4 every implicit one, always on); then
     masters (`.esm`, `.esl` or flagged in the header), then the rest,
     in the order kept, new ones last. Each plugin's `TES4` header gives
     its masters: one not on, or loading after it, shows in red; one
     that cannot be read says so. The order lives in `mods.toml`
     (`[[plugin]]`, every plugin seen, so a mod turned off and on keeps
     its place) and per profile. A plugin new to the list is on when it
     comes in a mod, and the game's own as the game's `plugins.txt` had
     them before Zendo, or on when it had none [tentative].
     Apply writes the list into the prefix's
     `AppData/Local/<game>/` (Steam's `compatdata/<app>/pfx`, or
     Zendo's prefix for the games it starts), CRLF lines, in each
     game's way: `plugins.txt` of the ones on (FO3, NV), plus
     `loadorder.txt` (Skyrim LE), or every plugin with `*` for on and
     the implicit ones left out (SSE, FO4). For FO3 and NV, which load
     by file date, it also dates the plugins a minute apart from
     2008-01-10, in order [tentative]. `applied.json` keeps the lists as
     they were, the folders it made and the game's own plugins' dates;
     Back to vanilla puts them back. A plugin change shows as changed
     since applied. With no prefix yet (the game never started), the
     card says to start it once and Apply writes no list. Not handled
     [tentative]: the game's launcher changing the list while applied is
     undone by Back to vanilla. Oblivion, Starfield, Creation Club
     plugins and FOMOD's plugin conditions came after (list item 1).
     Checked: clippy and tests (headers with flags and masters, each
     game's list format, the order with the game's own first and red
     masters, moves only among a kind; on a scratch Fallout 3: Apply
     writes `plugins.txt` and dates the plugins in order, a move shows
     as changed and back as applied, Back to vanilla removes the list
     and the folders it made and puts the dates back, a list the game
     had decides its plugins' first state and comes back as it was;
     profiles keep their own order). The real Fallout 3 GOTY's six
     plugins read with their masters (an ignored test, read only). Steam
     installed them dated with `Fallout3.esm` third, so the game's own
     order matters there. The user tried it in the window on Fallout 3
     GOTY (2026-11-07): it works.

## After phase 7: the list [firm order]

The user's list of 2026-11-07, in the order proposed the same day; the
user confirmed the order, and the findings noted in the items (DLC
skipped today, what the keyring gains, Nexus downloads for free
accounts, Steam's install link, Quiver's licence), the same day [firm].
Each item is planned in detail when it starts.

Quick and fresh first:

1. Phase 7's gaps, built 2026-11-07. Checked against LOOT's
   libloadorder (GPL-3.0, read for facts only: folders, file names,
   plugin lists), which showed more than the three listed:
   - FOMOD's file conditions follow the plugin list: a plugin there and
     on is active, there and off inactive (before, inactive never
     matched and an off plugin counted as active).
   - Oblivion known (Steam 22330 and 900883, `oblivion` on Nexus): its
     records' 20-byte header, `Oblivion.esm` its only plugin first, the
     list by file date in `AppData/Local/Oblivion`, or in the game's
     folder when `Oblivion.ini` says `bUseMyGamesDirectory=0`.
   - Starfield's plugin order: twelve plugins always loaded (from
     libloadorder's list), the asterisk list, its light flag (`0x100`).
   - Creation Club plugins always load after the game's own, from
     `Skyrim.ccc` or `Fallout4.ccc` in the game's folder, or for
     Starfield `Starfield.ccc` in `Documents/My Games/Starfield` before
     the game folder's; New Vegas loads a plugin a `.nam` file names.
   - The list is `Plugins.txt` as the games read it; one already there
     keeps its spelling (launchers write `plugins.txt`), so no second
     list appears beside it on Linux, where case counts.
   - Each store's copy has its own `AppData/Local` folder: Skyrim SE's
     `… GOG` (`Galaxy64.dll` in the game) and `… EPIC`
     (`EOSSDK-Win64-Shipping.dll`), `Fallout4 EPIC`, `FalloutNV_Epic`
     (`EOSSDK-Win32-Shipping.dll`).
   Not handled [tentative]: Oblivion Remastered (Steam 2623190, another
   layout); Fallout 4's and Starfield's `sTestFile` settings, which make
   those games ignore the list; Enderal. Checked: clippy and tests
   (Oblivion's header, Starfield's light flag, each store's folder,
   Oblivion's ini, CCC files and Starfield's own winning even empty,
   `.nam` files, CCC plugins out of the asterisk list, an existing
   `plugins.txt` keeping its spelling, FOMOD's three states); the real
   Fallout 3's plugins read again. The user tried it on Fallout 3
   (2026-11-07): it works.
2. A review of the Rust code with the `rust-review` skill, then fixing
   what it finds, before more code is built on the same patterns.
   Reviewed and fixed 2026-11-07; the user chose to fix findings 1–10
   and leave 11:
   1. Zendo's own files lost a change when two writers overlapped (the
      window's threads, a `zendo hide` from a keybind). Each change to
      `state.json`, `local.toml` and `games.toml` now holds a file lock
      (`<file>.lock`, `File::lock`), and the mods' lock is a file lock
      too (`$XDG_DATA_HOME/zendo/mods.lock`), so two Zendos wait for
      each other.
   2. Writes did not reach the disk before the rename: `applied.json`,
      made new on each Apply, could be empty after a crash (btrfs keeps
      a rename that replaces a file, not one that makes a new one). Each
      write now syncs the file and its folder, and removes the
      temporary when it fails.
   3. A stalled download hung and Stop did nothing: downloads now give
      up when no bytes come for a minute.
   4. A panic in background work closed the window: it now ends as that
      work's error.
   5. `Config`'s debug output showed the API keys: written by hand now,
      it shows `…`.
   6. The download loop, URL decoding and HTTP agents were copied in
      several files: one `http.rs` now, with one user agent.
   7. `paths.rs` had no tests and `safe_name("..")` gave `..`: names
      of dots alone become underscores; tests for names, writes, private
      writes and the lock.
   8. Lint exceptions got their reasons; `allow_attributes` and
      `allow_attributes_without_reason` are on, so new ones need one.
   9. FOMOD's choices are a `Selection` type, which answers for places
      its installer lacks instead of panicking.
   10. `mods.rs` (3,177 lines) split into `mods/`: `known`, `add`,
       `order`, `apply`, `files` and `tests`.
   To fix, later [firm]: 11, a game's process group id could be
   reused by another program after the game ends, and Stop would then
   signal it. The minimum Rust is 1.98 [firm]: no older than the
   systems Zendo is for, checked 2026-11-07 (Arch 1.99.1, Fedora 44
   1.99.1, Fedora 45 1.98.1; Fedora updates Rust in its released
   versions); `File::lock` needs 1.89. Clippy then asked for
   `Duration::from_mins` and `from_hours` where they read better.
   Checked: fmt, clippy (pedantic) and the tests, new ones for each
   fix.
3. API keys (SteamGridDB, Nexus) in the system keyring (Secret Service:
   gnome-keyring runs here, KWallet is installed), the 0600 file as
   fallback. What it gains: the keys encrypted at rest with the login
   password (backups, a copied dotfiles folder, a stolen disk). What it
   does not: any program running as the user can still ask an unlocked
   keyring, as it can read the file.
   Built 2026-11-07 [tentative until the user tries it], with the
   `secret-service` crate (5.2, MIT or Apache-2.0, on the zbus 5 Iced
   already brings; its 6.0 is a release candidate):
   - Each key is an item with `application = zendo`, `key = steamgriddb`
     or `nexus`, in the default collection; the session to the keyring
     is encrypted (Diffie-Hellman), as libsecret's is.
   - Reading the settings with the keys (the window's start, `zendo
     covers`) asks the keyring, and its unlock prompt when locked. A key
     found in `config.toml` moves into the keyring then and out of the
     file. Launching, Proton and installing read the file alone and
     never ask the keyring.
   - Saving writes a key to the keyring only when it changed, and never
     prompts: it runs on the window's thread. A locked or missing
     keyring keeps the key in the file (0600), and the next start
     moves it. A key removed while the keyring is away is written `""`
     in the file, so the keyring's old copy is not brought back, and is
     removed from the keyring once it answers.
   - Settings says under each key where it is kept, or why the keyring
     could not be read.
   Checked: tests with a keyring in memory (moving in, no keyring, a
   locked one, a key removed while away, other settings leaving a key
   alone), and the real Secret Service code in a throwaway
   `gnome-keyring-daemon` on a private D-Bus, never the user's keyring:
   keeps, replaces, removes, and a locked keyring refuses without a
   prompt. Not checked: KDE Wallet, and the unlock prompt (it needs
   the user's desktop).
4. Diagnostics: `zendo logs` and `zendo report`, and a Settings section
   with Copy report (tools and engines found, versions, Protons,
   prefixes, recent launch logs), as Hikari has. It helps with every
   real-world check after.
   Built 2026-11-07 [tentative until the user tries it]:
   - `report.rs` gathers the report as text to paste: Zendo's version,
     the system, desktop and each GPU's driver and PCI ids; Steam and
     Heroic found; legendary, gogdl, nile, umu-run, gamescope, mangohud,
     gamemoderun, 7-Zip, bsdtar, xdg-utils with their path and version
     (`--version` where it is safe, five seconds at most, colours taken
     out); the Protons and the default; the settings, each key only
     "set" and where it is kept; each store signed in or not, from the
     engine's files (nile's status for Amazon), never the account; the
     library's counts per source and its problems; the Wine prefixes of
     the Windows games Zendo starts, made or not; and the last 40 lines
     of the last three launch logs. The home folder shows as `~`, and a
     key's value is hidden should it be anywhere in the text.
   - `zendo report` prints it and changes nothing: it reads the keys
     as they are (`Config::load_as_is`), without moving one from the
     file into the keyring. `zendo logs` lists the launch logs, latest
     first (when, size, id, title); `zendo logs <id>` prints one.
   - Settings' Diagnostics section: Copy report (gathered on a thread,
     then on the clipboard, with how many lines) and Open launch logs.
   Not in it [tentative]: mods (what each game has on and applied),
   and Vulkan details (`vulkaninfo` is not always installed). Checked:
   clippy and tests (dates, the home folder and keys hidden, a log's
   last lines, colours out, versions asked, the copied note); the
   report run here, without D-Bus so the keys stayed where they are,
   and checked to hold no key and no home path. Not checked: the
   section on screen, as opening the window would move the keys
   before the user's own test.

Groundwork before more screens:

5. Languages: English and Portuguese (Portugal) to start. Early,
   because every screen built after adds text; the window has some 130
   `text("…")` calls and many `format!` messages to move now.
   Decided by the user 2026-11-07 [firm]: Fluent (`.ftl` files built
   into the binary, `fluent-bundle`); the window's own words first,
   the core's error messages (about 150, with paths and tool output)
   later, and the command line stays English for scripts; the language
   follows the system's (`sys-locale`, already in the build), with a
   Settings choice: System, English, Português (Portugal).
   Built 2026-11-07: `crates/zendo/src/i18n.rs` and `i18n/en.ftl`,
   `i18n/pt-PT.ftl`; every page, the footer, the sheets, the file
   dialog and the sign-in window speak the chosen language, saved as
   `language` in `config.toml` (`en` or `pt-PT`; none follows the
   system). Tests check both files parse, have the same messages with
   the same values, format cleanly, and that every `t!` names a message
   and every message is used. Not checked on screen yet (opening the
   window would move the keys before the user's own test).
   Left as they are [tentative]: sizes and speeds keep the decimal point
   and MiB/GiB (`Size`'s Display lives in the core); the core's error
   messages, store and tool names stay English; Brazilian Portuguese
   gets Portugal's.
6. Themes: a setting in Settings for Zendo's own theme or hyprmonk's
   (read from `tone.json`, Zendo's own when it is missing), built so
   more themes can be added. With it, a note in the hyprmonk project
   (the user asked for it) saying what `tone.json` should hold and when
   to write it.
   Built 2026-11-07:
   - Settings' Theme section lists Zendo (the default [firm], decided by
     the user the same day: Zendo may run on any desktop, not only
     hyprmonk's), the user's themes, and hyprmonk's tone, offered only
     when hyprmonk's state directory is there or it is already chosen.
     Saved as `theme` in `config.toml`.
   - Themes are files (asked by the user the same day): the built-in
     ones in `crates/zendo/themes/` (`zendo.toml`, built into the binary;
     a new one is a file there and a line in `BUILT_IN`), the user's in
     `~/.config/zendo/themes/` (one named as a built-in replaces it),
     read at start and each time Settings opens. TOML with name, light,
     background, foreground, accent, optional positive and negative, and
     optional greys (sidebar, surface, raised, line, dim), mixed from the
     background and text when left out (`zendo-core/src/theme.rs`). A
     chosen file that is gone shows Zendo's own and says so; a file that
     is not a theme is named in the section.
   - hyprmonk's tone: `zendo-core/src/tone.rs` reads
     `~/.local/state/hyprmonk/theme/tone.json` (version 1: name, label,
     light, background, foreground, accent, optional positive and
     negative) and makes it a theme of the same shape; `app/themes.rs`
     looks at its modification time every 2 seconds and repaints. Zendo's
     own colours while the file is missing. The note is hyprmonk's
     `docs/zendo-tone.md` (written, not committed there); hyprmonk does
     not write the file yet.
   - `style.rs` keeps one palette for the window, made from the chosen
     theme. Tests check Noir mixes within a shade of Zendo's own, and
     that `zendo.toml` gives Zendo's own exactly.
   Checked on screen in a throwaway home (no keyring, no real config)
   with hand-written tones for Noir, Harbor (light) and Kanso, each
   painting the window and a change repainting it, and a `dusk.toml`
   theme chosen in its `config.toml`.
7. An Arch package (PKGBUILD for the AUR: binary, `.desktop` file, icon).
   It also gives the `nxm://` handler a fixed path instead of a build
   folder.
   Built 2026-11-07, as the user chose: `zendo-git` (the latest commit on
   main; a release package `zendo` once releases are tagged) in
   `packaging/arch/zendo-git/` with its `.SRCINFO`. Fetches with
   `--locked`, builds `--frozen --release`, runs the tests in `check()`,
   and installs `/usr/bin/zendo`, `data/zendo.desktop` (no `MimeType`:
   the `nxm://` handler stays the Settings button's alone [firm]) and
   `data/zendo.svg`, an ensō drawn here [tentative, the user may replace
   it]. Depends on what the binary links (GTK 3, WebKitGTK 4.1, libsoup
   3, GLib, udev) and what Iced loads (Wayland, xkbcommon, the Vulkan
   loader); the stores' tools, umu-launcher, gamescope, MangoHud,
   GameMode and 7-Zip are optional. The handler gets the fixed path by
   itself: the button writes the running program's path, `/usr/bin/zendo`
   once installed; Settings now tells a handler left pointing at another
   copy of Zendo (a build folder's) and offers the button again. Not
   done: publishing it on the AUR (the user's account), and a release
   tag.

Features:

8. A survey of Heroic (first) and Hikari for settings and automations
   Zendo lacks; what it finds joins this list.
9. The game page made clearer and friendlier, with DLC on it: which are
   owned and installed, and install or uninstall each. Today Zendo
   installs Epic and GOG games with `--skip-dlcs` (`downloads.rs`), so no
   DLC comes with them, and Heroic's DLC is left out of the library.
   Amazon (nile) is to check.
10. EAC and BattlEye runtimes for the Windows games Zendo starts (Proton
    reads them from `PROTON_EAC_RUNTIME` and `PROTON_BATTLEYE_RUNTIME`);
    how umu gets them is to check.
11. Steam: the games owned but not installed (Steam's Web API needs the
    user's own key and SteamID; what local files hold is to check), and
    installing them through Steam (`steam://install/<app>` opens Steam's
    install dialog).
12. Searching Nexus Mods in Zendo instead of the browser. Nexus's v1 API
    has no search; its GraphQL v2 API is to check. Downloading still
    needs the site's Mod manager download for accounts without Premium:
    the API gives free accounts a link only with the key in an `nxm://`
    link.

Later:

13. Recompiled games (N64 recomps and the like) and their mods, with
    ideas from Quiver Launcher. Checked 2026-11-07: Quiver installs and
    runs apps from GitHub and GitLab releases with community catalogs
    (recomps ship that way); TypeScript; Business Source License 1.1 from
    version 4 (MIT after a year, version 3 and before MIT): ideas only,
    no code.
14. A Fedora package (a native RPM, as the user prefers, through COPR
    [tentative]), after the Arch one.
15. OptiScaler, last: niche.

## On hyprmonk's side

- Install it (package list or the installer) and bind a key (Super+G).
- The tone: Zendo reads `~/.local/state/hyprmonk/theme/tone.json`; what it
  holds and when to write it is in hyprmonk's `docs/zendo-tone.md`. hyprmonk
  does not write it yet, so Zendo shows its own colours.
- hyprmonk's roadmap points here; this file is the plan.

## Next session

- The list after phase 7 (above), confirmed 2026-11-07. Item 1 built
  and checked by the user the same day. Item 2, the Rust review, fixed
  the same day (findings 1–10; 11 to fix later [firm]). Item 3, the
  API keys in the keyring, item 4, diagnostics, item 5, languages, and
  item 6, themes, and item 7, the AUR package, built the same day; next,
  item 8, the survey of Heroic and Hikari.
- To try when the user is back home (item 3): start Zendo, which moves
  both keys from `config.toml` into the keyring; Settings should say
  "Kept in the system keyring" under each; `grep -c "_key"
  ~/.config/zendo/config.toml` should print 0, and `secret-tool search
  --all application zendo | grep "^label"` list both items (without
  printing the keys); covers and Nexus should still work. Then item 3
  is [firm]. Item 4 the same visit: Settings' Diagnostics section, Copy
  report and paste it somewhere to read; Open launch logs. Item 5 the
  same visit: Settings' Language section, choose Português (Portugal)
  and go through each page (Library, a game, Store, Downloads,
  Accounts, Settings, About, launch options, Change art, Add game, the
  mods page) for text that is cut off, still English or reads wrong;
  then System, and English again. Item 6 the same visit: Settings' Theme
  section should offer Zendo and hyprmonk (this computer has hyprmonk);
  choosing hyprmonk should say it has written no tone, and keep the
  colours (the same until hyprmonk writes the file).
- Phase 7 is done. 7e, Bethesda plugin order, built 2026-11-07; the
  user tried it on Fallout 3 GOTY (Steam 22370) the same day: it works.
  Before that, Fallout 3 was made
  known (Steam ids 22300 and 22370, `Data`, `fallout3`) and games are
  matched on Nexus by the part of the title before " - " or ":" too;
  the user confirmed the Nexus button shows.

## Conventions

As in hyprmonk: commit on main, no attribution lines, commit only when asked.
