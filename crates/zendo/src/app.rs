//! The window: a sidebar of pages (Library, Settings, About), the library as
//! a grid of covers with store and sort chips, and a page for each game with
//! its art, actions and details. Keyboard and controller drive all of it;
//! missing covers and wide images download in the background.

mod about;
mod format;
mod game;
mod library;
mod settings;
mod sidebar;
mod style;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;

use iced::futures::channel::{mpsc, oneshot};
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::scrollable::{AbsoluteOffset, Viewport};
use iced::widget::{Id, column, container, operation, row, space, text};
use iced::{Element, Event, Length, Size, Subscription, Task, event, window};
use zendo_core::config::Config;
use zendo_core::covers::{Fetcher, KeyCheck, Kind};
use zendo_core::{Game, Library, Sort, Source, State};

use crate::gamepad::{self, Pad};
use style::DIM;

const START_SIZE: Size = Size::new(1360.0, 860.0);

fn grid_id() -> Id {
    Id::new("grid")
}

fn game_scroll_id() -> Id {
    Id::new("game")
}

fn search_id() -> Id {
    Id::new("search")
}

fn key_id() -> Id {
    Id::new("steamgriddb-key")
}

pub fn run(fullscreen: bool) -> anyhow::Result<()> {
    iced::application(Zendo::boot, Zendo::update, Zendo::view)
        .title("Zendo")
        .theme(style::theme())
        .subscription(Zendo::subscription)
        .window(window::Settings {
            size: START_SIZE,
            min_size: Some(Size::new(library::min_width(), 560.0)),
            fullscreen,
            ..window::Settings::default()
        })
        .run()?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Page {
    Library,
    /// The game with this id.
    Game(String),
    Settings,
    About,
}

#[derive(Debug, Clone, Copy)]
enum Move {
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    First,
    Last,
}

#[derive(Debug, Clone)]
enum Message {
    Loaded(Library, Sort, Result<Config, String>),
    Refresh,
    /// An image download finished: the file, none found, or what went wrong.
    Image(String, Kind, Result<Option<PathBuf>, String>),
    ImagesDone,
    Resized(window::Id, Size),
    Focused(window::Id, bool),
    Scrolled(Viewport),
    Navigate(Page),
    Move(Move),
    /// The search field's new text.
    Search(String),
    /// Text typed while no field has focus: it goes to the search.
    Type(String),
    Backspace,
    /// Escape, B: clears the search, or goes back to the library.
    Back,
    /// Enter, A: opens the selected game, or plays the one on its page.
    Activate,
    Clicked(usize),
    Play,
    SetSort(Sort),
    CycleSort,
    SetSource(Option<Source>),
    /// Hides the current game, or brings it back when it is hidden.
    ToggleHidden,
    TogglePinned,
    ShowHidden,
    ToggleFullscreen,
    FocusSearch,
    /// A folder or web page, opened with `xdg-open`.
    Open(String),
    KeyInput(String),
    ToggleKeyShown,
    SaveKey,
    CheckKey,
    KeyChecked(Result<KeyCheck, String>),
    SetDownloadCovers(bool),
    DownloadNow,
    Pad(Pad),
}

/// The Settings page's key field and what was last said about it.
#[derive(Debug, Default)]
struct KeyForm {
    value: String,
    shown: bool,
    /// The key `SteamGridDB` is being asked about.
    checking: Option<String>,
    /// Keep the key once `SteamGridDB` has answered.
    save_after_check: bool,
    note: Option<(String, bool)>,
}

#[derive(Debug, Default)]
struct Downloads {
    left: usize,
    failed: usize,
}

#[expect(clippy::struct_excessive_bools, reason = "independent window flags")]
struct Zendo {
    /// `None` while the first scan runs.
    library: Option<Library>,
    scanning: bool,
    page: Page,
    search: String,
    /// The store chip chosen; `None` is "All".
    source: Option<Source>,
    /// Index into the games the filters let through.
    selected: usize,
    columns: usize,
    window: Option<window::Id>,
    focused: bool,
    fullscreen: bool,
    scroll_top: f32,
    viewport_height: f32,
    status: Option<String>,
    sort: Sort,
    show_hidden: bool,
    downloads: Downloads,
    config: Config,
    key: KeyForm,
    /// Off in tests, so they never touch the user's files or the network.
    persist: bool,
}

impl Zendo {
    fn boot() -> (Self, Task<Message>) {
        let zendo = Self {
            library: None,
            scanning: true,
            page: Page::Library,
            search: String::new(),
            source: None,
            selected: 0,
            columns: library::columns_for(START_SIZE.width),
            window: None,
            focused: true,
            fullscreen: false,
            scroll_top: 0.0,
            viewport_height: START_SIZE.height,
            status: None,
            sort: Sort::default(),
            show_hidden: false,
            downloads: Downloads::default(),
            config: Config::default(),
            key: KeyForm::default(),
            persist: true,
        };
        // The window's id, so F11 works before the first resize event.
        let window = window::latest().and_then(|id| Task::done(Message::Focused(id, true)));
        (zendo, Task::batch([scan(), window]))
    }

    /// The games the grid shows, in its order.
    fn visible(&self) -> Vec<&Game> {
        let Some(library) = &self.library else {
            return Vec::new();
        };
        let words: Vec<String> = self
            .search
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let mut games: Vec<&Game> = library
            .games
            .iter()
            .filter(|game| self.show_hidden || !game.hidden)
            .filter(|game| self.source.is_none_or(|source| game.source == source))
            .filter(|game| {
                let title = game.title.to_lowercase();
                words.iter().all(|word| title.contains(word.as_str()))
            })
            .collect();
        games.sort_by(|a, b| self.sort.compare(a, b));
        games
    }

    fn selected_id(&self) -> Option<String> {
        self.visible()
            .get(self.selected)
            .map(|game| game.id.clone())
    }

    /// The game the actions apply to: the page's, or the selected card's.
    fn current_id(&self) -> Option<String> {
        match &self.page {
            Page::Game(id) => Some(id.clone()),
            Page::Library => self.selected_id(),
            Page::Settings | Page::About => None,
        }
    }

    fn game(&self, id: &str) -> Option<&Game> {
        self.library.as_ref()?.find(id)
    }

    fn game_mut(&mut self, id: &str) -> Option<&mut Game> {
        self.library
            .as_mut()?
            .games
            .iter_mut()
            .find(|game| game.id == id)
    }

    /// Keeps the cursor on the same game after the order or filters change.
    fn select(&mut self, id: Option<&str>) -> Task<Message> {
        let visible = self.visible();
        let index = id.and_then(|id| visible.iter().position(|game| game.id == id));
        self.selected = index.unwrap_or_else(|| self.selected.min(visible.len().saturating_sub(1)));
        self.reveal()
    }

    /// Writes a change to the state file, saying so when it fails.
    fn save(&mut self, change: impl FnOnce(&mut State)) {
        if !self.persist {
            return;
        }
        if let Err(err) = State::update(change) {
            self.status = Some(format!("{err:#}"));
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded(library, sort, config) => self.loaded(library, sort, config),
            Message::Refresh => {
                if self.scanning {
                    return Task::none();
                }
                self.scanning = true;
                self.status = Some("Looking for games…".to_owned());
                scan()
            }
            Message::Image(id, kind, result) => {
                self.image_arrived(&id, kind, result);
                Task::none()
            }
            Message::ImagesDone => {
                self.images_done();
                Task::none()
            }
            Message::Resized(id, size) => {
                self.window = Some(id);
                self.columns = library::columns_for(size.width);
                self.reveal()
            }
            Message::Focused(id, focused) => {
                self.window = Some(id);
                self.focused = focused;
                Task::none()
            }
            Message::Scrolled(viewport) => {
                self.scroll_top = viewport.absolute_offset().y;
                self.viewport_height = viewport.bounds().height;
                Task::none()
            }
            Message::Navigate(page) => self.navigate(page),
            Message::Move(step) if self.page == Page::Library => self.step(step),
            Message::Search(search) => {
                self.search = search;
                self.filter_changed()
            }
            Message::Type(typed) if self.page == Page::Settings => self.type_key(&typed),
            Message::Type(typed) if self.page == Page::Library => {
                self.search.push_str(&typed);
                self.filter_changed()
            }
            Message::Backspace if self.page == Page::Library => {
                self.search.pop();
                self.filter_changed()
            }
            Message::Back => self.back(),
            Message::Activate => match &self.page {
                Page::Library => self.open_selected(),
                Page::Game(_) => self.play(),
                Page::Settings | Page::About => Task::none(),
            },
            Message::Clicked(index) => {
                self.selected = index;
                self.open_selected()
            }
            Message::Play => self.play(),
            Message::SetSort(sort) => self.set_sort(sort),
            Message::CycleSort if self.page == Page::Library => self.set_sort(self.sort.next()),
            Message::SetSource(source) => {
                let id = self.selected_id();
                self.source = source;
                self.select(id.as_deref())
            }
            Message::ToggleHidden => self.toggle_hidden(),
            Message::TogglePinned => self.toggle_pinned(),
            Message::ShowHidden => {
                let id = self.selected_id();
                self.show_hidden = !self.show_hidden;
                self.select(id.as_deref())
            }
            Message::ToggleFullscreen => self.toggle_fullscreen(),
            // From another page the search is not on screen to focus yet;
            // typing reaches it there all the same.
            Message::FocusSearch if self.page != Page::Library => self.navigate(Page::Library),
            Message::FocusSearch => operation::focus(search_id()),
            Message::Open(target) => {
                if self.persist {
                    open(&target);
                }
                Task::none()
            }
            Message::KeyInput(_)
            | Message::ToggleKeyShown
            | Message::SaveKey
            | Message::CheckKey
            | Message::KeyChecked(_)
            | Message::SetDownloadCovers(_)
            | Message::DownloadNow => self.update_settings(message),
            Message::Pad(_) if !self.focused => Task::none(),
            Message::Pad(pad) => self.pad(pad),
            Message::Move(_) | Message::Type(_) | Message::Backspace | Message::CycleSort => {
                Task::none()
            }
        }
    }

    fn loaded(
        &mut self,
        library: Library,
        sort: Sort,
        config: Result<Config, String>,
    ) -> Task<Message> {
        let first = self.library.is_none();
        let id = self.selected_id();
        self.library = Some(library);
        self.scanning = false;
        self.status = None;
        if first {
            self.sort = sort;
            match config {
                Ok(config) => {
                    self.key.value = config.steamgriddb_key.clone().unwrap_or_default();
                    self.config = config;
                }
                Err(err) => self.status = Some(err),
            }
        }
        // A game whose page is open may be gone after a rescan.
        if let Page::Game(id) = &self.page
            && self.game(id).is_none()
        {
            self.page = Page::Library;
        }
        let select = self.select(id.as_deref());
        Task::batch([select, self.download_images()])
    }

    fn navigate(&mut self, page: Page) -> Task<Message> {
        if self.page == page {
            return Task::none();
        }
        self.page = page;
        self.status = None;
        match &self.page {
            Page::Game(_) => {
                operation::scroll_to(game_scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 })
            }
            // The grid is built anew; put it back where the cursor is.
            Page::Library => operation::scroll_to(
                grid_id(),
                AbsoluteOffset {
                    x: 0.0,
                    y: self.scroll_top,
                },
            ),
            Page::Settings | Page::About => Task::none(),
        }
    }

    fn back(&mut self) -> Task<Message> {
        match self.page {
            Page::Library if self.search.is_empty() => Task::none(),
            Page::Library => {
                self.search.clear();
                self.filter_changed()
            }
            _ => self.navigate(Page::Library),
        }
    }

    fn open_selected(&mut self) -> Task<Message> {
        match self.selected_id() {
            Some(id) => self.navigate(Page::Game(id)),
            None => Task::none(),
        }
    }

    fn set_sort(&mut self, sort: Sort) -> Task<Message> {
        let id = self.selected_id();
        self.sort = sort;
        self.save(|state| state.sort = sort);
        self.select(id.as_deref())
    }

    fn toggle_fullscreen(&mut self) -> Task<Message> {
        let Some(id) = self.window else {
            return Task::none();
        };
        self.fullscreen = !self.fullscreen;
        let mode = if self.fullscreen {
            window::Mode::Fullscreen
        } else {
            window::Mode::Windowed
        };
        window::set_mode(id, mode)
    }

    /// A controller button, by page: on a game's page Y pins and X hides.
    fn pad(&mut self, pad: Pad) -> Task<Message> {
        let on_game = matches!(self.page, Page::Game(_));
        self.update(match pad {
            Pad::Left => Message::Move(Move::Left),
            Pad::Right => Message::Move(Move::Right),
            Pad::Up => Message::Move(Move::Up),
            Pad::Down => Message::Move(Move::Down),
            Pad::Activate => Message::Activate,
            Pad::Back => Message::Back,
            Pad::Sort if on_game => Message::TogglePinned,
            Pad::Sort => Message::CycleSort,
            Pad::Hide => Message::ToggleHidden,
            Pad::ShowHidden => Message::ShowHidden,
            Pad::ToggleFullscreen => Message::ToggleFullscreen,
        })
    }

    fn image_arrived(&mut self, id: &str, kind: Kind, result: Result<Option<PathBuf>, String>) {
        self.downloads.left = self.downloads.left.saturating_sub(1);
        match result {
            Ok(Some(path)) => {
                if let Some(game) = self.game_mut(id) {
                    match kind {
                        Kind::Cover => game.cover = Some(path),
                        Kind::Hero => game.hero = Some(path),
                    }
                }
            }
            Ok(None) => {}
            Err(_) => self.downloads.failed += 1,
        }
    }

    fn images_done(&mut self) {
        self.downloads.left = 0;
        if self.downloads.failed > 0 {
            self.status = Some(format!(
                "{} could not be downloaded; `zendo covers` says why",
                format::count(self.downloads.failed, "image", "images")
            ));
        }
    }

    fn toggle_hidden(&mut self) -> Task<Message> {
        let Some(id) = self.current_id() else {
            return Task::none();
        };
        let Some(game) = self.game_mut(&id) else {
            return Task::none();
        };
        game.hidden = !game.hidden;
        let (hidden, title) = (game.hidden, game.title.clone());
        self.save(|state| state.set_hidden(&id, hidden));
        self.status = Some(if hidden {
            format!("Hid {title} · Ctrl+H shows hidden games")
        } else {
            format!("{title} is back in the library")
        });
        // A hidden game leaves the grid; the cursor stays in place.
        if self.page == Page::Library {
            self.select(Some(&id))
        } else {
            Task::none()
        }
    }

    fn toggle_pinned(&mut self) -> Task<Message> {
        let Some(id) = self.current_id() else {
            return Task::none();
        };
        let Some(game) = self.game_mut(&id) else {
            return Task::none();
        };
        game.pinned = !game.pinned;
        let (pinned, title) = (game.pinned, game.title.clone());
        self.save(|state| state.set_pinned(&id, pinned));
        self.status = Some(if pinned {
            format!("Pinned {title}: it leads the library")
        } else {
            format!("Unpinned {title}")
        });
        self.select(Some(&id))
    }

    fn filter_changed(&mut self) -> Task<Message> {
        self.selected = 0;
        self.status = None;
        operation::scroll_to(grid_id(), AbsoluteOffset { x: 0.0, y: 0.0 })
    }

    fn step(&mut self, step: Move) -> Task<Message> {
        let count = self.visible().len();
        if count == 0 {
            return Task::none();
        }
        let last = count - 1;
        let page = self.columns * self.rows_on_screen();
        let current = self.selected.min(last);
        self.selected = match step {
            Move::Left => current.saturating_sub(1),
            Move::Right => (current + 1).min(last),
            Move::Up => current.checked_sub(self.columns).unwrap_or(current),
            Move::Down => {
                let below = current + self.columns;
                // From the row above the last, a short last row still counts.
                if below <= last || current / self.columns < last / self.columns {
                    below.min(last)
                } else {
                    current
                }
            }
            Move::PageUp => current.saturating_sub(page),
            Move::PageDown => (current + page).min(last),
            Move::First => 0,
            Move::Last => last,
        };
        self.reveal()
    }

    fn rows_on_screen(&self) -> usize {
        // A whole number of rows, at least one; heights are never negative.
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let rows = ((self.viewport_height + library::SPACING)
            / (library::CARD_HEIGHT + library::SPACING)) as usize;
        rows.max(1)
    }

    /// Scrolls just enough to show the selected card.
    fn reveal(&self) -> Task<Message> {
        if self.page != Page::Library {
            return Task::none();
        }
        #[expect(clippy::cast_precision_loss, reason = "row counts are small")]
        let row = (self.selected / self.columns.max(1)) as f32;
        let top = row * (library::CARD_HEIGHT + library::SPACING);
        let bottom = top + library::CARD_HEIGHT + 2.0 * library::GRID_PADDING;
        let y = if top < self.scroll_top {
            top
        } else if bottom > self.scroll_top + self.viewport_height {
            bottom - self.viewport_height
        } else {
            return Task::none();
        };
        operation::scroll_to(grid_id(), AbsoluteOffset { x: 0.0, y })
    }

    /// Starts the game on the page, or the selected one in the library.
    fn play(&mut self) -> Task<Message> {
        let Some(game) = self.current_id().and_then(|id| self.game(&id)).cloned() else {
            return Task::none();
        };
        let result = if self.persist {
            zendo_core::launch(&game)
        } else {
            Ok(())
        };
        match result {
            Ok(()) => {
                self.status = Some(format!("Starting {}…", game.title));
                // Zendo's state now has the launch; show it in the order
                // without scanning again.
                if let Some(played) = self.game_mut(&game.id).map(|game| &mut game.played) {
                    played.launches += 1;
                    played.last = Some(now());
                }
                self.select(Some(&game.id))
            }
            Err(err) => {
                self.status = Some(format!("{err:#}"));
                Task::none()
            }
        }
    }

    /// Starts downloading the covers and wide images that are missing, one
    /// at a time, in a thread of their own.
    fn download_images(&mut self) -> Task<Message> {
        if !self.persist || self.downloads.left > 0 {
            return Task::none();
        }
        let Some(fetcher) = Fetcher::new(&self.config) else {
            return Task::none();
        };
        let wanted: Vec<(Game, Kind)> = self
            .library
            .iter()
            .flat_map(|library| &library.games)
            .flat_map(|game| [(game, Kind::Cover), (game, Kind::Hero)])
            .filter(|(game, kind)| fetcher.wants(game, *kind))
            .map(|(game, kind)| (game.clone(), kind))
            .collect();
        if wanted.is_empty() {
            return Task::none();
        }
        self.downloads = Downloads {
            left: wanted.len(),
            failed: 0,
        };
        Task::run(fetch_images(fetcher, wanted), |message| message)
    }

    fn subscription(_: &Self) -> Subscription<Message> {
        Subscription::batch([
            event::listen_with(on_event),
            Subscription::run(gamepad::events).map(Message::Pad),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        let page = match &self.page {
            Page::Library => self.library_page(),
            Page::Game(id) => match self.game(id) {
                Some(game) => self.game_page(game),
                None => centered(text("This game is no longer in the library").color(DIM)),
            },
            Page::Settings => self.settings_page(),
            Page::About => about::page(),
        };
        let main = column![page, self.footer()].width(Length::Fill);
        row![self.sidebar(), main].into()
    }

    fn footer(&self) -> Element<'_, Message> {
        let problems = self
            .library
            .as_ref()
            .map_or(0, |library| library.problems.len());
        // A game's page shows the status beside its actions.
        let status = if matches!(self.page, Page::Game(_)) {
            &None
        } else {
            &self.status
        };
        let left = match (status, problems) {
            _ if self.downloads.left > 0 => {
                text(format!("Downloading images, {} left", self.downloads.left))
            }
            (Some(status), _) => text(status.clone()),
            (None, 0) => text(""),
            (None, n) => text(format!(
                "{} could not be read; Settings names them",
                format::count(n, "file", "files")
            )),
        };
        let hints = match self.page {
            Page::Library => {
                "Enter open · Tab sort · Ctrl+P pin · Del hide · Ctrl+H hidden · F11 full-screen"
            }
            Page::Game(_) => "Enter play · Esc back · Ctrl+P pin · Del hide · F11 full-screen",
            Page::Settings | Page::About => "Esc back · F11 full-screen",
        };
        row![
            left.size(13).color(DIM),
            space::horizontal(),
            text(hints).size(13).color(DIM),
        ]
        .spacing(16)
        .padding([10, 24])
        .into()
    }
}

/// Reads every source in a thread, with the saved sort and settings.
fn scan() -> Task<Message> {
    Task::perform(
        blocking(|| {
            let sort = State::load().map(|state| state.sort).unwrap_or_default();
            let config = Config::load().map_err(|err| format!("{err:#}"));
            (Library::scan(), sort, config)
        }),
        |(library, sort, config)| Message::Loaded(library, sort, config),
    )
}

/// Runs blocking work (file reads, HTTP) on a thread of its own.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = oneshot::channel();
    thread::spawn(move || {
        let _ = sender.send(work());
    });
    receiver
        .await
        .expect("the worker thread sends before it ends")
}

