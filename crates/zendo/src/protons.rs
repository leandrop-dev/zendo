//! The Proton group of Settings: the default Proton, the Protons found,
//! GE-Proton or Proton-CachyOS releases to download (the latest few, older
//! ones on asking), one at a time with its progress, and Remove for the ones
//! in the user's own tools folder. The work that waits on GitHub or a
//! download runs on a thread of its own.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use zendo_core::Size;
use zendo_core::config::Config;
use zendo_core::ge_proton::{self, Build, Release};
use zendo_core::launch_options::LaunchOptions;
use zendo_core::proton::{self, Proton};
use zendo_shell::t;

use crate::library::LibraryState;
use crate::settings::{BarDto, RowDto, SetRowDto, SettingsDto, bar_of};

/// How many releases the card offers at first.
const SHOWN: usize = 5;
/// How many more each "Show older versions" adds.
const OLDER: usize = 10;
/// How often a download's bar reaches the page.
const BAR_EVERY: Duration = Duration::from_millis(100);

/// What is known of one build's releases.
struct List {
    releases: Vec<Release>,
    pages: u32,
    more: bool,
    shown: usize,
    error: Option<String>,
    asking: bool,
}

impl Default for List {
    fn default() -> Self {
        Self {
            releases: Vec::new(),
            pages: 0,
            more: false,
            shown: SHOWN,
            error: None,
            asking: false,
        }
    }
}

fn slot(build: Build) -> usize {
    match build {
        Build::Ge => 0,
        Build::CachyOs => 1,
    }
}

struct Download {
    name: String,
    done: u64,
    total: u64,
    stop: Arc<AtomicBool>,
}

/// The Protons found and the releases on offer.
pub struct ProtonsState {
    installed: Mutex<Vec<Proton>>,
    build: Mutex<Build>,
    lists: Mutex<[List; 2]>,
    download: Mutex<Option<Download>>,
    /// The Proton whose Remove was pressed once, and the games using it.
    confirm_remove: Mutex<Option<(String, usize)>>,
    note: Mutex<Option<(String, bool)>>,
    /// A removal put the default back to Automatic and said so.
    default_reset: AtomicBool,
    ready: AtomicBool,
}

impl Default for ProtonsState {
    fn default() -> Self {
        Self {
            installed: Mutex::default(),
            build: Mutex::new(Build::default()),
            lists: Mutex::default(),
            download: Mutex::default(),
            confirm_remove: Mutex::default(),
            note: Mutex::default(),
            default_reset: AtomicBool::new(false),
            ready: AtomicBool::new(false),
        }
    }
}

/// The group, as the page draws it.
#[derive(Debug, Serialize)]
pub struct ProtonsDto {
    defaults: Vec<DefaultChip>,
    installed: Vec<SetRowDto>,
    installed_none: String,
    builds: Vec<BuildChip>,
    releases_title: String,
    releases: Vec<SetRowDto>,
    refresh_on: bool,
    line: Option<RowDto>,
    older: bool,
    bar: BarDto,
    note: Option<RowDto>,
}

/// A default Proton chip: `None` is Automatic.
#[derive(Debug, Serialize)]
pub struct DefaultChip {
    name: Option<String>,
    label: String,
    chosen: bool,
}

/// A build's chip, chosen by its index in [`Build::ALL`].
#[derive(Debug, Serialize)]
pub struct BuildChip {
    label: String,
    chosen: bool,
}

/// Looks at the Protons and asks GitHub for releases, once as the page first
/// opens.
pub(crate) fn ready(app: &AppHandle) {
    let state = app.state::<ProtonsState>();
    if state.ready.swap(true, Ordering::Relaxed) {
        return;
    }
    read_protons(app);
    let build = *state.build.lock().unwrap_or_else(PoisonError::into_inner);
    let fresh = {
        let lists = state.lists.lock().unwrap_or_else(PoisonError::into_inner);
        let list = &lists[slot(build)];
        list.pages == 0 && !list.asking
    };
    if fresh {
        ask_page(app, build, 1);
    }
}

/// Looks at the Protons on the machine.
fn read_protons(app: &AppHandle) {
    let state = app.state::<ProtonsState>();
    *state
        .installed
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = proton::installed();
}

