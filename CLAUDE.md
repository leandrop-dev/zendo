# Zendo

Read `PLAN.md` first: what this project is for, the direction, the stack and
where it stands. It is short, and it is the only file to read up front.

The rest is read on demand, never all at once:

- `docs/backlog.md`: the work left, items 20 to 26 in the user's order. Read
  the item you are starting.
- `docs/history.md`: what was built, when, and what was checked. Read it when
  you need to know why something is the way it is, or before changing an area
  whose history matters.
- `docs/reference.md`: verified facts about the stores, the engines and their
  files. Read it before driving an engine, reading a launcher's files or
  calling a store's API.

Keep `PLAN.md` current as decisions are made. What is built and checked goes
into `docs/history.md`, dated, rather than growing `PLAN.md`.

## Checking the window

Say so before launching Zendo for a check: a start can scan the library,
fetch covers and queue auto-updates. `grim` saves a still
(`grim /tmp/opencode/zendo.png`), `wtype` types into it, and mouse input
goes through `ydotool` with `ydotool.service` running (`systemctl --user
start ydotool`) and Zendo focused. The virtual mouse reaches the whole
session, so use it with the user present.

After a layout change, click every new control once in the app. A control
can be drawn outside the area that receives clicks.
