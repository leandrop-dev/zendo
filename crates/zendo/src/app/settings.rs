//! The Settings page: the window's language, the `SteamGridDB` key, the
//! Nexus Mods key (in `nexus.rs`), covers, Proton (in `protons.rs`), the
//! sources found and Zendo's own files.

use std::path::{Path, PathBuf};

use iced::widget::{
    button, column, container, operation, row, scrollable, text, text_input, toggler,
};
use iced::{Alignment, Element, Font, Length, Task};
use zendo_core::config::{Config, Kept};
use zendo_core::covers::{self, KeyCheck};
use zendo_core::nexus::User;
use zendo_core::secrets::Key;
use zendo_core::{Source, State, launch, report};

use super::downloads::UpdateState;
use super::style::{self, BOLD, colors};
use super::themes::Choice;
use super::{Message, Zendo, blocking, key_id};
use crate::i18n::{self, Language, t};

const KEY_PAGE: &str = "https://www.steamgriddb.com/profile/preferences/api";

/// The download workers Settings offers besides the engine's own choice,
/// which is the computer's processors (legendary at most 16).
const WORKERS: [u32; 4] = [2, 4, 8, 16];

/// A change to the settings, written on a thread: asking the keyring can
/// wait for the user, and the window must not.
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
    NexusKey {
        key: Option<String>,
        then: NexusThen,
    },
    GogPlaytime(bool),
    GogGalaxy(bool),
    NexusAdult(bool),
    DefaultProton(Option<String>),
}

/// What the Settings page says once the `SteamGridDB` key is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum KeyThen {
    Removed,
    /// Saved after `SteamGridDB` accepted it.
    Saved,
    /// Saved although the check could not reach `SteamGridDB`.
    Unchecked(String),
}

/// What the Settings page says once the Nexus key is written, and whose
/// key it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum NexusThen {
    Removed,
    /// Saved after Nexus accepted it, as this account.
    Saved(String, User),
    /// Saved although the check could not reach Nexus.
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
            Self::NexusKey { key, .. } => config.nexus_key.clone_from(key),
            Self::GogPlaytime(on) => config.gog_playtime = *on,
            Self::GogGalaxy(on) => config.gog_galaxy = *on,
            Self::NexusAdult(on) => config.nexus_adult = *on,
            Self::DefaultProton(name) => config.default_proton.clone_from(name),
        }
    }
}

impl Zendo {
    /// Finds the paths Settings offers, once as it opens: `view` must not
    /// stat the disk.
    pub(super) fn settings_paths(&mut self) {
        let folder = |path: Option<PathBuf>| -> Option<PathBuf> {
            let path = path?;
            let folder = if path.extension().is_some() {
                path.parent().unwrap_or(&path).to_path_buf()
            } else {
                path
            };
            folder.is_dir().then_some(folder)
        };
        self.diagnostics.logs = launch::logs_dir().filter(|dir| dir.is_dir());
        self.diagnostics.folders = vec![
            folder(Config::path()),
            folder(State::path()),
            folder(covers::dir()),
            folder(State::path().map(|state| state.with_file_name("downloads.json"))),
            folder(
                zendo_core::stores::Store::Epic
                    .dir()
                    .and_then(|dir| dir.parent().map(Path::to_path_buf)),
            ),
            folder(self.config.install_dir()),
        ];
    }

