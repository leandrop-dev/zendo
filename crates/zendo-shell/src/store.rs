//! The Store page's state, with no toolkit in it: the games the signed-in
//! stores say the user owns, what the chips and the search let through, the
//! cursor, the stores being asked, and what a card or the empty page says.
//! The window's Store page carries it; the tests here need no window.
//!
//! Every change answers with the Library's [`Outcome`] (rows to draw again,
//! whether to scroll); the Store writes nothing to `state.json`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::store_facts::InfoState;
use crate::{format, t};
use zendo_core::Sort;
use zendo_core::browse::{self, Move};
use zendo_core::covers::Kind;
use zendo_core::owned::{self, Owned, Shop, StoreDetails};
use zendo_core::xodus::Package;

use crate::library::{CardData, GEOMETRY, Outcome, Rows, Scroll};

/// What a chip does when pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorePick {
    /// "All": every store.
    All,
    Shop(Shop),
    ToggleInstalled,
    Sort(Sort),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreChip {
    pub label: String,
    pub count: Option<usize>,
    pub chosen: bool,
    pub pick: StorePick,
}

/// The two chip rows.
#[derive(Debug, Default)]
pub struct StoreChips {
    pub sources: Vec<StoreChip>,
    pub sorts: Vec<StoreChip>,
}

/// What the page says in place of the grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreEmpty {
    pub title: String,
    /// Whether the way to Accounts is offered: no store is signed in.
    pub accounts: bool,
}

#[derive(Debug)]
pub struct StoreBrowser {
    /// Each store's list, as the stores or the cache gave it.
    lists: HashMap<Shop, Vec<Owned>>,
    /// Every list, flat, in the stores' own order.
    games: Vec<Owned>,
    /// The stores whose games the page shows: not signed out.
    listed: HashSet<Shop>,
    /// The stores signed in (Steam with a session), in the stores' order.
    connected: Vec<Shop>,
    /// The ids of the games in the library.
    installed: HashSet<String>,
    asking: HashSet<Shop>,
    errors: HashMap<Shop, String>,
    search: String,
    source: Option<Shop>,
    hide_installed: bool,
    sort: Sort,
    /// The games the filters let through, as places in `games`.
    shown: Vec<usize>,
    cursor: usize,
    columns: usize,
    scroll_top: f32,
    viewport: f32,
    /// Sizes and versions asked of the stores, by game id.
    pub info: HashMap<String, InfoState>,
    /// What the Store would install for each Game Pass game, by game id: its
    /// size, or that this PC cannot install it at all.
    packages: HashMap<String, zendo_core::xodus::Package>,
}

impl Default for StoreBrowser {
    fn default() -> Self {
        Self::new()
    }
}

impl StoreBrowser {
    #[must_use]
    pub fn new() -> Self {
        Self {
            lists: HashMap::new(),
            games: Vec::new(),
            listed: HashSet::new(),
            connected: Vec::new(),
            installed: HashSet::new(),
            asking: HashSet::new(),
            errors: HashMap::new(),
            search: String::new(),
            source: None,
            hide_installed: false,
            sort: Sort::Title,
            shown: Vec::new(),
            cursor: 0,
            columns: 5,
            scroll_top: 0.0,
            viewport: 800.0,
            info: HashMap::new(),
            packages: HashMap::new(),
        }
    }

    /// The Store's answer about a Game Pass game's package: what a card says
    /// about installing it.
    pub fn set_package(&mut self, id: &str, package: zendo_core::xodus::Package) -> Outcome {
        self.packages.insert(id.to_owned(), package);
        self.rebuild()
    }

    /// What the Store said about a Game Pass game's package, once asked.
    #[must_use]
    pub fn package(&self, id: &str) -> Option<&zendo_core::xodus::Package> {
        self.packages.get(id)
    }

    // What the window reads.

    #[must_use]
    pub fn len(&self) -> usize {
        self.shown.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shown.is_empty()
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor.min(self.shown.len().saturating_sub(1))
    }

    #[must_use]
    pub fn columns(&self) -> usize {
        self.columns
    }

    #[must_use]
    pub fn search(&self) -> &str {
        &self.search
    }

    /// The signed-in stores, in the stores' order.
    #[must_use]
    pub fn connected(&self) -> &[Shop] {
        &self.connected
    }

