//! The library window: a grid of covers, filtered by typing, moved through
//! with the arrows or a controller, and a game started with Enter or A.
//! Missing covers are downloaded in the background once the library is in.

use std::path::PathBuf;
use std::thread;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::scrollable::{AbsoluteOffset, Viewport};
use iced::widget::{Id, button, column, container, image, operation, row, scrollable, space, text};
use iced::{
    Background, Border, Color, ContentFit, Element, Event, Length, Size, Subscription, Task, Theme,
    event, window,
};
use zendo_core::config::Config;
use zendo_core::covers::Fetcher;
use zendo_core::{Game, Library, Sort, State};

use crate::gamepad::{self, Pad};

const COVER_WIDTH: f32 = 180.0;
const COVER_HEIGHT: f32 = 270.0;
const TITLE_HEIGHT: f32 = 40.0;
const CARD_PADDING: f32 = 6.0;
const CARD_WIDTH: f32 = COVER_WIDTH + 2.0 * CARD_PADDING;
const CARD_HEIGHT: f32 = COVER_HEIGHT + TITLE_HEIGHT + 2.0 * CARD_PADDING;
const SPACING: f32 = 16.0;
const PADDING: f32 = 24.0;
/// Room kept for the scrollbar beside the grid.
const SCROLLBAR: f32 = 12.0;
const START_SIZE: Size = Size::new(1280.0, 820.0);

const BACKGROUND: Color = Color::from_rgb8(0x17, 0x17, 0x17);
const SURFACE: Color = Color::from_rgb8(0x22, 0x22, 0x22);
const TEXT: Color = Color::from_rgb8(0xcc, 0xd0, 0xcf);
const DIM: Color = Color::from_rgb8(0x80, 0x84, 0x83);
const ACCENT: Color = Color::from_rgb8(0xf2, 0x56, 0x23);

fn grid_id() -> Id {
    Id::new("grid")
}

pub fn run(fullscreen: bool) -> anyhow::Result<()> {
    iced::application(Zendo::boot, Zendo::update, Zendo::view)
        .title("Zendo")
        .theme(theme())
        .subscription(Zendo::subscription)
        .window(window::Settings {
            size: START_SIZE,
            min_size: Some(Size::new(CARD_WIDTH + 2.0 * PADDING + SCROLLBAR, 480.0)),
            fullscreen,
            ..window::Settings::default()
        })
        .run()?;
    Ok(())
}

fn theme() -> Theme {
    Theme::custom(
        "Zendo",
        iced::theme::Palette {
            background: BACKGROUND,
            text: TEXT,
            primary: ACCENT,
            success: Color::from_rgb8(0x7f, 0xb0, 0x69),
            warning: Color::from_rgb8(0xe0, 0xb0, 0x4c),
            danger: Color::from_rgb8(0xe0, 0x5a, 0x4c),
        },
    )
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
    Loaded(Library, Sort),
    /// A cover download finished: the file, none found, or what went wrong.
    Cover(String, Result<Option<PathBuf>, String>),
    CoversDone,
    Resized(window::Id, Size),
    Focused(window::Id, bool),
    Scrolled(Viewport),
    Move(Move),
    Type(String),
    Backspace,
    Back,
    Activate,
    Clicked(usize),
    CycleSort,
    /// Hides the selected game, or brings it back when it is hidden.
    ToggleHidden,
    ShowHidden,
    ToggleFullscreen,
    Pad(Pad),
}

#[expect(clippy::struct_excessive_bools, reason = "independent window flags")]
struct Zendo {
    /// `None` while the first scan runs.
    library: Option<Library>,
    filter: String,
    /// Index into the games the filter lets through.
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
    covers_left: usize,
    covers_failed: usize,
    /// Off in tests, so they never write the user's state file.
    persist: bool,
}

impl Zendo {
    fn boot() -> (Self, Task<Message>) {
        let zendo = Self {
            library: None,
            filter: String::new(),
            selected: 0,
            columns: columns_for(START_SIZE.width),
            window: None,
            focused: true,
            fullscreen: false,
            scroll_top: 0.0,
            viewport_height: START_SIZE.height,
            status: None,
            sort: Sort::default(),
            show_hidden: false,
            covers_left: 0,
            covers_failed: 0,
            persist: true,
        };
        let scan = Task::perform(
            async {
                let sort = State::load().map(|state| state.sort).unwrap_or_default();
                (Library::scan(), sort)
            },
            |(library, sort)| Message::Loaded(library, sort),
        );
        // The window's id, so F11 works before the first resize event.
        let window = window::latest().and_then(|id| Task::done(Message::Focused(id, true)));
        (zendo, Task::batch([scan, window]))
    }

