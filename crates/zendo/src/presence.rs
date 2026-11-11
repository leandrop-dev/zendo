//! Discord rich presence: the user's status shows the game Zendo is
//! playing, from when it starts until it ends. Off by default; Settings'
//! Discord section turns it on. It runs on a thread of its own, so Discord
//! not running (or its IPC socket not being there) never blocks the window.
//! The status words stay English, as the command line and the report do.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use discord_rich_presence::activity::{Activity, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

/// Zendo's application on Discord's developer portal; the presence shows
/// under this name.
const CLIENT_ID: &str = "1570036457047986256";

/// What the presence should show.
#[derive(Debug, Clone)]
pub enum Presence {
    /// A game is running: its title, and when it started (Unix seconds).
    Playing { title: String, since: u64 },
    /// Nothing runs: no activity.
    Idle,
}

/// The presence's thread, told what to show. Dropped, it clears its
/// activity and ends.
pub struct PresenceHandle {
    sender: mpsc::Sender<Presence>,
}

impl PresenceHandle {
    /// Starts the presence thread. The window keeps this value while the
    /// setting is on.
    #[must_use]
    pub fn spawn() -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || run(receiver));
        Self { sender }
    }

    /// Tells the thread what to show. A send never blocks.
    pub fn show(&self, presence: Presence) {
        let _ = self.sender.send(presence);
    }
}

/// The thread: connect to Discord, then show whatever is asked, reconnecting
/// as Discord comes and goes.
#[expect(
    clippy::needless_pass_by_value,
    reason = "the receiver is moved into the thread, which owns it for its life"
)]
fn run(receiver: mpsc::Receiver<Presence>) {
    let mut client = DiscordIpcClient::new(CLIENT_ID);
    let mut wanted = Presence::Idle;
    loop {
        // Connect to Discord, retrying while it is not running, and keeping
        // the latest ask.
        while client.connect().is_err() {
            loop {
                match receiver.try_recv() {
                    Ok(presence) => wanted = presence,
                    Err(mpsc::TryRecvError::Disconnected) => return,
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
            thread::sleep(Duration::from_secs(5));
        }
        // Connected: show what is wanted, then follow it. A failed write
        // means Discord went away; close and reconnect.
        if !apply(&mut client, &wanted) {
            let _ = client.close();
            continue;
        }
        loop {
            if let Ok(presence) = receiver.recv() {
                wanted = presence;
                if !apply(&mut client, &wanted) {
                    let _ = client.close();
                    break;
                }
            } else {
                // The window is gone: clear the activity and end.
                let _ = client.clear_activity();
                let _ = client.close();
                return;
            }
        }
    }
}

/// Puts `presence` on Discord; `false` when the write failed.
fn apply(client: &mut DiscordIpcClient, presence: &Presence) -> bool {
    match presence {
        Presence::Idle => client.clear_activity().is_ok(),
        Presence::Playing { title, since } => {
            // Discord counts timestamps in milliseconds.
            let since = i64::try_from(*since).unwrap_or(0).saturating_mul(1000);
            let activity = Activity::new()
                .state("Playing")
                .details(title.as_str())
                .timestamps(Timestamps::new().start(since));
            client.set_activity(activity).is_ok()
        }
    }
}
