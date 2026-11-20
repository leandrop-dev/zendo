//! The Library page's commands: the games with their cards, the chips, the
//! search, the sort and the cursor. The state itself is
//! [`zendo_shell::library::Browser`], shared with the Slint window; these
//! commands carry it over IPC and write the sort back to `state.json`.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use zendo_core::browse::Move;
use zendo_core::config::Config;
use zendo_core::covers::{Fetcher, Kind};
use zendo_core::{Game, Library, Source, State as CoreState, Status, View};
use zendo_shell::library::{Browser, CardData, Chip, Chips, Empty, Pick, Save};
use zendo_shell::running::Running;

/// How often the launchers' files are looked at, as the Slint window does.
const WATCH: Duration = Duration::from_secs(10);

/// The Library's state, and whether the first scan has finished.
pub struct LibraryState {
    browser: Mutex<Browser>,
    ready: AtomicBool,
    /// A scan is running.
    scanning: AtomicBool,
    /// The covers are being fetched.
    downloading: AtomicBool,
    /// The game a page may be showing, and what it is doing.
    pub(crate) running: Mutex<Running>,
}

impl LibraryState {
    pub fn new() -> Self {
        Self {
            browser: Mutex::new(Browser::new()),
            ready: AtomicBool::new(false),
            scanning: AtomicBool::new(false),
            downloading: AtomicBool::new(false),
            running: Mutex::new(Running::default()),
        }
    }
}

impl Default for LibraryState {
    fn default() -> Self {
        Self::new()
    }
}

/// Scans every source and then watches the launchers' files, as the Slint
/// window does.
pub fn start(app: AppHandle) {
    rescan(app.clone());
    watch(app);
}

/// Reads every source on a thread of its own, then says `library_loaded` so
/// the page asks for the list, and fetches the covers that are missing.
pub(crate) fn rescan(app: AppHandle) {
    if app
        .state::<LibraryState>()
        .scanning
        .swap(true, Ordering::Relaxed)
    {
        return;
    }
    std::thread::spawn(move || {
        let state = CoreState::load().unwrap_or_default();
        let library = Library::scan();
        let shell = app.state::<LibraryState>();
        if let Ok(mut browser) = shell.browser.lock() {
            browser.loaded(
                library,
                Some(state.sort),
                state.categories(),
                state.filters(),
            );
        }
        shell.ready.store(true, Ordering::Relaxed);
        shell.scanning.store(false, Ordering::Relaxed);
        let _ = app.emit("library_loaded", ());
        crate::store::library_changed(&app);
        // The games Zendo installed are asked about their updates once.
        crate::downloads::check_new_updates(&app);
        download_covers(app);
    });
}

/// Looks at the launchers' files every so often, and reads the games again
/// when one changed. The first look only notes them.
fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        let mut previous = zendo_core::watch::signature();
        loop {
            std::thread::sleep(WATCH);
            let signature = zendo_core::watch::signature();
            if signature == previous {
                continue;
            }
            previous = signature;
            rescan(app.clone());
        }
    });
}

impl LibraryState {
    /// A game by its id, scanning first when nothing has yet: a page opened
    /// straight from a link finds its game.
    pub(crate) fn game(&self, id: &str) -> Option<Game> {
        if !self.ready.load(Ordering::Relaxed) {
            let state = CoreState::load().unwrap_or_default();
            let library = Library::scan();
            if let Ok(mut browser) = self.browser.lock() {
                browser.loaded(
                    library,
                    Some(state.sort),
                    state.categories(),
                    state.filters(),
                );
            }
            self.ready.store(true, Ordering::Relaxed);
        }
        let browser = self.browser.lock().ok()?;
        browser.game(id).cloned()
    }

    /// Changes the Library and writes what it asked to remember.
    pub(crate) fn change(
        &self,
        change: impl FnOnce(&mut Browser) -> zendo_shell::library::Outcome,
    ) {
        let Ok(mut browser) = self.browser.lock() else {
            return;
        };
        let outcome = change(&mut browser);
        drop(browser);
        save(outcome.save);
    }

