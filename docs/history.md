# Zendo: build history

What was built, when, and what was checked. Read this when you need to
know why something is the way it is, or before changing an area whose
history matters. For the plan itself see ../PLAN.md; for the remaining
work, backlog.md; for verified facts about the stores and tools,
reference.md.

## Journal, 2026-11-04 to 2026-11-10

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

2026-11-10: item 19, the library's categories, planned with the user, built
and tried the same day; a whole-workspace review (Rust and Iced, ten
read-only passes) found 144 faults, 25 of them must fix, 24 of those fixed
the same day.

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
     Proton here. That was also the first real Amazon install. An Epic
     game through legendary was checked later: on 2026-11-10 the user
     installed and played Cat Quest II, and it worked. Its log
     (`epic_6a2cc8b03e684bbd9096a338f96a7ba2.log`) is headed `Started by
     Zendo`, runs `legendary launch … --no-wine --wrapper umu-run` with
     `GAMEID=umu-default` and proton-cachyos, and umu's database has no
     entry for the game. Let's Build a Zoo the same evening had the same
     shape (2026-11-11). Still not checked: how long umu's first run takes
     to fetch the Steam Linux Runtime on a fresh PC, with nothing shown
     meanwhile.
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

## The list after phase 7: items 1 to 19

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
   signal it. Fixed 2026-11-09 (the user asked): the group is known by
   its first process's start time too (`launch::Group`); its processes
   are those started since, the id held by a later process means the
   group ended, a group seen ended stays so, and Stop and Kill signal
   its processes by pid, never the group id. Tested with a group whose
   start does not match (not running, nothing signalled) and the real
   one (running, then ended). The minimum Rust is 1.98 [firm]: no older than the
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
   Zendo lacks; what it finds joins this list. Done 2026-11-07, from
   facts only (both GPL-3.0): Heroic's setting names (the user's
   `~/.config/heroic/config.json`, keys only, and `GameSettings` and
   `AppSettings` in `src/common/types.ts`), its game menu's labels
   (`GameSubMenu`), its backend folders and the files it keeps
   (`known_fixes.json`, `areweanticheatyet.json`, `tools/`); Hikari's
   `internal/config/gamesettings.go` field names, its game settings
   sheet's labels and its `internal/` packages. Hikari's game settings
   are nearly Heroic's, so most findings are both's. Zendo already has:
   install, update (and knows when one is waiting, `info.rs`), repair,
   uninstall, the game's folder, pins, hiding, sorts, played time, Proton
   per game with GE-Proton downloads, MangoHud, GameMode, gamescope,
   arguments and environment, ProtonDB, covers, local games, mods,
   `zendo launch` for scripts (Heroic's `heroic://launch`). What it
   lacks became items 16 to 23 (order [firm], confirmed the same day;
   13 to 20 since 2026-11-09); DLC (9), EAC and BattlEye (10) and
   OptiScaler (15, now 23) were
   already on the list. Left out: plain Wine with DXVK, VKD3D and
   DXVK-NVAPI installed into the prefix (Zendo runs Proton, which brings
   them), CrossOver and macOS settings, Hikari's update check against
   its source branch (the AUR `-git` package updates Zendo), Heroic's
   analytics, discounts and affiliate links.
9. The game page made clearer and friendlier, with DLC on it: which are
   owned and installed, and install or uninstall each. Today Zendo
   installs Epic and GOG games with `--skip-dlcs` (`downloads.rs`), so no
   DLC comes with them, and Heroic's DLC is left out of the library.
   Amazon (nile) is to check.
   Planned 2026-11-07; the user chose the layout and the install
   question the same day [firm]:
   - The page in two columns (one on a narrow window): under the hero,
     one action bar, Play (or Running and Stop), Update when one waits,
     Launch options, Pin and More; More opens a row with Hide, Check
     for updates, Repair, Uninstall, and Edit and Remove for a game
     added by hand. Then About, DLC and Mods on the left; a short
     Details panel on the right (store, platform, version, size, played,
     publisher, released, genres, the install folder) and a Technical
     section folded under it (launch command, Zendo id, Wine prefix,
     last log). The "Installed by Zendo" and launch cards go; what they
     said goes into the bar, the status line and those panels.
   - The Store's install asks "Also install its DLC" (ticked when the
     game has some to install); the game page's DLC card installs or
     uninstalls each, or all.
   - What the engines do, from their sources (facts only; legendary
     and gogdl are GPL-3.0): legendary's `info --json` lists the owned
     DLC under `game.owned_dlc` (`app_name`, `title`, `installable`; one
     that is not installable comes with the game), `install <dlc>`
     installs one into its base game's folder, which must be installed,
     `uninstall <dlc>` removes it, and its `installed.json` keeps DLC
     as entries with `is_dlc` and the base game's `install_path`.
     gogdl's `info` lists the owned DLC (`dlcs`: `title`, `id`, and for
     Windows builds `size` per language as for the game); `download`
     and `update` take `--with-dlcs --dlcs <ids>` or `--skip-dlcs`, and
     `update` deletes what its new file list leaves out, so it adds and
     removes DLC, and every update and repair must name the installed
     DLC or it deletes them (Heroic does the same, its
     `installedDLCs`). nile has no DLC: Amazon's stay out.
   - Zendo's GOG record keeps `installedDLCs`; an Epic update updates
     the installed DLC too (`install <dlc> --update-only`). Heroic's
     games show their DLC (from Heroic's records, and the store's list
     when signed in) with no buttons: Heroic installs those. Not done:
     DLC updates in the update check (it compares the game only), and
     GOG's DLC uninstallers in the prefix (Zendo runs none of GOG's
     installer scripts).
   Built 2026-11-07: `info.rs` reads the owned DLC (`Dlc`), `heroic.rs`
   the installed ones into `Details::dlc` (legendary's by the folder
   they share with their game, GOG's from `installedDLCs`), and
   `downloads.rs` has `Action::AddDlc` and `RemoveDlc` with the DLC in
   `Job::dlc` (also the ones an install takes). The window: `game.rs`
   (the bar, More, Details, Technical), `dlc.rs` (the card), the
   Store's "Also install its DLC" (`store.rs`). Checked on screen in a
   throwaway home with a stand-in gogdl: the two columns, the DLC card
   (one of two installed, sizes from GOG) and the Store's tick box. Not
   checked: the jobs against the real engines (the window's buttons
   could not be pressed from here; their arguments are tested).
10. EAC and BattlEye runtimes for the Windows games Zendo starts (Proton
    reads them from `PROTON_EAC_RUNTIME` and `PROTON_BATTLEYE_RUNTIME`);
    how umu gets them is to check.
    Built 2026-11-07. What was found: umu does not fetch them
    (umu-protonfixes installs them only through Steam); Steam keeps them
    as apps 1826330 (EAC) and 1161040 (BattlEye); Heroic downloads them
    from Lutris's list (<https://lutris.net/api/runtimes>: a URL and a
    build time per runtime, no checksums; each tarball holds one folder
    with `v2` or `v1` inside) into its own tools folder.
    Decisions [tentative]:
    - Steam's copy first (its appmanifest says installed), else Zendo's
      own in `~/.local/share/zendo/runtimes/<key>/` with the build time in
      `current_version`, as Heroic writes it. Heroic's copies are not
      used: its folder is Heroic's to change.
    - Both variables are set for every Windows game whose runtime is on
      the machine; Proton uses one only for a game that ships it.
    - A game whose files hold an `EasyAntiCheat`, `EasyAntiCheat_EOS` or
      `BattlEye` folder (three levels deep) gets the missing runtime
      downloaded before it starts, in the window (the status line says
      so) and from `zendo launch`. A failed download is said and the game
      starts without it.
    - Settings' Anti-cheat section shows which copy each runtime uses,
      and offers Download, Update (when Lutris lists a newer build) and
      Remove for Zendo's own. `zendo report` lists them.
    - Steam games are left to Steam; Heroic's games started by Zendo go
      through Heroic, which sets its own.
    Code: `zendo-core/src/runtimes.rs` (finding, the list, download and
    unpack in place of the older copy), `steam.rs`'s `app_folder`,
    `launch.rs`'s `missing_runtimes` and `fetch_runtimes`, the window's
    `runtimes.rs` (the section) and `running.rs` (the download before a
    start). Checked: both real runtimes downloaded and unpacked from
    Lutris's list (an ignored test, `downloads_the_real_runtimes`). Not
    checked: a game with EAC played online through Zendo (none here);
    to try at home with one.
11. Steam: the games owned but not installed (Steam's Web API needs the
    user's own key and SteamID; what local files hold is to check), and
    installing them through Steam (`steam://install/<app>` opens Steam's
    install dialog).
    Built 2026-11-07. What the local files hold: no list of the games
    owned. `localconfig.vdf` lists 229 apps here, tools, free things and
    games played but not owned among them; `licensecache` is encrypted;
    `config/loginusers.vdf` gives the accounts' SteamIDs and names.
    Decisions:
    - The user's own Steam Web API key, kept in the keyring like the
      others (`Key::Steam`, `steam_key` in `config.toml`), with
      `IPlayerService/GetOwnedGames` for the account Steam signed in last
      (`MostRecent`, else the latest `timestamp`) [firm: the user chose it
      over a Steam sign-in in Zendo's window and over public profiles
      only]. Whether the user's own key reads a private profile is not
      known; when Steam lists nothing, Zendo says to make Game details
      public.
    - The key and the account are on Accounts, as Steam's card, not on
      Settings [firm: the user asked for it, 2026-11-07].
    - Steam joins the Store page as a shop of its own (`owned::Shop`,
      beside the engine stores, so the download queue's `StoreKey` stays
      as it is), its list kept in `~/.cache/zendo/store/steam.json` with
      the minutes Steam counted [tentative].
    - Install opens `steam://install/<app>` (and Enter on the page does
      too); the library shows the game after a refresh [tentative: a
      rescan when Steam finishes is item 20's watch (23 before)].
    - Covers: Steam's own cache first (it keeps art for games owned, not
      only installed), then Steam's CDN
      (`shared.steamstatic.com/store_item_assets/steam/apps/<app>/`), and a
      404 there now falls through to SteamGridDB for every store.
    Code: `owned.rs` (`Shop`, `ask_steam`, `fetch_steam`, `steam_user`),
    `steam.rs` (`user`, `cached_image`), `covers.rs` (the 404), the
    window's `steam.rs` (the Accounts card) and `store.rs`. Checked:
    the parser and the key flow in tests; a bad key gets 401 from Steam;
    the report finds the account here. Not checked: a real key (the user
    has none yet), so the list, a private profile and Install with Steam
    are to try.
    The user tried it with a key the same day; changes after:
    - The sidebar's Accounts count includes Steam once its key is set
      (4 of 4).
    - Install asks Steam as `steam -silent steam://install/<app>`, as
      Playnite starts Steam games with `-silent`, so a Steam that was not
      running stays in the tray behind its dialog; `xdg-open` when there is
      no `steam` command. Whether a running Steam still raises its window
      is to try.
    - Zendo follows Steam's installs (`app/steam_installs.rs`): Steam's
      manifests (`steam::states`: `StateFlags`, `BytesDownloaded` of
      `BytesToDownload`, `BytesStaged` of `BytesToStage`) are read every
      2 seconds while something is under way, else every 15 to notice
      downloads started in Steam; a Steam card on the Downloads page and
      the game's Store page show the progress, and when one finishes the
      library is read again and the status line says so [tentative].
      Pausing and cancelling stay in Steam. Not known: how often Steam
      writes `BytesDownloaded` during a download (the 64 MB game tried
      finished in two seconds); to watch with a big game.
    - The Store's game page is laid out as the library's (the hero, the
      actions, About and the DLC owned beside a Details card and a folded
      Technical card, two columns on a wide window; `app/store_game.rs`),
      and a Steam game's details come from Steam's store
      (`store/api/appdetails`, no key: description, developers,
      publishers, release date, genres, platforms), asked once and kept in
      the Steam list's cache.
    - Steam's Web API is asked again on 429, which Playnite does too.
    The user's second try (2026-11-07); changes 2026-11-08:
    - The progress bar stood still: Steam wrote `BytesDownloaded` only at
      the end of an 889 MB download (`content_log.txt` said "preallocated
      0 files"). The progress is now the size of the files Steam writes
      into `steamapps/downloading/<app>/`, of `BytesToStage`, while it
      downloads or is paused; the manifest's figures otherwise.
    - Steam writes nothing when its install dialog is cancelled, so Zendo
      cannot tell: while it waits, the page's button reads Ask Steam
      again, Stop waiting (on the page and the Downloads card) ends the
      wait, and the wait ends by itself after 3 minutes (15 before).
    - The space a Steam game needs, from its store page's requirements
      ("Storage: 8 GB available space", minimum first, GB counted as
      1024 MB), kept in the list's cache and shown on the Store page as
      "Space needed (store's estimate)". Steam's store gives no download
      size; once Steam starts, the progress line has its totals.
    - Uninstall with Steam and Verify with Steam on a Steam game's page
      (`steam://uninstall/<app>`, `steam://validate/<app>`; Steam asks
      and shows the progress itself). Zendo now remembers what was
      installed at the last look: an app whose manifest is gone was
      uninstalled, one newly installed was installed, whether from Zendo
      or in Steam, and either reads the library again. A game waiting
      for an update with automatic updates off (`StateFlags` 2 alone)
      counts as installed, so it no longer sits on the Downloads card.
    - The Store page opened slowly with Steam's 1,150 games: the cover
      grid (the Store's and the library's) builds only the rows on
      screen and two spare, with spacers for the rest.
    - A Steam already running raises its own window when asked through a
      `steam://` link, `-silent` or not; `-silent` only keeps a Steam
      that was not running in the tray. Zendo cannot change that. The
      user found: uninstall shows only Steam's question; verify opens
      Steam's window.
    - Verify showed nothing in Zendo (the user, 2026-11-08). Steam's
      manifest says only "update running" while it verifies, but its
      `logs/content_log.txt` says "Start validating appID <app>" and then
      "AppID <app> scheduler finished". Zendo notes where the log ends
      when Verify is pressed, reads it from there while following, shows
      "Verifying in Steam" on the game page and the Downloads card, and
      says when Steam is done; given up after 3 minutes if Steam never
      started. One verify of 140 here verified two Steam Linux Runtimes
      too. A game Steam updates now says "is updated", not "is
      installed", and neither a verify nor an update counts as a new
      install.
11b. The Steam sign-in, replacing the Web API key [firm: the user chose
    it 2026-11-08, both ways to sign in, now before 12], for the games
    the family shares (the user's children see the user's games in
    Steam; MECCHA CHAMELEON, app 4704690, was missing from Zendo).
    Checked first, 2026-11-08: the user's key works for `GetOwnedGames`
    (717 games) but both `IFamilyGroupsService` calls answer 401 to a
    key, and `appcache/librarycache` (1,270 apps, 553 not owned) cannot
    tell shared games from tools and runtimes. Steam's
    `IAuthenticationService` takes form fields and answers JSON (tried
    without an account: an RSA key for a made-up name, a QR session
    nobody approved, which polls as waiting); its
    `GenerateAccessTokenForApp` wants a routing parameter a made-up
    token does not give.
    Built 2026-11-08 [tentative until the user tries it], as Playnite
    does it: the user signs in on Steam's own store page in Zendo's
    sign-in window (`zendo sign-in-window steam`), with the password and
    Steam Guard or the Steam app's QR code, so Zendo never handles the
    password and needs no crypto of its own. Signed in, the store page
    carries the account's Web API token in `#application_config`
    (`data-store_user_config.webapi_token`, with `data-userinfo`'s
    `steamid`; checked on the page signed out, where the token is
    empty); a script sends both to the window. Decisions:
    - The session (Steam ID and token, as JSON) is kept in the keyring
      as `steam-session` (`Key::SteamSession`), like the keys.
    - The window keeps Steam's cookies in
      `~/.local/share/zendo/steam-web` (0700), so Steam's page remembers
      the sign-in. When the token runs out (its JWT `exp`, ten minutes
      early) or Steam refuses it, Zendo asks Steam's page again in a
      window drawn off screen (`steam-renew`); a signed-out page (Steam
      sends a browser not signed in to `/login`) means signing in again.
      Tried signed out in a throwaway home: the hidden page loads,
      reports signed out and ends in a second.
    - With the token: `GetOwnedGames` (`access_token`), then
      `GetFamilyGroupForUser` and `GetSharedLibraryApps` (excluded apps
      and those the account owns itself left out); shared games join the
      Steam list marked "Shared by your family" (`Owned::shared`). The
      family failing keeps the owned list and says why on Accounts.
    - The account's name comes from `loginusers.vdf` by Steam ID, else
      the Steam ID is shown.
    - Sign out removes the session and Steam's cookies.
    - The Web API key is gone (the user asked, 2026-11-09): an old
      `steam_key` in `config.toml` is read and dropped, and the keyring's
      `steam` item removed, as Zendo starts (`secrets::RETIRED`).
    Code: core `steam_web.rs`, `secrets.rs`, `config.rs`, `owned.rs`
    (`ask_owned`, `steam_game`, `shared`), `steam.rs` (`user_named`); the
    window's `sign_in_window.rs` (`steam` module), `app/steam.rs` (the
    card, the fetch and renewal), `store.rs`.
    Tried by the user 2026-11-09 [firm]: signed in with the QR code, and
    after Refresh the Store showed the family's games. Not tried yet:
    the password and Steam Guard (the children's way) and a renewal once
    the token runs out (about a day).
    What else the token reads (checked 2026-11-09, field names only), for
    later items; nothing changed for now:
    - `GetOwnedGames` with `include_extended_appinfo`: `rtime_last_played`,
      `playtime_2weeks`, playtime per system (`_linux_`, `_windows_`,
      `_deck_`, `_mac_forever`), `has_dlc`, `has_workshop`,
      `has_community_visible_stats`, `capsule_filename`, `sort_as`.
    - `ClientGetLastPlayedTimes`: first and last played times, per system
      too; `GetRecentlyPlayedGames`: the last two weeks.
    - `GetSharedLibraryApps`: also `rt_last_played`, `rt_time_acquired`,
      `capsule_filename`, `sort_as`; `GetFamilyGroup`: the family's name
      and members (Steam ID, role, joined).
    - `IWishlistService/GetWishlist`: app id, date added, priority.
    - Not with the token: `ISteamUserStats/GetPlayerAchievements`
      answers 400.
    The wishlist as a Store filter: dropped by the user 2026-11-09.
11c. Steam's extra details [firm: the user asked 2026-11-09, all but the
    achievements and the wishlist], built the same day:
    - The Store's Steam games keep `last_played`, `recent_minutes` (two
      weeks) and `played_on` (Linux, Windows, macOS, Steam Deck) from
      `GetOwnedGames`; shared games their `rt_last_played`. Their page
      shows "Last played", "Last two weeks" and "Played on". On the
      user's account the per-system minutes add up to the total for
      about half the games played, less for the rest (time from before
      Steam counted by system), and no Deck time, so whether Steam counts
      Deck time inside Linux is not known: shown as Steam gives it.
    - The Store has an order like the library's: A–Z (the default), Recent
      and Most played; games with nothing to go by come last, by title.
    - Accounts shows the Steam family's name and how many are in it
      (`GetFamilyGroup`); the members' names are not shown, as Steam
      gives only their Steam IDs.
    - Left out: `has_dlc`, `has_workshop` and the like (nothing in Zendo
      uses them yet); first played times (`ClientGetLastPlayedTimes`).
    - Installed Steam games already had their last played time and
      minutes from Steam's own files (`localconfig.vdf`), so the library
      is unchanged. This covers the old item 24's "last played time of games not
      installed".
