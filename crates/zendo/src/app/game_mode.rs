//! Game Mode: the full-screen, controller-first way in. It replaces the old
//! F11 and opens on a screen of one game large (its hero) over a row of
//! covers (a carousel) taken from a shelf: what was played lately, the
//! up-next queue, or every game. The other pages keep their normal look.

use iced::widget::{button, column, container, image, row, space, text};
use iced::{Alignment, Background, ContentFit, Element, Length, Task, window};
use zendo_core::Game;

use super::launch_options::{frame, overlay};
use super::queue::{QueueMessage, meta_line, played_games, queued_games};
use super::style::{self, BOLD, SPACE_M, SPACE_S, SPACE_XL, SPACE_XS, colors};
use super::{Message, Move, Page, Zendo, now};
use crate::i18n::t;

const HERO_COVER_WIDTH: f32 = 260.0;
const HERO_COVER_HEIGHT: f32 = 390.0;
const TILE_WIDTH: f32 = 110.0;
const TILE_HEIGHT: f32 = 165.0;
const TILE_PADDING: f32 = 4.0;
/// The most lines of a description the hero shows.
const DESCRIPTION_LINES: usize = 4;

/// What the carousel lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shelf {
    /// The games played lately, the latest first.
    Continue,
    /// The user's own queue.
    UpNext,
    /// Every game the library shows.
    All,
}

impl Shelf {
    const ALL: [Self; 3] = [Self::Continue, Self::UpNext, Self::All];

    fn label(self) -> String {
        match self {
            Self::Continue => t!("shelf-continue"),
            Self::UpNext => t!("shelf-up-next"),
            Self::All => t!("gm-all-games"),
        }
    }
}

/// Game Mode, while it is on.
#[derive(Debug, Clone)]
pub(super) struct GameMode {
    pub shelf: Shelf,
    /// The game in the hero: an index into the shelf.
    pub index: usize,
    /// The Start menu, open on this choice.
    pub menu: Option<usize>,
    /// The library's own grid is showing, not the Game Mode screen.
    pub browsing: bool,
    /// The page to go back to when Game Mode ends.
    pub came_from: Page,
}

impl GameMode {
    pub(super) fn new(came_from: Page) -> Self {
        Self {
            shelf: Shelf::Continue,
            index: 0,
            menu: None,
            browsing: false,
            came_from,
        }
    }
}

/// The Start menu's choices, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuChoice {
    Resume,
    Library,
    Leave,
}

impl MenuChoice {
    const ALL: [Self; 3] = [Self::Resume, Self::Library, Self::Leave];