    /// Reads every source again, in place, and is ready: for a change made
    /// outside the scan's own thread, as adding a game is.
    pub(crate) fn reload(&self) {
        let state = CoreState::load().unwrap_or_default();
        let library = Library::scan();
        if let Ok(mut browser) = self.browser.lock() {
            browser.loaded(
                library,
                Some(state.sort),
                state.categories(),
                state.filters(),
            );
        }
        self.ready.store(true, Ordering::Relaxed);
    }

    /// Gives a game a Steam app id, or takes it back with `None`, and writes
    /// it: the game's title when it is there.
    pub(crate) fn set_steam_app(&self, id: &str, app: Option<u32>) -> Option<String> {
        let title = {
            let mut browser = self.browser.lock().ok()?;
            browser.set_steam_app(id, app)?
        };
        let _ = CoreState::update(|state| {
            state.set_steam_app(id, app);
        });
        Some(title)
    }

    /// How many games each source gave.
    pub(crate) fn sources(&self) -> Vec<(Source, usize)> {
        let browser = self
            .browser
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let counts: Vec<(Source, usize)> = Source::ALL
            .iter()
            .map(|source| {
                let count = browser
                    .games()
                    .iter()
                    .filter(|game| game.source == *source)
                    .count();
                (*source, count)
            })
            .collect();
        counts
    }

    /// What the last scan could not read, by source.
    pub(crate) fn problems(&self) -> Vec<String> {
        let browser = self
            .browser
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        browser.problems().to_vec()
    }

    /// Every game as the last scan read them.
    /// The library was read at least once.
    pub(crate) fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    pub(crate) fn games(&self) -> Vec<Game> {
        let browser = self
            .browser
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        browser.games().to_vec()
    }

    /// The hand-picked collections and the saved views, for the sheet.
    pub(crate) fn collections(&self) -> (Vec<zendo_core::Category>, Vec<zendo_core::SavedFilter>) {
        let browser = self
            .browser
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (browser.categories().to_vec(), browser.saved().to_vec())
    }

    /// The games marked on the grid.
    pub(crate) fn marked(&self) -> std::collections::BTreeSet<String> {
        let browser = self
            .browser
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        browser.marked().clone()
    }

    /// The filter the grid shows now, for a saved view.
    pub(crate) fn current_filter(&self) -> zendo_core::Filter {
        let browser = self
            .browser
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        browser.current().clone()
    }

    /// Puts a state's collections on the grid, and shows them again.
    pub(crate) fn set_collections(
        &self,
        categories: Vec<zendo_core::Category>,
        filters: Vec<zendo_core::SavedFilter>,
    ) {
        let mut browser = self
            .browser
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = browser.set_collections(categories, filters);
    }

    /// A scan is running now.
    pub(crate) fn scanning(&self) -> bool {
        self.scanning.load(Ordering::Relaxed)
    }

    /// The covers are being fetched.
    pub(crate) fn downloading(&self) -> bool {
        self.downloading.load(Ordering::Relaxed)
    }

    /// The running games.
    pub(crate) fn running(&self) -> std::sync::MutexGuard<'_, Running> {
        self.running
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// What a new cover tells the page.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CoverChanged {
    pub(crate) id: String,
    pub(crate) path: String,
}

/// Fetches the covers on demand, as Settings' Download now asks.
pub(crate) fn download_covers_now(app: &AppHandle) {
    download_covers(app.clone());
}

