//! The Store page and a store game's page in the window: what `StoreBrowser`
//! holds of the games the signed-in stores say the user owns, carried to the
//! Slint models, and what the stores are asked (their lists, a game's size
//! and latest version, Steam's page for a Steam game). A part of the
//! Library's glue (`library_ui`), which owns the covers and the accounts.
//!
//! Install, Import and the queue are `downloads_ui`'s (51b); Install with
//! Steam is `steam_installs_ui`'s (51c).

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use slint::platform::Key;
use slint::{ComponentHandle, Model, ModelNotify, ModelRc, ModelTracker, SharedString, VecModel};
use zendo_core::browse::Move;
use zendo_core::config::Config;
use zendo_core::downloads;
use zendo_core::info::{self, Dlc};
use zendo_core::owned::{self, Owned, Shop};
use zendo_core::stores::Store;
use zendo_core::{Game, Launch, Size, Source, xodus};
use zendo_shell::steam_list;
use zendo_shell::store_facts::{self, InfoState};
use zendo_shell::{format, running, t};

use super::game::OwnedPage;
use super::{Glue, to_card, with_glue};
use crate::covers::Covers;
use crate::library::{Outcome, Rows, Scroll};
use crate::store::{StoreBrowser, StoreChip, StorePick};
use crate::{
    CardRow, ChipData, DlcRow, Fact, GameData, MainWindow, OwnedBadge, Page, StoreData, Tile,
};

/// What the DLC card shows: the count, a row each, Install all, and a note.
#[derive(Default)]
pub(super) struct DlcView {
    pub(super) count: String,
    pub(super) rows: Vec<DlcRow>,
    pub(super) all: String,
    pub(super) all_on: bool,
    pub(super) note: String,
}

/// What a DLC's row has at its end: the button's words and whether it is
/// pressed once already (Uninstall asks twice), or, where Zendo cannot manage
/// the DLC, a status word and its tone (1 good, 3 dim). A DLC that comes with
/// the game has neither.
///
/// `manage` is `None` where Zendo cannot manage the DLC, else whether the
/// DLC's Uninstall was pressed once.
fn dlc_button(
    installable: bool,
    installed: bool,
    manage: Option<bool>,
) -> (String, bool, String, i32) {
    let none = String::new;
    match (installable, installed, manage) {
        (false, ..) => (none(), false, none(), 0),
        (true, true, Some(true)) => (t!("downloads-press-again"), true, none(), 0),
        (true, true, Some(false)) => (t!("action-label", action = "uninstall"), false, none(), 0),
        (true, false, Some(_)) => (t!("install"), false, none(), 0),
        (true, true, None) => (none(), false, t!("installed"), 1),
        (true, false, None) => (none(), false, t!("dlc-not-installed"), 3),
    }
}

/// The Store's grid as Slint reads it: a model of rows, each made when asked.
struct StoreGrid {
    browser: Rc<RefCell<StoreBrowser>>,
    covers: Rc<RefCell<Covers>>,
    notify: ModelNotify,
}

impl Model for StoreGrid {
    type Data = CardRow;

    fn row_count(&self) -> usize {
        let browser = self.browser.borrow();
        browser.len().div_ceil(browser.columns())
    }

    fn row_data(&self, row: usize) -> Option<CardRow> {
        let browser = self.browser.borrow();
        let columns = browser.columns();
        let first = row * columns;
        if first >= browser.len() {
            return None;
        }
        let last = (first + columns).min(browser.len());
        let mut covers = self.covers.borrow_mut();
        let cards: Vec<crate::Card> = (first..last)
            .filter_map(|place| browser.card(place))
            .map(|card| to_card(card, &mut covers))
            .collect();
        Some(CardRow {
            first: i32::try_from(first).unwrap_or(i32::MAX),
            cards: ModelRc::new(VecModel::from(cards)),
        })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}

pub(super) struct StoreState {
    pub(super) browser: Rc<RefCell<StoreBrowser>>,
    grid: Rc<StoreGrid>,
    sources: Rc<VecModel<ChipData>>,
    sorts: Rc<VecModel<ChipData>>,
    problems: Rc<VecModel<SharedString>>,
    badges: Rc<VecModel<OwnedBadge>>,
    /// What each chip does, by the id the window sends back.
    picks: RefCell<Vec<StorePick>>,
    /// The store game whose page is open.
    open: RefCell<Option<String>>,
    /// The lists kept from last time have been read: until then no store is
    /// asked for a list, or it would be asked at every start.
    lists_loaded: Cell<bool>,
    /// The first scan was read, so the settings, Steam's sign-in among them,
    /// are in.
    first_scan: Cell<bool>,
}

impl StoreState {
    pub(super) fn new(covers: &Rc<RefCell<Covers>>) -> Self {
        let browser = Rc::new(RefCell::new(StoreBrowser::new()));
        let grid = Rc::new(StoreGrid {
            browser: Rc::clone(&browser),
            covers: Rc::clone(covers),
            notify: ModelNotify::default(),
        });
        Self {
            browser,
            grid,
            sources: Rc::new(VecModel::default()),
            sorts: Rc::new(VecModel::default()),
            problems: Rc::new(VecModel::default()),
            badges: Rc::new(VecModel::default()),
            picks: RefCell::new(Vec::new()),
            open: RefCell::new(None),
            lists_loaded: Cell::new(false),
            first_scan: Cell::new(false),
        }
    }
}

fn fact(label: impl Into<String>, value: impl Into<String>) -> Fact {
    Fact {
        label: label.into().into(),
        value: value.into().into(),
        path: false,
        mono: false,
        url: "".into(),
        tone: 0,
    }
}

fn chip_data(chips: Vec<StoreChip>, picks: &mut Vec<StorePick>) -> Vec<ChipData> {
    chips
        .into_iter()
        .map(|chip| {
            let id = i32::try_from(picks.len()).unwrap_or(i32::MAX);
            picks.push(chip.pick);
            ChipData {
                id,
                label: chip.label.into(),
                count: chip.count.map_or_default(|count| count.to_string()).into(),
                chosen: chip.chosen,
            }
        })
        .collect()
}

/// A game as the cards (`ProtonDB`, requirements, achievements) ask about
/// it, for a store game that is not in the library: nothing but what the
/// store says.
fn synthesized(owned: &Owned) -> Game {
    Game::new(
        owned.id.clone(),
        owned.title.clone(),
        owned.source(),
        Launch::Uri { uri: String::new() },
    )
}

/// What a Game Pass game's page says about installing it: Zendo's own
/// install when its backend and runtime are there, the launcher's otherwise.
fn xbox_install_note(package: Option<&xodus::Package>, ready: bool, running: bool) -> String {
    let Some(package) = package else {
        return if ready && !running {
            t!("xbox-install-here")
        } else if running {
            t!("xbox-close-launcher")
        } else {
            t!("xbox-install-in-launcher")
        };
    };
    if !package.supported {
        return t!("xbox-not-available");
    }
    if running {
        return t!("xbox-close-launcher");
    }
    match (ready, package.download_bytes) {
        (true, Some(bytes)) => t!(
            "xbox-install-here-needs",
            size = Size::Bytes(bytes).to_string()
        ),
        (true, None) => t!("xbox-install-here"),
        (false, Some(bytes)) => t!(
            "xbox-install-in-launcher-needs",
            size = Size::Bytes(bytes).to_string()
        ),
        (false, None) => t!("xbox-install-in-launcher"),
    }
}

impl Glue {
    /// Hooks the page's callbacks up.
    pub(super) fn connect_store(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<StoreData>();
        data.set_rows(ModelRc::from(Rc::clone(&self.store.grid)));
        data.set_sources(ModelRc::from(Rc::clone(&self.store.sources)));
        data.set_sorts(ModelRc::from(Rc::clone(&self.store.sorts)));
        data.set_problems(ModelRc::from(Rc::clone(&self.store.problems)));
        window
            .global::<GameData>()
            .set_owned_badges(ModelRc::from(Rc::clone(&self.store.badges)));
        let glue = Rc::clone(self);
        data.on_chip(move |id| {
            let pick = usize::try_from(id)
                .ok()
                .and_then(|id| glue.store.picks.borrow().get(id).copied());
            if let Some(pick) = pick {
                let outcome = glue.store.browser.borrow_mut().pick(pick);
                glue.apply_store(outcome);
                glue.focus_page();
            }
        });
        let glue = Rc::clone(self);
        data.on_card_clicked(move |id| {
            glue.store.browser.borrow_mut().click(&id);
            glue.open_owned(&id);
            glue.focus_page();
        });
        let glue = Rc::clone(self);
        data.on_columns_changed(move |columns| {
            let columns = usize::try_from(columns).unwrap_or(1);
            let outcome = glue.store.browser.borrow_mut().set_columns(columns);
            glue.apply_store(outcome);
        });
        let glue = Rc::clone(self);
        data.on_scrolled(move |top, height| glue.store.browser.borrow_mut().scrolled(top, height));
        let glue = Rc::clone(self);
        data.on_search_edited(move |text| {
            let outcome = glue.store.browser.borrow_mut().set_search(&text);
            glue.apply_store(outcome);
        });
        let glue = Rc::clone(self);
        data.on_refresh(move || glue.refresh_stores());
        let glue = Rc::clone(self);
        window
            .global::<GameData>()
            .on_open_launcher(move || glue.open_launcher());
        let glue = Rc::clone(self);
        data.on_go_accounts(move || glue.navigate(Page::Accounts));
        let glue = Rc::clone(self);
        data.on_relabel(move || {
            glue.apply_store(Outcome {
                rows: Rows::All,
                ..Outcome::default()
            });
            glue.show_store();
            if glue.store_page_open() {
                glue.show_owned();
            }
        });
        let glue = Rc::clone(self);
        window
            .global::<GameData>()
            .on_open_library(move || glue.open_in_library());
    }

