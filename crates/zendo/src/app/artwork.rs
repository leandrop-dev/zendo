//! The art picker: "Change art" on a game's wide image opens a sheet that
//! searches `SteamGridDB` for the game's title and shows the covers, or the
//! wide images, of the game chosen; a click makes one the game's own,
//! before Steam's and the stores'. "Use the automatic one" goes back.

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
use zendo_core::steamgriddb::{self, Found, Page as ImagePage};

use super::launch_options::overlay;
use super::style::{self, BAD, BOLD, DIM, SURFACE};
use super::{Message, Page, Zendo, blocking};

/// Thumbnails downloaded at once.
const WORKERS: usize = 4;
/// Games found shown to choose from.
const GAMES_SHOWN: usize = 8;

/// The picker, open over a game's page.
pub struct Picker {
    id: String,
    title: String,
    kind: Kind,
    query: String,
    games: Vec<Found>,
    game: Option<u64>,
    images: Vec<steamgriddb::Image>,
    thumbs: HashMap<u64, image::Handle>,
    page: u32,
    more: bool,
    busy: Option<&'static str>,
    note: Option<String>,
    /// Whether the game has an image of this kind chosen by hand.
    chosen: bool,
    /// Bumped with each new search or list, so late answers are dropped.
    generation: u64,
    /// The thumbnails being downloaded, a page each; dropping them stops
    /// them.
    downloading: Vec<iced::task::Handle>,
}

#[derive(Debug, Clone)]
pub enum ArtMessage {
    Open(String),
    Kind(Kind),
    Query(String),
    Search,
    Found(u64, Result<Vec<Found>, String>),
    Game(u64),
    Listed(u64, Result<ImagePage, String>),
    More,
    Thumb(u64, u64, Option<Vec<u8>>),
    Choose(u64),
    Chosen(String, Kind, Result<PathBuf, String>),
    Automatic,
    Settings,
    Close,
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Cover => "cover",
        Kind::Hero => "wide image",
    }
}

impl Zendo {
    pub(super) fn update_art(&mut self, message: ArtMessage) -> Task<Message> {
        if let ArtMessage::Open(id) = message {
            return self.open_picker(id);
        }
        if let ArtMessage::Chosen(id, kind, result) = message {
            return self.art_chosen(&id, kind, result);
        }
        let Some(picker) = self.picker.as_mut() else {
            return Task::none();
        };
        match message {
            ArtMessage::Kind(kind) if kind != picker.kind => {
                picker.kind = kind;
                picker.chosen = self.persist && covers::chosen(&picker.id, kind).is_some();
                if let Some(game) = picker.game {
                    return self.list_images(game, 0);
                }
            }
            ArtMessage::Query(query) => picker.query = query,
            ArtMessage::Search => return self.search_art(None),
            ArtMessage::Found(generation, result) if generation == picker.generation => {
                picker.busy = None;
                match result {
                    Ok(games) if games.is_empty() => {
                        picker.games.clear();
                        picker.note = Some(format!(
                            "SteamGridDB has no game called “{}”; try another title",
                            picker.query.trim()
                        ));
                    }
                    Ok(games) => {
                        let first = games[0].id;
                        picker.games = games;
                        return self.list_images(first, 0);
                    }
                    Err(err) => picker.note = Some(err),
                }
            }
            ArtMessage::Game(game) if picker.game != Some(game) => {
                return self.list_images(game, 0);
            }
            ArtMessage::More if picker.more && picker.busy.is_none() => {
                if let Some(game) = picker.game {
                    let page = picker.page + 1;
                    return self.list_images(game, page);
                }
            }
            ArtMessage::Listed(generation, result) if generation == picker.generation => {
                picker.busy = None;
                match result {
                    Ok(page) => {
                        picker.more = page.more;
                        let wanted: Vec<(u64, String)> = page
                            .images
                            .iter()
                            .map(|image| (image.id, image.thumb.clone()))
                            .collect();
                        picker.images.extend(page.images);
                        if picker.images.is_empty() {
                            picker.note = Some(format!(
                                "This game has no {} on SteamGridDB yet",
                                kind_name(picker.kind)
                            ));
                        }
                        return self.download_thumbs(wanted);
                    }
                    Err(err) => picker.note = Some(err),
                }
            }
            ArtMessage::Thumb(generation, id, Some(bytes)) if generation == picker.generation => {
                picker.thumbs.insert(id, image::Handle::from_bytes(bytes));
            }
            ArtMessage::Choose(image) if picker.busy.is_none() => {
                return self.choose_art(image);
            }
            ArtMessage::Automatic => return self.automatic_art(),
            ArtMessage::Settings => {
                self.picker = None;
                return self.navigate(Page::Settings);
            }
            ArtMessage::Close => self.picker = None,
            _ => {}
        }
        Task::none()
    }

