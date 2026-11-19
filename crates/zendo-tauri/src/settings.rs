//! The Settings page: the window's language and theme, the tray, the covers
//! and downloads, the sources found, and Zendo's own files with the report
//! for a bug. A change shows at once and is written on a thread (asking the
//! keyring can wait for the user, and the window must not); one that fails
//! comes back and puts the page as the file has it.
//!
//! The Keys, Launchers, Engines and Proton groups, and Discord's presence,
//! are the next part of this slice.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use zendo_core::config::{Config, Kept};
use zendo_core::covers::{self, KeyCheck};
use zendo_core::nexus::{self as core_nexus, KeyCheck as NexusKeyCheck};
use zendo_core::secrets::Key;
use zendo_core::stores::Store;
use zendo_core::{Library, Source, State as CoreState, launch, report};
use zendo_shell::i18n::{self, Language};
use zendo_shell::nexus::{self as nexus, NexusState, Then};
use zendo_shell::theme::Themes;
use zendo_shell::{clipboard, open, t};

use crate::engines;
use crate::launchers;
use crate::library::LibraryState;
use crate::platform::{self, Shell};
use crate::presence::PresenceState;
use crate::protons;

/// The page a key is fetched from, as the "Get a key" line opens.
const STEAMGRIDDB_KEY_PAGE: &str = "https://www.steamgriddb.com/profile/preferences/api";

/// The download workers Settings offers besides the engine's own choice,
/// which is the computer's processors (legendary at most 16).
const WORKERS: [u32; 4] = [2, 4, 8, 16];

/// A change to the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setting {
    DownloadCovers(bool),
    Workers(Option<u32>),
    Language(Option<String>),
    AutoUpdate(bool),
    Theme(Option<String>),
    MinimizeToTray(bool),
    DiscordPresence(bool),
    SteamGridDbKey {
        key: Option<String>,
        then: KeyThen,
    },
    NexusKey {
        key: Option<String>,
        then: Then,
    },
    EnginesPathFirst(bool),
    DefaultProton(Option<String>),
    GogPlaytime(bool),
    GogGalaxy(bool),
    /// Zendo's Steam sign-in, as the keyring keeps it (`Session::to_text`).
    SteamSession(Option<String>),
}

impl Setting {
    fn apply(&self, config: &mut Config) {
        match self {
            Self::DownloadCovers(on) => config.download_covers = *on,
            Self::Workers(workers) => config.download_workers = *workers,
            Self::Language(language) => config.language.clone_from(language),
            Self::AutoUpdate(on) => config.auto_update = *on,
            Self::Theme(theme) => config.theme.clone_from(theme),
            Self::MinimizeToTray(on) => config.minimize_to_tray = *on,
            Self::DiscordPresence(on) => config.discord_presence = *on,
            Self::SteamGridDbKey { key, .. } => config.steamgriddb_key.clone_from(key),
            Self::NexusKey { key, .. } => config.nexus_key.clone_from(key),
            Self::EnginesPathFirst(on) => config.engines_on_path_first = *on,
            Self::DefaultProton(name) => config.default_proton.clone_from(name),
            Self::GogPlaytime(on) => config.gog_playtime = *on,
            Self::GogGalaxy(on) => config.gog_galaxy = *on,
            Self::SteamSession(session) => config.steam_session.clone_from(session),
        }
    }

    /// What the page does once a change is in the file.
    fn after(&self, app: &AppHandle) {
        match self {
            Self::Language(_) => language_changed(app),
            Self::Theme(_) => theme_changed(app),
            Self::MinimizeToTray(on) => platform::set_tray(app, *on),
            Self::DiscordPresence(on) => PresenceState::set(app, *on),
            Self::SteamGridDbKey { then, .. } => {
                let settings = app.state::<SettingsState>();
                match then {
                    KeyThen::Removed => settings.key_say(Some((t!("key-removed"), true))),
                    KeyThen::Saved => {
                        settings.key_say(Some((t!("steamgriddb-saved"), true)));
                        crate::library::download_covers_now(app);
                    }
                    KeyThen::Unchecked(error) => {
                        settings.key_say(Some((
                            t!("key-saved-unchecked", error = error.as_str()),
                            false,
                        )));
                    }
                }
            }
            Self::NexusKey { then, .. } => {
                let settings = app.state::<SettingsState>();
                settings.nexus_key_stored(then);
            }
            Self::DownloadCovers(true) => crate::library::download_covers_now(app),
            Self::EnginesPathFirst(_) => crate::engines::refresh_engines(app),
            Self::DefaultProton(_) => crate::protons::default_proton_set(app),
            Self::AutoUpdate(true) => crate::downloads::update_by_themselves(app),
            Self::DownloadCovers(false)
            | Self::Workers(_)
            | Self::AutoUpdate(false)
            | Self::GogPlaytime(_)
            | Self::GogGalaxy(_)
            | Self::SteamSession(_) => {}
        }
    }
}

