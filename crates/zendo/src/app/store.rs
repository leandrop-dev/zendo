//! The Store page: every  the signed-in stores (and Steam, with the
//! user's Web API key) say the user owns, with the installed ones marked,
//! and a page for each. Steam games install through Steam's own dialog.

use std::collections::{HashMap, HashSet};

use iced::widget::{
    button, checkbox, column, container, progress_bar, row, space, text, text_input,
};
use iced::{Alignment, Element, Task};
use zendo_core::Sort;
use zendo_core::downloads::{self, Job};
use zendo_core::info::{self, Dlc, GameInfo};
use zendo_core::owned::{self, Owned, Shop, StoreDetails};
use zendo_core::stores::Store;

use super::accounts::Account;
use super::downloads::DownloadMessage;
use super::format;
use super::library::{Card, GRID_PADDING, Window, card, cards_grid, chip};
use super::steam_installs::{Following, SteamInstallMessage, percent, progress_line};
use super::style::{self, BOLD, colors};
use super::{Message, Page, Zendo, blocking, centered, store_search_id};
use crate::i18n::t;

#[derive(Debug, Default)]
pub struct StoreState {
    lists: HashMap<Shop, Vec<Owned>>,
    asking: HashSet<Shop>,
    errors: HashMap<Shop, String>,
    pub search: String,
    source: Option<Shop>,
    hide_installed: bool,
    sort: StoreSort,
    pub selected: usize,
    pub scroll_top: f32,
    /// Sizes and versions asked of the stores, by game id; once a session.
    pub info: HashMap<String, InfoState>,
    /// The games whose "Also install its DLC" was unticked.
    skip_dlc: HashSet<String>,
}

/// The Store grid's order: by title until another is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StoreSort(Sort);

