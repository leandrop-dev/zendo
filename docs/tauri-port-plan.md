# Zendo: the port from Slint to Tauri

The plan for replacing the Slint window with a Tauri 2 shell and a SvelteKit
frontend. Work happens on the branch `f/ui-revamp`; `main` stays releasable and
the Slint window stays in place until the parity checklist is signed off.

## Decision [tentative until the slices are signed off]

- The UI moves to **Tauri 2 + SvelteKit (SPA) + Svelte 5 + Tailwind CSS 4 +
  shadcn-svelte**, with `zendo-core` as the Rust backend.
- The translucent window is **dropped**. It was a Slint-only nicety; the goal
  is design flexibility and features working, not the glass effect.
- The binary licence simplifies: Slint's GPLv3 requirement goes away, and
  Tauri (MIT/Apache) with the system WebKitGTK (LGPL) allows an MIT/Apache
  binary, subject to the same font and asset checks as today.
- The Slint window (`crates/zendo`) is not deleted until the parity checklist
  passes, as with the Iced to Slint cut-over (items 47 to 54).

## Why Tauri, with the spike's evidence (2026-11-18)

The spike at `/home/leandrop-dev/Projects/zendo-tauri-spike` measured the same
grid (2,700 covers, 192x370 cards, 16px gaps) and confirmed:

| Check | Result |
|---|---|
| Idle, virtualized scroll, full DOM | 60 fps each; one 18 fps hitch mounting 2,700 cards |
| Real backend data over IPC | `zendo list --all --json` returned the 3 real games |
| Controller | real DualSense D-pad and face buttons moved the cursor |
| Keyboard | arrows moved the same cursor |
| Binary | 13 MB, links the system WebKitGTK, no bundled browser |

The Dioxus webview held 51 to 58 fps virtualized and 40 to 50 full DOM, could
not render a translucent window on NVIDIA/Wayland through four workarounds, and
its native renderer is too young. Tauri won on stability, speed and tooling.

## Architecture

- `zendo-core` (unchanged): scan, launch, stores, mods, state, settings,
  covers, sessions. This becomes the Tauri backend.
- `crates/zendo-tauri` (new workspace member): the Tauri shell, the commands,
  the events, the tray, the controller, the keyring, the window.
- `crates/zendo-tauri/ui` (new): the SvelteKit frontend.
- `zendo-shell`: keeps the parts that need no toolkit and are not UI state:
  tray (`ksni`), controller (`gilrs`), links, the command line, and file
  helpers. The page words and the per-page sheet/job state move to the
  frontend or are reworked as commands.
- The frontend calls Rust through `#[tauri::command]`; Rust pushes changes
  (scan progress, job progress, game started or stopped, pad input, language)
  through Tauri events.

## Frontend stack

- SvelteKit 3 with `@sveltejs/adapter-static` 4, `fallback: 'index.html'` and
  `ssr = false` in the root layout (SPA mode; Tauri has no server).
- Svelte 5 (runes) and TypeScript.
- Tailwind CSS 4, shadcn-svelte (Bits UI primitives, Lucide icons,
  `@lucide/svelte`), initialized with `npx shadcn-svelte@latest init`.
- State in Svelte runes and stores; a thin typed API client over `invoke`.
- Routing: file-based routes for Library, game page, Store, Downloads, Stats,
  Settings, Accounts and About.
- Performance: a windowed list (only visible rows), `loading="lazy"` for
  offscreen images, and no re-render on scroll beyond the visible window.
  The spike showed 60 fps with this shape; keep it.

## Repository layout

```
crates/zendo-core/          backend, unchanged
crates/zendo-shell/         tray, controller, links, command line, file helpers
crates/zendo/               Slint window, kept until parity
crates/zendo-tauri/         new: Tauri shell (Cargo crate, workspace member)
crates/zendo-tauri/ui/      new: SvelteKit frontend
crates/zendo-tauri/tauri.conf.json
crates/zendo-tauri/capabilities/
```

`tauri.conf.json`:

- `build.frontendDist`: `ui/build`
- `build.devUrl`: `http://localhost:1420` (Vite on 1420, strict port)
- `build.beforeDevCommand`: `npm --prefix ui run dev`
- `build.beforeBuildCommand`: `npm --prefix ui run build`
- `app.withGlobalTauri`: `true`
- `identifier`: `dev.leandrop.zendo`
- `productName`: `zendo` (the window class on Wayland; the blur rules key on
  it, even though translucency is dropped)
- `bundle.active`: `false` while developing; the release pipeline turns it on

## Backend contract

Commands, each a thin wrapper over an existing `zendo-core` or `zendo-shell`
function, with results typed for the frontend:

- Library: `library_list`, `library_rescan`, `cover_get`, `covers_download`,
  `local_add`, `local_remove`.