    fn open_picker(&mut self, id: String) -> Task<Message> {
        let title = match (self.game(&id), self.store.owned(&id)) {
            (Some(game), _) => game.title.clone(),
            (None, Some(owned)) => owned.title.clone(),
            (None, None) => return Task::none(),
        };
        let chosen = self.persist && covers::chosen(&id, Kind::Cover).is_some();
        let steam = id.strip_prefix("steam:").map(str::to_owned);
        self.picker = Some(Picker {
            id,
            query: title.clone(),
            title,
            kind: Kind::Cover,
            games: Vec::new(),
            game: None,
            images: Vec::new(),
            thumbs: HashMap::new(),
            page: 0,
            more: false,
            busy: None,
            note: None,
            chosen,
            generation: 0,
            downloading: Vec::new(),
        });
        if self.config.steamgriddb_key.is_none() {
            if let Some(picker) = self.picker.as_mut() {
                picker.note =
                    Some("Searching SteamGridDB needs a key: add yours in Settings".to_owned());
            }
            return Task::none();
        }
        self.search_art(steam)
    }

    /// Searches by the title typed; a Steam game's own entry comes first.
    fn search_art(&mut self, steam: Option<String>) -> Task<Message> {
        let key = self.config.steamgriddb_key.clone();
        let Some(picker) = self.picker.as_mut() else {
            return Task::none();
        };
        let Some(key) = key else {
            return Task::none();
        };
        picker.generation += 1;
        picker.downloading.clear();
        picker.games.clear();
        picker.game = None;
        picker.images.clear();
        picker.thumbs.clear();
        picker.note = None;
        picker.busy = Some("Searching…");
        if !self.persist {
            return Task::none();
        }
        let generation = picker.generation;
        let query = picker.query.clone();
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

    /// A page of the game's images of the picker's kind; page 0 starts over.
    fn list_images(&mut self, game: u64, page: u32) -> Task<Message> {
        let key = self.config.steamgriddb_key.clone();
        let Some(picker) = self.picker.as_mut() else {
            return Task::none();
        };
        let Some(key) = key else {
            return Task::none();
        };
        picker.generation += 1;
        if page == 0 {
            picker.downloading.clear();
            picker.images.clear();
            picker.thumbs.clear();
        }
        picker.game = Some(game);
        picker.page = page;
        picker.more = false;
        picker.note = None;
        picker.busy = Some("Looking for images…");
        if !self.persist {
            return Task::none();
        }
        let generation = picker.generation;
        let kind = picker.kind;
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

    fn download_thumbs(&mut self, wanted: Vec<(u64, String)>) -> Task<Message> {
        let Some(picker) = self.picker.as_mut() else {
            return Task::none();
        };
        if wanted.is_empty() || !self.persist {
            return Task::none();
        }
        let (task, handle) =
            Task::run(thumbnails(picker.generation, wanted), |message| message).abortable();
        // A later page's thumbnails join the earlier ones still coming.
        picker.downloading.push(handle.abort_on_drop());
        task
    }

    fn choose_art(&mut self, image: u64) -> Task<Message> {
        let Some(picker) = self.picker.as_mut() else {
            return Task::none();
        };
        let Some(chosen) = picker
            .images
            .iter()
            .find(|found| found.id == image)
            .cloned()
        else {
            return Task::none();
        };
        picker.busy = Some("Saving…");
        picker.note = None;
        let (id, kind) = (picker.id.clone(), picker.kind);
        if !self.persist {
            return Task::none();
        }
        Task::perform(
            blocking({
                let id = id.clone();
                move || covers::choose(&id, kind, &chosen)
            }),
            move |result| {
                Message::Art(ArtMessage::Chosen(
                    id.clone(),
                    kind,
                    result.map_err(|err| format!("{err:#}")),
                ))
            },
        )
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
                let title = self
                    .picker
                    .take()
                    .map_or_else(|| id.to_owned(), |picker| picker.title);
                self.status = Some(format!("New {} for {title}", kind_name(kind)));
            }
            Err(err) => {
                if let Some(picker) = self.picker.as_mut() {
                    picker.busy = None;
                    picker.note = Some(err);
                }
            }
        }
        Task::none()
    }

