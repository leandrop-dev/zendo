<div align="center">

# Zendo

A game launcher for Linux: every game in one library, started with the
right tools.

[![License: MIT](https://img.shields.io/badge/license-MIT-f25623?style=flat-square)](LICENSE)
![Linux](https://img.shields.io/badge/Linux-171717?style=flat-square&logo=linux&logoColor=ccd0cf)
![Rust](https://img.shields.io/badge/Rust-Iced-171717?style=flat-square&logo=rust&logoColor=ccd0cf)

</div>

Zendo gathers the games on this machine into one library: Steam's, the ones
Heroic installed from Epic, GOG and Amazon, and the ones with a `.desktop`
file. A sidebar leads to the Library, Settings and About; each game has a
page with its art, Play, and what its store records about the install. It
opens in a normal window and goes full-screen for the couch, and it is
driven as well by a controller as by the keyboard.

It is early: phases 1 to 3 of the [Roadmap](#roadmap), a library that reads
the launchers' files without changing them and starts games through the
launcher that owns them. Its layout follows
[Hikari](https://github.com/dodo986/hikari)'s. A sister
project of [hyprmonk](https://github.com/leandrop-dev/hyprmonk), which will
install it and give it its colours, but it runs on any Linux desktop.

## Contents

- [What it does](#what-it-does)
- [What you need](#what-you-need)
- [Install](#install)
- [Build](#build)
- [Keys](#keys)
- [The zendo command](#the-zendo-command)
- [Settings and files](#settings-and-files)
- [Layout](#layout)
- [Roadmap](#roadmap)
- [Credits](#credits)

## What it does

- Speaks English or Portuguese (Portugal): the system's language, or the
  one chosen in Settings.
- Its own colours on any desktop, a theme of your own, or, under KDE Plasma,
  the colour scheme in use (Settings → Theme → System), which it follows as
  it changes (read from `~/.config/kdeglobals`).
- Finds your games without changing anything of the launchers':
  - **Steam**: every library folder in `libraryfolders.vdf`, native or
    Flatpak, and the installed games in them (Proton and Steam's runtimes
    left out). Covers come from Steam's own cache, so they show without a key
    or a download.
  - **Heroic**: what it installed from Epic (legendary), GOG (gogdl) and
    Amazon (nile), and the games added to it by hand.
  - **`.desktop` files** in the Game category, except the launchers
    themselves and the shortcuts Steam and Heroic make for games already
    listed.
  - **Games you add yourself** with "Add game": a Windows `.exe`, run
    through umu-launcher and Proton in a prefix of its own, or a Linux
    program, chosen in the desktop's file dialog, with a title and
    arguments. Edit and Remove are on its page; Remove leaves its files.
- Starts a game the way its launcher would: Steam games through Steam,
  Heroic's through Heroic (so its settings for the game apply), the others
  from their `.desktop` file.
- Shows a cover for every game it can: Steam's from Steam's cache, the
  portrait images Epic, GOG and Amazon publish (the ones Heroic shows),
  and from [SteamGridDB](https://www.steamgriddb.com) for the rest when you
  give it a key. Downloads happen in the background and are kept.
- Lets you pick any game's cover or wide image from SteamGridDB with
  "Change art" on its page: search by title, choose among the game's
  images, or go back to the automatic one.
- Sorts by what you played lately, what you played most, or by title, from
  the launchers' own records (Steam's and Heroic's play times) and the games
  Zendo started.
- Filters as you type, anywhere in the window, and by store with the chips
  above the grid.
- Pins the games you want first, in every order.
- Hides what is not a game, or what you do not want to see, until you ask
  for it back.
- Gives each game a page: a wide image (Steam's own, the one the store
  publishes, or SteamGridDB's), the developer and play time, one bar of
  actions (Play, Launch options, Pin, and More for the rest), what the
  game is about, its DLC and mods beside a short panel of details, and a
  Technical section, folded, with the exact launch command.
- Takes the SteamGridDB key in Settings, checks it with SteamGridDB, and
  keeps it in the system keyring (GNOME Keyring, KDE Wallet), or in a
  settings file only you can read when there is none.
- Signs in to Epic, GOG and Amazon on the Accounts page, with accounts of
  its own (not Heroic's): it opens the store's login page in a window of
  its own and, once you have signed in there, takes it from there through
  legendary, gogdl or nile, with nothing to copy. The window keeps no
  cookies. If it cannot open, the page opens in your browser instead and
  you paste back what it ends on. The engines are used from your `PATH`, or from
  Heroic's own copies.
- Shows every game those stores say you own on the Store page, with the
  installed ones marked, a search, store chips, "Not installed only", an
  order (A–Z, recently played or most played) and a page for each.
- Lists the Steam games you own there too, installed or not, with the time
  Steam counted (when last played, the last two weeks, and on which
  systems), and the games your Steam family shares with you, once you
  sign in with Steam on the Accounts page. You sign in on Steam's own page,
  in Zendo's sign-in window, with your password and Steam Guard or the
  Steam app's QR code; Zendo never sees the password. It keeps the Web API
  token Steam's page gives in the system keyring, and gets a new one from
  Steam's page, in a window that never shows, when it runs out (Steam's
  page remembers the sign-in in `~/.local/share/zendo/steam-web`). The
  Steam Web API key Zendo took before is no longer used: Zendo removes it
  from the settings file and the keyring. A Steam game's page has the
  description, makers, genres and release date from Steam's store, and
  its download size and size on disk as the Steam client counts them (from
  its `appcache/appinfo.vdf`, read only; the store's "Storage" estimate
  when the client has not seen the game). Install
  opens Steam's own install dialog (`steam -silent steam://install/<app>`);
  Steam downloads the game, Zendo shows how far it is on the Downloads page
  and the game's page, and adds it to the library when Steam is done. If
  you close Steam's dialog, press Stop waiting (Zendo cannot tell). The
  Store page shows the space a game needs, from its store requirements.
  An installed Steam game's page has Verify with Steam and Uninstall with
  Steam, which Steam asks and carries out; Zendo follows a verify through
  Steam's content log and says when it is done. Downloads, installs and
  uninstalls done in Steam show in Zendo too; pause and cancel downloads
  in Steam. A Steam that is already running may raise its own window.
- Installs, updates, repairs and uninstalls those games on the Downloads
  page, one at a time, with the engine's progress, speed and time left.
  Pause keeps what was downloaded and Resume carries on; Cancel stops and
  deletes a half-done install. Games go into a folder of their own in
  `~/Games` (or `install_dir`), and join the library when done. Closing
  the window while something downloads asks first, then pauses the queue
  for next time. A game's page shows its download size, size on disk and
  latest version, and for the games Zendo installed whether an update is
  waiting.
- Updates those games by themselves when an update is found (turned off
  in Settings, Downloads). An update, repair, move or uninstall of a game
  that is running waits on the Downloads page until it is closed.
  Settings also chooses how many files Epic's and GOG's engines download
  at once; Amazon's always downloads six.
- Moves a game Zendo installed to another folder (More, then Move…, in
  the desktop's folder dialog): renamed at once on the same drive, copied
  with its progress on the Downloads page to another drive, then the old
  folder deleted. Stopping or a failure leaves the game where it was. The
  store's record, applied mods and GOG's setup (run again at the next
  start) follow it.
- Imports an Epic, GOG or Amazon game already on disk (Import from a
  folder… on its Store page): the store's engine checks the files, and
  Zendo then updates, repairs and starts it as one it installed. A GOG
  game keeps its language, and gets the list of files gogdl compares
  against, so its first update downloads only what changed. Nothing in
  the folder is changed.
- Installs DLC: the Store asks whether to install a game's DLC with it,
  and the DLC card on an Epic or GOG game's page installs or uninstalls
  each, or all. Heroic's games show their DLC; Heroic installs those.
  Amazon's engine, nile, has no DLC.
- Shows GOG's changelog on a GOG game's page, when the developer wrote
  one: the newest update first, earlier ones on asking.
- Starts the games it installed through their engine, Windows games with
  Proton through umu-launcher, each in a Wine prefix of its own
  (`<install_dir>/Prefixes/<title>`), with each launch's output kept in a
  log. While a game Zendo started runs, Play turns into Running with a
  Stop beside it, which asks the game's windows to close, as their X
  button would, so it can save first; pressed again, Stop ends the game
  at once. Steam and
  Heroic games get the same once their launcher has started them, found
  by their install folder.
- Gives each game it starts itself (the ones it installed or you added,
  `.desktop` ones) launch options on its page: the Proton build, MangoHud, GameMode,
  gamescope with full screen and a size, NVIDIA offload on a computer
  with two cards, arguments and environment; and for Windows games
  Proton's switches: controllers as Xbox pads for games that ignore
  others, Wine's Wayland driver, HDR, Wine's FSR with its sharpening,
  DXVK's FPS counter, a frame cap, WoW64, and esync and fsync off. A
  switch the chosen Proton build does not read says so (CachyOS's
  Proton has no `PROTON_NO_ESYNC`). A command can run before the game
  (which waits for it, and does not start if it fails) and after it
  ends, with `ZENDO_GAME_ID`, `ZENDO_GAME_TITLE`, `ZENDO_GAME_FOLDER` and
  the game's `WINEPREFIX`. Epic and GOG games can run another program in
  their folder instead of the store's, and start another way their store
  lists: GOG's play tasks (Cyberpunk 2077's game without its launcher)
  and Epic's extra launch options (Rocket League without its
  anti-cheat). gamescope can show as a window, a frameless window or
  full screen, with its size and the game's, how it fits and scales
  (FSR, NIS and the rest, with their sharpening), the game's frames a
  second and any more of its options. A Windows game can run without
  umu: Zendo then runs Proton in Steam Linux Runtime 4.0 itself, as umu
  would. A native game can run in one of Steam's Linux runtimes (scout,
  soldier, sniper, 4.0, whichever Steam installed), and an Epic game in
  another language (legendary's `--language`), offline (for the games
  Epic approves; the sheet says when it has not), or while its update
  waits (legendary otherwise refuses an out-of-date game), with or
  without Epic's EOS overlay, which Settings installs once through
  legendary. The Windows games installed from Epic and GOG keep their
  cloud saves in sync, as Steam's do: before the game starts and after
  it ends, unless their launch options turn it off, or by hand (Sync
  now, or Upload or Download to replace one side), through legendary's
  `sync-saves` and gogdl's `save-sync`. The game's page says whether
  they are in sync, when they last synced, or what went wrong. The time
  played in a GOG game Zendo starts goes to the GOG account as the game
  ends, as Heroic sends it (a session that cannot be sent waits for the
  next; Accounts' GOG card turns it off). A GOG game's page shows GOG's
  total for it, from every client that sends GOG its sessions, where it
  is more than the launchers' count, as Steam's games show Steam's. Steam and Heroic games keep their launcher's own settings.
- Downloads GE-Proton from Settings: the latest releases, and older ones
  on asking, checked against their checksums, into Steam's compatibility
  tools folder, where Steam and Heroic find them too; removes the ones
  there; and sets the Proton games run with by default.
- Gives each Windows game it starts a Wine prefix card on its page:
  Wine's settings (winecfg), winetricks' own window (with GE-Proton or
  UMU-Proton, whose protonfixes bring it), winetricks components
  installed without questions (`vcrun2022 d3dx9`; one already there is
  said and skipped), a Windows program run in the prefix (an installer
  or a patch, `.exe` or `.msi`), and the prefix's folder. Each runs through
  umu-run with the game's Proton and environment, one at a time and not
  while the game runs, and logs to
  `~/.local/state/zendo/logs/<id>.prefix.log`.
- Sets up a Windows game installed from GOG before its first start, as
  Heroic does: the redistributables GOG lists for it (Visual C++, DirectX,
  PhysX…) downloaded once through gogdl into
  `~/.local/share/zendo/gog-redist` and installed quietly into its prefix,
  then the game's own install steps (GOG's script interpreter), which
  write registry keys some games need. The status line says each step; a
  step that fails is said and does not stop the game. The Wine prefix
  card's GOG's setup runs it again; the log is
  `~/.local/state/zendo/logs/<id>.gog-setup.log`.
- Applies the known fixes Heroic keeps for Windows games installed from
  Epic, GOG and Amazon (the winetricks components a game needs, and
  programs from its folder run in its prefix) before the game starts,
  each once per prefix; the list is downloaded from Heroic's repository
  at most once a day, never shipped. The Wine prefix card lists a game's
  fixes with Heroic's notes and applies them again.
- Starts the GOG games it installed with GOG Galaxy, as Heroic does:
  comet (from its GitHub releases, or Heroic's copy) runs beside the game and answers it as
  the GOG account signed in, for achievements, the game's own sign-ins
  (a CD Projekt RED account linked to GOG, say) and the DLC it checks;
  a Windows game's prefix gets Galaxy's stand-in service once (Zendo downloads it from comet's release, `dummy-service.zip`, the first time). comet
  reads Zendo's GOG tokens through a link, never from its command line;
  its log is `~/.local/state/zendo/logs/<id>.comet.log`. Accounts' GOG
  card turns it off.
- Gives the Windows games it starts the Easy Anti-Cheat and BattlEye
  runtimes they need to play online under Proton: Steam's when Steam has
  them, else its own, downloaded from the list Lutris keeps the first time
  a game that ships one starts (Settings shows them, updates and removes
  its own).
- Mods, on each game's page: add archives (zip, 7z, rar), turn them on
  and off, order them (later ones win a file both have, and the card
  says which), then Apply copies them into the game. Back to vanilla
  takes out exactly what went in and puts back the files mods replaced;
  what the game wrote meanwhile is kept for next time. Zendo knows where
  mods go for Stardew Valley, Oblivion, Skyrim, Fallout 3, 4 and New Vegas,
  Starfield and Cyberpunk 2077; for any other game they go in its folder, or one
  you set. Profiles keep different sets of mods on and in order for the
  same game; switching while mods are applied puts the other set in.
  Mods with a FOMOD installer ask their options in steps, with their
  descriptions and pictures, and Options… asks them again later. For
  Bethesda's games, the card also lists the plugins in load order, each on or off, and says when one needs a
  master that is off or loads after it; Apply writes the order where the
  game reads it, in its Proton prefix.
- Mods from [Nexus Mods](https://www.nexusmods.com), with your personal
  API key in Settings: once Zendo opens `nxm://` links (one button in
  Settings), a Mod manager download on Nexus goes straight into the
  game's mods, with progress on its page and the Downloads page. Zendo
  tells when a mod has a newer file; Update downloads it with Premium,
  or opens its page for the button there. Collections are not supported
  yet.
- Searches a game's mods on Nexus from its Mods card (Search Nexus): by
  name, most endorsed, most downloaded or recently updated, with their
  pictures, and the mods and files you have marked. A mod's files
  download into the game's mods with Premium; without it, Get on Nexus
  opens the file's page in a Nexus window of Zendo's (signed in once; it
  keeps the site's cookies in `~/.local/share/zendo/nexus-web`), and
  what Slow download gives there, the mod manager link or the file, goes
  into the game's mods with no copy left behind. An archive Nexus named,
  added by hand with Add mod, is known as that mod's file too. Adult
  mods are left out unless you switch them on there.
- Shows a game's [ProtonDB](https://www.protondb.com) rating on its page,
  Platinum to Borked, or Native for a game with a Linux version on Steam,
  and opens its ProtonDB page when clicked. Games
  from other stores are matched to Steam through umu's database or their
  exact title.
- Copies a report for bug reports from Settings (or `zendo report`): the
  system, the tools and store engines with their versions, the Protons
  and anti-cheat runtimes, the settings without the keys, whether each
  store is signed in without
  the account, the library's counts and problems, the Wine prefixes and
  the end of the last three launches' logs, with your home folder as `~`.

## What you need

Nothing beyond Zendo: it reads what is there and leaves out the rest. Each
launcher is needed only for its own games:

| Tool | For |
|------|-----|
| Steam | Steam games; they start through the Steam client |
| Heroic | The Epic, GOG and Amazon games it installed, and games added to it |
| legendary, gogdl, nile, comet | Signing in to Epic, GOG and Amazon, listing your games there and installing them, and GOG Galaxy. From each project's GitHub releases ([legendary](https://github.com/derrod/legendary), [gogdl](https://github.com/Heroic-Games-Launcher/heroic-gogdl), [nile](https://github.com/imLinguin/nile), [comet](https://github.com/imLinguin/comet)); Heroic's own copies are used when these are not installed alone |
| WebKitGTK 4.1 (`webkit2gtk-4.1`) | The sign-in window; Zendo is built against it, so it is needed to build and run |
| umu-launcher | Starting the Windows games Zendo installed; it uses the newest GE-Proton or other Proton in Steam's compatibility tools folders, or downloads UMU-Proton (another can be chosen per game). A game set to run without umu needs Steam Linux Runtime 4.0 instead (Steam's, or the copy umu keeps) |
| MangoHud, GameMode, gamescope | Only for a game whose launch options turn them on |
| 7-Zip (`7zip`), or bsdtar | Unpacking mods |
| xdg-utils | Opening `nxm://` links from Nexus Mods in Zendo (`xdg-mime`) |
| A keyring (GNOME Keyring, KDE Wallet) | Keeping the API keys encrypted; without one they stay in `config.toml`, readable by you only |

## Install

On Arch and its relatives, `zendo-git` builds the latest commit on main;
its PKGBUILD is in `packaging/arch/zendo-git/`:

```bash
git clone https://github.com/leandrop-dev/zendo.git
cd zendo/packaging/arch/zendo-git
makepkg -si
```

It installs `zendo` in `/usr/bin`, its menu entry and its icon. Steam,
Heroic, umu-launcher, gamescope, MangoHud, GameMode and 7-Zip are optional;
pacman lists what each one adds.

## Build

Rust 1.98 or newer, and WebKitGTK 4.1 with GTK 3 (on Arch, `webkit2gtk-4.1`;
on Debian and Ubuntu, `libwebkit2gtk-4.1-dev`; on Fedora, `gtk3-devel
webkit2gtk4.1-devel libsoup3-devel javascriptcoregtk4.1-devel
systemd-devel`, checked on Fedora 45). The repository's `mise.toml` pins
Rust 1.99.1 for [mise](https://mise.jdx.dev) users (`mise install`):

```bash
git clone https://github.com/leandrop-dev/zendo.git
cd zendo
cargo build --release
./target/release/zendo
```

To run it from a build folder with a menu entry, copy `data/zendo.desktop`
to `~/.local/share/applications/` and `data/zendo.svg` to
`~/.local/share/icons/hicolor/scalable/apps/`, with `Exec=` set to the
built program.

## Keys

In the library:

| Key | Controller | Does |
|-----|------------|------|
| Arrows, Page Up/Down, Home/End | D-pad, left stick | Move through the grid |
| Enter, a click | A | Open the game's page |
| Typing, Ctrl+F | | Search by title |
| Backspace | | Take back a letter |
| Esc | B | Clear the search |
| Tab | Y | Sort: recent, most played, A–Z |
| Ctrl+P | | Pin the game, or unpin it |
| Delete | X | Hide the game, or bring a hidden one back |
| Ctrl+H | Start | Show or leave out the hidden games |
| F5, Ctrl+R | | Look for games again |

The Store page moves, opens, searches and clears the same way. On a store
game's page Enter installs it (or opens it when installed), and Esc goes
back to the Store. When closing asks about a running download, Enter
closes anyway and Esc keeps downloading.

On a game's page:

| Key | Controller | Does |
|-----|------------|------|
| Enter | A | Play |
| Esc | B | Back to the library |
| Ctrl+P | Y | Pin or unpin |
| Delete | X | Hide or bring back |

Anywhere: Ctrl+1 opens the Library, Ctrl+2 the Store, Ctrl+3 Downloads,
Ctrl+4 Accounts, Ctrl+5 or Ctrl+, Settings, Ctrl+6 or F1 About, and F11 or Select switches
between full-screen and windowed. On Settings, typing starts the
SteamGridDB key; on Accounts, while signing in, typing or Ctrl+V fills in
the code. On a Steam game's Store page, Enter opens Steam's install
dialog.

The controller is read only while Zendo's window is in front, so it does not
move through the library while you play.

## The zendo command

```
zendo                        open the library
zendo --fullscreen           open it full-screen
zendo list [--all] [--json]  print the games in the window's order: id, source, title;
                             --all adds the hidden ones
zendo launch <id>            start a game by the id `list` prints
zendo hide <id>              leave a game out of the window
zendo unhide <id>            bring it back
zendo pin <id>               put a game first in every order
zendo unpin <id>             stop
zendo categorize <id> <category>
                             put a game in a category, made if new
zendo uncategorize <id> <category>
                             take it out of one
zendo covers                 download the covers and wide images missing, saying what it found
zendo nxm://…                download a mod from a Nexus Mods link, in the open window or a new one
zendo logs                   list the games' last launch logs, latest first: when, size, id, title
zendo logs <id>              print a game's last launch log
zendo report                 print what Zendo found, to paste into a bug report; it changes nothing
```

`list` also names any launcher file it found but could not read.

## Settings and files

`~/.config/zendo/config.toml`, all optional. The Settings page writes it,
readable by you only, and does not keep comments added by hand:

```toml
# A key from https://www.steamgriddb.com/profile/preferences, for covers of
# games whose store has none (.desktop games, Steam games Steam has no cover
# for).
steamgriddb_key = "…"
# Your personal key from API Access on Nexus Mods, for downloading mods.
nexus_key = "…"
# The Steam sign-in, written by Accounts when no keyring answers; it is
# kept in the keyring otherwise. Not for writing by hand.
steam_session = "…"
# false to never download covers.
download_covers = true
# false to keep the time played in GOG games Zendo starts from GOG (Accounts'
# GOG card sets it).
gog_playtime = true
# false to start GOG games without GOG Galaxy (comet); Accounts' GOG card
# sets it.
gog_galaxy = true
# Whether Nexus searches show mods marked adult.
nexus_adult = false
# Where store games are installed, each in a folder of its own.
install_dir = "~/Games"
# The Proton Windows games run with unless their launch options choose one
# (a folder name; Settings sets it). Left out: the newest GE-Proton found.
default_proton = "GE-Proton11-12-x86_64"
# The window's language: "en" or "pt-PT". Left out: the system's.
language = "pt-PT"
# The window's colours: "zendo", a theme file's name, or "system" to follow
# the KDE colour scheme in use (Zendo's own where there is none). Left
# out: "zendo".
theme = "system"
```

Your own themes are TOML files in `~/.config/zendo/themes/`, one per
theme, chosen by the file's name; Settings lists them, reading the folder
each time it opens. A file named like a built-in one (`zendo.toml`)
replaces it. Only the background, text and accent are needed; the greys
are mixed from the first two when left out:

```toml
# ~/.config/zendo/themes/dusk.toml
name = "Dusk"
light = false               # true: dark text on a light background
background = "#1a1b26"
foreground = "#c0caf5"      # the text
accent = "#e0af68"
positive = "#9ece6a"        # optional green; Zendo's own when left out
negative = "#f7768e"        # optional red
# Optional greys: sidebar, surface (cards, inputs), raised (under the
# pointer), line (rims) and dim (quieter text).
```

The built-in themes are the files in `crates/zendo/themes/`.

The keys and the Steam sign-in go in the system keyring when one answers
(the Secret Service: GNOME Keyring, KDE Wallet), and in this file only
when none does; Settings (Accounts for Steam's) says where each is. A key
written in the file by hand moves into the keyring the next time Zendo
starts. In the keyring they are items with `application` `zendo` and `key`
`steamgriddb`, `nexus` or `steam-session`, so
`secret-tool lookup application zendo key nexus` prints one. A key written
`""` means none, without asking the keyring.

`~/.config/zendo/local.toml` holds the games added by hand; the
Library's "Add game" and their pages' Edit and Remove write it:

```toml
[[game]]
id = "local:cave-story"         # from the first title; stays when it changes
title = "Cave Story"
program = "/home/you/Games/Cave Story/Doukutsu.exe"
arguments = "-fullscreen"
```

`~/.config/zendo/games.toml` holds each game's launch options, as set on
its page (readable by you only, as an environment can hold a token):

```toml
["gog:1299228602"]
proton = "GE-Proton10-4"        # a folder name; the default when left out
mangohud = true
gamemode = true
gamescope = true
gamescope_fullscreen = true
gamescope_width = 1920          # with a height; the game's own size when left out
gamescope_height = 1080
gamescope_borderless = true     # -b, when not full screen
gamescope_game_width = 1280     # -w and -h: the size the game draws at
gamescope_game_height = 720
gamescope_scaler = "fit"        # -S: auto, integer, fit, fill, stretch
gamescope_filter = "fsr"        # -F: linear, nearest, fsr, nis, pixel, sgsr
gamescope_sharpness = 5         # 0 sharpest to 20, for fsr, nis and sgsr
gamescope_fps = 60              # -r
gamescope_more = "--adaptive-sync"
no_umu = true                   # Proton in Steam Linux Runtime 4.0, without umu
steam_runtime = "scout"         # native games: scout, soldier, sniper, steamrt4
language = "fr"                 # Epic games: legendary's --language
offline = true                  # Epic games: --offline
skip_update_check = true        # Epic games: --skip-version-check
eos_overlay = true              # Epic games: the EOS overlay on in the prefix
cloud_saves = false             # Epic and GOG games: no saves' sync (on when unset)
xbox_controllers = true         # PROTON_PREFER_SDL=1
wayland = true                  # PROTON_ENABLE_WAYLAND=1
hdr = true                      # PROTON_ENABLE_HDR=1 DXVK_HDR=1; gamescope --hdr-enabled
fsr = true                      # WINE_FULLSCREEN_FSR=1
fsr_strength = 3                # 0 sharpest to 5; Wine's 2 when left out
wow64 = true                    # PROTON_USE_WOW64=1
no_sync = true                  # PROTON_NO_ESYNC=1 PROTON_NO_FSYNC=1
fps_counter = true              # DXVK_HUD=fps
frame_cap = 60                  # DXVK_FRAME_RATE and VKD3D_FRAME_RATE
nvidia_offload = true           # __NV_PRIME_RENDER_OFFLOAD=1 and the rest
arguments = "-windowed"
environment = "DXVK_HUD=fps"    # after the switches', so it wins
executable = "bin/x64/Cyberpunk2077.exe"   # Epic and GOG: in the game's folder
task = "bin\\x64\\Cyberpunk2077.exe" # GOG: a play task's file; Epic: an option's name
before = "notify-send 'Starting'"          # sh -c; the game waits for it
after = "~/bin/back-up-saves"              # once the game has ended
```

What Zendo keeps itself:

| Path | What |
|------|------|
| `~/.local/state/zendo/state.json` | The games it started and when, the hidden and pinned games, the sort, the categories and their games |
| `~/.cache/zendo/covers/` | Downloaded covers (`<id>.jpg`) and wide images (`<id>.hero.jpg`); `misses.json` lists games with none, asked about again after a week; `chosen/cover/` and `chosen/hero/` hold the ones picked with Change art, which come first |
| `~/.local/share/zendo/stores/` | Zendo's sign-ins, one folder per store engine (`epic`, `gog`, `amazon`), readable by you only, with each engine's record of what Zendo installed (`installed.json`; for GOG, Zendo writes it) |
| `~/.local/state/zendo/downloads.json` | What is left in the download queue, there only while something is |
| `~/.cache/zendo/store/` | Each store's list of your games (`<store>.json`), shown at once on the next start; `gogdb/` keeps GOG's answers about each game |
| `~/.local/state/zendo/logs/` | The output of each game's last launch (`<id>.log`) |
| `~/.cache/zendo/umu.json` | umu-launcher's id for each game, which picks the fixes it applies |
| `~/.local/share/Steam/compatibilitytools.d/` | GE-Proton builds downloaded from Settings (Zendo does not own the folder; Steam reads it) |
| `~/.local/share/zendo/runtimes/` | The Easy Anti-Cheat and BattlEye runtimes Zendo downloaded (`eac_runtime/`, `battleye_runtime/`), each with its build date in `current_version` |
| `~/.cache/zendo/protondb.json` | Each game's Steam app id and ProtonDB rating, asked again after a week |
| `~/.local/share/zendo/mods/<id>/` | A game's mods: `mods.toml` (order, on or off, the Nexus file each came from, mod folder, the game's name on Nexus when you chose it, the plugin order for Bethesda's games, the profile in use and the others' order, mods and plugins on), `files/` (each mod unpacked, or built by its installer), `installers/` (archives with a FOMOD installer, kept to choose again), `downloads/` (files from Nexus while they download), `applied.json` (what Apply placed, and the plugin lists and dates it changed), `backup/` (game files mods replaced), `overwrite/` (what the game wrote while modded) |
| `~/.cache/zendo/nexus-games.json` | The games Nexus Mods has, to match yours by title, asked again after a week |
| `~/.local/share/applications/zendo-nxm.desktop` | Written by Settings' "Open them with Zendo", so `nxm://` links open in Zendo; hidden from menus |
| `$XDG_RUNTIME_DIR/zendo/links.sock` | Where `zendo nxm://…` hands a link to the open window |
| `*.lock` beside `config.toml`, `state.json`, `local.toml` and `games.toml`; `~/.local/share/zendo/mods.lock` | Empty files locked while a change is written, so two writers wait for each other |

Play time of games Zendo starts itself (the ones it installed or you
added, `.desktop` ones) is counted until the game's last process ends; Steam and Heroic
count their own.

## Layout

| Path | What |
|------|------|
| `crates/zendo-core/` | The library: `steam.rs`, `heroic.rs`, `desktop.rs` find games, `local.rs` keeps the ones added by hand, `mods.rs` and `mods/` keep mods and apply them, `fomod.rs` reads FOMOD installers, `plugins.rs` reads Bethesda plugins and writes their order, `nexus.rs` asks Nexus Mods (`nexus_search.rs` searches it), `launch.rs` starts them (with `windows.rs` to ask their windows to close, `proton.rs` and `umu.rs` for Windows games, `launch_options.rs` for each game's options, `store_tasks.rs` for the other ways a store lists to start a game, `steam_runtime.rs` for Steam's Linux runtimes, `eos_overlay.rs` for Epic's overlay, `cloud_saves.rs` for Epic's and GOG's cloud saves, `gog_playtime.rs` for the time played sent to GOG and its total, `prefix_tools.rs` for the tools run in a game's Wine prefix, `gog_setup.rs` for GOG's setup of a game's prefix, `known_fixes.rs` for Heroic's known fixes, `galaxy.rs` for GOG Galaxy through comet) and stops them, `protondb.rs` asks ProtonDB, `ge_proton.rs` downloads GE-Proton, `runtimes.rs` the anti-cheat runtimes, `http.rs` makes the web requests and downloads, `secrets.rs` keeps the API keys in the keyring, `report.rs` makes the diagnostics report, `covers.rs` downloads covers and keeps the ones picked, `steamgriddb.rs` asks SteamGridDB, `theme.rs` reads theme files, `kde.rs` reads the KDE colour scheme (`ini.rs` reads the groups of its file and of `.desktop` ones), `state.rs` and `config.rs` are Zendo's own files, `stores.rs` signs in through the store engines, `owned.rs` lists the games they own (Steam's through its Web API, with the sign-in from `steam_web.rs`, which also lists the family's), `info.rs` asks their sizes, updates and the DLC owned, and `downloads.rs` installs them and their DLC |
| `crates/zendo-core/tests/fixtures/` | Sample launcher files the tests read |
| `crates/zendo/` | The `zendo` command (`main.rs`), the window's words (`i18n.rs`, with `i18n/en.ftl` and `i18n/pt-PT.ftl`), the controller (`gamepad.rs`), the file dialog (`file_dialog.rs`: programs, mods and installers), `nxm://` links handed to the open window (`links.rs`) and the window: `app.rs` holds its state and keys, `app/` a module per page (`library.rs`, `game.rs`, `store.rs`, `downloads.rs`, `accounts.rs`, `settings.rs`, `about.rs`), `running.rs` for the games running, `themes.rs` for the theme (the built-in ones are in `themes/`) and the KDE colour scheme, `launch_options.rs` for a game's launch options sheet (gamescope's part in `launch_options/gamescope.rs`), `cloud_saves.rs` for the game page's cloud saves line, `gog_playtime.rs` for GOG's total played time on a game's page, `prefix.rs` for its Wine prefix card, `local.rs` for the Add game sheet, `artwork.rs` for the art picker, `dlc.rs` for a game's DLC card, `mods.rs` for its Mods card, `nexus.rs` for Nexus (its Settings section, links, downloads, updates), `nexus_search.rs` for its search sheet, `installer.rs` for the FOMOD installer sheet, `protondb.rs` for its rating pill, `protons.rs` for Settings' Proton section, `runtimes.rs` for its Anti-cheat section, `eos_overlay.rs` for its Epic's overlay section, `steam.rs` for Accounts' Steam card and the sign-in's token, `steam_installs.rs` for following Steam's installs, `store_game.rs` for a store game's page, the sign-in window (`sign_in_window.rs`, Steam's too, and its hidden renewal), the `sidebar.rs` and the colours (`style.rs`) |
| `data/` | The menu entry (`zendo.desktop`) and the icon (`zendo.svg`) |
| `packaging/arch/zendo-git/` | The AUR package: `PKGBUILD` and `.SRCINFO` |
| `PLAN.md` | What Zendo is for, the direction, the stack and where it stands |
| `docs/backlog.md` | The work left, in order |
| `docs/history.md` | What was built, when, and what was checked |
| `docs/reference.md` | Verified facts about the stores, the engines and their files |

## Roadmap

1. **Library, read-only** (done): Steam, Heroic and `.desktop` games, the
   cover grid and launching.
2. **Covers and order** (done): covers from the stores and SteamGridDB,
   sorting by recent and most played, hiding.
3. **The shell** (done): Hikari's layout in hyprmonk's colours: the
   sidebar, the Library with store and sort chips and pins, a page per
   game, Settings with the SteamGridDB key, About.
4. **Accounts, Store and Downloads** (built, tried with real GOG and Amazon
   accounts): sign in to Epic, GOG and Amazon, the games owned but not
   installed, and install, update, repair and uninstall through legendary,
   gogdl and nile, with pause and resume, update status and sizes, and a
   sign-in window of Zendo's own, so there is no code to copy.
5. **Launch options** (built): Proton through umu-launcher with a prefix
   per game, a launch log, Running and Stop, a launch options sheet with
   MangoHud, GameMode and gamescope, the ProtonDB rating on the game's
   page, and GE-Proton downloads.
6. **Local games** (built): add a game by hand, and pick any game's
   cover or wide image from SteamGridDB.
7. **Mods** (built): mods from archives with
   Apply and Back to vanilla, from Nexus Mods links with updates,
   profiles, FOMOD installers and Bethesda plugin order.

See [PLAN.md](PLAN.md) for the details.

## Credits

Ideas, no code, from:

- [omarchy-games-menu](https://github.com/akitaonrails/omarchy-games-menu)
  (MIT): the grid of covers, typing anywhere to filter, frecency.
- [Hikari](https://github.com/dodo986/hikari) (GPL-3.0): the model for
  Zendo's pages and what they do: the sidebar, the library's chips and pins,
  the game page; driving the store engines and reading the launchers'
  libraries without writing to them.
- [Heroic](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher)
  (GPL-3.0): where it keeps its installed games, and its `heroic://launch`
  links.
- [omakade](https://github.com/btsouth/omakade) (GPL-3.0-or-later): the
  look Zendo's redesign follows (a top bar instead of a sidebar, a
  translucent window, monospace type, Home and a controller-first Game
  Mode, Organize, play sessions and stats). Ideas and look only; no code or
  assets are used (`docs/design.md`).

Built on [Iced](https://iced.rs) (MIT), [gilrs](https://gitlab.com/gilrs-project/gilrs)
(MIT or Apache-2.0), [keyvalues-parser](https://codeberg.org/CosmicHarper/vdf-rs)
(MIT or Apache-2.0), [ureq](https://github.com/algesten/ureq) (MIT or
Apache-2.0), [toml](https://github.com/toml-rs/toml) (MIT or Apache-2.0),
[Fluent](https://projectfluent.org) through fluent-bundle and unic-langid
(MIT or Apache-2.0) and [sys-locale](https://github.com/1Password/sys-locale)
(MIT or Apache-2.0).
Covers of games no store has art for come from
[SteamGridDB](https://www.steamgriddb.com) when you give Zendo a key.

## License

MIT, see [LICENSE](LICENSE).