/// What the page says once the `SteamGridDB` key is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyThen {
    Removed,
    /// Saved after `SteamGridDB` accepted it.
    Saved,
    /// Saved although the check could not reach `SteamGridDB`.
    Unchecked(String),
}

/// The `SteamGridDB` key's field and what it last said.
#[derive(Default)]
struct KeyField {
    value: String,
    shown: bool,
    /// The key being checked, so an answer for another is let go.
    checking: Option<String>,
    save_after_check: bool,
    /// What happened, and whether it went well.
    note: Option<(String, bool)>,
}

/// The page's settings as they stand, and the one writer that keeps the
/// changes in the order they were made.
pub struct SettingsState {
    config: Mutex<Config>,
    saves: Mutex<Option<Sender<Setting>>>,
    /// The report's line, and whether it is being gathered.
    note: Mutex<Option<(String, i32)>>,
    gathering: AtomicBool,
    /// The `SteamGridDB` key's field and what it last said.
    key: Mutex<KeyField>,
    /// The Nexus key's field, whose key it is and what opens links.
    nexus: Mutex<NexusState>,
    /// Nexus's key and handler were asked once, as the page first opened.
    nexus_ready: AtomicBool,
}

impl SettingsState {
    pub fn new() -> Self {
        Self {
            config: Mutex::new(Config::load_file().unwrap_or_default()),
            saves: Mutex::default(),
            note: Mutex::default(),
            gathering: AtomicBool::new(false),
            key: Mutex::default(),
            nexus: Mutex::default(),
            nexus_ready: AtomicBool::new(false),
        }
    }

    pub(crate) fn config(&self) -> MutexGuard<'_, Config> {
        self.config.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn note(&self) -> Option<(String, i32)> {
        self.note
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn say(&self, note: Option<(String, i32)>) {
        *self.note.lock().unwrap_or_else(PoisonError::into_inner) = note;
    }

    fn key(&self) -> MutexGuard<'_, KeyField> {
        self.key.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn key_say(&self, note: Option<(String, bool)>) {
        self.key().note = note;
    }

    fn nexus(&self) -> MutexGuard<'_, NexusState> {
        self.nexus.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The key is in the file: the page says what became of it.
    fn nexus_key_stored(&self, then: &Then) {
        self.nexus().key_stored(then.clone());
    }
}

impl Default for SettingsState {
    fn default() -> Self {
        Self::new()
    }
}

/// A language the window speaks: `None` is the desktop's.
#[derive(Debug, Serialize)]
pub struct LanguageChip {
    tag: Option<String>,
    label: String,
    chosen: bool,
}

/// A theme: a file's id, or `system` for the KDE scheme.
#[derive(Debug, Serialize)]
pub struct ThemeChip {
    tag: String,
    label: String,
    chosen: bool,
}

/// A number of files to download at once: `None` is the engine's own choice.
#[derive(Debug, Serialize)]
pub struct WorkerChip {
    count: Option<u32>,
    label: String,
    chosen: bool,
}

/// A line of a list: a source and its count, or a file and where it is.
#[derive(Debug, Serialize)]
pub struct RowDto {
    pub(crate) label: String,
    pub(crate) value: String,
    /// 0 plain, 1 good, 2 bad, 3 dim.
    pub(crate) tone: i32,
}

/// One of Zendo's own files, and the folder to open.
#[derive(Debug, Serialize)]
pub struct FileDto {
    label: String,
    value: String,
    open: Option<String>,
}

/// A thing Settings lists with buttons: an engine, a runtime, a Proton, a
/// release or a launcher. An empty text is not drawn.
#[derive(Debug, Default, Serialize)]
pub struct SetRowDto {
    pub(crate) title: String,
    pub(crate) sub: String,
    pub(crate) path: String,
    pub(crate) note: String,
    pub(crate) note_tone: i32,
    pub(crate) action: String,
    pub(crate) action_on: bool,
    pub(crate) remove: String,
    pub(crate) remove_on: bool,
    pub(crate) remove_danger: bool,
    pub(crate) status: String,
    pub(crate) status_tone: i32,
}

/// A download in progress: its name, the bytes so far and how far it is.
#[derive(Debug, Default, Serialize)]
pub struct BarDto {
    active: bool,
    name: String,
    size: String,
    percent: f32,
    stopping: bool,
}

/// A download's bar, from the bytes so far.
pub(crate) fn bar_of(name: &str, done: u64, total: u64, stopping: bool) -> BarDto {
    BarDto {
        active: true,
        name: t!("protons-downloading", name = name),
        size: t!(
            "size-of",
            done = zendo_core::Size::Bytes(done).to_string(),
            total = zendo_core::Size::Bytes(total).to_string()
        ),
        #[expect(clippy::cast_precision_loss, reason = "a bar's width")]
        percent: if total == 0 {
            0.0
        } else {
            done as f32 * 100.0 / total as f32
        },
        stopping,
    }
}

/// The `SteamGridDB` key's field, with where the key is kept.
#[derive(Debug, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one thing the field draws, as the Slint struct has them"
)]
pub struct KeyDto {
    value: String,
    shown: bool,
    checking: bool,
    /// The field holds another key than the one saved.
    changed: bool,
    note: Option<RowDto>,
    kept: String,
    kept_bad: bool,
}

