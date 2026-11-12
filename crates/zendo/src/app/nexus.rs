//! Nexus Mods in the window: the key on the Settings page, `nxm://` links
//! arriving (from the browser, through `links.rs`), the game each one is
//! for, its download into the game's mods, and updates to mods that came
//! from Nexus. The calls are `zendo_core::nexus`'.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, column, container, progress_bar, row, space, text, text_input};
use iced::{Alignment, Element, Length, Task};
use zendo_core::Game;
use zendo_core::mods::{self, Added, Overview};
use zendo_core::nexus::{self, FileInfo, Handler, KeyCheck, Link, User};
use zendo_core::secrets::Key;

use super::launch_options::{frame, overlay};
use super::settings::{NexusThen, kept_note, section};
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

/// The newer files of a game's mods, and the mods whose file list could
/// not be asked.
type UpdateList = (Vec<(String, FileInfo)>, Vec<String>);

#[derive(Debug, Default)]
pub struct NexusState {
    key: KeyField,
    /// Whose key it is, once Nexus said.
    user: Option<User>,
    /// What opens `nxm://` links, once asked.
    handler: Option<Handler>,
    /// Links that came before the library was read.
    pending: Vec<String>,
    /// A link no single game matched: the user says which game.
    choosing: Option<Choosing>,
    pub downloads: Vec<ModDownload>,
    next: u64,
    /// Newer files for a game's mods, by game and mod.
    updates: HashMap<String, HashMap<String, FileInfo>>,
    /// Games whose mods were checked this session.
    checked: HashSet<String>,
}

#[derive(Debug, Default)]
pub(super) struct KeyField {
    pub(super) value: String,
    pub(super) shown: bool,
    /// The key Nexus is being asked about.
    pub(super) checking: Option<String>,
    pub(super) save_after_check: bool,
    pub(super) note: Option<(String, bool)>,
}

#[derive(Debug, Clone)]
struct Choosing {
    link: Link,
    /// Nexus's name for the game.
    name: String,
    games: Vec<(String, String)>,
    /// No game matched, so every installed one is offered.
    all: bool,
}

#[derive(Debug)]
pub struct ModDownload {
    id: u64,
    pub game: String,
    /// The file on Nexus.
    file: u64,
    pub name: String,
    pub done: u64,
    pub total: Option<u64>,
    stop: Arc<AtomicBool>,
}

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
    Matched(Link, Vec<(String, String)>, Vec<(String, String)>, String),
    /// Something to say on the status line.
    Note(String),
    Choose(String),
    CancelChoose,
    Progress(u64, u64, Option<u64>),
    Finished(u64, String, Result<(Added, String), String>),
    Stop(u64),
    Updates(String, Result<UpdateList, String>),
    Update(String, String),
}

impl NexusState {
    /// Says in the Settings page what became of the key once it was
    /// written, and whose key Zendo now has.
    pub(super) fn key_stored(&mut self, then: NexusThen) {
        match then {
            NexusThen::Removed => {
                self.user = None;
                self.key.note = Some((t!("key-removed"), true));
            }
            NexusThen::Saved(who, user) => {
                self.user = Some(user);
                // Updates are asked again with the new key.
                self.checked.clear();
                self.key.note = Some((t!("nexus-key-saved", who = who), true));
            }
            NexusThen::Unchecked(error) => {
                self.key.note = Some((t!("key-saved-unchecked", error = error), false));
            }
        }
    }
}

