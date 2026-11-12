# Zendo: the redesign

Zendo's new look is modelled on [omakade](https://github.com/btsouth/omakade)
(a Qt/QML and C++ game library for Omarchy, GPL-3.0-or-later). Ideas and look
only, no code and no assets, as with Quiver. Written 2026-11-12 from omakade's
README, `docs/GUIDE.md`, `docs/GAME-MODE.md`, its library and game page
screenshots and both demo videos. Built in Iced (backlog items 31 to 34).

## Look

- A translucent window over the wallpaper with a thin accent border; the
  fallback, if Iced's transparency does not blur on KWin, is an opaque
  themed window (decided by the spike in item 31).
- Monospace type throughout (JetBrains Mono, OFL-1.1, bundled), small
  uppercase letter-spaced labels, flat rectangular chips with 1px borders,
  the selection shown as an accent outline.
- No sidebar. The header holds the name, view chips (All, Favorites,
  Recent, Hidden), search and Settings; a second row holds the source
  chips and the count; a third holds Organize (Status, Collection, Tag).
  Sort sits on the right with key hints.
- Colours: the active KDE scheme (item 28). plasmonk keeps surfaces as
  fixed dark greys (`[Colors:Window]` BackgroundNormal 37,39,47) and tints
  only accent, links and selection, so Zendo is accent-tinted, with the
  translucency supplying the rest.

## Library

Portrait covers in a grid: the title in uppercase over the bottom of the
cover, a thin progress line, and the title plus store and hours below the
card.

## Game page

Back at the top left; a large portrait cover on the left with "Change
cover" below. On the right: a big title, the source, link chips (ProtonDB,
PCGamingWiki), actions (Play, Favorite, Manage in the launcher, Hide),
Organize (status chips, tags, collections), stat tiles (play time,
achievements, completion), an insights block and an achievements list.

## Home

Jump back in (the last game, with Play, Details and + Up Next), Continue
playing, Up next (the user's own queue, up to 100, reorderable), Find your
next game (backlog first, then favourites, then similar, then unplayed;
completed, abandoned and hidden left out) and Quick access (All, Favorites,
Backlog, collections, sources, saved filters).

## Game Mode

Full screen is Game Mode, which opens on Home. A header of chips (Show,
Source, Sort, Consoles, View, Search, Filters; Home, Settings, Leave,
Stats), a hero (meta line, title, description, actions, large cover), a
horizontal cover carousel along the bottom and controller button hints at
the bottom right. Start opens a small modal (Back to library, Leave Game
Mode). omakade also starts it on a chosen display, silences notifications,
switches the sound output and holds launcher windows while a game starts;
none of that is planned for Zendo yet.

## Decided in item 32

Favorites are Zendo's pins, renamed, and still lead every sort. Recent is
played in the last 30 days; Hidden shows only hidden games. The card's line is
the achievements share, known once a game's page was opened. The wide art is a
faint backdrop behind the game page's header. The insights block waits for
item 36, and "Manage in the launcher" is Steam's only until item 33.

## Not taken

Console libraries and native ports (item 37 covers emulation its own way),
Sunshine/Moonlight export, Year in Review imagery (item 36 may do a plain
summary), and omakade's Omarchy-specific parts.