/// The Nexus key's field, with whose key it is and what opens links.
#[derive(Debug, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one thing the field draws, as the Slint struct has them"
)]
pub struct NexusKeyDto {
    value: String,
    shown: bool,
    checking: bool,
    changed: bool,
    note: Option<RowDto>,
    kept: String,
    kept_bad: bool,
    handler_line: String,
    handler_good: bool,
    handler_can: bool,
}

/// Everything the page's General, Library and Diagnostics groups show.
#[derive(Debug, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one switch the page draws, as the Slint struct has them"
)]
pub struct SettingsDto {
    languages: Vec<LanguageChip>,
    themes: Vec<ThemeChip>,
    theme_about: String,
    theme_note: Option<RowDto>,
    theme_problems: Vec<String>,
    tray: bool,
    discord: bool,
    steamgriddb: KeyDto,
    nexus: NexusKeyDto,
    covers: bool,
    /// This computer can download now: the setting is on and nothing runs.
    covers_now: bool,
    auto_update: bool,
    workers: Vec<WorkerChip>,
    sources: Vec<RowDto>,
    launchers: Vec<SetRowDto>,
    launchers_bar: BarDto,
    path_first: bool,
    engines: Vec<SetRowDto>,
    engines_bar: BarDto,
    engines_note: Option<RowDto>,
    runtimes: Vec<SetRowDto>,
    runtimes_note: Option<RowDto>,
    overlay: engines::OverlayDto,
    proton: protons::ProtonsDto,
    files: Vec<FileDto>,
    report_busy: bool,
    report_note: Option<RowDto>,
    logs: Option<String>,
}

/// The page as it stands.
#[tauri::command]
pub fn settings_get(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    nexus_ready(&app);
    launchers::ready(&app);
    engines::ready(&app);
    protons::ready(&app);
    let settings = app.state::<SettingsState>();
    let config = settings.config();
    let shell = app.state::<Shell>();
    let themes = shell.themes.lock().unwrap_or_else(PoisonError::into_inner);
    SettingsDto {
        languages: languages(&config),
        themes: theme_chips(&themes),
        theme_about: theme_about(),
        theme_note: themes.note().map(|(note, good)| RowDto {
            label: String::new(),
            value: note,
            tone: if good { 3 } else { 2 },
        }),
        theme_problems: themes
            .problems()
            .iter()
            .map(|problem| t!("theme-not-read", problem = problem.as_str()))
            .collect(),
        tray: config.minimize_to_tray,
        discord: config.discord_presence,
        steamgriddb: steamgriddb_dto(&settings, &config),
        nexus: nexus_dto(&settings, &config),
        covers: config.download_covers,
        covers_now: config.download_covers && !library.downloading(),
        auto_update: config.auto_update,
        workers: workers(&config),
        sources: sources(&library),
        launchers: launchers::rows(&app),
        launchers_bar: launchers::bar(&app),
        path_first: config.engines_on_path_first,
        engines: engines::engines_rows(&app),
        engines_bar: engines::engines_bar(&app),
        engines_note: engines::engines_note(&app),
        runtimes: engines::runtimes_rows(&app),
        runtimes_note: engines::runtimes_note(&app),
        overlay: engines::overlay_dto(&app),
        proton: protons::protons_dto(&app, &config),
        files: files(&config),
        report_busy: settings.gathering.load(Ordering::Relaxed),
        report_note: settings.note().map(|(note, tone)| RowDto {
            label: String::new(),
            value: note,
            tone,
        }),
        logs: launch::logs_dir()
            .filter(|dir| dir.is_dir())
            .map(|dir| dir.display().to_string()),
    }
}

