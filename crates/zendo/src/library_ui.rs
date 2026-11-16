//! The Library page in the window: `library::Browser`'s state, carried to
//! the Slint models and properties, and the window's callbacks carried back.
//! Everything here runs on the window's thread; the scan runs on one of its
//! own and comes back through the event loop.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::Duration;

use slint::platform::Key;
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Model, ModelNotify, ModelRc, ModelTracker, VecModel, Weak};
use zendo_core::browse::Move;
use zendo_core::config::Config;
use zendo_core::covers::{Artwork, Fetcher, Kind};
use zendo_core::{Category, Library, SavedFilter, Sort, State};
use zendo_shell::gamepad::{self, Pad};
use zendo_shell::links;
use zendo_shell::presence::PresenceHandle;
use zendo_shell::t;

use crate::covers::{Cover, Covers, Decoded};
use crate::library::{Browser, CardData, Chip, Outcome, Pick, Rows, Save, Scroll};
use crate::theme::Themes;
use crate::{Card, CardRow, ChipData, LibraryData, MainWindow, Page};

#[path = "accounts_ui.rs"]
mod accounts;
#[path = "artwork_ui.rs"]
mod artwork;
#[path = "cards_ui.rs"]
mod cards;
#[path = "collections_ui.rs"]
mod collections;
#[path = "compat_ui.rs"]
mod compat;
#[path = "downloads_ui.rs"]
mod downloads;
#[path = "engines_ui.rs"]
mod engines;
#[path = "game_ui.rs"]
mod game;
#[path = "game_mode_ui.rs"]
mod game_mode;
#[path = "gog_ui.rs"]
mod gog;
#[path = "installer_ui.rs"]
mod installer;
#[path = "game_jobs_ui.rs"]
mod jobs;
#[path = "local_ui.rs"]
mod local;
#[path = "mods_ui.rs"]
mod mods;
#[path = "nexus_ui.rs"]
mod nexus;
#[path = "options_ui.rs"]
mod options;
#[path = "overlay_ui.rs"]
mod overlay;
#[path = "prefix_ui.rs"]
mod prefix;
#[path = "protons_ui.rs"]
mod protons;
#[path = "runtimes_ui.rs"]
mod runtimes;
#[path = "search_ui.rs"]
mod search;
#[path = "settings_ui.rs"]
mod settings;
#[path = "stats_ui.rs"]
mod stats;
#[path = "steam_ui.rs"]
mod steam;
#[path = "steam_installs_ui.rs"]
mod steam_installs;
#[path = "store_ui.rs"]
mod store;

/// How long covers that came in are let to gather before the rows show them.
const COVER_DELAY: Duration = Duration::from_millis(40);

/// How often the launchers' files are looked at for a change.
const WATCH: Duration = Duration::from_secs(10);

/// What a scan reads: the games, the saved sort, the user's categories and
/// saved views.
type Scanned = (
    Library,
    Sort,
    Vec<Category>,
    Vec<SavedFilter>,
    Result<Config, String>,
);

thread_local! {
    /// The page's glue, for work that comes back from a thread: the event
    /// loop's closures must be `Send`, so they cannot carry it.
    static GLUE: RefCell<Option<Rc<Glue>>> = const { RefCell::new(None) };
}

/// Runs `then` with the page's glue, when there is one.
/// What the window says when work on a thread hit a bug: the panic's own
/// words when it carried any, as in Iced.
fn panicked_note(payload: &(dyn std::any::Any + Send)) -> String {
    let said = payload.downcast_ref::<String>().cloned().or_else(|| {
        payload
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
    });
    match said {
        Some(said) if !said.is_empty() => t!("panicked-with", error = said),
        _ => t!("panicked"),
    }
}

fn with_glue(then: impl FnOnce(&Rc<Glue>)) {
    let glue = GLUE.with(|cell| cell.borrow().clone());
    if let Some(glue) = glue {
        then(&glue);
    }
}

/// The window is to close for good: says whether it may. While a download
/// runs it may not, and the Downloads page asks first.
pub fn quit_requested() -> bool {
    let mut may = true;
    with_glue(|glue| may = glue.quit_requested());
    may
}

/// The KDE colour scheme changed and the window was painted again: Settings
/// says which scheme it follows.
pub fn scheme_followed() {
    with_glue(|glue| glue.show_settings());
}

/// The grid as Slint reads it: a model of rows, each made when asked, so only
/// the rows near the screen cost anything.
struct Grid {
    browser: Rc<RefCell<Browser>>,
    covers: Rc<RefCell<Covers>>,
    notify: ModelNotify,
}