/// A default Proton was chosen: says which one is used now.
pub(crate) fn default_proton_set(app: &AppHandle) {
    let state = app.state::<ProtonsState>();
    if state.default_reset.swap(false, Ordering::Relaxed) {
        return;
    }
    let config = app
        .state::<crate::settings::SettingsState>()
        .config()
        .clone();
    let installed = state
        .installed
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let shown = config
        .default_proton
        .as_deref()
        .and_then(|name| installed.iter().find(|found| found.name == name))
        .or_else(|| installed.first())
        .map_or_else(|| t!("protons-umu"), |found| found.name.clone());
    *state.note.lock().unwrap_or_else(PoisonError::into_inner) =
        Some((t!("protons-default-set", name = shown), true));
}

/// Remove on an installed Proton, pressed once to ask and again to delete.
#[tauri::command]
pub fn settings_proton_remove(
    index: usize,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    let name = app
        .state::<ProtonsState>()
        .installed
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(index)
        .map(|found| found.name.clone());
    if let Some(name) = name {
        remove_proton(&app, &name);
    }
    crate::settings::settings_get(app, library)
}

/// The build whose releases are on show.
#[tauri::command]
pub fn settings_build_chosen(
    index: usize,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    if let Some(build) = Build::ALL.get(index).copied() {
        set_build(&app, build);
    }
    crate::settings::settings_get(app, library)
}

/// Reads the Protons and the releases again.
#[tauri::command]
pub fn settings_proton_refresh(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    read_protons(&app);
    let state = app.state::<ProtonsState>();
    let build = *state.build.lock().unwrap_or_else(PoisonError::into_inner);
    let ask = {
        let mut lists = state.lists.lock().unwrap_or_else(PoisonError::into_inner);
        let list = &mut lists[slot(build)];
        list.shown = SHOWN;
        !list.asking
    };
    if ask {
        ask_page(&app, build, 1);
    }
    crate::settings::settings_get(app, library)
}

/// Shows more of the build's older releases.
#[tauri::command]
pub fn settings_proton_show_older(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    let state = app.state::<ProtonsState>();
    let build = *state.build.lock().unwrap_or_else(PoisonError::into_inner);
    let next = {
        let mut lists = state.lists.lock().unwrap_or_else(PoisonError::into_inner);
        let list = &mut lists[slot(build)];
        list.shown += OLDER;
        (list.shown > list.releases.len() && list.more && !list.asking).then(|| list.pages + 1)
    };
    if let Some(page) = next {
        ask_page(&app, build, page);
    }
    crate::settings::settings_get(app, library)
}

/// Downloads a release.
#[tauri::command]
pub fn settings_release_download(
    index: usize,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    let state = app.state::<ProtonsState>();
    let build = *state.build.lock().unwrap_or_else(PoisonError::into_inner);
    let release = state.lists.lock().unwrap_or_else(PoisonError::into_inner)[slot(build)]
        .releases
        .get(index)
        .cloned();
    if let Some(release) = release {
        download_proton(&app, release);
    }
    crate::settings::settings_get(app, library)
}

/// Stops the Proton download in progress.
#[tauri::command]
pub fn settings_proton_stop(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    let state = app.state::<ProtonsState>();
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

fn set_build(app: &AppHandle, build: Build) {
    let state = app.state::<ProtonsState>();
    *state.build.lock().unwrap_or_else(PoisonError::into_inner) = build;
    let fresh = {
        let lists = state.lists.lock().unwrap_or_else(PoisonError::into_inner);
        let list = &lists[slot(build)];
        list.pages == 0 && !list.asking
    };
    if fresh {
        ask_page(app, build, 1);
    }
}

fn ask_page(app: &AppHandle, build: Build, page: u32) {
    {
        let state = app.state::<ProtonsState>();
        let mut lists = state.lists.lock().unwrap_or_else(PoisonError::into_inner);
        let list = &mut lists[slot(build)];
        list.asking = true;
        list.error = None;
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = ge_proton::releases(build, page).map_err(|err| format!("{err:#}"));
        let state = handle.state::<ProtonsState>();
        {
            let mut lists = state.lists.lock().unwrap_or_else(PoisonError::into_inner);
            let list = &mut lists[slot(build)];
            list.asking = false;
            match result {
                Ok((releases, more)) => {
                    if page == 1 {
                        list.releases.clear();
                    }
                    list.releases.extend(releases);
                    list.pages = page;
                    list.more = more;
                }
                Err(err) => list.error = Some(err),
            }
        }
        let _ = handle.emit("settings_changed", ());
    });
}

