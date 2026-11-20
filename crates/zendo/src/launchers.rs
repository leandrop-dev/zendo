//! The Launchers group of Settings: the programs that need a window of their
//! own. Nexus Launcher, brought as its `AppImage`, and Greenlight, which
//! streams an Xbox Series S's screen. Their sign-in and their games stay with
//! them. The work that waits on the network or a child's window runs on a
//! thread of its own.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, State};
use zendo_core::{greenlight, xodus};
use zendo_shell::t;

use crate::library::{self, LibraryState};
use crate::settings::{BarDto, SetRowDto, SettingsDto, bar_of};

/// How often a download's bar reaches the page.
const BAR_EVERY: Duration = Duration::from_millis(150);

/// Which launcher a download or a row is about.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Launcher {
    Nexus,
    Greenlight,
}

/// What Nexus Launcher's row shows.
#[derive(Default)]
struct Nexus {
    appimage: Option<PathBuf>,
    games: usize,
    running: bool,
    note: Option<(String, bool)>,
}

/// What the Xbox Series S's row shows: Greenlight found, and the last word.
#[derive(Default)]
struct Series {
    program: Option<PathBuf>,
    note: Option<(String, bool)>,
}

struct Download {
    which: Launcher,
    done: u64,
    total: u64,
    stop: Arc<AtomicBool>,
}

/// What the window remembers of the two launchers, and the one download.
pub struct LaunchersState {
    nexus: Mutex<Nexus>,
    series: Mutex<Series>,
    download: Mutex<Option<Download>>,
    ready: AtomicBool,
}