/// The window's language: `None` follows the desktop's.
#[tauri::command]
pub fn settings_set_language(
    language: Option<String>,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    save(&app, Setting::Language(language));
    settings_get(app, library)
}

/// The window's colours: a theme file's id, `system`, or `None` for Zendo's.
#[tauri::command]
pub fn settings_set_theme(
    theme: Option<String>,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    save(&app, Setting::Theme(theme));
    settings_get(app, library)
}

#[tauri::command]
pub fn settings_set_tray(
    on: bool,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    save(&app, Setting::MinimizeToTray(on));
    settings_get(app, library)
}

#[tauri::command]
pub fn settings_set_discord(
    on: bool,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    save(&app, Setting::DiscordPresence(on));
    settings_get(app, library)
}

/// Typing in the `SteamGridDB` field: the field, the buttons and the note.
#[tauri::command]
pub fn settings_key_edited(
    text: String,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    {
        let settings = app.state::<SettingsState>();
        let mut key = settings.key();
        key.value = text;
        key.note = None;
    }
    settings_get(app, library)
}

#[tauri::command]
pub fn settings_key_toggle_shown(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    {
        let settings = app.state::<SettingsState>();
        let mut key = settings.key();
        key.shown = !key.shown;
    }
    settings_get(app, library)
}

/// Checks the key in the field, without writing it.
#[tauri::command]
pub fn settings_key_check(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    let key = {
        let settings = app.state::<SettingsState>();
        let mut field = settings.key();
        field.save_after_check = false;
        let key = field.value.trim().to_owned();
        if key.is_empty() {
            field.note = Some((t!("key-paste-first"), false));
            None
        } else {
            field.checking = Some(key.clone());
            field.note = None;
            Some(key)
        }
    };
    if let Some(key) = key {
        check_steamgriddb_key(app.clone(), key);
    }
    settings_get(app, library)
}

/// Saves the key: an empty field removes it, any other is checked first.
#[tauri::command]
pub fn settings_key_save(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    let key = {
        let settings = app.state::<SettingsState>();
        let mut field = settings.key();
        let key = field.value.trim().to_owned();
        field.value.clone_from(&key);
        if key.is_empty() {
            field.checking = None;
            field.save_after_check = false;
        } else {
            field.save_after_check = true;
        }
        key
    };
    if key.is_empty() {
        save(
            &app,
            Setting::SteamGridDbKey {
                key: None,
                then: KeyThen::Removed,
            },
        );
    } else {
        check_steamgriddb_key(app.clone(), key);
    }
    settings_get(app, library)
}

/// Opens the page a `SteamGridDB` key comes from.
#[tauri::command]
pub fn settings_key_get() -> Result<(), String> {
    open::target(STEAMGRIDDB_KEY_PAGE)
}

/// Typing in the Nexus field.
#[tauri::command]
pub fn settings_nexus_key_edited(
    text: String,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    app.state::<SettingsState>().nexus().edit_key(text);
    settings_get(app, library)
}

#[tauri::command]
pub fn settings_nexus_key_toggle_shown(
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    app.state::<SettingsState>().nexus().toggle_shown();
    settings_get(app, library)
}

/// Checks the Nexus key in the field, without writing it.
#[tauri::command]
pub fn settings_nexus_key_check(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    let step = app.state::<SettingsState>().nexus().check_key(true);
    nexus_key_step(&app, step);
    settings_get(app, library)
}