/// Fetches the covers and wide images the library is missing, one at a time,
/// on a thread of its own; covers first, since the grid shows them. The
/// keyring is read only when the settings ask for downloads.
fn download_covers(app: AppHandle) {
    if app
        .state::<LibraryState>()
        .downloading
        .swap(true, Ordering::Relaxed)
    {
        return;
    }
    std::thread::spawn(move || {
        let finish = |app: &AppHandle| {
            app.state::<LibraryState>()
                .downloading
                .store(false, Ordering::Relaxed);
            // Settings' Download now goes quiet with it.
            let _ = app.emit("settings_changed", ());
        };
        let file = Config::load_file().unwrap_or_default();
        if !file.download_covers {
            finish(&app);
            return;
        }
        let Ok(config) = Config::load() else {
            finish(&app);
            return;
        };
        let Some(mut fetcher) = Fetcher::new(&config) else {
            finish(&app);
            return;
        };
        let wanted: Vec<(Game, Kind)> = {
            let shell = app.state::<LibraryState>();
            let Ok(browser) = shell.browser.lock() else {
                finish(&app);
                return;
            };
            let mut wanted = Vec::new();
            for kind in [Kind::Cover, Kind::Hero] {
                wanted.extend(
                    browser
                        .games()
                        .iter()
                        .filter(|game| fetcher.wants(*game, kind))
                        .map(|game| (game.clone(), kind)),
                );
            }
            wanted
        };
        let mut failed = 0_u64;
        for (game, kind) in wanted {
            let id = game.id.clone();
            match fetcher.fetch(&game, kind) {
                Ok(Some(path)) => {
                    if let Ok(mut browser) = app.state::<LibraryState>().browser.lock() {
                        browser.set_image(&id, kind, path.clone());
                    }
                    if matches!(kind, Kind::Cover) {
                        let _ = app.emit(
                            "cover_changed",
                            CoverChanged {
                                id,
                                path: path.display().to_string(),
                            },
                        );
                    }
                }
                Ok(None) => {}
                Err(_) => failed = failed.saturating_add(1),
            }
        }
        if failed > 0 {
            let note = zendo_shell::t!("images-failed", count = failed);
            if let Ok(mut browser) = app.state::<LibraryState>().browser.lock() {
                browser.say(Some(note.clone()));
            }
            let _ = app.emit("library_status", note);
        }
        finish(&app);
    });
}

/// A card as the grid draws it.
#[derive(Debug, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one thing the card draws, as the Slint struct has them"
)]
pub struct CardDto {
    id: String,
    title: String,
    caption: String,
    caption_accent: bool,
    faded: bool,
    badge: Option<String>,
    badge_running: bool,
    /// The share of achievements, 0 to 1; `None` when the game has none.
    progress: Option<f32>,
    marked: bool,
    /// The cover in the cache; the frontend turns it into an asset URL.
    cover: Option<String>,
}

impl From<CardData> for CardDto {
    fn from(card: CardData) -> Self {
        Self {
            id: card.id,
            title: card.title,
            caption: card.caption,
            caption_accent: card.caption_accent,
            faded: card.faded,
            badge: card.badge,
            badge_running: card.badge_running,
            progress: card.progress,
            marked: card.marked,
            cover: card.cover.map(|path| path.display().to_string()),
        }
    }
}

