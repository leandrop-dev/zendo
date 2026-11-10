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
use zendo_core::secrets::Key;
use zendo_core::{Source, State, launch, report};

use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking, key_id};
use crate::i18n::{self, Language, t};

const KEY_PAGE: &str = "https://www.steamgriddb.com/profile/preferences/api";

/// The download workers Settings offers besides the engine's own choice,
/// which is the computer's processors (legendary at most 16).
const WORKERS: [u32; 4] = [2, 4, 8, 16];

impl Zendo {
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
                    if self.store_key(None) {
                        self.key.note = Some((t!("key-removed"), true));
                    }
                    return Task::none();
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
            Message::SetDownloadCovers(on) => {
                self.config.download_covers = on;
                if !self.save_config(|config| config.download_covers = on) {
                    self.config.download_covers = !on;
                    return Task::none();
                }
                if on {
                    self.download_images()
                } else {
                    Task::none()
                }
            }
            Message::SetAutoUpdate(on) => self.set_auto_update(on),
            Message::SetWorkers(workers) => {
                let previous = std::mem::replace(&mut self.config.download_workers, workers);
                if !self.save_config(|config| config.download_workers = workers) {
                    self.config.download_workers = previous;
                }
                Task::none()
            }
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
                        self.diagnostics.note = Some((t!("report-copied", count = lines), true));
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
                let previous = std::mem::replace(
                    &mut self.config.language,
                    language.map(|language| language.tag().to_owned()),
                );
                if !self.save_config(|config| {
                    config.language = language.map(|language| language.tag().to_owned());
                }) {
                    self.config.language = previous;
                }
                i18n::set(Language::chosen(self.config.language.as_deref()));
                Task::none()
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
            Ok(KeyCheck::Accepted) if save => {
                if self.store_key(Some(key)) {
                    self.key.note = Some((t!("steamgriddb-saved"), true));
                    return self.download_images();
                }
            }
            Ok(KeyCheck::Accepted) => {
                self.key.note = Some((t!("steamgriddb-accepted"), true));
            }
            Ok(KeyCheck::Rejected) => {
                let note = if save {
                    t!("steamgriddb-rejected-unsaved")
                } else {
                    t!("steamgriddb-rejected")
                };
                self.key.note = Some((note, false));
            }
            // Offline is no reason to lose what the user pasted.
            Err(err) if save => {
                if self.store_key(Some(key)) {
                    self.key.note = Some((t!("key-saved-unchecked", error = err), false));
                }
            }
            Err(err) => self.key.note = Some((err, false)),
        }
        Task::none()
    }

    /// Sets the key and writes the settings; on failure the old key stays.
    fn store_key(&mut self, key: Option<String>) -> bool {
        let setting = key.clone();
        let previous = std::mem::replace(&mut self.config.steamgriddb_key, key);
        if self.save_config(|config| config.steamgriddb_key = setting) {
            true
        } else {
            self.config.steamgriddb_key = previous;
            false
        }
    }

    /// Writes a change to the settings, saying so on the page when it
    /// fails. The change is applied to the file as it is now, under its
    /// lock, so a hand edit or another writer is not overwritten.
    pub(super) fn save_config(&mut self, change: impl FnOnce(&mut Config)) -> bool {
        if !self.persist {
            change(&mut self.config);
            return true;
        }
        match Config::update(change) {
            Ok(config) => {
                self.config = config;
                true
            }
            Err(err) => {
                self.key.note = Some((format!("{err:#}"), false));
                false
            }
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
                files_section(self.config.install_dir()),
                self.diagnostics_section(),
            ]
            .spacing(20)
            .padding(24)
            .max_width(960),
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
        let logs = launch::logs_dir().filter(|dir| dir.is_dir());
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
                if count == 0 {
                    text(t!("sources-none-found")).size(14).color(colors().dim)
                } else {
                    text(t!("count-games", count = count)).size(14)
                },
            ]
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

fn files_section<'a>(games: Option<PathBuf>) -> Element<'a, Message> {
    let entry = |label: String, path: Option<PathBuf>| -> Element<'a, Message> {
        let shown = path
            .as_ref()
            .map_or_else(|| t!("files-no-home"), |path| path.display().to_string());
        // The folder, as the file may not exist yet.
        let folder = path
            .as_deref()
            .map(|path| {
                if path.extension().is_some() {
                    path.parent().unwrap_or(path)
                } else {
                    path
                }
            })
            .filter(|folder| folder.is_dir())
            .map(Path::to_path_buf);
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
    section(
        t!("files-title"),
        t!("files-about"),
        column![
            entry(t!("files-settings"), Config::path()),
            entry(t!("files-state"), State::path()),
            entry(t!("files-images"), covers::dir()),
            entry(
                t!("files-queue"),
                State::path().map(|state| state.with_file_name("downloads.json"))
            ),
            entry(
                t!("files-sign-ins"),
                zendo_core::stores::Store::Epic
                    .dir()
                    .and_then(|dir| dir.parent().map(Path::to_path_buf))
            ),
            entry(t!("files-games"), games),
        ]
        .spacing(10),
    )
}

/// Settings' Diagnostics section.
#[derive(Debug, Default)]
pub(super) struct Diagnostics {
    gathering: bool,
    /// What happened to the last report, and whether it went well.
    note: Option<(String, bool)>,
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