/// Saves the Nexus key: an empty field removes it, any other is checked first.
#[tauri::command]
pub fn settings_nexus_key_save(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    let step = app.state::<SettingsState>().nexus().save_key(true);
    nexus_key_step(&app, step);
    settings_get(app, library)
}

/// Opens the page a Nexus key comes from.
#[tauri::command]
pub fn settings_nexus_key_get() -> Result<(), String> {
    open::target(core_nexus::KEY_PAGE)
}

/// Makes this Zendo open `nxm://` links.
#[tauri::command]
pub fn settings_nexus_become_handler(
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = nexus::become_handler();
        let settings = handle.state::<SettingsState>();
        let note = settings.nexus().became_handler(result);
        settings.say(Some((note, 1)));
        let _ = handle.emit("settings_changed", ());
    });
    settings_get(app, library)
}

#[tauri::command]
pub fn settings_set_covers(
    on: bool,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    save(&app, Setting::DownloadCovers(on));
    settings_get(app, library)
}

#[tauri::command]
pub fn settings_set_auto_update(
    on: bool,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    save(&app, Setting::AutoUpdate(on));
    settings_get(app, library)
}

/// The workers: `None` is the engine's own choice.
#[tauri::command]
pub fn settings_set_workers(
    workers: Option<u32>,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    save(&app, Setting::Workers(workers));
    settings_get(app, library)
}

/// Whether a program on `PATH` wins over Zendo's own copy of an engine.
#[tauri::command]
pub fn settings_set_path_first(
    on: bool,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    save(&app, Setting::EnginesPathFirst(on));
    settings_get(app, library)
}

/// The default Proton: `None` is Automatic.
#[tauri::command]
pub fn settings_default_chosen(
    name: Option<String>,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> SettingsDto {
    save(&app, Setting::DefaultProton(name));
    settings_get(app, library)
}

/// Fetches the covers the library is missing, as Download now asks.
#[tauri::command]
pub fn settings_download_covers(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    crate::library::download_covers_now(&app);
    settings_get(app, library)
}

/// Gathers the report and puts it in the clipboard, on a thread.
#[tauri::command]
pub fn settings_copy_report(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
    let settings = app.state::<SettingsState>();
    if settings.gathering.swap(true, Ordering::Relaxed) {
        return settings_get(app, library);
    }
    settings.say(None);
    let config = settings.config().clone();
    let games = library.games();
    let handle = app.clone();
    std::thread::spawn(move || {
        let library = Library {
            games,
            problems: Vec::new(),
        };
        let text = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            report::gather(&config, &library)
        }))
        .unwrap_or_else(|_| "Zendo hit a bug gathering the report".to_owned());
        let note = match clipboard::copy(&text) {
            Ok(()) => (t!("report-gathered", count = text.lines().count()), 1),
            Err(err) => (err, 2),
        };
        let settings = handle.state::<SettingsState>();
        settings.gathering.store(false, Ordering::Relaxed);
        settings.say(Some(note));
        let _ = handle.emit("settings_changed", ());
    });
    settings_get(app, library)
}

/// Opens a folder with the desktop's file manager.
#[tauri::command]
pub fn settings_open(path: String) -> Result<(), String> {
    open::target(&path)
}

/// Shows a change at once, then writes it on the one writer's thread.
pub(crate) fn save(app: &AppHandle, setting: Setting) {
    let settings = app.state::<SettingsState>();
    setting.apply(&mut settings.config());
    let sender = settings
        .saves
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_with(|| {
            let (sender, receiver) = channel::<Setting>();
            let handle = app.clone();
            std::thread::spawn(move || {
                while let Ok(writing) = receiver.recv() {
                    let result = Config::update(|config| writing.apply(config))
                        .map_err(|err| format!("{err:#}"));
                    let settings = handle.state::<SettingsState>();
                    match result {
                        Ok(config) => {
                            *settings.config() = config;
                            writing.after(&handle);
                        }
                        Err(err) => {
                            if let Ok(config) = Config::load_file() {
                                *settings.config() = config;
                            }
                            settings.say(Some((err, 2)));
                        }
                    }
                    let _ = handle.emit("settings_changed", ());
                }
            });
            sender
        })
        .clone();
    let _ = sender.send(setting);
}

/// The words follow the file, and the page hears the new ones.
fn language_changed(app: &AppHandle) {
    let config = app.state::<SettingsState>().config().clone();
    i18n::set(Language::chosen(config.language.as_deref()));
    let _ = app.emit(
        "language_changed",
        LanguageDto {
            language: i18n::current().tag().to_owned(),
            messages: i18n::current_catalogue().into_iter().collect(),
        },
    );
}

