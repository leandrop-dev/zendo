# Zendo: verified reference

Facts checked on this machine against the real tools and files, with the
date each was checked. Read the part you need before changing anything that
drives a store engine, reads a launcher's files or talks to a store's API.
The stack decision itself is in ../PLAN.md.

## Tools, and where they come from

The tools, each called instead of written again:

- `steam` (`steam://rungameid/ID`)
- legendary (Epic), gogdl (GOG) and nile (Amazon), Heroic's store engines
- umu-launcher, mangohud, gamemoderun and gamescope

Check where one is packaged (Arch repos, CachyOS repos, AUR) and its licence
before depending on it. All are run as separate programs, never linked, so
their GPL does not reach Zendo's MIT. Checked 2026-11-05:

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

## Zendo's own files, covers and stores

Zendo's own files [tentative]: `~/.config/zendo/config.toml` for what the
user sets (`steamgriddb_key`, `download_covers`), written by the Settings
page with mode 0600 because it holds the key (comments added by hand are not
kept; Hikari uses the system keyring first, which Zendo could add later
[tentative]); `~/.local/state/zendo/state.json`
for what Zendo remembers (launches, hidden and pinned games, the sort, and
the categories with the games in each),
written by the window and the command alike; and `~/.cache/zendo/covers/`
with `<id>.<ext>` covers, `<id>.hero.<ext>` wide images and a
`misses.json` (keys `<id>` and `<id>#hero`) so a game without one is asked
about again only after a week. One state file, not omarchy-games-menu's
split by owner, since both sides write the same few things; each write holds
`<file>.lock`, reads the file as it is now, changes it and renames it into
place. `config.toml` is written the same way, since a save there must not
lose a hand edit or another writer's change.

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
- Epic's save folder is `CloudSaveFolder` in legendary's metadata
  (`metadata/<app name>.json` in Zendo's Epic folder); its variables are
  `{AppData}` (Local AppData), `{InstallDir}`, `{UserDir}` (Documents),
  `{UserSavedGames}`, `{EpicID}` and `{UserProfile}`. legendary 0.21.1
  resolves all of them in a Wine prefix except `{UserProfile}`, which it
  leaves unprocessed and then skips the game (Cat Quest II), so Zendo
  works that folder out itself and passes `sync-saves --save-path`.

Covers are kept at the size the store serves them. Iced decodes each to
full-size RGBA (the Amazon one is about 6 MB in memory), which is fine for a
few dozen games; for large libraries, scale them down when saving [re-evaluate
once someone has a few hundred games].


## Ideas from other projects (ideas only, no code copied)

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
- [Playnite](https://github.com/JosefNemec/Playnite) (MIT, C# and WPF,
  Windows 10 and 11 only; 10.63 is the latest release, and version 11 is
  being rewritten in a private repository with no stated platforms), read
  2026-11-07 from its README, its site and the Steam integration in
  [PlayniteExtensions](https://github.com/JosefNemec/PlayniteExtensions)
  (MIT). Its manual needs a browser and was not read. MIT allows more than
  ideas, but it is C#, so ideas it is. What it confirmed for Zendo: Steam
  games started with `steam -silent steam://rungameid/<id>` (and
  `steam://launch/<id>/Dialog` for Steam's launch menu, a setting);
  installs through `steam://install/<id>` with no progress, only a look
  every 10 seconds for the game to be installed (Zendo now shows
  progress); uninstalls through `steam://uninstall/<id>` with a look every
  5 seconds; play time by watching the processes in the install folder (as
  Zendo's 5b does); owned games from `IPlayerService/GetOwnedGames` with the
  user's own key for a private account, or a web sign-in's access token
  (which also gives `IClientCommService/GetClientAppList` while Steam runs,
  family sharing, and `IPlayerService/ClientGetLastPlayedTimes`); retries on
  Steam's 429. Ideas for Zendo, not on the list yet [tentative, for the
  user to place]:
  The user agreed with these 2026-11-07; placed as items 24 to 27, their
  order [tentative] (24 dropped 2026-11-09, the rest now 24 to 26). Uninstall through Steam went into item 11. Dropped by
  the user 2026-11-08: scripts on game events and on Zendo's start (the
  per-game script in item 16 stays), saved filter presets and the random
  game picker.
  Left out: .NET plugins and PowerShell extensions, a portable mode.
- [steam-tui](https://github.com/dmadisetti/steam-tui) (MIT, Rust, a
  terminal client for `steamcmd`; last change 2026-03-12, its author says
  the project is done), read 2026-11-08. It lists the games owned through
  `steamcmd`'s `licenses_print` and `app_info_print` (after a `steamcmd`
  sign-in), installs with `app_update`, reads `app_status` for the folder
  and size, and starts games with `app_run` or their executable. Little
  for Zendo, which leaves installs to the Steam client; one lead [to
  check]: Steam's own `appcache/appinfo.vdf` (binary, version 29 here,
  4.9 MB) holds the app info `app_info_print` prints, which should include
  each depot's size, a download size for Steam games without asking
  anyone. A parser for its binary format is needed.
  Checked 2026-11-09 (a throwaway script, the file read only): version
  29, keys by index into a string table at the offset the header gives;
  each app's `depots.<id>.manifests.public` has `size` (installed bytes)
  and `download`, a number or a string. Leaving out depots from another
  app (`depotfromapp`, `sharedinstall`: redistributables), optional DLC
  (`config.optionaldlc`), other systems (`config.oslist`), other
  languages and low violence, 748 of the user's 749 Steam games in the
  Store's list have a size (MECCHA CHAMELEON 3.65 GB, 3.43 GB to
  download); the cache holds what the Steam client has seen, so a
  computer with Steam signed in to that account. The Store's Steam games
  take their size from it, before the store requirements' "Storage"
  estimate [firm, the user 2026-11-09]. Built 2026-11-09
  (`zendo-core/src/appinfo.rs`, versions 28 and 29): read, never written,
  each time the Steam list is loaded or refreshed. Over the throwaway
  check it also leaves out DLC depots (`dlcappid`), takes the Linux
  depots when the app has a released one (Steam installs the native
  version then; some list a Linux depot with no public build) and the
  64-bit ones where both are (`osarch` a number or text); a `download`
  of 0 (old apps) counts as the size. All 749 games in the user's list
  have a size. The game page shows Download size and Size on disk; the
  Install with Steam note says what it needs.
