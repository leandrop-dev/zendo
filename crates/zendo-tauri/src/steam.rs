//! Steam's installs in the window: what Steam is downloading, what it
//! installed, updated or uninstalled since the last look, and the installs
//! asked of it from a Steam game's page in the Store.
//!
//! The state and its rules are [`zendo_shell::steam_installs`]'s (shared with
//! the Slint window); this part reads Steam's manifests on a thread of its
//! own, faster while Steam is busy, and writes the pages.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use zendo_core::steam::{AppState, VerifyEvent};
use zendo_shell::steam_installs::{self, Following, SteamInstalls};
use zendo_shell::t;

use crate::downloads;

/// Steam's state, and whether a look at its manifests runs now.
#[derive(Default)]
pub struct SteamState {
    installs: Mutex<SteamInstalls>,
}

/// The installs followed, poisoned or not: a look never leaves them broken.
fn lock(state: &SteamState) -> MutexGuard<'_, SteamInstalls> {
    state
        .installs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Reads Steam's manifests every so often, and more often while something is
/// going on, as the Slint window does.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            let every = {
                let state = app.state::<SteamState>();
                lock(&state).every()
            };
            std::thread::sleep(every);
            look(&app);
        }
    });
}

/// A look is due: Steam's manifests are read on a thread of their own.
fn look(app: &AppHandle) {
    let from = {
        let state = app.state::<SteamState>();
        lock(&state).begin_look()
    };
    let Some(from) = from else {
        return;
    };
    let handle = app.clone();
    std::thread::spawn(move || {
        let (states, verifies) = steam_installs::look(from);
        looked(&handle, states, &verifies);
    });
}

/// Steam's manifests read: what changed is said, and the pages ask again.
fn looked(app: &AppHandle, states: Vec<AppState>, verifies: &[VerifyEvent]) {
    let looked = {
        let state = app.state::<SteamState>();
        lock(&state).looked(states, verifies)
    };
    if let Some(line) = looked.say {
        downloads::say(app, line);
    }
    if looked.rescan {
        crate::library::rescan(app.clone());
    }
    changed(app);
}

/// What Steam is doing changed: the Downloads page, a store game's page and
/// an installed game's page ask again.
pub(crate) fn changed(app: &AppHandle) {
    let _ = app.emit("steam_changed", ());
    let _ = app.emit("downloads_changed", ());
    let _ = app.emit("store_changed", ());
}

/// How many installs Steam shows on the Downloads page, for the tab's count.
#[must_use]
pub fn count(app: &AppHandle) -> usize {
    let state = app.state::<SteamState>();
    lock(&state).count()
}

/// What a Steam game's page says of Steam's install: the page's job numbers
/// 5 (Install with Steam), 6 (asked; Steam's dialog was open) and 7 (Steam is
/// on it), the words on the button, and how far (-1 for no bar).
pub(crate) fn install_state(app: &AppHandle, id: &str) -> Option<(i32, String, f32)> {
    let app_id = id.strip_prefix("steam:")?;
    let state = app.state::<SteamState>();
    let installs = lock(&state);
    Some(match installs.following(app_id) {
        None => (5, t!("store-install-with-steam"), -1.0),
        // Steam writes nothing when its dialog is cancelled: the button asks
        // again, and Stop waiting ends the wait.
        Some(Following::Asked) => (6, t!("steam-ask-again"), -1.0),
        Some(Following::Busy(busy)) => (
            7,
            if matches!(busy.progress, zendo_core::steam::Progress::Paused { .. }) {
                t!("steam-paused")
            } else {
                t!("installing")
            },
            steam_installs::percent(busy.progress),
        ),
    })
}

/// What Steam's own pages say of the install, under the buttons.
pub(crate) fn install_note(app: &AppHandle, id: &str) -> Option<String> {
    let app_id = id.strip_prefix("steam:")?;
    let state = app.state::<SteamState>();
    let installs = lock(&state);
    match installs.following(app_id)? {
        Following::Asked => Some(t!("steam-waiting-dialog")),
        Following::Busy(busy) => Some(steam_installs::progress_line(busy.progress)),
    }
}

/// Install with Steam pressed: Steam's own dialog opens, and the install is
/// followed. Asked again while waiting, the dialog opens again.
pub(crate) fn install(app: &AppHandle, id: &str) {
    let Some(app_id) = id.strip_prefix("steam:") else {
        return;
    };
    let title = {
        let state = app.state::<crate::store::StoreState>();
        let browser = state.browser.lock().unwrap_or_else(PoisonError::into_inner);
        browser
            .owned(id)
            .map_or_else(|| id.to_owned(), |owned| owned.title.clone())
    };
    let line = {
        let state = app.state::<SteamState>();
        lock(&state).install(app_id, &title, true)
    };
    if let Some(line) = line {
        downloads::say(app, line);
    }
    changed(app);
}

/// Stop waiting for Steam to start an app's install.
pub(crate) fn forget(app: &AppHandle, app_id: &str) {
    {
        let state = app.state::<SteamState>();
        lock(&state).forget(app_id);
    }
    changed(app);
}

/// One line of the Downloads page's Steam card.
#[derive(Debug, Serialize)]
pub struct SteamRowDto {
    pub(crate) app: String,
    pub(crate) title: String,
    pub(crate) line: String,
    /// How far along, 0 to 100; -1 for no bar.
    pub(crate) percent: f32,
    /// Stop waiting, for an install asked of Steam that has not started.
    pub(crate) forget: String,
}

/// The card's rows, and whether there is anything to show.
#[must_use]
pub fn card(app: &AppHandle) -> (bool, Vec<SteamRowDto>) {
    let state = app.state::<SteamState>();
    let installs = lock(&state);
    let show = installs.active();
    let rows = installs
        .rows()
        .into_iter()
        .map(|row| SteamRowDto {
            app: row.app,
            title: row.title,
            line: row.line,
            percent: row.percent.unwrap_or(-1.0),
            forget: if row.forget {
                t!("steam-stop-waiting")
            } else {
                String::new()
            },
        })
        .collect();
    (show, rows)
}

#[tauri::command]
pub fn steam_stop_waiting(app: String, handle: AppHandle) -> downloads::DownloadsDto {
    forget(&handle, &app);
    downloads::dto(&handle)
}

#[tauri::command]
pub fn steam_open_downloads(app: AppHandle) {
    if let Err(err) = zendo_shell::open::target("steam://open/downloads") {
        downloads::say(&app, err);
        let _ = app.emit("downloads_changed", ());
    }
}