impl Model for Grid {
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
        let cards: Vec<Card> = (first..last)
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

fn to_card(card: CardData, covers: &mut Covers) -> Card {
    let cover = card.cover.as_deref().map(|path| covers.get(path));
    let loading = matches!(cover, Some(Cover::Loading));
    let cover = match cover {
        Some(Cover::Ready(image)) => Some(image),
        _ => None,
    };
    Card {
        loading,
        id: card.id.into(),
        title: card.title.into(),
        caption: card.caption.into(),
        caption_accent: card.caption_accent,
        faded: card.faded,
        badge: card.badge.unwrap_or_default().into(),
        badge_running: card.badge_running,
        platform: card.platform.into(),
        progress: card.progress.unwrap_or(-1.0),
        marked: card.marked,
        has_cover: cover.is_some(),
        cover: cover.unwrap_or_default(),
    }
}

/// The chip rows' models.
struct Chips {
    views: Rc<VecModel<ChipData>>,
    sources: Rc<VecModel<ChipData>>,
    sorts: Rc<VecModel<ChipData>>,
    organize: Rc<VecModel<ChipData>>,
    bulk: Rc<VecModel<ChipData>>,
}

/// Where the source is and where problems are reported, on the About page.
const REPOSITORY: &str = "https://github.com/leandrop-dev/zendo";

struct Glue {
    window: Weak<MainWindow>,
    browser: Rc<RefCell<Browser>>,
    covers: Rc<RefCell<Covers>>,
    /// A redraw of the rows is already waiting for more covers to arrive.
    covers_due: Cell<bool>,
    /// The settings as the last scan read them, for the cover downloads.
    config: RefCell<Option<Config>>,
    /// Images still to come, how many failed, and whether more games came in
    /// while they downloaded.
    images_left: Cell<usize>,
    images_failed: Cell<usize>,
    images_again: Cell<bool>,
    /// The launchers' files as last seen, and whether a look is under way.
    signature: RefCell<Option<Vec<(PathBuf, u64)>>>,
    watching: Cell<bool>,
    /// The watch's timer, kept for as long as the page lives.
    timer: RefCell<Option<slint::Timer>>,
    grid: Rc<Grid>,
    chips: Chips,
    /// What each chip does, by the id the window sends back.
    picks: RefCell<Vec<Pick>>,
    scanning: Cell<bool>,
    /// The game whose page is open.
    open: RefCell<Option<String>>,
    /// The game page's own state (`game_ui.rs`).
    game: game::GameState,
    /// The Add game sheet and Remove (`local_ui.rs`).
    local: local::LocalState,
    /// The Launch options sheet (`options_ui.rs`).
    options: options::OptionsState,
    /// The art picker (`artwork_ui.rs`).
    art: artwork::ArtState,
    /// The Collections sheet (`collections_ui.rs`).
    collections: collections::CollectionsState,
    /// The Accounts page (`accounts_ui.rs`) and Steam's card on it
    /// (`steam_ui.rs`).
    accounts: accounts::AccountsState,
    steam: steam::SteamState,
    /// The Store page and a store game's page (`store_ui.rs`).
    store: store::StoreState,
    /// The Downloads page and the queue (`downloads_ui.rs`).
    downloads: downloads::DownloadsState,
    /// The Stats page (`stats_ui.rs`).
    stats: stats::StatsState,
    /// Game Mode (`game_mode_ui.rs`).
    game_mode: game_mode::GameModeState,
    /// Steam's installs (`steam_installs_ui.rs`) and what an installed game's
    /// page does with the queue (`game_jobs_ui.rs`).
    steam_installs: steam_installs::SteamInstallState,
    jobs: jobs::JobsState,
    /// What the stores and sites say about the open game (`cards_ui.rs`).
    cards: cards::CardsState,
    /// How the open game plays on Linux: anti-cheat, the Deck, cloud saves
    /// (`compat_ui.rs`).
    compat: compat::CompatState,
    /// The Mods card (`mods_ui.rs`).
    mods: mods::ModsUiState,
    /// A mod's FOMOD installer sheet (`installer_ui.rs`).
    installer: installer::InstallerUiState,
    /// Nexus Mods: the key, links and downloads (`nexus_ui.rs`).
    nexus: nexus::NexusUiState,
    /// Search Nexus (`search_ui.rs`).
    search: search::SearchUiState,
    /// The Wine prefix card (`prefix_ui.rs`).
    prefix: prefix::PrefixState,
    /// GOG's changelog and total time played (`gog_ui.rs`).
    gog: gog::GogState,
    /// The Settings page (`settings_ui.rs`).
    settings: settings::SettingsState,
    /// The Engines, Anti-cheat and Epic overlay cards (`engines_ui.rs`,
    /// `runtimes_ui.rs`, `overlay_ui.rs`) and the Proton group
    /// (`protons_ui.rs`).
    engines: engines::EnginesState,
    runtimes: runtimes::RuntimesState,
    overlay: overlay::OverlayState,
    protons: protons::ProtonsState,
    /// The window's colours, shared with the watcher of the KDE scheme.
    themes: Rc<RefCell<Themes>>,
    /// Discord's rich presence, while the setting is on.
    presence: RefCell<Option<PresenceHandle>>,
    /// Off in tests, so they never write the user's files.
    persist: bool,
}

/// Puts the Library in the window and starts reading the games.
pub fn install(
    window: &MainWindow,
    themes: Rc<RefCell<Themes>>,
    link: Option<String>,
    game_mode: bool,
) {
    let glue = Glue::new(window, true, themes);
    glue.connect(window);
    if game_mode {
        glue.start_game_mode();
    }
    glue.start_presence();
    GLUE.with(|cell| *cell.borrow_mut() = Some(Rc::clone(&glue)));
    *glue.timer.borrow_mut() = Some(glue.watch());
    let weak = window.as_weak();
    gamepad::events(move |pad| {
        weak.upgrade_in_event_loop(move |_| with_glue(|glue| glue.pad(pad)))
            .is_ok()
    });
    // `nxm://` links: the one the window was started for waits for the
    // library, and the ones other `zendo` processes pass over while it is
    // open arrive on a thread of their own.
    if let Some(link) = link {
        glue.nexus_link(&link);
    }
    let weak = window.as_weak();
    links::listen(move |text| {
        weak.upgrade_in_event_loop(move |_| with_glue(|glue| glue.nexus_link(&text)))
            .is_ok()
    });
    glue.sync();
    glue.show_store();
    glue.load_store_cache();
    glue.load_queue();
    glue.start_steam_installs();
    glue.rescan();
    glue.check_accounts();
    glue.show_accounts();
}

impl Glue {
    fn new(window: &MainWindow, persist: bool, themes: Rc<RefCell<Themes>>) -> Rc<Self> {
        let browser = Rc::new(RefCell::new(Browser::new()));
        // A worker's cover goes to the window's thread, which keeps it.
        let weak = Mutex::new(window.as_weak());
        let covers = Rc::new(RefCell::new(Covers::new(move |path, pixels| {
            let window = weak
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let _ = window.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.cover_ready(path, pixels));
            });
        })));
        let grid = Rc::new(Grid {
            browser: Rc::clone(&browser),
            covers: Rc::clone(&covers),
            notify: ModelNotify::default(),
        });
        let chips = Chips {
            views: Rc::new(VecModel::default()),
            sources: Rc::new(VecModel::default()),
            sorts: Rc::new(VecModel::default()),
            organize: Rc::new(VecModel::default()),
            bulk: Rc::new(VecModel::default()),
        };
        let about = window.global::<crate::AboutData>();
        about.set_version(env!("CARGO_PKG_VERSION").into());
        about.set_repository(REPOSITORY.into());
        let data = window.global::<LibraryData>();
        data.set_rows(ModelRc::from(Rc::clone(&grid)));
        data.set_views(ModelRc::from(Rc::clone(&chips.views)));
        data.set_sources(ModelRc::from(Rc::clone(&chips.sources)));
        data.set_sorts(ModelRc::from(Rc::clone(&chips.sorts)));
        data.set_organize(ModelRc::from(Rc::clone(&chips.organize)));
        data.set_bulk(ModelRc::from(Rc::clone(&chips.bulk)));
        let store = store::StoreState::new(&covers);
        Rc::new(Self {
            window: window.as_weak(),
            browser,
            covers,
            store,
            downloads: downloads::DownloadsState::new(),
            stats: stats::StatsState::new(),
            game_mode: game_mode::GameModeState::new(),
            steam_installs: steam_installs::SteamInstallState::new(),
            jobs: jobs::JobsState::new(),
            steam: steam::SteamState::default(),
            grid,
            chips,
            covers_due: Cell::new(false),
            config: RefCell::new(None),
            images_left: Cell::new(0),
            images_failed: Cell::new(0),
            images_again: Cell::new(false),
            signature: RefCell::new(None),
            watching: Cell::new(false),
            timer: RefCell::new(None),
            picks: RefCell::new(Vec::new()),
            scanning: Cell::new(false),
            open: RefCell::new(None),
            game: game::GameState::new(persist),
            local: local::LocalState::default(),
            options: options::OptionsState::default(),
            art: artwork::ArtState::new(persist),
            collections: collections::CollectionsState::new(),
            accounts: accounts::AccountsState::new(),
            cards: cards::CardsState::new(),
            compat: compat::CompatState::new(persist),
            mods: mods::ModsUiState::new(),
            installer: installer::InstallerUiState::new(),
            nexus: nexus::NexusUiState::new(),
            search: search::SearchUiState::new(),
            prefix: prefix::PrefixState::new(),
            gog: gog::GogState::new(),
            settings: settings::SettingsState::new(),
            engines: engines::EnginesState::default(),
            runtimes: runtimes::RuntimesState::default(),
            overlay: overlay::OverlayState::default(),
            protons: protons::ProtonsState::default(),
            themes,
            presence: RefCell::new(None),
            persist,
        })
    }

    /// Hooks the window's callbacks up.
    fn connect(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<LibraryData>();
        let glue = Rc::clone(self);
        data.on_chip(move |id| {
            let pick = usize::try_from(id)
                .ok()
                .and_then(|id| glue.picks.borrow().get(id).cloned());
            if let Some(Pick::Collections { marked }) = pick {
                let targets = if marked {
                    glue.marked_games()
                } else {
                    std::collections::BTreeSet::new()
                };
                glue.open_collections(targets);
            } else if let Some(pick) = pick {
                let outcome = glue.browser.borrow_mut().pick(&pick);
                glue.apply(outcome);
                glue.focus_page();
            }
        });
        let glue = Rc::clone(self);
        data.on_card_clicked(move |id| {
            glue.open_game(&id);
            glue.focus_page();
        });
        let glue = Rc::clone(self);
        data.on_card_marked(move |id| {
            let outcome = glue.browser.borrow_mut().toggle_mark(Some(&id));
            glue.apply(outcome);
            glue.focus_page();
        });
        let glue = Rc::clone(self);
        data.on_columns_changed(move |columns| {
            let columns = usize::try_from(columns).unwrap_or(1);
            let outcome = glue.browser.borrow_mut().set_columns(columns);
            glue.apply(outcome);
        });
        let glue = Rc::clone(self);
        data.on_scrolled(move |top, height| glue.browser.borrow_mut().scrolled(top, height));
        let glue = Rc::clone(self);
        data.on_search_edited(move |text| {
            let outcome = glue.browser.borrow_mut().set_search(&text);
            glue.apply(outcome);
        });
        let glue = Rc::clone(self);
        window.on_page_requested(move |page| glue.navigate(page));
        let glue = Rc::clone(self);
        data.on_refresh(move || glue.rescan());
        let glue = Rc::clone(self);
        data.on_relabel(move || {
            glue.apply(Outcome {
                rows: Rows::All,
                ..Outcome::default()
            });
            // The chips and the footer say their words again too.
            glue.sync();
            glue.show_game();
        });
        self.connect_game(window);
        self.connect_local(window);
        self.connect_options(window);
        self.connect_art(window);
        self.connect_collections(window);
        self.connect_accounts(window);
        self.connect_downloads(window);
        self.connect_stats(window);
        self.connect_game_mode(window);
        self.connect_steam(window);
        self.connect_steam_installs(window);
        self.connect_game_jobs(window);
        self.connect_store(window);
        self.connect_cards(window);
        self.connect_compat(window);
        self.connect_mods(window);
        self.connect_installer(window);
        self.connect_nexus(window);
        self.connect_search(window);
        self.connect_prefix(window);
        self.connect_gog(window);
        self.connect_settings(window);
        let glue = Rc::clone(self);
        data.on_back(move || glue.back());
        let glue = Rc::clone(self);
        data.on_key(move |text, ctrl, alt, meta, _shift| glue.key(&text, ctrl, alt, meta));
    }

    // From the browser to the window.

    /// Carries an outcome to the window: the write to `state.json`, the rows
    /// to draw again, the scroll, and whatever else changed.
    fn apply(&self, outcome: Outcome) {
        let Outcome { save, rows, scroll } = outcome;
        let changed = save.is_some() || rows != Rows::Same;
        if let Some(save) = save {
            self.persist(&save);
        }
        match rows {
            Rows::Same => {}
            Rows::Marks(places) => {
                let columns = self.browser.borrow().columns();
                for place in places {
                    self.grid.notify.row_changed(place / columns);
                }
            }
            Rows::All => self.grid.notify.reset(),
        }
        self.scroll(&scroll);
        self.sync_cursor();
        // The chips are counted over every game: only what changed what the
        // page lists needs them again.
        if changed {
            self.sync();
        }
    }

    fn persist(&self, save: &Save) {
        if !self.persist {
            return;
        }
        let saved = State::update(|state| match save {
            Save::Sort(sort) => state.sort = *sort,
            Save::Pinned(ids, pinned) => {
                for id in ids {
                    state.set_pinned(id, *pinned);
                }
            }
            Save::Hidden(ids, hidden) => {
                for id in ids {
                    state.set_hidden(id, *hidden);
                }
            }
            Save::Status(id, status) => state.set_status(id, *status),
        });
        if let Err(err) = saved {
            self.browser.borrow_mut().say(Some(format!("{err:#}")));
        }
    }

    fn scroll(&self, scroll: &Scroll) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let top = match scroll {
            Scroll::Keep => return,
            Scroll::Top => 0.0,
            Scroll::To(y) => *y,
        };
        window.global::<LibraryData>().set_viewport_y(-top);
    }

    fn sync_cursor(&self) {
        if let Some(window) = self.window.upgrade() {
            let cursor = self.browser.borrow().cursor();
            window
                .global::<LibraryData>()
                .set_cursor(i32::try_from(cursor).unwrap_or(0));
        }
    }

    /// Everything the page shows apart from the cards: the chips, what stands
    /// in for the grid, the footer and the count on the tab.
    fn sync(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<LibraryData>();
        let browser = self.browser.borrow();
        let chips = browser.chips();

        let mut picks: Vec<Pick> = Vec::new();
        let mut convert = |list: Vec<Chip>| -> Vec<ChipData> {
            list.into_iter()
                .map(|chip| {
                    let id = i32::try_from(picks.len()).unwrap_or(i32::MAX);
                    picks.push(chip.pick);
                    ChipData {
                        id,
                        label: chip.label.into(),
                        count: chip
                            .count
                            .map(|count| count.to_string())
                            .unwrap_or_default()
                            .into(),
                        chosen: chip.chosen,
                    }
                })
                .collect()
        };
        self.chips.views.set_vec(convert(chips.views));
        self.chips.sources.set_vec(convert(chips.sources));
        self.chips.sorts.set_vec(convert(chips.sorts));
        self.chips.organize.set_vec(convert(chips.organize));
        self.chips.bulk.set_vec(convert(chips.bulk));
        *self.picks.borrow_mut() = picks;
        data.set_marked(chips.marked.unwrap_or_default().into());

        let (title, about) = browser
            .empty()
            .map_or((String::new(), String::new()), |empty| {
                (empty.title, empty.about.unwrap_or_default())
            });
        data.set_empty_title(title.into());
        data.set_empty_about(about.into());
        data.set_scanning(self.scanning.get());
        data.set_hints(browser.hints().into());
        data.set_status(self.status(&browser).into());
        data.set_cursor(i32::try_from(browser.cursor()).unwrap_or(0));
        // What Zendo shows in the search is what it filters by.
        if data.get_search() != browser.search() {
            data.set_search(browser.search().into());
        }
        let shown = browser.games().iter().filter(|game| !game.hidden).count();
        window.set_library_count(shown.to_string().into());
    }

    /// Writes the footer's left side again.
    fn sync_status(&self) {
        if let Some(window) = self.window.upgrade() {
            let status = self.status(&self.browser.borrow());
            window.global::<LibraryData>().set_status(status.into());
        }
    }

    /// The footer's left side: what Zendo is doing or last did.
    fn status(&self, browser: &Browser) -> String {
        if self.images_left.get() > 0 {
            t!("images-downloading", count = self.images_left.get())
        } else if self.scanning.get() {
            t!("library-looking")
        } else if let Some(status) = browser.status() {
            status.to_owned()
        } else if let Some(summary) = self.queue_summary() {
            summary
        } else if !browser.problems().is_empty() {
            t!("footer-unreadable", count = browser.problems().len())
        } else {
            String::new()
        }
    }

    fn focus_page(&self) {
        if let Some(window) = self.window.upgrade() {
            window.invoke_focus_page();
        }
    }

    // Covers.

    /// A worker decoded a cover. The rows near the screen are drawn again a
    /// moment later, once for all the covers that came in that time.
    fn cover_ready(self: &Rc<Self>, path: PathBuf, pixels: Decoded) {
        let shown = self.page_shows(&path);
        let delivered = self.covers.borrow_mut().deliver(path, pixels);
        if delivered && shown {
            self.show_game();
        }
        if !delivered || self.covers_due.replace(true) {
            return;
        }
        slint::Timer::single_shot(COVER_DELAY, || with_glue(|glue| glue.show_covers()));
    }

    fn show_covers(&self) {
        self.covers_due.set(false);
        self.show_game_mode();
        let (rows, near) = {
            let browser = self.browser.borrow();
            (
                browser.len().div_ceil(browser.columns()),
                browser.rows_near(),
            )
        };
        for row in near.start..near.end.min(rows) {
            self.grid.notify.row_changed(row);
        }
        self.show_store_covers();
    }

    // Reading the games.

    /// Reads every source again on a thread of its own, unless that is going on.
    fn rescan(self: &Rc<Self>) {
        if self.scanning.replace(true) {
            return;
        }
        let outcome = self.browser.borrow_mut().pick(&Pick::ClearMarks);
        self.apply(outcome);
        self.sync();
        let window = self.window.clone();
        std::thread::spawn(move || {
            let scanned = std::panic::catch_unwind(scan).unwrap_or_else(|payload| {
                let note = panicked_note(&*payload);
                let library = Library {
                    problems: vec![note.clone()],
                    ..Library::default()
                };
                (library, Sort::default(), Vec::new(), Vec::new(), Err(note))
            });
            let _ = window.upgrade_in_event_loop(move |_| with_glue(|glue| glue.scanned(scanned)));
        });
    }

    fn scanned(self: &Rc<Self>, (library, sort, categories, saved, config): Scanned) {
        self.scanning.set(false);
        match config {
            Ok(config) => *self.config.borrow_mut() = Some(config),
            Err(note) => self.browser.borrow_mut().say(Some(note)),
        }
        let outcome = self
            .browser
            .borrow_mut()
            .loaded(library, Some(sort), categories, saved);
        self.apply(outcome);
        self.sync();
        self.store_scanned();
        self.nexus_scanned();
        self.refresh_game();
        self.after_scan();
        self.check_new_updates();
        self.download_images();
    }

    /// Fetches the covers and wide images that are missing, one at a time, on
    /// a thread of its own: covers first, since the grid shows them.
    fn download_images(self: &Rc<Self>) {
        if !self.persist {
            return;
        }
        if self.images_left.get() > 0 {
            self.images_again.set(true);
            return;
        }
        let Some(mut fetcher) = self.config.borrow().as_ref().and_then(Fetcher::new) else {
            return;
        };
        // The library's games, and the store games not in it (an installed
        // one comes with the library's). Every cover first: the grids show
        // them.
        let wanted: Vec<(Box<dyn Artwork + Send>, Kind)> = {
            let browser = self.browser.borrow();
            let owned = self.store.browser.borrow().images_wanted();
            let mut wanted: Vec<(Box<dyn Artwork + Send>, Kind)> = Vec::new();
            for kind in [Kind::Cover, Kind::Hero] {
                wanted.extend(
                    browser
                        .games()
                        .iter()
                        .filter(|game| fetcher.wants(*game, kind))
                        .map(|game| (Box::new(game.clone()) as Box<dyn Artwork + Send>, kind)),
                );
                wanted.extend(
                    owned
                        .iter()
                        .filter(|game| fetcher.wants(*game, kind))
                        .map(|game| (Box::new(game.clone()) as Box<dyn Artwork + Send>, kind)),
                );
            }
            wanted
        };
        if wanted.is_empty() {
            return;
        }
        self.images_left.set(wanted.len());
        self.images_failed.set(0);
        self.sync();
        let window = self.window.clone();
        std::thread::spawn(move || {
            for (game, kind) in wanted {
                let result = fetcher
                    .fetch(game.as_ref(), kind)
                    .map_err(|err| format!("{err:#}"));
                let id = game.id().to_owned();
                let sent = window.upgrade_in_event_loop(move |_| {
                    with_glue(|glue| glue.image_arrived(&id, kind, result));
                });
                if sent.is_err() {
                    return;
                }
            }
            let _ = window.upgrade_in_event_loop(|_| with_glue(Glue::images_done));
        });
    }

    fn image_arrived(&self, id: &str, kind: Kind, result: Result<Option<PathBuf>, String>) {
        self.images_left
            .set(self.images_left.get().saturating_sub(1));
        match result {
            Ok(Some(path)) => {
                // A store game's id is not a library game's, and the other
                // way round: only one of them takes it.
                self.store_image(id, kind, &path);
                let outcome = self.browser.borrow_mut().set_image(id, kind, path);
                self.apply(outcome);
            }
            Ok(None) => {}
            Err(_) => self.images_failed.set(self.images_failed.get() + 1),
        }
        self.sync_status();
    }

    fn images_done(self: &Rc<Self>) {
        self.images_left.set(0);
        if self.images_failed.get() > 0 {
            let note = t!("images-failed", count = self.images_failed.get());
            self.browser.borrow_mut().say(Some(note));
        }
        self.sync_status();
        if self.images_again.replace(false) {
            self.download_images();
        }
    }

    // The launchers' files.

    /// Looks at the launchers' files every so often, and reads the games
    /// again when they changed. The first look only notes them.
    fn watch(self: &Rc<Self>) -> slint::Timer {
        let timer = slint::Timer::default();
        if self.persist {
            timer.start(slint::TimerMode::Repeated, WATCH, || {
                with_glue(|glue| glue.watch_tick());
            });
        }
        timer
    }

    fn watch_tick(&self) {
        if self.scanning.get() || self.watching.replace(true) {
            return;
        }
        let window = self.window.clone();
        std::thread::spawn(move || {
            let signature = zendo_core::watch::signature();
            let _ = window.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.watch_checked(signature));
            });
        });
    }

    fn watch_checked(self: &Rc<Self>, signature: Vec<(PathBuf, u64)>) {
        self.watching.set(false);
        let changed = self
            .signature
            .borrow()
            .as_ref()
            .is_some_and(|previous| *previous != signature);
        *self.signature.borrow_mut() = Some(signature);
        if changed {
            self.rescan();
        }
    }

    // The controller.

    /// What a controller asked for, while the window has the focus.
    fn pad(self: &Rc<Self>, pad: Pad) {
        let focused = self
            .window
            .upgrade()
            .and_then(|window| {
                window
                    .window()
                    .with_winit_window(slint::winit_030::winit::window::Window::has_focus)
            })
            .unwrap_or(false);
        if !focused {
            return;
        }
        if self.installer_is_open() {
            self.installer_pad(pad);
            return;
        }
        if self.form_open()
            || self.collections_open()
            || self.options_open()
            || self.art_open_now()
            || self.nexus_choosing()
            || self.search_is_open()
        {
            // The sheet takes the keyboard; the pad only closes it.
            if pad == Pad::Back {
                self.back();
            }
        } else if pad == Pad::ToggleGameMode {
            self.toggle_game_mode();
        } else if self.game_mode_screen() {
            self.game_mode_pad(pad);
        } else if pad == Pad::Back {
            self.back();
        } else if self.asking() {
            if pad == Pad::Activate {
                self.close_anyway();
            }
        } else if self.on_library() {
            match pad {
                Pad::Left => self.step(Move::Left),
                Pad::Right => self.step(Move::Right),
                Pad::Up => self.step(Move::Up),
                Pad::Down => self.step(Move::Down),
                Pad::Activate => self.open_selected(),
                Pad::Sort => self.change(Browser::cycle_sort),
                Pad::Hide => self.change(Browser::toggle_hidden),
                Pad::ShowHidden => self.change(Browser::show_hidden),
                // Game Mode and its shelves are `game_mode_ui`'s.
                Pad::Back | Pad::ToggleGameMode | Pad::PrevShelf | Pad::NextShelf => false,
            };
        } else if self.on_stats() {
            match pad {
                Pad::Left => self.stats_step(Move::Left),
                Pad::Right => self.stats_step(Move::Right),
                _ => false,
            };
        } else if self.on_store() {
            match pad {
                Pad::Left => self.store_step(Move::Left),
                Pad::Right => self.store_step(Move::Right),
                Pad::Up => self.store_step(Move::Up),
                Pad::Down => self.store_step(Move::Down),
                Pad::Activate => self.open_selected_owned(),
                Pad::Sort => self.store_cycle_sort(),
                // Game Mode and its shelves are `game_mode_ui`'s.
                Pad::Back
                | Pad::Hide
                | Pad::ShowHidden
                | Pad::ToggleGameMode
                | Pad::PrevShelf
                | Pad::NextShelf => false,
            };
        } else if self.store_page_open() {
            // On a store game's page: A opens an installed game, or installs.
            if pad == Pad::Activate {
                self.owned_activate();
            }
        } else if self.open.borrow().is_some() {
            // On a game's page: A plays, Y makes it a favorite, X hides it.
            match pad {
                Pad::Activate => self.play(),
                Pad::Sort => self.toggle_pinned(),
                Pad::Hide => self.toggle_hidden(),
                _ => {}
            }
        }
    }

    // From the window to the browser.

    /// Escape: a game's page closes, then the marks go, then the search.
    fn back(&self) {
        // The installer is over everything, so it hears Escape first.
        if self.installer_back()
            || self.nexus_back()
            || self.search_back()
            || self.art_back()
            || self.options_back()
            || self.shots_back()
            || self.stay_open()
            || self.collections_back()
            || self.close_form()
            || self.close_game()
            || self.game_mode_back()
        {
            return;
        }
        if self.on_store() {
            self.store_back();
            return;
        }
        let outcome = self.browser.borrow_mut().back();
        self.apply(outcome);
    }

    fn navigate(&self, page: Page) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        // The same tab again closes a game's page, and nothing else.
        if window.get_page() == page {
            self.close_game();
            return;
        }
        if window.get_page() == Page::Library {
            let outcome = self.browser.borrow_mut().pick(&Pick::ClearMarks);
            self.apply(outcome);
        }
        self.close_game();
        self.browser.borrow_mut().say(None);
        self.sync_status();
        if page == Page::Settings {
            self.open_settings();
        }
        if page == Page::Stats {
            self.show_stats();
        }
        if page == Page::Accounts {
            // Who is signed in is asked again each time the page opens.
            self.check_accounts();
            self.show_accounts();
        }
        window.set_page(page);
    }

    /// A key the focused field did not use. Says whether it was taken.
    fn key(self: &Rc<Self>, text: &str, ctrl: bool, alt: bool, meta: bool) -> bool {
        // The installer takes every key, Ctrl shortcuts included.
        if self.installer_is_open() {
            return self.installer_key(text);
        }
        // Over a sheet, the fields have the keys.
        if self.art_open_now() {
            return self.art_key(text);
        }
        if self.options_open() {
            return self.options_key(text);
        }
        if self.form_open()
            || self.collections_open()
            || self.nexus_choosing()
            || self.search_is_open()
            || self.shot_open()
        {
            return false;
        }
        let mut chars = text.chars();
        let (Some(key), None) = (chars.next(), chars.next()) else {
            return false;
        };
        // F11: Game Mode on or off, from any page.
        if key == char::from(Key::F11) {
            self.toggle_game_mode();
            return true;
        }
        if ctrl && !alt && !meta {
            return self.control_key(key);
        }
        // Game Mode's screen hears the keys; none reach the grid behind it.
        if !alt && !meta && self.game_mode_screen() {
            return self.game_mode_key(key);
        }
        // The question about closing a running download: Enter is Close
        // anyway, and Escape (in `back`) keeps downloading.
        if self.asking() {
            if key == char::from(Key::Return) {
                self.close_anyway();
                return true;
            }
            return false;
        }
        if key == char::from(Key::F1) {
            self.navigate(Page::About);
            return true;
        }
        if self.open.borrow().is_some() && !alt && !meta {
            return self.game_key(key);
        }
        if self.store_page_open() && !alt && !meta {
            return self.owned_key(key);
        }
        if !alt && !meta && !key.is_control() && !is_special(key) && self.type_code(text) {
            return true;
        }
        if self.on_store() && !alt && !meta {
            return self.store_key(text, key);
        }
        if !alt && !meta && self.on_stats() {
            return self.stats_key(key);
        }
        if alt || meta || !self.on_library() {
            return false;
        }
        let step = |step| self.step(step);
        if key == char::from(Key::LeftArrow) {
            step(Move::Left)
        } else if key == char::from(Key::RightArrow) {
            step(Move::Right)
        } else if key == char::from(Key::UpArrow) {
            step(Move::Up)
        } else if key == char::from(Key::DownArrow) {
            step(Move::Down)
        } else if key == char::from(Key::PageUp) {
            step(Move::PageUp)
        } else if key == char::from(Key::PageDown) {
            step(Move::PageDown)
        } else if key == char::from(Key::Home) {
            step(Move::First)
        } else if key == char::from(Key::End) {
            step(Move::Last)
        } else if key == char::from(Key::Return) {
            self.open_selected()
        } else if key == char::from(Key::Backspace) {
            self.change(Browser::backspace)
        } else if key == char::from(Key::F5) {
            self.rescan();
            true
        } else if key == char::from(Key::Tab) {
            self.change(Browser::cycle_sort)
        } else if key == char::from(Key::Delete) {
            self.change(Browser::toggle_hidden)
        } else if key.is_control() || is_special(key) {
            false
        } else {
            // Typing with no field focused starts the search.
            let outcome = self.browser.borrow_mut().type_text(text);
            self.apply(outcome);
            true
        }
    }

    /// The keys with Control held: the pages, and what acts on the grid.
    fn control_key(self: &Rc<Self>, key: char) -> bool {
        // Some systems send Ctrl+F as the control character, not the letter.
        let letter = match key {
            '\u{1}'..='\u{1a}' => char::from(u8::try_from(u32::from(key)).unwrap_or(0) + 96),
            other => other.to_ascii_lowercase(),
        };
        match letter {
            '1' => self.navigate(Page::Library),
            '2' => self.navigate(Page::Store),
            '3' => self.navigate(Page::Downloads),
            '4' => self.navigate(Page::Accounts),
            '5' | ',' => self.navigate(Page::Settings),
            '6' => self.navigate(Page::About),
            '7' => self.navigate(Page::Stats),
            'v' if self.paste_code() => {}
            'p' if self.open.borrow().is_some() => self.toggle_pinned(),
            'f' if self.on_store() => self.with_window(MainWindow::invoke_focus_search),
            'v' if self.on_store() => self.with_window(MainWindow::invoke_paste_search),
            'r' if self.on_store() => self.refresh_stores(),
            _ if !self.on_library() => return false,
            'f' => self.with_window(MainWindow::invoke_focus_search),
            'v' => self.with_window(MainWindow::invoke_paste_search),
            'r' => self.rescan(),
            'h' => return self.change(Browser::show_hidden),
            'p' => return self.change(Browser::toggle_pinned),
            ' ' | '\0' => {
                let outcome = self.browser.borrow_mut().toggle_mark(None);
                self.apply(outcome);
            }
            _ => return false,
        }
        true
    }

    fn with_window(&self, then: impl FnOnce(&MainWindow)) {
        if let Some(window) = self.window.upgrade() {
            then(&window);
        }
    }

    fn on_library(&self) -> bool {
        self.window
            .upgrade()
            .is_some_and(|window| window.get_page() == Page::Library)
            && self.open.borrow().is_none()
    }

    fn change(&self, change: impl FnOnce(&mut Browser) -> Outcome) -> bool {
        let outcome = change(&mut self.browser.borrow_mut());
        self.apply(outcome);
        true
    }

    fn step(&self, step: Move) -> bool {
        self.change(|browser| browser.step(step))
    }

    pub(super) fn open_selected(&self) -> bool {
        let id = self.browser.borrow().selected().map(|game| game.id.clone());
        if let Some(id) = id {
            self.open_game(&id);
        }
        true
    }
}