    // The lists.

    /// Reads the lists kept from last time, on a thread.
    pub(super) fn load_store_cache(self: &Rc<Self>) {
        if !self.persist {
            return;
        }
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let lists: Vec<(Shop, Vec<Owned>)> = Shop::ALL
                .into_iter()
                .map(|shop| (shop, owned::cached(shop)))
                .collect();
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.store_cached(lists));
            });
        });
    }

    pub(super) fn store_cached(self: &Rc<Self>, lists: Vec<(Shop, Vec<Owned>)>) {
        let outcome = self.store.browser.borrow_mut().cached(lists);
        self.apply_store(outcome);
        self.store.lists_loaded.set(true);
        self.store_accounts_changed();
        self.fetch_missing();
        self.download_images();
    }

    /// The library was read: which store games are in it, and, with the
    /// first scan, the settings (Steam's sign-in among them) are in.
    pub(super) fn store_scanned(self: &Rc<Self>) {
        let ids: HashSet<String> = self
            .browser
            .borrow()
            .games()
            .iter()
            .map(|game| game.id.clone())
            .collect();
        let outcome = self.store.browser.borrow_mut().set_installed(ids);
        self.apply_store(outcome);
        self.store_accounts_changed();
        if !self.store.first_scan.replace(true) {
            self.fetch_missing();
        }
    }

    /// Which stores are signed in, and whose games the page lists: those not
    /// signed out (a list kept from last time shows while the check runs),
    /// and Steam while it has a sign-in.
    pub(super) fn store_accounts_changed(&self) {
        let steam = self.steam_session().is_some();
        let mut listed = HashSet::new();
        let mut connected = Vec::new();
        // Game Pass is read from Nexus Launcher's backend, not signed in
        // through an engine: it is there when the launcher is. A window that
        // writes nothing (the tests') looks at no launcher at all.
        let xbox = self.persist && xodus::installed().is_some();
        for shop in Shop::ALL {
            let (listed_here, connected_here) = match shop {
                Shop::Xbox => (xbox, xbox),
                Shop::Steam => (steam, steam),
                engine => {
                    let store = engine.store().expect("a shop with an engine");
                    (!self.store_signed_out(store), self.store_signed_in(store))
                }
            };
            if listed_here {
                listed.insert(shop);
            }
            if connected_here {
                connected.push(shop);
            }
        }
        let outcome = self
            .store
            .browser
            .borrow_mut()
            .set_accounts(listed, connected);
        self.apply_store(outcome);
        self.show_store();
    }

    /// Asks every signed-in store that has no list yet. A list kept from last
    /// time waits for Refresh.
    pub(super) fn fetch_missing(self: &Rc<Self>) {
        if !self.store.lists_loaded.get() {
            return;
        }
        let missing: Vec<Shop> = {
            let browser = self.store.browser.borrow();
            browser
                .connected()
                .iter()
                .copied()
                .filter(|shop| !browser.has_list(*shop))
                .collect()
        };
        for shop in missing {
            self.fetch_owned(shop);
        }
    }

    pub(super) fn refresh_stores(self: &Rc<Self>) {
        let connected: Vec<Shop> = self.store.browser.borrow().connected().to_vec();
        for shop in connected {
            self.fetch_owned(shop);
        }
    }

    /// Asks a store for its games, on a thread, unless it is being asked
    /// already.
    pub(super) fn fetch_owned(self: &Rc<Self>, shop: Shop) {
        if !self.persist || !self.store.browser.borrow_mut().start_asking(shop) {
            return;
        }
        let weak = self.window.clone();
        if shop == Shop::Xbox {
            self.show_store();
            std::thread::spawn(move || {
                let result = xodus::owned().map_err(|err| format!("{err:#}"));
                let _ = weak.upgrade_in_event_loop(move |_| {
                    with_glue(|glue| match result {
                        Ok(games) => glue.store_fetched(Shop::Xbox, Ok(games), &[]),
                        Err(err) => glue.store_fetched(Shop::Xbox, Err(err), &[]),
                    });
                });
            });
            return;
        }
        if let Some(store) = shop.store() {
            self.show_store();
            std::thread::spawn(move || {
                let result = owned::fetch(store).map_err(|err| format!("{err:#}"));
                let _ = weak.upgrade_in_event_loop(move |_| {
                    with_glue(|glue| match result {
                        Ok((games, problems)) => glue.store_fetched(shop, Ok(games), &problems),
                        Err(err) => glue.store_fetched(shop, Err(err), &[]),
                    });
                });
            });
            return;
        }
        // Steam: the sign-in's token, renewed when it ran out.
        let Some(session) = self.steam_session() else {
            self.store.browser.borrow_mut().stop_asking(shop);
            return;
        };
        self.show_store();
        std::thread::spawn(move || {
            let result = steam_list::fetch_signed_in(session).map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.steam_fetched(result));
            });
        });
    }

    /// A store answered.
    pub(super) fn store_fetched(
        self: &Rc<Self>,
        shop: Shop,
        result: Result<Vec<Owned>, String>,
        problems: &[String],
    ) {
        let kept = result.is_ok();
        let outcome = self
            .store
            .browser
            .borrow_mut()
            .fetched(shop, result, problems);
        self.apply_store(outcome);
        self.show_store();
        if kept {
            self.download_images();
        }
        if kept && shop == Shop::Xbox && self.persist {
            self.ask_xbox_packages();
        }
    }

    /// Asks the Store what it would install for every Game Pass game on the
    /// list, one at a time on a thread, so a card can say its size or that
    /// this PC cannot install it, before its page is opened.
    fn ask_xbox_packages(self: &Rc<Self>) {
        let ids: Vec<(String, String)> = self
            .store
            .browser
            .borrow()
            .list(Shop::Xbox)
            .unwrap_or_default()
            .iter()
            .filter(|game| self.store.browser.borrow().package(&game.id).is_none())
            .filter_map(|game| {
                let app = game.id.strip_prefix("xbox:")?;
                Some((game.id.clone(), app.to_owned()))
            })
            .collect();
        if ids.is_empty() {
            return;
        }
        let weak = self.window.clone();
        std::thread::spawn(move || {
            for (id, app) in ids {
                let result = xodus::package(&app).map_err(|err| format!("{err:#}"));
                let done = weak.upgrade_in_event_loop(move |_| {
                    with_glue(|glue| glue.xbox_package_arrived(&id, result));
                });
                if done.is_err() {
                    break;
                }
            }
        });
    }

    /// A signed-out store's games leave the page.
    pub(super) fn store_forget(&self, shop: Shop) {
        let outcome = self.store.browser.borrow_mut().forget(shop);
        self.apply_store(outcome);
        self.store_accounts_changed();
    }

    /// A game's picture came in.
    pub(super) fn store_image(
        &self,
        id: &str,
        kind: zendo_core::covers::Kind,
        path: &std::path::Path,
    ) {
        let outcome = self.store.browser.borrow_mut().set_image(id, kind, path);
        self.apply_store(outcome);
        if self.store.open.borrow().as_deref() == Some(id) {
            self.show_owned();
        }
    }

    // What the stores say of a game.

    /// Asks the game's store, on a thread, how big it is and its latest
    /// version, unless that was asked already or Zendo is not signed in to
    /// the store; for a Steam game, asks Steam's store for its page.
    pub(super) fn want_info(&self, id: &str) {
        if !self.persist || self.store.browser.borrow().info.contains_key(id) {
            return;
        }
        if let Some(app) = id.strip_prefix("steam:") {
            self.want_steam_details(id, app);
            return;
        }
        if let Some(app) = id.strip_prefix("xbox:") {
            self.want_xbox_package(id, app);
            return;
        }
        let Some((store, app)) = owned::store_of(id) else {
            return;
        };
        if !self.store_signed_in(store) {
            return;
        }
        let platform = match (
            self.browser.borrow().game(id),
            self.store.browser.borrow().owned(id),
        ) {
            (Some(game), owned) => game
                .details
                .platform
                .clone()
                .or_else(|| owned.map(|owned| downloads::install_platform(owned).to_owned())),
            (None, Some(owned)) => Some(downloads::install_platform(owned).to_owned()),
            (None, None) => None,
        };
        let Some(platform) = platform else {
            return;
        };
        self.store
            .browser
            .borrow_mut()
            .info
            .insert(id.to_owned(), InfoState::Asking);
        let (id, app) = (id.to_owned(), app.to_owned());
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = info::fetch(store, &app, &platform).map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.info_arrived(&id, result));
            });
        });
        self.show_game();
    }

    /// Asks the Store, through the launcher's backend, what it would install
    /// for a Game Pass game: whether this PC can, and its size. Nothing is
    /// installed by Zendo; Nexus Launcher does that (item 60b).
    fn want_xbox_package(&self, id: &str, app: &str) {
        self.store
            .browser
            .borrow_mut()
            .info
            .insert(id.to_owned(), InfoState::Asking);
        let (id, app) = (id.to_owned(), app.to_owned());
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = xodus::package(&app).map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.xbox_package_arrived(&id, result));
            });
        });
        self.show_game();
    }

    /// Opens Nexus Launcher from a Game Pass game's page, where the game is
    /// installed; reaped when it ends.
    pub(super) fn open_launcher(self: &Rc<Self>) {
        if !self.persist {
            return;
        }
        match xodus::open() {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Err(err) => self.say(format!("{err:#}")),
        }
    }

    /// The Store answered about a Game Pass game's package.
    pub(super) fn xbox_package_arrived(&self, id: &str, result: Result<xodus::Package, String>) {
        let state = match result {
            Ok(package) => {
                let size = package.download_bytes;
                let outcome = self.store.browser.borrow_mut().set_package(id, package);
                self.apply_store(outcome);
                InfoState::Got(info::GameInfo {
                    download_size: size,
                    ..info::GameInfo::default()
                })
            }
            Err(err) => InfoState::Failed(err),
        };
        self.store
            .browser
            .borrow_mut()
            .info
            .insert(id.to_owned(), state);
        self.show_game();
    }

    /// Asks Steam's store, on a thread, about a Steam game the list knows
    /// only by name; once a session.
    fn want_steam_details(&self, id: &str, app: &str) {
        let known = self.store.browser.borrow().owned(id).is_none_or(|game| {
            (game.summary.is_some() || game.developer.is_some()) && game.space.is_some()
        });
        if known {
            return;
        }
        self.store
            .browser
            .borrow_mut()
            .info
            .insert(id.to_owned(), InfoState::Asking);
        let (id, app) = (id.to_owned(), app.to_owned());
        let weak = self.window.clone();
        std::thread::spawn(move || {
            // Through the shared cache, so the game page's facts and this are
            // asked once between them.
            let result = match app.parse::<u32>() {
                Ok(number) => zendo_core::steam_facts::details(number),
                Err(_) => owned::steam_details(&app),
            }
            .map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.steam_details_arrived(&id, result));
            });
        });
        self.show_game();
    }

    pub(super) fn info_arrived(&self, id: &str, result: Result<info::GameInfo, String>) {
        let state = match result {
            Ok(info) => InfoState::Got(info),
            Err(err) => InfoState::Failed(err),
        };
        self.store
            .browser
            .borrow_mut()
            .info
            .insert(id.to_owned(), state);
        self.show_game();
    }

    pub(super) fn steam_details_arrived(
        &self,
        id: &str,
        result: Result<Option<owned::StoreDetails>, String>,
    ) {
        let state = match result {
            Ok(details) => {
                if let Some(details) = details {
                    let mut browser = self.store.browser.borrow_mut();
                    // Kept, so the page has them on the next start.
                    if browser.fill_steam(id, &details)
                        && self.persist
                        && let Some(games) = browser.list(Shop::Steam)
                        && let Err(err) = owned::save(Shop::Steam, games)
                    {
                        drop(browser);
                        self.say(format!("{err:#}"));
                    }
                }
                InfoState::Got(info::GameInfo::default())
            }
            Err(err) => InfoState::Failed(err),
        };
        self.store
            .browser
            .borrow_mut()
            .info
            .insert(id.to_owned(), state);
        self.show_game();
    }

    /// The store's rows for the details card: sizes and the latest version,
    /// then what its list records. `installed` leaves out the size on disk.
    pub(super) fn store_fact_rows(&self, id: &str, installed: bool) -> Vec<Fact> {
        let browser = self.store.browser.borrow();
        store_facts::facts(browser.owned(id), browser.info.get(id), installed)
            .into_iter()
            .map(|(label, value)| fact(label, value))
            .collect()
    }

    pub(super) fn owned_of(&self, id: &str) -> Option<Owned> {
        self.store.browser.borrow().owned(id).cloned()
    }

    /// The game as the cards ask about it: the library's, or, for a store
    /// game that is not in it, what the store says.
    pub(super) fn card_game(&self, id: &str) -> Option<Game> {
        if let Some(game) = self.browser.borrow().game(id) {
            return Some(game.clone());
        }
        self.store.browser.borrow().owned(id).map(synthesized)
    }

    /// The DLC of a game: those the store says the user owns and those
    /// installed, with how many of the installable are, when the game is in
    /// the library.
    pub(super) fn dlc_rows(&self, game: &Game) -> DlcView {
        struct Line {
            id: String,
            title: String,
            installed: bool,
            installable: bool,
            size: Option<u64>,
        }
        let installed: HashSet<&str> = game.details.dlc.iter().map(|dlc| dlc.id.as_str()).collect();
        let owned: Vec<Dlc> = match self.store.browser.borrow().info.get(&game.id) {
            Some(InfoState::Got(info)) => info.dlc.clone(),
            _ => Vec::new(),
        };
        let mut lines: Vec<Line> = owned
            .iter()
            .map(|dlc| Line {
                id: dlc.id.clone(),
                title: dlc.title.clone(),
                installed: installed.contains(dlc.id.as_str()),
                installable: dlc.installable,
                size: dlc.install_size,
            })
            .collect();
        // Installed, but not in the store's list (or the list is not here).
        for dlc in &game.details.dlc {
            if !owned.iter().any(|item| item.id == dlc.id) {
                lines.push(Line {
                    id: dlc.id.clone(),
                    title: dlc.title.clone().unwrap_or_else(|| dlc.id.clone()),
                    installed: true,
                    installable: true,
                    size: None,
                });
            }
        }
        let count = if self.browser.borrow().game(&game.id).is_some() {
            let total = lines.iter().filter(|line| line.installable).count();
            let have = lines
                .iter()
                .filter(|line| line.installable && line.installed)
                .count();
            t!("dlc-count", installed = have, total = total)
        } else {
            String::new()
        };
        // Zendo installs and removes the DLC of a game it installed; for
        // one Heroic installed, Heroic does.
        // The buttons are for the game's own page; a store game's page lists
        // the DLC and leaves them to Install and Open in library.
        let on_page = self.open.borrow().as_deref() == Some(game.id.as_str());
        let manage = game.installed_by_zendo() && on_page;
        let free = manage && self.downloads.queue.borrow().job_for(&game.id).is_none();
        let confirming = self.jobs.confirm_dlc.borrow().clone();
        let to_install = lines
            .iter()
            .filter(|line| line.installable && !line.installed)
            .count();
        let rows = lines
            .into_iter()
            .map(|line| {
                let about = if line.installable {
                    line.size
                        .map_or_default(|bytes| Size::Bytes(bytes).to_string())
                } else {
                    t!("dlc-comes-with")
                };
                let asked = confirming.as_deref() == Some(line.id.as_str());
                let (label, danger, status, tone) =
                    dlc_button(line.installable, line.installed, manage.then_some(asked));
                DlcRow {
                    id: line.id.into(),
                    title: line.title.into(),
                    about: about.into(),
                    label: label.into(),
                    danger,
                    enabled: free,
                    status: status.into(),
                    status_tone: tone,
                }
            })
            .collect();
        DlcView {
            count,
            rows,
            all: if manage && to_install > 1 {
                t!("dlc-install-all")
            } else {
                String::new()
            },
            all_on: free,
            note: self.dlc_note(game, !manage && on_page),
        }
    }

    /// Why Zendo cannot manage the DLC (`unmanaged`, on the game's own
    /// page): sign in to see all of them, or Heroic does; and that the store
    /// is being asked.
    fn dlc_note(&self, game: &Game, unmanaged: bool) -> String {
        let store = match game.source {
            Source::Gog => Store::Gog,
            _ => Store::Epic,
        };
        let info = self.store.browser.borrow().info.get(&game.id).cloned();
        let mut notes = Vec::new();
        if unmanaged {
            notes.push(if info.is_none() && !self.store_signed_in(store) {
                t!("dlc-sign-in", store = store.label())
            } else {
                t!("dlc-in-heroic")
            });
        }
        if matches!(info, Some(InfoState::Asking)) {
            notes.push(t!("dlc-asking"));
        }
        notes.join("\n")
    }

    // Writing the page.

    /// Carries an outcome to the window: the rows to draw again, the scroll,
    /// and the chips when what the page lists changed.
    pub(super) fn apply_store(&self, outcome: Outcome) {
        let Outcome { rows, scroll, .. } = outcome;
        let changed = rows != Rows::Same;
        match rows {
            Rows::Same => {}
            Rows::Marks(places) => {
                let columns = self.store.browser.borrow().columns();
                for place in places {
                    self.store.grid.notify.row_changed(place / columns);
                }
            }
            Rows::All => self.store.grid.notify.reset(),
        }
        if let Some(window) = self.window.upgrade() {
            let data = window.global::<StoreData>();
            match scroll {
                Scroll::Keep => {}
                Scroll::Top => data.set_viewport_y(0.0),
                Scroll::To(y) => data.set_viewport_y(-y),
            }
            let cursor = self.store.browser.borrow().cursor();
            data.set_cursor(i32::try_from(cursor).unwrap_or(0));
        }
        if changed {
            self.show_store();
        }
    }

    /// Everything the page shows apart from the cards: the chips, what stands
    /// in for the grid, Refresh, what is being asked, and the tab's count.
    pub(super) fn show_store(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<StoreData>();
        let browser = self.store.browser.borrow();
        let chips = browser.chips();
        let mut picks = Vec::new();
        self.store
            .sources
            .set_vec(chip_data(chips.sources, &mut picks));
        self.store.sorts.set_vec(chip_data(chips.sorts, &mut picks));
        *self.store.picks.borrow_mut() = picks;
        self.store.problems.set_vec(
            browser
                .problems()
                .into_iter()
                .map(SharedString::from)
                .collect::<Vec<_>>(),
        );
        let asking = browser.asking();
        data.set_asking(
            if asking.is_empty() {
                String::new()
            } else {
                let names: Vec<&str> = asking.iter().map(|shop| shop.label()).collect();
                t!("store-asking-stores", stores = names.join(", "))
            }
            .into(),
        );
        data.set_refresh_label(
            if asking.is_empty() {
                t!("library-refresh")
            } else {
                t!("store-asking")
            }
            .into(),
        );
        data.set_refresh_on(asking.is_empty() && !browser.connected().is_empty());
        let (title, accounts) = browser.empty().map_or((String::new(), false), |empty| {
            (empty.title, empty.accounts)
        });
        data.set_empty_title(title.into());
        data.set_empty_accounts(accounts);
        data.set_cursor(i32::try_from(browser.cursor()).unwrap_or(0));
        // What Zendo shows in the search is what it filters by.
        if data.get_search() != browser.search() {
            data.set_search(browser.search().into());
        }
        window.set_store_count(browser.listed_count().to_string().into());
    }

    /// Writes the rows of the grid near the screen again, once covers came in.
    pub(super) fn show_store_covers(&self) {
        let (rows, near) = {
            let browser = self.store.browser.borrow();
            (
                browser.len().div_ceil(browser.columns()),
                browser.rows_near(),
            )
        };
        for row in near.start..near.end.min(rows) {
            self.store.grid.notify.row_changed(row);
        }
    }

    // The keyboard and the pad.

    /// Whether the Store's grid is the page on screen.
    pub(super) fn on_store(&self) -> bool {
        self.window
            .upgrade()
            .is_some_and(|window| window.get_page() == Page::Store)
            && !self.store_page_open()
    }

    pub(super) fn store_page_open(&self) -> bool {
        self.store.open.borrow().is_some()
    }

    /// The store game whose page is open.
    pub(super) fn store_open_id(&self) -> Option<String> {
        self.store.open.borrow().clone()
    }

    fn store_change(&self, change: impl FnOnce(&mut StoreBrowser) -> Outcome) -> bool {
        let outcome = change(&mut self.store.browser.borrow_mut());
        self.apply_store(outcome);
        true
    }

    pub(super) fn store_step(&self, step: Move) -> bool {
        self.store_change(|browser| browser.step(step))
    }

    pub(super) fn store_cycle_sort(&self) -> bool {
        self.store_change(StoreBrowser::cycle_sort)
    }

    /// Escape on the Store's grid: the search goes.
    pub(super) fn store_back(&self) {
        self.store_change(StoreBrowser::back);
    }

    pub(super) fn open_selected_owned(&self) -> bool {
        let id = self
            .store
            .browser
            .borrow()
            .selected()
            .map(|game| game.id.clone());
        if let Some(id) = id {
            self.open_owned(&id);
        }
        true
    }

    /// A key on the Store's grid that the search field did not use. Says
    /// whether it was taken.
    pub(super) fn store_key(self: &Rc<Self>, text: &str, key: char) -> bool {
        let arrows = [
            (Key::LeftArrow, Move::Left),
            (Key::RightArrow, Move::Right),
            (Key::UpArrow, Move::Up),
            (Key::DownArrow, Move::Down),
            (Key::PageUp, Move::PageUp),
            (Key::PageDown, Move::PageDown),
            (Key::Home, Move::First),
            (Key::End, Move::Last),
        ];
        if let Some((_, step)) = arrows.iter().find(|(arrow, _)| key == char::from(*arrow)) {
            self.store_step(*step)
        } else if key == char::from(Key::Return) {
            self.open_selected_owned()
        } else if key == char::from(Key::Backspace) {
            self.store_change(StoreBrowser::backspace)
        } else if key == char::from(Key::F5) {
            self.refresh_stores();
            true
        } else if key == char::from(Key::Tab) {
            self.store_cycle_sort()
        } else if key.is_control() || ('\u{f700}'..='\u{f8ff}').contains(&key) {
            false
        } else {
            // Typing with no field focused starts the search.
            self.store_change(|browser| browser.type_text(text))
        }
    }

    /// A key on a store game's page: Enter opens an installed game in the
    /// library.
    pub(super) fn owned_key(self: &Rc<Self>, key: char) -> bool {
        if key == char::from(Key::Return) {
            self.owned_activate();
            true
        } else {
            false
        }
    }

    // The store game's page.

    pub(super) fn open_owned(&self, id: &str) {
        if self.store.browser.borrow().owned(id).is_none() {
            return;
        }
        *self.store.open.borrow_mut() = Some(id.to_owned());
        self.game.technical.set(false);
        self.want_info(id);
        self.want_owned_cards(id);
        if let Some(window) = self.window.upgrade() {
            // A click moved the cursor to the card.
            let cursor = self.store.browser.borrow().cursor();
            window
                .global::<StoreData>()
                .set_cursor(i32::try_from(cursor).unwrap_or(0));
            let data = window.global::<GameData>();
            data.set_owned(true);
            data.set_open(true);
        }
        self.show_owned();
    }

    /// Closes the store game's page. Says whether one was open.
    pub(super) fn close_owned(&self) -> bool {
        let was_open = self.store.open.borrow_mut().take().is_some();
        if was_open {
            // The badges are the store game's: a library page shows none.
            self.store.badges.set_vec(Vec::new());
        }
        was_open
    }

    /// Open in library: the store game is there, so its own page opens.
    pub(super) fn open_in_library(self: &Rc<Self>) {
        let Some(id) = self.store.open.borrow().clone() else {
            return;
        };
        if !self.store.browser.borrow().is_installed(&id) {
            return;
        }
        self.close_game();
        self.navigate(Page::Library);
        self.open_game(&id);
    }

    /// Says what installing the game would do.
    fn install_note(&self, owned: &Owned, installed: bool) -> String {
        if installed {
            return String::new();
        }
        if owned.shop == Shop::Xbox {
            return xbox_install_note(
                self.store.browser.borrow().package(&owned.id),
                xodus::ready(),
                xodus::running(),
            );
        }
        if let Some(app) = owned.id.strip_prefix("steam:") {
            // Steam's dialog is open, or Steam is on it: how far it is.
            if let Some(note) = self.steam_install_note(app) {
                return note;
            }
            return owned.sizes.map_or_else(
                || t!("store-steam-installs"),
                |sizes| {
                    t!(
                        "store-steam-installs-needs",
                        size = Size::Bytes(sizes.install).to_string()
                    )
                },
            );
        }
        // Queued, running or paused: where it goes.
        if let Some((job, _)) = self.downloads.queue.borrow().job_for(&owned.id) {
            return t!("store-into", path = job.path.display().to_string());
        }
        let needs = match self.store.browser.borrow().info.get(&owned.id) {
            Some(InfoState::Got(info)) => info
                .install_size
                .map(|bytes| Size::Bytes(bytes).to_string()),
            _ => None,
        };
        let dir = self
            .config
            .borrow()
            .as_ref()
            .and_then(Config::install_dir)
            .map(|dir| dir.display().to_string());
        match (dir, needs) {
            (Some(path), Some(size)) => t!("store-installs-into-needs", path = path, size = size),
            (Some(path), None) => t!("store-installs-into", path = path),
            (None, _) => t!("store-no-home"),
        }
    }

    /// The details card of a store game: the store, platforms and makers,
    /// what the store says of sizes and versions, and the time it counted.
    fn owned_details(&self, owned: &Owned, installed: bool, now: u64) -> Vec<Fact> {
        let mut details = vec![fact(t!("fact-store"), owned.shop.label())];
        if owned.shared {
            details.push(fact(t!("fact-owner"), t!("store-shared")));
        }
        details.push(Fact {
            tone: i32::from(installed),
            ..fact(
                t!("fact-status"),
                if installed {
                    t!("installed")
                } else {
                    t!("fact-not-installed")
                },
            )
        });
        if !owned.platforms.is_empty() {
            details.push(fact(t!("fact-platforms"), owned.platforms.join(", ")));
        }
        if let Some(developer) = &owned.developer {
            details.push(fact(t!("fact-developer"), developer.clone()));
        }
        details.extend(self.store_fact_rows(&owned.id, false));
        if let Some(minutes) = owned.minutes {
            details.push(fact(
                t!("fact-play-time"),
                if minutes == 0 {
                    t!("fact-never")
                } else {
                    format::play_time(minutes)
                },
            ));
        }
        if let Some(played) = owned.last_played {
            details.push(fact(t!("fact-last-played"), format::ago(played, now)));
        }
        if owned.recent_minutes > 0 {
            details.push(fact(
                t!("fact-recent-time"),
                format::play_time(owned.recent_minutes),
            ));
        }
        if let Some(systems) = format::played_on(owned.played_on) {
            details.push(fact(t!("fact-played-on"), systems));
        }
        details
    }

    /// Writes everything the open store game's page shows.
    pub(super) fn show_owned(&self) {
        let Some(id) = self.store.open.borrow().clone() else {
            return;
        };
        let (owned, installed) = {
            let browser = self.store.browser.borrow();
            (browser.owned(&id).cloned(), browser.is_installed(&id))
        };
        let Some(owned) = owned else {
            // The list lost the game while its page was open: back to the
            // Store, with a word on why.
            self.close_game();
            self.say(t!("owned-gone"));
            return;
        };
        let now = running::now();

        let mut badges: Vec<OwnedBadge> = owned
            .platforms
            .iter()
            .map(|platform| OwnedBadge {
                label: platform.to_uppercase().into(),
                good: false,
            })
            .collect();
        if installed {
            badges.push(OwnedBadge {
                label: t!("badge-installed").into(),
                good: true,
            });
        }
        self.store.badges.set_vec(badges);

        let mut source = vec![format::source(owned.source())];
        source.extend(owned.developer.clone());
        let tiles: Vec<Tile> = owned
            .minutes
            .filter(|minutes| *minutes > 0)
            .map(|minutes| Tile {
                label: t!("fact-play-time").into(),
                value: format::play_time(minutes).into(),
            })
            .into_iter()
            .collect();

        let details = self.owned_details(&owned, installed, now);
        let technical = vec![Fact {
            mono: true,
            ..fact(t!("fact-zendo-id"), owned.id.clone())
        }];

        // Install, Import and See downloads sit beside Open in library
        // (`downloads_ui.rs`), and a Steam game's Install with Steam
        // (`steam_installs_ui.rs`).
        let game = synthesized(&owned);
        let summary = owned.summary.clone();
        let parts = OwnedPage {
            title: owned.title.clone(),
            source: source.join(" · "),
            installed,
            note: self.install_note(&owned, installed),
            cover: owned.cover.clone(),
            hero: owned.hero.clone(),
            tiles,
            details,
            technical,
        };
        self.show_owned_page(parts, |data| {
            self.show_owned_install(data, &owned, installed);
            // A Game Pass game may be installed by Zendo or by the launcher:
            // the page says where it went and opens the launcher too.
            if owned.shop == Shop::Xbox {
                data.set_owned_launcher(t!("xbox-open").into());
            }
            let images = self.show_cards(&game, data);
            // The store's own summary leads; Steam's page says it otherwise.
            if let Some(summary) = summary {
                data.set_about(summary.into());
            }
            // No Steam app id to give or Technical row for it here.
            data.set_no_match("".into());
            data.set_steam_app_row(false);
            images
        });
    }
}