    pub(super) fn update_settings(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::KeyInput(value) => {
                self.key.value = value;
                self.key.note = None;
                Task::none()
            }
            Message::ToggleKeyShown => {
                self.key.shown = !self.key.shown;
                Task::none()
            }
            Message::SaveKey => {
                let key = self.key.value.trim().to_owned();
                self.key.value.clone_from(&key);
                if key.is_empty() {
                    // A check still in flight is about a key that is gone.
                    self.key.checking = None;
                    self.key.save_after_check = false;
                    return self.save_setting(Setting::SteamGridDbKey {
                        key: None,
                        then: KeyThen::Removed,
                    });
                }
                // Kept once SteamGridDB has answered, so a mistyped key is
                // never saved.
                self.key.save_after_check = true;
                self.check_key()
            }
            Message::CheckKey => {
                self.key.save_after_check = false;
                self.check_key()
            }
            Message::KeyChecked(checked, result) => self.key_checked(checked, result),
            Message::ConfigSaved(previous, setting, result) => {
                self.config_saved(previous, setting, result)
            }
            Message::SetDownloadCovers(on) => self.save_setting(Setting::DownloadCovers(on)),
            Message::SetAutoUpdate(on) => self.save_setting(Setting::AutoUpdate(on)),
            Message::SetWorkers(workers) => self.save_setting(Setting::Workers(workers)),
            Message::CopyReport => {
                if self.diagnostics.gathering {
                    return Task::none();
                }
                self.diagnostics.gathering = true;
                self.diagnostics.note = None;
                let config = self.config.clone();
                let library = self.library.clone().unwrap_or_default();
                Task::perform(
                    blocking(move || Ok(report::gather(&config, &library))),
                    Message::ReportReady,
                )
            }
            Message::ReportReady(result) => {
                self.diagnostics.gathering = false;
                match result {
                    Ok(text) => {
                        let lines = text.lines().count();
                        // iced's clipboard write has no result to wait for,
                        // so the note says what is true: the report was made.
                        self.diagnostics.note = Some((t!("report-gathered", count = lines), true));
                        if self.persist {
                            iced::clipboard::write(text)
                        } else {
                            Task::none()
                        }
                    }
                    Err(err) => {
                        self.diagnostics.note = Some((err, false));
                        Task::none()
                    }
                }
            }
            Message::SetLanguage(language) => {
                let task = self.save_setting(Setting::Language(
                    language.map(|language| language.tag().to_owned()),
                ));
                // The page follows at once, not after the file is written.
                i18n::set(Language::chosen(self.config.language.as_deref()));
                task
            }
            Message::DownloadNow => {
                let task = self.download_images();
                if self.images.left == 0 {
                    self.status = Some(t!("images-none"));
                }
                task
            }
            _ => Task::none(),
        }
    }

    /// Text typed on the page with no field focused: it starts the key, as
    /// typing in the library starts a search.
    pub(super) fn type_key(&mut self, typed: &str) -> Task<Message> {
        if !self.key.value.is_empty() {
            return Task::none();
        }
        self.key.value.push_str(typed);
        self.key.note = None;
        Task::batch([
            operation::focus(key_id()),
            operation::move_cursor_to_end(key_id()),
        ])
    }

    fn key_checked(&mut self, key: String, result: Result<KeyCheck, String>) -> Task<Message> {
        // An answer is about the key it checked: one that arrived after the
        // field moved on says nothing about what is in it now.
        if self.key.checking.as_deref() != Some(key.as_str()) {
            return Task::none();
        }
        self.key.checking = None;
        let save = std::mem::take(&mut self.key.save_after_check);
        match result {
            Ok(KeyCheck::Accepted) if save => self.save_setting(Setting::SteamGridDbKey {
                key: Some(key),
                then: KeyThen::Saved,
            }),
            Ok(KeyCheck::Accepted) => {
                self.key.note = Some((t!("steamgriddb-accepted"), true));
                Task::none()
            }
            Ok(KeyCheck::Rejected) => {
                let note = if save {
                    t!("steamgriddb-rejected-unsaved")
                } else {
                    t!("steamgriddb-rejected")
                };
                self.key.note = Some((note, false));
                Task::none()
            }
            // Offline is no reason to lose what the user pasted.
            Err(err) if save => self.save_setting(Setting::SteamGridDbKey {
                key: Some(key),
                then: KeyThen::Unchecked(err),
            }),
            Err(err) => {
                self.key.note = Some((err, false));
                Task::none()
            }
        }
    }

    /// Applies a change to the window at once and writes it on a thread.
    /// The page goes on as if it worked; one that fails comes back and
    /// puts the settings, the language and the colours back.
    pub(super) fn save_setting(&mut self, setting: Setting) -> Task<Message> {
        let previous = self.config.clone();
        setting.apply(&mut self.config);
        if !self.persist {
            return self.after_saved(setting);
        }
        let written = setting.clone();
        Task::perform(
            blocking(move || {
                Config::update(|config| written.apply(config))
                    .map(Box::new)
                    .map_err(|err| format!("{err:#}"))
            }),
            move |result| Message::ConfigSaved(Box::new(previous), setting, result),
        )
    }

    /// A write came back: the file's settings when it worked, or what went
    /// wrong with the window back as the file has it.
    fn config_saved(
        &mut self,
        previous: Box<Config>,
        setting: Setting,
        result: Result<Box<Config>, String>,
    ) -> Task<Message> {
        match result {
            Ok(config) => {
                self.config = *config;
                self.config_changed();
                self.after_saved(setting)
            }
            Err(err) => {
                self.config = *previous;
                self.config_changed();
                self.status = Some(err);
                Task::none()
            }
        }
    }

    /// The window's language and colours follow the settings again.
    fn config_changed(&mut self) {
        i18n::set(Language::chosen(self.config.language.as_deref()));
        self.themes.choice = Choice::chosen(self.config.theme.as_deref());
        self.themes.apply();
    }

    /// What the page does once a change is in the file.
    fn after_saved(&mut self, setting: Setting) -> Task<Message> {
        match setting {
            Setting::DownloadCovers(true) => self.download_images(),
            Setting::AutoUpdate(_) => {
                let found: Vec<String> = self
                    .downloads
                    .updates
                    .iter()
                    .filter(|(_, state)| matches!(state, UpdateState::Available(_)))
                    .map(|(id, _)| id.clone())
                    .collect();
                self.update_by_themselves(&found)
            }
            Setting::SteamGridDbKey { then, .. } => match then {
                KeyThen::Removed => {
                    self.key.note = Some((t!("key-removed"), true));
                    Task::none()
                }
                KeyThen::Saved => {
                    self.key.note = Some((t!("steamgriddb-saved"), true));
                    self.download_images()
                }
                KeyThen::Unchecked(error) => {
                    self.key.note = Some((t!("key-saved-unchecked", error = error), false));
                    Task::none()
                }
            },
            Setting::NexusKey { then, .. } => {
                self.nexus.key_stored(then);
                Task::none()
            }
            Setting::DefaultProton(_) => {
                self.protons.default_set();
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn check_key(&mut self) -> Task<Message> {
        let key = self.key.value.trim().to_owned();
        if key.is_empty() {
            self.key.note = Some((t!("key-paste-first"), false));
            return Task::none();
        }
        self.key.checking = Some(key.clone());
        self.key.note = None;
        if !self.persist {
            return Task::none();
        }
        let asking = key.clone();
        Task::perform(
            blocking(move || covers::check_key(&key).map_err(|err| format!("{err:#}"))),
            move |result| Message::KeyChecked(asking.clone(), result),
        )
    }

    pub(super) fn settings_page(&self) -> Element<'_, Message> {
        let header = column![
            text(t!("page-settings")).size(28).font(BOLD),
            text(t!("settings-subtitle")).size(13).color(colors().dim),
        ]
        .spacing(2);
        scrollable(
            container(
                column![
                    header,
                    self.language_section(),
                    self.theme_section(),
                    self.key_section(),
                    self.nexus_section(),
                    self.covers_section(),
                    self.downloads_section(),
                    self.protons_section(),
                    self.runtimes_section(),
                    self.overlay_section(),
                    self.sources_section(),
                    files_section(self.config.install_dir(), &self.diagnostics.folders),
                    self.diagnostics_section(),
                ]
                .spacing(20)
                .padding(24)
                .max_width(960),
            )
            .center_x(Length::Fill),
        )
        .height(Length::Fill)
        .into()
    }

    fn key_section(&self) -> Element<'_, Message> {
        let saved = self.config.steamgriddb_key.as_deref().unwrap_or("");
        let changed = self.key.value.trim() != saved;
        let field = row![
            text_input(&t!("steamgriddb-paste"), &self.key.value)
                .id(key_id())
                .secure(!self.key.shown)
                .on_input(Message::KeyInput)
                .on_submit(Message::SaveKey)
                .padding([8, 12])
                .size(14)
                .style(style::input),
            button(
                text(if self.key.shown {
                    t!("key-hide")
                } else {
                    t!("key-show")
                })
                .size(14)
            )
            .padding([8, 14])
            .style(style::secondary)
            .on_press(Message::ToggleKeyShown),
            button(
                text(if self.key.checking.is_some() {
                    t!("key-checking")
                } else {
                    t!("key-check")
                })
                .size(14)
            )
            .padding([8, 14])
            .style(style::secondary)
            .on_press_maybe(
                (self.key.checking.is_none() && !self.key.value.trim().is_empty())
                    .then_some(Message::CheckKey)
            ),
            button(text(t!("key-save")).size(14).font(BOLD))
                .padding([8, 18])
                .style(style::primary)
                .on_press_maybe(changed.then_some(Message::SaveKey)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let mut body = column![field].spacing(10);
        match &self.key.note {
            Some((note, good)) => {
                body = body.push(text(note).size(13).color(if *good {
                    colors().good
                } else {
                    colors().bad
                }));
            }
            None if saved.is_empty() => {
                body = body.push(text(t!("key-none")).size(13).color(colors().dim));
            }
            None => {}
        }
        body = body.push(kept_note(self.config.kept(Key::SteamGridDb)));
        body = body.push(
            button(text(t!("steamgriddb-get-key")).size(13))
                .padding(0)
                .style(style::link)
                .on_press(Message::Open(KEY_PAGE.to_owned())),
        );
        section("SteamGridDB", t!("steamgriddb-about"), body)
    }

    fn diagnostics_section(&self) -> Element<'_, Message> {
        // Found when Settings opened: `view` reads state only.
        let logs = self.diagnostics.logs.as_ref();
        let mut body = column![
            row![
                button(
                    text(if self.diagnostics.gathering {
                        t!("report-gathering")
                    } else {
                        t!("report-copy")
                    })
                    .size(14)
                    .font(BOLD)
                )
                .padding([8, 18])
                .style(style::primary)
                .on_press_maybe((!self.diagnostics.gathering).then_some(Message::CopyReport)),
                button(text(t!("report-open-logs")).size(14))
                    .padding([8, 14])
                    .style(style::secondary)
                    .on_press_maybe(
                        logs.map(|dir| Message::Open(dir.to_string_lossy().into_owned()))
                    ),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        ]
        .spacing(10);
        if let Some((note, good)) = &self.diagnostics.note {
            body = body.push(text(note).size(13).color(if *good {
                colors().good
            } else {
                colors().bad
            }));
        }
        section(t!("report-title"), t!("report-about"), body)
    }

    fn language_section(&self) -> Element<'_, Message> {
        let chosen = self.config.language.as_deref().and_then(Language::from_tag);
        let choice = |label: String, language: Option<Language>| {
            button(text(label).size(13))
                .padding([5, 12])
                .style(style::chip(chosen == language))
                .on_press(Message::SetLanguage(language))
        };
        let mut choices = row![choice(
            t!("language-system", language = Language::system().name()),
            None
        )]
        .spacing(8);
        for language in Language::ALL {
            choices = choices.push(choice(language.name().to_owned(), Some(language)));
        }
        section(t!("language-title"), t!("language-about"), choices)
    }

    fn covers_section(&self) -> Element<'_, Message> {
        let busy = self.images.left > 0;
        let progress = if busy {
            t!("images-left", count = self.images.left)
        } else {
            String::new()
        };
        let body = column![
            toggler(self.config.download_covers)
                .label(t!("covers-download"))
                .text_size(14)
                .on_toggle(Message::SetDownloadCovers),
            row![
                button(text(t!("covers-download-now")).size(14))
                    .padding([8, 14])
                    .style(style::secondary)
                    .on_press_maybe(
                        (self.config.download_covers && !busy).then_some(Message::DownloadNow)
                    ),
                text(progress).size(13).color(colors().dim),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        ]
        .spacing(14);
        section(t!("covers-title"), t!("covers-about"), body)
    }

    /// Updating by itself, and how many files the engines download at once.
    fn downloads_section(&self) -> Element<'_, Message> {
        let chosen = self.config.download_workers.filter(|workers| *workers > 0);
        let choice = |label: String, workers: Option<u32>| {
            button(text(label).size(13))
                .padding([5, 12])
                .style(style::chip(chosen == workers))
                .on_press(Message::SetWorkers(workers))
        };
        let mut choices = row![choice(t!("workers-engine"), None)]
            .spacing(8)
            .align_y(Alignment::Center);
        for workers in WORKERS {
            choices = choices.push(choice(workers.to_string(), Some(workers)));
        }
        let body = column![
            toggler(self.config.auto_update)
                .label(t!("auto-update"))
                .text_size(14)
                .on_toggle(Message::SetAutoUpdate),
            column![
                text(t!("workers-title")).size(14),
                choices,
                text(t!("workers-about")).size(13).color(colors().dim),
            ]
            .spacing(8),
        ]
        .spacing(14);
        section(
            t!("downloads-settings-title"),
            t!("downloads-settings-about"),
            body,
        )
    }

    fn sources_section(&self) -> Element<'_, Message> {
        let scanning = self.library.is_none();
        let games = self.library.iter().flat_map(|library| &library.games);
        let mut rows: Vec<Element<'_, Message>> = [
            (Source::Steam, "Steam".to_owned()),
            (Source::Epic, t!("sources-epic")),
            (Source::Gog, t!("sources-gog")),
            (Source::Amazon, t!("sources-amazon")),
            (Source::Heroic, t!("sources-heroic")),
            (Source::Desktop, t!("sources-desktop")),
            (Source::Local, t!("sources-local")),
        ]
        .into_iter()
        .map(|(source, label)| {
            let count = games.clone().filter(|game| game.source == source).count();
            row![
                text(label).size(14).width(320),
                if scanning {
                    // A first scan still running is not "none found".
                    text(t!("library-scanning")).size(14).color(colors().dim)
                } else if count == 0 {
                    text(t!("sources-none-found")).size(14).color(colors().dim)
                } else {
                    text(t!("count-games", count = count)).size(14)
                },
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .wrap()
            .into()
        })
        .collect();
        for problem in self.library.iter().flat_map(|library| &library.problems) {
            rows.push(
                text(t!("sources-unreadable", problem = problem.as_str()))
                    .size(13)
                    .color(colors().bad)
                    .into(),
            );
        }
        section(
            t!("sources-title"),
            t!("sources-about"),
            column(rows).spacing(8),
        )
    }
}

