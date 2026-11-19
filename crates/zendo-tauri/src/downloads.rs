//! The downloads: the queue behind the Downloads page, and a store game's
//! Install, Import and Resume.
//!
//! The queue's state and its rules are [`zendo_shell::queue`]'s (shared with
//! the Slint window); what is here carries its effects out (the job's
//! thread, the files it keeps, the lines it says) and writes the page. The
//! page follows a running job's progress by looking again every so often,
//! since the engine writes its log many times a second.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use zendo_core::Size;
use zendo_core::downloads::{self, Action, Job};
use zendo_core::info::Dlc;
use zendo_core::owned::{Owned, Shop};
use zendo_shell::file_dialog::{self, Kind};
use zendo_shell::queue::{self, Effect, Queue, Report, Then};
use zendo_shell::store_facts::InfoState;
use zendo_shell::t;

use crate::library::LibraryState;
use crate::settings::SettingsState;
use crate::store::StoreState;

/// The queue, and what the pages keep about it.
pub struct DownloadsState {
    pub(crate) queue: Mutex<Queue>,
    /// The store games whose "Also install its DLC" was unticked.
    skip_dlc: Mutex<HashSet<String>>,
    /// The line the queue last said, for the foot of the page.
    message: Mutex<Option<String>>,
    /// What was left from last time came in.
    loaded: AtomicBool,
}

impl Default for DownloadsState {
    fn default() -> Self {
        Self {
            queue: Mutex::new(Queue::default()),
            skip_dlc: Mutex::default(),
            message: Mutex::default(),
            loaded: AtomicBool::new(false),
        }
    }
}

/// The queue, poisoned or not: a job never leaves it half-written.
pub(crate) fn lock(state: &DownloadsState) -> MutexGuard<'_, Queue> {
    state.queue.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Whether a download runs now, so a store engine must not be replaced and
/// the window does not close under it.
#[must_use]
pub fn is_running(app: &AppHandle) -> bool {
    let state = app.state::<DownloadsState>();
    lock(&state).is_running()
}

/// Reads the queue left from last time and starts what is due, as the Slint
/// window does at its start.
pub fn start(app: AppHandle) {
    {
        let state = app.state::<DownloadsState>();
        state.loaded.store(true, Ordering::Relaxed);
    }
    match downloads::load_queue() {
        Ok(jobs) => {
            let effects = {
                let state = app.state::<DownloadsState>();
                lock(&state).loaded(jobs)
            };
            apply(&app, effects);
        }
        Err(err) => say(&app, format!("{err:#}")),
    }
}

/// Does what the queue asked, starts what is due and tells the page.
pub(crate) fn apply(app: &AppHandle, effects: Vec<Effect>) {
    for effect in effects {
        match effect {
            Effect::Say(line) => say(app, line),
            Effect::Save => save(app),
            Effect::Discard(job) => discard(app, job),
            Effect::Rescan => crate::library::rescan(app.clone()),
            Effect::Exit => app.exit(0),
            // The question about closing is the next part of the slice: a
            // job is never left to run under a window that went.
            Effect::ShowDownloads => show_downloads(app),
            Effect::Start(..) => {}
        }
    }
    next(app);
    let _ = app.emit("downloads_changed", ());
}

fn say(app: &AppHandle, line: String) {
    let state = app.state::<DownloadsState>();
    *state.message.lock().unwrap_or_else(PoisonError::into_inner) = Some(line);
}

fn save(app: &AppHandle) {
    let jobs: Vec<Job> = {
        let state = app.state::<DownloadsState>();
        lock(&state).queue.clone()
    };
    if let Err(err) = downloads::save_queue(&jobs) {
        say(app, format!("{err:#}"));
    }
}

/// A cancelled install's files are cleared away on a thread of their own.
fn discard(app: &AppHandle, job: Job) {
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = queue::clear_away(&job);
        if let Effect::Say(line) = Queue::discarded(&job.title, &result) {
            say(&handle, line);
        }
        let _ = handle.emit("downloads_changed", ());
    });
}

