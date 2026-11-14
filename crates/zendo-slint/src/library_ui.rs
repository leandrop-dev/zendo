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
use zendo_core::covers::{Fetcher, Kind};
use zendo_core::{Category, Game, Library, SavedFilter, Sort, State};
use zendo_shell::gamepad::{self, Pad};
use zendo_shell::t;

use crate::covers::{Cover, Covers, Decoded};
use crate::library::{Browser, CardData, Chip, Outcome, Pick, Rows, Save, Scroll};
use crate::{Card, CardRow, ChipData, LibraryData, MainWindow, Page};

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
fn with_glue(then: impl FnOnce(&Rc<Glue>)) {
    let glue = GLUE.with(|cell| cell.borrow().clone());
    if let Some(glue) = glue {
        then(&glue);
    }
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
    /// The game whose page was asked for, until the game page exists (item 49).
    open: RefCell<Option<String>>,
    /// Off in tests, so they never write the user's files.
    persist: bool,
}

/// Puts the Library in the window and starts reading the games.
pub fn install(window: &MainWindow) {
    let glue = Glue::new(window, true);
    glue.connect(window);
    GLUE.with(|cell| *cell.borrow_mut() = Some(Rc::clone(&glue)));
    *glue.timer.borrow_mut() = Some(glue.watch());
    let weak = window.as_weak();
    gamepad::events(move |pad| {
        weak.upgrade_in_event_loop(move |_| with_glue(|glue| glue.pad(pad)))
            .is_ok()
    });
    glue.sync();
    glue.rescan();
}

impl Glue {
    fn new(window: &MainWindow, persist: bool) -> Rc<Self> {
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
        let data = window.global::<LibraryData>();
        data.set_rows(ModelRc::from(Rc::clone(&grid)));
        data.set_views(ModelRc::from(Rc::clone(&chips.views)));
        data.set_sources(ModelRc::from(Rc::clone(&chips.sources)));
        data.set_sorts(ModelRc::from(Rc::clone(&chips.sorts)));
        data.set_organize(ModelRc::from(Rc::clone(&chips.organize)));
        data.set_bulk(ModelRc::from(Rc::clone(&chips.bulk)));
        Rc::new(Self {
            window: window.as_weak(),
            browser,
            covers,
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
            if let Some(pick) = pick {
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
        });
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
        } else if browser.problems() > 0 {
            t!("footer-unreadable", count = browser.problems())
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
        if !self.covers.borrow_mut().deliver(path, pixels) || self.covers_due.replace(true) {
            return;
        }
        slint::Timer::single_shot(COVER_DELAY, || with_glue(|glue| glue.show_covers()));
    }

    fn show_covers(&self) {
        self.covers_due.set(false);
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
            let scanned = std::panic::catch_unwind(scan).unwrap_or_else(|_| {
                let library = Library {
                    problems: vec![t!("panicked")],
                    ..Library::default()
                };
                let note = t!("panicked");
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
        let wanted: Vec<(Game, Kind)> = {
            let browser = self.browser.borrow();
            [Kind::Cover, Kind::Hero]
                .into_iter()
                .flat_map(|kind| browser.games().iter().map(move |game| (game, kind)))
                .filter(|(game, kind)| fetcher.wants(*game, *kind))
                .map(|(game, kind)| (game.clone(), kind))
                .collect()
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
                let result = fetcher.fetch(&game, kind).map_err(|err| format!("{err:#}"));
                let id = game.id;
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
        if pad == Pad::Back {
            self.back();
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
                // Game Mode and its shelves come in item 53.
                Pad::Back | Pad::ToggleGameMode | Pad::PrevShelf | Pad::NextShelf => false,
            };
        }
    }

    // From the window to the browser.

    /// Opens a game's page: until the game page exists, a stand-in.
    fn open_game(&self, id: &str) {
        let title = {
            let mut browser = self.browser.borrow_mut();
            browser.click(id);
            browser.selected().map(|game| game.title.clone())
        };
        self.sync_cursor();
        if let Some(title) = title {
            *self.open.borrow_mut() = Some(id.to_owned());
            if let Some(window) = self.window.upgrade() {
                window.set_open_title(title.into());
            }
        }
    }

    fn close_game(&self) -> bool {
        let was_open = self.open.borrow_mut().take().is_some();
        if was_open && let Some(window) = self.window.upgrade() {
            window.set_open_title("".into());
        }
        was_open
    }

    /// Escape: a game's page closes, then the marks go, then the search.
    fn back(&self) {
        if self.close_game() {
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
        window.set_page(page);
    }

    /// A key the focused field did not use. Says whether it was taken.
    fn key(self: &Rc<Self>, text: &str, ctrl: bool, alt: bool, meta: bool) -> bool {
        let mut chars = text.chars();
        let (Some(key), None) = (chars.next(), chars.next()) else {
            return false;
        };
        if ctrl && !alt && !meta {
            return self.control_key(key);
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
            '5' | ',' => self.navigate(Page::Settings),
            '7' => self.navigate(Page::Stats),
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

    fn open_selected(&self) -> bool {
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
    use zendo_core::{Launch, Source, View};
    use zendo_shell::format;

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
    fn page(count: usize) -> (MainWindow, Rc<Glue>) {
        i_slint_backend_testing::init_no_event_loop();
        let window = MainWindow::new().expect("the window opens");
        let glue = Glue::new(&window, false);
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
        assert_eq!(window.get_open_title(), "Game 06");
        // Open, the page takes no arrows; Escape closes it.
        assert!(!press(&glue, Key::DownArrow));
        data.invoke_back();
        assert_eq!(window.get_open_title(), "");
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
        // Off the Library, typing is not a search.
        assert!(!glue.key("x", false, false, false));
        assert_eq!(data.get_search(), "");
        assert!(glue.key("1", true, false, false));
        assert_eq!(window.get_page(), Page::Library);
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
    fn a_tab_press_lets_go_of_an_open_game_the_marks_and_the_message() {
        let (window, glue) = page(3);
        let data = window.global::<LibraryData>();
        assert!(press(&glue, Key::Delete));
        data.invoke_card_marked("desktop:1".into());
        data.invoke_card_clicked("desktop:2".into());
        assert_eq!(window.get_open_title(), "Game 02");
        window.invoke_page_requested(Page::Store);
        assert_eq!(window.get_page(), Page::Store);
        assert_eq!(window.get_open_title(), "");
        assert_eq!(data.get_marked(), "");
        assert_eq!(data.get_status(), "");
        // The Library's own tab, pressed over an open game, closes it.
        window.invoke_page_requested(Page::Library);
        data.invoke_card_clicked("desktop:2".into());
        window.invoke_page_requested(Page::Library);
        assert_eq!(window.get_open_title(), "");
        assert_eq!(window.get_page(), Page::Library);
    }
}
