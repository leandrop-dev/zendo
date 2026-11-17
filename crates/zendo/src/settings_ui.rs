//! The Settings page: the window's language and tray, the `SteamGridDB` key,
//! the covers and downloads, the sources found and Zendo's own files, with
//! the report for a bug. A change shows at once and is written on a thread
//! (asking the keyring can wait for the user, and the window must not); one
//! that fails comes back and puts the page as the file has it. A part of the
//! Library's glue (`library_ui`), which owns the config, the games and the
//! covers.

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::config::{Config, Kept};
use zendo_core::covers::{self, KeyCheck};
use zendo_core::secrets::Key;
use zendo_core::{Source, State, launch, report};
use zendo_shell::i18n::Language;
use zendo_shell::presence::PresenceHandle;
use zendo_shell::t;

use super::{Glue, with_glue};
use crate::{ChipData, Fact, MainWindow, SettingsData};

const KEY_PAGE: &str = "https://www.steamgriddb.com/profile/preferences/api";

/// The download workers Settings offers besides the engine's own choice,
/// which is the computer's processors (legendary at most 16).
const WORKERS: [u32; 4] = [2, 4, 8, 16];

/// The groups Settings shows one at a time. Not saved: the page opens on
/// General.
const GENERAL: i32 = 0;
const KEYS: i32 = 1;
const LIBRARY: i32 = 2;
const SOURCES: i32 = 3;
const ENGINES: i32 = 4;
const PROTON: i32 = 5;
const DIAGNOSTICS: i32 = 6;

/// A change to the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Setting {
    DownloadCovers(bool),
    Workers(Option<u32>),
    Language(Option<String>),
    AutoUpdate(bool),
    Theme(Option<String>),
    SteamGridDbKey {
        key: Option<String>,
        then: KeyThen,
    },
    DefaultProton(Option<String>),
    MinimizeToTray(bool),
    DiscordPresence(bool),
    EnginesPathFirst(bool),
    Opaque(bool),
    /// Whether the sessions of GOG games go to GOG.
    GogPlaytime(bool),
    /// Whether GOG games start with GOG Galaxy.
    GogGalaxy(bool),
    /// Zendo's Steam sign-in, as the keyring keeps it (`Session::to_text`).
    SteamSession(Option<String>),
    /// The Nexus Mods key; `then` is what the page says once it is written.
    /// Whether Nexus's adult mods are searched.
    NexusAdult(bool),
    NexusKey {
        key: Option<String>,
        then: zendo_shell::nexus::Then,
    },
}

/// What the page says once the `SteamGridDB` key is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum KeyThen {
    Removed,
    /// Saved after `SteamGridDB` accepted it.
    Saved,
    /// Saved although the check could not reach `SteamGridDB`.
    Unchecked(String),
}

impl Setting {
    fn apply(&self, config: &mut Config) {
        match self {
            Self::DownloadCovers(on) => config.download_covers = *on,
            Self::Workers(workers) => config.download_workers = *workers,
            Self::Language(language) => config.language.clone_from(language),
            Self::AutoUpdate(on) => config.auto_update = *on,
            Self::Theme(theme) => config.theme.clone_from(theme),
            Self::SteamGridDbKey { key, .. } => config.steamgriddb_key.clone_from(key),
            Self::DefaultProton(name) => config.default_proton.clone_from(name),
            Self::MinimizeToTray(on) => config.minimize_to_tray = *on,
            Self::DiscordPresence(on) => config.discord_presence = *on,
            Self::EnginesPathFirst(on) => config.engines_on_path_first = *on,
            Self::Opaque(on) => config.opaque = *on,
            Self::GogPlaytime(on) => config.gog_playtime = *on,
            Self::GogGalaxy(on) => config.gog_galaxy = *on,
            Self::SteamSession(session) => config.steam_session.clone_from(session),
            Self::NexusKey { key, .. } => config.nexus_key.clone_from(key),
            Self::NexusAdult(on) => config.nexus_adult = *on,
        }
    }
}

