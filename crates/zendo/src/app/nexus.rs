//! Nexus Mods in the window: the key on the Settings page, `nxm://` links
//! arriving (from the browser, through `links.rs`), the game each one is
//! for, its download into the game's mods, and updates to mods that came
//! from Nexus. The state and the transitions are `zendo_shell::nexus`'
//! (shared with the Slint window); this file draws them and runs the work
//! on threads.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, column, container, progress_bar, row, space, text, text_input};
use iced::{Alignment, Element, Length, Task};
use zendo_core::Game;
use zendo_core::mods::Overview;
use zendo_core::nexus::{self, FileInfo, Handler, KeyCheck, Link};
use zendo_core::secrets::Key;
use zendo_shell::nexus::{
    self as shell, Arrived, Checked, Choosing, Download, Finished, KeyStep, Matched, Matching,
    Update,
};

use super::launch_options::{frame, overlay};
use super::settings::{NexusThen, kept_note, section};
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

pub use shell::NexusState;
use shell::UpdateList;

#[derive(Debug, Clone)]
pub enum NexusMessage {
    KeyInput(String),
    ToggleShown,
    SaveKey,
    CheckKey,
    KeyChecked(String, Result<KeyCheck, String>),
    /// What opens links, as `xdg-mime` says.
    Handler(Handler),
    BecomeHandler,
    BecameHandler(Result<String, String>),
    Link(String),
    /// The games a link's game matched, the installed ones, and Nexus's
    /// name for it.
    Matched(Matched),
    /// Something to say on the status line.
    Note(String),
    Choose(String),
    CancelChoose,
    Progress(u64, u64, Option<u64>),
    Finished(
        u64,
        String,
        Result<(zendo_core::mods::Added, String), String>,
    ),
    Stop(u64),
    Updates(String, Result<UpdateList, String>),
    Update(String, String),
}

impl Zendo {
    /// After the settings are read: whose key it is, what opens links, and
    /// the links that came while the library was read.
    pub(super) fn nexus_loaded(&mut self) -> Task<Message> {
        self.nexus.loaded(self.config.nexus_key.as_deref());
        if !self.persist {
            return Task::none();
        }
        let mut tasks = vec![Task::perform(blocking(nexus::handler), |handler| {
            Message::Nexus(NexusMessage::Handler(handler))
        })];
        if let Some(key) = self.config.nexus_key.clone() {
            let asking = key.clone();
            tasks.push(Task::perform(
                blocking(move || shell::check_key(&key, true)),
                move |result| Message::Nexus(NexusMessage::KeyChecked(asking.clone(), result)),
            ));
        }
        for link in std::mem::take(&mut self.nexus.pending) {
            tasks.push(Task::done(Message::Nexus(NexusMessage::Link(link))));
        }
        Task::batch(tasks)
    }

    /// Whether the key's account has Premium; `None` before Nexus said.
    pub(super) fn nexus_premium(&self) -> Option<bool> {
        self.nexus.premium()
    }

    /// Downloads a file of a mod into a game's mods, as a Premium account
    /// may without the site's button.
    pub(super) fn download_nexus_file(&mut self, id: &str, link: Link) -> Task<Message> {
        self.download_mod(id, link, false)
    }

    /// A link opened before the window: taken once the library is read.
    pub(super) fn nexus_pending(&mut self, link: String) {
        self.nexus.pending.push(link);
    }

    pub(super) fn update_nexus(&mut self, message: NexusMessage) -> Task<Message> {
        match message {
            NexusMessage::Link(text) => self.link_arrived(&text),
            NexusMessage::Note(note) => {
                self.status = Some(note);
                Task::none()
            }
            NexusMessage::Matched(found) => match self.nexus.matched(found) {
                Matching::Download(id, link) => self.download_mod(&id, link, false),
                Matching::Say(note) => {
                    self.status = Some(note);
                    Task::none()
                }
                Matching::Choose => Task::none(),
            },
            NexusMessage::Choose(id) => match self.nexus.choose(&id) {
                Some((id, link, remember)) => self.download_mod(&id, link, remember),
                None => Task::none(),
            },
            NexusMessage::CancelChoose => {
                self.nexus.cancel_choose();
                Task::none()
            }
            NexusMessage::Progress(id, done, total) => {
                self.nexus.progress(id, done, total);
                Task::none()
            }
            NexusMessage::Finished(id, game, result) => self.mod_downloaded(id, game, result),
            NexusMessage::Stop(id) => {
                self.nexus.stop(id);
                Task::none()
            }
            NexusMessage::Updates(game, result) => {
                if let Some(note) = self.nexus.updates_found(&game, result) {
                    self.status = Some(note);
                }
                Task::none()
            }
            NexusMessage::Update(game, mod_id) => self.update_from_nexus(&game, &mod_id),
            key => self.update_nexus_key(key),
        }
    }