#[cfg(test)]
mod tests {
    use slint::Model;
    use zendo_core::info::GameInfo;
    use zendo_core::steam_web::{Family, Session};
    use zendo_core::stores::Store;
    use zendo_shell::steam_list::SteamFetch;

    use super::super::tests::page;
    use super::*;
    use crate::{AccountsData, LibraryData};

    fn owned(store: Store, id: &str, title: &str) -> Owned {
        Owned::new(store, id, title.into())
    }

    /// A window on the Store: Epic signed in with two games, one of them in
    /// the library (as `desktop:0`), GOG signed out with one.
    fn store_page() -> (MainWindow, Rc<Glue>) {
        let (window, glue) = page(3);
        let mut installed = owned(Store::Epic, "a", "Alpha Quest");
        installed.id = "desktop:0".into();
        glue.store_cached(vec![
            (
                Shop::Epic,
                vec![installed, owned(Store::Epic, "b", "Beta Racer")],
            ),
            (Shop::Gog, vec![owned(Store::Gog, "1", "Gamma Tales")]),
        ]);
        glue.account_checked(Store::Epic, Ok(Some("me".into())), None);
        glue.account_checked(Store::Gog, Ok(None), None);
        glue.store_scanned();
        window.invoke_page_requested(Page::Store);
        (window, glue)
    }