/// The colours follow the file, and the page hears them.
fn theme_changed(app: &AppHandle) {
    let config = app.state::<SettingsState>().config().clone();
    let shell = app.state::<Shell>();
    let Ok(mut themes) = shell.themes.lock() else {
        return;
    };
    if !themes.is_chosen(config.theme.as_deref()) {
        themes.set_tag(config.theme.as_deref());
    }
    let _ = app.emit("theme_changed", crate::commands::palette(&themes.palette()));
}

/// The new words, for the frontend's own store.
#[derive(Debug, Clone, Serialize)]
pub struct LanguageDto {
    language: String,
    messages: std::collections::BTreeMap<String, String>,
}

/// The languages: the desktop's first, then each Zendo speaks.
fn languages(config: &Config) -> Vec<LanguageChip> {
    let chosen = config.language.as_deref().and_then(Language::from_tag);
    let mut chips = vec![LanguageChip {
        tag: None,
        label: t!("language-system", language = Language::system().name()),
        chosen: chosen.is_none(),
    }];
    chips.extend(Language::ALL.iter().map(|language| LanguageChip {
        tag: Some(language.tag().to_owned()),
        label: language.name().to_owned(),
        chosen: chosen == Some(*language),
    }));
    chips
}

/// The theme files, and the KDE scheme where there is one.
fn theme_chips(themes: &Themes) -> Vec<ThemeChip> {
    let mut chips: Vec<ThemeChip> = themes
        .files()
        .into_iter()
        .map(|(id, name)| ThemeChip {
            chosen: *themes.choice() == zendo_shell::theme::Choice::File(id.clone()),
            tag: id,
            label: name,
        })
        .collect();
    let system = *themes.choice() == zendo_shell::theme::Choice::System;
    if system || zendo_core::kde::Scheme::found() {
        chips.push(ThemeChip {
            tag: zendo_shell::theme::Choice::System.tag().to_owned(),
            label: t!("theme-system"),
            chosen: system,
        });
    }
    chips
}

fn theme_about() -> String {
    let folder =
        zendo_core::theme::Theme::dir().map_or_else(String::new, |dir| dir.display().to_string());
    t!("theme-about", folder = folder)
}

fn workers(config: &Config) -> Vec<WorkerChip> {
    let chosen = config.download_workers.filter(|workers| *workers > 0);
    let mut chips = vec![WorkerChip {
        count: None,
        label: t!("workers-engine"),
        chosen: chosen.is_none(),
    }];
    chips.extend(WORKERS.iter().map(|count| WorkerChip {
        count: Some(*count),
        label: count.to_string(),
        chosen: chosen == Some(*count),
    }));
    chips
}

/// Each source and how many games it gave, then what could not be read.
fn sources(library: &LibraryState) -> Vec<RowDto> {
    let scanning = library.scanning();
    let mut rows: Vec<RowDto> = library
        .sources()
        .into_iter()
        .map(|(source, count)| RowDto {
            label: source_label(source),
            value: if scanning {
                t!("library-scanning")
            } else if count == 0 {
                t!("sources-none-found")
            } else {
                t!("count-games", count = count)
            },
            tone: if count == 0 || scanning { 3 } else { 0 },
        })
        .collect();
    rows.extend(library.problems().iter().map(|problem| RowDto {
        label: String::new(),
        value: t!("sources-unreadable", problem = problem.as_str()),
        tone: 2,
    }));
    rows
}

fn source_label(source: Source) -> String {
    match source {
        Source::Steam => "Steam".to_owned(),
        Source::Epic => t!("sources-epic"),
        Source::Gog => t!("sources-gog"),
        Source::Amazon => t!("sources-amazon"),
        Source::Xbox => t!("sources-xbox"),
        Source::Heroic => t!("sources-heroic"),
        Source::Desktop => t!("sources-desktop"),
        Source::Local => t!("sources-local"),
    }
}