    /// Deletes the image chosen by hand and reads the library again, which
    /// puts the one found by itself back, or downloads it.
    fn automatic_art(&mut self) -> Task<Message> {
        let Some(picker) = self.picker.as_ref() else {
            return Task::none();
        };
        let (id, kind) = (picker.id.clone(), picker.kind);
        if self.persist
            && let Err(err) = covers::forget_choice(&id, kind)
        {
            if let Some(picker) = self.picker.as_mut() {
                picker.note = Some(format!("{err:#}"));
            }
            return Task::none();
        }
        let title = self
            .picker
            .take()
            .map_or_else(|| id.clone(), |picker| picker.title);
        if self.persist {
            for owned in self.store.each_mut(&id) {
                let image = covers::cached(&id, kind);
                match kind {
                    Kind::Cover => owned.cover = image,
                    Kind::Hero => owned.hero = image,
                }
            }
        }
        let note = format!("Back to the automatic {} for {title}", kind_name(kind));
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
        self.picker.as_ref()?;
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
        match &self.picker {
            Some(picker) => overlay(page, picker_card(picker)),
            None => page,
        }
    }
}

/// Downloads thumbnails a few at a time, each sent as it comes.
fn thumbnails(generation: u64, wanted: Vec<(u64, String)>) -> impl Stream<Item = Message> {
    iced::stream::channel(16, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        let queue = Arc::new(Mutex::new(wanted.into_iter()));
        for _ in 0..WORKERS {
            let queue = Arc::clone(&queue);
            let sender = sender.clone();
            thread::spawn(move || {
                loop {
                    let next = queue.lock().ok().and_then(|mut queue| queue.next());
                    let Some((id, url)) = next else {
                        return;
                    };
                    let bytes = steamgriddb::thumbnail(&url).ok();
                    let message = Message::Art(ArtMessage::Thumb(generation, id, bytes));
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

fn picker_card(picker: &Picker) -> Element<'_, Message> {
    let tabs = row![
        button(text("Cover").size(14))
            .padding([6, 14])
            .style(style::chip(picker.kind == Kind::Cover))
            .on_press(Message::Art(ArtMessage::Kind(Kind::Cover))),
        button(text("Wide image").size(14))
            .padding([6, 14])
            .style(style::chip(picker.kind == Kind::Hero))
            .on_press(Message::Art(ArtMessage::Kind(Kind::Hero))),
    ]
    .spacing(8);
    let search = row![
        text_input("A title to search SteamGridDB for", &picker.query)
            .on_input(|query| Message::Art(ArtMessage::Query(query)))
            .on_submit(Message::Art(ArtMessage::Search))
            .size(14)
            .padding([8, 12])
            .style(style::input),
        button(text("Search").size(14))
            .padding([8, 14])
            .style(style::secondary)
            .on_press(Message::Art(ArtMessage::Search)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let mut body = column![
        row![
            text(format!("Art for {}", picker.title))
                .size(20)
                .font(BOLD),
            space::horizontal(),
            tabs,
        ]
        .align_y(Alignment::Center),
        search,
    ]
    .spacing(16);

    if !picker.games.is_empty() {
        let games = picker.games.iter().take(GAMES_SHOWN).map(|game| {
            let label = match game.year() {
                Some(year) => format!("{} ({year})", game.name),
                None => game.name.clone(),
            };
            button(text(label).size(13))
                .padding([5, 12])
                .style(style::chip(picker.game == Some(game.id)))
                .on_press(Message::Art(ArtMessage::Game(game.id)))
                .into()
        });
        body = body.push(row(games).spacing(8).wrap().vertical_spacing(8));
    }

    let thumbs = picker.images.iter().map(|found| thumb(picker, found));
    body = body.push(row(thumbs).spacing(12).wrap().vertical_spacing(12));
    if picker.more {
        body = body.push(
            button(text("Show more").size(14))
                .padding([8, 14])
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
            scrollable(body.padding(24)).height(height),
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
    let mut footer = row![].spacing(10).align_y(Alignment::Center);
    if let Some(busy) = picker.busy {
        footer = footer.push(text(busy).size(13).color(DIM));
    } else if let Some(note) = &picker.note {
        footer = footer.push(text(note).size(13).color(BAD));
    }
    footer = footer.push(space::horizontal());
    if picker
        .note
        .as_deref()
        .is_some_and(|note| note.contains("Settings"))
    {
        footer = footer.push(
            button(text("Open Settings").size(14))
                .padding([10, 18])
                .style(style::secondary)
                .on_press(Message::Art(ArtMessage::Settings)),
        );
    }
    if picker.chosen {
        footer = footer.push(
            button(text(format!("Use the automatic {}", kind_name(picker.kind))).size(14))
                .padding([10, 18])
                .style(style::secondary)
                .on_press(Message::Art(ArtMessage::Automatic)),
        );
    }
    footer = footer.push(
        button(text("Close").size(14))
            .padding([10, 18])
            .style(style::secondary)
            .on_press(Message::Art(ArtMessage::Close)),
    );

    footer.into()
}

/// One image to choose: its thumbnail, or a grey box while it comes.
fn thumb<'a>(picker: &'a Picker, found: &'a steamgriddb::Image) -> Element<'a, Message> {
    let (width, height) = match picker.kind {
        Kind::Cover => (120.0, 180.0),
        Kind::Hero => (288.0, 93.0),
    };
    let shown: Element<'_, Message> = match picker.thumbs.get(&found.id) {
        Some(handle) => image(handle.clone())
            .width(width)
            .height(height)
            .content_fit(iced::ContentFit::Cover)
            .border_radius(8.0)
            .into(),
        None => container(space::horizontal())
            .width(width)
            .height(height)
            .style(|_| container::Style {
                background: Some(SURFACE.into()),
                border: iced::Border {
                    radius: 8.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .into(),
    };
    let mut about = format!("{}x{}", found.width, found.height);
    if !found.style.is_empty() {
        about = format!("{about}, {}", found.style);
    }
    if let Some(author) = &found.author {
        about = format!("{about}, by {}", author.name);
    }
    tooltip(
        button(shown).padding(0).style(style::link).on_press_maybe(
            picker
                .busy
                .is_none()
                .then_some(Message::Art(ArtMessage::Choose(found.id))),
        ),
        container(text(about).size(12))
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
    fn asks_for_a_key_and_drops_late_answers() {
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
        let _ = zendo.update(Message::Art(ArtMessage::Open("steam:620".into())));
        let note = zendo.picker.as_ref().and_then(|picker| picker.note.clone());
        assert!(note.is_some_and(|note| note.contains("Settings")));
        let _ = zendo.update(Message::Back);
        assert!(zendo.picker.is_none());

        zendo.config.steamgriddb_key = Some("key".into());
        let _ = zendo.update(Message::Art(ArtMessage::Open("steam:620".into())));
        let generation = zendo.picker.as_ref().map(|picker| picker.generation);
        assert_eq!(
            zendo.picker.as_ref().and_then(|picker| picker.busy),
            Some("Searching…")
        );
        // An answer to an earlier search is dropped.
        let late = Found {
            id: 1,
            name: "Portal".into(),
            release_date: None,
        };
        let _ = zendo.update(Message::Art(ArtMessage::Found(0, Ok(vec![late]))));
        assert!(
            zendo
                .picker
                .as_ref()
                .is_some_and(|picker| picker.games.is_empty())
        );
        let portal = Found {
            id: 17830,
            name: "Portal 2".into(),
            release_date: Some(1_303_084_800),
        };
        let _ = zendo.update(Message::Art(ArtMessage::Found(
            generation.unwrap(),
            Ok(vec![portal]),
        )));
        let picker = zendo.picker.as_ref().unwrap();
        assert_eq!(picker.game, Some(17830));
        assert_eq!(picker.busy, Some("Looking for images…"));
        let _ = zendo.update(Message::Play);
        assert!(zendo.picker.is_some(), "Play does nothing under the picker");
    }
}
