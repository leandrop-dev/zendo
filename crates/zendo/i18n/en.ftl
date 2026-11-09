# Zendo's window in English. Each message is used by the window's code as
# t!("id"); values come in as { $name }. See https://projectfluent.org.
# pt-PT.ftl holds the same messages in Portuguese (Portugal).

## Counts and times

count-games = { $count ->
    [one] { $count } game
   *[other] { $count } games
}
count-sources = { $count ->
    [one] { $count } source
   *[other] { $count } sources
}
count-files = { $count ->
    [one] { $count } file
   *[other] { $count } files
}
time-minutes = { $minutes } min
time-hours = { $hours } h
time-hours-minutes = { $hours } h { $minutes } min
ago-now = just now
ago-minutes = { $count } min ago
ago-hours = { $count } h ago
ago-today = today
ago-yesterday = yesterday
ago-days = { $count } days ago
ago-weeks = { $count } weeks ago
ago-months = { $count } months ago
ago-years = { $count } years ago

## Pages and the sidebar

page-library = Library
page-store = Store
page-downloads = Downloads
page-accounts = Accounts
page-settings = Settings
page-about = About
sidebar-browse = BROWSE
sidebar-system = SYSTEM
sidebar-signed-in = { $count } of { $stores }
sidebar-your-library = YOUR LIBRARY
sidebar-summary = { $sources } · { $time }

## Sources and orders

source-desktop = Desktop
source-added = Added
sort-recent = Recent
sort-most-played = Most played
sort-title = A–Z

## Library

library-looking = Looking for games…
library-none-found = No games found
library-none-found-about = Zendo reads Steam's libraries, the games Heroic installed and .desktop files in the Game category.
library-no-match = Nothing matches “{ $search }”
library-none-here = No games here · choose All to see every store
library-all-hidden = Every game is hidden · Ctrl+H or Start shows them
library-subtitle = { $games } from { $sources }
library-subtitle-hidden = { $subtitle } · { $hidden } hidden
library-scanning = Scanning…
library-refresh = Refresh
library-search = Search your library
library-add-game = Add game
library-all = All
library-sort = Sort
library-show-hidden = Show hidden
library-hidden = Hidden
badge-running = RUNNING
badge-pinned = PINNED
played-never = Not played yet
played-when = Played { $when }
played-time = { $time } played

## About

about-source-code = Source code
about-report-problem = Report a problem
about-version = Version { $version }
about-intro = Every game on this machine in one library: Steam, Epic, GOG, Amazon and the rest, each started the way its own launcher would.
about-licence = Licence
about-licence-text = MIT. Zendo reads the launchers' files and never writes to them.
about-built-on = Built on
about-built-iced = Iced, the window and everything in it
about-built-gilrs = gilrs, for controllers
about-built-keyvalues = keyvalues-parser, for Steam's files
about-built-ureq = ureq, serde and toml
about-built-steamgriddb = SteamGridDB, for covers no store has
about-built-fluent = Fluent, for the window's languages
about-ideas-from = Ideas from
about-ideas-hikari = Hikari: the sidebar, the library and the game pages
about-ideas-omarchy = omarchy-games-menu: typing anywhere to filter
about-ideas-heroic = Heroic: where its games are kept, and its launch links

## The window

game-gone = This game is no longer in the library
owned-gone = This game is no longer in your stores' lists
images-failed = { $count ->
    [one] { $count } image could not be downloaded; `zendo covers` says why
   *[other] { $count } images could not be downloaded; `zendo covers` says why
}
images-downloading = Downloading images, { $count } left
footer-unreadable = { $count ->
    [one] { $count } file could not be read; Settings names it
   *[other] { $count } files could not be read; Settings names them
}
status-hid = Hid { $title } · Ctrl+H shows hidden games
status-unhid = { $title } is back in the library
status-pinned = Pinned { $title }: it leads the library
status-unpinned = Unpinned { $title }
hints-library = Enter open · Tab sort · Ctrl+P pin · Del hide · Ctrl+H hidden · F11 full-screen
hints-sheet = Enter save · Esc cancel
hints-game = Enter play · Esc back · Ctrl+P pin · Del hide · F11 full-screen
hints-store = Enter open · type to search · Esc clear · F11 full-screen
hints-owned = Enter install or open · Esc back · F11 full-screen
hints-closing = Enter close anyway · Esc keep downloading
hints-other = Esc back · F11 full-screen
panicked = Zendo hit a bug doing that; the terminal has the details

## Settings

