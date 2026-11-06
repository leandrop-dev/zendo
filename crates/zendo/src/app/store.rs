//! The Store page: every game the signed-in stores say the user owns, with
//! the installed ones marked, and a page for each.

use std::collections::{HashMap, HashSet};

use iced::widget::{button, column, container, row, scrollable, space, text, text_input};
use iced::{Alignment, Element, Font, Length, Task};
use zendo_core::downloads::{self, Job};
use zendo_core::info::{self, GameInfo};
use zendo_core::owned::{self, Owned};
use zendo_core::stores::Store;

use super::accounts::Account;
use super::downloads::DownloadMessage;
use super::game::{Hero, about_card, hero, store_facts};
use super::library::{Card, GRID_PADDING, card, cards_grid};
use super::style::{self, ACCENT, BAD, BOLD, DIM, GOOD};
use super::{Message, Page, Zendo, blocking, centered, format, game_scroll_id, store_search_id};

#[derive(Debug, Default)]
pub struct StoreState {
    lists: HashMap<Store, Vec<Owned>>,
    asking: HashSet<Store>,
    errors: HashMap<Store, String>,
    pub search: String,
    source: Option<Store>,
    hide_installed: bool,
    pub selected: usize,
    pub scroll_top: f32,
    /// Sizes and versions asked of the stores, by game id; once a session.
    pub info: HashMap<String, InfoState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoState {
    Asking,
    Got(GameInfo),
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum StoreMessage {
    Cached(Vec<(Store, Vec<Owned>)>),
    Refresh,
    Fetched(Store, Result<Vec<Owned>, String>),
    Search(String),
    Source(Option<Store>),
    ToggleInstalled,
    Clicked(usize),
    Info(String, Result<GameInfo, String>),
}

/// The lists kept from last time, read on a thread.
pub fn load_cached() -> Task<Message> {
    Task::perform(
        blocking(|| {
            Store::ALL
                .into_iter()
                .map(|store| (store, owned::cached(store)))
                .collect()
        }),
        |lists| Message::Store(StoreMessage::Cached(lists)),
    )
}

impl StoreState {
    pub fn owned(&self, id: &str) -> Option<&Owned> {
        self.lists.values().flatten().find(|game| game.id == id)
    }

    pub fn all(&self) -> impl Iterator<Item = &Owned> {
        self.lists.values().flatten()
    }

    /// Every entry with this id, for an image that arrived.
    pub fn each_mut<'a>(&'a mut self, id: &'a str) -> impl Iterator<Item = &'a mut Owned> {
        self.lists
            .values_mut()
            .flatten()
            .filter(move |game| game.id == id)
    }

    pub fn has_list(&self, store: Store) -> bool {
        self.lists.contains_key(&store)
    }

    /// A signed-out store's games leave the page; the caller removes the
    /// cache.
    pub fn forget(&mut self, store: Store) {
        self.lists.remove(&store);
        self.errors.remove(&store);
    }
}

impl Zendo {
    pub(super) fn update_store(&mut self, message: StoreMessage) -> Task<Message> {
        match message {
            StoreMessage::Cached(lists) => {
                for (store, games) in lists {
                    if !games.is_empty() {
                        self.store.lists.entry(store).or_insert(games);
                    }
                }
                Task::none()
            }
            StoreMessage::Refresh => {
                let signed_in: Vec<Store> = Store::ALL
                    .into_iter()
                    .filter(|store| matches!(self.accounts.get(*store), Account::SignedIn(_)))
                    .collect();
                Task::batch(signed_in.into_iter().map(|store| self.fetch_owned(store)))
            }
            StoreMessage::Fetched(store, result) => {
                self.store.asking.remove(&store);
                match result {
                    Ok(games) => {
                        self.store.errors.remove(&store);
                        self.store.lists.insert(store, games);
                        self.download_images()
                    }
                    Err(err) => {
                        self.store.errors.insert(store, err);
                        Task::none()
                    }
                }
            }
            StoreMessage::Search(search) => {
                self.store.search = search;
                self.store_filter_changed()
            }
            StoreMessage::Source(source) => {
                self.store.source = source;
                self.store_filter_changed()
            }
            StoreMessage::ToggleInstalled => {
                self.store.hide_installed = !self.store.hide_installed;
                self.store_filter_changed()
            }
            StoreMessage::Clicked(index) => {
                self.store.selected = index;
                self.open_owned()
            }
            StoreMessage::Info(id, result) => {
                let state = match result {
                    Ok(info) => InfoState::Got(info),
                    Err(err) => InfoState::Failed(err),
                };
                self.store.info.insert(id, state);
                Task::none()
            }
        }
    }

    /// Asks the game's store, on a thread, how big it is and its latest
    /// version, unless that was asked already or Zendo is not signed in to
    /// the store.
    pub(super) fn want_info(&mut self, id: &str) -> Task<Message> {
        if !self.persist || self.store.info.contains_key(id) {
            return Task::none();
        }
        let Some((store, app)) = store_of(id) else {
            return Task::none();
        };
        if !matches!(self.accounts.get(store), Account::SignedIn(_)) {
            return Task::none();
        }
        let platform = match (self.game(id), self.store.owned(id)) {
            (Some(game), owned) => game
                .details
                .platform
                .clone()
                .or_else(|| owned.map(|owned| downloads::install_platform(owned).to_owned())),
            (None, Some(owned)) => Some(downloads::install_platform(owned).to_owned()),
            (None, None) => None,
        };
        let Some(platform) = platform else {
            return Task::none();
        };
        self.store.info.insert(id.to_owned(), InfoState::Asking);
        let id = id.to_owned();
        let app = app.to_owned();
        Task::perform(
            blocking(move || info::fetch(store, &app, &platform).map_err(|err| format!("{err:#}"))),
            move |result| Message::Store(StoreMessage::Info(id.clone(), result)),
        )
    }

    /// Asks a store for its games, unless it is being asked already.
    pub(super) fn fetch_owned(&mut self, store: Store) -> Task<Message> {
        if !self.persist || !self.store.asking.insert(store) {
            return Task::none();
        }
        Task::perform(
            blocking(move || owned::fetch(store).map_err(|err| format!("{err:#}"))),
            move |result| Message::Store(StoreMessage::Fetched(store, result)),
        )
    }

    pub(super) fn store_filter_changed(&mut self) -> Task<Message> {
        self.store.selected = 0;
        iced::widget::operation::scroll_to(
            super::grid_id(),
            iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: 0.0 },
        )
    }

    pub(super) fn open_owned(&mut self) -> Task<Message> {
        match self
            .store_visible()
            .get(self.store.selected)
            .map(|game| game.id.clone())
        {
            Some(id) => self.navigate(Page::Owned(id)),
            None => Task::none(),
        }
    }

    pub(super) fn is_installed(&self, id: &str) -> bool {
        self.game(id).is_some()
    }

    /// The games of every store not signed out, before any filter.
    fn shown(&self) -> impl Iterator<Item = &Owned> {
        self.store
            .lists
            .iter()
            .filter(|(store, _)| !matches!(self.accounts.get(**store), Account::SignedOut))
            .flat_map(|(_, games)| games)
    }

    pub(super) fn owned_count(&self) -> usize {
        self.shown().count()
    }

    /// The owned games the page shows, by title.
    pub(super) fn store_visible(&self) -> Vec<&Owned> {
        let words: Vec<String> = self
            .store
            .search
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let mut games: Vec<&Owned> = self
            .shown()
            .filter(|game| {
                self.store
                    .source
                    .is_none_or(|source| source == game.store())
            })
            .filter(|game| !self.store.hide_installed || !self.is_installed(&game.id))
            .filter(|game| {
                let title = game.title.to_lowercase();
                words.iter().all(|word| title.contains(word.as_str()))
            })
            .collect();
        games.sort_by_cached_key(|game| (game.title.to_lowercase(), game.id.clone()));
        games
    }

    pub(super) fn store_page(&self) -> Element<'_, Message> {
        let visible = self.store_visible();
        let signed_in: Vec<Store> = Store::ALL
            .into_iter()
            .filter(|store| matches!(self.accounts.get(*store), Account::SignedIn(_)))
            .collect();
        let shown: Vec<&Owned> = self.shown().collect();
        let installed = shown
            .iter()
            .filter(|game| self.is_installed(&game.id))
            .count();

        let asking: Vec<&str> = self
            .store
            .asking
            .iter()
            .map(|store| store.label())
            .collect();
        let subtitle = if asking.is_empty() {
            format!(
                "{} owned · {installed} installed",
                format::count(shown.len(), "game", "games")
            )
        } else {
            format!("Asking {} for your games…", asking.join(", "))
        };
        let header = row![
            column![
                text("Store").size(28).font(BOLD),
                text(subtitle).size(13).color(DIM),
            ]
            .spacing(2),
            space::horizontal(),
            text_input("Search your games", &self.store.search)
                .id(store_search_id())
                .on_input(|search| Message::Store(StoreMessage::Search(search)))
                .on_submit(Message::Activate)
                .padding([8, 12])
                .size(14)
                .width(280)
                .style(style::input),
            button(
                text(if asking.is_empty() {
                    "Refresh"
                } else {
                    "Asking…"
                })
                .size(14)
            )
            .padding([8, 14])
            .style(style::secondary)
            .on_press_maybe(
                (asking.is_empty() && !signed_in.is_empty())
                    .then_some(Message::Store(StoreMessage::Refresh))
            ),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .padding(iced::Padding {
            top: 20.0,
            right: GRID_PADDING,
            bottom: 8.0,
            left: GRID_PADDING,
        });

        let toolbar = self.store_toolbar(&shown, installed);

        let mut problems = column![].spacing(4).padding([0.0, GRID_PADDING]);
        for (store, err) in &self.store.errors {
            problems = problems.push(
                text(format!("{}: {err}", store.label()))
                    .size(13)
                    .color(BAD),
            );
        }

        let body: Element<'_, Message> = if signed_in.is_empty() && shown.is_empty() {
            centered(
                column![
                    text("Sign in to a store to see the games you own").size(18),
                    button(text("Go to Accounts").size(14))
                        .padding([8, 14])
                        .style(style::secondary)
                        .on_press(Message::Navigate(Page::Accounts)),
                ]
                .spacing(12)
                .align_x(Alignment::Center),
            )
        } else if visible.is_empty() && !self.store.asking.is_empty() {
            centered(text("Asking the stores for your games…").color(DIM))
        } else if visible.is_empty() {
            centered(text("Nothing here").color(DIM))
        } else {
            self.store_grid(visible)
        };
        column![header, toolbar, problems, body].spacing(4).into()
    }