    /// The games the grid shows, in its order.
    fn visible(&self) -> Vec<&Game> {
        let Some(library) = &self.library else {
            return Vec::new();
        };
        let words: Vec<String> = self
            .filter
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let mut games: Vec<&Game> = library
            .games
            .iter()
            .filter(|game| self.show_hidden || !game.hidden)
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

    /// Keeps the cursor on the same game after the order changes.
    fn select(&mut self, id: Option<&str>) -> Task<Message> {
        let visible = self.visible();
        let index = id.and_then(|id| visible.iter().position(|game| game.id == id));
        self.selected = index.unwrap_or_else(|| self.selected.min(visible.len().saturating_sub(1)));
        self.reveal()
    }

    fn game_mut(&mut self, id: &str) -> Option<&mut Game> {
        self.library
            .as_mut()?
            .games
            .iter_mut()
            .find(|game| game.id == id)
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
            Message::Loaded(library, sort) => {
                self.library = Some(library);
                self.sort = sort;
                self.selected = 0;
                self.download_covers()
            }
            Message::Cover(id, result) => {
                self.cover_arrived(&id, result);
                Task::none()
            }
            Message::CoversDone => {
                self.covers_done();
                Task::none()
            }
            Message::CycleSort => {
                let id = self.selected_id();
                self.sort = self.sort.next();
                let sort = self.sort;
                self.save(|state| state.sort = sort);
                self.select(id.as_deref())
            }
            Message::ToggleHidden => self.toggle_hidden(),
            Message::ShowHidden => {
                let id = self.selected_id();
                self.show_hidden = !self.show_hidden;
                self.select(id.as_deref())
            }
            Message::Resized(id, size) => {
                self.window = Some(id);
                self.columns = columns_for(size.width);
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
            Message::Move(step) => self.step(step),
            Message::Type(typed) => {
                self.filter.push_str(&typed);
                self.filter_changed()
            }
            Message::Backspace => {
                self.filter.pop();
                self.filter_changed()
            }
            Message::Back => {
                if self.filter.is_empty() {
                    Task::none()
                } else {
                    self.filter.clear();
                    self.filter_changed()
                }
            }
            Message::Activate => self.launch_selected(),
            Message::Clicked(index) => {
                self.selected = index;
                self.launch_selected()
            }
            Message::ToggleFullscreen => {
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
            // A controller keeps sending while a game has the screen; only
            // the window in front listens to it.
            Message::Pad(_) if !self.focused => Task::none(),
            Message::Pad(pad) => self.update(match pad {
                Pad::Left => Message::Move(Move::Left),
                Pad::Right => Message::Move(Move::Right),
                Pad::Up => Message::Move(Move::Up),
                Pad::Down => Message::Move(Move::Down),
                Pad::Activate => Message::Activate,
                Pad::Back => Message::Back,
                Pad::Sort => Message::CycleSort,
                Pad::Hide => Message::ToggleHidden,
                Pad::ShowHidden => Message::ShowHidden,
                Pad::ToggleFullscreen => Message::ToggleFullscreen,
            }),
        }
    }

    fn cover_arrived(&mut self, id: &str, result: Result<Option<PathBuf>, String>) {
        self.covers_left = self.covers_left.saturating_sub(1);
        match result {
            Ok(Some(path)) => {
                if let Some(game) = self.game_mut(id) {
                    game.cover = Some(path);
                }
            }
            Ok(None) => {}
            Err(_) => self.covers_failed += 1,
        }
    }

    fn covers_done(&mut self) {
        self.covers_left = 0;
        if self.covers_failed > 0 {
            self.status = Some(format!(
                "{} cover{} could not be downloaded; `zendo covers` says why",
                self.covers_failed,
                if self.covers_failed == 1 { "" } else { "s" }
            ));
        }
    }

    fn toggle_hidden(&mut self) -> Task<Message> {
        let Some(id) = self.selected_id() else {
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
        let rows = ((self.viewport_height + SPACING) / (CARD_HEIGHT + SPACING)) as usize;
        rows.max(1)
    }

    /// Scrolls just enough to show the selected card.
    fn reveal(&self) -> Task<Message> {
        #[expect(clippy::cast_precision_loss, reason = "row counts are small")]
        let row = (self.selected / self.columns.max(1)) as f32;
        let top = row * (CARD_HEIGHT + SPACING);
        let bottom = top + CARD_HEIGHT + 2.0 * PADDING;
        let y = if top < self.scroll_top {
            top
        } else if bottom > self.scroll_top + self.viewport_height {
            bottom - self.viewport_height
        } else {
            return Task::none();
        };
        operation::scroll_to(grid_id(), AbsoluteOffset { x: 0.0, y })
    }

    fn launch_selected(&mut self) -> Task<Message> {
        let Some(game) = self
            .visible()
            .get(self.selected)
            .map(|game| (*game).clone())
        else {
            return Task::none();
        };
        match zendo_core::launch(&game) {
            Ok(()) => {
                self.status = Some(format!("Starting {}", game.title));
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

    /// Starts downloading the covers that are missing, one at a time, in a
    /// thread of their own.
    fn download_covers(&mut self) -> Task<Message> {
        if !self.persist {
            return Task::none();
        }
        let config = match Config::load() {
            Ok(config) => config,
            Err(err) => {
                self.status = Some(format!("{err:#}"));
                return Task::none();
            }
        };
        let Some(fetcher) = Fetcher::new(&config) else {
            return Task::none();
        };
        let wanted: Vec<Game> = self
            .library
            .iter()
            .flat_map(|library| &library.games)
            .filter(|game| fetcher.wants(game))
            .cloned()
            .collect();
        if wanted.is_empty() {
            return Task::none();
        }
        self.covers_left = wanted.len();
        self.covers_failed = 0;
        Task::run(fetch_covers(fetcher, wanted), |message| message)
    }

    fn subscription(_: &Self) -> Subscription<Message> {
        Subscription::batch([
            event::listen_with(on_event),
            Subscription::run(gamepad::events).map(Message::Pad),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        let visible = self.visible();
        let header = row![
            text("Zendo").size(24),
            space::horizontal(),
            self.filter_line(),
            text(self.sort.label()).color(DIM),
            text(if self.show_hidden {
                format!("{} · hidden shown", count_label(visible.len()))
            } else {
                count_label(visible.len())
            })
            .color(DIM),
        ]
        .spacing(24)
        .align_y(iced::Alignment::Center)
        .padding([16.0, PADDING]);

        let body: Element<'_, Message> = match &self.library {
            None => centered(text("Looking for games…").color(DIM)),
            Some(library) if library.games.is_empty() => centered(
                column![
                    text("No games found").size(20),
                    text(
                        "Zendo reads Steam's libraries, the games Heroic installed \
                         and .desktop files in the Game category."
                    )
                    .color(DIM),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            ),
            Some(_) if visible.is_empty() && self.filter.is_empty() => {
                centered(text("Every game is hidden · Ctrl+H or Start shows them").color(DIM))
            }
            Some(_) if visible.is_empty() => {
                centered(text(format!("Nothing matches “{}”", self.filter)).color(DIM))
            }
            Some(_) => self.grid(&visible),
        };

        column![header, body, self.footer()].into()
    }

    fn filter_line(&self) -> Element<'_, Message> {
        if self.filter.is_empty() {
            text("Type to filter").color(DIM).into()
        } else {
            row![text("Filter").color(DIM), text(&self.filter).color(ACCENT)]
                .spacing(8)
                .into()
        }
    }

    fn grid<'a>(&'a self, visible: &[&'a Game]) -> Element<'a, Message> {
        let columns = self.columns.max(1);
        let selected = self.selected.min(visible.len().saturating_sub(1));
        let rows = visible
            .chunks(columns)
            .enumerate()
            .map(|(row_index, games)| {
                row(games.iter().enumerate().map(|(column_index, game)| {
                    let index = row_index * columns + column_index;
                    card(game, index, index == selected)
                }))
                .spacing(SPACING)
                .into()
            });
        // A fixed width, centred, so a short last row stays in its columns.
        #[expect(clippy::cast_precision_loss, reason = "column counts are small")]
        let width = columns as f32 * (CARD_WIDTH + SPACING) - SPACING;
        let grid = column(rows).spacing(SPACING).width(width);
        scrollable(container(grid).center_x(Length::Fill).padding(PADDING))
            .id(grid_id())
            .on_scroll(Message::Scrolled)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn footer(&self) -> Element<'_, Message> {
        let problems = self
            .library
            .as_ref()
            .map_or(0, |library| library.problems.len());
        let left = match (&self.status, problems) {
            _ if self.covers_left > 0 => {
                text(format!("Downloading covers, {} left", self.covers_left))
            }
            (Some(status), _) => text(status.clone()),
            (None, 0) => text(""),
            (None, n) => text(format!(
                "{n} file{} could not be read; `zendo list` names them",
                if n == 1 { "" } else { "s" }
            )),
        };
        row![
            left.color(DIM),
            space::horizontal(),
            text("Enter play · Tab sort · Del hide · Ctrl+H hidden · F11 full-screen").color(DIM),
        ]
        .spacing(16)
        .padding([10.0, PADDING])
        .into()
    }
}

fn card<'a>(game: &'a Game, index: usize, selected: bool) -> Element<'a, Message> {
    let cover: Element<'a, Message> = match &game.cover {
        Some(path) => image(image::Handle::from_path(path))
            .width(COVER_WIDTH)
            .height(COVER_HEIGHT)
            .content_fit(ContentFit::Cover)
            .opacity(if game.hidden { 0.35_f32 } else { 1.0_f32 })
            .into(),
        None => container(text(&game.title).size(18).center())
            .padding(12)
            .center(Length::Fill)
            .width(COVER_WIDTH)
            .height(COVER_HEIGHT)
            .style(|_| container::Style {
                background: Some(Background::Color(SURFACE)),
                border: Border {
                    radius: 6.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .into(),
    };
    let caption = column![
        text(&game.title).size(14),
        if game.hidden {
            text(format!("{} · hidden", game.source.label()))
                .size(12)
                .color(ACCENT)
        } else {
            text(game.source.label()).size(12).color(DIM)
        },
    ]
    .spacing(2)
    .height(TITLE_HEIGHT)
    .clip(true);

    button(column![cover, caption].spacing(4))
        .padding(CARD_PADDING)
        .width(CARD_WIDTH)
        .height(CARD_HEIGHT)
        .on_press(Message::Clicked(index))
        .style(move |_, status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered | button::Status::Pressed => SURFACE,
                _ => Color::TRANSPARENT,
            })),
            text_color: TEXT,
            border: Border {
                color: if selected { ACCENT } else { Color::TRANSPARENT },
                width: 2.0,
                radius: 10.0.into(),
            },
            ..button::Style::default()
        })
        .into()
}

/// Fetches each game's cover on a thread (the requests block) and reports
/// each one as it lands, then `CoversDone`.
fn fetch_covers(mut fetcher: Fetcher, games: Vec<Game>) -> impl Stream<Item = Message> {
    iced::stream::channel(8, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        thread::spawn(move || {
            for game in games {
                let result = fetcher.fetch(&game).map_err(|err| format!("{err:#}"));
                if sender
                    .unbounded_send(Message::Cover(game.id, result))
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
        let _ = output.send(Message::CoversDone).await;
    })
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).center(Length::Fill).into()
}

fn count_label(count: usize) -> String {
    match count {
        1 => "1 game".to_owned(),
        n => format!("{n} games"),
    }
}

/// How many cards fit across a window this wide.
fn columns_for(width: f32) -> usize {
    let room = width - 2.0 * PADDING - SCROLLBAR + SPACING;
    // Widths are positive and far below usize's range.
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let columns = (room / (CARD_WIDTH + SPACING)).floor().max(1.0) as usize;
    columns
}

fn on_event(event: Event, _status: event::Status, id: window::Id) -> Option<Message> {
    match event {
        Event::Window(window::Event::Resized(size)) => Some(Message::Resized(id, size)),
        Event::Window(window::Event::Focused) => Some(Message::Focused(id, true)),
        Event::Window(window::Event::Unfocused) => Some(Message::Focused(id, false)),
        Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modifiers,
            text,
            ..
        }) => key_message(&key, modifiers, text.as_deref()),
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
            Named::F11 => Message::ToggleFullscreen,
            Named::Tab => Message::CycleSort,
            Named::Delete => Message::ToggleHidden,
            Named::Space => Message::Type(" ".to_owned()),
            _ => return None,
        };
        return Some(message);
    }
    if modifiers.control() && matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("h")) {
        return Some(Message::ShowHidden);
    }
    if modifiers.control() || modifiers.alt() || modifiers.logo() {
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
                    zendo_core::Source::Desktop,
                    zendo_core::Launch::Uri { uri: String::new() },
                )
            })
            .collect();
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
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
        let titles: Vec<&str> = zendo
            .visible()
            .iter()
            .map(|game| game.title.as_str())
            .collect();
        assert_eq!(titles, ["Game 01", "Game 10", "Game 11"]);
        let _ = zendo.update(Message::Back);
        assert_eq!(zendo.visible().len(), 12);
    }

    #[test]
    fn fits_whole_cards_across() {
        assert_eq!(columns_for(100.0), 1);
        let five = 2.0 * PADDING + SCROLLBAR + 5.0 * CARD_WIDTH + 4.0 * SPACING;
        assert_eq!(columns_for(five), 5);
        assert_eq!(columns_for(five - 1.0), 4);
    }
}