/// A chip, with the key the frontend sends back.
#[derive(Debug, Serialize)]
pub struct ChipDto {
    key: String,
    label: String,
    count: Option<usize>,
    chosen: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct ChipsDto {
    views: Vec<ChipDto>,
    sources: Vec<ChipDto>,
    sorts: Vec<ChipDto>,
    organize: Vec<ChipDto>,
    /// The bulk chips, while games are marked.
    bulk: Vec<ChipDto>,
    /// "3 marked", while any are.
    marked: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EmptyDto {
    title: String,
    about: Option<String>,
}

/// Everything the Library needs: every game's card once, and which are shown.
#[derive(Debug, Serialize)]
pub struct ListDto {
    ready: bool,
    cards: Vec<CardDto>,
    shown: Vec<String>,
    chips: ChipsDto,
    cursor: usize,
    empty: Option<EmptyDto>,
    status: Option<String>,
    search: String,
    marked: Vec<String>,
}

/// What changes when the filter, the sort, the search or the cursor does.
#[derive(Debug, Serialize)]
pub struct FilteredDto {
    ready: bool,
    shown: Vec<String>,
    chips: ChipsDto,
    cursor: usize,
    empty: Option<EmptyDto>,
    status: Option<String>,
    search: String,
    marked: Vec<String>,
}

#[tauri::command]
pub fn library_list(state: State<'_, LibraryState>) -> ListDto {
    let browser = state
        .browser
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let cards: Vec<CardDto> = browser
        .games()
        .iter()
        .map(|game| browser.card_of(game).into())
        .collect();
    ListDto {
        ready: state.ready.load(Ordering::Relaxed),
        cards,
        shown: browser.shown_ids(),
        chips: chips(browser.chips()),
        cursor: browser.cursor(),
        empty: browser.empty().map(empty),
        status: browser.status().map(str::to_owned),
        search: browser.search().to_owned(),
        marked: browser.marked().iter().cloned().collect(),
    }
}

#[tauri::command]
pub fn library_pick(key: String, app: AppHandle, state: State<'_, LibraryState>) -> FilteredDto {
    let Some(pick) = pick_of(&key) else {
        let browser = state
            .browser
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        return filtered(&browser, state.ready.load(Ordering::Relaxed));
    };
    // The Collections chip opens the sheet, not a filter.
    if let Pick::Collections { marked } = pick {
        crate::collections::open(&app, marked);
        let browser = state
            .browser
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        return filtered(&browser, state.ready.load(Ordering::Relaxed));
    }
    let mut browser = state
        .browser
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let outcome = browser.pick(&pick);
    save(outcome.save);
    filtered(&browser, state.ready.load(Ordering::Relaxed))
}

#[tauri::command]
pub fn library_search(text: String, state: State<'_, LibraryState>) -> FilteredDto {
    let mut browser = state
        .browser
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    browser.set_search(&text);
    filtered(&browser, state.ready.load(Ordering::Relaxed))
}

/// The sort cycle, for the controller's Sort button and the keyboard.
#[tauri::command]
pub fn library_cycle_sort(state: State<'_, LibraryState>) -> FilteredDto {
    let mut browser = state
        .browser
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let outcome = browser.cycle_sort();
    save(outcome.save);
    filtered(&browser, state.ready.load(Ordering::Relaxed))
}

/// A card was clicked: the cursor goes to it.
#[tauri::command]
pub fn library_click(id: String, state: State<'_, LibraryState>) -> FilteredDto {
    let mut browser = state
        .browser
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    browser.click(&id);
    filtered(&browser, state.ready.load(Ordering::Relaxed))
}

/// The cursor moved: `left`, `right`, `up`, `down`, `page-up`, `page-down`,
/// `first` or `last`.
#[tauri::command]
pub fn library_step(step: String, state: State<'_, LibraryState>) -> FilteredDto {
    let mut browser = state
        .browser
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(step) = move_of(&step) {
        browser.step(step);
    }
    filtered(&browser, state.ready.load(Ordering::Relaxed))
}

/// The width's columns and the window's height, so paging and the cursor's
/// scrolling know the grid.
#[tauri::command]
pub fn library_fit(columns: usize, height: f32, state: State<'_, LibraryState>) -> FilteredDto {
    let mut browser = state
        .browser
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    browser.set_columns(columns);
    browser.scrolled(0.0, height);
    filtered(&browser, state.ready.load(Ordering::Relaxed))
}

/// Escape: the marks go first, then the search.
/// Marks a game, or leaves it; `None` is the card under the cursor.
#[tauri::command]
pub fn library_toggle_mark(id: Option<String>, state: State<'_, LibraryState>) -> FilteredDto {
    let mut browser = state
        .browser
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let outcome = browser.toggle_mark(id.as_deref());
    save(outcome.save);
    filtered(&browser, state.ready.load(Ordering::Relaxed))
}

#[tauri::command]
pub fn library_back(state: State<'_, LibraryState>) -> FilteredDto {
    let mut browser = state
        .browser
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    browser.back();
    filtered(&browser, state.ready.load(Ordering::Relaxed))
}

/// Shows the hidden games, or brings back the ones shown.
#[tauri::command]
pub fn library_show_hidden(state: State<'_, LibraryState>) -> FilteredDto {
    let mut browser = state
        .browser
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    browser.show_hidden();
    filtered(&browser, state.ready.load(Ordering::Relaxed))
}

fn filtered(browser: &Browser, ready: bool) -> FilteredDto {
    FilteredDto {
        ready,
        shown: browser.shown_ids(),
        chips: chips(browser.chips()),
        cursor: browser.cursor(),
        empty: browser.empty().map(empty),
        status: browser.status().map(str::to_owned),
        search: browser.search().to_owned(),
        marked: browser.marked().iter().cloned().collect(),
    }
}

fn chips(chips: Chips) -> ChipsDto {
    ChipsDto {
        views: chips.views.into_iter().map(chip).collect(),
        sources: chips.sources.into_iter().map(chip).collect(),
        sorts: chips.sorts.into_iter().map(chip).collect(),
        organize: chips.organize.into_iter().map(chip).collect(),
        bulk: chips.bulk.into_iter().map(chip).collect(),
        marked: chips.marked,
    }
}

fn chip(chip: Chip) -> ChipDto {
    ChipDto {
        key: key_of(&chip.pick),
        label: chip.label,
        count: chip.count,
        chosen: chip.chosen,
    }
}

fn empty(empty: Empty) -> EmptyDto {
    EmptyDto {
        title: empty.title,
        about: empty.about,
    }
}

/// Writes what the page asked to remember: the sort, for now.
fn save(save: Option<Save>) {
    let Some(save) = save else {
        return;
    };
    let _ = CoreState::update(|state| match save {
        Save::Sort(sort) => state.sort = sort,
        Save::Pinned(ids, pinned) => {
            for id in ids {
                state.set_pinned(&id, pinned);
            }
        }
        Save::Hidden(ids, hidden) => {
            for id in ids {
                state.set_hidden(&id, hidden);
            }
        }
        Save::Status(id, status) => state.set_status(&id, status),
    });
}

fn move_of(step: &str) -> Option<Move> {
    Some(match step {
        "left" => Move::Left,
        "right" => Move::Right,
        "up" => Move::Up,
        "down" => Move::Down,
        "page-up" => Move::PageUp,
        "page-down" => Move::PageDown,
        "first" => Move::First,
        "last" => Move::Last,
        _ => return None,
    })
}

/// The key a chip's action travels under: `view:all`, `sort:title`,
/// `source:steam`, `status:playing`, `category:Dusk`, `saved:Recent`.
fn key_of(pick: &Pick) -> String {
    match pick {
        Pick::View(view) => format!("view:{}", name_of_view(*view)),
        Pick::Clear => "clear".to_owned(),
        Pick::Source(source) => format!("source:{}", name_of_source(*source)),
        Pick::Sort(sort) => format!("sort:{}", name_of_sort(*sort)),
        Pick::Status(status) => format!("status:{}", status.name()),
        Pick::Category(name) => format!("category:{name}"),
        Pick::Saved(name) => format!("saved:{name}"),
        Pick::BulkPin(true) => "bulk-pin".to_owned(),
        Pick::BulkPin(false) => "bulk-unpin".to_owned(),
        Pick::BulkHide(true) => "bulk-hide".to_owned(),
        Pick::BulkHide(false) => "bulk-unhide".to_owned(),
        Pick::ClearMarks => "clear-marks".to_owned(),
        Pick::Collections { marked } => format!("collections:{marked}"),
    }
}

/// The action a key names, as the chips wrote it.
fn pick_of(key: &str) -> Option<Pick> {
    let (kind, value) = key.split_once(':').unwrap_or((key, ""));
    Some(match kind {
        "view" => Pick::View(view_of(value)?),
        "clear" => Pick::Clear,
        "source" => Pick::Source(source_of(value)?),
        "sort" => Pick::Sort(sort_of(value)?),
        "status" => Pick::Status(Status::parse(value)?),
        "category" => Pick::Category(value.to_owned()),
        "saved" => Pick::Saved(value.to_owned()),
        "bulk-pin" => Pick::BulkPin(true),
        "bulk-unpin" => Pick::BulkPin(false),
        "bulk-hide" => Pick::BulkHide(true),
        "bulk-unhide" => Pick::BulkHide(false),
        "clear-marks" => Pick::ClearMarks,
        "collections" => Pick::Collections {
            marked: value == "true",
        },
        _ => return None,
    })
}

fn name_of_view(view: View) -> &'static str {
    match view {
        View::All => "all",
        View::Favorites => "favorites",
        View::Recent => "recent",
        View::Hidden => "hidden",
    }
}

fn view_of(name: &str) -> Option<View> {
    Some(match name {
        "all" => View::All,
        "favorites" => View::Favorites,
        "recent" => View::Recent,
        "hidden" => View::Hidden,
        _ => return None,
    })
}

fn name_of_source(source: Source) -> &'static str {
    match source {
        Source::Steam => "steam",
        Source::Epic => "epic",
        Source::Gog => "gog",
        Source::Amazon => "amazon",
        Source::Xbox => "xbox",
        Source::Heroic => "heroic",
        Source::Desktop => "desktop",
        Source::Local => "local",
    }
}