    /// A Game Pass game's page: Zendo installs nothing, so it says so, with
    /// the Store's size, and opens the launcher that does.
    #[test]
    fn a_game_pass_install_note_follows_the_backend() {
        let package = xodus::Package {
            supported: true,
            status: "ready".into(),
            download_bytes: Some(2_000_000_000),
            format: Some("MSIXVC".into()),
        };
        let size = Size::Bytes(2_000_000_000).to_string();
        assert_eq!(
            xbox_install_note(Some(&package), true, false),
            t!("xbox-install-here-needs", size = size.clone())
        );
        assert_eq!(
            xbox_install_note(Some(&package), false, false),
            t!("xbox-install-in-launcher-needs", size = size.clone())
        );
        let no_size = xodus::Package {
            download_bytes: None,
            ..package.clone()
        };
        assert_eq!(
            xbox_install_note(Some(&no_size), true, false),
            t!("xbox-install-here")
        );
        assert_eq!(
            xbox_install_note(None, false, false),
            t!("xbox-install-in-launcher")
        );
        assert_eq!(
            xbox_install_note(Some(&package), true, true),
            t!("xbox-close-launcher"),
            "the launcher must be closed to write its state"
        );
        let unsupported = xodus::Package {
            supported: false,
            status: "unavailable".into(),
            ..package
        };
        assert_eq!(
            xbox_install_note(Some(&unsupported), true, false),
            t!("xbox-not-available")
        );
    }

