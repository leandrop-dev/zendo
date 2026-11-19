//! The Engines group of Settings: the programs Zendo drives (legendary, gogdl,
//! nile, comet and umu-launcher), the anti-cheat runtimes, and Epic's EOS
//! overlay. Each shows the copy found now, whose it is, and offers Download,
//! Update or Remove. The work that waits on GitHub, Lutris or a download runs
//! on a thread of its own.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, State};
use zendo_core::engines::{self, Engine, Located, Origin, Release};
use zendo_core::eos_overlay;
use zendo_core::runtimes::{self, Found, Remote, Runtime};
use zendo_core::stores::Store;
use zendo_shell::t;

use crate::library::LibraryState;
use crate::settings::{BarDto, RowDto, SetRowDto, SettingsDto, bar_of};

/// How often a download's bar reaches the page.
const BAR_EVERY: Duration = Duration::from_millis(100);

/// What is known of the latest release of an engine.
#[derive(Debug, Clone)]
enum Latest {
    Asking,
    Known(Release),
    Failed(String),
}

/// An engine as the card shows it.
struct Row {
    engine: Engine,
    found: Option<Located>,
    installed: Option<String>,
}

struct Download {
    engine: Engine,
    done: u64,
    total: u64,
    stop: Arc<AtomicBool>,
}

/// The programs Zendo drives.
pub struct EnginesState {
    rows: Mutex<Vec<Row>>,
    latest: Mutex<HashMap<Engine, Latest>>,
    asked: AtomicBool,
    download: Mutex<Option<Download>>,
    confirm_remove: Mutex<Option<Engine>>,
    note: Mutex<Option<(String, bool)>>,
}

impl Default for EnginesState {
    fn default() -> Self {
        Self {
            rows: Mutex::default(),
            latest: Mutex::default(),
            asked: AtomicBool::new(false),
            download: Mutex::default(),
            confirm_remove: Mutex::default(),
            note: Mutex::default(),
        }
    }
}

/// The anti-cheat runtimes.
pub struct RuntimesState {
    found: Mutex<Vec<(Runtime, Option<Found>)>>,
    own: Mutex<Vec<(Runtime, Option<String>)>>,
    latest: Mutex<Option<Vec<Remote>>>,
    error: Mutex<Option<String>>,
    asking: AtomicBool,
    downloading: Mutex<Option<Runtime>>,
    confirm_remove: Mutex<Option<Runtime>>,
    note: Mutex<Option<(String, bool)>>,
}

impl Default for RuntimesState {
    fn default() -> Self {
        Self {
            found: Mutex::default(),
            own: Mutex::default(),
            latest: Mutex::default(),
            error: Mutex::default(),
            asking: AtomicBool::new(false),
            downloading: Mutex::default(),
            confirm_remove: Mutex::default(),
            note: Mutex::default(),
        }
    }
}

/// Where Install and Remove of the overlay are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Step {
    #[default]
    Idle,
    ConfirmRemove,
    Working,
}

/// Epic's EOS overlay.
pub struct OverlayState {
    installed: AtomicBool,
    engine: AtomicBool,
    step: Mutex<Step>,
    note: Mutex<Option<(String, bool)>>,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            installed: AtomicBool::new(false),
            engine: AtomicBool::new(false),
            step: Mutex::default(),
            note: Mutex::default(),
        }
    }
}

/// The overlay's card, as the page draws it.
#[derive(Debug, serde::Serialize)]
pub struct OverlayDto {
    status: String,
    install: String,
    install_on: bool,
    remove: String,
    remove_on: bool,
    note: Option<RowDto>,
}

/// Looks at the group, once as the page first opens, and asks GitHub and
/// Lutris once a session.
pub(crate) fn ready(app: &AppHandle) {
    engines_ready(app);
    runtimes_ready(app);
    overlay_ready(app);
}

/// The engines' rows: read again, and GitHub asked once.
fn engines_ready(app: &AppHandle) {
    let state = app.state::<EnginesState>();
    refresh_engines(app);
    if !state.asked.swap(true, Ordering::Relaxed) {
        ask_latest(app);
    }
}

