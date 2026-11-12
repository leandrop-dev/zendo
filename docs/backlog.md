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

27. Generic platform. Remove the `hyprctl`/hyprmonk code: the tray restore
    (`crates/zendo/src/app.rs`, `show_from_tray`), `tone.rs`, the hyprmonk
    choice in `app/themes.rs`, `config.rs`, `theme.rs` and the i18n
    strings. Tray restore on Wayland: first check whether ksni passes an
    xdg-activation token; otherwise close the window on hide and reopen it
    on restore (`iced::daemon` instead of `iced::application`).
    No Heroic needed: `galaxy.rs` `install_service` also looks for
    `GalaxyCommunication.exe` outside Heroic, in a Zendo data folder filled
    from comet's `dummy-service.zip`; the messages that say "it comes with
    Heroic" name where to get each tool instead; the README and
    reference.md list the standalone releases (legendary, gogdl, nile,
    comet, umu-launcher). Heroic's folders stay as a fallback and its
    installed games are still imported. Later [tentative]: Settings →
    Engines downloads and updates the engines itself, as `ge_proton.rs`
    does for GE-Proton.
28. The KDE colour scheme as a theme. A `kdeglobals` reader in zendo-core
    replaces `tone.rs`, reusing `theme.rs`'s mixing and the file watch.
    "System" becomes a theme choice next to Zendo's own. plasmonk keeps its
    surfaces fixed dark greys and tints only the accent, links and
    selection, so the result is an accent-tinted Zendo.
29. Fedora package: an RPM spec and COPR (was 22), with the README's Fedora
    install path. The Arch package stays.
30. Redesign, the shell. First a throwaway spike: a transparent Iced 0.14
    window on KWin, checking whether the blur shows through; if it does not,
    the fallback is an opaque themed window, written down before going on.
    Then design tokens in `app/style.rs`, a bundled monospace font
    (JetBrains Mono, OFL-1.1), uppercase labels and flat bordered chips, and
    a top bar with Library, Store and Downloads tabs, search and a Settings
    button for Accounts, Settings and About. Replaces `sidebar.rs`.
31. Redesign, the Library and the game page, as in design.md, with Zendo's
    pins, hiding, categories, play time, achievements and ProtonDB.
32. Redesign, the remaining pages: Store, Downloads, Accounts, Settings,
    the launch options sheet, mods and Nexus.
33. Home and Game Mode, as in design.md. Game Mode replaces today's F11 full
    screen.
34. Organize: play status (backlog, playing, completed, abandoned), tags,
    saved filters, bulk edits and a random pick. Builds on item 19.
35. Play sessions and stats: a session history, a stats screen and a yearly
    summary.
36. Emulation (was 26, moved up): emulator profiles, ROM folders and
    console libraries in the library.
37. Descriptions and facts for every game, local ones too: Steam's store for
    any game matched to a Steam app id (was 24).
38. Library views beyond the grid: a list and a details view (was 25),
    unless 31 and 33 already cover them.
39. OptiScaler, last: niche (was 23).

## On hold

- Recompiled games (N64 recomps and the like) and their mods (was 21), with
  ideas from Quiver Launcher (checked 2026-11-07: it installs apps from
  GitHub and GitLab releases with community catalogs; Business Source
  License 1.1 from version 4, so ideas only, no code).