    /// The Settings page's half: the key and what opens links.
    fn update_nexus_key(&mut self, message: NexusMessage) -> Task<Message> {
        match message {
            NexusMessage::KeyInput(value) => {
                self.nexus.edit_key(value);
                Task::none()
            }
            NexusMessage::ToggleShown => {
                self.nexus.toggle_shown();
                Task::none()
            }
            NexusMessage::SaveKey => match self.nexus.save_key(self.persist) {
                KeyStep::Remove => self.save_setting(super::settings::Setting::NexusKey {
                    key: None,
                    then: NexusThen::Removed,
                }),
                KeyStep::Check(key) => check_nexus_key(key),
                KeyStep::Nothing => Task::none(),
            },
            NexusMessage::CheckKey => match self.nexus.check_key(self.persist) {
                KeyStep::Check(key) => check_nexus_key(key),
                KeyStep::Nothing | KeyStep::Remove => Task::none(),
            },
            NexusMessage::KeyChecked(checked, result) => {
                let stored = self.config.nexus_key.clone();
                match self.nexus.key_checked(&checked, result, stored.as_deref()) {
                    Checked::Save(key, then) => {
                        self.save_setting(super::settings::Setting::NexusKey {
                            key: Some(key),
                            then,
                        })
                    }
                    Checked::Nothing => Task::none(),
                }
            }
            NexusMessage::Handler(handler) => {
                self.nexus.handler_known(handler);
                Task::none()
            }
            NexusMessage::BecomeHandler => {
                if !self.persist {
                    return Task::none();
                }
                Task::perform(blocking(shell::become_handler), |result| {
                    Message::Nexus(NexusMessage::BecameHandler(result))
                })
            }
            NexusMessage::BecameHandler(result) => {
                self.status = Some(self.nexus.became_handler(result));
                Task::none()
            }
            _ => Task::none(),
        }
    }

    /// A link from the browser: which game it is for, asked on a thread.
    fn link_arrived(&mut self, text: &str) -> Task<Message> {
        let key = self.config.nexus_key.clone();
        let library = self.library.is_some();
        let (link, note) = match self.nexus.link_arrived(text, library, key.is_some()) {
            Arrived::Say(note) => {
                self.status = Some(note);
                return Task::none();
            }
            Arrived::Wait => return Task::none(),
            Arrived::Ask(link, note) => (link, note),
        };
        let (Some(key), Some(library)) = (key, &self.library) else {
            return Task::none();
        };
        if !self.persist {
            return Task::none();
        }
        let games = shell::with_folders(&library.games);
        self.status = Some(note);
        Task::perform(
            blocking(move || Ok::<_, String>(shell::match_games(&key, &link, &games))),
            |found| match found {
                Ok(found) => Message::Nexus(NexusMessage::Matched(found)),
                Err(err) => Message::Nexus(NexusMessage::Note(err)),
            },
        )
    }

    /// Downloads the link's file into a game's mods; `remember` keeps the
    /// game as the link's game on Nexus.
    fn download_mod(&mut self, id: &str, link: Link, remember: bool) -> Task<Message> {
        let (Some(game), Some(key)) = (self.game(id).cloned(), self.config.nexus_key.clone())
        else {
            return Task::none();
        };
        let Some(started) = self.nexus.begin(&game, &link) else {
            return Task::none();
        };
        self.status = Some(started.note);
        if !self.persist {
            return Task::none();
        }
        Task::run(
            fetch_mod(started.id, game, key, link, remember, started.stop),
            Message::Nexus,
        )
    }

