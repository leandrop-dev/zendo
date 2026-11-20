//! The installed game page's jobs: Update in the action bar, the line saying
//! what the queue is doing with the game, the More row (Check for updates,
//! Repair, Move… and Uninstall, pressed twice) and the DLC card's Install,
//! Install all and Uninstall.
//!
//! The jobs themselves are `zendo_shell::queue`'s; a Steam game's Verify and
//! Uninstall are asked of Steam (`crate::steam`). The words are that
//! module's and `zendo_shell::dlc`'s, shared with the Slint window.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use zendo_core::downloads::{Action, Job};
use zendo_core::{Game, Source};
use zendo_shell::file_dialog::{self, Kind};
use zendo_shell::queue::{self, UpdateState};
use zendo_shell::store_facts::InfoState;
use zendo_shell::t;

use crate::downloads::{self, DownloadsState};
use crate::game::GameDto;
use crate::library::LibraryState;
use crate::steam::{self, SteamState};
use crate::store::StoreState;

/// What the page keeps while it is open on a game; folded on each game.
#[derive(Default)]
pub struct JobsState {
    /// The More row is open.
    more: AtomicBool,
    /// The DLC whose Uninstall was pressed once.
    confirm_dlc: Mutex<Option<String>>,
}

fn confirm_dlc(state: &JobsState) -> MutexGuard<'_, Option<String>> {
    state
        .confirm_dlc
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// The page opens on another game, or closes: everything starts folded and
/// unasked.
pub fn reset(app: &AppHandle) {
    {
        let state = app.state::<JobsState>();
        state.more.store(false, Ordering::Relaxed);
        *confirm_dlc(&state) = None;
    }
    let downloads = app.state::<DownloadsState>();
    downloads::lock(&downloads).confirm_uninstall = None;
    crate::options::reset(app);
}

/// The page opened on a game: everything starts folded and unasked.
#[tauri::command]
pub fn game_open(id: String, app: AppHandle) -> Result<GameDto, String> {
    reset(&app);
    crate::game::page(&app, &id)
}

// What the page shows.

/// Update in the action bar, and whether it can be pressed.
#[derive(Debug, Default, Serialize)]
pub(crate) struct UpdateDto {
    /// Empty when no update was found.
    label: String,
    badge: String,
    on: bool,
}

/// The More row: what the game can be done to, and what it is doing.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is one fact the row is drawn by"
)]
#[derive(Debug, Default, Serialize)]
pub(crate) struct ManageDto {
    has_more: bool,
    more_open: bool,
    managed: bool,
    store_managed: bool,
    job_free: bool,
    check_on: bool,
    move_on: bool,
    uninstall_again: bool,
    /// Repair and Uninstall, as the action's own words.
    repair_label: String,
    queue_uninstall_label: String,
    verify_label: String,
    verify_on: bool,
    uninstall_label: String,
}

/// The line under the action bar: the job queued for the game, if any.
#[derive(Debug, Default, Serialize)]
pub(crate) struct JobLineDto {
    shown: bool,
    line: String,
}

/// One DLC: its words and its button.
#[derive(Debug, Serialize)]
pub(crate) struct DlcRowDto {
    id: String,
    title: String,
    about: String,
    label: String,
    danger: bool,
    enabled: bool,
    status: String,
    status_tone: i32,
}

/// The DLC card: the count, a row each, Install all and a note.
#[derive(Debug, Default, Serialize)]
pub(crate) struct DlcCardDto {
    count: String,
    rows: Vec<DlcRowDto>,
    all: String,
    all_on: bool,
    note: String,
}

/// The update the stores found for the game, if any.
pub(crate) fn update(app: &AppHandle, game: &Game) -> UpdateDto {
    let downloads = app.state::<DownloadsState>();
    let found = {
        let queue = downloads::lock(&downloads);
        matches!(queue.updates.get(&game.id), Some(UpdateState::Available(_)))
            && game.store_managed()
    };
    if !found {
        return UpdateDto {
            on: queue_free(app, &game.id),
            ..UpdateDto::default()
        };
    }
    UpdateDto {
        label: t!("action-label", action = "update"),
        badge: t!("badge-update"),
        on: queue_free(app, &game.id),
    }
}

/// Whether the game has no job in the queue.
pub(crate) fn queue_free(app: &AppHandle, id: &str) -> bool {
    let downloads = app.state::<DownloadsState>();
    downloads::lock(&downloads).job_for(id).is_none()
}