    #[test]
    fn a_game_pass_page_says_where_it_is_installed() {
        let (window, glue) = store_page();
        glue.store.browser.borrow_mut().cached(vec![(
            Shop::Xbox,
            vec![Owned::new(Shop::Xbox, "9AAA", "Halo Infinite".into())],
        )]);
        glue.store.browser.borrow_mut().set_accounts(
            HashSet::from([Shop::Epic, Shop::Xbox]),
            vec![Shop::Epic, Shop::Xbox],
        );
        let package = xodus::Package {
            supported: true,
            status: "ready".into(),
            download_bytes: Some(2_000_000_000),
            format: Some("MSIXVC".into()),
        };
        glue.store
            .browser
            .borrow_mut()
            .set_package("xbox:9AAA", package);
        glue.open_owned("xbox:9AAA");
        let data = window.global::<GameData>();
        assert_eq!(data.get_owned_launcher().as_str(), t!("xbox-open"));
        let size = Size::Bytes(2_000_000_000).to_string();
        let note = if xodus::running() {
            t!("xbox-close-launcher")
        } else if xodus::ready() {
            t!("xbox-install-here-needs", size = size.clone())
        } else {
            t!("xbox-install-in-launcher-needs", size = size.clone())
        };
        assert_eq!(
            data.get_owned_note().as_str(),
            note,
            "the size the Store gives, and where it is installed"
        );

        // The Store has no build for this PC: the page says that instead.
        let package = xodus::Package {
            supported: false,
            status: "unavailable".into(),
            download_bytes: Some(5_399_841_994),
            format: Some("EAPPX".into()),
        };
        glue.store
            .browser
            .borrow_mut()
            .set_package("xbox:9AAA", package);
        glue.show_owned();
        assert_eq!(
            window.global::<GameData>().get_owned_note().as_str(),
            t!("xbox-not-available")
        );
    }

