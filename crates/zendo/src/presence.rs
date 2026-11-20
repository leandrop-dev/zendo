//! Discord rich presence, wired from Settings' Discord toggle and the game's
//! launch: the window keeps the presence thread while the setting is on, and
//! the launch's effects tell it who is playing. The thread itself is
//! `zendo_shell::presence`'s.

use std::sync::{Mutex, PoisonError};

use tauri::{AppHandle, Manager};
use zendo_shell::presence::PresenceHandle;
use zendo_shell::running::Names;

use crate::library::LibraryState;

/// The presence thread, while the setting is on.
pub struct PresenceState {
    handle: Mutex<Option<PresenceHandle>>,
}

impl PresenceState {
    /// The window starts: the presence runs when the setting asks for it.
    pub fn new(on: bool) -> Self {
        Self {
            handle: Mutex::new(on.then(PresenceHandle::spawn)),
        }
    }

    /// The setting changed: start the presence thread, or drop it. Dropping
    /// clears the activity and ends the thread.
    pub fn set(app: &AppHandle, on: bool) {
        *app.state::<PresenceState>()
            .handle
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = on.then(PresenceHandle::spawn);
        Self::show(app);
    }

    /// Tells Discord who is playing, when the setting is on.
    pub fn show(app: &AppHandle) {
        let state = app.state::<PresenceState>();
        let handle = state.handle.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(handle) = handle.as_ref() else {
            return;
        };
        let library = app.state::<LibraryState>();
        let shown = library
            .running()
            .presence(|id| Names::of(id, library.game(id).as_ref()).title);
        handle.show(shown);
    }
}
