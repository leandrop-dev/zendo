//! The art picker: "Change art" on a game's wide image opens a sheet that
//! searches `SteamGridDB` for the game's title and shows the covers, or the
//! wide images, of the game chosen; a click makes one the game's own,
//! before Steam's and the stores'. "Use the automatic one" goes back.
//!
//! The rules are `zendo_shell::artwork`, shared with the Slint window; this
//! draws the sheet, runs its effects as tasks and keeps the thumbnails.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{
    button, column, container, image, row, scrollable, space, text, text_input, tooltip,
};
use iced::{Alignment, Element, Length, Task};
use zendo_core::covers::{self, Kind};
use zendo_core::steamgriddb::{self, Found};
use zendo_shell::artwork::{self as shared, Art, Effect, GAMES_SHOWN, Picker, WORKERS};

use super::launch_options::overlay;
use super::style::{self, colors};
use super::{Message, Page, Zendo, blocking};
use crate::i18n::t;

/// The picker and the thumbnails it shows.
pub struct Pictures {
    art: Art,
    thumbs: HashMap<u64, image::Handle>,
    /// The thumbnails being downloaded, a page each; dropping them stops
    /// them.
    downloading: Vec<iced::task::Handle>,
}

impl Pictures {
    pub fn new(persist: bool) -> Self {
        Self {
            art: Art::new(persist),
            thumbs: HashMap::new(),
            downloading: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ArtMessage {
    /// The picker, open on covers.
    Open(String),
    /// The picker, open on the wide images.
    OpenHero(String),
    Kind(Kind),
    Query(String),
    Search,
    Found(u64, Result<Vec<Found>, String>),
    Game(u64),
    Listed(u64, Result<steamgriddb::Page, String>),
    More,
    Thumb(u64, u64, Option<Vec<u8>>),
    Choose(u64),
    Chosen(String, Kind, Result<PathBuf, String>),
    Automatic,
    Settings,
    Close,
}

impl Zendo {
    pub(super) fn update_art(&mut self, message: ArtMessage) -> Task<Message> {
        self.pictures.art.persist = self.persist;
        if let ArtMessage::Open(id) = message {
            return self.open_picker(&id, Kind::Cover);
        }
        if let ArtMessage::OpenHero(id) = message {
            return self.open_picker(&id, Kind::Hero);
        }
        if let ArtMessage::Chosen(id, kind, result) = message {
            return self.art_chosen(&id, kind, result);
        }
        if self.pictures.art.picker.is_none() {
            return Task::none();
        }
        let art = &mut self.pictures.art;
        let effect = match message {
            ArtMessage::Kind(kind) => art.set_kind(kind),
            ArtMessage::Query(query) => {
                art.set_query(query);
                None
            }
            ArtMessage::Search => art.search_typed(),
            ArtMessage::Found(generation, result) => art.found(generation, result),
            ArtMessage::Game(game) => art.pick_game(game),
            ArtMessage::More => art.more(),
            ArtMessage::Listed(generation, result) => art.listed(generation, result),
            ArtMessage::Thumb(generation, id, Some(bytes)) if art.wants(generation) => {
                self.pictures
                    .thumbs
                    .insert(id, image::Handle::from_bytes(bytes));
                None
            }
            ArtMessage::Choose(image) => art.choose(image),
            ArtMessage::Automatic => return self.automatic_art(),
            ArtMessage::Settings => {
                art.close();
                // The key is what the picker is asking for.
                self.settings_group = super::settings::Group::Keys;
                return self.navigate(Page::Settings);
            }
            ArtMessage::Close => {
                art.close();
                None
            }
            _ => None,
        };
        self.run_art(effect)
    }

    fn open_picker(&mut self, id: &str, kind: Kind) -> Task<Message> {
        let title = match (self.game(id), self.store.owned(id)) {
            (Some(game), _) => game.title.clone(),
            (None, Some(owned)) => owned.title.clone(),
            (None, None) => return Task::none(),
        };
        let key = self.config.steamgriddb_key.clone();
        self.pictures.thumbs.clear();
        self.pictures.downloading.clear();
        let effect = self.pictures.art.open(id, &title, kind, key);
        self.run_art(effect)
    }

    /// Starts what the picker asked for.
    fn run_art(&mut self, effect: Option<Effect>) -> Task<Message> {
        match effect {
            None => Task::none(),
            Some(Effect::Search {
                generation,
                key,
                query,
                steam,
            }) => {
                self.pictures.downloading.clear();
                self.pictures.thumbs.clear();
                Task::perform(
                    blocking(move || {
                        let mut found = Vec::new();
                        if let Some(app) = steam
                            && let Some(game) = steamgriddb::steam_game(&key, &app)?
                        {
                            found.push(game);
                        }
                        for game in steamgriddb::search(&key, &query)? {
                            if !found.iter().any(|kept: &Found| kept.id == game.id) {
                                found.push(game);
                            }
                        }
                        Ok(found)
                    }),
                    move |result: anyhow::Result<Vec<Found>>| {
                        Message::Art(ArtMessage::Found(
                            generation,
                            result.map_err(|err| format!("{err:#}")),
                        ))
                    },
                )
            }
            Some(Effect::List {
                generation,
                key,
                game,
                kind,
                page,
            }) => {
                if page == 0 {
                    self.pictures.downloading.clear();
                    self.pictures.thumbs.clear();
                }
                Task::perform(
                    blocking(move || steamgriddb::images(&key, game, kind, page)),
                    move |result| {
                        Message::Art(ArtMessage::Listed(
                            generation,
                            result.map_err(|err| format!("{err:#}")),
                        ))
                    },
                )
            }
            Some(Effect::Thumbs { generation, wanted }) => {
                let (task, handle) = Task::run(
                    thumbnails(wanted, move |id, bytes| {
                        Message::Art(ArtMessage::Thumb(generation, id, bytes))
                    }),
                    |message| message,
                )
                .abortable();
                // A later page's thumbnails join the earlier ones still
                // coming.
                self.pictures.downloading.push(handle.abort_on_drop());
                task
            }
            Some(Effect::Choose { id, kind, image }) => Task::perform(
                blocking({
                    let id = id.clone();
                    move || covers::choose(&id, kind, &image)
                }),
                move |result| {
                    Message::Art(ArtMessage::Chosen(
                        id.clone(),
                        kind,
                        result.map_err(|err| format!("{err:#}")),
                    ))
                },
            ),
        }
    }

    fn art_chosen(
        &mut self,
        id: &str,
        kind: Kind,
        result: Result<PathBuf, String>,
    ) -> Task<Message> {
        match result {
            Ok(path) => {
                self.set_image(id, kind, &path);
                // The picker may have been closed and opened for another
                // game while the file was written.
                let title = self
                    .pictures
                    .art
                    .take_for(id)
                    .map_or_else(|| id.to_owned(), |picker| picker.title);
                self.status = Some(Art::new_note(kind, &title));
            }
            Err(err) => self.pictures.art.choose_failed(id, err),
        }
        Task::none()
    }

    /// Deletes the image chosen by hand and reads the library again, which
    /// puts the one found by itself back, or downloads it.
    fn automatic_art(&mut self) -> Task<Message> {
        let Some(done) = self.pictures.art.automatic() else {
            return Task::none();
        };
        let (id, kind) = (done.id, done.kind);
        if self.persist {
            for owned in self.store.each_mut(&id) {
                let image = covers::cached(&id, kind);
                match kind {
                    Kind::Cover => owned.cover = image,
                    Kind::Hero => owned.hero = image,
                }
            }
        }
        let note = Art::automatic_note(kind, &done.title);
        self.status = Some(note.clone());
        if self.game(&id).is_none() {
            return Task::none();
        }
        self.local.after_scan = Some((None, note));
        self.refresh()
    }

    /// While the picker is open, Escape closes it, Enter searches, and the
    /// page's own keys and the controller do nothing.
    pub(super) fn picker_takes(&mut self, message: &Message) -> Option<Task<Message>> {
        self.pictures.art.picker.as_ref()?;
        match message {
            Message::Back => Some(self.update_art(ArtMessage::Close)),
            Message::Activate => Some(self.update_art(ArtMessage::Search)),
            Message::Move(_)
            | Message::Type(_)
            | Message::Backspace
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

    pub(super) fn with_picker<'a>(&'a self, page: Element<'a, Message>) -> Element<'a, Message> {
        match &self.pictures.art.picker {
            Some(picker) => overlay(page, picker_card(&self.pictures, picker)),
            None => page,
        }
    }
}

/// Downloads images a few at a time, each into the message `made` makes of
/// its id and bytes (`None` when it could not be had); dropped, it stops.
pub(super) fn thumbnails(
    wanted: Vec<(u64, String)>,
    made: impl Fn(u64, Option<Vec<u8>>) -> Message + Send + Sync + 'static,
) -> impl Stream<Item = Message> {
    let made = Arc::new(made);
    iced::stream::channel(16, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        let queue = Arc::new(Mutex::new(wanted.into_iter()));
        for _ in 0..WORKERS {
            let queue = Arc::clone(&queue);
            let sender = sender.clone();
            let made = Arc::clone(&made);
            thread::spawn(move || {
                loop {
                    let next = queue.lock().ok().and_then(|mut queue| queue.next());
                    let Some((id, url)) = next else {
                        return;
                    };
                    let bytes = steamgriddb::thumbnail(&url).ok();
                    let message = made(id, bytes);
                    // Closed when the picker moved on.
                    if sender.unbounded_send(message).is_err() {
                        return;
                    }
                }
            });
        }
        drop(sender);
        while let Some(message) = receiver.next().await {
            if output.send(message).await.is_err() {
                return;
            }
        }
    })
}

fn picker_card<'a>(pictures: &'a Pictures, picker: &'a Picker) -> Element<'a, Message> {
    let tabs = row![
        button(style::button_text(t!("art-cover")))
            .padding([6, 14])
            .style(style::chip(picker.kind == Kind::Cover))
            .on_press(Message::Art(ArtMessage::Kind(Kind::Cover))),
        button(style::button_text(t!("art-hero")))
            .padding([6, 14])
            .style(style::chip(picker.kind == Kind::Hero))
            .on_press(Message::Art(ArtMessage::Kind(Kind::Hero))),
    ]
    .spacing(style::SPACE_S);
    let search = row![
        text_input(&t!("art-search-placeholder"), &picker.query)
            .on_input(|query| Message::Art(ArtMessage::Query(query)))
            .on_submit(Message::Art(ArtMessage::Search))
            .size(14)
            .padding([style::SPACE_S, style::SPACE_M])
            .style(style::input),
        button(style::button_text(t!("art-search")))
            .padding(style::BUTTON)
            .style(style::secondary)
            .on_press(Message::Art(ArtMessage::Search)),
    ]
    .spacing(style::SPACE_M)
    .align_y(Alignment::Center);

    let mut body = column![
        row![
            style::sheet_title(t!("art-title", title = picker.title.as_str())),
            space::horizontal(),
            tabs,
        ]
        .align_y(Alignment::Center),
        search,
    ]
    .spacing(style::SPACE_L);

    if !picker.games.is_empty() {
        let games = picker.games.iter().take(GAMES_SHOWN).map(|game| {
            button(style::button_text(shared::game_label(game)))
                .padding(style::BUTTON_SMALL)
                .style(style::chip(picker.game == Some(game.id)))
                .on_press(Message::Art(ArtMessage::Game(game.id)))
                .into()
        });
        body = body.push(
            row(games)
                .spacing(style::SPACE_S)
                .wrap()
                .vertical_spacing(8),
        );
    }

    let thumbs = picker
        .images
        .iter()
        .map(|found| thumb(pictures, picker, found));
    body = body.push(
        row(thumbs)
            .spacing(style::SPACE_M)
            .wrap()
            .vertical_spacing(12),
    );
    if picker.more {
        body = body.push(
            button(style::button_text(t!("art-show-more")))
                .padding(style::BUTTON)
                .style(style::secondary)
                .on_press_maybe(
                    picker
                        .busy
                        .is_none()
                        .then_some(Message::Art(ArtMessage::More)),
                ),
        );
    }

    // Window-tall once there are images, so the buttons below stay in view.
    let height = if picker.images.is_empty() {
        Length::Shrink
    } else {
        Length::Fill
    };
    container(
        column![
            scrollable(body.padding(style::SPACE_XL)).height(height),
            container(picker_footer(picker)).padding([16, 24]),
        ]
        .spacing(0),
    )
    .max_width(980)
    .style(style::card)
    .into()
}

/// What the picker is doing or what went wrong, and its buttons.
fn picker_footer(picker: &Picker) -> Element<'_, Message> {
    let mut footer = row![].spacing(style::SPACE_M).align_y(Alignment::Center);
    if let Some(busy) = &picker.busy {
        footer = footer.push(text(busy).size(13).color(colors().dim));
    } else if let Some(note) = &picker.note {
        footer = footer.push(text(note).size(13).color(colors().bad));
    }
    footer = footer.push(space::horizontal());
    if picker.needs_key {
        footer = footer.push(
            button(style::button_text(t!("art-open-settings")))
                .padding(style::BUTTON)
                .style(style::secondary)
                .on_press(Message::Art(ArtMessage::Settings)),
        );
    }
    if picker.chosen {
        footer = footer.push(
            button(style::button_text(Art::automatic_label(picker.kind)))
                .padding(style::BUTTON)
                .style(style::secondary)
                .on_press(Message::Art(ArtMessage::Automatic)),
        );
    }
    footer = footer.push(
        button(style::button_text(t!("close")))
            .padding(style::BUTTON)
            .style(style::secondary)
            .on_press(Message::Art(ArtMessage::Close)),
    );

    footer.into()
}

/// One image to choose: its thumbnail, or a grey box while it comes.
fn thumb<'a>(
    pictures: &'a Pictures,
    picker: &'a Picker,
    found: &'a steamgriddb::Image,
) -> Element<'a, Message> {
    let (width, height) = match picker.kind {
        Kind::Cover => (120.0, 180.0),
        Kind::Hero => (288.0, 93.0),
    };
    let shown: Element<'_, Message> = match pictures.thumbs.get(&found.id) {
        Some(handle) => image(handle.clone())
            .width(width)
            .height(height)
            .content_fit(iced::ContentFit::Cover)
            .border_radius(8.0)
            .into(),
        None => container(space::horizontal())
            .width(width)
            .height(height)
            .style(style::placeholder)
            .into(),
    };
    tooltip(
        button(shown).padding(0).style(style::link).on_press_maybe(
            picker
                .busy
                .is_none()
                .then_some(Message::Art(ArtMessage::Choose(found.id))),
        ),
        container(text(shared::thumb_about(found)).size(12))
            .padding([4, 8])
            .style(style::card),
        tooltip::Position::Bottom,
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picker_swallows_play_and_closes_on_escape() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.library = Some(zendo_core::Library {
            games: vec![zendo_core::Game::new(
                "steam:620".into(),
                "Portal 2".into(),
                zendo_core::Source::Steam,
                zendo_core::Launch::Uri { uri: String::new() },
            )],
            problems: Vec::new(),
        });
        zendo.config.steamgriddb_key = None;
        let _ = zendo.update(Message::Art(ArtMessage::Open("steam:620".into())));
        let note = zendo
            .pictures
            .art
            .picker
            .as_ref()
            .and_then(|picker| picker.note.clone());
        assert!(note.is_some_and(|note| note.contains("Settings")));
        let _ = zendo.update(Message::Back);
        assert!(zendo.pictures.art.picker.is_none());

        zendo.config.steamgriddb_key = Some("key".into());
        let _ = zendo.update(Message::Art(ArtMessage::Open("steam:620".into())));
        let _ = zendo.update(Message::Play);
        assert!(
            zendo.pictures.art.picker.is_some(),
            "Play does nothing under the picker"
        );
    }
}