- Launch and state: `launch`, `launch_options_get`, `launch_options_set`,
  `state_set_pinned`, `state_set_hidden`, `state_set_status`,
  `collections_*`, `sessions_*`, `stats_*`.
- Settings: `settings_get`, `settings_set`, `key_check`, `key_set`,
  `key_remove`, `theme_get`, `diagnostics_report`.
- Stores and accounts: `accounts_*`, `engines_*`, `proton_*`, `store_*`,
  `downloads_*`.
- Mods and Nexus: `mods_*`, `nexus_*`, `nexus_search`, `installer_*`.
- Events: `scan_progress`, `job_progress`, `game_started`, `game_stopped`,
  `pad`, `language_changed`.

The spike proved the boundary with `zendo list --all --json` and a cover
listing; the rest is mechanical mapping.

## Slices

One commit per slice, each with a parity checklist line, as items 47 to 54 did
for Slint. Do not port a page the user has not asked for (a Home page was built
and removed once already).

0. **Scaffold**: branch, `crates/zendo-tauri`, SvelteKit, Tailwind,
   shadcn-svelte, Tauri dev loop, window and title, a placeholder page.
1. **Shell**: top bar, tabs, view and sort chips, theme (KDE scheme through
   `zendo-core`, Zendo's own fallback), i18n, tray hiding, controller, keyboard,
   fullscreen toggle, the command line commands (`list`, `launch`, `hide`,
   `unhide`, `covers`). Built 2026-11-18, except the view and sort chips and
   the search field: they act on the Library's `Browser` state, so they come
   with slice 2, where that state moves to `zendo-shell` and the grid is
   ported.
2. **Library**: grid with virtualization, chips, search, sort, views, marks and
   bulk edits, the cursor (keyboard and pad), cover downloads, the file watch,
   collections. Built 2026-11-18 as **2a** (the grid, the chips, the search,
   the sort, the views, the cards with covers over the asset protocol and the
   cursor) and **2b** (the marks and bulk edits, the file watch, the cover
   downloads and the FPS harness). The Collections chip waits for its sheet,
   which is slice 4's.
3. **Game page**: header, actions (Play, Stop, Favorite, Hide, Manage), status,
   tiles, sessions, details, local games, Steam facts and requirements, DLC,
   ProtonDB, achievements, Wine prefix, GOG changelog, cloud saves, the Mods
   card. Built 2026-11-18 as **3a** (the route, the header, Play/Stop, Favorite,
   Hide, the status chips and the tiles, over `zendo_shell::running`) and
   **3b** (sessions recorded and shown, and the details card) and **3c** (local
   games: the add/edit sheet, the file dialog, and Remove). **3d** carries
   Steam's facts and requirements, DLC, ProtonDB and achievements (none
   checkable without a Steam game or a store account); **3e** the Wine prefix,
   GOG's changelog, cloud saves and the Mods card.
4. **Settings, Accounts, About**: General, Keys, Library, Engines, Proton,
   Theme, Diagnostics; Epic, GOG and Amazon and the Xbox card; About; the
   Collections sheet.
5. **Store and Downloads**: the Store page, a store game's page, Steam's card,
   installs, the queue, updates, the close-while-running question.
6. **Stats and Game Mode**: the Stats tab and Game Mode (F11 and Select, hero,
   carousel, shelves, the Start menu, `--game-mode`).
7. **Sheets**: launch options, artwork chooser, FOMOD installer, Nexus and
   Nexus search, the screenshot lightbox.
8. **Parity and cut-over**: the parity checklist signed off, the Slint crate
   deleted, `cargo` and packaging updated, the licence and spec checked, the
   README and docs updated.

## Tooling

Present: Node 26, npm 12, Rust, WebKitGTK 4.1, GTK3, libsoup3, librsvg,
ayatana-appindicator, openssl.

To add, mostly as npm dev dependencies in `ui/`:

- `@tauri-apps/cli`, `@tauri-apps/api`
- `@sveltejs/adapter-static`, `svelte-check`
- `tailwindcss`, `@tailwindcss/vite` (Tailwind 4 via `sv add tailwindcss`)
- `shadcn-svelte` (CLI), `bits-ui` (pulled by shadcn-svelte)
- `prettier` and `prettier-plugin-svelte`, `eslint` and `typescript-eslint`
- `vitest` and `@testing-library/svelte` for unit tests
- optionally `playwright` for the frontend in a browser, and `tauri-driver`
  with `webdriverio` for the packaged app

Optional: `pnpm` (npm is enough), `cargo-deny` or `cargo-about` for licence
auditing.

On Fedora, for the package build: `webkit2gtk4.1-devel`,
`libappindicator-gtk3-devel` (or the ayatana equivalent), `librsvg2-devel`,
`openssl-devel`.

## MCPs, skills and plugins in OpenCode

- **Context7** (already enabled): Svelte, SvelteKit, Tailwind, shadcn-svelte,
  Bits UI, Tauri. This is the one that matters for a fast-moving frontend.
