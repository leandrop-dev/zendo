//! The Settings page: the `SteamGridDB` key, covers, the sources found and
//! Zendo's own files.

use std::path::{Path, PathBuf};

use iced::widget::{
    button, column, container, operation, row, scrollable, text, text_input, toggler,
};
use iced::{Alignment, Element, Font, Length, Task};
use zendo_core::config::Config;
use zendo_core::covers::{self, KeyCheck};
use zendo_core::{Source, State};

use super::style::{self, BAD, BOLD, DIM, GOOD};
use super::{Message, Zendo, blocking, format, key_id};

const KEY_PAGE: &str = "https://www.steamgriddb.com/profile/preferences/api";

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
                        self.key.note = Some(("Key removed".to_owned(), true));
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
            Message::KeyChecked(result) => self.key_checked(result),
            Message::SetDownloadCovers(on) => {
                self.config.download_covers = on;
                if !self.save_config() {
                    self.config.download_covers = !on;
                    return Task::none();
                }
                if on {
                    self.download_images()
                } else {
                    Task::none()
                }
            }
            Message::DownloadNow => {
                let task = self.download_images();
                if self.downloads.left == 0 {
                    self.status = Some("No images to download".to_owned());
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

    fn key_checked(&mut self, result: Result<KeyCheck, String>) -> Task<Message> {
        let Some(key) = self.key.checking.take() else {
            return Task::none();
        };
        let save = std::mem::take(&mut self.key.save_after_check);
        match result {
            Ok(KeyCheck::Accepted) if save => {
                if self.store_key(Some(key)) {
                    self.key.note = Some(("Saved. SteamGridDB accepts this key.".to_owned(), true));
                    return self.download_images();
                }
            }
            Ok(KeyCheck::Accepted) => {
                self.key.note = Some((
                    "SteamGridDB accepts this key; Save keeps it.".to_owned(),
                    true,
                ));
            }
            Ok(KeyCheck::Rejected) => {
                self.key.note = Some((
                    format!(
                        "SteamGridDB turned this key down{}. Copy it again from your \
                         SteamGridDB preferences.",
                        if save { ", so it was not saved" } else { "" }
                    ),
                    false,
                ));
            }
            // Offline is no reason to lose what the user pasted.
            Err(err) if save => {
                if self.store_key(Some(key)) {
                    self.key.note = Some((format!("Saved, but not checked: {err}"), false));
                }
            }
            Err(err) => self.key.note = Some((err, false)),
        }
        Task::none()
    }

    /// Sets the key and writes the settings; on failure the old key stays.
    fn store_key(&mut self, key: Option<String>) -> bool {
        let previous = std::mem::replace(&mut self.config.steamgriddb_key, key);
        if self.save_config() {
            true
        } else {
            self.config.steamgriddb_key = previous;
            false
        }
    }

    /// Writes the settings, saying so on the page when it fails.
    fn save_config(&mut self) -> bool {
        if !self.persist {
            return true;
        }
        match self.config.save() {
            Ok(()) => true,
            Err(err) => {
                self.key.note = Some((format!("{err:#}"), false));
                false
            }
        }
    }

    fn check_key(&mut self) -> Task<Message> {
        let key = self.key.value.trim().to_owned();
        if key.is_empty() {
            self.key.note = Some(("Paste a key first".to_owned(), false));
            return Task::none();
        }
        self.key.checking = Some(key.clone());
        self.key.note = None;
        if !self.persist {
            return Task::none();
        }
        Task::perform(
            blocking(move || covers::check_key(&key).map_err(|err| format!("{err:#}"))),
            Message::KeyChecked,
        )
    }

    pub(super) fn settings_page(&self) -> Element<'_, Message> {
        let header = column![
            text("Settings").size(28).font(BOLD),
            text("Covers, the sources Zendo reads and its own files")
                .size(13)
                .color(DIM),
        ]
        .spacing(2);
        scrollable(
            column![
                header,
                self.key_section(),
                self.covers_section(),
                self.sources_section(),
                files_section(),
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
            text_input("Paste your SteamGridDB API key", &self.key.value)
                .id(key_id())
                .secure(!self.key.shown)
                .on_input(Message::KeyInput)
                .on_submit(Message::SaveKey)
                .padding([8, 12])
                .size(14)
                .style(style::input),
            button(text(if self.key.shown { "Hide" } else { "Show" }).size(14))
                .padding([8, 14])
                .style(style::secondary)
                .on_press(Message::ToggleKeyShown),
            button(
                text(if self.key.checking.is_some() {
                    "Checking…"
                } else {
                    "Check"
                })
                .size(14)
            )
            .padding([8, 14])
            .style(style::secondary)
            .on_press_maybe(
                (self.key.checking.is_none() && !self.key.value.trim().is_empty())
                    .then_some(Message::CheckKey)
            ),
            button(text("Save").size(14).font(BOLD))
                .padding([8, 18])
                .style(style::primary)
                .on_press_maybe(changed.then_some(Message::SaveKey)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let mut body = column![field].spacing(10);
        match &self.key.note {
            Some((note, good)) => {
                body = body.push(text(note).size(13).color(if *good { GOOD } else { BAD }));
            }
            None if saved.is_empty() => {
                body = body.push(text("No key set").size(13).color(DIM));
            }
            None => {}
        }
        body = body.push(
            button(text("Get a free key from your SteamGridDB preferences").size(13))
                .padding(0)
                .style(style::link)
                .on_press(Message::Open(KEY_PAGE.to_owned())),
        );
        section(
            "SteamGridDB",
            "Covers and wide images for games whose store has none. The key is \
             kept in your settings file, readable only by you.",
            body,
        )
    }

    fn covers_section(&self) -> Element<'_, Message> {
        let busy = self.downloads.left > 0;
        let progress = if busy {
            format!("{} left", self.downloads.left)
        } else {
            String::new()
        };
        let body = column![
            toggler(self.config.download_covers)
                .label("Download missing covers and wide images")
                .text_size(14)
                .on_toggle(Message::SetDownloadCovers),
            row![
                button(text("Download missing images now").size(14))
                    .padding([8, 14])
                    .style(style::secondary)
                    .on_press_maybe(
                        (self.config.download_covers && !busy).then_some(Message::DownloadNow)
                    ),
                text(progress).size(13).color(DIM),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        ]
        .spacing(14);
        section(
            "Covers",
            "Steam's come from its own cache. Epic, GOG and Amazon publish theirs; \
             the rest come from SteamGridDB. A game with none anywhere is asked \
             about again after a week.",
            body,
        )
    }

    fn sources_section(&self) -> Element<'_, Message> {
        let games = self.library.iter().flat_map(|library| &library.games);
        let mut rows: Vec<Element<'_, Message>> = [
            (Source::Steam, "Steam"),
            (Source::Epic, "Epic Games, through Heroic"),
            (Source::Gog, "GOG, through Heroic"),
            (Source::Amazon, "Amazon Games, through Heroic"),
            (Source::Heroic, "Added to Heroic by hand"),
            (Source::Desktop, ".desktop files in the Game category"),
        ]
        .into_iter()
        .map(|(source, label)| {
            let count = games.clone().filter(|game| game.source == source).count();
            row![
                text(label).size(14).width(320),
                if count == 0 {
                    text("none found").size(14).color(DIM)
                } else {
                    text(format::count(count, "game", "games")).size(14)
                },
            ]
            .into()
        })
        .collect();
        for problem in self.library.iter().flat_map(|library| &library.problems) {
            rows.push(
                text(format!("Could not read {problem}"))
                    .size(13)
                    .color(BAD)
                    .into(),
            );
        }
        section(
            "Sources",
            "Where Zendo looks for games. It reads these and never writes to them; \
             Refresh in the library looks again.",
            column(rows).spacing(8),
        )
    }
}

fn files_section<'a>() -> Element<'a, Message> {
    let entry = |label: &'static str, path: Option<PathBuf>| -> Element<'a, Message> {
        let shown = path.as_ref().map_or_else(
            || "no home folder".to_owned(),
            |path| path.display().to_string(),
        );
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
            text(label).size(14).color(DIM).width(160),
            text(shown)
                .size(13)
                .font(Font::MONOSPACE)
                .width(Length::Fill),
            button(text("Open folder").size(13))
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
        "Files",
        "Zendo's own files. Removing one resets what it holds.",
        column![
            entry("Settings", Config::path()),
            entry("Launches, pins, hidden", State::path()),
            entry("Downloaded images", covers::dir()),
        ]
        .spacing(10),
    )
}

fn section<'a>(
    title: &'a str,
    about: &'a str,
    body: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(
        column![
            text(title).size(18).font(BOLD),
            text(about).size(13).color(DIM),
            body.into(),
        ]
        .spacing(10),
    )
    .padding(20)
    .width(Length::Fill)
    .style(style::card)
    .into()
}