    fn store_grid<'a>(&'a self, visible: Vec<&'a Owned>) -> Element<'a, Message> {
        let selected = self.store.selected.min(visible.len().saturating_sub(1));
        cards_grid(
            visible.into_iter().enumerate().map(|(index, game)| {
                card(Card {
                    title: &game.title,
                    source: game.store().source(),
                    cover: game.cover.as_deref(),
                    faded: false,
                    badge: self.is_installed(&game.id).then_some(("INSTALLED", GOOD)),
                    line: game
                        .developer
                        .clone()
                        .unwrap_or_else(|| game.platforms.join(", ")),
                    line_color: DIM,
                    selected: index == selected,
                    on_press: Message::Store(StoreMessage::Clicked(index)),
                })
            }),
            self.columns,
        )
    }

    /// A chip for each store, and one to leave the installed games out.
    fn store_toolbar(&self, shown: &[&Owned], installed: usize) -> Element<'_, Message> {
        let mut chips: Vec<Element<'_, Message>> = vec![chip(
            "All".to_owned(),
            shown.len(),
            self.store.source.is_none(),
            Message::Store(StoreMessage::Source(None)),
        )];
        for store in Store::ALL {
            let count = shown.iter().filter(|game| game.store() == store).count();
            if count > 0 {
                chips.push(chip(
                    store.source().label().to_owned(),
                    count,
                    self.store.source == Some(store),
                    Message::Store(StoreMessage::Source(Some(store))),
                ));
            }
        }
        chips.push(space::horizontal().width(16).into());
        chips.push(chip(
            "Not installed only".to_owned(),
            shown.len() - installed,
            self.store.hide_installed,
            Message::Store(StoreMessage::ToggleInstalled),
        ));
        row(chips)
            .spacing(8)
            .padding([4.0, GRID_PADDING])
            .wrap()
            .vertical_spacing(8)
            .into()
    }

    /// Installing this store game where Settings says, unless it is queued.
    pub(super) fn install_job(&self, id: &str) -> Option<Job> {
        if self.downloads.job_for(id).is_some() {
            return None;
        }
        Job::install(self.store.owned(id)?, self.config.install_dir()?)
    }

    /// Open in library, the queued job, or Install.
    fn owned_actions<'a>(&'a self, game: &'a Owned, installed: bool) -> Element<'a, Message> {
        if installed {
            row![
                button(text("Open in library").size(16).font(BOLD))
                    .padding([10, 24])
                    .style(style::primary)
                    .on_press(Message::Navigate(Page::Game(game.id.clone()))),
                space::horizontal(),
                text("Installed").size(14).color(DIM),
            ]
        } else if let Some((job, running)) = self.downloads.job_for(&game.id) {
            let (label, press) = match (running, job.paused) {
                (true, _) => ("Installing…", None),
                (false, true) => (
                    "Resume",
                    Some(Message::Download(DownloadMessage::Resume(game.id.clone()))),
                ),
                (false, false) => ("Queued", None),
            };
            row![
                button(text(label).size(16).font(BOLD))
                    .padding([10, 32])
                    .style(style::primary)
                    .on_press_maybe(press),
                button(text("See downloads").size(14))
                    .padding([10, 18])
                    .style(style::secondary)
                    .on_press(Message::Navigate(Page::Downloads)),
                space::horizontal(),
                text(format!("Into {}", job.path.display()))
                    .size(14)
                    .color(DIM),
            ]
        } else {
            let job = self.install_job(&game.id);
            let needs = match self.store.info.get(&game.id) {
                Some(InfoState::Got(info)) => info
                    .install_size
                    .map(|bytes| format!(" · needs {}", zendo_core::Size::Bytes(bytes)))
                    .unwrap_or_default(),
                _ => String::new(),
            };
            let note = match &job {
                Some(job) => format!("Installs into {}{needs}", job.path.display()),
                None => "No home folder to install into".to_owned(),
            };
            row![
                button(text("Install").size(16).font(BOLD))
                    .padding([10, 32])
                    .style(style::primary)
                    .on_press_maybe(job.map(|job| Message::Download(DownloadMessage::Queue(job)))),
                space::horizontal(),
                text(note).size(14).color(DIM),
            ]
        }
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    }

    pub(super) fn owned_page<'a>(&'a self, game: &'a Owned) -> Element<'a, Message> {
        let installed = self.is_installed(&game.id);
        let trail = row![
            button(text("← Store").size(14))
                .padding(0)
                .style(style::link)
                .on_press(Message::Navigate(Page::Store)),
            text("/").size(14).color(DIM),
            text(&game.title).size(14).color(DIM),
        ]
        .spacing(8);

        let mut badges: Vec<(String, iced::Color)> = game
            .platforms
            .iter()
            .map(|platform| (platform.clone(), DIM))
            .collect();
        if installed {
            badges.push(("INSTALLED".to_owned(), GOOD));
        }
        let art = hero(Hero {
            title: &game.title,
            source: game.store().source(),
            art: game.hero.as_deref(),
            cover: game.cover.as_deref(),
            badges,
            developer: game.developer.as_deref(),
            facts: Vec::new(),
        });

        let actions = self.owned_actions(game, installed);

        let detail = |label: &'static str, value: String, code: bool| {
            row![
                text(label).size(14).color(DIM).width(170),
                if code {
                    text(value).size(13).font(Font::MONOSPACE)
                } else {
                    text(value).size(14)
                },
            ]
            .align_y(Alignment::Center)
        };
        let mut details = column![
            text("Game details").size(18).font(BOLD),
            detail("Store", game.store().label().to_owned(), false),
        ]
        .spacing(12);
        if !game.platforms.is_empty() {
            details = details.push(detail("Platforms", game.platforms.join(", "), false));
        }
        if let Some(developer) = &game.developer {
            details = details.push(detail("Developer", developer.clone(), false));
        }
        for (label, value) in store_facts(Some(game), self.store.info.get(&game.id), false) {
            details = details.push(detail(label, value, false));
        }
        details = details.push(detail("Zendo id", game.id.clone(), true));

        let mut page = column![
            trail,
            art,
            actions,
            container(details)
                .padding(20)
                .width(Length::Fill)
                .style(style::card),
        ]
        .spacing(20)
        .padding(24);
        if let Some(summary) = &game.summary {
            page = page.push(about_card(summary));
        }
        scrollable(page)
            .id(game_scroll_id())
            .height(Length::Fill)
            .into()
    }
}