/// Zendo's own files, with the folder each is in.
fn files(config: &Config) -> Vec<FileDto> {
    let store = Store::Epic
        .dir()
        .and_then(|dir| dir.parent().map(Path::to_path_buf));
    let rows: [(String, Option<PathBuf>); 6] = [
        (t!("files-settings"), Config::path()),
        (t!("files-state"), CoreState::path()),
        (t!("files-images"), covers::dir()),
        (
            t!("files-queue"),
            CoreState::path().map(|state| state.with_file_name("downloads.json")),
        ),
        (t!("files-sign-ins"), store),
        (t!("files-games"), config.install_dir()),
    ];
    rows.into_iter()
        .map(|(label, path)| FileDto {
            label,
            value: path
                .as_ref()
                .map_or_else(|| t!("files-no-home"), |path| path.display().to_string()),
            open: path.filter(|path| path.is_dir()).and_then(existing),
        })
        .collect()
}

/// The folder itself when it is there, else its nearest parent that is.
fn existing(path: PathBuf) -> Option<String> {
    let mut here = path.as_path();
    loop {
        if here.is_dir() {
            return Some(here.display().to_string());
        }
        here = here.parent()?;
    }
}

// The keys.

/// The `SteamGridDB` key's field, with where the key is kept.
fn steamgriddb_dto(settings: &SettingsState, config: &Config) -> KeyDto {
    let key = settings.key();
    let saved = config.steamgriddb_key.as_deref().unwrap_or("");
    let (kept, kept_bad) = kept_text(config.kept(Key::SteamGridDb));
    KeyDto {
        value: key.value.clone(),
        shown: key.shown,
        checking: key.checking.is_some(),
        changed: key.value.trim() != saved,
        note: key_note(&key, saved),
        kept,
        kept_bad,
    }
}

/// The Nexus key's field, with whose key it is and what opens links.
fn nexus_dto(settings: &SettingsState, config: &Config) -> NexusKeyDto {
    let nexus = settings.nexus();
    let saved = config.nexus_key.as_deref().unwrap_or("");
    let (kept, kept_bad) = kept_text(config.kept(Key::Nexus));
    let (handler_line, handler_good, handler_can) = nexus.handler_row();
    NexusKeyDto {
        value: nexus.key.value.clone(),
        shown: nexus.key.shown,
        checking: nexus.key.checking.is_some(),
        changed: nexus.key.value.trim() != saved,
        note: nexus_note(&nexus, saved),
        kept,
        kept_bad,
        handler_line,
        handler_good,
        handler_can,
    }
}

/// The `SteamGridDB` field's note: none when a key is saved and nothing was
/// said.
fn key_note(key: &KeyField, saved: &str) -> Option<RowDto> {
    match &key.note {
        Some((note, good)) => Some(note_row(note.clone(), *good)),
        None if saved.is_empty() => Some(RowDto {
            label: String::new(),
            value: t!("key-none"),
            tone: 3,
        }),
        None => None,
    }
}

/// The Nexus field's note, which also says whose the stored key is.
fn nexus_note(nexus: &NexusState, saved: &str) -> Option<RowDto> {
    match (&nexus.key.note, &nexus.user) {
        (Some((note, good)), _) => Some(note_row(note.clone(), *good)),
        (None, _) if saved.is_empty() => Some(RowDto {
            label: String::new(),
            value: t!("key-none"),
            tone: 3,
        }),
        (None, Some(user)) => Some(RowDto {
            label: String::new(),
            value: nexus::signed_in(user),
            tone: 1,
        }),
        (None, None) => None,
    }
}

/// A note line: 1 good, 2 bad.
fn note_row(value: String, good: bool) -> RowDto {
    RowDto {
        label: String::new(),
        value,
        tone: if good { 1 } else { 2 },
    }
}

/// Where a secret is kept, in words, and whether that is a problem.
pub(crate) fn kept_text(kept: &Kept) -> (String, bool) {
    match kept {
        Kept::Nowhere => (String::new(), false),
        Kept::Keyring => (t!("kept-keyring"), false),
        Kept::File(None) => (t!("kept-file"), false),
        Kept::File(Some(why)) => (t!("kept-file-why", why = why.as_str()), false),
        Kept::Unread(why) => (t!("kept-unread", why = why.as_str()), true),
    }
}

