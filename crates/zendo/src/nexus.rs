//! Nexus links and the mods they bring: `nxm://` handed over by the browser,
//! the sheet that asks which game a link is for, the downloads shown on the
//! game's Mods card, and the newer files Nexus has for a game's mods.
//!
//! The state machine is `zendo_shell::nexus`'s, kept in `zendo-shell`;
//! its key and handler live in `SettingsState`. The key may be kept in the
//! keyring, so it is read on the threads here, never on the window's.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::panic::AssertUnwindSafe;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use zendo_core::config::Config;
use zendo_core::mods::Added;
use zendo_core::nexus::Link;
use zendo_shell::nexus::{
    self as shell, Arrived, Finished, Matching, NexusState, Update, UpdateList,
};
use zendo_shell::t;

use crate::game::emit_game_changed;
use crate::library::LibraryState;
use crate::mods::ModsHandle;
use crate::settings::SettingsState;

/// How often a download's line is drawn while it runs.
const PROGRESS_EVERY: Duration = Duration::from_millis(300);
/// How long a link waits for the first scan.
const READY_TRIES: usize = 600;
const READY_WAIT: Duration = Duration::from_millis(250);

/// The shared Nexus state, for the time of `with`; the settings state must
/// outlive the guard, which a helper cannot return.
fn with_nexus<T>(app: &AppHandle, with: impl FnOnce(&mut NexusState) -> T) -> T {
    let settings = app.state::<SettingsState>();
    let mut state = settings.nexus();
    with(&mut state)
}

/// The Nexus key: the settings file's, or the keyring's. It may prompt, so
/// only from a thread of the work's own.
fn key() -> Option<String> {
    Config::load().ok()?.nexus_key
}

/// The last line Nexus said, kept until the page takes it: a link can come
/// in before the window has listened.
#[derive(Debug, Default)]
pub struct NexusFeed {
    line: Mutex<Option<String>>,
}

/// A line for the window's foot.
pub(crate) fn say(app: &AppHandle, line: String) {
    {
        let feed = app.state::<NexusFeed>();
        *feed.line.lock().unwrap_or_else(PoisonError::into_inner) = Some(line.clone());
    }
    let _ = app.emit("nexus_said", line);
}

/// The last line said, taken once: the page asks as it starts and whenever
/// Nexus changes.
#[tauri::command]
pub fn nexus_notice(app: AppHandle) -> Option<String> {
    let feed = app.state::<NexusFeed>();
    feed.line
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
}

fn changed(app: &AppHandle) {
    let _ = app.emit("nexus_changed", ());
}

/// A mod coming in from Nexus.
#[derive(Debug, Serialize)]
pub struct DownloadDto {
    id: u64,
    name: String,
    /// How far it got, in words.
    words: String,
    /// 0 to 1, or 0 when its size is not known.
    fraction: f32,
    stopping: bool,
}

/// The sheet that asks which game a link is for, and one game's downloads.
#[derive(Debug, Default, Serialize)]
pub struct NexusDto {
    choosing: bool,
    choose_about: String,
    choices: Vec<String>,
    downloads: Vec<DownloadDto>,
}

fn download_dto(download: &shell::Download) -> DownloadDto {
    let (fraction, words) = download.progress();
    DownloadDto {
        id: download.id,
        name: download.name.clone(),
        words,
        fraction,
        stopping: download.stopping(),
    }
}

fn dto(app: &AppHandle, game: Option<&str>) -> NexusDto {
    with_nexus(app, |state| {
        let choosing = state.choosing.as_ref();
        NexusDto {
            choosing: choosing.is_some(),
            choose_about: choosing.map_or_default(|choosing| {
                if choosing.all {
                    t!("nexus-choose-none", name = choosing.name.as_str())
                } else {
                    t!("nexus-choose-several", name = choosing.name.as_str())
                }
            }),
            choices: choosing.map_or_default(|choosing| {
                choosing
                    .games
                    .iter()
                    .map(|(_, title)| title.clone())
                    .collect()
            }),
            downloads: state
                .downloads
                .iter()
                .filter(|download| game.is_none_or(|game| download.game == game))
                .map(download_dto)
                .collect(),
        }
    })
}

#[tauri::command]
pub fn nexus_get(game: Option<String>, app: AppHandle) -> NexusDto {
    dto(&app, game.as_deref())
}

/// A game was chosen for the link: its file comes down.
#[tauri::command]
pub fn nexus_choose(at: usize, app: AppHandle) -> NexusDto {
    let id = with_nexus(&app, |state| {
        state
            .choosing
            .as_ref()
            .and_then(|choosing| choosing.games.get(at))
            .map(|(id, _)| id.clone())
    });
    let Some(id) = id else {
        return dto(&app, None);
    };
    let chosen = with_nexus(&app, |state| state.choose(&id));
    let answer = dto(&app, None);
    if let Some((id, link, remember)) = chosen {
        download(&app, &id, link, remember);
    }
    answer
}

#[tauri::command]
pub fn nexus_cancel(app: AppHandle) -> NexusDto {
    with_nexus(&app, NexusState::cancel_choose);
    dto(&app, None)
}

/// Stop on a download's line.
#[tauri::command]
pub fn nexus_stop(id: u64, game: Option<String>, app: AppHandle) -> NexusDto {
    with_nexus(&app, |state| state.stop(id));
    dto(&app, game.as_deref())
}