    #[must_use]
    pub fn has_list(&self, shop: Shop) -> bool {
        self.lists.contains_key(&shop)
    }

    #[must_use]
    pub fn list(&self, shop: Shop) -> Option<&[Owned]> {
        self.lists.get(&shop).map(Vec::as_slice)
    }

    /// Any owned game by id, whether or not the page shows it now.
    #[must_use]
    pub fn owned(&self, id: &str) -> Option<&Owned> {
        self.games.iter().find(|game| game.id == id)
    }

    /// Whether the game is in the library.
    #[must_use]
    pub fn is_installed(&self, id: &str) -> bool {
        self.installed.contains(id)
    }

    /// The game under the cursor.
    #[must_use]
    pub fn selected(&self) -> Option<&Owned> {
        self.shown.get(self.cursor()).map(|&at| &self.games[at])
    }

    fn selected_id(&self) -> Option<String> {
        self.selected().map(|game| game.id.clone())
    }

    /// How many games the page lists before any chip or search.
    #[must_use]
    pub fn listed_count(&self) -> usize {
        self.games
            .iter()
            .filter(|game| self.listed.contains(&game.shop))
            .count()
    }

    /// The stores being asked, in the stores' order.
    #[must_use]
    pub fn asking(&self) -> Vec<Shop> {
        Shop::ALL
            .into_iter()
            .filter(|shop| self.asking.contains(shop))
            .collect()
    }