fn download_proton(app: &AppHandle, release: Release) {
    let state = app.state::<ProtonsState>();
    if state
        .download
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some()
    {
        return;
    }
    let stop = Arc::new(AtomicBool::new(false));
    *state
        .download
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(Download {
        name: release.name.clone(),
        done: 0,
        total: release.size,
        stop: Arc::clone(&stop),
    });
    *state.note.lock().unwrap_or_else(PoisonError::into_inner) = None;
    let handle = app.clone();
    std::thread::spawn(move || {
        let name = release.name.clone();
        let progress_handle = handle.clone();
        let mut last: Option<Instant> = None;
        let mut progress = |done: u64, total: u64| {
            if last.is_some_and(|at| at.elapsed() < BAR_EVERY) {
                return;
            }
            last = Some(Instant::now());
            if let Some(download) = progress_handle
                .state::<ProtonsState>()
                .download
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_mut()
                .filter(|download| download.name == name)
            {
                download.done = done;
                download.total = total;
            }
            let _ = progress_handle.emit("settings_changed", ());
        };
        let result = ge_proton::install(&release, &stop, &mut progress)
            .map(|_| ())
            .map_err(|err| format!("{err:#}"));
        let stopped = handle
            .state::<ProtonsState>()
            .download
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .is_some_and(|download| download.stop.load(Ordering::Relaxed));
        read_protons(&handle);
        *handle
            .state::<ProtonsState>()
            .note
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(match result {
            Ok(()) => (t!("protons-installed-note", name = name), true),
            Err(_) if stopped => (t!("protons-stopped", name = name), true),
            Err(err) => (err, false),
        });
        let _ = handle.emit("settings_changed", ());
    });
}

fn remove_proton(app: &AppHandle, name: &str) {
    let state = app.state::<ProtonsState>();
    let confirming = state
        .confirm_remove
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .is_some_and(|(chosen, _)| chosen == name);
    if !confirming {
        let users = games_using(name);
        *state
            .confirm_remove
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some((name.to_owned(), users));
        return;
    }
    *state
        .confirm_remove
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = None;
    let found = state
        .installed
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find(|found| found.name == name)
        .cloned();
    let Some(found) = found else {
        return;
    };
    let handle = app.clone();
    let removed = name.to_owned();
    std::thread::spawn(move || {
        let result = ge_proton::remove(&found).map_err(|err| format!("{err:#}"));
        proton_removed(&handle, &removed, result);
    });
}

/// What came of a Remove: the list reads again, and a default that is gone
/// goes back to Automatic.
fn proton_removed(app: &AppHandle, name: &str, result: Result<(), String>) {
    let state = app.state::<ProtonsState>();
    read_protons(app);
    if let Err(err) = result {
        *state.note.lock().unwrap_or_else(PoisonError::into_inner) = Some((err, false));
        let _ = app.emit("settings_changed", ());
        return;
    }
    let reset = reset_games_using(name);
    let was_default = app
        .state::<crate::settings::SettingsState>()
        .config()
        .default_proton
        .as_deref()
        == Some(name);
    let note = if was_default {
        state.default_reset.store(true, Ordering::Relaxed);
        crate::settings::save(app, crate::settings::Setting::DefaultProton(None));
        t!("protons-removed-default", name = name)
    } else if reset > 0 {
        t!("protons-reset", name = name, count = reset)
    } else {
        t!("protons-removed", name = name)
    };
    *state.note.lock().unwrap_or_else(PoisonError::into_inner) = Some((note, true));
    let _ = app.emit("settings_changed", ());
}

/// Puts every game that chose this Proton back to Automatic, and says how
/// many.
fn reset_games_using(name: &str) -> usize {
    let Ok(all) = LaunchOptions::load_all() else {
        return 0;
    };
    let mut reset = 0;
    for (id, mut options) in all {
        if options.proton.as_deref() == Some(name) {
            options.proton = None;
            if options.save(&id).is_ok() {
                reset += 1;
            }
        }
    }
    reset
}

/// How many games' launch options chose this Proton.
fn games_using(name: &str) -> usize {
    LaunchOptions::load_all().map_or(0, |all| {
        all.values()
            .filter(|options| options.proton.as_deref() == Some(name))
            .count()
    })
}