/// Starts the first job not paused, unless one runs. A job that changes a
/// game's files waits while the game runs, and goes when it closes.
pub(crate) fn next(app: &AppHandle) {
    let started = {
        let library = app.state::<LibraryState>();
        let running = library.running();
        let state = app.state::<DownloadsState>();
        let mut queue = lock(&state);
        queue.next(|job| queue::changes_files(job.action) && running.is_open(&job.id()))
    };
    if let Some(Effect::Start(job, stop)) = started {
        let handle = app.clone();
        queue::run(job, stop, move |report| reported(&handle, report));
    }
}

/// What a job's thread said. Each progress goes into the queue at once, so
/// the page is never behind what was said; the page looks again for it.
fn reported(app: &AppHandle, report: Report) -> bool {
    match report {
        Report::Event(event) => {
            let effects = {
                let state = app.state::<DownloadsState>();
                lock(&state).event(event)
            };
            if !effects.is_empty() {
                apply(app, effects);
            }
        }
        Report::Ended(result) => {
            let effects = {
                let state = app.state::<DownloadsState>();
                lock(&state).ended(result)
            };
            apply(app, effects);
        }
    }
    true
}

/// Closing was asked while a job runs: the Downloads page comes up with the
/// question (the next part of the slice).
fn show_downloads(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    let _ = app.emit("downloads_asked", ());
}

// The page.

#[derive(Debug, Default, Serialize)]
pub struct CountsDto {
    up_next: String,
    paused: String,
    finished: String,
}

/// A job in a list. Every text is filled here; an empty one is not drawn.
#[derive(Debug, Serialize)]
pub struct JobRowDto {
    pub(crate) id: String,
    pub(crate) title: String,
    /// "Install · Epic".
    pub(crate) line: String,
    /// A dim line: "Waits until the game is closed".
    pub(crate) note: String,
    /// Why it failed, in red.
    pub(crate) error: String,
    /// How a finished job ended.
    pub(crate) verdict: String,
    pub(crate) verdict_bad: bool,
    /// The first button (Pause, Resume, Try again) and Cancel; Open is for
    /// a finished job. An empty label is no button.
    pub(crate) first: String,
    pub(crate) first_primary: bool,
    pub(crate) cancel: String,
    pub(crate) open: String,
}

/// The job the engine runs now.
#[derive(Debug, Serialize)]
pub struct RunningDto {
    id: String,
    title: String,
    line: String,
    /// How far it is, 0 to 100, or -1 when the engine did not say.
    percent: f32,
    progress_line: String,
    pause_label: String,
    cancel_label: String,
    pause_on: bool,
    cancel_on: bool,
}

/// The Downloads page as it stands.
#[derive(Debug, Serialize)]
pub struct DownloadsDto {
    /// Nothing queued and nothing finished: no tiles.
    empty: bool,
    /// Nothing at all going on either: the page invites to the Store.
    invite: bool,
    /// How many jobs wait, the running one included.
    count: usize,
    counts: CountsDto,
    /// Closing was asked for while a job runs.
    asking: bool,
    running: Option<RunningDto>,
    up_next: Vec<JobRowDto>,
    paused: Vec<JobRowDto>,
    finished: Vec<JobRowDto>,
    /// What the queue last said, such as "Installing Beta Racer".
    message: Option<String>,
    /// The running job's line, or how many jobs wait paused, for the foot
    /// of any page.
    summary: Option<String>,
}

fn row(job: &Job) -> JobRowDto {
    JobRowDto {
        id: job.id(),
        title: job.title.clone(),
        line: format!("{} · {}", queue::action_label(job.action), job.shop.label()),
        note: String::new(),
        error: String::new(),
        verdict: String::new(),
        verdict_bad: false,
        first: String::new(),
        first_primary: false,
        cancel: String::new(),
        open: String::new(),
    }
}