    /// What went wrong, a line for each store, in the stores' order.
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        Shop::ALL
            .into_iter()
            .filter_map(|shop| {
                self.errors
                    .get(&shop)
                    .map(|err| format!("{}: {err}", shop.label()))
            })
            .collect()
    }

    /// The card at this place in the shown list.
    #[must_use]
    pub fn card(&self, place: usize) -> Option<CardData> {
        let game = self.games.get(*self.shown.get(place)?)?;
        let installed = self.installed.contains(&game.id);
        // A Game Pass game this PC cannot install is marked, as a Steam game
        // is not: the Store, not the launcher, says so.
        let not_for_pc = game.shop == Shop::Xbox
            && self
                .packages
                .get(&game.id)
                .is_some_and(|package| !package.supported);
        let (badge, badge_running) = if installed {
            (Some(t!("badge-installed")), true)
        } else if not_for_pc {
            (Some(t!("badge-unsupported")), false)
        } else {
            (None, false)
        };
        Some(CardData {
            id: game.id.clone(),
            title: game.title.clone(),
            caption: format::owned_line(game),
            caption_accent: false,
            faded: false,
            // The badge in the colour for good news, as the Library's
            // Running one.
            badge,
            badge_running,
            platform: game.shop.label().to_uppercase(),
            progress: None,
            marked: false,
            cover: game.cover.clone(),
        })
    }

    /// What the page says instead of the grid, when it has no cards.
    #[must_use]
    pub fn empty(&self) -> Option<StoreEmpty> {
        if !self.shown.is_empty() {
            return None;
        }
        let say = |title: String| StoreEmpty {
            title,
            accounts: false,
        };
        Some(if self.connected.is_empty() && self.listed_count() == 0 {
            StoreEmpty {
                title: t!("store-sign-in-first"),
                accounts: true,
            }
        } else if !self.asking.is_empty() {
            say(t!("store-asking-all"))
        } else if !self.search.trim().is_empty() {
            say(t!("library-no-match", search = self.search.as_str()))
        } else if self.source.is_some() || self.hide_installed {
            say(t!("library-none-here"))
        } else {
            say(t!("store-nothing"))
        })
    }

    /// The chips, with the counts the lists give them now.
    #[must_use]
    pub fn chips(&self) -> StoreChips {
        let listed: Vec<&Owned> = self
            .games
            .iter()
            .filter(|game| self.listed.contains(&game.shop))
            .collect();
        let installed = listed
            .iter()
            .filter(|game| self.installed.contains(&game.id))
            .count();
        let chip = |label: String, count: Option<usize>, chosen: bool, pick: StorePick| StoreChip {
            label,
            count,
            chosen,
            pick,
        };
        let mut sources = vec![chip(
            t!("library-all"),
            Some(listed.len()),
            self.source.is_none(),
            StorePick::All,
        )];
        for shop in Shop::ALL {
            let count = listed.iter().filter(|game| game.shop == shop).count();
            if count > 0 {
                sources.push(chip(
                    shop.label().to_owned(),
                    Some(count),
                    self.source == Some(shop),
                    StorePick::Shop(shop),
                ));
            }
        }
        sources.push(chip(
            t!("store-not-installed-only"),
            Some(listed.len() - installed),
            self.hide_installed,
            StorePick::ToggleInstalled,
        ));
        let sorts = [Sort::Title, Sort::Recent, Sort::MostPlayed]
            .into_iter()
            .map(|sort| {
                chip(
                    format::sort(sort),
                    None,
                    self.sort == sort,
                    StorePick::Sort(sort),
                )
            })
            .collect();
        StoreChips { sources, sorts }
    }

    // What changes it.

    /// The lists kept from last time; a store already listed keeps its own.
    pub fn cached(&mut self, lists: Vec<(Shop, Vec<Owned>)>) -> Outcome {
        for (shop, games) in lists {
            if !games.is_empty() {
                self.lists.entry(shop).or_insert(games);
            }
        }
        self.rebuild()
    }

    /// A store answered: its list, with the games it could not ask about
    /// said and the rest kept, or why it could not answer.
    pub fn fetched(
        &mut self,
        shop: Shop,
        result: Result<Vec<Owned>, String>,
        problems: &[String],
    ) -> Outcome {
        self.asking.remove(&shop);
        match result {
            Ok(games) => {
                if problems.is_empty() {
                    self.errors.remove(&shop);
                } else {
                    self.errors.insert(shop, problems.join("; "));
                }
                self.lists.insert(shop, games);
                self.rebuild()
            }
            Err(err) => {
                self.errors.insert(shop, err);
                Outcome::default()
            }
        }
    }

    /// A signed-out store's games leave the page; the caller removes the
    /// cache.
    pub fn forget(&mut self, shop: Shop) -> Outcome {
        self.lists.remove(&shop);
        self.errors.remove(&shop);
        self.rebuild()
    }

    /// Which stores are signed in, and whose games the page lists.
    pub fn set_accounts(&mut self, listed: HashSet<Shop>, connected: Vec<Shop>) -> Outcome {
        self.connected = connected;
        if listed == self.listed {
            return Outcome::default();
        }
        self.listed = listed;
        self.refilter_keeping()
    }

    /// The ids of the games in the library, after a scan.
    pub fn set_installed(&mut self, ids: HashSet<String>) -> Outcome {
        if ids == self.installed {
            return Outcome::default();
        }
        self.installed = ids;
        self.refilter_keeping()
    }

    /// Marks a store as being asked; `false` when it was already.
    pub fn start_asking(&mut self, shop: Shop) -> bool {
        self.asking.insert(shop)
    }

    pub fn stop_asking(&mut self, shop: Shop) {
        self.asking.remove(&shop);
    }

    /// A game's image was downloaded. Only a cover changes a card.
    pub fn set_image(&mut self, id: &str, kind: Kind, path: &Path) -> Outcome {
        self.put_image(id, kind, Some(path))
    }

    /// A game's image is now this one, or none: the art picker's "Use the
    /// automatic one" puts back the one found by itself, which may not be
    /// there.
    pub fn put_image(&mut self, id: &str, kind: Kind, path: Option<&Path>) -> Outcome {
        let mut found = false;
        // Both copies of the list: the flat one the page reads, and the
        // stores' own, which is kept.
        let both = self
            .games
            .iter_mut()
            .chain(self.lists.values_mut().flatten())
            .filter(|game| game.id == id);
        for game in both {
            found = true;
            match kind {
                Kind::Cover => game.cover = path.map(Path::to_path_buf),
                Kind::Hero => game.hero = path.map(Path::to_path_buf),
            }
        }
        match self.place_of(id).filter(|_| found && kind == Kind::Cover) {
            Some(place) => Outcome {
                rows: Rows::Marks(vec![place]),
                ..Outcome::default()
            },
            None => Outcome::default(),
        }
    }

    /// What Steam's store says of a Steam game, put in its entries. Says
    /// whether Steam's list changed, so the caller keeps it.
    pub fn fill_steam(&mut self, id: &str, details: &StoreDetails) -> bool {
        let mut filled = false;
        for game in self
            .lists
            .values_mut()
            .flatten()
            .filter(|game| game.id == id)
        {
            details.fill(game);
            filled = true;
        }
        for game in self.games.iter_mut().filter(|game| game.id == id) {
            details.fill(game);
        }
        filled
    }

    /// The games not in the library, for their images.
    #[must_use]
    pub fn images_wanted(&self) -> Vec<Owned> {
        self.games
            .iter()
            .filter(|game| !self.installed.contains(&game.id))
            .cloned()
            .collect()
    }

    /// The columns the window's width gives.
    pub fn set_columns(&mut self, columns: usize) -> Outcome {
        let columns = columns.max(1);
        if columns == self.columns {
            return Outcome::default();
        }
        self.columns = columns;
        Outcome {
            rows: Rows::All,
            scroll: self.reveal(),
            ..Outcome::default()
        }
    }

    /// How far the grid is scrolled, and how tall its window is.
    pub fn scrolled(&mut self, top: f32, height: f32) {
        self.scroll_top = top.max(0.0);
        if height > 0.0 {
            self.viewport = height;
        }
    }

    /// The rows on screen, and two above and below, as places in the grid.
    #[must_use]
    pub fn rows_near(&self) -> std::ops::Range<usize> {
        let pitch = crate::library::CARD_HEIGHT + crate::library::SPACING;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "non-negative, and far below usize's range"
        )]
        let (first, last) = (
            (self.scroll_top / pitch) as usize,
            ((self.scroll_top + self.viewport) / pitch).ceil() as usize,
        );
        first.saturating_sub(2)..last + 2
    }

    /// A chip was pressed.
    pub fn pick(&mut self, pick: StorePick) -> Outcome {
        match pick {
            StorePick::All => self.source = None,
            StorePick::Shop(shop) => self.source = Some(shop),
            StorePick::ToggleInstalled => self.hide_installed = !self.hide_installed,
            StorePick::Sort(sort) => self.sort = sort,
        }
        self.filter_changed()
    }

    pub fn cycle_sort(&mut self) -> Outcome {
        self.sort = self.sort.next();
        self.filter_changed()
    }

    /// The search field's text changed.
    pub fn set_search(&mut self, search: &str) -> Outcome {
        if search == self.search {
            return Outcome::default();
        }
        search.clone_into(&mut self.search);
        self.filter_changed()
    }

    /// Typing with no field focused starts the search.
    pub fn type_text(&mut self, typed: &str) -> Outcome {
        self.search.push_str(typed);
        self.filter_changed()
    }

    pub fn backspace(&mut self) -> Outcome {
        self.search.pop();
        self.filter_changed()
    }

    /// Escape clears the search.
    pub fn back(&mut self) -> Outcome {
        if self.search.is_empty() {
            return Outcome::default();
        }
        self.search.clear();
        self.filter_changed()
    }

    /// A card was clicked: the cursor goes to it.
    pub fn click(&mut self, id: &str) {
        if let Some(place) = self.place_of(id) {
            self.cursor = place;
        }
    }

    pub fn step(&mut self, step: Move) -> Outcome {
        if self.shown.is_empty() {
            return Outcome::default();
        }
        let rows = browse::rows_in(self.viewport, GEOMETRY);
        self.cursor = browse::step(self.cursor, self.shown.len(), self.columns, rows, step);
        Outcome {
            scroll: self.reveal(),
            ..Outcome::default()
        }
    }

    // Underneath.

    fn place_of(&self, id: &str) -> Option<usize> {
        self.shown.iter().position(|&at| self.games[at].id == id)
    }

    /// The lists changed: the flat list is made again.
    fn rebuild(&mut self) -> Outcome {
        let id = self.selected_id();
        self.games = Shop::ALL
            .into_iter()
            .filter_map(|shop| self.lists.get(&shop))
            .flatten()
            .cloned()
            .collect();
        self.reselect(id)
    }

    /// The chips, the search or the lists changed: what is shown is worked out
    /// again, the cursor staying on its game when the game stays.
    fn refilter_keeping(&mut self) -> Outcome {
        let id = self.selected_id();
        self.reselect(id)
    }

    fn reselect(&mut self, id: Option<String>) -> Outcome {
        self.recompute();
        let kept = id.and_then(|id| self.place_of(&id));
        self.cursor = kept.unwrap_or_else(|| self.cursor.min(self.shown.len().saturating_sub(1)));
        Outcome {
            rows: Rows::All,
            scroll: self.reveal(),
            ..Outcome::default()
        }
    }

    /// A chip or the search changed: back to the first card, at the top.
    fn filter_changed(&mut self) -> Outcome {
        self.recompute();
        self.cursor = 0;
        Outcome {
            rows: Rows::All,
            scroll: Scroll::Top,
            ..Outcome::default()
        }
    }

    fn recompute(&mut self) {
        let wanted = owned::Filter {
            source: self.source,
            hide_installed: self.hide_installed,
            search: &self.search,
        };
        let mut shown: Vec<usize> = (0..self.games.len())
            .filter(|&at| {
                let game = &self.games[at];
                self.listed.contains(&game.shop)
                    && wanted.keeps(game, self.installed.contains(&game.id))
            })
            .collect();
        shown.sort_by_cached_key(|&at| owned::sort_key(&self.games[at], self.sort));
        self.shown = shown;
    }

    /// Scrolls just enough to show the cursor's card.
    fn reveal(&self) -> Scroll {
        browse::reveal_y(
            self.cursor(),
            self.columns,
            self.scroll_top,
            self.viewport,
            GEOMETRY,
        )
        .map_or(Scroll::Keep, Scroll::To)
    }
}