/// Where a secret is kept, in words, and whether that is a problem.
pub(super) fn kept_text(kept: &Kept) -> (String, bool) {
    match kept {
        Kept::Nowhere => (String::new(), false),
        Kept::Keyring => (t!("kept-keyring"), false),
        Kept::File(None) => (t!("kept-file"), false),
        Kept::File(Some(why)) => (t!("kept-file-why", why = why.as_str()), false),
        Kept::Unread(why) => (t!("kept-unread", why = why.as_str()), true),
    }
}

/// The `SteamGridDB` key's field and what it last said.
#[derive(Default)]
struct KeyState {
    value: String,
    shown: bool,
    /// The key being checked, so an answer for another is let go.
    checking: Option<String>,
    save_after_check: bool,
    /// What happened, and whether it went well.
    note: Option<(String, bool)>,
}

/// The report: gathering, and what became of the last.
#[derive(Default)]
struct Report {
    gathering: bool,
    note: Option<(String, bool)>,
}

/// What Settings holds that the config does not.
pub(super) struct SettingsState {
    group: Cell<i32>,
    key: RefCell<KeyState>,
    report: RefCell<Report>,
    /// Where the logs and Zendo's folders are, found as the page opens: the
    /// page must not stat the disk each time it is drawn.
    logs: RefCell<Option<PathBuf>>,
    folders: RefCell<Vec<Option<PathBuf>>>,
    groups: Rc<VecModel<ChipData>>,
    languages: Rc<VecModel<ChipData>>,
    themes: Rc<VecModel<ChipData>>,
    /// What `config.toml`'s `theme` says for each theme chip, by its id.
    theme_tags: RefCell<Vec<String>>,
    theme_problems: Rc<VecModel<slint::SharedString>>,
    workers: Rc<VecModel<ChipData>>,
    sources: Rc<VecModel<Fact>>,
    files: Rc<VecModel<Fact>>,
}

impl SettingsState {
    pub(super) fn new() -> Self {
        Self {
            group: Cell::new(GENERAL),
            key: RefCell::default(),
            report: RefCell::default(),
            logs: RefCell::default(),
            folders: RefCell::default(),
            groups: Rc::new(VecModel::default()),
            languages: Rc::new(VecModel::default()),
            themes: Rc::new(VecModel::default()),
            theme_tags: RefCell::default(),
            theme_problems: Rc::new(VecModel::default()),
            workers: Rc::new(VecModel::default()),
            sources: Rc::new(VecModel::default()),
            files: Rc::new(VecModel::default()),
        }
    }
}

/// The tone of a note a card says: 1 good, 2 bad.
pub(super) fn note_tone(good: bool) -> i32 {
    if good { 1 } else { 2 }
}

/// A download's bar: its name, the bytes so far and how far it is.
#[expect(clippy::cast_precision_loss, reason = "a bar's width")]
pub(super) fn bar_of(name: &str, done: u64, total: u64, stopping: bool) -> crate::Bar {
    crate::Bar {
        active: true,
        name: name.into(),
        size: t!(
            "size-of",
            done = zendo_core::Size::Bytes(done).to_string(),
            total = zendo_core::Size::Bytes(total).to_string()
        )
        .into(),
        percent: if total == 0 {
            0.0
        } else {
            done as f32 * 100.0 / total as f32
        },
        stopping,
    }
}

pub(super) fn chip(id: i32, label: String, chosen: bool) -> ChipData {
    ChipData {
        id,
        label: label.into(),
        count: "".into(),
        chosen,
    }
}

fn row(label: impl Into<String>, value: impl Into<String>, tone: i32) -> Fact {
    Fact {
        label: label.into().into(),
        value: value.into().into(),
        path: false,
        mono: false,
        url: "".into(),
        tone,
    }
}

/// The folder a file lives in, or the folder itself, when it is there.
fn folder(path: Option<PathBuf>) -> Option<PathBuf> {
    let path = path?;
    let folder = if path.extension().is_some() {
        path.parent().unwrap_or(&path).to_path_buf()
    } else {
        path
    };
    folder.is_dir().then_some(folder)
}

/// Puts `text` on the clipboard through the tool the session has.
fn copy_text(text: &str) -> Result<(), String> {
    for (program, args) in [
        ("wl-copy", &[][..]),
        ("xclip", &["-selection", "clipboard"][..]),
    ] {
        let Ok(mut child) = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            continue;
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if child.wait().is_ok_and(|status| status.success()) {
            return Ok(());
        }
    }
    Err(t!("report-no-clipboard"))
}