pub(crate) fn dto(app: &AppHandle) -> DownloadsDto {
    let state = app.state::<DownloadsState>();
    let queue = lock(&state);
    let library = app.state::<LibraryState>();
    let running_game = library.running();
    let start = usize::from(queue.running.is_some());
    let rest = || queue.queue.iter().skip(start);

    let up_next: Vec<JobRowDto> = rest()
        .filter(|job| !job.paused)
        .map(|job| JobRowDto {
            note: if queue::changes_files(job.action) && running_game.is_open(&job.id()) {
                t!("downloads-waits-running")
            } else {
                String::new()
            },
            first: t!("pause"),
            cancel: t!("cancel"),
            ..row(job)
        })
        .collect();
    let paused: Vec<JobRowDto> = rest()
        .filter(|job| job.paused)
        .map(|job| {
            let error = queue.errors.get(&job.id());
            JobRowDto {
                error: error.map_or_else(String::new, |error| {
                    t!("downloads-failed", error = error.as_str())
                }),
                first: if error.is_some() {
                    t!("try-again")
                } else {
                    t!("resume")
                },
                first_primary: true,
                cancel: t!("cancel"),
                ..row(job)
            }
        })
        .collect();
    let finished: Vec<JobRowDto> = queue
        .finished
        .iter()
        .rev()
        .map(|(job, outcome)| JobRowDto {
            verdict: match outcome {
                Ok(()) => t!("downloads-done"),
                Err(err) => err.clone(),
            },
            verdict_bad: outcome.is_err(),
            open: if outcome.is_ok() && job.action != Action::Uninstall {
                t!("open")
            } else {
                String::new()
            },
            ..row(job)
        })
        .collect();
    drop(running_game);
    let message = state
        .message
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let counts = CountsDto {
        // As in Iced, the running job counts as waiting its turn's end.
        up_next: queue.len().saturating_sub(paused.len()).to_string(),
        paused: paused.len().to_string(),
        finished: finished.len().to_string(),
    };
    DownloadsDto {
        empty: queue.queue.is_empty() && queue.finished.is_empty(),
        invite: queue.queue.is_empty() && queue.finished.is_empty(),
        count: queue.len(),
        counts,
        asking: queue.confirm_close,
        running: running_of(&queue),
        up_next,
        paused,
        finished,
        message,
        summary: queue.summary(),
    }
}

fn running_of(queue: &Queue) -> Option<RunningDto> {
    let running = queue.running.as_ref()?;
    let job = queue.queue.first()?;
    Some(RunningDto {
        id: job.id(),
        title: job.title.clone(),
        line: format!("{} · {}", queue::action_doing(job.action), job.shop.label()),
        percent: running.progress.percent.unwrap_or(-1.0),
        progress_line: running
            .step
            .clone()
            .unwrap_or_else(|| queue::progress_line(job, &running.progress)),
        pause_label: if running.then == Some(Then::Pause) {
            t!("pausing")
        } else {
            t!("pause")
        },
        cancel_label: if running.then == Some(Then::Cancel) {
            t!("cancelling")
        } else {
            t!("cancel")
        },
        pause_on: running.then.is_none(),
        cancel_on: running.then != Some(Then::Cancel),
    })
}

#[tauri::command]
pub fn downloads_get(app: AppHandle) -> DownloadsDto {
    dto(&app)
}

#[tauri::command]
pub fn downloads_pause(id: String, app: AppHandle) -> DownloadsDto {
    let effects = {
        let state = app.state::<DownloadsState>();
        lock(&state).pause(&id)
    };
    apply(&app, effects);
    dto(&app)
}

#[tauri::command]
pub fn downloads_resume(id: String, app: AppHandle) -> DownloadsDto {
    let effects = {
        let state = app.state::<DownloadsState>();
        lock(&state).resume(&id)
    };
    apply(&app, effects);
    dto(&app)
}

#[tauri::command]
pub fn downloads_cancel(id: String, app: AppHandle) -> DownloadsDto {
    let effects = {
        let state = app.state::<DownloadsState>();
        lock(&state).cancel(&id)
    };
    apply(&app, effects);
    dto(&app)
}

#[tauri::command]
pub fn downloads_clear(app: AppHandle) -> DownloadsDto {
    {
        let state = app.state::<DownloadsState>();
        lock(&state).clear_finished();
    }
    let _ = app.emit("downloads_changed", ());
    dto(&app)
}

/// Where a finished job's game is to open: its page in the library when the
/// scan has it, otherwise the Store's.
#[derive(Debug, Serialize)]
pub struct OpenDto {
    /// `game` or `store`.
    kind: &'static str,
    id: String,
}

#[tauri::command]
pub fn downloads_open(id: String, app: AppHandle) -> Option<OpenDto> {
    let library = app.state::<LibraryState>();
    if library.game(&id).is_some() {
        return Some(OpenDto { kind: "game", id });
    }
    let store = app.state::<StoreState>();
    let browser = store.browser.lock().unwrap_or_else(PoisonError::into_inner);
    browser.owned(&id).map(|_| OpenDto { kind: "store", id })
}