/// The runtimes: read again, and Lutris asked once.
fn runtimes_ready(app: &AppHandle) {
    let state = app.state::<RuntimesState>();
    refresh_runtimes(app);
    if state
        .latest
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some()
        || state.asking.load(Ordering::Relaxed)
    {
        return;
    }
    state.asking.store(true, Ordering::Relaxed);
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = runtimes::latest().map_err(|err| format!("{err:#}"));
        let state = handle.state::<RuntimesState>();
        state.asking.store(false, Ordering::Relaxed);
        match result {
            Ok(latest) => {
                *state.latest.lock().unwrap_or_else(PoisonError::into_inner) = Some(latest);
            }
            Err(err) => *state.error.lock().unwrap_or_else(PoisonError::into_inner) = Some(err),
        }
        let _ = handle.emit("settings_changed", ());
    });
}

/// The overlay: read again.
fn overlay_ready(app: &AppHandle) {
    let state = app.state::<OverlayState>();
    state
        .installed
        .store(eos_overlay::installed(), Ordering::Relaxed);
    state
        .engine
        .store(Store::Epic.engine().is_some(), Ordering::Relaxed);
}

/// Looks again at where each engine is found.
pub(crate) fn refresh_engines(app: &AppHandle) {
    let state = app.state::<EnginesState>();
    let rows = Engine::ALL
        .into_iter()
        .map(|engine| Row {
            engine,
            found: engines::find_with_origin(engine.program()),
            installed: engine.installed(),
        })
        .collect();
    *state.rows.lock().unwrap_or_else(PoisonError::into_inner) = rows;
}

/// Whether replacing or deleting an engine could pull the program out from
/// under a game.
fn engines_busy(app: &AppHandle) -> bool {
    app.state::<LibraryState>().running().busy()
}

/// Asks GitHub for the latest releases, once a session or on asking.
#[tauri::command]
pub fn settings_engines_check(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    ask_latest(&app);
    crate::settings::settings_get(app, library)
}

fn ask_latest(app: &AppHandle) {
    let state = app.state::<EnginesState>();
    for engine in Engine::ALL {
        state
            .latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(engine, Latest::Asking);
        let handle = app.clone();
        std::thread::spawn(move || {
            let result = engines::latest(engine).map_err(|err| format!("{err:#}"));
            let state = handle.state::<EnginesState>();
            state
                .latest
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(
                    engine,
                    match result {
                        Ok(release) => Latest::Known(release),
                        Err(err) => Latest::Failed(err),
                    },
                );
            let _ = handle.emit("settings_changed", ());
        });
    }
}

/// The row's one button: download or update an engine.
#[tauri::command]
pub fn settings_engine_act(
    index: usize,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    let engine = engine_at(&app, index);
    if let Some(engine) = engine {
        download_engine(&app, engine);
    }
    crate::settings::settings_get(app, library)
}

/// Remove on an engine, pressed once to ask and again to delete.
#[tauri::command]
pub fn settings_engine_remove(
    index: usize,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    let engine = engine_at(&app, index);
    if let Some(engine) = engine {
        remove_engine(&app, engine);
    }
    crate::settings::settings_get(app, library)
}

/// Stops the engine download in progress.
#[tauri::command]
pub fn settings_engines_stop(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    let state = app.state::<EnginesState>();
    if let Some(download) = state
        .download
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
    {
        download.stop.store(true, Ordering::Relaxed);
    }
    crate::settings::settings_get(app, library)
}

fn engine_at(app: &AppHandle, index: usize) -> Option<Engine> {
    let state = app.state::<EnginesState>();
    state
        .rows
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(index)
        .map(|row| row.engine)
}

