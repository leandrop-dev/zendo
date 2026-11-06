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
- [Build](#build)
- [Keys](#keys)
- [The zendo command](#the-zendo-command)
- [Settings and files](#settings-and-files)
- [Layout](#layout)
- [Roadmap](#roadmap)
- [Credits](#credits)

## What it does

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
- Starts a game the way its launcher would: Steam games through Steam,
  Heroic's through Heroic (so its settings for the game apply), the others
  from their `.desktop` file.
- Shows a cover for every game it can: Steam's from Steam's cache, the
  portrait images Epic, GOG and Amazon publish (the ones Heroic shows),
  and from [SteamGridDB](https://www.steamgriddb.com) for the rest when you
  give it a key. Downloads happen in the background and are kept.
- Sorts by what you played lately, what you played most, or by title, from
  the launchers' own records (Steam's and Heroic's play times) and the games
  Zendo started.
- Filters as you type, anywhere in the window, and by store with the chips
  above the grid.
- Pins the games you want first, in every order.
- Hides what is not a game, or what you do not want to see, until you ask
  for it back.
- Gives each game a page: a wide image (Steam's own, the one the store
  publishes, or SteamGridDB's), the developer, play time, install size,
  version, install path and the exact launch command.
- Takes the SteamGridDB key in Settings, checks it with SteamGridDB, and
  keeps it in a settings file only you can read.
- Signs in to Epic, GOG and Amazon on the Accounts page, with accounts of
  its own (not Heroic's): it opens the store's login page in a window of
  its own and, once you have signed in there, takes it from there through
  legendary, gogdl or nile, with nothing to copy. The window keeps no
  cookies. If it cannot open, the page opens in your browser instead and
  you paste back what it ends on. The engines are used from your `PATH`, or from
  Heroic's own copies.
- Shows every game those stores say you own on the Store page, with the
  installed ones marked, a search, store chips and "Not installed only",
  and a page for each.
- Installs, updates, repairs and uninstalls those games on the Downloads
  page, one at a time, with the engine's progress, speed and time left.
  Pause keeps what was downloaded and Resume carries on; Cancel stops and
  deletes a half-done install. Games go into a folder of their own in
  `~/Games` (or `install_dir`), and join the library when done. Closing
  the window while something downloads asks first, then pauses the queue
  for next time. A game's page shows its download size, size on disk and
  latest version, and for the games Zendo installed whether an update is
  waiting.
- Starts the games it installed through their engine, Windows games with
  Proton through umu-launcher, each in a Wine prefix of its own
  (`<install_dir>/Prefixes/<title>`), with each launch's output kept in a
  log. While a game Zendo started runs, Play turns into Running with a
  Stop beside it; pressed again, Stop ends the game at once. Steam and
  Heroic games get the same once their launcher has started them, found
  by their install folder.
- Gives each game it starts itself (the ones it installed, `.desktop`
  ones) launch options on its page: the Proton build, MangoHud, GameMode,
  gamescope with full screen and a size, controllers as Xbox pads for
  games that ignore others, arguments and environment. Steam and Heroic
  games keep their launcher's own settings.
- Downloads GE-Proton from Settings: the latest releases, checked against
  their checksums, into Steam's compatibility tools folder, where Steam
  and Heroic find them too; and removes the ones there.
- Shows a game's [ProtonDB](https://www.protondb.com) rating on its page,
  Platinum to Borked, or Native for a game with a Linux version on Steam,
  and opens its ProtonDB page when clicked. Games
  from other stores are matched to Steam through umu's database or their
  exact title.

## What you need

Nothing beyond Zendo: it reads what is there and leaves out the rest. Each
launcher is needed only for its own games:

| Tool | For |
|------|-----|
| Steam | Steam games; they start through the Steam client |
| Heroic | The Epic, GOG and Amazon games it installed, and games added to it |
| legendary, gogdl, nile | Signing in to Epic, GOG and Amazon, listing your games there and installing them; Heroic's own copies are used when these are not installed alone |
| WebKitGTK 4.1 (`webkit2gtk-4.1`) | The sign-in window; Zendo is built against it, so it is needed to build and run |
| umu-launcher | Starting the Windows games Zendo installed; it uses the newest GE-Proton or other Proton in Steam's compatibility tools folders, or downloads UMU-Proton (another can be chosen per game) |
| MangoHud, GameMode, gamescope | Only for a game whose launch options turn them on |

## Build

Rust 1.88 or newer, and WebKitGTK 4.1 with GTK 3 (on Arch, `webkit2gtk-4.1`;
on Debian and Ubuntu, `libwebkit2gtk-4.1-dev`):

```bash
git clone https://github.com/leandrop-dev/zendo.git
cd zendo
cargo build --release
./target/release/zendo
```

There is no package or installer yet; hyprmonk's installer will set it up.

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
the code.

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
zendo covers                 download the covers and wide images missing, saying what it found
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
# false to never download covers.
download_covers = true
# Where store games are installed, each in a folder of its own.
install_dir = "~/Games"
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
xbox_controllers = true         # PROTON_PREFER_SDL=1
arguments = "-windowed"
environment = "DXVK_HUD=fps"
```

What Zendo keeps itself:

| Path | What |
|------|------|
| `~/.local/state/zendo/state.json` | The games it started and when, the hidden and pinned games, the sort |
| `~/.cache/zendo/covers/` | Downloaded covers (`<id>.jpg`) and wide images (`<id>.hero.jpg`); `misses.json` lists games with none, asked about again after a week |
| `~/.local/share/zendo/stores/` | Zendo's sign-ins, one folder per store engine (`epic`, `gog`, `amazon`), readable by you only, with each engine's record of what Zendo installed (`installed.json`; for GOG, Zendo writes it) |
| `~/.local/state/zendo/downloads.json` | What is left in the download queue, there only while something is |
| `~/.cache/zendo/store/` | Each store's list of your games (`<store>.json`), shown at once on the next start; `gogdb/` keeps GOG's answers about each game |
| `~/.local/state/zendo/logs/` | The output of each game's last launch (`<id>.log`) |
| `~/.cache/zendo/umu.json` | umu-launcher's id for each game, which picks the fixes it applies |
| `~/.local/share/Steam/compatibilitytools.d/` | GE-Proton builds downloaded from Settings (Zendo does not own the folder; Steam reads it) |
| `~/.cache/zendo/protondb.json` | Each game's Steam app id and ProtonDB rating, asked again after a week |

Play time of games Zendo starts itself (the ones it installed, `.desktop`
ones) is counted until the game's last process ends; Steam and Heroic
count their own.

## Layout

| Path | What |
|------|------|
| `crates/zendo-core/` | The library: `steam.rs`, `heroic.rs`, `desktop.rs` find games, `launch.rs` starts them (with `proton.rs` and `umu.rs` for Windows games, `launch_options.rs` for each game's options) and stops them, `protondb.rs` asks ProtonDB, `ge_proton.rs` downloads GE-Proton, `covers.rs` downloads covers, `state.rs` and `config.rs` are Zendo's own files, `stores.rs` signs in through the store engines, `owned.rs` lists the games they own, `info.rs` asks their sizes and updates and `downloads.rs` installs them |
| `crates/zendo-core/tests/fixtures/` | Sample launcher files the tests read |
| `crates/zendo/` | The `zendo` command (`main.rs`), the controller (`gamepad.rs`) and the window: `app.rs` holds its state and keys, `app/` a module per page (`library.rs`, `game.rs`, `store.rs`, `downloads.rs`, `accounts.rs`, `settings.rs`, `about.rs`), `running.rs` for the games running, `launch_options.rs` for a game's launch options sheet, `protondb.rs` for its rating pill, `protons.rs` for Settings' Proton section, the sign-in window (`sign_in_window.rs`), the `sidebar.rs` and the colours (`style.rs`) |
| `PLAN.md` | What Zendo is for, the decisions so far and the phases |

## Roadmap

1. **Library, read-only** (done): Steam, Heroic and `.desktop` games, the
   cover grid and launching.
2. **Covers and order** (done): covers from the stores and SteamGridDB,
   sorting by recent and most played, hiding.
3. **The shell** (done): Hikari's layout in hyprmonk's colours: the
   sidebar, the Library with store and sort chips and pins, a page per
   game, Settings with the SteamGridDB key, About.
4. **Accounts, Store and Downloads** (built, waiting on a try with real
   accounts): sign in to Epic, GOG and Amazon, the games owned but not
   installed, and install, update, repair and uninstall through legendary,
   gogdl and nile, with pause and resume, update status and sizes, and a
   sign-in window of Zendo's own, so there is no code to copy.
5. **Launch options** (built): Proton through umu-launcher with a prefix
   per game, a launch log, Running and Stop, a launch options sheet with
   MangoHud, GameMode and gamescope, the ProtonDB rating on the game's
   page, and GE-Proton downloads.
6. Local games: add a game by hand, with a cover from SteamGridDB.
7. Mods: Nexus links, profiles, back to vanilla.

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

Built on [Iced](https://iced.rs) (MIT), [gilrs](https://gitlab.com/gilrs-project/gilrs)
(MIT or Apache-2.0), [keyvalues-parser](https://codeberg.org/CosmicHarper/vdf-rs)
(MIT or Apache-2.0), [ureq](https://github.com/algesten/ureq) (MIT or
Apache-2.0) and [toml](https://github.com/toml-rs/toml) (MIT or Apache-2.0).
Covers of games no store has art for come from
[SteamGridDB](https://www.steamgriddb.com) when you give Zendo a key.

## License

MIT, see [LICENSE](LICENSE).