/// Whether the character is one of Slint's private-use codes for a key that
/// has no text, so it is never typed.
fn is_special(key: char) -> bool {
    ('\u{f700}'..='\u{f8ff}').contains(&key)
}

/// Reads every source, with the saved sort, categories and saved views.
fn scan() -> Scanned {
    let state = State::load().unwrap_or_default();
    (
        Library::scan(),
        state.sort,
        state.categories(),
        state.filters(),
        Config::load().map_err(|err| format!("{err:#}")),
    )
}

#[cfg(test)]
mod tests {
    use crate::{GameData, SettingsData};
    use zendo_core::launch::{Started, Watch};
    use zendo_core::{Game, Launch, Source, View};
    use zendo_shell::format;
    use zendo_shell::running::RunMessage;

    use super::*;

    fn game(n: usize) -> Game {
        Game::new(
            format!("desktop:{n}"),
            format!("Game {n:02}"),
            Source::Desktop,
            Launch::Uri { uri: String::new() },
        )
    }

    /// A window with the page's glue and `count` games, with no thread
    /// started and nothing written to the user's files.
    pub(super) fn page(count: usize) -> (MainWindow, Rc<Glue>) {
        i_slint_backend_testing::init_no_event_loop();
        let window = MainWindow::new().expect("the window opens");
        let glue = Glue::new(&window, false, crate::theme::Themes::plain());
        glue.connect(&window);
        let library = Library {
            games: (0..count).map(game).collect(),
            problems: Vec::new(),
        };
        let outcome =
            glue.browser
                .borrow_mut()
                .loaded(library, Some(Sort::Title), Vec::new(), Vec::new());
        glue.apply(outcome);
        glue.sync();
        (window, glue)
    }