fn download_engine(app: &AppHandle, engine: Engine) {
    let state = app.state::<EnginesState>();
    if state
        .download
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some()
    {
        return;
    }
    let Some(Latest::Known(release)) = state
        .latest
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&engine)
        .cloned()
    else {
        return;
    };
    let own = state
        .rows
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .any(|row| row.engine == engine && row.installed.is_some());
    if own && engines_busy(app) {
        return;
    }
    let stop = Arc::new(AtomicBool::new(false));
    *state
        .download
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(Download {
        engine,
        done: 0,
        total: release.asset.size,
        stop: Arc::clone(&stop),
    });
    *state.note.lock().unwrap_or_else(PoisonError::into_inner) = None;
    let handle = app.clone();
    std::thread::spawn(move || {
        let progress_handle = handle.clone();
        let mut last: Option<Instant> = None;
        let mut progress = |done: u64, total: u64| {
            if last.is_some_and(|at| at.elapsed() < BAR_EVERY) {
                return;
            }
            last = Some(Instant::now());
            if let Some(download) = progress_handle
                .state::<EnginesState>()
                .download
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_mut()
                .filter(|download| download.engine == engine)
            {
                download.done = done;
                download.total = total;
            }
            let _ = progress_handle.emit("settings_changed", ());
        };
        let result = engines::install(engine, &release, &stop, &mut progress)
            .map(|_| ())
            .map_err(|err| format!("{err:#}"));
        let stopped = handle
            .state::<EnginesState>()
            .download
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .is_some_and(|download| download.stop.load(Ordering::Relaxed));
        refresh_engines(&handle);
        *handle
            .state::<EnginesState>()
            .note
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(match result {
            Ok(()) => (
                t!(
                    "engines-installed-note",
                    name = engine.label(),
                    tag = engine.installed().unwrap_or_default()
                ),
                true,
            ),
            Err(_) if stopped => (t!("protons-stopped", name = engine.label()), true),
            Err(err) => (err, false),
        });
        let _ = handle.emit("settings_changed", ());
    });
}

fn remove_engine(app: &AppHandle, engine: Engine) {
    let state = app.state::<EnginesState>();
    if engines_busy(app) {
        return;
    }
    let confirming = state
        .confirm_remove
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        == Some(&engine);
    if !confirming {
        *state
            .confirm_remove
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(engine);
        return;
    }
    *state
        .confirm_remove
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = None;
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = engines::remove(engine).map_err(|err| format!("{err:#}"));
        refresh_engines(&handle);
        *handle
            .state::<EnginesState>()
            .note
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(match result {
            Ok(()) => (t!("engines-removed-note", name = engine.label()), true),
            Err(err) => (err, false),
        });
        let _ = handle.emit("settings_changed", ());
    });
}

// The runtimes.

fn refresh_runtimes(app: &AppHandle) {
    let state = app.state::<RuntimesState>();
    *state.found.lock().unwrap_or_else(PoisonError::into_inner) = Runtime::ALL
        .into_iter()
        .map(|runtime| (runtime, runtimes::find(runtime)))
        .collect();
    *state.own.lock().unwrap_or_else(PoisonError::into_inner) = Runtime::ALL
        .into_iter()
        .filter_map(|runtime| match runtimes::own(runtime)? {
            Found::Zendo { version, .. } => Some((runtime, version)),
            Found::Steam(_) => None,
        })
        .collect();
}

/// The row's one button: download a runtime.
#[tauri::command]
pub fn settings_runtime_act(
    index: usize,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    let runtime = Runtime::ALL.get(index).copied();
    if let Some(runtime) = runtime {
        download_runtime(&app, runtime);
    }
    crate::settings::settings_get(app, library)
}

/// Remove on a runtime, pressed once to ask and again to delete.
#[tauri::command]
pub fn settings_runtime_remove(
    index: usize,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    let runtime = Runtime::ALL.get(index).copied();
    if let Some(runtime) = runtime {
        remove_runtime(&app, runtime);
    }
    crate::settings::settings_get(app, library)
}