/// The store a game id belongs to, and the store's own id for the game.
pub fn store_of(id: &str) -> Option<(Store, &str)> {
    let (key, app) = id.split_once(':')?;
    let store = match key {
        "epic" => Store::Epic,
        "gog" => Store::Gog,
        "amazon" => Store::Amazon,
        _ => return None,
    };
    Some((store, app))
}

fn chip<'a>(label: String, count: usize, chosen: bool, message: Message) -> Element<'a, Message> {
    button(
        row![
            text(label).size(13),
            text(count.to_string())
                .size(12)
                .color(if chosen { ACCENT } else { DIM }),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([5, 12])
    .style(style::chip(chosen))
    .on_press(message)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Move;
    use zendo_core::{Game, Launch, Library, Source};

    /// Epic and GOG signed in, Amazon signed out; Epic's first game is
    /// installed.
    fn zendo() -> Zendo {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.columns = 2;
        let owned = |store, id: &str, title: &str| Owned::new(store, id, title.to_owned());
        zendo.store.lists.insert(
            Store::Epic,
            vec![
                owned(Store::Epic, "a", "Alpha Quest"),
                owned(Store::Epic, "b", "Beta Racer"),
            ],
        );
        zendo.store.lists.insert(
            Store::Gog,
            vec![
                owned(Store::Gog, "1", "Gamma Tales"),
                owned(Store::Gog, "2", "Delta Quest"),
            ],
        );
        zendo
            .store
            .lists
            .insert(Store::Amazon, vec![owned(Store::Amazon, "x", "Omega")]);
        zendo
            .accounts
            .set(Store::Epic, Account::SignedIn("me".into()));
        zendo
            .accounts
            .set(Store::Gog, Account::SignedIn("me".into()));
        zendo.accounts.set(Store::Amazon, Account::SignedOut);
        zendo.library = Some(Library {
            games: vec![Game::new(
                "epic:a".to_owned(),
                "Alpha Quest".to_owned(),
                Source::Epic,
                Launch::Uri { uri: String::new() },
            )],
            problems: Vec::new(),
        });
        zendo.page = Page::Store;
        zendo
    }

    fn titles(zendo: &Zendo) -> Vec<&str> {
        zendo
            .store_visible()
            .iter()
            .map(|game| game.title.as_str())
            .collect()
    }

    #[test]
    fn shows_the_signed_in_stores_by_title() {
        let zendo = zendo();
        assert_eq!(
            titles(&zendo),
            ["Alpha Quest", "Beta Racer", "Delta Quest", "Gamma Tales"]
        );
        assert_eq!(zendo.owned_count(), 4, "Amazon is signed out");
        assert!(zendo.is_installed("epic:a"));
    }

    #[test]
    fn filters_by_store_search_and_installed() {
        let mut zendo = zendo();
        let _ = zendo.update(Message::Store(StoreMessage::Source(Some(Store::Gog))));
        assert_eq!(titles(&zendo), ["Delta Quest", "Gamma Tales"]);
        let _ = zendo.update(Message::Store(StoreMessage::Source(None)));
        let _ = zendo.update(Message::Type("quest".into()));
        assert_eq!(titles(&zendo), ["Alpha Quest", "Delta Quest"]);
        let _ = zendo.update(Message::Store(StoreMessage::ToggleInstalled));
        assert_eq!(titles(&zendo), ["Delta Quest"]);
        let _ = zendo.update(Message::Back);
        assert_eq!(zendo.store.search, "", "Esc clears the search first");
        assert_eq!(zendo.page, Page::Store);
    }

    #[test]
    fn the_store_keeps_a_cursor_of_its_own() {
        let mut zendo = zendo();
        let _ = zendo.update(Message::Move(Move::Down));
        let _ = zendo.update(Message::Move(Move::Right));
        assert_eq!((zendo.store.selected, zendo.selected), (3, 0));
        let _ = zendo.update(Message::Activate);
        assert_eq!(zendo.page, Page::Owned("gog:1".to_owned()));
        let _ = zendo.update(Message::Back);
        assert_eq!(zendo.page, Page::Store);
        assert_eq!(zendo.store.selected, 3, "the cursor is where it was");
    }

    #[test]
    fn signing_out_forgets_the_list() {
        let mut zendo = zendo();
        let _ = zendo.update(Message::Account(super::super::AccountMessage::SignedOut(
            Store::Epic,
            Ok(()),
        )));
        assert_eq!(titles(&zendo), ["Delta Quest", "Gamma Tales"]);
    }
}