impl Default for StoreSort {
    fn default() -> Self {
        Self(Sort::Title)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoState {
    Asking,
    Got(GameInfo),
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum StoreMessage {
    Cached(Vec<(Shop, Vec<Owned>)>),
    Refresh,
    Fetched(Shop, Result<Vec<Owned>, String>),
    Search(String),
    Source(Option<Shop>),
    ToggleInstalled,
    Sort(Sort),
    /// "Also install its DLC" ticked or unticked for a game.
    ToggleDlc(String),
    Clicked(usize),
    Info(String, Result<GameInfo, String>),
    /// What Steam's store says about a Steam game.
    SteamDetails(String, Result<Option<StoreDetails>, String>),
}

/// The lists kept from last time, read on a thread.
pub fn load_cached() -> Task<Message> {
    Task::perform(
        blocking(|| {
            Shop::ALL
                .into_iter()
                .map(|shop| (shop, owned::cached(shop)))
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

    pub fn has_list(&self, shop: impl Into<Shop>) -> bool {
        self.lists.contains_key(&shop.into())
    }

    /// A signed-out store's games leave the page; the caller removes the
    /// cache.
    pub fn forget(&mut self, shop: impl Into<Shop>) {
        let shop = shop.into();
        self.lists.remove(&shop);
        self.errors.remove(&shop);
    }
}

impl Zendo {
    pub(super) fn update_store(&mut self, message: StoreMessage) -> Task<Message> {
        match message {
            StoreMessage::Cached(lists) => {
                for (shop, games) in lists {
                    if !games.is_empty() {
                        self.store.lists.entry(shop).or_insert(games);
                    }
                }
                Task::none()
            }
            StoreMessage::Refresh => {
                let connected = self.connected();
                Task::batch(connected.into_iter().map(|shop| self.fetch_owned(shop)))
            }
            StoreMessage::Fetched(shop, result) => {
                self.store.asking.remove(&shop);
                match result {
                    Ok(games) => {
                        self.store.errors.remove(&shop);
                        self.store.lists.insert(shop, games);
                        self.download_images()
                    }
                    Err(err) => {
                        self.store.errors.insert(shop, err);
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
            StoreMessage::ToggleDlc(id) => {
                if !self.store.skip_dlc.remove(&id) {
                    self.store.skip_dlc.insert(id);
                }
                Task::none()
            }
            StoreMessage::ToggleInstalled => {
                self.store.hide_installed = !self.store.hide_installed;
                self.store_filter_changed()
            }
            StoreMessage::Sort(sort) => {
                self.store.sort = StoreSort(sort);
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
            StoreMessage::SteamDetails(id, result) => {
                let state = match result {
                    Ok(details) => {
                        if let Some(details) = details {
                            for game in self.store.each_mut(&id) {
                                details.fill(game);
                            }
                            // Kept, so the page has them on the next start.
                            if self.persist
                                && let Some(games) = self.store.lists.get(&Shop::Steam)
                            {
                                let _ = owned::save(Shop::Steam, games);
                            }
                        }
                        InfoState::Got(GameInfo::default())
                    }
                    Err(err) => InfoState::Failed(err),
                };
                self.store.info.insert(id, state);
                Task::none()
            }
        }
    }

    /// Asks Steam's store, on a thread, about a Steam game the list knows
    /// only by name; once a session.
    fn want_steam_details(&mut self, id: &str, app: &str) -> Task<Message> {
        let known = self.store.owned(id).is_none_or(|game| {
            (game.summary.is_some() || game.developer.is_some()) && game.space.is_some()
        });
        if known || self.store.info.contains_key(id) {
            return Task::none();
        }
        self.store.info.insert(id.to_owned(), InfoState::Asking);
        let id = id.to_owned();
        let app = app.to_owned();
        Task::perform(
            blocking(move || owned::steam_details(&app).map_err(|err| format!("{err:#}"))),
            move |result| Message::Store(StoreMessage::SteamDetails(id.clone(), result)),
        )
    }

    /// Asks the game's store, on a thread, how big it is and its latest
    /// version, unless that was asked already or Zendo is not signed in to
    /// the store.
    pub(super) fn want_info(&mut self, id: &str) -> Task<Message> {
        if !self.persist || self.store.info.contains_key(id) {
            return Task::none();
        }
        if let Some(app) = id.strip_prefix("steam:") {
            return self.want_steam_details(id, app);
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

    /// The stores signed in, Steam among them: the ones whose games the
    /// page shows and Refresh asks for.
    pub(super) fn connected(&self) -> Vec<Shop> {
        Shop::ALL
            .into_iter()
            .filter(|shop| self.is_connected(*shop))
            .collect()
    }

    fn is_connected(&self, shop: Shop) -> bool {
        match shop.store() {
            Some(store) => matches!(self.accounts.get(store), Account::SignedIn(_)),
            None => self.config.steam_session.is_some(),
        }
    }

    /// Asks a store for its games, unless it is being asked already.
    pub(super) fn fetch_owned(&mut self, shop: impl Into<Shop>) -> Task<Message> {
        let shop = shop.into();
        if !self.persist || !self.store.asking.insert(shop) {
            return Task::none();
        }
        let Some(store) = shop.store() else {
            // Steam: the sign-in's token, renewed when it ran out.
            if let Some(session) = self.steam_session() {
                return super::steam::fetch_steam_signed_in(session);
            }
            self.store.asking.remove(&shop);
            return Task::none();
        };
        Task::perform(
            blocking(move || owned::fetch(store).map_err(|err| format!("{err:#}"))),
            move |result| Message::Store(StoreMessage::Fetched(shop, result)),
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

    /// The games of every store not signed out (Steam's while signed in,
    /// or its old key is set), before any filter.
    fn shown(&self) -> impl Iterator<Item = &Owned> {
        self.store
            .lists
            .iter()
            .filter(|(shop, _)| match shop.store() {
                Some(store) => !matches!(self.accounts.get(store), Account::SignedOut),
                None => self.is_connected(Shop::Steam),
            })
            .flat_map(|(_, games)| games)
    }

    pub(super) fn owned_count(&self) -> usize {
        self.shown().count()
    }

    /// The owned games the page shows, in the order chosen.
    pub(super) fn store_visible(&self) -> Vec<&Owned> {
        let words: Vec<String> = self
            .store
            .search
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let mut games: Vec<&Owned> = self
            .shown()
            .filter(|game| self.store.source.is_none_or(|source| source == game.shop))
            .filter(|game| !self.store.hide_installed || !self.is_installed(&game.id))
            .filter(|game| {
                let title = game.title.to_lowercase();
                words.iter().all(|word| title.contains(word.as_str()))
            })
            .collect();
        sort_owned(&mut games, self.store.sort.0);
        games
    }

    pub(super) fn store_page(&self) -> Element<'_, Message> {
        let visible = self.store_visible();
        let signed_in = self.connected();
        let shown: Vec<&Owned> = self.shown().collect();
        let installed = shown
            .iter()
            .filter(|game| self.is_installed(&game.id))
            .count();

        let asking: Vec<&str> = self.store.asking.iter().map(|shop| shop.label()).collect();
        let subtitle = if asking.is_empty() {
            format!(
                "{} · {}",
                t!("store-owned", count = shown.len()),
                t!("store-installed", count = installed)
            )
        } else {
            t!("store-asking-stores", stores = asking.join(", "))
        };
        let header = row![
            column![
                text(t!("page-store")).size(28).font(BOLD),
                text(subtitle).size(13).color(colors().dim),
            ]
            .spacing(2),
            space::horizontal(),
            text_input(&t!("store-search"), &self.store.search)
                .id(store_search_id())
                .on_input(|search| Message::Store(StoreMessage::Search(search)))
                .on_submit(Message::Activate)
                .padding([8, 12])
                .size(14)
                .width(280)
                .style(style::input),
            button(
                text(if asking.is_empty() {
                    t!("library-refresh")
                } else {
                    t!("store-asking")
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
        for (shop, err) in &self.store.errors {
            problems = problems.push(
                text(format!("{}: {err}", shop.label()))
                    .size(13)
                    .color(colors().bad),
            );
        }

        let body: Element<'_, Message> = if signed_in.is_empty() && shown.is_empty() {
            centered(
                column![
                    text(t!("store-sign-in-first")).size(18),
                    button(text(t!("store-go-accounts")).size(14))
                        .padding([8, 14])
                        .style(style::secondary)
                        .on_press(Message::Navigate(Page::Accounts)),
                ]
                .spacing(12)
                .align_x(Alignment::Center),
            )
        } else if visible.is_empty() && !self.store.asking.is_empty() {
            centered(text(t!("store-asking-all")).color(colors().dim))
        } else if visible.is_empty() {
            centered(text(t!("store-nothing")).color(colors().dim))
        } else {
            self.store_grid(visible)
        };
        column![header, toolbar, problems, body].spacing(4).into()
    }

    fn store_grid<'a>(&'a self, visible: Vec<&'a Owned>) -> Element<'a, Message> {
        let selected = self.store.selected.min(visible.len().saturating_sub(1));
        let count = visible.len();
        let window = Window {
            scroll_top: self.store.scroll_top,
            height: self.viewport_height,
        };
        cards_grid(
            visible.into_iter().enumerate().map(|(index, game)| {
                card(Card {
                    title: &game.title,
                    source: game.source(),
                    cover: game.cover.as_deref(),
                    faded: false,
                    badge: self
                        .is_installed(&game.id)
                        .then(|| (t!("badge-installed"), colors().good)),
                    line: card_line(game),
                    line_color: colors().dim,
                    selected: index == selected,
                    on_press: Message::Store(StoreMessage::Clicked(index)),
                })
            }),
            count,
            self.columns,
            window,
        )
    }

    /// A chip for each store, and one to leave the installed games out.
    fn store_toolbar(&self, shown: &[&Owned], installed: usize) -> Element<'_, Message> {
        let mut chips: Vec<Element<'_, Message>> = vec![chip(
            t!("library-all"),
            Some(shown.len()),
            self.store.source.is_none(),
            Message::Store(StoreMessage::Source(None)),
        )];
        for shop in Shop::ALL {
            let count = shown.iter().filter(|game| game.shop == shop).count();
            if count > 0 {
                chips.push(chip(
                    shop.label().to_owned(),
                    Some(count),
                    self.store.source == Some(shop),
                    Message::Store(StoreMessage::Source(Some(shop))),
                ));
            }
        }
        chips.push(space::horizontal().width(16).into());
        chips.push(chip(
            t!("store-not-installed-only"),
            Some(shown.len() - installed),
            self.store.hide_installed,
            Message::Store(StoreMessage::ToggleInstalled),
        ));
        chips.push(space::horizontal().width(16).into());
        chips.push(
            container(text(t!("library-sort")).size(13).color(colors().dim))
                .padding([6, 0])
                .into(),
        );
        for sort in [Sort::Title, Sort::Recent, Sort::MostPlayed] {
            chips.push(chip(
                format::sort(sort),
                None,
                self.store.sort.0 == sort,
                Message::Store(StoreMessage::Sort(sort)),
            ));
        }
        row(chips)
            .spacing(8)
            .padding([4.0, GRID_PADDING])
            .wrap()
            .vertical_spacing(8)
            .into()
    }

    /// Installing this store game where Settings says, unless it is queued,
    /// with its DLC unless "Also install its DLC" was unticked.
    pub(super) fn install_job(&self, id: &str) -> Option<Job> {
        if self.downloads.job_for(id).is_some() {
            return None;
        }
        let dlc = if self.store.skip_dlc.contains(id) {
            Vec::new()
        } else {
            self.installable_dlc(id)
                .into_iter()
                .map(|dlc| dlc.id.clone())
                .collect()
        };
        Job::install(self.store.owned(id)?, self.config.install_dir()?, dlc)
    }

    /// The DLC the store says the user owns and can install for a game.
    pub(super) fn installable_dlc(&self, id: &str) -> Vec<&Dlc> {
        match self.store.info.get(id) {
            Some(InfoState::Got(info)) => info.dlc.iter().filter(|dlc| dlc.installable).collect(),
            _ => Vec::new(),
        }
    }

    /// "Also install its DLC", with how many and how big when the store
    /// says, for a game with DLC to install.
    fn dlc_box(&self, id: &str) -> Option<Element<'_, Message>> {
        let dlc = self.installable_dlc(id);
        if dlc.is_empty() {
            return None;
        }
        let bytes: u64 = dlc.iter().filter_map(|dlc| dlc.install_size).sum();
        let label = if bytes > 0 {
            t!(
                "store-with-dlc-size",
                count = dlc.len(),
                size = zendo_core::Size::Bytes(bytes).to_string()
            )
        } else {
            t!("store-with-dlc", count = dlc.len())
        };
        let id = id.to_owned();
        Some(
            checkbox(!self.store.skip_dlc.contains(&id))
                .label(label)
                .text_size(14)
                .size(16)
                .on_toggle(move |_| Message::Store(StoreMessage::ToggleDlc(id.clone())))
                .into(),
        )
    }

    /// Open in library, the queued job, or Install.
    pub(super) fn owned_actions<'a>(
        &'a self,
        game: &'a Owned,
        installed: bool,
    ) -> Element<'a, Message> {
        if installed {
            row![
                button(text(t!("store-open-in-library")).size(16).font(BOLD))
                    .padding([10, 24])
                    .style(style::primary)
                    .on_press(Message::Navigate(Page::Game(game.id.clone()))),
                space::horizontal(),
                text(t!("installed")).size(14).color(colors().dim),
            ]
        } else if let Some(app) = game.id.strip_prefix("steam:") {
            return self.steam_actions(game, app);
        } else if let Some((job, running)) = self.downloads.job_for(&game.id) {
            let (label, press) = match (running, job.paused) {
                (true, _) => (t!("installing"), None),
                (false, true) => (
                    t!("resume"),
                    Some(Message::Download(DownloadMessage::Resume(game.id.clone()))),
                ),
                (false, false) => (t!("queued"), None),
            };
            row![
                button(text(label).size(16).font(BOLD))
                    .padding([10, 32])
                    .style(style::primary)
                    .on_press_maybe(press),
                button(text(t!("see-downloads")).size(14))
                    .padding([10, 18])
                    .style(style::secondary)
                    .on_press(Message::Navigate(Page::Downloads)),
                space::horizontal(),
                text(t!("store-into", path = job.path.display().to_string()))
                    .size(14)
                    .color(colors().dim),
            ]
        } else {
            let job = self.install_job(&game.id);
            let needs = match self.store.info.get(&game.id) {
                Some(InfoState::Got(info)) => info
                    .install_size
                    .map(|bytes| zendo_core::Size::Bytes(bytes).to_string()),
                _ => None,
            };
            let note = match (&job, needs) {
                (Some(job), Some(needs)) => t!(
                    "store-installs-into-needs",
                    path = job.path.display().to_string(),
                    size = needs
                ),
                (Some(job), None) => {
                    t!("store-installs-into", path = job.path.display().to_string())
                }
                (None, _) => t!("store-no-home"),
            };
            let install = row![
                button(text(t!("install")).size(16).font(BOLD))
                    .padding([10, 32])
                    .style(style::primary)
                    .on_press_maybe(job.map(|job| Message::Download(DownloadMessage::Queue(job)))),
                space::horizontal(),
                text(note).size(14).color(colors().dim),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            return match self.dlc_box(&game.id) {
                Some(dlc) => column![install, dlc].spacing(12).into(),
                None => install.into(),
            };
        }
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    }

    /// Install with Steam, or how far Steam is with it.
    fn steam_actions<'a>(&'a self, game: &'a Owned, app: &str) -> Element<'a, Message> {
        let (label, press, note, progress) = match self.steam_installs.following(app) {
            None => (
                t!("store-install-with-steam"),
                Some(Message::SteamInstall(SteamInstallMessage::Install(
                    app.to_owned(),
                    game.title.clone(),
                ))),
                game.sizes.map_or_else(
                    || t!("store-steam-installs"),
                    |sizes| {
                        t!(
                            "store-steam-installs-needs",
                            size = zendo_core::Size::Bytes(sizes.install).to_string()
                        )
                    },
                ),
                None,
            ),
            // Steam writes nothing when its dialog is cancelled: the button
            // asks again, and Stop waiting ends the wait.
            Some(Following::Asked) => (
                t!("steam-ask-again"),
                Some(Message::SteamInstall(SteamInstallMessage::Install(
                    app.to_owned(),
                    game.title.clone(),
                ))),
                t!("steam-waiting-dialog"),
                None,
            ),
            Some(Following::Busy(state)) => (
                t!("installing"),
                None,
                progress_line(state.progress),
                Some(percent(state.progress)),
            ),
        };
        let following = self.steam_installs.following(app);
        let asked = matches!(following, Some(Following::Asked));
        let mut actions = row![
            button(text(label).size(16).font(BOLD))
                .padding([10, 32])
                .style(style::primary)
                .on_press_maybe(press),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if asked {
            actions = actions.push(
                button(text(t!("steam-stop-waiting")).size(14))
                    .padding([10, 18])
                    .style(style::secondary)
                    .on_press(Message::SteamInstall(SteamInstallMessage::Forget(
                        app.to_owned(),
                    ))),
            );
        }
        if following.is_some() {
            actions = actions.push(
                button(text(t!("see-downloads")).size(14))
                    .padding([10, 18])
                    .style(style::secondary)
                    .on_press(Message::Navigate(Page::Downloads)),
            );
        }
        actions = actions
            .push(space::horizontal())
            .push(text(note).size(14).color(colors().dim));
        match progress {
            Some(percent) => column![
                actions,
                progress_bar(0.0..=100.0, percent)
                    .girth(8)
                    .style(style::progress),
            ]
            .spacing(10)
            .into(),
            None => actions.into(),
        }
    }
}

/// The Store's order: by title, last played first, or longest played
/// first; games with nothing to go by after the rest, by title.
fn sort_owned(games: &mut [&Owned], sort: Sort) {
    games.sort_by_cached_key(|game| {
        let first = match sort {
            Sort::Title => 0,
            Sort::Recent => game.last_played.unwrap_or(0),
            Sort::MostPlayed => game.minutes.unwrap_or(0),
        };
        (
            std::cmp::Reverse(first),
            game.title.to_lowercase(),
            game.id.clone(),
        )
    });
}

/// The line under a card's title: the developer or platforms, or for a
/// Steam game the time Steam counted.
fn card_line(game: &Owned) -> String {
    if game.shared && game.minutes.is_none_or(|minutes| minutes == 0) {
        return t!("store-shared");
    }
    match game.minutes {
        Some(0) => t!("played-never"),
        Some(minutes) => t!("played-time", time = format::play_time(minutes)),
        None => game
            .developer
            .clone()
            .unwrap_or_else(|| game.platforms.join(", ")),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Move;
    use zendo_core::{Game, Launch, Library, Source};

    /// Epic and GOG signed in, Amazon signed out, no Steam key; Epic's
    /// first game is installed.
    fn zendo() -> Zendo {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.columns = 2;
        let owned = |store: Store, id: &str, title: &str| Owned::new(store, id, title.to_owned());
        zendo.store.lists.insert(
            Shop::Epic,
            vec![
                owned(Store::Epic, "a", "Alpha Quest"),
                owned(Store::Epic, "b", "Beta Racer"),
            ],
        );
        zendo.store.lists.insert(
            Shop::Gog,
            vec![
                owned(Store::Gog, "1", "Gamma Tales"),
                owned(Store::Gog, "2", "Delta Quest"),
            ],
        );
        zendo
            .store
            .lists
            .insert(Shop::Amazon, vec![owned(Store::Amazon, "x", "Omega")]);
        zendo.store.lists.insert(
            Shop::Steam,
            vec![Owned::new(Shop::Steam, "620", "Portal 2".into())],
        );
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
    fn shows_steams_games_while_signed_in() {
        let mut zendo = zendo();
        zendo.config.steam_session = Some("session".into());
        assert_eq!(zendo.owned_count(), 5);
        assert_eq!(zendo.connected(), [Shop::Steam, Shop::Epic, Shop::Gog]);
        let _ = zendo.update(Message::Store(StoreMessage::Source(Some(Shop::Steam))));
        assert_eq!(titles(&zendo), ["Portal 2"]);
        assert!(
            zendo.install_job("steam:620").is_none(),
            "Steam installs its own games"
        );
    }

    #[test]
    fn sorts_by_title_last_played_or_time_played() {
        let game = |id: &str, title: &str, last: Option<u64>, minutes: Option<u64>| {
            let mut game = Owned::new(Shop::Steam, id, title.into());
            game.last_played = last;
            game.minutes = minutes;
            game
        };
        let games = [
            game("1", "Beta", Some(100), Some(5)),
            game("2", "Alpha", None, None),
            game("3", "Gamma", Some(300), Some(1)),
        ];
        let order = |sort| {
            let mut list: Vec<&Owned> = games.iter().collect();
            sort_owned(&mut list, sort);
            list.iter()
                .map(|game| game.title.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(order(Sort::Title), ["Alpha", "Beta", "Gamma"]);
        assert_eq!(order(Sort::Recent), ["Gamma", "Beta", "Alpha"]);
        assert_eq!(order(Sort::MostPlayed), ["Beta", "Gamma", "Alpha"]);
        let mut zendo = zendo();
        assert_eq!(zendo.store.sort.0, Sort::Title);
        let _ = zendo.update(Message::Store(StoreMessage::Sort(Sort::Recent)));
        assert_eq!(zendo.store.sort.0, Sort::Recent);
    }

    #[test]
    fn filters_by_store_search_and_installed() {
        let mut zendo = zendo();
        let _ = zendo.update(Message::Store(StoreMessage::Source(Some(Shop::Gog))));
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