fn download_runtime(app: &AppHandle, runtime: Runtime) {
    let state = app.state::<RuntimesState>();
    if state
        .downloading
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some()
    {
        return;
    }
    let Some(remote) = state
        .latest
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .and_then(|latest| {
            latest
                .iter()
                .find(|remote| remote.runtime == runtime)
                .cloned()
        })
    else {
        return;
    };
    *state
        .downloading
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(runtime);
    *state.note.lock().unwrap_or_else(PoisonError::into_inner) = None;
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = runtimes::install(&remote, &AtomicBool::new(false), &mut |_, _| {})
            .map(|_| ())
            .map_err(|err| format!("{err:#}"));
        let state = handle.state::<RuntimesState>();
        *state
            .downloading
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        refresh_runtimes(&handle);
        *state.note.lock().unwrap_or_else(PoisonError::into_inner) = Some(match result {
            Ok(()) => (t!("runtimes-downloaded", runtime = runtime.label()), true),
            Err(err) => (err, false),
        });
        let _ = handle.emit("settings_changed", ());
    });
}

fn remove_runtime(app: &AppHandle, runtime: Runtime) {
    let state = app.state::<RuntimesState>();
    let confirming = state
        .confirm_remove
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        == Some(&runtime);
    if !confirming {
        *state
            .confirm_remove
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(runtime);
        return;
    }
    *state
        .confirm_remove
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = None;
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = runtimes::remove(runtime).map_err(|err| format!("{err:#}"));
        refresh_runtimes(&handle);
        *handle
            .state::<RuntimesState>()
            .note
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(match result {
            Ok(()) => (t!("runtimes-removed", runtime = runtime.label()), true),
            Err(err) => (err, false),
        });
        let _ = handle.emit("settings_changed", ());
    });
}

// The overlay.

/// Installs the overlay, or updates it once installed.
#[tauri::command]
pub fn settings_overlay_do_install(
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    install_overlay(&app);
    crate::settings::settings_get(app, library)
}

/// Removes the overlay, pressed once to ask and again to remove.
#[tauri::command]
pub fn settings_overlay_do_remove(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    remove_overlay(&app);
    crate::settings::settings_get(app, library)
}

fn install_overlay(app: &AppHandle) {
    let state = app.state::<OverlayState>();
    if *state.step.lock().unwrap_or_else(PoisonError::into_inner) == Step::Working {
        return;
    }
    *state.step.lock().unwrap_or_else(PoisonError::into_inner) = Step::Working;
    *state.note.lock().unwrap_or_else(PoisonError::into_inner) = None;
    let done = if state.installed.load(Ordering::Relaxed) {
        t!("overlay-updated")
    } else {
        t!("overlay-installed")
    };
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = eos_overlay::install().map_err(|err| format!("{err:#}"));
        overlay_done(&handle, done, result);
    });
}

fn remove_overlay(app: &AppHandle) {
    let state = app.state::<OverlayState>();
    let mut step = state.step.lock().unwrap_or_else(PoisonError::into_inner);
    match *step {
        Step::Working => return,
        Step::Idle => {
            *step = Step::ConfirmRemove;
            return;
        }
        Step::ConfirmRemove => *step = Step::Working,
    }
    drop(step);
    *state.note.lock().unwrap_or_else(PoisonError::into_inner) = None;
    let done = t!("overlay-removed");
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = eos_overlay::remove().map_err(|err| format!("{err:#}"));
        overlay_done(&handle, done, result);
    });
}

fn overlay_done(app: &AppHandle, done: String, result: Result<(), String>) {
    let state = app.state::<OverlayState>();
    *state.step.lock().unwrap_or_else(PoisonError::into_inner) = Step::Idle;
    state
        .installed
        .store(eos_overlay::installed(), Ordering::Relaxed);
    *state.note.lock().unwrap_or_else(PoisonError::into_inner) = Some(match result {
        Ok(()) => (done, true),
        Err(err) => (err, false),
    });
    let _ = app.emit("settings_changed", ());
}

// What the page shows.