fn source_of(name: &str) -> Option<Source> {
    Some(match name {
        "steam" => Source::Steam,
        "epic" => Source::Epic,
        "gog" => Source::Gog,
        "amazon" => Source::Amazon,
        "xbox" => Source::Xbox,
        "heroic" => Source::Heroic,
        "desktop" => Source::Desktop,
        "local" => Source::Local,
        _ => return None,
    })
}

fn name_of_sort(sort: zendo_core::Sort) -> &'static str {
    match sort {
        zendo_core::Sort::Recent => "recent",
        zendo_core::Sort::MostPlayed => "most-played",
        zendo_core::Sort::Title => "title",
    }
}

fn sort_of(name: &str) -> Option<zendo_core::Sort> {
    Some(match name {
        "recent" => zendo_core::Sort::Recent,
        "most-played" => zendo_core::Sort::MostPlayed,
        "title" => zendo_core::Sort::Title,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zendo_core::config::Config;

    #[test]
    fn every_chip_key_travels_back_to_its_action() {
        for pick in [
            Pick::View(View::Favorites),
            Pick::Clear,
            Pick::Source(Source::Gog),
            Pick::Sort(zendo_core::Sort::Title),
            Pick::Status(Status::Playing),
            Pick::Category("Dusk".to_owned()),
            Pick::Saved("Recent".to_owned()),
            Pick::BulkPin(true),
            Pick::BulkHide(false),
            Pick::ClearMarks,
            Pick::Collections { marked: true },
        ] {
            assert_eq!(pick_of(&key_of(&pick)), Some(pick.clone()), "{pick:?}");
        }
    }

    #[test]
    fn a_key_that_names_nothing_is_refused() {
        assert_eq!(pick_of("nonsense"), None);
        assert_eq!(pick_of("view:sideways"), None);
        assert_eq!(pick_of("sort:colour"), None);
    }

    #[test]
    fn moves_match_their_names() {
        assert_eq!(move_of("left"), Some(Move::Left));
        assert_eq!(move_of("page-down"), Some(Move::PageDown));
        assert_eq!(move_of("sideways"), None);
    }

    #[test]
    fn the_scan_reads_state_without_panicking() {
        // `Config::load_file` is what a scan needs; it must not need the
        // keyring, which a headless test cannot open.
        let _ = Config::load_file();
    }
}