fn files_section<'a>(
    games: Option<PathBuf>,
    folders: &'a [Option<PathBuf>],
) -> Element<'a, Message> {
    let entry =
        |label: String, path: Option<PathBuf>, folder: Option<PathBuf>| -> Element<'a, Message> {
            let shown = path
                .as_ref()
                .map_or_else(|| t!("files-no-home"), |path| path.display().to_string());
            row![
                text(label).size(14).color(colors().dim).width(160),
                text(shown)
                    .size(13)
                    .font(Font::MONOSPACE)
                    .width(Length::Fill),
                button(text(t!("files-open-folder")).size(13))
                    .padding([4, 10])
                    .style(style::secondary)
                    .on_press_maybe(
                        folder.map(|folder| Message::Open(folder.to_string_lossy().into_owned()))
                    ),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
        };
    let rows: Vec<Element<'a, Message>> = [
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
        (t!("files-games"), games),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (label, path))| entry(label, path, folders.get(index).cloned().flatten()))
    .collect();
    section(
        t!("files-title"),
        t!("files-about"),
        column(rows).spacing(10),
    )
}

/// Settings' Diagnostics section.
#[derive(Debug, Default)]
pub(super) struct Diagnostics {
    gathering: bool,
    /// What happened to the last report, and whether it went well.
    note: Option<(String, bool)>,
    /// Where the logs are, and the folders the Files section can open:
    /// found when Settings opens, not in `view`.
    logs: Option<PathBuf>,
    folders: Vec<Option<PathBuf>>,
}

