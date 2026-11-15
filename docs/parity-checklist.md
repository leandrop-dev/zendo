# Item 54: the parity checklist

Written 2026-11-15 (item 54a). It is the gate before the Iced crate is deleted
(54b): the user signs it off. Nothing here has been run on screen yet; the
first section is what a read of the code found, the rest is what the earlier
items left for this item.

## 1. Differences found by reading the code

Method: every Fluent message id in `crates/zendo-shell/i18n/en.ftl` (996) was
looked up in the Iced crate and in the Slint crate, shell and core. Eleven are
used by Iced only. Each is below with what was found in Slint.

| Message | Iced shows | Slint | Action |
|---|---|---|---|
| `about-built-iced`, `about-licence-text` | About's "built with Iced" and MIT line | `about-licence-text-slint` replaces them (`ui/about.slint:98`) | none; delete the two ids with the Iced crate |
| `badge-update` | an "UPDATE AVAILABLE" badge in the game page's header | ported 2026-11-15 (`update-badge` in `ui/game.slint`, set in `game_jobs_ui.rs`) | not seen on screen |
| `game-gone`, `owned-gone` | a note when the open game leaves the library or the store list | ported 2026-11-15: the page closes and the footer says it (`refresh_game`, `show_owned`); test added for the library case | library case seen on screen (second run); the store case not tested or seen |
| `sources-unreadable` | one line per unreadable library source under Settings → Library | ported 2026-11-15: `Browser` keeps the lines, Settings lists them (`settings_ui.rs`); the footer keeps its count | seen on screen (second run) |
| `images-none` | "No images to download" after Download now with nothing left | ported 2026-11-15: the Download now handler says it in the footer when nothing started (`settings_ui.rs`) | seen on screen (second run) |
| `panicked-with` | the panic's own text in a failure | ported 2026-11-15: `panicked_note` in `library_ui.rs`, used by the scan, the cards and Nexus work; unit test | not seen on screen |
| `mods-reading` | "Reading…" while mods load | not ported, on purpose: `show_mods` draws no card until the mods are read, because a card that grows after the page is laid out left the page's height short of it (52c1) | none, a deliberate difference |
| `run-open` | an Open button beside a path in the Wine prefix card | `files-open-folder` is used instead (`game.slint:782`) | none, same behaviour |
| `hints-stats` | the Stats page's key hint line | was missing; fixed | seen on screen (second run) |

### Run on screen, 2026-11-15

Two runs, each in a throwaway home with four to six seeded desktop games and
no session bus; keys and clicks only after a pid check, everything closed and
deleted afterwards.

- **First run (before the ports):** with Test Game 01's page open and its
  `.desktop` file removed, a rescan (F5) returned to the Library with no word
  on why; the Stats footer never showed the period hint. Both differences from
  Iced were confirmed.
- **Second run (after the ports), all seen:**
  - the Stats footer reads "← → period · Esc back · F11 Game Mode";
  - the same removal now leaves "This game is no longer in the library" in the
    footer;
  - an unreadable `.desktop` file (mode 000) gives "1 file could not be read;
    Settings names it" in the footer and, in Settings → Library, a red line
    under Sources with the path and the reason;
  - Download missing images now, with nothing to fetch, says "No images to
    download" in the footer.
- **Not seen:** the Update badge (needs an installed store game with an
  update), the store game's "gone" note, the panic text (a unit test covers
  it).
- Page keys: Ctrl+1 Library, Ctrl+3 Downloads, Ctrl+4 Accounts, Ctrl+5
  Settings, Ctrl+6 About, Ctrl+7 Stats. A click on a scrollbar track or Page
  Down did not scroll Settings; dragging the thumb did.

## 2. Left by items 49 to 53 for this item

From `docs/backlog.md`; the user's own "to try" lists there are the long form.

- 49: the whole of the list at the top of item 49.
- 51: the Store, Downloads, Install, Import and Steam installs, with real
  accounts and a real engine. `move_game` in `zendo-core` `downloads.rs` can
  delete the old folder without checking the new one exists.
- 52a: a failed Save on a field scrolled out of sight says nothing at the
  footer (Iced is the same).
- 52c1: Iced shows "Reading mods…", Slint draws no card until read; the bottom
  of the Mods card could not be scrolled to once on Stardew Valley.
- 52c2b: Page on Nexus, Get on Nexus and a Premium Download not seen.
- 52c3: the waiting installer slot, the Downloads card of mods coming in, the
  pad and pt-PT not seen.
- 53: a real controller, a hero with a Steam summary and a light scheme not
  seen.
- Everywhere: pt-PT, a light KDE scheme, a window narrower than a page needs,
  a controller, the tray with the session bus.

## 3. Iced files that go with the crate

`crates/zendo/` whole (`app.rs` and `app/*.rs`, 24,000 lines or so). Its
`game_mode.rs` and `hero.rs` were never switched to `zendo_shell::game_mode`
(item 53b), so there is nothing to keep from them. `zendo-shell` re-exports
the toolkit-free modules and stays.

## 4. The cut-over itself (54b, after the sign-off)

1. Rename the crate `zendo-slint` to `zendo` (package, directory, workspace
   members, the `Cargo.lock` entry), delete `crates/zendo`.
2. Drop `iced` from `[workspace.dependencies]` and what only it used (check
   each with `cargo tree -i`); the `gtk` and `wry` pins stay if the sign-in
   window still uses them.
3. `packaging/fedora/zendo.spec`: licence `MIT AND OFL-1.1 AND GPL-3.0-only`,
   build dependencies Slint needs; `packaging/arch/zendo-git`; `data/zendo.desktop`.
4. `README.md`, `PLAN.md` (stack, "Where it stands"), `docs/backlog.md`,
   `docs/history.md`, `CLAUDE.md` (the Iced mentions).
5. Checks: `cargo build`, `cargo test --workspace`, `cargo clippy --workspace`,
   an RPM mock build, one launch each of a Steam, an Epic and a desktop game.
6. COPR resubmit: the user's command (`docs/backlog.md`, "Waiting on the
   user"). Claude does not run it.