/// Automatic, each Proton found, and the one chosen even when gone.
fn default_choices(app: &AppHandle, config: &Config) -> Vec<DefaultChip> {
    let state = app.state::<ProtonsState>();
    let installed = state
        .installed
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let automatic = match installed.first() {
        Some(first) => t!("protons-automatic", name = first.name.as_str()),
        None => t!("protons-automatic-umu"),
    };
    let mut choices = vec![(None, automatic)];
    choices.extend(
        installed
            .iter()
            .map(|found| (Some(found.name.clone()), found.name.clone())),
    );
    if let Some(chosen) = &config.default_proton
        && !choices
            .iter()
            .any(|(name, _)| name.as_ref() == Some(chosen))
    {
        choices.push((
            Some(chosen.clone()),
            t!("protons-missing", name = chosen.as_str()),
        ));
    }
    choices
        .into_iter()
        .map(|(name, label)| DefaultChip {
            chosen: name == config.default_proton,
            name,
            label,
        })
        .collect()
}

/// The group as the page draws it.
pub(crate) fn protons_dto(app: &AppHandle, config: &Config) -> ProtonsDto {
    let state = app.state::<ProtonsState>();
    let defaults = default_choices(app, config);
    let build = *state.build.lock().unwrap_or_else(PoisonError::into_inner);
    let confirm = state
        .confirm_remove
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let installed = state
        .installed
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let installed_none = if installed.is_empty() {
        t!("protons-none")
    } else {
        String::new()
    };
    let installed_rows = installed_rows(&installed, confirm.as_ref());
    drop(installed);
    let lists = state.lists.lock().unwrap_or_else(PoisonError::into_inner);
    let list = &lists[slot(build)];
    let idle = state
        .download
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_none();
    let releases = release_rows(app, list, idle);
    let line = if list.asking {
        Some(RowDto {
            label: String::new(),
            value: t!("protons-asking"),
            tone: 3,
        })
    } else {
        list.error.as_ref().map(|err| RowDto {
            label: String::new(),
            value: err.clone(),
            tone: 2,
        })
    };
    let older =
        !list.asking && list.error.is_none() && (list.releases.len() > list.shown || list.more);
    let bar = {
        let download = state
            .download
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match download.as_ref() {
            Some(download) => bar_of(
                &download.name,
                download.done,
                download.total,
                download.stop.load(Ordering::Relaxed),
            ),
            None => BarDto::default(),
        }
    };
    let note = state
        .note
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|(note, good)| RowDto {
            label: String::new(),
            value: note.clone(),
            tone: if *good { 1 } else { 2 },
        });
    ProtonsDto {
        defaults,
        installed: installed_rows,
        installed_none,
        builds: Build::ALL
            .into_iter()
            .map(|each| BuildChip {
                label: each.label().to_owned(),
                chosen: each == build,
            })
            .collect(),
        releases_title: t!("protons-releases", build = build.label()),
        releases,
        refresh_on: !list.asking,
        line,
        older,
        bar,
        note,
    }
}

/// The Protons found, each with its Remove button.
fn installed_rows(installed: &[Proton], confirm: Option<&(String, usize)>) -> Vec<SetRowDto> {
    installed
        .iter()
        .map(|found| {
            let (confirming, users) = match confirm {
                Some((chosen, users)) if chosen == &found.name => (true, *users),
                _ => (false, 0),
            };
            let removable = ge_proton::removable(found);
            SetRowDto {
                title: found.name.clone(),
                path: found.path.display().to_string(),
                note: if users > 0 {
                    t!("protons-chosen-by", count = users)
                } else {
                    String::new()
                },
                note_tone: 2,
                remove: if !removable {
                    String::new()
                } else if confirming {
                    t!("really-remove")
                } else {
                    t!("remove")
                },
                remove_on: true,
                remove_danger: confirming,
                ..SetRowDto::default()
            }
        })
        .collect()
}

/// A build's releases, with Download for the ones not here yet.
fn release_rows(app: &AppHandle, list: &List, idle: bool) -> Vec<SetRowDto> {
    list.releases
        .iter()
        .take(list.shown)
        .map(|release| {
            let here = app
                .state::<ProtonsState>()
                .installed
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .any(|found| ge_proton::is_release(found, release));
            SetRowDto {
                title: release.name.clone(),
                sub: format!("{} · {}", release.published, Size::Bytes(release.size)),
                action: if here { String::new() } else { t!("download") },
                action_on: idle,
                status: if here { t!("installed") } else { String::new() },
                status_tone: 1,
                ..SetRowDto::default()
            }
        })
        .collect()
}