    fn chip_labels(model: &ModelRc<ChipData>) -> Vec<String> {
        model.iter().map(|chip| chip.label.to_string()).collect()
    }

    fn press(glue: &Rc<Glue>, key: Key) -> bool {
        glue.key(&String::from(char::from(key)), false, false, false)
    }

    #[test]
    fn the_grid_is_rows_of_cards_made_when_asked() {
        let (window, _glue) = page(12);
        let data = window.global::<LibraryData>();
        // Five columns to start with: 5, 5 and 2.
        assert_eq!(data.get_rows().row_count(), 3);
        let last = data.get_rows().row_data(2).expect("a third row");
        assert_eq!((last.first, last.cards.row_count()), (10, 2));
        assert_eq!(window.get_library_count(), "12");
        assert_eq!(data.get_empty_title(), "");
    }

    #[test]
    fn typing_anywhere_searches_and_escape_clears_it() {
        let (window, glue) = page(12);
        let data = window.global::<LibraryData>();
        for typed in ["g", "a", "m", "e", " ", "1"] {
            assert!(glue.key(typed, false, false, false));
        }
        assert_eq!(data.get_search(), "game 1");
        assert_eq!(data.get_rows().row_count(), 1, "Game 01, 10 and 11");
        data.invoke_back();
        assert_eq!(data.get_search(), "");
        assert_eq!(data.get_rows().row_count(), 3);
    }

