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

## What you need

Nothing beyond Zendo: it reads what is there and leaves out the rest. Each
launcher is needed only for its own games:

| Tool | For |
|------|-----|
| Steam | Steam games; they start through the Steam client |
| Heroic | Epic, GOG and Amazon games, and games added to Heroic |

Signing in to the stores yourself (phase 4) and launch options of Zendo's
own (Proton through umu-launcher, MangoHud, GameMode, gamescope; phase 5)
come later, and each will be optional too.

## Build

Rust 1.88 or newer:

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

On a game's page:

| Key | Controller | Does |
|-----|------------|------|
| Enter | A | Play |
| Esc | B | Back to the library |
| Ctrl+P | Y | Pin or unpin |
| Delete | X | Hide or bring back |

Anywhere: Ctrl+, opens Settings, F1 About, and F11 or Select switches
between full-screen and windowed. On Settings, typing starts the
SteamGridDB key.

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
```

What Zendo keeps itself:

| Path | What |
|------|------|
| `~/.local/state/zendo/state.json` | The games it started and when, the hidden and pinned games, the sort |
| `~/.cache/zendo/covers/` | Downloaded covers (`<id>.jpg`) and wide images (`<id>.hero.jpg`); `misses.json` lists games with none, asked about again after a week |

Play time of games Zendo runs itself (`.desktop` ones) is counted while its
window stays open; Steam and Heroic count their own.

## Layout

| Path | What |
|------|------|
| `crates/zendo-core/` | The library: `steam.rs`, `heroic.rs`, `desktop.rs` find games, `launch.rs` starts them, `covers.rs` downloads covers, `state.rs` and `config.rs` are Zendo's own files |
| `crates/zendo-core/tests/fixtures/` | Sample launcher files the tests read |
| `crates/zendo/` | The `zendo` command (`main.rs`), the controller (`gamepad.rs`) and the window: `app.rs` holds its state and keys, `app/` a module per page (`library.rs`, `game.rs`, `settings.rs`, `about.rs`), the `sidebar.rs` and the colours (`style.rs`) |
| `PLAN.md` | What Zendo is for, the decisions so far and the phases |

## Roadmap

1. **Library, read-only** (done): Steam, Heroic and `.desktop` games, the
   cover grid and launching.
2. **Covers and order** (done): covers from the stores and SteamGridDB,
   sorting by recent and most played, hiding.
3. **The shell** (done): Hikari's layout in hyprmonk's colours: the
   sidebar, the Library with store and sort chips and pins, a page per
   game, Settings with the SteamGridDB key, About.
4. Accounts, Store and Downloads: sign in to Epic, GOG and Amazon, the games
   owned but not installed, and install, update and uninstall through
   legendary, gogdl and nile.
5. Launch options per game: Proton through umu-launcher with a prefix per
   game, a Wine/Proton manager, MangoHud, GameMode, gamescope, a launch log.
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