/// Fetches each image on a thread (the requests block) and reports each one
/// as it lands, then `ImagesDone`.
fn fetch_images(mut fetcher: Fetcher, wanted: Vec<(Game, Kind)>) -> impl Stream<Item = Message> {
    iced::stream::channel(8, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        thread::spawn(move || {
            for (game, kind) in wanted {
                let result = fetcher.fetch(&game, kind).map_err(|err| format!("{err:#}"));
                if sender
                    .unbounded_send(Message::Image(game.id, kind, result))
                    .is_err()
                {
                    return;
                }
            }
        });
        while let Some(message) = receiver.next().await {
            if output.send(message).await.is_err() {
                return;
            }
        }
        let _ = output.send(Message::ImagesDone).await;
    })
}

/// Opens a folder or a web page in the user's program for it.
fn open(target: &str) {
    let child = Command::new("xdg-open")
        .arg(target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if let Ok(mut child) = child {
        thread::spawn(move || child.wait());
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).center(Length::Fill).into()
}

fn on_event(event: Event, status: event::Status, id: window::Id) -> Option<Message> {
    match event {
        Event::Window(window::Event::Resized(size)) => Some(Message::Resized(id, size)),
        Event::Window(window::Event::Focused) => Some(Message::Focused(id, true)),
        Event::Window(window::Event::Unfocused) => Some(Message::Focused(id, false)),
        // A key a text field used (typing, its cursor, Escape) is its own.
        Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modifiers,
            text,
            ..
        }) if status == event::Status::Ignored => key_message(&key, modifiers, text.as_deref()),
        _ => None,
    }
}

