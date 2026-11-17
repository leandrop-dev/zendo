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

Standalone releases, no Heroic needed (installed on Fedora 45 on 2026-11-12
through mise's `github:` backend; each answered `--version` on `PATH`):
legendary 0.21.1 (`derrod/legendary`; dnf has only 0.20.34), gogdl 1.4.0
(`Heroic-Games-Launcher/heroic-gogdl`), nile 1.2.0 (`imLinguin/nile`), comet
0.4.1 (`imLinguin/comet`) and umu-launcher 1.4.4 (`Open-Wine-Components/umu-launcher`,
the zipapp; Fedora RPMs stop at fc44). comet's release also has
`dummy-service.zip` (36 KB, three files at its top level:
`GalaxyCommunication.exe`, an `MZ` program, `install-dummy-service.bat` and
`update-permissions.exe`; checked 2026-11-12); Zendo downloads it into `$XDG_DATA_HOME/zendo/galaxy/`
the first time a GOG Windows game needs it (`galaxy.rs`).

Release assets Zendo downloads (`gh api repos/.../releases/latest`, checked
2026-11-12; x86_64 and aarch64 names). Every asset has a `digest`
(`sha256:<hex>`) in the API, though no release has a checksum file:

- legendary 0.21.1: `legendary_linux_x64`, `legendary_linux_arm64` (0.20.34
  had a bare `legendary`)
- gogdl v1.4.0: `gogdl_linux_x86_64`, `gogdl_linux_arm64`
- nile v1.2.0: `nile_linux_x86_64`, `nile_linux_arm64`
- comet v0.4.1: `comet-x86_64-unknown-linux-gnu`,
  `comet-aarch64-unknown-linux-gnu`, and `dummy-service.zip`
- umu-launcher 1.4.4: `umu-launcher-1.4.4-zipapp.tar`, holding `umu/umu-run`
  (a python3 zipapp, `#!/usr/bin/env python3`) and `umu/umu_run.py` linking to
  it
- Proton-CachyOS `cachyos-11.0-20261015-slr`:
  `proton-cachyos-<version>-slr-{x86_64,x86_64_v3,arm64}.tar.xz` (~340 MB) and
  a `.sha512sum` each; the tarball's top folder is
  `proton-cachyos-<version>-slr-x86_64`; the `x86_64_v3` one unpacks to
  `proton-cachyos-<version>-slr-x86_64_v3` (checked 2026-11-12)

Zendo keeps its copies in `$XDG_DATA_HOME/zendo/engines/<engine>/` (`gogdl/gogdl`,
`umu/umu-run`, with a `release` file holding the tag).

What phase 1 reads from Heroic, checked in Heroic 2.22.4's own code: under
`~/.config/heroic` (or Flatpak's `~/.var/app/com.heroicgameslauncher.hgl/config/heroic`),
`legendaryConfig/legendary/installed.json`, `gog_store/installed.json` with
titles from `store_cache/gog_library.json`, `nile_config/nile/installed.json`
with titles from its `library.json`, and `sideload_apps/library.json`. Games
start with `heroic://launch?appName=<id>&runner=legendary|gog|nile|sideload`.
Heroic's protocol handler (`src/backend/protocol.ts` on main, blob b4fa1c1, read
through `gh api` on 2026-11-12) switches on the URL's host and knows only `ping`
and `launch`: there is no route that opens a game's page, so Zendo offers no
"Manage in Heroic".

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
the categories with the games in each, and `achievements`: game id to
`[unlocked, total]`, kept since item 32 for the library's progress line),
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

Covers are kept at the size the store serves them. The old Iced window decoded each to
full-size RGBA (the Amazon one is about 6 MB in memory), which is fine for a
few dozen games; for large libraries, scale them down when saving [re-evaluate
once someone has a few hundred games].


## Steam's `appdetails`, as Zendo reads it (checked 2026-11-13)

`https://store.steampowered.com/api/appdetails?appids=<id>&l=english`, no
key. The reply is `{"<id>": {"success": bool, "data": {...}}}`.

- An unknown, removed or tool app: `{"<id>":{"success":false}}`, no `data`
  (checked: 480, 1, 999999999).
- Read: `about_the_game` (else `short_description`; HTML, made plain),
  `developers`, `publishers`, `release_date.date`, `genres[].description`,
  `platforms`, `pc_requirements` (`[]` when none), `categories[].description`
  (features: "Single-player", "Co-op"), `controller_support` (`"full"`; absent
  on some games), `metacritic` (`score`, `url`; absent for Palworld and Black
  Myth: Wukong) and `screenshots[]` (`path_thumbnail`, a 600x338 image, and
  `path_full`, 1920x1080; addresses carry a `?t=` query).
- Kept in `$XDG_CACHE_HOME/zendo/steam_facts.json` (`{app: {version, asked,
  details}}`, a week) and `.../zendo/screenshots/` (the first 8 thumbnails,
  named after the address). `protondb.json` entries also carry `chosen`, the
  app id the user gave the game. The user's ids are in `state.json` as
  `steam_apps`.

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

## New sources: Battle.net, Nexus Launcher, Xbox streaming (checked 2026-11-16)

Read from omakade's source on `main` (blob `8a9ed1d`): `src/sources/battlenet/`,
`src/sources/xodus/`, `src/launch/GameLauncher.cpp`, and `docs/GUIDE.md` and
`docs/COMPATIBILITY.md`. Formats and commands are facts; no code is copied
(omakade is GPL-3.0-or-later, ideas only, the same rule as everywhere else).

### Battle.net

- The Battle.net Agent keeps its install database at
  `<prefix>/drive_c/ProgramData/Battle.net/Agent/product.db`, where `<prefix>`
  is a Wine, Proton or Bottles prefix. It is a protobuf-like wire format, not
  SQLite: top level repeats a product record in field 1; the record has `uid`
  in field 1, the product code in field 2, user settings in field 3 (field 1 is
  the Windows install path) and cached product state in field 4 (field 1 is the
  base state, field 1 installed and field 2 playable). omakade caps the read at
  16 MB, skips the tool products `agent` and `bna`, and keeps a product only
  when `installed` or `playable` and its install path maps into the prefix
  (through `dosdevices/<drive>:`; `c:` falls back to `drive_c`).
- The product code is the same as Battle.net's own code for most titles
  (`wow`, `wow_classic`, `pro`/`prometheus`, `d3`, `fenris`, `anbs`, `hsb`,
  `hero`, `d2`, `d2LOD`, `osi`, `w3`, `w3tft`, `s1`, `s2`, `sca`, `codhq`, …).
  The launch code differs for a few: `wow` → `WoW`, `pro`/`prometheus` → `Pro`,
  `fenris` → `Fen`, `d2`/`d2LOD` → `Diablo II`, `w3` → `W3`, `w3ROC`/`w3tft` →
  `Warcraft III`. omakade keeps a fixed product → (title, launch code, Lutris
  slug) table; a code it does not know is titled from the code (underscores to
  spaces, first letter upper) and launched with the code as is.
- Game identity is `product@<first 8 hex of sha256(clean(prefix))>`, so the same
  product installed in two prefixes is two games.
- Prefixes scanned: `~/.wine`, `$XDG_DATA_HOME/wineprefixes/*`,
  `~/.local/share/wineprefixes/*`, `$XDG_DATA_HOME/bottles/bottles/*`, the
  Bottles Flatpak's `~/.var/app/com.usebottles.bottles/data/bottles/bottles/*`,
  `~/Games/*`, and every `steamapps/compatdata/<app>/pfx` under Steam's
  libraries. Only a prefix whose `product.db` exists is kept.
- Runner: `bottle.yml` at the prefix root → Bottles; `pfx` with a sibling
  `version` file, or `version` in the prefix root → Proton; else Wine. Flatpak
  is noted when the path is under `~/.var/app/com.usebottles.bottles/` or
  `~/.var/app/com.valvesoftware.Steam/`.
- **The client on stock Wine**, checked 2026-11-17: with wine 11.22 in a
  plain prefix the client dies at its loading screen (`wine: Unhandled
  exception 0x80000003 ... starting debugger`) and the Agent never writes a
  product, so no game appears. It is run under `umu-run` with a Proton
  instead (Zendo's own prefix always is; item 58c).
- The client is `drive_c/Program Files (x86)/Battle.net/Battle.net.exe` (or
  `Program Files`, `Battle.net Launcher.exe`). A game starts as
  `<client> --exec=launch <launch code>`:
  - Wine: `wine <client> --exec=launch <code>`, with `WINEPREFIX=<prefix>`.
  - Proton: `env WINEPREFIX=<prefix> GAMEID=umu-battlenet STORE=battlenet
    PROTONPATH=GE-Proton PROTON_VERB=run umu-run <client> --exec=launch <code>`
    (omakade uses `GAMEID=0` and `STEAM_COMPAT_DATA_PATH` for a Steam-style
    prefix; `umu-run` and `wine` are both on this machine).
  - Bottles: `bottles-cli run -b <bottle> -e <client> -- --exec=launch <code>`;
    the bottle name is `Name:` in `bottle.yml`, else the prefix folder's name.
    Flatpak Bottles: `flatpak run --command=bottles-cli
    com.usebottles.bottles run …`.
- Last played comes from `<prefix>/drive_c/users/<account>/AppData/Roaming/
  Battle.net/Battle.net.config` (JSON): `Games.<product>.LastPlayed`, a Windows
  FILETIME (100-ns since 1601), milliseconds or seconds. There is no total
  playtime.
- Artwork: omakade downloads Lutris's public `https://lutris.net/games/
  cover/<slug>.jpg` and `banner/<slug>.jpg`, and prefers artwork the game's
  own folder carries (`cover.png`, `library_600x900.jpg`,
  `library_hero.png`, `header.jpg`, …). Zendo could use SteamGridDB instead,
  as it does for other sources.
- Installer, checked 2026-11-16:
  `https://us.battle.net/download/getInstallerForGame?os=win&locale=enUS&
  version=LIVE&gameProgram=BATTLENET_APP` redirects (301, then 302) to
  `https://downloader.battle.net/download/installer/win/1.0.69/
  Battle.net-Setup.exe` (`application/x-msdos-program`, HTTP 200). The version
  in the path moves; no checksum is published, only HTTPS.

### Nexus Launcher (Microsoft Store, PC Game Pass)

- The project is `domsvoboda/nexus-launcher` (MIT; renamed from `xodus-nexus`
  on 2026-11-09; built on the Xbox team's Xodus). The latest release checked is
  v0.5.0: `Xodus-Nexus-0.5.0-x86_64.AppImage` (~131 MB; 130930811 bytes, digest
  `sha256:9a28294adb20345dcc5673e6085b44824d5fce105060e822867ce9d375a633ff`)
  and `Xodus-Nexus-0.5.0-arm64.AppImage` (132107015 bytes), each with a
  `.sha512sum`; GitHub's API gives each asset a `sha256` digest, checked
  2026-11-17. The AppImage's runtime answers `--appimage-version` on CachyOS
  (FUSE works). The launcher installs its own backend first and its runtime
  family from Settings; it needs glibc 2.38 or newer.
- Profile: `$XDG_CONFIG_HOME/xodus-nexus` (or `~/.config/xodus-nexus`), with
  `launcher-state.json`. The `games` array's records use `id` (16 hex),
  `title`, `publisher`, `storeId` (12 `[0-9A-Z]`), `path`, `installState`
  (`"ready"` for a playable install), `executable`, an inline `coverDataUrl`
  data URL, `heroUrl` and an `antiCheat` object. A game is imported only when
  its state is `ready` and its path is absolute.
- Starting one: `<profile>/shortcuts/launch-game <id>`; the launcher itself is
  `<profile>/shortcuts/launch-nexus` (no argument). Both are scripts Nexus
  writes, marked `# X-Xodus-Nexus-Profile=` on line 2 and private to the user;
  omakade refuses anything else in that place. Its backend CLI is below.
- **The backend CLI**, checked on the user's machine 2026-11-17 (v0.2.15, at
  the `binaryPath` of `current.json`): `library --json [--market US]` answers
  the account's entitlements (`account`, `checkedAt`, and per entitlement
  `productId` (12 `[0-9A-Z]`, the Store id), `status`, `endDate`, `isTrial`,
  `sharingSource`, `satisfiedByProductId`);
  `resolve-package` resolves Store packages without signing in; `streaming
  <source> <destination>` downloads and extracts a game, `--progress-json`
  over stdout and pause/resume JSON over stdin (`--control-stdin`), with
  `--parallel` and `--download-limit-kib`; `download` fetches msixvc/xsp
  files; `extract`, `license`, `saved-accounts`, `account`, `profile`,
  `achievements`, `game-activity`, `run` and `social` read or start other
  things. The `productId` is the Microsoft Store id (the art query's
  `bigIds`).