/// The More row and the Steam buttons.
pub(crate) fn manage(app: &AppHandle, game: &Game) -> ManageDto {
    let managed = game.installed_by_zendo();
    let store_managed = game.store_managed();
    let free = queue_free(app, &game.id);
    let steam_app = steam_app(game);
    let (verify, verify_on, uninstall) = match steam_app {
        Some(app_id) => {
            let state = app.state::<SteamState>();
            let installs = steam::lock(&state);
            (
                if installs.verifying(app_id) {
                    t!("steam-verifying")
                } else {
                    t!("steam-verify")
                },
                !installs.verifying(app_id),
                if installs.removing(app_id) {
                    t!("steam-ask-again-uninstall")
                } else {
                    t!("steam-uninstall")
                },
            )
        }
        None => (String::new(), false, String::new()),
    };
    let checking = {
        let downloads = app.state::<DownloadsState>();
        let queue = downloads::lock(&downloads);
        queue.updates.get(&game.id) == Some(&UpdateState::Checking)
    };
    let running = app.state::<LibraryState>().running().is_open(&game.id);
    let uninstall_again = {
        let downloads = app.state::<DownloadsState>();
        downloads::lock(&downloads).confirm_uninstall.as_deref() == Some(game.id.as_str())
    };
    let jobs = app.state::<JobsState>();
    ManageDto {
        has_more: managed || steam_app.is_some(),
        more_open: jobs.more.load(Ordering::Relaxed),
        managed,
        store_managed,
        job_free: free,
        check_on: store_managed && !checking,
        move_on: store_managed && free && !running,
        uninstall_again,
        repair_label: t!("action-label", action = "repair"),
        queue_uninstall_label: t!("action-label", action = "uninstall"),
        verify_label: verify,
        verify_on,
        uninstall_label: uninstall,
    }
}

/// The line saying what the queue is doing with the game.
pub(crate) fn job_line(app: &AppHandle, game: &Game) -> JobLineDto {
    let downloads = app.state::<DownloadsState>();
    let queue = downloads::lock(&downloads);
    let Some((job, running)) = queue.job_for(&game.id) else {
        return JobLineDto::default();
    };
    let state = match (running, job.paused) {
        (true, _) => "now",
        (false, true) => "paused",
        (false, false) => "queued",
    };
    JobLineDto {
        shown: true,
        line: t!(
            "downloads-job-state",
            doing = queue::action_doing_alone(job.action),
            state = state
        ),
    }
}

/// The DLC card, through `zendo_shell::dlc`.
pub(crate) fn dlc_card(app: &AppHandle, game: &Game) -> DlcCardDto {
    if !matches!(game.source, Source::Epic | Source::Gog) {
        return DlcCardDto::default();
    }
    let store = match game.source {
        Source::Gog => zendo_core::stores::Store::Gog,
        _ => zendo_core::stores::Store::Epic,
    };
    let info = {
        let store_state = app.state::<StoreState>();
        let browser = store_state
            .browser
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        browser.info.get(&game.id).cloned()
    };
    let signed_in = crate::accounts::signed_in(app, store);
    // The game's page is the library's: its id is there.
    let listed = app.state::<LibraryState>().game(&game.id).is_some();
    let downloads = app.state::<DownloadsState>();
    let free = downloads::lock(&downloads).job_for(&game.id).is_none();
    let manage = game.installed_by_zendo();
    let confirm = {
        let jobs = app.state::<JobsState>();
        confirm_dlc(&jobs).clone()
    };
    let view = zendo_shell::dlc::view(&zendo_shell::dlc::Ask {
        game,
        info: info.as_ref(),
        signed_in,
        manage,
        unmanaged: false,
        confirm: confirm.as_deref(),
        free: manage && free,
        listed,
    });
    let rows = view
        .rows
        .into_iter()
        .map(|row| DlcRowDto {
            id: row.id,
            title: row.title,
            about: row.about,
            label: row.label,
            danger: row.danger,
            enabled: row.enabled,
            status: row.status,
            status_tone: row.status_tone,
        })
        .collect();
    DlcCardDto {
        count: view.count,
        rows,
        all: view.all,
        all_on: view.all_on,
        note: view.note,
    }
}

/// A Steam game's app id.
#[must_use]
pub(crate) fn steam_app(game: &Game) -> Option<&str> {
    zendo_shell::game::steam_app(game)
}

// The presses.

/// Queues a job for the game, unless it has one already.
fn queue_for(app: &AppHandle, game: &Game, job: Option<Job>) {
    let Some(job) = job else {
        return;
    };
    {
        let state = app.state::<JobsState>();
        *confirm_dlc(&state) = None;
    }
    let effects = {
        let downloads = app.state::<DownloadsState>();
        let mut queue = downloads::lock(&downloads);
        if queue.job_for(&game.id).is_some() {
            return;
        }
        queue.enqueue(job)
    };
    downloads::apply(app, effects);
}

/// The game a command is about.
fn game_of(app: &AppHandle, id: &str) -> Result<Game, String> {
    app.state::<LibraryState>()
        .game(id)
        .ok_or_else(|| format!("no game with the id {id:?}"))
}

/// Update or Repair.
fn game_job(app: &AppHandle, id: &str, action: Action) -> Result<GameDto, String> {
    let game = game_of(app, id)?;
    let job = Job::for_game(&game, action);
    queue_for(app, &game, job);
    crate::game::page(app, id)
}

#[tauri::command]
pub fn game_update(id: String, app: AppHandle) -> Result<GameDto, String> {
    game_job(&app, &id, Action::Update)
}

#[tauri::command]
pub fn game_repair(id: String, app: AppHandle) -> Result<GameDto, String> {
    game_job(&app, &id, Action::Repair)
}