/// What a Game Pass game's page says about installing it: Zendo's own
/// install when the launcher's backend and runtime are there, the launcher's
/// otherwise. Both windows say it the same.
#[must_use]
pub fn xbox_install_note(package: Option<&Package>, ready: bool, running: bool) -> String {
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
            size = zendo_core::Size::Bytes(bytes).to_string()
        ),
        (true, None) => t!("xbox-install-here"),
        (false, Some(bytes)) => t!(
            "xbox-install-in-launcher-needs",
            size = zendo_core::Size::Bytes(bytes).to_string()
        ),
        (false, None) => t!("xbox-install-in-launcher"),
    }
}

#[cfg(test)]
mod tests {
    use zendo_core::stores::Store;

    use super::*;

    /// Epic and GOG signed in, Amazon signed out, Steam without a session;
    /// Epic's first game is in the library.
    fn browser() -> StoreBrowser {
        let owned = |store: Store, id: &str, title: &str| Owned::new(store, id, title.to_owned());
        let mut browser = StoreBrowser::new();
        browser.cached(vec![
            (
                Shop::Epic,
                vec![
                    owned(Store::Epic, "a", "Alpha Quest"),
                    owned(Store::Epic, "b", "Beta Racer"),
                ],
            ),
            (
                Shop::Gog,
                vec![
                    owned(Store::Gog, "1", "Gamma Tales"),
                    owned(Store::Gog, "2", "Delta Quest"),
                ],
            ),
            (Shop::Amazon, vec![owned(Store::Amazon, "x", "Omega")]),
            (
                Shop::Steam,
                vec![Owned::new(Shop::Steam, "620", "Portal 2".into())],
            ),
        ]);
        browser.set_accounts(
            HashSet::from([Shop::Epic, Shop::Gog]),
            vec![Shop::Epic, Shop::Gog],
        );
        browser.set_installed(HashSet::from(["epic:a".to_owned()]));
        browser.set_columns(2);
        browser
    }

