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
  (above). Next: item 20, the window
  (minimise when a game starts, Discord presence, rescanning when a
  launcher's files change, processes tracked by their start time).