settings-subtitle = The language, covers, Nexus Mods, Proton, the sources Zendo reads and its own files
language-title = Language
language-about = The window's language. System follows your desktop's; the command line stays in English.
language-system = System ({ $language })
key-removed = Key removed
key-saved-unchecked = Saved, but not checked: { $error }
key-paste-first = Paste a key first
key-hide = Hide
key-show = Show
key-checking = Checking…
key-check = Check
key-save = Save
key-none = No key set
kept-keyring = Kept in the system keyring.
kept-file = Kept in config.toml, readable only by you.
kept-file-why = Kept in config.toml, readable only by you: { $why }.
kept-unread = Could not read the keyring: { $why }.
steamgriddb-saved = Saved. SteamGridDB accepts this key.
steamgriddb-accepted = SteamGridDB accepts this key; Save keeps it.
steamgriddb-rejected = SteamGridDB turned this key down. Copy it again from your SteamGridDB preferences.
steamgriddb-rejected-unsaved = SteamGridDB turned this key down, so it was not saved. Copy it again from your SteamGridDB preferences.
steamgriddb-paste = Paste your SteamGridDB API key
steamgriddb-get-key = Get a free key from your SteamGridDB preferences
steamgriddb-about = Covers and wide images for games whose store has none. The key is kept in the system keyring, or in your settings file, readable only by you, when there is none.
report-title = Diagnostics
report-about = A report of what Zendo found, to paste into a bug report: the system, the tools and their versions, the Protons, the settings (never the keys), whether each store is signed in (never the account), the library's counts, the Wine prefixes and the last three launches' logs. Your home folder shows as ~. Read it before you share it: a game's log can name more. `zendo report` prints the same, `zendo logs` lists the logs. The report is in English.
report-gathering = Gathering…
report-copy = Copy report
report-open-logs = Open launch logs
report-copied = Copied the report: { $count } lines.
images-none = No images to download
images-left = { $count } left
covers-title = Covers
covers-about = Steam's come from its own cache. Epic, GOG and Amazon publish theirs; the rest come from SteamGridDB. A game with none anywhere is asked about again after a week.
covers-download = Download missing covers and wide images
covers-download-now = Download missing images now
sources-title = Sources
sources-about = Where Zendo looks for games. It reads these and never writes to them; Refresh in the library looks again.
sources-epic = Epic Games, through Heroic
sources-gog = GOG, through Heroic
sources-amazon = Amazon Games, through Heroic
sources-heroic = Added to Heroic by hand
sources-desktop = .desktop files in the Game category
sources-local = Added by hand in Zendo
sources-none-found = none found
sources-unreadable = Could not read { $problem }
files-title = Files
files-about = Zendo's own files. Removing one resets what it holds. Store games install into the games folder; `install_dir` in the settings file moves it.
files-no-home = no home folder
files-open-folder = Open folder
files-settings = Settings
files-state = Launches, pins, hidden
files-images = Downloaded images
files-queue = Download queue
files-sign-ins = Store sign-ins
files-games = Games

## Nexus Mods