// A store game's Install, Import and Resume.

/// Where a store game stands with the queue, for its page's buttons.
#[derive(Debug, Default)]
pub(crate) struct InstallState {
    /// 0 installed, 1 Install and Import, 2 queued, 3 running, 4 paused.
    pub(crate) job: i32,
    /// What the running button says, such as "Installing…".
    pub(crate) label: String,
    /// How far a Steam install is, 0 to 100; -1 for no bar.
    pub(crate) progress: f32,
    /// "Also install its DLC (n)", empty when there is none to give.
    pub(crate) dlc: String,
    pub(crate) dlc_on: bool,
    /// What installing it would do, or why it cannot be.
    pub(crate) note: String,
}

impl InstallState {
    fn plain() -> Self {
        Self {
            job: 0,
            label: String::new(),
            progress: -1.0,
            dlc: String::new(),
            dlc_on: true,
            note: String::new(),
        }
    }
}

/// What the store game's page shows under its title.
pub(crate) fn install_state(app: &AppHandle, owned: &Owned, installed: bool) -> InstallState {
    let mut state = InstallState::plain();
    state.dlc_on = !skip_dlc(app).contains(&owned.id);
    if installed {
        return state;
    }
    let held = {
        let downloads = app.state::<DownloadsState>();
        let queue = lock(&downloads);
        queue
            .job_for(&owned.id)
            .map(|(job, running)| (job.action, job.paused, running, job.path.clone()))
    };
    if let Some((action, paused, running, path)) = held {
        state.job = match (running, paused) {
            (true, _) => 3,
            (false, true) => 4,
            (false, false) => 2,
        };
        if running {
            state.label = queue::action_doing_alone(action);
        }
        state.note = t!("store-into", path = path.display().to_string());
        return state;
    }
    state.note = install_note(app, owned, installed);
    if !can_install(app, owned) {
        return state;
    }
    state.job = 1;
    let dlc = installable_dlc(app, &owned.id);
    state.dlc = dlc_words(&dlc);
    state
}