/// Asks `SteamGridDB` about the key, on a thread, and says what came of it.
fn check_steamgriddb_key(app: AppHandle, key: String) {
    std::thread::spawn(move || {
        let result = covers::check_key(&key).map_err(|err| format!("{err:#}"));
        let settings = app.state::<SettingsState>();
        let save_after = {
            let mut field = settings.key();
            if field.checking.as_deref() != Some(key.as_str()) {
                return;
            }
            field.checking = None;
            std::mem::take(&mut field.save_after_check)
        };
        let note = match result {
            Ok(KeyCheck::Accepted) if save_after => {
                save(
                    &app,
                    Setting::SteamGridDbKey {
                        key: Some(key),
                        then: KeyThen::Saved,
                    },
                );
                return;
            }
            Ok(KeyCheck::Accepted) => Some((t!("steamgriddb-accepted"), true)),
            Ok(KeyCheck::Rejected) if save_after => {
                Some((t!("steamgriddb-rejected-unsaved"), false))
            }
            Ok(KeyCheck::Rejected) => Some((t!("steamgriddb-rejected"), false)),
            Err(err) if save_after => {
                save(
                    &app,
                    Setting::SteamGridDbKey {
                        key: Some(key),
                        then: KeyThen::Unchecked(err),
                    },
                );
                return;
            }
            Err(err) => Some((err, false)),
        };
        settings.key_say(note);
        let _ = app.emit("settings_changed", ());
    });
}

/// The Nexus key and handler, asked once as the page first opens.
fn nexus_ready(app: &AppHandle) {
    let settings = app.state::<SettingsState>();
    if settings.nexus_ready.swap(true, Ordering::Relaxed) {
        return;
    }
    let stored = settings.config().nexus_key.clone();
    settings.nexus().loaded(stored.as_deref());
    // What opens `nxm://` links, and whose the stored key is.
    let handle = app.clone();
    std::thread::spawn(move || {
        let handler = core_nexus::handler();
        handle
            .state::<SettingsState>()
            .nexus()
            .handler_known(handler);
        let _ = handle.emit("settings_changed", ());
    });
    if let Some(key) = stored {
        let handle = app.clone();
        std::thread::spawn(move || {
            let result = nexus::check_key(&key, true);
            nexus_key_checked(&handle, &key, result);
        });
    }
}

/// What a press on Check or Save leaves to do.
fn nexus_key_step(app: &AppHandle, step: nexus::KeyStep) {
    match step {
        nexus::KeyStep::Nothing => {}
        nexus::KeyStep::Remove => save(
            app,
            Setting::NexusKey {
                key: None,
                then: Then::Removed,
            },
        ),
        nexus::KeyStep::Check(key) => {
            check_nexus_key(app.clone(), key);
        }
    }
}

/// Asks Nexus about the key, on a thread.
fn check_nexus_key(app: AppHandle, key: String) {
    std::thread::spawn(move || {
        let result = nexus::check_key(&key, false);
        nexus_key_checked(&app, &key, result);
    });
}

/// Nexus answered about a key.
fn nexus_key_checked(app: &AppHandle, key: &str, result: Result<NexusKeyCheck, String>) {
    let settings = app.state::<SettingsState>();
    let stored = settings.config().nexus_key.clone();
    let checked = settings.nexus().key_checked(key, result, stored.as_deref());
    match checked {
        nexus::Checked::Save(key, then) => save(
            app,
            Setting::NexusKey {
                key: Some(key),
                then,
            },
        ),
        nexus::Checked::Nothing => {
            let _ = app.emit("settings_changed", ());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setting_changes_only_its_own_field() {
        let mut config = Config::default();
        Setting::MinimizeToTray(true).apply(&mut config);
        Setting::Workers(Some(4)).apply(&mut config);
        Setting::Language(Some("pt-PT".into())).apply(&mut config);
        assert!(config.minimize_to_tray);
        assert_eq!(config.download_workers, Some(4));
        assert_eq!(config.language.as_deref(), Some("pt-PT"));
        assert!(config.theme.is_none(), "Theme was not among the changes");
    }

    #[test]
    fn the_workers_offer_the_engines_own_choice_and_each_count() {
        let config = Config::default();
        let chips = workers(&config);
        assert_eq!(chips.len(), WORKERS.len() + 1);
        assert!(chips[0].chosen, "the engine's own choice is the default");
        let mut config = Config::default();
        config.download_workers = Some(8);
        let chips = workers(&config);
        let chosen: Vec<&str> = chips
            .iter()
            .filter(|chip| chip.chosen)
            .map(|chip| chip.label.as_str())
            .collect();
        assert_eq!(chosen, ["8"]);
    }
}