- **Browser automation** (optional): a Playwright or Chrome DevTools MCP to
  inspect the Svelte app in a browser during `vite dev`. The real target is
  WebKitGTK, so the Tauri window is still the source of truth; screenshots and
  the logs worked well in the spike.
- **Skills**: the existing `rust-best-practices` and `rust-review` cover the
  backend. There is no Svelte or Tauri skill installed; use `find-skills` to
  look for community ones, or `skill-creator` to add a small project skill
  that records the port's conventions (Svelte 5 runes, shadcn-svelte usage,
  the command and event patterns).
- **Plugins**: `@upstash/context7-opencode` adds a skill alongside the MCP;
  optional, since the remote MCP is already in `opencode.jsonc`. There is no
  official Tauri MCP; the Tauri CLI is the tooling.

## Risks and mitigations

- **Framework overhead on the grid**: keep the windowed list and measure with
  the FPS harness after each slice. Svelte's compiler helps; avoid reactive
  statements over the whole item list.
- **WebKitGTK differences**: test in the Tauri window, not only in the browser.
  The spike already showed it renders the same grid at 60 fps.
- **i18n**: decide early. Keeping Fluent in Rust and exposing `tr` over IPC
  reuses the translations; moving to a JS i18n library is more work but avoids
  an IPC call per string. Recommendation: keep Fluent in Rust and send a
  language bundle at startup, with `tr` for dynamic messages.
- **Two languages**: keep the backend contract thin and generate TypeScript
  types from Rust (`ts-rs`, or `tauri-specta`) so the boundary cannot drift.
- **Packaging**: the Tauri bundler plus the Arch PKGBUILD and Fedora COPR; the
  run-time libraries change from Slint's to WebKitGTK's. Update the spec,
  README and the licence line.
- **API churn**: pin SvelteKit 3, Svelte 5, Tailwind 4 and Tauri 2; they are
  stable lines.

## Decisions (settled 2026-11-18)

- **SvelteKit in SPA mode**: SvelteKit 3 (changed from 2 with the user on
  2026-11-18, since 3 is the line the CLI and `sv` now scaffold) with
  `@sveltejs/adapter-static` 4, `fallback: 'index.html'` and `ssr = false` in
  the root layout. Not plain Vite plus Svelte.
- **i18n stays in Rust**: Fluent in `zendo-shell`, exposed to the frontend
  over IPC. The frontend does not reimplement the translations, so the two
  `.ftl` files stay the single source of words.
- **Tray stays `ksni`**, in `zendo-shell`.
- **Controller and tray stay in `zendo-shell`**, not in `zendo-tauri`.
- Branch: `f/ui-revamp`. The plan is its first commit; the port started with
  slice 0 on 2026-11-18, and slice 1 the same day.

## First steps

1. Scaffold `crates/zendo-tauri` and `crates/zendo-tauri/ui` (SvelteKit,
   Tailwind, shadcn-svelte, Tauri dev loop). Slice 0, built 2026-11-18.
2. Port the shell: words, colours, tray, controller, keyboard, full screen and
   the command line. Slice 1, built 2026-11-18.
3. Port the Library: the grid, chips, search, sort, cursor, marks, bulk
   edits, cover downloads and the file watch. Slice 2, built 2026-11-18 (2a
   and 2b); the Collections sheet is slice 4's.
4. The FPS harness is in (`ZENDO_FPS`); add the parity checklist and proceed
   slice by slice. Slice 3 is in parts: 3a, 3b and 3c built; 3d and 3e next.
5. Keep the Slint window until the checklist is signed off.

## Handoff for the next session

- Branch `f/ui-revamp`. Slices 0, 1, 2, 3a, 3b and 3c are in place: the
  crate, the SvelteKit app, the dev loop, the shell, the Library, the game
  page's header, actions, sessions and details, and local games. Its commands
  and checks are in `crates/zendo-tauri/README.md`.
- The two spikes are outside the repo and can be deleted now that the port has
  started: `/home/leandrop-dev/Projects/zendo-dioxus-spike` and
  `/home/leandrop-dev/Projects/zendo-tauri-spike`.
- Next is slice 3d: Steam's facts and requirements, DLC, ProtonDB and
  achievements. These ask the stores or the network, so they need a machine
  with a Steam game and an account to check them; this one has neither. Then
  3e: the Wine prefix, GOG's changelog, cloud saves and the Mods card. The
  Library's Collections chip waits for slice 4's sheet.
- The FPS harness is `ZENDO_FPS=1`, with the seeder at
  `crates/zendo-tauri/scripts/seed-library.py` for a large throwaway library;
  it measured 62 fps on 2,700 games in a debug build. `history.md` has the
  runs.
- The last runs could not check clicks or keys: this session's `ydotool` and
  `wtype` delivered nothing. The user tested the shell's input and tray in
  their own session.
- The decisions above are firm; do not reopen them without the user.