    #[test]
    fn the_arrows_move_the_cursor_and_enter_opens_the_game() {
        let (window, glue) = page(12);
        let data = window.global::<LibraryData>();
        assert!(press(&glue, Key::DownArrow));
        assert_eq!(data.get_cursor(), 5);
        assert!(press(&glue, Key::RightArrow));
        assert_eq!(data.get_cursor(), 6);
        assert!(press(&glue, Key::Return));
        assert!(window.global::<GameData>().get_open());
        assert_eq!(window.global::<GameData>().get_title(), "Game 06");
        // Open, the page takes no arrows; Escape closes it.
        assert!(!press(&glue, Key::DownArrow));
        data.invoke_back();
        assert!(!window.global::<GameData>().get_open());
    }

    #[test]
    fn marks_offer_the_bulk_chips_and_escape_takes_them_back() {
        let (window, _glue) = page(4);
        let data = window.global::<LibraryData>();
        assert_eq!(chip_labels(&data.get_bulk()), Vec::<String>::new());
        data.invoke_card_marked("desktop:1".into());
        data.invoke_card_marked("desktop:2".into());
        assert_eq!(data.get_marked(), t!("library-selected", count = 2));
        assert_eq!(
            chip_labels(&data.get_bulk()),
            [
                t!("library-bulk-pin"),
                t!("library-bulk-hide"),
                t!("collections-assign"),
                t!("library-clear-selection")
            ]
        );
        let marked = data.get_rows().row_data(0).expect("a row").cards;
        assert!(marked.row_data(1).is_some_and(|card| card.marked));
        data.invoke_back();
        assert_eq!(data.get_marked(), "");
        assert_eq!(chip_labels(&data.get_bulk()), Vec::<String>::new());
    }

    #[test]
    fn a_chip_filters_and_says_why_when_nothing_is_left() {
        let (window, _glue) = page(3);
        let data = window.global::<LibraryData>();
        let favorites = data
            .get_views()
            .iter()
            .find(|chip| chip.label == format::view(View::Favorites))
            .expect("the Favorites chip");
        data.invoke_chip(favorites.id);
        assert_eq!(data.get_empty_title(), t!("library-no-favorites"));
        assert_eq!(data.get_rows().row_count(), 0);
        // The chip is chosen now.
        assert!(
            data.get_views()
                .iter()
                .any(|chip| chip.chosen && chip.id == favorites.id)
        );
    }

    #[test]
    fn control_and_a_number_changes_the_page_and_drops_the_marks() {
        let (window, glue) = page(3);
        let data = window.global::<LibraryData>();
        data.invoke_card_marked("desktop:0".into());
        assert!(glue.key("2", true, false, false));
        assert_eq!(window.get_page(), Page::Store);
        assert_eq!(data.get_marked(), "");
        // On the Store, typing is the Store's search, not the Library's.
        assert!(glue.key("x", false, false, false));
        assert_eq!(data.get_search(), "");
        assert_eq!(window.global::<crate::StoreData>().get_search(), "x");
        // Off the Library and the Store, typing is not a search.
        assert!(glue.key("4", true, false, false));
        assert!(!glue.key("x", false, false, false));
        assert_eq!(data.get_search(), "");
        assert!(glue.key("1", true, false, false));
        assert_eq!(window.get_page(), Page::Library);
    }

    #[test]
    fn accounts_and_about_are_reached_by_keys_and_by_the_settings_row() {
        let (window, glue) = page(3);
        assert!(glue.key("4", true, false, false));
        assert_eq!(window.get_page(), Page::Accounts);
        assert!(glue.key("6", true, false, false));
        assert_eq!(window.get_page(), Page::About);
        assert!(glue.key("5", true, false, false));
        assert_eq!(window.get_page(), Page::Settings);
        assert!(glue.key("1", true, false, false));
        assert!(glue.key(&char::from(Key::F1).to_string(), false, false, false));
        assert_eq!(window.get_page(), Page::About);
        window.invoke_page_requested(Page::Accounts);
        assert_eq!(window.get_page(), Page::Accounts);
    }

    #[test]
    fn the_footer_lists_the_keys_and_says_what_changed() {
        let (window, glue) = page(3);
        let data = window.global::<LibraryData>();
        assert_eq!(data.get_hints(), t!("hints-library"));
        assert!(
            press(&glue, Key::Delete),
            "Delete hides the game under the cursor"
        );
        assert_eq!(data.get_status(), t!("status-hid", title = "Game 00"));
        assert_eq!(window.get_library_count(), "2");
        data.invoke_card_marked("desktop:1".into());
        assert_eq!(data.get_hints(), t!("hints-selection"));
    }

    #[test]
    fn a_panic_says_its_own_words_when_it_had_any() {
        assert_eq!(
            panicked_note(&"a bug in the work"),
            t!("panicked-with", error = "a bug in the work")
        );
        assert_eq!(
            panicked_note(&String::from("owned words")),
            t!("panicked-with", error = "owned words")
        );
        assert_eq!(panicked_note(&42_u8), t!("panicked"));
    }

    #[test]
    fn a_game_that_leaves_while_its_page_is_open_closes_it_and_says_so() {
        let (window, glue) = page(3);
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:1".into());
        assert!(window.global::<GameData>().get_open());
        // A rescan without it, and with a source it could not read.
        let library = Library {
            games: vec![game(0), game(2)],
            problems: vec!["a.desktop: no Exec".to_owned()],
        };
        let outcome =
            glue.browser
                .borrow_mut()
                .loaded(library, Some(Sort::Title), Vec::new(), Vec::new());
        glue.apply(outcome);
        glue.sync();
        glue.refresh_game();
        assert!(!window.global::<GameData>().get_open());
        assert_eq!(window.global::<LibraryData>().get_status(), t!("game-gone"));
        assert_eq!(
            glue.browser.borrow().problems(),
            ["a.desktop: no Exec".to_owned()]
        );
    }

    #[test]
    fn a_tab_press_lets_go_of_an_open_game_the_marks_and_the_message() {
        let (window, glue) = page(3);
        let data = window.global::<LibraryData>();
        assert!(press(&glue, Key::Delete));
        data.invoke_card_marked("desktop:1".into());
        data.invoke_card_clicked("desktop:2".into());
        assert!(window.global::<GameData>().get_open());
        assert_eq!(window.global::<GameData>().get_title(), "Game 02");
        window.invoke_page_requested(Page::Store);
        assert_eq!(window.get_page(), Page::Store);
        assert!(!window.global::<GameData>().get_open());
        assert_eq!(data.get_marked(), "");
        assert_eq!(data.get_status(), "");
        // The Library's own tab, pressed over an open game, closes it.
        window.invoke_page_requested(Page::Library);
        data.invoke_card_clicked("desktop:2".into());
        window.invoke_page_requested(Page::Library);
        assert!(!window.global::<GameData>().get_open());
        assert_eq!(window.get_page(), Page::Library);
    }

    fn badge_of(window: &MainWindow, id: &str) -> (String, bool) {
        let rows = window.global::<LibraryData>().get_rows();
        for row in rows.iter() {
            if let Some(card) = row.cards.iter().find(|card| card.id == id) {
                return (card.badge.to_string(), card.badge_running);
            }
        }
        panic!("no card for {id}");
    }

    #[test]
    fn a_game_page_shows_the_game_and_its_buttons_change_it() {
        let (window, _glue) = page(3);
        let library = window.global::<LibraryData>();
        let game = window.global::<GameData>();
        library.invoke_card_clicked("desktop:1".into());
        assert_eq!(game.get_title(), "Game 01");
        assert_eq!(game.get_tiles().row_count(), 4);
        assert_eq!(game.get_run_state(), 0);
        assert_eq!(game.get_statuses().row_count(), 4);
        // Favorite: the card's badge says so, and a second press undoes it.
        game.invoke_toggle_pinned();
        assert!(game.get_pinned());
        assert_eq!(badge_of(&window, "desktop:1").0, t!("badge-favorite"));
        game.invoke_toggle_pinned();
        assert!(!game.get_pinned());
        // The status chips: the chosen one is pressed again to clear it.
        game.invoke_status(1);
        assert!(game.get_statuses().iter().any(|chip| chip.chosen));
        game.invoke_status(1);
        assert!(!game.get_statuses().iter().any(|chip| chip.chosen));
        // Hide: the page stays on the game, which leaves the grid.
        game.invoke_toggle_hidden();
        assert!(game.get_hidden());
        assert_eq!(game.get_title(), "Game 01");
        game.invoke_toggle_hidden();
        assert!(!game.get_hidden());
    }

    #[test]
    fn play_counts_a_launch_and_a_running_game_shows_its_badge_and_stop() {
        let (window, glue) = page(3);
        let game = window.global::<GameData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:2".into());
        // In tests Play hands the game to its launcher and starts nothing.
        game.invoke_play();
        assert_eq!(game.get_run_state(), 0);
        assert_eq!(
            glue.browser
                .borrow()
                .game("desktop:2")
                .map(|g| g.played.launches),
            Some(1)
        );
        // A game Zendo watches: Running, with Stop, and a badge on its card.
        glue.game.running.borrow_mut().begin("desktop:2");
        let started = Started {
            watch: Watch::Group(zendo_core::launch::Group::new(4_000_000, None)),
            log: None,
            warning: None,
        };
        glue.run_event(RunMessage::Started("desktop:2".into(), Ok(started)));
        assert_eq!(game.get_run_state(), 2);
        assert_eq!(badge_of(&window, "desktop:2"), (t!("badge-running"), true));
        glue.run_event(RunMessage::Ended("desktop:2".into()));
        assert_eq!(game.get_run_state(), 0);
        assert_eq!(badge_of(&window, "desktop:2"), (String::new(), false));
        assert_eq!(
            window.global::<LibraryData>().get_status(),
            t!("run-closed", title = "Game 02", minutes = 0)
        );
    }

    #[test]
    fn a_game_page_keys_play_and_hide_and_the_grid_hears_nothing() {
        let (window, glue) = page(3);
        let game = window.global::<GameData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:0".into());
        assert!(!press(&glue, Key::DownArrow));
        assert!(!glue.key("x", false, false, false));
        assert_eq!(window.global::<LibraryData>().get_search(), "");
        assert!(glue.key("p", true, false, false));
        assert!(game.get_pinned());
        assert!(press(&glue, Key::Delete));
        assert!(game.get_hidden());
        assert!(press(&glue, Key::Return));
        assert_eq!(
            glue.browser
                .borrow()
                .game("desktop:0")
                .map(|g| g.played.launches),
            Some(1)
        );
    }