/// Uninstall: asks once more first, then queues it.
#[tauri::command]
pub fn game_uninstall(id: String, app: AppHandle) -> Result<GameDto, String> {
    let game = game_of(&app, &id)?;
    let confirming = {
        let downloads = app.state::<DownloadsState>();
        let queue = downloads::lock(&downloads);
        if queue.job_for(&id).is_some() {
            return crate::game::page(&app, &id);
        }
        queue.confirm_uninstall.as_deref() == Some(id.as_str())
    };
    if confirming {
        let job = Job::for_game(&game, Action::Uninstall);
        queue_for(&app, &game, job);
    } else {
        let downloads = app.state::<DownloadsState>();
        downloads::lock(&downloads).confirm_uninstall = Some(id.clone());
    }
    crate::game::page(&app, &id)
}

/// Check for updates: the game is asked of its store again.
#[tauri::command]
pub fn game_check_updates(id: String, app: AppHandle) -> Result<GameDto, String> {
    downloads::check_these(&app, std::slice::from_ref(&id));
    crate::game::page(&app, &id)
}

/// Move…: the folder dialog opens, unless the game is running.
#[tauri::command]
pub fn game_move(id: String, app: AppHandle) -> Result<GameDto, String> {
    let game = game_of(&app, &id)?;
    if app.state::<LibraryState>().running().is_open(&id) {
        downloads::say(
            &app,
            t!("downloads-move-running", title = game.title.as_str()),
        );
        let _ = app.emit("downloads_changed", ());
        return crate::game::page(&app, &id);
    }
    if !queue_free(&app, &id) {
        return crate::game::page(&app, &id);
    }
    let handle = app.clone();
    let title = t!("downloads-move-to", title = game.title.as_str());
    let wanted = id.clone();
    std::thread::spawn(move || {
        let result = file_dialog::pick(Kind::Folder, &title).map_err(|err| format!("{err:#}"));
        match result {
            Ok(Some(folder)) => {
                let game = game_of(&handle, &wanted);
                if let Ok(game) = game {
                    let job = Job::for_move(&game, folder);
                    queue_for(&handle, &game, job);
                }
            }
            Ok(None) => {}
            Err(err) => {
                downloads::say(&handle, err);
                let _ = handle.emit("downloads_changed", ());
            }
        }
    });
    crate::game::page(&app, &id)
}

/// The More row, opened or folded.
#[tauri::command]
pub fn game_toggle_more(id: String, app: AppHandle) -> Result<GameDto, String> {
    {
        let state = app.state::<JobsState>();
        let open = state.more.load(Ordering::Relaxed);
        state.more.store(!open, Ordering::Relaxed);
    }
    crate::game::page(&app, &id)
}

/// A DLC's button: Install, or Uninstall, which asks once more first.
#[tauri::command]
pub fn game_dlc(id: String, dlc: String, app: AppHandle) -> Result<GameDto, String> {
    let game = game_of(&app, &id)?;
    let installed = game.details.dlc.iter().any(|item| item.id == dlc);
    if !installed {
        let job = Job::for_dlc(&game, Action::AddDlc, vec![dlc]);
        queue_for(&app, &game, job);
        return crate::game::page(&app, &id);
    }
    let confirming = {
        let state = app.state::<JobsState>();
        confirm_dlc(&state).as_deref() == Some(dlc.as_str())
    };
    if confirming {
        let job = Job::for_dlc(&game, Action::RemoveDlc, vec![dlc]);
        queue_for(&app, &game, job);
    } else {
        let state = app.state::<JobsState>();
        *confirm_dlc(&state) = Some(dlc);
    }
    crate::game::page(&app, &id)
}

/// Install all: every DLC the store says can be installed and is not.
#[tauri::command]
pub fn game_dlc_all(id: String, app: AppHandle) -> Result<GameDto, String> {
    let game = game_of(&app, &id)?;
    let have: HashSet<&str> = game.details.dlc.iter().map(|dlc| dlc.id.as_str()).collect();
    let info = {
        let store = app.state::<StoreState>();
        let browser = store.browser.lock().unwrap_or_else(PoisonError::into_inner);
        browser.info.get(&id).cloned()
    };
    let wanted: Vec<String> = match info {
        Some(InfoState::Got(info)) => info
            .dlc
            .into_iter()
            .filter(|dlc| dlc.installable)
            .map(|dlc| dlc.id)
            .filter(|dlc| !have.contains(dlc.as_str()))
            .collect(),
        _ => Vec::new(),
    };
    let job = Job::for_dlc(&game, Action::AddDlc, wanted);
    queue_for(&app, &game, job);
    crate::game::page(&app, &id)
}

/// Verify with Steam: Steam shows its own progress.
#[tauri::command]
pub fn steam_verify_game(id: String, app: AppHandle) -> Result<GameDto, String> {
    let game = game_of(&app, &id)?;
    steam::verify(&app, &game);
    crate::game::page(&app, &id)
}

/// Uninstall with Steam: Steam asks its own question.
#[tauri::command]
pub fn steam_uninstall_game(id: String, app: AppHandle) -> Result<GameDto, String> {
    let game = game_of(&app, &id)?;
    steam::uninstall(&app, &game);
    crate::game::page(&app, &id)
}
