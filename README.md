<div align="center">

# Zendo

A game launcher for Linux: every game in one library, started with the
right tools.

[![License: MIT](https://img.shields.io/badge/license-MIT-f25623?style=flat-square)](LICENSE)
![Linux](https://img.shields.io/badge/Linux-171717?style=flat-square&logo=linux&logoColor=ccd0cf)
![Rust](https://img.shields.io/badge/Rust-Iced-171717?style=flat-square&logo=rust&logoColor=ccd0cf)

</div>

Zendo gathers the games on this machine into one grid of covers: Steam's,
the ones Heroic installed from Epic, GOG and Amazon, and the ones with a
`.desktop` file. It opens in a normal window and goes full-screen for the
couch, and it is driven as well by a controller as by the keyboard.

It is early: this is phase 1 of the [Roadmap](#roadmap), a read-only
library that starts games through the launcher that owns them. A sister
project of [hyprmonk](https://github.com/leandrop-dev/hyprmonk), which will
install it and give it its colours, but it runs on any Linux desktop.

## Contents

- [What it does](#what-it-does)
- [What you need](#what-you-need)
- [Build](#build)
- [Keys](#keys)
- [The zendo command](#the-zendo-command)
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
- Filters as you type, anywhere in the window.

## What you need

Nothing beyond Zendo: it reads what is there and leaves out the rest. Each
launcher is needed only for its own games:

| Tool | For |
|------|-----|
| Steam | Steam games; they start through the Steam client |
| Heroic | Epic, GOG and Amazon games, and games added to Heroic |

Launch options of Zendo's own (Proton through umu-launcher, MangoHud,
GameMode, gamescope) come in phase 3, and each will be optional too.

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

| Key | Controller | Does |
|-----|------------|------|
| Arrows, Page Up/Down, Home/End | D-pad, left stick | Move through the grid |
| Enter, a click | A | Start the game |
| Typing | | Filter by title |
| Backspace | | Take back a letter |
| Esc | B | Clear the filter |
| F11 | Select | Full-screen or windowed |

The controller is read only while Zendo's window is in front, so it does not
move through the library while you play.

## The zendo command

```
zendo                  open the library
zendo --fullscreen     open it full-screen
zendo list [--json]    print the games found: id, source, title
zendo launch <id>      start a game by the id `list` prints
```

`list` also names any launcher file it found but could not read.

## Layout

| Path | What |
|------|------|
| `crates/zendo-core/` | The library: `steam.rs`, `heroic.rs`, `desktop.rs` find games, `launch.rs` starts them |
| `crates/zendo-core/tests/fixtures/` | Sample launcher files the tests read |
| `crates/zendo/` | The `zendo` command and the window (`app.rs`, `gamepad.rs`) |
| `PLAN.md` | What Zendo is for, the decisions so far and the phases |

## Roadmap

1. **Library, read-only** (now): Steam, Heroic and `.desktop` games, the
   cover grid and launching.
2. Covers from SteamGridDB for the games without one, most played and lately
   played first, a way to hide what is not a game.
3. Launch options per game: Proton through umu-launcher with a prefix per
   game, MangoHud, GameMode, gamescope.
4. Stores: sign in, install, update and uninstall through legendary, gogdl
   and nile.
5. Mods: Nexus links, profiles, back to vanilla.

See [PLAN.md](PLAN.md) for the details.

## Credits

Ideas, no code, from:

- [omarchy-games-menu](https://github.com/akitaonrails/omarchy-games-menu)
  (MIT): the grid of covers, typing anywhere to filter, frecency.
- [Hikari](https://github.com/dodo986/hikari) (GPL-3.0): driving Heroic's
  store engines and reading the launchers' libraries without writing to them.
- [Heroic](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher)
  (GPL-3.0): where it keeps its installed games, and its `heroic://launch`
  links.

Built on [Iced](https://iced.rs) (MIT), [gilrs](https://gitlab.com/gilrs-project/gilrs)
(MIT or Apache-2.0) and [keyvalues-parser](https://codeberg.org/CosmicHarper/vdf-rs)
(MIT or Apache-2.0).

## License

MIT, see [LICENSE](LICENSE).