fn skip_dlc(app: &AppHandle) -> HashSet<String> {
    let state = app.state::<DownloadsState>();
    state
        .skip_dlc
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Whether Zendo can install it now: a store whose engine it drives, into
/// the folder Settings names. Steam's own dialog installs Steam's games
/// (the next part of the slice), and Game Pass needs the launcher's backend
/// and a launcher that is not running.
fn can_install(app: &AppHandle, owned: &Owned) -> bool {
    if owned.shop == Shop::Steam {
        return false;
    }
    if owned.shop == Shop::Xbox && (!zendo_core::xodus::ready() || zendo_core::xodus::running()) {
        return false;
    }
    install_dir(app).is_some()
}

fn install_dir(app: &AppHandle) -> Option<PathBuf> {
    let settings = app.state::<SettingsState>();
    let config = settings.config();
    let dir = config.install_dir();
    drop(config);
    dir
}

/// Says what installing the game would do, as the Slint page does.
fn install_note(app: &AppHandle, owned: &Owned, installed: bool) -> String {
    if installed {
        return String::new();
    }
    if owned.shop == Shop::Xbox {
        let store = app.state::<StoreState>();
        let browser = store.browser.lock().unwrap_or_else(PoisonError::into_inner);
        return zendo_shell::store::xbox_install_note(
            browser.package(&owned.id),
            zendo_core::xodus::ready(),
            zendo_core::xodus::running(),
        );
    }
    if owned.id.starts_with("steam:") {
        return owned.sizes.map_or_else(
            || t!("store-steam-installs"),
            |sizes| {
                t!(
                    "store-steam-installs-needs",
                    size = Size::Bytes(sizes.install).to_string()
                )
            },
        );
    }
    let needs = match info_of(app, &owned.id) {
        Some(InfoState::Got(info)) => info
            .install_size
            .map(|bytes| Size::Bytes(bytes).to_string()),
        _ => None,
    };
    let dir = install_dir(app).map(|dir| dir.display().to_string());
    match (dir, needs) {
        (Some(path), Some(size)) => t!("store-installs-into-needs", path = path, size = size),
        (Some(path), None) => t!("store-installs-into", path = path),
        (None, _) => t!("store-no-home"),
    }
}

fn info_of(app: &AppHandle, id: &str) -> Option<InfoState> {
    let store = app.state::<StoreState>();
    let browser = store.browser.lock().unwrap_or_else(PoisonError::into_inner);
    browser.info.get(id).cloned()
}

/// The DLC the store says the user owns and can install for a game.
fn installable_dlc(app: &AppHandle, id: &str) -> Vec<Dlc> {
    match info_of(app, id) {
        Some(InfoState::Got(info)) => info.dlc.into_iter().filter(|dlc| dlc.installable).collect(),
        _ => Vec::new(),
    }
}

fn dlc_words(dlc: &[Dlc]) -> String {
    let bytes: u64 = dlc.iter().filter_map(|dlc| dlc.install_size).sum();
    if dlc.is_empty() {
        String::new()
    } else if bytes > 0 {
        t!(
            "store-with-dlc-size",
            count = dlc.len(),
            size = Size::Bytes(bytes).to_string()
        )
    } else {
        t!("store-with-dlc", count = dlc.len())
    }
}

/// The job for a store game, if Zendo can install it and it is not queued.
fn install_job(app: &AppHandle, id: &str) -> Option<Job> {
    if lock(&app.state::<DownloadsState>()).job_for(id).is_some() {
        return None;
    }
    let owned = {
        let store = app.state::<StoreState>();
        let browser = store.browser.lock().unwrap_or_else(PoisonError::into_inner);
        browser.owned(id).cloned()?
    };
    if !can_install(app, &owned) {
        return None;
    }
    let dlc = if skip_dlc(app).contains(id) {
        Vec::new()
    } else {
        installable_dlc(app, id)
            .into_iter()
            .map(|dlc| dlc.id)
            .collect()
    };
    Job::install(&owned, install_dir(app)?, dlc)
}

/// Install: queues the game where Settings says, with the DLC unless the box
/// was unticked. The page asks for the game again by itself.
#[tauri::command]
pub fn owned_install(id: String, app: AppHandle) -> Result<crate::store::OwnedDto, String> {
    if let Some(job) = install_job(&app, &id) {
        let effects = {
            let state = app.state::<DownloadsState>();
            lock(&state).enqueue(job)
        };
        apply(&app, effects);
    }
    crate::store::page_of(&app, &id)
}

/// Import: the folder dialog opens; the folder chosen is where the store
/// game already is.
#[tauri::command]
pub fn owned_import(id: String, app: AppHandle) {
    let title = {
        let store = app.state::<StoreState>();
        let browser = store.browser.lock().unwrap_or_else(PoisonError::into_inner);
        browser.owned(&id).map(|owned| owned.title.clone())
    };
    let Some(title) = title else {
        return;
    };
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = file_dialog::pick(
            Kind::Folder,
            &t!("downloads-import-where", title = title.as_str()),
        )
        .map_err(|err| format!("{err:#}"));
        let folder = match result {
            Ok(Some(folder)) => folder,
            Ok(None) => return,
            Err(err) => {
                say(&handle, err);
                let _ = handle.emit("downloads_changed", ());
                return;
            }
        };
        let job = {
            let store = handle.state::<StoreState>();
            let browser = store.browser.lock().unwrap_or_else(PoisonError::into_inner);
            browser
                .owned(&id)
                .and_then(|owned| Job::import(owned, folder))
        };
        if let Some(job) = job {
            let effects = {
                let state = handle.state::<DownloadsState>();
                lock(&state).enqueue(job)
            };
            apply(&handle, effects);
        }
    });
}

/// Resume: the paused job starts again.
#[tauri::command]
pub fn owned_resume(id: String, app: AppHandle) -> Result<crate::store::OwnedDto, String> {
    let effects = {
        let state = app.state::<DownloadsState>();
        lock(&state).resume(&id)
    };
    apply(&app, effects);
    crate::store::page_of(&app, &id)
}

/// "Also install its DLC": ticked or unticked.
#[tauri::command]
pub fn owned_toggle_dlc(id: String, app: AppHandle) -> Result<crate::store::OwnedDto, String> {
    {
        let state = app.state::<DownloadsState>();
        let mut skipped = state
            .skip_dlc
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if !skipped.remove(&id) {
            skipped.insert(id.clone());
        }
    }
    let _ = app.emit("downloads_changed", ());
    crate::store::page_of(&app, &id)
}