    fn mod_downloaded(
        &mut self,
        id: u64,
        game: String,
        result: Result<(zendo_core::mods::Added, String), String>,
    ) -> Task<Message> {
        match self.nexus.finished(id, &game, result) {
            Finished::Choose(pending) => self.open_installer(game, pending),
            Finished::Done(overview, note) => {
                self.status = Some(note);
                self.mods_ready(game, *overview)
            }
            Finished::Say(note) => {
                self.status = Some(note);
                Task::none()
            }
        }
    }

    /// Asks Nexus, once a session, for newer files of a game's mods.
    pub(super) fn check_mod_updates(&mut self, game: &str, overview: &Overview) -> Task<Message> {
        let key = self.config.nexus_key.clone();
        let Some(job) = self
            .nexus
            .want_updates(game, overview, key.is_some(), self.persist)
        else {
            return Task::none();
        };
        let Some(key) = key else {
            return Task::none();
        };
        let game = job.game;
        Task::perform(
            blocking(move || Ok::<_, String>(shell::ask_updates(&key, job.sources))),
            move |result| Message::Nexus(NexusMessage::Updates(game.clone(), result)),
        )
    }

    /// Update on a mod: downloaded straight away with Premium; otherwise
    /// the file's page opens, for the site's button to send it back.
    fn update_from_nexus(&mut self, game: &str, mod_id: &str) -> Task<Message> {
        let update = self
            .mods_overview(game)
            .and_then(|overview| self.nexus.update(game, mod_id, overview));
        match update {
            Some(Update::Download(link)) => self.download_mod(game, link, false),
            Some(Update::Page(url, note)) => {
                self.status = Some(note);
                Task::done(Message::Open(url))
            }
            None => Task::none(),
        }
    }

    /// The newer file Nexus has for a mod, when one was found.
    pub(super) fn mod_update(&self, game: &str, mod_id: &str) -> Option<&FileInfo> {
        self.nexus.update_for(game, mod_id)
    }