    #[test]
    fn the_add_game_sheet_checks_the_form_and_escape_closes_it_first() {
        use crate::LocalData;
        let (window, glue) = page(3);
        let sheet = window.global::<LocalData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:1".into());
        sheet.invoke_add();
        assert!(sheet.get_open());
        assert_eq!(sheet.get_about(), t!("local-about-empty"));
        // Over the sheet the page's keys are the fields', not the grid's.
        assert!(!glue.key("x", false, false, false));
        assert!(!press(&glue, Key::Delete));
        // No title and no program: the form says so, and stays open.
        sheet.invoke_save();
        assert!(sheet.get_open());
        assert!(sheet.get_error().contains("Give the game a title"));
        sheet.invoke_program_edited("/g/Game.exe".into());
        assert_eq!(sheet.get_about(), t!("local-about-windows"));
        // Escape closes the sheet and leaves the open game's page alone.
        window.global::<LibraryData>().invoke_back();
        assert!(!sheet.get_open());
        assert!(window.global::<GameData>().get_open());
        window.global::<LibraryData>().invoke_back();
        assert!(!window.global::<GameData>().get_open());
    }

    #[test]
    fn a_good_form_adds_the_game_and_remove_asks_twice() {
        use crate::LocalData;
        use std::os::unix::fs::PermissionsExt;
        let (window, glue) = page(3);
        let sheet = window.global::<LocalData>();
        let program = std::env::temp_dir().join(format!("zendo-local-{}.sh", std::process::id()));
        std::fs::write(&program, "#!/bin/sh\n").expect("a program");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
            .expect("executable");
        sheet.invoke_add();
        sheet.invoke_program_edited(program.display().to_string().into());
        sheet.invoke_title_edited("Handmade".into());
        sheet.invoke_save();
        assert!(!sheet.get_open(), "{}", sheet.get_error());
        // The library is read again, then the note is said.
        glue.after_scan();
        assert_eq!(
            glue.browser.borrow().status(),
            Some(t!("local-added", title = "Handmade").as_str())
        );
        let _ = std::fs::remove_file(program);

        // Remove: the first press asks, the second takes the game out.
        let game = window.global::<GameData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:2".into());
        game.invoke_remove();
        assert!(game.get_confirm_remove());
        game.invoke_remove();
        assert!(!game.get_open());
        assert_eq!(
            glue.browser.borrow().status(),
            Some(t!("local-removed", title = "Game 02").as_str())
        );
    }

    #[test]
    fn the_theme_and_discord_choices_apply_at_once() {
        let (window, glue) = page(2);
        let settings = window.global::<SettingsData>();
        window.invoke_page_requested(Page::Settings);
        assert!(settings.get_themes().row_count() >= 1);
        assert_eq!(settings.get_theme_note(), "");
        settings.invoke_set_discord(true);
        assert!(settings.get_discord());
        assert!(glue.presence.borrow().is_none(), "no thread in tests");
        // A theme that is gone shows Zendo's own and says so.
        glue.save_setting(settings::Setting::Theme(Some("no-such".into())));
        assert!(settings.get_theme_note().contains("no-such"));
        assert_eq!(settings.get_theme_note_tone(), 2);
        // Back to a theme that is there: the note goes.
        settings.invoke_theme_chosen(0);
        assert_eq!(settings.get_theme_note(), "");
    }

    #[test]
    fn settings_apply_at_once_and_a_key_is_kept_only_once_checked() {
        use zendo_core::covers::KeyCheck;
        let (window, glue) = page(2);
        let settings = window.global::<SettingsData>();
        window.invoke_page_requested(Page::Settings);
        assert_eq!(window.get_page(), Page::Settings);
        assert_eq!(settings.get_groups().row_count(), 6);
        assert_eq!(settings.get_group(), 0);
        // The window's paint and the switches follow a change.
        settings.invoke_set_opaque(true);
        assert!((window.get_translucency() - 1.0).abs() < f32::EPSILON);
        assert!(settings.get_opaque());
        settings.invoke_set_opaque(false);
        assert!(window.get_translucency() < 1.0);
        settings.invoke_set_covers(true);
        assert!(settings.get_covers() && settings.get_covers_now());
        settings.invoke_workers_chosen(4);
        let chosen: Vec<_> = settings
            .get_workers()
            .iter()
            .filter(|chip| chip.chosen)
            .map(|chip| chip.id)
            .collect();
        assert_eq!(chosen, [4]);
        // The key: checked, then saved only once the site accepted it.
        settings.invoke_key_edited("abc".into());
        assert!(settings.get_key_changed());
        settings.invoke_key_save();
        assert!(settings.get_key_checking());
        glue.key_checked("abc".into(), Ok(KeyCheck::Rejected));
        assert_eq!(settings.get_key_note(), t!("steamgriddb-rejected-unsaved"));
        assert!(settings.get_key_changed(), "a rejected key is not saved");
        settings.invoke_key_save();
        glue.key_checked("other".into(), Ok(KeyCheck::Accepted));
        assert!(
            settings.get_key_checking(),
            "an answer for another key is let go"
        );
        glue.key_checked("abc".into(), Ok(KeyCheck::Accepted));
        assert_eq!(settings.get_key_note(), t!("steamgriddb-saved"));
        assert!(!settings.get_key_changed());
        // The groups, and the files the last one lists.
        settings.invoke_group_chosen(3);
        assert_eq!(settings.get_group(), 3);
        assert_eq!(settings.get_files().row_count(), 6);
        assert!(settings.get_sources().row_count() >= 7);
    }