impl Glue {
    /// Hooks the page's callbacks up and gives it its models.
    pub(super) fn connect_settings(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<SettingsData>();
        data.set_groups(ModelRc::from(Rc::clone(&self.settings.groups)));
        data.set_languages(ModelRc::from(Rc::clone(&self.settings.languages)));
        data.set_workers(ModelRc::from(Rc::clone(&self.settings.workers)));
        data.set_sources(ModelRc::from(Rc::clone(&self.settings.sources)));
        data.set_files(ModelRc::from(Rc::clone(&self.settings.files)));

        let glue = Rc::clone(self);
        data.on_group_chosen(move |group| {
            glue.settings.group.set(group);
            glue.show_settings();
        });
        let glue = Rc::clone(self);
        data.on_language_chosen(move |index| {
            let language = usize::try_from(index)
                .ok()
                .and_then(|index| Language::ALL.get(index).copied());
            glue.save_setting(Setting::Language(
                language.map(|language| language.tag().to_owned()),
            ));
        });
        let glue = Rc::clone(self);
        data.on_set_tray(move |on| glue.save_setting(Setting::MinimizeToTray(on)));
        let glue = Rc::clone(self);
        data.on_set_opaque(move |on| glue.save_setting(Setting::Opaque(on)));
        let glue = Rc::clone(self);
        data.on_set_discord(move |on| glue.save_setting(Setting::DiscordPresence(on)));
        let glue = Rc::clone(self);
        data.on_set_path_first(move |on| glue.save_setting(Setting::EnginesPathFirst(on)));
        let glue = Rc::clone(self);
        data.on_theme_chosen(move |index| {
            let tag = usize::try_from(index)
                .ok()
                .and_then(|index| glue.settings.theme_tags.borrow().get(index).cloned());
            if let Some(tag) = tag {
                glue.save_setting(Setting::Theme(Some(tag)));
            }
        });
        data.set_themes(ModelRc::from(Rc::clone(&self.settings.themes)));
        data.set_theme_problems(ModelRc::from(Rc::clone(&self.settings.theme_problems)));
        self.connect_engines(window);
        self.connect_runtimes(window);
        self.connect_overlay(window);
        self.connect_protons(window);
        let glue = Rc::clone(self);
        data.on_set_covers(move |on| glue.save_setting(Setting::DownloadCovers(on)));
        let glue = Rc::clone(self);
        data.on_set_auto_update(move |on| glue.save_setting(Setting::AutoUpdate(on)));
        let glue = Rc::clone(self);
        data.on_workers_chosen(move |workers| {
            glue.save_setting(Setting::Workers(
                u32::try_from(workers).ok().filter(|n| *n > 0),
            ));
        });
        let glue = Rc::clone(self);
        data.on_covers_download(move || {
            glue.download_images();
            if glue.persist && glue.images_left.get() == 0 {
                // Nothing was missing, so nothing started.
                glue.say(t!("images-none"));
            }
            glue.show_settings();
        });
        let glue = Rc::clone(self);
        data.on_key_edited(move |typed| {
            {
                let mut key = glue.settings.key.borrow_mut();
                key.value = typed.to_string();
                key.note = None;
            }
            glue.show_settings();
        });
        let glue = Rc::clone(self);
        data.on_key_toggle_shown(move || {
            {
                let mut key = glue.settings.key.borrow_mut();
                key.shown = !key.shown;
            }
            glue.show_settings();
        });
        let glue = Rc::clone(self);
        data.on_key_check(move || {
            glue.settings.key.borrow_mut().save_after_check = false;
            glue.check_key();
        });
        let glue = Rc::clone(self);
        data.on_key_save(move || glue.save_key());
        let glue = Rc::clone(self);
        data.on_key_get(move || glue.open_target(KEY_PAGE));
        let glue = Rc::clone(self);
        data.on_open(move |target| glue.open_target(&target));
        let glue = Rc::clone(self);
        data.on_open_logs(move || {
            let logs = glue.settings.logs.borrow().clone();
            if let Some(dir) = logs {
                glue.open_target(&dir.to_string_lossy());
            }
        });
        let glue = Rc::clone(self);
        data.on_copy_report(move || glue.copy_report());
        let glue = Rc::clone(self);
        data.on_relabel(move || glue.show_settings());
    }