fn key_message(key: &Key, modifiers: keyboard::Modifiers, text: Option<&str>) -> Option<Message> {
    if let Key::Named(named) = key {
        let message = match named {
            Named::ArrowLeft => Message::Move(Move::Left),
            Named::ArrowRight => Message::Move(Move::Right),
            Named::ArrowUp => Message::Move(Move::Up),
            Named::ArrowDown => Message::Move(Move::Down),
            Named::PageUp => Message::Move(Move::PageUp),
            Named::PageDown => Message::Move(Move::PageDown),
            Named::Home => Message::Move(Move::First),
            Named::End => Message::Move(Move::Last),
            Named::Enter => Message::Activate,
            Named::Escape => Message::Back,
            Named::Backspace => Message::Backspace,
            Named::F1 => Message::Navigate(Page::About),
            Named::F5 => Message::Refresh,
            Named::F11 => Message::ToggleFullscreen,
            Named::Tab => Message::CycleSort,
            Named::Delete => Message::ToggleHidden,
            Named::Space => Message::Type(" ".to_owned()),
            _ => return None,
        };
        return Some(message);
    }
    if modifiers.control() {
        let Key::Character(c) = key else {
            return None;
        };
        return match c.to_lowercase().as_str() {
            "h" => Some(Message::ShowHidden),
            "p" => Some(Message::TogglePinned),
            "f" => Some(Message::FocusSearch),
            "r" => Some(Message::Refresh),
            "," => Some(Message::Navigate(Page::Settings)),
            _ => None,
        };
    }
    if modifiers.alt() || modifiers.logo() {
        return None;
    }
    text.filter(|text| !text.chars().any(char::is_control))
        .map(|text| Message::Type(text.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zendo_with(count: usize, columns: usize) -> Zendo {
        let games = (0..count)
            .map(|n| {
                Game::new(
                    format!("desktop:{n}"),
                    format!("Game {n:02}"),
                    if n % 2 == 0 {
                        Source::Desktop
                    } else {
                        Source::Steam
                    },
                    zendo_core::Launch::Uri { uri: String::new() },
                )
            })
            .collect();
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.scanning = false;
        zendo.sort = Sort::Title;
        zendo.library = Some(Library {
            games,
            problems: Vec::new(),
        });
        zendo.columns = columns;
        zendo
    }

    fn titles(zendo: &Zendo) -> Vec<&str> {
        zendo
            .visible()
            .iter()
            .map(|game| game.title.as_str())
            .collect()
    }

    #[test]
    fn hiding_leaves_a_game_out_until_hidden_games_are_shown() {
        let mut zendo = zendo_with(3, 3);
        zendo.selected = 1;
        let _ = zendo.update(Message::ToggleHidden);
        assert_eq!(titles(&zendo), ["Game 00", "Game 02"]);
        assert_eq!(zendo.selected, 1, "the cursor stays in place");
        // The cursor is on Game 02 now, and follows it when the hidden
        // game comes back between them.
        let _ = zendo.update(Message::ShowHidden);
        assert_eq!(titles(&zendo), ["Game 00", "Game 01", "Game 02"]);
        assert_eq!(zendo.selected, 2);
        // Bringing Game 01 back keeps it once hidden games are left out.
        zendo.selected = 1;
        let _ = zendo.update(Message::ToggleHidden);
        let _ = zendo.update(Message::ShowHidden);
        assert_eq!(titles(&zendo), ["Game 00", "Game 01", "Game 02"]);
    }

    #[test]
    fn the_cursor_follows_its_game_when_the_sort_changes() {
        let mut zendo = zendo_with(3, 3);
        if let Some(library) = zendo.library.as_mut() {
            library.games[2].played.last = Some(100);
        }
        zendo.selected = 2;
        // A–Z, then Recent: Game 02 was played, so it comes first.
        let _ = zendo.update(Message::CycleSort);
        assert_eq!(zendo.sort, Sort::Recent);
        assert_eq!(titles(&zendo)[0], "Game 02");
        assert_eq!(zendo.selected, 0);
    }

    #[test]
    fn a_pinned_game_leads_and_keeps_the_cursor() {
        let mut zendo = zendo_with(3, 3);
        zendo.selected = 2;
        let _ = zendo.update(Message::TogglePinned);
        assert_eq!(titles(&zendo), ["Game 02", "Game 00", "Game 01"]);
        assert_eq!(zendo.selected, 0);
    }

    #[test]
    fn store_chips_filter_the_grid() {
        let mut zendo = zendo_with(4, 3);
        let _ = zendo.update(Message::SetSource(Some(Source::Steam)));
        assert_eq!(titles(&zendo), ["Game 01", "Game 03"]);
        let _ = zendo.update(Message::SetSource(None));
        assert_eq!(zendo.visible().len(), 4);
    }

    #[test]
    fn enter_opens_a_game_and_escape_comes_back() {
        let mut zendo = zendo_with(3, 3);
        zendo.selected = 1;
        let _ = zendo.update(Message::Activate);
        assert_eq!(zendo.page, Page::Game("desktop:1".into()));
        // Arrows and typing do nothing on a game's page.
        let _ = zendo.update(Message::Move(Move::Right));
        let _ = zendo.update(Message::Type("x".into()));
        assert_eq!((zendo.selected, zendo.search.as_str()), (1, ""));
        // Enter there plays it.
        let _ = zendo.update(Message::Activate);
        assert_eq!(zendo.game("desktop:1").map(|g| g.played.launches), Some(1));
        let _ = zendo.update(Message::Back);
        assert_eq!(zendo.page, Page::Library);
        assert_eq!(zendo.selected, 1);
    }

    #[test]
    fn the_controller_pins_on_a_game_page_and_sorts_in_the_library() {
        let mut zendo = zendo_with(3, 3);
        let _ = zendo.update(Message::Pad(Pad::Sort));
        assert_eq!(zendo.sort, Sort::Recent);
        let _ = zendo.update(Message::Pad(Pad::Activate));
        let _ = zendo.update(Message::Pad(Pad::Sort));
        assert_eq!(zendo.game("desktop:0").map(|g| g.pinned), Some(true));
    }

    fn after(zendo: &mut Zendo, from: usize, step: Move) -> usize {
        zendo.selected = from;
        let _ = zendo.step(step);
        zendo.selected
    }

    #[test]
    fn arrows_move_through_the_grid() {
        // Seven games in three columns: rows 0-2, 3-5 and a short 6.
        let mut zendo = zendo_with(7, 3);
        assert_eq!(after(&mut zendo, 1, Move::Up), 1);
        assert_eq!(after(&mut zendo, 1, Move::Down), 4);
        assert_eq!(after(&mut zendo, 5, Move::Down), 6);
        assert_eq!(after(&mut zendo, 6, Move::Down), 6);
        assert_eq!(after(&mut zendo, 6, Move::Right), 6);
        assert_eq!(after(&mut zendo, 0, Move::Left), 0);
        assert_eq!(after(&mut zendo, 2, Move::Last), 6);
    }

    #[test]
    fn typing_filters_by_every_word() {
        let mut zendo = zendo_with(12, 3);
        let _ = zendo.update(Message::Type("game 1".into()));
        assert_eq!(titles(&zendo), ["Game 01", "Game 10", "Game 11"]);
        let _ = zendo.update(Message::Back);
        assert_eq!(zendo.visible().len(), 12);
    }

    #[test]
    fn a_key_is_kept_only_once_steamgriddb_takes_it() {
        let mut zendo = zendo_with(1, 3);
        let _ = zendo.update(Message::Navigate(Page::Settings));
        let _ = zendo.update(Message::Type("bad".into()));
        let _ = zendo.update(Message::SaveKey);
        let _ = zendo.update(Message::KeyChecked(Ok(KeyCheck::Rejected)));
        assert_eq!(zendo.config.steamgriddb_key, None);

        let _ = zendo.update(Message::KeyInput(" good ".into()));
        let _ = zendo.update(Message::SaveKey);
        let _ = zendo.update(Message::KeyChecked(Ok(KeyCheck::Accepted)));
        assert_eq!(zendo.config.steamgriddb_key.as_deref(), Some("good"));

        // Offline, Save still keeps it; Check alone never does.
        let _ = zendo.update(Message::KeyInput("other".into()));
        let _ = zendo.update(Message::CheckKey);
        let _ = zendo.update(Message::KeyChecked(Err("offline".into())));
        assert_eq!(zendo.config.steamgriddb_key.as_deref(), Some("good"));
        let _ = zendo.update(Message::SaveKey);
        let _ = zendo.update(Message::KeyChecked(Err("offline".into())));
        assert_eq!(zendo.config.steamgriddb_key.as_deref(), Some("other"));

        let _ = zendo.update(Message::KeyInput(String::new()));
        let _ = zendo.update(Message::SaveKey);
        assert_eq!(zendo.config.steamgriddb_key, None);
    }
    #[test]
    fn keys_a_field_used_are_left_to_it() {
        let enter = Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::Enter),
            modified_key: Key::Named(Named::Enter),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        });
        let id = window::Id::unique();
        assert!(on_event(enter.clone(), event::Status::Captured, id).is_none());
        assert!(matches!(
            on_event(enter, event::Status::Ignored, id),
            Some(Message::Activate)
        ));
    }
}
