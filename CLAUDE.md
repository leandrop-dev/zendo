# Zendo

Read `PLAN.md` first: what this project is for, the direction, the stack and
where it stands. It is short, and it is the only file to read up front.

The rest is read on demand, never all at once:

- `docs/backlog.md`: the work left, items 38 to 40 in order. Read the item
  you are starting.
- `docs/design.md`: the redesign (omakade's ideas, no code). Read it before
  working on the window's look or pages.
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
fetch covers and queue auto-updates. Run it only in a throwaway home with
no session bus (`env -i ... HOME=<scratch> DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent
target/debug/zendo`): Zendo reads its API keys from the keyring at start,
and on the real bus that is the user's real KDE Wallet.

On KDE Plasma (Fedora), checked 2026-11-12:

- A still of the focused window: `spectacle -b -n -a -o FILE`.
- Finding and driving the window: `kdotool search --class zendo`, then
  `getwindowpid`, `getwindowgeometry`, `windowminimize`, `windowactivate`,
  `windowclose`. `kdotool getactivewindow getwindowpid` is the focus check.
- Keys and the mouse: `ydotool`. Fedora's `ydotool.service` is a root
  system unit; instead run `ydotoold --socket-path=$XDG_RUNTIME_DIR/ydt.sock
  --socket-perm=0600` as the user (the user has an ACL on `/dev/uinput`),
  set `YDOTOOL_SOCKET` to that path, and stop it afterwards. Keep the path
  short: a socket path over 108 characters fails, and the scratchpad's is
  too long. Send a key only when the active window's pid is Zendo's. This
  keyboard is shared with another computer through Deskflow.
- `grim` and `wtype` are untested on KWin; they need wlroots protocols.

The virtual mouse and keyboard reach the whole session, so use them with
the user present.

After a layout change, click every new control once in the app. A control
can be drawn outside the area that receives clicks.