    #[test]
    fn a_windows_game_gets_the_prefix_tools_and_gogs_changelog_and_total() {
        use zendo_core::gog_changelog::{Block, Entry};
        use zendo_core::prefix_tools::Ran;
        i_slint_backend_testing::init_no_event_loop();
        let window = MainWindow::new().expect("the window opens");
        let glue = Glue::new(&window, false, crate::theme::Themes::plain());
        glue.connect(&window);
        let mut sugar = Game::new(
            "gog:1".into(),
            "Sugar".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1".into(),
            },
        );
        sugar.details.platform = Some("windows".into());
        sugar.played.minutes = 90;
        let outcome = glue.browser.borrow_mut().loaded(
            Library {
                games: vec![sugar],
                problems: Vec::new(),
            },
            Some(Sort::Title),
            Vec::new(),
            Vec::new(),
        );
        glue.apply(outcome);
        glue.sync();
        let game = window.global::<GameData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("gog:1".into());
        assert!(game.get_prefix_show());
        assert!(game.get_prefix_free() && game.get_prefix_gog());
        // One tool at a time, and how it went.
        game.invoke_prefix_run(0);
        assert!(!game.get_prefix_free());
        assert_eq!(game.get_prefix_status_tone(), 3);
        game.invoke_prefix_run(1);
        assert!(
            game.get_prefix_status().contains("Wine settings"),
            "the first still runs"
        );
        glue.prefix_done("gog:1".into(), Ok(Ran::default()));
        assert!(game.get_prefix_free());
        assert_eq!(game.get_prefix_status(), "Wine settings ended");
        assert_eq!(game.get_prefix_status_tone(), 1);
        // An option is not a component.
        game.invoke_prefix_typed("vcrun2022 -q".into());
        game.invoke_prefix_install();
        assert!(game.get_prefix_free());
        assert_eq!(game.get_prefix_status_tone(), 2);
        game.invoke_prefix_typed("vcrun2022".into());
        game.invoke_prefix_install();
        assert!(!game.get_prefix_free(), "installing");
        glue.prefix_done(
            "gog:1".into(),
            Ok(Ran {
                installed: vec!["vcrun2022".into()],
                already: Vec::new(),
            }),
        );
        assert_eq!(game.get_prefix_components(), "", "typed text is cleared");
        // GOG's changelog: the newest update, and a few more on demand.
        assert_eq!(game.get_changelog().row_count(), 0);
        let update = |title: &str| Entry {
            title: Some(title.into()),
            blocks: vec![Block::Item("A fix".into())],
        };
        glue.changelog_arrived("gog:1".into(), Ok(vec![update("2.0"), update("1.1")]));
        assert_eq!(game.get_changelog().row_count(), 2);
        assert_eq!(
            game.get_changelog_more(),
            t!("changelog-earlier", count = 1)
        );
        game.invoke_more_changelog();
        assert_eq!(
            game.get_changelog().row_count(),
            5,
            "a gap between two updates"
        );
        assert_eq!(game.get_changelog_more(), "");
        // GOG's total time, where it is more than Zendo's count.
        let minutes = || {
            glue.browser
                .borrow()
                .game("gog:1")
                .map(|game| game.played.minutes)
        };
        glue.gog_total_arrived("gog:1", &Ok(Some(60)));
        assert_eq!(minutes(), Some(90));
        glue.gog_total_arrived("gog:1", &Ok(Some(300)));
        assert_eq!(minutes(), Some(300));
    }

    #[test]
    fn the_anti_cheat_card_leads_when_online_play_is_blocked_and_the_deck_pill_follows() {
        use zendo_core::areweanticheatyet::{Entry, Note, Status};
        use zendo_core::steam_deck::{Category, Report};
        let (window, glue) = page(3);
        let game = window.global::<GameData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:1".into());
        assert!(!game.get_ac_show());
        let entry = |status| Entry {
            name: "Rust".into(),
            status,
            anti_cheats: vec!["Easy Anti-Cheat".into()],
            notes: vec![Note {
                text: "A note".into(),
                link: Some("https://example.org/n".into()),
            }],
            latest: None,
            slug: "rust".into(),
        };
        glue.anti_cheat_arrived("desktop:1".into(), Ok(Some(entry(Status::Denied))));
        assert!(game.get_ac_show() && game.get_ac_first());
        assert_eq!(game.get_ac_label(), "DENIED");
        assert_eq!(game.get_ac_tone(), 2);
        assert_eq!(game.get_ac_makers(), "Easy Anti-Cheat");
        let notes: Vec<_> = game.get_ac_notes().iter().collect();
        assert_eq!(
            (notes.len(), notes[0].url.as_str()),
            (1, "https://example.org/n")
        );
        glue.anti_cheat_arrived("desktop:1".into(), Ok(Some(entry(Status::Supported))));
        assert!(game.get_ac_show() && !game.get_ac_first(), "after About");
        glue.anti_cheat_arrived("desktop:1".into(), Ok(None));
        assert!(!game.get_ac_show(), "not on the list");
        // A failure becomes a card with the way to ask again.
        glue.anti_cheat_arrived("desktop:1".into(), Err("offline".into()));
        let failures: Vec<_> = game.get_failures().iter().collect();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].label, t!("anti-cheat-title"));
        // Valve's review: a pill after ProtonDB's, and a failure a card.
        glue.deck_arrived(
            "desktop:1".into(),
            Ok(Some(Report {
                app: 1,
                category: Category::Verified,
                notes: Vec::new(),
            })),
        );
        let pills: Vec<_> = game.get_pills().iter().collect();
        assert_eq!(pills.len(), 1);
        assert_eq!(pills[0].label, t!("deck-verified"));
        glue.deck_arrived("desktop:1".into(), Err("offline".into()));
        assert_eq!(game.get_pills().row_count(), 0);
        assert_eq!(game.get_failures().row_count(), 2);
        // Another Steam app id: both answers are forgotten, to be asked again.
        glue.give_steam_app("desktop:1", Some(7));
        assert_eq!(game.get_failures().row_count(), 0);
    }

    #[test]
    fn the_cards_fill_from_what_the_stores_say_and_failures_offer_to_ask_again() {
        use zendo_core::achievements::{Achievement, Achievements};
        use zendo_core::owned::StoreDetails;
        use zendo_core::protondb::Rating;
        use zendo_core::requirements::{From, Requirements, Row, System};
        use zendo_core::steam_facts::Facts;
        let (window, glue) = page(3);
        let game = window.global::<GameData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:1".into());
        assert_eq!(game.get_about(), "");
        // ProtonDB found nothing: the card offers the way to give an app id.
        glue.rating_arrived("desktop:1".into(), Ok(None));
        assert_eq!(game.get_no_match(), t!("steam-app-no-match"));
        assert_eq!(game.get_pills().row_count(), 0);
        glue.rating_arrived(
            "desktop:1".into(),
            Ok(Some(Rating {
                app: 620,
                tier: Some("platinum".into()),
                reports: 12,
                native: false,
            })),
        );
        assert_eq!(game.get_pills().row_count(), 1);
        assert_eq!(game.get_no_match(), "");
        // Steam's page: the description and facts, a link for Metacritic.
        let details = StoreDetails {
            summary: Some("A game about portals.".into()),
            developer: Some("Valve".into()),
            features: vec!["Single-player".into()],
            metacritic: Some(zendo_core::owned::Metacritic {
                score: 95,
                url: "https://example.org/mc".into(),
            }),
            ..StoreDetails::default()
        };
        glue.facts_arrived(
            "desktop:1".into(),
            Ok(Some(Facts {
                details,
                shots: Vec::new(),
            })),
        );
        assert_eq!(game.get_about(), "A game about portals.");
        let rows: Vec<_> = game.get_details().iter().collect();
        assert!(rows.iter().any(|row| row.label == t!("fact-developer")));
        assert!(
            rows.iter().any(
                |row| row.label == t!("fact-metacritic") && row.url == "https://example.org/mc"
            )
        );
        // Requirements, minimum and recommended.
        glue.requirements_arrived(
            "desktop:1".into(),
            Ok(Some(Requirements {
                system: System::Windows,
                from: From::Steam,
                rows: vec![Row {
                    label: "Memory".into(),
                    minimum: Some("2 GB".into()),
                    recommended: Some("4 GB".into()),
                }],
            })),
        );
        assert_eq!(game.get_req().row_count(), 1);
        assert!(game.get_req_both());
        // Achievements: the progress shows, and fifteen show ten at first.
        let list = (0..15)
            .map(|n| Achievement {
                name: format!("A{n}"),
                description: String::new(),
                hidden: false,
                unlocked: (n < 3).then_some(1_700_000_000),
                rarity: Some(5.0),
                icon: None,
            })
            .collect();
        glue.achievements_arrived(
            "desktop:1".into(),
            Ok(Some(Achievements {
                list,
                unlocks_known: true,
            })),
        );
        assert_eq!(game.get_ach().row_count(), 10);
        assert_eq!(game.get_ach_more(), t!("achievements-more", count = 5));
        assert!((game.get_ach_share() - 0.2).abs() < 1e-6);
        game.invoke_more_achievements();
        assert_eq!(game.get_ach().row_count(), 15);
        assert_eq!(game.get_ach_more(), "");
        // The library's card now has its progress line.
        let rows = window.global::<LibraryData>().get_rows();
        let card = rows
            .iter()
            .flat_map(|row| row.cards.iter().collect::<Vec<_>>())
            .find(|card| card.id == "desktop:1")
            .expect("the card");
        assert!(card.progress > 0.0);
        // A failure is shown with its card's name, and asking again clears it.
        glue.requirements_arrived("desktop:1".into(), Err("offline".into()));
        assert_eq!(game.get_failures().row_count(), 1);
        assert_eq!(game.get_req().row_count(), 0);
    }

    #[test]
    fn a_steam_app_id_is_given_checked_and_taken_back() {
        let (window, _glue) = page(3);
        let game = window.global::<GameData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:0".into());
        assert!(game.get_steam_app_row());
        assert!(!game.get_steam_app_clearable());
        game.invoke_steam_app_save("not a number".into());
        assert_eq!(
            window.global::<LibraryData>().get_status(),
            t!("steam-app-invalid")
        );
        game.invoke_steam_app_save(" 620 ".into());
        assert!(game.get_steam_app_clearable());
        assert_eq!(game.get_steam_app_input(), "620");
        assert_eq!(
            game.get_steam_app_line(),
            t!("steam-app-by-you", app = "620".to_owned())
        );
        game.invoke_steam_app_clear();
        assert!(!game.get_steam_app_clearable());
        assert_eq!(game.get_steam_app_input(), "");
    }

    fn typed(data: &crate::CollectionsData<'_>, text: &str) {
        data.set_draft(text.into());
        data.invoke_draft_edited(text.into());
    }

    fn note_of(data: &crate::CollectionsData<'_>) -> (String, bool) {
        (data.get_note().to_string(), data.get_note_good())
    }

    #[test]
    fn a_collection_is_deleted_only_on_the_second_press() {
        let (window, glue) = page(3);
        let data = window.global::<crate::CollectionsData>();
        glue.open_collections(std::collections::BTreeSet::new());
        assert!(data.get_open());
        assert_eq!(data.get_empty(), t!("collections-empty"));
        typed(&data, "Co-op");
        data.invoke_create();
        assert_eq!(glue.browser.borrow().categories().len(), 1);
        assert_eq!(data.get_rows().row_count(), 1);
        data.invoke_delete("Co-op".into());
        assert_eq!(glue.browser.borrow().categories().len(), 1, "it only asks");
        assert!(data.get_rows().row_data(0).is_some_and(|row| row.confirm));
        data.invoke_delete("Co-op".into());
        assert_eq!(glue.browser.borrow().categories().len(), 0);
        assert_eq!(data.get_rows().row_count(), 0);
        // Any other press takes the question back.
        typed(&data, "Co-op");
        data.invoke_create();
        data.invoke_delete("Co-op".into());
        data.invoke_save_view();
        assert!(data.get_rows().row_data(0).is_some_and(|row| !row.confirm));
    }

    #[test]
    fn a_saved_view_cannot_take_a_hand_picked_name_and_the_reverse() {
        let (window, glue) = page(3);
        let data = window.global::<crate::CollectionsData>();
        glue.open_collections(std::collections::BTreeSet::new());
        typed(&data, "Co-op");
        data.invoke_create();
        typed(&data, "co-op");
        data.invoke_save_view();
        assert!(
            glue.browser.borrow().saved().is_empty(),
            "a collection's name"
        );
        assert_eq!(
            note_of(&data),
            (t!("collections-taken", name = "co-op"), false)
        );
        typed(&data, "Evening");
        data.invoke_save_view();
        assert_eq!(glue.browser.borrow().saved().len(), 1);
        typed(&data, "evening");
        data.invoke_create();
        assert_eq!(glue.browser.borrow().categories().len(), 1, "a view's name");
        // Nothing typed: nothing made.
        typed(&data, "  ");
        data.invoke_create();
        assert_eq!(note_of(&data), (t!("collections-blank"), false));
    }

    #[test]
    fn a_saved_view_is_updated_renamed_applied_and_deleted_from_the_same_sheet() {
        let (window, glue) = page(3);
        let data = window.global::<crate::CollectionsData>();
        let library = window.global::<LibraryData>();
        glue.open_collections(std::collections::BTreeSet::new());
        typed(&data, "Evening");
        data.invoke_save_view();
        assert_eq!(glue.browser.borrow().saved()[0].name, "Evening");
        // The same name keeps what the page shows now.
        library.invoke_search_edited("game 01".into());
        typed(&data, "evening");
        data.invoke_save_view();
        assert_eq!(
            glue.browser.borrow().saved().len(),
            1,
            "the same name updates"
        );
        assert_eq!(glue.browser.borrow().saved()[0].filter.search, "game 01");
        assert_eq!(
            note_of(&data),
            (t!("collections-updated", name = "evening"), true)
        );
        // Rename in place.
        data.invoke_rename("Evening".into());
        assert!(data.get_rows().row_data(0).is_some_and(|row| row.renaming));
        assert_eq!(data.get_rename_draft(), "Evening");
        data.set_rename_draft("Night".into());
        data.invoke_rename_edited("Night".into());
        data.invoke_rename_done();
        assert_eq!(glue.browser.borrow().saved()[0].name, "Night");
        assert!(data.get_rows().row_data(0).is_some_and(|row| !row.renaming));
        // Open puts the view on the page and closes the sheet.
        library.invoke_search_edited(String::new().into());
        data.invoke_apply("Night".into());
        assert!(!data.get_open());
        assert_eq!(library.get_search(), "game 01");
        glue.open_collections(std::collections::BTreeSet::new());
        data.invoke_delete("Night".into());
        data.invoke_delete("Night".into());
        assert_eq!(glue.browser.borrow().saved().len(), 0);
    }

    #[test]
    fn the_bulk_chip_opens_the_sheet_for_the_marked_games_and_a_tick_assigns_them() {
        let (window, glue) = page(3);
        let data = window.global::<crate::CollectionsData>();
        let library = window.global::<LibraryData>();
        library.invoke_card_marked("desktop:0".into());
        library.invoke_card_marked("desktop:1".into());
        let chip = library
            .get_bulk()
            .iter()
            .find(|chip| chip.label == t!("collections-assign"))
            .expect("the chip is offered while games are marked");
        library.invoke_chip(chip.id);
        assert!(data.get_open());
        typed(&data, "Co-op");
        data.invoke_create();
        // Made with the marked games in it, ticked.
        let row = data.get_rows().row_data(0).expect("one row");
        assert!(row.checkable && row.checked);
        assert_eq!(row.count, t!("count-games", count = 2));
        assert_eq!(glue.browser.borrow().games()[0].categories, ["Co-op"]);
        assert_eq!(glue.browser.borrow().games()[2].categories.len(), 0);
        data.invoke_set_member("Co-op".into(), false);
        assert_eq!(
            note_of(&data),
            (t!("collections-unassigned", count = 2), true)
        );
        assert_eq!(glue.browser.borrow().games()[0].categories.len(), 0);
        data.invoke_set_member("Co-op".into(), true);
        assert_eq!(glue.browser.borrow().games()[1].categories, ["Co-op"]);
        // Only to manage, there is no tick.
        data.invoke_close();
        glue.open_collections(std::collections::BTreeSet::new());
        assert!(
            data.get_rows()
                .row_data(0)
                .is_some_and(|row| !row.checkable)
        );
    }

    #[test]
    fn the_collections_chip_counts_and_opens_the_sheet_and_escape_leaves_in_order() {
        let (window, glue) = page(3);
        let data = window.global::<crate::CollectionsData>();
        let library = window.global::<LibraryData>();
        let chip = library
            .get_organize()
            .iter()
            .find(|chip| chip.label == t!("collections-open"))
            .expect("the way to the sheet is always there");
        assert_eq!(chip.count, "");
        library.invoke_chip(chip.id);
        assert!(data.get_open());
        typed(&data, "Co-op");
        data.invoke_create();
        let chip = library
            .get_organize()
            .iter()
            .find(|chip| chip.label == t!("collections-open"))
            .expect("still there");
        assert_eq!(chip.count, "1");
        // Over the sheet the page's keys are the fields', not the grid's.
        assert!(!press(&glue, Key::DownArrow));
        // Escape: a rename first, then the sheet.
        data.invoke_rename("Co-op".into());
        glue.back();
        assert!(data.get_open());
        assert!(data.get_rows().row_data(0).is_some_and(|row| !row.renaming));
        glue.back();
        assert!(!data.get_open());
    }

    #[test]
    fn a_game_page_lists_its_collections_and_edits_them_in_the_sheet() {
        let (window, glue) = page(3);
        let game = window.global::<GameData>();
        let data = window.global::<crate::CollectionsData>();
        glue.open_game("desktop:1");
        assert_eq!(game.get_collections().row_count(), 0);
        game.invoke_edit_collections();
        assert!(data.get_open());
        typed(&data, "Co-op");
        data.invoke_create();
        assert_eq!(game.get_collections().row_count(), 1, "made with this game");
        data.invoke_close();
        // Escape closes the sheet and leaves the open game's page alone.
        game.invoke_edit_collections();
        glue.back();
        assert!(!data.get_open());
        assert!(game.get_open());
    }

    /// A GOG sign-in with its window open, as `open_window` leaves it.
    fn with_sign_in_window() -> (MainWindow, Rc<Glue>) {
        let (window, glue) = page(2);
        window.global::<crate::AccountsData>().invoke_start(1);
        glue.give_sign_in_a_window();
        (window, glue)
    }

    fn card(window: &MainWindow, at: usize) -> crate::AccountCard {
        window
            .global::<crate::AccountsData>()
            .get_cards()
            .row_data(at)
            .expect("three cards")
    }

    #[test]
    fn the_count_beside_accounts_is_the_stores_signed_in_and_gog_gets_its_switches() {
        use zendo_core::stores::Store;
        let (window, glue) = page(2);
        let data = window.global::<crate::AccountsData>();
        assert_eq!(window.get_accounts_count(), "");
        glue.account_checked(Store::Epic, Ok(None), None);
        glue.account_checked(Store::Gog, Ok(Some("me".into())), None);
        assert_eq!(
            window.get_accounts_count(),
            t!("top-bar-signed-in", count = 1, stores = 4)
        );
        assert!(card(&window, 1).gog && !card(&window, 0).gog);
        assert_eq!(
            card(&window, 1).status,
            t!("accounts-signed-in-as", name = "me")
        );
        // No engine found: Sign in cannot be pressed, and the card says why.
        assert!(!card(&window, 0).action_on);
        assert!(!card(&window, 0).engine_ok);
        // The switches are on until turned off, and are the config's.
        assert!(data.get_gog_playtime() && data.get_gog_galaxy());
        data.invoke_set_gog_playtime(false);
        data.invoke_set_gog_galaxy(false);
        assert!(!glue.settings_config().gog_playtime);
        assert!(!glue.settings_config().gog_galaxy);
        assert!(!data.get_gog_playtime() && !data.get_gog_galaxy());
        // Signed out takes the switches and the count away.
        glue.account_checked(Store::Gog, Ok(None), None);
        assert!(!card(&window, 1).gog);
        assert_eq!(window.get_accounts_count(), "");
    }

    #[test]
    fn closing_the_sign_in_window_ends_the_sign_in() {
        use zendo_core::stores::Store;
        let (window, glue) = with_sign_in_window();
        assert_eq!(card(&window, 1).flow, 1);
        glue.window_ended(Store::Gog, Ok(None));
        assert!(glue.sign_in_state().is_none());
        assert_eq!(card(&window, 1).flow, 0);
    }

    #[test]
    fn the_windows_code_signs_in_without_pasting() {
        use zendo_core::stores::Store;
        let (window, glue) = with_sign_in_window();
        glue.window_ended(Store::Gog, Ok(Some("code".into())));
        // The code went to gogdl (not in tests), and the window is let go.
        assert_eq!(glue.sign_in_state(), Some((true, false, false, None)));
        assert_eq!(card(&window, 1).line, t!("accounts-signing-in"));
    }

    #[test]
    fn without_a_window_the_browser_takes_over() {
        use zendo_core::stores::Store;
        let (window, glue) = with_sign_in_window();
        glue.window_ended(
            Store::Gog,
            Err("the sign-in window failed: no WebKitGTK".into()),
        );
        let (_, _, browser, note) = glue.sign_in_state().expect("still signing in");
        assert!(browser);
        assert!(note.is_some_and(|note| note.contains("browser")));
        assert_eq!(card(&window, 1).flow, 2);
    }

    #[test]
    fn going_to_the_browser_closes_the_window() {
        use zendo_core::stores::Store;
        let (_window, glue) = with_sign_in_window();
        glue.use_browser();
        // The window, closed for it, ends; the sign-in carries on by paste.
        glue.window_ended(Store::Gog, Ok(None));
        let (_, window_open, browser, _) = glue.sign_in_state().expect("still signing in");
        assert!(browser && !window_open);
    }

    #[test]
    fn a_pasted_text_with_no_code_is_refused_and_a_cancel_ends_the_sign_in() {
        let (window, glue) = page(2);
        let data = window.global::<crate::AccountsData>();
        data.invoke_start(0);
        data.invoke_use_browser();
        assert!(!card(&window, 0).finish_on, "nothing pasted yet");
        data.invoke_code_edited("this is not a code".into());
        assert!(card(&window, 0).finish_on);
        data.invoke_finish();
        assert_eq!(glue.sign_in_state().map(|state| state.0), Some(false));
        assert_eq!(card(&window, 0).note, t!("accounts-no-code-epic"));
        // Typing again takes the note away; Cancel closes the steps.
        data.invoke_code_edited("this is not a code either".into());
        assert_eq!(card(&window, 0).note, "");
        data.invoke_cancel();
        assert!(glue.sign_in_state().is_none());
        assert_eq!(card(&window, 0).flow, 0);
    }

    #[test]
    fn text_typed_on_the_page_starts_the_code_and_only_there() {
        let (window, glue) = page(2);
        let data = window.global::<crate::AccountsData>();
        data.invoke_start(0);
        data.invoke_use_browser();
        // Not on the Accounts page: typing is the library's search.
        assert!(glue.key("x", false, false, false));
        assert_eq!(data.get_code(), "");
        assert_eq!(window.global::<LibraryData>().get_search(), "x");
        window.invoke_page_requested(Page::Accounts);
        assert!(glue.key("a", false, false, false));
        assert_eq!(data.get_code(), "a");
        // With something there, the field has the keys.
        assert!(!glue.key("b", false, false, false));
    }

    #[test]
    fn an_answer_after_cancel_still_says_how_the_account_stands_and_sign_out_works() {
        use zendo_core::stores::Store;
        let (window, glue) = page(2);
        glue.sign_in_finished(Store::Gog, Ok("me".into()));
        assert!(glue.account_is_signed_in(Store::Gog));
        assert_eq!(
            window.global::<LibraryData>().get_status(),
            t!("accounts-signed-in-to", store = "GOG", name = "me")
        );
        window.global::<crate::AccountsData>().invoke_sign_out(1);
        assert!(!glue.account_is_signed_in(Store::Gog));
    }

    #[test]
    fn control_v_pastes_into_the_code_only_on_accounts_in_the_browser_steps() {
        let (window, glue) = page(2);
        let data = window.global::<crate::AccountsData>();
        // On the Library it is the search's paste; the code field is not asked.
        assert!(glue.key("v", true, false, false));
        assert_eq!(data.get_paste_code(), 0);
        window.invoke_page_requested(Page::Accounts);
        data.invoke_start(0);
        // In Zendo's window there is no field to paste into.
        assert!(!glue.key("v", true, false, false));
        assert_eq!(data.get_paste_code(), 0);
        data.invoke_use_browser();
        assert!(glue.key("v", true, false, false));
        assert_eq!(data.get_paste_code(), 1);
    }
}
