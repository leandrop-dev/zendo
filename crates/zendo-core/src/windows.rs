//! Asking a game's windows to close, as their X button would, so the game
//! can save and quit by itself. Windows games under Proton draw through X11
//! (`XWayland` on the desktop, or gamescope's own X server, whose `DISPLAY`
//! the game's processes carry), and Wine turns `WM_DELETE_WINDOW` into the
//! game's own close. Windows are matched by `_NET_WM_PID`, which Wine and
//! most toolkits set; umu's container shares the system's process ids
//! (pressure-vessel's `--share-pid`, its default), so they are the ones
//! Zendo sees.

use std::collections::HashSet;
use std::fs;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, MapState, Window,
};

/// Asks every mapped window of these processes to close, on each X display
/// they use; how many were asked.
pub fn ask_to_close(pids: &HashSet<u32>) -> usize {
    displays(pids)
        .iter()
        .map(|display| on_display(display, pids).unwrap_or(0))
        .sum()
}

/// The X displays the processes use, from their environment, and Zendo's
/// own.
fn displays(pids: &HashSet<u32>) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let own = std::env::var("DISPLAY").ok();
    let theirs = pids.iter().filter_map(|pid| {
        let environ = fs::read(format!("/proc/{pid}/environ")).ok()?;
        environ
            .split(|&byte| byte == 0)
            .find_map(|pair| pair.strip_prefix(b"DISPLAY="))
            .map(|value| String::from_utf8_lossy(value).into_owned())
    });
    for display in own.into_iter().chain(theirs) {
        if !display.is_empty() && !found.contains(&display) {
            found.push(display);
        }
    }
    found
}

fn on_display(display: &str, pids: &HashSet<u32>) -> anyhow::Result<usize> {
    let (conn, screen) = x11rb::connect(Some(display))?;
    let root = conn.setup().roots[screen].root;
    let atom =
        |name: &[u8]| -> anyhow::Result<Atom> { Ok(conn.intern_atom(false, name)?.reply()?.atom) };
    let protocols = atom(b"WM_PROTOCOLS")?;
    let delete = atom(b"WM_DELETE_WINDOW")?;
    let net_pid = atom(b"_NET_WM_PID")?;

    // Top-level windows, and one level down for a manager that frames them.
    let mut windows: Vec<Window> = Vec::new();
    for top in conn.query_tree(root)?.reply()?.children {
        windows.push(top);
        if let Ok(tree) = conn.query_tree(top)?.reply() {
            windows.extend(tree.children);
        }
    }
    let mut asked = 0;
    for window in windows {
        let Ok(attributes) = conn.get_window_attributes(window)?.reply() else {
            continue;
        };
        if attributes.map_state != MapState::VIEWABLE {
            continue;
        }
        let pid = conn
            .get_property(false, window, net_pid, AtomEnum::CARDINAL, 0, 1)?
            .reply()
            .ok()
            .and_then(|reply| reply.value32().and_then(|mut values| values.next()));
        if !pid.is_some_and(|pid| pids.contains(&pid)) {
            continue;
        }
        let supports_delete = conn
            .get_property(false, window, protocols, AtomEnum::ATOM, 0, 32)?
            .reply()
            .ok()
            .and_then(|reply| reply.value32().map(|mut atoms| atoms.any(|a| a == delete)))
            .unwrap_or(false);
        if !supports_delete {
            continue;
        }
        let event = ClientMessageEvent::new(
            32,
            window,
            protocols,
            [delete, x11rb::CURRENT_TIME, 0, 0, 0],
        );
        conn.send_event(false, window, EventMask::NO_EVENT, event)?;
        asked += 1;
    }
    conn.flush()?;
    Ok(asked)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use x11rb::COPY_DEPTH_FROM_PARENT;
    use x11rb::protocol::Event;
    use x11rb::protocol::xproto::{CreateWindowAux, PropMode, WindowClass};
    use x11rb::wrapper::ConnectionExt as _;

    use super::*;

    /// Opens windows on an X server of the test's own and closes one of
    /// them. Never on the desktop: only with `ZENDO_X_TEST` set, as in
    /// `gamescope --backend headless -- env ZENDO_X_TEST=1 cargo test -p
    /// zendo-core windows -- --ignored`.
    #[test]
    #[ignore = "needs an X server of its own, through gamescope's headless backend"]
    fn asks_only_the_games_windows_to_close() {
        if std::env::var_os("ZENDO_X_TEST").is_none() {
            return;
        }
        let (conn, screen) = x11rb::connect(None).unwrap();
        let root = conn.setup().roots[screen].root;
        let atom = |name: &[u8]| conn.intern_atom(false, name).unwrap().reply().unwrap().atom;
        let (protocols, delete, net_pid) = (
            atom(b"WM_PROTOCOLS"),
            atom(b"WM_DELETE_WINDOW"),
            atom(b"_NET_WM_PID"),
        );
        let own = std::process::id();
        let open = |pid: u32| {
            let window = conn.generate_id().unwrap();
            conn.create_window(
                COPY_DEPTH_FROM_PARENT,
                window,
                root,
                0,
                0,
                64,
                64,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new(),
            )
            .unwrap();
            conn.change_property32(
                PropMode::REPLACE,
                window,
                net_pid,
                AtomEnum::CARDINAL,
                &[pid],
            )
            .unwrap();
            conn.change_property32(
                PropMode::REPLACE,
                window,
                protocols,
                AtomEnum::ATOM,
                &[delete],
            )
            .unwrap();
            conn.map_window(window).unwrap();
            window
        };
        let game = open(own);
        let other = open(own + 1);
        conn.flush().unwrap();
        // Mapped once the server's manager lets it be.
        let deadline = Instant::now() + Duration::from_secs(5);
        while conn
            .get_window_attributes(game)
            .unwrap()
            .reply()
            .unwrap()
            .map_state
            != MapState::VIEWABLE
        {
            assert!(Instant::now() < deadline, "the window was never mapped");
            std::thread::sleep(Duration::from_millis(20));
        }

        let asked = ask_to_close(&HashSet::from([own]));
        assert_eq!(asked, 1, "only the window of the game's process");

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(Instant::now() < deadline, "no close request came");
            match conn.poll_for_event().unwrap() {
                Some(Event::ClientMessage(event)) => {
                    assert_eq!(event.window, game);
                    assert_ne!(event.window, other);
                    assert_eq!(event.data.as_data32()[0], delete);
                    break;
                }
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    }
}