    fn labels(model: &ModelRc<ChipData>) -> Vec<String> {
        model.iter().map(|chip| chip.label.to_string()).collect()
    }

    fn session() -> Session {
        Session {
            steamid: "76561197960265730".into(),
            token: "token".into(),
        }
    }

    #[test]
    fn the_store_lists_the_signed_in_stores_and_counts_them() {
        let (window, _glue) = store_page();
        let data = window.global::<StoreData>();
        assert_eq!(window.get_store_count(), "2", "GOG is signed out");
        assert_eq!(
            labels(&data.get_sources()),
            [
                t!("library-all"),
                "Epic".to_owned(),
                t!("store-not-installed-only"),
            ]
        );
        assert_eq!(data.get_rows().row_count(), 1, "two cards in a row or more");
        assert_eq!(data.get_empty_title(), "");
        assert!(data.get_refresh_on());
        assert_eq!(data.get_refresh_label(), t!("library-refresh"));
    }

    #[test]
    fn a_chip_and_the_search_filter_and_say_why_nothing_is_left() {
        let (window, glue) = store_page();
        let data = window.global::<StoreData>();
        let only = data
            .get_sources()
            .iter()
            .find(|chip| chip.label == t!("store-not-installed-only"))
            .expect("the chip");
        data.invoke_chip(only.id);
        assert_eq!(
            data.get_sources().row_data(2).map(|chip| chip.count),
            Some("1".into())
        );
        data.invoke_search_edited("zzz".into());
        assert_eq!(
            data.get_empty_title(),
            t!("library-no-match", search = "zzz")
        );
        // Escape clears the search first.
        glue.back();
        assert_eq!(data.get_search(), "");
        assert_eq!(data.get_empty_title(), "");
        assert_eq!(window.get_page(), Page::Store);
    }