12. Searching Nexus Mods in Zendo instead of the browser. Nexus's v1 API
    has no search; its GraphQL v2 API is to check. Downloading still
    needs the site's Mod manager download for accounts without Premium:
    the API gives free accounts a link only with the key in an `nxm://`
    link.
    Checked 2026-11-09, no key sent: `POST
    https://api.nexusmods.com/v2/graphql` answers `mods(filter, sort,
    count, offset)` without a key. Filter `gameDomainName` EQUALS the
    game's Nexus name, `name` WILDCARD the words, `adultContent` EQUALS
    false; each `Mod` has `modId`, `name`, `summary`, `author`,
    `version`, `endorsements`, `downloads`, `fileSize` (KB), `updatedAt`,
    `adultContent`, `thumbnailUrl`, `modCategory.name`, and the result
    `totalCount`. Sorts: `relevance`, `endorsements`, `downloads`,
    `updatedAt`, `createdAt`, `name`. Tried on Stardew Valley and Skyrim
    SE: `name` WILDCARD sorted by endorsements puts the mod people mean
    first ("content patcher", "smapi", "skyui", "skse"); `relevance`
    (with `nameStemmed`) does not ("smapi" gave SMAPI Templates, "skse"
    a mod with one endorsement), so endorsements is the default order.
    Proposed [tentative, to confirm with the user]: "Search Nexus" on the
    Mods card (beside Find mods on Nexus) opens a search view on the
    game page: a search box, orders (Most endorsed, Most downloaded,
    Recently updated), results with thumbnail, name, author, summary,
    category, version, endorsements and downloads, 20 at a time with
    More; mods already in the game's list marked. A result opens its
    files (v1 `files`, with the key): Download for Premium, or the file's
    page for the Mod manager download, which comes back through the
    `nxm://` handler as now. Adult mods left out unless a toggle is on.
    Confirmed by the user 2026-11-09 [firm]: on the game page, adult mods
    hidden with a toggle remembered in the settings. Built the same day:
    core `nexus_search.rs` (the GraphQL query, 20 at a time, summaries
    made plain); the window's `nexus_search.rs`, a sheet over the game's
    page from Search Nexus on the Mods card (when the game's Nexus name
    is known): search box (Enter searches, empty lists every mod), the
    three orders, Adult mods (`nexus_adult` in `config.toml`), each mod
    with its thumbnail (fetched as the art picker's are, kept in memory),
    version, "In your mods" when a mod from Nexus in the game's list has
    its id, author, category, updated date, summary, endorsements,
    downloads and size; Files lists the files not old or archived,
    newest first (v1 `files`, with the key), each with Download
    (Premium: the usual Nexus download into the game's mods) or Get on
    Nexus (the file's page); Page on Nexus; More; Escape closes. A late
    answer to an earlier search is dropped. Checked: clippy, tests (the
    request and reply read, files offered, the sheet's searches, orders
    and closing) and an ignored test against the real Nexus ("smapi"
    gives SMAPI first). Not seen in the window yet (no mouse tool here to
    open it): for the user to try.
    The user's first try (2026-11-09), and the changes the same day:
    - The sheet opened half way down; it is now window-tall from the
      start, so it stays put as results come.
    - Get on Nexus and Page on Nexus open in Zendo's own Nexus window
      (`zendo nexus-window <url> <folder>`, as the sign-in window runs),
      not the browser [firm: the user asked to keep it in Zendo]. Its
      cookies are kept in `~/.local/share/zendo/nexus-web` (0700), so the
      site's sign-in and cookie choice stay. Get on Nexus opens the
      file's Mod manager download (`&nmm=1`). The window ends with what
      the site gives: an `nxm://` link (caught as a navigation or a new
      window, never followed), taken as any Mod manager download; or a
      manual download, saved into the game's `downloads/` folder and
      added as that mod's file (found by its name among the mod's files
      with the key) or, without the key, as a mod of its own; the
      archive is removed once unpacked. Checked: the window loads Nexus's
      mod page in a throwaway home (no Cloudflare stop; the cookie
      consent shows once).
    - A mod's files the game already has say "You have this file"
      instead of a button. Mods added by hand from an archive Nexus
      named (`<name>-<mod id>-<version>-<time>`) count as that mod in
      the search, and Add mod now takes such an archive as the mod's
      file when the key finds it (`nexus::identify`, `ids_in_name`),
      so its updates are followed; the archive picked stays where it is.
    Tried by the user 2026-11-09 [firm]: the search, the Nexus window and
    the download taken in work; 11c's Steam details too.

Later:

From the survey (item 8), order confirmed by the user 2026-11-07 [firm];
put first and renumbered by the user 2026-11-09 (the old number in
brackets):

13. (was 16) The launch options sheet, more of what both have: the game's own
    executable chosen instead of the store's (`targetExe`), the extra
    launch options a store lists for a game (legendary and GOG's play
    tasks; Hikari: "This game reports no extra launch options"), a
    script before and after the game, and switches for what is today
    typed into the environment: esync and fsync off, Wine's Wayland
    driver, HDR, FSR with its sharpness, WoW64, NVIDIA offload (PRIME),
    an FPS counter, DXVK's frame cap. The variable each sets is to check
    against the Proton builds Zendo lists. gamescope's other settings
    (Hikari: upscale size and method, window type, FPS limit, more
    options), running Proton without umu, the game's language (legendary
    and gogdl take one), and the Steam Runtime for native games.
    Taken in three parts [tentative]: (a) the switches, (b) the
    executable, the store's launch tasks and the scripts, (c) gamescope's
    other settings, Proton without umu, the language and the Steam
    Runtime.
    13a, the switches, built 2026-11-09. Checked first against the two
    builds here (their `proton` scripts and Wine's libraries, read
    only): GE-Proton11-12 reads `PROTON_NO_ESYNC`, `PROTON_NO_FSYNC`,
    `PROTON_ENABLE_WAYLAND`, `PROTON_ENABLE_HDR` (it then sets
    `DXVK_HDR`), `PROTON_USE_WOW64`; proton-cachyos-slr
    (cachyos-11.0-20261015) the same but `PROTON_NO_ESYNC` (it turns
    esync off itself for a few games) and any HDR name (DXVK reads
    `DXVK_HDR` itself, so Zendo sets both). Both builds' Wine reads
    `WINE_FULLSCREEN_FSR` and `WINE_FULLSCREEN_FSR_STRENGTH` (in
    `win32u.so`; the X11 driver's full-screen hack, which the Wayland
    driver turns off), and both scripts handle `DXVK_FRAME_RATE` and
    `VKD3D_FRAME_RATE`. The switches go before the game's own
    environment, which can change any of them; the Proton ones only for
    a Windows game. The sheet names a switch the chosen build's script
    does not read (`Proton::ignores`, for the `PROTON_*` names). NVIDIA
    offload (`__NV_PRIME_RENDER_OFFLOAD`, `__VK_LAYER_NV_optimus`,
    `__GLX_VENDOR_LIBRARY_NAME`) is for every game Zendo starts, and
    usable only with NVIDIA's driver and a second card (this computer
    has one card: shown greyed). HDR under gamescope adds
    `--hdr-enabled`. Checked: clippy, tests (the environment, its order,
    the check of a build's script, the sheet's switches). Not yet seen in
    the window.
    13b, built 2026-11-09 (`store_tasks.rs`). Checked first: legendary's
    and gogdl's `launch --help` (Heroic's bundled copies) both take
    `--override-exe`; nile has none, so Amazon games get no program
    choice. gogdl (facts read from its source, no code taken):
    `--prefer-task` is a 0-based index into every task of
    `playTasks`; without it the primary task runs, with its arguments
    and working folder; `--override-exe` is used as given (so Zendo
    passes it whole) and its folder becomes the working one; words after
    are passed to the game. legendary takes `--override-exe` relative to
    the game's folder (Zendo strips the folder, and refuses a program
    outside it). The stores' records here, read only: GOG's
    `goggame-<id>.info` (Cyberpunk 2077: REDprelauncher primary, the
    game itself hidden), Epic's metadata `extraLaunchOption_NNN_Name`
    and `_Args` (one of 172 games: Rocket League, "Launch without
    Anti-Cheat", `-noeac`; `AdditionalCommandLine` legendary applies
    itself). The sheet's Start with lists GOG's other file tasks (by
    their file, as names repeat) and Epic's options (by name), says when
    the store lists none, and keeps one no longer listed as such (the
    store's own runs). Program has Browse (the file dialog; kept relative
    to the game's folder when in it). The scripts run through `sh -c`
    from the game's folder with `ZENDO_GAME_ID`, `ZENDO_GAME_TITLE`,
    `ZENDO_GAME_FOLDER` and the game's `WINEPREFIX`: before, for every
    game Zendo starts itself, waited for, a failure stopping the launch
    with its status and last error line; after, once its process group
    is gone, only while the window followed it (not `zendo launch`).
    Checked: clippy, tests (the tasks read from both stores' records,
    gogdl's index, the program as each engine takes it, the scripts'
    environment and failure, the sheet). Not yet seen in the window.
    13c, built 2026-11-09 (`steam_runtime.rs`, the sheet's
    `launch_options/gamescope.rs`). Checked first:
    - gamescope 3.17.0's `--help`: `-b` borderless, `-w`/`-h` the
      game's size beside `-W`/`-H`, `-S` auto, integer, fit, fill,
      stretch, `-F` linear, nearest, fsr, nis, pixel, sgsr,
      `--sharpness` 0 (most) to 20, `-r` the game's refresh. The sheet
      has Shows as (a window, without its frame, full screen), both
      sizes, Fit and Scaling (sharpening for fsr, nis and sgsr), Frames
      a second and More options (any others, split as a shell would).
    - Proton without umu: both builds' `toolmanifest.vdf` ask for tool
      4183110, Steam Linux Runtime 4.0 (`SteamLinuxRuntime_4` here;
      umu keeps its own in `~/.local/share/umu/steamrt4`). What umu
      gives Proton (read from the installed umu, facts only):
      `STEAM_COMPAT_DATA_PATH` the prefix, an empty
      `STEAM_COMPAT_CLIENT_INSTALL_PATH` (Proton reads it unguarded),
      `STEAM_COMPAT_TOOL_PATHS` and `_MOUNTS` Proton's and the
      runtime's folders, `UMU_ID` the game's id (protonfixes reads it),
      the Steam app id from `umu-<number>` or 0. Zendo runs
      `<runtime>/_v2-entry-point --verb=waitforexitandrun --
      <proton>/proton waitforexitandrun` with those, and makes the
      prefix's `pfx` link to itself first, as umu does (the prefixes
      here have it), so Proton keeps using the same prefix.
    - Native games: scout (`scout-on-soldier-entry-point-v2 --`, which
      needs soldier), soldier, sniper and 4.0 (`_v2-entry-point
      --verb=waitforexitandrun --`), after the other wrappers; scout,
      soldier and 4.0 are installed here.
    - The language: legendary's `launch --language`; gogdl's `launch`
      takes none (a GOG game's language is chosen when installed), nile
      neither. Checked as two or three letters, with a region.
    Checked: clippy, tests (gamescope's words, the runtimes' words, the
    environment without umu, the `pfx` link, the language, the sheet).
    Not yet seen in the window, and no game started without umu or in a
    runtime yet: for the user to try.
    Tried by the user 2026-11-09 [firm]: 13a, 13b and 13c work.
14. (was 17) Epic and GOG while playing: offline mode (Epic games that allow it,
    legendary's `--offline`), cloud saves (upload and download by hand,
    and by itself before and after the game: Heroic's `autoSyncSaves`,
    legendary's `sync-saves`, gogdl's save sync, a save path per game),
    Epic's EOS overlay on or off, starting a game while its update waits
    (`ignoreGameUpdates`), and GOG's played time sent back (Heroic's
    playtime sync).
    Taken in three parts [tentative]: (a) Epic's launch switches and its
    overlay, (b) cloud saves, (c) GOG's played time.
    14a, built 2026-11-09 (`eos_overlay.rs`, Settings' `eos_overlay.rs`).
    Checked first (legendary 0.21.1's help, and its source for facts
    only, no code taken):
    - `launch` checks the version unless offline or
      `--skip-version-check`, and refuses an out-of-date game ("Game is
      out of date, please update or launch with update check
      skipping!"): so an Epic game with an update waiting could not start
      from Zendo at all. The sheet's Start while an update waits passes
      the flag.
    - `--offline` skips the sign-in and the version check; for a game
      whose `CanRunOffline` is false legendary only warns. Of the 172
      Epic games' metadata here, 85 say true, 3 false, 84 nothing; the
      sheet's Offline says when Epic has not approved the game.
    - `eos-overlay install --path`, `update`, `remove`; `enable` and
      `disable --prefix` edit the prefix's `user.reg` without Wine
      (`OverlayPath` under `Software\Epic Games\EOS`), so the prefix
      must have been started once. `-y` answers legendary's prompts.
      Settings has Epic's overlay (Install or Update, Remove asked twice)
      into `~/.local/share/zendo/eos-overlay`; not run here, as it
      downloads Epic's overlay. A Windows Epic game's Epic's overlay
      switch turns it on or off in its prefix as it starts, when that
      changes anything (Zendo reads `user.reg` first); a failure is shown
      and does not stop the game.
    Checked: clippy, tests (the flags, reading `user.reg`, Settings'
    Remove). Tried by the user 2026-11-09 [firm].
    14b, built 2026-11-09 (`cloud_saves.rs`). Checked first (legendary's
    and gogdl's help, their source for facts only, no code taken; GOG's
    remote config for Cyberpunk 2077):
    - legendary: `sync-saves <app>` with `--skip-upload`,
      `--skip-download`, `--force-upload`, `--force-download`; it finds
      the save folder from the game's `CloudSaveFolder` and the prefix in
      `WINEPREFIX`, so the prefix must have been started once.
    - gogdl: `save-sync <path> <id> --ts <time> --name <location> --os
      windows` with the same four flags; it prints the time of this sync
      (0 before the first), which Zendo keeps per game and location in
      `~/.local/share/zendo/cloud-saves.json`. On a conflict (both sides
      changed) it logs "Files in conflict", moves nothing and still
      prints a time: Zendo keeps the old time and asks for Upload or
      Download.
    - GOG's save locations: `remote-config.gog.com/components/
      galaxy_client/clients/<clientId>` (the `clientId` in
      `goggame-<id>.info`), `content.Windows.cloudStorage.locations`,
      each a path with placeholders (`<?SAVED_GAMES?>/CD Projekt
      Red/Cyberpunk 2077`). Zendo puts them into the prefix as Proton
      makes it (user `steamuser`: Documents, Saved Games, AppData's
      Local, LocalLow, Roaming; `<?INSTALL?>` the game's folder) and
      refuses a placeholder it does not know rather than guess.
    The sheet's Cloud saves, for Windows games from Epic and GOG: the
    switch fetches the cloud's newer saves before the game (a failure is
    shown and does not stop it) and sends the newer ones after it ends;
    Sync now syncs each way; Upload and Download replace one side, each
    asked twice. Not run against a real game here, as it would move the
    user's saves. Checked: clippy, tests (the locations, the
    placeholders, the flags, the sheet's buttons).
    Tried by the user 2026-11-09: the syncs work; asked for them on by
    default, as Steam has them, and their state on the game's page.
    Then [firm]:
    - On unless the game's launch options say `cloud_saves = false`
      (unset is on, so the files written before still read).
    - Before a prefix's first start nothing syncs (nothing to sync
      into); after it the game's end syncs.
    - legendary 0.21.1 (Heroic's fork, the installed `cli.py` read):
      `-y` skips a game whose save folder it has not kept ("Save path
      for this title has not been set, skipping due to --yes"), and exits
      well; the hidden `--accept-path` has it work the folder out from
      `CloudSaveFolder` and the prefix's `user.reg` and keep it. Cat
      Quest's `save_path` was still null, so Epic's syncs had done
      nothing before this. legendary also exits well for a game without
      cloud saves (not named in its log: no `Checking "…"` line) and
      skips a folder it cannot work out ("unprocessed variables":
      `{UserProfile}`, 1 of the 64 Epic games here with a save folder),
      so Zendo reads its log. It takes unknown flags without a word
      (`parse_known_args`).
    - GOG: no client id or no Windows locations is "no cloud saves", not
      an error.
    - Each sync's outcome (in sync, no cloud saves, failed and why) and
      time in `~/.local/share/zendo/cloud-saves-status.json`; the game
      page's Details says it ("In sync, 5 min ago", "Failed 2 h ago:
      …", "Syncing the saves…" as the game ends, "Off" when turned off).
    Tried by the user 2026-11-09 [firm]: the syncs and the page's line work.
    14c, built 2026-11-09 (`gog_playtime.rs`). Checked first (Heroic
    2.22.4's bundled `app.asar`, read for facts only, no code taken):
    - As a GOG game ends Heroic posts `{"session_date": <start, Unix
      seconds>, "time": <whole minutes>}` to
      `https://gameplay.gog.com/games/<id>/users/<user_id>/sessions`
      with the account's Bearer token; 201 is taken. A session under a
      minute is not sent. One that fails waits in a queue per user and
      goes when back online. A `GET` on the same address gives
      `time_sum`, GOG's total (Heroic shows it; Zendo does not yet).
      Heroic's setting is `disablePlaytimeSync`, with no default in the
      bundle: unset (as in this computer's `config.json`), it sends.
    Zendo [tentative]: after a GOG game it started through gogdl ends
    (Heroic sends its own), after the saves and the script, with
    gogdl's refreshed token (`gogdl auth`). One not sent waits in
    `~/.local/share/zendo/gog-playtime.json` (under a lock) and goes
    with the next GOG game's; the status line says it waits. Signing
    out of GOG drops them. On unless Accounts' GOG card turns it off
    (`gog_playtime = false` in `config.toml`). Not sent against GOG
    here, as it writes to the user's account. Checked: clippy, tests
    (whole minutes, under one skipped, only gogdl's games, the failed
    ones kept, the body, the switch). Tried by the user 2026-11-09: it
    works [firm]. The user then asked for GOG's total on the game's page,
    as Steam's games show Steam's; built the same day. Heroic's game page
    (its renderer in `app.asar`) asks `time_sum` as the page opens and
    as the game's status changes, takes it as minutes, and keeps the
    larger of it and its own count. Zendo [tentative]: asked once a
    session as a `gog:` game's page opens (installed or only owned),
    and again after a session is sent; a failure is forgotten, so the
    page asks again. Kept in `~/.local/share/zendo/gog-playtime-totals.json`
    and merged into the library and the Store's list as they load (the
    larger counts), so the cards and Most played have it from the start;
    signing out of GOG drops it. Checked: clippy, tests (`time_sum` read,
    none yet as 0, the library and the page taking the larger). Tried by
    the user 2026-11-09: it works [firm]. Item 14 is done.
15. (was 18) The prefix's tools: winecfg, winetricks, running an `.exe` in the
    game's prefix (installers, patches), and opening the prefix's
    folder.
    Built 2026-11-09 (`prefix_tools.rs`, the window's `prefix.rs`).
    Checked first (umu-launcher 1.4.3's `umu_run.py` and `umu_util.py`,
    GE-Proton 11-12's protonfixes, Heroic 2.22.4's bundle; facts only, no
    code taken):
    - umu: a program name that is no file (`winecfg`, `msiexec`) goes to
      Wine as it is ("Assume that the executable will be inside prefix");
      `umu-run winetricks <verbs>` runs Proton's own
      `protonfixes/winetricks` with `-q`, and exits 1 running none when
      one verb is in the prefix's `winetricks.log` already ("ERROR:
      winetricks verb '…' is already installed in '…'"; settings verbs
      such as `win10` excepted); a verb is
      `^[a-zA-Z_0-9-]+(=[a-zA-Z0-9]*)?$`.
    - winetricks' own window: protonfixes' fix for the id
      `winetricks-gui` runs `winetricks --unattended` with no verb
      (`gamefixes-umu/winetricks-gui.py`); Heroic starts it as `umu-run
      ""` with `GAMEID=winetricks-gui`. GE-Proton and UMU-Proton carry
      protonfixes; Valve's Proton does not.
    - Heroic runs winecfg and a program with the game's own umu id, and
      winetricks verbs with `umu-0`.
    Zendo [tentative]: a Wine prefix card on the page of a Windows game
    Zendo starts (installed by it or added by hand): Wine settings,
    winetricks (refused, saying why, when the game's Proton has no
    `winetricks-gui` fix), Run a program… (the file dialog's new
    `installer` kind, `.exe` and `.msi` from `~/Downloads`; an `.msi`
    through `msiexec /i`, run from its own folder), components typed and
    installed one verb at a time (so one already there is said and
    skipped, from umu's log, without copying umu's list of settings
    verbs; a leading dash refused as an option), and Open folder once the
    prefix exists (the Technical card keeps its own). Always through
    umu, even when the launch options leave umu out (the prefix is the
    same); the game's Proton, prefix and environment, not its wrappers.
    One tool at a time per game and none while it runs; no Stop (the
    tool's window is closed). Each logs to
    `~/.local/state/zendo/logs/<id>.prefix.log`. Checked: clippy, tests
    (the verbs, `.msi`, umu's refusal, which games, the card's states),
    and the real umu and GE-Proton 11-12 in a throwaway prefix (`win10`
    installed, `vcrun2022` written into `winetricks.log` heard as
    already there); winecfg, winetricks' window and a program not run
    here, as they open windows. Tried by the user 2026-11-09: it all
    works [firm].
16. (was 19) What happens by itself on install: GOG's redistributables (Visual
    C++, DirectX) into the prefix (gogdl's `redist`, Heroic's
    `tools/redist/gog`); known fixes per game (Heroic's
    `known_fixes.json`: winetricks verbs, environment, a program run in
    the prefix and a note, for example `vcrun2022` for 29 games; where
    Heroic gets the file and its licence are to check, and how much
    umu's own fixes already cover); a desktop entry and a Steam
    shortcut for the game (Heroic adds both, each a setting).
    Decided by the user 2026-11-09 [firm]: (a) GOG's setup first, then
    (b) the known fixes, fetched at run time from Heroic's repository
    as Heroic does, never shipped (the `Heroic-Games-Launcher/known-fixes`
    repository states no licence; 104 games: 74 Epic, 29 GOG, 1 Amazon;
    66 with winetricks verbs, 8 with environment, 1 with a program run in
    the prefix; its README says some are redundant with umu's
    protonfixes, which key their fixes by umu id, `umu-<Steam id>`, so
    the overlap needs the umu database to count). (c), the desktop entry
    and Steam shortcut, dropped: games are started from Zendo, and a
    desktop entry would come back into the library through the
    `.desktop` scan.
    16a, built 2026-11-09 (`gog_setup.rs`). Checked first (gogdl in
    Heroic 2.22.4's bundle and Heroic's GOG setup, facts only, no code
    taken):
    - Heroic on Linux runs the setup when a game is first added to its
      prefix (as it starts), and again after an update or a move without
      the redistributables. From the game's manifest
      (`heroic_gogdl/manifests/<id>`): `dependencies` (version 2) or each
      depot's `redist` (version 1); with `scriptInterpreter`, GOG's
      `__redist/ISI/scriptinterpreter.exe` for the game and each DLC
      installed (`/VERYSILENT /DIR= /Language= /LANG= /ProductId=
      /galaxyclient /buildId= /versionName= /lang-code= /supportDir=` and
      both spellings of `/nodesktopshortcut`, GOG's typo); otherwise each
      product's `temp_executable` from the support files.
    - `gogdl redist --ids <ids> --path <folder>` downloads the shared
      ones (`__redist…`) and keeps GOG's list in
      `.gogdl-redist-manifest` (each one's `executable` `path` and
      `arguments`, `HGLInstalled` the ids asked). It removes what it was
      not asked for this time, so Heroic asks for every installed game's
      and `ISI`. Arguments are split as a shell would (`shlex`); Visual
      C++ 2005's carry quotes. PhysX Legacy, an `.msi`, through `msiexec
      /i … /qb`.
    Zendo [tentative]: before the first start of a Windows game it
    installed from GOG in its prefix (kept in the prefix's
    `.zendo-gog-setup.json`), on the game's thread: gogdl downloads every
    GOG game's redistributables into `~/.local/share/zendo/gog-redist`,
    each of this game's runs in its prefix through umu (the game's
    Proton, prefix and environment; from that folder), then the game's
    own steps. The status line says each step; a failed step is said once
    the game runs, does not stop it, and the prefix counts as set up all
    the same (so no game is held at every start); the Wine prefix card's
    GOG's setup runs it again. Language: English, as Zendo installs. Not
    yet: after an update or a move, and version 1's `support_commands`.
    Logged to `~/.local/state/zendo/logs/<id>.gog-setup.log`. Checked:
    clippy, tests (both manifest versions, the shell split with Visual
    C++ 2005's, `.msi`, Wine's `Z:` paths, the prefix's record, the
    script interpreter's arguments for the game and its DLC); for real,
    gogdl downloaded `ISI` and `MSVC2015` (14 MiB) into a throwaway
    folder with Zendo's sign-in, and GOG's Visual C++ 2015 installer ran
    with GOG's arguments through umu and GE-Proton 11-12 in a throwaway
    prefix (exit 0; its registry entries and DLLs there). Tried by the
    user 2026-11-09: it works [firm].
    16b, built 2026-11-09 (`known_fixes.rs`). Checked first (Heroic
    2.22.4's bundle, facts only): Heroic downloads
    `raw.githubusercontent.com/Heroic-Games-Launcher/known-fixes/main/known_fixes.json`
    and applies a game's fixes when the game is first set up in its
    prefix, after the store's own setup: winetricks verbs, then each
    `runInPrefix` path (relative to the game's folder). All 8
    `envVariables` are `USE_FAKE_EPIC_EXE`, Heroic's own switch for a
    stand-in Epic launcher, so Zendo leaves them out.
    Zendo [tentative]: for Windows games it installed from Epic, GOG or
    Amazon, before the start and after GOG's setup, on the game's
    thread: the list is downloaded at most once a day into
    `$XDG_CACHE_HOME/zendo/known_fixes.json` (a failed download keeps the
    old one); the fixes not yet tried in the prefix run through the
    prefix tools (verbs checked as typed in the card; a program only
    inside the game's folder), the status line says each, and each is
    kept tried in the prefix's `.zendo-known-fixes.json`, failed or not,
    so a fix added to the list later is applied at the next start and a
    game is not held at every start. A failure is said once the game
    runs. The Wine prefix card lists the game's fixes with Heroic's notes
    and "Apply known fixes" runs them again. Checked: clippy, tests (the
    list's shape, the store keys, which games, paths kept in the game's
    folder, the prefix's record, the card), and Heroic's real list read
    whole (104 games, 66 with fixes Zendo applies). Taken as working by
    the user 2026-11-09 [firm]: none of the games installed is on the
    list (Cyberpunk 2077 is not), so not yet seen applied on a real game.
    16c, GOG Galaxy for GOG games, added 2026-11-09 after the user's
    try of Cyberpunk 2077 (GOG 1423049311): from Zendo, its in-game CD
    Projekt RED sign-in was asked at every start. REDlauncher's log in
    the prefix (`AppData/Local/Programs/CD Projekt Red/REDlauncher/logs`)
    showed "Signing in to GalaxySdk failed", "Failed to get GOG Galaxy
    executable name", and the DLC checks "assuming as not owned". Checked
    first (Heroic 2.22.4's bundle; comet's source at its tag v0.2.0, the
    version Heroic bundles; facts only):
    - Heroic, for GOG games, unless its `cometSupport` experiment is
      turned off (on by default): before the start, copies comet's
      stand-in `GalaxyCommunication.exe` to
      `C:\ProgramData\GOG.com\Galaxy\redists` when it is not there and
      runs `sc create GalaxyCommunication binpath=…`, and at every start
      `reg add HKLM\SOFTWARE\WOW6432Node\GOG.com\GalaxyClient\paths /v
      client /d "C:\Program Files\GOG Galaxy"`; it spawns `comet
      --from-heroic --username <name> --quit` beside the game and kills
      it when gogdl returns.
    - comet (Apache-2.0) listens on 127.0.0.1:9977 as Galaxy's service
      does, reads the tokens once (from Heroic's
      `$XDG_CONFIG_PATH/heroic/gog_store/auth.json`, which has gogdl's
      layout, client `46899977096215655`, or from `--access-token`,
      `--refresh-token` and `--user-id`) and writes nothing back; keeps
      its files in `$XDG_DATA_HOME/comet`; with `--quit` it ends 15 s
      after the last game disconnects, and on Ctrl+C. Its compatibility
      list has Cyberpunk 2077's achievements working with v0.2.0.
    Zendo [tentative]: for GOG games it installed, unless Accounts' GOG
    card turns it off (`gog_galaxy`, on by default): before the start,
    after the known fixes, a Windows game's prefix gets the service and
    both registry entries once (the service's file there marks it done);
    on the game's thread, gogdl refreshes the tokens and comet starts
    with `XDG_CONFIG_PATH` at `stores/gog/comet`, whose
    `heroic/gog_store/auth.json` is a link to Zendo's tokens (never the
    command line, where other programs could read them), and
    `XDG_DATA_HOME` at Zendo's data folder (so `zendo/comet`), in a
    process group of its own; it is asked to stop (SIGINT, then killed
    after 5 s) as soon as the game ends. A failure is said once the game
    runs, which starts without Galaxy. Signing out of GOG removes comet's
    link and files. Logged to `<id>.comet.log` and `<id>.galaxy.log`.
    Checked: clippy, tests (which games, the service's place, the logs,
    the switch); for real, comet read Zendo's tokens through the link,
    signed in to GOG's notifications, fetched Galaxy's peer library into
    a throwaway data folder and stopped on SIGINT; the service and both
    registry entries were put into a throwaway prefix through umu and
    GE-Proton (`ControlSet001\Services\GalaxyCommunication`,
    `Wow6432Node\GOG.com\GalaxyClient\paths`). Also found in that try,
    not Zendo's: REDlauncher's window takes no input and the game starts
    by itself 30 s later, because a lock left at 14:51
    (`REDlauncher/locks/REDlauncher.exe.lock`, holding 356) names the PID
    Wine gives every REDlauncher in that prefix, so each one waits for
    "another instance", which is itself.
    Tried by the user 2026-11-09 on Cyberpunk 2077, with the lock removed
    and the game's own launcher chosen again: the launcher takes the
    mouse, and the CD Projekt RED sign-in stays [firm].
17. (was 20) Managing installs: moving a game to another folder (legendary's
    `move`), importing a game already on disk (legendary's `import`),
    updating games by themselves (Heroic's `autoUpdateGames`), the
    download workers (`maxWorkers`), GOG's changelog.
    Checked first (2026-11-09; legendary and gogdl in Heroic 2.22.4's
    bundle, nile's source on GitHub, Heroic's bundle; facts only):
    - legendary `move <app> <new base> [--skip-move]` only renames: across
      drives it logs an error and returns, exit 0 (as on every refusal).
      gogdl and nile have no move. Heroic moves the files itself (rsync
      `--archive --remove-source-files`, then `rm -rf` the old folder, or
      `mv`), then tells the engine: legendary `move --skip-move`, its own
      GOG record, nile's `installed.json` `path`. After a GOG Windows
      game's move it runs GOG's setup again.
    - Import: legendary `import <app> <path>` checks the files against the
      manifest (no files found, or no executable, refuses; under 95 %
      warns) and may need a repair before updating; nile `import --path
      <path> <id>`; gogdl `import <path>` only prints what the folder's
      `goggame-*.info` says (id, build, language, DLC), the launcher
      keeps the record, and Heroic remakes gogdl's manifest from it.
    - Workers: legendary `install --max-workers` (default min(2 × CPUs,
      16)), gogdl `--max-workers` on download, update, repair and redist
      (default the CPUs), nile `install`/`update`/`verify --max-workers`.
      Heroic's `maxWorkers`, 0 meaning the engine's own.
    - Updates: Heroic's `autoUpdateGames` (off by default) queues each
      update its check finds, unless the game's `ignoreGameUpdates` or
      the game is not on disk.
    - GOG's changelog: `api.gog.com/products/<id>?expand=changelog`, HTML.
    Decided by the user 2026-11-09 [firm]: the order 17a moving, 17b
    importing, 17c the workers and automatic updates (two settings), 17d
    GOG's changelog; moves to other drives too (copied, then the old
    folder deleted); import from a folder the user picks, not Heroic's
    installs (both would keep records of one folder); automatic updates
    on by default, a setting turns them off.
    17a, built 2026-11-09 (`moving.rs`, a Move job in `downloads.rs`).
    Zendo [tentative]: the game page's More row has Move…, which opens the
    desktop's folder dialog (`zendo pick-file folder`, through the portal)
    and queues a job on the Downloads page; the game's folder keeps its
    name in the folder chosen. Not while the game runs: the button is off,
    a move queued for a game started since waits paused with why, and Play
    waits while the files move. On the same drive the folder is renamed;
    across drives it is copied into a hidden `.<name>.zendo-move` beside
    the new place (permissions, modified times and links kept, Stop
    checked each MiB, progress, speed and time left as a download's),
    checked to hold as many files and bytes, then named. Then the record:
    legendary `move --skip-move` for the game and each DLC in its folder
    (read back, as legendary ends well when it refuses), Zendo's GOG
    record, nile's `installed.json`; if that fails, the files go back.
    Then the old folder is deleted (only when it holds no other game and
    passes the uninstall's checks); applied mods' record (`applied.json`,
    the game's folder and a plugin list kept in it) names the new folder;
    a GOG Windows game's prefix forgets GOG's setup, so it runs again at
    the next start, as Heroic does (16a's "after a move"). Stopping or a
    failure deletes the hidden copy and leaves the game where it was; a
    move resumed after a crash carries on from the step it reached. A
    full drive says so; there is no free-space check first (std has no
    `statvfs`). Checked: clippy, tests (rename, a copy keeping files,
    links, modes and times, stopping, a place taken or inside the game,
    the job in the queue, the progress, Back to vanilla after a move);
    for real in a throwaway home, from btrfs to tmpfs so copies crossed
    drives: Epic's by rename with the real legendary rewriting its
    record, GOG's and Amazon's copied (40 MiB, checksums equal), records,
    prefix and mods record changed, old folders gone; a stopped copy left
    the old folder and record as they were and no hidden copy. Tried by
    the user 2026-11-09: it works [firm].
    17b, built 2026-11-09 (`gog_import.rs`, an Import job in
    `downloads.rs`). Checked first (gogdl in Heroic 2.22.4's bundle, nile's
    source, Heroic's bundle; facts only): without its manifest in
    `heroic_gogdl/manifests/<id>`, gogdl's update downloads every file
    again (its diff counts all as new) and its repair refuses ("No manifest
    stored locally"), so Heroic remakes it: GOG's builds for the system
    (`content-system.gog.com/products/<id>/os/<os>/builds?generation=2`),
    the folder's build (or the latest outside a branch), its manifest
    (zlib for generation 2), with `HGLPlatform`, `HGLInstallLanguage` and
    `HGLdlcs` added. GOG lists public builds without a token and gives a
    `link` instead of `urls` then. nile's import verifies every file
    against Amazon's list and refuses otherwise; legendary's refuses with
    no files found or no executable, and switches a first update to a
    repair when it must.
    Zendo [tentative]: an owned Epic, GOG or Amazon game's Store page has
    Import from a folder… beside Install; the folder dialog asks where the
    game is and an Import job is queued. Refused: a game Zendo has
    installed, a folder inside or around one of Zendo's games. Epic:
    `legendary import <app> <folder> --with-dlcs --platform Windows`;
    Amazon: `nile import --path <folder> <id>`; both end well when they
    refuse, so the record is read back and the engine's last error said
    (nile's `ERROR [IMPORT]:` lines read too). GOG: `gogdl import` must name
    this game; the manifest is remade as Heroic does (with the token when
    signed in), then Zendo's record written with the folder's build,
    version, language, DLC and size. gogdl's updates and repairs now use
    the record's language. Nothing in the folder is changed. Checked:
    clippy, tests (what gogdl reads, the build picked, a packed manifest
    and the fields added, the job, the engines' error lines); for real,
    GOG's manifest for Cyberpunk 2077's build fetched and unpacked is the
    one gogdl stored when Zendo installed it, bar its own fields; in a
    throwaway home, importing the real Cyberpunk 2077 folder wrote the
    same record Zendo's install had (build, version, language, Phantom
    Liberty) and a manifest equal to gogdl's, the folder untouched; a
    folder of another game, one without GOG's files, and Epic's and
    Amazon's imports without a sign-in refused with why. Not tried yet
    in the window: no store game is on disk outside Zendo (the two
    folders in `~/Games/Heroic` are Wine prefixes).
    17c, built 2026-11-09 (`config.rs`, `downloads.rs`, Settings'
    Downloads section). Checked first (nile's source, facts only): nile's
    `install`, `update` and `verify` take `--max-workers` but ignore it,
    as its downloader always runs six (`ThreadPoolExecutor(max_workers=6)`
    in `manager.py`). Zendo [tentative]: `auto_update` in `config.toml`,
    on unless set false (the user's choice), and `download_workers`, left
    out for the engine's choice. Settings has a switch for the first and
    chips for the second (the engine's choice, 2, 4, 8, 16). When the
    update check (once a session and after each download) finds updates
    and the switch is on, each one not queued already is queued as an
    Update job, by title; turning the switch on queues the ones found.
    The workers go to legendary's install, update, repair and DLC jobs and
    to gogdl's download, update, repair and redist; not to nile, and
    Settings says so. Jobs that change a running game's files (updates,
    repairs, moves, uninstalls, DLC) now wait in the queue, saying why,
    until the game is closed, and the queue goes on with the next job
    meanwhile; closing the game starts them. This replaces 17a's move
    paused with why. The diagnostics report lists both settings.
    Checked: clippy, tests (the settings' defaults and what they write,
    the workers in each engine's arguments and not in an uninstall's,
    updates found queued by themselves or not when off, a job for a
    running game waiting and starting once it is closed). Not tried yet
    in the window; no game has an update now (Cyberpunk 2077's newest GOG
    build is the one installed).
    17d, built 2026-11-09 (`gog_changelog.rs` in both crates). Checked
    first (Heroic 2.22.4's bundle, GOG's answers; facts only): Heroic asks
    `api.gog.com/products/<id>?expand=changelog&locale=…` with the game's
    extra info and shows the HTML as it is in a dialog (Show Changelog).
    GOG answers without a token; `changelog` is `null` for 54 of the
    user's 113 GOG games and 68 bytes to 250 KB of HTML for the rest
    (Cyberpunk 2077's). In all 59, entries are split by `<hr>` and open
    with an `<h4>` naming the update; later `<h4>`/`<h5>` and paragraphs
    all in bold name parts of an entry; the rest is `<p>`, `<ul>`/`<li>`,
    `<br>`, inline tags, and some developers write one change a line in a
    paragraph (Jupiter Hell). An unknown product answers 404.
    Zendo [tentative]: a Changelog card on a GOG game's page, after the
    other cards of the main column, asked once a session as the page
    opens and not shown when GOG has none (a 404 counts as none; a
    failure is forgotten, so the page asks again). The HTML is read into
    entries (title, part headings, paragraphs, list items; line breaks in
    a paragraph kept, character references written out, other tags
    dropped with their text kept), not shown as HTML. The newest entry
    shows; Show earlier updates adds three each time, back to the newest
    one when the page opens again. No locale is sent: the changelogs are
    the developers' own text. Checked: clippy, tests (Cyberpunk's,
    Frostpunk's, Bō's and Jupiter Hell's shapes, references, stray `<key>`
    text, the card's states); all 59 real changelogs read with a title
    for every entry and no tags left; fetched for real (Cyberpunk 2077:
    28 entries, Fallout: none, an unknown id: none); in a throwaway home
    with Cyberpunk 2077's record, a build with the card moved up for the
    screenshot showed Patch 2.5 with its parts and lists. Tried by the
    user 2026-11-09: it works [firm].
18. (was 21) What the game page tells: anti-cheat status before installing
    (Heroic keeps areweanticheatyet.com's list, 1167 games, and warns;
    pairs with 10), PCGamingWiki, HowLongToBeat, the Steam Deck rating,
    the store's requirements, achievements. Pairs with 9.
    Checked first (2026-11-09; Heroic 2.22.4's bundle, the endpoints
    asked once each without a sign-in; facts only):
    - Anti-cheat: Heroic downloads `games.json` from AreWeAntiCheatYet
      (the repo moved to `AreWeAntiCheatYet/AreWeAntiCheatYet`, MIT,
      last pushed 2026-10-24) and matches only `storeIds.epic.namespace`,
      so Steam and GOG games never get its warning. The list: 1167
      games, `status` Supported 196, Running 280, Planned 2, Broken 636,
      Denied 53, `anticheats` names, `storeIds` with `steam` (682 games)
      or `epic` (73), `reference` and `updates`. Heroic warns on Denied,
      Broken and Unknown. Zendo already gives every game a Steam app id
      (ProtonDB's matching), so the list can match by it as well.
    - Steam Deck: `store.steampowered.com/saleaction/
      ajaxgetdeckappcompatibilityreport?nAppID=<id>`,
      `results.resolved_category` (Cyberpunk 2077: 3) and
      `resolved_items` (`display_type`, `loc_token` such as
      `#SteamDeckVerified_TestResult_InterfaceTextIsLegible`). Which
      number means Verified, Playable or Unsupported is not checked.
    - PCGamingWiki: Heroic finds the page by GOG id with a Cargo query,
      but the wiki now refuses those ("You don't have permission to run
      arbitrary Cargo queries"), so only its fallback works: a title
      search, then the page's wikitext. The wikitext holds `steam appid`,
      `gogcom id` (and `… side`), `hltb`, Metacritic and OpenCritic
      scores, `direct3d versions`, `controller support`,
      `ultrawidescreen`, `anticheat` and release dates. The wiki's text
      is CC BY-NC-SA 3.0 (its `rightsinfo`).
    - HowLongToBeat: Heroic takes the id from PCGamingWiki's `hltb`
      (Cyberpunk 2077: 2127) and reads the game's page on
      howlongtobeat.com, the JSON in `<script id="__NEXT_DATA__">`:
      `comp_main`, `comp_plus`, `comp_100` in seconds. The page answers
      a browser User-Agent; `robots.txt` answered curl's own with a 403.
      HowLongToBeat has no public API; its terms are not checked. For
      GOG games Heroic reads the HowLongToBeat box on GOG's store page.
    - Requirements: Steam's `appdetails` `pc_requirements` (HTML; Zendo
      already reads it for the space a game needs); GOG's
      `api.gog.com/v2/games/<id>` `supportedOperatingSystems[].
      systemRequirements` (often empty: Cyberpunk 2077 and The Witcher 3
      had none, The Witcher had seven minimum lines); Epic's
      `store-content.ak.epicgames.com/api/<lang>/content/products/<slug>`
      `productHome` page, `requirements.systems[].details` with title,
      minimum and recommended (Cyberpunk 2077 answered). Amazon's not
      found in Heroic.
    - Achievements: Heroic has GOG's only, signed in:
      `gameplay.gog.com/clients/<id>/users/<user>/achievements` with the
      bearer token and `X-Gog-Lc`. Heroic passes the product id where
      the path says client id; whether that answers is not checked.
      Steam's global percentages need no key
      (`ISteamUserStats/GetGlobalAchievementPercentagesForApp`, 57 for
      Cyberpunk 2077); a player's own need a key or the session, not
      checked. Epic's not checked.
    Decided by the user 2026-11-09 [firm]: five parts in this order, 18a
    the anti-cheat status, 18b the Steam Deck rating, 18c the
    requirements, 18d PCGamingWiki's facts, 18e achievements;
    HowLongToBeat left out; a game Denied or Broken is warned about on
    its page only, with Install and Play left as they are.
    18a, built 2026-11-09 (`areweanticheatyet.rs`, the window's
    `anti_cheat.rs`). Zendo [tentative]: the list is downloaded at most
    once a day into `$XDG_CACHE_HOME/zendo/areweanticheatyet.json`
    (offline, the one kept is used); a game is matched by the Epic
    namespace in legendary's record, else by the Steam app id ProtonDB's
    lookup found, so the lookup runs once ProtonDB has answered. Both
    game pages, installed and store, get an Anti-cheat card: the status
    as a badge (Supported and Running good, Planned dim, Broken, Denied
    and a status Zendo does not know bad), the anti-cheats, the site's
    meaning of the status, the newest update with its date, the notes
    (each opening its source) and Open AreWeAntiCheatYet. A game that
    will not play online has the card first, else it comes after About;
    a game not on the list has none. The list's notes are `[text,
    null]` when they have no source. Checked: clippy, tests (Steam id and
    namespace, the namespace winning, notes with and without links, a
    status Zendo does not know, the card and its lookup only after
    ProtonDB); an ignored test against the real list (Rust Denied by
    Steam id, Fortnite Denied by namespace, an unknown GOG game none); in
    a throwaway home with only Fortnite's Epic record, its page showed
    DENIED, Easy Anti-Cheat, "Removed BattlEye · Jun 11, 2024" and the
    Xbox Cloud note above About. Games the user owns on the list:
    Fortnite and Grand Theft Auto V (Denied), Fall Guys (Running),
    Rocket League and Killing Floor 2 (Supported) on Epic; Rust
    (Denied), 7 Days to Die and Cry of Fear (Supported) on Steam.
    Tried by the user 2026-11-09: it works [firm].
    18b, built 2026-11-09 (`steam_deck.rs`, the window's
    `steam_deck.rs`). Checked first: Heroic shows only an icon per
    category (0 and 1 grey, 2 yellow, 3 green); Steam's own strings
    (`steamui/localization/shared_<language>-json.js` in a Steam
    install) name 0 to 3 Unknown, Unsupported, Playable and Verified and
    hold 41 test results, as JSON in a webpack script with JavaScript
    escapes; Half-Life: Alyx (VR) is 1, Fallout 3 2, Cyberpunk 2077 3.
    `display_type` 4 is a passed test, 3 a warning, 2 why a game is
    unsupported, 1 something to know; a game Valve has not reviewed
    answers `"results": []`. Zendo [tentative]: a pill beside
    ProtonDB's, DECK VERIFIED (good), DECK PLAYABLE (ProtonDB's gold)
    or DECK UNSUPPORTED (dim), none for Unknown or no review; its
    tooltip says what the category means and lists the notes that are
    not passed tests, in Zendo's own words for the 37 such tests in
    English and Portuguese (Portugal), not Valve's text; a test Zendo
    does not know shows its name as words ("Reads like this VR").
    Clicking opens the game's Steam store page. Asked by the Steam app
    id once ProtonDB's lookup found one, kept a week in
    `$XDG_CACHE_HOME/zendo/steamdeck.json`. The hero's one pill slot
    became a row of pills. Checked: clippy, tests (Fallout 3's and
    Half-Life: Alyx's shapes, kinds and tokens Zendo does not know left
    out, no review, a failed answer, the pill and its lookup after
    ProtonDB, a test's name as words); an ignored test against the real
    store (Cyberpunk 2077 Verified, an unknown app none); in a throwaway
    home with only Rocket League's Epic record, the page showed
    PROTONDB PLATINUM and DECK VERIFIED. Of the 55 games the user had
    looked up, 22 are Verified, 13 Playable, 9 Unsupported and 11 have
    no review. Tried by the user 2026-11-09: it works [firm].
    18c, built 2026-11-09 (`requirements.rs`, the window's
    `requirements.rs`). Checked first: of 20 of the user's GOG games, 15
    had requirements on GOG (older ones, Dark Forces II, BioShock 2, the
    Savage Frontier games, none); Epic's store-content answered for 3 of
    10 Epic games, only those whose store slug has no code on the end
    (`world-war-z`, not `botany-manor-e7c456`); Steam's `appdetails`
    needs no key, though its `filters` drops the requirements fields.
    Zendo [tentative]: the game's own store first (GOG v2; Epic's slug
    from the launcher GraphQL by the namespace in legendary's record,
    then store-content), else Steam's store page for the Steam app id
    ProtonDB's lookup found (Amazon's and local games always); in
    English, as Zendo's other store details; for Windows, or Linux for a
    game installed as a Linux build, falling back to Windows when no
    store lists Linux's. Steam's HTML is read through the plain-text
    reader into "Label: value" lines, a line with no label kept as it
    is. A System requirements card at the end of the main column on
    both pages: where they came from ("Windows · from GOG"), then each
    row with Minimum and Recommended side by side, or one column when
    the store gives no recommended. Kept a week in
    `$XDG_CACHE_HOME/zendo/requirements.json`, by game and system.
    Checked: clippy, tests (Steam's, GOG's and Epic's shapes, levels
    paired by label, lines with no label, none listed); an ignored test
    against the real stores (Conarium's Linux ones from GOG, World War
    Z's from Epic, Cyberpunk 2077 on GOG from Steam as GOG lists none,
    an unknown GOG game none); in a throwaway home, Rocket League's page
    (with a build that moved the card up for the screenshot, reverted)
    showed "Windows · from the Epic Games Store" with OS, Processor,
    Memory and Storage at both levels. Tried by the user 2026-11-09: it
    works [firm].
    18d, PCGamingWiki's facts, dropped by the user 2026-11-09 [firm]: no
    value seen for Zendo now.
    18e, checked first (2026-11-09, read-only, with the user's own
    sign-ins, as the user agreed; no token printed or kept):
    - GOG: `gameplay.gog.com/clients/<id>/users/<user>/achievements`
      with gogdl's token answers the same for the product id
      (1423049311) and the client id from `goggame-<id>.info`
      (53652320436400298), so a game not installed works too.
      Cyberpunk 2077: 57, one unlocked; each with `achievement_key`,
      `name`, `description`, `date_unlocked` (null when locked),
      `rarity` (percent), `rarity_level_description` ("Uncommon"),
      `visible` and both images.
    - Steam: with the sign-in's `access_token`,
      `IPlayerService/GetGameAchievements/v1` gives the list (names,
      descriptions, `icon` and `icon_gray` files, `hidden`,
      `player_percent_unlocked`); `IPlayerService/GetAchievementsProgress/v1`
      (POST only) gives unlocked and total per app (Rust 58 of 116).
      `ISteamUserStats/GetPlayerAchievements` and `GetSchemaForGame` want
      a Web API key, which Zendo no longer has (dropped with 11c). Which
      ones are unlocked, and when: the profile's
      `steamcommunity.com/profiles/<id>/stats/<app>/achievements/?xml=1`
      (no sign-in; `closed="1"` and `unlockTimestamp`; 58 for Rust), only
      while the profile's game details are public (the user's are).
    - Epic: the store GraphQL (`store.epicgames.com/graphql`)
      `Achievement.productAchievementsRecordBySandbox(sandboxId:
      <namespace>)` gives the list without a sign-in (`productId`,
      names, `rarity.percent`, `hidden`, icons): Rocket League 88,
      Farming Simulator 22 43, World War Z none, some games null. The
      player's own: `PlayerProfile.playerProfile(epicAccountId)
      .productAchievements(productId)` with legendary's token
      (`stores/epic/user.json`; refused without it): Rocket League 26
      unlocked, each with `unlocked` and `unlockDate`. The token lasts
      about two days; legendary renews it when it runs.
    - Amazon: not checked.
    Decided by the user 2026-11-09 [firm]: an Achievements card with the
    count and a bar, then each achievement with its icon, name,
    description, and its unlock date or how rare it is; unlocked first
    (newest first), then locked (most common first); ten shown, with
    Show more. A locked hidden achievement shows as "Hidden achievement",
    its name and description kept back until unlocked.
    18e, built 2026-11-09 (`achievements.rs`, the window's
    `achievements.rs`). Zendo [tentative]: GOG's from gameplay.gog.com
    with gogdl's token; Steam's list from `GetGameAchievements` (no
    key) and the unlocks from the profile's public stats page, for the
    Steam sign-in's account, else the one Steam on this computer signed
    in last; Epic's list from the store GraphQL by namespace and the
    unlocks with legendary's token. Unlocks not known (a private Steam
    profile, a stale Epic token) leave the list without them, saying
    why. Asked once a session as the page opens; icons kept in
    `$XDG_CACHE_HOME/zendo/achievements/`, each address downloaded once
    (Rust's locked ones share icons; two threads writing one file
    raced), with the extension the bytes call for (the window draws
    nothing for Epic's icons, whose addresses have none). Steam's icons
    come from `shared.fastly.steamstatic.com/community_assets/images/apps`,
    as the profile page's do: the older `cdn.fastly…/steamcommunity/
    public/images/apps` lacked ten of Rust's grey ones. The card sits
    near the end of the main column, before the requirements. Checked:
    clippy, tests (each store's shape, GOG's `+0000` dates, Steam's names
    matched ignoring case, a private profile, Epic refusing, the order,
    icon kinds, the card and Show more); an ignored test against the real
    Steam without a profile (Rust's 116, unlocks not known, icons); once,
    with the user's sign-ins (read-only, the test removed after):
    Cyberpunk 2077 on GOG 1 of 57 (20 hidden still locked), Rocket League
    on Epic 26 of 88, Rust on Steam 58 of 116 (9 hidden still locked),
    every icon there; in a throwaway home with no sign-ins, Rocket
    League's page (a build that moved the card up, reverted) showed 88
    achievements with icons and their share of players. Tried by the
    user 2026-11-09: it works [firm].


19. (was 22) The library's categories, the user's own groups. Planned with
    the user 2026-11-10: a flat list, not nested, and a game may be in
    several; chips in the library's toolbar beside the store chips, sorted
    by name, narrowing together with the store chip and the search words;
    made, renamed and deleted in a sheet opened from the toolbar (a delete
    asks once and drops the category on the second press, as removing a
    game added by hand does); assigned from a game's page or from a
    selection in the grid; and
    `zendo categorize` and `zendo uncategorize` for keybinds. Kept in
    `state.json` as `categories`, a name to the ids of the games in it:
    both the window and the command write it, as they write hidden and
    pinned, and a name is the identity, so a rename is a key move. A name
    is found trimmed and whatever its case, and a category whose games all
    left the library is kept. Selecting uses modifiers (Ctrl+Space, a check
    in each cover's corner, Esc to leave), since bare Space and `c` are
    typing keys for the filter (`app.rs`); a selection mode was offered
    and declined, and so was a controller binding: X, Y and Start are taken
    in the library, so marks are keyboard and mouse only. Built 2026-11-10 in seven commits: the model, the sheet,
    the chips, the game page's row, the marks, the verbs, the documents.
    Checked: clippy, tests (13 new in the window, 7 in the core: the
    round trip and a file from before the field, blank and case-taken
    names, rename with a clash and a change of case only, delete, setting
    only where the category exists, the sorted order, `Library::apply`,
    the AND with the store and the search, the cursor kept, a chip
    pressed twice, the sheet's create, rename, delete and refuse, the
    assignment of several targets, the button's own game, the tick, and
    the marks cleared by Esc, a rescan and a page change); the verbs by
    hand in a throwaway home. Tried by the user 2026-11-10: it works
    [firm]. After the trial the mark on each cover became a drawn square
    instead of the `+` and `✓` glyphs, which the system font set at
    different heights and, on some fonts, has no check mark for; the
    square fills with the good colour when marked and is outlined when
    not, with no glyph to misplace.

## The whole-workspace review and its must-fix findings, 2026-11-10

Read-only, ten passes over the two crates: five over `zendo-core` with the
Rust review rules, five over `zendo` with the Rust and Iced rules. Every
report is kept in `.mallet/reviews/2026-11-10/` (untracked). 144 findings:
101 Rust, 43 Iced, 25 of them Must fix. The tooling at the time: `cargo fmt
--check` clean, `cargo clippy --all-targets --all-features -- -W
clippy::pedantic` with no warning, 119 window tests and 222 core tests
green.

The 25 Must fix, all built the same day:

- Names read from files stay inside the folders they name: a mod id
  (`mods.rs`), a GOG save location from remote config (`cloud_saves.rs`),
  a local game's id and Steam's `installdir` (`launch.rs`, `steam.rs`).
- A store record that cannot be read no longer reads as "nothing
  installed": `discard`, a move's `delete_old` and `run`'s post-check
  refuse instead of deleting a folder or blaming the engine
  (`downloads.rs`); a move's Epic records are undone together
  (`downloads.rs`); GOG's own setup steps run for games with no
  redistributables (`gog_setup.rs`).
- In the window: a queued mod's install waits for the slot instead of
  being dropped (`installer.rs`, `mods.rs`); a checked key's account is
  taken only when the key is the stored one (`nexus.rs`); an answer is
  matched to its request for the SteamGridDB key (`settings.rs`,
  `KeyChecked` carries the key), the art picker (`artwork.rs`) and a
  sign-in (`accounts.rs`); starting a sign-in closes the previous window,
  and one the engine finished after Cancel still counts.
- On outside input: an image answer is capped at 16 MiB and a refused
  status keeps its type, so the SteamGridDB fallback runs (`covers.rs`);
  key-values in a Steam cache nest no deeper than 32 (`appinfo.rs`); a
  date's year is bounded (`time.rs`); one vanished X window no longer
  abandons the close sweep (`windows.rs`); `main` refuses a non-UTF-8
  argument instead of panicking.
- Settings are written into the file as it is now, under its lock
  (`Config::update`), so a save no longer overwrites a hand edit or
  another writer with a copy read at startup (`config.rs`, and its
  callers).
- Iced: overlays take keys in the order they are drawn, topmost first,
  and Escape reaches the sheet before a focused field can keep it
  (`app.rs`); the game page no longer stats the disk to draw its Open
  buttons, and a failed `xdg-open` says so; a Proton or a runtime is
  removed on a thread (`protons.rs`, `runtimes.rs`).

Left open that day: `save_config` still read the file and asked the
keyring on the window thread, now without the data loss it had. Closed
later the same day, after the user's first tries: every settings write
is now a `Setting` applied to the window at once and written by
`Config::update` in a `blocking` task (`settings.rs`,
`Message::ConfigSaved`); a write that fails puts the settings, the
language and the colours back and says so in the status line. The Nexus
key, Nexus's adult filter and the default Proton went through the same
path, which used to read the keyring in `update` too. The other Should
fix and Consider findings are in the reports above and not built.

## The user's first tries after the review, 2026-11-10

- The category sheet's tick boxes were green, Iced's success colour, not
  the palette's. Every tick box now follows the window's colours
  (`style::checkbox`: the accent when ticked, surface and line when not),
  in the category sheet, the FOMOD options, a mod's and a plugin's
  switch, and a store game's DLC list.
- Cat Quest II's saves did not sync. legendary 0.21.1 works out
  `{AppData}`, `{InstallDir}`, `{UserDir}`, `{UserSavedGames}` and
  `{EpicID}` in a Wine prefix but not `{UserProfile}`, which Cat Quest II
  names, so it warned of unprocessed variables and skipped the game.
  Zendo now reads the game's `CloudSaveFolder` from legendary's own
  metadata when that warning comes, puts the variables into the prefix as
  Proton makes it and retries `sync-saves --save-path` once
  (`cloud_saves.rs`). Checked against the real record: the folder worked
  out is the one Cat Quest II writes to
  (`Prefixes/Cat Quest II/drive_c/users/steamuser/Documents/Cat Quest
  II`), and legendary then found the save (local newer, 2026-11-10). Run
  with both skip flags, so no save moved.


## The core quick pass over the review's findings, 2026-11-11

The user asked for the review's non-blocking findings that are quick and
cannot break the system, and agreed to two passes: the core first, then the
window. This is the core pass: 32 of the reports' Rust findings in
`crates/zendo-core`, all the mechanical ones. The window's findings and the
core ones that change behaviour, stored data or threading are still open.

- `rust-facts`: three caches read and write under `paths::locked` again, with
  the network call outside it (`protondb.rs`, `steam_deck.rs`,
  `requirements.rs`); a non-2xx answer is no longer "the store has none"
  (403 and 404 stay "none", the rest are errors); `nexus::identify` skips
  only a 404 and reports a rate limit or an outage; a bad `expires` in a
  Nexus link is an error, not a key silently dropped; the report scrubs the
  Steam session's token on its own; Steam's unlocks match `closed` in any
  attribute position; sizes from Nexus, Steam's depots and GOG saturate; one
  malformed AreWeAntiCheatYet entry no longer loses the whole list; body-read
  errors name the endpoint.
- `rust-launch`: `dedup` sorts first; a quote left open in a store task's
  arguments is an error instead of silently launching with none; unpack and
  cache paths say which path failed; `split_first` replaces `words[0]` at
  the three sites; a non-engine game in `cloud_saves` is an error, not
  `unreachable!`.
- `rust-mods`: the "outside the mod" error names the source or the
  destination that left it; the 7-Zip test also checks `bsdtar`; a record
  header outside 12 to 24 bytes is refused, not sliced; the FOMOD read
  sites use the total accessors; the module doc matches the inactive-plugin
  behaviour; parse errors say "could not parse"; `Overview::target` does
  not end in a slash.
- `rust-spine`: a category's identity uses the same Unicode lowering as its
  order; `Key::index()` replaces `key as usize` and the held array is
  `Key::ALL.len()`; a failed rename removes the temporary file, and a failed
  directory fsync no longer fails the write; `parse_utc` refuses a day the
  month does not have; `Config`'s equality is over the settings only;
  `command_line` quotes a quote, a backslash and the empty word.
- `rust-stores`: the prefix's setup record is written under its lock; a
  failed `gogdl import` keeps what gogdl said; `words_for` returns an error
  instead of `unreachable!`; the Store list is de-duplicated by id with a
  set, not neighbours; GOG's pagination stops after 200 pages.

Deliberate differences from the reports' suggested code:

- 404 counts as "none" beside 403 in `call_text`/`get_text`: Epic's content
  URL answers 404 for a page a game does not have, and turning that into an
  error would report "no requirements" as a failure.
- `owned::keep` de-duplicates with a `HashSet` rather than `dedup_by` after
  the sort: sorting by title does not put two different titles for one id
  side by side, which is the case the finding names.
- The core findings left open are `rust-facts` 11 (a note: the envelope has
  no message field to surface yet) and `rust-launch` 7 (the umu id lookup,
  which changes `prefix_command`'s shape).

Checked: `cargo fmt --check` clean, `cargo clippy --all-targets
--all-features -- -W clippy::pedantic` with no warning, `cargo test
--workspace` green (228 core, 120 window). Six tests are new: the parse
skips a bad anti-cheat entry, the category case rule is Unicode, the report
hides a session token, `parse_utc` refuses February 31 and April 31 and
accepts 2024's leap day, `command_line` round-trips a quote, a backslash and
an empty argument, and Steam's unlock survives an attribute before `closed`.

The Epic gap from phase 5a is also closed: the user installed and played Cat
Quest II through legendary on 2026-11-10, and Let's Build a Zoo the same
evening (see phase 5a).

## The window quick pass over the review's findings, 2026-11-11

The second pass, the window's mechanical findings: 26 built. Like the core
pass, the ones that change interaction, layout or threading are left for a
session where each can be tried.

- Shell: `Language::current` decodes through `Language::ALL` and a test pins
  each index; `links::bind` refuses a symlink before chmod; `uncategorize`
  checks membership before `State::update` writes; doc comments moved to the
  items they describe; the footer formats `Downloads::summary()` once; the
  footer's 10 px padding became 8; `reveal`'s arithmetic is `reveal_y`, with
  the three cases tested.
- Library: `Source::ALL` in the core replaces the window's copy of the list,
  with a duplicate check; `queue`/`dlc_job` results are filtered after the
  fact instead of `bool::then(...).flatten()`; `cards_grid` takes the count
  from its `ExactSizeIterator`; the "assign a category" chip no longer looks
  chosen.
- Store: `open_owned` clamps the cursor like the drawing does; a failed
  Steam-list write is in the status line; the asking and error lists follow
  `Shop::ALL`; `has_list`/`forget` take a `Shop`; the empty page says which
  filter found nothing; a paused Steam download says so; the progress bar is
  drawn only once a percentage exists; a finished job's Open goes to the
  game's page only when the scan already knows it.
- Accounts: the page's capped column is centred, and Checking keeps the
  Sign in button's slot, disabled.
- Settings: the sources list says "Scanning…" until the first scan is in,
  and a disabled secondary button is drawn faded.
- Mods and Nexus: the two untranslated status lines are in the catalogue;
  a Nexus file the user asked for reports an identification failure instead
  of being added without its identity; one mod's file list failing no longer
  hides the other mods' updates.

Deliberate differences from the reports' suggested code:

- The window has no `mod spacing` module of 8-point tokens, so the footer's
  padding is the numbers 8 and 24, matching the rest of the window.
- The footer's summary is computed once per frame rather than only in the
  branch that shows it, since the branch is not known first.
- `add_as_from_nexus` (`mods.rs`) keeps its best-effort lookup: an archive
  added by hand whose Nexus identity cannot be found is still added as any
  other mod, now with a comment saying so.

Checked: `cargo fmt --check` clean, pedantic clippy with no warning,
`cargo test --workspace` green (229 core, 122 window). Two window tests are
new: the language index round trip, and `reveal_y`'s above, below and
already-visible cases. The rest are covered by existing tests, which pass.

The window findings still open are the ones that need a look at the screen
or a decision: the overlay input order's neighbours (per-page scroll, the
palette global, Ctrl+F on the Store), the grid's virtualisation and
re-sorting in `update`, the game page's width and type scale, the toolbar's
grouping and wrapping, the FOMOD installer's pad input and footer, the
launch-options form's sticky footer and field errors, the card-fetch error
states, and the filesystem reads in `view`.

## The engineering pass, 2026-11-11

The third pass over the review, after the two quick ones: the findings that
change state or scheduling but need no visual decision. 26 built; the few
that add a new card state or a test suite are left for the batch that tries
the window.

- Core: `gog_playtime` takes the queue under its lock and sends outside it,
  putting back what could not be sent before anything added meanwhile; the
  umu id is passed into `prefix_command` instead of overwritten, so no
  lookup is made for it; a hand-broken `gamescope_more` is a warning on
  `Started` rather than silently fewer options; a mod's files and installer
  are moved aside and put back when the list write fails (`mods/add.rs`);
  `.nam` is found in any case; a symlink or unreadable mod folder is an
  error in `overview`, not "0 files"; `Sort::compare_with_titles` takes the
  lowered titles once; a Stop after a successful install is `Done`, not a
  job that can never resume; one GOG games-database record failing keeps the
  rest of the library and is reported; `Comet::drop` signals its whole
  group; a hand-edited `local.toml` whose arguments have an open quote is a
  problem line; nile's sign-in asks the account first and uses the prose
  only when it is not there yet.
- Window: a panic's payload reaches the status line
  (`panicked-with`); `visible()` lowers each title once; `WindowReady` is
  its own message, so the controller is not armed before a focus event; the
  palette comes from `Themes`, not a global, in the application's theme;
  scroll events name their grid and one from the page being left is
  ignored; Ctrl+F on the Store focuses its search field; a card click and a
  mark carry the game's id, in the library and the Store; the cover
  placeholder and the card button's style live in `style.rs`; a saved queue
  arriving under a running job is merged instead of reordered; the finished
  job is read through checked accessors; the Accounts page caches each
  engine found by the check instead of walking `PATH` in `view`; the two
  early returns of the Downloads page scroll; cancelling a FOMOD clears its
  scratch folder through `blocking`.

Deliberate differences from the reports' suggested code:

- The window keeps one `viewport_height` for both grids: the window's
  height is the same for both, and naming the grid already stops the
  misapplied event. Per-page heights would only matter for a few pixels of
  scrollbar.
- `queue[0]` is still used to remove the finished job after the front has
  been checked; the reads that described it now go through `first()` and
  `first_mut()`.
- `add_as_from_nexus` remains a best-effort lookup for an archive added by
  hand; only the file the user asked Nexus for reports an identification
  failure.

Deferred to the next batch: the four card fetches keeping their error and
offering Retry, "Copied the report" waiting for the clipboard, the
filesystem reads in `view` on the Proton and Settings pages, and the test
suites the reports name for the library, the pages and the mods.

Checked: `cargo fmt --check` clean, pedantic clippy with no warning,
`cargo test --workspace` green (229 core, 122 window). The panic test now
expects the payload's message, and the test window sets `focused`, since a
focus event cannot arrive in a test.

## The leftovers, 2026-11-11

After the three passes, the rest of the review's findings, plus the visual
batch, were built and then looked at on screen.

- The four cached cards keep a failure: `Failed(String)` in requirements,
  achievements, anti-cheat and the Steam Deck pill, with Retry through
  `Message::Retry` (the pill's reason in a tooltip). The reason no longer
  disappears into "the store lists none".
- The report note says "Report gathered", not "Copied": `iced` 0.14's
  `clipboard::write` gives no result to wait for, so the note states only
  what is known.
- No filesystem reads in `view`: the Proton row's `games_using` is counted
  when Remove is pressed (`confirm_remove` holds the count); the Settings
  file and log paths are found when the page opens (`settings_paths`).
- Stored-data policies, as agreed: a corrupt `state.json` is renamed to
  `state.json.bad` and the next start is empty; Epic's launch options are
  keyed by `extraLaunchOption_<n>` with a display-name migration in the
  sheet and in `store_tasks::chosen`; `local::put` refuses a vanished id; a
  failed Apply writes the previous profile list back; removing a Proton
  sets the games that chose it back to Automatic; winetricks components are
  per game (`LaunchOptions::winetricks_components`, loaded with the page);
  the Nexus domain is filled in every mods entry point through `look`.
- The Settings key flow has a test now; it found that removing a key left
  an in-flight check able to write the old key back, so the empty-key path
  clears `checking` and `save_after_check`. The category delete confirm and
  `played_line` have tests too.
- Visual batch: the game page is capped at 1280 and centred; the type
  scale is 38/18/14/13/12; the library toolbar is rows of groups that wrap
  between groups; a card's caption is two lines of title plus the played
  line (60 px, `WordOrGlyph`); 20/18/5 px spacings became 24/16/4; the
  zero-sized Launch options and Local buttons are left out instead of
  spacing the bar; the FOMOD installer takes arrows to focus and Enter to
  pick, with the focused row marked; the launch-options sheet has a pinned
  footer and the error beside Save; Proton and Sources rows wrap.
- Looked at on screen with `cargo build --release`, `grim`, and `ydotool`:
  the library (grouped toolbar, two-line captions, five columns), the game
  page (hero, pills, action bar, two columns), the launch-options sheet
  (pinned footer, scrolling body), the Settings page, and its lower
  sections after a scrollbar drag. The Proton release row was fixed then:
  a `Fill` cell inside a wrapping row pushed the Download button onto a
  line of its own even when there was room, so the row uses natural widths.

Left open from this batch: clicking the dimmed backdrop still does
nothing, a long launch-options form still closes without asking, validation
errors are pinned beside Save rather than at their field, and the test
suites named for the pages and mods are only partly added. Item 20 is next.

Checked: `cargo fmt --check` clean, pedantic clippy with no warning,
`cargo test --workspace` green (229 core, 126 window).

## The second look at the leftovers, 2026-11-11

The user looked at the screens and asked for two changes, and the rest of
the open items followed.

- The game page is full width again, as the store's game page is: the 1280
  cap is gone. Settings and About are centred like Accounts, keeping their
  960 and 860 widths.
- The launch-options sheet's dimmed backdrop closes on a click now; only
  the card is opaque, so a click outside it reaches the backdrop.
- Closing the sheet with changes asks first: `Sheet::build` makes the
  options the form would save, and `Close` compares them with the ones the
  sheet opened with. Keep editing or Discard.
- Validation errors sit at their field: arguments and environment under
  their inputs, the gamescope sizes under the size rows; the rest stay
  beside Save.
- Tests: the page's sections open and close, a Nexus update with one failed
  file list keeps the others and says so, and the close guard asks only
  when there is a change. 129 window tests, 229 core.

On screen, the footer of the launch-options sheet was drawn but did not
take clicks: the `max_height` container left the footer outside its hit
region. The sheets now take the height they are given (the 760 and 640 caps
are gone), so the footer is inside the card and its buttons work; checked
with Cancel, Keep editing and Discard. The backdrop click and the field
error for the environment were checked too.

## Next session, as logged to 2026-11-09

- The list after phase 7 (above), confirmed 2026-11-07. Item 1 built
  and checked by the user the same day. Item 2, the Rust review, fixed
  the same day (findings 1–10; 11 to fix later [firm]). Item 3, the
  API keys in the keyring, item 4, diagnostics, item 5, languages, and
  item 6, themes, and item 7, the AUR package, built the same day; item
  8, the survey of Heroic and Hikari, done the same day, added items 16
  to 23 (their order confirmed; 13 to 20 since 2026-11-09). Item 9, the game page with DLC, built
  the same day. Item 10, the EAC and BattlEye runtimes, built the same
  day. Item 11, Steam's owned games, built the same day,
  and changed after the user's two tries (2026-11-08). Item 11b, the
  Steam sign-in with the family's games, built 2026-11-08, tried by the
  user with the QR code 2026-11-09; the old key dropped and item 11c,
  Steam's extra details, the same day. Item 12, searching Nexus Mods in
  Zendo, built and tried by the user 2026-11-09. Finding 11 fixed the
  same day; the list then reordered by the user. Item 13, the launch
  options sheet, built in three parts (13a to 13c) and tried by the
  user 2026-11-09; 14a (Epic's switches and overlay) and 14b (cloud
  saves) the same day, both tried by the user. 14c, GOG's played time
  sent back, built the same day and tried by the user, then GOG's total
  on the game's page (tried). Item 15, the prefix's tools, built the
  same day and tried by the user. Item 16 split by the user: 16a,
  GOG's setup, built the same day and tried by the user; 16b, the
  known fixes, built the same day and taken as working by the user; the
  desktop entry and Steam shortcut dropped. 16c, GOG Galaxy for GOG games
  (comet), built the same day and tried by the user.
  Item 17, managing installs, split by the user (17a to 17d) the same
  day; 17a, moving a game, built the same day and tried by the user.
  17b, importing a game from a folder, built the same day. 17c, the
  download workers and automatic updates, built the same day and taken
  as working by the user. 17d, GOG's changelog on the game page, built
  and tried by the user the same day. Item 17 is done. Item 18, what
  the game page tells, checked and split by the user (18a to 18e) the
  same day; 18a, the anti-cheat status, built and tried by the user
  the same day; 18b, the Steam Deck rating, built and tried by the
  user the same day; 18c, the requirements, built and tried by the
  user the same day; 18d, PCGamingWiki's facts, dropped by the user;
  18e, achievements, built and tried by the user the same day. Item 18
  is done. Next: item 19, the library's categories.
- Items 3 (keys in the keyring), 4 (diagnostics), 5 (languages), 6
  (themes) and 9 (the game page and DLC) tried by the user at home,
  2026-11-09: they work [firm].
- Phase 7 is done. 7e, Bethesda plugin order, built 2026-11-07; the
  user tried it on Fallout 3 GOTY (Steam 22370) the same day: it works.
  Before that, Fallout 3 was made
  known (Steam ids 22300 and 22370, `Data`, `fallout3`) and games are
  matched on Nexus by the part of the title before " - " or ":" too;
  the user confirmed the Nexus button shows.

## Next session, as logged to 2026-11-10

- Item 19, the library's categories, planned with the user and built
  2026-11-10; tried by the user the same day: it works [firm]. The
  whole-workspace review and its must-fix findings followed the same day
  (above); all 25 are built, the settings save last, after the user's
  first tries found the green tick boxes and Cat Quest II's saves
  (above). On 2026-11-11 the first of two quick passes over the review's
  non-blocking findings was built (the core's, above); the window's pass
  and the findings that need a try-out or a decision are next, before
  item 20, the window (minimise when a game starts, Discord presence,
  rescanning when a launcher's files change, processes tracked by their
  start time).

## Item 20, the window, 2026-11-11

Planned with the user the same day, then built and tried by the user.

- **The tray.** `crates/zendo/src/tray.rs`, through the `tray-icon` crate's
  `ksni` backend (a freedesktop `StatusNotifierItem`; it keeps a thread of
  its own, so the Iced window needs no GTK loop). The icon is
  `data/zendo.svg` rasterised into `crates/zendo/tray-icon.png` (64 px, the
  charcoal ensō with its orange dot), decoded with `image` and set as the
  64×64 pixmap. Both the tooltip and the `Title` are "Zendo": a panel lists
  the item by its title, and without one it showed a numeric id. A menu
  with Open and Quit.
- **Hiding to the tray.** The close button (X) and a game's start hide the
  window when `minimize_to_tray` is on (default off, Settings' Window
  section). The hide is `window::minimize(id, true)`, not Iced's
  `Mode::Hidden`: winit 0.30's Wayland backend makes `set_visible(false)` a
  no-op (`// Not possible on Wayland`), so a "hidden" window never hid.
  Minimising works because Hyprland's `minimise.lua` turns `window.minimize`
  into a move to `special:minimized`.
- **Restoring.** On X11 `minimize(false)` and focus bring it back. On
  Wayland an app cannot unminimise itself, so Hyprland is asked through
  `hyprctl`. The first try used `hl.dsp.focus`, which restored the window
  but warped the pointer to its centre; the user asked to avoid that.
  `hyprmonk_focus` (hyprmonk's `minimise.lua`) sets `cursor.no_warps`
  around the focus and puts it back after, which is the dock's "without
  moving the pointer". Zendo calls `hyprmonk_focus` when it is defined and
  falls back to `hl.dsp.focus` otherwise. The tray's Quit goes through the
  same close path, so a running download still asks first.
- **Minimising on start.** A game is hidden as it is seen running: at once
  for a game Zendo starts itself (its process group), and when its
  processes turn up for a Steam or Heroic game. Only with the setting on
  and a tray.
- **Discord presence.** `crates/zendo/src/presence.rs` on the
  `discord-rich-presence` crate (1.1.0, MIT), a thread of its own: it
  connects to Discord's local IPC with Zendo's application id
  (`1570036457047986256`), shows "Playing", the game's title and the time it
  started, clears when it ends, and reconnects as Discord comes and goes.
  Off by default; Settings' Discord section turns it on. The game shown is
  the most recently started still-running one.
- **Rescanning on a change.** `crates/zendo-core/src/watch.rs` gives the
  modification times of the launchers' files (Steam's `libraryfolders.vdf`
  and each `appmanifest_*.acf`, Heroic's `installed.json` records, the
  `.desktop` applications folders) as one sorted list; the window keeps the
  last list and polls every 10 s, calling `refresh()` when it differs. Only
  the launchers' files, never Zendo's own, so Zendo never rescans itself.
  Always on.
- **Settings.** Two sections, Window (`minimize_to_tray`) and Discord
  (`discord_presence`), each a `Setting` written by `Config::update` as the
  others are.

Decisions [firm]: close hides to the tray and only the tray's Quit exits;
the minimising button stays a taskbar minimise; the Discord application id
is Zendo's own, baked in, and presence is off by default; the orange dot is
part of the icon.

Checked: `cargo fmt --check` clean, pedantic clippy with no warning,
`cargo test --workspace` green (231 core, 129 window; two watch tests are
new). On screen, through `grim`, `ydotool` and `hyprctl`: the tray
registered as `org.kde.StatusNotifierItem-<pid>-1` with tooltip and title
"Zendo" and the 64×64 icon; the left click minimises and restores the
window, the restore does not move the pointer, and the tray's Quit exits.
Not seen live: the Discord presence (Vesktop was running, but the status
was not watched); the id is real, so it should show once Discord's Activity
Status is on.

Left open: on Wayland the minimised window stays in hyprmonk's dock rather
than vanishing from it, as that dock is built to restore minimised windows;
and restoring off Hyprland (another Wayland compositor) still needs the
compositor rather than Zendo.

## The move to Fedora 45 and the reformulation, 2026-11-12

The user reinstalled the PC with Fedora 45 (KDE Plasma 6, Wayland); until
then Zendo was built on Arch with Hyprland, next to hyprmonk. The sibling
project on disk is now `plasmonk-kde` (KDE, matugen colours), and hyprmonk is
not there. Heroic, its engines, umu-launcher and every game manifest are gone
with the old install, and so is `.mallet/reviews/2026-11-10/` (gitignored).

Set up, by the user's own commands (Claude only checked):

- `sudo dnf install gtk3-devel webkit2gtk4.1-devel libsoup3-devel
  javascriptcoregtk4.1-devel systemd-devel ydotool kdotool`. Fedora 45 still
  ships WebKitGTK 4.1 for GTK 3 (2.54.2), so the sign-in window is unchanged.
- Rust 1.99.1 through mise (`mise use rust@1.99.1`, which wrote `mise.toml`;
  mise links it to a rustup toolchain in `~/.cargo/bin`, clippy and rustfmt
  included).
- Without Heroic, from each project's GitHub release through mise's `github:`
  backend: legendary 0.21.1 (derrod's; dnf has only 0.20.34), gogdl 1.4.0,
  nile 1.2.0, comet 0.4.1 (its release also holds `dummy-service.zip` with
  `GalaxyCommunication.exe`) and umu-launcher 1.4.4 (the zipapp; Fedora RPMs
  stop at fc44). All answered `--version` on `PATH` under their plain names.
  Zendo already looks on `PATH` before Heroic's folders
  (`stores.rs` `bundled`); only `GalaxyCommunication.exe` is taken from
  Heroic's folder alone (`galaxy.rs` `install_service`), which item 27 fixes.

Checked on the new machine: `cargo build --workspace` (1m44),
`cargo test --workspace` (129 core and 231 window pass, 23 ignored, none
failing) and `cargo clippy --workspace --all-targets` (no warning). On KDE, in
a throwaway home with no session bus: the window opens and draws; KWin sees
it as class `zendo`; `spectacle -b -n -a -o` takes a still of it; a Tab
through `ydotool` (run as the user, since Fedora's `ydotool.service` is a
root system unit and a socket path over 108 characters fails) moved the sort
chip from Recent to Most played; `kdotool` minimised, reactivated and closed
it. The three games it listed were KDE's own `.desktop` entries.

Not checked: the tray hide and restore on KDE. Zendo reads its API keys from
the keyring at start, so a run with the tray needs the real session bus and
so the user's real KDE Wallet; the user chose to skip it until item 27 has
reworked the restore, which was only ever tried on Hyprland. Also not seen:
whether `grim` and `wtype` work on KWin (not installed), the dummy-service
step without Heroic, and any store feature with a real account.

Decisions [tentative until the user confirms the order in docs/backlog.md]:
generic platform with no Hyprland code, Fedora first, the theme from the KDE
colour scheme, and a complete redesign in the style of omakade, built in Iced
with ideas and look only (it is GPL-3.0-or-later; `docs/design.md`).
Recompiled games are on hold; emulation moves up. The backlog is renumbered
from 27 so it does not collide with the built items cited above.

## Item 27, the generic platform, 2026-11-12

Planned with the user the same day. Decided: Zendo downloads comet's
`dummy-service.zip` itself, and the hyprmonk tone's removal moves to item 28
so the file watch is changed once.

- **The tray restore, without `hyprctl`.** Hiding now closes the window and
  restoring opens a new one, so `run()` is `iced::daemon` and every way out
  ends in `iced::exit()` (`downloads.rs` already did). The activation-token
  route is closed: ksni gives the token only inside `activate` and menu
  clicks, tray-icon 0.25.1's `activate` only emits a Click, winit 0.30.13's
  Wayland `focus_window` is empty and uses a token only when a window is
  created, iced 0.14's `PlatformSpecific` has no field for one, and
  raw-handle Wayland calls need `unsafe` (forbidden). Spike first (a tray
  icon and a daemon window in a throwaway crate, no keyring): the user ran it
  on KDE, and a window opened from a tray click came up and worked as
  expected, for repeated hides and restores. `hide_to_tray` takes
  `self.window`, clears `focused` and closes it; `show_from_tray` opens a
  window of the last size and mode (`window_size`, `fullscreen`) and resets the
  grids' `scroll_top`, since the new scrollable starts at the top and the next
  resize reveals the selected card. A tray Quit while hidden with a download
  running reopens the window first, as the question is asked there.
  `--fullscreen` now also sets `Zendo.fullscreen`, so F11 starts right.
- **No Heroic for GOG Galaxy.** `galaxy.rs` `service_exe`: Zendo's own copy in
  `$XDG_DATA_HOME/zendo/galaxy/`, else Heroic's, else
  `dummy-service.zip` from comet's latest release, unpacked with
  `mods::add::unpack` (now `pub(crate)`) and checked to start with `MZ`.
  The zip (checked 2026-11-12) holds `GalaxyCommunication.exe`,
  `install-dummy-service.bat` and `update-permissions.exe` at its top level.
- **Words.** `accounts-engine-missing` (both languages), the README and
  `reference.md` name the engines' own GitHub releases; Heroic stays a
  fallback.

Checked: `cargo fmt --check`, pedantic clippy with no warning,
`cargo test --workspace` green (232 core with a new `galaxy` test, 129
window). `git grep -i hyprctl crates` finds nothing. On KDE, in a throwaway
home with no session bus: the window opens, `kdotool` shows Zendo's pid, and
`windowclose` with the tray off ends the process.

Tried by the user on KDE the same day, with the real session bus [firm]: the
tray hides and restores the window; Accounts sign-ins for Steam and GOG work;
a GOG game installed through Zendo (GE-Proton installed from Settings too)
and started without trouble, with Galaxy on (`gog_galaxy = true`) and
no Heroic: `~/.local/share/zendo/galaxy/GalaxyCommunication.exe` is there
(59,684 bytes, the size in the zip), so the download and unpack worked.
Quitting from the tray while Steam
was downloading a game showed no question and Zendo simply closed, which is as
built: the question is for Zendo's own queue (`downloads.running`, the
legendary, gogdl and nile installs), and a Steam download belongs to the
Steam client, which kept going. Also tried and working: Quit from the tray,
hidden, with a download of Zendo's own running, and the grids' scroll after a
restore.

## Item 28: the KDE colour scheme as a theme (2026-11-12)

- **`kde.rs`** (zendo-core) replaces `tone.rs`: `Scheme::read` takes the
  background and text from `kdeglobals`' `[Colors:Window]` and the accent
  from `General`'s `AccentColor`, else `[Colors:View]` `DecorationFocus`,
  else `[Colors:Selection]` `BackgroundNormal`. Colours are `r,g,b` (alpha
  ignored); no colours means no scheme. `light` is the background being
  brighter than the text. `ini.rs` is `desktop.rs`'s group reader, moved so
  both share it. `Rgb` moved to `theme.rs`.
- **Window.** `Choice::System` (tag `system`) next to Zendo's own, shown when
  `kdeglobals` exists or when chosen; watched by the same 2 s look as the
  tone was (`LookAtScheme`). Greys are mixed by `Colors::from_theme`; green
  and red stay Zendo's, as plasmonk leaves KDE's at Breeze's defaults
  (decided with the user). A saved `theme = "hyprmonk"` shows Zendo's own,
  without a note.
- Checked against the live `kdeglobals` (PlasmonkA): Window 37,39,47 on
  227,226,230, accent 61,120,198 (no `AccentColor`, which plasmonk deletes).

Checked: `cargo fmt`, clippy with no warning, `cargo test --workspace` green.
Checked on screen on KDE (2026-11-12), in a throwaway home with a copy of the
live `kdeglobals` and no session bus: the window starts on System from
`theme = "system"` (blue-grey, accent 61,120,198) and Settings shows both
chips and "Following the system's colour scheme: PlasmonkA". Editing the
copy's `DecorationFocus` to 220,40,40 repainted the open window red within
the 2 s look; removing `[Colors:Window]`'s `BackgroundNormal` showed Zendo's
own colours and the "no KDE colour scheme" note; restoring the file brought
the scheme back. Clicking the Zendo chip and then System worked and wrote
`theme = "system"`. A saved `theme = "hyprmonk"` is covered by a unit test
only. Not tried by the user on the real desktop (plasmonk's accent changing
under an open Zendo).

## Item 29: engines and Protons downloaded by Zendo (2026-11-12)

Decided with the user the same day: Zendo's own copies are found first
(`PATH`, then Heroic's bundle, after them), with a Settings switch for
`PATH` first; updates are noticed once a session when Settings opens and
applied on a click.

- **`engines.rs` (core).** `Engine` is legendary, gogdl, nile, comet or
  umu-launcher. `latest` asks `releases/latest`; `install` downloads into
  `$XDG_CACHE_HOME/zendo/engines/`, checks the SHA-256 GitHub gives each asset
  (`digest`; an asset without one is refused) with `sha256sum`, builds
  `$XDG_DATA_HOME/zendo/engines/<engine>/` (the program named as it runs, a
  `release` file with the tag) in a hidden `.partial` folder, and swaps it for
  the old one. umu's zipapp tar gives `umu/umu-run`. comet's install also
  replaces `zendo/galaxy/GalaxyCommunication.exe` from the same release's
  `dummy-service.zip` (`galaxy::replace_service`, checked against its digest);
  the on-demand download in `galaxy.rs` stays as the fallback for a comet found
  elsewhere. `find` is the one lookup: `stores::bundled` and `launch.rs`'s
  umu-run go through it, and it reads `engines_on_path_first` from
  `config.toml` on every call. The diagnostics report names whose copy each
  tool is.
- **`ge_proton.rs`.** `Build::{Ge, CachyOs}`. Proton-CachyOS: repo
  `CachyOS/proton-cachyos`, `proton-cachyos-<version>-slr-<variant>.tar.xz`
  with a `.sha512sum`; `x86_64_v3` is chosen when the CPU has avx2, bmi1,
  bmi2, fma, f16c, lzcnt and movbe. A release counts as installed under either
  variant's folder. Automatic is unchanged (`rank`): CachyOS comes after
  GE-Proton and UMU-Proton, and dated folders sort by their numbers.
- **Window.** Settings → Engines (`app/engines.rs`): a row per engine with the
  copy in use and whose, the latest tag, Download/Update, and Remove (asks
  twice). Update and Remove are disabled while a game or a store download runs
  (umu-run is a zipapp that lives as long as the game; not tested whether
  replacing it would break one). Settings → Proton has a GE-Proton /
  Proton-CachyOS chip pair, each build keeping its own list. The download bar
  moved to `settings.rs`, shared by both.

Checked: `cargo fmt`, clippy with no warning, `cargo test` green. Against the
real GitHub on 2026-11-12, in a throwaway `XDG_*` (ignored tests): gogdl
v1.4.0, nile v1.2.0 and umu-launcher 1.4.4 installed, digest-checked, found by
`find` and answered `--version` from Zendo's folder; both Proton lists read
(GE-Proton11-12; proton-cachyos-11.0-20261015-slr-x86_64_v3, with sums).
Checked on screen on KDE (2026-11-12, with the user present), in a throwaway
home with no session bus and `PATH` without the mise shims: Settings → Engines
lists all five as "Not found" with the latest tags; Download on gogdl
installed it ("Zendo's copy", its path, "Up to date", Remove); Remove asked
"Really remove" and then deleted the folder, with its note; the PATH switch
wrote `engines_on_path_first = true`; Check again re-asked. In Proton the
Proton-CachyOS chip loaded its list on demand, and a full download (325.65 MiB,
under 15 s) unpacked to `proton-cachyos-11.0-20261015-slr-x86_64_v3`, showed
"Installed" and became Automatic. Not checked: comet's install and its Galaxy
service in a real GOG start; Update (no older copy to update from); the
disabled state while a game runs. Not tried by the user.
Two wrinkles seen, then fixed: the Automatic label said "newest GE-Proton
first" while naming a CachyOS build, and the Proton blurb spoke of GE-Proton
only.

## Item 30: the Fedora package (2026-11-12)

`packaging/fedora/zendo.spec` and `.copr/Makefile` (COPR's `make_srpm`
entry). The Makefile exports HEAD with `git archive`, runs `cargo vendor
--locked` on it (the chroot build has no network), writes the commit, short
commit and date over the spec's three defaults and runs `rpmbuild -bs`. The
spec is a snapshot version, `0.1.0^<date>git<short>`, builds with
`cargo-rpm-macros` (`%cargo_prep -v vendor`, `%cargo_build`, `%cargo_test`)
and installs the binary, the desktop file and the icon. No engines in the
package (item 29 downloads them); `Requires` lists what Iced loads at run
time and rpm cannot see (`vulkan-loader`, `libxkbcommon`,
`libxkbcommon-x11`, `libwayland-client`, found with `strings` on the binary),
and what Settings → Engines needs (`python3`, `tar`, `xz`, `coreutils`).
Mods and `dummy-service.zip` need `(7zip or bsdtar)`.

Checked on 2026-11-12: the SRPM from HEAD, then `mock -r fedora-45-x86_64`
(no network) built it: 133 core and 247 window tests pass (25 ignored),
`desktop-file-validate` passes. A first mock run failed because mock re-runs
`rpmbuild -bs` on the spec inside the SRPM, where the `--define`s were gone;
the Makefile now bakes the values in. rpmlint: no error that matters
(`explicit-lib-dependency` is wanted, the libraries are dlopened; spelling
errors on tool names). Every `Requires` name exists in Fedora's repos
(`dnf repoquery`). COPR: the project `leandrop/zendo` (fedora-45 and fedora-rawhide, x86_64) was
made with `copr-cli create`; the first build (11316245) failed because
the Makefile ran `git` while parsing, before the `dnf install git` step, in a
source chroot that starts without git. Fixed (the install runs first, then a
second make does the work, 9d89054); the second build (11316285, submitted by
the user with `copr-cli buildscm ... --method make_srpm`) succeeded for both
chroots, 2026-11-12. Installed from COPR by the user (`dnf copr enable`, `dnf install`) and
started in a throwaway home with no session bus: `rpm -V` clean, no missing
library, `desktop-file-validate` passes, the window opened and listed the three
KDE entries, and closed cleanly. Not checked: Fedora 44, the tray from the
package. COPR's homepage and contact
fields are set in the web page; `copr-cli` has no option for them.

## Item 31, the spike: a transparent Iced window on KWin (2026-11-12)

A throwaway crate (Iced 0.14.0, `transparent: true`, `blur: true`, a
`.style()` background of `rgba(23,23,23,a)`), run in a throwaway home with no
session bus; checked with a full-screen `spectacle -b -n -f` still and a
pixel sample against the wallpaper (118,118,118 behind).

- **Alpha 0.85 shows as fully opaque.** The pixel read exactly 23,23,23.
  Iced's log says the Vulkan surface offers `Opaque` and `PreMultiplied`,
  and that it selects format `Rgb10a2Unorm` with `PreMultiplied`
  (`iced_wgpu` 0.14.0 `window/compositor.rs` takes the first non-sRGB
  format, with no setting to change it). That format has 2 bits of alpha:
  four levels (0, 1/3, 2/3, 1), so 0.85 rounds to 1.
- **Alpha 0.6667 works**: the pixel read 86,87,83 and the wallpaper shows
  through. So Zendo's translucency is 2/3 or 1/3 opacity, not a free value.
- **Blur works through Better Blur DX only for listed classes.** On this
  machine the stock Blur effect is off (`blurEnabled=false`) and Better Blur
  DX blurs the classes in `~/.config/kwinrc` `[Effect-better-blur-dx]
  WindowClasses`; `zendo` is not in it, so a window of class `zendo-spike`
  was translucent but sharp. With the class `kate` (already listed) the
  same window was blurred: the wallpaper's detail and the terminal's text
  behind it were unreadable. So `blur: true` is not what blurs under Better
  Blur DX; the class list is, and the stock Blur effect would honour the
  request.
- Decision [tentative]: the default is 2/3 opacity, with the Opaque switch;
  plasmonk's class list gets `zendo` (PLAN.md, "On the desktop's side"). No
  fallback to an opaque window is needed.

## Item 31: the shell, 2026-11-12

Built after the spike above. **Icons** first, as their own commit: the
user's new app icon (601×580) and round tray icon (288×284), padded to a
square on a transparent canvas, are `data/icons/zendo-{16,22,24,32,48,64,
128,256,512}.png` (installed under `hicolor/<n>x<n>/apps/` by the spec and the
PKGBUILD; `data/zendo.svg`, the ensō, is gone) and `crates/zendo/tray-icon.png`
at 64 px. Mock-built on Fedora 45, `rpm -qpl` lists the nine icons.

- **Translucency**: `Config.opaque` (off by default, written only when on),
  `window_settings(size, fullscreen, opaque)` sets `transparent` and `blur`
  unless opaque, and the daemon's `.style(Zendo::style)` paints the palette's
  background at 2/3 alpha (`style::window`, the spike's finding; 0.85 would
  draw opaque). Settings → Theme has the "Opaque window" switch; it saves
  and `reopen_window` opens a new window and closes the old one, since
  transparency is fixed at creation.
- **Type**: JetBrains Mono 2.304 (OFL-1.1) Regular and Bold in
  `crates/zendo/fonts/` with its OFL.txt and AUTHORS.txt, loaded in
  `app::run` and set as the default font; `style::FONT`, `BOLD` is its bold.
  The spec lists `License: MIT AND OFL-1.1` and `%license` for the OFL, the
  PKGBUILD installs it, and About credits it.
- **Tokens and flat controls** in `style.rs`: `SPACE_S/M/L/XL`,
  `RADIUS_CONTROL` (0) and `RADIUS_PANEL` (2); chips, buttons, inputs and
  checkboxes square, the chosen chip outlined in the accent with no fill.
  The uppercase `label` helper and `SPACE_XS` were left out: nothing used
  them yet (Iced 0.14 text has no letter spacing, so it would be uppercase
  only); items 32 and 33 add them where the pages need them.
- **The top bar** (`top_bar.rs`, replacing `sidebar.rs`): ZENDO and the
  accent dot, LIBRARY, STORE and DOWNLOADS tabs with their counts, the search
  (the library's on Library, the Store's on Store, none elsewhere, since
  Enter means Play on a game's page) and a SETTINGS button. Settings,
  Accounts and About share a row of chips under the bar. The bar's
  background is the palette's `sidebar` colour. `library_header` and the
  Store's header lost their search box; the library summary of the sidebar
  went (the subtitle already says games and sources). `sidebar::WIDTH` is
  gone from `columns_for`, `min_width` (now at least `top_bar::MIN_WIDTH`,
  960) and the two game pages; its i18n keys went, and `sidebar-signed-in`
  became `top-bar-signed-in`.

Checked: `cargo fmt`, clippy with no warning, tests green (133 and 247).
On screen on KDE (2026-11-12, with the user present, throwaway home, no
session bus): Library, Store, Downloads, Settings, Accounts and About drawn;
each control clicked once with `ydotool` after a pid check: the three tabs,
the Settings button, the three sub-tabs, search on Library (typing "kmi"
left KMines) and on Store, and the Opaque switch, which wrote and removed
`opaque = true` and reopened the window solid and translucent again
(same pid, new window id). The reopened window came up at a different place
(KWin placed it at 649,137 instead of 596,115), which threw off a first set
of clicks aimed with the old position; the app was fine.

Then tried by the user with `cargo build` (2026-11-12): blur works with
`zendo` in their Better Blur DX class list, the tray shows the new icon, and
the minimum width looks right. The window list and decoration still showed
the old ensō: the installed RPM (git 9d89054) ships
`/usr/share/icons/hicolor/scalable/apps/zendo.svg`, which KDE prefers; the
new package ships PNGs only, so installing it fixes that, and a Wayland app
cannot set its own window icon. About's "Ideas from" card is gone (the
credits belong on GitHub). Not checked: a light KDE colour scheme (the user
leaves it for later) and a COPR build with the font and icons.

## Item 32: the Library and the game page, 2026-11-12

Built from the plan agreed the same day (pins become Favorites, the card's
progress line is the achievements share, the wide art is a faint backdrop,
the Store's game page waits for item 33).

- **Views**: `library::View` (All, Favorites, Recent, Hidden) replaces
  `show_hidden`. Recent is played in the last 30 days; Hidden shows only the
  hidden games; each view has its own empty text. `Message::SetView` is the
  chips; `Message::ShowHidden` (Ctrl+H, Start) toggles Hidden and All.
  The sort chip Recent is labelled "Last played" so it is not mistaken for the
  view. The Library header is three rows: views with Add game and Refresh,
  stores with the sort chips and their Tab hint, and Organize (categories,
  marked games). `style::label` is the small uppercase label item 31 left out.
- **Card** (also the Store's grid): the title in uppercase over a fade at the
  cover's foot, a 3px accent line for the achievements share (`progress`, the
  Store passes none), the mark box top left, FAVORITE or RUNNING top right, and
  under the card the title and `STORE · played line`. Chip labels are uppercase
  everywhere.
- **Achievements cache**: `state.json` gets `achievements` (id to unlocked and
  total), written when a game's achievements arrive with the unlocks known
  (`achievements_arrived`), read into `Game.achievements` by `Library::apply`.
  A game shows its line only after its page was opened once.
- **Game page**: the portrait cover (200×300) with Change cover and Change
  backdrop (`ArtMessage::OpenHero` opens the picker on wide images), the wide
  art at 22 % opacity behind the header, the title, the source line and the
  ProtonDB, Steam Deck and PCGamingWiki chips; an action row that wraps (Play,
  Update, Favorite, Launch options, Hide, Manage in Steam, More); Organize
  (the game's categories and Edit categories); four stat tiles (play time, last
  played, achievements, completion); then the two columns as before. Hide moved
  out of More; More shows only when it has something. `hero`, `Hero` and
  `shade` moved to `store_game.rs` unchanged for item 33.
- **Links**: PCGamingWiki by Steam app id (`api/appid.php?appid=`, documented in
  its `PCGamingWiki/api` repository; curl gets a Cloudflare challenge, so not
  fetched here) else a search by title; Manage in Steam is
  `steam://nav/games/details/<app>` (Valve's community wiki lists it and warns
  some commands no longer work).
- Deferred: design.md's insights block to item 36; Manage in Heroic to item 33
  (no verified URI).

Checked: `cargo fmt`, clippy with no warning, tests green (136 and 248),
including each view, the achievements cache and the PCGamingWiki URLs.

On screen on KDE (2026-11-12, with the user present, throwaway home, no
session bus, `ydotoold` on `/run/user/1000/ydt.sock`, a pid check before every
click and key): the home held KPat's and KMahjongg's real covers copied from
the cache, KPat's wide art, a favorite (KPat), a hidden game (KMines) and
`achievements` `[4, 15]` for KPat. Clicked once each: the four view chips (the
counts and the games were right), a source chip, a sort chip, the Categories
chip (the sheet opened, Escape closed it), a card's mark box (twice: "1
marked", Add to category and Clear appeared, then went), a card, and on the
game page Back, Change cover (the picker opened on Cover), Change backdrop,
Launch options, Edit categories, Unfavorite and Favorite, Hide and Unhide,
and PCGamingWiki (Firefox opened; its window was minimised afterwards). The
progress line, the FAVORITE badge, the four tiles (2 h 5 min, today, 4 / 15,
27 %) all showed. In Portuguese at 960 px wide the Library and the game page
fit with nothing outside the window.
- **Backdrop seam**: the first build faded the wide art with a gradient, and
  over the translucent window it showed a hard line (a light band above, flat
  dark below): item 31's two bits of alpha again. The gradient is gone; the art
  is a constant 22 % and showed as a flat faint block with no seam.
- The status line said "Unpinned"; the status and key-hint strings now say
  favorite (en and pt-PT).
- Dim and accent text on the translucent window is hard to read over this
  machine's mid-grey wallpaper (the window's colour is about 86,86,86 there); it
  read clearly over a dark window behind. Not changed.
- Not checked: Change backdrop opening the picker on Wide image (the click
  worked, no still was taken), Manage in Steam (no Steam game in the throwaway
  home, and the URI is not tried on the real Steam), the Opaque switch for this
  page, Edit categories' sheet (opened and closed, no still).

## Item 33: the remaining pages, 2026-11-12

Planned with the user the same day (Settings grouped under chips; page titles
dropped; mods stay a card). Built in this order:

- **Shared pieces** (`style.rs`): `BUTTON_SMALL`, `BUTTON`, `BUTTON_LARGE`
  paddings, `button_text`, `subheading`, `sheet_title`, and flat `pick_list`,
  `menu` and `toggler` styles (iced_widget 0.14.2's `Style` types, read in the
  cargo registry). `game::card` and `settings::section` share one heading
  (uppercase, bold, 14); `about.rs`'s own copy is gone and the intro sits in a
  card. `launch_options::frame` is the sheet layout (launch options,
  Categories, the Nexus chooser).
- **Store game page**: `game::header(Header { .. })` is the one header; the
  installed game's page and `owned_page` both call it. `hero`, `Hero`, `shade`,
  `fact_chip` and their constants went, and so did `game-change-art`.
  `owned_page` has the uppercase trail and a play-time tile when the store
  counted time.
- **Store**: the title is gone. Row one holds the shop chips and "Not installed
  only" with Refresh at the right; row two the sort chips (`Sort · Tab`) and,
  while asking, the stores being asked. Errors sit in a card. The grid reads its
  height from the scrollable's own bounds (`app.rs`, `viewport_height`), so the
  header's height does not matter to its row maths.
- **Downloads**: three stat tiles (up next, paused, finished) replace the title;
  the section labels are `style::label` and ordinary case in en and pt-PT.
  `downloads-nothing`, `downloads-in-queue*`, `store-owned`, `store-installed`,
  `library-sort` and `settings-subtitle` went (the i18n test lists unused keys).
- **Accounts**: no title; `accounts-about` is in a card; the store name is
  uppercase 16.
- **Settings**: `settings::Group` (General, Keys, Library, Engines, Proton,
  Diagnostics) with `Message::SettingsGroup`, not saved. Typing or pasting on
  Settings starts the key only under Keys; `ArtMessage::Settings` opens Keys.
  Test: `typing_on_settings_starts_the_key_only_under_keys`.
- A token pass over the other pages' buttons, paddings and spacings (SPACE_*),
  and the sub-headings (Proton, Plugins, Options) as `style::subheading`.
- **Manage in Heroic: not taken.** Heroic's `protocol.ts` (blob b4fa1c1) knows
  only `ping` and `launch` (docs/reference.md).

Checked: `cargo fmt`, clippy with no warning, tests green (137 and 248).

On screen on KDE (2026-11-12, with the user present, throwaway home with the
store cache and covers copied in, no session bus, `ydotoold` on
`/run/user/1000/ydt.sock`, a guarded click helper that clicks only when
Zendo's window is both under the pointer and active): drawn and clicked once
each: the Library, Store, Downloads and Settings tabs; the six Settings group
chips (General, Keys, Library, Engines, Proton, Diagnostics) and the
Accounts and About sub-tabs; a game page (KPatience: shared header, action
row, Organize, four tiles); the Launch options sheet (the frame, flat
switches, the flat pick list, Save and Cancel, which closed it); the
Categories sheet (the frame, Cancel, which closed it). In pt-PT at 960 px wide
the Store and Settings fit with nothing outside the window.

Seen and left:
- Buttons whose text was built from data stayed mixed case (Engines' "Download
  0.21.1", Anti-Cheat's "Download", "Copy report", the key fields' "Show" and
  "Check") and "GE-Proton releases" was a size-15 heading. Fixed afterwards:
  every button takes `style::button_text` (uppercase, 13; Play, Install and
  Open in library keep 16), and the heading is `style::subheading`. The
  language and download-worker chips keep their names as written. Built, with
  clippy clean and tests green; not looked at on screen again.
- Dim text straight on the translucent window (the "Sort · Tab" label, the
  hint line, "Ordenar") is hard to read over this wallpaper, as in item 32.
- The Store's cached list does not show without a sign-in, so the owned grid
  and a store game's page are **not checked on screen**. Also not checked:
  Downloads with jobs (the stat tiles), a signed-in Accounts page, the Add
  game, installer and art picker sheets, the mods card, Nexus search and its
  chooser, and a click on every pick list's open menu.

## Item 34: Game Mode, 2026-11-12

Planned with the user as Home plus a lean Game Mode. Home (a first tab and the
start page, with Jump back in, Continue playing, an up-next list, suggestions
and quick access) was built and checked on screen, then **removed the same day
at the user's wish**: it made no sense to them, and Game Mode does not need it.
The app keeps Library, Store and Downloads.

- **Queue** (`zendo-core/src/state.rs`): `State.up_next`, `queue_next`,
  `unqueue`, `UP_NEXT_MAX` 100; tests for order, no duplicates, the cap and an
  older file without the field. The window keeps its copy (`Zendo.up_next`, read
  with the scan in `Scanned`) and writes through `Zendo::save`. `app/queue.rs`
  holds `QueueMessage` (Play, Details, ToggleQueued), `played_games`,
  `queued_games` and `meta_line`, with tests. The game page has "+ Up next".
- **Game Mode** (`app/game_mode.rs`): `Zendo.game_mode: Option<GameMode>`
  (shelf, index, menu, `browsing`, the page it came from) beside `fullscreen`;
  `ToggleFullscreen` became `ToggleGameMode`. The screen shows on the Library
  page while Game Mode is on and `browsing` is false; `game_mode_takes` hears
  Left/Right, Enter, Esc, Tab and Space first. `gamepad.rs` gained `PrevShelf`
  and `NextShelf` (LB, RB); Select toggles Game Mode; in Game Mode Y queues and
  Start opens the menu. `--game-mode` is new and `--fullscreen` starts it too.
  The top bar has a GAME MODE chip while it is on.
- Tests: toggling and returning to the page left, the chip returning to the
  screen, the menu's Library choice, moving along a shelf and stopping at the
  ends, shelf cycling past an empty one, nothing played showing every game, the
  controller mapping in and out of Game Mode, the queue, and the start page.

Checked: `cargo fmt`, clippy with no warning, tests green (150 and 250).
On screen on KDE (2026-11-12, with the user present, throwaway home with a
seeded `state.json`; `ydotoold`, pid check before every key and click): F11
went full screen into Game Mode (2560x1080) with the hero and the carousel at
the foot; Right moved to the next game, Tab changed the shelf, Esc opened the
menu and Down, Enter chose an entry; the menu's Library showed the grid with
the GAME MODE chip, a click on the chip returned to the screen, and F11 left
(back to the windowed 1368x892 page it was left from). While Home existed the
top bar overflowed in pt-PT at 960 px; with Home gone the bar, its 280 px search
and its 960 px minimum are as before item 34.
Not checked: a real controller (no device here; the mapping is unit tested
only), PLAY from Game Mode (it would start a real game), the hero's description
for a store game, and the game page's "+ Up next" button.

## Item 35: Organize, 2026-11-12

Planned with the user the same day: tags are the categories, saved filters
hold everything on screen, no random pick, bulk edits are Favorite and Hide.

- **Core** (`state.rs`, `lib.rs`): `Status`, `View` (moved from the window,
  with `RECENT_DAYS`) and `Filter` with serde; `State` gets `status` and
  `filters` (both `#[serde(default)]`, so an older `state.json` reads);
  `set_status`, `status_of`, `filters`, `save_filter` (an existing name in
  any case is overwritten and keeps its spelling), `rename_filter`,
  `delete_filter`. `category_key` and the filters share `key_in`. A category
  rename moves the name inside the saved filters and a delete clears it.
  `Source` derives `Deserialize`; `Game.status` is set by `Library::apply`.
- **Command**: `zendo status <id> <backlog|playing|completed|abandoned|none>`.
- **Window**: `Zendo.play_status` (not `status`, the status line) and
  `filters`; `Message::SetStatus`, `SetGameStatus`, `ApplyFilter`, `Filters`,
  `BulkPinned`, `BulkHidden`; the scan result gained the saved filters
  (`Scanned` is a six-tuple). New `app/filters.rs`, the Saved filters sheet
  modelled on the Categories sheet (name field, Save, Apply, Rename, a delete
  that asks once). Library: status chips, filter chips, FILTERS, the four bulk
  chips and the status on a card's line; game page: the STATUS row.
  `format::view` and `format::play_status`. Strings in en and pt-PT.
- A saved filter's store with no games, or its deleted category, is applied
  as none.

Checked: `cargo fmt --check`, clippy with no warning, tests green (156 in the
window, 256 in the core). New tests: status and filter round trips and an
older file, saving over a name in another case, rename with a clash, delete,
a category rename and delete reaching the filters (core); the status filter
with the store and the search, SetStatus pressed twice, ClearFilters, a game's
status set and cleared, ApplyFilter restoring all six fields and reading as
chosen, a filter naming a deleted category, the sheet's save, update, rename
and two-press delete, and bulk favorite and hide over a mixed selection
(window). `zendo status` by hand in a throwaway home: set, change, `none`, a
bad word and an unknown id (the last two refuse, `state.json` read with `jq`).

On screen on KDE (2026-11-12, with the user present, throwaway home with eight
`.desktop` games, no session bus, `ydotoold` on `/run/user/1000/ydt.sock`, a
click helper that fires only when Zendo is the active window and the one under
the pointer). Clicked once each: a status chip (BACKLOG: two games), a saved
filter chip (put category and status back, its chip lit), FILTERS (the sheet
opened), Save with a typed name, Rename (field, OK), Delete (first press asked,
second removed), Cancel, the ALL chip, a card's mark box, the bulk chips
FAVORITE, UNFAVORITE, HIDE and UNHIDE (each only when it would change a marked
game; the status line counted; hiding cleared the marks, favoriting kept them),
a card, and on the game page a status chip twice (set, then cleared, as
`state.json` showed). In pt-PT at 960 px wide the Organize row wrapped to two
lines with nothing outside the window.
- The grid moves down when the Organize row wraps onto a second line (bulk
  chips appearing), so a click aimed from an earlier still can miss.
- A card's line with a status can be three lines in pt-PT (`AMBIENTE DE
  TRABALHO · ABANDONADO · Ainda não jogado`) and the caption's fixed height
  clipped the last word. Fixed the same day: `CAPTION_HEIGHT` 60 to 84 (a
  two-line title and a three-line detail line; cards are 24 px taller, and the
  row maths and tests read the constant). Looked at in pt-PT at 960 px: Bomber's
  three lines show whole. Tests, clippy and fmt unchanged.
- The first Zendo process had gone when I went to stop it, after the last
  click (the window was alive in the last still); its log was empty and there
  was no coredump, so the cause is not known.

Not checked: the Saved filters sheet in pt-PT, a rename or delete of a
category reaching a saved filter on screen (covered by tests only), and the
`zendo status` verb against the running window.

## Item 36: play sessions and stats, 2026-11-13

Planned with the user the same day: Stats is a fourth tab, the yearly summary
a section of its page (no command).

- **Core**: new `sessions.rs`. `Session` (id, title, start, minutes, local
  offset), `record` appends one JSON line to `$XDG_STATE_HOME/zendo/sessions.jsonl`
  under `paths::locked`, with a newline first when the last line was cut short;
  `load` skips a line that does not read. `local_offset` runs `date -d @T +%z`
  (`unsafe_code` is forbidden, there is no date crate); `Summary::of(sessions,
  Period)` (minutes, sessions, games, active days, longest, top ten, by month,
  by weekday Monday first, games started, longest streak), `years`, `of_game`.
  `time.rs` gained `weekday_of` and made `civil_from_days` and
  `days_from_civil` crate-visible.
- **Recording**: `RunMessage::Ended` (`running.rs`) keeps a session for every
  game that was seen running and lasted a minute or more, in `Zendo.sessions`
  and, outside tests, in the file. The list is read at boot.
- **Window**: `Page::Stats`, the tab, Ctrl+7, `app/stats.rs` (chips, tiles,
  bars, column charts, highlights, library by status, the empty text); Left,
  Right, Home and End change the period on that page only; the game page's
  Sessions card; month and weekday names, `stats-*` strings in en and pt-PT.

Checked: `cargo fmt --check`, clippy with no warning, tests green (162 in the
window, 266 in the core). New tests: sessions append and read, a cut-short line
skipped and the next kept, `parse_offset`, a year against all, the local day
and year across midnight and New Year, the streak over a month end, the top
list and its cap, `years`, `of_game`, the weekday of known dates; a watched
game that ran two minutes is one session, one under a minute and one never seen
are none; Ctrl+7 and the arrows changing the period on Stats only.

On screen on KDE (2026-11-13, with the user present, throwaway home in pt-PT
with nine `.desktop` games, no session bus, `ydotoold` on
`/run/user/1000/ydt.sock`, a pid check before every key and click). Seeded
`sessions.jsonl` with 13 sessions over 2025 and 2026, one line that is not
JSON and a last line cut short. Looked at: the empty page, Ctrl+7, the year
chips by key (Right to 2025 and ALL TIME, stopping at the last, Left back) and
by click (2025), the tiles, Most played, By month, By weekday, Highlights and
Library by status (every total checked by hand: 2026 is 9 sessions and 12 h
55 min, 2025 is 4 and 3 h 50 min, all time 13 and 16 h 45 min), and the game
page's Sessions card (KPat: 5, 1 h 24 min average, 2 h 30 min longest, latest
first). Then a real game: a `.desktop` file running `sleep 70`, found with
F5, opened and played with Enter; its session was written as
`{"id":"desktop:sleeper","title":"Sleeper","minutes":1,...}` on its own line
after the cut-short one, and Stats showed it (10 sessions, 12 h 56 min) with no
restart.
Found and fixed in the check:
- At 960 px in pt-PT the fourth tab pushed the Settings button off the bar
  (DEFINIÇÕES clipped). `top_bar::MIN_WIDTH` is now 1040, so the window cannot
  be narrower; looked at again at 1040 with nothing clipped.
- The empty page's text ran edge to edge; it is now a 520 px centred block.
- The month and weekday labels of the tallest and of empty columns sat 4 px
  higher than the others. Each bar now stands at the foot of an area as tall
  for every column. A value under an hour read "0" under an "hours" heading;
  values are now `55m` or `3h` and the headings say only By month and By
  weekday.
Checked afterwards the same day (English, 1040 px wide, same throwaway home,
pid check before every key): the Stats page (2026: 10 sessions, 12 h 56 min),
and Ctrl+2, Ctrl+3 and Ctrl+5 for the Store, Downloads and Settings pages, with
all four tabs in the bar and nothing clipped. A game a launcher runs: a test
drives the folder-watch path (Seen, then Ended after ten minutes) and finds
one session and no minutes added to Zendo's own total; a real Steam or Heroic
was not run, as neither is in the throwaway home. A Zendo of the user's own
(their real home) was running during the first check and was not touched.
Known limit, unchanged from the totals: quit Zendo while a game runs and that
session is lost.

## Item 38: Steam's facts for every game, 2026-11-13

Built, tests green (166 in the window, 271 in core), clippy clean; checked on
screen the same day with the user present (below), and tried by the user
afterwards: the Technical button's scroll, the Features row, and the
screenshot click (it opened the full image in the user's browser, Zen) all
work [firm].

Checked on screen (throwaway home, no session bus, two `.desktop` games:
"Portal 2" and "Zzyzx Unmatched Test", the user present, a pid check before
every key and click): Portal 2's page found app 620 by title (the ProtonDB
Native and Deck Verified pills came with it) and showed the About card from
Steam, Developer/Publisher/Released/Genres, Features (the controller line not
repeated), Metacritic 95 / 100, a sideways-scrolling Screenshots card and the
System requirements card. The Technical card's row read "620 (found by
Zendo)". Typing 400 and Save changed the page to Portal (released Oct 10,
2007, Metacritic 90, other screenshots and requirements) with "400 (set by
you)" and a Clear button; Clear brought Portal 2 back. The unmatched game had
no pills, the "No Steam game was found" card in the About place, and its Set
Steam app id button opened the Technical card. After a restart, Game Mode's
hero showed Portal 2's Steam description from the cache, and "No description
yet." for the unmatched game. The Metacritic link opened Firefox on Portal 2's
reviews page.
Found in that check and fixed the same day, then looked at again: the Set
Steam app id button opened the Technical card below the visible part of the
page, so it now snaps the page to its end (the card is the last); the
Features row listed every Steam category (about 30 for Portal 2), so it now
names the first 6 and counts the rest ("+26 more"). The About text is
Steam's `about_the_game`, marketing included, left as it is.
Not checked: a screenshot's click (the second `xdg-open` made Firefox say it
was already running; it is the same `Message::Open` path as Metacritic's, so
this is Firefox's start-up and not Zendo's, but the full-size image was not
seen), pt-PT, the failed-facts row, a hero for a game never opened that
ProtonDB knows, and the Store page's Steam details through the shared cache.

- `owned.rs` reads, besides what it did, Steam's `categories` (features),
  `controller_support`, `metacritic` and `screenshots` from `appdetails`.
  Real replies were checked first (2026-11-13): an unknown or removed app
  answers `{"success":false}` with no `data`; `metacritic` is absent for
  Palworld and Black Myth: Wukong; `controller_support` can be absent;
  `screenshots` items have `path_thumbnail` and `path_full`.
  `StoreDetails` is now `Serialize` and `Deserialize`.
- `steam_facts.rs` (core): `details(app)` is cached in
  `$XDG_CACHE_HOME/zendo/steam_facts.json` for a week (the Store page's
  Steam details go through it too); `for_app(app)` adds the first 8
  screenshot thumbnails as files in `.../zendo/screenshots/`; `app_for(id,
  chosen)` finds the app id with no network: the user's, a Steam game's own,
  else the one the ProtonDB cache holds. It skips umu's id, which can touch
  the network.
- `http::cached_image` is achievements' icon downloader, moved so both use
  it.
- A Steam app id the user gives a game is in `state.json` (`steam_apps`),
  never for a `steam:` id, and on `Game.steam_app`. `protondb::rating` takes
  it first and records it in its cache entry (`chosen`; version 3), so
  setting, changing or clearing it asks again. The requirements cache key
  holds the resolved app id. AreWeAntiCheatYet keeps no per-game cache and
  needed nothing.
- The game page: the About card falls back to Steam's description; the
  details card gains Developer, Publisher, Released and Genres only where
  the game's own store left them out, plus Features (with controller
  support) and Metacritic (opens its page); a Screenshots card scrolls
  sideways and each opens its full size in the browser. A game with no Steam
  match gets a card saying so, which opens the Technical card, whose Steam
  app id row shows the id in use, where it came from, and a field with Save
  and Clear.
- Game Mode's hero shows the description once the game's app id is known
  without the network (a Steam game, a user's id, or an earlier page open or
  ProtonDB lookup). Carousel browsing never triggers a lookup for games not
  yet known.
- What was and was not checked on screen is above.

## Items 41 to 44: the user's notes of 2026-11-13

Planned from ten notes; 41 to 44 built and 45 (the Slint spike) not yet.
Checked together on screen in a throwaway home (24 `.desktop` games with
covers and heroes copied from the real cache, an old `state.json` holding a
category and a saved filter of the same name, an `up_next` list and two
favourites), with the user at the PC: `spectacle` stills and `ydotool`
clicks guarded by the window's pid.

### Item 41: icons, top bar and cards

- Iced's `svg` feature is on. Lucide icons (ISC, `crates/zendo/icons/` with
  its `LICENSE`) are built in; `app/icons.rs` draws one in a given colour.
  Only the four in use are kept (download, settings, plus, refresh-cw); add
  one by putting its SVG there and a variant in `Icon`.
- Downloads and Settings are square icon buttons on the right of the top bar
  with a tooltip each (they use the `page-downloads` and `page-settings`
  strings); Downloads shows its count beside the icon when there is one. The
  tabs are Library, Store, Stats.
- All, Favorites, Recent (and Hidden) are in the top bar beside the search,
  on the Library page only. Add game and Refresh moved to the right of the
  store/sort row, with icons. `MIN_WIDTH` 1040 to 1100 and the search box
  280 to 220 px to fit; not tried at the minimum width.
- A card's title is drawn once, under the cover; the overlay and its
  `cover_fade` style are gone. A cover-less card still shows its title in the
  placeholder.
- Checked: every new control clicked once (Favorites chip filters to the two
  favourites; Downloads and Settings open their pages, highlight, show the
  tooltip; Library tab returns). Not checked: Recent, Add game and Refresh
  clicked; a Downloads count; the minimum width.
- The store row's "All" and the top bar's "All 24" look alike but are not the
  same chip (the store row's clears the store, status and collection filters).

### Item 42: Up Next removed

- Game page button, Game Mode's shelf, hero button, Space and Y, `up_next` in
  `zendo-core` (`UP_NEXT_MAX`, `queue_next`, `unqueue`), the `queue-*` and
  `shelf-up-next` strings and their tests. `app/queue.rs` became `app/hero.rs`
  (the played-games list, the meta line and the hero's Play and Details,
  `HeroMessage`). `queue-details` became `game-mode-details`. An old
  `state.json` with `up_next` loads and the field is gone at the next save
  (tested, and seen in the throwaway state file).
- Checked: F11 shows Continue playing and All games, Play and Details only.
  The footer hint still said "Space / Y up next"; fixed afterwards (en and
  pt-PT) and not looked at again.

### Item 43: Collections

- Not the planned migration to one `collections` field: categories and saved
  filters stay as two maps in `state.json`, but are one list to the user and
  share one set of names (`State::separate_names`, and `add_category`,
  `save_filter` and both renames refuse a name the other kind has). A file
  with a clash is read with the filter renamed "Name (filter)", then
  "(filter 2)" and on; none is lost (tested).
- `app/categories.rs` is `app/collections.rs`; `filters.rs` is gone. One
  sheet lists both by name: hand-picked ones with a count, ticked per game
  when opened from a game or the marked games; saved views with their
  summary and an Open button. CREATE makes a hand-picked one, SAVE VIEW keeps
  the screen. The Organize row shows both kinds in one list by name, then one
  COLLECTIONS chip with the total. Strings say collection / coleção.
- `zendo categorize` refuses a name a saved view has instead of reporting
  success.
- Checked: the migrated "co-op (filter)" beside "Co-op", the sheet, and
  SAVE VIEW ("Saved view Mine", `state.json` rewritten without `up_next`).
  Not checked: Create, Rename, Delete and Open clicked on screen (tests cover
  them), the tick boxes from a game's page, pt-PT.

### Item 44: faster images

- Measured first, with a throwaway benchmark on the user's 1,186 cached
  images (read only): covers are small (467 of 598 are 300 by 450, 99 are 342
  by 482; Steam's own `library_600x900.jpg` files are 300 by 450 too), heroes
  are large (484 of 596 are 1920 by 620, then 2560, 3840 and one 7184 by 3512;
  median 299 KB, largest 5.9 MB). Decoding with the `image` crate:

  | | cover | hero (mean) | hero (worst) |
  |---|---|---|---|
  | release | 1.8 ms | 11 ms | 145 ms |
  | debug, as built | 52 ms | 465 ms | 5.6 s |
  | debug, dependencies at `opt-level = 2` | 1.8 ms | 12 ms | 153 ms |

  Iced decodes on the thread that draws, so a screen of covers cost over a
  second in the debug build the user runs and a game page a hero decode.
- Main fix: `[profile.dev.package."*"] opt-level = 2` in the workspace
  `Cargo.toml` (about 30 times faster; the first rebuild compiles every
  dependency again). Release is unchanged.
- Hero copies: `zendo_core::thumbs` makes a 960-pixel-wide copy (JPEG at 85,
  or PNG when it has transparency; the source itself when it is already that
  small) in `$XDG_CACHE_HOME/zendo/thumbs/`, named from the source's path,
  size and modification time. `app/backdrops.rs` asks for the page on screen
  after every message, on a thread, and the header draws the copy when it is
  ready, so no big file is decoded on the thread that draws. A decode of the
  copy takes 0.6 ms in the benchmark (at 640 wide; 960 was not timed).
  Covers get no copy: they would save nothing.
- Not built from the plan: an in-memory cache of the copies' bytes (not
  needed once the decode is cheap) and clearing `thumbs/` (nothing clears
  `covers/` either; the README says it is safe to delete).
- Checked: opening a game page made one 960 by 310 copy (54 KB from a
  226 KB source) and the backdrop showed. Not measured: page-open time in the
  window itself (only the decode); the library's scroll speed after the
  profile change; a PNG hero.

## Item 45, the Slint spike (2026-11-14)

A throwaway crate (Slint 1.18.1, winit backend, FemtoVG on OpenGL) outside the
repo, with `zendo-core` as a path dependency: the Library grid with real
covers, caption lines, the four Lucide SVGs and the KDE scheme. It lives in the
session scratchpad (`slint-spike/`), builds into `~/.cache/slint-spike-target`
(the scratchpad is tmpfs and a 5 GB `target/` ran the linker out of space:
"ld terminated with signal 7 [Bus error]"), and was run only in a throwaway
home (24 seeded `.desktop` games, a read-only copy of `kdeglobals`, no session
bus). Nothing in the repo changed.

| Point | Verdict | Evidence |
|---|---|---|
| Translucency | **Yes, with any alpha** | A blank pixel read 60,61,68 at alpha 0.85, over wallpaper 190,189,188 and the opaque value 37,39,47 (0.85·37+0.15·190 = 60). Iced's `Rgb10a2` surface has four alpha levels and showed 0.85 as opaque (item 31), so Zendo's 2/3 or 1/3 is not a limit here. The window is transparent when its `background` has alpha. |
| Blur | **Yes, the same way as Iced: by window class** | Under Better Blur DX a window with class `zendo` blurred the opaque window behind it (smeared, unreadable) and one with class `slint-spike` stayed sharp, in the same crop of a full-screen still (16,991 differing pixels). `slint::set_xdg_app_id("zendo")` sets the class, and needs `BackendSelector::new().select()` first (it panics with "No default Slint platform was selected" otherwise). `with_blur(true)` and `set_blur(true)` (feature `unstable-winit-030`) change nothing here: winit 0.30.13's Wayland `set_blur` only speaks `org_kde_kwin_blur_manager`, which this KWin does not advertise (it offers `ext_background_effect_manager_v1`); a `WAYLAND_DEBUG` trace shows Zendo (Iced) sends no blur request either. |
| Fonts | **Yes** | `import "../fonts/JetBrainsMono-Regular.ttf";` in the `.slint` file embeds the font; monospace drew. Without it the family is not installed and Slint fell back to a sans. |
| KDE colours | **Not built in; Zendo's code carries over** | `zendo_core::kde::Scheme` and `Theme::from_scheme` fed a `Colors` global: scheme "PlasmonkA", accent 61,120,198 applied; raised and dim mixed in Rust. Slint's own styles (cosmic, fluent, material, qt) were not tried: the spike draws its own widgets. A light scheme was not tried. |
| Scrolling | **Works; the grid is built by hand** | `ListView` is vertical and virtual only, so the Rust side regroups the cards into rows when the column count (computed in `.slint` from the width) changes: 6 columns at 1376 px, 4 at 900 px. On 2,700 cards (the library ×100) auto-scrolled end to end with `SLINT_DEBUG_PERFORMANCE=refresh_full_speed,console` on a 75 Hz screen: 37 to 68 fps in release, 35 to 61 in debug. RSS 243 MB when the repeats share one decoded `Image`; it grew from 0.9 to 3.5 GB in nine seconds when each card loaded its own copy, so cards must share handles or use the thumbnails. No Iced number was taken to compare with. Wheel and drag were not tried (the scroll was driven by a timer). |
| SVG | **Yes** | `Image` with `colorize` drew `plus`, `refresh-cw`, `download` and `settings` at 18 px in the foreground colour. |
| Size | **Not a measured saving** | Release (strip, thin LTO, same profile as the workspace): the spike 26,156,112 bytes in 2 min 28 s cold; Zendo from `main` (`cargo build --release --locked -p zendo`, same machine) 41,707,576 bytes in 4 min 28 s, with other builds running. Normal-edge crates: `zendo-core` 247, Zendo 585, the spike 570. But the spike has no sign-in web view (wry, WebKitGTK), tray or Discord, so the two are not like for like: the Iced-versus-Slint difference cannot be read from them. The installed RPM's `/usr/bin/zendo` (28,319,736, built 2026-11-12) is an older build. |
| Keyboard and controller | **Yes** | Arrow keys through a `FocusScope` moved a cursor (accent border) over the 6-column grid, wrapped by rows, kept its row on screen and Enter activated it; sent inside the process with `dispatch_event`, so the real key path through KWin was not exercised. gilrs on its own thread (Zendo's `gamepad.rs` logic: D-pad, left stick with the 0.6/0.3 lean, Cross) handed moves to the window with `Weak::upgrade_in_event_loop`; on the user's real DualSense ("PS5 Controller") D-pad, stick and Cross moved and activated the cursor. The first 40 seconds produced no events only because nobody pressed anything. |
| Licence | **The user's call** | Slint is offered under Royalty-free, GPLv3 or commercial. Its FAQ (`FAQ.md`, read 2026-11-14): your own files may stay MIT whichever you choose. **GPLv3**: the combined product is GPLv3 as a whole, on any platform (MIT is GPL-compatible). **Royalty-free**: free for desktop, mobile and web, not embedded; one of (a) the `AboutSlint` widget in an About screen reachable from the top-level menu, or (b) the "Made with Slint" badge on a public web page (where the binaries are, if possible); the Slint licence notices stay; an MIT fork of the app may be redistributed under MIT, and a fork using Slint takes its own Slint licence. **Not confirmed**: whether Fedora allows `LicenseRef-Slint-Royalty-free-2.0` (a search found no Fedora page on it, only that Fedora's licensing rules apply to COPR repositories, with some flexibility; those pages are old and I did not read them in full). GPLv3 is on Fedora's allowed list, from general knowledge, not checked here. |

- **Not tried**: a click on a card (only hover, by the real pointer), keys
  from the real keyboard (only synthetic, inside the process), Game Mode's
  shelves and hero, text input, the Skia renderer, the game page and its hero,
  a light scheme, and a rendering comparison with Iced.
- **What carries over unchanged**: `zendo-core` (library, covers, thumbs, KDE
  scheme, themes) and the icons and fonts. What would be rewritten: every
  `app/*.rs` view, in `.slint`, with models and callbacks in between.
- **The checks' side effects**: KWin's stock Blur effect was loaded and
  unloaded six times through D-Bus (the user agreed to it) while I looked for
  blur; `blurEnabled=false` in `kwinrc` before and after, and `isEffectLoaded`
  was false at the end. It was never what blurs here (see item 31); I should
  have read item 31 first. By mistake I moved the invisible
  `xwaylandvideobridge` helper window to 900,200 with `kdotool windowmove`
  (`kdotool search --pid` is not a filter; match `getwindowpid` instead). Its
  old position is unknown and I did not look at it afterwards.
- **Code size**: the spike is 170 lines of `.slint` and 230 of Rust (about 60 of
  them hooks for the checks: blur, class, scroll, alpha, synthetic keys) for
  covers, captions, a progress line, the top bar, the row regrouping, the
  cursor and the controller thread. `app/library.rs`'s
  card and grid code (lines 363 to 668) is about as long but also does
  selection, marking, badges, fading and virtual rows by hand, and leans on
  `style.rs` (671 lines) for hover and borders, which the spike hard-codes. So
  the sizes are the same order; this is not a measured saving either way.
- **Left open**: whether Fedora and COPR accept the Royalty-free licence (see
  its row); a like-for-like Iced and Slint comparison of build time and size
  (it needs the same screens in both).

## Item 46, the Slint licence and `zendo-shell` (2026-11-14)

The user chose a full port to Slint after item 45, first under Slint's
Royalty-free licence, with the plan to check Fedora and COPR first.

- **The licence check.** COPR's terms (`docs.copr.fedorainfracloud.org`,
  "What I can build in Copr?", read 2026-11-14) say a user must not upload
  material "governed in whole or in part by a license not contained in the
  list of acceptable licenses for Fedora". Neither
  `docs.fedoraproject.org/en-US/legal/allowed-licenses/` nor
  `.../not-allowed-licenses/` names Slint (0 matches in each); GPL-3.0-only is on
  the allowed list (9 matches). `packaging/fedora/zendo.spec` uploads a
  vendored source tarball, so Slint's source would be in the SRPM. Not found:
  any Fedora statement about `LicenseRef-Slint-Royalty-free-2.0`, so not being
  listed is not a recorded rejection, only not allowed by the list's wording.
  `dnf repoquery` finds no `slint` package in the enabled repositories. Slint's
  FAQ (`FAQ.md`) says the combined product under GPLv3 is GPLv3 as a whole and
  MIT files stay MIT. **Decision (the user, shown the COPR text): Slint under
  GPLv3.** The spec's `License:` (now `MIT AND OFL-1.1`) gains
  `AND GPL-3.0-only` at the cut-over (item 54); no `AboutSlint` widget is
  required.
- **`crates/zendo-shell`** (new library, workspace member, `zendo-shell` in
  `[workspace.dependencies]`): `i18n.rs` with `i18n/en.ftl` and `i18n/pt-PT.ftl`,
  `links.rs`, `presence.rs`, `file_dialog.rs`, `sign_in_window.rs`, `tray.rs`
  (with `tray-icon.png`) and `gamepad.rs`, moved with `git mv` out of
  `crates/zendo`, plus `APP_ID`. What used to be an Iced stream is now a function
  that takes a closure and spawns its thread: `links::listen(sink)`,
  `gamepad::events(sink)`, `tray::events(sink)`, where `sink` returns `false`
  once nobody wants the events and the thread then ends. The `t!` macro is
  `#[macro_export]` and expands through `$crate::fluent_bundle`, so a crate that
  calls it needs no Fluent dependency. `crates/zendo` re-exports the modules at
  its root (`pub(crate) use zendo_shell::{...}`, so `crate::i18n` and the rest
  still resolve) and adapts the three sources with one function,
  `forwarded` in `app.rs`, which turns such a closure into an Iced stream.
  `crates/zendo` no longer depends on `discord-rich-presence`, `fluent-bundle`,
  `gilrs`, `gtk`, `sys-locale`, `tray-icon`, `unic-langid`, `webkit2gtk` or `wry`.
- **Tests.** The count of `#[test]` is 170 before and after, plus one new, for
  `forwarded` (a source's events arrive in order and its thread ends when the
  stream is dropped): `cargo test --workspace` gives 160 in `zendo`, 276 in
  `zendo-core` (25 ignored, as before) and 11 in `zendo-shell`. The i18n test
  that every `t!("…")` names an English message, and every message is used,
  now reads the `src/` of every crate in the workspace. `cargo clippy
  --workspace --all-targets` has no warnings (the library's public items
  needed ten `#[must_use]`, applied with `clippy --fix`). `cargo fmt --check`
  still lists `app/steam_facts.rs` and `zendo-core/src/achievements.rs`, which
  it listed at HEAD too, and nothing of this item's.
- **Checked on screen** (throwaway home, no session bus, a private runtime
  folder `/tmp/zr46`, since a socket path over 108 characters fails): the
  Iced window opened with the 24 seeded games and the window class `zendo`;
  `links.sock` was created under the runtime folder (the link listener runs);
  with the real DualSense, three presses of D-pad right moved the cursor from
  the first card to the fourth (`Test Game 01`), through the moved gamepad code and
  `forwarded`. **Not checked**: the tray (no session bus, so no
  StatusNotifierItem to see), a link handed to the open window (`zendo
  nxm://…`), the sign-in window, the file dialog and Discord's presence (their
  code moved, with only the `APP_ID` path changed, and was not run).
- Left uncommitted with item 45's docs; one commit per item when asked.
- Not run: an RPM mock build. `packaging/fedora/zendo.spec` and the PKGBUILD name
  no path that moved (`crates/zendo/fonts/OFL.txt` and `crates/zendo/Cargo.toml`
  stay), and the new crate is a workspace library, but the spec was not built.

## Item 47, the `zendo-slint` skeleton (2026-11-14)

Built from the plan (`~/.claude/plans/drifting-hugging-scone.md`): a window
in Slint 1.18 that opens, paints itself from a theme, switches pages and
hides to the tray, with the pages empty (their name shows).

- **`zendo-shell::cli`** (moved out of `crates/zendo/src/main.rs`, which is now
  four lines): the command line, with the window passed in as
  `cli::RunWindow = fn(bool, Option<String>) -> anyhow::Result<()>`. Both
  binaries share it, so `zendo-slint list`, `launch`, `report`, the helper
  windows and `nxm://` behave as `zendo`'s do. `serde_json` left `crates/zendo`.
- **`crates/zendo-slint`**: `ui/style.slint` (a `Colors` global with the
  palette `app/style.rs` has, `Space`, and `Tr`), `ui/components.slint` (`Tab`,
  `IconButton`), `ui/top_bar.slint`, `ui/main.slint`; `src/window.rs`
  (`run`, tray, close, size), `theme.rs` (the Iced `style.rs` and `themes.rs`
  colour rules without the toolkit, over `zendo_core::theme` and `kde`), `tr.rs`.
  The Lucide icons and JetBrains Mono are imported from `crates/zendo/`, not
  copied.
- **Strings: kept Fluent** [the plan's tentative choice, now verified]. `Tr` is a
  global with `pure callback lookup(int, string) -> string` (plus `lookup-with`
  and `lookup-count`, not yet used by a page) and `pure public function t(id)`
  that passes `Tr.language`, so every text asks again when it changes. A test
  (`tr::tests`, on `i-slint-backend-testing`) changes the language on a live
  window and the page title goes `Library`, `Store`, `Loja`, `Biblioteca`. A
  message with two values (`top-bar-signed-in`) needs a second lookup shape; it
  comes with the first page that uses one.
- **Lints.** The generated code carries `#[allow(unsafe_code)]`, which the
  workspace's `unsafe_code = "forbid"` overrules (error E0453, four times). The
  crate restates the workspace lints in its own `[lints]` with `unsafe_code =
  "deny"`; handwritten `unsafe` would need an `allow`, which `allow_attributes`
  flags. Clippy (pedantic) then has no warning on the generated code.
- **Window.** `BackendSelector::new().select()` then `slint::set_xdg_app_id("zendo")`
  (the class blur rules name); `set_fullscreen(true)` for `--game-mode`;
  background `Colors.background.with-alpha(translucency)`, 2/3 by default (any
  value is possible now, item 45) and 1 when `config.toml` has `opaque = true`.
  Closing quits unless the tray is on; with it, closing hides, and the tray
  (`zendo-shell::tray::events`, a closure into `upgrade_in_event_loop`) toggles,
  opens and quits. `run_event_loop_until_quit` so hiding the last window does not
  end the program. The size is kept in a `thread_local` `Cell` and set before
  `show()`, because a Wayland app cannot unminimise: showing makes a new window.
- **Checked on screen** (throwaway home; no session bus except the tray run;
  stills with `spectacle`, clicks with `ydotool` guarded by the pid of the
  active and the pointed-at window, about 20 clicks, no stray one):
  - class `zendo`, 1368×892 like Iced's, JetBrains Mono, the translucent window
    (2/3) and the Library tab's accent rim;
  - each of Library, Store, Stats, Downloads and Settings clicked, the page's
    name changing and the tab or icon marked; the icon buttons' tooltips drawn
    over the page;
  - Portuguese from `language = "pt-PT"` (BIBLIOTECA, LOJA, ESTATÍSTICAS) and
    `theme = "system"` taking the scheme "PlasmonkA";
  - `opaque = true`: a blank pixel reads 23,23,23 (`#171717`) against 26,26,26
    translucent; `--game-mode`: 2560×1080 at 0,0; close with no tray: the
    process ends;
  - **the tray, on the real session bus** (the user agreed, since the skeleton
    reads only `Config::load_file`, which does not touch the keyring), driven by
    the StatusNotifierItem's own `Activate` and the dbusmenu's `Event`: close
    request leaves the process and the icon; a tray click brings back a window
    in front with focus; a second click hides it; menu Open shows it; menu Quit
    ends the process and the icon is gone; a window resized to 1200×800 and
    then 1150×700 came back at those sizes.
- **Found and fixed in the check.** (1) The icon buttons' tooltip was a
  `PopupWindow` opened on hover; the press never reached the button under it
  (Downloads and Settings did not navigate, with `pointer-event` logging `move`
  and `up` but no `down`); it is now a plain element drawn below the button,
  with `z: 1` on the top bar so it draws over the page. (2) The window came back
  from the tray at the default size; fixed with the size kept at hide.
- **Not checked**: the tray icon's look in the panel (only that it registers and
  answers), a real keyboard's focus (no key was sent), `Tab` keyboard navigation
  and accessibility (labels are set, not exercised), blur on a `zendo` window
  with `zendo-slint` itself (item 45 showed the class works; here only the class
  was read), a light theme, user themes from `~/.config/zendo/themes/`, a
  window on a second monitor or at another scale, and the tray's click on the
  panel itself (the same method, called over D-Bus).
- During the on-screen checks KDE's application launcher opened once, unasked by
  these tests (only pointer moves and left clicks were sent, and the click
  landed on Settings); the helper refused to click while the focus was elsewhere.

## Item 48, the Library in Slint (2026-11-14)

The Library page of `zendo-slint`, from the plan (`~/.claude/plans/drifting-hugging-scone.md`),
checked against Iced's on the same libraries.

- **Scope, as decided with the user.** The Collections sheet moved to item 50
  (the collection and saved-view chips filter now; COLLECTIONS and ADD TO
  COLLECTION come with the sheet). The hero backdrop moved to item 49: Iced's
  Library draws none, and `docs/design.md` puts the wide art behind the game
  page's header. Until item 49 a card opens a stand-in page that says so
  (`open-title`, Esc goes back). Add game, the RUNNING badge and Play wait for
  49, the update checks for 51.
- **Shared logic, out of the Iced app.** `zendo_core::browse`: `visible` and
  `visible_indices` (the filter and the sort), `step` and `Move`, `reveal_y`
  and `rows_in` (with a `Geometry`, since the toolkits' cards differ), and
  `set_pinned` and `set_hidden` for the bulk changes. `format.rs` moved to
  `zendo-shell/src/format.rs`, with `played_line` and `progress_of`; Iced's
  `app/format.rs` is a one-line re-export. Iced's `visible`, `step` and
  `reveal_y` call the shared code, and its 158 tests still pass.
- **`zendo-slint`.** `src/library.rs` is the page's state with no toolkit
  (filters, cursor, marks, chips with their counts, cards, empty states, and an
  `Outcome` of what to save, which rows to redraw and whether to scroll);
  `src/library_ui.rs` carries it to Slint (a hand-written `Model` of rows made
  when asked, the chip models, the scan and cover threads, the file watch, the
  controller, the keys); `src/covers.rs` is the cover cache; `ui/library.slint`
  is the page, `components.slint` gained `ChipButton` and `SearchField`, and
  `top_bar.slint` the view chips and the search. The cursor is a property
  compared by index, so a move touches no model; a mark redraws one row.
- **Covers, three tries** (the bar was the plan's: at least 35 fps on the
  2,700-card library with every game's own cover file, measured against Iced's
  Library first). `Image::load_from_path` on the UI thread: 45 to 71 fps but RSS
  1.4 GB, because some store covers are 1080 to 1200 px wide (6 MB decoded). The
  same with a downscale to 360 px on the UI thread: RSS 290 MB but dips to 2 fps
  on runs of big covers. Kept: two worker threads decode to a
  `SharedPixelBuffer` (an `Image` is not `Send`), the newest request first and
  at most 96 waiting, a blank card until the cover arrives, rows redrawn 40 ms
  after arrivals, 150 covers kept.
- **Measured** (1368×892, six columns, the library ×2,700 `.desktop` games in a
  throwaway home, `ZENDO_BENCH_SPEED` px/s through the real scroll path, a
  75 Hz screen; the rig was a throwaway worktree, never committed):

  | | idle | scrolling | RSS |
  |---|---|---|---|
  | Iced, dev, 3,000 px/s | 75 fps | 1 to 5 fps | 314 MB, up to about 1 GB |
  | Iced, dev, 800 px/s | 75 fps | 3 to 6 fps | 312 to 560 MB |
  | Iced, release, 800 px/s | 75 fps | 6 to 9 fps | 255 to 533 MB |
  | Slint, dev, 3,000 px/s | 74 fps | 61 to 75 fps | about 300 MB, flat |
  | Slint, dev, 800 px/s | 74 fps | 61 to 75 fps | about 300 MB, flat |
  | Slint, release, 800 px/s | 75 fps | 66 to 75 fps | about 310 MB, flat |

  The bar is met in both profiles. Iced's low figures are on distinct covers:
  it decodes each on the thread that draws.
- **Checked on screen** (the 24-game throwaway home with a seeded `state.json`:
  favorites, statuses, achievements, two collections and a saved view; stills
  with `spectacle`; about 40 clicks and as many keys with `ydotool`, guarded by
  the pid of the active window and of the one under the pointer):
  - every new control clicked: the view chips (All, Favorites, Recent, Hidden),
    the store chip, the three sort chips, Refresh, a status chip, a collection
    chip, the saved view, the mark box, the bulk Hide and Clear, a card, and the
    search field by mouse;
  - through the real keyboard: the arrows (with the grid scrolling to keep the
    cursor's card whole), Home, typing with no field focused, Esc (search, then
    marks, then a game's page), Ctrl+Space, Enter, Tab, Delete, Ctrl+H, Ctrl+F
    then Esc then Down, Ctrl+1 and Ctrl+2, F5 (the hidden game and the sort came
    back from `state.json`);
  - a resize from 1368 to 1100 wide: six columns to five, the cursor's card kept
    in view; the footer's status elides.
- **Found and fixed in the check.** (1) The FAVORITE badge, a `Rectangle` with no
  size, filled and dimmed the whole cover. (2) `FlexboxLayout` took the leftover
  height and stretched the chips when the page had no grid. (3) The grid reported
  its height only when it scrolled, so the cursor's reveal assumed 800 px and a
  card hung off the bottom. (4) The page rectangles did not stretch, so the
  footer floated. (5) The status message stayed after a page change and a tab
  press left an open game and the marks in place; tab presses now go through
  Rust. (6) My own `zclick.sh` clicked at the pointer when handed one argument
  instead of two (zsh does not split `"$p"`); it now refuses anything but two
  integers, and that click landed on a harmless chip.
- **Tests.** `browse` (5), `library` (10, with no window), `covers` (4, with
  real files on disk) and the window glue (8, on `i-slint-backend-testing`:
  typing, Esc's order, arrows, marks and the bulk chips, a chip that empties the
  page, page changes, the footer, a tab press). Workspace clippy is clean;
  `cargo fmt --check` still flags `zendo-core/src/achievements.rs`, which is
  not from this item.
- **Not checked.** The controller (the code gates it on the window's focus
  through winit; no pad was pressed), cover downloads and the file-watch rescan
  (the throwaway homes ran with `download_covers = false` and no launcher file
  changed), the wheel and scrollbar drag (no wheel through `ydotool`; the grid
  was moved by keys and by the bench's timer), Ctrl+V paste, a language change
  on a live window (`changed language` calls `relabel`; only the `Tr` part has a
  test), focus after the tray makes a new window, a light theme, blur, a
  second monitor or another scale, and the bulk Unfavorite and Unhide chips and
  Ctrl+P (covered by tests, not clicked). The 2,700-card library was measured,
  not looked at. F11 and the shelves are item 53.

## Item 49a, the game page in Slint (2026-11-14)

Item 49 is sliced in three (49a here; 49b the Steam facts, requirements, DLC,
ProtonDB and achievements; 49c anti-cheat, cloud saves, prefix, EOS overlay,
Deck, GOG changelog and playtime). 49a is not finished: local games and the
Add game sheet are still to do.

- **Run logic moved, not copied.** `zendo_shell::running` holds `Running`,
  `Session`, `RunMessage` and `follow` (the thread that prepares, starts and
  watches the game). `Running::apply` takes a message and returns `Effect`s
  (`Say`, `Launched`, `Played`, `Session`, `HideOnLaunch`, `Changed`, and
  the ones whose item is later: `CloudReload`, `CloudSyncing`, `Presence`,
  `NextJob`, `RefreshGogTotal`); each toolkit carries them out. Iced's
  `running.rs` is now a wrapper, and its seven tests became six in
  `zendo-shell` plus the Iced ones that still drive `Zendo`. `follow` takes a
  closure that returns `false` once nothing wants the messages (as the other
  `zendo-shell` threads do), and keeps `launch::after` and `gog_playtime::send`
  so a game started from Slint still syncs saves.
- **`zendo-slint`.** `ui/game.slint` (`GameData`, `GamePage`) and
  `src/game_ui.rs`, a child module of `library_ui` so it reaches the glue's
  fields. The page: back link, header over the faint wide art (the 960-pixel
  copy from `zendo_core::thumbs`, made on a thread, decoded by the covers
  worker through `Covers::get_wide`), cover, title, source line, Favorite and
  PCGamingWiki badges, Play / Starting / Running with Stop and Force stop,
  Favorite, Hide, Manage in Steam (Steam games), status chips, four tiles, the
  Sessions card, Details and a Technical card folded under it. Cards show a
  green RUNNING badge. Enter plays, Delete hides, Ctrl+P favorites, Esc goes
  back; on the pad A plays, Y favorites and X hides. `Browser` got
  `toggle_pinned_of`, `toggle_hidden_of`, `set_status_of`, `set_running`,
  `launched`, `add_minutes` and `game`, so the page acts on its own game even
  after hiding it moves the cursor. `zendo_shell::open` is `xdg-open`.
- **Checked** (clippy clean; 158 Iced, 19 shell and 32 `zendo-slint` tests,
  three of them new for the page): on screen in a throwaway home (12
  `.desktop` games, `Exec=sleep 70`, real art, two seeded sessions, no
  session bus) with `ydotool` behind pid checks: the grid opens a game; the
  page draws the backdrop, cover and tiles; Play started `sleep 70`, the
  button turned to Running with Stop and the line said so; Stop ended the
  process and the line said "closed"; Favorite and the Playing chip (pressed
  once; the chip highlights and the footer says so); Hide (HIDDEN badge, the
  tab counts moved, the button became Unhide); the Sessions card and the
  Details card side by side; the Technical toggle; the scrollbar by dragging;
  Enter played from the keyboard, Ctrl+P favorited and Esc went back to the
  grid with the RUNNING badge on the card; the PCGamingWiki button ran
  `xdg-open` (no error in the footer).
- **Found on the way.** Two columns left to the layout (stretch factors) put
  the narrow one where the wide one should be; explicit widths from the page's
  width fixed it. `game-back` already has its own arrow.
- **Not checked.** Where the PCGamingWiki link went (the run had no session
  bus and Zen is the default browser; no new window appeared), Manage in
  Steam (no Steam game in the seed), the controller, hide-on-launch with the
  tray, a game that ends by itself after a minute (the session written to
  `sessions.jsonl`), a game launched by Steam or Heroic (`Watch::Folder`),
  force stop, a light theme, a smaller window than 1100 pixels (the minimum),
  the wheel. **Open question for the user's eyes:** while the page is
  scrolled, the whole window looks lighter in a `spectacle` still (the 2/3
  window alpha over the wallpaper), and darker back at the top; the Library
  could not be scrolled in the seed (two rows) to compare. Not known whether
  this is the stills or Slint's partial redraw of a translucent window.

## Item 49a, local games and Add game (2026-11-14)

The last of 49a. `guess_title` moved to `zendo_core::local` (its test with it;
Iced uses it from there). `zendo-slint` got `ui/local_form.slint` (`LocalData`,
`LocalSheet`), `TextField` and `ActionButton` in `components.slint`, and
`src/local_ui.rs`, another child module of `library_ui`.

- **The sheet** is the first one that grabs input: a dimmed overlay inside the
  window's focus scope, so Escape reaches `LibraryData.back` first (sheet, then
  the game page, then marks, then search); a click outside the card closes it;
  while it is open the page's keys and the controller do nothing but close it
  (`Glue::key` returns early). Enter in a field saves. The same sheet edits a
  game (heading, Save). After a save or a Remove the library is read again and
  `after_scan` opens the game and says what happened. A game page for a local
  game has Edit and Remove (the first press asks, "Really remove").
- **Checked** (clippy clean; 157 Iced, 282 core, 19 shell and 35 `zendo-slint`
  tests, two new for the sheet): on screen, same throwaway home plus a
  `run.sh` (`sleep 70`) in `Games/Cave Story`: Add game opens the sheet with
  the program field focused; Add with nothing says "Give the game a title"
  and stays open; a pasted path (through `wl-copy` and Ctrl+V, clipboard
  restored) turned the hint to "A Linux program"; a title typed; Enter saved
  to `local.toml`, closed the sheet, opened the game's page ("Added Cave
  Story", library 12 to 13); Edit filled the sheet from the record and Esc
  closed it leaving the page; Play ran `run.sh` and showed Running with Stop,
  Stop ended it; Remove asked, then removed, back on the grid with the note
  and the count at 12 and `local.toml` without the game; Choose opened the
  GTK dialog as a child process, and closing it left the sheet as it was.
- **Found on the way.** With `XDG_DATA_DIRS=/nonexistent` (the throwaway
  recipe) the file dialog helper aborts (SIGABRT, GTK finds no GSettings
  schemas); the sheet showed "the file dialog failed: signal: 6" and stayed
  usable. For a check of the dialog, run with `XDG_DATA_DIRS=/usr/share:/usr/local/share`
  (the grid then also lists the system's applications). `ydotool type` uses a
  US layout: `/` came out as `-` on this keyboard, so paths go through
  `wl-copy` and Ctrl+V. The card re-centres when an error line appears, so
  fields move; take a still before each click.
- **Not checked.** Choosing a file in the dialog and the title guessed from
  it, a Windows `.exe` through Proton, the arguments field, the
  `Ctrl+V`/focus order with Tab (Tab does nothing in the fields), the sheet
  at a window near the minimum height, the controller with the sheet open.

## Item 49b, the store cards in Slint (2026-11-14)

The cards that come from the network, on the game page, in `zendo-slint`:
`src/cards_ui.rs` (a child module of `library_ui`, with `Ask<T>` for each
question: asking, got, failed) and the cards in `ui/game.slint`.

- **ProtonDB** pill beside the badges (Native, or the tier, in the tier's own
  colour or the theme's good, bad and dim; its words show on hover below it,
  the header is above the page for that), and the lookup that starts the
  rest: the Steam app id it finds asks Steam's store and the requirements.
- **About** (Steam's description), the Details rows from Steam (developer,
  publisher, released, genres, features with the controller line, Metacritic
  as a link), **Screenshots** (a row that scrolls sideways; each opens its full
  size), **System requirements** (minimum and recommended), **Achievements**
  (progress bar, ten at first, Show more; also writes the progress line the
  Library's cards show), **DLC** installed, and the card that offers to give a
  game a **Steam app id** when none was found, with the field and Save and
  Clear in the Technical card (it saves to `state.json` and asks again).
  Failed cards show why with Try again.
- Asked on a thread once a session when the page opens; tests ask nothing and
  feed the answers in. The page scrolls to its end through a request the page
  clamps (`GameData.scroll-end`): setting the scroll position straight from
  Rust went past the content and left the window blank.
- **Left for later, on purpose.** The DLC's owned list, Install and Uninstall
  (51, with the stores); the store's own facts (download size, publisher from
  the owned list) and the Details row that hides what a store already said
  (51); the Steam sign-in's account for achievements (50: the account Steam
  on this computer signed in last is used); anti-cheat and the Deck review
  (49c). Duplicated from Iced until the cut-over: `features_line`,
  `rarity_text`, the tier colours.
- **Checked** (clippy clean; 40 `zendo-slint` tests, two new for the cards):
  on screen in the throwaway home, with the network: a game ProtonDB did not
  know showed the "No Steam game was found" card; Set Steam app id opened
  the Technical card and scrolled to it; app 620 typed and saved: the
  footer said so, the PROTONDB NATIVE pill came (its hover said "Proton:
  Platinum from 353 reports"), About, the Details rows (developer to
  Metacritic 95 / 100), three screenshots and the requirements from Steam
  appeared; Technical said "620 (set by you)" with Save and Clear; Clear
  took it back, the cards went and `state.json` lost the entry.
- **Not checked.** A game with achievements and DLC (the seed has none; tests
  cover the lists and Show more), a failed card offline, a click on a
  screenshot, the pill or Metacritic (all `xdg-open`, as PCGamingWiki in
  49a), the pt-PT words, a window narrower than the page needs.

## Item 49c, how a game plays on Linux, in Slint (2026-11-14)

The rest of the game page's network and tool cards, in `zendo-slint`: three
child modules of `library_ui` beside `cards_ui` (`compat_ui.rs`, `prefix_ui.rs`,
`gog_ui.rs`), the cards in `ui/game.slint`.

- **Anti-cheat** card (`AreWeAntiCheatYet`): the verdict badge, the
  anti-cheats the game ships, the site's meaning, the latest change and the
  list's notes (each opens its source), and a button to the site. Shown first
  when the status warns (denied, broken, unknown), else after About; asked
  once a session after ProtonDB's answer gives the Steam app id.
- **Steam Deck** pill beside ProtonDB's (Verified in the theme's good,
  Playable in ProtonDB's Gold, Unsupported dim), Valve's notes in its hover
  text, the game's Steam page when clicked. The words for Valve's tests are
  duplicated from Iced until the cut-over. A failure of either shows as a
  card in the failures list with Try again (which asks both again).
- **Cloud saves** row in the Details card for Epic and GOG Windows games, in
  the tone of how the last sync went. The syncs themselves were already in
  `zendo_shell::running`; the window now reads their effects (`CloudReload`,
  `CloudSyncing`) instead of ignoring them.
- **Wine prefix** card for a Windows game Zendo starts: Wine settings,
  winetricks, Run a program, GOG's setup, the prefix's folder, a box to
  install components (saved in the game's launch options), the game's known
  fixes with why, and how the last tool went. One tool at a time per game,
  none while the game runs.
- **GOG**: the Changelog card (the newest update, three more each time Show
  earlier updates is pressed) and GOG's total time played, raised over
  Zendo's count (`Browser::raise_minutes`) when the page opens and again after
  a session was sent (`Effect::RefreshGogTotal`).
- **Moved to item 50**: the EOS overlay is a Settings section in Iced
  (`eos_overlay.rs`: Install, Update, Remove), not a game-page card as
  the backlog said.
- **Found on screen, fixed**: giving a game a Steam app id did not forget the
  anti-cheat and Deck answers, so a lookup made with no app id stayed "not on
  the list" (Iced forgets both); and a Changelog card built as conditionals
  inside a repeated layout drew its height but no text, so each line is one
  `Text` whose size, weight and colour come from its kind.
- **Checked** (clippy clean; 48 `zendo-slint` tests, three new): on screen in
  the throwaway home with the network. A Windows game added by hand, given
  Steam app 252490 (Rust): the Deck pill (Unsupported, its hover text with
  the note and "Opens the game's Steam page"), the Anti-cheat card first,
  DENIED in red with Easy Anti-Cheat, VAC, the site's words, the dated latest
  note and its button; the Wine prefix card: Wine settings and Install both
  ended with umu's own "needed" error in red (no umu on the stripped PATH, so
  no Wine started), the typed component was saved to `games.toml`, Run a
  program showed the file dialog's failure (`XDG_DATA_DIRS=/nonexistent`
  aborts the helper). A GOG-installed Windows game (Cyberpunk 2077's product
  id): the Cloud saves row ("They sync once the game has started"), GOG's
  setup button, the Changelog card (Patch 2.5 with headings and bullets), Show
  earlier updates (27 more) added Patch 2.31 after a gap, ProtonDB Gold and
  Deck Verified pills.
- **Not checked.** GOG's total (needs a GOG sign-in), the cloud line after a
  real sync, Run a program with a real dialog, a real Wine or winetricks run,
  known fixes (needs a game Heroic's list knows), the buttons that open the
  browser (the anti-cheat site, a note's source, the Deck pill), pt-PT, a
  light scheme, a window narrower than the page needs.

## Item 50a, the Settings page in Slint (2026-11-14)

The first slice of item 50: `ui/settings.slint` (the page, its `SettingsData`
global, a `Toggle` and a `Section`) and `src/settings_ui.rs` (a child module
of `library_ui`, which owns the config). Groups General, Keys, Library and
Diagnostics, as chips; Engines and Proton come with 50b.

- **General**: the language (the system's or each language; the window
  follows at once through `tr::show`, and Rust-built texts through
  `SettingsData.relabel`), the tray switch (`window::set_tray` puts the icon
  in the tray or takes it out while the program runs; closing hides the window
  only while the icon is there, and one hidden when the icon goes is shown
  again) and the Opaque switch (the window's `translucency` goes to 1 or 2/3
  at once: Slint needs no reopened window, as Iced did).
- **Keys**: the `SteamGridDB` key, as in Iced: Show/Hide, Check, Save (kept
  only after the site accepted it, or when offline), the notes (accepted,
  rejected, saved unchecked, removed, where it is kept), the link to get one.
  An answer for a key no longer in the field is let go. `TextField` has a
  `password` property for this.
- **Library**: covers on or off and Download now with its progress; Update by
  themselves and the download workers; the sources found with their counts.
- **Diagnostics**: Zendo's files with Open folder, Copy report (through
  `wl-copy`, else `xclip`, there being no clipboard in the Slint crate; the
  note says so when neither is there) and Open logs.
- A change is written on a thread through `Config::update`; the page shows it
  at once, and one that fails is put back and said in the footer. Nothing is
  written in tests (`persist`).
- **Left for later, on purpose**: the Theme section (the colour choices; the
  Opaque switch is already here) and the Discord switch (50b, with presence),
  Nexus's key (52), what Auto-update does (51: the switch is saved), Engines,
  Proton and runtimes, the Epic overlay section (50b), Accounts, About and
  Collections (50c).
- **Checked** (clippy clean; 50 `zendo-slint` tests, one new): the new test
  changes Opaque, the covers switch and the workers, checks a key by hand
  (rejected: not saved; an answer for another key: let go; accepted: saved,
  and the Save button goes dim), moves between groups and counts the files
  and sources.
- **Checked on screen** (2026-11-14, throwaway home, no session bus, the user
  watching): Ctrl+5 opens the page; the Portuguese chip changed every text
  at once and wrote `language = "pt-PT"`, English put it back; Opaque made
  the window solid (`opaque = true`) and off again; Keys: typing shows dots,
  Show/Hide, Check reached the real SteamGridDB, which turned a made-up key
  down, Save said "so it was not saved" and stored nothing; Library: covers
  and update switches and workers 4 landed in `config.toml`, the sources'
  counts; Diagnostics: Copy report put the 63-line report on the clipboard
  with a green note (the clipboard was empty before and was cleared after).
- **Found and fixed on screen**: the Files card was measured too short and
  the last row drew under the next card (wrapped texts need explicit widths:
  the card's inner width less the other columns); the "Get a free key" link
  was centred (a child of a plain `Rectangle` is centred unless `x: 0`).
- **Not checked.** The tray switch (no session bus in a throwaway run), Open
  folder and Open logs (they start Dolphin), Get a key (the browser), a good
  key's Save, removing a key, Download now with a real key, the keyring (the
  throwaway run has none: the page said so in red), the switches surviving a
  restart, a narrower window.

## Item 50b, Engines, Proton, Theme and Discord in Slint (2026-11-14)

The second slice of item 50. Settings now has Iced's six groups in its order
(General, Keys, Library, Engines, Proton, Diagnostics; Diagnostics is group 5
now) and the cards Iced had on them, except the Nexus key (52). New modules,
child modules of `library_ui` by `#[path]` like the others: `engines_ui.rs`,
`runtimes_ui.rs`, `overlay_ui.rs`, `protons_ui.rs`; the page is `ui/settings.slint`
(a `SetRow` struct, a `SetList` and a `DownloadBar` component, a `RowButton`).
All logic is `zendo_core`'s (`engines`, `ge_proton`, `proton`, `runtimes`,
`eos_overlay`, `theme`, `kde`); the ported part is the glue and the rules.

- **Engines group**: the engines card (PATH first, Check again, a row for each
  of legendary, gogdl, nile, comet and umu-launcher: whose copy and where, the
  latest on GitHub, Download, Update, Up to date, Remove asked twice, one
  download at a time with a live bar and Stop); the Anti-cheat card (Easy
  Anti-Cheat and BattlEye from Lutris, Download, Update, Remove asked twice);
  the Epic overlay card (Install or Update, Remove asked twice, through
  legendary; it follows legendary being installed or removed, which Iced only
  looked at when Settings opened).
- **Proton group**: the default Proton as chips (Automatic, each Proton found,
  and the chosen one even when gone), the Protons found with Remove asked twice
  (only in the user's own tools folder, and "chosen by N games"), GE-Proton and
  Proton-CachyOS releases (five, then ten more a click, reading GitHub's next
  page when needed), Download with the live bar, and the notes. A removed
  default goes back to Automatic and the removal note stays (Iced's generic
  "default set" note arrived later and replaced it).
- **General group**: the Theme card (a chip for Zendo's own, each user theme
  file and, where there is a `kdeglobals`, System; the note for a missing
  theme or the scheme followed; a line for each file that is not a theme; the
  Opaque switch moved here, as in Iced) and the Discord card. Choosing a theme
  repaints the whole window at once and writes `theme`. `Themes` moved into
  `Glue` (shared with the watcher), gained the problems, the note, `reload`
  (as Settings opens) and the sort. The watcher of `kdeglobals` now always runs
  (it only started when System was chosen at launch, so choosing System in
  Settings would not have followed) and tells Settings after a repaint.
- **Discord**: the switch spawns or drops the presence thread; the Presence
  effect of a running game (`Effect::Presence`, dropped until now) tells it.
  It starts at launch when the setting is on.
- **Busy rule**: replacing or removing an engine waits while a game runs;
  Iced also waited for a store job, which comes with the Downloads page (51).
- **Tests** (clippy clean; 66 `zendo-slint` tests, 16 new, most ported from Iced's):
  a download at a time and how it ended (engines, Protons, runtimes), Remove
  asks once first (engines, Protons, runtimes, the overlay), older releases a
  few at a time, each build keeps its releases, the latest release of each
  engine, a row's offers, the default choices, removing the default, and the
  theme and Discord choices. Nothing in them reads or writes the user's files
  or the network (`persist`), and the launch-options rewrite on removing a
  Proton, which Iced did even in tests, is gated.
- **Checked on screen** (2026-11-14, throwaway home, no session bus, the user at
  the PC): the General cards (Dusk repainted everything and wrote `theme`; the
  broken theme file's parse error showed in red; Zendo's chip put it back;
  Opaque and Discord switched and persisted); a seeded `kdeglobals` made the
  System chip appear, its note named the scheme, and editing the file changed
  the whole window within seconds and the note's name; Engines: GitHub's five
  latest tags, a real download of legendary 0.21.1 (checked, installed,
  "Up to date", the overlay card then offered Install), Remove asked, then
  deleted it; PATH first wrote and unwrote `engines_on_path_first`; Check again;
  Anti-cheat: Lutris' list gave Download on both; Proton: the default chips,
  two seeded Protons, choosing one as the default and removing it (back to
  Automatic with the removal note), GitHub's GE-Proton list, Show older
  versions, Proton-CachyOS and back, and two real downloads of about 500 MB
  that installed (the bar showed "536.51 MiB of 540.60 MiB") and were
  removed again.
- **Found and fixed on screen**: a button with no label still added its gap,
  so the buttons of different rows did not line up and the status words sat
  below the button line (the gap now lives inside the button); the overlay
  card kept saying "legendary is needed" after legendary was installed; the
  removed-default note was replaced by the generic one; the scheme's name in
  the note went stale after a live change.
- **Mistake of the check**: after a download finished the page re-laid out,
  and a click aimed at Stop landed on the last release's Download and started
  a second 500 MB download. Aim at a moving page from a fresh still.
- **Not checked.** Stop on a download (the link is too fast to catch one: 540
  MB in seconds; covered by the glue tests only), the Update button (needs an
  older copy), Anti-cheat Download, Update and Remove (nothing was
  downloaded), the overlay's Install and Remove (through legendary, a large
  download; the button was enabled and not pressed), Discord presence itself
  (no session bus, no Discord), pt-PT on the new cards, a light KDE scheme,
  the Engines buttons while a game runs, a window narrower than the page.

## Item 50c, Accounts, About and Collections in Slint (2026-11-14)

The last slice of item 50: the two pages under the Settings button besides
Settings, and the Collections sheet with the Library chips and the game page's
row that open it. New files: `ui/about.slint`, `ui/accounts.slint`,
`ui/collections.slint`; `src/accounts_ui.rs` and `src/collections_ui.rs`, child
modules of `library_ui` by `#[path]` like the others; `zendo_shell::collections`
(what the sheet lists, shared with Iced). The Steam card (sign-in, owned and
family lists) was decided with the user to go whole into item 51, with the
Store's lists; nothing in the Slint code reads a Steam session yet.

- **Navigation**: `Page` gained `accounts` and `about`; a row under the top bar
  (Settings, Accounts with "n of 4" when any store is signed in, About) shows on
  the three pages and the settings icon is current on all of them. Ctrl+4 and
  Ctrl+6 open Accounts and About, F1 opens About. The row needs
  `vertical-stretch: 0` or its chips stretch to the page's height.
- **About**: Zendo, the version, the intro, Source code and Report a problem
  (through `SettingsData.open`), Licence and Built on. Two new messages, in
  both languages, say it is built with Slint under the GPLv3 (`about-built-slint`,
  `about-licence-text-slint`); Iced's stay for Iced. No `AboutSlint` widget (the
  plan's table was out of date: item 46 chose GPLv3).
- **Accounts**: a card per store (Epic, GOG, Amazon) with who is signed in, the
  engine and its path or that it is missing (nothing is said while the engines
  are still being looked for, which Iced did), Sign in or Sign out, GOG's two
  switches while signed in (`gog_playtime`, `gog_galaxy`, new `Setting` cases),
  and the sign-in's steps: in Zendo's window (a line for what it waits on,
  Try again, Use my browser instead, Cancel) or in the browser (the three
  steps, Open the page again, the field, Finish, Cancel). The window's code, the
  window being closed and a window that cannot open (the browser takes over)
  are all handled as in Iced, with the guards against an answer for a sign-in
  that is gone. Typing with no field focused on the page starts the code; Ctrl+V
  pastes into it. The engines are asked when Zendo starts and each time the page
  opens.
- **Collections sheet**: one list of the hand-picked collections and the saved
  views by name (`CollRow`, one struct with every text and flag); a tick per
  hand-picked one when the sheet was opened for games; Create, Save view (a
  same-name view is updated), Rename in place, Delete asked twice, Open on a
  view (closes the sheet and applies it); notes in green and orange; Enter in a
  field creates or renames; Escape leaves a rename, then closes the sheet. It
  opens from the COLLECTIONS chip (with the count), ADD TO COLLECTION in the
  bulk chips (for the marked games) and Edit collections on a game page, which
  now has Iced's collections row (badges, or None). The page's keys and the pad
  do nothing while it is open. `Browser` gained `set_collections` (puts the
  state's lists on every game and drops a filter on a collection that is gone).
- **Shared code and tests**: `summary`, `items` and `member` moved to
  `zendo_shell::collections` with their two tests, and Iced uses them. The i18n
  test now also reads `Tr.t("…")` in `.slint` files, because a message used only
  by Slint looked unused. `Section`, `Toggle`, `Note` and `RowButton` moved from
  `settings.slint` to `components.slint`, with a new `CheckBox`. 16 new tests in
  `zendo-slint` (82 in all; workspace tests and clippy are clean): the sheet's
  create, delete asked twice, names taken by the other kind, update, rename,
  apply and delete of a view, the bulk chip and the ticks, Escape's order, the
  game page's row; the keys and the row; the card counts, the switches, the four
  window tests ported from Iced, a refused paste, typed text and an answer after
  Cancel. `persist == false` keeps every one of them off the engines, the
  window, the browser and the user's files.
- **Checked on screen** (2026-11-14, throwaway home, no session bus, the user at
  the PC, a seeded 24-game library, fake `legendary`, `gogdl` and `nile`
  scripts in the home's `bin`, and a fake `xdg-open` that logs): the COLLECTIONS
  chip and its count; Rename in place (typed a new name, Enter: renamed, list
  re-sorted, chips behind it followed), Delete asked then done, Create with
  nothing typed (refused) and with a name by Enter, Save view (its chip lit as
  the view on screen), Open on a saved view (sheet closed, A-Z chosen), two
  marked games and ADD TO COLLECTION, the tick (note, count, chip), Escape
  closing only the sheet, Edit collections on a game page (badges updated
  behind the sheet, the page stayed open); the new row and Ctrl+4; About;
  Accounts with no engines (Sign in disabled, why), with engines, GOG signed in
  (fake `gogdl auth`) with its switches (one turned off: `gog_playtime = false`
  in `config.toml`), Epic's Sign in opening the real sign-in window (its page
  failed to load here, "WebKit encountered an internal error": no session bus),
  Use my browser instead (the window closed, the fake `xdg-open` logged
  `https://legendary.gl/epiclogin`), typing with nothing focused filling the
  field, Finish refusing "no code here" with the right note, Cancel, Amazon's
  Sign in (fake nile made the page, the window opened, Cancel closed it and its
  process), GOG's Sign out (card, switches, count and the credentials file
  gone, footer note).
- **A second run** (same day, same setup, after review) covered what the first
  skipped, by mouse: the ACCOUNTS chip, both About buttons (the fake
  `xdg-open` logged `https://github.com/leandrop-dev/zendo` and `.../issues`),
  the second GOG switch (`gog_galaxy = false` written), TRY AGAIN (a fake nile
  that printed garbage gave nile's parse error with TRY AGAIN, USE MY BROWSER
  and CANCEL; with the fake fixed TRY AGAIN restarted and the window opened),
  Amazon's browser steps (the page logged, OPEN THE PAGE AGAIN logged it a
  second time, FINISH on "nothing to see" gave "No code there: paste the
  address the page ended on"), the sheet's OK and CANCEL buttons, Escape during
  a rename (the rename ended, the sheet stayed, and typed text went into the
  name field), and a sheet over 26 collections: the card stopped at the
  window's height, the list scrolled with the wheel, and CREATE, SAVE VIEW and
  CANCEL stayed on screen and took clicks.
- **Found and fixed on screen**: the rename field did not take the keyboard,
  because the model is written again for each change and `changed` does not
  fire for a row's first value (`init` does the same); the new row's chips
  were as tall as the page; **keys died after a field went away**: Cancel on
  the Accounts steps or on the Collections sheet destroyed the field that had
  the keyboard, and Ctrl+1 and Escape reached nothing until a click on the page
  (the Add game sheet's Cancel does the same, fixed alike but not tried on
  screen). Closing a sheet or the steps now calls `focus_page`, and a rename
  that ends hands the keyboard to the name field (`CollectionsData.focus-draft`);
  the list's scrollbar sat on the Delete buttons (the list now leaves 14 px).
- **Added beyond Iced**: the engines are asked again each time Accounts opens
  (Iced asked only at start). A slow answer for GOG can land after Sign out and
  show the account signed in until the page is opened again.
- **Not checked.** A real sign-in to Epic, GOG or Amazon (the codes, the
  engines' own answers, the keyring), a pasted code that is well formed, Ctrl+V
  into the field with a real clipboard (the key's routing is a glue test), the
  Add game sheet's Cancel keeping the keys, the sign-in window loading a real page, `fetch_owned` and the Store's lists after a
  sign-in (51), pt-PT on the three pages, a light KDE scheme, a controller on
  the sheet (the pad only closes it), and the count with a Steam session (none
  can be made until 51).

## Item 51a, the Store, a store game's page and Steam's card in Slint (2026-11-14)

The first slice of item 51 (sliced with the user: 51a here, 51b Downloads and
the installer, 51c Steam installs and the game page's jobs). New files:
`ui/store.slint`; `src/store.rs` (the page's state with no toolkit in it,
`StoreBrowser`), `src/store_ui.rs` (its glue and the store game's page) and
`src/steam_ui.rs` (Steam's card), the last two child modules of `library_ui` by
`#[path]` like the others; in `zendo-shell` `steam_list` (Steam's list with the
sign-in, moved from Iced) and `store_facts` (`InfoState` and the rows the details
card shows, moved from Iced). `zendo_core::owned` gained `store_of`, `Filter`
(`keeps`), `sort` and `sort_key`, and `zendo_shell::format` gained `owned_line` and
`played_on`; Iced calls all of them, so each rule is written once.

- **The Store page**: a chip for All, each store with its count and Not
  installed only; the three sort chips (Tab cycles); Refresh (says Asking…
  while a store is asked, with a line naming the stores); the errors, in a
  card (a line per store); the empty states of Iced's page (sign in first with
  Go to Accounts, asking, no match, none here, nothing); the count beside the
  tab; the Library's grid of covers with an Installed badge and no mark box
  (`CardView` got `markable`). Arrows, PageUp/PageDown, Home/End, Enter, typing,
  Backspace, Tab, Esc, F5 and Ctrl+R/F/V, and the pad (A opens, Y sorts), work
  on it; the footer has `hints-store` and `hints-owned`. The top bar's search is
  one field per page in the same place, so the tabs do not shift.
- **Where the lists come from**: the lists kept from last time are read on a
  thread at start, and no store is asked until they are in (`lists_loaded`);
  after that a store signed in with no list is asked, on each account check
  and once when the first scan has brought the settings (Steam's session
  among them). A list kept from last time waits for Refresh, a sign-in asks
  its store at once, and a sign-out forgets its list. Covers of store games
  not in the library download with the library's (`download_images` takes
  `Box<dyn Artwork + Send>`, as Iced's `Job`).
- **A store game's page** is the game page in an owned mode (`GameData.owned`)
  rather than a copy: the header, About, the network cards (ProtonDB and Deck
  pills, anti-cheat, screenshots, achievements, requirements, Try again) and the
  Technical fold are shared; the trail `STORE / TITLE`, the platform badges and
  Installed, Open in library (Enter and the pad's A as well) or what installing
  would do ("Installs into … · needs …", "no home folder", Steam's words), the
  play-time tile, the DLC the store says the user owns (the size, or "comes with
  the game") and the details card (store, owner when shared, status, platforms,
  developer, the store's sizes and latest version, publisher, released, genres,
  time played, last played, played on) are its own. `show_game` is the one
  dispatch point: every answer that arrives calls it, and it writes the store
  game's page when that is the open one. The cards look a game up through
  `card_game`, the library's or a stand-in made from what the store says.
- **The installed game's page** now has the store's rows in its details card
  and the DLC list the store says the user owns beside the installed ones (the
  count "n of m"); Steam's own rows leave out what the owned entry said.
- **Steam's card on Accounts** (above the three stores, as in Iced): Sign in
  with Steam opens Zendo's sign-in window on Steam's page, Cancel closes it,
  Sign out forgets the session, the kept list and Steam's cached profile; the
  signed-in line, the family when the last list said, the notes and where the
  token is kept. The session is saved through `Setting::SteamSession`, so it is
  applied at once, written on a thread and put back if the write fails. One
  rule decides "signed in to Steam": the session must parse (Iced used its mere
  presence for the Store and the parse for the card); the card, the Store, the
  tab's count and the achievements all use it. Achievements for a Steam game
  ask with the sign-in's account first, then Steam's own user on this computer.
  A renewed token is kept; Steam's page not knowing the sign-in drops the
  session with "sign in again".
- **Tests**: 11 in `zendo-slint`'s Store glue (the chips and counts, the search,
  a card opens the page and Escape returns with the cursor, Open in library,
  the keys, sign-out, a store that failed, the store's words on the page,
  Steam's card from sign-in to a renewed token to a lost session, sign-out, a
  session that does not parse) and 9 for `StoreBrowser`; the sort, the filter and
  `store_of` moved to `zendo-core` with their tests, `owned_line`/`played_on` and
  `store_facts` have their own. 102 tests in `zendo-slint`; the workspace's
  tests and clippy are clean. `persist == false` keeps every one of them off
  the disk, the engines, the network and the window.
- **Found by tests**: `StoreBrowser` read the cursor's game after the lists were
  replaced, so a list landing moved the cursor to another game; a click moved
  the cursor in the browser but not in the window.
- **Checked on screen** (2026-11-14, throwaway home, no session bus, the user at
  the PC; copies of the first ten games of each store's cached list with their
  covers, fake `legendary` (prints `[]` for `list`), `gogdl` and `nile`, a
  seeded Steam session with a far-off expiry, one GOG game installed): the Store
  with 40 games and the Installed badge; each chip (Steam, Epic, GOG, Amazon,
  Not installed only on and off, All), each sort chip; a Steam card (the page
  with the ProtonDB and Deck pills, About, the details card, screenshots,
  achievements, the Technical fold, Escape back with the cursor on that card), an
  Amazon card (its install note with the install folder), the installed GOG card
  (Installed badge, Open in library, which opened its own page); typing
  searched and Escape cleared it, Right, Down, Tab and Enter; Accounts with
  Steam's card, Sign out (the list left the Store, `steam_session = ""` in
  `config.toml`, the cached list removed, the counts fell to 30 and 3 of 4),
  Sign in with Steam (the sign-in window opened as its own process) and Cancel
  (it closed); Refresh (Epic's list emptied by the fake, GOG's 401 and Amazon's
  missing `library.json` in the errors card, the other lists kept).
- **A second run** (same day, same setup, after a review of the first found a
  bug in its own still): the Library page after Open in library showed the
  store game's platform and Installed badges before PCGamingWiki, because the
  header's badge row read a model that was never emptied; `close_owned` now
  empties it (and a test asserts it), and the store page's empty tiles row no
  longer adds a second gap under the actions. Checked after the fix: the
  library page has only PCGamingWiki and the pills; both pills on a store page
  (the Deck pill logged `https://store.steampowered.com/app/735580` in a fake
  `xdg-open`; the ProtonDB pill opened the real browser, see below); the
  trail's `← STORE` link by mouse (back on the grid, cursor on that card); the
  Store's search field by click and typing, then Escape and Ctrl+F (the field
  took the keyboard and "ar" filtered to five games); the Library's mark box and
  search field (restructured with the top bar); Refresh with Steam signed in
  (the label said ASKING…, the line "Asking Steam for your games…" showed while
  the hidden renewal window ran, then the card said "Steam's sign-in has
  ended: sign in again", Steam's games left, the other lists and the errors
  stayed, the cursor stayed on its card); and, in a home with nothing signed
  in and no cache, the sign-in-first state (Refresh disabled, counts 0) and GO
  TO ACCOUNTS, which opened Accounts.
- **Not checked.** A real Steam sign-in on Steam's page, a real token and a
  list fetched with it, the family line, a renewal that works; the DLC list and
  the store's sizes ("needs …") from a real engine (the fakes said nothing
  about a game); the library page's details with the store's rows; Ctrl+V and
  Ctrl+R on the Store (glue tests only); the pad; pt-PT; a light KDE scheme.
  Left out on purpose, each marked in the code and listed in
  `docs/backlog.md`: Install, Import, See downloads and "Also install its DLC"
  (51b), Install with Steam (51c).
- **Network and side effects during the checks**: ProtonDB, Steam's store and
  the achievements were asked for real; GOG's library with the fake token
  (401); Steam's Web API with a fake token on Refresh; the sign-in window
  loaded Steam's page. In the second run the first ProtonDB pill click went to
  the real `xdg-open` (the seed script had been cut short before it made the
  logging fake) and opened `protondb.com/app/735580` in the user's Firefox: a
  tab for them to close.

## Item 51b, Downloads, the queue and Install in Slint (2026-11-14)

The second slice of item 51. **The "installer sheet" listed under 51b is the
FOMOD mod installer** (Iced `app/installer.rs`: a mod's steps, options and
pictures), opened only from a mod's install; store installs have no sheet in
Iced (Install queues at once, "Also install its DLC" is a checkbox on the
page). Decided with the user: it moves to item 52, with the Mods page.

- **The queue is shared, as `running` is.** `zendo_shell::queue` holds the
  state and every rule (`Queue`, was Iced's `DownloadsState`): the jobs, the one
  running, what finished, why a job failed, the question about closing, the
  updates found. Each method says what happened and returns `Effect`s (`Say`,
  `Save`, `Start`, `Discard`, `Rescan`, `Exit`, `ShowDownloads`); the window
  carries them out and calls `next(held)` after every change to start what is
  due. Also there: the action words, `progress_line`, `update_line` (a `Tone`,
  each toolkit has its own colours), `ask_updates`, `run` (the job's thread,
  reporting to a closure like `running::follow`) and `clear_away`. Iced was
  switched to it first: `app/downloads.rs` went from 1,242 lines to the message
  mapping, the folder dialog and the views; `DownloadMessage::Event` and
  `Ended` became one `Report`; the six queue tests moved with the logic (12 in
  `queue.rs`, with the order of a Done job, a held job, the question about
  closing and the update checks added) and Iced kept six that go through its
  messages. `confirm_uninstall` lives in `Queue` too (51c's game page needs it).
- **The Downloads page** (`ui/downloads.slint`, `src/downloads_ui.rs`): three
  tiles (Up next, Paused, Finished), the question about closing, the running
  card (title, "Installing · Epic Games", Pause and Cancel that say Pausing… and
  Cancelling…, a bar once the engine gives a percentage, the line "48.0% ·
  432.0 MiB downloaded · 25.0 MiB/s · 00:00:10 left"), Up next (Pause, Cancel,
  "Waits until the game is closed"), Paused (Resume, orange, or Try again with
  the error in red, Cancel), Finished (Done or why, Open, Clear), and the
  empty state with Go to the Store. The tiles count as Iced's: Up next is the
  queue less the paused, so the running job is in it. The tab's count is the
  queue's length, the footer falls back to the running job's line or "n
  downloads paused" when Zendo has nothing of its own to say (as in Iced, a
  status line comes first).
- **Progress**: the engine's log is read many times a second. Every event goes
  into the queue at once, so the page is never behind what the engine said, and
  only the redraw is coalesced: the first progress of a burst starts a 100 ms
  timer (`progress_due`, as the covers' `covers_due`) and the timer writes the
  running card, the bar and the footer from the latest. (The first version held
  progress back in the worker and flushed it before the end only; the stills
  showed it, a percentage with the previous tick's megabytes, and the next burst
  dropped what was held.) `Folder` is kept in the queue file at once.
- **The store game's page**: Install and Import from a folder… (Zendo's file
  dialog), "Also install its DLC (n, size)" (the DLC the store says the user
  owns and can install; the box keeps its choice per game), or Queued /
  Installing / Resume with See downloads and "Into <folder>". Enter (and the
  pad's A) installs, or opens an installed game in the library. Steam's own
  games keep their note until 51c.
- **Around it**: the queue left from last time is read at start (paused jobs stay
  paused; a job not paused goes on); a scan asks the stores about the games
  Zendo installed that were not asked about; with "Update games by themselves"
  on, the updates found are queued, and turning it on queues those already found;
  an engine cannot be replaced while a job runs; a game cannot start while its
  files move; jobs that change a game's files wait while the game runs.
- **Closing**, in Iced's order: with the tray there, the close button only hides
  (even mid-download); the question is asked where the program would end, the
  close with no tray and the tray's Quit (which shows the window first). The
  window stays (`KeepWindowShown`), Downloads opens with the question, Enter or
  A is Close anyway (the engine stops, the queue is held and saved, then the
  window goes), Escape or B keeps downloading.
- **Tests**: 12 in `zendo_shell::queue`, 7 in `zendo-slint`'s glue (Install once
  and the buttons through queued, running, paused, the tile and the coalesced
  progress; a failure and Try again, Cancel, Clear; the DLC box; Enter; closing
  and its keys; the engines' buttons; the update checks and the switch), 154 in `zendo` (Iced), 109 in
  `zendo-slint`, 35 in `zendo-shell`, 285 in `zendo-core`. Clippy is clean.
- **Checked on screen** (2026-11-14, throwaway home, no session bus, the user at
  the PC; three Epic games from the real cache with a cover, a fake `legendary`
  that prints the real log's lines for about 20 s, ends on SIGINT, fails when a
  marker file is there and writes the record Zendo reads back; a logging fake
  `xdg-open`): the store game's page with Install, Import from a folder…, the
  note and the DLC box; Install (the engine got `--with-dlcs`, the tab said 1,
  the footer said Installing); See downloads; the running card; Pause (the job
  waited paused, the footer said so), Resume (the engine ran again) and Cancel
  (a red Cancelled, "its files are gone"); a failure (the engine's error in
  red) and Try again; a close request with a job running (the question, the
  footer's keys), Keep downloading (the job went on) and Close anyway (the
  process ended, `downloads.json` had the job paused with its folder), and the
  queue back, paused, after a restart and Resume; Done (Library 3 to 4, the
  Installed badge in the Store, the update check in the engine's log); the
  Settings switch (three Update jobs queued, one running, the tab said 3, the
  engine got `--update-only`); Import with the real folder dialog and a typed
  path (the engine's `import`, Open in library, Library 7); Open (the game's
  library page), Clear, Go to the Store, and Enter on the store game's page.
- **A second run** (same day and setup, after a review of the first found
  controls never clicked and a stale progress line): the DLC words clicked (the
  box unticked, the engine got `--skip-dlcs`); three installs queued (Install,
  then Enter on two pages) and the queue went on by itself, one at a time;
  Pause on a row in Up next (it moved to Paused, Resume orange); Resume from the
  store game's page (the page read Resume, then Installing, and the engine
  ran); Pause on the running card and Cancel on the paused row (nothing
  running: "Cancelled …; its files are gone", the tiles followed); a close
  request with a job running answered by Escape (the question went, the job
  went on) and again by Enter (Close anyway: the process ended, the job saved
  paused); the progress lines pair up (92.0% with 828.0 MiB, 48.0% with 432.0,
  36.0% with 324.0; the fake prints percent times nine). **The Iced window**,
  which the rewrite of `app/downloads.rs` made run on the shared queue, was
  launched in the same home: it read the saved queue (Paused 1, Resume orange,
  the footer "1 download paused"), Resume ran the engine (24.0% with 216.0 MiB
  on the running card), the job ended Done with Open, Library went to 6, the
  tab's count cleared and the update check ran after it.
- **Found on screen**: the title, the tiles' numbers and the empty message drew
  black on the dark cards (a `Text` with no colour; each now says
  `Colors.foreground`); Enter on a store game's page only opened an installed
  game, where Iced queues the install (found by reading Iced's `activate` while
  the page was open; now `owned_activate`, with a test). A Pause that did not
  stop the fake was the rig's: a background process started from a script
  ignores SIGINT, and so did the engine under it (`grep SigIgn
  /proc/<pid>/status` showed 0x1006); `set -m` in the launcher fixed it. The
  job then ended Done, which is what `zendo_core::downloads::run` does with a
  Stop at the last moment of an install the record already has. A real
  legendary, a Python program, would behave the same: Python keeps a SIGINT it
  inherited as ignored (`signal.getsignal` printed `SIG_IGN` from a background
  job and the default handler from a normal shell, checked); Zendo started from
  a launcher or a terminal does not hand it down, and a Stop that goes unheard
  ends in `kill -KILL` after the grace. The review of the first run found: the
  stale progress above; the Up next tile that left the running job out (Iced
  counts the queue less the paused); Resume and Try again, which set a primary
  flag the Slint never read (`RowButton` has `primary` now); the DLC box's words,
  which did not toggle it where Iced's label does; and a claim in the backlog
  that every control was clicked when four were not (corrected).
- **Not checked.** A real engine or a real install (the fake stands for
  legendary: GOG's and Amazon's paths are `zendo_core::downloads`' and Iced's,
  with its tests; the sizes in the page's note come from the fake's `info`); the
  pad (A, B) on the question about closing; the tray's Quit with a job running
  (no session bus); the "waits until the game is closed" note (a game running
  while an update is due); the folder a Cancel clears (the fake makes it only at
  the end, `downloads::discard` has its own tests); a game with DLC the engine
  really reports (the DLC box was the fake's); the Iced window's other flows
  (Install, Import, Move, close question) beyond the Resume, Done and update
  check above, whose code is the shared queue's and has tests; pt-PT; a light KDE
  scheme. Left out on purpose: the Steam card and count on Downloads (51c), the
  mods' card (52), the game page's Update, Check, Move, Repair, Uninstall and job
  line (51c).
- **Side effects**: only the scratch home (7 MB, deleted); nothing opened a
  browser (the fake `xdg-open` logged nothing); `ydotoold` and the sockets were
  stopped and removed.

## Item 51c, Steam installs and the game page's jobs in Slint (2026-11-15)

The last slice of item 51.

- **Steam's installs are shared, as the queue is.** `zendo_shell::steam_installs`
  holds the state and the rules (was Iced's `app/steam_installs.rs`): what was
  asked of Steam and has not started, uninstalls and verifies asked, what Steam
  is downloading, the apps seen at the last look (an install, an update, an
  uninstall noticed there), and the "looking" guard. The methods return what
  the window must do instead of touching its status: `install`, `uninstall` and
  `verify` give the line to say (and call `steam::ask`, unless `persist` is
  false), `begin_look` says where the log's verifies begin, `looked` returns
  `Looked { say, rescan }`, `rows()` gives the Downloads card's lines in Iced's
  order (asked, uninstalling, verifying, busy), and `every()` is 2 s while
  something is going on and 15 s otherwise. Iced was switched to it first: its
  `steam_installs.rs` went from 713 to about 190 lines (the messages, the
  card and the tick streams); its four tests moved with the logic (6 in
  `steam_installs.rs` in the shell: the install, what Steam does by itself, a
  verify, the card's order, one look at a time, the progress words) and Iced
  kept one that goes through its messages.
- **Slint, `steam_installs_ui.rs`.** A repeating `slint::Timer` looks at Steam's
  manifests on a thread and hands the answer to `steam_looked`; the interval
  follows `every()`. A store game of Steam's shows Install with Steam (and Enter
  or A asks the same), Ask Steam again and Stop waiting while Steam has not
  started, and, once it has, "Installing…" or "Paused in Steam" with the line
  ("Downloading, 400 B of 1000 B") and a bar (`owned-job` 5 to 7,
  `owned-progress`). The Downloads page has the Steam card (Open Steam's
  downloads, one line per install with Stop waiting only for an install not
  started, a bar for a download) and does not invite to the Store while Steam
  has something going. The tab's count is the queue plus Steam's. When Steam
  finishes, updates or removes something the line says so and the library is
  read again.
- **Slint, `game_jobs_ui.rs`, the installed game's page.** Update in the bar (when
  one was found, off while the game has a job), More and Less, and the line:
  "Updating: now" or "…: queued" with See downloads when the queue has a job for
  the game. The More row: Check for updates, Repair, Move… (the folder dialog;
  "is running" when it is), Uninstall (a second press, "Press again to delete
  its files", queues it), and for a Steam game Verify with Steam ("Verifying in
  Steam" while it is asked) and Uninstall with Steam ("Ask Steam again to
  uninstall" after). The details have the Updates row, with a fourth colour
  (accent, `Fact.tone` 4). The DLC card is `DlcRow`s now: Install, Install all
  (when more than one), Uninstall (a second press), or "Installed" and "Not
  installed" where Zendo cannot manage them, and its notes. **Decision:** the
  buttons and notes are for the game's own page; a store game's page lists the
  DLC and leaves them to its Install and Open in library. More, the second
  press and the DLC's second press start folded and unasked on each game
  (Iced's `GameView::default()` on `navigate`), and the queue's own
  `confirm_uninstall` goes with them.
- **Small changes in what was there**: `folder_chosen` takes a purpose (Import or
  Move, as Iced's `FolderFor`); `check_updates(ids)` asks about given games,
  not only the unasked; `queue_changed` redraws the open game's page (library
  or store) so the job line and the buttons follow the queue; the check marks
  the games and redraws before it asks.
- **Tests**: 6 in `zendo_shell::steam_installs`, 5 new in `zendo-slint` (Install
  with Steam through asked, busy, paused, installed, Stop waiting on the page and
  the card, Enter once, the tab's count and the Downloads card; what Steam does
  by itself; Update, More, the job line, the one job per game, Uninstall twice,
  Repair, Check, Move; the page opening folded; the DLC card's Install, Install
  all, Uninstall twice and the buttons off while the game has a job), 151 in
  `zendo` (Iced), 114 in `zendo-slint`, 41 in `zendo-shell`, 285 in `zendo-core`.
  Clippy is clean.
- **Checked on screen** (2026-11-15, throwaway home, no session bus, the user at
  the PC; fakes first on `PATH`: `steam` and `xdg-open` that only log, and a
  `legendary` with `info`, `install` (also `--update-only` and DLC), `repair`,
  `uninstall` and, from the middle of the run, `move`; Steam's six first
  games from the real cache, a manifest-seeded Steam root and an Epic game
  installed with a record, a DLC installed and three owned): Install with Steam
  (the fake got `steam://install/735580`; Ask Steam again, Stop waiting, See
  downloads and the tab's 1), the card on Downloads (Open Steam's downloads,
  which reached the fake `xdg-open`, and Stop waiting, after which the page
  invited to the Store again), Steam starting (a manifest with a file in
  `downloading/`: "Installing…", "Downloading, 400 B of 1000 B", the bar at
  40%), paused ("Paused in Steam"), finished (the footer said "is installed",
  Library 5 to 6, "Open in library"), then on that game's page More, Verify
  with Steam (`steam://validate/735580`, "Verifying in Steam", then the content
  log's two lines: "Steam verified the files", the button back) and Uninstall
  with Steam (`steam://uninstall/735580`, "Ask Steam again to uninstall"; the
  manifest removed: the game went, Library 6 to 5, the page closed). On the Epic
  game: UPDATE in the bar, clicked (the fake got `install --update-only`, the
  line "Updating: now", See downloads, Repair, Move… and Uninstall dimmed while
  it ran, Check for updates enabled; afterwards 2.0 in the record and the button
  gone), Check for updates ("Up to date" in the details), the DLC card (1 of 3,
  Install all, Uninstall, Install: Install all ran both DLC one after the
  other, 3 of 3; Uninstall pressed twice on one; Install on it), Repair (the
  fake got `repair`; See downloads from the game page opened Downloads with the
  job at 45%), Move… (Zendo's folder dialog, a path pasted with Ctrl+L and
  Ctrl+V, Enter: the job queued and Zendo called `legendary move` for the game
  and each DLC), More folded again on a game opened a second time, Uninstall
  pressed twice (the fake got `uninstall`, Library 5 to 4, the page closed).
  The store page again with a second Steam game: Ask Steam again (a second
  request in the fake's log), Stop waiting, Enter (a third). **The Iced window**,
  which now runs on the shared module, was launched in the same home: Install
  with Steam (the request, "Installing…" with the bar), its Downloads card with
  the bar, and Steam finishing (Library 5 to 6, the count cleared).
- **Found on screen and by a test.** A rescan started by Steam's own finish
  would have read the user's files in a test (`rescan` has no `persist` guard at
  its call sites): `steam_looked` gates it. The "checking" state of a game did
  not draw in a test because the check returned before a redraw when `persist`
  is false: it redraws first now.
- **Not checked, or not explained.** The footer's note after Steam uninstalled a
  game: the game and the count followed, but the footer's left side was empty
  afterwards; a later scan clears it (`Browser::loaded` does), and the file
  watch may have started a second one after the note's own; not confirmed, and
  Iced clears the line on a refresh too. **The first Move failed, and its
  retry did not say why:** the fake had no `move` (the engine's refusal was
  read back, the job paused with the error, Try again); after the fake learned
  `move`, one retry failed the read-back of a DLC's record and the next said
  Done with both folders empty. That was the rig: the failed attempt left the
  fake's records changed, and `move_game`'s first branch (a record already at
  the new folder) then deleted the old folder, which the failed attempt's
  rollback had already emptied of its copy. Real legendary does not leave a
  record changed by a move it refuses (Zendo reads it back and rolls the
  records back), so this is unlikely outside the rig, but that branch trusts
  the new folder to exist without looking (`downloads.rs` `move_game`, shared
  with Iced and not changed here): worth an `is_dir` check on `new`. Not done
  on screen: a real Steam client (the fake stands for it: the real dialog, the
  real manifests' flags beyond the ones used, a real download); the second
  press of "Ask Steam again to uninstall"; a Steam install by Steam itself
  while Zendo is open (only the seeded manifest); Update, Repair and the DLC on
  GOG or Amazon (Zendo's own steps, tests in `zendo_core::downloads`); a job
  held while the game runs (51b's code, unchanged); the pad; pt-PT; a light KDE
  scheme; "Check for updates" with a real store.
- **Side effects**: only the scratch home (5 MB, left in the session's folder);
  nothing opened a browser; `ydotoold` and the sockets were stopped and
  removed, and the clipboard, empty before, was cleared again.

## Item 52a, launch options and gamescope in Slint (2026-11-15)

The first slice of item 52 (sliced with the user: 52a launch options, 52b the
artwork chooser, 52c Mods, Nexus, Nexus search, the FOMOD sheet and the mods
card on Downloads).

- **The form is shared, as the queue is.** `zendo_shell::launch_options` holds
  `Sheet` (was Iced's), its lists (`ProtonChoice`, `TaskChoice`, `Pick`,
  `Sharpening`, the runtimes), `Tools::find`, `build_for`, `kept_program`,
  `number` and `digits`, and gamescope's `Fields`, `WindowKind` and lists in
  `launch_options/gamescope.rs`. The methods are the ones the two windows
  need: `Sheet::open(game, persist)` (also moves an Epic task saved by name to
  its number before `original` is copied), `build`, `changed`, `close` (true
  when the sheet may go; otherwise `confirm_close` is set), `save(persist)`,
  `set_proton`, `set_fsr_strength`, `set_gamescope`, `noted` (the "the build
  ignores" words), `keep_picked_program`, and `Saves::press` / `synced` (Upload
  and Download ask once more). With `persist` false nothing is read from the
  computer: no saved options, no `Tools::find`, no installed Protons or
  runtimes, no store lists. Iced was switched to it first: its
  `launch_options.rs` lost the model (about 400 lines) and its `gamescope.rs`
  the lists; its seven tests still go through its messages, the pure ones
  moved to the shell (9 tests there).
- **Slint, `ui/launch_options.slint` and `options_ui.rs`.** The Launch options
  button sits on the game page of any game `applies` says Zendo starts
  (`GameData.launch-options-shown`; false on a store game's page). The sheet
  is a child of the window's `FocusScope`: a card with a scrolling form and a
  footer outside the scroll, Cancel and Save, and the question (Keep editing,
  Discard) in the footer's place. Switches are flat `OptSwitch` rows in
  models by place in the form; a switch that cannot be used is dimmed and its
  click does nothing unless it is already on. The eight lists of Iced's pick
  lists are chip rows (`ChipData`, the chip's id its place in the list), so no
  menu has to draw over the scroll. Fields are `in-out` strings with an
  `edited(field, text)`; the five digit fields are filtered in Rust and
  written back only when the filter changed the text. Escape, Cancel and a
  click outside all go through `options_back`, first in `back()`'s order; with
  the sheet open Enter saves and every other key and the pad do nothing.
  Save asks `cloud_reload` and `show_game`, so the Details cloud row follows.
  Sync runs `cloud_saves::sync` on a thread; Browse runs the file dialog on a
  thread, as the Add game sheet does.
- **Checked** (2026-11-15, a throwaway home with no session bus, four seeded
  games: a GOG Windows game, an Epic one, a local `.exe` and a local native
  script; empty `mangohud`, `mangoapp`, `gamemoderun` and `gamescope` files on
  `PATH`; one empty Proton build): 150 Iced, 50 shell and 120 Slint tests
  green, clippy and fmt clean. On screen, with stills and by the options file
  (`games.toml`): the button shows on the four games and the sheet's sections
  differ right (Epic: language, Offline, the update switch; GOG: the cloud part
  and Browse; the Windows ones: Proton and its switches; native: the runtime
  list and no Proton); every switch was clicked (the NVIDIA offload, Epic's
  overlay and "without umu" are not usable here: a click left them off, and
  the file has none of them); the Proton chip, the window kind (the last one,
  borderless, is what the file has), the scaler, the filter, the sharpness,
  FSR's sharpening, the width and height (`19x20` became `1920`), the frame
  cap (`6o0` became `60`), fps, the extra gamescope words, arguments, the
  environment, before and after; Browse (the dialog's answer became
  `bin/x.exe`, relative to the game's folder); Sync now, Upload (asked, then
  ran) and Download (asked); Save with a bad environment line (the error under
  its field, the sheet open), with a width and no height (the error under the
  sizes, the sheet open and nothing written), then with a good form (the file
  has every value, the page says "Saved …'s launch options"); Cancel with
  changes (asks), Keep editing, Escape with changes (asks), Discard (the file
  untouched, then Ctrl+P worked on the page), a click outside and Escape
  with no changes (both close), Enter with no field focused (saves).
- **Second run, after a review**: the button moved to Iced's place (after Play
  and Update, before Favorite) and opened the sheet; a seeded
  `goggame-<id>.info` with a second FileTask made the GOG sheet list "Config
  (bin/cfg.exe)", the chip was clicked and the file has `task = 'bin\cfg.exe'`;
  a seeded `SteamLinuxRuntime_sniper/_v2-entry-point` made Local Linux's list
  offer "Steam Linux Runtime 3.0 (sniper)", clicked, and the file has
  `steam_runtime = "sniper"`. Then Iced's own window (`target/debug/zendo`, the
  same home): its sheet opened on the values the Slint run had saved, Cloud
  saves was switched off and Save wrote one line, `cloud_saves = false`, with
  the page saying "Saved …'s launch options". A glue test also pushes a task
  and a runtime into the form and chooses them.
- **Not done**: a real Proton run with the options; the Upload and Download
  sync against a real store (Sync now answered "start 1423049311 once first:
  its saves are found in its prefix", the engine's or Zendo's own line, whose
  cause was not looked into); the pad; pt-PT; a light KDE scheme; the "Epic
  does not approve offline play" note; Iced's sheet beyond the one switch
  (its Browse, Sync and gamescope part were not clicked after the refactor,
  only its seven tests).
- **Found on screen**: `ScrollView` does not report its content's height as its
  preferred height, so the card was 120 px tall until the form's was given as
  `preferred-height`. The second field of a size row was pushed to the card's
  edge until the row had `alignment: start`. A save that fails on a field
  scrolled out of sight (the gamescope sizes, when the user is at the bottom)
  shows nothing at the footer: Iced's sheet does the same, and it is left for
  the parity pass.
- **Check recipe**: window at 596,115, screen = still + (586, 105). Relative
  pointer moves land at about twice the distance asked on this session, so
  `zclick.sh` reads `kdotool getmouselocation` and corrects until within a
  pixel or two. Launch options at screen (992, 673) on a game's page; the
  sheet's Cancel at (1431, 934), Save (1534, 934); the wheel (`ydotool
  mousemove -w -x 0 -y -7` after a `HOVER=1` move into the form) scrolls
  about 400 px for 7. `ydotool type` mangles `-` and `=` (US layout): paste
  those through `wl-copy` and Ctrl+V. The file dialog is Zendo's child: let
  `zguard.sh` accept it only with a parent pid of Zendo's and `pick-file` in
  its command line.
- **Side effects**: only the scratch home (about 2 MB, left in the session's
  folder, with its `games.toml`); no `xdg-open` was called (the fake's log was
  never written); `ydotoold` and the sockets were stopped and removed, and
  the clipboard, empty before, was cleared again (it held the pasted path
  once after the Browse check and was cleared at the end).

## Item 52b, the artwork chooser in Slint (2026-11-15)

The second slice of item 52: Change cover and Change backdrop on a game's page.
Slint already drew the backdrop (item 49), so Iced's `backdrops.rs` needed no
port; what was missing was the two buttons, the sheet and its glue.

- **The picker is shared, as the launch options form is.** `zendo_shell::artwork`
  holds `Art` (the picker and a generation counter that lives across opens),
  `Picker`, `Effect` (`Search`, `List`, `Thumbs`, `Choose`), `Automatic`, and
  the words' helpers (`game_label`, `thumb_about`, `kind_name`). Each handler
  changes the state and returns the effect the window runs; with `persist`
  false the state changes the same and no effect comes back, and
  `covers::chosen` is not read. The counter is `Art`'s, not the picker's, so a
  late answer for a closed picker cannot match the next one (a test opens,
  closes and reopens). `needs_key` is a flag now; Iced compared the note with
  the translated words. Iced was switched first: its `artwork.rs` keeps the
  sheet, the `thumbnails` stream (which `nexus_search.rs` also uses) and the
  thumbnail handles, and maps each `Effect` to a `Task`; its one test moved to
  the shell (5 tests there) and a thin one stayed (Play does nothing under the
  picker, Escape closes it).
- **Slint, `ui/artwork.slint` and `artwork_ui.rs`.** Two small buttons under
  the cover on the installed and the store game's page (`GameData.change-art`;
  the header grew from 360 to 400 px). The sheet is a child of the window's
  `FocusScope`: Cover and Wide image chips, the search field (it takes the
  keyboard on open) and Search, the games found as chips (the chip id is the
  index in the list), the images in a scrolling `FlexboxLayout` of flat
  `ArtThumb` rows (120 by 180 for covers, 288 by 93 for wide images), Show
  more, and a footer with the status or note, Open Settings (only when the
  note asks for a key), the Use the automatic one button (only when the game
  has one chosen by hand) and Close. What is under the pointer shows as a line
  above the footer. Searches and lists run on a thread each; thumbnails on
  four threads, decoded on the worker (`covers::decode_bytes`) and put in their
  row with `set_row_data`; a new search or list sets a stop flag the workers
  read before each download, and a later page shares the flag. Choosing
  downloads on a thread, then `store_image`, the library's `set_image` and
  `show_game`; the sheet closes only when it is still that game's. Use the
  automatic one deletes the choice, puts `covers::cached` (which may be none)
  back on the store's copies through the new `StoreBrowser::put_image`, and for
  a library game says the note through `after_scan` and rescans. Open Settings
  closes the sheet and opens Settings on the Keys group. Escape, Close and a
  click outside all go through `art_back`, first in `back()`'s order; with the
  sheet open Enter searches and every other key and the pad do nothing.
- **Checked** (2026-11-15, with the user at the PC; a throwaway home with no
  session bus, four seeded desktop games with art and six Epic store games
  from the real cache; the user typed their own SteamGridDB key into Settings
  → Keys of that run, and the search, the images and the thumbnails went to
  the real SteamGridDB): 150 Iced, 55 shell and 126 Slint tests green, clippy
  and fmt clean. On screen, with stills: both buttons render; with no key the
  sheet says so and Open Settings lands on the Keys group; with the key,
  search by Enter and by the Search button, the games found as chips, a
  second chip (it lists that game's images), thumbnails filling in, the
  hover line, a cover chosen (page and footer note change, and the grid card
  after Escape), a second cover chosen in a row (one file left in
  `covers/chosen/cover`), Wide image chosen (the backdrop changed), Use the
  automatic wide image and Use the automatic cover (the seeded pictures came
  back, the chosen folders were empty, the notes said so), the Wide image and
  Cover tabs, Show more on Portal 2 (a second page joined the 50 images and
  its thumbnails filled in), Close, Escape on the sheet (the page stayed) then
  Escape again (the Library), a click outside (closed). After a click outside,
  Close and Escape, and after a choice on a store game, Escape on the page
  went back, so the page had the keyboard; that was not tried after Use the
  automatic one, a choice on a library game or Open Settings. A store
  game's page (Absolute Drift): the sheet opened on its title, a cover was
  chosen (page and store grid card changed) and Use the automatic cover put
  the seeded one back. Iced's own window (same home): the sheet opened,
  searched through the shared picker and Close closed it.
- **Found on screen**: the line under the pointer was drawn only while it had
  text, so the sheet's footer moved by a line when the pointer came and went
  and a click on Show more landed where the button had been; the line now has a
  fixed height. Search empties the list, the card shrinks and re-centres: a
  click aimed at the tall card then hit outside it and closed the sheet (take a
  still between a search and the next click).
- **Not done**: Change backdrop on a store game's page (the same callback was
  clicked on library games); pt-PT; a light KDE scheme; the pad; Iced's sheet
  beyond opening and closing it (its choose and automatic paths were not
  clicked after the refactor, only its test); a failed save on screen (a glue
  test covers it).
- **Side effects**: the scratch home, with the user's key in its `config.toml`,
  was deleted afterwards; `ydotoold` and its socket were stopped and removed;
  no `xdg-open` was called; nothing in the user's own files was written (the
  real cache was only read, to seed the home). The seeded store games' covers
  and the pictures chosen came from SteamGridDB over the network.
- **Check recipe**: window at 596,115, screen = still + (586, 105). Change
  cover is at screen (739, 601) and Change backdrop at (739, 628) on a game's
  page; the library's first card at (754, 441). With results the sheet is
  window-tall (tabs at y 204, Close at (1710, 938)); with none it is short and
  centred, so take a still before each click.

## Item 52c1, the Mods card in Slint (2026-11-15)

The first slice of item 52c (sliced with the user: 52c1 the Mods card on a
game's page, 52c2 Nexus and Nexus search, 52c3 the FOMOD installer sheet and
the mods card on Downloads). It is Iced's `app/mods.rs` (1,162 lines): the
profile row, the place line, each mod's row, a Bethesda game's plugins, the
overwrite row, the folder row, the Apply row, Add mod and Find on Nexus.

- **The card is shared, as the launch options form is.** New
  `zendo_shell::mods`: `ModsState` (what was read per game, the change in
  flight, a press that asks first, the folder and the profile name being
  typed), `Work` and `Job` (one change, for a thread), `run` (calls
  `zendo_core::mods` and builds the note), `Outcome` and `Done` (a line for
  the footer and the `After`s: `CheckUpdates`, `NextInstaller`,
  `OpenInstaller`), and every formula the view computed (`place_line`,
  `mod_facts`, `apply_state` with its `Tone`, `can_apply`, `can_restore`,
  `plugins_note`, `plugin_facts`, `plugin_problems`, `delete_label`, the
  folder and profile rules). `add_as_from_nexus` and `added_note` moved here
  too (Iced's `nexus.rs` re-exports `added_note`). Each transition takes
  `runnable`; with it false the state moves the same and no job comes back,
  so tests need no user files. Iced was switched first: its messages `Read`
  and `Added` and `Done` became one `Done(id, Box<Outcome>)`, and its two
  tests became six shell tests plus one thin Iced test; `add_mods` and
  `change_mods` stayed, as the installer and Nexus still call them.
- **Slint**: `ui/mods.slint` (`ModsData`, the flat rows `ModRow` and
  `PluginRow`), `ModsCard` in `game.slint` between the DLC and prefix cards,
  `TextField.read-only` (the folder field while mods are applied), and
  `mods_ui.rs` (a `#[path]` child of `library_ui.rs`). Profiles are chips;
  the name field takes the keyboard on `init`; Save and Cancel call
  `focus_page`. A mod's facts sit on a line of their own under its name, so
  they wrap. The card is not drawn while the mods are being read.
- **Seams left for the next slices**, marked in the code: `// 52c2:`
  (the downloads and Update buttons on the card, Search Nexus, the update
  check after a read), `// 52c3:` (Options…, which is not drawn; the slot for
  a waiting installer). An archive with a FOMOD installer is cancelled
  (`mods::cancel_install`) and the footer says so with the new string
  `mods-installer-not-yet`, to be removed in 52c3.
- **Checked** (2026-11-15, with the user at the PC; a throwaway home with no
  session bus; five seeded local games under it, two zip archives that share
  a file and one with a FOMOD installer; a fake `xdg-open`): 149 Iced, 285
  core, 60 shell and 132 Slint tests green, clippy and fmt clean. On screen,
  with stills and the files on disk: Add mod through the file dialog twice
  (the second says "1 replaced by mod-a" and the first "replaces 1 of
  mod-b"), ↑ on a mod, the box off and on (the overlap facts go with it), Apply
  (the files were in the game's folder and the later mod won the shared one),
  a box while applied (the state turned red: "Changed since applied"), Back to
  vanilla (only `game.sh` left), New (field focused, Save dimmed while empty),
  a typed name and Enter (the chip, the footer, the Delete button enabled),
  profile Delete twice ("Really delete", then the profile and its note went),
  Rename (prefilled) then Cancel then Escape straight after (the page closed,
  so the keys were alive), the folder field typed and Save (the place line,
  the target and "As Zendo knows" changed) and "As Zendo knows" back, the
  overwrite row (a file seeded in `overwrite/`) and its Delete twice (folder
  gone), a mod's Delete twice (folder gone), Find mods on Nexus (the logging
  `xdg-open` got `https://www.nexusmods.com/stardewvalley/mods/`), a game
  with no folder (no card, and nothing left from the game before it), Skyrim
  with no prefix (the plugins section, its red note, `Skyrim.esm` locked with
  "its header cannot be read"), the FOMOD archive (the footer says why, no
  `.adding-*` left). Iced's own window, same home: Add mod, Apply and Back to
  vanilla through the shared code.
- **Found on screen and fixed**: (1) after most clicks on the card the page
  jumped up by hundreds of pixels when it was scrolled low: a model reset
  (`set_vec`) made the card measure 116 px short for a moment and the scroll
  view clamped to that; the models are now filled row by row when the count is
  the same (`refill`), and the busy line has a fixed height. (2) Save stayed
  dimmed while a name was typed (only a redraw lit it); the typing handler
  sets it. A page jump that is left: removing a mod (the count changes, so
  the model is reset) moved the page by about the row's height.
- **Not fixed, seen once**: on Stardew Valley's page (a long Steam
  description, screenshots, no requirements card yet) the page would not
  scroll far enough to show the bottom 235 px of the Mods card, while the
  scroll offset was at the page's reported limit. It did not happen on a
  local game with no Steam match (Mods is the last card there, fully
  reachable), on Cat Quest, or on Stardew once the requirements card arrived
  after it. The pages' content height also flips by the Screenshots card's
  height on every rebuild, and by 99 px on KMines' page, which has no Mods
  card, so a stale page height is not specific to this card; it is listed for
  item 54's parity pass.
- **Not done**: ↓ on a mod, the plugin boxes and arrows (the only plugin
  there was locked and the list had one row), a rename's Save (only a new
  profile's), Apply while the game runs (a shell test covers the refusal), a
  failed read on screen (a glue test), Add mod cancelled in the dialog, a
  Nexus key and an archive Nexus named (`add_as_from_nexus`), pt-PT, a light
  KDE scheme, the pad.
- **Side effects**: the scratch home was deleted; `ydotoold` was stopped and
  its socket removed.

## Item 52c2a, Nexus in Slint (2026-11-15)

The second slice of item 52c, and the first half of 52c2 (sliced again with the
user: 52c2a the key, links, downloads and updates; 52c2b Search Nexus). It is
Iced's `app/nexus.rs` (993 lines) without the search.

- **Shared, as the Mods card was.** New `zendo_shell::nexus`: `NexusState`
  (the key field, whose key it is, what opens links, links that came before the
  library, the game chooser, the downloads with their stop flags, the newer
  files found per game and mod, the games checked this session), the
  transitions (`save_key`, `check_key`, `key_checked`, `link_arrived`,
  `matched`, `choose`, `begin`, `finished`, `want_updates`, `updates_found`,
  `update`), each taking `runnable` where a job would start; the workers for a
  thread (`match_games`, `fetch` with a progress callback, `ask_updates`,
  `check_key`, `become_handler`); and the words (`signed_in`, `update_label`,
  `Download::progress`). `Then` replaces Iced's `NexusThen`. Iced was switched
  first: its `nexus.rs` keeps its views and messages and calls the shell; its
  three tests became two plus seven shell tests (148 Iced, 285 core, 67 shell
  and 139 Slint tests green, clippy clean).
- **Slint**: `ui/nexus.slint` (`NexusData`, `DownloadRow`, `DownloadLine`,
  `ChooserSheet`), the Nexus Mods section in the Keys group of Settings, an
  Update button on each mod's row (`ModRow.update`), the downloads above the
  card's body (`ModsData.downloads`), and `nexus_ui.rs` (a `#[path]` child of
  `library_ui.rs`). Links: `links::listen` and the start-up link go to
  `nexus_link`; one before the first scan waits and is taken at it
  (`nexus_scanned`, which also asks whose key it is and what opens links). A
  download's progress reaches the window at most every 150 ms and rewrites its
  row (`refill`, so the card's height does not change); an archive with a FOMOD
  installer still goes to `mods_installer_not_yet` (52c3). Escape and the pad's
  Back close the chooser; `window.rs` no longer carries the `needless_pass_by_value`
  expectation for `link`.
- **Checked** (2026-11-15, in two runs with the user at the PC; a throwaway home
  with no session bus, its own `XDG_RUNTIME_DIR` (`/run/user/1000/zrun`, short
  enough for the links socket and apart from the user's own window) and
  `XDG_DATA_HOME`/`XDG_CONFIG_HOME` under it, a fake Nexus key in `config.toml`,
  five games, two of them local with a folder; the user's own Zendo was not
  running). **Clicked, with stills and the files on disk**: Check on the fake key
  (red "Nexus turned this key down"); Show (the key is read, the label becomes
  Hide); typing a key in the field and Save (the Save button lit, red "turned
  this key down, so it was not saved", `config.toml` kept the old key); the field
  emptied and Save (green "Key removed"; core writes `nexus_key = ""` for a
  removed key when the keyring cannot be asked); the key link (a logging fake
  `xdg-open` got `https://www.nexusmods.com/users/myaccount?tab=api`); "Open them
  with Zendo" (the line turned green, the footer named the binary;
  `zendo-nxm.desktop` and `mimeapps.list` were written under the scratch home
  only, and the real `mimeapps.list` kept its hash and the real
  `zendo-nxm.desktop` its time); an `nxm://` link sent by a second process to the
  open window (with only desktop games: "no installed game takes mods"; with the
  two local games: the chooser), Escape, Cancel (a still before and after), the
  first row (the download ran and failed on the fake key: the footer said so,
  and `nexus = "catquest"` was kept for that game; its page then showed Find mods
  on Nexus) and the second row (`nexus = "catquest"` kept for the second game
  only).
- **Found on screen and fixed**: the handler line sat beside its button in a row
  with no explicit width; a long foreign program path (a seeded
  `zendo-nxm.desktop`, which is what the user's real home has from 13 November)
  would be measured short. It is now a `Note` on its own row above the button,
  and the second run showed it wrapping at the card's width with the button
  enabled below it.
- **Tests added after a review** (7 Slint glue tests in all): progress moves the
  bar and words of the same row; a finished download removes the row and leaves
  the card showing the new mods; a failure and a Stop say so (a Stop is not a
  failure); Update without Premium says to press the site's button, with Premium
  it adds a download row, and a mod with nothing newer does nothing.
- **Not checked on screen**: a download's bar and Stop, Update on a mod and the
  update check (all need real answers from Nexus; the glue tests above cover the
  rows, `stop_download` and both Update paths), Save after an accepted key (a
  glue test covers it), an archive with an installer arriving from a download
  (`Added::Choose` goes to `mods_installer_not_yet`; `Pending` cannot be built
  outside `zendo-core`, so no test), a Premium account, pt-PT, the pad on the
  chooser. A page jump when the download rows come and go (52c1 recorded one
  for the mod rows) was not looked for, since no row appeared. `NexusAdult` is
  drawn only in the search sheet, so it stays with 52c2b.
- **Side effects**: the scratch home stays in the scratchpad (tmpfs); `ydotoold` was
  stopped and its socket, and the scratch runtime folder, removed.

## Item 52c2b, Search Nexus in Slint (2026-11-15)

The second half of 52c2: Iced's `app/nexus_search.rs` (868 lines), a sheet
opened from the Mods card of a game that Nexus knows by a name.

- **Shared.** New `zendo_shell::nexus_search`: `Search` (the words, the order,
  the results a page at a time, each search's number so a late answer is
  dropped, the mod whose files are open) with `restart`, `more`, `found`,
  `toggle_files`, `got_files`, `link`, `footer`; and the words and rules
  (`site_url` with the `nmm=1` flag, `have`, `offered`, `file_facts`, `byline`,
  `numbers`, `order_name`, `file_kind`, `count`). `add_from_site` and a
  `Work::AddFromSite` job moved into `zendo_shell::mods`. Iced was switched
  first: its `Search` holds the shared one (through `Deref`) plus its
  pictures, thumbnail tasks and the site window's handle; its tests are one
  plus six shell tests (146 Iced, 285 core, 73 shell, 148 Slint green, clippy
  clean).
- **Slint**: `ui/nexus_search.slint` (`SearchData`, flat `SearchRow`s of kind
  mod, file or note, `SearchSheet`), a Search Nexus button on the Mods card,
  `search_ui.rs` (a `#[path]` child of `library_ui.rs`), `Setting::NexusAdult`.
  Pictures are fetched and decoded on worker threads as the art picker's are
  (stopped on a new search or Close). The sheet is window-tall, Escape and the
  pad's Back close it, a click outside closes it, the field takes the keyboard
  and Enter searches. A free account's Get on Nexus and a mod's Page on Nexus
  open Zendo's Nexus window (`sign_in_window::open_nexus`) and what it gives
  goes to `nexus_download` (a link) or `Work::AddFromSite` (a file); Premium
  downloads straight away.
- **Checked** (2026-11-15, with the user at the PC; a throwaway home as for
  52c2a, with the game's Nexus name set to `stardewvalley` in its `mods.toml`
  and the fake key): on screen, against Nexus's real public search, with
  stills: the sheet opening from the card with 20 of 34,179 mods and their
  pictures; Files (the fake key's red error under the mod; the button becomes
  Hide files, and Hide files closes it); Most downloaded (the order changed);
  Adult mods (the count changed, `nexus_adult = true` written to the scratch
  config); typing `smapi` in the field and Enter (36 mods); scrolling to the
  bottom and Show more ("36 of 36 mods", the scroll position held); Close
  with the button; Escape (the sheet closed and the game's page stayed).
- **Found on screen and fixed**: the "Without Premium…" line ran past the
  card's padding (`parent.width` of an element in a layout includes the
  layout's padding); it now has `parent.width - 2 * Space.xl` and wraps inside.
- **A slip of mine**: one click was aimed at still coordinates, outside the
  card, which closed the sheet; the typed letters and Enter then went to the
  game's page and started the seeded `exit 0` game once (its "started from
  Zendo 1 time" is in the scratch home only).
- **Not checked on screen**: Page on Nexus and Get on Nexus (they open Nexus's
  site in a window of Zendo's, which needs the user's login), a file list from
  a real key, the Download button for Premium, what Nexus's window gives (a
  link, a file), the "In your mods" mark on a real result (a glue test covers
  it), pt-PT, the pad. The glue tests (8) cover the rows, the late answer, the
  file lists and their failure, the files already in the mods, the free and
  Premium download, the adult switch and Escape and Close.

## Item 52c3, the FOMOD installer sheet in Slint (2026-11-15)

The last slice of item 52: Iced's `app/installer.rs` (578 lines) in Slint, with
Options… on a mod, the slot for an installer that has to wait, and the mods
coming in from Nexus as a card on Downloads. Until now Slint dropped an archive
with an installer and said so (`mods-installer-not-yet`).

- **Shared.** New `zendo_shell::installer`: `InstallerState` holds the open
  sheet and the installers that wait, and is the whole state machine:
  `open` (says `Waits`, `Shown(status)` or `InstallNow` when no step is
  visible), `next_waiting` (only when no sheet is open and mods are free, and
  in order), `pick` (a radio clicked again clears a group that may have none),
  `back`, `advance` (on the last step it closes the sheet and hands the mod
  over), `focus`, `pick_focused`, `activate` (Enter: pick the focused option,
  else go on), `cancel`, and `view`, which flattens the sheet for drawing
  (title, the step line, the warning, groups with options that say `single`,
  `on`, `enabled`, `dim`, `focused`, the picture and description). New
  `Work::Install` and `ModsState::install` in `zendo_shell::mods`; Iced's two
  `change_mods` closures and `change_mods` itself are gone. Iced was switched
  first: its sheet draws from `View`, and its two tests became nine shell tests
  (and one for `ModsState::install`) plus one thin Iced test.
- **One change of behaviour, in both windows**: a finished add
  (`Added::Done`) now asks for the next waiting installer too. Before, an
  installer that waited behind an add stayed waiting until some other change
  ended.
- **Slint**: `ui/installer.slint` (`InstallerData`, flat `InstallerRow`s of
  kind group, radio or box, `InstallerSheet`), `installer_ui.rs` (a `#[path]`
  child of `library_ui.rs`, methods prefixed `installer_*`), `ModRow.options`
  and `ModsData.options` (the Options… button, which runs the mod's kept
  installer again through `ModsState::reinstall`), and on Downloads
  `DownloadsData.mods-show`, `mods-rows` and `stop-mod` (every Nexus download,
  not only the open game's, drawn with `DownloadLine`; the page's invitation to
  the Store is off while one is coming in; the tab's count stays without them,
  as in Iced). The sheet is the last child of the window's `FocusScope`, so it
  draws over the search sheet and the chooser; it takes every key (arrows move
  the cursor, Enter picks or goes on, Right is Next, Left is Back, Escape
  cancels, letters and Ctrl shortcuts do nothing) and the pad's presses the
  same way. A click outside it does nothing: closing drops the unpacked mod, so
  only Cancel and Escape close it. The picture of the option described is read
  and decoded on a thread under a generation number, so a late one does not land
  on another option. `mods_installer_not_yet` and its string are removed.
- **Checked** (2026-11-15, with the user at the PC; a throwaway home with no
  session bus, a seeded local game titled Stardew Valley and a zip with a
  FOMOD config of two steps, the second shown only for the Strong body): 145
  Iced, 285 core, 83 shell and 158 Slint tests green, clippy and fmt clean. On
  screen, with stills and the files on disk: Add mod through the file dialog
  opened the sheet; each control by name: the Slim and Strong radios (choosing
  Slim hid step 2: "Step 1 of 1" and the button became Install), Maybe one,
  Maybe one's group radio cleared by a second click, Zz extra, the unusable
  option (no change), the picture and description following the last click
  (the mod's own picture when the option has none), Next (the required Big
  was ticked), Back (the choices were kept), Small, Install by click (the mod
  was built: `core.txt`, `body/strong.txt`, `docs/z.txt`, `maybe.txt`,
  `big.txt`, `small.txt`; the card said "6 files" and the footer "Installed
  fancy"); Options… (the earlier choices ticked), Down, Down, Enter (Zz extra
  off), Right (step 2, Small still ticked), Enter as Install (`docs/z.txt`
  gone, "5 files", `mods.toml` lists the four choices); a letter, Ctrl+3 and a
  click on the dimmed backdrop (the sheet and the page did not change); Escape
  ("fancy was not installed", the page stayed, the files stayed), then Ctrl+3
  straight after (the Downloads page: the keys were alive); a second add and
  Cancel by click (the `files/.adding-fancy-2` folder was there while the sheet
  was open and gone after). Iced's sheet was opened once through Options… and
  closed with Cancel (it drew as before the refactor).
- **Not seen on screen** (covered by glue or shell tests, which say what they
  pin): an installer waiting behind another or behind a mods change and
  opening when the slot is free; the Downloads card with mods coming in from
  Nexus, and Stop there (needs a real Nexus download); the pad on the sheet
  (no controller; the test drives `installer_pad`); pt-PT; a light KDE scheme;
  an installer with no visible step installing at once; a picture that comes
  late for another option.
- **Things to know**: with nothing touched the cursor counts as on the first
  option, so the first Down lands on the second (Iced does the same, and a
  shell test pins it). The page behind the sheet moved by about 150 px when
  Options… opened (the Mods card's busy line; the known page-height flip of
  52c1). A small picture is scaled up to 220 px in Slint and drawn at its own
  size in Iced.

## Item 53a, the Stats page in Slint (2026-11-15)

Item 53 is sliced in two [firm, my call, said in chat before building]: 53a is
the Stats page and 53b is Game Mode. The user chose to check 53a on screen at
the PC.

- **Built**: `ui/stats.slint` (`StatsPage`, `StatsData` with `StatBar`,
  `StatCol` and `StatLine`) and `src/stats_ui.rs` (`StatsState`, a part of the
  Library's glue like the other pages). Nothing new in `zendo-core` or
  `zendo-shell`: the sums are `sessions::Summary`, the words are the shared
  `stats-*` messages and `format::{play_time, month_short, weekday_short,
  play_status}`, which Iced already used. The page keeps no sessions of its
  own: it reads the list `GameState` keeps (its field is now `pub(super)`), so
  a session recorded while the page is open redraws it (`record_session`).
  Parts: the period chips (the years with sessions, latest first, then all
  time), four tiles, Most played with bars, By month and By weekday as column
  charts (value on top, label under; "55m" under an hour, then whole hours),
  Highlights (games started, longest streak, longest session) and Library by
  status. No session at all gives the explaining text; a period with none gives
  "No sessions in this period". Left, Right, Home and End (and the pad's Left
  and Right) change the period on that page only; Ctrl+7 and the tab were there
  since item 47.
- **Checked**: `cargo fmt`, clippy with no warning, 162 tests (4 new: the
  column values, the page's empty and filled states with the models' row
  counts, the arrows on Stats only through `key()`, and the keys, Home and End
  and a chip click moving the period).
- **On screen** (KDE, 2026-11-15, with the user at the PC, throwaway home
  `/tmp/zh`, no session bus, a seeded `sessions.jsonl` of ten sessions over
  2025 and 2026, `ydotoold` with the pid guard before every key and click):
  every figure was compared by hand with the file: 2026 is 584 min (9 h 44 min)
  over 7 sessions on 7 days and 3 games, KPat 4 h 49 min, Sleeper 3 h 50 min,
  Other Game 1 h 5 min; 2025 is 230 min over 3 sessions, 3 days, 2 games, with
  Thursday, Friday and Wednesday as the played weekdays (1 and 2 May and 31
  December 2025); all time is 13 h 34 min over 10 sessions, a streak of 2 days
  and a longest session of 3 h 20 min on 2026-07-04. Clicked once each: the
  2026, 2025 and ALL TIME chips. Keys: Right twice (stops at ALL TIME), End,
  Left, Home, Ctrl+1 to the Library and Ctrl+7 back (the period was kept). The
  wheel scrolled to Highlights and Library by status. A real run: a local game
  (`sleep 70` in a script) played from its page; with Stats open when it ended
  the page went to 8 sessions, 9 h 45 min and 4 games with no restart, and the
  line was in `sessions.jsonl`. The empty page, the "no sessions in this
  period" text and the whole page in pt-PT were looked at too.
- **Found and fixed on screen**: the year chips sat at the window's left edge
  while the cards were a centred column; they are in the column now. The "Library
  by status" label was a dim 12 px and nearly invisible on the translucent
  window; it is 13 px in the foreground colour.
- **Not checked**: a real controller (the pad's Left and Right share the
  keys' code and no device is here), a light KDE scheme, a window narrower than
  1100 px, a library with statuses set (the four tiles read 0 in the check; the
  count is `Game.status`, tested only by its row count), and a game a launcher
  runs. Known and unchanged from Iced: quit Zendo while a game runs and that
  session is lost.
- **Things to know**: the tiles' widths follow their content, so they differ a
  little between periods (Iced's are equal). A `cargo test` or `clippy` does not
  rebuild `target/debug/zendo-slint`: the first still showed the old placeholder
  until `cargo build -p zendo-slint`. The seeded `.desktop` files in
  `$HOME/.local/share/applications` were not what the library listed (it showed
  the system's KDE games), so a played game has to be a local one
  (`~/.config/zendo/local.toml`).

## Item 53b, Game Mode in Slint (2026-11-15)

Closes item 53. The toolkit-free half is `zendo_shell::game_mode` (the shelves
Continue playing and All games, the hero's index, one tile along, shelf
cycling past an empty one, the Start menu and its three choices, the
description rule, the carousel's tile count and first tile, `played_games`
and `meta_line`); the Slint side is `ui/game_mode.slint` (`GameModeScreen`,
`GameModeData`) and `src/game_mode_ui.rs`. Iced's `app/game_mode.rs` and
`hero.rs` were not switched to the shared module: they go at the cut-over
(item 54), so the rules exist twice until then.

- **Behaviour**: F11 and the pad's Select toggle Game Mode from any page: on,
  the window goes full screen on the Library and shows the screen; off, it
  returns to the page it was left from. The screen is drawn over the window
  (the top bar and pages are hidden, not removed) while Game Mode is on, the
  page is the Library, no game's page is open and the grid was not asked for.
  Left and Right move the hero, Enter or A plays it, Tab or LB/RB/Y change the
  shelf, Escape or B opens the Start menu (Back to Game Mode, Library, Leave
  Game Mode; Up and Down choose, Enter confirms, Escape closes), Details opens
  the hero's own page and Escape there comes back to the screen. Library in the
  menu shows the grid with a GAME MODE chip in the top bar; the chip returns to
  the screen. Typed text and other keys never reach the grid behind the
  screen. `--game-mode` (and `--fullscreen`) start in it. The hero's
  description is the store's or Steam's summary, asked once when a game takes
  the hero (`want_known_facts`), else the developer and platform.
- **Changed in shared code**: `play` in `game_ui.rs` is split into `play` and
  `play_record(game)`; `want_known_facts` and `facts_of` are `pub(super)`;
  `show_game` and the cover timer redraw the screen; `install` takes the
  `--game-mode` flag.
- **Not the same as Iced, on purpose**: after Play the hero follows the game
  played. A launch moves it to the front of Continue playing, and Iced's hero
  stayed on the same place, so it jumped to another game (seen on screen in
  Slint first).
- **Checked**: `cargo fmt`, clippy with no warning, tests green (91 in
  `zendo-shell`, 166 in `zendo-slint`). New: 8 shell tests (played order and
  hidden games, the meta line, the description, an odd tile count, tiles held
  inside the shelf, stepping to the ends, shelf cycling, the menu) and 4 glue
  tests (F11 in and out through the Store, the arrows and Enter counting only
  the hero's launch and the hero following it, the menu and its choices, Tab
  and Details with Escape).
- **On screen** (KDE, 2026-11-15, with the user at the PC, throwaway home
  `/tmp/zh`, no session bus, eight local games with real covers copied from the
  cache and five of them played, `ydotoold` and the pid guard before every
  key and click): F11 went full screen (2560x1080) into the screen with the
  hero, the five played games in the carousel and the hints; Right twice moved
  the outline to the third tile and the hero to Gamma Gate; Tab showed All
  games (11 tiles); Escape opened the menu; Down and Enter on Library showed
  the grid with the chip. Clicked once each: the GAME MODE chip, the Continue
  playing chip, a carousel tile, Menu, the dim area outside the menu (closes
  it), Play (a launch was counted; the hero stayed on the game after the fix),
  Details (the page opened over the screen) and Escape back, the menu's
  Library and Leave Game Mode choices (Leave returned to the 1368x892 window on
  the Library). A second run with the new build: `--game-mode` started in it, in
  pt-PT (JOGAR, DETALHES, TODOS OS JOGOS), then F11 out, Ctrl+2 to the Store,
  F11 in and F11 out back to the Store.
- **Not checked**: a real controller (no device; the pad's code is
  `game_mode_pad`, only the key path is tested), All games chip and Back to
  Game Mode by click, a game with a Steam summary for the hero's description
  (the seeded games showed "linux", the platform line), a light KDE scheme,
  a window narrower than the carousel, and Game Mode with a library of
  thousands (the carousel draws at most as many tiles as fit).
- **Things to know**: a window started with `--game-mode` has no windowed
  size to go back to, so leaving it opened at 1108x632, not the default
  1360x860. The top bar and pages are hidden with `visible: false`, not
  removed, so their state survives a visit to the screen. With 11 games all 11
  tiles showed at 2560 px; a narrower window shows an odd number around the
  hero.

## Item 54a, the parity checklist and its fixes (2026-11-15)

`docs/parity-checklist.md` is the gate before the Iced crate goes. Every
Fluent message id (996) was looked up in the Iced crate and in the rest: 11
were Iced's only. Of those, `about-*` and `run-open` needed nothing, and the
rest were ported to `zendo-slint`: the "UPDATE AVAILABLE" badge on the game
page (`update-badge`), the note when an open game or store game leaves its list
(the page closes and the footer says it), one line per unreadable source in
Settings → Library (`Browser` keeps the lines, not a count), "No images to
download" after Download now, the panic's own words (`panicked_note`), and the
Stats page's key hint, which was missing. "Reading mods…" was not ported on
purpose: a card that grows after the page is laid out left the page's height
short (52c1).

Checked: `cargo test -p zendo-slint` 168 passed, rustfmt and clippy clean. On
screen, in two throwaway homes with keys and clicks guarded by the window's
pid: the Stats footer, the game-gone note, the unreadable file in the footer
and in Settings (red, with path and reason) and "No images to download". Not
seen: the update badge, the store game's gone note, the panic text. Page keys:
Ctrl+3 Downloads, 4 Accounts, 5 Settings, 6 About, 7 Stats. On Settings a click
on the scrollbar track and Page Down did not scroll; dragging the thumb did.
A helper that read `kdotool getmouselocation --shell` clobbered its own `X` and
`Y` (the output names are `X`, `Y`, `WINDOW`); it aborted on the pid check
before any click.

## Item 54b, the cut-over from Iced to Slint (2026-11-15)

The user signed off `docs/parity-checklist.md`, and the Iced window went.

- `crates/zendo/src` (`app.rs`, `app/`) and its `Cargo.toml` were deleted; the
  Slint crate's `src/`, `ui/`, `build.rs` and `Cargo.toml` moved into
  `crates/zendo` (git sees renames), next to the `fonts/`, `icons/` and
  `themes/` folders that were already there. The package is named `zendo`; the
  asset paths became `../fonts`, `../icons` and `../themes`. `iced` left the
  workspace dependencies (the only one nothing else used); the comments that
  named it were reworded.
- `zendo-shell`'s test that every message is used failed once, naming four
  messages only Iced used (`about-built-iced`, `about-licence-text`,
  `mods-reading`, `run-open`); they were removed from both `.ftl` files.
- `packaging/fedora/zendo.spec`: licence `MIT AND OFL-1.1 AND GPL-3.0-only`;
  `vulkan-loader` replaced by `libglvnd-egl`, `libglvnd-glx` and `fontconfig`,
  and `libwayland-egl`, `libX11`, `libXcursor` and `libXi` added. The list
  came from the libraries the built binary loads (`grep -a` for `lib*.so` in
  it: EGL, GL, Wayland, xkbcommon, fontconfig, X11, no Vulkan) and each name
  was looked up with `rpm -qf` on this Fedora. `packaging/arch/zendo-git/PKGBUILD`:
  the same change with Arch's package names (`libglvnd`, `fontconfig`, `libx11`,
  `libxcursor`, `libxi`, `libxkbcommon-x11`), written from knowledge, not
  checked on Arch.
- README (badge, layout table, credits, licence), `PLAN.md` (where it stands,
  stack), `CLAUDE.md`, `docs/backlog.md` and the module comments in
  `zendo-shell` and `zendo` were brought up to date.

Checked: `cargo build`, `cargo test --workspace` (168, 285 and 91 passed,
25 ignored), `cargo clippy --workspace --all-targets` clean. `cargo fmt
--check` still reports one old diff in `zendo-core/src/achievements.rs`, which
this item did not touch. A launch after the rename (throwaway home) showed the
font, icons and covers. Not done: an RPM mock build and the COPR resubmit (the
user's).

### Item 54b, the RPM mock build (2026-11-15)

`make -f .copr/Makefile build` made the SRPM from HEAD (the spec is taken
from the working tree, the sources from the commit), then `mock -r
fedora-45-x86_64` with no network. First run: `%build` passed, `%check` was
killed: `rustc` (SIGKILL, the kernel's OOM kill, 17 GB resident) while building
the window's test target with Fedora's flags `-Cdebuginfo=2
-Ccodegen-units=1`; Slint's generated code is one large unit. The spec now sets
`%global rustflags_debuginfo 1` and `%global rustflags_codegen_units 16`. Second
run: passes, 168, 285 and 91 tests, `desktop-file-validate`, an RPM with
licence `MIT AND OFL-1.1 AND GPL-3.0-only`, `/usr/bin/zendo`, the desktop file,
the nine icons, `LICENSE`, `LICENSE.dependencies` and `OFL.txt`, and
`Requires` for `libglvnd-egl`, `libglvnd-glx`, `fontconfig`, `libwayland-egl`,
`libX11`, `libXcursor`, `libXi`, `libxkbcommon(-x11)`. A mock option
`--enable-network=false` is not valid (the flag takes no value; network is off
by default). Not known: how much memory COPR's builders have; the lighter
flags were only tested on this machine (31 GB).

## The move to CachyOS, 2026-11-16

The development machine moved from Fedora 45 + KDE Plasma to CachyOS (Arch)
with Hyprland (Wayland). Zendo's window is distro-agnostic (item 27), so it
needs no code change; this session brought the documents up to date.

- Toolchain: CachyOS' Rust is 1.100.0, managed by rustup (`/usr/bin/cargo`
  and `/usr/bin/rustc` are rustup symlinks; `stable` is the default, and
  1.99.1 is also installed). `mise ls` reports the `mise.toml` pin of 1.99.1
  as "missing" because mise does not manage rustup's toolchains; builds use
  rustup's `stable`, which is fine (the minimum is 1.98).
- System libraries: `gtk3`, `webkit2gtk-4.1`, `libsoup3` and
  `javascriptcoregtk-4.1` are present, with `gcc`, `make`, `cmake` and
  `pkg-config`; `cargo check --workspace` finished clean (3m57s).
- Tools: `umu-run`, `gamescope`, `mangohud`, `steam`, `heroic`, `python3`,
  `tar`, `xz`, `7z`, `bsdtar` are on `PATH`. The standalone store engines
  (legendary, gogdl, nile, comet) are not, but Heroic bundles all four at
  `/usr/lib/heroic-games-launcher/app.asar.unpacked/build/bin/x64/linux/`,
  which Zendo already uses as its fallback. `gamemode` is not installed.
- Hyprland 0.57.2. For checking: `grim` and `hyprctl` are present; `wtype`
  is not (install with `sudo pacman -S wtype`). The KDE-side checking tools
  (`spectacle`, `kdotool`, `ydotool`) are not installed here.
- Documents: README, `PLAN.md` and `CLAUDE.md` now frame CachyOS + Hyprland
  as the development machine while keeping Fedora + KDE Plasma supported,
  and keep the repo distro-agnostic. Both distro packages (Arch PKGBUILD,
  Fedora COPR spec) build with the distribution's toolchain; the `mise.toml`
  pin is only for contributors.

Install: from the Arch PKGBUILD, `cd packaging/arch/zendo-git && makepkg -si`
(`--nocheck` skips the test suite). Not done yet: a release tag, an AUR
release package and a COPR build from the tag (`docs/backlog.md`).

## Item 55a, the layout fixes (2026-11-16)

The five fixes from the user's testing of 2026-11-16 (`docs/backlog.md`,
"From the user's testing"). All in `crates/zendo/ui/`.

- **The empty page's footer**: `LibraryPage` and `StorePage` had no
  `vertical-stretch` in `main.slint` (the other pages do), so with nothing to
  show the page took only its content's height and the footer floated under
  the message. Both now stretch, and each page's empty-state `Rectangle` does
  too, so the message centres and the footer stays at the bottom.
- **The view chips**: All, Favorites, Recent and Hidden sat after the tabs;
  `docs/design.md` puts them beside the search. They now draw in the right
  cluster, immediately left of the search field, on the Library only.
- **The sort and organize labels**: the labels were 12px `Text` in a
  `FlexboxLayout` with `cross-axis-alignment: start`, so they sat at the top
  of the taller 13px chips. The rows now use `center`.
- **The Store's sort chips**: they had a row of their own; they now share the
  platform chips' row, as the Library's do.
- **The Technical card's `+`/`−`**: it was a small button under the heading.
  `Card` (in `game.slint`) now takes an optional `action-label`, drawn on the
  heading row and right-aligned, and the heading itself presses it. The
  Technical card wires it to `toggle-technical`.

Checked on screen in a throwaway home with no session bus (2026-11-16): an
empty Library and Store (footer at the bottom, message centred, view chips
beside the search, labels aligned, Store sorts in the platform row), and a
seeded local game (`local.toml`, `program = "/bin/true"`) whose page shows
the Technical card with `+` at the right of the heading (the page was forced
to its bottom for the look, then the force was removed). `cargo build -p
zendo` and 168 `zendo` tests green.

## Item 55b, the tray (2026-11-16)

The two tray fixes from the user's testing (`docs/backlog.md`, "From the
user's testing").

- **The window hides only when the game's window is up.** Before, a game
  Zendo starts itself (`Watch::Group`) hid the window on `Started`, as soon
  as its process existed, before its window, its prefix or its splash: the
  Play button's change and an early failure happened out of sight. Now
  `zendo_core::windows::any_mapped` (the same X11 `_NET_WM_PID` walk Stop's
  close request uses) and `zendo_core::launch::any_window_mapped` say when a
  mapped window of the game's processes exists, and `running.rs`'s `follow`
  sends a new `RunMessage::Shown` then, which `Running::apply` turns into
  `Effect::HideOnLaunch`. A game watched by its folder (`Seen`) hides the
  same way. A native Wayland game has no X11 window Zendo can see, so after
  ten seconds (`WINDOW_WAIT`) Zendo hides anyway. A game that ends before its
  window appears never sends `Shown`, so the window stays and its failure is
  seen.
- **The window comes back when the game ends.** `Effect::ShowOnClose` is new,
  emitted by `ended`; `game_ui.rs` calls the new `window::show_after_game`,
  which shows the window only when it was hidden for the launch
  (`HID_FOR_GAME`), never one the user hid.

Tests: the shell's transitions (Started no longer hides; `Shown` does; Ended
shows). `cargo test` green (168 zendo, 285 core, 91 shell); `cargo build
-p zendo` clean. Not seen on screen: a real game opening and closing with the
tray on (needs the session bus and a game that opens a window).

## Item 55c, the game page's scroll (2026-11-16, fixed 2026-11-17)

The user's report: on a game's page the scroll is sometimes longer and
sometimes shorter than the content, and the bottom can be out of reach. It is
the known item 52c1 note ("the pages' content height flips on every rebuild").

- **Tried.** Like the other scrolling pages (Settings, Downloads, Stats), the
  game page's content now has an explicit width (`parent.width - 14px`), the
  `ScrollView` an explicit size, and `inner` counts the 14px bar. A `changed
  content-height` handler re-clamps `content-y`, so a card that shrinks does
  not leave the scroll past the content.
- **Measured** (a throwaway home, a seeded local game, the page forced to its
  bottom, a temporary debug line): the page reports a content height of
  1323 px against 767 px visible, while what it draws ends about 1235 px down,
  so the measured height is about 88 px too tall and the scroll runs past the
  content. The explicit width did not change that number. The suspect is a
  card's preferred height (the Mods or Details card) measured taller than it
  renders, most likely a `Text` with `wrap` measured at another width.
- **Reproduced with a real page** (a local game given Steam app 620, so the
  page loaded Portal 2's About, screenshots, Mods and System Requirements,
  driven with `ydotool` and `wtype`): the page stops at the top of System
  Requirements and cannot scroll further, while more cards (the rest of the
  requirements, the achievements) are below. The content height is the stale
  one from before the cards arrived: it does not grow with them. That is the
  "too little scroll" half.
- **The cause, found 2026-11-17.** Both halves are the same measurement, and it
  is the wrapped text's: a card's text is measured at the width the layout
  hands it *during the layout pass*, not at the width the page is drawn at. A
  width assigned inside that pass does not dirty the page's cached sizes above
  it, so the page keeps the first figure. Measured headless on a real
  `MainWindow` (the testing backend, `mock_elapsed_time` for the tree pass, the
  page opened with a game and an About card): the About text measured 2 lines
  at a 100 px card, while it drew 11 lines at 341 px; the page reported
  1047 px while the cards ran to 1127 px. Slint's own content height made the
  other half: it is the layout's *minimum*, measured the same way, so it can
  also be too tall.
- **Fixed** in `crates/zendo/ui/game.slint`, two changes:
  1. The two columns' widths are properties (`wide-column`, `narrow-column`),
     and every card in them takes its column's width as a binding, so a
     wrapped text is measured at the width it is drawn at and the change
     reaches the page once the window has its width.
  2. The scroll's `content-height` is `max(scroll-content.preferred-height,
     sv.visible-height)`, not the ScrollView's own figure: the page is laid
     out at exactly the height the scroll uses, so the two always match, and a
     card that arrives later still changes it.
- **Measured after the fix** (the same headless page): 1151 px against the
  cards' 1127 px plus the page's 24 px bottom padding, and with a
  400-sentence About 3572 px, the page and the scroll equal. Removing only the
  About card's `width` binding puts it back to 1047 px, so the width binding is
  what fixes it. `cargo test --workspace` green (169 zendo, 296 core, 91
  shell).
- **Checked by the user on screen** (2026-11-17, `cargo run --release` in the
  project, the pages gone through by hand): all better. Not tried: a narrow
  window, pt-PT and a light KDE scheme.

## Items 56a to 56c, the improvements (2026-11-16)

The three improvements from the user's testing (`docs/backlog.md`, "From the
user's testing").

- **56a, the screenshot lightbox.** A screenshot on a game page opened its
  full picture in the browser. `steam_facts::Shot` now keeps the full-size
  file as well as the small one (`shots_of` downloads both, and a full one
  that fails falls back to the small one); `cards_ui` loads both through the
  covers (the full one at the wide 960 px) and puts them in the page's images;
  and `ui/shots.slint` (`ShotSheet`) draws one full picture over the page with
  PREVIOUS, a "n / N" count, NEXT and CLOSE. `GameData.shot` is the open index,
  driven by `open-shot`, `close-shot` and `step-shot` (stepping wraps);
  Escape closes it (`shots_back`), the page's other keys do not reach the game
  while it is open (`shot_open`), and opening or closing a game resets it. New
  words `shots-previous` and `shots-next` in both languages.
- **56b, the Store's platform badge.** `CardData` has `platform` (the Library
  leaves it empty; the Store sets the store's name), and a Store card draws it
  in the top left, where the Library has its mark box.
- **56c, the wide art's fade.** The game page's backdrop was a flat
  `opacity: 0.22`. It is now 0.30 under a vertical gradient that fades the
  page's background in at the top edge and out towards the bottom
  (`@linear-gradient(180deg, ...)`), as omakade's game page does.

Checked (2026-11-16, a throwaway home with no session bus, a local game given
Steam app 620 in `state.json` so the page asked Steam's store): the lightbox
with the first of Portal 2's eight screenshots (full size, "1 / 8",
PREVIOUS, NEXT, CLOSE), then driven by hand with `ydotool` and `wtype`: a
click on a screenshot opened it, NEXT went to "2 / 8", PREVIOUS back to
"1 / 8", a second PREVIOUS wrapped to "8 / 8", and CLOSE and Escape closed
it; the gradient's direction with a temporary red-to-blue fill (red at the
top, blue at the bottom); the fade itself with the first screenshot as the
backdrop; and the platform badge with a temporary "ADDED" on a Library card.
`cargo build -p zendo` and 168 zendo, 285 core and 91 shell tests green. Not
seen: a Store card's own badge (no signed-in store here) and pt-PT.

## Item 57a, the Battle.net import and launch (2026-11-16)

The read-only half of item 57: the games the Battle.net client installed in a
Wine, Proton or Bottles prefix, launched through that prefix's client.

- **`crates/zendo-core/src/battlenet.rs`**: `prefixes()` looks in `~/.wine`,
  `$XDG_DATA_HOME/wineprefixes` and `~/.local/share/wineprefixes`,
  `$XDG_DATA_HOME/bottles/bottles`, the Bottles Flatpak's, `~/Games`, and every
  `steamapps/compatdata/<app>/pfx` under Steam's libraries, keeping only the
  ones with a `product.db`. The database is read with a small protobuf wire
  reader: field 1 a product record, field 2 its code, field 3.1 the Windows
  install path, field 4.1.1/4.1.2 installed and playable; other fields are
  skipped. The tools `agent` and `bna`, uninstalled and unplayable products,
  and records without an install path are left out. The id is
  `battlenet:<code>@<8 hex>` (FNV-1a of the canonical prefix), so the same
  product in two prefixes is two games. `CATALOGUE` names the products Zendo
  knows and the launch code where it differs (`wow` → `WoW`, `pro` →
  `Pro`, `fenris` → `Fen`, `w3tft` → `Warcraft III`, …); the rest take their
  own code. Last played comes from `Battle.net.config` (FILETIME, ms or s).
  Artwork in the game's own folder comes first; SteamGridDB fills in the rest
  through the existing covers fetch.
- **`zendo-core/src/lib.rs`**: `Source::BattleNet` (eight sources now),
  `Launch::Prefix { prefix, runner, bottle, program, args }` and
  `WineRunner { Wine, Proton, Bottles }`, with `launcher()` and
  `command_line()` saying what runs the client; `Library::scan` adds the
  Battle.net scan after Heroic's.
- **`zendo-core/src/launch.rs`**: `in_prefix` builds the command: `wine
  <client> --exec=launch <code>` with `WINEPREFIX`; `umu-run <client> …` with
  `WINEPREFIX`, `GAMEID=umu-battlenet`, `STORE=battlenet` and the chosen or
  default Proton; `bottles-cli run -b <bottle> -e <client> -- …` for a native
  Bottles prefix. The command starts in the client's folder, the launch
  options' wrappers and environment still apply, and `prefix()` returns the
  explicit prefix for a `Launch::Prefix`.
- **`zendo/src/game_ui.rs`**: a Battle.net game's Technical card shows its
  prefix and last log. `report.rs` and `settings_ui.rs` list Battle.net among
  the sources.

Checked: `cargo test --workspace` green (168 zendo, 295 core, 91 shell; the 10
new core tests cover the wire reader, the tool and absent products, the
catalogue, Windows paths, the timestamps, the config, the runner and the
game). Clippy reports nothing in the new code; the pre-existing
`map_unwrap_or` notes and the `achievements.rs` fmt diff are the toolchain's
1.100, left alone. In a throwaway home with a generated `product.db`:
`zendo list` showed World of Warcraft Classic and Overwatch 2 from a Wine
prefix and Diablo IV from a Steam Proton prefix, with the `agent` tool left
out, the ids `battlenet:<code>@<8 hex>` and last played read from the config;
`zendo launch` with a fake `wine` got `wine <client> --exec=launch Pro`,
`WINEPREFIX=<prefix>` and the client's folder as the working directory; with a
fake `umu-run` it got `umu-run <client> --exec=launch Fen`, the prefix,
`GAMEID=umu-battlenet`, `STORE=battlenet` and `PROTONPATH=<the default>`.
Not seen: a real Battle.net client and a real installation, a Bottles prefix,
the window on screen, Play/Stop and the folder watch with a real game,
SteamGridDB covers for a Battle.net title, and pt-PT.

Things to know: the watch is the game's install folder when it exists, else
the client's process group, so a client left open after the game ends does not
count as the game still running. `in_prefix` runs native Bottles only (the
Flatpak's `bottles-cli` is not on `PATH`). The installer and the Accounts
card are 57b, not built.

## Item 57b, the installer and the Accounts card (2026-11-16)

The second half of item 57: Zendo brings Blizzard's installer, runs it in a
prefix of its own, and opens the client for the sign-in.

- **`zendo-core/src/battlenet.rs`**: `managed_prefix()` is the games folder's
  `Prefixes/Battle.net`, and `prefixes()` now includes it, so 57a imports the
  games installed there. `installer_path()` is
  `$XDG_CACHE_HOME/zendo/battlenet/Battle.net-Setup.exe`; `download_installer`
  brings it from
  `us.battle.net/download/getInstallerForGame?os=win&locale=enUS&version=LIVE&gameProgram=BATTLENET_APP`
  (HTTPS only: Blizzard publishes no checksum). `run_installer` runs it in the
  managed prefix with `wine` and writes its output to
  `$XDG_STATE_HOME/zendo/logs/battlenet-installer.log`; `open_client` runs the
  client found (Zendo's prefix first, then any other) with the runner its
  prefix asks for, and reaps it. `installed()` and `count()` feed both pages.
- **Settings**: a new group, **Launchers** (the chip reads LAUNCHERS; it is not
  called Sources because the Library group already has a Sources section), in
  `ui/settings.slint` and the new `src/sources_ui.rs`. Its one row is
  Battle.net: Not installed → **Download installer** (with the download bar
  and Stop) → **Run installer** → Installed (n games) and **Open Battle.net**.
  When the installer's window closes, the row is looked at again and the
  library rescanned, so the games appear without a manual Refresh.
- **Accounts**: a Battle.net card under the three stores
  (`LauncherCardData`, `LauncherCard` in `ui/accounts.slint`,
  `accounts_ui.rs`): the badge, the prefix, Not installed / Installed / n
  games, **Open Battle.net**, and the line that the sign-in happens in the
  client. `check_accounts` looks at the prefix when the page opens.
- New messages in both languages (`settings-group-launchers`,
  `launchers-*`, `battlenet-*`); `settings_ui.rs`'s group constants shifted by
  one for the new group.

Checked: `cargo test --workspace` green (169 zendo, 296 core, 91 shell). New:
the client in a prefix, the launcher row and the Accounts card in the window
tests, and an ignored test that downloads the real installer into a scratch
cache and checks its `MZ` start. The old group-count assertion moved from 6 to
7. Clippy reports nothing in the new code; the pre-existing `map_unwrap_or`
notes of the 1.100 toolchain remain. On screen in a throwaway home with a
seeded `Prefixes/Battle.net`: the Library showed the BATTLE.NET chip and the
Overwatch 2 card; Ctrl+5 showed the Settings page with the LAUNCHERS chip, and
Ctrl+4 the Accounts page with the store cards. The pointer hovered but did not
activate the chip in this session (ydotool clicks and PageDown did not scroll
the Accounts page), so the group's row and the Battle.net card itself were not
seen. Not run: the real installer, download and client; the Accounts card's
Open.

Things to know: the installer is interactive, so Zendo only starts it and
watches the process; it is downloaded over HTTPS with no checksum to check.
The managed prefix is a plain Wine one, so `runner()` reads it as Wine. The
Launchers row has no Remove: deleting the prefix would delete the games.

## Item 58a, the Nexus Launcher import and launch (2026-11-17)

The read-only half of item 58: the PC Game Pass games the Microsoft Store
installed through Nexus Launcher, listed and started through the launcher's
own script.

- **`crates/zendo-core/src/xodus.rs`**: `profile()` is
  `$XDG_CONFIG_HOME/xodus-nexus`, kept only when it is a folder.
  `scan(profile)` reads `launcher-state.json` (its `games` array), and keeps a
  record only when `installState` is `ready`, its `id` is not empty and its
  `path` is absolute. `game()` builds `xbox:<id>` with the record's `title`
  (or the id when it is empty), `Source::Xbox` and
  `Launch::Command { program: <profile>/shortcuts/launch-game, args: [<id>] }`;
  the record's `publisher` becomes the details' developer, `path` the install
  folder and the platform is Windows.
- **The script is checked before anything is started**: `shortcuts/launch-game`
  must carry the launcher's `# X-Xodus-Nexus-Profile=` marker on line 2, as
  omakade's own guard does. Without it the profile gives one problem and no
  games, so a file that is not the launcher's is never run.
- **Artwork, as the user chose**: the record's inline `coverDataUrl` is the
  cover (a `data:` URL) and its `heroUrl` the wide art. `covers`' `download`
  now reads a `data:` URL itself (`inline_image`: the payload after the comma,
  base64, decoded, the format taken from the bytes' signature, at most 16 MB),
  so a scan writes nothing and the covers fetch handles it with the rest. With
  no inline cover, SteamGridDB still fills in as for any other source.
  **Option 2, recorded for later**: Microsoft Store art from
  `displaycatalog.mp.microsoft.com` by `storeId` (`Poster` at `w=600`,
  `SuperHeroArt` else `TitledHeroArt` at `w=1920`). It looks better than the
  launcher's inline picture but costs one request per game, so it is not in
  this slice.
- **`zendo-core/src/lib.rs`**: `Source::Xbox` (nine sources now), `Scan`'s
  hookup after Battle.net's, and `Game::launcher()` says "Nexus Launcher" for
  a `Launch::Command` from Xbox. `report.rs` and `settings_ui.rs` list the
  source; the new words `sources-xbox` are in both languages. The Library's
  chip comes from `Source::ALL`, so it needed nothing.

Checked (2026-11-17): `cargo test --workspace` green (169 zendo, 301 core, 91
shell). New tests: the profile's read (a ready game kept, an installing one,
a relative path and one without a path left out), a title that is empty take
the id, a foreign `launch-game` gives one problem and no games, and no
profile gives neither; `inline_image` with a one-pixel PNG. In a throwaway
home with a profile and a fake `launch-game`: `zendo list` printed
`xbox:0123456789abcdef  Xbox  Halo Infinite`, and `zendo launch` ran the
script with `0123456789abcdef` and exited 0; with a foreign script, `zendo
list` printed one problem and no game. Not seen: the window (the chip, the
card, the page), a real launcher or a real Game Pass game, the inline cover
on screen, and pt-PT.

Things to know: Nexus Launcher is not installed on this machine, so the real
check waits for 58b, which brings the AppImage. A game whose shortcut is
missing is not listed at all, since nothing could start it.

## Item 58b, the launcher's download and the Accounts card (2026-11-17)

The second half of item 58: Settings brings Nexus Launcher and opens it, and
the Accounts page has a card for it.

- **`zendo-core/src/xodus.rs`**: `release()` asks GitHub for the latest
  release of `domsvoboda/nexus-launcher` and takes the AppImage for this
  machine (`Xodus-Nexus-<version>-x86_64.AppImage`, or `-arm64`/`aarch64` on
  arm), through the new `engines::latest_asset`: the engines' GitHub call,
  split out, with the digest still required and no digest meaning no release.
  `install()` downloads into the cache, checks with `sha256sum`
  (`engines::check_sha256`), copies it to
  `$XDG_DATA_HOME/zendo/launchers/Nexus.AppImage` executable (0755) and swaps
  it in; a failure leaves the AppImage there before and clears the partial.
  `open()` starts it in its own window, its output in
  `$XDG_STATE_HOME/zendo/logs/nexus.log`, and hands the child back so the
  caller reaps it; `installed()` and `count()` feed the row and the card.
- **`launch::spawn_logged`** is the "start it, output into the logs folder"
  helper Battle.net had to itself, moved there so both use it.
- **Settings → Launchers** has a second row (`sources_ui.rs`): Not installed →
  Download launcher (the same bar and Stop) → Open Nexus Launcher, with the
  AppImage's path, the games found and a note. The section's note and bar are
  shared by the two rows, and the bar says which download it is; the list is
  addressed by index (0 Battle.net, 1 Nexus Launcher). When the launcher's
  window closes, the page looks again and the library rescans.
- **Accounts** has its card: `LauncherCardData` gained `title`, so the badge
  can read XBOX while the card reads Nexus Launcher, and `LauncherCard` gained
  an `act()` callback instead of the hard-wired Battle.net one. XBOX / Nexus
  Launcher, the AppImage's path, Not installed or Installed or n games, Open
  Nexus Launcher, and the line that the sign-in and its runtime are done in
  the launcher's own window.
- Words: eleven `xbox-*` in both languages, and `launchers-about` now speaks
  of launchers rather than of Battle.net's installer.

Checked (2026-11-17): `cargo test --workspace` green (169 zendo, 302 core, 91
shell): the two Settings rows and both launchers' cards. Clippy clean in the
new code, `too_many_arguments` expected on the two functions that take a row's
or a card's words. The real release: `cargo test -p zendo-core xodus::online --
--ignored --nocapture` printed v0.5.0, `Xodus-Nexus-0.5.0-x86_64.AppImage`,
130930811 bytes and the digest the API gives, and
`installs_the_real_launcher` (with `XDG_DATA_HOME` and `XDG_CACHE_HOME` in a
scratch folder) downloaded it, checked it and put it in place, mode 0755, an
ELF file, in 4.5 s. The AppImage's own runtime answers `--appimage-version`
(`effcebc`) there, so FUSE works on this machine.

The user's first look on screen (2026-11-17) found one bug: after Download
installer, Battle.net's line was drawn three times, in its row, in Nexus
Launcher's row and again under the group. Both rows had been given the
section's one note. **Fixed** the same day: each launcher keeps its own note
and its row shows it; the section's `launchers-note` and
`launchers-note-tone` are gone from `ui/settings.slint` and
`SettingsData`, and one test pins it (`a_note_belongs_to_the_row_it_is_about`:
Battle.net's note in Battle.net's row, nothing in the other). Also then: the
section's note is cleared when the window it announced closes.

Not seen: the two rows and the card on screen after that fix, the download's
bar in the window, and the launcher's own window with its runtime download
and the Microsoft sign-in, which is the user's to do. Not tried: pt-PT.

## Item 58c, Battle.net on Proton (2026-11-17)

The user installed Battle.net from Zendo's Settings and the client never got
past its loading screen; the Library stayed empty because the Agent's
`product.db` never gained a product.

- **What the log said** (`$XDG_STATE_HOME/zendo/logs/battlenet.log`, 2026-11-16
  01:12): `wine: Unhandled exception 0x80000003 in thread 49c at address
  68F300E1 (thread 049c), starting debugger...`, then only the client's
  shutdown noise. `wine --version` is wine-11.22, and the prefix Zendo made,
  `~/Games/Prefixes/Battle.net`, carries no `bottle.yml` and no `version`
  file, so `runner()` read it as plain Wine: the client crashes in stock Wine
  rather than hanging.
- **The fix** (`crates/zendo-core/src/battlenet.rs`): Zendo's own prefix runs
  on Proton. `managed(prefix)` recognises it by its place,
  `Prefixes/Battle.net` under the games folder ([`managed_prefix`]), so no
  configuration is read to tell, and `runner()` answers `Proton` for it, as it
  already did for a Steam-style `pfx` or a `version` file at the prefix root.
  `run_installer` now starts Blizzard's installer with `runner(&prefix)`
  instead of a hard-coded Wine, so the installer, the client and every game
  imported from that prefix all go through `umu-run`. With no Proton chosen in
  Settings, `in_prefix` leaves `PROTONPATH` unset and umu-launcher fetches and
  keeps its own UMU-Proton (umu-launcher 1.4.3 here), which is what it is for.

Checked: `cargo test --workspace` green (170 zendo, 302 core, 91 shell); the
runner test now also checks that `Prefixes/Battle.net` answers Proton. Clippy
reports nothing in the file. Not seen: the client signing in under Proton,
and a game imported from that prefix started through it. The prefix was made
by Wine, so Proton's first run upgrades it.

Things to know: a prefix at `<any>/Prefixes/Battle.net` is read as Zendo's
own and so runs on Proton; a Battle.net prefix made by hand elsewhere keeps
Wine unless it carries a Proton marker. `umu-run` or a Proton missing now
gives its own error at Play or Open ("umu-launcher is needed…").

## Item 59, the Xbox Series S's row (2026-11-17)

Remote play is a streaming client, not a library source: Greenlight streams a
console's own screen and lists no games, so the item is the thin entry that
opens it.

- **`zendo-core/src/greenlight.rs`**: `installed()` finds the Flatpak's export
  desktop file
  (`$XDG_DATA_HOME/flatpak/exports/share/applications/io.github.unknownskl.greenlight.desktop`,
  else the system's under `/var/lib/flatpak`), or a `greenlight`/`Greenlight`
  on `PATH`; `open()` runs `flatpak run io.github.unknownskl.greenlight` (or
  the program), its output in `$XDG_STATE_HOME/zendo/logs/greenlight.log`, and
  hands the child back; `PAGE` is its Flathub page.
- **Settings → Launchers** has a third row (index 2): **Xbox Series S**, Not
  installed with **Get on Flathub** (which opens its page, since Zendo installs
  no other program's Flatpak) or Installed with **Open Greenlight**, and the
  program's path. When Greenlight's window closes the row is looked at again.
- **No `--connect`**: Greenlight's `--connect=<console id>` starts a stream
  once signed in, but the id is a console's own 16-hex id, which Zendo does not
  know (its README: "For console use `F000000000000000` format"), so the window
  opens with its own console list. Recorded in `docs/reference.md`. (The
  AppImage this entry's row first lacked is the next section.)
- **Words**: `launcher-not-installed` and `launcher-installed` are now shared by
  the three rows and both Accounts cards, the four per-launcher copies gone,
  and new `series-*` words in both languages (their count changed with the
  AppImage, below).

Checked: `cargo test --workspace` green (170 zendo, 303 core, 91 shell): the
third row's words, its action and its empty note, and `exported()` over a
temporary Flatpak export folder. Clippy reports nothing in the new code. Not
seen: the row on screen, and Greenlight itself, which is not installed on this
machine (no export desktop file in either place, no `greenlight` on `PATH`), so
neither Open Greenlight nor Get on Flathub was pressed and no console was
streamed. Not tried: pt-PT.

## Item 59 again: the AppImage, and the embedding question (2026-11-17)

The user had not tried Greenlight yet and asked two things: why the Flatpak
rather than the AppImage its release carries, and whether the streaming could
live inside Zendo instead of an external program.

- **The AppImage** is now what Zendo brings. Item 59 first only found a
  Flatpak or a program on `PATH`, because the reference had recorded the
  Flathub route. `greenlight::release()`/`install()` take
  `Greenlight-<version>.AppImage` from the latest GitHub release (v2.4.2,
  130720574 bytes, digest `sha256:1ceb56c4…`) and put it in Zendo's launchers
  folder, executable, with the download's bar and Stop in the row. The
  placement moved out of `xodus` into `engines::bring` (and
  `engines::launchers_dir`), so Nexus Launcher and Greenlight share it. A
  Flatpak or a `greenlight` on `PATH` still works: `installed()` prefers
  Zendo's AppImage, then the Flatpak's export desktop file, then `PATH`.
- **Embedding** is recorded on hold in `docs/backlog.md` with what it would
  take: the Xbox sign-in, WebRTC, video and audio decoding, input over the data
  channel, and a video surface in Slint. Greenlight's streaming half is MIT
  (`xbox-xcloud-player`, TypeScript), so ideas and protocol knowledge are
  usable; a Rust port is a phase of its own, and the only part of Zendo that
  would need a GPU video path and could not be checked offline.

The user tried it on screen the same day and got "Greenlight is not
installed: get it on Flathub" from **Open Greenlight**, with the AppImage
already in place. A replacement had silently missed: `open()` still asked the
Flatpak first and the `AppImage` never, while `installed()` (and so the row)
had found it. **Fixed**: `open()` asks Zendo's `AppImage`, then the Flatpak,
then `PATH`, through a small `opener()` that takes the three answers, and a
test pins the order (`opens_the_appimage_before_a_flatpak`) so an AppImage
never loses to a Flatpak again.

Checked: `cargo test --workspace` green (170 zendo, 305 core, 91 shell); two
new ignored tests ask GitHub for the real release and, in a scratch home,
download it and check it. `cargo test -p zendo-core greenlight::online --
--ignored` printed v2.4.2, `Greenlight-2.4.2.AppImage`, 130720574 bytes and the
digest; the install put it at `$XDG_DATA_HOME/zendo/launchers/Greenlight.AppImage`,
mode 0755, an ELF, in 3.1 s, and its runtime answers `--appimage-version`
(`effcebc`). Seen on screen (2026-11-17, the
user): the row, the download, and **Open Greenlight** starting the AppImage.
Not seen: a console streamed. Not tried: pt-PT.

## Item 60a, the Game Pass list on the Store page (2026-11-17)

The first slice of item 60: the games the account owns in Game Pass, listed in
Zendo, read from Nexus Launcher's backend. Installing them is 60b.

- **`crates/zendo-core/src/xodus.rs`**: `owned()` asks the launcher's backend
  for its entitlements (`xodus-cli library --json`, at the `binaryPath` of
  `xodus-backends/current.json`; the first `{` in its output starts the
  answer, its diagnostics come first), keeps the `Active` ones, and then asks
  the Microsoft Store's catalogue for them, 20 ids at a time
  (`displaycatalog.mp.microsoft.com/v7.0/products?bigIds=…&market=US&languages=en-US&fieldsTemplate=Details`).
  A product counts as a game when its `ProductBSchema` starts with
  `ProductGame`; an add-on (`ProductAddOn`) is left out. The title is
  `ProductTitle` (or `ShortTitle`), with the developer, the publisher and the
  short description; the art is `Poster` at `w=600` and `SuperHeroArt` (else
  `TitledHeroArt`) at `w=1920` from `LocalizedProperties[].Images`, whose
  `Uri` the catalogue gives without a scheme and to which `?w=` is appended
  (checked: 600x900 and 1920-wide JPEGs). The list goes through
  `owned::keep`, so it is sorted, deduplicated, cached and given its images by
  the same path the stores' lists take.
- **`owned.rs`**: `Shop::Xbox` (five shops now), `store()` `None` (no engine
  installs it), `source()` `Source::Xbox`, and `key()` written out rather than
  taken from a `Store`, since two shops now have none.
- **The Store page** (`store_ui.rs`): Game Pass is "listed and connected" when
  the launcher is installed (a file check; a window that persists nothing, the
  tests', looks at no launcher at all), and its list is asked for on the same
  threads as the stores' (`fetch_owned`, then `store_fetched`). Its chip, its
  cards, its sort and its search come from the existing code; the cover fetch
  follows `cover_url`/`hero_url` as for any store.
- **Accounts' count** now says "n of `Store::ALL.len() + 1`": Game Pass is not
  one Zendo signs in to, so the launcher's sign-in is not counted there.

Checked (2026-11-17): `cargo test --workspace` green (170 zendo, 307 core, 91
shell). New: the entitlements' parse (diagnostics before the JSON, a revoked
entitlement left out), the catalogue's game-and-add-on split, the title
fallback and the art addresses; a Store-page test (an Xbox list gets its chip,
its card and its id); and an ignored test that asks the real backend and
Store. Run against the user's account: 36 entitlements, 22 games (14 add-ons
left out), each with a publisher and a cover, in 1.9 s, for example
`xbox:9NJRX71M5X9P Cuphead | Studio MDHR` and `xbox:9PK09BL31FK1 DOOM Eternal
Standard Edition (PC) | Bethesda Softworks`.

Not seen: the Store page on screen with the Xbox chip (the user's own
account), the covers downloaded, a game's page from that list, and a refresh.
Not tried: a signed-out launcher, pt-PT.

Things to know: entitlements are read as the launcher holds them, so a title
Zendo lists is one the account has; whether it can be installed on this PC is
`resolve-package`'s answer, which is 60b's. Two editions of one game (DOOM
Eternal's Battlemode build and standard edition) are two products and so two
cards, as the Store lists them.

## Item 60b, the Game Pass page's size and its launcher (2026-11-17)

The second slice of item 60: what the Store would install for a Game Pass
game, said on its page, and the way to the program that installs it.

- **What the backend can say** (checked 2026-11-17, `xodus-cli
  resolve-package <product> --json`): `choices` with `platform`
  (`Windows.Desktop` for a PC, `Windows.Xbox` for a console), `packageFormat`
  (`MSIXVC`, `EAPPX`, `XVC`), `downloadBytes`, `installBytes` and `supported`,
  and a top-level `status` (`ready`, `choice`, `unavailable`). Minecraft is
  `supported: true`, `ready`, 2087256064 bytes, MSIXVC; Cuphead is
  `unavailable`, its desktop build an EAPPX of 5399841994 bytes with
  `supported: false`. The Store, not the launcher, answers this, so it needs
  nothing installed but the backend.
- **`xodus::package(product)`** runs that and picks the desktop build (else
  the first), with `supported`, `status`, the size and the format.
- **The game's page** (`store_ui.rs`): an Xbox store game's `want_info` asks
  that instead of a store engine, keeps the answer by game id, and
  `install_note` says where it goes: "Install it in Nexus Launcher; it needs
  2.1 GB…", or "The Store has no install of it for this PC" when it is not
  supported. The action row gets **Open Nexus Launcher**
  (`GameData.owned-launcher` and its `open-launcher` callback, empty for every
  other store game and reset when a library game's page opens), so the game is
  a click and a sign-in away without hunting Settings. Words: three
  `xbox-install-*`/`xbox-not-available` in both languages.
- **Why Zendo does not run the install itself yet.** `xodus-cli streaming
  <product> <folder>` does exist (progress as `--progress-json`, pause and
  resume over stdin), but it needs the launcher's runtime installed, and
  `download <product>` refuses with "Select an installable package in Nexus
  before starting this download", i.e. the CLI expects the launcher's own
  selection; an install made behind the launcher's back would also not be in
  `launcher-state.json`, which is what Zendo reads to list the game and what
  `shortcuts/launch-game <id>` needs to start it. `run <path> <wine>` could
  start one without that state, but that is a second launching path to design.
  Recorded in `docs/backlog.md` (60c) with the one experiment that would
  settle it: install a small title with `streaming` and see whether the
  launcher's state gains it.

Checked: `cargo test --workspace` green (172 zendo, 308 core, 91 shell). New:
the package picking (a desktop build preferred over a console one), the page
test (a supported game's note with its size and the launcher button; an
unsupported one's note), and two ignored tests against the real backend, one
of which resolved Minecraft and Cuphead as above. Not seen on screen: the
button, the note with a real size, and a game installed in the launcher and
then listed by Zendo. Not tried: pt-PT.

## The Game Pass list after the first real install (2026-11-17)

The user installed Minecraft Legends through Nexus Launcher and tried the
Store again. Two bugs, both fixed:

- **The Library did not see it until Refresh.** Nothing watched the launcher's
  state. `watch::signature` now notes
  `$XDG_CONFIG_HOME/xodus-nexus/launcher-state.json`, so an install (or an
  uninstall) there rescans within the ten seconds the watcher ticks at, as
  Steam's and Heroic's files do.
- **The Store never marked a Game Pass game installed, and its page still
  offered "Open Nexus Launcher".** The library's id was the launcher's own
  (`xbox:<16 hex>`), while the Store's list is keyed by the Store's product id
  (`xbox:<storeId>`), so the two could never meet. `xodus::game()` now names
  the game `xbox:<storeId>` and keeps the launcher's id where it belongs, in
  `launch-game`'s argument; a record without a `storeId` falls back to the
  launcher's id. Checked against the user's own state in a throwaway home:
  `zendo list` printed `xbox:9N98Z825TNFW  Xbox  Minecraft Legends - Windows`.

Also checked, since the user had installed the launcher's runtimes (Xodus
backend, Standard and CachyOS): they do not change `supported`.
`resolve-package` still answers `unavailable` and `supported: false` for
Cuphead, so the flag is about the Store's entitlement for a PC, not about the
runtime. The whole list's packages, resolved one by one on 2026-11-17:

```
   0.01 GB  yes  ready        MSIXVC  Minecraft Launcher
   0.09 GB  yes  ready        MSIXVC  Retro Classics
   0.19 GB  no   unavailable  MSIXBUNDLE Roblox - Not Supported
   0.20 GB  no   unavailable  EAPPXBUNDLE Candy Crush Saga
   0.39 GB  yes  ready        MSIXVC  Roblox - Windows
   2.09 GB  yes  ready        MSIXVC  Minecraft for Windows
   5.40 GB  no   unavailable  EAPPX   Cuphead
  10.45 GB  yes  ready        MSIXVC  Pool Blitz
  12.47 GB  yes  ready        MSIXVC  Minecraft Legends - Windows
  25.10 GB  yes  ready        MSIXVC  DRAGON BALL XENOVERSE 2
  96.70 GB  yes  ready        MSIXVC  DOOM Eternal Standard Edition (PC)
 120.45 GB  yes  ready        MSIXVC  Fallout 76 (PC)
 338.18 GB  yes  ready        MSIXVC  Call of Duty®
```

(The rest are editions of the same titles.) `cargo test --workspace` green
(172 zendo, 309 core, 91 shell) with the new id test. Not seen on screen: the
badge and the page after the fix.

## The Game Pass install experiment (2026-11-17)

The one test that decides whether Zendo can install Game Pass games itself:
does the launcher's state learn about an install made with the CLI? It does
not. The run, with the user's go-ahead, on Retro Classics (the smallest
supported title, 88,821,760 bytes):

```
xodus-cli streaming 9MTVJ3HHTQGS "$HOME/Games/Xodus Nexus/Retro Classics" \
    --progress-json --market US
```

- **It works**: exit 0, 84,879,403 bytes reported as `totalBytes`, 85 MB in the
  folder, the same shape as the launcher's own install
  (`appxmanifest.xml`, the payload, `RetroClassics.exe` at 6,565,120 bytes).
- **The progress protocol** is one `NEXUS_PROGRESS ` line per event, then
  JSON: `schema: 1`, `sequence`, `event: "progress"`, `phase` (`downloading`,
  `finalizing`, `complete`), `completedBytes`, `totalBytes`, `bytesPerSecond`,
  `canPause`, `paused`. It ended with `phase: "complete"`, `canPause: false`.
- **`launcher-state.json` was untouched**: the same MD5 before and after
  (`b9e22952b8ddd92a71f26b968f3af76c`), `games` holding only the Minecraft
  Legends the launcher had installed. The launcher's `shortcuts/launch-game`
  is `exec Nexus.AppImage --launch-game "$@"`, i.e. the launcher itself
  starts a game, so a game it never recorded cannot be started through Zendo's
  current path.
- **Launching without the launcher** would be `xodus-cli run <folder> <wine>`,
  and the Wine matters: with the system Wine 11.22 the game page-faulted
  during the prefix setup (`wineboot.exe` appeared, then `wine: Unhandled page
  fault on write access`), exit 5. The launcher's runtimes are Proton builds
  under `~/.local/share/xodus-nexus/runtimes/variants/{nexus,nexus-cachyos}/releases/<hash>/`,
  and `run` was not tried with one.

The test install was removed afterwards (85 MB), and the state file left
exactly as it was. Recorded as item 62 in `docs/backlog.md`: a Zendo install
would be a manager of its own, with its own record, launch and progress, and
the launcher would not see those games.

## Items 60d and 60e, the Game Pass cards and achievements (2026-11-17)

- **60d, the packages on the cards.** The Store was only asked about a game's
  package when its page opened. Now, when the Game Pass list arrives, a thread
  asks for every game's package one at a time (about 1.4 s each), and each
  answer goes into the browser (`StoreBrowser::package` and `set_package`), so
  a card carries an **UNSUPPORTED** badge (`badge-unsupported`, in the accent
  colour) when this PC cannot install the game, and the page's note and size
  row are instant afterwards. The first shape was a caption ("Needs 1.94 GiB"
  or "Not for this PC"), which the user turned down the same day: no other
  store's card shows a size, and the page's details already do; a badge says
  the one thing a card cannot otherwise show.
- **60e, achievements.** `xodus::title_id` reads the `<TitleId>` (hex) from
  the `MicrosoftGame.config` beside an installed game's executable, since the
  backend wants it in decimal; `xodus::achievements(title)` asks
  `achievements <title> --json` and maps the items into Zendo's
  `Achievement`: name, description, `hidden` from `secret`, `unlocked` from
  `unlockedAt` (ISO 8601, through `time::parse_utc`), revoked items dropped,
  unlocked first newest first then the locked by name. An `xbox:` library
  game's Achievements card fills from that, with `unlocks_known: true` so its
  "n of m" line is right. Their pictures are the backend's `iconUrl`, fetched
  through `http::cached_image` into the same folder Steam's achievement icons
  use (`$XDG_CACHE_HOME/zendo/achievements`, `achievements::icons_dir`), so
  the first look at a game downloads them (40 for Minecraft Legends, 12.6 s)
  and the next looks find them (3 s). `entitlements`, `package` and `achievements` now
  share one `backend()` runner.

Checked: `cargo test --workspace` green (173 zendo, 310 core, 91 shell): the
card captions (a size and "not for this PC", in title order), the title id
from a config file, and the ordering and secret handling of a list of
achievements. Against the real backend: 40 achievements for Minecraft Legends
(title `61B215AE`, decimal 1639060910) in 3 s. Not seen on screen: a card
saying "Needs …", a Game Pass game's achievements on its page, and the icons,
which are absent on purpose. Not tried: pt-PT.

## Battle.net taken out (2026-11-17)

The user asked for Battle.net to come out for now and to stay a possible
future source, beside EA app, Ubisoft Connect and others, to look at again
after the first stable release. It had never worked: the client started under
Proton but its window stayed blank (item 61), so no game was read from a real
installation. Removed whole:

- **`crates/zendo-core/src/battlenet.rs`**, and with it `Source::BattleNet`
  (the sources go from nine to eight), `Library::scan`'s Battle.net pass,
  `Launch::Prefix` and `WineRunner` (no other source built one),
  `launch::in_prefix`, and the prefix and log rows a Battle.net game's
  Technical card added in `game_ui.rs`.
- **Settings → Launchers**: its row, the installer's download and run, and
  its place in the download enum and the bar. Nexus Launcher and the Series S
  are now rows 0 and 1.
- **Accounts**: the BATTLE.NET card, its `battlenet-open` callback and its
  state; Nexus Launcher's card stays alone under the stores.
- **Text and lists**: the `battlenet-*` messages from `en.ftl` and
  `pt-PT.ftl`, the Settings → Sources row and the report's source counts.
- **Tests**: the module's 12 (one ignored) went with it; the Settings page's
  launcher test now expects two rows and the Accounts page's one card.

What was verified stays where it belongs: `docs/reference.md` keeps the
Agent's `product.db`, the client's paths, the launch codes and the installer
for the future source, and `docs/backlog.md` keeps the removal and item 61's
blank-window notes. Zendo's own leftovers on disk are left alone, since
nothing reads them now: the installer in `$XDG_CACHE_HOME/zendo/battlenet/`
and the prefix Zendo made at `<games folder>/Prefixes/Battle.net` (the user
can delete both). There is no migration: a state file with a saved filter
on Battle.net would not load, and the user's `state.json` held no Battle.net
game or filter (checked, 855 bytes). Checked: `cargo test --workspace` green
(173 zendo, 299 core, 91 shell); `cargo fmt` clean on the touched files;
clippy reports only the warnings the newer toolchain already gave (the
`map_or_default` suggestions). Not seen: the window after the removal (the
rows and the card left).

## Item 60c closed: the launcher keeps update and uninstall (2026-11-17)

Item 60c was "uninstall and update if the CLI allows them". Checked against
the user's backend (v0.2.15, `current.json`'s `binaryPath`) and closed
without code, because it allows neither:

- `xodus-cli --help` lists `keyring-*`, `resolve-package`, `download`,
  `license`, `extract`, `extract-eappx`, `saved-accounts`, `account`,
  `profile`, `native-social`, `social`, `game-activity`, `achievements`,
  `library`, `login`, `logout`, `streaming`, `run`, `clep`, `sp-license`,
  `offline-pc` and `help`: no uninstall, no update.
- `resolve-package 9N98Z825TNFW --json --market US` answers `contentId`
  (`f8afac2c-44cd-4302-afc3-b6dc979e6656`), `downloadBytes` (12470362112),
  `installBytes: null`, `packageFormat` (`MSIXVC`), `supported` and `status`:
  no version. The Store catalogue's package `Version` is `"0"`.
- `launcher-state.json` does carry `games[].installedVersion`
  (`1.18.19068.0` for Minecraft Legends) and `downloads[].selectedContentId`,
  and that id equals the `resolve-package` `contentId` above. Comparing the
  two could mark an update, but no update existed to see whether a new build
  changes the content id, so it was not built on a guess.
- Zendo already watches `launcher-state.json` (`zendo-core/src/watch.rs`), so
  the launcher's own install, update and uninstall are followed by a rescan.

Recorded in `docs/reference.md` ("Nexus Launcher") and `docs/backlog.md`
(item 60); item 62 (installing Game Pass games from Zendo, and the launch
path behind the launcher's back) is untouched. No code changed; tests were
not rerun for this.