    /// The page opens on the Keys group, where the art picker sends the user.
    pub(super) fn show_keys_group(&self) {
        self.settings.group.set(KEYS);
    }

    /// The page opens: looks for the folders it can open, and shows itself.
    pub(super) fn open_settings(&self) {
        let config = self.config.borrow();
        let install = config.as_ref().and_then(Config::install_dir);
        *self.settings.logs.borrow_mut() = launch::logs_dir().filter(|dir| dir.is_dir());
        *self.settings.folders.borrow_mut() = vec![
            folder(Config::path()),
            folder(State::path()),
            folder(covers::dir()),
            folder(State::path().map(|state| state.with_file_name("downloads.json"))),
            folder(
                zendo_core::stores::Store::Epic
                    .dir()
                    .and_then(|dir| dir.parent().map(Path::to_path_buf)),
            ),
            folder(install),
        ];
        drop(config);
        // A theme file added since shows, and what the page lists is looked
        // at again; GitHub and Lutris are asked once a session.
        self.themes
            .borrow_mut()
            .reload(zendo_core::theme::Theme::dir().as_deref());
        self.want_overlay();
        self.want_engines();
        self.want_protons();
        self.want_runtimes();
        self.want_sources();
        self.show_settings();
    }

    /// The config as the page shows it: the file's, or the defaults while it
    /// has not been read.
    pub(super) fn settings_config(&self) -> Config {
        self.config.borrow().clone().unwrap_or_default()
    }

    // Writing a change.