    fn titles(browser: &StoreBrowser) -> Vec<String> {
        (0..browser.len())
            .filter_map(|place| browser.card(place))
            .map(|card| card.title)
            .collect()
    }

    #[test]
    fn shows_the_signed_in_stores_by_title() {
        let browser = browser();
        assert_eq!(
            titles(&browser),
            ["Alpha Quest", "Beta Racer", "Delta Quest", "Gamma Tales"]
        );
        assert_eq!(browser.listed_count(), 4, "Amazon is signed out");
        assert!(browser.is_installed("epic:a"));
        assert!(browser.card(0).is_some_and(|card| card.badge.is_some()));
        assert!(browser.card(1).is_some_and(|card| card.badge.is_none()));
    }

    #[test]
    fn a_game_pass_card_marks_what_this_pc_cannot_install() {
        let mut browser = browser();
        browser.cached(vec![(
            Shop::Xbox,
            vec![
                Owned::new(Shop::Xbox, "9AAA", "Halo Infinite".into()),
                Owned::new(Shop::Xbox, "9BBB", "Cuphead".into()),
            ],
        )]);
        browser.set_accounts(HashSet::from([Shop::Xbox]), vec![Shop::Xbox]);
        browser.pick(StorePick::Shop(Shop::Xbox));
        let package = |supported: bool| zendo_core::xodus::Package {
            supported,
            status: String::new(),
            download_bytes: None,
            format: None,
        };
        browser.set_package("xbox:9AAA", package(true));
        browser.set_package("xbox:9BBB", package(false));
        let cards: Vec<_> = (0..browser.len())
            .filter_map(|place| browser.card(place))
            .collect();
        // By title: Cuphead, then Halo Infinite.
        assert_eq!(
            cards[0].badge.as_deref(),
            Some(t!("badge-unsupported").as_str()),
            "a badge, not the caption"
        );
        assert!(!cards[0].badge_running, "not the good news colour");
        assert_eq!(
            cards[0].caption,
            format::owned_line(browser.owned("xbox:9BBB").unwrap()),
            "the caption is what every store card says"
        );
        assert_eq!(cards[1].badge, None, "an installable one has no mark");
    }