    fn label(self) -> String {
        match self {
            Self::Resume => t!("gm-resume"),
            Self::Library => t!("gm-to-library"),
            Self::Leave => t!("gm-leave"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum GameModeMessage {
    /// The next shelf (positive) or the one before.
    CycleShelf(isize),
    SetShelf(Shelf),
    /// The carousel's tile at this place.
    Pick(usize),
    /// Puts the game in the hero in the up-next queue, or takes it out.
    ToggleQueued,
    OpenMenu,
    MenuChoose(MenuChoice),
    /// Back to the Game Mode screen, from the grid or another page.
    Show,
}

/// How many tiles fit in a window this wide, odd so one sits in the middle.
fn tiles_that_fit(width: f32) -> usize {
    let room = (width - 2.0 * SPACE_XL).max(TILE_WIDTH);
    let each = TILE_WIDTH + 2.0 * TILE_PADDING + SPACE_S;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a count of tiles"
    )]
    let fit = (room / each) as usize;
    fit.max(1) | 1
}

/// The first of `count` tiles shown around `index` among `len`: centred, and
/// held inside the shelf at both ends.
fn first_tile(index: usize, len: usize, count: usize) -> usize {
    index
        .saturating_sub(count / 2)
        .min(len.saturating_sub(count))
}

impl Zendo {
    /// Whether the Game Mode screen is what the window shows: Game Mode is
    /// on, the page is the Library's and its own grid was not asked for.
    pub(super) fn game_mode_screen(&self) -> bool {
        self.page == Page::Library && self.game_mode.as_ref().is_some_and(|mode| !mode.browsing)
    }

    /// The games on a shelf, in its order.
    fn shelf_games(&self, which: Shelf) -> Vec<&Game> {
        let Some(library) = &self.library else {
            return Vec::new();
        };
        match which {
            Shelf::Continue => played_games(&library.games),
            Shelf::UpNext => queued_games(&library.games, &self.up_next),
            Shelf::All => library.games.iter().filter(|game| !game.hidden).collect(),
        }
    }

    /// The shelf shown: the one chosen, or the next with games when it is
    /// empty (Continue with nothing played shows every game).
    pub(super) fn shown_shelf(&self) -> Shelf {
        let chosen = self
            .game_mode
            .as_ref()
            .map_or(Shelf::All, |mode| mode.shelf);
        if !self.shelf_games(chosen).is_empty() {
            return chosen;
        }
        Shelf::ALL
            .into_iter()
            .find(|shelf| !self.shelf_games(*shelf).is_empty())
            .unwrap_or(chosen)
    }

    /// The game in the hero.
    pub(super) fn hero_game(&self) -> Option<&Game> {
        let index = self.game_mode.as_ref()?.index;
        let games = self.shelf_games(self.shown_shelf());
        games.get(index.min(games.len().saturating_sub(1))).copied()
    }

    /// F11 and Select: Game Mode on or off. On: full screen, on its screen;
    /// off: the window again, on the page it was left from.
    pub(super) fn toggle_game_mode(&mut self) -> Task<Message> {
        if self.game_mode.is_some() {
            self.leave_game_mode()
        } else {
            self.enter_game_mode()
        }
    }

    fn enter_game_mode(&mut self) -> Task<Message> {
        let Some(id) = self.window else {
            return Task::none();
        };
        let came_from = self.page.clone();
        self.fullscreen = true;
        let page = self.navigate(Page::Library);
        self.game_mode = Some(GameMode::new(came_from));
        Task::batch([window::set_mode(id, window::Mode::Fullscreen), page])
    }

    fn leave_game_mode(&mut self) -> Task<Message> {
        let Some(id) = self.window else {
            return Task::none();
        };
        let back = self
            .game_mode
            .take()
            .map_or(Page::Library, |mode| mode.came_from);
        self.fullscreen = false;
        let page = self.navigate(back);
        Task::batch([window::set_mode(id, window::Mode::Windowed), page])
    }

    /// Begins in Game Mode when Zendo was started with `--game-mode`: the
    /// window is made full screen already.
    pub(super) fn start_in_game_mode(&mut self) {
        self.game_mode = Some(GameMode::new(Page::Library));
        self.fullscreen = true;
    }

    /// What the Game Mode screen does with a key or a button. `None` leaves
    /// it to the page (when the screen is not showing).
    pub(super) fn game_mode_takes(&mut self, message: &Message) -> Option<Task<Message>> {
        if !self.game_mode_screen() {
            return None;
        }
        let menu_open = self
            .game_mode
            .as_ref()
            .is_some_and(|mode| mode.menu.is_some());
        match message {
            Message::Move(step) if menu_open => {
                self.move_menu(*step);
                Some(Task::none())
            }
            Message::Move(Move::Left) => Some(self.step_hero(-1)),
            Message::Move(Move::Right) => Some(self.step_hero(1)),
            Message::Activate if menu_open => Some(self.choose_menu()),
            Message::Activate => Some(self.play_hero()),
            Message::Back if menu_open => {
                self.set_menu(None);
                Some(Task::none())
            }
            Message::Back => {
                self.set_menu(Some(0));
                Some(Task::none())
            }
            Message::CycleSort => Some(self.cycle_shelf(1)),
            // Space: the up-next queue, as the controller's Y.
            Message::Type(typed) if typed == " " => Some(self.queue_hero()),
            Message::Move(_)
            | Message::Type(_)
            | Message::Backspace
            | Message::TogglePinned
            | Message::ToggleHidden
            | Message::ShowHidden
            | Message::FocusSearch
            | Message::ToggleSelected
            | Message::Pasted(_) => Some(Task::none()),
            _ => None,
        }
    }

    pub(super) fn update_game_mode(&mut self, message: GameModeMessage) -> Task<Message> {
        match message {
            GameModeMessage::CycleShelf(by) => self.cycle_shelf(by),
            GameModeMessage::SetShelf(chosen) => {
                if let Some(mode) = &mut self.game_mode {
                    mode.shelf = chosen;
                    mode.index = 0;
                }
                Task::none()
            }
            GameModeMessage::Pick(index) => {
                if let Some(mode) = &mut self.game_mode {
                    mode.index = index;
                }
                Task::none()
            }
            GameModeMessage::ToggleQueued => self.queue_hero(),
            GameModeMessage::OpenMenu => {
                self.set_menu(Some(0));
                Task::none()
            }
            GameModeMessage::MenuChoose(choice) => self.run_choice(choice),
            GameModeMessage::Show => {
                if let Some(mode) = &mut self.game_mode {
                    mode.browsing = false;
                }
                self.navigate(Page::Library)
            }
        }
    }

    fn set_menu(&mut self, menu: Option<usize>) {
        if let Some(mode) = &mut self.game_mode {
            mode.menu = menu;
        }
    }

    fn move_menu(&mut self, step: Move) {
        let Some(mode) = &mut self.game_mode else {
            return;
        };
        let Some(at) = mode.menu else {
            return;
        };
        let last = MenuChoice::ALL.len() - 1;
        mode.menu = Some(match step {
            Move::Up | Move::Left => at.saturating_sub(1),
            Move::Down | Move::Right => (at + 1).min(last),
            Move::PageUp | Move::First => 0,
            Move::PageDown | Move::Last => last,
        });
    }

    fn choose_menu(&mut self) -> Task<Message> {
        let at = self.game_mode.as_ref().and_then(|mode| mode.menu);
        match at.and_then(|at| MenuChoice::ALL.get(at).copied()) {
            Some(choice) => self.run_choice(choice),
            None => Task::none(),
        }
    }

    fn run_choice(&mut self, choice: MenuChoice) -> Task<Message> {
        self.set_menu(None);
        match choice {
            MenuChoice::Resume => Task::none(),
            MenuChoice::Library => {
                if let Some(mode) = &mut self.game_mode {
                    mode.browsing = true;
                }
                Task::none()
            }
            MenuChoice::Leave => self.leave_game_mode(),
        }
    }

    /// One tile along, stopping at the ends.
    fn step_hero(&mut self, by: isize) -> Task<Message> {
        let len = self.shelf_games(self.shown_shelf()).len();
        let shown = self.shown_shelf();
        if let Some(mode) = &mut self.game_mode {
            mode.shelf = shown;
            mode.index = mode
                .index
                .saturating_add_signed(by)
                .min(len.saturating_sub(1));
        }
        Task::none()
    }

    /// The next shelf with games (or the one before), from the first game.
    fn cycle_shelf(&mut self, by: isize) -> Task<Message> {
        let current = self.shown_shelf();
        let at = Shelf::ALL
            .iter()
            .position(|shelf| *shelf == current)
            .unwrap_or(0);
        let count = Shelf::ALL.len();
        for turn in 1..=count {
            #[expect(clippy::cast_possible_wrap, reason = "three shelves")]
            let next = (at as isize + by * turn as isize).rem_euclid(count as isize) as usize;
            let target = Shelf::ALL[next];
            if !self.shelf_games(target).is_empty() {
                if let Some(mode) = &mut self.game_mode {
                    mode.shelf = target;
                    mode.index = 0;
                }
                break;
            }
        }
        Task::none()
    }

    fn play_hero(&mut self) -> Task<Message> {
        match self.hero_game().cloned() {
            Some(game) => self.start_game(game),
            None => Task::none(),
        }
    }

    fn queue_hero(&mut self) -> Task<Message> {
        if let Some(id) = self.hero_game().map(|game| game.id.clone()) {
            self.toggle_queued(&id);
        }
        Task::none()
    }

    /// The whole screen: the shelves, the hero, the carousel and the keys.
    pub(super) fn game_mode_view(&self) -> Element<'_, Message> {
        let shown = self.shown_shelf();
        let games = self.shelf_games(shown);
        let index = self
            .game_mode
            .as_ref()
            .map_or(0, |mode| mode.index)
            .min(games.len().saturating_sub(1));

        let shelves = row(Shelf::ALL.map(|shelf| {
            let count = self.shelf_games(shelf).len();
            super::library::chip(
                shelf.label(),
                Some(count),
                shelf == shown,
                Message::GameMode(GameModeMessage::SetShelf(shelf)),
            )
        }))
        .spacing(SPACE_S)
        .wrap()
        .vertical_spacing(SPACE_S);
        let header = row![
            text("ZENDO").size(20).font(BOLD),
            text("•").size(20).color(colors().accent),
            space::horizontal().width(SPACE_XL),
            shelves,
            space::horizontal(),
            button(style::button_text(t!("gm-menu")))
                .padding(style::BUTTON)
                .style(style::secondary)
                .on_press(Message::GameMode(GameModeMessage::OpenMenu)),
        ]
        .spacing(SPACE_S)
        .align_y(Alignment::Center)
        .padding([SPACE_M, SPACE_XL]);

        let body: Element<'_, Message> = match games.get(index) {
            Some(game) => column![
                self.hero(game),
                space::vertical(),
                self.carousel(&games, index)
            ]
            .spacing(SPACE_XL)
            .into(),
            None => container(text(t!("gm-empty")).color(colors().dim))
                .center(Length::Fill)
                .into(),
        };

        let page = column![
            container(header)
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(colors().sidebar)),
                    ..container::Style::default()
                }),
            container(body)
                .padding([SPACE_XL, SPACE_XL])
                .width(Length::Fill)
                .height(Length::Fill),
            container(text(t!("hints-game-mode")).size(13).color(colors().dim))
                .padding([SPACE_S, SPACE_XL]),
        ]
        .width(Length::Fill);
        self.with_game_menu(page.into())
    }

    /// The game large: cover, meta line, title, a description and actions.
    fn hero<'a>(&'a self, game: &'a Game) -> Element<'a, Message> {
        let cover: Element<'a, Message> = match &game.cover {
            Some(path) => image(image::Handle::from_path(path))
                .width(HERO_COVER_WIDTH)
                .height(HERO_COVER_HEIGHT)
                .content_fit(ContentFit::Cover)
                .border_radius(style::RADIUS_PANEL)
                .into(),
            None => container(text(&game.title).size(22).center())
                .padding(SPACE_M)
                .center(Length::Fill)
                .width(HERO_COVER_WIDTH)
                .height(HERO_COVER_HEIGHT)
                .style(style::placeholder)
                .into(),
        };
        let id = game.id.clone();
        let queued = self.is_queued(&game.id);
        let actions = row![
            button(style::button_text(t!("run-play")).size(18).font(BOLD))
                .padding(style::BUTTON_LARGE)
                .style(style::primary)
                .on_press(Message::Queue(QueueMessage::Play(id.clone()))),
            button(style::button_text(t!("queue-details")))
                .padding(style::BUTTON)
                .style(style::secondary)
                .on_press(Message::Queue(QueueMessage::Details(id.clone()))),
            button(style::button_text(if queued {
                t!("queue-remove")
            } else {
                t!("queue-add")
            }))
            .padding(style::BUTTON)
            .style(style::secondary)
            .on_press(Message::GameMode(GameModeMessage::ToggleQueued)),
        ]
        .spacing(SPACE_S)
        .align_y(Alignment::Center);

        let description = describe(self, game);
        let info = column![
            text(meta_line(game, now())).size(13).color(colors().dim),
            text(&game.title).size(40).font(BOLD),
            container(text(description).size(15).line_height(1.4))
                .padding(SPACE_M)
                .width(Length::Fill)
                .max_height(120)
                .clip(true)
                .style(style::card),
            actions,
        ]
        .spacing(SPACE_M)
        .width(Length::Fill);
        row![cover, info].spacing(SPACE_XL).into()
    }

    /// A row of covers around the chosen one.
    fn carousel<'a>(&'a self, games: &[&'a Game], index: usize) -> Element<'a, Message> {
        let count = tiles_that_fit(self.window_width).min(games.len());
        let first = first_tile(index, games.len(), count);
        let tiles = (first..first + count).map(|at| {
            let game = games[at];
            let cover: Element<'a, Message> = match &game.cover {
                Some(path) => image(image::Handle::from_path(path))
                    .width(TILE_WIDTH)
                    .height(TILE_HEIGHT)
                    .content_fit(ContentFit::Cover)
                    .border_radius(style::RADIUS_PANEL)
                    .into(),
                None => container(text(&game.title).size(12).center())
                    .padding(SPACE_XS)
                    .center(Length::Fill)
                    .width(TILE_WIDTH)
                    .height(TILE_HEIGHT)
                    .style(style::placeholder)
                    .into(),
            };
            button(cover)
                .padding(TILE_PADDING)
                .style(style::card_button(false, at == index))
                .on_press(Message::GameMode(GameModeMessage::Pick(at)))
                .into()
        });
        container(row(tiles).spacing(SPACE_S).align_y(Alignment::Center))
            .center_x(Length::Fill)
            .into()
    }

    /// The Start menu over the screen, when it is open.
    fn with_game_menu<'a>(&'a self, page: Element<'a, Message>) -> Element<'a, Message> {
        let Some(at) = self.game_mode.as_ref().and_then(|mode| mode.menu) else {
            return page;
        };
        let choices = MenuChoice::ALL.iter().enumerate().map(|(place, choice)| {
            button(
                container(style::button_text(choice.label()))
                    .width(Length::Fill)
                    .center_x(Length::Fill),
            )
            .width(Length::Fill)
            .padding(style::BUTTON)
            .style(style::chip(place == at))
            .on_press(Message::GameMode(GameModeMessage::MenuChoose(*choice)))
            .into()
        });
        let footer = text(t!("gm-menu-hint")).size(13).color(colors().dim);
        overlay(
            page,
            frame(
                &t!("gm-menu"),
                column(choices).spacing(SPACE_S),
                footer,
                420.0,
                false,
            ),
        )
    }
}

