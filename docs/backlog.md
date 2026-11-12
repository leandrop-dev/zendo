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
30. Fedora package: an RPM spec and COPR (was 22), with the README's Fedora
    install path. The Arch package stays. The RPM bundles no engines
    (decided 2026-11-12 [tentative], see item 29): Zendo downloads them, so
    the package needs only Zendo and what it links. Item 29 comes first, or the
    package reaches other users as a Zendo with no engines. Item 29 adds run-time
    needs the spec must list: `python3` (umu-run), `tar` with xz support,
    `sha256sum` and `sha512sum`.
31. Redesign, the shell. First a throwaway spike: a transparent Iced 0.14
    window on KWin, checking whether the blur shows through; if it does not,
    the fallback is an opaque themed window, written down before going on.
    Then design tokens in `app/style.rs`, a bundled monospace font
    (JetBrains Mono, OFL-1.1), uppercase labels and flat bordered chips, and
    a top bar with Library, Store and Downloads tabs, search and a Settings
    button for Accounts, Settings and About. Replaces `sidebar.rs`.
32. Redesign, the Library and the game page, as in design.md, with Zendo's
    pins, hiding, categories, play time, achievements and ProtonDB.
33. Redesign, the remaining pages: Store, Downloads, Accounts, Settings,
    the launch options sheet, mods and Nexus.
34. Home and Game Mode, as in design.md. Game Mode replaces today's F11 full
    screen.
35. Organize: play status (backlog, playing, completed, abandoned), tags,
    saved filters, bulk edits and a random pick. Builds on item 19.
36. Play sessions and stats: a session history, a stats screen and a yearly
    summary.
37. Emulation (was 26, moved up): emulator profiles, ROM folders and
    console libraries in the library.
38. Descriptions and facts for every game, local ones too: Steam's store for
    any game matched to a Steam app id (was 24).
39. Library views beyond the grid: a list and a details view (was 25),
    unless 32 and 34 already cover them.
40. OptiScaler, last: niche (was 23).

## On hold

- Recompiled games (N64 recomps and the like) and their mods (was 21), with
  ideas from Quiver Launcher (checked 2026-11-07: it installs apps from
  GitHub and GitLab releases with community catalogs; Business Source
  License 1.1 from version 4, so ideas only, no code).