impl LaunchersState {
    fn nexus(&self) -> std::sync::MutexGuard<'_, Nexus> {
        self.nexus.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn series(&self) -> std::sync::MutexGuard<'_, Series> {
        self.series.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for LaunchersState {
    fn default() -> Self {
        Self {
            nexus: Mutex::default(),
            series: Mutex::default(),
            download: Mutex::default(),
            ready: AtomicBool::new(false),
        }
    }
}

/// Looks at the launchers, once as the page first opens.
pub(crate) fn ready(app: &AppHandle) {
    let state = app.state::<LaunchersState>();
    if state.ready.swap(true, Ordering::Relaxed) {
        return;
    }
    refresh(app);
}

/// Looks at the launchers again, after a change or a window closed.
fn refresh(app: &AppHandle) {
    let state = app.state::<LaunchersState>();
    state.nexus().appimage = xodus::installed();
    state.nexus().games = xodus::count();
    state.series().program = greenlight::installed();
}

/// A row's one button: open a launcher, or bring one.
#[tauri::command]
pub fn settings_launcher_act(
    index: usize,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    match index {
        0 => nexus_act(&app),
        1 => series_act(&app),
        _ => {}
    }
    crate::settings::settings_get(app, library)
}

/// Stops the download in progress, if there is one.
#[tauri::command]
pub fn settings_launchers_stop(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    let state = app.state::<LaunchersState>();
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

fn nexus_act(app: &AppHandle) {
    let installed = app.state::<LaunchersState>().nexus().appimage.is_some();
    if installed {
        open_nexus(app);
    } else {
        download(app, Launcher::Nexus);
    }
}

fn series_act(app: &AppHandle) {
    if app.state::<LaunchersState>().series().program.is_none() {
        download(app, Launcher::Greenlight);
        return;
    }
    match greenlight::open() {
        Ok(mut child) => {
            app.state::<LaunchersState>().series().note = Some((t!("series-opened"), true));
            let handle = app.clone();
            std::thread::spawn(move || {
                let _ = child.wait();
                let state = handle.state::<LaunchersState>();
                state.series().program = greenlight::installed();
                let _ = handle.emit("settings_changed", ());
            });
        }
        Err(err) => {
            app.state::<LaunchersState>().series().note = Some((format!("{err:#}"), false));
        }
    }
}

/// Opens Nexus Launcher, which writes its profile the first time and needs
/// its runtime and a sign-in done in its own window.
fn open_nexus(app: &AppHandle) {
    let state = app.state::<LaunchersState>();
    if state.nexus().running {
        return;
    }
    match xodus::open() {
        Ok(mut child) => {
            state.nexus().running = true;
            state.nexus().note = Some((t!("xbox-running"), true));
            let handle = app.clone();
            std::thread::spawn(move || {
                let _ = child.wait();
                let state = handle.state::<LaunchersState>();
                state.nexus().running = false;
                state.nexus().note = None;
                refresh(&handle);
                let _ = handle.emit("settings_changed", ());
                // Nexus Launcher may have gained games.
                library::rescan(handle);
            });
        }
        Err(err) => {
            state.nexus().note = Some((format!("{err:#}"), false));
        }
    }
}

/// Brings a launcher: its `AppImage`.
fn download(app: &AppHandle, which: Launcher) {
    let state = app.state::<LaunchersState>();
    {
        let mut download = state
            .download
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if download.is_some() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        *download = Some(Download {
            which,
            done: 0,
            total: 0,
            stop: Arc::clone(&stop),
        });
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        let state = handle.state::<LaunchersState>();
        let stop = {
            let download = state
                .download
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let Some(download) = download.as_ref() else {
                return;
            };
            Arc::clone(&download.stop)
        };
        let progress_handle = handle.clone();
        let mut last: Option<Instant> = None;
        let mut progress = |done: u64, total: u64| {
            if last.is_some_and(|at| at.elapsed() < BAR_EVERY) {
                return;
            }
            last = Some(Instant::now());
            if let Some(download) = progress_handle
                .state::<LaunchersState>()
                .download
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_mut()
            {
                download.done = done;
                download.total = total;
            }
            let _ = progress_handle.emit("settings_changed", ());
        };
        let result = match which {
            Launcher::Nexus => xodus::release()
                .and_then(|release| xodus::install(&release, &stop, &mut progress))
                .map(drop),
            Launcher::Greenlight => greenlight::release()
                .and_then(|release| greenlight::install(&release, &stop, &mut progress))
                .map(drop),
        }
        .map_err(|err| format!("{err:#}"));
        {
            let mut download = state
                .download
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            *download = None;
        }
        let note = match result {
            Ok(()) => match which {
                Launcher::Nexus => {
                    state.nexus().appimage = xodus::installed();
                    (t!("xbox-downloaded"), true)
                }
                Launcher::Greenlight => {
                    state.series().program = greenlight::installed();
                    (t!("series-downloaded"), true)
                }
            },
            Err(err) => (err, false),
        };
        match which {
            Launcher::Nexus => state.nexus().note = Some(note),
            Launcher::Greenlight => state.series().note = Some(note),
        }
        let _ = handle.emit("settings_changed", ());
    });
}

/// The group's rows: Nexus Launcher, then the Xbox Series S.
pub(crate) fn rows(app: &AppHandle) -> Vec<SetRowDto> {
    let state = app.state::<LaunchersState>();
    let downloading = state
        .download
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some();
    vec![nexus_row(app, downloading), series_row(app, downloading)]
}

fn nexus_row(app: &AppHandle, downloading: bool) -> SetRowDto {
    let launchers = app.state::<LaunchersState>();
    let state = launchers.nexus();
    let installed = state.appimage.is_some();
    let (action, action_on) = if installed {
        (t!("xbox-open"), !downloading && !state.running)
    } else {
        (t!("xbox-install"), !downloading)
    };
    let (status, status_tone) = match (installed, state.games) {
        (true, 1..) => (t!("count-games", count = state.games), 1),
        (true, _) => (t!("launcher-installed"), 1),
        (false, _) => (t!("launcher-not-installed"), 0),
    };
    row(
        t!("xbox-title"),
        t!("xbox-about"),
        state
            .appimage
            .as_ref()
            .map_or_else(String::new, |path| path.display().to_string()),
        state.note.as_ref(),
        action,
        action_on,
        status,
        status_tone,
    )
}

fn series_row(app: &AppHandle, downloading: bool) -> SetRowDto {
    let launchers = app.state::<LaunchersState>();
    let state = launchers.series();
    let installed = state.program.is_some();
    row(
        t!("series-title"),
        t!("series-about"),
        state
            .program
            .as_ref()
            .map_or_else(String::new, |path| path.display().to_string()),
        state.note.as_ref(),
        if installed {
            t!("series-open")
        } else {
            t!("series-download")
        },
        !downloading || installed,
        if installed {
            t!("launcher-installed")
        } else {
            t!("launcher-not-installed")
        },
        i32::from(installed),
    )
}

/// One row of the group, with all its texts.
#[expect(
    clippy::too_many_arguments,
    reason = "one argument per field of the row"
)]
fn row(
    title: String,
    sub: String,
    path: String,
    note: Option<&(String, bool)>,
    action: String,
    action_on: bool,
    status: String,
    status_tone: i32,
) -> SetRowDto {
    let (note, note_tone) = note.map_or_else(
        || (String::new(), 0),
        |(note, good)| (note.clone(), if *good { 1 } else { 2 }),
    );
    SetRowDto {
        title,
        sub,
        path,
        note,
        note_tone,
        action,
        action_on,
        remove: String::new(),
        remove_on: false,
        remove_danger: false,
        status,
        status_tone,
    }
}

/// The download's bar, when one runs.
pub(crate) fn bar(app: &AppHandle) -> BarDto {
    let state = app.state::<LaunchersState>();
    let download = state
        .download
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let Some(download) = download.as_ref() else {
        return BarDto::default();
    };
    let name = match download.which {
        Launcher::Nexus => t!("xbox-launcher"),
        Launcher::Greenlight => t!("series-launcher"),
    };
    bar_of(
        &name,
        download.done,
        download.total,
        download.stop.load(Ordering::Relaxed),
    )
}
