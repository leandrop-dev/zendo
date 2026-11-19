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
use zendo_core::config::Config;
use zendo_core::covers;
use zendo_core::stores::Store;
use zendo_core::{Library, Source, State as CoreState, launch, report};
use zendo_shell::i18n::{self, Language};
use zendo_shell::theme::Themes;
use zendo_shell::{clipboard, open, t};

use crate::library::LibraryState;
use crate::platform::{self, Shell};
use crate::presence::PresenceState;

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
        }
    }

    /// What the page does once a change is in the file.
    fn after(&self, app: &AppHandle) {
        match self {
            Self::Language(_) => language_changed(app),
            Self::Theme(_) => theme_changed(app),
            Self::MinimizeToTray(on) => platform::set_tray(app, *on),
            Self::DiscordPresence(on) => PresenceState::set(app, *on),
            Self::DownloadCovers(true) => crate::library::download_covers_now(app),
            Self::DownloadCovers(false) | Self::Workers(_) | Self::AutoUpdate(_) => {}
        }
    }
}

/// The page's settings as they stand, and the one writer that keeps the
/// changes in the order they were made.
pub struct SettingsState {
    config: Mutex<Config>,
    saves: Mutex<Option<Sender<Setting>>>,
    /// The report's line, and whether it is being gathered.
    note: Mutex<Option<(String, i32)>>,
    gathering: AtomicBool,
}

impl SettingsState {
    pub fn new() -> Self {
        Self {
            config: Mutex::new(Config::load_file().unwrap_or_default()),
            saves: Mutex::default(),
            note: Mutex::default(),
            gathering: AtomicBool::new(false),
        }
    }

    fn config(&self) -> MutexGuard<'_, Config> {
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
    label: String,
    value: String,
    /// 0 plain, 2 bad, 3 dim.
    tone: i32,
}

/// One of Zendo's own files, and the folder to open.
#[derive(Debug, Serialize)]
pub struct FileDto {
    label: String,
    value: String,
    open: Option<String>,
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
    covers: bool,
    /// This computer can download now: the setting is on and nothing runs.
    covers_now: bool,
    auto_update: bool,
    workers: Vec<WorkerChip>,
    sources: Vec<RowDto>,
    files: Vec<FileDto>,
    report_busy: bool,
    report_note: Option<RowDto>,
    logs: Option<String>,
}

/// The page as it stands.
#[tauri::command]
pub fn settings_get(app: AppHandle, library: State<'_, LibraryState>) -> SettingsDto {
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
        covers: config.download_covers,
        covers_now: config.download_covers && !library.downloading(),
        auto_update: config.auto_update,
        workers: workers(&config),
        sources: sources(&library),
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
fn save(app: &AppHandle, setting: Setting) {
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