    #[test]
    fn a_card_opens_the_store_games_page_and_escape_comes_back_to_it() {
        let (window, glue) = store_page();
        let data = window.global::<StoreData>();
        let game = window.global::<GameData>();
        data.invoke_card_clicked("epic:b".into());
        assert!(game.get_open() && game.get_owned());
        assert_eq!(game.get_title(), "Beta Racer");
        assert!(!game.get_owned_installed());
        assert_ne!(game.get_owned_note(), "", "says what installing would do");
        let rows: Vec<String> = game
            .get_details()
            .iter()
            .map(|fact| fact.label.to_string())
            .collect();
        assert!(rows.contains(&t!("fact-store")) && rows.contains(&t!("fact-status")));
        assert_eq!(game.get_technical().row_count(), 1);
        glue.back();
        assert!(!game.get_open() && !game.get_owned());
        assert_eq!(window.get_page(), Page::Store);
        assert_eq!(data.get_cursor(), 1, "the cursor is where it was");
    }

    #[test]
    fn an_installed_store_game_opens_in_the_library() {
        let (window, glue) = store_page();
        let game = window.global::<GameData>();
        window
            .global::<StoreData>()
            .invoke_card_clicked("desktop:0".into());
        assert!(game.get_owned_installed());
        assert_eq!(game.get_owned_note(), "");
        assert_eq!(game.get_owned_badges().row_count(), 1, "Installed");
        // Enter does what the button does.
        assert!(glue.key("\n", false, false, false));
        assert_eq!(window.get_page(), Page::Library);
        assert!(game.get_open() && !game.get_owned());
        assert_eq!(
            game.get_owned_badges().row_count(),
            0,
            "a library page has no store badges"
        );
        assert_eq!(game.get_title(), "Game 00");
    }