    pub(super) fn nexus_downloads_for<'a>(
        &'a self,
        game: &'a str,
    ) -> impl Iterator<Item = &'a Download> + 'a {
        self.nexus.downloads_for(game)
    }

    /// The Downloads page's card of mods downloading from Nexus.
    pub(super) fn mod_downloads_card(&self) -> Option<Element<'_, Message>> {
        if self.nexus.downloads.is_empty() {
            return None;
        }
        let mut card = column![
            text(t!("nexus-downloads-title").to_uppercase())
                .size(14)
                .font(BOLD)
        ]
        .spacing(style::SPACE_M);
        for download in &self.nexus.downloads {
            card = card.push(download_row(download));
        }
        Some(
            container(card)
                .padding(style::SPACE_L)
                .width(Length::Fill)
                .style(style::card)
                .into(),
        )
    }

    /// The Escape and Enter of the game chooser, while it is open.
    pub(super) fn chooser_takes(&mut self, message: &Message) -> Option<Task<Message>> {
        self.nexus.choosing.as_ref()?;
        match message {
            Message::Back => Some(self.update_nexus(NexusMessage::CancelChoose)),
            Message::Move(_)
            | Message::Type(_)
            | Message::Backspace
            | Message::Activate
            | Message::Play
            | Message::Navigate(_)
            | Message::Clicked(_)
            | Message::ToggleHidden
            | Message::TogglePinned
            | Message::ShowHidden
            | Message::CycleSort
            | Message::FocusSearch
            | Message::PasteClipboard
            | Message::Pad(_) => Some(Task::none()),
            _ => None,
        }
    }

    pub(super) fn with_chooser<'a>(&'a self, page: Element<'a, Message>) -> Element<'a, Message> {
        match &self.nexus.choosing {
            Some(choosing) => overlay(page, chooser_card(choosing)),
            None => page,
        }
    }

    /// What opens Mod manager download links, and the button to make it
    /// this Zendo.
    fn handler_row(&self) -> Element<'_, Message> {
        let (handler, is_zendo, can) = self.nexus.handler_row();
        row![
            text(handler)
                .size(13)
                .color(if is_zendo {
                    colors().good
                } else {
                    colors().dim
                })
                .width(Length::Fill),
            button(style::button_text(t!("nexus-become-handler")))
                .padding(style::BUTTON_SMALL)
                .style(style::secondary)
                .on_press_maybe(can.then_some(Message::Nexus(NexusMessage::BecomeHandler))),
        ]
        .spacing(style::SPACE_M)
        .align_y(Alignment::Center)
        .into()
    }

    pub(super) fn nexus_section(&self) -> Element<'_, Message> {
        let key = &self.nexus.key;
        let saved = self.config.nexus_key.as_deref().unwrap_or("");
        let changed = key.value.trim() != saved;
        let field = row![
            text_input(&t!("nexus-paste"), &key.value)
                .secure(!key.shown)
                .on_input(|value| Message::Nexus(NexusMessage::KeyInput(value)))
                .on_submit(Message::Nexus(NexusMessage::SaveKey))
                .padding([style::SPACE_S, style::SPACE_M])
                .size(14)
                .style(style::input),
            button(style::button_text(if key.shown {
                t!("key-hide")
            } else {
                t!("key-show")
            }))
            .padding(style::BUTTON)
            .style(style::secondary)
            .on_press(Message::Nexus(NexusMessage::ToggleShown)),
            button(style::button_text(if key.checking.is_some() {
                t!("key-checking")
            } else {
                t!("key-check")
            }))
            .padding(style::BUTTON)
            .style(style::secondary)
            .on_press_maybe(
                (key.checking.is_none() && !key.value.trim().is_empty())
                    .then_some(Message::Nexus(NexusMessage::CheckKey))
            ),
            button(style::button_text(t!("key-save")).font(BOLD))
                .padding(style::BUTTON)
                .style(style::primary)
                .on_press_maybe(changed.then_some(Message::Nexus(NexusMessage::SaveKey))),
        ]
        .spacing(style::SPACE_S)
        .align_y(Alignment::Center);

        let mut body = column![field].spacing(style::SPACE_M);
        match (&key.note, &self.nexus.user) {
            (Some((note, good)), _) => {
                body = body.push(text(note).size(13).color(if *good {
                    colors().good
                } else {
                    colors().bad
                }));
            }
            (None, _) if saved.is_empty() => {
                body = body.push(text(t!("key-none")).size(13).color(colors().dim));
            }
            (None, Some(user)) => {
                body = body.push(text(shell::signed_in(user)).size(13).color(colors().good));
            }
            (None, None) => {}
        }
        body = body.push(kept_note(self.config.kept(Key::Nexus)));
        body = body.push(
            button(style::button_text(t!("nexus-get-key")))
                .padding(0)
                .style(style::link)
                .on_press(Message::Open(nexus::KEY_PAGE.to_owned())),
        );
        body = body.push(self.handler_row());
        section("Nexus Mods", t!("nexus-about"), body)
    }
}

/// Asks Nexus about the key in the field, on a thread.
fn check_nexus_key(key: String) -> Task<Message> {
    let asking = key.clone();
    Task::perform(
        blocking(move || shell::check_key(&key, false)),
        move |result| Message::Nexus(NexusMessage::KeyChecked(asking.clone(), result)),
    )
}

/// One download's row, for the Mods card and the Downloads page.
pub(super) fn download_row(download: &Download) -> Element<'_, Message> {
    let (bar, words) = download.progress();
    let stopping = download.stopping();
    row![
        column![
            text(&download.name).size(14),
            progress_bar(0.0..=1.0, bar).girth(6).style(style::progress),
            text(words).size(12).color(colors().dim),
        ]
        .spacing(style::SPACE_XS)
        .width(Length::Fill),
        button(style::button_text(if stopping {
            t!("nexus-stopping")
        } else {
            t!("nexus-stop")
        }))
        .padding(style::BUTTON_SMALL)
        .style(style::secondary)
        .on_press_maybe((!stopping).then_some(Message::Nexus(NexusMessage::Stop(download.id)))),
    ]
    .spacing(style::SPACE_M)
    .align_y(Alignment::Center)
    .into()
}