/// Update on a mod of the game: Premium downloads the newer file straight
/// away; otherwise its page opens for the site's button to send it back.
#[tauri::command]
pub fn nexus_update(id: String, at: usize, app: AppHandle) -> NexusDto {
    let handle = app.clone();
    let answer = dto(&app, Some(&id));
    std::thread::spawn(move || {
        let overview = {
            let mods = handle.state::<ModsHandle>();
            let state = mods.lock();
            state.overview(&id).cloned()
        };
        let Some(overview) = overview else {
            return;
        };
        let Some(mod_id) = overview.mods.get(at).map(|info| info.id.clone()) else {
            return;
        };
        let update = with_nexus(&handle, |state| state.update(&id, &mod_id, &overview));
        match update {
            Some(Update::Download(link)) => download(&handle, &id, link, false),
            Some(Update::Page(url, note)) => {
                say(&handle, note);
                let _ = zendo_shell::open::target(&url);
            }
            None => {}
        }
    });
    answer
}

/// Which game a link is for; the chooser when several match.
#[tauri::command]
pub fn nexus_link(text: String, app: AppHandle) -> NexusDto {
    link(&app, text);
    dto(&app, None)
}

/// A link from the browser, on a thread that waits for the library's first
/// scan and reads the key where it is kept.
pub fn link(app: &AppHandle, text: String) {
    let handle = app.clone();
    std::thread::spawn(move || {
        for _ in 0..READY_TRIES {
            if handle.state::<LibraryState>().is_ready() {
                break;
            }
            std::thread::sleep(READY_WAIT);
        }
        let key = key();
        let ready = handle.state::<LibraryState>().is_ready();
        let arrived = with_nexus(&handle, |state| {
            state.link_arrived(&text, ready, key.is_some())
        });
        match arrived {
            Arrived::Say(note) => say(&handle, note),
            Arrived::Wait => {}
            Arrived::Ask(link, note) => {
                let Some(key) = key else {
                    return;
                };
                let games = handle.state::<LibraryState>().games();
                let games = shell::with_folders(&games);
                say(&handle, note);
                let found = shell::match_games(&key, &link, &games);
                match with_nexus(&handle, |state| state.matched(found)) {
                    Matching::Download(id, link) => download(&handle, &id, link, false),
                    Matching::Say(note) => say(&handle, note),
                    Matching::Choose => changed(&handle),
                }
            }
        }
    });
}

/// Downloads the link's file into a game's mods; its thread says how far it
/// got every so often and the end reloads the card.
pub(crate) fn download(app: &AppHandle, id: &str, link: Link, remember: bool) {
    let Some(game) = (app.state::<LibraryState>()).game(id) else {
        return;
    };
    let Some(key) = key() else {
        say(app, t!("nexus-needs-key"));
        return;
    };
    let started = with_nexus(app, |state| state.begin(&game, &link));
    let Some(started) = started else {
        return;
    };
    say(app, started.note);
    changed(app);
    let handle = app.clone();
    let (number, stop) = (started.id, started.stop);
    std::thread::spawn(move || {
        let mut last: Option<Instant> = None;
        let mut progress = |done: u64, total: Option<u64>| {
            if last.is_some_and(|last| last.elapsed() < PROGRESS_EVERY) {
                return;
            }
            last = Some(Instant::now());
            with_nexus(&handle, |state| state.progress(number, done, total));
            changed(&handle);
        };
        let game_id = game.id.clone();
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            shell::fetch(&game, &key, &link, remember, &stop, &mut progress)
        }))
        .unwrap_or_else(|_| Err("the download panicked".to_owned()));
        finished(&handle, number, &game_id, result);
    });
}

/// A download ended: the mods as they stand, what to say, and the updates
/// that follow.
fn finished(app: &AppHandle, id: u64, game: &str, result: Result<(Added, String), String>) {
    let finished = with_nexus(app, |state| state.finished(id, game, result));
    match finished {
        Finished::Choose(pending) => crate::installer::opened(app, game.to_owned(), pending),
        Finished::Done(overview, note) => {
            {
                let mods = app.state::<ModsHandle>();
                mods.lock().ready(game.to_owned(), *overview);
            }
            say(app, note);
            want_updates(app, game);
        }
        Finished::Say(note) => say(app, note),
    }
    emit_game_changed(app, game);
    changed(app);
}

/// Asks Nexus, once a session, for the newer files of a game's mods.
pub fn want_updates(app: &AppHandle, game: &str) {
    let Some(key) = key() else {
        return;
    };
    let overview = {
        let mods = app.state::<ModsHandle>();
        let state = mods.lock();
        state.overview(game).cloned()
    };
    let Some(overview) = overview else {
        return;
    };
    let runnable = (app.state::<LibraryState>()).game(game).is_some();
    let job = with_nexus(app, |state| {
        state.want_updates(game, &overview, true, runnable)
    });
    let Some(job) = job else {
        return;
    };
    let handle = app.clone();
    std::thread::spawn(move || {
        let found: UpdateList = shell::ask_updates(&key, job.sources);
        let note = with_nexus(&handle, |state| state.updates_found(&job.game, Ok(found)));
        if let Some(note) = note {
            say(&handle, note);
        }
        emit_game_changed(&handle, &job.game);
    });
}

/// Whether the key's account has Premium.
pub(crate) fn premium(app: &AppHandle) -> Option<bool> {
    with_nexus(app, |state| state.premium())
}

/// The card's rows, with Update written on the mods Nexus has a newer file
/// for.
pub(crate) fn write_updates(app: &AppHandle, game: &str, card: &mut crate::mods::ModsDto) {
    let overview = {
        let mods = app.state::<ModsHandle>();
        let state = mods.lock();
        state.overview(game).cloned()
    };
    let Some(overview) = overview else {
        return;
    };
    with_nexus(app, |state| {
        for (at, info) in overview.mods.iter().enumerate() {
            if let Some(file) = state.update_for(game, &info.id)
                && let Some(row) = card.mods.get_mut(at)
            {
                row.update = shell::update_label(file);
            }
        }
    });
}
