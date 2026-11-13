# Zendo: the backlog

The remaining work. Read only the item you are starting. Items 1 to 20 are
built; their detail is in history.md.

## Reformulated 2026-11-12 [tentative until the user confirms the order]

Zendo moved to a Fedora 45 + KDE Plasma machine, and the user changed the
direction the same day:

- Generic platform: no Hyprland-specific code; Fedora first.
- The theme follows the active KDE colour scheme.
- A complete redesign in the style of omakade, built in Iced (ideas and
  look only; omakade is GPL-3.0-or-later, so no code). The analysis is in
  `docs/design.md`.
- Recompiled games are on hold.

Numbering continues from 27 so it never collides with the built items 1 to
20 cited in history.md and the commits.

27. Generic platform. Built on 2026-11-12 (docs/history.md, item 27): the
    tray restore without `hyprctl`, and no Heroic needed for GOG Galaxy.
    The hyprmonk tone went in 28. The engines' own
    downloader is item 29.
28. The KDE colour scheme as a theme. Built on 2026-11-12 (docs/history.md,
    item 28): "System" reads `~/.config/kdeglobals`; the hyprmonk tone is
    gone, and a saved `theme = "hyprmonk"` falls back to Zendo's own.
29. Built on 2026-11-12 (docs/history.md, item 29): Settings → Engines and
    Proton-CachyOS; the text below is the original plan, with the questions
    settled in history. Engines and Protons downloaded by Zendo, decided
    2026-11-12 [tentative until the user confirms]. Fedora 45 packages none of gogdl, nile, comet
    or umu-launcher and has legendary 0.20.34 (checked with `dnf repoquery`),
    so neither `Requires` nor a bundled copy in the RPM works well. Settings →
    Engines downloads and updates, from each project's GitHub release, as
    `ge_proton.rs` already does for GE-Proton: legendary, gogdl, nile, comet
    and umu-launcher, into a folder of Zendo's own, found before `PATH` or
    after it as the user chooses (`stores.rs` `bundled` looks on `PATH`
    first today). Settings → Protons gets, next to GE-Proton, CachyOS's
    (`CachyOS/proton-cachyos`; checked 2026-11-12: a release per build,
    `proton-cachyos-<version>-slr-x86_64.tar.xz` of about 340 MB, an
    `x86_64_v3` variant, an arm64 one, and a `.sha512sum` for each, so the
    download is checked as GE-Proton's is). The `x86_64_v3` build only when
    the CPU has it. The same downloader fetches comet's
    `dummy-service.zip` (already done in `galaxy.rs`, item 27). To settle
    when it starts: where the engines go, how updates are noticed, and how a
    Proton CachyOS names the unpacked folder (`proton.rs` already knows the
    `proton-cachyos-slr` name).
30. Built on 2026-11-12 (docs/history.md, item 30), COPR project still to
    be made by the user. Fedora package: an RPM spec and COPR (was 22), with the README's Fedora
    install path. The Arch package stays. The RPM bundles no engines
    (decided 2026-11-12 [tentative], see item 29): Zendo downloads them, so
    the package needs only Zendo and what it links. Item 29 comes first, or the
    package reaches other users as a Zendo with no engines. Item 29 adds run-time
    needs the spec must list: `python3` (umu-run), `tar` with xz support,
    `sha256sum` and `sha512sum`.
31. Built on 2026-11-12 (docs/history.md, item 31), not yet tried by the
    user. Redesign, the shell. First a throwaway spike: a transparent Iced 0.14
    window on KWin, checking whether the blur shows through; if it does not,
    the fallback is an opaque themed window, written down before going on.
    Then design tokens in `app/style.rs`, a bundled monospace font
    (JetBrains Mono, OFL-1.1), uppercase labels and flat bordered chips, and
    a top bar with Library, Store and Downloads tabs, search and a Settings
    button for Accounts, Settings and About. Replaces `sidebar.rs`.
32. Built on 2026-11-12 (docs/history.md, item 32), checked on screen with the user
    present, not yet tried by the user. Redesign, the Library and the game page, as in design.md, with Zendo's
    pins, hiding, categories, play time, achievements and ProtonDB.
33. Built on 2026-11-12 (docs/history.md, item 33), tests green, checked on
    screen in part (history.md lists what was not). Redesign, the remaining pages: Store, Downloads,
    Accounts, Settings, the launch options sheet, mods and Nexus.
34. Built on 2026-11-12 (docs/history.md, item 34), checked on screen with
    the user present except the controller. Game Mode, replacing today's F11
    full screen. The Home page of design.md was built and removed at the
    user's wish.
35. Built on 2026-11-12 (docs/history.md, item 35), tests green, checked
    on screen with the user present (history.md lists what was not). Organize: play status (backlog, playing, completed,
    abandoned), saved filters and bulk edits. Tags are item 19's categories
    (no second label system) and the random pick was dropped, both at the
    user's choice.
36. Built on 2026-11-13 (docs/history.md, item 36), tests green, checked
    on screen with the user present (history.md lists what was not). Play
    sessions and stats: a session history, a Stats tab
    with a summary per year and for all time, and a Sessions card on the game
    page. Sessions are the runs Zendo watched; there is no backfill.
37. On hold since 2026-11-13: emulation (see "On hold"). The number stays
    reserved.
38. Built on 2026-11-13 (docs/history.md, item 38), tests green, checked on
    screen with the user present and tried by the user. Descriptions and facts for every
    game, local ones too: Steam's store for any game matched to a Steam app
    id (was 24), with a Steam app id the user can give a game.
39. Library views beyond the grid: a list and a details view (was 25),
    unless 32 and 34 already cover them.
40. OptiScaler, last: niche (was 23).

## On hold

- Emulation (was 37, and 26 before that), put on hold by the user on
  2026-11-13 after planning it. Decided in that planning [tentative]:
  presets for common emulators plus custom profiles (a program and an
  argument template with `{rom}`); ROMs from folders the user adds, one
  console and one emulator per folder; a fifth tab, Emulation, with a card
  per console, then that console's ROM grid, then the normal game page, with
  the setup (emulators and ROM folders) on the tab; ROMs also under an
  Emulation chip in the Library; the user installs the emulators, as in
  omakade; checked with the RetroArch Flatpak. Ideas from omakade, not yet
  chosen (ideas only, it is GPL): RetroArch playlists as a source, a scan of
  the EmuDeck `~/Emulation/roms/<system>` layout, and covers from Libretro's
  thumbnails.
- Recompiled games (N64 recomps and the like) and their mods (was 21), with
  ideas from Quiver Launcher (checked 2026-11-07: it installs apps from
  GitHub and GitLab releases with community catalogs; Business Source
  License 1.1 from version 4, so ideas only, no code).