cancel = Cancel
nexus-no-game = A mod for { $name } came from Nexus, but no installed game takes mods
nexus-updates-failed = Could not ask Nexus for updates: { $error }
nexus-no-exe = could not tell where Zendo is: { $error }
nexus-links-open-in = Nexus's links now open in { $exe }
nexus-key-saved = Saved. { $who }.
nexus-key-accepted = { $who }; Save keeps the key.
nexus-key-rejected = Nexus turned this key down. Copy it again from API Access on Nexus.
nexus-key-rejected-unsaved = Nexus turned this key down, so it was not saved. Copy it again from API Access on Nexus.
nexus-known-as = Nexus knows this key as { $name }
nexus-known-as-premium = Nexus knows this key as { $name }, with Premium
nexus-mod-arrived = A mod came from Nexus…
nexus-download-name = { $title } · file #{ $file }
nexus-downloading = Downloading a mod for { $title } from Nexus…
nexus-download-stopped = Download stopped
nexus-download-failed = Could not download the mod: { $error }
nexus-downloads-title = Mods from Nexus
nexus-paste = Paste your Nexus Mods personal API key
nexus-get-key = Get your personal API key from API Access on Nexus Mods
steam-sign-in-about = Sign in on Steam's own page, with your password and Steam Guard or the Steam app's QR code; Zendo never sees your password. The Store then lists the games you own and those your Steam family shares, and installs them through Steam. Zendo keeps the sign-in in the system keyring and Steam's page remembers it.
steam-sign-in = Sign in with Steam
steam-signing-in = Sign in on Steam's page in the window that opened
steam-signed-in = Signed in: the Store is asking Steam for your games
steam-signed-in-about = The Store lists the games this account owns and those its Steam family shares.
steam-signed-out = Signed out of Steam
steam-sign-in-again = Steam's sign-in has ended: sign in again
steam-family = Steam family: { $name } · { $count ->
    [one] { $count } member
   *[other] { $count } members
}
steam-family-failed = Your family's games could not be listed: { $error }
store-shared = Shared by your family
steam-asked = Asked Steam to install { $title }: choose where in Steam's dialog
steam-installed = { $title } is installed
steam-downloads-title = Steam
steam-open-downloads = Open Steam's downloads
steam-downloads-about = Steam downloads these itself; pause or cancel them in Steam.
steam-waiting-dialog = Waiting for Steam's install dialog
steam-stop-waiting = Stop waiting
steam-ask-again = Ask Steam again
steam-uninstall = Uninstall with Steam
steam-verify = Verify with Steam
steam-ask-again-uninstall = Ask Steam again to uninstall
steam-asked-uninstall = Asked Steam to uninstall { $title }: answer its question
steam-asked-verify = Asked Steam to verify { $title }'s files: it shows the progress
steam-waiting-uninstall = Waiting for Steam to uninstall
steam-uninstalled = { $title } is uninstalled
steam-updated = { $title } is updated
steam-verifying = Verifying in Steam
steam-waiting-verify = Waiting for Steam to start verifying
steam-verified = Steam verified the files of { $title }
steam-state-waiting = Queued in Steam
steam-state-downloading = Downloading, { $progress }
steam-state-installing = Installing, { $progress }
steam-state-paused = Paused in Steam, { $progress }
nexus-handler-zendo = Zendo opens Mod manager download links.
nexus-handler-other = Mod manager download links open in { $other }, not Zendo.
nexus-handler-nothing = Nothing opens Mod manager download links yet.
nexus-handler-other-zendo = Mod manager download links open in another copy of Zendo, { $program }; press Open them with Zendo for this one.
nexus-become-handler = Open them with Zendo
nexus-about = Mods from Nexus's Mod manager download buttons go straight into the game's mods, and Zendo tells when they have updates. Without Premium, Nexus gives files only through that button on its site. The key is kept in the system keyring, or in your settings file, readable only by you, when there is none.
nexus-progress = { $done } of { $total } MiB
nexus-progress-unknown = { $done } MiB
nexus-stopping = Stopping…
nexus-stop = Stop
nexus-choose-title = Which game is this mod for?
nexus-choose-none = Nexus says this mod is for { $name }, and no installed game goes by that name. Choose the game it is for; Zendo remembers it for this game's next mods.
nexus-choose-several = Nexus says this mod is for { $name }, and more than one installed game matches.
nexus-updated-to = Updated { $name } to { $version }
nexus-updated = Updated { $name }
nexus-added-version = Added { $name } { $version } to { $title }
nexus-added = Added { $name } to { $title }

## Mods

saving = Saving…
deleting = Deleting…
delete = Delete
really-delete = Really delete
profile-made = Made { $now }, a copy of the profile before
profile-switched = Switched to { $now }{ $state ->
    [applied] ; { $now } is applied
    [changed] ; Apply puts it in the game
   *[other] {""}
}
profile-renamed = Renamed the profile { $name }
profile-deleted = Deleted { $name }; now in { $now }{ $state ->
    [applied] ; { $now } is applied
    [changed] ; Apply puts it in the game
   *[other] {""}
}
mods-applied-count = { $count ->
    [one] Applied { $count } mod to { $title }
   *[other] Applied { $count } mods to { $title }
}
mods-vanilla = { $title } is back to vanilla
mods-applied = Applied to { $title }
mods-choosing = Choosing a mod…
mods-choose = Choose a mod
mods-deleted = Mod deleted
mods-new-folder = Mods go to the new folder
mods-overwrite-deleted = Deleted what the game wrote
mods-reading-installer = Reading the installer…
mods-applying = Applying…
mods-restoring = Going back to vanilla…
mods-added = Added { $name }
mods-unpacking = Unpacking…
mods-switching = Switching…
mods-close-game-first = Close the game first: its files are in use
mods-reading = Reading…
mods-add = Add mod…
mods-title = Mods
mods-find-on-nexus = Find mods on Nexus
mods-search-nexus = Search Nexus
search-title = Nexus Mods for { $title }
search-placeholder = Words in the mod's name, or none for every mod
search-order-endorsed = Most endorsed
search-order-downloaded = Most downloaded
search-order-updated = Recently updated
search-adult = Adult mods
search-adult-badge = Adult
search-have = In your mods
search-by = by { $author }
search-updated = updated { $date }
search-endorsements = { $count } endorsements
search-downloads = { $count } downloads
search-files = Files
search-hide-files = Hide files
search-page = Page on Nexus
search-asking-files = Asking Nexus for the files…
search-no-files = Nexus lists no current files for this mod.
search-download = Download
search-get-on-nexus = Get on Nexus
search-file-main = main
search-file-optional = optional
search-file-update = update
search-file-misc = miscellaneous
search-asking = Searching Nexus…
search-nothing = Nexus found no mods.
search-count = { $shown } of { $total } mods
search-downloading = Downloading from Nexus…
search-needs-key = Set your Nexus key in Settings to see a mod's files and download them from here.
search-not-premium = Without Premium, Get on Nexus opens the file's page in a window of Zendo's: sign in to Nexus there once, press Slow download, and the mod comes to Zendo.
search-have-file = You have this file
search-site-open = Nexus's page is open in its own window: press Slow download there, and Zendo adds the mod.
nexus-window-title = Nexus Mods · Zendo
nexus-window-downloading = Downloading… · Nexus Mods · Zendo
nexus-window-download-failed = The download from Nexus's page failed.
mods-place-known-root = Zendo knows this game: mods go in its own folder.
mods-place-known = Zendo knows this game: mods go in { $folder }.
mods-place-custom = Mods go in { $folder }, as you set.
mods-place-root = Mods go in the game's own folder; set another below if they belong elsewhere.
mods-none = No mods yet. Add mod… takes an archive (zip, 7z, rar) you downloaded.
mods-overwrite = Files the game wrote while modded: { $count }, applied after every mod
mods-from-nexus-version = version { $version } from Nexus
mods-from-nexus = from Nexus
mods-wins = replaces { $count } of { $other }
mods-loses = { $count } replaced by { $other }
mods-options = Options…
mods-update-to = Update to { $version }
mods-update = Update
mods-profile = Profile
mods-profile-rename = A new name for this profile
mods-profile-new-name = A name for the new profile, a copy of this one
mods-profile-new = New…
mods-profile-rename-button = Rename…
mods-folder = Mod folder
mods-folder-placeholder = the game's own folder
mods-folder-known = As Zendo knows
mods-state-no = Not applied: the game is as it was
mods-state-applied = { $count ->
    [one] Applied: { $count } mod in the game
   *[other] Applied: { $count } mods in the game
}
mods-state-applied-plugins = { $count ->
    [one] Applied: { $count } mod and the plugin order in the game
   *[other] Applied: { $count } mods and the plugin order in the game
}
mods-state-changed = Changed since applied: Apply to put the list in
mods-state-running = The game is running: close it to apply or go back to vanilla
mods-back-to-vanilla = Back to vanilla
mods-apply = Apply
plugins-title = Plugins
plugins-note-dates = Apply writes this order to the game's plugin list below, and dates the plugins in it, which is how this game reads the order.
plugins-note = Apply writes this order to the game's plugin list below.
plugins-note-no-prefix = Start the game once, so its prefix is made; Apply writes this order there after.
plugins-none = No plugins in the game's Data folder or the mods on.
plugins-from = from { $name }
plugins-games-own = the game's
plugins-master = master
plugins-always = always loads
plugins-needs = needs { $names }, not on
plugins-before = loads before { $names }, which it needs
plugins-unreadable = its header cannot be read

