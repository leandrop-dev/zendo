# 0001. Install Game Pass games through Zendo

Date: 2026-11-17
Status: Accepted

## Context

Game Pass games were read-only in Zendo: Nexus Launcher installs them into
its own folder, records them in `launcher-state.json`, and Zendo lists and
starts them through the launcher (item 58). Item 62 asks whether Zendo can
install them itself.

Nexus Launcher's backend (`xodus-cli`) can: `streaming <product> <folder>`
downloads and extracts a Store package without the launcher, with a
`NEXUS_PROGRESS` JSON protocol and pause/resume. It writes nothing to
`launcher-state.json`, so the launcher neither lists nor launches what it
installs. `run <folder> <wine>` starts such a folder and needs a Wine; the
launcher already ships Proton builds under
`~/.local/share/xodus-nexus/runtimes/`. The CLI has no uninstall and no
update command (item 60c).

Zendo's download queue is built on `downloads::Job`, keyed by `StoreKey`
(Epic, GOG, Amazon) and run by the store engines. Game Pass is not a store
with an engine; it is a shop with a backend.

## Decision

Zendo will install Game Pass games into its own games folder, under
`GamePass/<title>`, through the existing one-at-a-time queue, and record each
install with a `.zendo-xbox.json` marker in the game's folder. Games whose
marker exists are read back into the library as `Source::Xbox` games with
their own launch (`xodus-cli run <folder> <wine>`), and Zendo can uninstall
the ones it installed by deleting that folder. The queue's `Job` identifies
its shop by `Shop` rather than `StoreKey` (with a serde alias so old
`downloads.json` files still load), and only engine jobs require a `Store`.
The Wine comes from the launcher's own runtime; no launcher runtime, no
install.

## Alternatives Considered

- **Leave installs to Nexus Launcher (item 60)**, the current behaviour. It
  works, but the user asked for installs from Zendo; kept as the fallback
  when the backend or runtime is missing.
- **Write `launcher-state.json` so the launcher sees Zendo's installs.** It
  would make one manager, but that file is the launcher's private state, its
  full schema is unknown, and a running launcher can overwrite the write.
  Zendo stays read-only there.
- **One central Zendo state file for Game Pass installs.** Simpler to write,
  but a moved or renamed folder orphans the record. The marker travels with
  the game.
- **A second queue for Game Pass jobs.** Avoids changing the job format, but
  two queues can run at once, and the Downloads page would have to merge and
  order them. One queue with one job shape is less machinery.
- **Zendo's own Proton or umu for `run`.** The command expects a Wine, and
  the launcher's runtime is already required by `streaming`; Zendo's Proton
  was left for later if the launcher's runtime proves unsuitable.

## Consequences

- `$XDG_STATE_HOME/zendo/downloads.json` writes jobs with `shop` where it
  used to write `store`; the alias keeps existing queues readable.
- Zendo becomes a second manager of Game Pass installs, but only of folders
  it wrote and recorded. Nexus Launcher never sees them, cannot update or
  repair them, and Zendo cannot update them either (no version in the CLI;
  item 60c). Updates stay the launcher's for its own installs.
- Zendo must refuse a second install of a game the launcher or Zendo already
  has, because both use the same library id (`xbox:<storeId>`).
- Installing depends on Nexus Launcher's backend and runtime being present;
  the Store page says so when they are not.
- Free space is checked before a job starts, from `resolve-package`'s
  `downloadBytes` against the games folder's `df`.
- DLC is out of scope: the entitlement list does not give Zendo a game's DLC.