- Artwork: `https://displaycatalog.mp.microsoft.com/v7.0/products?bigIds=
  <storeId>&market=US&languages=en-US`, keeping `Poster` images at `w=600` and
  `SuperHeroArt` (else `TitledHeroArt`) at `w=1920` from
  `store-images.s-microsoft.com`. The Xbox title id is `<TitleId>` in the
  game's `MicrosoftGame.config`.
- It is a separate GUI with a command-line backend, not an engine with its own
  licence or store API: Zendo reads the games it installed and starts them
  through its script, as it does for Steam and Heroic, and can go further on
  the CLI (item 60): list the account's Game Pass entitlements, and install
  through `streaming` with the queue's progress and Stop.

### Xbox Series S (streaming)

- **Greenlight** (`unknownskl/greenlight`, MIT, TypeScript; the README read
  here is on the default branch `main-v2`, v2.4.2, 2026-11-17; `master` is the
  v1 line): an open client for xCloud *and* Xbox home streaming, on Flathub as
  `io.github.unknownskl.greenlight`, and as GitHub releases holding
  `Greenlight-2.4.2.AppImage` (130720574 bytes, digest
  `sha256:1ceb56c4b3b348480565f784083e3a4222ad782dd9cb9a0aa8934df070331777`),
  a `.deb`, a `.flatpak`, a Windows `.exe`/`.zip` and a macOS `.dmg`, each with
  a digest; the AppImage's runtime answers `--appimage-version` on CachyOS. It
  is an Electron wrapper around `xbox-xcloud-player` (TypeScript). Its
  README's
  optional arguments, checked 2026-11-17: `--fullscreen`, and
  `--connect=<value>`, which "will start stream once the user is
  authenticated", "For console use `F000000000000000` format and for xCloud use
  `xcloud_<title>`", i.e. a console's own 16-hex id. It streams the console's
  own screen; it does not expose the console's installed games as a list, so it
  is a streaming client, not a library source. Zendo opens it without
  `--connect`: the console id is Greenlight's to know, and its window carries
  the list (item 59).
- **xbPlay** was a closed client (Flathub `net.studio08.xbplay`); Flathub
  archived that repository as the app is end-of-life, and the project's public
  history carries a copyright dispute (`XBplay-OSC`, `Xbplay-probe`). Not a
  candidate to depend on; Greenlight is the open one.
- Cloud gaming through Xodus/Nexus is the same mechanism as the Microsoft
  Store source above (item 58), not a separate one.