## FOMOD installer

installer-installed = Installed { $name }
installer-installing = Installing…
installer-choose = Choose the options of { $name }
installer-cancelled = { $name } was not installed
installer-exactly-one = choose one
installer-at-most-one = choose one or none
installer-at-least-one = choose at least one
installer-all = all of these
installer-any = choose any
installer-required = { $name } (required)
installer-recommended = { $name } (recommended)
installer-not-usable = { $name } (not usable)
installer-may-not-work = { $name } (may not work)
installer-step = Step { $at } of { $steps } · { $name }
installer-needs = The mod says it needs files Zendo did not find; it may not work.
installer-back = Back
installer-install = Install
installer-next = Next
installer-no-description = No description.

## A game's page

game-back = ← Library
game-ready = Ready when you are
game-pin = Pin
game-unpin = Unpin
game-hide = Hide
game-unhide = Unhide
game-more = More
game-less = Less
game-technical = Technical
game-change-art = Change art
game-last-played = Last played { $when }
game-about = About
game-details = Game details
game-launch-options = Launch options
badge-update = UPDATE AVAILABLE
badge-hidden = HIDDEN
count-times = { $count ->
    [one] { $count } time
   *[other] { $count } times
}
fact-download-size = Download size
fact-asking = Asking the store…
fact-size-on-disk = Size on disk
fact-space-needed = Space needed (store's estimate)
fact-latest-version = Latest version
fact-latest-build = Latest build
fact-store-details = Store details
fact-store-failed = The store could not say: { $error }
fact-publisher = Publisher
fact-released = Released
fact-genres = Genres
fact-store = Store
fact-started-by = Started by
fact-platform = Platform
fact-version = Version
fact-install-size = Install size
fact-install-path = Install path
fact-updates = Updates
fact-cloud-saves = Cloud saves
fact-last-played = Last played
fact-never = Never
fact-play-time = Play time
fact-status = Status
fact-owner = Owned by
fact-recent-time = Last two weeks
fact-played-on = Played on
system-linux = Linux
system-windows = Windows
system-mac = macOS
system-deck = Steam Deck
fact-not-installed = Not installed
fact-started-from-zendo = Started from Zendo
fact-launch-command = Launch command
fact-zendo-id = Zendo id

## Running games

run-its-launcher = its launcher
run-starting-game = Starting { $title }…
run-handed = Handed { $title } to { $launcher }
run-running = { $title } is running
run-waiting = Waiting for { $launcher } to start { $title }…
run-fetching-runtime = Downloading the { $runtime } runtime for { $title }…
run-runtime-failed = { $title } is running without its anti-cheat runtime: { $error }
run-missed = Handed { $title } to { $launcher }; Zendo did not see it start
run-closed = { $minutes ->
    [0] { $title } closed
    [one] { $title } closed after a minute
   *[other] { $title } closed after { $minutes } minutes
}
run-asked-close = Asked { $title } to close; it may ask to save first
run-asked-end = Asked { $title } to end
run-play = Play
run-state-running = Running
run-state-starting = Starting…
run-stop = Stop
run-force-stop = Force stop
run-wine-prefix = Wine prefix
run-last-log = Last launch's log
run-open = Open

## ProtonDB

protondb-no-reports-yet = No Proton reports yet
protondb-reports = { $count ->
    [one] Proton: { $tier } from 1 report
   *[other] Proton: { $tier } from { $count } reports
}
protondb-native = PROTONDB NATIVE
protondb-native-about = Has a Linux version · { $reports } · opens ProtonDB
protondb-tier-about = { $reports } · opens ProtonDB
protondb-none = PROTONDB: NO REPORTS
protondb-none-about = No one has reported on it yet · opens ProtonDB

## Common words

save = Save
add = Add
edit = Edit
remove = Remove
really-remove = Really remove
choose = Choose…
choosing = Choosing…

## Games added by hand

local-no-record = Zendo has no record of this game
local-choose-program = Choose the game's program
local-added = Added { $title }
local-saved = Saved { $title }
local-removed = Removed { $title } from the library; its files are where they were
local-edit-game = Edit game
local-add-game = Add a game
local-about-empty = A game you installed yourself: its .exe for a Windows game, which runs through Proton in a prefix of its own, or the program for a Linux one.
local-about-windows = A Windows game: it runs through umu-launcher and Proton, in a prefix of its own.
local-about-linux = A Linux program: it runs as it is.
local-program = Program
local-program-placeholder = /home/you/Games/Some Game/Game.exe
local-title = Title
local-title-placeholder = Some Game
local-arguments = Arguments
local-arguments-placeholder = Passed to the game, as a shell splits them
local-cover-note = Its cover comes from SteamGridDB by its title when Settings has a key, and Change art on its page picks another. Proton, MangoHud and the rest are in its launch options.

## Launch options

options-proton-default = Default ({ $name })
options-proton-default-umu = Default (UMU-Proton, downloaded by umu)
options-proton-missing = { $name } (not installed; the default runs)
options-size-both = Give gamescope both a width and a height, or neither
options-saved = Saved { $title }'s launch options
options-title = { $title } · Launch options
options-proton-about = Runs the Windows game. GE-Proton carries the most fixes; Settings sets the default.
options-mangohud-about = An overlay with the frame rate, frame times and temperatures.
options-gamemode-about = Asks the system for full performance while the game runs.
options-gamescope-about = Runs the game in a compositor of its own: full screen, a size of your choice, for games that will not go full screen themselves.
options-xbox = Controllers as Xbox pads
options-xbox-about = For games that ignore a PlayStation or other controller. The game then gets no touchpad, motion or adaptive triggers from it.
options-width = Width
options-height = Height
options-full-screen = Full screen
options-arguments-about = Passed to the game, as a shell splits them.
options-environment = Environment
options-environment-about = NAME=value pairs for the game, separated by spaces.
options-not-installed = Not installed.
saves-around = Cloud saves
saves-around-about = Brings the cloud's newer saves before the game starts and sends the newer ones here after it ends, through legendary or gogdl. The game must have been started once.
saves-sync-now = Sync now
saves-upload = Upload
saves-download = Download
saves-really = { $action }: replace the other side?
saves-syncing = Syncing the saves…
saves-synced = The saves are in sync
cloud-off = Off (turn them on in Launch options)
cloud-not-yet = Not synced yet
cloud-after-first-start = They sync once the game has started
cloud-synced = In sync, { $when }
cloud-none = { $store } keeps no cloud saves for this game
cloud-failed = Failed { $when }: { $error }
options-offline = Offline
options-offline-about = Starts without signing in to Epic, for playing with no connection; the game's online parts and achievements stay off.
options-offline-not-approved = Epic has not approved this game for offline play: it may not start.
options-skip-update = Start while an update waits
options-skip-update-about = legendary refuses to start a game whose update is not installed; this starts the version installed.
options-eos-overlay = Epic's overlay
options-eos-overlay-about = Epic's EOS overlay over the game (friends, invites, achievements' pop-ups). Turned on in the game's prefix as it starts, from its second start.
options-eos-overlay-missing = Install it first in Settings.
overlay-title = Epic's overlay
overlay-about = Epic's EOS overlay, which legendary installs once for every Epic game; each game turns it on in its launch options.
overlay-working = Working with legendary…
overlay-is-installed = Installed in { $path }
overlay-not-installed = Not installed
overlay-no-legendary = legendary is needed: sign in to Epic on the Accounts page, or install legendary.
overlay-install = Install
overlay-update = Update
overlay-installed = Epic's overlay is installed
overlay-updated = Epic's overlay is up to date
overlay-removed = Removed Epic's overlay
options-gamescope-shows = Shows as
options-gamescope-window = A window
options-gamescope-borderless = A window without its frame
options-output-size = Its size
options-game-size = The game's
options-sizes-about = Empty: the game's own. A smaller size for the game is scaled up to gamescope's.
options-game-size-both = Give the game's size both a width and a height, or neither
options-scaler = Fit
options-filter = Scaling
options-gamescope-own = gamescope's own
options-scaler-auto = Auto
options-scaler-integer = Whole steps (pixel art)
options-scaler-fit = Fit inside
options-scaler-fill = Fill, cropping
options-scaler-stretch = Stretch
options-filter-linear = Smooth
options-filter-nearest = Nearest pixel
options-filter-pixel = Pixel art
options-gamescope-softest = 20 (softest)
options-gamescope-fps = Frames a second
options-gamescope-fps-empty = Empty: the screen's
options-gamescope-more = More options
options-gamescope-more-about = Any of gamescope's own, as a shell splits them: --adaptive-sync, --force-grab-cursor.
options-runtime = Steam Linux Runtime
options-runtime-about = Runs the game in one of Steam's containers, with the libraries it was built for: older native games often need 1.0 (scout).
options-runtime-none = None: the system's libraries
options-runtime-missing = { $name } (not installed)
options-no-umu = Without umu
options-no-umu-about = Zendo runs Proton itself in Steam Linux Runtime 4.0, as umu would: for when umu-launcher is missing or misbehaves.
options-no-umu-runtime = Steam Linux Runtime 4.0 is not installed: install it in Steam.
options-language = Language
options-language-about = The language legendary asks the game for, as a code: fr, de, pt-BR.
options-language-empty = Empty: the system's
options-task = Start with
options-task-about = The store's way to start the game, or another it lists (a launcher skipped, a mode without anti-cheat).
options-task-default = The store's own
options-task-gone = { $name } (no longer listed; the store's own runs)
options-task-none = The store lists no other way to start this game.
options-executable = Program
options-executable-about = Runs this program instead of the store's: one in the game's folder.
options-executable-empty = Empty: the store's
options-executable-browse = Browse…
options-executable-choose = Choose the game's program
options-before = Before the game
options-before-about = A command run before the game starts, as a shell would; the game waits for it, and does not start if it fails. It gets ZENDO_GAME_ID, ZENDO_GAME_TITLE, ZENDO_GAME_FOLDER and the game's WINEPREFIX.
options-after = After the game
options-after-about = A command run once the game has ended, with the same names; only while Zendo's window is open.
options-offload = NVIDIA offload
options-offload-about = Draws the game on the NVIDIA card of a computer with two (PRIME render offload), as on most laptops.
options-offload-none = This computer has no second card with NVIDIA's driver.
options-proton-switches = Proton
options-build-ignores = { $build } does not read { $names }.
options-wayland = Wayland driver
options-wayland-about = Wine draws through Wayland instead of X11: sharper on scaled screens, and needed for HDR outside gamescope.
options-hdr-about = For games that offer HDR, on a screen and desktop with it on; needs the Wayland driver or gamescope.
options-fsr-about = Wine's FSR 1: a full-screen game set below the screen's resolution is upscaled with AMD's FSR. Not with the Wayland driver.
options-fsr-sharpening = Sharpening
options-fsr-sharpest = 0 (sharpest)
options-fsr-softest = 5 (softest)
options-fsr-default = 2 (Wine's default)
options-fps-counter = FPS counter
options-fps-counter-about = DXVK's frame rate, top left, for Direct3D 8 to 11 games; MangoHud shows more.
options-frame-cap = Frame cap
options-frame-cap-about = The most frames a second the game draws, through DXVK and VKD3D-Proton.
options-frame-cap-empty = Empty: no cap
options-wow64-about = Runs 32-bit Windows code without the system's 32-bit libraries. Some older games run worse with it.
options-no-sync = Esync and fsync off
options-no-sync-about = For games that hang or crash with Wine's faster ways of waiting.

## More common words

installed = Installed
download = Download
stop = Stop
stopping = Stopping…
open = Open
close = Close
try-again = Try again
size-of = { $done } of { $total }

## Proton

protons-installed-note = { $name } is installed; launch options can choose it
protons-stopped = Stopped downloading { $name }
protons-removed-default = Removed { $name }; it was the default, so Automatic is used now
protons-removed = Removed { $name }
protons-umu = UMU-Proton, which umu downloads
protons-default-set = Games without a Proton of their own now run with { $name }
protons-automatic = Automatic: newest GE-Proton first ({ $name })
protons-automatic-umu = Automatic: UMU-Proton, downloaded by umu
protons-missing = { $name } (not installed; Automatic is used)
protons-default = Default
protons-default-about = For the Windows games whose launch options leave Proton as it comes.
protons-here = On this computer
protons-none = None yet: umu downloads UMU-Proton the first time a Windows game starts.
protons-releases = GE-Proton releases
protons-check-again = Check again
protons-asking = Asking GitHub…
protons-show-older = Show older versions
protons-about = Runs the Windows games Zendo installed; each game's launch options choose one. GE-Proton carries the most fixes, and goes into Steam's compatibility tools folder, where Steam and Heroic find it too.
protons-chosen-by = { $count ->
    [one] 1 game chose it; it will run with the default
   *[other] { $count } games chose it; they will run with the default
}
protons-downloading = Downloading { $name }
runtimes-title = Anti-cheat
runtimes-about = Games that ship Easy Anti-Cheat or BattlEye need its runtime to play online under Proton. Zendo uses Steam's when Steam has it, else downloads its own from Lutris the first time such a game starts.
runtimes-from-steam = From Steam
runtimes-downloaded-on = Downloaded by Zendo, built { $date }
runtimes-downloaded-undated = Downloaded by Zendo
runtimes-not-here = Not on this computer; downloaded when a game needs it
runtimes-update = Update to { $date }
runtimes-downloading = Downloading…
runtimes-asking = Asking Lutris…
runtimes-downloaded = The { $runtime } runtime is ready
runtimes-removed = Removed Zendo's copy of the { $runtime } runtime

## The art picker

art-no-game = SteamGridDB has no game called “{ $query }”; try another title
art-no-images = { $kind ->
    [cover] This game has no cover on SteamGridDB yet
   *[other] This game has no wide image on SteamGridDB yet
}
art-needs-key = Searching SteamGridDB needs a key: add yours in Settings
art-searching = Searching…
art-looking = Looking for images…
art-new = { $kind ->
    [cover] New cover for { $title }
   *[other] New wide image for { $title }
}
art-back-to-automatic = { $kind ->
    [cover] Back to the automatic cover for { $title }
   *[other] Back to the automatic wide image for { $title }
}
art-cover = Cover
art-hero = Wide image
art-search-placeholder = A title to search SteamGridDB for
art-search = Search
art-title = Art for { $title }
art-show-more = Show more
art-open-settings = Open Settings
art-use-automatic = { $kind ->
    [cover] Use the automatic cover
   *[other] Use the automatic wide image
}
art-by = { $about }, by { $author }

## Accounts

accounts-signed-in-to = Signed in to { $store } as { $name }
accounts-signed-out-of = Signed out of { $store }
accounts-closed = Closed the { $store } sign-in
accounts-no-web-view = { $error }. Sign in in your browser instead.
accounts-no-code-epic = No code there: paste the whole page Epic showed
accounts-no-code-address = No code there: paste the address the page ended on
accounts-about = Sign in to see and install the games you own. These sign-ins are Zendo's own, apart from Heroic's.
accounts-signed-out = Not signed in
accounts-signed-in-as = Signed in as { $name }
accounts-sign-out = Sign out
accounts-sign-in = Sign in
accounts-through = Through { $path }
accounts-gog-playtime = Send the time played to your GOG account
accounts-engine-missing = { $engine } not found: install it, or Heroic, which bundles it
accounts-preparing-amazon = Preparing Amazon's page…
accounts-signing-in = Signing in…
accounts-in-window = Sign in to { $store } in the window that opened; Zendo finishes by itself.
accounts-opening-window = Opening the sign-in window…
accounts-use-browser = Use my browser instead
accounts-step-one = 1. Sign in on the store's page, in your browser.
accounts-step-two-epic = 2. Epic then shows a page of text. Copy all of it, or the authorizationCode value.
accounts-step-two-gog = 2. GOG then shows an empty page. Copy its address from the address bar.
accounts-step-two-amazon = 2. Amazon then goes to its home page. Copy that page's address from the address bar.
accounts-step-three = 3. Paste it below. Ctrl+V works anywhere on this page.
accounts-paste-here = Paste it here
accounts-finish = Finish
accounts-open-again = Open the page again
accounts-preparing = Preparing…

## The store

install = Install
installing = Installing…
resume = Resume
queued = Queued
see-downloads = See downloads
badge-installed = INSTALLED
fact-platforms = Platforms
fact-developer = Developer
store-owned = { $count ->
    [one] 1 game owned
   *[other] { $count } games owned
}
store-installed = { $count ->
    [one] 1 installed
   *[other] { $count } installed
}
store-asking-stores = Asking { $stores } for your games…
store-search = Search your games
store-asking = Asking…
store-sign-in-first = Sign in to a store in Accounts to see the games you own
store-go-accounts = Go to Accounts
store-asking-all = Asking the stores for your games…
store-nothing = Nothing here
store-not-installed-only = Not installed only
store-open-in-library = Open in library
store-into = Into { $path }
store-installs-into = Installs into { $path }
store-install-with-steam = Install with Steam
store-steam-installs = Steam asks where to install it; Zendo follows the download and adds the game when it is done
store-steam-installs-needs = Steam asks where to install it; Zendo follows the download and adds the game when it is done · needs { $size }
store-installs-into-needs = Installs into { $path } · needs { $size }
store-with-dlc = Also install its DLC ({ $count })
store-with-dlc-size = Also install its DLC ({ $count }, { $size })

## The DLC card on a game's page

dlc-title = DLC
dlc-count = { $installed } of { $total } installed
dlc-install-all = Install all
dlc-comes-with = Comes with the game
dlc-not-installed = Not installed
dlc-in-heroic = Heroic installs and removes these.
dlc-sign-in = Sign in to { $store } in Accounts to see all the DLC you own.
dlc-asking = Asking the store about the DLC you own…
store-no-home = No home folder to install into
store-back = ← Store

## Downloads

pause = Pause
pausing = Pausing…
cancelling = Cancelling…
action-label = { $action ->
    [install] Install
    [update] Update
    [repair] Repair
    [add-dlc] Install DLC
    [remove-dlc] Uninstall DLC
   *[uninstall] Uninstall
}
action-doing = { $action ->
    [install] Installing
    [update] Updating
    [repair] Repairing
    [add-dlc] Installing DLC for
    [remove-dlc] Uninstalling DLC from
    [add-dlc-alone] Installing DLC
    [remove-dlc-alone] Uninstalling DLC
   *[uninstall] Uninstalling
}
downloads-paused-count = { $count ->
    [one] 1 download paused
   *[other] { $count } downloads paused
}
downloads-pausing = Pausing { $title }…
downloads-cancelling = Cancelling { $title }…
downloads-doing-percent = { $doing } { $title }, { $percent }%
downloads-doing-dots = { $doing } { $title }…
downloads-doing = { $doing } { $title }
downloads-cancelled-gone = Cancelled { $title }; its files are gone
downloads-cancelled-but = Cancelled { $title }, but { $error }
downloads-pausing-closing = Pausing the download, then closing…
downloads-cancelled-job = Cancelled: { $doing } { $title }
downloads-cancelled = Cancelled
downloads-already-queued = { $title } is in the queue already
downloads-ahead = { $count ->
    [one] { $action } { $title }: 1 ahead of it
   *[other] { $action } { $title }: { $count } ahead of it
}
downloads-paused-job = Paused: { $doing } { $title }
downloads-done-job = { $action } { $title }: done
downloads-failed-job = { $action } { $title } failed: { $error }
downloads-job-state = { $state ->
    [now] { $doing }: now
    [paused] { $doing }: paused
   *[queued] { $doing }: queued
}
downloads-press-again = Press again to delete its files
downloads-check-updates = Check for updates
downloads-checking-updates = Checking for updates…
downloads-up-to-date = Up to date
downloads-update-version = Update available: { $version }
downloads-update-available = Update available
downloads-update-failed = Could not check for updates: { $error }
downloads-nothing = Nothing to download
downloads-in-queue = { $count } in the queue
downloads-in-queue-paused = { $count } in the queue, { $paused } paused
downloads-empty = Install games from the Store and they show up here
downloads-go-store = Go to the Store
downloads-up-next = UP NEXT
downloads-paused = PAUSED
downloads-finished = FINISHED
downloads-clear = Clear
downloads-running-title = A download is running
downloads-running-about = Closing pauses it, with what it has so far; resume it from here next time
downloads-close-anyway = Close anyway
downloads-keep = Keep downloading
downloads-downloaded = { $size } downloaded
downloads-left = { $time } left
downloads-starting = Starting { $engine }…
downloads-failed = Failed: { $error }
downloads-done = Done

## The file dialog and the sign-in window

dialog-choose = Choose
dialog-archives = Archives
dialog-all-files = All files
sign-in-window-title = Sign in to { $store } · Zendo
sign-in-popup-title = Sign in · Zendo

## Theme

theme-title = Theme
theme-about = The window's colours: Zendo's own, a theme of your own (a file in { $folder }, read when Settings opens), or, on a computer with hyprmonk, hyprmonk's tone, which Zendo follows as it changes.
theme-hyprmonk = hyprmonk's tone
theme-gone = No theme called “{ $theme }” is left, so Zendo's own colours show.
theme-not-read = Not a theme: { $problem }
theme-missing = hyprmonk has written no tone, so Zendo's own colours show.
theme-following = Following hyprmonk's tone: { $tone }.
theme-failed = hyprmonk's tone could not be read, so the colours stay as they were: { $error }
