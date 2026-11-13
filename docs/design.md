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
item 36, and "Manage in the launcher" is Steam's only: Heroic's protocol has
no route to a game's page (item 33).

## Decided in item 33

- No page titles: the tabs and the chip rows name the page, and a size-28
  title over a dim line is gone from Store, Downloads, Accounts and Settings.
  Counts live on chips and in stat tiles (Downloads: up next, paused,
  finished; a store game: play time).
- Settings shows one group at a time under a chip row (General, Keys, Library,
  Engines, Proton, Diagnostics); the art picker's Settings button opens Keys.
- One card heading everywhere: uppercase, bold, size 14 (`game::card`,
  `settings::section`); sub-headings are `style::subheading`; button text is
  `style::button_text` (uppercase) on `BUTTON_SMALL`, `BUTTON` or
  `BUTTON_LARGE`.
- A store game's page uses the installed game's header (`game::header`); the
  old hero with its gradients is gone.
- Sheets share `launch_options::frame` (title, scrolling body, footer) where
  they have a footer: launch options, Categories and the Nexus chooser. The
  rest (Add game, the installer, the art picker, Nexus search) keep their own
  layout with `style::sheet_title` and tokens.
- Pick lists and switches are flat (`style::pick_list`, `menu`, `toggler`).
- Mods stay a card on the game page.

## Decided in item 34

- Home, as design.md first described it (Jump back in, Continue playing, Up
  next, Find your next game, Quick access), was built and then removed the same
  day at the user's wish: the app has Library, Store and Downloads only. Game
  Mode does not need it. Its pieces may come back with item 35 (play status).
- Game Mode is F11 and Select (`--game-mode`, with `--fullscreen` the same). It
  puts the window full screen and shows its own screen over the Library page: a
  hero (meta line, title, description, PLAY, DETAILS, + UP NEXT, a large cover)
  over a carousel of as many covers as fit, centred on the chosen one. A shelf
  (Continue playing, Up next, All games) is chosen with LB and RB or Tab; empty
  shelves are skipped. Y or Space queues the game; Start or Esc/B opens a menu
  (Back to Game Mode, Library, Leave Game Mode). The menu's Library shows the
  normal grid, and every other page keeps its normal chrome; while Game Mode is
  on the top bar has a GAME MODE chip that returns to the screen.
- The up-next queue is the user's own, in `state.json` as `up_next`, up to 100;
  it is added to from the game page's "+ Up next", the hero, and Y or Space. No
  page lists or reorders it yet.
- The hero's description is a store game's summary, else who made it and for
  what; descriptions for every game are item 38. The header's filter chips
  (Show, Source, Sort, View, Search, Filters, Stats) are not built: Filters and
  Stats depend on items 35 and 36.

## Decided in item 35

- Tags are the categories; the UI keeps the word "Categories". Play status is
  Backlog, Playing, Completed or Abandoned, one per game, set by the user only
  (nothing sets Playing on a launch).
- The Library's Organize row: status chips (a status nobody has is hidden
  until chosen), the categories, the saved filters' chips, then CATEGORIES and
  FILTERS. The game page has a STATUS row under the categories; pressing the
  chosen status clears it. A card's line leads with the status when there is
  one.
- A saved filter holds view, store, category, status, search and sort. Its
  chip is chosen when the window shows exactly that; applying one sets the
  sort for good, as the sort chips do. Renaming or deleting a category updates
  the filters naming it (a delete leaves the filter without the category).
- Bulk edits on the marked games: FAVORITE, UNFAVORITE, HIDE, UNHIDE, each
  shown only when it would change a marked game, setting all alike. Hiding
  clears the marks; favoriting keeps them.
- Left out: the random pick, bulk status, bulk Up next, Ctrl+A, Home (the
  user rejected it) and Game Mode's Filters chip.

## Not taken

Console libraries and native ports (item 37 covers emulation its own way),
Sunshine/Moonlight export, Year in Review imagery (item 36 may do a plain
summary), and omakade's Omarchy-specific parts.