fn chooser_card(choosing: &Choosing) -> Element<'_, Message> {
    let about = if choosing.all {
        t!("nexus-choose-none", name = choosing.name.as_str())
    } else {
        t!("nexus-choose-several", name = choosing.name.as_str())
    };
    let mut games = column![text(about).size(13).color(colors().dim)].spacing(style::SPACE_XS);
    for (id, title) in &choosing.games {
        games = games.push(
            button(style::button_text(title))
                .width(Length::Fill)
                .padding([style::SPACE_S, style::SPACE_M])
                .style(style::secondary)
                .on_press(Message::Nexus(NexusMessage::Choose(id.clone()))),
        );
    }
    let footer = row![
        space::horizontal(),
        button(style::button_text(t!("cancel")))
            .padding(style::BUTTON)
            .style(style::secondary)
            .on_press(Message::Nexus(NexusMessage::CancelChoose)),
    ];
    frame(&t!("nexus-choose-title"), games, footer, 560.0, false)
}

/// What the status line says of a file from Nexus taken in.
pub(super) use zendo_shell::mods::added_note;

/// Downloads a file from Nexus and adds it to the game's mods, on a thread,
/// saying how far it got.
fn fetch_mod(
    id: u64,
    game: Game,
    key: String,
    link: Link,
    remember: bool,
    stop: Arc<AtomicBool>,
) -> impl Stream<Item = NexusMessage> {
    iced::stream::channel(16, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        let game_id = game.id.clone();
        thread::spawn(move || {
            let progress_sender = sender.clone();
            let mut progress = move |done, total| {
                let _ = progress_sender.unbounded_send(NexusMessage::Progress(id, done, total));
            };
            let result = shell::fetch(&game, &key, &link, remember, &stop, &mut progress);
            let _ = sender.unbounded_send(NexusMessage::Finished(id, game_id, result));
        });
        while let Some(message) = receiver.next().await {
            if output.send(message).await.is_err() {
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zendo_core::nexus::User;

    #[test]
    fn a_saved_key_is_checked_first_and_a_bad_link_says_so() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let nexus = |message| Message::Nexus(message);
        let _ = zendo.update(nexus(NexusMessage::KeyInput("good".into())));
        let _ = zendo.update(nexus(NexusMessage::SaveKey));
        let _ = zendo.update(nexus(NexusMessage::KeyChecked(
            "good".into(),
            Ok(KeyCheck::Rejected),
        )));
        assert_eq!(zendo.config.nexus_key, None);

        let _ = zendo.update(nexus(NexusMessage::SaveKey));
        let user = User {
            name: "someone".into(),
            is_premium: false,
        };
        let _ = zendo.update(nexus(NexusMessage::KeyChecked(
            "good".into(),
            Ok(KeyCheck::Accepted(user.clone())),
        )));
        assert_eq!(zendo.config.nexus_key.as_deref(), Some("good"));
        assert_eq!(zendo.nexus.user, Some(user));

        let _ = zendo.update(nexus(NexusMessage::Link("nxm://catquest/mods/x".into())));
        assert!(zendo.status.as_deref().unwrap_or("").contains("not a"));

        // Before the library is read, a link waits.
        let _ = zendo.update(nexus(NexusMessage::Link(
            "nxm://catquest/mods/1/files/2".into(),
        )));
        assert_eq!(zendo.nexus.pending.len(), 1);
    }

    #[test]
    fn a_link_for_no_known_game_asks_which_and_back_closes_it() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let link = Link::parse("nxm://catquest/mods/1/files/2").unwrap();
        let installed = vec![("epic:cat".to_owned(), "Cat Quest".to_owned())];
        let _ = zendo.update(Message::Nexus(NexusMessage::Matched(Matched {
            link,
            matched: Vec::new(),
            installed,
            name: "Cat Quest".into(),
        })));
        let choosing = zendo.nexus.choosing.as_ref().unwrap();
        assert!(choosing.all);
        let _ = zendo.update(Message::Back);
        assert!(zendo.nexus.choosing.is_none());
    }
}
