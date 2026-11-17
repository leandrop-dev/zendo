# 0002. Register Zendo's Game Pass installs with Nexus Launcher

Date: 2026-11-17
Status: Accepted
Supersedes [0001](0001-install-game-pass-through-zendo.md)

## Context

ADR 0001 had Zendo install Game Pass games into its own folder with a
`.zendo-xbox.json` marker and launch them with `xodus-cli run <folder> <wine>`,
never writing Nexus Launcher's state. The live check of 2026-11-17 showed the
install itself works, but the launch does not: `run` exits in D3D setup
(`DXGI: CreateSwapChainForHwnd: Unsupported device type`, exit 3) because the
launcher, when it starts a game, prepares a per-game `compatdata/<id>` folder
with a `xodus-proton-game` runner, staged vkd3d-proton/DXVK components, the
prefix, PlayFab/Gaming Services registration and its environment. Reproducing
all of that in Zendo is a per-title compatibility project.

A hand-written record in `launcher-state.json` (backed up first) made Nexus
Launcher list the game, prepare `~/.local/share/xodus-nexus/compatdata/<id>`
and start it. The user's launcher-installed Minecraft Legends played normally;
Retro Classics crashed inside vkd3d-proton under the launcher as well, so that
title's failure is not caused by the record.

## Decision

Zendo will register a Game Pass game it installed in Nexus Launcher's
`launcher-state.json` when the install finishes, and start it through the
launcher's own `launch-game <id>` script, the same path used for games the
launcher installed. The `.zendo-xbox.json` marker stays as Zendo's record of
ownership: it decides which records Zendo may uninstall, which removes the
game's folder, its record and the launcher's `compatdata/<id>` folder. Zendo
refuses to change the state while Nexus Launcher is running (its
`SingletonLock` is there), because the launcher caches the state and would
write over it.

## Alternatives Considered

- **Keep the state read-only (ADR 0001) and build the launcher's launch
  environment in Zendo.** It keeps the launcher's file untouched, but it means
  reproducing a prepared runner, DLL staging, package registration and shader
  caches, title by title. The live check shows the first attempt fails.
- **Leave installs to Nexus Launcher (item 60).** It works and stays the
  fallback when the launcher is missing, but the user asked for installs from
  Zendo.
- **Use `xodus-cli run` only when the launcher is absent.** It cannot start a
  title on this machine, so a game installed by Zendo would be unplayable;
  rejected rather than shipped as a broken Play button.

## Consequences

- Zendo now writes a private file of another program. The record's shape is
  learned from the launcher's own records (v0.5.0, backend 0.2.15) and may
  change with a launcher update; a failed write must leave the old state and
  say so, and the game's install is reported as failed rather than listed as
  playable.
- Installing requires Nexus Launcher to be closed; the Store page and the job
  say so. Uninstalling writes the state too, under the same rule.
- The launcher owns compatibility, updates and repair for these games from
  then on; item 60c found no CLI update or uninstall, so the launcher's window
  is the place for both. Zendo's own uninstall removes the game it made.
- The library reads Zendo's installs from the launcher's record like any other
  Game Pass game (`launch-game <id>`), so `Launch::Xodus` and the raw-Wine
  launch path go away; the marker is no longer a library source.
- The launcher's `compatdata/<id>` folder survives a launcher-side removal;
  Zendo's uninstall removes the one for the record it created.