    #[test]
    fn shows_game_pass_games_under_their_own_chip() {
        let mut browser = browser();
        browser.cached(vec![(
            Shop::Xbox,
            vec![Owned::new(Shop::Xbox, "9AAA", "Halo Infinite".into())],
        )]);
        browser.set_accounts(
            HashSet::from([Shop::Epic, Shop::Gog, Shop::Xbox]),
            vec![Shop::Epic, Shop::Gog, Shop::Xbox],
        );
        let chips = browser.chips();
        assert!(
            chips
                .sources
                .iter()
                .any(|chip| chip.label == "Xbox" && chip.count == Some(1)),
            "a chip of its own"
        );
        browser.pick(StorePick::Shop(Shop::Xbox));
        assert_eq!(titles(&browser), ["Halo Infinite"]);
        assert_eq!(
            browser.card(0).map(|card| card.id),
            Some("xbox:9AAA".to_owned())
        );
        assert!(!browser.is_installed("xbox:9AAA"), "not installed yet");
    }

    #[test]
    fn shows_steams_games_while_it_is_signed_in() {
        let mut browser = browser();
        browser.set_accounts(
            HashSet::from([Shop::Steam, Shop::Epic, Shop::Gog]),
            vec![Shop::Steam, Shop::Epic, Shop::Gog],
        );
        assert_eq!(browser.listed_count(), 5);
        browser.pick(StorePick::Shop(Shop::Steam));
        assert_eq!(titles(&browser), ["Portal 2"]);
    }

    #[test]
    fn filters_by_store_search_and_installed() {
        let mut browser = browser();
        browser.pick(StorePick::Shop(Shop::Gog));
        assert_eq!(titles(&browser), ["Delta Quest", "Gamma Tales"]);
        browser.pick(StorePick::All);
        browser.set_search("quest");
        assert_eq!(titles(&browser), ["Alpha Quest", "Delta Quest"]);
        browser.pick(StorePick::ToggleInstalled);
        assert_eq!(titles(&browser), ["Delta Quest"]);
        browser.back();
        assert_eq!(browser.search(), "", "Esc clears the search first");
        assert_eq!(
            browser.empty(),
            None,
            "the chips still leave a game to show"
        );
    }

    #[test]
    fn the_chips_count_what_each_leaves() {
        let mut browser = browser();
        let counts = |browser: &StoreBrowser| {
            browser
                .chips()
                .sources
                .iter()
                .map(|chip| (chip.label.clone(), chip.count, chip.chosen))
                .collect::<Vec<_>>()
        };
        let all = t!("library-all");
        let only = t!("store-not-installed-only");
        assert_eq!(
            counts(&browser),
            [
                (all.clone(), Some(4), true),
                ("Epic".into(), Some(2), false),
                ("GOG".into(), Some(2), false),
                (only.clone(), Some(3), false),
            ]
        );
        browser.pick(StorePick::Shop(Shop::Epic));
        browser.pick(StorePick::ToggleInstalled);
        let chosen: Vec<String> = browser
            .chips()
            .sources
            .into_iter()
            .filter(|chip| chip.chosen)
            .map(|chip| chip.label)
            .collect();
        assert_eq!(chosen, ["Epic".to_owned(), only]);
    }