#[cfg(test)]
impl Diagnostics {
    pub(super) fn note(&self) -> Option<&(String, bool)> {
        self.note.as_ref()
    }
}

/// Where a key is kept, or why the keyring could not be used; nothing
/// when there is no key.
pub(super) fn kept_note<'a>(kept: &Kept) -> Option<Element<'a, Message>> {
    let (note, color) = match kept {
        Kept::Nowhere => return None,
        Kept::Keyring => (t!("kept-keyring"), colors().dim),
        Kept::File(None) => (t!("kept-file"), colors().dim),
        Kept::File(Some(why)) => (t!("kept-file-why", why = why.as_str()), colors().dim),
        Kept::Unread(why) => (t!("kept-unread", why = why.as_str()), colors().bad),
    };
    Some(text(note).size(13).color(color).into())
}

pub(super) fn section<'a>(
    title: impl text::IntoFragment<'a>,
    about: impl text::IntoFragment<'a>,
    body: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(
        column![
            text(title).size(18).font(BOLD),
            text(about).size(13).color(colors().dim),
            body.into(),
        ]
        .spacing(10),
    )
    .padding(20)
    .width(Length::Fill)
    .style(style::card)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_for_a_key_no_longer_checked_saves_nothing() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.key.value = "sgdb-old".into();
        let _ = zendo.update(Message::SaveKey); // checks "sgdb-old"
        zendo.key.value.clear();
        let _ = zendo.update(Message::SaveKey); // removes the key
        let _ = zendo.update(Message::KeyChecked(
            "sgdb-old".into(),
            Ok(KeyCheck::Accepted),
        ));
        assert_eq!(
            zendo.config.steamgriddb_key, None,
            "the removed key stays gone"
        );
    }
}