/// What the hero says about a game: the store's summary when there is one,
/// else who made it and for what. (A description for every game is item 38.)
fn describe(zendo: &Zendo, game: &Game) -> String {
    if let Some(summary) = zendo
        .store
        .owned(&game.id)
        .and_then(|owned| owned.summary.as_deref())
    {
        let lines: Vec<&str> = summary
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        return lines
            .into_iter()
            .take(DESCRIPTION_LINES)
            .collect::<Vec<_>>()
            .join("\n");
    }
    let mut parts: Vec<String> = Vec::new();
    parts.extend(game.details.developer.clone());
    parts.extend(game.details.platform.clone());
    if parts.is_empty() {
        t!("gm-no-description")
    } else {
        parts.join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_carousel_shows_an_odd_number_of_tiles() {
        for width in [960.0, 1360.0, 1920.0, 2560.0] {
            assert_eq!(tiles_that_fit(width) % 2, 1, "{width}");
        }
        assert!(tiles_that_fit(1920.0) > tiles_that_fit(960.0));
    }

    #[test]
    fn the_tiles_are_centred_and_held_inside_the_shelf() {
        // 20 games, 7 tiles: the 10th is in the middle.
        assert_eq!(first_tile(10, 20, 7), 7);
        assert_eq!(first_tile(0, 20, 7), 0, "at the start");
        assert_eq!(first_tile(19, 20, 7), 13, "at the end");
        assert_eq!(first_tile(2, 4, 4), 0, "a shelf no longer than the row");
    }
}