    #[test]
    fn sorts_by_title_last_played_or_time_played() {
        let mut browser = browser();
        assert_eq!(browser.chips().sorts[0].label, format::sort(Sort::Title));
        assert!(browser.chips().sorts[0].chosen);
        browser.cached(vec![]);
        let mut steam = Owned::new(Shop::Steam, "1", "Zed".into());
        steam.minutes = Some(500);
        steam.last_played = Some(10);
        let mut other = Owned::new(Shop::Steam, "2", "Yak".into());
        other.minutes = Some(5);
        other.last_played = Some(99);
        browser.fetched(Shop::Steam, Ok(vec![steam, other]), &[]);
        browser.set_accounts(HashSet::from([Shop::Steam]), vec![Shop::Steam]);
        assert_eq!(titles(&browser), ["Yak", "Zed"]);
        browser.pick(StorePick::Sort(Sort::MostPlayed));
        assert_eq!(titles(&browser), ["Zed", "Yak"]);
        browser.pick(StorePick::Sort(Sort::Recent));
        assert_eq!(titles(&browser), ["Yak", "Zed"]);
        browser.cycle_sort();
        assert_eq!(browser.chips().sorts.iter().filter(|c| c.chosen).count(), 1);
    }

    #[test]
    fn the_cursor_moves_and_stays_on_its_game_when_a_list_lands() {
        let mut browser = browser();
        browser.step(Move::Down);
        browser.step(Move::Right);
        assert_eq!(browser.cursor(), 3);
        assert_eq!(
            browser.selected().map(|game| game.title.as_str()),
            Some("Gamma Tales")
        );
        // A new game sorts before it: the cursor follows its game.
        let mut list = browser.list(Shop::Epic).unwrap_or_default().to_vec();
        list.push(Owned::new(Store::Epic, "0", "Aardvark".into()));
        browser.fetched(Shop::Epic, Ok(list), &[]);
        assert_eq!(
            browser.selected().map(|game| game.title.as_str()),
            Some("Gamma Tales")
        );
        assert_eq!(browser.cursor(), 4);
    }

    #[test]
    fn signing_out_forgets_the_list_and_a_failed_ask_keeps_it() {
        let mut browser = browser();
        browser.start_asking(Shop::Epic);
        browser.fetched(Shop::Epic, Err("offline".into()), &[]);
        assert_eq!(titles(&browser).len(), 4, "the list kept from before stays");
        assert_eq!(browser.problems(), ["Epic: offline".to_owned()]);
        assert_eq!(browser.asking().len(), 0);
        browser.forget(Shop::Epic);
        assert_eq!(browser.problems().len(), 0);
        assert_eq!(titles(&browser), ["Delta Quest", "Gamma Tales"]);
    }

    #[test]
    fn says_why_there_are_no_cards() {
        let mut browser = browser();
        browser.set_search("zzz");
        assert_eq!(
            browser.empty().map(|empty| empty.title),
            Some(t!("library-no-match", search = "zzz"))
        );
        browser.set_search("");
        browser.set_accounts(HashSet::new(), Vec::new());
        assert_eq!(
            browser.empty(),
            Some(StoreEmpty {
                title: t!("store-sign-in-first"),
                accounts: true,
            })
        );
        browser.set_accounts(HashSet::from([Shop::Amazon]), vec![Shop::Amazon]);
        browser.pick(StorePick::Shop(Shop::Epic));
        assert_eq!(
            browser.empty().map(|empty| empty.title),
            Some(t!("library-none-here"))
        );
    }

    #[test]
    fn a_cover_that_arrives_redraws_its_row() {
        let mut browser = browser();
        let outcome = browser.set_image("gog:1", Kind::Cover, Path::new("/c.png"));
        assert_eq!(outcome.rows, Rows::Marks(vec![3]));
        assert_eq!(
            browser.card(3).and_then(|card| card.cover),
            Some(Path::new("/c.png").to_path_buf())
        );
        let hero = browser.set_image("gog:1", Kind::Hero, Path::new("/h.png"));
        assert_eq!(hero.rows, Rows::Same);
        assert_eq!(
            browser.owned("gog:1").and_then(|game| game.hero.clone()),
            Some(Path::new("/h.png").to_path_buf())
        );
    }
}
