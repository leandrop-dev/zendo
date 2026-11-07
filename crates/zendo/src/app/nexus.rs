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
use iced::widget::{
    button, column, container, progress_bar, row, scrollable, space, text, text_input,
};
use iced::{Alignment, Element, Length, Task};
use zendo_core::Game;
use zendo_core::mods::{self, Added, Overview};
use zendo_core::nexus::{self, FileInfo, Handler, KeyCheck, Link, User};

use super::launch_options::overlay;
use super::settings::section;
use super::style::{self, BAD, BOLD, DIM, GOOD};
use super::{Message, Zendo, blocking};

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
struct KeyField {
    value: String,
    shown: bool,
    /// The key Nexus is being asked about.
    checking: Option<String>,
    save_after_check: bool,
    note: Option<(String, bool)>,
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
    KeyChecked(Result<KeyCheck, String>),
    /// What opens links, as `xdg-mime` says.
    Handler(Handler),
    BecomeHandler,
    BecameHandler(Result<String, String>),
    Link(String),
    /// The games a link's game matched, the installed ones, and Nexus's
    /// name for it.
    Matched(Link, Vec<(String, String)>, Vec<(String, String)>, String),
    Choose(String),
    CancelChoose,
    Progress(u64, u64, Option<u64>),
    Finished(u64, String, Result<(Added, String), String>),
    Stop(u64),
    Updates(String, Result<Vec<(String, FileInfo)>, String>),
    Update(String, String),
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
            tasks.push(Task::perform(
                blocking(move || {
                    // Nexus's games, for the games it has by title; weekly.
                    let _ = nexus::refresh_games(&key);
                    nexus::check_key(&key).map_err(|err| format!("{err:#}"))
                }),
                |result| Message::Nexus(NexusMessage::KeyChecked(result)),
            ));
        }
        for link in std::mem::take(&mut self.nexus.pending) {
            tasks.push(Task::done(Message::Nexus(NexusMessage::Link(link))));
        }
        Task::batch(tasks)
    }

    /// A link opened before the window: taken once the library is read.
    pub(super) fn nexus_pending(&mut self, link: String) {
        self.nexus.pending.push(link);
    }

    pub(super) fn update_nexus(&mut self, message: NexusMessage) -> Task<Message> {
        match message {
            NexusMessage::Link(text) => self.link_arrived(text),
            NexusMessage::Matched(link, matched, installed, name) => {
                if let [(id, _)] = matched.as_slice() {
                    return self.download_mod(id, link, false);
                }
                if installed.is_empty() {
                    self.status = Some(format!(
                        "A mod for {name} came from Nexus, but no installed game takes mods"
                    ));
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
                    Ok(found) => {
                        self.nexus.updates.insert(game, found.into_iter().collect());
                    }
                    Err(err) => {
                        self.status = Some(format!("Could not ask Nexus for updates: {err}"));
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
                    if self.store_nexus_key(None) {
                        self.nexus.user = None;
                        self.nexus.key.note = Some(("Key removed".to_owned(), true));
                    }
                    return Task::none();
                }
                self.nexus.key.save_after_check = true;
                self.check_nexus_key(key)
            }
            NexusMessage::CheckKey => {
                self.nexus.key.save_after_check = false;
                let key = self.nexus.key.value.trim().to_owned();
                self.check_nexus_key(key)
            }
            NexusMessage::KeyChecked(result) => self.nexus_key_checked(result),
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
                            .map_err(|err| format!("could not tell where Zendo is: {err}"))?;
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
                        self.status = Some(format!("Nexus's links now open in {exe}"));
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
            self.nexus.key.note = Some(("Paste a key first".to_owned(), false));
            return Task::none();
        }
        self.nexus.key.checking = Some(key.clone());
        self.nexus.key.note = None;
        if !self.persist {
            return Task::none();
        }
        Task::perform(
            blocking(move || nexus::check_key(&key).map_err(|err| format!("{err:#}"))),
            |result| Message::Nexus(NexusMessage::KeyChecked(result)),
        )
    }

    fn nexus_key_checked(&mut self, result: Result<KeyCheck, String>) -> Task<Message> {
        // The check at start, with nothing typed.
        let Some(key) = self.nexus.key.checking.take() else {
            if let Ok(KeyCheck::Accepted(user)) = result {
                self.nexus.user = Some(user);
            }
            return Task::none();
        };
        let save = std::mem::take(&mut self.nexus.key.save_after_check);
        match result {
            Ok(KeyCheck::Accepted(user)) => {
                let who = signed_in(&user);
                if save && self.store_nexus_key(Some(key)) {
                    self.nexus.key.note = Some((format!("Saved. {who}."), true));
                    self.nexus.user = Some(user);
                    // Updates are asked again with the new key.
                    self.nexus.checked.clear();
                } else if !save {
                    self.nexus.key.note = Some((format!("{who}; Save keeps the key."), true));
                }
            }
            Ok(KeyCheck::Rejected) => {
                self.nexus.key.note = Some((
                    format!(
                        "Nexus turned this key down{}. Copy it again from API Access on Nexus.",
                        if save { ", so it was not saved" } else { "" }
                    ),
                    false,
                ));
            }
            Err(err) if save => {
                if self.store_nexus_key(Some(key)) {
                    self.nexus.key.note = Some((format!("Saved, but not checked: {err}"), false));
                }
            }
            Err(err) => self.nexus.key.note = Some((err, false)),
        }
        Task::none()
    }

    /// Sets the key and writes the settings; on failure the old key stays.
    fn store_nexus_key(&mut self, key: Option<String>) -> bool {
        let previous = std::mem::replace(&mut self.config.nexus_key, key);
        if !self.persist {
            return true;
        }
        match self.config.save() {
            Ok(()) => true,
            Err(err) => {
                self.config.nexus_key = previous;
                self.nexus.key.note = Some((format!("{err:#}"), false));
                false
            }
        }
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
            self.status = Some(
                "A mod came from Nexus: set your Nexus key in Settings, then press the \
                 button on Nexus again"
                    .to_owned(),
            );
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
        self.status = Some("A mod came from Nexus…".to_owned());
        Task::perform(
            blocking(move || {
                // Nexus's list of games, to match titles, at most weekly.
                let _ = nexus::refresh_games(&key);
                let name = nexus::game_name(&link.domain, &nexus::cached_games());
                let pair = |game: &Game| (game.id.clone(), game.title.clone());
                let matched = games
                    .iter()
                    .filter(|game| mods::nexus_domain(game).as_deref() == Some(&link.domain))
                    .map(pair)
                    .collect();
                let installed = games.iter().map(pair).collect();
                (link, matched, installed, name)
            }),
            |(link, matched, installed, name)| {
                Message::Nexus(NexusMessage::Matched(link, matched, installed, name))
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
            name: format!("{} · file #{}", game.title, link.file_id),
            done: 0,
            total: None,
            stop: Arc::clone(&stop),
        });
        self.status = Some(format!("Downloading a mod for {} from Nexus…", game.title));
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
                self.status = Some("Download stopped".to_owned());
                Task::none()
            }
            Err(err) => {
                self.status = Some(format!("Could not download the mod: {err}"));
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
                let mut asked: HashMap<(String, u64), nexus::Files> = HashMap::new();
                for (mod_id, source) in sources {
                    let wanted = (source.domain.clone(), source.mod_id);
                    if !asked.contains_key(&wanted) {
                        let files = nexus::files(&key, &source.domain, source.mod_id)
                            .map_err(|err| format!("{err:#}"))?;
                        asked.insert(wanted.clone(), files);
                    }
                    if let Some(newer) = asked[&wanted].newest(source.file_id) {
                        found.push((mod_id, newer.clone()));
                    }
                }
                Ok(found)
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
        self.status = Some(
            "On Nexus, press Mod manager download for the new file; Zendo puts it in place \
             of the old one"
                .to_owned(),
        );
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
        let mut card = column![text("Mods from Nexus").size(18).font(BOLD)].spacing(12);
        for download in &self.nexus.downloads {
            card = card.push(download_row(download));
        }
        Some(
            container(card)
                .padding(20)
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

    pub(super) fn nexus_section(&self) -> Element<'_, Message> {
        let key = &self.nexus.key;
        let saved = self.config.nexus_key.as_deref().unwrap_or("");
        let changed = key.value.trim() != saved;
        let field = row![
            text_input("Paste your Nexus Mods personal API key", &key.value)
                .secure(!key.shown)
                .on_input(|value| Message::Nexus(NexusMessage::KeyInput(value)))
                .on_submit(Message::Nexus(NexusMessage::SaveKey))
                .padding([8, 12])
                .size(14)
                .style(style::input),
            button(text(if key.shown { "Hide" } else { "Show" }).size(14))
                .padding([8, 14])
                .style(style::secondary)
                .on_press(Message::Nexus(NexusMessage::ToggleShown)),
            button(
                text(if key.checking.is_some() {
                    "Checking…"
                } else {
                    "Check"
                })
                .size(14)
            )
            .padding([8, 14])
            .style(style::secondary)
            .on_press_maybe(
                (key.checking.is_none() && !key.value.trim().is_empty())
                    .then_some(Message::Nexus(NexusMessage::CheckKey))
            ),
            button(text("Save").size(14).font(BOLD))
                .padding([8, 18])
                .style(style::primary)
                .on_press_maybe(changed.then_some(Message::Nexus(NexusMessage::SaveKey))),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let mut body = column![field].spacing(10);
        match (&key.note, &self.nexus.user) {
            (Some((note, good)), _) => {
                body = body.push(text(note).size(13).color(if *good { GOOD } else { BAD }));
            }
            (None, _) if saved.is_empty() => {
                body = body.push(text("No key set").size(13).color(DIM));
            }
            (None, Some(user)) => {
                body = body.push(text(signed_in(user)).size(13).color(GOOD));
            }
            (None, None) => {}
        }
        body = body.push(
            button(text("Get your personal API key from API Access on Nexus Mods").size(13))
                .padding(0)
                .style(style::link)
                .on_press(Message::Open(nexus::KEY_PAGE.to_owned())),
        );
        let (handler, is_zendo) = match &self.nexus.handler {
            Some(Handler::Zendo) => ("Zendo opens Mod manager download links.".to_owned(), true),
            Some(Handler::Other(other)) => (
                format!("Mod manager download links open in {other}, not Zendo."),
                false,
            ),
            Some(Handler::Nothing) => (
                "Nothing opens Mod manager download links yet.".to_owned(),
                false,
            ),
            None => (String::new(), false),
        };
        body = body.push(
            row![
                text(handler)
                    .size(13)
                    .color(if is_zendo { GOOD } else { DIM })
                    .width(Length::Fill),
                button(text("Open them with Zendo").size(13))
                    .padding([6, 12])
                    .style(style::secondary)
                    .on_press_maybe(
                        (!is_zendo && self.nexus.handler.is_some())
                            .then_some(Message::Nexus(NexusMessage::BecomeHandler)),
                    ),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        );
        section(
            "Nexus Mods",
            "Mods from Nexus's Mod manager download buttons go straight into the game's \
             mods, and Zendo tells when they have updates. Without Premium, Nexus gives \
             files only through that button on its site. The key is kept in your \
             settings file, readable only by you.",
            body,
        )
    }
}

fn signed_in(user: &User) -> String {
    format!(
        "Nexus knows this key as {}{}",
        user.name,
        if user.is_premium {
            ", with Premium"
        } else {
            ""
        }
    )
}

/// One download's row, for the Mods card and the Downloads page.
pub(super) fn download_row(download: &ModDownload) -> Element<'_, Message> {
    let mib = |bytes: u64| {
        #[allow(clippy::cast_precision_loss)]
        let mib = bytes as f32 / (1024.0 * 1024.0);
        mib
    };
    let (bar, words) = match download.total {
        Some(total) if total > 0 => (
            mib(download.done) / mib(total).max(f32::MIN_POSITIVE),
            format!("{:.1} of {:.1} MiB", mib(download.done), mib(total)),
        ),
        _ => (0.0, format!("{:.1} MiB", mib(download.done))),
    };
    let stopping = download.stop.load(Ordering::Relaxed);
    row![
        column![
            text(&download.name).size(14),
            progress_bar(0.0..=1.0, bar).girth(6).style(style::progress),
            text(words).size(12).color(DIM),
        ]
        .spacing(4)
        .width(Length::Fill),
        button(text(if stopping { "Stopping…" } else { "Stop" }).size(13))
            .padding([4, 10])
            .style(style::secondary)
            .on_press_maybe((!stopping).then_some(Message::Nexus(NexusMessage::Stop(download.id)))),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

fn chooser_card(choosing: &Choosing) -> Element<'_, Message> {
    let about = if choosing.all {
        format!(
            "Nexus says this mod is for {}, and no installed game goes by that name. \
             Choose the game it is for; Zendo remembers it for this game's next mods.",
            choosing.name
        )
    } else {
        format!(
            "Nexus says this mod is for {}, and more than one installed game matches.",
            choosing.name
        )
    };
    let mut games = column![].spacing(4);
    for (id, title) in &choosing.games {
        games = games.push(
            button(text(title).size(14))
                .width(Length::Fill)
                .padding([8, 12])
                .style(style::secondary)
                .on_press(Message::Nexus(NexusMessage::Choose(id.clone()))),
        );
    }
    container(
        column![
            text("Which game is this mod for?").size(20).font(BOLD),
            text(about).size(13).color(DIM),
            scrollable(games).height(Length::Shrink),
            row![
                space::horizontal(),
                button(text("Cancel").size(14))
                    .padding([8, 16])
                    .style(style::secondary)
                    .on_press(Message::Nexus(NexusMessage::CancelChoose)),
            ],
        ]
        .spacing(14),
    )
    .padding(24)
    .max_width(560)
    .max_height(620)
    .style(style::card)
    .into()
}

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
            let result = (|| {
                if remember {
                    mods::set_nexus(&game, &link.domain)?;
                }
                let folder = mods::downloads_dir(&game)?;
                let fetched = nexus::fetch(&key, &link, &folder, &stop, &mut progress)?;
                let (added, updated) =
                    mods::add_from_nexus(&game, &fetched, &link.domain, link.mod_id)?;
                let version = fetched
                    .file
                    .version
                    .as_deref()
                    .filter(|version| !version.is_empty())
                    .map(|version| format!(" {version}"))
                    .unwrap_or_default();
                let note = if updated {
                    format!("Updated {} to{version}", fetched.mod_name)
                } else {
                    format!("Added {}{version} to {}", fetched.mod_name, game.title)
                };
                anyhow::Ok((added, note))
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
        let _ = zendo.update(nexus(NexusMessage::KeyChecked(Ok(KeyCheck::Rejected))));
        assert_eq!(zendo.config.nexus_key, None);

        let _ = zendo.update(nexus(NexusMessage::SaveKey));
        let user = User {
            name: "someone".into(),
            is_premium: false,
        };
        let _ = zendo.update(nexus(NexusMessage::KeyChecked(Ok(KeyCheck::Accepted(
            user.clone(),
        )))));
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