    /// Applies a change to the window at once and writes it on a thread. The
    /// page goes on as if it worked; one that fails comes back and puts the
    /// settings back.
    pub(super) fn save_setting(self: &Rc<Self>, setting: Setting) {
        let previous = self.config.borrow().clone();
        setting.apply(self.config.borrow_mut().get_or_insert_with(Config::default));
        self.config_changed();
        if !self.persist {
            self.after_saved(&setting);
            return;
        }
        let written = setting.clone();
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result =
                Config::update(|config| written.apply(config)).map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.config_saved(previous, &setting, result));
            });
        });
    }

    /// A write came back: the file's settings when it worked, or what went
    /// wrong with the window back as the file has it.
    pub(super) fn config_saved(
        self: &Rc<Self>,
        previous: Option<Config>,
        setting: &Setting,
        result: Result<Config, String>,
    ) {
        match result {
            Ok(config) => {
                *self.config.borrow_mut() = Some(config);
                self.config_changed();
                self.after_saved(setting);
            }
            Err(err) => {
                *self.config.borrow_mut() = previous;
                self.config_changed();
                self.say(err);
            }
        }
    }

    /// The window follows the settings: its words, its paint, the page.
    fn config_changed(&self) {
        let config = self.settings_config();
        if let Some(window) = self.window.upgrade() {
            crate::tr::show(&window, Language::chosen(config.language.as_deref()));
            window.set_translucency(if config.opaque { 1.0 } else { 2.0 / 3.0 });
            // The colours follow the file, so a write that failed puts them
            // back too.
            let palette = {
                let mut themes = self.themes.borrow_mut();
                if !themes.is_chosen(config.theme.as_deref()) {
                    themes.set_tag(config.theme.as_deref());
                }
                themes.palette()
            };
            crate::window::paint(&window, palette);
        }
        self.show_settings();
        // The GOG switches and the count beside Accounts read the settings.
        self.show_accounts();
    }

    /// What the page does once a change is in the file.
    fn after_saved(self: &Rc<Self>, setting: &Setting) {
        match setting {
            Setting::DownloadCovers(true) => self.download_images(),
            Setting::SteamGridDbKey { then, .. } => match then {
                KeyThen::Removed => {
                    self.settings.key.borrow_mut().note = Some((t!("key-removed"), true));
                }
                KeyThen::Saved => {
                    self.settings.key.borrow_mut().note = Some((t!("steamgriddb-saved"), true));
                    self.download_images();
                }
                KeyThen::Unchecked(error) => {
                    self.settings.key.borrow_mut().note =
                        Some((t!("key-saved-unchecked", error = error.as_str()), false));
                }
            },
            Setting::NexusKey { then, .. } => self.nexus_key_stored(then),
            Setting::MinimizeToTray(on) => {
                if self.persist
                    && let Some(window) = self.window.upgrade()
                {
                    crate::window::set_tray(&window, *on);
                }
            }
            Setting::DiscordPresence(on) => {
                *self.presence.borrow_mut() = (*on && self.persist).then(PresenceHandle::spawn);
                self.update_presence();
            }
            Setting::DefaultProton(_) => self.default_proton_set(),
            Setting::EnginesPathFirst(_) => self.refresh_engines(),
            // Steam's list shows, or leaves the Store, with the sign-in.
            Setting::SteamSession(_) => self.store_accounts_changed(),
            // Turning it on queues the updates already found.
            Setting::AutoUpdate(_) => self.update_by_themselves(),
            Setting::DownloadCovers(false)
            | Setting::Workers(_)
            | Setting::Language(_)
            | Setting::Theme(_)
            | Setting::Opaque(_)
            | Setting::GogPlaytime(_)
            | Setting::GogGalaxy(_)
            | Setting::NexusAdult(_) => {}
        }
        self.show_settings();
    }

    // The key.

    fn save_key(self: &Rc<Self>) {
        let key = {
            let mut state = self.settings.key.borrow_mut();
            let key = state.value.trim().to_owned();
            state.value.clone_from(&key);
            if key.is_empty() {
                // A check still in flight is about a key that is gone.
                state.checking = None;
                state.save_after_check = false;
            } else {
                // Kept once SteamGridDB has answered, so a mistyped key is
                // never saved.
                state.save_after_check = true;
            }
            key
        };
        if key.is_empty() {
            self.save_setting(Setting::SteamGridDbKey {
                key: None,
                then: KeyThen::Removed,
            });
        } else {
            self.check_key();
        }
    }

    fn check_key(self: &Rc<Self>) {
        let key = self.settings.key.borrow().value.trim().to_owned();
        if key.is_empty() {
            self.settings.key.borrow_mut().note = Some((t!("key-paste-first"), false));
            self.show_settings();
            return;
        }
        {
            let mut state = self.settings.key.borrow_mut();
            state.checking = Some(key.clone());
            state.note = None;
        }
        self.show_settings();
        if !self.persist {
            return;
        }
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let asked = key.clone();
            let result = covers::check_key(&key).map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.key_checked(asked, result));
            });
        });
    }

    pub(super) fn key_checked(self: &Rc<Self>, key: String, result: Result<KeyCheck, String>) {
        // An answer is about the key it checked: one that arrived after the
        // field moved on says nothing about what is in it now.
        let save = {
            let mut state = self.settings.key.borrow_mut();
            if state.checking.as_deref() != Some(key.as_str()) {
                return;
            }
            state.checking = None;
            std::mem::take(&mut state.save_after_check)
        };
        match result {
            Ok(KeyCheck::Accepted) if save => self.save_setting(Setting::SteamGridDbKey {
                key: Some(key),
                then: KeyThen::Saved,
            }),
            Ok(KeyCheck::Accepted) => {
                self.settings.key.borrow_mut().note = Some((t!("steamgriddb-accepted"), true));
            }
            Ok(KeyCheck::Rejected) => {
                let note = if save {
                    t!("steamgriddb-rejected-unsaved")
                } else {
                    t!("steamgriddb-rejected")
                };
                self.settings.key.borrow_mut().note = Some((note, false));
            }
            // Offline is no reason to lose what the user pasted.
            Err(err) if save => self.save_setting(Setting::SteamGridDbKey {
                key: Some(key),
                then: KeyThen::Unchecked(err),
            }),
            Err(err) => self.settings.key.borrow_mut().note = Some((err, false)),
        }
        self.show_settings();
    }

    // The report.

    fn copy_report(self: &Rc<Self>) {
        {
            let mut report = self.settings.report.borrow_mut();
            if report.gathering {
                return;
            }
            report.gathering = true;
            report.note = None;
        }
        self.show_settings();
        if !self.persist {
            return;
        }
        let config = self.settings_config();
        let library = zendo_core::Library {
            games: self.browser.borrow().games().to_vec(),
            problems: Vec::new(),
        };
        self.ask(
            move || Ok(report::gather(&config, &library)),
            Self::report_ready,
        );
    }

    pub(super) fn report_ready(&self, result: Result<String, String>) {
        let note = match result {
            Ok(text) => match copy_text(&text) {
                Ok(()) => (t!("report-gathered", count = text.lines().count()), true),
                Err(err) => (err, false),
            },
            Err(err) => (err, false),
        };
        {
            let mut report = self.settings.report.borrow_mut();
            report.gathering = false;
            report.note = Some(note);
        }
        self.show_settings();
    }

    // What the page shows.

    /// Writes everything the page shows.
    pub(super) fn show_settings(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<SettingsData>();
        let config = self.settings_config();
        let group = self.settings.group.get();

        self.settings.groups.set_vec(
            [
                (GENERAL, t!("settings-group-general")),
                (KEYS, t!("settings-group-keys")),
                (LIBRARY, t!("settings-group-library")),
                (SOURCES, t!("settings-group-launchers")),
                (ENGINES, t!("settings-group-engines")),
                (PROTON, t!("settings-group-proton")),
                (DIAGNOSTICS, t!("settings-group-diagnostics")),
            ]
            .into_iter()
            .map(|(id, label)| chip(id, label, id == group))
            .collect::<Vec<_>>(),
        );
        data.set_group(group);

        // General: index 0 of the languages is the system's.
        let chosen = config.language.as_deref().and_then(Language::from_tag);
        let mut languages = vec![chip(
            -1,
            t!("language-system", language = Language::system().name()),
            chosen.is_none(),
        )];
        for (index, language) in Language::ALL.iter().enumerate() {
            languages.push(chip(
                i32::try_from(index).unwrap_or(0),
                language.name().to_owned(),
                chosen == Some(*language),
            ));
        }
        self.settings.languages.set_vec(languages);
        data.set_tray(config.minimize_to_tray);
        data.set_opaque(config.opaque);
        data.set_discord(config.discord_presence);
        self.show_themes(&data);

        // Keys.
        let key = self.settings.key.borrow();
        let saved = config.steamgriddb_key.as_deref().unwrap_or("");
        if data.get_key_value() != key.value.as_str() {
            data.set_key_value(key.value.as_str().into());
        }
        data.set_key_shown(key.shown);
        data.set_key_checking(key.checking.is_some());
        data.set_key_changed(key.value.trim() != saved);
        let (note, tone) = match &key.note {
            Some((note, good)) => (note.clone(), if *good { 1 } else { 2 }),
            None if saved.is_empty() => (t!("key-none"), 3),
            None => (String::new(), 0),
        };
        data.set_key_note(note.into());
        data.set_key_tone(tone);
        let (kept, bad) = kept_text(config.kept(Key::SteamGridDb));
        data.set_key_kept(kept.into());
        data.set_key_kept_bad(bad);
        drop(key);
        self.show_nexus_settings();
        self.show_library_and_files(&data, &config);
        self.show_engines(&data, &config);
        self.show_runtimes(&data);
        self.show_overlay(&data);
        self.show_protons(&data, &config);
    }

    /// The Theme card: a chip for each theme file and for the KDE scheme
    /// where there is one, and what the window shows.
    fn show_themes(&self, data: &SettingsData<'_>) {
        use crate::theme::Choice;
        let themes = self.themes.borrow();
        let mut chips = Vec::new();
        let mut tags = Vec::new();
        for (id, name) in themes.files() {
            let chosen = *themes.choice() == Choice::File(id.clone());
            chips.push(chip(i32::try_from(tags.len()).unwrap_or(0), name, chosen));
            tags.push(id);
        }
        // System's only where there is a `kdeglobals`, or when already
        // chosen: elsewhere there is no scheme to follow.
        let system = *themes.choice() == Choice::System;
        if system || zendo_core::kde::Scheme::found() {
            chips.push(chip(
                i32::try_from(tags.len()).unwrap_or(0),
                t!("theme-system"),
                system,
            ));
            tags.push(Choice::System.tag().to_owned());
        }
        self.settings.themes.set_vec(chips);
        *self.settings.theme_tags.borrow_mut() = tags;
        let (note, tone) = match themes.note() {
            Some((note, true)) => (note, 3),
            Some((note, false)) => (note, 2),
            None => (String::new(), 0),
        };
        data.set_theme_note(note.into());
        data.set_theme_note_tone(tone);
        self.settings.theme_problems.set_vec(
            themes
                .problems()
                .iter()
                .map(|problem| t!("theme-not-read", problem = problem.as_str()).into())
                .collect::<Vec<slint::SharedString>>(),
        );
        let folder = zendo_core::theme::Theme::dir()
            .map_or_else(String::new, |dir| dir.display().to_string());
        data.set_theme_about(t!("theme-about", folder = folder).into());
    }

    /// The Library and Diagnostics groups' texts.
    fn show_library_and_files(&self, data: &SettingsData<'_>, config: &Config) {
        // Library.
        let left = self.images_left.get();
        data.set_covers(config.download_covers);
        data.set_covers_progress(if left > 0 {
            t!("images-left", count = left).into()
        } else {
            "".into()
        });
        data.set_covers_now(config.download_covers && left == 0);
        data.set_auto_update(config.auto_update);
        let workers = config.download_workers.filter(|workers| *workers > 0);
        let mut choices = vec![chip(0, t!("workers-engine"), workers.is_none())];
        for count in WORKERS {
            choices.push(chip(
                i32::try_from(count).unwrap_or(0),
                count.to_string(),
                workers == Some(count),
            ));
        }
        self.settings.workers.set_vec(choices);
        self.settings.sources.set_vec(self.sources_of());

        // Diagnostics.
        let folders = self.settings.folders.borrow();
        let files: Vec<Fact> = [
            (t!("files-settings"), Config::path()),
            (t!("files-state"), State::path()),
            (t!("files-images"), covers::dir()),
            (
                t!("files-queue"),
                State::path().map(|state| state.with_file_name("downloads.json")),
            ),
            (
                t!("files-sign-ins"),
                zendo_core::stores::Store::Epic
                    .dir()
                    .and_then(|dir| dir.parent().map(Path::to_path_buf)),
            ),
            (t!("files-games"), config.install_dir()),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (label, path))| {
            let open = folders.get(index).cloned().flatten();
            Fact {
                path: open.is_some(),
                url: open
                    .map_or_default(|folder| folder.to_string_lossy().into_owned())
                    .into(),
                ..row(
                    label,
                    path.map_or_else(|| t!("files-no-home"), |path| path.display().to_string()),
                    0,
                )
            }
        })
        .collect();
        self.settings.files.set_vec(files);
        let report = self.settings.report.borrow();
        data.set_report_busy(report.gathering);
        let (note, good) = report.note.clone().unwrap_or_default();
        data.set_report_note(note.into());
        data.set_report_tone(if good { 1 } else { 2 });
        data.set_logs(self.settings.logs.borrow().is_some());
        self.show_sources();
    }

    /// How many games each source gave, and what could not be read.
    fn sources_of(&self) -> Vec<Fact> {
        let browser = self.browser.borrow();
        let scanning = self.scanning.get();
        let mut rows: Vec<Fact> = [
            (Source::Steam, "Steam".to_owned()),
            (Source::Epic, t!("sources-epic")),
            (Source::Gog, t!("sources-gog")),
            (Source::Amazon, t!("sources-amazon")),
            (Source::Xbox, t!("sources-xbox")),
            (Source::Heroic, t!("sources-heroic")),
            (Source::Desktop, t!("sources-desktop")),
            (Source::Local, t!("sources-local")),
        ]
        .into_iter()
        .map(|(source, label)| {
            let count = browser
                .games()
                .iter()
                .filter(|game| game.source == source)
                .count();
            if scanning {
                // A first scan still running is not "none found".
                row(label, t!("library-scanning"), 3)
            } else if count == 0 {
                row(label, t!("sources-none-found"), 3)
            } else {
                row(label, t!("count-games", count = count), 0)
            }
        })
        .collect();
        // One line for each source the scan could not read, as in Iced.
        rows.extend(
            browser
                .problems()
                .iter()
                .map(|problem| row("", t!("sources-unreadable", problem = problem.as_str()), 2)),
        );
        rows
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
        assert!(!config.opaque);
    }
}