    #[test]
    fn the_keys_move_the_cursor_open_a_game_and_tab_sorts() {
        let (window, glue) = store_page();
        let data = window.global::<StoreData>();
        let game = window.global::<GameData>();
        assert!(glue.key("\u{f703}", false, false, false), "right");
        assert_eq!(data.get_cursor(), 1);
        assert!(glue.key("\t", false, false, false));
        assert_eq!(data.get_cursor(), 0, "a new order starts at the first card");
        assert!(glue.key("\n", false, false, false));
        assert!(game.get_owned());
        glue.back();
        // Ctrl+F is the Store's search, not the Library's.
        assert!(glue.key("f", true, false, false));
        assert_eq!(window.global::<LibraryData>().get_search(), "");
    }

    #[test]
    fn signing_out_takes_a_stores_games_off_the_page() {
        let (window, glue) = store_page();
        let data = window.global::<StoreData>();
        glue.signed_out(Store::Epic, Ok(()));
        assert_eq!(window.get_store_count(), "0");
        assert_eq!(data.get_empty_title(), t!("store-sign-in-first"));
        assert!(data.get_empty_accounts());
        assert!(!data.get_refresh_on());
        data.invoke_go_accounts();
        assert_eq!(window.get_page(), Page::Accounts);
    }

    #[test]
    fn a_store_that_failed_says_so_and_keeps_its_list() {
        let (window, glue) = store_page();
        let data = window.global::<StoreData>();
        glue.store_fetched(Shop::Epic, Err("offline".into()), &[]);
        assert_eq!(data.get_problems().row_count(), 1);
        assert_eq!(window.get_store_count(), "2");
    }

    #[test]
    fn what_the_store_says_of_a_game_fills_its_page() {
        let (window, glue) = store_page();
        let game = window.global::<GameData>();
        let info = GameInfo {
            download_size: Some(2_000_000_000),
            version: Some("1.2".into()),
            dlc: vec![
                zendo_core::info::Dlc {
                    id: "d1".into(),
                    title: "Season pass".into(),
                    installable: false,
                    download_size: None,
                    install_size: None,
                },
                zendo_core::info::Dlc {
                    id: "d2".into(),
                    title: "Soundtrack".into(),
                    installable: true,
                    download_size: None,
                    install_size: Some(1_000_000_000),
                },
            ],
            ..GameInfo::default()
        };
        glue.store
            .browser
            .borrow_mut()
            .info
            .insert("epic:b".into(), InfoState::Got(info));
        window
            .global::<StoreData>()
            .invoke_card_clicked("epic:b".into());
        assert_eq!(game.get_dlc().row_count(), 2);
        assert_eq!(game.get_dlc_count(), "", "no count off the library");
        assert_eq!(
            game.get_dlc().row_data(0).map(|row| row.about),
            Some(t!("dlc-comes-with").into())
        );
        assert_eq!(game.get_dlc_note(), "", "a store game's page has no note");
        assert_eq!(
            game.get_dlc().row_data(1).map(|row| row.label),
            Some("".into())
        );
        let rows: Vec<String> = game
            .get_details()
            .iter()
            .map(|fact| fact.label.to_string())
            .collect();
        assert!(rows.contains(&t!("fact-download-size")));
        assert!(rows.contains(&t!("fact-latest-version")));
    }

    #[test]
    fn steams_card_signs_in_and_out_and_its_games_come_with_the_sign_in() {
        let (window, glue) = store_page();
        let data = window.global::<AccountsData>();
        assert_eq!(data.get_steam().state, 0);
        data.invoke_steam_sign_in();
        assert_eq!(data.get_steam().state, 1);
        data.invoke_steam_cancel();
        assert_eq!(data.get_steam().state, 0);
        // A cancelled window's answer is let go.
        glue.steam_signed_in(Ok(Some(session())));
        assert_eq!(glue.steam_session(), None);

        data.invoke_steam_sign_in();
        glue.steam_signed_in(Ok(Some(session())));
        assert_eq!(glue.steam_session(), Some(session()));
        assert_eq!(data.get_steam().state, 2);
        assert!(data.get_steam().status.contains("76561197960265730"));
        assert_eq!(
            window.get_accounts_count(),
            t!("top-bar-signed-in", count = 2, stores = 4)
        );

        let games = vec![
            Owned::new(Shop::Steam, "620", "Portal 2".into()),
            Owned::new(Shop::Steam, "400", "Portal".into()),
        ];
        glue.steam_fetched(Ok(SteamFetch::Games {
            games,
            renewed: Some(Session {
                token: "new".into(),
                ..session()
            }),
            family: Some(Family {
                name: "Home".into(),
                members: 3,
            }),
            family_error: Some("answered 401".into()),
        }));
        assert_eq!(window.get_store_count(), "4");
        assert_eq!(
            glue.steam_session().map(|session| session.token),
            Some("new".into()),
            "a renewed token is kept"
        );
        let card = data.get_steam();
        assert_eq!(card.family, t!("steam-family", name = "Home", count = 3));
        assert_eq!(
            card.family_note,
            t!("steam-family-failed", error = "answered 401")
        );

        // Steam's page no longer knows the sign-in.
        glue.steam_fetched(Ok(SteamFetch::SignedOut));
        assert_eq!(glue.steam_session(), None);
        assert_eq!(data.get_steam().state, 0);
        assert_eq!(data.get_steam().note, t!("steam-sign-in-again"));
        assert_eq!(window.get_store_count(), "2", "Steam's games left the page");
    }

    #[test]
    fn signing_out_of_steam_forgets_the_session_and_the_list() {
        let (window, glue) = store_page();
        let data = window.global::<AccountsData>();
        data.invoke_steam_sign_in();
        glue.steam_signed_in(Ok(Some(session())));
        glue.steam_fetched(Ok(SteamFetch::Games {
            games: vec![Owned::new(Shop::Steam, "620", "Portal 2".into())],
            renewed: None,
            family: None,
            family_error: None,
        }));
        assert_eq!(window.get_store_count(), "3");
        data.invoke_steam_sign_out();
        assert_eq!(glue.steam_session(), None);
        assert_eq!(window.get_store_count(), "2");
        assert_eq!(data.get_steam().note, t!("steam-signed-out"));
    }

    #[test]
    fn a_session_that_does_not_parse_counts_as_signed_out_everywhere() {
        let (window, glue) = store_page();
        glue.save_setting(super::super::settings::Setting::SteamSession(Some(
            "not a session".into(),
        )));
        glue.store_cached(vec![(
            Shop::Steam,
            vec![Owned::new(Shop::Steam, "620", "Portal 2".into())],
        )]);
        assert_eq!(window.get_store_count(), "2", "Steam's list stays hidden");
        assert_eq!(window.global::<AccountsData>().get_steam().state, 0);
    }
}