/// The engines' rows.
pub(crate) fn engines_rows(app: &AppHandle) -> Vec<SetRowDto> {
    let state = app.state::<EnginesState>();
    let busy = engines_busy(app);
    let downloading = state
        .download
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some();
    let latest = state.latest.lock().unwrap_or_else(PoisonError::into_inner);
    let confirm = *state
        .confirm_remove
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    state
        .rows
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .map(|row| {
            let engine = row.engine;
            let whose = match row.found.as_ref().map(|found| found.origin) {
                Some(Origin::Zendo) => t!("engines-found-zendo"),
                Some(Origin::Path) => t!("engines-found-path"),
                Some(Origin::Heroic) => t!("engines-found-heroic"),
                None => t!("engines-missing"),
            };
            let (mut note, mut bad) = match latest.get(&engine) {
                Some(Latest::Asking) => (t!("protons-asking"), false),
                Some(Latest::Known(release)) => {
                    (t!("engines-latest", tag = release.tag.as_str()), false)
                }
                Some(Latest::Failed(err)) => (err.clone(), true),
                None => (String::new(), false),
            };
            if busy && row.installed.is_some() {
                if !note.is_empty() {
                    note.push('\n');
                }
                note.push_str(&t!("engines-busy"));
                bad = false;
            }
            let known = match latest.get(&engine) {
                Some(Latest::Known(release)) => Some(release),
                _ => None,
            };
            let (mut action, mut action_on, mut status) = (String::new(), false, String::new());
            match (&row.installed, known) {
                (Some(tag), Some(release)) if &release.tag != tag => {
                    action = t!("engines-update", tag = release.tag.as_str());
                    action_on = !downloading && !busy;
                }
                (Some(_), Some(_)) => status = t!("engines-up-to-date"),
                (None, Some(release)) => {
                    action = t!("engines-download-tag", tag = release.tag.as_str());
                    action_on = !downloading;
                }
                _ => {}
            }
            let confirming = confirm == Some(engine);
            SetRowDto {
                title: engine.label().to_owned(),
                sub: format!("{} · {whose}", engine.purpose()),
                path: row
                    .found
                    .as_ref()
                    .map_or_default(|found| found.path.display().to_string()),
                note,
                note_tone: if bad { 2 } else { 0 },
                action,
                action_on,
                remove: if row.installed.is_none() {
                    String::new()
                } else if confirming {
                    t!("really-remove")
                } else {
                    t!("remove")
                },
                remove_on: !busy,
                remove_danger: confirming,
                status,
                status_tone: 1,
            }
        })
        .collect()
}

/// The engines' download bar.
pub(crate) fn engines_bar(app: &AppHandle) -> BarDto {
    let state = app.state::<EnginesState>();
    let download = state
        .download
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let Some(download) = download.as_ref() else {
        return BarDto::default();
    };
    bar_of(
        download.engine.label(),
        download.done,
        download.total,
        download.stop.load(Ordering::Relaxed),
    )
}

/// The engines' note.
pub(crate) fn engines_note(app: &AppHandle) -> Option<RowDto> {
    let state = app.state::<EnginesState>();
    note_of(
        state
            .note
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref(),
    )
}