impl Zendo {
    /// After the settings are read: whose key it is, what opens links, and
    /// the links that came while the library was read.
    pub(super) fn nexus_loaded(&mut self) -> Task<Message> {
        self.nexus.key.value = self.config.nexus_key.clone().unwrap_or_default();
        if !self.persist {
            return Task::none();
        }
        let mut tasks = vec![Task::perform(blocking(nexus::handler), |handler| {
            Message::Nexus(NexusMessage::Handler(handler))
        })];
        if let Some(key) = self.config.nexus_key.clone() {
            let asking = key.clone();
            tasks.push(Task::perform(
                blocking(move || {
                    // Nexus's games, for the games it has by title; weekly.
                    let _ = nexus::refresh_games(&key);
                    nexus::check_key(&key).map_err(|err| format!("{err:#}"))
                }),
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
        self.nexus.user.as_ref().map(|user| user.is_premium)
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
            NexusMessage::Link(text) => self.link_arrived(text),
            NexusMessage::Note(note) => {
                self.status = Some(note);
                Task::none()
            }
            NexusMessage::Matched(link, matched, installed, name) => {
                if let [(id, _)] = matched.as_slice() {
                    return self.download_mod(id, link, false);
                }
                if installed.is_empty() {
                    self.status = Some(t!("nexus-no-game", name = name.as_str()));
                    return Task::none();
                }
                let all = matched.is_empty();
                self.nexus.choosing = Some(Choosing {
                    link,
                    name,
                    games: if all { installed } else { matched },
                    all,
                });
                Task::none()
            }
            NexusMessage::Choose(id) => match self.nexus.choosing.take() {
                Some(choosing) => self.download_mod(&id, choosing.link, choosing.all),
                None => Task::none(),
            },
            NexusMessage::CancelChoose => {
                self.nexus.choosing = None;
                Task::none()
            }
            NexusMessage::Progress(id, done, total) => {
                if let Some(download) = self.nexus.downloads.iter_mut().find(|d| d.id == id) {
                    download.done = done;
                    download.total = total;
                }
                Task::none()
            }
            NexusMessage::Finished(id, game, result) => self.mod_downloaded(id, game, result),
            NexusMessage::Stop(id) => {
                if let Some(download) = self.nexus.downloads.iter().find(|d| d.id == id) {
                    download.stop.store(true, Ordering::Relaxed);
                }
                Task::none()
            }
            NexusMessage::Updates(game, result) => {
                match result {
                    Ok((found, problems)) => {
                        self.nexus.updates.insert(game, found.into_iter().collect());
                        // One mod's file list failing does not hide the
                        // others' updates, but it is still said.
                        if !problems.is_empty() {
                            self.status =
                                Some(t!("nexus-updates-failed", error = problems.join("; ")));
                        }
                    }
                    Err(err) => {
                        self.status = Some(t!("nexus-updates-failed", error = err));
                    }
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
                self.nexus.key.value = value;
                self.nexus.key.note = None;
                Task::none()
            }
            NexusMessage::ToggleShown => {
                self.nexus.key.shown = !self.nexus.key.shown;
                Task::none()
            }
            NexusMessage::SaveKey => {
                let key = self.nexus.key.value.trim().to_owned();
                self.nexus.key.value.clone_from(&key);
                if key.is_empty() {
                    return self.save_setting(super::settings::Setting::NexusKey {
                        key: None,
                        then: super::settings::NexusThen::Removed,
                    });
                }
                self.nexus.key.save_after_check = true;
                self.check_nexus_key(key)
            }
            NexusMessage::CheckKey => {
                self.nexus.key.save_after_check = false;
                let key = self.nexus.key.value.trim().to_owned();
                self.check_nexus_key(key)
            }
            NexusMessage::KeyChecked(checked, result) => self.nexus_key_checked(checked, result),
            NexusMessage::Handler(handler) => {
                self.nexus.handler = Some(handler);
                Task::none()
            }
            NexusMessage::BecomeHandler => {
                if !self.persist {
                    return Task::none();
                }
                Task::perform(
                    blocking(|| {
                        let exe = std::env::current_exe()
                            .map_err(|err| t!("nexus-no-exe", error = err.to_string()))?;
                        nexus::become_handler(&exe)
                            .map(|_| exe.display().to_string())
                            .map_err(|err| format!("{err:#}"))
                    }),
                    |result| Message::Nexus(NexusMessage::BecameHandler(result)),
                )
            }
            NexusMessage::BecameHandler(result) => {
                match result {
                    Ok(exe) => {
                        self.nexus.handler = Some(Handler::Zendo);
                        self.status = Some(t!("nexus-links-open-in", exe = exe));
                    }
                    Err(err) => self.status = Some(err),
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn check_nexus_key(&mut self, key: String) -> Task<Message> {
        if key.is_empty() {
            self.nexus.key.note = Some((t!("key-paste-first"), false));
            return Task::none();
        }
        self.nexus.key.checking = Some(key.clone());
        self.nexus.key.note = None;
        if !self.persist {
            return Task::none();
        }
        let asking = key.clone();
        Task::perform(
            blocking(move || nexus::check_key(&key).map_err(|err| format!("{err:#}"))),
            move |result| Message::Nexus(NexusMessage::KeyChecked(asking.clone(), result)),
        )
    }

    fn nexus_key_checked(
        &mut self,
        key: String,
        result: Result<KeyCheck, String>,
    ) -> Task<Message> {
        match self.nexus.key.checking.as_deref() {
            // The check at start, with nothing typed: it is the stored key.
            None => {
                if let Ok(KeyCheck::Accepted(user)) = result {
                    self.nexus.user = Some(user);
                }
                return Task::none();
            }
            Some(checking) if checking == key => {}
            // An answer about a key the field has moved on from.
            Some(_) => return Task::none(),
        }
        self.nexus.key.checking = None;
        let save = std::mem::take(&mut self.nexus.key.save_after_check);
        // The check proves the key in the field. That is Zendo's key only
        // when it is the one stored, and only then does its account tell
        // whether Premium applies.
        let stored = self.config.nexus_key.as_deref() == Some(key.as_str());
        match result {
            Ok(KeyCheck::Accepted(user)) => {
                let who = signed_in(&user);
                if save {
                    return self.save_setting(super::settings::Setting::NexusKey {
                        key: Some(key),
                        then: super::settings::NexusThen::Saved(who, user),
                    });
                }
                if stored {
                    self.nexus.user = Some(user);
                    // Updates are asked again with the new key.
                    self.nexus.checked.clear();
                }
                self.nexus.key.note = Some((t!("nexus-key-accepted", who = who), true));
            }
            Ok(KeyCheck::Rejected) => {
                let note = if save {
                    t!("nexus-key-rejected-unsaved")
                } else {
                    t!("nexus-key-rejected")
                };
                self.nexus.key.note = Some((note, false));
            }
            Err(err) if save => {
                return self.save_setting(super::settings::Setting::NexusKey {
                    key: Some(key),
                    then: super::settings::NexusThen::Unchecked(err),
                });
            }
            Err(err) => self.nexus.key.note = Some((err, false)),
        }
        Task::none()
    }

    /// A link from the browser: which game it is for, asked on a thread.
    fn link_arrived(&mut self, text: String) -> Task<Message> {
        let link = match Link::parse(&text) {
            Ok(link) => link,
            Err(err) => {
                self.status = Some(format!("{err:#}"));
                return Task::none();
            }
        };
        let Some(library) = &self.library else {
            self.nexus.pending.push(text);
            return Task::none();
        };
        let Some(key) = self.config.nexus_key.clone() else {
            self.status = Some(t!("nexus-needs-key"));
            return Task::none();
        };
        if !self.persist {
            return Task::none();
        }
        let games: Vec<Game> = library
            .games
            .iter()
            .filter(|game| {
                game.details
                    .install_path
                    .as_ref()
                    .is_some_and(|dir| dir.is_dir())
            })
            .cloned()
            .collect();
        self.status = Some(t!("nexus-mod-arrived"));
        Task::perform(
            blocking({
                let domain = link.domain.clone();
                move || {
                    // Nexus's list of games, to match titles, at most weekly.
                    let _ = nexus::refresh_games(&key);
                    let name = nexus::game_name(&domain, &nexus::cached_games());
                    let pair = |game: &Game| (game.id.clone(), game.title.clone());
                    let matched = games
                        .iter()
                        .filter(|game| mods::nexus_domain(game).as_deref() == Some(&domain))
                        .map(pair)
                        .collect();
                    let installed = games.iter().map(pair).collect();
                    Ok((matched, installed, name))
                }
            }),
            move |found: Result<_, String>| match found {
                Ok((matched, installed, name)) => Message::Nexus(NexusMessage::Matched(
                    link.clone(),
                    matched,
                    installed,
                    name,
                )),
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
        if self
            .nexus
            .downloads
            .iter()
            .any(|download| download.game == id && download.file == link.file_id)
        {
            return Task::none();
        }
        self.nexus.next += 1;
        let number = self.nexus.next;
        let stop = Arc::new(AtomicBool::new(false));
        self.nexus.downloads.push(ModDownload {
            id: number,
            game: id.to_owned(),
            file: link.file_id,
            name: t!(
                "nexus-download-name",
                title = game.title.as_str(),
                file = link.file_id
            ),
            done: 0,
            total: None,
            stop: Arc::clone(&stop),
        });
        self.status = Some(t!("nexus-downloading", title = game.title.as_str()));
        if !self.persist {
            return Task::none();
        }
        Task::run(
            fetch_mod(number, game, key, link, remember, stop),
            Message::Nexus,
        )
    }

    fn mod_downloaded(
        &mut self,
        id: u64,
        game: String,
        result: Result<(Added, String), String>,
    ) -> Task<Message> {
        let stopped = self
            .nexus
            .downloads
            .iter()
            .find(|download| download.id == id)
            .is_some_and(|download| download.stop.load(Ordering::Relaxed));
        self.nexus.downloads.retain(|download| download.id != id);
        match result {
            Ok((Added::Choose(pending), _)) => self.open_installer(game, pending),
            Ok((Added::Done(overview), note)) => {
                // An update found before is done once its file is in.
                if let Some(updates) = self.nexus.updates.get_mut(&game) {
                    updates.retain(|mod_id, file| {
                        !overview.mods.iter().any(|info| {
                            &info.id == mod_id
                                && info
                                    .nexus
                                    .as_ref()
                                    .is_some_and(|nexus| nexus.file_id == file.file_id)
                        })
                    });
                }
                self.status = Some(note);
                self.mods_ready(game, *overview)
            }
            Err(_) if stopped => {
                self.status = Some(t!("nexus-download-stopped"));
                Task::none()
            }
            Err(err) => {
                self.status = Some(t!("nexus-download-failed", error = err));
                Task::none()
            }
        }
    }

    /// Asks Nexus, once a session, for newer files of a game's mods.
    pub(super) fn check_mod_updates(&mut self, game: &str, overview: &Overview) -> Task<Message> {
        let Some(key) = self.config.nexus_key.clone() else {
            return Task::none();
        };
        let sources: Vec<(String, mods::NexusSource)> = overview
            .mods
            .iter()
            .filter_map(|info| info.nexus.clone().map(|nexus| (info.id.clone(), nexus)))
            .collect();
        if sources.is_empty() || !self.persist || !self.nexus.checked.insert(game.to_owned()) {
            return Task::none();
        }
        let game = game.to_owned();
        Task::perform(
            blocking(move || {
                let mut found = Vec::new();
                let mut problems = Vec::new();
                let mut asked: HashMap<(String, u64), nexus::Files> = HashMap::new();
                for (mod_id, source) in sources {
                    let wanted = (source.domain.clone(), source.mod_id);
                    let files = match asked.entry(wanted) {
                        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            match nexus::files(&key, &source.domain, source.mod_id) {
                                Ok(files) => entry.insert(files),
                                Err(err) => {
                                    // One mod's file list failing is not
                                    // the whole game's.
                                    problems.push(format!("{}: {err:#}", source.domain));
                                    continue;
                                }
                            }
                        }
                    };
                    if let Some(newer) = files.newest(source.file_id) {
                        found.push((mod_id, newer.clone()));
                    }
                }
                Ok((found, problems))
            }),
            move |result| Message::Nexus(NexusMessage::Updates(game.clone(), result)),
        )
    }

    /// Update on a mod: downloaded straight away with Premium; otherwise
    /// the file's page opens, for the site's button to send it back.
    fn update_from_nexus(&mut self, game: &str, mod_id: &str) -> Task<Message> {
        let Some(file) = self
            .nexus
            .updates
            .get(game)
            .and_then(|updates| updates.get(mod_id))
            .cloned()
        else {
            return Task::none();
        };
        let Some(source) = self
            .mods_overview(game)
            .and_then(|overview| overview.mods.iter().find(|info| info.id == mod_id))
            .and_then(|info| info.nexus.clone())
        else {
            return Task::none();
        };
        let link = Link {
            domain: source.domain,
            mod_id: source.mod_id,
            file_id: file.file_id,
            key: None,
            expires: None,
        };
        if self.nexus.user.as_ref().is_some_and(|user| user.is_premium) {
            return self.download_mod(game, link, false);
        }
        self.status = Some(t!("nexus-press-mod-manager"));
        Task::done(Message::Open(link.page()))
    }

    /// The newer file Nexus has for a mod, when one was found.
    pub(super) fn mod_update(&self, game: &str, mod_id: &str) -> Option<&FileInfo> {
        self.nexus.updates.get(game)?.get(mod_id)
    }

    pub(super) fn nexus_downloads_for<'a>(
        &'a self,
        game: &'a str,
    ) -> impl Iterator<Item = &'a ModDownload> + 'a {
        self.nexus
            .downloads
            .iter()
            .filter(move |download| download.game == game)
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
        let (handler, is_zendo) = match &self.nexus.handler {
            Some(Handler::Zendo) => (t!("nexus-handler-zendo"), true),
            Some(Handler::Other(other)) => {
                (t!("nexus-handler-other", other = other.as_str()), false)
            }
            Some(Handler::OtherZendo(program)) => (
                t!("nexus-handler-other-zendo", program = program.as_str()),
                false,
            ),
            Some(Handler::Nothing) => (t!("nexus-handler-nothing"), false),
            None => (String::new(), false),
        };
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
                .on_press_maybe(
                    (!is_zendo && self.nexus.handler.is_some())
                        .then_some(Message::Nexus(NexusMessage::BecomeHandler)),
                ),
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
                body = body.push(text(signed_in(user)).size(13).color(colors().good));
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

fn signed_in(user: &User) -> String {
    if user.is_premium {
        t!("nexus-known-as-premium", name = user.name.as_str())
    } else {
        t!("nexus-known-as", name = user.name.as_str())
    }
}

/// One download's row, for the Mods card and the Downloads page.
pub(super) fn download_row(download: &ModDownload) -> Element<'_, Message> {
    let mib = |bytes: u64| {
        #[expect(
            clippy::cast_precision_loss,
            reason = "shown to one decimal; the precision lost is far below"
        )]
        let mib = bytes as f32 / (1024.0 * 1024.0);
        mib
    };
    let (bar, words) = match download.total {
        Some(total) if total > 0 => (
            mib(download.done) / mib(total).max(f32::MIN_POSITIVE),
            t!(
                "nexus-progress",
                done = format!("{:.1}", mib(download.done)),
                total = format!("{:.1}", mib(total))
            ),
        ),
        _ => (
            0.0,
            t!(
                "nexus-progress-unknown",
                done = format!("{:.1}", mib(download.done))
            ),
        ),
    };
    let stopping = download.stop.load(Ordering::Relaxed);
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

/// Downloads a file from Nexus and adds it to the game's mods, on a thread,
/// saying how far it got.
/// What the status line says of a file from Nexus taken in.
pub(super) fn added_note(updated: bool, fetched: &nexus::Fetched, title: &str) -> String {
    let version = fetched
        .file
        .version
        .as_deref()
        .filter(|version| !version.is_empty());
    let name = fetched.mod_name.as_str();
    match (updated, version) {
        (true, Some(version)) => t!("nexus-updated-to", name = name, version = version),
        (true, None) => t!("nexus-updated", name = name),
        (false, Some(version)) => t!(
            "nexus-added-version",
            name = name,
            version = version,
            title = title
        ),
        (false, None) => t!("nexus-added", name = name, title = title),
    }
}

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
            let result = (|| {
                if remember {
                    mods::set_nexus(&game, &link.domain)?;
                }
                let folder = mods::downloads_dir(&game)?;
                let fetched = nexus::fetch(&key, &link, &folder, &stop, &mut progress)?;
                let (added, updated) =
                    mods::add_from_nexus(&game, &fetched, &link.domain, link.mod_id)?;
                anyhow::Ok((added, added_note(updated, &fetched, &game.title)))
            })()
            .map_err(|err| format!("{err:#}"));
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
    fn one_mods_failed_file_list_keeps_the_others_updates() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let file = FileInfo {
            file_id: 2,
            name: "New".into(),
            version: Some("2".into()),
            file_name: "a.zip".into(),
            size_kb: None,
            category_name: None,
            uploaded_timestamp: 0,
        };
        let _ = zendo.update(Message::Nexus(NexusMessage::Updates(
            "game:1".into(),
            Ok((
                vec![("mod-a".to_owned(), file)],
                vec!["mod-b: offline".to_owned()],
            )),
        )));
        assert!(
            zendo
                .nexus
                .updates
                .get("game:1")
                .is_some_and(|updates| updates.contains_key("mod-a")),
            "the rest stays"
        );
        assert!(
            zendo
                .status
                .as_deref()
                .is_some_and(|status| status.contains("mod-b")),
            "the failure is said"
        );
    }

    #[test]
    fn a_link_for_no_known_game_asks_which() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let link = Link::parse("nxm://catquest/mods/1/files/2").unwrap();
        let installed = vec![("epic:cat".to_owned(), "Cat Quest".to_owned())];
        let _ = zendo.update(Message::Nexus(NexusMessage::Matched(
            link,
            Vec::new(),
            installed,
            "Cat Quest".into(),
        )));
        let choosing = zendo.nexus.choosing.as_ref().unwrap();
        assert!(choosing.all);
        let _ = zendo.update(Message::Back);
        assert!(zendo.nexus.choosing.is_none());
    }
}