/// The runtimes' rows.
pub(crate) fn runtimes_rows(app: &AppHandle) -> Vec<SetRowDto> {
    let state = app.state::<RuntimesState>();
    let confirm = *state
        .confirm_remove
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let downloading = *state
        .downloading
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let latest = state.latest.lock().unwrap_or_else(PoisonError::into_inner);
    let found = state.found.lock().unwrap_or_else(PoisonError::into_inner);
    let own = state.own.lock().unwrap_or_else(PoisonError::into_inner);
    let own_version = |runtime: Runtime| {
        own.iter()
            .find(|(r, _)| *r == runtime)
            .and_then(|(_, version)| version.clone())
    };
    let has_own = |runtime: Runtime| own.iter().any(|(r, _)| *r == runtime);
    found
        .iter()
        .map(|(runtime, found)| {
            let runtime = *runtime;
            let status = match found {
                Some(Found::Steam(_)) => t!("runtimes-from-steam"),
                Some(Found::Zendo { version, .. }) => match version {
                    Some(version) => t!("runtimes-downloaded-on", date = day(version)),
                    None => t!("runtimes-downloaded-undated"),
                },
                None => t!("runtimes-not-here"),
            };
            let remote = latest
                .as_ref()
                .and_then(|latest| latest.iter().find(|remote| remote.runtime == runtime));
            let offer = match remote {
                Some(_) if !has_own(runtime) && !matches!(found, Some(Found::Steam(_))) => {
                    Some(t!("download"))
                }
                Some(remote)
                    if has_own(runtime)
                        && own_version(runtime).as_deref() != Some(remote.version.as_str()) =>
                {
                    Some(t!("runtimes-update", date = day(&remote.version)))
                }
                _ => None,
            };
            let this_downloading = downloading == Some(runtime);
            let confirming = confirm == Some(runtime);
            SetRowDto {
                title: runtime.label().to_owned(),
                sub: status,
                path: found
                    .as_ref()
                    .map_or_else(String::new, |found| found.path().display().to_string()),
                note: String::new(),
                note_tone: 0,
                action: if this_downloading {
                    String::new()
                } else {
                    offer.unwrap_or_default()
                },
                action_on: downloading.is_none(),
                remove: if has_own(runtime) && !this_downloading {
                    if confirming {
                        t!("really-remove")
                    } else {
                        t!("remove")
                    }
                } else {
                    String::new()
                },
                remove_on: true,
                remove_danger: confirming,
                status: if this_downloading {
                    t!("runtimes-downloading")
                } else {
                    String::new()
                },
                status_tone: 0,
            }
        })
        .collect()
}

/// The runtimes' note.
pub(crate) fn runtimes_note(app: &AppHandle) -> Option<RowDto> {
    let state = app.state::<RuntimesState>();
    if state.asking.load(Ordering::Relaxed) {
        return Some(RowDto {
            label: String::new(),
            value: t!("runtimes-asking"),
            tone: 3,
        });
    }
    if let Some(err) = state
        .error
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
    {
        return Some(RowDto {
            label: String::new(),
            value: err.clone(),
            tone: 2,
        });
    }
    note_of(
        state
            .note
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref(),
    )
}

/// The overlay's card.
pub(crate) fn overlay_dto(app: &AppHandle) -> OverlayDto {
    let state = app.state::<OverlayState>();
    let installed = state.installed.load(Ordering::Relaxed);
    let engine = state.engine.load(Ordering::Relaxed);
    let working = *state.step.lock().unwrap_or_else(PoisonError::into_inner) == Step::Working;
    let status = if working {
        t!("overlay-working")
    } else if installed {
        t!(
            "overlay-is-installed",
            path = eos_overlay::folder().map_or_default(|folder| folder.display().to_string())
        )
    } else if engine {
        t!("overlay-not-installed")
    } else {
        t!("overlay-no-legendary")
    };
    let ready = engine && !working;
    OverlayDto {
        status,
        install: if installed {
            t!("overlay-update")
        } else {
            t!("overlay-install")
        },
        install_on: ready,
        remove: if !installed {
            String::new()
        } else if *state.step.lock().unwrap_or_else(PoisonError::into_inner) == Step::ConfirmRemove
        {
            t!("really-remove")
        } else {
            t!("remove")
        },
        remove_on: ready,
        note: note_of(
            state
                .note
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref(),
        ),
    }
}

/// A note line, when there is one: 1 good, 2 bad.
fn note_of(note: Option<&(String, bool)>) -> Option<RowDto> {
    note.map(|(note, good)| RowDto {
        label: String::new(),
        value: note.clone(),
        tone: if *good { 1 } else { 2 },
    })
}

/// The day of a Lutris build time: `2024-01-14` of
/// `2024-01-14T19:38:26.671443Z`.
fn day(version: &str) -> &str {
    version.get(..10).unwrap_or(version)
}
